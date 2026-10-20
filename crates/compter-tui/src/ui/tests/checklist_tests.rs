use crate::ui::key::Key;

use crate::ui::form::{Checklist, ChecklistItem, FormEvent};
use crate::ui::view::{Action, Notice};

/// Design (picked, ₹500 to edit), Build (₹300 to edit), Launch (nothing left)
fn list() -> Checklist {
    Checklist::new(
        "Lines",
        ["Milestone", "Billed", "Credit"],
        vec![
            ChecklistItem::new("Design", "₹500.00")
                .with_amount("500")
                .checked(true),
            ChecklistItem::new("Build", "₹300.00").with_amount("300"),
            ChecklistItem::new("Launch", "₹200.00").disabled(),
        ],
    )
}

#[test]
fn space_picks_the_row_under_the_cursor_and_a_picks_all_or_none() {
    let mut list = list();
    assert_eq!(list.checked(), [0]);
    list.handle_key(Key::Char('j'));
    assert_eq!(list.handle_key(Key::Char(' ')), FormEvent::Edited);
    assert_eq!(list.checked(), [0, 1]);
    list.handle_key(Key::Char(' '));
    assert_eq!(list.checked(), [0]);

    // All the rows that can be picked, then none
    list.handle_key(Key::Char('a'));
    assert_eq!(list.checked(), [0, 1]);
    list.handle_key(Key::Char('a'));
    assert!(list.checked().is_empty());
}

#[test]
fn a_row_with_nothing_left_cant_be_picked() {
    let mut list = list();
    list.handle_key(Key::Char('k'));
    assert_eq!(list.handle_key(Key::Char(' ')), FormEvent::Ignored);
    assert_eq!(list.handle_key(Key::Char('5')), FormEvent::Ignored);
    assert_eq!(list.checked(), [0]);
}

#[test]
fn with_nothing_picked_the_cursor_starts_on_a_row_that_can_be_picked() {
    let mut list = Checklist::new(
        "Lines",
        ["Milestone", "Billed", "Credit"],
        vec![
            ChecklistItem::new("Design", "₹500.00").disabled(),
            ChecklistItem::new("Build", "₹300.00").with_amount(""),
        ],
    );
    assert_eq!(list.handle_key(Key::Char('7')), FormEvent::Edited);
    assert_eq!(list.amount(1), "7");
    assert_eq!(list.checked(), [1]);
}

#[test]
fn typing_an_amount_edits_and_picks_its_row() {
    let mut list = list();
    list.handle_key(Key::Down);
    list.handle_key(Key::Ctrl('u'));
    assert_eq!(list.amount(1), "");
    for c in "120.5".chars() {
        list.handle_key(Key::Char(c));
    }
    list.handle_key(Key::Backspace);
    assert_eq!(list.amount(1), "120.");
    assert_eq!(list.checked(), [0, 1]);
    assert_eq!(list.amount(2), "");
}

#[test]
fn a_refused_submit_keeps_the_list_open_with_the_error() {
    let (list, action) = list().step(Key::Enter, |_| Err("Pick at least one".to_owned()));
    let list = list.expect("still open");
    assert_eq!(list.error(), Some("Pick at least one"));
    assert_eq!(action, Action::Handled);
    assert_eq!(list.checked(), [0]);

    let (closed, action) = list.step(Key::Enter, |list| {
        Ok(Notice::info(format!("{} picked", list.checked().len())))
    });
    assert!(closed.is_none());
    assert_eq!(action, Action::Notify(Notice::info("1 picked")));

    let (closed, action) = self::list().step(Key::Esc, |_| unreachable!());
    assert!(closed.is_none());
    assert_eq!(action, Action::Handled);
}

#[test]
fn amount_keys_are_hinted_only_on_lists_that_take_amounts() {
    assert_eq!(list().hints().len(), Checklist::AMOUNT_HINTS.len());
    let plain = Checklist::new(
        "Milestones",
        ["Milestone", "Amount", ""],
        vec![ChecklistItem::new("Design", "₹500.00")],
    );
    assert!(plain.hints().iter().all(|hint| hint.key != "0-9"));
    assert!(list().hints().iter().any(|hint| hint.key == "0-9"));
}

#[test]
fn picking_a_row_fills_an_empty_amount_and_typing_keeps_its_own() {
    let mut list = Checklist::new(
        "Lines",
        ["Milestone", "Billed", "Credit"],
        vec![
            ChecklistItem::new("Design", "₹500.00")
                .with_amount("")
                .with_fill("400.00"),
            ChecklistItem::new("Build", "₹300.00")
                .with_amount("")
                .with_fill("300.00"),
        ],
    );
    list.handle_key(Key::Char(' '));
    assert_eq!(list.amount(0), "400.00");
    // Unpicking keeps what was there, and an amount typed is not replaced
    list.handle_key(Key::Char(' '));
    list.handle_key(Key::Char('j'));
    list.handle_key(Key::Char('5'));
    assert_eq!(list.amount(1), "5");
    list.handle_key(Key::Char('a'));
    assert_eq!(list.checked(), [0, 1]);
    assert_eq!((list.amount(0), list.amount(1)), ("400.00", "5"));
}

#[test]
fn typing_over_an_amount_put_in_replaces_it() {
    let mut list = list();
    // Design's 500 was prefilled: typing 4 starts a new amount rather than making 5004
    list.handle_key(Key::Char('4'));
    list.handle_key(Key::Char('0'));
    assert_eq!(list.amount(0), "40");

    // An amount filled in by picking is replaced the same way
    let mut list = Checklist::new(
        "Lines",
        ["Milestone", "Billed", "Credit"],
        vec![
            ChecklistItem::new("Build", "₹300.00")
                .with_amount("")
                .with_fill("300.00"),
        ],
    );
    list.handle_key(Key::Char(' '));
    assert_eq!(list.amount(0), "300.00");
    list.handle_key(Key::Char('2'));
    assert_eq!(list.amount(0), "2");
}

#[test]
fn clearing_an_amount_leaves_the_row_picked_or_not() {
    let mut list = list();
    list.handle_key(Key::Char('j'));
    list.handle_key(Key::Ctrl('u'));
    list.handle_key(Key::Backspace);
    assert_eq!(list.amount(1), "");
    assert_eq!(list.checked(), [0]);
    list.handle_key(Key::Char('k'));
    list.handle_key(Key::Ctrl('u'));
    assert_eq!(list.checked(), [0]);
}
