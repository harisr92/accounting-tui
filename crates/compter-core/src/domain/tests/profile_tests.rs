use crate::domain::{
    BasisSchedule, BusinessProfile, Currency, DomainError, FiscalYear, IncomeBasis, LogoId,
    ProfileDraft,
};

fn draft() -> ProfileDraft {
    ProfileDraft {
        name: " Harin Dev Studio ".to_owned(),
        address: vec![
            "12 MG Road".to_owned(),
            String::new(),
            "Bengaluru 560001".to_owned(),
        ],
        gstin: Some("29aapfu0939f1zr".to_owned()),
        sac_code: "998314".to_owned(),
        invoice_prefix: "inv".to_owned(),
        lut_arn: Some("  ".to_owned()),
        terms: vec!["Bank: HDFC 0001234".to_owned()],
        ..Default::default()
    }
}

#[test]
fn normalises_a_valid_profile() {
    let profile = BusinessProfile::try_from(draft()).unwrap();
    assert_eq!(profile.name(), "Harin Dev Studio");
    assert_eq!(profile.address(), ["12 MG Road", "Bengaluru 560001"]);
    assert_eq!(profile.gstin().map(|g| g.as_str()), Some("29AAPFU0939F1ZR"));
    assert_eq!(profile.invoice_prefix(), "INV");
    assert_eq!(profile.lut_arn(), None);
    assert_eq!(
        profile.gst_rate().map(ToString::to_string).as_deref(),
        Some("18")
    );
}

#[test]
fn blank_gstin_means_unregistered() {
    let mut d = draft();
    d.gstin = Some("   ".to_owned());
    assert!(BusinessProfile::try_from(d).unwrap().gstin().is_none());
}

#[test]
fn rejects_bad_settings() {
    let with = |change: fn(&mut ProfileDraft)| {
        let mut d = draft();
        change(&mut d);
        BusinessProfile::try_from(d)
    };
    assert!(matches!(
        with(|d| d.address.clear()),
        Err(DomainError::EmptyField("business address"))
    ));
    assert!(matches!(
        with(|d| d.sac_code = "123456".to_owned()),
        Err(DomainError::UnknownSacCode(_))
    ));
    assert!(matches!(
        with(|d| d.invoice_prefix = "TOOLONG".to_owned()),
        Err(DomainError::InvalidInvoicePrefix)
    ));
    assert!(matches!(
        with(|d| d.invoice_prefix = "IN/V".to_owned()),
        Err(DomainError::InvalidInvoicePrefix)
    ));
    assert!(matches!(
        with(|d| d.gstin = Some("nope".to_owned())),
        Err(DomainError::InvalidGstin(_))
    ));
}

#[test]
fn round_trips_through_its_draft() {
    let profile = BusinessProfile::try_from(draft()).unwrap();
    assert_eq!(
        BusinessProfile::try_from(profile.to_draft()).unwrap(),
        profile
    );
    let json = serde_json::to_string(&profile).unwrap();
    assert_eq!(
        serde_json::from_str::<BusinessProfile>(&json).unwrap(),
        profile
    );
}

#[test]
fn the_default_currency_is_kept_and_older_profiles_are_in_rupees() {
    let mut d = draft();
    d.currency = Currency::Eur;
    let profile = BusinessProfile::try_from(d).unwrap();
    assert_eq!(profile.currency(), Currency::Eur);
    assert_eq!(profile.to_draft().currency, Currency::Eur);

    let mut json = serde_json::to_value(&profile).unwrap();
    json.as_object_mut().unwrap().remove("currency");
    let read = serde_json::from_value::<BusinessProfile>(json).unwrap();
    assert_eq!(read.currency(), Currency::Inr);
}

#[test]
fn logo_survives_editing_and_older_records_have_none() {
    let logo = LogoId::generate();
    let profile = BusinessProfile::try_from(ProfileDraft {
        logo: Some(logo),
        ..draft()
    })
    .unwrap();
    assert_eq!(profile.logo(), Some(logo));
    assert_eq!(profile.to_draft().logo, Some(logo));

    let mut stored = serde_json::to_value(&profile).unwrap();
    stored.as_object_mut().unwrap().remove("logo");
    let older: BusinessProfile = serde_json::from_value(stored).unwrap();
    assert_eq!(older.logo(), None);
}

#[test]
fn a_change_of_basis_leaves_earlier_years_alone() {
    let fy = FiscalYear::starting;
    let mut schedule = BasisSchedule::default();
    assert_eq!(schedule.basis(fy(2026)), IncomeBasis::Accrual);

    schedule.set(fy(2026), IncomeBasis::Cash);
    assert_eq!(schedule.basis(fy(2025)), IncomeBasis::Accrual);
    assert_eq!(schedule.basis(fy(2026)), IncomeBasis::Cash);
    assert_eq!(schedule.basis(fy(2030)), IncomeBasis::Cash);
    assert!(schedule.switched_in(fy(2026)));
    assert!(!schedule.switched_in(fy(2027)));
    assert_eq!(schedule.describe(), "accrual, cash from FY 2026-27");

    // Saving the form again with the year's own basis changes nothing
    let before = schedule.clone();
    schedule.set(fy(2027), IncomeBasis::Cash);
    assert_eq!(schedule, before);

    // Going back later keeps the cash years
    schedule.set(fy(2028), IncomeBasis::Accrual);
    assert_eq!(schedule.basis(fy(2027)), IncomeBasis::Cash);
    assert_eq!(schedule.basis(fy(2028)), IncomeBasis::Accrual);
    // An earlier change drops the ones after it
    schedule.set(fy(2024), IncomeBasis::Cash);
    assert_eq!(schedule.basis(fy(2023)), IncomeBasis::Accrual);
    assert_eq!(schedule.basis(fy(2030)), IncomeBasis::Cash);
}

#[test]
fn a_stored_schedule_reads_as_its_real_changes() {
    let stored = r#"[{"from":2027,"basis":"accrual"},{"from":2026,"basis":"cash"},{"from":2025,"basis":"accrual"}]"#;
    let schedule: BasisSchedule = serde_json::from_str(stored).unwrap();
    // 2025 changes nothing and 2027 goes back: sorted, and only real changes kept
    assert_eq!(
        serde_json::to_string(&schedule).unwrap(),
        r#"[{"from":2026,"basis":"cash"},{"from":2027,"basis":"accrual"}]"#
    );

    let mut draft = draft();
    draft.income_basis = schedule.clone();
    let profile = BusinessProfile::try_from(draft).unwrap();
    assert_eq!(profile.income_basis(), &schedule);
    assert_eq!(profile.to_draft().income_basis, schedule);
}
