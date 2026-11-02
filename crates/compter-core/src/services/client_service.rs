use bigdecimal::{BigDecimal, Zero};

use super::ServiceError;
use crate::domain::{
    Client, ClientDetails, ClientId, ClientIndex, Currency, InvoiceSummary, Project, ProjectSummary,
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

/// The saved clients with their projects and invoices, matched by any of the client's names
/// ignoring case ([`Client::matches`]), as projects and invoices keep the name they were saved
/// with. Lists read summaries only. A client's details can be corrected here, the client renamed,
/// or a duplicate merged into them, and each change carried to their open projects; issued
/// invoices never change, so the old names stay on the client as aliases.
pub struct ClientService<R> {
    repo: R,
}

impl<R: ClientRepository + ProjectRepository + InvoiceRepository> ClientService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }

    /// Every client, sorted by name
    pub fn list(&self) -> Result<Vec<ClientSummary>, ServiceError> {
        let clients = self.repo.clients()?;
        let index = ClientIndex::new(&clients);
        let mut projects = vec![0_usize; clients.len()];
        for project in self.repo.list()? {
            if let Some(count) = index
                .find(project.client_name())
                .and_then(|i| projects.get_mut(i))
            {
                *count += 1;
            }
        }
        let mut owing: Vec<Vec<Owed>> = vec![Vec::new(); clients.len()];
        for invoice in self.repo.list_invoices()? {
            if invoice.is_settled() {
                continue;
            }
            if let Some(owed) = index
                .find(invoice.client_name())
                .and_then(|i| owing.get_mut(i))
            {
                add_owed(owed, &invoice);
            }
        }
        Ok(clients
            .into_iter()
            .zip(projects)
            .zip(owing)
            .map(|((client, projects), owed)| ClientSummary {
                projects,
                owed: tidy_owed(owed),
                client,
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
        Ok(summaries(self.stale(&client, details)?))
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
        let projects = self.adopting(&client, &details, update_projects)?;
        if projects.is_empty() && unchanged {
            return self.account(id);
        }
        self.repo.save_client(&ClientRecord {
            client: &client,
            projects: &projects,
            removed: None,
        })?;
        self.account(id)
    }

    /// The client's open projects that [`rename`](Self::rename) would give the new `name`:
    /// asked about first; none when `name` is already theirs. A name it would refuse is
    /// refused here.
    pub fn projects_to_rename(
        &self,
        id: ClientId,
        name: &str,
    ) -> Result<Vec<ProjectSummary>, ServiceError> {
        match self.renamed(id, name)? {
            Some(client) => Ok(summaries(self.stale(&client, &client.details())?)),
            None => Ok(Vec::new()),
        }
    }

    /// Call the client `id` by `name` from now on, and with `update_projects` their open
    /// projects too, in one write. The old name stays on the client as an alias, so the invoices
    /// issued under it (which never change) and the projects left as they are still count as
    /// theirs. Refused when another client already goes by `name`: merge them instead. Their
    /// own name again changes nothing.
    pub fn rename(
        &mut self,
        id: ClientId,
        name: &str,
        update_projects: bool,
    ) -> Result<ClientAccount, ServiceError> {
        let Some(client) = self.renamed(id, name)? else {
            return self.account(id);
        };
        let projects = self.adopting(&client, &client.details(), update_projects)?;
        self.repo.save_client(&ClientRecord {
            client: &client,
            projects: &projects,
            removed: None,
        })?;
        self.account(id)
    }

    /// The open projects of both clients that [`merge`](Self::merge) would give the merged
    /// client's name and details: `from`'s, and `into`'s own when the merge fills in what they
    /// lacked. Asked about first. A merge it would refuse is refused here.
    pub fn projects_to_merge(
        &self,
        from: ClientId,
        into: ClientId,
    ) -> Result<Vec<ProjectSummary>, ServiceError> {
        let client = self.merged(from, into)?;
        Ok(summaries(self.stale(&client, &client.details())?))
    }

    /// Merge the client `from`, a duplicate, into `into`, in one write: `from`'s names become
    /// aliases of `into`, whose details are kept (and filled in from `from`'s where `into` had
    /// none), and `from` leaves the address book. With `update_projects`, both clients' open
    /// projects take the merged name and details. Invoices never
    /// change; they count under `into` by the alias. Refused when both clients have GSTINs that
    /// differ. Returns `into`'s account.
    pub fn merge(
        &mut self,
        from: ClientId,
        into: ClientId,
        update_projects: bool,
    ) -> Result<ClientAccount, ServiceError> {
        let client = self.merged(from, into)?;
        let projects = self.adopting(&client, &client.details(), update_projects)?;
        self.repo.save_client(&ClientRecord {
            client: &client,
            projects: &projects,
            removed: Some(from),
        })?;
        self.account(into)
    }

    /// The client `id` renamed `name`, unless another client goes by it; `None` when it is
    /// their name already
    fn renamed(&self, id: ClientId, name: &str) -> Result<Option<Client>, ServiceError> {
        let clients = self.repo.clients()?;
        if let Some(other) = clients.iter().find(|c| c.id() != id && c.matches(name)) {
            return Err(ServiceError::ClientNameTaken {
                other: other.name().to_owned(),
            });
        }
        let mut client = find(clients, id)?;
        if client.name() == name.trim() {
            return Ok(None);
        }
        client.rename(name)?;
        Ok(Some(client))
    }

    /// The client `into` with `from` absorbed, answering to both clients' names
    fn merged(&self, from: ClientId, into: ClientId) -> Result<Client, ServiceError> {
        if from == into {
            return Err(ServiceError::MergeIntoSelf);
        }
        let (mut target, mut merged) = (None, None);
        for client in self.repo.clients()? {
            if client.id() == into {
                target = Some(client);
            } else if client.id() == from {
                merged = Some(client);
            }
        }
        let mut target = target.ok_or(ServiceError::NoSuchClient(into))?;
        let merged = merged.ok_or(ServiceError::NoSuchClient(from))?;
        target.absorb(&merged)?;
        Ok(target)
    }

    /// With `update`, the open projects of `client` given `details`; none without
    fn adopting(
        &self,
        client: &Client,
        details: &ClientDetails,
        update: bool,
    ) -> Result<Vec<Project>, ServiceError> {
        if !update {
            return Ok(Vec::new());
        }
        let mut projects = Vec::new();
        for (_, mut project) in self.stale(client, details)? {
            adopt(&mut project, details)?;
            projects.push(project);
        }
        Ok(projects)
    }

    /// The open projects of `client` whose copy of the client `details` would change. One that
    /// can't take them (a foreign-currency project and a client in India) refuses only when
    /// asked to, so the client alone can still change.
    fn stale(
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
        find(self.repo.clients()?, id)
    }
}

/// The client `id` among `clients`
fn find(clients: Vec<Client>, id: ClientId) -> Result<Client, ServiceError> {
    clients
        .into_iter()
        .find(|c| c.id() == id)
        .ok_or(ServiceError::NoSuchClient(id))
}

/// The summaries of the projects [`ClientService::stale`] found
fn summaries(stale: Vec<(ProjectSummary, Project)>) -> Vec<ProjectSummary> {
    stale.into_iter().map(|(summary, _)| summary).collect()
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
