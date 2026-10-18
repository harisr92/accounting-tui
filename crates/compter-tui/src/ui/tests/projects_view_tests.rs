use crate::ui::key::Key;
use bigdecimal::BigDecimal;

use crate::ui::tests::support::{projects_app, screen, type_keys};
use compter_core::demo;
use compter_core::storage::{ProjectRepository, Store};

const W: u16 = 110;
const H: u16 = 30;

fn names(store: &Store) -> Vec<String> {
    let summaries = store.repository().list().unwrap();
    summaries.iter().map(|s| s.name().to_owned()).collect()
}

/// `n`, type `fields` from the first field (Tab moves on), then Enter
fn create_project(app: &mut crate::app::App, fields: &str) {
    app.handle_key(Key::Char('n'));
    type_keys(app, fields);
    app.handle_key(Key::Enter);
}

/// Fields of the project form up to Billing: name, client, GSTIN, client in, state, country,
/// two address lines
fn hourly_project_keys(name: &str, client: &str, rate: &str) -> String {
    // State 29, then past country, address and currency to Billing (space: hourly) and the rate
    format!("{name}\t{client}\t\t\t29\t\t\t\t\t \t{rate}")
}

#[test]
fn empty_database_shows_how_to_start() {
    let store = Store::in_memory().unwrap();
    let app = projects_app(&store);
    assert!(screen(&app, W, H).contains("No projects yet. Press n to create one."));
}

#[test]
fn creating_a_project_saves_it() {
    let store = Store::in_memory().unwrap();
    let mut app = projects_app(&store);
    create_project(&mut app, "Website\tAcme Pvt Ltd\t29aapfu0939f1zr");

    assert_eq!(names(&store), ["Website"]);
    let project = store
        .repository()
        .load(store.repository().list().unwrap()[0].id())
        .unwrap();
    assert_eq!(
        project.client_gstin().map(|g| g.as_str()),
        Some("29AAPFU0939F1ZR")
    );
    let screen = screen(&app, W, H);
    assert!(screen.contains("Project created"), "{screen}");
    assert!(screen.contains("Acme Pvt Ltd"));
}

#[test]
fn invalid_input_keeps_the_form_open_with_the_error() {
    let store = Store::in_memory().unwrap();
    let mut app = projects_app(&store);
    create_project(&mut app, "Site\tAcme\tNOT-A-GSTIN");

    assert!(names(&store).is_empty());
    let screen = screen(&app, W, H);
    assert!(screen.contains("New project"), "{screen}");
    assert!(screen.contains("invalid GSTIN"), "{screen}");
    assert!(screen.contains("NOT-A-GSTIN"));
}

#[test]
fn hourly_project_logs_time_into_amounts() {
    let store = Store::in_memory().unwrap();
    let mut app = projects_app(&store);
    create_project(&mut app, &hourly_project_keys("App", "Globex", "1500"));
    app.handle_key(Key::Tab);
    app.handle_key(Key::Char('a'));
    type_keys(&mut app, "Prototype\n");

    app.handle_key(Key::Char('t'));
    type_keys(&mut app, "\t1h30m\tspike");
    let preview = screen(&app, W, H);
    assert!(preview.contains("= 1h 30m"), "{preview}");
    app.handle_key(Key::Enter);

    let summary = &store.repository().list().unwrap()[0];
    assert_eq!(summary.total_amount(), &BigDecimal::from(2250));
    let screen = screen(&app, W, H);
    assert!(screen.contains("Logged 1h 30m on Prototype"), "{screen}");
    assert!(screen.contains("₹2,250.00"));
}

#[test]
fn a_plain_90_is_hours_and_is_refused_as_one_entry() {
    let store = Store::in_memory().unwrap();
    let mut app = projects_app(&store);
    create_project(&mut app, &hourly_project_keys("App", "Globex", "1500"));
    app.handle_key(Key::Tab);
    app.handle_key(Key::Char('a'));
    type_keys(&mut app, "Prototype\n");

    app.handle_key(Key::Char('t'));
    type_keys(&mut app, "\t90");
    assert!(screen(&app, W, H).contains("over 24h"));
    app.handle_key(Key::Enter);

    let screen = screen(&app, W, H);
    assert!(screen.contains("at most 24 hours"), "{screen}");
    assert_eq!(
        store.repository().list().unwrap()[0].total_amount(),
        &BigDecimal::from(0)
    );
}

#[test]
fn delete_asks_first() {
    let store = Store::in_memory().unwrap();
    let mut app = projects_app(&store);
    create_project(&mut app, "Pitch\tAcme");
    type_keys(&mut app, "xn");
    assert_eq!(names(&store), ["Pitch"]);
    type_keys(&mut app, "xy");
    assert!(names(&store).is_empty());
    assert!(screen(&app, W, H).contains("Project deleted"));
}

#[test]
fn projects_with_invoiced_milestones_cannot_be_deleted() {
    let store = demo::store().unwrap();
    let mut app = projects_app(&store);
    // "API Audit" sorts first and is fully paid
    type_keys(&mut app, "xy");
    assert_eq!(names(&store).len(), 4);
    let screen = screen(&app, W, H);
    assert!(screen.contains("Can't delete"), "{screen}");
    assert!(!screen.contains("Confirm"));
}

#[test]
fn invoiced_milestones_are_locked() {
    let store = demo::store().unwrap();
    let mut app = projects_app(&store);
    // "API Audit" sorts first; its only milestone is paid
    type_keys(&mut app, "\te");
    assert!(screen(&app, W, H).contains("Invoiced milestones are locked"));
}

#[test]
fn typing_in_a_form_never_triggers_shortcuts() {
    let store = Store::in_memory().unwrap();
    let mut app = projects_app(&store);
    app.handle_key(Key::Char('n'));
    type_keys(&mut app, "q?x1");
    assert!(app.is_running());
    assert!(!app.show_help());
    app.handle_key(Key::Esc);
    assert!(names(&store).is_empty());

    app.handle_key(Key::Char('n'));
    app.handle_key(Key::Ctrl('c'));
    assert!(!app.is_running());
}

/// Demo project "Mobile App" (third by name), milestones focused
fn mobile_app(store: &Store) -> crate::app::App {
    let mut app = projects_app(store);
    type_keys(&mut app, "jj\t");
    app
}

#[test]
fn o_shows_and_hides_a_milestones_time_log() {
    let store = demo::store().unwrap();
    let mut app = mobile_app(&store);
    assert!(!screen(&app, W, H).contains("Wireframes"));

    app.handle_key(Key::Char('o'));
    let open = screen(&app, W, H);
    for expected in [
        "06 Oct",
        "Wireframes",
        "6h 00m",
        "Navigation shell",
        "4h 30m",
    ] {
        assert!(open.contains(expected), "missing {expected:?}\n{open}");
    }
    // The log sits under Prototype, above Beta
    let line = |text: &str| open.lines().position(|l| l.contains(text)).unwrap();
    assert!(line("Prototype") < line("Wireframes"));
    assert!(line("Navigation shell") < line("Beta"));

    app.handle_key(Key::Char('o'));
    assert!(!screen(&app, W, H).contains("Wireframes"));
}

#[test]
fn selection_steps_over_open_logs() {
    let store = demo::store().unwrap();
    let mut app = mobile_app(&store);
    type_keys(&mut app, "oj");
    let screen = screen(&app, W, H);
    assert!(screen.contains("> Beta"), "{screen}");
    assert!(!screen.contains("> Prototype"));
    assert!(screen.contains("Wireframes"));
}

#[test]
fn time_log_follows_undo() {
    let store = Store::in_memory().unwrap();
    let mut app = projects_app(&store);
    create_project(&mut app, &hourly_project_keys("App", "Globex", "1500"));
    app.handle_key(Key::Tab);
    app.handle_key(Key::Char('a'));
    type_keys(&mut app, "Prototype\n");
    app.handle_key(Key::Char('o'));
    assert!(screen(&app, W, H).contains("no time logged yet"));

    app.handle_key(Key::Char('t'));
    type_keys(&mut app, "\t2h\tspike\n");
    assert!(screen(&app, W, H).contains("spike"));
    type_keys(&mut app, "uy");
    let after = screen(&app, W, H);
    assert!(!after.contains("spike"), "{after}");
    assert!(after.contains("no time logged yet"));
}

#[test]
fn fixed_price_milestones_have_no_log() {
    let store = demo::store().unwrap();
    let mut app = projects_app(&store);
    type_keys(&mut app, "\to");
    assert!(screen(&app, W, H).contains("Fixed-price milestones have no time log"));
}

#[test]
fn opening_a_log_at_the_bottom_scrolls_it_into_view() {
    let store = demo::store().unwrap();
    let mut app = mobile_app(&store);
    // At this height Beta is the last row the milestone panel shows
    const SHORT: u16 = 15;
    type_keys(&mut app, "jo");
    let screen = screen(&app, W, SHORT);
    assert!(screen.contains("> Beta"), "{screen}");
    assert!(screen.contains("└ "), "{screen}");
}

/// A store with "Website" (Acme, nothing billed) and "Audit" (Initech, its one milestone
/// invoiced, so finished)
fn store_with_finished_project() -> Store {
    use compter_core::domain::{Billing, Project};
    let store = Store::in_memory().unwrap();
    let mut repo = store.repository();
    let date = chrono::NaiveDate::from_ymd_opt(2026, 6, 30).unwrap();
    let mut audit = Project::new("Audit", "Initech", None, Billing::Fixed).unwrap();
    audit
        .add_milestone("Report", date, Some(BigDecimal::from(900)))
        .unwrap();
    audit.invoice_milestone(0, "T-1").unwrap();
    repo.save(&audit).unwrap();
    let site = Project::new("Website", "Acme", None, Billing::Fixed).unwrap();
    repo.save(&site).unwrap();
    store
}

/// The project list's rows: the left third of the screen
fn listed(app: &crate::app::App) -> String {
    screen(app, W, H)
        .lines()
        .map(|line| line.chars().take(36).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn archived_projects_leave_the_list_until_shown() {
    let store = store_with_finished_project();
    let mut app = projects_app(&store);
    // "Audit" sorts first and is selected
    app.handle_key(Key::Char('A'));
    let list = listed(&app);
    assert!(!list.contains("Audit"), "{list}");
    assert!(list.contains("Website"), "{list}");

    app.handle_key(Key::Char('H'));
    let screen = screen(&app, W, H);
    assert!(screen.contains("Projects + archived"), "{screen}");
    assert!(screen.contains("Audit"), "{screen}");

    // The cursor stayed on Website; Audit is listed above it again. Restore it.
    app.handle_key(Key::Char('k'));
    app.handle_key(Key::Char('A'));
    assert!(!store.repository().list().unwrap()[0].is_archived());
}

#[test]
fn unfinished_projects_are_not_archived() {
    let store = store_with_finished_project();
    let mut app = projects_app(&store);
    app.handle_key(Key::Char('j'));
    app.handle_key(Key::Char('A'));
    let screen = screen(&app, W, H);
    assert!(screen.contains("only finished projects"), "{screen}");
    assert!(
        store
            .repository()
            .list()
            .unwrap()
            .iter()
            .all(|s| !s.is_archived())
    );
}

#[test]
fn search_narrows_the_list_by_name_or_client() {
    let store = store_with_finished_project();
    let mut app = projects_app(&store);
    app.handle_key(Key::Char('/'));
    type_keys(&mut app, "acme\n");
    let list = listed(&app);
    assert!(list.contains("Website"), "{list}");
    assert!(!list.contains("Audit"), "{list}");

    app.handle_key(Key::Char('/'));
    for _ in 0.."acme".len() {
        app.handle_key(Key::Backspace);
    }
    type_keys(&mut app, "nothing\n");
    let none = screen(&app, W, H);
    assert!(none.contains("No projects match the search"), "{none}");

    app.handle_key(Key::Esc);
    assert!(screen(&app, W, H).contains("Search cleared"));
    let list = listed(&app);
    assert!(list.contains("Website") && list.contains("Audit"), "{list}");
}
