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
fn payment_must_settle_the_total_exactly() {
    let mut inv = invoice();
    let rupees = BigDecimal::from;
    assert!(matches!(
        inv.record_payment(date(5), rupees(800), rupees(100)),
        Err(DomainError::PaymentMismatch { .. })
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

    inv.record_payment(date(5), rupees(900), rupees(100))
        .unwrap();
    assert!(matches!(inv.status(), InvoiceStatus::Paid { tds, .. } if *tds == rupees(100)));
    assert!(InvoiceSummary::from(&inv).is_paid());
    assert!(matches!(
        inv.record_payment(date(6), rupees(1000), rupees(0)),
        Err(DomainError::InvoiceAlreadyPaid)
    ));
}

#[test]
fn stored_payment_must_add_up() {
    let mut inv = invoice();
    inv.record_payment(date(5), BigDecimal::from(1000), BigDecimal::from(0))
        .unwrap();
    let json = serde_json::to_value(&inv).unwrap();
    assert_eq!(
        serde_json::from_value::<Invoice>(json.clone()).unwrap(),
        inv
    );

    let mut tampered = json;
    tampered["status"]["received"] = "1".into();
    assert!(serde_json::from_value::<Invoice>(tampered).is_err());
}
