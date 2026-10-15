//! Small rendering helpers shared by every view

use accounting_core::utils::formatting::format_inr;
use bigdecimal::BigDecimal;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Block;

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
    format!("₹{}", format_inr(amount))
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
