use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::widgets::Paragraph;

use crate::ui::theme::Theme;
use crate::ui::view::View;
use crate::ui::widgets::panel;

/// A tab whose content arrives in a later step
pub struct PlaceholderView {
    title: &'static str,
    message: &'static str,
}

impl PlaceholderView {
    pub const fn new(title: &'static str, message: &'static str) -> Self {
        Self { title, message }
    }
}

impl View for PlaceholderView {
    fn title(&self) -> &'static str {
        self.title
    }

    fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let body = Paragraph::new(self.message)
            .style(theme.dim())
            .alignment(Alignment::Center)
            .block(panel(self.title, false, theme));
        frame.render_widget(body, area);
    }
}
