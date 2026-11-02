use std::collections::HashMap;

use bigdecimal::{BigDecimal, Zero};

use super::ServiceError;
use crate::domain::{Client, ClientId, Currency, InvoiceSummary, ProjectSummary, client_key};
use crate::storage::{ClientRepository, InvoiceRepository, ProjectRepository};

/// What a client still owes in one currency. Amounts in different currencies are never added
/// up or converted: a client billed in euros owes euros.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Owed {
    pub currency: Currency,
    pub amount: BigDecimal,
}

/// A client in the Clients list: how much work they gave and what they still owe
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientSummary {
    pub client: Client,
    /// Projects for this client, archived ones included
    pub projects: usize,
    /// Rupees first, then by currency code; empty when nothing is due
    pub owed: Vec<Owed>,
}

/// One client opened: their projects and every invoice issued to them
#[derive(Debug, Clone, PartialEq)]
pub struct ClientAccount {
    pub client: Client,
    /// Sorted by name, archived ones included
    pub projects: Vec<ProjectSummary>,
    /// Newest first
    pub invoices: Vec<InvoiceSummary>,
    /// As in [`ClientSummary::owed`]
    pub owed: Vec<Owed>,
}

/// The saved clients with their projects and invoices, matched by client name ignoring case
/// ([`Client::matches`]), as projects and invoices keep the name they were saved with. Reads
/// summaries only.
pub struct ClientService<R> {
    repo: R,
}

impl<R: ClientRepository + ProjectRepository + InvoiceRepository> ClientService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }

    /// Every client, sorted by name
    pub fn list(&self) -> Result<Vec<ClientSummary>, ServiceError> {
        let mut projects: HashMap<String, usize> = HashMap::new();
        for project in self.repo.list()? {
            *projects
                .entry(client_key(project.client_name()))
                .or_default() += 1;
        }
        let mut owing: HashMap<String, Vec<Owed>> = HashMap::new();
        for invoice in self.repo.list_invoices()? {
            if !invoice.is_settled() {
                add_owed(
                    owing.entry(client_key(invoice.client_name())).or_default(),
                    &invoice,
                );
            }
        }
        Ok(self
            .repo
            .clients()?
            .into_iter()
            .map(|client| {
                let key = client_key(client.name());
                ClientSummary {
                    projects: projects.get(&key).copied().unwrap_or_default(),
                    owed: tidy_owed(owing.remove(&key).unwrap_or_default()),
                    client,
                }
            })
            .collect())
    }

    /// The client `id` with their projects and invoices
    pub fn account(&self, id: ClientId) -> Result<ClientAccount, ServiceError> {
        let client = self
            .repo
            .clients()?
            .into_iter()
            .find(|c| c.id() == id)
            .ok_or(ServiceError::NoSuchClient(id))?;
        let projects: Vec<_> = self
            .repo
            .list()?
            .into_iter()
            .filter(|p| client.matches(p.client_name()))
            .collect();
        let invoices: Vec<_> = self
            .repo
            .list_invoices()?
            .into_iter()
            .filter(|i| client.matches(i.client_name()))
            .collect();
        Ok(ClientAccount {
            owed: owed(&invoices),
            client,
            projects,
            invoices,
        })
    }
}

/// The balances of `invoices`, added up per currency: rupees first, then by code
fn owed(invoices: &[InvoiceSummary]) -> Vec<Owed> {
    let mut owed = Vec::new();
    for invoice in invoices.iter().filter(|i| !i.is_settled()) {
        add_owed(&mut owed, invoice);
    }
    tidy_owed(owed)
}

/// Add `invoice`'s balance to its currency's amount in `owed`
fn add_owed(owed: &mut Vec<Owed>, invoice: &InvoiceSummary) {
    match owed.iter_mut().find(|o| o.currency == invoice.currency()) {
        Some(due) => due.amount += invoice.balance(),
        None => owed.push(Owed {
            currency: invoice.currency(),
            amount: invoice.balance().clone(),
        }),
    }
}

/// Drop currencies with nothing due and put rupees first, then the rest by code
fn tidy_owed(mut owed: Vec<Owed>) -> Vec<Owed> {
    owed.retain(|o| !o.amount.is_zero());
    owed.sort_by_key(|o| (!o.currency.is_inr(), o.currency.code()));
    owed
}
