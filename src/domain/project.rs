use std::fmt;

use accounting_core::Gstin;
use bigdecimal::{BigDecimal, RoundingMode, Zero};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::billing::amount_for;
use super::error::require;
use super::{Billing, DomainError, Milestone, MilestoneStatus, TimeEntry};

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
    /// A new project with no milestones; `client_gstin` is `None` for an unregistered client
    pub fn new(
        name: impl Into<String>,
        client_name: impl Into<String>,
        client_gstin: Option<&str>,
        billing: Billing,
    ) -> Result<Self, DomainError> {
        let project = Self {
            id: ProjectId::generate(),
            name: name.into(),
            client_name: client_name.into(),
            client_gstin: client_gstin.map(Gstin::parse).transpose()?,
            billing,
            milestones: Vec::new(),
            total_amount: BigDecimal::zero(),
            status: ProjectStatus::Active,
        };
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
        client_name: impl Into<String>,
        client_gstin: Option<&str>,
    ) -> Result<(), DomainError> {
        let (name, client_name) = (name.into(), client_name.into());
        require(&name, "project name")?;
        require(&client_name, "client name")?;
        let client_gstin = client_gstin.map(Gstin::parse).transpose()?;
        self.name = name;
        self.client_name = client_name;
        self.client_gstin = client_gstin;
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

    pub fn invoice_milestone(&mut self, index: usize) -> Result<(), DomainError> {
        self.advance(index, MilestoneStatus::Invoiced)
    }

    pub fn record_payment(&mut self, index: usize) -> Result<(), DomainError> {
        self.advance(index, MilestoneStatus::Paid)
    }

    fn advance(&mut self, index: usize, to: MilestoneStatus) -> Result<(), DomainError> {
        self.milestone_mut(index)?.advance(to)?;
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
