use bigdecimal::Zero;
use compter_core::domain::{
    Billing, ClientRegion, Milestone, Project, ProjectSummary, format_duration, state_name,
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
    pub milestones: Vec<MilestoneRow>,
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
            milestones: project
                .milestones()
                .iter()
                .map(|m| MilestoneRow::new(m, project, hourly))
                .collect(),
        }
    }
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
}

impl MilestoneRow {
    fn new(milestone: &Milestone, project: &Project, hourly: bool) -> Self {
        Self {
            id: milestone.id().to_string(),
            name: milestone.name().to_owned(),
            amount: project.currency().display(milestone.amount()),
            due_date: milestone.due_date().to_string(),
            status: milestone.status().label().to_owned(),
            logged: hourly.then(|| format_duration(milestone.logged_minutes())),
            invoice_number: milestone.invoice_number().map(str::to_owned),
        }
    }
}

/// A new project as typed. A client with a country is abroad (billed in any currency, invoiced
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
    /// Address lines; blank lines are dropped
    pub client_address: Vec<String>,
    /// `INR`, `USD`, `GBP` or `EUR`; blank means INR
    pub currency: Option<String>,
    /// Blank or `None` for a fixed-price project
    pub hourly_rate: Option<String>,
}

/// A new milestone as typed
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MilestoneInput {
    pub name: String,
    /// `YYYY-MM-DD`; blank means today
    pub due_date: String,
    /// Needed on fixed-price projects, ignored on hourly ones
    pub amount: Option<String>,
}
