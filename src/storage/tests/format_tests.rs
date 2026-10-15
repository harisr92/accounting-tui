//! Records saved by earlier builds must keep loading. If one of these tests fails, the stored
//! format changed: add `#[serde(default)]` or a migration (see `storage/migrations.rs`).

use bigdecimal::BigDecimal;

use crate::domain::{
    Billing, BusinessProfile, ClientRegion, Invoice, InvoiceKind, InvoiceStatus, MilestoneStatus,
    Project, ProjectStatus,
};
use crate::storage::codec::decode;

const HOURLY_V1: &str = include_str!("fixtures/project_v1.json");
const FIXED_V1: &str = include_str!("fixtures/project_fixed_v1.json");
const CLIENT_V1: &str = include_str!("fixtures/project_client_v1.json");
const PROFILE_V1: &str = include_str!("fixtures/business_profile_v1.json");
const TAX_INVOICE_V1: &str = include_str!("fixtures/invoice_tax_v1.json");
const EXPORT_INVOICE_V1: &str = include_str!("fixtures/invoice_export_v1.json");
const PLAIN_INVOICE_V1: &str = include_str!("fixtures/invoice_plain_v1.json");

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

#[test]
fn invoices_v1_of_every_kind_still_load() {
    let load = |json: &str| -> Invoice { decode("invoices", "fixture", json.as_bytes()).unwrap() };

    let tax = load(TAX_INVOICE_V1);
    assert_eq!(tax.kind(), InvoiceKind::Tax);
    assert!(tax.gst().is_some());
    assert_eq!(tax.print().title, "Tax Invoice");

    let export = load(EXPORT_INVOICE_V1);
    assert_eq!(export.kind(), InvoiceKind::Export);
    assert!(export.terms()[0].contains("export under LUT"));
    assert!(matches!(export.status(), InvoiceStatus::Paid { .. }));

    let plain = load(PLAIN_INVOICE_V1);
    assert_eq!(plain.kind(), InvoiceKind::Plain);
    assert!(plain.print().seller.gstin.is_none());
    assert!(matches!(plain.status(), InvoiceStatus::Paid { tds, .. } if *tds == 4_500));
}
