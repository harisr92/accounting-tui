use accounting_core::{Account, Transaction};

use super::StorageError;
use crate::domain::{
    BusinessProfile, FiscalYear, Invoice, InvoiceNumber, InvoiceSummary, Project, ProjectId,
    ProjectSummary,
};

/// Where projects live. The app depends on this trait, not on redb.
pub trait ProjectRepository {
    /// Every project's summary, sorted by name
    fn list(&self) -> Result<Vec<ProjectSummary>, StorageError>;

    fn load(&self, id: ProjectId) -> Result<Project, StorageError>;

    /// Insert or replace a project and its summary
    fn save(&mut self, project: &Project) -> Result<(), StorageError>;

    fn delete(&mut self, id: ProjectId) -> Result<(), StorageError>;
}

/// Everything that changes when an invoice is issued, saved together or not at all
pub struct IssueRecord<'a> {
    /// Must be the year's next sequence; refused if another invoice took it first
    pub sequence: u32,
    pub invoice: &'a Invoice,
    /// The project with the milestone now invoiced
    pub project: &'a Project,
    /// Ledger posting of the invoice
    pub transaction: &'a Transaction,
    /// Ledger accounts the posting needs; any that don't exist yet are created
    pub accounts: &'a [Account],
}

/// Everything that changes when an invoice is paid, saved together or not at all
pub struct PaymentRecord<'a> {
    pub invoice: &'a Invoice,
    pub project: &'a Project,
    pub transaction: &'a Transaction,
    pub accounts: &'a [Account],
}

/// Issued invoices and their numbering
pub trait InvoiceRepository {
    /// The sequence the next invoice of `year` will get (1 for a new year)
    fn next_sequence(&self, year: FiscalYear) -> Result<u32, StorageError>;

    fn issue(&mut self, record: &IssueRecord<'_>) -> Result<(), StorageError>;

    fn record_payment(&mut self, record: &PaymentRecord<'_>) -> Result<(), StorageError>;

    /// Every invoice's summary, newest first
    fn list_invoices(&self) -> Result<Vec<InvoiceSummary>, StorageError>;

    fn load_invoice(&self, number: &InvoiceNumber) -> Result<Invoice, StorageError>;
}

/// The freelancer's business details
pub trait SettingsRepository {
    /// `None` until the profile is first saved
    fn load_profile(&self) -> Result<Option<BusinessProfile>, StorageError>;

    fn save_profile(&mut self, profile: &BusinessProfile) -> Result<(), StorageError>;
}

/// All of the repositories at once, as the app's views use them
pub trait Repository: ProjectRepository + InvoiceRepository + SettingsRepository + Clone {}

impl<T: ProjectRepository + InvoiceRepository + SettingsRepository + Clone> Repository for T {}
