use crate::app::App;
use crate::ui::tests::support::screen;
use crate::ui::theme::Theme;
use crate::ui::view::View;
use crate::ui::views::{PlaceholderView, ProjectsView};
use compter_core::demo;

#[test]
fn renders_shell_at_80x24() {
    let store = demo::store().unwrap();
    let projects = ProjectsView::new(store.repository(), None).unwrap();
    let views: Vec<Box<dyn View>> = vec![
        Box::new(PlaceholderView::new("Dashboard", "")),
        Box::new(projects),
    ];
    let mut app = App::new(views, Theme::default());
    app.handle_key(crate::ui::key::Key::Char('2'));
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

#[test]
fn the_milestone_picker_fits_at_80x24() {
    use crate::ui::key::Key;
    use crate::ui::tests::support::{projects_app, type_keys};
    use bigdecimal::BigDecimal;
    use compter_core::domain::{Billing, Project};
    use compter_core::storage::{ProjectRepository, Store};

    let store = Store::in_memory().unwrap();
    let mut project = Project::new("Site", "Acme", None, Billing::Fixed).unwrap();
    let due = chrono::NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
    for name in ["Design", "Build", "Launch"] {
        project
            .add_milestone(name, due, Some(BigDecimal::from(1_000)))
            .unwrap();
    }
    store.repository().save(&project).unwrap();
    let mut app = projects_app(&store);
    type_keys(&mut app, "\ti");
    app.handle_key(Key::Char('a'));
    let screen = screen(&app, 80, 24);
    for expected in [
        "Milestones to invoice",
        "[x] Design",
        "[x] Launch",
        "3 picked · Enter next · Esc cancel · Space pick · a all/none",
    ] {
        assert!(screen.contains(expected), "missing {expected:?}\n{screen}");
    }
}
