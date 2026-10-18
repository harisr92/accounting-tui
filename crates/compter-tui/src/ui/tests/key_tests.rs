use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::ui::key::Key;

fn press(code: KeyCode, modifiers: KeyModifiers) -> Option<Key> {
    Key::from_event(KeyEvent::new(code, modifiers))
}

#[test]
fn plain_and_control_characters_are_told_apart() {
    assert_eq!(
        press(KeyCode::Char('j'), KeyModifiers::NONE),
        Some(Key::Char('j'))
    );
    assert_eq!(
        press(KeyCode::Char('J'), KeyModifiers::SHIFT),
        Some(Key::Char('J'))
    );
    assert_eq!(
        press(KeyCode::Char('c'), KeyModifiers::CONTROL),
        Some(Key::Ctrl('c'))
    );
}

#[test]
fn enter_and_tabs_have_their_own_keys() {
    assert_eq!(press(KeyCode::Enter, KeyModifiers::NONE), Some(Key::Enter));
    assert_eq!(press(KeyCode::Tab, KeyModifiers::NONE), Some(Key::Tab));
    assert_eq!(
        press(KeyCode::BackTab, KeyModifiers::SHIFT),
        Some(Key::BackTab)
    );
    assert_eq!(press(KeyCode::Tab, KeyModifiers::SHIFT), Some(Key::BackTab));
}

#[test]
fn releases_are_ignored_so_windows_keys_act_once() {
    let release = KeyEvent::new_with_kind(
        KeyCode::Char('q'),
        KeyModifiers::NONE,
        KeyEventKind::Release,
    );
    assert_eq!(Key::from_event(release), None);
}

#[test]
fn unused_keys_are_dropped() {
    assert_eq!(press(KeyCode::F(1), KeyModifiers::NONE), None);
    assert_eq!(press(KeyCode::Home, KeyModifiers::NONE), None);
}
