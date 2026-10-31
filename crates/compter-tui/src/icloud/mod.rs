//! Sync through the app's hidden iCloud container on a Mac. The iPhone app owns the container
//! (`iCloud.app.compter`, iCloud Documents); iCloud Drive mirrors it on every Mac signed in to
//! the same Apple Account, where the terminal reads and writes it as a folder. It isn't shown in
//! Finder or the Files app, so the sync files can't be moved or deleted by accident.

mod folder;

use std::env;
use std::path::PathBuf;

pub use folder::ICloudFolder;

/// What the user sees for this sync backend in messages
pub const PLACE: &str = "iCloud Drive";

/// The container's folder name under `~/Library/Mobile Documents`
const CONTAINER: &str = "iCloud~app~compter";

/// The container's `Documents` folder, where the phone keeps the sync files; `None` off a Mac.
/// The folder exists only once iCloud Drive is on and the phone has synced through it.
pub fn container() -> Option<PathBuf> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let home = env::var_os("HOME").filter(|home| !home.is_empty())?;
    Some(
        PathBuf::from(home)
            .join("Library/Mobile Documents")
            .join(CONTAINER)
            .join("Documents"),
    )
}

#[cfg(test)]
mod tests;
