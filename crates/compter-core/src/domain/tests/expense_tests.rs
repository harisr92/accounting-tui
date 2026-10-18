use bigdecimal::BigDecimal;
use chrono::NaiveDate;

use crate::domain::{DomainError, Expense, ExpenseCategory};

fn date() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, 1).unwrap()
}

fn expense(amount: i64, credit: i64) -> Result<Expense, DomainError> {
    Expense::new(
        date(),
        ExpenseCategory::Software,
        " Figma ",
        BigDecimal::from(amount),
        BigDecimal::from(credit),
    )
}

#[test]
fn cost_excludes_claimed_gst() {
    let e = expense(1180, 180).unwrap();
    assert_eq!(e.description(), "Figma");
    assert_eq!(e.cost(), BigDecimal::from(1000));
}

#[test]
fn rejects_bad_amounts() {
    assert!(matches!(expense(0, 0), Err(DomainError::NonPositiveAmount)));
    assert!(matches!(expense(100, -1), Err(DomainError::NegativeAmount)));
    assert!(matches!(
        expense(100, 100),
        Err(DomainError::GstCreditTooLarge)
    ));
}

#[test]
fn stored_expenses_are_validated() {
    let e = expense(1180, 180).unwrap();
    let mut json = serde_json::to_value(&e).unwrap();
    assert_eq!(serde_json::from_value::<Expense>(json.clone()).unwrap(), e);
    json["gst_credit"] = "5000".into();
    assert!(serde_json::from_value::<Expense>(json).is_err());
}
