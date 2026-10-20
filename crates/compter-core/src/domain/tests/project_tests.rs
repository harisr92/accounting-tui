use bigdecimal::{BigDecimal, Zero};
use chrono::NaiveDate;

use crate::domain::{
    Billing, ClientDetails, ClientRegion, Currency, DomainError, MilestoneId, MilestoneStatus,
    Project, ProjectId, ProjectStatus, ProjectSummary, TimeEntry,
};

const GSTIN: &str = "29AAPFU0939F1ZR";

fn date() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, 1).unwrap()
}

fn fixed() -> Project {
    Project::new("Site", "Acme", Some(GSTIN), Billing::Fixed).unwrap()
}

fn hourly() -> Project {
    Project::new(
        "App",
        "Globex",
        None,
        Billing::hourly(BigDecimal::from(1500)).unwrap(),
    )
    .unwrap()
}

fn rupees(amount: i64) -> Option<BigDecimal> {
    Some(BigDecimal::from(amount))
}

#[test]
fn milestones_move_pending_invoiced_paid() {
    let mut p = fixed();
    let i = p.add_milestone("Design", date(), rupees(100)).unwrap();
    p.invoice_milestone(i, "T-1").unwrap();
    assert_eq!(p.milestones()[i].status(), MilestoneStatus::Invoiced);
    p.record_payment(i).unwrap();
    assert_eq!(p.milestones()[i].status(), MilestoneStatus::Paid);
}

#[test]
fn transitions_cannot_be_skipped_or_repeated() {
    let mut p = fixed();
    p.add_milestone("Design", date(), rupees(100)).unwrap();
    assert!(matches!(
        p.record_payment(0),
        Err(DomainError::InvalidTransition {
            from: MilestoneStatus::Pending,
            to: MilestoneStatus::Paid
        })
    ));
    p.invoice_milestone(0, "T-1").unwrap();
    assert!(matches!(
        p.invoice_milestone(0, "T-1"),
        Err(DomainError::InvalidTransition { .. })
    ));
    assert!(matches!(
        p.remove_milestone(0),
        Err(DomainError::MilestoneLocked)
    ));
    assert!(matches!(
        p.invoice_milestone(5, "T-1"),
        Err(DomainError::MilestoneNotFound(5))
    ));
}

#[test]
fn status_is_derived_from_milestones() {
    let mut p = fixed();
    assert_eq!(p.status(), ProjectStatus::Active);
    p.add_milestone("A", date(), rupees(100)).unwrap();
    p.add_milestone("B", date(), rupees(200)).unwrap();
    p.invoice_milestone(0, "T-1").unwrap();
    assert_eq!(p.status(), ProjectStatus::Active);
    p.invoice_milestone(1, "T-1").unwrap();
    assert_eq!(p.status(), ProjectStatus::Completed);
    p.record_payment(0).unwrap();
    assert_eq!(p.paid_amount(), BigDecimal::from(100));
    assert_eq!(p.outstanding_amount(), BigDecimal::from(200));
    p.record_payment(1).unwrap();
    assert_eq!(p.status(), ProjectStatus::Paid);
    assert_eq!(p.total_amount(), &BigDecimal::from(300));
    assert_eq!(p.outstanding_amount(), BigDecimal::zero());
}

#[test]
fn hourly_time_drives_amounts() {
    let mut p = hourly();
    p.add_milestone("Prototype", date(), None).unwrap();
    assert!(matches!(
        p.invoice_milestone(0, "T-1"),
        Err(DomainError::NothingToInvoice)
    ));
    p.log_time(0, TimeEntry::new(date(), 60, "setup").unwrap())
        .unwrap();
    p.log_time(0, TimeEntry::new(date(), 30, "review").unwrap())
        .unwrap();
    assert_eq!(p.milestones()[0].logged_minutes(), 90);
    assert_eq!(p.milestones()[0].amount(), &BigDecimal::from(2250));
    assert_eq!(p.total_amount(), &BigDecimal::from(2250));

    p.invoice_milestone(0, "T-1").unwrap();
    let entry = TimeEntry::new(date(), 15, "late").unwrap();
    assert!(matches!(
        p.log_time(0, entry),
        Err(DomainError::MilestoneLocked)
    ));
}

#[test]
fn logged_minutes_sum_every_milestone() {
    let mut p = hourly();
    assert_eq!(p.logged_minutes(), 0);
    p.add_milestone("Prototype", date(), None).unwrap();
    p.add_milestone("Launch", date(), None).unwrap();
    p.log_time(0, TimeEntry::new(date(), 90, "spike").unwrap())
        .unwrap();
    p.log_time(1, TimeEntry::new(date(), 45, "deploy").unwrap())
        .unwrap();
    assert_eq!(p.logged_minutes(), 135);
}

#[test]
fn billing_type_must_match() {
    let mut f = fixed();
    assert!(matches!(
        f.add_milestone("A", date(), None),
        Err(DomainError::BillingMismatch)
    ));
    f.add_milestone("A", date(), rupees(100)).unwrap();
    let entry = TimeEntry::new(date(), 30, "").unwrap();
    assert!(matches!(
        f.log_time(0, entry),
        Err(DomainError::BillingMismatch)
    ));

    let mut h = hourly();
    assert!(matches!(
        h.add_milestone("A", date(), rupees(100)),
        Err(DomainError::BillingMismatch)
    ));
}

#[test]
fn rejects_invalid_input() {
    assert!(matches!(
        Project::new(" ", "Acme", None, Billing::Fixed),
        Err(DomainError::EmptyField("project name"))
    ));
    assert!(matches!(
        Project::new("Site", "", None, Billing::Fixed),
        Err(DomainError::EmptyField("client name"))
    ));
    assert!(matches!(
        Project::new("Site", "Acme", Some("29ABCDE1234F1Z5"), Billing::Fixed),
        Err(DomainError::InvalidGstin(_))
    ));
    let mut p = fixed();
    assert!(matches!(
        p.add_milestone("A", date(), rupees(0)),
        Err(DomainError::NonPositiveAmount)
    ));
    assert!(matches!(
        p.add_milestone("", date(), rupees(10)),
        Err(DomainError::EmptyField("milestone name"))
    ));
}

#[test]
fn json_round_trip() {
    let mut p = hourly();
    p.add_milestone("Prototype", date(), None).unwrap();
    p.log_time(0, TimeEntry::new(date(), 45, "spike").unwrap())
        .unwrap();
    let json = serde_json::to_string(&p).unwrap();
    assert_eq!(serde_json::from_str::<Project>(&json).unwrap(), p);
}

#[test]
fn loading_recomputes_derived_fields() {
    let mut p = fixed();
    p.add_milestone("A", date(), rupees(100)).unwrap();
    let mut json = serde_json::to_value(&p).unwrap();
    json["total_amount"] = "999999".into();
    json["status"] = "PAID".into();
    let loaded: Project = serde_json::from_value(json).unwrap();
    assert_eq!(loaded.total_amount(), &BigDecimal::from(100));
    assert_eq!(loaded.status(), ProjectStatus::Active);
}

#[test]
fn rate_change_only_reprices_pending_milestones() {
    let mut p = hourly();
    p.add_milestone("Invoiced", date(), None).unwrap();
    p.add_milestone("Pending", date(), None).unwrap();
    p.log_time(0, TimeEntry::new(date(), 60, "").unwrap())
        .unwrap();
    p.log_time(1, TimeEntry::new(date(), 60, "").unwrap())
        .unwrap();
    p.invoice_milestone(0, "T-1").unwrap();

    let mut json = serde_json::to_value(&p).unwrap();
    json["billing"]["rate"] = "2000".into();
    let loaded: Project = serde_json::from_value(json).unwrap();
    assert_eq!(loaded.milestones()[0].amount(), &BigDecimal::from(1500));
    assert_eq!(loaded.milestones()[1].amount(), &BigDecimal::from(2000));
    assert_eq!(loaded.total_amount(), &BigDecimal::from(3500));
}

#[test]
fn loading_rejects_billed_milestone_without_amount() {
    let mut p = hourly();
    p.add_milestone("A", date(), None).unwrap();
    p.log_time(0, TimeEntry::new(date(), 60, "").unwrap())
        .unwrap();
    p.invoice_milestone(0, "T-1").unwrap();
    let mut json = serde_json::to_value(&p).unwrap();
    json["milestones"][0]["amount"] = "0".into();
    assert!(serde_json::from_value::<Project>(json).is_err());
}

#[test]
fn loading_rejects_invalid_records() {
    let p = fixed();
    let mut bad_name = serde_json::to_value(&p).unwrap();
    bad_name["name"] = "".into();
    assert!(serde_json::from_value::<Project>(bad_name).is_err());

    let mut bad_gstin = serde_json::to_value(&p).unwrap();
    bad_gstin["client_gstin"] = "29ABCDE1234F1Z5".into();
    assert!(serde_json::from_value::<Project>(bad_gstin).is_err());
}

#[test]
fn fixed_amounts_round_to_paise_before_validation() {
    let mut p = fixed();
    let amount = |s: &str| Some(s.parse::<BigDecimal>().unwrap());
    p.add_milestone("A", date(), amount("100.005")).unwrap();
    assert_eq!(
        p.milestones()[0].amount(),
        &"100.01".parse::<BigDecimal>().unwrap()
    );
    assert!(matches!(
        p.add_milestone("B", date(), amount("0.004")),
        Err(DomainError::NonPositiveAmount)
    ));
}

#[test]
fn summary_mirrors_project() {
    let mut p = fixed();
    p.add_milestone("A", date(), rupees(100)).unwrap();
    p.invoice_milestone(0, "T-1").unwrap();
    let summary = ProjectSummary::from(&p);
    assert_eq!(summary.id(), p.id());
    assert_eq!(summary.name(), "Site");
    assert_eq!(summary.client_name(), "Acme");
    assert_eq!(summary.status(), ProjectStatus::Completed);
    assert_eq!(summary.total_amount(), &BigDecimal::from(100));
}

#[test]
fn edits_pending_milestones_only() {
    let mut p = fixed();
    p.add_milestone("A", date(), rupees(100)).unwrap();
    let due = NaiveDate::from_ymd_opt(2026, 12, 1).unwrap();
    p.edit_milestone(0, "Design", due, rupees(250)).unwrap();
    let m = &p.milestones()[0];
    assert_eq!((m.name(), m.due_date()), ("Design", due));
    assert_eq!(p.total_amount(), &BigDecimal::from(250));

    assert!(matches!(
        p.edit_milestone(0, "", due, rupees(1)),
        Err(DomainError::EmptyField("milestone name"))
    ));
    assert!(matches!(
        p.edit_milestone(0, "X", due, None),
        Err(DomainError::BillingMismatch)
    ));
    p.invoice_milestone(0, "T-1").unwrap();
    assert!(matches!(
        p.edit_milestone(0, "X", due, rupees(1)),
        Err(DomainError::MilestoneLocked)
    ));
    assert_eq!(p.milestones()[0].name(), "Design");
}

#[test]
fn undo_last_time_entry() {
    let mut p = hourly();
    p.add_milestone("A", date(), None).unwrap();
    assert!(matches!(
        p.remove_last_time_entry(0),
        Err(DomainError::NoTimeEntries)
    ));
    p.log_time(0, TimeEntry::new(date(), 60, "ok").unwrap())
        .unwrap();
    p.log_time(0, TimeEntry::new(date(), 600, "typo").unwrap())
        .unwrap();
    assert_eq!(p.remove_last_time_entry(0).unwrap().note(), "typo");
    assert_eq!(p.total_amount(), &BigDecimal::from(1500));
}

#[test]
fn update_details_is_all_or_nothing() {
    let mut p = fixed();
    p.update_details("Shop", ClientDetails::named("Globex", None))
        .unwrap();
    assert_eq!((p.name(), p.client_name()), ("Shop", "Globex"));
    assert!(p.client_gstin().is_none());

    let bad = ClientDetails::named("Client", Some("bad"));
    assert!(p.update_details("New", bad).is_err());
    assert_eq!((p.name(), p.client_name()), ("Shop", "Globex"));
}

#[test]
fn rate_change_reprices_pending_only_and_type_is_locked_by_milestones() {
    let mut p = hourly();
    p.add_milestone("Billed", date(), None).unwrap();
    p.add_milestone("Open", date(), None).unwrap();
    p.log_time(0, TimeEntry::new(date(), 60, "").unwrap())
        .unwrap();
    p.log_time(1, TimeEntry::new(date(), 60, "").unwrap())
        .unwrap();
    p.invoice_milestone(0, "T-1").unwrap();

    p.set_billing(Billing::hourly(BigDecimal::from(2000)).unwrap())
        .unwrap();
    assert_eq!(p.milestones()[0].amount(), &BigDecimal::from(1500));
    assert_eq!(p.milestones()[1].amount(), &BigDecimal::from(2000));

    assert!(matches!(
        p.set_billing(Billing::Fixed),
        Err(DomainError::BillingMismatch)
    ));
    let mut empty = hourly();
    empty.set_billing(Billing::Fixed).unwrap();
    assert_eq!(empty.billing(), &Billing::Fixed);
}

#[test]
fn only_unbilled_projects_can_be_deleted() {
    let mut p = fixed();
    assert!(p.ensure_deletable().is_ok());
    p.add_milestone("A", date(), rupees(100)).unwrap();
    assert!(p.ensure_deletable().is_ok());
    p.invoice_milestone(0, "T-1").unwrap();
    assert!(matches!(
        p.ensure_deletable(),
        Err(DomainError::HasBilledMilestones)
    ));
}

#[test]
fn foreign_clients_have_a_country_and_no_gstin() {
    let abroad = |country: &str, gstin: Option<&str>| ClientDetails {
        name: "Initrode GmbH".to_owned(),
        gstin: gstin.map(str::to_owned),
        address: vec!["Hauptstr. 1".to_owned(), "  ".to_owned()],
        region: ClientRegion::Abroad {
            country: country.to_owned(),
        },
        ..ClientDetails::default()
    };
    let p = Project::for_client("Audit", abroad("Germany", None), Billing::Fixed).unwrap();
    assert!(p.client_region().is_abroad());
    assert_eq!(p.client_address(), ["Hauptstr. 1"]);

    assert!(matches!(
        Project::for_client("Audit", abroad("Germany", Some(GSTIN)), Billing::Fixed),
        Err(DomainError::ForeignClientWithGstin)
    ));
    assert!(matches!(
        Project::for_client("Audit", abroad(" ", None), Billing::Fixed),
        Err(DomainError::EmptyField("client country"))
    ));
}

#[test]
fn milestones_keep_their_id_and_invoice_number() {
    let mut p = fixed();
    p.add_milestone("A", date(), rupees(100)).unwrap();
    p.add_milestone("B", date(), rupees(200)).unwrap();
    let id = p.milestones()[1].id();
    p.remove_milestone(0).unwrap();
    assert_eq!(p.milestone_index(id), Some(0));

    p.invoice_milestone(0, "INV/26-27/001").unwrap();
    assert_eq!(p.milestones()[0].invoice_number(), Some("INV/26-27/001"));
    let json = serde_json::to_string(&p).unwrap();
    let loaded: Project = serde_json::from_str(&json).unwrap();
    assert_eq!(loaded.milestones()[0].id(), id);
    assert_eq!(
        loaded.milestones()[0].invoice_number(),
        Some("INV/26-27/001")
    );
}

fn abroad_in(currency: Currency) -> ClientDetails {
    ClientDetails {
        name: "Initrode GmbH".to_owned(),
        region: ClientRegion::Abroad {
            country: "Germany".to_owned(),
        },
        currency,
        ..ClientDetails::default()
    }
}

#[test]
fn only_clients_abroad_are_billed_in_a_foreign_currency() {
    let p = Project::for_client("Audit", abroad_in(Currency::Usd), Billing::Fixed).unwrap();
    assert_eq!(p.currency(), Currency::Usd);
    assert_eq!(p.client_details().currency, Currency::Usd);

    let india = ClientDetails {
        currency: Currency::Eur,
        ..ClientDetails::named("Acme", Some(GSTIN))
    };
    assert!(matches!(
        Project::for_client("Site", india, Billing::Fixed),
        Err(DomainError::ForeignCurrencyInIndia)
    ));

    // Moving a dollar client to India without switching to rupees is refused, and changes nothing
    let mut p = p;
    let moved = ClientDetails {
        currency: Currency::Usd,
        ..ClientDetails::named("Initrode", None)
    };
    assert!(matches!(
        p.update_details("Audit", moved),
        Err(DomainError::ForeignCurrencyInIndia)
    ));
    assert!(p.client_region().is_abroad());
    p.update_details("Audit", ClientDetails::named("Initrode", None))
        .unwrap();
    assert_eq!(p.currency(), Currency::Inr);
}

#[test]
fn the_currency_is_fixed_once_a_milestone_is_invoiced() {
    let mut p = Project::for_client("Audit", abroad_in(Currency::Usd), Billing::Fixed).unwrap();
    p.add_milestone("Report", date(), Some(BigDecimal::from(1000)))
        .unwrap();
    p.update_details("Audit", abroad_in(Currency::Eur)).unwrap();
    assert_eq!(p.currency(), Currency::Eur);

    p.invoice_milestone(0, "INV/26-27/001").unwrap();
    assert!(matches!(
        p.update_details("Audit", abroad_in(Currency::Gbp)),
        Err(DomainError::CurrencyLocked)
    ));
    assert_eq!(p.currency(), Currency::Eur);
    p.update_details("Audit v2", abroad_in(Currency::Eur))
        .unwrap();
}

#[test]
fn stored_projects_without_a_currency_are_rupees_and_are_checked() {
    let p = Project::for_client("Audit", abroad_in(Currency::Gbp), Billing::Fixed).unwrap();
    let json = serde_json::to_value(&p).unwrap();
    assert_eq!(json["currency"], "GBP");

    let mut old = json.clone();
    old.as_object_mut().unwrap().remove("currency");
    let read = serde_json::from_value::<Project>(old).unwrap();
    assert_eq!(read.currency(), Currency::Inr);

    let mut tampered = json;
    tampered["client_region"] = serde_json::json!({"type": "india"});
    assert!(serde_json::from_value::<Project>(tampered).is_err());
}

#[test]
fn only_finished_projects_are_archived_and_take_no_new_work() {
    let mut p = fixed();
    p.add_milestone("Design", date(), rupees(100)).unwrap();
    assert!(matches!(p.archive(), Err(DomainError::ProjectNotFinished)));

    p.invoice_milestone(0, "T-1").unwrap();
    p.archive().unwrap();
    assert!(p.is_archived());
    assert!(ProjectSummary::from(&p).is_archived());
    assert!(matches!(
        p.add_milestone("More", date(), rupees(50)),
        Err(DomainError::ProjectArchived)
    ));
    // Payments still arrive for archived work
    p.record_payment(0).unwrap();

    p.unarchive();
    p.add_milestone("More", date(), rupees(50)).unwrap();
}

#[test]
fn archived_flag_survives_a_round_trip() {
    let mut p = fixed();
    p.add_milestone("Design", date(), rupees(100)).unwrap();
    p.invoice_milestone(0, "T-1").unwrap();
    p.archive().unwrap();
    let json = serde_json::to_string(&p).unwrap();
    let back: Project = serde_json::from_str(&json).unwrap();
    assert!(back.is_archived());
}

#[test]
fn credits_come_off_what_is_paid_and_owed() {
    let mut p = fixed();
    p.add_milestone("Design", date(), rupees(100)).unwrap();
    assert!(matches!(
        p.credit_milestone(0, &BigDecimal::from(10)),
        Err(DomainError::NotInvoiced)
    ));
    p.invoice_milestone(0, "T-1").unwrap();
    // Credited in part, it is still invoiced
    p.credit_milestone(0, &BigDecimal::from(40)).unwrap();
    assert_eq!(p.milestones()[0].status(), MilestoneStatus::Invoiced);
    assert!(matches!(
        p.credit_milestone(0, &BigDecimal::from(61)),
        Err(DomainError::CreditExceedsMilestone)
    ));
    assert_eq!(p.outstanding_amount(), BigDecimal::from(60));

    // Credited in full, it is credited
    p.credit_milestone(0, &BigDecimal::from(60)).unwrap();
    assert_eq!(p.milestones()[0].status(), MilestoneStatus::Credited);
    assert_eq!(p.paid_amount(), BigDecimal::zero());
    assert_eq!(p.outstanding_amount(), BigDecimal::zero());
    assert_eq!(p.status(), ProjectStatus::Paid);

    // A credited milestone round-trips, and a stored one that isn't fully credited is refused
    let json = serde_json::to_string(&p).unwrap();
    let back: Project = serde_json::from_str(&json).unwrap();
    assert_eq!(back.milestones()[0].credited(), &BigDecimal::from(100));
    let tampered = json.replace("\"credited\":\"100\"", "\"credited\":\"99\"");
    assert_ne!(tampered, json);
    assert!(serde_json::from_str::<Project>(&tampered).is_err());
}

#[test]
fn project_ids_round_trip_through_text() {
    let id = fixed().id();
    assert_eq!(id.to_string().parse::<ProjectId>().unwrap(), id);
    assert!("not-an-id".parse::<ProjectId>().is_err());
}

#[test]
fn milestone_ids_round_trip_through_text() {
    let id = MilestoneId::generate();
    assert_eq!(id.to_string().parse::<MilestoneId>().unwrap(), id);
    assert!("not-an-id".parse::<MilestoneId>().is_err());
}

#[test]
fn milestones_billed_together_change_together_or_not_at_all() {
    let mut p = fixed();
    p.add_milestone("Design", date(), rupees(1000)).unwrap();
    p.add_milestone("Build", date(), rupees(2000)).unwrap();
    p.add_milestone("Extra", date(), rupees(500)).unwrap();
    let ids: Vec<MilestoneId> = p.milestones().iter().map(|m| m.id()).collect();
    p.invoice_milestones(&ids[2..], "INV/26-27/000").unwrap();

    // Extra is already invoiced, so neither of the others is invoiced either
    assert!(p.invoice_milestones(&ids, "INV/26-27/001").is_err());
    assert_eq!(p.milestones()[0].status(), MilestoneStatus::Pending);
    assert_eq!(p.milestones()[1].status(), MilestoneStatus::Pending);

    p.invoice_milestones(&ids[..2], "INV/26-27/001").unwrap();
    assert_eq!(p.milestones()[1].invoice_number(), Some("INV/26-27/001"));
    assert!(matches!(
        p.credit_milestones(&[
            (ids[0], BigDecimal::from(1000)),
            (ids[1], BigDecimal::from(2001))
        ]),
        Err(DomainError::CreditExceedsMilestone)
    ));
    assert!(p.milestones()[0].credited().is_zero());

    // Design credited in full is credited at once, while Build is still due
    p.credit_milestones(&[(ids[0], BigDecimal::from(1000))])
        .unwrap();
    assert_eq!(p.milestones()[0].status(), MilestoneStatus::Credited);
    assert_eq!(p.milestones()[1].status(), MilestoneStatus::Invoiced);
    // Nothing paid: the invoice can't be cancelled while Build still bills something
    assert!(p.settle_invoice(&ids[..2], false).is_err());
    // Paying the rest pays Build; Design stays credited
    p.settle_invoice(&ids[..2], true).unwrap();
    assert_eq!(p.milestones()[0].status(), MilestoneStatus::Credited);
    assert_eq!(p.milestones()[1].status(), MilestoneStatus::Paid);
    assert_eq!(p.paid_amount(), BigDecimal::from(2000));
    // A milestone already paid stays paid
    p.settle_invoice(&ids[..2], true).unwrap();
    assert_eq!(p.milestones()[1].status(), MilestoneStatus::Paid);
}

#[test]
fn an_invoice_cancelled_by_credit_notes_credits_every_milestone() {
    let mut p = fixed();
    p.add_milestone("Design", date(), rupees(1000)).unwrap();
    p.add_milestone("Build", date(), rupees(2000)).unwrap();
    let ids: Vec<MilestoneId> = p.milestones().iter().map(|m| m.id()).collect();
    p.invoice_milestones(&ids, "INV/26-27/001").unwrap();
    p.credit_milestones(&[
        (ids[0], BigDecimal::from(1000)),
        (ids[1], BigDecimal::from(2000)),
    ])
    .unwrap();
    p.settle_invoice(&ids, false).unwrap();
    assert!(
        p.milestones()
            .iter()
            .all(|m| m.status() == MilestoneStatus::Credited)
    );
    assert_eq!(p.status(), ProjectStatus::Paid);
}
