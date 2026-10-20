mod support;

use std::path::Path;

use compter_mobile::api::{
    CompterApp, CreditInput, CreditLineInput, PaymentInput, ProfileInput, ProjectDetail,
    ProjectInput, TimeInput,
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
    app.issue_invoice(website.id, vec![launch], None, None)
        .unwrap()
        .invoice
        .number
}

/// The milestone billed on invoice `number`'s first line
fn first_line(app: &CompterApp, number: &str) -> String {
    app.invoice(number.to_owned()).unwrap().lines[0]
        .milestone_id
        .clone()
}

fn payment(received: &str, tds: &str) -> PaymentInput {
    PaymentInput {
        date: String::new(),
        received: received.to_owned(),
        tds_or_banked: tds.to_owned(),
    }
}

/// A note taking `amount` before GST off the invoice line billing milestone `line`
fn credit(line: &str, amount: &str) -> CreditInput {
    CreditInput {
        date: String::new(),
        lines: vec![CreditLineInput {
            milestone_id: line.to_owned(),
            amount: amount.to_owned(),
        }],
        reason: "Discount agreed".to_owned(),
        keep_as_advance: false,
        refunded_inr: String::new(),
        tds_reversed: String::new(),
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
        .preview_invoice(website.id.clone(), vec![launch.clone()], None, None)
        .unwrap();
    assert_eq!(preview.kind, "TAX");
    assert_eq!(preview.total, "₹47,200.00");
    assert_eq!(
        preview.breakdown,
        "40,000.00 + CGST 3,600.00 + SGST 3,600.00"
    );
    assert_eq!(preview.total_inr, None);

    let issued = app
        .issue_invoice(website.id, vec![launch.clone()], None, None)
        .unwrap();

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
    let line = invoice.lines[0].milestone_id.clone();
    let whole = invoice.lines[0].credit_value.clone().unwrap();
    assert_eq!(whole, "40000.00");

    let preview = app
        .preview_credit(number.clone(), credit(&line, &whole))
        .unwrap();
    assert_eq!(preview.total, "₹47,200.00");
    assert_eq!(preview.gst.as_deref(), Some("₹7,200.00"));
    assert_eq!(preview.balance_after, "₹0.00");

    let credited = app.issue_credit(number, credit(&line, &whole)).unwrap();

    assert_eq!(credited.note_number, preview.number);
    assert_eq!(credited.pdf_error, None);
    assert_eq!(credited.invoice.status, "CREDITED");
    assert!(credited.invoice.history[0].credit_note);
    let pdf = app.credit_note_pdf(credited.note_number).unwrap();
    assert!(Path::new(&pdf).exists(), "{pdf}");
}

#[test]
fn a_paid_invoice_can_be_credited_with_a_refund_or_an_advance() {
    let (_scratch, mut app) = demo();
    let number = issue_launch(&mut app);
    let line = first_line(&app, &number);
    let paid = app
        .record_payment(number.clone(), payment("47,200", "0"))
        .unwrap();
    assert!(paid.settled);
    assert!(paid.creditable);
    assert_eq!(paid.client_advance, None);

    // ₹10,000 + 18% GST, all of it already paid
    let preview = app
        .preview_credit(number.clone(), credit(&line, "10000"))
        .unwrap();
    assert_eq!(
        preview.returned.as_deref(),
        Some("₹11,800.00 already paid will be refunded")
    );
    let refunded = app
        .issue_credit(number.clone(), credit(&line, "10000"))
        .unwrap();
    assert_eq!(refunded.invoice.status, "PAID");
    assert_eq!(
        refunded.invoice.history[1].returned.as_deref(),
        Some("₹11,800.00 refunded")
    );

    let advance = CreditInput {
        keep_as_advance: true,
        ..credit(&line, "5000")
    };
    let kept = app.issue_credit(number.clone(), advance).unwrap().invoice;
    assert_eq!(
        kept.history[2].returned.as_deref(),
        Some("₹5,900.00 kept as advance")
    );
    assert_eq!(kept.client_advance.as_deref(), Some("₹5,900.00"));

    // Together the notes can't take off more than the line's ₹40,000: ₹25,000 is left
    let too_much = app
        .issue_credit(number.clone(), credit(&line, "30000"))
        .unwrap_err();
    assert_eq!(too_much.field, Some(format!("line:{line}")));
    assert!(
        too_much.message.contains("25000.00"),
        "{}",
        too_much.message
    );
    let rest = app
        .issue_credit(number.clone(), credit(&line, "25000"))
        .unwrap();
    assert!(!rest.invoice.creditable);
}

#[test]
fn a_refund_after_tds_names_the_tds_the_client_reverses() {
    let (_scratch, mut app) = demo();
    // Website Revamp "Design": ₹59,000, paid ₹54,000 with ₹5,000 TDS
    let number = app
        .invoices()
        .unwrap()
        .invoices
        .into_iter()
        .find(|i| i.milestones == "Design")
        .unwrap()
        .number;
    let line = first_line(&app, &number);
    assert!(app.invoice(number.clone()).unwrap().tds_withheld);

    let all_cash = app
        .issue_credit(number.clone(), credit(&line, "50000"))
        .unwrap_err();
    assert!(
        all_cash.message.contains("received"),
        "{}",
        all_cash.message
    );
    let bad = CreditInput {
        tds_reversed: "some".to_owned(),
        ..credit(&line, "50000")
    };
    assert_eq!(
        app.issue_credit(number.clone(), bad)
            .unwrap_err()
            .field
            .as_deref(),
        Some("tds_reversed")
    );

    let reversed = CreditInput {
        tds_reversed: "5,000".to_owned(),
        ..credit(&line, "50000")
    };
    let preview = app
        .preview_credit(number.clone(), reversed.clone())
        .unwrap();
    assert_eq!(
        preview.returned.as_deref(),
        Some("₹59,000.00 already paid will be refunded (₹54,000.00 cash, ₹5,000.00 TDS reversed)")
    );
    let credited = app.issue_credit(number, reversed).unwrap();
    assert!(!credited.invoice.creditable);
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
        .preview_invoice(
            created.id.clone(),
            vec![draft.clone()],
            Some("ninety".to_owned()),
            None,
        )
        .unwrap_err();
    assert_eq!(bad_rate.field.as_deref(), Some("rate"));
    let preview = app
        .preview_invoice(created.id, vec![draft], Some("90".to_owned()), None)
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

#[test]
fn several_milestones_go_on_one_invoice_and_credit_notes_pick_its_lines() {
    let (_scratch, mut app) = demo();
    // An unregistered client in Karnataka: IGST
    let created = app.create_project(project("Brochure", None)).unwrap();
    app.add_milestone(
        created.id.clone(),
        milestone("Copy", "2026-10-01", Some("10,000")),
    )
    .unwrap();
    let detail = app
        .add_milestone(
            created.id.clone(),
            milestone("Print", "2026-10-05", Some("5,000")),
        )
        .unwrap();
    let ids: Vec<String> = detail.milestones.iter().map(|m| m.id.clone()).collect();

    let preview = app
        .preview_invoice(created.id.clone(), ids.clone(), None, None)
        .unwrap();
    assert_eq!(preview.total, "₹17,700.00");
    let project_id = created.id.clone();
    let issued = app
        .issue_invoice(created.id, ids.clone(), None, None)
        .unwrap();
    assert!(
        issued
            .project
            .milestones
            .iter()
            .all(|m| m.status == "INVOICED")
    );
    let invoice = issued.invoice;
    assert_eq!(invoice.milestones, "Copy, Print");
    let names: Vec<_> = invoice.lines.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["Copy", "Print"]);
    assert_eq!(invoice.lines[1].amount, "₹5,000.00");
    assert_eq!(invoice.lines[1].credit_value.as_deref(), Some("5000.00"));
    let listed = app.invoices().unwrap().invoices;
    assert_eq!(listed[0].milestones, "Copy, Print");

    // A bad amount names its line
    let bad = CreditInput {
        lines: vec![
            CreditLineInput {
                milestone_id: ids[0].clone(),
                amount: "1,000".to_owned(),
            },
            CreditLineInput {
                milestone_id: ids[1].clone(),
                amount: "lots".to_owned(),
            },
        ],
        ..credit(&ids[0], "")
    };
    let error = app.preview_credit(invoice.number.clone(), bad).unwrap_err();
    assert_eq!(error.field, Some(format!("line:{}", ids[1])));

    // Lines are checked while they can still be changed: more than is left names the line
    let over = app
        .check_credit_lines(
            invoice.number.clone(),
            vec![CreditLineInput {
                milestone_id: ids[1].clone(),
                amount: "5,000.01".to_owned(),
            }],
        )
        .unwrap_err();
    assert_eq!(over.field, Some(format!("line:{}", ids[1])));
    assert!(over.message.contains("left to credit"), "{}", over.message);
    app.check_credit_lines(invoice.number.clone(), credit(&ids[1], "5000").lines)
        .unwrap();
    // A fraction of a paisa is a mistyped amount, refused rather than rounded
    let fraction = app
        .check_credit_lines(invoice.number.clone(), credit(&ids[1], "5000.004").lines)
        .unwrap_err();
    assert_eq!(fraction.field, Some(format!("line:{}", ids[1])));
    // Previewing or issuing checks the lines too, naming the one refused
    let preview = app
        .preview_credit(invoice.number.clone(), credit(&ids[1], "6000"))
        .unwrap_err();
    assert_eq!(preview.field, Some(format!("line:{}", ids[1])));
    assert!(
        preview.message.starts_with("Print: "),
        "{}",
        preview.message
    );

    // Print in full, then what is left on Copy
    let credited = app
        .issue_credit(invoice.number.clone(), credit(&ids[1], "5000"))
        .unwrap();
    let lines = &credited.invoice.lines;
    assert!(!lines[1].can_credit);
    assert_eq!(lines[1].creditable, "₹0.00");
    // Once anything is credited, amounts are left to type
    assert!(lines[0].can_credit);
    assert_eq!(lines[0].credit_value, None);
    assert_eq!(lines[0].left_value, "10000.00");
    assert_eq!(
        credited.invoice.credit_guide,
        "₹11,800.00 due · up to ₹11,800.00 incl. GST can be credited"
    );
    // Print, taken back in full, is credited while Copy is still due
    let statuses: Vec<_> = app
        .project(project_id.clone())
        .unwrap()
        .milestones
        .iter()
        .map(|m| (m.name.clone(), m.status.clone()))
        .collect();
    assert_eq!(
        statuses,
        [
            ("Copy".to_owned(), "INVOICED".to_owned()),
            ("Print".to_owned(), "CREDITED".to_owned())
        ]
    );
    let cancelled = app
        .issue_credit(invoice.number.clone(), credit(&ids[0], "10000"))
        .unwrap();
    assert_eq!(cancelled.invoice.status, "CREDITED");
}

#[test]
fn a_discount_comes_off_before_gst_and_shows_on_the_invoice() {
    let (_scratch, mut app) = demo();
    let website = open(&app, "Website Revamp");
    let launch = milestone_id(&website, "Launch");

    let refused = app
        .preview_invoice(
            website.id.clone(),
            vec![launch.clone()],
            None,
            Some("100%".to_owned()),
        )
        .unwrap_err();
    assert_eq!(refused.field.as_deref(), Some("discount"));
    let refused = app
        .preview_invoice(
            website.id.clone(),
            vec![launch.clone()],
            None,
            Some("lots".to_owned()),
        )
        .unwrap_err();
    assert_eq!(refused.field.as_deref(), Some("discount"));

    let preview = app
        .preview_invoice(
            website.id.clone(),
            vec![launch.clone()],
            None,
            Some("10%".to_owned()),
        )
        .unwrap();
    assert_eq!(preview.total, "₹42,480.00");
    assert_eq!(preview.discount.as_deref(), Some("₹4,000.00"));

    let issued = app
        .issue_invoice(website.id, vec![launch], None, Some("10%".to_owned()))
        .unwrap();
    assert_eq!(issued.invoice.discount.as_deref(), Some("₹4,000.00"));
    assert_eq!(issued.invoice.balance, "₹42,480.00");
}
