use bigdecimal::BigDecimal;
use chrono::NaiveDate;

use crate::demo;
use crate::domain::{Billing, ClientId, Currency, ExchangeRate};
use crate::services::{
    ClientService, ClientSummary, InvoiceService, MilestoneDraft, Owed, ProjectDraft,
    ProjectService, ServiceError,
};

fn rupees(amount: i64) -> Vec<Owed> {
    vec![Owed {
        currency: Currency::Inr,
        amount: BigDecimal::from(amount),
    }]
}

fn named<'a>(clients: &'a [ClientSummary], name: &str) -> &'a ClientSummary {
    clients.iter().find(|c| c.client.name() == name).unwrap()
}

#[test]
fn demo_clients_show_their_projects_and_what_they_owe() {
    let store = demo::store().unwrap();
    let clients = ClientService::new(store.repository()).list().unwrap();

    let names: Vec<_> = clients.iter().map(|c| c.client.name()).collect();
    assert_eq!(
        names,
        [
            "Acme Pvt Ltd",
            "Globex Labs",
            "Initrode GmbH",
            "Umbrella Analytics"
        ]
    );
    assert!(clients.iter().all(|c| c.projects == 1));
    // Dashboards unpaid; half of Frontend; Prototype; the export was paid
    assert_eq!(named(&clients, "Umbrella Analytics").owed, rupees(41_300));
    assert_eq!(named(&clients, "Acme Pvt Ltd").owed, rupees(35_400));
    assert_eq!(named(&clients, "Globex Labs").owed, rupees(18_585));
    assert!(named(&clients, "Initrode GmbH").owed.is_empty());
}

#[test]
fn projects_count_for_a_client_whatever_the_case_of_their_name() {
    let store = demo::store().unwrap();
    let acme = named(
        &ClientService::new(store.repository()).list().unwrap(),
        "Acme Pvt Ltd",
    )
    .client
    .clone();
    let mut projects = ProjectService::new(store.repository());
    projects
        .create(ProjectDraft {
            name: "Brochure".to_owned(),
            client: acme.details(),
            billing: Billing::Fixed,
        })
        .unwrap();
    let mut lower = acme.details();
    lower.name = " acme pvt ltd ".to_owned();
    projects
        .create(ProjectDraft {
            name: "Annual report".to_owned(),
            client: lower,
            billing: Billing::Fixed,
        })
        .unwrap();

    let service = ClientService::new(store.repository());
    let clients = service.list().unwrap();
    assert_eq!(clients.len(), 4);
    assert_eq!(clients[0].projects, 3);

    let account = service.account(clients[0].client.id()).unwrap();
    let names: Vec<_> = account.projects.iter().map(|p| p.name()).collect();
    assert_eq!(names, ["Annual report", "Brochure", "Website Revamp"]);
    let numbers: Vec<_> = account
        .invoices
        .iter()
        .map(|i| i.number().as_str())
        .collect();
    assert_eq!(numbers.len(), 2);
    // Newest first: Frontend after Design
    assert!(account.invoices[0].date() > account.invoices[1].date());
    assert_eq!(account.owed, rupees(35_400));
}

#[test]
fn a_foreign_client_owes_in_their_own_currency() {
    let store = demo::store().unwrap();
    let service = ClientService::new(store.repository());
    let initrode = named(&service.list().unwrap(), "Initrode GmbH").client.id();
    let audit = service.account(initrode).unwrap().projects[0].id();

    let mut projects = ProjectService::new(store.repository());
    let project = projects
        .add_milestone(
            audit,
            MilestoneDraft {
                name: "Follow-up".to_owned(),
                due_date: NaiveDate::from_ymd_opt(2026, 10, 1).unwrap(),
                amount: Some(BigDecimal::from(250)),
            },
        )
        .unwrap();
    let follow_up = project.milestones().last().unwrap().id();
    InvoiceService::new(store.repository(), None)
        .issue(
            audit,
            &[follow_up],
            NaiveDate::from_ymd_opt(2026, 10, 2).unwrap(),
            Some(ExchangeRate::new(BigDecimal::from(90)).unwrap()),
            None,
        )
        .unwrap();

    let account = service.account(initrode).unwrap();
    assert_eq!(
        account.owed,
        [Owed {
            currency: Currency::Eur,
            amount: BigDecimal::from(250),
        }]
    );
    assert_eq!(account.invoices.len(), 2);
}

#[test]
fn an_unknown_client_is_refused() {
    let store = demo::store().unwrap();
    let error = ClientService::new(store.repository())
        .account(ClientId::generate())
        .unwrap_err();
    assert!(matches!(error, ServiceError::NoSuchClient(_)));
}
