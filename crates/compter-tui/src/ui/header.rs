use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Tabs;

use crate::app::App;

const TAB_KEYS: [&str; 9] = ["1", "2", "3", "4", "5", "6", "7", "8", "9"];

/// One unboxed line, like the footer: the app name and tabs on the left, the fiscal year on the right
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let theme = app.theme();
    let titles = app.titles().zip(TAB_KEYS).map(|(title, key)| {
        Line::from(vec![
            Span::styled(key, theme.key()),
            Span::raw(" "),
            Span::raw(title),
        ])
    });
    let name = Span::styled("compter", theme.key());
    let year = Line::styled(app.fiscal_year().to_string(), theme.dim());
    let year_width = u16::try_from(year.width()).unwrap_or(u16::MAX);
    let [name_area, tabs_area, year_area] = Layout::horizontal([
        Constraint::Length(7),
        Constraint::Min(0),
        Constraint::Length(year_width),
    ])
    .spacing(1)
    .areas(area);

    let tabs = Tabs::new(titles)
        .select(app.active_index())
        .highlight_style(Style::new().add_modifier(Modifier::BOLD | Modifier::REVERSED))
        .divider(Span::styled("│", theme.dim()));
    frame.render_widget(name, name_area);
    frame.render_widget(tabs, tabs_area);
    frame.render_widget(year, year_area);
}
