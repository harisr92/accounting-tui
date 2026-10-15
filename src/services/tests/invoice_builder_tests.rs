use accounting_core::{EntryType, StateCode};
use bigdecimal::BigDecimal;

use super::fixtures::{
    OTHER_STATE_GSTIN, SAME_STATE_GSTIN, SELLER_GSTIN, abroad, client, date, fixed_project,
    hourly_project, india, profile,
};
use crate::domain::{ClientRegion, FiscalYear, InvoiceKind, InvoiceNumber, Project};
use crate::services::{InvoicingError, build_invoice, chart};

fn number() -> InvoiceNumber {
    InvoiceNumber::new("INV", FiscalYear::containing(date()), 1)
}

fn build(
    profile: &crate::domain::BusinessProfile,
    project: &Project,
) -> Result<crate::services::BuiltInvoice, InvoicingError> {
    build_invoice(profile, project, &project.milestones()[0], number(), date())
}

fn rupees(amount: i64) -> BigDecimal {
    BigDecimal::from(amount)
}

#[test]
fn intra_state_splits_cgst_and_sgst() {
    let seller = profile(Some(SELLER_GSTIN), None);
    let project = fixed_project(client(Some(SAME_STATE_GSTIN), india(), &[]), 50_000);
    let built = build(&seller, &project).unwrap();
    let amounts = built.invoice.amounts();
    assert_eq!(built.invoice.kind(), InvoiceKind::Tax);
    assert_eq!(
        (&amounts.cgst, &amounts.sgst, &amounts.igst),
        (&rupees(4500), &rupees(4500), &rupees(0))
    );
    assert_eq!(amounts.total, rupees(59_000));
    assert_eq!(built.invoice.print().title, "Tax Invoice");
    assert!(built.invoice.gst().is_some());
}

#[test]
fn inter_state_charges_igst_and_posts_balanced_entries() {
    let seller = profile(Some(SELLER_GSTIN), None);
    let project = fixed_project(client(Some(OTHER_STATE_GSTIN), india(), &[]), 50_000);
    let built = build(&seller, &project).unwrap();
    assert_eq!(built.invoice.amounts().igst, rupees(9000));

    let side = |kind: EntryType| -> BigDecimal {
        built
            .entries
            .iter()
            .filter(|e| e.entry_type == kind)
            .map(|e| e.amount.clone())
            .sum()
    };
    assert_eq!(side(EntryType::Debit), rupees(59_000));
    assert_eq!(side(EntryType::Credit), rupees(59_000));
    assert!(
        built
            .entries
            .iter()
            .any(|e| e.account_id == chart::IGST_OUTPUT && e.amount == rupees(9000))
    );
}

#[test]
fn unregistered_indian_client_needs_a_state_and_large_ones_an_address() {
    let seller = profile(Some(SELLER_GSTIN), None);
    let no_state = fixed_project(client(None, india(), &[]), 10_000);
    assert!(matches!(
        build(&seller, &no_state),
        Err(InvoicingError::MissingClientState)
    ));

    let karnataka = ClientRegion::India {
        state: Some(StateCode::parse("29").unwrap()),
    };
    let small = fixed_project(client(None, karnataka.clone(), &[]), 10_000);
    let built = build(&seller, &small).unwrap();
    assert_eq!(built.invoice.print().place_of_supply, "29");
    assert_eq!(built.invoice.amounts().igst, rupees(1800));

    let large = fixed_project(client(None, karnataka, &[]), 60_000);
    assert!(build(&seller, &large).is_err());
}

#[test]
fn export_under_lut_is_zero_rated_and_endorsed() {
    let address = ["Hauptstr. 1", "10115 Berlin"];
    let project = fixed_project(client(None, abroad(), &address), 100_000);

    let no_lut = profile(Some(SELLER_GSTIN), None);
    assert!(matches!(
        build(&no_lut, &project),
        Err(InvoicingError::MissingLut)
    ));

    let seller = profile(Some(SELLER_GSTIN), Some("AD2710260012345"));
    let built = build(&seller, &project).unwrap();
    let invoice = &built.invoice;
    assert_eq!(invoice.kind(), InvoiceKind::Export);
    assert_eq!(invoice.amounts().total, rupees(100_000));
    assert_eq!(invoice.amounts().total_tax(), rupees(0));
    assert!(invoice.terms()[0].contains("export under LUT (ARN AD2710260012345)"));
    assert_eq!(invoice.print().place_of_supply, "96");
    assert_eq!(
        invoice.print().buyer.address.last().map(String::as_str),
        Some("Germany")
    );
    assert!(
        built
            .entries
            .iter()
            .any(|e| e.account_id == chart::EXPORT_REVENUE)
    );
}

#[test]
fn export_needs_the_client_address() {
    let seller = profile(Some(SELLER_GSTIN), Some("AD2710260012345"));
    let project = fixed_project(client(None, abroad(), &[]), 5_000);
    assert!(matches!(
        build(&seller, &project),
        Err(InvoicingError::MissingClientAddress)
    ));
}

#[test]
fn unregistered_seller_issues_a_plain_invoice() {
    let seller = profile(None, None);
    let project = fixed_project(client(Some(OTHER_STATE_GSTIN), india(), &[]), 50_000);
    let built = build(&seller, &project).unwrap();
    let invoice = &built.invoice;
    assert_eq!(invoice.kind(), InvoiceKind::Plain);
    assert_eq!(invoice.print().title, "Invoice");
    assert!(invoice.print().tax_lines.is_empty());
    assert_eq!(invoice.amounts().total, rupees(50_000));
    assert!(invoice.print().seller.gstin.is_none());
    assert_eq!(invoice.terms()[0], crate::services::PLAIN_NOTE);
    assert_eq!(&invoice.terms()[1..], seller.terms());

    let unregistered = fixed_project(client(None, india(), &[]), 50_000);
    assert!(matches!(
        build(&seller, &unregistered),
        Err(InvoicingError::MissingClientAddress)
    ));
}

#[test]
fn hourly_lines_show_the_time_and_rate() {
    let seller = profile(Some(SELLER_GSTIN), None);
    let project = hourly_project(client(Some(OTHER_STATE_GSTIN), india(), &[]));
    let built = build(&seller, &project).unwrap();
    let row = &built.invoice.print().rows[0];
    assert_eq!(
        row.description,
        "App - Prototype (10h 30m @ Rs. 1,500.00/hour)"
    );
    assert_eq!(built.invoice.amounts().taxable_value, rupees(15_750));
}

#[test]
fn only_pending_milestones_are_invoiced() {
    let seller = profile(Some(SELLER_GSTIN), None);
    let mut project = fixed_project(client(Some(OTHER_STATE_GSTIN), india(), &[]), 1_000);
    project.invoice_milestone(0, "INV/26-27/001").unwrap();
    assert!(matches!(
        build(&seller, &project),
        Err(InvoicingError::NotPending)
    ));
}
