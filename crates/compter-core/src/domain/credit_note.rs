use accounting_core::InvoicePrint;
use bigdecimal::{BigDecimal, Zero};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use super::error::require;
use super::invoice::check_currency;
use super::{
    Currency, DomainError, ExchangeRate, FiscalYear, InvoiceAmounts, InvoiceKind, InvoiceNumber,
    LogoId, MilestoneId,
};

/// Credit notes are numbered in their own series: `CN/26-27/001`
pub const CREDIT_NOTE_PREFIX: &str = "CN";

/// What a credit note takes back from one line of its invoice
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditLine {
    /// The milestone the invoice line billed
    pub milestone_id: MilestoneId,
    /// Before GST, in the note's currency
    pub taxable_value: BigDecimal,
}

/// A credit note: takes all or part of an invoice back, reducing what the client owes. The way
/// to correct an issued invoice, which is never edited. A billing record like an invoice:
/// immutable once issued.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "RawCreditNote")]
pub struct CreditNote {
    number: InvoiceNumber,
    date: NaiveDate,
    /// The invoice it corrects
    invoice: InvoiceNumber,
    /// The kind of that invoice
    kind: InvoiceKind,
    client_name: String,
    reason: String,
    /// The invoice lines it takes back from. Empty on notes from before v5, which credited
    /// their invoice's only line.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    lines: Vec<CreditLine>,
    /// The invoice's currency and rate: a note credits at the rate the invoice was issued at
    #[serde(default)]
    currency: Currency,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    exchange_rate: Option<ExchangeRate>,
    /// What is credited, tax included, in `currency`
    amounts: InvoiceAmounts,
    /// The printed document, kept so the PDF can be produced again exactly
    print: InvoicePrint,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    terms: Vec<String>,
    /// The GST credit note behind a note on a tax invoice, for GSTR-1
    #[serde(default, skip_serializing_if = "Option::is_none")]
    gst: Option<accounting_core::CreditNote>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    logo: Option<LogoId>,
}

#[derive(Deserialize)]
struct RawCreditNote {
    number: InvoiceNumber,
    date: NaiveDate,
    invoice: InvoiceNumber,
    kind: InvoiceKind,
    client_name: String,
    reason: String,
    #[serde(default)]
    lines: Vec<CreditLine>,
    #[serde(default)]
    currency: Currency,
    #[serde(default)]
    exchange_rate: Option<ExchangeRate>,
    amounts: InvoiceAmounts,
    print: InvoicePrint,
    #[serde(default)]
    terms: Vec<String>,
    #[serde(default)]
    gst: Option<accounting_core::CreditNote>,
    #[serde(default)]
    logo: Option<LogoId>,
}

impl TryFrom<RawCreditNote> for CreditNote {
    type Error = DomainError;

    fn try_from(raw: RawCreditNote) -> Result<Self, Self::Error> {
        Self::issue(CreditNoteParts {
            number: raw.number,
            date: raw.date,
            invoice: raw.invoice,
            kind: raw.kind,
            client_name: raw.client_name,
            reason: raw.reason,
            lines: raw.lines,
            currency: raw.currency,
            exchange_rate: raw.exchange_rate,
            amounts: raw.amounts,
            print: raw.print,
            terms: raw.terms,
            gst: raw.gst,
            logo: raw.logo,
        })
    }
}

/// Parts of a new credit note, assembled by the invoicing service
#[derive(Debug, Clone)]
pub struct CreditNoteParts {
    pub number: InvoiceNumber,
    pub date: NaiveDate,
    pub invoice: InvoiceNumber,
    pub kind: InvoiceKind,
    pub client_name: String,
    pub reason: String,
    pub lines: Vec<CreditLine>,
    pub currency: Currency,
    pub exchange_rate: Option<ExchangeRate>,
    pub amounts: InvoiceAmounts,
    pub print: InvoicePrint,
    pub terms: Vec<String>,
    pub gst: Option<accounting_core::CreditNote>,
    pub logo: Option<LogoId>,
}

impl CreditNote {
    pub fn issue(parts: CreditNoteParts) -> Result<Self, DomainError> {
        require(parts.number.as_str(), "credit note number")?;
        require(&parts.reason, "reason for the credit note")?;
        check_currency(parts.currency, parts.exchange_rate.as_ref(), parts.kind)?;
        if parts.amounts.total <= BigDecimal::zero() {
            return Err(DomainError::NonPositiveAmount);
        }
        for (i, line) in parts.lines.iter().enumerate() {
            if line.taxable_value <= BigDecimal::zero() {
                return Err(DomainError::NonPositiveAmount);
            }
            if parts.lines[..i]
                .iter()
                .any(|l| l.milestone_id == line.milestone_id)
            {
                return Err(DomainError::DuplicateCreditLine);
            }
        }
        Ok(Self {
            number: parts.number,
            date: parts.date,
            invoice: parts.invoice,
            kind: parts.kind,
            client_name: parts.client_name,
            reason: parts.reason.trim().to_owned(),
            lines: parts.lines,
            currency: parts.currency,
            exchange_rate: parts.exchange_rate,
            amounts: parts.amounts,
            print: parts.print,
            terms: parts.terms,
            gst: parts.gst,
            logo: parts.logo,
        })
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

    /// The invoice it corrects
    pub fn invoice(&self) -> &InvoiceNumber {
        &self.invoice
    }

    pub fn kind(&self) -> InvoiceKind {
        self.kind
    }

    pub fn client_name(&self) -> &str {
        &self.client_name
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }

    /// The invoice lines taken back from; empty on a note from before v5
    pub fn lines(&self) -> &[CreditLine] {
        &self.lines
    }

    pub fn currency(&self) -> Currency {
        self.currency
    }

    pub fn exchange_rate(&self) -> Option<&ExchangeRate> {
        self.exchange_rate.as_ref()
    }

    /// In the note's (the invoice's) currency
    pub fn amounts(&self) -> &InvoiceAmounts {
        &self.amounts
    }

    /// The amounts in rupees at the invoice's rate: what GST, the ledger and reports use
    pub fn inr_amounts(&self) -> InvoiceAmounts {
        match &self.exchange_rate {
            Some(rate) => self.amounts.converted(rate),
            None => self.amounts.clone(),
        }
    }

    pub fn print(&self) -> &InvoicePrint {
        &self.print
    }

    pub fn terms(&self) -> &[String] {
        &self.terms
    }

    pub fn gst(&self) -> Option<&accounting_core::CreditNote> {
        self.gst.as_ref()
    }

    pub fn logo(&self) -> Option<LogoId> {
        self.logo
    }
}
