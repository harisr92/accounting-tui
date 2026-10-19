//! Turning a correction to an issued invoice into a credit note and its ledger entries. Pure: no
//! storage.

use accounting_core::invoice::credit_note_deadline;
use accounting_core::invoice::print::PRINT_DATE_FORMAT;
use accounting_core::invoice::{PrintRow, TaxLine};
use accounting_core::{Entry, GstDocument, GstLineItem, HsnMaster, validate_credit_note};
use bigdecimal::{BigDecimal, RoundingMode, Zero};
use chrono::NaiveDate;

use super::chart;
use super::invoice_builder::InvoicingError;
use crate::domain::{
    CreditNote, CreditNoteParts, Currency, DomainError, Invoice, InvoiceAmounts, InvoiceKind,
    InvoiceNumber, ReturnTerms,
};

const TITLE: &str = "Credit Note";

/// A built, not yet saved, credit note
#[derive(Debug, Clone)]
pub struct BuiltCreditNote {
    pub note: CreditNote,
    /// The balanced ledger posting: the invoice's legs reversed
    pub entries: Vec<Entry>,
    /// Compliance warnings that don't block issuing
    pub warnings: Vec<String>,
}

/// What a credit note takes back, as entered
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreditDraft {
    pub date: NaiveDate,
    /// Value taken back before GST, in the invoice's currency; GST on it is added at the
    /// invoice's rate
    pub taxable_value: BigDecimal,
    pub reason: String,
    /// What happens to the part of the note the client already paid: refunded or kept as an
    /// advance, less any TDS the client reverses. Ignored when the note takes off only what is
    /// due.
    pub returned: ReturnTerms,
}

/// Build credit note `number` against `invoice`. Notes on GST tax invoices go through
/// accounting-core; notes on export and plain invoices carry no GST, so compter lays them out.
/// Either way the note is at the invoice's exchange rate, and prints as the invoice did.
pub fn build(
    invoice: &Invoice,
    number: InvoiceNumber,
    draft: CreditDraft,
) -> Result<BuiltCreditNote, InvoicingError> {
    let taxable = draft
        .taxable_value
        .with_scale_round(2, RoundingMode::HalfUp);
    if taxable <= BigDecimal::zero() {
        return Err(DomainError::NonPositiveAmount.into());
    }
    // Section 34(2): the last day is 30 November after the invoice's financial year. Plain
    // invoices are outside GST.
    if invoice.kind() != InvoiceKind::Plain {
        let deadline = credit_note_deadline(invoice.date());
        if draft.date > deadline {
            return Err(InvoicingError::CreditNoteTooLate(deadline));
        }
    }
    let (amounts, gst, entries, warnings) = match invoice.kind() {
        InvoiceKind::Tax => gst_credit(invoice, &number, &draft, taxable)?,
        InvoiceKind::Export | InvoiceKind::Plain => {
            let amounts = InvoiceAmounts::untaxed(taxable);
            // A note that clears the balance takes the rest of the receivable, so no paisa of
            // rounding at the invoice's rate stays behind
            let inr = if amounts.total == invoice.balance() {
                invoice.receivable_inr()
            } else {
                invoice.to_inr(&amounts.total)
            };
            let entries = vec![
                Entry::debit(chart::CREDIT_NOTES, inr.clone(), None),
                Entry::credit(chart::RECEIVABLE, inr, None),
            ];
            (amounts, None, entries, Vec::new())
        }
    };
    let print = print(invoice, &number, &draft, &amounts);
    let note = CreditNote::issue(CreditNoteParts {
        number,
        date: draft.date,
        invoice: invoice.number().clone(),
        kind: invoice.kind(),
        client_name: invoice.client_name().to_owned(),
        terms: terms(invoice, &draft),
        reason: draft.reason,
        currency: invoice.currency(),
        exchange_rate: invoice.exchange_rate().cloned(),
        amounts,
        print,
        gst,
        logo: invoice.logo(),
    })?;
    Ok(BuiltCreditNote {
        note,
        entries,
        warnings,
    })
}

type GstCredit = (
    InvoiceAmounts,
    Option<accounting_core::CreditNote>,
    Vec<Entry>,
    Vec<String>,
);

/// A GST credit note: one line at the invoice line's SAC and rate
fn gst_credit(
    invoice: &Invoice,
    number: &InvoiceNumber,
    draft: &CreditDraft,
    taxable: BigDecimal,
) -> Result<GstCredit, InvoicingError> {
    let original = invoice.gst().ok_or(InvoicingError::MissingGstRecord)?;
    let line = original
        .line_items
        .first()
        .ok_or(InvoicingError::MissingGstRecord)?;
    let item = GstLineItem::new(
        line.hsn_sac.as_str(),
        description(invoice, draft),
        BigDecimal::from(1),
        taxable,
        line.gst_rate.clone(),
    )?;
    let note = accounting_core::CreditNote::new(original, number.as_str(), draft.date, vec![item])?;
    let report = validate_credit_note(&note, draft.date, HsnMaster::global());
    let errors: Vec<String> = report.errors().map(ToString::to_string).collect();
    if !errors.is_empty() {
        return Err(InvoicingError::NotCompliant(errors));
    }
    let warnings = report.warnings().map(ToString::to_string).collect();
    let totals = note.breakdown()?;
    let entries = note.to_entries(&chart::credit_note_accounts())?;
    let amounts = InvoiceAmounts {
        taxable_value: totals.taxable_value,
        cgst: totals.cgst,
        sgst: totals.sgst,
        igst: totals.igst,
        total: totals.total,
    };
    Ok((amounts, Some(note), entries, warnings))
}

/// `Website - Design: rate corrected`
fn description(invoice: &Invoice, draft: &CreditDraft) -> String {
    let billed = invoice
        .print()
        .rows
        .first()
        .map_or(invoice.milestone_name(), |row| row.description.as_str());
    format!("{billed}: {}", draft.reason.trim())
}

/// The invoice's print, with the note's number, date, line and totals
fn print(
    invoice: &Invoice,
    number: &InvoiceNumber,
    draft: &CreditDraft,
    amounts: &InvoiceAmounts,
) -> accounting_core::InvoicePrint {
    let currency = invoice.currency();
    let format = |amount: &BigDecimal| currency.format(amount);
    let original = invoice.print();
    let row = original.rows.first();
    let tax_lines = match invoice.kind() {
        InvoiceKind::Tax => [
            ("CGST", &amounts.cgst),
            ("SGST", &amounts.sgst),
            ("IGST", &amounts.igst),
        ]
        .into_iter()
        .filter(|(_, amount)| !amount.is_zero())
        .map(|(label, amount)| TaxLine {
            label: label.to_owned(),
            amount: format(amount),
        })
        .collect(),
        // An export keeps its "IGST @ 0%" line, at zero
        InvoiceKind::Export | InvoiceKind::Plain => original
            .tax_lines
            .iter()
            .map(|line| TaxLine {
                label: line.label.clone(),
                amount: format(&BigDecimal::zero()),
            })
            .collect(),
    };
    let total_tax = amounts.total_tax();
    accounting_core::InvoicePrint {
        title: TITLE.to_owned(),
        invoice_number: number.to_string(),
        invoice_date: draft.date.format(PRINT_DATE_FORMAT).to_string(),
        rows: vec![PrintRow {
            serial: 1,
            hsn_sac: row.map(|r| r.hsn_sac.clone()).unwrap_or_default(),
            description: description(invoice, draft),
            quantity: "1".to_owned(),
            rate: format(&amounts.taxable_value),
            taxable_value: format(&amounts.taxable_value),
            gst_rate: row.map_or_else(|| "-".to_owned(), |r| r.gst_rate.clone()),
            tax: format(&total_tax),
            amount: format(&amounts.total),
        }],
        taxable_value: format(&amounts.taxable_value),
        tax_lines,
        total_tax: format(&total_tax),
        total: format(&amounts.total),
        total_in_words: currency.in_words(&amounts.total),
        ..original.clone()
    }
}

/// Which invoice the note corrects and why, printed at the end
fn terms(invoice: &Invoice, draft: &CreditDraft) -> Vec<String> {
    let mut terms = vec![
        format!(
            "Credit note against invoice {} dated {}",
            invoice.number(),
            invoice.date().format(PRINT_DATE_FORMAT)
        ),
        format!("Reason: {}", draft.reason.trim()),
    ];
    if let Some(rate) = invoice.exchange_rate() {
        let code = invoice.currency().code();
        terms.push(format!(
            "Amounts in {code} at the invoice's rate: 1 {code} = {} {rate}",
            Currency::Inr.pdf_label()
        ));
    }
    terms
}
