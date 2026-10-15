use crate::app::App;
use crate::demo;
use crate::services::ProjectService;
use crate::ui::tests::support::screen;
use crate::ui::theme::Theme;
use crate::ui::view::View;
use crate::ui::views::{PlaceholderView, ProjectsView};

#[test]
fn renders_shell_at_80x24() {
    let store = demo::store().unwrap();
    let projects = ProjectsView::new(ProjectService::new(store.projects())).unwrap();
    let views: Vec<Box<dyn View>> = vec![
        Box::new(PlaceholderView::new("Dashboard", "")),
        Box::new(projects),
    ];
    let mut app = App::new(views, Theme::default());
    app.handle_key(termion::event::Key::Char('2'));
    let screen = screen(&app, 80, 24);

    for expected in [
        "compter",
        "Dashboard",
        "Projects",
        "Milestones",
        "API Audit",
        "Website Revamp",
        "help",
        "quit",
    ] {
        assert!(screen.contains(expected), "missing {expected:?}\n{screen}");
    }
}
