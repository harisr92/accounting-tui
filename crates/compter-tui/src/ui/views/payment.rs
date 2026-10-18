//! The "record payment" form, shared by the Projects and Invoices tabs

use bigdecimal::BigDecimal;
use chrono::{Local, NaiveDate};

use crate::ui::form::{Field, Form, parse};
use crate::ui::view::Notice;
use crate::ui::widgets::{money, money_in};
use compter_core::domain::{Invoice, InvoiceNumber};
use compter_core::services::InvoiceService;
use compter_core::storage::Repository;

const DATE: usize = 0;
const RECEIVED: usize = 1;
/// TDS on an invoice in rupees; the rupees banked on one in a foreign currency
const THIRD: usize = 2;

/// TDS on professional fees under section 194J
const TDS_PERCENT: u32 = 10;

/// Prefilled with the whole balance received and no TDS; a smaller amount is a part payment.
/// An invoice in a foreign currency asks for the rupees that reached the bank instead of TDS.
pub fn payment_form(invoice: &Invoice) -> Form {
    let amounts = invoice.amounts();
    let currency = invoice.currency();
    let balance = invoice.balance();
    let received_hint = format!(
        "in {currency}; {} due, part payments are fine",
        money_in(&balance, currency)
    );
    let third = if currency.is_inr() {
        let tds_hint = format!(
            "withheld by the client; 10% of taxable is {}",
            money(
                &(&amounts.taxable_value * BigDecimal::from(TDS_PERCENT) / BigDecimal::from(100))
            )
        );
        Field::text("TDS", tds_hint).with_value("0")
    } else {
        let rate = invoice
            .exchange_rate()
            .map(ToString::to_string)
            .unwrap_or_default();
        Field::text(
            "Banked ₹",
            format!("rupees credited to your bank; invoiced at ₹{rate}/{currency}"),
        )
    };
    Form::new(
        "Record payment",
        vec![
            Field::text("Date", "YYYY-MM-DD, blank for today"),
            Field::text("Received", received_hint).with_value(balance.to_string()),
            third,
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
    let third = parse::money(form.value(THIRD))?;
    let currency = service.load(number).map_err(|e| e.to_string())?.currency();
    let invoice = if currency.is_inr() {
        service.record_payment(number, date, received, third)
    } else {
        service.record_foreign_payment(number, date, received, third)
    }
    .map_err(|e| e.to_string())?;
    Ok(Notice::success(if invoice.is_paid() {
        format!("{} paid in full", invoice.number())
    } else {
        format!(
            "Payment recorded on {}; {} still due",
            invoice.number(),
            money_in(&invoice.balance(), currency)
        )
    }))
}
