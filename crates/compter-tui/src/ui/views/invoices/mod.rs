//! Invoices tab: every issued invoice, newest first; open the PDF, record a payment or issue a
//! credit note

use std::path::PathBuf;

use crate::ui::key::Key;
use bigdecimal::{BigDecimal, Zero};
use chrono::Local;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Row, Table};

use crate::ui::filter::Filter;
use crate::ui::form::{Checklist, ChecklistItem, Confirm, Field, Form, parse};
use crate::ui::selection::{Nav, Selection};
use crate::ui::theme::Theme;
use crate::ui::view::{Action, KeyHint, Notice, View};
use crate::ui::views::payment::{payment_form, submit_payment};
use crate::ui::widgets::{header, money, money_in, panel, status_span};
use crate::viewer;
use compter_core::domain::{Invoice, InvoiceNumber, InvoiceSummary, ReturnKind, ReturnTerms};
use compter_core::services::{
    CreditDraft, CreditLineDraft, CreditPreview, CreditableLine, Credited, InvoiceService,
    ServiceError,
};
use compter_core::storage::Repository;

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
const CREDIT_REASON: usize = 1;
const CREDIT_RETURN: usize = 2;
/// Last, and only on some invoices: the rupees refunded on a foreign invoice, or the TDS the
/// client reverses on a rupee invoice that had TDS withheld
const CREDIT_EXTRA: usize = 3;

/// Options of the credit form's `CREDIT_RETURN` choice, in `ReturnKind` order
const RETURN_OPTIONS: &[&str] = &["Refund", "Keep as advance"];

/// What is open over the list; it takes every key
enum Modal {
    /// Payment for the selected invoice
    Payment(Form),
    /// The lines of the selected invoice a credit note takes back from, and how much of each;
    /// a row each in `lines` order
    CreditLines {
        lines: Vec<CreditableLine>,
        list: Checklist,
    },
    /// When and why the credit note takes back `lines`; Esc goes back to `picker`, as it was
    Credit {
        lines: Vec<CreditLineDraft>,
        form: Form,
        picker: Box<(Vec<CreditableLine>, Checklist)>,
    },
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
    /// Rupees credit notes hold as an advance for a client, kept while the cursor stays on that
    /// client's invoices
    advance: Option<(String, BigDecimal)>,
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
            advance: None,
            modal: None,
        };
        view.reload()?;
        Ok(view)
    }

    fn reload(&mut self) -> Result<(), ServiceError> {
        self.summaries = self.service.list()?;
        self.advance = None;
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
                s.milestones(),
                s.kind().label(),
                s.status().label(),
            ]
        });
    }

    /// Load the invoice under the cursor, dropping the previous one first
    fn load_current(&mut self) -> Result<(), ServiceError> {
        self.current = None;
        if let Some(number) = self.selected().map(|s| s.number().clone()) {
            let invoice = self.service.load(&number)?;
            let client = invoice.client_name();
            if self
                .advance
                .as_ref()
                .is_none_or(|(held_for, _)| held_for != client)
            {
                let held = self.service.client_advance(client)?;
                self.advance = Some((client.to_owned(), held));
            }
            self.current = Some(invoice);
        }
        Ok(())
    }

    /// Rupees held as an advance for the selected invoice's client
    fn client_advance(&self) -> Option<&BigDecimal> {
        let client = self.current.as_ref()?.client_name();
        self.advance
            .as_ref()
            .filter(|(held_for, held)| held_for == client && !held.is_zero())
            .map(|(_, held)| held)
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

    /// `c`: which lines to take off the selected invoice and how much of each, then when and
    /// why. Paid invoices can be credited too, until credit notes have taken off their whole
    /// total.
    fn open_credit(&mut self) -> Action {
        let invoice = match &self.current {
            None => return Action::Notify(Notice::info("No invoice selected")),
            Some(invoice) if invoice.creditable().is_zero() => {
                return Action::Notify(Notice::info(
                    "Credit notes already take off this whole invoice",
                ));
            }
            Some(invoice) => invoice,
        };
        let currency = invoice.currency();
        let lines = match self.service.creditable_lines(invoice) {
            Ok(lines) => lines,
            Err(error) => return Action::Notify(Notice::error(error.to_string())),
        };
        // Cancelling an unpaid invoice is the usual case, so it is prefilled; once anything was
        // paid or credited, the amount is left to type, so nothing is refunded by default
        let untouched = invoice.untouched();
        let single = lines.len() == 1;
        let items = lines
            .iter()
            .map(|line| {
                let item = ChecklistItem::new(line.name.as_str(), money_in(&line.billed, currency));
                if line.creditable.is_zero() {
                    item.disabled()
                } else if untouched {
                    item.with_amount(line.creditable.to_string())
                        .checked(single)
                } else {
                    // Picking the line fills in all that is left, to lower if need be
                    item.with_amount("").with_fill(line.creditable.to_string())
                }
            })
            .collect();
        let list = Checklist::new(
            "Credit note: lines to take back",
            ["Milestone", "Billed", "Credit excl. GST"],
            items,
        )
        .with_note(invoice.credit_guide());
        self.modal = Some(Modal::CreditLines { lines, list });
        Action::Handled
    }

    /// Read the lines picked, then ask when and why
    fn pick_credit_lines(
        &mut self,
        creditable: &[CreditableLine],
        list: &Checklist,
    ) -> Result<Notice, String> {
        let mut lines = Vec::with_capacity(creditable.len());
        for row in list.checked() {
            let Some(line) = creditable.get(row) else {
                continue;
            };
            let amount = parse::money(list.amount(row))
                .map_err(|error| format!("{}: {error}", line.name))?;
            lines.push(CreditLineDraft {
                milestone_id: line.milestone_id,
                taxable_value: amount,
            });
        }
        if lines.is_empty() {
            return Err("Pick at least one line (Space, or a for all)".to_owned());
        }
        let invoice = self.selected_invoice()?;
        // Refused here, while the amounts can still be changed
        self.service
            .check_credit_lines(invoice, &lines)
            .map_err(|e| e.to_string())?;
        let form = credit_form(invoice);
        self.modal = Some(Modal::Credit {
            lines,
            form,
            picker: Box::new((creditable.to_vec(), list.clone())),
        });
        Ok(Notice::info("When and why the credit note is issued"))
    }

    /// The selected invoice in full
    fn selected_invoice(&self) -> Result<&Invoice, String> {
        self.current
            .as_ref()
            .ok_or_else(|| "No invoice selected".to_owned())
    }

    /// Read the credit form and show the note it makes for confirmation
    fn preview_credit(&mut self, lines: &[CreditLineDraft], form: &Form) -> Result<Notice, String> {
        let number = self.current_number()?;
        let foreign = self
            .current
            .as_ref()
            .is_some_and(|i| !i.currency().is_inr());
        let draft = CreditDraft {
            date: parse::date_or(form.value(CREDIT_DATE), Local::now().date_naive())?,
            lines: lines.to_vec(),
            reason: form.value(CREDIT_REASON).trim().to_owned(),
            returned: return_terms(form, foreign)?,
        };
        let preview = self
            .service
            .preview_credit(&number, draft.clone())
            .map_err(|e| e.to_string())?;
        let client = self.current.as_ref().map_or("", Invoice::client_name);
        self.modal = Some(Modal::ConfirmCredit(
            draft,
            Confirm::new(credit_question(&preview, client, lines.len())),
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
        Action::Notify(open_in_viewer(
            self.service.ensure_credit_pdf(&credit.number),
        ))
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
        Action::Notify(open_in_viewer(self.service.ensure_pdf(summary.number())))
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
            Modal::CreditLines { lines, list } => {
                let (list, action) = list.step(key, |list| self.pick_credit_lines(&lines, list));
                // A good submit opened the credit form in its place
                if let Some(list) = list {
                    self.modal = Some(Modal::CreditLines { lines, list });
                }
                action
            }
            Modal::Credit {
                lines,
                form,
                picker,
            } => {
                let (form, action) = form.step(key, |form| self.preview_credit(&lines, form));
                match (form, &action) {
                    (Some(form), _) => {
                        self.modal = Some(Modal::Credit {
                            lines,
                            form,
                            picker,
                        });
                    }
                    // Esc: back to the lines, with what was picked and typed
                    (None, Action::Handled) => {
                        let (lines, list) = *picker;
                        self.modal = Some(Modal::CreditLines { lines, list });
                    }
                    // A good submit opened the confirm in its place
                    (None, _) => {}
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

/// When and why, after the lines were picked. A note beyond what is due gives the rest back to
/// the client: refunded (in rupees, on a foreign invoice) or kept as an advance.
fn credit_form(invoice: &Invoice) -> Form {
    let tds_withheld = invoice.payments().iter().any(|p| !p.tds.is_zero());
    let currency = invoice.currency();
    let mut fields = vec![
        Field::text("Date", "YYYY-MM-DD, blank for today"),
        Field::text("Reason", "e.g. Discount agreed, billed in error"),
        Field::choice("If already paid", RETURN_OPTIONS),
    ];
    if !currency.is_inr() {
        fields.push(Field::text(
            "Rupees refunded",
            "from your bank, for a refund of what was paid",
        ));
    } else if tds_withheld {
        fields.push(Field::text(
            "TDS reversed",
            "TDS the client cancels in its return; blank keeps it as your credit",
        ));
    }
    Form::new("Credit note", fields)
}

/// What the credit form says to do with money already paid; its last field is the rupees
/// refunded on a `foreign` invoice and the TDS reversed on a rupee one
fn return_terms(form: &Form, foreign: bool) -> Result<ReturnTerms, String> {
    let extra = parse::optional(form.value(CREDIT_EXTRA))
        .map(|amount| parse::money(&amount))
        .transpose()?;
    let (refunded_inr, tds_reversed) = if foreign {
        (extra, None)
    } else {
        (None, extra)
    };
    Ok(ReturnTerms {
        kind: match form.choice(CREDIT_RETURN) {
            0 => ReturnKind::Refunded,
            _ => ReturnKind::Advance,
        },
        refunded_inr,
        tds_reversed,
    })
}

/// "Issue CN/26-27/001 against INV/26-27/004 for ₹11,800.00 (10,000.00 + IGST 1,800.00)? …",
/// naming how many of its lines it takes back from when more than one
fn credit_question(preview: &CreditPreview, client: &str, lines: usize) -> String {
    let a = &preview.amounts;
    let tax = if a.total_tax().is_zero() {
        String::new()
    } else {
        format!(" incl. GST {}", preview.currency.format(&a.total_tax()))
    };
    let from = if lines > 1 {
        format!(" ({lines} lines)")
    } else {
        String::new()
    };
    let mut question = format!(
        "Issue {} against {}{from} for {}{tax}? {} will still be due.",
        preview.number,
        preview.invoice,
        money_in(&a.total, preview.currency),
        money_in(&preview.balance_after, preview.currency),
    );
    if let Some(returned) = &preview.returned {
        let amount = money_in(&returned.amount, preview.currency);
        let split = returned
            .tds_reversed
            .as_ref()
            .map_or_else(String::new, |tds| {
                let cash = money_in(&returned.cash(), preview.currency);
                match returned.kind {
                    ReturnKind::Refunded => {
                        format!(" ({cash} from your bank, {} TDS reversed)", money(tds))
                    }
                    ReturnKind::Advance => format!(" ({cash} of it, {} TDS reversed)", money(tds)),
                }
            });
        question.push_str(&match returned.kind {
            ReturnKind::Refunded => format!(" {amount} already paid will be refunded{split}."),
            ReturnKind::Advance => {
                format!(" {amount} already paid will be kept as an advance for {client}{split}.")
            }
        });
    }
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
            let returns = i.credits().iter().filter(|c| c.returned.is_some()).count();
            u16::try_from(i.payments().len() + i.credits().len() + returns)
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
            render_payments(frame, history_area, invoice, self.client_advance(), theme);
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
                        Line::raw(s.milestones()),
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
                        "Milestones",
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
            Some(Modal::Payment(form) | Modal::Credit { form, .. } | Modal::Search(form)) => {
                form.render(frame, theme);
            }
            Some(Modal::CreditLines { list, .. }) => list.render(frame, theme),
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
            Some(Modal::CreditLines { list, .. }) => list.hints(),
            Some(_) => Form::HINTS,
            None => HINTS,
        }
    }

    fn on_activate(&mut self) -> Option<Notice> {
        self.reload().err().map(|e| Notice::error(e.to_string()))
    }
}

/// Every payment received and credit note issued against the selected invoice
fn render_payments(
    frame: &mut Frame,
    area: Rect,
    invoice: &Invoice,
    advance: Option<&BigDecimal>,
    theme: &Theme,
) {
    let currency = invoice.currency();
    let held = advance.map_or_else(String::new, |held| {
        format!(
            " · advance held for {} {}",
            invoice.client_name(),
            money(held)
        )
    });
    let discount = invoice.discount();
    let discount = if discount.is_zero() {
        String::new()
    } else {
        format!(" · {} discount", money_in(&discount, currency))
    };
    let title = format!(
        "Payments for {} · {} due{discount}{held}",
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
    // A credit that gave back money already paid is followed by a row for it: negative, as it
    // left the client's payments
    let credits = invoice.credits().iter().flat_map(|c| {
        let date = c.date.format("%d-%m-%Y").to_string();
        let note = Row::new(vec![
            Line::raw(date.clone()),
            Line::raw(c.number.as_str()),
            Line::default(),
            Line::default(),
            Line::raw(money_in(&c.amount, currency)).right_aligned(),
        ]);
        let returned = c.returned.as_ref().map(|r| {
            // The TDS (or, abroad, Banked) column: TDS reversed, or the rupees refunded
            let third = r
                .refunded_inr
                .as_ref()
                .or(r.tds_reversed.as_ref())
                .map_or_else(Line::default, |inr| {
                    Line::raw(money(&-inr.clone())).right_aligned()
                });
            Row::new(vec![
                Line::raw(date),
                Line::raw(r.kind.label()),
                Line::raw(money_in(&-r.cash(), currency)).right_aligned(),
                third,
                Line::default(),
            ])
        });
        std::iter::once(note).chain(returned)
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

/// Open a PDF the service just made sure is written
fn open_in_viewer(path: Result<PathBuf, ServiceError>) -> Notice {
    let opened = path.map_err(|e| e.to_string()).and_then(|path| {
        viewer::open(&path)
            .map(|()| path)
            .map_err(|e| e.to_string())
    });
    match opened {
        Ok(path) => Notice::success(format!("Opened {}", path.display())),
        Err(error) => Notice::error(error),
    }
}
