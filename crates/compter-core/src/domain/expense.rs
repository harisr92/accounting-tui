use std::fmt;
use std::str::FromStr;

use bigdecimal::{BigDecimal, RoundingMode, Zero};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::DomainError;
use super::error::require;

/// Stable expense identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ExpenseId(Uuid);

impl ExpenseId {
    pub fn generate() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl fmt::Display for ExpenseId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// The [`Display`](fmt::Display) form back, for front ends that hand ids around as text
impl FromStr for ExpenseId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(Self)
    }
}

/// What a business expense was for; each has its own ledger account
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpenseCategory {
    Software,
    Equipment,
    Internet,
    Travel,
    Workspace,
    ProfessionalFees,
    Other,
}

impl ExpenseCategory {
    pub const ALL: [Self; 7] = [
        Self::Software,
        Self::Equipment,
        Self::Internet,
        Self::Travel,
        Self::Workspace,
        Self::ProfessionalFees,
        Self::Other,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Software => "Software & subscriptions",
            Self::Equipment => "Equipment",
            Self::Internet => "Internet & phone",
            Self::Travel => "Travel",
            Self::Workspace => "Rent & coworking",
            Self::ProfessionalFees => "Professional fees",
            Self::Other => "Other",
        }
    }
}

/// Money spent on the business
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawExpense")]
pub struct Expense {
    id: ExpenseId,
    date: NaiveDate,
    category: ExpenseCategory,
    description: String,
    /// Total paid, GST included
    amount: BigDecimal,
    /// The part of `amount` that is GST you claim back as input tax credit
    gst_credit: BigDecimal,
}

#[derive(Deserialize)]
struct RawExpense {
    id: ExpenseId,
    date: NaiveDate,
    category: ExpenseCategory,
    description: String,
    amount: BigDecimal,
    #[serde(default)]
    gst_credit: BigDecimal,
}

impl TryFrom<RawExpense> for Expense {
    type Error = DomainError;

    fn try_from(raw: RawExpense) -> Result<Self, Self::Error> {
        let expense = Self {
            id: raw.id,
            date: raw.date,
            category: raw.category,
            description: raw.description,
            amount: raw.amount,
            gst_credit: raw.gst_credit,
        };
        expense.validate()?;
        Ok(expense)
    }
}

impl Expense {
    pub fn new(
        date: NaiveDate,
        category: ExpenseCategory,
        description: impl Into<String>,
        amount: BigDecimal,
        gst_credit: BigDecimal,
    ) -> Result<Self, DomainError> {
        let round = |value: BigDecimal| value.with_scale_round(2, RoundingMode::HalfUp);
        let expense = Self {
            id: ExpenseId::generate(),
            date,
            category,
            description: description.into().trim().to_owned(),
            amount: round(amount),
            gst_credit: round(gst_credit),
        };
        expense.validate()?;
        Ok(expense)
    }

    /// Correct every detail of the expense, keeping its id; nothing changes if any is invalid
    pub fn edit(
        &mut self,
        date: NaiveDate,
        category: ExpenseCategory,
        description: impl Into<String>,
        amount: BigDecimal,
        gst_credit: BigDecimal,
    ) -> Result<(), DomainError> {
        let edited = Self {
            id: self.id,
            ..Self::new(date, category, description, amount, gst_credit)?
        };
        *self = edited;
        Ok(())
    }

    pub fn id(&self) -> ExpenseId {
        self.id
    }

    pub fn date(&self) -> NaiveDate {
        self.date
    }

    pub fn category(&self) -> ExpenseCategory {
        self.category
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn amount(&self) -> &BigDecimal {
        &self.amount
    }

    pub fn gst_credit(&self) -> &BigDecimal {
        &self.gst_credit
    }

    /// What the expense costs the business: the amount less GST claimed back
    pub fn cost(&self) -> BigDecimal {
        &self.amount - &self.gst_credit
    }

    fn validate(&self) -> Result<(), DomainError> {
        require(&self.description, "expense description")?;
        if self.amount <= BigDecimal::zero() {
            return Err(DomainError::NonPositiveAmount);
        }
        if self.gst_credit < BigDecimal::zero() {
            return Err(DomainError::NegativeAmount);
        }
        if self.gst_credit >= self.amount {
            return Err(DomainError::GstCreditTooLarge);
        }
        Ok(())
    }
}
