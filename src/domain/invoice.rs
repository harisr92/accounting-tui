use std::fmt;

use accounting_core::{GstInvoice, InvoicePrint};
use bigdecimal::{BigDecimal, Zero};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use super::error::require;
use super::{DomainError, FiscalYear, MilestoneId, ProjectId};

/// `INV/26-27/004`: prefix, financial year and a sequence that restarts every year
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InvoiceNumber(String);

impl InvoiceNumber {
    pub fn new(prefix: &str, year: FiscalYear, sequence: u32) -> Self {
        Self(format!("{prefix}/{}/{sequence:03}", year.short()))
    }

    /// A number read back from a record that refers to an invoice (a milestone's)
    pub fn from_stored(number: &str) -> Self {
        Self(number.to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Safe as a file name: `INV-26-27-004`
    pub fn file_stem(&self) -> String {
        self.0.replace('/', "-")
    }
}

impl fmt::Display for InvoiceNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Which kind of document was issued, decided by the seller's and the client's registration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvoiceKind {
    /// GST tax invoice to a client in India
    Tax,
    /// Zero-rated export of services under LUT
    Export,
    /// No GST: the seller is not registered
    Plain,
}

impl InvoiceKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Tax => "TAX",
            Self::Export => "EXPORT",
            Self::Plain => "PLAIN",
        }
    }
}

/// Money on the invoice, in rupees
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvoiceAmounts {
    pub taxable_value: BigDecimal,
    pub cgst: BigDecimal,
    pub sgst: BigDecimal,
    pub igst: BigDecimal,
    pub total: BigDecimal,
}

impl InvoiceAmounts {
    /// No GST: the total is the taxable value
    pub fn untaxed(taxable_value: BigDecimal) -> Self {
        Self {
            total: taxable_value.clone(),
            taxable_value,
            cgst: BigDecimal::zero(),
            sgst: BigDecimal::zero(),
            igst: BigDecimal::zero(),
        }
    }

    pub fn total_tax(&self) -> BigDecimal {
        &self.cgst + &self.sgst + &self.igst
    }
}

/// Whether the client has settled the invoice
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum InvoiceStatus {
    Issued,
    /// `received` reached the bank; `tds` was withheld by the client and is claimable in the ITR
    Paid {
        date: NaiveDate,
        received: BigDecimal,
        tds: BigDecimal,
    },
}

impl InvoiceStatus {
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Issued => "ISSUED",
            Self::Paid { .. } => "PAID",
        }
    }
}

/// An issued invoice: the billing record. Only its payment status ever changes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "RawInvoice")]
pub struct Invoice {
    number: InvoiceNumber,
    date: NaiveDate,
    kind: InvoiceKind,
    project_id: ProjectId,
    milestone_id: MilestoneId,
    client_name: String,
    milestone_name: String,
    amounts: InvoiceAmounts,
    /// The printed document, kept so the PDF can be produced again exactly
    print: InvoicePrint,
    /// Lines printed at the end (bank details, LUT endorsement), as they were when issued
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    terms: Vec<String>,
    /// The GST invoice behind a tax invoice, for GSTR-1
    #[serde(default, skip_serializing_if = "Option::is_none")]
    gst: Option<GstInvoice>,
    status: InvoiceStatus,
}

#[derive(Deserialize)]
struct RawInvoice {
    number: InvoiceNumber,
    date: NaiveDate,
    kind: InvoiceKind,
    project_id: ProjectId,
    milestone_id: MilestoneId,
    client_name: String,
    milestone_name: String,
    amounts: InvoiceAmounts,
    print: InvoicePrint,
    #[serde(default)]
    terms: Vec<String>,
    #[serde(default)]
    gst: Option<GstInvoice>,
    status: InvoiceStatus,
}

impl TryFrom<RawInvoice> for Invoice {
    type Error = DomainError;

    fn try_from(raw: RawInvoice) -> Result<Self, Self::Error> {
        require(raw.number.as_str(), "invoice number")?;
        if let InvoiceStatus::Paid { received, tds, .. } = &raw.status {
            check_payment(&raw.amounts.total, received, tds)?;
        }
        Ok(Self {
            number: raw.number,
            date: raw.date,
            kind: raw.kind,
            project_id: raw.project_id,
            milestone_id: raw.milestone_id,
            client_name: raw.client_name,
            milestone_name: raw.milestone_name,
            amounts: raw.amounts,
            print: raw.print,
            terms: raw.terms,
            gst: raw.gst,
            status: raw.status,
        })
    }
}

/// Parts of a new invoice, assembled by the invoicing service
#[derive(Debug, Clone)]
pub struct InvoiceParts {
    pub number: InvoiceNumber,
    pub date: NaiveDate,
    pub kind: InvoiceKind,
    pub project_id: ProjectId,
    pub milestone_id: MilestoneId,
    pub client_name: String,
    pub milestone_name: String,
    pub amounts: InvoiceAmounts,
    pub print: InvoicePrint,
    pub terms: Vec<String>,
    pub gst: Option<GstInvoice>,
}

impl Invoice {
    /// A freshly issued, unpaid invoice
    pub fn issue(parts: InvoiceParts) -> Self {
        Self {
            number: parts.number,
            date: parts.date,
            kind: parts.kind,
            project_id: parts.project_id,
            milestone_id: parts.milestone_id,
            client_name: parts.client_name,
            milestone_name: parts.milestone_name,
            amounts: parts.amounts,
            print: parts.print,
            terms: parts.terms,
            gst: parts.gst,
            status: InvoiceStatus::Issued,
        }
    }

    pub fn number(&self) -> &InvoiceNumber {
        &self.number
    }

    pub fn date(&self) -> NaiveDate {
        self.date
    }

    pub fn fiscal_year(&self) -> FiscalYear {
        FiscalYear::containing(self.date)
    }

    pub fn kind(&self) -> InvoiceKind {
        self.kind
    }

    pub fn project_id(&self) -> ProjectId {
        self.project_id
    }

    pub fn milestone_id(&self) -> MilestoneId {
        self.milestone_id
    }

    pub fn client_name(&self) -> &str {
        &self.client_name
    }

    pub fn milestone_name(&self) -> &str {
        &self.milestone_name
    }

    pub fn amounts(&self) -> &InvoiceAmounts {
        &self.amounts
    }

    pub fn print(&self) -> &InvoicePrint {
        &self.print
    }

    pub fn terms(&self) -> &[String] {
        &self.terms
    }

    pub fn gst(&self) -> Option<&GstInvoice> {
        self.gst.as_ref()
    }

    pub fn status(&self) -> &InvoiceStatus {
        &self.status
    }

    /// Settle in full: `received` in the bank plus `tds` withheld must equal the total.
    /// Returns both amounts as recorded (rounded to paise).
    pub fn record_payment(
        &mut self,
        date: NaiveDate,
        received: BigDecimal,
        tds: BigDecimal,
    ) -> Result<(BigDecimal, BigDecimal), DomainError> {
        if matches!(self.status, InvoiceStatus::Paid { .. }) {
            return Err(DomainError::InvoiceAlreadyPaid);
        }
        if date < self.date {
            return Err(DomainError::PaymentBeforeInvoice);
        }
        let received = received.with_scale_round(2, bigdecimal::RoundingMode::HalfUp);
        let tds = tds.with_scale_round(2, bigdecimal::RoundingMode::HalfUp);
        check_payment(&self.amounts.total, &received, &tds)?;
        self.status = InvoiceStatus::Paid {
            date,
            received: received.clone(),
            tds: tds.clone(),
        };
        Ok((received, tds))
    }
}

fn check_payment(
    total: &BigDecimal,
    received: &BigDecimal,
    tds: &BigDecimal,
) -> Result<(), DomainError> {
    if *received < BigDecimal::zero() || *tds < BigDecimal::zero() {
        return Err(DomainError::NegativeAmount);
    }
    let paid = received + tds;
    if paid != *total {
        return Err(DomainError::PaymentMismatch {
            expected: total.to_string(),
            got: paid.to_string(),
        });
    }
    Ok(())
}

/// What the invoice list shows, without the printed document
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvoiceSummary {
    number: InvoiceNumber,
    date: NaiveDate,
    kind: InvoiceKind,
    client_name: Box<str>,
    milestone_name: Box<str>,
    total: BigDecimal,
    paid: bool,
}

impl InvoiceSummary {
    pub fn number(&self) -> &InvoiceNumber {
        &self.number
    }

    pub fn date(&self) -> NaiveDate {
        self.date
    }

    pub fn kind(&self) -> InvoiceKind {
        self.kind
    }

    pub fn client_name(&self) -> &str {
        &self.client_name
    }

    pub fn milestone_name(&self) -> &str {
        &self.milestone_name
    }

    pub fn total(&self) -> &BigDecimal {
        &self.total
    }

    pub fn is_paid(&self) -> bool {
        self.paid
    }
}

impl From<&Invoice> for InvoiceSummary {
    fn from(invoice: &Invoice) -> Self {
        Self {
            number: invoice.number.clone(),
            date: invoice.date,
            kind: invoice.kind,
            client_name: invoice.client_name.as_str().into(),
            milestone_name: invoice.milestone_name.as_str().into(),
            total: invoice.amounts.total.clone(),
            paid: matches!(invoice.status, InvoiceStatus::Paid { .. }),
        }
    }
}
