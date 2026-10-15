use crate::ui::widgets::duration;

#[test]
fn formats_duration() {
    assert_eq!(duration(390), "6h 30m");
    assert_eq!(duration(5), "0h 05m");
}
