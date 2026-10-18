use std::fmt;

use accounting_core::{GstInvoice, InvoicePrint};
use bigdecimal::{BigDecimal, Zero};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use super::error::require;
use super::{
    CreditNote, Currency, DomainError, ExchangeRate, FiscalYear, LogoId, MilestoneId, ProjectId,
};

/// TDS on professional fees under section 194J
const TDS_PERCENT: u32 = 10;

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

/// Money on the invoice, in the invoice's currency
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

    /// How the total is made up, for confirming an invoice: `50,000.00 + IGST 9,000.00`, or
    /// why there is no GST
    pub fn breakdown(&self, kind: InvoiceKind, currency: Currency) -> String {
        let format = |amount| currency.format(amount);
        let base = format(&self.taxable_value);
        match kind {
            InvoiceKind::Export => format!("{base}, zero-rated export under LUT"),
            InvoiceKind::Plain => format!("{base}, no GST"),
            InvoiceKind::Tax if !self.igst.is_zero() => {
                format!("{base} + IGST {}", format(&self.igst))
            }
            InvoiceKind::Tax => format!(
                "{base} + CGST {} + SGST {}",
                format(&self.cgst),
                format(&self.sgst)
            ),
        }
    }

    /// Every amount in rupees at `rate`
    pub(super) fn converted(&self, rate: &ExchangeRate) -> Self {
        Self {
            taxable_value: rate.to_inr(&self.taxable_value),
            cgst: rate.to_inr(&self.cgst),
            sgst: rate.to_inr(&self.sgst),
            igst: rate.to_inr(&self.igst),
            total: rate.to_inr(&self.total),
        }
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
    /// Rupees that reached the bank, for an invoice in a foreign currency (where `received` is in
    /// that currency)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credited_inr: Option<BigDecimal>,
}

impl Payment {
    /// How much of the invoice this payment settles, in the invoice's currency
    pub fn settled(&self) -> BigDecimal {
        &self.received + &self.tds
    }

    /// Rupees that reached the bank
    pub fn received_inr(&self) -> &BigDecimal {
        self.credited_inr.as_ref().unwrap_or(&self.received)
    }
}

/// A credit note's effect on the invoice it corrects
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credit {
    /// The credit note
    pub number: InvoiceNumber,
    pub date: NaiveDate,
    /// Taken off what the client owes, tax included, in the invoice's currency
    pub amount: BigDecimal,
}

/// How far an invoice has been settled, derived from its payments and credit notes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvoiceStatus {
    Issued,
    PartlyPaid,
    Paid,
    /// Credited in full, with nothing paid: the invoice is cancelled
    Credited,
}

impl InvoiceStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Issued => "ISSUED",
            Self::PartlyPaid => "PART PAID",
            Self::Paid => "PAID",
            Self::Credited => "CREDITED",
        }
    }

    /// Nothing is left to pay
    pub const fn is_settled(self) -> bool {
        matches!(self, Self::Paid | Self::Credited)
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
    #[serde(default)]
    currency: Currency,
    /// Rupees per unit of `currency`, on the invoice date; `None` for an invoice in rupees
    #[serde(default, skip_serializing_if = "Option::is_none")]
    exchange_rate: Option<ExchangeRate>,
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
    /// Credit notes issued against it
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    credits: Vec<Credit>,
    /// The logo printed on it, as it was when issued; `None` for an invoice without one
    #[serde(default, skip_serializing_if = "Option::is_none")]
    logo: Option<LogoId>,
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
    #[serde(default)]
    currency: Currency,
    #[serde(default)]
    exchange_rate: Option<ExchangeRate>,
    amounts: InvoiceAmounts,
    print: InvoicePrint,
    #[serde(default)]
    terms: Vec<String>,
    #[serde(default)]
    gst: Option<GstInvoice>,
    #[serde(default)]
    payments: Vec<Payment>,
    #[serde(default)]
    credits: Vec<Credit>,
    #[serde(default)]
    logo: Option<LogoId>,
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
        check_currency(raw.currency, raw.exchange_rate.as_ref(), raw.kind)?;
        let mut settled = BigDecimal::zero();
        for payment in &raw.payments {
            check_payment(raw.date, raw.currency, payment)?;
            settled += payment.settled();
        }
        for credit in &raw.credits {
            if credit.date < raw.date {
                return Err(DomainError::CreditBeforeInvoice);
            }
            if credit.amount <= BigDecimal::zero() {
                return Err(DomainError::NonPositiveAmount);
            }
            settled += &credit.amount;
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
            currency: raw.currency,
            exchange_rate: raw.exchange_rate,
            amounts: raw.amounts,
            print: raw.print,
            terms: raw.terms,
            gst: raw.gst,
            payments: raw.payments,
            credits: raw.credits,
            logo: raw.logo,
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
    pub currency: Currency,
    /// Required for a foreign currency, `None` for rupees
    pub exchange_rate: Option<ExchangeRate>,
    pub amounts: InvoiceAmounts,
    pub print: InvoicePrint,
    pub terms: Vec<String>,
    pub gst: Option<GstInvoice>,
    pub logo: Option<LogoId>,
}

impl Invoice {
    /// A freshly issued, unpaid invoice
    pub fn issue(parts: InvoiceParts) -> Result<Self, DomainError> {
        check_currency(parts.currency, parts.exchange_rate.as_ref(), parts.kind)?;
        Ok(Self {
            number: parts.number,
            date: parts.date,
            kind: parts.kind,
            project_id: parts.project_id,
            milestone_id: parts.milestone_id,
            client_name: parts.client_name,
            milestone_name: parts.milestone_name,
            currency: parts.currency,
            exchange_rate: parts.exchange_rate,
            amounts: parts.amounts,
            print: parts.print,
            terms: parts.terms,
            gst: parts.gst,
            payments: Vec::new(),
            credits: Vec::new(),
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

    /// In the invoice's currency
    pub fn amounts(&self) -> &InvoiceAmounts {
        &self.amounts
    }

    pub fn currency(&self) -> Currency {
        self.currency
    }

    pub fn exchange_rate(&self) -> Option<&ExchangeRate> {
        self.exchange_rate.as_ref()
    }

    /// The amounts in rupees at the invoice's rate: what GST, the ledger and reports use
    pub fn inr_amounts(&self) -> InvoiceAmounts {
        match &self.exchange_rate {
            Some(rate) => self.amounts.converted(rate),
            None => self.amounts.clone(),
        }
    }

    /// An amount of the invoice's currency in rupees at the invoice's rate
    pub fn to_inr(&self, amount: &BigDecimal) -> BigDecimal {
        match &self.exchange_rate {
            Some(rate) => rate.to_inr(amount),
            None => amount.clone(),
        }
    }

    pub fn print(&self) -> &InvoicePrint {
        &self.print
    }

    pub fn terms(&self) -> &[String] {
        &self.terms
    }

    /// The logo it was issued with, to print it again exactly
    pub fn logo(&self) -> Option<LogoId> {
        self.logo
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

    pub fn credits(&self) -> &[Credit] {
        &self.credits
    }

    /// Taken off by credit notes, in the invoice's currency
    pub fn credited(&self) -> BigDecimal {
        self.credits.iter().map(|c| &c.amount).sum()
    }

    /// Taken off by credit notes, in rupees at the invoice's rate
    pub fn credited_inr(&self) -> BigDecimal {
        self.credits.iter().map(|c| self.to_inr(&c.amount)).sum()
    }

    /// The receivable still on the books for this invoice, in rupees at its rate
    pub fn receivable_inr(&self) -> BigDecimal {
        let paid: BigDecimal = self
            .payments
            .iter()
            .map(|p| self.to_inr(&p.settled()))
            .sum();
        self.inr_amounts().total - paid - self.credited_inr()
    }

    /// What the client still owes
    pub fn balance(&self) -> BigDecimal {
        &self.amounts.total - self.settled() - self.credited()
    }

    /// TDS a client usually withholds on professional fees (section 194J): 10% of the taxable
    /// value. Only a suggestion; what was actually withheld is entered with the payment.
    pub fn usual_tds(&self) -> BigDecimal {
        &self.amounts.taxable_value * BigDecimal::from(TDS_PERCENT) / BigDecimal::from(100)
    }

    pub fn status(&self) -> InvoiceStatus {
        match (
            self.balance() > BigDecimal::zero(),
            self.payments.is_empty(),
        ) {
            (true, true) => InvoiceStatus::Issued,
            (true, false) => InvoiceStatus::PartlyPaid,
            (false, true) => InvoiceStatus::Credited,
            (false, false) => InvoiceStatus::Paid,
        }
    }

    pub fn is_paid(&self) -> bool {
        self.status() == InvoiceStatus::Paid
    }

    /// Nothing is left to pay: paid, credited, or both
    pub fn is_settled(&self) -> bool {
        self.status().is_settled()
    }

    /// Take a credit note against this invoice off what the client owes. A note credits only
    /// what is still due; money already received isn't refunded by one.
    pub fn apply_credit(&mut self, note: &CreditNote) -> Result<(), DomainError> {
        if note.invoice() != &self.number || note.currency() != self.currency {
            return Err(DomainError::CreditForAnotherInvoice);
        }
        if note.date() < self.date {
            return Err(DomainError::CreditBeforeInvoice);
        }
        let amount = note.amounts().total.clone();
        let balance = self.balance();
        if amount > balance {
            return Err(DomainError::CreditExceedsBalance {
                balance: balance.to_string(),
                got: amount.to_string(),
            });
        }
        self.credits.push(Credit {
            number: note.number().clone(),
            date: note.date(),
            amount,
        });
        Ok(())
    }

    /// Record a payment of all or part of the balance of an invoice in rupees; `received`
    /// reached the bank and `tds` was withheld. Returns the payment as recorded (rounded to paise).
    pub fn record_payment(
        &mut self,
        date: NaiveDate,
        received: BigDecimal,
        tds: BigDecimal,
    ) -> Result<Payment, DomainError> {
        self.add_payment(Payment {
            date,
            received: round(received),
            tds: round(tds),
            credited_inr: None,
        })
    }

    /// Record a payment on an invoice in a foreign currency: `received` (in that currency)
    /// settles the balance, `credited_inr` is what reached the bank at the day's rate
    pub fn record_foreign_payment(
        &mut self,
        date: NaiveDate,
        received: BigDecimal,
        credited_inr: BigDecimal,
    ) -> Result<Payment, DomainError> {
        self.add_payment(Payment {
            date,
            received: round(received),
            tds: BigDecimal::zero(),
            credited_inr: Some(round(credited_inr)),
        })
    }

    fn add_payment(&mut self, payment: Payment) -> Result<Payment, DomainError> {
        if self.is_settled() {
            return Err(DomainError::InvoiceAlreadyPaid);
        }
        check_payment(self.date, self.currency, &payment)?;
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

fn round(value: BigDecimal) -> BigDecimal {
    value.with_scale_round(2, bigdecimal::RoundingMode::HalfUp)
}

/// Rupees carry no rate; any other currency needs one and can't be on a GST tax invoice
pub(super) fn check_currency(
    currency: Currency,
    rate: Option<&ExchangeRate>,
    kind: InvoiceKind,
) -> Result<(), DomainError> {
    match (currency.is_inr(), rate) {
        (true, Some(_)) => Err(DomainError::UnexpectedExchangeRate),
        (true, None) => Ok(()),
        (false, None) => Err(DomainError::MissingExchangeRate(currency.code())),
        (false, Some(_)) if kind == InvoiceKind::Tax => Err(DomainError::ForeignTaxInvoice),
        (false, Some(_)) => Ok(()),
    }
}

/// A payment on its own: not before the invoice, nothing negative, something paid. A payment in
/// a foreign currency says what reached the bank in rupees and has no TDS.
fn check_payment(
    invoice_date: NaiveDate,
    currency: Currency,
    payment: &Payment,
) -> Result<(), DomainError> {
    if payment.date < invoice_date {
        return Err(DomainError::PaymentBeforeInvoice);
    }
    match (currency.is_inr(), &payment.credited_inr) {
        (true, Some(_)) => return Err(DomainError::UnexpectedCreditedAmount),
        (false, None) => return Err(DomainError::MissingCreditedAmount),
        (false, Some(credited)) if *credited <= BigDecimal::zero() => {
            return Err(DomainError::MissingCreditedAmount);
        }
        (false, Some(_)) if !payment.tds.is_zero() => {
            return Err(DomainError::TdsOnForeignInvoice);
        }
        _ => {}
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
    #[serde(default)]
    currency: Currency,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    exchange_rate: Option<ExchangeRate>,
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

    /// What the client still owes, in the invoice's currency
    pub fn balance(&self) -> &BigDecimal {
        &self.balance
    }

    /// What the client still owes, in rupees at the invoice's rate
    pub fn balance_inr(&self) -> BigDecimal {
        match &self.exchange_rate {
            Some(rate) => rate.to_inr(&self.balance),
            None => self.balance.clone(),
        }
    }

    pub fn currency(&self) -> Currency {
        self.currency
    }

    pub fn status(&self) -> InvoiceStatus {
        self.status
    }

    pub fn is_paid(&self) -> bool {
        self.status == InvoiceStatus::Paid
    }

    /// Nothing is left to pay: paid, credited, or both
    pub fn is_settled(&self) -> bool {
        self.status.is_settled()
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
            currency: invoice.currency,
            exchange_rate: invoice.exchange_rate.clone(),
            total: invoice.amounts.total.clone(),
            balance: invoice.balance(),
            status: invoice.status(),
        }
    }
}
