use std::ffi::OsString;
use std::path::Path;

use crate::storage::{DataDir, StorageError};

fn env<'a>(vars: &'a [(&str, &str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
    move |key| {
        vars.iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| OsString::from(v))
    }
}

#[test]
fn compter_home_wins() {
    let vars = [
        ("COMPTER_HOME", "/data"),
        ("XDG_DATA_HOME", "/xdg"),
        ("HOME", "/home/me"),
    ];
    let dir = DataDir::resolve(env(&vars)).unwrap();
    assert_eq!(dir.root(), Path::new("/data"));
    assert_eq!(dir.database_path(), Path::new("/data/compter.redb"));
}

#[test]
fn falls_back_to_xdg_then_home() {
    let xdg = [("XDG_DATA_HOME", "/xdg"), ("HOME", "/home/me")];
    assert_eq!(
        DataDir::resolve(env(&xdg)).unwrap().root(),
        Path::new("/xdg/compter")
    );

    let home = [("COMPTER_HOME", ""), ("HOME", "/home/me")];
    assert_eq!(
        DataDir::resolve(env(&home)).unwrap().root(),
        Path::new("/home/me/.local/share/compter")
    );
}

#[test]
fn no_home_is_an_error() {
    assert!(matches!(
        DataDir::resolve(env(&[])),
        Err(StorageError::NoDataDir)
    ));
}
