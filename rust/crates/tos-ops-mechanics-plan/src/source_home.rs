//! The maintained ToS source-home manifest/topology validator. This validates
//! route mechanics only; branch meaning, source review and canon stay owned.

use std::collections::BTreeSet;
use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::{AtomicI32, Ordering};

use serde_json::Value;
use tos_foundation::{FoundationErrorCode, JsonLimits, JsonMode, parse_json};

use crate::route_cards::{RouteSources, python_space};

const MANIFEST: &str = "ToS/source_home.manifest.json";
const LANES: &str = "docs/validation/validation_lanes.json";
const README: &str = "ToS/README.md";
const CORE: [&str; 11] = [
    "doctrine",
    "source_witnesses",
    "zarathustra",
    "research_packets",
    "philosophy",
    "candidate_intake",
    "canon",
    "public_compatibility",
    "derived_exports",
    "contracts",
    "review_ledger",
];
const FRAGMENTS: [&str; 4] = [
    "## Operating Card",
    "## Boundary Routes",
    "| role | source-home entrypoint for ToS-authored philosophical work |",
    "| next route | witness or research packet -> zarathustra, philosophy, or candidate intake -> canon -> public compatibility -> derived export |",
];
const LEGACY: [&str; 6] = [
    "sources",
    "intake",
    "tree",
    "examples",
    "generated",
    "schemas",
];
const BANNED: [&str; 2] = ["## Stop Lines", "## Hard no"];

pub type Issue = (String, String);
struct Issues {
    rows: Vec<Issue>,
    bytes: usize,
}
impl Issues {
    fn push(&mut self, path: &str, message: impl Into<String>) -> io::Result<()> {
        let message = message.into();
        let bytes = self
            .bytes
            .checked_add(path.len())
            .and_then(|n| n.checked_add(message.len()))
            .and_then(|n| n.checked_add(5))
            .ok_or_else(|| io::Error::other("source-home issue accounting overflow"))?;
        if self.rows.len() >= 4096 || bytes > 1_048_576 {
            return Err(io::Error::other("source-home issue output bound exceeded"));
        }
        self.rows.push((path.into(), message));
        self.bytes = bytes;
        Ok(())
    }
}
fn check(source: &RouteSources, cancel: &AtomicI32) -> io::Result<()> {
    if cancel.load(Ordering::Relaxed) != 0 {
        return Err(io::Error::other("source-home validation cancelled"));
    }
    source.check()
}
fn normalized(text: &str) -> io::Result<String> {
    // The required route fragments are ASCII; Unicode lowercase expansion
    // cannot select a different owner or execute the text. No token Vec needed.
    let mut output = String::new();
    let mut space = false;
    for c in text.chars().flat_map(char::to_lowercase) {
        if python_space(c) {
            space = !output.is_empty();
            continue;
        }
        let growth = c.len_utf8() + usize::from(space);
        if growth > 24 * 1_048_576usize - output.len() {
            return Err(io::Error::other(
                "source-home normalized README exceeds bound",
            ));
        }
        if space {
            output.push(' ');
            space = false;
        }
        output.push(c);
    }
    Ok(output)
}
fn load_json(
    source: &mut RouteSources,
    path: &str,
    issues: &mut Issues,
    manifest: bool,
) -> io::Result<Option<Value>> {
    let Some(text) = source.text(path)? else {
        issues.push(
            path,
            if manifest {
                "missing ToS source-home manifest"
            } else {
                "unable to load validation lanes: missing file"
            },
        )?;
        return Ok(None);
    };
    // Original json.loads keeps the last decoded duplicate. Strict duplicate
    // source admission is a different route; do not silently add it here.
    match parse_json(
        text.as_bytes(),
        JsonMode::RequestLastWins,
        JsonLimits::default(),
    ) {
        Ok(document) => drop(document),
        Err(error)
            if error.code == FoundationErrorCode::BudgetExceeded
                || matches!(
                    error.code,
                    FoundationErrorCode::NonfiniteFloat | FoundationErrorCode::InvalidUnicodeScalar
                ) =>
        {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!("source-home finite JSON profile: {error:?}"),
            ));
        }
        Err(error) => {
            issues.push(
                path,
                format!(
                    "{}: {error:?}",
                    if manifest {
                        "invalid JSON"
                    } else {
                        "unable to load validation lanes"
                    }
                ),
            )?;
            return Ok(None);
        }
    }
    let value: Value = serde_json::from_str(&text).map_err(|error| {
        io::Error::new(
            io::ErrorKind::Unsupported,
            format!("source-home JSON decoded representation: {error}"),
        )
    })?;
    if manifest && !value.is_object() {
        issues.push(path, "manifest root must be a JSON object")?;
        return Ok(None);
    }
    Ok(Some(value))
}

/// Same ordered diagnostics and meaningful topology checks as the maintained
/// script. The reader's finite relative/no-symlink profile is explicit; paths
/// outside it are unsupported, never silently counted as existing owners.
pub fn run_validation(root: &Path, cancel: &AtomicI32) -> io::Result<Vec<Issue>> {
    let mut source = RouteSources::new(root)?;
    let mut issues = Issues {
        rows: Vec::new(),
        bytes: "ToS source-home validation failed.\n".len(),
    };
    check(&source, cancel)?;
    if !source.is_file("ToS/AGENTS.md")? {
        issues.push("ToS/AGENTS.md", "missing ToS home route card")?;
    }
    if !source.is_file(README)? {
        issues.push(README, "missing ToS home map")?;
    } else {
        let text = source
            .text(README)?
            .ok_or_else(|| io::Error::other("source-home README disappeared"))?;
        let normal = normalized(&text)?;
        for fragment in FRAGMENTS {
            if !normal.contains(&normalized(fragment)?) {
                issues.push(
                    README,
                    format!("missing source-home route fragment: {fragment}"),
                )?;
            }
        }
        for marker in BANNED {
            if text.contains(marker) {
                issues.push(
                    README,
                    format!("use Operating Card/Boundary Routes instead of {marker}"),
                )?;
            }
        }
    }
    for legacy in LEGACY {
        check(&source, cancel)?;
        if source.exists(legacy)? {
            issues.push(
                legacy,
                "legacy root home must not exist as an active ToS surface",
            )?;
        }
    }
    let Some(manifest) = load_json(&mut source, MANIFEST, &mut issues, true)? else {
        return Ok(issues.rows);
    };
    check(&source, cancel)?;
    let lane_manifest = load_json(&mut source, LANES, &mut issues, false)?;
    let lane_ids = if let Some(lanes) = lane_manifest
        .as_ref()
        .and_then(|v| v.get("lanes"))
        .and_then(Value::as_object)
    {
        lanes.keys().map(String::as_str).collect::<BTreeSet<_>>()
    } else {
        if lane_manifest.is_some() {
            issues.push(LANES, "lanes must be an object")?;
        }
        BTreeSet::new()
    };
    for (key, expected, message) in [
        (
            "schema_version",
            "tos_source_home_v1",
            "schema_version must be tos_source_home_v1",
        ),
        (
            "owner_repo",
            "Tree-of-Sophia",
            "owner_repo must be Tree-of-Sophia",
        ),
        ("home", "ToS/", "home must be ToS/"),
    ] {
        if manifest[key] != expected {
            issues.push(MANIFEST, message)?;
        }
    }
    let Some(branches) = manifest["branches"].as_array() else {
        issues.push(MANIFEST, "branches must be a list")?;
        return Ok(issues.rows);
    };
    let mut seen = BTreeSet::new();
    for branch in branches {
        check(&source, cancel)?;
        if !branch.is_object() {
            issues.push(MANIFEST, "each branch must be an object")?;
            continue;
        }
        let Some(id) = branch["id"].as_str().filter(|v| !v.is_empty()) else {
            issues.push(MANIFEST, "branch id must be a non-empty string")?;
            continue;
        };
        if !seen.insert(id) {
            issues.push(MANIFEST, format!("duplicate branch id {id}"))?;
        }
        if id.len() > 4092 {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "source-home branch path exceeds reader profile",
            ));
        }
        let expected = format!("ToS/{}", id.replace('_', "-"));
        if branch["path"] != expected {
            issues.push(MANIFEST, format!("{id}.path must be {expected}"))?;
        }
        if !source.is_dir(&expected)? {
            issues.push(&expected, "branch directory is missing")?;
        }
        if let Some(owner) = branch["owner_surface"].as_str().filter(|s| !s.is_empty()) {
            if !source.is_file(owner)? {
                issues.push(owner, format!("{id}.owner_surface is missing"))?;
            }
        } else {
            issues.push(
                MANIFEST,
                format!("{id}.owner_surface must be a non-empty string"),
            )?;
        }
        if branch.get("validators").is_some() {
            issues.push(
                MANIFEST,
                format!("{id}.validators must move to validation_lanes"),
            )?;
        }
        if let Some(lanes) = branch["validation_lanes"]
            .as_array()
            .filter(|a| !a.is_empty())
        {
            for lane in lanes {
                if let Some(lane) = lane.as_str().filter(|s| !s.is_empty()) {
                    if !lane_ids.contains(lane) {
                        issues.push(
                            MANIFEST,
                            format!("{id}.validation_lanes references missing lane {lane}"),
                        )?;
                    }
                } else {
                    issues.push(
                        MANIFEST,
                        format!("{id}.validation_lanes must contain strings"),
                    )?;
                }
            }
        } else {
            issues.push(
                MANIFEST,
                format!("{id}.validation_lanes must be a non-empty list"),
            )?;
        }
    }
    let missing = CORE
        .into_iter()
        .filter(|id| !seen.contains(id))
        .collect::<BTreeSet<_>>();
    for id in missing {
        issues.push(MANIFEST, format!("missing core branch id {id}"))?;
    }
    check(&source, cancel)?;
    Ok(issues.rows)
}

pub fn run(root: &Path, cancel: &AtomicI32) -> io::Result<i32> {
    let issues = match run_validation(root, cancel) {
        Ok(issues) => issues,
        Err(error) => {
            let signal = cancel.load(Ordering::Relaxed);
            if signal != 0 {
                return Ok(128 + signal);
            }
            return Err(error);
        }
    };
    let signal = cancel.load(Ordering::Relaxed);
    if signal != 0 {
        return Ok(128 + signal);
    }
    let code = if issues.is_empty() {
        println!("[ok] validated ToS source-home manifest and branch topology");
        0
    } else {
        let mut stderr = io::stderr().lock();
        writeln!(stderr, "ToS source-home validation failed.")?;
        for (location, message) in issues {
            writeln!(stderr, "- {location}: {message}")?;
        }
        1
    };
    let signal = cancel.load(Ordering::Relaxed);
    Ok(if signal != 0 { 128 + signal } else { code })
}

#[cfg(test)]
mod tests {
    #[test]
    fn source_home_manifest_matches_its_declared_schema() {
        let schema: serde_json::Value = serde_json::from_slice(include_bytes!(
            "../../../../ToS/contracts/tos-source-home.schema.json"
        ))
        .unwrap();
        let manifest: serde_json::Value =
            serde_json::from_slice(include_bytes!("../../../../ToS/source_home.manifest.json"))
                .unwrap();
        assert_eq!(
            manifest["$schema"],
            "https://tree-of-sophia.local/ToS/contracts/tos-source-home.schema.json"
        );
        let validator = jsonschema::options()
            .with_draft(jsonschema::Draft::Draft202012)
            .should_validate_formats(false)
            .offline()
            .build(&schema)
            .unwrap();
        assert!(validator.is_valid(&manifest));
    }
}
