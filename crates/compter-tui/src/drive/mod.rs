//! Sync through Google Drive's hidden app folder, reached through the Drive API. The phone app
//! signs in to the same Cloud project, so both devices share one `appDataFolder` under the
//! narrow `drive.appdata` scope; neither ever needs to see the rest of the user's Drive.

mod folder;
mod oauth;
mod token;

use std::io;

pub use folder::DriveFolder;
pub use oauth::Client;

/// What the user sees for this sync backend in messages
pub const PLACE: &str = "Google Drive";

/// Why talking to Google failed
#[derive(Debug, thiserror::Error)]
pub enum DriveError {
    #[error(
        "Google sign-in isn't set up in this build (COMPTER_GOOGLE_DESKTOP_CLIENT_ID and _SECRET)"
    )]
    NotConfigured,
    #[error("signed out of Google Drive; run `compter drive sign-in`")]
    SignedOut,
    #[error("Google sign-in was refused: {0}")]
    Refused(String),
    #[error("Google Drive answered {status}: {message}")]
    Status { status: u16, message: String },
    #[error("unexpected answer from Google: {0}")]
    Unexpected(String),
    #[error("could not reach Google")]
    Http(#[from] ureq::Error),
    #[error("could not use this computer's keychain; sync through a folder instead (Settings, s)")]
    Keychain(#[from] keyring::Error),
    #[error("could not wait for Google's sign-in page")]
    Io(#[from] io::Error),
}

/// Sign in to Google in the browser, which `open` shows the consent page in, and keep the grant
/// in the keychain. Returns the account signed in to.
pub fn sign_in(client: &Client, open: impl FnOnce(&str)) -> Result<String, DriveError> {
    let granted = oauth::sign_in(client, open)?;
    token::save(&granted.account, &granted.refresh_token)?;
    Ok(granted.account)
}

/// Forget the grant for `account` on this computer
pub fn sign_out(account: &str) -> Result<(), DriveError> {
    token::delete(account)
}

#[cfg(test)]
mod tests;
