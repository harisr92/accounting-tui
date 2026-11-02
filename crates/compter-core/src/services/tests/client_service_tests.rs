use bigdecimal::BigDecimal;
use chrono::NaiveDate;

use crate::demo;
use crate::domain::{
    Billing, ClientDetails, ClientId, ClientRegion, Currency, DomainError, ExchangeRate, ProjectId,
};
use crate::services::{
    ClientService, ClientSummary, InvoiceService, MilestoneDraft, Owed, ProjectDraft,
    ProjectService, ServiceError,
};
use crate::storage::{
    ClientRepository, InvoiceRepository, ProjectRepository, RedbRepository, Store,
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

/// The demo's Acme with a corrected address and a GSTIN from the same state
fn acme_corrected(service: &ClientService<RedbRepository>) -> (ClientId, ClientDetails) {
    let acme = named(&service.list().unwrap(), "Acme Pvt Ltd")
        .client
        .clone();
    let details = ClientDetails {
        address: vec!["Plot 9, MIDC".to_owned(), "Pune 411019".to_owned()],
        gstin: Some("27AAPFU0939F1ZV".to_owned()),
        ..acme.details()
    };
    (acme.id(), details)
}

/// A second, finished and archived project for Acme: its only milestone invoiced
fn finished_acme_project(store: &Store, acme: ClientDetails) -> ProjectId {
    let mut projects = ProjectService::new(store.repository());
    let project = projects
        .create(ProjectDraft {
            name: "Logo".to_owned(),
            client: acme,
            billing: Billing::Fixed,
        })
        .unwrap();
    let project = projects
        .add_milestone(
            project.id(),
            MilestoneDraft {
                name: "Logo".to_owned(),
                due_date: NaiveDate::from_ymd_opt(2026, 10, 1).unwrap(),
                amount: Some(BigDecimal::from(10_000)),
            },
        )
        .unwrap();
    let milestone = project.milestones()[0].id();
    InvoiceService::new(store.repository(), None)
        .issue(
            project.id(),
            &[milestone],
            NaiveDate::from_ymd_opt(2026, 10, 2).unwrap(),
            None,
            None,
        )
        .unwrap();
    projects.set_archived(project.id(), true).unwrap();
    project.id()
}

#[test]
fn a_corrected_client_offers_and_updates_only_their_open_projects() {
    let store = demo::store().unwrap();
    let mut service = ClientService::new(store.repository());
    let (id, corrected) = acme_corrected(&service);
    let logo = finished_acme_project(&store, service.account(id).unwrap().client.details());
    let invoices_before = store.repository().list_invoices().unwrap();

    let offered: Vec<_> = service
        .projects_to_update(id, &corrected)
        .unwrap()
        .iter()
        .map(|p| p.name().to_owned())
        .collect();
    assert_eq!(offered, ["Website Revamp"]);

    let account = service.update(id, corrected.clone(), true).unwrap();
    assert!(account.client.details().same_as(&corrected));
    let repo = store.repository();
    let website = account
        .projects
        .iter()
        .find(|p| p.name() == "Website Revamp")
        .unwrap();
    assert!(
        repo.load(website.id())
            .unwrap()
            .client_details()
            .same_as(&corrected)
    );
    // The archived project and every issued invoice keep what they were billed with
    assert_eq!(
        repo.load(logo).unwrap().client_address(),
        ["Plot 7, MIDC", "Pune 411019"]
    );
    assert_eq!(repo.list_invoices().unwrap(), invoices_before);
    assert!(
        service
            .projects_to_update(id, &corrected)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn a_corrected_client_can_leave_their_projects_as_they_are() {
    let store = demo::store().unwrap();
    let mut service = ClientService::new(store.repository());
    let (id, corrected) = acme_corrected(&service);

    let account = service.update(id, corrected.clone(), false).unwrap();
    assert!(account.client.details().same_as(&corrected));
    let website = store.repository().load(account.projects[0].id()).unwrap();
    assert_eq!(website.client_address(), ["Plot 7, MIDC", "Pune 411019"]);
    assert_eq!(service.projects_to_update(id, &corrected).unwrap().len(), 1);
}

#[test]
fn a_clients_name_cannot_be_changed() {
    let store = demo::store().unwrap();
    let mut service = ClientService::new(store.repository());
    let (id, mut corrected) = acme_corrected(&service);
    corrected.name = "Acme Private Limited".to_owned();

    let error = service.update(id, corrected, true).unwrap_err();
    assert!(matches!(error, ServiceError::ClientNameFixed));
    assert_eq!(
        service.account(id).unwrap().client.address(),
        ["Plot 7, MIDC", "Pune 411019"]
    );
}

#[test]
fn a_project_that_refuses_the_correction_saves_nothing() {
    let store = demo::store().unwrap();
    let mut service = ClientService::new(store.repository());
    let initrode = named(&service.list().unwrap(), "Initrode GmbH")
        .client
        .clone();
    // API Audit, finished but not archived, is billed in euros: it can't follow them to India
    let before = store.repository().clients().unwrap();
    let moved = ClientDetails {
        region: ClientRegion::India { state: None },
        currency: Currency::Inr,
        ..initrode.details()
    };

    let error = service
        .update(initrode.id(), moved.clone(), true)
        .unwrap_err();
    assert!(matches!(
        error,
        ServiceError::ProjectRefusedClient { ref project, ref source }
            if project == "API Audit" && matches!(**source, DomainError::ForeignCurrencyInIndia)
    ));
    assert_eq!(store.repository().clients().unwrap(), before);
    // Without the projects, the client alone is corrected
    service.update(initrode.id(), moved, false).unwrap();
}

#[test]
fn an_unchanged_client_is_not_saved_again() {
    let store = demo::store().unwrap();
    let mut service = ClientService::new(store.repository());
    let acme = named(&service.list().unwrap(), "Acme Pvt Ltd")
        .client
        .clone();
    let mut tidied = acme.details();
    tidied.name = " Acme Pvt Ltd ".to_owned();
    tidied.address.push(String::new());

    let account = service.update(acme.id(), tidied, true).unwrap();
    assert_eq!(account.client, acme);
}

#[test]
fn a_bad_gstin_is_refused_as_the_clients_not_a_projects() {
    let store = demo::store().unwrap();
    let mut service = ClientService::new(store.repository());
    let (id, mut corrected) = acme_corrected(&service);
    corrected.gstin = Some("27NOTAGSTIN".to_owned());

    let error = service.update(id, corrected, true).unwrap_err();
    assert!(matches!(error, ServiceError::Domain(_)), "{error:?}");
}
