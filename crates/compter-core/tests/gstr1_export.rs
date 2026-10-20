//! The quarterly GSTR-1 is written as portal JSON under the data folder

use bigdecimal::BigDecimal;
use chrono::NaiveDate;
use compter_core::domain::{Billing, ClientDetails, ProfileDraft, Project, Quarter};
use compter_core::services::{InvoiceService, ReportService};
use compter_core::storage::{DataDir, ProjectRepository, Store};

#[test]
fn quarterly_gstr1_json_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = DataDir::new(dir.path());
    let store = Store::open(&data_dir).unwrap();
    let mut invoices = InvoiceService::new(store.repository(), None);
    invoices
        .save_profile(ProfileDraft {
            name: "Studio".to_owned(),
            address: vec!["Pune".to_owned()],
            gstin: Some("27AAPFU0939F1ZV".to_owned()),
            sac_code: "998314".to_owned(),
            invoice_prefix: "INV".to_owned(),
            lut_arn: None,
            terms: Vec::new(),
            ..Default::default()
        })
        .unwrap();
    let date = NaiveDate::from_ymd_opt(2026, 8, 10).unwrap();
    let client = ClientDetails::named("Acme", Some("29AAPFU0939F1ZR"));
    let mut project = Project::for_client("Site", client, Billing::Fixed).unwrap();
    project
        .add_milestone("Design", date, Some(BigDecimal::from(50_000)))
        .unwrap();
    store.repository().save(&project).unwrap();
    invoices
        .issue(project.id(), &[project.milestones()[0].id()], date, None)
        .unwrap();

    let reports = ReportService::new(store.repository(), Some(data_dir));
    let (path, gstr1) = reports
        .export_gstr1(Quarter::containing(date), &[])
        .unwrap();
    assert_eq!(
        path,
        dir.path().join("returns/2026-27/GSTR1-Q2-2026-27.json")
    );
    assert_eq!(gstr1.tax_invoices, 1);
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(json["fp"], "092026");
    assert_eq!(json["b2b"][0]["ctin"], "29AAPFU0939F1ZR");
    assert_eq!(json["b2b"][0]["inv"][0]["val"], 59_000.0);
}
