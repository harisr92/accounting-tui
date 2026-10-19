//! Two devices hand one database back and forth through a shared sync folder

use std::fs;

use chrono::{Duration, Utc};
use compter_core::domain::{Billing, Project};
use compter_core::storage::sync::{
    self, Lease, Manifest, Resolution, SyncDir, SyncError, SyncSession, SyncSource, SyncState,
};
use compter_core::storage::{DataDir, ProjectRepository, StorageError, Store};
use tempfile::TempDir;

/// One device's data folder, plus the folder both devices sync
struct Device {
    _dir: TempDir,
    data: DataDir,
    name: &'static str,
}

impl Device {
    fn new(name: &'static str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let data = DataDir::new(dir.path());
        Self {
            _dir: dir,
            data,
            name,
        }
    }

    fn begin(&self, sync: &SyncDir) -> Result<(SyncSession, Store), SyncError> {
        let session = SyncSession::begin(&self.data, sync.clone(), self.name)?;
        let store = Store::open(&self.data)?;
        Ok((session, store))
    }

    /// Open, add a project named `name`, quit
    fn add_project(&self, sync: &SyncDir, name: &str) -> Option<u64> {
        let (session, store) = self.begin(sync).unwrap();
        save(&store, name);
        session.finish(store).unwrap()
    }

    fn sync_state(&self) -> SyncState {
        let store = Store::open(&self.data).unwrap();
        sync::state(&self.data, &store).unwrap()
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

fn save(store: &Store, name: &str) {
    let project = Project::new(name, "Acme", None, Billing::Fixed).unwrap();
    store.repository().save(&project).unwrap();
}

fn sync_dir() -> (TempDir, SyncDir) {
    let dir = tempfile::tempdir().unwrap();
    let sync = SyncDir::new(dir.path());
    (dir, sync)
}

fn manifest(sync: &SyncDir) -> Manifest {
    serde_json::from_slice(&fs::read(sync.manifest_path()).unwrap()).unwrap()
}

#[test]
fn first_device_publishes_and_the_next_picks_it_up() {
    let (_dir, sync) = sync_dir();
    let laptop = Device::new("laptop");
    let desktop = Device::new("desktop");

    assert_eq!(laptop.add_project(&sync, "Site"), Some(1));
    assert_eq!(manifest(&sync).device_name, "laptop");
    assert!(!sync.lease_path().exists());

    let (session, store) = desktop.begin(&sync).unwrap();
    session.finish(store).unwrap();
    assert_eq!(desktop.project_names(), ["Site"]);
}

#[test]
fn handing_off_keeps_records_from_both_devices() {
    let (_dir, sync) = sync_dir();
    let laptop = Device::new("laptop");
    let desktop = Device::new("desktop");

    laptop.add_project(&sync, "Site");
    desktop.add_project(&sync, "App");
    laptop.add_project(&sync, "Audit");

    let (session, store) = desktop.begin(&sync).unwrap();
    session.finish(store).unwrap();
    assert_eq!(desktop.project_names(), ["App", "Audit", "Site"]);
    assert_eq!(laptop.project_names(), ["App", "Audit", "Site"]);
}

#[test]
fn changes_on_both_devices_are_refused_until_resolved() {
    let (_dir, sync) = sync_dir();
    let laptop = Device::new("laptop");
    let desktop = Device::new("desktop");
    laptop.add_project(&sync, "Site");
    desktop.add_project(&sync, "Synced");

    // The laptop works offline from the old snapshot while the desktop publishes again
    {
        let store = Store::open(&laptop.data).unwrap();
        save(&store, "Offline");
    }
    desktop.add_project(&sync, "Remote");

    let refused = laptop.begin(&sync).err().unwrap();
    assert!(matches!(refused, SyncError::Conflict { ref device, .. } if device == "desktop"));
    assert!(
        !sync.lease_path().exists(),
        "a refused start releases the lease"
    );

    SyncSession::resolve(&laptop.data, &sync, "laptop", Resolution::KeepRemote).unwrap();
    assert_eq!(laptop.project_names(), ["Remote", "Site", "Synced"]);
    let backups: Vec<_> = fs::read_dir(laptop.data.root())
        .unwrap()
        .filter_map(|entry| entry.unwrap().file_name().into_string().ok())
        .filter(|name| name.starts_with("compter.conflict-"))
        .collect();
    assert_eq!(backups.len(), 1, "the replaced copy is kept");
    laptop.begin(&sync).unwrap();
}

#[test]
fn keeping_the_local_copy_publishes_it_as_the_newest() {
    let (_dir, sync) = sync_dir();
    let laptop = Device::new("laptop");
    let desktop = Device::new("desktop");
    laptop.add_project(&sync, "Site");
    {
        let store = Store::open(&desktop.data).unwrap();
        save(&store, "Local");
    }

    assert!(matches!(
        desktop.begin(&sync),
        Err(SyncError::Conflict { .. })
    ));
    SyncSession::resolve(&desktop.data, &sync, "desktop", Resolution::KeepLocal).unwrap();
    assert_eq!(manifest(&sync).generation, 2);

    let (session, store) = laptop.begin(&sync).unwrap();
    session.finish(store).unwrap();
    assert_eq!(laptop.project_names(), ["Local"]);
}

#[test]
fn a_fresh_lease_from_another_device_blocks_start() {
    let (_dir, sync) = sync_dir();
    let laptop = Device::new("laptop");
    let desktop = Device::new("desktop");

    let (session, store) = laptop.begin(&sync).unwrap();
    let refused = desktop.begin(&sync).err().unwrap();
    assert!(matches!(refused, SyncError::Leased { ref device, .. } if device == "laptop"));

    session.finish(store).unwrap();
    desktop.begin(&sync).unwrap();
}

#[test]
fn a_stale_lease_is_taken_over() {
    let (_dir, sync) = sync_dir();
    let stale = Lease {
        device_id: uuid::Uuid::now_v7(),
        device_name: "crashed".to_owned(),
        heartbeat: Utc::now() - Duration::minutes(30),
    };
    fs::write(sync.lease_path(), serde_json::to_vec(&stale).unwrap()).unwrap();

    Device::new("laptop").begin(&sync).unwrap();
}

#[test]
fn a_snapshot_still_syncing_is_not_pulled() {
    let (_dir, sync) = sync_dir();
    let laptop = Device::new("laptop");
    laptop.add_project(&sync, "Site");
    fs::write(sync.snapshot_path(), b"half uploaded").unwrap();

    let desktop = Device::new("desktop");
    assert!(matches!(
        desktop.begin(&sync),
        Err(SyncError::Incomplete { .. })
    ));
    assert!(!desktop.data.database_path().exists());
}

#[test]
fn an_unchanged_database_is_not_published_again() {
    let (_dir, sync) = sync_dir();
    let laptop = Device::new("laptop");
    laptop.add_project(&sync, "Site");
    let desktop = Device::new("desktop");
    desktop.add_project(&sync, "App");

    // The laptop pulls generation 2 and closes without touching the file
    let session = SyncSession::begin(&laptop.data, sync.clone(), "laptop").unwrap();
    let store = Store::in_memory().unwrap();
    assert_eq!(session.finish(store).unwrap(), None);
    assert_eq!(manifest(&sync).generation, 2);
}

#[test]
fn opening_compter_without_changing_anything_publishes_nothing() {
    let (_dir, sync) = sync_dir();
    let laptop = Device::new("laptop");
    laptop.add_project(&sync, "Site");
    let desktop = Device::new("desktop");

    // redb rewrites its header on every open; only the records count as changes
    let (session, store) = desktop.begin(&sync).unwrap();
    assert_eq!(session.finish(store).unwrap(), None);
    let (session, store) = desktop.begin(&sync).unwrap();
    assert_eq!(session.finish(store).unwrap(), None);
    assert_eq!(manifest(&sync).generation, 1);
}

#[test]
fn conflict_copies_left_by_the_sync_client_are_reported() {
    let (_dir, sync) = sync_dir();
    let laptop = Device::new("laptop");
    laptop.add_project(&sync, "Site");
    fs::write(sync.root().join("compter (1).redb"), b"").unwrap();
    fs::write(sync.root().join("notes.txt"), b"").unwrap();

    let (session, _store) = laptop.begin(&sync).unwrap();
    assert_eq!(session.conflict_copies(), ["compter (1).redb"]);
}

#[test]
fn a_store_still_shared_cannot_close() {
    let store = Store::in_memory().unwrap();
    let _repo = store.repository();
    assert!(matches!(store.close(), Err(StorageError::StillInUse)));
}

#[test]
fn a_device_that_published_meanwhile_is_never_overwritten() {
    let (_dir, sync) = sync_dir();
    let laptop = Device::new("laptop");
    laptop.add_project(&sync, "Site");
    let theirs = manifest(&sync);

    let (session, store) = laptop.begin(&sync).unwrap();
    save(&store, "Ours");
    // The desktop's lease hadn't reached this device, and it published generation 2
    let newer = Manifest {
        generation: 2,
        device_name: "desktop".to_owned(),
        ..theirs.clone()
    };
    fs::write(sync.manifest_path(), serde_json::to_vec(&newer).unwrap()).unwrap();

    assert!(matches!(
        session.finish(store),
        Err(SyncError::Conflict { ref device, .. }) if device == "desktop"
    ));
    assert_eq!(manifest(&sync), newer, "their manifest is left alone");
    assert_eq!(laptop.project_names(), ["Ours", "Site"], "ours stays local");
}

#[test]
fn keeping_a_half_synced_snapshot_leaves_the_local_copy_in_place() {
    let (_dir, sync) = sync_dir();
    let laptop = Device::new("laptop");
    laptop.add_project(&sync, "Site");
    fs::write(sync.snapshot_path(), b"half uploaded").unwrap();

    let refused = SyncSession::resolve(&laptop.data, &sync, "laptop", Resolution::KeepRemote).err();
    assert!(matches!(refused, Some(SyncError::Incomplete { .. })));
    assert_eq!(laptop.project_names(), ["Site"]);
    assert!(
        !sync.lease_path().exists(),
        "a failed resolve releases the lease"
    );
}

#[test]
fn the_folder_saved_in_settings_is_used_unless_the_variable_overrides_it() {
    let device = Device::new("laptop");
    let (_folder, sync) = sync_dir();
    assert!(SyncDir::configured(&device.data, None).unwrap().is_none());

    SyncDir::save(&device.data, Some(sync.root())).unwrap();
    assert_eq!(
        SyncDir::configured(&device.data, None).unwrap(),
        Some((sync.clone(), SyncSource::Settings))
    );
    let other = tempfile::tempdir().unwrap();
    assert_eq!(
        SyncDir::configured(&device.data, Some(SyncDir::new(other.path()))).unwrap(),
        Some((SyncDir::new(other.path()), SyncSource::Env))
    );

    SyncDir::save(&device.data, None).unwrap();
    assert!(SyncDir::saved(&device.data).unwrap().is_none());
}

#[test]
fn a_sync_folder_must_exist_and_stay_apart_from_the_data() {
    let device = Device::new("laptop");
    let missing = device.data.root().parent().unwrap().join("no-such-folder");
    assert!(matches!(
        SyncDir::save(&device.data, Some(&missing)),
        Err(SyncError::NotAFolder(_))
    ));
    let inside = device.data.root().join("sync");
    fs::create_dir_all(&inside).unwrap();
    for folder in [device.data.root(), inside.as_path()] {
        assert!(matches!(
            SyncDir::save(&device.data, Some(folder)),
            Err(SyncError::InsideDataDir(_))
        ));
    }
    assert!(SyncDir::saved(&device.data).unwrap().is_none());
}

/// What the user carries by hand: the snapshot and manifest, copied between folders
fn carry(from: &SyncDir, to: &SyncDir) {
    fs::create_dir_all(to.root()).unwrap();
    fs::copy(from.snapshot_path(), to.snapshot_path()).unwrap();
    fs::copy(from.manifest_path(), to.manifest_path()).unwrap();
}

/// A phone: its own data folder, with the database the app creates on first start
fn phone() -> Device {
    let phone = Device::new("phone");
    Store::open(&phone.data).unwrap().close().unwrap();
    phone
}

/// Open the phone's database, add a project, close it
fn phone_adds(phone: &Device, name: &str) {
    let store = Store::open(&phone.data).unwrap();
    save(&store, name);
    store.close().unwrap();
}

#[test]
fn a_phone_takes_the_snapshot_and_hands_its_changes_back() {
    let (_dir, folder) = sync_dir();
    let (_inbox_dir, inbox) = sync_dir();
    let (_outbox_dir, outbox) = sync_dir();
    let laptop = Device::new("laptop");
    let phone = phone();
    laptop.add_project(&folder, "Site");

    carry(&folder, &inbox);
    // A new install's database is blank, so it is replaced although it never synced
    assert_eq!(sync::receive(&phone.data, &inbox, true).unwrap(), Some(1));
    assert_eq!(phone.project_names(), ["Site"]);
    assert_eq!(sync::receive(&phone.data, &inbox, false).unwrap(), None);
    let synced = phone.sync_state();
    assert_eq!((synced.generation, synced.changed), (1, false));
    assert_eq!(synced.sha256, Some(manifest(&folder).sha256));

    phone_adds(&phone, "App");
    assert!(phone.sync_state().changed);
    assert_eq!(sync::send(&phone.data, &outbox, "phone").unwrap(), 2);
    assert_eq!(manifest(&outbox).device_name, "phone");
    carry(&outbox, &folder);

    let (session, store) = laptop.begin(&folder).unwrap();
    session.finish(store).unwrap();
    assert_eq!(laptop.project_names(), ["App", "Site"]);
}

#[test]
fn an_unchanged_phone_sends_its_generation_again() {
    let (_dir, folder) = sync_dir();
    let (_outbox_dir, outbox) = sync_dir();
    let laptop = Device::new("laptop");
    let phone = phone();
    laptop.add_project(&folder, "Site");
    sync::receive(&phone.data, &folder, true).unwrap();

    assert_eq!(sync::send(&phone.data, &outbox, "phone").unwrap(), 1);
}

#[test]
fn a_phone_with_its_own_records_is_never_overwritten_silently() {
    let (_dir, folder) = sync_dir();
    let laptop = Device::new("laptop");
    let phone = phone();
    phone_adds(&phone, "App");
    laptop.add_project(&folder, "Site");

    let refused = sync::receive(&phone.data, &folder, false);

    assert!(matches!(refused, Err(SyncError::Conflict { .. })));
    assert_eq!(phone.project_names(), ["App"]);
}

#[test]
fn a_phone_settles_a_conflict_either_way() {
    let (_dir, folder) = sync_dir();
    let (_outbox_dir, outbox) = sync_dir();
    let laptop = Device::new("laptop");
    let phone = phone();
    laptop.add_project(&folder, "Site");
    sync::receive(&phone.data, &folder, true).unwrap();
    phone_adds(&phone, "App");
    laptop.add_project(&folder, "Audit");
    assert!(sync::receive(&phone.data, &folder, false).is_err());

    // Keeping the phone's: the next send goes out above the laptop's snapshot
    sync::settle_received(&phone.data, &folder, "phone", Resolution::KeepLocal).unwrap();
    assert_eq!(sync::send(&phone.data, &outbox, "phone").unwrap(), 3);

    // Taking the laptop's instead: the phone's database is kept beside it
    let other = phone_with_conflict(&folder, &laptop);
    sync::settle_received(&other.data, &folder, "phone", Resolution::KeepRemote).unwrap();
    assert_eq!(other.project_names(), ["Audit", "Site", "Spec"]);
    let backups = fs::read_dir(other.data.root())
        .unwrap()
        .filter(|entry| {
            let name = entry.as_ref().unwrap().file_name();
            name.to_string_lossy().starts_with("compter.conflict-")
        })
        .count();
    assert_eq!(backups, 1);
}

/// A second phone that took the laptop's snapshot, then changed while the laptop did too
fn phone_with_conflict(folder: &SyncDir, laptop: &Device) -> Device {
    let phone = phone();
    sync::receive(&phone.data, folder, true).unwrap();
    phone_adds(&phone, "Draft");
    laptop.add_project(folder, "Spec");
    assert!(sync::receive(&phone.data, folder, false).is_err());
    phone
}

#[test]
fn a_new_install_has_a_blank_database() {
    let phone = phone();
    let store = Store::open(&phone.data).unwrap();
    assert!(store.is_blank().unwrap());
    save(&store, "App");
    assert!(!store.is_blank().unwrap());
}

#[test]
fn a_hand_off_that_never_reached_the_folder_is_caught() {
    let (_dir, folder) = sync_dir();
    let (_outbox_dir, outbox) = sync_dir();
    let laptop = Device::new("laptop");
    let phone = phone();
    laptop.add_project(&folder, "Site");
    sync::receive(&phone.data, &folder, true).unwrap();
    phone_adds(&phone, "App");
    // Sent as generation 2, but saved as a copy: the folder still has generation 1
    assert_eq!(sync::send(&phone.data, &outbox, "phone").unwrap(), 2);

    let lost = sync::receive(&phone.data, &folder, false);
    assert!(matches!(
        lost,
        Err(SyncError::Undelivered { sent: 2, found: 1 })
    ));

    // Meanwhile the laptop published its own generation 2: the same number, other records
    laptop.add_project(&folder, "Audit");
    let clash = sync::receive(&phone.data, &folder, false);
    assert!(matches!(
        clash,
        Err(SyncError::Undelivered { sent: 2, found: 2 })
    ));

    // Keeping the phone's sends a generation above both
    sync::settle_received(&phone.data, &folder, "phone", Resolution::KeepLocal).unwrap();
    assert_eq!(sync::send(&phone.data, &outbox, "phone").unwrap(), 3);
}

#[test]
fn the_snapshot_a_phone_sent_counts_as_synced_once_it_arrives() {
    let (_dir, folder) = sync_dir();
    let (_outbox_dir, outbox) = sync_dir();
    let laptop = Device::new("laptop");
    let phone = phone();
    laptop.add_project(&folder, "Site");
    sync::receive(&phone.data, &folder, true).unwrap();
    phone_adds(&phone, "App");
    sync::send(&phone.data, &outbox, "phone").unwrap();
    carry(&outbox, &folder);

    assert_eq!(sync::receive(&phone.data, &folder, false).unwrap(), None);
}

#[test]
fn a_phone_writes_its_own_lease_and_sees_a_computers() {
    let (_dir, folder) = sync_dir();
    let laptop = Device::new("laptop");
    let phone = phone();

    // The phone's own lease, which it uploads itself, never counts against it
    let own = sync::lease_for(&phone.data, "iPhone").unwrap();
    assert_eq!(own.device_name, "iPhone");
    assert_eq!(sync::held_by_other(&phone.data, &own).unwrap(), None);
    assert_eq!(
        sync::lease_for(&phone.data, "iPhone").unwrap().device_id,
        own.device_id
    );

    // compter open on the laptop: its lease is fresh, so it counts
    let (session, store) = laptop.begin(&folder).unwrap();
    let theirs: Lease = serde_json::from_slice(&fs::read(folder.lease_path()).unwrap()).unwrap();
    let held = sync::held_by_other(&phone.data, &theirs).unwrap().unwrap();
    assert_eq!(held.device_name, "laptop");
    session.finish(store).unwrap();

    // A lease a crashed laptop left behind goes stale
    let stale = Lease {
        heartbeat: Utc::now() - Duration::minutes(10),
        ..theirs
    };
    assert_eq!(sync::held_by_other(&phone.data, &stale).unwrap(), None);
}

#[test]
fn a_computer_wont_open_while_the_phone_holds_the_lease() {
    let (_dir, folder) = sync_dir();
    let laptop = Device::new("laptop");
    let phone = phone();
    let lease = sync::lease_for(&phone.data, "iPhone").unwrap();
    fs::write(folder.lease_path(), serde_json::to_vec(&lease).unwrap()).unwrap();

    let refused = laptop.begin(&folder);

    assert!(matches!(refused, Err(SyncError::Leased { device, .. }) if device == "iPhone"));
}
