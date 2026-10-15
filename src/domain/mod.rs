//! Pure domain types and rules. No I/O, no UI.

mod billing;
mod error;
mod fiscal_year;
mod invoice;
mod milestone;
mod profile;
mod project;
mod states;

pub use billing::{Billing, MAX_ENTRY_MINUTES, TimeEntry};
pub use error::DomainError;
pub use fiscal_year::FiscalYear;
pub use invoice::{
    Invoice, InvoiceAmounts, InvoiceKind, InvoiceNumber, InvoiceParts, InvoiceStatus,
    InvoiceSummary,
};
pub use milestone::{Milestone, MilestoneId, MilestoneStatus};
pub use profile::{BusinessProfile, DEFAULT_INVOICE_PREFIX, DEFAULT_SAC, ProfileDraft};
pub use project::{ClientDetails, ClientRegion, Project, ProjectId, ProjectStatus, ProjectSummary};
pub use states::state_name;

#[cfg(test)]
mod tests;
