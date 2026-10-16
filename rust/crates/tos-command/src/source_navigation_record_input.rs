//! Exact selected-file assembler for one bounded navigation Record cohort.
//!
//! This module derives only the direct and persisted affected Record owner
//! closure, then resolves each owner's maintained Record history from the
//! caller's final PG-selected file set. It never reads the filesystem or
//! manufactures a whole-corpus source revision.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use tos_compiler::{
    SelectedBibliographicFilesV1, resolve_selected_metadata_archive_forms_v1,
    resolve_selected_metadata_history_v1, selected_metadata_archive_forms_blob_path_v1,
    selected_metadata_archive_manifest_paths_v1, selected_metadata_archive_record_blob_path_v1,
    selected_metadata_record_schema_path_v1,
    source_bibliographic::{BibliographicForms, BibliographicLimits},
    source_navigation_event::{
        NavigationDependencyRoleV1, NavigationDependencyStateV1, NavigationDependencyV1,
        NavigationRecordEventInputV1, NavigationVersionSourceInputV1,
    },
    source_witness_catalog::{
        CandidateEventSourceIdentityV1, SourceCatalogValidator, render_catalog_record,
    },
};
use tos_foundation::{
    CanonicalProfile, Digest256, JsonLimits, RelativePath, canonical_raw_bytes_v1,
};

use crate::{
    DurableError, DurableNavigationEventContextV1, DurableResult,
    source_navigation_producer::{NavigationEventSourceCutIdentityV1, compiler_refusal},
};

const MAX_PATHS: usize = 4096;
const MAX_RECORD_BYTES: usize = 2 * 1024 * 1024;
const EVENT_SCHEMA: &str = "ToS/contracts/provenance-event-v2.schema.json";
const EVENT_HOME_FILE: &str = "source-create-provenance.jsonl";
const RECEIPT_HOME_FILE: &str = "source-create-receipt.json";
const REGISTRY: &str = "ToS/doctrine/semantic-interchange/entity-types.v1.json";
const REGISTRY_SCHEMA: &str = "ToS/contracts/semantic-entity-type-registry.schema.json";
const FORMS_SCHEMA: &str = "ToS/contracts/human-form-set.schema.json";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RecordOwnerOriginV1 {
    /// The command carries the exact initial source-create receipt, event, and
    /// Record bytes as committed members.
    DirectSourceCreate,
    /// The bounded PG reverse lookup found a previously projected Record.
    PersistedAffected,
}

/// Owner tuple consumed by the compiler builder. This local type can also
/// represent a direct creation, whose owner is absent from the old projection
/// index and therefore has no persisted affected-owner row yet.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RecordOwnerV1 {
    pub(crate) source_path: String,
    pub(crate) record_id: String,
    pub(crate) origin: RecordOwnerOriginV1,
}

/// Sorted owner closure and exact addresses to select before resolving the
/// Record history. `owners` is the union of receipt-proven direct creations
/// and persisted reverse-lookup owners; `paths` also includes the CMD seed
/// members and configured CMD event source used to identify this event.
#[derive(Clone, Debug)]
pub(crate) struct RecordEventPathPlanV1 {
    pub(crate) owners: Vec<RecordOwnerV1>,
    pub(crate) paths: Vec<String>,
}

/// Receipt-checked Record creation from one exact historical range event.
/// The original bytes are retained so the range compiler can bind the same
/// source, receipt, and authored provenance event instead of consulting only
/// the terminal files.
#[derive(Clone, Debug)]
pub(crate) struct RecordRangeSourceCreateV1 {
    pub(crate) owner: RecordOwnerV1,
    pub(crate) event_sequence: u64,
    pub(crate) event_id: String,
    pub(crate) command_id: String,
    pub(crate) source_path: String,
    pub(crate) source_sha256: Digest256,
    pub(crate) source_bytes: Vec<u8>,
    /// Optional human-form bytes as they existed at creation time. The current
    /// terminal forms file may have changed since, so a range proof keeps the
    /// historical member and binds it through the same create receipt/event.
    pub(crate) forms_path: Option<String>,
    pub(crate) forms_sha256: Option<Digest256>,
    pub(crate) forms_bytes: Option<Vec<u8>>,
    pub(crate) receipt_path: String,
    pub(crate) receipt_sha256: Digest256,
    pub(crate) receipt_bytes: Vec<u8>,
    pub(crate) provenance_event_path: String,
    pub(crate) provenance_event_sha256: Digest256,
    pub(crate) provenance_event_bytes: Vec<u8>,
}

/// One exact source-revision history event carried in the bounded range.
/// The historical bytes stay attached to the event that committed them; the
/// terminal Record source is selected separately for rendering.
#[derive(Clone, Debug)]
pub(crate) struct RecordRangeRevisionV1 {
    pub(crate) owner: RecordOwnerV1,
    pub(crate) event_sequence: u64,
    pub(crate) event_id: String,
    pub(crate) command_id: String,
    pub(crate) source_path: String,
    pub(crate) source_bytes: Vec<u8>,
    pub(crate) history_path: String,
    pub(crate) history_bytes: Vec<u8>,
}

/// Read and validate every exact source-create receipt in a complete range.
/// Claim receipts are left to the Claim range compiler; every other ToS
/// source-create receipt must prove one Record owner in that same event.
pub(crate) fn direct_source_create_records_for_range_v1(
    coverage: &crate::durable_adapter::durable_projection::DurableNavigationEventRangeCoverageV1,
    maximum: usize,
    mut read_member: impl FnMut(u64, &str) -> DurableResult<Vec<u8>>,
) -> DurableResult<Vec<RecordRangeSourceCreateV1>> {
    if maximum == 0 || maximum > MAX_PATHS {
        return Err(DurableError::Invalid(
            "navigation range direct Record owner limit",
        ));
    }
    let mut owners = BTreeMap::<String, RecordRangeSourceCreateV1>::new();
    for event in coverage.events() {
        if event.event_kind != "command" {
            continue;
        }
        let receipt_paths = event
            .members
            .iter()
            .filter(|member| {
                member.is_source
                    && member.source_path.starts_with("ToS/")
                    && member
                        .source_path
                        .ends_with(&format!("/{RECEIPT_HOME_FILE}"))
            })
            .map(|member| member.source_path.as_str())
            .collect::<Vec<_>>();
        for receipt_path in receipt_paths {
            let receipt_member =
                coverage
                    .member(event.sequence, receipt_path)
                    .ok_or(DurableError::Conflict(
                        "range Record receipt left exact event coverage",
                    ))?;
            let receipt_bytes = read_member(event.sequence, receipt_path)?;
            verify_range_member_bytes(receipt_member, &receipt_bytes, MAX_RECORD_BYTES)?;
            let receipt = parse_value(&receipt_bytes, MAX_RECORD_BYTES)?;
            match receipt.get("schema_version").and_then(Value::as_str) {
                Some("tos_local_claim_create_receipt_v1")
                    if receipt_path.starts_with("ToS/source-witnesses/relations/") =>
                {
                    continue;
                }
                Some("tos_local_source_create_receipt_v1") => {}
                _ => {
                    return Err(DurableError::Invalid(
                        "range direct Record creation receipt profile",
                    ));
                }
            }
            let source_path =
                receipt
                    .get("source_path")
                    .and_then(Value::as_str)
                    .ok_or(DurableError::Invalid(
                        "range direct Record creation source path",
                    ))?;
            let source_path = canonical_path(source_path)?.to_owned();
            let home = receipt_path
                .rsplit_once('/')
                .map(|(home, _)| home)
                .ok_or(DurableError::Invalid("range direct Record receipt home"))?;
            if source_path
                .rsplit_once('/')
                .map(|(source_home, _)| source_home)
                != Some(home)
            {
                return Err(DurableError::Conflict(
                    "range direct Record receipt source is outside its home",
                ));
            }
            let source_member = coverage
                .member(event.sequence, &source_path)
                .filter(|member| member.is_source)
                .ok_or(DurableError::Invalid(
                    "range direct Record source is not an exact command member",
                ))?;
            let source_bytes = read_member(event.sequence, &source_path)?;
            verify_range_member_bytes(source_member, &source_bytes, MAX_RECORD_BYTES)?;
            let record = parse_value(&source_bytes, MAX_RECORD_BYTES)?;
            let id = receipt
                .pointer("/source/id")
                .and_then(Value::as_str)
                .ok_or(DurableError::Invalid(
                    "range direct Record receipt identity",
                ))?;
            let source_version = receipt.pointer("/source/version").and_then(Value::as_u64);
            let source_digest = receipt
                .pointer("/source/digest")
                .and_then(Value::as_str)
                .and_then(|value| value.strip_prefix("sha256:"));
            // source.digest identifies the canonical Record; files and exact
            // range coverage independently bind its published source bytes.
            let record_digest = Digest256::of_bytes(
                &canonical_raw_bytes_v1(
                    &source_bytes,
                    CanonicalProfile::SourceRecordDigestV1,
                    JsonLimits {
                        max_bytes: MAX_RECORD_BYTES,
                        ..JsonLimits::default()
                    },
                )
                .map_err(|_| DurableError::Invalid("range direct Record canonical digest"))?,
            );
            let record_id = record
                .get("record_id")
                .or_else(|| record.get("artifact_id"))
                .or_else(|| record.get("composite_id"))
                .and_then(Value::as_str);
            if source_version != Some(1)
                || !id.starts_with("tos.")
                || id.len() > 256
                || record_id != Some(id)
                || source_digest != Some(record_digest.to_hex().as_str())
            {
                return Err(DurableError::Conflict(
                    "range direct Record source differs from its creation receipt",
                ));
            }
            let basename = source_path
                .rsplit('/')
                .next()
                .ok_or(DurableError::Invalid("range direct Record basename"))?;
            let source_binding = receipt
                .get("files")
                .and_then(|value| value.get(basename))
                .ok_or(DurableError::Invalid(
                    "range direct Record receipt source member absent",
                ))?;
            verify_receipt_file(source_binding, &source_bytes)?;

            let provenance_event_path = format!("{home}/{EVENT_HOME_FILE}");
            let provenance_member = coverage
                .member(event.sequence, &provenance_event_path)
                .filter(|member| member.is_source)
                .ok_or(DurableError::Invalid(
                    "range direct Record creation event is not a command member",
                ))?;
            let provenance_event_bytes = read_member(event.sequence, &provenance_event_path)?;
            verify_range_member_bytes(
                provenance_member,
                &provenance_event_bytes,
                MAX_RECORD_BYTES,
            )?;
            let event_binding = receipt
                .get("files")
                .and_then(|value| value.get(EVENT_HOME_FILE))
                .ok_or(DurableError::Invalid(
                    "range direct Record event receipt member absent",
                ))?;
            verify_receipt_file(event_binding, &provenance_event_bytes)?;
            validate_creation_event_output(
                &provenance_event_bytes,
                receipt_path,
                &source_path,
                &source_bytes,
                source_binding,
            )?;
            let source_basename = source_path
                .rsplit('/')
                .next()
                .ok_or(DurableError::Invalid("range direct Record source basename"))?;
            let forms_path = source_basename
                .strip_suffix(".json")
                .map(|stem| format!("{home}/{stem}.human-forms.json"))
                .ok_or(DurableError::Invalid("range direct Record source suffix"))?;
            let forms_basename = source_basename
                .strip_suffix(".json")
                .map(|stem| format!("{stem}.human-forms.json"))
                .ok_or(DurableError::Invalid("range direct Record source suffix"))?;
            let forms_binding = receipt
                .get("files")
                .and_then(|value| value.get(forms_basename.as_str()));
            let forms_member = coverage.member(event.sequence, &forms_path);
            let (forms_path, forms_sha256, forms_bytes) = match (forms_binding, forms_member) {
                (Some(binding), Some(member)) if member.is_source => {
                    let raw = read_member(event.sequence, &forms_path)?;
                    verify_range_member_bytes(member, &raw, MAX_RECORD_BYTES)?;
                    verify_receipt_file(binding, &raw)?;
                    validate_creation_event_output(
                        &provenance_event_bytes,
                        receipt_path,
                        &forms_path,
                        &raw,
                        binding,
                    )?;
                    (Some(forms_path), Some(member.sha256), Some(raw))
                }
                (None, None) => (None, None, None),
                (None, Some(_)) | (Some(_), None) | (Some(_), Some(_)) => {
                    return Err(DurableError::Conflict(
                        "range direct Record creation forms differ from receipt membership",
                    ));
                }
            };
            let command_id = event.command_id.clone().ok_or(DurableError::Corrupt(
                "range direct Record command identity is absent",
            ))?;
            let owner = RecordOwnerV1 {
                source_path: source_path.clone(),
                record_id: id.to_owned(),
                origin: RecordOwnerOriginV1::DirectSourceCreate,
            };
            if owners
                .insert(
                    source_path.clone(),
                    RecordRangeSourceCreateV1 {
                        owner,
                        event_sequence: event.sequence,
                        event_id: event.event_id.clone(),
                        command_id,
                        source_path,
                        source_sha256: source_member.sha256,
                        source_bytes,
                        forms_path,
                        forms_sha256,
                        forms_bytes,
                        receipt_path: receipt_path.to_owned(),
                        receipt_sha256: receipt_member.sha256,
                        receipt_bytes,
                        provenance_event_path,
                        provenance_event_sha256: provenance_member.sha256,
                        provenance_event_bytes,
                    },
                )
                .is_some()
            {
                return Err(DurableError::Conflict(
                    "range Record source has multiple creation receipts",
                ));
            }
            if owners.len() > maximum {
                return Err(DurableError::Refused(
                    "range direct Record owner count budget",
                ));
            }
        }
    }
    Ok(owners.into_values().collect())
}

/// Collect each retained Record revision from its exact historical CMD cut.
/// The source file is identified by the history record ID among that same
/// event's changed JSON members; later terminal bytes never stand in for the
/// mutation that the event actually committed.
pub(crate) fn direct_record_revisions_for_range_v1(
    coverage: &crate::durable_adapter::durable_projection::DurableNavigationEventRangeCoverageV1,
    maximum: usize,
    mut read_member: impl FnMut(u64, &str) -> DurableResult<Vec<u8>>,
) -> DurableResult<Vec<RecordRangeRevisionV1>> {
    if maximum == 0 || maximum > MAX_PATHS {
        return Err(DurableError::Invalid(
            "navigation range Record revision limit",
        ));
    }
    let mut revisions = Vec::new();
    for event in coverage.events() {
        if event.event_kind != "command" {
            continue;
        }
        let command_id = event.command_id.clone().ok_or(DurableError::Corrupt(
            "range Record revision command identity is absent",
        ))?;
        for history_member in event.members.iter().filter(|member| {
            member.is_source
                && member.source_path.starts_with("ToS/")
                && member
                    .source_path
                    .ends_with("/source-revision-history.json")
        }) {
            let history_path = history_member.source_path.clone();
            let home = history_path
                .rsplit_once('/')
                .map(|(home, _)| home)
                .ok_or(DurableError::Invalid("range Record revision history home"))?;
            let history_bytes = read_member(event.sequence, &history_path)?;
            verify_range_member_bytes(history_member, &history_bytes, MAX_RECORD_BYTES)?;
            let history = parse_value(&history_bytes, MAX_RECORD_BYTES)?;
            if !matches!(
                history.get("schema_version").and_then(Value::as_str),
                Some("tos_source_revision_history_v1" | "tos_source_revision_history_v2")
            ) {
                return Err(DurableError::Invalid(
                    "range Record revision history profile",
                ));
            }
            let record_id = history
                .get("record_id")
                .and_then(Value::as_str)
                .filter(|id| id.starts_with("tos.") && id.len() <= 256)
                .ok_or(DurableError::Invalid("range Record revision identity"))?;
            let receipts = history
                .get("receipts")
                .and_then(Value::as_array)
                .filter(|receipts| !receipts.is_empty() && receipts.len() <= 128)
                .ok_or(DurableError::Invalid("range Record revision receipts"))?;
            let latest = receipts.last().ok_or(DurableError::Invalid(
                "range Record latest revision receipt",
            ))?;
            let source_ref = latest
                .get("source")
                .ok_or(DurableError::Invalid("range Record revision successor"))?;
            if latest.get("command_id").and_then(Value::as_str) != Some(command_id.as_str())
                || source_ref.get("id").and_then(Value::as_str) != Some(record_id)
            {
                return Err(DurableError::Conflict(
                    "range Record revision history does not bind its event",
                ));
            }
            let mut matching_sources = Vec::new();
            for member in event.members.iter().filter(|member| {
                member.is_source
                    && member.source_path.starts_with(&format!("{home}/"))
                    && member.source_path.ends_with(".json")
                    // The history carries the same record_id but is not the record.
                    && member.source_path != history_path
                    && !member.source_path.ends_with(".human-forms.json")
            }) {
                let raw = read_member(event.sequence, &member.source_path)?;
                verify_range_member_bytes(member, &raw, MAX_RECORD_BYTES)?;
                let value = parse_value(&raw, MAX_RECORD_BYTES)?;
                let id = value
                    .get("record_id")
                    .or_else(|| value.get("artifact_id"))
                    .or_else(|| value.get("composite_id"))
                    .and_then(Value::as_str);
                if id == Some(record_id) {
                    matching_sources.push((member.source_path.clone(), raw));
                }
            }
            if matching_sources.len() != 1 {
                return Err(DurableError::Conflict(
                    "range Record revision source is not uniquely bound",
                ));
            }
            let (source_path, source_bytes) = matching_sources
                .pop()
                .ok_or(DurableError::Corrupt("range Record revision source absent"))?;
            let source_member =
                coverage
                    .member(event.sequence, &source_path)
                    .ok_or(DurableError::Conflict(
                        "range Record revision source left event coverage",
                    ))?;
            let source_value = parse_value(&source_bytes, MAX_RECORD_BYTES)?;
            let version = source_value
                .get("record_version")
                .and_then(Value::as_u64)
                .ok_or(DurableError::Invalid("range Record revision version"))?;
            if source_ref.get("version").and_then(Value::as_u64) != Some(version)
                || source_member.sha256 == Digest256::of_bytes(&[])
            {
                return Err(DurableError::Conflict(
                    "range Record revision successor differs from event source",
                ));
            }
            revisions.push(RecordRangeRevisionV1 {
                owner: RecordOwnerV1 {
                    source_path: source_path.clone(),
                    record_id: record_id.to_owned(),
                    origin: RecordOwnerOriginV1::PersistedAffected,
                },
                event_sequence: event.sequence,
                event_id: event.event_id.clone(),
                command_id: command_id.clone(),
                source_path,
                source_bytes: source_bytes.to_vec(),
                history_path,
                history_bytes,
            });
            if revisions.len() > maximum {
                return Err(DurableError::Refused(
                    "navigation range Record revision owner count",
                ));
            }
        }
    }
    revisions.sort_by(|left, right| {
        left.event_sequence
            .cmp(&right.event_sequence)
            .then_with(|| left.source_path.cmp(&right.source_path))
    });
    if revisions.windows(2).any(|pair| {
        (pair[0].event_sequence, pair[0].source_path.as_str())
            >= (pair[1].event_sequence, pair[1].source_path.as_str())
    }) {
        return Err(DurableError::Corrupt(
            "range Record revision causes are not uniquely ordered",
        ));
    }
    Ok(revisions)
}

fn verify_range_member_bytes(
    member: &crate::durable_adapter::durable_projection::DurableNavigationEventRangeMemberV1,
    raw: &[u8],
    maximum_bytes: usize,
) -> DurableResult<()> {
    if raw.is_empty()
        || raw.len() > maximum_bytes
        || raw.len() as u64 != member.size_bytes
        || Digest256::of_bytes(raw) != member.sha256
    {
        return Err(DurableError::Conflict(
            "range Record historical member digest or size differs",
        ));
    }
    Ok(())
}

fn verify_receipt_file(binding: &Value, raw: &[u8]) -> DurableResult<()> {
    let expected_digest = format!("sha256:{}", Digest256::of_bytes(raw).to_hex());
    if binding.get("sha256").and_then(Value::as_str) != Some(expected_digest.as_str())
        || binding.get("bytes").and_then(Value::as_u64) != Some(raw.len() as u64)
    {
        return Err(DurableError::Conflict(
            "direct Record creation receipt byte binding differs",
        ));
    }
    Ok(())
}

fn validate_creation_event_output(
    event_bytes: &[u8],
    receipt_path: &str,
    source_path: &str,
    source_bytes: &[u8],
    source_binding: &Value,
) -> DurableResult<()> {
    let mut selected = None;
    for line in event_bytes.split(|byte| *byte == b'\n') {
        if line.is_empty() {
            continue;
        }
        let event = parse_value(line.strip_suffix(b"\r").unwrap_or(line), MAX_RECORD_BYTES)?;
        if event
            .pointer("/record_binding/manifest_ref")
            .and_then(Value::as_str)
            == Some(receipt_path)
        {
            if selected.replace(event).is_some() {
                return Err(DurableError::Conflict(
                    "direct Record creation event receipt reference is ambiguous",
                ));
            }
        }
    }
    let event = selected.ok_or(DurableError::Invalid(
        "direct Record creation event receipt reference absent",
    ))?;
    if event
        .pointer("/record_binding/digest_algorithm")
        .and_then(Value::as_str)
        != Some("sha256")
        || event
            .pointer("/record_binding/digest_scope")
            .and_then(Value::as_str)
            != Some("exact_event_record_bytes")
    {
        return Err(DurableError::Conflict(
            "direct Record creation event digest profile differs",
        ));
    }
    let output = event
        .pointer("/entities/outputs")
        .and_then(Value::as_array)
        .and_then(|outputs| {
            let mut matching = outputs.iter().filter(|output| {
                output.get("entity_ref").and_then(Value::as_str) == Some(source_path)
            });
            let output = matching.next()?;
            matching.next().is_none().then_some(output)
        })
        .ok_or(DurableError::Invalid(
            "direct Record creation event output absent or ambiguous",
        ))?;
    let raw_digest = Digest256::of_bytes(source_bytes).to_hex();
    if output.get("sha256").and_then(Value::as_str) != Some(raw_digest.as_str())
        || output.get("size_bytes").and_then(Value::as_u64) != Some(source_bytes.len() as u64)
        || source_binding.get("sha256").and_then(Value::as_str)
            != Some(format!("sha256:{raw_digest}").as_str())
    {
        return Err(DurableError::Conflict(
            "direct Record creation event output differs from exact source",
        ));
    }
    Ok(())
}

fn insert_record_owner(
    owners: &mut BTreeMap<String, RecordOwnerV1>,
    owner: RecordOwnerV1,
    maximum: usize,
) -> DurableResult<()> {
    let source_path = canonical_path(&owner.source_path)?;
    if !owner.record_id.starts_with("tos.") || owner.record_id.len() > 256 {
        return Err(DurableError::Invalid("navigation Record owner identity"));
    }
    if let Some(previous) = owners.get(source_path) {
        if previous.record_id != owner.record_id {
            return Err(DurableError::Conflict(
                "navigation Record owner path has conflicting identities",
            ));
        }
        if previous.origin == RecordOwnerOriginV1::DirectSourceCreate
            || owner.origin == RecordOwnerOriginV1::PersistedAffected
        {
            return Ok(());
        }
    }
    if owners.values().any(|previous| {
        previous.source_path != source_path && previous.record_id == owner.record_id
    }) {
        return Err(DurableError::Conflict(
            "navigation Record identity has multiple source paths",
        ));
    }
    owners.insert(source_path.to_owned(), owner);
    if owners.len() > maximum {
        return Err(DurableError::Refused("navigation Record owner budget"));
    }
    Ok(())
}

fn add_owner_paths(
    paths: &mut BTreeSet<String>,
    owner: &RecordOwnerV1,
    maximum: usize,
) -> DurableResult<()> {
    let source_path = canonical_path(&owner.source_path)?;
    let home = source_path
        .rsplit_once('/')
        .map(|(home, _)| home)
        .ok_or(DurableError::Invalid("navigation Record source home"))?;
    let basename = source_path
        .rsplit('/')
        .next()
        .ok_or(DurableError::Invalid("navigation Record source basename"))?;
    let forms = basename
        .strip_suffix(".json")
        .map(|stem| format!("{home}/{stem}.human-forms.json"))
        .ok_or(DurableError::Invalid("navigation Record source suffix"))?;
    for path in [
        source_path.to_owned(),
        format!("{home}/{EVENT_HOME_FILE}"),
        format!("{home}/{RECEIPT_HOME_FILE}"),
        forms,
        format!("{home}/source-revision-history.json"),
    ] {
        add_path(paths, &path, maximum)?;
    }
    Ok(())
}

/// Derive the terminal current-source closure for an already receipt-checked
/// owner set. Range callers use this after they have validated every direct
/// creation receipt at its historical event cut and unioned predecessor
/// affected owners; the context itself stays a truthful terminal event row.
pub(crate) fn required_paths_for_owners_v1(
    terminal: &DurableNavigationEventContextV1,
    owners: &[RecordOwnerV1],
    configured_event_path: &str,
    schema_paths: &[String],
    max_dependencies: usize,
) -> DurableResult<RecordEventPathPlanV1> {
    if terminal.event_id().is_empty()
        || terminal.event_sequence() == 0
        || terminal.event_sequence() > terminal.source_head_sequence()
        || terminal.consumer_cursor().sequence >= terminal.event_sequence()
        || max_dependencies == 0
        || max_dependencies > MAX_PATHS
    {
        return Err(DurableError::Invalid(
            "navigation terminal Record path plan context or limits",
        ));
    }
    let mut paths = BTreeSet::new();
    add_path(&mut paths, configured_event_path, max_dependencies)?;
    for member in terminal.members() {
        add_path(&mut paths, &member.source_path, max_dependencies)?;
    }
    for selected in terminal.selected_sources() {
        add_path(&mut paths, &selected.source_path, max_dependencies)?;
    }
    let mut by_path = BTreeMap::<String, RecordOwnerV1>::new();
    for owner in owners {
        insert_record_owner(&mut by_path, owner.clone(), max_dependencies)?;
    }
    for owner in by_path.values() {
        add_owner_paths(&mut paths, owner, max_dependencies)?;
    }
    if !by_path.is_empty() {
        for path in [REGISTRY, REGISTRY_SCHEMA, EVENT_SCHEMA, FORMS_SCHEMA] {
            add_path(&mut paths, path, max_dependencies)?;
        }
    }
    if schema_paths.len() > max_dependencies {
        return Err(DurableError::Refused(
            "navigation Record schema dependency budget",
        ));
    }
    for path in schema_paths {
        add_path(&mut paths, path, max_dependencies)?;
    }

    Ok(RecordEventPathPlanV1 {
        owners: by_path.into_values().collect(),
        paths: paths.into_iter().collect(),
    })
}

/// Derive only the archived Record manifests needed by maintained metadata
/// history after the caller has selected the current Record, registry, and
/// history bytes. The manifest determines each exact historical Record blob.
pub(crate) fn archive_manifest_paths(
    owners: &[RecordOwnerV1],
    files: &SelectedBibliographicFilesV1,
    limits: BibliographicLimits,
) -> DurableResult<Vec<String>> {
    let entities = parse_value(
        files
            .required(REGISTRY)
            .map_err(|_| DurableError::Invalid("selected entity registry absent"))?,
        MAX_RECORD_BYTES,
    )?;
    let mut manifests = BTreeSet::new();
    for owner in owners {
        let (entry, _, _) = selected_record_entry(owner, files, &entities, limits)?;
        for path in selected_metadata_archive_manifest_paths_v1(&entry, &entities, files, limits)
            .map_err(compiler_refusal)?
        {
            add_path(&mut manifests, &path, MAX_PATHS)?;
        }
    }
    Ok(manifests.into_iter().collect())
}

/// Derive the exact archived Record blob paths from selected manifests. The
/// compiler's maintained history resolver later verifies the manifest/package
/// commitment and the exact archived bytes again.
pub(crate) fn archive_blob_paths(
    owners: &[RecordOwnerV1],
    manifest_paths: &[String],
    files: &SelectedBibliographicFilesV1,
    limits: BibliographicLimits,
) -> DurableResult<Vec<String>> {
    if manifest_paths.len() > 128 {
        return Err(DurableError::Refused(
            "selected navigation Record archive count",
        ));
    }
    let baseline_manifests = owners
        .iter()
        .map(|owner| baseline_archive_manifest_path(owner, files, limits))
        .collect::<DurableResult<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect::<BTreeSet<_>>();
    if !baseline_manifests
        .iter()
        .all(|path| manifest_paths.contains(path))
    {
        return Err(DurableError::Conflict(
            "Record baseline archive is outside selected history",
        ));
    }
    let mut blobs = BTreeSet::new();
    for manifest_path in manifest_paths {
        let raw = files
            .required(manifest_path)
            .map_err(|_| DurableError::Invalid("selected Record archive manifest absent"))?;
        let path = selected_metadata_archive_record_blob_path_v1(manifest_path, raw, limits)
            .map_err(compiler_refusal)?;
        add_path(&mut blobs, &path, MAX_PATHS)?;
        if baseline_manifests.contains(manifest_path) {
            if let Some(forms_path) =
                selected_metadata_archive_forms_blob_path_v1(manifest_path, raw, limits)
                    .map_err(compiler_refusal)?
            {
                add_path(&mut blobs, &forms_path, MAX_PATHS)?;
            }
        }
    }
    Ok(blobs.into_iter().collect())
}

// History identity and continuous receipts are verified by the compiler
// before projection; this only selects the first retained archive address.
fn baseline_archive_manifest_path(
    owner: &RecordOwnerV1,
    files: &SelectedBibliographicFilesV1,
    limits: BibliographicLimits,
) -> DurableResult<Option<String>> {
    let home = owner
        .source_path
        .rsplit_once('/')
        .map(|(home, _)| home)
        .ok_or(DurableError::Invalid("Record baseline source home"))?;
    let history_path = format!("{home}/source-revision-history.json");
    let Some(raw) = files.optional(&history_path).map_err(compiler_refusal)? else {
        return Ok(None);
    };
    let history = parse_value(raw, limits.catalog.max_row_bytes)?;
    let archive = history
        .get("receipts")
        .and_then(Value::as_array)
        .and_then(|receipts| receipts.first())
        .and_then(|receipt| receipt.get("archive_path"))
        .and_then(Value::as_str)
        .ok_or(DurableError::Invalid(
            "Record baseline archive absent from retained history",
        ))?;
    Ok(Some(format!("{archive}/manifest.json")))
}

/// Preview only the exact selected terminal dependency rows needed to derive
/// complete Record reverse-owner sets. The temporary cohort is discarded in
/// this function; its placeholder basis never reaches the range coordinator
/// or the PG publisher. Final rows are rendered again with compiler-issued
/// cause roots.
#[allow(clippy::too_many_arguments)]
pub(crate) fn preview_record_range_terminal_dependencies_v1(
    context: &DurableNavigationEventContextV1,
    identity: NavigationEventSourceCutIdentityV1,
    validator: &SourceCatalogValidator<'_>,
    files: &SelectedBibliographicFilesV1,
    owner: &RecordOwnerV1,
    predecessor_cohort_root: Option<Digest256>,
    coverage: &tos_compiler::source_navigation_event::NavigationEventRangeCoverageV1,
    origin_forms_bytes: Option<&[u8]>,
    forms_materializer: &mut dyn BibliographicForms,
    limits: BibliographicLimits,
) -> DurableResult<(
    Vec<NavigationDependencyV1>,
    tos_compiler::source_navigation_event::NavigationRecordEventBudgetCostV1,
)> {
    // This digest is only a non-empty renderer input. The resulting cohort is
    // immediately dropped, and no value derived from this digest is retained.
    let preview_basis = Digest256::of_bytes(b"tos-source-navigation-range-private-preview-v1");
    let (cohort, cost) = assemble_record_range_terminal_cohort_v1(
        context,
        identity,
        validator,
        files,
        owner,
        predecessor_cohort_root,
        coverage,
        preview_basis,
        origin_forms_bytes,
        forms_materializer,
        limits,
    )?;
    Ok((cohort.dependencies().to_vec(), cost))
}

/// Assemble the terminal Record cohort for the compiler-issued event range.
#[allow(clippy::too_many_arguments)]
pub(crate) fn assemble_record_range_terminal_cohort_v1<'a, 'validator>(
    context: &'a DurableNavigationEventContextV1,
    identity: NavigationEventSourceCutIdentityV1,
    validator: &'a SourceCatalogValidator<'validator>,
    files: &'a SelectedBibliographicFilesV1,
    owner: &'a RecordOwnerV1,
    predecessor_cohort_root: Option<Digest256>,
    coverage: &'a tos_compiler::source_navigation_event::NavigationEventRangeCoverageV1,
    cause_root: Digest256,
    origin_forms_bytes: Option<&'a [u8]>,
    forms_materializer: &'a mut dyn BibliographicForms,
    limits: BibliographicLimits,
) -> DurableResult<(
    tos_compiler::source_navigation_event::NavigationRecordEventCohortV1,
    tos_compiler::source_navigation_event::NavigationRecordEventBudgetCostV1,
)> {
    if context.event_sequence() != identity.event_sequence()
        || context.source_head_sequence() != identity.source_head_sequence()
    {
        return Err(DurableError::Conflict(
            "selected navigation Record owner/event identity differs",
        ));
    }
    let entities_raw = files
        .required(REGISTRY)
        .map_err(|_| DurableError::Invalid("selected entity registry absent"))?;
    let entities = parse_value(entities_raw, limits.catalog.max_row_bytes)?;
    let (entry, source_record, schema_path) =
        selected_record_entry(owner, files, &entities, limits)?;
    let source_path = owner.source_path.as_str();
    let schema_bytes = files
        .required(&schema_path)
        .map_err(|_| DurableError::Invalid("selected Record schema absent"))?;
    let event_revision = identity.event_source_revision();
    let selected_history = resolve_selected_metadata_history_v1(
        &entry,
        &entities,
        files,
        validator,
        event_revision,
        limits,
    )
    .map_err(compiler_refusal)?;
    if selected_history.current_record != source_record
        || selected_history.schema_path != schema_path
        || selected_history.schema_bytes != schema_bytes
        || selected_history.source_path != source_path
        || selected_history.current_record["record_id"] != owner.record_id
            && selected_history.current_record["artifact_id"] != owner.record_id
            && selected_history.current_record["composite_id"] != owner.record_id
    {
        return Err(DurableError::Conflict(
            "selected navigation Record owner binding differs",
        ));
    }

    let source_basename = source_path
        .rsplit('/')
        .next()
        .ok_or(DurableError::Invalid("navigation Record source basename"))?;
    let forms_path = source_basename
        .strip_suffix(".json")
        .map(|stem| {
            format!(
                "{}/{stem}.human-forms.json",
                source_path
                    .rsplit_once('/')
                    .map(|(home, _)| home)
                    .unwrap_or("")
            )
        })
        .ok_or(DurableError::Invalid("navigation Record source suffix"))?;
    let forms_bytes = files
        .optional(&forms_path)
        .map_err(|_| DurableError::Invalid("selected Record forms state unknown"))?;
    let (forms_schema_path, forms_schema_bytes) = if forms_bytes.is_some() {
        (
            FORMS_SCHEMA,
            Some(
                files
                    .required(FORMS_SCHEMA)
                    .map_err(|_| DurableError::Invalid("selected Record forms schema absent"))?,
            ),
        )
    } else {
        ("", None)
    };

    let home = source_path
        .rsplit_once('/')
        .map(|(home, _)| home)
        .ok_or(DurableError::Invalid("navigation Record source home"))?;
    let event_path = format!("{home}/{EVENT_HOME_FILE}");
    let receipt_path = format!("{home}/{RECEIPT_HOME_FILE}");
    let event_file_bytes = files
        .optional(&event_path)
        .map_err(|_| DurableError::Invalid("selected Record creation event state unknown"))?;
    let event_receipt_bytes = files
        .optional(&receipt_path)
        .map_err(|_| DurableError::Invalid("selected Record creation receipt state unknown"))?;
    match (event_file_bytes, event_receipt_bytes) {
        (Some(event), Some(receipt)) => {
            verify_creation_receipt(&entry, source_path, &receipt_path, event, receipt, limits)?
        }
        (None, None)
            if predecessor_cohort_root.is_some()
                && owner.origin == RecordOwnerOriginV1::PersistedAffected => {}
        _ => {
            return Err(DurableError::Conflict(
                "Record creation evidence is incomplete or lacks an admitted predecessor",
            ));
        }
    }

    let entity_schema_bytes = files
        .required(REGISTRY_SCHEMA)
        .map_err(|_| DurableError::Invalid("selected entity registry schema absent"))?;
    let event_schema_bytes = files
        .required(EVENT_SCHEMA)
        .map_err(|_| DurableError::Invalid("selected provenance event schema absent"))?;

    let mut dependencies = BTreeMap::<String, NavigationDependencyV1>::new();
    // CMD member witnesses are mandatory for every Record cohort. Owner-specific
    // roles below refine matching paths without duplicating a dependency row.
    for member in context.members() {
        let role = NavigationDependencyRoleV1::Additional;
        let state = if member.is_source {
            let raw = files
                .required(&member.source_path)
                .map_err(|_| DurableError::Invalid("selected CMD event member absent"))?;
            if member.content_digest != Some(Digest256::of_bytes(raw))
                || member.content_length != Some(raw.len() as u64)
            {
                return Err(DurableError::Conflict(
                    "selected CMD event member bytes differ",
                ));
            }
            NavigationDependencyStateV1::Present {
                sha256: Digest256::of_bytes(raw),
                size_bytes: raw.len() as u64,
            }
        } else {
            if files
                .optional(&member.source_path)
                .map_err(|_| DurableError::Invalid("selected CMD absence unknown"))?
                .is_some()
            {
                return Err(DurableError::Conflict(
                    "selected CMD absence has source bytes",
                ));
            }
            NavigationDependencyStateV1::Absent {
                reason: "committed event member is explicitly non-source".into(),
            }
        };
        insert_dependency(&mut dependencies, &member.source_path, role, state)?;
    }

    for (path, role) in [
        (source_path, NavigationDependencyRoleV1::Record),
        (EVENT_SCHEMA, NavigationDependencyRoleV1::EventSchema),
        (REGISTRY, NavigationDependencyRoleV1::EntityRegistry),
        (
            REGISTRY_SCHEMA,
            NavigationDependencyRoleV1::EntityRegistrySchema,
        ),
        (&schema_path, NavigationDependencyRoleV1::RecordSchema),
    ] {
        insert_file_dependency(files, &mut dependencies, path, role, true)?;
    }
    // Imported/authored Records can predate the source-create command. Their
    // admitted predecessor and range cause own continuity; absent creation
    // files are explicit witnesses, never fabricated provenance.
    for (path, role) in [
        (&event_path, NavigationDependencyRoleV1::Event),
        (&receipt_path, NavigationDependencyRoleV1::EventReceipt),
    ] {
        insert_file_dependency(files, &mut dependencies, path, role, false)?;
    }
    insert_file_dependency(
        files,
        &mut dependencies,
        &forms_path,
        NavigationDependencyRoleV1::HumanForms,
        false,
    )?;
    let forms_history_path = format!("{home}/source-revision-history.json");
    insert_file_dependency(
        files,
        &mut dependencies,
        &forms_history_path,
        NavigationDependencyRoleV1::RecordHistory,
        false,
    )?;
    if forms_bytes.is_some() {
        insert_file_dependency(
            files,
            &mut dependencies,
            FORMS_SCHEMA,
            NavigationDependencyRoleV1::FormsSchema,
            true,
        )?;
    }
    for manifest_path in
        selected_metadata_archive_manifest_paths_v1(&entry, &entities, files, limits)
            .map_err(compiler_refusal)?
    {
        insert_file_dependency(
            files,
            &mut dependencies,
            &manifest_path,
            NavigationDependencyRoleV1::VersionSource,
            true,
        )?;
    }

    let version_sources = selected_history
        .version_sources
        .iter()
        .map(|source| {
            insert_file_dependency(
                files,
                &mut dependencies,
                &source.source_path,
                if source.source_path == source_path {
                    NavigationDependencyRoleV1::Record
                } else {
                    NavigationDependencyRoleV1::VersionSource
                },
                true,
            )?;
            insert_file_dependency(
                files,
                &mut dependencies,
                &source.schema_path,
                if source.schema_path == schema_path {
                    NavigationDependencyRoleV1::RecordSchema
                } else {
                    NavigationDependencyRoleV1::VersionSchema
                },
                true,
            )?;
            Ok(Some(NavigationVersionSourceInputV1 {
                source_record_bytes: source.source_record_bytes,
                schema_path: &source.schema_path,
                schema_bytes: source.schema_bytes,
            }))
        })
        .collect::<DurableResult<Vec<_>>>()?;

    let history_bytes = selected_history.history_bytes;
    let history = &selected_history.history;
    let event_id = context.event_id();
    let event_sequence = context.event_sequence();
    // The renderer's range basis describes the event cut at `through`; the
    // typed source-cut identity separately retains the held current head.
    // Preserve the historical prefix here instead of relabeling it as the
    // later held head.
    let source_head_sequence = coverage.through_sequence();
    let predecessor_source_revision =
        context
            .consumer_cursor()
            .source_revision
            .ok_or(DurableError::Conflict(
                "navigation Record consumer source revision absent",
            ))?;
    // Existing owners are absent from the range's creation list. Bind their
    // original forms to the first retained archive, never to revised forms.
    let retained_origin_forms = if origin_forms_bytes.is_none() {
        if let Some(manifest_path) = baseline_archive_manifest_path(owner, files, limits)? {
            match resolve_selected_metadata_archive_forms_v1(&manifest_path, files, limits)
                .map_err(compiler_refusal)?
            {
                Some((path, raw)) => {
                    insert_file_dependency(
                        files,
                        &mut dependencies,
                        &path,
                        NavigationDependencyRoleV1::VersionSource,
                        true,
                    )?;
                    Some(raw)
                }
                None => None,
            }
        } else {
            forms_bytes
        }
    } else {
        None
    };
    let dependency_rows = dependencies.into_values().collect::<Vec<_>>();
    let input = NavigationRecordEventInputV1 {
        validator,
        event_id,
        event_sequence,
        source_head_sequence,
        predecessor_source_revision,
        predecessor_cohort_root,
        event_path: &event_path,
        event_file_bytes,
        event_schema_path: EVENT_SCHEMA,
        event_schema_bytes,
        event_receipt_path: &receipt_path,
        event_receipt_bytes,
        source_path,
        source_record_bytes: selected_history.current_record_bytes,
        entity_registry_path: REGISTRY,
        entity_registry_bytes: entities_raw,
        entity_registry_schema_path: REGISTRY_SCHEMA,
        entity_registry_schema_bytes: entity_schema_bytes,
        schema_path: &schema_path,
        schema_bytes,
        forms_path: &forms_path,
        forms_bytes,
        forms_schema_path,
        forms_schema_bytes,
        forms_materializer,
        history_path: &selected_history.history_path,
        history_bytes,
        entry: &entry,
        source_record: &selected_history.current_record,
        entities: &entities,
        history: Some(history),
        versions: &selected_history.versions,
        version_sources: &version_sources,
        dependencies: &dependency_rows,
    };
    let cohort = tos_compiler::source_navigation_event::
        project_navigation_record_range_terminal_event_from_event_cut_v1(
            input,
            coverage,
            cause_root,
            origin_forms_bytes.or(retained_origin_forms),
            limits,
        )
        .map_err(compiler_refusal)?;
    let cost = cohort.budget_cost().map_err(compiler_refusal)?;
    Ok((cohort, cost))
}

fn selected_record_entry(
    owner: &RecordOwnerV1,
    files: &SelectedBibliographicFilesV1,
    entities: &Value,
    limits: BibliographicLimits,
) -> DurableResult<(Value, Value, String)> {
    let source_path = canonical_path(&owner.source_path)?;
    let raw = files
        .required(source_path)
        .map_err(|_| DurableError::Invalid("selected Record source absent"))?;
    if raw.len() > MAX_RECORD_BYTES {
        return Err(DurableError::Refused("selected Record source byte budget"));
    }
    let record = parse_value(raw, limits.catalog.max_row_bytes)?;
    let record_type = record_type(&record, &owner.record_id)?;
    let schema_path =
        selected_metadata_record_schema_path_v1(&record_type, source_path, &record, entities)
            .map_err(compiler_refusal)?;
    let entry = render_catalog_record(
        &record,
        source_path,
        Some(&schema_path),
        limits.catalog.max_output_row_bytes,
    )
    .map_err(compiler_refusal)?;
    let identity = record
        .get("record_id")
        .or_else(|| record.get("artifact_id"))
        .or_else(|| record.get("composite_id"))
        .and_then(Value::as_str);
    if identity != Some(owner.record_id.as_str()) {
        return Err(DurableError::Conflict(
            "selected Record ID differs from owner",
        ));
    }
    Ok((entry, record, schema_path))
}

fn record_type(record: &Value, owner_id: &str) -> DurableResult<String> {
    if let Some(kind) = record.get("record_type").and_then(Value::as_str) {
        if !kind.is_empty() && kind.len() <= 64 {
            return Ok(kind.to_owned());
        }
    }
    let kind = if record.get("artifact_id").is_some() {
        "artifact"
    } else if record.get("composite_id").is_some() {
        "composite"
    } else {
        return Err(DurableError::Invalid("selected Record type absent"));
    };
    if !owner_id.starts_with(&format!("tos.{kind}.")) {
        return Err(DurableError::Conflict(
            "selected native Record type differs",
        ));
    }
    Ok(kind.to_owned())
}

fn verify_creation_receipt(
    entry: &Value,
    source_path: &str,
    receipt_path: &str,
    event_file_bytes: &[u8],
    receipt_bytes: &[u8],
    limits: BibliographicLimits,
) -> DurableResult<()> {
    let receipt = parse_value(receipt_bytes, limits.catalog.max_row_bytes)?;
    let source = receipt
        .get("source")
        .ok_or(DurableError::Invalid("Record create receipt source absent"))?;
    let id = entry
        .get("record_id")
        .and_then(Value::as_str)
        .ok_or(DurableError::Invalid("Record create entry ID absent"))?;
    if receipt.get("schema_version").and_then(Value::as_str)
        != Some("tos_local_source_create_receipt_v1")
        || receipt.get("source_path").and_then(Value::as_str) != Some(source_path)
        || receipt.pointer("/source/id").and_then(Value::as_str) != Some(id)
        || receipt.pointer("/source/version").and_then(Value::as_u64) != Some(1)
    {
        return Err(DurableError::Conflict(
            "Record create receipt identity differs",
        ));
    }
    let manifest_ref = receipt_path;
    let mut selected = None;
    for line in event_file_bytes.split(|byte| *byte == b'\n') {
        if line.is_empty() {
            continue;
        }
        let value = parse_value(
            line.strip_suffix(b"\r").unwrap_or(line),
            limits.catalog.max_row_bytes,
        )?;
        if value
            .pointer("/record_binding/manifest_ref")
            .and_then(Value::as_str)
            == Some(manifest_ref)
        {
            if selected.replace(value).is_some() {
                return Err(DurableError::Conflict(
                    "Record create provenance receipt reference is ambiguous",
                ));
            }
        }
    }
    let event = selected.ok_or(DurableError::Conflict(
        "Record create provenance receipt reference is absent",
    ))?;
    if event
        .pointer("/record_binding/digest_algorithm")
        .and_then(Value::as_str)
        != Some("sha256")
        || event
            .pointer("/record_binding/digest_scope")
            .and_then(Value::as_str)
            != Some("exact_event_record_bytes")
    {
        return Err(DurableError::Conflict(
            "Record create event binding differs",
        ));
    }
    let output = event
        .pointer("/entities/outputs")
        .and_then(Value::as_array)
        .and_then(|outputs| {
            let mut matches = outputs.iter().filter(|output| {
                output.get("entity_ref").and_then(Value::as_str) == Some(source_path)
            });
            let selected = matches.next()?;
            matches.next().is_none().then_some(selected)
        })
        .ok_or(DurableError::Invalid("Record create event output absent"))?;
    let basename = source_path
        .rsplit('/')
        .next()
        .ok_or(DurableError::Invalid("Record create source basename"))?;
    let receipt_file =
        receipt
            .pointer(&format!("/files/{basename}"))
            .ok_or(DurableError::Invalid(
                "Record create receipt source file absent",
            ))?;
    let receipt_digest = receipt_file
        .get("sha256")
        .and_then(Value::as_str)
        .and_then(|digest| digest.strip_prefix("sha256:"));
    if output.get("sha256").and_then(Value::as_str) != receipt_digest
        || output.get("size_bytes").and_then(Value::as_u64)
            != receipt_file.get("bytes").and_then(Value::as_u64)
    {
        return Err(DurableError::Conflict(
            "Record create event output differs from exact receipt",
        ));
    }
    Ok(())
}

fn insert_file_dependency(
    files: &SelectedBibliographicFilesV1,
    dependencies: &mut BTreeMap<String, NavigationDependencyV1>,
    path: &str,
    role: NavigationDependencyRoleV1,
    required: bool,
) -> DurableResult<()> {
    let state = if required {
        let raw = files
            .required(path)
            .map_err(|_| DurableError::Invalid("selected navigation dependency absent"))?;
        NavigationDependencyStateV1::Present {
            sha256: Digest256::of_bytes(raw),
            size_bytes: raw.len() as u64,
        }
    } else if let Some(raw) = files
        .optional(path)
        .map_err(|_| DurableError::Invalid("selected navigation optional state unknown"))?
    {
        NavigationDependencyStateV1::Present {
            sha256: Digest256::of_bytes(raw),
            size_bytes: raw.len() as u64,
        }
    } else {
        NavigationDependencyStateV1::Absent {
            reason: "optional source member is absent in the selected event cut".into(),
        }
    };
    insert_dependency(dependencies, path, role, state)
}

fn insert_dependency(
    dependencies: &mut BTreeMap<String, NavigationDependencyV1>,
    path: &str,
    role: NavigationDependencyRoleV1,
    state: NavigationDependencyStateV1,
) -> DurableResult<()> {
    let path = canonical_path(path)?;
    let dependency = NavigationDependencyV1 {
        path: RelativePath::parse(path)
            .map_err(|_| DurableError::Invalid("navigation dependency path"))?,
        role,
        state,
    };
    if let Some(previous) = dependencies.get(path) {
        if previous.state != dependency.state {
            return Err(DurableError::Conflict(
                "selected navigation dependency has conflicting observations",
            ));
        }
        if previous.role != role && previous.role != NavigationDependencyRoleV1::Additional {
            return Err(DurableError::Conflict(
                "selected navigation dependency has conflicting roles",
            ));
        }
    }
    dependencies.insert(path.to_owned(), dependency);
    Ok(())
}

fn parse_value(raw: &[u8], max_bytes: usize) -> DurableResult<Value> {
    if raw.is_empty() || raw.len() > max_bytes {
        return Err(DurableError::Refused(
            "selected navigation JSON byte budget",
        ));
    }
    let parsed = crate::source_command::parse(raw)
        .map_err(|_| DurableError::Invalid("selected navigation strict JSON"))?;
    let canonical = crate::source_command::canonical(&parsed)
        .map_err(|_| DurableError::Invalid("selected navigation JSON canonicalization"))?;
    serde_json::from_slice(&canonical)
        .map_err(|_| DurableError::Invalid("selected navigation JSON conversion"))
}

fn add_path(paths: &mut BTreeSet<String>, path: &str, maximum: usize) -> DurableResult<()> {
    let canonical = canonical_path(path)?;
    paths.insert(canonical.to_owned());
    if paths.len() > maximum {
        return Err(DurableError::Refused("navigation Record path budget"));
    }
    Ok(())
}

fn canonical_path(path: &str) -> DurableResult<&str> {
    let parsed = RelativePath::parse(path)
        .map_err(|_| DurableError::Invalid("navigation Record dependency path"))?;
    if parsed.as_str() != path {
        return Err(DurableError::Invalid(
            "navigation Record dependency path is not canonical",
        ));
    }
    Ok(path)
}
