//! Projects tab: browse projects and milestones, and change them through forms

mod forms;
mod render;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use termion::event::Key;

use crate::domain::{Milestone, MilestoneStatus, Project, ProjectId, ProjectSummary};
use crate::services::{ProjectService, ServiceError};
use crate::storage::ProjectRepository;
use crate::ui::form::{Confirm, Form, FormEvent};
use crate::ui::selection::{Nav, Selection};
use crate::ui::theme::Theme;
use crate::ui::view::{Action, KeyHint, Notice, View};
use crate::ui::widgets::duration;

const PROJECT_HINTS: &[KeyHint] = &[
    KeyHint::new("j/k", "move"),
    KeyHint::new("Tab", "milestones"),
    KeyHint::new("n", "new"),
    KeyHint::new("e", "edit"),
    KeyHint::new("x", "delete"),
];

const MILESTONE_HINTS: &[KeyHint] = &[
    KeyHint::new("j/k", "move"),
    KeyHint::new("Tab", "projects"),
    KeyHint::new("a", "add"),
    KeyHint::new("e", "edit"),
    KeyHint::new("x", "delete"),
    KeyHint::new("t", "log time"),
    KeyHint::new("u", "undo time"),
    KeyHint::new("i", "invoice"),
];

/// Leaves about 18 columns for a project name next to its status
const LIST_MIN_WIDTH: u16 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Projects,
    Milestones,
}

/// Which form is open, and what it changes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FormKind {
    NewProject,
    EditProject,
    AddMilestone,
    EditMilestone(usize),
    LogTime(usize),
}

/// A change waiting for a yes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfirmAction {
    DeleteProject(ProjectId),
    DeleteMilestone(ProjectId, usize),
    UndoTime(ProjectId, usize),
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
    summaries: Vec<ProjectSummary>,
    selected: Option<Project>,
    project_sel: Selection,
    milestone_sel: Selection,
    focus: Focus,
    modal: Option<Modal>,
}

impl<R: ProjectRepository> ProjectsView<R> {
    pub fn new(service: ProjectService<R>) -> Result<Self, ServiceError> {
        let mut view = Self {
            service,
            summaries: Vec::new(),
            selected: None,
            project_sel: Selection::default(),
            milestone_sel: Selection::default(),
            focus: Focus::Projects,
            modal: None,
        };
        view.refresh(None)?;
        Ok(view)
    }

    /// Re-read the summaries; select `changed` if given, else reload the project under the cursor
    fn refresh(&mut self, changed: Option<Project>) -> Result<(), ServiceError> {
        self.summaries = self.service.list()?;
        match changed {
            Some(project) => {
                if let Some(index) = self.summaries.iter().position(|s| s.id() == project.id()) {
                    self.project_sel.select(index);
                }
                self.selected = Some(project);
            }
            None => {
                self.project_sel.clamp(self.summaries.len());
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
        if let Some(summary) = self.summaries.get(self.project_sel.index()) {
            self.selected = Some(self.service.load(summary.id())?);
        }
        Ok(())
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
        match self.build_form(kind) {
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
            Modal::Form(kind, mut form) => match form.handle_key(key) {
                FormEvent::Cancel => Action::Handled,
                FormEvent::Submit => match self.submit(kind, &form) {
                    Ok(notice) => Action::Notify(notice),
                    Err(error) => {
                        form.set_error(error);
                        self.modal = Some(Modal::Form(kind, form));
                        Action::Handled
                    }
                },
                FormEvent::Edited | FormEvent::Ignored => {
                    self.modal = Some(Modal::Form(kind, form));
                    Action::Handled
                }
            },
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
        };
        Action::Notify(match result {
            Ok(done) => Notice::success(done),
            Err(error) => Notice::error(error.to_string()),
        })
    }

    fn handle_browse(&mut self, key: Key) -> Action {
        if let Some(nav) = Nav::from_key(key) {
            return self.navigate(nav);
        }
        match (self.focus, key) {
            (_, Key::Char('\t') | Key::BackTab) => {
                self.focus = match self.focus {
                    Focus::Projects => Focus::Milestones,
                    Focus::Milestones => Focus::Projects,
                };
                Action::Handled
            }
            (Focus::Projects, Key::Char('\n')) if self.selected.is_some() => {
                self.focus = Focus::Milestones;
                Action::Handled
            }
            (_, Key::Char('n')) => self.open_form(FormKind::NewProject),
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
            (Focus::Milestones, Key::Char(c @ ('e' | 'x' | 't' | 'u'))) => self.milestone_action(c),
            (_, Key::Char('i')) => Action::Notify(Notice::info("Invoicing arrives in step 5")),
            _ => Action::Ignored,
        }
    }

    fn navigate(&mut self, nav: Nav) -> Action {
        match self.focus {
            Focus::Projects => {
                self.project_sel.move_by(nav, self.summaries.len());
                self.milestone_sel.reset();
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

    /// `e` edit, `x` delete, `t` log time, `u` undo time on the milestone under the cursor
    fn milestone_action(&mut self, key: char) -> Action {
        let Some((id, index, milestone)) = self.current_milestone() else {
            return Action::Notify(Notice::info("Select a milestone first"));
        };
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

impl<R: ProjectRepository> View for ProjectsView<R> {
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

    fn key_hints(&self) -> &'static [KeyHint] {
        match (&self.modal, self.focus) {
            (Some(Modal::Form(..)), _) => Form::HINTS,
            (Some(Modal::Confirm(..)), _) => Confirm::HINTS,
            (None, Focus::Projects) => PROJECT_HINTS,
            (None, Focus::Milestones) => MILESTONE_HINTS,
        }
    }
}
