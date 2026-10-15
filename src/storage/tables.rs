//! Every table in the database. Values are JSON bytes written by `codec`.

use redb::TableDefinition;
use uuid::Uuid;

/// 1: first release · 2: invoices record a list of payments instead of a single paid status
pub(super) const SCHEMA_VERSION: u32 = 2;
pub(super) const SCHEMA_KEY: &str = "schema_version";

pub(super) const META: TableDefinition<&str, u32> = TableDefinition::new("meta");

/// Full projects, keyed by `ProjectId`
pub(super) const PROJECTS: TableDefinition<Uuid, &[u8]> = TableDefinition::new("projects");

/// `ProjectSummary` rows, always written in the same transaction as `PROJECTS`
pub(super) const PROJECT_SUMMARIES: TableDefinition<Uuid, &[u8]> =
    TableDefinition::new("project_summaries");

/// accounting-core ledger records, keyed by their string ids
pub(super) type LedgerTable = TableDefinition<'static, &'static str, &'static [u8]>;
pub(super) const ACCOUNTS: LedgerTable = TableDefinition::new("accounts");
pub(super) const TRANSACTIONS: LedgerTable = TableDefinition::new("transactions");

/// Issued invoices, keyed by invoice number
pub(super) const INVOICES: TableDefinition<&str, &[u8]> = TableDefinition::new("invoices");

/// `InvoiceSummary` rows, always written in the same transaction as `INVOICES`
pub(super) const INVOICE_SUMMARIES: TableDefinition<&str, &[u8]> =
    TableDefinition::new("invoice_summaries");

/// Last invoice sequence used in each financial year, keyed by `2026-27`
pub(super) const INVOICE_COUNTERS: TableDefinition<&str, u32> =
    TableDefinition::new("invoice_counters");

/// Business expenses, keyed by `ExpenseId`
pub(super) const EXPENSES: TableDefinition<Uuid, &[u8]> = TableDefinition::new("expenses");

/// Single-row settings, keyed by name (`business_profile`)
pub(super) const SETTINGS: TableDefinition<&str, &[u8]> = TableDefinition::new("settings");
pub(super) const PROFILE_KEY: &str = "business_profile";
