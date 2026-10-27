use crate::domain::Appearance;
use crate::services::{PreferencesService, ServiceError};
use crate::storage::Store;

#[test]
fn themes_are_saved_as_they_cycle() {
    let store = Store::in_memory().unwrap();
    let mut service = PreferencesService::new(store.repository(), None);
    assert_eq!(service.appearance().unwrap(), Appearance::Dark);
    assert_eq!(service.next_appearance().unwrap(), Appearance::Light);
    let reopened = PreferencesService::new(store.repository(), None);
    assert_eq!(reopened.appearance().unwrap(), Appearance::Light);
}

#[test]
fn demo_mode_has_no_sync_folder_to_set() {
    let store = Store::in_memory().unwrap();
    let mut service = PreferencesService::new(store.repository(), None);
    assert!(service.sync_backend().unwrap().is_none());
    let folder = tempfile::tempdir().unwrap();
    assert!(matches!(
        service.set_sync_folder(Some(folder.path())),
        Err(ServiceError::NoSyncInDemo)
    ));
}
