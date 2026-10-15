use std::io::{self, Write};
use std::time::Duration;

use color_eyre::Result;
use ratatui::Terminal;
use ratatui::backend::TermionBackend;
use termion::raw::IntoRawMode;
use termion::screen::IntoAlternateScreen;

use compter_tui::app::App;
use compter_tui::event::{Event, Events};
use compter_tui::mock;
use compter_tui::ui::{
    self, theme::Theme, view::View, views::PlaceholderView, views::ProjectsView,
};

const TICK_RATE: Duration = Duration::from_millis(250);

fn main() -> Result<()> {
    install_hooks()?;

    let views: Vec<Box<dyn View>> = vec![
        Box::new(PlaceholderView::new(
            "Dashboard",
            "Earnings overview arrives in step 6",
        )),
        Box::new(ProjectsView::new(mock::projects()?)),
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
    // Open on Projects, the only live tab in this step
    app.handle_key(termion::event::Key::Char('2'));

    let stdout = io::stdout().into_raw_mode()?.into_alternate_screen()?;
    let mut terminal = Terminal::new(TermionBackend::new(stdout))?;
    let result = run(&mut terminal, &mut app);
    terminal.show_cursor()?;
    result
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
