//! Projects tab: browse projects and milestones, and change them through forms

mod forms;
mod render;

use crate::ui::key::Key;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};

use std::collections::BTreeSet;

use chrono::Local;

use crate::ui::filter::Filter;
use crate::ui::form::{Checklist, ChecklistItem, Confirm, Form};
use crate::ui::selection::{Nav, Selection};
use crate::ui::theme::Theme;
use crate::ui::view::{Action, KeyHint, Notice, View};
use crate::ui::views::payment::payment_form;
use compter_core::domain::{
    Currency, ExchangeRate, InvoiceNumber, Milestone, MilestoneId, MilestoneSort, MilestoneStatus,
    Project, ProjectId, ProjectSummary,
};
use compter_core::services::{
    InvoicePreview, InvoiceService, Issued, ProjectService, ServiceError,
};
use compter_core::storage::{DataDir, Repository};

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
    KeyHint::new("i", "invoice (pick)"),
    KeyHint::new("p", "paid"),
    KeyHint::new("s", "sort"),
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
    /// The exchange rate to invoice milestones of a foreign-currency project at
    IssueRate(ProjectId, Vec<MilestoneId>),
    Search,
}

/// A change waiting for a yes
#[derive(Debug, Clone, PartialEq, Eq)]
enum ConfirmAction {
    DeleteProject(ProjectId),
    DeleteMilestone(ProjectId, usize),
    UndoTime(ProjectId, usize),
    /// The milestones billed together, with the exchange rate for a foreign-currency project
    Issue(ProjectId, Vec<MilestoneId>, Option<ExchangeRate>),
}

enum Modal {
    Form(FormKind, Form),
    Confirm(ConfirmAction, Confirm),
    /// The pending milestones of a project to invoice together, a row each in `ids` order
    Pick {
        project: ProjectId,
        ids: Vec<MilestoneId>,
        list: Checklist,
    },
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
    /// Moves over the milestones in `milestone_sort` order, not their stored order: map a row
    /// to a milestone only through `milestone_order`
    milestone_sel: Selection,
    milestone_sort: MilestoneSort,
    /// Milestones whose time log is shown under them (`o`)
    open_logs: BTreeSet<MilestoneId>,
    focus: Focus,
    modal: Option<Modal>,
}

impl<R> ProjectsView<R> {
    /// Stored positions of the selected project's milestones, in the order they are listed
    pub(super) fn milestone_order(&self) -> Vec<usize> {
        self.selected
            .as_ref()
            .map(|p| self.milestone_sort.positions(p.milestones()))
            .unwrap_or_default()
    }
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
            milestone_sort: MilestoneSort::default(),
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
        // A change can reorder the milestones (a new due date): keep the cursor on the same one
        let keep = self.current_milestone().map(|(_, _, m)| m.id());
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
        if let Some(id) = keep {
            self.select_milestone(id);
        }
        Ok(())
    }

    /// Put the cursor on milestone `id`, wherever the sort lists it
    pub(super) fn select_milestone(&mut self, id: MilestoneId) {
        let Some(project) = &self.selected else {
            return;
        };
        let row = self
            .milestone_order()
            .iter()
            .position(|&i| project.milestones().get(i).is_some_and(|m| m.id() == id));
        if let Some(row) = row {
            self.milestone_sel.select(row);
        }
    }

    /// `s`: list the milestones by the next order, keeping the cursor on the same milestone
    fn cycle_sort(&mut self) -> Action {
        let keep = self.current_milestone().map(|(_, _, m)| m.id());
        self.milestone_sort = self.milestone_sort.next();
        if let Some(id) = keep {
            self.select_milestone(id);
        }
        Action::Notify(Notice::info(format!(
            "Milestones by {}",
            self.milestone_sort.label()
        )))
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

    /// The milestone under the cursor, with its stored index. Every milestone action goes
    /// through here, so the row shown and the milestone changed are always the same one.
    fn current_milestone(&self) -> Option<(ProjectId, usize, &Milestone)> {
        let project = self.selected.as_ref()?;
        let index = *self.milestone_order().get(self.milestone_sel.index())?;
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
            Modal::Pick { project, ids, list } => {
                let (list, action) = list.step(key, |list| {
                    let picked: Vec<MilestoneId> = list
                        .checked()
                        .iter()
                        .filter_map(|&row| ids.get(row).copied())
                        .collect();
                    if picked.is_empty() {
                        return Err("Pick at least one milestone (Space, or a for all)".to_owned());
                    }
                    self.start_issue(project, picked).map(Notice::info)
                });
                // A good submit opened the rate form or the confirm in its place
                if let Some(list) = list {
                    self.modal = Some(Modal::Pick { project, ids, list });
                }
                action
            }
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
            ConfirmAction::Issue(id, milestones, rate) => {
                return self.issue(id, &milestones, rate);
            }
        };
        Action::Notify(match result {
            Ok(done) => Notice::success(done),
            Err(error) => Notice::error(error.to_string()),
        })
    }

    fn issue(
        &mut self,
        id: ProjectId,
        milestones: &[MilestoneId],
        rate: Option<ExchangeRate>,
    ) -> Action {
        let today = Local::now().date_naive();
        let issued = match self.invoices.issue(id, milestones, today, rate) {
            Ok(issued) => issued,
            Err(error) => return Action::Notify(Notice::error(error.to_string())),
        };
        let reloaded = self.service.load(id).and_then(|p| self.refresh(Some(p)));
        Action::Notify(match reloaded {
            Ok(()) => issued_notice(&issued),
            Err(error) => Notice::error(error.to_string()),
        })
    }

    /// `i` on a pending milestone: with other milestones of the project pending too, pick which
    /// to invoice together (`a` picks them all), starting from this one; otherwise go straight
    /// to the invoice
    fn pick_to_invoice(&mut self, id: ProjectId, milestone: MilestoneId) -> Action {
        let Some(project) = &self.selected else {
            return Action::Notify(Notice::info("Select a project first"));
        };
        let currency = project.currency();
        let pending: Vec<&Milestone> = self
            .milestone_order()
            .into_iter()
            .filter_map(|i| project.milestones().get(i))
            .filter(|m| m.status() == MilestoneStatus::Pending)
            .collect();
        if pending.len() < 2 {
            return match self.start_issue(id, vec![milestone]) {
                Ok(_) => Action::Handled,
                Err(error) => Action::Notify(Notice::error(error)),
            };
        }
        let items = pending
            .iter()
            .map(|m| {
                ChecklistItem::new(m.name(), money_in(m.amount(), currency))
                    .checked(m.id() == milestone)
            })
            .collect();
        let ids = pending.iter().map(|m| m.id()).collect();
        self.modal = Some(Modal::Pick {
            project: id,
            ids,
            list: Checklist::new("Milestones to invoice", ["Milestone", "Amount", ""], items),
        });
        Action::Handled
    }

    /// Show what the invoice billing `milestones` will be and ask before issuing it. A project
    /// billed in a foreign currency first asks for the day's exchange rate. Returns what to do
    /// next, for the footer.
    fn start_issue(
        &mut self,
        id: ProjectId,
        milestones: Vec<MilestoneId>,
    ) -> Result<&'static str, String> {
        let currency = self
            .selected
            .as_ref()
            .map_or(Currency::Inr, Project::currency);
        if !currency.is_inr() {
            self.modal = Some(Modal::Form(
                FormKind::IssueRate(id, milestones),
                forms::rate_form(currency),
            ));
            return Ok("Enter today's exchange rate");
        }
        self.preview_issue(id, milestones, None)?;
        Ok("Check the invoice before issuing it")
    }

    /// Open the confirm showing the invoice `milestones` would get today
    fn preview_issue(
        &mut self,
        id: ProjectId,
        milestones: Vec<MilestoneId>,
        rate: Option<ExchangeRate>,
    ) -> Result<(), String> {
        let today = Local::now().date_naive();
        let preview = self
            .invoices
            .preview(id, &milestones, today, rate.clone())
            .map_err(|e| e.to_string())?;
        let message = issue_question(&preview, milestones.len());
        self.ask(ConfirmAction::Issue(id, milestones, rate), message);
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
                    "Credit notes took back all of this milestone; nothing is due on it"
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
            (Focus::Milestones, Key::Char('s')) => self.cycle_sort(),
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
            return self.pick_to_invoice(id, milestone);
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
            Some(Modal::Pick { list, .. }) => list.render(frame, theme),
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
            (Some(Modal::Pick { list, .. }), _) => list.hints(),
            (None, Focus::Projects) => PROJECT_HINTS,
            (None, Focus::Milestones) => MILESTONE_HINTS,
        }
    }
}

/// "Issue INV/26-27/004 to Acme for ₹59,000.00 (50,000.00 + IGST 9,000.00)?", naming how many
/// milestones it bills when more than one
fn issue_question(preview: &InvoicePreview, milestones: usize) -> String {
    let a = &preview.amounts;
    let in_rupees = preview
        .total_inr
        .as_ref()
        .map(|inr| format!(", {} in INR", money(inr)))
        .unwrap_or_default();
    let lines = if milestones > 1 {
        format!(" billing {milestones} milestones")
    } else {
        String::new()
    };
    let mut question = format!(
        "Issue {}{lines} to {} for {} ({}{in_rupees})?",
        preview.number,
        preview.client_name,
        money_in(&a.total, preview.currency),
        a.breakdown(preview.kind, preview.currency)
    );
    for warning in &preview.warnings {
        question.push_str(&format!(" Note: {warning}."));
    }
    question
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
