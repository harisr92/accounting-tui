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

/// One billed milestone on an invoice: a row of the printed document
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvoiceLine {
    pub milestone_id: MilestoneId,
    pub milestone_name: String,
    /// Before GST, in the invoice's currency: the milestone's amount when it was invoiced
    pub taxable_value: BigDecimal,
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
    /// The part of `amount` the client had already paid, given back; `None` when the note took
    /// off only what was still due
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub returned: Option<Returned>,
}

/// Money already received that a credit note gives back to the client
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Returned {
    /// In the invoice's currency
    pub amount: BigDecimal,
    pub kind: ReturnKind,
    /// Rupees that left the bank, for a refund on an invoice in a foreign currency
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refunded_inr: Option<BigDecimal>,
    /// Part of `amount` that is TDS the client withheld and is cancelling in its TDS return, so
    /// it comes off your TDS credit rather than going back as cash. Rupee invoices only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tds_reversed: Option<BigDecimal>,
}

impl Returned {
    /// What actually goes back to the client as money: refunded from the bank or held as an
    /// advance. `amount` less the TDS reversed.
    pub fn cash(&self) -> BigDecimal {
        match &self.tds_reversed {
            Some(tds) => &self.amount - tds,
            None => self.amount.clone(),
        }
    }
}

/// How a credit note gives back money already paid, as entered; used only when the note takes
/// off more than is due
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReturnTerms {
    pub kind: ReturnKind,
    /// Rupees refunded from the bank, for a refund on an invoice in a foreign currency
    pub refunded_inr: Option<BigDecimal>,
    /// TDS the client is reversing in its return, on a rupee invoice
    pub tds_reversed: Option<BigDecimal>,
}

/// How money a credit note gives back reaches the client
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReturnKind {
    /// Paid back from the bank
    #[default]
    Refunded,
    /// Held for the client against a future invoice
    Advance,
}

impl ReturnKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Refunded => "refunded",
            Self::Advance => "advance",
        }
    }
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
    client_name: String,
    /// The milestones billed, one printed row each, in the order printed
    lines: Vec<InvoiceLine>,
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
    client_name: String,
    #[serde(default)]
    lines: Vec<InvoiceLine>,
    /// Before v5 an invoice billed one milestone, named by these two fields instead of `lines`
    #[serde(default)]
    milestone_id: Option<MilestoneId>,
    #[serde(default)]
    milestone_name: Option<String>,
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
        let lines = match (raw.lines.is_empty(), raw.milestone_id, raw.milestone_name) {
            (true, Some(milestone_id), Some(milestone_name)) => vec![InvoiceLine {
                milestone_id,
                milestone_name,
                taxable_value: raw.amounts.taxable_value.clone(),
            }],
            _ => raw.lines,
        };
        check_lines(&lines, &raw.amounts)?;
        let mut paid = BigDecimal::zero();
        for payment in &raw.payments {
            check_payment(raw.date, raw.currency, payment)?;
            paid += payment.settled();
        }
        let mut credited = BigDecimal::zero();
        let mut returned = BigDecimal::zero();
        for credit in &raw.credits {
            if credit.date < raw.date {
                return Err(DomainError::CreditBeforeInvoice);
            }
            if credit.amount <= BigDecimal::zero() {
                return Err(DomainError::NonPositiveAmount);
            }
            credited += &credit.amount;
            if let Some(back) = &credit.returned {
                if back.amount <= BigDecimal::zero() || back.amount > credit.amount {
                    return Err(DomainError::ReturnExceedsPaid);
                }
                returned += &back.amount;
            }
        }
        check_returns(raw.currency, &raw.payments, &raw.credits)?;
        if credited > raw.amounts.total {
            return Err(DomainError::CreditExceedsInvoice {
                remaining: raw.amounts.total.to_string(),
                got: credited.to_string(),
            });
        }
        if returned > paid {
            return Err(DomainError::ReturnExceedsPaid);
        }
        let owed = &paid + &credited - &returned;
        if owed > raw.amounts.total {
            return Err(DomainError::PaymentExceedsBalance {
                balance: raw.amounts.total.to_string(),
                got: owed.to_string(),
            });
        }
        Ok(Self {
            number: raw.number,
            date: raw.date,
            kind: raw.kind,
            project_id: raw.project_id,
            client_name: raw.client_name,
            lines,
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
    pub client_name: String,
    pub lines: Vec<InvoiceLine>,
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
        check_lines(&parts.lines, &parts.amounts)?;
        Ok(Self {
            number: parts.number,
            date: parts.date,
            kind: parts.kind,
            project_id: parts.project_id,
            client_name: parts.client_name,
            lines: parts.lines,
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

    pub fn client_name(&self) -> &str {
        &self.client_name
    }

    /// The milestones billed, in the order printed
    pub fn lines(&self) -> &[InvoiceLine] {
        &self.lines
    }

    /// The line billing milestone `id`, if this invoice billed it
    pub fn line(&self, id: MilestoneId) -> Option<&InvoiceLine> {
        self.lines.iter().find(|line| line.milestone_id == id)
    }

    pub fn milestone_ids(&self) -> Vec<MilestoneId> {
        self.lines.iter().map(|line| line.milestone_id).collect()
    }

    /// The milestones billed, for lists: `Design, Build`
    pub fn milestones(&self) -> String {
        milestone_names(&self.lines)
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

    /// Money already received that credit notes gave back, in the invoice's currency
    pub fn returned(&self) -> BigDecimal {
        self.credits
            .iter()
            .filter_map(|c| c.returned.as_ref())
            .map(|r| &r.amount)
            .sum()
    }

    /// Nothing paid or credited yet: a credit note can only cancel what is still owed
    pub fn untouched(&self) -> bool {
        self.payments.is_empty() && self.credits.is_empty()
    }

    /// What credit notes can still take off: the total less what they already took
    pub fn creditable(&self) -> BigDecimal {
        &self.amounts.total - self.credited()
    }

    /// What is due and what credit notes can still take off, and that any part beyond what is
    /// due goes back to the client: `₹47,200.00 due · up to ₹59,000.00 incl. GST can be
    /// credited; …`. Shown while picking the lines of a credit note, by every front end.
    pub fn credit_guide(&self) -> String {
        let currency = self.currency();
        let balance = self.balance();
        let creditable = self.creditable();
        let mut guide = format!(
            "{} due · up to {} incl. GST can be credited",
            currency.display(&balance),
            currency.display(&creditable)
        );
        if creditable > balance {
            guide.push_str("; beyond what is due goes back to the client");
        }
        guide
    }

    /// Kept for the client as an advance by credit notes, in rupees at the invoice's rate
    pub fn advance_inr(&self) -> BigDecimal {
        self.credits
            .iter()
            .filter_map(|c| c.returned.as_ref())
            .filter(|r| r.kind == ReturnKind::Advance)
            .map(|r| self.to_inr(&r.amount))
            .sum()
    }

    /// The receivable still on the books for this invoice, in rupees at its rate
    pub fn receivable_inr(&self) -> BigDecimal {
        let paid: BigDecimal = self
            .payments
            .iter()
            .map(|p| self.to_inr(&p.settled()))
            .sum();
        self.inr_amounts().total - paid - self.credited_inr() + self.to_inr(&self.returned())
    }

    /// What the client still owes
    pub fn balance(&self) -> BigDecimal {
        &self.amounts.total - self.settled() - self.credited() + self.returned()
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

    /// Take a credit note against this invoice off what the client owes. The part of the note
    /// beyond what is still due was already paid, so it goes back to the client as `terms` say:
    /// a refund on a foreign invoice says the rupees that left the bank, and TDS the client
    /// reverses comes off your TDS credit instead of going back as cash. Cash given back never
    /// exceeds cash received, and all notes on an invoice together take off at most its total.
    /// Returns the credit as recorded.
    pub fn apply_credit(
        &mut self,
        note: &CreditNote,
        terms: ReturnTerms,
    ) -> Result<Credit, DomainError> {
        if note.invoice() != &self.number || note.currency() != self.currency {
            return Err(DomainError::CreditForAnotherInvoice);
        }
        if note.date() < self.date {
            return Err(DomainError::CreditBeforeInvoice);
        }
        let amount = note.amounts().total.clone();
        let remaining = self.creditable();
        if amount > remaining {
            return Err(DomainError::CreditExceedsInvoice {
                remaining: remaining.to_string(),
                got: amount.to_string(),
            });
        }
        let excess = &amount - self.balance();
        let returned = (excess > BigDecimal::zero()).then(|| Returned {
            amount: excess,
            kind: terms.kind,
            refunded_inr: match terms.kind {
                ReturnKind::Refunded => terms.refunded_inr.map(round),
                ReturnKind::Advance => None,
            },
            tds_reversed: terms.tds_reversed.map(round),
        });
        let credit = Credit {
            number: note.number().clone(),
            date: note.date(),
            amount,
            returned,
        };
        self.credits.push(credit.clone());
        if let Err(error) = check_returns(self.currency, &self.payments, &self.credits) {
            self.credits.pop();
            return Err(error);
        }
        Ok(credit)
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

/// An invoice bills at least one milestone, each once, and its lines add up to its taxable value
fn check_lines(lines: &[InvoiceLine], amounts: &InvoiceAmounts) -> Result<(), DomainError> {
    if lines.is_empty() {
        return Err(DomainError::NoInvoiceLines);
    }
    for (i, line) in lines.iter().enumerate() {
        require(&line.milestone_name, "milestone name")?;
        if lines[..i]
            .iter()
            .any(|l| l.milestone_id == line.milestone_id)
        {
            return Err(DomainError::DuplicateInvoiceLine);
        }
    }
    let sum: BigDecimal = lines.iter().map(|l| &l.taxable_value).sum();
    if sum != amounts.taxable_value {
        return Err(DomainError::LinesDontAddUp {
            lines: sum.to_string(),
            taxable: amounts.taxable_value.to_string(),
        });
    }
    Ok(())
}

fn milestone_names(lines: &[InvoiceLine]) -> String {
    lines
        .iter()
        .map(|l| l.milestone_name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
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

/// Everything credit notes gave back on one invoice: each return on its own, TDS reversed no more
/// than was withheld, and cash given back no more than was received
fn check_returns(
    currency: Currency,
    payments: &[Payment],
    credits: &[Credit],
) -> Result<(), DomainError> {
    let returns = credits.iter().filter_map(|c| c.returned.as_ref());
    let mut reversed = BigDecimal::zero();
    let mut cash = BigDecimal::zero();
    for returned in returns {
        check_return(currency, returned)?;
        if let Some(tds) = &returned.tds_reversed {
            reversed += tds;
        }
        cash += returned.cash();
    }
    let withheld: BigDecimal = payments.iter().map(|p| &p.tds).sum();
    if reversed > withheld {
        return Err(DomainError::TdsReversedExceedsWithheld {
            withheld: withheld.to_string(),
            got: reversed.to_string(),
        });
    }
    let received: BigDecimal = payments.iter().map(|p| &p.received).sum();
    if cash > received {
        return Err(DomainError::ReturnExceedsReceived {
            received: received.to_string(),
            got: cash.to_string(),
        });
    }
    Ok(())
}

/// A refund on a foreign invoice says the rupees that left the bank; nothing else carries them.
/// TDS reversed is a positive part of a rupee invoice's return.
fn check_return(currency: Currency, returned: &Returned) -> Result<(), DomainError> {
    if let Some(tds) = &returned.tds_reversed
        && (!currency.is_inr() || *tds <= BigDecimal::zero() || *tds > returned.amount)
    {
        return Err(DomainError::UnexpectedTdsReversed);
    }
    match (returned.kind, currency.is_inr(), &returned.refunded_inr) {
        (ReturnKind::Refunded, false, None) => Err(DomainError::MissingRefundedAmount),
        (ReturnKind::Refunded, false, Some(inr)) if *inr <= BigDecimal::zero() => {
            Err(DomainError::MissingRefundedAmount)
        }
        (ReturnKind::Refunded, true, Some(_)) | (ReturnKind::Advance, _, Some(_)) => {
            Err(DomainError::UnexpectedRefundedAmount)
        }
        _ => Ok(()),
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
    /// The milestones billed, joined: `Design, Build`
    #[serde(alias = "milestone_name")]
    milestones: Box<str>,
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

    /// The milestones billed, joined: `Design, Build`
    pub fn milestones(&self) -> &str {
        &self.milestones
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
            milestones: milestone_names(&invoice.lines).into(),
            currency: invoice.currency,
            exchange_rate: invoice.exchange_rate.clone(),
            total: invoice.amounts.total.clone(),
            balance: invoice.balance(),
            status: invoice.status(),
        }
    }
}
