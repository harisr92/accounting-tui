//! Dashboard tab: this financial year at a glance, and what clients still owe

use chrono::Local;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Row, Table};

use crate::domain::InvoiceSummary;
use crate::services::reports::YearReport;
use crate::services::{InvoiceService, ReportService, ServiceError};
use crate::storage::Repository;
use crate::ui::theme::{Theme, Tone};
use crate::ui::view::{Notice, View};
use crate::ui::views::year::YearPicker;
use crate::ui::widgets::{header, money, money_in, panel, stat_tiles};

/// After this many days an unpaid invoice is shown as overdue
const OVERDUE_DAYS: i64 = 30;

pub struct DashboardView<R> {
    reports: ReportService<R>,
    invoices: InvoiceService<R>,
    report: Option<YearReport>,
    /// Unpaid invoices, oldest first
    unpaid: Vec<InvoiceSummary>,
}

impl<R: Repository> DashboardView<R> {
    pub fn new(
        reports: ReportService<R>,
        invoices: InvoiceService<R>,
    ) -> Result<Self, ServiceError> {
        let mut view = Self {
            reports,
            invoices,
            report: None,
            unpaid: Vec::new(),
        };
        view.reload()?;
        Ok(view)
    }

    fn reload(&mut self) -> Result<(), ServiceError> {
        self.report = Some(self.reports.year(YearPicker::current().year())?);
        let mut unpaid: Vec<_> = self
            .invoices
            .list()?
            .into_iter()
            .filter(|s| !s.is_paid())
            .collect();
        unpaid.reverse();
        self.unpaid = unpaid;
        Ok(())
    }
}

impl<R: Repository> View for DashboardView<R> {
    fn title(&self) -> &'static str {
        "Dashboard"
    }

    fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let [year_area, unpaid_area] =
            Layout::vertical([Constraint::Length(4), Constraint::Min(3)]).areas(area);
        if let Some(report) = &self.report {
            let title = format!("This year, {} · amounts excl. GST", report.year.label());
            let block = panel(&title, false, theme);
            let inner = block.inner(year_area);
            frame.render_widget(block, year_area);
            let stats = [
                ("Invoiced", money(&report.gross_receipts)),
                ("Received", money(&report.received)),
                ("Outstanding", money(&report.outstanding)),
                ("TDS", money(&report.tds)),
                ("Expenses", money(&report.expenses)),
                ("Profit", money(&report.books_profit())),
            ];
            stat_tiles(frame, inner, &stats, theme);
        }

        let block = panel("Waiting for payment", false, theme);
        if self.unpaid.is_empty() {
            let empty = Paragraph::new("Every invoice is paid.")
                .style(theme.dim())
                .block(block);
            frame.render_widget(empty, unpaid_area);
            return;
        }
        let today = Local::now().date_naive();
        let rows = self.unpaid.iter().map(|s| {
            let days = (today - s.date()).num_days();
            let tone = if days > OVERDUE_DAYS {
                Tone::Error
            } else {
                Tone::Neutral
            };
            Row::new(vec![
                Line::raw(s.number().as_str()),
                Line::raw(s.client_name()),
                Line::raw(s.milestone_name()),
                Line::raw(money_in(s.balance(), s.currency())).right_aligned(),
                Line::styled(format!("{days} days"), theme.tone(tone)).right_aligned(),
            ])
        });
        let widths = [
            Constraint::Length(16),
            Constraint::Fill(1),
            Constraint::Fill(1),
            Constraint::Length(14),
            Constraint::Length(9),
        ];
        let table = Table::new(rows, widths)
            .header(header(
                &["Invoice", "Client", "Milestone", "Due", "Waiting"],
                &[3, 4],
                theme,
            ))
            .column_spacing(1)
            .block(block);
        frame.render_widget(table, unpaid_area);
    }

    fn on_activate(&mut self) -> Option<Notice> {
        self.reload().err().map(|e| Notice::error(e.to_string()))
    }
}
