use std::fmt;

use bigdecimal::BigDecimal;

use super::{Milestone, MilestoneStatus};

/// Lifecycle state of a project
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectStatus {
    Active,
    Completed,
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
}

impl fmt::Display for ProjectStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Work done for one client, split into milestones
#[derive(Debug, Clone, PartialEq)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub client_gstin: Option<String>,
    pub client_name: String,
    pub milestones: Vec<Milestone>,
    pub total_amount: BigDecimal,
    pub status: ProjectStatus,
}

impl Project {
    /// Build a project whose `total_amount` is the sum of its milestones
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        client_name: impl Into<String>,
        client_gstin: Option<String>,
        milestones: Vec<Milestone>,
        status: ProjectStatus,
    ) -> Self {
        let total_amount = milestones.iter().map(|m| &m.amount).sum();
        Self {
            id: id.into(),
            name: name.into(),
            client_gstin,
            client_name: client_name.into(),
            milestones,
            total_amount,
            status,
        }
    }

    /// Sum of milestones already paid
    pub fn paid_amount(&self) -> BigDecimal {
        self.milestones
            .iter()
            .filter(|m| m.status == MilestoneStatus::Paid)
            .map(|m| &m.amount)
            .sum()
    }

    /// Amount still to be received
    pub fn outstanding_amount(&self) -> BigDecimal {
        &self.total_amount - self.paid_amount()
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;

    fn milestone(amount: i64, status: MilestoneStatus) -> Milestone {
        Milestone {
            name: "m".into(),
            amount: BigDecimal::from(amount),
            due_date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            status,
        }
    }

    #[test]
    fn totals_derive_from_milestones() {
        let project = Project::new(
            "p",
            "P",
            "C",
            None,
            vec![
                milestone(100, MilestoneStatus::Paid),
                milestone(250, MilestoneStatus::Invoiced),
            ],
            ProjectStatus::Active,
        );
        assert_eq!(project.total_amount, BigDecimal::from(350));
        assert_eq!(project.paid_amount(), BigDecimal::from(100));
        assert_eq!(project.outstanding_amount(), BigDecimal::from(250));
    }
}
