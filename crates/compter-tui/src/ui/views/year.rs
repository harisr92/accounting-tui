//! `[` and `]` step through financial years; shared by every tab that shows a year

use crate::ui::key::Key;
use chrono::Local;

use crate::ui::view::KeyHint;
use compter_core::domain::FiscalYear;

pub const HINT: KeyHint = KeyHint::new("[ ]", "year");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct YearPicker(FiscalYear);

impl YearPicker {
    /// Starts on the current financial year
    pub fn current() -> Self {
        Self(FiscalYear::containing(Local::now().date_naive()))
    }

    pub fn year(self) -> FiscalYear {
        self.0
    }

    /// `true` when the key changed the year
    pub fn handle(&mut self, key: Key) -> bool {
        match key {
            Key::Char('[') => self.0 = self.0.previous(),
            Key::Char(']') => self.0 = self.0.next(),
            _ => return false,
        }
        true
    }
}
