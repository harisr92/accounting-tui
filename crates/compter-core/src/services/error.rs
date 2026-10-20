use accounting_core::{LedgerError, PdfError};

use super::InvoicingError;
use crate::domain::DomainError;
use crate::storage::StorageError;
use crate::storage::sync::SyncError;

/// A use-case failed: a domain rule, invoicing prerequisites, the ledger, PDF output or storage
#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    #[error(transparent)]
    Domain(#[from] DomainError),
    #[error(transparent)]
    Invoicing(#[from] InvoicingError),
    #[error(transparent)]
    Ledger(#[from] LedgerError),
    #[error("could not produce the PDF: {0}")]
    Pdf(#[from] PdfError),
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error("{name}: {source}")]
    CreditLine {
        /// The milestone the refused line billed
        milestone: crate::domain::MilestoneId,
        name: String,
        #[source]
        source: Box<DomainError>,
    },
    #[error("the milestone billed by invoice {0} no longer exists")]
    MissingMilestone(String),
    #[error("no project is named {0:?}")]
    NoSuchProject(String),
    #[error("more than one project is named {0:?}")]
    AmbiguousProject(String),
    #[error("PDFs are not saved in demo mode")]
    NoPdfFolder,
    #[error("GST input credit can only be claimed once your GSTIN is in Settings")]
    CreditWithoutGstin,
    #[error("returns are not saved in demo mode")]
    NoReturnsFolder,
    #[error("GSTR-1 needs your GSTIN in Settings")]
    NotRegistered,
    #[error(transparent)]
    Gstr1(#[from] accounting_core::Gstr1Error),
    #[error("could not read the logo {path}")]
    ReadLogo {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("logo {0} is missing from the database")]
    MissingLogo(crate::domain::LogoId),
    #[error(transparent)]
    Sync(#[from] SyncError),
    #[error("sync is off in demo mode")]
    NoSyncInDemo,
}
