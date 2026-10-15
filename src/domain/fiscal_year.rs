use std::fmt;

use chrono::{Datelike, NaiveDate};

/// Indian financial year, April to March, identified by its starting year
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FiscalYear {
    start_year: i32,
}

impl FiscalYear {
    pub fn containing(date: NaiveDate) -> Self {
        let year = date.year();
        let start_year = if date.month() >= 4 { year } else { year - 1 };
        Self { start_year }
    }
}

impl fmt::Display for FiscalYear {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "FY {}-{:02}",
            self.start_year,
            (self.start_year + 1) % 100
        )
    }
}
