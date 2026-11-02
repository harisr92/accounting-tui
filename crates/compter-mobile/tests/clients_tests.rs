mod support;

use compter_mobile::api::clients::{gst_states, gstin_state};
use support::{demo, project};

#[test]
fn a_projects_client_is_offered_once_with_what_the_form_needs() {
    let (_scratch, mut app) = demo();
    app.create_project(project("Website", None)).unwrap();
    app.create_project(project("Support", Some("1,500")))
        .unwrap();

    let clients = app.clients().unwrap();
    let acme: Vec<_> = clients
        .iter()
        .filter(|c| c.name == "Acme Pvt Ltd")
        .collect();
    assert_eq!(acme.len(), 1, "{clients:?}");
    let acme = acme[0];
    assert_eq!(acme.state.as_deref(), Some("29"));
    assert_eq!(acme.summary, "Karnataka (29)");
    assert_eq!(acme.address, ["MG Road", "Bengaluru 560001"]);
    assert_eq!(acme.currency, "INR");

    let initrode = clients.iter().find(|c| c.name == "Initrode GmbH").unwrap();
    assert_eq!(initrode.country.as_deref(), Some("Germany"));
    assert_eq!(initrode.summary, "Germany · EUR");
}

#[test]
fn a_gstin_names_its_state_once_it_is_whole() {
    assert_eq!(
        gstin_state(" 29aapfu0939f1zr ".to_owned()).as_deref(),
        Some("29")
    );
    assert_eq!(gstin_state("29AAPFU".to_owned()), None);
    assert!(
        gst_states()
            .iter()
            .any(|s| s.code == "29" && s.name == "Karnataka")
    );
}

#[test]
fn a_state_other_than_the_gstins_is_refused() {
    let (_scratch, mut app) = demo();
    let mut input = project("Website", None);
    input.client_gstin = Some("27AAPFU0939F2ZU".to_owned());

    let error = app.create_project(input.clone()).unwrap_err();
    assert_eq!(error.field.as_deref(), Some("client_state"));
    assert!(error.message.contains("Maharashtra"), "{}", error.message);

    input.client_state = Some("27".to_owned());
    app.create_project(input.clone()).unwrap();
    input.client_state = None;
    app.create_project(input).unwrap();
}

#[test]
fn clients_show_their_place_projects_and_what_they_owe() {
    let (_scratch, app) = demo();
    let clients = app.client_list().unwrap();
    let names: Vec<_> = clients.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Acme Pvt Ltd",
            "Globex Labs",
            "Initrode GmbH",
            "Umbrella Analytics"
        ]
    );
    let acme = &clients[0];
    assert_eq!(acme.place.as_deref(), Some("27AAPFU0939F2ZU"));
    assert_eq!(acme.projects, 1);
    assert_eq!(acme.owed.as_deref(), Some("₹35,400.00"));
    assert_eq!(clients[1].place.as_deref(), Some("Karnataka (29)"));
    assert_eq!(clients[2].place.as_deref(), Some("Germany"));
    assert_eq!(clients[2].owed, None);
}

#[test]
fn an_opened_client_lists_their_projects_and_invoices() {
    let (_scratch, app) = demo();
    let acme = app.client_list().unwrap().remove(0);
    let detail = app.client(acme.id).unwrap();
    assert_eq!(detail.name, "Acme Pvt Ltd");
    assert_eq!(detail.address, ["Plot 7, MIDC", "Pune 411019"]);
    assert_eq!(detail.owed.as_deref(), Some("₹35,400.00"));
    let projects: Vec<_> = detail.projects.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(projects, ["Website Revamp"]);
    let milestones: Vec<_> = detail
        .invoices
        .iter()
        .map(|i| i.milestones.as_str())
        .collect();
    assert_eq!(milestones, ["Frontend", "Design"]);

    let error = app.client("not an id".to_owned()).unwrap_err();
    assert_eq!(error.field.as_deref(), Some("client_id"));
}

#[test]
fn a_corrected_client_offers_their_open_projects_and_keeps_their_invoices() {
    let (_scratch, mut app) = demo();
    let acme = app.client(app.client_list().unwrap().remove(0).id).unwrap();
    assert_eq!(acme.edit.gstin.as_deref(), Some("27AAPFU0939F2ZU"));
    let mut input = acme.edit.clone();
    input.address = vec!["Plot 9, MIDC".to_owned(), "Pune 411019".to_owned()];

    let offered = app
        .client_projects_to_update(acme.id.clone(), input.clone())
        .unwrap();
    assert_eq!(offered, ["Website Revamp"]);

    let saved = app.edit_client(acme.id.clone(), input, true).unwrap();
    assert_eq!(saved.address, ["Plot 9, MIDC", "Pune 411019"]);
    assert_eq!(saved.invoices.len(), acme.invoices.len());
    let website = app.project(saved.projects[0].id.clone()).unwrap();
    assert_eq!(website.edit.client_address, ["Plot 9, MIDC", "Pune 411019"]);
}

#[test]
fn a_bad_correction_names_its_field() {
    let (_scratch, app) = demo();
    let acme = app.client(app.client_list().unwrap().remove(0).id).unwrap();
    let mut input = acme.edit.clone();
    input.gstin = None;
    input.state = Some("99".to_owned());

    let error = app.client_projects_to_update(acme.id, input).unwrap_err();
    assert_eq!(error.field.as_deref(), Some("client_state"));
}

#[test]
fn a_renamed_client_keeps_the_old_name_as_an_alias() {
    let (_scratch, mut app) = demo();
    let acme = app.client_list().unwrap().remove(0);

    let asked = app
        .client_projects_to_rename(acme.id.clone(), "Acme Private Limited".to_owned())
        .unwrap();
    assert_eq!(asked, ["Website Revamp"]);
    let renamed = app
        .rename_client(acme.id.clone(), "Acme Private Limited".to_owned(), false)
        .unwrap();
    assert_eq!(renamed.name, "Acme Private Limited");
    assert_eq!(renamed.aliases, ["Acme Pvt Ltd"]);
    assert_eq!(renamed.invoices.len(), 2);

    let taken = app
        .rename_client(acme.id, "Globex Labs".to_owned(), false)
        .unwrap_err();
    assert_eq!(taken.field.as_deref(), Some("client_name"));
}

#[test]
fn a_duplicate_merges_into_the_client_kept() {
    let (_scratch, mut app) = demo();
    // "Acme Pvt Ltd" in Karnataka with no GSTIN is the demo's own Acme; a typo is a new one
    let mut typo = project("Brochure", None);
    typo.client_name = "Acme Pvt".to_owned();
    app.create_project(typo).unwrap();
    let clients = app.client_list().unwrap();
    let id = |name: &str| clients.iter().find(|c| c.name == name).unwrap().id.clone();
    let (from, into) = (id("Acme Pvt"), id("Acme Pvt Ltd"));

    let asked = app
        .client_projects_to_merge(from.clone(), into.clone())
        .unwrap();
    assert_eq!(asked, ["Brochure"]);
    let merged = app.merge_client(from, into, true).unwrap();
    assert_eq!(merged.name, "Acme Pvt Ltd");
    assert_eq!(merged.aliases, ["Acme Pvt"]);
    assert_eq!(merged.projects.len(), 2);
    assert_eq!(app.client_list().unwrap().len(), 4);
}
