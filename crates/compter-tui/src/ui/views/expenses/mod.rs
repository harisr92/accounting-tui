//! Expenses tab: business spending for the year, posted to the ledger

use crate::ui::key::Key;
use bigdecimal::{BigDecimal, Zero};
use chrono::Local;
use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Row, Table};

use super::year::{self, YearPicker};
use crate::ui::filter::Filter;
use crate::ui::form::{Confirm, Field, Form, parse};
use crate::ui::selection::{Nav, Selection};
use crate::ui::theme::Theme;
use crate::ui::view::{Action, KeyHint, Notice, View};
use crate::ui::widgets::{header, money, panel};
use compter_core::domain::{Expense, ExpenseCategory, ExpenseId};
use compter_core::services::{ExpenseDraft, ExpenseService, ServiceError};
use compter_core::storage::Repository;

const HINTS: &[KeyHint] = &[
    KeyHint::new("j/k", "move"),
    KeyHint::new("a", "add"),
    KeyHint::new("e", "edit"),
    KeyHint::new("x", "delete"),
    year::HINT,
    Filter::HINT,
    Filter::CLEAR_HINT,
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
    /// Adding a new expense (`None`) or editing this one
    Form(Option<ExpenseId>, Form),
    Delete(ExpenseId, Confirm),
    Search(Form),
}

pub struct ExpensesView<R> {
    service: ExpenseService<R>,
    year: YearPicker,
    expenses: Vec<Expense>,
    /// Positions in `expenses` of the rows matching the search; `selection` moves over these
    visible: Vec<usize>,
    filter: Filter,
    selection: Selection,
    modal: Option<Modal>,
}

impl<R: Repository> ExpensesView<R> {
    pub fn new(service: ExpenseService<R>) -> Result<Self, ServiceError> {
        let mut view = Self {
            service,
            year: YearPicker::current(),
            expenses: Vec::new(),
            visible: Vec::new(),
            filter: Filter::default(),
            selection: Selection::default(),
            modal: None,
        };
        view.reload()?;
        Ok(view)
    }

    fn reload(&mut self) -> Result<(), ServiceError> {
        self.expenses = self.service.list(self.year.year())?;
        self.update_visible();
        self.selection.clamp(self.visible.len());
        Ok(())
    }

    /// Rows matching the search: description or category
    fn update_visible(&mut self) {
        self.visible = self
            .filter
            .positions(&self.expenses, |e| [e.description(), e.category().label()]);
    }

    fn selected(&self) -> Option<&Expense> {
        self.visible
            .get(self.selection.index())
            .and_then(|&i| self.expenses.get(i))
    }

    fn listed(&self) -> impl Iterator<Item = &Expense> {
        self.visible.iter().filter_map(|&i| self.expenses.get(i))
    }

    fn search(&mut self, form: &Form) -> Result<Notice, String> {
        self.filter = Filter::from_form(form);
        self.update_visible();
        self.selection.reset();
        Ok(Notice::info(if self.filter.is_empty() {
            "Search cleared".to_owned()
        } else {
            format!(
                "{} expense(s) match; Esc clears the search",
                self.visible.len()
            )
        }))
    }

    fn submit(&mut self, id: Option<ExpenseId>, form: &Form) -> Result<Notice, String> {
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
        let draft = ExpenseDraft {
            date,
            category,
            description: form.value(DESCRIPTION).to_owned(),
            amount,
            gst_credit: credit,
        };
        let (expense, done) = match id {
            Some(id) => (self.service.edit(id, draft), "Saved"),
            None => (self.service.add(draft), "Added"),
        };
        let expense = expense.map_err(|e| e.to_string())?;
        self.reload().map_err(|e| e.to_string())?;
        // Keep the cursor on the expense, which may have moved (a new date) or left the year
        if let Some(index) = self
            .visible
            .iter()
            .position(|&i| self.expenses.get(i).is_some_and(|e| e.id() == expense.id()))
        {
            self.selection.select(index);
        }
        Ok(Notice::success(format!(
            "{done} {} for {}",
            expense.description(),
            money(expense.amount())
        )))
    }
}

/// The expense form, empty to add one or filled with `existing` to edit it
fn expense_form(existing: Option<&Expense>) -> Form {
    let title = if existing.is_some() {
        "Edit expense"
    } else {
        "Add expense"
    };
    let value = |text: Option<String>| text.unwrap_or_default();
    let category = existing
        .and_then(|e| ExpenseCategory::ALL.iter().position(|&c| c == e.category()))
        .unwrap_or_default();
    Form::new(
        title,
        vec![
            Field::text("Date", "YYYY-MM-DD, blank for today").with_value(value(
                existing.map(|e| e.date().format("%Y-%m-%d").to_string()),
            )),
            Field::choice("Category", CATEGORY_OPTIONS).with_choice(category),
            Field::text("Description", "e.g. Figma, annual plan")
                .with_value(value(existing.map(|e| e.description().to_owned()))),
            Field::text("Amount", "total paid, GST included")
                .with_value(value(existing.map(|e| e.amount().to_string()))),
            Field::text(
                "GST credit",
                "GST in the amount you claim back; blank if none",
            )
            .with_value(value(
                existing
                    .filter(|e| !e.gst_credit().is_zero())
                    .map(|e| e.gst_credit().to_string()),
            )),
        ],
    )
}

impl<R: Repository> View for ExpensesView<R> {
    fn title(&self) -> &'static str {
        "Expenses"
    }

    fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let total: BigDecimal = self.listed().map(Expense::cost).sum();
        let search = if self.filter.is_empty() {
            String::new()
        } else {
            format!(" /{}", self.filter.query())
        };
        let title = format!(
            "Expenses {}{search} · {} after GST credit",
            self.year.year().label(),
            money(&total)
        );
        let block = panel(&title, true, theme);
        if self.visible.is_empty() {
            let message = if self.expenses.is_empty() {
                "No expenses this year. Press a to add one; [ and ] change the year."
            } else {
                "No expenses match the search. Esc clears it."
            };
            let empty = Paragraph::new(message).style(theme.dim()).block(block);
            frame.render_widget(empty, area);
        } else {
            let rows = self.listed().map(|e| {
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
            let mut state = self.selection.table_state(self.visible.len());
            frame.render_stateful_widget(table, area, &mut state);
        }
        match &self.modal {
            Some(Modal::Form(_, form) | Modal::Search(form)) => form.render(frame, theme),
            Some(Modal::Delete(_, confirm)) => confirm.render(frame, theme),
            None => {}
        }
    }

    fn handle_key(&mut self, key: Key) -> Action {
        match self.modal.take() {
            Some(Modal::Form(id, form)) => {
                let (form, action) = form.step(key, |form| self.submit(id, form));
                self.modal = form.map(|form| Modal::Form(id, form));
                return action;
            }
            Some(Modal::Search(form)) => {
                let (form, action) = form.step(key, |form| self.search(form));
                self.modal = form.map(Modal::Search);
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
            self.selection.move_by(nav, self.visible.len());
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
                self.modal = Some(Modal::Form(None, expense_form(None)));
                Action::Handled
            }
            Key::Char('e') => match self.selected() {
                Some(e) => {
                    self.modal = Some(Modal::Form(Some(e.id()), expense_form(Some(e))));
                    Action::Handled
                }
                None => Action::Notify(Notice::info("No expense selected")),
            },
            Key::Char('/') => {
                self.modal = Some(Modal::Search(self.filter.form("description or category")));
                Action::Handled
            }
            Key::Esc if !self.filter.is_empty() => {
                self.filter = Filter::default();
                self.update_visible();
                self.selection.reset();
                Action::Notify(Notice::info("Search cleared"))
            }
            Key::Char('x') => match self.selected() {
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
            Some(Modal::Form(..) | Modal::Search(_)) => Form::HINTS,
            Some(Modal::Delete(..)) => Confirm::HINTS,
            None => HINTS,
        }
    }

    fn on_activate(&mut self) -> Option<Notice> {
        self.reload().err().map(|e| Notice::error(e.to_string()))
    }
}
