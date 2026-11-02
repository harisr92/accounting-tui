//! Building each form (prefilled when editing) and turning a submitted form into a saved change

use chrono::{Local, NaiveDate};

use super::{FormKind, ProjectsView};

use crate::ui::form::{Field, Form, ListItem, parse};
use crate::ui::view::Notice;
use crate::ui::views::client_form;
use crate::ui::views::payment::submit_payment;
use crate::ui::widgets::duration;
use compter_core::domain::{
    Billing, Client, ClientDetails, ClientRegion, Currency, ExchangeRate, MAX_ENTRY_MINUTES,
    Milestone, Project, TimeEntry, state_name,
};
use compter_core::services::{MilestoneDraft, ProjectDraft};
use compter_core::storage::Repository;

const BILLING_OPTIONS: &[&str] = &["Fixed price", "Hourly"];
const HOURLY: usize = 1;

/// Field positions, so building and reading a form agree
mod project_field {
    use crate::ui::views::client_form;

    pub const NAME: usize = 0;
    pub const CLIENT: usize = 1;
    /// The first of the client's other fields ([`client_form`])
    pub const CLIENT_DETAILS: usize = 2;
    pub const BILLING: usize = CLIENT_DETAILS + client_form::COUNT;
    pub const RATE: usize = BILLING + 1;
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
            FormKind::MilestoneSearch => {
                project?;
                self.milestone_filter.form("milestone name or amount")
            }
            // Built from the stored invoice by `open_payment`, and by `start_issue`
            FormKind::Payment(_) | FormKind::Issue(..) => return None,
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
            FormKind::Issue(id, milestones) => {
                let discount = parse::discount(form.value(issue::DISCOUNT))?;
                let foreign = self
                    .selected
                    .as_ref()
                    .is_some_and(|p| !p.currency().is_inr());
                let rate = if foreign {
                    let rate = parse::money(form.value(issue::RATE))?;
                    Some(ExchangeRate::new(rate).map_err(fail)?)
                } else {
                    None
                };
                self.preview_issue(id, milestones, rate, discount)?;
                return Ok(Notice::info("Check the invoice before issuing it"));
            }
            FormKind::Search => return self.search(form),
            FormKind::MilestoneSearch => return self.search_milestones(form),
            FormKind::Payment(number) => {
                let notice = submit_payment(&mut self.invoices, &number, form)?;
                let id = id.ok_or_else(no_project)?;
                (self.service.load(id).map_err(fail)?, notice)
            }
        };
        // A project made while only archived ones are listed goes back to the list to show it
        self.archived_only &= project.is_archived();
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
    let client = ClientDetails {
        currency,
        ..existing.map(Project::client_details).unwrap_or_default()
    };
    let billing = existing.map_or(0, |p| usize::from(p.billing().rate().is_some()));
    let rate = existing
        .and_then(|p| p.billing().rate())
        .map(ToString::to_string)
        .unwrap_or_default();
    let mut fields = vec![
        Field::text("Name", "e.g. Website revamp").with_value(existing.map_or("", Project::name)),
        Field::text("Client", "client or company name · Ctrl-p earlier clients")
            .with_value(client.name.clone()),
    ];
    fields.extend(client_form::fields(&client));
    fields.extend([
        Field::choice("Billing", BILLING_OPTIONS).with_choice(billing),
        Field::text("Rate / hour", "hourly only, in the currency above").with_value(rate),
    ]);
    Form::new(title, fields)
}

/// Fill the project form's client fields in from `client`, billed before
pub(super) fn fill_client(form: &mut Form, client: &Client) {
    form.set_value(project_field::CLIENT, client.name());
    client_form::fill(form, project_field::CLIENT_DETAILS, &client.details());
}

/// A client's row in the earlier-clients picker: the name, and their GSTIN or where they are
pub(super) fn client_item(client: &Client) -> ListItem {
    let place = match (client.gstin(), client.region()) {
        (Some(gstin), _) => gstin.as_str().to_owned(),
        (None, ClientRegion::India { state: Some(code) }) => state_name(code.as_str())
            .unwrap_or(code.as_str())
            .to_owned(),
        (None, ClientRegion::India { state: None }) => String::new(),
        (None, ClientRegion::Abroad { country }) => country.clone(),
    };
    ListItem::new(client.name(), place)
}

fn project_draft(form: &Form) -> Result<ProjectDraft, String> {
    use project_field::{BILLING, CLIENT, CLIENT_DETAILS, NAME, RATE};

    let billing = if form.choice(BILLING) == HOURLY {
        Billing::hourly(parse::money(form.value(RATE))?).map_err(fail)?
    } else {
        Billing::Fixed
    };
    Ok(ProjectDraft {
        name: form.value(NAME).trim().to_owned(),
        client: client_form::read(form, CLIENT_DETAILS, form.value(CLIENT))?,
        billing,
    })
}

/// Fields of the form asked before invoicing
mod issue {
    pub const DISCOUNT: usize = 0;
    /// Only for a project billed in a foreign currency
    pub const RATE: usize = 1;
}

/// Asked before invoicing: a discount off the total before GST, and the day's exchange rate
/// for a project billed in a foreign currency
pub(super) fn issue_form(currency: Currency) -> Form {
    let mut fields = vec![Field::text("Discount", "e.g. 5000 or 10%; blank for none")];
    if !currency.is_inr() {
        let hint = format!("rupees per {currency} today, e.g. RBI reference rate");
        fields.push(Field::text("₹ per unit", hint));
    }
    Form::new("Invoice", fields)
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
