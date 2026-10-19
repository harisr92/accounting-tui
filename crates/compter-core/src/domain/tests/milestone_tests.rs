use bigdecimal::BigDecimal;
use chrono::NaiveDate;

use crate::domain::{Billing, MilestoneSort, Project};

fn day(d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, d).unwrap()
}

/// Added in this order: "launch" due 20th ₹500, "Design" due 5th ₹900, "build" due 20th ₹900
fn project() -> Project {
    let mut p = Project::new("Site", "Acme", None, Billing::Fixed).unwrap();
    for (name, due, amount) in [("launch", 20, 500), ("Design", 5, 900), ("build", 20, 900)] {
        p.add_milestone(name, day(due), Some(BigDecimal::from(amount)))
            .unwrap();
    }
    p
}

#[test]
fn milestones_sort_by_due_date_by_default() {
    assert_eq!(MilestoneSort::default(), MilestoneSort::Due);
    // launch and build are both due on the 20th: launch was added first
    assert_eq!(
        MilestoneSort::Due.positions(project().milestones()),
        [1, 0, 2]
    );
}

#[test]
fn names_sort_ignoring_case() {
    assert_eq!(
        MilestoneSort::Name.positions(project().milestones()),
        [2, 1, 0]
    );
}

#[test]
fn amounts_sort_largest_first_and_ties_keep_the_order_added() {
    assert_eq!(
        MilestoneSort::Amount.positions(project().milestones()),
        [1, 2, 0]
    );
}

#[test]
fn the_sort_cycles_through_every_order() {
    let mut sort = MilestoneSort::default();
    let mut seen = Vec::new();
    for _ in MilestoneSort::ALL {
        seen.push(sort);
        sort = sort.next();
    }
    assert_eq!(seen, MilestoneSort::ALL);
    assert_eq!(sort, MilestoneSort::Due);
}
