//! Devices hand one database over through a cloud drive reached through its API, not a local
//! folder: the engine sees only whole files by name

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use compter_core::domain::{Billing, Project};
use compter_core::storage::sync::{
    self, SyncBackend, SyncDir, SyncError, SyncFolder, SyncSession, SyncSource,
};
use compter_core::storage::{DataDir, ProjectRepository, Store};
use tempfile::TempDir;

/// A cloud drive's app folder, shared by every device holding a clone
#[derive(Clone, Default)]
struct Cloud {
    files: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    offline: Arc<Mutex<bool>>,
}

impl Cloud {
    fn has(&self, name: &str) -> bool {
        self.files.lock().unwrap().contains_key(name)
    }

    fn go_offline(&self) {
        *self.offline.lock().unwrap() = true;
    }

    fn reachable(&self) -> Result<(), SyncError> {
        if *self.offline.lock().unwrap() {
            return Err(SyncError::Remote {
                place: self.place(),
                source: "offline".into(),
            });
        }
        Ok(())
    }
}

impl SyncFolder for Cloud {
    fn read(&self, name: &str) -> Result<Option<Vec<u8>>, SyncError> {
        self.reachable()?;
        Ok(self.files.lock().unwrap().get(name).cloned())
    }

    fn write(&self, name: &str, bytes: &[u8]) -> Result<(), SyncError> {
        self.reachable()?;
        self.files
            .lock()
            .unwrap()
            .insert(name.to_owned(), bytes.to_vec());
        Ok(())
    }

    fn delete(&self, name: &str) -> Result<(), SyncError> {
        self.reachable()?;
        self.files.lock().unwrap().remove(name);
        Ok(())
    }

    fn place(&self) -> String {
        "the cloud".to_owned()
    }
}

struct Device {
    _dir: TempDir,
    data: DataDir,
}

impl Device {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let data = DataDir::new(dir.path());
        Self { _dir: dir, data }
    }

    fn add_project(&self, cloud: &Cloud, name: &str) -> Option<u64> {
        let session = SyncSession::begin(&self.data, cloud.clone(), "device").unwrap();
        let store = Store::open(&self.data).unwrap();
        let project = Project::new(name, "Acme", None, Billing::Fixed).unwrap();
        store.repository().save(&project).unwrap();
        session.finish(store).unwrap()
    }

    fn project_names(&self) -> Vec<String> {
        let store = Store::open(&self.data).unwrap();
        let mut names: Vec<String> = store
            .repository()
            .list()
            .unwrap()
            .iter()
            .map(|summary| summary.name().to_owned())
            .collect();
        names.sort();
        names
    }
}

#[test]
fn devices_hand_the_database_over_through_the_cloud() {
    let cloud = Cloud::default();
    let laptop = Device::new();
    let desktop = Device::new();
    assert_eq!(laptop.add_project(&cloud, "Site"), Some(1));

    let session = SyncSession::begin(&desktop.data, cloud.clone(), "desktop").unwrap();
    assert!(
        cloud.has("lease.json"),
        "the lease is held while compter is open"
    );
    assert!(matches!(
        SyncSession::begin(&laptop.data, cloud.clone(), "laptop"),
        Err(SyncError::Leased { .. })
    ));
    let store = Store::open(&desktop.data).unwrap();
    assert_eq!(session.finish(store).unwrap(), None);
    assert!(!cloud.has("lease.json"));
    assert_eq!(desktop.project_names(), ["Site"]);

    assert_eq!(desktop.add_project(&cloud, "App"), Some(2));
    laptop.add_project(&cloud, "Shop");
    assert_eq!(laptop.project_names(), ["App", "Shop", "Site"]);
}

#[test]
fn an_unreachable_cloud_changes_nothing_on_the_device() {
    let cloud = Cloud::default();
    let laptop = Device::new();
    laptop.add_project(&cloud, "Site");
    cloud.go_offline();

    let desktop = Device::new();
    assert!(matches!(
        SyncSession::begin(&desktop.data, cloud.clone(), "desktop"),
        Err(SyncError::Remote { .. })
    ));
    assert!(!desktop.data.database_path().exists());
}

#[test]
fn a_cloud_without_a_snapshot_is_seeded_once() {
    let cloud = Cloud::default();
    let laptop = Device::new();
    let store = Store::open(&laptop.data).unwrap();
    let project = Project::new("Site", "Acme", None, Billing::Fixed).unwrap();
    store.repository().save(&project).unwrap();
    store.close().unwrap();

    assert_eq!(sync::seed(&laptop.data, &cloud, "laptop").unwrap(), Some(1));
    assert_eq!(sync::seed(&laptop.data, &cloud, "laptop").unwrap(), None);

    let phone = Device::new();
    assert_eq!(sync::seed(&phone.data, &cloud, "phone").unwrap(), None);
    phone.add_project(&cloud, "App");
    assert_eq!(phone.project_names(), ["App", "Site"]);
}

#[test]
fn google_drive_and_a_folder_replace_each_other_unless_the_variable_overrides_both() {
    let device = Device::new();
    let folder = tempfile::tempdir().unwrap();
    assert_eq!(SyncBackend::configured(&device.data, None).unwrap(), None);

    SyncDir::save(&device.data, Some(folder.path())).unwrap();
    SyncBackend::save_google_drive(&device.data, Some("me@example.com")).unwrap();
    assert_eq!(
        SyncBackend::configured(&device.data, None).unwrap(),
        Some(SyncBackend::GoogleDrive {
            account: "me@example.com".to_owned()
        })
    );
    assert!(SyncDir::saved(&device.data).unwrap().is_none());

    let env = SyncDir::new(folder.path());
    assert_eq!(
        SyncBackend::configured(&device.data, Some(env.clone())).unwrap(),
        Some(SyncBackend::Folder(env, SyncSource::Env))
    );

    SyncDir::save(&device.data, Some(folder.path())).unwrap();
    assert!(matches!(
        SyncBackend::configured(&device.data, None).unwrap(),
        Some(SyncBackend::Folder(_, SyncSource::Settings))
    ));

    SyncBackend::save_google_drive(&device.data, Some("me@example.com")).unwrap();
    SyncBackend::save_google_drive(&device.data, None).unwrap();
    assert_eq!(SyncBackend::configured(&device.data, None).unwrap(), None);
}

#[test]
fn a_generation_counted_in_a_folder_never_hides_a_snapshot_in_google_drive() {
    let folder = tempfile::tempdir().unwrap();
    let laptop = Device::new();
    SyncDir::save(&laptop.data, Some(folder.path())).unwrap();
    for name in ["Site", "App", "Shop"] {
        let session =
            SyncSession::begin(&laptop.data, SyncDir::new(folder.path()), "laptop").unwrap();
        let store = Store::open(&laptop.data).unwrap();
        let project = Project::new(name, "Acme", None, Billing::Fixed).unwrap();
        store.repository().save(&project).unwrap();
        session.finish(store).unwrap();
    }

    let cloud = Cloud::default();
    let phone = Device::new();
    assert_eq!(phone.add_project(&cloud, "Phone"), Some(1));

    SyncBackend::save_google_drive(&laptop.data, Some("me@example.com")).unwrap();
    assert!(matches!(
        SyncSession::begin(&laptop.data, cloud.clone(), "laptop"),
        Err(SyncError::Conflict { .. })
    ));
}

#[test]
fn the_google_sign_in_is_remembered_until_signing_out_even_after_switching_to_a_folder() {
    let device = Device::new();
    let folder = tempfile::tempdir().unwrap();
    assert_eq!(SyncBackend::google_account(&device.data).unwrap(), None);

    SyncBackend::save_google_drive(&device.data, Some("me@example.com")).unwrap();
    SyncDir::save(&device.data, Some(folder.path())).unwrap();
    assert!(matches!(
        SyncBackend::configured(&device.data, None).unwrap(),
        Some(SyncBackend::Folder(..))
    ));
    assert_eq!(
        SyncBackend::google_account(&device.data)
            .unwrap()
            .as_deref(),
        Some("me@example.com")
    );

    SyncBackend::forget_google_account(&device.data).unwrap();
    assert_eq!(SyncBackend::google_account(&device.data).unwrap(), None);
    assert!(matches!(
        SyncBackend::configured(&device.data, None).unwrap(),
        Some(SyncBackend::Folder(..))
    ));
}

#[test]
fn a_device_json_from_before_the_kept_account_still_finds_its_sign_in() {
    let device = Device::new();
    std::fs::create_dir_all(device.data.root()).unwrap();
    std::fs::write(
        device.data.device_settings_path(),
        br#"{"sync_dir":null,"google_drive":"me@example.com"}"#,
    )
    .unwrap();
    assert_eq!(
        SyncBackend::google_account(&device.data)
            .unwrap()
            .as_deref(),
        Some("me@example.com")
    );
}
