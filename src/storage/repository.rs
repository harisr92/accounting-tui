use accounting_core::{Account, Transaction};

use super::StorageError;
use crate::domain::{
    Appearance, BusinessProfile, CreditNote, Expense, ExpenseId, FiscalYear, Invoice,
    InvoiceNumber, InvoiceSummary, Logo, LogoId, Project, ProjectId, ProjectSummary,
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

/// Everything that changes when a payment is recorded, saved together or not at all
pub struct PaymentRecord<'a> {
    /// The invoice with the payment added
    pub invoice: &'a Invoice,
    /// The project, when this payment settled the invoice and its milestone is now paid
    pub project: Option<&'a Project>,
    pub transaction: &'a Transaction,
    pub accounts: &'a [Account],
}

/// Everything that changes when a credit note is issued, saved together or not at all
pub struct CreditRecord<'a> {
    /// Must be the year's next credit note sequence
    pub sequence: u32,
    pub note: &'a CreditNote,
    /// The invoice with the credit applied
    pub invoice: &'a Invoice,
    /// The project, with the credit recorded on the invoice's milestone
    pub project: &'a Project,
    pub transaction: &'a Transaction,
    pub accounts: &'a [Account],
}

/// Issued invoices, the credit notes against them, and their numbering
pub trait InvoiceRepository {
    /// The sequence the next invoice of `year` will get (1 for a new year)
    fn next_sequence(&self, year: FiscalYear) -> Result<u32, StorageError>;

    fn issue(&mut self, record: &IssueRecord<'_>) -> Result<(), StorageError>;

    fn record_payment(&mut self, record: &PaymentRecord<'_>) -> Result<(), StorageError>;

    /// Every invoice's summary, newest first
    fn list_invoices(&self) -> Result<Vec<InvoiceSummary>, StorageError>;

    fn load_invoice(&self, number: &InvoiceNumber) -> Result<Invoice, StorageError>;

    /// Hand every invoice to `visit`, one at a time, so reports never hold them all
    fn scan_invoices(&self, visit: &mut dyn FnMut(Invoice)) -> Result<(), StorageError>;

    /// The sequence the next credit note of `year` will get (1 for a new year)
    fn next_credit_sequence(&self, year: FiscalYear) -> Result<u32, StorageError>;

    fn issue_credit_note(&mut self, record: &CreditRecord<'_>) -> Result<(), StorageError>;

    fn load_credit_note(&self, number: &InvoiceNumber) -> Result<CreditNote, StorageError>;

    /// Hand every credit note to `visit`, one at a time
    fn scan_credit_notes(&self, visit: &mut dyn FnMut(CreditNote)) -> Result<(), StorageError>;
}

/// An expense and its ledger posting, saved together
pub struct ExpenseRecord<'a> {
    pub expense: &'a Expense,
    pub transaction: &'a Transaction,
    pub accounts: &'a [Account],
}

/// Business expenses, each posted to the ledger
pub trait ExpenseRepository {
    fn add_expense(&mut self, record: &ExpenseRecord<'_>) -> Result<(), StorageError>;

    fn load_expense(&self, id: ExpenseId) -> Result<Expense, StorageError>;

    /// Replace a stored expense and its ledger posting (same transaction id) with `record`
    fn update_expense(&mut self, record: &ExpenseRecord<'_>) -> Result<(), StorageError>;

    /// Remove an expense and the ledger transaction `transaction_id` that posted it
    fn delete_expense(&mut self, id: ExpenseId, transaction_id: &str) -> Result<(), StorageError>;

    /// Expenses dated in `year`, newest first
    fn list_expenses(&self, year: FiscalYear) -> Result<Vec<Expense>, StorageError>;
}

/// The freelancer's business details
pub trait SettingsRepository {
    /// `None` until the profile is first saved
    fn load_profile(&self) -> Result<Option<BusinessProfile>, StorageError>;

    fn save_profile(&mut self, profile: &BusinessProfile) -> Result<(), StorageError>;

    /// A stored logo; `None` if no logo has that id
    fn load_logo(&self, id: LogoId) -> Result<Option<Logo>, StorageError>;

    /// Save `profile` and, when given, the new logo it points at, in one write
    fn set_logo(
        &mut self,
        profile: &BusinessProfile,
        logo: Option<&Logo>,
    ) -> Result<(), StorageError>;
}

/// All of the repositories at once, as the app's views use them
pub trait Repository:
    ProjectRepository + InvoiceRepository + SettingsRepository + ExpenseRepository + Clone
{
}

impl<T> Repository for T where
    T: ProjectRepository + InvoiceRepository + SettingsRepository + ExpenseRepository + Clone
{
}
