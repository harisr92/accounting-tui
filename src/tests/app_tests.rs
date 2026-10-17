use crate::ui::key::Key;

use crate::app::App;
use crate::ui::theme::Theme;
use crate::ui::view::{Notice, View};
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

#[test]
fn startup_notice_shows_until_it_times_out() {
    let mut app = app();
    app.take_dirty();
    app.notify(Notice::error("sync folder has 1 conflict copy"));
    assert!(app.take_dirty());
    assert_eq!(
        app.notice(),
        Some(&Notice::error("sync folder has 1 conflict copy"))
    );
    for _ in 0..20 {
        app.on_tick();
    }
    assert_eq!(app.notice(), None);
}
