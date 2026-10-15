use bigdecimal::{BigDecimal, Zero};
use chrono::NaiveDate;

use crate::domain::{
    Billing, DomainError, MilestoneStatus, Project, ProjectStatus, ProjectSummary, TimeEntry,
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
    p.invoice_milestone(i).unwrap();
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
    p.invoice_milestone(0).unwrap();
    assert!(matches!(
        p.invoice_milestone(0),
        Err(DomainError::InvalidTransition { .. })
    ));
    assert!(matches!(
        p.remove_milestone(0),
        Err(DomainError::MilestoneLocked)
    ));
    assert!(matches!(
        p.invoice_milestone(5),
        Err(DomainError::MilestoneNotFound(5))
    ));
}

#[test]
fn status_is_derived_from_milestones() {
    let mut p = fixed();
    assert_eq!(p.status(), ProjectStatus::Active);
    p.add_milestone("A", date(), rupees(100)).unwrap();
    p.add_milestone("B", date(), rupees(200)).unwrap();
    p.invoice_milestone(0).unwrap();
    assert_eq!(p.status(), ProjectStatus::Active);
    p.invoice_milestone(1).unwrap();
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
        p.invoice_milestone(0),
        Err(DomainError::NothingToInvoice)
    ));
    p.log_time(0, TimeEntry::new(date(), 60, "setup").unwrap())
        .unwrap();
    p.log_time(0, TimeEntry::new(date(), 30, "review").unwrap())
        .unwrap();
    assert_eq!(p.milestones()[0].logged_minutes(), 90);
    assert_eq!(p.milestones()[0].amount(), &BigDecimal::from(2250));
    assert_eq!(p.total_amount(), &BigDecimal::from(2250));

    p.invoice_milestone(0).unwrap();
    let entry = TimeEntry::new(date(), 15, "late").unwrap();
    assert!(matches!(
        p.log_time(0, entry),
        Err(DomainError::MilestoneLocked)
    ));
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
    p.invoice_milestone(0).unwrap();

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
    p.invoice_milestone(0).unwrap();
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
    p.invoice_milestone(0).unwrap();
    let summary = ProjectSummary::from(&p);
    assert_eq!(summary.id(), p.id());
    assert_eq!(summary.name(), "Site");
    assert_eq!(summary.client_name(), "Acme");
    assert_eq!(summary.status(), ProjectStatus::Completed);
    assert_eq!(summary.total_amount(), &BigDecimal::from(100));
}
