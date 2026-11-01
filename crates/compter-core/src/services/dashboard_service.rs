use bigdecimal::{BigDecimal, Zero};
use chrono::{Datelike, NaiveDate};

use super::ServiceError;
use crate::domain::{Currency, InvoiceSummary, MilestoneStatus, ProjectId, ReturnKind};
use crate::storage::{InvoiceRepository, ProjectRepository};

/// What needs the freelancer now: money owed, overdue invoices, work ready to bill. Unlike a
/// year report it has no year: it is always as of today.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dashboard {
    /// Still due across all invoices, in rupees (foreign invoices at their rate)
    pub owed: BigDecimal,
    /// The part of `owed` on overdue invoices ([InvoiceSummary::is_overdue]), in rupees
    pub overdue: BigDecimal,
    pub overdue_count: usize,
    /// Rupees that reached the bank this calendar month
    pub received_this_month: BigDecimal,
    /// Invoices with something left to pay, oldest first
    pub unpaid: Vec<InvoiceSummary>,
    /// Pending milestones already due, on projects not archived, earliest first
    pub ready: Vec<ReadyMilestone>,
}

/// A pending milestone whose due date has come: it can be invoiced
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadyMilestone {
    pub project_id: ProjectId,
    pub project_name: String,
    pub client_name: String,
    pub milestone_name: String,
    pub due_date: NaiveDate,
    /// What it would bill, less its discount, in `currency`
    pub amount: BigDecimal,
    pub currency: Currency,
}

/// Builds the [Dashboard] from invoice summaries and one project at a time
pub struct DashboardService<R> {
    repo: R,
}

impl<R: InvoiceRepository + ProjectRepository> DashboardService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }

    /// Everything Home shows, as of `today`
    pub fn now(&self, today: NaiveDate) -> Result<Dashboard, ServiceError> {
        let unpaid = self.unpaid()?;
        let overdue: Vec<_> = unpaid.iter().filter(|s| s.is_overdue(today)).collect();
        Ok(Dashboard {
            owed: unpaid.iter().map(InvoiceSummary::balance_inr).sum(),
            overdue: overdue.iter().map(|s| s.balance_inr()).sum(),
            overdue_count: overdue.len(),
            received_this_month: self.received_in_month(today)?,
            ready: self.ready(today)?,
            unpaid,
        })
    }

    /// Invoices with something left to pay, oldest first. Read from the summaries only, so it
    /// is cheap enough to call on every visit.
    pub fn unpaid(&self) -> Result<Vec<InvoiceSummary>, ServiceError> {
        // The repository lists newest first
        Ok(self
            .repo
            .list_invoices()?
            .into_iter()
            .rev()
            .filter(|s| !s.is_settled())
            .collect())
    }

    /// Rupees that reached the bank in the calendar month of `day`, less refunds by credit notes
    /// dated in it, as the year report counts `received`
    pub fn received_in_month(&self, day: NaiveDate) -> Result<BigDecimal, ServiceError> {
        let in_month = |date: NaiveDate| date.year() == day.year() && date.month() == day.month();
        let mut received = BigDecimal::zero();
        self.repo.scan_invoices(&mut |invoice| {
            for payment in invoice.payments().iter().filter(|p| in_month(p.date)) {
                received += payment.received_inr();
            }
            let refunds = invoice
                .credits()
                .iter()
                .filter(|c| in_month(c.date))
                .filter_map(|c| c.returned.as_ref())
                .filter(|r| r.kind == ReturnKind::Refunded);
            for returned in refunds {
                received -= invoice.refunded_inr(returned);
            }
        })?;
        Ok(received)
    }

    /// Pending milestones due by `today` with something to bill, on projects not archived,
    /// earliest first. Projects are read one at a time.
    pub fn ready(&self, today: NaiveDate) -> Result<Vec<ReadyMilestone>, ServiceError> {
        let mut ready = Vec::new();
        self.repo.scan_projects(&mut |project| {
            if project.is_archived() {
                return;
            }
            let due = project.milestones().iter().filter(|m| {
                m.status() == MilestoneStatus::Pending
                    && m.due_date() <= today
                    && m.billed() > BigDecimal::zero()
            });
            ready.extend(due.map(|milestone| ReadyMilestone {
                project_id: project.id(),
                project_name: project.name().to_owned(),
                client_name: project.client_name().to_owned(),
                milestone_name: milestone.name().to_owned(),
                due_date: milestone.due_date(),
                amount: milestone.billed(),
                currency: project.currency(),
            }));
        })?;
        ready.sort_by(|a, b| {
            a.due_date
                .cmp(&b.due_date)
                .then_with(|| a.project_name.cmp(&b.project_name))
        });
        Ok(ready)
    }
}
