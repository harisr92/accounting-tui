//! Building each form (prefilled when editing) and turning a submitted form into a saved change

use chrono::{Local, NaiveDate};

use super::{FormKind, ProjectsView};
use crate::domain::{Billing, MAX_ENTRY_MINUTES, Milestone, Project, TimeEntry};
use crate::services::{MilestoneDraft, ProjectDraft};
use crate::storage::ProjectRepository;
use crate::ui::form::{Field, Form, parse};
use crate::ui::view::Notice;
use crate::ui::widgets::duration;

const BILLING_OPTIONS: &[&str] = &["Fixed price", "Hourly"];
const HOURLY: usize = 1;

/// Field positions, so building and reading a form agree
mod project_field {
    pub const NAME: usize = 0;
    pub const CLIENT: usize = 1;
    pub const GSTIN: usize = 2;
    pub const BILLING: usize = 3;
    pub const RATE: usize = 4;
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

impl<R: ProjectRepository> ProjectsView<R> {
    /// The form for `kind`; `None` when it needs a selected project or milestone that isn't there
    pub(super) fn build_form(&self, kind: FormKind) -> Option<Form> {
        let project = self.selected.as_ref();
        let hourly = project.is_some_and(|p| p.billing().rate().is_some());
        Some(match kind {
            FormKind::NewProject => project_form("New project", None),
            FormKind::EditProject => project_form("Edit project", Some(project?)),
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
        })
    }

    /// Save the form's change; on `Err` the message is shown in the still-open form
    pub(super) fn submit(&mut self, kind: FormKind, form: &Form) -> Result<Notice, String> {
        let today = Local::now().date_naive();
        let id = self.selected.as_ref().map(Project::id);
        let hourly = self
            .selected
            .as_ref()
            .is_some_and(|p| p.billing().rate().is_some());
        let no_project = || "No project is selected".to_owned();

        let (project, notice) = match kind {
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
                self.milestone_sel
                    .select(project.milestones().len().saturating_sub(1));
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
        };
        self.refresh(Some(project)).map_err(fail)?;
        Ok(notice)
    }
}

fn fail(error: impl ToString) -> String {
    error.to_string()
}

fn project_form(title: &'static str, existing: Option<&Project>) -> Form {
    let (name, client, gstin, billing, rate) = existing.map_or_else(Default::default, |p| {
        let rate = p
            .billing()
            .rate()
            .map(ToString::to_string)
            .unwrap_or_default();
        let gstin = p
            .client_gstin()
            .map(|g| g.as_str().to_owned())
            .unwrap_or_default();
        let billing = usize::from(p.billing().rate().is_some());
        (
            p.name().to_owned(),
            p.client_name().to_owned(),
            gstin,
            billing,
            rate,
        )
    });
    Form::new(
        title,
        vec![
            Field::text("Name", "e.g. Website revamp").with_value(name),
            Field::text("Client", "client or company name").with_value(client),
            Field::text("GSTIN", "blank if the client is unregistered").with_value(gstin),
            Field::choice("Billing", BILLING_OPTIONS).with_choice(billing),
            Field::text("Rate / hour", "hourly billing only, e.g. 1500").with_value(rate),
        ],
    )
}

fn project_draft(form: &Form) -> Result<ProjectDraft, String> {
    use project_field::{BILLING, CLIENT, GSTIN, NAME, RATE};

    let billing = if form.choice(BILLING) == HOURLY {
        Billing::hourly(parse::money(form.value(RATE))?).map_err(fail)?
    } else {
        Billing::Fixed
    };
    Ok(ProjectDraft {
        name: form.value(NAME).trim().to_owned(),
        client_name: form.value(CLIENT).trim().to_owned(),
        client_gstin: parse::optional(form.value(GSTIN)).map(|g| g.to_uppercase()),
        billing,
    })
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
