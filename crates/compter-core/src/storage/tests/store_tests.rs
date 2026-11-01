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
    const RETURNED_V4: &str = include_str!("fixtures/invoice_returned_v4.json");
    for (from, number, json) in [
        (2, "NDC/26-27/004", PARTLY_PAID_V2),
        (3, "INV/26-27/001", CREDITED_V3),
        (4, "INV/26-27/001", RETURNED_V4),
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
        assert_eq!(version, SCHEMA_VERSION);
        // v5 rewrote the invoice's single milestone as its one line
        let stored: serde_json::Value = serde_json::from_slice(
            txn.open_table(INVOICES)
                .unwrap()
                .get(number)
                .unwrap()
                .unwrap()
                .value(),
        )
        .unwrap();
        assert!(stored.get("milestone_id").is_none());
        assert_eq!(stored["lines"].as_array().map(Vec::len), Some(1));
        drop(txn);
        let invoice = store
            .repository()
            .load_invoice(&InvoiceNumber::from_stored(number))
            .unwrap();
        assert_eq!(invoice.lines().len(), 1);
        assert_eq!(
            invoice.lines()[0].taxable_value,
            invoice.amounts().taxable_value
        );
        if from < 4 {
            assert_eq!(invoice.returned(), bigdecimal::BigDecimal::from(0));
        }
    }
}

#[test]
fn a_v6_database_gets_a_client_for_each_client_name_with_the_newest_details() {
    use crate::domain::{Billing, ClientDetails, Project};
    use crate::storage::{ClientRepository, ProjectRepository};

    let dir = tempfile::tempdir().unwrap();
    let data_dir = DataDir::new(dir.path());
    let with_address = |client: &str, line: &str| {
        let details = ClientDetails {
            address: vec![line.to_owned()],
            ..ClientDetails::named(client, None)
        };
        Project::for_client("Site", details, Billing::Fixed).unwrap()
    };
    {
        // A v6 database: projects saved without clients
        let store = Store::open(&data_dir).unwrap();
        let mut repo = store.repository();
        for project in [
            with_address("Acme", "Old street"),
            with_address("Beta", "Beta street"),
            with_address("ACME ", "New street"),
        ] {
            repo.save(&project).unwrap();
        }
        let txn = store.database().begin_write().unwrap();
        txn.open_table(META).unwrap().insert(SCHEMA_KEY, 6).unwrap();
        txn.commit().unwrap();
    }

    let store = Store::open(&data_dir).unwrap();
    let clients = store.repository().clients().unwrap();

    let found: Vec<(&str, &[String])> = clients.iter().map(|c| (c.name(), c.address())).collect();
    assert_eq!(
        found,
        [
            ("ACME", ["New street".to_owned()].as_slice()),
            ("Beta", ["Beta street".to_owned()].as_slice()),
        ]
    );
}

#[test]
fn a_v6_upgrade_counts_projects_from_before_v7_ids_as_the_oldest() {
    use crate::domain::{Billing, ClientDetails, Project};
    use crate::storage::codec::decode;
    use crate::storage::{ClientRepository, ProjectRepository};

    // Shipped with a UUIDv4 id, which sorts after every UUIDv7 one
    let old: Project = decode(
        "projects",
        "fixture",
        include_str!("fixtures/project_fixed_v1.json").as_bytes(),
    )
    .unwrap();
    assert_eq!(old.id().as_uuid().get_version_num(), 4);
    let newer = Project::for_client(
        "Follow-up",
        ClientDetails {
            address: vec!["New street".to_owned()],
            ..old.client_details()
        },
        Billing::Fixed,
    )
    .unwrap();

    let dir = tempfile::tempdir().unwrap();
    let data_dir = DataDir::new(dir.path());
    {
        let store = Store::open(&data_dir).unwrap();
        let mut repo = store.repository();
        repo.save(&old).unwrap();
        repo.save(&newer).unwrap();
        let txn = store.database().begin_write().unwrap();
        txn.open_table(META).unwrap().insert(SCHEMA_KEY, 6).unwrap();
        txn.commit().unwrap();
    }

    let clients = Store::open(&data_dir)
        .unwrap()
        .repository()
        .clients()
        .unwrap();
    assert_eq!(clients.len(), 1);
    assert_eq!(clients[0].address(), ["New street"]);
}

#[test]
fn a_v6_upgrade_names_clients_after_their_newest_project_and_drops_a_state_against_the_gstin() {
    use crate::domain::{Billing, ClientDetails, ClientId, ClientRegion, Project};
    use crate::storage::tables::PROJECTS;
    use crate::storage::{ClientRepository, ProjectRepository};

    let dir = tempfile::tempdir().unwrap();
    let data_dir = DataDir::new(dir.path());
    let project = Project::for_client(
        "Site",
        ClientDetails::named("Acme", Some("27AAPFU0939F2ZU")),
        Billing::Fixed,
    )
    .unwrap();
    {
        let store = Store::open(&data_dir).unwrap();
        store.repository().save(&project).unwrap();
        // Stored by an older build, with a state that isn't the GSTIN's
        let mut json = serde_json::to_value(&project).unwrap();
        json["client_region"] = serde_json::json!({"type": "india", "state": "29"});
        let txn = store.database().begin_write().unwrap();
        txn.open_table(PROJECTS)
            .unwrap()
            .insert(
                project.id().as_uuid(),
                serde_json::to_vec(&json).unwrap().as_slice(),
            )
            .unwrap();
        txn.open_table(META).unwrap().insert(SCHEMA_KEY, 6).unwrap();
        txn.commit().unwrap();
    }

    let store = Store::open(&data_dir).unwrap();
    let clients = store.repository().clients().unwrap();

    assert_eq!(clients.len(), 1);
    assert_eq!(clients[0].id(), ClientId::of_project(project.id()));
    assert_eq!(clients[0].region(), &ClientRegion::India { state: None });
}
