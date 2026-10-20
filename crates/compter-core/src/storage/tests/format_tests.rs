//! Records saved by earlier builds must keep loading. If one of these tests fails, the stored
//! format changed: add `#[serde(default)]` or a migration (see `storage/migrations.rs`).

use bigdecimal::BigDecimal;

use crate::domain::{
    Billing, BusinessProfile, ClientRegion, CreditNote, Currency, Expense, ExpenseCategory,
    Invoice, InvoiceKind, InvoiceStatus, MilestoneStatus, Project, ProjectStatus, ReturnKind,
};
use crate::storage::codec::decode;
use crate::storage::migrations::invoice_v1_to_v2;

const HOURLY_V1: &str = include_str!("fixtures/project_v1.json");
const FIXED_V1: &str = include_str!("fixtures/project_fixed_v1.json");
const CLIENT_V1: &str = include_str!("fixtures/project_client_v1.json");
const EXPENSE_V1: &str = include_str!("fixtures/expense_v1.json");
const PARTLY_PAID_V2: &str = include_str!("fixtures/invoice_partly_paid_v2.json");
const PROFILE_V1: &str = include_str!("fixtures/business_profile_v1.json");
const TAX_INVOICE_V1: &str = include_str!("fixtures/invoice_tax_v1.json");
const EXPORT_INVOICE_V1: &str = include_str!("fixtures/invoice_export_v1.json");
const PLAIN_INVOICE_V1: &str = include_str!("fixtures/invoice_plain_v1.json");
const EUR_INVOICE_V2: &str = include_str!("fixtures/invoice_export_eur_v2.json");
const EUR_PROJECT_V2: &str = include_str!("fixtures/project_eur_v2.json");
const CREDITED_PROJECT_V3: &str = include_str!("fixtures/project_credited_v3.json");
const CREDIT_NOTE_V3: &str = include_str!("fixtures/credit_note_v3.json");
const CREDITED_INVOICE_V3: &str = include_str!("fixtures/invoice_credited_v3.json");
const RETURNED_INVOICE_V4: &str = include_str!("fixtures/invoice_returned_v4.json");
const REFUNDED_EUR_INVOICE_V4: &str = include_str!("fixtures/invoice_refunded_eur_v4.json");
const TDS_REVERSED_INVOICE_V4: &str = include_str!("fixtures/invoice_tds_reversed_v4.json");
const LINES_INVOICE_V5: &str = include_str!("fixtures/invoice_lines_v5.json");
const LINES_CREDIT_NOTE_V5: &str = include_str!("fixtures/credit_note_lines_v5.json");

#[test]
fn hourly_project_v1_still_loads() {
    let project: Project = decode("projects", "fixture", HOURLY_V1.as_bytes()).unwrap();
    assert_eq!(project.name(), "Mobile App");
    assert_eq!(project.billing().rate(), Some(&BigDecimal::from(1500)));
    assert!(project.client_gstin().is_none());
    let milestones = project.milestones();
    assert_eq!(milestones[0].status(), MilestoneStatus::Invoiced);
    assert_eq!(milestones[0].logged_minutes(), 630);
    assert_eq!(milestones[1].time_entries()[0].note(), "Auth spike");
    assert_eq!(project.total_amount(), &BigDecimal::from(18_750));
    assert_eq!(project.status(), ProjectStatus::Active);
}

#[test]
fn fixed_project_v1_still_loads() {
    let project: Project = decode("projects", "fixture", FIXED_V1.as_bytes()).unwrap();
    assert_eq!(project.billing(), &Billing::Fixed);
    assert_eq!(
        project.client_gstin().map(|g| g.as_str()),
        Some("27AAPFU0939F1ZV")
    );
    assert_eq!(project.milestones()[0].status(), MilestoneStatus::Paid);
    assert_eq!(project.status(), ProjectStatus::Paid);
}

#[test]
fn project_with_foreign_client_v1_still_loads() {
    let project: Project = decode("projects", "fixture", CLIENT_V1.as_bytes()).unwrap();
    assert!(matches!(
        project.client_region(),
        ClientRegion::Abroad { country } if country == "Germany"
    ));
    assert_eq!(project.client_address(), ["Hauptstrasse 1", "10115 Berlin"]);
    let milestone = &project.milestones()[0];
    assert_eq!(milestone.status(), MilestoneStatus::Paid);
    assert_eq!(milestone.invoice_number(), Some("NDC/26-27/001"));
}

#[test]
fn business_profile_v1_still_loads() {
    let profile: BusinessProfile = decode("settings", "fixture", PROFILE_V1.as_bytes()).unwrap();
    assert_eq!(profile.invoice_prefix(), "NDC");
    assert_eq!(profile.sac_code(), "998314");
    assert!(profile.gstin().is_some());
    assert!(profile.lut_arn().is_some());
}

/// v1 invoices are upgraded by the v1 → v2 migration before they load
fn upgrade_v1(json: &str) -> Invoice {
    let v1: serde_json::Value = serde_json::from_str(json).unwrap();
    let v2 = serde_json::to_vec(&invoice_v1_to_v2(v1)).unwrap();
    decode("invoices", "fixture", &v2).unwrap()
}

#[test]
fn invoices_v1_of_every_kind_upgrade_and_load() {
    let tax = upgrade_v1(TAX_INVOICE_V1);
    assert_eq!(tax.kind(), InvoiceKind::Tax);
    assert!(tax.gst().is_some());
    assert_eq!(tax.print().title, "Tax Invoice");
    assert_eq!(tax.status(), InvoiceStatus::Issued);

    let export = upgrade_v1(EXPORT_INVOICE_V1);
    assert_eq!(export.kind(), InvoiceKind::Export);
    assert!(export.terms()[0].contains("export under LUT"));
    assert!(export.is_paid());

    let plain = upgrade_v1(PLAIN_INVOICE_V1);
    assert_eq!(plain.kind(), InvoiceKind::Plain);
    assert!(plain.print().seller.gstin.is_none());
    assert!(plain.is_paid());
    assert_eq!(plain.payments()[0].tds, 4_500);
}

#[test]
fn v1_invoices_are_never_read_without_the_upgrade() {
    // A paid v1 invoice read as v2 would look unpaid; it must be refused instead
    let result = decode::<Invoice>("invoices", "fixture", PLAIN_INVOICE_V1.as_bytes());
    assert!(result.is_err());
}

#[test]
fn expense_v1_still_loads() {
    let expense: Expense = decode("expenses", "fixture", EXPENSE_V1.as_bytes()).unwrap();
    assert_eq!(expense.category(), ExpenseCategory::Equipment);
    assert_eq!(expense.cost(), BigDecimal::from(150_000));
}

#[test]
fn partly_paid_invoice_v2_still_loads() {
    let invoice: Invoice = decode("invoices", "fixture", PARTLY_PAID_V2.as_bytes()).unwrap();
    assert_eq!(invoice.status(), InvoiceStatus::PartlyPaid);
    assert_eq!(invoice.payments()[0].received, 35_400);
    assert_eq!(invoice.balance(), BigDecimal::from(35_400));
}

#[test]
fn records_from_before_currencies_are_in_rupees() {
    let project: Project = decode("projects", "fixture", CLIENT_V1.as_bytes()).unwrap();
    assert_eq!(project.currency(), Currency::Inr);
    let invoice: Invoice = decode("invoices", "fixture", PARTLY_PAID_V2.as_bytes()).unwrap();
    assert_eq!(invoice.currency(), Currency::Inr);
    assert!(invoice.exchange_rate().is_none());
    let profile: BusinessProfile = decode("settings", "fixture", PROFILE_V1.as_bytes()).unwrap();
    assert_eq!(profile.currency(), Currency::Inr);
}

#[test]
fn euro_export_v2_still_loads() {
    let invoice: Invoice = decode("invoices", "fixture", EUR_INVOICE_V2.as_bytes()).unwrap();
    assert_eq!(invoice.currency(), Currency::Eur);
    assert_eq!(
        invoice.exchange_rate().map(|r| r.value().clone()),
        Some(BigDecimal::from(90))
    );
    assert_eq!(invoice.amounts().total, BigDecimal::from(1_000));
    assert_eq!(invoice.inr_amounts().total, BigDecimal::from(90_000));
    assert!(invoice.is_paid());
    assert_eq!(
        invoice.payments()[0].received_inr(),
        &BigDecimal::from(90_450)
    );

    let project: Project = decode("projects", "fixture", EUR_PROJECT_V2.as_bytes()).unwrap();
    assert_eq!(project.currency(), Currency::Eur);
    assert_eq!(project.status(), ProjectStatus::Paid);
}

#[test]
fn credit_note_v3_still_loads() {
    let note: CreditNote = decode("credit_notes", "fixture", CREDIT_NOTE_V3.as_bytes()).unwrap();
    assert_eq!(note.number().as_str(), "CN/26-27/001");
    assert_eq!(note.invoice().as_str(), "INV/26-27/001");
    assert_eq!(note.kind(), InvoiceKind::Tax);
    assert_eq!(note.amounts().igst, BigDecimal::from(1_800));
    assert_eq!(note.reason(), "Discount agreed");
    assert!(note.gst().is_some());
}

#[test]
fn credited_invoice_v2_still_loads() {
    let invoice: Invoice = decode("invoices", "fixture", CREDITED_INVOICE_V3.as_bytes()).unwrap();
    assert_eq!(invoice.credits()[0].number.as_str(), "CN/26-27/001");
    assert_eq!(invoice.credited(), BigDecimal::from(11_800));
    assert_eq!(invoice.balance(), BigDecimal::from(47_200));
    assert_eq!(invoice.status(), InvoiceStatus::Issued);
}

#[test]
fn credited_project_v3_still_loads() {
    let project: Project = decode("projects", "fixture", CREDITED_PROJECT_V3.as_bytes()).unwrap();
    let milestone = &project.milestones()[0];
    assert_eq!(milestone.status(), MilestoneStatus::Credited);
    assert_eq!(milestone.credited(), &BigDecimal::from(50_000));
    assert_eq!(project.paid_amount(), BigDecimal::from(0));
    assert_eq!(project.outstanding_amount(), BigDecimal::from(0));
}

#[test]
fn credits_given_back_on_a_paid_invoice_v4_still_load() {
    let invoice: Invoice = decode("invoices", "fixture", RETURNED_INVOICE_V4.as_bytes()).unwrap();
    assert_eq!(invoice.credited(), BigDecimal::from(35_400));
    assert_eq!(invoice.returned(), BigDecimal::from(35_400));
    assert_eq!(invoice.balance(), BigDecimal::from(0));
    assert_eq!(invoice.status(), InvoiceStatus::Paid);
    assert_eq!(invoice.advance_inr(), BigDecimal::from(23_600));
    let kinds: Vec<_> = invoice
        .credits()
        .iter()
        .filter_map(|c| c.returned.as_ref().map(|r| r.kind))
        .collect();
    assert_eq!(kinds, [ReturnKind::Refunded, ReturnKind::Advance]);
}

#[test]
fn a_foreign_refund_v4_still_loads() {
    let invoice: Invoice =
        decode("invoices", "fixture", REFUNDED_EUR_INVOICE_V4.as_bytes()).unwrap();
    let returned = invoice.credits()[0].returned.as_ref().unwrap();
    assert_eq!(returned.refunded_inr, Some(BigDecimal::from(9_200)));
    assert_eq!(invoice.balance(), BigDecimal::from(0));
    assert_eq!(invoice.creditable(), BigDecimal::from(900));
}

#[test]
fn a_refund_with_tds_reversed_v4_still_loads() {
    let invoice: Invoice =
        decode("invoices", "fixture", TDS_REVERSED_INVOICE_V4.as_bytes()).unwrap();
    let returned = invoice.credits()[0].returned.as_ref().unwrap();
    assert_eq!(returned.tds_reversed, Some(BigDecimal::from(1_000)));
    assert_eq!(returned.cash(), BigDecimal::from(10_800));
    assert_eq!(invoice.balance(), BigDecimal::from(0));
}

#[test]
fn invoice_of_several_milestones_v5_still_loads() {
    let invoice: Invoice = decode("invoices", "fixture", LINES_INVOICE_V5.as_bytes()).unwrap();
    let names: Vec<_> = invoice
        .lines()
        .iter()
        .map(|l| l.milestone_name.as_str())
        .collect();
    assert_eq!(names, ["Design", "Build"]);
    assert_eq!(invoice.amounts().taxable_value, BigDecimal::from(60_000));
    assert_eq!(invoice.print().rows.len(), 2);
    assert_eq!(invoice.credited(), BigDecimal::from(17_700));

    let note: CreditNote =
        decode("credit_notes", "fixture", LINES_CREDIT_NOTE_V5.as_bytes()).unwrap();
    let taken: Vec<_> = note.lines().iter().map(|l| &l.taxable_value).collect();
    assert_eq!(taken, [&BigDecimal::from(5_000), &BigDecimal::from(10_000)]);
    assert_eq!(
        note.lines()[1].milestone_id,
        invoice.lines()[1].milestone_id
    );
    assert_eq!(note.print().rows.len(), 2);
}

#[test]
fn credit_notes_from_before_lines_name_none() {
    let note: CreditNote = decode("credit_notes", "fixture", CREDIT_NOTE_V3.as_bytes()).unwrap();
    assert!(note.lines().is_empty());
}
