use bigdecimal::BigDecimal;

use crate::ui::widgets::{duration, money};

#[test]
fn formats_duration() {
    assert_eq!(duration(390), "6h 30m");
    assert_eq!(duration(5), "0h 05m");
}

#[test]
fn money_puts_the_sign_before_the_rupee() {
    assert_eq!(money(&BigDecimal::from(150_000)), "₹1,50,000.00");
    assert_eq!(money(&BigDecimal::from(-13_680)), "-₹13,680.00");
}
