//! Bounded discovery and dedicated native execution for mechanics-local validation.
//! Lane selection and the planned tools retain their own authority.

pub mod active_naming;
pub mod agent_surface;
pub mod agent_surface_budget;
pub mod agent_surface_validation;
#[cfg(all(feature = "compiler-backed-validators", target_os = "linux"))]
pub mod artifact_bundle;
pub mod ci_artifacts;
#[cfg(target_os = "linux")]
pub mod ci_verification;
#[cfg(target_os = "linux")]
mod conformance_products;
pub mod decision_records;
pub mod derived_kag;
pub mod documentation_cross_corpus;
pub mod documentation_family;
pub mod executor;
pub mod growth_coverage;
pub mod growth_native_plan;
pub mod intake_pack;
#[cfg(target_os = "linux")]
pub mod kag_corpus_export;
#[cfg(target_os = "linux")]
pub mod kag_downstream_status;
#[cfg(target_os = "linux")]
pub mod kag_release;
pub mod lived_witness;
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
#[cfg(all(feature = "compiler-backed-validators", target_os = "linux"))]
pub mod source_registry;
#[cfg(all(feature = "compiler-backed-validators", target_os = "linux"))]
pub mod source_registry_views;
#[cfg(target_os = "linux")]
pub mod stats_release;
pub mod tiny_entry;
#[cfg(feature = "compiler-backed-validators")]
pub mod tree_nodes;

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
pub mod witness_structure;

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

// Fixed owner routes survive retirement of their interpreter wrappers.
// Package/part existence selects the route; source-owned flags select behavior.
const NATIVE_MECHANICS: &[(&str, &str, &str, bool)] = &[
    (
        "mechanics/agon/parts/threshold-registry",
        "builder_check",
        "--threshold-registry-build",
        true,
    ),
    (
        "mechanics/agon/parts/threshold-registry",
        "validator",
        "--threshold-registry-validate",
        false,
    ),
    (
        "mechanics/boundary-bridge/parts/public-mirror-sync",
        "validator",
        "--public-mirror-validate",
        false,
    ),
    (
        "mechanics/questbook",
        "validator",
        "--questbook-validate",
        false,
    ),
    (
        "mechanics/relation-weaving/parts/graph-promotion",
        "validator",
        "--relation-pack-validate",
        false,
    ),
    (
        "mechanics/release-support/parts/artifact-bundles",
        "validator",
        "--artifact-bundle",
        false,
    ),
];
fn native_mechanics_command(root: &Path, mode: &str, check: bool) -> io::Result<Vec<String>> {
    let mut argv = vec![
        std::env::current_exe()?
            .to_str()
            .ok_or_else(|| invalid("non-UTF-8 native mechanics executable"))?
            .into(),
        mode.into(),
        "--repo-root".into(),
        root.to_str()
            .ok_or_else(|| invalid("non-UTF-8 mechanics root"))?
            .into(),
    ];
    if check {
        argv.push("--check".into());
    }
    Ok(argv)
}
fn mechanics_command(
    root: &Path,
    python: &str,
    script: &Path,
    check: bool,
) -> io::Result<Vec<String>> {
    let mut argv = vec![python.into(), relative(root, script)?];
    if check {
        argv.push("--check".into());
    }
    Ok(argv)
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
        // Native assertion ownership follows the package/part home. The
        // replaced Python tests are no longer executable discovery inputs.
        let native = matches!(
            relative(root, &home)?.as_str(),
            "mechanics/agon/parts/threshold-registry"
                | "mechanics/experience"
                | "mechanics/questbook"
        );
        if native {
            if !tests.is_empty() {
                return Err(invalid(
                    "native mechanics home contains unreviewed Python tests",
                ));
            }
            test_homes.insert(home.clone());
        }
        if !tests.is_empty() {
            test_file_count += tests.len();
            test_homes.insert(home.clone());
        }
        let scripts = home.join("scripts");
        if NATIVE_MECHANICS
            .iter()
            .any(|r| r.0 == relative(root, &home).unwrap_or_default())
            || !named_files(&scripts, "build_", &mut visited)?.is_empty()
            || !named_files(&scripts, "validate_", &mut visited)?.is_empty()
        {
            script_homes.insert(home);
        }
    }
    if test_homes.is_empty() {
        return Err(invalid("no mechanics-local contract homes were discovered"));
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
        let routes: Vec<_> = NATIVE_MECHANICS
            .iter()
            .filter(|r| r.0 == home_string)
            .collect();
        if !routes.is_empty() {
            if !named_files(&home.join("scripts"), "build_", &mut visited)?.is_empty()
                || !named_files(&home.join("scripts"), "validate_", &mut visited)?.is_empty()
            {
                return Err(invalid(
                    "native mechanics home contains unreviewed Python scripts",
                ));
            }
            for (_, kind, mode, check) in routes {
                let command = Command {
                    kind,
                    home: home_string.clone(),
                    argv: native_mechanics_command(root, mode, *check)?,
                };
                if *kind == "builder_check" {
                    builders.push(command);
                } else {
                    validators.push(command);
                }
            }
            continue;
        }
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
        let mut generic = Plan {
            schema_version: "fixture",
            test_file_count: 0,
            commands: Vec::new(),
        };
        assert!(!growth_coverage::uses_native_route(&root, &generic).unwrap());
        generic.commands.push(Command {
            kind: "unittest",
            home: "mechanics/growth-cycle".into(),
            argv: vec!["reference".into()],
        });
        assert!(growth_coverage::uses_native_route(&root, &generic).unwrap());
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
    fn native_growth_partition_keeps_isolated_assertions_once_and_refuses_empty_success() {
        let root = fixture();
        touch(&root, growth_coverage::CONTRACT);
        fs::write(root.join(growth_coverage::CONTRACT), serde_json::to_vec(&serde_json::json!({
            "schema_version":"tos_growth_native_coverage_v1", "whole_route_status":"incomplete",
            "maintained_route":"native-pipeline", "bounded_native_route":"bounded",
            "reference_route":"reference", "assessment_route":"owner",
            "native_execution_sequence":"rust_workspace", "reference_discovery":{"root":"mechanics/growth-cycle/tests"},
            "native_test_routes":[{"cargo_manifest":"native/Cargo.toml", "target_kind":"lib", "module_routes":[["owner"]]}]
        })).unwrap()).unwrap();
        touch(&root, "native/src/lib.rs");
        fs::write(
            root.join("native/Cargo.toml"),
            "[package]\nname = \"fixture\"\n[lib]\nname = \"fixture\"\npath = \"src/lib.rs\"\n",
        )
        .unwrap();
        fs::write(root.join("native/src/lib.rs"), "mod owner;\n").unwrap();
        fs::write(
            root.join("native/src/owner.rs"),
            "#[test]\nfn retained() {}\n#[test]\nfn current() {}\n",
        )
        .unwrap();
        let steps = vec![
            (
                (
                    "remainder".into(),
                    vec![
                        "cargo".into(),
                        "test".into(),
                        "--workspace".into(),
                        "--".into(),
                        growth_native_plan::EXCLUSIONS.into(),
                    ],
                ),
                None,
            ),
            (
                (
                    "native".into(),
                    vec![growth_native_plan::CLASS_SEQUENCE.into()],
                ),
                Some(900_000),
            ),
            (
                (
                    "isolated".into(),
                    vec![
                        "cargo".into(),
                        "test".into(),
                        "--lib".into(),
                        "owner::retained".into(),
                        "--".into(),
                        "--exact".into(),
                    ],
                ),
                None,
            ),
        ];
        let expanded = growth_native_plan::expand_steps(&root, &steps).unwrap();
        assert!(
            expanded[0]
                .0
                .1
                .ends_with(&["--skip".into(), "owner::".into()])
        );
        let native = &expanded[1].0.1;
        assert_eq!(native[0], growth_native_plan::NATIVE_CLASS);
        assert!(native.ends_with(&["--skip".into(), "owner::retained".into()]));
        assert_eq!(expanded[2], steps[2]);
        assert!(
            growth_native_plan::verify_result(
                native,
                b"test result: ok. 1 passed; 0 failed; 0 ignored;"
            )
            .is_ok()
        );
        assert!(
            growth_native_plan::verify_result(
                native,
                b"test result: ok. 0 passed; 0 failed; 0 ignored;"
            )
            .is_err()
        );
        assert!(
            growth_native_plan::verify_result(
                native,
                b"test result: ok. 2 passed; 0 failed; 0 ignored;"
            )
            .is_err()
        );
        touch(&root, "docs/validation/validation_lanes.json");
        let sequence = serde_json::json!({"command_sequences":{"rust_workspace":[
            {"label":"prepare", "command":["cargo","test","--no-run","--workspace","--locked","--message-format=json"]},
            {"label":"native", "command":[growth_native_plan::CLASS_SEQUENCE]}
        ]}});
        fs::write(
            root.join("docs/validation/validation_lanes.json"),
            serde_json::to_vec(&sequence).unwrap(),
        )
        .unwrap();
        let mechanics = Plan {
            schema_version: "fixture",
            test_file_count: 42,
            commands: vec![
                Command {
                    kind: "unittest",
                    home: "mechanics/growth-cycle".into(),
                    argv: vec!["reference".into()],
                },
                Command {
                    kind: "unittest",
                    home: "mechanics/other".into(),
                    argv: vec!["other-check".into()],
                },
                Command {
                    kind: "validator",
                    home: "mechanics/growth-cycle".into(),
                    argv: vec!["growth-validator".into()],
                },
            ],
        };
        let whole = growth_coverage::whole_steps(&root, "exact-python", &mechanics).unwrap();
        assert!(
            whole
                .iter()
                .any(|((_, argv), _)| argv.len() == 1 && argv[0] == "other-check")
        );
        assert!(
            whole
                .iter()
                .any(|((_, argv), _)| argv.len() == 1 && argv[0] == "growth-validator")
        );
        assert!(
            !whole
                .iter()
                .any(|((_, argv), _)| argv.len() == 1 && argv[0] == "reference")
        );
        assert!(
            whole
                .iter()
                .any(|((_, argv), _)| argv.len() == 1
                    && argv[0] == growth_native_plan::CLASS_SEQUENCE)
        );
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
    fn supported_contract_homes_need_no_python_files_and_growth_stays_reference() {
        let root = fixture();
        for home in [
            "mechanics/agon/parts/threshold-registry",
            "mechanics/experience",
            "mechanics/questbook",
        ] {
            fs::create_dir_all(root.join(home)).unwrap();
        }
        touch(&root, "mechanics/growth-cycle/tests/test_contract.py");

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
        assert_eq!(plan.test_file_count, 1);
        touch(&root, "mechanics/questbook/tests/test_unmapped.py");
        assert!(
            discover(&root, "python")
                .unwrap_err()
                .to_string()
                .contains("unreviewed Python tests")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn fixed_native_routes_survive_wrapper_retirement_and_preserve_artifact_prefix() {
        let root = fixture();
        for home in [
            "mechanics/agon/parts/threshold-registry",
            "mechanics/experience",
            "mechanics/questbook",
            "mechanics/boundary-bridge/parts/public-mirror-sync",
            "mechanics/relation-weaving/parts/graph-promotion",
            "mechanics/release-support/parts/artifact-bundles",
        ] {
            fs::create_dir_all(root.join(home)).unwrap();
        }
        let plan = discover(&root, "/no-interpreter").unwrap();
        assert_eq!(plan.test_file_count, 0);
        assert_eq!(plan.commands.len(), 9);
        assert_eq!(
            plan.commands
                .iter()
                .filter(|c| c.kind == "builder_check")
                .count(),
            1
        );
        assert_eq!(
            plan.commands
                .iter()
                .filter(|c| c.kind == "validator")
                .count(),
            5
        );
        assert!(plan.commands.iter().all(|c| {
            !c.argv
                .iter()
                .any(|v| v == "/no-interpreter" || v.ends_with(".py"))
        }));
        let artifact = plan
            .commands
            .iter()
            .find(|c| c.home.ends_with("/artifact-bundles"))
            .unwrap();
        assert_eq!(artifact.argv[1], "--artifact-bundle");
        assert_eq!(artifact.argv[2], "--repo-root");
        touch(&root, "mechanics/questbook/scripts/validate_unreviewed.py");
        assert!(
            discover(&root, "python")
                .unwrap_err()
                .to_string()
                .contains("unreviewed Python scripts")
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
                .contains("contract homes")
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

#[cfg(feature = "compiler-backed-validators")]
pub mod open_work_queue;
