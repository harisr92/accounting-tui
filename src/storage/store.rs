use std::fs;
use std::sync::Arc;

use redb::backends::InMemoryBackend;
use redb::{Database, DatabaseError, ReadableTable};

use super::migrations;
use super::tables::{
    ACCOUNTS, EXPENSES, INVOICE_COUNTERS, INVOICE_SUMMARIES, INVOICES, META, PROJECT_SUMMARIES,
    PROJECTS, SCHEMA_KEY, SCHEMA_VERSION, SETTINGS, TRANSACTIONS,
};
use super::{DataDir, RedbLedgerStorage, RedbRepository, StorageError};

/// The open database; hands out repositories that share it
#[derive(Clone)]
pub struct Store {
    db: Arc<Database>,
}

impl Store {
    /// Open or create `compter.redb` in `dir`
    pub fn open(dir: &DataDir) -> Result<Self, StorageError> {
        fs::create_dir_all(dir.root()).map_err(|source| StorageError::Io {
            path: dir.root().to_path_buf(),
            source,
        })?;
        let db = Database::create(dir.database_path()).map_err(|error| match error {
            DatabaseError::DatabaseAlreadyOpen => StorageError::AlreadyOpen,
            error => error.into(),
        })?;
        Self::init(db)
    }

    /// A fresh database that lives only as long as this `Store`
    pub fn in_memory() -> Result<Self, StorageError> {
        let db = Database::builder().create_with_backend(InMemoryBackend::new())?;
        Self::init(db)
    }

    /// Projects, invoices and settings, sharing this database
    pub fn repository(&self) -> RedbRepository {
        RedbRepository::new(Arc::clone(&self.db))
    }

    pub fn ledger(&self) -> RedbLedgerStorage {
        RedbLedgerStorage::new(Arc::clone(&self.db))
    }

    #[cfg(test)]
    pub(super) fn database(&self) -> &Database {
        &self.db
    }

    /// Create missing tables, then record, upgrade or refuse the schema version
    fn init(db: Database) -> Result<Self, StorageError> {
        let txn = db.begin_write()?;
        {
            let mut meta = txn.open_table(META)?;
            let version = meta.get(SCHEMA_KEY)?.map(|v| v.value());
            match version {
                None => {}
                Some(SCHEMA_VERSION) => {}
                // Older database: upgrade in this same transaction, so a failure changes nothing
                Some(old) if (1..SCHEMA_VERSION).contains(&old) => migrations::upgrade(&txn, old)?,
                // Written by a newer compter (or garbage): never guess
                Some(other) => return Err(StorageError::UnsupportedSchema(other)),
            }
            if version != Some(SCHEMA_VERSION) {
                meta.insert(SCHEMA_KEY, SCHEMA_VERSION)?;
            }
            txn.open_table(PROJECTS)?;
            txn.open_table(PROJECT_SUMMARIES)?;
            txn.open_table(ACCOUNTS)?;
            txn.open_table(TRANSACTIONS)?;
            txn.open_table(INVOICES)?;
            txn.open_table(INVOICE_SUMMARIES)?;
            txn.open_table(INVOICE_COUNTERS)?;
            txn.open_table(SETTINGS)?;
            txn.open_table(EXPENSES)?;
        }
        txn.commit()?;
        Ok(Self { db: Arc::new(db) })
    }
}
