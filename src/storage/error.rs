use std::io;
use std::path::PathBuf;

use accounting_core::LedgerError;

use super::tables::SCHEMA_VERSION;
use crate::domain::ProjectId;

/// Why reading or writing the database failed
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("no data directory: set COMPTER_HOME or HOME")]
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
    #[error("invalid record {key} in table {table}")]
    Codec {
        table: &'static str,
        key: String,
        #[source]
        source: serde_json::Error,
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
