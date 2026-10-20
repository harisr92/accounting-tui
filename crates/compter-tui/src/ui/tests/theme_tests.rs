use ratatui::style::{Color, Modifier};

use crate::ui::theme::{Theme, Tone};
use compter_core::domain::Appearance;

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
    for tone in [Tone::Info, Tone::Warning, Tone::Success, Tone::Credited] {
        let fg = theme.tone(tone).fg;
        assert!(!matches!(fg, Some(Color::Yellow | Color::Cyan)), "{tone:?}");
    }
    assert_eq!(theme.key().fg, Some(Color::Blue));
}

#[test]
fn credited_has_a_colour_of_its_own() {
    for appearance in [Appearance::Dark, Appearance::Light] {
        let theme = Theme::new(appearance, false);
        let credited = theme.tone(Tone::Credited).fg;
        for other in [
            Tone::Neutral,
            Tone::Info,
            Tone::Warning,
            Tone::Success,
            Tone::Error,
        ] {
            assert_ne!(credited, theme.tone(other).fg, "{appearance:?} {other:?}");
        }
    }
}
