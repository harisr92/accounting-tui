//! A client's details as form fields, shared by the project form (Projects tab) and the edit
//! client form (Clients tab), so both read a client the same way

use accounting_core::StateCode;

use crate::ui::form::{Field, Form, ListItem, parse};
use compter_core::domain::{ClientDetails, ClientRegion, Currency, state_name, states};

const REGION_OPTIONS: &[&str] = &["India", "Abroad"];
const ABROAD: usize = 1;
/// Address lines a client can have; plenty for a street, a building, a city and a postcode
const ADDRESS_LINES: usize = 6;

/// Positions of the client fields, counted from the first of them in a form
mod field {
    pub const GSTIN: usize = 0;
    pub const REGION: usize = 1;
    pub const STATE: usize = 2;
    pub const COUNTRY: usize = 3;
    pub const ADDRESS: usize = 4;
    pub const CURRENCY: usize = 5;
}

/// How many fields [`fields`] adds
pub const COUNT: usize = 6;

/// GSTIN, where the client is, the address and the currency, filled from `client`. The name
/// isn't among them: the project form asks for it, and a client's own name can't change.
pub fn fields(client: &ClientDetails) -> Vec<Field> {
    let shown = Shown::of(client);
    vec![
        Field::text("GSTIN", "blank if unregistered or abroad").with_value(shown.gstin),
        Field::choice("Client in", REGION_OPTIONS).with_choice(shown.region),
        Field::text("State", "India, no GSTIN: a code like 29, or Ctrl-l")
            .with_value(shown.state)
            .with_preview(describe_state)
            .with_list("States", state_list())
            .derived_from(
                field::STATE - field::GSTIN,
                parse::gstin_state,
                "from the client's GSTIN",
            ),
        Field::text("Country", "abroad only, e.g. Germany").with_value(shown.country),
        Field::lines(
            "Address",
            "street, then city and PIN / postcode",
            ADDRESS_LINES,
        )
        .with_value(shown.address),
        Field::choice("Currency", Currency::LABELS).with_choice(shown.currency),
    ]
}

/// Set the client fields starting at `first` from `client`
pub fn fill(form: &mut Form, first: usize, client: &ClientDetails) {
    let shown = Shown::of(client);
    form.set_value(first + field::GSTIN, shown.gstin);
    form.set_choice(first + field::REGION, shown.region);
    form.set_value(first + field::STATE, shown.state);
    form.set_value(first + field::COUNTRY, shown.country);
    form.set_value(first + field::ADDRESS, shown.address);
    form.set_choice(first + field::CURRENCY, shown.currency);
}

/// A client as the fields show them
struct Shown<'a> {
    gstin: &'a str,
    region: usize,
    state: &'a str,
    country: &'a str,
    /// A line break between lines, as a lines field is set
    address: String,
    currency: usize,
}

impl<'a> Shown<'a> {
    fn of(client: &'a ClientDetails) -> Self {
        let (region, state, country) = match &client.region {
            ClientRegion::India { state } => (0, state.as_ref().map_or("", |s| s.as_str()), ""),
            ClientRegion::Abroad { country } => (ABROAD, "", country.as_str()),
        };
        Self {
            gstin: client.gstin.as_deref().unwrap_or_default(),
            region,
            state,
            country,
            address: client.address.join("\n"),
            currency: client.currency.index(),
        }
    }
}

/// The client called `name` as typed into the fields starting at `first`
pub fn read(form: &Form, first: usize, name: &str) -> Result<ClientDetails, String> {
    let value = |index: usize| form.value(first + index);
    let region = if form.choice(first + field::REGION) == ABROAD {
        ClientRegion::Abroad {
            country: value(field::COUNTRY).trim().to_owned(),
        }
    } else {
        let state = parse::optional(value(field::STATE))
            .map(|code| StateCode::parse(&code))
            .transpose()
            .map_err(|e| e.to_string())?;
        ClientRegion::India { state }
    };
    // A GSTIN on a client abroad is refused by the domain rather than silently dropped
    let gstin = parse::optional(value(field::GSTIN)).map(|g| g.to_uppercase());
    Ok(ClientDetails {
        name: name.trim().to_owned(),
        gstin,
        address: form.lines(first + field::ADDRESS),
        region,
        currency: Currency::from_index(form.choice(first + field::CURRENCY)),
    })
}

/// Every GST state to pick from, labelled as core labels them; the code goes in the field
fn state_list() -> Vec<(String, ListItem)> {
    states()
        .map(|state| (state.code().to_owned(), ListItem::new(state.label(), "")))
        .collect()
}

/// Name the state a typed GST state code stands for
fn describe_state(input: &str) -> String {
    state_name(input.trim()).map_or_else(|| "not a GST state code".to_owned(), str::to_owned)
}
