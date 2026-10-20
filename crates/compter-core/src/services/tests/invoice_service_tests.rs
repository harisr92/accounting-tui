use accounting_core::Ledger;
use bigdecimal::BigDecimal;
use pollster::block_on;

use super::fixtures::{
    LOGO_PNG, OTHER_STATE_GSTIN, SELLER_GSTIN, client, date, fixed_project, foreign_client, india,
    profile_draft,
};
use crate::domain::{
    Currency, DomainError, ExchangeRate, InvoiceStatus, MilestoneId, MilestoneStatus,
    ProjectStatus, ReturnKind, ReturnTerms,
};
use crate::services::{CreditLineDraft, InvoiceService, InvoicingError, ServiceError, chart};
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
        service.issue(project.id(), &[milestone], date(), None),
        Err(ServiceError::Invoicing(InvoicingError::NoProfile))
    ));
}

#[test]
fn preview_matches_what_is_issued_and_saves_nothing() {
    let (store, mut service, project) = setup();
    let milestone = project.milestones()[0].id();
    let preview = service
        .preview(project.id(), &[milestone], date(), None)
        .unwrap();
    assert_eq!(preview.number.as_str(), "INV/26-27/001");
    assert_eq!(preview.amounts.total, rupees(59_000));
    assert!(service.list().unwrap().is_empty());

    let issued = service
        .issue(project.id(), &[milestone], date(), None)
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

    let first = service
        .issue(project.id(), &[ids[0]], date(), None)
        .unwrap();
    let second = service
        .issue(project.id(), &[ids[1]], date(), None)
        .unwrap();
    assert_eq!(first.invoice.number().as_str(), "INV/26-27/001");
    assert_eq!(second.invoice.number().as_str(), "INV/26-27/002");
    assert!(matches!(
        service.issue(project.id(), &[ids[0]], date(), None),
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
        .issue(project.id(), &[milestone], date(), None)
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
        .issue(project.id(), &[milestone], date(), None)
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
        .preview(project.id(), &[milestone], date(), rate())
        .unwrap();
    assert_eq!(preview.currency, Currency::Usd);
    assert_eq!(preview.total_inr, Some(rupees(90_000)));
    let issued = service
        .issue(project.id(), &[milestone], date(), rate())
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

/// `bytes` written to a temp file, for `set_logo`
fn logo_file(bytes: &[u8]) -> tempfile::NamedTempFile {
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), bytes).unwrap();
    file
}

#[test]
fn invoices_print_the_logo_they_were_issued_with() {
    let (store, mut service, mut project) = setup();
    let first = project.milestones()[0].id();
    let plain = service
        .issue(project.id(), &[first], date(), None)
        .unwrap()
        .invoice;
    assert_eq!(plain.logo(), None);

    let file = logo_file(&LOGO_PNG);
    let profile = service.set_logo(Some(file.path())).unwrap();
    let logo = profile.logo().unwrap();
    assert_eq!(service.current_logo().unwrap().unwrap().id(), logo);

    project
        .add_milestone("Build", date(), Some(rupees(10_000)))
        .unwrap();
    store.repository().save(&project).unwrap();
    let second = project.milestones()[1].id();
    let branded = service
        .issue(project.id(), &[second], date(), None)
        .unwrap()
        .invoice;
    assert_eq!(branded.logo(), Some(logo));
    let with_logo = service.pdf_bytes(&branded).unwrap();
    assert!(with_logo.starts_with(b"%PDF") && has_image(&with_logo));

    // Removing the logo affects only new invoices; issued ones reprint unchanged
    service.set_logo(None).unwrap();
    assert!(service.current_logo().unwrap().is_none());
    let reloaded = service.load(branded.number()).unwrap();
    assert_eq!(reloaded.logo(), Some(logo));
    assert!(has_image(&service.pdf_bytes(&reloaded).unwrap()));
    assert!(!has_image(&service.pdf_bytes(&plain).unwrap()));
}

/// Whether the PDF embeds an image XObject
fn has_image(pdf: &[u8]) -> bool {
    pdf.windows(b"/Image".len()).any(|w| w == b"/Image")
}

#[test]
fn a_file_that_is_not_an_image_is_refused_and_nothing_changes() {
    let (_store, mut service, _project) = setup();
    let file = logo_file(b"not an image");
    assert!(matches!(
        service.set_logo(Some(file.path())),
        Err(ServiceError::Domain(DomainError::InvalidLogo))
    ));
    assert!(matches!(
        service.set_logo(Some(std::path::Path::new("/no/such/logo.png"))),
        Err(ServiceError::ReadLogo { .. })
    ));
    // Starts like a PNG but can't be drawn
    let broken = logo_file(&LOGO_PNG[..40]);
    assert!(matches!(
        service.set_logo(Some(broken.path())),
        Err(ServiceError::Pdf(_))
    ));
    assert!(service.profile().unwrap().unwrap().logo().is_none());
}

#[test]
fn a_logo_needs_a_profile_first() {
    let store = Store::in_memory().unwrap();
    let mut service = InvoiceService::new(store.repository(), None);
    let file = logo_file(&LOGO_PNG);
    assert!(matches!(
        service.set_logo(Some(file.path())),
        Err(ServiceError::Invoicing(InvoicingError::NoProfile))
    ));
}

/// A note taking `taxable` before GST off the line billing `milestone`
fn credit(milestone: MilestoneId, taxable: i64, reason: &str) -> crate::services::CreditDraft {
    crate::services::CreditDraft {
        date: date(),
        lines: vec![CreditLineDraft {
            milestone_id: milestone,
            taxable_value: rupees(taxable),
        }],
        reason: reason.to_owned(),
        returned: crate::domain::ReturnTerms::default(),
    }
}

#[test]
fn a_part_credit_reverses_its_share_of_tax_and_lowers_the_balance() {
    let (store, mut service, project) = setup();
    let milestone = project.milestones()[0].id();
    let number = service
        .issue(project.id(), &[milestone], date(), None)
        .unwrap()
        .invoice
        .number()
        .clone();

    let preview = service
        .preview_credit(&number, credit(milestone, 10_000, "Discount agreed"))
        .unwrap();
    assert_eq!(preview.number.as_str(), "CN/26-27/001");
    assert_eq!(preview.amounts.igst, rupees(1_800));
    assert_eq!(preview.balance_after, rupees(47_200));
    let year = crate::domain::FiscalYear::containing(date());
    assert_eq!(store.repository().next_credit_sequence(year).unwrap(), 1);

    let credited = service
        .issue_credit(&number, credit(milestone, 10_000, "Discount agreed"))
        .unwrap();
    assert_eq!(credited.note.number(), &preview.number);
    assert_eq!(credited.invoice.balance(), rupees(47_200));
    assert_eq!(credited.invoice.status(), InvoiceStatus::Issued);
    assert!(credited.note.gst().is_some());

    assert_eq!(balance(&store, chart::RECEIVABLE), rupees(47_200));
    assert_eq!(balance(&store, chart::IGST_OUTPUT), rupees(7_200));
    assert_eq!(balance(&store, chart::CREDIT_NOTES), rupees(10_000));
    let stored = store.repository().load_invoice(&number).unwrap();
    assert_eq!(stored.credits()[0].number.as_str(), "CN/26-27/001");
    assert_eq!(
        store.repository().list_invoices().unwrap()[0].balance(),
        &rupees(47_200)
    );
    // The milestone is still waiting for the rest, which is ₹40,000 before GST
    let project = store.repository().load(project.id()).unwrap();
    assert_eq!(project.milestones()[0].status(), MilestoneStatus::Invoiced);
    assert_eq!(project.milestones()[0].credited(), &rupees(10_000));
    assert_eq!(project.outstanding_amount(), rupees(40_000));
}

#[test]
fn a_full_credit_cancels_the_invoice_and_its_milestone() {
    let (store, mut service, project) = setup();
    let milestone = project.milestones()[0].id();
    let number = service
        .issue(project.id(), &[milestone], date(), None)
        .unwrap()
        .invoice
        .number()
        .clone();
    let credited = service
        .issue_credit(&number, credit(milestone, 50_000, "Billed in error"))
        .unwrap();
    assert_eq!(credited.invoice.status(), InvoiceStatus::Credited);
    assert_eq!(balance(&store, chart::RECEIVABLE), rupees(0));
    assert!(store.repository().list_invoices().unwrap()[0].is_settled());
    let project = store.repository().load(project.id()).unwrap();
    assert_eq!(project.milestones()[0].status(), MilestoneStatus::Credited);
    // Nothing was received, and nothing is still owed
    assert_eq!(project.paid_amount(), rupees(0));
    assert_eq!(project.credited_amount(), rupees(50_000));
    assert_eq!(project.outstanding_amount(), rupees(0));
    assert_eq!(project.status(), ProjectStatus::Paid);

    // Nothing is left to credit or pay
    assert!(
        service
            .issue_credit(&number, credit(milestone, 1, "again"))
            .is_err()
    );
    assert!(
        service
            .record_payment(&number, date(), rupees(1), rupees(0))
            .is_err()
    );
}

fn credit_back(
    milestone: MilestoneId,
    taxable: i64,
    kind: ReturnKind,
) -> crate::services::CreditDraft {
    crate::services::CreditDraft {
        returned: ReturnTerms {
            kind,
            ..ReturnTerms::default()
        },
        ..credit(milestone, taxable, "Discount")
    }
}

#[test]
fn a_credit_beyond_what_is_due_refunds_the_rest() {
    let (store, mut service, project) = setup();
    let milestone = project.milestones()[0].id();
    let number = service
        .issue(project.id(), &[milestone], date(), None)
        .unwrap()
        .invoice
        .number()
        .clone();
    // ₹11,800 is still due: ₹10,000 plus IGST
    service
        .record_payment(&number, date(), rupees(47_200), rupees(0))
        .unwrap();
    // ₹20,000 plus IGST is ₹23,600: ₹11,800 comes off what is due, ₹11,800 is refunded
    let preview = service
        .preview_credit(
            &number,
            credit_back(milestone, 20_000, ReturnKind::Refunded),
        )
        .unwrap();
    assert_eq!(preview.balance_after, rupees(0));
    let returned = preview.returned.unwrap();
    assert_eq!(
        (returned.amount, returned.kind),
        (rupees(11_800), ReturnKind::Refunded)
    );
    let credited = service
        .issue_credit(
            &number,
            credit_back(milestone, 20_000, ReturnKind::Refunded),
        )
        .unwrap();
    assert_eq!(credited.invoice.status(), InvoiceStatus::Paid);
    assert_eq!(credited.invoice.balance(), rupees(0));
    assert_eq!(balance(&store, chart::RECEIVABLE), rupees(0));
    assert_eq!(balance(&store, chart::BANK), rupees(35_400));
    assert_eq!(balance(&store, chart::CREDIT_NOTES), rupees(20_000));
    let stored = store.repository().load_invoice(&number).unwrap();
    assert_eq!(stored.returned(), rupees(11_800));
    let project = store.repository().load(project.id()).unwrap();
    assert_eq!(project.milestones()[0].status(), MilestoneStatus::Paid);
    // Paid counts only what was kept: ₹50,000 billed, ₹20,000 of it credited
    assert_eq!(project.paid_amount(), rupees(30_000));
    assert_eq!(project.outstanding_amount(), rupees(0));
    // The refund comes back out of the year's receipts
    let report = crate::services::ReportService::new(store.repository(), None)
        .year(crate::domain::FiscalYear::containing(date()))
        .unwrap();
    assert_eq!(report.received, rupees(35_400));
}

#[test]
fn credits_on_a_paid_invoice_can_be_kept_as_an_advance_up_to_its_total() {
    let (store, mut service, project) = setup();
    let milestone = project.milestones()[0].id();
    let number = service
        .issue(project.id(), &[project.milestones()[0].id()], date(), None)
        .unwrap()
        .invoice
        .number()
        .clone();
    service
        .record_payment(&number, date(), rupees(59_000), rupees(0))
        .unwrap();
    let credited = service
        .issue_credit(&number, credit_back(milestone, 10_000, ReturnKind::Advance))
        .unwrap();
    assert_eq!(credited.invoice.status(), InvoiceStatus::Paid);
    assert_eq!(balance(&store, chart::RECEIVABLE), rupees(0));
    assert_eq!(balance(&store, chart::BANK), rupees(59_000));
    assert_eq!(balance(&store, chart::CUSTOMER_ADVANCES), rupees(11_800));
    let client = credited.invoice.client_name().to_owned();
    assert_eq!(service.client_advance(&client).unwrap(), rupees(11_800));
    assert_eq!(service.client_advance("Someone else").unwrap(), rupees(0));

    // Every note together takes off at most the invoice's ₹59,000
    assert!(matches!(
        service.issue_credit(&number, credit_back(milestone, 40_001, ReturnKind::Advance)),
        Err(ServiceError::Domain(
            DomainError::CreditExceedsInvoice { .. }
        ))
    ));
    service
        .issue_credit(
            &number,
            credit_back(milestone, 40_000, ReturnKind::Refunded),
        )
        .unwrap();
    assert_eq!(balance(&store, chart::RECEIVABLE), rupees(0));
    assert_eq!(balance(&store, chart::BANK), rupees(11_800));
    assert!(
        service
            .issue_credit(&number, credit(milestone, 1, "again"))
            .is_err()
    );
    let project = store.repository().load(project.id()).unwrap();
    assert_eq!(project.milestones()[0].status(), MilestoneStatus::Paid);
    assert_eq!(project.milestones()[0].credited(), &rupees(50_000));
}

/// A refund of `taxable` plus GST, with `tds` of it reversed by the client
fn refund_with_tds(
    milestone: MilestoneId,
    taxable: i64,
    tds: Option<i64>,
) -> crate::services::CreditDraft {
    crate::services::CreditDraft {
        returned: ReturnTerms {
            tds_reversed: tds.map(rupees),
            ..ReturnTerms::default()
        },
        ..credit(milestone, taxable, "Cancelled")
    }
}

#[test]
fn a_refund_never_hands_back_tds_the_client_kept_unless_it_is_reversed() {
    let (store, mut service, project) = setup();
    let milestone = project.milestones()[0].id();
    let number = service
        .issue(project.id(), &[project.milestones()[0].id()], date(), None)
        .unwrap()
        .invoice
        .number()
        .clone();
    // ₹59,000: ₹54,000 banked, ₹5,000 TDS withheld
    service
        .record_payment(&number, date(), rupees(54_000), rupees(5_000))
        .unwrap();

    // Cancelling it all would hand back ₹59,000, but only ₹54,000 was received
    assert!(matches!(
        service.issue_credit(&number, refund_with_tds(milestone, 50_000, None)),
        Err(ServiceError::Domain(
            DomainError::ReturnExceedsReceived { .. }
        ))
    ));
    assert!(matches!(
        service.issue_credit(&number, refund_with_tds(milestone, 50_000, Some(6_000))),
        Err(ServiceError::Domain(
            DomainError::TdsReversedExceedsWithheld { .. }
        ))
    ));
    // The client reverses its TDS: only the cash goes back
    service
        .issue_credit(&number, refund_with_tds(milestone, 50_000, Some(5_000)))
        .unwrap();
    assert_eq!(balance(&store, chart::RECEIVABLE), rupees(0));
    assert_eq!(balance(&store, chart::BANK), rupees(0));
    assert_eq!(balance(&store, chart::TDS_RECEIVABLE), rupees(0));
    let report = crate::services::ReportService::new(store.repository(), None)
        .year(crate::domain::FiscalYear::containing(date()))
        .unwrap();
    assert_eq!(report.received, rupees(0));
    assert_eq!(report.tds, rupees(0));
}

#[test]
fn a_part_refund_can_reverse_part_of_the_tds() {
    let (store, mut service, project) = setup();
    let milestone = project.milestones()[0].id();
    let number = service
        .issue(project.id(), &[project.milestones()[0].id()], date(), None)
        .unwrap()
        .invoice
        .number()
        .clone();
    service
        .record_payment(&number, date(), rupees(54_000), rupees(5_000))
        .unwrap();
    // ₹10,000 + IGST is ₹11,800 back: ₹1,000 of it TDS the client reverses, ₹10,800 refunded
    let preview = service
        .preview_credit(&number, refund_with_tds(milestone, 10_000, Some(1_000)))
        .unwrap();
    assert_eq!(preview.returned.unwrap().cash(), rupees(10_800));
    service
        .issue_credit(&number, refund_with_tds(milestone, 10_000, Some(1_000)))
        .unwrap();
    assert_eq!(balance(&store, chart::RECEIVABLE), rupees(0));
    assert_eq!(balance(&store, chart::BANK), rupees(43_200));
    assert_eq!(balance(&store, chart::TDS_RECEIVABLE), rupees(4_000));
    let report = crate::services::ReportService::new(store.repository(), None)
        .year(crate::domain::FiscalYear::containing(date()))
        .unwrap();
    assert_eq!(report.received, rupees(43_200));
    assert_eq!(report.tds, rupees(4_000));
    assert_eq!(report.clients[0].tds, rupees(4_000));
}

#[test]
fn a_foreign_refund_books_the_rupees_that_left_the_bank() {
    let store = Store::in_memory().unwrap();
    let mut service = InvoiceService::new(store.repository(), None);
    service
        .save_profile(profile_draft(Some(SELLER_GSTIN), Some("AD2904260000001")))
        .unwrap();
    let project = fixed_project(foreign_client(Currency::Eur), 1_000);
    store.repository().save(&project).unwrap();
    let milestone = project.milestones()[0].id();
    let rate = ExchangeRate::new(BigDecimal::from(90)).unwrap();
    let number = service
        .issue(
            project.id(),
            &[project.milestones()[0].id()],
            date(),
            Some(rate),
        )
        .unwrap()
        .invoice
        .number()
        .clone();
    service
        .record_foreign_payment(&number, date(), rupees(1_000), rupees(91_000))
        .unwrap();

    assert!(matches!(
        service.issue_credit(&number, credit_back(milestone, 100, ReturnKind::Refunded)),
        Err(ServiceError::Domain(DomainError::MissingRefundedAmount))
    ));
    // €100 was booked at ₹9,000; refunding it cost ₹9,200
    let refund = crate::services::CreditDraft {
        returned: ReturnTerms {
            refunded_inr: Some(rupees(9_200)),
            ..ReturnTerms::default()
        },
        ..credit(milestone, 100, "Discount")
    };
    service.issue_credit(&number, refund).unwrap();
    assert_eq!(balance(&store, chart::RECEIVABLE), rupees(0));
    assert_eq!(balance(&store, chart::BANK), rupees(81_800));
    // ₹1,000 gained on the payment, ₹200 lost on the refund
    assert_eq!(balance(&store, chart::EXCHANGE_DIFFERENCE), rupees(800));
    let report = crate::services::ReportService::new(store.repository(), None)
        .year(crate::domain::FiscalYear::containing(date()))
        .unwrap();
    assert_eq!(report.received, rupees(81_800));
    assert_eq!(report.exchange_difference, rupees(800));
}

#[test]
fn credits_on_a_foreign_invoice_are_at_its_rate_without_gst() {
    let store = Store::in_memory().unwrap();
    let mut service = InvoiceService::new(store.repository(), None);
    service
        .save_profile(profile_draft(Some(SELLER_GSTIN), Some("AD2904260000001")))
        .unwrap();
    let project = fixed_project(foreign_client(Currency::Eur), 1_000);
    store.repository().save(&project).unwrap();
    let milestone = project.milestones()[0].id();
    let rate = ExchangeRate::new(BigDecimal::from(90)).unwrap();
    let number = service
        .issue(
            project.id(),
            &[project.milestones()[0].id()],
            date(),
            Some(rate),
        )
        .unwrap()
        .invoice
        .number()
        .clone();

    let credited = service
        .issue_credit(&number, credit(milestone, 1_000, "Cancelled"))
        .unwrap();
    assert!(credited.note.gst().is_none());
    assert_eq!(credited.note.currency(), Currency::Eur);
    assert_eq!(credited.note.inr_amounts().total, rupees(90_000));
    assert_eq!(credited.invoice.status(), InvoiceStatus::Credited);
    assert_eq!(balance(&store, chart::RECEIVABLE), rupees(0));
    assert_eq!(balance(&store, chart::CREDIT_NOTES), rupees(90_000));
}

#[test]
fn credit_notes_close_on_30_november_after_the_invoice_year() {
    let (_store, mut service, project) = setup();
    let milestone = project.milestones()[0].id();
    let number = service
        .issue(project.id(), &[project.milestones()[0].id()], date(), None)
        .unwrap()
        .invoice
        .number()
        .clone();
    // Invoiced in FY 2026-27, so the last day is 30 November 2027
    let late = crate::services::CreditDraft {
        date: chrono::NaiveDate::from_ymd_opt(2027, 12, 1).unwrap(),
        ..credit(milestone, 1_000, "Late discount")
    };
    assert!(matches!(
        service.preview_credit(&number, late),
        Err(ServiceError::Invoicing(InvoicingError::CreditNoteTooLate(day)))
            if day == chrono::NaiveDate::from_ymd_opt(2027, 11, 30).unwrap()
    ));
}

#[test]
fn invoices_cannot_take_the_credit_note_prefix() {
    let store = Store::in_memory().unwrap();
    let mut service = InvoiceService::new(store.repository(), None);
    let mut draft = profile_draft(Some(SELLER_GSTIN), None);
    draft.invoice_prefix = "cn".to_owned();
    assert!(matches!(
        service.save_profile(draft),
        Err(ServiceError::Domain(DomainError::ReservedInvoicePrefix(_)))
    ));
}

/// The ₹50,000 inter-state project from `setup`, with Build (₹10,000, due a week later) and
/// Launch (₹20,000, due a day earlier) added: due-date order is Launch, Design, Build
fn three_milestones() -> (
    Store,
    InvoiceService<RedbRepository>,
    crate::domain::Project,
    Vec<MilestoneId>,
) {
    let (store, service, mut project) = setup();
    project
        .add_milestone("Build", date() + chrono::Days::new(7), Some(rupees(10_000)))
        .unwrap();
    project
        .add_milestone(
            "Launch",
            date() - chrono::Days::new(1),
            Some(rupees(20_000)),
        )
        .unwrap();
    store.repository().save(&project).unwrap();
    let ids = project.milestones().iter().map(|m| m.id()).collect();
    (store, service, project, ids)
}

fn line(milestone: MilestoneId, taxable: &str) -> CreditLineDraft {
    CreditLineDraft {
        milestone_id: milestone,
        taxable_value: taxable.parse().unwrap(),
    }
}

fn credit_lines(lines: Vec<CreditLineDraft>) -> crate::services::CreditDraft {
    crate::services::CreditDraft {
        date: date(),
        lines,
        reason: "Scope cut".to_owned(),
        returned: ReturnTerms::default(),
    }
}

#[test]
fn several_milestones_are_billed_together_in_due_date_order() {
    let (store, mut service, project, ids) = three_milestones();
    let preview = service
        .preview(project.id(), &[ids[1], ids[0], ids[2]], date(), None)
        .unwrap();
    assert_eq!(preview.amounts.taxable_value, rupees(80_000));
    assert_eq!(preview.amounts.igst, rupees(14_400));

    let issued = service
        .issue(project.id(), &[ids[1], ids[0], ids[2]], date(), None)
        .unwrap();
    let invoice = &issued.invoice;
    let names: Vec<_> = invoice
        .lines()
        .iter()
        .map(|l| l.milestone_name.as_str())
        .collect();
    assert_eq!(names, ["Launch", "Design", "Build"]);
    assert_eq!(invoice.print().rows.len(), 3);
    assert_eq!(invoice.gst().unwrap().line_items.len(), 3);
    assert_eq!(invoice.amounts().total, rupees(94_400));
    assert_eq!(
        service.list().unwrap()[0].milestones(),
        "Launch, Design, Build"
    );
    assert_eq!(balance(&store, chart::RECEIVABLE), rupees(94_400));

    let stored = store.repository().load(project.id()).unwrap();
    for m in stored.milestones() {
        assert_eq!(m.status(), MilestoneStatus::Invoiced);
        assert_eq!(m.invoice_number(), Some("INV/26-27/001"));
    }

    // Paying the whole invoice pays every milestone on it
    service
        .record_payment(invoice.number(), date(), rupees(94_400), rupees(0))
        .unwrap();
    let stored = store.repository().load(project.id()).unwrap();
    assert!(
        stored
            .milestones()
            .iter()
            .all(|m| m.status() == MilestoneStatus::Paid)
    );
    assert_eq!(stored.status(), ProjectStatus::Paid);
}

#[test]
fn an_invoice_bills_only_pending_milestones_of_its_project_each_once() {
    let (_store, mut service, project, ids) = three_milestones();
    assert!(matches!(
        service.preview(project.id(), &[], date(), None),
        Err(ServiceError::Invoicing(InvoicingError::NothingSelected))
    ));
    assert!(matches!(
        service.preview(project.id(), &[ids[0], ids[0]], date(), None),
        Err(ServiceError::Domain(DomainError::DuplicateInvoiceLine))
    ));
    assert!(matches!(
        service.preview(
            project.id(),
            &[ids[0], MilestoneId::generate()],
            date(),
            None
        ),
        Err(ServiceError::Invoicing(InvoicingError::NotPending))
    ));
    service
        .issue(project.id(), &[ids[0]], date(), None)
        .unwrap();
    // One already invoiced leaves the others pending
    assert!(matches!(
        service.issue(project.id(), &[ids[1], ids[0]], date(), None),
        Err(ServiceError::Invoicing(InvoicingError::NotPending))
    ));
    assert_eq!(service.list().unwrap().len(), 1);
}

#[test]
fn a_credit_note_takes_back_from_the_lines_picked() {
    let (store, mut service, project, ids) = three_milestones();
    let number = service
        .issue(project.id(), &ids, date(), None)
        .unwrap()
        .invoice
        .number()
        .clone();
    // Build in full and ₹5,000 of Launch; Design untouched
    let draft = credit_lines(vec![line(ids[1], "10000"), line(ids[2], "5000")]);
    let credited = service.issue_credit(&number, draft).unwrap();
    assert_eq!(credited.note.amounts().taxable_value, rupees(15_000));
    assert_eq!(credited.note.amounts().igst, rupees(2_700));
    assert_eq!(credited.note.lines().len(), 2);
    let rows: Vec<_> = credited
        .note
        .print()
        .rows
        .iter()
        .map(|r| r.description.as_str())
        .collect();
    // In the invoice's order: Launch before Build
    assert_eq!(
        rows,
        ["Website - Launch: Scope cut", "Website - Build: Scope cut"]
    );
    assert_eq!(credited.invoice.balance(), rupees(76_700));

    let stored = store.repository().load(project.id()).unwrap();
    let credited_of = |id| {
        stored.milestones()[stored.milestone_index(id).unwrap()]
            .credited()
            .clone()
    };
    assert_eq!(credited_of(ids[0]), rupees(0));
    assert_eq!(credited_of(ids[1]), rupees(10_000));
    assert_eq!(credited_of(ids[2]), rupees(5_000));
    // Build, taken back in full, is credited; the others are still due
    let status_of = |id| stored.milestones()[stored.milestone_index(id).unwrap()].status();
    assert_eq!(status_of(ids[0]), MilestoneStatus::Invoiced);
    assert_eq!(status_of(ids[1]), MilestoneStatus::Credited);
    assert_eq!(status_of(ids[2]), MilestoneStatus::Invoiced);

    let lines = service
        .creditable_lines(&service.load(&number).unwrap())
        .unwrap();
    let left: Vec<_> = lines.iter().map(|l| l.creditable.clone()).collect();
    assert_eq!(left, [rupees(15_000), rupees(50_000), rupees(0)]);

    // Paying what is left pays the others; Build stays credited
    service
        .record_payment(&number, date(), rupees(76_700), rupees(0))
        .unwrap();
    let stored = store.repository().load(project.id()).unwrap();
    let status_of = |id| stored.milestones()[stored.milestone_index(id).unwrap()].status();
    assert_eq!(status_of(ids[0]), MilestoneStatus::Paid);
    assert_eq!(status_of(ids[1]), MilestoneStatus::Credited);
    assert_eq!(status_of(ids[2]), MilestoneStatus::Paid);
}

#[test]
fn a_line_is_credited_at_most_what_it_billed() {
    let (store, mut service, project, ids) = three_milestones();
    let number = service
        .issue(project.id(), &ids, date(), None)
        .unwrap()
        .invoice
        .number()
        .clone();
    // The invoice has ₹80,000 to credit, but Build billed only ₹10,000
    let draft = credit_lines(vec![line(ids[1], "10000.01")]);
    assert!(matches!(
        service.preview_credit(&number, draft.clone()),
        Err(ServiceError::Domain(DomainError::CreditExceedsMilestone))
    ));
    assert!(matches!(
        service.issue_credit(&number, draft),
        Err(ServiceError::Domain(DomainError::CreditExceedsMilestone))
    ));
    assert!(matches!(
        service.issue_credit(
            &number,
            credit_lines(vec![line(MilestoneId::generate(), "1")])
        ),
        Err(ServiceError::Invoicing(InvoicingError::Domain(
            DomainError::NotOnInvoice
        )))
    ));
    assert!(matches!(
        service.issue_credit(&number, credit_lines(Vec::new())),
        Err(ServiceError::Invoicing(InvoicingError::Domain(
            DomainError::NoCreditLines
        )))
    ));
    assert!(
        store
            .repository()
            .load_invoice(&number)
            .unwrap()
            .credits()
            .is_empty()
    );
}

#[test]
fn crediting_every_line_in_full_cancels_the_invoice_and_its_milestones() {
    let (store, mut service, mut project) = setup();
    // Odd paise, so GST on each line rounds
    project
        .edit_milestone(0, "Design", date(), Some("333.33".parse().unwrap()))
        .unwrap();
    project
        .add_milestone("Build", date(), Some("333.33".parse().unwrap()))
        .unwrap();
    project
        .add_milestone("Launch", date(), Some("333.34".parse().unwrap()))
        .unwrap();
    store.repository().save(&project).unwrap();
    let ids: Vec<_> = project.milestones().iter().map(|m| m.id()).collect();
    let invoice = service
        .issue(project.id(), &ids, date(), None)
        .unwrap()
        .invoice;

    let lines = service.creditable_lines(&invoice).unwrap();
    let draft = credit_lines(
        lines
            .iter()
            .map(|l| CreditLineDraft {
                milestone_id: l.milestone_id,
                taxable_value: l.creditable.clone(),
            })
            .collect(),
    );
    let credited = service.issue_credit(invoice.number(), draft).unwrap();
    assert_eq!(credited.note.amounts().total, invoice.amounts().total);
    assert_eq!(credited.invoice.status(), InvoiceStatus::Credited);
    assert_eq!(balance(&store, chart::RECEIVABLE), rupees(0));
    let stored = store.repository().load(project.id()).unwrap();
    assert!(
        stored
            .milestones()
            .iter()
            .all(|m| m.status() == MilestoneStatus::Credited)
    );
}

#[test]
fn a_line_takes_a_credit_of_at_most_what_is_left_on_it() {
    let (_store, mut service, project, ids) = three_milestones();
    let invoice = service
        .issue(project.id(), &ids, date(), None)
        .unwrap()
        .invoice;
    assert!(invoice.untouched());
    let lines = service.creditable_lines(&invoice).unwrap();
    let build = lines.iter().find(|l| l.name == "Build").unwrap();
    assert_eq!(
        build.check(&"10000.000".parse().unwrap()).unwrap(),
        rupees(10_000)
    );
    // A fraction of a paisa is a mistyped amount, not one to round: 5000.004000 is not 5000
    assert!(matches!(
        build.check(&"5000.004000".parse().unwrap()),
        Err(DomainError::FractionOfPaisa)
    ));
    assert!(matches!(
        build.check(&"10000.01".parse().unwrap()),
        Err(DomainError::CreditExceedsLine { .. })
    ));
    assert!(matches!(
        build.check(&rupees(0)),
        Err(DomainError::NonPositiveAmount)
    ));
}

#[test]
fn picked_lines_are_checked_together_and_a_refusal_names_its_line() {
    let (_store, mut service, project, ids) = three_milestones();
    let invoice = service
        .issue(project.id(), &ids, date(), None)
        .unwrap()
        .invoice;
    let ok = [line(ids[0], "50000"), line(ids[1], "10000")];
    service.check_credit_lines(&invoice, &ok).unwrap();

    let over = [line(ids[0], "50000"), line(ids[1], "10000.01")];
    match service.check_credit_lines(&invoice, &over) {
        Err(ServiceError::CreditLine {
            milestone,
            name,
            source,
        }) => {
            assert_eq!(milestone, ids[1]);
            assert_eq!(name, "Build");
            assert!(matches!(*source, DomainError::CreditExceedsLine { .. }));
        }
        other => panic!("{other:?}"),
    }
    let error = service.check_credit_lines(&invoice, &over).unwrap_err();
    assert!(
        error.to_string().starts_with("Build: only 10000.00"),
        "{error}"
    );
    assert!(matches!(
        service.check_credit_lines(&invoice, &[line(ids[1], "1"), line(ids[1], "2")]),
        Err(ServiceError::CreditLine { source, .. })
            if matches!(*source, DomainError::DuplicateCreditLine)
    ));
    assert!(matches!(
        service.check_credit_lines(&invoice, &[]),
        Err(ServiceError::Domain(DomainError::NoCreditLines))
    ));
    // The note itself refuses a fraction of a paisa too
    assert!(
        service
            .preview_credit(
                invoice.number(),
                credit_lines(vec![line(ids[1], "5000.004")])
            )
            .is_err()
    );
}
