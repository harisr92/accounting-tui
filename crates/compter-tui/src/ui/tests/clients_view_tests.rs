use crate::ui::key::Key;

use crate::ui::tests::support::{clients_app, screen, type_keys};
use compter_core::demo;
use compter_core::services::ProjectService;
use compter_core::storage::ProjectRepository;

const W: u16 = 130;
const H: u16 = 36;

#[test]
fn clients_list_their_place_projects_and_what_they_owe() {
    let store = demo::store().unwrap();
    let mut app = clients_app(&store);
    app.handle_key(Key::Char('2'));
    let screen = screen(&app, W, H);
    for expected in [
        "Acme Pvt Ltd",
        "27AAPFU0939F2ZU",
        "₹35,400.00",
        "Globex Labs",
        "Karnataka (29)",
        "₹18,585.00",
        "Initrode GmbH",
        "Germany",
        "Umbrella Analytics",
        "₹41,300.00",
    ] {
        assert!(screen.contains(expected), "missing {expected:?}\n{screen}");
    }
}

#[test]
fn opening_a_client_shows_their_projects_and_invoices_until_esc() {
    let store = demo::store().unwrap();
    let mut app = clients_app(&store);
    app.handle_key(Key::Char('2'));
    app.handle_key(Key::Enter);
    let opened = screen(&app, W, H);
    for expected in [
        "Projects (1)",
        "Website Revamp",
        "Invoices (2)",
        "Frontend",
        "Design",
        "Plot 7, MIDC, Pune 411019",
        "e edit client",
    ] {
        assert!(opened.contains(expected), "missing {expected:?}\n{opened}");
    }
    assert!(!opened.contains("Umbrella Analytics"), "{opened}");

    // Tab moves focus between the panels; Esc goes back to the list
    app.handle_key(Key::Tab);
    app.handle_key(Key::Char('j'));
    app.handle_key(Key::Esc);
    let list = screen(&app, W, H);
    assert!(list.contains("Umbrella Analytics"), "{list}");
}

#[test]
fn search_narrows_the_clients() {
    let store = demo::store().unwrap();
    let mut app = clients_app(&store);
    app.handle_key(Key::Char('2'));
    type_keys(&mut app, "/germany\n");
    let found = screen(&app, W, H);
    assert!(found.contains("Initrode GmbH"), "{found}");
    assert!(!found.contains("Acme Pvt Ltd"), "{found}");

    // Enter opens the one match
    app.handle_key(Key::Enter);
    let opened = screen(&app, W, H);
    assert!(opened.contains("API Audit"), "{opened}");
    assert!(opened.contains("€1,000.00"), "{opened}");

    app.handle_key(Key::Esc);
    app.handle_key(Key::Esc);
    let cleared = screen(&app, W, H);
    assert!(cleared.contains("Acme Pvt Ltd"), "{cleared}");
}

#[test]
fn the_list_follows_changes_made_on_other_tabs() {
    let store = demo::store().unwrap();
    let mut app = clients_app(&store);
    app.handle_key(Key::Char('2'));
    // A new project for Acme, saved behind the tab's back
    let mut projects = compter_core::services::ProjectService::new(store.repository());
    let acme = projects.clients().unwrap().remove(0);
    projects
        .create(compter_core::services::ProjectDraft {
            name: "Brochure".to_owned(),
            client: acme.details(),
            billing: compter_core::domain::Billing::Fixed,
        })
        .unwrap();
    app.handle_key(Key::Char('1'));
    app.handle_key(Key::Char('2'));
    app.handle_key(Key::Enter);
    let opened = screen(&app, W, H);
    assert!(opened.contains("Projects (2)"), "{opened}");
    assert!(opened.contains("Brochure"), "{opened}");
}

/// The demo's Acme opened and its edit form on the first address line, cleared
fn edit_acme_address(app: &mut crate::app::App) {
    app.handle_key(Key::Char('2'));
    app.handle_key(Key::Enter);
    app.handle_key(Key::Char('e'));
    type_keys(app, "\t\t\t\t");
    app.handle_key(Key::Ctrl('u'));
}

#[test]
fn editing_a_client_asks_whether_their_open_projects_change_too() {
    let store = demo::store().unwrap();
    let mut app = clients_app(&store);
    edit_acme_address(&mut app);
    let form = screen(&app, W, H);
    assert!(form.contains("Edit client"), "{form}");
    assert!(form.contains("27AAPFU0939F2ZU"), "{form}");

    type_keys(&mut app, "Plot 9, MIDC\n");
    let asked = screen(&app, W, H);
    assert!(asked.contains("Website Revamp"), "{asked}");
    assert!(asked.contains("y yes"), "{asked}");

    app.handle_key(Key::Char('y'));
    let saved = screen(&app, W, H);
    assert!(saved.contains("Plot 9, MIDC, Pune 411019"), "{saved}");
    let projects = ProjectService::new(store.repository());
    let website = projects.find_by_name("Website Revamp").unwrap();
    assert_eq!(
        store.repository().load(website).unwrap().client_address(),
        ["Plot 9, MIDC", "Pune 411019"]
    );
}

#[test]
fn answering_no_saves_the_client_only() {
    let store = demo::store().unwrap();
    let mut app = clients_app(&store);
    edit_acme_address(&mut app);
    type_keys(&mut app, "Plot 9, MIDC\nn");

    let saved = screen(&app, W, H);
    assert!(saved.contains("Plot 9, MIDC, Pune 411019"), "{saved}");
    let projects = ProjectService::new(store.repository());
    let website = projects.find_by_name("Website Revamp").unwrap();
    assert_eq!(
        store.repository().load(website).unwrap().client_address(),
        ["Plot 7, MIDC", "Pune 411019"]
    );
}

#[test]
fn a_client_with_no_open_projects_is_saved_without_asking() {
    let store = demo::store().unwrap();
    let mut projects = ProjectService::new(store.repository());
    let audit = projects.find_by_name("API Audit").unwrap();
    projects.set_archived(audit, true).unwrap();
    let mut app = clients_app(&store);
    app.handle_key(Key::Char('2'));
    type_keys(&mut app, "/germany\n");
    app.handle_key(Key::Enter);
    app.handle_key(Key::Char('e'));
    type_keys(&mut app, "\t\t\t\t");
    app.handle_key(Key::Ctrl('u'));
    type_keys(&mut app, "Hauptstrasse 2\n");

    let saved = screen(&app, W, H);
    assert!(saved.contains("Hauptstrasse 2, 10115 Berlin"), "{saved}");
    assert!(!saved.contains("y yes"), "{saved}");
    // The archived project keeps the address it was billed at
    assert_eq!(
        store.repository().load(audit).unwrap().client_address(),
        ["Hauptstrasse 1", "10115 Berlin"]
    );
}

#[test]
fn a_bad_gstin_keeps_the_edit_form_open() {
    let store = demo::store().unwrap();
    let mut app = clients_app(&store);
    app.handle_key(Key::Char('2'));
    app.handle_key(Key::Enter);
    app.handle_key(Key::Char('e'));
    app.handle_key(Key::Ctrl('u'));
    type_keys(&mut app, "27NOTAGSTIN\n");

    let form = screen(&app, W, H);
    assert!(form.contains("Edit client"), "{form}");
    assert!(form.contains("27NOTAGSTIN"), "{form}");
    app.handle_key(Key::Esc);
    let opened = screen(&app, W, H);
    assert!(opened.contains("27AAPFU0939F2ZU"), "{opened}");
}

#[test]
fn esc_on_the_question_goes_back_to_the_form_and_saves_nothing() {
    let store = demo::store().unwrap();
    let mut app = clients_app(&store);
    edit_acme_address(&mut app);
    type_keys(&mut app, "Plot 9, MIDC\n");
    app.handle_key(Key::Esc);

    let form = screen(&app, W, H);
    assert!(form.contains("Edit client"), "{form}");
    assert!(form.contains("Plot 9, MIDC"), "{form}");
    app.handle_key(Key::Esc);
    let opened = screen(&app, W, H);
    assert!(opened.contains("Plot 7, MIDC, Pune 411019"), "{opened}");
}

/// The demo's Acme opened and renamed `name`, the form cleared first
fn rename_acme(app: &mut crate::app::App, name: &str) {
    app.handle_key(Key::Char('2'));
    app.handle_key(Key::Enter);
    app.handle_key(Key::Char('r'));
    app.handle_key(Key::Ctrl('u'));
    type_keys(app, name);
    app.handle_key(Key::Enter);
}

#[test]
fn renaming_a_client_keeps_the_old_name_as_an_alias() {
    let store = demo::store().unwrap();
    let mut app = clients_app(&store);
    rename_acme(&mut app, "Acme Private Limited");
    let asked = screen(&app, W, H);
    assert!(asked.contains("Website Revamp"), "{asked}");

    app.handle_key(Key::Char('y'));
    let renamed = screen(&app, W, H);
    assert!(renamed.contains("Acme Private Limited"), "{renamed}");
    assert!(renamed.contains("Also known as Acme Pvt Ltd"), "{renamed}");
    assert!(renamed.contains("Invoices (2)"), "{renamed}");
    let projects = ProjectService::new(store.repository());
    let website = projects.find_by_name("Website Revamp").unwrap();
    assert_eq!(
        store.repository().load(website).unwrap().client_name(),
        "Acme Private Limited"
    );

    // The list finds them by the old name too
    app.handle_key(Key::Esc);
    type_keys(&mut app, "/pvt ltd\n");
    let found = screen(&app, W, H);
    assert!(found.contains("Acme Private Limited"), "{found}");
}

#[test]
fn a_name_another_client_has_keeps_the_rename_form_open() {
    let store = demo::store().unwrap();
    let mut app = clients_app(&store);
    rename_acme(&mut app, "globex labs");

    let form = screen(&app, W, H);
    assert!(form.contains("Rename client"), "{form}");
    assert!(form.contains("already goes by that name"), "{form}");
}

#[test]
fn merging_a_duplicate_asks_and_opens_the_client_kept() {
    let store = demo::store().unwrap();
    let mut projects = ProjectService::new(store.repository());
    projects
        .create(compter_core::services::ProjectDraft {
            name: "Brochure".to_owned(),
            client: compter_core::domain::ClientDetails::named("Acme", None),
            billing: compter_core::domain::Billing::Fixed,
        })
        .unwrap();
    let mut app = clients_app(&store);
    app.handle_key(Key::Char('2'));
    // "Acme" sorts first
    app.handle_key(Key::Enter);
    app.handle_key(Key::Char('m'));
    let picker = screen(&app, W, H);
    assert!(picker.contains("Merge into"), "{picker}");
    assert!(picker.contains("Acme Pvt Ltd"), "{picker}");

    type_keys(&mut app, "pvt");
    app.handle_key(Key::Enter);
    let asked = screen(&app, W, H);
    assert!(asked.contains("Brochure"), "{asked}");

    app.handle_key(Key::Char('y'));
    let merged = screen(&app, W, H);
    assert!(merged.contains("Projects (2)"), "{merged}");
    assert!(merged.contains("Also known as Acme"), "{merged}");
    let brochure = projects.find_by_name("Brochure").unwrap();
    assert_eq!(
        store.repository().load(brochure).unwrap().client_name(),
        "Acme Pvt Ltd"
    );

    app.handle_key(Key::Esc);
    let list = screen(&app, W, H);
    assert!(!list.contains("Acme  "), "{list}");
}

#[test]
fn a_merge_across_gstins_is_refused_and_the_picker_stays() {
    let store = demo::store().unwrap();
    let mut app = clients_app(&store);
    app.handle_key(Key::Char('2'));
    app.handle_key(Key::Enter);
    app.handle_key(Key::Char('m'));
    type_keys(&mut app, "umbrella");
    app.handle_key(Key::Enter);

    let refused = screen(&app, W, H);
    assert!(refused.contains("different GSTIN"), "{refused}");
    assert!(refused.contains("Merge into"), "{refused}");
    app.handle_key(Key::Esc);
    assert_eq!(
        compter_core::services::ClientService::new(store.repository())
            .list()
            .unwrap()
            .len(),
        4
    );
}

#[test]
fn a_client_left_without_projects_is_removed_after_asking() {
    let store = demo::store().unwrap();
    let mut projects = ProjectService::new(store.repository());
    let project = projects
        .create(compter_core::services::ProjectDraft {
            name: "Brochure".to_owned(),
            client: compter_core::domain::ClientDetails::named("Aardvark", None),
            billing: compter_core::domain::Billing::Fixed,
        })
        .unwrap();
    projects.delete(project.id()).unwrap();
    let mut app = clients_app(&store);
    app.handle_key(Key::Char('2'));
    // "Aardvark" sorts first
    app.handle_key(Key::Enter);
    app.handle_key(Key::Char('x'));
    let asked = screen(&app, W, H);
    assert!(asked.contains("Remove Aardvark from clients?"), "{asked}");

    // No backs out to their page
    app.handle_key(Key::Char('n'));
    let kept = screen(&app, W, H);
    assert!(kept.contains("Projects (0)"), "{kept}");

    app.handle_key(Key::Char('x'));
    app.handle_key(Key::Char('y'));
    let list = screen(&app, W, H);
    assert!(list.contains("Aardvark removed from clients"), "{list}");
    assert!(list.contains("Acme Pvt Ltd"), "{list}");
    assert_eq!(
        compter_core::services::ClientService::new(store.repository())
            .list()
            .unwrap()
            .len(),
        4
    );
}

#[test]
fn a_client_with_projects_is_not_removed() {
    let store = demo::store().unwrap();
    let mut app = clients_app(&store);
    app.handle_key(Key::Char('2'));
    app.handle_key(Key::Enter);
    app.handle_key(Key::Char('x'));

    let refused = screen(&app, W, H);
    assert!(refused.contains("only a client with none"), "{refused}");
    assert!(!refused.contains("Remove Acme"), "{refused}");
    assert!(refused.contains("Projects ("), "{refused}");
}

#[test]
fn an_address_keeps_every_line_through_an_edit() {
    let store = demo::store().unwrap();
    let mut app = clients_app(&store);
    edit_acme_address(&mut app);
    type_keys(&mut app, "Plot 9, MIDC");
    // Past the second line onto a new third one
    app.handle_key(Key::Down);
    app.handle_key(Key::Down);
    type_keys(&mut app, "Maharashtra\nn");

    let projects = ProjectService::new(store.repository());
    let website = projects.find_by_name("Website Revamp").unwrap();
    // The project kept its own copy; the client has all three lines
    assert_eq!(
        store.repository().load(website).unwrap().client_address(),
        ["Plot 7, MIDC", "Pune 411019"]
    );
    app.handle_key(Key::Char('e'));
    let form = screen(&app, W, H);
    // A row each in the form, not joined onto two
    for line in [
        "│  Address     Plot 9, MIDC  ",
        "│              Pune 411019  ",
        "│              Maharashtra  ",
    ] {
        assert!(form.contains(line), "missing {line:?}\n{form}");
    }
}
