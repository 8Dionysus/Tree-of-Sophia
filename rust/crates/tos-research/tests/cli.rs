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
