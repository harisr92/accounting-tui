use std::fmt;
use std::str::FromStr;

use bigdecimal::{BigDecimal, Zero};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::error::require;
use super::{DomainError, TimeEntry};

/// Billing state of a milestone
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MilestoneStatus {
    Pending,
    Invoiced,
    Paid,
    /// Its invoice was cancelled in full by credit notes, with nothing paid
    Credited,
}

impl MilestoneStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::Invoiced => "INVOICED",
            Self::Paid => "PAID",
            Self::Credited => "CREDITED",
        }
    }

    /// Nothing more will be billed or received for it
    pub const fn is_settled(self) -> bool {
        matches!(self, Self::Paid | Self::Credited)
    }
}

impl fmt::Display for MilestoneStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Stable milestone identifier; invoices refer to milestones by id, never by position
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MilestoneId(Uuid);

impl MilestoneId {
    pub fn generate() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl fmt::Display for MilestoneId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// The [`Display`](fmt::Display) form back, for front ends that hand ids around as text
impl FromStr for MilestoneId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(Self)
    }
}

/// A billable unit of work inside a project. Changed only through [`super::Project`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "RawMilestone")]
pub struct Milestone {
    id: MilestoneId,
    name: String,
    amount: BigDecimal,
    due_date: NaiveDate,
    status: MilestoneStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    time_entries: Vec<TimeEntry>,
    /// Set when the milestone is invoiced; absent on records from before invoicing existed
    #[serde(default, skip_serializing_if = "Option::is_none")]
    invoice_number: Option<String>,
    /// Taken back by credit notes on its invoice, before GST: part of `amount` that will never be
    /// received
    #[serde(default, skip_serializing_if = "Zero::is_zero")]
    credited: BigDecimal,
    /// Taken off `amount` by the invoice that billed it, before GST: its share of the
    /// invoice's discount
    #[serde(default, skip_serializing_if = "Zero::is_zero")]
    discount: BigDecimal,
}

/// Unvalidated shape of a [`Milestone`]; project-level rules are checked by [`super::Project`]
#[derive(Deserialize)]
struct RawMilestone {
    /// Records saved before ids existed get a fresh one, kept from their next save on
    #[serde(default = "MilestoneId::generate")]
    id: MilestoneId,
    name: String,
    amount: BigDecimal,
    due_date: NaiveDate,
    status: MilestoneStatus,
    #[serde(default)]
    time_entries: Vec<TimeEntry>,
    #[serde(default)]
    invoice_number: Option<String>,
    #[serde(default)]
    credited: BigDecimal,
    #[serde(default)]
    discount: BigDecimal,
}

impl TryFrom<RawMilestone> for Milestone {
    type Error = DomainError;

    fn try_from(raw: RawMilestone) -> Result<Self, Self::Error> {
        let milestone = Self {
            id: raw.id,
            name: raw.name,
            amount: raw.amount,
            due_date: raw.due_date,
            status: raw.status,
            time_entries: raw.time_entries,
            invoice_number: raw.invoice_number,
            credited: raw.credited,
            discount: raw.discount,
        };
        milestone.validate()?;
        Ok(milestone)
    }
}

impl Milestone {
    pub(super) fn new(name: String, due_date: NaiveDate, amount: BigDecimal) -> Self {
        Self {
            id: MilestoneId::generate(),
            name,
            amount,
            due_date,
            status: MilestoneStatus::Pending,
            time_entries: Vec::new(),
            invoice_number: None,
            credited: BigDecimal::zero(),
            discount: BigDecimal::zero(),
        }
    }

    pub fn id(&self) -> MilestoneId {
        self.id
    }

    pub fn invoice_number(&self) -> Option<&str> {
        self.invoice_number.as_deref()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn amount(&self) -> &BigDecimal {
        &self.amount
    }

    pub fn due_date(&self) -> NaiveDate {
        self.due_date
    }

    pub fn status(&self) -> MilestoneStatus {
        self.status
    }

    /// Taken back by credit notes, before GST
    pub fn credited(&self) -> &BigDecimal {
        &self.credited
    }

    /// Taken off by its invoice's discount, before GST
    pub fn discount(&self) -> &BigDecimal {
        &self.discount
    }

    /// What its invoice billed for it, before GST: `amount` less `discount`
    pub fn billed(&self) -> BigDecimal {
        &self.amount - &self.discount
    }

    /// What was billed and not taken back: `billed` less `credited`
    pub fn net_amount(&self) -> BigDecimal {
        self.billed() - &self.credited
    }

    pub fn time_entries(&self) -> &[TimeEntry] {
        &self.time_entries
    }

    pub fn logged_minutes(&self) -> u32 {
        self.time_entries
            .iter()
            .fold(0, |total, entry| total.saturating_add(entry.minutes()))
    }

    pub(super) fn ensure_pending(&self) -> Result<(), DomainError> {
        if self.status == MilestoneStatus::Pending {
            Ok(())
        } else {
            Err(DomainError::MilestoneLocked)
        }
    }

    pub(super) fn log(&mut self, entry: TimeEntry) -> Result<(), DomainError> {
        self.ensure_pending()?;
        self.time_entries.push(entry);
        Ok(())
    }

    /// Replace the details of a pending milestone
    pub(super) fn edit(
        &mut self,
        name: String,
        due_date: NaiveDate,
        amount: BigDecimal,
    ) -> Result<(), DomainError> {
        self.ensure_pending()?;
        self.name = name;
        self.due_date = due_date;
        self.amount = amount;
        Ok(())
    }

    /// Take back the most recent time entry of a pending milestone
    pub(super) fn undo_last_entry(&mut self) -> Result<TimeEntry, DomainError> {
        self.ensure_pending()?;
        self.time_entries.pop().ok_or(DomainError::NoTimeEntries)
    }

    pub(super) fn set_amount(&mut self, amount: BigDecimal) {
        self.amount = amount;
    }

    /// Pending → Invoiced, recording the invoice that billed it and the `discount` it took off
    pub(super) fn invoice(
        &mut self,
        number: String,
        discount: &BigDecimal,
    ) -> Result<(), DomainError> {
        require(&number, "invoice number")?;
        if *discount < BigDecimal::zero() || (!discount.is_zero() && *discount >= self.amount) {
            return Err(DomainError::InvalidMilestoneDiscount);
        }
        self.advance(MilestoneStatus::Invoiced)?;
        self.invoice_number = Some(number);
        self.discount = discount.clone();
        Ok(())
    }

    /// Record a credit note taking `amount` (before GST) off an invoiced or paid milestone's
    /// billing. An invoiced milestone whose billing is all taken back is credited at once, even
    /// while other lines of its invoice are still due; a paid one stays paid.
    pub(super) fn credit(&mut self, amount: &BigDecimal) -> Result<(), DomainError> {
        if !matches!(
            self.status,
            MilestoneStatus::Invoiced | MilestoneStatus::Paid
        ) {
            return Err(DomainError::NotInvoiced);
        }
        let credited = &self.credited + amount;
        if *amount <= BigDecimal::zero() || credited > self.billed() {
            return Err(DomainError::CreditExceedsMilestone);
        }
        self.credited = credited;
        if self.status == MilestoneStatus::Invoiced && self.credited == self.billed() {
            self.advance(MilestoneStatus::Credited)?;
        }
        Ok(())
    }

    /// Move one step along Pending → Invoiced → Paid, or Invoiced → Credited once credit notes
    /// have cancelled the whole invoice
    pub(super) fn advance(&mut self, to: MilestoneStatus) -> Result<(), DomainError> {
        use MilestoneStatus::{Credited, Invoiced, Paid, Pending};

        match (self.status, to) {
            (Pending, Invoiced) if self.amount <= BigDecimal::zero() => {
                Err(DomainError::NothingToInvoice)
            }
            (Invoiced, Credited) if self.credited != self.billed() => {
                Err(DomainError::CreditExceedsMilestone)
            }
            (Pending, Invoiced) | (Invoiced, Paid) | (Invoiced, Credited) => {
                self.status = to;
                Ok(())
            }
            (from, to) => Err(DomainError::InvalidTransition { from, to }),
        }
    }

    pub(super) fn validate(&self) -> Result<(), DomainError> {
        require(&self.name, "milestone name")?;
        if self.amount < BigDecimal::zero() {
            return Err(DomainError::NonPositiveAmount);
        }
        let pending = self.status == MilestoneStatus::Pending;
        if self.discount < BigDecimal::zero()
            || (!self.discount.is_zero() && (pending || self.discount >= self.amount))
        {
            return Err(DomainError::InvalidMilestoneDiscount);
        }
        let billed = self.billed();
        let credit_allowed = !pending || self.credited.is_zero();
        let fully_credited = self.credited == billed;
        if self.credited < BigDecimal::zero()
            || self.credited > billed
            || !credit_allowed
            || (self.status == MilestoneStatus::Credited && !fully_credited)
        {
            return Err(DomainError::CreditExceedsMilestone);
        }
        self.time_entries.iter().try_for_each(TimeEntry::validate)
    }
}

/// The order a project's milestones are listed in. Ties keep the order the milestones were
/// added in, so a list never shuffles between redraws.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MilestoneSort {
    /// Earliest due first
    #[default]
    Due,
    /// A to Z, ignoring case
    Name,
    /// Largest first
    Amount,
}

impl MilestoneSort {
    pub const ALL: [Self; 3] = [Self::Due, Self::Name, Self::Amount];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Due => "due date",
            Self::Name => "name",
            Self::Amount => "amount",
        }
    }

    /// The sort after this one: due date, name, amount, then due date again
    pub const fn next(self) -> Self {
        match self {
            Self::Due => Self::Name,
            Self::Name => Self::Amount,
            Self::Amount => Self::Due,
        }
    }

    /// Positions in `milestones`, in this order
    pub fn positions(self, milestones: &[Milestone]) -> Vec<usize> {
        let mut positions: Vec<usize> = (0..milestones.len()).collect();
        // Lowercased once, not on every comparison
        let names: Vec<String> = match self {
            Self::Name => milestones.iter().map(|m| m.name().to_lowercase()).collect(),
            Self::Due | Self::Amount => Vec::new(),
        };
        positions.sort_by(|&a, &b| {
            let (x, y) = (&milestones[a], &milestones[b]);
            let order = match self {
                Self::Due => x.due_date().cmp(&y.due_date()),
                Self::Name => names[a].cmp(&names[b]),
                Self::Amount => y.amount().cmp(x.amount()),
            };
            order.then(a.cmp(&b))
        });
        positions
    }
}
