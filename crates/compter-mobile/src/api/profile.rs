use chrono::Local;
use compter_core::domain::{
    BusinessProfile, Currency, DEFAULT_INVOICE_PREFIX, DEFAULT_SAC, FiscalYear, IncomeBasis,
    ProfileDraft,
};

/// The business profile as shown in Settings
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileRow {
    pub name: String,
    pub address: Vec<String>,
    pub gstin: Option<String>,
    pub sac_code: String,
    /// `IT design and development services (18%)`, or `None` for a code not in the rate list
    pub sac_description: Option<String>,
    pub invoice_prefix: String,
    pub lut_arn: Option<String>,
    pub terms: Vec<String>,
    /// What new projects are billed in: `INR`, `USD`, `GBP` or `EUR`
    pub currency: String,
    /// This year's reports count fees received for 44ADA, not invoices dated in it
    pub cash_basis: bool,
    /// Every year's basis: `accrual, cash from FY 2026-27`
    pub income_basis: String,
}

impl From<&BusinessProfile> for ProfileRow {
    fn from(profile: &BusinessProfile) -> Self {
        Self {
            name: profile.name().to_owned(),
            address: profile.address().to_vec(),
            gstin: profile.gstin().map(ToString::to_string),
            sac_code: profile.sac_code().to_owned(),
            sac_description: profile
                .sac_entry()
                .map(|entry| format!("{} ({}%)", entry.description, entry.gst_rate)),
            invoice_prefix: profile.invoice_prefix().to_owned(),
            lut_arn: profile.lut_arn().map(str::to_owned),
            terms: profile.terms().to_vec(),
            currency: profile.currency().code().to_owned(),
            cash_basis: profile.income_basis().basis(this_year()) == IncomeBasis::Cash,
            income_basis: profile.income_basis().describe(),
        }
    }
}

/// The business profile as typed. `profile_input` prefills it, with the usual SAC code and
/// prefix before anything is saved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileInput {
    pub name: String,
    /// One entry per line; blank lines are dropped
    pub address: Vec<String>,
    /// Blank when not registered under GST: invoices then carry no GST
    pub gstin: String,
    pub sac_code: String,
    pub invoice_prefix: String,
    /// Letter of Undertaking ARN, needed to invoice clients abroad
    pub lut_arn: String,
    /// Printed at the end of every invoice; blank lines are dropped
    pub terms: Vec<String>,
    /// `INR`, `USD`, `GBP` or `EUR`
    pub currency: String,
    /// Count fees received in the year for 44ADA (cash basis), not invoices dated in it
    pub cash_basis: bool,
    /// The first year on `cash_basis`, `2026-27`; earlier years keep theirs. Blank means this
    /// year.
    pub basis_from: String,
}

impl From<ProfileDraft> for ProfileInput {
    fn from(draft: ProfileDraft) -> Self {
        Self {
            name: draft.name,
            address: draft.address,
            gstin: draft.gstin.unwrap_or_default(),
            sac_code: draft.sac_code,
            invoice_prefix: draft.invoice_prefix,
            lut_arn: draft.lut_arn.unwrap_or_default(),
            terms: draft.terms,
            currency: draft.currency.code().to_owned(),
            cash_basis: draft.income_basis.basis(this_year()) == IncomeBasis::Cash,
            basis_from: this_year().label(),
        }
    }
}

impl ProfileInput {
    /// A first profile: the usual SAC code and prefix, billed in rupees
    pub(crate) fn blank() -> Self {
        Self::from(ProfileDraft {
            sac_code: DEFAULT_SAC.to_owned(),
            invoice_prefix: DEFAULT_INVOICE_PREFIX.to_owned(),
            ..ProfileDraft::default()
        })
    }
}

/// The financial year today is in
pub(crate) fn this_year() -> FiscalYear {
    FiscalYear::containing(Local::now().date_naive())
}

/// The currency with `code`, as `ProfileRow::currency` and `ProjectDetail::currency` give it
pub(crate) fn currency(code: &str) -> Option<Currency> {
    Currency::ALL.into_iter().find(|c| c.code() == code.trim())
}
