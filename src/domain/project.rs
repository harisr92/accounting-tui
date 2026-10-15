use std::fmt;

use accounting_core::{Gstin, StateCode};
use bigdecimal::{BigDecimal, RoundingMode, Zero};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::billing::amount_for;
use super::error::require;
use super::{Billing, DomainError, Milestone, MilestoneId, MilestoneStatus, TimeEntry};

/// Lifecycle state of a project, derived from its milestones
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProjectStatus {
    /// Some milestone is still pending, or there are none yet
    Active,
    /// Every milestone is invoiced, some payment is outstanding
    Completed,
    /// Every milestone is paid
    Paid,
}

impl ProjectStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Active => "ACTIVE",
            Self::Completed => "COMPLETED",
            Self::Paid => "PAID",
        }
    }

    fn derive(milestones: &[Milestone]) -> Self {
        let pending = milestones
            .iter()
            .any(|m| m.status() == MilestoneStatus::Pending);
        let all_paid = milestones
            .iter()
            .all(|m| m.status() == MilestoneStatus::Paid);
        match (milestones.is_empty(), pending, all_paid) {
            (true, ..) | (_, true, _) => Self::Active,
            (_, _, true) => Self::Paid,
            _ => Self::Completed,
        }
    }
}

impl fmt::Display for ProjectStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Where a client is, which decides how they are taxed
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientRegion {
    /// In India. `state` is the place of supply for a client without a GSTIN; a registered
    /// client's state comes from their GSTIN.
    India {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        state: Option<StateCode>,
    },
    /// Outside India: services to them are exports
    Abroad { country: String },
}

impl Default for ClientRegion {
    fn default() -> Self {
        Self::India { state: None }
    }
}

impl ClientRegion {
    pub fn is_abroad(&self) -> bool {
        matches!(self, Self::Abroad { .. })
    }
}

/// Everything about a project's client, as entered; validated by [`Project`]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClientDetails {
    pub name: String,
    /// `None` for an unregistered or foreign client
    pub gstin: Option<String>,
    /// Address lines, top to bottom; blank lines are dropped
    pub address: Vec<String>,
    pub region: ClientRegion,
}

impl ClientDetails {
    /// An Indian client known only by name and (optionally) GSTIN
    pub fn named(name: impl Into<String>, gstin: Option<&str>) -> Self {
        Self {
            name: name.into(),
            gstin: gstin.map(str::to_owned),
            ..Self::default()
        }
    }
}

/// Stable project identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectId(Uuid);

impl ProjectId {
    pub fn generate() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl fmt::Display for ProjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Work done for one client, split into milestones
///
/// `total_amount`, `status` and pending hourly milestone amounts are derived: every mutation and
/// every deserialisation recomputes them, so stored values are never trusted. Invoiced amounts
/// are the billed record and stay fixed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "RawProject")]
pub struct Project {
    id: ProjectId,
    name: String,
    client_name: String,
    client_gstin: Option<Gstin>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    client_address: Vec<String>,
    #[serde(default)]
    client_region: ClientRegion,
    billing: Billing,
    milestones: Vec<Milestone>,
    total_amount: BigDecimal,
    status: ProjectStatus,
}

/// Unvalidated shape of a [`Project`]; derived fields are ignored when loading
#[derive(Deserialize)]
struct RawProject {
    id: ProjectId,
    name: String,
    client_name: String,
    client_gstin: Option<Gstin>,
    #[serde(default)]
    client_address: Vec<String>,
    #[serde(default)]
    client_region: ClientRegion,
    billing: Billing,
    #[serde(default)]
    milestones: Vec<Milestone>,
}

impl TryFrom<RawProject> for Project {
    type Error = DomainError;

    fn try_from(raw: RawProject) -> Result<Self, Self::Error> {
        let mut project = Self {
            id: raw.id,
            name: raw.name,
            client_name: raw.client_name,
            client_gstin: raw.client_gstin,
            client_address: raw.client_address,
            client_region: raw.client_region,
            billing: raw.billing,
            milestones: raw.milestones,
            total_amount: BigDecimal::zero(),
            status: ProjectStatus::Active,
        };
        project.validate()?;
        project.refresh();
        Ok(project)
    }
}

/// What list views need about a project, without its milestones
///
/// Stored next to each project and rewritten with it, so lists never load full records.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectSummary {
    id: ProjectId,
    name: Box<str>,
    client_name: Box<str>,
    status: ProjectStatus,
    total_amount: BigDecimal,
}

impl ProjectSummary {
    pub fn id(&self) -> ProjectId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn client_name(&self) -> &str {
        &self.client_name
    }

    pub fn status(&self) -> ProjectStatus {
        self.status
    }

    pub fn total_amount(&self) -> &BigDecimal {
        &self.total_amount
    }
}

impl From<&Project> for ProjectSummary {
    fn from(project: &Project) -> Self {
        Self {
            id: project.id,
            name: project.name.as_str().into(),
            client_name: project.client_name.as_str().into(),
            status: project.status,
            total_amount: project.total_amount.clone(),
        }
    }
}

impl Project {
    /// A new project for an Indian client; `client_gstin` is `None` for an unregistered client
    pub fn new(
        name: impl Into<String>,
        client_name: impl Into<String>,
        client_gstin: Option<&str>,
        billing: Billing,
    ) -> Result<Self, DomainError> {
        Self::for_client(
            name,
            ClientDetails::named(client_name, client_gstin),
            billing,
        )
    }

    /// A new project with no milestones
    pub fn for_client(
        name: impl Into<String>,
        client: ClientDetails,
        billing: Billing,
    ) -> Result<Self, DomainError> {
        let mut project = Self {
            id: ProjectId::generate(),
            name: String::new(),
            client_name: String::new(),
            client_gstin: None,
            client_address: Vec::new(),
            client_region: ClientRegion::default(),
            billing,
            milestones: Vec::new(),
            total_amount: BigDecimal::zero(),
            status: ProjectStatus::Active,
        };
        project.update_details(name, client)?;
        project.validate()?;
        Ok(project)
    }

    pub fn id(&self) -> ProjectId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn client_name(&self) -> &str {
        &self.client_name
    }

    pub fn client_gstin(&self) -> Option<&Gstin> {
        self.client_gstin.as_ref()
    }

    pub fn client_address(&self) -> &[String] {
        &self.client_address
    }

    pub fn client_region(&self) -> &ClientRegion {
        &self.client_region
    }

    /// The client as entered, for prefilling an edit form
    pub fn client_details(&self) -> ClientDetails {
        ClientDetails {
            name: self.client_name.clone(),
            gstin: self.client_gstin.as_ref().map(|g| g.as_str().to_owned()),
            address: self.client_address.clone(),
            region: self.client_region.clone(),
        }
    }

    /// Position of the milestone with this id
    pub fn milestone_index(&self, id: MilestoneId) -> Option<usize> {
        self.milestones.iter().position(|m| m.id() == id)
    }

    pub fn billing(&self) -> &Billing {
        &self.billing
    }

    pub fn milestones(&self) -> &[Milestone] {
        &self.milestones
    }

    pub fn total_amount(&self) -> &BigDecimal {
        &self.total_amount
    }

    pub fn status(&self) -> ProjectStatus {
        self.status
    }

    /// Sum of milestones already paid
    pub fn paid_amount(&self) -> BigDecimal {
        self.milestones
            .iter()
            .filter(|m| m.status() == MilestoneStatus::Paid)
            .map(Milestone::amount)
            .sum()
    }

    /// Amount still to be received
    pub fn outstanding_amount(&self) -> BigDecimal {
        &self.total_amount - self.paid_amount()
    }

    /// Add a pending milestone and return its position
    ///
    /// Fixed projects need an `amount`; hourly projects must pass `None`, since the amount comes
    /// from logged time.
    pub fn add_milestone(
        &mut self,
        name: impl Into<String>,
        due_date: NaiveDate,
        amount: Option<BigDecimal>,
    ) -> Result<usize, DomainError> {
        let name = name.into();
        require(&name, "milestone name")?;
        let amount = self.resolve_amount(amount)?;
        self.milestones.push(Milestone::new(name, due_date, amount));
        self.refresh();
        Ok(self.milestones.len() - 1)
    }

    /// Change a pending milestone; `amount` follows the same rules as [`Self::add_milestone`]
    pub fn edit_milestone(
        &mut self,
        index: usize,
        name: impl Into<String>,
        due_date: NaiveDate,
        amount: Option<BigDecimal>,
    ) -> Result<(), DomainError> {
        let name = name.into();
        require(&name, "milestone name")?;
        let amount = self.resolve_amount(amount)?;
        self.milestone_mut(index)?.edit(name, due_date, amount)?;
        self.refresh();
        Ok(())
    }

    /// Fixed projects need a positive amount (rounded to paise); hourly ones derive it from time
    fn resolve_amount(&self, amount: Option<BigDecimal>) -> Result<BigDecimal, DomainError> {
        match (&self.billing, amount) {
            (Billing::Fixed, Some(amount)) => {
                let amount = amount.with_scale_round(2, RoundingMode::HalfUp);
                if amount <= BigDecimal::zero() {
                    return Err(DomainError::NonPositiveAmount);
                }
                Ok(amount)
            }
            (Billing::Hourly { .. }, None) => Ok(BigDecimal::zero()),
            _ => Err(DomainError::BillingMismatch),
        }
    }

    /// Rename the project or change its client; nothing changes if any value is invalid
    pub fn update_details(
        &mut self,
        name: impl Into<String>,
        client: ClientDetails,
    ) -> Result<(), DomainError> {
        let name = name.into();
        require(&name, "project name")?;
        require(&client.name, "client name")?;
        let gstin = client.gstin.as_deref().map(Gstin::parse).transpose()?;
        validate_region(&client.region, gstin.as_ref())?;
        let address = client
            .address
            .into_iter()
            .map(|line| line.trim().to_owned())
            .filter(|line| !line.is_empty())
            .collect();
        self.name = name;
        self.client_name = client.name;
        self.client_gstin = gstin;
        self.client_address = address;
        self.client_region = client.region;
        Ok(())
    }

    /// Change the hourly rate, or the billing type while there are no milestones
    ///
    /// A new rate re-prices pending milestones only; invoiced amounts stay as billed.
    pub fn set_billing(&mut self, billing: Billing) -> Result<(), DomainError> {
        billing.validate()?;
        let same_type = billing.rate().is_some() == self.billing.rate().is_some();
        if !same_type && !self.milestones.is_empty() {
            return Err(DomainError::BillingMismatch);
        }
        self.billing = billing;
        self.refresh();
        Ok(())
    }

    /// Remove a milestone that has not been invoiced yet
    pub fn remove_milestone(&mut self, index: usize) -> Result<Milestone, DomainError> {
        self.milestone(index)?.ensure_pending()?;
        let removed = self.milestones.remove(index);
        self.refresh();
        Ok(removed)
    }

    /// Log time on a pending milestone of an hourly project
    pub fn log_time(&mut self, index: usize, entry: TimeEntry) -> Result<(), DomainError> {
        if self.billing.rate().is_none() {
            return Err(DomainError::BillingMismatch);
        }
        self.milestone_mut(index)?.log(entry)?;
        self.refresh();
        Ok(())
    }

    /// A project may be deleted only while nothing in it has been invoiced: invoiced and paid
    /// milestones are the records income tax and GST filings are built from
    pub fn ensure_deletable(&self) -> Result<(), DomainError> {
        let billed = self
            .milestones
            .iter()
            .any(|m| m.status() != MilestoneStatus::Pending);
        if billed {
            Err(DomainError::HasBilledMilestones)
        } else {
            Ok(())
        }
    }

    /// Undo the most recent time entry of a pending milestone
    pub fn remove_last_time_entry(&mut self, index: usize) -> Result<TimeEntry, DomainError> {
        let entry = self.milestone_mut(index)?.undo_last_entry()?;
        self.refresh();
        Ok(entry)
    }

    /// Mark a pending milestone as billed by invoice `number`
    pub fn invoice_milestone(
        &mut self,
        index: usize,
        number: impl Into<String>,
    ) -> Result<(), DomainError> {
        self.milestone_mut(index)?.invoice(number.into())?;
        self.refresh();
        Ok(())
    }

    pub fn record_payment(&mut self, index: usize) -> Result<(), DomainError> {
        self.milestone_mut(index)?.advance(MilestoneStatus::Paid)?;
        self.refresh();
        Ok(())
    }

    fn milestone(&self, index: usize) -> Result<&Milestone, DomainError> {
        self.milestones
            .get(index)
            .ok_or(DomainError::MilestoneNotFound(index))
    }

    fn milestone_mut(&mut self, index: usize) -> Result<&mut Milestone, DomainError> {
        self.milestones
            .get_mut(index)
            .ok_or(DomainError::MilestoneNotFound(index))
    }

    /// Recompute every derived field
    ///
    /// Hourly amounts are only recomputed while a milestone is pending: once invoiced, its
    /// amount is what the client was billed and must not follow later rate changes.
    fn refresh(&mut self) {
        if let Some(rate) = self.billing.rate() {
            let pending = self
                .milestones
                .iter_mut()
                .filter(|m| m.status() == MilestoneStatus::Pending);
            for milestone in pending {
                milestone.set_amount(amount_for(rate, milestone.logged_minutes()));
            }
        }
        self.total_amount = self.milestones.iter().map(Milestone::amount).sum();
        self.status = ProjectStatus::derive(&self.milestones);
    }

    fn validate(&self) -> Result<(), DomainError> {
        require(&self.name, "project name")?;
        require(&self.client_name, "client name")?;
        validate_region(&self.client_region, self.client_gstin.as_ref())?;
        self.billing.validate()?;
        for milestone in &self.milestones {
            milestone.validate()?;
            let billed = milestone.status() != MilestoneStatus::Pending;
            if billed && *milestone.amount() <= BigDecimal::zero() {
                return Err(DomainError::NothingToInvoice);
            }
            if self.billing == Billing::Fixed {
                if !milestone.time_entries().is_empty() {
                    return Err(DomainError::BillingMismatch);
                }
                if *milestone.amount() <= BigDecimal::zero() {
                    return Err(DomainError::NonPositiveAmount);
                }
            }
        }
        Ok(())
    }
}

/// A foreign client has a country and no GSTIN
fn validate_region(region: &ClientRegion, gstin: Option<&Gstin>) -> Result<(), DomainError> {
    match region {
        ClientRegion::Abroad { country } => {
            require(country, "client country")?;
            if gstin.is_some() {
                return Err(DomainError::ForeignClientWithGstin);
            }
            Ok(())
        }
        ClientRegion::India { .. } => Ok(()),
    }
}
