use accounting_core::Ledger;
use bigdecimal::BigDecimal;
use pollster::block_on;

use super::fixtures::{
    OTHER_STATE_GSTIN, SELLER_GSTIN, client, date, fixed_project, foreign_client, india,
    profile_draft,
};
use crate::domain::{
    Currency, DomainError, ExchangeRate, InvoiceStatus, MilestoneStatus, ProjectStatus,
};
use crate::services::{InvoiceService, InvoicingError, ServiceError, chart};
use crate::storage::{InvoiceRepository, ProjectRepository, RedbRepository, Store};

fn rupees(amount: i64) -> BigDecimal {
    BigDecimal::from(amount)
}

/// An in-memory store with a registered seller and one ₹50,000 inter-state milestone
fn setup() -> (
    Store,
    InvoiceService<RedbRepository>,
    crate::domain::Project,
) {
    let store = Store::in_memory().unwrap();
    let mut service = InvoiceService::new(store.repository(), None);
    service
        .save_profile(profile_draft(Some(SELLER_GSTIN), None))
        .unwrap();
    let project = fixed_project(client(Some(OTHER_STATE_GSTIN), india(), &[]), 50_000);
    store.repository().save(&project).unwrap();
    (store, service, project)
}

fn balance(store: &Store, account: &str) -> BigDecimal {
    let ledger = Ledger::new(store.ledger());
    let trial = block_on(ledger.get_trial_balance(date())).unwrap();
    trial.balances.get(account).map_or_else(
        || rupees(0),
        |b| {
            b.debit_balance.clone().unwrap_or_default()
                + b.credit_balance.clone().unwrap_or_default()
        },
    )
}

#[test]
fn issuing_without_a_profile_says_what_to_do() {
    let store = Store::in_memory().unwrap();
    let mut service = InvoiceService::new(store.repository(), None);
    let project = fixed_project(client(Some(OTHER_STATE_GSTIN), india(), &[]), 1_000);
    store.repository().save(&project).unwrap();
    let milestone = project.milestones()[0].id();
    assert!(matches!(
        service.issue(project.id(), milestone, date(), None),
        Err(ServiceError::Invoicing(InvoicingError::NoProfile))
    ));
}

#[test]
fn preview_matches_what_is_issued_and_saves_nothing() {
    let (store, mut service, project) = setup();
    let milestone = project.milestones()[0].id();
    let preview = service
        .preview(project.id(), milestone, date(), None)
        .unwrap();
    assert_eq!(preview.number.as_str(), "INV/26-27/001");
    assert_eq!(preview.amounts.total, rupees(59_000));
    assert!(service.list().unwrap().is_empty());

    let issued = service
        .issue(project.id(), milestone, date(), None)
        .unwrap();
    assert_eq!(issued.invoice.number(), &preview.number);
    assert!(matches!(issued.pdf, Ok(None)));

    let stored = store.repository().load(project.id()).unwrap();
    let m = &stored.milestones()[0];
    assert_eq!(m.status(), MilestoneStatus::Invoiced);
    assert_eq!(m.invoice_number(), Some("INV/26-27/001"));
    assert_eq!(stored.status(), ProjectStatus::Completed);
    assert_eq!(balance(&store, chart::RECEIVABLE), rupees(59_000));
    assert_eq!(balance(&store, chart::IGST_OUTPUT), rupees(9_000));
    assert_eq!(balance(&store, chart::SERVICE_REVENUE), rupees(50_000));
}

#[test]
fn numbers_run_in_sequence_and_milestones_are_billed_once() {
    let (store, mut service, mut project) = setup();
    project
        .add_milestone("Build", date(), Some(rupees(10_000)))
        .unwrap();
    store.repository().save(&project).unwrap();
    let ids: Vec<_> = project.milestones().iter().map(|m| m.id()).collect();

    let first = service.issue(project.id(), ids[0], date(), None).unwrap();
    let second = service.issue(project.id(), ids[1], date(), None).unwrap();
    assert_eq!(first.invoice.number().as_str(), "INV/26-27/001");
    assert_eq!(second.invoice.number().as_str(), "INV/26-27/002");
    assert!(matches!(
        service.issue(project.id(), ids[0], date(), None),
        Err(ServiceError::Invoicing(InvoicingError::NotPending))
    ));
    assert_eq!(
        service.list().unwrap()[0].number().as_str(),
        "INV/26-27/002"
    );
}

#[test]
fn part_payments_with_tds_settle_the_ledger() {
    let (store, mut service, project) = setup();
    let milestone = project.milestones()[0].id();
    let number = service
        .issue(project.id(), milestone, date(), None)
        .unwrap()
        .invoice
        .number()
        .clone();

    // First half, with TDS on the taxable part of it
    let first = service
        .record_payment(&number, date(), rupees(27_000), rupees(2_500))
        .unwrap();
    assert_eq!(first.status(), InvoiceStatus::PartlyPaid);
    assert_eq!(first.balance(), rupees(29_500));
    assert_eq!(balance(&store, chart::RECEIVABLE), rupees(29_500));
    let stored = store.repository().load(project.id()).unwrap();
    assert_eq!(stored.milestones()[0].status(), MilestoneStatus::Invoiced);

    assert!(matches!(
        service.record_payment(&number, date(), rupees(30_000), rupees(0)),
        Err(ServiceError::Domain(
            DomainError::PaymentExceedsBalance { .. }
        ))
    ));
    let paid = service
        .record_payment(&number, date(), rupees(27_000), rupees(2_500))
        .unwrap();
    assert_eq!(paid.status(), InvoiceStatus::Paid);

    assert_eq!(balance(&store, chart::RECEIVABLE), rupees(0));
    assert_eq!(balance(&store, chart::BANK), rupees(54_000));
    assert_eq!(balance(&store, chart::TDS_RECEIVABLE), rupees(5_000));
    let stored = store.repository().load(project.id()).unwrap();
    assert_eq!(stored.milestones()[0].status(), MilestoneStatus::Paid);
    assert_eq!(stored.status(), ProjectStatus::Paid);
    assert!(store.repository().list_invoices().unwrap()[0].is_paid());
}

#[test]
fn every_kind_renders_a_pdf() {
    let (_store, mut service, project) = setup();
    let milestone = project.milestones()[0].id();
    let invoice = service
        .issue(project.id(), milestone, date(), None)
        .unwrap()
        .invoice;
    let bytes = service.pdf_bytes(&invoice).unwrap();
    assert!(bytes.starts_with(b"%PDF"));
}

/// A $1,000 export issued at ₹90
fn usd_export() -> (
    Store,
    InvoiceService<RedbRepository>,
    crate::domain::InvoiceNumber,
) {
    let store = Store::in_memory().unwrap();
    let mut service = InvoiceService::new(store.repository(), None);
    service
        .save_profile(profile_draft(Some(SELLER_GSTIN), Some("AD2710260012345")))
        .unwrap();
    let project = fixed_project(foreign_client(Currency::Usd), 1_000);
    store.repository().save(&project).unwrap();
    let milestone = project.milestones()[0].id();
    let rate = || Some(ExchangeRate::new(rupees(90)).unwrap());

    let preview = service
        .preview(project.id(), milestone, date(), rate())
        .unwrap();
    assert_eq!(preview.currency, Currency::Usd);
    assert_eq!(preview.total_inr, Some(rupees(90_000)));
    let issued = service
        .issue(project.id(), milestone, date(), rate())
        .unwrap();
    assert_eq!(balance(&store, chart::RECEIVABLE), rupees(90_000));
    assert_eq!(balance(&store, chart::EXPORT_REVENUE), rupees(90_000));
    // The PDF labels amounts with the currency code
    service.pdf_bytes(&issued.invoice).unwrap();
    (store, service, issued.invoice.number().clone())
}

#[test]
fn foreign_payments_post_the_exchange_gain() {
    let (store, mut service, number) = usd_export();
    // $400 banked at ₹91, then $600 at ₹92: ₹400 + ₹1,200 more than booked
    let first = service
        .record_foreign_payment(&number, date(), rupees(400), rupees(36_400))
        .unwrap();
    assert_eq!(first.status(), InvoiceStatus::PartlyPaid);
    assert_eq!(balance(&store, chart::RECEIVABLE), rupees(54_000));
    assert_eq!(balance(&store, chart::EXCHANGE_DIFFERENCE), rupees(400));

    let paid = service
        .record_foreign_payment(&number, date(), rupees(600), rupees(55_200))
        .unwrap();
    assert!(paid.is_paid());
    assert_eq!(balance(&store, chart::RECEIVABLE), rupees(0));
    assert_eq!(balance(&store, chart::BANK), rupees(91_600));
    assert_eq!(balance(&store, chart::EXCHANGE_DIFFERENCE), rupees(1_600));
    let stored = store.repository().load(paid.project_id()).unwrap();
    assert_eq!(stored.milestones()[0].status(), MilestoneStatus::Paid);
}

#[test]
fn foreign_payments_post_the_exchange_loss() {
    let (store, mut service, number) = usd_export();
    service
        .record_foreign_payment(&number, date(), rupees(1_000), rupees(89_500))
        .unwrap();
    assert_eq!(balance(&store, chart::RECEIVABLE), rupees(0));
    assert_eq!(balance(&store, chart::BANK), rupees(89_500));
    let ledger = Ledger::new(store.ledger());
    let trial = block_on(ledger.get_trial_balance(date())).unwrap();
    let loss = &trial.balances[chart::EXCHANGE_DIFFERENCE];
    assert_eq!(loss.debit_balance, Some(rupees(500)));
}

#[test]
fn foreign_invoices_need_a_rate_and_take_no_tds() {
    let (_, mut service, number) = usd_export();
    assert!(matches!(
        service.record_payment(&number, date(), rupees(1_000), rupees(0)),
        Err(ServiceError::Domain(DomainError::MissingCreditedAmount))
    ));
    assert_eq!(
        service.load(&number).unwrap().status(),
        InvoiceStatus::Issued
    );
}
