//! Turning what the user typed into domain values. The only place input text is parsed.

use std::str::FromStr;

use bigdecimal::{BigDecimal, RoundingMode, ToPrimitive};
use chrono::NaiveDate;

pub type ParseResult<T> = Result<T, &'static str>;

/// `150000`, `1,50,000.50`, `₹1,50,000` or `$1,500.50`
pub fn money(input: &str) -> ParseResult<BigDecimal> {
    let cleaned: String = input
        .chars()
        .filter(|c| !matches!(c, ',' | '₹' | '$' | '£' | '€' | ' '))
        .collect();
    if cleaned.is_empty() {
        return Err("enter an amount, like 1,50,000.50");
    }
    BigDecimal::from_str(&cleaned).map_err(|_| "amount must be a number, like 1,50,000.50")
}

/// `YYYY-MM-DD`; blank means `default`
pub fn date_or(input: &str, default: NaiveDate) -> ParseResult<NaiveDate> {
    let input = input.trim();
    if input.is_empty() {
        return Ok(default);
    }
    NaiveDate::parse_from_str(input, "%Y-%m-%d").map_err(|_| "date must look like 2026-10-31")
}

/// Whole minutes from `1h30m`, `2h`, `45m`, `1:30`, or a plain number of hours (`1.5`)
pub fn duration_minutes(input: &str) -> ParseResult<u32> {
    const INVALID: &str = "time must look like 1h30m, 1.5, 45m or 1:30";
    let input: String = input.trim().to_lowercase().split_whitespace().collect();
    if input.is_empty() {
        return Err("enter the time worked, like 1h30m");
    }

    let (hours, minutes) = if let Some((h, m)) = input.split_once(':') {
        (h, m)
    } else if let Some((h, rest)) = input.split_once('h') {
        (h, rest.strip_suffix('m').unwrap_or(rest))
    } else if let Some(m) = input.strip_suffix('m') {
        ("", m)
    } else {
        (input.as_str(), "")
    };

    let hours = if hours.is_empty() {
        BigDecimal::from(0)
    } else {
        BigDecimal::from_str(hours).map_err(|_| INVALID)?
    };
    let minutes: u32 = if minutes.is_empty() {
        0
    } else {
        minutes.parse().map_err(|_| INVALID)?
    };
    let from_hours = (hours * BigDecimal::from(60))
        .with_scale_round(0, RoundingMode::HalfUp)
        .to_u32()
        .ok_or(INVALID)?;
    from_hours.checked_add(minutes).ok_or(INVALID)
}

/// Trimmed text, or `None` when blank
pub fn optional(input: &str) -> Option<String> {
    let input = input.trim();
    (!input.is_empty()).then(|| input.to_owned())
}
