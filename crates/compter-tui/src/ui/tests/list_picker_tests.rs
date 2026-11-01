use ratatui::Terminal;
use ratatui::backend::TestBackend;

use crate::ui::form::{ListEvent, ListItem, ListPicker};
use crate::ui::key::Key;
use crate::ui::theme::Theme;

fn picker() -> ListPicker {
    ListPicker::new(
        "Earlier clients",
        vec![
            ListItem::new("Acme Pvt Ltd", "27AAPFU0939F2ZU"),
            ListItem::new("Globex Labs", "Karnataka"),
            ListItem::new("Initrode GmbH", "Germany"),
        ],
    )
}

fn shown(picker: &ListPicker) -> String {
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal
        .draw(|frame| picker.render(frame, &Theme::default()))
        .unwrap();
    terminal
        .backend()
        .buffer()
        .content()
        .chunks(80)
        .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

fn type_in(picker: &mut ListPicker, text: &str) {
    for c in text.chars() {
        picker.handle_key(Key::Char(c));
    }
}

#[test]
fn arrows_move_and_enter_picks_the_row_under_the_cursor() {
    let mut picker = picker();
    assert_eq!(picker.handle_key(Key::Enter), ListEvent::Picked(0));

    picker.handle_key(Key::Down);
    picker.handle_key(Key::Down);
    assert_eq!(picker.handle_key(Key::Enter), ListEvent::Picked(2));
    picker.handle_key(Key::Down);
    assert_eq!(picker.handle_key(Key::Enter), ListEvent::Picked(0));
    assert_eq!(picker.handle_key(Key::Esc), ListEvent::Cancel);
}

#[test]
fn typing_searches_name_and_detail_even_with_j_and_k() {
    let mut picker = picker();
    // `k` and `j` are letters here, not movement
    type_in(&mut picker, "kar");
    assert_eq!(picker.handle_key(Key::Enter), ListEvent::Picked(1));
    let screen = shown(&picker);
    assert!(screen.contains("Globex Labs"), "{screen}");
    assert!(!screen.contains("Acme"), "{screen}");

    picker.handle_key(Key::Ctrl('u'));
    type_in(&mut picker, "germ");
    assert_eq!(picker.handle_key(Key::Enter), ListEvent::Picked(2));

    type_in(&mut picker, "zz");
    assert!(shown(&picker).contains("nothing matches"));
    assert_eq!(picker.handle_key(Key::Enter), ListEvent::Ignored);
    picker.handle_key(Key::Backspace);
    picker.handle_key(Key::Backspace);
    assert_eq!(picker.handle_key(Key::Enter), ListEvent::Picked(2));
}
