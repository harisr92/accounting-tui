use std::fmt;

use super::{
    ClientField, CreditNoteField, ExpenseField, InvoiceField, MilestoneField, PaymentField,
    ProfileField, ProjectField,
};

/// The field of a record a refusal is about, in [`DomainError::EmptyField`](super::DomainError::EmptyField) and
/// [`DomainError::NegativeAmount`](super::DomainError): the record, and its own field. Each
/// domain type names its fields itself ([`ProjectField`], [`PaymentField`], ...), so a front end
/// tells which input was refused by type, never by reading the message, and a new field touches
/// only its own domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecordField {
    Project(ProjectField),
    Client(ClientField),
    Milestone(MilestoneField),
    Profile(ProfileField),
    Expense(ExpenseField),
    CreditNote(CreditNoteField),
    Invoice(InvoiceField),
    Payment(PaymentField),
}

impl RecordField {
    /// How the user knows it, in an error's message
    pub fn label(self) -> &'static str {
        match self {
            Self::Project(field) => field.label(),
            Self::Client(field) => field.label(),
            Self::Milestone(field) => field.label(),
            Self::Profile(field) => field.label(),
            Self::Expense(field) => field.label(),
            Self::CreditNote(field) => field.label(),
            Self::Invoice(field) => field.label(),
            Self::Payment(field) => field.label(),
        }
    }
}

impl fmt::Display for RecordField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

impl From<ProjectField> for RecordField {
    fn from(field: ProjectField) -> Self {
        Self::Project(field)
    }
}

impl From<ClientField> for RecordField {
    fn from(field: ClientField) -> Self {
        Self::Client(field)
    }
}

impl From<MilestoneField> for RecordField {
    fn from(field: MilestoneField) -> Self {
        Self::Milestone(field)
    }
}

impl From<ProfileField> for RecordField {
    fn from(field: ProfileField) -> Self {
        Self::Profile(field)
    }
}

impl From<ExpenseField> for RecordField {
    fn from(field: ExpenseField) -> Self {
        Self::Expense(field)
    }
}

impl From<CreditNoteField> for RecordField {
    fn from(field: CreditNoteField) -> Self {
        Self::CreditNote(field)
    }
}

impl From<InvoiceField> for RecordField {
    fn from(field: InvoiceField) -> Self {
        Self::Invoice(field)
    }
}

impl From<PaymentField> for RecordField {
    fn from(field: PaymentField) -> Self {
        Self::Payment(field)
    }
}
