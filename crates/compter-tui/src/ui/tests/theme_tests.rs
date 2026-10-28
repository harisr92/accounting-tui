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
    for appearance in Appearance::ALL
        .into_iter()
        .filter(|&a| a != Appearance::Mono)
    {
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

#[test]
fn a_selected_row_keeps_muted_text_readable() {
    use crate::app::App;
    use crate::ui::draw;
    use crate::ui::tests::support::type_keys;
    use crate::ui::view::View;
    use crate::ui::views::ProjectsView;
    use compter_core::demo;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    for appearance in Appearance::ALL {
        let store = demo::store().unwrap();
        let views: Vec<Box<dyn View>> = vec![Box::new(
            ProjectsView::new(store.repository(), None).unwrap(),
        )];
        let mut app = App::new(views, Theme::new(appearance, false));
        // Website Revamp's milestones, on Launch: PENDING, in the muted colour
        type_keys(&mut app, "jjj\tjj");
        let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
        terminal.draw(|frame| draw(frame, &app)).unwrap();
        let buffer = terminal.backend().buffer();

        let row = buffer
            .content()
            .chunks(120)
            .find(|row| {
                let text: String = row.iter().map(|cell| cell.symbol()).collect();
                text.contains("> Launch")
            })
            .expect("Launch is selected");
        // By cell, not byte: ₹ and the borders are several bytes each
        let word = "PENDING".len();
        let start = (0..=row.len() - word)
            .find(|&i| {
                row[i..i + word]
                    .iter()
                    .map(|cell| cell.symbol())
                    .collect::<String>()
                    == "PENDING"
            })
            .expect("Launch shows its status");
        for cell in &row[start..start + word] {
            assert!(
                cell.modifier.contains(Modifier::REVERSED) || cell.fg != cell.bg,
                "{appearance:?}: PENDING is drawn {:?} on {:?}",
                cell.fg,
                cell.bg
            );
        }
    }
}
