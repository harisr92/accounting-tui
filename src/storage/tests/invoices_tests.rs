use accounting_core::TransactionBuilder;
use bigdecimal::BigDecimal;
use chrono::NaiveDate;

use crate::domain::{
    Billing, BusinessProfile, ClientDetails, FiscalYear, InvoiceNumber, MilestoneStatus,
    ProfileDraft, Project,
};
use crate::services::{build_invoice, chart};
use crate::storage::{
    InvoiceRepository, IssueRecord, ProjectRepository, SettingsRepository, StorageError, Store,
};

fn date() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, 15).unwrap()
}

fn profile() -> BusinessProfile {
    BusinessProfile::try_from(ProfileDraft {
        name: "Studio".to_owned(),
        address: vec!["Pune".to_owned()],
        gstin: Some("27AAPFU0939F1ZV".to_owned()),
        sac_code: "998314".to_owned(),
        invoice_prefix: "INV".to_owned(),
        lut_arn: None,
        terms: Vec::new(),
    })
    .unwrap()
}

fn project() -> Project {
    let client = ClientDetails::named("Acme", Some("29AAPFU0939F1ZR"));
    let mut project = Project::for_client("Site", client, Billing::Fixed).unwrap();
    project
        .add_milestone("Design", date(), Some(BigDecimal::from(1000)))
        .unwrap();
    project
}

/// Issue invoice `sequence` for the project's first milestone under transaction `txn_id`
fn issue(
    store: &Store,
    project: &Project,
    sequence: u32,
    txn_id: &str,
) -> Result<(), StorageError> {
    let number = InvoiceNumber::new("INV", FiscalYear::containing(date()), sequence);
    let built = build_invoice(
        &profile(),
        project,
        &project.milestones()[0],
        number,
        date(),
    )
    .unwrap();
    let mut invoiced = project.clone();
    invoiced
        .invoice_milestone(0, built.invoice.number().as_str())
        .unwrap();
    let transaction = built
        .entries
        .iter()
        .cloned()
        .fold(
            TransactionBuilder::new(txn_id, date(), "test"),
            TransactionBuilder::entry,
        )
        .build()
        .unwrap();
    store.repository().issue(&IssueRecord {
        sequence,
        invoice: &built.invoice,
        project: &invoiced,
        transaction: &transaction,
        accounts: &chart::accounts(),
    })
}

#[test]
fn issue_saves_invoice_project_counter_and_posting_together() {
    let store = Store::in_memory().unwrap();
    let project = project();
    store.repository().save(&project).unwrap();
    let year = FiscalYear::containing(date());
    assert_eq!(store.repository().next_sequence(year).unwrap(), 1);

    issue(&store, &project, 1, "inv-1").unwrap();
    let repo = store.repository();
    assert_eq!(repo.next_sequence(year).unwrap(), 2);
    assert_eq!(repo.list_invoices().unwrap().len(), 1);
    let number = InvoiceNumber::new("INV", year, 1);
    assert_eq!(repo.load_invoice(&number).unwrap().number(), &number);
    let stored = repo.load(project.id()).unwrap();
    assert_eq!(stored.milestones()[0].status(), MilestoneStatus::Invoiced);
}

#[test]
fn a_stale_number_or_clashing_posting_writes_nothing() {
    let store = Store::in_memory().unwrap();
    let project = project();
    store.repository().save(&project).unwrap();
    issue(&store, &project, 1, "inv-1").unwrap();

    let fresh = self::project();
    store.repository().save(&fresh).unwrap();

    assert!(matches!(
        issue(&store, &fresh, 1, "inv-x"),
        Err(StorageError::StaleInvoiceNumber)
    ));
    assert!(matches!(
        issue(&store, &fresh, 2, "inv-1"),
        Err(StorageError::DuplicateTransaction(_))
    ));

    let repo = store.repository();
    let year = FiscalYear::containing(date());
    assert_eq!(repo.next_sequence(year).unwrap(), 2);
    assert_eq!(repo.list_invoices().unwrap().len(), 1);
    let untouched = repo.load(fresh.id()).unwrap();
    assert_eq!(untouched.milestones()[0].status(), MilestoneStatus::Pending);
}

#[test]
fn profile_is_saved_and_loaded() {
    let store = Store::in_memory().unwrap();
    let mut repo = store.repository();
    assert!(repo.load_profile().unwrap().is_none());
    repo.save_profile(&profile()).unwrap();
    assert_eq!(repo.load_profile().unwrap(), Some(profile()));
}

#[test]
fn missing_invoice_is_not_found() {
    let store = Store::in_memory().unwrap();
    let number = InvoiceNumber::new("INV", FiscalYear::containing(date()), 7);
    assert!(matches!(
        store.repository().load_invoice(&number),
        Err(StorageError::InvoiceNotFound(_))
    ));
}
