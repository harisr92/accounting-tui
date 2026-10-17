use std::path::Path;

use super::ServiceError;
use crate::domain::Appearance;
use crate::storage::sync::{SyncDir, SyncSource};
use crate::storage::{DataDir, SettingsRepository};

/// How the app looks and how this device syncs, kept apart from the business profile
pub struct PreferencesService<R> {
    repo: R,
    /// Where this device's own settings live (none in demo mode)
    data_dir: Option<DataDir>,
    /// `$COMPTER_SYNC_DIR`, read by the caller so the service never looks at the environment
    sync_override: Option<SyncDir>,
}

impl<R: SettingsRepository> PreferencesService<R> {
    pub fn new(repo: R, data_dir: Option<DataDir>) -> Self {
        Self {
            repo,
            data_dir,
            sync_override: None,
        }
    }

    /// A sync folder that wins over the one saved in Settings
    pub fn with_sync_override(mut self, dir: Option<SyncDir>) -> Self {
        self.sync_override = dir;
        self
    }

    pub fn appearance(&self) -> Result<Appearance, ServiceError> {
        Ok(self.repo.load_appearance()?)
    }

    /// Draw the app in the next theme from now on, and return it
    pub fn next_appearance(&mut self) -> Result<Appearance, ServiceError> {
        let next = self.repo.load_appearance()?.next();
        self.repo.save_appearance(next)?;
        Ok(next)
    }

    /// The folder this device syncs through on its next start, and where that was set
    pub fn sync_folder(&self) -> Result<Option<(SyncDir, SyncSource)>, ServiceError> {
        let Some(data) = &self.data_dir else {
            return Ok(None);
        };
        Ok(SyncDir::configured(data, self.sync_override.clone())?)
    }

    /// Sync this device through `dir` from its next start, or stop when `None`
    pub fn set_sync_folder(&mut self, dir: Option<&Path>) -> Result<(), ServiceError> {
        let data = self.data_dir.as_ref().ok_or(ServiceError::NoSyncInDemo)?;
        Ok(SyncDir::save(data, dir)?)
    }
}
