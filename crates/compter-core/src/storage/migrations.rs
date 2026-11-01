//! Upgrades an older database to the current `SCHEMA_VERSION`
//!
//! To change a stored format incompatibly (rename or remove a field, change a type):
//! 1. bump `SCHEMA_VERSION` in `tables.rs`
//! 2. append a step here that rewrites the affected rows from the old shape to the new one
//! 3. add a fixture of the old shape under `storage/tests/fixtures/` and a test that upgrades it
//!
//! Adding a field with `#[serde(default)]` needs no migration.

use redb::{ReadableTable, TableHandle, WriteTransaction};
use serde_json::{Value, json};

use super::StorageError;
use super::codec::decode;
use super::redb_repository::{put_client, put_invoice};
use super::tables::{INVOICES, PROJECTS};
use crate::domain::{Client, ClientId, ClientRegion, Invoice, Project, same_client};

pub(super) type Migration = fn(&WriteTransaction) -> Result<(), StorageError>;

/// `MIGRATIONS[i]` upgrades schema version `i + 1` to `i + 2`
pub(super) const MIGRATIONS: &[Migration] = &[
    payments_list,
    credit_notes,
    credit_returns,
    invoice_lines,
    invoice_discounts,
    clients,
];

/// Run every step from `from` up to the current version, inside the caller's transaction
pub(super) fn upgrade(txn: &WriteTransaction, from: u32) -> Result<(), StorageError> {
    let start = usize::try_from(from.saturating_sub(1)).unwrap_or(usize::MAX);
    for migration in MIGRATIONS.get(start..).unwrap_or_default() {
        migration(txn)?;
    }
    Ok(())
}

/// v1 → v2: an invoice's single `status` (issued, or paid in full) becomes a list of payments.
/// Every invoice is rewritten with its summary, which now carries status and balance.
fn payments_list(txn: &WriteTransaction) -> Result<(), StorageError> {
    for (key, bytes) in invoice_rows(txn)? {
        let codec = |source| StorageError::Codec {
            table: INVOICES.name(),
            key: key.clone(),
            source,
        };
        let v1: Value = serde_json::from_slice(&bytes).map_err(codec)?;
        let v2 = serde_json::to_vec(&invoice_v1_to_v2(v1)).map_err(codec)?;
        let invoice: Invoice = decode(INVOICES.name(), &key, &v2)?;
        put_invoice(txn, &invoice)?;
    }
    Ok(())
}

/// v2 → v3: nothing to rewrite. Credit notes added tables (created by `Store::init`), defaulted
/// fields and the CREDITED milestone status. The version moves so an older compter refuses the
/// file instead of reading a credited invoice as still owed in full.
fn credit_notes(_txn: &WriteTransaction) -> Result<(), StorageError> {
    Ok(())
}

/// v3 → v4: nothing to rewrite. Credit notes on paid invoices record the money they give back
/// (`returned`, defaulted). The version moves so an older compter refuses the file instead of
/// reading a refunded invoice's credits as more than its total.
fn credit_returns(_txn: &WriteTransaction) -> Result<(), StorageError> {
    Ok(())
}

/// v4 → v5: an invoice's single `milestone_id` and `milestone_name` become `lines`, so one
/// invoice can bill several milestones. The domain still reads the old shape as one line; every
/// invoice is rewritten in the new one, with its summary, which now lists its milestones.
fn invoice_lines(txn: &WriteTransaction) -> Result<(), StorageError> {
    for (key, bytes) in invoice_rows(txn)? {
        let invoice: Invoice = decode(INVOICES.name(), &key, &bytes)?;
        put_invoice(txn, &invoice)?;
    }
    Ok(())
}

/// v5 → v6: nothing to rewrite. Invoice lines and milestones record a discount (defaulted). The
/// version moves so an older compter refuses the file instead of reading a discounted milestone
/// as billed, and owed, in full.
fn invoice_discounts(_txn: &WriteTransaction) -> Result<(), StorageError> {
    Ok(())
}

/// v6 → v7: a clients table, filled with one client per client name on the projects (names
/// that differ only in case are one client), with the details of the newest project for them.
/// The version moves so an older compter can't add projects whose clients the table misses.
fn clients(txn: &WriteTransaction) -> Result<(), StorageError> {
    // Newest last: project ids are UUIDv7 and sort by when the project was created. Projects
    // from before that have UUIDv4 ids, which carry no time and would sort after every v7 one,
    // so they count as the oldest.
    let mut projects = Vec::new();
    for row in txn.open_table(PROJECTS)?.iter()? {
        let (key, value) = row?;
        let project: Project = decode(PROJECTS.name(), key.value(), value.value())?;
        projects.push(project);
    }
    projects.sort_by_key(|p| (p.id().as_uuid().get_version_num() == 7, p.id()));
    let mut clients: Vec<Client> = Vec::new();
    for project in projects.iter().rev() {
        if clients
            .iter()
            .any(|c| same_client(c.name(), project.client_name()))
        {
            continue;
        }
        // Deterministic, so devices upgrading the same file make the same table
        let id = ClientId::of_project(project.id());
        // A registered client's state is their GSTIN's; one stored against it mustn't stop
        // the upgrade
        let mut details = project.client_details();
        if details.gstin.is_some() && !details.region.is_abroad() {
            details.region = ClientRegion::India { state: None };
        }
        let client =
            Client::with_id(id, details).map_err(|source| StorageError::InvalidClient {
                project: project.id(),
                source,
            })?;
        clients.push(client);
    }
    for client in &clients {
        put_client(txn, client)?;
    }
    Ok(())
}

/// Every invoice row as stored, read out so the table can be written while going through them
fn invoice_rows(txn: &WriteTransaction) -> Result<Vec<(String, Vec<u8>)>, StorageError> {
    let table = txn.open_table(INVOICES)?;
    let mut rows = Vec::new();
    for row in table.iter()? {
        let (key, value) = row?;
        rows.push((key.value().to_owned(), value.value().to_vec()));
    }
    Ok(rows)
}

/// `"status": {"state": "paid", date, received, tds}` → `"payments": [{date, received, tds}]`;
/// an issued invoice simply loses its status
pub(super) fn invoice_v1_to_v2(mut invoice: Value) -> Value {
    let Some(fields) = invoice.as_object_mut() else {
        return invoice;
    };
    if let Some(status) = fields.remove("status")
        && status["state"] == "paid"
    {
        fields.insert(
            "payments".to_owned(),
            json!([{
                "date": status["date"],
                "received": status["received"],
                "tds": status["tds"],
            }]),
        );
    }
    invoice
}
