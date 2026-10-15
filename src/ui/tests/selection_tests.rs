use crate::ui::selection::{Nav, Selection};

#[test]
fn wraps_both_ways() {
    let mut sel = Selection::default();
    sel.move_by(Nav::Prev, 3);
    assert_eq!(sel.index(), 2);
    sel.move_by(Nav::Next, 3);
    assert_eq!(sel.index(), 0);
}

#[test]
fn empty_list_has_no_selection() {
    let mut sel = Selection::default();
    sel.move_by(Nav::Next, 0);
    assert_eq!(sel.index(), 0);
    assert_eq!(sel.table_state(0).selected(), None);
}
