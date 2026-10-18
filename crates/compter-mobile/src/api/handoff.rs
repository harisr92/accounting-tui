//! Rows for handing the database between the phone and the computer's sync folder. The phone
//! can't see that folder, so the user carries a snapshot and its manifest across (see
//! `compter_core::storage::sync::receive`).

use compter_core::storage::sync::SyncState;

/// Where this phone's data stands against the sync folder
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncStatusRow {
    /// `Synced with snapshot 3; changed since`
    pub summary: String,
    /// Something changed here since the last hand-off: worth sending
    pub changed: bool,
    /// The SHA-256 of the snapshot last synced; a manifest recording the same hash needs no
    /// download
    pub sha256: Option<String>,
}

impl From<SyncState> for SyncStatusRow {
    fn from(state: SyncState) -> Self {
        let summary = match (state.generation, state.changed) {
            (0, _) => "Not synced with a computer yet".to_owned(),
            (generation, true) => format!("Synced with snapshot {generation}; changed since"),
            (generation, false) => format!("Synced with snapshot {generation}; no changes since"),
        };
        Self {
            summary,
            changed: state.changed,
            sha256: state.sha256,
        }
    }
}

/// What taking the folder's snapshot did
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceivedRow {
    pub message: String,
    /// Nothing was taken: both sides changed, or what this phone sent never reached the
    /// folder. Ask which to keep, then call `settle_snapshot`.
    pub conflict: bool,
}

/// The snapshot and manifest written for the sync folder
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentRow {
    pub snapshot: String,
    pub manifest: String,
    pub message: String,
}
