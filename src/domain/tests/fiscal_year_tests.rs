use chrono::NaiveDate;

use crate::domain::FiscalYear;

#[test]
fn april_starts_a_new_year() {
    let d = |m, day| NaiveDate::from_ymd_opt(2026, m, day).unwrap();
    assert_eq!(FiscalYear::containing(d(3, 31)).to_string(), "FY 2025-26");
    assert_eq!(FiscalYear::containing(d(4, 1)).to_string(), "FY 2026-27");
}

#[test]
fn short_and_long_labels() {
    let year = FiscalYear::containing(NaiveDate::from_ymd_opt(2026, 10, 15).unwrap());
    assert_eq!(year.label(), "2026-27");
    assert_eq!(year.short(), "26-27");
    let y2k = FiscalYear::containing(NaiveDate::from_ymd_opt(2100, 1, 1).unwrap());
    assert_eq!(y2k.short(), "99-00");
}
