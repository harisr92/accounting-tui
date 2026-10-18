//! Shared builders for invoicing tests

use bigdecimal::BigDecimal;
use chrono::NaiveDate;

use crate::domain::{
    Billing, BusinessProfile, ClientDetails, ClientRegion, Currency, ProfileDraft, Project,
    TimeEntry,
};

/// Maharashtra, the seller's state in these tests
pub const SELLER_GSTIN: &str = "27AAPFU0939F1ZV";
/// Another Maharashtra registration: intra-state
pub const SAME_STATE_GSTIN: &str = "27AAPFU0939F2ZU";
/// Karnataka: inter-state
pub const OTHER_STATE_GSTIN: &str = "29AAPFU0939F1ZR";

pub fn date() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, 15).unwrap()
}

pub fn profile_draft(gstin: Option<&str>, lut: Option<&str>) -> ProfileDraft {
    ProfileDraft {
        name: "Harin Dev Studio".to_owned(),
        address: vec!["12 MG Road".to_owned(), "Pune 411001".to_owned()],
        gstin: gstin.map(str::to_owned),
        sac_code: "998314".to_owned(),
        invoice_prefix: "INV".to_owned(),
        lut_arn: lut.map(str::to_owned),
        terms: vec!["Bank: HDFC 0001234, IFSC HDFC0000001".to_owned()],
        ..Default::default()
    }
}

pub fn profile(gstin: Option<&str>, lut: Option<&str>) -> BusinessProfile {
    BusinessProfile::try_from(profile_draft(gstin, lut)).unwrap()
}

pub fn client(gstin: Option<&str>, region: ClientRegion, address: &[&str]) -> ClientDetails {
    ClientDetails {
        name: "Acme".to_owned(),
        gstin: gstin.map(str::to_owned),
        address: address.iter().map(|line| (*line).to_owned()).collect(),
        region,
        ..ClientDetails::default()
    }
}

/// A client in Germany billed in `currency`, with an address
pub fn foreign_client(currency: Currency) -> ClientDetails {
    ClientDetails {
        currency,
        ..client(None, abroad(), &["Hauptstr. 1", "10115 Berlin"])
    }
}

pub fn india() -> ClientRegion {
    ClientRegion::India { state: None }
}

pub fn abroad() -> ClientRegion {
    ClientRegion::Abroad {
        country: "Germany".to_owned(),
    }
}

/// A fixed-price project with one pending ₹`amount` milestone
pub fn fixed_project(client: ClientDetails, amount: i64) -> Project {
    let mut project = Project::for_client("Website", client, Billing::Fixed).unwrap();
    project
        .add_milestone("Design", date(), Some(BigDecimal::from(amount)))
        .unwrap();
    project
}

/// An hourly project at ₹1,500/h with 10h 30m logged on its one milestone
pub fn hourly_project(client: ClientDetails) -> Project {
    let billing = Billing::hourly(BigDecimal::from(1500)).unwrap();
    let mut project = Project::for_client("App", client, billing).unwrap();
    project.add_milestone("Prototype", date(), None).unwrap();
    project
        .log_time(0, TimeEntry::new(date(), 630, "build").unwrap())
        .unwrap();
    project
}

/// A 2x1 pixel RGB PNG that accounting-core can draw
pub const LOGO_PNG: [u8; 70] = [
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x7B, 0x40, 0xE8,
    0xDD, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x38, 0x21, 0x07, 0x04,
    0x27, 0x00, 0x07, 0x23, 0x02, 0x09, 0xE4, 0x58, 0x88, 0x39, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
];
