use bigdecimal::BigDecimal;

use crate::domain::{Discount, DomainError};

fn dec(value: &str) -> BigDecimal {
    value.parse().unwrap()
}

#[test]
fn percent_is_taken_of_the_subtotal_in_whole_paise() {
    let discount = Discount::Percent(dec("12.5"));
    assert_eq!(discount.amount_of(&dec("1000.10")).unwrap(), dec("125.01"));
    assert_eq!(
        Discount::Amount(dec("500"))
            .amount_of(&dec("1000"))
            .unwrap(),
        dec("500")
    );
}

#[test]
fn discount_must_leave_something_to_bill() {
    let subtotal = dec("1000");
    for discount in [Discount::Amount(dec("1000")), Discount::Amount(dec("1500"))] {
        assert!(matches!(
            discount.amount_of(&subtotal),
            Err(DomainError::DiscountTooLarge { .. })
        ));
    }
    for percent in ["0", "100", "-5"] {
        assert!(matches!(
            Discount::Percent(dec(percent)).amount_of(&subtotal),
            Err(DomainError::DiscountPercent)
        ));
    }
    assert!(matches!(
        Discount::Amount(dec("0")).amount_of(&subtotal),
        Err(DomainError::NonPositiveAmount)
    ));
    assert!(matches!(
        Discount::Amount(dec("10.005")).amount_of(&subtotal),
        Err(DomainError::FractionOfPaisa)
    ));
}

#[test]
fn split_is_proportional_and_the_last_line_takes_the_rounding() {
    let gross = [dec("100"), dec("100"), dec("100")];
    let shares = Discount::Amount(dec("100")).split(&gross).unwrap();
    assert_eq!(shares, vec![dec("33.33"), dec("33.33"), dec("33.34")]);

    let gross = [dec("30000"), dec("10000")];
    let shares = Discount::Percent(dec("10")).split(&gross).unwrap();
    assert_eq!(shares, vec![dec("3000"), dec("1000")]);
}

#[test]
fn shows_as_typed() {
    assert_eq!(Discount::Percent(dec("10.0")).to_string(), "10%");
    assert_eq!(Discount::Amount(dec("500")).to_string(), "500");
}
