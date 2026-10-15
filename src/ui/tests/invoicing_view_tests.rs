use bigdecimal::BigDecimal;
use chrono::NaiveDate;
use termion::event::Key;

use crate::domain::{
    Billing, BusinessProfile, ClientDetails, MilestoneStatus, ProfileDraft, Project,
};
use crate::storage::{InvoiceRepository, ProjectRepository, SettingsRepository, Store};
use crate::ui::tests::support::{invoicing_app, screen, type_keys};

const W: u16 = 120;
const H: u16 = 32;

fn registered_profile() -> BusinessProfile {
    BusinessProfile::try_from(ProfileDraft {
        name: "Studio".to_owned(),
        address: vec!["Pune".to_owned()],
        gstin: Some("27AAPFU0939F1ZV".to_owned()),
        sac_code: "998314".to_owned(),
        invoice_prefix: "INV".to_owned(),
        lut_arn: None,
        terms: Vec::new(),
    })
    .unwrap()
}

/// A ₹50,000 milestone for a client in another state
fn store_with_milestone(with_profile: bool) -> (Store, Project) {
    let store = Store::in_memory().unwrap();
    let mut repo = store.repository();
    if with_profile {
        repo.save_profile(&registered_profile()).unwrap();
    }
    let client = ClientDetails::named("Acme", Some("29AAPFU0939F1ZR"));
    let mut project = Project::for_client("Site", client, Billing::Fixed).unwrap();
    let due = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
    project
        .add_milestone("Design", due, Some(BigDecimal::from(50_000)))
        .unwrap();
    repo.save(&project).unwrap();
    (store, project)
}

fn milestone_status(store: &Store, project: &Project) -> MilestoneStatus {
    store.repository().load(project.id()).unwrap().milestones()[0].status()
}

#[test]
fn invoicing_before_settings_says_where_to_go() {
    let (store, _) = store_with_milestone(false);
    let mut app = invoicing_app(&store);
    type_keys(&mut app, "\ti");
    let screen = screen(&app, W, H);
    assert!(screen.contains("Settings (tab 5) first"), "{screen}");
}

#[test]
fn settings_form_saves_the_profile() {
    let store = Store::in_memory().unwrap();
    let mut app = invoicing_app(&store);
    app.handle_key(Key::Char('3'));
    assert!(screen(&app, W, H).contains("Not set up yet"));

    app.handle_key(Key::Char('e'));
    type_keys(
        &mut app,
        "Nila Studio\t12 FC Road\t\tPune 411004\t27aapfu0939f1zv",
    );
    assert!(screen(&app, W, H).contains("IT design and development services (18%)"));
    app.handle_key(Key::Char('\n'));

    let profile = store.repository().load_profile().unwrap().unwrap();
    assert_eq!(profile.name(), "Nila Studio");
    assert_eq!(profile.address(), ["12 FC Road", "Pune 411004"]);
    assert_eq!(profile.gstin().map(|g| g.as_str()), Some("27AAPFU0939F1ZV"));
    let screen = screen(&app, W, H);
    assert!(screen.contains("Business details saved"), "{screen}");
    assert!(screen.contains("INV/"));
}

#[test]
fn issue_from_projects_then_see_it_in_invoices() {
    let (store, project) = store_with_milestone(true);
    let mut app = invoicing_app(&store);
    type_keys(&mut app, "\ti");
    let question = screen(&app, W, H);
    assert!(question.contains("Issue INV/"), "{question}");
    assert!(question.contains("₹59,000.00"));
    assert!(question.contains("IGST 9,000.00"));

    app.handle_key(Key::Char('y'));
    assert_eq!(
        milestone_status(&store, &project),
        MilestoneStatus::Invoiced
    );
    assert!(screen(&app, W, H).contains("Issued INV/"));

    app.handle_key(Key::Char('2'));
    let invoices = screen(&app, W, H);
    assert!(invoices.contains("/001"), "{invoices}");
    assert!(invoices.contains("ISSUED"));
    assert!(invoices.contains("outstanding ₹59,000.00"));
}

#[test]
fn payment_must_add_up_and_shows_in_projects() {
    let (store, project) = store_with_milestone(true);
    let mut app = invoicing_app(&store);
    type_keys(&mut app, "\tiy");
    app.handle_key(Key::Char('2'));

    app.handle_key(Key::Char('p'));
    type_keys(&mut app, "\t\t");
    app.handle_key(Key::Ctrl('u'));
    type_keys(&mut app, "5000\n");
    let mismatch = screen(&app, W, H);
    assert!(mismatch.contains("received plus TDS"), "{mismatch}");
    assert!(mismatch.contains("Record payment"));

    app.handle_key(Key::BackTab);
    app.handle_key(Key::Ctrl('u'));
    type_keys(&mut app, "54000\n");
    assert!(store.repository().list_invoices().unwrap()[0].is_paid());
    assert!(screen(&app, W, H).contains("PAID"));

    app.handle_key(Key::Char('1'));
    assert_eq!(milestone_status(&store, &project), MilestoneStatus::Paid);
    assert!(screen(&app, W, H).contains("PAID"));
}

#[test]
fn locked_milestones_still_take_the_payment_key() {
    let (store, _) = store_with_milestone(true);
    let mut app = invoicing_app(&store);
    type_keys(&mut app, "\tp");
    assert!(screen(&app, W, H).contains("Invoice the milestone first"));
    type_keys(&mut app, "iyp");
    assert!(screen(&app, W, H).contains("Record payment"));
}
