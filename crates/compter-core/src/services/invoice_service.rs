use std::path::{Path, PathBuf};

use accounting_core::invoice::render_pdf;
use accounting_core::{
    DefaultTransactionValidator, InvoiceParty, InvoicePrint, PdfOptions, TransactionBuilder,
    TransactionValidator,
};
use bigdecimal::{BigDecimal, Zero};
use chrono::NaiveDate;

use super::credit_note_builder::{self, BuiltCreditNote, CreditDraft};
use super::invoice_builder::{self, BuiltInvoice, InvoicingError};
use super::{ServiceError, chart};
use crate::domain::{
    BusinessProfile, CREDIT_NOTE_PREFIX, CreditNote, Currency, DomainError, ExchangeRate,
    FiscalYear, Invoice, InvoiceAmounts, InvoiceKind, InvoiceNumber, InvoiceStatus, InvoiceSummary,
    Logo, LogoId, MilestoneId, Payment, ProfileDraft, Project, ProjectId,
};
use crate::storage::{
    CreditRecord, DataDir, InvoiceRepository, IssueRecord, PaymentRecord, ProjectRepository,
    SettingsRepository, write_file,
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

/// What a credit note would be, shown for confirmation before anything is saved
#[derive(Debug, Clone)]
pub struct CreditPreview {
    pub number: InvoiceNumber,
    pub invoice: InvoiceNumber,
    pub currency: Currency,
    /// In `currency`
    pub amounts: InvoiceAmounts,
    /// What the client still owes after the note
    pub balance_after: BigDecimal,
    pub warnings: Vec<String>,
}

/// The result of issuing a credit note: the saved note, and where its PDF went (or why it didn't)
#[derive(Debug)]
pub struct Credited {
    pub note: CreditNote,
    pub invoice: Invoice,
    pub pdf: Result<Option<PathBuf>, String>,
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
        // Invoices and credit notes must never share a number (or a PDF file)
        if profile.invoice_prefix() == CREDIT_NOTE_PREFIX {
            return Err(DomainError::ReservedInvoicePrefix(CREDIT_NOTE_PREFIX).into());
        }
        self.repo.save_profile(&profile)?;
        Ok(profile)
    }

    /// The logo new invoices are issued with
    pub fn current_logo(&self) -> Result<Option<Logo>, ServiceError> {
        let Some(id) = self.repo.load_profile()?.and_then(|p| p.logo()) else {
            return Ok(None);
        };
        Ok(Some(self.load_logo(id)?))
    }

    /// Print the image at `path` on new invoices, or no logo for `None`. Invoices already issued
    /// keep the logo they were issued with.
    pub fn set_logo(&mut self, path: Option<&Path>) -> Result<BusinessProfile, ServiceError> {
        let profile = self.repo.load_profile()?.ok_or(InvoicingError::NoProfile)?;
        let logo = path
            .map(|path| {
                let bytes = std::fs::read(path).map_err(|source| ServiceError::ReadLogo {
                    path: path.to_owned(),
                    source,
                })?;
                Ok::<_, ServiceError>(Logo::new(bytes)?)
            })
            .transpose()?;
        let profile = BusinessProfile::try_from(ProfileDraft {
            logo: logo.as_ref().map(Logo::id),
            ..profile.to_draft()
        })?;
        if let Some(logo) = &logo {
            check_logo_prints(&profile, logo)?;
        }
        self.repo.set_logo(&profile, logo.as_ref())?;
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
        let pdf = self.render(&invoice)?;

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
        let project = if invoice.is_settled() {
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

    /// Build the credit note `draft` would make against invoice `number`, without saving
    pub fn preview_credit(
        &self,
        number: &InvoiceNumber,
        draft: CreditDraft,
    ) -> Result<CreditPreview, ServiceError> {
        let (mut invoice, built, _) = self.build_credit(number, draft)?;
        invoice.apply_credit(&built.note)?;
        let note = &built.note;
        Ok(CreditPreview {
            number: note.number().clone(),
            invoice: number.clone(),
            currency: note.currency(),
            amounts: note.amounts().clone(),
            balance_after: invoice.balance(),
            warnings: built.warnings,
        })
    }

    /// Issue a credit note against invoice `number`, post it and write its PDF. A note that
    /// leaves nothing due settles the invoice's milestone.
    pub fn issue_credit(
        &mut self,
        number: &InvoiceNumber,
        draft: CreditDraft,
    ) -> Result<Credited, ServiceError> {
        let (mut invoice, built, sequence) = self.build_credit(number, draft)?;
        let BuiltCreditNote { note, entries, .. } = built;
        // Render before saving: a document that can't be printed is never issued
        let pdf = self.render_document(note.print(), note.terms(), note.logo(), note.currency())?;
        invoice.apply_credit(&note)?;
        let mut project = self.repo.load(invoice.project_id())?;
        let index = project
            .milestone_index(invoice.milestone_id())
            .ok_or_else(|| ServiceError::MissingMilestone(number.to_string()))?;
        project.credit_milestone(index, &note.amounts().taxable_value)?;
        match invoice.status() {
            InvoiceStatus::Credited => project.cancel_milestone(index)?,
            InvoiceStatus::Paid => project.record_payment(index)?,
            InvoiceStatus::Issued | InvoiceStatus::PartlyPaid => {}
        }
        let transaction = entries
            .into_iter()
            .fold(
                TransactionBuilder::new(
                    format!("cn-{}", note.number()),
                    note.date(),
                    format!("Credit note {} against {number}", note.number()),
                )
                .reference(note.number().as_str()),
                TransactionBuilder::entry,
            )
            .build()?;
        DefaultTransactionValidator.validate_transaction(&transaction)?;

        self.repo.issue_credit_note(&CreditRecord {
            sequence,
            note: &note,
            invoice: &invoice,
            project: &project,
            transaction: &transaction,
            accounts: &chart::accounts(),
        })?;
        let pdf = self.write_document(note.fiscal_year(), note.number(), &pdf);
        Ok(Credited { note, invoice, pdf })
    }

    pub fn load_credit_note(&self, number: &InvoiceNumber) -> Result<CreditNote, ServiceError> {
        Ok(self.repo.load_credit_note(number)?)
    }

    /// Where a credit note's PDF lives, writing it first if it is missing
    pub fn ensure_credit_pdf(&self, number: &InvoiceNumber) -> Result<PathBuf, ServiceError> {
        let dir = self.data_dir.as_ref().ok_or(ServiceError::NoPdfFolder)?;
        let note = self.repo.load_credit_note(number)?;
        let path = dir.invoice_pdf_path(&note.fiscal_year().label(), &number.file_stem());
        if !path.exists() {
            let bytes =
                self.render_document(note.print(), note.terms(), note.logo(), note.currency())?;
            write_file(&path, &bytes)?;
        }
        Ok(path)
    }

    /// Load the invoice, take the year's next credit note number and build the note
    fn build_credit(
        &self,
        number: &InvoiceNumber,
        draft: CreditDraft,
    ) -> Result<(Invoice, BuiltCreditNote, u32), ServiceError> {
        let invoice = self.repo.load_invoice(number)?;
        if invoice.is_settled() {
            return Err(DomainError::InvoiceAlreadyPaid.into());
        }
        let year = FiscalYear::containing(draft.date);
        let sequence = self.repo.next_credit_sequence(year)?;
        let note_number = InvoiceNumber::new(CREDIT_NOTE_PREFIX, year, sequence);
        let built = credit_note_builder::build(&invoice, note_number, draft)?;
        Ok((invoice, built, sequence))
    }

    /// The invoice's PDF, rendered from its stored print
    pub fn pdf_bytes(&self, invoice: &Invoice) -> Result<Vec<u8>, ServiceError> {
        self.render(invoice)
    }

    /// Where the invoice's PDF lives, writing it first if it is missing
    pub fn ensure_pdf(&self, number: &InvoiceNumber) -> Result<PathBuf, ServiceError> {
        let dir = self.data_dir.as_ref().ok_or(ServiceError::NoPdfFolder)?;
        let invoice = self.repo.load_invoice(number)?;
        let path = pdf_path(dir, &invoice);
        if !path.exists() {
            write_file(&path, &self.render(&invoice)?)?;
        }
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

    fn load_logo(&self, id: LogoId) -> Result<Logo, ServiceError> {
        self.repo
            .load_logo(id)?
            .ok_or(ServiceError::MissingLogo(id))
    }

    /// The PDF from what the invoice stored, with the logo it was issued with
    fn render(&self, invoice: &Invoice) -> Result<Vec<u8>, ServiceError> {
        self.render_document(
            invoice.print(),
            invoice.terms(),
            invoice.logo(),
            invoice.currency(),
        )
    }

    /// An invoice or credit note's PDF from its stored print, terms and logo
    fn render_document(
        &self,
        print: &InvoicePrint,
        terms: &[String],
        logo: Option<LogoId>,
        currency: Currency,
    ) -> Result<Vec<u8>, ServiceError> {
        let logo = logo
            .map(|id| self.load_logo(id))
            .transpose()?
            .map(|logo| logo.bytes().to_vec());
        let options = PdfOptions {
            currency_label: currency.pdf_label().to_owned(),
            terms: terms.to_vec(),
            // accounting-core's default note claims e-invoicing, which compter doesn't do
            footer_note: String::new(),
            logo,
            ..PdfOptions::default()
        };
        Ok(render_pdf(print, &options)?)
    }

    fn write_pdf(&self, invoice: &Invoice, bytes: &[u8]) -> Result<Option<PathBuf>, String> {
        self.write_document(invoice.fiscal_year(), invoice.number(), bytes)
    }

    /// Save a rendered document as `invoices/<year>/<number>.pdf`; nothing in demo mode
    fn write_document(
        &self,
        year: FiscalYear,
        number: &InvoiceNumber,
        bytes: &[u8],
    ) -> Result<Option<PathBuf>, String> {
        let Some(dir) = &self.data_dir else {
            return Ok(None);
        };
        let path = dir.invoice_pdf_path(&year.label(), &number.file_stem());
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
    if invoice.is_settled() {
        let cleared_before: BigDecimal = earlier.iter().map(|p| invoice.to_inr(&p.settled())).sum();
        invoice.inr_amounts().total - cleared_before - invoice.credited_inr()
    } else {
        invoice.to_inr(&latest.settled())
    }
}

/// Draw `logo` on a sample page, so an image accounting-core can't read is refused in Settings
/// rather than when the next invoice is issued
fn check_logo_prints(profile: &BusinessProfile, logo: &Logo) -> Result<(), ServiceError> {
    let seller = InvoiceParty::unregistered(profile.name(), profile.address().to_vec());
    let print = InvoicePrint {
        title: "Invoice".to_owned(),
        invoice_number: String::new(),
        invoice_date: String::new(),
        place_of_supply: String::new(),
        is_inter_state: false,
        buyer: seller.clone(),
        seller,
        rows: Vec::new(),
        taxable_value: String::new(),
        tax_lines: Vec::new(),
        total_tax: String::new(),
        total: String::new(),
        total_in_words: String::new(),
    };
    let options = PdfOptions {
        logo: Some(logo.bytes().to_vec()),
        ..PdfOptions::default()
    };
    render_pdf(&print, &options)?;
    Ok(())
}

fn pdf_path(dir: &DataDir, invoice: &Invoice) -> PathBuf {
    dir.invoice_pdf_path(
        &invoice.fiscal_year().label(),
        &invoice.number().file_stem(),
    )
}
