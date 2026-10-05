//! Source-derived Growth test classes. Planning never builds or executes a child.
//! Cargo owns target names/paths; Rust declarations own class membership.
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use tos_foundation::{Digest256, RelativePath};

const MAX_SOURCE_BYTES: u64 = 1_048_576;
const MAX_CLASSES: usize = 256;
pub(crate) const NATIVE_CLASS: &str = "@tos-native-growth-class";
pub(crate) const CLASS_SEQUENCE: &str = "@tos-native-growth-classes";
pub(crate) const EXCLUSIONS: &str = "@tos-native-growth-exclusions";

#[derive(Deserialize)]
pub(crate) struct TestRoute {
    cargo_manifest: String,
    target_kind: String,
    #[serde(default)]
    target_name: Option<String>,
    #[serde(default)]
    module_prefix: Option<String>,
    #[serde(default)]
    module_routes: Vec<Vec<String>>,
}

#[derive(Serialize)]
pub struct NativePlan {
    schema_version: &'static str,
    posture: &'static str,
    pub(crate) classes: Vec<NativeClass>,
}

#[derive(Serialize)]
pub(crate) struct NativeClass {
    pub(crate) package: String,
    cargo_manifest: String,
    pub(crate) target_kind: String,
    pub(crate) target_name: String,
    source: String,
    source_sha256: String,
    pub(crate) class_filter: String,
    pub(crate) assertions: Vec<Assertion>,
}

#[derive(Serialize)]
pub(crate) struct Assertion {
    pub(crate) function: String,
    pub(crate) ignored: bool,
}

fn invalid(message: &str) -> io::Error {
    io::Error::other(message)
}

fn read(root: &Path, path: &Path) -> io::Result<String> {
    let relative = path.strip_prefix(root).map_err(io::Error::other)?;
    RelativePath::parse(
        relative
            .to_str()
            .ok_or_else(|| invalid("non-UTF8 source path"))?,
    )
    .map_err(io::Error::other)?;
    if path.canonicalize()? != path || !fs::symlink_metadata(path)?.is_file() {
        return Err(invalid(
            "native class source must be a regular file without symlinks",
        ));
    }
    let mut raw = Vec::new();
    fs::File::open(path)?
        .take(MAX_SOURCE_BYTES + 1)
        .read_to_end(&mut raw)?;
    if raw.len() as u64 > MAX_SOURCE_BYTES {
        return Err(invalid("native class source byte bound exceeded"));
    }
    String::from_utf8(raw).map_err(io::Error::other)
}

// These owner manifests use literal Cargo names/paths. Refuse unsupported
// syntax rather than silently inventing a target or falling back to a path.
fn cargo_target(raw: &str, route: &TestRoute) -> io::Result<(String, String, String)> {
    let mut sections = Vec::<(String, Vec<(String, String)>)>::new();
    for line in raw.lines().map(str::trim) {
        if line.starts_with('[') && line.ends_with(']') {
            sections.push((line.into(), Vec::new()));
        } else if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            if matches!(key, "name" | "path") {
                let value: String = serde_json::from_str(value.trim()).map_err(io::Error::other)?;
                if let Some((_, fields)) = sections.last_mut() {
                    fields.push((key.into(), value));
                }
            }
        }
    }
    let field = |fields: &[(String, String)], key: &str| -> io::Result<String> {
        let matches: Vec<_> = fields.iter().filter(|(name, _)| name == key).collect();
        if matches.len() != 1 {
            return Err(invalid(
                "Cargo native target requires one exact name/path field",
            ));
        }
        Ok(matches[0].1.clone())
    };
    let package = sections
        .iter()
        .find(|(name, _)| name == "[package]")
        .ok_or_else(|| invalid("Cargo package declaration missing"))?;
    let package = field(&package.1, "name")?;
    let header = match route.target_kind.as_str() {
        "lib" => "[lib]",
        "test" => "[[test]]",
        _ => return Err(invalid("unsupported native assertion target kind")),
    };
    let mut targets = Vec::new();
    for (name, fields) in &sections {
        if name == header {
            let name = field(fields, "name")?;
            if route
                .target_name
                .as_ref()
                .is_none_or(|selected| selected == &name)
            {
                targets.push((name, field(fields, "path")?));
            }
        }
    }
    if targets.len() != 1 {
        return Err(invalid(
            "native class route must select one declared Cargo target",
        ));
    }
    let (name, path) = targets.remove(0);
    RelativePath::parse(&path).map_err(io::Error::other)?;
    Ok((package, name, path))
}

fn declarations(path: &Path, raw: &str) -> io::Result<Vec<(String, PathBuf)>> {
    let declarations = Regex::new(r"(?m)^((?:[ \t]*#\[[^\n]*\][ \t]*\n)*)(?:pub(?:\([^)]*\))?[ \t]+)?mod[ \t]+([A-Za-z_][A-Za-z0-9_]*)[ \t]*;").map_err(io::Error::other)?;
    let explicit_path = Regex::new(r#"#\[path\s*=\s*("[^"\n]*")\]"#).map_err(io::Error::other)?;
    let parent = path
        .parent()
        .ok_or_else(|| invalid("native module has no parent"))?;
    let mut result = Vec::new();
    for declaration in declarations.captures_iter(raw) {
        let name = declaration[2].to_owned();
        let file = if let Some(explicit) = explicit_path.captures(&declaration[1]) {
            let path: String = serde_json::from_str(&explicit[1]).map_err(io::Error::other)?;
            RelativePath::parse(&path).map_err(io::Error::other)?;
            parent.join(path)
        } else {
            parent.join(format!("{name}.rs"))
        };
        result.push((name, file));
        if result.len() > MAX_CLASSES {
            return Err(invalid("native module declaration bound exceeded"));
        }
    }
    Ok(result)
}

fn assertions(raw: &str) -> io::Result<Vec<Assertion>> {
    let tests = Regex::new(r"(?m)((?:[ \t]*#\[[^\n]*\][ \t]*\n)+)[ \t]*(?:pub[ \t]+)?fn[ \t]+([A-Za-z_][A-Za-z0-9_]*)\s*\(").map_err(io::Error::other)?;
    Ok(tests
        .captures_iter(raw)
        .filter(|test| test[1].contains("#[test]"))
        .map(|test| Assertion {
            function: test[2].to_owned(),
            ignored: test[1].contains("#[ignore"),
        })
        .collect())
}

pub fn discover(root: &Path) -> io::Result<NativePlan> {
    if !root.is_absolute() || root.canonicalize()? != root {
        return Err(invalid(
            "native Growth plan requires an exact absolute source root",
        ));
    }
    let coverage = crate::growth_coverage::load(root)?;
    if coverage.native_test_routes.is_empty() {
        return Err(invalid("owner contract has no native assertion routes"));
    }
    let mut classes = Vec::new();
    let mut seen = BTreeSet::new();
    for route in coverage.native_test_routes {
        RelativePath::parse(&route.cargo_manifest).map_err(io::Error::other)?;
        let manifest = root.join(&route.cargo_manifest);
        let (package, target_name, entry) = cargo_target(&read(root, &manifest)?, &route)?;
        let entry = manifest.parent().unwrap().join(entry);
        let mut selected = Vec::new();
        if let Some(prefix) = &route.module_prefix {
            if prefix.is_empty() {
                return Err(invalid("native class prefix must be nonempty"));
            }
            selected.extend(
                declarations(&entry, &read(root, &entry)?)?
                    .into_iter()
                    .filter(|(name, _)| name.starts_with(prefix))
                    .map(|(name, path)| (vec![name], path)),
            );
        }
        for modules in &route.module_routes {
            if modules.is_empty() || modules.len() > 16 {
                return Err(invalid("native class module route is empty or too deep"));
            }
            let mut path = entry.clone();
            for module in modules {
                let matches: Vec<_> = declarations(&path, &read(root, &path)?)?
                    .into_iter()
                    .filter(|(name, _)| name == module)
                    .collect();
                if matches.len() != 1 {
                    return Err(invalid(
                        "native owner module declaration is absent or ambiguous",
                    ));
                }
                path = matches[0].1.clone();
            }
            selected.push((modules.clone(), path));
        }
        if selected.is_empty() {
            return Err(invalid("native assertion route selected no source classes"));
        }
        for (modules, path) in selected {
            let raw = read(root, &path)?;
            let assertions = assertions(&raw)?;
            if assertions.is_empty() {
                return Err(invalid("native class contains no declared assertions"));
            }
            let class_filter = format!("{}::", modules.join("::"));
            if seen.iter().any(
                |(seen_package, seen_kind, seen_target, seen_filter): &(
                    String,
                    String,
                    String,
                    String,
                )| {
                    seen_package == &package
                        && seen_kind == &route.target_kind
                        && seen_target == &target_name
                        && (class_filter.starts_with(seen_filter)
                            || seen_filter.starts_with(&class_filter))
                },
            ) {
                return Err(invalid(
                    "overlapping native Growth classes would execute assertions twice",
                ));
            }
            if !seen.insert((
                package.clone(),
                route.target_kind.clone(),
                target_name.clone(),
                class_filter.clone(),
            )) {
                return Err(invalid("duplicate native Growth class"));
            }
            classes.push(NativeClass {
                package: package.clone(),
                cargo_manifest: route.cargo_manifest.clone(),
                target_kind: route.target_kind.clone(),
                target_name: target_name.clone(),
                source: path.strip_prefix(root).unwrap().to_str().unwrap().into(),
                source_sha256: Digest256::of_bytes(raw.as_bytes()).to_hex(),
                class_filter,
                assertions,
            });
            if classes.len() > MAX_CLASSES {
                return Err(invalid("native Growth class bound exceeded"));
            }
        }
    }
    Ok(NativePlan {
        schema_version: "tos_growth_native_class_plan_v1",
        posture: "source_owned_assertion_plan_not_execution_or_equivalence",
        classes,
    })
}

/// Expand only explicit source-lane markers. Existing exact process-isolated
/// selections retain their own route and are excluded from grouped classes.
pub(crate) fn expand_steps(
    root: &Path,
    steps: &[crate::validation_lanes::BudgetedCommandStep],
) -> io::Result<Vec<crate::validation_lanes::BudgetedCommandStep>> {
    if !steps.iter().any(|((_, argv), _)| {
        argv.iter()
            .any(|arg| arg == CLASS_SEQUENCE || arg == EXCLUSIONS)
    }) {
        return Ok(steps.to_vec());
    }
    if steps
        .iter()
        .filter(|((_, argv), _)| argv.first().is_some_and(|arg| arg == CLASS_SEQUENCE))
        .count()
        != 1
    {
        return Err(invalid(
            "native Growth partition requires exactly one class sequence",
        ));
    }
    let native = discover(root)?;
    let isolated: Vec<(String, Option<String>)> = steps
        .iter()
        .filter_map(|((_, argv), _)| {
            if !argv.iter().any(|arg| arg == "--exact") {
                return None;
            }
            let end = argv
                .iter()
                .position(|arg| arg == "--")
                .unwrap_or(argv.len());
            let name = argv[..end].iter().find(|arg| arg.contains("::"))?.clone();
            let target = argv
                .iter()
                .position(|arg| arg == "--test")
                .and_then(|index| argv.get(index + 1))
                .cloned();
            Some((name, target))
        })
        .collect();
    let mut expanded = Vec::new();
    for ((label, original), timeout) in steps {
        if original.first().is_some_and(|arg| arg == CLASS_SEQUENCE) {
            if original.len() != 1 {
                return Err(invalid("native class marker takes no arbitrary arguments"));
            }
            for class in &native.classes {
                let isolated: Vec<_> = isolated
                    .iter()
                    .filter(|(name, target)| {
                        name.starts_with(&class.class_filter)
                            && target
                                .as_ref()
                                .map_or(class.target_kind == "lib", |target| {
                                    class.target_kind == "test" && target == &class.target_name
                                })
                    })
                    .map(|(name, _)| name)
                    .collect();
                let covered = |assertion: &Assertion| {
                    isolated
                        .iter()
                        .any(|name| name.ends_with(&format!("::{}", assertion.function)))
                };
                let active = class
                    .assertions
                    .iter()
                    .filter(|a| !a.ignored && !covered(a))
                    .count();
                let ignored = class
                    .assertions
                    .iter()
                    .filter(|a| a.ignored && !covered(a))
                    .count();
                if active == 0 {
                    continue;
                }
                let mut command = vec![
                    NATIVE_CLASS.into(),
                    class.package.clone(),
                    class.target_kind.clone(),
                    class.target_name.clone(),
                    class.class_filter.clone(),
                    active.to_string(),
                    ignored.to_string(),
                ];
                for name in isolated {
                    command.extend(["--skip".into(), name.clone()]);
                }
                expanded.push((
                    (
                        format!("{label}: {} {}", class.package, class.class_filter),
                        command,
                    ),
                    *timeout,
                ));
            }
        } else if original.iter().any(|arg| arg == EXCLUSIONS) {
            let target = original
                .iter()
                .position(|arg| arg == "--test")
                .and_then(|index| original.get(index + 1));
            let mut command: Vec<_> = original
                .iter()
                .filter(|arg| *arg != EXCLUSIONS)
                .cloned()
                .collect();
            for class in &native.classes {
                if target.map_or(class.target_kind == "lib", |target| {
                    class.target_kind == "test" && target == &class.target_name
                }) {
                    command.extend(["--skip".into(), class.class_filter.clone()]);
                }
            }
            for (name, isolated_target) in &isolated {
                if isolated_target.as_ref() == target
                    && !native.classes.iter().any(|class| {
                        name.starts_with(&class.class_filter)
                            && target.map_or(class.target_kind == "lib", |target| {
                                class.target_kind == "test" && target == &class.target_name
                            })
                    })
                {
                    command.extend(["--skip".into(), name.clone()]);
                }
            }
            expanded.push(((label.clone(), command), *timeout));
        } else {
            expanded.push(((label.clone(), original.clone()), *timeout));
        }
    }
    Ok(expanded)
}

pub(crate) fn verify_result(command: &[String], stdout: &[u8]) -> io::Result<()> {
    let active = command
        .get(5)
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value != 0)
        .ok_or_else(|| invalid("native class has no active declared assertions"))?;
    let ignored = command
        .get(6)
        .and_then(|value| value.parse::<usize>().ok())
        .ok_or_else(|| invalid("native class ignored assertion count missing"))?;
    let result = Regex::new(r"test result: ok\. ([0-9]+) passed; 0 failed; ([0-9]+) ignored;")
        .map_err(io::Error::other)?;
    let stdout = std::str::from_utf8(stdout).map_err(io::Error::other)?;
    let results: Vec<_> = result.captures_iter(stdout).collect();
    if results.len() != 1
        || results[0][1].parse::<usize>().ok() != Some(active)
        || results[0][2].parse::<usize>().ok() != Some(ignored)
    {
        return Err(invalid(
            "native class did not execute its declared active assertions or changed ignored scope",
        ));
    }
    Ok(())
}
