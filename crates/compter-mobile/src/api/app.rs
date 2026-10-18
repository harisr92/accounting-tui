use accounting_core::StateCode;
use chrono::{Local, NaiveDate};
use flutter_rust_bridge::frb;

use compter_core::demo;
use compter_core::domain::{
    Billing, ClientDetails, ClientRegion, Currency, ExchangeRate, InvoiceNumber, MilestoneId,
    ProfileDraft, ProjectId, TimeEntry,
};
use compter_core::input;
use compter_core::services::{
    CreditDraft, InvoiceService, MilestoneDraft, ProjectDraft, ProjectService, ServiceError,
};
use compter_core::storage::{DataDir, RedbRepository, Store};

use super::profile::currency;
use super::{
    ApiError, CreditInput, CreditPreviewRow, CreditedRow, InvoiceDetail, InvoiceList,
    InvoicePreviewRow, InvoiceRow, IssuedRow, MilestoneInput, PaymentInput, ProfileInput,
    ProfileRow, ProjectDetail, ProjectInput, ProjectRow, TimeInput,
};

/// The open database and the services over it. Dart opens one at start-up and keeps it for the
/// life of the app; only one can have the database file open.
#[frb(opaque)]
pub struct CompterApp {
    projects: ProjectService<RedbRepository>,
    invoices: InvoiceService<RedbRepository>,
    /// Kept so the database stays open as long as the services use it
    _store: Store,
}

impl CompterApp {
    /// Open (or create) `compter.redb` in `data_dir`, the app's private support folder. PDFs
    /// are written under it too.
    pub fn open(data_dir: String) -> Result<Self, ApiError> {
        let dir = DataDir::new(data_dir);
        let store = Store::open(&dir).map_err(ServiceError::from)?;
        Ok(Self::over(store, dir))
    }

    /// The sample business of `compter --demo`, in memory; nothing is saved. Its PDFs are
    /// written to `scratch_dir`, a cache folder.
    pub fn demo(scratch_dir: String) -> Result<Self, ApiError> {
        Ok(Self::over(demo::store()?, DataDir::new(scratch_dir)))
    }

    fn over(store: Store, dir: DataDir) -> Self {
        Self {
            projects: ProjectService::new(store.repository()),
            invoices: InvoiceService::new(store.repository(), Some(dir)),
            _store: store,
        }
    }

    /// The project list, from summaries; archived projects only when asked for
    pub fn projects(&self, include_archived: bool) -> Result<Vec<ProjectRow>, ApiError> {
        Ok(self
            .projects
            .list()?
            .iter()
            .filter(|p| include_archived || !p.is_archived())
            .map(ProjectRow::from)
            .collect())
    }

    pub fn project(&self, id: String) -> Result<ProjectDetail, ApiError> {
        Ok(ProjectDetail::from(&self.projects.load(project_id(&id)?)?))
    }

    pub fn create_project(&mut self, input: ProjectInput) -> Result<ProjectDetail, ApiError> {
        let billing = match input.hourly_rate.as_deref().and_then(input::optional) {
            Some(rate) => {
                let rate = input::money(&rate).map_err(|e| ApiError::invalid("hourly_rate", e))?;
                Billing::hourly(rate)
                    .map_err(|e| ApiError::invalid("hourly_rate", e.to_string()))?
            }
            None => Billing::Fixed,
        };
        let optional = |text: &Option<String>| text.as_deref().and_then(input::optional);
        let region = match optional(&input.client_country) {
            Some(country) => ClientRegion::Abroad { country },
            None => ClientRegion::India {
                state: optional(&input.client_state)
                    .map(|code| StateCode::parse(&code))
                    .transpose()
                    .map_err(|e| ApiError::invalid("client_state", e.to_string()))?,
            },
        };
        let currency = match optional(&input.currency) {
            Some(code) => currency(&code)
                .ok_or_else(|| ApiError::invalid("currency", "choose INR, USD, GBP or EUR"))?,
            None => Currency::Inr,
        };
        let draft = ProjectDraft {
            name: input.name.trim().to_owned(),
            client: ClientDetails {
                name: input.client_name.trim().to_owned(),
                // Upper-cased as the terminal does; a GSTIN on a client abroad is refused by
                // the domain rather than dropped
                gstin: optional(&input.client_gstin).map(|g| g.to_uppercase()),
                address: input.client_address,
                region,
                currency,
            },
            billing,
        };
        Ok(ProjectDetail::from(&self.projects.create(draft)?))
    }

    pub fn add_milestone(
        &mut self,
        project_id: String,
        input: MilestoneInput,
    ) -> Result<ProjectDetail, ApiError> {
        let id = self::project_id(&project_id)?;
        let due_date = date(&input.due_date, "due_date")?;
        let hourly = matches!(self.projects.load(id)?.billing(), Billing::Hourly { .. });
        let amount = match (hourly, input.amount.as_deref()) {
            (true, _) => None,
            (false, amount) => Some(
                input::money(amount.unwrap_or_default())
                    .map_err(|e| ApiError::invalid("amount", e))?,
            ),
        };
        let draft = MilestoneDraft {
            name: input.name.trim().to_owned(),
            due_date,
            amount,
        };
        Ok(ProjectDetail::from(
            &self.projects.add_milestone(id, draft)?,
        ))
    }

    /// Log time worked on a pending milestone of an hourly project
    pub fn log_time(
        &mut self,
        project_id: String,
        milestone_id: String,
        input: TimeInput,
    ) -> Result<ProjectDetail, ApiError> {
        let id = self::project_id(&project_id)?;
        let milestone = self::milestone_id(&milestone_id)?;
        let date = date(&input.date, "date")?;
        let minutes = input::duration_minutes(&input.duration)
            .map_err(|e| ApiError::invalid("duration", e))?;
        let entry = TimeEntry::new(date, minutes, input.note.trim())
            .map_err(|e| ApiError::invalid("duration", e.to_string()))?;
        let index = self
            .projects
            .load(id)?
            .milestone_index(milestone)
            .ok_or_else(|| ApiError::invalid("milestone_id", "no such milestone"))?;
        Ok(ProjectDetail::from(
            &self.projects.log_time(id, index, entry)?,
        ))
    }

    /// The business profile, or `None` before it is set up (invoicing needs it)
    pub fn profile(&self) -> Result<Option<ProfileRow>, ApiError> {
        Ok(self.invoices.profile()?.as_ref().map(ProfileRow::from))
    }

    /// The profile form, prefilled with what is saved or the usual first values
    pub fn profile_input(&self) -> Result<ProfileInput, ApiError> {
        Ok(self
            .invoices
            .profile()?
            .map_or_else(ProfileInput::blank, |p| ProfileInput::from(p.to_draft())))
    }

    /// Save the business profile; the logo, set elsewhere, is kept
    pub fn save_profile(&mut self, input: ProfileInput) -> Result<ProfileRow, ApiError> {
        let currency = currency(&input.currency)
            .ok_or_else(|| ApiError::invalid("currency", "choose INR, USD, GBP or EUR"))?;
        let lines = |lines: Vec<String>| {
            lines
                .iter()
                .filter_map(|line| input::optional(line))
                .collect::<Vec<_>>()
        };
        let draft = ProfileDraft {
            name: input.name.trim().to_owned(),
            address: lines(input.address),
            gstin: input::optional(&input.gstin).map(|g| g.to_uppercase()),
            sac_code: input.sac_code.trim().to_owned(),
            invoice_prefix: input.invoice_prefix.trim().to_owned(),
            lut_arn: input::optional(&input.lut_arn),
            terms: lines(input.terms),
            currency,
            logo: self.invoices.profile()?.and_then(|p| p.logo()),
        };
        Ok(ProfileRow::from(&self.invoices.save_profile(draft)?))
    }

    /// Every issued invoice, newest first
    pub fn invoices(&self) -> Result<InvoiceList, ApiError> {
        let mut summaries = self.invoices.list()?;
        summaries.sort_by(|a, b| (b.date(), b.number()).cmp(&(a.date(), a.number())));
        let outstanding = summaries.iter().map(|s| s.balance_inr()).sum();
        Ok(InvoiceList {
            outstanding: Currency::Inr.display(&outstanding),
            invoices: summaries.iter().map(InvoiceRow::from).collect(),
        })
    }

    pub fn invoice(&self, number: String) -> Result<InvoiceDetail, ApiError> {
        Ok(InvoiceDetail::from(
            &self.invoices.load(&InvoiceNumber::from_stored(&number))?,
        ))
    }

    /// The invoice a pending milestone would get today, without saving anything. `rate` (rupees
    /// per unit) is needed when the project is billed in a foreign currency.
    pub fn preview_invoice(
        &self,
        project_id: String,
        milestone_id: String,
        rate: Option<String>,
    ) -> Result<InvoicePreviewRow, ApiError> {
        let preview = self.invoices.preview(
            self::project_id(&project_id)?,
            self::milestone_id(&milestone_id)?,
            today(),
            exchange_rate(rate.as_deref())?,
        )?;
        Ok(InvoicePreviewRow::from(&preview))
    }

    /// Issue today's invoice for a pending milestone and write its PDF
    pub fn issue_invoice(
        &mut self,
        project_id: String,
        milestone_id: String,
        rate: Option<String>,
    ) -> Result<IssuedRow, ApiError> {
        let id = self::project_id(&project_id)?;
        let issued = self.invoices.issue(
            id,
            self::milestone_id(&milestone_id)?,
            today(),
            exchange_rate(rate.as_deref())?,
        )?;
        Ok(IssuedRow {
            invoice: InvoiceDetail::from(&issued.invoice),
            // The service returns only the invoice; the milestone it invoiced is in the project
            project: ProjectDetail::from(&self.projects.load(id)?),
            pdf_error: issued.pdf.err(),
        })
    }

    /// Record all or part of a payment. On a rupee invoice `tds_or_banked` is the TDS withheld;
    /// on a foreign one it is the rupees that reached the bank.
    pub fn record_payment(
        &mut self,
        number: String,
        input: PaymentInput,
    ) -> Result<InvoiceDetail, ApiError> {
        let number = InvoiceNumber::from_stored(&number);
        let date = date(&input.date, "date")?;
        let received =
            input::money(&input.received).map_err(|e| ApiError::invalid("received", e))?;
        let third = input::money(&input.tds_or_banked)
            .map_err(|e| ApiError::invalid("tds_or_banked", e))?;
        let invoice = if self.invoices.load(&number)?.currency().is_inr() {
            self.invoices.record_payment(&number, date, received, third)
        } else {
            self.invoices
                .record_foreign_payment(&number, date, received, third)
        }?;
        Ok(InvoiceDetail::from(&invoice))
    }

    /// The credit note `input` would make against invoice `number`, without saving
    pub fn preview_credit(
        &self,
        number: String,
        input: CreditInput,
    ) -> Result<CreditPreviewRow, ApiError> {
        let preview = self
            .invoices
            .preview_credit(&InvoiceNumber::from_stored(&number), credit_draft(input)?)?;
        Ok(CreditPreviewRow::from(&preview))
    }

    /// Issue a credit note against invoice `number` and write its PDF
    pub fn issue_credit(
        &mut self,
        number: String,
        input: CreditInput,
    ) -> Result<CreditedRow, ApiError> {
        let credited = self
            .invoices
            .issue_credit(&InvoiceNumber::from_stored(&number), credit_draft(input)?)?;
        Ok(CreditedRow {
            note_number: credited.note.number().to_string(),
            invoice: InvoiceDetail::from(&credited.invoice),
            pdf_error: credited.pdf.err(),
        })
    }

    /// Where invoice `number`'s PDF is, writing it first if it is missing
    pub fn invoice_pdf(&self, number: String) -> Result<String, ApiError> {
        let path = self
            .invoices
            .ensure_pdf(&InvoiceNumber::from_stored(&number))?;
        Ok(path.to_string_lossy().into_owned())
    }

    /// Where credit note `number`'s PDF is, writing it first if it is missing
    pub fn credit_note_pdf(&self, number: String) -> Result<String, ApiError> {
        let path = self
            .invoices
            .ensure_credit_pdf(&InvoiceNumber::from_stored(&number))?;
        Ok(path.to_string_lossy().into_owned())
    }
}

fn today() -> NaiveDate {
    Local::now().date_naive()
}

/// A typed date for the input called `field`; blank means today
fn date(input: &str, field: &str) -> Result<NaiveDate, ApiError> {
    input::date_or(input, today()).map_err(|e| ApiError::invalid(field, e))
}

fn project_id(id: &str) -> Result<ProjectId, ApiError> {
    id.parse()
        .map_err(|_| ApiError::invalid("project_id", "not a project id"))
}

fn milestone_id(id: &str) -> Result<MilestoneId, ApiError> {
    id.parse()
        .map_err(|_| ApiError::invalid("milestone_id", "not a milestone id"))
}

/// The typed rupees-per-unit rate, when one was asked for
fn exchange_rate(rate: Option<&str>) -> Result<Option<ExchangeRate>, ApiError> {
    rate.and_then(input::optional)
        .map(|rate| {
            let rate = input::money(&rate).map_err(|e| ApiError::invalid("rate", e))?;
            ExchangeRate::new(rate).map_err(|e| ApiError::invalid("rate", e.to_string()))
        })
        .transpose()
}

fn credit_draft(input: CreditInput) -> Result<CreditDraft, ApiError> {
    Ok(CreditDraft {
        date: date(&input.date, "date")?,
        taxable_value: input::money(&input.amount).map_err(|e| ApiError::invalid("amount", e))?,
        reason: input.reason.trim().to_owned(),
    })
}
