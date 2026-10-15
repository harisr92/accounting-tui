//! accounting-core's `Ledger` running on the redb ledger store

use accounting_core::{
    AccountStore, Ledger, LedgerError, PaginationOption, PaginationParams, TransactionBuilder,
    TransactionFilter, TransactionStore,
};
use bigdecimal::BigDecimal;
use chrono::NaiveDate;
use compter_tui::storage::{DataDir, RedbLedgerStorage, Store};
use pollster::block_on;

fn date(day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, day).unwrap()
}

/// Account ids of the standard chart, which accounting-core keys by account code
struct Chart {
    cash: String,
    receivable: String,
}

/// Invoice ₹1,000 on the 1st, receive it on the 5th
fn record_sample(ledger: &mut Ledger<RedbLedgerStorage>) -> Chart {
    let chart = block_on(ledger.setup_standard_chart_of_accounts()).unwrap();
    let id = |key: &str| chart[key].id.clone();
    let (cash, receivable, revenue) =
        (id("cash"), id("accounts_receivable"), id("service_revenue"));

    let invoice = TransactionBuilder::new("txn-invoice", date(1), "Invoice INV-1")
        .debit(receivable.as_str(), BigDecimal::from(1000), None)
        .credit(revenue.as_str(), BigDecimal::from(1000), None)
        .build()
        .unwrap();
    let receipt = TransactionBuilder::new("txn-receipt", date(5), "Payment for INV-1")
        .debit(cash.as_str(), BigDecimal::from(1000), None)
        .credit(receivable.as_str(), BigDecimal::from(1000), None)
        .build()
        .unwrap();
    block_on(ledger.record_transaction(invoice)).unwrap();
    block_on(ledger.record_transaction(receipt)).unwrap();
    Chart { cash, receivable }
}

#[test]
fn ledger_persists_across_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = DataDir::new(dir.path());

    let (accounts, trial_balance) = {
        let store = Store::open(&data_dir).unwrap();
        let mut ledger = Ledger::new(store.ledger());
        record_sample(&mut ledger);
        (
            block_on(ledger.list_all_accounts()).unwrap(),
            block_on(ledger.get_trial_balance(date(31))).unwrap(),
        )
    };
    assert!(trial_balance.is_balanced);

    let store = Store::open(&data_dir).unwrap();
    let ledger = Ledger::new(store.ledger());
    assert_eq!(block_on(ledger.list_all_accounts()).unwrap(), accounts);
    assert_eq!(
        block_on(ledger.get_trial_balance(date(31))).unwrap(),
        trial_balance
    );
    let invoice = block_on(ledger.get_transaction("txn-invoice"))
        .unwrap()
        .unwrap();
    assert_eq!(invoice.description, "Invoice INV-1");
}

#[test]
fn lists_filter_and_paginate_newest_first() {
    let store = Store::in_memory().unwrap();
    let mut ledger = Ledger::new(store.ledger());
    let chart = record_sample(&mut ledger);
    let storage = store.ledger();

    let filter = TransactionFilter {
        account_id: Some(chart.receivable),
        ..TransactionFilter::default()
    };
    let first_page = PaginationOption::Paginated(PaginationParams::new(1, 1).unwrap());
    let page = block_on(storage.list_transactions(&filter, first_page))
        .unwrap()
        .into_paginated_response();
    assert_eq!(page.total_count, 2);
    assert!(page.has_next);
    assert_eq!(page.items[0].id, "txn-receipt");

    let cash_only = TransactionFilter {
        account_id: Some(chart.cash),
        ..TransactionFilter::default()
    };
    let cash = block_on(storage.list_transactions(&cash_only, PaginationOption::All)).unwrap();
    assert_eq!(cash.items().len(), 1);

    let accounts = block_on(storage.list_accounts(None, PaginationOption::All)).unwrap();
    let ids: Vec<&str> = accounts.items().iter().map(|a| a.id.as_str()).collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    assert_eq!(ids, sorted);
}

#[test]
fn missing_records_are_not_found() {
    let store = Store::in_memory().unwrap();
    let mut ledger = Ledger::new(store.ledger());
    let chart = record_sample(&mut ledger);
    let mut storage = store.ledger();

    let mut ghost = block_on(storage.get_account(&chart.cash)).unwrap().unwrap();
    ghost.id = "ghost".to_owned();
    assert!(matches!(
        block_on(storage.update_account(&ghost)),
        Err(LedgerError::AccountNotFound(id)) if id == "ghost"
    ));
    assert!(block_on(storage.get_account("ghost")).unwrap().is_none());
    assert!(matches!(
        block_on(storage.delete_transaction("nope")),
        Err(LedgerError::TransactionNotFound(_))
    ));
}
