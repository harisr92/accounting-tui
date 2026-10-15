//! Persistence in a single redb database. Domain validation runs on every load.

mod codec;
mod data_dir;
mod error;
mod files;
mod ledger;
mod migrations;
mod redb_repository;
mod repository;
mod store;
mod tables;

pub use data_dir::DataDir;
pub use error::StorageError;
pub use files::write_file;
pub use ledger::RedbLedgerStorage;
pub use redb_repository::RedbRepository;
pub use repository::{
    ExpenseRecord, ExpenseRepository, InvoiceRepository, IssueRecord, PaymentRecord,
    ProjectRepository, Repository, SettingsRepository,
};
pub use store::Store;

#[cfg(test)]
mod tests;
