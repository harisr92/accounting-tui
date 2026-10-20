//! Issuing writes the invoice PDF under the data folder, and `ensure_pdf` restores a lost one

use bigdecimal::BigDecimal;
use chrono::NaiveDate;
use compter_core::domain::{Billing, ClientDetails, ProfileDraft, Project};
use compter_core::services::InvoiceService;
use compter_core::storage::{DataDir, ProjectRepository, Store};

#[test]
fn issued_invoice_pdf_is_written_and_restored() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = DataDir::new(dir.path());
    let store = Store::open(&data_dir).unwrap();
    let mut service = InvoiceService::new(store.repository(), Some(data_dir.clone()));
    service
        .save_profile(ProfileDraft {
            name: "Studio".to_owned(),
            address: vec!["Pune".to_owned()],
            gstin: Some("27AAPFU0939F1ZV".to_owned()),
            sac_code: "998314".to_owned(),
            invoice_prefix: "INV".to_owned(),
            lut_arn: None,
            terms: vec!["Bank: HDFC 0001234".to_owned()],
            ..Default::default()
        })
        .unwrap();

    let date = NaiveDate::from_ymd_opt(2026, 10, 15).unwrap();
    let client = ClientDetails::named("Acme", Some("29AAPFU0939F1ZR"));
    let mut project = Project::for_client("Site", client, Billing::Fixed).unwrap();
    project
        .add_milestone("Design", date, Some(BigDecimal::from(50_000)))
        .unwrap();
    store.repository().save(&project).unwrap();

    let issued = service
        .issue(project.id(), &[project.milestones()[0].id()], date, None)
        .unwrap();
    let path = issued.pdf.unwrap().unwrap();
    assert_eq!(path, dir.path().join("invoices/2026-27/INV-26-27-001.pdf"));
    let bytes = std::fs::read(&path).unwrap();
    assert!(bytes.starts_with(b"%PDF"));
    assert!(!path.with_extension("tmp").exists());

    std::fs::remove_file(&path).unwrap();
    let restored = service.ensure_pdf(issued.invoice.number()).unwrap();
    assert_eq!(restored, path);
    assert_eq!(std::fs::read(&path).unwrap().len(), bytes.len());
}
