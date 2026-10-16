//! Use-cases: load a record, apply a domain change, save it. Views call these, never storage.

pub mod chart;
mod error;
mod expense_service;
mod gstr1;
mod invoice_builder;
mod invoice_service;
pub mod merge_import;
mod project_service;
mod report_service;
pub mod reports;

pub use error::ServiceError;
pub use expense_service::ExpenseService;
pub use gstr1::QuarterlyGstr1;
pub use invoice_builder::{BuiltInvoice, InvoicingError, PLAIN_NOTE, build as build_invoice};
pub use invoice_service::{InvoicePreview, InvoiceService, Issued};
pub use project_service::{MilestoneDraft, ProjectDraft, ProjectService};
pub use report_service::ReportService;

#[cfg(test)]
mod tests;
