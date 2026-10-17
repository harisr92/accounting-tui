//! Projects tab: browse projects and milestones, and change them through forms

mod forms;
mod render;

use crate::ui::key::Key;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};

use std::collections::BTreeSet;

use bigdecimal::Zero;
use chrono::Local;

use crate::domain::{
    Currency, ExchangeRate, InvoiceAmounts, InvoiceKind, InvoiceNumber, Milestone, MilestoneId,
    MilestoneStatus, Project, ProjectId, ProjectSummary,
};
use crate::services::{InvoicePreview, InvoiceService, Issued, ProjectService, ServiceError};
use crate::storage::{DataDir, Repository};
use crate::ui::filter::Filter;
use crate::ui::form::{Confirm, Form};
use crate::ui::selection::{Nav, Selection};
use crate::ui::theme::Theme;
use crate::ui::view::{Action, KeyHint, Notice, View};
use crate::ui::views::payment::payment_form;

use crate::ui::widgets::{duration, money, money_in};

const PROJECT_HINTS: &[KeyHint] = &[
    KeyHint::new("j/k", "move"),
    KeyHint::new("Tab", "milestones"),
    KeyHint::new("n", "new"),
    KeyHint::new("e", "edit"),
    KeyHint::new("x", "delete"),
    KeyHint::new("A", "archive/restore"),
    KeyHint::new("H", "show archived"),
    Filter::HINT,
    Filter::CLEAR_HINT,
];

const MILESTONE_HINTS: &[KeyHint] = &[
    KeyHint::new("j/k", "move"),
    KeyHint::new("Tab", "projects"),
    KeyHint::new("a", "add"),
    KeyHint::new("e", "edit"),
    KeyHint::new("x", "delete"),
    KeyHint::new("t", "log time"),
    KeyHint::new("o", "time log"),
    KeyHint::new("u", "undo time"),
    KeyHint::new("i", "invoice"),
    KeyHint::new("p", "paid"),
];

/// Leaves about 18 columns for a project name next to its status
const LIST_MIN_WIDTH: u16 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Projects,
    Milestones,
}

/// Which form is open, and what it changes
#[derive(Debug, Clone, PartialEq, Eq)]
enum FormKind {
    NewProject,
    EditProject,
    AddMilestone,
    EditMilestone(usize),
    LogTime(usize),
    /// Payment for the invoice that billed a milestone
    Payment(InvoiceNumber),
    /// The exchange rate to invoice a milestone of a foreign-currency project at
    IssueRate(ProjectId, MilestoneId),
    Search,
}

/// A change waiting for a yes
#[derive(Debug, Clone, PartialEq, Eq)]
enum ConfirmAction {
    DeleteProject(ProjectId),
    DeleteMilestone(ProjectId, usize),
    UndoTime(ProjectId, usize),
    /// With the exchange rate for a foreign-currency project
    Issue(ProjectId, MilestoneId, Option<ExchangeRate>),
}

enum Modal {
    Form(FormKind, Form),
    Confirm(ConfirmAction, Confirm),
}

/// Project list on the left, the selected project's milestones on the right
///
/// Holds only the summary rows and the one selected project; everything else stays on disk.
pub struct ProjectsView<R> {
    service: ProjectService<R>,
    invoices: InvoiceService<R>,
    summaries: Vec<ProjectSummary>,
    /// Positions in `summaries` of the rows listed: not archived (unless shown) and matching the
    /// search. `project_sel` moves over these.
    visible: Vec<usize>,
    show_archived: bool,
    filter: Filter,
    selected: Option<Project>,
    project_sel: Selection,
    milestone_sel: Selection,
    /// Milestones whose time log is shown under them (`o`)
    open_logs: BTreeSet<MilestoneId>,
    focus: Focus,
    modal: Option<Modal>,
}

impl<R: Repository> ProjectsView<R> {
    /// `data_dir` is where invoice PDFs are written; `None` keeps them in memory only
    pub fn new(repo: R, data_dir: Option<DataDir>) -> Result<Self, ServiceError> {
        let mut view = Self {
            service: ProjectService::new(repo.clone()),
            invoices: InvoiceService::new(repo, data_dir),
            summaries: Vec::new(),
            visible: Vec::new(),
            show_archived: false,
            filter: Filter::default(),
            selected: None,
            project_sel: Selection::default(),
            milestone_sel: Selection::default(),
            open_logs: BTreeSet::new(),
            focus: Focus::Projects,
            modal: None,
        };
        view.refresh(None)?;
        Ok(view)
    }

    /// Re-read the summaries; select `changed` if given, else reload the project under the cursor.
    /// A changed project the search hides clears the search; one just archived drops out of view.
    fn refresh(&mut self, changed: Option<Project>) -> Result<(), ServiceError> {
        self.summaries = self.service.list()?;
        self.update_visible();
        match changed {
            Some(project) if !project.is_archived() || self.show_archived => {
                if self.position_of(project.id()).is_none() {
                    self.filter = Filter::default();
                    self.update_visible();
                }
                if let Some(index) = self.position_of(project.id()) {
                    self.project_sel.select(index);
                }
                self.selected = Some(project);
            }
            _ => {
                self.project_sel.clamp(self.visible.len());
                self.load_selected()?;
            }
        }
        let milestones = self.selected.as_ref().map_or(0, |p| p.milestones().len());
        self.milestone_sel.clamp(milestones);
        Ok(())
    }

    /// Load the project under the cursor, dropping the previous one first
    fn load_selected(&mut self) -> Result<(), ServiceError> {
        self.selected = None;
        if let Some(id) = self
            .listed(self.project_sel.index())
            .map(ProjectSummary::id)
        {
            self.selected = Some(self.service.load(id)?);
        }
        Ok(())
    }

    /// The `index`th listed project
    fn listed(&self, index: usize) -> Option<&ProjectSummary> {
        self.visible.get(index).and_then(|&i| self.summaries.get(i))
    }

    /// Where project `id` is in the list, if it is listed
    fn position_of(&self, id: ProjectId) -> Option<usize> {
        self.visible
            .iter()
            .position(|&i| self.summaries.get(i).is_some_and(|s| s.id() == id))
    }

    fn update_visible(&mut self) {
        let (show_archived, filter) = (self.show_archived, &self.filter);
        self.visible = self
            .summaries
            .iter()
            .enumerate()
            .filter(|(_, s)| show_archived || !s.is_archived())
            .filter(|(_, s)| filter.matches(&[s.name(), s.client_name()]))
            .map(|(index, _)| index)
            .collect();
    }

    /// Re-list after the search or the archived toggle changed, keeping the selected project
    /// when it is still listed
    fn relist(&mut self) -> Result<(), ServiceError> {
        let current = self.selected.as_ref().map(Project::id);
        self.update_visible();
        self.milestone_sel.reset();
        match current.and_then(|id| self.position_of(id)) {
            Some(index) => self.project_sel.select(index),
            None => {
                self.project_sel.reset();
                self.open_logs.clear();
                self.load_selected()?;
            }
        }
        if self.selected.is_none() {
            self.focus = Focus::Projects;
        }
        Ok(())
    }

    /// `A`: archive the selected finished project, or restore an archived one
    fn toggle_archived(&mut self) -> Action {
        let Some(project) = &self.selected else {
            return Action::Notify(Notice::info("Select a project first"));
        };
        let archive = !project.is_archived();
        let id = project.id();
        let result = self.service.set_archived(id, archive).and_then(|p| {
            if p.is_archived() && !self.show_archived {
                // It drops out of the list: the project now under the cursor starts afresh
                self.milestone_sel.reset();
                self.open_logs.clear();
            }
            self.refresh(Some(p))
        });
        Action::Notify(match result {
            Ok(()) if archive && !self.show_archived => {
                Notice::success("Project archived: H shows archived projects")
            }
            Ok(()) if archive => Notice::success("Project archived"),
            Ok(()) => Notice::success("Project restored to the list"),
            Err(error) => Notice::error(error.to_string()),
        })
    }

    /// `H`: show or hide archived projects
    fn toggle_show_archived(&mut self) -> Action {
        self.show_archived = !self.show_archived;
        if let Err(error) = self.relist() {
            return Action::Notify(Notice::error(error.to_string()));
        }
        let count = self.summaries.iter().filter(|s| s.is_archived()).count();
        Action::Notify(Notice::info(if self.show_archived {
            format!("Showing {count} archived project(s)")
        } else {
            "Archived projects hidden".to_owned()
        }))
    }

    fn search(&mut self, form: &Form) -> Result<Notice, String> {
        self.filter = Filter::from_form(form);
        self.relist().map_err(|e| e.to_string())?;
        Ok(if self.filter.is_empty() {
            Notice::info("Search cleared")
        } else {
            Notice::info(format!(
                "{} project(s) match; Esc clears the search",
                self.visible.len()
            ))
        })
    }

    fn current_milestone(&self) -> Option<(ProjectId, usize, &Milestone)> {
        let project = self.selected.as_ref()?;
        let index = self.milestone_sel.index();
        project
            .milestones()
            .get(index)
            .map(|m| (project.id(), index, m))
    }

    fn open_form(&mut self, kind: FormKind) -> Action {
        match self.build_form(&kind) {
            Some(form) => {
                self.modal = Some(Modal::Form(kind, form));
                Action::Handled
            }
            None => Action::Notify(Notice::info("Select a project first")),
        }
    }

    fn ask(&mut self, action: ConfirmAction, message: String) -> Action {
        self.modal = Some(Modal::Confirm(action, Confirm::new(message)));
        Action::Handled
    }

    fn handle_modal(&mut self, modal: Modal, key: Key) -> Action {
        match modal {
            Modal::Form(kind, form) => {
                let (form, action) = form.step(key, |form| self.submit(&kind, form));
                // A submit may open the next modal (the rate form opens its confirm)
                if let Some(form) = form {
                    self.modal = Some(Modal::Form(kind, form));
                }
                action
            }
            Modal::Confirm(action, confirm) => match Confirm::answer(key) {
                Some(true) => self.run_confirmed(action),
                Some(false) => Action::Handled,
                None => {
                    self.modal = Some(Modal::Confirm(action, confirm));
                    Action::Handled
                }
            },
        }
    }

    fn run_confirmed(&mut self, action: ConfirmAction) -> Action {
        let result = match action {
            ConfirmAction::DeleteProject(id) => self
                .service
                .delete(id)
                .and_then(|()| self.refresh(None))
                .map(|()| "Project deleted"),
            ConfirmAction::DeleteMilestone(id, index) => self
                .service
                .remove_milestone(id, index)
                .and_then(|p| self.refresh(Some(p)))
                .map(|()| "Milestone deleted"),
            ConfirmAction::UndoTime(id, index) => self
                .service
                .remove_last_time_entry(id, index)
                .and_then(|p| self.refresh(Some(p)))
                .map(|()| "Time entry removed"),
            ConfirmAction::Issue(id, milestone, rate) => return self.issue(id, milestone, rate),
        };
        Action::Notify(match result {
            Ok(done) => Notice::success(done),
            Err(error) => Notice::error(error.to_string()),
        })
    }

    fn issue(
        &mut self,
        id: ProjectId,
        milestone: MilestoneId,
        rate: Option<ExchangeRate>,
    ) -> Action {
        let today = Local::now().date_naive();
        let issued = match self.invoices.issue(id, milestone, today, rate) {
            Ok(issued) => issued,
            Err(error) => return Action::Notify(Notice::error(error.to_string())),
        };
        let reloaded = self.service.load(id).and_then(|p| self.refresh(Some(p)));
        Action::Notify(match reloaded {
            Ok(()) => issued_notice(&issued),
            Err(error) => Notice::error(error.to_string()),
        })
    }

    /// `i`: show what the invoice will be and ask before issuing it. A project billed in a
    /// foreign currency first asks for the day's exchange rate.
    fn confirm_issue(&mut self, id: ProjectId, milestone: MilestoneId) -> Action {
        let currency = self
            .selected
            .as_ref()
            .map_or(Currency::Inr, Project::currency);
        if !currency.is_inr() {
            self.modal = Some(Modal::Form(
                FormKind::IssueRate(id, milestone),
                forms::rate_form(currency),
            ));
            return Action::Handled;
        }
        match self.preview_issue(id, milestone, None) {
            Ok(()) => Action::Handled,
            Err(error) => Action::Notify(Notice::error(error)),
        }
    }

    /// Open the confirm showing the invoice `milestone` would get today
    fn preview_issue(
        &mut self,
        id: ProjectId,
        milestone: MilestoneId,
        rate: Option<ExchangeRate>,
    ) -> Result<(), String> {
        let today = Local::now().date_naive();
        let preview = self
            .invoices
            .preview(id, milestone, today, rate.clone())
            .map_err(|e| e.to_string())?;
        let message = issue_question(&preview);
        self.ask(ConfirmAction::Issue(id, milestone, rate), message);
        Ok(())
    }

    /// `o`: show or hide the milestone's time entries under it
    fn toggle_log(&mut self, milestone: MilestoneId) -> Action {
        let hourly = self
            .selected
            .as_ref()
            .is_some_and(|p| p.billing().rate().is_some());
        if !hourly {
            return Action::Notify(Notice::info(
                "Fixed-price milestones have no time log; time is logged on hourly projects",
            ));
        }
        if !self.open_logs.remove(&milestone) {
            self.open_logs.insert(milestone);
        }
        Action::Handled
    }

    /// `p`: record the payment for the invoice that billed this milestone
    fn open_payment(&mut self, milestone: &Milestone) -> Action {
        if milestone.status() != MilestoneStatus::Invoiced {
            let why = match milestone.status() {
                MilestoneStatus::Paid => "This milestone is already paid",
                MilestoneStatus::Credited => {
                    "This milestone's invoice was cancelled by credit notes"
                }
                _ => "Invoice the milestone first (i)",
            };
            return Action::Notify(Notice::info(why));
        }
        let Some(number) = milestone.invoice_number() else {
            return Action::Notify(Notice::info(
                "This milestone was invoiced outside compter; there is no invoice to settle",
            ));
        };
        let number = InvoiceNumber::from_stored(number);
        match self.invoices.load(&number) {
            Ok(invoice) => {
                self.modal = Some(Modal::Form(
                    FormKind::Payment(number),
                    payment_form(&invoice),
                ));
                Action::Handled
            }
            Err(error) => Action::Notify(Notice::error(error.to_string())),
        }
    }

    fn handle_browse(&mut self, key: Key) -> Action {
        if let Some(nav) = Nav::from_key(key) {
            return self.navigate(nav);
        }
        match (self.focus, key) {
            (_, Key::Tab | Key::BackTab) => {
                self.focus = match self.focus {
                    Focus::Projects => Focus::Milestones,
                    Focus::Milestones => Focus::Projects,
                };
                Action::Handled
            }
            (Focus::Projects, Key::Enter) if self.selected.is_some() => {
                self.focus = Focus::Milestones;
                Action::Handled
            }
            (_, Key::Char('n')) => self.open_form(FormKind::NewProject),
            (Focus::Projects, Key::Char('/')) => self.open_form(FormKind::Search),
            (Focus::Projects, Key::Esc) if !self.filter.is_empty() => {
                self.filter = Filter::default();
                match self.relist() {
                    Ok(()) => Action::Notify(Notice::info("Search cleared")),
                    Err(error) => Action::Notify(Notice::error(error.to_string())),
                }
            }
            (Focus::Projects, Key::Char('A')) => self.toggle_archived(),
            (Focus::Projects, Key::Char('H')) => self.toggle_show_archived(),
            (Focus::Projects, Key::Char('e')) => self.open_form(FormKind::EditProject),
            (Focus::Projects, Key::Char('x')) => match &self.selected {
                Some(p) if p.ensure_deletable().is_err() => Action::Notify(Notice::error(
                    "Can't delete: this project has invoiced milestones (billing records)",
                )),
                Some(p) => {
                    let message = format!("Delete project “{}” and all its milestones?", p.name());
                    self.ask(ConfirmAction::DeleteProject(p.id()), message)
                }
                None => Action::Notify(Notice::info("Select a project first")),
            },
            (Focus::Milestones, Key::Char('a')) => self.open_form(FormKind::AddMilestone),
            (Focus::Milestones, Key::Char(c @ ('e' | 'x' | 't' | 'u' | 'i' | 'p' | 'o'))) => {
                self.milestone_action(c)
            }
            (Focus::Projects, Key::Char('i' | 'p')) => {
                Action::Notify(Notice::info("Press Tab and select a milestone"))
            }
            _ => Action::Ignored,
        }
    }

    fn navigate(&mut self, nav: Nav) -> Action {
        match self.focus {
            Focus::Projects => {
                self.project_sel.move_by(nav, self.visible.len());
                self.milestone_sel.reset();
                self.open_logs.clear();
                if let Err(error) = self.load_selected() {
                    return Action::Notify(Notice::error(error.to_string()));
                }
            }
            Focus::Milestones => {
                let len = self.selected.as_ref().map_or(0, |p| p.milestones().len());
                self.milestone_sel.move_by(nav, len);
            }
        }
        Action::Handled
    }

    /// `e` edit, `x` delete, `t` log time, `u` undo time, `i` invoice, `p` paid on the
    /// milestone under the cursor
    fn milestone_action(&mut self, key: char) -> Action {
        let Some((id, index, milestone)) = self.current_milestone() else {
            return Action::Notify(Notice::info("Select a milestone first"));
        };
        if key == 'p' {
            let milestone = milestone.clone();
            return self.open_payment(&milestone);
        }
        if key == 'o' {
            let milestone = milestone.id();
            return self.toggle_log(milestone);
        }
        if key == 'i' && milestone.status() == MilestoneStatus::Pending {
            let milestone = milestone.id();
            return self.confirm_issue(id, milestone);
        }
        if milestone.status() != MilestoneStatus::Pending {
            return Action::Notify(Notice::info(
                "Invoiced milestones are locked; only pending ones can change",
            ));
        }
        let hourly = self
            .selected
            .as_ref()
            .is_some_and(|p| p.billing().rate().is_some());
        match key {
            'e' => self.open_form(FormKind::EditMilestone(index)),
            'x' => {
                let message = format!("Delete milestone “{}”?", milestone.name());
                self.ask(ConfirmAction::DeleteMilestone(id, index), message)
            }
            't' | 'u' if !hourly => {
                Action::Notify(Notice::info("Time is logged on hourly projects only"))
            }
            't' => self.open_form(FormKind::LogTime(index)),
            _ => match milestone.time_entries().last() {
                Some(entry) => {
                    let message = format!(
                        "Remove the last time entry ({} on {}, “{}”)?",
                        duration(entry.minutes()),
                        entry.date(),
                        entry.note()
                    );
                    self.ask(ConfirmAction::UndoTime(id, index), message)
                }
                None => Action::Notify(Notice::info("No time logged on this milestone")),
            },
        }
    }
}

impl<R: Repository> View for ProjectsView<R> {
    fn title(&self) -> &'static str {
        "Projects"
    }

    fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        // A third of the width, but never so narrow that names are cut short
        let list_width = (area.width / 3).max(LIST_MIN_WIDTH).min(area.width);
        let [list_area, detail_area] =
            Layout::horizontal([Constraint::Length(list_width), Constraint::Min(0)]).areas(area);
        self.render_list(frame, list_area, theme);
        self.render_detail(frame, detail_area, theme);
        match &self.modal {
            Some(Modal::Form(_, form)) => form.render(frame, theme),
            Some(Modal::Confirm(_, confirm)) => confirm.render(frame, theme),
            None => {}
        }
    }

    /// An open form or confirmation takes every key, so typing never triggers shortcuts
    fn handle_key(&mut self, key: Key) -> Action {
        match self.modal.take() {
            Some(modal) => self.handle_modal(modal, key),
            None => self.handle_browse(key),
        }
    }

    /// Pick up invoices issued or paid from other tabs, keeping the same project selected
    fn on_activate(&mut self) -> Option<Notice> {
        let current = self.selected.as_ref().map(Project::id);
        let result = self.service.list().and_then(|summaries| {
            self.summaries = summaries;
            self.update_visible();
            if let Some(index) = current.and_then(|id| self.position_of(id)) {
                self.project_sel.select(index);
            }
            self.refresh(None)
        });
        result.err().map(|e| Notice::error(e.to_string()))
    }

    fn key_hints(&self) -> &'static [KeyHint] {
        match (&self.modal, self.focus) {
            (Some(Modal::Form(..)), _) => Form::HINTS,
            (Some(Modal::Confirm(..)), _) => Confirm::HINTS,
            (None, Focus::Projects) => PROJECT_HINTS,
            (None, Focus::Milestones) => MILESTONE_HINTS,
        }
    }
}

/// "Issue INV/26-27/004 to Acme for ₹59,000.00 (50,000.00 + IGST 9,000.00)?"
fn issue_question(preview: &InvoicePreview) -> String {
    let a = &preview.amounts;
    let in_rupees = preview
        .total_inr
        .as_ref()
        .map(|inr| format!(", {} in INR", money(inr)))
        .unwrap_or_default();
    let mut question = format!(
        "Issue {} to {} for {} ({}{in_rupees})?",
        preview.number,
        preview.client_name,
        money_in(&a.total, preview.currency),
        tax_breakdown(preview.kind, preview.currency, a)
    );
    for warning in &preview.warnings {
        question.push_str(&format!(" Note: {warning}."));
    }
    question
}

fn tax_breakdown(kind: InvoiceKind, currency: Currency, a: &InvoiceAmounts) -> String {
    let format = |amount| currency.format(amount);
    let base = format(&a.taxable_value);
    match kind {
        InvoiceKind::Export => format!("{base}, zero-rated export under LUT"),
        InvoiceKind::Plain => format!("{base}, no GST"),
        InvoiceKind::Tax if !a.igst.is_zero() => format!("{base} + IGST {}", format(&a.igst)),
        InvoiceKind::Tax => format!(
            "{base} + CGST {} + SGST {}",
            format(&a.cgst),
            format(&a.sgst)
        ),
    }
}

fn issued_notice(issued: &Issued) -> Notice {
    let number = issued.invoice.number();
    match &issued.pdf {
        Ok(Some(path)) => {
            let file = path
                .file_name()
                .map(|f| f.to_string_lossy())
                .unwrap_or_default();
            Notice::success(format!(
                "Issued {number}: saved {file} (o in Invoices opens it)"
            ))
        }
        Ok(None) => Notice::success(format!("Issued {number}")),
        Err(error) => Notice::error(format!(
            "Issued {number}, but its PDF wasn't saved ({error}); press o in Invoices to retry"
        )),
    }
}
