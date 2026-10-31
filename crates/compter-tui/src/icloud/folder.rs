use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use compter_core::storage::sync::{SyncDir, SyncError, SyncFolder};

use super::PLACE;

/// How long a read waits for iCloud to download a file it evicted to save space
const DOWNLOAD_WAIT: Duration = Duration::from_secs(60);
const POLL: Duration = Duration::from_millis(250);

/// The iCloud container as a [`SyncFolder`]: plain files, except that iCloud may have evicted
/// one, leaving a `.<name>.icloud` placeholder that has to be downloaded before it can be read.
/// A container that isn't on this Mac means iCloud Drive is off or signed out.
pub struct ICloudFolder {
    /// `None` off a Mac
    root: Option<PathBuf>,
    download: fn(&Path) -> io::Result<()>,
    wait: Duration,
}

impl ICloudFolder {
    /// The container at `root` ([`super::container`])
    pub fn new(root: Option<PathBuf>) -> Self {
        Self::with_download(root, brctl_download, DOWNLOAD_WAIT)
    }

    pub(crate) fn with_download(
        root: Option<PathBuf>,
        download: fn(&Path) -> io::Result<()>,
        wait: Duration,
    ) -> Self {
        Self {
            root,
            download,
            wait,
        }
    }

    /// The container as a folder, when iCloud Drive has put it on this Mac
    fn dir(&self) -> Result<SyncDir, SyncError> {
        self.root
            .as_ref()
            .filter(|root| root.is_dir())
            .map(SyncDir::new)
            .ok_or_else(|| SyncError::SignedOut(PLACE.to_owned()))
    }

    /// Bring back a file iCloud evicted. Newer macOS keeps evicted files under their own name
    /// and downloads them on read; older versions leave only the placeholder.
    fn materialise(&self, dir: &SyncDir, name: &str) -> Result<(), SyncError> {
        let path = dir.root().join(name);
        if path.exists() || !dir.root().join(placeholder(name)).exists() {
            return Ok(());
        }
        (self.download)(&path).map_err(|source| remote(source.into()))?;
        let started = Instant::now();
        while !path.exists() {
            if started.elapsed() >= self.wait {
                return Err(remote(
                    format!("{name} is still downloading from iCloud").into(),
                ));
            }
            thread::sleep(POLL);
        }
        Ok(())
    }
}

impl SyncFolder for ICloudFolder {
    fn read(&self, name: &str) -> Result<Option<Vec<u8>>, SyncError> {
        let dir = self.dir()?;
        self.materialise(&dir, name)?;
        dir.read(name)
    }

    fn write(&self, name: &str, bytes: &[u8]) -> Result<(), SyncError> {
        self.dir()?.write(name, bytes)
    }

    fn delete(&self, name: &str) -> Result<(), SyncError> {
        let dir = self.dir()?;
        dir.delete(&placeholder(name))?;
        dir.delete(name)
    }

    fn place(&self) -> String {
        PLACE.to_owned()
    }

    fn reconnect(&self) -> &'static str {
        "turn on iCloud Drive in System Settings, Apple Account, iCloud"
    }

    fn conflict_copies(&self) -> Vec<String> {
        self.dir()
            .map(|dir| dir.conflict_copies())
            .unwrap_or_default()
    }
}

/// What iCloud leaves in place of an evicted file
pub(crate) fn placeholder(name: &str) -> String {
    format!(".{name}.icloud")
}

fn remote(source: Box<dyn std::error::Error + Send + Sync>) -> SyncError {
    SyncError::Remote {
        place: PLACE.to_owned(),
        source,
    }
}

/// Ask iCloud's daemon to download `path`; it returns before the download finishes
fn brctl_download(path: &Path) -> io::Result<()> {
    let status = Command::new("brctl")
        .arg("download")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("brctl download {status}")))
    }
}
