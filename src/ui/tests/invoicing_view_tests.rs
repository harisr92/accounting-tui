use crate::ui::key::Key;
use bigdecimal::BigDecimal;
use chrono::NaiveDate;

use crate::domain::{
    Billing, BusinessProfile, ClientDetails, ClientRegion, Currency, InvoiceStatus,
    MilestoneStatus, ProfileDraft, Project,
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
        ..Default::default()
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
    assert!(screen.contains("Settings tab first"), "{screen}");
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
    app.handle_key(Key::Enter);

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

#[test]
fn part_payments_show_their_history_and_balance() {
    let (store, project) = store_with_milestone(true);
    let mut app = invoicing_app(&store);
    type_keys(&mut app, "\tiy");
    app.handle_key(Key::Char('2'));

    // ₹59,000 due: receive 20,000 now
    app.handle_key(Key::Char('p'));
    app.handle_key(Key::Tab);
    app.handle_key(Key::Ctrl('u'));
    type_keys(&mut app, "20000\n");
    let screen_after = screen(&app, W, H);
    assert!(
        screen_after.contains("₹39,000.00 still due"),
        "{screen_after}"
    );
    assert!(screen_after.contains("PART PAID"));
    assert!(screen_after.contains("Payments for INV/"));
    assert!(screen_after.contains("₹20,000.00"));
    assert_eq!(
        milestone_status(&store, &project),
        MilestoneStatus::Invoiced
    );

    // The form now offers the balance; accepting it settles the invoice
    app.handle_key(Key::Char('p'));
    assert!(screen(&app, W, H).contains("39000"));
    app.handle_key(Key::Enter);
    assert!(screen(&app, W, H).contains("paid in full"));
    assert_eq!(milestone_status(&store, &project), MilestoneStatus::Paid);
}

#[test]
fn settings_currency_is_the_default_for_new_projects() {
    let store = Store::in_memory().unwrap();
    store
        .repository()
        .save_profile(&registered_profile())
        .unwrap();
    let mut app = invoicing_app(&store);
    app.handle_key(Key::Char('3'));
    app.handle_key(Key::Char('e'));
    // Past name, address, GSTIN, SAC, prefix, LUT and terms to Currency: INR -> USD
    type_keys(&mut app, "\t\t\t\t\t\t\t\t\t\t ");
    assert!(screen(&app, W, H).contains("USD $"));
    app.handle_key(Key::Enter);
    let profile = store.repository().load_profile().unwrap().unwrap();
    assert_eq!(profile.currency(), Currency::Usd);
    assert!(screen(&app, W, H).contains("default for new projects"));

    // A new project starts in dollars: one for a client abroad is saved in them
    app.handle_key(Key::Char('1'));
    app.handle_key(Key::Char('n'));
    type_keys(
        &mut app,
        "Audit\tInitrode\t\t \t\tGermany\tHauptstr. 1\tBerlin\n",
    );
    let project = &store.repository().list().unwrap()[0];
    let project = store.repository().load(project.id()).unwrap();
    assert_eq!(project.currency(), Currency::Usd);

    // ...and one in India is refused until it is switched to rupees
    app.handle_key(Key::Char('n'));
    type_keys(&mut app, "Site\tAcme\n");
    let refused = screen(&app, W, H);
    assert!(refused.contains("billed in INR"), "{refused}");
}

/// A registered seller with an LUT, and a $1,000 milestone for a client in Germany
fn store_with_dollar_milestone() -> (Store, Project) {
    let store = Store::in_memory().unwrap();
    let mut repo = store.repository();
    let mut draft = registered_profile().to_draft();
    draft.lut_arn = Some("AD2710260012345".to_owned());
    repo.save_profile(&BusinessProfile::try_from(draft).unwrap())
        .unwrap();
    let client = ClientDetails {
        name: "Initrode".to_owned(),
        address: vec!["Hauptstr. 1".to_owned(), "Berlin".to_owned()],
        region: ClientRegion::Abroad {
            country: "Germany".to_owned(),
        },
        currency: Currency::Usd,
        ..ClientDetails::default()
    };
    let mut project = Project::for_client("Audit", client, Billing::Fixed).unwrap();
    let due = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
    project
        .add_milestone("Report", due, Some(BigDecimal::from(1_000)))
        .unwrap();
    repo.save(&project).unwrap();
    (store, project)
}

#[test]
fn dollar_invoices_ask_for_the_rate_and_payments_for_rupees_banked() {
    let (store, project) = store_with_dollar_milestone();
    let mut app = invoicing_app(&store);
    let milestones = screen(&app, W, H);
    assert!(milestones.contains("$1,000.00"), "{milestones}");

    type_keys(&mut app, "\ti");
    assert!(screen(&app, W, H).contains("Exchange rate"));
    type_keys(&mut app, "abc\n");
    assert!(screen(&app, W, H).contains("amount must be a number"));
    app.handle_key(Key::Ctrl('u'));
    type_keys(&mut app, "83.5\n");
    let question = screen(&app, W, H);
    assert!(question.contains("$1,000.00"), "{question}");
    assert!(question.contains("₹83,500.00 in"), "{question}");
    app.handle_key(Key::Char('y'));
    assert_eq!(
        milestone_status(&store, &project),
        MilestoneStatus::Invoiced
    );

    app.handle_key(Key::Char('2'));
    let invoices = screen(&app, W, H);
    assert!(invoices.contains("outstanding ₹83,500.00"), "{invoices}");
    app.handle_key(Key::Char('p'));
    assert!(screen(&app, W, H).contains("Banked ₹"));
    type_keys(&mut app, "\t\t84,000\n");
    let paid = screen(&app, W, H);
    assert!(paid.contains("paid in full"), "{paid}");
    assert!(paid.contains("Banked"));
    let summary = &store.repository().list_invoices().unwrap()[0];
    assert_eq!(summary.status(), InvoiceStatus::Paid);
    assert_eq!(milestone_status(&store, &project), MilestoneStatus::Paid);
}
