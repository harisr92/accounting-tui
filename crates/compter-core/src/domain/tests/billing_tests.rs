use bigdecimal::{BigDecimal, Zero};
use chrono::NaiveDate;

use crate::domain::billing::amount_for;
use crate::domain::{Billing, DomainError, TimeEntry, format_duration};

#[test]
fn hourly_amounts_round_to_paise() {
    assert_eq!(
        amount_for(&BigDecimal::from(1500), 90),
        BigDecimal::from(2250)
    );
    let expected: BigDecimal = "116.67".parse().unwrap();
    assert_eq!(amount_for(&BigDecimal::from(1000), 7), expected);
}

#[test]
fn rejects_bad_rate_and_duration() {
    assert!(matches!(
        Billing::hourly(BigDecimal::zero()),
        Err(DomainError::NonPositiveRate)
    ));
    let date = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
    assert!(matches!(
        TimeEntry::new(date, 0, ""),
        Err(DomainError::ZeroDuration)
    ));
}

#[test]
fn loading_validates_billing_and_time_entries() {
    let negative_rate = r#"{"type":"hourly","rate":"-5"}"#;
    assert!(serde_json::from_str::<Billing>(negative_rate).is_err());
    let rate = r#"{"type":"hourly","rate":"1500"}"#;
    assert!(serde_json::from_str::<Billing>(rate).is_ok());

    let zero_minutes = r#"{"date":"2026-10-01","minutes":0,"note":""}"#;
    assert!(serde_json::from_str::<TimeEntry>(zero_minutes).is_err());
}

#[test]
fn time_entry_is_at_most_one_day() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
    assert!(TimeEntry::new(date, 24 * 60, "").is_ok());
    assert!(matches!(
        TimeEntry::new(date, 90 * 60, "typed 90 meaning minutes"),
        Err(DomainError::EntryTooLong)
    ));
    let stored = r#"{"date":"2026-10-01","minutes":5400,"note":""}"#;
    assert!(serde_json::from_str::<TimeEntry>(stored).is_err());
}

#[test]
fn durations_show_hours_and_padded_minutes() {
    assert_eq!(format_duration(0), "0h 00m");
    assert_eq!(format_duration(390), "6h 30m");
    assert_eq!(format_duration(65), "1h 05m");
}
