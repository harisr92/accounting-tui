//! Quarterly GSTR-1 (QRMP). accounting-core builds one month at a time, so the quarter is three
//! monthly returns merged, plus the sections it doesn't cover: exports (table 6A) and a
//! document summary (table 13) that includes them.

use std::collections::BTreeMap;

use accounting_core::returns::{
    B2csRow, DocIssue, DocSeries, DocSummary, Gstr1Return, HsnRow, NilRow,
    OUTWARD_INVOICES_DOC_TYPE, ReturnPeriod, SERVICES_UQC,
};
use accounting_core::{Gstin, Gstr1Error, HsnMaster};
use bigdecimal::BigDecimal;
use serde_json::{Value, json};

use crate::domain::{Invoice, InvoiceKind, Quarter};

/// Table 13 serial number for outward invoices
const OUTWARD_INVOICES_DOC_NUM: usize = 1;

/// A quarter's return as GST portal JSON, with what went into it
#[derive(Debug, Clone)]
pub struct QuarterlyGstr1 {
    pub json: Value,
    pub tax_invoices: usize,
    pub exports: usize,
}

/// Build the GSTR-1 for `quarter` from its tax and export invoices (plain ones don't belong)
pub fn quarterly_gstr1(
    filer: &Gstin,
    quarter: Quarter,
    invoices: &[Invoice],
    master: &HsnMaster,
) -> Result<QuarterlyGstr1, Gstr1Error> {
    let in_quarter = |kind| {
        invoices
            .iter()
            .filter(move |i| i.kind() == kind && quarter.contains(i.date()))
    };
    let [first, second, last] = quarter.months();
    let mut merged =
        Gstr1Return::build(filer, ReturnPeriod::new(last.0, last.1)?, &[], &[], master)?;
    let mut tax_invoices = 0;
    for (year, month) in [first, second, last] {
        let period = ReturnPeriod::new(year, month)?;
        let month_invoices: Vec<_> = in_quarter(InvoiceKind::Tax)
            .filter(|i| period.contains(i.date()))
            .filter_map(|i| i.gst().cloned())
            .collect();
        if month_invoices.is_empty() {
            continue;
        }
        tax_invoices += month_invoices.len();
        let month_return = Gstr1Return::build(filer, period, &month_invoices, &[], master)?;
        merge(&mut merged, month_return);
    }

    let exports: Vec<&Invoice> = in_quarter(InvoiceKind::Export).collect();
    add_export_hsn(&mut merged, &exports, master);
    let billed: Vec<&Invoice> = in_quarter(InvoiceKind::Tax)
        .chain(exports.iter().copied())
        .collect();
    merged.doc_issue = doc_issue(&billed);
    renumber(&mut merged.hsn.b2b);
    renumber(&mut merged.hsn.b2c);

    let mut json = serde_json::to_value(&merged)?;
    if !exports.is_empty() {
        json["exp"] = export_section(&exports);
    }
    Ok(QuarterlyGstr1 {
        json,
        tax_invoices,
        exports: exports.len(),
    })
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

/// Exports in the HSN summary at 0%, reported with supplies to unregistered recipients
fn add_export_hsn(merged: &mut Gstr1Return, exports: &[&Invoice], master: &HsnMaster) {
    let rows = exports.iter().filter_map(|invoice| {
        let row = invoice.print().rows.first()?;
        let taxable = invoice.amounts().taxable_value.clone();
        Some(HsnRow {
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
        })
    });
    merge_hsn(&mut merged.hsn.b2c, rows.collect());
}

/// Table 13: one series per prefix and year, from the first to the last number issued
fn doc_issue(invoices: &[&Invoice]) -> DocIssue {
    let mut series: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    for invoice in invoices {
        let number = invoice.number().as_str();
        let prefix = number.rsplit_once('/').map_or(number, |(prefix, _)| prefix);
        series.entry(prefix.to_owned()).or_default().push(number);
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
    let documents = if series.is_empty() {
        Vec::new()
    } else {
        vec![DocSummary {
            doc_num: OUTWARD_INVOICES_DOC_NUM,
            doc_type: OUTWARD_INVOICES_DOC_TYPE.to_owned(),
            series,
        }]
    };
    DocIssue { documents }
}

/// Table 6A: exports of services under LUT, without payment of IGST
fn export_section(exports: &[&Invoice]) -> Value {
    let number = |amount: &BigDecimal| -> Value {
        serde_json::from_str(
            &amount
                .with_scale_round(2, bigdecimal::RoundingMode::HalfUp)
                .to_string(),
        )
        .unwrap_or(Value::Null)
    };
    let invoices: Vec<Value> = exports
        .iter()
        .map(|invoice| {
            let taxable = &invoice.amounts().taxable_value;
            json!({
                "inum": invoice.number().as_str(),
                "idt": invoice.date().format("%d-%m-%Y").to_string(),
                "val": number(&invoice.amounts().total),
                "itms": [{ "txval": number(taxable), "rt": 0, "iamt": 0, "csamt": 0 }],
            })
        })
        .collect();
    json!([{ "exp_typ": "WOPAY", "inv": invoices }])
}
