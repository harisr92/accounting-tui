use std::str::FromStr;

use bigdecimal::BigDecimal;

use crate::domain::{Currency, DomainError, ExchangeRate};

fn amount(text: &str) -> BigDecimal {
    BigDecimal::from_str(text).unwrap()
}

#[test]
fn rupees_keep_indian_grouping_and_others_group_by_thousands() {
    assert_eq!(Currency::Inr.format(&amount("150000")), "1,50,000.00");
    assert_eq!(Currency::Usd.format(&amount("150000")), "150,000.00");
    assert_eq!(Currency::Eur.format(&amount("1234567.891")), "1,234,567.89");
    assert_eq!(Currency::Gbp.format(&amount("999")), "999.00");
    assert_eq!(Currency::Usd.format(&amount("-1000.5")), "-1,000.50");
    assert_eq!(Currency::Usd.format(&amount("-0.001")), "0.00");
}

#[test]
fn amounts_are_spelled_out_in_their_currency() {
    assert_eq!(
        Currency::Inr.in_words(&amount("17700")),
        "Rupees Seventeen Thousand Seven Hundred Only"
    );
    assert_eq!(
        Currency::Usd.in_words(&amount("1250.50")),
        "US Dollars One Thousand Two Hundred Fifty and Fifty Cents Only"
    );
    assert_eq!(
        Currency::Gbp.in_words(&amount("2000015")),
        "Pounds Sterling Two Million Fifteen Only"
    );
    assert_eq!(
        Currency::Eur.in_words(&amount("0.05")),
        "Euros Zero and Five Cents Only"
    );
    assert_eq!(Currency::Eur.in_words(&amount("0")), "Euros Zero Only");
}

#[test]
fn rupees_print_as_rs_since_the_pdf_font_has_no_rupee_sign() {
    assert_eq!(Currency::Inr.pdf_label(), "Rs.");
    assert_eq!(Currency::Usd.pdf_label(), "USD");
    assert_eq!(Currency::Inr.symbol(), "₹");
    assert_eq!(Currency::Eur.symbol(), "€");
}

#[test]
fn form_options_map_back_to_the_currency() {
    for currency in Currency::ALL {
        assert_eq!(Currency::from_index(currency.index()), currency);
        assert!(Currency::LABELS[currency.index()].starts_with(currency.code()));
    }
    assert_eq!(Currency::from_index(99), Currency::Inr);
}

#[test]
fn currencies_are_stored_by_code() {
    assert_eq!(serde_json::to_string(&Currency::Gbp).unwrap(), "\"GBP\"");
    let read: Currency = serde_json::from_str("\"USD\"").unwrap();
    assert_eq!(read, Currency::Usd);
}

#[test]
fn rates_are_positive_and_kept_to_four_places() {
    let rate = ExchangeRate::new(amount("83.123456")).unwrap();
    assert_eq!(rate.value(), &amount("83.1235"));
    assert_eq!(rate.to_inr(&amount("1000")), amount("83123.50"));
    assert_eq!(rate.to_inr(&amount("0.01")), amount("0.83"));
    assert!(matches!(
        ExchangeRate::new(amount("0")),
        Err(DomainError::InvalidExchangeRate)
    ));
    assert!(serde_json::from_str::<ExchangeRate>("-1").is_err());
    let read: ExchangeRate = serde_json::from_str("90.5").unwrap();
    assert_eq!(read.value(), &amount("90.5"));
}

#[test]
fn display_puts_the_symbol_after_a_minus_sign() {
    let amount = BigDecimal::from_str("150000.5").unwrap();
    assert_eq!(Currency::Inr.display(&amount), "₹1,50,000.50");
    assert_eq!(Currency::Usd.display(&amount), "$150,000.50");
    assert_eq!(Currency::Eur.display(&-amount), "-€150,000.50");
}
