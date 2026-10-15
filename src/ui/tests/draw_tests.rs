use ratatui::Terminal;
use ratatui::backend::TestBackend;

use crate::app::App;
use crate::mock;
use crate::ui::draw;
use crate::ui::theme::Theme;
use crate::ui::view::View;
use crate::ui::views::{PlaceholderView, ProjectsView};

#[test]
fn renders_shell_at_80x24() {
    let views: Vec<Box<dyn View>> = vec![
        Box::new(PlaceholderView::new("Dashboard", "")),
        Box::new(ProjectsView::new(mock::projects().unwrap())),
    ];
    let mut app = App::new(views, Theme::default());
    app.handle_key(termion::event::Key::Char('2'));

    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal.draw(|frame| draw(frame, &app)).unwrap();
    let screen: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();

    for expected in [
        "compter",
        "Dashboard",
        "Projects",
        "Milestones",
        "Website Revamp",
        "help",
        "quit",
    ] {
        assert!(screen.contains(expected), "missing {expected:?}");
    }
}
