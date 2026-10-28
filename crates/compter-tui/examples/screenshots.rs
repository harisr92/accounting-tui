//! The README's screenshots: compter on the demo business, drawn into ratatui's `TestBackend`
//! and written as SVG, one text run per stretch of cells styled alike. Run it after a UI
//! change: `cargo run -p compter-tui --example screenshots`.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use color_eyre::Result;
use compter_core::demo;
use compter_core::domain::Appearance;
use compter_core::services::{ExpenseService, InvoiceService, PreferencesService, ReportService};
use compter_core::storage::Store;
use compter_tui::app::App;
use compter_tui::ui::draw;
use compter_tui::ui::key::Key;
use compter_tui::ui::theme::Theme;
use compter_tui::ui::view::View;
use compter_tui::ui::views::{
    DashboardView, ExpensesView, InvoicesView, ProjectsView, ReportsView, SettingsView,
};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::{Buffer, Cell};
use ratatui::style::{Color, Modifier};

const WIDTH: u16 = 120;
const HEIGHT: u16 = 36;
/// Next to this crate's manifest, wherever it is run from
const OUT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/screenshots");

/// Each shot: file name, theme, and the keys pressed from the start (`\t` is Tab)
const SHOTS: &[(&str, Appearance, &str)] = &[
    ("dashboard", Appearance::Dark, ""),
    ("projects", Appearance::Dark, "2jjj"),
    ("issue", Appearance::Dark, "2jjj\tjji\n"),
    ("invoices", Appearance::GruvboxDark, "3"),
    ("reports", Appearance::Light, "5"),
];

fn main() -> Result<()> {
    color_eyre::install()?;
    let out = Path::new(OUT);
    fs::create_dir_all(out)?;
    for &(name, appearance, keys) in SHOTS {
        let store = demo::store()?;
        let mut app = app(&store, appearance)?;
        for key in keys.chars() {
            app.handle_key(match key {
                '\t' => Key::Tab,
                '\n' => Key::Enter,
                c => Key::Char(c),
            });
        }
        let mut terminal = Terminal::new(TestBackend::new(WIDTH, HEIGHT))?;
        terminal.draw(|frame| draw(frame, &app))?;
        let path = out.join(format!("{name}.svg"));
        fs::write(
            &path,
            svg(terminal.backend().buffer(), Palette::of(appearance)),
        )?;
        println!("wrote {}", path.display());
    }
    Ok(())
}

/// The tabs `compter --demo` shows, in the same order, writing no files
fn app(store: &Store, appearance: Appearance) -> Result<App> {
    let repo = store.repository();
    let invoices = || InvoiceService::new(repo.clone(), None);
    let reports = || ReportService::new(repo.clone(), None);
    let views: Vec<Box<dyn View>> = vec![
        Box::new(DashboardView::new(reports(), invoices())?),
        Box::new(ProjectsView::new(repo.clone(), None)?),
        Box::new(InvoicesView::new(invoices())?),
        Box::new(ExpensesView::new(ExpenseService::new(repo.clone()))?),
        Box::new(ReportsView::new(reports())?),
        Box::new(SettingsView::new(
            invoices(),
            PreferencesService::new(repo.clone(), None),
        )?),
    ];
    Ok(App::new(views, Theme::new(appearance, false)))
}

/// What a terminal shows for the colours a theme names: its default text and background,
/// and its 16 ANSI colours
struct Palette {
    fg: &'static str,
    bg: &'static str,
    ansi: [&'static str; 16],
}

impl Palette {
    fn of(appearance: Appearance) -> Self {
        // One Dark's ANSI colours for a dark terminal
        const DARK: [&str; 16] = [
            "#3f4451", "#e06c75", "#98c379", "#e5c07b", "#61afef", "#c678dd", "#56b6c2", "#abb2bf",
            "#5c6370", "#e9969d", "#b5e890", "#f0d197", "#8dc5f4", "#d6a4e8", "#8ad0d8", "#ffffff",
        ];
        match appearance {
            Appearance::Light => Self {
                fg: "#24292f",
                bg: "#ffffff",
                ansi: [
                    "#24292f", "#cf222e", "#116329", "#4d2d00", "#0969da", "#8250df", "#1b7c83",
                    "#6e7781", "#57606a", "#a40e26", "#1a7f37", "#633c01", "#218bff", "#a475f9",
                    "#3192aa", "#8c959f",
                ],
            },
            Appearance::GruvboxDark => Self {
                fg: "#ebdbb2",
                bg: "#282828",
                ansi: DARK,
            },
            Appearance::Dark | Appearance::Mono => Self {
                fg: "#abb2bf",
                bg: "#1e2127",
                ansi: DARK,
            },
        }
    }

    /// `color` as CSS, or `None` for the terminal's own
    fn css(&self, color: Color) -> Option<String> {
        let named = |i: usize| Some(self.ansi[i].to_owned());
        match color {
            Color::Reset => None,
            Color::Black => named(0),
            Color::Red => named(1),
            Color::Green => named(2),
            Color::Yellow => named(3),
            Color::Blue => named(4),
            Color::Magenta => named(5),
            Color::Cyan => named(6),
            Color::Gray => named(7),
            Color::DarkGray => named(8),
            Color::LightRed => named(9),
            Color::LightGreen => named(10),
            Color::LightYellow => named(11),
            Color::LightBlue => named(12),
            Color::LightMagenta => named(13),
            Color::LightCyan => named(14),
            Color::White => named(15),
            Color::Rgb(r, g, b) => Some(format!("#{r:02x}{g:02x}{b:02x}")),
            Color::Indexed(i) => Some(indexed(i, &self.ansi)),
        }
    }
}

/// xterm's 256 colours: the 16 ANSI ones, a 6×6×6 cube, then 24 grays
fn indexed(i: u8, ansi: &[&str; 16]) -> String {
    match i {
        0..=15 => ansi[usize::from(i)].to_owned(),
        16..=231 => {
            let level = |n: u8| if n == 0 { 0 } else { 55 + 40 * n };
            let n = i - 16;
            format!(
                "#{:02x}{:02x}{:02x}",
                level(n / 36),
                level(n / 6 % 6),
                level(n % 6)
            )
        }
        232..=255 => {
            let v = 8 + 10 * (i - 232);
            format!("#{v:02x}{v:02x}{v:02x}")
        }
    }
}

/// One cell's look, which a run of cells shares
#[derive(PartialEq)]
struct Look {
    fg: String,
    bg: Option<String>,
    bold: bool,
    dim: bool,
    italic: bool,
    underline: bool,
}

impl Look {
    fn of(cell: &Cell, palette: &Palette) -> Self {
        let mods = cell.modifier;
        let fg = palette.css(cell.fg);
        let bg = palette.css(cell.bg);
        let (fg, bg) = if mods.contains(Modifier::REVERSED) {
            (
                bg.unwrap_or_else(|| palette.bg.to_owned()),
                Some(fg.unwrap_or_else(|| palette.fg.to_owned())),
            )
        } else {
            (fg.unwrap_or_else(|| palette.fg.to_owned()), bg)
        };
        Self {
            fg,
            bg,
            bold: mods.contains(Modifier::BOLD),
            dim: mods.contains(Modifier::DIM),
            italic: mods.contains(Modifier::ITALIC),
            underline: mods.contains(Modifier::UNDERLINED),
        }
    }
}

const CELL_W: f32 = 10.0;
const CELL_H: f32 = 20.0;
const PAD: f32 = 16.0;
const BAR: f32 = 32.0;

/// [buffer] as an SVG terminal window: a title bar with its three buttons, then each row as
/// background rectangles and text runs, each run stretched to its cells so columns line up
/// whatever the font
fn svg(buffer: &Buffer, palette: Palette) -> String {
    let width = buffer.area.width;
    let w = f32::from(width) * CELL_W + 2.0 * PAD;
    let h = f32::from(buffer.area.height) * CELL_H + 2.0 * PAD + BAR;
    let mut out = String::with_capacity(256 * 1024);
    let _ = write!(
        out,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">
<style>text{{font-family:"SF Mono",Menlo,Consolas,"DejaVu Sans Mono","Liberation Mono",monospace;font-size:17px;white-space:pre}}</style>
<rect width="{w}" height="{h}" rx="10" fill="{bg}"/>
<circle cx="20" cy="16" r="6" fill="#ff5f57"/><circle cx="40" cy="16" r="6" fill="#febc2e"/><circle cx="60" cy="16" r="6" fill="#28c840"/>
"##,
        bg = palette.bg,
    );
    let cells = buffer.content();
    for (row, line) in cells.chunks(usize::from(width)).enumerate() {
        let y = BAR + PAD + row as f32 * CELL_H;
        let mut col = 0;
        while col < line.len() {
            let look = Look::of(&line[col], &palette);
            let start = col;
            let mut text = String::new();
            while col < line.len() && Look::of(&line[col], &palette) == look {
                text.push_str(line[col].symbol());
                col += 1;
            }
            let x = PAD + start as f32 * CELL_W;
            let span = (col - start) as f32 * CELL_W;
            if let Some(bg) = &look.bg {
                let _ = writeln!(
                    out,
                    r#"<rect x="{x}" y="{y}" width="{span}" height="{CELL_H}" fill="{bg}"/>"#
                );
            }
            if text.trim().is_empty() {
                continue;
            }
            let mut attrs = format!(r#" fill="{}""#, look.fg);
            if look.bold {
                attrs.push_str(r#" font-weight="bold""#);
            }
            if look.dim {
                attrs.push_str(r#" opacity="0.6""#);
            }
            if look.italic {
                attrs.push_str(r#" font-style="italic""#);
            }
            if look.underline {
                attrs.push_str(r#" text-decoration="underline""#);
            }
            let _ = writeln!(
                out,
                r#"<text x="{x}" y="{baseline}" textLength="{span}" lengthAdjust="spacingAndGlyphs"{attrs}>{text}</text>"#,
                baseline = y + CELL_H * 0.75,
                text = escape(&text),
            );
        }
    }
    out.push_str("</svg>\n");
    out
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
