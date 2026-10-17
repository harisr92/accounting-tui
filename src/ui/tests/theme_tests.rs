use ratatui::style::{Color, Modifier};

use crate::domain::Appearance;
use crate::ui::theme::{Theme, Tone};

#[test]
fn no_color_draws_every_theme_without_colour() {
    for appearance in Appearance::ALL {
        let theme = Theme::new(appearance, true);
        assert_eq!(theme.tone(Tone::Error).fg, Some(Color::Reset));
        assert!(theme.selected.add_modifier.contains(Modifier::REVERSED));
        assert!(theme.dim().add_modifier.contains(Modifier::DIM));
    }
}

#[test]
fn light_theme_avoids_yellow_and_cyan() {
    let theme = Theme::new(Appearance::Light, false);
    for tone in [Tone::Info, Tone::Warning, Tone::Success] {
        let fg = theme.tone(tone).fg;
        assert!(!matches!(fg, Some(Color::Yellow | Color::Cyan)), "{tone:?}");
    }
    assert_eq!(theme.key().fg, Some(Color::Blue));
}
