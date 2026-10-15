use accounting_core::{DefaultTransactionValidator, TransactionBuilder, TransactionValidator};
use bigdecimal::{BigDecimal, Zero};
use chrono::NaiveDate;

use super::{ServiceError, chart};
use crate::domain::{Expense, ExpenseCategory, ExpenseId, FiscalYear};
use crate::storage::{ExpenseRecord, ExpenseRepository, SettingsRepository};

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
    pub fn add(
        &mut self,
        date: NaiveDate,
        category: ExpenseCategory,
        description: &str,
        amount: BigDecimal,
        gst_credit: BigDecimal,
    ) -> Result<Expense, ServiceError> {
        let expense = Expense::new(date, category, description, amount, gst_credit)?;
        let registered = self
            .repo
            .load_profile()?
            .is_some_and(|p| p.gstin().is_some());
        if !registered && !expense.gst_credit().is_zero() {
            return Err(ServiceError::CreditWithoutGstin);
        }

        let mut builder = TransactionBuilder::new(
            transaction_id(expense.id()),
            date,
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

        self.repo.add_expense(&ExpenseRecord {
            expense: &expense,
            transaction: &transaction,
            accounts: &chart::accounts(),
        })?;
        Ok(expense)
    }

    /// Remove an expense and its ledger posting
    pub fn delete(&mut self, id: ExpenseId) -> Result<(), ServiceError> {
        Ok(self.repo.delete_expense(id, &transaction_id(id))?)
    }
}
