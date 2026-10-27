mod support;

use std::path::Path;

use compter_mobile::api::{CompterApp, ProfileInput, ReportRow, Tone};
use support::demo;

fn figure<'a>(report: &'a ReportRow, label: &str) -> &'a str {
    &report
        .income
        .iter()
        .chain(&report.income_tax)
        .find(|f| f.label == label)
        .unwrap()
        .value
}

#[test]
fn a_year_report_has_income_tax_gst_and_clients() {
    let (_scratch, app) = demo();

    let report = app.report(2026).unwrap();

    assert_eq!(report.year, "FY 2026-27");
    assert_eq!(figure(&report, "Invoices"), "5");
    assert_eq!(figure(&report, "Receipts"), "₹2,50,750.00");
    assert_eq!(figure(&report, "Exchange gain"), "₹450.00");
    assert_eq!(figure(&report, "44ADA income (50%)"), "₹1,25,375.00");
    assert_eq!(figure(&report, "Books profit"), "₹89,200.00");
    assert_eq!(report.income_tax_tone, Tone::Warning);
    assert_eq!(report.gst.len(), 4);
    assert_eq!(report.gst[1].quarter, "Q2 (Jul-Sep)");
    assert_eq!(report.gst[1].net, "-₹13,680.00");
    assert_eq!(report.clients[0].client, "Acme Pvt Ltd");
    assert_eq!(report.quarters.len(), 4);
    assert_eq!(report.iff_months[0], "Apr 2026");
    assert_eq!(report.iff_months[7], "Feb 2027");
    assert!((1..=4).contains(&report.quarter_to_file));
}

#[test]
fn the_cash_basis_counts_what_was_received() {
    let (_scratch, mut app) = demo();
    assert_eq!(
        figure(&app.report(2026).unwrap(), "Receipts (accrual)"),
        "₹2,50,750.00"
    );
    let input = app.profile_input().unwrap();
    assert!(!input.cash_basis);
    let saved = app
        .save_profile(ProfileInput {
            cash_basis: true,
            basis_from: "2026-27".to_owned(),
            ..input
        })
        .unwrap();
    assert_eq!(saved.income_basis, "accrual, cash from FY 2026-27");

    let report = app.report(2026).unwrap();
    assert_eq!(figure(&report, "Receipts (cash)"), "₹1,70,450.00");
    assert_eq!(figure(&report, "44ADA income (50%)"), "₹85,225.00");
    // The Income panel still shows what was invoiced
    assert_eq!(figure(&report, "Receipts"), "₹2,50,750.00");
}

#[test]
fn returns_are_written_for_the_gst_portal() {
    let (_scratch, app) = demo();

    let gstr1 = app.export_gstr1(2026, 2, false, false).unwrap();
    assert!(Path::new(&gstr1.path).exists(), "{}", gstr1.path);
    assert!(gstr1.path.ends_with(".json"), "{}", gstr1.path);
    assert!(
        gstr1.summary.starts_with("GSTR-1 Q2 (Jul-Sep) 2026-27: "),
        "{}",
        gstr1.summary
    );

    let iff = app.export_iff(2026, 3).unwrap();
    assert!(Path::new(&iff.path).exists(), "{}", iff.path);
    assert!(iff.summary.starts_with("IFF Aug 2026: "), "{}", iff.summary);

    let wrong = app.export_gstr1(2026, 5, false, false).unwrap_err();
    assert_eq!(wrong.field.as_deref(), Some("quarter"));
    let wrong = app.export_iff(2026, 8).unwrap_err();
    assert_eq!(wrong.field.as_deref(), Some("month"));
}

#[test]
fn returns_need_a_gstin() {
    let dir = tempfile::tempdir().unwrap();
    let app = CompterApp::open(dir.path().to_string_lossy().into_owned()).unwrap();

    let error = app.export_gstr1(2026, 2, false, false).unwrap_err();

    assert_eq!(error.field, None);
    assert!(error.message.contains("GSTIN"), "{}", error.message);
}

#[test]
fn a_change_of_basis_needs_a_financial_year() {
    let (_scratch, mut app) = demo();
    let error = app
        .save_profile(ProfileInput {
            cash_basis: true,
            basis_from: "April".to_owned(),
            ..app.profile_input().unwrap()
        })
        .unwrap_err();
    assert_eq!(error.field.as_deref(), Some("basis_from"));
    // Earlier years keep accrual
    let report = app.report(2025).unwrap();
    assert_eq!(report.income_tax_switch, None);
    assert_eq!(figure(&report, "Receipts (accrual)"), "₹45,000.00");
}
