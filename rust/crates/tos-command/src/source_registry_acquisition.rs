//! Native executor for the reviewed registry-source acquisition route.
//!
//! The frozen preparation remains the selection authority. This module checks
//! that selection, verifies provider bytes and records bounded local custody;
//! it does not admit corpus records or decide source meaning, rights, or canon.

use base64::Engine as _;
use serde_json::{Map, Value, json};
use sha1::{Digest as _, Sha1};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tos_foundation::Digest256;
use xml::reader::{ParserConfig, XmlEvent};

use crate::source_acquisition_batch as custody;

const SOURCE: &str = "ToS/source-witnesses";
const TOPOLOGY: &str = "ToS/source-witnesses/relations/provenance.jsonl";
const TOPOLOGY_EVENT: &str =
    "tos.event.annotation.source-witness-bibliographic-topology.2026-07-31";
const MAX_JSON: u64 = 16 * 1024 * 1024;
const ALLOWED_REPOSITORIES: &[&str] = &[
    "openscriptures/morphhb",
    "oraec/corpus_raw_data",
    "suttacentral/bilara-data",
    "PerseusDL/canonical-greekLit",
    "PerseusDL/canonical-latinLit",
];

type Result<T> = std::result::Result<T, String>;

#[cfg(test)]
thread_local! {
    static TEST_FETCH_CALLBACK: RefCell<Option<Box<dyn Fn(&Value) -> Result<Vec<u8>>>>> = const { RefCell::new(None) };
}

fn fetch_source_bytes(payload: &Value) -> Result<Vec<u8>> {
    #[cfg(test)]
    if let Some(result) =
        TEST_FETCH_CALLBACK.with(|callback| callback.borrow().as_ref().map(|fetch| fetch(payload)))
    {
        return result;
    }
    custody::fetch_request(payload)
}

fn field<'a>(value: &'a Value, key: &str) -> Result<&'a Value> {
    value
        .get(key)
        .ok_or_else(|| format!("missing field: {key}"))
}
fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    field(value, key)?
        .as_str()
        .ok_or_else(|| format!("{key} must be a string"))
}
fn arr<'a>(value: &'a Value, key: &str) -> Result<&'a Vec<Value>> {
    field(value, key)?
        .as_array()
        .ok_or_else(|| format!("{key} must be an array"))
}
fn object<'a>(value: &'a Value, key: &str) -> Result<&'a Map<String, Value>> {
    field(value, key)?
        .as_object()
        .ok_or_else(|| format!("{key} must be an object"))
}
fn u64_field(value: &Value, key: &str) -> Result<u64> {
    field(value, key)?
        .as_u64()
        .ok_or_else(|| format!("{key} must be a nonnegative integer"))
}
fn sha256(body: &[u8]) -> String {
    Digest256::of_bytes(body).to_hex()
}
fn json_bytes(value: &Value) -> Result<Vec<u8>> {
    let mut body = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    body.push(b'\n');
    Ok(body)
}
fn strict_json(body: &[u8]) -> Result<Value> {
    custody::parse(body)
}
fn read_json(path: &Path) -> Result<Value> {
    custody::read_json(path)
}
fn read_ref(root: &Path, reference: &str) -> Result<Vec<u8>> {
    let path = custody::path_under(root, reference)?;
    custody::read_bytes(&path, None, false, false, MAX_JSON)
}
fn json_ref(root: &Path, reference: &str) -> Result<Value> {
    strict_json(&read_ref(root, reference)?)
}
fn path_ref(root: &Path, reference: &str) -> Result<PathBuf> {
    custody::path_under(root, reference)
}
fn payload_destination(root: &Path, item_root_ref: &str, relative: &str) -> Result<PathBuf> {
    let item = item_root_ref
        .strip_prefix("ToS/source-witnesses/")
        .ok_or("payload Item outside source witnesses")?;
    if !relative.starts_with("payload/") {
        return Err("payload path must begin with payload/".into());
    }
    path_ref(root, &format!("{item}/{relative}"))
}
fn publish_json(path: &Path, value: &Value) -> Result<()> {
    custody::publish(path, &json_bytes(value)?, 0o644).map(|_| ())
}

fn absolute_path(value: &Value, key: &str) -> Result<PathBuf> {
    let path = PathBuf::from(text(value, key)?);
    if !path.is_absolute() {
        return Err(format!("{key} must be absolute"));
    }
    Ok(path)
}

fn optional_absolute_path(value: &Value, key: &str) -> Result<Option<PathBuf>> {
    match value.get(key) {
        None => Ok(None),
        Some(_) => absolute_path(value, key).map(Some),
    }
}

fn optional_bool(value: &Value, key: &str, default: bool) -> Result<bool> {
    match value.get(key) {
        None => Ok(default),
        Some(value) => value
            .as_bool()
            .ok_or_else(|| format!("{key} must be a boolean")),
    }
}

fn optional_string_array(value: &Value, key: &str) -> Result<Vec<String>> {
    match value.get(key) {
        None => Ok(Vec::new()),
        Some(value) => value
            .as_array()
            .ok_or_else(|| format!("{key} must be an array of strings"))?
            .iter()
            .map(|item| {
                item.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| format!("{key} entries must be strings"))
            })
            .collect(),
    }
}

fn operation(request: &Value) -> Result<&str> {
    request
        .get("operation")
        .and_then(Value::as_str)
        .ok_or_else(|| "registry request operation is missing".to_owned())
}

/// JSON request boundary used by the owner dispatcher and the Python shim.
/// Production requests use `registry.acquire`, `registry.verify_preparation`,
/// or `registry.verify_local`; helper operations preserve the import surface
/// consumed by the existing Python unit tests.
pub fn invoke(request: &Value) -> Result<Value> {
    match operation(request)? {
        "registry.acquire" | "registry.verify_preparation" | "registry.verify_local" => {
            run_command(request)
        }
        operation if operation.starts_with("registry.helper.") => helper(
            operation.trim_start_matches("registry.helper."),
            field(request, "args")?,
        ),
        _ => Err("unsupported registry acquisition operation".into()),
    }
}

fn helper(name: &str, args: &Value) -> Result<Value> {
    match name {
        "sha256" => Ok(Value::String(sha256(&decode_bytes(args, "body")?))),
        "utcnow" => Ok(Value::String(utcnow()?)),
        "event" => {
            let inputs = arr(args, "inputs")?.clone();
            let outputs = arr(args, "outputs")?.clone();
            let receipts = arr(args, "receipts")?
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| "event receipt refs must be strings".to_owned())
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(event(
                text(args, "event_id")?,
                text(args, "event_type")?,
                text(args, "started")?,
                text(args, "ended")?,
                inputs,
                outputs,
                text(args, "name")?,
                field(args, "configuration")?.clone(),
                text(args, "rights_ref")?,
                receipts,
            ))
        }
        "json_bytes" => Ok(Value::String(base64_encode(&json_bytes(field(
            args, "value",
        )?)?))),
        "strict_json" => strict_json(&decode_bytes(args, "body")?),
        "manifest_fixity" => Ok(Value::String(manifest_fixity(field(args, "manifest")?)?)),
        "safe_path" => {
            let root = absolute_path(args, "root")?;
            let path = path_ref(&root, text(args, "reference")?)?;
            Ok(Value::String(path.to_string_lossy().into_owned()))
        }
        "check_file" => {
            let body = decode_bytes(args, "body")?;
            Ok(Value::String(check_file(&body, field(args, "entry")?)?))
        }
        "validate_json" => {
            validate_json(
                &absolute_path(args, "root")?,
                field(args, "value")?,
                text(args, "schema_name")?,
            )?;
            Ok(Value::Null)
        }
        "rights_with_file_scopes" => {
            rights_with_file_scopes(field(args, "rights")?, field(args, "manifest")?)
        }
        "operation_date" => Ok(Value::String(operation_date(field(args, "target")?)?)),
        "selected_metadata_observations" => Ok(Value::Array(selected_metadata_observations(
            field(args, "preparation")?,
            field(args, "target")?,
        )?)),
        "metadata_elapsed_seconds" => Ok(json!(metadata_elapsed_seconds(args)?)),
        "tei_division_addresses" => {
            let root = parse_xml(text(args, "xml")?.as_bytes())?;
            let milestone = match args.get("milestone_unit") {
                None | Some(Value::Null) => None,
                Some(value) => Some(
                    value
                        .as_str()
                        .ok_or("milestone_unit must be a string or null")?,
                ),
            };
            let repeated_milestones = optional_bool(args, "collect_repeated_milestones", false)?;
            let repeated_divisions = optional_bool(args, "collect_repeated_divisions", false)?;
            let (addresses, milestone_repetitions, division_repetitions) =
                tei_division_addresses(&root, milestone, repeated_milestones, repeated_divisions)?;
            Ok(
                json!({"addresses":addresses,"repeated_milestones":milestone_repetitions,"repeated_divisions":division_repetitions}),
            )
        }
        "inspect_payloads" => {
            let target = field(args, "target")?;
            let bodies = arr(args, "bodies")?
                .iter()
                .map(|row| Ok((field(row, "entry")?.clone(), decode_bytes(row, "body")?)))
                .collect::<Result<Vec<_>>>()?;
            inspect_payloads(target, &bodies)
        }
        "load_preparation" => {
            let root = absolute_path(args, "root")?;
            let path = absolute_path(args, "path")?;
            let (manifest, packages) =
                load_preparation(&root, &path, optional_bool(args, "allow_unbound", false)?)?;
            Ok(json!({"manifest":manifest,"packages":packages}))
        }
        "validate_work_extension" => {
            let root = absolute_path(args, "root")?;
            let extension =
                validate_work_extension(&root, field(args, "target")?, field(args, "package")?)?;
            Ok(match extension {
                Some((path, before)) => {
                    json!({"path":path,"before_base64":base64_encode(&before)})
                }
                None => Value::Null,
            })
        }
        "preflight_identities" => {
            let root = absolute_path(args, "root")?;
            let targets = arr(args, "targets")?;
            let packages: BTreeMap<String, Value> = object(args, "packages")?
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect();
            preflight_identities(&root, targets, &packages)?;
            Ok(Value::Null)
        }
        "check_preparation_receipt" => {
            let root = absolute_path(args, "root")?;
            let manifest = absolute_path(args, "manifest_path")?;
            let receipt = absolute_path(args, "receipt_path")?;
            check_preparation_receipt(&root, &manifest, &receipt)
        }
        "append_jsonl" => {
            let path = absolute_path(args, "path")?;
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            append_jsonl(&path, field(args, "value")?)?;
            Ok(Value::Null)
        }
        "write_json" => {
            publish_json(&absolute_path(args, "path")?, field(args, "value")?)?;
            Ok(Value::Null)
        }
        "refresh_topology" => {
            refresh_topology(
                &absolute_path(args, "root")?,
                &absolute_path(args, "evidence_root")?,
                text(args, "ended")?,
            )?;
            Ok(Value::Null)
        }
        "verify_target" => {
            let payload_root = optional_absolute_path(args, "payload_source_root")?;
            verify_target(
                &absolute_path(args, "root")?,
                field(args, "target")?,
                payload_root.as_deref(),
            )
        }
        "transfer" => {
            let root = absolute_path(args, "root")?;
            let target = field(args, "target")?;
            let entry = field(args, "entry")?;
            let log = absolute_path(args, "log")?;
            let payload_root = optional_absolute_path(args, "payload_source_root")?;
            let (body, receipt) = transfer(
                &root,
                target,
                entry,
                &log,
                payload_root.as_deref(),
                optional_bool(args, "fetch_callback", false)?,
            )?;
            Ok(json!({"body_base64":base64_encode(&body),"receipt":receipt}))
        }
        "install_target" => {
            let payload_root = optional_absolute_path(args, "payload_source_root")?;
            install_target(
                &absolute_path(args, "root")?,
                &absolute_path(args, "manifest_path")?,
                field(args, "preparation")?,
                field(args, "target")?,
                field(args, "package")?,
                payload_root.as_deref(),
                optional_bool(args, "fetch_callback", false)?,
            )
        }
        "write_discovery" => {
            write_discovery(
                &absolute_path(args, "root")?,
                &absolute_path(args, "manifest_path")?,
                field(args, "preparation")?,
                field(args, "target")?,
                arr(args, "transfers")?,
                field(args, "acquisition")?,
            )?;
            Ok(Value::Null)
        }
        _ => Err(format!("unsupported registry helper: {name}")),
    }
}

fn decode_bytes(value: &Value, key: &str) -> Result<Vec<u8>> {
    let encoded = text(value, key)?;
    base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| format!("{key} is not valid base64"))
}
fn base64_encode(value: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(value)
}

fn manifest_fixity(manifest: &Value) -> Result<String> {
    Ok(arr(manifest, "payload_files")?
        .iter()
        .map(|entry| {
            Ok(format!(
                "{}  {}\n",
                text(entry, "sha256")?,
                text(entry, "relative_path")?
            ))
        })
        .collect::<Result<String>>()?)
}

fn rights_with_file_scopes(prepared: &Value, manifest: &Value) -> Result<Value> {
    let mut rights = prepared.clone();
    let rights_obj = rights
        .as_object_mut()
        .ok_or("rights record must be an object")?;
    let scope = rights_obj
        .get_mut("scope_refs")
        .and_then(Value::as_array_mut)
        .ok_or("rights scope_refs must be an array")?;
    let mut seen = scope
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    for entry in arr(manifest, "payload_files")? {
        let id = text(entry, "file_id")?;
        if seen.insert(id.to_owned()) {
            scope.push(Value::String(id.to_owned()));
        }
    }
    if let Some(layers) = rights_obj
        .get_mut("layer_assessments")
        .and_then(Value::as_array_mut)
    {
        for layer in layers {
            if layer.get("assessment_status").and_then(Value::as_str)
                == Some("public_domain_reviewed")
            {
                layer
                    .as_object_mut()
                    .ok_or("rights layer assessment must be an object")?
                    .insert(
                        "rights_statement_uri".into(),
                        Value::String("https://creativecommons.org/publicdomain/mark/1.0/".into()),
                    );
            }
        }
    }
    Ok(rights)
}

fn valid_slug(value: &str) -> bool {
    !value.is_empty()
        && value.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
}

fn operation_date(target: &Value) -> Result<String> {
    let day = target
        .get("operation_date")
        .and_then(Value::as_str)
        .unwrap_or("2026-09-08");
    let parts = day.split('-').collect::<Vec<_>>();
    if parts.len() != 3
        || parts[0].len() != 4
        || parts[1].len() != 2
        || parts[2].len() != 2
        || !parts
            .iter()
            .all(|part| part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return Err("invalid acquisition operation date".into());
    }
    let year = parts[0]
        .parse::<i32>()
        .map_err(|_| "invalid acquisition operation date")?;
    let month = parts[1]
        .parse::<u32>()
        .map_err(|_| "invalid acquisition operation date")?;
    let day_number = parts[2]
        .parse::<u32>()
        .map_err(|_| "invalid acquisition operation date")?;
    if month == 0 || month > 12 {
        return Err("invalid acquisition operation date".into());
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    if day_number == 0 || day_number > days[(month - 1) as usize] {
        return Err("invalid acquisition operation date".into());
    }
    Ok(day.to_owned())
}

fn selected_metadata_observations(preparation: &Value, target: &Value) -> Result<Vec<Value>> {
    let observations = arr(preparation, "metadata_observations")?;
    let Some(refs) = target.get("metadata_evidence_refs") else {
        let provider = text(target, "provider")?;
        return Ok(observations
            .iter()
            .filter(|value| {
                value
                    .get("retained_ref")
                    .and_then(Value::as_str)
                    .is_some_and(|reference| {
                        reference.contains(provider)
                            || provider == "bilara" && reference.contains("suttacentral")
                    })
            })
            .cloned()
            .collect());
    };
    let refs = refs
        .as_array()
        .ok_or("target metadata evidence refs must be an array")?;
    let selected = refs
        .iter()
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| "metadata evidence reference must be a string".to_owned())
        })
        .collect::<Result<Vec<_>>>()?;
    if selected.is_empty() || selected.iter().collect::<HashSet<_>>().len() != selected.len() {
        return Err("target metadata evidence refs are empty or duplicated".into());
    }
    let matching = observations
        .iter()
        .filter(|value| {
            value
                .get("retained_ref")
                .and_then(Value::as_str)
                .is_some_and(|reference| selected.contains(&reference))
        })
        .cloned()
        .collect::<Vec<_>>();
    let observed = matching
        .iter()
        .map(|value| text(value, "retained_ref"))
        .collect::<Result<HashSet<_>>>()?;
    if matching.len() != selected.len()
        || selected
            .iter()
            .any(|reference| !observed.contains(reference))
    {
        return Err("target metadata evidence is not exactly bound by the preparation".into());
    }
    Ok(matching)
}

fn parse_instant_seconds(value: &str) -> Result<f64> {
    let (local, offset) = if let Some(local) = value.strip_suffix('Z') {
        (local, 0i32)
    } else {
        let split = value
            .char_indices()
            .rev()
            .find(|(_, c)| *c == '+' || *c == '-')
            .map(|(index, _)| index)
            .ok_or("instant requires explicit valid timezone")?;
        let (local, zone) = value.split_at(split);
        let zone = &zone[1..];
        let (hours, minutes) = zone
            .split_once(':')
            .ok_or("instant requires explicit valid timezone")?;
        let hours = hours
            .parse::<i32>()
            .map_err(|_| "instant requires explicit valid timezone")?;
        let minutes = minutes
            .parse::<i32>()
            .map_err(|_| "instant requires explicit valid timezone")?;
        if hours > 23 || minutes > 59 {
            return Err("instant requires explicit valid timezone".into());
        }
        let sign = if value.as_bytes()[split] == b'+' {
            1
        } else {
            -1
        };
        (local, sign * (hours * 3600 + minutes * 60))
    };
    let (date, clock) = local
        .split_once('T')
        .ok_or("instant requires explicit valid timezone")?;
    let date = date.split('-').collect::<Vec<_>>();
    let clock = clock.split(':').collect::<Vec<_>>();
    if date.len() != 3 || clock.len() != 3 {
        return Err("instant requires explicit valid timezone".into());
    }
    let year = date[0]
        .parse::<i32>()
        .map_err(|_| "instant requires explicit valid timezone")?;
    let month = date[1]
        .parse::<u32>()
        .map_err(|_| "instant requires explicit valid timezone")?;
    let day = date[2]
        .parse::<u32>()
        .map_err(|_| "instant requires explicit valid timezone")?;
    let hour = clock[0]
        .parse::<u32>()
        .map_err(|_| "instant requires explicit valid timezone")?;
    let minute = clock[1]
        .parse::<u32>()
        .map_err(|_| "instant requires explicit valid timezone")?;
    let (second, fraction) = clock[2].split_once('.').unwrap_or((clock[2], ""));
    let second = second
        .parse::<u32>()
        .map_err(|_| "instant requires explicit valid timezone")?;
    let fraction = if fraction.is_empty() {
        0.0
    } else {
        if !fraction.bytes().all(|b| b.is_ascii_digit()) {
            return Err("instant requires explicit valid timezone".into());
        }
        format!("0.{fraction}")
            .parse::<f64>()
            .map_err(|_| "instant requires explicit valid timezone")?
    };
    if month == 0 || month > 12 || hour > 23 || minute > 59 || second > 59 {
        return Err("instant requires explicit valid timezone".into());
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let month_days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    if day == 0 || day > month_days[(month - 1) as usize] {
        return Err("instant requires explicit valid timezone".into());
    }
    let adjusted_year = year - i32::from(month <= 2);
    let era = adjusted_year.div_euclid(400);
    let year_of_era = adjusted_year - era * 400;
    let adjusted_month = month as i32 + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * adjusted_month + 2) / 5 + day as i32 - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = era as i64 * 146097 + day_of_era as i64 - 719468;
    Ok(
        (days * 86400 + hour as i64 * 3600 + minute as i64 * 60 + second as i64 - offset as i64)
            as f64
            + fraction,
    )
}

fn metadata_elapsed_seconds(observation: &Value) -> Result<f64> {
    let elapsed = if let Some(value) = observation.get("elapsed_seconds") {
        if value.is_boolean() {
            return Err("metadata receipt elapsed time is invalid".into());
        }
        value
            .as_f64()
            .ok_or("metadata receipt elapsed time is invalid")?
    } else {
        parse_instant_seconds(text(observation, "ended_at")?)?
            - parse_instant_seconds(text(observation, "started_at")?)?
    };
    if !elapsed.is_finite() || elapsed < 0.0 {
        return Err("metadata receipt elapsed time is invalid".into());
    }
    Ok(elapsed)
}

fn check_file(body: &[u8], entry: &Value) -> Result<String> {
    let expected_size = u64_field(entry, "byte_size")?;
    if body.len() as u64 != expected_size {
        return Err(format!("byte-size mismatch: {}", text(entry, "basename")?));
    }
    let mut git = Sha1::new();
    git.update(format!("blob {}\0", body.len()).as_bytes());
    git.update(body);
    let git = format!("{:x}", git.finalize());
    if git != text(entry, "git_blob_sha1")? {
        return Err(format!(
            "Git blob digest mismatch: {}",
            text(entry, "basename")?
        ));
    }
    Ok(sha256(body))
}

thread_local! {
    static SCHEMA_CACHE: RefCell<HashMap<PathBuf, Rc<tos_validation::SchemaBackendProbe>>> = RefCell::new(HashMap::new());
}

fn schema_probe(root: &Path) -> Result<Rc<tos_validation::SchemaBackendProbe>> {
    let directory = path_ref(root, "ToS/contracts")?;
    if let Some(probe) = SCHEMA_CACHE.with(|cache| cache.borrow().get(&directory).cloned()) {
        return Ok(probe);
    }
    let mut paths = fs::read_dir(&directory)
        .map_err(|error| error.to_string())?
        .map(|entry| {
            entry
                .map(|item| item.path())
                .map_err(|error| error.to_string())
        })
        .collect::<Result<Vec<_>>>()?;
    paths.sort();
    let mut resources = Vec::new();
    for path in paths {
        if path.extension().and_then(|extension| extension.to_str()) != Some("json")
            || !path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".schema.json"))
        {
            continue;
        }
        let raw = custody::read_bytes(&path, None, false, false, 4 * 1024 * 1024)?;
        let parsed = strict_json(&raw)?;
        resources.push(tos_validation::SchemaResource {
            uri: text(&parsed, "$id")?.to_owned(),
            raw,
        });
    }
    let probe = tos_validation::SchemaBackendProbe::new(
        resources,
        tos_validation::FormatProfile::LegacyPythonObserved20260923,
    )
    .map_err(|error| format!("source contract schema set refused: {error:?}"))?;
    let probe = Rc::new(probe);
    SCHEMA_CACHE.with(|cache| {
        cache.borrow_mut().insert(directory, Rc::clone(&probe));
    });
    Ok(probe)
}

fn validate_json(root: &Path, value: &Value, schema_name: &str) -> Result<()> {
    let probe = schema_probe(root)?;
    let schema_uri =
        format!("https://tree-of-sophia.local/ToS/contracts/{schema_name}.schema.json");
    let raw = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    if !probe
        .is_valid_raw(&schema_uri, &raw)
        .map_err(|error| format!("{schema_name} schema execution failed: {error:?}"))?
    {
        return Err(format!("{schema_name} schema validation failed"));
    }
    Ok(())
}

fn load_preparation(
    root: &Path,
    path: &Path,
    allow_unbound: bool,
) -> Result<(Value, BTreeMap<String, Value>)> {
    let manifest = read_json(path)?;
    if manifest.get("schema_version").and_then(Value::as_str)
        != Some("tos_registry_first_planting_preparation_v1")
        || manifest.get("status").and_then(Value::as_str) != Some("prepared-not-acquired")
    {
        return Err("not a reviewed registry acquisition preparation".into());
    }
    let package_ref = text(&manifest, "prepared_packages_ref")?;
    let package_path = path_ref(root, package_ref)?;
    let package_raw = custody::read_bytes(&package_path, None, false, false, MAX_JSON)?;
    if sha256(&package_raw) != text(&manifest, "prepared_packages_sha256")? {
        return Err("prepared source package digest mismatch".into());
    }
    let package_text =
        std::str::from_utf8(&package_raw).map_err(|_| "prepared source packages are not UTF-8")?;
    let mut packages = BTreeMap::new();
    for line in package_text.lines().filter(|line| !line.trim().is_empty()) {
        let package = strict_json(line.as_bytes())?;
        let slug = text(&package, "target_slug")?.to_owned();
        if packages.insert(slug, package.clone()).is_some() {
            return Err("duplicate prepared target".into());
        }
        for record in object(&package, "records")?.values() {
            validate_json(root, record, "corpus-record")?;
        }
        validate_json(root, field(&package, "rights")?, "rights-record")?;
        for claim in arr(&package, "claims")? {
            validate_json(root, field(claim, "record")?, "claim-packet")?;
        }
    }
    let snapshot_ref = manifest
        .get("source_registry_snapshot_ref")
        .and_then(Value::as_str);
    if snapshot_ref.is_none() && !allow_unbound {
        return Err("normalized source-registry snapshot is not bound".into());
    }
    if let Some(snapshot_ref) = snapshot_ref {
        let snapshot = read_ref(root, snapshot_ref)?;
        if sha256(&snapshot) != text(&manifest, "source_registry_snapshot_sha256")? {
            return Err("normalized source-registry snapshot digest mismatch".into());
        }
    }
    for observation in arr(&manifest, "metadata_observations")? {
        let body = read_ref(root, text(observation, "retained_ref")?)?;
        if body.len() as u64 != u64_field(observation, "retained_byte_size")?
            || sha256(&body) != text(observation, "retained_sha256")?
        {
            return Err("upstream evidence snapshot fixity mismatch".into());
        }
    }
    let targets = arr(&manifest, "targets")?;
    let mut target_slugs = BTreeSet::new();
    let mut destinations = BTreeSet::new();
    let mut total_bytes = 0u64;
    for target in targets {
        let slug = text(target, "slug")?;
        if !valid_slug(slug) || !target_slugs.insert(slug.to_owned()) {
            return Err("duplicate or invalid acquisition target".into());
        }
        let repository = text(target, "repository")?;
        let pin = text(target, "pin")?;
        if !ALLOWED_REPOSITORIES.contains(&repository)
            || pin.len() != 40
            || !pin
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err("unapproved repository or unpinned version".into());
        }
        let package = packages
            .get(slug)
            .ok_or("preparation target has no source package")?;
        operation_date(target)?;
        let work_slug = target
            .get("work_slug")
            .and_then(Value::as_str)
            .unwrap_or(slug);
        if !valid_slug(work_slug) {
            return Err("invalid existing Work path identity".into());
        }
        validate_work_extension(root, target, package)?;
        if target.get("metadata_evidence_refs").is_some() {
            selected_metadata_observations(&manifest, target)?;
        }
        for reference in object(package, "records")?.keys() {
            custody::safe_ref(reference)?;
            if !reference.starts_with(&format!(
                "{SOURCE}/works/{}/{work_slug}/",
                text(target, "family")?
            )) {
                return Err("prepared source record leaves its Work territory".into());
            }
        }
        for entry in arr(target, "files")? {
            let upstream = text(entry, "upstream_path")?;
            custody::safe_ref(upstream)?;
            if Path::new(upstream)
                .file_name()
                .and_then(|name| name.to_str())
                != Some(text(entry, "basename")?)
                || text(entry, "git_blob_sha1")?.len() != 40
                || !text(entry, "git_blob_sha1")?
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            {
                return Err("invalid upstream file identity".into());
            }
            let expected_url =
                format!("https://raw.githubusercontent.com/{repository}/{pin}/{upstream}");
            if text(entry, "url")? != expected_url
                || !(1..=8_000_000).contains(&u64_field(entry, "byte_size")?)
            {
                return Err("unbound source URL or oversized source file".into());
            }
            let item_root = text(field(target, "paths")?, "item_root")?;
            let destination = format!("{item_root}/payload/{}", text(entry, "basename")?);
            custody::safe_ref(&destination)?;
            if !destinations.insert(destination) {
                return Err("duplicate payload destination".into());
            }
            total_bytes = total_bytes
                .checked_add(u64_field(entry, "byte_size")?)
                .ok_or("manifest payload byte total overflow")?;
        }
    }
    if target_slugs != packages.keys().cloned().collect()
        || u64_field(field(&manifest, "totals")?, "payload_files")? != destinations.len() as u64
        || u64_field(field(&manifest, "totals")?, "payload_bytes")? != total_bytes
    {
        return Err("manifest total closure differs".into());
    }
    Ok((manifest, packages))
}

fn validate_work_extension(
    root: &Path,
    target: &Value,
    package: &Value,
) -> Result<Option<(String, Vec<u8>)>> {
    let Some(binding) = package
        .get("existing_work")
        .filter(|value| !value.is_null())
    else {
        return Ok(None);
    };
    let work_ref = text(field(target, "paths")?, "work")?;
    if text(binding, "record_ref")? != work_ref {
        return Err("existing Work binding names another record".into());
    }
    let before = read_ref(root, text(binding, "preimage_ref")?)?;
    if sha256(&before) != text(binding, "sha256")? {
        return Err("existing Work preimage digest mismatch".into());
    }
    let old = strict_json(&before)?;
    if old.get("record_type").and_then(Value::as_str) != Some("work")
        || old.get("record_id").and_then(Value::as_str)
            != Some(text(field(target, "ids")?, "work")?)
    {
        return Err("existing Work identity differs from the prepared target".into());
    }
    let incoming = arr(package, "claims")?
        .iter()
        .map(|claim| field(claim, "record"))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .filter(|record| {
            record.get("subject_ref") == old.get("record_id")
                && record.get("predicate").and_then(Value::as_str) == Some("has_expression")
        })
        .collect::<Vec<_>>();
    let new_expressions = object(package, "records")?
        .values()
        .filter(|record| {
            record.get("record_type").and_then(Value::as_str) == Some("expression")
                && record.get("work_ref") == old.get("record_id")
        })
        .filter_map(|record| record.get("record_id").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    let incoming_objects = incoming
        .iter()
        .filter_map(|claim| claim.get("object").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    if incoming.is_empty() || incoming_objects != new_expressions {
        return Err("Work extension does not close over its new Expressions".into());
    }
    let prior = arr(&old, "expression_claim_refs")?
        .iter()
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| "Work claim reference must be text".to_owned())
        })
        .collect::<Result<Vec<_>>>()?;
    let additions = incoming
        .iter()
        .map(|claim| text(claim, "claim_id"))
        .collect::<Result<Vec<_>>>()?;
    let mut all = prior.clone();
    all.extend(additions.iter().copied());
    if all.iter().collect::<HashSet<_>>().len() != all.len() {
        return Err("Work extension repeats an existing or prepared claim".into());
    }
    let mut expected = old.clone();
    expected
        .as_object_mut()
        .ok_or("existing Work record must be an object")?
        .insert("expression_claim_refs".into(), json!(all));
    let version = u64_field(&old, "record_version")?
        .checked_add(1)
        .ok_or("Work record version overflow")?;
    expected
        .as_object_mut()
        .ok_or("existing Work record must be an object")?
        .insert("record_version".into(), json!(version));
    if object(package, "records")?.get(work_ref) != Some(&expected) {
        return Err("Work extension changes fields outside additive Expression closure".into());
    }
    Ok(Some((work_ref.to_owned(), before)))
}

fn walk_files(root: &Path, directory: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| error.to_string())?
        .map(|entry| entry.map_err(|error| error.to_string()))
        .collect::<Result<Vec<_>>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
        if metadata.file_type().is_symlink() {
            return Err(format!("symlink in source path: {}", path.display()));
        }
        if metadata.is_dir() {
            walk_files(root, &path, files)?;
        } else if metadata.is_file() {
            let _ = path.strip_prefix(root).map_err(|error| error.to_string())?;
            files.push(path);
        }
    }
    Ok(())
}

fn preflight_identities(
    root: &Path,
    targets: &[Value],
    packages: &BTreeMap<String, Value>,
) -> Result<()> {
    let mut claims = BTreeMap::<String, (String, Value)>::new();
    let mut runs = BTreeMap::<String, (String, BTreeSet<String>)>::new();
    for target in targets {
        let slug = text(target, "slug")?;
        let package = packages.get(slug).ok_or("target source package missing")?;
        for claim in arr(package, "claims")? {
            let record = field(claim, "record")?;
            let claim_id = text(record, "claim_id")?.to_owned();
            let reference = text(claim, "path")?.to_owned();
            path_ref(root, &reference)?;
            if claims
                .insert(claim_id.clone(), (reference, record.clone()))
                .is_some()
            {
                return Err(format!(
                    "duplicate prepared bibliographic claim ID: {claim_id}"
                ));
            }
        }
        let day = operation_date(target)?;
        let run_id = format!("tos.discovery.registry-{slug}.{day}.v1");
        let run_ref = format!("{SOURCE}/discovery/runs/registry-{slug}.{day}.v1.json");
        custody::safe_ref(&run_ref)?;
        let ids = object(target, "ids")?
            .values()
            .map(|value| {
                value
                    .as_str()
                    .ok_or_else(|| "target ID must be text".to_owned())
                    .map(str::to_owned)
            })
            .collect::<Result<BTreeSet<_>>>()?;
        if runs.insert(run_id.clone(), (run_ref, ids)).is_some() {
            return Err(format!("duplicate prepared discovery ID: {run_id}"));
        }
    }
    let mut source_paths = Vec::new();
    let source_root = root.join(SOURCE);
    if source_root.exists() {
        walk_files(root, &source_root, &mut source_paths)?;
    }
    let allowed_names = [
        "work-expression-claims.jsonl",
        "expression-edition-claims.jsonl",
        "edition-item-claims.jsonl",
        "source-claims.jsonl",
    ];
    let mut claim_paths = source_paths
        .iter()
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| allowed_names.contains(&name))
        })
        .cloned()
        .collect::<BTreeSet<_>>();
    for (reference, _) in claims.values() {
        claim_paths.insert(path_ref(root, reference)?);
    }
    let mut seen = BTreeSet::new();
    for path in claim_paths {
        if !path.exists() {
            continue;
        }
        let reference = path
            .strip_prefix(root)
            .map_err(|error| error.to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        let raw = custody::read_bytes(&path, None, false, false, MAX_JSON)?;
        let body = std::str::from_utf8(&raw).map_err(|_| "claim stream is not UTF-8")?;
        for line in body.lines().filter(|line| !line.trim().is_empty()) {
            let record = strict_json(line.as_bytes())?;
            let Some(claim_id) = record.get("claim_id").and_then(Value::as_str) else {
                continue;
            };
            if let Some((prepared_ref, expected)) = claims.get(claim_id) {
                if !seen.insert(claim_id.to_owned())
                    || prepared_ref != &reference
                    || expected != &record
                {
                    return Err(format!(
                        "existing bibliographic claim differs from the prepared record: {claim_id} at {reference}"
                    ));
                }
            }
        }
    }
    let run_directory = root.join(SOURCE).join("discovery/runs");
    if run_directory.exists() {
        let mut paths = fs::read_dir(&run_directory)
            .map_err(|error| error.to_string())?
            .map(|entry| {
                entry
                    .map(|entry| entry.path())
                    .map_err(|error| error.to_string())
            })
            .collect::<Result<Vec<_>>>()?;
        paths.sort();
        for path in paths
            .into_iter()
            .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
        {
            let reference = path
                .strip_prefix(root)
                .map_err(|error| error.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            let run = read_json(&path)?;
            let run_id = run
                .get("discovery_id")
                .and_then(Value::as_str)
                .unwrap_or("");
            let expected_id = runs
                .iter()
                .find_map(|(identity, (run_ref, _))| {
                    (run_ref == &reference).then_some(identity.as_str())
                })
                .unwrap_or(run_id);
            if let Some((expected_ref, expected_ids)) = runs.get(expected_id) {
                let actual = run
                    .get("target")
                    .and_then(|target| target.get("known_tos_refs"))
                    .and_then(Value::as_array)
                    .ok_or("discovery run known_tos_refs is missing")?;
                let actual_ids = actual
                    .iter()
                    .map(|value| {
                        value
                            .as_str()
                            .ok_or_else(|| "discovery run target ref must be text".to_owned())
                            .map(str::to_owned)
                    })
                    .collect::<Result<Vec<_>>>()?;
                if run_id != expected_id
                    || &reference != expected_ref
                    || actual_ids.iter().collect::<BTreeSet<_>>()
                        != expected_ids.iter().collect::<BTreeSet<_>>()
                    || actual_ids.len() != expected_ids.len()
                {
                    return Err(format!(
                        "existing discovery run identity collision: {expected_id} at {reference}"
                    ));
                }
            }
        }
    }
    Ok(())
}

fn check_preparation_receipt(
    root: &Path,
    manifest_path: &Path,
    receipt_path: &Path,
) -> Result<Value> {
    let receipt = read_json(receipt_path)?;
    for key in [
        "manifest_sha256",
        "commit",
        "runtime_session_id",
        "checkpoint_review_ref",
        "passed_checks",
    ] {
        if receipt.get(key).is_none_or(|value| {
            value.is_null()
                || value.as_str() == Some("")
                || value.as_array().is_some_and(Vec::is_empty)
        }) {
            return Err("preparation receipt lacks checkpoint evidence".into());
        }
    }
    let raw_manifest = custody::read_bytes(manifest_path, None, false, false, MAX_JSON)?;
    if text(&receipt, "manifest_sha256")? != sha256(&raw_manifest) {
        return Err("preparation receipt does not bind this manifest".into());
    }
    let cancel = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(10);
    let output = crate::source_text_owner_ocr::bounded_process(
        "git",
        &["rev-parse", "HEAD"],
        Some(root),
        256,
        deadline,
        &cancel,
    )
    .map_err(|error| format!("unable to verify preparation commit: {error:?}"))?;
    let commit = std::str::from_utf8(&output)
        .map_err(|_| "git commit id is not UTF-8")?
        .trim();
    if text(&receipt, "commit")? != commit {
        return Err("preparation receipt does not bind the current repository commit".into());
    }
    let passed = arr(&receipt, "passed_checks")?;
    if passed
        .iter()
        .any(|value| value.as_str().is_none_or(str::is_empty))
    {
        return Err("preparation receipt passed_checks must name the completed checks".into());
    }
    if receipt
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("passed")
        != "passed"
    {
        return Err("preparation checkpoint is not passed".into());
    }
    Ok(receipt)
}

fn append_jsonl(path: &Path, value: &Value) -> Result<()> {
    let parent = path.parent().ok_or("JSONL path parent missing")?;
    let directory =
        tos_fd_open::open_absolute_directory(parent).map_err(|error| error.to_string())?;
    let name = path.file_name().ok_or("JSONL file name missing")?;
    let fd = rustix::fs::openat(
        &directory,
        name,
        rustix::fs::OFlags::WRONLY
            | rustix::fs::OFlags::APPEND
            | rustix::fs::OFlags::CREATE
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::from_raw_mode(0o644),
    )
    .map_err(|error| format!("JSONL append open failed: {error}"))?;
    let mut file = File::from(fd);
    let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    file.write_all(&bytes).map_err(|error| error.to_string())?;
    file.write_all(b"\n").map_err(|error| error.to_string())?;
    file.sync_data().map_err(|error| error.to_string())?;
    Ok(())
}

#[derive(Clone, Debug)]
struct XmlNode {
    namespace: Option<String>,
    local: String,
    attributes: BTreeMap<String, String>,
    direct_text: String,
    children: Vec<XmlNode>,
}
impl XmlNode {
    fn attr(&self, name: &str) -> Option<&str> {
        self.attributes.get(name).map(String::as_str)
    }
    fn is(&self, namespace: &str, local: &str) -> bool {
        self.namespace.as_deref() == Some(namespace) && self.local == local
    }
    fn descendants<'a>(&'a self, out: &mut Vec<&'a XmlNode>) {
        out.push(self);
        for child in &self.children {
            child.descendants(out);
        }
    }
    fn text_content(&self, out: &mut String) {
        out.push_str(&self.direct_text);
        for child in &self.children {
            child.text_content(out);
        }
    }
}

fn parse_xml(raw: &[u8]) -> Result<XmlNode> {
    let config = ParserConfig::new()
        .max_name_length(65536)
        .max_attributes(128)
        .max_attribute_length(65536)
        .max_data_length(16 * 1024 * 1024)
        .allow_multiple_root_elements(false)
        .ignore_end_of_stream(false)
        .replace_unknown_entity_references(false);
    let mut stack: Vec<XmlNode> = Vec::new();
    let mut root: Option<XmlNode> = None;
    let mut count = 0usize;
    for event in config.create_reader(raw) {
        count += 1;
        if count > 262_144 {
            return Err("XML event budget exceeded".into());
        }
        match event.map_err(|error| format!("invalid XML: {error}"))? {
            XmlEvent::StartElement {
                name, attributes, ..
            } => {
                if stack.len() >= 128 {
                    return Err("XML depth budget exceeded".into());
                }
                let mut attrs = BTreeMap::new();
                for attribute in attributes {
                    let key = match attribute.name.namespace {
                        Some(namespace) => format!("{{{namespace}}}{}", attribute.name.local_name),
                        None => attribute.name.local_name,
                    };
                    attrs.insert(key, attribute.value);
                }
                stack.push(XmlNode {
                    namespace: name.namespace,
                    local: name.local_name,
                    attributes: attrs,
                    direct_text: String::new(),
                    children: Vec::new(),
                });
            }
            XmlEvent::EndElement { .. } => {
                let node = stack.pop().ok_or("unbalanced XML end element")?;
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(node);
                } else if root.replace(node).is_some() {
                    return Err("XML has multiple document roots".into());
                }
            }
            XmlEvent::Characters(value) | XmlEvent::Whitespace(value) | XmlEvent::CData(value) => {
                if let Some(node) = stack.last_mut() {
                    node.direct_text.push_str(&value);
                }
            }
            XmlEvent::ProcessingInstruction { .. } | XmlEvent::Doctype { .. } => {
                return Err(
                    "XML declarations with executable or external content are refused".into(),
                );
            }
            XmlEvent::StartDocument { .. } | XmlEvent::EndDocument | XmlEvent::Comment(_) => (),
        }
    }
    if !stack.is_empty() {
        return Err("truncated XML document".into());
    }
    root.ok_or_else(|| "XML document root missing".into())
}

type TeiAddress = Vec<(String, Option<String>)>;
fn tei_division_addresses(
    edition: &XmlNode,
    milestone_unit: Option<&str>,
    collect_repeated_milestones: bool,
    collect_repeated_divisions: bool,
) -> Result<(Vec<String>, Vec<Value>, Vec<Value>)> {
    const TEI: &str = "http://www.tei-c.org/ns/1.0";
    let mut addresses = Vec::new();
    let mut milestone_counts = BTreeMap::<TeiAddress, usize>::new();
    let mut division_counts = BTreeMap::<TeiAddress, usize>::new();
    let mut seen = BTreeSet::<TeiAddress>::new();
    fn visit(
        node: &XmlNode,
        parents: &TeiAddress,
        milestone_unit: Option<&str>,
        collect_repeated_milestones: bool,
        collect_repeated_divisions: bool,
        addresses: &mut Vec<String>,
        milestone_counts: &mut BTreeMap<TeiAddress, usize>,
        division_counts: &mut BTreeMap<TeiAddress, usize>,
        seen: &mut BTreeSet<TeiAddress>,
    ) -> Result<()> {
        for child in &node.children {
            let mut address = parents.clone();
            if child.is(TEI, "div") {
                let label = child
                    .attr("subtype")
                    .or_else(|| child.attr("type"))
                    .unwrap_or("");
                let number = child.attr("n");
                if label.trim().is_empty() || number.is_some_and(|value| value.trim().is_empty()) {
                    return Err("Perseus text division lacks a supplied type or number".into());
                }
                if number.is_none() {
                    let mut descendants = Vec::new();
                    child.descendants(&mut descendants);
                    if !descendants.iter().skip(1).any(|descendant| {
                        descendant.is(TEI, "div")
                            && descendant.attr("n").is_some_and(|value| !value.is_empty())
                    }) {
                        return Err(
                            "Perseus unnumbered division is not a numbered-text container".into(),
                        );
                    }
                }
                address.push((label.to_owned(), number.map(str::to_owned)));
                if collect_repeated_divisions {
                    let count = division_counts.entry(address.clone()).or_default();
                    *count += 1;
                    address.push(("division_occurrence".into(), Some(count.to_string())));
                } else if !seen.insert(address.clone()) {
                    return Err("Perseus qualified division address is duplicated".into());
                }
                addresses.push(serde_json::to_string(&address).map_err(|error| error.to_string())?);
            }
            if let Some(unit) = milestone_unit {
                if child.is(TEI, "milestone") && child.attr("unit") == Some(unit) {
                    let number = child
                        .attr("n")
                        .filter(|value| !value.is_empty())
                        .ok_or("Perseus citation milestone lacks a supplied number")?;
                    let mut marker = parents.clone();
                    marker.push((unit.to_owned(), Some(number.to_owned())));
                    let mut physical = marker.clone();
                    let count = milestone_counts.entry(marker).or_default();
                    *count += 1;
                    physical.push(("marker_occurrence".into(), Some(count.to_string())));
                    addresses
                        .push(serde_json::to_string(&physical).map_err(|error| error.to_string())?);
                }
            }
            visit(
                child,
                &address,
                milestone_unit,
                collect_repeated_milestones,
                collect_repeated_divisions,
                addresses,
                milestone_counts,
                division_counts,
                seen,
            )?;
        }
        Ok(())
    }
    visit(
        edition,
        &TeiAddress::default(),
        milestone_unit,
        collect_repeated_milestones,
        collect_repeated_divisions,
        &mut addresses,
        &mut milestone_counts,
        &mut division_counts,
        &mut seen,
    )?;
    let repetitions = |counts: BTreeMap<TeiAddress, usize>| -> Result<Vec<Value>> {
        counts
            .into_iter()
            .filter(|(_, count)| *count > 1)
            .map(|(address, count)| Ok(json!({"source_address":address,"occurrences":count})))
            .collect()
    };
    Ok((
        addresses,
        if collect_repeated_milestones {
            repetitions(milestone_counts)?
        } else {
            Vec::new()
        },
        if collect_repeated_divisions {
            repetitions(division_counts)?
        } else {
            Vec::new()
        },
    ))
}

fn inspect_payloads(target: &Value, bodies: &[(Value, Vec<u8>)]) -> Result<Value> {
    let coverage = field(target, "coverage")?;
    let kind = text(coverage, "kind")?;
    let total_bytes: usize = bodies.iter().map(|(_, body)| body.len()).sum();
    let mut report = json!({
        "target_slug":text(target,"slug")?,"file_count":bodies.len(),"byte_size":total_bytes,
        "source_bytes_changed":false,"textual_acceptance":false,"files":[]
    });
    if matches!(kind, "bilara-root" | "bilara-translation")
        && (coverage.get("reviewed_body_prefix_sha256").is_some()
            || coverage.get("reviewed_body_prefix_bytes").is_some())
    {
        let length = coverage
            .get("reviewed_body_prefix_bytes")
            .and_then(Value::as_u64);
        if bodies.len() != 1
            || length.is_none_or(|length| length == 0 || length as usize > bodies[0].1.len())
            || sha256(&bodies[0].1[..length.unwrap_or(0) as usize])
                != coverage
                    .get("reviewed_body_prefix_sha256")
                    .and_then(Value::as_str)
                    .unwrap_or("")
        {
            return Err("Bilara source opening differs from the reviewed evidence".into());
        }
    }
    match kind {
        "perseus-tei-work" | "perseus-tei-translation" | "perseus-tei-latin-work" => {
            let translated = kind == "perseus-tei-translation";
            let latin = kind == "perseus-tei-latin-work";
            if latin
                && (target.get("language").and_then(Value::as_str) != Some("la")
                    || target
                        .get("expression_role")
                        .and_then(Value::as_str)
                        .unwrap_or("source_language")
                        != "source_language")
            {
                return Err(
                    "Latin edition profile requires its explicit language and source role".into(),
                );
            }
            if translated
                && (target.get("language").and_then(Value::as_str) != Some("en")
                    || target.get("expression_role").and_then(Value::as_str) != Some("translation"))
            {
                return Err(
                    "English translation profile requires its explicit language and role".into(),
                );
            }
            if bodies.len() != 1 {
                return Err("Perseus work requires exactly one prepared TEI file".into());
            }
            let (entry, body) = &bodies[0];
            if coverage.get("reviewed_body_prefix_sha256").is_some() {
                let length = coverage
                    .get("reviewed_body_prefix_bytes")
                    .and_then(Value::as_u64);
                if length.is_none_or(|length| length == 0 || length as usize > body.len())
                    || sha256(&body[..length.unwrap_or(0) as usize])
                        != text(coverage, "reviewed_body_prefix_sha256")?
                {
                    return Err("source opening differs from the reviewed language evidence".into());
                }
            }
            let marker = b"</teiHeader>";
            let position = body
                .windows(marker.len())
                .position(|window| window == marker)
                .ok_or("Perseus header differs from the reviewed metadata prefix")?;
            let mut prefix = body[..position].to_vec();
            prefix.extend_from_slice(marker);
            if sha256(&prefix) != text(coverage, "header_prefix_sha256")? {
                return Err("Perseus header differs from the reviewed metadata prefix".into());
            }
            let document = parse_xml(body)?;
            const TEI: &str = "http://www.tei-c.org/ns/1.0";
            const XML: &str = "http://www.w3.org/XML/1998/namespace";
            if !document.is(TEI, "TEI") {
                return Err("Perseus file lacks the expected TEI body".into());
            }
            let text_node = document
                .children
                .iter()
                .find(|node| node.is(TEI, "text"))
                .and_then(|node| node.children.iter().find(|node| node.is(TEI, "body")))
                .ok_or("Perseus file lacks the expected TEI body")?;
            let mut body_nodes = Vec::new();
            text_node.descendants(&mut body_nodes);
            let editions = body_nodes
                .into_iter()
                .filter(|node| {
                    node.is(TEI, "div")
                        && matches!(node.attr("type"), Some("edition" | "translation"))
                })
                .collect::<Vec<_>>();
            let expected_kind = if translated { "translation" } else { "edition" };
            let expected_identity = text(coverage, "cts_urn")?;
            let identity = if editions.len() == 1 {
                editions[0].attr("n")
            } else {
                None
            };
            let mut selected_identity = identity;
            if latin || translated && coverage.get("identity_anchor").is_some() {
                let anchor = text(coverage, "identity_anchor")?;
                let body_identity = text_node.attr("{http://www.w3.org/XML/1998/namespace}base");
                if !matches!(anchor, "edition_n" | "body_xml_base") {
                    return Err("Perseus identity anchor must name its reviewed XML carrier".into());
                }
                if identity.is_some_and(|value| value != expected_identity)
                    || body_identity.is_some_and(|value| value != expected_identity)
                {
                    return Err("Perseus source identity carriers conflict".into());
                }
                if anchor == "body_xml_base" {
                    selected_identity = body_identity;
                }
            }
            let language = editions
                .first()
                .and_then(|edition| edition.attr(&format!("{{{XML}}}lang")));
            let language_allowed = if translated {
                matches!(language, Some("en" | "eng"))
            } else if latin {
                matches!(language, Some("la" | "lat"))
            } else {
                language == Some("grc")
            };
            if editions.len() != 1
                || editions[0].attr("type") != Some(expected_kind)
                || selected_identity != Some(expected_identity)
                || !language_allowed
            {
                return Err("Perseus edition identity or source language differs".into());
            }
            let edition = editions[0];
            let mut nodes = Vec::new();
            edition.descendants(&mut nodes);
            let sections = nodes
                .iter()
                .filter(|node| node.is(TEI, "div") && node.attr("subtype") == Some("section"))
                .filter_map(|node| node.attr("n"))
                .collect::<Vec<_>>();
            let mut text_content = String::new();
            edition.text_content(&mut text_content);
            let count = if translated || latin {
                text_content
                    .chars()
                    .filter(|character| character.is_ascii_alphabetic())
                    .count()
            } else {
                text_content
                    .chars()
                    .filter(|character| {
                        ('\u{0370}'..='\u{03ff}').contains(character)
                            || ('\u{1f00}'..='\u{1fff}').contains(character)
                    })
                    .count()
            };
            let count_field = if translated || latin {
                "latin_letter_count"
            } else {
                "greek_character_count"
            };
            let scope = coverage
                .get("citation_scope")
                .and_then(Value::as_str)
                .unwrap_or("");
            let file_report = if matches!(
                scope,
                "hierarchical_divisions" | "hierarchical_divisions_and_section_milestones"
            ) {
                let milestones = scope == "hierarchical_divisions_and_section_milestones";
                let (addresses, repetitions, division_repetitions) = tei_division_addresses(
                    edition,
                    milestones.then_some("section"),
                    milestones,
                    milestones,
                )?;
                if addresses.is_empty() || count < 1000 {
                    return Err("Perseus qualified divisions or nonempty language-profile text check failed".into());
                }
                let address_digest = sha256((addresses.join("\n") + "\n").as_bytes());
                let address_scope = if milestones {
                    "occurrence-qualified source divisions and section markers; repeated numbering is retained, not corrected; no CTS service resolution asserted"
                } else {
                    "source-supplied division type/number chain; no CTS service resolution asserted"
                };
                let mut file_report = json!({"basename":text(entry,"basename")?,"cts_urn":expected_identity,"division_count":addresses.len(),"first_division":addresses[0],"last_division":addresses[addresses.len()-1],"division_addresses_sha256":address_digest,"address_scope":address_scope});
                file_report[count_field] = json!(count);
                let mut unnumbered = Vec::<Vec<(String, Option<String>)>>::new();
                for address in &addresses {
                    let path: TeiAddress = strict_json(address.as_bytes())?
                        .as_array()
                        .ok_or("invalid division address")?
                        .iter()
                        .map(|part| {
                            let pair = part.as_array().ok_or("invalid division address")?;
                            Ok((
                                pair.first()
                                    .and_then(Value::as_str)
                                    .ok_or("invalid division label")?
                                    .to_owned(),
                                pair.get(1)
                                    .filter(|value| !value.is_null())
                                    .and_then(Value::as_str)
                                    .map(str::to_owned),
                            ))
                        })
                        .collect::<Result<Vec<_>>>()?;
                    for index in 0..path.len() {
                        if path[index].1.is_none() {
                            let prefix = path[..=index].to_vec();
                            if !unnumbered.contains(&prefix) {
                                unnumbered.push(prefix);
                            }
                        }
                    }
                }
                if !unnumbered.is_empty() {
                    file_report["unnumbered_containers"] = json!(unnumbered);
                    file_report["address_scope"] = json!(format!(
                        "{address_scope}; null numbers mark supplied typed containers without n; not supplied numeric citations"
                    ));
                }
                if milestones {
                    file_report["repeated_section_markers"] = json!(repetitions);
                    file_report["repeated_division_labels"] = json!(division_repetitions);
                }
                file_report
            } else {
                let unique = sections.iter().collect::<BTreeSet<_>>();
                if sections.is_empty() || unique.len() != sections.len() || count < 1000 {
                    return Err(
                        "Perseus section identity or nonempty language-profile text check failed"
                            .into(),
                    );
                }
                let mut file_report = json!({"basename":text(entry,"basename")?,"cts_urn":expected_identity,"section_count":sections.len(),"first_section":sections[0],"last_section":sections[sections.len()-1]});
                file_report[count_field] = json!(count);
                file_report
            };
            report["files"]
                .as_array_mut()
                .ok_or("report files missing")?
                .push(file_report);
            report["coverage_limit"] = json!(
                "Complete pinned supplied file; no independent critical-edition or missing-passage judgment."
            );
        }
        "osis-book" => {
            if bodies.is_empty() {
                return Err("OSIS source book/chapter/nonempty-word coverage mismatch".into());
            }
            const OSIS: &str = "http://www.bibletechnologies.net/2003/OSIS/namespace";
            let (entry, body) = &bodies[0];
            let document = parse_xml(body)?;
            if !document.is(OSIS, "osis") {
                return Err("source is not the expected OSIS XML".into());
            }
            let mut nodes = Vec::new();
            document.descendants(&mut nodes);
            let chapters = nodes
                .iter()
                .filter(|node| node.is(OSIS, "chapter"))
                .filter_map(|node| node.attr("osisID"))
                .map(str::to_owned)
                .collect::<Vec<_>>();
            let book = text(coverage, "book")?;
            let chapter_count = u64_field(coverage, "chapter_count")?;
            let expected = (1..=chapter_count)
                .map(|number| format!("{book}.{number}"))
                .collect::<Vec<_>>();
            let verses = nodes
                .iter()
                .filter(|node| node.is(OSIS, "verse"))
                .filter_map(|node| node.attr("osisID"))
                .map(str::to_owned)
                .collect::<Vec<_>>();
            let words = nodes
                .iter()
                .filter(|node| node.is(OSIS, "w"))
                .collect::<Vec<_>>();
            if chapters != expected
                || verses.is_empty()
                || words.is_empty()
                || !words.iter().any(|node| !node.direct_text.trim().is_empty())
            {
                return Err("OSIS source book/chapter/nonempty-word coverage mismatch".into());
            }
            if verses
                .iter()
                .any(|address| !address.starts_with(&format!("{book}.")))
                || verses.iter().collect::<BTreeSet<_>>().len() != verses.len()
            {
                return Err("OSIS verse identity is duplicated or belongs to another book".into());
            }
            report["files"].as_array_mut().ok_or("report files missing")?.push(json!({"basename":text(entry,"basename")?,"chapter_ids":chapters,"verse_count":verses.len(),"word_count":words.len(),"first_verse":verses[0],"last_verse":verses[verses.len()-1]}));
        }
        "bilara-translation" => inspect_bilara_translation(target, coverage, bodies, &mut report)?,
        "bilara-root" => inspect_bilara_root(coverage, bodies, &mut report)?,
        "oraec-composition" => inspect_oraec(target, bodies, &mut report)?,
        _ => return Err("unknown prepared coverage control".into()),
    }
    Ok(report)
}

fn inspect_bilara_translation(
    target: &Value,
    coverage: &Value,
    bodies: &[(Value, Vec<u8>)],
    report: &mut Value,
) -> Result<()> {
    if target.get("language").and_then(Value::as_str) != Some("en")
        || target.get("expression_role").and_then(Value::as_str) != Some("translation")
    {
        return Err("Bilara English translation requires its explicit language and role".into());
    }
    if u64_field(coverage, "file_count")? != 1 || bodies.len() != 1 {
        return Err("Bilara translation requires exactly one prepared file".into());
    }
    let (entry, body) = &bodies[0];
    let uid = text(coverage, "uid")?;
    if uid.is_empty()
        || uid.contains(':')
        || text(entry, "basename")? != format!("{uid}_translation-en-sujato.json")
    {
        return Err("Bilara translation filename or supplied UID differs".into());
    }
    let parsed = strict_json(body)?;
    let mapping = parsed
        .as_object()
        .filter(|object| !object.is_empty())
        .ok_or("Bilara translation is not a nonempty segment-to-string mapping")?;
    if mapping.values().any(|value| !value.is_string()) {
        return Err("Bilara translation is not a nonempty segment-to-string mapping".into());
    }
    if !mapping
        .values()
        .filter_map(Value::as_str)
        .any(|value| !value.trim().is_empty())
        || mapping
            .keys()
            .any(|key| !key.starts_with(&format!("{uid}:")) || key.len() == uid.len() + 1)
    {
        return Err("Bilara translation segment identity or nonempty-text check failed".into());
    }
    let count = mapping.len();
    report["files"].as_array_mut().ok_or("report files missing")?.push(json!({"basename":text(entry,"basename")?,"uid":uid,"segment_count":count,"nonempty_segment_count":mapping.values().filter_map(Value::as_str).filter(|value| !value.trim().is_empty()).count()}));
    report["unique_segment_count"] = json!(count);
    report["coverage_limit"] = json!(
        "Exact supplied translation file and segment identifiers; no segment alignment or translation-quality assessment."
    );
    Ok(())
}

fn inspect_bilara_root(
    coverage: &Value,
    bodies: &[(Value, Vec<u8>)],
    report: &mut Value,
) -> Result<()> {
    let mut all_segments = BTreeSet::<String>::new();
    let mut uids = Vec::new();
    for (entry, body) in bodies {
        let parsed = strict_json(body)?;
        let mapping = parsed
            .as_object()
            .filter(|object| !object.is_empty())
            .ok_or("Bilara root is not a nonempty segment-to-string mapping")?;
        if mapping.values().any(|value| !value.is_string()) {
            return Err("Bilara root is not a nonempty segment-to-string mapping".into());
        }
        let basename = text(entry, "basename")?;
        let suffix = "_root-pli-ms.json";
        let uid = basename.strip_suffix(suffix).unwrap_or(basename).to_owned();
        let mut valid_uids = BTreeSet::from([uid.clone()]);
        if let Some((start, end)) = parse_dhp_range(&uid) {
            for value in start..=end {
                valid_uids.insert(format!("dhp{value}"));
            }
        }
        if !mapping
            .values()
            .filter_map(Value::as_str)
            .any(|value| !value.trim().is_empty())
            || mapping.keys().any(|key| {
                !key.contains(':')
                    || !valid_uids.contains(key.split_once(':').map(|pair| pair.0).unwrap_or(""))
            })
        {
            return Err("Bilara root segment identity or nonempty-text check failed".into());
        }
        if mapping.keys().any(|key| !all_segments.insert(key.clone())) {
            return Err("Bilara segment is duplicated across files".into());
        }
        report["files"].as_array_mut().ok_or("report files missing")?.push(json!({"basename":basename,"uid":uid,"segment_count":mapping.len(),"nonempty_segment_count":mapping.values().filter_map(Value::as_str).filter(|value| !value.trim().is_empty()).count()}));
        uids.push(uid);
    }
    if bodies.len() as u64 != u64_field(coverage, "file_count")? {
        return Err("Bilara exact file coverage failed".into());
    }
    let primary = text(coverage, "uid")?;
    if primary == "kp" || primary == "iti" {
        let expected = (1..=bodies.len())
            .map(|number| format!("{primary}{number}"))
            .collect::<BTreeSet<_>>();
        if uids.iter().cloned().collect::<BTreeSet<_>>() != expected {
            return Err("Bilara consecutive work-unit coverage failed".into());
        }
    } else if primary == "dhp" {
        let mut ranges = Vec::new();
        for uid in &uids {
            let (start, end) =
                parse_dhp_range(uid).ok_or("unrecognized Dhammapada range filename")?;
            ranges.extend(start..=end);
        }
        ranges.sort_unstable();
        if ranges != (1..=423).collect::<Vec<_>>() {
            return Err("Dhammapada file-range coverage is not exactly 1–423".into());
        }
    } else if primary == "ud" {
        let expected = (1..=8)
            .flat_map(|vagga| (1..=10).map(move |number| format!("ud{vagga}.{number}")))
            .collect::<BTreeSet<_>>();
        if uids.iter().cloned().collect::<BTreeSet<_>>() != expected {
            return Err("Udāna file coverage is not exactly 8 × 10".into());
        }
    } else if primary == "snp" {
        let expected = [12usize, 14, 12, 16, 19];
        let counts = (1..=5)
            .map(|vagga| {
                uids.iter()
                    .filter(|uid| uid.starts_with(&format!("snp{vagga}.")))
                    .count()
            })
            .collect::<Vec<_>>();
        if counts != expected {
            return Err("Suttanipāta five-vagga supplied file coverage differs".into());
        }
    } else if uids != [primary] {
        return Err("single-sutta identity differs".into());
    }
    report["unique_segment_count"] = json!(all_segments.len());
    Ok(())
}

fn parse_dhp_range(uid: &str) -> Option<(usize, usize)> {
    let digits = uid.strip_prefix("dhp")?;
    let (start, end) = digits.split_once('-')?;
    if start.is_empty()
        || end.is_empty()
        || !start.bytes().all(|byte| byte.is_ascii_digit())
        || !end.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let start = start.parse().ok()?;
    let end = end.parse().ok()?;
    (start <= end).then_some((start, end))
}

fn inspect_oraec(target: &Value, bodies: &[(Value, Vec<u8>)], report: &mut Value) -> Result<()> {
    if bodies.is_empty() {
        return Err("ORAEC JSON bundle is empty or not structured".into());
    }
    let (entry, body) = &bodies[0];
    let parsed = strict_json(body)?;
    let root_member_count = parsed
        .as_object()
        .map(Map::len)
        .or_else(|| parsed.as_array().map(Vec::len))
        .filter(|count| *count > 0)
        .ok_or("ORAEC JSON bundle is empty or not structured")?;
    let mut components = BTreeMap::<&str, BTreeMap<String, usize>>::from([
        ("egy", BTreeMap::new()),
        ("de", BTreeMap::new()),
    ]);
    const EGY: &[&str] = &[
        "writtenform",
        "transliteration",
        "transcription",
        "hiero",
        "hieroglyphs",
        "hieroglyphic",
    ];
    const DE: &[&str] = &[
        "translation",
        "germantranslation",
        "translationde",
        "gloss",
        "wordtranslation",
        "cotexttranslation",
    ];
    fn pattern(pointer: &str) -> String {
        pointer
            .split('/')
            .map(|segment| {
                if !segment.is_empty() && segment.bytes().all(|byte| byte.is_ascii_digit()) {
                    "*"
                } else {
                    segment
                }
            })
            .collect::<Vec<_>>()
            .join("/")
    }
    fn gather_strings(value: &Value, pointer: &str, counts: &mut BTreeMap<String, usize>) {
        match value {
            Value::String(text) if !text.trim().is_empty() => {
                *counts.entry(pattern(pointer)).or_default() += 1
            }
            Value::Array(items) => {
                for (index, item) in items.iter().enumerate() {
                    gather_strings(item, &format!("{pointer}/{index}"), counts);
                }
            }
            Value::Object(items) => {
                for (key, item) in items {
                    gather_strings(
                        item,
                        &format!("{pointer}/{}", key.replace('~', "~0").replace('/', "~1")),
                        counts,
                    );
                }
            }
            _ => (),
        }
    }
    fn visit(
        value: &Value,
        pointer: &str,
        components: &mut BTreeMap<&str, BTreeMap<String, usize>>,
    ) {
        match value {
            Value::Object(items) => {
                for (key, child) in items {
                    let at = format!("{pointer}/{}", key.replace('~', "~0").replace('/', "~1"));
                    let normalized = key
                        .to_ascii_lowercase()
                        .chars()
                        .filter(|character| character.is_ascii_lowercase())
                        .collect::<String>();
                    let language = if EGY.contains(&normalized.as_str()) {
                        Some("egy")
                    } else if DE.contains(&normalized.as_str()) {
                        Some("de")
                    } else {
                        None
                    };
                    if let Some(language) = language {
                        gather_strings(
                            child,
                            &at,
                            components.get_mut(language).expect("declared component"),
                        );
                    }
                    visit(child, &at, components);
                }
            }
            Value::Array(items) => {
                for (index, child) in items.iter().enumerate() {
                    visit(child, &format!("{pointer}/{index}"), components);
                }
            }
            _ => (),
        }
    }
    visit(&parsed, "", &mut components);
    if components.values().any(BTreeMap::is_empty) {
        return Err("ORAEC bundle requires observed Egyptian written-form and separate German translation/gloss fields; inspect format before source installation".into());
    }
    report["files"].as_array_mut().ok_or("report files missing")?.push(json!({"basename":text(entry,"basename")?,"root_type":if parsed.is_object(){"dict"}else{"list"},"root_member_count":root_member_count}));
    let ids = field(target, "ids")?;
    let file_digest = sha256(body);
    let mut witnesses = Vec::new();
    for language in ["egy", "de"] {
        let id_key = if language == "egy" {
            "expression"
        } else {
            "translation_expression"
        };
        let selectors = components.get(language).ok_or("ORAEC component profile missing")?.iter().map(|(pointer, count)| json!({"json_pointer_pattern":pointer,"nonempty_string_count":count})).collect::<Vec<_>>();
        witnesses.push(json!({"expression_ref":text(ids,id_key)?,"language":language,"file_id":format!("tos.file.sha256.{file_digest}"),"file_sha256":file_digest,"selectors":selectors,"selection_posture":"Observation of supplied fields, retaining their current source and assessment status."}));
    }
    report["component_witnesses"] = Value::Array(witnesses);
    Ok(())
}

fn run_command(request: &Value) -> Result<Value> {
    let root = absolute_path(request, "root")?;
    tos_fd_open::open_absolute_directory(&root).map_err(|error| error.to_string())?;
    let requested_manifest = absolute_path(request, "manifest_path")?;
    let manifest_ref = requested_manifest
        .strip_prefix(&root)
        .map_err(|_| "registry manifest must be inside the repository root")?
        .to_string_lossy()
        .replace('\\', "/");
    custody::safe_ref(&manifest_ref)?;
    let manifest_path = path_ref(&root, &manifest_ref)?;
    let operation = operation(request)?;
    let allow_unbound = optional_bool(request, "allow_unbound_registry", false)?
        && operation == "registry.verify_preparation";
    let payload_source_root = optional_absolute_path(request, "payload_source_root")?;
    if operation == "registry.acquire" && payload_source_root.is_none() {
        return Err("acquire requires an explicit --payload-source-root; refusing to write inside the metadata checkout".into());
    }
    if let Some(payload_root) = payload_source_root.as_deref() {
        custody::private_root(payload_root)
            .map_err(|_| "payload source root must be an owner-only (0700) existing directory")?;
    }
    let requested = optional_string_array(request, "target_slugs")?;
    let fetch_callback = optional_bool(request, "fetch_callback", false)?;
    let (preparation, packages) = load_preparation(&root, &manifest_path, allow_unbound)?;
    let targets = arr(&preparation, "targets")?
        .iter()
        .filter(|target| {
            requested.is_empty()
                || target
                    .get("slug")
                    .and_then(Value::as_str)
                    .is_some_and(|slug| requested.iter().any(|selected| selected.as_str() == slug))
        })
        .cloned()
        .collect::<Vec<_>>();
    if !requested.is_empty()
        && targets
            .iter()
            .filter_map(|target| target.get("slug").and_then(Value::as_str))
            .collect::<BTreeSet<_>>()
            != requested
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>()
    {
        return Err("unknown requested acquisition target".into());
    }
    if operation == "registry.verify_preparation" {
        let manifest_bytes = custody::read_bytes(&manifest_path, None, false, false, MAX_JSON)?;
        return Ok(
            json!({"status":"preparation-verified","manifest_sha256":sha256(&manifest_bytes),"totals":preparation["totals"],"registry_bound":preparation.get("source_registry_snapshot_ref").is_some_and(|value|!value.is_null()),"payloads_downloaded":false}),
        );
    }
    if operation == "registry.acquire" {
        let receipt_path = absolute_path(request, "preparation_receipt_path")
            .map_err(|_| "acquire requires the completed preparation checkpoint receipt")?;
        check_preparation_receipt(&root, &manifest_path, &receipt_path)?;
        preflight_identities(&root, &targets, &packages)?;
    }
    let mut results = Vec::new();
    for target in targets {
        let result = if operation == "registry.acquire" {
            install_target(
                &root,
                &manifest_path,
                &preparation,
                &target,
                packages
                    .get(text(&target, "slug")?)
                    .ok_or("prepared source package missing")?,
                payload_source_root.as_deref(),
                fetch_callback,
            )?
        } else {
            verify_target(&root, &target, payload_source_root.as_deref())?
        };
        results.push(result);
    }
    Ok(json!({"results":results}))
}

fn manifest_fixity_value(manifest: &Value) -> Result<String> {
    manifest_fixity(manifest)
}

fn verify_target(root: &Path, target: &Value, payload_source_root: Option<&Path>) -> Result<Value> {
    let paths = field(target, "paths")?;
    let item_root_ref = text(paths, "item_root")?;
    let item_root = path_ref(root, item_root_ref)?;
    let manifest_ref = format!("{item_root_ref}/item.manifest.json");
    let manifest = json_ref(root, &manifest_ref)?;
    validate_json(root, &manifest, "source-item-manifest")?;
    if manifest.get("item_id") != field(target, "ids")?.get("item")
        || arr(&manifest, "payload_files")?.len() != arr(target, "files")?.len()
    {
        return Err("installed Item manifest identity differs".into());
    }
    let mut indexed = BTreeMap::<String, &Value>::new();
    for entry in arr(&manifest, "payload_files")? {
        let original = text(entry, "original_basename")?.to_owned();
        if indexed.insert(original, entry).is_some() {
            return Err("installed Item manifest repeats a source basename".into());
        }
    }
    let default_payload_root = root.join(SOURCE);
    let payload_root = payload_source_root.unwrap_or(&default_payload_root);
    if let Some(payload_source_root) = payload_source_root {
        custody::private_root(payload_source_root)?;
    }
    let mut bodies = Vec::new();
    let mut total = 0usize;
    for expected in arr(target, "files")? {
        let basename = text(expected, "basename")?;
        let payload_path =
            payload_destination(payload_root, item_root_ref, &format!("payload/{basename}"))?;
        let body = custody::read_bytes(&payload_path, Some(0o444), true, true, 300 * 1024 * 1024)?;
        let digest = check_file(&body, expected)?;
        let entry = indexed
            .get(basename)
            .ok_or("installed File identity differs")?;
        if text(entry, "sha256")? != digest
            || text(entry, "file_id")? != format!("tos.file.sha256.{digest}")
            || u64_field(entry, "byte_size")? != body.len() as u64
            || text(entry, "relative_path")? != format!("payload/{basename}")
        {
            return Err("installed File identity differs".into());
        }
        total += body.len();
        bodies.push((expected.clone(), body));
    }
    if let Some(payload_source_root) = payload_source_root {
        custody::private_root(payload_source_root)?;
    }
    inspect_payloads(target, &bodies)?;
    for key in [
        "rights_ref",
        "provenance_ref",
        "forensic_report_ref",
        "resource_inventory_ref",
    ] {
        let companion = path_ref(root, text(&manifest, key)?)?;
        if !companion.is_file() {
            return Err(format!("installed Item companion missing: {key}"));
        }
    }
    let fixity_ref = format!("{item_root_ref}/fixity.sha256");
    let fixity = path_ref(root, &fixity_ref)?;
    let actual_fixity = custody::read_bytes(&fixity, None, false, false, MAX_JSON)?;
    let expected_fixity = manifest_fixity_value(&manifest)?;
    if actual_fixity != expected_fixity.as_bytes() {
        return Err("installed fixity companion differs from the exact manifest".into());
    }
    let rights = json_ref(root, text(&manifest, "rights_ref")?)?;
    validate_json(root, &rights, "rights-record")?;
    let scopes = arr(&rights, "scope_refs")?
        .iter()
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    if !scopes.contains(text(&manifest, "item_id")?)
        || arr(&manifest, "payload_files")?.iter().any(|entry| {
            entry
                .get("file_id")
                .and_then(Value::as_str)
                .is_none_or(|id| !scopes.contains(id))
        })
    {
        return Err("installed rights scope does not cover the Item and every File".into());
    }
    for key in object(target, "ids")?.keys() {
        let record_ref = text(field(target, "paths")?, key)?;
        if !path_ref(root, record_ref)?.is_file() {
            return Err("installed corpus identity record missing".into());
        }
    }
    let _ = item_root;
    Ok(
        json!({"target_slug":text(target,"slug")?,"status":"local-payload-and-owner-package-verified","item_id":manifest["item_id"],"item_manifest_ref":manifest_ref,"files":bodies.len(),"bytes":total,"textual_acceptance":false}),
    )
}

fn utcnow() -> Result<String> {
    crate::source_serialization::instant()
        .map_err(|error| format!("native clock unavailable: {error:?}"))
}

fn git_ok(root: &Path, args: &[&str]) -> bool {
    let cancel = AtomicBool::new(false);
    crate::source_text_owner_ocr::bounded_process(
        "git",
        args,
        Some(root),
        256,
        Instant::now() + Duration::from_secs(10),
        &cancel,
    )
    .is_ok()
}

fn transfer(
    root: &Path,
    target: &Value,
    entry: &Value,
    log: &Path,
    payload_source_root: Option<&Path>,
    fetch_callback: bool,
) -> Result<(Vec<u8>, Value)> {
    let default_payload_root = root.join(SOURCE);
    let payload_root = payload_source_root.unwrap_or(&default_payload_root);
    if let Some(payload_source_root) = payload_source_root {
        custody::private_root(payload_source_root)?;
    }
    let item_root = text(field(target, "paths")?, "item_root")?;
    let basename = text(entry, "basename")?;
    let relative = format!("{item_root}/payload/{basename}");
    let destination = payload_destination(payload_root, item_root, &format!("payload/{basename}"))?;
    if !git_ok(root, &["check-ignore", "--quiet", "--", &relative])
        || git_ok(root, &["ls-files", "--error-unmatch", "--", &relative])
    {
        return Err("source bytes must use an untracked, ignored Item payload path".into());
    }
    match fs::symlink_metadata(&destination) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err("existing destination payload is a symlink or special file".into());
            }
            let body = custody::read_bytes(&destination, Some(0o444), true, true, 8_000_001)?;
            if let Some(payload_source_root) = payload_source_root {
                custody::private_root(payload_source_root)?;
            }
            let digest = check_file(&body, entry)?;
            let previous = if log.exists() {
                let raw = custody::read_bytes(log, None, false, false, MAX_JSON)?;
                std::str::from_utf8(&raw)
                    .map_err(|_| "acquisition transfer log is not UTF-8")?
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .map(|line| strict_json(line.as_bytes()))
                    .collect::<Result<Vec<_>>>()?
            } else {
                Vec::new()
            };
            let matching = previous
                .iter()
                .filter(|row| {
                    row.get("status").and_then(Value::as_str) == Some("completed")
                        && row.get("destination_ref").and_then(Value::as_str) == Some(&relative)
                        && row.get("sha256").and_then(Value::as_str) == Some(&digest)
                })
                .last()
                .cloned();
            let receipt = matching
                .ok_or("existing matching payload lacks this operation's acquisition receipt")?;
            return Ok((body, receipt));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
        Err(error) => return Err(error.to_string()),
    }
    if let Some(parent) = log.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let started = utcnow()?;
    let tick = Instant::now();
    let mut row = json!({"target_slug":text(target,"slug")?,"url":text(entry,"url")?,"destination_ref":relative,"started_at":started,"expected_byte_size":u64_field(entry,"byte_size")?,"expected_git_blob_sha1":text(entry,"git_blob_sha1")?});
    append_jsonl(
        log,
        &json!({"target_slug":row["target_slug"],"url":row["url"],"destination_ref":row["destination_ref"],"started_at":row["started_at"],"expected_byte_size":row["expected_byte_size"],"expected_git_blob_sha1":row["expected_git_blob_sha1"],"status":"started"}),
    )?;
    let transfer_result = (|| -> Result<(Vec<u8>, u16, String)> {
        let url = text(entry, "url")?;
        let size = u64_field(entry, "byte_size")?;
        let body = if fetch_callback {
            let payload = json!({"url":url,"target_slug":text(target,"slug")?,"expected_byte_size":size,"expected_git_blob_sha1":text(entry,"git_blob_sha1")?});
            fetch_source_bytes(&payload)?
        } else {
            let metadata = "\n__TOS_HTTP_META__%{http_code}\n%{url_effective}";
            let args = vec![
                "-q",
                "--fail",
                "--silent",
                "--show-error",
                "--location",
                "--max-redirs",
                "10",
                "--max-time",
                "45",
                "--proto",
                "=https",
                "--proto-redir",
                "=https",
                "--user-agent",
                "Tree-of-Sophia-source-acquisition",
                "--write-out",
                metadata,
                url,
            ];
            let cancel = AtomicBool::new(false);
            let cap = usize::try_from(size)
                .map_err(|_| "source payload size overflow")?
                .checked_add(4096)
                .ok_or("source payload cap overflow")?;
            let raw = crate::source_text_owner_ocr::bounded_process(
                "/usr/bin/curl",
                &args,
                None,
                cap,
                Instant::now() + Duration::from_secs(50),
                &cancel,
            )
            .map_err(|error| format!("source transfer failed: {error:?}"))?;
            let marker = b"\n__TOS_HTTP_META__";
            let position = raw
                .windows(marker.len())
                .rposition(|window| window == marker)
                .ok_or("source transfer response metadata missing")?;
            let body = raw[..position].to_vec();
            let metadata = std::str::from_utf8(&raw[position + marker.len()..])
                .map_err(|_| "source transfer response metadata is not UTF-8")?;
            let (status, final_url) = metadata
                .split_once('\n')
                .ok_or("source transfer response metadata is incomplete")?;
            let status = status
                .parse::<u16>()
                .map_err(|_| "source transfer HTTP status is invalid")?;
            if status != 200 || url_host(final_url) != Some("raw.githubusercontent.com") {
                return Err(
                    "source transfer response was not a successful pinned raw-provider response"
                        .into(),
                );
            }
            return Ok((body, status, final_url.to_owned()));
        };
        Ok((body, 200, url.to_owned()))
    })();
    match transfer_result {
        Ok((body, status, final_url)) => {
            let digest = match check_file(&body, entry) {
                Ok(digest) => digest,
                Err(error) => {
                    let ended = utcnow()?;
                    let elapsed = tick.elapsed().as_secs_f64();
                    append_jsonl(
                        log,
                        &json!({"target_slug":row["target_slug"],"url":row["url"],"destination_ref":row["destination_ref"],"started_at":row["started_at"],"expected_byte_size":row["expected_byte_size"],"expected_git_blob_sha1":row["expected_git_blob_sha1"],"status":"failed","ended_at":ended,"elapsed_seconds":elapsed,"error_type":"ValueError","error":error}),
                    )?;
                    return Err(error);
                }
            };
            match custody::publish(&destination, &body, 0o444)? {
                "conflict" => {
                    return Err(
                        "destination payload appeared with different bytes during acquisition"
                            .into(),
                    );
                }
                _ => (),
            }
            if let Some(payload_source_root) = payload_source_root {
                custody::private_root(payload_source_root)?;
            }
            let ended = utcnow()?;
            let elapsed = tick.elapsed().as_secs_f64();
            row["http_status"] = json!(status);
            row["final_url"] = json!(final_url);
            row["status"] = json!("completed");
            row["ended_at"] = json!(ended);
            row["elapsed_seconds"] = json!(elapsed);
            row["byte_size"] = json!(body.len());
            row["sha256"] = json!(digest);
            append_jsonl(log, &row)?;
            Ok((body, row))
        }
        Err(error) => {
            let ended = utcnow()?;
            let elapsed = tick.elapsed().as_secs_f64();
            append_jsonl(
                log,
                &json!({"target_slug":row["target_slug"],"url":row["url"],"destination_ref":row["destination_ref"],"started_at":row["started_at"],"expected_byte_size":row["expected_byte_size"],"expected_git_blob_sha1":row["expected_git_blob_sha1"],"status":"failed","ended_at":ended,"elapsed_seconds":elapsed,"error_type":"ValueError","error":error}),
            )?;
            Err(error)
        }
    }
}

fn url_host(url: &str) -> Option<&str> {
    let rest = url.strip_prefix("https://")?;
    let host = rest.split('/').next()?;
    if host.contains('@') || host.contains(':') || host.is_empty() {
        None
    } else {
        Some(host)
    }
}

fn replace_bytes_guarded(path: &Path, before: &[u8], after: &[u8], mode: u32) -> Result<()> {
    if custody::read_bytes(path, None, false, false, MAX_JSON)? != before {
        return Err("source file changed before guarded replacement".into());
    }
    let parent = path.parent().ok_or("replacement parent missing")?;
    let directory =
        tos_fd_open::open_absolute_directory(parent).map_err(|error| error.to_string())?;
    let leaf = path.file_name().ok_or("replacement file name missing")?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "native clock predates Unix epoch")?
        .as_nanos();
    let temporary = format!(
        ".{}.{}.{}.tmp",
        leaf.to_string_lossy(),
        std::process::id(),
        nonce
    );
    let descriptor = rustix::fs::openat(
        &directory,
        temporary.as_str(),
        rustix::fs::OFlags::WRONLY
            | rustix::fs::OFlags::CREATE
            | rustix::fs::OFlags::EXCL
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::from_raw_mode(0o600),
    )
    .map_err(|error| format!("guarded replacement temporary open failed: {error}"))?;
    let mut file = File::from(descriptor);
    let result = (|| {
        file.write_all(after).map_err(|error| error.to_string())?;
        rustix::fs::fchmod(&file, rustix::fs::Mode::from_raw_mode(mode))
            .map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        if custody::read_bytes(path, None, false, false, MAX_JSON)? != before {
            return Err("source file changed during guarded replacement".into());
        }
        rustix::fs::renameat(&directory, temporary.as_str(), &directory, leaf)
            .map_err(|error| format!("guarded replacement publish failed: {error}"))?;
        directory.sync_all().map_err(|error| error.to_string())?;
        if custody::read_bytes(path, Some(mode), false, false, MAX_JSON)? != after {
            return Err("guarded replacement readback differs".into());
        }
        Ok(())
    })();
    let _ = rustix::fs::unlinkat(&directory, temporary.as_str(), rustix::fs::AtFlags::empty());
    result
}

fn refresh_topology(root: &Path, evidence_root: &Path, ended: &str) -> Result<()> {
    let path = path_ref(root, TOPOLOGY)?;
    let before = custody::read_bytes(&path, None, false, false, MAX_JSON)?;
    let history_ref = format!("topology-before/{}.json", sha256(&before));
    let history = path_ref(evidence_root, &history_ref)?;
    if !history.exists() {
        custody::publish(&history, &before, 0o644)?;
    }
    let mut batch = strict_json(&before)?;
    if batch.get("event_id").and_then(Value::as_str) != Some(TOPOLOGY_EVENT) {
        return Err("unexpected bibliographic topology owner event".into());
    }
    let mapping = [
        (
            "work-expression/work-expression-claims.jsonl",
            "work_expression_claims_materialized",
            "unreviewed-work-expression-topology-claims",
        ),
        (
            "expression-edition/expression-edition-claims.jsonl",
            "expression_edition_claims_materialized",
            "unreviewed-expression-edition-topology-claims",
        ),
        (
            "edition-item/edition-item-claims.jsonl",
            "edition_item_claims_materialized",
            "unreviewed-edition-item-topology-claims",
        ),
    ];
    let mut required_inputs = BTreeMap::<String, Value>::new();
    let mut outputs = Vec::new();
    for (relative, count_field, role) in mapping {
        let reference = format!("{SOURCE}/relations/{relative}");
        let data = read_ref(root, &reference)?;
        let body = std::str::from_utf8(&data).map_err(|_| "topology claim stream is not UTF-8")?;
        let claims = body
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| strict_json(line.as_bytes()))
            .collect::<Result<Vec<_>>>()?;
        batch["method"]["configuration"][count_field] = json!(claims.len());
        outputs.push(json!({"ref":reference,"role":role,"sha256":sha256(&data)}));
        for claim in claims {
            for evidence_ref_value in arr(&claim, "evidence_refs")? {
                let evidence_ref = evidence_ref_value
                    .as_str()
                    .ok_or("topology evidence reference must be text")?;
                let evidence_path = path_ref(root, evidence_ref)?;
                let kind = if evidence_path.file_name().and_then(|name| name.to_str())
                    == Some("item.manifest.json")
                {
                    "item-embodiment-manifest".to_owned()
                } else {
                    let stem = evidence_path
                        .file_stem()
                        .and_then(|name| name.to_str())
                        .ok_or("topology evidence stem missing")?;
                    format!("{stem}-topology-record")
                };
                let evidence_raw =
                    custody::read_bytes(&evidence_path, None, false, false, MAX_JSON)?;
                required_inputs.insert(evidence_ref.to_owned(), json!({"ref":evidence_ref,"role":format!("declared-{kind}"),"sha256":sha256(&evidence_raw)}));
            }
        }
    }
    batch["inputs"] = Value::Array(required_inputs.into_values().collect());
    batch["outputs"] = Value::Array(outputs);
    batch["ended_at"] = json!(ended);
    batch["event_version"] = json!(
        u64_field(&batch, "event_version")?
            .checked_add(1)
            .ok_or("topology event version overflow")?
    );
    let mut after = serde_json::to_vec(&batch).map_err(|error| error.to_string())?;
    after.push(b'\n');
    replace_bytes_guarded(&path, &before, &after, 0o644)
}

fn event(
    event_id: &str,
    event_type: &str,
    started: &str,
    ended: &str,
    inputs: Vec<Value>,
    outputs: Vec<Value>,
    name: &str,
    configuration: Value,
    rights_ref: &str,
    receipts: Vec<String>,
) -> Value {
    json!({
        "schema_version":"tos_provenance_event_v1","event_id":event_id,"event_type":event_type,
        "started_at":started,"ended_at":ended,"agent_refs":["model:codex","software:acquire-registry-sources"],
        "inputs":inputs,"outputs":outputs,
        "method":{"maker_type":"mixed","name":name,"version":"1","artifact_digest":sha256(include_bytes!("source_registry_acquisition.rs")),"runtime":"rust-native","configuration":configuration},
        "status":"completed_with_warnings","warnings":["Exact local custody and mechanical observations, retaining the source records and their current assessment, rights and publication status."],
        "receipt_refs":receipts,"rights_basis_ref":rights_ref,"event_version":1,"supersedes_event_ref":null
    })
}

fn selected_name(reference: &str) -> &str {
    Path::new(reference)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(reference)
}

fn write_discovery(
    root: &Path,
    manifest_path: &Path,
    preparation: &Value,
    target: &Value,
    transfers: &[Value],
    acquisition: &Value,
) -> Result<()> {
    let slug = text(target, "slug")?;
    let day = operation_date(target)?;
    let run_ref = format!("{SOURCE}/discovery/runs/registry-{slug}.{day}.v1.json");
    let event_id = format!(
        "tos.event.discovery.registry-{}.{slug}",
        day.replace('-', "")
    );
    let matching = selected_metadata_observations(preparation, target)?;
    if matching.is_empty() {
        return Err("target has no selected metadata observations".into());
    }
    let mut channels = Vec::new();
    let mut selected = Vec::new();
    for (index, observation) in matching.iter().enumerate() {
        let sequence = index + 1;
        let url = text(observation, "url")?;
        let interface = if url.contains("api.github.com") {
            "api"
        } else {
            "web"
        };
        channels.push(json!({
            "channel_id":format!("channel-{slug}-metadata-{sequence}"),"sequence":sequence,"channel_type":"specialized-scholarly-project","role":"originating-record",
            "source_name":format!("{} upstream metadata/license evidence",text(target,"repository")?),"endpoint_url":url,"interface_type":interface,"interface_version":"pinned repository metadata or dated official page",
            "exact_query":format!("GET {url}"),"queried_at":text(observation,"started_at")?,"elapsed_seconds":metadata_elapsed_seconds(observation)?,"result_order_preserved":true,
            "results":[{"result_id":format!("tos-discovery-result.registry-{slug}-metadata-{sequence}"),"rank":1,"title_as_displayed":selected_name(text(observation,"retained_ref")?),"result_url":url,"originating_record_url":url,"identifiers":[],"available_formats":["metadata/license evidence"],
                "declared_rights":{"statement":"Separate exact-provider statements are retained in the source rights record.","scope":"unknown","evidence_url":url,"tos_conclusion":"evidence-only-not-a-rights-conclusion"},
                "availability":"metadata-only","machine_interface":if interface=="api"{"api"}else{"html"},"decision":"needs-reconciliation",
                "rationale":"The originating project is the first applicable source for this born-digital version. The retained metadata supplies the stated identity and license evidence; authorship and textual assessment follow their recorded source reviews.",
                "acquisition":{"downloaded":false,"acquired_at":null,"byte_size":null,"sha256":null,"event_ref":null},"snapshot":{"state":"captured","format":"static-snapshot","sha256":text(observation,"retained_sha256")?,"reason":text(observation,"retained_ref")?}}]
        }));
    }
    let media_types = arr(target, "files")?
        .iter()
        .map(|entry| text(entry, "media_type").map(str::to_owned))
        .collect::<Result<BTreeSet<_>>>()?;
    for (offset, transfer_row) in transfers.iter().enumerate() {
        let index = matching.len() + offset + 1;
        let result_id = format!("tos-discovery-result.registry-{slug}-file-{index}");
        selected.push(Value::String(result_id.clone()));
        let url = text(transfer_row, "url")?;
        let transfer_formats = media_types.iter().cloned().collect::<Vec<_>>();
        let evidence_url = text(
            matching.last().ok_or("target metadata evidence missing")?,
            "url",
        )?;
        channels.push(json!({
            "channel_id":format!("channel-{slug}-file-{index}"),"sequence":index,"channel_type":"specialized-scholarly-project","role":"digital-object-record",
            "source_name":text(target,"repository")?,"endpoint_url":url,"interface_type":"bulk","interface_version":text(target,"pin")?,"exact_query":format!("GET {url}"),"queried_at":text(transfer_row,"started_at")?,"elapsed_seconds":transfer_row["elapsed_seconds"],"result_order_preserved":true,
            "results":[{"result_id":result_id,"rank":1,"title_as_displayed":selected_name(text(transfer_row,"destination_ref")?),"result_url":url,"originating_record_url":format!("https://github.com/{}/tree/{}",text(target,"repository")?,text(target,"pin")?),
                "identifiers":[{"scheme":"Git commit","value":text(target,"pin")?},{"scheme":"Git blob SHA-1","value":text(transfer_row,"expected_git_blob_sha1")?}],"available_formats":transfer_formats,
                "declared_rights":{"statement":"Provider license evidence is assessed separately in the exact Item rights record.","scope":"digital-object","evidence_url":evidence_url,"tos_conclusion":"evidence-only-not-a-rights-conclusion"},
                "availability":"open-download","machine_interface":"bulk-data","decision":"select","rationale":"Exact prepared version, size and Git blob matched; local SHA-256 and parsing/coverage were observed.",
                "acquisition":{"downloaded":true,"acquired_at":text(transfer_row,"ended_at")?,"byte_size":u64_field(transfer_row,"byte_size")?,"sha256":text(transfer_row,"sha256")?,"event_ref":text(acquisition,"event_id")?},
                "snapshot":{"state":"not-needed","format":null,"sha256":null,"reason":"The immutable acquired File is separately retained in ignored local custody and described by its Item manifest."}}]
        }));
    }
    let comparisons = channels.iter().map(|channel| json!({
        "channel_id":channel["channel_id"],"completeness":"adequate","metadata_precision":"strong","rights_clarity":"adequate","machine_interface_quality":"strong","human_minutes":0,"machine_seconds":channel["elapsed_seconds"],
        "notes":"Elapsed time uses the retained duration or explicit receipt timestamp interval for the frozen exact version. Benchmarking, human effort, source review and rights assessment require their own evidence."
    })).collect::<Vec<_>>();
    let started = channels
        .iter()
        .filter_map(|channel| channel["queried_at"].as_str())
        .min()
        .ok_or("discovery route start time missing")?;
    let manifest_ref = manifest_path
        .strip_prefix(root)
        .map_err(|_| "manifest path outside repository root")?
        .to_string_lossy()
        .replace('\\', "/");
    let mut languages = vec![text(target, "language")?.to_owned()];
    if text(target, "provider")? == "oraec" {
        languages.push("de".into());
    }
    let run = json!({
        "$schema":"https://tree-of-sophia.local/ToS/contracts/material-discovery-record.schema.json","schema_version":"tos_material_discovery_record_v1",
        "discovery_id":format!("tos.discovery.registry-{slug}.{day}.v1"),"protocol_ref":format!("{SOURCE}/discovery/DISCOVERY_PROTOCOL.md"),
        "target":{"target_kind":"expression","known_tos_refs":object(target,"ids")?.values().cloned().collect::<Vec<_>>(),"description":target["version_description"],"required_properties":target["limits"].as_array().cloned().unwrap_or_default().into_iter().chain([json!("exact pinned provider version and immutable local file identity")]).collect::<Vec<_>>(),"acceptable_substitutions":[],"languages":languages,"formats":media_types,"purpose_ref":manifest_ref},
        "channels":channels,"channel_comparison":comparisons,"selected_result_ids":selected,"rejected_result_ids":[],"rights_inference_from_availability_prohibited":true,"general_web_search_is_last_resort":true,"technical_access_bypass_used":false,
        "maker":{"maker_type":"mixed","agent_ref":"model:codex"},"started_at":started,"ended_at":text(acquisition,"ended_at")?,"status":"reconciled","provenance_event_refs":[event_id,text(acquisition,"event_id")?],"record_version":1,"supersedes_discovery_ref":null
    });
    validate_json(root, &run, "material-discovery-record")?;
    publish_json(&path_ref(root, &run_ref)?, &run)?;
    let manifest_raw = custody::read_bytes(manifest_path, None, false, false, MAX_JSON)?;
    let run_raw = custody::read_bytes(&path_ref(root, &run_ref)?, None, false, false, MAX_JSON)?;
    let provenance = event(
        &event_id,
        "discovery",
        started,
        text(acquisition, "ended_at")?,
        vec![
            json!({"ref":manifest_ref,"role":"frozen-registry-version-target","sha256":sha256(&manifest_raw)}),
        ],
        vec![
            json!({"ref":run_ref,"role":"ordered-source-discovery-and-acquisition-receipt","sha256":sha256(&run_raw)}),
        ],
        "ordered-exact-provider-source-route",
        json!({"general_web_search_performed":false,"earlier_research_queries_not_reconstructed":true,"originating_project_first_applicable":true}),
        &format!(
            "{}/rights.json",
            text(field(target, "paths")?, "item_root")?
        ),
        vec![run_ref],
    );
    validate_json(root, &provenance, "provenance-event")?;
    append_jsonl(
        &path_ref(root, &format!("{SOURCE}/discovery/provenance.jsonl"))?,
        &provenance,
    )
}

fn install_target(
    root: &Path,
    manifest_path: &Path,
    preparation: &Value,
    target: &Value,
    package: &Value,
    payload_source_root: Option<&Path>,
    fetch_callback: bool,
) -> Result<Value> {
    preflight_identities(
        root,
        std::slice::from_ref(target),
        &BTreeMap::from([(text(target, "slug")?.to_owned(), package.clone())]),
    )?;
    let evidence_root = manifest_path.parent().ok_or("manifest directory missing")?;
    let log = evidence_root.join("acquisition-transfers.jsonl");
    let manifest_ref = manifest_path
        .strip_prefix(root)
        .map_err(|_| "manifest path outside repository root")?
        .to_string_lossy()
        .replace('\\', "/");
    let package_ref = text(preparation, "prepared_packages_ref")?;
    let item_root_ref = text(field(target, "paths")?, "item_root")?;
    let item_root = path_ref(root, item_root_ref)?;
    let item_manifest_ref = format!("{item_root_ref}/item.manifest.json");
    let item_manifest_path = path_ref(root, &item_manifest_ref)?;
    let item_json = path_ref(root, &format!("{item_root_ref}/item.json"))?;
    if item_json.exists() {
        let checked = verify_target(root, target, payload_source_root)?;
        let day = operation_date(target)?;
        let run_ref = format!(
            "{SOURCE}/discovery/runs/registry-{0}.{day}.v1.json",
            text(target, "slug")?
        );
        if !path_ref(root, &run_ref)?.exists() {
            let provenance_ref = format!("{item_root_ref}/provenance.jsonl");
            let provenance_raw = custody::read_bytes(
                &path_ref(root, &provenance_ref)?,
                None,
                false,
                false,
                MAX_JSON,
            )?;
            let events = std::str::from_utf8(&provenance_raw)
                .map_err(|_| "Item provenance stream is not UTF-8")?
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(|line| strict_json(line.as_bytes()))
                .collect::<Result<Vec<_>>>()?;
            let acquisitions = events
                .iter()
                .filter(|value| {
                    value.get("event_type").and_then(Value::as_str) == Some("acquisition")
                })
                .collect::<Vec<_>>();
            let manifest_bytes = custody::read_bytes(manifest_path, None, false, false, MAX_JSON)?;
            if acquisitions.len() != 1
                || !arr(acquisitions[0], "inputs")?.iter().any(|input| {
                    input.get("ref").and_then(Value::as_str) == Some(&manifest_ref)
                        && input.get("sha256").and_then(Value::as_str)
                            == Some(&sha256(&manifest_bytes))
                })
            {
                return Err("incomplete discovery does not bind the current preparation".into());
            }
            let mut transfers = Vec::new();
            for entry in arr(target, "files")? {
                transfers.push(
                    transfer(
                        root,
                        target,
                        entry,
                        &log,
                        payload_source_root,
                        fetch_callback,
                    )?
                    .1,
                );
            }
            write_discovery(
                root,
                manifest_path,
                preparation,
                target,
                &transfers,
                acquisitions[0],
            )?;
        }
        return Ok(checked);
    }
    let extension = validate_work_extension(root, target, package)?;
    for reference in object(package, "records")?.keys() {
        let path = path_ref(root, reference)?;
        if path.exists() {
            if extension.as_ref().is_none_or(|(extension_ref, before)| {
                extension_ref != reference
                    || custody::read_bytes(&path, None, false, false, MAX_JSON)
                        .ok()
                        .as_deref()
                        != Some(before.as_slice())
            }) {
                return Err(format!(
                    "refusing replacement of an existing source record: {reference}"
                ));
            }
        } else if extension
            .as_ref()
            .is_some_and(|(extension_ref, _)| extension_ref == reference)
        {
            return Err("the Work being extended is no longer present".into());
        }
    }
    for observation in selected_metadata_observations(preparation, target)? {
        metadata_elapsed_seconds(&observation)?;
    }
    let started = utcnow()?;
    let mut bodies = Vec::new();
    let mut transfers = Vec::new();
    for entry in arr(target, "files")? {
        let (body, receipt) = transfer(
            root,
            target,
            entry,
            &log,
            payload_source_root,
            fetch_callback,
        )?;
        bodies.push((entry.clone(), body));
        transfers.push(receipt);
    }
    let observations = inspect_payloads(target, &bodies)?;
    if let Some((work_ref, before)) = &extension {
        if custody::read_bytes(&path_ref(root, work_ref)?, None, false, false, MAX_JSON)? != *before
        {
            return Err("existing Work changed during acquisition; refusing to overwrite".into());
        }
    }
    let ended = utcnow()?;
    let mut records = object(package, "records")?.clone();
    let component_ref = format!("{item_root_ref}/component-witnesses.json");
    if observations
        .get("component_witnesses")
        .and_then(Value::as_array)
        .is_some_and(|rows| !rows.is_empty())
    {
        let component = json!({"schema_version":"tos_observed_bundle_components_v1","item_ref":field(target,"ids")?["item"],"components":observations["component_witnesses"]});
        publish_json(&path_ref(root, &component_ref)?, &component)?;
        for key in ["expression", "translation_expression"] {
            if let Some(reference) = field(target, "paths")?.get(key).and_then(Value::as_str) {
                if let Some(record) = records.get_mut(reference) {
                    record
                        .as_object_mut()
                        .ok_or("prepared corpus record must be an object")?
                        .get_mut("source_refs")
                        .and_then(Value::as_array_mut)
                        .ok_or("prepared source_refs must be an array")?
                        .push(json!(component_ref));
                }
            }
        }
    }
    let snapshot_ref = text(preparation, "source_registry_snapshot_ref")?;
    for (reference, record) in records.iter_mut() {
        if extension
            .as_ref()
            .is_some_and(|(extension_ref, _)| extension_ref == reference)
        {
            continue;
        }
        let sources = record
            .as_object_mut()
            .ok_or("prepared corpus record must be an object")?
            .get_mut("source_refs")
            .and_then(Value::as_array_mut)
            .ok_or("prepared source_refs must be an array")?;
        if !sources
            .iter()
            .any(|source| source.as_str() == Some(snapshot_ref))
        {
            sources.push(json!(snapshot_ref));
        }
    }
    let mut item_manifest = field(package, "manifest_fields")?.clone();
    let source_refs = item_manifest
        .as_object_mut()
        .ok_or("prepared item manifest fields must be an object")?
        .get_mut("source_record_refs")
        .and_then(Value::as_array_mut)
        .ok_or("source_record_refs must be an array")?;
    source_refs.push(json!(snapshot_ref));
    let mut payload_files = Vec::new();
    for (entry, body) in &bodies {
        let digest = sha256(body);
        payload_files.push(json!({
            "file_id":format!("tos.file.sha256.{digest}"),"relative_path":format!("payload/{}",text(entry,"basename")?),"original_basename":text(entry,"basename")?,
            "media_type":text(entry,"media_type")?,"byte_size":body.len(),"sha256":digest,"fixity_verified_at":ended
        }));
    }
    item_manifest
        .as_object_mut()
        .ok_or("prepared item manifest fields must be an object")?
        .insert("payload_files".into(), Value::Array(payload_files));
    validate_json(root, &item_manifest, "source-item-manifest")?;
    publish_json(&item_manifest_path, &item_manifest)?;
    let fixity_ref = format!("{item_root_ref}/fixity.sha256");
    let fixity_path = path_ref(root, &fixity_ref)?;
    custody::publish(
        &fixity_path,
        manifest_fixity_value(&item_manifest)?.as_bytes(),
        0o644,
    )?;
    let rights_ref = text(&item_manifest, "rights_ref")?.to_owned();
    let rights_path = path_ref(root, &rights_ref)?;
    let acquired_rights = rights_with_file_scopes(field(package, "rights")?, &item_manifest)?;
    validate_json(root, &acquired_rights, "rights-record")?;
    publish_json(&rights_path, &acquired_rights)?;
    let forensic_ref = format!("{item_root_ref}/forensic-observations.json");
    publish_json(&path_ref(root, &forensic_ref)?, &observations)?;
    let inventory_ref = text(&item_manifest, "resource_inventory_ref")?;
    let inventory = crate::source_item_inventory::build_registry_inventory(
        root,
        &item_manifest_ref,
        payload_source_root.unwrap_or(&root.join(SOURCE)),
        ended.get(..10).ok_or("native end timestamp lacks date")?,
    )?;
    validate_json(root, &inventory, "source-resource-inventory")?;
    publish_json(&path_ref(root, inventory_ref)?, &inventory)?;
    let forensic_report_ref = format!("{item_root_ref}/forensic-report.md");
    let target_title = text(target, "title")?;
    let byte_size = u64_field(&observations, "byte_size")?;
    let source_statement = if observations
        .get("component_witnesses")
        .and_then(Value::as_array)
        .is_some_and(|rows| !rows.is_empty())
    {
        "`component-witnesses.json` identifies the separate Egyptian and German components in the same immutable JSON file.\n\n"
    } else {
        ""
    };
    let limits = arr(target, "limits")?
        .iter()
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| "target limit must be text".to_owned())
        })
        .collect::<Result<Vec<_>>>()?
        .join("\n");
    let report = format!(
        "# Exact local source intake — {target_title}\n\nInspected: {ended}\n\nVersion: `{}`.\n\nThe {} original files ({} bytes) match the prepared Git blob identities and sizes. The Item manifest records computed SHA-256. The files were opened and parsed locally; source bytes and code-point order were preserved.\n\n`forensic-observations.json` records the exact observed format and target-specific mechanical coverage. `resource-inventory.json` contains the owner's text-free enumeration.\n\n{}\n\n{}\n\n{}\n\nRights are layer-specific provider/license assessments for local acquisition. Visibility remains local-only. Authorship, textual fidelity, translation, semantic assessment, canon and publication follow their recorded owner decisions.\n",
        format!("{}@{}", text(target, "repository")?, text(target, "pin")?),
        bodies.len(),
        byte_size,
        source_statement,
        limits,
        text(target, "responsibility")?
    );
    let report_path = path_ref(root, &forensic_report_ref)?;
    custody::publish(&report_path, report.as_bytes(), 0o644)?;
    for (reference, record) in &records {
        validate_json(root, record, "corpus-record")?;
        let path = path_ref(root, reference)?;
        if let Some((extension_ref, before)) = &extension {
            if extension_ref == reference {
                let metadata = fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
                if metadata.file_type().is_symlink() || !metadata.is_file() {
                    return Err("existing Work replacement target is not a regular file".into());
                }
                replace_bytes_guarded(
                    &path,
                    before,
                    &json_bytes(record)?,
                    metadata.mode() & 0o7777,
                )?;
                continue;
            }
        }
        publish_json(&path, record)?;
    }
    let package_raw = read_ref(root, package_ref)?;
    let inputs = vec![
        json!({"ref":manifest_ref,"role":"reviewed-exact-version-file-list","sha256":sha256(&custody::read_bytes(manifest_path,None,false,false,MAX_JSON)?)}),
        json!({"ref":package_ref,"role":"prepared-source-and-rights-records","sha256":text(preparation,"prepared_packages_sha256")?}),
    ];
    let _ = package_raw;
    let payload_outputs = item_manifest["payload_files"].as_array().ok_or("item manifest payload files missing")?.iter().map(|entry| json!({"ref":entry["file_id"],"role":"immutable-acquired-source-file","sha256":entry["sha256"]})).collect::<Vec<_>>();
    let receipt_refs = vec![
        forensic_report_ref.clone(),
        log.strip_prefix(root)
            .map_err(|_| "transfer log outside repository")?
            .to_string_lossy()
            .replace('\\', "/"),
    ];
    let transfer_starts = transfers
        .iter()
        .map(|row| text(row, "started_at"))
        .collect::<Result<Vec<_>>>()?;
    let transfer_ends = transfers
        .iter()
        .map(|row| text(row, "ended_at"))
        .collect::<Result<Vec<_>>>()?;
    let acquisition_event = event(
        text(&item_manifest, "acquisition_event_ref")?,
        "acquisition",
        transfer_starts
            .into_iter()
            .min()
            .ok_or("no completed transfer start time")?,
        transfer_ends
            .into_iter()
            .max()
            .ok_or("no completed transfer end time")?,
        inputs.clone(),
        payload_outputs.clone(),
        "pinned-upstream-immutable-acquisition",
        json!({"source_urls":arr(target,"files")?.iter().map(|entry|entry["url"].clone()).collect::<Vec<_>>(),"byte_identity":"exact Git blob SHA-1 and local SHA-256","source_bytes_changed":false}),
        &rights_ref,
        receipt_refs.clone(),
    );
    let rights_event = event(
        &format!(
            "tos.event.rights-assessment.registry-{}.{}",
            operation_date(target)?.replace('-', ""),
            text(target, "slug")?
        ),
        "rights_assessment",
        text(field(package, "rights")?, "assessed_at")?,
        &ended,
        inputs.clone(),
        vec![
            json!({"ref":rights_ref,"role":"layer-separated-license-assessment","sha256":sha256(&custody::read_bytes(&rights_path,None,false,false,MAX_JSON)? )}),
        ],
        "prepared-provider-license-scope-assessment",
        json!({"jurisdictions_reviewed":["MX"],"publication_authority":false,"human_legal_review_performed":false,"acquired_rights_transformations":["Bind every computed File ID to the assessed Item scope.","Use the Public Domain Mark URI for layers explicitly assessed from provider public-domain statements; retain the separate digital-object license."],"prepared_rights_template_unchanged":true,"file_scope_binding_performed_at":ended}),
        &rights_ref,
        vec![rights_ref.clone()],
    );
    let inventory_event = event(
        text(&inventory, "provenance_event_ref")?,
        "forensic_inspection",
        &started,
        &ended,
        payload_outputs,
        vec![
            json!({"ref":inventory_ref,"role":"tracked_text_free_resource_inventory","sha256":sha256(&custody::read_bytes(&path_ref(root,inventory_ref)?,None,false,false,MAX_JSON)? )}),
        ],
        "source-resource-inventory",
        json!({"profiles":inventory["files"].as_array().ok_or("inventory files missing")?.iter().map(|row|text(row,"profile").map(str::to_owned)).collect::<Result<BTreeSet<_>>>()?,"source_text_included":false}),
        &rights_ref,
        receipt_refs.clone(),
    );
    let mut events = vec![acquisition_event.clone(), rights_event, inventory_event];
    if let Some((work_ref, before)) = &extension {
        let work_after =
            custody::read_bytes(&path_ref(root, work_ref)?, None, false, false, MAX_JSON)?;
        events.push(event(
            &format!("tos.event.annotation.registry-{}.{}.work-extension",operation_date(target)?.replace('-',""),text(target,"slug")?),"annotation",&started,&ended,
            vec![json!({"ref":text(field(package,"existing_work")?,"preimage_ref")?,"role":"exact-prior-work-record","sha256":sha256(before)}),inputs.first().ok_or("manifest input missing")?.clone(),inputs.get(1).ok_or("package input missing")?.clone()],
            vec![json!({"ref":work_ref,"role":"existing-work-with-additive-expression-claim","sha256":sha256(&work_after)})],
            "guarded-existing-work-expression-extension",json!({"allowed_fields":["expression_claim_refs","record_version"],"prior_claims_preserved":true,"no_new_work_created":true}),
            &rights_ref,receipt_refs.clone(),
        ));
    }
    for value in &events {
        validate_json(root, value, "provenance-event")?;
    }
    let provenance_ref = format!("{item_root_ref}/provenance.jsonl");
    let provenance_raw = events
        .iter()
        .map(|value| serde_json::to_vec(value).map_err(|error| error.to_string()))
        .collect::<Result<Vec<_>>>()?;
    let mut provenance_bytes = Vec::new();
    for bytes in provenance_raw {
        provenance_bytes.extend(bytes);
        provenance_bytes.push(b'\n');
    }
    custody::publish(&path_ref(root, &provenance_ref)?, &provenance_bytes, 0o644)?;
    for claim in arr(package, "claims")? {
        let reference = text(claim, "path")?;
        let path = path_ref(root, reference)?;
        let raw = custody::read_bytes(&path, None, false, false, MAX_JSON)?;
        let body =
            std::str::from_utf8(&raw).map_err(|_| "prepared relation stream is not UTF-8")?;
        let claim_id = text(field(claim, "record")?, "claim_id")?;
        let existing = body
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| strict_json(line.as_bytes()))
            .collect::<Result<Vec<_>>>()?;
        let matches = existing
            .iter()
            .filter(|row| row.get("claim_id").and_then(Value::as_str) == Some(claim_id))
            .cloned()
            .collect::<Vec<_>>();
        if !matches.is_empty() && matches != vec![field(claim, "record")?.clone()] {
            return Err("existing bibliographic claim differs from the prepared record".into());
        }
        if matches.is_empty() {
            append_jsonl(&path, field(claim, "record")?)?;
        }
    }
    refresh_topology(root, evidence_root, &ended)?;
    write_discovery(
        root,
        manifest_path,
        preparation,
        target,
        &transfers,
        &acquisition_event,
    )?;
    verify_target(root, target, payload_source_root)
}

#[cfg(test)]
#[path = "source_registry_acquisition_tests.rs"]
mod tests;
