//! compter's ledger accounts, numbered in the style of accounting-core's standard chart

use accounting_core::{Account, AccountType, InvoiceAccounts};

use crate::domain::ExpenseCategory;

pub const BANK: &str = "1100";
pub const RECEIVABLE: &str = "1200";
/// Tax deducted at source by clients, claimable against income tax
pub const TDS_RECEIVABLE: &str = "1250";
/// GST paid on business purchases, claimed back as input tax credit
pub const INPUT_GST: &str = "1300";
pub const CGST_OUTPUT: &str = "2300";
pub const SGST_OUTPUT: &str = "2310";
pub const IGST_OUTPUT: &str = "2320";
pub const SERVICE_REVENUE: &str = "4100";
/// Services to clients abroad, kept apart for export reporting
pub const EXPORT_REVENUE: &str = "4110";
/// Gain (credit) or loss (debit) when a foreign-currency payment banks more or fewer rupees than
/// the invoice was booked at
pub const EXCHANGE_DIFFERENCE: &str = "4900";

/// Ledger account for each expense category
pub fn expense_account(category: ExpenseCategory) -> &'static str {
    match category {
        ExpenseCategory::Software => "5100",
        ExpenseCategory::Equipment => "5110",
        ExpenseCategory::Internet => "5120",
        ExpenseCategory::Travel => "5130",
        ExpenseCategory::Workspace => "5140",
        ExpenseCategory::ProfessionalFees => "5150",
        ExpenseCategory::Other => "5190",
    }
}

const CHART: &[(&str, &str, AccountType)] = &[
    (BANK, "Bank", AccountType::Asset),
    (RECEIVABLE, "Accounts Receivable", AccountType::Asset),
    (TDS_RECEIVABLE, "TDS Receivable", AccountType::Asset),
    (INPUT_GST, "Input GST", AccountType::Asset),
    (CGST_OUTPUT, "CGST Output", AccountType::Liability),
    (SGST_OUTPUT, "SGST Output", AccountType::Liability),
    (IGST_OUTPUT, "IGST Output", AccountType::Liability),
    (SERVICE_REVENUE, "Service Revenue", AccountType::Income),
    (EXPORT_REVENUE, "Export Revenue", AccountType::Income),
    (
        EXCHANGE_DIFFERENCE,
        "Exchange Gain/Loss",
        AccountType::Income,
    ),
];

/// Every account compter posts to; storage creates the ones that are missing
pub fn accounts() -> Vec<Account> {
    let expenses = ExpenseCategory::ALL
        .iter()
        .map(|&c| Account::new(expense_account(c), c.label(), AccountType::Expense, None));
    CHART
        .iter()
        .map(|&(id, name, kind)| Account::new(id, name, kind, None))
        .chain(expenses)
        .collect()
}

/// Where a GST tax invoice posts
pub fn invoice_accounts() -> InvoiceAccounts {
    InvoiceAccounts::new(
        RECEIVABLE,
        SERVICE_REVENUE,
        CGST_OUTPUT,
        SGST_OUTPUT,
        IGST_OUTPUT,
    )
}
