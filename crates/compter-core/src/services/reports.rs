//! A financial year in numbers: income, 44ADA vs books, GST by quarter, TDS and clients.
//! Pure: built from invoices and expenses, no storage.

use std::collections::{BTreeMap, BTreeSet};

use bigdecimal::{BigDecimal, Zero};

use crate::domain::{
    BasisSchedule, CreditNote, Currency, Expense, ExpenseCategory, FiscalYear, IncomeBasis,
    Invoice, InvoiceAmounts, InvoiceKind, Quarter, ReturnKind,
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
    /// Withheld by the client on payments in the year, less what it reversed on credit notes
    /// (match with Form 26AS)
    pub tds: BigDecimal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YearReport {
    pub year: FiscalYear,
    /// The year's basis for income tax, from the business profile's schedule
    pub basis: IncomeBasis,
    pub invoice_count: usize,
    /// Taxable value of invoices dated in the year, less credit notes dated in it: the gross
    /// receipts for income tax
    pub gross_receipts: BigDecimal,
    /// The gross receipts 44ADA takes 50% of. Each invoice counts once, on the basis of the
    /// year it is dated in: on accrual its taxable value counts in that year (less credit notes,
    /// in theirs), on cash the fee part (excl. GST, TDS included) of each payment counts in the
    /// year it arrives (less refunds and TDS reversed, in theirs), whatever basis that later
    /// year is on. Foreign payments count the rupees banked.
    pub tax_receipts: BigDecimal,
    /// Fee part of payments dated in this accrual year on invoices from earlier cash years:
    /// never counted when invoiced, so in `tax_receipts` here
    pub carried_in: BigDecimal,
    /// Fee part of payments dated in this cash year on invoices from earlier accrual years:
    /// counted when invoiced, so left out of `tax_receipts`
    pub already_counted: BigDecimal,
    /// The part of `exchange_difference` on invoices counted on accrual, a gain on top of their
    /// taxable value. Invoices counted on cash already count the rupees banked.
    pub accrual_exchange: BigDecimal,
    /// Taxable value taken back by credit notes dated in the year (already out of
    /// `gross_receipts`)
    pub credited: BigDecimal,
    /// Part of `gross_receipts` from clients abroad
    pub export_receipts: BigDecimal,
    /// Invoice totals including GST, for invoices dated in the year
    pub invoiced_total: BigDecimal,
    /// Payments (whole or part) dated in the year that reached the bank, less refunds by credit
    /// notes dated in it, in rupees
    pub received: BigDecimal,
    /// On foreign-currency payments dated in the year: rupees banked less their value at the
    /// invoice's rate. Positive is a gain.
    pub exchange_difference: BigDecimal,
    /// TDS on payments dated in the year, less TDS clients reversed on credit notes dated in it
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
        (&self.tax_receipts * BigDecimal::from(PRESUMPTIVE_PERCENT) / BigDecimal::from(100))
            .with_scale_round(2, bigdecimal::RoundingMode::HalfUp)
    }

    /// Profit from the books: gross receipts and exchange gains less expenses
    pub fn books_profit(&self) -> BigDecimal {
        &self.tax_receipts + &self.accrual_exchange - &self.expenses
    }

    pub fn within_presumptive_limit(&self) -> bool {
        self.tax_receipts <= PRESUMPTIVE_LIMIT_RUPEES
    }

    /// In the first year on a new basis, what crossed over from the old one, so the invoices
    /// at the boundary can be checked; `None` when nothing did
    pub fn switch_note(&self) -> Option<String> {
        let rupees = |amount: &BigDecimal| Currency::Inr.display(amount);
        if !self.already_counted.is_zero() {
            Some(format!(
                "Switched to cash: {} received this year is left out, its invoices were counted on accrual when issued",
                rupees(&self.already_counted)
            ))
        } else if !self.carried_in.is_zero() {
            Some(format!(
                "Switched to accrual: {} received this year on invoices from cash years is counted, as it never was",
                rupees(&self.carried_in)
            ))
        } else {
            None
        }
    }

    /// What the Income tax panel says under its figures
    pub fn presumptive_note(&self) -> &'static str {
        match (self.presumptive_check(), self.basis) {
            (PresumptiveCheck::Within, IncomeBasis::Accrual) => {
                "Within the 44ADA limit. Gross receipts are invoices dated in the year, excl. GST"
            }
            (PresumptiveCheck::Within, IncomeBasis::Cash) => {
                "Within the 44ADA limit. Gross receipts are fees received in the year (TDS included), excl. GST"
            }
            (check, _) => check.message(),
        }
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
            Self::Within => "Within the 44ADA limit",
        }
    }
}

/// Accumulates a [`YearReport`] one invoice at a time. Feed every invoice before any credit
/// note: a note needs to know whether its invoice was counted on cash.
pub struct YearReportBuilder {
    report: YearReport,
    schedule: BasisSchedule,
    clients: BTreeMap<String, ClientYear>,
    /// Invoices counted on cash that have credit notes dated in the year; only refunds take
    /// anything off their receipts
    cash_credited: BTreeSet<String>,
}

impl YearReportBuilder {
    pub fn new(year: FiscalYear, schedule: BasisSchedule) -> Self {
        let zero = BigDecimal::zero;
        Self {
            report: YearReport {
                year,
                basis: schedule.basis(year),
                invoice_count: 0,
                gross_receipts: zero(),
                tax_receipts: zero(),
                carried_in: zero(),
                already_counted: zero(),
                accrual_exchange: zero(),
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
            schedule,
            clients: BTreeMap::new(),
            cash_credited: BTreeSet::new(),
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
        let on_cash = self.schedule.basis(invoice.fiscal_year()) == IncomeBasis::Cash;
        let year_on_cash = self.report.basis == IncomeBasis::Cash;
        if year.contains(invoice.date()) {
            let r = &mut self.report;
            r.invoice_count += 1;
            r.gross_receipts += &amounts.taxable_value;
            if !on_cash {
                r.tax_receipts += &amounts.taxable_value;
            }
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
            let r = &mut self.report;
            if payment.credited_inr.is_some() {
                let difference = received - invoice.to_inr(&payment.settled());
                if !on_cash {
                    r.accrual_exchange += &difference;
                }
                r.exchange_difference += difference;
            }
            let fee = fee_part(amounts, &(received + &payment.tds));
            match (on_cash, year_on_cash) {
                (true, true) => r.tax_receipts += fee,
                (true, false) => {
                    r.tax_receipts += &fee;
                    r.carried_in += fee;
                }
                (false, true) => r.already_counted += fee,
                (false, false) => {}
            }
            self.report.received += received;
            self.report.tds += &payment.tds;
            let client = self.client(invoice.client_name());
            client.received += received;
            client.tds += &payment.tds;
        }
        // Credit notes dated in the year that give money back: TDS the client reverses comes
        // off the year's TDS, and a refund takes its cash back out of the bank (an advance stays
        // received)
        let credits = invoice.credits().iter().filter(|c| year.contains(c.date));
        if on_cash && credits.clone().next().is_some() {
            self.cash_credited
                .insert(invoice.number().as_str().to_owned());
        }
        for returned in credits.filter_map(|c| c.returned.as_ref()) {
            if let Some(tds) = &returned.tds_reversed {
                self.take_back(on_cash, amounts, tds);
                self.report.tds -= tds;
                self.client(invoice.client_name()).tds -= tds;
            }
            if returned.kind == ReturnKind::Refunded {
                let booked = invoice.to_inr(&returned.cash());
                let refunded = returned
                    .refunded_inr
                    .clone()
                    .unwrap_or_else(|| booked.clone());
                self.take_back(on_cash, amounts, &refunded);
                if !on_cash {
                    self.report.accrual_exchange += &booked - &refunded;
                }
                self.report.exchange_difference += &booked - &refunded;
                self.report.received -= &refunded;
                self.client(invoice.client_name()).received -= refunded;
            }
        }
    }

    /// Money given back on an invoice counted on cash comes off the year it leaves in; on one
    /// counted on accrual its credit note takes the value off instead
    fn take_back(&mut self, on_cash: bool, amounts: &InvoiceAmounts, amount: &BigDecimal) {
        if !on_cash {
            return;
        }
        let fee = fee_part(amounts, amount);
        if self.report.basis == IncomeBasis::Accrual {
            self.report.carried_in -= &fee;
        }
        self.report.tax_receipts -= fee;
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
        if !self.cash_credited.contains(note.invoice().as_str()) {
            r.tax_receipts -= &amounts.taxable_value;
        }
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

/// The fee part of money that settled an invoice, without its GST: each rupee settles the
/// taxable value and the tax in proportion
fn fee_part(amounts: &InvoiceAmounts, paid: &BigDecimal) -> BigDecimal {
    if amounts.total.is_zero() {
        return BigDecimal::zero();
    }
    (paid * &amounts.taxable_value / &amounts.total)
        .with_scale_round(2, bigdecimal::RoundingMode::HalfUp)
}
