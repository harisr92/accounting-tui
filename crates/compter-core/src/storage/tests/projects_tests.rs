use bigdecimal::BigDecimal;
use redb::ReadableDatabase;

use crate::demo;
use crate::domain::{Billing, Project, ProjectId, ProjectStatus};
use crate::storage::tables::{PROJECT_SUMMARIES, PROJECTS};
use crate::storage::{ProjectRepository, StorageError, Store};

fn store_with_samples() -> (Store, Vec<Project>) {
    let store = Store::in_memory().unwrap();
    let projects = demo::projects().unwrap();
    let mut repo = store.repository();
    for project in &projects {
        repo.save(project).unwrap();
    }
    (store, projects)
}

/// Overwrite a stored project with hand-edited JSON
fn write_raw(store: &Store, id: ProjectId, json: &serde_json::Value) {
    let bytes = serde_json::to_vec(json).unwrap();
    let txn = store.database().begin_write().unwrap();
    txn.open_table(PROJECTS)
        .unwrap()
        .insert(id.as_uuid(), bytes.as_slice())
        .unwrap();
    txn.commit().unwrap();
}

#[test]
fn saves_and_loads_projects() {
    let (store, projects) = store_with_samples();
    let repo = store.repository();
    for project in &projects {
        assert_eq!(&repo.load(project.id()).unwrap(), project);
    }
}

#[test]
fn lists_summaries_sorted_by_name() {
    let (store, _) = store_with_samples();
    let names: Vec<String> = store
        .repository()
        .list()
        .unwrap()
        .iter()
        .map(|s| s.name().to_owned())
        .collect();
    assert_eq!(
        names,
        ["API Audit", "Data Pipeline", "Mobile App", "Website Revamp"]
    );
}

#[test]
fn save_replaces_project_and_summary_together() {
    let (store, mut projects) = store_with_samples();
    let mut repo = store.repository();
    let website = &mut projects[0];
    website.invoice_milestone(2, "T-1").unwrap();
    repo.save(website).unwrap();

    let summary = repo
        .list()
        .unwrap()
        .into_iter()
        .find(|s| s.id() == website.id())
        .unwrap();
    assert_eq!(summary.status(), ProjectStatus::Completed);
    assert_eq!(
        repo.load(website.id()).unwrap().status(),
        ProjectStatus::Completed
    );
    assert_eq!(repo.list().unwrap().len(), 4);
}

#[test]
fn delete_removes_project_and_summary() {
    let (store, projects) = store_with_samples();
    let mut repo = store.repository();
    let id = projects[1].id();
    repo.delete(id).unwrap();

    assert!(matches!(
        repo.load(id),
        Err(StorageError::ProjectNotFound(_))
    ));
    assert!(repo.list().unwrap().iter().all(|s| s.id() != id));
    let txn = store.database().begin_read().unwrap();
    let summaries = txn.open_table(PROJECT_SUMMARIES).unwrap();
    assert!(summaries.get(id.as_uuid()).unwrap().is_none());
}

#[test]
fn missing_project_is_not_found() {
    let store = Store::in_memory().unwrap();
    let mut repo = store.repository();
    let id = ProjectId::generate();
    assert!(matches!(
        repo.load(id),
        Err(StorageError::ProjectNotFound(_))
    ));
    assert!(matches!(
        repo.delete(id),
        Err(StorageError::ProjectNotFound(_))
    ));
}

#[test]
fn stored_derived_fields_are_recomputed_on_load() {
    let store = Store::in_memory().unwrap();
    let mut repo = store.repository();
    let mut project = Project::new("Site", "Acme", None, Billing::Fixed).unwrap();
    let due = chrono::NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
    project
        .add_milestone("A", due, Some(BigDecimal::from(100)))
        .unwrap();
    repo.save(&project).unwrap();

    let mut json = serde_json::to_value(&project).unwrap();
    json["total_amount"] = "999".into();
    json["status"] = "PAID".into();
    write_raw(&store, project.id(), &json);

    let loaded = repo.load(project.id()).unwrap();
    assert_eq!(loaded.total_amount(), &BigDecimal::from(100));
    assert_eq!(loaded.status(), ProjectStatus::Active);
}

#[test]
fn invalid_stored_project_is_rejected_with_its_key() {
    let store = Store::in_memory().unwrap();
    let mut repo = store.repository();
    let project = Project::new("Site", "Acme", None, Billing::Fixed).unwrap();
    repo.save(&project).unwrap();

    let mut json = serde_json::to_value(&project).unwrap();
    json["client_gstin"] = "29ABCDE1234F1Z5".into();
    write_raw(&store, project.id(), &json);

    match repo.load(project.id()) {
        Err(StorageError::Codec { table, key, .. }) => {
            assert_eq!(table, "projects");
            assert_eq!(key, project.id().to_string());
        }
        other => panic!("unexpected {other:?}"),
    }
}
