//! Building each form (prefilled when editing) and turning a submitted form into a saved change

use chrono::{Local, NaiveDate};

use super::{FormKind, ProjectsView};
use accounting_core::StateCode;

use crate::ui::form::{Field, Form, parse};
use crate::ui::view::Notice;
use crate::ui::views::payment::submit_payment;
use crate::ui::widgets::duration;
use compter_core::domain::{
    Billing, ClientDetails, ClientRegion, Currency, ExchangeRate, MAX_ENTRY_MINUTES, Milestone,
    Project, TimeEntry, state_name,
};
use compter_core::services::{MilestoneDraft, ProjectDraft};
use compter_core::storage::Repository;

const BILLING_OPTIONS: &[&str] = &["Fixed price", "Hourly"];
const HOURLY: usize = 1;
const REGION_OPTIONS: &[&str] = &["India", "Abroad"];
const ABROAD: usize = 1;

/// Field positions, so building and reading a form agree
mod project_field {
    pub const NAME: usize = 0;
    pub const CLIENT: usize = 1;
    pub const GSTIN: usize = 2;
    pub const REGION: usize = 3;
    pub const STATE: usize = 4;
    pub const COUNTRY: usize = 5;
    pub const ADDRESS_1: usize = 6;
    pub const ADDRESS_2: usize = 7;
    pub const CURRENCY: usize = 8;
    pub const BILLING: usize = 9;
    pub const RATE: usize = 10;
}

mod milestone_field {
    pub const NAME: usize = 0;
    pub const DUE: usize = 1;
    /// Present on fixed-price projects only
    pub const AMOUNT: usize = 2;
}

mod time_field {
    pub const DATE: usize = 0;
    pub const TIME: usize = 1;
    pub const NOTE: usize = 2;
}

impl<R: Repository> ProjectsView<R> {
    /// The form for `kind`; `None` when it needs a selected project or milestone that isn't there
    pub(super) fn build_form(&self, kind: &FormKind) -> Option<Form> {
        let project = self.selected.as_ref();
        let hourly = project.is_some_and(|p| p.billing().rate().is_some());
        Some(match *kind {
            FormKind::NewProject => {
                // A broken or missing profile only loses the default; INR is always valid
                let currency = self
                    .invoices
                    .profile()
                    .ok()
                    .flatten()
                    .map_or(Currency::Inr, |p| p.currency());
                project_form("New project", None, currency)
            }
            FormKind::EditProject => {
                let project = project?;
                project_form("Edit project", Some(project), project.currency())
            }
            FormKind::AddMilestone => {
                project?;
                milestone_form("Add milestone", hourly, None)
            }
            FormKind::EditMilestone(index) => {
                let milestone = project?.milestones().get(index)?;
                milestone_form("Edit milestone", hourly, Some(milestone))
            }
            FormKind::LogTime(index) => {
                project?.milestones().get(index)?;
                time_form()
            }
            FormKind::Search => self.filter.form("project or client name"),
            // Built from the stored invoice by `open_payment`, and by `confirm_issue`
            FormKind::Payment(_) | FormKind::IssueRate(..) => return None,
        })
    }

    /// Save the form's change; on `Err` the message is shown in the still-open form
    pub(super) fn submit(&mut self, kind: &FormKind, form: &Form) -> Result<Notice, String> {
        let today = Local::now().date_naive();
        let id = self.selected.as_ref().map(Project::id);
        let hourly = self
            .selected
            .as_ref()
            .is_some_and(|p| p.billing().rate().is_some());
        let no_project = || "No project is selected".to_owned();

        // A new milestone is selected wherever the sort lists it
        let mut added = None;
        let (project, notice) = match kind.clone() {
            FormKind::NewProject => {
                let project = self.service.create(project_draft(form)?).map_err(fail)?;
                (project, Notice::success("Project created"))
            }
            FormKind::EditProject => {
                let id = id.ok_or_else(no_project)?;
                let project = self
                    .service
                    .update_details(id, project_draft(form)?)
                    .map_err(fail)?;
                (project, Notice::success("Project saved"))
            }
            FormKind::AddMilestone => {
                let id = id.ok_or_else(no_project)?;
                let draft = milestone_draft(form, hourly, today)?;
                let project = self.service.add_milestone(id, draft).map_err(fail)?;
                added = project.milestones().last().map(Milestone::id);
                (project, Notice::success("Milestone added"))
            }
            FormKind::EditMilestone(index) => {
                let id = id.ok_or_else(no_project)?;
                let draft = milestone_draft(form, hourly, today)?;
                let project = self
                    .service
                    .edit_milestone(id, index, draft)
                    .map_err(fail)?;
                (project, Notice::success("Milestone saved"))
            }
            FormKind::LogTime(index) => {
                let id = id.ok_or_else(no_project)?;
                let entry = time_entry(form, today)?;
                let logged = format!("Logged {}", duration(entry.minutes()));
                let project = self.service.log_time(id, index, entry).map_err(fail)?;
                let name = project.milestones().get(index).map_or("", Milestone::name);
                let notice = Notice::success(format!("{logged} on {name}"));
                (project, notice)
            }
            // Opens the confirm; nothing changes until it is answered
            FormKind::IssueRate(id, milestones) => {
                let rate = ExchangeRate::new(parse::money(form.value(0))?).map_err(fail)?;
                self.preview_issue(id, milestones, Some(rate))?;
                return Ok(Notice::info("Check the invoice before issuing it"));
            }
            FormKind::Search => return self.search(form),
            FormKind::Payment(number) => {
                let notice = submit_payment(&mut self.invoices, &number, form)?;
                let id = id.ok_or_else(no_project)?;
                (self.service.load(id).map_err(fail)?, notice)
            }
        };
        self.refresh(Some(project)).map_err(fail)?;
        if let Some(added) = added {
            self.select_milestone(added);
        }
        Ok(notice)
    }
}

fn fail(error: impl ToString) -> String {
    error.to_string()
}

fn project_form(title: &'static str, existing: Option<&Project>, currency: Currency) -> Form {
    let client = existing.map(Project::client_details).unwrap_or_default();
    let (region, state, country) = match &client.region {
        ClientRegion::India { state } => (0, state.as_ref().map(|s| s.as_str().to_owned()), None),
        ClientRegion::Abroad { country } => (ABROAD, None, Some(country.clone())),
    };
    let address = |line: usize| client.address.get(line).cloned().unwrap_or_default();
    let billing = existing.map_or(0, |p| usize::from(p.billing().rate().is_some()));
    let rate = existing
        .and_then(|p| p.billing().rate())
        .map(ToString::to_string)
        .unwrap_or_default();
    Form::new(
        title,
        vec![
            Field::text("Name", "e.g. Website revamp")
                .with_value(existing.map_or("", Project::name)),
            Field::text("Client", "client or company name").with_value(client.name.clone()),
            Field::text("GSTIN", "blank if unregistered or abroad")
                .with_value(client.gstin.clone().unwrap_or_default()),
            Field::choice("Client in", REGION_OPTIONS).with_choice(region),
            Field::text("State code", "India, no GSTIN: e.g. 29")
                .with_value(state.unwrap_or_default())
                .with_preview(describe_state),
            Field::text("Country", "abroad only, e.g. Germany")
                .with_value(country.unwrap_or_default()),
            Field::text("Address", "street, building").with_value(address(0)),
            Field::text("", "city, PIN / postcode").with_value(address(1)),
            Field::choice("Currency", Currency::LABELS).with_choice(currency.index()),
            Field::choice("Billing", BILLING_OPTIONS).with_choice(billing),
            Field::text("Rate / hour", "hourly only, in the currency above").with_value(rate),
        ],
    )
}

/// Name the state a typed GST state code stands for
fn describe_state(input: &str) -> String {
    state_name(input.trim()).map_or_else(|| "not a GST state code".to_owned(), str::to_owned)
}

fn project_draft(form: &Form) -> Result<ProjectDraft, String> {
    use project_field::{
        ADDRESS_1, ADDRESS_2, BILLING, CLIENT, COUNTRY, CURRENCY, GSTIN, NAME, RATE, REGION, STATE,
    };

    let billing = if form.choice(BILLING) == HOURLY {
        Billing::hourly(parse::money(form.value(RATE))?).map_err(fail)?
    } else {
        Billing::Fixed
    };
    let region = if form.choice(REGION) == ABROAD {
        ClientRegion::Abroad {
            country: form.value(COUNTRY).trim().to_owned(),
        }
    } else {
        let state = parse::optional(form.value(STATE))
            .map(|code| StateCode::parse(&code))
            .transpose()
            .map_err(fail)?;
        ClientRegion::India { state }
    };
    // A GSTIN on a client abroad is refused by the domain rather than silently dropped
    let gstin = parse::optional(form.value(GSTIN)).map(|g| g.to_uppercase());
    Ok(ProjectDraft {
        name: form.value(NAME).trim().to_owned(),
        client: ClientDetails {
            name: form.value(CLIENT).trim().to_owned(),
            gstin,
            address: vec![
                form.value(ADDRESS_1).to_owned(),
                form.value(ADDRESS_2).to_owned(),
            ],
            region,
            currency: Currency::from_index(form.choice(CURRENCY)),
        },
        billing,
    })
}

/// The one field of the exchange-rate form, asked before invoicing in a foreign currency
pub(super) fn rate_form(currency: Currency) -> Form {
    let hint = format!("rupees per {currency} today, e.g. RBI reference rate");
    Form::new("Exchange rate", vec![Field::text("₹ per unit", hint)])
}

fn milestone_form(title: &'static str, hourly: bool, existing: Option<&Milestone>) -> Form {
    let mut fields = vec![
        Field::text("Name", "e.g. Design").with_value(existing.map_or("", Milestone::name)),
        Field::text("Due date", "YYYY-MM-DD, blank for today").with_value(
            existing
                .map(|m| m.due_date().to_string())
                .unwrap_or_default(),
        ),
    ];
    if !hourly {
        fields.push(
            Field::text("Amount", "e.g. 50,000")
                .with_value(existing.map(|m| m.amount().to_string()).unwrap_or_default()),
        );
    }
    Form::new(title, fields)
}

fn milestone_draft(form: &Form, hourly: bool, today: NaiveDate) -> Result<MilestoneDraft, String> {
    use milestone_field::{AMOUNT, DUE, NAME};

    let amount = if hourly {
        None
    } else {
        Some(parse::money(form.value(AMOUNT))?)
    };
    Ok(MilestoneDraft {
        name: form.value(NAME).trim().to_owned(),
        due_date: parse::date_or(form.value(DUE), today)?,
        amount,
    })
}

fn time_form() -> Form {
    Form::new(
        "Log time",
        vec![
            Field::text("Date", "YYYY-MM-DD, blank for today"),
            Field::text("Time", "1h30m, 45m, 1:30 or 1.5 (hours)").with_preview(describe_time),
            Field::text("Note", "what you worked on"),
        ],
    )
}

/// Live read-back of the time field, so `90` (hours) is never mistaken for minutes
fn describe_time(input: &str) -> String {
    match parse::duration_minutes(input) {
        Ok(minutes) if minutes > MAX_ENTRY_MINUTES => "over 24h: split by day".to_owned(),
        Ok(minutes) => format!("= {}", duration(minutes)),
        Err(error) => error.to_owned(),
    }
}

fn time_entry(form: &Form, today: NaiveDate) -> Result<TimeEntry, String> {
    use time_field::{DATE, NOTE, TIME};

    let date = parse::date_or(form.value(DATE), today)?;
    let minutes = parse::duration_minutes(form.value(TIME))?;
    TimeEntry::new(date, minutes, form.value(NOTE).trim()).map_err(fail)
}
