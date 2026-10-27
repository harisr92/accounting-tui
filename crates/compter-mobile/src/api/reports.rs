use bigdecimal::{BigDecimal, Zero};
use chrono::{Local, NaiveDate};

use compter_core::domain::Currency;
use compter_core::services::reports::{PRESUMPTIVE_PERCENT, PresumptiveCheck, YearReport};

/// How a note should look, like `Tone` in the terminal's theme
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Neutral,
    Warning,
    Error,
}

/// A figure with its label, for a row of tiles
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FigureRow {
    pub label: String,
    pub value: String,
}

/// GST charged and claimed in one quarter, in rupees
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuarterGstRow {
    /// `Q2 (Jul-Sep)`
    pub quarter: String,
    pub cgst: String,
    pub sgst: String,
    pub igst: String,
    pub input_credit: String,
    /// Output less input credit; negative is credit carried forward
    pub net: String,
}

/// One client's year, to match TDS with Form 26AS
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientRow {
    pub client: String,
    pub invoiced: String,
    pub received: String,
    pub tds: String,
}

/// A financial year in numbers, like the terminal's Reports tab, with what the return exports
/// offer first
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportRow {
    /// `FY 2026-27`
    pub year: String,
    pub income: Vec<FigureRow>,
    /// Gross receipts on the profile's basis, 44ADA income, expenses and books profit
    pub income_tax: Vec<FigureRow>,
    pub income_tax_note: String,
    /// In the first year on a new basis: what crossed over from the old one, to check by hand
    pub income_tax_switch: Option<String>,
    pub income_tax_tone: Tone,
    pub gst: Vec<QuarterGstRow>,
    /// Largest invoiced first
    pub clients: Vec<ClientRow>,
    /// `Q1 (Apr-Jun)` to `Q4 (Jan-Mar)`, for the GSTR-1 export
    pub quarters: Vec<String>,
    /// 1 to 4: the last quarter that has ended
    pub quarter_to_file: u32,
    /// `Apr 2026`, ...: the months an IFF can be filed for
    pub iff_months: Vec<String>,
    /// Position in `iff_months` of the latest that has ended
    pub iff_month_to_file: u32,
}

impl From<&YearReport> for ReportRow {
    fn from(report: &YearReport) -> Self {
        let figure = |label: &str, amount: &BigDecimal| FigureRow {
            label: label.to_owned(),
            value: rupees(amount),
        };
        let mut income = vec![
            FigureRow {
                label: "Invoices".to_owned(),
                value: report.invoice_count.to_string(),
            },
            figure("Receipts", &report.gross_receipts),
            figure("Exports", &report.export_receipts),
            figure("Received", &report.received),
            figure("TDS", &report.tds),
            figure("Outstanding", &report.outstanding),
        ];
        if !report.credited.is_zero() {
            income.insert(2, figure("Credit notes", &report.credited));
        }
        if !report.exchange_difference.is_zero() {
            income.push(figure("Exchange gain", &report.exchange_difference));
        }
        let check = report.presumptive_check();
        let today = Local::now().date_naive();
        let year = report.year;
        Self {
            year: year.to_string(),
            income,
            income_tax: vec![
                figure(
                    &format!("Receipts ({})", report.basis.label()),
                    &report.tax_receipts,
                ),
                figure(
                    &format!("44ADA income ({PRESUMPTIVE_PERCENT}%)"),
                    &report.presumptive_income(),
                ),
                figure("Expenses", &report.expenses),
                figure("Books profit", &report.books_profit()),
            ],
            income_tax_note: report.presumptive_note().to_owned(),
            income_tax_switch: report.switch_note(),
            income_tax_tone: match check {
                PresumptiveCheck::OverLimit => Tone::Error,
                PresumptiveCheck::BooksBelow => Tone::Warning,
                PresumptiveCheck::Within => Tone::Neutral,
            },
            gst: report
                .quarters
                .iter()
                .map(|q| QuarterGstRow {
                    quarter: q.quarter.label().to_owned(),
                    cgst: rupees(&q.cgst),
                    sgst: rupees(&q.sgst),
                    igst: rupees(&q.igst),
                    input_credit: rupees(&q.input_credit),
                    net: rupees(&q.net()),
                })
                .collect(),
            clients: report
                .clients
                .iter()
                .map(|c| ClientRow {
                    client: c.client.clone(),
                    invoiced: rupees(&c.invoiced),
                    received: rupees(&c.received),
                    tds: rupees(&c.tds),
                })
                .collect(),
            quarters: year
                .quarters()
                .iter()
                .map(|q| q.label().to_owned())
                .collect(),
            quarter_to_file: u32::from(year.quarter_to_file(today).number()),
            iff_months: year
                .iff_months()
                .iter()
                .map(|&month| month_label(month))
                .collect(),
            iff_month_to_file: u32::try_from(year.iff_month_to_file(today)).unwrap_or_default(),
        }
    }
}

/// A return written under `returns/<year>/`, to share with the GST portal
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportRow {
    pub path: String,
    /// `GSTR-1 Q2 (Jul-Sep) 2026-27: 3 tax invoice(s), 1 export(s), 0 credit note(s)`
    pub summary: String,
}

/// `Aug 2026` for `(2026, 8)`
pub(crate) fn month_label((year, month): (i32, u32)) -> String {
    NaiveDate::from_ymd_opt(year, month, 1)
        .map(|first| first.format("%b %Y").to_string())
        .unwrap_or_default()
}

fn rupees(amount: &BigDecimal) -> String {
    Currency::Inr.display(amount)
}
