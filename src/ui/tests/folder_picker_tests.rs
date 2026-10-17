use std::fs;
use std::path::Path;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use tempfile::TempDir;

use crate::ui::form::{FolderPicker, PickerEvent};
use crate::ui::key::Key;
use crate::ui::theme::Theme;

/// `root/{alpha,Beta,.hidden,gamma}` plus a file, which is never listed
fn tree() -> TempDir {
    let root = tempfile::tempdir().unwrap();
    for name in ["alpha", "Beta", ".hidden", "gamma/inner"] {
        fs::create_dir_all(root.path().join(name)).unwrap();
    }
    fs::write(root.path().join("notes.txt"), "x").unwrap();
    root
}

fn canonical(path: &Path) -> std::path::PathBuf {
    fs::canonicalize(path).unwrap()
}

fn shown(picker: &FolderPicker) -> String {
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal
        .draw(|frame| picker.render(frame, &Theme::default()))
        .unwrap();
    let buffer = terminal.backend().buffer();
    buffer
        .content()
        .chunks(80)
        .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn lists_only_visible_folders_sorted_by_name() {
    let root = tree();
    let mut picker = FolderPicker::open("Pick", Some(root.path()));
    let screen = shown(&picker);
    let alpha = screen.find("alpha/").unwrap();
    let beta = screen.find("Beta/").unwrap();
    let gamma = screen.find("gamma/").unwrap();
    assert!(alpha < beta && beta < gamma, "{screen}");
    assert!(
        !screen.contains(".hidden") && !screen.contains("notes.txt"),
        "{screen}"
    );

    picker.handle_key(Key::Char('.'));
    assert!(shown(&picker).contains(".hidden/"));
}

#[test]
fn opens_folders_goes_up_and_chooses_the_one_shown() {
    let root = tree();
    let mut picker = FolderPicker::open("Pick", Some(root.path()));
    picker.handle_key(Key::Char('j'));
    picker.handle_key(Key::Char('j'));
    picker.handle_key(Key::Enter);
    assert_eq!(picker.dir(), canonical(&root.path().join("gamma")));
    picker.handle_key(Key::Enter);
    assert_eq!(picker.dir(), canonical(&root.path().join("gamma/inner")));

    picker.handle_key(Key::Backspace);
    picker.handle_key(Key::Backspace);
    assert_eq!(picker.dir(), canonical(root.path()));
    // The folder just left stays under the cursor
    assert_eq!(picker.handle_key(Key::Enter), PickerEvent::Moved);
    assert_eq!(picker.dir(), canonical(&root.path().join("gamma")));

    assert_eq!(
        picker.handle_key(Key::Char(' ')),
        PickerEvent::Chosen(canonical(&root.path().join("gamma")))
    );
    assert_eq!(picker.handle_key(Key::Esc), PickerEvent::Cancel);
}

#[test]
fn starts_at_the_nearest_folder_that_exists() {
    let root = tree();
    let missing = root.path().join("alpha/gone/deeper");
    let picker = FolderPicker::open("Pick", Some(&missing));
    assert_eq!(picker.dir(), canonical(&root.path().join("alpha")));
    assert!(shown(&picker).contains("no folders here"));
}

#[test]
fn a_rejected_choice_keeps_the_picker_open_with_the_reason() {
    let root = tree();
    let picker = FolderPicker::open("Pick", Some(root.path()));
    let (picker, _) = picker.step(Key::Char(' '), |_| Err("not that one".to_owned()));
    let picker = picker.unwrap();
    assert_eq!(picker.error(), Some("not that one"));
    assert!(shown(&picker).contains("not that one"));
}
