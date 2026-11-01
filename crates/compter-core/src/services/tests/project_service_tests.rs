use bigdecimal::BigDecimal;
use chrono::NaiveDate;

use crate::domain::{Billing, ClientDetails, DomainError, ProjectStatus, TimeEntry};
use crate::services::merge_import::WorkSession;
use crate::services::{MilestoneDraft, ProjectDraft, ProjectService, ServiceError};
use crate::storage::{ProjectRepository, RedbRepository, StorageError, Store};

fn service() -> ProjectService<RedbRepository> {
    ProjectService::new(Store::in_memory().unwrap().repository())
}

fn date() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, 1).unwrap()
}

fn draft(name: &str, billing: Billing) -> ProjectDraft {
    ProjectDraft {
        name: name.to_owned(),
        client: ClientDetails::named("Acme", Some("29AAPFU0939F1ZR")),
        billing,
    }
}

fn milestone(amount: Option<i64>) -> MilestoneDraft {
    MilestoneDraft {
        name: "Design".to_owned(),
        due_date: date(),
        amount: amount.map(BigDecimal::from),
    }
}

#[test]
fn every_change_is_saved() {
    let mut service = service();
    let id = service.create(draft("Site", Billing::Fixed)).unwrap().id();
    service.add_milestone(id, milestone(Some(500))).unwrap();
    service.edit_milestone(id, 0, milestone(Some(750))).unwrap();

    let stored = service.load(id).unwrap();
    assert_eq!(stored.total_amount(), &BigDecimal::from(750));
    let summary = &service.list().unwrap()[0];
    assert_eq!(summary.total_amount(), &BigDecimal::from(750));
}

#[test]
fn rejected_changes_are_not_saved() {
    let mut service = service();
    let id = service.create(draft("Site", Billing::Fixed)).unwrap().id();
    service.add_milestone(id, milestone(Some(500))).unwrap();

    let mut bad = draft("Renamed", Billing::hourly(BigDecimal::from(100)).unwrap());
    bad.client.gstin = None;
    assert!(matches!(
        service.update_details(id, bad),
        Err(ServiceError::Domain(DomainError::BillingMismatch))
    ));
    let stored = service.load(id).unwrap();
    assert_eq!(stored.name(), "Site");
    assert!(stored.client_gstin().is_some());
}

#[test]
fn hourly_time_logging_and_undo() {
    let mut service = service();
    let hourly = Billing::hourly(BigDecimal::from(1200)).unwrap();
    let id = service.create(draft("App", hourly)).unwrap().id();
    service.add_milestone(id, milestone(None)).unwrap();
    let entry = TimeEntry::new(date(), 90, "build").unwrap();
    let project = service.log_time(id, 0, entry).unwrap();
    assert_eq!(project.total_amount(), &BigDecimal::from(1800));

    let project = service.remove_last_time_entry(id, 0).unwrap();
    assert_eq!(project.total_amount(), &BigDecimal::from(0));
    assert_eq!(service.load(id).unwrap(), project);
}

#[test]
fn removing_milestones_and_projects() {
    let mut service = service();
    let id = service.create(draft("Site", Billing::Fixed)).unwrap().id();
    service.add_milestone(id, milestone(Some(500))).unwrap();
    let project = service.remove_milestone(id, 0).unwrap();
    assert!(project.milestones().is_empty());
    assert_eq!(project.status(), ProjectStatus::Active);

    service.delete(id).unwrap();
    assert!(service.list().unwrap().is_empty());
    assert!(matches!(
        service.load(id),
        Err(ServiceError::Storage(StorageError::ProjectNotFound(_)))
    ));
}

#[test]
fn invalid_draft_creates_nothing() {
    let mut service = service();
    let mut bad = draft("Site", Billing::Fixed);
    bad.client.gstin = Some("nope".to_owned());
    assert!(matches!(
        service.create(bad),
        Err(ServiceError::Domain(DomainError::InvalidGstin(_)))
    ));
    assert!(service.list().unwrap().is_empty());
}

#[test]
fn projects_with_invoiced_milestones_are_kept() {
    let store = Store::in_memory().unwrap();
    let mut service = ProjectService::new(store.repository());
    let id = service.create(draft("Site", Billing::Fixed)).unwrap().id();
    service.add_milestone(id, milestone(Some(500))).unwrap();

    // Invoicing gets a service method in step 5; until then invoice through the repository
    let mut repo = store.repository();
    let mut project = repo.load(id).unwrap();
    project.invoice_milestone(0, "T-1").unwrap();
    repo.save(&project).unwrap();

    assert!(matches!(
        service.delete(id),
        Err(ServiceError::Domain(DomainError::HasBilledMilestones))
    ));
    assert_eq!(service.load(id).unwrap(), project);
}

fn session(hash: &str, minutes: u32) -> WorkSession {
    WorkSession {
        name: format!("feat/{hash}"),
        date: date(),
        minutes,
        note: format!("{hash} Merge branch 'feat/{hash}' into 'main'"),
    }
}

#[test]
fn imported_merges_become_priced_milestones_once() {
    let mut service = service();
    let rate = Billing::hourly(BigDecimal::from(1_800)).unwrap();
    let id = service.create(draft("Web app", rate)).unwrap().id();
    let sessions = [session("aaaa", 90), session("bbbb", 480)];

    let (project, added) = service.import_sessions(id, &sessions).unwrap();
    assert_eq!(added, 2);
    assert_eq!(project.milestones()[0].amount(), &BigDecimal::from(2_700));
    assert_eq!(project.total_amount(), &BigDecimal::from(17_100));
    assert_eq!(service.find_by_name("Web app").unwrap(), id);

    let more = [
        session("aaaa", 90),
        session("bbbb", 480),
        session("cccc", 60),
    ];
    let (project, added) = service.import_sessions(id, &more).unwrap();
    assert_eq!(added, 1);
    assert_eq!(service.load(id).unwrap().milestones().len(), 3);
    assert_eq!(project.milestones()[2].name(), "feat/cccc");
}

#[test]
fn importing_into_a_fixed_project_saves_nothing() {
    let mut service = service();
    let id = service.create(draft("Site", Billing::Fixed)).unwrap().id();

    let result = service.import_sessions(id, &[session("aaaa", 60)]);
    assert!(matches!(
        result,
        Err(ServiceError::Domain(DomainError::BillingMismatch))
    ));
    assert!(service.load(id).unwrap().milestones().is_empty());
}

#[test]
fn projects_are_found_by_exact_name() {
    let mut service = service();
    service.create(draft("Site", Billing::Fixed)).unwrap();
    service.create(draft("Twin", Billing::Fixed)).unwrap();
    service.create(draft("Twin", Billing::Fixed)).unwrap();

    assert!(matches!(
        service.find_by_name("Nope"),
        Err(ServiceError::NoSuchProject(_))
    ));
    assert!(matches!(
        service.find_by_name("Twin"),
        Err(ServiceError::AmbiguousProject(_))
    ));
}

#[test]
fn archiving_is_saved_with_the_summary() {
    let mut repo = Store::in_memory().unwrap().repository();
    let mut service = ProjectService::new(repo.clone());
    let id = service.create(draft("Site", Billing::Fixed)).unwrap().id();
    assert!(service.set_archived(id, true).is_err());
    assert!(!service.list().unwrap()[0].is_archived());

    service.add_milestone(id, milestone(Some(500))).unwrap();
    let mut project = service.load(id).unwrap();
    project.invoice_milestone(0, "T-1").unwrap();
    repo.save(&project).unwrap();

    service.set_archived(id, true).unwrap();
    assert!(service.load(id).unwrap().is_archived());
    assert!(service.list().unwrap()[0].is_archived());
    service.set_archived(id, false).unwrap();
    assert!(!service.list().unwrap()[0].is_archived());
}

#[test]
fn entering_a_client_records_them_once_by_name() {
    let mut service = service();
    let first = service.create(draft("Site", Billing::Fixed)).unwrap();
    let mut again = draft("App", Billing::Fixed);
    again.client = ClientDetails {
        address: vec!["Pune".to_owned()],
        ..ClientDetails::named(" acme ", Some("29AAPFU0939F1ZR"))
    };
    service.create(again).unwrap();

    let clients = service.clients().unwrap();
    assert_eq!(clients.len(), 1, "{clients:?}");
    assert_eq!(clients[0].address(), ["Pune"]);

    let mut renamed = draft("Site", Billing::Fixed);
    renamed.client = ClientDetails::named("Globex", None);
    service.update_details(first.id(), renamed).unwrap();

    let names: Vec<String> = service
        .clients()
        .unwrap()
        .iter()
        .map(|c| c.name().to_owned())
        .collect();
    assert_eq!(names, ["acme", "Globex"]);
}

#[test]
fn a_rejected_edit_records_no_client() {
    let mut service = service();
    let id = service.create(draft("Site", Billing::Fixed)).unwrap().id();
    service.add_milestone(id, milestone(Some(500))).unwrap();

    let mut bad = draft("Site", Billing::hourly(BigDecimal::from(100)).unwrap());
    bad.client = ClientDetails::named("Globex", None);
    assert!(service.update_details(id, bad).is_err());

    let clients = service.clients().unwrap();
    assert_eq!(clients.len(), 1);
    assert_eq!(clients[0].name(), "Acme");
}

#[test]
fn editing_an_older_project_without_touching_its_client_keeps_the_newer_details() {
    let mut service = service();
    let old = service.create(draft("Site", Billing::Fixed)).unwrap().id();
    let mut moved = draft("App", Billing::Fixed);
    moved.client.address = vec!["New street".to_owned()];
    service.create(moved).unwrap();

    // A new rate on last year's project, its client as it was
    let mut rate_only = draft("Site", Billing::hourly(BigDecimal::from(100)).unwrap());
    rate_only.client = service.load(old).unwrap().client_details();
    service.update_details(old, rate_only).unwrap();

    assert_eq!(service.clients().unwrap()[0].address(), ["New street"]);
}

#[test]
fn an_edit_that_only_tidies_the_client_keeps_the_newer_details() {
    let mut service = service();
    let old = service.create(draft("Site", Billing::Fixed)).unwrap().id();
    let mut moved = draft("App", Billing::Fixed);
    moved.client.address = vec!["New street".to_owned()];
    service.create(moved).unwrap();

    // The same client, typed in another case with a lower-case GSTIN
    let mut tidied = draft("Site", Billing::Fixed);
    tidied.client = ClientDetails::named("ACME ", Some("29aapfu0939f1zr"));
    service.update_details(old, tidied).unwrap();

    assert_eq!(service.clients().unwrap()[0].address(), ["New street"]);
}

#[test]
fn a_form_with_fewer_address_lines_never_rewrites_a_clients_address() {
    let mut service = service();
    let mut three = draft("Site", Billing::Fixed);
    three.client.address = vec!["Plot 7".to_owned(), "MIDC".to_owned(), "Pune".to_owned()];
    service.create(three).unwrap();

    // The terminal's two address lines, the rest joined as it fills them from the client
    let mut two = draft("App", Billing::Fixed);
    two.client.address = vec!["Plot 7".to_owned(), "MIDC, Pune".to_owned()];
    service.create(two).unwrap();

    assert_eq!(
        service.clients().unwrap()[0].address(),
        ["Plot 7", "MIDC", "Pune"]
    );
}
