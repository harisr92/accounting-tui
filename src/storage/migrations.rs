//! Upgrades an older database to the current `SCHEMA_VERSION`
//!
//! To change a stored format incompatibly (rename or remove a field, change a type):
//! 1. bump `SCHEMA_VERSION` in `tables.rs`
//! 2. append a step here that rewrites the affected rows from the old shape to the new one
//! 3. add a fixture of the old shape under `storage/tests/fixtures/` and a test that upgrades it
//!
//! Adding a field with `#[serde(default)]` needs no migration.

use redb::WriteTransaction;

use super::StorageError;

pub(super) type Migration = fn(&WriteTransaction) -> Result<(), StorageError>;

/// `MIGRATIONS[i]` upgrades schema version `i + 1` to `i + 2`
pub(super) const MIGRATIONS: &[Migration] = &[];

/// Run every step from `from` up to the current version, inside the caller's transaction
pub(super) fn upgrade(txn: &WriteTransaction, from: u32) -> Result<(), StorageError> {
    let start = usize::try_from(from.saturating_sub(1)).unwrap_or(usize::MAX);
    for migration in MIGRATIONS.get(start..).unwrap_or_default() {
        migration(txn)?;
    }
    Ok(())
}
