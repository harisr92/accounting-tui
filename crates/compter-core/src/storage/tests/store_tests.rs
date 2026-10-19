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

#[test]
fn a_v1_database_is_upgraded_on_open() {
    use crate::domain::InvoiceNumber;
    use crate::storage::InvoiceRepository;
    use crate::storage::tables::{INVOICE_SUMMARIES, INVOICES};

    const PAID_V1: &str = include_str!("fixtures/invoice_plain_v1.json");
    let dir = tempfile::tempdir().unwrap();
    let data_dir = DataDir::new(dir.path());
    {
        // Make it look like a v1 database holding one paid v1 invoice
        let store = Store::open(&data_dir).unwrap();
        let txn = store.database().begin_write().unwrap();
        txn.open_table(META).unwrap().insert(SCHEMA_KEY, 1).unwrap();
        txn.open_table(INVOICES)
            .unwrap()
            .insert("NDC/25-26/001", PAID_V1.as_bytes())
            .unwrap();
        txn.open_table(INVOICE_SUMMARIES)
            .unwrap()
            .insert("NDC/25-26/001", br#"{"stale": true}"#.as_slice())
            .unwrap();
        txn.commit().unwrap();
    }

    let store = Store::open(&data_dir).unwrap();
    let txn = store.database().begin_read().unwrap();
    let version = txn
        .open_table(META)
        .unwrap()
        .get(SCHEMA_KEY)
        .unwrap()
        .unwrap()
        .value();
    assert_eq!(version, SCHEMA_VERSION);
    drop(txn);

    let repo = store.repository();
    let invoice = repo
        .load_invoice(&InvoiceNumber::from_stored("NDC/25-26/001"))
        .unwrap();
    assert!(invoice.is_paid());
    assert_eq!(invoice.payments().len(), 1);
    let summaries = repo.list_invoices().unwrap();
    assert!(summaries[0].is_paid());
}

#[test]
fn a_v2_database_moves_to_the_current_version_with_its_records_unchanged() {
    use crate::domain::InvoiceNumber;
    use crate::storage::InvoiceRepository;
    use crate::storage::tables::INVOICES;

    const PARTLY_PAID_V2: &str = include_str!("fixtures/invoice_partly_paid_v2.json");
    const CREDITED_V3: &str = include_str!("fixtures/invoice_credited_v3.json");
    for (from, number, json) in [
        (2, "NDC/26-27/004", PARTLY_PAID_V2),
        (3, "INV/26-27/001", CREDITED_V3),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let data_dir = DataDir::new(dir.path());
        {
            let store = Store::open(&data_dir).unwrap();
            let txn = store.database().begin_write().unwrap();
            txn.open_table(META)
                .unwrap()
                .insert(SCHEMA_KEY, from)
                .unwrap();
            txn.open_table(INVOICES)
                .unwrap()
                .insert(number, json.as_bytes())
                .unwrap();
            txn.commit().unwrap();
        }

        let store = Store::open(&data_dir).unwrap();
        let txn = store.database().begin_read().unwrap();
        let version = txn
            .open_table(META)
            .unwrap()
            .get(SCHEMA_KEY)
            .unwrap()
            .unwrap()
            .value();
        assert_eq!(version, 4);
        drop(txn);
        let invoice = store
            .repository()
            .load_invoice(&InvoiceNumber::from_stored(number))
            .unwrap();
        assert_eq!(invoice.returned(), bigdecimal::BigDecimal::from(0));
    }
}
