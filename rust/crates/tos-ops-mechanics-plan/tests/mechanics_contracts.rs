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
fn compact_topology_preserves_owner_history_and_source_return() {
    // The scoped executor requires a dedicated single-threaded process.
    // Exercise the installed native assertion route instead of weakening it
    // for the Rust test harness, whose main and worker threads coexist.
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_tos-ops-mechanics-plan"))
        .arg("--repo-root")
        .arg(root())
        .arg("--local-contracts")
        .arg("mechanics/agon/parts/threshold-registry")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
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
