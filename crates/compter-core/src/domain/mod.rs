//! Pure domain types and rules. No I/O, no UI.

mod appearance;
mod billing;
mod client;
mod credit_note;
mod currency;
mod discount;
mod error;
mod expense;
mod fiscal_year;
mod invoice;
mod logo;
mod milestone;
mod profile;
mod project;
mod record_field;
mod states;

pub use appearance::Appearance;
pub use billing::{Billing, MAX_ENTRY_MINUTES, TimeEntry, format_duration};
pub use client::{
    Client, ClientDetails, ClientField, ClientId, ClientIndex, ClientRegion, client_key,
    same_client,
};
pub use credit_note::{
    CREDIT_NOTE_PREFIX, CreditLine, CreditNote, CreditNoteField, CreditNoteParts,
};
pub use currency::{Currency, ExchangeRate};
pub use discount::Discount;
pub use error::DomainError;
pub use expense::{Expense, ExpenseCategory, ExpenseField, ExpenseId};
pub use fiscal_year::{FiscalYear, Quarter};
pub use invoice::{
    Credit, Invoice, InvoiceAmounts, InvoiceField, InvoiceKind, InvoiceLine, InvoiceNumber,
    InvoiceParts, InvoiceStatus, InvoiceSummary, OVERDUE_DAYS, Payment, PaymentField, ReturnKind,
    ReturnTerms, Returned,
};
pub use logo::{Logo, LogoFormat, LogoId, MAX_LOGO_BYTES};
pub use milestone::{Milestone, MilestoneField, MilestoneId, MilestoneSort, MilestoneStatus};
pub use profile::{
    BasisChange, BasisSchedule, BusinessProfile, DEFAULT_INVOICE_PREFIX, DEFAULT_SAC, IncomeBasis,
    ProfileDraft, ProfileField,
};
pub use project::{Project, ProjectField, ProjectId, ProjectStatus, ProjectSummary};
pub use record_field::RecordField;
pub use states::{state_label, state_name, states};

#[cfg(test)]
mod tests;
