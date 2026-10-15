use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Row, Table};

use super::{Focus, ProjectsView};
use crate::domain::{Billing, Milestone, Project};
use crate::ui::theme::Theme;
use crate::ui::widgets::{duration, money, panel, status_span};

/// Wide enough for the longest status label
const STATUS_WIDTH: u16 = 9;
/// Fits up to `999h 59m`
const HOURS_WIDTH: u16 = 8;
/// Fits up to `₹99,99,999.99`
const AMOUNT_WIDTH: u16 = 13;
/// Below this the milestone table drops its Due column
const WIDE_TABLE: u16 = 56;

#[derive(Debug, Clone, Copy)]
enum Column {
    Name,
    Hours,
    Amount,
    Due,
    Status,
}

impl Column {
    fn title(self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::Hours => "Hours",
            Self::Amount => "Amount",
            Self::Due => "Due",
            Self::Status => "Status",
        }
    }

    fn width(self) -> Constraint {
        match self {
            Self::Name => Constraint::Fill(1),
            Self::Hours => Constraint::Length(HOURS_WIDTH),
            Self::Amount => Constraint::Length(AMOUNT_WIDTH),
            Self::Due => Constraint::Length(10),
            Self::Status => Constraint::Length(STATUS_WIDTH),
        }
    }

    fn cell<'a>(self, m: &'a Milestone, theme: &Theme) -> Line<'a> {
        match self {
            Self::Name => Line::raw(m.name()),
            Self::Hours => Line::raw(duration(m.logged_minutes())).right_aligned(),
            Self::Amount => Line::raw(money(m.amount())).right_aligned(),
            Self::Due => Line::raw(m.due_date().format("%Y-%m-%d").to_string()),
            Self::Status => Line::from(status_span(m.status(), theme)),
        }
    }
}

impl<R> ProjectsView<R> {
    pub(super) fn render_list(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let rows = self.summaries.iter().map(|p| {
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
        let mut state = self.project_sel.table_state(self.summaries.len());
        frame.render_stateful_widget(table, area, &mut state);
    }

    pub(super) fn render_detail(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let Some(project) = &self.selected else {
            let message = if self.summaries.is_empty() {
                "No projects yet. Press n to create one."
            } else {
                "This project could not be loaded."
            };
            let empty = Paragraph::new(message)
                .style(theme.dim())
                .block(panel("Project", false, theme));
            frame.render_widget(empty, area);
            return;
        };

        let [summary_area, milestones_area] =
            Layout::vertical([Constraint::Length(6), Constraint::Min(0)]).areas(area);
        render_summary(frame, summary_area, project, theme);
        self.render_milestones(frame, milestones_area, project, theme);
    }

    fn render_milestones(&self, frame: &mut Frame, area: Rect, project: &Project, theme: &Theme) {
        let block = panel("Milestones", self.focus == Focus::Milestones, theme);
        if project.milestones().is_empty() {
            let empty = Paragraph::new("No milestones yet. Press Tab, then a to add one.")
                .style(theme.dim())
                .block(block);
            frame.render_widget(empty, area);
            return;
        }

        let hourly = project.billing().rate().is_some();
        // Hours only matter on hourly work; the due date gives way on narrow screens
        let columns: Vec<Column> = [
            Some(Column::Name),
            hourly.then_some(Column::Hours),
            Some(Column::Amount),
            (area.width >= WIDE_TABLE).then_some(Column::Due),
            Some(Column::Status),
        ]
        .into_iter()
        .flatten()
        .collect();

        let rows = project
            .milestones()
            .iter()
            .map(|m| Row::new(columns.iter().map(|column| column.cell(m, theme))));
        let table = Table::new(rows, columns.iter().map(|c| c.width()))
            .header(Row::new(columns.iter().map(|c| c.title())).style(theme.key()))
            .column_spacing(1)
            .block(block)
            .row_highlight_style(theme.selected)
            .highlight_symbol("> ");
        let mut state = self.milestone_sel.table_state(project.milestones().len());
        frame.render_stateful_widget(table, area, &mut state);
    }
}

fn render_summary(frame: &mut Frame, area: Rect, project: &Project, theme: &Theme) {
    let block = panel(project.name(), false, theme).title(
        Line::from(vec![
            Span::raw(" "),
            status_span(project.status(), theme),
            Span::raw(" "),
        ])
        .right_aligned(),
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let [client_area, billing_area, stats_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(2),
    ])
    .areas(inner);
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
    let billing = match project.billing() {
        Billing::Fixed => Span::raw("Fixed price"),
        Billing::Hourly { rate } => Span::raw(format!("Hourly {}/h", money(rate))),
    };
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
}
