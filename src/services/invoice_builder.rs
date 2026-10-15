//! Turning a milestone into an invoice document and its ledger entries. Pure: no storage.

use accounting_core::invoice::print::PRINT_DATE_FORMAT;
use accounting_core::invoice::{PrintRow, Recipient, TAX_INVOICE_TITLE, TaxLine};
use accounting_core::utils::formatting::{amount_in_words, format_inr};
use accounting_core::{
    Entry, GstInvoice, GstLineItem, HsnMaster, InvoiceParties, InvoiceParty, InvoicePrint,
    validate_invoice,
};
use bigdecimal::{BigDecimal, Zero};
use chrono::NaiveDate;

use super::chart;
use crate::domain::{
    Billing, BusinessProfile, ClientRegion, Invoice, InvoiceAmounts, InvoiceKind, InvoiceNumber,
    InvoiceParts, Milestone, Project,
};

/// Taxable value from which an unregistered client's name and address must be printed (Rule 46)
const ADDRESS_REQUIRED_FROM: u32 = accounting_core::invoice::UNREGISTERED_DETAILS_THRESHOLD_RUPEES;
/// GST portal code for a place of supply outside India
const FOREIGN_PLACE_OF_SUPPLY: &str = "96";
const PLAIN_TITLE: &str = "Invoice";
/// Printed first on every invoice from a seller not registered under GST
pub const PLAIN_NOTE: &str = "Supplier not registered under GST: no GST is charged on this invoice";

/// Why a milestone can't be invoiced yet; each says what to fix
#[derive(Debug, thiserror::Error)]
pub enum InvoicingError {
    #[error("set up your business details in the Settings tab first")]
    NoProfile,
    #[error("add the client's state code (place of supply) to invoice a client without a GSTIN")]
    MissingClientState,
    #[error("add the client's address to the project; it must be on this invoice")]
    MissingClientAddress,
    #[error("add your LUT ARN in Settings to invoice a client abroad without IGST")]
    MissingLut,
    #[error("the SAC code in Settings has no GST rate")]
    UnknownSac,
    #[error("only pending milestones can be invoiced")]
    NotPending,
    #[error("invoice is not compliant: {}", .0.join("; "))]
    NotCompliant(Vec<String>),
    #[error(transparent)]
    Invoice(#[from] accounting_core::InvoiceError),
    #[error(transparent)]
    Posting(#[from] accounting_core::PostingError),
}

/// A built, not yet saved, invoice
#[derive(Debug, Clone)]
pub struct BuiltInvoice {
    pub invoice: Invoice,
    /// The balanced ledger posting: receivable debited, revenue and output tax credited
    pub entries: Vec<Entry>,
    /// Compliance warnings that don't block issuing
    pub warnings: Vec<String>,
}

/// Build invoice `number` for `milestone`, choosing the kind from who is billing whom
pub fn build(
    profile: &BusinessProfile,
    project: &Project,
    milestone: &Milestone,
    number: InvoiceNumber,
    date: NaiveDate,
) -> Result<BuiltInvoice, InvoicingError> {
    if milestone.status() != crate::domain::MilestoneStatus::Pending {
        return Err(InvoicingError::NotPending);
    }
    let line = Line::describe(project, milestone);
    match (profile.gstin(), project.client_region()) {
        (Some(_), ClientRegion::India { .. }) => tax_invoice(profile, project, line, number, date),
        (Some(_), ClientRegion::Abroad { .. }) => {
            let arn = profile.lut_arn().ok_or(InvoicingError::MissingLut)?;
            let terms = std::iter::once(format!(
                "Supply meant for export under LUT (ARN {arn}) without payment of IGST"
            ))
            .chain(profile.terms().iter().cloned())
            .collect();
            untaxed_invoice(
                profile,
                project,
                line,
                number,
                date,
                InvoiceKind::Export,
                terms,
            )
        }
        (None, _) => {
            // accounting-core's layout always prints a GST supply line, so say it outright
            let terms = std::iter::once(PLAIN_NOTE.to_owned())
                .chain(profile.terms().iter().cloned())
                .collect();
            untaxed_invoice(
                profile,
                project,
                line,
                number,
                date,
                InvoiceKind::Plain,
                terms,
            )
        }
    }
}

/// The single invoice line for a milestone
struct Line {
    description: String,
    amount: BigDecimal,
    milestone_id: crate::domain::MilestoneId,
    milestone_name: String,
}

impl Line {
    fn describe(project: &Project, milestone: &Milestone) -> Self {
        let mut description = format!("{} - {}", project.name(), milestone.name());
        if let Billing::Hourly { rate } = project.billing() {
            let minutes = milestone.logged_minutes();
            description.push_str(&format!(
                " ({}h {:02}m @ Rs. {}/hour)",
                minutes / 60,
                minutes % 60,
                format_inr(rate)
            ));
        }
        Self {
            description,
            amount: milestone.amount().clone(),
            milestone_id: milestone.id(),
            milestone_name: milestone.name().to_owned(),
        }
    }
}

fn seller_party(profile: &BusinessProfile) -> InvoiceParty {
    match profile.gstin() {
        Some(gstin) => InvoiceParty::new(profile.name(), profile.address().to_vec(), gstin.clone()),
        None => InvoiceParty::unregistered(profile.name(), profile.address().to_vec()),
    }
}

fn buyer_party(project: &Project) -> InvoiceParty {
    let mut address = project.client_address().to_vec();
    if let ClientRegion::Abroad { country } = project.client_region() {
        address.push(country.clone());
    }
    match project.client_gstin() {
        Some(gstin) => InvoiceParty::new(project.client_name(), address, gstin.clone()),
        None => InvoiceParty::unregistered(project.client_name(), address),
    }
}

/// GST tax invoice: accounting-core computes the tax, checks compliance and lays out the print
fn tax_invoice(
    profile: &BusinessProfile,
    project: &Project,
    line: Line,
    number: InvoiceNumber,
    date: NaiveDate,
) -> Result<BuiltInvoice, InvoicingError> {
    let seller_gstin = profile.gstin().cloned().ok_or(InvoicingError::NoProfile)?;
    let recipient = match (project.client_gstin(), project.client_region()) {
        (Some(gstin), _) => Recipient::Registered(gstin.clone()),
        (None, ClientRegion::India { state: Some(state) }) => {
            Recipient::unregistered(state.clone())
        }
        (None, _) => return Err(InvoicingError::MissingClientState),
    };
    let rate = profile
        .gst_rate()
        .ok_or(InvoicingError::UnknownSac)?
        .clone();
    let item = GstLineItem::new(
        profile.sac_code(),
        line.description,
        BigDecimal::from(1),
        line.amount,
        rate,
    )?;
    let gst = GstInvoice::new(number.as_str(), date, seller_gstin, recipient, vec![item])?;

    let report = validate_invoice(&gst, date, HsnMaster::global());
    let errors: Vec<String> = report.errors().map(ToString::to_string).collect();
    if !errors.is_empty() {
        return Err(InvoicingError::NotCompliant(errors));
    }
    let warnings = report.warnings().map(ToString::to_string).collect();

    let parties = InvoiceParties::new(seller_party(profile), buyer_party(project));
    let print = InvoicePrint::from_invoice(&gst, &parties)?;
    let totals = gst.breakdown()?;
    let entries = gst.to_entries(&chart::invoice_accounts())?;
    let amounts = InvoiceAmounts {
        taxable_value: totals.taxable_value,
        cgst: totals.cgst,
        sgst: totals.sgst,
        igst: totals.igst,
        total: totals.total,
    };
    let invoice = Invoice::issue(InvoiceParts {
        number,
        date,
        kind: InvoiceKind::Tax,
        project_id: project.id(),
        milestone_id: line.milestone_id,
        client_name: project.client_name().to_owned(),
        milestone_name: line.milestone_name,
        amounts,
        print,
        terms: profile.terms().to_vec(),
        gst: Some(gst),
    });
    Ok(BuiltInvoice {
        invoice,
        entries,
        warnings,
    })
}

/// Export (zero-rated under LUT) or plain (seller not registered): no GST is charged, so compter
/// lays out the print itself in accounting-core's format
fn untaxed_invoice(
    profile: &BusinessProfile,
    project: &Project,
    line: Line,
    number: InvoiceNumber,
    date: NaiveDate,
    kind: InvoiceKind,
    terms: Vec<String>,
) -> Result<BuiltInvoice, InvoicingError> {
    // Same rule accounting-core applies to tax invoices: unregistered clients from the Rule 46
    // threshold up; exports always, as the recipient abroad has no GSTIN to identify them
    let abroad = project.client_region().is_abroad();
    let unregistered_large =
        project.client_gstin().is_none() && line.amount >= ADDRESS_REQUIRED_FROM;
    if (abroad || unregistered_large) && project.client_address().is_empty() {
        return Err(InvoicingError::MissingClientAddress);
    }

    let export = kind == InvoiceKind::Export;
    let amount = format_inr(&line.amount);
    let zero = format_inr(&BigDecimal::zero());
    let (title, gst_rate, tax_lines, place_of_supply) = if export {
        let tax_lines = vec![TaxLine {
            label: "IGST @ 0% (export under LUT)".to_owned(),
            amount: zero.clone(),
        }];
        (
            TAX_INVOICE_TITLE,
            "0%",
            tax_lines,
            FOREIGN_PLACE_OF_SUPPLY.to_owned(),
        )
    } else {
        let place = match project.client_region() {
            ClientRegion::India { state: Some(state) } => state.as_str().to_owned(),
            ClientRegion::India { state: None } => project
                .client_gstin()
                .map(|g| g.state_code().to_owned())
                .unwrap_or_default(),
            ClientRegion::Abroad { .. } => FOREIGN_PLACE_OF_SUPPLY.to_owned(),
        };
        (PLAIN_TITLE, "-", Vec::new(), place)
    };

    let print = InvoicePrint {
        title: title.to_owned(),
        invoice_number: number.to_string(),
        invoice_date: date.format(PRINT_DATE_FORMAT).to_string(),
        place_of_supply,
        is_inter_state: export,
        seller: seller_party(profile),
        buyer: buyer_party(project),
        rows: vec![PrintRow {
            serial: 1,
            hsn_sac: profile.sac_code().to_owned(),
            description: line.description,
            quantity: "1".to_owned(),
            rate: amount.clone(),
            taxable_value: amount.clone(),
            gst_rate: gst_rate.to_owned(),
            tax: zero.clone(),
            amount: amount.clone(),
        }],
        taxable_value: amount.clone(),
        tax_lines,
        total_tax: zero,
        total: amount,
        total_in_words: amount_in_words(&line.amount),
    };
    let revenue = if abroad {
        chart::EXPORT_REVENUE
    } else {
        chart::SERVICE_REVENUE
    };
    let entries = vec![
        Entry::debit(chart::RECEIVABLE, line.amount.clone(), None),
        Entry::credit(revenue, line.amount.clone(), None),
    ];
    let invoice = Invoice::issue(InvoiceParts {
        number,
        date,
        kind,
        project_id: project.id(),
        milestone_id: line.milestone_id,
        client_name: project.client_name().to_owned(),
        milestone_name: line.milestone_name,
        amounts: InvoiceAmounts::untaxed(line.amount),
        print,
        terms,
        gst: None,
    });
    Ok(BuiltInvoice {
        invoice,
        entries,
        warnings: Vec::new(),
    })
}
