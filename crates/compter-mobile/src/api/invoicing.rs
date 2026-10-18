use bigdecimal::Zero;
use compter_core::domain::{Currency, Invoice, InvoiceSummary};
use compter_core::services::{CreditPreview, InvoicePreview};

use super::ProjectDetail;

/// The Invoices list: every issued invoice, newest first
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvoiceList {
    /// Still due across all invoices, in rupees (foreign invoices at their rate)
    pub outstanding: String,
    pub invoices: Vec<InvoiceRow>,
}

/// One line of the Invoices list, from the stored summary
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvoiceRow {
    pub number: String,
    /// `YYYY-MM-DD`
    pub date: String,
    pub client_name: String,
    pub milestone_name: String,
    /// `TAX`, `EXPORT` or `PLAIN`
    pub kind: String,
    pub total: String,
    /// `None` once nothing is left to pay
    pub due: Option<String>,
    /// `ISSUED`, `PART PAID`, `PAID` or `CREDITED`
    pub status: String,
}

impl From<&InvoiceSummary> for InvoiceRow {
    fn from(summary: &InvoiceSummary) -> Self {
        let currency = summary.currency();
        Self {
            number: summary.number().to_string(),
            date: summary.date().to_string(),
            client_name: summary.client_name().to_owned(),
            milestone_name: summary.milestone_name().to_owned(),
            kind: summary.kind().label().to_owned(),
            total: currency.display(summary.total()),
            due: (!summary.is_settled()).then(|| currency.display(summary.balance())),
            status: summary.status().label().to_owned(),
        }
    }
}

/// An opened invoice with what has been paid and credited against it
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvoiceDetail {
    pub number: String,
    /// `YYYY-MM-DD`
    pub date: String,
    pub kind: String,
    pub client_name: String,
    pub milestone_name: String,
    pub currency: String,
    /// `₹83.50 per USD`, for an invoice in a foreign currency
    pub exchange_rate: Option<String>,
    pub total: String,
    /// `50,000.00 + IGST 9,000.00`
    pub breakdown: String,
    /// The total in rupees, for an invoice in a foreign currency
    pub total_inr: Option<String>,
    pub balance: String,
    pub status: String,
    /// Nothing is left to pay: no payment or credit note can be added
    pub settled: bool,
    /// Payments ask for the rupees banked instead of TDS
    pub foreign: bool,
    pub history: Vec<HistoryRow>,
    /// To prefill the payment form: the whole balance, as a plain number
    pub balance_value: String,
    /// The TDS a client usually withholds, as a hint on rupee invoices
    pub usual_tds: Option<String>,
    /// To prefill the credit note form: the whole taxable value, while nothing has been paid
    /// or credited (cancelling the invoice); otherwise left for the user
    pub credit_value: Option<String>,
}

impl From<&Invoice> for InvoiceDetail {
    fn from(invoice: &Invoice) -> Self {
        let currency = invoice.currency();
        let amounts = invoice.amounts();
        let foreign = !currency.is_inr();
        let payments = invoice.payments().iter().map(|p| HistoryRow {
            date: p.date.to_string(),
            document: "Payment".to_owned(),
            credit_note: false,
            received: Some(currency.display(&p.received)),
            // Foreign clients withhold no TDS; show the rupees banked instead
            tds_or_banked: Some(if foreign {
                Currency::Inr.display(p.received_inr())
            } else {
                currency.display(&p.tds)
            }),
            settled: currency.display(&p.settled()),
        });
        let credits = invoice.credits().iter().map(|c| HistoryRow {
            date: c.date.to_string(),
            document: c.number.to_string(),
            credit_note: true,
            received: None,
            tds_or_banked: None,
            settled: currency.display(&c.amount),
        });
        let untouched = invoice.payments().is_empty() && invoice.credits().is_empty();
        Self {
            number: invoice.number().to_string(),
            date: invoice.date().to_string(),
            kind: invoice.kind().label().to_owned(),
            client_name: invoice.client_name().to_owned(),
            milestone_name: invoice.milestone_name().to_owned(),
            currency: currency.code().to_owned(),
            exchange_rate: invoice
                .exchange_rate()
                .map(|rate| format!("₹{rate} per {currency}")),
            total: currency.display(&amounts.total),
            breakdown: amounts.breakdown(invoice.kind(), currency),
            total_inr: invoice
                .exchange_rate()
                .map(|_| Currency::Inr.display(&invoice.inr_amounts().total)),
            balance: currency.display(&invoice.balance()),
            status: invoice.status().label().to_owned(),
            settled: invoice.is_settled(),
            foreign,
            history: payments.chain(credits).collect(),
            balance_value: invoice.balance().to_string(),
            usual_tds: (!foreign).then(|| currency.display(&invoice.usual_tds())),
            credit_value: untouched.then(|| amounts.taxable_value.to_string()),
        }
    }
}

/// A payment received or a credit note issued against an invoice
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryRow {
    /// `YYYY-MM-DD`
    pub date: String,
    /// `Payment`, or the credit note's number
    pub document: String,
    pub credit_note: bool,
    /// Payments only: what the client paid, in the invoice's currency
    pub received: Option<String>,
    /// Payments only: TDS withheld on a rupee invoice, rupees banked on a foreign one
    pub tds_or_banked: Option<String>,
    /// How much of the invoice this cleared
    pub settled: String,
}

/// The invoice a milestone would get today, shown for confirmation before issuing
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvoicePreviewRow {
    pub number: String,
    pub client_name: String,
    pub kind: String,
    pub total: String,
    pub breakdown: String,
    pub total_inr: Option<String>,
    pub warnings: Vec<String>,
}

impl From<&InvoicePreview> for InvoicePreviewRow {
    fn from(preview: &InvoicePreview) -> Self {
        let currency = preview.currency;
        Self {
            number: preview.number.to_string(),
            client_name: preview.client_name.clone(),
            kind: preview.kind.label().to_owned(),
            total: currency.display(&preview.amounts.total),
            breakdown: preview.amounts.breakdown(preview.kind, currency),
            total_inr: preview
                .total_inr
                .as_ref()
                .map(|inr| Currency::Inr.display(inr)),
            warnings: preview.warnings.clone(),
        }
    }
}

/// What issuing did: the invoice, the project with its milestone now invoiced, and whether
/// the PDF was written
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuedRow {
    pub invoice: InvoiceDetail,
    pub project: ProjectDetail,
    /// The invoice is issued either way; only writing its file failed
    pub pdf_error: Option<String>,
}

/// A credit note as it would be issued, shown for confirmation
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreditPreviewRow {
    pub number: String,
    pub invoice: String,
    pub total: String,
    /// GST included in the total, when there is any
    pub gst: Option<String>,
    /// What the client still owes after the note
    pub balance_after: String,
    pub warnings: Vec<String>,
}

impl From<&CreditPreview> for CreditPreviewRow {
    fn from(preview: &CreditPreview) -> Self {
        let currency = preview.currency;
        let tax = preview.amounts.total_tax();
        Self {
            number: preview.number.to_string(),
            invoice: preview.invoice.to_string(),
            total: currency.display(&preview.amounts.total),
            gst: (!tax.is_zero()).then(|| currency.display(&tax)),
            balance_after: currency.display(&preview.balance_after),
            warnings: preview.warnings.clone(),
        }
    }
}

/// What issuing a credit note did
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreditedRow {
    pub note_number: String,
    pub invoice: InvoiceDetail,
    pub pdf_error: Option<String>,
}

/// A payment as typed. Amounts go to Rust unparsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaymentInput {
    /// `YYYY-MM-DD`; blank means today
    pub date: String,
    /// In the invoice's currency
    pub received: String,
    /// TDS withheld on a rupee invoice; rupees banked on a foreign one
    pub tds_or_banked: String,
}

/// A credit note as typed
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreditInput {
    /// `YYYY-MM-DD`; blank means today
    pub date: String,
    /// Value taken back before GST, in the invoice's currency
    pub amount: String,
    pub reason: String,
}

/// Time worked on an hourly milestone, as typed
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeInput {
    /// `YYYY-MM-DD`; blank means today
    pub date: String,
    /// `1h30m`, `45m`, `1:30` or `1.5` (hours)
    pub duration: String,
    pub note: String,
}
