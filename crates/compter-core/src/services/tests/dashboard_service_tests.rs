use bigdecimal::BigDecimal;
use chrono::NaiveDate;

use crate::demo;
use crate::domain::{Currency, InvoiceNumber, ReturnKind, ReturnTerms};
use crate::services::{CreditDraft, CreditLineDraft, DashboardService, InvoiceService};

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

fn rupees(amount: i64) -> BigDecimal {
    BigDecimal::from(amount)
}

#[test]
fn demo_shows_what_is_owed_and_what_is_overdue() {
    let store = demo::store().unwrap();
    let today = date(2026, 11, 1);
    let dashboard = DashboardService::new(store.repository())
        .now(today)
        .unwrap();

    // Dashboards ₹41,300, the unpaid half of Frontend ₹35,400, Prototype ₹18,585
    assert_eq!(dashboard.owed, rupees(95_285));
    let billed: Vec<_> = dashboard
        .unpaid
        .iter()
        .map(|s| s.milestones().to_owned())
        .collect();
    assert_eq!(billed, ["Dashboards", "Frontend", "Prototype"]);

    // Only Dashboards, issued 25 September, has waited more than 30 days
    assert_eq!(dashboard.overdue, rupees(41_300));
    assert_eq!(dashboard.overdue_count, 1);
    assert!(dashboard.unpaid[0].is_overdue(today));
    assert!(!dashboard.unpaid[1].is_overdue(today));
    assert_eq!(dashboard.unpaid[0].days_waiting(today), 37);
}

#[test]
fn received_counts_only_the_current_month() {
    let store = demo::store().unwrap();
    let service = DashboardService::new(store.repository());

    // Frontend's half payment on 12 October
    let october = service.now(date(2026, 10, 31)).unwrap();
    assert_eq!(october.received_this_month, rupees(35_400));
    let november = service.now(date(2026, 11, 1)).unwrap();
    assert_eq!(november.received_this_month, rupees(0));
}

#[test]
fn milestones_are_ready_once_due_and_still_pending() {
    let store = demo::store().unwrap();
    let service = DashboardService::new(store.repository());

    // Launch is due 15 November, Beta 15 December
    assert!(service.now(date(2026, 11, 1)).unwrap().ready.is_empty());

    let ready = service.now(date(2026, 12, 20)).unwrap().ready;
    let names: Vec<_> = ready.iter().map(|m| m.milestone_name.as_str()).collect();
    assert_eq!(names, ["Launch", "Beta"]);
    assert_eq!(ready[0].project_name, "Website Revamp");
    assert_eq!(ready[0].amount, rupees(40_000));
    assert_eq!(ready[0].currency, Currency::Inr);
    // Beta's 2 hours logged at ₹1,500 an hour
    assert_eq!(ready[1].amount, rupees(3_000));
}

#[test]
fn an_invoice_issued_after_today_has_waited_no_days() {
    let store = demo::store().unwrap();
    let dashboard = DashboardService::new(store.repository())
        .now(date(2026, 9, 1))
        .unwrap();
    assert!(
        dashboard
            .unpaid
            .iter()
            .all(|s| s.days_waiting(date(2026, 9, 1)) == 0)
    );
}

#[test]
fn unpaid_alone_matches_the_full_dashboard() {
    let store = demo::store().unwrap();
    let service = DashboardService::new(store.repository());
    let today = date(2026, 11, 1);
    assert_eq!(
        service.unpaid().unwrap(),
        service.now(today).unwrap().unpaid
    );
}

#[test]
fn a_refund_comes_off_what_was_received_that_month() {
    let store = demo::store().unwrap();
    let mut invoices = InvoiceService::new(store.repository(), None);
    // Design, ₹50,000 + GST, paid in full: crediting ₹10,000 of it refunds ₹10,800 in cash and
    // reverses ₹1,000 of TDS
    let number = InvoiceNumber::from_stored("NDC/26-27/002");
    let draft = CreditDraft {
        date: date(2027, 1, 15),
        lines: vec![CreditLineDraft {
            milestone_id: invoices.load(&number).unwrap().lines()[0].milestone_id,
            taxable_value: rupees(10_000),
        }],
        reason: "Scope cut".to_owned(),
        returned: ReturnTerms {
            kind: ReturnKind::Refunded,
            refunded_inr: None,
            tds_reversed: Some(rupees(1_000)),
        },
    };
    invoices.issue_credit(&number, draft).unwrap();

    let january = DashboardService::new(store.repository())
        .now(date(2027, 1, 31))
        .unwrap();
    assert_eq!(january.received_this_month, rupees(-10_800));
}
