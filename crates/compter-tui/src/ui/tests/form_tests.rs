use crate::ui::key::Key;

use crate::ui::form::{Confirm, Field, Form, FormEvent, ListItem};

const OPTIONS: &[&str] = &["Fixed price", "Hourly"];

fn form() -> Form {
    Form::new(
        "Test",
        vec![
            Field::text("Name", "hint").with_value("Ab"),
            Field::choice("Billing", OPTIONS),
        ],
    )
}

fn press(form: &mut Form, keys: &str) {
    for c in keys.chars() {
        form.handle_key(Key::Char(c));
    }
}

#[test]
fn edits_text_in_the_focused_field() {
    let mut form = form();
    press(&mut form, "c");
    assert_eq!(form.value(0), "Abc");
    form.handle_key(Key::Backspace);
    assert_eq!(form.value(0), "Ab");
    form.handle_key(Key::Ctrl('u'));
    assert_eq!(form.value(0), "");
}

#[test]
fn tab_moves_between_fields_and_choices_cycle() {
    let mut form = form();
    assert_eq!(form.handle_key(Key::Tab), FormEvent::Edited);
    press(&mut form, " ");
    assert_eq!((form.choice(1), form.value(1)), (1, "Hourly"));
    form.handle_key(Key::Right);
    assert_eq!(form.choice(1), 0);
    form.handle_key(Key::Left);
    assert_eq!(form.choice(1), 1);
    assert_eq!(form.handle_key(Key::Char('x')), FormEvent::Ignored);
    form.handle_key(Key::BackTab);
    press(&mut form, "!");
    assert_eq!(form.value(0), "Ab!");
}

#[test]
fn enter_submits_escape_cancels() {
    let mut form = form();
    assert_eq!(form.handle_key(Key::Enter), FormEvent::Submit);
    assert_eq!(form.handle_key(Key::Esc), FormEvent::Cancel);
}

#[test]
fn editing_clears_a_reported_error() {
    let mut form = form();
    form.set_error("bad name");
    assert_eq!(form.error(), Some("bad name"));
    press(&mut form, "x");
    assert_eq!(form.error(), None);
}

#[test]
fn text_input_is_capped() {
    let mut form = form();
    press(&mut form, &"x".repeat(500));
    assert_eq!(form.value(0).chars().count(), 120);
}

#[test]
fn confirm_answers() {
    assert_eq!(Confirm::answer(Key::Char('y')), Some(true));
    assert_eq!(Confirm::answer(Key::Char('n')), Some(false));
    assert_eq!(Confirm::answer(Key::Esc), Some(false));
    assert_eq!(Confirm::answer(Key::Char('q')), None);
}

#[test]
fn long_choices_show_one_option_and_errors_wrap() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use crate::ui::theme::Theme;

    const MANY: &[&str] = &["One", "Two", "Three", "Four"];
    let mut form = Form::new("Test", vec![Field::choice("Pick", MANY).with_choice(1)]);
    form.set_error(
        "a very long explanation that certainly does not fit on one line of the form at all",
    );
    let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
    terminal
        .draw(|frame| form.render(frame, &Theme::default()))
        .unwrap();
    let screen: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(screen.contains("‹ Two ›"));
    assert!(screen.contains("2/4"));
    assert!(!screen.contains("Three"));
    assert!(screen.contains("of the form at all"));
}

#[test]
fn a_picker_fills_fields_in_and_clears_the_error() {
    let mut form = form();
    form.set_error("refused");

    form.set_value(0, "Acme");
    form.set_choice(1, 1);
    form.set_choice(1, 9);
    form.set_value(7, "out of range");

    assert_eq!(form.value(0), "Acme");
    assert_eq!(form.choice(1), 1);
    assert_eq!(form.error(), None);
}

fn coded() -> Form {
    let entries = [("29", "Karnataka (29)"), ("32", "Kerala (32)")]
        .map(|(code, label)| (code.to_owned(), ListItem::new(label, "")))
        .to_vec();
    Form::new(
        "Test",
        vec![
            Field::text("Code", "hint").with_list("Codes", entries),
            Field::text("Name", "hint"),
        ],
    )
}

#[test]
fn ctrl_l_fills_a_field_in_from_its_list() {
    let mut form = coded();
    assert_eq!(form.handle_key(Key::Ctrl('l')), FormEvent::Edited);
    assert!(form.is_picking());
    // Every key goes to the list, Enter included
    press(&mut form, "ker");
    assert_eq!(form.handle_key(Key::Enter), FormEvent::Edited);
    assert!(!form.is_picking());
    assert_eq!(form.value(0), "32");

    form.handle_key(Key::Ctrl('l'));
    assert_eq!(form.handle_key(Key::Esc), FormEvent::Edited);
    assert_eq!(form.value(0), "32");
    // A field without a list has nothing to open
    form.handle_key(Key::Tab);
    assert_eq!(form.handle_key(Key::Ctrl('l')), FormEvent::Ignored);
}

/// `ABC` once three letters are typed above
fn shout(sources: &[&str]) -> Option<String> {
    match sources {
        [source] => (source.len() == 3).then(|| source.to_uppercase()),
        _ => None,
    }
}

#[test]
fn a_derived_field_follows_its_source_and_gives_back_what_was_typed() {
    let mut form = Form::new(
        "Test",
        vec![
            Field::text("Source", "hint"),
            Field::text("Derived", "hint")
                .with_value("typed")
                .with_list("Codes", vec![("x".to_owned(), ListItem::new("x", ""))])
                .derived_from(&[1], shout, "from the source"),
        ],
    );
    press(&mut form, "ab");
    assert_eq!(form.value(1), "typed");
    press(&mut form, "c");
    assert_eq!(form.value(1), "ABC");

    // Locked while derived: no typing, no list
    form.handle_key(Key::Tab);
    assert_eq!(form.handle_key(Key::Char('z')), FormEvent::Ignored);
    assert_eq!(form.handle_key(Key::Ctrl('l')), FormEvent::Ignored);
    // Filled in from outside while derived, it shows once the source no longer derives
    form.set_value(1, "filled");
    assert_eq!(form.value(1), "ABC");
    form.set_value(0, "abcd");
    assert_eq!(form.value(1), "filled");
}

#[test]
fn a_lines_field_moves_between_its_lines_and_grows_up_to_its_limit() {
    let mut form = Form::new(
        "Test",
        vec![
            Field::text("Before", "hint"),
            Field::lines("Address", "hint", 3),
            Field::text("After", "hint"),
        ],
    );
    form.handle_key(Key::Tab);
    // No second line before the first has something on it
    assert_eq!(form.handle_key(Key::Down), FormEvent::Edited);
    press(&mut form, "x");
    assert_eq!(form.value(2), "x");
    form.handle_key(Key::Up);

    press(&mut form, "a");
    form.handle_key(Key::Down);
    press(&mut form, "b");
    form.handle_key(Key::Down);
    press(&mut form, "c");
    // At its limit ↓ leaves the field
    form.handle_key(Key::Down);
    press(&mut form, "!");
    assert_eq!(form.value(2), "x!");
    assert_eq!(form.lines(1), ["a", "b", "c"]);

    // Coming back from below lands on the last line; Backspace on a blank line removes it
    form.handle_key(Key::Up);
    form.handle_key(Key::Ctrl('u'));
    form.handle_key(Key::Backspace);
    press(&mut form, "2");
    assert_eq!(form.lines(1), ["a", "b2"]);
    // ↑ from the first line goes to the field above
    form.handle_key(Key::Up);
    form.handle_key(Key::Up);
    press(&mut form, "?");
    assert_eq!(form.value(0), "?");
}

#[test]
fn a_lines_field_is_set_from_text_and_reads_back_without_blank_lines() {
    let mut form = Form::new("Test", vec![Field::lines("Address", "hint", 6)]);
    form.set_value(0, "  12 Main Road \n\nBengaluru");
    assert_eq!(form.lines(0), ["12 Main Road", "Bengaluru"]);
    assert_eq!(form.value(0), "");
}
