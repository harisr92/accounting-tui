//! Expenses tab: business spending for the year, posted to the ledger

use bigdecimal::{BigDecimal, Zero};
use chrono::Local;
use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Row, Table};
use termion::event::Key;

use super::year::{self, YearPicker};
use crate::domain::{Expense, ExpenseCategory, ExpenseId};
use crate::services::{ExpenseService, ServiceError};
use crate::storage::Repository;
use crate::ui::form::{Confirm, Field, Form, parse};
use crate::ui::selection::{Nav, Selection};
use crate::ui::theme::Theme;
use crate::ui::view::{Action, KeyHint, Notice, View};
use crate::ui::widgets::{header, money, panel};

const HINTS: &[KeyHint] = &[
    KeyHint::new("j/k", "move"),
    KeyHint::new("a", "add"),
    KeyHint::new("x", "delete"),
    year::HINT,
];

const CATEGORY_OPTIONS: &[&str] = &[
    ExpenseCategory::ALL[0].label(),
    ExpenseCategory::ALL[1].label(),
    ExpenseCategory::ALL[2].label(),
    ExpenseCategory::ALL[3].label(),
    ExpenseCategory::ALL[4].label(),
    ExpenseCategory::ALL[5].label(),
    ExpenseCategory::ALL[6].label(),
];

const DATE: usize = 0;
const CATEGORY: usize = 1;
const DESCRIPTION: usize = 2;
const AMOUNT: usize = 3;
const CREDIT: usize = 4;

enum Modal {
    Add(Form),
    Delete(ExpenseId, Confirm),
}

pub struct ExpensesView<R> {
    service: ExpenseService<R>,
    year: YearPicker,
    expenses: Vec<Expense>,
    selection: Selection,
    modal: Option<Modal>,
}

impl<R: Repository> ExpensesView<R> {
    pub fn new(service: ExpenseService<R>) -> Result<Self, ServiceError> {
        let mut view = Self {
            service,
            year: YearPicker::current(),
            expenses: Vec::new(),
            selection: Selection::default(),
            modal: None,
        };
        view.reload()?;
        Ok(view)
    }

    fn reload(&mut self) -> Result<(), ServiceError> {
        self.expenses = self.service.list(self.year.year())?;
        self.selection.clamp(self.expenses.len());
        Ok(())
    }

    fn submit(&mut self, form: &Form) -> Result<Notice, String> {
        let today = Local::now().date_naive();
        let date = parse::date_or(form.value(DATE), today)?;
        let category = ExpenseCategory::ALL
            .get(form.choice(CATEGORY))
            .copied()
            .unwrap_or(ExpenseCategory::Other);
        let amount = parse::money(form.value(AMOUNT))?;
        let credit = match parse::optional(form.value(CREDIT)) {
            Some(text) => parse::money(&text)?,
            None => BigDecimal::zero(),
        };
        let expense = self
            .service
            .add(date, category, form.value(DESCRIPTION), amount, credit)
            .map_err(|e| e.to_string())?;
        self.reload().map_err(|e| e.to_string())?;
        Ok(Notice::success(format!(
            "Added {} for {}",
            expense.description(),
            money(expense.amount())
        )))
    }
}

fn add_form() -> Form {
    Form::new(
        "Add expense",
        vec![
            Field::text("Date", "YYYY-MM-DD, blank for today"),
            Field::choice("Category", CATEGORY_OPTIONS),
            Field::text("Description", "e.g. Figma, annual plan"),
            Field::text("Amount", "total paid, GST included"),
            Field::text(
                "GST credit",
                "GST in the amount you claim back; blank if none",
            ),
        ],
    )
}

impl<R: Repository> View for ExpensesView<R> {
    fn title(&self) -> &'static str {
        "Expenses"
    }

    fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let total: BigDecimal = self.expenses.iter().map(Expense::cost).sum();
        let title = format!(
            "Expenses {} · {} after GST credit",
            self.year.year().label(),
            money(&total)
        );
        let block = panel(&title, true, theme);
        if self.expenses.is_empty() {
            let empty = Paragraph::new(
                "No expenses this year. Press a to add one; [ and ] change the year.",
            )
            .style(theme.dim())
            .block(block);
            frame.render_widget(empty, area);
        } else {
            let rows = self.expenses.iter().map(|e| {
                Row::new(vec![
                    Line::raw(e.date().format("%d-%m-%Y").to_string()),
                    Line::raw(e.category().label()),
                    Line::raw(e.description()),
                    Line::raw(money(e.amount())).right_aligned(),
                    Line::raw(money(e.gst_credit())).right_aligned(),
                    Line::raw(money(&e.cost())).right_aligned(),
                ])
            });
            let widths = [
                Constraint::Length(10),
                Constraint::Length(24),
                Constraint::Fill(1),
                Constraint::Length(14),
                Constraint::Length(12),
                Constraint::Length(14),
            ];
            let table = Table::new(rows, widths)
                .header(header(
                    &[
                        "Date",
                        "Category",
                        "Description",
                        "Paid",
                        "GST credit",
                        "Cost",
                    ],
                    &[3, 4, 5],
                    theme,
                ))
                .column_spacing(1)
                .block(block)
                .row_highlight_style(theme.selected)
                .highlight_symbol("> ");
            let mut state = self.selection.table_state(self.expenses.len());
            frame.render_stateful_widget(table, area, &mut state);
        }
        match &self.modal {
            Some(Modal::Add(form)) => form.render(frame, theme),
            Some(Modal::Delete(_, confirm)) => confirm.render(frame, theme),
            None => {}
        }
    }

    fn handle_key(&mut self, key: Key) -> Action {
        match self.modal.take() {
            Some(Modal::Add(form)) => {
                let (form, action) = form.step(key, |form| self.submit(form));
                self.modal = form.map(Modal::Add);
                return action;
            }
            Some(Modal::Delete(id, confirm)) => {
                return match Confirm::answer(key) {
                    Some(true) => {
                        let result = self.service.delete(id).and_then(|()| self.reload());
                        Action::Notify(match result {
                            Ok(()) => Notice::success("Expense deleted"),
                            Err(error) => Notice::error(error.to_string()),
                        })
                    }
                    Some(false) => Action::Handled,
                    None => {
                        self.modal = Some(Modal::Delete(id, confirm));
                        Action::Handled
                    }
                };
            }
            None => {}
        }
        if let Some(nav) = Nav::from_key(key) {
            self.selection.move_by(nav, self.expenses.len());
            return Action::Handled;
        }
        if self.year.handle(key) {
            self.selection.reset();
            return match self.reload() {
                Ok(()) => Action::Handled,
                Err(error) => Action::Notify(Notice::error(error.to_string())),
            };
        }
        match key {
            Key::Char('a') => {
                self.modal = Some(Modal::Add(add_form()));
                Action::Handled
            }
            Key::Char('x') => match self.expenses.get(self.selection.index()) {
                Some(e) => {
                    let message = format!(
                        "Delete {} ({}) and its ledger entry?",
                        e.description(),
                        money(e.amount())
                    );
                    self.modal = Some(Modal::Delete(e.id(), Confirm::new(message)));
                    Action::Handled
                }
                None => Action::Notify(Notice::info("No expense selected")),
            },
            _ => Action::Ignored,
        }
    }

    fn key_hints(&self) -> &'static [KeyHint] {
        match &self.modal {
            Some(Modal::Add(_)) => Form::HINTS,
            Some(Modal::Delete(..)) => Confirm::HINTS,
            None => HINTS,
        }
    }

    fn on_activate(&mut self) -> Option<Notice> {
        self.reload().err().map(|e| Notice::error(e.to_string()))
    }
}
