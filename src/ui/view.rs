use std::borrow::Cow;

use crate::ui::key::Key;
use ratatui::Frame;
use ratatui::layout::Rect;

use super::theme::{Theme, Tone};
use crate::domain::Appearance;

/// A transient footer message
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub text: Cow<'static, str>,
    pub tone: Tone,
}

impl Notice {
    pub fn info(text: impl Into<Cow<'static, str>>) -> Self {
        Self::new(text, Tone::Warning)
    }

    pub fn success(text: impl Into<Cow<'static, str>>) -> Self {
        Self::new(text, Tone::Success)
    }

    pub fn error(text: impl Into<Cow<'static, str>>) -> Self {
        Self::new(text, Tone::Error)
    }

    fn new(text: impl Into<Cow<'static, str>>, tone: Tone) -> Self {
        Self {
            text: text.into(),
            tone,
        }
    }
}

/// What a view did with a key press
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Not handled; the app may apply its global bindings
    Ignored,
    /// Handled; state changed and needs a redraw
    Handled,
    /// Handled; show a transient message in the footer
    Notify(Notice),
    /// Handled; draw the whole app in this theme from now on
    Theme(Appearance),
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

    /// Called when the tab becomes active, so it can re-read what other tabs changed
    fn on_activate(&mut self) -> Option<Notice> {
        None
    }
}
