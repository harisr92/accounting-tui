use serde::{Deserialize, Serialize};

/// Which colour theme the app is drawn in; a preference, kept with the settings
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Appearance {
    /// Light text on a dark terminal
    #[default]
    Dark,
    /// Dark text on a light terminal
    Light,
    /// The warm, low-contrast gruvbox palette, for a dark terminal with true colour
    GruvboxDark,
    /// No colours, only bold, dim and reverse: for monochrome terminals and `NO_COLOR`
    Mono,
}

impl Appearance {
    pub const ALL: [Self; 4] = [Self::Dark, Self::Light, Self::GruvboxDark, Self::Mono];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
            Self::GruvboxDark => "gruvbox dark",
            Self::Mono => "monochrome",
        }
    }

    /// The theme after this one, wrapping around
    pub fn next(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|&a| a == self)
            .unwrap_or_default();
        Self::ALL[(index + 1) % Self::ALL.len()]
    }
}
