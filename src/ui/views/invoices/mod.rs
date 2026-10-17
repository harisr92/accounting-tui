//! Invoices tab: every issued invoice, newest first; open the PDF, record a payment or issue a
//! credit note

use crate::ui::key::Key;
use bigdecimal::{BigDecimal, Zero};
use chrono::Local;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Row, Table};

use crate::domain::{Invoice, InvoiceNumber, InvoiceSummary};
use crate::services::{CreditDraft, CreditPreview, Credited, InvoiceService, ServiceError};
use crate::storage::Repository;
use crate::ui::filter::Filter;
use crate::ui::form::{Confirm, Field, Form, parse};
use crate::ui::selection::{Nav, Selection};
use crate::ui::theme::Theme;
use crate::ui::view::{Action, KeyHint, Notice, View};
use crate::ui::views::payment::{payment_form, submit_payment};
use crate::ui::widgets::{header, money, money_in, panel, status_span};

const HINTS: &[KeyHint] = &[
    KeyHint::new("j/k", "move"),
    KeyHint::new("o", "open PDF"),
    KeyHint::new("p", "record payment"),
    KeyHint::new("c", "credit note"),
    KeyHint::new("O", "open credit note"),
    Filter::HINT,
    Filter::CLEAR_HINT,
];

const CREDIT_DATE: usize = 0;
const CREDIT_AMOUNT: usize = 1;
const CREDIT_REASON: usize = 2;

/// What is open over the list; it takes every key
enum Modal {
    /// Payment for the selected invoice
    Payment(Form),
    /// A credit note against the selected invoice
    Credit(Form),
    /// The credit note as it will be issued, waiting for a yes
    ConfirmCredit(CreditDraft, Confirm),
    Search(Form),
}

pub struct InvoicesView<R> {
    service: InvoiceService<R>,
    summaries: Vec<InvoiceSummary>,
    /// Positions in `summaries` of the rows matching the search; `selection` moves over these
    visible: Vec<usize>,
    filter: Filter,
    selection: Selection,
    /// The selected invoice in full, for its payment history; the only one held in memory
    current: Option<Invoice>,
    modal: Option<Modal>,
}

impl<R: Repository> InvoicesView<R> {
    pub fn new(service: InvoiceService<R>) -> Result<Self, ServiceError> {
        let mut view = Self {
            service,
            summaries: Vec::new(),
            visible: Vec::new(),
            filter: Filter::default(),
            selection: Selection::default(),
            current: None,
            modal: None,
        };
        view.reload()?;
        Ok(view)
    }

    fn reload(&mut self) -> Result<(), ServiceError> {
        self.summaries = self.service.list()?;
        self.update_visible();
        self.selection.clamp(self.visible.len());
        self.load_current()
    }

    /// Rows matching the search: number, client, milestone, kind or status
    fn update_visible(&mut self) {
        self.visible = self.filter.positions(&self.summaries, |s| {
            [
                s.number().as_str(),
                s.client_name(),
                s.milestone_name(),
                s.kind().label(),
                s.status().label(),
            ]
        });
    }

    /// Load the invoice under the cursor, dropping the previous one first
    fn load_current(&mut self) -> Result<(), ServiceError> {
        self.current = None;
        if let Some(number) = self.selected().map(|s| s.number().clone()) {
            self.current = Some(self.service.load(&number)?);
        }
        Ok(())
    }

    fn selected(&self) -> Option<&InvoiceSummary> {
        self.visible
            .get(self.selection.index())
            .and_then(|&i| self.summaries.get(i))
    }

    /// In rupees, foreign invoices at their rate, across the invoices listed
    fn outstanding(&self) -> BigDecimal {
        self.visible
            .iter()
            .filter_map(|&i| self.summaries.get(i))
            .map(InvoiceSummary::balance_inr)
            .sum()
    }

    /// The selected invoice, when something is still due on it
    fn unsettled(&self) -> Result<&Invoice, Notice> {
        match &self.current {
            None => Err(Notice::info("No invoice selected")),
            Some(invoice) if invoice.is_settled() => {
                Err(Notice::info("Nothing is due on this invoice"))
            }
            Some(invoice) => Ok(invoice),
        }
    }

    fn open_payment(&mut self) -> Action {
        match self.unsettled() {
            Ok(invoice) => {
                self.modal = Some(Modal::Payment(payment_form(invoice)));
                Action::Handled
            }
            Err(notice) => Action::Notify(notice),
        }
    }

    /// `c`: what to take off the selected invoice, and why
    fn open_credit(&mut self) -> Action {
        match self.unsettled() {
            Ok(invoice) => {
                self.modal = Some(Modal::Credit(credit_form(invoice)));
                Action::Handled
            }
            Err(notice) => Action::Notify(notice),
        }
    }

    /// Read the credit form and show the note it makes for confirmation
    fn preview_credit(&mut self, form: &Form) -> Result<Notice, String> {
        let number = self.current_number()?;
        let draft = CreditDraft {
            date: parse::date_or(form.value(CREDIT_DATE), Local::now().date_naive())?,
            taxable_value: parse::money(form.value(CREDIT_AMOUNT))?,
            reason: form.value(CREDIT_REASON).trim().to_owned(),
        };
        let preview = self
            .service
            .preview_credit(&number, draft.clone())
            .map_err(|e| e.to_string())?;
        self.modal = Some(Modal::ConfirmCredit(
            draft,
            Confirm::new(credit_question(&preview)),
        ));
        Ok(Notice::info("Check the credit note before issuing it"))
    }

    fn issue_credit(&mut self, draft: CreditDraft) -> Action {
        let notice = match self.current_number() {
            Ok(number) => match self.service.issue_credit(&number, draft) {
                Ok(credited) => credited_notice(&credited),
                Err(error) => Notice::error(error.to_string()),
            },
            Err(error) => Notice::error(error),
        };
        if let Err(error) = self.reload() {
            return Action::Notify(Notice::error(error.to_string()));
        }
        Action::Notify(notice)
    }

    fn current_number(&self) -> Result<InvoiceNumber, String> {
        self.current
            .as_ref()
            .map(|i| i.number().clone())
            .ok_or_else(|| "No invoice selected".to_owned())
    }

    /// `O`: the latest credit note against the selected invoice
    fn open_credit_pdf(&self) -> Action {
        let Some(credit) = self.current.as_ref().and_then(|i| i.credits().last()) else {
            return Action::Notify(Notice::info("No credit note against this invoice"));
        };
        Action::Notify(match self.service.open_credit_pdf(&credit.number) {
            Ok(path) => Notice::success(format!("Opened {}", path.display())),
            Err(error) => Notice::error(error.to_string()),
        })
    }

    fn search(&mut self, form: &Form) -> Result<Notice, String> {
        self.filter = Filter::from_form(form);
        self.relist().map_err(|e| e.to_string())?;
        Ok(Notice::info(if self.filter.is_empty() {
            "Search cleared".to_owned()
        } else {
            format!(
                "{} invoice(s) match; Esc clears the search",
                self.visible.len()
            )
        }))
    }

    fn relist(&mut self) -> Result<(), ServiceError> {
        self.update_visible();
        self.selection.reset();
        self.load_current()
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

impl<R: Repository> InvoicesView<R> {
    fn handle_modal(&mut self, modal: Modal, key: Key) -> Action {
        match modal {
            Modal::Payment(form) => {
                let Some(number) = self.selected().map(|s| s.number().clone()) else {
                    return Action::Handled;
                };
                let service = &mut self.service;
                let (form, action) = form.step(key, |form| submit_payment(service, &number, form));
                self.modal = form.map(Modal::Payment);
                if matches!(action, Action::Notify(_))
                    && let Err(error) = self.reload()
                {
                    return Action::Notify(Notice::error(error.to_string()));
                }
                action
            }
            Modal::Credit(form) => {
                let (form, action) = form.step(key, |form| self.preview_credit(form));
                // A good submit opened the confirm in its place
                if let Some(form) = form {
                    self.modal = Some(Modal::Credit(form));
                }
                action
            }
            Modal::ConfirmCredit(draft, confirm) => match Confirm::answer(key) {
                Some(true) => self.issue_credit(draft),
                Some(false) => Action::Handled,
                None => {
                    self.modal = Some(Modal::ConfirmCredit(draft, confirm));
                    Action::Handled
                }
            },
            Modal::Search(form) => {
                let (form, action) = form.step(key, |form| self.search(form));
                self.modal = form.map(Modal::Search);
                action
            }
        }
    }

    fn handle_browse(&mut self, key: Key) -> Action {
        if let Some(nav) = Nav::from_key(key) {
            self.selection.move_by(nav, self.visible.len());
            return match self.load_current() {
                Ok(()) => Action::Handled,
                Err(error) => Action::Notify(Notice::error(error.to_string())),
            };
        }
        match key {
            Key::Char('p') => self.open_payment(),
            Key::Char('c') => self.open_credit(),
            Key::Char('o') => self.open_pdf(),
            Key::Char('O') => self.open_credit_pdf(),
            Key::Char('/') => {
                self.modal = Some(Modal::Search(
                    self.filter.form("number, client, milestone or status"),
                ));
                Action::Handled
            }
            Key::Esc if !self.filter.is_empty() => {
                self.filter = Filter::default();
                match self.relist() {
                    Ok(()) => Action::Notify(Notice::info("Search cleared")),
                    Err(error) => Action::Notify(Notice::error(error.to_string())),
                }
            }
            _ => Action::Ignored,
        }
    }
}

/// Prefilled to cancel the whole invoice while nothing has been paid or credited; otherwise left
/// for the amount, with what is due as a guide
fn credit_form(invoice: &Invoice) -> Form {
    let currency = invoice.currency();
    let untouched = invoice.payments().is_empty() && invoice.credits().is_empty();
    let amount = Field::text(
        "Amount",
        format!(
            "before GST, in {currency}; {} due incl. GST",
            money_in(&invoice.balance(), currency)
        ),
    );
    let amount = if untouched {
        amount.with_value(invoice.amounts().taxable_value.to_string())
    } else {
        amount
    };
    Form::new(
        "Credit note",
        vec![
            Field::text("Date", "YYYY-MM-DD, blank for today"),
            amount,
            Field::text("Reason", "e.g. Discount agreed, billed in error"),
        ],
    )
}

/// "Issue CN/26-27/001 against INV/26-27/004 for ₹11,800.00 (10,000.00 + IGST 1,800.00)? …"
fn credit_question(preview: &CreditPreview) -> String {
    let a = &preview.amounts;
    let tax = if a.total_tax().is_zero() {
        String::new()
    } else {
        format!(" incl. GST {}", preview.currency.format(&a.total_tax()))
    };
    let mut question = format!(
        "Issue {} against {} for {}{tax}? {} will still be due.",
        preview.number,
        preview.invoice,
        money_in(&a.total, preview.currency),
        money_in(&preview.balance_after, preview.currency),
    );
    for warning in &preview.warnings {
        question.push_str(&format!(" Note: {warning}."));
    }
    question
}

fn credited_notice(credited: &Credited) -> Notice {
    let number = credited.note.number();
    match &credited.pdf {
        Ok(Some(path)) => {
            let file = path
                .file_name()
                .map(|f| f.to_string_lossy())
                .unwrap_or_default();
            Notice::success(format!("Issued {number}: saved {file} (O opens it)"))
        }
        Ok(None) => Notice::success(format!("Issued credit note {number}")),
        Err(error) => Notice::error(format!(
            "Issued {number}, but its PDF wasn't saved ({error}); press O to retry"
        )),
    }
}

impl<R: Repository> View for InvoicesView<R> {
    fn title(&self) -> &'static str {
        "Invoices"
    }

    fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let search = if self.filter.is_empty() {
            String::new()
        } else {
            format!(" /{}", self.filter.query())
        };
        let title = format!(
            "Invoices ({}){search} · outstanding {}",
            self.visible.len(),
            money(&self.outstanding())
        );
        let history_height = self.current.as_ref().map_or(0, |i| {
            u16::try_from(i.payments().len() + i.credits().len())
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
        if self.visible.is_empty() {
            let message = if self.summaries.is_empty() {
                "No invoices yet. In Projects, select a pending milestone and press i."
            } else {
                "No invoices match the search. Esc clears it."
            };
            let empty = Paragraph::new(message).style(theme.dim()).block(block);
            frame.render_widget(empty, area);
        } else {
            let rows = self
                .visible
                .iter()
                .filter_map(|&i| self.summaries.get(i))
                .map(|s| {
                    let due = if s.is_settled() {
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
                        Line::from(status_span(s.status(), theme)),
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
            let mut state = self.selection.table_state(self.visible.len());
            frame.render_stateful_widget(table, area, &mut state);
        }
        match &self.modal {
            Some(Modal::Payment(form) | Modal::Credit(form) | Modal::Search(form)) => {
                form.render(frame, theme);
            }
            Some(Modal::ConfirmCredit(_, confirm)) => confirm.render(frame, theme),
            None => {}
        }
    }

    fn handle_key(&mut self, key: Key) -> Action {
        match self.modal.take() {
            Some(modal) => self.handle_modal(modal, key),
            None => self.handle_browse(key),
        }
    }

    fn key_hints(&self) -> &'static [KeyHint] {
        match &self.modal {
            Some(Modal::ConfirmCredit(..)) => Confirm::HINTS,
            Some(_) => Form::HINTS,
            None => HINTS,
        }
    }

    fn on_activate(&mut self) -> Option<Notice> {
        self.reload().err().map(|e| Notice::error(e.to_string()))
    }
}

/// Every payment received and credit note issued against the selected invoice
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
            Line::raw("Payment"),
            Line::raw(money_in(&p.received, currency)).right_aligned(),
            Line::raw(third).right_aligned(),
            Line::raw(money_in(&p.settled(), currency)).right_aligned(),
        ])
    });
    let credits = invoice.credits().iter().map(|c| {
        Row::new(vec![
            Line::raw(c.date.format("%d-%m-%Y").to_string()),
            Line::raw(c.number.as_str()),
            Line::default(),
            Line::default(),
            Line::raw(money_in(&c.amount, currency)).right_aligned(),
        ])
    });
    let titles: &[&str] = if foreign {
        &["Date", "Document", "Received", "Banked", "Settled"]
    } else {
        &["Date", "Document", "Received", "TDS", "Settled"]
    };
    let widths = [
        Constraint::Length(10),
        Constraint::Length(14),
        Constraint::Length(14),
        Constraint::Length(14),
        Constraint::Length(14),
    ];
    let table = Table::new(rows.chain(credits), widths)
        .header(header(titles, &[2, 3, 4], theme))
        .column_spacing(2)
        .block(panel(&title, false, theme));
    frame.render_widget(table, area);
}
