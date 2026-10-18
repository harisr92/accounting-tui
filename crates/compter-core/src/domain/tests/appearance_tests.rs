use crate::domain::Appearance;

#[test]
fn themes_cycle_dark_light_mono() {
    assert_eq!(Appearance::Dark.next(), Appearance::Light);
    assert_eq!(Appearance::Light.next(), Appearance::Mono);
    assert_eq!(Appearance::Mono.next(), Appearance::Dark);
}
