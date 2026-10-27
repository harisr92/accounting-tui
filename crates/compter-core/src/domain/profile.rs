use accounting_core::{Gstin, HsnMaster, HsnSacEntry};
use bigdecimal::BigDecimal;
use serde::{Deserialize, Serialize};

use super::error::require;
use super::{Currency, DomainError, FiscalYear, LogoId};

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
    /// Printed on new invoices; set from Settings, stored in its own table
    #[serde(default)]
    pub logo: Option<LogoId>,
    /// How the Reports tab counts gross receipts for 44ADA, year by year
    #[serde(default)]
    pub income_basis: BasisSchedule,
}

/// When fees count as gross receipts for income tax: the accounting method under section 145
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IncomeBasis {
    /// Mercantile: invoices dated in the year, whenever they are paid
    #[default]
    Accrual,
    /// Fees received in the year (TDS included), whenever they were invoiced
    Cash,
}

impl IncomeBasis {
    pub const ALL: [Self; 2] = [Self::Accrual, Self::Cash];
    /// Form options, in the order of [`Self::ALL`]
    pub const LABELS: &'static [&'static str] = &["Accrual (invoiced)", "Cash (received)"];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Accrual => "accrual",
            Self::Cash => "cash",
        }
    }

    /// Position in [`Self::ALL`] and [`Self::LABELS`]
    pub fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|&b| b == self)
            .unwrap_or_default()
    }

    /// The basis at `index` of [`Self::ALL`]; accrual when out of range
    pub fn from_index(index: usize) -> Self {
        Self::ALL.get(index).copied().unwrap_or_default()
    }
}

/// One stored change of basis: `basis` applies from the financial year starting in `from`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BasisChange {
    /// Start year of the first financial year on `basis` (`2026` for 2026-27)
    pub from: i32,
    pub basis: IncomeBasis,
}

/// Which basis each financial year was filed on: accrual until the first change. A year keeps
/// its basis when a later one changes, so the years already filed never move.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(from = "Vec<BasisChange>", into = "Vec<BasisChange>")]
pub struct BasisSchedule(Vec<BasisChange>);

impl From<Vec<BasisChange>> for BasisSchedule {
    /// Sorted by year, keeping only real changes, so any stored list reads one way
    fn from(mut changes: Vec<BasisChange>) -> Self {
        changes.sort_by_key(|change| change.from);
        let mut schedule = Self(Vec::with_capacity(changes.len()));
        for change in changes {
            schedule.0.retain(|kept| kept.from != change.from);
            if schedule.before(change.from) != change.basis {
                schedule.0.push(change);
            }
        }
        schedule
    }
}

impl From<BasisSchedule> for Vec<BasisChange> {
    fn from(schedule: BasisSchedule) -> Self {
        schedule.0
    }
}

impl BasisSchedule {
    /// The basis `year` is counted on
    pub fn basis(&self, year: FiscalYear) -> IncomeBasis {
        self.before(year.start_year() + 1)
    }

    /// Use `basis` from `from` on. Years before `from` keep theirs; later changes are dropped,
    /// as the new basis is meant to be kept.
    pub fn set(&mut self, from: FiscalYear, basis: IncomeBasis) {
        self.0.retain(|change| change.from < from.start_year());
        if self.basis(from) != basis {
            self.0.push(BasisChange {
                from: from.start_year(),
                basis,
            });
        }
    }

    /// Whether `year` is the first on a new basis, so invoices may cross between the two
    pub fn switched_in(&self, year: FiscalYear) -> bool {
        self.0.iter().any(|change| change.from == year.start_year())
    }

    /// `accrual`, or `accrual, cash from FY 2026-27`
    pub fn describe(&self) -> String {
        let mut text = IncomeBasis::Accrual.label().to_owned();
        for change in &self.0 {
            text.push_str(&format!(
                ", {} from {}",
                change.basis.label(),
                FiscalYear::starting(change.from)
            ));
        }
        text
    }

    /// The basis of the year before the one starting in `start_year`
    fn before(&self, start_year: i32) -> IncomeBasis {
        self.0
            .iter()
            .rev()
            .find(|change| change.from < start_year)
            .map_or(IncomeBasis::Accrual, |change| change.basis)
    }
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
    logo: Option<LogoId>,
    income_basis: BasisSchedule,
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
            logo: draft.logo,
            income_basis: draft.income_basis,
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

    /// The logo new invoices are issued with
    pub fn logo(&self) -> Option<LogoId> {
        self.logo
    }

    /// How gross receipts are counted for 44ADA, year by year
    pub fn income_basis(&self) -> &BasisSchedule {
        &self.income_basis
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
            logo: self.logo,
            income_basis: self.income_basis.clone(),
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
