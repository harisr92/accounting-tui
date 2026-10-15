use redb::ReadableDatabase;

use crate::storage::tables::{META, SCHEMA_KEY, SCHEMA_VERSION};
use crate::storage::{DataDir, StorageError, Store};

#[test]
fn new_database_records_schema_version() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = DataDir::new(dir.path().join("nested"));
    let store = Store::open(&data_dir).unwrap();
    assert!(data_dir.database_path().exists());

    let txn = store.database().begin_read().unwrap();
    let meta = txn.open_table(META).unwrap();
    let version = meta.get(SCHEMA_KEY).unwrap().unwrap();
    assert_eq!(version.value(), SCHEMA_VERSION);
}

#[test]
fn unknown_schema_version_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = DataDir::new(dir.path());
    {
        let store = Store::open(&data_dir).unwrap();
        let txn = store.database().begin_write().unwrap();
        txn.open_table(META)
            .unwrap()
            .insert(SCHEMA_KEY, SCHEMA_VERSION + 1)
            .unwrap();
        txn.commit().unwrap();
    }
    assert!(matches!(
        Store::open(&data_dir),
        Err(StorageError::UnsupportedSchema(v)) if v == SCHEMA_VERSION + 1
    ));
}

#[test]
fn second_open_of_the_same_file_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = DataDir::new(dir.path());
    let _first = Store::open(&data_dir).unwrap();
    assert!(matches!(
        Store::open(&data_dir),
        Err(StorageError::AlreadyOpen)
    ));
}
