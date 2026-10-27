use std::sync::Arc;

use accounting_core::{Account, Transaction};
use redb::{
    Database, ReadableDatabase, ReadableTable, ReadableTableMetadata, TableHandle, WriteTransaction,
};

use super::codec::{decode, encode};
use super::tables::{
    ACCOUNTS, APPEARANCE_KEY, CREDIT_NOTE_COUNTERS, CREDIT_NOTES, EXPENSES, INVOICE_COUNTERS,
    INVOICE_SUMMARIES, INVOICES, LOGOS, PROFILE_KEY, PROJECT_SUMMARIES, PROJECTS, SETTINGS,
    TRANSACTIONS,
};
use super::{
    CreditRecord, ExpenseRecord, ExpenseRepository, InvoiceRepository, IssueRecord, PaymentRecord,
    ProjectRepository, SettingsRepository, StorageError,
};
use crate::domain::{
    Appearance, BusinessProfile, CreditNote, Expense, ExpenseId, FiscalYear, Invoice,
    InvoiceNumber, InvoiceSummary, Logo, LogoId, Project, ProjectId, ProjectSummary,
};

/// All of compter's records on redb. One type behind every repository trait, so a change that
/// spans projects, invoices and the ledger is one write transaction.
#[derive(Clone)]
pub struct RedbRepository {
    db: Arc<Database>,
}

impl RedbRepository {
    pub(super) fn new(db: Arc<Database>) -> Self {
        Self { db }
    }
}

/// Write a project and its summary
fn put_project(txn: &WriteTransaction, project: &Project) -> Result<(), StorageError> {
    let id = project.id();
    let record = encode(PROJECTS.name(), id, project)?;
    let summary = encode(PROJECT_SUMMARIES.name(), id, &ProjectSummary::from(project))?;
    txn.open_table(PROJECTS)?
        .insert(id.as_uuid(), record.as_slice())?;
    txn.open_table(PROJECT_SUMMARIES)?
        .insert(id.as_uuid(), summary.as_slice())?;
    Ok(())
}

/// Write an invoice and its summary
pub(super) fn put_invoice(txn: &WriteTransaction, invoice: &Invoice) -> Result<(), StorageError> {
    let key = invoice.number().as_str();
    let record = encode(INVOICES.name(), key, invoice)?;
    let summary = encode(
        INVOICE_SUMMARIES.name(),
        key,
        &InvoiceSummary::from(invoice),
    )?;
    txn.open_table(INVOICES)?.insert(key, record.as_slice())?;
    txn.open_table(INVOICE_SUMMARIES)?
        .insert(key, summary.as_slice())?;
    Ok(())
}

/// Add a new ledger transaction, refusing to overwrite one with the same id
fn put_transaction(txn: &WriteTransaction, transaction: &Transaction) -> Result<(), StorageError> {
    let id = transaction.id.as_str();
    let bytes = encode(TRANSACTIONS.name(), id, transaction)?;
    let mut table = txn.open_table(TRANSACTIONS)?;
    if table.get(id)?.is_some() {
        return Err(StorageError::DuplicateTransaction(id.to_owned()));
    }
    table.insert(id, bytes.as_slice())?;
    Ok(())
}

/// Create the ledger accounts that don't exist yet; existing ones are left as they are
fn put_missing_accounts(txn: &WriteTransaction, accounts: &[Account]) -> Result<(), StorageError> {
    let mut table = txn.open_table(ACCOUNTS)?;
    for account in accounts {
        if table.get(account.id.as_str())?.is_none() {
            let bytes = encode(ACCOUNTS.name(), &account.id, account)?;
            table.insert(account.id.as_str(), bytes.as_slice())?;
        }
    }
    Ok(())
}

impl ProjectRepository for RedbRepository {
    fn list(&self) -> Result<Vec<ProjectSummary>, StorageError> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(PROJECT_SUMMARIES)?;
        let mut summaries = Vec::with_capacity(usize::try_from(table.len()?).unwrap_or_default());
        for row in table.iter()? {
            let (key, value) = row?;
            summaries.push(decode(
                PROJECT_SUMMARIES.name(),
                key.value(),
                value.value(),
            )?);
        }
        summaries.sort_by(|a: &ProjectSummary, b| a.name().cmp(b.name()).then(a.id().cmp(&b.id())));
        Ok(summaries)
    }

    fn load(&self, id: ProjectId) -> Result<Project, StorageError> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(PROJECTS)?;
        let row = table
            .get(id.as_uuid())?
            .ok_or(StorageError::ProjectNotFound(id))?;
        decode(PROJECTS.name(), id, row.value())
    }

    fn save(&mut self, project: &Project) -> Result<(), StorageError> {
        let txn = self.db.begin_write()?;
        put_project(&txn, project)?;
        txn.commit()?;
        Ok(())
    }

    fn delete(&mut self, id: ProjectId) -> Result<(), StorageError> {
        let txn = self.db.begin_write()?;
        {
            let removed = txn.open_table(PROJECTS)?.remove(id.as_uuid())?.is_some();
            if !removed {
                // Dropping the transaction aborts it
                return Err(StorageError::ProjectNotFound(id));
            }
            txn.open_table(PROJECT_SUMMARIES)?.remove(id.as_uuid())?;
        }
        txn.commit()?;
        Ok(())
    }
}

impl InvoiceRepository for RedbRepository {
    fn next_sequence(&self, year: FiscalYear) -> Result<u32, StorageError> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(INVOICE_COUNTERS)?;
        let last = table.get(year.label().as_str())?.map_or(0, |v| v.value());
        Ok(last.saturating_add(1))
    }

    fn issue(&mut self, record: &IssueRecord<'_>) -> Result<(), StorageError> {
        let number = record.invoice.number().as_str();
        let year = record.invoice.fiscal_year().label();
        let txn = self.db.begin_write()?;
        {
            let mut counters = txn.open_table(INVOICE_COUNTERS)?;
            let last = counters.get(year.as_str())?.map_or(0, |v| v.value());
            if record.sequence != last.saturating_add(1) {
                return Err(StorageError::StaleInvoiceNumber);
            }
            counters.insert(year.as_str(), record.sequence)?;
        }
        // Invoices and credit notes share one numbering space: a number is never reused
        if txn.open_table(INVOICES)?.get(number)?.is_some()
            || txn.open_table(CREDIT_NOTES)?.get(number)?.is_some()
        {
            return Err(StorageError::DuplicateInvoice(number.to_owned()));
        }
        put_invoice(&txn, record.invoice)?;
        put_project(&txn, record.project)?;
        put_missing_accounts(&txn, record.accounts)?;
        put_transaction(&txn, record.transaction)?;
        txn.commit()?;
        Ok(())
    }

    fn record_payment(&mut self, record: &PaymentRecord<'_>) -> Result<(), StorageError> {
        let number = record.invoice.number().as_str();
        let txn = self.db.begin_write()?;
        if txn.open_table(INVOICES)?.get(number)?.is_none() {
            return Err(StorageError::InvoiceNotFound(number.to_owned()));
        }
        put_invoice(&txn, record.invoice)?;
        if let Some(project) = record.project {
            put_project(&txn, project)?;
        }
        put_missing_accounts(&txn, record.accounts)?;
        put_transaction(&txn, record.transaction)?;
        txn.commit()?;
        Ok(())
    }

    fn list_invoices(&self) -> Result<Vec<InvoiceSummary>, StorageError> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(INVOICE_SUMMARIES)?;
        let mut summaries = Vec::with_capacity(usize::try_from(table.len()?).unwrap_or_default());
        for row in table.iter()? {
            let (key, value) = row?;
            summaries.push(decode(
                INVOICE_SUMMARIES.name(),
                key.value(),
                value.value(),
            )?);
        }
        summaries.sort_by(|a: &InvoiceSummary, b| {
            b.date().cmp(&a.date()).then(b.number().cmp(a.number()))
        });
        Ok(summaries)
    }

    fn load_invoice(&self, number: &InvoiceNumber) -> Result<Invoice, StorageError> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(INVOICES)?;
        let row = table
            .get(number.as_str())?
            .ok_or_else(|| StorageError::InvoiceNotFound(number.to_string()))?;
        decode(INVOICES.name(), number, row.value())
    }

    fn scan_invoices(&self, visit: &mut dyn FnMut(Invoice)) -> Result<(), StorageError> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(INVOICES)?;
        for row in table.iter()? {
            let (key, value) = row?;
            visit(decode(INVOICES.name(), key.value(), value.value())?);
        }
        Ok(())
    }

    fn next_credit_sequence(&self, year: FiscalYear) -> Result<u32, StorageError> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(CREDIT_NOTE_COUNTERS)?;
        let last = table.get(year.label().as_str())?.map_or(0, |v| v.value());
        Ok(last.saturating_add(1))
    }

    fn issue_credit_note(&mut self, record: &CreditRecord<'_>) -> Result<(), StorageError> {
        let number = record.note.number().as_str();
        let year = record.note.fiscal_year().label();
        let invoice = record.invoice.number().as_str();
        let txn = self.db.begin_write()?;
        {
            let mut counters = txn.open_table(CREDIT_NOTE_COUNTERS)?;
            let last = counters.get(year.as_str())?.map_or(0, |v| v.value());
            if record.sequence != last.saturating_add(1) {
                return Err(StorageError::StaleInvoiceNumber);
            }
            counters.insert(year.as_str(), record.sequence)?;
        }
        if txn.open_table(INVOICES)?.get(invoice)?.is_none() {
            return Err(StorageError::InvoiceNotFound(invoice.to_owned()));
        }
        {
            let mut notes = txn.open_table(CREDIT_NOTES)?;
            if notes.get(number)?.is_some() || txn.open_table(INVOICES)?.get(number)?.is_some() {
                return Err(StorageError::DuplicateInvoice(number.to_owned()));
            }
            let bytes = encode(CREDIT_NOTES.name(), number, record.note)?;
            notes.insert(number, bytes.as_slice())?;
        }
        put_invoice(&txn, record.invoice)?;
        put_project(&txn, record.project)?;
        put_missing_accounts(&txn, record.accounts)?;
        put_transaction(&txn, record.transaction)?;
        if let Some(returned) = record.returned {
            put_transaction(&txn, returned)?;
        }
        txn.commit()?;
        Ok(())
    }

    fn load_credit_note(&self, number: &InvoiceNumber) -> Result<CreditNote, StorageError> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(CREDIT_NOTES)?;
        let row = table
            .get(number.as_str())?
            .ok_or_else(|| StorageError::CreditNoteNotFound(number.to_string()))?;
        decode(CREDIT_NOTES.name(), number, row.value())
    }

    fn scan_credit_notes(&self, visit: &mut dyn FnMut(CreditNote)) -> Result<(), StorageError> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(CREDIT_NOTES)?;
        for row in table.iter()? {
            let (key, value) = row?;
            visit(decode(CREDIT_NOTES.name(), key.value(), value.value())?);
        }
        Ok(())
    }
}

impl ExpenseRepository for RedbRepository {
    fn add_expense(&mut self, record: &ExpenseRecord<'_>) -> Result<(), StorageError> {
        let id = record.expense.id();
        let bytes = encode(EXPENSES.name(), id, record.expense)?;
        let txn = self.db.begin_write()?;
        txn.open_table(EXPENSES)?
            .insert(id.as_uuid(), bytes.as_slice())?;
        put_missing_accounts(&txn, record.accounts)?;
        put_transaction(&txn, record.transaction)?;
        txn.commit()?;
        Ok(())
    }

    fn load_expense(&self, id: ExpenseId) -> Result<Expense, StorageError> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(EXPENSES)?;
        let row = table
            .get(id.as_uuid())?
            .ok_or(StorageError::ExpenseNotFound(id))?;
        decode(EXPENSES.name(), id, row.value())
    }

    fn update_expense(&mut self, record: &ExpenseRecord<'_>) -> Result<(), StorageError> {
        let id = record.expense.id();
        let bytes = encode(EXPENSES.name(), id, record.expense)?;
        let txn = self.db.begin_write()?;
        if txn
            .open_table(EXPENSES)?
            .insert(id.as_uuid(), bytes.as_slice())?
            .is_none()
        {
            return Err(StorageError::ExpenseNotFound(id));
        }
        txn.open_table(TRANSACTIONS)?
            .remove(record.transaction.id.as_str())?;
        put_missing_accounts(&txn, record.accounts)?;
        put_transaction(&txn, record.transaction)?;
        txn.commit()?;
        Ok(())
    }

    fn delete_expense(&mut self, id: ExpenseId, transaction_id: &str) -> Result<(), StorageError> {
        let txn = self.db.begin_write()?;
        if txn.open_table(EXPENSES)?.remove(id.as_uuid())?.is_none() {
            return Err(StorageError::ExpenseNotFound(id));
        }
        txn.open_table(TRANSACTIONS)?.remove(transaction_id)?;
        txn.commit()?;
        Ok(())
    }

    fn list_expenses(&self, year: FiscalYear) -> Result<Vec<Expense>, StorageError> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(EXPENSES)?;
        let mut expenses = Vec::new();
        for row in table.iter()? {
            let (key, value) = row?;
            let expense: Expense = decode(EXPENSES.name(), key.value(), value.value())?;
            if year.contains(expense.date()) {
                expenses.push(expense);
            }
        }
        expenses.sort_by(|a, b| b.date().cmp(&a.date()).then(a.id().cmp(&b.id())));
        Ok(expenses)
    }
}

impl SettingsRepository for RedbRepository {
    fn load_profile(&self) -> Result<Option<BusinessProfile>, StorageError> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(SETTINGS)?;
        let row = table.get(PROFILE_KEY)?;
        row.map(|value| decode(SETTINGS.name(), PROFILE_KEY, value.value()))
            .transpose()
    }

    fn save_profile(&mut self, profile: &BusinessProfile) -> Result<(), StorageError> {
        let txn = self.db.begin_write()?;
        put_profile(&txn, profile)?;
        txn.commit()?;
        Ok(())
    }

    fn load_appearance(&self) -> Result<Appearance, StorageError> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(SETTINGS)?;
        let row = table.get(APPEARANCE_KEY)?;
        // A theme only a newer build knows (the database travels between devices) falls
        // back to the default rather than stopping the app from starting
        Ok(row
            .and_then(|value| decode(SETTINGS.name(), APPEARANCE_KEY, value.value()).ok())
            .unwrap_or_default())
    }

    fn save_appearance(&mut self, appearance: Appearance) -> Result<(), StorageError> {
        let bytes = encode(SETTINGS.name(), APPEARANCE_KEY, &appearance)?;
        let txn = self.db.begin_write()?;
        txn.open_table(SETTINGS)?
            .insert(APPEARANCE_KEY, bytes.as_slice())?;
        txn.commit()?;
        Ok(())
    }

    fn load_logo(&self, id: LogoId) -> Result<Option<Logo>, StorageError> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(LOGOS)?;
        let row = table.get(id.as_uuid())?;
        row.map(|value| {
            Logo::from_stored(id, value.value().to_vec()).map_err(|source| {
                StorageError::InvalidLogo {
                    key: id.to_string(),
                    source,
                }
            })
        })
        .transpose()
    }

    fn set_logo(
        &mut self,
        profile: &BusinessProfile,
        logo: Option<&Logo>,
    ) -> Result<(), StorageError> {
        let txn = self.db.begin_write()?;
        if let Some(logo) = logo {
            put_logo(&txn, logo)?;
        }
        put_profile(&txn, profile)?;
        txn.commit()?;
        Ok(())
    }
}

fn put_profile(txn: &WriteTransaction, profile: &BusinessProfile) -> Result<(), StorageError> {
    let bytes = encode(SETTINGS.name(), PROFILE_KEY, profile)?;
    txn.open_table(SETTINGS)?
        .insert(PROFILE_KEY, bytes.as_slice())?;
    Ok(())
}

/// Logos are stored as their raw image bytes, never overwritten
fn put_logo(txn: &WriteTransaction, logo: &Logo) -> Result<(), StorageError> {
    txn.open_table(LOGOS)?
        .insert(logo.id().as_uuid(), logo.bytes())?;
    Ok(())
}
