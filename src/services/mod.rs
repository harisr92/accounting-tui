//! Use-cases: load a record, apply a domain change, save it. Views call these, never storage.

pub mod chart;
mod error;
mod invoice_builder;
mod invoice_service;
mod project_service;

pub use error::ServiceError;
pub use invoice_builder::{BuiltInvoice, InvoicingError, PLAIN_NOTE, build as build_invoice};
pub use invoice_service::{InvoicePreview, InvoiceService, Issued};
pub use project_service::{MilestoneDraft, ProjectDraft, ProjectService};

#[cfg(test)]
mod tests;
