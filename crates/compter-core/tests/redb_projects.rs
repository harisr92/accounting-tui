//! Projects survive closing and reopening the database file

use compter_core::demo;
use compter_core::storage::{DataDir, ProjectRepository, StorageError, Store};

#[test]
fn projects_persist_across_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = DataDir::new(dir.path());
    let projects = demo::projects().unwrap();

    let saved_list = {
        let store = Store::open(&data_dir).unwrap();
        let mut repo = store.repository();
        for project in &projects {
            repo.save(project).unwrap();
        }
        repo.list().unwrap()
    };

    let store = Store::open(&data_dir).unwrap();
    let repo = store.repository();
    assert_eq!(repo.list().unwrap(), saved_list);
    for project in &projects {
        assert_eq!(&repo.load(project.id()).unwrap(), project);
    }
}

#[test]
fn deletes_persist_across_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = DataDir::new(dir.path());
    let projects = demo::projects().unwrap();
    let removed = projects[0].id();
    {
        let store = Store::open(&data_dir).unwrap();
        let mut repo = store.repository();
        for project in &projects {
            repo.save(project).unwrap();
        }
        repo.delete(removed).unwrap();
    }

    let store = Store::open(&data_dir).unwrap();
    let repo = store.repository();
    assert_eq!(repo.list().unwrap().len(), projects.len() - 1);
    assert!(matches!(
        repo.load(removed),
        Err(StorageError::ProjectNotFound(_))
    ));
}
