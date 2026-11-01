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
