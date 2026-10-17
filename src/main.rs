use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;
use std::time::Duration;

use color_eyre::Result;
use color_eyre::eyre::WrapErr;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::crossterm::{cursor, execute};

use compter_tui::app::App;
use compter_tui::cli;
use compter_tui::demo;
use compter_tui::event::{Event, Events};
use compter_tui::services::{ExpenseService, InvoiceService, PreferencesService, ReportService};
use compter_tui::storage::{DataDir, Store};
use compter_tui::ui::views::{
    DashboardView, ExpensesView, InvoicesView, ProjectsView, ReportsView, SettingsView,
};
use compter_tui::ui::{self, theme::Theme, view::View};

const TICK_RATE: Duration = Duration::from_millis(250);

const USAGE: &str = "usage: compter [--demo] [-V | --version] [-h | --help]
       compter import-merges <repo> <project> [--dry-run]

  --demo           try compter on sample projects kept in memory; nothing is saved
  -V, --version    display version information
  -h, --help       display this help text

  import-merges    add a milestone to an hourly project for each merge into the main branch
                   of a git repository, timed from the previous merge (8h at most across days)

Data is stored in $COMPTER_HOME, $XDG_DATA_HOME/compter, or else %APPDATA%\\compter on Windows
and ~/.local/share/compter elsewhere.";

fn main() -> Result<ExitCode> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("import-merges") {
        return import_merges(&args[1..]);
    }
    let mut demo_mode = false;
    for arg in &args {
        match arg.as_str() {
            "--demo" => demo_mode = true,
            "-V" | "--version" => {
                println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
                return Ok(ExitCode::SUCCESS);
            }
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
    let (store, data_dir) = open_store(demo_mode)?;
    let repo = store.repository();
    let invoices = || InvoiceService::new(repo.clone(), data_dir.clone());
    let projects =
        ProjectsView::new(repo.clone(), data_dir.clone()).wrap_err("could not load projects")?;
    let reports = || ReportService::new(repo.clone(), data_dir.clone());
    let dashboard =
        DashboardView::new(reports(), invoices()).wrap_err("could not load the dashboard")?;
    let invoice_list = InvoicesView::new(invoices()).wrap_err("could not load invoices")?;
    let expenses =
        ExpensesView::new(ExpenseService::new(repo.clone())).wrap_err("could not load expenses")?;
    let report_view = ReportsView::new(reports()).wrap_err("could not load reports")?;
    let settings = SettingsView::new(invoices(), PreferencesService::new(repo.clone()))
        .wrap_err("could not load settings")?;
    let appearance = PreferencesService::new(repo.clone())
        .appearance()
        .wrap_err("could not load the theme")?;
    // https://no-color.org: set and not empty
    let no_color = std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty());

    let views: Vec<Box<dyn View>> = vec![
        Box::new(dashboard),
        Box::new(projects),
        Box::new(invoice_list),
        Box::new(expenses),
        Box::new(report_view),
        Box::new(settings),
    ];
    let mut app = App::with_colour(views, Theme::new(appearance, no_color), no_color);

    enable_raw_mode()?;
    // Restore even when setup or the loop fails, so the shell isn't left in raw mode
    let result = run_fullscreen(&mut app);
    restore_terminal()?;
    result.map(|()| ExitCode::SUCCESS)
}

fn run_fullscreen(app: &mut App) -> Result<()> {
    execute!(io::stdout(), EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    run(&mut terminal, app)
}

/// `compter import-merges <repo> <project> [--dry-run]`
fn import_merges(args: &[String]) -> Result<ExitCode> {
    let dry_run = args.iter().any(|arg| arg == "--dry-run");
    let positional: Vec<&String> = args.iter().filter(|arg| *arg != "--dry-run").collect();
    let [repo, project] = positional.as_slice() else {
        eprintln!("import-merges needs a repository and a project name\n\n{USAGE}");
        return Ok(ExitCode::from(2));
    };
    let (store, _) = open_store(false)?;
    cli::import_merges(&store, Path::new(repo.as_str()), project, dry_run)?;
    Ok(ExitCode::SUCCESS)
}

/// The database, and the folder invoice PDFs go in (none in demo mode)
fn open_store(demo_mode: bool) -> Result<(Store, Option<DataDir>)> {
    if demo_mode {
        let store = demo::store().wrap_err("could not build the demo data")?;
        return Ok((store, None));
    }
    let dir = DataDir::from_env()?;
    let store = Store::open(&dir)
        .wrap_err_with(|| format!("could not open {}", dir.database_path().display()))?;
    Ok((store, Some(dir)))
}

fn run<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()>
where
    B::Error: Send + Sync + 'static,
{
    let events = Events::spawn(TICK_RATE);

    while app.is_running() {
        if app.take_dirty() {
            terminal.draw(|frame| ui::draw(frame, app))?;
        }
        match events.next()? {
            Event::Key(key) => app.handle_key(key),
            Event::Resize => app.mark_dirty(),
            Event::Tick => app.on_tick(),
        }
    }
    Ok(())
}

/// Restore the terminal before printing a panic, so the report stays visible
fn install_hooks() -> Result<()> {
    let (panic_hook, eyre_hook) = color_eyre::config::HookBuilder::default().into_hooks();
    eyre_hook.install()?;
    let panic_hook = panic_hook.into_panic_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = restore_terminal();
        panic_hook(info);
    }));
    Ok(())
}

/// Leave raw mode and the alternate screen; safe to call when neither was entered
fn restore_terminal() -> io::Result<()> {
    disable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, LeaveAlternateScreen, cursor::Show)?;
    stdout.flush()
}
