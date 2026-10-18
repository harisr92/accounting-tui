use crate::domain::{Billing, Project};
use crate::storage::StorageError;
use crate::storage::codec::{decode, encode};

#[test]
fn round_trips_domain_types() {
    let project = Project::new("Site", "Acme", None, Billing::Fixed).unwrap();
    let bytes = encode("projects", project.id(), &project).unwrap();
    let decoded: Project = decode("projects", project.id(), &bytes).unwrap();
    assert_eq!(decoded, project);
}

#[test]
fn decode_error_names_table_and_key() {
    let error = decode::<Project>("projects", "abc", b"{not json").unwrap_err();
    match &error {
        StorageError::Codec { table, key, .. } => {
            assert_eq!(*table, "projects");
            assert_eq!(key, "abc");
        }
        other => panic!("unexpected error {other:?}"),
    }
    assert_eq!(error.to_string(), "invalid record abc in table projects");
}
