//! The refresh token, kept in the OS keychain (Keychain, Credential Manager, Secret Service)
//! rather than in the data folder, which travels in backups

use keyring::Entry;

use super::DriveError;

const SERVICE: &str = "compter-google-drive";

pub fn save(account: &str, refresh_token: &str) -> Result<(), DriveError> {
    Ok(Entry::new(SERVICE, account)?.set_password(refresh_token)?)
}

/// The grant for `account`; none is [`DriveError::SignedOut`]
pub fn load(account: &str) -> Result<String, DriveError> {
    match Entry::new(SERVICE, account)?.get_password() {
        Err(keyring::Error::NoEntry) => Err(DriveError::SignedOut),
        found => Ok(found?),
    }
}

pub fn delete(account: &str) -> Result<(), DriveError> {
    match Entry::new(SERVICE, account)?.delete_credential() {
        Err(keyring::Error::NoEntry) => Ok(()),
        done => Ok(done?),
    }
}
