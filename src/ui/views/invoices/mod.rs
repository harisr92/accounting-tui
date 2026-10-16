//! Invoices tab: every issued invoice, newest first; open the PDF or record a payment

use bigdecimal::BigDecimal;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Row, Table};
use termion::event::Key;

use crate::domain::{Invoice, InvoiceStatus, InvoiceSummary};
use crate::services::{InvoiceService, ServiceError};
use crate::storage::Repository;
use crate::ui::form::Form;
use crate::ui::selection::{Nav, Selection};
use crate::ui::theme::{Theme, Tone};
use crate::ui::view::{Action, KeyHint, Notice, View};
use crate::ui::views::payment::{payment_form, submit_payment};
use crate::ui::widgets::{header, money, money_in, panel};

const HINTS: &[KeyHint] = &[
    KeyHint::new("j/k", "move"),
    KeyHint::new("o", "open PDF"),
    KeyHint::new("p", "record payment"),
];

pub struct InvoicesView<R> {
    service: InvoiceService<R>,
    summaries: Vec<InvoiceSummary>,
    selection: Selection,
    /// The selected invoice in full, for its payment history; the only one held in memory
    current: Option<Invoice>,
    /// Payment form for the selected invoice
    form: Option<Form>,
}

impl<R: Repository> InvoicesView<R> {
    pub fn new(service: InvoiceService<R>) -> Result<Self, ServiceError> {
        let mut view = Self {
            service,
            summaries: Vec::new(),
            selection: Selection::default(),
            current: None,
            form: None,
        };
        view.reload()?;
        Ok(view)
    }

    fn reload(&mut self) -> Result<(), ServiceError> {
        self.summaries = self.service.list()?;
        self.selection.clamp(self.summaries.len());
        self.load_current()
    }

    /// Load the invoice under the cursor, dropping the previous one first
    fn load_current(&mut self) -> Result<(), ServiceError> {
        self.current = None;
        if let Some(summary) = self.summaries.get(self.selection.index()) {
            self.current = Some(self.service.load(summary.number())?);
        }
        Ok(())
    }

    fn selected(&self) -> Option<&InvoiceSummary> {
        self.summaries.get(self.selection.index())
    }

    /// In rupees, foreign invoices at their rate
    fn outstanding(&self) -> BigDecimal {
        self.summaries.iter().map(InvoiceSummary::balance_inr).sum()
    }

    fn open_payment(&mut self) -> Action {
        let Some(summary) = self.selected() else {
            return Action::Notify(Notice::info("No invoice selected"));
        };
        if summary.is_paid() {
            return Action::Notify(Notice::info("This invoice is already paid"));
        }
        match self.service.load(summary.number()) {
            Ok(invoice) => {
                self.form = Some(payment_form(&invoice));
                Action::Handled
            }
            Err(error) => Action::Notify(Notice::error(error.to_string())),
        }
    }

    fn open_pdf(&self) -> Action {
        let Some(summary) = self.selected() else {
            return Action::Notify(Notice::info("No invoice selected"));
        };
        Action::Notify(match self.service.open_pdf(summary.number()) {
            Ok(path) => Notice::success(format!("Opened {}", path.display())),
            Err(error) => Notice::error(error.to_string()),
        })
    }
}

impl<R: Repository> View for InvoicesView<R> {
    fn title(&self) -> &'static str {
        "Invoices"
    }

    fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let title = format!(
            "Invoices ({}) · outstanding {}",
            self.summaries.len(),
            money(&self.outstanding())
        );
        let history_height = self.current.as_ref().map_or(0, |i| {
            u16::try_from(i.payments().len())
                .unwrap_or(u16::MAX)
                .saturating_add(3)
        });
        let [area, history_area] = Layout::vertical([
            Constraint::Min(3),
            Constraint::Length(if history_height > 3 {
                history_height
            } else {
                0
            }),
        ])
        .areas(area);
        if let Some(invoice) = &self.current {
            render_payments(frame, history_area, invoice, theme);
        }
        let block = panel(&title, true, theme);
        if self.summaries.is_empty() {
            let empty = Paragraph::new(
                "No invoices yet. In Projects, select a pending milestone and press i.",
            )
            .style(theme.dim())
            .block(block);
            frame.render_widget(empty, area);
        } else {
            let rows = self.summaries.iter().map(|s| {
                let tone = match s.status() {
                    InvoiceStatus::Paid => Tone::Success,
                    InvoiceStatus::PartlyPaid => Tone::Info,
                    InvoiceStatus::Issued => Tone::Warning,
                };
                let due = if s.is_paid() {
                    "-".to_owned()
                } else {
                    money_in(s.balance(), s.currency())
                };
                Row::new(vec![
                    Line::raw(s.number().as_str()),
                    Line::raw(s.date().format("%d-%m-%Y").to_string()),
                    Line::raw(s.client_name()),
                    Line::raw(s.milestone_name()),
                    Line::raw(s.kind().label()),
                    Line::raw(money_in(s.total(), s.currency())).right_aligned(),
                    Line::raw(due).right_aligned(),
                    Line::styled(s.status().label(), theme.tone(tone)),
                ])
            });
            let widths = [
                Constraint::Length(16),
                Constraint::Length(10),
                Constraint::Fill(1),
                Constraint::Fill(1),
                Constraint::Length(6),
                Constraint::Length(14),
                Constraint::Length(14),
                Constraint::Length(9),
            ];
            let table = Table::new(rows, widths)
                .header(header(
                    &[
                        "Number",
                        "Date",
                        "Client",
                        "Milestone",
                        "Kind",
                        "Total",
                        "Due",
                        "Status",
                    ],
                    &[5, 6],
                    theme,
                ))
                .column_spacing(1)
                .block(block)
                .row_highlight_style(theme.selected)
                .highlight_symbol("> ");
            let mut state = self.selection.table_state(self.summaries.len());
            frame.render_stateful_widget(table, area, &mut state);
        }
        if let Some(form) = &self.form {
            form.render(frame, theme);
        }
    }

    fn handle_key(&mut self, key: Key) -> Action {
        if let Some(form) = self.form.take() {
            let Some(number) = self.selected().map(|s| s.number().clone()) else {
                return Action::Handled;
            };
            let service = &mut self.service;
            let (form, action) = form.step(key, |form| submit_payment(service, &number, form));
            self.form = form;
            if matches!(action, Action::Notify(_))
                && let Err(error) = self.reload()
            {
                return Action::Notify(Notice::error(error.to_string()));
            }
            return action;
        }
        if let Some(nav) = Nav::from_key(key) {
            self.selection.move_by(nav, self.summaries.len());
            return match self.load_current() {
                Ok(()) => Action::Handled,
                Err(error) => Action::Notify(Notice::error(error.to_string())),
            };
        }
        match key {
            Key::Char('p') => self.open_payment(),
            Key::Char('o') => self.open_pdf(),
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

/// Every payment received against the selected invoice
fn render_payments(frame: &mut Frame, area: Rect, invoice: &Invoice, theme: &Theme) {
    let currency = invoice.currency();
    let title = format!(
        "Payments for {} · {} due",
        invoice.number(),
        money_in(&invoice.balance(), currency)
    );
    // Foreign clients withhold no TDS; show the rupees banked instead
    let foreign = !currency.is_inr();
    let rows = invoice.payments().iter().map(|p| {
        let third = if foreign {
            money(p.received_inr())
        } else {
            money(&p.tds)
        };
        Row::new(vec![
            Line::raw(p.date.format("%d-%m-%Y").to_string()),
            Line::raw(money_in(&p.received, currency)).right_aligned(),
            Line::raw(third).right_aligned(),
            Line::raw(money_in(&p.settled(), currency)).right_aligned(),
        ])
    });
    let titles: &[&str] = if foreign {
        &["Date", "Received", "Banked", "Settled"]
    } else {
        &["Date", "Received", "TDS", "Settled"]
    };
    let widths = [
        Constraint::Length(10),
        Constraint::Length(14),
        Constraint::Length(14),
        Constraint::Length(14),
    ];
    let table = Table::new(rows, widths)
        .header(header(titles, &[1, 2, 3], theme))
        .column_spacing(2)
        .block(panel(&title, false, theme));
    frame.render_widget(table, area);
}
