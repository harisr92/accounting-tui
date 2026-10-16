use std::fmt;

use accounting_core::utils::formatting::{amount_in_words, format_inr};
use bigdecimal::{BigDecimal, RoundingMode, Signed, Zero};
use serde::{Deserialize, Serialize};

use super::DomainError;

/// Decimal places kept on an exchange rate, as RBI reference rates are quoted
const RATE_SCALE: i64 = 4;

/// What a client is billed in. GST, the ledger and returns are always in rupees; only clients
/// abroad can be billed in a foreign currency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Currency {
    #[default]
    Inr,
    Usd,
    Gbp,
    Eur,
}

impl Currency {
    pub const ALL: [Self; 4] = [Self::Inr, Self::Usd, Self::Gbp, Self::Eur];
    /// Form options, in the order of [`Self::ALL`]
    pub const LABELS: &'static [&'static str] = &["INR ₹", "USD $", "GBP £", "EUR €"];

    pub const fn code(self) -> &'static str {
        match self {
            Self::Inr => "INR",
            Self::Usd => "USD",
            Self::Gbp => "GBP",
            Self::Eur => "EUR",
        }
    }

    pub const fn symbol(self) -> &'static str {
        match self {
            Self::Inr => "₹",
            Self::Usd => "$",
            Self::Gbp => "£",
            Self::Eur => "€",
        }
    }

    /// Printed before PDF totals. The built-in PDF font has no `₹`, so rupees are `Rs.`
    pub const fn pdf_label(self) -> &'static str {
        match self {
            Self::Inr => "Rs.",
            other => other.code(),
        }
    }

    pub const fn is_inr(self) -> bool {
        matches!(self, Self::Inr)
    }

    /// Position in [`Self::ALL`] and [`Self::LABELS`]
    pub fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|&c| c == self)
            .unwrap_or_default()
    }

    /// The currency at `index` of [`Self::ALL`]; rupees when out of range
    pub fn from_index(index: usize) -> Self {
        Self::ALL.get(index).copied().unwrap_or_default()
    }

    /// Digits without a symbol: Indian grouping for rupees (`1,50,000.00`), else `150,000.00`
    pub fn format(self, amount: &BigDecimal) -> String {
        if self.is_inr() {
            return format_inr(amount);
        }
        let (whole, cents) = split(amount);
        let sign = if amount.is_negative() && (whole != "0" || cents != 0) {
            "-"
        } else {
            ""
        };
        format!("{sign}{}.{cents:02}", group_thousands(&whole))
    }

    /// The amount spelled out the way an invoice prints it
    pub fn in_words(self, amount: &BigDecimal) -> String {
        let (major, minor) = match self {
            Self::Inr => return amount_in_words(amount),
            Self::Usd => ("US Dollars", "Cents"),
            Self::Gbp => ("Pounds Sterling", "Pence"),
            Self::Eur => ("Euros", "Cents"),
        };
        let (whole, cents) = split(amount);
        let words = match (whole.as_str(), cents) {
            ("0", 0) => format!("{major} Zero"),
            ("0", cents) => format!("{major} Zero and {} {minor}", integer_words(cents.into())),
            (whole, 0) => format!("{major} {}", integer_words(parse_whole(whole))),
            (whole, cents) => format!(
                "{major} {} and {} {minor}",
                integer_words(parse_whole(whole)),
                integer_words(cents.into())
            ),
        };
        let sign = if amount.is_negative() && (whole != "0" || cents != 0) {
            "Minus "
        } else {
            ""
        };
        format!("{sign}{words} Only")
    }
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// Rupees for one unit of a foreign currency, fixed on the invoice date
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "BigDecimal", into = "BigDecimal")]
pub struct ExchangeRate(BigDecimal);

impl ExchangeRate {
    /// Rounded to four decimal places; must be positive
    pub fn new(rate: BigDecimal) -> Result<Self, DomainError> {
        let rate = rate.with_scale_round(RATE_SCALE, RoundingMode::HalfUp);
        if rate <= BigDecimal::zero() {
            return Err(DomainError::InvalidExchangeRate);
        }
        Ok(Self(rate))
    }

    pub fn value(&self) -> &BigDecimal {
        &self.0
    }

    /// `amount` of the foreign currency in rupees, rounded to the paisa
    pub fn to_inr(&self, amount: &BigDecimal) -> BigDecimal {
        (amount * &self.0).with_scale_round(2, RoundingMode::HalfUp)
    }
}

impl TryFrom<BigDecimal> for ExchangeRate {
    type Error = DomainError;

    fn try_from(rate: BigDecimal) -> Result<Self, Self::Error> {
        Self::new(rate)
    }
}

impl From<ExchangeRate> for BigDecimal {
    fn from(rate: ExchangeRate) -> Self {
        rate.0
    }
}

impl fmt::Display for ExchangeRate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// The whole units (no sign) and the two decimal digits of `amount`, rounded half up
fn split(amount: &BigDecimal) -> (String, u8) {
    let rounded = amount.abs().with_scale_round(2, RoundingMode::HalfUp);
    let text = rounded.to_string();
    match text.split_once('.') {
        Some((whole, cents)) => (whole.to_owned(), cents.parse().unwrap_or_default()),
        None => (text, 0),
    }
}

fn parse_whole(whole: &str) -> u128 {
    whole.parse().unwrap_or_default()
}

/// `1234567` → `1,234,567`
fn group_thousands(digits: &str) -> String {
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    grouped
}

const ONES: [&str; 20] = [
    "Zero",
    "One",
    "Two",
    "Three",
    "Four",
    "Five",
    "Six",
    "Seven",
    "Eight",
    "Nine",
    "Ten",
    "Eleven",
    "Twelve",
    "Thirteen",
    "Fourteen",
    "Fifteen",
    "Sixteen",
    "Seventeen",
    "Eighteen",
    "Nineteen",
];
const TENS: [&str; 10] = [
    "", "", "Twenty", "Thirty", "Forty", "Fifty", "Sixty", "Seventy", "Eighty", "Ninety",
];
const SCALES: [(u128, &str); 4] = [
    (1_000_000_000_000, "Trillion"),
    (1_000_000_000, "Billion"),
    (1_000_000, "Million"),
    (1_000, "Thousand"),
];

/// A whole number in Western (million, billion) words
fn integer_words(n: u128) -> String {
    if n == 0 {
        return ONES[0].to_owned();
    }
    let mut parts = Vec::new();
    let mut rest = n;
    for (scale, name) in SCALES {
        if rest >= scale {
            parts.push(format!("{} {name}", integer_words(rest / scale)));
            rest %= scale;
        }
    }
    if rest > 0 {
        parts.push(below_thousand(rest));
    }
    parts.join(" ")
}

fn below_thousand(n: u128) -> String {
    let mut parts = Vec::new();
    if n >= 100 {
        parts.push(format!("{} Hundred", ONES[(n / 100) as usize]));
    }
    let rest = (n % 100) as usize;
    match rest {
        0 => {}
        1..=19 => parts.push(ONES[rest].to_owned()),
        _ if rest.is_multiple_of(10) => parts.push(TENS[rest / 10].to_owned()),
        _ => parts.push(format!("{} {}", TENS[rest / 10], ONES[rest % 10])),
    }
    parts.join(" ")
}
