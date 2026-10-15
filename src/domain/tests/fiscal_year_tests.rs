use chrono::NaiveDate;

use crate::domain::{FiscalYear, Quarter};

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

#[test]
fn quarters_follow_april_to_march() {
    let d = |y, m| NaiveDate::from_ymd_opt(y, m, 15).unwrap();
    let q = Quarter::containing(d(2027, 2));
    assert_eq!((q.year().label(), q.number()), ("2026-27".to_owned(), 4));
    assert_eq!(q.months(), [(2027, 1), (2027, 2), (2027, 3)]);
    assert_eq!(
        Quarter::containing(d(2026, 7)).months(),
        [(2026, 7), (2026, 8), (2026, 9)]
    );
    assert!(Quarter::containing(d(2026, 10)).contains(d(2026, 12)));

    let year = FiscalYear::containing(d(2026, 5));
    assert_eq!(
        year.first_day(),
        NaiveDate::from_ymd_opt(2026, 4, 1).unwrap()
    );
    assert_eq!(
        year.last_day(),
        NaiveDate::from_ymd_opt(2027, 3, 31).unwrap()
    );
    assert!(year.contains(d(2027, 3)) && !year.contains(d(2027, 4)));
    assert_eq!(year.quarters()[2].label(), "Q3 (Oct-Dec)");
    assert_eq!(year.next().previous(), year);
}
