//! A discount on an invoice's total, taken off before GST

use std::fmt;

use bigdecimal::{BigDecimal, RoundingMode, Zero};

use super::DomainError;

/// Taken off an invoice's subtotal before GST, so GST is charged on what is left. Entered as an
/// amount or a percent of the subtotal, and spread over the invoice's lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Discount {
    /// In the invoice's currency
    Amount(BigDecimal),
    /// Of the subtotal, more than 0 and less than 100
    Percent(BigDecimal),
}

impl Discount {
    /// What comes off `subtotal`, in whole paise: more than zero, and less than the subtotal so
    /// something is still billed
    pub fn amount_of(&self, subtotal: &BigDecimal) -> Result<BigDecimal, DomainError> {
        let amount = match self {
            Self::Amount(amount) => {
                if amount.with_scale_round(2, RoundingMode::HalfUp) != *amount {
                    return Err(DomainError::FractionOfPaisa);
                }
                amount.clone()
            }
            Self::Percent(percent) => {
                let hundred = BigDecimal::from(100);
                if *percent <= BigDecimal::zero() || *percent >= hundred {
                    return Err(DomainError::DiscountPercent);
                }
                (subtotal * percent / hundred).with_scale_round(2, RoundingMode::HalfUp)
            }
        };
        if amount <= BigDecimal::zero() {
            return Err(DomainError::NonPositiveAmount);
        }
        if amount >= *subtotal {
            return Err(DomainError::DiscountTooLarge {
                subtotal: subtotal.to_string(),
            });
        }
        Ok(amount)
    }

    /// The discount spread over lines worth `gross` each, in proportion and in whole paise; the
    /// last line takes what rounding left over. Every line keeps something to bill.
    pub fn split(&self, gross: &[BigDecimal]) -> Result<Vec<BigDecimal>, DomainError> {
        let subtotal: BigDecimal = gross.iter().sum();
        let total = self.amount_of(&subtotal)?;
        let mut shares = Vec::with_capacity(gross.len());
        let mut given = BigDecimal::zero();
        for (i, line) in gross.iter().enumerate() {
            let share = if i + 1 == gross.len() {
                &total - &given
            } else {
                (&total * line / &subtotal).with_scale_round(2, RoundingMode::HalfUp)
            };
            if share < BigDecimal::zero() || share >= *line {
                return Err(DomainError::DiscountTooLarge {
                    subtotal: subtotal.to_string(),
                });
            }
            given += &share;
            shares.push(share);
        }
        Ok(shares)
    }
}

/// `10%`, or the amount as typed
impl fmt::Display for Discount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Amount(amount) => write!(f, "{amount}"),
            Self::Percent(percent) => write!(f, "{}%", percent.normalized()),
        }
    }
}
