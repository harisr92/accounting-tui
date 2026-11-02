use accounting_core::StateCode;
use bigdecimal::{BigDecimal, Zero};
use chrono::{Local, NaiveDate};
use flutter_rust_bridge::frb;

use compter_core::demo;
use compter_core::domain::{
    BasisSchedule, Billing, ClientDetails, ClientId, ClientRegion, Currency, Discount,
    ExchangeRate, ExpenseId, IncomeBasis, Invoice, InvoiceNumber, MilestoneId, ProfileDraft,
    Project, ProjectId, ProjectSummary, ReturnKind, ReturnTerms, TimeEntry,
};
use compter_core::input;
use compter_core::services::{
    ClientService, CreditDraft, CreditLineDraft, DashboardService, ExpenseDraft, ExpenseService,
    InvoiceService, MilestoneDraft, ProjectDraft, ProjectService, ReportService, ServiceError,
    creditable_lines_of,
};
use compter_core::storage::sync::{self, Lease, Resolution, SyncDir, SyncError};
use compter_core::storage::{DataDir, RedbRepository, Store};

use super::error::Form;
use super::expenses::{category, fiscal_year};
use super::profile::{currency, this_year};
use super::reports::month_label;
use super::{
    ApiError, ClientChoiceRow, ClientDetail, ClientInput, ClientSummaryRow, CreditInput,
    CreditLineInput, CreditPreviewRow, CreditedRow, DashboardRow, ExpenseInput, ExpenseList,
    ExpenseRow, ExportRow, InputField, InvoiceDetail, InvoiceList, InvoicePreviewRow, InvoiceRow,
    IssuedRow, MilestoneInput, PaymentInput, ProfileInput, ProfileRow, ProjectDetail, ProjectInput,
    ProjectRow, ReceivedRow, ReportRow, SentRow, SyncStatusRow, TimeInput,
};

/// The open database and the services over it. Dart opens one at start-up and keeps it for the
/// life of the app; only one can have the database file open. Handing the data to or from the
/// computer closes and reopens it.
#[frb(opaque)]
pub struct CompterApp {
    projects: ProjectService<RedbRepository>,
    clients: ClientService<RedbRepository>,
    invoices: InvoiceService<RedbRepository>,
    expenses: ExpenseService<RedbRepository>,
    reports: ReportService<RedbRepository>,
    dashboard: DashboardService<RedbRepository>,
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
            clients: ClientService::new(store.repository()),
            invoices: InvoiceService::new(store.repository(), Some(dir.clone())),
            expenses: ExpenseService::new(store.repository()),
            reports: ReportService::new(store.repository(), Some(dir.clone())),
            dashboard: DashboardService::new(store.repository()),
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
            clients,
            invoices,
            expenses,
            reports,
            dashboard,
            store,
            ..
        } = std::mem::replace(self, stand_in);
        // Every repository goes before the store can close
        drop((projects, clients, invoices, expenses, reports, dashboard));
        store.close().map_err(ServiceError::from)?;
        let result = step(&dir);
        let store =
            Store::open(&dir).map_err(|e| ApiError::general(format!("{e}; restart compter")))?;
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

    /// Every client billed before, by name, to fill a new project's client from
    pub fn clients(&self) -> Result<Vec<ClientChoiceRow>, ApiError> {
        Ok(self
            .projects
            .clients()?
            .iter()
            .map(ClientChoiceRow::from)
            .collect())
    }

    /// The Clients screen: every saved client with their projects counted and what they owe
    pub fn client_list(&self) -> Result<Vec<ClientSummaryRow>, ApiError> {
        Ok(self
            .clients
            .list()?
            .iter()
            .map(ClientSummaryRow::from)
            .collect())
    }

    /// One client with their projects and invoices
    pub fn client(&self, id: String) -> Result<ClientDetail, ApiError> {
        Ok(ClientDetail::from(&self.clients.account(client_id(&id)?)?))
    }

    /// The names of the client's open projects that the corrected details in `input` would
    /// change: asked about before [`edit_client`](Self::edit_client). Details that would be
    /// refused are refused here.
    pub fn client_projects_to_update(
        &self,
        id: String,
        input: ClientInput,
    ) -> Result<Vec<String>, ApiError> {
        let id = client_id(&id)?;
        let details = client_details(self.clients.client(id)?.name(), input)?;
        let stale = self
            .clients
            .projects_to_update(id, &details)
            .map_err(ApiError::on(Form::Client))?;
        Ok(project_names(&stale))
    }

    /// Correct a client's GSTIN, place, address or currency, and with `update_projects` their
    /// open projects' copies too (each keeps its own currency). The name can't change, and
    /// issued invoices never do.
    pub fn edit_client(
        &mut self,
        id: String,
        input: ClientInput,
        update_projects: bool,
    ) -> Result<ClientDetail, ApiError> {
        let id = client_id(&id)?;
        let details = client_details(self.clients.client(id)?.name(), input)?;
        let account = self
            .clients
            .update(id, details, update_projects)
            .map_err(ApiError::on(Form::Client))?;
        Ok(ClientDetail::from(&account))
    }

    /// The names of the client's open projects that renaming them `name` would change: asked
    /// about before [`rename_client`](Self::rename_client). A name another client goes by is
    /// refused here.
    pub fn client_projects_to_rename(
        &self,
        id: String,
        name: String,
    ) -> Result<Vec<String>, ApiError> {
        let id = client_id(&id)?;
        let stale = self
            .clients
            .projects_to_rename(id, &name)
            .map_err(ApiError::on(Form::RenameClient))?;
        Ok(project_names(&stale))
    }

    /// Call a client `name` from now on, and with `update_projects` their open projects too.
    /// The old name stays on the client as an alias: issued invoices keep it.
    pub fn rename_client(
        &mut self,
        id: String,
        name: String,
        update_projects: bool,
    ) -> Result<ClientDetail, ApiError> {
        let id = client_id(&id)?;
        let account = self
            .clients
            .rename(id, &name, update_projects)
            .map_err(ApiError::on(Form::RenameClient))?;
        Ok(ClientDetail::from(&account))
    }

    /// The names of the open projects of `from` that merging it into `into` would give
    /// `into`'s details: asked about before [`merge_client`](Self::merge_client). A merge that
    /// would be refused is refused here.
    pub fn client_projects_to_merge(
        &self,
        from: String,
        into: String,
    ) -> Result<Vec<String>, ApiError> {
        let (from, into) = (client_id(&from)?, client_id(&into)?);
        Ok(project_names(&self.clients.projects_to_merge(from, into)?))
    }

    /// Merge the client `from`, a duplicate, into `into`, whose details are kept; with
    /// `update_projects`, `from`'s open projects take them too. Returns `into`.
    pub fn merge_client(
        &mut self,
        from: String,
        into: String,
        update_projects: bool,
    ) -> Result<ClientDetail, ApiError> {
        let (from, into) = (client_id(&from)?, client_id(&into)?);
        Ok(ClientDetail::from(&self.clients.merge(
            from,
            into,
            update_projects,
        )?))
    }

    /// Take a client out of the address book. Refused while any project (archived included) or
    /// invoice is still theirs; a new project under their name adds them again.
    pub fn remove_client(&mut self, id: String) -> Result<(), ApiError> {
        Ok(self.clients.remove(client_id(&id)?)?)
    }

    pub fn create_project(&mut self, input: ProjectInput) -> Result<ProjectDetail, ApiError> {
        let project = self
            .projects
            .create(project_draft(input)?)
            .map_err(ApiError::on(Form::Project))?;
        Ok(ProjectDetail::from(&project))
    }

    /// Change a project's details and billing; the currency is fixed once something is invoiced
    pub fn edit_project(
        &mut self,
        id: String,
        input: ProjectInput,
    ) -> Result<ProjectDetail, ApiError> {
        let id = project_id(&id)?;
        let project = self
            .projects
            .update_details(id, project_draft(input)?)
            .map_err(ApiError::on(Form::Project))?;
        Ok(ProjectDetail::from(&project))
    }

    /// Delete a project with nothing invoiced in it
    pub fn delete_project(&mut self, id: String) -> Result<(), ApiError> {
        Ok(self.projects.delete(project_id(&id)?)?)
    }

    /// Archive a finished project, hiding it from the list, or restore an archived one
    pub fn set_archived(&mut self, id: String, archived: bool) -> Result<ProjectDetail, ApiError> {
        let id = project_id(&id)?;
        Ok(ProjectDetail::from(
            &self.projects.set_archived(id, archived)?,
        ))
    }

    pub fn add_milestone(
        &mut self,
        project_id: String,
        input: MilestoneInput,
    ) -> Result<ProjectDetail, ApiError> {
        let id = self::project_id(&project_id)?;
        let draft = milestone_draft(&self.projects.load(id)?, input)?;
        let project = self
            .projects
            .add_milestone(id, draft)
            .map_err(ApiError::on(Form::Milestone))?;
        Ok(ProjectDetail::from(&project))
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
        let project = self
            .projects
            .edit_milestone(id, index, draft)
            .map_err(ApiError::on(Form::Milestone))?;
        Ok(ProjectDetail::from(&project))
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
        let date = date(&input.date, InputField::Date)?;
        let minutes = input::duration_minutes(&input.duration)
            .map_err(|e| ApiError::invalid(InputField::Duration, e))?;
        let entry = TimeEntry::new(date, minutes, input.note.trim())
            .map_err(|e| ApiError::invalid(InputField::Duration, e.to_string()))?;
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
        let currency = currency(&input.currency).ok_or_else(|| {
            ApiError::invalid(InputField::Currency, "choose INR, USD, GBP or EUR")
        })?;
        let lines = |lines: Vec<String>| {
            lines
                .iter()
                .filter_map(|line| input::optional(line))
                .collect::<Vec<_>>()
        };
        let saved = self.invoices.profile()?;
        let from = input::fiscal_year_or(&input.basis_from, this_year())
            .map_err(|message| ApiError::invalid(InputField::BasisFrom, message))?;
        let basis = if input.cash_basis {
            IncomeBasis::Cash
        } else {
            IncomeBasis::Accrual
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
            logo: saved.and_then(|p| p.logo()),
            // Set from the stored schedule by `save_profile_with_basis`
            income_basis: BasisSchedule::default(),
        };
        let profile = self
            .invoices
            .save_profile_with_basis(draft, from, basis)
            .map_err(ApiError::on(Form::Profile))?;
        Ok(ProfileRow::from(&profile))
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

    /// The invoice pending milestones of a project would get together today, without saving
    /// anything. `rate` (rupees per unit) is needed when the project is billed in a foreign
    /// currency; `discount` (`5000` or `10%`, blank for none) comes off before GST.
    pub fn preview_invoice(
        &self,
        project_id: String,
        milestone_ids: Vec<String>,
        rate: Option<String>,
        discount: Option<String>,
    ) -> Result<InvoicePreviewRow, ApiError> {
        let discount = discount_of(discount.as_deref())?;
        let preview = self
            .invoices
            .preview(
                self::project_id(&project_id)?,
                &milestone_ids_of(&milestone_ids)?,
                today(),
                exchange_rate(rate.as_deref())?,
                discount.as_ref(),
            )
            .map_err(ApiError::on(Form::Issue))?;
        Ok(InvoicePreviewRow::from(&preview))
    }

    /// Issue today's invoice billing pending milestones of a project, a line each, and write
    /// its PDF
    pub fn issue_invoice(
        &mut self,
        project_id: String,
        milestone_ids: Vec<String>,
        rate: Option<String>,
        discount: Option<String>,
    ) -> Result<IssuedRow, ApiError> {
        let id = self::project_id(&project_id)?;
        let discount = discount_of(discount.as_deref())?;
        let issued = self
            .invoices
            .issue(
                id,
                &milestone_ids_of(&milestone_ids)?,
                today(),
                exchange_rate(rate.as_deref())?,
                discount.as_ref(),
            )
            .map_err(ApiError::on(Form::Issue))?;
        // The service returns only the invoice; the milestones it invoiced are in the project,
        // read once for both
        let project = self.projects.load(id)?;
        Ok(IssuedRow {
            invoice: self.detail_in(&issued.invoice, &project)?,
            project: ProjectDetail::from(&project),
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
        let date = date(&input.date, InputField::Date)?;
        let received = input::money(&input.received)
            .map_err(|e| ApiError::invalid(InputField::Received, e))?;
        let third = input::money(&input.tds_or_banked)
            .map_err(|e| ApiError::invalid(InputField::TdsOrBanked, e))?;
        let invoice = if self.invoices.load(&number)?.currency().is_inr() {
            self.invoices.record_payment(&number, date, received, third)
        } else {
            self.invoices
                .record_foreign_payment(&number, date, received, third)
        }
        .map_err(ApiError::on(Form::Payment))?;
        self.detail(&invoice)
    }

    /// The credit note `input` would make against invoice `number`, without saving
    pub fn preview_credit(
        &self,
        number: String,
        input: CreditInput,
    ) -> Result<CreditPreviewRow, ApiError> {
        let number = InvoiceNumber::from_stored(&number);
        let (draft, form) = self.checked_draft(&number, input)?;
        let preview = self
            .invoices
            .preview_credit(&number, draft)
            .map_err(ApiError::on(form))?;
        Ok(CreditPreviewRow::from(&preview))
    }

    /// Check the lines picked for a credit note against invoice `number` before asking when and
    /// why: each amount is a number, more than zero and no more than is left on its line. A
    /// refusal names the line ([`InputField::CreditLine`], with its milestone as `line`).
    pub fn check_credit_lines(
        &self,
        number: String,
        lines: Vec<CreditLineInput>,
    ) -> Result<(), ApiError> {
        let invoice = self.invoices.load(&InvoiceNumber::from_stored(&number))?;
        Ok(self
            .invoices
            .check_credit_lines(&invoice, &credit_lines(&lines)?)?)
    }

    /// `input` as a credit note draft, its lines checked first so a refusal names its line
    /// even when issuing (another device may have credited it since the preview), with the
    /// form it came from: on an invoice the client withheld TDS from, that form asks for TDS
    /// reversed
    fn checked_draft(
        &self,
        number: &InvoiceNumber,
        input: CreditInput,
    ) -> Result<(CreditDraft, Form), ApiError> {
        let draft = credit_draft(input)?;
        let invoice = self.invoices.load(number)?;
        self.invoices.check_credit_lines(&invoice, &draft.lines)?;
        let tds = invoice.tds_withheld();
        Ok((draft, Form::Credit { tds }))
    }

    /// Issue a credit note against invoice `number` and write its PDF
    pub fn issue_credit(
        &mut self,
        number: String,
        input: CreditInput,
    ) -> Result<CreditedRow, ApiError> {
        let number = InvoiceNumber::from_stored(&number);
        let (draft, form) = self.checked_draft(&number, input)?;
        let credited = self
            .invoices
            .issue_credit(&number, draft)
            .map_err(ApiError::on(form))?;
        Ok(CreditedRow {
            note_number: credited.note.number().to_string(),
            invoice: self.detail(&credited.invoice)?,
            pdf_error: credited.pdf.err(),
        })
    }

    /// An invoice as the invoice screen shows it, with what is left to credit on each line and
    /// the advance its client holds
    fn detail(&self, invoice: &Invoice) -> Result<InvoiceDetail, ApiError> {
        let project = self.projects.load(invoice.project_id())?;
        self.detail_in(invoice, &project)
    }

    /// [`Self::detail`], with the invoice's `project` already loaded
    fn detail_in(&self, invoice: &Invoice, project: &Project) -> Result<InvoiceDetail, ApiError> {
        let held = self.invoices.client_advance(invoice.client_name())?;
        let lines = creditable_lines_of(invoice, project)?;
        Ok(InvoiceDetail {
            client_advance: (!held.is_zero()).then(|| Currency::Inr.display(&held)),
            ..InvoiceDetail::new(invoice, &lines)
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
        let expense = self
            .expenses
            .add(expense_draft(input)?)
            .map_err(ApiError::on(Form::Expense))?;
        Ok(ExpenseRow::from(&expense))
    }

    /// Correct an expense; its ledger posting is replaced with it
    pub fn edit_expense(
        &mut self,
        id: String,
        input: ExpenseInput,
    ) -> Result<ExpenseRow, ApiError> {
        let id = expense_id(&id)?;
        let expense = self
            .expenses
            .edit(id, expense_draft(input)?)
            .map_err(ApiError::on(Form::Expense))?;
        Ok(ExpenseRow::from(&expense))
    }

    /// Remove an expense and its ledger posting
    pub fn delete_expense(&mut self, id: String) -> Result<(), ApiError> {
        Ok(self.expenses.delete(expense_id(&id)?)?)
    }

    /// What needs the user today: money owed, overdue invoices, milestones ready to bill
    pub fn dashboard(&self) -> Result<DashboardRow, ApiError> {
        let today = today();
        Ok(DashboardRow::new(&self.dashboard.now(today)?, today))
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
            .ok_or_else(|| ApiError::invalid(InputField::Quarter, "choose a quarter, 1 to 4"))?;
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
            .ok_or_else(|| ApiError::invalid(InputField::Month, "choose a month"))?;
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
        serde_json::to_string_pretty(&lease).map_err(|e| ApiError::general(e.to_string()))
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
fn date(input: &str, field: InputField) -> Result<NaiveDate, ApiError> {
    input::date_or(input, today()).map_err(|e| ApiError::invalid(field, e))
}

fn project_id(id: &str) -> Result<ProjectId, ApiError> {
    id.parse()
        .map_err(|_| ApiError::invalid(InputField::ProjectId, "not a project id"))
}

/// The projects' names, as a question about them lists them
fn project_names(projects: &[ProjectSummary]) -> Vec<String> {
    projects.iter().map(|p| p.name().to_owned()).collect()
}

fn client_id(id: &str) -> Result<ClientId, ApiError> {
    id.parse()
        .map_err(|_| ApiError::invalid(InputField::ClientId, "not a client id"))
}

fn milestone_id(id: &str) -> Result<MilestoneId, ApiError> {
    id.parse()
        .map_err(|_| ApiError::invalid(InputField::MilestoneId, "not a milestone id"))
}

fn milestone_ids_of(ids: &[String]) -> Result<Vec<MilestoneId>, ApiError> {
    ids.iter().map(|id| milestone_id(id)).collect()
}

/// Where milestone `id` is in `project`, as the project service takes it
fn index_of(project: &Project, id: &str) -> Result<usize, ApiError> {
    project
        .milestone_index(milestone_id(id)?)
        .ok_or_else(|| ApiError::invalid(InputField::MilestoneId, "no such milestone"))
}

fn project_draft(input: ProjectInput) -> Result<ProjectDraft, ApiError> {
    let billing = match input.hourly_rate.as_deref().and_then(input::optional) {
        Some(rate) => {
            let rate =
                input::money(&rate).map_err(|e| ApiError::invalid(InputField::HourlyRate, e))?;
            Billing::hourly(rate)
                .map_err(|e| ApiError::invalid(InputField::HourlyRate, e.to_string()))?
        }
        None => Billing::Fixed,
    };
    let client = ClientInput {
        gstin: input.client_gstin,
        state: input.client_state,
        country: input.client_country,
        address: input.client_address,
        currency: input.currency,
    };
    Ok(ProjectDraft {
        name: input.name.trim().to_owned(),
        client: client_details(&input.client_name, client)?,
        billing,
    })
}

/// The client called `name` as typed; errors name the project form's fields, which the edit
/// client form shares
fn client_details(name: &str, input: ClientInput) -> Result<ClientDetails, ApiError> {
    let optional = |text: &Option<String>| text.as_deref().and_then(input::optional);
    let region = match optional(&input.country) {
        Some(country) => ClientRegion::Abroad { country },
        None => ClientRegion::India {
            state: optional(&input.state)
                .map(|code| StateCode::parse(&code))
                .transpose()
                .map_err(|e| ApiError::invalid(InputField::ClientState, e.to_string()))?,
        },
    };
    let currency = match optional(&input.currency) {
        Some(code) => currency(&code).ok_or_else(|| {
            ApiError::invalid(InputField::Currency, "choose INR, USD, GBP or EUR")
        })?,
        None => Currency::Inr,
    };
    Ok(ClientDetails {
        name: name.trim().to_owned(),
        // Upper-cased as the terminal does; a GSTIN on a client abroad is refused by the
        // domain rather than dropped
        gstin: optional(&input.gstin).map(|g| g.to_uppercase()),
        address: input.address,
        region,
        currency,
    })
}

/// A milestone for `project`: an hourly one ignores any amount, it comes from logged time
fn milestone_draft(project: &Project, input: MilestoneInput) -> Result<MilestoneDraft, ApiError> {
    let hourly = matches!(project.billing(), Billing::Hourly { .. });
    let amount = match (hourly, input.amount.as_deref()) {
        (true, _) => None,
        (false, amount) => Some(
            input::money(amount.unwrap_or_default())
                .map_err(|e| ApiError::invalid(InputField::Amount, e))?,
        ),
    };
    Ok(MilestoneDraft {
        name: input.name.trim().to_owned(),
        due_date: date(&input.due_date, InputField::DueDate)?,
        amount,
    })
}

/// The typed rupees-per-unit rate, when one was asked for
fn exchange_rate(rate: Option<&str>) -> Result<Option<ExchangeRate>, ApiError> {
    rate.and_then(input::optional)
        .map(|rate| {
            let rate = input::money(&rate).map_err(|e| ApiError::invalid(InputField::Rate, e))?;
            ExchangeRate::new(rate).map_err(|e| ApiError::invalid(InputField::Rate, e.to_string()))
        })
        .transpose()
}

fn discount_of(discount: Option<&str>) -> Result<Option<Discount>, ApiError> {
    input::discount(discount.unwrap_or_default())
        .map_err(|e| ApiError::invalid(InputField::Discount, e))
}

/// Credit note lines as typed; a refused one names its line
fn credit_lines(lines: &[CreditLineInput]) -> Result<Vec<CreditLineDraft>, ApiError> {
    lines
        .iter()
        .map(|line| {
            let refused = |message: &str| ApiError::invalid_line(&line.milestone_id, message);
            Ok(CreditLineDraft {
                milestone_id: line
                    .milestone_id
                    .parse()
                    .map_err(|_| refused("not a milestone id"))?,
                taxable_value: input::money(&line.amount).map_err(refused)?,
            })
        })
        .collect()
}

fn credit_draft(input: CreditInput) -> Result<CreditDraft, ApiError> {
    Ok(CreditDraft {
        date: date(&input.date, InputField::Date)?,
        lines: credit_lines(&input.lines)?,
        reason: input.reason.trim().to_owned(),
        returned: ReturnTerms {
            kind: if input.keep_as_advance {
                ReturnKind::Advance
            } else {
                ReturnKind::Refunded
            },
            refunded_inr: optional_money(&input.refunded_inr, InputField::RefundedInr)?,
            tds_reversed: optional_money(&input.tds_reversed, InputField::TdsReversed)?,
        },
    })
}

/// A money field that may be left blank
fn optional_money(value: &str, field: InputField) -> Result<Option<BigDecimal>, ApiError> {
    input::optional(value)
        .map(|amount| input::money(&amount).map_err(|e| ApiError::invalid(field, e)))
        .transpose()
}

fn expense_id(id: &str) -> Result<ExpenseId, ApiError> {
    id.parse()
        .map_err(|_| ApiError::invalid(InputField::ExpenseId, "not an expense id"))
}

fn expense_draft(input: ExpenseInput) -> Result<ExpenseDraft, ApiError> {
    Ok(ExpenseDraft {
        date: date(&input.date, InputField::Date)?,
        category: category(&input.category)
            .ok_or_else(|| ApiError::invalid(InputField::Category, "choose a category"))?,
        description: input.description.trim().to_owned(),
        amount: input::money(&input.amount)
            .map_err(|e| ApiError::invalid(InputField::Amount, e))?,
        gst_credit: match input::optional(&input.gst_credit) {
            Some(credit) => {
                input::money(&credit).map_err(|e| ApiError::invalid(InputField::GstCredit, e))?
            }
            None => BigDecimal::zero(),
        },
    })
}
