use accounting_core::Ledger;
use bigdecimal::BigDecimal;
use pollster::block_on;

use super::fixtures::{SELLER_GSTIN, date, profile_draft};
use crate::domain::{ExpenseCategory, FiscalYear};
use crate::services::{ExpenseDraft, ExpenseService, InvoiceService, ServiceError, chart};
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

fn draft(category: ExpenseCategory, what: &str, amount: i64, credit: i64) -> ExpenseDraft {
    ExpenseDraft {
        date: date(),
        category,
        description: what.to_owned(),
        amount: rupees(amount),
        gst_credit: rupees(credit),
    }
}

#[test]
fn expenses_post_cost_credit_and_bank() {
    let store = store(true);
    let mut service = ExpenseService::new(store.repository());
    let expense = service
        .add(draft(ExpenseCategory::Software, "Figma", 1180, 180))
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
        service.add(draft(ExpenseCategory::Travel, "Train", 1180, 180)),
        Err(ServiceError::CreditWithoutGstin)
    ));
    service
        .add(draft(ExpenseCategory::Travel, "Train", 1180, 0))
        .unwrap();
}

#[test]
fn editing_an_expense_replaces_its_posting() {
    let store = store(true);
    let mut service = ExpenseService::new(store.repository());
    let expense = service
        .add(draft(ExpenseCategory::Software, "Figma", 1180, 180))
        .unwrap();
    let edited = service
        .edit(
            expense.id(),
            draft(ExpenseCategory::Equipment, "Monitor", 23_600, 3_600),
        )
        .unwrap();
    assert_eq!(edited.id(), expense.id());
    assert_eq!(
        balance(&store, chart::expense_account(ExpenseCategory::Software)),
        rupees(0)
    );
    assert_eq!(
        balance(&store, chart::expense_account(ExpenseCategory::Equipment)),
        rupees(20_000)
    );
    assert_eq!(balance(&store, chart::INPUT_GST), rupees(3_600));
    assert_eq!(balance(&store, chart::BANK), rupees(-23_600));
    let year = FiscalYear::containing(date());
    assert_eq!(service.list(year).unwrap(), [edited]);

    // A rejected edit changes nothing
    assert!(
        service
            .edit(expense.id(), draft(ExpenseCategory::Other, "Bad", 100, 100))
            .is_err()
    );
    assert_eq!(service.list(year).unwrap()[0].description(), "Monitor");
    assert_eq!(balance(&store, chart::BANK), rupees(-23_600));
}
