mod support;

use std::fs;
use std::path::Path;

use compter_core::domain::{Billing, Project};
use compter_core::storage::sync::{SyncDir, SyncSession};
use compter_core::storage::{DataDir, ProjectRepository, Store};
use compter_mobile::api::CompterApp;
use support::{demo, project};
use tempfile::TempDir;

/// The computer: compter on the desktop, publishing to the sync folder as it quits
struct Laptop {
    _dir: TempDir,
    data: DataDir,
}

impl Laptop {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let data = DataDir::new(dir.path());
        Self { _dir: dir, data }
    }

    /// Start compter, add a project if `name` is given, quit
    fn run(&self, folder: &SyncDir, name: Option<&str>) {
        let session = SyncSession::begin(&self.data, folder.clone(), "laptop").unwrap();
        let store = Store::open(&self.data).unwrap();
        if let Some(name) = name {
            let project = Project::new(name, "Acme", None, Billing::Fixed).unwrap();
            store.repository().save(&project).unwrap();
        }
        session.finish(store).unwrap();
    }

    fn project_names(&self) -> Vec<String> {
        let store = Store::open(&self.data).unwrap();
        let mut names: Vec<String> = store
            .repository()
            .list()
            .unwrap()
            .iter()
            .map(|p| p.name().to_owned())
            .collect();
        names.sort();
        names
    }
}

fn folder() -> (TempDir, SyncDir) {
    let dir = tempfile::tempdir().unwrap();
    let folder = SyncDir::new(dir.path());
    (dir, folder)
}

fn path(dir: &TempDir) -> String {
    dir.path().to_string_lossy().into_owned()
}

/// What the user carries by hand: both files, copied into `to`
fn carry(from: &Path, to: &Path) {
    for file in ["compter.redb", "manifest.json"] {
        fs::copy(from.join(file), to.join(file)).unwrap();
    }
}

fn names(app: &CompterApp) -> Vec<String> {
    let mut names: Vec<String> = app
        .projects(true)
        .unwrap()
        .into_iter()
        .map(|p| p.name)
        .collect();
    names.sort();
    names
}

#[test]
fn a_new_phone_takes_the_computers_data_and_hands_its_own_back() {
    let (folder_dir, folder) = folder();
    let laptop = Laptop::new();
    laptop.run(&folder, Some("Site"));
    let phone_dir = tempfile::tempdir().unwrap();
    let inbox = tempfile::tempdir().unwrap();
    let outbox = tempfile::tempdir().unwrap();
    let mut phone = CompterApp::open(path(&phone_dir)).unwrap();
    assert_eq!(
        phone.sync_status().unwrap().summary,
        "Not synced with a computer yet"
    );

    carry(folder_dir.path(), inbox.path());
    let received = phone.receive_snapshot(path(&inbox)).unwrap();
    assert!(!received.conflict);
    assert_eq!(received.message, "Took snapshot 1 from the computer");
    assert_eq!(names(&phone), ["Site"]);
    assert_eq!(
        phone.sync_status().unwrap().summary,
        "Synced with snapshot 1; no changes since"
    );

    phone.create_project(project("App", None)).unwrap();
    assert!(phone.sync_status().unwrap().changed);
    let sent = phone
        .send_snapshot(path(&outbox), "iPhone".to_owned())
        .unwrap();
    assert!(Path::new(&sent.snapshot).exists() && Path::new(&sent.manifest).exists());
    assert!(
        sent.message.starts_with("Snapshot 2 is ready"),
        "{}",
        sent.message
    );
    // The phone keeps working on its reopened database
    assert_eq!(names(&phone), ["App", "Site"]);

    carry(outbox.path(), folder_dir.path());
    laptop.run(&folder, None);
    assert_eq!(laptop.project_names(), ["App", "Site"]);
}

#[test]
fn changes_on_both_sides_are_settled_by_choice() {
    let (folder_dir, folder) = folder();
    let laptop = Laptop::new();
    laptop.run(&folder, Some("Site"));
    let phone_dir = tempfile::tempdir().unwrap();
    let mut phone = CompterApp::open(path(&phone_dir)).unwrap();
    let inbox = path(&folder_dir);
    phone.receive_snapshot(inbox.clone()).unwrap();
    phone.create_project(project("App", None)).unwrap();
    laptop.run(&folder, Some("Audit"));

    let received = phone.receive_snapshot(inbox.clone()).unwrap();
    assert!(received.conflict);
    assert!(received.message.contains("laptop"), "{}", received.message);
    assert_eq!(names(&phone), ["App", "Site"], "nothing taken yet");

    let settled = phone
        .settle_snapshot(inbox, false, "iPhone".to_owned())
        .unwrap();
    assert!(settled.starts_with("Took the computer's data"), "{settled}");
    assert_eq!(names(&phone), ["Audit", "Site"]);
}

#[test]
fn a_hand_off_saved_as_a_copy_is_reported_on_the_next_receive() {
    let (folder_dir, folder) = folder();
    let laptop = Laptop::new();
    laptop.run(&folder, Some("Site"));
    let phone_dir = tempfile::tempdir().unwrap();
    let outbox = tempfile::tempdir().unwrap();
    let mut phone = CompterApp::open(path(&phone_dir)).unwrap();
    phone.receive_snapshot(path(&folder_dir)).unwrap();
    phone.create_project(project("App", None)).unwrap();
    phone
        .send_snapshot(path(&outbox), "Pixel".to_owned())
        .unwrap();

    // The files never replaced the folder's (saved as "compter (1).redb", say)
    let received = phone.receive_snapshot(path(&folder_dir)).unwrap();

    assert!(received.conflict);
    assert!(
        received.message.contains("saved as a copy"),
        "{}",
        received.message
    );
}

#[test]
fn the_demo_has_nothing_to_hand_off() {
    let (_scratch, mut app) = demo();
    let inbox = tempfile::tempdir().unwrap();

    assert!(app.sync_status().is_err());
    assert!(app.receive_snapshot(path(&inbox)).is_err());
    assert!(!names(&app).is_empty(), "the demo is untouched");
}

#[test]
fn the_phone_writes_a_lease_the_computer_honours() {
    let (folder_dir, folder) = folder();
    let laptop = Laptop::new();
    laptop.run(&folder, Some("Site"));
    let phone_dir = tempfile::tempdir().unwrap();
    let phone = CompterApp::open(path(&phone_dir)).unwrap();

    let lease = phone.lease_json("iPhone".to_owned()).unwrap();
    // Its own lease, read back from the folder, never blocks the phone
    assert_eq!(phone.lease_holder(lease.clone()).unwrap(), None);
    fs::write(folder_dir.path().join("lease.json"), &lease).unwrap();

    let refused = SyncSession::begin(&laptop.data, folder.clone(), "laptop");
    assert!(
        refused.is_err(),
        "the laptop must not open while the phone holds the lease"
    );
}

#[test]
fn the_phone_sees_who_has_compter_open() {
    let (folder_dir, folder) = folder();
    let laptop = Laptop::new();
    let phone_dir = tempfile::tempdir().unwrap();
    let phone = CompterApp::open(path(&phone_dir)).unwrap();
    let session = SyncSession::begin(&laptop.data, folder.clone(), "laptop").unwrap();

    let lease = fs::read_to_string(folder_dir.path().join("lease.json")).unwrap();
    let holder = phone.lease_holder(lease).unwrap().unwrap();

    assert!(
        holder.starts_with("compter is open on laptop (last seen "),
        "{holder}"
    );
    assert_eq!(
        phone.lease_holder("{ half synced".to_owned()).unwrap(),
        None
    );
    session.finish(Store::open(&laptop.data).unwrap()).unwrap();
}

#[test]
fn the_status_names_the_snapshot_last_synced() {
    let (folder_dir, folder) = folder();
    let laptop = Laptop::new();
    laptop.run(&folder, Some("Site"));
    let phone_dir = tempfile::tempdir().unwrap();
    let mut phone = CompterApp::open(path(&phone_dir)).unwrap();
    assert_eq!(phone.sync_status().unwrap().sha256, None);

    phone.receive_snapshot(path(&folder_dir)).unwrap();

    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(folder_dir.path().join("manifest.json")).unwrap())
            .unwrap();
    assert_eq!(
        phone.sync_status().unwrap().sha256.as_deref(),
        manifest["sha256"].as_str()
    );
}
