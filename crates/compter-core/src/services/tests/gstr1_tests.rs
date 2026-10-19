use bigdecimal::BigDecimal;
use chrono::NaiveDate;
use serde_json::Value;

use super::fixtures::{
    OTHER_STATE_GSTIN, SELLER_GSTIN, client, fixed_project, foreign_client, india, profile_draft,
};
use crate::demo;
use crate::domain::{Currency, ExchangeRate, Quarter};
use crate::services::{CreditDraft, InvoiceService, ReportService, ServiceError};
use crate::storage::{ProjectRepository, Store};

fn quarter(year: i32, month: u32) -> Quarter {
    Quarter::containing(NaiveDate::from_ymd_opt(year, month, 1).unwrap())
}

fn gstr1(store: &Store, q: Quarter) -> Value {
    ReportService::new(store.repository(), None)
        .gstr1(q, &[])
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
    // Billed €1,000 at ₹90: returns are in rupees
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
            .issue(project.id(), project.milestones()[0].id(), day, None)
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
        ReportService::new(store.repository(), None).gstr1(quarter(2026, 7), &[]),
        Err(ServiceError::NotRegistered)
    ));
}

/// A registered seller with a ₹10,000 B2B invoice on 10 July and 10 August 2026; the July one
/// gets a ₹1,000 credit note on 20 August
fn store_with_credited_quarter() -> Store {
    let store = Store::in_memory().unwrap();
    let mut invoices = InvoiceService::new(store.repository(), None);
    invoices
        .save_profile(profile_draft(Some(SELLER_GSTIN), None))
        .unwrap();
    let mut numbers = Vec::new();
    for month in [7, 8] {
        let project = fixed_project(client(Some(OTHER_STATE_GSTIN), india(), &[]), 10_000);
        store.repository().save(&project).unwrap();
        let day = NaiveDate::from_ymd_opt(2026, month, 10).unwrap();
        let issued = invoices
            .issue(project.id(), project.milestones()[0].id(), day, None)
            .unwrap();
        numbers.push(issued.invoice.number().clone());
    }
    let draft = CreditDraft {
        date: NaiveDate::from_ymd_opt(2026, 8, 20).unwrap(),
        taxable_value: BigDecimal::from(1_000),
        reason: "Discount".to_owned(),
        returned: crate::domain::ReturnTerms::default(),
    };
    invoices.issue_credit(&numbers[0], draft).unwrap();
    store
}

#[test]
fn credit_notes_go_to_table_9b_and_13() {
    let store = store_with_credited_quarter();
    let gstr1 = ReportService::new(store.repository(), None)
        .gstr1(quarter(2026, 7), &[])
        .unwrap();
    assert_eq!(gstr1.credit_notes, 1);
    let json = gstr1.json;
    let note = &json["cdnr"][0]["nt"][0];
    assert_eq!(json["cdnr"][0]["ctin"], OTHER_STATE_GSTIN);
    assert_eq!(note["nt_num"], "CN/26-27/001");
    assert_eq!(note["val"], 1_180.0);
    // Net of the credit note
    assert_eq!(json["hsn"]["hsn_b2b"][0]["txval"], 19_000.0);
    let docs = json["doc_issue"]["doc_det"].as_array().unwrap();
    assert_eq!(docs.len(), 2);
    assert_eq!(docs[1]["doc_typ"], "Credit Note");
    assert_eq!(docs[1]["docs"][0]["from"], "CN/26-27/001");
}

#[test]
fn iff_holds_a_months_b2b_and_the_quarter_leaves_it_out() {
    let store = store_with_credited_quarter();
    let reports = ReportService::new(store.repository(), None);
    let iff = reports.iff((2026, 8)).unwrap();
    assert_eq!((iff.invoices, iff.credit_notes), (1, 1));
    assert_eq!(iff.summary(), "1 B2B invoice(s), 1 credit note(s)");
    assert_eq!(iff.json["fp"], "082026");
    assert_eq!(iff.json["b2b"][0]["inv"].as_array().unwrap().len(), 1);
    assert_eq!(iff.json["cdnr"][0]["nt"][0]["nt_num"], "CN/26-27/001");
    assert!(iff.json.get("hsn").is_none());

    let gstr1 = reports.gstr1(quarter(2026, 7), &[(2026, 8)]).unwrap();
    assert_eq!((gstr1.tax_invoices, gstr1.credit_notes), (1, 0));
    assert_eq!(
        gstr1.summary(),
        "1 tax invoice(s), 0 export(s), 0 credit note(s)"
    );
    let b2b = &gstr1.json["b2b"][0]["inv"];
    assert_eq!(b2b.as_array().unwrap().len(), 1);
    assert_eq!(b2b[0]["idt"], "10-07-2026");
    assert!(gstr1.json.get("cdnr").is_none());
    // Table 12 still covers the whole quarter
    assert_eq!(gstr1.json["hsn"]["hsn_b2b"][0]["txval"], 19_000.0);
}

#[test]
fn credit_notes_on_exports_are_cdnur_expwop() {
    let store = Store::in_memory().unwrap();
    let mut invoices = InvoiceService::new(store.repository(), None);
    invoices
        .save_profile(profile_draft(Some(SELLER_GSTIN), Some("AD2904260000001")))
        .unwrap();
    let project = fixed_project(foreign_client(Currency::Eur), 1_000);
    store.repository().save(&project).unwrap();
    let day = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
    let rate = ExchangeRate::new(BigDecimal::from(90)).unwrap();
    let number = invoices
        .issue(project.id(), project.milestones()[0].id(), day, Some(rate))
        .unwrap()
        .invoice
        .number()
        .clone();
    let draft = CreditDraft {
        date: day,
        taxable_value: BigDecimal::from(100),
        reason: "Discount".to_owned(),
        returned: crate::domain::ReturnTerms::default(),
    };
    invoices.issue_credit(&number, draft).unwrap();

    let gstr1 = ReportService::new(store.repository(), None)
        .gstr1(quarter(2026, 7), &[])
        .unwrap();
    assert_eq!(gstr1.credit_notes, 1);
    let note = &gstr1.json["cdnur"][0];
    assert_eq!(note["typ"], "EXPWOP");
    assert_eq!(note["nt_num"], "CN/26-27/001");
    // €100 at the invoice's ₹90
    assert_eq!(note["val"], 9_000.0);
    assert_eq!(gstr1.json["hsn"]["hsn_b2c"][0]["txval"], 81_000.0);
}
