mod support;

use compter_mobile::api::DashboardRow;
use support::demo;

fn figure<'a>(dashboard: &'a DashboardRow, label: &str) -> &'a str {
    &dashboard
        .figures
        .iter()
        .find(|f| f.label.starts_with(label))
        .unwrap()
        .value
}

#[test]
fn home_shows_what_clients_owe_oldest_first() {
    let (_scratch, app) = demo();

    let dashboard = app.dashboard().unwrap();

    // The demo's invoices are all from 2026, so these hold on any later day
    assert_eq!(figure(&dashboard, "Owed to you"), "₹95,285.00");
    let billed: Vec<_> = dashboard
        .unpaid
        .iter()
        .map(|row| row.milestones.as_str())
        .collect();
    assert_eq!(billed, ["Dashboards", "Frontend", "Prototype"]);
    let dashboards = &dashboard.unpaid[0];
    assert_eq!(dashboards.client_name, "Umbrella Analytics");
    assert_eq!(dashboards.date, "2026-09-25");
    assert_eq!(dashboards.due, "₹41,300.00");
    assert_eq!(dashboards.status.as_deref(), Some("OVERDUE"));
    assert!(dashboards.days > 30);
    assert_eq!(dashboard.figures.len(), 4);
}

#[test]
fn a_ready_milestone_opens_its_project() {
    let (_scratch, app) = demo();

    for ready in app.dashboard().unwrap().ready {
        let project = app.project(ready.project_id.clone()).unwrap();
        assert_eq!(project.name, ready.project_name);
        assert!(["Launch", "Beta"].contains(&ready.milestone_name.as_str()));
    }
}
