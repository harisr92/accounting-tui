//! Quarterly GSTR-1 (QRMP), and the monthly IFF. accounting-core builds one month at a time, so
//! the quarter is three monthly returns merged, plus the sections it doesn't cover: exports
//! (table 6A), credit notes on exports (9B `cdnur`, `EXPWOP`) and a document summary (table 13)
//! that includes them.
//!
//! Under QRMP the B2B invoices of a quarter's first two months may be furnished early through
//! the IFF. Those months' B2B invoices and credit notes are then left out of the quarter's
//! return; they still count in its HSN summary and table 13.

use std::collections::BTreeMap;

use accounting_core::returns::{
    B2csRow, CREDIT_NOTES_DOC_TYPE, DocIssue, DocSeries, DocSummary, Gstr1Return, HsnRow, NilRow,
    OUTWARD_INVOICES_DOC_TYPE, ReturnPeriod, SERVICES_UQC,
};
use accounting_core::{Gstin, Gstr1Error, HsnMaster};
use bigdecimal::BigDecimal;
use chrono::{Datelike, NaiveDate};
use serde_json::{Value, json};

use crate::domain::{CreditNote, Invoice, InvoiceKind, Quarter};

/// Table 13 serial numbers for outward invoices and credit notes
const OUTWARD_INVOICES_DOC_NUM: usize = 1;
const CREDIT_NOTES_DOC_NUM: usize = 5;

/// A quarter's return as GST portal JSON, with what went into it
#[derive(Debug, Clone)]
pub struct QuarterlyGstr1 {
    pub json: Value,
    pub tax_invoices: usize,
    pub exports: usize,
    pub credit_notes: usize,
}

/// A month's IFF as GST portal JSON: its B2B invoices and the credit notes on B2B invoices
#[derive(Debug, Clone)]
pub struct MonthlyIff {
    pub json: Value,
    pub invoices: usize,
    pub credit_notes: usize,
}

/// Build the GSTR-1 for `quarter` from its tax and export invoices and the credit notes on them
/// (plain ones don't belong). The B2B part of the months in `furnished` went in an IFF.
pub fn quarterly_gstr1(
    filer: &Gstin,
    quarter: Quarter,
    invoices: &[Invoice],
    notes: &[CreditNote],
    furnished: &[(i32, u32)],
    master: &HsnMaster,
) -> Result<QuarterlyGstr1, Gstr1Error> {
    let in_quarter = |kind| {
        invoices
            .iter()
            .filter(move |i| i.kind() == kind && quarter.contains(i.date()))
    };
    let notes_in_quarter = |kind| {
        notes
            .iter()
            .filter(move |n| n.kind() == kind && quarter.contains(n.date()))
    };
    let [first, second, last] = quarter.months();
    let mut merged =
        Gstr1Return::build(filer, ReturnPeriod::new(last.0, last.1)?, &[], &[], master)?;
    for (year, month) in [first, second, last] {
        if let Some(mut built) = month_return(filer, (year, month), invoices, notes, master)? {
            if furnished.contains(&(year, month)) {
                built.b2b.clear();
                built.cdnr.clear();
            }
            merge(&mut merged, built);
        }
    }
    // What the return reports: B2B documents of furnished months went in their IFF
    let reported =
        |date: NaiveDate, b2b: bool| !(b2b && furnished.contains(&(date.year(), date.month())));
    let tax_invoices = in_quarter(InvoiceKind::Tax)
        .filter(|i| reported(i.date(), i.gst().is_some_and(|g| g.buyer.is_registered())))
        .count();
    let credit_notes = notes_in_quarter(InvoiceKind::Tax)
        .filter(|n| reported(n.date(), n.gst().is_some_and(|g| g.buyer.is_registered())))
        .count();

    let exports: Vec<&Invoice> = in_quarter(InvoiceKind::Export).collect();
    let export_notes: Vec<&CreditNote> = notes_in_quarter(InvoiceKind::Export).collect();
    add_export_hsn(&mut merged, &exports, &export_notes, master);
    let billed: Vec<&str> = in_quarter(InvoiceKind::Tax)
        .chain(exports.iter().copied())
        .map(|i| i.number().as_str())
        .collect();
    let credited: Vec<&str> = notes_in_quarter(InvoiceKind::Tax)
        .chain(export_notes.iter().copied())
        .map(|n| n.number().as_str())
        .collect();
    merged.doc_issue = doc_issue(&billed, &credited);
    renumber(&mut merged.hsn.b2b);
    renumber(&mut merged.hsn.b2c);

    let mut json = serde_json::to_value(&merged)?;
    if !exports.is_empty() {
        json["exp"] = export_section(&exports);
    }
    if !export_notes.is_empty() {
        let mut cdnur = json
            .get("cdnur")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        cdnur.extend(export_notes.iter().map(|note| export_note(note)));
        json["cdnur"] = Value::Array(cdnur);
    }
    Ok(QuarterlyGstr1 {
        json,
        tax_invoices,
        exports: exports.len(),
        credit_notes: credit_notes + export_notes.len(),
    })
}

/// The IFF for one month: its B2B invoices (table 4A) and credit notes on B2B invoices (9B)
pub fn monthly_iff(
    filer: &Gstin,
    month: (i32, u32),
    invoices: &[Invoice],
    notes: &[CreditNote],
    master: &HsnMaster,
) -> Result<MonthlyIff, Gstr1Error> {
    let period = ReturnPeriod::new(month.0, month.1)?;
    let built = month_return(filer, month, invoices, notes, master)?;
    let mut json = serde_json::json!({ "gstin": filer, "fp": period });
    if let Some(built) = built {
        let built = serde_json::to_value(&built)?;
        for section in ["b2b", "cdnr"] {
            if let Some(rows) = built.get(section) {
                json[section] = rows.clone();
            }
        }
    }
    let b2b =
        |gst: Option<&accounting_core::GstInvoice>| gst.is_some_and(|g| g.buyer.is_registered());
    let invoices = invoices
        .iter()
        .filter(|i| i.kind() == InvoiceKind::Tax && period.contains(i.date()) && b2b(i.gst()))
        .count();
    let credit_notes = notes
        .iter()
        .filter(|n| period.contains(n.date()) && n.gst().is_some_and(|g| g.buyer.is_registered()))
        .count();
    Ok(MonthlyIff {
        json,
        invoices,
        credit_notes,
    })
}

/// accounting-core's return for one month's tax invoices and the GST credit notes dated in it;
/// `None` for a month with neither
fn month_return(
    filer: &Gstin,
    (year, month): (i32, u32),
    invoices: &[Invoice],
    notes: &[CreditNote],
    master: &HsnMaster,
) -> Result<Option<Gstr1Return>, Gstr1Error> {
    let period = ReturnPeriod::new(year, month)?;
    let month_invoices: Vec<_> = invoices
        .iter()
        .filter(|i| i.kind() == InvoiceKind::Tax && period.contains(i.date()))
        .filter_map(|i| i.gst().cloned())
        .collect();
    let month_notes: Vec<_> = notes
        .iter()
        .filter(|n| period.contains(n.date()))
        .filter_map(|n| n.gst().cloned())
        .collect();
    if month_invoices.is_empty() && month_notes.is_empty() {
        return Ok(None);
    }
    Gstr1Return::build(filer, period, &month_invoices, &month_notes, master).map(Some)
}

/// Fold one month's return into the quarter's
fn merge(quarter: &mut Gstr1Return, month: Gstr1Return) {
    for party in month.b2b {
        match quarter
            .b2b
            .iter_mut()
            .find(|p| p.buyer_gstin == party.buyer_gstin)
        {
            Some(existing) => existing.invoices.extend(party.invoices),
            None => quarter.b2b.push(party),
        }
    }
    for place in month.b2cl {
        match quarter
            .b2cl
            .iter_mut()
            .find(|p| p.place_of_supply == place.place_of_supply)
        {
            Some(existing) => existing.invoices.extend(place.invoices),
            None => quarter.b2cl.push(place),
        }
    }
    for row in month.b2cs {
        match quarter.b2cs.iter_mut().find(|r| same_b2cs(r, &row)) {
            Some(existing) => add_b2cs(existing, &row),
            None => quarter.b2cs.push(row),
        }
    }
    for row in month.nil.rows {
        match quarter
            .nil
            .rows
            .iter_mut()
            .find(|r| r.supply_type == row.supply_type)
        {
            Some(existing) => add_nil(existing, &row),
            None => quarter.nil.rows.push(row),
        }
    }
    for party in month.cdnr {
        match quarter
            .cdnr
            .iter_mut()
            .find(|p| p.buyer_gstin == party.buyer_gstin)
        {
            Some(existing) => existing.notes.extend(party.notes),
            None => quarter.cdnr.push(party),
        }
    }
    quarter.cdnur.extend(month.cdnur);
    merge_hsn(&mut quarter.hsn.b2b, month.hsn.b2b);
    merge_hsn(&mut quarter.hsn.b2c, month.hsn.b2c);
}

fn same_b2cs(a: &B2csRow, b: &B2csRow) -> bool {
    a.supply_type == b.supply_type
        && a.rate == b.rate
        && a.b2cs_type == b.b2cs_type
        && a.place_of_supply == b.place_of_supply
}

fn add_optional(total: &mut Option<BigDecimal>, more: Option<&BigDecimal>) {
    if let Some(more) = more {
        *total = Some(total.take().unwrap_or_default() + more);
    }
}

fn add_b2cs(total: &mut B2csRow, row: &B2csRow) {
    total.taxable_value += &row.taxable_value;
    add_optional(&mut total.igst, row.igst.as_ref());
    add_optional(&mut total.cgst, row.cgst.as_ref());
    add_optional(&mut total.sgst, row.sgst.as_ref());
    total.cess += &row.cess;
}

fn add_nil(total: &mut NilRow, row: &NilRow) {
    total.nil_rated += &row.nil_rated;
    total.exempt += &row.exempt;
    total.non_gst += &row.non_gst;
}

fn merge_hsn(total: &mut Vec<HsnRow>, rows: Vec<HsnRow>) {
    for row in rows {
        let same =
            |r: &&mut HsnRow| r.hsn_sac == row.hsn_sac && r.rate == row.rate && r.uqc == row.uqc;
        match total.iter_mut().find(same) {
            Some(existing) => {
                existing.quantity += &row.quantity;
                existing.total_value += &row.total_value;
                existing.taxable_value += &row.taxable_value;
                existing.igst += &row.igst;
                existing.cgst += &row.cgst;
                existing.sgst += &row.sgst;
                existing.cess += &row.cess;
            }
            None => total.push(row),
        }
    }
}

fn renumber(rows: &mut [HsnRow]) {
    for (index, row) in rows.iter_mut().enumerate() {
        row.number = index + 1;
    }
}

/// Exports in the HSN summary at 0%, reported with supplies to unregistered recipients; credit
/// notes on exports take their value back out
fn add_export_hsn(
    merged: &mut Gstr1Return,
    exports: &[&Invoice],
    notes: &[&CreditNote],
    master: &HsnMaster,
) {
    let invoices = exports.iter().filter_map(|invoice| {
        let row = invoice.print().rows.first()?;
        Some((row, invoice.inr_amounts().taxable_value))
    });
    let notes = notes.iter().filter_map(|note| {
        let row = note.print().rows.first()?;
        Some((row, -note.inr_amounts().taxable_value))
    });
    let rows = invoices.chain(notes).map(|(row, taxable)| HsnRow {
        number: 0,
        hsn_sac: row.hsn_sac.clone(),
        description: master
            .lookup(&row.hsn_sac)
            .map_or_else(|| row.description.clone(), |e| e.description.clone()),
        uqc: SERVICES_UQC.to_owned(),
        quantity: BigDecimal::from(0),
        total_value: taxable.clone(),
        taxable_value: taxable,
        igst: BigDecimal::from(0),
        cgst: BigDecimal::from(0),
        sgst: BigDecimal::from(0),
        cess: BigDecimal::from(0),
        rate: BigDecimal::from(0),
    });
    merge_hsn(&mut merged.hsn.b2c, rows.collect());
}

/// Table 13: invoices and credit notes, each with one series per prefix and year, from the first
/// to the last number issued
fn doc_issue(invoices: &[&str], notes: &[&str]) -> DocIssue {
    let documents = [
        doc_summary(
            OUTWARD_INVOICES_DOC_NUM,
            OUTWARD_INVOICES_DOC_TYPE,
            invoices,
        ),
        doc_summary(CREDIT_NOTES_DOC_NUM, CREDIT_NOTES_DOC_TYPE, notes),
    ]
    .into_iter()
    .flatten()
    .collect();
    DocIssue { documents }
}

fn doc_summary(doc_num: usize, doc_type: &str, numbers: &[&str]) -> Option<DocSummary> {
    let mut series: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for number in numbers {
        let prefix = number
            .rsplit_once('/')
            .map_or(*number, |(prefix, _)| prefix);
        series.entry(prefix).or_default().push(number);
    }
    let series: Vec<DocSeries> = series
        .into_values()
        .enumerate()
        .filter_map(|(index, mut numbers)| {
            numbers.sort_unstable();
            Some(DocSeries {
                number: index + 1,
                from: (*numbers.first()?).to_owned(),
                to: (*numbers.last()?).to_owned(),
                total: numbers.len(),
                cancelled: 0,
                net_issued: numbers.len(),
            })
        })
        .collect();
    (!series.is_empty()).then(|| DocSummary {
        doc_num,
        doc_type: doc_type.to_owned(),
        series,
    })
}

/// Returns are in rupees, two decimals, as JSON numbers
fn portal_number(amount: &BigDecimal) -> Value {
    serde_json::from_str(
        &amount
            .with_scale_round(2, bigdecimal::RoundingMode::HalfUp)
            .to_string(),
    )
    .unwrap_or(Value::Null)
}

/// Table 9B `cdnur`: a credit note on an export without payment of IGST
fn export_note(note: &CreditNote) -> Value {
    let amounts = note.inr_amounts();
    json!({
        "typ": "EXPWOP",
        "ntty": "C",
        "nt_num": note.number().as_str(),
        "nt_dt": note.date().format("%d-%m-%Y").to_string(),
        "val": portal_number(&amounts.total),
        "itms": [{
            "num": 1,
            "itm_det": { "rt": 0, "txval": portal_number(&amounts.taxable_value), "iamt": 0, "csamt": 0 },
        }],
    })
}

/// Table 6A: exports of services under LUT, without payment of IGST
fn export_section(exports: &[&Invoice]) -> Value {
    let number = portal_number;
    let invoices: Vec<Value> = exports
        .iter()
        .map(|invoice| {
            // Returns are in rupees, at the rate on the invoice date
            let amounts = invoice.inr_amounts();
            json!({
                "inum": invoice.number().as_str(),
                "idt": invoice.date().format("%d-%m-%Y").to_string(),
                "val": number(&amounts.total),
                "itms": [{ "txval": number(&amounts.taxable_value), "rt": 0, "iamt": 0, "csamt": 0 }],
            })
        })
        .collect();
    json!([{ "exp_typ": "WOPAY", "inv": invoices }])
}
