use accounting_core::{Account, Transaction};

use super::StorageError;
use crate::domain::{
    Appearance, BusinessProfile, Client, ClientId, CreditNote, Expense, ExpenseId, FiscalYear,
    Invoice, InvoiceNumber, InvoiceSummary, Logo, LogoId, Project, ProjectId, ProjectSummary,
};

/// Where projects live. The app depends on this trait, not on redb.
pub trait ProjectRepository {
    /// Every project's summary, sorted by name
    fn list(&self) -> Result<Vec<ProjectSummary>, StorageError>;

    fn load(&self, id: ProjectId) -> Result<Project, StorageError>;

    /// Hand every project to `visit`, one at a time, so a scan never holds them all
    fn scan_projects(&self, visit: &mut dyn FnMut(Project)) -> Result<(), StorageError>;

    /// Insert or replace a project and its summary
    fn save(&mut self, project: &Project) -> Result<(), StorageError>;

    /// Save a project whose client was entered or changed, with that client's record, in one
    /// write
    fn save_with_client(&mut self, project: &Project, client: &Client) -> Result<(), StorageError>;

    fn delete(&mut self, id: ProjectId) -> Result<(), StorageError>;
}

/// A client corrected, renamed or merged in the address book, with the open projects that take
/// the change too, saved together or not at all
pub struct ClientRecord<'a> {
    pub client: &'a Client,
    /// Projects whose copy of the client was updated; may be empty
    pub projects: &'a [Project],
    /// The client merged into `client`, whose record goes
    pub removed: Option<ClientId>,
}

/// The clients billed so far: the address book a new project's client is picked from. Clients
/// are written with a project ([`ProjectRepository::save_with_client`]), or corrected on their
/// own ([`save_client`](Self::save_client)), which also removes a client merged into another.
/// A client is deleted ([`delete_client`](Self::delete_client)) only once nothing is filed
/// under them; the service checks that.
pub trait ClientRepository {
    /// Every client, sorted by name
    fn clients(&self) -> Result<Vec<Client>, StorageError>;

    /// Replace a client's record and the projects in `record`, and remove the client it
    /// names as merged, in one write
    fn save_client(&mut self, record: &ClientRecord) -> Result<(), StorageError>;

    /// Remove a client from the address book; [`StorageError::ClientNotFound`] when there is
    /// no such client
    fn delete_client(&mut self, id: ClientId) -> Result<(), StorageError>;
}

/// Everything that changes when an invoice is issued, saved together or not at all
pub struct IssueRecord<'a> {
    /// Must be the year's next sequence; refused if another invoice took it first
    pub sequence: u32,
    pub invoice: &'a Invoice,
    /// The project with the milestones now invoiced
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
    /// The project, when this payment settled the invoice and its milestones are now paid
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
    /// The project, with the credit recorded on the invoice's milestones
    pub project: &'a Project,
    pub transaction: &'a Transaction,
    /// The posting that gives back money already paid (a refund, or an advance), if the note
    /// took off more than was due
    pub returned: Option<&'a Transaction>,
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

    /// The colour theme; the default until one is chosen
    fn load_appearance(&self) -> Result<Appearance, StorageError>;

    fn save_appearance(&mut self, appearance: Appearance) -> Result<(), StorageError>;

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
    ProjectRepository
    + ClientRepository
    + InvoiceRepository
    + SettingsRepository
    + ExpenseRepository
    + Clone
{
}

impl<T> Repository for T where
    T: ProjectRepository
        + ClientRepository
        + InvoiceRepository
        + SettingsRepository
        + ExpenseRepository
        + Clone
{
}
