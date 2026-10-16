//! Helpers for driving the UI in tests: build an app, press keys, read the screen

use crate::ui::key::Key;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use crate::app::App;
use crate::services::{ExpenseService, InvoiceService, ReportService};
use crate::storage::Store;
use crate::ui::draw;
use crate::ui::theme::Theme;
use crate::ui::view::View;
use crate::ui::views::{
    DashboardView, ExpensesView, InvoicesView, ProjectsView, ReportsView, SettingsView,
};

/// An app whose only tab is Projects on `store`
pub fn projects_app(store: &Store) -> App {
    let view = ProjectsView::new(store.repository(), None).unwrap();
    let views: Vec<Box<dyn View>> = vec![Box::new(view)];
    App::new(views, Theme::default())
}

/// Projects (tab 1), Invoices (tab 2) and Settings (tab 3) on `store`, writing no PDFs
pub fn invoicing_app(store: &Store) -> App {
    let repo = store.repository();
    let views: Vec<Box<dyn View>> = vec![
        Box::new(ProjectsView::new(repo.clone(), None).unwrap()),
        Box::new(InvoicesView::new(InvoiceService::new(repo.clone(), None)).unwrap()),
        Box::new(SettingsView::new(InvoiceService::new(repo, None)).unwrap()),
    ];
    App::new(views, Theme::default())
}

/// Dashboard (1), Expenses (2) and Reports (3) on `store`, writing no files
pub fn reporting_app(store: &Store) -> App {
    let repo = store.repository();
    let views: Vec<Box<dyn View>> = vec![
        Box::new(
            DashboardView::new(
                ReportService::new(repo.clone(), None),
                InvoiceService::new(repo.clone(), None),
            )
            .unwrap(),
        ),
        Box::new(ExpensesView::new(ExpenseService::new(repo.clone())).unwrap()),
        Box::new(ReportsView::new(ReportService::new(repo, None)).unwrap()),
    ];
    App::new(views, Theme::default())
}

/// Press `[` or `]` until a tab with a year picker shows `start_year`'s financial year
pub fn go_to_year(app: &mut App, start_year: i32) {
    let current = crate::domain::FiscalYear::containing(chrono::Local::now().date_naive());
    let steps = start_year - current.start_year();
    let key = if steps < 0 { '[' } else { ']' };
    for _ in 0..steps.abs() {
        app.handle_key(Key::Char(key));
    }
}

/// Press each character as a key (`\t` is Tab, `\n` is Enter)
pub fn type_keys(app: &mut App, text: &str) {
    for c in text.chars() {
        let key = match c {
            '\t' => Key::Tab,
            '\n' => Key::Enter,
            c => Key::Char(c),
        };
        app.handle_key(key);
    }
}

/// Everything on screen at `width` x `height`, one line per row
pub fn screen(app: &App, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| draw(frame, app)).unwrap();
    let buffer = terminal.backend().buffer();
    buffer
        .content()
        .chunks(usize::from(width))
        .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}
