use bigdecimal::BigDecimal;
use chrono::NaiveDate;

use super::ServiceError;
use super::merge_import::WorkSession;
use crate::domain::{
    Billing, Client, ClientDetails, Project, ProjectId, ProjectSummary, TimeEntry,
};
use crate::storage::{ClientRepository, ProjectRepository};

/// Input for creating or editing a project
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectDraft {
    pub name: String,
    pub client: ClientDetails,
    pub billing: Billing,
}

/// Input for adding or editing a milestone; `amount` is `None` on hourly projects
#[derive(Debug, Clone, PartialEq)]
pub struct MilestoneDraft {
    pub name: String,
    pub due_date: NaiveDate,
    pub amount: Option<BigDecimal>,
}

/// Project use-cases over any [`ProjectRepository`]. Each change is saved before it returns.
/// Entering a project's client also records them in the clients' address book.
pub struct ProjectService<R> {
    repo: R,
}

impl<R: ProjectRepository + ClientRepository> ProjectService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }

    pub fn list(&self) -> Result<Vec<ProjectSummary>, ServiceError> {
        Ok(self.repo.list()?)
    }

    pub fn load(&self, id: ProjectId) -> Result<Project, ServiceError> {
        Ok(self.repo.load(id)?)
    }

    /// Every client billed so far, by name: the choices for a new project's client
    pub fn clients(&self) -> Result<Vec<Client>, ServiceError> {
        Ok(self.repo.clients()?)
    }

    pub fn create(&mut self, draft: ProjectDraft) -> Result<Project, ServiceError> {
        let project = Project::for_client(draft.name, draft.client, draft.billing)?;
        self.save_recording_client(&project)?;
        Ok(project)
    }

    /// Apply the details and the billing of `draft`; nothing is saved if either is rejected.
    /// The client's record is updated only when the client's details changed.
    pub fn update_details(
        &mut self,
        id: ProjectId,
        draft: ProjectDraft,
    ) -> Result<Project, ServiceError> {
        let mut project = self.repo.load(id)?;
        let before = project.client_details();
        project.update_details(draft.name, draft.client)?;
        project.set_billing(draft.billing)?;
        // Only a change to the client is news: saving an older project as it was (a new rate,
        // a new name) must not put its older client details back over newer ones
        if project.client_details().same_as(&before) {
            self.repo.save(&project)?;
        } else {
            self.save_recording_client(&project)?;
        }
        Ok(project)
    }

    /// Delete a project that has nothing invoiced (see [`Project::ensure_deletable`])
    pub fn delete(&mut self, id: ProjectId) -> Result<(), ServiceError> {
        self.repo.load(id)?.ensure_deletable()?;
        Ok(self.repo.delete(id)?)
    }

    /// Hide a finished project from the list (see [`Project::archive`]), or bring it back
    pub fn set_archived(&mut self, id: ProjectId, archived: bool) -> Result<Project, ServiceError> {
        self.update(id, |p| {
            if archived {
                p.archive()
            } else {
                p.unarchive();
                Ok(())
            }
        })
    }

    pub fn add_milestone(
        &mut self,
        id: ProjectId,
        draft: MilestoneDraft,
    ) -> Result<Project, ServiceError> {
        self.update(id, |p| {
            p.add_milestone(draft.name, draft.due_date, draft.amount)
                .map(drop)
        })
    }

    pub fn edit_milestone(
        &mut self,
        id: ProjectId,
        index: usize,
        draft: MilestoneDraft,
    ) -> Result<Project, ServiceError> {
        self.update(id, |p| {
            p.edit_milestone(index, draft.name, draft.due_date, draft.amount)
        })
    }

    pub fn remove_milestone(
        &mut self,
        id: ProjectId,
        index: usize,
    ) -> Result<Project, ServiceError> {
        self.update(id, |p| p.remove_milestone(index).map(drop))
    }

    pub fn log_time(
        &mut self,
        id: ProjectId,
        index: usize,
        entry: TimeEntry,
    ) -> Result<Project, ServiceError> {
        self.update(id, |p| p.log_time(index, entry))
    }

    pub fn remove_last_time_entry(
        &mut self,
        id: ProjectId,
        index: usize,
    ) -> Result<Project, ServiceError> {
        self.update(id, |p| p.remove_last_time_entry(index).map(drop))
    }

    /// The one project called `name`
    pub fn find_by_name(&self, name: &str) -> Result<ProjectId, ServiceError> {
        let mut matches = self.repo.list()?.into_iter().filter(|p| p.name() == name);
        match (matches.next(), matches.next()) {
            (Some(project), None) => Ok(project.id()),
            (None, _) => Err(ServiceError::NoSuchProject(name.to_owned())),
            (Some(_), Some(_)) => Err(ServiceError::AmbiguousProject(name.to_owned())),
        }
    }

    /// Add a milestone with one time entry per session, in one save; sessions whose merge is
    /// already noted on a time entry are skipped, so a re-run imports only new merges.
    /// Returns the project and how many milestones were added.
    pub fn import_sessions(
        &mut self,
        id: ProjectId,
        sessions: &[WorkSession],
    ) -> Result<(Project, usize), ServiceError> {
        let mut added = 0;
        let project = self.update(id, |p| {
            let imported: Vec<String> = p
                .milestones()
                .iter()
                .flat_map(|m| m.time_entries())
                .filter_map(|entry| entry.note().split_whitespace().next())
                .map(str::to_owned)
                .collect();
            for session in sessions {
                if imported.iter().any(|hash| hash == session.hash()) {
                    continue;
                }
                let index = p.add_milestone(session.name.as_str(), session.date, None)?;
                let entry = TimeEntry::new(session.date, session.minutes, session.note.as_str())?;
                p.log_time(index, entry)?;
                added += 1;
            }
            Ok(())
        })?;
        Ok((project, added))
    }

    /// Save `project` with its client recorded: the stored client of that name updated, or a
    /// new one. A stored client its details only tidy (see [`ClientDetails::same_as`]) is
    /// left as it is, so a form with fewer address lines never rewrites theirs.
    fn save_recording_client(&mut self, project: &Project) -> Result<(), ServiceError> {
        let details = project.client_details();
        let known = self
            .repo
            .clients()?
            .into_iter()
            .find(|c| c.matches(&details.name));
        let client = match known {
            Some(client) if client.details().same_as(&details) => {
                return Ok(self.repo.save(project)?);
            }
            Some(mut client) => {
                client.update(details)?;
                client
            }
            None => Client::new(details)?,
        };
        Ok(self.repo.save_with_client(project, &client)?)
    }

    /// Load, change, save. A failed change is never saved, since the edit is on a fresh copy.
    fn update(
        &mut self,
        id: ProjectId,
        change: impl FnOnce(&mut Project) -> Result<(), crate::domain::DomainError>,
    ) -> Result<Project, ServiceError> {
        let mut project = self.repo.load(id)?;
        change(&mut project)?;
        self.repo.save(&project)?;
        Ok(project)
    }
}
