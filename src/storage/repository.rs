use super::StorageError;
use crate::domain::{Project, ProjectId, ProjectSummary};

/// Where projects live. The app depends on this trait, not on redb.
pub trait ProjectRepository {
    /// Every project's summary, sorted by name
    fn list(&self) -> Result<Vec<ProjectSummary>, StorageError>;

    fn load(&self, id: ProjectId) -> Result<Project, StorageError>;

    /// Insert or replace a project and its summary
    fn save(&mut self, project: &Project) -> Result<(), StorageError>;

    fn delete(&mut self, id: ProjectId) -> Result<(), StorageError>;
}
