//! Bounded discovery and dedicated native execution for mechanics-local validation.
//! Lane selection and the planned tools retain their own authority.

pub mod active_naming;
pub mod agent_surface;
pub mod agent_surface_budget;
pub mod agent_surface_validation;
pub mod ci_artifacts;
#[cfg(target_os = "linux")]
mod conformance_products;
pub mod decision_records;
pub mod derived_kag;
pub mod documentation_cross_corpus;
pub mod documentation_family;
pub mod executor;
pub mod growth_coverage;
pub mod growth_native_plan;
#[cfg(target_os = "linux")]
pub mod kag_corpus_export;
#[cfg(target_os = "linux")]
pub mod kag_downstream_status;
#[cfg(target_os = "linux")]
pub mod kag_release;
#[path = "../tests/mechanics_contracts/native.rs"]
pub mod local_contracts;
pub mod mechanics_topology;
#[cfg(feature = "compiler-backed-validators")]
pub mod philosophy_graph_views;
#[cfg(feature = "compiler-backed-validators")]
pub mod philosophy_products;
pub mod philosophy_topology;
#[cfg(all(feature = "compiler-backed-validators", target_os = "linux"))]
pub mod prepared_dossier_docx_adapter;
#[cfg(all(feature = "compiler-backed-validators", target_os = "linux"))]
pub mod prepared_dossier_entry;
#[cfg(all(feature = "compiler-backed-validators", target_os = "linux"))]
pub mod prepared_dossier_native;
#[cfg(all(feature = "compiler-backed-validators", target_os = "linux"))]
pub mod prepared_dossier_native_directory;
#[cfg(all(feature = "compiler-backed-validators", target_os = "linux"))]
pub mod prepared_dossier_readiness;
#[cfg(all(feature = "compiler-backed-validators", target_os = "linux"))]
pub mod prepared_dossier_render;
pub mod tiny_entry;

pub mod provider_controls;
pub mod public_mirror;
pub mod questbook;
pub mod relation_pack;
pub mod root_entry_map;
pub mod route_cards;
pub mod route_harness;
pub mod semantic_registry_transition;
pub mod software_ci;
pub mod source_home;
pub mod threshold_registry;
pub mod validation_lanes;

use serde::Serialize;
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const MAX_VISITED_ENTRIES: usize = 10_000;
const MAX_COMMANDS: usize = 4_096;

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Command {
    pub kind: &'static str,
    pub home: String,
    pub argv: Vec<String>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Plan {
    pub schema_version: &'static str,
    pub test_file_count: usize,
    pub commands: Vec<Command>,
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn children(path: &Path, visited: &mut usize) -> io::Result<Vec<PathBuf>> {
    let mut result = Vec::new();
    for item in fs::read_dir(path)? {
        *visited += 1;
        if *visited > MAX_VISITED_ENTRIES {
            return Err(invalid("mechanics discovery entry bound exceeded"));
        }
        let item = item?;
        let kind = item.file_type()?;
        if kind.is_symlink() {
            return Err(invalid("mechanics discovery does not follow symlinks"));
        }
        result.push(item.path());
    }
    result.sort();
    Ok(result)
}

fn named_files(path: &Path, prefix: &str, visited: &mut usize) -> io::Result<Vec<PathBuf>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(invalid("mechanics discovery does not follow symlinks"));
        }
        Ok(metadata) if !metadata.is_dir() => {
            return Err(invalid("mechanics discovery home is not a directory"));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    }
    let mut files = Vec::new();
    for child in children(path, visited)? {
        if child.is_file()
            && child
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(prefix) && name.ends_with(".py"))
        {
            files.push(child);
        }
    }
    Ok(files)
}

fn homes(root: &Path, visited: &mut usize) -> io::Result<Vec<PathBuf>> {
    let mechanics = root.join("mechanics");
    if fs::symlink_metadata(&mechanics)?.file_type().is_symlink() {
        return Err(invalid("mechanics discovery does not follow symlinks"));
    }
    if !mechanics.is_dir() {
        return Err(invalid("mechanics root is missing"));
    }
    let mut result = Vec::new();
    for package in children(&mechanics, visited)? {
        if !package.is_dir() {
            continue;
        }
        result.push(package.clone());
        let parts = package.join("parts");
        if parts.is_symlink() {
            return Err(invalid("mechanics discovery does not follow symlinks"));
        }
        if parts.is_dir() {
            result.extend(
                children(&parts, visited)?
                    .into_iter()
                    .filter(|part| part.is_dir()),
            );
        }
    }
    result.sort();
    Ok(result)
}

fn relative(root: &Path, path: &Path) -> io::Result<String> {
    let path = path
        .strip_prefix(root)
        .map_err(|_| invalid("path escapes repository root"))?;
    let rendered = path
        .to_str()
        .ok_or_else(|| invalid("non-UTF-8 mechanics path"))?;
    Ok(rendered.replace(std::path::MAIN_SEPARATOR, "/"))
}

fn mechanics_command(
    root: &Path,
    python: &str,
    script: &Path,
    check: bool,
) -> io::Result<Vec<String>> {
    let relative = relative(root, script)?;
    let native_mode = match relative.as_str() {
        "mechanics/agon/parts/threshold-registry/scripts/build_tos_agon_threshold_intake_registry.py" => {
            Some("--threshold-registry-build")
        }
        "mechanics/agon/parts/threshold-registry/scripts/validate_tos_agon_threshold_intake_registry.py" => {
            Some("--threshold-registry-validate")
        }
        "mechanics/relation-weaving/parts/graph-promotion/scripts/validate_tree_relation_pack.py" => {
            Some("--relation-pack-validate")
        }
        "mechanics/questbook/scripts/validate_questbook_surface.py" => Some("--questbook-validate"),
        "mechanics/boundary-bridge/parts/public-mirror-sync/scripts/validate_tree_example_sync.py" => {
            Some("--public-mirror-validate")
        }
        _ => None,
    };
    if let Some(mode) = native_mode {
        let executable = std::env::current_exe()?;
        let executable = executable
            .to_str()
            .ok_or_else(|| invalid("non-UTF-8 native mechanics executable"))?;
        let root = root
            .to_str()
            .ok_or_else(|| invalid("non-UTF-8 mechanics root"))?;
        let mut argv = vec![
            executable.to_owned(),
            "--repo-root".into(),
            root.into(),
            mode.into(),
        ];
        if check {
            argv.push("--check".into());
        }
        Ok(argv)
    } else {
        let mut argv = vec![python.into(), relative];
        if check {
            argv.push("--check".into());
        }
        Ok(argv)
    }
}

/// Discover the same package and part homes as the Python lane oracle.
///
/// The returned argv retains `python` as an explicit host adapter. A caller
/// must select an interpreter and enforce process limits before execution.
pub fn discover(root: &Path, python: &str) -> io::Result<Plan> {
    if python.is_empty() || python.contains('\0') {
        return Err(invalid("python adapter must be a non-empty command"));
    }
    if !root.is_absolute() || fs::canonicalize(root)? != root {
        return Err(invalid(
            "repository root must be an absolute path without symlinks",
        ));
    }
    let mut visited = 0;
    let mut test_homes = BTreeSet::new();
    let mut script_homes = BTreeSet::new();
    let mut test_file_count = 0;
    for home in homes(root, &mut visited)? {
        let tests = named_files(&home.join("tests"), "test", &mut visited)?;
        let expected: Option<&[&str]> = match relative(root, &home)?.as_str() {
            "mechanics/agon/parts/threshold-registry" => {
                Some(&["test_tos_agon_threshold_intake_registry.py"])
            }
            "mechanics/experience" => Some(&[
                "test_experience_candidate_adoption_write_guard_contracts.py",
                "test_experience_governance_boundary_contracts.py",
                "test_experience_installation_service_office_contracts.py",
            ]),
            "mechanics/questbook" => Some(&["test_validate_questbook_surface.py"]),
            _ => None,
        };
        if let Some(expected) = expected {
            let actual = tests
                .iter()
                .map(|path| {
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("")
                })
                .collect::<Vec<_>>();
            if actual != expected {
                return Err(invalid(
                    "native mechanics reference-file set changed; review assertion coverage before execution",
                ));
            }
            // These files are frozen reference evidence for the retained native
            // assertions, not a second maintained assertion authority. Refuse
            // changed methods as well as changed filenames until owner review.
            for path in &tests {
                let digest = match path.file_name().and_then(|name| name.to_str()) {
                    Some("test_experience_candidate_adoption_write_guard_contracts.py") => {
                        "93772ada0074667f2924ed8ad4b0e59d0d106de8e92c273ffcf1ed19e296e894"
                    }
                    Some("test_experience_governance_boundary_contracts.py") => {
                        "d4ea8c24cb7d7347bfd81c6ffd98082b5aa349d8397d411c908a294724f423f2"
                    }
                    Some("test_experience_installation_service_office_contracts.py") => {
                        "bb7dea48655f31502359b44456f8b1ad67c5ca373ee62f9cf5ad25441c26abfc"
                    }
                    Some("test_validate_questbook_surface.py") => {
                        "690e049d3ee96c161f475b10fef70656ea80672760255b5bb2987bf2e877dcee"
                    }
                    Some("test_tos_agon_threshold_intake_registry.py") => {
                        "719ac8ea10af77e450c11a8bd06e70d2d2077113da55f182c0430c45a12d1403"
                    }
                    _ => return Err(invalid("unmapped native mechanics reference file")),
                };
                if tos_foundation::Digest256::of_bytes(&fs::read(path)?).to_hex() != digest {
                    return Err(invalid(
                        "native mechanics reference bytes changed; review retained assertions before execution",
                    ));
                }
            }
        }
        if !tests.is_empty() {
            test_file_count += tests.len();
            test_homes.insert(home.clone());
        }
        let scripts = home.join("scripts");
        if !named_files(&scripts, "build_", &mut visited)?.is_empty()
            || !named_files(&scripts, "validate_", &mut visited)?.is_empty()
        {
            script_homes.insert(home);
        }
    }
    if test_file_count == 0 {
        return Err(invalid("no mechanics-local unittest files were discovered"));
    }
    let mut commands = Vec::new();
    for home in test_homes {
        let home_string = relative(root, &home)?;
        if matches!(
            home_string.as_str(),
            "mechanics/agon/parts/threshold-registry"
                | "mechanics/experience"
                | "mechanics/questbook"
        ) {
            commands.push(Command {
                kind: "native_assertions",
                home: home_string.clone(),
                argv: vec![
                    std::env::current_exe()?
                        .to_str()
                        .ok_or_else(|| invalid("non-UTF-8 native mechanics executable"))?
                        .into(),
                    "--repo-root".into(),
                    root.to_str()
                        .ok_or_else(|| invalid("non-UTF-8 mechanics root"))?
                        .into(),
                    "--local-contracts".into(),
                    home_string,
                ],
            });
            continue;
        }
        commands.push(Command {
            kind: "unittest",
            home: home_string.clone(),
            argv: vec![
                python.into(),
                "-m".into(),
                "unittest".into(),
                "discover".into(),
                "-s".into(),
                format!("{home_string}/tests"),
                "-p".into(),
                "test*.py".into(),
            ],
        });
    }
    let native_count = commands
        .iter()
        .filter(|command| command.kind == "native_assertions")
        .count();
    if native_count != 0 && native_count != 3 {
        return Err(invalid(
            "native mechanics contracts require all three supported homes",
        ));
    }
    let mut builders = Vec::new();
    let mut validators = Vec::new();
    for home in script_homes {
        let home_string = relative(root, &home)?;
        for script in named_files(&home.join("scripts"), "build_", &mut visited)? {
            builders.push(Command {
                kind: "builder_check",
                home: home_string.clone(),
                argv: mechanics_command(root, python, &script, true)?,
            });
        }
        for script in named_files(&home.join("scripts"), "validate_", &mut visited)? {
            validators.push(Command {
                kind: "validator",
                home: home_string.clone(),
                argv: mechanics_command(root, python, &script, false)?,
            });
        }
    }
    if builders.is_empty() {
        return Err(invalid(
            "no mechanics-local builder --check commands were discovered",
        ));
    }
    if validators.is_empty() {
        return Err(invalid(
            "no mechanics-local validator commands were discovered",
        ));
    }
    commands.extend(builders);
    commands.extend(validators);
    if commands.len() > MAX_COMMANDS {
        return Err(invalid("mechanics command plan bound exceeded"));
    }
    Ok(Plan {
        schema_version: "tos_mechanics_local_plan_v1",
        test_file_count,
        commands,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("tos-mechanics-plan-{}-{nonce}", std::process::id()));
        fs::create_dir(&root).unwrap();
        root
    }

    fn touch(root: &Path, path: &str) {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"").unwrap();
    }

    #[test]
    fn whole_growth_cannot_succeed_without_a_native_execution_plan() {
        let root = fixture();
        let path = root.join(growth_coverage::CONTRACT);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        for status in ["incomplete", "accepted"] {
            let contract = serde_json::json!({
                "schema_version": "tos_growth_native_coverage_v1",
                "whole_route_status": status,
                "maintained_route": "selected-native-source",
                "bounded_native_route": "bounded-native",
                "reference_route": "explicit-reference",
                "assessment_route": "source-owned-assessment"
            });
            fs::write(&path, serde_json::to_vec(&contract).unwrap()).unwrap();
            let result = growth_coverage::require_whole_route(&root);
            assert!(
                result.is_err(),
                "an assessment alone must not omit Growth and succeed"
            );
            if status == "incomplete" {
                assert_eq!(result.unwrap_err().kind(), io::ErrorKind::Unsupported);
            }
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn discovers_package_and_part_homes_in_oracle_order() {
        let root = fixture();
        touch(&root, "mechanics/example/tests/test_contract.py");
        touch(
            &root,
            "mechanics/agon/parts/threshold/tests/test_registry.py",
        );
        touch(
            &root,
            "mechanics/agon/parts/threshold/scripts/build_registry.py",
        );
        touch(
            &root,
            "mechanics/agon/parts/threshold/scripts/validate_registry.py",
        );
        touch(&root, "mechanics/example/scripts/build_experience.py");
        touch(&root, "mechanics/example/scripts/validate_experience.py");
        touch(&root, "mechanics/example/tests/fixture.py");
        let plan = discover(&root, "/chosen/python").unwrap();
        assert_eq!(plan.schema_version, "tos_mechanics_local_plan_v1");
        assert_eq!(plan.test_file_count, 2);
        assert_eq!(plan.commands.len(), 6);
        assert_eq!(
            plan.commands.iter().map(|c| c.kind).collect::<Vec<_>>(),
            [
                "unittest",
                "unittest",
                "builder_check",
                "builder_check",
                "validator",
                "validator"
            ]
        );
        assert_eq!(plan.commands[0].home, "mechanics/agon/parts/threshold");
        assert_eq!(plan.commands[1].home, "mechanics/example");
        assert_eq!(
            plan.commands[2].argv,
            [
                "/chosen/python",
                "mechanics/agon/parts/threshold/scripts/build_registry.py",
                "--check"
            ]
        );
        let actual = serde_json::to_value(&plan).unwrap();
        let expected: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/oracle-plan.json")).unwrap();
        assert_eq!(actual, expected);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn supported_contract_homes_use_native_assertions_and_growth_stays_reference() {
        let root = fixture();
        for file in [
            "mechanics/agon/parts/threshold-registry/tests/test_tos_agon_threshold_intake_registry.py",
            "mechanics/experience/tests/test_experience_candidate_adoption_write_guard_contracts.py",
            "mechanics/experience/tests/test_experience_governance_boundary_contracts.py",
            "mechanics/experience/tests/test_experience_installation_service_office_contracts.py",
            "mechanics/questbook/tests/test_validate_questbook_surface.py",
            "mechanics/growth-cycle/tests/test_contract.py",
        ] {
            touch(&root, file);
            if !file.contains("growth-cycle") {
                fs::write(
                    root.join(file),
                    fs::read(
                        Path::new(env!("CARGO_MANIFEST_DIR"))
                            .join("../../..")
                            .join(file),
                    )
                    .unwrap(),
                )
                .unwrap();
            }
        }
        touch(&root, "mechanics/fixture/scripts/build_fixture.py");
        touch(&root, "mechanics/fixture/scripts/validate_fixture.py");
        let plan = discover(&root, "/must-not-run-python").unwrap();
        let native = plan
            .commands
            .iter()
            .filter(|c| c.kind == "native_assertions")
            .collect::<Vec<_>>();
        assert_eq!(native.len(), 3);
        for command in native {
            assert_eq!(command.argv[3], "--local-contracts");
            assert_eq!(command.argv[4], command.home);
            assert!(!command.argv.iter().any(|arg| arg == "/must-not-run-python"));
        }
        let reference = plan
            .commands
            .iter()
            .filter(|c| c.kind == "unittest")
            .collect::<Vec<_>>();
        assert_eq!(reference.len(), 1);
        assert_eq!(reference[0].home, "mechanics/growth-cycle");
        let reference_path =
            root.join("mechanics/questbook/tests/test_validate_questbook_surface.py");
        let original = fs::read(&reference_path).unwrap();
        fs::write(
            &reference_path,
            b"def test_new_unmapped_assertion(): pass\n",
        )
        .unwrap();
        assert!(
            discover(&root, "python")
                .unwrap_err()
                .to_string()
                .contains("reference bytes changed")
        );
        fs::write(&reference_path, original).unwrap();
        touch(&root, "mechanics/questbook/tests/test_unmapped.py");
        assert!(
            discover(&root, "python")
                .unwrap_err()
                .to_string()
                .contains("reference-file set changed")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn empty_discovery_fails_closed() {
        let root = fixture();
        fs::create_dir(root.join("mechanics")).unwrap();
        assert!(
            discover(&root, "python")
                .unwrap_err()
                .to_string()
                .contains("unittest")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_mechanics_entry_is_rejected() {
        use std::os::unix::fs::symlink;
        let root = fixture();
        fs::create_dir(root.join("mechanics")).unwrap();
        symlink("/tmp", root.join("mechanics/foreign")).unwrap();
        assert!(
            discover(&root, "python")
                .unwrap_err()
                .to_string()
                .contains("symlinks")
        );
        fs::remove_dir_all(root).unwrap();
    }
}
