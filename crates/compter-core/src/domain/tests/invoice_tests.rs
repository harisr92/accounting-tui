use accounting_core::{InvoiceParty, InvoicePrint};
use bigdecimal::BigDecimal;
use chrono::NaiveDate;

use crate::domain::{
    Currency, DomainError, ExchangeRate, FiscalYear, Invoice, InvoiceAmounts, InvoiceKind,
    InvoiceNumber, InvoiceParts, InvoiceStatus, InvoiceSummary, LogoId, MilestoneId, ProjectId,
};

fn date(day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, day).unwrap()
}

fn number(sequence: u32) -> InvoiceNumber {
    InvoiceNumber::new("INV", FiscalYear::containing(date(1)), sequence)
}

fn print() -> InvoicePrint {
    InvoicePrint {
        title: "Invoice".to_owned(),
        invoice_number: "INV/26-27/001".to_owned(),
        invoice_date: "01-10-2026".to_owned(),
        place_of_supply: String::new(),
        is_inter_state: false,
        seller: InvoiceParty::unregistered("Seller", vec!["Addr".to_owned()]),
        buyer: InvoiceParty::unregistered("Buyer", vec!["Addr".to_owned()]),
        rows: Vec::new(),
        taxable_value: "1000.00".to_owned(),
        tax_lines: Vec::new(),
        total_tax: "0.00".to_owned(),
        total: "1000.00".to_owned(),
        total_in_words: "Rupees One Thousand Only".to_owned(),
    }
}

fn invoice() -> Invoice {
    Invoice::issue(parts()).unwrap()
}

fn parts() -> InvoiceParts {
    InvoiceParts {
        number: number(1),
        date: date(1),
        kind: InvoiceKind::Plain,
        project_id: ProjectId::generate(),
        milestone_id: MilestoneId::generate(),
        client_name: "Acme".to_owned(),
        milestone_name: "Design".to_owned(),
        currency: Currency::Inr,
        exchange_rate: None,
        amounts: InvoiceAmounts::untaxed(BigDecimal::from(1000)),
        print: print(),
        terms: Vec::new(),
        gst: None,
        logo: None,
    }
}

/// $1,000 at ₹90
fn usd_invoice() -> Invoice {
    Invoice::issue(InvoiceParts {
        kind: InvoiceKind::Export,
        currency: Currency::Usd,
        exchange_rate: Some(ExchangeRate::new(BigDecimal::from(90)).unwrap()),
        ..parts()
    })
    .unwrap()
}

#[test]
fn numbers_follow_the_financial_year() {
    assert_eq!(number(4).as_str(), "INV/26-27/004");
    assert_eq!(number(1234).as_str(), "INV/26-27/1234");
    assert_eq!(number(4).file_stem(), "INV-26-27-004");
    let march = FiscalYear::containing(NaiveDate::from_ymd_opt(2027, 3, 31).unwrap());
    assert_eq!(
        InvoiceNumber::new("INV", march, 9).as_str(),
        "INV/26-27/009"
    );
}

#[test]
fn payments_may_come_in_parts_up_to_the_total() {
    let mut inv = invoice();
    let rupees = BigDecimal::from;
    assert_eq!(inv.status(), InvoiceStatus::Issued);

    inv.record_payment(date(5), rupees(400), rupees(100))
        .unwrap();
    assert_eq!(inv.status(), InvoiceStatus::PartlyPaid);
    assert_eq!(inv.balance(), rupees(500));
    let summary = InvoiceSummary::from(&inv);
    assert!(!summary.is_paid());
    assert_eq!(summary.balance(), &rupees(500));

    assert!(matches!(
        inv.record_payment(date(6), rupees(600), rupees(0)),
        Err(DomainError::PaymentExceedsBalance { .. })
    ));
    inv.record_payment(date(6), rupees(500), rupees(0)).unwrap();
    assert!(inv.is_paid());
    assert_eq!(inv.payments().len(), 2);
    assert!(matches!(
        inv.record_payment(date(7), rupees(1), rupees(0)),
        Err(DomainError::InvoiceAlreadyPaid)
    ));
}

#[test]
fn a_payment_must_be_positive_and_not_before_the_invoice() {
    let mut inv = invoice();
    let rupees = BigDecimal::from;
    assert!(matches!(
        inv.record_payment(date(5), rupees(0), rupees(0)),
        Err(DomainError::NonPositiveAmount)
    ));
    assert!(matches!(
        inv.record_payment(date(5), rupees(1100), rupees(-100)),
        Err(DomainError::NegativeAmount)
    ));
    let too_early = NaiveDate::from_ymd_opt(2026, 9, 30).unwrap();
    assert!(matches!(
        inv.record_payment(too_early, rupees(1000), rupees(0)),
        Err(DomainError::PaymentBeforeInvoice)
    ));
    assert!(inv.payments().is_empty());
}

#[test]
fn stored_payments_must_not_exceed_the_total() {
    let mut inv = invoice();
    inv.record_payment(date(5), BigDecimal::from(600), BigDecimal::from(0))
        .unwrap();
    let json = serde_json::to_value(&inv).unwrap();
    assert_eq!(
        serde_json::from_value::<Invoice>(json.clone()).unwrap(),
        inv
    );

    let mut tampered = json.clone();
    tampered["payments"][0]["received"] = "5000".into();
    assert!(serde_json::from_value::<Invoice>(tampered).is_err());

    let mut v1 = json;
    v1["status"] = serde_json::json!({"state": "issued"});
    assert!(matches!(
        serde_json::from_value::<Invoice>(v1).map_err(|e| e.to_string()),
        Err(message) if message.contains("old (v1) format")
    ));
}

#[test]
fn a_foreign_currency_needs_a_rate_and_rupees_have_none() {
    let rate = || Some(ExchangeRate::new(BigDecimal::from(90)).unwrap());
    let usd_without_rate = InvoiceParts {
        currency: Currency::Usd,
        ..parts()
    };
    assert!(matches!(
        Invoice::issue(usd_without_rate),
        Err(DomainError::MissingExchangeRate("USD"))
    ));
    let inr_with_rate = InvoiceParts {
        exchange_rate: rate(),
        ..parts()
    };
    assert!(matches!(
        Invoice::issue(inr_with_rate),
        Err(DomainError::UnexpectedExchangeRate)
    ));
    let foreign_tax = InvoiceParts {
        kind: InvoiceKind::Tax,
        currency: Currency::Eur,
        exchange_rate: rate(),
        ..parts()
    };
    assert!(matches!(
        Invoice::issue(foreign_tax),
        Err(DomainError::ForeignTaxInvoice)
    ));
}

#[test]
fn foreign_amounts_convert_to_rupees_at_the_invoice_rate() {
    let inv = usd_invoice();
    assert_eq!(inv.amounts().total, BigDecimal::from(1000));
    assert_eq!(inv.inr_amounts().total, BigDecimal::from(90_000));
    assert_eq!(inv.inr_amounts().taxable_value, BigDecimal::from(90_000));
    assert_eq!(inv.to_inr(&BigDecimal::from(10)), BigDecimal::from(900));
    assert_eq!(invoice().inr_amounts(), *invoice().amounts());
}

#[test]
fn foreign_payments_settle_in_the_currency_and_record_rupees_banked() {
    let mut inv = usd_invoice();
    let zero = || BigDecimal::from(0);
    assert!(matches!(
        inv.record_payment(date(5), BigDecimal::from(1000), zero()),
        Err(DomainError::MissingCreditedAmount)
    ));
    assert!(matches!(
        inv.record_foreign_payment(date(5), BigDecimal::from(400), zero()),
        Err(DomainError::MissingCreditedAmount)
    ));
    let first = inv
        .record_foreign_payment(date(5), BigDecimal::from(400), BigDecimal::from(36_400))
        .unwrap();
    assert_eq!(first.received_inr(), &BigDecimal::from(36_400));
    assert_eq!(inv.status(), InvoiceStatus::PartlyPaid);
    // The rest at a lower rate still settles exactly: the balance is in dollars
    inv.record_foreign_payment(date(9), BigDecimal::from(600), BigDecimal::from(53_000))
        .unwrap();
    assert!(inv.is_paid());

    let mut rupee = invoice();
    assert!(matches!(
        rupee.record_foreign_payment(date(5), BigDecimal::from(10), BigDecimal::from(10)),
        Err(DomainError::UnexpectedCreditedAmount)
    ));
}

#[test]
fn stored_foreign_payments_keep_their_rules() {
    let mut inv = usd_invoice();
    inv.record_foreign_payment(date(5), BigDecimal::from(400), BigDecimal::from(36_000))
        .unwrap();
    let json = serde_json::to_value(&inv).unwrap();
    assert_eq!(json["currency"], "USD");
    assert_eq!(
        serde_json::from_value::<Invoice>(json.clone()).unwrap(),
        inv
    );
    let summary = InvoiceSummary::from(&inv);
    assert_eq!(summary.currency(), Currency::Usd);
    assert_eq!(summary.balance_inr(), BigDecimal::from(54_000));

    let mut with_tds = json.clone();
    with_tds["payments"][0]["tds"] = "10".into();
    assert!(serde_json::from_value::<Invoice>(with_tds).is_err());
    let mut without_rate = json;
    without_rate
        .as_object_mut()
        .unwrap()
        .remove("exchange_rate");
    assert!(serde_json::from_value::<Invoice>(without_rate).is_err());
}

#[test]
fn records_without_a_currency_are_rupees() {
    let mut json = serde_json::to_value(invoice()).unwrap();
    json.as_object_mut().unwrap().remove("currency");
    let read = serde_json::from_value::<Invoice>(json).unwrap();
    assert_eq!(read.currency(), Currency::Inr);
    assert!(read.exchange_rate().is_none());
}

#[test]
fn the_logo_is_kept_with_the_invoice_and_older_records_have_none() {
    let logo = LogoId::generate();
    let inv = Invoice::issue(InvoiceParts {
        logo: Some(logo),
        ..parts()
    })
    .unwrap();
    assert_eq!(inv.logo(), Some(logo));

    let json = serde_json::to_value(&inv).unwrap();
    assert_eq!(
        serde_json::from_value::<Invoice>(json).unwrap().logo(),
        Some(logo)
    );
    let without = serde_json::to_value(Invoice::issue(parts()).unwrap()).unwrap();
    assert!(without.get("logo").is_none());
    assert_eq!(
        serde_json::from_value::<Invoice>(without).unwrap().logo(),
        None
    );
}

#[test]
fn the_breakdown_names_the_gst_or_why_there_is_none() {
    let amount = |s: &str| s.parse::<BigDecimal>().unwrap();
    let mut a = InvoiceAmounts::untaxed(amount("50000"));
    assert_eq!(
        a.breakdown(InvoiceKind::Plain, Currency::Inr),
        "50,000.00, no GST"
    );
    assert_eq!(
        a.breakdown(InvoiceKind::Export, Currency::Usd),
        "50,000.00, zero-rated export under LUT"
    );
    a.cgst = amount("4500");
    a.sgst = amount("4500");
    assert_eq!(
        a.breakdown(InvoiceKind::Tax, Currency::Inr),
        "50,000.00 + CGST 4,500.00 + SGST 4,500.00"
    );
    a.cgst = amount("0");
    a.sgst = amount("0");
    a.igst = amount("9000");
    assert_eq!(
        a.breakdown(InvoiceKind::Tax, Currency::Inr),
        "50,000.00 + IGST 9,000.00"
    );
}

#[test]
fn the_usual_tds_is_ten_percent_of_the_taxable_value() {
    assert_eq!(
        invoice().usual_tds(),
        &invoice().amounts().taxable_value / BigDecimal::from(10)
    );
}
