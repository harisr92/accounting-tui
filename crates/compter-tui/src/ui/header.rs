use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Tabs};

use crate::app::App;

const TAB_KEYS: [&str; 9] = ["1", "2", "3", "4", "5", "6", "7", "8", "9"];

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let theme = app.theme();
    let titles = app.titles().zip(TAB_KEYS).map(|(title, key)| {
        Line::from(vec![
            Span::styled(key, theme.key()),
            Span::raw(" "),
            Span::raw(title),
        ])
    });
    let block = Block::bordered()
        .border_style(Style::new().fg(theme.border))
        .title(Line::styled(" compter ", theme.key()))
        .title(Line::styled(format!(" {} ", app.fiscal_year()), theme.dim()).right_aligned());
    let tabs = Tabs::new(titles)
        .select(app.active_index())
        .highlight_style(Style::new().add_modifier(Modifier::BOLD | Modifier::REVERSED))
        .divider(Span::styled("│", theme.dim()))
        .block(block);
    frame.render_widget(tabs, area);
}
