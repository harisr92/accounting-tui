//! Sample data for the layout shell. Replaced by `FileStorage` in step 4.

use bigdecimal::BigDecimal;
use chrono::NaiveDate;

use crate::domain::{Milestone, MilestoneStatus, Project, ProjectStatus};

fn milestone(
    name: &str,
    amount: i64,
    (y, m, d): (i32, u32, u32),
    status: MilestoneStatus,
) -> Milestone {
    Milestone {
        name: name.to_owned(),
        amount: BigDecimal::from(amount),
        due_date: NaiveDate::from_ymd_opt(y, m, d).unwrap_or_default(),
        status,
    }
}

pub fn projects() -> Vec<Project> {
    use MilestoneStatus::{Invoiced, Paid, Pending};

    vec![
        Project::new(
            "p-001",
            "Website Revamp",
            "Acme Pvt Ltd",
            Some("29ABCDE1234F1Z5".to_owned()),
            vec![
                milestone("Design", 50_000, (2026, 9, 1), Paid),
                milestone("Frontend", 60_000, (2026, 10, 20), Invoiced),
                milestone("Launch", 40_000, (2026, 11, 15), Pending),
            ],
            ProjectStatus::Active,
        ),
        Project::new(
            "p-002",
            "Mobile App",
            "Globex Labs",
            None,
            vec![
                milestone("Prototype", 75_000, (2026, 10, 30), Pending),
                milestone("Beta", 125_000, (2026, 12, 15), Pending),
            ],
            ProjectStatus::Active,
        ),
        Project::new(
            "p-003",
            "API Audit",
            "Initech Solutions",
            Some("27AAACI1234K1Z9".to_owned()),
            vec![milestone("Audit report", 90_000, (2026, 6, 30), Paid)],
            ProjectStatus::Paid,
        ),
        Project::new(
            "p-004",
            "Data Pipeline",
            "Umbrella Analytics",
            Some("07AABCU9603R1ZX".to_owned()),
            vec![
                milestone("Ingestion", 45_000, (2026, 8, 10), Paid),
                milestone("Dashboards", 35_000, (2026, 9, 25), Invoiced),
            ],
            ProjectStatus::Completed,
        ),
    ]
}
