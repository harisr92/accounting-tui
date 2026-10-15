//! Use-cases: load a record, apply a domain change, save it. Views call these, never storage.

mod error;
mod project_service;

pub use error::ServiceError;
pub use project_service::{MilestoneDraft, ProjectDraft, ProjectService};

#[cfg(test)]
mod tests;
