//! Records saved by earlier builds must keep loading. If one of these tests fails, the stored
//! format changed: add `#[serde(default)]` or a migration (see `storage/migrations.rs`).

use bigdecimal::BigDecimal;

use crate::domain::{Billing, MilestoneStatus, Project, ProjectStatus};
use crate::storage::codec::decode;

const HOURLY_V1: &str = include_str!("fixtures/project_v1.json");
const FIXED_V1: &str = include_str!("fixtures/project_fixed_v1.json");

#[test]
fn hourly_project_v1_still_loads() {
    let project: Project = decode("projects", "fixture", HOURLY_V1.as_bytes()).unwrap();
    assert_eq!(project.name(), "Mobile App");
    assert_eq!(project.billing().rate(), Some(&BigDecimal::from(1500)));
    assert!(project.client_gstin().is_none());
    let milestones = project.milestones();
    assert_eq!(milestones[0].status(), MilestoneStatus::Invoiced);
    assert_eq!(milestones[0].logged_minutes(), 630);
    assert_eq!(milestones[1].time_entries()[0].note(), "Auth spike");
    assert_eq!(project.total_amount(), &BigDecimal::from(18_750));
    assert_eq!(project.status(), ProjectStatus::Active);
}

#[test]
fn fixed_project_v1_still_loads() {
    let project: Project = decode("projects", "fixture", FIXED_V1.as_bytes()).unwrap();
    assert_eq!(project.billing(), &Billing::Fixed);
    assert_eq!(
        project.client_gstin().map(|g| g.as_str()),
        Some("27AAPFU0939F1ZV")
    );
    assert_eq!(project.milestones()[0].status(), MilestoneStatus::Paid);
    assert_eq!(project.status(), ProjectStatus::Paid);
}
