use crate::ui::form::parse::path;

#[test]
fn paths_drop_drag_and_drop_quotes_and_expand_home() {
    assert_eq!(path("  "), None);
    assert_eq!(path("''"), None);
    assert_eq!(
        path(" '/tmp/my logo.png' ").unwrap(),
        std::path::PathBuf::from("/tmp/my logo.png")
    );
    assert_eq!(
        path("\"/tmp/logo.png\"").unwrap(),
        std::path::PathBuf::from("/tmp/logo.png")
    );
    if let Some(home) = std::env::var_os("HOME") {
        assert_eq!(
            path("~/logo.png").unwrap(),
            std::path::PathBuf::from(home).join("logo.png")
        );
    }
    assert_eq!(
        path("~user/logo.png").unwrap(),
        std::path::PathBuf::from("~user/logo.png")
    );
}
