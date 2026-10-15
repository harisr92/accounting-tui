//! Pure domain types and rules. No I/O, no UI.

mod fiscal_year;
mod milestone;
mod project;

pub use fiscal_year::FiscalYear;
pub use milestone::{Milestone, MilestoneStatus};
pub use project::{Project, ProjectStatus};
