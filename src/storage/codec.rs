//! The one place values are turned into bytes and back

use std::fmt::Display;

use serde::Serialize;
use serde::de::DeserializeOwned;

use super::StorageError;

pub(super) fn encode<T: Serialize>(
    table: &'static str,
    key: impl Display,
    value: &T,
) -> Result<Vec<u8>, StorageError> {
    serde_json::to_vec(value).map_err(|source| codec_error(table, key, source))
}

/// Decoding runs the type's own validation (`#[serde(try_from = ...)]` on domain types)
pub(super) fn decode<T: DeserializeOwned>(
    table: &'static str,
    key: impl Display,
    bytes: &[u8],
) -> Result<T, StorageError> {
    serde_json::from_slice(bytes).map_err(|source| codec_error(table, key, source))
}

fn codec_error(table: &'static str, key: impl Display, source: serde_json::Error) -> StorageError {
    StorageError::Codec {
        table,
        key: key.to_string(),
        source,
    }
}
