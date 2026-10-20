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
use super::redb_repository::put_invoice;
use super::tables::INVOICES;
use crate::domain::Invoice;

pub(super) type Migration = fn(&WriteTransaction) -> Result<(), StorageError>;

/// `MIGRATIONS[i]` upgrades schema version `i + 1` to `i + 2`
pub(super) const MIGRATIONS: &[Migration] = &[
    payments_list,
    credit_notes,
    credit_returns,
    invoice_lines,
    invoice_discounts,
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
