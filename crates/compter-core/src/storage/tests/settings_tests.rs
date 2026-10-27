use crate::domain::{Appearance, BusinessProfile, Logo, LogoId, ProfileDraft};
use crate::storage::tables::{APPEARANCE_KEY, SETTINGS};
use crate::storage::{SettingsRepository, Store};

const PNG_HEADER: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";

fn profile(logo: Option<LogoId>) -> BusinessProfile {
    BusinessProfile::try_from(ProfileDraft {
        name: "Studio".to_owned(),
        address: vec!["Pune".to_owned()],
        sac_code: "998314".to_owned(),
        invoice_prefix: "INV".to_owned(),
        logo,
        ..Default::default()
    })
    .unwrap()
}

#[test]
fn a_logo_is_saved_with_the_profile_that_uses_it() {
    let store = Store::in_memory().unwrap();
    let mut repo = store.repository();
    let logo = Logo::new(PNG_HEADER.to_vec()).unwrap();
    repo.set_logo(&profile(Some(logo.id())), Some(&logo))
        .unwrap();

    let saved = repo.load_profile().unwrap().unwrap();
    assert_eq!(saved.logo(), Some(logo.id()));
    assert_eq!(repo.load_logo(logo.id()).unwrap(), Some(logo));
}

#[test]
fn removing_the_logo_keeps_the_image_for_invoices_that_used_it() {
    let store = Store::in_memory().unwrap();
    let mut repo = store.repository();
    let logo = Logo::new(PNG_HEADER.to_vec()).unwrap();
    repo.set_logo(&profile(Some(logo.id())), Some(&logo))
        .unwrap();
    repo.set_logo(&profile(None), None).unwrap();

    assert_eq!(repo.load_profile().unwrap().unwrap().logo(), None);
    assert!(repo.load_logo(logo.id()).unwrap().is_some());
}

#[test]
fn an_unknown_logo_is_none() {
    let store = Store::in_memory().unwrap();
    assert_eq!(
        store.repository().load_logo(LogoId::generate()).unwrap(),
        None
    );
}

#[test]
fn the_theme_is_dark_until_one_is_saved() {
    let store = Store::in_memory().unwrap();
    let mut repo = store.repository();
    assert_eq!(repo.load_appearance().unwrap(), Appearance::Dark);
    repo.save_appearance(Appearance::Light).unwrap();
    assert_eq!(repo.load_appearance().unwrap(), Appearance::Light);
    // It is kept apart from the business profile
    assert!(repo.load_profile().unwrap().is_none());
}

#[test]
fn a_theme_from_a_newer_build_falls_back_to_dark() {
    let store = Store::in_memory().unwrap();
    let txn = store.database().begin_write().unwrap();
    txn.open_table(SETTINGS)
        .unwrap()
        .insert(APPEARANCE_KEY, br#""solarized""#.as_slice())
        .unwrap();
    txn.commit().unwrap();
    assert_eq!(
        store.repository().load_appearance().unwrap(),
        Appearance::Dark
    );
}
