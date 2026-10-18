use std::fs;
use std::sync::Arc;

use redb::backends::InMemoryBackend;
use redb::{
    Database, DatabaseError, Key, ReadTransaction, ReadableDatabase, ReadableTable,
    ReadableTableMetadata, TableDefinition, TableError, TableHandle, Value,
};
use sha2::{Digest, Sha256};

use super::migrations;
use super::tables::{
    ACCOUNTS, CREDIT_NOTE_COUNTERS, CREDIT_NOTES, EXPENSES, INVOICE_COUNTERS, INVOICE_SUMMARIES,
    INVOICES, LOGOS, META, PROJECT_SUMMARIES, PROJECTS, SCHEMA_KEY, SCHEMA_VERSION, SETTINGS,
    TRANSACTIONS,
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

    /// No records yet: no projects, invoices, credit notes, expenses, postings or settings. A
    /// new install's database, which replacing loses nothing.
    pub fn is_blank(&self) -> Result<bool, StorageError> {
        let txn = self.db.begin_read()?;
        Ok(txn.open_table(PROJECTS)?.is_empty()?
            && txn.open_table(INVOICES)?.is_empty()?
            && txn.open_table(CREDIT_NOTES)?.is_empty()?
            && txn.open_table(EXPENSES)?.is_empty()?
            && txn.open_table(TRANSACTIONS)?.is_empty()?
            && txn.open_table(SETTINGS)?.is_empty()?)
    }

    /// A hash of every record, as hex. Unlike a hash of the file, it ignores redb's own
    /// bookkeeping, which changes whenever the file is opened, so it changes only with the data.
    pub fn content_hash(&self) -> Result<String, StorageError> {
        let txn = self.db.begin_read()?;
        let mut hasher = Sha256::new();
        hash_table(&txn, META, &mut hasher)?;
        hash_table(&txn, PROJECTS, &mut hasher)?;
        hash_table(&txn, PROJECT_SUMMARIES, &mut hasher)?;
        hash_table(&txn, ACCOUNTS, &mut hasher)?;
        hash_table(&txn, TRANSACTIONS, &mut hasher)?;
        hash_table(&txn, INVOICES, &mut hasher)?;
        hash_table(&txn, INVOICE_SUMMARIES, &mut hasher)?;
        hash_table(&txn, INVOICE_COUNTERS, &mut hasher)?;
        hash_table(&txn, CREDIT_NOTES, &mut hasher)?;
        hash_table(&txn, CREDIT_NOTE_COUNTERS, &mut hasher)?;
        hash_table(&txn, SETTINGS, &mut hasher)?;
        hash_table(&txn, EXPENSES, &mut hasher)?;
        hash_table(&txn, LOGOS, &mut hasher)?;
        Ok(hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect())
    }

    /// Close the file, so it can be copied. Fails while a repository still shares it.
    pub fn close(self) -> Result<(), StorageError> {
        Arc::try_unwrap(self.db)
            .map(drop)
            .map_err(|_| StorageError::StillInUse)
    }

    #[cfg(test)]
    pub(super) fn database(&self) -> &Database {
        &self.db
    }

    /// Create missing tables, then record, upgrade or refuse the schema version. A database that
    /// is already current is only read, with no write transaction.
    fn init(db: Database) -> Result<Self, StorageError> {
        if Self::is_current(&db)? {
            return Ok(Self { db: Arc::new(db) });
        }
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
            txn.open_table(CREDIT_NOTES)?;
            txn.open_table(CREDIT_NOTE_COUNTERS)?;
            txn.open_table(SETTINGS)?;
            txn.open_table(EXPENSES)?;
            txn.open_table(LOGOS)?;
        }
        txn.commit()?;
        Ok(Self { db: Arc::new(db) })
    }

    /// At this schema version with every table in place
    fn is_current(db: &Database) -> Result<bool, StorageError> {
        let txn = db.begin_read()?;
        let version = match txn.open_table(META) {
            Ok(meta) => meta.get(SCHEMA_KEY)?.map(|v| v.value()),
            Err(TableError::TableDoesNotExist(_)) => return Ok(false),
            Err(error) => return Err(error.into()),
        };
        if version != Some(SCHEMA_VERSION) {
            return Ok(false);
        }
        let tables = txn
            .list_tables()?
            .map(|table| table.name().to_owned())
            .collect::<Vec<_>>();
        let expected = [
            PROJECTS.name(),
            PROJECT_SUMMARIES.name(),
            ACCOUNTS.name(),
            TRANSACTIONS.name(),
            INVOICES.name(),
            INVOICE_SUMMARIES.name(),
            INVOICE_COUNTERS.name(),
            CREDIT_NOTES.name(),
            CREDIT_NOTE_COUNTERS.name(),
            SETTINGS.name(),
            EXPENSES.name(),
            LOGOS.name(),
        ];
        Ok(expected.iter().all(|name| tables.iter().any(|t| t == name)))
    }
}

/// Feed a table's name and every key and value into `hasher`, each length-prefixed so that
/// different records can't run together into the same bytes
fn hash_table<K: Key + 'static, V: Value + 'static>(
    txn: &ReadTransaction,
    table: TableDefinition<K, V>,
    hasher: &mut Sha256,
) -> Result<(), StorageError> {
    let mut feed = |bytes: &[u8]| {
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    };
    feed(table.name().as_bytes());
    for entry in txn.open_table(table)?.iter()? {
        let (key, value) = entry?;
        feed(K::as_bytes(&key.value()).as_ref());
        feed(V::as_bytes(&value.value()).as_ref());
    }
    Ok(())
}
