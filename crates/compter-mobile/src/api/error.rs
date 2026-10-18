use compter_core::services::ServiceError;

/// Why a call failed, as Dart shows it. `field` names the input that was refused, so a form can
/// show the message next to it; it is `None` for anything else.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct ApiError {
    pub field: Option<String>,
    pub message: String,
}

impl ApiError {
    pub(crate) fn invalid(field: &str, message: impl Into<String>) -> Self {
        Self {
            field: Some(field.to_owned()),
            message: message.into(),
        }
    }
}

impl From<ServiceError> for ApiError {
    fn from(error: ServiceError) -> Self {
        Self {
            field: None,
            message: error.to_string(),
        }
    }
}
