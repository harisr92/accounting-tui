//! The sync files in Google Drive's `appDataFolder`, found by name. Their IDs are remembered,
//! so a write updates the same file rather than adding another of the same name.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use compter_core::storage::sync::{SNAPSHOT_FILE, SyncError, SyncFolder};
use serde::Deserialize;
use ureq::Agent;
use ureq::http::Response;

use super::oauth::{self, Client};
use super::{DriveError, PLACE, token};

const FILES_URL: &str = "https://www.googleapis.com/drive/v3/files";
const UPLOAD_URL: &str = "https://www.googleapis.com/upload/drive/v3/files";
const BOUNDARY: &str = "compter-sync-boundary";

/// Google Drive's hidden app folder, as `account`
pub struct DriveFolder {
    client: Client,
    account: String,
    /// For everything but the snapshot: short, so quitting never waits long on the lease
    agent: Agent,
    /// For the snapshot, which may be large
    transfer: Agent,
    session: Mutex<Session>,
}

/// What a sign-in gave: tokens, and the IDs of the files found so far
#[derive(Default)]
struct Session {
    access_token: Option<String>,
    ids: HashMap<String, String>,
}

impl DriveFolder {
    pub fn new(client: Client, account: impl Into<String>) -> Self {
        Self {
            client,
            account: account.into(),
            agent: oauth::agent(oauth::SHORT),
            transfer: oauth::agent(oauth::TRANSFER),
            session: Mutex::default(),
        }
    }

    pub fn account(&self) -> &str {
        &self.account
    }

    fn session(&self) -> MutexGuard<'_, Session> {
        // A panic mid-request leaves only a token or an ID behind, both safe to reuse
        self.session.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The agent for moving `name`'s contents
    fn agent_for(&self, name: &str) -> &Agent {
        if name == SNAPSHOT_FILE {
            &self.transfer
        } else {
            &self.agent
        }
    }

    /// Run `request` through `agent` with an access token, once more with a fresh one if Google
    /// refused it
    fn call(
        &self,
        agent: &Agent,
        request: impl Fn(&Agent, &str) -> Result<Response<ureq::Body>, ureq::Error>,
    ) -> Result<Response<ureq::Body>, DriveError> {
        let cached = self.session().access_token.clone();
        let token = match cached {
            Some(token) => token,
            None => self.refresh()?,
        };
        let response = request(agent, &token)?;
        if response.status().as_u16() != 401 {
            return Ok(response);
        }
        let response = request(agent, &self.refresh()?)?;
        if response.status().as_u16() == 401 {
            return Err(DriveError::SignedOut);
        }
        Ok(response)
    }

    fn refresh(&self) -> Result<String, DriveError> {
        let refresh_token = token::load(&self.account)?;
        let access = oauth::refresh(&self.agent, &self.client, &refresh_token)?;
        self.session().access_token = Some(access.clone());
        Ok(access)
    }

    fn file_id(&self, name: &str) -> Result<Option<String>, DriveError> {
        if let Some(id) = self.session().ids.get(name) {
            return Ok(Some(id.clone()));
        }
        let query = format!("name = '{}' and trashed = false", name.replace('\'', "\\'"));
        let mut response = self.call(&self.agent, |agent, token| {
            agent
                .get(FILES_URL)
                .header("Authorization", format!("Bearer {token}"))
                .query("spaces", "appDataFolder")
                .query("q", &query)
                .query("fields", "files(id)")
                // Two devices may each have created one at once; the newest wins
                .query("orderBy", "modifiedTime desc")
                .query("pageSize", "1")
                .call()
        })?;
        check(&mut response)?;
        let id = response
            .body_mut()
            .read_json::<FileList>()?
            .files
            .into_iter()
            .next()
            .map(|file| file.id);
        if let Some(id) = &id {
            self.session().ids.insert(name.to_owned(), id.clone());
        }
        Ok(id)
    }

    fn forget(&self, name: &str) {
        self.session().ids.remove(name);
    }

    fn read_file(&self, name: &str) -> Result<Option<Vec<u8>>, DriveError> {
        let Some(id) = self.file_id(name)? else {
            return Ok(None);
        };
        let url = format!("{FILES_URL}/{id}");
        let mut response = self.call(self.agent_for(name), |agent, token| {
            agent
                .get(&url)
                .header("Authorization", format!("Bearer {token}"))
                .query("alt", "media")
                .call()
        })?;
        // Deleted by another device since its ID was found
        if response.status().as_u16() == 404 {
            self.forget(name);
            return Ok(None);
        }
        check(&mut response)?;
        // The snapshot can outgrow ureq's default 10 MB cap
        Ok(Some(
            response
                .body_mut()
                .with_config()
                .limit(u64::MAX)
                .read_to_vec()?,
        ))
    }

    fn write_file(&self, name: &str, bytes: &[u8]) -> Result<(), DriveError> {
        if let Some(id) = self.file_id(name)? {
            let url = format!("{UPLOAD_URL}/{id}");
            let mut response = self.call(self.agent_for(name), |agent, token| {
                agent
                    .patch(&url)
                    .header("Authorization", format!("Bearer {token}"))
                    .query("uploadType", "media")
                    .content_type("application/octet-stream")
                    .send(bytes)
            })?;
            if response.status().as_u16() != 404 {
                return check(&mut response);
            }
            self.forget(name);
        }
        let body = multipart(name, bytes);
        let mut response = self.call(self.agent_for(name), |agent, token| {
            agent
                .post(UPLOAD_URL)
                .header("Authorization", format!("Bearer {token}"))
                .query("uploadType", "multipart")
                .query("fields", "id")
                .content_type(format!("multipart/related; boundary={BOUNDARY}"))
                .send(&body[..])
        })?;
        check(&mut response)?;
        let created = response.body_mut().read_json::<File>()?;
        self.session().ids.insert(name.to_owned(), created.id);
        Ok(())
    }

    fn delete_file(&self, name: &str) -> Result<(), DriveError> {
        let Some(id) = self.file_id(name)? else {
            return Ok(());
        };
        let url = format!("{FILES_URL}/{id}");
        let mut response = self.call(&self.agent, |agent, token| {
            agent
                .delete(&url)
                .header("Authorization", format!("Bearer {token}"))
                .call()
        })?;
        self.forget(name);
        if response.status().as_u16() == 404 {
            return Ok(());
        }
        check(&mut response)
    }
}

impl SyncFolder for DriveFolder {
    fn read(&self, name: &str) -> Result<Option<Vec<u8>>, SyncError> {
        self.read_file(name).map_err(to_sync)
    }

    fn write(&self, name: &str, bytes: &[u8]) -> Result<(), SyncError> {
        self.write_file(name, bytes).map_err(to_sync)
    }

    fn delete(&self, name: &str) -> Result<(), SyncError> {
        self.delete_file(name).map_err(to_sync)
    }

    fn place(&self) -> String {
        PLACE.to_owned()
    }
}

#[derive(Deserialize)]
pub struct FileList {
    #[serde(default)]
    pub files: Vec<File>,
}

#[derive(Deserialize)]
pub struct File {
    pub id: String,
}

#[derive(Deserialize)]
struct ApiError {
    error: ApiErrorBody,
}

#[derive(Deserialize)]
struct ApiErrorBody {
    message: String,
}

/// The upload that creates `name` in the app folder: its metadata, then its bytes
pub fn multipart(name: &str, bytes: &[u8]) -> Vec<u8> {
    let metadata = serde_json::json!({ "name": name, "parents": ["appDataFolder"] });
    let mut body = Vec::with_capacity(bytes.len() + 256);
    body.extend_from_slice(
        format!(
            "--{BOUNDARY}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{metadata}\r\n\
             --{BOUNDARY}\r\nContent-Type: application/octet-stream\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
    body
}

/// A refusal other than an expired token, with Google's own message
fn check(response: &mut Response<ureq::Body>) -> Result<(), DriveError> {
    let status = response.status().as_u16();
    if (200..300).contains(&status) {
        return Ok(());
    }
    let message = response
        .body_mut()
        .read_json::<ApiError>()
        .map(|e| e.error.message)
        .unwrap_or_default();
    Err(DriveError::Status { status, message })
}

fn to_sync(error: DriveError) -> SyncError {
    match error {
        DriveError::SignedOut => SyncError::SignedOut(PLACE.to_owned()),
        error => SyncError::Remote {
            place: PLACE.to_owned(),
            source: Box::new(error),
        },
    }
}
