use bigdecimal::BigDecimal;
use chrono::NaiveDate;

use crate::domain::{Billing, DomainError, ProjectStatus, TimeEntry};
use crate::services::{MilestoneDraft, ProjectDraft, ProjectService, ServiceError};
use crate::storage::{ProjectRepository, RedbProjectRepository, StorageError, Store};

fn service() -> ProjectService<RedbProjectRepository> {
    ProjectService::new(Store::in_memory().unwrap().projects())
}

fn date() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, 1).unwrap()
}

fn draft(name: &str, billing: Billing) -> ProjectDraft {
    ProjectDraft {
        name: name.to_owned(),
        client_name: "Acme".to_owned(),
        client_gstin: Some("29AAPFU0939F1ZR".to_owned()),
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
    bad.client_gstin = None;
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
    bad.client_gstin = Some("nope".to_owned());
    assert!(matches!(
        service.create(bad),
        Err(ServiceError::Domain(DomainError::InvalidGstin(_)))
    ));
    assert!(service.list().unwrap().is_empty());
}

#[test]
fn projects_with_invoiced_milestones_are_kept() {
    let store = Store::in_memory().unwrap();
    let mut service = ProjectService::new(store.projects());
    let id = service.create(draft("Site", Billing::Fixed)).unwrap().id();
    service.add_milestone(id, milestone(Some(500))).unwrap();

    // Invoicing gets a service method in step 5; until then invoice through the repository
    let mut repo = store.projects();
    let mut project = repo.load(id).unwrap();
    project.invoice_milestone(0).unwrap();
    repo.save(&project).unwrap();

    assert!(matches!(
        service.delete(id),
        Err(ServiceError::Domain(DomainError::HasBilledMilestones))
    ));
    assert_eq!(service.load(id).unwrap(), project);
}
