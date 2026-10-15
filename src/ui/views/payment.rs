//! The "record payment" form, shared by the Projects and Invoices tabs

use bigdecimal::BigDecimal;
use chrono::{Local, NaiveDate};

use crate::domain::{Invoice, InvoiceNumber};
use crate::services::InvoiceService;
use crate::storage::Repository;
use crate::ui::form::{Field, Form, parse};
use crate::ui::view::Notice;
use crate::ui::widgets::money;

const DATE: usize = 0;
const RECEIVED: usize = 1;
const TDS: usize = 2;

/// TDS on professional fees under section 194J
const TDS_PERCENT: u32 = 10;

/// Prefilled with the full total received and no TDS; the hint shows what 10% TDS would be
pub fn payment_form(invoice: &Invoice) -> Form {
    let amounts = invoice.amounts();
    let tds_hint = format!(
        "withheld by the client; 10% of taxable is {}",
        money(&(&amounts.taxable_value * BigDecimal::from(TDS_PERCENT) / BigDecimal::from(100)))
    );
    Form::new(
        "Record payment",
        vec![
            Field::text("Date", "YYYY-MM-DD, blank for today"),
            Field::text("Received", "amount that reached your bank")
                .with_value(amounts.total.to_string()),
            Field::text("TDS", tds_hint).with_value("0"),
        ],
    )
}

/// Read the form and settle invoice `number`
pub fn submit_payment<R: Repository>(
    service: &mut InvoiceService<R>,
    number: &InvoiceNumber,
    form: &Form,
) -> Result<Notice, String> {
    let today: NaiveDate = Local::now().date_naive();
    let date = parse::date_or(form.value(DATE), today)?;
    let received = parse::money(form.value(RECEIVED))?;
    let tds = parse::money(form.value(TDS))?;
    let invoice = service
        .record_payment(number, date, received, tds)
        .map_err(|e| e.to_string())?;
    Ok(Notice::success(format!("{} marked paid", invoice.number())))
}
