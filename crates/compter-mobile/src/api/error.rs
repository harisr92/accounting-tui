use accounting_core::InvoiceError;
use compter_core::domain::{
    ClientField, CreditNoteField, DomainError, ExpenseField, MilestoneField, PaymentField,
    ProfileField, ProjectField, RecordField,
};
use compter_core::services::{InvoicingError, ServiceError};

/// Why a call failed, as Dart shows it. `field` names the input that was refused, so a form can
/// show the message under it; it is `None` for anything else, which a form shows as a toast.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct ApiError {
    pub field: Option<InputField>,
    /// With [`InputField::CreditLine`], the milestone whose credit note line was refused
    pub line: Option<String>,
    pub message: String,
}

/// An input of the phone's forms, as both sides name it: a refusal says which one it is about
/// ([`ApiError::field`]) and the Dart form keys its fields by it, so renaming one on either side
/// stops the build instead of moving the error off its field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputField {
    /// A project's, milestone's or the business's name
    Name,
    ClientName,
    ClientGstin,
    ClientState,
    ClientCountry,
    /// The client's or the business's
    Address,
    Currency,
    HourlyRate,
    DueDate,
    /// A milestone's or an expense's
    Amount,
    /// The business's own
    Gstin,
    SacCode,
    InvoicePrefix,
    LutArn,
    Terms,
    IncomeBasis,
    BasisFrom,
    /// An expense's, payment's, credit note's or time entry's
    Date,
    Category,
    Description,
    GstCredit,
    Discount,
    /// The exchange rate an invoice is issued at
    Rate,
    Received,
    /// TDS withheld, or for a foreign invoice the rupees banked
    TdsOrBanked,
    Reason,
    /// Refund, or keep as the client's advance
    Returned,
    RefundedInr,
    TdsReversed,
    Duration,
    Note,
    Quarter,
    /// The months of a quarter filed by IFF
    Filed,
    Month,
    /// One line of a credit note: [`ApiError::line`] names which
    CreditLine,
    /// Not typed: the record a call was made on
    ProjectId,
    /// Not typed: the record a call was made on
    MilestoneId,
    /// Not typed: the record a call was made on
    ClientId,
    /// Not typed: the record a call was made on
    ExpenseId,
}

impl ApiError {
    /// A failure about no input
    pub(crate) fn general(message: impl Into<String>) -> Self {
        Self {
            field: None,
            line: None,
            message: message.into(),
        }
    }

    pub(crate) fn invalid(field: InputField, message: impl Into<String>) -> Self {
        Self {
            field: Some(field),
            line: None,
            message: message.into(),
        }
    }

    /// A credit note line's amount refused, the line named by its `milestone`
    pub(crate) fn invalid_line(milestone: impl ToString, message: impl Into<String>) -> Self {
        Self {
            field: Some(InputField::CreditLine),
            line: Some(milestone.to_string()),
            message: message.into(),
        }
    }

    /// The refusal of a form's submit, naming the form's field it is about. The same rule can
    /// be about different fields in different forms (a GSTIN is the business's in the profile,
    /// the client's in a project), so the form decides.
    pub(crate) fn on(form: Form) -> impl Fn(ServiceError) -> Self {
        move |error| {
            let field = field_for(form, &error);
            Self::refused(error, field)
        }
    }

    fn refused(error: ServiceError, field: Option<InputField>) -> Self {
        let line = match &error {
            ServiceError::CreditLine { milestone, .. } => Some(milestone.to_string()),
            _ => None,
        };
        Self {
            field,
            line,
            message: error.to_string(),
        }
    }
}

/// The phone's forms whose refusals name a field
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Form {
    /// New or edited project, with its client's details
    Project,
    Milestone,
    /// A client's details corrected on their own
    Client,
    RenameClient,
    Profile,
    /// Issuing an invoice: its discount and exchange rate
    Issue,
    Payment,
    /// A credit note's terms; `tds` when the client withheld TDS from the invoice, where the
    /// form asks for the TDS the client reverses
    Credit {
        tds: bool,
    },
    Expense,
}

impl From<ServiceError> for ApiError {
    /// Outside a form only the refusals that name their field anywhere do: a credit note line,
    /// a state other than the GSTIN's, a client name another client goes by
    fn from(error: ServiceError) -> Self {
        let field = any_form(&error);
        Self::refused(error, field)
    }
}

/// The field of `form` that `error` is about, if it is about one
fn field_for(form: Form, error: &ServiceError) -> Option<InputField> {
    any_form(error).or_else(|| match domain_error(error) {
        Some(error) => domain_field(form, error),
        None => service_field(form, error),
    })
}

/// Fields named the same in every form
fn any_form(error: &ServiceError) -> Option<InputField> {
    match error {
        ServiceError::CreditLine { .. } => Some(InputField::CreditLine),
        ServiceError::Domain(DomainError::StateNotGstins(_)) => Some(InputField::ClientState),
        ServiceError::ClientNameTaken { .. } => Some(InputField::ClientName),
        _ => None,
    }
}

/// The domain rule `error` broke. A project refusing a client's change isn't one: it is about
/// that project, which its message names, not a field of the form.
fn domain_error(error: &ServiceError) -> Option<&DomainError> {
    match error {
        ServiceError::Domain(e) | ServiceError::Invoicing(InvoicingError::Domain(e)) => Some(e),
        _ => None,
    }
}

fn domain_field(form: Form, error: &DomainError) -> Option<InputField> {
    use DomainError as E;
    use InputField as F;
    match (form, error) {
        (Form::Project, E::EmptyField(RecordField::Project(ProjectField::Name)))
        | (Form::Milestone, E::EmptyField(RecordField::Milestone(MilestoneField::Name)))
        | (Form::Profile, E::EmptyField(RecordField::Profile(ProfileField::Name))) => Some(F::Name),
        (
            Form::Project | Form::RenameClient,
            E::EmptyField(RecordField::Client(ClientField::Name)),
        ) => Some(F::ClientName),
        (
            Form::Project | Form::Client,
            E::EmptyField(RecordField::Client(ClientField::Country)),
        ) => Some(F::ClientCountry),
        (Form::Project | Form::Client, E::InvalidGstin(_) | E::ForeignClientWithGstin) => {
            Some(F::ClientGstin)
        }
        (Form::Project | Form::Client, E::ForeignCurrencyInIndia)
        | (Form::Project, E::CurrencyLocked) => Some(F::Currency),
        (Form::Project, E::BillingMismatch | E::NonPositiveRate) => Some(F::HourlyRate),
        (Form::Milestone | Form::Expense, E::NonPositiveAmount) => Some(F::Amount),
        (Form::Profile, E::EmptyField(RecordField::Profile(ProfileField::Address))) => {
            Some(F::Address)
        }
        (Form::Profile, E::InvalidGstin(_)) => Some(F::Gstin),
        (Form::Profile, E::UnknownSacCode(_)) => Some(F::SacCode),
        (Form::Profile, E::InvalidInvoicePrefix | E::ReservedInvoicePrefix(_)) => {
            Some(F::InvoicePrefix)
        }
        // A discount of zero is the only amount issuing can find not above zero
        (
            Form::Issue,
            E::DiscountPercent
            | E::DiscountTooLarge { .. }
            | E::FractionOfPaisa
            | E::NonPositiveAmount,
        ) => Some(F::Discount),
        (Form::Issue, E::InvalidExchangeRate) => Some(F::Rate),
        (
            Form::Payment,
            E::PaymentExceedsBalance { .. }
            | E::NonPositiveAmount
            | E::NegativeAmount(RecordField::Payment(PaymentField::Received)),
        ) => Some(F::Received),
        (Form::Payment, E::PaymentBeforeInvoice)
        | (Form::Credit { .. }, E::CreditBeforeInvoice) => Some(F::Date),
        (
            Form::Payment,
            E::MissingCreditedAmount | E::NegativeAmount(RecordField::Payment(PaymentField::Tds)),
        ) => Some(F::TdsOrBanked),
        (Form::Credit { .. }, E::EmptyField(RecordField::CreditNote(CreditNoteField::Reason))) => {
            Some(F::Reason)
        }
        (Form::Credit { .. }, E::ReturnExceedsPaid)
        | (Form::Credit { tds: false }, E::ReturnExceedsReceived { .. }) => Some(F::Returned),
        (Form::Credit { .. }, E::MissingRefundedAmount | E::UnexpectedRefundedAmount) => {
            Some(F::RefundedInr)
        }
        (
            Form::Credit { tds: true },
            E::ReturnExceedsReceived { .. }
            | E::UnexpectedTdsReversed
            | E::TdsReversedExceedsWithheld { .. },
        ) => Some(F::TdsReversed),
        (Form::Expense, E::EmptyField(RecordField::Expense(ExpenseField::Description))) => {
            Some(F::Description)
        }
        (
            Form::Expense,
            E::NegativeAmount(RecordField::Expense(ExpenseField::GstCredit)) | E::GstCreditTooLarge,
        ) => Some(F::GstCredit),
        _ => None,
    }
}

fn service_field(form: Form, error: &ServiceError) -> Option<InputField> {
    use InputField as F;
    match (form, error) {
        (Form::Issue, ServiceError::Invoicing(InvoicingError::MissingExchangeRate(_))) => {
            Some(F::Rate)
        }
        (
            Form::Credit { .. },
            ServiceError::Invoicing(
                InvoicingError::CreditNoteTooLate(_)
                | InvoicingError::Invoice(InvoiceError::NoteBeforeOriginal { .. }),
            ),
        ) => Some(F::Date),
        (Form::Expense, ServiceError::CreditWithoutGstin) => Some(F::GstCredit),
        _ => None,
    }
}
