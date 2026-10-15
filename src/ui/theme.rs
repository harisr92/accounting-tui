use ratatui::style::{Color, Modifier, Style};

/// Semantic colour of a status badge
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Neutral,
    Info,
    Warning,
    Success,
}

/// The single source of colours and styles for the whole UI
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub accent: Color,
    pub muted: Color,
    pub border: Color,
    pub border_focused: Color,
    pub selected: Style,
}

impl Theme {
    pub const fn tone(&self, tone: Tone) -> Style {
        let fg = match tone {
            Tone::Neutral => self.muted,
            Tone::Info => Color::Cyan,
            Tone::Warning => Color::Yellow,
            Tone::Success => Color::Green,
        };
        Style::new().fg(fg).add_modifier(Modifier::BOLD)
    }

    pub const fn key(&self) -> Style {
        Style::new().fg(self.accent).add_modifier(Modifier::BOLD)
    }

    pub const fn dim(&self) -> Style {
        Style::new().fg(self.muted)
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            accent: Color::Cyan,
            muted: Color::DarkGray,
            border: Color::DarkGray,
            border_focused: Color::Cyan,
            selected: Style::new()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        }
    }
}
