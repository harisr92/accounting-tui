//! Modal input: a vertical list of fields, and a yes/no confirmation

mod confirm;
mod field;
pub mod parse;

pub use confirm::Confirm;
pub use field::{Field, Preview};

use ratatui::Frame;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use termion::event::Key;

use super::theme::{Theme, Tone};
use super::view::KeyHint;
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
}

impl Form {
    pub const HINTS: &'static [KeyHint] = &[
        KeyHint::new("Enter", "save"),
        KeyHint::new("Esc", "cancel"),
        KeyHint::new("Tab/↑↓", "field"),
        KeyHint::new("Space/←→", "choose"),
    ];

    pub fn new(title: &'static str, fields: Vec<Field>) -> Self {
        Self {
            title,
            fields,
            focus: 0,
            error: None,
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

    /// Show why the submit was rejected; the form keeps its input
    pub fn set_error(&mut self, error: impl Into<String>) {
        self.error = Some(error.into());
    }

    pub fn handle_key(&mut self, key: Key) -> FormEvent {
        let count = self.fields.len().max(1);
        match key {
            Key::Esc => FormEvent::Cancel,
            Key::Char('\n') => FormEvent::Submit,
            Key::Char('\t') | Key::Down => {
                self.focus = (self.focus + 1) % count;
                FormEvent::Edited
            }
            Key::BackTab | Key::Up => {
                self.focus = (self.focus + count - 1) % count;
                FormEvent::Edited
            }
            _ => {
                let edited = self
                    .fields
                    .get_mut(self.focus)
                    .is_some_and(|field| field.input(key));
                if edited {
                    self.error = None;
                    FormEvent::Edited
                } else {
                    FormEvent::Ignored
                }
            }
        }
    }

    pub fn render(&self, frame: &mut Frame, theme: &Theme) {
        let mut lines = Vec::with_capacity(self.fields.len() + 3);
        lines.push(Line::default());
        for (index, field) in self.fields.iter().enumerate() {
            lines.push(self.field_line(field, index == self.focus, theme));
        }
        lines.push(Line::default());
        lines.push(match &self.error {
            Some(error) => Line::styled(format!("  {error}"), theme.tone(Tone::Error)),
            None => Line::styled("  Enter save · Esc cancel", theme.dim()),
        });

        let height = u16::try_from(lines.len())
            .unwrap_or(u16::MAX)
            .saturating_add(2);
        let area = modal(frame, self.title, WIDTH, height, theme);
        frame.render_widget(Paragraph::new(lines), area);
    }

    fn field_line<'a>(&self, field: &'a Field, focused: bool, theme: &Theme) -> Line<'a> {
        let label_style = if focused { theme.key() } else { theme.dim() };
        let mut spans = vec![Span::styled(
            format!("  {:<LABEL_WIDTH$}", field.label()),
            label_style,
        )];
        spans.extend(field.spans(focused, theme));
        Line::from(spans)
    }
}
