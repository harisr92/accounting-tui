use bigdecimal::BigDecimal;
use chrono::NaiveDate;

use crate::ui::form::parse::{date_or, duration_minutes, money, optional};

#[test]
fn money_accepts_indian_grouping_and_rupee_sign() {
    let expected: BigDecimal = "150000.50".parse().unwrap();
    assert_eq!(money("1,50,000.50").unwrap(), expected);
    assert_eq!(money(" ₹1,50,000.50 ").unwrap(), expected);
    assert_eq!(money("1500").unwrap(), BigDecimal::from(1500));
    assert_eq!(money("$150,000.50").unwrap(), expected);
    assert_eq!(money("€1,500").unwrap(), BigDecimal::from(1500));
    assert_eq!(money("£ 1500").unwrap(), BigDecimal::from(1500));
    assert!(money("").is_err());
    assert!(money("12abc").is_err());
}

#[test]
fn blank_date_means_default() {
    let today = NaiveDate::from_ymd_opt(2026, 10, 15).unwrap();
    assert_eq!(date_or("  ", today).unwrap(), today);
    assert_eq!(
        date_or("2026-12-01", today).unwrap(),
        NaiveDate::from_ymd_opt(2026, 12, 1).unwrap()
    );
    assert!(date_or("01/12/2026", today).is_err());
}

#[test]
fn durations_in_every_supported_form() {
    for (input, minutes) in [
        ("1h30m", 90),
        ("1H 30M", 90),
        ("2h", 120),
        ("45m", 45),
        ("1:30", 90),
        ("1.5", 90),
        ("0.25", 15),
        ("1.5h", 90),
        ("90", 5400),
    ] {
        assert_eq!(duration_minutes(input), Ok(minutes), "{input}");
    }
    for input in ["", "abc", "1h3x", "-2", "1:xx"] {
        assert!(duration_minutes(input).is_err(), "{input}");
    }
}

#[test]
fn optional_text_is_trimmed() {
    assert_eq!(
        optional("  29AAPFU0939F1ZR "),
        Some("29AAPFU0939F1ZR".to_owned())
    );
    assert_eq!(optional("   "), None);
}
