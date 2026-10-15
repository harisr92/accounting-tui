use std::fmt;

use bigdecimal::BigDecimal;
use chrono::NaiveDate;

/// Billing state of a milestone
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// A billable unit of work inside a project
#[derive(Debug, Clone, PartialEq)]
pub struct Milestone {
    pub name: String,
    pub amount: BigDecimal,
    pub due_date: NaiveDate,
    pub status: MilestoneStatus,
}
