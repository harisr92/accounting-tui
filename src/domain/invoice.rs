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

/// One receipt against an invoice. A client may pay in parts; each part can have TDS withheld.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Payment {
    pub date: NaiveDate,
    /// Amount that reached the bank
    pub received: BigDecimal,
    /// Tax the client withheld on this payment, claimable in the ITR
    pub tds: BigDecimal,
}

impl Payment {
    /// How much of the invoice this payment settles
    pub fn settled(&self) -> BigDecimal {
        &self.received + &self.tds
    }
}

/// How far an invoice has been paid, derived from its payments
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvoiceStatus {
    Issued,
    PartlyPaid,
    Paid,
}

impl InvoiceStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Issued => "ISSUED",
            Self::PartlyPaid => "PART PAID",
            Self::Paid => "PAID",
        }
    }
}

/// An issued invoice: the billing record. Only its list of payments ever grows.
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    payments: Vec<Payment>,
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
    #[serde(default)]
    payments: Vec<Payment>,
    /// The v1 format's single payment status; v2 records list `payments` instead
    #[serde(default)]
    status: Option<serde::de::IgnoredAny>,
}

impl TryFrom<RawInvoice> for Invoice {
    type Error = DomainError;

    fn try_from(raw: RawInvoice) -> Result<Self, Self::Error> {
        require(raw.number.as_str(), "invoice number")?;
        if raw.status.is_some() {
            return Err(DomainError::OutdatedInvoiceFormat);
        }
        let mut settled = BigDecimal::zero();
        for payment in &raw.payments {
            check_payment(raw.date, payment)?;
            settled += payment.settled();
        }
        if settled > raw.amounts.total {
            return Err(DomainError::PaymentExceedsBalance {
                balance: raw.amounts.total.to_string(),
                got: settled.to_string(),
            });
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
            payments: raw.payments,
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
            payments: Vec::new(),
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

    pub fn payments(&self) -> &[Payment] {
        &self.payments
    }

    /// Received plus TDS across every payment
    pub fn settled(&self) -> BigDecimal {
        self.payments.iter().map(Payment::settled).sum()
    }

    /// What the client still owes
    pub fn balance(&self) -> BigDecimal {
        &self.amounts.total - self.settled()
    }

    pub fn status(&self) -> InvoiceStatus {
        if self.payments.is_empty() {
            InvoiceStatus::Issued
        } else if self.balance() > BigDecimal::zero() {
            InvoiceStatus::PartlyPaid
        } else {
            InvoiceStatus::Paid
        }
    }

    pub fn is_paid(&self) -> bool {
        self.status() == InvoiceStatus::Paid
    }

    /// Record a payment of all or part of the balance; `received` reached the bank and `tds`
    /// was withheld. Returns the payment as recorded (rounded to paise).
    pub fn record_payment(
        &mut self,
        date: NaiveDate,
        received: BigDecimal,
        tds: BigDecimal,
    ) -> Result<Payment, DomainError> {
        if self.is_paid() {
            return Err(DomainError::InvoiceAlreadyPaid);
        }
        let round = |value: BigDecimal| value.with_scale_round(2, bigdecimal::RoundingMode::HalfUp);
        let payment = Payment {
            date,
            received: round(received),
            tds: round(tds),
        };
        check_payment(self.date, &payment)?;
        let balance = self.balance();
        if payment.settled() > balance {
            return Err(DomainError::PaymentExceedsBalance {
                balance: balance.to_string(),
                got: payment.settled().to_string(),
            });
        }
        self.payments.push(payment.clone());
        Ok(payment)
    }
}

/// A payment on its own: not before the invoice, nothing negative, something paid
fn check_payment(invoice_date: NaiveDate, payment: &Payment) -> Result<(), DomainError> {
    if payment.date < invoice_date {
        return Err(DomainError::PaymentBeforeInvoice);
    }
    if payment.received < BigDecimal::zero() || payment.tds < BigDecimal::zero() {
        return Err(DomainError::NegativeAmount);
    }
    if payment.settled() <= BigDecimal::zero() {
        return Err(DomainError::NonPositiveAmount);
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
    balance: BigDecimal,
    status: InvoiceStatus,
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

    /// What the client still owes
    pub fn balance(&self) -> &BigDecimal {
        &self.balance
    }

    pub fn status(&self) -> InvoiceStatus {
        self.status
    }

    pub fn is_paid(&self) -> bool {
        self.status == InvoiceStatus::Paid
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
            balance: invoice.balance(),
            status: invoice.status(),
        }
    }
}
