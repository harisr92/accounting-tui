//! Turning milestones into an invoice document and its ledger entries. Pure: no storage.

use accounting_core::invoice::print::PRINT_DATE_FORMAT;
use accounting_core::invoice::{PrintRow, Recipient, TAX_INVOICE_TITLE, TaxLine};
use accounting_core::{
    Entry, GstInvoice, GstLineItem, HsnMaster, InvoiceParties, InvoiceParty, InvoicePrint,
    validate_invoice,
};
use bigdecimal::{BigDecimal, Zero};
use chrono::NaiveDate;

use super::chart;
use crate::domain::{
    Billing, BusinessProfile, ClientRegion, Currency, Discount, DomainError, ExchangeRate, Invoice,
    InvoiceAmounts, InvoiceKind, InvoiceLine, InvoiceNumber, InvoiceParts, Milestone,
    MilestoneStatus, Project,
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
    #[error("select at least one milestone to invoice")]
    NothingSelected,
    #[error("enter the exchange rate (rupees per {0}) to invoice in {0}")]
    MissingExchangeRate(Currency),
    #[error("a credit note on this invoice had to be issued by {0} (GST section 34)")]
    CreditNoteTooLate(NaiveDate),
    #[error("this tax invoice has no GST record to credit")]
    MissingGstRecord,
    #[error("invoice is not compliant: {}", .0.join("; "))]
    NotCompliant(Vec<String>),
    #[error(transparent)]
    Invoice(#[from] accounting_core::InvoiceError),
    #[error(transparent)]
    Posting(#[from] accounting_core::PostingError),
    #[error(transparent)]
    Domain(#[from] DomainError),
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

/// Build invoice `number` billing `milestones` of `project`, a line each in the order given,
/// choosing the kind from who is billing whom. A project billed in a foreign currency needs
/// `rate`, the rupees per unit on `date`; rupee projects ignore it. A `discount` comes off the
/// total before GST, spread over the lines.
pub fn build(
    profile: &BusinessProfile,
    project: &Project,
    milestones: &[&Milestone],
    number: InvoiceNumber,
    date: NaiveDate,
    rate: Option<ExchangeRate>,
    discount: Option<&Discount>,
) -> Result<BuiltInvoice, InvoicingError> {
    if milestones.is_empty() {
        return Err(InvoicingError::NothingSelected);
    }
    for (i, milestone) in milestones.iter().enumerate() {
        let on_project = project.milestone_index(milestone.id()).is_some();
        let repeated = milestones[..i].iter().any(|m| m.id() == milestone.id());
        if milestone.status() != MilestoneStatus::Pending || !on_project {
            return Err(InvoicingError::NotPending);
        }
        if repeated {
            return Err(DomainError::DuplicateInvoiceLine.into());
        }
    }
    let currency = project.currency();
    let rate = if currency.is_inr() {
        None
    } else {
        Some(rate.ok_or(InvoicingError::MissingExchangeRate(currency))?)
    };
    let mut lines: Vec<Line> = milestones
        .iter()
        .map(|milestone| Line::describe(project, milestone))
        .collect();
    let discount_note = match discount {
        Some(discount) => Some(apply_discount(discount, &mut lines, currency)?),
        None => None,
    };
    match (profile.gstin(), project.client_region()) {
        (Some(_), ClientRegion::India { .. }) => {
            let terms = discount_note
                .into_iter()
                .chain(profile.terms().iter().cloned())
                .collect();
            tax_invoice(profile, project, lines, number, date, terms)
        }
        (Some(_), ClientRegion::Abroad { .. }) => {
            let arn = profile.lut_arn().ok_or(InvoicingError::MissingLut)?;
            let terms = std::iter::once(format!(
                "Supply meant for export under LUT (ARN {arn}) without payment of IGST"
            ))
            .chain(discount_note)
            .chain(profile.terms().iter().cloned())
            .collect();
            untaxed_invoice(
                profile,
                project,
                lines,
                Issue {
                    number,
                    date,
                    kind: InvoiceKind::Export,
                    rate,
                    terms,
                },
            )
        }
        (None, _) => {
            // accounting-core's layout always prints a GST supply line, so say it outright
            let terms = std::iter::once(PLAIN_NOTE.to_owned())
                .chain(discount_note)
                .chain(profile.terms().iter().cloned())
                .collect();
            untaxed_invoice(
                profile,
                project,
                lines,
                Issue {
                    number,
                    date,
                    kind: InvoiceKind::Plain,
                    rate,
                    terms,
                },
            )
        }
    }
}

/// What an untaxed invoice is issued as
struct Issue {
    number: InvoiceNumber,
    date: NaiveDate,
    kind: InvoiceKind,
    rate: Option<ExchangeRate>,
    terms: Vec<String>,
}

/// The invoice line for one milestone
struct Line {
    description: String,
    /// Before GST, after `discount`
    amount: BigDecimal,
    discount: BigDecimal,
    milestone_id: crate::domain::MilestoneId,
    milestone_name: String,
}

/// Take `discount` off `lines` in proportion, noting each line's share on its row, and return
/// the line printed with the terms. GST is then charged on what is left.
fn apply_discount(
    discount: &Discount,
    lines: &mut [Line],
    currency: Currency,
) -> Result<String, InvoicingError> {
    let gross: Vec<BigDecimal> = lines.iter().map(|line| line.amount.clone()).collect();
    let shares = discount.split(&gross)?;
    let label = currency.pdf_label();
    for (line, share) in lines.iter_mut().zip(&shares) {
        line.description.push_str(&format!(
            " less discount {label} {}",
            currency.format(share)
        ));
        line.amount = &line.amount - share;
        line.discount = share.clone();
    }
    let subtotal: BigDecimal = gross.iter().sum();
    let total: BigDecimal = shares.iter().sum();
    let percent = match discount {
        Discount::Percent(_) => format!(" ({discount})"),
        Discount::Amount(_) => String::new(),
    };
    Ok(format!(
        "Discount of {label} {}{percent} on {label} {}, taken off before tax",
        currency.format(&total),
        currency.format(&subtotal)
    ))
}

/// The lines as the invoice records them
fn invoice_lines(lines: &[Line]) -> Vec<InvoiceLine> {
    lines
        .iter()
        .map(|line| InvoiceLine {
            milestone_id: line.milestone_id,
            milestone_name: line.milestone_name.clone(),
            taxable_value: line.amount.clone(),
            discount: line.discount.clone(),
        })
        .collect()
}

impl Line {
    fn describe(project: &Project, milestone: &Milestone) -> Self {
        let mut description = format!("{} - {}", project.name(), milestone.name());
        if let Billing::Hourly { rate } = project.billing() {
            let minutes = milestone.logged_minutes();
            let currency = project.currency();
            description.push_str(&format!(
                " ({}h {:02}m @ {} {}/hour)",
                minutes / 60,
                minutes % 60,
                currency.pdf_label(),
                currency.format(rate)
            ));
        }
        Self {
            description,
            amount: milestone.amount().clone(),
            discount: BigDecimal::zero(),
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
    lines: Vec<Line>,
    number: InvoiceNumber,
    date: NaiveDate,
    terms: Vec<String>,
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
    let items = lines
        .iter()
        .map(|line| {
            GstLineItem::new(
                profile.sac_code(),
                line.description.clone(),
                BigDecimal::from(1),
                line.amount.clone(),
                rate.clone(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let gst = GstInvoice::new(number.as_str(), date, seller_gstin, recipient, items)?;

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
        client_name: project.client_name().to_owned(),
        lines: invoice_lines(&lines),
        currency: Currency::Inr,
        exchange_rate: None,
        amounts,
        print,
        terms,
        gst: Some(gst),
        logo: profile.logo(),
    })?;
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
    lines: Vec<Line>,
    issue: Issue,
) -> Result<BuiltInvoice, InvoicingError> {
    let Issue {
        number,
        date,
        kind,
        rate,
        mut terms,
    } = issue;
    // Same rule accounting-core applies to tax invoices: unregistered clients from the Rule 46
    // threshold up; exports always, as the recipient abroad has no GSTIN to identify them
    let abroad = project.client_region().is_abroad();
    let taxable: BigDecimal = lines.iter().map(|line| &line.amount).sum();
    let unregistered_large = project.client_gstin().is_none() && taxable >= ADDRESS_REQUIRED_FROM;
    if (abroad || unregistered_large) && project.client_address().is_empty() {
        return Err(InvoicingError::MissingClientAddress);
    }

    let export = kind == InvoiceKind::Export;
    let currency = project.currency();
    let total = currency.format(&taxable);
    let zero = currency.format(&BigDecimal::zero());
    // Rupees for the ledger, and the rate printed so the rupee value can be checked
    let inr = match &rate {
        Some(rate) => {
            let inr = rate.to_inr(&taxable);
            let code = currency.code();
            terms.insert(
                0,
                format!(
                    "Amounts in {code}. Exchange rate: 1 {code} = Rs. {rate}; value Rs. {}",
                    Currency::Inr.format(&inr)
                ),
            );
            inr
        }
        None => taxable.clone(),
    };
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
        rows: lines
            .iter()
            .zip(1..)
            .map(|(line, serial)| {
                let amount = currency.format(&line.amount);
                PrintRow {
                    serial,
                    hsn_sac: profile.sac_code().to_owned(),
                    description: line.description.clone(),
                    quantity: "1".to_owned(),
                    rate: amount.clone(),
                    taxable_value: amount.clone(),
                    gst_rate: gst_rate.to_owned(),
                    tax: zero.clone(),
                    amount,
                }
            })
            .collect(),
        taxable_value: total.clone(),
        tax_lines,
        total_tax: zero,
        total,
        total_in_words: currency.in_words(&taxable),
    };
    let revenue = if abroad {
        chart::EXPORT_REVENUE
    } else {
        chart::SERVICE_REVENUE
    };
    let entries = vec![
        Entry::debit(chart::RECEIVABLE, inr.clone(), None),
        Entry::credit(revenue, inr, None),
    ];
    let invoice = Invoice::issue(InvoiceParts {
        number,
        date,
        kind,
        project_id: project.id(),
        client_name: project.client_name().to_owned(),
        lines: invoice_lines(&lines),
        currency,
        exchange_rate: rate,
        amounts: InvoiceAmounts::untaxed(taxable),
        print,
        terms,
        gst: None,
        logo: profile.logo(),
    })?;
    Ok(BuiltInvoice {
        invoice,
        entries,
        warnings: Vec::new(),
    })
}
