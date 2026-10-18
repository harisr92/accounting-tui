use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use crate::ui::key::Key;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Row, Table, Wrap};

use super::parse;
use crate::ui::selection::{Nav, Selection};
use crate::ui::theme::{Theme, Tone};
use crate::ui::view::{Action, KeyHint, Notice};
use crate::ui::widgets::modal;

const WIDTH: u16 = 72;
const HEIGHT: u16 = 22;

/// What a key press did to a folder picker
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickerEvent {
    Chosen(PathBuf),
    Cancel,
    Moved,
    Ignored,
}

/// Browse the file system one folder at a time and choose one, instead of typing its path
#[derive(Debug, Clone)]
pub struct FolderPicker {
    title: &'static str,
    dir: PathBuf,
    /// Subfolders of `dir`, sorted by name
    entries: Vec<OsString>,
    selection: Selection,
    show_hidden: bool,
    error: Option<String>,
}

impl FolderPicker {
    pub const HINTS: &'static [KeyHint] = &[
        KeyHint::new("Space", "choose this folder"),
        KeyHint::new("Enter/→", "open"),
        KeyHint::new("Bksp/←", "up"),
        KeyHint::new("~", "home"),
        KeyHint::new(".", "hidden folders"),
        KeyHint::new("Esc", "cancel"),
    ];

    /// Start in `start`, or the nearest folder above it that exists, else the home folder
    pub fn open(title: &'static str, start: Option<&Path>) -> Self {
        let dir = start
            .and_then(|start| start.ancestors().find(|dir| dir.is_dir()))
            .map(Path::to_path_buf)
            .or_else(parse::home_dir)
            .unwrap_or_else(|| PathBuf::from("."));
        let mut picker = Self {
            title,
            dir: canonical(dir),
            entries: Vec::new(),
            selection: Selection::default(),
            show_hidden: false,
            error: None,
        };
        picker.reload();
        picker
    }

    /// The folder being shown, which Space chooses
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Show why the choice was rejected; the picker stays where it is
    pub fn set_error(&mut self, error: impl Into<String>) {
        self.error = Some(error.into());
    }

    /// Feed a key to an open picker, like [`super::Form::step`]: `submit` runs on Space with the
    /// folder shown, `Ok` closes the picker with a notice and `Err` keeps it open with the message
    pub fn step(
        mut self,
        key: Key,
        submit: impl FnOnce(&Path) -> Result<Notice, String>,
    ) -> (Option<Self>, Action) {
        match self.handle_key(key) {
            PickerEvent::Cancel => (None, Action::Handled),
            PickerEvent::Chosen(dir) => match submit(&dir) {
                Ok(notice) => (None, Action::Notify(notice)),
                Err(error) => {
                    self.set_error(error);
                    (Some(self), Action::Handled)
                }
            },
            PickerEvent::Moved | PickerEvent::Ignored => (Some(self), Action::Handled),
        }
    }

    pub fn handle_key(&mut self, key: Key) -> PickerEvent {
        if let Some(nav) = Nav::from_key(key) {
            self.selection.move_by(nav, self.entries.len());
            return PickerEvent::Moved;
        }
        match key {
            Key::Esc => PickerEvent::Cancel,
            Key::Char(' ') => PickerEvent::Chosen(self.dir.clone()),
            Key::Enter | Key::Right | Key::Char('l') => {
                let Some(name) = self.entries.get(self.selection.index()) else {
                    return PickerEvent::Ignored;
                };
                let child = self.dir.join(name);
                self.go_to(child, None);
                PickerEvent::Moved
            }
            Key::Backspace | Key::Left | Key::Char('h') => {
                let Some(parent) = self.dir.parent().map(Path::to_path_buf) else {
                    return PickerEvent::Ignored;
                };
                let from = self.dir.file_name().map(OsString::from);
                self.go_to(parent, from);
                PickerEvent::Moved
            }
            Key::Char('~') => match parse::home_dir() {
                Some(home) => {
                    self.go_to(home, None);
                    PickerEvent::Moved
                }
                None => PickerEvent::Ignored,
            },
            Key::Char('.') => {
                self.show_hidden = !self.show_hidden;
                let selected = self.entries.get(self.selection.index()).cloned();
                self.reload();
                self.select(selected.as_deref());
                PickerEvent::Moved
            }
            _ => PickerEvent::Ignored,
        }
    }

    /// Show `dir`, with `select` (the folder just left) under the cursor; a folder that can't be
    /// read leaves the picker where it was, with the error
    fn go_to(&mut self, dir: PathBuf, select: Option<OsString>) {
        match list_folders(&dir, self.show_hidden) {
            Ok(entries) => {
                self.dir = dir;
                self.entries = entries;
                self.error = None;
                self.selection.reset();
                self.select(select.as_deref());
            }
            Err(error) => self.error = Some(format!("can't open {}: {error}", dir.display())),
        }
    }

    fn reload(&mut self) {
        match list_folders(&self.dir, self.show_hidden) {
            Ok(entries) => self.entries = entries,
            Err(error) => {
                self.entries.clear();
                self.error = Some(format!("can't open {}: {error}", self.dir.display()));
            }
        }
        self.selection.clamp(self.entries.len());
    }

    fn select(&mut self, name: Option<&std::ffi::OsStr>) {
        if let Some(index) = name.and_then(|name| self.entries.iter().position(|e| e == name)) {
            self.selection.select(index);
        }
    }

    pub fn render(&self, frame: &mut Frame, theme: &Theme) {
        let area = modal(frame, self.title, WIDTH, HEIGHT, theme);
        let [path_area, list_area, status_area] = Layout::vertical([
            Constraint::Length(2),
            Constraint::Min(1),
            Constraint::Length(4),
        ])
        .areas(area);

        let shown = self.dir.display().to_string();
        let width = usize::from(path_area.width).saturating_sub(2);
        frame.render_widget(
            Paragraph::new(Line::styled(
                format!(" {}", tail(&shown, width)),
                theme.key(),
            )),
            path_area,
        );

        if self.entries.is_empty() {
            frame.render_widget(
                Paragraph::new(Line::styled("   no folders here", theme.dim())),
                list_area,
            );
        } else {
            let rows = self.entries.iter().map(|name| {
                let mut label = name.to_string_lossy().into_owned();
                label.push('/');
                Row::new([label])
            });
            let table = Table::new(rows, [Constraint::Fill(1)])
                .row_highlight_style(theme.selected)
                .highlight_symbol(" > ");
            let mut state = self.selection.table_state(self.entries.len());
            frame.render_stateful_widget(table, list_area, &mut state);
        }

        let status = match &self.error {
            Some(error) => Line::styled(format!(" {error}"), theme.tone(Tone::Error)),
            None => Line::styled(
                " Space choose this folder · Enter open · Bksp up · Esc cancel",
                theme.dim(),
            ),
        };
        // An error names a path, so it may need more than one line
        let status = Paragraph::new(vec![Line::default(), status]).wrap(Wrap { trim: false });
        frame.render_widget(status, status_area);
    }
}

/// `dir` made absolute with links resolved. Windows answers with `\\?\C:\…`, which users don't
/// expect to see or have saved, so a plain drive path drops that prefix.
fn canonical(dir: PathBuf) -> PathBuf {
    let Ok(full) = fs::canonicalize(&dir) else {
        return dir;
    };
    match full.to_str().and_then(|path| path.strip_prefix(r"\\?\")) {
        Some(plain) if plain.as_bytes().get(1) == Some(&b':') => PathBuf::from(plain),
        _ => full,
    }
}

/// The subfolders of `dir`, sorted case-insensitively; dot-folders only with `hidden`
fn list_folders(dir: &Path, hidden: bool) -> std::io::Result<Vec<OsString>> {
    let mut names: Vec<OsString> = fs::read_dir(dir)?
        .filter_map(Result::ok)
        // Follows symlinks, so a link to a folder can be opened too
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name())
        .filter(|name| hidden || !name.to_string_lossy().starts_with('.'))
        .collect();
    names.sort_by_cached_key(|name| name.to_string_lossy().to_lowercase());
    Ok(names)
}

/// The last `width` characters of `text`, marked with `…` when cut, so a long path keeps the
/// folder it ends in
fn tail(text: &str, width: usize) -> String {
    let count = text.chars().count();
    if count <= width || width == 0 {
        return text.to_owned();
    }
    let keep: String = text.chars().skip(count - width + 1).collect();
    format!("…{keep}")
}
