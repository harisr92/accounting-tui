use bigdecimal::Zero;
use compter_core::domain::{
    Billing, ClientRegion, Milestone, MilestoneSort, MilestoneStatus, Project, ProjectSummary,
    TimeEntry, format_duration, state_name,
};

/// One line of the project list, from the stored summary (never the full record)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRow {
    pub id: String,
    pub name: String,
    pub client_name: String,
    /// `ACTIVE`, `COMPLETED` or `PAID`
    pub status: String,
    pub archived: bool,
}

impl From<&ProjectSummary> for ProjectRow {
    fn from(summary: &ProjectSummary) -> Self {
        Self {
            id: summary.id().to_string(),
            name: summary.name().to_owned(),
            client_name: summary.client_name().to_owned(),
            status: summary.status().label().to_owned(),
            archived: summary.is_archived(),
        }
    }
}

/// An opened project with its milestones; amounts are formatted in the project's currency
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectDetail {
    pub id: String,
    pub name: String,
    pub client_name: String,
    pub client_gstin: Option<String>,
    /// `Karnataka (29)` or `Germany`; `None` when not known
    pub client_location: Option<String>,
    /// `INR`, `USD`, `GBP` or `EUR`
    pub currency: String,
    /// `Fixed` or `Hourly ₹1,500.00/h`
    pub billing: String,
    /// Hourly milestones take no amount: it comes from logged time
    pub hourly: bool,
    pub status: String,
    pub archived: bool,
    pub total: String,
    pub paid: String,
    /// `None` when nothing was credited, as the terminal hides that tile
    pub credited: Option<String>,
    pub outstanding: String,
    /// Time logged on every milestone, `12h 30m`; `None` on fixed-price projects
    pub logged: Option<String>,
    /// In the order they were added. Listed through one of the orders below, never sorted in
    /// Dart.
    pub milestones: Vec<MilestoneRow>,
    /// Positions in `milestones` by due date, earliest first: the default order
    pub by_due: Vec<u32>,
    /// Positions in `milestones` by name, A to Z
    pub by_name: Vec<u32>,
    /// Positions in `milestones` by amount, largest first
    pub by_amount: Vec<u32>,
    /// Nothing in it was invoiced, so it may be deleted
    pub deletable: bool,
    /// Finished (nothing pending), so it may be archived
    pub archivable: bool,
    /// The form, filled with this project, to edit it
    pub edit: ProjectInput,
}

impl From<&Project> for ProjectDetail {
    fn from(project: &Project) -> Self {
        let currency = project.currency();
        let billing = match project.billing() {
            Billing::Fixed => "Fixed".to_owned(),
            Billing::Hourly { rate } => format!("Hourly {}/h", currency.display(rate)),
        };
        let credited = project.credited_amount();
        let hourly = matches!(project.billing(), Billing::Hourly { .. });
        Self {
            id: project.id().to_string(),
            name: project.name().to_owned(),
            client_name: project.client_name().to_owned(),
            client_gstin: project.client_gstin().map(ToString::to_string),
            client_location: match project.client_region() {
                ClientRegion::India { state: Some(code) } => {
                    Some(match state_name(code.as_str()) {
                        Some(name) => format!("{name} ({code})"),
                        None => code.as_str().to_owned(),
                    })
                }
                ClientRegion::India { state: None } => None,
                ClientRegion::Abroad { country } => Some(country.clone()),
            },
            currency: currency.code().to_owned(),
            billing,
            hourly,
            status: project.status().label().to_owned(),
            archived: project.is_archived(),
            total: currency.display(project.total_amount()),
            paid: currency.display(&project.paid_amount()),
            credited: (!credited.is_zero()).then(|| currency.display(&credited)),
            outstanding: currency.display(&project.outstanding_amount()),
            logged: hourly.then(|| format_duration(project.logged_minutes())),
            milestones: project
                .milestones()
                .iter()
                .map(|m| MilestoneRow::new(m, project, hourly))
                .collect(),
            by_due: order(MilestoneSort::Due, project),
            by_name: order(MilestoneSort::Name, project),
            by_amount: order(MilestoneSort::Amount, project),
            deletable: project.ensure_deletable().is_ok(),
            archivable: project.ensure_archivable().is_ok(),
            edit: ProjectInput::from(project),
        }
    }
}

/// `project`'s milestone positions in `sort` order
fn order(sort: MilestoneSort, project: &Project) -> Vec<u32> {
    sort.positions(project.milestones())
        .into_iter()
        .filter_map(|i| u32::try_from(i).ok())
        .collect()
}

/// One milestone of an opened project
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MilestoneRow {
    pub id: String,
    pub name: String,
    pub amount: String,
    /// `YYYY-MM-DD`
    pub due_date: String,
    /// `PENDING`, `INVOICED`, `PAID` or `CREDITED`
    pub status: String,
    /// Logged time on an hourly milestone, as `6h 30m`
    pub logged: Option<String>,
    pub invoice_number: Option<String>,
    /// The time entry undoing would remove, as `2026-10-18 · 1h 30m · note`; `None` when
    /// there is none or the milestone is no longer pending
    pub last_time: Option<String>,
    /// Every time entry logged on it, newest logged first; empty on fixed-price projects
    pub time_log: Vec<TimeEntryRow>,
    /// The form, filled with this milestone, to edit it
    pub edit: MilestoneInput,
}

impl MilestoneRow {
    fn new(milestone: &Milestone, project: &Project, hourly: bool) -> Self {
        let pending = milestone.status() == MilestoneStatus::Pending;
        Self {
            id: milestone.id().to_string(),
            name: milestone.name().to_owned(),
            amount: project.currency().display(milestone.amount()),
            due_date: milestone.due_date().to_string(),
            status: milestone.status().label().to_owned(),
            logged: hourly.then(|| format_duration(milestone.logged_minutes())),
            invoice_number: milestone.invoice_number().map(str::to_owned),
            last_time: milestone
                .time_entries()
                .last()
                .filter(|_| pending)
                .map(|entry| {
                    let duration = format_duration(entry.minutes());
                    match entry.note() {
                        "" => format!("{} · {duration}", entry.date()),
                        note => format!("{} · {duration} · {note}", entry.date()),
                    }
                }),
            time_log: milestone
                .time_entries()
                .iter()
                .rev()
                .map(TimeEntryRow::from)
                .collect(),
            edit: MilestoneInput {
                name: milestone.name().to_owned(),
                due_date: milestone.due_date().to_string(),
                amount: (!hourly).then(|| milestone.amount().to_string()),
            },
        }
    }
}

/// Time logged on an hourly milestone: when, how long and what was done
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeEntryRow {
    /// `YYYY-MM-DD`
    pub date: String,
    /// `1h 30m`
    pub duration: String,
    /// What was done; may be blank
    pub note: String,
}

impl From<&TimeEntry> for TimeEntryRow {
    fn from(entry: &TimeEntry) -> Self {
        Self {
            date: entry.date().to_string(),
            duration: format_duration(entry.minutes()),
            note: entry.note().to_owned(),
        }
    }
}

/// A project as typed. A client with a country is abroad (billed in any currency, invoiced
/// as an export); one without is in India, billed in rupees.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectInput {
    pub name: String,
    pub client_name: String,
    /// Blank for an unregistered client or one abroad
    pub client_gstin: Option<String>,
    /// GST state code (`29`): the place of supply for a client in India without a GSTIN
    pub client_state: Option<String>,
    /// Blank for a client in India
    pub client_country: Option<String>,
    /// One entry per line; blank lines are dropped
    pub client_address: Vec<String>,
    /// `INR`, `USD`, `GBP` or `EUR`; blank means INR
    pub currency: Option<String>,
    /// Blank or `None` for a fixed-price project
    pub hourly_rate: Option<String>,
}

/// A milestone as typed
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MilestoneInput {
    pub name: String,
    /// `YYYY-MM-DD`; blank means today
    pub due_date: String,
    /// Needed on fixed-price projects, ignored on hourly ones
    pub amount: Option<String>,
}

impl From<&Project> for ProjectInput {
    fn from(project: &Project) -> Self {
        let client = project.client_details();
        let (client_state, client_country) = match client.region {
            ClientRegion::India { state } => (state.map(|s| s.as_str().to_owned()), None),
            ClientRegion::Abroad { country } => (None, Some(country)),
        };
        Self {
            name: project.name().to_owned(),
            client_name: client.name,
            client_gstin: client.gstin,
            client_state,
            client_country,
            client_address: client.address,
            currency: Some(client.currency.code().to_owned()),
            hourly_rate: project.billing().rate().map(ToString::to_string),
        }
    }
}
