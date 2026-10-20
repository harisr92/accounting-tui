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
    CreditLine, CreditNote, CreditNoteParts, Currency, DomainError, Invoice, InvoiceAmounts,
    InvoiceKind, InvoiceLine, InvoiceNumber, MilestoneId, ReturnTerms,
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

/// What a credit note takes back from one invoice line, as entered
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreditLineDraft {
    /// The milestone the line billed
    pub milestone_id: MilestoneId,
    /// Value taken back before GST, in the invoice's currency; GST on it is added at the line's
    /// rate
    pub taxable_value: BigDecimal,
}

/// What a credit note takes back, as entered
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreditDraft {
    pub date: NaiveDate,
    /// The invoice lines taken back from, each once
    pub lines: Vec<CreditLineDraft>,
    pub reason: String,
    /// What happens to the part of the note the client already paid: refunded or kept as an
    /// advance, less any TDS the client reverses. Ignored when the note takes off only what is
    /// due.
    pub returned: ReturnTerms,
}

/// A draft line matched to the invoice line it takes back from
struct Taken<'a> {
    /// Position of the line on the invoice: its print row and GST line item
    position: usize,
    line: &'a InvoiceLine,
    taxable: BigDecimal,
}

/// Build credit note `number` against `invoice`. Notes on GST tax invoices go through
/// accounting-core; notes on export and plain invoices carry no GST, so compter lays them out.
/// Either way the note is at the invoice's exchange rate, and prints as the invoice did, a row
/// per line taken back from.
pub fn build(
    invoice: &Invoice,
    number: InvoiceNumber,
    draft: CreditDraft,
) -> Result<BuiltCreditNote, InvoicingError> {
    let taken = match_lines(invoice, &draft.lines)?;
    // Section 34(2): the last day is 30 November after the invoice's financial year. Plain
    // invoices are outside GST.
    if invoice.kind() != InvoiceKind::Plain {
        let deadline = credit_note_deadline(invoice.date());
        if draft.date > deadline {
            return Err(InvoicingError::CreditNoteTooLate(deadline));
        }
    }
    let (amounts, gst, entries, warnings) = match invoice.kind() {
        InvoiceKind::Tax => gst_credit(invoice, &number, &draft, &taken)?,
        InvoiceKind::Export | InvoiceKind::Plain => {
            let amounts = InvoiceAmounts::untaxed(taken.iter().map(|t| &t.taxable).sum());
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
    let rows = rows(invoice, &draft, &taken, gst.as_ref())?;
    let print = print(invoice, &number, &draft, &amounts, rows);
    let lines = taken
        .iter()
        .map(|t| CreditLine {
            milestone_id: t.line.milestone_id,
            taxable_value: t.taxable.clone(),
        })
        .collect();
    let note = CreditNote::issue(CreditNoteParts {
        number,
        date: draft.date,
        invoice: invoice.number().clone(),
        kind: invoice.kind(),
        client_name: invoice.client_name().to_owned(),
        terms: terms(invoice, &draft),
        reason: draft.reason,
        lines,
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

/// Each draft line on the invoice, once, for more than zero (rounded to paise), in the
/// invoice's order
fn match_lines<'a>(
    invoice: &'a Invoice,
    drafts: &[CreditLineDraft],
) -> Result<Vec<Taken<'a>>, InvoicingError> {
    if drafts.is_empty() {
        return Err(DomainError::NoCreditLines.into());
    }
    let mut taken = Vec::with_capacity(drafts.len());
    for (i, draft) in drafts.iter().enumerate() {
        if drafts[..i]
            .iter()
            .any(|d| d.milestone_id == draft.milestone_id)
        {
            return Err(DomainError::DuplicateCreditLine.into());
        }
        let (position, line) = invoice
            .lines()
            .iter()
            .enumerate()
            .find(|(_, line)| line.milestone_id == draft.milestone_id)
            .ok_or(DomainError::NotOnInvoice)?;
        let taxable = credit_amount(&draft.taxable_value)?;
        taken.push(Taken {
            position,
            line,
            taxable,
        });
    }
    taken.sort_by_key(|t| t.position);
    Ok(taken)
}

/// What a credit note line takes off, before GST: more than zero and in whole paise. A fraction
/// of a paisa is refused rather than rounded, so a mistyped amount is never taken as another.
pub(super) fn credit_amount(amount: &BigDecimal) -> Result<BigDecimal, DomainError> {
    let paise = amount.with_scale_round(2, RoundingMode::HalfUp);
    if paise != *amount {
        return Err(DomainError::FractionOfPaisa);
    }
    if paise <= BigDecimal::zero() {
        return Err(DomainError::NonPositiveAmount);
    }
    Ok(paise)
}

type GstCredit = (
    InvoiceAmounts,
    Option<accounting_core::CreditNote>,
    Vec<Entry>,
    Vec<String>,
);

/// A GST credit note: a line per invoice line taken back from, at that line's SAC and rate
fn gst_credit(
    invoice: &Invoice,
    number: &InvoiceNumber,
    draft: &CreditDraft,
    taken: &[Taken<'_>],
) -> Result<GstCredit, InvoicingError> {
    let original = invoice.gst().ok_or(InvoicingError::MissingGstRecord)?;
    let items = taken
        .iter()
        .map(|t| {
            let line = original
                .line_items
                .get(t.position)
                .ok_or(InvoicingError::MissingGstRecord)?;
            Ok(GstLineItem::new(
                line.hsn_sac.as_str(),
                description(invoice, t, draft),
                BigDecimal::from(1),
                t.taxable.clone(),
                line.gst_rate.clone(),
            )?)
        })
        .collect::<Result<Vec<_>, InvoicingError>>()?;
    let note = accounting_core::CreditNote::new(original, number.as_str(), draft.date, items)?;
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
fn description(invoice: &Invoice, taken: &Taken<'_>, draft: &CreditDraft) -> String {
    let billed = invoice
        .print()
        .rows
        .get(taken.position)
        .map_or(taken.line.milestone_name.as_str(), |row| {
            row.description.as_str()
        });
    format!("{billed}: {}", draft.reason.trim())
}

/// A printed row per line taken back from, with the invoice row's SAC and rate. Tax per row
/// comes from the GST note's own lines; untaxed notes have none.
fn rows(
    invoice: &Invoice,
    draft: &CreditDraft,
    taken: &[Taken<'_>],
    gst: Option<&accounting_core::CreditNote>,
) -> Result<Vec<PrintRow>, InvoicingError> {
    let currency = invoice.currency();
    let format = |amount: &BigDecimal| currency.format(amount);
    let inter_state = invoice.print().is_inter_state;
    taken
        .iter()
        .zip(1..)
        .map(|(t, serial)| {
            let row = invoice.print().rows.get(t.position);
            let item = gst.and_then(|note| note.line_items.get(serial - 1));
            let (taxable, tax) = match item {
                Some(item) => {
                    let line = item.breakdown(inter_state)?;
                    let tax = &line.total - &line.taxable_value;
                    (line.taxable_value, tax)
                }
                None => (t.taxable.clone(), BigDecimal::zero()),
            };
            Ok(PrintRow {
                serial,
                hsn_sac: row.map(|r| r.hsn_sac.clone()).unwrap_or_default(),
                description: description(invoice, t, draft),
                quantity: "1".to_owned(),
                rate: format(&taxable),
                taxable_value: format(&taxable),
                gst_rate: row.map_or_else(|| "-".to_owned(), |r| r.gst_rate.clone()),
                tax: format(&tax),
                amount: format(&(&taxable + &tax)),
            })
        })
        .collect()
}

/// The invoice's print, with the note's number, date, rows and totals
fn print(
    invoice: &Invoice,
    number: &InvoiceNumber,
    draft: &CreditDraft,
    amounts: &InvoiceAmounts,
    rows: Vec<PrintRow>,
) -> accounting_core::InvoicePrint {
    let currency = invoice.currency();
    let format = |amount: &BigDecimal| currency.format(amount);
    let original = invoice.print();
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
        rows,
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
