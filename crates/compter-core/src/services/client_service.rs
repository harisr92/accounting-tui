use std::collections::HashMap;

use bigdecimal::{BigDecimal, Zero};

use super::ServiceError;
use crate::domain::{
    Client, ClientDetails, ClientId, Currency, InvoiceSummary, Project, ProjectSummary, client_key,
};
use crate::storage::{ClientRecord, ClientRepository, InvoiceRepository, ProjectRepository};

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
/// ([`Client::matches`]), as projects and invoices keep the name they were saved with. Lists
/// read summaries only. A client's details can be corrected here, and carried to their open
/// projects; issued invoices never change.
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
        let client = self.client(id)?;
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

    /// The client's open projects (not archived: they can still take new work, finished or
    /// not) whose copy of the client `details` would change: what [`update`](Self::update)
    /// offers to correct too. Details it would refuse are refused here, before anyone is asked.
    pub fn projects_to_update(
        &self,
        id: ClientId,
        details: &ClientDetails,
    ) -> Result<Vec<ProjectSummary>, ServiceError> {
        let client = self.client(id)?;
        ensure_same_name(&client, details)?;
        Client::with_id(id, details.clone())?;
        Ok(self
            .stale_projects(&client, details)?
            .into_iter()
            .map(|(summary, _)| summary)
            .collect())
    }

    /// Correct the client `id`'s GSTIN, address, region or currency, and with
    /// `update_projects` their open projects' copies too (each keeps its own currency), in one
    /// write. The name can't change: projects and invoices are filed under it. Issued invoices
    /// keep the details they were printed with. Nothing is saved if anything is refused.
    pub fn update(
        &mut self,
        id: ClientId,
        details: ClientDetails,
        update_projects: bool,
    ) -> Result<ClientAccount, ServiceError> {
        let mut client = self.client(id)?;
        ensure_same_name(&client, &details)?;
        let unchanged = client.details().same_as(&details);
        // Checked before any project, so a refusal of the details themselves names no project
        client.update(details.clone())?;
        let mut projects = Vec::new();
        if update_projects {
            for (_, mut project) in self.stale_projects(&client, &details)? {
                adopt(&mut project, &details)?;
                projects.push(project);
            }
        }
        if projects.is_empty() && unchanged {
            return self.account(id);
        }
        self.repo.save_client(&ClientRecord {
            client: &client,
            projects: &projects,
        })?;
        self.account(id)
    }

    /// The open projects of `client` whose copy of the client `details` would change
    fn stale_projects(
        &self,
        client: &Client,
        details: &ClientDetails,
    ) -> Result<Vec<(ProjectSummary, Project)>, ServiceError> {
        let mut stale = Vec::new();
        for summary in self.repo.list()? {
            if !is_open(client, &summary) {
                continue;
            }
            let project = self.repo.load(summary.id())?;
            if project.would_adopt(details) {
                stale.push((summary, project));
            }
        }
        Ok(stale)
    }

    /// The client `id` from the address book
    pub fn client(&self, id: ClientId) -> Result<Client, ServiceError> {
        self.repo
            .clients()?
            .into_iter()
            .find(|c| c.id() == id)
            .ok_or(ServiceError::NoSuchClient(id))
    }
}

/// A client's name stays as it is: their projects and invoices are filed under it
fn ensure_same_name(client: &Client, details: &ClientDetails) -> Result<(), ServiceError> {
    if details.name.trim() == client.name() {
        Ok(())
    } else {
        Err(ServiceError::ClientNameFixed)
    }
}

/// A project of `client` that can still bill: any not archived, as a finished one can take
/// new milestones, whose invoices print the project's copy of the client
fn is_open(client: &Client, project: &ProjectSummary) -> bool {
    client.matches(project.client_name()) && !project.is_archived()
}

/// `project` with the client's corrected details; a refusal names the project
fn adopt(project: &mut Project, details: &ClientDetails) -> Result<(), ServiceError> {
    project
        .adopt_client(details.clone())
        .map_err(|source| ServiceError::ProjectRefusedClient {
            project: project.name().to_owned(),
            source: Box::new(source),
        })
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
