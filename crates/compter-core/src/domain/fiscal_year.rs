use std::fmt;

use chrono::{Datelike, NaiveDate};

/// Indian financial year, April to March, identified by its starting year
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FiscalYear {
    start_year: i32,
}

impl FiscalYear {
    pub fn containing(date: NaiveDate) -> Self {
        let year = date.year();
        let start_year = if date.month() >= 4 { year } else { year - 1 };
        Self { start_year }
    }

    pub fn start_year(self) -> i32 {
        self.start_year
    }

    pub fn previous(self) -> Self {
        Self {
            start_year: self.start_year - 1,
        }
    }

    pub fn next(self) -> Self {
        Self {
            start_year: self.start_year + 1,
        }
    }

    /// 1 April
    pub fn first_day(self) -> NaiveDate {
        NaiveDate::from_ymd_opt(self.start_year, 4, 1).unwrap_or_default()
    }

    /// 31 March of the next calendar year
    pub fn last_day(self) -> NaiveDate {
        NaiveDate::from_ymd_opt(self.start_year + 1, 3, 31).unwrap_or_default()
    }

    pub fn contains(self, date: NaiveDate) -> bool {
        Self::containing(date) == self
    }

    pub fn quarters(self) -> [Quarter; 4] {
        [1, 2, 3, 4].map(|number| Quarter { year: self, number })
    }

    /// `2026-27`
    pub fn label(self) -> String {
        format!("{}-{:02}", self.start_year, (self.start_year + 1) % 100)
    }

    /// `26-27`, as used in invoice numbers
    pub fn short(self) -> String {
        format!(
            "{:02}-{:02}",
            self.start_year % 100,
            (self.start_year + 1) % 100
        )
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

/// A quarter of a financial year: Q1 is April to June, Q4 January to March
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Quarter {
    year: FiscalYear,
    number: u8,
}

impl Quarter {
    pub fn containing(date: NaiveDate) -> Self {
        let number = match date.month() {
            4..=6 => 1,
            7..=9 => 2,
            10..=12 => 3,
            _ => 4,
        };
        Self {
            year: FiscalYear::containing(date),
            number,
        }
    }

    pub fn year(self) -> FiscalYear {
        self.year
    }

    /// 1 to 4
    pub fn number(self) -> u8 {
        self.number
    }

    /// Calendar `(year, month)` of each month in the quarter, in order
    pub fn months(self) -> [(i32, u32); 3] {
        let first = 4 + 3 * u32::from(self.number - 1);
        [0, 1, 2].map(|offset| {
            let month = first + offset;
            if month > 12 {
                (self.year.start_year + 1, month - 12)
            } else {
                (self.year.start_year, month)
            }
        })
    }

    pub fn contains(self, date: NaiveDate) -> bool {
        Self::containing(date) == self
    }

    /// `Q2 (Jul-Sep)`
    pub fn label(self) -> &'static str {
        match self.number {
            1 => "Q1 (Apr-Jun)",
            2 => "Q2 (Jul-Sep)",
            3 => "Q3 (Oct-Dec)",
            _ => "Q4 (Jan-Mar)",
        }
    }
}
