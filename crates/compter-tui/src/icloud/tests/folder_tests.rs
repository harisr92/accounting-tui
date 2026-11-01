use std::fs;
use std::io;
use std::path::Path;
use std::time::Duration;

use compter_core::storage::sync::{SyncError, SyncFolder};

use crate::icloud::ICloudFolder;
use crate::icloud::folder::placeholder;

fn restore(path: &Path) -> io::Result<()> {
    fs::write(path, b"downloaded")
}

fn unreachable(_: &Path) -> io::Result<()> {
    Err(io::Error::other("offline"))
}

fn never_arrives(_: &Path) -> io::Result<()> {
    Ok(())
}

#[test]
fn an_evicted_file_is_downloaded_before_it_is_read() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(placeholder("manifest.json")), b"").unwrap();
    let folder =
        ICloudFolder::with_download(Some(dir.path().into()), restore, Duration::from_secs(1));

    assert_eq!(
        folder.read("manifest.json").unwrap().as_deref(),
        Some(&b"downloaded"[..])
    );
    assert_eq!(folder.read("lease.json").unwrap(), None);
}

#[test]
fn a_download_that_fails_or_never_arrives_is_a_cloud_that_cant_be_reached() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(placeholder("compter.redb")), b"").unwrap();
    for download in [unreachable, never_arrives] {
        let folder = ICloudFolder::with_download(Some(dir.path().into()), download, Duration::ZERO);
        assert!(matches!(
            folder.read("compter.redb"),
            Err(SyncError::Remote { .. })
        ));
    }
}

#[test]
fn a_container_missing_from_this_mac_is_signed_out() {
    let dir = tempfile::tempdir().unwrap();
    for root in [None, Some(dir.path().join("iCloud~app~compter"))] {
        let folder = ICloudFolder::with_download(root, restore, Duration::ZERO);
        assert!(matches!(
            folder.write("lease.json", b"{}"),
            Err(SyncError::SignedOut(_))
        ));
        assert!(folder.conflict_copies().is_empty());
    }
}

#[test]
fn deleting_a_file_also_removes_its_placeholder() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(placeholder("lease.json")), b"").unwrap();
    let folder = ICloudFolder::new(Some(dir.path().into()));

    folder.delete("lease.json").unwrap();
    assert!(fs::read_dir(dir.path()).unwrap().next().is_none());
}
