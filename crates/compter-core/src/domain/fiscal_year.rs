use std::cmp::Ordering;
use std::fmt;

use chrono::{Datelike, Months, NaiveDate};

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

    /// The year from April of `start_year`
    pub const fn starting(start_year: i32) -> Self {
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

    /// The quarter whose GSTR-1 is due on `today`: the last one of this year that has ended (Q1
    /// while Q1 is still running), Q4 of a past year, Q1 of a future one
    pub fn quarter_to_file(self, today: NaiveDate) -> Quarter {
        let current = Quarter::containing(today);
        let number = match self.cmp(&current.year()) {
            Ordering::Equal => current.number().saturating_sub(1).max(1),
            Ordering::Less => 4,
            Ordering::Greater => 1,
        };
        Quarter { year: self, number }
    }

    /// Calendar `(year, month)` of the months an IFF can be filed for: the first two of each
    /// quarter, in order
    pub fn iff_months(self) -> [(i32, u32); 8] {
        let [q1, q2, q3, q4] = self.quarters().map(|q| q.months());
        [q1[0], q1[1], q2[0], q2[1], q3[0], q3[1], q4[0], q4[1]]
    }

    /// Position in [`Self::iff_months`] of the latest month that has ended on `today`; the
    /// first for a year with none ended yet
    pub fn iff_month_to_file(self, today: NaiveDate) -> usize {
        let ended = |(year, month): (i32, u32)| {
            NaiveDate::from_ymd_opt(year, month, 1)
                .and_then(|first| first.checked_add_months(Months::new(1)))
                .is_some_and(|next| next <= today)
        };
        self.iff_months().into_iter().rposition(ended).unwrap_or(0)
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
