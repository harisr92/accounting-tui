//! Helpers shared by the API test files

use compter_mobile::api::{CompterApp, MilestoneInput, ProjectInput};
use tempfile::TempDir;

/// The demo business, writing its PDFs to a scratch folder kept as long as the `TempDir`
pub fn demo() -> (TempDir, CompterApp) {
    let scratch = tempfile::tempdir().unwrap();
    let app = CompterApp::demo(scratch.path().to_string_lossy().into_owned()).unwrap();
    (scratch, app)
}

/// A client in Karnataka without a GSTIN, billed in rupees
pub fn project(name: &str, hourly_rate: Option<&str>) -> ProjectInput {
    ProjectInput {
        name: name.to_owned(),
        client_name: "Acme Pvt Ltd".to_owned(),
        client_gstin: None,
        client_state: Some("29".to_owned()),
        client_country: None,
        client_address: vec!["MG Road".to_owned(), "Bengaluru 560001".to_owned()],
        currency: None,
        hourly_rate: hourly_rate.map(str::to_owned),
    }
}

pub fn milestone(name: &str, due_date: &str, amount: Option<&str>) -> MilestoneInput {
    MilestoneInput {
        name: name.to_owned(),
        due_date: due_date.to_owned(),
        amount: amount.map(str::to_owned),
    }
}
