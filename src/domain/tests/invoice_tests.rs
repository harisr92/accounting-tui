use accounting_core::{InvoiceParty, InvoicePrint};
use bigdecimal::BigDecimal;
use chrono::NaiveDate;

use crate::domain::{
    DomainError, FiscalYear, Invoice, InvoiceAmounts, InvoiceKind, InvoiceNumber, InvoiceParts,
    InvoiceStatus, InvoiceSummary, MilestoneId, ProjectId,
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
    Invoice::issue(InvoiceParts {
        number: number(1),
        date: date(1),
        kind: InvoiceKind::Plain,
        project_id: ProjectId::generate(),
        milestone_id: MilestoneId::generate(),
        client_name: "Acme".to_owned(),
        milestone_name: "Design".to_owned(),
        amounts: InvoiceAmounts::untaxed(BigDecimal::from(1000)),
        print: print(),
        terms: Vec::new(),
        gst: None,
    })
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
