//! The keys compter reacts to, so views don't depend on a terminal library

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

/// A key press, already reduced to what the views distinguish
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    Char(char),
    Ctrl(char),
    Enter,
    Tab,
    BackTab,
    Backspace,
    Esc,
    Up,
    Down,
    Left,
    Right,
}

impl Key {
    /// `None` for key releases (Windows reports them) and for keys compter doesn't use
    pub fn from_event(event: KeyEvent) -> Option<Self> {
        if event.kind == KeyEventKind::Release {
            return None;
        }
        let key = match event.code {
            KeyCode::Char(c) if event.modifiers.contains(KeyModifiers::CONTROL) => {
                Self::Ctrl(c.to_ascii_lowercase())
            }
            KeyCode::Char(c) => Self::Char(c),
            KeyCode::Enter => Self::Enter,
            KeyCode::Tab if event.modifiers.contains(KeyModifiers::SHIFT) => Self::BackTab,
            KeyCode::Tab => Self::Tab,
            KeyCode::BackTab => Self::BackTab,
            KeyCode::Backspace => Self::Backspace,
            KeyCode::Esc => Self::Esc,
            KeyCode::Up => Self::Up,
            KeyCode::Down => Self::Down,
            KeyCode::Left => Self::Left,
            KeyCode::Right => Self::Right,
            _ => return None,
        };
        Some(key)
    }
}
