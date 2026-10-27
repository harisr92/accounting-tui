use ratatui::style::{Color, Modifier, Style};

use compter_core::domain::Appearance;

/// Semantic colour of a status badge
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Neutral,
    Info,
    Warning,
    Success,
    Error,
    /// Taken back by credit notes: settled, but not by payment
    Credited,
}

/// The single source of colours and styles for the whole UI
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub accent: Color,
    pub muted: Color,
    pub border: Color,
    pub border_focused: Color,
    pub selected: Style,
    /// Secondary text: hints, labels, empty states
    dim: Style,
    info: Color,
    warning: Color,
    success: Color,
    error: Color,
    credited: Color,
}

impl Theme {
    /// The theme for `appearance`; `no_color` (the `NO_COLOR` convention) always gives the
    /// monochrome one
    pub fn new(appearance: Appearance, no_color: bool) -> Self {
        match (appearance, no_color) {
            (_, true) | (Appearance::Mono, _) => Self::mono(),
            (Appearance::Dark, false) => Self::dark(),
            (Appearance::Light, false) => Self::light(),
            (Appearance::GruvboxDark, false) => Self::gruvbox_dark(),
        }
    }

    pub const fn tone(&self, tone: Tone) -> Style {
        let fg = match tone {
            Tone::Neutral => self.muted,
            Tone::Info => self.info,
            Tone::Warning => self.warning,
            Tone::Success => self.success,
            Tone::Error => self.error,
            Tone::Credited => self.credited,
        };
        Style::new().fg(fg).add_modifier(Modifier::BOLD)
    }

    pub const fn key(&self) -> Style {
        Style::new().fg(self.accent).add_modifier(Modifier::BOLD)
    }

    pub const fn dim(&self) -> Style {
        self.dim
    }

    fn dark() -> Self {
        Self {
            accent: Color::Cyan,
            muted: Color::DarkGray,
            border: Color::DarkGray,
            border_focused: Color::Cyan,
            selected: Style::new()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
            dim: Style::new().fg(Color::DarkGray),
            info: Color::Cyan,
            warning: Color::Yellow,
            success: Color::Green,
            error: Color::Red,
            credited: Color::Magenta,
        }
    }

    /// 256-colour shades that stay readable on a white background, where yellow and cyan don't
    fn light() -> Self {
        let gray = Color::Indexed(244);
        Self {
            accent: Color::Blue,
            muted: gray,
            border: Color::Indexed(250),
            border_focused: Color::Blue,
            selected: Style::new()
                .fg(Color::Black)
                .bg(Color::Indexed(253))
                .add_modifier(Modifier::BOLD),
            dim: Style::new().fg(gray),
            info: Color::Blue,
            warning: Color::Indexed(130),
            success: Color::Indexed(28),
            error: Color::Red,
            credited: Color::Indexed(127),
        }
    }

    /// The gruvbox dark palette in true colour; the background stays the terminal's own
    fn gruvbox_dark() -> Self {
        let gray = Color::Rgb(0x92, 0x83, 0x74);
        let orange = Color::Rgb(0xfe, 0x80, 0x19);
        Self {
            accent: orange,
            muted: gray,
            border: Color::Rgb(0x66, 0x5c, 0x54),
            border_focused: orange,
            selected: Style::new()
                .fg(Color::Rgb(0xeb, 0xdb, 0xb2))
                .bg(Color::Rgb(0x50, 0x49, 0x45))
                .add_modifier(Modifier::BOLD),
            dim: Style::new().fg(gray),
            info: Color::Rgb(0x83, 0xa5, 0x98),
            warning: Color::Rgb(0xfa, 0xbd, 0x2f),
            success: Color::Rgb(0xb8, 0xbb, 0x26),
            error: Color::Rgb(0xfb, 0x49, 0x34),
            credited: Color::Rgb(0xd3, 0x86, 0x9b),
        }
    }

    /// The terminal's own colours, with emphasis from bold, dim and reverse only
    fn mono() -> Self {
        Self {
            accent: Color::Reset,
            muted: Color::Reset,
            border: Color::Reset,
            border_focused: Color::Reset,
            selected: Style::new().add_modifier(Modifier::REVERSED),
            dim: Style::new().add_modifier(Modifier::DIM),
            info: Color::Reset,
            warning: Color::Reset,
            success: Color::Reset,
            error: Color::Reset,
            credited: Color::Reset,
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}
