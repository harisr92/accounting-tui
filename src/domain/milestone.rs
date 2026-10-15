use std::fmt;

use bigdecimal::{BigDecimal, Zero};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use super::error::require;
use super::{DomainError, TimeEntry};

/// Billing state of a milestone
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MilestoneStatus {
    Pending,
    Invoiced,
    Paid,
}

impl MilestoneStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::Invoiced => "INVOICED",
            Self::Paid => "PAID",
        }
    }
}

impl fmt::Display for MilestoneStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// A billable unit of work inside a project. Changed only through [`super::Project`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "RawMilestone")]
pub struct Milestone {
    name: String,
    amount: BigDecimal,
    due_date: NaiveDate,
    status: MilestoneStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    time_entries: Vec<TimeEntry>,
}

/// Unvalidated shape of a [`Milestone`]; project-level rules are checked by [`super::Project`]
#[derive(Deserialize)]
struct RawMilestone {
    name: String,
    amount: BigDecimal,
    due_date: NaiveDate,
    status: MilestoneStatus,
    #[serde(default)]
    time_entries: Vec<TimeEntry>,
}

impl TryFrom<RawMilestone> for Milestone {
    type Error = DomainError;

    fn try_from(raw: RawMilestone) -> Result<Self, Self::Error> {
        let milestone = Self {
            name: raw.name,
            amount: raw.amount,
            due_date: raw.due_date,
            status: raw.status,
            time_entries: raw.time_entries,
        };
        milestone.validate()?;
        Ok(milestone)
    }
}

impl Milestone {
    pub(super) fn new(name: String, due_date: NaiveDate, amount: BigDecimal) -> Self {
        Self {
            name,
            amount,
            due_date,
            status: MilestoneStatus::Pending,
            time_entries: Vec::new(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn amount(&self) -> &BigDecimal {
        &self.amount
    }

    pub fn due_date(&self) -> NaiveDate {
        self.due_date
    }

    pub fn status(&self) -> MilestoneStatus {
        self.status
    }

    pub fn time_entries(&self) -> &[TimeEntry] {
        &self.time_entries
    }

    pub fn logged_minutes(&self) -> u32 {
        self.time_entries
            .iter()
            .fold(0, |total, entry| total.saturating_add(entry.minutes()))
    }

    pub(super) fn ensure_pending(&self) -> Result<(), DomainError> {
        if self.status == MilestoneStatus::Pending {
            Ok(())
        } else {
            Err(DomainError::MilestoneLocked)
        }
    }

    pub(super) fn log(&mut self, entry: TimeEntry) -> Result<(), DomainError> {
        self.ensure_pending()?;
        self.time_entries.push(entry);
        Ok(())
    }

    /// Replace the details of a pending milestone
    pub(super) fn edit(
        &mut self,
        name: String,
        due_date: NaiveDate,
        amount: BigDecimal,
    ) -> Result<(), DomainError> {
        self.ensure_pending()?;
        self.name = name;
        self.due_date = due_date;
        self.amount = amount;
        Ok(())
    }

    /// Take back the most recent time entry of a pending milestone
    pub(super) fn undo_last_entry(&mut self) -> Result<TimeEntry, DomainError> {
        self.ensure_pending()?;
        self.time_entries.pop().ok_or(DomainError::NoTimeEntries)
    }

    pub(super) fn set_amount(&mut self, amount: BigDecimal) {
        self.amount = amount;
    }

    /// Move one step along Pending → Invoiced → Paid
    pub(super) fn advance(&mut self, to: MilestoneStatus) -> Result<(), DomainError> {
        use MilestoneStatus::{Invoiced, Paid, Pending};

        match (self.status, to) {
            (Pending, Invoiced) if self.amount <= BigDecimal::zero() => {
                Err(DomainError::NothingToInvoice)
            }
            (Pending, Invoiced) | (Invoiced, Paid) => {
                self.status = to;
                Ok(())
            }
            (from, to) => Err(DomainError::InvalidTransition { from, to }),
        }
    }

    pub(super) fn validate(&self) -> Result<(), DomainError> {
        require(&self.name, "milestone name")?;
        if self.amount < BigDecimal::zero() {
            return Err(DomainError::NonPositiveAmount);
        }
        self.time_entries.iter().try_for_each(TimeEntry::validate)
    }
}
