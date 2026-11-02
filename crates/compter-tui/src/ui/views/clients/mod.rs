//! Clients tab: every saved client with what they still owe; open one to see their projects
//! and invoices, and to correct their details

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Row, Table};

use crate::ui::filter::Filter;
use crate::ui::form::{Confirm, Form};
use crate::ui::key::Key;
use crate::ui::selection::{Nav, Selection};
use crate::ui::theme::Theme;
use crate::ui::view::{Action, KeyHint, Notice, View};
use crate::ui::views::client_form;
use crate::ui::widgets::{header, money_in, panel, status_span};
use compter_core::domain::{ClientDetails, ClientId};
use compter_core::services::{ClientAccount, ClientService, ClientSummary, Owed, ServiceError};
use compter_core::storage::{ClientRepository, InvoiceRepository, ProjectRepository};

const LIST_HINTS: &[KeyHint] = &[
    KeyHint::new("j/k", "move"),
    KeyHint::new("Enter", "open"),
    Filter::HINT,
    Filter::CLEAR_HINT,
];

const ACCOUNT_HINTS: &[KeyHint] = &[
    KeyHint::new("j/k", "move"),
    KeyHint::new("Tab", "projects/invoices"),
    KeyHint::new("e", "edit client"),
    KeyHint::new("Esc", "back to clients"),
];

/// Shown for a client without a GSTIN or a known place, and for one who owes nothing
const NONE: &str = "—";

const NOT_OPEN: &str = "No client is open";

/// Asking whether the open projects take the corrected details too
const PENDING_HINTS: &[KeyHint] = &[
    KeyHint::new("y", "update projects too"),
    KeyHint::new("n", "client only"),
    KeyHint::new("Esc", "back to the form"),
];

/// A client in the list, with the text the search looks through
struct Listed {
    summary: ClientSummary,
    /// GSTIN, else state or country ([`Client::place`](compter_core::domain::Client::place))
    place: String,
    /// What they owe, `₹35,400.00 · €250.00`; empty when nothing is due
    owed: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Projects,
    Invoices,
}

/// The client opened with Enter: the one full account held in memory
struct Opened {
    account: ClientAccount,
    place: String,
    owed: String,
    focus: Focus,
    projects: Selection,
    invoices: Selection,
}

impl Opened {
    fn new(account: ClientAccount) -> Self {
        Self {
            place: account.client.place().unwrap_or_default(),
            owed: owed_text(&account.owed),
            account,
            focus: Focus::Invoices,
            projects: Selection::default(),
            invoices: Selection::default(),
        }
    }

    /// Show `account`, re-read after a change, with the cursors kept where they fit
    fn replace(&mut self, account: ClientAccount) {
        self.place = account.client.place().unwrap_or_default();
        self.owed = owed_text(&account.owed);
        self.projects.clamp(account.projects.len());
        self.invoices.clamp(account.invoices.len());
        self.account = account;
    }
}

/// Corrected details waiting on whether the client's open projects take them too, with the
/// form they came from, which `Esc` or a refused save goes back to
struct Pending {
    confirm: Confirm,
    details: ClientDetails,
    form: Form,
}

pub struct ClientsView<R> {
    service: ClientService<R>,
    clients: Vec<Listed>,
    /// Positions in `clients` of the rows matching the search; `selection` moves over these
    visible: Vec<usize>,
    filter: Filter,
    selection: Selection,
    opened: Option<Opened>,
    search: Option<Form>,
    /// Correcting the opened client's details
    editing: Option<Form>,
    pending: Option<Pending>,
}

impl<R: ClientRepository + ProjectRepository + InvoiceRepository> ClientsView<R> {
    pub fn new(service: ClientService<R>) -> Result<Self, ServiceError> {
        let mut view = Self {
            service,
            clients: Vec::new(),
            visible: Vec::new(),
            filter: Filter::default(),
            selection: Selection::default(),
            opened: None,
            search: None,
            editing: None,
            pending: None,
        };
        view.reload()?;
        Ok(view)
    }

    /// Re-read the list, and the opened client's account with its cursors kept where they fit
    fn reload(&mut self) -> Result<(), ServiceError> {
        self.reload_list()?;
        if let Some(opened) = &mut self.opened {
            opened.replace(self.service.account(opened.account.client.id())?);
        }
        Ok(())
    }

    fn reload_list(&mut self) -> Result<(), ServiceError> {
        self.clients = self
            .service
            .list()?
            .into_iter()
            .map(|summary| Listed {
                place: summary.client.place().unwrap_or_default(),
                owed: owed_text(&summary.owed),
                summary,
            })
            .collect();
        self.update_visible();
        Ok(())
    }

    fn update_visible(&mut self) {
        self.visible = self
            .filter
            .positions(&self.clients, |c| [c.summary.client.name(), &c.place]);
        self.selection.clamp(self.visible.len());
    }

    fn selected(&self) -> Option<&Listed> {
        self.visible
            .get(self.selection.index())
            .and_then(|&i| self.clients.get(i))
    }

    fn open(&mut self, id: ClientId) -> Action {
        match self.service.account(id) {
            Ok(account) => {
                self.opened = Some(Opened::new(account));
                Action::Handled
            }
            Err(error) => Action::Notify(Notice::error(error.to_string())),
        }
    }

    /// The opened client's details in a form; the name stays, so it is the form's subject
    fn start_edit(&mut self) -> Action {
        let Some(opened) = &self.opened else {
            return Action::Ignored;
        };
        let fields = client_form::fields(&opened.account.client.details());
        self.editing = Some(Form::new("Edit client", fields));
        Action::Notify(Notice::info(format!(
            "Editing {}; issued invoices keep the details they were printed with",
            opened.account.client.name()
        )))
    }

    /// Check the corrected details, then ask about the open projects that would change, or
    /// save straight away when there are none
    fn submit_edit(&mut self, form: &Form) -> Result<Notice, String> {
        let client = &self.opened.as_ref().ok_or(NOT_OPEN)?.account.client;
        let (id, details) = (client.id(), client_form::read(form, 0, client.name())?);
        let stale = self
            .service
            .projects_to_update(id, &details)
            .map_err(|e| e.to_string())?;
        if stale.is_empty() {
            return self.save_edit(details, false);
        }
        let names: Vec<_> = stale.iter().map(|p| p.name()).collect();
        let message = format!(
            "Update {} open project(s) too: {}? n saves the client only. {}",
            stale.len(),
            names.join(", "),
            "Issued invoices never change."
        );
        self.pending = Some(Pending {
            confirm: Confirm::new(message),
            details,
            form: form.clone(),
        });
        Ok(Notice::info(
            "Choose whether their open projects change too",
        ))
    }

    fn save_edit(&mut self, details: ClientDetails, with_projects: bool) -> Result<Notice, String> {
        let opened = self.opened.as_mut().ok_or(NOT_OPEN)?;
        let account = self
            .service
            .update(opened.account.client.id(), details, with_projects)
            .map_err(|e| e.to_string())?;
        opened.replace(account);
        self.reload_list().map_err(|e| e.to_string())?;
        Ok(Notice::success(if with_projects {
            "Client and their open projects saved"
        } else {
            "Client saved"
        }))
    }

    /// `y` or `n` saves; `Esc` goes back to the form, as does a refused save, with the error
    fn answer_pending(&mut self, pending: Pending, key: Key) -> Action {
        let with_projects = match key {
            Key::Esc => {
                self.editing = Some(pending.form);
                return Action::Handled;
            }
            key => Confirm::answer(key),
        };
        let Some(with_projects) = with_projects else {
            self.pending = Some(pending);
            return Action::Handled;
        };
        match self.save_edit(pending.details, with_projects) {
            Ok(notice) => Action::Notify(notice),
            Err(error) => {
                let mut form = pending.form;
                form.set_error(error);
                self.editing = Some(form);
                Action::Handled
            }
        }
    }

    fn search(&mut self, form: &Form) -> Result<Notice, String> {
        self.filter = Filter::from_form(form);
        self.update_visible();
        Ok(Notice::info(if self.filter.is_empty() {
            "Search cleared".to_owned()
        } else {
            format!(
                "{} client(s) match; Esc clears the search",
                self.visible.len()
            )
        }))
    }

    fn handle_list(&mut self, key: Key) -> Action {
        if let Some(nav) = Nav::from_key(key) {
            self.selection.move_by(nav, self.visible.len());
            return Action::Handled;
        }
        match key {
            Key::Enter => match self.selected().map(|c| c.summary.client.id()) {
                Some(id) => self.open(id),
                None => Action::Notify(Notice::info("No client selected")),
            },
            Key::Char('/') => {
                self.search = Some(self.filter.form("name, GSTIN or place"));
                Action::Handled
            }
            Key::Esc if !self.filter.is_empty() => {
                self.filter = Filter::default();
                self.update_visible();
                Action::Notify(Notice::info("Search cleared"))
            }
            _ => Action::Ignored,
        }
    }

    fn render_list(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let title = if self.filter.is_empty() {
            "Clients".to_owned()
        } else {
            format!("Clients /{}", self.filter.query())
        };
        let block = panel(&title, true, theme);
        if self.visible.is_empty() {
            let message = if self.clients.is_empty() {
                "No clients yet. A client is saved with their first project."
            } else {
                "No clients match the search. Esc clears it."
            };
            frame.render_widget(
                Paragraph::new(message).style(theme.dim()).block(block),
                area,
            );
            return;
        }
        let rows = self
            .visible
            .iter()
            .filter_map(|&i| self.clients.get(i))
            .map(|c| {
                Row::new(vec![
                    Line::raw(c.summary.client.name()),
                    or_none(&c.place, theme),
                    Line::raw(c.summary.projects.to_string()).right_aligned(),
                    or_none(&c.owed, theme).right_aligned(),
                ])
            });
        // Wide enough for the longest amount, so several currencies aren't cut off
        let owed_width = self
            .clients
            .iter()
            .map(|c| c.owed.chars().count())
            .max()
            .unwrap_or_default()
            .max(14);
        let widths = [
            Constraint::Fill(1),
            Constraint::Fill(1),
            Constraint::Length(8),
            Constraint::Length(u16::try_from(owed_width).unwrap_or(u16::MAX)),
        ];
        let table = Table::new(rows, widths)
            .header(header(
                &["Client", "GSTIN / place", "Projects", "Owes"],
                &[2, 3],
                theme,
            ))
            .column_spacing(1)
            .block(block)
            .row_highlight_style(theme.selected)
            .highlight_symbol("> ");
        let mut state = self.selection.table_state(self.visible.len());
        frame.render_stateful_widget(table, area, &mut state);
    }
}

impl Opened {
    fn handle_key(&mut self, key: Key) -> Action {
        if let Some(nav) = Nav::from_key(key) {
            match self.focus {
                Focus::Projects => self.projects.move_by(nav, self.account.projects.len()),
                Focus::Invoices => self.invoices.move_by(nav, self.account.invoices.len()),
            }
            return Action::Handled;
        }
        match key {
            Key::Tab | Key::BackTab => {
                self.focus = match self.focus {
                    Focus::Projects => Focus::Invoices,
                    Focus::Invoices => Focus::Projects,
                };
                Action::Handled
            }
            _ => Action::Ignored,
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let account = &self.account;
        // Room for every project up to ten, then the invoices take the rest
        let project_rows = u16::try_from(account.projects.len().clamp(1, 10)).unwrap_or(10);
        let [summary_area, projects_area, invoices_area] = Layout::vertical([
            Constraint::Length(5),
            Constraint::Length(project_rows + 3),
            Constraint::Min(4),
        ])
        .areas(area);
        self.render_summary(frame, summary_area, theme);
        self.render_projects(frame, projects_area, theme);
        self.render_invoices(frame, invoices_area, theme);
    }

    fn render_summary(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let client = &self.account.client;
        let block = panel(client.name(), false, theme);
        let address = client.address().join(", ");
        let lines = vec![
            Line::from(vec![
                Span::styled("GSTIN / place ", theme.dim()),
                Span::raw(if self.place.is_empty() {
                    NONE
                } else {
                    &self.place
                }),
            ]),
            Line::from(vec![
                Span::styled("Address       ", theme.dim()),
                Span::raw(if address.is_empty() {
                    NONE.to_owned()
                } else {
                    address
                }),
            ]),
            Line::from(
                [Span::styled("Owes          ", theme.dim())]
                    .into_iter()
                    .chain(or_none(&self.owed, theme).spans)
                    .collect::<Vec<_>>(),
            ),
        ];
        frame.render_widget(Paragraph::new(lines).block(block), area);
    }

    fn render_projects(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let projects = &self.account.projects;
        let title = format!("Projects ({})", projects.len());
        let block = panel(&title, self.focus == Focus::Projects, theme);
        if projects.is_empty() {
            let empty = Paragraph::new("No projects under this name now.")
                .style(theme.dim())
                .block(block);
            frame.render_widget(empty, area);
            return;
        }
        let rows = projects.iter().map(|p| {
            let mut status = vec![status_span(p.status(), theme)];
            if p.is_archived() {
                status.push(Span::styled(" · archived", theme.dim()));
            }
            Row::new(vec![Line::raw(p.name()), Line::from(status)])
        });
        let widths = [Constraint::Fill(1), Constraint::Length(24)];
        let table = Table::new(rows, widths)
            .header(header(&["Project", "Status"], &[], theme))
            .column_spacing(1)
            .block(block)
            .row_highlight_style(theme.selected)
            .highlight_symbol("> ");
        let mut state = self.projects.table_state(projects.len());
        frame.render_stateful_widget(table, area, &mut state);
    }

    fn render_invoices(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let invoices = &self.account.invoices;
        let title = format!("Invoices ({})", invoices.len());
        let block = panel(&title, self.focus == Focus::Invoices, theme);
        if invoices.is_empty() {
            let empty = Paragraph::new("No invoices yet.")
                .style(theme.dim())
                .block(block);
            frame.render_widget(empty, area);
            return;
        }
        let rows = invoices.iter().map(|s| {
            let currency = s.currency();
            Row::new(vec![
                Line::raw(s.number().as_str()),
                Line::raw(s.date().format("%Y-%m-%d").to_string()),
                Line::raw(s.milestones()),
                Line::raw(money_in(s.total(), currency)).right_aligned(),
                Line::raw(money_in(s.balance(), currency)).right_aligned(),
                Line::from(status_span(s.status(), theme)),
            ])
        });
        let widths = [
            Constraint::Length(16),
            Constraint::Length(10),
            Constraint::Fill(1),
            Constraint::Length(14),
            Constraint::Length(14),
            Constraint::Length(12),
        ];
        let table = Table::new(rows, widths)
            .header(header(
                &["Invoice", "Date", "Milestones", "Total", "Due", "Status"],
                &[3, 4],
                theme,
            ))
            .column_spacing(1)
            .block(block)
            .row_highlight_style(theme.selected)
            .highlight_symbol("> ");
        let mut state = self.invoices.table_state(invoices.len());
        frame.render_stateful_widget(table, area, &mut state);
    }
}

/// `text`, or a dim dash when it is empty
fn or_none<'a>(text: &'a str, theme: &Theme) -> Line<'a> {
    if text.is_empty() {
        Line::styled(NONE, theme.dim())
    } else {
        Line::raw(text)
    }
}

/// What is owed in each currency, `₹35,400.00 · €250.00`; empty when nothing is due
fn owed_text(owed: &[Owed]) -> String {
    let amounts: Vec<_> = owed
        .iter()
        .map(|o| money_in(&o.amount, o.currency))
        .collect();
    amounts.join(" · ")
}

impl<R: ClientRepository + ProjectRepository + InvoiceRepository> View for ClientsView<R> {
    fn title(&self) -> &'static str {
        "Clients"
    }

    fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        match &self.opened {
            Some(opened) => opened.render(frame, area, theme),
            None => self.render_list(frame, area, theme),
        }
        if let Some(form) = self.search.as_ref().or(self.editing.as_ref()) {
            form.render(frame, theme);
        }
        if let Some(pending) = &self.pending {
            pending.confirm.render(frame, theme);
        }
    }

    fn handle_key(&mut self, key: Key) -> Action {
        if let Some(form) = self.search.take() {
            let (form, action) = form.step(key, |form| self.search(form));
            self.search = form;
            return action;
        }
        if let Some(form) = self.editing.take() {
            let (form, action) = form.step(key, |form| self.submit_edit(form));
            self.editing = form;
            return action;
        }
        if let Some(pending) = self.pending.take() {
            return self.answer_pending(pending, key);
        }
        match &mut self.opened {
            Some(_) if key == Key::Esc => {
                self.opened = None;
                Action::Handled
            }
            Some(_) if key == Key::Char('e') => self.start_edit(),
            Some(opened) => opened.handle_key(key),
            None => self.handle_list(key),
        }
    }

    fn key_hints(&self) -> &'static [KeyHint] {
        if self.pending.is_some() {
            PENDING_HINTS
        } else if self.search.is_some() || self.editing.is_some() {
            Form::HINTS
        } else if self.opened.is_some() {
            ACCOUNT_HINTS
        } else {
            LIST_HINTS
        }
    }

    fn on_activate(&mut self) -> Option<Notice> {
        let reloaded = self.reload();
        if let Err(ServiceError::NoSuchClient(_)) = reloaded {
            // Only the opened client can be missing; show the list instead
            self.opened = None;
            return None;
        }
        reloaded.err().map(|e| Notice::error(e.to_string()))
    }
}
