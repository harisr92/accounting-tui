mod support;

use std::path::Path;

use compter_mobile::api::{
    CompterApp, CreditInput, PaymentInput, ProfileInput, ProjectDetail, ProjectInput, TimeInput,
};
use support::{demo, milestone, project};

/// The demo project called `name`
fn open(app: &CompterApp, name: &str) -> ProjectDetail {
    let row = app
        .projects(true)
        .unwrap()
        .into_iter()
        .find(|p| p.name == name)
        .unwrap();
    app.project(row.id).unwrap()
}

/// The id of `project`'s milestone called `name`
fn milestone_id(project: &ProjectDetail, name: &str) -> String {
    project
        .milestones
        .iter()
        .find(|m| m.name == name)
        .unwrap()
        .id
        .clone()
}

/// Issue the demo's pending Website Revamp "Launch" milestone: ₹40,000 + 9% CGST + 9% SGST
fn issue_launch(app: &mut CompterApp) -> String {
    let website = open(app, "Website Revamp");
    let launch = milestone_id(&website, "Launch");
    app.issue_invoice(website.id, launch, None)
        .unwrap()
        .invoice
        .number
}

fn payment(received: &str, tds: &str) -> PaymentInput {
    PaymentInput {
        date: String::new(),
        received: received.to_owned(),
        tds_or_banked: tds.to_owned(),
    }
}

fn credit(amount: &str) -> CreditInput {
    CreditInput {
        date: String::new(),
        amount: amount.to_owned(),
        reason: "Discount agreed".to_owned(),
    }
}

#[test]
fn a_new_business_starts_from_the_usual_profile_values() {
    let dir = tempfile::tempdir().unwrap();
    let app = CompterApp::open(dir.path().to_string_lossy().into_owned()).unwrap();

    assert_eq!(app.profile().unwrap(), None);
    let input = app.profile_input().unwrap();
    assert_eq!(input.sac_code, "998314");
    assert_eq!(input.invoice_prefix, "INV");
    assert_eq!(input.currency, "INR");
}

#[test]
fn saving_the_profile_drops_blank_lines_and_names_a_bad_currency() {
    let (_scratch, mut app) = demo();
    let input = ProfileInput {
        address: vec!["12 FC Road".to_owned(), " ".to_owned()],
        terms: vec![String::new(), "Payment due within 15 days".to_owned()],
        ..app.profile_input().unwrap()
    };

    let wrong = app
        .save_profile(ProfileInput {
            currency: "JPY".to_owned(),
            ..input.clone()
        })
        .unwrap_err();
    assert_eq!(wrong.field.as_deref(), Some("currency"));

    let saved = app.save_profile(input).unwrap();
    assert_eq!(saved.address, ["12 FC Road"]);
    assert_eq!(saved.terms, ["Payment due within 15 days"]);
    assert_eq!(saved.gstin.as_deref(), Some("27AAPFU0939F1ZV"));
    assert!(saved.sac_description.is_some());
}

#[test]
fn issuing_shows_the_invoice_first_then_writes_it_and_its_pdf() {
    let (_scratch, mut app) = demo();
    let website = open(&app, "Website Revamp");
    let launch = milestone_id(&website, "Launch");

    let preview = app
        .preview_invoice(website.id.clone(), launch.clone(), None)
        .unwrap();
    assert_eq!(preview.kind, "TAX");
    assert_eq!(preview.total, "₹47,200.00");
    assert_eq!(
        preview.breakdown,
        "40,000.00 + CGST 3,600.00 + SGST 3,600.00"
    );
    assert_eq!(preview.total_inr, None);

    let issued = app.issue_invoice(website.id, launch.clone(), None).unwrap();

    assert_eq!(issued.invoice.number, preview.number);
    assert_eq!(issued.pdf_error, None);
    let billed = issued
        .project
        .milestones
        .iter()
        .find(|m| m.id == launch)
        .unwrap();
    assert_eq!(billed.status, "INVOICED");
    assert_eq!(
        billed.invoice_number.as_deref(),
        Some(preview.number.as_str())
    );
    let listed = app.invoices().unwrap();
    assert_eq!(listed.invoices[0].number, preview.number);
    assert_eq!(listed.invoices[0].due.as_deref(), Some("₹47,200.00"));
    let pdf = app.invoice_pdf(preview.number).unwrap();
    assert!(pdf.ends_with(".pdf") && Path::new(&pdf).exists(), "{pdf}");
}

#[test]
fn payments_may_come_in_parts_with_tds() {
    let (_scratch, mut app) = demo();
    let number = issue_launch(&mut app);
    let invoice = app.invoice(number.clone()).unwrap();
    assert_eq!(invoice.balance_value, "47200.00");
    assert_eq!(invoice.usual_tds.as_deref(), Some("₹4,000.00"));

    let part = app
        .record_payment(number.clone(), payment("20,000", "0"))
        .unwrap();
    assert_eq!(part.status, "PART PAID");
    assert_eq!(part.balance, "₹27,200.00");

    let rest = app
        .record_payment(number.clone(), payment("23,200", "4,000"))
        .unwrap();
    assert_eq!(rest.status, "PAID");
    assert!(rest.settled);
    assert_eq!(rest.history.len(), 2);
    assert_eq!(rest.history[1].tds_or_banked.as_deref(), Some("₹4,000.00"));
    let launch = open(&app, "Website Revamp").milestones[2].clone();
    assert_eq!(launch.status, "PAID");
}

#[test]
fn a_refused_payment_names_its_field_and_records_nothing() {
    let (_scratch, mut app) = demo();
    let number = issue_launch(&mut app);

    let bad_amount = app
        .record_payment(number.clone(), payment("lots", "0"))
        .unwrap_err();
    assert_eq!(bad_amount.field.as_deref(), Some("received"));
    let bad_date = app
        .record_payment(
            number.clone(),
            PaymentInput {
                date: "18/10/2026".to_owned(),
                ..payment("100", "0")
            },
        )
        .unwrap_err();
    assert_eq!(bad_date.field.as_deref(), Some("date"));
    let too_much = app
        .record_payment(number.clone(), payment("1,00,000", "0"))
        .unwrap_err();
    assert_eq!(too_much.field, None);

    assert!(app.invoice(number).unwrap().history.is_empty());
}

#[test]
fn a_credit_note_for_the_whole_invoice_cancels_it() {
    let (_scratch, mut app) = demo();
    let number = issue_launch(&mut app);
    let invoice = app.invoice(number.clone()).unwrap();
    let whole = invoice.credit_value.unwrap();
    assert_eq!(whole, "40000.00");

    let preview = app.preview_credit(number.clone(), credit(&whole)).unwrap();
    assert_eq!(preview.total, "₹47,200.00");
    assert_eq!(preview.gst.as_deref(), Some("₹7,200.00"));
    assert_eq!(preview.balance_after, "₹0.00");

    let credited = app.issue_credit(number, credit(&whole)).unwrap();

    assert_eq!(credited.note_number, preview.number);
    assert_eq!(credited.pdf_error, None);
    assert_eq!(credited.invoice.status, "CREDITED");
    assert!(credited.invoice.history[0].credit_note);
    let pdf = app.credit_note_pdf(credited.note_number).unwrap();
    assert!(Path::new(&pdf).exists(), "{pdf}");
}

#[test]
fn a_client_abroad_is_invoiced_at_the_rate_typed() {
    let (_scratch, mut app) = demo();
    let created = app
        .create_project(ProjectInput {
            client_state: None,
            client_country: Some("Germany".to_owned()),
            currency: Some("EUR".to_owned()),
            ..project("Translation", None)
        })
        .unwrap();
    assert_eq!(created.currency, "EUR");
    assert_eq!(created.client_location.as_deref(), Some("Germany"));
    let detail = app
        .add_milestone(created.id.clone(), milestone("Draft", "", Some("1,000")))
        .unwrap();
    let draft = detail.milestones[0].id.clone();

    let bad_rate = app
        .preview_invoice(created.id.clone(), draft.clone(), Some("ninety".to_owned()))
        .unwrap_err();
    assert_eq!(bad_rate.field.as_deref(), Some("rate"));
    let preview = app
        .preview_invoice(created.id, draft, Some("90".to_owned()))
        .unwrap();
    assert_eq!(preview.kind, "EXPORT");
    assert_eq!(preview.total, "€1,000.00");
    assert_eq!(preview.total_inr.as_deref(), Some("₹90,000.00"));
}

#[test]
fn a_client_in_india_needs_a_real_state_code() {
    let (_scratch, mut app) = demo();
    let created = app.create_project(project("Website", None)).unwrap();
    assert_eq!(created.client_location.as_deref(), Some("Karnataka (29)"));

    let wrong = app
        .create_project(ProjectInput {
            client_state: Some("99".to_owned()),
            ..project("Website", None)
        })
        .unwrap_err();
    assert_eq!(wrong.field.as_deref(), Some("client_state"));
}

#[test]
fn time_is_logged_on_hourly_milestones() {
    let (_scratch, mut app) = demo();
    let mobile = open(&app, "Mobile App");
    let beta = milestone_id(&mobile, "Beta");
    let time = |duration: &str| TimeInput {
        date: String::new(),
        duration: duration.to_owned(),
        note: "Push notifications".to_owned(),
    };

    let wrong = app
        .log_time(mobile.id.clone(), beta.clone(), time("soon"))
        .unwrap_err();
    assert_eq!(wrong.field.as_deref(), Some("duration"));
    let logged = app
        .log_time(mobile.id, beta.clone(), time("1h30m"))
        .unwrap();

    let beta = logged.milestones.iter().find(|m| m.id == beta).unwrap();
    assert_eq!(beta.logged.as_deref(), Some("3h 30m"));
    assert_eq!(beta.amount, "₹5,250.00");
}

#[test]
fn a_project_lists_its_milestones_in_every_order() {
    let (_scratch, app) = demo();
    let website = open(&app, "Website Revamp");
    let names = |order: &[u32]| -> Vec<String> {
        order
            .iter()
            .map(|&i| website.milestones[i as usize].name.clone())
            .collect()
    };
    assert_eq!(names(&website.by_due), ["Design", "Frontend", "Launch"]);
    assert_eq!(names(&website.by_name), ["Design", "Frontend", "Launch"]);
    assert_eq!(names(&website.by_amount), ["Frontend", "Design", "Launch"]);
}
