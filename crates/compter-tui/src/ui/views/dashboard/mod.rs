//! Dashboard tab: this financial year at a glance, and what clients still owe

use chrono::Local;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Row, Table};

use crate::ui::theme::{Theme, Tone};
use crate::ui::view::{Notice, View};
use crate::ui::views::year::YearPicker;
use crate::ui::widgets::{header, money, money_in, panel, stat_tiles};
use compter_core::domain::InvoiceSummary;
use compter_core::services::reports::YearReport;
use compter_core::services::{InvoiceService, ReportService, ServiceError};
use compter_core::storage::Repository;

/// Room for a label like "Invoiced (net)" or an amount like ₹12,34,567.00, with a gap
const TILE_WIDTH: u16 = 15;

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
            .filter(|s| !s.is_settled())
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
        let stats = self.report.as_ref().map(|report| {
            [
                ("Invoiced (net)", money(&report.gross_receipts)),
                ("Credit notes", money(&report.credited)),
                ("Received", money(&report.received)),
                ("Outstanding", money(&report.outstanding)),
                ("TDS", money(&report.tds)),
                ("Expenses", money(&report.expenses)),
                ("Profit", money(&report.books_profit())),
            ]
        });
        // One row of tiles when they all fit, else two
        let tile_rows: u16 = if area.width >= TILE_WIDTH * 7 + 2 {
            1
        } else {
            2
        };
        let [year_area, unpaid_area] =
            Layout::vertical([Constraint::Length(2 + tile_rows * 2), Constraint::Min(3)])
                .areas(area);
        if let (Some(report), Some(stats)) = (&self.report, &stats) {
            let title = format!("This year, {} · amounts excl. GST", report.year.label());
            let block = panel(&title, false, theme);
            let inner = block.inner(year_area);
            frame.render_widget(block, year_area);
            if tile_rows == 1 {
                stat_tiles(frame, inner, stats, theme);
            } else {
                let [top, bottom] =
                    Layout::vertical([Constraint::Length(2), Constraint::Length(2)]).areas(inner);
                let (first, second) = stats.split_at(4);
                stat_tiles(frame, top, first, theme);
                stat_tiles(frame, bottom, second, theme);
            }
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
