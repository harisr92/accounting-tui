use crate::domain::{
    ClientField, CreditNoteField, DomainError, ProfileField, ProjectField, RecordField,
};

#[test]
fn a_blank_field_reads_as_before_in_its_message() {
    assert_eq!(
        DomainError::EmptyField(ProjectField::Name.into()).to_string(),
        "project name must not be empty"
    );
    assert_eq!(
        DomainError::EmptyField(CreditNoteField::Reason.into()).to_string(),
        "reason for the credit note must not be empty"
    );
}

#[test]
fn the_same_field_name_is_told_apart_by_its_record() {
    // A project's name and the business's name are both "Name" to the user's form
    let project = RecordField::from(ProjectField::Name);
    let business = RecordField::from(ProfileField::Name);
    assert_ne!(project, business);
    assert_eq!(project, RecordField::Project(ProjectField::Name));
    assert_eq!(
        RecordField::from(ClientField::Country).label(),
        "client country"
    );
}
