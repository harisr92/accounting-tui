use bigdecimal::Zero;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Row, Table, TableState};

use super::{Focus, ProjectsView};
use crate::domain::{Billing, Currency, Milestone, Project};
use crate::ui::theme::Theme;
use crate::ui::widgets::{duration, header, money_in, panel, stat_tiles, status_span};

/// Wide enough for the longest status label
const STATUS_WIDTH: u16 = 9;
/// Fits up to `999h 59m`
const HOURS_WIDTH: u16 = 8;
/// Fits up to `₹99,99,999.99`
const AMOUNT_WIDTH: u16 = 13;
/// Below this the milestone table drops its Due column
const WIDE_TABLE: u16 = 56;
/// Below this the Due column also gives way while a time log is open
const WIDE_WITH_LOG: u16 = 80;

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

    /// Amounts and durations line up on the right
    fn right_aligned(self) -> bool {
        matches!(self, Self::Hours | Self::Amount)
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

    fn cell<'a>(self, m: &'a Milestone, currency: Currency, theme: &Theme) -> Line<'a> {
        match self {
            Self::Name => Line::raw(m.name()),
            Self::Hours => Line::raw(duration(m.logged_minutes())).right_aligned(),
            Self::Amount => Line::raw(money_in(m.amount(), currency)).right_aligned(),
            Self::Due => Line::raw(m.due_date().format("%Y-%m-%d").to_string()),
            Self::Status => Line::from(status_span(m.status(), theme)),
        }
    }
}

impl<R> ProjectsView<R> {
    pub(super) fn render_list(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let rows = self
            .visible
            .iter()
            .filter_map(|&i| self.summaries.get(i))
            .map(|p| {
                // Archived projects are only listed while `H` shows them; their names are dimmed
                let name = if p.is_archived() {
                    Line::styled(p.name(), theme.dim())
                } else {
                    Line::raw(p.name())
                };
                Row::new(vec![name, Line::from(status_span(p.status(), theme))])
            });
        let title = match (self.filter.is_empty(), self.show_archived) {
            (true, false) => "Projects".to_owned(),
            (true, true) => "Projects + archived".to_owned(),
            (false, archived) => format!(
                "Projects{} /{}",
                if archived { " + archived" } else { "" },
                self.filter.query()
            ),
        };
        let widths = [Constraint::Fill(1), Constraint::Length(STATUS_WIDTH)];
        let table = Table::new(rows, widths)
            .block(panel(&title, self.focus == Focus::Projects, theme))
            .row_highlight_style(theme.selected)
            .highlight_symbol("> ");
        let mut state = self.project_sel.table_state(self.visible.len());
        frame.render_stateful_widget(table, area, &mut state);
    }

    pub(super) fn render_detail(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let Some(project) = &self.selected else {
            let message = if self.summaries.is_empty() {
                "No projects yet. Press n to create one."
            } else if !self.filter.is_empty() && self.visible.is_empty() {
                "No projects match the search. Esc clears it."
            } else if self.visible.is_empty() {
                "Every project is archived. Press H to show them, or n to create one."
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
        // Hours only matter on hourly work. The due date gives way on narrow screens, and on
        // medium ones while a time log is open, so its notes have room.
        let log_open = project
            .milestones()
            .iter()
            .any(|m| self.open_logs.contains(&m.id()));
        let show_due = area.width >= WIDE_TABLE && !(log_open && area.width < WIDE_WITH_LOG);
        let columns: Vec<Column> = [
            Some(Column::Name),
            hourly.then_some(Column::Hours),
            Some(Column::Amount),
            show_due.then_some(Column::Due),
            Some(Column::Status),
        ]
        .into_iter()
        .flatten()
        .collect();

        let titles: Vec<&str> = columns.iter().map(|c| c.title()).collect();
        let right: Vec<usize> = (0..columns.len())
            .filter(|&i| columns[i].right_aligned())
            .collect();
        // Each open milestone is followed by its time entries, so the selected milestone's
        // display row is its index plus the entry rows shown above it
        let mut rows = Vec::with_capacity(project.milestones().len());
        let mut selected_row = None;
        let mut selected_end = 0;
        for (index, m) in project.milestones().iter().enumerate() {
            let selected = index == self.milestone_sel.index();
            if selected {
                selected_row = Some(rows.len());
            }
            let currency = project.currency();
            rows.push(Row::new(
                columns.iter().map(|column| column.cell(m, currency, theme)),
            ));
            if self.open_logs.contains(&m.id()) {
                rows.extend(time_log_rows(m, &columns, theme));
            }
            if selected {
                selected_end = rows.len();
            }
        }
        // Scroll so the selected milestone's open log is visible too, keeping the milestone
        // itself on screen when its log is taller than the panel (borders and header take 3)
        let visible = usize::from(area.height.saturating_sub(3));
        let offset = selected_end
            .saturating_sub(visible)
            .min(selected_row.unwrap_or_default());
        let table = Table::new(rows, columns.iter().map(|c| c.width()))
            .header(header(&titles, &right, theme))
            .column_spacing(1)
            .block(block)
            .row_highlight_style(theme.selected)
            .highlight_symbol("> ");
        let mut state = TableState::default()
            .with_offset(offset)
            .with_selected(selected_row);
        frame.render_stateful_widget(table, area, &mut state);
    }
}

/// The time log under an open milestone: one row per entry with its date (`06 Oct`, to leave
/// room for the note), note and duration
fn time_log_rows<'a>(milestone: &'a Milestone, columns: &[Column], theme: &Theme) -> Vec<Row<'a>> {
    let entries = milestone.time_entries();
    if entries.is_empty() {
        let cells = columns.iter().map(|column| match column {
            Column::Name => Line::styled("└ no time logged yet; t logs some", theme.dim()),
            _ => Line::default(),
        });
        return vec![Row::new(cells)];
    }
    let last = entries.len() - 1;
    entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let branch = if i == last { "└ " } else { "├ " };
            let cells = columns.iter().map(|column| match column {
                Column::Name => Line::from(vec![
                    Span::styled(branch, theme.dim()),
                    Span::styled(entry.date().format("%d %b").to_string(), theme.dim()),
                    Span::raw(" "),
                    Span::raw(entry.note()),
                ]),
                Column::Hours => {
                    Line::styled(duration(entry.minutes()), theme.dim()).right_aligned()
                }
                _ => Line::default(),
            });
            Row::new(cells)
        })
        .collect()
}

fn render_summary(frame: &mut Frame, area: Rect, project: &Project, theme: &Theme) {
    let mut status = vec![Span::raw(" "), status_span(project.status(), theme)];
    if project.is_archived() {
        status.push(Span::styled(" · ARCHIVED", theme.dim()));
    }
    status.push(Span::raw(" "));
    let block = panel(project.name(), false, theme).title(Line::from(status).right_aligned());
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
    let currency = project.currency();
    let billing = match project.billing() {
        Billing::Fixed => Span::raw("Fixed price"),
        Billing::Hourly { rate } => Span::raw(format!("Hourly {}/h", money_in(rate, currency))),
    };
    let billing = Line::from(vec![Span::styled("Billing ", theme.dim()), billing]);
    frame.render_widget(billing, billing_area);

    let mut stats = vec![
        (
            "Total excl. GST",
            money_in(project.total_amount(), currency),
        ),
        ("Paid", money_in(&project.paid_amount(), currency)),
        (
            "Outstanding",
            money_in(&project.outstanding_amount(), currency),
        ),
    ];
    let credited = project.credited_amount();
    if !credited.is_zero() {
        stats.insert(2, ("Credited", money_in(&credited, currency)));
    }
    stat_tiles(frame, stats_area, &stats, theme);
}
