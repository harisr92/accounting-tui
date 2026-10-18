//! Opens a written file (an invoice or credit note PDF) in the desktop's default app. The core
//! only writes the file; how a front end shows it is the front end's business.

use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The viewer couldn't be started; the file itself was written
#[derive(Debug, thiserror::Error)]
#[error("could not open {path} in a PDF viewer")]
pub struct ViewerError {
    path: PathBuf,
    #[source]
    source: io::Error,
}

/// Open `path` with `$COMPTER_OPEN`, else `open` / `xdg-open` / `start`, without waiting for it
pub fn open(path: &Path) -> Result<(), ViewerError> {
    viewer_command()
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(drop)
        .map_err(|source| ViewerError {
            path: path.to_path_buf(),
            source,
        })
}

/// `$COMPTER_OPEN`, else the platform's "open with the default app" command, waiting for a path
fn viewer_command() -> Command {
    if let Ok(opener) = std::env::var("COMPTER_OPEN") {
        return Command::new(opener);
    }
    if cfg!(windows) {
        // `start` is a cmd built-in, and its first quoted argument is the window title
        let mut command = Command::new("cmd");
        command.args(["/C", "start", ""]);
        command
    } else if cfg!(target_os = "macos") {
        Command::new("open")
    } else {
        Command::new("xdg-open")
    }
}
