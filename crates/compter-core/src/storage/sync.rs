//! Hands the database between devices through a folder that Google Drive, OneDrive, Dropbox or
//! iCloud keeps in sync.
//!
//! The live database never sits in the synced folder: a sync client could upload it mid-commit,
//! and edits on two devices would turn into a silent "conflict copy". Instead each device keeps
//! its own `compter.redb` and, on a clean exit, publishes a snapshot plus a manifest numbering it.
//! On start a device pulls a newer snapshot, but only when its own copy is unchanged since it last
//! synced; if both sides changed it refuses, because invoice numbers can't be merged.
//!
//! A lease file says which device has compter open. Sync is slow, so the lease is advisory; the
//! generation check is what keeps records from being lost.

use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError, SyncSender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{DataDir, StorageError, Store, write_file};

const SNAPSHOT_FILE: &str = "compter.redb";
const MANIFEST_FILE: &str = "manifest.json";
const LEASE_FILE: &str = "lease.json";

/// A lease whose heartbeat is older than this was left by a device that crashed or lost power
const LEASE_TTL: Duration = Duration::from_secs(5 * 60);
const HEARTBEAT: Duration = Duration::from_secs(60);

/// Why handing the database between devices failed. Nothing local is overwritten on any of them.
#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("compter is open on {device} (last seen {seen}); quit it there first")]
    Leased { device: String, seen: DateTime<Utc> },
    #[error(
        "this device and {device} both changed the data since they last synced (theirs published {published}); run `compter sync --keep-local` or `compter sync --keep-remote`"
    )]
    Conflict {
        device: String,
        published: DateTime<Utc>,
    },
    #[error(
        "the snapshot from {device} hasn't finished syncing; wait for the sync client, then try again"
    )]
    Incomplete { device: String },
    #[error(
        "the snapshot this device sent (generation {sent}) isn't the one in the sync folder, which has generation {found}; it may have been saved as a copy. Keep this device's data or take the folder's"
    )]
    Undelivered { sent: u64, found: u64 },
    #[error("there is no snapshot in {0} yet")]
    NoSnapshot(PathBuf),
    #[error("there is no database on this device to publish")]
    NoLocalDatabase,
    #[error("{0} is not a folder")]
    NotAFolder(PathBuf),
    #[error("the sync folder and compter's data folder {0} must not be inside one another")]
    InsideDataDir(PathBuf),
    #[error("could not read or write {path}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("invalid sync file {path}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error(transparent)]
    Storage(#[from] StorageError),
}

/// The folder a sync client shares between devices
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncDir {
    root: PathBuf,
}

impl SyncDir {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// `$COMPTER_SYNC_DIR`, which overrides the folder saved in Settings
    pub fn from_env() -> Option<Self> {
        env::var_os("COMPTER_SYNC_DIR")
            .filter(|value| !value.is_empty())
            .map(Self::new)
    }

    /// The folder this device syncs through: `env` (see [`Self::from_env`]), else the one saved
    /// in Settings; syncing is off when neither is set
    pub fn configured(
        data: &DataDir,
        env: Option<Self>,
    ) -> Result<Option<(Self, SyncSource)>, SyncError> {
        if let Some(dir) = env {
            return Ok(Some((dir, SyncSource::Env)));
        }
        Ok(Self::saved(data)?.map(|dir| (dir, SyncSource::Settings)))
    }

    /// The folder saved in Settings on this device
    pub fn saved(data: &DataDir) -> Result<Option<Self>, SyncError> {
        let settings = read_json::<DeviceSettings>(&data.device_settings_path())?;
        Ok(settings.and_then(|s| s.sync_dir).map(Self::new))
    }

    /// Sync this device through `dir` from the next start, or stop syncing when `None`. The
    /// folder must exist and be apart from the data folder, whose database it would clash with.
    pub fn save(data: &DataDir, dir: Option<&Path>) -> Result<(), SyncError> {
        if let Some(dir) = dir {
            check_folder(data, dir)?;
        }
        let path = data.device_settings_path();
        let mut settings = read_json::<DeviceSettings>(&path)?.unwrap_or_default();
        settings.sync_dir = dir.map(Path::to_path_buf);
        write_json(&path, &settings)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn snapshot_path(&self) -> PathBuf {
        self.root.join(SNAPSHOT_FILE)
    }

    pub fn manifest_path(&self) -> PathBuf {
        self.root.join(MANIFEST_FILE)
    }

    pub fn lease_path(&self) -> PathBuf {
        self.root.join(LEASE_FILE)
    }

    /// Files a sync client made when two devices wrote the same file, e.g. `compter (1).redb`
    pub fn conflict_copies(&self) -> Vec<String> {
        let Ok(entries) = fs::read_dir(&self.root) else {
            return Vec::new();
        };
        let mut found: Vec<String> = entries
            .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
            .filter(|name| is_conflict_copy(name))
            .collect();
        found.sort();
        found
    }
}

/// Where the sync folder in use was set
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncSource {
    /// `$COMPTER_SYNC_DIR`, which overrides Settings
    Env,
    Settings,
}

/// What this device keeps in `device.json`
#[derive(Debug, Default, Serialize, Deserialize)]
struct DeviceSettings {
    #[serde(default)]
    sync_dir: Option<PathBuf>,
}

fn check_folder(data: &DataDir, dir: &Path) -> Result<(), SyncError> {
    let folder = fs::canonicalize(dir)
        .ok()
        .filter(|folder| folder.is_dir())
        .ok_or_else(|| SyncError::NotAFolder(dir.to_path_buf()))?;
    // The data folder may not exist yet on a first start; then nothing can overlap it
    let Ok(home) = fs::canonicalize(data.root()) else {
        return Ok(());
    };
    if folder.starts_with(&home) || home.starts_with(&folder) {
        return Err(SyncError::InsideDataDir(data.root().to_path_buf()));
    }
    Ok(())
}

fn is_conflict_copy(name: &str) -> bool {
    let ours = [SNAPSHOT_FILE, MANIFEST_FILE, LEASE_FILE];
    if ours.contains(&name) {
        return false;
    }
    let lower = name.to_lowercase();
    let stem_matches = ["compter", "manifest", "lease"]
        .iter()
        .any(|stem| lower.starts_with(stem));
    stem_matches && (lower.contains("conflict") || lower.contains(" (") || lower.contains('-'))
}

/// What the snapshot in the synced folder is: written after the snapshot, so a snapshot whose
/// hash doesn't match is still uploading or downloading
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub generation: u64,
    pub sha256: String,
    pub device_id: Uuid,
    pub device_name: String,
    pub written_at: DateTime<Utc>,
}

/// Which device has compter open, refreshed every minute while it runs
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lease {
    pub device_id: Uuid,
    pub device_name: String,
    pub heartbeat: DateTime<Utc>,
}

/// What this device's database was last synced from or to, kept in `<data dir>/sync.json`
#[derive(Debug, Clone, Serialize, Deserialize)]
struct LocalState {
    device_id: Uuid,
    #[serde(default)]
    base_generation: u64,
    /// Hash of the local database file right after that sync. Kept for state written before
    /// `base_content`; redb rewrites its header on every open, so it can't tell an open from an
    /// edit.
    #[serde(default)]
    base_sha256: Option<String>,
    /// [`Store::content_hash`] right after that sync; a different hash means local changes
    #[serde(default)]
    base_content: Option<String>,
}

/// How to settle a [`SyncError::Conflict`]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    /// Publish this device's database; the other device's changes are dropped from the sync
    KeepLocal,
    /// Replace this device's database with the snapshot, keeping the old file as a backup
    KeepRemote,
}

/// Holds the lease while compter runs; [`Self::finish`] publishes what changed
pub struct SyncSession {
    data: DataDir,
    sync: SyncDir,
    state: LocalState,
    device_name: String,
    heartbeat: Option<Heartbeat>,
}

impl SyncSession {
    /// Take the lease and bring the local database up to date. Call before [`Store::open`].
    pub fn begin(
        data: &DataDir,
        sync: SyncDir,
        device_name: impl Into<String>,
    ) -> Result<Self, SyncError> {
        let device_name = device_name.into();
        let mut state = load_state(data)?;
        take_lease(&sync, &state, &device_name)?;
        if let Err(error) = catch_up(data, &sync, &mut state) {
            release_lease(&sync, &state)?;
            return Err(error);
        }
        let heartbeat = Heartbeat::spawn(sync.lease_path(), state.device_id, &device_name);
        Ok(Self {
            data: data.clone(),
            sync,
            state,
            device_name,
            heartbeat: Some(heartbeat),
        })
    }

    /// Conflict copies the sync client left in the folder, worth telling the user about
    pub fn conflict_copies(&self) -> Vec<String> {
        self.sync.conflict_copies()
    }

    /// Close the database, publish it if it changed, and release the lease. Returns the
    /// generation published, if any.
    pub fn finish(mut self, store: Store) -> Result<Option<u64>, SyncError> {
        store.close()?;
        if let Some(heartbeat) = self.heartbeat.take() {
            heartbeat.stop();
        }
        let published = self.publish_changes();
        release_lease(&self.sync, &self.state)?;
        published
    }

    fn publish_changes(&mut self) -> Result<Option<u64>, SyncError> {
        if !local_changed(&self.data, &self.state)? {
            return Ok(None);
        }
        let remote = read_json::<Manifest>(&self.sync.manifest_path())?;
        // Another device published while this one ran (its lease hadn't synced yet): keep ours
        // local, so the next start reports the conflict instead of overwriting theirs
        if let Some(remote) = remote.filter(|m| m.generation > self.state.base_generation) {
            return Err(SyncError::Conflict {
                device: remote.device_name,
                published: remote.written_at,
            });
        }
        let generation = self.state.base_generation + 1;
        publish(
            &self.data,
            &self.sync,
            &mut self.state,
            &self.device_name,
            generation,
        )?;
        Ok(Some(generation))
    }

    /// Settle a conflict that [`Self::begin`] refused to open with
    pub fn resolve(
        data: &DataDir,
        sync: &SyncDir,
        device_name: &str,
        resolution: Resolution,
    ) -> Result<(), SyncError> {
        let mut state = load_state(data)?;
        take_lease(sync, &state, device_name)?;
        let settled = settle(data, sync, &mut state, device_name, resolution);
        release_lease(sync, &state)?;
        settled
    }
}

fn settle(
    data: &DataDir,
    sync: &SyncDir,
    state: &mut LocalState,
    device_name: &str,
    resolution: Resolution,
) -> Result<(), SyncError> {
    let manifest = read_json::<Manifest>(&sync.manifest_path())?;
    match resolution {
        Resolution::KeepLocal => {
            if !data.database_path().exists() {
                return Err(SyncError::NoLocalDatabase);
            }
            let latest = manifest.map_or(0, |m| m.generation);
            let generation = latest.max(state.base_generation) + 1;
            publish(data, sync, state, device_name, generation)
        }
        Resolution::KeepRemote => {
            let manifest =
                manifest.ok_or_else(|| SyncError::NoSnapshot(sync.root().to_path_buf()))?;
            // Check before moving the local copy aside, so a half-synced snapshot replaces nothing
            if hash_file(&sync.snapshot_path())?.as_deref() != Some(manifest.sha256.as_str()) {
                return Err(SyncError::Incomplete {
                    device: manifest.device_name,
                });
            }
            let local = data.database_path();
            if local.exists() {
                let backup = data.conflict_backup_path(Utc::now());
                fs::rename(&local, &backup).map_err(io_error(&local))?;
            }
            pull(data, sync, &manifest, state)
        }
    }
}

impl Drop for SyncSession {
    fn drop(&mut self) {
        // Without `finish` (a crash, or the database couldn't close): stop refreshing the lease
        // but leave it, so it goes stale instead of inviting another device in over unpublished
        // work
        if let Some(heartbeat) = self.heartbeat.take() {
            heartbeat.stop();
        }
    }
}

/// Where this device's database stands against the sync folder
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncState {
    /// The snapshot generation it last synced from or to; 0 before any sync
    pub generation: u64,
    /// It changed since then
    pub changed: bool,
    /// The SHA-256 of that snapshot, as its manifest records it; a manifest with the same hash
    /// is a snapshot this device already has
    pub sha256: Option<String>,
}

/// Where this device's database, open as `store`, stands; the sync folder isn't touched
pub fn state(data: &DataDir, store: &Store) -> Result<SyncState, SyncError> {
    let state = load_state(data)?;
    let changed = match &state.base_content {
        Some(base) => store.content_hash()? != *base,
        None => hash_file(&data.database_path())? != state.base_sha256,
    };
    Ok(SyncState {
        generation: state.base_generation,
        changed,
        sha256: state.base_sha256,
    })
}

/// This device's lease, heartbeat now, for a device that writes the lease to the sync folder
/// itself (a phone, through a cloud API)
pub fn lease_for(data: &DataDir, device_name: &str) -> Result<Lease, SyncError> {
    let state = load_state(data)?;
    Ok(Lease {
        device_id: state.device_id,
        device_name: device_name.to_owned(),
        heartbeat: Utc::now(),
    })
}

/// `lease`, found in the sync folder, when another device holds it and it isn't stale: that
/// device has compter open
pub fn held_by_other(data: &DataDir, lease: &Lease) -> Result<Option<Lease>, SyncError> {
    let state = load_state(data)?;
    Ok((lease.device_id != state.device_id && is_fresh(lease)).then(|| lease.clone()))
}

// Hand-off for a device that can't see the sync folder, like a phone. The user carries the
// snapshot and manifest between the folder and the device; the generation checks are the same.
// There is no lease, since nothing stays open across a hand-off. Close the store first.

/// Take the snapshot in `from`, a copy of the sync folder's snapshot and manifest. A `blank`
/// local database (no records yet, see [`Store::is_blank`]) is replaced even though it never
/// synced. Returns the generation taken, or `None` when this device already has it.
///
/// A hand-off can go wrong where a sync client can't: the user may never save what
/// [`send`] wrote, or a sync app may store it as a copy. So the folder's snapshot must be the
/// very one this device last synced, or a newer one; an older or different one is refused as
/// [`SyncError::Undelivered`], since taking it or sending over it would lose records.
pub fn receive(data: &DataDir, from: &SyncDir, blank: bool) -> Result<Option<u64>, SyncError> {
    let mut state = load_state(data)?;
    let manifest = read_json::<Manifest>(&from.manifest_path())?
        .ok_or_else(|| SyncError::NoSnapshot(from.root().to_path_buf()))?;
    if state.base_sha256.as_deref() == Some(manifest.sha256.as_str()) {
        return Ok(None);
    }
    if manifest.generation <= state.base_generation {
        return Err(SyncError::Undelivered {
            sent: state.base_generation,
            found: manifest.generation,
        });
    }
    if !blank && local_changed(data, &state)? {
        return Err(SyncError::Conflict {
            device: manifest.device_name,
            published: manifest.written_at,
        });
    }
    pull(data, from, &manifest, &mut state)?;
    Ok(Some(manifest.generation))
}

/// Write this device's database to `to` as a snapshot and manifest, for the user to put in the
/// sync folder. A database changed since it last synced gets the next generation; an unchanged
/// one is written again under its own. Returns the generation written.
pub fn send(data: &DataDir, to: &SyncDir, device_name: &str) -> Result<u64, SyncError> {
    if !data.database_path().exists() {
        return Err(SyncError::NoLocalDatabase);
    }
    let mut state = load_state(data)?;
    let generation = if local_changed(data, &state)? {
        state.base_generation + 1
    } else {
        state.base_generation
    };
    publish(data, to, &mut state, device_name, generation)?;
    Ok(generation)
}

/// Settle a conflict or undelivered hand-off [`receive`] refused. [`Resolution::KeepRemote`]
/// takes the snapshot in `from`, keeping the local database beside it as a backup;
/// [`Resolution::KeepLocal`] keeps this device's, so the next [`send`] goes out as new, above
/// the snapshot's generation.
pub fn settle_received(
    data: &DataDir,
    from: &SyncDir,
    device_name: &str,
    resolution: Resolution,
) -> Result<(), SyncError> {
    let mut state = load_state(data)?;
    match resolution {
        Resolution::KeepRemote => settle(data, from, &mut state, device_name, resolution),
        Resolution::KeepLocal => {
            let manifest = read_json::<Manifest>(&from.manifest_path())?
                .ok_or_else(|| SyncError::NoSnapshot(from.root().to_path_buf()))?;
            state.base_generation = state.base_generation.max(manifest.generation);
            // Counts as changed, so the next send takes a new generation
            state.base_sha256 = None;
            state.base_content = None;
            save_state(data, &state)
        }
    }
}

/// `$COMPTER_DEVICE`, else the host name, else a generic label
pub fn device_name_from_env() -> String {
    ["COMPTER_DEVICE", "HOSTNAME", "COMPUTERNAME", "HOST"]
        .iter()
        .find_map(|key| env::var(key).ok().filter(|value| !value.is_empty()))
        .unwrap_or_else(|| "another device".to_owned())
}

fn load_state(data: &DataDir) -> Result<LocalState, SyncError> {
    let path = data.sync_state_path();
    if let Some(state) = read_json(&path)? {
        return Ok(state);
    }
    let state = LocalState {
        device_id: Uuid::new_v4(),
        base_generation: 0,
        // A database that predates syncing counts as changed, so it is never silently replaced
        base_sha256: None,
        base_content: None,
    };
    save_state(data, &state)?;
    Ok(state)
}

fn save_state(data: &DataDir, state: &LocalState) -> Result<(), SyncError> {
    write_json(&data.sync_state_path(), state)
}

/// Pull a newer snapshot, unless this device changed its own copy since it last synced
fn catch_up(data: &DataDir, sync: &SyncDir, state: &mut LocalState) -> Result<(), SyncError> {
    let Some(manifest) = read_json::<Manifest>(&sync.manifest_path())? else {
        return Ok(());
    };
    if manifest.generation <= state.base_generation {
        return Ok(());
    }
    if local_changed(data, state)? {
        return Err(SyncError::Conflict {
            device: manifest.device_name,
            published: manifest.written_at,
        });
    }
    pull(data, sync, &manifest, state)
}

fn take_lease(sync: &SyncDir, state: &LocalState, device_name: &str) -> Result<(), SyncError> {
    if let Some(lease) = read_json::<Lease>(&sync.lease_path())?
        && lease.device_id != state.device_id
        && is_fresh(&lease)
    {
        return Err(SyncError::Leased {
            device: lease.device_name,
            seen: lease.heartbeat,
        });
    }
    write_lease(&sync.lease_path(), state.device_id, device_name)
}

/// Refreshed within [`LEASE_TTL`]; a heartbeat in the future (clocks disagree) counts as fresh
fn is_fresh(lease: &Lease) -> bool {
    let age = Utc::now().signed_duration_since(lease.heartbeat);
    !matches!(age.to_std(), Ok(age) if age >= LEASE_TTL)
}

fn write_lease(path: &Path, device_id: Uuid, device_name: &str) -> Result<(), SyncError> {
    let lease = Lease {
        device_id,
        device_name: device_name.to_owned(),
        heartbeat: Utc::now(),
    };
    write_json(path, &lease)
}

fn release_lease(sync: &SyncDir, state: &LocalState) -> Result<(), SyncError> {
    let path = sync.lease_path();
    match read_json::<Lease>(&path)? {
        Some(lease) if lease.device_id == state.device_id => {
            fs::remove_file(&path).map_err(io_error(&path))
        }
        _ => Ok(()),
    }
}

/// Did the local database's records change since it was last synced?
fn local_changed(data: &DataDir, state: &LocalState) -> Result<bool, SyncError> {
    match &state.base_content {
        Some(base) => Ok(content_hash(data)?.as_ref() != Some(base)),
        // Synced before contents were hashed: compare the file, as then
        None => Ok(hash_file(&data.database_path())? != state.base_sha256),
    }
}

/// [`Store::content_hash`] of the local database, `None` when there is none. The store must be
/// closed.
fn content_hash(data: &DataDir) -> Result<Option<String>, SyncError> {
    if !data.database_path().exists() {
        return Ok(None);
    }
    let store = Store::open(data)?;
    let hash = store.content_hash()?;
    store.close()?;
    Ok(Some(hash))
}

fn pull(
    data: &DataDir,
    sync: &SyncDir,
    manifest: &Manifest,
    state: &mut LocalState,
) -> Result<(), SyncError> {
    let path = sync.snapshot_path();
    let bytes = fs::read(&path).map_err(io_error(&path))?;
    let sha256 = sha256_hex(&bytes);
    if sha256 != manifest.sha256 {
        return Err(SyncError::Incomplete {
            device: manifest.device_name.clone(),
        });
    }
    fs::create_dir_all(data.root()).map_err(io_error(data.root()))?;
    write_file(&data.database_path(), &bytes)?;
    state.base_generation = manifest.generation;
    state.base_sha256 = Some(sha256);
    state.base_content = content_hash(data)?;
    save_state(data, state)
}

/// Snapshot first, manifest second: a device that sees the new manifest before the snapshot
/// arrives finds a hash mismatch and waits
fn publish(
    data: &DataDir,
    sync: &SyncDir,
    state: &mut LocalState,
    device_name: &str,
    generation: u64,
) -> Result<(), SyncError> {
    let local = data.database_path();
    let bytes = fs::read(&local).map_err(io_error(&local))?;
    let sha256 = sha256_hex(&bytes);
    write_file(&sync.snapshot_path(), &bytes)?;
    let manifest = Manifest {
        generation,
        sha256: sha256.clone(),
        device_id: state.device_id,
        device_name: device_name.to_owned(),
        written_at: Utc::now(),
    };
    write_json(&sync.manifest_path(), &manifest)?;
    state.base_generation = generation;
    state.base_sha256 = Some(sha256);
    state.base_content = content_hash(data)?;
    save_state(data, state)
}

/// `None` when the file doesn't exist
fn hash_file(path: &Path) -> Result<Option<String>, SyncError> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(sha256_hex(&bytes))),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(SyncError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<Option<T>, SyncError> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(SyncError::Io {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|source| SyncError::Json {
            path: path.to_path_buf(),
            source,
        })
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), SyncError> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|source| SyncError::Json {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(write_file(path, &bytes)?)
}

fn io_error(path: &Path) -> impl FnOnce(io::Error) -> SyncError + '_ {
    move |source| SyncError::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// Rewrites the lease every minute until stopped
struct Heartbeat {
    stop: SyncSender<()>,
    thread: JoinHandle<()>,
}

impl Heartbeat {
    fn spawn(path: PathBuf, device_id: Uuid, device_name: &str) -> Self {
        let (stop, stopped) = mpsc::sync_channel(1);
        let device_name = device_name.to_owned();
        let thread = thread::spawn(move || {
            while let Err(RecvTimeoutError::Timeout) = stopped.recv_timeout(HEARTBEAT) {
                // A missed beat only lets the lease go stale sooner; the next one retries
                let _ = write_lease(&path, device_id, &device_name);
            }
        });
        Self { stop, thread }
    }

    fn stop(self) {
        let _ = self.stop.try_send(());
        let _ = self.thread.join();
    }
}
