use bigdecimal::{BigDecimal, Zero};
use chrono::NaiveDate;

use crate::domain::billing::amount_for;
use crate::domain::{Billing, DomainError, TimeEntry};

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
