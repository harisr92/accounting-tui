//! compter's ledger accounts, numbered in the style of accounting-core's standard chart

use accounting_core::{Account, AccountType, InvoiceAccounts};

pub const BANK: &str = "1100";
pub const RECEIVABLE: &str = "1200";
/// Tax deducted at source by clients, claimable against income tax
pub const TDS_RECEIVABLE: &str = "1250";
pub const CGST_OUTPUT: &str = "2300";
pub const SGST_OUTPUT: &str = "2310";
pub const IGST_OUTPUT: &str = "2320";
pub const SERVICE_REVENUE: &str = "4100";
/// Services to clients abroad, kept apart for export reporting
pub const EXPORT_REVENUE: &str = "4110";

const CHART: &[(&str, &str, AccountType)] = &[
    (BANK, "Bank", AccountType::Asset),
    (RECEIVABLE, "Accounts Receivable", AccountType::Asset),
    (TDS_RECEIVABLE, "TDS Receivable", AccountType::Asset),
    (CGST_OUTPUT, "CGST Output", AccountType::Liability),
    (SGST_OUTPUT, "SGST Output", AccountType::Liability),
    (IGST_OUTPUT, "IGST Output", AccountType::Liability),
    (SERVICE_REVENUE, "Service Revenue", AccountType::Income),
    (EXPORT_REVENUE, "Export Revenue", AccountType::Income),
];

/// Every account invoicing and payments post to; storage creates the ones that are missing
pub fn accounts() -> Vec<Account> {
    CHART
        .iter()
        .map(|&(id, name, kind)| Account::new(id, name, kind, None))
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
