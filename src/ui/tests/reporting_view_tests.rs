use termion::event::Key;

use crate::demo;
use crate::domain::FiscalYear;
use crate::storage::{ExpenseRepository, Store};
use crate::ui::tests::support::{go_to_year, reporting_app, screen, type_keys};

const W: u16 = 140;
const H: u16 = 34;

fn fy_2026() -> FiscalYear {
    FiscalYear::containing(chrono::NaiveDate::from_ymd_opt(2026, 4, 1).unwrap())
}

#[test]
fn dashboard_lists_what_clients_still_owe() {
    let store = demo::store().unwrap();
    let app = reporting_app(&store);
    let screen = screen(&app, W, H);
    assert!(screen.contains("Waiting for payment"), "{screen}");
    for unpaid in ["NDC/26-27/003", "NDC/26-27/004", "NDC/26-27/005"] {
        assert!(screen.contains(unpaid), "missing {unpaid}");
    }
    assert!(!screen.contains("NDC/26-27/002"), "paid invoice listed");
}

#[test]
fn reports_show_the_year_for_tax_and_gst() {
    let store = demo::store().unwrap();
    let mut app = reporting_app(&store);
    app.handle_key(Key::Char('3'));
    go_to_year(&mut app, 2026);
    let screen = screen(&app, W, H);
    for expected in [
        "Income 2026-27",
        "₹2,50,750.00",
        "44ADA income (50%)",
        "₹1,25,375.00",
        "₹88,750.00",
        "Q2 (Jul-Sep)",
        "₹28,980.00",
        "Acme Pvt Ltd",
        "Form 26AS",
    ] {
        assert!(screen.contains(expected), "missing {expected:?}\n{screen}");
    }
    assert!(
        screen.contains("tax audit (44AB)"),
        "books profit is below 44ADA income"
    );
}

#[test]
fn gstr1_export_needs_a_data_folder() {
    let store = demo::store().unwrap();
    let mut app = reporting_app(&store);
    app.handle_key(Key::Char('3'));
    go_to_year(&mut app, 2026);
    type_keys(&mut app, "g\n");
    let screen = screen(&app, W, H);
    assert!(
        screen.contains("returns are not saved in demo mode"),
        "{screen}"
    );
}

#[test]
fn expenses_are_added_and_deleted() {
    let store = Store::in_memory().unwrap();
    let mut app = reporting_app(&store);
    app.handle_key(Key::Char('2'));
    assert!(screen(&app, W, H).contains("No expenses this year"));

    app.handle_key(Key::Char('a'));
    type_keys(&mut app, "2026-07-05\t \tTrain to Mumbai\t2,500");
    app.handle_key(Key::Char('\n'));
    go_to_year(&mut app, 2026);
    let listed = store.repository().list_expenses(fy_2026()).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].category().label(), "Equipment");
    assert!(screen(&app, W, H).contains("Train to Mumbai"));

    type_keys(&mut app, "xy");
    assert!(
        store
            .repository()
            .list_expenses(fy_2026())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn credit_without_gstin_keeps_the_form_open() {
    let store = Store::in_memory().unwrap();
    let mut app = reporting_app(&store);
    app.handle_key(Key::Char('2'));
    app.handle_key(Key::Char('a'));
    type_keys(&mut app, "\t\tFigma\t1180\t180\n");
    let screen = screen(&app, W, H);
    assert!(
        screen.contains("GST input credit can only be claimed"),
        "{screen}"
    );
    assert!(screen.contains("Add expense"));
}
