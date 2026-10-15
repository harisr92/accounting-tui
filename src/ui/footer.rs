use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::theme::Theme;
use super::view::KeyHint;
use crate::app::{App, FOOTER_HINTS};

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

/// View hints on the left (cut off when narrow); on the right a notice, or else `? help · q quit`
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let theme = app.theme();
    let view_hints = app.active_view().key_hints();
    let mut left = Vec::with_capacity(view_hints.len() * 4);
    hint_spans(&mut left, view_hints, theme);

    let right = match app.notice() {
        Some(notice) => Line::styled(notice.text.as_ref(), theme.tone(notice.tone)),
        None => {
            let mut spans = Vec::with_capacity(FOOTER_HINTS.len() * 4);
            hint_spans(&mut spans, FOOTER_HINTS, theme);
            Line::from(spans)
        }
    };
    let right_width = u16::try_from(right.width()).unwrap_or(u16::MAX);
    let [left_area, right_area] =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(right_width)])
            .spacing(2)
            .areas(area);

    frame.render_widget(Paragraph::new(Line::from(left)), left_area);
    frame.render_widget(right, right_area);
}
