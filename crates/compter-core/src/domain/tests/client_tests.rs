use accounting_core::StateCode;

use crate::domain::{
    Client, ClientDetails, ClientRegion, Currency, DomainError, client_key, same_client,
};

fn abroad(country: &str) -> ClientRegion {
    ClientRegion::Abroad {
        country: country.to_owned(),
    }
}

#[test]
fn a_client_keeps_what_was_entered_without_blank_address_lines() {
    let client = Client::new(ClientDetails {
        name: "Acme Pvt Ltd".to_owned(),
        gstin: Some("29AAPFU0939F1ZR".to_owned()),
        address: vec![" Plot 7 ".to_owned(), "  ".to_owned(), "Pune".to_owned()],
        region: ClientRegion::default(),
        currency: Currency::Inr,
    })
    .unwrap();

    assert_eq!(client.name(), "Acme Pvt Ltd");
    assert_eq!(client.gstin().unwrap().as_str(), "29AAPFU0939F1ZR");
    assert_eq!(client.address(), ["Plot 7", "Pune"]);
    assert_eq!(client.details().address, ["Plot 7", "Pune"]);
}

#[test]
fn a_client_meets_the_same_rules_as_a_projects_client() {
    let nameless = Client::new(ClientDetails::named("  ", None));
    assert!(matches!(nameless, Err(DomainError::EmptyField(_))));

    let bad_gstin = Client::new(ClientDetails::named("Acme", Some("29ABC")));
    assert!(matches!(bad_gstin, Err(DomainError::InvalidGstin(_))));

    let foreign_with_gstin = Client::new(ClientDetails {
        region: abroad("Germany"),
        ..ClientDetails::named("Initrode", Some("29AAPFU0939F1ZR"))
    });
    assert!(matches!(
        foreign_with_gstin,
        Err(DomainError::ForeignClientWithGstin)
    ));

    let euros_in_india = Client::new(ClientDetails {
        currency: Currency::Eur,
        ..ClientDetails::named("Acme", None)
    });
    assert!(matches!(
        euros_in_india,
        Err(DomainError::ForeignCurrencyInIndia)
    ));
}

#[test]
fn an_update_replaces_the_details_but_keeps_the_id_and_a_bad_one_changes_nothing() {
    let mut client = Client::new(ClientDetails::named("Acme", None)).unwrap();
    let id = client.id();

    client
        .update(ClientDetails {
            region: abroad("Germany"),
            currency: Currency::Eur,
            ..ClientDetails::named("Acme GmbH", None)
        })
        .unwrap();
    assert_eq!(client.id(), id);
    assert_eq!(client.name(), "Acme GmbH");
    assert_eq!(client.currency(), Currency::Eur);

    let before = client.clone();
    assert!(client.update(ClientDetails::named("", None)).is_err());
    assert_eq!(client, before);
}

#[test]
fn a_client_is_matched_by_name_ignoring_case_and_spaces() {
    let client = Client::new(ClientDetails::named("Acme Pvt Ltd", None)).unwrap();

    assert!(client.matches("  acme pvt ltd "));
    assert!(!client.matches("Acme"));
    assert!(same_client("ACME", "acme"));
}

#[test]
fn a_stored_client_loads_back_and_an_invalid_one_is_refused() {
    let client = Client::new(ClientDetails {
        region: ClientRegion::India {
            state: StateCode::parse("29").ok(),
        },
        ..ClientDetails::named("Globex", None)
    })
    .unwrap();

    let json = serde_json::to_string(&client).unwrap();
    assert_eq!(serde_json::from_str::<Client>(&json).unwrap(), client);

    let invalid = json.replace("Globex", " ");
    assert!(serde_json::from_str::<Client>(&invalid).is_err());
}

#[test]
fn a_state_as_well_as_a_gstin_must_be_the_gstins() {
    let with_state = |code: &str| ClientDetails {
        region: ClientRegion::India {
            state: StateCode::parse(code).ok(),
        },
        ..ClientDetails::named("Acme", Some("27AAPFU0939F2ZU"))
    };

    let error = Client::new(with_state("29")).unwrap_err();
    assert!(matches!(&error, DomainError::StateNotGstins(s) if s == "Maharashtra (27)"));
    assert!(Client::new(with_state("27")).is_ok());
}

#[test]
fn tidying_alone_is_not_a_different_client() {
    let stored = ClientDetails {
        address: vec!["Plot 7".to_owned()],
        region: ClientRegion::India {
            state: StateCode::parse("27").ok(),
        },
        ..ClientDetails::named(" Acme", Some("27aapfu0939f2zu"))
    };
    let tidied = ClientDetails {
        address: vec![" Plot 7 ".to_owned(), String::new()],
        ..ClientDetails::named("acme", Some("27AAPFU0939F2ZU"))
    };
    assert!(stored.same_as(&tidied));

    let moved = ClientDetails {
        address: vec!["Plot 9".to_owned()],
        ..tidied.clone()
    };
    assert!(!tidied.same_as(&moved));
}

#[test]
fn a_client_is_told_apart_by_gstin_else_by_place() {
    let registered = Client::new(ClientDetails::named("Acme", Some("29aapfu0939f1zr"))).unwrap();
    assert_eq!(registered.place().as_deref(), Some("29AAPFU0939F1ZR"));

    let mut details = ClientDetails::named("Globex", None);
    details.region = ClientRegion::India {
        state: accounting_core::StateCode::parse("29").ok(),
    };
    let in_state = Client::new(details).unwrap();
    assert_eq!(in_state.place().as_deref(), Some("Karnataka (29)"));

    let mut details = ClientDetails::named("Initrode", None);
    details.region = ClientRegion::Abroad {
        country: "Germany".to_owned(),
    };
    details.currency = Currency::Eur;
    assert_eq!(
        Client::new(details).unwrap().place().as_deref(),
        Some("Germany")
    );

    assert_eq!(
        Client::new(ClientDetails::named("Hooli", None))
            .unwrap()
            .place(),
        None
    );
}

#[test]
fn client_keys_ignore_case_and_surrounding_spaces() {
    assert_eq!(client_key("  Acme Pvt Ltd "), client_key("ACME PVT LTD"));
    assert_ne!(client_key("Acme"), client_key("Acme Pvt Ltd"));
}
