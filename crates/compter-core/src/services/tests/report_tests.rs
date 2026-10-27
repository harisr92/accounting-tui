use bigdecimal::BigDecimal;
use chrono::NaiveDate;

use crate::demo;
use crate::domain::{ExpenseCategory, FiscalYear, IncomeBasis};
use crate::services::reports::PresumptiveCheck;
use crate::services::{InvoiceService, ReportService};
use crate::storage::{ProjectRepository, Store};

fn rupees(amount: i64) -> BigDecimal {
    BigDecimal::from(amount)
}

fn fy(start_year: i32) -> FiscalYear {
    FiscalYear::containing(NaiveDate::from_ymd_opt(start_year, 4, 1).unwrap())
}

#[test]
fn demo_year_adds_up() {
    let store = demo::store().unwrap();
    let report = ReportService::new(store.repository(), None)
        .year(fy(2026))
        .unwrap();

    // Export €1,000 at ₹90 + Design 50,000 + Dashboards 35,000 + Frontend 60,000 + Prototype 15,750
    assert_eq!(report.invoice_count, 5);
    assert_eq!(report.gross_receipts, rupees(250_750));
    assert_eq!(report.export_receipts, rupees(90_000));
    // The plain invoice was paid in March 2026, the previous year; Frontend is half paid; the
    // export banked ₹90,450, ₹450 more than it was invoiced at
    assert_eq!(report.received, rupees(179_850));
    assert_eq!(report.exchange_difference, rupees(450));
    assert_eq!(report.tds, rupees(5_000));
    assert_eq!(report.outstanding, rupees(95_285));

    assert_eq!(report.expenses, rupees(162_000));
    assert_eq!(report.books_profit(), rupees(89_200));
    assert_eq!(report.presumptive_income(), rupees(125_375));
    assert!(report.within_presumptive_limit());
    assert!(
        report
            .expenses_by_category
            .contains(&(ExpenseCategory::Equipment, rupees(150_000)))
    );

    let [q1, q2, q3, _] = &report.quarters;
    assert_eq!(q1.output(), rupees(0));
    assert_eq!(
        (&q2.cgst, &q2.sgst, &q2.igst),
        (&rupees(4_500), &rupees(4_500), &rupees(6_300))
    );
    assert_eq!(q2.input_credit, rupees(28_980));
    assert_eq!(q2.net(), rupees(15_300) - rupees(28_980));
    assert_eq!(q3.output(), rupees(10_800) + rupees(2_835));

    let acme = report
        .clients
        .iter()
        .find(|c| c.client == "Acme Pvt Ltd")
        .unwrap();
    assert_eq!(
        (&acme.invoiced, &acme.received, &acme.tds),
        (&rupees(110_000), &rupees(89_400), &rupees(5_000))
    );
    assert_eq!(report.clients[0].client, "Acme Pvt Ltd");
}

#[test]
fn previous_year_sees_only_the_plain_invoice() {
    let store = demo::store().unwrap();
    let report = ReportService::new(store.repository(), None)
        .year(fy(2025))
        .unwrap();
    assert_eq!(report.invoice_count, 1);
    assert_eq!(report.gross_receipts, rupees(45_000));
    assert_eq!(
        (&report.received, &report.tds),
        (&rupees(40_500), &rupees(4_500))
    );
    assert_eq!(report.outstanding, rupees(0));
}

#[test]
fn credit_notes_come_off_the_year_they_are_dated_in() {
    let store = demo::store().unwrap();
    let mut invoices = crate::services::InvoiceService::new(store.repository(), None);
    // Frontend, ₹60,000 + GST, half paid
    let number = crate::domain::InvoiceNumber::from_stored("NDC/26-27/004");
    let draft = crate::services::CreditDraft {
        date: NaiveDate::from_ymd_opt(2027, 1, 15).unwrap(),
        lines: vec![crate::services::CreditLineDraft {
            milestone_id: invoices.load(&number).unwrap().lines()[0].milestone_id,
            taxable_value: rupees(10_000),
        }],
        reason: "Discount".to_owned(),
        returned: crate::domain::ReturnTerms::default(),
    };
    let note = invoices.issue_credit(&number, draft).unwrap().note;
    let tax = note.amounts().total_tax();

    let reports = ReportService::new(store.repository(), None);
    let report = reports.year(fy(2026)).unwrap();
    assert_eq!(report.gross_receipts, rupees(240_750));
    assert_eq!(report.credited, rupees(10_000));
    assert_eq!(report.outstanding, rupees(95_285) - rupees(10_000) - &tax);
    let q4 = &report.quarters[3];
    assert_eq!(q4.output(), -tax);
    // The next year is untouched
    assert_eq!(reports.year(fy(2027)).unwrap().credited, rupees(0));
}

#[test]
fn the_44ada_check_warns_when_the_books_show_less() {
    let store = demo::store().unwrap();
    let reports = ReportService::new(store.repository(), None);

    // Books profit ₹89,200 against ₹1,25,375 deemed under 44ADA
    let this_year = reports.year(fy(2026)).unwrap();
    assert_eq!(this_year.presumptive_check(), PresumptiveCheck::BooksBelow);
    assert!(this_year.presumptive_note().contains("44AB"));
    // A plain invoice and no expenses: the books show all of it
    let last_year = reports.year(fy(2025)).unwrap();
    assert_eq!(last_year.presumptive_check(), PresumptiveCheck::Within);
}

/// Save the demo profile with `basis` from each `(start year, basis)` in turn
fn use_bases(store: &Store, changes: &[(i32, IncomeBasis)]) {
    let mut invoices = InvoiceService::new(store.repository(), None);
    let mut draft = invoices.profile().unwrap().unwrap().to_draft();
    for &(from, basis) in changes {
        draft.income_basis.set(FiscalYear::starting(from), basis);
    }
    invoices.save_profile(draft).unwrap();
}

/// Website Revamp's Launch, ₹40,000 + 18% GST, invoiced in March 2026 and paid in full in April:
/// an invoice across the year boundary
fn invoice_launch_across_the_year_end(store: &Store) {
    let repo = store.repository();
    let website = repo
        .list()
        .unwrap()
        .into_iter()
        .find(|p| p.name() == "Website Revamp")
        .unwrap();
    let project = repo.load(website.id()).unwrap();
    let launch = project
        .milestones()
        .iter()
        .find(|m| m.name() == "Launch")
        .unwrap()
        .id();
    let mut invoices = InvoiceService::new(store.repository(), None);
    let date = |m, d| NaiveDate::from_ymd_opt(2026, m, d).unwrap();
    let issued = invoices
        .issue(website.id(), &[launch], date(3, 20), None, None)
        .unwrap();
    invoices
        .record_payment(
            issued.invoice.number(),
            date(4, 10),
            rupees(47_200),
            rupees(0),
        )
        .unwrap();
}

#[test]
fn the_cash_basis_counts_fees_received_in_the_year() {
    let store = demo::store().unwrap();
    use_bases(&store, &[(2025, IncomeBasis::Cash)]);
    let reports = ReportService::new(store.repository(), None);

    // The export banked ₹90,450; Design's ₹54,000 + ₹5,000 TDS settled ₹50,000 + GST in full;
    // Frontend's ₹35,400 is half of ₹60,000 + GST. Dashboards and Prototype are unpaid.
    let this_year = reports.year(fy(2026)).unwrap();
    assert_eq!(this_year.basis, IncomeBasis::Cash);
    assert_eq!(this_year.tax_receipts, rupees(170_450));
    assert_eq!(this_year.presumptive_income(), rupees(85_225));
    // The exchange gain is in the rupees banked, so it isn't added again
    assert_eq!(this_year.books_profit(), rupees(8_450));
    // The accrual figure is still there for the Income panel
    assert_eq!(this_year.gross_receipts, rupees(250_750));
    assert_eq!(this_year.presumptive_check(), PresumptiveCheck::BooksBelow);
    // Both years are on cash: nothing crossed over
    assert_eq!(this_year.switch_note(), None);

    // The plain invoice of March 2026 was paid that month, ₹40,500 + ₹4,500 TDS
    let last_year = reports.year(fy(2025)).unwrap();
    assert_eq!(last_year.tax_receipts, rupees(45_000));
    assert!(last_year.presumptive_note().contains("fees received"));
}

#[test]
fn switching_to_cash_leaves_out_payments_on_invoices_already_counted() {
    let store = demo::store().unwrap();
    invoice_launch_across_the_year_end(&store);
    use_bases(&store, &[(2026, IncomeBasis::Cash)]);
    let reports = ReportService::new(store.repository(), None);

    // 2025-26 stays on accrual: the plain invoice and Launch, at their dates
    let last_year = reports.year(fy(2025)).unwrap();
    assert_eq!(last_year.basis, IncomeBasis::Accrual);
    assert_eq!(last_year.tax_receipts, rupees(85_000));
    // 2026-27 on cash: Launch's April payment was counted in March, so not again
    let this_year = reports.year(fy(2026)).unwrap();
    assert_eq!(this_year.tax_receipts, rupees(170_450));
    assert_eq!(this_year.already_counted, rupees(40_000));
    assert!(
        this_year
            .switch_note()
            .unwrap()
            .contains("₹40,000.00 received this year is left out")
    );
}

#[test]
fn switching_to_accrual_counts_payments_on_invoices_never_counted() {
    let store = demo::store().unwrap();
    invoice_launch_across_the_year_end(&store);
    use_bases(
        &store,
        &[(2025, IncomeBasis::Cash), (2026, IncomeBasis::Accrual)],
    );
    let reports = ReportService::new(store.repository(), None);

    // 2025-26 on cash: Launch wasn't paid yet
    let last_year = reports.year(fy(2025)).unwrap();
    assert_eq!(last_year.tax_receipts, rupees(45_000));
    // 2026-27 on accrual: its own invoices at their dates, and Launch when paid
    let this_year = reports.year(fy(2026)).unwrap();
    assert_eq!(this_year.basis, IncomeBasis::Accrual);
    assert_eq!(this_year.tax_receipts, rupees(290_750));
    assert_eq!(this_year.carried_in, rupees(40_000));
    // The export was invoiced in an accrual year, so its exchange gain still counts
    assert_eq!(this_year.accrual_exchange, rupees(450));
    assert!(this_year.switch_note().unwrap().contains("₹40,000.00"));
}

#[test]
fn a_refund_comes_off_the_cash_receipts_without_its_gst() {
    let store = demo::store().unwrap();
    use_bases(&store, &[(2026, IncomeBasis::Cash)]);
    let mut invoices = InvoiceService::new(store.repository(), None);
    // Design, ₹50,000 + GST, paid in full: crediting ₹10,000 of it refunds ₹10,000 + GST, less
    // the TDS the client reverses
    let number = crate::domain::InvoiceNumber::from_stored("NDC/26-27/002");
    let draft = crate::services::CreditDraft {
        date: NaiveDate::from_ymd_opt(2027, 1, 15).unwrap(),
        lines: vec![crate::services::CreditLineDraft {
            milestone_id: invoices.load(&number).unwrap().lines()[0].milestone_id,
            taxable_value: rupees(10_000),
        }],
        reason: "Scope cut".to_owned(),
        returned: crate::domain::ReturnTerms {
            kind: crate::domain::ReturnKind::Refunded,
            refunded_inr: None,
            tds_reversed: Some(rupees(1_000)),
        },
    };
    invoices.issue_credit(&number, draft).unwrap();

    let report = ReportService::new(store.repository(), None)
        .year(fy(2026))
        .unwrap();
    // ₹11,800 went back: ₹10,800 refunded and ₹1,000 of TDS reversed, ₹10,000 of it fees. The
    // credit note itself takes nothing more off: the invoice was counted on cash.
    assert_eq!(report.tax_receipts, rupees(160_450));
    assert_eq!(report.gross_receipts, rupees(240_750));
}
