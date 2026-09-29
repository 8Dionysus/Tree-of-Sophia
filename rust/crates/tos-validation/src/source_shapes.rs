//! Current source instance/schema and repository-reference mechanics.
//! Routes derive from exact selected schema roots; ambiguity or an unknown
//! version remains an explicit gap. A schema result never covers other rules.
use crate::item_rules::{ItemLimits, ItemRefusal};
use crate::source_cut::{CutSchemaExecutor, CutWorkerSchemaExecutor};
use crate::{KeyState, PredicateRead};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tos_foundation::{Digest256, RelativePath, SourceRevision};
use tos_source_store::{CorpusCutReader, SourceMembershipV1};

const PROVENANCE_V2: &str = "tos_provenance_event_v2";
const PROVENANCE_V2_CONTRACT: &str = "ToS/contracts/provenance-event-v2.schema.json";

#[derive(Debug, Clone)]
pub struct SourceShapeReport {
    pub revision: SourceRevision,
    pub carrier_membership: SourceMembershipV1,
    pub checked_instances: u64,
    pub issues: Vec<(String, String)>,
    pub unsupported: Vec<(String, String)>,
    pub reads: Vec<PredicateRead>,
}
struct State {
    limits: ItemLimits,
    bytes: u64,
    state: usize,
    transient: usize,
    issues: Vec<(String, String)>,
    gaps: Vec<(String, String)>,
    reads: Vec<PredicateRead>,
}
impl State {
    fn reserve(&mut self, n: usize) -> Result<(), ItemRefusal> {
        self.state = self
            .state
            .checked_add(n)
            .filter(|n| {
                n.checked_add(self.transient)
                    .is_some_and(|total| total <= self.limits.max_state_bytes)
            })
            .ok_or(ItemRefusal::Budget)?;
        Ok(())
    }
    fn raw(&mut self, n: usize) -> Result<(), ItemRefusal> {
        self.bytes = self
            .bytes
            .checked_add(n as u64)
            .filter(|n| *n <= self.limits.max_total_bytes)
            .ok_or(ItemRefusal::Budget)?;
        if n > self.limits.max_member_bytes {
            Err(ItemRefusal::Budget)
        } else {
            Ok(())
        }
    }
    fn issue(&mut self, path: &str, code: &str) -> Result<(), ItemRefusal> {
        if self.issues.len() >= self.limits.max_issues {
            return Err(ItemRefusal::Budget);
        }
        self.reserve(path.len() + code.len() + 64)?;
        self.issues.push((path.into(), code.into()));
        Ok(())
    }
    fn gap(&mut self, path: &str, code: &str) -> Result<(), ItemRefusal> {
        self.reserve(path.len() + code.len() + 64)?;
        self.gaps.push((path.into(), code.into()));
        Ok(())
    }
    fn read(&mut self, read: PredicateRead) -> Result<(), ItemRefusal> {
        let bytes = match &read {
            PredicateRead::ExactPath { path, digest } => path.len() + digest.len(),
            PredicateRead::ExactBytes { locator, digest } => locator.len() + digest.len(),
            PredicateRead::SchemaResource { uri, digest } => uri.len() + digest.len(),
            PredicateRead::RefEndpoint {
                endpoint_type, id, ..
            } => endpoint_type.len() + id.len(),
            _ => {
                return Err(ItemRefusal::Unsupported(
                    "source-shape read accounting".into(),
                ));
            }
        };
        self.reserve(bytes + 96)?;
        self.reads.push(read);
        Ok(())
    }
}

/// An event records original inputs, while this family can prove only bytes
/// selected in the current source cut. A changed historical input is a gap;
/// a selected, tracked output claiming its current digest is a defect.
fn provenance_ref(
    cut: &CorpusCutReader,
    revision: SourceRevision,
    owner: &str,
    reference: &str,
    digest: Option<&str>,
    current_output: bool,
    state: &mut State,
    cancelled: &AtomicBool,
) -> Result<(), ItemRefusal> {
    check(state.limits, cancelled)?;
    if cut.current().revision() != revision {
        return Err(ItemRefusal::Source(
            "provenance reference belongs to another source cut".into(),
        ));
    }
    if !(reference.starts_with("ToS/") || reference.starts_with("scripts/")) {
        state.gap(owner, "provenance-ref-outside-selected-source")?;
        return Ok(());
    }
    let Ok(relative) = RelativePath::parse(reference) else {
        state.issue(owner, "unsafe-provenance-source-ref")?;
        return Ok(());
    };
    let metadata = cut.current().member(&relative);
    state.read(PredicateRead::RefEndpoint {
        endpoint_type: "provenance-source-path".into(),
        id: reference.into(),
        observed: if metadata.is_some() {
            KeyState::Present
        } else {
            KeyState::Absent
        },
    })?;
    let Some(metadata) = metadata else {
        // The selected cut need not contain every local/private output.
        // Absence cannot establish that the owner's original bytes vanished.
        state.gap(owner, "provenance-recorded-ref-unavailable")?;
        return Ok(());
    };
    let actual = metadata.sha256.to_hex();
    state.read(PredicateRead::ExactBytes {
        locator: reference.into(),
        digest: actual.clone(),
    })?;
    if digest.is_some_and(|expected| expected != actual) {
        if current_output {
            state.issue(owner, "current-provenance-output-fixity")?;
        } else {
            state.gap(owner, "provenance-recorded-input-differs-from-current")?;
        }
    }
    Ok(())
}

fn provenance_bindings(
    cut: &CorpusCutReader,
    revision: SourceRevision,
    owner: &str,
    node: &Value,
    depth: usize,
    state: &mut State,
    cancelled: &AtomicBool,
) -> Result<(), ItemRefusal> {
    if depth > 128 {
        return Err(ItemRefusal::Unsupported("provenance binding depth".into()));
    }
    check(state.limits, cancelled)?;
    match node {
        Value::Object(object) => {
            if let (Some(reference), Some(digest)) = (
                object.get("ref").and_then(Value::as_str),
                object.get("sha256").and_then(Value::as_str),
            ) {
                provenance_ref(
                    cut,
                    revision,
                    owner,
                    reference,
                    Some(digest),
                    false,
                    state,
                    cancelled,
                )?;
            }
            if let (Some(reference), Some(digest)) = (
                object.get("artifact_ref").and_then(Value::as_str),
                object.get("artifact_sha256").and_then(Value::as_str),
            ) {
                provenance_ref(
                    cut,
                    revision,
                    owner,
                    reference,
                    Some(digest),
                    false,
                    state,
                    cancelled,
                )?;
            }
            for field in ["agent_ref", "actor_ref"] {
                if let Some(reference) = object.get(field).and_then(Value::as_str) {
                    if reference.starts_with("ToS/") || reference.starts_with("scripts/") {
                        provenance_ref(
                            cut, revision, owner, reference, None, false, state, cancelled,
                        )?;
                    }
                }
            }
            for value in object.values() {
                provenance_bindings(cut, revision, owner, value, depth + 1, state, cancelled)?;
            }
        }
        Value::Array(values) => {
            for value in values {
                provenance_bindings(cut, revision, owner, value, depth + 1, state, cancelled)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn provenance_v2(
    cut: &CorpusCutReader,
    revision: SourceRevision,
    owner: &str,
    event: &Value,
    state: &mut State,
    cancelled: &AtomicBool,
) -> Result<(), ItemRefusal> {
    let available = state
        .limits
        .max_state_bytes
        .checked_sub(state.state)
        .and_then(|n| n.checked_sub(state.transient))
        .ok_or(ItemRefusal::Budget)?;
    let workspace = crate::provenance_rules::semantic_workspace(
        event,
        state.limits.max_issues.saturating_sub(state.issues.len()),
        available,
    )?;
    state.transient = state
        .transient
        .checked_add(workspace)
        .ok_or(ItemRefusal::Budget)?;
    state.reserve(0)?;
    let messages = crate::provenance_rules::semantic_issues(
        event,
        state.limits.max_issues.saturating_sub(state.issues.len()),
        state.limits.deadline,
    )?;
    state.transient -= workspace;
    let message_state = std::mem::size_of::<Vec<&'static str>>()
        .checked_add(
            messages
                .len()
                .checked_mul(std::mem::size_of::<&'static str>())
                .ok_or(ItemRefusal::Budget)?,
        )
        .ok_or(ItemRefusal::Budget)?;
    state.transient = state
        .transient
        .checked_add(message_state)
        .ok_or(ItemRefusal::Budget)?;
    for message in messages {
        state.issue(owner, message)?;
    }
    state.transient -= message_state;

    if let Some(reference) = event
        .pointer("/record_binding/manifest_ref")
        .and_then(Value::as_str)
    {
        provenance_ref(
            cut, revision, owner, reference, None, false, state, cancelled,
        )?;
    }
    for group in ["inputs", "outputs", "byproducts"] {
        if let Some(entities) = event
            .pointer(&format!("/entities/{group}"))
            .and_then(Value::as_array)
        {
            for entity in entities {
                check(state.limits, cancelled)?;
                if let (Some(reference), Some(digest)) = (
                    entity.get("entity_ref").and_then(Value::as_str),
                    entity.get("sha256").and_then(Value::as_str),
                ) {
                    let current_output = group != "inputs"
                        && entity.get("availability").and_then(Value::as_str) == Some("tracked");
                    provenance_ref(
                        cut,
                        revision,
                        owner,
                        reference,
                        Some(digest),
                        current_output,
                        state,
                        cancelled,
                    )?;
                }
            }
        }
    }
    provenance_bindings(cut, revision, owner, event, 0, state, cancelled)
}
fn check(limits: ItemLimits, cancelled: &AtomicBool) -> Result<(), ItemRefusal> {
    if cancelled.load(Ordering::Relaxed) || Instant::now() >= limits.deadline {
        Err(ItemRefusal::Deadline)
    } else if limits.max_member_bytes == 0
        || limits.max_member_bytes == usize::MAX
        || limits.max_total_bytes == 0
        || limits.max_total_bytes == u64::MAX
        || limits.max_state_bytes == 0
        || limits.max_state_bytes == usize::MAX
        || limits.max_issues == 0
        || limits.max_issues == usize::MAX
    {
        Err(ItemRefusal::Budget)
    } else {
        Ok(())
    }
}

/// Extract only root instance version constraints, following same-document
/// root $refs/allOf/anyOf/oneOf. Do not harvest unrelated nested object/Claim
/// schema_version properties and turn them into an instance routing registry.
fn root_versions(
    root: &Value,
    node: &Value,
    pending: &mut BTreeSet<String>,
    out: &mut BTreeSet<String>,
    depth: usize,
) -> Result<(), ItemRefusal> {
    if depth > 64 {
        return Err(ItemRefusal::Unsupported("schema root route depth".into()));
    }
    if let Some(version) = node
        .pointer("/properties/schema_version/const")
        .and_then(Value::as_str)
    {
        out.insert(version.into());
    }
    if let Some(versions) = node
        .pointer("/properties/schema_version/enum")
        .and_then(Value::as_array)
    {
        out.extend(versions.iter().filter_map(Value::as_str).map(str::to_owned));
    }
    if let Some(reference) = node.get("$ref").and_then(Value::as_str) {
        if reference.starts_with('#') && pending.insert(reference.into()) {
            let pointer = &reference[1..];
            let target = if pointer.is_empty() {
                Some(root)
            } else {
                root.pointer(pointer)
            };
            if let Some(target) = target {
                root_versions(root, target, pending, out, depth + 1)?;
            }
        }
    }
    for keyword in ["allOf", "anyOf", "oneOf"] {
        if let Some(branches) = node.get(keyword).and_then(Value::as_array) {
            for branch in branches {
                root_versions(root, branch, pending, out, depth + 1)?;
            }
        }
    }
    Ok(())
}

pub fn inspect_source_shapes_from_cut(
    cut: &CorpusCutReader,
    limits: ItemLimits,
    cancelled: &AtomicBool,
    schemas: &mut CutWorkerSchemaExecutor,
) -> Result<SourceShapeReport, ItemRefusal> {
    check(limits, cancelled)?;
    let revision = cut.current().revision();
    if schemas.source_revision() != revision {
        return Err(ItemRefusal::Source(
            "shape worker belongs to another source cut".into(),
        ));
    }
    let mut state = State {
        limits,
        bytes: 0,
        state: 0,
        transient: 0,
        issues: Vec::new(),
        gaps: Vec::new(),
        reads: Vec::new(),
    };
    let mut by_version: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut by_uri = BTreeMap::new();
    let mut provenance_contract = None;
    for metadata in cut.current().members() {
        check(limits, cancelled)?;
        let path = metadata.path.as_str();
        if !path.starts_with("ToS/contracts/") || !path.ends_with(".schema.json") {
            continue;
        }
        let member = cut
            .read_member(
                revision,
                &metadata.path,
                limits.max_member_bytes as u64,
                limits.deadline,
                cancelled,
            )
            .map_err(store_error)?;
        state.raw(member.raw.len())?;
        state.transient = member.raw.len().checked_mul(8).ok_or(ItemRefusal::Budget)?;
        state.reserve(0)?;
        let schema = crate::published_value(&member.raw, limits.max_member_bytes)
            .map_err(|error| ItemRefusal::Unsupported(format!("source schema route: {error:?}")))?;
        let uri = schema
            .get("$id")
            .and_then(Value::as_str)
            .ok_or_else(|| ItemRefusal::Unsupported("source schema route ID".into()))?;
        state.reserve(uri.len() + path.len() + 128)?;
        if by_uri.insert(uri.to_owned(), path.to_owned()).is_some() {
            return Err(ItemRefusal::Unsupported(
                "duplicate source schema route ID".into(),
            ));
        }
        let mut versions = BTreeSet::new();
        root_versions(&schema, &schema, &mut BTreeSet::new(), &mut versions, 0)?;
        if path == PROVENANCE_V2_CONTRACT {
            if uri != format!("https://tree-of-sophia.local/{PROVENANCE_V2_CONTRACT}")
                || !versions.contains(PROVENANCE_V2)
            {
                return Err(ItemRefusal::Unsupported(
                    "current provenance-v2 contract identity".into(),
                ));
            }
            let digest = Digest256::of_bytes(&member.raw).to_hex();
            state.reserve(digest.len() + std::mem::size_of::<Option<String>>())?;
            state.read(PredicateRead::SchemaResource {
                uri: uri.into(),
                digest: digest.clone(),
            })?;
            provenance_contract = Some(digest);
        }
        for version in versions {
            state.reserve(version.len() + path.len() + 128)?;
            by_version.entry(version).or_default().insert(path.into());
        }
        state.transient = 0;
    }
    let mut stream = cut.stream(revision).map_err(store_error)?;
    let mut checked_instances = 0u64;
    while let Some(member) = stream
        .next_member(limits.deadline, cancelled)
        .map_err(store_error)?
    {
        check(limits, cancelled)?;
        state.raw(member.raw.len())?;
        let path = member.path.as_str();
        if !(path.starts_with("ToS/source-witnesses/") || path.starts_with("ToS/research-packets/"))
            || !path.ends_with(".json")
        {
            continue;
        }
        // Admit the live member and decoded tree before allocating the tree;
        // the same transient remains charged through schema and ref checks.
        state.transient = member.raw.len().checked_mul(8).ok_or(ItemRefusal::Budget)?;
        state.reserve(0)?;
        let value = match crate::native_decoded_value(&member.raw, limits.max_member_bytes) {
            Ok(value) => value,
            Err(ItemRefusal::Source(_)) => {
                state.issue(path, "invalid-json")?;
                state.transient = 0;
                continue;
            }
            Err(error) => return Err(error),
        };
        if !value.is_object() {
            state.gap(path, "non-object-source-owner-route")?;
            state.transient = 0;
            continue;
        }
        state.read(PredicateRead::ExactPath {
            path: path.into(),
            digest: Digest256::of_bytes(&member.raw).to_hex(),
        })?;
        let version = value.get("schema_version").and_then(Value::as_str);
        let explicit = value
            .get("$schema")
            .and_then(Value::as_str)
            .and_then(|uri| by_uri.get(uri));
        let route = if version == Some(PROVENANCE_V2) {
            provenance_contract.as_ref().ok_or_else(|| {
                ItemRefusal::Unsupported(
                    "current provenance-v2 contract absent from selected cut".into(),
                )
            })?;
            Some(PROVENANCE_V2_CONTRACT.to_owned())
        } else {
            explicit.cloned().or_else(|| {
                version
                    .and_then(|version| by_version.get(version))
                    .filter(|routes| routes.len() == 1)
                    .and_then(|routes| routes.first().cloned())
            })
        };
        if let Some(contract) = route {
            if !schemas.check(path, &member.raw, &contract, limits.deadline, cancelled)? {
                state.issue(path, "source-owner-schema")?;
            }
            checked_instances = checked_instances
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
        } else {
            state.gap(
                path,
                if version.is_some() {
                    "unknown-or-ambiguous-source-schema-route"
                } else {
                    "source-owner-without-root-schema-version"
                },
            )?;
        }
        if version == Some(PROVENANCE_V2) {
            provenance_v2(cut, revision, path, &value, &mut state, cancelled)?;
        }
        let refs = ["source_refs", "source_record_refs", "receipt_refs"]
            .into_iter()
            .flat_map(|field| {
                value
                    .get(field)
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
            })
            .chain(
                [
                    "rights_ref",
                    "provenance_ref",
                    "forensic_report_ref",
                    "resource_inventory_ref",
                    "generated_from_manifest_ref",
                    "item_manifest_ref",
                ]
                .into_iter()
                .filter_map(|field| value.get(field)),
            );
        for reference in refs {
            check(limits, cancelled)?;
            let Some(reference) = reference.as_str() else {
                state.issue(path, "unresolved-source-ref")?;
                continue;
            };
            if !reference.starts_with("ToS/") {
                continue;
            }
            let relative = RelativePath::parse(reference)
                .map_err(|_| ItemRefusal::Unsupported("source-shape reference path".into()))?;
            let present = cut.presence(revision, &relative).is_some();
            state.read(PredicateRead::RefEndpoint {
                endpoint_type: "source-path".into(),
                id: reference.into(),
                observed: if present {
                    KeyState::Present
                } else {
                    KeyState::Absent
                },
            })?;
            if !present {
                state.issue(path, "unresolved-source-ref")?;
            }
        }
        state.transient = 0;
    }
    let carrier_membership = stream
        .coverage()
        .ok_or_else(|| ItemRefusal::Source("source shape EOF incomplete".into()))?;
    check(limits, cancelled)?;
    Ok(SourceShapeReport {
        revision,
        carrier_membership,
        checked_instances,
        issues: state.issues,
        unsupported: state.gaps,
        reads: state.reads,
    })
}
fn store_error(error: tos_source_store::StoreError) -> ItemRefusal {
    match error.code {
        tos_source_store::StoreErrorCode::BudgetExceeded => ItemRefusal::Budget,
        tos_source_store::StoreErrorCode::UnsupportedFormat
        | tos_source_store::StoreErrorCode::UnsupportedPlatform => {
            ItemRefusal::Unsupported(error.to_string())
        }
        _ => ItemRefusal::Source(error.to_string()),
    }
}
