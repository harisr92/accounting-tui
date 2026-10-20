use crate::ui::key::Key;
use bigdecimal::BigDecimal;

use crate::ui::tests::support::{projects_app, screen, type_keys};
use chrono::NaiveDate;
use compter_core::demo;
use compter_core::domain::{
    Billing, BusinessProfile, ClientDetails, MilestoneStatus, ProfileDraft, Project,
};
use compter_core::storage::{ProjectRepository, SettingsRepository, Store};

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
fn hourly_projects_show_the_time_logged_on_every_milestone() {
    let store = Store::in_memory().unwrap();
    let mut app = projects_app(&store);
    create_project(&mut app, &hourly_project_keys("App", "Globex", "1500"));
    app.handle_key(Key::Tab);
    app.handle_key(Key::Char('a'));
    type_keys(&mut app, "Prototype\n");
    app.handle_key(Key::Char('a'));
    type_keys(&mut app, "Launch\n");
    for _ in 0..2 {
        app.handle_key(Key::Char('t'));
        type_keys(&mut app, "\t1h\tbuild\n");
        app.handle_key(Key::Char('j'));
    }
    app.handle_key(Key::Char('t'));
    type_keys(&mut app, "\t15m\tfix\n");

    let screen = screen(&app, W, H);
    assert!(screen.contains("Logged"), "{screen}");
    assert!(screen.contains("2h 15m"), "{screen}");
}

#[test]
fn fixed_price_projects_show_no_logged_time() {
    let store = demo::store().unwrap();
    let app = projects_app(&store);
    let screen = screen(&app, W, H);
    assert!(screen.contains("Fixed price"), "{screen}");
    assert!(!screen.contains("Logged"), "{screen}");
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
fn archived_projects_are_listed_on_their_own() {
    let store = store_with_finished_project();
    let mut app = projects_app(&store);
    // "Audit" sorts first and is selected
    app.handle_key(Key::Char('A'));
    let list = listed(&app);
    assert!(!list.contains("Audit"), "{list}");
    assert!(list.contains("Website"), "{list}");

    app.handle_key(Key::Char('H'));
    let list = listed(&app);
    assert!(list.contains("Archived projects"), "{list}");
    assert!(list.contains("Audit"), "{list}");
    assert!(!list.contains("Website"), "{list}");

    // Restored, it leaves the archived list for the other one
    app.handle_key(Key::Char('A'));
    assert!(!store.repository().list().unwrap()[0].is_archived());
    let screen = screen(&app, W, H);
    assert!(screen.contains("No archived projects"), "{screen}");

    app.handle_key(Key::Char('H'));
    let list = listed(&app);
    assert!(list.contains("Audit") && list.contains("Website"), "{list}");
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

/// A registered seller and one fixed project whose milestones were added in this order:
/// "launch" due 20 Oct ₹500, "Design" due 5 Oct ₹900, "build" due 12 Oct ₹700. Listed by due
/// date that is Design, build, launch; by name build, Design, launch; by amount Design, build,
/// launch.
fn store_with_three_milestones() -> (Store, Project) {
    let store = Store::in_memory().unwrap();
    let mut repo = store.repository();
    let profile = BusinessProfile::try_from(ProfileDraft {
        name: "Studio".to_owned(),
        address: vec!["Pune".to_owned()],
        gstin: Some("27AAPFU0939F1ZV".to_owned()),
        sac_code: "998314".to_owned(),
        invoice_prefix: "INV".to_owned(),
        ..Default::default()
    })
    .unwrap();
    repo.save_profile(&profile).unwrap();
    let client = ClientDetails::named("Acme", Some("29AAPFU0939F1ZR"));
    let mut project = Project::for_client("Site", client, Billing::Fixed).unwrap();
    for (name, day, amount) in [("launch", 20, 500), ("Design", 5, 900), ("build", 12, 700)] {
        let due = NaiveDate::from_ymd_opt(2026, 10, day).unwrap();
        project
            .add_milestone(name, due, Some(BigDecimal::from(amount)))
            .unwrap();
    }
    repo.save(&project).unwrap();
    (store, project)
}

/// Stored milestones of `project` as (name, status)
fn milestones(store: &Store, project: &Project) -> Vec<(String, MilestoneStatus)> {
    let project = store.repository().load(project.id()).unwrap();
    project
        .milestones()
        .iter()
        .map(|m| (m.name().to_owned(), m.status()))
        .collect()
}

#[test]
fn milestones_list_by_due_date_and_s_changes_the_order() {
    let (store, _) = store_with_three_milestones();
    let mut app = projects_app(&store);
    app.handle_key(Key::Tab);
    let by_due = screen(&app, W, H);
    assert!(by_due.contains("Milestones · by due date"), "{by_due}");
    let at = |text: &str, name: &str| text.find(name).unwrap();
    assert!(at(&by_due, "Design") < at(&by_due, "build"));
    assert!(at(&by_due, "build") < at(&by_due, "launch"));

    app.handle_key(Key::Char('s'));
    let by_name = screen(&app, W, H);
    assert!(by_name.contains("Milestones · by name"), "{by_name}");
    assert!(at(&by_name, "build") < at(&by_name, "Design"));
    app.handle_key(Key::Char('s'));
    assert!(screen(&app, W, H).contains("Milestones · by amount"));
    app.handle_key(Key::Char('s'));
    assert!(screen(&app, W, H).contains("Milestones · by due date"));
}

#[test]
fn invoicing_after_sorting_bills_the_highlighted_milestone() {
    let (store, project) = store_with_three_milestones();
    let mut app = projects_app(&store);
    // By due date the second row is "build"; sorting by name keeps the cursor on it
    type_keys(&mut app, "\tjs");
    let sorted = screen(&app, W, H);
    assert!(sorted.contains("> build"), "{sorted}");
    // Three are pending, so i opens the picker with the highlighted one picked
    app.handle_key(Key::Char('i'));
    let picker = screen(&app, W, H);
    assert!(picker.contains("Milestones to invoice"), "{picker}");
    assert!(picker.contains("1 picked"), "{picker}");
    type_keys(&mut app, "\n\ny");
    assert_eq!(
        milestones(&store, &project),
        [
            ("launch".to_owned(), MilestoneStatus::Pending),
            ("Design".to_owned(), MilestoneStatus::Pending),
            ("build".to_owned(), MilestoneStatus::Invoiced),
        ]
    );
}

#[test]
fn editing_and_deleting_after_sorting_change_the_highlighted_milestone() {
    let (store, project) = store_with_three_milestones();
    let mut app = projects_app(&store);
    // By amount: Design, build, launch. The first row is Design.
    type_keys(&mut app, "\tss");
    app.handle_key(Key::Char('e'));
    app.handle_key(Key::Ctrl('u'));
    type_keys(&mut app, "Design v2\n");
    let names: Vec<String> = milestones(&store, &project)
        .into_iter()
        .map(|(n, _)| n)
        .collect();
    assert_eq!(names, ["launch", "Design v2", "build"]);

    // The last row is launch
    type_keys(&mut app, "jjxy");
    let names: Vec<String> = milestones(&store, &project)
        .into_iter()
        .map(|(n, _)| n)
        .collect();
    assert_eq!(names, ["Design v2", "build"]);
}

#[test]
fn a_new_due_date_moves_the_milestone_and_the_cursor_with_it() {
    let (store, project) = store_with_three_milestones();
    let mut app = projects_app(&store);
    // By due date the first row is Design; push it past launch
    type_keys(&mut app, "\te\t");
    app.handle_key(Key::Ctrl('u'));
    type_keys(&mut app, "2026-10-25\n");
    let moved = screen(&app, W, H);
    assert!(moved.contains("> Design"), "{moved}");
    // Deleting acts on it, not on what took its old row
    type_keys(&mut app, "xy");
    let names: Vec<String> = milestones(&store, &project)
        .into_iter()
        .map(|(n, _)| n)
        .collect();
    assert_eq!(names, ["launch", "build"]);
}

#[test]
fn a_new_milestone_is_selected_where_the_sort_lists_it() {
    let (store, _) = store_with_three_milestones();
    let mut app = projects_app(&store);
    // Due before every other one, so it is listed first, not last
    type_keys(&mut app, "\tjjaKickoff\t2026-10-01\t100\n");
    let added = screen(&app, W, H);
    assert!(added.contains("> Kickoff"), "{added}");
}

#[test]
fn a_picks_every_pending_milestone_for_one_invoice() {
    let (store, project) = store_with_three_milestones();
    let mut app = projects_app(&store);
    type_keys(&mut app, "\ti");
    // Picking none is refused, with the list kept open
    type_keys(&mut app, " \n");
    let refused = screen(&app, W, H);
    assert!(refused.contains("Pick at least one milestone"), "{refused}");

    type_keys(&mut app, "a\n\n");
    let question = screen(&app, W, H);
    assert!(question.contains("billing 3 milestones"), "{question}");
    // ₹2,100 + IGST 18%
    assert!(question.contains("₹2,478.00"), "{question}");
    app.handle_key(Key::Char('y'));
    assert!(
        milestones(&store, &project)
            .iter()
            .all(|(_, status)| *status == MilestoneStatus::Invoiced)
    );
    let project = store.repository().load(project.id()).unwrap();
    assert!(
        project
            .milestones()
            .iter()
            .all(|m| m.invoice_number() == Some("INV/26-27/001"))
    );
}

#[test]
fn the_last_pending_milestone_is_invoiced_without_a_picker() {
    let (store, project) = store_with_three_milestones();
    let mut app = projects_app(&store);
    // Invoice two of them together, leaving launch
    type_keys(&mut app, "\tia ");
    // a picked all three; Space on the cursor's row (Design) unpicks it
    let picker = screen(&app, W, H);
    assert!(picker.contains("2 picked"), "{picker}");
    type_keys(&mut app, "\n\ny");
    assert_eq!(
        milestones(&store, &project),
        [
            ("launch".to_owned(), MilestoneStatus::Invoiced),
            ("Design".to_owned(), MilestoneStatus::Pending),
            ("build".to_owned(), MilestoneStatus::Invoiced),
        ]
    );
    type_keys(&mut app, "i\n");
    let question = screen(&app, W, H);
    assert!(question.contains("Issue INV/26-27/002"), "{question}");
}

/// The milestone table's part of the screen: right of the project list
fn milestone_panel(app: &crate::app::App) -> String {
    screen(app, W, H)
        .lines()
        .map(|line| line.chars().skip(36).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn milestone_search_narrows_rows_by_name_or_amount() {
    let (store, _) = store_with_three_milestones();
    let mut app = projects_app(&store);
    type_keys(&mut app, "\t/des\n");
    let by_name = milestone_panel(&app);
    assert!(
        by_name.contains("Milestones · by due date /des"),
        "{by_name}"
    );
    assert!(by_name.contains("Design"), "{by_name}");
    assert!(
        !by_name.contains("build") && !by_name.contains("launch"),
        "{by_name}"
    );

    // An amount matches as typed plainly or as shown
    for amount in ["700", "₹700.00"] {
        app.handle_key(Key::Char('/'));
        app.handle_key(Key::Ctrl('u'));
        type_keys(&mut app, &format!("{amount}\n"));
        let by_amount = milestone_panel(&app);
        assert!(by_amount.contains("build"), "{amount}: {by_amount}");
        assert!(!by_amount.contains("Design"), "{amount}: {by_amount}");
    }

    app.handle_key(Key::Char('/'));
    app.handle_key(Key::Ctrl('u'));
    type_keys(&mut app, "nothing\n");
    let none = screen(&app, W, H);
    assert!(none.contains("No milestones match the search"), "{none}");
}

#[test]
fn milestone_actions_follow_the_filtered_rows() {
    let (store, project) = store_with_three_milestones();
    let mut app = projects_app(&store);
    // The only row left is launch, stored first but listed last by due date
    type_keys(&mut app, "\t/launch\nxy");
    let names: Vec<String> = milestones(&store, &project)
        .into_iter()
        .map(|(n, _)| n)
        .collect();
    assert_eq!(names, ["Design", "build"]);
}

#[test]
fn invoicing_a_filtered_milestone_still_offers_every_pending_one() {
    let (store, _) = store_with_three_milestones();
    let mut app = projects_app(&store);
    type_keys(&mut app, "\t/build\ni");
    let picker = screen(&app, W, H);
    assert!(picker.contains("Milestones to invoice"), "{picker}");
    assert!(
        picker.contains("Design") && picker.contains("launch"),
        "{picker}"
    );
    assert!(picker.contains("1 picked"), "{picker}");
}

#[test]
fn esc_clears_the_milestone_search_and_keeps_the_cursor() {
    let (store, _) = store_with_three_milestones();
    let mut app = projects_app(&store);
    type_keys(&mut app, "\t/launch\n");
    app.handle_key(Key::Esc);
    let cleared = milestone_panel(&app);
    assert!(
        cleared.contains("Design") && cleared.contains("build"),
        "{cleared}"
    );
    assert!(cleared.contains("> launch"), "{cleared}");
}

#[test]
fn selecting_another_project_clears_the_milestone_search() {
    let store = store_with_finished_project();
    let mut app = projects_app(&store);
    type_keys(&mut app, "\t/nothing\n");
    assert!(screen(&app, W, H).contains("No milestones match the search"));
    type_keys(&mut app, "\tj");
    let other = screen(&app, W, H);
    assert!(!other.contains("No milestones match"), "{other}");
    assert!(!other.contains("/nothing"), "{other}");
}

#[test]
fn a_renamed_milestone_the_search_hides_clears_the_search() {
    let (store, _) = store_with_three_milestones();
    let mut app = projects_app(&store);
    type_keys(&mut app, "\t/build\ne");
    app.handle_key(Key::Ctrl('u'));
    type_keys(&mut app, "Backend\n");
    let renamed = milestone_panel(&app);
    assert!(renamed.contains("> Backend"), "{renamed}");
    assert!(
        renamed.contains("Design") && renamed.contains("launch"),
        "{renamed}"
    );
}
