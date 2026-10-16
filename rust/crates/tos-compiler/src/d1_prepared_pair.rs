//! Validate exact source-input and reader bindings for private D1 capture.
//! The caller holds the snapshots and owns selection, rights and publication.

use crate::d1::{
    D1PairFailure, D1PrivatePreparedInput, D1PrivatePreparedMode, private_prepared_target_revision,
};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Component, Path},
};
use tos_foundation::Digest256;

const SOURCE_INPUTS_SCHEMA: &str = "tos_prepared_source_inputs_v1";
const D1_READER_SCHEMA: &str = "tos_cloudflare_edge_read_model_v9";
const MAX_SOURCE_INPUTS_BYTES: usize = 1_048_576;
const MAX_ROOTS: usize = 16;
const MAX_DEPENDENCIES: usize = 1024;
const MAX_NAME_BYTES: usize = 4096;

#[derive(Debug)]
pub enum D1GateFailure {
    Pair(D1PairFailure),
    Invalid(&'static str),
    Budget(&'static str),
}
impl From<D1PairFailure> for D1GateFailure {
    fn from(value: D1PairFailure) -> Self {
        Self::Pair(value)
    }
}
pub type D1GateResult<T> = std::result::Result<T, D1GateFailure>;

fn digest(value: &[u8]) -> String {
    Digest256::of_bytes(value).to_hex()
}
fn exact_digest(value: &str) -> D1GateResult<()> {
    Digest256::from_hex(value).map_err(|_| D1GateFailure::Invalid("D1 snapshot digest"))?;
    Ok(())
}

fn bounded_name(value: &str) -> D1GateResult<()> {
    if value.is_empty() || value.len() > MAX_NAME_BYTES || value.contains('\0') {
        return Err(D1GateFailure::Budget("D1 owner name bytes"));
    }
    Ok(())
}
fn field<'a>(value: &'a Value, key: &str) -> D1GateResult<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or(D1GateFailure::Invalid("D1 paired JSON field"))
}
fn parse_json(raw: &[u8], max: usize) -> D1GateResult<Value> {
    if raw.is_empty() || raw.len() > max {
        return Err(D1GateFailure::Budget("D1 paired JSON bytes"));
    }
    serde_json::from_slice(raw).map_err(|_| D1GateFailure::Invalid("D1 paired JSON"))
}

fn verify_inputs_raw(
    raw: &[u8],
    source_revision: &str,
    navigation_sha256: Option<&str>,
    expected_sha: &str,
) -> D1GateResult<(BTreeMap<String, String>, BTreeMap<String, String>)> {
    exact_digest(expected_sha)?;
    if digest(raw) != expected_sha {
        return Err(D1GateFailure::Invalid(
            "exact prepared source inputs digest",
        ));
    }
    let value = parse_json(raw, MAX_SOURCE_INPUTS_BYTES)?;
    let mut canonical = serde_json::to_vec(&value)
        .map_err(|_| D1GateFailure::Invalid("source inputs canonical JSON"))?;
    canonical.push(b'\n');
    if canonical != raw {
        return Err(D1GateFailure::Invalid("source inputs canonical bytes"));
    }
    let object = value
        .as_object()
        .ok_or(D1GateFailure::Invalid("source inputs object"))?;
    if object.keys().map(String::as_str).collect::<BTreeSet<_>>()
        != BTreeSet::from([
            "schema",
            "source_revision",
            "source_publication",
            "dependencies",
            "roots",
        ])
        || field(&value, "schema")? != SOURCE_INPUTS_SCHEMA
        || field(&value, "source_revision")? != source_revision
    {
        return Err(D1GateFailure::Invalid("source inputs identity"));
    }
    if let Some(token) = value.get("source_publication").and_then(Value::as_str) {
        let digest = token
            .strip_prefix("sha256:")
            .ok_or(D1GateFailure::Invalid("source publication token"))?;
        exact_digest(digest)?;
    } else if !value.get("source_publication").is_some_and(Value::is_null) {
        return Err(D1GateFailure::Invalid("source publication token"));
    }
    let dependencies = value
        .get("dependencies")
        .and_then(Value::as_object)
        .ok_or(D1GateFailure::Invalid("source dependencies"))?;
    if dependencies.len() > MAX_DEPENDENCIES {
        return Err(D1GateFailure::Budget("source dependency count"));
    }
    let mut deps = BTreeMap::new();
    for (name, value) in dependencies {
        bounded_name(name)?;
        let sha = value
            .as_str()
            .ok_or(D1GateFailure::Invalid("source dependency digest"))?;
        exact_digest(sha)?;
        deps.insert(name.clone(), sha.to_owned());
    }
    let roots = value
        .get("roots")
        .and_then(Value::as_object)
        .ok_or(D1GateFailure::Invalid("source roots"))?;
    if roots.is_empty() || roots.len() > MAX_ROOTS {
        return Err(D1GateFailure::Budget("source root count"));
    }
    let mut selected = BTreeMap::new();
    for (name, root) in roots {
        bounded_name(name)?;
        let path = Path::new(field(root, "namespace_path")?);
        if !path.is_absolute() || path.components().any(|part| part == Component::ParentDir) {
            return Err(D1GateFailure::Invalid("source root namespace path"));
        }
        let raw = field(root, "root_json")?;
        if raw.len() > MAX_SOURCE_INPUTS_BYTES {
            return Err(D1GateFailure::Budget("source root JSON bytes"));
        }
        let sha = field(root, "snapshot_sha256")?;
        exact_digest(sha)?;
        if digest(raw.as_bytes()) != sha {
            return Err(D1GateFailure::Invalid("source root bytes digest"));
        }
        selected.insert(name.clone(), sha.to_owned());
    }
    if selected.get("source-navigation").map(String::as_str) != navigation_sha256 {
        return Err(D1GateFailure::Invalid("paired navigation root differs"));
    }
    Ok((selected, deps))
}

fn verify_nonparticipating_source_scope(
    before_roots: &BTreeMap<String, String>,
    after_roots: &BTreeMap<String, String>,
    before_dependencies: &BTreeMap<String, String>,
    after_dependencies: &BTreeMap<String, String>,
    failure: &'static str,
) -> D1GateResult<()> {
    if before_roots.keys().ne(after_roots.keys())
        || before_roots.iter().any(|(name, sha)| {
            !matches!(
                name.as_str(),
                "source-catalog" | "bibliographic-claims" | "source-navigation"
            ) && after_roots.get(name) != Some(sha)
        })
        || before_dependencies.keys().ne(after_dependencies.keys())
        || before_dependencies.iter().any(|(name, sha)| {
            !matches!(
                name.as_str(),
                "claim-publication-profile" | "metadata-addition-publication-profile"
            ) && after_dependencies.get(name) != Some(sha)
        })
    {
        return Err(D1GateFailure::Invalid(failure));
    }
    Ok(())
}

fn verify_optional_rights_root(
    roots: &BTreeMap<String, String>,
    navigation_sha256: Option<&str>,
    rights_sha256: Option<&str>,
    failure: &'static str,
) -> D1GateResult<()> {
    match (navigation_sha256, rights_sha256) {
        (None, None) => Ok(()),
        (Some(_), Some(expected)) => {
            let source_rights = roots
                .get("source-navigation-rights")
                .or_else(|| roots.get("source-navigation"));
            if source_rights.map(String::as_str) != Some(expected) {
                return Err(D1GateFailure::Invalid(failure));
            }
            Ok(())
        }
        _ => Err(D1GateFailure::Invalid(failure)),
    }
}

fn verify_reader_top(raw: &str, revision: &str, source_revision: &str) -> D1GateResult<()> {
    let value = parse_json(raw.as_bytes(), 32_000)?;
    if field(&value, "read_model_schema")? != D1_READER_SCHEMA
        || field(&value, "data_revision")? != revision
        || field(&value, "source_revision")? != source_revision
    {
        return Err(D1GateFailure::Invalid("selected D1 reader top"));
    }
    Ok(())
}

pub fn inspect_private_prepared_source_inputs(
    spec: &D1PrivatePreparedInput,
    before_inputs_raw: Option<&[u8]>,
    after_inputs_raw: Option<&[u8]>,
) -> D1GateResult<()> {
    use D1PrivatePreparedMode as Mode;

    let target = private_prepared_target_revision(spec)?;
    verify_reader_top(
        &spec.before_reader_top,
        &spec.base_d1_revision,
        &spec.before_source_revision,
    )?;
    verify_reader_top(&spec.after_reader_top, &target, &spec.after_source_revision)?;
    match spec.predecessor_mode {
        Mode::Pair | Mode::CatchUp => {
            let before_raw = before_inputs_raw.ok_or(D1GateFailure::Invalid(
                "private predecessor source inputs absent",
            ))?;
            let after_raw = after_inputs_raw.ok_or(D1GateFailure::Invalid(
                "private successor source inputs absent",
            ))?;
            let (before_roots, before_dependencies) = verify_inputs_raw(
                before_raw,
                &spec.before_source_revision,
                spec.before_navigation_sha256.as_deref(),
                spec.before_source_inputs_sha256
                    .as_deref()
                    .ok_or(D1GateFailure::Invalid(
                        "private predecessor inputs digest absent",
                    ))?,
            )?;
            let (after_roots, after_dependencies) = verify_inputs_raw(
                after_raw,
                &spec.after_source_revision,
                spec.after_navigation_sha256.as_deref(),
                spec.after_source_inputs_sha256
                    .as_deref()
                    .ok_or(D1GateFailure::Invalid(
                        "private successor inputs digest absent",
                    ))?,
            )?;
            verify_nonparticipating_source_scope(
                &before_roots,
                &after_roots,
                &before_dependencies,
                &after_dependencies,
                "private nonparticipating source scope changed",
            )?;
            verify_optional_rights_root(
                &before_roots,
                spec.before_navigation_sha256.as_deref(),
                spec.before_rights_sha256.as_deref(),
                "private predecessor source rights root differs",
            )?;
            verify_optional_rights_root(
                &after_roots,
                spec.after_navigation_sha256.as_deref(),
                spec.after_rights_sha256.as_deref(),
                "private successor source rights root differs",
            )?;
        }
        Mode::Bootstrap => {
            if before_inputs_raw.is_some() {
                return Err(D1GateFailure::Invalid(
                    "private bootstrap predecessor source inputs present",
                ));
            }
            let after_raw = after_inputs_raw.ok_or(D1GateFailure::Invalid(
                "private bootstrap source inputs absent",
            ))?;
            let (after_roots, _) = verify_inputs_raw(
                after_raw,
                &spec.after_source_revision,
                spec.after_navigation_sha256.as_deref(),
                spec.after_source_inputs_sha256
                    .as_deref()
                    .ok_or(D1GateFailure::Invalid(
                        "private bootstrap inputs digest absent",
                    ))?,
            )?;
            verify_optional_rights_root(
                &after_roots,
                spec.after_navigation_sha256.as_deref(),
                spec.after_rights_sha256.as_deref(),
                "private bootstrap source rights root differs",
            )?;
        }
        Mode::Integrity { .. } => {
            if before_inputs_raw.is_some() || after_inputs_raw.is_some() {
                return Err(D1GateFailure::Invalid(
                    "private integrity source inputs must be absent",
                ));
            }
        }
    }
    Ok(())
}
