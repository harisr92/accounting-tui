use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use accounting_core::invoice::render_pdf;
use accounting_core::{
    DefaultTransactionValidator, PdfOptions, TransactionBuilder, TransactionValidator,
};
use bigdecimal::{BigDecimal, Zero};
use chrono::NaiveDate;

use super::invoice_builder::{self, BuiltInvoice, InvoicingError};
use super::{ServiceError, chart};
use crate::domain::{
    BusinessProfile, Currency, ExchangeRate, FiscalYear, Invoice, InvoiceAmounts, InvoiceKind,
    InvoiceNumber, InvoiceSummary, MilestoneId, Payment, ProfileDraft, Project, ProjectId,
};
use crate::storage::{
    DataDir, InvoiceRepository, IssueRecord, PaymentRecord, ProjectRepository, SettingsRepository,
    write_file,
};

/// What issuing would produce, shown for confirmation before anything is saved
#[derive(Debug, Clone)]
pub struct InvoicePreview {
    pub number: InvoiceNumber,
    pub kind: InvoiceKind,
    pub client_name: String,
    pub currency: Currency,
    /// In `currency`
    pub amounts: InvoiceAmounts,
    /// The total in rupees, for an invoice in a foreign currency
    pub total_inr: Option<BigDecimal>,
    pub warnings: Vec<String>,
}

/// The result of issuing: the saved invoice, and where its PDF went (or why it didn't)
#[derive(Debug)]
pub struct Issued {
    pub invoice: Invoice,
    pub pdf: Result<Option<PathBuf>, String>,
    pub warnings: Vec<String>,
}

/// Invoicing and payments. With a [`DataDir`], PDFs are written under it; without one (demo,
/// tests) they are only rendered.
pub struct InvoiceService<R> {
    repo: R,
    data_dir: Option<DataDir>,
}

impl<R: ProjectRepository + InvoiceRepository + SettingsRepository> InvoiceService<R> {
    pub fn new(repo: R, data_dir: Option<DataDir>) -> Self {
        Self { repo, data_dir }
    }

    pub fn profile(&self) -> Result<Option<BusinessProfile>, ServiceError> {
        Ok(self.repo.load_profile()?)
    }

    pub fn save_profile(&mut self, draft: ProfileDraft) -> Result<BusinessProfile, ServiceError> {
        let profile = BusinessProfile::try_from(draft)?;
        self.repo.save_profile(&profile)?;
        Ok(profile)
    }

    pub fn list(&self) -> Result<Vec<InvoiceSummary>, ServiceError> {
        Ok(self.repo.list_invoices()?)
    }

    pub fn load(&self, number: &InvoiceNumber) -> Result<Invoice, ServiceError> {
        Ok(self.repo.load_invoice(number)?)
    }

    /// Build the invoice the milestone would get on `date`, without saving anything. `rate` is
    /// needed for a project billed in a foreign currency (rupees per unit on `date`).
    pub fn preview(
        &self,
        project_id: ProjectId,
        milestone_id: MilestoneId,
        date: NaiveDate,
        rate: Option<ExchangeRate>,
    ) -> Result<InvoicePreview, ServiceError> {
        let (_, built, _) = self.build(project_id, milestone_id, date, rate)?;
        let invoice = &built.invoice;
        Ok(InvoicePreview {
            number: invoice.number().clone(),
            kind: invoice.kind(),
            client_name: invoice.client_name().to_owned(),
            currency: invoice.currency(),
            amounts: invoice.amounts().clone(),
            total_inr: invoice.exchange_rate().map(|_| invoice.inr_amounts().total),
            warnings: built.warnings,
        })
    }

    /// Issue the next invoice for a pending milestone, post it and write its PDF
    pub fn issue(
        &mut self,
        project_id: ProjectId,
        milestone_id: MilestoneId,
        date: NaiveDate,
        rate: Option<ExchangeRate>,
    ) -> Result<Issued, ServiceError> {
        let (mut project, built, sequence) = self.build(project_id, milestone_id, date, rate)?;
        let BuiltInvoice {
            invoice,
            entries,
            warnings,
        } = built;
        // Render before saving: a document that can't be printed is never issued
        let pdf = render(&invoice)?;

        let index = project
            .milestone_index(milestone_id)
            .ok_or(InvoicingError::NotPending)?;
        project.invoice_milestone(index, invoice.number().as_str())?;
        let transaction = entries
            .into_iter()
            .fold(
                TransactionBuilder::new(
                    format!("inv-{}", invoice.number()),
                    date,
                    format!("Invoice {} to {}", invoice.number(), invoice.client_name()),
                )
                .reference(invoice.number().as_str()),
                TransactionBuilder::entry,
            )
            .build()?;
        DefaultTransactionValidator.validate_transaction(&transaction)?;

        self.repo.issue(&IssueRecord {
            sequence,
            invoice: &invoice,
            project: &project,
            transaction: &transaction,
            accounts: &chart::accounts(),
        })?;
        let pdf = self.write_pdf(&invoice, &pdf);
        Ok(Issued {
            invoice,
            pdf,
            warnings,
        })
    }

    /// Record all or part of what the client owes on an invoice in rupees: `received` reached
    /// the bank, `tds` was withheld. The milestone becomes paid once nothing is left to pay.
    pub fn record_payment(
        &mut self,
        number: &InvoiceNumber,
        date: NaiveDate,
        received: BigDecimal,
        tds: BigDecimal,
    ) -> Result<Invoice, ServiceError> {
        let mut invoice = self.repo.load_invoice(number)?;
        let payment = invoice.record_payment(date, received, tds)?;
        self.save_payment(invoice, &payment)
    }

    /// Record a payment on an invoice in a foreign currency: `received` (in that currency) settles
    /// the balance and `credited_inr` reached the bank. The difference from the invoice's rate is
    /// posted as an exchange gain or loss.
    pub fn record_foreign_payment(
        &mut self,
        number: &InvoiceNumber,
        date: NaiveDate,
        received: BigDecimal,
        credited_inr: BigDecimal,
    ) -> Result<Invoice, ServiceError> {
        let mut invoice = self.repo.load_invoice(number)?;
        let payment = invoice.record_foreign_payment(date, received, credited_inr)?;
        self.save_payment(invoice, &payment)
    }

    /// Post `payment`, just added to `invoice`, and save both (with the milestone once paid)
    fn save_payment(
        &mut self,
        invoice: Invoice,
        payment: &Payment,
    ) -> Result<Invoice, ServiceError> {
        let number = invoice.number().clone();
        let date = payment.date;
        let project = if invoice.is_paid() {
            let mut project = self.repo.load(invoice.project_id())?;
            let index = project
                .milestone_index(invoice.milestone_id())
                .ok_or_else(|| ServiceError::MissingMilestone(number.to_string()))?;
            project.record_payment(index)?;
            Some(project)
        } else {
            None
        };

        // One posting per payment: pay-INV/26-27/004-1, -2, …
        let mut builder = TransactionBuilder::new(
            format!("pay-{number}-{}", invoice.payments().len()),
            date,
            format!("Payment for {number} from {}", invoice.client_name()),
        )
        .reference(number.as_str());
        let banked = payment.received_inr();
        if *banked > BigDecimal::zero() {
            builder = builder.debit(chart::BANK, banked.clone(), None);
        }
        if payment.tds > BigDecimal::zero() {
            builder = builder.debit(
                chart::TDS_RECEIVABLE,
                payment.tds.clone(),
                Some("TDS withheld".to_owned()),
            );
        }
        let cleared = receivable_cleared(&invoice);
        // Rupees banked beyond (or short of) the receivable at the invoice's rate
        let difference = banked + invoice.to_inr(&payment.tds) - &cleared;
        if difference > BigDecimal::zero() {
            builder = builder.credit(
                chart::EXCHANGE_DIFFERENCE,
                difference,
                Some("Exchange gain".to_owned()),
            );
        } else if difference < BigDecimal::zero() {
            builder = builder.debit(
                chart::EXCHANGE_DIFFERENCE,
                -difference,
                Some("Exchange loss".to_owned()),
            );
        }
        let transaction = builder.credit(chart::RECEIVABLE, cleared, None).build()?;
        DefaultTransactionValidator.validate_transaction(&transaction)?;

        self.repo.record_payment(&PaymentRecord {
            invoice: &invoice,
            project: project.as_ref(),
            transaction: &transaction,
            accounts: &chart::accounts(),
        })?;
        Ok(invoice)
    }

    /// The invoice's PDF, rendered from its stored print
    pub fn pdf_bytes(&self, invoice: &Invoice) -> Result<Vec<u8>, ServiceError> {
        render(invoice)
    }

    /// Where the invoice's PDF lives, writing it first if it is missing
    pub fn ensure_pdf(&self, number: &InvoiceNumber) -> Result<PathBuf, ServiceError> {
        let dir = self.data_dir.as_ref().ok_or(ServiceError::NoPdfFolder)?;
        let invoice = self.repo.load_invoice(number)?;
        let path = pdf_path(dir, &invoice);
        if !path.exists() {
            write_file(&path, &render(&invoice)?)?;
        }
        Ok(path)
    }

    /// Open the invoice's PDF in the system viewer (`$COMPTER_OPEN`, else `open` / `xdg-open`)
    pub fn open_pdf(&self, number: &InvoiceNumber) -> Result<PathBuf, ServiceError> {
        let path = self.ensure_pdf(number)?;
        open_in_viewer(&path).map_err(|source| ServiceError::OpenPdf {
            path: path.clone(),
            source,
        })?;
        Ok(path)
    }

    /// Load the project, take the year's next number and build the invoice
    fn build(
        &self,
        project_id: ProjectId,
        milestone_id: MilestoneId,
        date: NaiveDate,
        rate: Option<ExchangeRate>,
    ) -> Result<(Project, BuiltInvoice, u32), ServiceError> {
        let profile = self.repo.load_profile()?.ok_or(InvoicingError::NoProfile)?;
        let project = self.repo.load(project_id)?;
        let milestone = project
            .milestone_index(milestone_id)
            .and_then(|i| project.milestones().get(i))
            .ok_or(InvoicingError::NotPending)?;
        let year = FiscalYear::containing(date);
        let sequence = self.repo.next_sequence(year)?;
        let number = InvoiceNumber::new(profile.invoice_prefix(), year, sequence);
        let built = invoice_builder::build(&profile, &project, milestone, number, date, rate)?;
        Ok((project, built, sequence))
    }

    fn write_pdf(&self, invoice: &Invoice, bytes: &[u8]) -> Result<Option<PathBuf>, String> {
        let Some(dir) = &self.data_dir else {
            return Ok(None);
        };
        let path = pdf_path(dir, invoice);
        write_file(&path, bytes)
            .map(|()| Some(path))
            .map_err(|e| e.to_string())
    }
}

/// Rupees the latest payment takes off the receivable: its settled amount at the invoice's rate,
/// or whatever is left of the receivable when it settles the invoice, so no paisa of rounding
/// stays behind
fn receivable_cleared(invoice: &Invoice) -> BigDecimal {
    let Some((latest, earlier)) = invoice.payments().split_last() else {
        return BigDecimal::zero();
    };
    if invoice.is_paid() {
        let cleared_before: BigDecimal = earlier.iter().map(|p| invoice.to_inr(&p.settled())).sum();
        invoice.inr_amounts().total - cleared_before
    } else {
        invoice.to_inr(&latest.settled())
    }
}

fn open_in_viewer(path: &Path) -> std::io::Result<()> {
    viewer_command()
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(drop)
}

/// `$COMPTER_OPEN`, else the platform's "open with the default app" command, waiting for a path
fn viewer_command() -> Command {
    if let Ok(opener) = std::env::var("COMPTER_OPEN") {
        return Command::new(opener);
    }
    if cfg!(windows) {
        // `start` is a cmd built-in, and its first quoted argument is the window title
        let mut command = Command::new("cmd");
        command.args(["/C", "start", ""]);
        command
    } else if cfg!(target_os = "macos") {
        Command::new("open")
    } else {
        Command::new("xdg-open")
    }
}

fn pdf_path(dir: &DataDir, invoice: &Invoice) -> PathBuf {
    dir.invoice_pdf_path(
        &invoice.fiscal_year().label(),
        &invoice.number().file_stem(),
    )
}

fn render(invoice: &Invoice) -> Result<Vec<u8>, ServiceError> {
    let options = PdfOptions {
        currency_label: invoice.currency().pdf_label().to_owned(),
        terms: invoice.terms().to_vec(),
        // accounting-core's default note claims e-invoicing, which compter doesn't do
        footer_note: String::new(),
        ..PdfOptions::default()
    };
    Ok(render_pdf(invoice.print(), &options)?)
}
