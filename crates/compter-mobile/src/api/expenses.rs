use bigdecimal::{BigDecimal, Zero};
use chrono::{Local, NaiveDate};
use flutter_rust_bridge::frb;

use compter_core::domain::{Currency, Expense, ExpenseCategory, FiscalYear};

/// The financial year (April to March) today is in, by its starting year: where a year picker
/// starts
#[frb(sync)]
pub fn current_year() -> i32 {
    FiscalYear::containing(Local::now().date_naive()).start_year()
}

/// The categories an expense can have, as `ExpenseInput::category` takes them
#[frb(sync)]
pub fn expense_categories() -> Vec<String> {
    ExpenseCategory::ALL
        .iter()
        .map(|c| c.label().to_owned())
        .collect()
}

/// The financial year starting in April of `start_year`
pub(crate) fn fiscal_year(start_year: i32) -> FiscalYear {
    FiscalYear::containing(NaiveDate::from_ymd_opt(start_year, 4, 1).unwrap_or_default())
}

/// The category called `label`, as `expense_categories` lists them
pub(crate) fn category(label: &str) -> Option<ExpenseCategory> {
    ExpenseCategory::ALL
        .into_iter()
        .find(|c| c.label() == label.trim())
}

/// A financial year's expenses, by date
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpenseList {
    /// `FY 2026-27`
    pub year: String,
    /// What the expenses cost the business, after GST claimed back
    pub cost: String,
    pub expenses: Vec<ExpenseRow>,
}

impl ExpenseList {
    pub(crate) fn new(year: FiscalYear, expenses: &[Expense]) -> Self {
        let cost: BigDecimal = expenses.iter().map(Expense::cost).sum();
        Self {
            year: year.to_string(),
            cost: rupees(&cost),
            expenses: expenses.iter().map(ExpenseRow::from).collect(),
        }
    }
}

/// One expense, in rupees
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpenseRow {
    pub id: String,
    /// `YYYY-MM-DD`
    pub date: String,
    /// `Software & subscriptions`, ...
    pub category: String,
    pub description: String,
    /// Paid, GST included
    pub amount: String,
    /// GST claimed back as input credit; `None` when there is none
    pub gst_credit: Option<String>,
    /// The form, filled with this expense, to edit it
    pub edit: ExpenseInput,
}

impl From<&Expense> for ExpenseRow {
    fn from(expense: &Expense) -> Self {
        let credit = expense.gst_credit();
        Self {
            id: expense.id().to_string(),
            date: expense.date().to_string(),
            category: expense.category().label().to_owned(),
            description: expense.description().to_owned(),
            amount: rupees(expense.amount()),
            gst_credit: (!credit.is_zero()).then(|| rupees(credit)),
            edit: ExpenseInput {
                date: expense.date().to_string(),
                category: expense.category().label().to_owned(),
                description: expense.description().to_owned(),
                amount: expense.amount().to_string(),
                gst_credit: if credit.is_zero() {
                    String::new()
                } else {
                    credit.to_string()
                },
            },
        }
    }
}

/// An expense as typed
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpenseInput {
    /// `YYYY-MM-DD`; blank means today
    pub date: String,
    /// One of `expense_categories`
    pub category: String,
    pub description: String,
    /// Total paid, GST included
    pub amount: String,
    /// GST in the amount claimed back; blank for none. Needs a GSTIN in the profile.
    pub gst_credit: String,
}

fn rupees(amount: &BigDecimal) -> String {
    Currency::Inr.display(amount)
}
