//! Clients billed before, and the GST states a client in India can be in, for the project form

use flutter_rust_bridge::frb;

use compter_core::domain::{Client, ClientRegion, state_label, states};
use compter_core::input;

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
        let place = client
            .gstin()
            .map(ToString::to_string)
            .or_else(|| location(client.region()));
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

/// `Karnataka (29)` or `Germany`; `None` for a client in India whose state isn't known
pub(crate) fn location(region: &ClientRegion) -> Option<String> {
    match region {
        ClientRegion::India { state: Some(code) } => Some(state_label(code.as_str())),
        ClientRegion::India { state: None } => None,
        ClientRegion::Abroad { country } => Some(country.clone()),
    }
}
