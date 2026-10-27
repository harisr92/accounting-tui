use crate::app::App;
use crate::ui::key::Key;
use bigdecimal::BigDecimal;
use chrono::NaiveDate;

use crate::ui::tests::support::{invoicing_app, screen, settings_app, type_keys};
use compter_core::domain::{
    Billing, BusinessProfile, ClientDetails, ClientRegion, Currency, InvoiceStatus,
    MilestoneStatus, ProfileDraft, Project,
};
use compter_core::storage::sync::{SyncBackend, SyncDir};
use compter_core::storage::{
    DataDir, InvoiceRepository, ProjectRepository, SettingsRepository, Store,
};

const W: u16 = 120;
/// A 2x1 pixel RGB PNG that accounting-core can draw
const LOGO_PNG: [u8; 70] = [
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x7B, 0x40, 0xE8,
    0xDD, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x38, 0x21, 0x07, 0x04,
    0x27, 0x00, 0x07, 0x23, 0x02, 0x09, 0xE4, 0x58, 0x88, 0x39, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
];
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
    type_keys(&mut app, "\ti\n");
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
    assert!(screen(&app, W, H).contains("Discount"));
    app.handle_key(Key::Enter);
    let question = screen(&app, W, H);
    assert!(question.contains("Issue INV/"), "{question}");
    assert!(!question.contains(" off"), "{question}");
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
    type_keys(&mut app, "\ti\ny");
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
    type_keys(&mut app, "i\nyp");
    assert!(screen(&app, W, H).contains("Record payment"));
}

#[test]
fn part_payments_show_their_history_and_balance() {
    let (store, project) = store_with_milestone(true);
    let mut app = invoicing_app(&store);
    type_keys(&mut app, "\ti\ny");
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
    assert!(screen(&app, W, H).contains("₹ per unit"));
    type_keys(&mut app, "\tabc\n");
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

#[test]
fn settings_sets_and_removes_the_logo() {
    let (store, _) = store_with_milestone(true);
    let mut app = invoicing_app(&store);
    app.handle_key(Key::Char('3'));
    assert!(screen(&app, W, H).contains("none: press l"));

    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), LOGO_PNG).unwrap();
    app.handle_key(Key::Char('l'));
    type_keys(&mut app, &file.path().display().to_string());
    app.handle_key(Key::Enter);
    let shown = screen(&app, W, H);
    assert!(shown.contains("PNG, 1 KB"), "{shown}");
    let logo = store.repository().load_profile().unwrap().unwrap().logo();
    assert!(logo.is_some());

    // Editing the details keeps the logo
    app.handle_key(Key::Char('e'));
    app.handle_key(Key::Enter);
    assert_eq!(
        store.repository().load_profile().unwrap().unwrap().logo(),
        logo
    );

    app.handle_key(Key::Char('l'));
    app.handle_key(Key::Enter);
    assert!(screen(&app, W, H).contains("Logo removed"));
    assert!(
        store
            .repository()
            .load_profile()
            .unwrap()
            .unwrap()
            .logo()
            .is_none()
    );
}

#[test]
fn a_logo_waits_for_the_business_details() {
    let store = Store::in_memory().unwrap();
    let mut app = invoicing_app(&store);
    app.handle_key(Key::Char('3'));
    app.handle_key(Key::Char('l'));
    let shown = screen(&app, W, H);
    assert!(shown.contains("before a logo"), "{shown}");
}

/// `c`, then type `amount` over what is prefilled on the invoice's one line, and go on to the
/// credit form
fn credit_lines(app: &mut App, amount: &str) {
    app.handle_key(Key::Char('c'));
    app.handle_key(Key::Ctrl('u'));
    type_keys(app, amount);
    app.handle_key(Key::Enter);
}

#[test]
fn a_credit_note_is_confirmed_then_cancels_the_invoice() {
    let (store, project) = store_with_milestone(true);
    let mut app = invoicing_app(&store);
    type_keys(&mut app, "\ti\ny");
    app.handle_key(Key::Char('2'));

    // The one line is picked, for all of it
    app.handle_key(Key::Char('c'));
    let lines = screen(&app, W, H);
    assert!(lines.contains("[x]"), "{lines}");
    assert!(lines.contains("50000"), "{lines}");
    app.handle_key(Key::Enter);

    // A reason is required
    app.handle_key(Key::Enter);
    let missing = screen(&app, W, H);
    assert!(missing.contains("reason for the credit note"), "{missing}");
    type_keys(&mut app, "\tBilled in error\n");
    let question = screen(&app, W, H);
    assert!(question.contains("Issue CN/"), "{question}");
    assert!(question.contains("₹59,000.00"), "{question}");
    assert_eq!(
        store.repository().list_invoices().unwrap()[0].status(),
        InvoiceStatus::Issued
    );

    app.handle_key(Key::Char('y'));
    let after = screen(&app, W, H);
    assert!(after.contains("Issued credit note CN/"), "{after}");
    assert!(after.contains("CREDITED"), "{after}");
    assert!(after.contains("CN/26-27/001"), "{after}");
    assert_eq!(
        store.repository().list_invoices().unwrap()[0].status(),
        InvoiceStatus::Credited
    );
    assert_eq!(
        milestone_status(&store, &project),
        MilestoneStatus::Credited
    );

    // Nothing is left to credit
    app.handle_key(Key::Char('c'));
    assert!(screen(&app, W, H).contains("already take off this whole invoice"));

    // Projects shows it as credited, not paid
    app.handle_key(Key::Char('1'));
    let projects = screen(&app, W, H);
    assert!(projects.contains("CREDITED"), "{projects}");
    assert!(projects.contains("Credited"), "{projects}");
}

#[test]
fn a_credit_note_on_a_paid_invoice_refunds_or_keeps_the_rest() {
    let (store, project) = store_with_milestone(true);
    let mut app = invoicing_app(&store);
    type_keys(&mut app, "\ti\ny");
    app.handle_key(Key::Char('2'));
    // Paid in full: ₹59,000
    app.handle_key(Key::Char('p'));
    app.handle_key(Key::Enter);
    assert!(store.repository().list_invoices().unwrap()[0].is_paid());

    // ₹10,000 + IGST, refunded
    credit_lines(&mut app, "10000");
    type_keys(&mut app, "\tDiscount\n");
    let question = screen(&app, W, H);
    assert!(
        question.contains("₹11,800.00 already paid will be refunded"),
        "{question}"
    );
    app.handle_key(Key::Char('y'));
    let after = screen(&app, W, H);
    assert!(after.contains("Issued credit note CN/"), "{after}");
    assert!(after.contains("refunded"), "{after}");
    assert!(after.contains("-₹11,800.00"), "{after}");

    // ₹5,000 + IGST, kept as an advance for the client
    credit_lines(&mut app, "5000");
    type_keys(&mut app, "\tGoodwill\t ");
    app.handle_key(Key::Enter);
    let question = screen(&app, W, H);
    assert!(
        question.contains("₹5,900.00 already paid will be kept as an advance for"),
        "{question}"
    );
    app.handle_key(Key::Char('y'));
    let after = screen(&app, W, H);
    assert!(after.contains("advance    "), "{after}");
    assert!(after.contains("advance held for Acme ₹5,900.00"), "{after}");
    assert!(after.contains("PAID"), "{after}");
    assert_eq!(milestone_status(&store, &project), MilestoneStatus::Paid);

    // Once paid, the picker says what is due and that the rest goes back to the client
    app.handle_key(Key::Char('c'));
    let guide = screen(&app, W, H);
    assert!(
        guide.contains("₹0.00 due · up to ₹41,300.00 incl. GST"),
        "{guide}"
    );
    assert!(guide.contains("[ ] Design"), "{guide}");
    // More than the line has left is refused while it can still be changed
    type_keys(&mut app, "40000\n");
    let refused = screen(&app, W, H);
    assert!(
        refused.contains("Design: only 35000.00 is left to credit"),
        "{refused}"
    );
}

#[test]
fn cancelling_after_a_tds_payment_asks_for_the_tds_reversed() {
    let (store, _) = store_with_milestone(true);
    let mut app = invoicing_app(&store);
    type_keys(&mut app, "\ti\ny");
    app.handle_key(Key::Char('2'));
    // ₹59,000: ₹54,000 banked, ₹5,000 TDS
    app.handle_key(Key::Char('p'));
    app.handle_key(Key::Tab);
    app.handle_key(Key::Ctrl('u'));
    type_keys(&mut app, "54000\t");
    app.handle_key(Key::Ctrl('u'));
    type_keys(&mut app, "5000\n");
    assert!(store.repository().list_invoices().unwrap()[0].is_paid());

    // Refunding it all would hand back more cash than was received. Once paid, nothing is
    // prefilled: typing the amount picks the line.
    type_keys(&mut app, "c");
    assert!(screen(&app, W, H).contains("0 picked"));
    type_keys(&mut app, "50000\n");
    let form = screen(&app, W, H);
    assert!(form.contains("TDS reversed"), "{form}");
    type_keys(&mut app, "\tCancelled\n");
    let refused = screen(&app, W, H);
    assert!(refused.contains("would go back to the client"), "{refused}");
    // Esc goes back to the lines, with the amount still typed
    app.handle_key(Key::Esc);
    let back = screen(&app, W, H);
    assert!(back.contains("lines to take back"), "{back}");
    assert!(back.contains("1 picked"), "{back}");
    assert!(back.contains("50000"), "{back}");

    type_keys(&mut app, "\n\tCancelled\t\t5000\n");
    let question = screen(&app, W, H);
    assert!(question.contains("₹5,000.00 TDS reversed"), "{question}");
    app.handle_key(Key::Char('y'));
    let after = screen(&app, W, H);
    assert!(after.contains("-₹54,000.00"), "{after}");
    assert!(after.contains("-₹5,000.00"), "{after}");
}

#[test]
fn search_narrows_invoices() {
    let (store, _) = store_with_milestone(true);
    let mut app = invoicing_app(&store);
    type_keys(&mut app, "\ti\ny");
    app.handle_key(Key::Char('2'));
    app.handle_key(Key::Char('/'));
    type_keys(&mut app, "globex\n");
    let screen_text = screen(&app, W, H);
    assert!(screen_text.contains("No invoices match"), "{screen_text}");
    app.handle_key(Key::Esc);
    assert!(screen(&app, W, H).contains("Acme"));
}

#[test]
fn settings_switches_and_saves_the_theme() {
    let (store, _) = store_with_milestone(true);
    let mut app = invoicing_app(&store);
    app.handle_key(Key::Char('3'));
    assert!(screen(&app, W, H).contains("dark"));
    app.handle_key(Key::Char('t'));
    let after = screen(&app, W, H);
    assert!(after.contains("Theme: light"), "{after}");
    assert_eq!(
        store.repository().load_appearance().unwrap(),
        compter_core::domain::Appearance::Light
    );
    assert_eq!(app.theme().key().fg, Some(ratatui::style::Color::Blue));
}

#[test]
fn settings_picks_the_sync_folder_for_the_next_start() {
    let store = Store::in_memory().unwrap();
    let root = tempfile::tempdir().unwrap();
    let root_path = root.path().to_path_buf();
    // Paths are compared by what they point at: the picker resolves links (macOS's /var) and
    // shows Windows paths without `\\?\`
    let saved = |data_dir: &DataDir| {
        SyncDir::saved(data_dir)
            .unwrap()
            .map(|dir| std::fs::canonicalize(dir.root()).unwrap())
    };
    let data_dir = DataDir::new(root_path.join("data"));
    std::fs::create_dir_all(root_path.join("data")).unwrap();
    std::fs::create_dir_all(root_path.join("sync/compter")).unwrap();
    SyncDir::save(&data_dir, Some(&root_path.join("sync"))).unwrap();
    let mut app = settings_app(&store, data_dir.clone());
    let shown = screen(&app, W, H);
    assert!(!shown.contains("Sync folder     off"), "{shown}");

    // Opens in the saved folder; Enter goes into compter/, Space chooses it
    app.handle_key(Key::Char('s'));
    assert!(screen(&app, W, H).contains("compter/"));
    app.handle_key(Key::Enter);
    app.handle_key(Key::Char(' '));
    let shown = screen(&app, W, H);
    assert!(shown.contains("from the next start"), "{shown}");
    let compter = std::fs::canonicalize(root_path.join("sync").join("compter")).unwrap();
    assert_eq!(saved(&data_dir), Some(compter.clone()));

    // Up twice to the root, then the data folder is refused and the picker stays open
    app.handle_key(Key::Char('s'));
    app.handle_key(Key::Backspace);
    app.handle_key(Key::Backspace);
    app.handle_key(Key::Char('k'));
    app.handle_key(Key::Enter);
    app.handle_key(Key::Char(' '));
    // The message names the temp path, so match one word that wrapping can't split
    let shown = screen(&app, W, H);
    assert!(shown.contains("inside"), "{shown}");
    assert!(shown.contains("Choose a folder"), "{shown}");
    assert_eq!(saved(&data_dir), Some(compter));
    app.handle_key(Key::Esc);

    app.handle_key(Key::Char('x'));
    let shown = screen(&app, W, H);
    assert!(shown.contains("Sync turned off"), "{shown}");
    assert!(shown.contains("Sync folder     off"), "{shown}");
    assert!(SyncDir::saved(&data_dir).unwrap().is_none());
    app.handle_key(Key::Char('x'));
    assert!(screen(&app, W, H).contains("already off"));
}

#[test]
fn a_credit_note_takes_back_the_lines_picked() {
    let (store, mut project) = store_with_milestone(true);
    let due = NaiveDate::from_ymd_opt(2026, 10, 2).unwrap();
    project
        .add_milestone("Build", due, Some(BigDecimal::from(20_000)))
        .unwrap();
    store.repository().save(&project).unwrap();
    let mut app = invoicing_app(&store);
    type_keys(&mut app, "\tia\n\ny");
    app.handle_key(Key::Char('2'));
    let listed = screen(&app, W, H);
    assert!(listed.contains("Design, Build"), "{listed}");

    // Nothing is picked on an invoice of several lines
    app.handle_key(Key::Char('c'));
    let lines = screen(&app, W, H);
    assert!(lines.contains("0 picked"), "{lines}");
    // Build in part: ₹5,000 of its ₹20,000
    app.handle_key(Key::Char('j'));
    app.handle_key(Key::Ctrl('u'));
    type_keys(&mut app, "5000\n\tScope cut\n");
    let question = screen(&app, W, H);
    assert!(question.contains("₹5,900.00"), "{question}");
    app.handle_key(Key::Char('y'));
    let after = screen(&app, W, H);
    assert!(after.contains("Issued credit note CN/"), "{after}");

    let status = |name: &str| {
        let project = store.repository().load(project.id()).unwrap();
        let m = project.milestones().iter().find(|m| m.name() == name);
        m.map(|m| m.status())
    };
    assert_eq!(status("Build"), Some(MilestoneStatus::Invoiced));

    // The rest of Build, typed now the invoice has a credit: taken back in full, Build is
    // credited while Design is still due
    type_keys(&mut app, "cj15000\n\tScope cut\n");
    let question = screen(&app, W, H);
    assert!(question.contains("₹17,700.00"), "{question}");
    app.handle_key(Key::Char('y'));
    assert_eq!(status("Build"), Some(MilestoneStatus::Credited));
    assert_eq!(status("Design"), Some(MilestoneStatus::Invoiced));
    app.handle_key(Key::Char('1'));
    let projects = screen(&app, W, H);
    assert!(projects.contains("CREDITED"), "{projects}");
    assert!(projects.contains("INVOICED"), "{projects}");
    app.handle_key(Key::Char('2'));

    // Then all of Design: Build has nothing left to pick, so a picks only Design
    // Picking it fills in all that is left on it
    type_keys(&mut app, "ca");
    let picked = screen(&app, W, H);
    assert!(picked.contains("1 picked"), "{picked}");
    assert!(picked.contains("50000.00"), "{picked}");
    type_keys(&mut app, "\n\tCancelled\n");
    let question = screen(&app, W, H);
    assert!(question.contains("₹59,000.00"), "{question}");
    assert!(question.contains("₹0.00 will still"), "{question}");
    app.handle_key(Key::Char('y'));
    let project = store.repository().load(project.id()).unwrap();
    assert!(
        project
            .milestones()
            .iter()
            .all(|m| m.status() == MilestoneStatus::Credited)
    );
}

#[test]
fn a_discount_comes_off_before_gst() {
    let (store, project) = store_with_milestone(true);
    let mut app = invoicing_app(&store);
    type_keys(&mut app, "\ti");
    type_keys(&mut app, "110%\n");
    assert!(screen(&app, W, H).contains("less than 100"));
    app.handle_key(Key::Ctrl('u'));
    type_keys(&mut app, "10%\n");
    let question = screen(&app, W, H);
    // ₹45,000 + IGST 8,100
    assert!(question.contains("₹53,100.00"), "{question}");
    assert!(question.contains("after ₹5,000.00 off"), "{question}");

    app.handle_key(Key::Char('y'));
    let stored = store.repository().load(project.id()).unwrap();
    assert_eq!(stored.milestones()[0].billed(), BigDecimal::from(45_000));
    app.handle_key(Key::Char('2'));
    let invoices = screen(&app, W, H);
    assert!(invoices.contains("outstanding ₹53,100.00"), "{invoices}");
}

#[test]
fn settings_shows_the_google_account_this_device_syncs_through() {
    let store = Store::in_memory().unwrap();
    let root = tempfile::tempdir().unwrap();
    let data_dir = DataDir::new(root.path());
    SyncBackend::save_google_drive(&data_dir, Some("me@example.com")).unwrap();
    let mut app = settings_app(&store, data_dir.clone());
    let shown = screen(&app, W, H);
    assert!(shown.contains("Google Drive (me@example.com)"), "{shown}");

    app.handle_key(Key::Char('x'));
    let shown = screen(&app, W, H);
    assert!(shown.contains("Sync folder     off"), "{shown}");
    assert!(shown.contains("drive sign-out"), "{shown}");
    assert_eq!(SyncBackend::configured(&data_dir, None).unwrap(), None);
    assert_eq!(
        SyncBackend::google_account(&data_dir).unwrap().as_deref(),
        Some("me@example.com")
    );
}
