use accounting_core::{Gstin, HsnMaster, HsnSacEntry};
use bigdecimal::BigDecimal;
use serde::{Deserialize, Serialize};

use super::error::require;
use super::{Currency, DomainError};

/// SAC for IT design and development services, the usual freelance software code
pub const DEFAULT_SAC: &str = "998314";
pub const DEFAULT_INVOICE_PREFIX: &str = "INV";
/// Keeps `PREFIX/26-27/0001` within the 16-character limit of Rule 46
const MAX_PREFIX_LEN: usize = 5;

/// The profile as entered in Settings; becomes a [`BusinessProfile`] once validated
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ProfileDraft {
    pub name: String,
    pub address: Vec<String>,
    /// Blank or `None` when not registered under GST
    #[serde(default)]
    pub gstin: Option<String>,
    pub sac_code: String,
    pub invoice_prefix: String,
    /// Letter of Undertaking ARN, needed to invoice foreign clients without IGST
    #[serde(default)]
    pub lut_arn: Option<String>,
    /// Printed at the end of every invoice: bank details, payment terms
    #[serde(default)]
    pub terms: Vec<String>,
    /// What new projects are billed in; clients in India are always billed in INR
    #[serde(default)]
    pub currency: Currency,
}

/// Who is issuing invoices: the freelancer's business
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ProfileDraft")]
pub struct BusinessProfile {
    name: String,
    address: Vec<String>,
    gstin: Option<Gstin>,
    sac_code: String,
    invoice_prefix: String,
    lut_arn: Option<String>,
    terms: Vec<String>,
    currency: Currency,
}

impl TryFrom<ProfileDraft> for BusinessProfile {
    type Error = DomainError;

    fn try_from(draft: ProfileDraft) -> Result<Self, Self::Error> {
        require(&draft.name, "business name")?;
        let address = non_blank(draft.address);
        if address.is_empty() {
            return Err(DomainError::EmptyField("business address"));
        }
        let gstin = draft
            .gstin
            .as_deref()
            .map(str::trim)
            .filter(|g| !g.is_empty())
            .map(|g| Gstin::parse(&g.to_uppercase()))
            .transpose()?;
        let sac_code = draft.sac_code.trim().to_owned();
        if HsnMaster::global().lookup(&sac_code).is_none() {
            return Err(DomainError::UnknownSacCode(sac_code));
        }
        let invoice_prefix = draft.invoice_prefix.trim().to_uppercase();
        let prefix_ok = (1..=MAX_PREFIX_LEN).contains(&invoice_prefix.len())
            && invoice_prefix
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-');
        if !prefix_ok {
            return Err(DomainError::InvalidInvoicePrefix);
        }
        let lut_arn = draft
            .lut_arn
            .map(|arn| arn.trim().to_owned())
            .filter(|arn| !arn.is_empty());
        Ok(Self {
            name: draft.name.trim().to_owned(),
            address,
            gstin,
            sac_code,
            invoice_prefix,
            lut_arn,
            terms: non_blank(draft.terms),
            currency: draft.currency,
        })
    }
}

impl BusinessProfile {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn address(&self) -> &[String] {
        &self.address
    }

    /// `None` when the business is not registered under GST
    pub fn gstin(&self) -> Option<&Gstin> {
        self.gstin.as_ref()
    }

    pub fn sac_code(&self) -> &str {
        &self.sac_code
    }

    /// The SAC's description and default GST rate (validated to exist)
    pub fn sac_entry(&self) -> Option<&'static HsnSacEntry> {
        HsnMaster::global().lookup(&self.sac_code)
    }

    pub fn gst_rate(&self) -> Option<&'static BigDecimal> {
        self.sac_entry().map(|entry| &entry.gst_rate)
    }

    pub fn invoice_prefix(&self) -> &str {
        &self.invoice_prefix
    }

    pub fn lut_arn(&self) -> Option<&str> {
        self.lut_arn.as_deref()
    }

    pub fn terms(&self) -> &[String] {
        &self.terms
    }

    /// Default billing currency for new projects
    pub fn currency(&self) -> Currency {
        self.currency
    }

    /// The profile as entered, for prefilling the Settings form
    pub fn to_draft(&self) -> ProfileDraft {
        ProfileDraft {
            name: self.name.clone(),
            address: self.address.clone(),
            gstin: self.gstin.as_ref().map(|g| g.as_str().to_owned()),
            sac_code: self.sac_code.clone(),
            invoice_prefix: self.invoice_prefix.clone(),
            lut_arn: self.lut_arn.clone(),
            terms: self.terms.clone(),
            currency: self.currency,
        }
    }
}

fn non_blank(lines: Vec<String>) -> Vec<String> {
    lines
        .into_iter()
        .map(|line| line.trim().to_owned())
        .filter(|line| !line.is_empty())
        .collect()
}
