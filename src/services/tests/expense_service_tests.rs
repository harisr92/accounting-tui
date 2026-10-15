use accounting_core::Ledger;
use bigdecimal::BigDecimal;
use pollster::block_on;

use super::fixtures::{SELLER_GSTIN, date, profile_draft};
use crate::domain::{ExpenseCategory, FiscalYear};
use crate::services::{ExpenseService, InvoiceService, ServiceError, chart};
use crate::storage::Store;

fn rupees(amount: i64) -> BigDecimal {
    BigDecimal::from(amount)
}

fn balance(store: &Store, account: &str) -> BigDecimal {
    let ledger = Ledger::new(store.ledger());
    let trial = block_on(ledger.get_trial_balance(date())).unwrap();
    trial.balances.get(account).map_or_else(
        || rupees(0),
        |b| {
            b.debit_balance.clone().unwrap_or_default()
                - b.credit_balance.clone().unwrap_or_default()
        },
    )
}

fn store(registered: bool) -> Store {
    let store = Store::in_memory().unwrap();
    let gstin = registered.then_some(SELLER_GSTIN);
    InvoiceService::new(store.repository(), None)
        .save_profile(profile_draft(gstin, None))
        .unwrap();
    store
}

#[test]
fn expenses_post_cost_credit_and_bank() {
    let store = store(true);
    let mut service = ExpenseService::new(store.repository());
    let expense = service
        .add(
            date(),
            ExpenseCategory::Software,
            "Figma",
            rupees(1180),
            rupees(180),
        )
        .unwrap();
    assert_eq!(
        balance(&store, chart::expense_account(ExpenseCategory::Software)),
        rupees(1000)
    );
    assert_eq!(balance(&store, chart::INPUT_GST), rupees(180));
    assert_eq!(balance(&store, chart::BANK), rupees(-1180));

    let year = FiscalYear::containing(date());
    assert_eq!(service.list(year).unwrap(), std::slice::from_ref(&expense));
    service.delete(expense.id()).unwrap();
    assert!(service.list(year).unwrap().is_empty());
    assert_eq!(balance(&store, chart::BANK), rupees(0));
}

#[test]
fn input_credit_needs_a_gstin() {
    let store = store(false);
    let mut service = ExpenseService::new(store.repository());
    assert!(matches!(
        service.add(
            date(),
            ExpenseCategory::Travel,
            "Train",
            rupees(1180),
            rupees(180)
        ),
        Err(ServiceError::CreditWithoutGstin)
    ));
    service
        .add(
            date(),
            ExpenseCategory::Travel,
            "Train",
            rupees(1180),
            rupees(0),
        )
        .unwrap();
}
