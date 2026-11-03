use crate::domain::{state_label, state_name, states};

#[test]
fn every_state_is_listed_in_code_order_with_its_label() {
    let listed: Vec<_> = states().collect();
    assert!(
        listed
            .windows(2)
            .all(|pair| pair[0].code() < pair[1].code())
    );
    let karnataka = listed.iter().find(|s| s.code() == "29").unwrap();
    assert_eq!(karnataka.name(), "Karnataka");
    assert_eq!(karnataka.label(), "Karnataka (29)");
}

#[test]
fn a_label_names_a_known_code_and_keeps_an_unknown_one_bare() {
    assert_eq!(state_label("29"), "Karnataka (29)");
    assert_eq!(state_label("99"), "99");
    assert_eq!(state_name("27"), Some("Maharashtra"));
}
