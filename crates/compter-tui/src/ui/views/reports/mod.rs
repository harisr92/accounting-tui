//! Reports tab: a financial year for income tax and GST, the quarterly GSTR-1 export and the
//! monthly IFF

use crate::ui::key::Key;
use bigdecimal::Zero;
use chrono::Local;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Row, Table, Wrap};

use super::year::{self, YearPicker};
use crate::ui::form::{Field, Form};
use crate::ui::theme::{Theme, Tone};
use crate::ui::view::{Action, KeyHint, Notice, View};
use crate::ui::widgets::{header, money, panel, stat_tiles};
use compter_core::services::reports::{PRESUMPTIVE_PERCENT, PresumptiveCheck, YearReport};
use compter_core::services::{ReportService, ServiceError};
use compter_core::storage::Repository;

const HINTS: &[KeyHint] = &[
    year::HINT,
    KeyHint::new("g", "export GSTR-1"),
    KeyHint::new("i", "export IFF"),
];
const QUARTER_OPTIONS: &[&str] = &[
    "Q1 (Apr-Jun)",
    "Q2 (Jul-Sep)",
    "Q3 (Oct-Dec)",
    "Q4 (Jan-Mar)",
];
/// Which of the quarter's first two months had their B2B invoices filed in an IFF
const FURNISHED_OPTIONS: &[&str] = &["neither", "1st month", "2nd month", "both"];
/// The months an IFF can be filed for: the first two of each quarter
const IFF_MONTHS: &[&str] = &["Apr", "May", "Jul", "Aug", "Oct", "Nov", "Jan", "Feb"];

const QUARTER: usize = 0;
const FURNISHED: usize = 1;

/// The export form that is open; it takes every key
enum Modal {
    Gstr1(Form),
    Iff(Form),
}

pub struct ReportsView<R> {
    service: ReportService<R>,
    year: YearPicker,
    report: Option<YearReport>,
    modal: Option<Modal>,
}

impl<R: Repository> ReportsView<R> {
    pub fn new(service: ReportService<R>) -> Result<Self, ServiceError> {
        let mut view = Self {
            service,
            year: YearPicker::current(),
            report: None,
            modal: None,
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
        let quarter = self.year.year().quarter_to_file(Local::now().date_naive());
        usize::from(quarter.number() - 1)
    }

    fn export(&mut self, form: &Form) -> Result<Notice, String> {
        let quarter = self.year.year().quarters()[form.choice(QUARTER).min(3)];
        let [first, second, _] = quarter.months();
        let furnished: &[(i32, u32)] = match form.choice(FURNISHED) {
            1 => &[first],
            2 => &[second],
            3 => &[first, second],
            _ => &[],
        };
        let (path, gstr1) = self
            .service
            .export_gstr1(quarter, furnished)
            .map_err(|e| e.to_string())?;
        Ok(Notice::success(format!(
            "GSTR-1 {} {}: {} saved to {}",
            quarter.label(),
            quarter.year().label(),
            gstr1.summary(),
            file_name(&path)
        )))
    }

    fn export_iff(&mut self, form: &Form) -> Result<Notice, String> {
        let choice = form.choice(0).min(IFF_MONTHS.len() - 1);
        let month = self.year.year().iff_months()[choice];
        let (path, iff) = self.service.export_iff(month).map_err(|e| e.to_string())?;
        Ok(Notice::success(format!(
            "IFF {} {}: {} saved to {}",
            IFF_MONTHS[choice],
            month.0,
            iff.summary(),
            file_name(&path)
        )))
    }

    /// The IFF month to offer first: the latest first-or-second month that has ended
    fn default_iff_month(&self) -> usize {
        self.year
            .year()
            .iff_month_to_file(Local::now().date_naive())
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
        let switch_note = report.switch_note();
        // Two more lines for what crossed over in the year the basis changed
        let tax_height = if switch_note.is_some() { 8 } else { 6 };
        let [income_area, tax_area, gst_area, clients_area] = Layout::vertical([
            Constraint::Length(4),
            Constraint::Length(tax_height),
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
        if !report.credited.is_zero() {
            income.insert(2, ("Credit notes", money(&report.credited)));
        }
        if !report.exchange_difference.is_zero() {
            income.push(("Exchange gain", money(&report.exchange_difference)));
        }
        stat_tiles(frame, inner, &income, theme);

        render_income_tax(frame, tax_area, report, switch_note.as_deref(), theme);
        render_gst(frame, gst_area, report, theme);
        render_clients(frame, clients_area, report, theme);

        if let Some(Modal::Gstr1(form) | Modal::Iff(form)) = &self.modal {
            form.render(frame, theme);
        }
    }

    fn handle_key(&mut self, key: Key) -> Action {
        match self.modal.take() {
            Some(Modal::Gstr1(form)) => {
                let (form, action) = form.step(key, |form| self.export(form));
                self.modal = form.map(Modal::Gstr1);
                return action;
            }
            Some(Modal::Iff(form)) => {
                let (form, action) = form.step(key, |form| self.export_iff(form));
                self.modal = form.map(Modal::Iff);
                return action;
            }
            None => {}
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
                let furnished = Field::choice("IFF filed", FURNISHED_OPTIONS);
                self.modal = Some(Modal::Gstr1(Form::new(
                    "Export GSTR-1",
                    vec![quarter, furnished],
                )));
                Action::Handled
            }
            Key::Char('i') => {
                let month =
                    Field::choice("Month", IFF_MONTHS).with_choice(self.default_iff_month());
                self.modal = Some(Modal::Iff(Form::new("Export IFF (B2B)", vec![month])));
                Action::Handled
            }
            _ => Action::Ignored,
        }
    }

    fn key_hints(&self) -> &'static [KeyHint] {
        if self.modal.is_some() {
            Form::HINTS
        } else {
            HINTS
        }
    }

    fn on_activate(&mut self) -> Option<Notice> {
        self.reload().err().map(|e| Notice::error(e.to_string()))
    }
}

fn file_name(path: &std::path::Path) -> std::borrow::Cow<'_, str> {
    path.file_name()
        .map(|f| f.to_string_lossy())
        .unwrap_or_default()
}

fn render_income_tax(
    frame: &mut Frame,
    area: Rect,
    report: &YearReport,
    switch_note: Option<&str>,
    theme: &Theme,
) {
    let block = panel("Income tax", false, theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let [tiles, note] = Layout::vertical([Constraint::Length(2), Constraint::Min(2)]).areas(inner);
    let presumptive_label = format!("44ADA income ({PRESUMPTIVE_PERCENT}%)");
    let receipts_label = format!("Receipts ({})", report.basis.label());
    let stats = [
        (receipts_label.as_str(), money(&report.tax_receipts)),
        (
            presumptive_label.as_str(),
            money(&report.presumptive_income()),
        ),
        ("Expenses", money(&report.expenses)),
        ("Books profit", money(&report.books_profit())),
    ];
    stat_tiles(frame, tiles, &stats, theme);
    let check = report.presumptive_check();
    let tone = match check {
        PresumptiveCheck::OverLimit => Tone::Error,
        PresumptiveCheck::BooksBelow => Tone::Warning,
        PresumptiveCheck::Within => Tone::Neutral,
    };
    let mut lines = Vec::with_capacity(2);
    if let Some(switch) = switch_note {
        lines.push(Line::styled(switch, theme.tone(Tone::Warning)));
    }
    lines.push(Line::styled(report.presumptive_note(), theme.tone(tone)));
    let note_text = Paragraph::new(lines).wrap(Wrap { trim: true });
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
