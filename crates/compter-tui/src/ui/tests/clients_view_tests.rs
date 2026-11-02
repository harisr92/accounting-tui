use crate::ui::key::Key;

use crate::ui::tests::support::{clients_app, screen, type_keys};
use compter_core::demo;

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
