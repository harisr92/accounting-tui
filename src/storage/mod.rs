//! Persistence in a single redb database. Domain validation runs on every load.

mod codec;
mod data_dir;
mod error;
mod ledger;
mod migrations;
mod projects;
mod repository;
mod store;
mod tables;

pub use data_dir::DataDir;
pub use error::StorageError;
pub use ledger::RedbLedgerStorage;
pub use projects::RedbProjectRepository;
pub use repository::ProjectRepository;
pub use store::Store;

#[cfg(test)]
mod tests;
