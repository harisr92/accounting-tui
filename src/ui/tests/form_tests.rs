use termion::event::Key;

use crate::ui::form::{Confirm, Field, Form, FormEvent};

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
    assert_eq!(form.handle_key(Key::Char('\t')), FormEvent::Edited);
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
    assert_eq!(form.handle_key(Key::Char('\n')), FormEvent::Submit);
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
