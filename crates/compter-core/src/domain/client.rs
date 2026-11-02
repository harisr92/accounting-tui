use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

use accounting_core::{Gstin, StateCode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::error::require;
use super::{Currency, DomainError, ProjectId, state_label};

/// Where a client is, which decides how they are taxed
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientRegion {
    /// In India. `state` is the place of supply for a client without a GSTIN; a registered
    /// client's state comes from their GSTIN.
    India {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        state: Option<StateCode>,
    },
    /// Outside India: services to them are exports
    Abroad { country: String },
}

impl Default for ClientRegion {
    fn default() -> Self {
        Self::India { state: None }
    }
}

impl ClientRegion {
    pub fn is_abroad(&self) -> bool {
        matches!(self, Self::Abroad { .. })
    }

    /// `Karnataka (29)` or `Germany`; `None` for a client in India whose state isn't known
    pub fn location(&self) -> Option<String> {
        match self {
            Self::India { state: Some(code) } => Some(state_label(code.as_str())),
            Self::India { state: None } => None,
            Self::Abroad { country } => Some(country.clone()),
        }
    }
}

/// Everything about a client, as entered; validated by [`Client`] and [`Project`](super::Project)
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClientDetails {
    pub name: String,
    /// `None` for an unregistered or foreign client
    pub gstin: Option<String>,
    /// Address lines, top to bottom; blank lines are dropped
    pub address: Vec<String>,
    pub region: ClientRegion,
    /// What the client is billed in; anything but INR needs a client abroad
    pub currency: Currency,
}

impl ClientDetails {
    /// An Indian client known only by name and (optionally) GSTIN
    pub fn named(name: impl Into<String>, gstin: Option<&str>) -> Self {
        Self {
            name: name.into(),
            gstin: gstin.map(str::to_owned),
            ..Self::default()
        }
    }

    /// The details once every rule holds: the one place a client is checked as entered
    pub(super) fn checked(self) -> Result<CheckedClient, DomainError> {
        let gstin = self.gstin.as_deref().map(Gstin::parse).transpose()?;
        validate_client(&self.name, gstin.as_ref(), &self.region, self.currency)?;
        // A state as well as a GSTIN is the place of supply on an invoice, so it must be the
        // state the GSTIN is registered in. Checked only as entered, so records stored before
        // still load.
        if let (Some(gstin), ClientRegion::India { state: Some(state) }) = (&gstin, &self.region)
            && state.as_str() != gstin.state_code()
        {
            return Err(DomainError::StateNotGstins(state_label(gstin.state_code())));
        }
        let address = self
            .address
            .into_iter()
            .map(|line| line.trim().to_owned())
            .filter(|line| !line.is_empty())
            .collect();
        Ok(CheckedClient {
            name: self.name.trim().to_owned(),
            gstin,
            address,
            region: self.region,
            currency: self.currency,
        })
    }

    /// Whether `other` describes the same client in the same way once both are tidied as
    /// [`checked`](Self::checked) stores them: names that differ only in case and spaces, the
    /// GSTIN in any case, blank address lines, the address split over lines differently (a
    /// form with fewer address lines joins the rest with `, `), and the state of a client with
    /// a GSTIN (which comes from the GSTIN) don't count as changes
    pub fn same_as(&self, other: &Self) -> bool {
        self.comparable() == other.comparable()
    }

    fn comparable(&self) -> (String, Option<String>, String, ClientRegion, Currency) {
        let gstin = self.gstin.as_deref().map(|g| g.trim().to_uppercase());
        let region = match (&gstin, &self.region) {
            (Some(_), ClientRegion::India { .. }) => ClientRegion::India { state: None },
            (_, region) => region.clone(),
        };
        let address = self
            .address
            .iter()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join(", ");
        (
            self.name.trim().to_lowercase(),
            gstin,
            address,
            region,
            self.currency,
        )
    }
}

/// [`ClientDetails`] that passed [`ClientDetails::checked`], ready to store
pub(super) struct CheckedClient {
    pub(super) name: String,
    pub(super) gstin: Option<Gstin>,
    pub(super) address: Vec<String>,
    pub(super) region: ClientRegion,
    pub(super) currency: Currency,
}

/// The rules every client meets, as entered or as loaded: a name, a foreign client has a
/// country and no GSTIN, and only a client abroad is billed in a foreign currency
pub(super) fn validate_client(
    name: &str,
    gstin: Option<&Gstin>,
    region: &ClientRegion,
    currency: Currency,
) -> Result<(), DomainError> {
    require(name, "client name")?;
    if let ClientRegion::Abroad { country } = region {
        require(country, "client country")?;
        if gstin.is_some() {
            return Err(DomainError::ForeignClientWithGstin);
        }
    }
    // Clients in India are billed in rupees: their invoices are GST tax invoices
    if !currency.is_inr() && !region.is_abroad() {
        return Err(DomainError::ForeignCurrencyInIndia);
    }
    Ok(())
}

/// Stable client identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ClientId(Uuid);

impl ClientId {
    pub fn generate() -> Self {
        Self(Uuid::now_v7())
    }

    /// The id a database upgrade gives the client of `project`, the newest project they were
    /// on: the same on every device that upgrades the same file, so their copies still match
    pub(crate) fn of_project(project: ProjectId) -> Self {
        Self(project.as_uuid())
    }

    pub fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl fmt::Display for ClientId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for ClientId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(Self)
    }
}

/// A client someone has billed, with the details they last entered for them
///
/// The address book a new project's client is picked from. Projects keep their own copy of the
/// details, so changing a client here never changes a project or an invoice. A client is known
/// by name or by an earlier name (an alias, kept by [`rename`](Self::rename) and
/// [`absorb`](Self::absorb), as issued invoices keep the name they were printed with): saving a
/// project whose client has one of those names (ignoring case and surrounding spaces) updates
/// this record instead of adding another.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawClient")]
pub struct Client {
    id: ClientId,
    name: String,
    /// Earlier names, each told apart by [`client_key`] and none the current name
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    aliases: Vec<String>,
    gstin: Option<Gstin>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    address: Vec<String>,
    #[serde(default)]
    region: ClientRegion,
    #[serde(default)]
    currency: Currency,
}

/// Unvalidated shape of a [`Client`]
#[derive(Deserialize)]
struct RawClient {
    id: ClientId,
    name: String,
    #[serde(default)]
    aliases: Vec<String>,
    gstin: Option<Gstin>,
    #[serde(default)]
    address: Vec<String>,
    #[serde(default)]
    region: ClientRegion,
    #[serde(default)]
    currency: Currency,
}

impl TryFrom<RawClient> for Client {
    type Error = DomainError;

    fn try_from(raw: RawClient) -> Result<Self, Self::Error> {
        validate_client(&raw.name, raw.gstin.as_ref(), &raw.region, raw.currency)?;
        let mut client = Self {
            id: raw.id,
            name: raw.name,
            aliases: Vec::new(),
            gstin: raw.gstin,
            address: raw.address,
            region: raw.region,
            currency: raw.currency,
        };
        // Tidied as they are added, so a stored list that isn't is never trusted
        for alias in raw.aliases {
            client.add_alias(&alias);
        }
        Ok(client)
    }
}

impl Client {
    pub fn new(details: ClientDetails) -> Result<Self, DomainError> {
        Self::with_id(ClientId::generate(), details)
    }

    /// A client with a known id, as a database upgrade makes one for every client on record
    pub fn with_id(id: ClientId, details: ClientDetails) -> Result<Self, DomainError> {
        let checked = details.checked()?;
        Ok(Self {
            id,
            name: checked.name,
            aliases: Vec::new(),
            gstin: checked.gstin,
            address: checked.address,
            region: checked.region,
            currency: checked.currency,
        })
    }

    /// Replace the details with newer ones; nothing changes if any is invalid. The earlier
    /// names stay, and details entered under one of them keep the current name, so a project
    /// saved under an old name doesn't rename the client back.
    pub fn update(&mut self, mut details: ClientDetails) -> Result<(), DomainError> {
        if self.is_alias(&details.name) {
            details.name = self.name.clone();
        }
        let mut updated = Self::with_id(self.id, details)?;
        updated.aliases = std::mem::take(&mut self.aliases);
        *self = updated;
        Ok(())
    }

    /// Call the client `name` from now on. The old name is kept as an alias, as their issued
    /// invoices still carry it, unless only its case changes; renaming back to an earlier name
    /// drops that alias.
    pub fn rename(&mut self, name: &str) -> Result<(), DomainError> {
        let name = name.trim();
        require(name, "client name")?;
        if !same_client(&self.name, name) {
            let old = std::mem::replace(&mut self.name, name.to_owned());
            self.aliases.retain(|alias| !same_client(alias, name));
            self.add_alias(&old);
        } else {
            name.clone_into(&mut self.name);
        }
        Ok(())
    }

    /// Take in `other`, a duplicate of this client: their names become aliases of this one.
    /// This client's details win, and what it lacks comes from `other`, so nothing entered is
    /// lost: their GSTIN (unless this client is abroad, where a GSTIN can't apply; the state
    /// then comes from the GSTIN), their state when neither is registered, and their address.
    /// Refused when both have a GSTIN and they differ, as those are two registrations, not one
    /// client entered twice.
    pub fn absorb(&mut self, other: &Client) -> Result<(), DomainError> {
        match (&self.gstin, &other.gstin) {
            (Some(mine), Some(theirs)) if mine != theirs => {
                return Err(DomainError::ClientGstinsDiffer {
                    other: other.name.clone(),
                });
            }
            (None, Some(theirs)) if !self.region.is_abroad() => {
                self.gstin = Some(theirs.clone());
                self.region = ClientRegion::India { state: None };
            }
            // Neither registered: a state only they recorded is still the place of supply
            (None, None) => {
                if let (
                    ClientRegion::India { state: mine @ None },
                    ClientRegion::India {
                        state: Some(theirs),
                    },
                ) = (&mut self.region, &other.region)
                {
                    *mine = Some(theirs.clone());
                }
            }
            _ => {}
        }
        if self.address.is_empty() {
            self.address.clone_from(&other.address);
        }
        for name in std::iter::once(&other.name).chain(&other.aliases) {
            self.add_alias(name);
        }
        Ok(())
    }

    /// Keep `name` as an earlier name, unless it is this client's name or already kept
    fn add_alias(&mut self, name: &str) {
        let name = name.trim();
        if !name.is_empty() && !self.matches(name) {
            self.aliases.push(name.to_owned());
        }
    }

    /// Whether `name` is this client's name spelled with other capitals, `ACME` for `Acme`: a
    /// correction of the name alone, which [`ClientDetails::same_as`] doesn't count
    pub fn recased_by(&self, name: &str) -> bool {
        same_client(&self.name, name) && self.name != name.trim()
    }

    fn is_alias(&self, name: &str) -> bool {
        self.aliases.iter().any(|alias| same_client(alias, name))
    }

    pub fn id(&self) -> ClientId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Names the client was known by before, oldest first
    pub fn aliases(&self) -> &[String] {
        &self.aliases
    }

    pub fn gstin(&self) -> Option<&Gstin> {
        self.gstin.as_ref()
    }

    pub fn address(&self) -> &[String] {
        &self.address
    }

    pub fn region(&self) -> &ClientRegion {
        &self.region
    }

    pub fn currency(&self) -> Currency {
        self.currency
    }

    /// The details as entered, for filling in a project's client
    pub fn details(&self) -> ClientDetails {
        ClientDetails {
            name: self.name.clone(),
            gstin: self.gstin.as_ref().map(|g| g.as_str().to_owned()),
            address: self.address.clone(),
            region: self.region.clone(),
            currency: self.currency,
        }
    }

    /// Whether `name` is this client's name or one of their earlier names, ignoring case and
    /// surrounding spaces
    pub fn matches(&self, name: &str) -> bool {
        same_client(&self.name, name) || self.is_alias(name)
    }

    /// What tells the client apart: their GSTIN, else where they are (`Karnataka (29)`,
    /// `Germany`); `None` for a client in India known by name only
    pub fn place(&self) -> Option<String> {
        self.gstin
            .as_ref()
            .map(ToString::to_string)
            .or_else(|| self.region.location())
    }
}

/// Two client names are one client when they differ only in case and surrounding spaces
pub fn same_client(a: &str, b: &str) -> bool {
    client_key(a) == client_key(b)
}

/// The form of a client name that [`same_client`] compares, for grouping records by client
pub fn client_key(name: &str) -> String {
    name.trim().to_lowercase()
}

/// The clients looked up by any of their names, for grouping projects and invoices (which keep
/// the name they were saved with) under the client they belong to now
#[derive(Debug, Clone, Default)]
pub struct ClientIndex {
    /// [`client_key`] of every name and alias → the client's position, and their current name
    names: HashMap<String, (usize, String)>,
}

impl ClientIndex {
    pub fn new(clients: &[Client]) -> Self {
        let mut names = HashMap::with_capacity(clients.len());
        for (position, client) in clients.iter().enumerate() {
            for name in std::iter::once(&client.name).chain(&client.aliases) {
                names
                    .entry(client_key(name))
                    .or_insert_with(|| (position, client.name.clone()));
            }
        }
        Self { names }
    }

    /// The position, in the clients the index was built from, of the client `name` names
    pub fn find(&self, name: &str) -> Option<usize> {
        self.names
            .get(&client_key(name))
            .map(|(position, _)| *position)
    }

    /// The current name of the client `name` names; `name` itself for a client not on record
    pub fn current_name<'a>(&'a self, name: &'a str) -> &'a str {
        self.names
            .get(&client_key(name))
            .map_or(name, |(_, current)| current.as_str())
    }
}
