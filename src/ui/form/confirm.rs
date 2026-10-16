use crate::ui::key::Key;
use ratatui::Frame;
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Wrap};

use crate::ui::theme::Theme;
use crate::ui::view::KeyHint;
use crate::ui::widgets::modal;

const WIDTH: u16 = 56;

/// A yes/no question shown before an irreversible change
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Confirm {
    message: String,
}

impl Confirm {
    pub const HINTS: &'static [KeyHint] = &[KeyHint::new("y", "yes"), KeyHint::new("n/Esc", "no")];

    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// `Some(true)` to proceed, `Some(false)` to back out, `None` for any other key
    pub fn answer(key: Key) -> Option<bool> {
        match key {
            Key::Char('y' | 'Y') => Some(true),
            Key::Char('n' | 'N') | Key::Esc => Some(false),
            _ => None,
        }
    }

    pub fn render(&self, frame: &mut Frame, theme: &Theme) {
        let lines = vec![
            Line::default(),
            Line::raw(format!(" {}", self.message)),
            Line::default(),
            Line::styled(" y yes · n no", theme.dim()),
        ];
        let area = modal(frame, "Confirm", WIDTH, 8, theme);
        frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
    }
}
