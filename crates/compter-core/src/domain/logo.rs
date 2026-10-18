use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::DomainError;

/// Keeps the database small; a logo printed 30 mm wide needs far less
pub const MAX_LOGO_BYTES: usize = 1024 * 1024;

const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
const JPEG_SIGNATURE: &[u8] = &[0xFF, 0xD8, 0xFF];

/// Which stored logo an invoice was issued with. Logos are never overwritten: a new one gets a
/// new id, so an invoice always reprints with the logo it was issued under.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LogoId(Uuid);

impl LogoId {
    pub fn generate() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(id: Uuid) -> Self {
        Self(id)
    }

    pub fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl fmt::Display for LogoId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Image formats accounting-core can draw on a PDF
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogoFormat {
    Png,
    Jpeg,
}

impl LogoFormat {
    pub fn label(self) -> &'static str {
        match self {
            Self::Png => "PNG",
            Self::Jpeg => "JPEG",
        }
    }
}

/// The business logo printed at the top left of invoices: PNG or JPEG bytes
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Logo {
    id: LogoId,
    format: LogoFormat,
    bytes: Vec<u8>,
}

impl Logo {
    /// A new logo with a fresh id
    pub fn new(bytes: Vec<u8>) -> Result<Self, DomainError> {
        Self::from_stored(LogoId::generate(), bytes)
    }

    /// A logo read back from storage, checked like a new one
    pub fn from_stored(id: LogoId, bytes: Vec<u8>) -> Result<Self, DomainError> {
        if bytes.len() > MAX_LOGO_BYTES {
            return Err(DomainError::LogoTooLarge { bytes: bytes.len() });
        }
        let format = if bytes.starts_with(PNG_SIGNATURE) {
            LogoFormat::Png
        } else if bytes.starts_with(JPEG_SIGNATURE) {
            LogoFormat::Jpeg
        } else {
            return Err(DomainError::InvalidLogo);
        };
        Ok(Self { id, format, bytes })
    }

    pub fn id(&self) -> LogoId {
        self.id
    }

    pub fn format(&self) -> LogoFormat {
        self.format
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}
