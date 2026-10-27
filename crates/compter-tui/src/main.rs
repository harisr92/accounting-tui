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

use compter_core::demo;
use compter_core::services::{ExpenseService, InvoiceService, PreferencesService, ReportService};
use compter_core::storage::sync::{
    self, Resolution, SyncBackend, SyncDir, SyncFolder, SyncSession,
};
use compter_core::storage::{DataDir, Store};
use compter_tui::app::App;
use compter_tui::cli;
use compter_tui::drive::{self, DriveFolder};
use compter_tui::event::{Event, Events};
use compter_tui::ui::views::{
    DashboardView, ExpensesView, InvoicesView, ProjectsView, ReportsView, SettingsView,
};
use compter_tui::ui::{
    self,
    theme::Theme,
    view::{Notice, View},
};

const TICK_RATE: Duration = Duration::from_millis(250);

const USAGE: &str = "usage: compter [--demo] [-V | --version] [-h | --help]
       compter import-merges <repo> <project> [--dry-run]
       compter sync (--keep-local | --keep-remote)
       compter drive (sign-in | sign-out)

  --demo           try compter on sample projects kept in memory; nothing is saved
  -V, --version    display version information
  -h, --help       display this help text

  import-merges    add a milestone to an hourly project for each merge into the main branch
                   of a git repository, timed from the previous merge (8h at most across days)
  sync             settle a conflict between devices: publish this device's data, or replace it
                   with the synced snapshot (the old file is kept as compter.conflict-*.redb)
  drive            sync through Google Drive: sign in to Google in the browser, or sign out

Data is stored in $COMPTER_HOME, $XDG_DATA_HOME/compter, or else %APPDATA%\\compter on Windows
and ~/.local/share/compter elsewhere.

To use several devices, run `compter drive sign-in` (the phone app syncs the same way), or set a
sync folder in Settings (s) to a folder OneDrive, Dropbox or iCloud syncs; $COMPTER_SYNC_DIR
overrides both. compter publishes a snapshot there on exit and picks up a newer one on start;
quit compter on one device before opening it on another.";

fn main() -> Result<ExitCode> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("import-merges") {
        return import_merges(&args[1..]);
    }
    if args.first().map(String::as_str) == Some("sync") {
        return resolve_sync(&args[1..]);
    }
    if args.first().map(String::as_str) == Some("drive") {
        return google_drive(&args[1..]);
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
    let Opened {
        store,
        data_dir,
        sync,
    } = open_store(demo_mode)?;
    let notice = sync.as_ref().and_then(conflict_notice);
    let result = run_tui(&store, data_dir, notice);
    if let Some(session) = sync {
        finish_sync(session, store)?;
    }
    result.map(|()| ExitCode::SUCCESS)
}

/// Build the tabs and run them; every repository is dropped on return, so the store can close
fn run_tui(store: &Store, data_dir: Option<DataDir>, notice: Option<Notice>) -> Result<()> {
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
    let preferences = || {
        PreferencesService::new(repo.clone(), data_dir.clone())
            .with_sync_override(SyncDir::from_env())
    };
    let settings =
        SettingsView::new(invoices(), preferences()).wrap_err("could not load settings")?;
    let appearance = preferences()
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
    if let Some(notice) = notice {
        app.notify(notice);
    }

    enable_raw_mode()?;
    // Restore even when setup or the loop fails, so the shell isn't left in raw mode
    let result = run_fullscreen(&mut app);
    restore_terminal()?;
    result
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
    let Opened { store, sync, .. } = open_store(false)?;
    let result = cli::import_merges(&store, Path::new(repo.as_str()), project, dry_run);
    if let Some(session) = sync {
        finish_sync(session, store)?;
    }
    result?;
    Ok(ExitCode::SUCCESS)
}

/// `compter sync (--keep-local | --keep-remote)`
fn resolve_sync(args: &[String]) -> Result<ExitCode> {
    let resolution = match args {
        [flag] if flag == "--keep-local" => Resolution::KeepLocal,
        [flag] if flag == "--keep-remote" => Resolution::KeepRemote,
        _ => {
            eprintln!("sync needs --keep-local or --keep-remote\n\n{USAGE}");
            return Ok(ExitCode::from(2));
        }
    };
    let dir = DataDir::from_env()?;
    let Some(folder) = sync_folder(&dir)? else {
        eprintln!(
            "no sync folder is set (Settings, `compter drive sign-in`, or $COMPTER_SYNC_DIR), so there is nothing to sync"
        );
        return Ok(ExitCode::from(2));
    };
    SyncSession::resolve(&dir, &*folder, &sync::device_name_from_env(), resolution)?;
    println!("synced with {}", folder.place());
    Ok(ExitCode::SUCCESS)
}

/// `compter drive (sign-in | sign-out)`
fn google_drive(args: &[String]) -> Result<ExitCode> {
    let dir = DataDir::from_env()?;
    match args {
        [command] if command == "sign-in" => {
            cli::drive_sign_in(&dir, &sync::device_name_from_env())?
        }
        [command] if command == "sign-out" => cli::drive_sign_out(&dir)?,
        _ => {
            eprintln!("drive needs sign-in or sign-out\n\n{USAGE}");
            return Ok(ExitCode::from(2));
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// Where this device syncs through, when it syncs
fn sync_folder(dir: &DataDir) -> Result<Option<Box<dyn SyncFolder>>> {
    Ok(match SyncBackend::configured(dir, SyncDir::from_env())? {
        None => None,
        Some(SyncBackend::Folder(sync_dir, _)) => Some(Box::new(sync_dir)),
        Some(SyncBackend::GoogleDrive { account }) => Some(Box::new(DriveFolder::new(
            drive::Client::from_build()?,
            account,
        ))),
    })
}

/// An open database, with the sync session that must publish it on exit
struct Opened {
    store: Store,
    /// The folder invoice PDFs go in (none in demo mode)
    data_dir: Option<DataDir>,
    sync: Option<SyncSession>,
}

/// The database, brought up to date from the sync folder first when one is set
fn open_store(demo_mode: bool) -> Result<Opened> {
    if demo_mode {
        let store = demo::store().wrap_err("could not build the demo data")?;
        return Ok(Opened {
            store,
            data_dir: None,
            sync: None,
        });
    }
    let dir = DataDir::from_env()?;
    let sync = sync_folder(&dir)?
        .map(|folder| SyncSession::begin(&dir, folder, sync::device_name_from_env()))
        .transpose()?;
    let store = Store::open(&dir)
        .wrap_err_with(|| format!("could not open {}", dir.database_path().display()))?;
    Ok(Opened {
        store,
        data_dir: Some(dir),
        sync,
    })
}

/// Tell the user about conflict copies a sync client left, which compter never reads
fn conflict_notice(session: &SyncSession) -> Option<Notice> {
    let copies = session.conflict_copies();
    let first = copies.first()?;
    Some(Notice::error(format!(
        "sync folder has {} conflict cop{} ({first}); compter ignores them",
        copies.len(),
        if copies.len() == 1 { "y" } else { "ies" },
    )))
}

fn finish_sync(session: SyncSession, store: Store) -> Result<()> {
    if let Some(generation) = session
        .finish(store)
        .wrap_err("could not publish to the sync folder; your data is safe on this device")?
    {
        println!("published snapshot {generation} to the sync folder");
    }
    Ok(())
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
