use chrono::Local;

mod support;

use compter_mobile::api::{ApiError, CompterApp};
use support::{demo, milestone, project};

fn field(error: ApiError) -> Option<String> {
    error.field
}

#[test]
fn the_app_handle_can_cross_threads() {
    // flutter_rust_bridge keeps an opaque handle behind a lock shared with its worker threads
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<CompterApp>();
}

#[test]
fn demo_lists_projects_and_opens_one_with_formatted_amounts() {
    let (_scratch, app) = demo();
    let rows = app.projects(true).unwrap();
    let pipeline = rows.iter().find(|p| p.name == "Data Pipeline").unwrap();

    let detail = app.project(pipeline.id.clone()).unwrap();

    assert_eq!(detail.name, "Data Pipeline");
    assert_eq!(detail.currency, "INR");
    assert!(detail.total.starts_with('₹'), "{}", detail.total);
    assert!(!detail.milestones.is_empty());
    assert!(detail.milestones.iter().all(|m| m.due_date.len() == 10));
}

#[test]
fn a_fixed_project_takes_milestones_with_amounts() {
    let (_scratch, mut app) = demo();
    let created = app.create_project(project("Website", Some("  "))).unwrap();
    assert_eq!(created.billing, "Fixed");
    assert!(!created.hourly);

    let detail = app
        .add_milestone(
            created.id.clone(),
            milestone("Design", "2026-11-01", Some("50,000")),
        )
        .unwrap();

    let design = &detail.milestones[0];
    assert_eq!(design.amount, "₹50,000.00");
    assert_eq!(design.due_date, "2026-11-01");
    assert_eq!(design.status, "PENDING");
    assert_eq!(design.logged, None);
    assert_eq!(detail.total, "₹50,000.00");
}

#[test]
fn an_hourly_milestone_ignores_an_amount_and_defaults_to_today() {
    let (_scratch, mut app) = demo();
    let created = app
        .create_project(project("Support", Some("1,500")))
        .unwrap();
    assert_eq!(created.billing, "Hourly ₹1,500.00/h");

    let detail = app
        .add_milestone(created.id, milestone("October", "", Some("99")))
        .unwrap();

    let october = &detail.milestones[0];
    assert_eq!(october.amount, "₹0.00");
    assert_eq!(october.logged.as_deref(), Some("0h 00m"));
    assert_eq!(october.due_date, Local::now().date_naive().to_string());
}

#[test]
fn refused_input_names_its_field_and_saves_nothing() {
    let (_scratch, mut app) = demo();
    let before = app.projects(true).unwrap().len();

    let bad_rate = app
        .create_project(project("Support", Some("lots")))
        .unwrap_err();
    assert_eq!(field(bad_rate).as_deref(), Some("hourly_rate"));
    assert_eq!(app.projects(true).unwrap().len(), before);

    let fixed = app.create_project(project("Website", None)).unwrap();
    let no_amount = app
        .add_milestone(fixed.id.clone(), milestone("Design", "", None))
        .unwrap_err();
    assert_eq!(field(no_amount).as_deref(), Some("amount"));
    let bad_date = app
        .add_milestone(
            fixed.id.clone(),
            milestone("Design", "01/11/2026", Some("1")),
        )
        .unwrap_err();
    assert_eq!(field(bad_date).as_deref(), Some("due_date"));
    assert!(app.project(fixed.id).unwrap().milestones.is_empty());

    let bad_id = app.project("nope".to_owned()).unwrap_err();
    assert_eq!(field(bad_id).as_deref(), Some("project_id"));
}

#[test]
fn domain_rules_come_back_as_messages_without_a_field() {
    let (_scratch, mut app) = demo();
    let error = app.create_project(project("", None)).unwrap_err();
    assert_eq!(error.field, None);
    assert!(!error.message.is_empty());
}

#[test]
fn projects_are_saved_in_the_data_folder() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_string_lossy().into_owned();
    {
        let mut app = CompterApp::open(path.clone()).unwrap();
        app.create_project(project("Website", None)).unwrap();
    }

    let reopened = CompterApp::open(path).unwrap();

    let rows = reopened.projects(false).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "Website");
    assert_eq!(rows[0].status, "ACTIVE");
}
