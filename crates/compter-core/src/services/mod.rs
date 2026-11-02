//! Use-cases: load a record, apply a domain change, save it. Views call these, never storage.

pub mod chart;
mod client_service;
mod credit_note_builder;
mod dashboard_service;
mod error;
mod expense_service;
mod gstr1;
mod invoice_builder;
mod invoice_service;
pub mod merge_import;
mod preferences_service;
mod project_service;
mod report_service;
pub mod reports;

pub use client_service::{ClientAccount, ClientService, ClientSummary, Owed};
pub use credit_note_builder::{BuiltCreditNote, CreditDraft, CreditLineDraft};
pub use dashboard_service::{Dashboard, DashboardService, ReadyMilestone};
pub use error::ServiceError;
pub use expense_service::{ExpenseDraft, ExpenseService};
pub use gstr1::{MonthlyIff, QuarterlyGstr1};
pub use invoice_builder::{BuiltInvoice, InvoicingError, PLAIN_NOTE, build as build_invoice};
pub use invoice_service::{
    CreditPreview, CreditableLine, Credited, InvoicePreview, InvoiceService, Issued,
    creditable_lines_of,
};
pub use preferences_service::PreferencesService;
pub use project_service::{MilestoneDraft, ProjectDraft, ProjectService};
pub use report_service::ReportService;

#[cfg(test)]
mod tests;
