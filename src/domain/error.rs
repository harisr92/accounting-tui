use accounting_core::InvoiceError;

use super::MilestoneStatus;

/// Why a domain operation or a loaded record was rejected
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("{0} must not be empty")]
    EmptyField(&'static str),
    #[error(transparent)]
    InvalidGstin(#[from] InvoiceError),
    #[error("amount must be greater than zero")]
    NonPositiveAmount,
    #[error("hourly rate must be greater than zero")]
    NonPositiveRate,
    #[error("time entry must be at least one minute")]
    ZeroDuration,
    #[error("one time entry can be at most 24 hours; split longer work by day")]
    EntryTooLong,
    #[error("milestone cannot move from {from} to {to}")]
    InvalidTransition {
        from: MilestoneStatus,
        to: MilestoneStatus,
    },
    #[error("milestone is already invoiced and can no longer change")]
    MilestoneLocked,
    #[error("milestone does not match the project's billing type")]
    BillingMismatch,
    #[error("no milestone at position {0}")]
    MilestoneNotFound(usize),
    #[error("milestone has nothing to invoice")]
    NothingToInvoice,
    #[error("milestone has no time entries")]
    NoTimeEntries,
    #[error("this project has invoiced milestones; they are billing records and can't be deleted")]
    HasBilledMilestones,
    #[error("a client abroad can't have a GSTIN")]
    ForeignClientWithGstin,
    #[error("SAC code {0} is not in the GST rate list")]
    UnknownSacCode(String),
    #[error("invoice prefix must be 1 to 5 letters, digits or '-'")]
    InvalidInvoicePrefix,
    #[error("amount can't be negative")]
    NegativeAmount,
    #[error("this invoice is already paid")]
    InvoiceAlreadyPaid,
    #[error("received plus TDS is {got}, but the invoice total is {expected}")]
    PaymentMismatch { expected: String, got: String },
    #[error("payment date is before the invoice date")]
    PaymentBeforeInvoice,
}

/// Reject blank text fields
pub(super) fn require(value: &str, field: &'static str) -> Result<(), DomainError> {
    if value.trim().is_empty() {
        Err(DomainError::EmptyField(field))
    } else {
        Ok(())
    }
}
