use bigdecimal::{BigDecimal, RoundingMode, Zero};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use super::DomainError;

const MINUTES_PER_HOUR: u32 = 60;
/// One entry records at most one day of work
pub const MAX_ENTRY_MINUTES: u32 = 24 * MINUTES_PER_HOUR;

/// How a project charges its client
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", try_from = "RawBilling")]
pub enum Billing {
    /// Each milestone carries an agreed amount
    Fixed,
    /// Milestone amounts come from logged time at this rate per hour
    Hourly { rate: BigDecimal },
}

/// Unvalidated shape of a [`Billing`]
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum RawBilling {
    Fixed,
    Hourly { rate: BigDecimal },
}

impl TryFrom<RawBilling> for Billing {
    type Error = DomainError;

    fn try_from(raw: RawBilling) -> Result<Self, Self::Error> {
        let billing = match raw {
            RawBilling::Fixed => Self::Fixed,
            RawBilling::Hourly { rate } => Self::Hourly { rate },
        };
        billing.validate()?;
        Ok(billing)
    }
}

impl Billing {
    pub fn hourly(rate: BigDecimal) -> Result<Self, DomainError> {
        let billing = Self::Hourly { rate };
        billing.validate()?;
        Ok(billing)
    }

    pub fn rate(&self) -> Option<&BigDecimal> {
        match self {
            Self::Fixed => None,
            Self::Hourly { rate } => Some(rate),
        }
    }

    pub(super) fn validate(&self) -> Result<(), DomainError> {
        match self.rate() {
            Some(rate) if *rate <= BigDecimal::zero() => Err(DomainError::NonPositiveRate),
            _ => Ok(()),
        }
    }
}

/// Amount for `minutes` of work at `rate` per hour, rounded to paise
pub fn amount_for(rate: &BigDecimal, minutes: u32) -> BigDecimal {
    (rate * BigDecimal::from(minutes) / BigDecimal::from(MINUTES_PER_HOUR))
        .with_scale_round(2, RoundingMode::HalfUp)
}

/// Time worked on an hourly milestone
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawTimeEntry")]
pub struct TimeEntry {
    date: NaiveDate,
    minutes: u32,
    note: String,
}

/// Unvalidated shape of a [`TimeEntry`]
#[derive(Deserialize)]
struct RawTimeEntry {
    date: NaiveDate,
    minutes: u32,
    note: String,
}

impl TryFrom<RawTimeEntry> for TimeEntry {
    type Error = DomainError;

    fn try_from(raw: RawTimeEntry) -> Result<Self, Self::Error> {
        Self::new(raw.date, raw.minutes, raw.note)
    }
}

impl TimeEntry {
    pub fn new(
        date: NaiveDate,
        minutes: u32,
        note: impl Into<String>,
    ) -> Result<Self, DomainError> {
        let entry = Self {
            date,
            minutes,
            note: note.into(),
        };
        entry.validate()?;
        Ok(entry)
    }

    pub fn date(&self) -> NaiveDate {
        self.date
    }

    pub fn minutes(&self) -> u32 {
        self.minutes
    }

    pub fn note(&self) -> &str {
        &self.note
    }

    pub(super) fn validate(&self) -> Result<(), DomainError> {
        match self.minutes {
            0 => Err(DomainError::ZeroDuration),
            m if m > MAX_ENTRY_MINUTES => Err(DomainError::EntryTooLong),
            _ => Ok(()),
        }
    }
}
