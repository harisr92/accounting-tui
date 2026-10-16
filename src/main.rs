use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;
use std::time::Duration;

use color_eyre::Result;
use color_eyre::eyre::WrapErr;
use ratatui::Terminal;
use ratatui::backend::TermionBackend;
use termion::raw::IntoRawMode;
use termion::screen::IntoAlternateScreen;

use compter_tui::app::App;
use compter_tui::cli;
use compter_tui::demo;
use compter_tui::event::{Event, Events};
use compter_tui::services::{ExpenseService, InvoiceService, ReportService};
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

Data is stored in $COMPTER_HOME, $XDG_DATA_HOME/compter or ~/.local/share/compter.";

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
    let settings = SettingsView::new(invoices()).wrap_err("could not load settings")?;

    let views: Vec<Box<dyn View>> = vec![
        Box::new(dashboard),
        Box::new(projects),
        Box::new(invoice_list),
        Box::new(expenses),
        Box::new(report_view),
        Box::new(settings),
    ];
    let mut app = App::new(views, Theme::default());

    let stdout = io::stdout().into_raw_mode()?.into_alternate_screen()?;
    let mut terminal = Terminal::new(TermionBackend::new(stdout))?;
    let result = run(&mut terminal, &mut app);
    terminal.show_cursor()?;
    result.map(|()| ExitCode::SUCCESS)
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
