//! Pure domain types and rules. No I/O, no UI.

mod billing;
mod error;
mod fiscal_year;
mod milestone;
mod project;

pub use billing::{Billing, MAX_ENTRY_MINUTES, TimeEntry};
pub use error::DomainError;
pub use fiscal_year::FiscalYear;
pub use milestone::{Milestone, MilestoneStatus};
pub use project::{Project, ProjectId, ProjectStatus, ProjectSummary};

#[cfg(test)]
mod tests;
