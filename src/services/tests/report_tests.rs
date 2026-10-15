use bigdecimal::BigDecimal;
use chrono::NaiveDate;

use crate::demo;
use crate::domain::{ExpenseCategory, FiscalYear};
use crate::services::ReportService;

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

    // Export 90,000 + Design 50,000 + Dashboards 35,000 + Frontend 60,000 + Prototype 15,750
    assert_eq!(report.invoice_count, 5);
    assert_eq!(report.gross_receipts, rupees(250_750));
    assert_eq!(report.export_receipts, rupees(90_000));
    // The plain invoice was paid in March 2026, the previous year; Frontend is half paid
    assert_eq!(report.received, rupees(179_400));
    assert_eq!(report.tds, rupees(5_000));
    assert_eq!(report.outstanding, rupees(95_285));

    assert_eq!(report.expenses, rupees(162_000));
    assert_eq!(report.books_profit(), rupees(88_750));
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
