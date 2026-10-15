//! Small rendering helpers shared by every view

use accounting_core::utils::formatting::format_inr;
use bigdecimal::BigDecimal;
use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Row};

use super::theme::{Theme, Tone};
use crate::domain::{MilestoneStatus, ProjectStatus};

/// Bordered block whose border lights up when the pane has focus
pub fn panel<'a>(title: &'a str, focused: bool, theme: &Theme) -> Block<'a> {
    let (border, title_style) = if focused {
        (theme.border_focused, theme.key())
    } else {
        (theme.border, Style::new().add_modifier(Modifier::BOLD))
    };
    Block::bordered()
        .border_style(Style::new().fg(border))
        .title(Line::styled(format!(" {title} "), title_style))
}

/// Rupee amount with Indian digit grouping
pub fn money(amount: &BigDecimal) -> String {
    let formatted = format_inr(amount);
    match formatted.strip_prefix('-') {
        Some(magnitude) => format!("-₹{magnitude}"),
        None => format!("₹{formatted}"),
    }
}

/// Logged time as `6h 30m`
pub fn duration(minutes: u32) -> String {
    format!("{}h {:02}m", minutes / 60, minutes % 60)
}

/// A `width` x `height` rect centred in `area`, clamped to fit
pub fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let [row] = Layout::vertical([Constraint::Length(height.min(area.height))])
        .flex(Flex::Center)
        .areas(area);
    let [rect] = Layout::horizontal([Constraint::Length(width.min(area.width))])
        .flex(Flex::Center)
        .areas(row);
    rect
}

/// Clear a centred box, draw a focused panel on it and return the area inside the border
pub fn modal(frame: &mut Frame, title: &str, width: u16, height: u16, theme: &Theme) -> Rect {
    let area = centered_rect(frame.area(), width, height);
    let block = panel(title, true, theme);
    let inner = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    inner
}

/// A table header lined up with its cells: `right` lists the columns whose cells are
/// right-aligned (amounts), so their titles sit over the numbers' right edge
pub fn header<'a>(titles: &[&'a str], right: &[usize], theme: &Theme) -> Row<'a> {
    let cells = titles.iter().enumerate().map(|(index, title)| {
        let line = Line::raw(*title);
        if right.contains(&index) {
            line.right_aligned()
        } else {
            line
        }
    });
    Row::new(cells).style(theme.key())
}

/// A row of equal-width "label over value" figures
pub fn stat_tiles(frame: &mut Frame, area: Rect, stats: &[(&str, String)], theme: &Theme) {
    let cells = Layout::horizontal(vec![Constraint::Fill(1); stats.len()])
        .spacing(1)
        .split(area);
    for ((label, value), cell) in stats.iter().zip(cells.iter()) {
        let stat = Paragraph::new(vec![
            Line::styled(*label, theme.dim()),
            Line::styled(value.as_str(), theme.key()),
        ]);
        frame.render_widget(stat, *cell);
    }
}

/// A status that renders as a coloured badge
pub trait Toned: Copy {
    fn label(self) -> &'static str;
    fn tone(self) -> Tone;
}

impl Toned for ProjectStatus {
    fn label(self) -> &'static str {
        ProjectStatus::label(self)
    }

    fn tone(self) -> Tone {
        match self {
            Self::Active => Tone::Info,
            Self::Completed => Tone::Warning,
            Self::Paid => Tone::Success,
        }
    }
}

impl Toned for MilestoneStatus {
    fn label(self) -> &'static str {
        MilestoneStatus::label(self)
    }

    fn tone(self) -> Tone {
        match self {
            Self::Pending => Tone::Neutral,
            Self::Invoiced => Tone::Warning,
            Self::Paid => Tone::Success,
        }
    }
}

pub fn status_span(status: impl Toned, theme: &Theme) -> Span<'static> {
    Span::styled(status.label(), theme.tone(status.tone()))
}
