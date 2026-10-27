//! Turning what a person typed into domain values, the same way in every front end. The only
//! place input text is parsed.

use std::str::FromStr;

use bigdecimal::{BigDecimal, RoundingMode, ToPrimitive};
use chrono::NaiveDate;

use crate::domain::{Discount, FiscalYear};

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

/// `2026-27`, `FY 2026-27` or `2026`; blank means `default`
pub fn fiscal_year_or(input: &str, default: FiscalYear) -> ParseResult<FiscalYear> {
    const INVALID: &str = "financial year must look like 2026-27";
    let input = input.trim();
    let input = input
        .strip_prefix("FY")
        .or_else(|| input.strip_prefix("fy"))
        .unwrap_or(input)
        .trim();
    if input.is_empty() {
        return Ok(default);
    }
    let (start, end) = match input.split_once('-') {
        Some((start, end)) => (start, Some(end)),
        None => (input, None),
    };
    let start: i32 = start.parse().map_err(|_| INVALID)?;
    if !(2000..=2999).contains(&start) {
        return Err(INVALID);
    }
    if let Some(end) = end {
        let end: i32 = end.parse().map_err(|_| INVALID)?;
        let next = start + 1;
        if end != next % 100 && end != next {
            return Err("a financial year runs April to March, like 2026-27");
        }
    }
    Ok(FiscalYear::starting(start))
}

/// Trimmed text, or `None` when blank
pub fn optional(input: &str) -> Option<String> {
    let input = input.trim();
    (!input.is_empty()).then(|| input.to_owned())
}

/// `10%` off, or an amount like `5,000`; blank means no discount
pub fn discount(input: &str) -> ParseResult<Option<Discount>> {
    let input = input.trim();
    if input.is_empty() {
        return Ok(None);
    }
    match input.strip_suffix('%') {
        Some(percent) => BigDecimal::from_str(percent.trim())
            .map(|p| Some(Discount::Percent(p)))
            .map_err(|_| "discount must be an amount or a percent, like 5,000 or 10%"),
        None => money(input).map(|amount| Some(Discount::Amount(amount))),
    }
}
