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
    #[error(
        "only finished projects can be archived: invoice or delete the pending milestones first"
    )]
    ProjectNotFinished,
    #[error("this project is archived; restore it (A) to add work")]
    ProjectArchived,
    #[error("a client abroad can't have a GSTIN")]
    ForeignClientWithGstin,
    #[error("SAC code {0} is not in the GST rate list")]
    UnknownSacCode(String),
    #[error("invoice prefix must be 1 to 5 letters, digits or '-'")]
    InvalidInvoicePrefix,
    #[error("invoice prefix {0} is the credit note series; choose another")]
    ReservedInvoicePrefix(&'static str),
    #[error("amount can't be negative")]
    NegativeAmount,
    #[error("nothing is due on this invoice")]
    InvoiceAlreadyPaid,
    #[error(
        "credit note total is {got}, more than the {balance} still due; it can't refund what was paid"
    )]
    CreditExceedsBalance { balance: String, got: String },
    #[error("credit note date is before the invoice date")]
    CreditBeforeInvoice,
    #[error("this credit note is for another invoice")]
    CreditForAnotherInvoice,
    #[error("only an invoiced milestone can be credited")]
    NotInvoiced,
    #[error("credit notes can take back at most what the milestone billed")]
    CreditExceedsMilestone,
    #[error("received plus TDS is {got}, more than the {balance} still due")]
    PaymentExceedsBalance { balance: String, got: String },
    #[error("this invoice record is in the old (v1) format; the database needs its upgrade")]
    OutdatedInvoiceFormat,
    #[error("payment date is before the invoice date")]
    PaymentBeforeInvoice,
    #[error("GST credit must be less than the amount paid")]
    GstCreditTooLarge,
    #[error("exchange rate must be greater than zero")]
    InvalidExchangeRate,
    #[error("a client in India is billed in INR; pick INR or set the client abroad")]
    ForeignCurrencyInIndia,
    #[error("the currency can't change once a milestone is invoiced")]
    CurrencyLocked,
    #[error("an invoice in {0} needs the exchange rate it was issued at")]
    MissingExchangeRate(&'static str),
    #[error("an invoice in INR has no exchange rate")]
    UnexpectedExchangeRate,
    #[error("a GST tax invoice must be in INR")]
    ForeignTaxInvoice,
    #[error("clients abroad don't withhold Indian TDS; record the payment without it")]
    TdsOnForeignInvoice,
    #[error("enter the rupees credited to your bank for this payment")]
    MissingCreditedAmount,
    #[error("rupees credited are only recorded for invoices in a foreign currency")]
    UnexpectedCreditedAmount,
    #[error("the logo must be a PNG or JPEG image")]
    InvalidLogo,
    #[error("the logo is {bytes} bytes; use an image under 1 MB")]
    LogoTooLarge { bytes: usize },
}

/// Reject blank text fields
pub(super) fn require(value: &str, field: &'static str) -> Result<(), DomainError> {
    if value.trim().is_empty() {
        Err(DomainError::EmptyField(field))
    } else {
        Ok(())
    }
}
