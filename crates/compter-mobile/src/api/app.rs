use chrono::Local;
use flutter_rust_bridge::frb;

use compter_core::demo;
use compter_core::domain::{Billing, ClientDetails, ProjectId};
use compter_core::input;
use compter_core::services::{MilestoneDraft, ProjectDraft, ProjectService, ServiceError};
use compter_core::storage::{DataDir, RedbRepository, Store};

use super::{ApiError, MilestoneInput, ProjectDetail, ProjectInput, ProjectRow};

/// The open database and the services over it. Dart opens one at start-up and keeps it for the
/// life of the app; only one can have the database file open.
#[frb(opaque)]
pub struct CompterApp {
    projects: ProjectService<RedbRepository>,
    /// Kept so the database stays open as long as the services use it
    _store: Store,
}

impl CompterApp {
    /// Open (or create) `compter.redb` in `data_dir`, the app's private support folder
    pub fn open(data_dir: String) -> Result<Self, ApiError> {
        let store = Store::open(&DataDir::new(data_dir)).map_err(ServiceError::from)?;
        Ok(Self::over(store))
    }

    /// The sample business of `compter --demo`, in memory; nothing is saved
    pub fn demo() -> Result<Self, ApiError> {
        Ok(Self::over(demo::store()?))
    }

    fn over(store: Store) -> Self {
        Self {
            projects: ProjectService::new(store.repository()),
            _store: store,
        }
    }

    /// The project list, from summaries; archived projects only when asked for
    pub fn projects(&self, include_archived: bool) -> Result<Vec<ProjectRow>, ApiError> {
        Ok(self
            .projects
            .list()?
            .iter()
            .filter(|p| include_archived || !p.is_archived())
            .map(ProjectRow::from)
            .collect())
    }

    pub fn project(&self, id: String) -> Result<ProjectDetail, ApiError> {
        Ok(ProjectDetail::from(&self.projects.load(project_id(&id)?)?))
    }

    pub fn create_project(&mut self, input: ProjectInput) -> Result<ProjectDetail, ApiError> {
        let billing = match input.hourly_rate.as_deref().and_then(input::optional) {
            Some(rate) => {
                let rate = input::money(&rate).map_err(|e| ApiError::invalid("hourly_rate", e))?;
                Billing::hourly(rate)
                    .map_err(|e| ApiError::invalid("hourly_rate", e.to_string()))?
            }
            None => Billing::Fixed,
        };
        // A GSTIN is upper-cased as the terminal does; the domain validates it
        let gstin = input
            .client_gstin
            .as_deref()
            .and_then(input::optional)
            .map(|g| g.to_uppercase());
        let mut client = ClientDetails::named(input.client_name.trim(), None);
        client.gstin = gstin;
        let draft = ProjectDraft {
            name: input.name.trim().to_owned(),
            client,
            billing,
        };
        Ok(ProjectDetail::from(&self.projects.create(draft)?))
    }

    pub fn add_milestone(
        &mut self,
        project_id: String,
        input: MilestoneInput,
    ) -> Result<ProjectDetail, ApiError> {
        let id = self::project_id(&project_id)?;
        let today = Local::now().date_naive();
        let due_date =
            input::date_or(&input.due_date, today).map_err(|e| ApiError::invalid("due_date", e))?;
        let hourly = matches!(self.projects.load(id)?.billing(), Billing::Hourly { .. });
        let amount = match (hourly, input.amount.as_deref()) {
            (true, _) => None,
            (false, amount) => Some(
                input::money(amount.unwrap_or_default())
                    .map_err(|e| ApiError::invalid("amount", e))?,
            ),
        };
        let draft = MilestoneDraft {
            name: input.name.trim().to_owned(),
            due_date,
            amount,
        };
        Ok(ProjectDetail::from(
            &self.projects.add_milestone(id, draft)?,
        ))
    }
}

fn project_id(id: &str) -> Result<ProjectId, ApiError> {
    id.parse()
        .map_err(|_| ApiError::invalid("project_id", "not a project id"))
}
