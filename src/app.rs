use chrono::Local;
use termion::event::Key;

use crate::domain::FiscalYear;
use crate::ui::theme::Theme;
use crate::ui::view::{Action, KeyHint, View};

pub const GLOBAL_HINTS: &[KeyHint] = &[
    KeyHint::new("1-9/←→", "tabs"),
    KeyHint::new("?", "help"),
    KeyHint::new("q", "quit"),
];

/// How many ticks a footer notice stays visible
const NOTICE_TICKS: u8 = 12;

/// Application state: registered views, active tab, overlays
pub struct App {
    views: Vec<Box<dyn View>>,
    active: usize,
    show_help: bool,
    notice: Option<(&'static str, u8)>,
    fiscal_year: FiscalYear,
    theme: Theme,
    running: bool,
    dirty: bool,
}

impl App {
    pub fn new(views: Vec<Box<dyn View>>, theme: Theme) -> Self {
        Self {
            views,
            active: 0,
            show_help: false,
            notice: None,
            fiscal_year: FiscalYear::containing(Local::now().date_naive()),
            theme,
            running: true,
            dirty: true,
        }
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn active_index(&self) -> usize {
        self.active
    }

    pub fn active_view(&self) -> &dyn View {
        self.views[self.active].as_ref()
    }

    pub fn titles(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.views.iter().map(|v| v.title())
    }

    pub fn show_help(&self) -> bool {
        self.show_help
    }

    pub fn notice(&self) -> Option<&'static str> {
        self.notice.map(|(text, _)| text)
    }

    pub fn fiscal_year(&self) -> FiscalYear {
        self.fiscal_year
    }

    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// Whether a redraw is due; clears the flag
    pub fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn on_tick(&mut self) {
        if let Some((_, ticks)) = &mut self.notice {
            *ticks = ticks.saturating_sub(1);
            if *ticks == 0 {
                self.notice = None;
                self.dirty = true;
            }
        }
    }

    pub fn handle_key(&mut self, key: Key) {
        self.dirty = true;

        if self.show_help {
            if matches!(key, Key::Esc | Key::Char('?' | 'q')) {
                self.show_help = false;
            }
            return;
        }

        match self.views[self.active].handle_key(key) {
            Action::Handled => return,
            Action::Notify(text) => {
                self.notice = Some((text, NOTICE_TICKS));
                return;
            }
            Action::Ignored => {}
        }

        let count = self.views.len();
        match key {
            Key::Char('q') | Key::Ctrl('c') => self.running = false,
            Key::Char('?') => self.show_help = true,
            Key::Right | Key::Char('l') => self.active = (self.active + 1) % count,
            Key::Left | Key::Char('h') => self.active = (self.active + count - 1) % count,
            Key::Char(c @ '1'..='9') => {
                let index = c as usize - '1' as usize;
                if index < count {
                    self.active = index;
                }
            }
            _ => self.dirty = false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::views::PlaceholderView;

    fn app() -> App {
        let views: Vec<Box<dyn View>> = vec![
            Box::new(PlaceholderView::new("A", "")),
            Box::new(PlaceholderView::new("B", "")),
            Box::new(PlaceholderView::new("C", "")),
        ];
        App::new(views, Theme::default())
    }

    #[test]
    fn switches_tabs_by_number_and_arrows() {
        let mut app = app();
        app.handle_key(Key::Char('3'));
        assert_eq!(app.active_index(), 2);
        app.handle_key(Key::Right);
        assert_eq!(app.active_index(), 0);
        app.handle_key(Key::Left);
        assert_eq!(app.active_index(), 2);
        app.handle_key(Key::Char('9'));
        assert_eq!(app.active_index(), 2);
    }

    #[test]
    fn help_swallows_keys_until_closed() {
        let mut app = app();
        app.handle_key(Key::Char('?'));
        assert!(app.show_help());
        app.handle_key(Key::Char('2'));
        assert_eq!(app.active_index(), 0);
        app.handle_key(Key::Esc);
        assert!(!app.show_help());
    }

    #[test]
    fn quits() {
        let mut app = app();
        app.handle_key(Key::Char('q'));
        assert!(!app.is_running());
    }
}
