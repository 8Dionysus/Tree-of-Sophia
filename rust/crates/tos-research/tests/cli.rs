use std::process::Command;

#[test]
fn cli_exposes_curation_and_keeps_legacy_and_foundation_routes() {
    let binary = env!("CARGO_BIN_EXE_tos-research");
    let help = Command::new(binary).arg("--help").output().unwrap();
    assert!(help.status.success());
    assert!(
        String::from_utf8(help.stdout)
            .unwrap()
            .contains("--curation PATH")
    );

    let legacy = Command::new(binary)
        .args(["structural-paragraph", "--help"])
        .output()
        .unwrap();
    assert!(legacy.status.success());
    assert!(
        String::from_utf8(legacy.stdout)
            .unwrap()
            .contains("--validate-tracked")
    );

    let foundation = Command::new(binary)
        .args(["foundation", "--help"])
        .output()
        .unwrap();
    assert!(foundation.status.success());

    let missing = Command::new(binary)
        .args(["build", "--stage", "numbered-structure"])
        .output()
        .unwrap();
    assert_eq!(missing.status.code(), Some(2));
    assert!(
        String::from_utf8(missing.stderr)
            .unwrap()
            .contains("--curation is required")
    );
}

#[test]
fn software_role_selects_the_research_package_and_compatibility_feature() {
    assert_eq!(
        tos_foundation::native_software_roles::package("tos-research"),
        Some(env!("CARGO_PKG_NAME"))
    );
    assert_eq!(
        tos_foundation::native_software_roles::explicit_features("tos-research"),
        ["research-tools"]
    );
}

#[test]
fn curation_validation_selects_owner_and_referenced_recipe() {
    let root =
        std::fs::canonicalize(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.."))
            .unwrap();
    for path in [
        "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/expressions/de-schmeitzner-1883-part-1/editions/chemnitz-schmeitzner-1883-part-1/items/dta-sbb-corrected-tei-p5/edition-curation.v1.json",
        "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/expressions/de-schmeitzner-1884-part-3/editions/chemnitz-schmeitzner-1884-part-3/items/dta-sbb-corrected-tei-p5/edition-curation.v1.json",
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_tos-research"))
            .args([
                "validate",
                "--source-root",
                root.to_str().unwrap(),
                "--curation",
                path,
                "--stage",
                "tei-technical-markup",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["status"], "validated");
        assert_eq!(value["stage"], "tei-technical-markup");
    }
}

#[test]
fn curation_selection_rejects_missing_and_traversing_paths() {
    let binary = env!("CARGO_BIN_EXE_tos-research");
    let missing = Command::new(binary)
        .args(["build", "--stage", "numbered-structure"])
        .output()
        .unwrap();
    assert_eq!(missing.status.code(), Some(2));
    let traversal = Command::new(binary)
        .args(["validate", "--curation", "../edition-curation.v1.json"])
        .output()
        .unwrap();
    assert_eq!(traversal.status.code(), Some(2));
}

#[test]
fn numbered_structure_validation_checks_both_frozen_witness_maps() {
    let root =
        std::fs::canonicalize(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.."))
            .unwrap();
    for path in [
        "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/expressions/de-naumann-1886/editions/leipzig-c-g-naumann-1886/items/internet-archive-google-harvard-scan-pdf/edition-curation.v1.json",
        "ToS/source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/editions/moscow-mysl-1996-volume-2/items/operator-pdf/edition-curation.v1.json",
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_tos-research"))
            .args([
                "validate",
                "--source-root",
                root.to_str().unwrap(),
                "--curation",
                path,
                "--stage",
                "numbered-structure",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
