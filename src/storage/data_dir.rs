use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};

use super::StorageError;

const DATABASE_FILE: &str = "compter.redb";

/// Folder that holds the database
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataDir {
    root: PathBuf,
}

impl DataDir {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// `$COMPTER_HOME`, else `$XDG_DATA_HOME/compter`, else `%APPDATA%\compter` on Windows or
    /// `$HOME/.local/share/compter` elsewhere
    pub fn from_env() -> Result<Self, StorageError> {
        Self::resolve(cfg!(windows), |key| env::var_os(key))
    }

    /// [`Self::from_env`] with an injectable platform and variable lookup
    pub fn resolve(
        windows: bool,
        lookup: impl Fn(&str) -> Option<OsString>,
    ) -> Result<Self, StorageError> {
        let var = |key| {
            lookup(key)
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
        };
        if let Some(home) = var("COMPTER_HOME") {
            return Ok(Self::new(home));
        }
        if let Some(data) = var("XDG_DATA_HOME") {
            return Ok(Self::new(data.join("compter")));
        }
        // Windows shells like Git Bash set HOME too; APPDATA keeps one folder whatever the shell
        if windows {
            return var("APPDATA")
                .map(|data| Self::new(data.join("compter")))
                .ok_or(StorageError::NoDataDir);
        }
        var("HOME")
            .map(|home| Self::new(home.join(".local/share/compter")))
            .ok_or(StorageError::NoDataDir)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn database_path(&self) -> PathBuf {
        self.root.join(DATABASE_FILE)
    }

    /// What this device's database was last synced from, see `storage::sync`
    pub fn sync_state_path(&self) -> PathBuf {
        self.root.join("sync.json")
    }

    /// `<root>/compter.conflict-20261017-093000.redb`: the local copy a resolved conflict replaced
    pub fn conflict_backup_path(&self, at: DateTime<Utc>) -> PathBuf {
        self.root.join(format!(
            "compter.conflict-{}.redb",
            at.format("%Y%m%d-%H%M%S")
        ))
    }

    /// `<root>/returns/2026-27/GSTR1-Q2-2026-27.json`
    pub fn return_path(&self, year_label: &str, file_stem: &str) -> PathBuf {
        self.root
            .join("returns")
            .join(year_label)
            .join(format!("{file_stem}.json"))
    }

    /// `<root>/invoices/2026-27/INV-26-27-004.pdf`
    pub fn invoice_pdf_path(&self, year_label: &str, file_stem: &str) -> PathBuf {
        self.root
            .join("invoices")
            .join(year_label)
            .join(format!("{file_stem}.pdf"))
    }
}
