use std::borrow::Cow;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Row, Table};
use termion::event::Key;

use crate::domain::{Billing, Project};
use crate::ui::selection::{Nav, Selection};
use crate::ui::theme::Theme;
use crate::ui::view::{Action, KeyHint, View};
use crate::ui::widgets::{duration, money, panel, status_span};

const HINTS: &[KeyHint] = &[
    KeyHint::new("Tab", "focus"),
    KeyHint::new("j/k", "move"),
    KeyHint::new("n", "new"),
    KeyHint::new("i", "invoice"),
];

/// Wide enough for the longest status label
const STATUS_WIDTH: u16 = 9;
/// Fits up to `999h 59m`
const HOURS_WIDTH: u16 = 8;
/// Fits up to `₹99,99,999.99`
const AMOUNT_WIDTH: u16 = 13;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Projects,
    Milestones,
}

/// Project list on the left, selected project's milestones on the right
pub struct ProjectsView {
    projects: Vec<Project>,
    project_sel: Selection,
    milestone_sel: Selection,
    focus: Focus,
}

impl ProjectsView {
    pub fn new(projects: Vec<Project>) -> Self {
        Self {
            projects,
            project_sel: Selection::default(),
            milestone_sel: Selection::default(),
            focus: Focus::Projects,
        }
    }

    fn selected(&self) -> Option<&Project> {
        self.projects.get(self.project_sel.index())
    }

    fn render_list(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let rows = self.projects.iter().map(|p| {
            Row::new(vec![
                Line::raw(p.name()),
                Line::from(status_span(p.status(), theme)),
            ])
        });
        let widths = [Constraint::Fill(1), Constraint::Length(STATUS_WIDTH)];
        let table = Table::new(rows, widths)
            .block(panel("Projects", self.focus == Focus::Projects, theme))
            .row_highlight_style(theme.selected)
            .highlight_symbol("> ");
        let mut state = self.project_sel.table_state(self.projects.len());
        frame.render_stateful_widget(table, area, &mut state);
    }

    fn render_detail(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let Some(project) = self.selected() else {
            let empty = Paragraph::new("No projects yet. Press n to create one.")
                .style(theme.dim())
                .block(panel("Project", false, theme));
            frame.render_widget(empty, area);
            return;
        };

        let [summary_area, milestones_area] =
            Layout::vertical([Constraint::Length(6), Constraint::Min(0)]).areas(area);

        let block = panel(project.name(), false, theme).title(
            Line::from(vec![
                Span::raw(" "),
                status_span(project.status(), theme),
                Span::raw(" "),
            ])
            .right_aligned(),
        );
        let inner = block.inner(summary_area);
        frame.render_widget(block, summary_area);

        let [client_area, billing_area, stats_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(2),
        ])
        .areas(inner);
        let billing = match project.billing() {
            Billing::Fixed => Span::raw("Fixed price"),
            Billing::Hourly { rate } => Span::raw(format!("Hourly {}/h", money(rate))),
        };
        let client = Line::from(vec![
            Span::styled("Client ", theme.dim()),
            Span::raw(project.client_name()),
            Span::styled("   GSTIN ", theme.dim()),
            Span::raw(
                project
                    .client_gstin()
                    .map_or("unregistered", |g| g.as_str()),
            ),
        ]);
        frame.render_widget(client, client_area);
        let billing = Line::from(vec![Span::styled("Billing ", theme.dim()), billing]);
        frame.render_widget(billing, billing_area);

        let stats = [
            ("Total", money(project.total_amount())),
            ("Paid", money(&project.paid_amount())),
            ("Outstanding", money(&project.outstanding_amount())),
        ];
        let cells = Layout::horizontal([Constraint::Fill(1); 3]).split(stats_area);
        for ((label, value), cell) in stats.into_iter().zip(cells.iter()) {
            let stat = Paragraph::new(vec![
                Line::styled(label, theme.dim()),
                Line::styled(value, theme.key()),
            ]);
            frame.render_widget(stat, *cell);
        }

        let hourly = project.billing().rate().is_some();
        let rows = project.milestones().iter().map(|m| {
            let hours: Cow<'static, str> = if hourly {
                duration(m.logged_minutes()).into()
            } else {
                "—".into()
            };
            Row::new(vec![
                Line::raw(m.name()),
                Line::raw(hours).right_aligned(),
                Line::raw(money(m.amount())).right_aligned(),
                Line::raw(m.due_date().format("%Y-%m-%d").to_string()),
                Line::from(status_span(m.status(), theme)),
            ])
        });
        let widths = [
            Constraint::Fill(1),
            Constraint::Length(HOURS_WIDTH),
            Constraint::Length(AMOUNT_WIDTH),
            Constraint::Length(10),
            Constraint::Length(STATUS_WIDTH),
        ];
        let table = Table::new(rows, widths)
            .header(Row::new(["Name", "Hours", "Amount", "Due", "Status"]).style(theme.key()))
            .column_spacing(1)
            .block(panel("Milestones", self.focus == Focus::Milestones, theme))
            .row_highlight_style(theme.selected)
            .highlight_symbol("> ");
        let mut state = self.milestone_sel.table_state(project.milestones().len());
        frame.render_stateful_widget(table, milestones_area, &mut state);
    }
}

impl View for ProjectsView {
    fn title(&self) -> &'static str {
        "Projects"
    }

    fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let [list_area, detail_area] =
            Layout::horizontal([Constraint::Percentage(32), Constraint::Min(0)]).areas(area);
        self.render_list(frame, list_area, theme);
        self.render_detail(frame, detail_area, theme);
    }

    fn handle_key(&mut self, key: Key) -> Action {
        if let Some(nav) = Nav::from_key(key) {
            match self.focus {
                Focus::Projects => {
                    self.project_sel.move_by(nav, self.projects.len());
                    self.milestone_sel.reset();
                }
                Focus::Milestones => {
                    let len = self.selected().map_or(0, |p| p.milestones().len());
                    self.milestone_sel.move_by(nav, len);
                }
            }
            return Action::Handled;
        }
        match key {
            Key::Char('\t') | Key::BackTab => {
                self.focus = match self.focus {
                    Focus::Projects => Focus::Milestones,
                    Focus::Milestones => Focus::Projects,
                };
                Action::Handled
            }
            Key::Char('n') => Action::Notify("New project: coming in step 4"),
            Key::Char('i') => Action::Notify("Invoicing: coming in step 5"),
            _ => Action::Ignored,
        }
    }

    fn key_hints(&self) -> &'static [KeyHint] {
        HINTS
    }
}
