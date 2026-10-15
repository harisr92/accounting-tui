use crate::storage::migrations::MIGRATIONS;
use crate::storage::tables::SCHEMA_VERSION;

#[test]
fn one_migration_per_version_step() {
    assert_eq!(MIGRATIONS.len() + 1, SCHEMA_VERSION as usize);
}
