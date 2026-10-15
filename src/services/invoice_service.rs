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
    BusinessProfile, FiscalYear, Invoice, InvoiceAmounts, InvoiceKind, InvoiceNumber,
    InvoiceSummary, MilestoneId, ProfileDraft, Project, ProjectId,
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
    pub amounts: InvoiceAmounts,
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

    /// Build the invoice the milestone would get on `date`, without saving anything
    pub fn preview(
        &self,
        project_id: ProjectId,
        milestone_id: MilestoneId,
        date: NaiveDate,
    ) -> Result<InvoicePreview, ServiceError> {
        let (_, built, _) = self.build(project_id, milestone_id, date)?;
        let invoice = &built.invoice;
        Ok(InvoicePreview {
            number: invoice.number().clone(),
            kind: invoice.kind(),
            client_name: invoice.client_name().to_owned(),
            amounts: invoice.amounts().clone(),
            warnings: built.warnings,
        })
    }

    /// Issue the next invoice for a pending milestone, post it and write its PDF
    pub fn issue(
        &mut self,
        project_id: ProjectId,
        milestone_id: MilestoneId,
        date: NaiveDate,
    ) -> Result<Issued, ServiceError> {
        let (mut project, built, sequence) = self.build(project_id, milestone_id, date)?;
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

    /// Settle an invoice in full: `received` reached the bank, `tds` was withheld by the client
    pub fn record_payment(
        &mut self,
        number: &InvoiceNumber,
        date: NaiveDate,
        received: BigDecimal,
        tds: BigDecimal,
    ) -> Result<Invoice, ServiceError> {
        let mut invoice = self.repo.load_invoice(number)?;
        let (received, tds) = invoice.record_payment(date, received, tds)?;

        let mut project = self.repo.load(invoice.project_id())?;
        let index = project
            .milestone_index(invoice.milestone_id())
            .ok_or_else(|| ServiceError::MissingMilestone(number.to_string()))?;
        project.record_payment(index)?;

        let mut builder = TransactionBuilder::new(
            format!("pay-{number}"),
            date,
            format!("Payment for {number} from {}", invoice.client_name()),
        )
        .reference(number.as_str());
        if received > BigDecimal::zero() {
            builder = builder.debit(chart::BANK, received, None);
        }
        if tds > BigDecimal::zero() {
            builder = builder.debit(chart::TDS_RECEIVABLE, tds, Some("TDS withheld".to_owned()));
        }
        let transaction = builder
            .credit(chart::RECEIVABLE, invoice.amounts().total.clone(), None)
            .build()?;
        DefaultTransactionValidator.validate_transaction(&transaction)?;

        self.repo.record_payment(&PaymentRecord {
            invoice: &invoice,
            project: &project,
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
        let built = invoice_builder::build(&profile, &project, milestone, number, date)?;
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

fn open_in_viewer(path: &Path) -> std::io::Result<()> {
    let opener = std::env::var("COMPTER_OPEN").unwrap_or_else(|_| {
        if cfg!(target_os = "macos") {
            "open".to_owned()
        } else {
            "xdg-open".to_owned()
        }
    });
    Command::new(opener)
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(drop)
}

fn pdf_path(dir: &DataDir, invoice: &Invoice) -> PathBuf {
    dir.invoice_pdf_path(
        &invoice.fiscal_year().label(),
        &invoice.number().file_stem(),
    )
}

fn render(invoice: &Invoice) -> Result<Vec<u8>, ServiceError> {
    let options = PdfOptions {
        terms: invoice.terms().to_vec(),
        // accounting-core's default note claims e-invoicing, which compter doesn't do
        footer_note: String::new(),
        ..PdfOptions::default()
    };
    Ok(render_pdf(invoice.print(), &options)?)
}
