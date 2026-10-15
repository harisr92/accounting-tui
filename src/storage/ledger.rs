use std::sync::Arc;

use accounting_core::{
    Account, AccountStore, AccountType, LedgerError, LedgerResult, ListResponse, PaginationOption,
    Transaction, TransactionFilter, TransactionStore,
};
use async_trait::async_trait;
use redb::{Database, ReadableDatabase, ReadableTable, TableHandle};
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::StorageError;
use super::codec::{decode, encode};
use super::tables::{ACCOUNTS, LedgerTable, TRANSACTIONS};

/// accounting-core ledger storage on redb. Holds no data in memory; every call reads or writes
/// the database, with the same semantics as accounting-core's `MemoryStorage`.
#[derive(Clone)]
pub struct RedbLedgerStorage {
    db: Arc<Database>,
}

impl RedbLedgerStorage {
    pub(super) fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    fn get<T: DeserializeOwned>(
        &self,
        table: &'static LedgerTable,
        id: &str,
    ) -> Result<Option<T>, StorageError> {
        let txn = self.db.begin_read()?;
        let opened = txn.open_table(*table)?;
        let row = opened.get(id)?;
        row.map(|value| decode(table.name(), id, value.value()))
            .transpose()
    }

    /// Decode the rows that pass `keep`, in key order
    fn scan<T: DeserializeOwned>(
        &self,
        table: &'static LedgerTable,
        keep: impl Fn(&T) -> bool,
    ) -> Result<Vec<T>, StorageError> {
        let txn = self.db.begin_read()?;
        let opened = txn.open_table(*table)?;
        let mut kept = Vec::new();
        for row in opened.iter()? {
            let (key, value) = row?;
            let item: T = decode(table.name(), key.value(), value.value())?;
            if keep(&item) {
                kept.push(item);
            }
        }
        Ok(kept)
    }

    /// Insert or replace a row; with `must_exist`, write nothing and return `false` when absent
    fn put<T: Serialize>(
        &self,
        table: &'static LedgerTable,
        id: &str,
        value: &T,
        must_exist: bool,
    ) -> Result<bool, StorageError> {
        let bytes = encode(table.name(), id, value)?;
        let txn = self.db.begin_write()?;
        {
            let mut opened = txn.open_table(*table)?;
            if must_exist && opened.get(id)?.is_none() {
                return Ok(false);
            }
            opened.insert(id, bytes.as_slice())?;
        }
        txn.commit()?;
        Ok(true)
    }

    /// Delete a row; `false` when it did not exist
    fn remove(&self, table: &'static LedgerTable, id: &str) -> Result<bool, StorageError> {
        let txn = self.db.begin_write()?;
        let removed = txn.open_table(*table)?.remove(id)?.is_some();
        if removed {
            txn.commit()?;
        }
        Ok(removed)
    }
}

#[async_trait]
impl AccountStore for RedbLedgerStorage {
    async fn save_account(&mut self, account: &Account) -> LedgerResult<()> {
        self.put(&ACCOUNTS, &account.id, account, false)?;
        Ok(())
    }

    async fn get_account(&self, account_id: &str) -> LedgerResult<Option<Account>> {
        Ok(self.get(&ACCOUNTS, account_id)?)
    }

    async fn list_accounts(
        &self,
        account_type: Option<AccountType>,
        pagination: PaginationOption,
    ) -> LedgerResult<ListResponse<Account>> {
        // Keys are account ids, so the scan is already sorted by id
        let accounts = self.scan(&ACCOUNTS, |account: &Account| {
            account_type.is_none_or(|t| account.account_type == t)
        })?;
        Ok(pagination.paginate(accounts))
    }

    async fn update_account(&mut self, account: &Account) -> LedgerResult<()> {
        if self.put(&ACCOUNTS, &account.id, account, true)? {
            Ok(())
        } else {
            Err(LedgerError::AccountNotFound(account.id.clone()))
        }
    }

    async fn delete_account(&mut self, account_id: &str) -> LedgerResult<()> {
        if self.remove(&ACCOUNTS, account_id)? {
            Ok(())
        } else {
            Err(LedgerError::AccountNotFound(account_id.to_owned()))
        }
    }
}

#[async_trait]
impl TransactionStore for RedbLedgerStorage {
    async fn save_transaction(&mut self, transaction: &Transaction) -> LedgerResult<()> {
        self.put(&TRANSACTIONS, &transaction.id, transaction, false)?;
        Ok(())
    }

    async fn get_transaction(&self, transaction_id: &str) -> LedgerResult<Option<Transaction>> {
        Ok(self.get(&TRANSACTIONS, transaction_id)?)
    }

    async fn list_transactions(
        &self,
        filter: &TransactionFilter,
        pagination: PaginationOption,
    ) -> LedgerResult<ListResponse<Transaction>> {
        let mut transactions = self.scan(&TRANSACTIONS, |t: &Transaction| filter.matches(t))?;
        // Newest first, then by id so the order is stable
        transactions.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| a.id.cmp(&b.id)));
        Ok(pagination.paginate(transactions))
    }

    async fn update_transaction(&mut self, transaction: &Transaction) -> LedgerResult<()> {
        if self.put(&TRANSACTIONS, &transaction.id, transaction, true)? {
            Ok(())
        } else {
            Err(LedgerError::TransactionNotFound(transaction.id.clone()))
        }
    }

    async fn delete_transaction(&mut self, transaction_id: &str) -> LedgerResult<()> {
        if self.remove(&TRANSACTIONS, transaction_id)? {
            Ok(())
        } else {
            Err(LedgerError::TransactionNotFound(transaction_id.to_owned()))
        }
    }
}
