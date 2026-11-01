use chrono::NaiveDate;

use compter_core::services::{Dashboard, ReadyMilestone};

use super::FigureRow;
use super::reports::rupees;

/// The chip on an invoice that has waited too long, like the other status labels
const OVERDUE: &str = "OVERDUE";

/// What needs the user now, for the Home tab. Unlike [super::ReportRow] it has no year: it is
/// always as of today.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashboardRow {
    /// Owed, overdue, received this month and ready to bill
    pub figures: Vec<FigureRow>,
    /// Invoices with something left to pay, oldest first
    pub unpaid: Vec<UnpaidRow>,
    /// Pending milestones already due, earliest first
    pub ready: Vec<ReadyRow>,
}

/// An invoice still waiting for payment
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnpaidRow {
    pub number: String,
    pub client_name: String,
    /// The milestones billed: `Design, Build`
    pub milestones: String,
    /// `YYYY-MM-DD`
    pub date: String,
    /// What is left to pay, in the invoice's currency
    pub due: String,
    /// Days since it was issued
    pub days: u32,
    /// `OVERDUE` once it has waited longer than [compter_core::domain::OVERDUE_DAYS]; `None` before that
    pub status: Option<String>,
}

/// A milestone that can be invoiced now
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadyRow {
    pub project_id: String,
    pub project_name: String,
    pub client_name: String,
    pub milestone_name: String,
    /// `YYYY-MM-DD`
    pub due_date: String,
    /// What it would bill, less its discount, in the project's currency
    pub amount: String,
}

impl DashboardRow {
    pub(crate) fn new(dashboard: &Dashboard, today: NaiveDate) -> Self {
        let ready = match dashboard.ready.len() {
            1 => "1 milestone".to_owned(),
            n => format!("{n} milestones"),
        };
        let overdue = match dashboard.overdue_count {
            0 => "Overdue".to_owned(),
            n => format!("Overdue ({n})"),
        };
        Self {
            figures: vec![
                figure("Owed to you", rupees(&dashboard.owed)),
                figure(&overdue, rupees(&dashboard.overdue)),
                figure(
                    &format!("Received in {}", today.format("%b")),
                    rupees(&dashboard.received_this_month),
                ),
                figure("Ready to bill", ready),
            ],
            unpaid: dashboard
                .unpaid
                .iter()
                .map(|s| UnpaidRow {
                    number: s.number().to_string(),
                    client_name: s.client_name().to_owned(),
                    milestones: s.milestones().to_owned(),
                    date: s.date().to_string(),
                    due: s.currency().display(s.balance()),
                    days: s.days_waiting(today),
                    status: s.is_overdue(today).then(|| OVERDUE.to_owned()),
                })
                .collect(),
            ready: dashboard.ready.iter().map(ReadyRow::from).collect(),
        }
    }
}

impl From<&ReadyMilestone> for ReadyRow {
    fn from(milestone: &ReadyMilestone) -> Self {
        Self {
            project_id: milestone.project_id.to_string(),
            project_name: milestone.project_name.clone(),
            client_name: milestone.client_name.clone(),
            milestone_name: milestone.milestone_name.clone(),
            due_date: milestone.due_date.to_string(),
            amount: milestone.currency.display(&milestone.amount),
        }
    }
}

fn figure(label: &str, value: String) -> FigureRow {
    FigureRow {
        label: label.to_owned(),
        value,
    }
}
