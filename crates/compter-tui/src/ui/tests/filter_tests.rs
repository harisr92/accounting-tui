use crate::ui::filter::Filter;

#[test]
fn every_word_must_appear_in_some_field_ignoring_case() {
    let filter = Filter::new("  acme  WEB ");
    assert_eq!(filter.query(), "acme  WEB");
    assert!(filter.matches(&["Website Revamp", "Acme Pvt Ltd"]));
    assert!(!filter.matches(&["Website Revamp", "Globex"]));
}

#[test]
fn an_empty_filter_matches_everything() {
    let filter = Filter::new("   ");
    assert!(filter.is_empty());
    assert!(filter.matches(&[]));
    assert!(filter.matches(&["anything"]));
}

#[test]
fn the_search_form_keeps_the_current_words() {
    let form = Filter::new("acme").form("name or client");
    assert_eq!(Filter::from_form(&form), Filter::new("acme"));
}

#[test]
fn positions_lists_the_matching_rows_in_order() {
    let rows = ["Acme web", "Globex", "acme app"];
    assert_eq!(Filter::new("acme").positions(&rows, |r| [*r]), vec![0, 2]);
    assert_eq!(Filter::default().positions(&rows, |r| [*r]), vec![0, 1, 2]);
}
