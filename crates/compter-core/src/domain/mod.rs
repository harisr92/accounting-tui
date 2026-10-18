//! Pure domain types and rules. No I/O, no UI.

mod appearance;
mod billing;
mod credit_note;
mod currency;
mod error;
mod expense;
mod fiscal_year;
mod invoice;
mod logo;
mod milestone;
mod profile;
mod project;
mod states;

pub use appearance::Appearance;
pub use billing::{Billing, MAX_ENTRY_MINUTES, TimeEntry, format_duration};
pub use credit_note::{CREDIT_NOTE_PREFIX, CreditNote, CreditNoteParts};
pub use currency::{Currency, ExchangeRate};
pub use error::DomainError;
pub use expense::{Expense, ExpenseCategory, ExpenseId};
pub use fiscal_year::{FiscalYear, Quarter};
pub use invoice::{
    Credit, Invoice, InvoiceAmounts, InvoiceKind, InvoiceNumber, InvoiceParts, InvoiceStatus,
    InvoiceSummary, Payment,
};
pub use logo::{Logo, LogoFormat, LogoId, MAX_LOGO_BYTES};
pub use milestone::{Milestone, MilestoneId, MilestoneStatus};
pub use profile::{BusinessProfile, DEFAULT_INVOICE_PREFIX, DEFAULT_SAC, ProfileDraft};
pub use project::{ClientDetails, ClientRegion, Project, ProjectId, ProjectStatus, ProjectSummary};
pub use states::state_name;

#[cfg(test)]
mod tests;
