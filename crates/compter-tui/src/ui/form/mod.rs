//! Modal input: a vertical list of fields, a yes/no confirmation, a checklist to pick rows from,
//! a list to pick one row from, and a folder picker

mod checklist;
mod confirm;
mod field;
mod folder_picker;
mod list_picker;
pub mod parse;

pub use checklist::{Checklist, ChecklistItem};
pub use confirm::Confirm;
pub use field::{Derive, Field, Preview};
pub use folder_picker::{FolderPicker, PickerEvent};
pub use list_picker::{ListEvent, ListItem, ListPicker};

use crate::ui::key::Key;
use ratatui::Frame;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::theme::{Theme, Tone};
use super::view::{Action, KeyHint, Notice};
use super::widgets::modal;

const LABEL_WIDTH: usize = 12;
const WIDTH: u16 = 64;

/// What a key press did to a form
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormEvent {
    Submit,
    Cancel,
    Edited,
    Ignored,
}

/// A titled list of fields; the owner reads values on `Submit` and may report an error back
#[derive(Debug, Clone)]
pub struct Form {
    title: &'static str,
    fields: Vec<Field>,
    focus: usize,
    error: Option<String>,
    /// The list a field is being filled in from (Ctrl-l), with that field's position; it
    /// takes every key while open
    picker: Option<(usize, ListPicker)>,
}

impl Form {
    pub const HINTS: &'static [KeyHint] = Self::LIST_HINTS.split_at(Self::LIST_HINTS.len() - 1).0;

    /// [`Form::HINTS`] and, last, Ctrl-l: while the focused field can be filled in from a list
    pub const LIST_HINTS: &'static [KeyHint] = &[
        KeyHint::new("Enter", "save"),
        KeyHint::new("Esc", "cancel"),
        KeyHint::new("Tab/↑↓", "field"),
        KeyHint::new("Space/←→", "choose"),
        KeyHint::new("Ctrl-l", "list"),
    ];

    pub fn new(title: &'static str, fields: Vec<Field>) -> Self {
        let mut form = Self {
            title,
            fields,
            focus: 0,
            error: None,
            picker: None,
        };
        form.rederive();
        form
    }

    /// The keys the form takes now: the list's while one is open, and Ctrl-l only while the
    /// focused field can open its list
    pub fn hints(&self) -> &'static [KeyHint] {
        if self.is_picking() {
            ListPicker::HINTS
        } else if self.can_pick() {
            Self::LIST_HINTS
        } else {
            Self::HINTS
        }
    }

    /// Ctrl-l would open the focused field's list: it has one, and isn't derived
    pub fn can_pick(&self) -> bool {
        self.fields.get(self.focus).is_some_and(Field::can_pick)
    }

    /// A field is being filled in from its list, which takes every key
    pub fn is_picking(&self) -> bool {
        self.picker.is_some()
    }

    /// The lines typed into lines field `index`, trimmed, blank ones left out
    pub fn lines(&self, index: usize) -> Vec<String> {
        self.fields
            .get(index)
            .map(Field::typed_lines)
            .unwrap_or_default()
    }

    /// Work every derived field out again from the fields it comes from
    fn rederive(&mut self) {
        for index in 0..self.fields.len() {
            let Some(field) = self.fields.get(index) else {
                continue;
            };
            let sources: Vec<String> = field
                .derived_sources(index)
                .into_iter()
                .filter_map(|source| self.fields.get(source))
                .map(|source| source.value().to_owned())
                .collect();
            if sources.is_empty() {
                continue;
            }
            let sources: Vec<&str> = sources.iter().map(String::as_str).collect();
            if let Some(field) = self.fields.get_mut(index) {
                field.rederive(&sources);
            }
        }
    }

    /// Move focus to `index`, telling the fields it leaves and enters
    fn focus_on(&mut self, index: usize, from_above: bool) {
        if let Some(field) = self.fields.get_mut(self.focus) {
            field.leave();
        }
        self.focus = index;
        if let Some(field) = self.fields.get_mut(index) {
            field.enter(from_above);
        }
    }

    /// Text of field `index` (the selected option for a choice); empty if out of range
    pub fn value(&self, index: usize) -> &str {
        self.fields.get(index).map_or("", Field::value)
    }

    pub fn choice(&self, index: usize) -> usize {
        self.fields.get(index).map_or(0, Field::selected)
    }

    pub fn len(&self) -> usize {
        self.fields.len()
    }

    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Fill text field `index` in, as a picker does; out of range does nothing
    pub fn set_value(&mut self, index: usize, text: impl Into<String>) {
        if let Some(field) = self.fields.get_mut(index) {
            field.set_value(text);
            self.error = None;
            self.rederive();
        }
    }

    /// Select option `choice` of choice field `index`; out of range does nothing
    pub fn set_choice(&mut self, index: usize, choice: usize) {
        if let Some(field) = self.fields.get_mut(index) {
            field.set_choice(choice);
            self.error = None;
            self.rederive();
        }
    }

    /// Show why the submit was rejected; the form keeps its input
    pub fn set_error(&mut self, error: impl Into<String>) {
        self.error = Some(error.into());
    }

    /// Feed a key to an open form. `submit` runs on Enter: `Ok` closes the form with a notice,
    /// `Err` keeps it open showing the message. Returns the form if it is still open.
    pub fn step(
        mut self,
        key: Key,
        submit: impl FnOnce(&Self) -> Result<Notice, String>,
    ) -> (Option<Self>, Action) {
        match self.handle_key(key) {
            FormEvent::Cancel => (None, Action::Handled),
            FormEvent::Submit => match submit(&self) {
                Ok(notice) => (None, Action::Notify(notice)),
                Err(error) => {
                    self.set_error(error);
                    (Some(self), Action::Handled)
                }
            },
            FormEvent::Edited | FormEvent::Ignored => (Some(self), Action::Handled),
        }
    }

    pub fn handle_key(&mut self, key: Key) -> FormEvent {
        if let Some((index, mut picker)) = self.picker.take() {
            match picker.handle_key(key) {
                ListEvent::Picked(row) => {
                    if let Some(field) = self.fields.get_mut(index) {
                        field.pick(row);
                    }
                    self.error = None;
                    self.rederive();
                }
                ListEvent::Cancel => {}
                ListEvent::Edited | ListEvent::Ignored => self.picker = Some((index, picker)),
            }
            return FormEvent::Edited;
        }
        let count = self.fields.len().max(1);
        let focus = self.focus;
        let input = |form: &mut Self| {
            form.fields
                .get_mut(focus)
                .is_some_and(|field| field.input(key))
        };
        match key {
            Key::Esc => FormEvent::Cancel,
            Key::Enter => FormEvent::Submit,
            Key::Ctrl('l') => match self.fields.get(focus).and_then(Field::picker) {
                Some(picker) => {
                    self.picker = Some((self.focus, picker));
                    FormEvent::Edited
                }
                None => FormEvent::Ignored,
            },
            // A lines field moves between its own lines first
            Key::Up | Key::Down if input(self) => FormEvent::Edited,
            Key::Tab | Key::Down => {
                self.focus_on((self.focus + 1) % count, true);
                FormEvent::Edited
            }
            Key::BackTab | Key::Up => {
                self.focus_on((self.focus + count - 1) % count, false);
                FormEvent::Edited
            }
            _ => {
                if input(self) {
                    self.error = None;
                    self.rederive();
                    FormEvent::Edited
                } else {
                    FormEvent::Ignored
                }
            }
        }
    }

    pub fn render(&self, frame: &mut Frame, theme: &Theme) {
        let rows: usize = self.fields.iter().map(Field::height).sum();
        let mut lines = Vec::with_capacity(rows + 3);
        lines.push(Line::default());
        for (index, field) in self.fields.iter().enumerate() {
            lines.extend(Self::field_lines(field, index == self.focus, theme));
        }
        lines.push(Line::default());
        match &self.error {
            Some(error) => {
                let width = usize::from(WIDTH).saturating_sub(6);
                for part in wrap_words(error, width) {
                    lines.push(Line::styled(format!("  {part}"), theme.tone(Tone::Error)));
                }
            }
            None => lines.push(Line::styled("  Enter save · Esc cancel", theme.dim())),
        }

        let height = u16::try_from(lines.len())
            .unwrap_or(u16::MAX)
            .saturating_add(2);
        let area = modal(frame, self.title, WIDTH, height, theme);
        frame.render_widget(Paragraph::new(lines), area);
        if let Some((_, picker)) = &self.picker {
            picker.render(frame, theme);
        }
    }

    /// A field's rows, its label on the first
    fn field_lines<'a>(
        field: &'a Field,
        focused: bool,
        theme: &Theme,
    ) -> impl Iterator<Item = Line<'a>> {
        let label_style = if focused { theme.key() } else { theme.dim() };
        field
            .rows(focused, theme)
            .into_iter()
            .enumerate()
            .map(move |(row, spans)| {
                let label = if row == 0 { field.label() } else { "" };
                let mut line = vec![Span::styled(
                    format!("  {label:<LABEL_WIDTH$}"),
                    label_style,
                )];
                line.extend(spans);
                Line::from(line)
            })
    }
}

/// Break `text` into lines of at most `width` characters, at spaces where possible
fn wrap_words(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let fits = line.chars().count() + 1 + word.chars().count() <= width;
        if !line.is_empty() && !fits {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}
