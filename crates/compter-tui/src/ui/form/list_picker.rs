use crate::ui::key::Key;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Row, Table};

use crate::ui::filter::Filter;
use crate::ui::selection::{Nav, Selection};
use crate::ui::theme::Theme;
use crate::ui::view::KeyHint;
use crate::ui::widgets::modal;

const WIDTH: u16 = 72;
/// Rows shown before the list scrolls
const MAX_ROWS: u16 = 12;
const DETAIL_WIDTH: u16 = 28;

/// What a key press did to a list picker
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListEvent {
    /// The row at this position in the items the picker was given
    Picked(usize),
    Cancel,
    Edited,
    Ignored,
}

/// One row of a [`ListPicker`]: what it is, and what tells it apart
#[derive(Debug, Clone)]
pub struct ListItem {
    label: String,
    detail: String,
}

impl ListItem {
    pub fn new(label: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            detail: detail.into(),
        }
    }
}

/// Choose one row from a list, narrowed by typing: every letter goes to the search, so rows
/// move with the arrow keys only
#[derive(Debug, Clone)]
pub struct ListPicker {
    title: &'static str,
    items: Vec<ListItem>,
    query: String,
    /// Positions in `items` of the rows matching `query`; `selection` moves over these
    visible: Vec<usize>,
    selection: Selection,
}

impl ListPicker {
    pub const HINTS: &'static [KeyHint] = &[
        KeyHint::new("Enter", "pick"),
        KeyHint::new("Esc", "cancel"),
        KeyHint::new("↑↓", "move"),
        KeyHint::new("type", "search"),
    ];

    pub fn new(title: &'static str, items: Vec<ListItem>) -> Self {
        let visible = (0..items.len()).collect();
        Self {
            title,
            items,
            query: String::new(),
            visible,
            selection: Selection::default(),
        }
    }

    pub fn handle_key(&mut self, key: Key) -> ListEvent {
        match key {
            Key::Esc => ListEvent::Cancel,
            Key::Enter => self
                .visible
                .get(self.selection.index())
                .map_or(ListEvent::Ignored, |&index| ListEvent::Picked(index)),
            Key::Up | Key::Down => {
                if let Some(nav) = Nav::from_key(key) {
                    self.selection.move_by(nav, self.visible.len());
                }
                ListEvent::Edited
            }
            Key::Char(c) if !c.is_control() => {
                self.query.push(c);
                self.narrow();
                ListEvent::Edited
            }
            Key::Backspace if self.query.pop().is_some() => {
                self.narrow();
                ListEvent::Edited
            }
            Key::Ctrl('u') => {
                self.query.clear();
                self.narrow();
                ListEvent::Edited
            }
            _ => ListEvent::Ignored,
        }
    }

    fn narrow(&mut self) {
        self.visible = Filter::new(&self.query)
            .positions(&self.items, |i| [i.label.as_str(), i.detail.as_str()]);
        self.selection.reset();
    }

    pub fn render(&self, frame: &mut Frame, theme: &Theme) {
        let rows = u16::try_from(self.items.len())
            .unwrap_or(MAX_ROWS)
            .clamp(1, MAX_ROWS);
        // Search line, gap, rows, gap, help line, and the border
        let area = modal(frame, self.title, WIDTH, rows + 6, theme);
        let [search_area, list_area, help_area] = Layout::vertical([
            Constraint::Length(2),
            Constraint::Min(1),
            Constraint::Length(2),
        ])
        .areas(area);

        let search = Line::from(vec![
            Span::styled("  Search  ", theme.key()),
            Span::raw(self.query.as_str()),
            Span::styled("█", theme.key()),
        ]);
        frame.render_widget(Paragraph::new(search), search_area);

        if self.visible.is_empty() {
            frame.render_widget(
                Paragraph::new(Line::styled("   nothing matches", theme.dim())),
                list_area,
            );
        } else {
            let rows = self.visible.iter().filter_map(|&index| {
                let item = self.items.get(index)?;
                Some(Row::new([
                    Span::raw(item.label.as_str()),
                    Span::styled(item.detail.as_str(), theme.dim()),
                ]))
            });
            let table = Table::new(
                rows,
                [Constraint::Fill(1), Constraint::Length(DETAIL_WIDTH)],
            )
            .row_highlight_style(theme.selected)
            .highlight_symbol(" > ");
            let mut state = self.selection.table_state(self.visible.len());
            frame.render_stateful_widget(table, list_area, &mut state);
        }

        frame.render_widget(
            Paragraph::new(vec![
                Line::default(),
                Line::styled("  Enter pick · Esc back · type to search", theme.dim()),
            ]),
            help_area,
        );
    }
}
