//! Retained assertions share the installed native consumer's implementation.
use std::path::Path;
use tos_ops_mechanics_plan::local_contracts;
fn root() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.."))
}
#[test]
fn agon_generated_shape_and_native_builder_validator() {
    local_contracts::agon_generated_shape_and_native_builder_validator(root());
}
#[test]
fn questbook_valid_surface_and_all_retained_mutation_boundaries() {
    local_contracts::questbook_valid_surface_and_all_retained_mutation_boundaries(root());
}
#[test]
fn experience_candidate_all_retained_schema_mutations() {
    local_contracts::experience_candidate(root());
}
#[test]
fn experience_governance_all_retained_schema_mutations() {
    local_contracts::experience_governance(root());
}
#[test]
fn experience_installation_all_retained_schema_mutations() {
    local_contracts::experience_installation(root());
}
