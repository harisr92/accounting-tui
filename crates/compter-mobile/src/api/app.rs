use accounting_core::StateCode;
use bigdecimal::{BigDecimal, Zero};
use chrono::{Local, NaiveDate};
use flutter_rust_bridge::frb;

use compter_core::demo;
use compter_core::domain::{
    Billing, ClientDetails, ClientRegion, Currency, ExchangeRate, ExpenseId, Invoice,
    InvoiceNumber, MilestoneId, ProfileDraft, Project, ProjectId, ReturnKind, ReturnTerms,
    TimeEntry,
};
use compter_core::input;
use compter_core::services::{
    CreditDraft, ExpenseDraft, ExpenseService, InvoiceService, MilestoneDraft, ProjectDraft,
    ProjectService, ReportService, ServiceError,
};
use compter_core::storage::sync::{self, Lease, Resolution, SyncDir, SyncError};
use compter_core::storage::{DataDir, RedbRepository, Store};

use super::expenses::{category, fiscal_year};
use super::profile::currency;
use super::reports::month_label;
use super::{
    ApiError, CreditInput, CreditPreviewRow, CreditedRow, ExpenseInput, ExpenseList, ExpenseRow,
    ExportRow, InvoiceDetail, InvoiceList, InvoicePreviewRow, InvoiceRow, IssuedRow,
    MilestoneInput, PaymentInput, ProfileInput, ProfileRow, ProjectDetail, ProjectInput,
    ProjectRow, ReceivedRow, ReportRow, SentRow, SyncStatusRow, TimeInput,
};

/// The open database and the services over it. Dart opens one at start-up and keeps it for the
/// life of the app; only one can have the database file open. Handing the data to or from the
/// computer closes and reopens it.
#[frb(opaque)]
pub struct CompterApp {
    projects: ProjectService<RedbRepository>,
    invoices: InvoiceService<RedbRepository>,
    expenses: ExpenseService<RedbRepository>,
    reports: ReportService<RedbRepository>,
    store: Store,
    dir: DataDir,
    /// In memory: nothing to hand off
    demo: bool,
}

impl CompterApp {
    /// Open (or create) `compter.redb` in `data_dir`, the app's private support folder. PDFs
    /// and returns are written under it too.
    pub fn open(data_dir: String) -> Result<Self, ApiError> {
        let dir = DataDir::new(data_dir);
        let store = Store::open(&dir).map_err(ServiceError::from)?;
        Ok(Self::over(store, dir, false))
    }

    /// The sample business of `compter --demo`, in memory; nothing is saved. Its PDFs and
    /// returns are written to `scratch_dir`, a cache folder.
    pub fn demo(scratch_dir: String) -> Result<Self, ApiError> {
        Ok(Self::over(demo::store()?, DataDir::new(scratch_dir), true))
    }

    fn over(store: Store, dir: DataDir, demo: bool) -> Self {
        Self {
            projects: ProjectService::new(store.repository()),
            invoices: InvoiceService::new(store.repository(), Some(dir.clone())),
            expenses: ExpenseService::new(store.repository()),
            reports: ReportService::new(store.repository(), Some(dir.clone())),
            store,
            dir,
            demo,
        }
    }

    /// Close the database, run `step` on its folder, and open it again, whether or not `step`
    /// worked. Should reopening fail, the app is left on an empty in-memory database, and the
    /// error says to restart.
    fn closed<T>(
        &mut self,
        step: impl FnOnce(&DataDir) -> Result<T, SyncError>,
    ) -> Result<T, ApiError> {
        if self.demo {
            return Err(ServiceError::NoSyncInDemo.into());
        }
        let dir = self.dir.clone();
        let stand_in = Self::over(
            Store::in_memory().map_err(ServiceError::from)?,
            dir.clone(),
            true,
        );
        let Self {
            projects,
            invoices,
            expenses,
            reports,
            store,
            ..
        } = std::mem::replace(self, stand_in);
        // Every repository goes before the store can close
        drop((projects, invoices, expenses, reports));
        store.close().map_err(ServiceError::from)?;
        let result = step(&dir);
        let store = Store::open(&dir).map_err(|e| ApiError {
            field: None,
            message: format!("{e}; restart compter"),
        })?;
        *self = Self::over(store, dir, false);
        Ok(result.map_err(ServiceError::from)?)
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
        Ok(ProjectDetail::from(
            &self.projects.create(project_draft(input)?)?,
        ))
    }

    /// Change a project's details and billing; the currency is fixed once something is invoiced
    pub fn edit_project(
        &mut self,
        id: String,
        input: ProjectInput,
    ) -> Result<ProjectDetail, ApiError> {
        let id = project_id(&id)?;
        Ok(ProjectDetail::from(
            &self.projects.update_details(id, project_draft(input)?)?,
        ))
    }

    /// Delete a project with nothing invoiced in it
    pub fn delete_project(&mut self, id: String) -> Result<(), ApiError> {
        Ok(self.projects.delete(project_id(&id)?)?)
    }

    pub fn add_milestone(
        &mut self,
        project_id: String,
        input: MilestoneInput,
    ) -> Result<ProjectDetail, ApiError> {
        let id = self::project_id(&project_id)?;
        let draft = milestone_draft(&self.projects.load(id)?, input)?;
        Ok(ProjectDetail::from(
            &self.projects.add_milestone(id, draft)?,
        ))
    }

    /// Change a pending milestone
    pub fn edit_milestone(
        &mut self,
        project_id: String,
        milestone_id: String,
        input: MilestoneInput,
    ) -> Result<ProjectDetail, ApiError> {
        let id = self::project_id(&project_id)?;
        let project = self.projects.load(id)?;
        let index = index_of(&project, &milestone_id)?;
        let draft = milestone_draft(&project, input)?;
        Ok(ProjectDetail::from(
            &self.projects.edit_milestone(id, index, draft)?,
        ))
    }

    /// Remove a milestone that has not been invoiced
    pub fn remove_milestone(
        &mut self,
        project_id: String,
        milestone_id: String,
    ) -> Result<ProjectDetail, ApiError> {
        let id = self::project_id(&project_id)?;
        let index = index_of(&self.projects.load(id)?, &milestone_id)?;
        Ok(ProjectDetail::from(
            &self.projects.remove_milestone(id, index)?,
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
        let date = date(&input.date, "date")?;
        let minutes = input::duration_minutes(&input.duration)
            .map_err(|e| ApiError::invalid("duration", e))?;
        let entry = TimeEntry::new(date, minutes, input.note.trim())
            .map_err(|e| ApiError::invalid("duration", e.to_string()))?;
        let index = index_of(&self.projects.load(id)?, &milestone_id)?;
        Ok(ProjectDetail::from(
            &self.projects.log_time(id, index, entry)?,
        ))
    }

    /// Remove the last time entry logged on a pending milestone
    pub fn undo_time(
        &mut self,
        project_id: String,
        milestone_id: String,
    ) -> Result<ProjectDetail, ApiError> {
        let id = self::project_id(&project_id)?;
        let index = index_of(&self.projects.load(id)?, &milestone_id)?;
        Ok(ProjectDetail::from(
            &self.projects.remove_last_time_entry(id, index)?,
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
        self.detail(&self.invoices.load(&InvoiceNumber::from_stored(&number))?)
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
            invoice: self.detail(&issued.invoice)?,
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
        self.detail(&invoice)
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
            invoice: self.detail(&credited.invoice)?,
            pdf_error: credited.pdf.err(),
        })
    }

    /// An invoice as the invoice screen shows it, with the advance its client holds
    fn detail(&self, invoice: &Invoice) -> Result<InvoiceDetail, ApiError> {
        let held = self.invoices.client_advance(invoice.client_name())?;
        Ok(InvoiceDetail {
            client_advance: (!held.is_zero()).then(|| Currency::Inr.display(&held)),
            ..InvoiceDetail::from(invoice)
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

    /// The expenses of the financial year starting in April of `year`, by date
    pub fn expenses(&self, year: i32) -> Result<ExpenseList, ApiError> {
        let year = fiscal_year(year);
        Ok(ExpenseList::new(year, &self.expenses.list(year)?))
    }

    /// Record an expense paid from the bank
    pub fn add_expense(&mut self, input: ExpenseInput) -> Result<ExpenseRow, ApiError> {
        Ok(ExpenseRow::from(&self.expenses.add(expense_draft(input)?)?))
    }

    /// Correct an expense; its ledger posting is replaced with it
    pub fn edit_expense(
        &mut self,
        id: String,
        input: ExpenseInput,
    ) -> Result<ExpenseRow, ApiError> {
        let id = expense_id(&id)?;
        Ok(ExpenseRow::from(
            &self.expenses.edit(id, expense_draft(input)?)?,
        ))
    }

    /// Remove an expense and its ledger posting
    pub fn delete_expense(&mut self, id: String) -> Result<(), ApiError> {
        Ok(self.expenses.delete(expense_id(&id)?)?)
    }

    /// The financial year starting in April of `year`, in numbers
    pub fn report(&self, year: i32) -> Result<ReportRow, ApiError> {
        Ok(ReportRow::from(&self.reports.year(fiscal_year(year))?))
    }

    /// Write quarter `quarter` (1 to 4) of `year`'s GSTR-1 for the GST portal. The B2B part of
    /// a month whose IFF was filed is left out.
    pub fn export_gstr1(
        &self,
        year: i32,
        quarter: u32,
        first_month_filed: bool,
        second_month_filed: bool,
    ) -> Result<ExportRow, ApiError> {
        let quarters = fiscal_year(year).quarters();
        let quarter = usize::try_from(quarter)
            .ok()
            .and_then(|q| q.checked_sub(1))
            .and_then(|q| quarters.get(q).copied())
            .ok_or_else(|| ApiError::invalid("quarter", "choose a quarter, 1 to 4"))?;
        let [first, second, _] = quarter.months();
        let furnished: Vec<(i32, u32)> = [(first_month_filed, first), (second_month_filed, second)]
            .into_iter()
            .filter_map(|(filed, month)| filed.then_some(month))
            .collect();
        let (path, gstr1) = self.reports.export_gstr1(quarter, &furnished)?;
        Ok(ExportRow {
            path: path.to_string_lossy().into_owned(),
            summary: format!(
                "GSTR-1 {} {}: {}",
                quarter.label(),
                quarter.year().label(),
                gstr1.summary()
            ),
        })
    }

    /// Write the IFF for `month`, a position in `ReportRow::iff_months` of `year`
    pub fn export_iff(&self, year: i32, month: u32) -> Result<ExportRow, ApiError> {
        let months = fiscal_year(year).iff_months();
        let month = usize::try_from(month)
            .ok()
            .and_then(|m| months.get(m).copied())
            .ok_or_else(|| ApiError::invalid("month", "choose a month"))?;
        let (path, iff) = self.reports.export_iff(month)?;
        Ok(ExportRow {
            path: path.to_string_lossy().into_owned(),
            summary: format!("IFF {}: {}", month_label(month), iff.summary()),
        })
    }

    /// Where this phone's data stands against the computer's sync folder
    pub fn sync_status(&self) -> Result<SyncStatusRow, ApiError> {
        if self.demo {
            return Err(ServiceError::NoSyncInDemo.into());
        }
        let state = sync::state(&self.dir, &self.store).map_err(ServiceError::from)?;
        Ok(SyncStatusRow::from(state))
    }

    /// This phone's lease, as the JSON compter on the computer reads from `lease.json`: write
    /// it to the sync folder while the app is in use, and refresh it every minute
    pub fn lease_json(&self, device_name: String) -> Result<String, ApiError> {
        if self.demo {
            return Err(ServiceError::NoSyncInDemo.into());
        }
        let lease = sync::lease_for(&self.dir, &device_name).map_err(ServiceError::from)?;
        serde_json::to_string_pretty(&lease).map_err(|e| ApiError {
            field: None,
            message: e.to_string(),
        })
    }

    /// Who has compter open, from the `lease.json` found in the sync folder: `compter is open
    /// on laptop (last seen 14:05)`, or `None` when it is this phone's lease or a stale one
    pub fn lease_holder(&self, lease_json: String) -> Result<Option<String>, ApiError> {
        if self.demo {
            return Err(ServiceError::NoSyncInDemo.into());
        }
        let Ok(lease) = serde_json::from_str::<Lease>(&lease_json) else {
            // Half-synced or garbled: nobody can be said to hold it
            return Ok(None);
        };
        let held = sync::held_by_other(&self.dir, &lease).map_err(ServiceError::from)?;
        Ok(held.map(|lease| {
            format!(
                "compter is open on {} (last seen {})",
                lease.device_name,
                lease.heartbeat.with_timezone(&Local).format("%H:%M")
            )
        }))
    }

    /// Take the snapshot in `folder`, where Dart copied `compter.redb` and `manifest.json` from
    /// the computer's sync folder. A new install's empty database is simply replaced.
    pub fn receive_snapshot(&mut self, folder: String) -> Result<ReceivedRow, ApiError> {
        let blank = self.store.is_blank().map_err(ServiceError::from)?;
        let from = SyncDir::new(folder);
        let received = self.closed(|dir| match sync::receive(dir, &from, blank) {
            Err(SyncError::Conflict { device, .. }) => Ok(ReceivedRow {
                message: format!(
                    "This phone and {device} both changed the data since they last synced. Keep this phone's, or take {device}'s?"
                ),
                conflict: true,
            }),
            Err(error @ SyncError::Undelivered { .. }) => Ok(ReceivedRow {
                message: error.to_string(),
                conflict: true,
            }),
            Ok(Some(generation)) => Ok(ReceivedRow {
                message: format!("Took snapshot {generation} from the computer"),
                conflict: false,
            }),
            Ok(None) => Ok(ReceivedRow {
                message: "Already up to date with the computer".to_owned(),
                conflict: false,
            }),
            Err(error) => Err(error),
        })?;
        Ok(received)
    }

    /// Settle what `receive_snapshot` couldn't: keep this phone's data (send it next), or take
    /// the snapshot in `folder`, keeping this phone's database beside it as a backup
    pub fn settle_snapshot(
        &mut self,
        folder: String,
        keep_this_phone: bool,
        device_name: String,
    ) -> Result<String, ApiError> {
        let from = SyncDir::new(folder);
        let resolution = if keep_this_phone {
            Resolution::KeepLocal
        } else {
            Resolution::KeepRemote
        };
        self.closed(|dir| sync::settle_received(dir, &from, &device_name, resolution))?;
        Ok(if keep_this_phone {
            "Kept this phone's data: send it to the computer next".to_owned()
        } else {
            "Took the computer's data; this phone's is kept as a backup".to_owned()
        })
    }

    /// Write this phone's database to `folder` as `compter.redb` and `manifest.json`, for the
    /// user to save into the computer's sync folder
    pub fn send_snapshot(
        &mut self,
        folder: String,
        device_name: String,
    ) -> Result<SentRow, ApiError> {
        let to = SyncDir::new(folder);
        let generation = self.closed(|dir| sync::send(dir, &to, &device_name))?;
        Ok(SentRow {
            snapshot: to.snapshot_path().to_string_lossy().into_owned(),
            manifest: to.manifest_path().to_string_lossy().into_owned(),
            message: format!(
                "Snapshot {generation} is ready: save both files into your compter sync folder, replacing the ones there"
            ),
        })
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

/// Where milestone `id` is in `project`, as the project service takes it
fn index_of(project: &Project, id: &str) -> Result<usize, ApiError> {
    project
        .milestone_index(milestone_id(id)?)
        .ok_or_else(|| ApiError::invalid("milestone_id", "no such milestone"))
}

fn project_draft(input: ProjectInput) -> Result<ProjectDraft, ApiError> {
    let billing = match input.hourly_rate.as_deref().and_then(input::optional) {
        Some(rate) => {
            let rate = input::money(&rate).map_err(|e| ApiError::invalid("hourly_rate", e))?;
            Billing::hourly(rate).map_err(|e| ApiError::invalid("hourly_rate", e.to_string()))?
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
    Ok(ProjectDraft {
        name: input.name.trim().to_owned(),
        client: ClientDetails {
            name: input.client_name.trim().to_owned(),
            // Upper-cased as the terminal does; a GSTIN on a client abroad is refused by the
            // domain rather than dropped
            gstin: optional(&input.client_gstin).map(|g| g.to_uppercase()),
            address: input.client_address,
            region,
            currency,
        },
        billing,
    })
}

/// A milestone for `project`: an hourly one ignores any amount, it comes from logged time
fn milestone_draft(project: &Project, input: MilestoneInput) -> Result<MilestoneDraft, ApiError> {
    let hourly = matches!(project.billing(), Billing::Hourly { .. });
    let amount = match (hourly, input.amount.as_deref()) {
        (true, _) => None,
        (false, amount) => Some(
            input::money(amount.unwrap_or_default()).map_err(|e| ApiError::invalid("amount", e))?,
        ),
    };
    Ok(MilestoneDraft {
        name: input.name.trim().to_owned(),
        due_date: date(&input.due_date, "due_date")?,
        amount,
    })
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
        returned: ReturnTerms {
            kind: if input.keep_as_advance {
                ReturnKind::Advance
            } else {
                ReturnKind::Refunded
            },
            refunded_inr: optional_money(&input.refunded_inr, "refunded_inr")?,
            tds_reversed: optional_money(&input.tds_reversed, "tds_reversed")?,
        },
    })
}

/// A money field that may be left blank
fn optional_money(value: &str, field: &'static str) -> Result<Option<BigDecimal>, ApiError> {
    input::optional(value)
        .map(|amount| input::money(&amount).map_err(|e| ApiError::invalid(field, e)))
        .transpose()
}

fn expense_id(id: &str) -> Result<ExpenseId, ApiError> {
    id.parse()
        .map_err(|_| ApiError::invalid("expense_id", "not an expense id"))
}

fn expense_draft(input: ExpenseInput) -> Result<ExpenseDraft, ApiError> {
    Ok(ExpenseDraft {
        date: date(&input.date, "date")?,
        category: category(&input.category)
            .ok_or_else(|| ApiError::invalid("category", "choose a category"))?,
        description: input.description.trim().to_owned(),
        amount: input::money(&input.amount).map_err(|e| ApiError::invalid("amount", e))?,
        gst_credit: match input::optional(&input.gst_credit) {
            Some(credit) => {
                input::money(&credit).map_err(|e| ApiError::invalid("gst_credit", e))?
            }
            None => BigDecimal::zero(),
        },
    })
}
