//! Google sign-in for an installed app: the consent page opens in the browser, which comes back
//! to a one-off server on 127.0.0.1 with a code, exchanged for tokens with PKCE

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::DriveError;

const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
/// The app's own hidden folder, plus the account's address to show who is signed in
pub const SCOPES: &str = "openid email https://www.googleapis.com/auth/drive.appdata";

/// The Desktop OAuth client of the Cloud project the phone app uses, set when building. The
/// secret of an installed app isn't secret; Google still asks for it.
#[derive(Debug, Clone, Copy)]
pub struct Client {
    pub id: &'static str,
    pub secret: &'static str,
}

impl Client {
    pub fn from_build() -> Result<Self, DriveError> {
        match (
            option_env!("COMPTER_GOOGLE_DESKTOP_CLIENT_ID"),
            option_env!("COMPTER_GOOGLE_DESKTOP_CLIENT_SECRET"),
        ) {
            (Some(id), Some(secret)) if !id.is_empty() && !secret.is_empty() => {
                Ok(Self { id, secret })
            }
            _ => Err(DriveError::NotConfigured),
        }
    }
}

/// A finished sign-in
pub struct Granted {
    pub account: String,
    pub refresh_token: String,
}

/// The PKCE pair: the verifier stays here, its hash goes to the consent page
pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
}

impl Pkce {
    pub fn from_random(random: &[u8; 32]) -> Self {
        let verifier = URL_SAFE_NO_PAD.encode(random);
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        Self {
            verifier,
            challenge,
        }
    }
}

pub fn consent_url(client_id: &str, redirect: &str, challenge: &str, state: &str) -> String {
    let query = form_urlencoded::Serializer::new(String::new())
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", redirect)
        .append_pair("response_type", "code")
        .append_pair("scope", SCOPES)
        .append_pair("code_challenge", challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", state)
        // A refresh token, every time: one from an earlier grant may be gone from the keychain
        .append_pair("access_type", "offline")
        .append_pair("prompt", "consent")
        .finish();
    format!("{AUTH_URL}?{query}")
}

/// What the browser brought back to the loopback server
#[derive(Debug, PartialEq, Eq)]
pub enum Redirect {
    Code(String),
    Refused(String),
    /// Another request (a favicon), or one that doesn't belong to this sign-in
    Other,
}

/// Read the first line of the browser's request, `GET /?code=…&state=… HTTP/1.1`
pub fn parse_redirect(request_line: &str, state: &str) -> Redirect {
    let Some(target) = request_line.split_whitespace().nth(1) else {
        return Redirect::Other;
    };
    let Some((_, query)) = target.split_once('?') else {
        return Redirect::Other;
    };
    let (mut code, mut error, mut matches) = (None, None, false);
    for (key, value) in form_urlencoded::parse(query.as_bytes()) {
        match &*key {
            "code" => code = Some(value.into_owned()),
            "error" => error = Some(value.into_owned()),
            "state" => matches = value == state,
            _ => {}
        }
    }
    match (matches, code, error) {
        (false, _, _) => Redirect::Other,
        (true, _, Some(error)) => Redirect::Refused(error),
        (true, Some(code), None) => Redirect::Code(code),
        (true, None, None) => Redirect::Other,
    }
}

/// The `email` claim of the ID token. It came straight from Google's token endpoint over TLS,
/// so its signature needn't be checked.
pub fn email_from_id_token(id_token: &str) -> Result<String, DriveError> {
    #[derive(Deserialize)]
    struct Claims {
        email: String,
    }
    let unexpected = || DriveError::Unexpected("an ID token without an address".to_owned());
    let payload = id_token.split('.').nth(1).ok_or_else(unexpected)?;
    let json = URL_SAFE_NO_PAD
        .decode(payload.trim_end_matches('='))
        .map_err(|_| unexpected())?;
    let claims: Claims = serde_json::from_slice(&json).map_err(|_| unexpected())?;
    Ok(claims.email)
}

#[derive(Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub id_token: Option<String>,
}

#[derive(Deserialize)]
struct TokenError {
    error: String,
}

pub fn sign_in(client: &Client, open: impl FnOnce(&str)) -> Result<Granted, DriveError> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let redirect = format!("http://127.0.0.1:{}", listener.local_addr()?.port());
    let pkce = Pkce::from_random(&random()?);
    let state = URL_SAFE_NO_PAD.encode(random()?);
    open(&consent_url(client.id, &redirect, &pkce.challenge, &state));

    let code = loop {
        let (stream, _) = listener.accept()?;
        match answer(stream, &state)? {
            Redirect::Code(code) => break code,
            Redirect::Refused(error) => return Err(DriveError::Refused(error)),
            Redirect::Other => {}
        }
    };
    let tokens = exchange(
        &agent(),
        client,
        &[
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("redirect_uri", &redirect),
            ("code_verifier", &pkce.verifier),
        ],
    )?;
    let missing = |what: &str| DriveError::Unexpected(format!("no {what} in Google's answer"));
    let id_token = tokens.id_token.ok_or_else(|| missing("ID token"))?;
    Ok(Granted {
        account: email_from_id_token(&id_token)?,
        refresh_token: tokens
            .refresh_token
            .ok_or_else(|| missing("refresh token"))?,
    })
}

/// A fresh access token for the grant `refresh_token`; a revoked grant is
/// [`DriveError::SignedOut`]
pub fn refresh(
    agent: &ureq::Agent,
    client: &Client,
    refresh_token: &str,
) -> Result<String, DriveError> {
    let tokens = exchange(
        agent,
        client,
        &[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ],
    )?;
    Ok(tokens.access_token)
}

/// Requests to Google: statuses are checked by the caller, and a slow link still finishes
/// uploading a snapshot
pub fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_global(Some(Duration::from_secs(300)))
        .build()
        .into()
}

fn exchange(
    agent: &ureq::Agent,
    client: &Client,
    form: &[(&str, &str)],
) -> Result<TokenResponse, DriveError> {
    let credentials = [("client_id", client.id), ("client_secret", client.secret)];
    let mut response = agent
        .post(TOKEN_URL)
        .send_form(credentials.iter().chain(form).copied())?;
    let status = response.status().as_u16();
    if status == 200 {
        return Ok(response.body_mut().read_json()?);
    }
    let error = response
        .body_mut()
        .read_json::<TokenError>()
        .map(|e| e.error)
        .unwrap_or_default();
    // `invalid_grant`: the user removed compter from their account, or the grant expired
    if error == "invalid_grant" {
        return Err(DriveError::SignedOut);
    }
    Err(DriveError::Status {
        status,
        message: error,
    })
}

/// Answer the browser and say what it brought
fn answer(stream: TcpStream, state: &str) -> Result<Redirect, DriveError> {
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    let mut line = String::new();
    BufReader::new(&stream).read_line(&mut line)?;
    let redirect = parse_redirect(&line, state);
    let (status, body) = match redirect {
        Redirect::Code(_) => ("200 OK", "Signed in to Google Drive. Return to compter."),
        Redirect::Refused(_) => ("200 OK", "Sign-in cancelled. Return to compter."),
        Redirect::Other => ("404 Not Found", ""),
    };
    let mut stream = stream;
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )?;
    Ok(redirect)
}

fn random() -> Result<[u8; 32], DriveError> {
    let mut bytes = [0; 32];
    getrandom::fill(&mut bytes)
        .map_err(|e| DriveError::Unexpected(format!("no randomness for sign-in: {e}")))?;
    Ok(bytes)
}
