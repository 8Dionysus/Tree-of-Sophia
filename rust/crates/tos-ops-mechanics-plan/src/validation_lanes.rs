//! Native consumer of the existing ToS validation-lane command manifest.
//! The authored JSON remains the sole command authority. Native sequences do
//! not require an interpreter; a retained Python step requires explicit selection.

use serde_json::Value;
use std::fs;
use std::io::{self, Read};
use std::path::Path;

pub type Issue = (String, String);
pub type CommandStep = (String, Vec<String>);
pub type BudgetedCommandStep = (CommandStep, Option<u64>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReleasePhase {
    All,
    Checks,
    Tests,
}

const MANIFEST: &str = "docs/validation/validation_lanes.json";
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn issue(issues: &mut Vec<Issue>, location: &str, detail: impl Into<String>) -> io::Result<()> {
    issues.push((location.to_owned(), detail.into()));
    Ok(())
}

fn read_manifest(root: &Path) -> io::Result<Option<Value>> {
    let path = root.join(MANIFEST);
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let file = match options.open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(invalid("validation lane manifest must be a regular file"));
    }
    if metadata.len() > MAX_MANIFEST_BYTES {
        return Err(invalid("validation lane manifest byte budget exceeded"));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(MAX_MANIFEST_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES || bytes.len() as u64 != metadata.len() {
        return Err(invalid(
            "validation lane manifest changed or exceeded byte budget",
        ));
    }
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|error| invalid(format!("invalid JSON: {error}")))
}

fn valid_command(value: Option<&Value>) -> bool {
    value.and_then(Value::as_array).is_some_and(|parts| {
        !parts.is_empty()
            && parts
                .iter()
                .all(|part| part.as_str().is_some_and(|part| !part.is_empty()))
    })
}

// Per-step deadlines are explicit Rust-lane source inputs, not global defaults.
fn command_timeout_ms(
    sequence: &str,
    step: &serde_json::Map<String, Value>,
) -> io::Result<Option<u64>> {
    let Some(value) = step.get("command_timeout_ms") else {
        return Ok(None);
    };
    if sequence != "rust_workspace" {
        return Err(invalid(
            "command_timeout_ms is only supported for rust_workspace",
        ));
    }
    let value = value
        .as_u64()
        .filter(|value| *value > 0 && *value <= 3_600_000)
        .ok_or_else(|| invalid("command_timeout_ms must be an integer in 1..=3600000"))?;
    Ok(Some(value))
}

/// Ordered mechanical findings matching the maintained validation-lanes
/// checker on an admissible manifest. A malformed JSON syntax refusal is
/// typed but is not represented as a successful manifest.
pub fn validate_manifest(root: &Path) -> io::Result<Vec<Issue>> {
    let mut issues = Vec::new();
    let Some(manifest) = read_manifest(root)? else {
        issue(&mut issues, MANIFEST, "missing validation lane manifest")?;
        return Ok(issues);
    };
    let Some(manifest) = manifest.as_object() else {
        issue(&mut issues, MANIFEST, "manifest root must be an object")?;
        return Ok(issues);
    };
    for (key, expected, message) in [
        (
            "schema_version",
            "tos_validation_lanes_v1",
            "schema_version must be tos_validation_lanes_v1",
        ),
        (
            "owner_repo",
            "Tree-of-Sophia",
            "owner_repo must be Tree-of-Sophia",
        ),
        (
            "command_authority",
            MANIFEST,
            "command_authority must point at this manifest",
        ),
    ] {
        if manifest.get(key).and_then(Value::as_str) != Some(expected) {
            issue(&mut issues, MANIFEST, message)?;
        }
    }
    let Some(lanes) = manifest
        .get("lanes")
        .and_then(Value::as_object)
        .filter(|lanes| !lanes.is_empty())
    else {
        issue(&mut issues, MANIFEST, "lanes must be a non-empty object")?;
        return Ok(issues);
    };
    let sequences = manifest.get("command_sequences").and_then(Value::as_object);
    if sequences.is_none() {
        issue(&mut issues, MANIFEST, "command_sequences must be an object")?;
    }
    let empty = serde_json::Map::new();
    let sequences = sequences.unwrap_or(&empty);
    for (lane_id, lane) in lanes {
        if lane_id.is_empty() {
            issue(&mut issues, MANIFEST, "lane id must be a non-empty string")?;
            continue;
        }
        let Some(lane) = lane.as_object() else {
            issue(
                &mut issues,
                MANIFEST,
                format!("{lane_id} lane must be an object"),
            )?;
            continue;
        };
        for field in [
            "label",
            "layer",
            "mode",
            "owner_surface",
            "purpose",
            "failure_route",
            "does_not_own",
        ] {
            if !lane.contains_key(field) {
                issue(
                    &mut issues,
                    MANIFEST,
                    format!("{lane_id}.{field} is required"),
                )?;
            }
        }
        for field in ["owner_surface", "failure_route"] {
            if let Some(path) = lane
                .get(field)
                .and_then(Value::as_str)
                .filter(|path| !path.is_empty())
            {
                if !root.join(path).exists() {
                    issue(&mut issues, path, format!("{lane_id}.{field} is missing"))?;
                }
            }
        }
        if !lane
            .get("does_not_own")
            .and_then(Value::as_array)
            .is_some_and(|values| {
                values
                    .iter()
                    .all(|value| value.as_str().is_some_and(|text| !text.is_empty()))
            })
        {
            issue(
                &mut issues,
                MANIFEST,
                format!("{lane_id}.does_not_own must be a non-empty string list"),
            )?;
        }
        let sequence = lane
            .get("command_sequence")
            .filter(|value| !value.is_null());
        let focused_target = lane.get("focused_target").filter(|value| !value.is_null());
        if sequence.is_none() && focused_target.is_none() {
            issue(
                &mut issues,
                MANIFEST,
                format!("{lane_id} needs command_sequence or focused_target"),
            )?;
        }
        if let Some(sequence) = sequence.and_then(Value::as_str) {
            if !sequences.contains_key(sequence) {
                issue(
                    &mut issues,
                    MANIFEST,
                    format!("{lane_id}.command_sequence references missing {sequence}"),
                )?;
            }
        }
    }
    for (sequence_id, steps) in sequences {
        if sequence_id.is_empty() {
            issue(
                &mut issues,
                MANIFEST,
                "command sequence id must be a non-empty string",
            )?;
            continue;
        }
        let Some(steps) = steps.as_array().filter(|steps| !steps.is_empty()) else {
            issue(
                &mut issues,
                MANIFEST,
                format!("{sequence_id} command sequence must be a non-empty list"),
            )?;
            continue;
        };
        for (index, step) in steps.iter().enumerate() {
            let location = format!("{sequence_id}[{index}]");
            let Some(step) = step.as_object() else {
                issue(
                    &mut issues,
                    MANIFEST,
                    format!("{location} must be an object"),
                )?;
                continue;
            };
            if !step
                .get("label")
                .and_then(Value::as_str)
                .is_some_and(|label| !label.is_empty())
            {
                issue(
                    &mut issues,
                    MANIFEST,
                    format!("{location}.label must be a non-empty string"),
                )?;
            }
            if !valid_command(step.get("command")) {
                issue(
                    &mut issues,
                    MANIFEST,
                    format!("{location}.command must be a non-empty string list"),
                )?;
            }
            if let Err(error) = command_timeout_ms(sequence_id, step) {
                issue(&mut issues, MANIFEST, format!("{location}: {error}"))?;
            }
        }
    }
    Ok(issues)
}

/// Select one sequence in authored order without imposing the global --check
/// gate. An empty `python` permits native commands only. `{repo_root}` resolves
/// to the explicitly selected canonical source root as one argv element.
pub fn command_sequence(
    root: &Path,
    sequence_id: &str,
    python: &str,
) -> io::Result<Vec<CommandStep>> {
    Ok(command_sequence_with_budgets(root, sequence_id, python)?
        .into_iter()
        .map(|(command, _)| command)
        .collect())
}

/// Read commands and their explicit deadlines from the same manifest snapshot.
pub fn command_sequence_with_budgets(
    root: &Path,
    sequence_id: &str,
    python: &str,
) -> io::Result<Vec<BudgetedCommandStep>> {
    let manifest = read_manifest(root)?.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "validation lane manifest is missing",
        )
    })?;
    let sequences = manifest
        .get("command_sequences")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("command_sequences must be an object"))?;
    let steps = sequences
        .get(sequence_id)
        .and_then(Value::as_array)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("'unknown command sequence: {sequence_id}'"),
            )
        })?;
    if steps.is_empty() {
        return Err(invalid(format!(
            "{sequence_id} must contain at least one command"
        )));
    }
    let mut resolved = Vec::with_capacity(steps.len());
    for step in steps {
        let step = step
            .as_object()
            .ok_or_else(|| invalid(format!("{sequence_id} contains a non-object step")))?;
        let label = step
            .get("label")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid(format!("{sequence_id} contains an invalid command step")))?;
        if !valid_command(step.get("command")) {
            return Err(invalid(format!(
                "{sequence_id} contains an invalid command step"
            )));
        }
        let mut parts: Vec<String> = step["command"]
            .as_array()
            .ok_or_else(|| invalid("command array disappeared"))?
            .iter()
            .map(|part| part.as_str().unwrap_or_default().to_owned())
            .collect();
        if parts[0] == "python" {
            if python.is_empty() || python.contains('\0') {
                return Err(invalid(
                    "retained Python step requires --python EXACT_INTERPRETER",
                ));
            }
            parts[0] = python.to_owned();
        }
        if parts.iter().any(|part| part == "{repo_root}") {
            let selected = root.canonicalize()?;
            let selected = selected
                .to_str()
                .ok_or_else(|| invalid("non-UTF-8 repository root"))?;
            for part in &mut parts {
                if part == "{repo_root}" {
                    *part = selected.to_owned();
                }
            }
        }
        let executor_key = match parts[0].as_str() {
            "tos" => Some("TOS_NATIVE_PREPARED_CONSUMER_BIN"),
            "tos-ops-mechanics-plan" => Some("TOS_OPS_MECHANICS_EXECUTOR"),
            "tos-validation-lanes" => Some("TOS_VALIDATION_LANES_EXECUTOR"),
            "tos-release-check" => Some("TOS_RELEASE_CHECK_EXECUTOR"),
            "tos-software-ci" => Some("TOS_SOFTWARE_CI_EXECUTOR"),
            _ => None,
        };
        if let Some(key) = executor_key
            && let Some(selected) = std::env::var_os(key)
        {
            let selected = selected
                .into_string()
                .map_err(|_| invalid("non-UTF-8 selected native executor"))?;
            if selected.is_empty() {
                return Err(invalid("empty selected native executor"));
            }
            parts[0] = selected;
        }
        resolved.push((
            (label.to_owned(), parts),
            command_timeout_ms(sequence_id, step)?,
        ));
    }
    Ok(resolved)
}

/// Preserve the maintained release_check phase split. Only the split phases
/// require a final test-step suffix; `all` executes authored order.
pub fn release_steps(
    root: &Path,
    python: &str,
    phase: ReleasePhase,
) -> io::Result<Vec<CommandStep>> {
    let mut steps = command_sequence(root, "release_check", python)?;
    if phase == ReleasePhase::All {
        return Ok(steps);
    }
    let positions: Vec<usize> = steps
        .iter()
        .enumerate()
        .filter_map(|(index, (label, _))| {
            (label == "run tests" || label.starts_with("run tests: ")).then_some(index)
        })
        .collect();
    let labels: Vec<&str> = positions
        .iter()
        .map(|index| steps[*index].0.as_str())
        .collect();
    let complete_suffix = positions
        .first()
        .is_some_and(|first| positions == (*first..steps.len()).collect::<Vec<_>>());
    let unique_labels = labels
        .iter()
        .enumerate()
        .all(|(index, label)| !labels[..index].contains(label));
    let named_groups = labels.iter().all(|label| {
        *label == "run tests"
            || label
                .strip_prefix("run tests: ")
                .is_some_and(|name| !name.trim().is_empty())
    });
    let legacy_is_single =
        !labels.contains(&"run tests") || (labels.len() == 1 && labels[0] == "run tests");
    if !complete_suffix || !unique_labels || !named_groups || !legacy_is_single {
        return Err(invalid(
            "selected sequence must end with one legacy run tests step or a complete run tests: group suffix",
        ));
    }
    let test_start = positions[0];
    if phase == ReleasePhase::Tests {
        Ok(steps.split_off(test_start))
    } else {
        steps.truncate(test_start);
        Ok(steps)
    }
}

#[cfg(test)]
mod timeout_tests {
    use super::*;

    #[test]
    fn explicit_command_deadline_refuses_malformed_or_unapplied_budget() {
        let absent = serde_json::Map::new();
        assert_eq!(command_timeout_ms("rust_workspace", &absent).unwrap(), None);
        let budget = serde_json::json!({"command_timeout_ms": 900000});
        let budget = budget.as_object().unwrap();
        assert_eq!(
            command_timeout_ms("rust_workspace", budget).unwrap(),
            Some(900000)
        );
        assert!(command_timeout_ms("software_browser", budget).is_err());
        for value in [
            serde_json::Value::Null,
            serde_json::json!(true),
            serde_json::json!(0),
            serde_json::json!(-1),
            serde_json::json!(900000.0),
            serde_json::json!(3600001),
        ] {
            let step = serde_json::json!({"command_timeout_ms": value});
            assert!(command_timeout_ms("rust_workspace", step.as_object().unwrap()).is_err());
        }
    }
}
