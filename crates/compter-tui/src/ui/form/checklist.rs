use crate::ui::key::Key;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Row, Table, Wrap};

use super::FormEvent;
use crate::ui::selection::{Nav, Selection};
use crate::ui::theme::{Theme, Tone};
use crate::ui::view::{Action, KeyHint, Notice};
use crate::ui::widgets::{header, modal};

/// Room for the status line: the count picked and every key but movement
const WIDTH: u16 = 78;
/// Rows shown before the list scrolls
const MAX_ROWS: u16 = 12;
/// Longest amount that can be typed
const MAX_AMOUNT: usize = 16;
const DETAIL_WIDTH: u16 = 16;
const AMOUNT_WIDTH: u16 = MAX_AMOUNT as u16 + 1;

/// One row of a [`Checklist`]: what it is, a figure beside it, and optionally an amount to edit
#[derive(Debug, Clone)]
pub struct ChecklistItem {
    label: String,
    detail: String,
    amount: Option<String>,
    /// Put in an empty amount when the row is picked
    fill: Option<String>,
    /// The amount was put in for the user (prefilled or filled on picking): the first key typed
    /// replaces it rather than adding to it, so `4000` typed over `5000.00` is 4000
    fresh: bool,
    checked: bool,
    enabled: bool,
}

impl ChecklistItem {
    pub fn new(label: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            detail: detail.into(),
            amount: None,
            fill: None,
            fresh: false,
            checked: false,
            enabled: true,
        }
    }

    /// An amount the user may change, typed straight into the row
    pub fn with_amount(mut self, amount: impl Into<String>) -> Self {
        let amount = amount.into();
        self.fresh = !amount.is_empty();
        self.amount = Some(amount);
        self
    }

    /// What an empty amount becomes when the row is picked (Space or `a`), e.g. all that is left
    pub fn with_fill(mut self, fill: impl Into<String>) -> Self {
        self.fill = Some(fill.into());
        self
    }

    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    /// Pick or unpick the row; picking fills an empty amount
    fn set_checked(&mut self, checked: bool) {
        self.checked = checked;
        if let (true, Some(amount), Some(fill)) = (checked, self.amount.as_mut(), &self.fill)
            && amount.is_empty()
        {
            amount.clone_from(fill);
            self.fresh = true;
        }
    }

    /// Shown but can't be picked (nothing is left on it)
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self.checked = false;
        self
    }
}

/// Pick any number of rows, with a key to pick them all; rows with an amount take it typed in.
/// Run with [`Checklist::step`], like a form.
#[derive(Debug, Clone)]
pub struct Checklist {
    title: &'static str,
    /// Titles of the label, detail and amount columns
    columns: [&'static str; 3],
    items: Vec<ChecklistItem>,
    /// Shown above the rows, to guide the choice
    note: Option<String>,
    selection: Selection,
    error: Option<String>,
}

impl Checklist {
    pub const HINTS: &'static [KeyHint] = &[
        KeyHint::new("Enter", "next"),
        KeyHint::new("Esc", "cancel"),
        KeyHint::new("j/k", "move"),
        KeyHint::new("Space", "pick"),
        KeyHint::new("a", "all/none"),
    ];

    /// [`Self::HINTS`] for a list whose rows take an amount
    pub const AMOUNT_HINTS: &'static [KeyHint] = &[
        KeyHint::new("Enter", "next"),
        KeyHint::new("Esc", "cancel"),
        KeyHint::new("j/k", "move"),
        KeyHint::new("Space", "pick"),
        KeyHint::new("a", "all/none"),
        KeyHint::new("0-9", "amount"),
    ];

    /// `columns` title the label, detail and amount columns; the cursor starts on the first
    /// checked row, or else the first that can be picked, so typing an amount reaches a row
    pub fn new(title: &'static str, columns: [&'static str; 3], items: Vec<ChecklistItem>) -> Self {
        let mut selection = Selection::default();
        if let Some(first) = items
            .iter()
            .position(|item| item.checked)
            .or_else(|| items.iter().position(|item| item.enabled))
        {
            selection.select(first);
        }
        Self {
            title,
            columns,
            items,
            note: None,
            selection,
            error: None,
        }
    }

    /// A line shown above the rows, e.g. what is due
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }

    /// The keys this list takes, for the footer and help: amount keys only when rows have one
    pub fn hints(&self) -> &'static [KeyHint] {
        if self.has_amounts() {
            Self::AMOUNT_HINTS
        } else {
            Self::HINTS
        }
    }

    fn has_amounts(&self) -> bool {
        self.items.iter().any(|item| item.amount.is_some())
    }

    /// Positions of the rows picked, in order
    pub fn checked(&self) -> Vec<usize> {
        self.items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.checked)
            .map(|(index, _)| index)
            .collect()
    }

    /// The amount typed on row `index`; empty if it has none
    pub fn amount(&self, index: usize) -> &str {
        self.items
            .get(index)
            .and_then(|item| item.amount.as_deref())
            .unwrap_or("")
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Show why the submit was rejected; the list keeps what was picked
    pub fn set_error(&mut self, error: impl Into<String>) {
        self.error = Some(error.into());
    }

    /// Feed a key to an open checklist, like [`super::Form::step`]: `submit` runs on Enter, `Ok`
    /// closes it with a notice and `Err` keeps it open showing the message
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
        if let Some(nav) = Nav::from_key(key) {
            self.selection.move_by(nav, self.items.len());
            return FormEvent::Edited;
        }
        let edited = match key {
            Key::Esc => return FormEvent::Cancel,
            Key::Enter => return FormEvent::Submit,
            Key::Char(' ') => self.current().is_some_and(|item| {
                item.set_checked(!item.checked);
                true
            }),
            Key::Char('a') => {
                let all = self
                    .items
                    .iter()
                    .filter(|item| item.enabled)
                    .all(|item| item.checked);
                for item in self.items.iter_mut().filter(|item| item.enabled) {
                    item.set_checked(!all);
                }
                self.items.iter().any(|item| item.enabled)
            }
            Key::Char(c) if c.is_ascii_digit() || c == '.' || c == ',' => self.type_amount(c),
            Key::Backspace => self.edit_amount(|amount| {
                amount.pop();
            }),
            Key::Ctrl('u') => self.edit_amount(String::clear),
            _ => false,
        };
        if edited {
            self.error = None;
            FormEvent::Edited
        } else {
            FormEvent::Ignored
        }
    }

    /// The row under the cursor, if it can be picked
    fn current(&mut self) -> Option<&mut ChecklistItem> {
        self.items
            .get_mut(self.selection.index())
            .filter(|item| item.enabled)
    }

    /// Type `c` into the amount on the row under the cursor, which picks it. An amount put in
    /// for the user is replaced, not added to.
    fn type_amount(&mut self, c: char) -> bool {
        let Some(item) = self.current() else {
            return false;
        };
        let Some(amount) = item.amount.as_mut() else {
            return false;
        };
        if item.fresh {
            amount.clear();
        }
        if amount.chars().count() < MAX_AMOUNT {
            amount.push(c);
        }
        item.fresh = false;
        item.checked = true;
        true
    }

    /// Change the amount on the row under the cursor; it stays picked or not, so clearing a
    /// row's amount never picks it
    fn edit_amount(&mut self, edit: impl FnOnce(&mut String)) -> bool {
        let Some(item) = self.current() else {
            return false;
        };
        let Some(amount) = item.amount.as_mut() else {
            return false;
        };
        edit(amount);
        item.fresh = false;
        true
    }

    /// ` 2 picked · Enter next · Esc cancel · Space pick · …`: the keys from [`Self::hints`], but
    /// for moving, which the footer shows
    fn status(&self) -> String {
        let picked = self.items.iter().filter(|item| item.checked).count();
        let keys = self
            .hints()
            .iter()
            .filter(|hint| hint.key != "j/k")
            .map(|hint| format!("{} {}", hint.key, hint.desc));
        std::iter::once(format!(" {picked} picked"))
            .chain(keys)
            .collect::<Vec<_>>()
            .join(" · ")
    }

    pub fn render(&self, frame: &mut Frame, theme: &Theme) {
        let rows = u16::try_from(self.items.len())
            .unwrap_or(u16::MAX)
            .clamp(1, MAX_ROWS);
        // Up to two lines of note, then a blank
        let note_height: u16 = if self.note.is_some() { 3 } else { 0 };
        // Note, header, blank, rows, blank, status
        let height = rows.saturating_add(6).saturating_add(note_height);
        let area = modal(frame, self.title, WIDTH, height, theme);
        let [note_area, list_area, status_area] = Layout::vertical([
            Constraint::Length(note_height),
            Constraint::Min(1),
            Constraint::Length(2),
        ])
        .areas(area);
        if let Some(note) = &self.note {
            frame.render_widget(
                Paragraph::new(Line::styled(format!(" {note}"), theme.dim()))
                    .wrap(Wrap { trim: false }),
                note_area,
            );
        }

        let has_amounts = self.has_amounts();
        let table_rows = self.items.iter().enumerate().map(|(index, item)| {
            let mark = if item.checked { "[x]" } else { "[ ]" };
            let amount = item.amount.as_deref().map_or_else(Line::default, |amount| {
                let editing = index == self.selection.index() && item.enabled;
                let text = if editing {
                    format!("{amount}▏")
                } else {
                    amount.to_owned()
                };
                Line::raw(text).right_aligned()
            });
            let row = Row::new(vec![
                Line::raw(mark),
                Line::raw(item.label.as_str()),
                Line::raw(item.detail.as_str()).right_aligned(),
                amount,
            ]);
            if item.enabled {
                row
            } else {
                row.style(theme.dim())
            }
        });
        let widths = [
            Constraint::Length(3),
            Constraint::Fill(1),
            // `₹1,00,00,000.00`
            Constraint::Length(DETAIL_WIDTH),
            // The longest amount plus the cursor
            Constraint::Length(if has_amounts { AMOUNT_WIDTH } else { 0 }),
        ];
        let [label, detail, amount] = self.columns;
        let table = Table::new(table_rows, widths)
            .header(header(&["", label, detail, amount], &[2, 3], theme))
            .column_spacing(1)
            .row_highlight_style(theme.selected)
            .highlight_symbol(" > ");
        let mut state = self.selection.table_state(self.items.len());
        frame.render_stateful_widget(table, list_area, &mut state);

        let status = match &self.error {
            Some(error) => Line::styled(format!(" {error}"), theme.tone(Tone::Error)),
            None => Line::styled(self.status(), theme.dim()),
        };
        frame.render_widget(Paragraph::new(vec![Line::default(), status]), status_area);
    }
}
