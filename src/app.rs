use crate::ui::key::Key;
use chrono::Local;

use crate::domain::FiscalYear;
use crate::ui::theme::Theme;
use crate::ui::view::{Action, KeyHint, Notice, View};

/// Bindings that work on every tab, as listed in the help popup
pub const GLOBAL_HINTS: &[KeyHint] = &[
    KeyHint::new("?", "help"),
    KeyHint::new("q", "quit"),
    KeyHint::new("1-9/←→", "tabs"),
    KeyHint::new("Ctrl-c", "quit, even in a form"),
];

/// The global bindings the footer always keeps visible
pub const FOOTER_HINTS: &[KeyHint] = GLOBAL_HINTS.split_at(2).0;

/// How many ticks a footer notice stays visible
const NOTICE_TICKS: u8 = 12;

/// Application state: registered views, active tab, overlays
pub struct App {
    views: Vec<Box<dyn View>>,
    active: usize,
    show_help: bool,
    notice: Option<(Notice, u8)>,
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

    pub fn notice(&self) -> Option<&Notice> {
        self.notice.as_ref().map(|(notice, _)| notice)
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

        // Always quits, even while a view has a form open
        if key == Key::Ctrl('c') {
            self.running = false;
            return;
        }

        if self.show_help {
            if matches!(key, Key::Esc | Key::Char('?' | 'q')) {
                self.show_help = false;
            }
            return;
        }

        match self.views[self.active].handle_key(key) {
            Action::Handled => return,
            Action::Notify(notice) => {
                self.notice = Some((notice, NOTICE_TICKS));
                return;
            }
            Action::Ignored => {}
        }

        let count = self.views.len();
        match key {
            Key::Char('q') => self.running = false,
            Key::Char('?') => self.show_help = true,
            Key::Right | Key::Char('l') => self.activate((self.active + 1) % count),
            Key::Left | Key::Char('h') => self.activate((self.active + count - 1) % count),
            Key::Char(c @ '1'..='9') => {
                let index = c as usize - '1' as usize;
                if index < count {
                    self.activate(index);
                }
            }
            _ => self.dirty = false,
        }
    }

    /// Switch tabs and let the new tab refresh itself
    fn activate(&mut self, index: usize) {
        if index == self.active {
            return;
        }
        self.active = index;
        if let Some(notice) = self.views[index].on_activate() {
            self.notice = Some((notice, NOTICE_TICKS));
        }
    }
}
