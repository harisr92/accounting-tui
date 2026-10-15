use ratatui::Frame;
use ratatui::layout::Rect;
use termion::event::Key;

use super::theme::Theme;

/// What a view did with a key press
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Not handled; the app may apply its global bindings
    Ignored,
    /// Handled; state changed and needs a redraw
    Handled,
    /// Handled; show a transient message in the footer
    Notify(&'static str),
}

/// A key binding shown in the footer and the help popup
#[derive(Debug, Clone, Copy)]
pub struct KeyHint {
    pub key: &'static str,
    pub desc: &'static str,
}

impl KeyHint {
    pub const fn new(key: &'static str, desc: &'static str) -> Self {
        Self { key, desc }
    }
}

/// One tab of the application. New screens implement this and get registered in `main`.
pub trait View {
    fn title(&self) -> &'static str;

    fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme);

    /// Views see keys before global bindings, so forms can capture text input
    fn handle_key(&mut self, _key: Key) -> Action {
        Action::Ignored
    }

    fn key_hints(&self) -> &'static [KeyHint] {
        &[]
    }
}
