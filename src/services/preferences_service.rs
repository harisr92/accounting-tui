use super::ServiceError;
use crate::domain::Appearance;
use crate::storage::SettingsRepository;

/// How the app looks, kept with the settings but apart from the business profile
pub struct PreferencesService<R> {
    repo: R,
}

impl<R: SettingsRepository> PreferencesService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
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
}
