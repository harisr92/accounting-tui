use crate::domain::DomainError;
use crate::storage::StorageError;

/// A use-case failed: either the change broke a domain rule or storage failed
#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    #[error(transparent)]
    Domain(#[from] DomainError),
    #[error(transparent)]
    Storage(#[from] StorageError),
}
