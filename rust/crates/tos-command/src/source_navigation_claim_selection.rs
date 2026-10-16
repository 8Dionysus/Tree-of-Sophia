//! Bounded staged source selection for event-local Global Claim projection.
//!
//! Each stage derives addresses only from exact selected bytes. It never walks
//! the authored corpus or treats the returned path list as source authority.

use crate::{DurableError, DurableNavigationEventContextV1, DurableResult};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use tos_foundation::{
    CanonicalProfile, Digest256, JsonLimits, JsonMode, JsonValue, RelativePath, canonical_bytes_v1,
    parse_json,
};

const MAX_PATH_BYTES: usize = 4096;
const MAX_CLAIM_STREAM_BYTES: usize = 1_048_576;
const MAX_CLAIM_LINE_BYTES: usize = 1_048_576;
const MAX_EXACT_RECORD_BYTES: usize = 8 * 1024 * 1024;
const MAX_CLAIM_OWNERS: usize = 16_384;
const MAX_ARCHIVES: usize = 128;
const MAX_ARCHIVE_FILES: usize = 64;
const MAX_PATHS: usize = 4096;
const ENTITY_REGISTRY: &str = "ToS/doctrine/semantic-interchange/entity-types.v1.json";
const ENTITY_SCHEMA: &str = "ToS/contracts/semantic-entity-type-registry.schema.json";
const RELATION_REGISTRY: &str = "ToS/doctrine/semantic-interchange/relation-types.v1.json";
const RELATION_SCHEMA: &str = "ToS/contracts/semantic-relation-type-registry.schema.json";
const CLAIM_SCHEMA: &str = "ToS/contracts/source-claim-record.schema.json";
const FORM_SCHEMA: &str = "ToS/contracts/human-form-set.schema.json";
const PROVENANCE_SCHEMA: &str = "ToS/contracts/provenance-event-v2.schema.json";
const PROVENANCE_V1_SCHEMA: &str = "ToS/contracts/provenance-event.schema.json";
const ANCHOR_SCHEMA: &str = "ToS/contracts/source-anchor.schema.json";
const CLAIM_PROFILE_SUPPORT_SCHEMAS: &[&str] = &[
    "ToS/contracts/claim-packet.schema.json",
    "ToS/contracts/knowledge-assessment.schema.json",
    "ToS/contracts/historical-claim.schema.json",
    "ToS/contracts/corpus-record.schema.json",
    "ToS/contracts/source-structured-value.schema.json",
    "ToS/contracts/scoped-member-structure.schema.json",
    "ToS/contracts/claim-display-fields.schema.json",
];

#[derive(Clone, Debug)]
pub(crate) struct ClaimEventOwnerRowV1<'a> {
    pub claim_id: String,
    pub source_line: u64,
    pub exact_row_bytes: &'a [u8],
}

#[derive(Clone, Debug)]
pub(crate) struct ClaimEventOwnerStreamV1<'a> {
    pub source_path: String,
    pub exact_stream_bytes: &'a [u8],
    pub owners: Vec<ClaimEventOwnerRowV1<'a>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ClaimEventIdentityRequestV1 {
    pub id: String,
    pub required: bool,
}

/// One result of the fenced `SourceRevisionReadSession::lookup_source_identity`
/// point lookup. The caller must reselect any returned path in the final event
/// context before supplying its bytes to the renderer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ClaimEventIdentityOwnerV1 {
    pub id: String,
    pub kind: Option<String>,
    pub source_path: Option<String>,
}

/// Read the bounded Claim member IDs from the exact selected CMD mutation
/// receipt using the renderer's maintained receipt-shape parser.
pub(crate) fn claim_event_mutation_claim_ids_v1(
    receipt_bytes: &[u8],
    max_bytes: usize,
) -> DurableResult<Vec<String>> {
    tos_compiler::source_navigation_claim_event::selected_claim_mutation_ids_v1(
        receipt_bytes,
        max_bytes,
    )
    .map_err(|_| DurableError::Conflict("selected Claim mutation receipt shape differs"))
}

fn claim_event_archive_manifest_paths_for_streams(
    context: &DurableNavigationEventContextV1,
    streams: &[ClaimEventOwnerStreamV1<'_>],
) -> DurableResult<Vec<String>> {
    let mut paths = BTreeSet::new();
    for stream in streams {
        let history_path = sibling_path(&stream.source_path, "claim-revision-history.json")?;
        let Some(history_bytes) = selected_optional_bytes(context, &history_path)? else {
            if stream.owners.iter().any(|owner| {
                strict_value(owner.exact_row_bytes)
                    .ok()
                    .and_then(|claim| claim.get("claim_version").and_then(Value::as_u64))
                    .is_some_and(|version| version > 1)
            }) {
                return Err(DurableError::Conflict("revised Claim history is absent"));
            }
            continue;
        };
        let history = strict_value(history_bytes)?;
        if text(&history, "schema_version")? != "tos_claim_revision_history_v1"
            || text(&history, "source_path")? != stream.source_path
        {
            return Err(DurableError::Conflict("Claim history owner differs"));
        }
        let receipts = array(&history, "receipts")?;
        if receipts.len() > MAX_ARCHIVES {
            return Err(DurableError::Refused("Claim history archive count"));
        }
        let mut archives = BTreeSet::new();
        for receipt in receipts {
            let previous = receipt
                .get("previous_source")
                .ok_or(DurableError::Corrupt("Claim history predecessor absent"))?;
            let id = text(previous, "id")?;
            let revision = text(receipt, "previous_revision")?
                .strip_prefix("sha256:")
                .ok_or(DurableError::Corrupt("Claim history revision digest"))?;
            Digest256::from_hex(revision)
                .map_err(|_| DurableError::Corrupt("Claim history revision digest"))?;
            let expected = format!(
                "ToS/source-witnesses/.record-revisions/{}-{revision}",
                Digest256::of_bytes(id.as_bytes()).to_hex()
            );
            if text(receipt, "archive_path")? != expected || !archives.insert(expected.clone()) {
                return Err(DurableError::Conflict(
                    "Claim history archive locator differs",
                ));
            }
            add_path(&mut paths, &format!("{expected}/manifest.json"))?;
        }
    }
    paths_within_limit(paths)
}

fn validate_identity_result_set(
    requests: &[ClaimEventIdentityRequestV1],
    owners: &[ClaimEventIdentityOwnerV1],
) -> DurableResult<()> {
    if owners.len() != requests.len()
        || owners
            .iter()
            .zip(requests)
            .any(|(owner, request)| owner.id != request.id)
    {
        return Err(DurableError::Conflict(
            "Claim identity index result set differs",
        ));
    }
    Ok(())
}

fn claim_event_identity_archive_paths_for_refs(
    context: &DurableNavigationEventContextV1,
    owners: &[ClaimEventIdentityOwnerV1],
    collection_refs: &BTreeMap<String, ExactSourceReferenceV1>,
    membership_refs: &BTreeMap<String, ExactSourceReferenceV1>,
) -> DurableResult<Vec<String>> {
    let owner_by_id = owners
        .iter()
        .map(|owner| (owner.id.as_str(), owner))
        .collect::<BTreeMap<_, _>>();
    let mut paths = BTreeSet::new();
    for exact in membership_refs.values() {
        let owner = owner_by_id
            .get(exact.id.as_str())
            .copied()
            .ok_or(DurableError::Conflict(
                "membership Claim identity is not indexed",
            ))?;
        let (Some(kind), Some(source_path)) = (&owner.kind, &owner.source_path) else {
            return Err(DurableError::Conflict(
                "membership Claim identity is not indexed",
            ));
        };
        if kind != "claim"
            || !(is_claim_stream_path(source_path) || is_historical_claim_stream_path(source_path))
        {
            return Err(DurableError::Conflict(
                "membership Claim identity owner differs",
            ));
        }
        add_exact_claim_history_archives(context, source_path, exact, &mut paths)?;
    }
    for exact in collection_refs.values() {
        let owner = owner_by_id
            .get(exact.id.as_str())
            .copied()
            .ok_or(DurableError::Conflict(
                "membership Collection identity is not indexed",
            ))?;
        let (Some(kind), Some(source_path)) = (&owner.kind, &owner.source_path) else {
            return Err(DurableError::Conflict(
                "membership Collection identity is not indexed",
            ));
        };
        if kind != "metadata" || !safe_path(source_path) || !source_path.starts_with("ToS/") {
            return Err(DurableError::Conflict(
                "membership Collection identity owner differs",
            ));
        }
        add_exact_record_history_archives(context, source_path, exact, &mut paths)?;
    }
    paths_within_limit(paths)
}

fn claim_event_historical_capture_paths_for_requests(
    context: &DurableNavigationEventContextV1,
    requests: &[ClaimEventIdentityRequestV1],
    owners: &[ClaimEventIdentityOwnerV1],
) -> DurableResult<Vec<String>> {
    validate_identity_result_set(requests, owners)?;
    let endpoints = claim_event_historical_endpoint_paths_for_requests(context, requests, owners)?;
    claim_event_historical_capture_paths_for_endpoints(context, &endpoints)
}

fn claim_event_historical_capture_paths_for_endpoints(
    context: &DurableNavigationEventContextV1,
    endpoints: &[String],
) -> DurableResult<Vec<String>> {
    let mut paths = BTreeSet::new();
    for endpoint in endpoints {
        let home = endpoint
            .rsplit_once('/')
            .map(|(home, _)| home)
            .ok_or(DurableError::Corrupt("historical Claim endpoint home"))?;
        let request_path = sibling_path(&endpoint, "source-create-request.json")?;
        let Some(request_bytes) = selected_optional_bytes(context, &request_path)? else {
            continue;
        };
        let request = strict_value(request_bytes)?;
        let claims = request
            .get("claims")
            .and_then(Value::as_array)
            .filter(|claims| !claims.is_empty() && claims.len() <= 32)
            .ok_or(DurableError::Conflict(
                "historical Claim captured request member set differs",
            ))?;
        let mut ids = BTreeSet::new();
        for claim in claims {
            let id = text(claim, "claim_id")?;
            if !id.starts_with("tos.claim.") || id.len() > MAX_PATH_BYTES || !ids.insert(id) {
                return Err(DurableError::Conflict(
                    "historical Claim captured request identity differs",
                ));
            }
            let form_path = claim_form_path(&endpoint, id)?;
            add_path(&mut paths, &form_path)?;
            let form_name = form_path
                .rsplit_once('/')
                .map(|(_, name)| name)
                .ok_or(DurableError::Corrupt("historical Claim form basename"))?;
            add_path(&mut paths, &format!("{home}/.{form_name}.writer.lock"))?;
        }
    }
    paths_within_limit(paths)
}

fn claim_event_historical_endpoint_paths_for_requests(
    context: &DurableNavigationEventContextV1,
    requests: &[ClaimEventIdentityRequestV1],
    owners: &[ClaimEventIdentityOwnerV1],
) -> DurableResult<Vec<String>> {
    validate_identity_result_set(requests, owners)?;
    let mut paths = BTreeSet::new();
    for (request, owner) in requests.iter().zip(owners) {
        if owner.kind.as_deref() != Some("claim") {
            continue;
        }
        let Some(source_path) = owner.source_path.as_deref() else {
            continue;
        };
        if is_claim_stream_path(source_path) {
            continue;
        }
        if !is_historical_claim_stream_path(source_path) {
            return Err(DurableError::Conflict(
                "historical Claim endpoint owner path differs",
            ));
        }
        // The point index supplies only the address. The exact event context
        // must carry the bytes before a renderer can use the Claim visibility.
        let bytes = selected_bytes(context, source_path)?;
        if !parse_claim_stream(source_path, bytes)?
            .owners
            .iter()
            .any(|owner| owner.claim_id == request.id)
        {
            return Err(DurableError::Conflict(
                "historical Claim endpoint identity is absent",
            ));
        }
        add_path(&mut paths, source_path)?;
    }
    paths_within_limit(paths)
}

fn claim_event_identity_owner_archive_manifest_paths_for_requests(
    context: &DurableNavigationEventContextV1,
    requests: &[ClaimEventIdentityRequestV1],
    owners: &[ClaimEventIdentityOwnerV1],
) -> DurableResult<BTreeSet<String>> {
    validate_identity_result_set(requests, owners)?;
    let mut streams = BTreeMap::<String, ClaimEventOwnerStreamV1<'_>>::new();
    for owner in owners {
        if owner.kind.as_deref() != Some("claim") {
            continue;
        }
        let source_path = owner.source_path.as_deref().ok_or(DurableError::Conflict(
            "Claim identity owner path is absent",
        ))?;
        if !(is_claim_stream_path(source_path) || is_historical_claim_stream_path(source_path)) {
            return Err(DurableError::Conflict("Claim identity owner path differs"));
        }
        if !streams.contains_key(source_path) {
            let bytes = selected_bytes(context, source_path)?;
            streams.insert(
                source_path.to_owned(),
                parse_claim_stream(source_path, bytes)?,
            );
        }
    }
    let streams = streams.into_values().collect::<Vec<_>>();
    Ok(
        claim_event_archive_manifest_paths_for_streams(context, &streams)?
            .into_iter()
            .collect(),
    )
}

fn claim_event_indexed_paths_for_requests(
    context: &DurableNavigationEventContextV1,
    streams: &[ClaimEventOwnerStreamV1<'_>],
    requests: &[ClaimEventIdentityRequestV1],
    owners: &[ClaimEventIdentityOwnerV1],
) -> DurableResult<Vec<String>> {
    validate_event_context(context)?;
    validate_identity_result_set(requests, owners)?;
    let (collection_refs, membership_refs) =
        claim_event_collection_order_refs_for_streams(streams)?;
    let collection_ids = collection_refs.keys().cloned().collect::<BTreeSet<_>>();
    let membership_ids = membership_refs.keys().cloned().collect::<BTreeSet<_>>();
    let relation_registry = strict_value(selected_bytes(context, RELATION_REGISTRY)?)?;
    let mut profile_schema_paths = BTreeSet::new();
    for stream in streams {
        for owner in &stream.owners {
            let claim = strict_value(owner.exact_row_bytes)?;
            let predicate = text(&claim, "predicate")?;
            if let Some(profile) = relation_profile(&relation_registry, predicate)? {
                let schema_version = text(&claim, "schema_version")?;
                let matching = array(profile, "schemas")?
                    .iter()
                    .filter(|route| {
                        route.get("schema_version").and_then(Value::as_str) == Some(schema_version)
                    })
                    .collect::<Vec<_>>();
                if matching.len() != 1 {
                    return Err(DurableError::Conflict(
                        "Claim profile schema route is absent or ambiguous",
                    ));
                }
                add_path(&mut profile_schema_paths, text(matching[0], "schema_ref")?)?;
                for dependency in array(matching[0], "schema_dependencies")? {
                    add_path(
                        &mut profile_schema_paths,
                        dependency
                            .as_str()
                            .ok_or(DurableError::Corrupt("Claim profile schema dependency"))?,
                    )?;
                }
            }
        }
    }
    let mut paths = BTreeSet::new();
    for (request, owner) in requests.iter().zip(owners) {
        let (Some(kind), Some(source_path)) = (&owner.kind, &owner.source_path) else {
            if request.required {
                return Err(DurableError::Conflict(
                    "required Claim identity is not indexed",
                ));
            }
            continue;
        };
        if !safe_path(source_path) {
            return Err(DurableError::Conflict("Claim identity index path differs"));
        }
        match kind.as_str() {
            "metadata" => {
                if !source_path.ends_with(".json") {
                    return Err(DurableError::Conflict(
                        "Claim metadata identity path differs",
                    ));
                }
                add_path(&mut paths, source_path)?;
                add_path(&mut paths, &identity_forms_path(source_path)?)?;
                if collection_ids.contains(&request.id) {
                    add_path(
                        &mut paths,
                        &sibling_path(source_path, "source-revision-history.json")?,
                    )?;
                }
            }
            "claim" => {
                if is_claim_stream_path(source_path) {
                    add_path(&mut paths, source_path)?;
                    add_stream_support_paths(&mut paths, source_path)?;
                    add_claim_form_path(&mut paths, source_path, &request.id)?;
                } else if is_historical_claim_stream_path(source_path) {
                    add_path(&mut paths, source_path)?;
                    add_path(&mut paths, "ToS/contracts/historical-claim.schema.json")?;
                    add_path(
                        &mut paths,
                        &sibling_path(source_path, "claim-revision-history.json")?,
                    )?;
                    add_path(
                        &mut paths,
                        &sibling_path(source_path, "source-create-provenance.jsonl")?,
                    )?;
                    for leaf in [
                        "source-create-request.json",
                        "source-create-receipt.json",
                        "source-create-environment.json",
                    ] {
                        add_path(&mut paths, &sibling_path(source_path, leaf)?)?;
                    }
                    add_claim_form_path(&mut paths, source_path, &request.id)?;
                } else {
                    return Err(DurableError::Conflict(
                        "Claim identity index owner kind differs",
                    ));
                }
            }
            "event" | "form" => add_path(&mut paths, source_path)?,
            "anchor" => {
                add_path(&mut paths, source_path)?;
                add_path(&mut paths, ANCHOR_SCHEMA)?;
            }
            _ => return Err(DurableError::Conflict("Claim identity index kind differs")),
        }
        if membership_ids.contains(&request.id) && owner.kind.as_deref() != Some("claim") {
            return Err(DurableError::Conflict(
                "collection-order membership identity owner differs",
            ));
        }
    }
    for path in CLAIM_PROFILE_SUPPORT_SCHEMAS {
        add_path(&mut paths, path)?;
    }
    paths.extend(profile_schema_paths);
    if owners
        .iter()
        .any(|owner| owner.kind.as_deref() == Some("metadata"))
    {
        add_path(&mut paths, FORM_SCHEMA)?;
    }
    add_static_paths(&mut paths)?;
    paths_within_limit(paths)
}

/// Parse the exact terminal Claim streams selected by a multi-event range.
/// Unlike a single-event mutation manifest, the stream set can contain many
/// changed and reverse-indexed owners from different events in the range.
fn claim_event_streams_for_range_v1<'a>(
    context: &'a DurableNavigationEventContextV1,
    source_paths: &[String],
) -> DurableResult<Vec<ClaimEventOwnerStreamV1<'a>>> {
    validate_range_event_context(context)?;
    if source_paths.is_empty() || source_paths.len() > MAX_PATHS {
        return Err(DurableError::Invalid("Claim range stream path count"));
    }
    let mut streams = Vec::with_capacity(source_paths.len());
    let mut previous: Option<&str> = None;
    for source_path in source_paths {
        if !is_claim_stream_path(source_path)
            || previous.is_some_and(|previous| previous >= source_path.as_str())
        {
            return Err(DurableError::Invalid("Claim range stream path order"));
        }
        previous = Some(source_path);
        let bytes = selected_bytes(context, source_path)?;
        streams.push(parse_claim_stream(source_path, bytes)?);
    }
    Ok(streams)
}

/// Stage current Claim forms, direct evidence, and stream support files for
/// an exact already-selected range stream set.
pub(crate) fn claim_event_range_current_owner_paths_v1(
    context: &DurableNavigationEventContextV1,
    source_paths: &[String],
) -> DurableResult<Vec<String>> {
    let streams = claim_event_streams_for_range_v1(context, source_paths)?;
    let mut paths = BTreeSet::new();
    for stream in streams {
        add_stream_support_paths(&mut paths, &stream.source_path)?;
        for owner in stream.owners {
            add_claim_form_path(&mut paths, &stream.source_path, &owner.claim_id)?;
            add_direct_evidence_paths(&mut paths, owner.exact_row_bytes)?;
        }
    }
    // Identity resolution reads the relation registry in the next phase,
    // including ranges with no Record owner to supply shared inputs.
    add_static_paths(&mut paths)?;
    paths_within_limit(paths)
}

/// Request the complete stable-identity set for a selected terminal range
/// stream set.
pub(crate) fn claim_event_range_identity_requests_v1(
    context: &DurableNavigationEventContextV1,
    source_paths: &[String],
) -> DurableResult<Vec<ClaimEventIdentityRequestV1>> {
    let streams = claim_event_streams_for_range_v1(context, source_paths)?;
    claim_event_identity_requests_for_streams(context, &streams)
}

pub(crate) fn claim_event_range_indexed_paths_v1(
    context: &DurableNavigationEventContextV1,
    source_paths: &[String],
    owners: &[ClaimEventIdentityOwnerV1],
) -> DurableResult<Vec<String>> {
    let streams = claim_event_streams_for_range_v1(context, source_paths)?;
    let requests = claim_event_identity_requests_for_streams(context, &streams)?;
    claim_event_indexed_paths_for_requests(context, &streams, &requests, owners)
}

pub(crate) fn claim_event_range_schema_paths_v1(
    context: &DurableNavigationEventContextV1,
    source_paths: &[String],
    owners: &[ClaimEventIdentityOwnerV1],
) -> DurableResult<Vec<String>> {
    let streams = claim_event_streams_for_range_v1(context, source_paths)?;
    let requests = claim_event_identity_requests_for_streams(context, &streams)?;
    claim_event_indexed_schema_paths_for_requests(context, &requests, owners)
}

pub(crate) fn claim_event_range_historical_endpoint_paths_v1(
    context: &DurableNavigationEventContextV1,
    source_paths: &[String],
    owners: &[ClaimEventIdentityOwnerV1],
) -> DurableResult<Vec<String>> {
    let requests = claim_event_range_identity_requests_v1(context, source_paths)?;
    claim_event_historical_endpoint_paths_for_requests(context, &requests, owners)
}

pub(crate) fn claim_event_range_historical_capture_paths_v1(
    context: &DurableNavigationEventContextV1,
    source_paths: &[String],
    owners: &[ClaimEventIdentityOwnerV1],
) -> DurableResult<Vec<String>> {
    let requests = claim_event_range_identity_requests_v1(context, source_paths)?;
    claim_event_historical_capture_paths_for_requests(context, &requests, owners)
}

pub(crate) fn claim_event_range_archive_manifest_paths_v1(
    context: &DurableNavigationEventContextV1,
    source_paths: &[String],
) -> DurableResult<Vec<String>> {
    let streams = claim_event_streams_for_range_v1(context, source_paths)?;
    claim_event_archive_manifest_paths_for_streams(context, &streams)
}

pub(crate) fn claim_event_range_identity_archive_manifest_paths_v1(
    context: &DurableNavigationEventContextV1,
    source_paths: &[String],
    owners: &[ClaimEventIdentityOwnerV1],
) -> DurableResult<Vec<String>> {
    let streams = claim_event_streams_for_range_v1(context, source_paths)?;
    let requests = claim_event_identity_requests_for_streams(context, &streams)?;
    validate_identity_result_set(&requests, owners)?;
    let (collection_refs, membership_refs) =
        claim_event_collection_order_refs_for_streams(&streams)?;
    let mut paths = claim_event_identity_archive_paths_for_refs(
        context,
        owners,
        &collection_refs,
        &membership_refs,
    )?
    .into_iter()
    .collect::<BTreeSet<_>>();
    paths.extend(
        claim_event_identity_owner_archive_manifest_paths_for_requests(context, &requests, owners)?,
    );
    paths_within_limit(paths)
}

pub(crate) fn claim_event_range_archive_blob_paths_v1(
    context: &DurableNavigationEventContextV1,
    manifest_paths: &[String],
) -> DurableResult<Vec<String>> {
    claim_event_archive_blob_paths_for_manifests_v1(context, manifest_paths)
}

pub(crate) fn claim_event_range_identity_archive_blob_paths_v1(
    context: &DurableNavigationEventContextV1,
    source_paths: &[String],
    owners: &[ClaimEventIdentityOwnerV1],
) -> DurableResult<Vec<String>> {
    let manifests =
        claim_event_range_identity_archive_manifest_paths_v1(context, source_paths, owners)?;
    claim_event_archive_blob_paths_for_manifests_v1(context, &manifests)
}

fn claim_event_archive_blob_paths_for_manifests_v1(
    context: &DurableNavigationEventContextV1,
    manifests: &[String],
) -> DurableResult<Vec<String>> {
    validate_event_context(context)?;
    let mut paths = BTreeSet::new();
    for manifest_path in manifests {
        let manifest_bytes = selected_bytes(context, &manifest_path)?;
        let manifest = strict_value(manifest_bytes)?;
        if !matches!(
            manifest.get("schema_version").and_then(Value::as_str),
            Some("tos_source_package_archive_v1" | "tos_source_package_archive_v2")
        ) {
            return Err(DurableError::Conflict(
                "Claim archive manifest schema differs",
            ));
        }
        let archive_path = manifest_path
            .strip_suffix("/manifest.json")
            .ok_or(DurableError::Corrupt("Claim archive manifest path"))?;
        let files = object(&manifest, "files")?;
        if files.is_empty() || files.len() > MAX_ARCHIVE_FILES {
            return Err(DurableError::Refused("Claim archive member count"));
        }
        for (name, binding) in files {
            let lock = tos_compiler::source_bibliographic::public_form_lock(name);
            if !safe_leaf(name)
                && !(lock && manifest["schema_version"] == "tos_source_package_archive_v1")
            {
                return Err(DurableError::Conflict("Claim archive member name differs"));
            }
            if lock
                && (binding.get("bytes").and_then(Value::as_u64) != Some(0)
                    || text(binding, "sha256")? != Digest256::of_bytes(&[]).to_prefixed())
            {
                return Err(DurableError::Conflict(
                    "Claim archive writer lock is not empty",
                ));
            }
            let digest = text(binding, "sha256")?
                .strip_prefix("sha256:")
                .ok_or(DurableError::Corrupt("Claim archive member digest"))?;
            Digest256::from_hex(digest)
                .map_err(|_| DurableError::Corrupt("Claim archive member digest"))?;
            let blob = format!("{digest}.blob");
            if text(binding, "blob")? != blob {
                return Err(DurableError::Conflict("Claim archive blob locator differs"));
            }
            add_path(&mut paths, &format!("{archive_path}/{blob}"))?;
        }
    }
    paths_within_limit(paths)
}

fn claim_event_identity_requests_for_streams(
    context: &DurableNavigationEventContextV1,
    streams: &[ClaimEventOwnerStreamV1<'_>],
) -> DurableResult<Vec<ClaimEventIdentityRequestV1>> {
    let relation_registry = strict_value(selected_bytes(context, RELATION_REGISTRY)?)?;
    let mut required = BTreeSet::new();
    let mut optional = BTreeSet::new();
    for stream in streams {
        for owner in &stream.owners {
            let claim = strict_value(owner.exact_row_bytes)?;
            required.insert(text(&claim, "subject_ref")?.to_owned());
            if let Some(object) = claim.get("object").and_then(Value::as_str) {
                if object.starts_with("tos.") {
                    required.insert(object.to_owned());
                }
            }
            if let Some(exact) = claim.pointer("/object/collection_version") {
                required.insert(exact_reference(exact, "tos.collection.")?.id);
            }
            if let Some(versions) = claim.pointer("/object/membership_versions") {
                let versions = versions
                    .as_array()
                    .ok_or(DurableError::Corrupt("Claim membership version list"))?;
                if versions.is_empty() || versions.len() > 128 {
                    return Err(DurableError::Refused("Claim membership version count"));
                }
                for exact in versions {
                    required.insert(exact_reference(exact, "tos.claim.")?.id);
                }
            }
            if let Some(maker) = claim.pointer("/maker/agent_ref").and_then(Value::as_str) {
                optional.insert(maker.to_owned());
            }
            for field in ["evidence_refs", "counterevidence_refs"] {
                if let Some(items) = claim.get(field).and_then(Value::as_array) {
                    for item in items {
                        let reference = item
                            .as_str()
                            .ok_or(DurableError::Corrupt("Claim evidence reference type"))?;
                        if reference.starts_with("tos.") {
                            optional.insert(reference.to_owned());
                        }
                    }
                }
            }
            for alternative in claim
                .get("alternative_claim_refs")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let reference = alternative
                    .as_str()
                    .ok_or(DurableError::Corrupt("Claim alternative reference type"))?;
                required.insert(reference.to_owned());
            }
            if let Some(reference) = claim.get("supersedes_claim_ref").and_then(Value::as_str) {
                required.insert(reference.to_owned());
            }
            let predicate = text(&claim, "predicate")?;
            if let Some(profile) = relation_profile(&relation_registry, predicate)? {
                for member in tos_compiler::source_bibliographic::supplied_claim_reference_members(
                    &claim,
                    profile,
                    MAX_CLAIM_LINE_BYTES,
                )
                .map_err(|_| DurableError::Corrupt("Claim profile member resolution failed"))?
                {
                    required.insert(member.to_owned());
                }
            }
            add_normalized_identity_refs(&claim, &mut required, &mut optional)?;
        }
    }
    optional.retain(|id| !required.contains(id));
    let mut requests = required
        .into_iter()
        .map(|id| ClaimEventIdentityRequestV1 { id, required: true })
        .chain(
            optional
                .into_iter()
                .filter(|id| id.starts_with("tos."))
                .map(|id| ClaimEventIdentityRequestV1 {
                    id,
                    required: false,
                }),
        )
        .collect::<Vec<_>>();
    requests.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(requests)
}

fn claim_event_collection_order_refs_for_streams(
    streams: &[ClaimEventOwnerStreamV1<'_>],
) -> DurableResult<(
    BTreeMap<String, ExactSourceReferenceV1>,
    BTreeMap<String, ExactSourceReferenceV1>,
)> {
    let mut collections = BTreeMap::new();
    let mut memberships = BTreeMap::new();
    for stream in streams {
        for owner in &stream.owners {
            let claim = strict_value(owner.exact_row_bytes)?;
            let Some(object) = claim.get("object").filter(|value| value.is_object()) else {
                continue;
            };
            if let Some(reference) = object.get("collection_version") {
                let reference = exact_reference(reference, "tos.collection.")?;
                insert_exact_reference(&mut collections, reference)?;
            }
            if let Some(references) = object.get("membership_versions") {
                let references = references
                    .as_array()
                    .ok_or(DurableError::Corrupt("Claim membership version list"))?;
                if references.is_empty() || references.len() > 128 {
                    return Err(DurableError::Refused("Claim membership version count"));
                }
                for reference in references {
                    let reference = exact_reference(reference, "tos.claim.")?;
                    insert_exact_reference(&mut memberships, reference)?;
                }
            }
        }
    }
    Ok((collections, memberships))
}

fn insert_exact_reference(
    references: &mut BTreeMap<String, ExactSourceReferenceV1>,
    reference: ExactSourceReferenceV1,
) -> DurableResult<()> {
    if references
        .insert(reference.id.clone(), reference.clone())
        .is_some_and(|previous| previous != reference)
    {
        return Err(DurableError::Conflict(
            "collection-order Claim repeats an identity at different versions",
        ));
    }
    Ok(())
}

fn exact_reference(value: &Value, expected_prefix: &str) -> DurableResult<ExactSourceReferenceV1> {
    let id = text(value, "id")?.to_owned();
    let version = value
        .get("version")
        .and_then(Value::as_u64)
        .filter(|version| (1..=9_007_199_254_740_991).contains(version))
        .ok_or(DurableError::Corrupt("Claim exact reference version"))?;
    let digest = text(value, "digest")?
        .strip_prefix("sha256:")
        .filter(|digest| {
            digest.len() == 64
                && digest
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
        .ok_or(DurableError::Corrupt("Claim exact reference digest"))?
        .to_owned();
    Digest256::from_hex(&digest)
        .map_err(|_| DurableError::Corrupt("Claim exact reference digest"))?;
    if !id.starts_with(expected_prefix) || id.len() > MAX_PATH_BYTES {
        return Err(DurableError::Conflict("Claim exact reference kind differs"));
    }
    Ok(ExactSourceReferenceV1 {
        id,
        version,
        digest,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExactSourceReferenceV1 {
    pub id: String,
    pub version: u64,
    pub digest: String,
}

fn add_exact_claim_history_archives(
    context: &DurableNavigationEventContextV1,
    source_path: &str,
    exact: &ExactSourceReferenceV1,
    paths: &mut BTreeSet<String>,
) -> DurableResult<()> {
    let stream_bytes = selected_bytes(context, source_path)?;
    let stream = parse_claim_stream(source_path, stream_bytes)?;
    let current = stream
        .owners
        .iter()
        .find(|owner| owner.claim_id == exact.id)
        .ok_or(DurableError::Conflict(
            "membership Claim owner row is absent",
        ))?;
    let current_value = strict_value(current.exact_row_bytes)?;
    let history_path = sibling_path(source_path, "claim-revision-history.json")?;
    let current_matches = exact_value_matches(&current_value, "claim_id", "claim_version", exact)?;
    let Some(history_bytes) = selected_optional_bytes(context, &history_path)? else {
        if current_matches && current_value["claim_version"] == 1 {
            return Ok(());
        }
        return Err(DurableError::Conflict(
            "membership Claim retained history is absent",
        ));
    };
    let history = strict_value(history_bytes)?;
    if text(&history, "schema_version")? != "tos_claim_revision_history_v1"
        || text(&history, "source_path")? != source_path
    {
        return Err(DurableError::Conflict(
            "membership Claim history owner differs",
        ));
    }
    let receipts = array(&history, "receipts")?;
    if receipts.len() > MAX_ARCHIVES {
        return Err(DurableError::Refused(
            "membership Claim history archive count",
        ));
    }
    if receipts.is_empty() {
        if current_matches && current_value["claim_version"] == 1 {
            return Ok(());
        }
        return Err(DurableError::Conflict("membership Claim history is absent"));
    }
    let mut found = current_matches;
    let mut commands = BTreeSet::new();
    let mut latest = BTreeMap::<String, ExactSourceReferenceV1>::new();
    for receipt in receipts {
        let previous = exact_reference(
            receipt
                .get("previous_source")
                .ok_or(DurableError::Corrupt("Claim history predecessor absent"))?,
            "tos.claim.",
        )?;
        let successor = exact_reference(
            receipt
                .get("source")
                .ok_or(DurableError::Corrupt("Claim history successor absent"))?,
            "tos.claim.",
        )?;
        if previous.id != successor.id
            || previous.version.checked_add(1) != Some(successor.version)
            || !commands.insert(text(receipt, "command_id")?.to_owned())
        {
            return Err(DurableError::Conflict(
                "Claim history revision sequence differs",
            ));
        }
        if let Some(head) = latest.get(&previous.id) {
            if head != &previous {
                return Err(DurableError::Conflict(
                    "Claim history revision chain differs",
                ));
            }
        } else if previous.version != 1 {
            return Err(DurableError::Conflict(
                "Claim history initial version differs",
            ));
        }
        found |= previous == *exact;
        let revision = text(receipt, "previous_revision")?
            .strip_prefix("sha256:")
            .filter(|revision| revision.len() == 64)
            .ok_or(DurableError::Corrupt("Claim history revision digest"))?;
        Digest256::from_hex(revision)
            .map_err(|_| DurableError::Corrupt("Claim history revision digest"))?;
        let archive = format!(
            "ToS/source-witnesses/.record-revisions/{}-{revision}",
            Digest256::of_bytes(previous.id.as_bytes()).to_hex()
        );
        if text(receipt, "archive_path")? != archive {
            return Err(DurableError::Conflict(
                "Claim history archive locator differs",
            ));
        }
        add_path(paths, &format!("{archive}/manifest.json"))?;
        latest.insert(successor.id.clone(), successor);
    }
    if !found {
        return Err(DurableError::Conflict(
            "membership Claim exact revision is not retained by its current history",
        ));
    }
    let current_ref = exact_source_reference(&current_value, "claim_id", "claim_version")?;
    match latest.get(&exact.id) {
        Some(head) if head == &current_ref => {}
        None if current_ref.version == 1 => {}
        _ => {
            return Err(DurableError::Conflict(
                "membership Claim current history head differs",
            ));
        }
    }
    Ok(())
}

fn add_exact_record_history_archives(
    context: &DurableNavigationEventContextV1,
    source_path: &str,
    exact: &ExactSourceReferenceV1,
    paths: &mut BTreeSet<String>,
) -> DurableResult<()> {
    let current = strict_value(selected_bytes(context, source_path)?)?;
    let history_path = sibling_path(source_path, "source-revision-history.json")?;
    let current_matches = exact_value_matches(&current, "record_id", "record_version", exact)?;
    let Some(history_bytes) = selected_optional_bytes(context, &history_path)? else {
        if current_matches && current["record_version"] == 1 {
            return Ok(());
        }
        return Err(DurableError::Conflict(
            "membership Collection retained history is absent",
        ));
    };
    let history = strict_value(history_bytes)?;
    if !matches!(
        text(&history, "schema_version")?,
        "tos_source_revision_history_v1" | "tos_source_revision_history_v2"
    ) || text(&history, "record_id")? != exact.id
    {
        return Err(DurableError::Conflict(
            "membership Collection history owner differs",
        ));
    }
    let receipts = array(&history, "receipts")?;
    if receipts.len() > MAX_ARCHIVES {
        return Err(DurableError::Refused(
            "membership Collection history archive count",
        ));
    }
    if receipts.is_empty() {
        if current_matches && current["record_version"] == 1 {
            return Ok(());
        }
        return Err(DurableError::Conflict(
            "membership Collection history is absent",
        ));
    }
    let current_ref = exact_source_reference(&current, "record_id", "record_version")?;
    let mut found = current_matches;
    let mut head: Option<ExactSourceReferenceV1> = None;
    for receipt in receipts {
        let previous = receipt.get("previous_source").ok_or(DurableError::Corrupt(
            "Collection history predecessor absent",
        ))?;
        let previous_ref = exact_reference(previous, "tos.collection.")?;
        let successor_ref = exact_reference(
            receipt
                .get("source")
                .ok_or(DurableError::Corrupt("Collection history successor absent"))?,
            "tos.collection.",
        )?;
        if previous_ref.id != successor_ref.id
            || previous_ref.version.checked_add(1) != Some(successor_ref.version)
            || head.as_ref().is_some_and(|prior| prior != &previous_ref)
            || head.is_none() && previous_ref.version != 1
        {
            return Err(DurableError::Conflict(
                "Collection history revision sequence differs",
            ));
        }
        found |= previous_ref == *exact;
        let revision = text(receipt, "previous_revision")?
            .strip_prefix("sha256:")
            .ok_or(DurableError::Corrupt("Collection history revision digest"))?;
        Digest256::from_hex(revision)
            .map_err(|_| DurableError::Corrupt("Collection history revision digest"))?;
        let archive = format!(
            "ToS/source-witnesses/.record-revisions/{}-{revision}",
            Digest256::of_bytes(exact.id.as_bytes()).to_hex()
        );
        if text(receipt, "archive_path")? != archive {
            return Err(DurableError::Conflict(
                "Collection history archive locator differs",
            ));
        }
        add_path(paths, &format!("{archive}/manifest.json"))?;
        head = Some(successor_ref);
    }
    if !found {
        return Err(DurableError::Conflict(
            "membership Collection exact revision is not retained",
        ));
    }
    if let Some(head) = head
        && head != current_ref
    {
        return Err(DurableError::Conflict(
            "membership Collection current history head differs",
        ));
    } else if current_ref.version != 1 && !current_matches && receipts.is_empty() {
        return Err(DurableError::Conflict(
            "membership Collection current history head differs",
        ));
    }
    Ok(())
}

fn exact_source_reference(
    value: &Value,
    id_field: &str,
    version_field: &str,
) -> DurableResult<ExactSourceReferenceV1> {
    let id = value
        .get(id_field)
        .and_then(Value::as_str)
        .ok_or(DurableError::Corrupt("selected exact source identity"))?;
    let version = value
        .get(version_field)
        .and_then(Value::as_u64)
        .filter(|version| *version > 0)
        .ok_or(DurableError::Corrupt("selected exact source version"))?;
    let canonical = canonical_serde_source_record_v1(value, MAX_EXACT_RECORD_BYTES)?;
    Ok(ExactSourceReferenceV1 {
        id: id.to_owned(),
        version,
        digest: Digest256::of_bytes(&canonical).to_hex(),
    })
}

fn exact_value_matches(
    value: &Value,
    id_field: &str,
    version_field: &str,
    exact: &ExactSourceReferenceV1,
) -> DurableResult<bool> {
    let id = value
        .get(id_field)
        .and_then(Value::as_str)
        .ok_or(DurableError::Corrupt("selected exact reference identity"))?;
    let version = value
        .get(version_field)
        .and_then(Value::as_u64)
        .ok_or(DurableError::Corrupt("selected exact reference version"))?;
    let canonical = canonical_serde_source_record_v1(value, MAX_EXACT_RECORD_BYTES)?;
    Ok(id == exact.id
        && version == exact.version
        && Digest256::of_bytes(&canonical).to_hex() == exact.digest)
}

/// Canonicalize a serde projection through the shared strict JSON grammar and
/// the owner's SourceRecordDigest profile, rather than hashing serde's wire
/// spelling directly.
pub(crate) fn canonical_serde_source_record_v1(
    value: &Value,
    max_bytes: usize,
) -> DurableResult<Vec<u8>> {
    let limits = JsonLimits::new(max_bytes, 96, 1_000_000, 4096)
        .map_err(|_| DurableError::Refused("Claim exact digest limits"))?;
    let raw = serde_json::to_vec(value)
        .map_err(|_| DurableError::Corrupt("selected exact JSON serialization"))?;
    if raw.is_empty() || raw.len() > limits.max_bytes {
        return Err(DurableError::Refused("Claim exact digest byte budget"));
    }
    let document = parse_json(&raw, JsonMode::PublishedStrict, limits)
        .map_err(|_| DurableError::Corrupt("selected exact JSON parsing"))?;
    canonical_bytes_v1(
        document.root(),
        CanonicalProfile::SourceRecordDigestV1,
        limits,
    )
    .map_err(|_| DurableError::Corrupt("selected exact canonical digest"))
}

fn claim_event_indexed_schema_paths_for_requests(
    context: &DurableNavigationEventContextV1,
    requests: &[ClaimEventIdentityRequestV1],
    owners: &[ClaimEventIdentityOwnerV1],
) -> DurableResult<Vec<String>> {
    validate_event_context(context)?;
    validate_identity_result_set(requests, owners)?;
    let mut metadata = Vec::new();
    for (request, owner) in requests.iter().zip(owners) {
        if owner.kind.as_deref() != Some("metadata") {
            continue;
        }
        let source_path = owner.source_path.as_deref().ok_or(DurableError::Conflict(
            "Claim metadata identity source path is absent",
        ))?;
        let record = strict_value(selected_bytes(context, source_path)?)?;
        let record_id = record
            .get("record_id")
            .or_else(|| record.get("artifact_id"))
            .or_else(|| record.get("composite_id"))
            .and_then(Value::as_str);
        if record_id != Some(request.id.as_str()) {
            return Err(DurableError::Conflict(
                "Claim metadata identity source row differs",
            ));
        }
        let record_type = record
            .get("record_type")
            .and_then(Value::as_str)
            .or_else(|| {
                if record.get("artifact_id").is_some() {
                    Some("artifact")
                } else if record.get("composite_id").is_some() {
                    Some("composite")
                } else {
                    None
                }
            })
            .ok_or(DurableError::Corrupt("Claim metadata record type absent"))?;
        metadata.push((record_type.to_owned(), source_path.to_owned(), record));
    }
    if metadata.is_empty() {
        return Ok(Vec::new());
    }
    let entities = strict_value(selected_bytes(context, ENTITY_REGISTRY)?)?;
    let mut paths = BTreeSet::new();
    for (record_type, source_path, record) in metadata {
        let schema_path = tos_compiler::selected_metadata_record_schema_path_v1(
            &record_type,
            &source_path,
            &record,
            &entities,
        )
        .map_err(|_| DurableError::Conflict("Claim metadata schema route differs"))?;
        add_path(&mut paths, &schema_path)?;
    }
    paths_within_limit(paths)
}

fn validate_event_context(context: &DurableNavigationEventContextV1) -> DurableResult<()> {
    if context.event_id().is_empty()
        || context.event_id().len() > 256
        || context.event_sequence() == 0
        || context.event_sequence() > context.source_head_sequence()
        || context.consumer_cursor().sequence >= context.event_sequence()
        || !matches!(context.event_kind(), "command" | "rights" | "rule")
    {
        return Err(DurableError::Conflict(
            "Claim navigation event or range context differs",
        ));
    }
    Ok(())
}

fn validate_range_event_context(context: &DurableNavigationEventContextV1) -> DurableResult<()> {
    validate_event_context(context)
}

fn selected_source<'a>(
    context: &'a DurableNavigationEventContextV1,
    path: &str,
) -> Option<&'a crate::DurableNavigationEventSourceV1> {
    context
        .selected_sources()
        .iter()
        .find(|source| source.source_path == path)
}

fn exact_selected_bytes<'a>(
    source: &'a crate::DurableNavigationEventSourceV1,
) -> DurableResult<&'a [u8]> {
    let bytes = source
        .exact_source_bytes
        .as_deref()
        .filter(|_| source.is_source)
        .ok_or(DurableError::Conflict("selected Claim source bytes absent"))?;
    if source.content_digest != Some(Digest256::of_bytes(bytes))
        || source.content_length != Some(bytes.len() as u64)
    {
        return Err(DurableError::Conflict(
            "selected Claim source digest differs",
        ));
    }
    Ok(bytes)
}

fn selected_bytes<'a>(
    context: &'a DurableNavigationEventContextV1,
    path: &str,
) -> DurableResult<&'a [u8]> {
    let source = selected_source(context, path).ok_or(DurableError::Conflict(
        "required Claim dependency is not selected",
    ))?;
    exact_selected_bytes(source)
}

fn selected_optional_bytes<'a>(
    context: &'a DurableNavigationEventContextV1,
    path: &str,
) -> DurableResult<Option<&'a [u8]>> {
    let Some(source) = selected_source(context, path) else {
        return Err(DurableError::Conflict(
            "optional Claim dependency was not selected",
        ));
    };
    if !source.is_source {
        if source.exact_source_bytes.is_some()
            || source.content_digest.is_some()
            || source.content_length.is_some()
        {
            return Err(DurableError::Conflict(
                "Claim absence witness carries bytes",
            ));
        }
        return Ok(None);
    }
    exact_selected_bytes(source).map(Some)
}

fn is_claim_stream_path(path: &str) -> bool {
    crate::source_claim_dependency_index::is_claim_stream_path(path) && safe_path(path)
}

fn is_historical_claim_stream_path(path: &str) -> bool {
    let parts = path.split('/').collect::<Vec<_>>();
    parts.len() >= 5
        && parts[..3] == ["ToS", "source-witnesses", "history"]
        && parts.last() == Some(&"historical-claims.jsonl")
        && safe_path(path)
        && parts[3..parts.len() - 1].iter().all(|part| {
            !part.is_empty()
                && part.split(['.', '-']).all(|component| {
                    !component.is_empty()
                        && component
                            .bytes()
                            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
                })
        })
}

fn parse_claim_stream<'a>(
    path: &str,
    bytes: &'a [u8],
) -> DurableResult<ClaimEventOwnerStreamV1<'a>> {
    if !(is_claim_stream_path(path) || is_historical_claim_stream_path(path))
        || bytes.is_empty()
        || bytes.len() > MAX_CLAIM_STREAM_BYTES
    {
        return Err(DurableError::Refused("Claim stream path or size bound"));
    }
    let mut rows = BTreeMap::<String, (u64, &'a [u8])>::new();
    for (index, raw_line) in bytes.split(|byte| *byte == b'\n').enumerate() {
        if blank(raw_line) {
            continue;
        }
        if raw_line.len() > MAX_CLAIM_LINE_BYTES {
            return Err(DurableError::Refused("Claim stream line bound"));
        }
        let line = raw_line.strip_suffix(b"\r").unwrap_or(raw_line);
        let row = strict_value(line)?;
        let id = text(&row, "claim_id")?;
        if id.len() > MAX_PATH_BYTES || !id.starts_with("tos.claim.") {
            return Err(DurableError::Corrupt("Claim stream identity shape"));
        }
        if rows
            .insert(id.to_owned(), (index as u64 + 1, line))
            .is_some()
        {
            return Err(DurableError::Conflict("duplicate Claim stream identity"));
        }
        if rows.len() > MAX_CLAIM_OWNERS {
            return Err(DurableError::Refused("Claim stream owner count"));
        }
    }
    if rows.is_empty() {
        return Err(DurableError::Conflict("Claim stream has no owner rows"));
    }
    Ok(ClaimEventOwnerStreamV1 {
        source_path: path.to_owned(),
        exact_stream_bytes: bytes,
        owners: rows
            .into_iter()
            .map(
                |(claim_id, (source_line, exact_row_bytes))| ClaimEventOwnerRowV1 {
                    claim_id,
                    source_line,
                    exact_row_bytes,
                },
            )
            .collect(),
    })
}

fn strict_value(raw: &[u8]) -> DurableResult<Value> {
    let parsed: JsonValue = crate::source_command::parse(raw).map_err(DurableError::Source)?;
    let canonical = crate::source_command::canonical(&parsed).map_err(DurableError::Source)?;
    serde_json::from_slice(&canonical)
        .map_err(|_| DurableError::Corrupt("Claim selected JSON value"))
}

fn text<'a>(value: &'a Value, key: &'static str) -> DurableResult<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or(DurableError::Corrupt("Claim selected JSON field"))
}

fn object<'a>(
    value: &'a Value,
    key: &'static str,
) -> DurableResult<&'a serde_json::Map<String, Value>> {
    value
        .get(key)
        .and_then(Value::as_object)
        .ok_or(DurableError::Corrupt("Claim selected JSON object"))
}

fn array<'a>(value: &'a Value, key: &'static str) -> DurableResult<&'a [Value]> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or(DurableError::Corrupt("Claim selected JSON array"))
}

pub(crate) fn relation_profile<'a>(
    registry: &'a Value,
    predicate: &str,
) -> DurableResult<Option<&'a Value>> {
    let relations = array(registry, "relations")?;
    let mut found = None;
    for relation in relations {
        let mappings = relation
            .get("source_mappings")
            .and_then(Value::as_array)
            .into_iter()
            .flatten();
        if mappings.into_iter().any(|mapping| {
            mapping.get("source_graph").and_then(Value::as_str) == Some("source-claims")
                && mapping.get("source_predicate_id").and_then(Value::as_str) == Some(predicate)
                && mapping.get("scope").and_then(Value::as_str) == Some("claim-predicate")
        }) {
            if found.is_some() {
                return Err(DurableError::Conflict(
                    "Claim relation profile is ambiguous",
                ));
            }
            found = relation.get("source_claim_profile");
        }
    }
    Ok(found)
}

fn add_normalized_identity_refs(
    claim: &Value,
    required: &mut BTreeSet<String>,
    all: &mut BTreeSet<String>,
) -> DurableResult<()> {
    if claim.get("predicate").and_then(Value::as_str) == Some("provision_activity") {
        for (group, field) in [
            ("places", "normalized_place_ref"),
            ("agents", "normalized_agent_ref"),
        ] {
            if let Some(items) = claim
                .pointer(&format!("/object/{group}"))
                .and_then(Value::as_array)
            {
                for item in items {
                    if let Some(id) = item.get(field).and_then(Value::as_str) {
                        required.insert(id.to_owned());
                        all.insert(id.to_owned());
                    }
                }
            }
        }
    }
    if let Some(id) = claim
        .pointer("/object/relative/anchor_ref")
        .and_then(Value::as_str)
    {
        required.insert(id.to_owned());
        all.insert(id.to_owned());
    }
    Ok(())
}

fn add_static_paths(paths: &mut BTreeSet<String>) -> DurableResult<()> {
    for path in [
        ENTITY_REGISTRY,
        ENTITY_SCHEMA,
        RELATION_REGISTRY,
        RELATION_SCHEMA,
        CLAIM_SCHEMA,
        FORM_SCHEMA,
        PROVENANCE_SCHEMA,
        PROVENANCE_V1_SCHEMA,
    ] {
        add_path(paths, path)?;
    }
    Ok(())
}

fn add_stream_support_paths(paths: &mut BTreeSet<String>, source_path: &str) -> DurableResult<()> {
    for leaf in [
        "claim-revision-history.json",
        "source-create-provenance.jsonl",
        "source-create-receipt.json",
    ] {
        add_path(paths, &sibling_path(source_path, leaf)?)?;
    }
    Ok(())
}

fn add_claim_form_path(
    paths: &mut BTreeSet<String>,
    source_path: &str,
    claim_id: &str,
) -> DurableResult<()> {
    add_path(paths, &claim_form_path(source_path, claim_id)?)
}

fn claim_form_path(source_path: &str, claim_id: &str) -> DurableResult<String> {
    let stem = source_path
        .strip_suffix(".jsonl")
        .ok_or(DurableError::Corrupt("Claim stream suffix"))?;
    Ok(format!(
        "{stem}.{}.human-forms.json",
        Digest256::of_bytes(claim_id.as_bytes()).to_hex()
    ))
}

fn add_direct_evidence_paths(paths: &mut BTreeSet<String>, raw: &[u8]) -> DurableResult<()> {
    let claim = strict_value(raw)?;
    for field in ["evidence_refs", "counterevidence_refs"] {
        if let Some(items) = claim.get(field).and_then(Value::as_array) {
            for item in items {
                let reference = item
                    .as_str()
                    .ok_or(DurableError::Corrupt("Claim evidence reference"))?;
                if reference.starts_with("ToS/") {
                    add_path(paths, reference)?;
                }
            }
        }
    }
    Ok(())
}

fn sibling_path(source_path: &str, leaf: &str) -> DurableResult<String> {
    let parent = source_path
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .ok_or(DurableError::Corrupt("Claim source owner directory"))?;
    let path = format!("{parent}/{leaf}");
    if !safe_path(&path) {
        return Err(DurableError::Corrupt("Claim sibling path"));
    }
    Ok(path)
}

fn identity_forms_path(source_path: &str) -> DurableResult<String> {
    let stem = source_path
        .strip_suffix(".json")
        .ok_or(DurableError::Corrupt("Claim identity source extension"))?;
    let path = format!("{stem}.human-forms.json");
    if !safe_path(&path) {
        return Err(DurableError::Corrupt("Claim identity form path"));
    }
    Ok(path)
}

fn safe_path(path: &str) -> bool {
    let path_depth = path.split('/').count();
    path.len() <= MAX_PATH_BYTES
        && path.starts_with("ToS/")
        && !path
            .chars()
            .any(|character| matches!(character, '\\' | '\0'))
        && !path.split('/').enumerate().any(|(index, part)| {
            part.is_empty()
                || part.starts_with('.')
                    && part != ".record-revisions"
                    && !(index + 1 == path_depth && is_claim_form_writer_lock(part))
                || matches!(
                    part,
                    ".." | "payload" | "local-content" | "owner-local" | "private"
                )
        })
        && RelativePath::parse(path).is_ok_and(|parsed| parsed.as_str() == path)
}

fn is_claim_form_writer_lock(name: &str) -> bool {
    name.strip_prefix('.')
        .and_then(|name| name.strip_suffix(".human-forms.json.writer.lock"))
        .is_some_and(|digest| {
            digest.len() == 64
                && digest
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
}

fn safe_leaf(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_PATH_BYTES
        && !name
            .chars()
            .any(|character| matches!(character, '/' | '\\' | '\0'))
        && !name.starts_with('.')
        && !matches!(
            name,
            ".." | "payload" | "owner-local" | "private" | "catalog"
        )
}

fn add_path(paths: &mut BTreeSet<String>, path: &str) -> DurableResult<()> {
    if !safe_path(path) {
        return Err(DurableError::Conflict(
            "Claim dependency path is not canonical public ToS",
        ));
    }
    paths.insert(path.to_owned());
    if paths.len() > MAX_PATHS {
        return Err(DurableError::Refused("Claim selected path count"));
    }
    Ok(())
}

fn paths_within_limit(paths: BTreeSet<String>) -> DurableResult<Vec<String>> {
    if paths.len() > MAX_PATHS {
        return Err(DurableError::Refused("Claim selected path count"));
    }
    Ok(paths.into_iter().collect())
}

fn blank(line: &[u8]) -> bool {
    line.iter()
        .all(|byte| matches!(*byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c))
}
