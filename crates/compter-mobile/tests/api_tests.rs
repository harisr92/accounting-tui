use chrono::Local;

mod support;

use compter_mobile::api::{ApiError, CompterApp, ProjectDetail, TimeInput};
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

#[test]
fn a_project_is_edited_from_its_own_filled_form() {
    let (_scratch, mut app) = demo();
    let created = app
        .create_project(project("Support", Some("1,500")))
        .unwrap();
    let with_october = app
        .add_milestone(created.id.clone(), milestone("October", "", None))
        .unwrap();
    let october = with_october.milestones[0].id.clone();
    app.log_time(created.id.clone(), october, time("2h"))
        .unwrap();

    let unchanged = app
        .edit_project(created.id.clone(), created.edit.clone())
        .unwrap();
    assert_eq!(unchanged.edit, created.edit);

    let mut input = created.edit;
    input.name = "Support retainer".to_owned();
    input.hourly_rate = Some("2000".to_owned());
    let edited = app.edit_project(created.id, input).unwrap();

    assert_eq!(edited.name, "Support retainer");
    assert_eq!(edited.billing, "Hourly ₹2,000.00/h");
    // The pending milestone is re-priced at the new rate
    assert_eq!(edited.milestones[0].amount, "₹4,000.00");
}

#[test]
fn a_project_with_an_invoice_keeps_its_currency_and_cannot_be_deleted() {
    let (_scratch, mut app) = demo();
    let billed = invoiced_project(&app);
    assert!(!billed.deletable);

    let mut input = billed.edit.clone();
    input.client_country = Some("Germany".to_owned());
    input.client_gstin = None;
    let other = if billed.currency == "USD" {
        "EUR"
    } else {
        "USD"
    };
    input.currency = Some(other.to_owned());
    assert!(app.edit_project(billed.id.clone(), input).is_err());
    assert!(app.delete_project(billed.id.clone()).is_err());
    assert_eq!(app.project(billed.id).unwrap().currency, billed.currency);
}

#[test]
fn a_project_with_nothing_invoiced_is_deleted() {
    let (_scratch, mut app) = demo();
    let created = app.create_project(project("Website", None)).unwrap();
    assert!(created.deletable);

    app.delete_project(created.id.clone()).unwrap();

    assert!(app.project(created.id).is_err());
}

#[test]
fn a_pending_milestone_is_edited_and_removed() {
    let (_scratch, mut app) = demo();
    let created = app.create_project(project("Website", None)).unwrap();
    let detail = app
        .add_milestone(
            created.id.clone(),
            milestone("Design", "2026-11-01", Some("50,000")),
        )
        .unwrap();
    let design = &detail.milestones[0];
    assert_eq!(
        design.edit,
        milestone("Design", "2026-11-01", Some("50000.00"))
    );

    let edited = app
        .edit_milestone(
            created.id.clone(),
            design.id.clone(),
            milestone("Design system", "2026-12-01", Some("60,000")),
        )
        .unwrap();
    assert_eq!(edited.milestones[0].name, "Design system");
    assert_eq!(edited.milestones[0].amount, "₹60,000.00");
    assert_eq!(edited.total, "₹60,000.00");

    let removed = app.remove_milestone(created.id, design.id.clone()).unwrap();
    assert!(removed.milestones.is_empty());
}

#[test]
fn an_invoiced_milestone_is_neither_edited_nor_removed() {
    let (_scratch, mut app) = demo();
    let billed = invoiced_project(&app);
    let invoiced = billed
        .milestones
        .iter()
        .find(|m| m.status != "PENDING")
        .unwrap();
    assert_eq!(invoiced.last_time, None);

    assert!(
        app.edit_milestone(
            billed.id.clone(),
            invoiced.id.clone(),
            invoiced.edit.clone()
        )
        .is_err()
    );
    assert!(
        app.remove_milestone(billed.id.clone(), invoiced.id.clone())
            .is_err()
    );
    assert_eq!(app.project(billed.id.clone()).unwrap(), billed);
}

#[test]
fn the_last_time_entry_is_undone() {
    let (_scratch, mut app) = demo();
    let created = app
        .create_project(project("Support", Some("1,500")))
        .unwrap();
    let detail = app
        .add_milestone(created.id.clone(), milestone("October", "", None))
        .unwrap();
    let october = detail.milestones[0].id.clone();
    assert_eq!(detail.milestones[0].last_time, None);

    let logged = app
        .log_time(created.id.clone(), october.clone(), time("1h 30m"))
        .unwrap();
    let last = logged.milestones[0].last_time.clone().unwrap();
    assert!(last.ends_with("1h 30m · Fixes"), "{last}");

    let undone = app.undo_time(created.id.clone(), october.clone()).unwrap();
    assert_eq!(undone.milestones[0].last_time, None);
    assert_eq!(undone.milestones[0].amount, "₹0.00");
    assert!(app.undo_time(created.id, october).is_err());
}

/// A demo project with at least one invoiced milestone
fn invoiced_project(app: &CompterApp) -> ProjectDetail {
    app.projects(true)
        .unwrap()
        .into_iter()
        .map(|row| app.project(row.id).unwrap())
        .find(|p| p.milestones.iter().any(|m| m.status != "PENDING"))
        .unwrap()
}

fn time(duration: &str) -> TimeInput {
    TimeInput {
        date: String::new(),
        duration: duration.to_owned(),
        note: "Fixes".to_owned(),
    }
}

#[test]
fn editing_keeps_every_address_line() {
    let (_scratch, mut app) = demo();
    let mut input = project("Website", None);
    input.client_address = vec![
        "Floor 3".to_owned(),
        "MG Road".to_owned(),
        "Bengaluru 560001".to_owned(),
    ];
    let created = app.create_project(input.clone()).unwrap();
    assert_eq!(created.edit.client_address, input.client_address);

    let edited = app.edit_project(created.id, created.edit).unwrap();

    assert_eq!(edited.edit.client_address, input.client_address);
}
