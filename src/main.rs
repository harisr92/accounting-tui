use std::io::{self, Write};
use std::process::ExitCode;
use std::time::Duration;

use color_eyre::Result;
use color_eyre::eyre::WrapErr;
use ratatui::Terminal;
use ratatui::backend::TermionBackend;
use termion::raw::IntoRawMode;
use termion::screen::IntoAlternateScreen;

use compter_tui::app::App;
use compter_tui::demo;
use compter_tui::event::{Event, Events};
use compter_tui::services::ProjectService;
use compter_tui::storage::{DataDir, Store};
use compter_tui::ui::{
    self, theme::Theme, view::View, views::PlaceholderView, views::ProjectsView,
};

const TICK_RATE: Duration = Duration::from_millis(250);

const USAGE: &str = "usage: compter [--demo]

  --demo   try compter on sample projects kept in memory; nothing is saved

Data is stored in $COMPTER_HOME, $XDG_DATA_HOME/compter or ~/.local/share/compter.";

fn main() -> Result<ExitCode> {
    let mut demo_mode = false;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--demo" => demo_mode = true,
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(ExitCode::SUCCESS);
            }
            _ => {
                eprintln!("unknown argument {arg:?}\n\n{USAGE}");
                return Ok(ExitCode::from(2));
            }
        }
    }

    install_hooks()?;
    // Open the database before raw mode, so errors like "already open" print normally
    let store = open_store(demo_mode)?;
    let projects = ProjectsView::new(ProjectService::new(store.projects()))
        .wrap_err("could not load projects")?;

    let views: Vec<Box<dyn View>> = vec![
        Box::new(PlaceholderView::new(
            "Dashboard",
            "Earnings overview arrives in step 6",
        )),
        Box::new(projects),
        Box::new(PlaceholderView::new(
            "Invoices",
            "GST invoices arrive in step 5",
        )),
        Box::new(PlaceholderView::new(
            "Reports",
            "Income tax and GSTR-1 reports arrive in step 6",
        )),
    ];
    let mut app = App::new(views, Theme::default());
    // Open on Projects, the only live tab so far
    app.handle_key(termion::event::Key::Char('2'));

    let stdout = io::stdout().into_raw_mode()?.into_alternate_screen()?;
    let mut terminal = Terminal::new(TermionBackend::new(stdout))?;
    let result = run(&mut terminal, &mut app);
    terminal.show_cursor()?;
    result.map(|()| ExitCode::SUCCESS)
}

fn open_store(demo_mode: bool) -> Result<Store> {
    if demo_mode {
        return demo::store().wrap_err("could not build the demo data");
    }
    let dir = DataDir::from_env()?;
    Store::open(&dir).wrap_err_with(|| format!("could not open {}", dir.database_path().display()))
}

fn run<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()>
where
    B::Error: Send + Sync + 'static,
{
    let events = Events::spawn(TICK_RATE);
    let mut size = terminal.size()?;

    while app.is_running() {
        if app.take_dirty() {
            terminal.draw(|frame| ui::draw(frame, app))?;
        }
        match events.next()? {
            Event::Key(key) => app.handle_key(key),
            Event::Tick => {
                app.on_tick();
                let current = terminal.size()?;
                if current != size {
                    size = current;
                    app.mark_dirty();
                }
            }
        }
    }
    Ok(())
}

/// Leave the alternate screen before printing a panic, so the report stays visible
fn install_hooks() -> Result<()> {
    let (panic_hook, eyre_hook) = color_eyre::config::HookBuilder::default().into_hooks();
    eyre_hook.install()?;
    let panic_hook = panic_hook.into_panic_hook();
    std::panic::set_hook(Box::new(move |info| {
        let mut stdout = io::stdout();
        let _ = write!(
            stdout,
            "{}{}\r",
            termion::screen::ToMainScreen,
            termion::cursor::Show
        );
        let _ = stdout.flush();
        panic_hook(info);
    }));
    Ok(())
}
