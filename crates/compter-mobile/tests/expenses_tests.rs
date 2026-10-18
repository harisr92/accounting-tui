mod support;

use compter_mobile::api::expenses::expense_categories;
use compter_mobile::api::{CompterApp, ExpenseInput};
use support::demo;

fn expense(date: &str, amount: &str, gst_credit: &str) -> ExpenseInput {
    ExpenseInput {
        date: date.to_owned(),
        category: "Software & subscriptions".to_owned(),
        description: "GitHub, annual plan".to_owned(),
        amount: amount.to_owned(),
        gst_credit: gst_credit.to_owned(),
    }
}

#[test]
fn a_year_lists_its_expenses_with_their_credit() {
    let (_scratch, app) = demo();

    let year = app.expenses(2026).unwrap();

    assert_eq!(year.year, "FY 2026-27");
    let laptop = year
        .expenses
        .iter()
        .find(|e| e.description == "Laptop")
        .unwrap();
    assert_eq!(laptop.category, "Equipment");
    assert_eq!(laptop.amount, "₹1,77,000.00");
    assert_eq!(laptop.gst_credit.as_deref(), Some("₹27,000.00"));
    assert_eq!(laptop.edit.amount, "177000.00");
    assert!(app.expenses(2020).unwrap().expenses.is_empty());
}

#[test]
fn expenses_are_added_edited_and_deleted() {
    let (_scratch, mut app) = demo();
    let before = app.expenses(2026).unwrap().expenses.len();

    let added = app
        .add_expense(expense("2026-10-02", "21,240", "3,240"))
        .unwrap();
    assert_eq!(added.amount, "₹21,240.00");
    assert_eq!(added.gst_credit.as_deref(), Some("₹3,240.00"));
    assert_eq!(app.expenses(2026).unwrap().expenses.len(), before + 1);

    let edited = app
        .edit_expense(added.id.clone(), expense("2026-10-02", "10,000", ""))
        .unwrap();
    assert_eq!(edited.id, added.id);
    assert_eq!(edited.amount, "₹10,000.00");
    assert_eq!(edited.gst_credit, None);

    app.delete_expense(added.id).unwrap();
    assert_eq!(app.expenses(2026).unwrap().expenses.len(), before);
}

#[test]
fn refused_expenses_name_their_field() {
    let (_scratch, mut app) = demo();
    let field =
        |input: ExpenseInput, app: &mut CompterApp| app.add_expense(input).unwrap_err().field;

    let no_category = ExpenseInput {
        category: "Snacks".to_owned(),
        ..expense("", "100", "")
    };
    assert_eq!(field(no_category, &mut app).as_deref(), Some("category"));
    assert_eq!(
        field(expense("", "lots", ""), &mut app).as_deref(),
        Some("amount")
    );
    assert_eq!(
        field(expense("", "100", "x"), &mut app).as_deref(),
        Some("gst_credit")
    );
    assert_eq!(
        field(expense("2/10/26", "100", ""), &mut app).as_deref(),
        Some("date")
    );
    assert!(expense_categories().contains(&"Software & subscriptions".to_owned()));
}

#[test]
fn gst_credit_needs_a_gstin_in_the_profile() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = CompterApp::open(dir.path().to_string_lossy().into_owned()).unwrap();

    let error = app
        .add_expense(expense("2026-10-02", "1,180", "180"))
        .unwrap_err();

    assert_eq!(error.field, None);
    assert!(error.message.contains("GSTIN"), "{}", error.message);
}
