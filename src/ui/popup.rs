use ratatui::Frame;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};

use super::view::KeyHint;
use super::widgets::{centered_rect, panel};
use crate::app::{App, GLOBAL_HINTS};

const KEY_COLUMN: usize = 10;

pub fn render_help(frame: &mut Frame, app: &App) {
    let theme = app.theme();
    let view = app.active_view();
    let section = |title: &'static str, hints: &'static [KeyHint]| {
        std::iter::once(Line::styled(title, theme.dim())).chain(hints.iter().map(|h| {
            Line::from(vec![
                Span::styled(format!("  {:<KEY_COLUMN$}", h.key), theme.key()),
                Span::raw(h.desc),
            ])
        }))
    };
    let lines: Vec<Line> = section(view.title(), view.key_hints())
        .chain(std::iter::once(Line::default()))
        .chain(section("Global", GLOBAL_HINTS))
        .collect();

    let height = u16::try_from(lines.len())
        .unwrap_or(u16::MAX)
        .saturating_add(2);
    let area = centered_rect(frame.area(), 44, height);
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(lines).block(panel("Help", true, theme)),
        area,
    );
}
