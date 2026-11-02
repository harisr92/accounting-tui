use accounting_core::StateCode;

use crate::domain::{
    Client, ClientDetails, ClientIndex, ClientRegion, Currency, DomainError, client_key,
    same_client,
};

const GSTIN: &str = "29AAPFU0939F1ZR";
const OTHER_GSTIN: &str = "27AAPFU0939F2ZU";

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
        ..ClientDetails::named("Acme ", Some("27AAPFU0939F2ZU"))
    };
    assert!(stored.same_as(&tidied));

    let acme = Client::new(ClientDetails::named("Acme", None)).unwrap();
    assert!(acme.recased_by(" ACME") && !acme.recased_by("Acme ") && !acme.recased_by("Globex"));

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

#[test]
fn a_renamed_client_keeps_the_old_name_as_an_alias_and_still_matches_it() {
    let mut client = Client::new(ClientDetails::named("Acme", None)).unwrap();

    client.rename(" Acme Pvt Ltd ").unwrap();
    assert_eq!(client.name(), "Acme Pvt Ltd");
    assert_eq!(client.aliases(), ["Acme"]);
    assert!(client.matches("acme") && client.matches("ACME PVT LTD"));

    // Only the case: no alias
    client.rename("ACME Pvt Ltd").unwrap();
    assert_eq!(client.name(), "ACME Pvt Ltd");
    assert_eq!(client.aliases(), ["Acme"]);

    // Back to an earlier name: that alias goes, the name before it comes in
    client.rename("acme").unwrap();
    assert_eq!(client.name(), "acme");
    assert_eq!(client.aliases(), ["ACME Pvt Ltd"]);

    let before = client.clone();
    assert!(matches!(
        client.rename("  "),
        Err(DomainError::EmptyField(_))
    ));
    assert_eq!(client, before);
}

#[test]
fn absorbing_a_duplicate_takes_their_names_and_keeps_these_details() {
    let mut acme = Client::new(ClientDetails {
        address: vec!["Plot 7".to_owned()],
        ..ClientDetails::named("Acme Pvt Ltd", Some(GSTIN))
    })
    .unwrap();
    let mut typo = Client::new(ClientDetails {
        address: vec!["Elsewhere".to_owned()],
        ..ClientDetails::named("Acme Pvt", None)
    })
    .unwrap();
    typo.rename("Acmee").unwrap();

    acme.absorb(&typo).unwrap();
    assert_eq!(acme.name(), "Acme Pvt Ltd");
    assert_eq!(acme.aliases(), ["Acmee", "Acme Pvt"]);
    assert_eq!(acme.gstin().map(|g| g.as_str()), Some(GSTIN));
    assert_eq!(acme.address(), ["Plot 7"]);
    assert!(acme.matches("acme pvt"));

    // The same names again add nothing
    acme.absorb(&typo).unwrap();
    assert_eq!(acme.aliases().len(), 2);
}

#[test]
fn clients_with_different_gstins_are_not_merged() {
    let mut acme = Client::new(ClientDetails::named("Acme", Some(GSTIN))).unwrap();
    let other = Client::new(ClientDetails::named("Acme MH", Some(OTHER_GSTIN))).unwrap();
    let same = Client::new(ClientDetails::named("ACME India", Some(GSTIN))).unwrap();

    let before = acme.clone();
    let error = acme.absorb(&other).unwrap_err();
    assert!(matches!(error, DomainError::ClientGstinsDiffer { other } if other == "Acme MH"));
    assert_eq!(acme, before);
    acme.absorb(&same).unwrap();
}

#[test]
fn an_update_keeps_the_aliases_and_one_under_an_old_name_keeps_the_current_name() {
    let mut client = Client::new(ClientDetails::named("Acme", None)).unwrap();
    client.rename("Acme Pvt Ltd").unwrap();

    client
        .update(ClientDetails {
            address: vec!["Plot 7".to_owned()],
            ..ClientDetails::named("ACME", None)
        })
        .unwrap();
    assert_eq!(client.name(), "Acme Pvt Ltd");
    assert_eq!(client.aliases(), ["Acme"]);
    assert_eq!(client.address(), ["Plot 7"]);
}

#[test]
fn aliases_load_back_tidied() {
    let mut client = Client::new(ClientDetails::named("Acme", None)).unwrap();
    client.rename("Acme Pvt Ltd").unwrap();
    let json = serde_json::to_string(&client).unwrap();
    assert_eq!(serde_json::from_str::<Client>(&json).unwrap(), client);

    let untidy = json.replace(r#"["Acme"]"#, r#"["Acme", " acme ", "ACME PVT LTD", ""]"#);
    let loaded: Client = serde_json::from_str(&untidy).unwrap();
    assert_eq!(loaded.aliases(), ["Acme"]);
}

#[test]
fn the_index_finds_a_client_by_any_of_their_names() {
    let mut acme = Client::new(ClientDetails::named("Acme Pvt Ltd", None)).unwrap();
    acme.rename("Acme Ltd").unwrap();
    let globex = Client::new(ClientDetails::named("Globex", None)).unwrap();
    let index = ClientIndex::new(&[acme, globex]);

    assert_eq!(index.find(" acme pvt ltd"), Some(0));
    assert_eq!(index.find("GLOBEX"), Some(1));
    assert_eq!(index.find("Initech"), None);
    assert_eq!(index.current_name("acme pvt ltd"), "Acme Ltd");
    assert_eq!(index.current_name("Initech"), "Initech");
}

#[test]
fn absorbing_fills_in_the_gstin_and_address_this_client_lacks() {
    let mut bare = Client::new(ClientDetails {
        region: ClientRegion::India {
            state: StateCode::parse("27").ok(),
        },
        ..ClientDetails::named("Acme", None)
    })
    .unwrap();
    let full = Client::new(ClientDetails {
        address: vec!["Plot 7".to_owned(), "Bengaluru".to_owned()],
        ..ClientDetails::named("Acme Pvt Ltd", Some(GSTIN))
    })
    .unwrap();

    bare.absorb(&full).unwrap();
    assert_eq!(bare.name(), "Acme");
    assert_eq!(bare.gstin().map(|g| g.as_str()), Some(GSTIN));
    // The state is the GSTIN's now
    assert_eq!(bare.region(), &ClientRegion::India { state: None });
    assert_eq!(bare.address(), ["Plot 7", "Bengaluru"]);

    // Neither registered: the state only the duplicate recorded comes across
    let mut stateless = Client::new(ClientDetails::named("Initech", None)).unwrap();
    let placed = Client::new(ClientDetails {
        region: ClientRegion::India {
            state: StateCode::parse("27").ok(),
        },
        ..ClientDetails::named("Initech Ltd", None)
    })
    .unwrap();
    stateless.absorb(&placed).unwrap();
    assert_eq!(stateless.region(), placed.region());

    // A client abroad takes no GSTIN
    let mut abroad_client = Client::new(ClientDetails {
        region: abroad("Germany"),
        ..ClientDetails::named("Acme GmbH", None)
    })
    .unwrap();
    abroad_client.absorb(&full).unwrap();
    assert!(abroad_client.gstin().is_none());
    assert!(abroad_client.region().is_abroad());
}
