use std::sync::Arc;

use redb::{Database, ReadableDatabase, ReadableTable, ReadableTableMetadata, TableHandle};

use super::codec::{decode, encode};
use super::tables::{PROJECT_SUMMARIES, PROJECTS};
use super::{ProjectRepository, StorageError};
use crate::domain::{Project, ProjectId, ProjectSummary};

/// [`ProjectRepository`] on redb: full records in `projects`, list rows in `project_summaries`
pub struct RedbProjectRepository {
    db: Arc<Database>,
}

impl RedbProjectRepository {
    pub(super) fn new(db: Arc<Database>) -> Self {
        Self { db }
    }
}

impl ProjectRepository for RedbProjectRepository {
    fn list(&self) -> Result<Vec<ProjectSummary>, StorageError> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(PROJECT_SUMMARIES)?;
        let mut summaries = Vec::with_capacity(usize::try_from(table.len()?).unwrap_or_default());
        for row in table.iter()? {
            let (key, value) = row?;
            summaries.push(decode(
                PROJECT_SUMMARIES.name(),
                key.value(),
                value.value(),
            )?);
        }
        summaries.sort_by(|a: &ProjectSummary, b| a.name().cmp(b.name()).then(a.id().cmp(&b.id())));
        Ok(summaries)
    }

    fn load(&self, id: ProjectId) -> Result<Project, StorageError> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(PROJECTS)?;
        let row = table
            .get(id.as_uuid())?
            .ok_or(StorageError::ProjectNotFound(id))?;
        decode(PROJECTS.name(), id, row.value())
    }

    fn save(&mut self, project: &Project) -> Result<(), StorageError> {
        let id = project.id();
        let record = encode(PROJECTS.name(), id, project)?;
        let summary = encode(PROJECT_SUMMARIES.name(), id, &ProjectSummary::from(project))?;

        let txn = self.db.begin_write()?;
        {
            txn.open_table(PROJECTS)?
                .insert(id.as_uuid(), record.as_slice())?;
            txn.open_table(PROJECT_SUMMARIES)?
                .insert(id.as_uuid(), summary.as_slice())?;
        }
        txn.commit()?;
        Ok(())
    }

    fn delete(&mut self, id: ProjectId) -> Result<(), StorageError> {
        let txn = self.db.begin_write()?;
        {
            let removed = txn.open_table(PROJECTS)?.remove(id.as_uuid())?.is_some();
            if !removed {
                // Dropping the transaction aborts it
                return Err(StorageError::ProjectNotFound(id));
            }
            txn.open_table(PROJECT_SUMMARIES)?.remove(id.as_uuid())?;
        }
        txn.commit()?;
        Ok(())
    }
}
