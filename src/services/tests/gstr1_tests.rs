use chrono::NaiveDate;
use serde_json::Value;

use super::fixtures::{
    OTHER_STATE_GSTIN, SELLER_GSTIN, client, fixed_project, india, profile_draft,
};
use crate::demo;
use crate::domain::Quarter;
use crate::services::{InvoiceService, ReportService, ServiceError};
use crate::storage::{ProjectRepository, Store};

fn quarter(year: i32, month: u32) -> Quarter {
    Quarter::containing(NaiveDate::from_ymd_opt(year, month, 1).unwrap())
}

fn gstr1(store: &Store, q: Quarter) -> Value {
    ReportService::new(store.repository(), None)
        .gstr1(q)
        .unwrap()
        .json
}

#[test]
fn export_quarter_has_table_6a_and_table_13() {
    let store = demo::store().unwrap();
    let json = gstr1(&store, quarter(2026, 6));
    assert_eq!(json["fp"], "062026");
    assert_eq!(json["gstin"], SELLER_GSTIN);
    let export = &json["exp"][0];
    assert_eq!(export["exp_typ"], "WOPAY");
    assert_eq!(export["inv"][0]["inum"], "NDC/26-27/001");
    assert_eq!(export["inv"][0]["val"], 90_000.0);
    assert_eq!(export["inv"][0]["itms"][0]["rt"], 0);
    assert!(json.get("b2b").is_none());
    let docs = &json["doc_issue"]["doc_det"][0]["docs"][0];
    assert_eq!(
        (&docs["from"], &docs["to"], &docs["totnum"]),
        (
            &Value::from("NDC/26-27/001"),
            &Value::from("NDC/26-27/001"),
            &Value::from(1)
        )
    );
    assert_eq!(json["hsn"]["hsn_b2c"][0]["txval"], 90_000.0);
}

#[test]
fn quarter_sorts_b2b_and_b2cs() {
    let store = demo::store().unwrap();
    let json = gstr1(&store, quarter(2026, 11));
    assert_eq!(json["fp"], "122026");
    assert_eq!(json["b2b"].as_array().unwrap().len(), 1);
    assert_eq!(json["b2b"][0]["inv"][0]["inum"], "NDC/26-27/004");
    // The hourly prototype went to an unregistered client in Karnataka
    assert_eq!(json["b2cs"][0]["pos"], "29");
    assert_eq!(json["b2cs"][0]["txval"], 15_750.0);
    let docs = &json["doc_issue"]["doc_det"][0]["docs"][0];
    assert_eq!(
        (&docs["from"], &docs["to"], &docs["totnum"]),
        (
            &Value::from("NDC/26-27/004"),
            &Value::from("NDC/26-27/005"),
            &Value::from(2)
        )
    );
}

#[test]
fn months_of_a_quarter_merge_per_buyer() {
    let store = Store::in_memory().unwrap();
    let mut invoices = InvoiceService::new(store.repository(), None);
    invoices
        .save_profile(profile_draft(Some(SELLER_GSTIN), None))
        .unwrap();
    for (month, amount) in [(7, 10_000), (8, 20_000)] {
        let project = fixed_project(client(Some(OTHER_STATE_GSTIN), india(), &[]), amount);
        store.repository().save(&project).unwrap();
        let day = NaiveDate::from_ymd_opt(2026, month, 10).unwrap();
        invoices
            .issue(project.id(), project.milestones()[0].id(), day)
            .unwrap();
    }
    let json = gstr1(&store, quarter(2026, 7));
    assert_eq!(json["b2b"].as_array().unwrap().len(), 1);
    assert_eq!(json["b2b"][0]["inv"].as_array().unwrap().len(), 2);
    let hsn = &json["hsn"]["hsn_b2b"];
    assert_eq!(hsn.as_array().unwrap().len(), 1);
    assert_eq!(hsn[0]["txval"], 30_000.0);
    assert_eq!(hsn[0]["iamt"], 5_400.0);
    assert_eq!(json["doc_issue"]["doc_det"][0]["docs"][0]["totnum"], 2);
}

#[test]
fn unregistered_sellers_have_no_gstr1() {
    let store = Store::in_memory().unwrap();
    InvoiceService::new(store.repository(), None)
        .save_profile(profile_draft(None, None))
        .unwrap();
    assert!(matches!(
        ReportService::new(store.repository(), None).gstr1(quarter(2026, 7)),
        Err(ServiceError::NotRegistered)
    ));
}
