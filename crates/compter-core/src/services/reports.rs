//! A financial year in numbers: income, 44ADA vs books, GST by quarter, TDS and clients.
//! Pure: built from invoices and expenses, no storage.

use std::collections::BTreeMap;

use bigdecimal::{BigDecimal, Zero};

use crate::domain::{
    CreditNote, Expense, ExpenseCategory, FiscalYear, Invoice, InvoiceKind, Quarter,
};

/// Share of gross receipts deemed to be profit under section 44ADA
pub const PRESUMPTIVE_PERCENT: u32 = 50;
/// Gross receipts limit for 44ADA when cash receipts are at most 5% (all compter receipts are
/// bank receipts)
pub const PRESUMPTIVE_LIMIT_RUPEES: u64 = 7_500_000; // ₹75 lakh

/// GST charged on invoices and claimed on expenses in one quarter
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuarterGst {
    pub quarter: Quarter,
    pub cgst: BigDecimal,
    pub sgst: BigDecimal,
    pub igst: BigDecimal,
    pub input_credit: BigDecimal,
}

impl QuarterGst {
    fn new(quarter: Quarter) -> Self {
        Self {
            quarter,
            cgst: BigDecimal::zero(),
            sgst: BigDecimal::zero(),
            igst: BigDecimal::zero(),
            input_credit: BigDecimal::zero(),
        }
    }

    pub fn output(&self) -> BigDecimal {
        &self.cgst + &self.sgst + &self.igst
    }

    /// Output tax less input credit; negative means credit carried forward
    pub fn net(&self) -> BigDecimal {
        self.output() - &self.input_credit
    }
}

/// One client's year
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientYear {
    pub client: String,
    /// Taxable value invoiced in the year
    pub invoiced: BigDecimal,
    /// Received in the bank in the year
    pub received: BigDecimal,
    /// Withheld by the client on payments in the year (match with Form 26AS)
    pub tds: BigDecimal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YearReport {
    pub year: FiscalYear,
    pub invoice_count: usize,
    /// Taxable value of invoices dated in the year, less credit notes dated in it: the gross
    /// receipts for income tax
    pub gross_receipts: BigDecimal,
    /// Taxable value taken back by credit notes dated in the year (already out of
    /// `gross_receipts`)
    pub credited: BigDecimal,
    /// Part of `gross_receipts` from clients abroad
    pub export_receipts: BigDecimal,
    /// Invoice totals including GST, for invoices dated in the year
    pub invoiced_total: BigDecimal,
    /// Payments (whole or part) dated in the year that reached the bank, in rupees
    pub received: BigDecimal,
    /// On foreign-currency payments dated in the year: rupees banked less their value at the
    /// invoice's rate. Positive is a gain.
    pub exchange_difference: BigDecimal,
    /// TDS on payments dated in the year
    pub tds: BigDecimal,
    /// What is still due on invoices dated in the year, GST included
    pub outstanding: BigDecimal,
    /// Expenses dated in the year, net of GST claimed back
    pub expenses: BigDecimal,
    pub expenses_by_category: Vec<(ExpenseCategory, BigDecimal)>,
    pub quarters: [QuarterGst; 4],
    /// Sorted by amount invoiced, largest first
    pub clients: Vec<ClientYear>,
}

impl YearReport {
    /// Income under 44ADA: 50% of gross receipts
    pub fn presumptive_income(&self) -> BigDecimal {
        (&self.gross_receipts * BigDecimal::from(PRESUMPTIVE_PERCENT) / BigDecimal::from(100))
            .with_scale_round(2, bigdecimal::RoundingMode::HalfUp)
    }

    /// Profit from the books: gross receipts and exchange gains less expenses
    pub fn books_profit(&self) -> BigDecimal {
        &self.gross_receipts + &self.exchange_difference - &self.expenses
    }

    pub fn within_presumptive_limit(&self) -> bool {
        self.gross_receipts <= PRESUMPTIVE_LIMIT_RUPEES
    }

    /// Whether 44ADA can be used for the year, and what to watch for
    pub fn presumptive_check(&self) -> PresumptiveCheck {
        if !self.within_presumptive_limit() {
            PresumptiveCheck::OverLimit
        } else if self.books_profit() < self.presumptive_income() {
            PresumptiveCheck::BooksBelow
        } else {
            PresumptiveCheck::Within
        }
    }
}

/// How a year stands against section 44ADA
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresumptiveCheck {
    /// Gross receipts over the limit: 44ADA is not available
    OverLimit,
    /// Books show less profit than 44ADA deems
    BooksBelow,
    Within,
}

impl PresumptiveCheck {
    pub const fn message(self) -> &'static str {
        match self {
            Self::OverLimit => {
                "Gross receipts are over ₹75 lakh: 44ADA is not available, use your books"
            }
            Self::BooksBelow => {
                "Books profit is below 44ADA income: declaring the lower figure needs a tax audit (44AB) if income is over the basic exemption"
            }
            Self::Within => {
                "Within the 44ADA limit. Gross receipts are invoices dated in the year, excl. GST"
            }
        }
    }
}

/// Accumulates a [`YearReport`] one invoice at a time
pub struct YearReportBuilder {
    report: YearReport,
    clients: BTreeMap<String, ClientYear>,
}

impl YearReportBuilder {
    pub fn new(year: FiscalYear) -> Self {
        let zero = BigDecimal::zero;
        Self {
            report: YearReport {
                year,
                invoice_count: 0,
                gross_receipts: zero(),
                credited: zero(),
                export_receipts: zero(),
                invoiced_total: zero(),
                received: zero(),
                exchange_difference: zero(),
                tds: zero(),
                outstanding: zero(),
                expenses: zero(),
                expenses_by_category: Vec::new(),
                quarters: year.quarters().map(QuarterGst::new),
                clients: Vec::new(),
            },
            clients: BTreeMap::new(),
        }
    }

    fn client(&mut self, name: &str) -> &mut ClientYear {
        self.clients
            .entry(name.to_owned())
            .or_insert_with(|| ClientYear {
                client: name.to_owned(),
                invoiced: BigDecimal::zero(),
                received: BigDecimal::zero(),
                tds: BigDecimal::zero(),
            })
    }

    /// Count an invoice: its billing if dated in the year, its payment if paid in the year. All
    /// figures are in rupees: foreign invoices at their rate, payments as banked.
    pub fn add_invoice(&mut self, invoice: &Invoice) {
        let year = self.report.year;
        let amounts = &invoice.inr_amounts();
        if year.contains(invoice.date()) {
            let r = &mut self.report;
            r.invoice_count += 1;
            r.gross_receipts += &amounts.taxable_value;
            r.invoiced_total += &amounts.total;
            if invoice.kind() == InvoiceKind::Export {
                r.export_receipts += &amounts.taxable_value;
            }
            r.outstanding += invoice.to_inr(&invoice.balance());
            if let Some(q) = r
                .quarters
                .iter_mut()
                .find(|q| q.quarter.contains(invoice.date()))
            {
                q.cgst += &amounts.cgst;
                q.sgst += &amounts.sgst;
                q.igst += &amounts.igst;
            }
            self.client(invoice.client_name()).invoiced += &amounts.taxable_value;
        }
        for payment in invoice.payments().iter().filter(|p| year.contains(p.date)) {
            let received = payment.received_inr();
            if payment.credited_inr.is_some() {
                self.report.exchange_difference += received - invoice.to_inr(&payment.settled());
            }
            self.report.received += received;
            self.report.tds += &payment.tds;
            let client = self.client(invoice.client_name());
            client.received += received;
            client.tds += &payment.tds;
        }
    }

    /// Count a credit note dated in the year: it takes its value and GST back out of the year's
    /// billing (the invoice's balance already shows it in `outstanding`)
    pub fn add_credit_note(&mut self, note: &CreditNote) {
        if !self.report.year.contains(note.date()) {
            return;
        }
        let amounts = note.inr_amounts();
        let r = &mut self.report;
        r.gross_receipts -= &amounts.taxable_value;
        r.credited += &amounts.taxable_value;
        r.invoiced_total -= &amounts.total;
        if note.kind() == InvoiceKind::Export {
            r.export_receipts -= &amounts.taxable_value;
        }
        if let Some(q) = r
            .quarters
            .iter_mut()
            .find(|q| q.quarter.contains(note.date()))
        {
            q.cgst -= &amounts.cgst;
            q.sgst -= &amounts.sgst;
            q.igst -= &amounts.igst;
        }
        self.client(note.client_name()).invoiced -= &amounts.taxable_value;
    }

    pub fn add_expenses(&mut self, expenses: &[Expense]) {
        let mut by_category: BTreeMap<ExpenseCategory, BigDecimal> = BTreeMap::new();
        for expense in expenses
            .iter()
            .filter(|e| self.report.year.contains(e.date()))
        {
            let cost = expense.cost();
            self.report.expenses += &cost;
            *by_category.entry(expense.category()).or_default() += cost;
            if let Some(q) = self
                .report
                .quarters
                .iter_mut()
                .find(|q| q.quarter.contains(expense.date()))
            {
                q.input_credit += expense.gst_credit();
            }
        }
        self.report.expenses_by_category = by_category.into_iter().collect();
    }

    pub fn finish(mut self) -> YearReport {
        let mut clients: Vec<ClientYear> = self.clients.into_values().collect();
        clients.sort_by(|a, b| b.invoiced.cmp(&a.invoiced).then(a.client.cmp(&b.client)));
        self.report.clients = clients;
        self.report
    }
}
