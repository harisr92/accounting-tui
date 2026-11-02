//! Clients billed before, and the GST states a client in India can be in, for the project form

use flutter_rust_bridge::frb;

use compter_core::domain::{Client, ClientRegion, states};
use compter_core::input;
use compter_core::services::{ClientAccount, ClientSummary, Owed};

use super::{InvoiceRow, ProjectRow};

/// A client billed before, with what the project form needs to fill itself in: the same fields
/// as `ProjectInput`, so picking one copies them across
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientChoiceRow {
    pub name: String,
    /// What tells clients apart in the list: `29AAPFU0939F1ZR`, `Karnataka (29)` or
    /// `Germany · EUR`
    pub summary: String,
    pub gstin: Option<String>,
    /// GST state code (`29`), for a client in India without a GSTIN
    pub state: Option<String>,
    /// Set only for a client abroad
    pub country: Option<String>,
    pub address: Vec<String>,
    /// `INR`, `USD`, `GBP` or `EUR`
    pub currency: String,
}

impl From<&Client> for ClientChoiceRow {
    fn from(client: &Client) -> Self {
        let (state, country) = region_fields(client.region());
        let place = client.place();
        let currency = client.currency();
        let foreign = (!currency.is_inr()).then(|| currency.code().to_owned());
        let summary = place.into_iter().chain(foreign).collect::<Vec<_>>();
        Self {
            name: client.name().to_owned(),
            summary: summary.join(" · "),
            gstin: client.gstin().map(ToString::to_string),
            state,
            country,
            address: client.address().to_vec(),
            currency: currency.code().to_owned(),
        }
    }
}

/// A GST state or union territory a client in India can be in
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateRow {
    /// `29`, as `ProjectInput::client_state` takes it
    pub code: String,
    /// `Karnataka`
    pub name: String,
}

/// Every GST state, in code order: the choices for a client's state
#[frb(sync)]
pub fn gst_states() -> Vec<StateRow> {
    states()
        .map(|(code, name)| StateRow {
            code: code.to_owned(),
            name: name.to_owned(),
        })
        .collect()
}

/// The state code a GSTIN is registered in (its first two digits) once `gstin` is a whole,
/// valid GSTIN in any case; `None` while it isn't. A registered client's state comes from this,
/// so the form needn't ask for it.
#[frb(sync)]
pub fn gstin_state(gstin: String) -> Option<String> {
    input::gstin_state(&gstin)
}

/// `(state code, country)` as the project form holds a region: one or the other, or neither
pub(crate) fn region_fields(region: &ClientRegion) -> (Option<String>, Option<String>) {
    match region {
        ClientRegion::India { state } => (state.as_ref().map(|s| s.as_str().to_owned()), None),
        ClientRegion::Abroad { country } => (None, Some(country.clone())),
    }
}

/// A line of the Clients screen
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientSummaryRow {
    pub id: String,
    pub name: String,
    /// The GSTIN, else `Karnataka (29)` or `Germany`; `None` for a client known by name only
    pub place: Option<String>,
    /// Projects for the client, archived ones included
    pub projects: u32,
    /// `₹35,400.00`, or `₹1,000.00 · €250.00` across currencies; `None` when nothing is due
    pub owed: Option<String>,
}

impl From<&ClientSummary> for ClientSummaryRow {
    fn from(summary: &ClientSummary) -> Self {
        let client = &summary.client;
        Self {
            id: client.id().to_string(),
            name: client.name().to_owned(),
            place: client.place(),
            projects: u32::try_from(summary.projects).unwrap_or(u32::MAX),
            owed: owed(&summary.owed),
        }
    }
}

/// A client's details as typed into the edit client form, which can't change the name. The
/// fields and their meanings are `ProjectInput`'s client fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientInput {
    /// Blank for an unregistered client or one abroad
    pub gstin: Option<String>,
    /// GST state code (`29`): the place of supply for a client in India without a GSTIN
    pub state: Option<String>,
    /// Blank for a client in India
    pub country: Option<String>,
    /// One entry per line; blank lines are dropped
    pub address: Vec<String>,
    /// `INR`, `USD`, `GBP` or `EUR`; blank means INR. The default for new projects: each
    /// project keeps its own.
    pub currency: Option<String>,
}

impl From<&Client> for ClientInput {
    fn from(client: &Client) -> Self {
        let (state, country) = region_fields(client.region());
        Self {
            gstin: client.gstin().map(ToString::to_string),
            state,
            country,
            address: client.address().to_vec(),
            currency: Some(client.currency().code().to_owned()),
        }
    }
}

/// An opened client: their details, projects and invoices
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientDetail {
    pub id: String,
    pub name: String,
    /// The details as the edit client form starts from
    pub edit: ClientInput,
    /// As in [`ClientSummaryRow::place`]
    pub place: Option<String>,
    /// Address lines, top to bottom
    pub address: Vec<String>,
    /// As in [`ClientSummaryRow::owed`]
    pub owed: Option<String>,
    /// Sorted by name, archived ones included
    pub projects: Vec<ProjectRow>,
    /// Newest first
    pub invoices: Vec<InvoiceRow>,
}

impl From<&ClientAccount> for ClientDetail {
    fn from(account: &ClientAccount) -> Self {
        let client = &account.client;
        Self {
            id: client.id().to_string(),
            name: client.name().to_owned(),
            edit: ClientInput::from(client),
            place: client.place(),
            address: client.address().to_vec(),
            owed: owed(&account.owed),
            projects: account.projects.iter().map(ProjectRow::from).collect(),
            invoices: account.invoices.iter().map(InvoiceRow::from).collect(),
        }
    }
}

/// Each currency's amount with its symbol, joined; `None` when nothing is owed
fn owed(owed: &[Owed]) -> Option<String> {
    let amounts: Vec<_> = owed.iter().map(|o| o.currency.display(&o.amount)).collect();
    (!amounts.is_empty()).then(|| amounts.join(" · "))
}
