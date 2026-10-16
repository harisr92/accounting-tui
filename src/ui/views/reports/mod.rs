//! Reports tab: a financial year for income tax and GST, and the quarterly GSTR-1 export

use crate::ui::key::Key;
use bigdecimal::Zero;
use chrono::Local;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Row, Table, Wrap};

use super::year::{self, YearPicker};
use crate::domain::Quarter;
use crate::services::reports::{PRESUMPTIVE_PERCENT, YearReport};
use crate::services::{ReportService, ServiceError};
use crate::storage::Repository;
use crate::ui::form::{Field, Form};
use crate::ui::theme::{Theme, Tone};
use crate::ui::view::{Action, KeyHint, Notice, View};
use crate::ui::widgets::{header, money, panel, stat_tiles};

const HINTS: &[KeyHint] = &[year::HINT, KeyHint::new("g", "export GSTR-1")];
const QUARTER_OPTIONS: &[&str] = &[
    "Q1 (Apr-Jun)",
    "Q2 (Jul-Sep)",
    "Q3 (Oct-Dec)",
    "Q4 (Jan-Mar)",
];

pub struct ReportsView<R> {
    service: ReportService<R>,
    year: YearPicker,
    report: Option<YearReport>,
    /// Quarter picker for the GSTR-1 export
    form: Option<Form>,
}

impl<R: Repository> ReportsView<R> {
    pub fn new(service: ReportService<R>) -> Result<Self, ServiceError> {
        let mut view = Self {
            service,
            year: YearPicker::current(),
            report: None,
            form: None,
        };
        view.reload()?;
        Ok(view)
    }

    fn reload(&mut self) -> Result<(), ServiceError> {
        self.report = Some(self.service.year(self.year.year())?);
        Ok(())
    }

    /// The quarter to offer first: the last one that has ended, or Q1 of a future year
    fn default_quarter(&self) -> usize {
        let today = Local::now().date_naive();
        let current = Quarter::containing(today);
        if current.year() == self.year.year() {
            usize::from(current.number().saturating_sub(2))
        } else if self.year.year() < current.year() {
            3
        } else {
            0
        }
    }

    fn export(&mut self, form: &Form) -> Result<Notice, String> {
        let quarter = self.year.year().quarters()[form.choice(0).min(3)];
        let (path, gstr1) = self
            .service
            .export_gstr1(quarter)
            .map_err(|e| e.to_string())?;
        Ok(Notice::success(format!(
            "GSTR-1 {} {}: {} tax invoice(s), {} export(s) saved to {}",
            quarter.label(),
            quarter.year().label(),
            gstr1.tax_invoices,
            gstr1.exports,
            path.file_name()
                .map(|f| f.to_string_lossy())
                .unwrap_or_default()
        )))
    }
}

impl<R: Repository> View for ReportsView<R> {
    fn title(&self) -> &'static str {
        "Reports"
    }

    fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let Some(report) = &self.report else {
            return;
        };
        let [income_area, tax_area, gst_area, clients_area] = Layout::vertical([
            Constraint::Length(4),
            Constraint::Length(6),
            Constraint::Length(7),
            Constraint::Min(3),
        ])
        .areas(area);

        let title = format!("Income {}", report.year.label());
        let block = panel(&title, true, theme);
        let inner = block.inner(income_area);
        frame.render_widget(block, income_area);
        let mut income = vec![
            ("Invoices", report.invoice_count.to_string()),
            ("Receipts", money(&report.gross_receipts)),
            ("Exports", money(&report.export_receipts)),
            ("Received", money(&report.received)),
            ("TDS", money(&report.tds)),
            ("Outstanding", money(&report.outstanding)),
        ];
        if !report.exchange_difference.is_zero() {
            income.push(("Exchange gain", money(&report.exchange_difference)));
        }
        stat_tiles(frame, inner, &income, theme);

        render_income_tax(frame, tax_area, report, theme);
        render_gst(frame, gst_area, report, theme);
        render_clients(frame, clients_area, report, theme);

        if let Some(form) = &self.form {
            form.render(frame, theme);
        }
    }

    fn handle_key(&mut self, key: Key) -> Action {
        if let Some(form) = self.form.take() {
            let (form, action) = form.step(key, |form| self.export(form));
            self.form = form;
            return action;
        }
        if self.year.handle(key) {
            return match self.reload() {
                Ok(()) => Action::Handled,
                Err(error) => Action::Notify(Notice::error(error.to_string())),
            };
        }
        match key {
            Key::Char('g') => {
                let quarter =
                    Field::choice("Quarter", QUARTER_OPTIONS).with_choice(self.default_quarter());
                self.form = Some(Form::new("Export GSTR-1", vec![quarter]));
                Action::Handled
            }
            _ => Action::Ignored,
        }
    }

    fn key_hints(&self) -> &'static [KeyHint] {
        if self.form.is_some() {
            Form::HINTS
        } else {
            HINTS
        }
    }

    fn on_activate(&mut self) -> Option<Notice> {
        self.reload().err().map(|e| Notice::error(e.to_string()))
    }
}

fn render_income_tax(frame: &mut Frame, area: Rect, report: &YearReport, theme: &Theme) {
    let block = panel("Income tax", false, theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let [tiles, note] =
        Layout::vertical([Constraint::Length(2), Constraint::Length(2)]).areas(inner);
    let presumptive_label = format!("44ADA income ({PRESUMPTIVE_PERCENT}%)");
    let stats = [
        (
            presumptive_label.as_str(),
            money(&report.presumptive_income()),
        ),
        ("Expenses", money(&report.expenses)),
        ("Books profit", money(&report.books_profit())),
    ];
    stat_tiles(frame, tiles, &stats, theme);
    let (text, tone) = if !report.within_presumptive_limit() {
        (
            "Gross receipts are over ₹75 lakh: 44ADA is not available, use your books",
            Tone::Error,
        )
    } else if report.books_profit() < report.presumptive_income() {
        (
            "Books profit is below 44ADA income: declaring the lower figure needs a tax audit (44AB) if income is over the basic exemption",
            Tone::Warning,
        )
    } else {
        (
            "Within the 44ADA limit. Gross receipts are invoices dated in the year, excl. GST",
            Tone::Neutral,
        )
    };
    let note_text = Paragraph::new(Line::styled(text, theme.tone(tone))).wrap(Wrap { trim: true });
    frame.render_widget(note_text, note);
}

fn render_gst(frame: &mut Frame, area: Rect, report: &YearReport, theme: &Theme) {
    let rows = report.quarters.iter().map(|q| {
        // Negative: input credit carried forward
        let net = money(&q.net());
        Row::new(vec![
            Line::raw(q.quarter.label()),
            Line::raw(money(&q.cgst)).right_aligned(),
            Line::raw(money(&q.sgst)).right_aligned(),
            Line::raw(money(&q.igst)).right_aligned(),
            Line::raw(money(&q.input_credit)).right_aligned(),
            Line::raw(net).right_aligned(),
        ])
    });
    let widths = [
        Constraint::Length(14),
        Constraint::Fill(1),
        Constraint::Fill(1),
        Constraint::Fill(1),
        Constraint::Fill(1),
        Constraint::Fill(1),
    ];
    let table = Table::new(rows, widths)
        .header(header(
            &[
                "Quarter",
                "CGST",
                "SGST",
                "IGST",
                "Input credit",
                "Net (- credit)",
            ],
            &[1, 2, 3, 4, 5],
            theme,
        ))
        .column_spacing(1)
        .block(panel("GST", false, theme));
    frame.render_widget(table, area);
}

fn render_clients(frame: &mut Frame, area: Rect, report: &YearReport, theme: &Theme) {
    let block = panel("Clients · match TDS with Form 26AS", false, theme);
    if report.clients.is_empty() {
        let empty = Paragraph::new(Span::styled(
            "No invoices or payments this year.",
            theme.dim(),
        ))
        .block(block);
        frame.render_widget(empty, area);
        return;
    }
    let rows = report.clients.iter().map(|c| {
        Row::new(vec![
            Line::raw(c.client.as_str()),
            Line::raw(money(&c.invoiced)).right_aligned(),
            Line::raw(money(&c.received)).right_aligned(),
            Line::raw(money(&c.tds)).right_aligned(),
        ])
    });
    let widths = [
        Constraint::Fill(2),
        Constraint::Fill(1),
        Constraint::Fill(1),
        Constraint::Fill(1),
    ];
    let table = Table::new(rows, widths)
        .header(header(
            &["Client", "Invoiced", "Received", "TDS"],
            &[1, 2, 3],
            theme,
        ))
        .column_spacing(1)
        .block(block);
    frame.render_widget(table, area);
}
