use std::io;
use std::path::PathBuf;

use accounting_core::LedgerError;

use super::tables::SCHEMA_VERSION;
use crate::domain::ProjectId;

/// Why reading or writing the database failed
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("no data directory: set COMPTER_HOME, or HOME (APPDATA on Windows)")]
    NoDataDir,
    #[error("could not create data directory {path}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("the database is already open in another compter process")]
    AlreadyOpen,
    #[error(
        "database schema version {0} is newer than this compter supports ({SCHEMA_VERSION}); upgrade compter"
    )]
    UnsupportedSchema(u32),
    #[error("project {0} not found")]
    ProjectNotFound(ProjectId),
    #[error("invoice {0} not found")]
    InvoiceNotFound(String),
    #[error("credit note {0} not found")]
    CreditNoteNotFound(String),
    #[error("expense {0} not found")]
    ExpenseNotFound(crate::domain::ExpenseId),
    #[error("invoice or credit note {0} already exists")]
    DuplicateInvoice(String),
    #[error("ledger transaction {0} already exists")]
    DuplicateTransaction(String),
    #[error("another document was numbered first; try again")]
    StaleInvoiceNumber,
    #[error("could not write {path}")]
    WriteFile {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("invalid record {key} in table {table}")]
    Codec {
        table: &'static str,
        key: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("invalid logo {key} in the database")]
    InvalidLogo {
        key: String,
        #[source]
        source: crate::domain::DomainError,
    },
    #[error(transparent)]
    Db(#[from] redb::Error),
}

/// redb has one error type per operation; all of them convert into `redb::Error`
macro_rules! from_redb {
    ($($error:ty),* $(,)?) => {
        $(impl From<$error> for StorageError {
            fn from(error: $error) -> Self {
                Self::Db(error.into())
            }
        })*
    };
}

from_redb!(
    redb::DatabaseError,
    redb::TransactionError,
    redb::TableError,
    redb::StorageError,
    redb::CommitError,
);

impl From<StorageError> for LedgerError {
    fn from(error: StorageError) -> Self {
        Self::Storage(Box::new(error))
    }
}
