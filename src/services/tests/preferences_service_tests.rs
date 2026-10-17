use crate::domain::Appearance;
use crate::services::PreferencesService;
use crate::storage::Store;

#[test]
fn themes_are_saved_as_they_cycle() {
    let store = Store::in_memory().unwrap();
    let mut service = PreferencesService::new(store.repository());
    assert_eq!(service.appearance().unwrap(), Appearance::Dark);
    assert_eq!(service.next_appearance().unwrap(), Appearance::Light);
    let reopened = PreferencesService::new(store.repository());
    assert_eq!(reopened.appearance().unwrap(), Appearance::Light);
}
