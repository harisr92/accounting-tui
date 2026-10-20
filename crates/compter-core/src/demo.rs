//! Sample data: a full in-memory business for `compter --demo`, and fixture projects for tests

use bigdecimal::BigDecimal;
use chrono::NaiveDate;

use accounting_core::StateCode;

use crate::domain::{
    Billing, ClientDetails, ClientRegion, Currency, DEFAULT_SAC, DomainError, ExchangeRate,
    ExpenseCategory, MilestoneId, ProfileDraft, Project, TimeEntry,
};
use crate::services::{ExpenseDraft, ExpenseService, InvoiceService, ServiceError};
use crate::storage::{ProjectRepository, Store};

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).unwrap_or_default()
}

fn rupees(amount: i64) -> Option<BigDecimal> {
    Some(BigDecimal::from(amount))
}

/// Fixture projects with milestones in every state; invoice numbers are placeholders
pub fn projects() -> Result<Vec<Project>, DomainError> {
    let mut website = Project::new(
        "Website Revamp",
        "Acme Pvt Ltd",
        Some("27AAPFU0939F2ZU"),
        Billing::Fixed,
    )?;
    website.add_milestone("Design", date(2026, 9, 1), rupees(50_000))?;
    website.add_milestone("Frontend", date(2026, 10, 20), rupees(60_000))?;
    website.add_milestone("Launch", date(2026, 11, 15), rupees(40_000))?;
    website.invoice_milestone(0, "DEMO-0")?;
    website.record_payment(0)?;
    website.invoice_milestone(1, "DEMO-1")?;

    let mut app = Project::new(
        "Mobile App",
        "Globex Labs",
        None,
        Billing::hourly(BigDecimal::from(1_500))?,
    )?;
    app.add_milestone("Prototype", date(2026, 10, 30), None)?;
    app.add_milestone("Beta", date(2026, 12, 15), None)?;
    app.log_time(0, TimeEntry::new(date(2026, 10, 6), 360, "Wireframes")?)?;
    app.log_time(
        0,
        TimeEntry::new(date(2026, 10, 8), 270, "Navigation shell")?,
    )?;
    app.log_time(1, TimeEntry::new(date(2026, 10, 13), 120, "Auth spike")?)?;
    app.invoice_milestone(0, "DEMO-0")?;

    let mut audit = Project::new(
        "API Audit",
        "Initech Solutions",
        Some("27AAPFU0939F1ZV"),
        Billing::Fixed,
    )?;
    audit.add_milestone("Audit report", date(2026, 6, 30), rupees(90_000))?;
    audit.invoice_milestone(0, "DEMO-0")?;
    audit.record_payment(0)?;

    let mut pipeline = Project::new(
        "Data Pipeline",
        "Umbrella Analytics",
        Some("29AAPFU0939F1ZR"),
        Billing::Fixed,
    )?;
    pipeline.add_milestone("Ingestion", date(2026, 8, 10), rupees(45_000))?;
    pipeline.add_milestone("Dashboards", date(2026, 9, 25), rupees(35_000))?;
    pipeline.invoice_milestone(0, "DEMO-0")?;
    pipeline.record_payment(0)?;
    pipeline.invoice_milestone(1, "DEMO-1")?;

    Ok(vec![website, app, audit, pipeline])
}

const SELLER_GSTIN: &str = "27AAPFU0939F1ZV";

fn profile(gstin: Option<&str>) -> ProfileDraft {
    ProfileDraft {
        name: "Nila Design & Code".to_owned(),
        address: vec!["12 FC Road".to_owned(), "Pune 411004".to_owned()],
        gstin: gstin.map(str::to_owned),
        sac_code: DEFAULT_SAC.to_owned(),
        invoice_prefix: "NDC".to_owned(),
        lut_arn: Some("AD270426001234X".to_owned()),
        terms: vec![
            "Bank: HDFC Bank, A/c 50100012345678, IFSC HDFC0000123".to_owned(),
            "Payment due within 15 days".to_owned(),
        ],
        currency: Currency::Inr,
        logo: None,
    }
}

fn client(
    name: &str,
    gstin: Option<&str>,
    address: &[&str],
    region: ClientRegion,
) -> ClientDetails {
    ClientDetails {
        name: name.to_owned(),
        gstin: gstin.map(str::to_owned),
        address: address.iter().map(|line| (*line).to_owned()).collect(),
        currency: if region.is_abroad() {
            Currency::Eur
        } else {
            Currency::Inr
        },
        region,
    }
}

/// Save a project with fixed-price milestones and return the ids of those milestones
fn fixed(
    repo: &mut impl ProjectRepository,
    name: &str,
    client: ClientDetails,
    milestones: &[(&str, i64, NaiveDate)],
) -> Result<(Project, Vec<MilestoneId>), ServiceError> {
    let mut project = Project::for_client(name, client, Billing::Fixed)?;
    for (milestone, amount, due) in milestones {
        project.add_milestone(*milestone, *due, rupees(*amount))?;
    }
    repo.save(&project)?;
    let ids = project.milestones().iter().map(|m| m.id()).collect();
    Ok((project, ids))
}

/// An in-memory database with a business profile, projects and every kind of invoice; nothing
/// is written to disk.
///
/// The story: the freelancer issued a plain invoice while unregistered (FY 2025-26), then
/// registered for GST in April 2026 and has billed tax invoices and an export since.
pub fn store() -> Result<Store, ServiceError> {
    let store = Store::in_memory()?;
    let mut repo = store.repository();
    let mut invoices = InvoiceService::new(store.repository(), None);
    let rupee = |amount: i64| BigDecimal::from(amount);
    let karnataka = || ClientRegion::India {
        state: StateCode::parse("29").ok(),
    };

    let (pipeline, pipeline_ms) = fixed(
        &mut repo,
        "Data Pipeline",
        client(
            "Umbrella Analytics",
            Some("29AAPFU0939F1ZR"),
            &["4th Floor, Prestige Tower", "Bengaluru 560001"],
            ClientRegion::default(),
        ),
        &[
            ("Ingestion", 45_000, date(2026, 3, 10)),
            ("Dashboards", 35_000, date(2026, 9, 25)),
        ],
    )?;
    let (website, website_ms) = fixed(
        &mut repo,
        "Website Revamp",
        client(
            "Acme Pvt Ltd",
            Some("27AAPFU0939F2ZU"),
            &["Plot 7, MIDC", "Pune 411019"],
            ClientRegion::default(),
        ),
        &[
            ("Design", 50_000, date(2026, 9, 1)),
            ("Frontend", 60_000, date(2026, 10, 5)),
            ("Launch", 40_000, date(2026, 11, 15)),
        ],
    )?;
    let (audit, audit_ms) = fixed(
        &mut repo,
        "API Audit",
        client(
            "Initrode GmbH",
            None,
            &["Hauptstrasse 1", "10115 Berlin"],
            ClientRegion::Abroad {
                country: "Germany".to_owned(),
            },
        ),
        &[("Audit report", 1_000, date(2026, 6, 30))],
    )?;
    let mut app = Project::for_client(
        "Mobile App",
        client(
            "Globex Labs",
            None,
            &["22 Residency Road", "Bengaluru 560025"],
            karnataka(),
        ),
        Billing::hourly(rupee(1_500))?,
    )?;
    app.add_milestone("Prototype", date(2026, 10, 10), None)?;
    app.add_milestone("Beta", date(2026, 12, 15), None)?;
    app.log_time(0, TimeEntry::new(date(2026, 10, 6), 360, "Wireframes")?)?;
    app.log_time(
        0,
        TimeEntry::new(date(2026, 10, 8), 270, "Navigation shell")?,
    )?;
    app.log_time(1, TimeEntry::new(date(2026, 10, 13), 120, "Auth spike")?)?;
    repo.save(&app)?;
    let app_ms: Vec<MilestoneId> = app.milestones().iter().map(|m| m.id()).collect();

    // Unregistered: a plain invoice, paid with 10% TDS
    invoices.save_profile(profile(None))?;
    let plain = invoices.issue(pipeline.id(), &[pipeline_ms[0]], date(2026, 3, 10), None)?;
    let plain = plain.invoice.number().clone();
    invoices.record_payment(&plain, date(2026, 3, 25), rupee(40_500), rupee(4_500))?;

    // Registered from April 2026
    invoices.save_profile(profile(Some(SELLER_GSTIN)))?;
    // Billed in euros at ₹90.00, banked at a better rate
    let rate = ExchangeRate::new(rupee(90))?;
    let export = invoices.issue(audit.id(), &[audit_ms[0]], date(2026, 6, 30), Some(rate))?;
    let export = export.invoice.number().clone();
    invoices.record_foreign_payment(&export, date(2026, 7, 15), rupee(1_000), rupee(90_450))?;

    let design = invoices.issue(website.id(), &[website_ms[0]], date(2026, 9, 1), None)?;
    let design = design.invoice.number().clone();
    invoices.record_payment(&design, date(2026, 9, 20), rupee(54_000), rupee(5_000))?;
    invoices.issue(pipeline.id(), &[pipeline_ms[1]], date(2026, 9, 25), None)?;
    let frontend = invoices.issue(website.id(), &[website_ms[1]], date(2026, 10, 5), None)?;
    // Half paid so far
    let frontend = frontend.invoice.number().clone();
    invoices.record_payment(&frontend, date(2026, 10, 12), rupee(35_400), rupee(0))?;
    invoices.issue(app.id(), &[app_ms[0]], date(2026, 10, 10), None)?;

    // Business expenses, GST claimed back as input credit
    let mut expenses = ExpenseService::new(store.repository());
    for (day, category, what, amount, credit) in [
        (
            date(2026, 7, 5),
            ExpenseCategory::Software,
            "Figma, annual plan",
            1_180,
            180,
        ),
        (
            date(2026, 8, 20),
            ExpenseCategory::Equipment,
            "Laptop",
            177_000,
            27_000,
        ),
        (
            date(2026, 9, 1),
            ExpenseCategory::Workspace,
            "Coworking desk, September",
            11_800,
            1_800,
        ),
        (
            date(2026, 10, 1),
            ExpenseCategory::Internet,
            "Fibre broadband",
            1_180,
            180,
        ),
    ] {
        expenses.add(ExpenseDraft {
            date: day,
            category,
            description: what.to_owned(),
            amount: rupee(amount),
            gst_credit: rupee(credit),
        })?;
    }
    Ok(store)
}
