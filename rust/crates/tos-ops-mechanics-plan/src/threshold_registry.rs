//! The one Agon threshold-registry companion. Authored config and schemas
//! remain owners; this route produces and checks candidate-only mechanics.

use jsonschema::Draft;
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::Path;
use tos_foundation::{
    CanonicalProfile, Digest256, JsonLimits, JsonMode, JsonNumber, JsonNumberKind, JsonString,
    JsonValue, canonical_bytes_v1, parse_json,
};

const PART: &str = "mechanics/agon/parts/threshold-registry";
const SOURCE: &str = "config/tos_agon_threshold_intakes.config.json";
const OUTPUT: &str = "generated/tos_agon_threshold_intake_registry.min.json";
const REGISTRY_SCHEMA: &str = "schemas/tos-agon-threshold-intake-registry.schema.json";
const ENTRY_SCHEMA: &str = "../threshold-intake/schemas/tos-agon-threshold-intake.schema.json";
const ITEM_KEY: &str = "threshold_intakes";
const REGISTRY_ID: &str = "tos.agon_threshold_intake.registry.v1";
const REVIEW_ORDER: &str = "XVIII";
const REVIEW_LABEL: &str = "Sophian Threshold";
const CANDIDATE: &str = "candidate_only";
const REQUIRED_FORBIDDEN: &[&str] = &[
    "live_verdict_authority",
    "durable_scar_write",
    "retention_execution",
    "rank_mutation",
    "trust_mutation",
    "tree_of_sophia_canon_write",
    "direct_tos_write",
    "automatic_canonization",
    "kag_as_canon",
    "hidden_scheduler_action",
    "assistant_contestant_drift",
    "auto_doctrine_rewrite",
    "center_takeover_of_owner_truth",
    "eval_memo_stats_sovereignty",
    "stats_as_canon_authority",
    "eval_as_canon_authority",
    "memo_as_canon_authority",
    "sdk_hidden_write",
];
const ALLOWED_RUNTIME: &[&str] = &[
    "none",
    "candidate_only",
    "local_dry_run_candidate_only",
    "local_rehearsal_candidate_only",
];

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}
fn selected_root(root: &Path) -> io::Result<()> {
    if !root.is_absolute() || fs::canonicalize(root)? != root {
        return Err(invalid(
            "repository root must be an absolute path without symlinks",
        ));
    }
    Ok(())
}
fn read(root: &Path, relative: &str) -> io::Result<Vec<u8>> {
    let path = root.join(PART).join(relative);
    let mut file = File::open(path)?;
    let metadata = file.metadata()?;
    // Use the existing foundation codec bound for this native route; the
    // owner schema, rather than this bound, specifies the item count.
    if metadata.len() > JsonLimits::default().max_bytes as u64 {
        return Err(invalid(format!(
            "threshold registry input exceeds JSON bound: {relative}"
        )));
    }
    let mut raw = Vec::new();
    file.take(JsonLimits::default().max_bytes as u64 + 1)
        .read_to_end(&mut raw)?;
    if raw.len() > JsonLimits::default().max_bytes {
        return Err(invalid(format!(
            "threshold registry input exceeds JSON bound: {relative}"
        )));
    }
    Ok(raw)
}
fn text(value: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(value))
}
fn field(name: &str, value: JsonValue) -> (JsonString, JsonValue) {
    (JsonString::from_utf8(name), value)
}
fn canonical(value: &JsonValue, profile: CanonicalProfile) -> io::Result<Vec<u8>> {
    canonical_bytes_v1(value, profile, JsonLimits::default())
        .map_err(|error| invalid(format!("threshold canonical JSON: {error:?}")))
}
fn source(root: &Path) -> io::Result<(Value, Vec<u8>)> {
    let raw = read(root, SOURCE)?;
    // Python json.load has last-value-wins input semantics. The emitted
    // companion is separately canonical and compared byte-for-byte by check.
    let document = parse_json(&raw, JsonMode::RequestLastWins, JsonLimits::default())
        .map_err(|error| invalid(format!("threshold source JSON: {error:?}")))?;
    let root_value = document.root();
    for (name, expected) in [
        ("registry_id", REGISTRY_ID),
        ("review_phase_order", REVIEW_ORDER),
        ("review_phase_label", REVIEW_LABEL),
        ("runtime_posture", CANDIDATE),
    ] {
        if root_value.object_get(name).and_then(JsonValue::as_str) != Some(expected) {
            return Err(invalid(format!("source {name} must be {expected}")));
        }
    }
    let items = root_value
        .object_get(ITEM_KEY)
        .and_then(JsonValue::as_array)
        .ok_or_else(|| invalid("source threshold_intakes must be a list"))?;
    let items_value = JsonValue::Array(items.to_vec());
    let items_bytes = canonical(&items_value, CanonicalProfile::SourceRecordDigestV1)?;
    let count = JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: items.len().to_string(),
    });
    let registry = JsonValue::Object(vec![
        field("registry_id", text(REGISTRY_ID)),
        field("review_phase_order", text(REVIEW_ORDER)),
        field("review_phase_label", text(REVIEW_LABEL)),
        field("runtime_posture", text(CANDIDATE)),
        field("count", count),
        field(ITEM_KEY, items_value),
        field("digest", text(&Digest256::of_bytes(&items_bytes).to_hex())),
    ]);
    let expected = canonical(&registry, CanonicalProfile::CorpusSnapshotV1)?;
    let source_view: Value = serde_json::from_slice(&canonical(
        root_value,
        CanonicalProfile::SourceRecordDigestV1,
    )?)
    .map_err(|error| invalid(format!("threshold source representation: {error}")))?;
    Ok((source_view, expected))
}

/// Build or byte-check the one generated companion. The no-check path is a
/// part-local generated write, never a ToS canon/source write.
pub fn build(root: &Path, check: bool) -> io::Result<()> {
    selected_root(root)?;
    let (_, expected) = source(root)?;
    let path = root.join(PART).join(OUTPUT);
    if check {
        let actual = match read(root, OUTPUT) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(invalid("missing generated registry"));
            }
            value => value?,
        };
        if actual != expected {
            return Err(invalid(
                "generated registry drift: run builder without --check",
            ));
        }
    } else {
        fs::create_dir_all(path.parent().ok_or_else(|| invalid("generated parent"))?)?;
        fs::write(path, expected)?;
    }
    Ok(())
}

fn nonempty_strings(item: &Value, field: &str, id: &str) -> io::Result<()> {
    let values = item
        .get(field)
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty())
        .ok_or_else(|| invalid(format!("{id} {field} must be a non-empty list")))?;
    if values
        .iter()
        .any(|value| value.as_str().is_none_or(str::is_empty))
    {
        return Err(invalid(format!(
            "{id} {field} must contain non-empty strings only"
        )));
    }
    Ok(())
}
fn item_contract(item: &Value) -> io::Result<String> {
    let id = item
        .get("intake_id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| invalid("missing intake_id"))?;
    if item.get("review_phase_order").and_then(Value::as_str) != Some(REVIEW_ORDER)
        || item.get("live_protocol") != Some(&Value::Bool(false))
        || item.get("assistant_contestant_allowed") != Some(&Value::Bool(false))
        || item.get("review_status").and_then(Value::as_str) != Some(CANDIDATE)
        || item.get("requires_review") != Some(&Value::Bool(true))
        || !item
            .get("runtime_effect")
            .and_then(Value::as_str)
            .is_some_and(|value| ALLOWED_RUNTIME.contains(&value))
        || [
            "tos_canonization_allowed",
            "direct_tos_write_allowed",
            "canon_write_allowed",
        ]
        .iter()
        .any(|field| item.get(*field) != Some(&Value::Bool(false)))
    {
        return Err(invalid(format!(
            "{id} violates candidate-only review or authority boundary"
        )));
    }
    nonempty_strings(item, "required_evidence", id)?;
    nonempty_strings(item, "forbidden_effects", id)?;
    let forbidden: BTreeSet<_> = item["forbidden_effects"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    for required in REQUIRED_FORBIDDEN {
        if !forbidden.contains(required) {
            return Err(invalid(format!("{id} missing forbidden effect {required}")));
        }
    }
    Ok(id.to_owned())
}

/// Validate source, both current schemas, generated equality and the owner
/// candidate-only rules. Validators are prepared once for the whole call.
pub fn validate(root: &Path) -> io::Result<Value> {
    selected_root(root)?;
    let (source, expected_raw) = source(root)?;
    let actual_raw = read(root, OUTPUT)?;
    let entry_schema: Value = serde_json::from_slice(&read(root, ENTRY_SCHEMA)?)
        .map_err(|error| invalid(format!("entry schema JSON: {error}")))?;
    let registry_schema: Value = serde_json::from_slice(&read(root, REGISTRY_SCHEMA)?)
        .map_err(|error| invalid(format!("registry schema JSON: {error}")))?;
    let entry_validator = jsonschema::options()
        .with_draft(Draft::Draft202012)
        .offline()
        .build(&entry_schema)
        .map_err(|error| invalid(format!("entry schema: {error}")))?;
    let registry_validator = jsonschema::options()
        .with_draft(Draft::Draft202012)
        .offline()
        .build(&registry_schema)
        .map_err(|error| invalid(format!("registry schema: {error}")))?;
    let items = source
        .get(ITEM_KEY)
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty())
        .ok_or_else(|| invalid("source threshold_intakes must not be empty"))?;
    let mut seen = BTreeSet::new();
    for item in items {
        if !item.is_object() {
            return Err(invalid("source threshold_intakes entries must be objects"));
        }
        if !entry_validator.is_valid(item) {
            return Err(invalid("source intake_id entry schema violation"));
        }
        let id = item_contract(item)?;
        if !seen.insert(id) {
            return Err(invalid("duplicate intake_id"));
        }
    }
    let actual: Value = serde_json::from_slice(&actual_raw)
        .map_err(|error| invalid(format!("generated registry JSON: {error}")))?;
    let expected: Value = serde_json::from_slice(&expected_raw)
        .map_err(|error| invalid(format!("expected registry JSON: {error}")))?;
    if actual != expected {
        return Err(invalid(
            "generated registry is stale or does not match builder output",
        ));
    }
    if !registry_validator.is_valid(&actual) {
        return Err(invalid("generated registry schema violation"));
    }
    if actual.get("count").and_then(Value::as_u64) != Some(items.len() as u64)
        || actual.get(ITEM_KEY).and_then(Value::as_array).map(Vec::len) != Some(items.len())
    {
        return Err(invalid("generated count does not match source items"));
    }
    Ok(serde_json::json!({"ok":true,"item_key":ITEM_KEY,"count":items.len()}))
}
