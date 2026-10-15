//! Sample data for the layout shell. Replaced by `FileStorage` in step 4.

use bigdecimal::BigDecimal;
use chrono::NaiveDate;

use crate::domain::{Billing, DomainError, Project, TimeEntry};

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).unwrap_or_default()
}

fn rupees(amount: i64) -> Option<BigDecimal> {
    Some(BigDecimal::from(amount))
}

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
    website.invoice_milestone(0)?;
    website.record_payment(0)?;
    website.invoice_milestone(1)?;

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
    app.invoice_milestone(0)?;

    let mut audit = Project::new(
        "API Audit",
        "Initech Solutions",
        Some("27AAPFU0939F1ZV"),
        Billing::Fixed,
    )?;
    audit.add_milestone("Audit report", date(2026, 6, 30), rupees(90_000))?;
    audit.invoice_milestone(0)?;
    audit.record_payment(0)?;

    let mut pipeline = Project::new(
        "Data Pipeline",
        "Umbrella Analytics",
        Some("29AAPFU0939F1ZR"),
        Billing::Fixed,
    )?;
    pipeline.add_milestone("Ingestion", date(2026, 8, 10), rupees(45_000))?;
    pipeline.add_milestone("Dashboards", date(2026, 9, 25), rupees(35_000))?;
    pipeline.invoice_milestone(0)?;
    pipeline.record_payment(0)?;
    pipeline.invoice_milestone(1)?;

    Ok(vec![website, app, audit, pipeline])
}
