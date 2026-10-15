use ratatui::widgets::TableState;
use termion::event::Key;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_both_ways() {
        let mut sel = Selection::default();
        sel.move_by(Nav::Prev, 3);
        assert_eq!(sel.index(), 2);
        sel.move_by(Nav::Next, 3);
        assert_eq!(sel.index(), 0);
    }

    #[test]
    fn empty_list_has_no_selection() {
        let mut sel = Selection::default();
        sel.move_by(Nav::Next, 0);
        assert_eq!(sel.index(), 0);
        assert_eq!(sel.table_state(0).selected(), None);
    }
}
