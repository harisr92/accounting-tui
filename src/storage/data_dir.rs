use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

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

    /// `$COMPTER_HOME`, else `$XDG_DATA_HOME/compter`, else `$HOME/.local/share/compter`
    pub fn from_env() -> Result<Self, StorageError> {
        Self::resolve(|key| env::var_os(key))
    }

    /// [`Self::from_env`] with an injectable variable lookup
    pub fn resolve(lookup: impl Fn(&str) -> Option<OsString>) -> Result<Self, StorageError> {
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

    /// `<root>/invoices/2026-27/INV-26-27-004.pdf`
    pub fn invoice_pdf_path(&self, year_label: &str, file_stem: &str) -> PathBuf {
        self.root
            .join("invoices")
            .join(year_label)
            .join(format!("{file_stem}.pdf"))
    }
}
