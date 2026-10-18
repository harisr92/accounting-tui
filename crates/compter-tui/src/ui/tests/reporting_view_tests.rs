use crate::ui::key::Key;

use crate::ui::tests::support::{go_to_year, reporting_app, screen, type_keys};
use compter_core::demo;
use compter_core::domain::FiscalYear;
use compter_core::storage::{ExpenseRepository, Store};

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
    assert!(screen.contains("Invoiced (net)"), "{screen}");
    assert!(screen.contains("Credit notes"), "{screen}");
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
        "₹89,200.00",
        "Exchange gain",
        "₹450.00",
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
    app.handle_key(Key::Enter);
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

#[test]
fn expenses_are_edited_in_a_prefilled_form_and_searched() {
    let store = Store::in_memory().unwrap();
    let mut app = reporting_app(&store);
    app.handle_key(Key::Char('2'));
    go_to_year(&mut app, 2026);
    for what in ["Train to Mumbai\t2,500", "Figma\t1,000"] {
        app.handle_key(Key::Char('a'));
        type_keys(&mut app, &format!("2026-07-05\t\t{what}\n"));
    }

    app.handle_key(Key::Char('/'));
    type_keys(&mut app, "train\n");
    let found = screen(&app, W, H);
    assert!(found.contains("Train to Mumbai"), "{found}");
    assert!(!found.contains("Figma"), "{found}");

    // The form holds the stored values; change the amount only
    app.handle_key(Key::Char('e'));
    let form = screen(&app, W, H);
    assert!(form.contains("Edit expense"), "{form}");
    assert!(form.contains("2026-07-05"), "{form}");
    type_keys(&mut app, "\t\t\t");
    app.handle_key(Key::Ctrl('u'));
    type_keys(&mut app, "3000\n");

    let listed = store.repository().list_expenses(fy_2026()).unwrap();
    assert_eq!(listed.len(), 2);
    let train = listed
        .iter()
        .find(|e| e.description() == "Train to Mumbai")
        .unwrap();
    assert_eq!(train.amount(), &bigdecimal::BigDecimal::from(3000));
    assert!(screen(&app, W, H).contains("Saved Train to Mumbai"));

    app.handle_key(Key::Esc);
    assert!(screen(&app, W, H).contains("Figma"));
}

#[test]
fn dashboard_tiles_wrap_on_a_narrow_screen() {
    let store = demo::store().unwrap();
    let app = reporting_app(&store);
    let narrow = screen(&app, 80, 24);
    for label in ["Invoiced (net)", "Credit notes", "Outstanding", "Profit"] {
        assert!(narrow.contains(label), "missing {label}\n{narrow}");
    }
}
