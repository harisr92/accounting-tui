use accounting_core::{
    DefaultTransactionValidator, Transaction, TransactionBuilder, TransactionValidator,
};
use bigdecimal::{BigDecimal, Zero};
use chrono::NaiveDate;

use super::{ServiceError, chart};
use crate::domain::{Expense, ExpenseCategory, ExpenseId, FiscalYear};
use crate::storage::{ExpenseRecord, ExpenseRepository, SettingsRepository};

/// Input for adding or editing an expense
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpenseDraft {
    pub date: NaiveDate,
    pub category: ExpenseCategory,
    pub description: String,
    /// Total paid, GST included
    pub amount: BigDecimal,
    /// GST in `amount` claimed back as input credit
    pub gst_credit: BigDecimal,
}

/// Business expenses for the regular-books view; each is posted to the ledger as it is saved
pub struct ExpenseService<R> {
    repo: R,
}

/// The ledger transaction that posts expense `id`
fn transaction_id(id: ExpenseId) -> String {
    format!("exp-{id}")
}

impl<R: ExpenseRepository + SettingsRepository> ExpenseService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }

    pub fn list(&self, year: FiscalYear) -> Result<Vec<Expense>, ServiceError> {
        Ok(self.repo.list_expenses(year)?)
    }

    /// Record an expense paid from the bank; `gst_credit` needs a GSTIN in the profile
    pub fn add(&mut self, draft: ExpenseDraft) -> Result<Expense, ServiceError> {
        let expense = Expense::new(
            draft.date,
            draft.category,
            draft.description,
            draft.amount,
            draft.gst_credit,
        )?;
        let transaction = self.posting(&expense)?;
        self.repo.add_expense(&ExpenseRecord {
            expense: &expense,
            transaction: &transaction,
            accounts: &chart::accounts(),
        })?;
        Ok(expense)
    }

    /// Correct an expense; its ledger posting is replaced in the same write
    pub fn edit(&mut self, id: ExpenseId, draft: ExpenseDraft) -> Result<Expense, ServiceError> {
        let mut expense = self.repo.load_expense(id)?;
        expense.edit(
            draft.date,
            draft.category,
            draft.description,
            draft.amount,
            draft.gst_credit,
        )?;
        let transaction = self.posting(&expense)?;
        self.repo.update_expense(&ExpenseRecord {
            expense: &expense,
            transaction: &transaction,
            accounts: &chart::accounts(),
        })?;
        Ok(expense)
    }

    /// The ledger transaction for an expense paid from the bank: its cost to the expense
    /// account, any input credit to Input GST. Input credit needs a GSTIN in the profile.
    fn posting(&self, expense: &Expense) -> Result<Transaction, ServiceError> {
        let registered = self
            .repo
            .load_profile()?
            .is_some_and(|p| p.gstin().is_some());
        if !registered && !expense.gst_credit().is_zero() {
            return Err(ServiceError::CreditWithoutGstin);
        }
        let category = expense.category();
        let mut builder = TransactionBuilder::new(
            transaction_id(expense.id()),
            expense.date(),
            format!("{}: {}", category.label(), expense.description()),
        )
        .debit(chart::expense_account(category), expense.cost(), None);
        if !expense.gst_credit().is_zero() {
            builder = builder.debit(chart::INPUT_GST, expense.gst_credit().clone(), None);
        }
        let transaction = builder
            .credit(chart::BANK, expense.amount().clone(), None)
            .build()?;
        DefaultTransactionValidator.validate_transaction(&transaction)?;
        Ok(transaction)
    }

    /// Remove an expense and its ledger posting
    pub fn delete(&mut self, id: ExpenseId) -> Result<(), ServiceError> {
        Ok(self.repo.delete_expense(id, &transaction_id(id))?)
    }
}
