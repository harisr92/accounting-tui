use crate::drive::folder::{FileList, multipart};

#[test]
fn files_are_created_in_the_hidden_app_folder() {
    let body = multipart("lease.json", b"{}");
    let text = String::from_utf8(body).unwrap();
    let metadata = text.lines().nth(3).unwrap();
    let metadata: serde_json::Value = serde_json::from_str(metadata).unwrap();
    assert_eq!(metadata["name"], "lease.json");
    assert_eq!(metadata["parents"][0], "appDataFolder");
    assert!(text.contains("application/octet-stream\r\n\r\n{}\r\n--"));
    assert!(text.ends_with("--\r\n"));
}

#[test]
fn an_empty_listing_finds_no_file() {
    let none: FileList = serde_json::from_str("{}").unwrap();
    assert!(none.files.is_empty());
    let one: FileList = serde_json::from_str(r#"{"files":[{"id":"abc"}]}"#).unwrap();
    assert_eq!(one.files[0].id, "abc");
}
