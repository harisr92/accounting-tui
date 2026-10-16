use crate::ui::key::Key;
use ratatui::widgets::TableState;

/// Direction of a list movement key
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nav {
    Next,
    Prev,
}

impl Nav {
    pub fn from_key(key: Key) -> Option<Self> {
        match key {
            Key::Char('j') | Key::Down => Some(Self::Next),
            Key::Char('k') | Key::Up => Some(Self::Prev),
            _ => None,
        }
    }
}

/// Wrap-around cursor over `len` rows, shared by every table
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Selection(usize);

impl Selection {
    pub const fn index(self) -> usize {
        self.0
    }

    pub fn reset(&mut self) {
        self.0 = 0;
    }

    pub fn select(&mut self, index: usize) {
        self.0 = index;
    }

    /// Keep the cursor inside a list that may have shrunk
    pub fn clamp(&mut self, len: usize) {
        self.0 = self.0.min(len.saturating_sub(1));
    }

    pub fn move_by(&mut self, nav: Nav, len: usize) {
        if len == 0 {
            self.0 = 0;
            return;
        }
        self.0 = match nav {
            Nav::Next => (self.0 + 1) % len,
            Nav::Prev => (self.0 + len - 1) % len,
        };
    }

    fn selected(self, len: usize) -> Option<usize> {
        (len > 0).then(|| self.0.min(len - 1))
    }

    pub fn table_state(self, len: usize) -> TableState {
        TableState::default().with_selected(self.selected(len))
    }
}
