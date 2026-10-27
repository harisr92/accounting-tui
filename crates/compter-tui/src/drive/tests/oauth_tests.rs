use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

use crate::drive::oauth::{
    Pkce, Redirect, SCOPES, consent_url, email_from_id_token, parse_redirect,
};

#[test]
fn the_challenge_is_the_hash_of_the_verifier_as_rfc_7636_shows() {
    // RFC 7636, appendix B
    let random = [
        116, 24, 223, 180, 151, 153, 224, 37, 79, 250, 96, 125, 216, 173, 187, 186, 22, 212, 37,
        77, 105, 214, 191, 240, 91, 88, 5, 88, 83, 132, 141, 121,
    ];
    let pkce = Pkce::from_random(&random);
    assert_eq!(pkce.verifier, "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk");
    assert_eq!(
        pkce.challenge,
        "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
    );
}

#[test]
fn consent_asks_only_for_the_app_folder_and_a_refresh_token() {
    let url = consent_url(
        "client.apps",
        "http://127.0.0.1:4711",
        "challenge",
        "state&1",
    );
    let (base, query) = url.split_once('?').unwrap();
    assert_eq!(base, "https://accounts.google.com/o/oauth2/v2/auth");
    let pairs: Vec<(String, String)> = form_urlencoded::parse(query.as_bytes())
        .into_owned()
        .collect();
    let get = |key: &str| {
        pairs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    };
    assert_eq!(get("scope"), Some(SCOPES));
    assert!(
        !SCOPES.contains("auth/drive "),
        "never the full Drive scope"
    );
    assert_eq!(get("redirect_uri"), Some("http://127.0.0.1:4711"));
    assert_eq!(get("code_challenge_method"), Some("S256"));
    assert_eq!(get("state"), Some("state&1"));
    assert_eq!(get("access_type"), Some("offline"));
}

#[test]
fn the_redirect_gives_the_code_only_for_this_sign_in() {
    let line = "GET /?state=abc&code=4%2F0Ab-xyz&scope=email HTTP/1.1\r\n";
    assert_eq!(
        parse_redirect(line, "abc"),
        Redirect::Code("4/0Ab-xyz".to_owned())
    );
    assert_eq!(parse_redirect(line, "other"), Redirect::Other);
    assert_eq!(
        parse_redirect("GET /?error=access_denied&state=abc HTTP/1.1", "abc"),
        Redirect::Refused("access_denied".to_owned())
    );
    assert_eq!(
        parse_redirect("GET /favicon.ico HTTP/1.1", "abc"),
        Redirect::Other
    );
    assert_eq!(parse_redirect("", "abc"), Redirect::Other);
}

#[test]
fn the_account_comes_from_the_id_token() {
    let claims = URL_SAFE_NO_PAD.encode(br#"{"email":"me@example.com","sub":"1"}"#);
    let token = format!("header.{claims}.signature");
    assert_eq!(email_from_id_token(&token).unwrap(), "me@example.com");
    assert!(email_from_id_token("not a token").is_err());
}
