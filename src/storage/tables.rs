//! Every table in the database. Values are JSON bytes written by `codec`.

use redb::TableDefinition;
use uuid::Uuid;

pub(super) const SCHEMA_VERSION: u32 = 1;
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
