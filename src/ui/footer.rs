use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::theme::{Theme, Tone};
use super::view::KeyHint;
use crate::app::{App, GLOBAL_HINTS};

/// Append `key desc` pairs, separated by a dim dot
pub fn hint_spans<'a>(spans: &mut Vec<Span<'a>>, hints: &'a [KeyHint], theme: &Theme) {
    for hint in hints {
        if !spans.is_empty() {
            spans.push(Span::styled(" · ", theme.dim()));
        }
        spans.push(Span::styled(hint.key, theme.key()));
        spans.push(Span::raw(" "));
        spans.push(Span::raw(hint.desc));
    }
}

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let theme = app.theme();
    let view_hints = app.active_view().key_hints();
    let mut spans = Vec::with_capacity((view_hints.len() + GLOBAL_HINTS.len()) * 4);
    hint_spans(&mut spans, view_hints, theme);
    hint_spans(&mut spans, GLOBAL_HINTS, theme);

    let notice = app.notice().unwrap_or_default();
    let notice_width = u16::try_from(notice.chars().count()).unwrap_or(u16::MAX);
    let [hints_area, notice_area] =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(notice_width)])
            .spacing(1)
            .areas(area);

    frame.render_widget(Paragraph::new(Line::from(spans)), hints_area);
    frame.render_widget(Line::styled(notice, theme.tone(Tone::Warning)), notice_area);
}
