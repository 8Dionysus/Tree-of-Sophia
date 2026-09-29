//! Owner-local native translation-alignment proposals. One stable Alignment
//! may have many immutable descriptive records; a Claim has its own version
//! and remapping creates a new Claim. This entry records supplied mappings,
//! never executes an aligner or grants textual, review or publication truth.

use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_creation_store::verify_owner_text_current_cut;
use crate::source_creation_store::{active, finish_creation_worker};
use crate::source_serialization::{
    capture_owner_alignment, executable, instant, selected_components,
};
use crate::source_sign_native::{
    NativeInput, NativeReadKind, ResolvedOwnerAlignment, ResolvedOwnerAlignmentSide,
    SignNativeRead, resolve_owner_alignment,
};
use crate::source_text_identity::selected_alignment_identity_snapshot;
use crate::source_text_layer_entry::{checked_schema, file_refs, line};
use crate::source_text_owner::{OwnerTextAlignmentSelection, OwnerTextContext};
use crate::source_text_private_store::{
    PrivateTextCustody, PrivateTextLocks, alignment_recovery_request, observe_private_text,
    publish_private_text,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tos_foundation::{Digest256, JsonValue, RelativePath};
use tos_source_store::{CorpusCutReader, SoftwareCaptureReader, SoftwareComponentSelectionV1};
use tos_validation::item_rules::ItemLimits;
use tos_validation::layer_family_rules::inspect_supplied_translation_alignment;
use tos_validation::source_cut::{CutSchemaExecutor, CutWorkerSchemaExecutor};

const CONTEXT_SCHEMA: &str = "ToS/contracts/owner-local-source-context.schema.json";
const RECORD_SCHEMA: &str = "ToS/contracts/native-translation-alignment-record-v1.schema.json";
const LEGACY_SCHEMA: &str = "ToS/contracts/translation-alignment-packet-v1.schema.json";
const BINDING_SCHEMA: &str = "ToS/contracts/native-text-unit-binding.schema.json";
const EVENT_SCHEMA: &str = "ToS/contracts/provenance-event-v2.schema.json";
const BASENAME: &str = "native-translation-alignment.v1.json";
const MAX_CONTRACT: usize = 1_048_576;
const MAX_RECORD: usize = 1_048_576;
const MAX_PACKAGE: usize = 12 * 1024 * 1024;
const MAX_HISTORY: usize = 64;
const FILES: [&str; 7] = [
    BASENAME,
    "source-create-owner-configuration.json",
    "source-create-inputs.json",
    "source-create-request.json",
    "source-create-environment.json",
    "source-create-provenance.jsonl",
    "source-create-receipt.json",
];

pub struct NativeAlignmentPreview {
    pub owner_configuration: String,
    pub expected_dependencies: String,
    pub source_path: String,
    pub target_exists: bool,
}

pub struct NativeAlignmentResult {
    pub receipt: JsonValue,
    pub replayed: bool,
    pub grants_admission: bool,
    pub aligner_executed: bool,
}

pub struct NativeAlignmentInspection {
    pub record_version: u64,
    pub claim_version: u64,
    pub change_kind: String,
    pub history_depth: usize,
    pub inspected_source_sha256: String,
    pub mapping_summary: JsonValue,
    pub competing_forward_count: usize,
    pub metadata_verified: bool,
    pub content_verified: bool,
    pub grants_admission: bool,
}

pub enum NativeAlignmentRecovery {
    Absent,
    RetainedExactPlan,
    Committed,
}

pub struct NativeAlignmentDescription {
    pub delegated_operation: String,
    pub target_exists: bool,
    pub content_disclosure: &'static str,
    pub grants_admission: bool,
    pub aligner_executed: bool,
}

struct PreparedAlignment {
    context: OwnerTextContext,
    grant: OwnerTextAlignmentSelection,
    publication: crate::source_creation_store::work_transaction::PublicationSnapshot,
    resolved: ResolvedOwnerAlignment,
    body: JsonValue,
    files: BTreeMap<String, Vec<u8>>,
    contracts: BTreeMap<String, Digest256>,
    inventory: Digest256,
    ancestry: BTreeMap<String, Digest256>,
    history_inputs: BTreeMap<String, (Digest256, usize)>,
    software_rows: Vec<Value>,
    owner_configuration: String,
    dependencies: String,
}

// A context transports bytes; this borrowed Alignment grant supplies the live
// permission fence for every actual resolver read, including its final reread.
struct AlignmentRead<'a> {
    context: &'a mut OwnerTextContext,
    grant: &'a OwnerTextAlignmentSelection,
}

impl SignNativeRead for AlignmentRead<'_> {
    fn read(
        &mut self,
        reference: &str,
        kind: NativeReadKind,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<u8>> {
        self.grant
            .verify_current(self.context.account_uid(), deadline, cancelled)?;
        SignNativeRead::read(
            self.context,
            reference,
            kind,
            max_bytes,
            deadline,
            cancelled,
        )
    }

    fn verify_current(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        self.grant
            .verify_current(self.context.account_uid(), deadline, cancelled)?;
        SignNativeRead::verify_current(self.context, deadline, cancelled)
    }

    fn owner_local(&self, reference: &str) -> SourceCommandResult<bool> {
        SignNativeRead::owner_local(self.context, reference)
    }
}

fn same(left: &JsonValue, right: &JsonValue) -> SourceCommandResult<bool> {
    Ok(cmd::canonical(left)? == cmd::canonical(right)?)
}

fn truth(value: &JsonValue) -> SourceCommandResult<bool> {
    match value {
        JsonValue::Bool(value) => Ok(*value),
        _ => Err(SourceCommandError::Invalid(
            "native alignment explicit boolean",
        )),
    }
}

fn selected_contracts(
    context: &OwnerTextContext,
    worker: &CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<BTreeMap<String, Digest256>> {
    let mut selected = BTreeMap::new();
    for name in [RECORD_SCHEMA, LEGACY_SCHEMA, BINDING_SCHEMA, EVENT_SCHEMA] {
        let raw = context.read(name, MAX_CONTRACT, deadline, cancelled)?;
        let digest = Digest256::of_bytes(&raw);
        if worker.contract_digest(name) != Some(digest) {
            return Err(SourceCommandError::Conflict(
                "native alignment selected contract changed",
            ));
        }
        selected.insert(name.to_owned(), digest);
    }
    Ok(selected)
}

fn configuration(
    context: &OwnerTextContext,
    grant: &OwnerTextAlignmentSelection,
    contracts: &BTreeMap<String, Digest256>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<String> {
    let selected = cmd::object(
        contracts
            .iter()
            .map(|(name, sha)| (name.as_str(), cmd::string(&sha.to_prefixed())))
            .collect(),
    );
    let binding = cmd::object(vec![
        (
            "owner_configuration_bytes",
            cmd::string(&Digest256::of_bytes(&grant.raw).to_prefixed()),
        ),
        (
            "context",
            cmd::string(&context.snapshot(deadline, cancelled)?.to_prefixed()),
        ),
        ("contracts", selected),
    ]);
    Ok(Digest256::of_bytes(&cmd::canonical(&binding)?).to_prefixed())
}

fn bindings<'a>(
    grant: &'a OwnerTextAlignmentSelection,
    role: &str,
) -> SourceCommandResult<&'a [JsonValue]> {
    let selected = cmd::array(cmd::field(&grant.config, "native_bindings")?, role)?;
    if !(1..=256).contains(&selected.len()) {
        return Err(SourceCommandError::Unsupported(
            "native alignment side count",
        ));
    }
    Ok(selected)
}

fn visibility_rank(value: &str) -> SourceCommandResult<usize> {
    [
        "public",
        "public_metadata_only",
        "controlled",
        "local_only",
        "restricted",
        "unknown",
    ]
    .iter()
    .position(|candidate| *candidate == value)
    .ok_or(SourceCommandError::Invalid("native alignment visibility"))
}

fn side(
    role: &str,
    selected: &[JsonValue],
    actual: &ResolvedOwnerAlignmentSide,
    tokenization: bool,
) -> SourceCommandResult<JsonValue> {
    let first = selected
        .first()
        .ok_or(SourceCommandError::Invalid("native alignment side absent"))?;
    for binding in selected.iter().skip(1) {
        for key in [
            "packet_ref",
            "packet_sha256",
            "packet_id",
            "packet_version",
            "segmentation_id",
            "segmentation_version",
            "text_layer",
            "source_record_refs",
        ] {
            if !same(cmd::field(binding, key)?, cmd::field(first, key)?)? {
                return Err(SourceCommandError::Conflict(
                    "native alignment side crosses segmentation",
                ));
            }
        }
    }
    let segment = cmd::array(&actual.packet, "segmentations")?
        .iter()
        .filter(|row| row.object_get("segmentation_id") == first.object_get("segmentation_id"))
        .collect::<Vec<_>>();
    if segment.len() != 1 {
        return Err(SourceCommandError::Conflict(
            "native alignment segmentation not unique",
        ));
    }
    let ordered = cmd::array(segment[0], "ordered_unit_refs")?;
    let selected_units = selected
        .iter()
        .map(|row| cmd::text(row, "unit_id"))
        .collect::<SourceCommandResult<Vec<_>>>()?;
    if selected_units
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .len()
        != selected_units.len()
        || ordered
            .iter()
            .filter_map(JsonValue::as_str)
            .filter(|id| selected_units.contains(id))
            .collect::<Vec<_>>()
            != selected_units
    {
        return Err(SourceCommandError::Conflict(
            "native alignment unit order or identity",
        ));
    }
    let mut anchor_refs = Vec::new();
    for binding in selected {
        anchor_refs.extend(
            cmd::array(binding, "ordered_anchor_refs")?
                .iter()
                .map(|row| {
                    row.as_str()
                        .ok_or(SourceCommandError::Invalid("native alignment anchor ref"))
                })
                .collect::<SourceCommandResult<Vec<_>>>()?,
        );
    }
    if anchor_refs.iter().copied().collect::<BTreeSet<_>>().len() != anchor_refs.len() {
        return Err(SourceCommandError::Conflict(
            "native alignment repeated anchor",
        ));
    }
    let packet_anchors = cmd::array(&actual.packet, "anchors")?;
    let mut anchors = Vec::with_capacity(anchor_refs.len());
    let mut ordinals = Vec::with_capacity(anchor_refs.len());
    for reference in anchor_refs {
        let matching = packet_anchors
            .iter()
            .filter(|row| {
                row.object_get("anchor_ref").and_then(JsonValue::as_str) == Some(reference)
            })
            .collect::<Vec<_>>();
        if matching.len() != 1 {
            return Err(SourceCommandError::Conflict(
                "native alignment anchor not unique",
            ));
        }
        let row = matching[0];
        ordinals.push(cmd::integer(row, "ordinal")?);
        anchors.push(cmd::object(
            [
                "anchor_ref",
                "ordinal",
                "text_layer_ref",
                "text_layer_sha256",
                "selector",
                "exact_sha256",
                "source_return",
            ]
            .iter()
            .map(|key| Ok((*key, cmd::field(row, key)?.clone())))
            .collect::<SourceCommandResult<Vec<_>>>()?,
        ));
    }
    if ordinals.windows(2).any(|pair| pair[0] > pair[1]) {
        return Err(SourceCommandError::Conflict(
            "native alignment anchor order",
        ));
    }
    let packet_ref = cmd::field(first, "packet_ref")?.clone();
    let packet_sha = cmd::field(first, "packet_sha256")?.clone();
    let frozen = cmd::object(vec![
        ("artifact_ref", packet_ref),
        ("sha256", packet_sha),
        ("state", cmd::string("frozen")),
    ]);
    let rep = cmd::field(&actual.layer, "representation")?;
    let mut rights = Vec::new();
    for row in cmd::array(rep, "rights_record_refs")? {
        rights.push(cmd::field(row, "ref")?.clone());
    }
    let visibility = actual
        .summaries
        .iter()
        .map(|summary| cmd::text(summary, "effective_visibility"))
        .collect::<SourceCommandResult<Vec<_>>>()?
        .into_iter()
        .max_by_key(|value| visibility_rank(value).unwrap_or(usize::MAX))
        .ok_or(SourceCommandError::Invalid(
            "native alignment rights summary",
        ))?;
    let scope = cmd::field(&actual.packet, "source_scope")?;
    let mut fields = vec![
        ("side_role", cmd::string(role)),
        (
            "text_layer_ref",
            cmd::field(cmd::field(first, "text_layer")?, "record_ref")?.clone(),
        ),
        (
            "text_layer_sha256",
            cmd::field(rep, "content_sha256")?.clone(),
        ),
        ("language", cmd::field(rep, "language")?.clone()),
        ("segmentation", frozen.clone()),
        (
            "tokenization",
            if tokenization {
                frozen
            } else {
                JsonValue::Null
            },
        ),
        ("anchors", JsonValue::Array(anchors)),
        ("rights_refs", JsonValue::Array(rights)),
        ("visibility", cmd::string(visibility)),
        ("publication_authorized", JsonValue::Bool(false)),
    ];
    for key in [
        "work_ref",
        "expression_ref",
        "edition_ref",
        "item_ref",
        "file_ref",
        "file_sha256",
    ] {
        fields.push((key, cmd::field(scope, key)?.clone()));
    }
    Ok(cmd::object(fields))
}

fn inherited_rights(source: &JsonValue, target: &JsonValue) -> SourceCommandResult<JsonValue> {
    let source_visibility = cmd::text(source, "visibility")?;
    let target_visibility = cmd::text(target, "visibility")?;
    let effective = [source_visibility, target_visibility, "local_only"]
        .into_iter()
        .max_by_key(|value| visibility_rank(value).unwrap_or(usize::MAX))
        .ok_or(SourceCommandError::Invalid("native alignment visibility"))?;
    let mut refs = BTreeSet::new();
    for side in [source, target] {
        for row in cmd::array(side, "rights_refs")? {
            refs.insert(
                row.as_str()
                    .ok_or(SourceCommandError::Invalid("native alignment rights ref"))?
                    .to_owned(),
            );
        }
    }
    Ok(cmd::object(vec![
        ("source_visibility", cmd::string(source_visibility)),
        ("target_visibility", cmd::string(target_visibility)),
        ("packet_visibility", cmd::string("local_only")),
        ("effective_visibility", cmd::string(effective)),
        (
            "rights_record_refs",
            JsonValue::Array(refs.iter().map(|r| cmd::string(r)).collect()),
        ),
        ("private_source_used", JsonValue::Bool(true)),
        ("publication_authorized", JsonValue::Bool(false)),
        (
            "inheritance_policy",
            cmd::string("most_restrictive_side_or_packet_wins"),
        ),
    ]))
}

fn authority_boundary() -> JsonValue {
    cmd::object(vec![
        ("tree_role", cmd::string("orientation")),
        ("graph_role", cmd::string("relation")),
        ("source_role", cmd::string("authority")),
        (
            "validators_prove_mechanics_not_truth",
            JsonValue::Bool(true),
        ),
        (
            "alignment_is_claim_not_translation_truth",
            JsonValue::Bool(true),
        ),
        (
            "machine_or_model_may_propose_not_accept",
            JsonValue::Bool(true),
        ),
        ("exports_are_not_owner_truth", JsonValue::Bool(true)),
        ("unaligned_members_are_first_class", JsonValue::Bool(true)),
        ("lexical_equivalence_not_inferred", JsonValue::Bool(true)),
        ("canon_effect", JsonValue::Bool(false)),
    ])
}

fn claim_ref(claim: &JsonValue) -> SourceCommandResult<JsonValue> {
    Ok(cmd::object(vec![
        ("claim_id", cmd::field(claim, "claim_id")?.clone()),
        ("claim_version", cmd::field(claim, "claim_version")?.clone()),
        (
            "sha256",
            cmd::string(&Digest256::of_bytes(&cmd::canonical(claim)?).to_hex()),
        ),
    ]))
}

fn record_ref(path: &str, body: &JsonValue, raw: &[u8]) -> SourceCommandResult<JsonValue> {
    Ok(cmd::object(vec![
        ("record_ref", cmd::string(path)),
        ("record_id", cmd::field(body, "record_id")?.clone()),
        (
            "record_version",
            cmd::field(body, "record_version")?.clone(),
        ),
        ("sha256", cmd::string(&Digest256::of_bytes(raw).to_hex())),
    ]))
}

fn legacy_view(body: &JsonValue) -> SourceCommandResult<JsonValue> {
    let claim = cmd::field(body, "claim")?;
    let mapping = cmd::field(claim, "mapping")?;
    let qualifications = cmd::field(claim, "qualifications")?;
    let mut alignment = vec![
        ("alignment_id", cmd::field(body, "alignment_id")?.clone()),
        ("alignment_version", cmd::number(1)),
        ("supersedes_alignment_ref", JsonValue::Null),
        (
            "identity_policy",
            cmd::string("opaque-id-independent-of-text-label-translation-and-current-mapping"),
        ),
        ("claim_id", cmd::field(claim, "claim_id")?.clone()),
        ("claim_version", cmd::number(1)),
        ("supersedes_claim_ref", JsonValue::Null),
    ];
    for key in [
        "direction",
        "correspondence_shape",
        "order_posture",
        "ordered_source_anchor_refs",
        "ordered_target_anchor_refs",
    ] {
        alignment.push((key, cmd::field(mapping, key)?.clone()));
    }
    for key in [
        "translation_techniques",
        "epistemic_status",
        "certainty",
        "status_reason",
        "maker",
        "evidence",
    ] {
        alignment.push((key, cmd::field(qualifications, key)?.clone()));
    }
    alignment.extend([
        ("status", cmd::string("proposed")),
        ("competing_alignment_refs", JsonValue::Array(vec![])),
        ("review_refs", JsonValue::Array(vec![])),
    ]);
    let packet_id = cmd::text(body, "record_id")?.replacen(
        "translation-alignment-record",
        "translation-alignment-packet",
        1,
    );
    Ok(cmd::object(vec![
        (
            "$schema",
            cmd::string(
                "https://tree-of-sophia.local/ToS/contracts/translation-alignment-packet-v1.schema.json",
            ),
        ),
        (
            "schema_version",
            cmd::string("tos_translation_alignment_packet_v1"),
        ),
        ("packet_id", cmd::string(&packet_id)),
        ("packet_version", cmd::number(1)),
        ("supersedes_packet_ref", JsonValue::Null),
        ("content_posture", cmd::string("source_bound")),
        ("granularity", cmd::field(body, "granularity")?.clone()),
        ("source_side", cmd::field(body, "source_side")?.clone()),
        ("target_side", cmd::field(body, "target_side")?.clone()),
        ("alignments", JsonValue::Array(vec![cmd::object(alignment)])),
        ("reviews", JsonValue::Array(vec![])),
        ("projections", JsonValue::Array(vec![])),
        (
            "rights_and_visibility",
            cmd::field(body, "rights_and_visibility")?.clone(),
        ),
        (
            "authority_boundary",
            cmd::field(body, "authority_boundary")?.clone(),
        ),
    ]))
}

fn description(qualifications: &JsonValue) -> SourceCommandResult<Value> {
    let mut value: Value = serde_json::from_slice(&cmd::canonical(qualifications)?)
        .map_err(|_| SourceCommandError::Invalid("native alignment description JSON"))?;
    value["maker"]
        .as_object_mut()
        .ok_or(SourceCommandError::Invalid("native alignment maker"))?
        .retain(|key, _| !matches!(key.as_str(), "made_at" | "provenance_event_ref"));
    Ok(value)
}

fn validate_record_law(
    body: &JsonValue,
    source: &JsonValue,
    target: &JsonValue,
    rights: &JsonValue,
    worker: &mut CutWorkerSchemaExecutor,
    reference: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    active(deadline, cancelled)?;
    let raw = cmd::canonical(body)?;
    if raw.len() > MAX_RECORD {
        return Err(SourceCommandError::Unsupported(
            "native alignment record byte budget",
        ));
    }
    checked_schema(worker, reference, &raw, RECORD_SCHEMA, deadline, cancelled)?;
    let legacy = legacy_view(body)?;
    let legacy_raw = cmd::canonical(&legacy)?;
    if legacy_raw.len() > MAX_RECORD {
        return Err(SourceCommandError::Unsupported(
            "native alignment legacy view budget",
        ));
    }
    checked_schema(
        worker,
        reference,
        &legacy_raw,
        LEGACY_SCHEMA,
        deadline,
        cancelled,
    )?;
    let decoded: Value = serde_json::from_slice(&legacy_raw)
        .map_err(|_| SourceCommandError::Invalid("native alignment legacy view JSON"))?;
    let report = inspect_supplied_translation_alignment(
        &decoded,
        ItemLimits {
            max_member_bytes: MAX_RECORD,
            max_total_bytes: MAX_RECORD as u64,
            max_state_bytes: 8_388_608,
            max_issues: 128,
            deadline,
        },
    )
    .map_err(|reason| SourceCommandError::SchemaExecution {
        path: reference.to_owned(),
        root: LEGACY_SCHEMA.to_owned(),
        reason,
    })?;
    if !report.issues.is_empty()
        || !report.unsupported.is_empty()
        || report.checked_predicates.is_empty()
    {
        return Err(SourceCommandError::Invalid(
            "native alignment mapping or evidence law",
        ));
    }
    if !same(cmd::field(body, "source_side")?, source)?
        || !same(cmd::field(body, "target_side")?, target)?
        || !same(cmd::field(body, "rights_and_visibility")?, rights)?
        || cmd::field(body, "publication_authorized")? != &JsonValue::Bool(false)
        || cmd::text(body, "status")? != "proposed"
    {
        return Err(SourceCommandError::Conflict(
            "native alignment exact side or rights differs",
        ));
    }
    let native = cmd::field(body, "native_bindings")?;
    let first_source = cmd::array(native, "source")?
        .first()
        .ok_or(SourceCommandError::Invalid(
            "native alignment source absent",
        ))?;
    let first_target = cmd::array(native, "target")?
        .first()
        .ok_or(SourceCommandError::Invalid(
            "native alignment target absent",
        ))?;
    if cmd::field(first_source, "packet_id")? == cmd::field(first_target, "packet_id")?
        || cmd::field(first_source, "segmentation_id")?
            == cmd::field(first_target, "segmentation_id")?
    {
        return Err(SourceCommandError::Conflict(
            "native alignment sides share packet or segmentation",
        ));
    }
    let source_units = cmd::array(native, "source")?
        .iter()
        .map(|v| cmd::text(v, "unit_id"))
        .collect::<SourceCommandResult<BTreeSet<_>>>()?;
    if cmd::array(native, "target")?
        .iter()
        .map(|v| cmd::text(v, "unit_id"))
        .collect::<SourceCommandResult<Vec<_>>>()?
        .iter()
        .any(|id| source_units.contains(id))
    {
        return Err(SourceCommandError::Conflict(
            "native alignment sides share unit",
        ));
    }
    let mapping = cmd::field(cmd::field(body, "claim")?, "mapping")?;
    if cmd::text(mapping, "order_posture")? == "monotonic" {
        for role in ["source", "target"] {
            let side = cmd::field(body, &format!("{role}_side"))?;
            let ordinals = cmd::array(side, "anchors")?
                .iter()
                .map(|row| Ok((cmd::text(row, "anchor_ref")?, cmd::integer(row, "ordinal")?)))
                .collect::<SourceCommandResult<BTreeMap<_, _>>>()?;
            let key = format!("ordered_{role}_anchor_refs");
            let positions = cmd::array(mapping, &key)?
                .iter()
                .map(|row| {
                    let id = row.as_str().ok_or(SourceCommandError::Invalid(
                        "native alignment mapping anchor",
                    ))?;
                    ordinals.get(id).copied().ok_or(SourceCommandError::Invalid(
                        "native alignment mapping anchor absent",
                    ))
                })
                .collect::<SourceCommandResult<Vec<_>>>()?;
            if positions.windows(2).any(|pair| pair[0] > pair[1]) {
                return Err(SourceCommandError::Conflict(
                    "native alignment monotonic mapping reversed",
                ));
            }
        }
    }
    Ok(())
}

struct History<'a> {
    context: &'a OwnerTextContext,
    worker: &'a mut CutWorkerSchemaExecutor,
    source: &'a JsonValue,
    target: &'a JsonValue,
    rights: &'a JsonValue,
    selected_bindings: &'a JsonValue,
    deadline: Instant,
    cancelled: &'a AtomicBool,
    cached: BTreeMap<(String, String, String, u64), JsonValue>,
    visiting: BTreeSet<(String, u64)>,
    ancestry: BTreeMap<String, Digest256>,
    observed: BTreeMap<String, (Digest256, usize)>,
    remaining_bytes: usize,
}

impl History<'_> {
    fn reference(
        &mut self,
        selected: &JsonValue,
        ancestry: bool,
    ) -> SourceCommandResult<JsonValue> {
        active(self.deadline, self.cancelled)?;
        let path = cmd::text(selected, "record_ref")?.to_owned();
        let sha = cmd::text(selected, "sha256")?.to_owned();
        let id = cmd::text(selected, "record_id")?.to_owned();
        let version = cmd::integer(selected, "record_version")?;
        let key = (path.clone(), sha.clone(), id, version);
        if let Some(cached) = self.cached.get(&key) {
            if ancestry {
                self.ancestry.insert(
                    path,
                    Digest256::from_hex(&sha)
                        .map_err(|_| SourceCommandError::Invalid("native alignment history SHA"))?,
                );
            }
            return Ok(cached.clone());
        }
        if self.cached.len() >= MAX_HISTORY || self.remaining_bytes == 0 {
            return Err(SourceCommandError::Unsupported(
                "native alignment history budget",
            ));
        }
        let raw = self.context.read(
            &path,
            self.remaining_bytes.min(MAX_RECORD),
            self.deadline,
            self.cancelled,
        )?;
        let digest = Digest256::of_bytes(&raw);
        if digest.to_hex() != sha || raw.len() > self.remaining_bytes {
            return Err(SourceCommandError::Conflict(
                "native alignment exact history bytes",
            ));
        }
        self.remaining_bytes -= raw.len();
        self.observed.insert(path.clone(), (digest, raw.len()));
        let body = cmd::parse(&raw)?;
        if !same(&record_ref(&path, &body, &raw)?, selected)? {
            return Err(SourceCommandError::Conflict(
                "native alignment history identity or version",
            ));
        }
        self.record(&body, &path)?;
        if ancestry {
            self.ancestry.insert(path, digest);
        }
        self.cached.insert(key, body.clone());
        Ok(body)
    }

    fn record(&mut self, body: &JsonValue, path: &str) -> SourceCommandResult<()> {
        let key = (
            cmd::text(body, "record_id")?.to_owned(),
            cmd::integer(body, "record_version")?,
        );
        if self.visiting.len() >= MAX_HISTORY || !self.visiting.insert(key.clone()) {
            return Err(SourceCommandError::Conflict(
                "native alignment history cyclic or over budget",
            ));
        }
        let result = self.record_inner(body, path);
        self.visiting.remove(&key);
        result
    }

    fn record_inner(&mut self, body: &JsonValue, path: &str) -> SourceCommandResult<()> {
        validate_record_law(
            body,
            self.source,
            self.target,
            self.rights,
            self.worker,
            path,
            self.deadline,
            self.cancelled,
        )?;
        if !same(cmd::field(body, "native_bindings")?, self.selected_bindings)? {
            return Err(SourceCommandError::Conflict(
                "native alignment historical source scope",
            ));
        }
        let change = cmd::text(body, "change_kind")?;
        let claim = cmd::field(body, "claim")?;
        let predecessor = cmd::field(body, "predecessor")?;
        if matches!(change, "initial" | "competing") {
            if predecessor != &JsonValue::Null {
                return Err(SourceCommandError::Conflict(
                    "native alignment initial predecessor",
                ));
            }
        } else {
            if !matches!(change, "describe" | "remap") {
                return Err(SourceCommandError::Invalid("native alignment change kind"));
            }
            let prior = self.reference(predecessor, true)?;
            if cmd::field(body, "record_id")? != cmd::field(&prior, "record_id")?
                || cmd::integer(body, "record_version")?
                    != cmd::integer(&prior, "record_version")?
                        .checked_add(1)
                        .ok_or(SourceCommandError::Invalid(
                            "native alignment record version overflow",
                        ))?
                || cmd::field(body, "alignment_id")? != cmd::field(&prior, "alignment_id")?
                || !same(
                    cmd::field(claim, "predecessor")?,
                    &claim_ref(cmd::field(&prior, "claim")?)?,
                )?
            {
                return Err(SourceCommandError::Conflict(
                    "native alignment descriptive succession",
                ));
            }
            for field in [
                "native_bindings",
                "source_side",
                "target_side",
                "granularity",
                "rights_and_visibility",
                "competing_records",
            ] {
                if !same(cmd::field(body, field)?, cmd::field(&prior, field)?)? {
                    return Err(SourceCommandError::Conflict(
                        "native alignment descriptive scope drift",
                    ));
                }
            }
            let old_claim = cmd::field(&prior, "claim")?;
            if change == "describe" {
                if cmd::field(claim, "claim_id")? != cmd::field(old_claim, "claim_id")?
                    || cmd::integer(claim, "claim_version")?
                        != cmd::integer(old_claim, "claim_version")?
                            .checked_add(1)
                            .ok_or(SourceCommandError::Invalid(
                                "native alignment Claim version overflow",
                            ))?
                    || !same(
                        cmd::field(claim, "mapping")?,
                        cmd::field(old_claim, "mapping")?,
                    )?
                    || description(cmd::field(claim, "qualifications")?)?
                        == description(cmd::field(old_claim, "qualifications")?)?
                {
                    return Err(SourceCommandError::Conflict(
                        "native alignment description unchanged or rekeyed",
                    ));
                }
            } else if cmd::field(claim, "claim_id")? == cmd::field(old_claim, "claim_id")?
                || same(
                    cmd::field(claim, "mapping")?,
                    cmd::field(old_claim, "mapping")?,
                )?
            {
                return Err(SourceCommandError::Conflict(
                    "native alignment remap needs new Claim and mapping",
                ));
            }
        }
        for competitor in cmd::array(body, "competing_records")? {
            let alternative = self.reference(competitor, false)?;
            if cmd::field(body, "alignment_id")? == cmd::field(&alternative, "alignment_id")?
                || cmd::field(body, "record_id")? == cmd::field(&alternative, "record_id")?
                || cmd::field(claim, "claim_id")?
                    == cmd::field(cmd::field(&alternative, "claim")?, "claim_id")?
                || !same(
                    cmd::field(body, "native_bindings")?,
                    cmd::field(&alternative, "native_bindings")?,
                )?
            {
                return Err(SourceCommandError::Conflict(
                    "native alignment competition scope or identity",
                ));
            }
        }
        if change == "initial" && !cmd::array(body, "competing_records")?.is_empty() {
            return Err(SourceCommandError::Conflict(
                "native alignment initial competition",
            ));
        }
        Ok(())
    }
}

fn body_from_proposal(
    grant: &OwnerTextAlignmentSelection,
    proposal: &JsonValue,
    resolved: &ResolvedOwnerAlignment,
    predecessor: Option<&JsonValue>,
) -> SourceCommandResult<(JsonValue, JsonValue, JsonValue)> {
    let operation = cmd::text(proposal, "operation")?;
    cmd::exact_keys(
        proposal,
        if operation.starts_with("prepare-") {
            &["schema_version", "operation", "mapping", "qualifications"][..]
        } else {
            &[
                "schema_version",
                "operation",
                "command_id",
                "expected_configuration",
                "expected_source",
                "expected_revision",
                "expected_dependencies",
                "mapping",
                "qualifications",
            ][..]
        },
    )?;
    let expected = if grant.operation == "alignment.create" {
        "prepare-create"
    } else {
        "prepare-revise"
    };
    if cmd::text(proposal, "schema_version")? != "tos_local_source_command_v1"
        || ![expected, grant.operation].contains(&operation)
    {
        return Err(SourceCommandError::Denied(
            "native alignment proposal operation",
        ));
    }
    let source = side(
        "source",
        bindings(grant, "source")?,
        &resolved.source,
        truth(cmd::field(
            cmd::field(&grant.config, "tokenization")?,
            "source",
        )?)?,
    )?;
    let target = side(
        "target",
        bindings(grant, "target")?,
        &resolved.target,
        truth(cmd::field(
            cmd::field(&grant.config, "tokenization")?,
            "target",
        )?)?,
    )?;
    let rights = inherited_rights(&source, &target)?;
    let qualifications = cmd::field(proposal, "qualifications")?;
    cmd::exact_keys(
        qualifications,
        &[
            "translation_techniques",
            "epistemic_status",
            "certainty",
            "status_reason",
            "evidence",
        ],
    )?;
    let mut qualification_fields = [
        "translation_techniques",
        "epistemic_status",
        "certainty",
        "status_reason",
        "evidence",
    ]
    .iter()
    .map(|key| Ok((*key, cmd::field(qualifications, key)?.clone())))
    .collect::<SourceCommandResult<Vec<_>>>()?;
    qualification_fields.push(("maker", cmd::field(&grant.config, "maker")?.clone()));
    let change = cmd::text(&grant.config, "change_kind")?;
    let record_version = predecessor
        .map(|previous| cmd::integer(previous, "record_version"))
        .transpose()?
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(SourceCommandError::Invalid(
            "native alignment record version overflow",
        ))?;
    let claim_version = if change == "describe" {
        cmd::integer(
            cmd::field(
                predecessor.ok_or(SourceCommandError::Invalid(
                    "native alignment description predecessor",
                ))?,
                "claim",
            )?,
            "claim_version",
        )?
        .checked_add(1)
        .ok_or(SourceCommandError::Invalid(
            "native alignment Claim version overflow",
        ))?
    } else {
        1
    };
    let claim = cmd::object(vec![
        ("claim_id", cmd::field(&grant.config, "claim_id")?.clone()),
        ("claim_version", cmd::number(claim_version)),
        (
            "predecessor",
            predecessor
                .map(|previous| claim_ref(cmd::field(previous, "claim")?))
                .transpose()?
                .unwrap_or(JsonValue::Null),
        ),
        ("mapping", cmd::field(proposal, "mapping")?.clone()),
        ("qualifications", cmd::object(qualification_fields)),
    ]);
    let body = cmd::object(vec![
        (
            "$schema",
            cmd::string(
                "https://tree-of-sophia.local/ToS/contracts/native-translation-alignment-record-v1.schema.json",
            ),
        ),
        (
            "schema_version",
            cmd::string("tos_native_translation_alignment_record_v1"),
        ),
        ("record_id", cmd::field(&grant.config, "record_id")?.clone()),
        ("record_version", cmd::number(record_version)),
        (
            "predecessor",
            cmd::field(&grant.config, "predecessor")?.clone(),
        ),
        (
            "change_kind",
            cmd::field(&grant.config, "change_kind")?.clone(),
        ),
        (
            "alignment_id",
            cmd::field(&grant.config, "alignment_id")?.clone(),
        ),
        (
            "granularity",
            cmd::field(&grant.config, "granularity")?.clone(),
        ),
        ("claim", claim),
        ("source_side", source.clone()),
        ("target_side", target.clone()),
        (
            "native_bindings",
            cmd::field(&grant.config, "native_bindings")?.clone(),
        ),
        (
            "competing_records",
            cmd::field(&grant.config, "competing_records")?.clone(),
        ),
        ("rights_and_visibility", rights),
        ("status", cmd::string("proposed")),
        (
            "execution_posture",
            cmd::string("supplied_mapping_captured_exact_sources_verified_not_aligner_execution"),
        ),
        (
            "assessment_posture",
            cmd::string("unassessed_translation_proposal"),
        ),
        ("publication_authorized", JsonValue::Bool(false)),
        ("authority_boundary", authority_boundary()),
    ]);
    Ok((body, source, target))
}

fn input_bytes(
    context: &OwnerTextContext,
    resolved: &ResolvedOwnerAlignment,
    history: &BTreeMap<String, (Digest256, usize)>,
    software_rows: &[Value],
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<u8>> {
    let mut inputs = BTreeMap::new();
    for input in &resolved.inputs {
        inputs.insert((input.reference.clone(), input.category), input.raw_sha256);
    }
    for (path, (sha, _)) in history {
        if let Some(previous) = inputs.insert((path.clone(), "metadata"), *sha) {
            if previous != *sha {
                return Err(SourceCommandError::Conflict(
                    "native alignment history input differs",
                ));
            }
        }
    }
    let implementation = software_rows
        .iter()
        .map(|row| {
            let name = row["artifact_ref"]
                .as_str()
                .ok_or(SourceCommandError::Invalid("native alignment software ref"))?;
            let sha = row["artifact_sha256"]
                .as_str()
                .ok_or(SourceCommandError::Invalid("native alignment software SHA"))?;
            Ok((name.to_owned(), format!("sha256:{sha}")))
        })
        .collect::<SourceCommandResult<BTreeMap<_, _>>>()?;
    if implementation.len() != software_rows.len() {
        return Err(SourceCommandError::Invalid(
            "native alignment software repeats",
        ));
    }
    let rows = inputs
        .iter()
        .map(|((path, category), sha)| json!([path, category, sha.to_hex()]))
        .collect::<Vec<_>>();
    let value = json!({"schema_version":"tos_native_construction_inputs_v1",
        "context":context.snapshot(deadline, cancelled)?.to_prefixed(),
        "inputs":rows,"implementation":implementation,
        "runtime":executable(deadline, cancelled)?.to_prefixed()});
    let raw = serde_json::to_vec(&value)
        .map_err(|_| SourceCommandError::Invalid("native alignment input JSON"))?;
    line(&cmd::parse(&raw)?)
}

fn dependencies(
    resolved: &ResolvedOwnerAlignment,
    history: &BTreeMap<String, (Digest256, usize)>,
    inventory: Digest256,
    inputs: &[u8],
) -> SourceCommandResult<String> {
    let historic = cmd::object(
        history
            .iter()
            .map(|(path, (sha, _))| (path.as_str(), cmd::string(&sha.to_prefixed())))
            .collect(),
    );
    let native = cmd::object(vec![
        ("selected", cmd::string(&resolved.input_snapshot)),
        ("history", historic),
    ]);
    let value = cmd::object(vec![
        ("native", native),
        ("identity", cmd::string(&inventory.to_prefixed())),
        (
            "retained_inputs",
            cmd::string(&Digest256::of_bytes(inputs).to_prefixed()),
        ),
    ]);
    Ok(Digest256::of_bytes(&cmd::canonical(&value)?).to_prefixed())
}

fn prepare(
    mut context: OwnerTextContext,
    grant: OwnerTextAlignmentSelection,
    request: &JsonValue,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    exclude: Option<&Path>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PreparedAlignment> {
    active(deadline, cancelled)?;
    let publication = context.select_publication(deadline, cancelled)?;
    let contracts = selected_contracts(&context, worker, deadline, cancelled)?;
    let owner_configuration = configuration(&context, &grant, &contracts, deadline, cancelled)?;
    let resolved = resolve_owner_alignment(
        &mut AlignmentRead {
            context: &mut context,
            grant: &grant,
        },
        worker,
        bindings(&grant, "source")?,
        bindings(&grant, "target")?,
        true,
        deadline,
        cancelled,
    )?;
    let source = side(
        "source",
        bindings(&grant, "source")?,
        &resolved.source,
        truth(cmd::field(
            cmd::field(&grant.config, "tokenization")?,
            "source",
        )?)?,
    )?;
    let target = side(
        "target",
        bindings(&grant, "target")?,
        &resolved.target,
        truth(cmd::field(
            cmd::field(&grant.config, "tokenization")?,
            "target",
        )?)?,
    )?;
    let rights = inherited_rights(&source, &target)?;
    let mut history = History {
        context: &context,
        worker,
        source: &source,
        target: &target,
        rights: &rights,
        selected_bindings: cmd::field(&grant.config, "native_bindings")?,
        deadline,
        cancelled,
        cached: BTreeMap::new(),
        visiting: BTreeSet::new(),
        ancestry: BTreeMap::new(),
        observed: BTreeMap::new(),
        remaining_bytes: 8_388_608,
    };
    let prior = match cmd::field(&grant.config, "predecessor")? {
        JsonValue::Null => None,
        reference => Some(history.reference(reference, true)?),
    };
    let (body, _, _) = body_from_proposal(&grant, request, &resolved, prior.as_ref())?;
    let source_path = cmd::text(&grant.config, "source_path")?;
    history.record(&body, source_path)?;
    let ancestry = std::mem::take(&mut history.ancestry);
    let history_inputs = std::mem::take(&mut history.observed);
    drop(history);
    let ids = [
        "record_id",
        "alignment_id",
        "claim_id",
        "provenance_event_id",
    ]
    .iter()
    .map(|key| cmd::text(&grant.config, key))
    .collect::<SourceCommandResult<Vec<_>>>()?;
    let inventory = selected_alignment_identity_snapshot(
        &context, &ids, exclude, &ancestry, deadline, cancelled,
    )?;
    let software_rows = selected_components(software, components, deadline, cancelled)?;
    let inputs = input_bytes(
        &context,
        &resolved,
        &history_inputs,
        &software_rows,
        deadline,
        cancelled,
    )?;
    let dependencies = dependencies(&resolved, &history_inputs, inventory, &inputs)?;
    let mut files = BTreeMap::new();
    files.insert(BASENAME.to_owned(), line(&body)?);
    files.insert(
        "source-create-owner-configuration.json".into(),
        grant.raw.clone(),
    );
    files.insert("source-create-inputs.json".into(), inputs);
    if files
        .values()
        .try_fold(0usize, |n, raw| n.checked_add(raw.len()))
        .is_none_or(|n| n > MAX_PACKAGE)
    {
        return Err(SourceCommandError::Unsupported(
            "native alignment prepared package budget",
        ));
    }
    context.verify_publication(&publication, deadline, cancelled)?;
    Ok(PreparedAlignment {
        context,
        grant,
        publication,
        resolved,
        body,
        files,
        contracts,
        inventory,
        ancestry,
        history_inputs,
        software_rows,
        owner_configuration,
        dependencies,
    })
}

impl PreparedAlignment {
    fn verify_stage_current(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        active(deadline, cancelled)?;
        self.context
            .verify_publication(&self.publication, deadline, cancelled)?;
        self.context.snapshot(deadline, cancelled)?;
        let now = OwnerTextAlignmentSelection::select(
            &self.context,
            &self.grant.path,
            deadline,
            cancelled,
        )?;
        if now.raw != self.grant.raw || !same(&now.config, &self.grant.config)? {
            return Err(SourceCommandError::Conflict(
                "native alignment grant changed",
            ));
        }
        Ok(())
    }

    fn verify_current(
        &self,
        cut: &CorpusCutReader,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        exclude: Option<&Path>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        self.verify_stage_current(deadline, cancelled)?;
        for (name, digest) in &self.contracts {
            if Digest256::of_bytes(&self.context.read(name, MAX_CONTRACT, deadline, cancelled)?)
                != *digest
            {
                return Err(SourceCommandError::Conflict(
                    "native alignment contract changed",
                ));
            }
        }
        if configuration(
            &self.context,
            &self.grant,
            &self.contracts,
            deadline,
            cancelled,
        )? != self.owner_configuration
        {
            return Err(SourceCommandError::Conflict(
                "native alignment configuration changed",
            ));
        }
        let mut metadata = 8_388_608usize;
        let mut content = 8_388_608usize;
        for input in &self.resolved.inputs {
            active(deadline, cancelled)?;
            let remaining = if input.category == "content" {
                &mut content
            } else {
                &mut metadata
            };
            let limit = if input.category == "content" {
                *remaining
            } else {
                (*remaining).min(MAX_CONTRACT)
            };
            let raw = self
                .context
                .read(&input.reference, limit, deadline, cancelled)?;
            *remaining =
                remaining
                    .checked_sub(raw.len())
                    .ok_or(SourceCommandError::Unsupported(
                        "native alignment selected reread budget",
                    ))?;
            if raw.len() != input.raw_size || Digest256::of_bytes(&raw) != input.raw_sha256 {
                return Err(SourceCommandError::Conflict(
                    "native alignment selected source changed",
                ));
            }
        }
        let mut historic = 8_388_608usize;
        for (path, (digest, size)) in &self.history_inputs {
            active(deadline, cancelled)?;
            let raw = self
                .context
                .read(path, historic.min(MAX_RECORD), deadline, cancelled)?;
            historic = historic
                .checked_sub(raw.len())
                .ok_or(SourceCommandError::Unsupported(
                    "native alignment history reread budget",
                ))?;
            if raw.len() != *size || Digest256::of_bytes(&raw) != *digest {
                return Err(SourceCommandError::Conflict(
                    "native alignment history changed",
                ));
            }
        }
        let ids = [
            "record_id",
            "alignment_id",
            "claim_id",
            "provenance_event_id",
        ]
        .iter()
        .map(|key| cmd::text(&self.grant.config, key))
        .collect::<SourceCommandResult<Vec<_>>>()?;
        if selected_alignment_identity_snapshot(
            &self.context,
            &ids,
            exclude,
            &self.ancestry,
            deadline,
            cancelled,
        )? != self.inventory
        {
            return Err(SourceCommandError::Conflict(
                "native alignment identity inventory changed",
            ));
        }
        if selected_components(software, components, deadline, cancelled)? != self.software_rows {
            return Err(SourceCommandError::Conflict(
                "native alignment selected software changed",
            ));
        }
        self.context
            .verify_publication(&self.publication, deadline, cancelled)?;
        verify_owner_text_current_cut(
            &self.context.public_root_handle()?,
            self.context.account_uid(),
            cut,
            deadline,
            cancelled,
        )?;
        self.grant
            .verify_current(self.context.account_uid(), deadline, cancelled)?;
        Ok(())
    }
}

fn request_create(
    request: &JsonValue,
    prepared: &PreparedAlignment,
    original: bool,
) -> SourceCommandResult<()> {
    cmd::exact_keys(
        request,
        &[
            "schema_version",
            "operation",
            "command_id",
            "expected_configuration",
            "expected_source",
            "expected_revision",
            "expected_dependencies",
            "mapping",
            "qualifications",
        ],
    )?;
    if cmd::text(request, "schema_version")? != "tos_local_source_command_v1"
        || cmd::text(request, "operation")? != prepared.grant.operation
        || cmd::text(request, "command_id")?.is_empty()
        || cmd::text(request, "command_id")?.len() > 256
        || cmd::text(request, "expected_configuration")? != prepared.owner_configuration
        || original && cmd::text(request, "expected_dependencies")? != prepared.dependencies
        || cmd::field(request, "expected_source")? != &JsonValue::Null
        || cmd::field(request, "expected_revision")? != &JsonValue::Null
    {
        return Err(SourceCommandError::Conflict(
            "native alignment exact creation request",
        ));
    }
    Ok(())
}

/// Preview is an independently bounded preparation, not authority to write.
pub fn prepare_owner_alignment_from_captures(
    context_path: &Path,
    grant_path: &Path,
    proposal: &JsonValue,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<NativeAlignmentPreview> {
    let deadline = deadline.min(Instant::now() + Duration::from_secs(60));
    let (context, grant) =
        selected_owner(context_path, grant_path, cut, worker, deadline, cancelled)?;
    let prepared = prepare(
        context, grant, proposal, software, components, worker, None, deadline, cancelled,
    )?;
    finish_creation_worker(worker, deadline, cancelled)?;
    prepared.verify_current(cut, software, components, None, deadline, cancelled)?;
    let target = prepared
        .context
        .private_new_package_target(cmd::text(&prepared.grant.config, "source_path")?)?;
    let target_exists = match target.symlink_metadata() {
        Ok(metadata)
            if metadata.is_dir()
                && metadata.uid() == prepared.context.account_uid()
                && metadata.mode() & 0o7777 == 0o700 =>
        {
            true
        }
        Ok(_) => {
            return Err(SourceCommandError::Denied(
                "native alignment preview target unsafe",
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(_) => {
            return Err(SourceCommandError::Denied(
                "native alignment preview target",
            ));
        }
    };
    Ok(NativeAlignmentPreview {
        owner_configuration: prepared.owner_configuration,
        expected_dependencies: prepared.dependencies,
        source_path: cmd::text(&prepared.grant.config, "source_path")?.to_owned(),
        target_exists,
    })
}

fn capture_inputs(
    prepared: &PreparedAlignment,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<Value>> {
    let selected_packets = ["source", "target"]
        .iter()
        .map(|role| cmd::text(&bindings(&prepared.grant, role)?[0], "packet_ref"))
        .collect::<SourceCommandResult<BTreeSet<_>>>()?;
    let at = instant()?;
    let mut rows = Vec::new();
    for input in &prepared.resolved.inputs {
        active(deadline, cancelled)?;
        if input.category != "content" && !selected_packets.contains(input.reference.as_str()) {
            continue;
        }
        rows.push(json!({"entity_ref":input.reference,
            "role":"verified-exact-native-alignment-input",
            "sha256":input.raw_sha256.to_hex(),"size_bytes":input.raw_size,
            "media_type":if input.category == "content" {"text/plain; charset=utf-8"} else {"application/json"},
            "availability":"owner_local","content_disclosure":"private_content",
            "fixity_verified":true,"fixity_verified_at":at}));
    }
    if rows.is_empty() || rows.len() > 128 {
        return Err(SourceCommandError::Unsupported(
            "native alignment capture entity budget",
        ));
    }
    Ok(rows)
}

fn capture_rights(prepared: &PreparedAlignment) -> SourceCommandResult<JsonValue> {
    let mut rights = BTreeMap::new();
    for side in [&prepared.resolved.source, &prepared.resolved.target] {
        let rep = cmd::field(&side.layer, "representation")?;
        for row in cmd::array(rep, "rights_record_refs")? {
            let name = cmd::text(row, "ref")?;
            if let Some(existing) = rights.insert(name.to_owned(), row.clone()) {
                if !same(&existing, row)? {
                    return Err(SourceCommandError::Conflict(
                        "native alignment rights ref collision",
                    ));
                }
            }
        }
    }
    Ok(JsonValue::Array(rights.into_values().collect()))
}

fn verify_retained(
    prepared: &PreparedAlignment,
    request: &JsonValue,
    files: &BTreeMap<String, Vec<u8>>,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    active(deadline, cancelled)?;
    if files.len() != FILES.len()
        || files.keys().any(|name| !FILES.contains(&name.as_str()))
        || prepared
            .files
            .iter()
            .any(|(name, raw)| files.get(name) != Some(raw))
        || files.get("source-create-request.json").map(Vec::as_slice)
            != Some(line(request)?.as_slice())
    {
        return Err(SourceCommandError::Conflict(
            "native alignment retained package differs",
        ));
    }
    let receipt_raw =
        files
            .get("source-create-receipt.json")
            .ok_or(SourceCommandError::Conflict(
                "native alignment receipt absent",
            ))?;
    let receipt = cmd::parse(receipt_raw)?;
    cmd::exact_keys(
        &receipt,
        &[
            "schema_version",
            "command_id",
            "request_digest",
            "principal_id",
            "authority_ref",
            "owner_configuration",
            "recorded_at",
            "source_path",
            "source",
            "dependencies",
            "files",
            "grants_admission",
        ],
    )?;
    let path = cmd::text(&prepared.grant.config, "source_path")?;
    let source_ref = cmd::reference(&prepared.body, "record_id", "record_version")?;
    if cmd::text(&receipt, "schema_version")? != "tos_local_source_create_receipt_v1"
        || !same(
            cmd::field(&receipt, "command_id")?,
            cmd::field(request, "command_id")?,
        )?
        || cmd::text(&receipt, "request_digest")?
            != Digest256::of_bytes(&cmd::canonical(request)?).to_prefixed()
        || !same(
            cmd::field(&receipt, "principal_id")?,
            cmd::field(&prepared.grant.config, "principal_id")?,
        )?
        || !same(
            cmd::field(&receipt, "authority_ref")?,
            cmd::field(&prepared.grant.config, "authority_ref")?,
        )?
        || cmd::text(&receipt, "owner_configuration")? != prepared.owner_configuration
        || cmd::text(&receipt, "source_path")? != path
        || cmd::text(&receipt, "dependencies")? != cmd::text(request, "expected_dependencies")?
        || !same(cmd::field(&receipt, "source")?, &source_ref)?
        || !same(
            cmd::field(&receipt, "files")?,
            &file_refs(
                files
                    .iter()
                    .filter(|(name, _)| name.as_str() != "source-create-receipt.json"),
            ),
        )?
        || cmd::field(&receipt, "grants_admission")? != &JsonValue::Bool(false)
        || line(&receipt)? != *receipt_raw
    {
        return Err(SourceCommandError::Conflict(
            "native alignment retained receipt differs",
        ));
    }
    cmd::validate_instant(cmd::text(&receipt, "recorded_at")?)?;
    let home = path
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("native alignment home"))?
        .0;
    let event_raw =
        files
            .get("source-create-provenance.jsonl")
            .ok_or(SourceCommandError::Conflict(
                "native alignment event absent",
            ))?;
    checked_schema(
        worker,
        &format!("{home}/source-create-provenance.jsonl"),
        event_raw,
        EVENT_SCHEMA,
        deadline,
        cancelled,
    )?;
    let event = cmd::parse(event_raw)?;
    if cmd::text(&event, "event_id")? != cmd::text(&prepared.grant.config, "provenance_event_id")?
        || cmd::text(cmd::field(&event, "record_binding")?, "manifest_ref")?
            != format!("{home}/source-create-receipt.json")
        || cmd::text(
            cmd::field(cmd::field(&event, "method")?, "procedure")?,
            "name",
        )? != "native-supplied-alignment-proposal-capture"
    {
        return Err(SourceCommandError::Conflict(
            "native alignment event binding differs",
        ));
    }
    let decoded: Value = serde_json::from_slice(event_raw)
        .map_err(|_| SourceCommandError::Invalid("native alignment event JSON"))?;
    if !tos_validation::provenance_rules::semantic_issues(&decoded, 128, deadline)
        .map_err(|_| SourceCommandError::Invalid("native alignment event semantics"))?
        .is_empty()
    {
        return Err(SourceCommandError::Invalid(
            "native alignment event semantics",
        ));
    }
    let method = cmd::field(&event, "method")?;
    let grant_raw =
        files
            .get("source-create-owner-configuration.json")
            .ok_or(SourceCommandError::Conflict(
                "native alignment grant absent",
            ))?;
    let environment_raw =
        files
            .get("source-create-environment.json")
            .ok_or(SourceCommandError::Conflict(
                "native alignment environment absent",
            ))?;
    let config = cmd::field(method, "configuration_binding")?;
    let env = cmd::field(
        cmd::field(method, "environment")?,
        "environment_profile_binding",
    )?;
    if cmd::text(config, "ref")? != format!("{home}/source-create-owner-configuration.json")
        || cmd::text(config, "sha256")? != Digest256::of_bytes(grant_raw).to_hex()
        || cmd::text(env, "ref")? != format!("{home}/source-create-environment.json")
        || cmd::text(env, "sha256")? != Digest256::of_bytes(environment_raw).to_hex()
    {
        return Err(SourceCommandError::Conflict(
            "native alignment capture environment differs",
        ));
    }
    let software = &decoded["method"]["software_components"];
    let rows = software.as_array().ok_or(SourceCommandError::Invalid(
        "native alignment software rows",
    ))?;
    let input_document = cmd::parse(files.get("source-create-inputs.json").ok_or(
        SourceCommandError::Conflict("native alignment input document absent"),
    )?)?;
    let runtime = cmd::text(&input_document, "runtime")?;
    if rows.len() != prepared.software_rows.len() + 1
        || rows[..prepared.software_rows.len()] != prepared.software_rows[..]
        || rows
            .last()
            .and_then(|row| row["artifact_sha256"].as_str())
            .is_none_or(|sha| format!("sha256:{sha}") != runtime)
    {
        return Err(SourceCommandError::Conflict(
            "native alignment selected software differs",
        ));
    }
    let rights = capture_rights(prepared)?;
    if !same(
        cmd::field(
            cmd::field(&event, "rights_and_visibility")?,
            "rights_record_bindings",
        )?,
        &rights,
    )? {
        return Err(SourceCommandError::Conflict(
            "native alignment captured rights differ",
        ));
    }
    let mut internal = BTreeSet::new();
    let mut external = BTreeMap::new();
    for group in ["inputs", "outputs", "byproducts"] {
        for row in cmd::array(cmd::field(&event, "entities")?, group)? {
            active(deadline, cancelled)?;
            let reference = cmd::text(row, "entity_ref")?;
            if let Some(name) = reference.strip_prefix(&format!("{home}/")) {
                let raw = files.get(name).ok_or(SourceCommandError::Conflict(
                    "native alignment internal entity absent",
                ))?;
                if !internal.insert(name.to_owned())
                    || cmd::integer(row, "size_bytes")? != raw.len() as u64
                    || cmd::text(row, "sha256")? != Digest256::of_bytes(raw).to_hex()
                {
                    return Err(SourceCommandError::Conflict(
                        "native alignment internal entity bytes",
                    ));
                }
            } else if group == "inputs" {
                if external
                    .insert(
                        reference.to_owned(),
                        (
                            cmd::text(row, "sha256")?.to_owned(),
                            cmd::integer(row, "size_bytes")?,
                        ),
                    )
                    .is_some()
                {
                    return Err(SourceCommandError::Conflict(
                        "native alignment repeated external entity",
                    ));
                }
            } else {
                return Err(SourceCommandError::Conflict(
                    "native alignment external output",
                ));
            }
        }
    }
    let expected_internal = files
        .keys()
        .filter(|name| {
            ![
                "source-create-provenance.jsonl",
                "source-create-receipt.json",
            ]
            .contains(&name.as_str())
        })
        .cloned()
        .collect::<BTreeSet<_>>();
    if internal != expected_internal {
        return Err(SourceCommandError::Conflict(
            "native alignment event file closure",
        ));
    }
    let expected_external = capture_inputs(prepared, deadline, cancelled)?
        .into_iter()
        .map(|row| {
            Ok((
                row["entity_ref"]
                    .as_str()
                    .ok_or(SourceCommandError::Invalid("native alignment external ref"))?
                    .to_owned(),
                (
                    row["sha256"]
                        .as_str()
                        .ok_or(SourceCommandError::Invalid("native alignment external SHA"))?
                        .to_owned(),
                    row["size_bytes"]
                        .as_u64()
                        .ok_or(SourceCommandError::Invalid(
                            "native alignment external size",
                        ))?,
                ),
            ))
        })
        .collect::<SourceCommandResult<BTreeMap<_, _>>>()?;
    if external != expected_external {
        return Err(SourceCommandError::Conflict(
            "native alignment external capture differs",
        ));
    }
    Ok(receipt)
}

/// One complete private construction or exact original-command replay. The
/// owner-selected proposal is rebuilt from current source and grants; only
/// the retained protected package can authorize a resumed stage.
pub fn execute_owner_alignment_from_captures(
    context_path: &Path,
    grant_path: &Path,
    request: &JsonValue,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<NativeAlignmentResult> {
    let deadline = deadline.min(Instant::now() + Duration::from_secs(60));
    active(deadline, cancelled)?;
    let (context, grant) =
        selected_owner(context_path, grant_path, cut, worker, deadline, cancelled)?;
    let source_path = cmd::text(&grant.config, "source_path")?.to_owned();
    let target = context.private_new_package_target(&source_path)?;
    let custody =
        observe_private_text(&context, &source_path, request, &FILES, deadline, cancelled)?;
    let exclude = if matches!(&custody, PrivateTextCustody::Published(_)) {
        Some(target.as_path())
    } else {
        None
    };
    let mut prepared = prepare(
        context, grant, request, software, components, worker, exclude, deadline, cancelled,
    )?;
    request_create(
        request,
        &prepared,
        matches!(&custody, PrivateTextCustody::Absent),
    )?;
    match custody {
        PrivateTextCustody::Published(files) => {
            let receipt = verify_retained(&prepared, request, &files, worker, deadline, cancelled)?;
            finish_creation_worker(worker, deadline, cancelled)?;
            let _locks = PrivateTextLocks::acquire(&prepared.context, deadline, cancelled)?;
            prepared.verify_stage_current(deadline, cancelled)?;
            let current = observe_private_text(
                &prepared.context,
                &source_path,
                request,
                &FILES,
                deadline,
                cancelled,
            )?;
            if !matches!(current, PrivateTextCustody::Published(ref raw) if raw == &files) {
                return Err(SourceCommandError::Conflict(
                    "native alignment replay package changed",
                ));
            }
            prepared.verify_current(
                cut,
                software,
                components,
                Some(&target),
                deadline,
                cancelled,
            )?;
            return Ok(NativeAlignmentResult {
                receipt,
                replayed: true,
                grants_admission: false,
                aligner_executed: false,
            });
        }
        PrivateTextCustody::Pending(files) => {
            let receipt = verify_retained(&prepared, request, &files, worker, deadline, cancelled)?;
            finish_creation_worker(worker, deadline, cancelled)?;
            // Preparation already resolved the selected inputs under both rights.
            // Staging authenticates the current owner/epoch; the publisher makes
            // the sole complete current-input check under its final lock hold.
            prepared.verify_stage_current(deadline, cancelled)?;
            publish_private_text(
                &prepared.context,
                &source_path,
                request,
                &files,
                || prepared.verify_stage_current(deadline, cancelled),
                || prepared.verify_current(cut, software, components, None, deadline, cancelled),
                None,
                deadline,
                cancelled,
            )?;
            return Ok(NativeAlignmentResult {
                receipt,
                replayed: true,
                grants_admission: false,
                aligner_executed: false,
            });
        }
        PrivateTextCustody::Absent => (),
    }
    let home = source_path
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("native alignment home"))?
        .0;
    let inputs = capture_inputs(&prepared, deadline, cancelled)?;
    let rights = capture_rights(&prepared)?;
    capture_owner_alignment(
        request,
        cmd::text(&prepared.grant.config, "provenance_event_id")?,
        home,
        &source_path,
        &rights,
        inputs,
        &mut prepared.files,
        software,
        components,
        deadline,
        cancelled,
    )?;
    let event_raw = prepared
        .files
        .get("source-create-provenance.jsonl")
        .ok_or(SourceCommandError::Invalid("native alignment event output"))?;
    checked_schema(
        worker,
        &format!("{home}/source-create-provenance.jsonl"),
        event_raw,
        EVENT_SCHEMA,
        deadline,
        cancelled,
    )?;
    let receipt = cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_local_source_create_receipt_v1"),
        ),
        ("command_id", cmd::field(request, "command_id")?.clone()),
        (
            "request_digest",
            cmd::string(&Digest256::of_bytes(&cmd::canonical(request)?).to_prefixed()),
        ),
        (
            "principal_id",
            cmd::field(&prepared.grant.config, "principal_id")?.clone(),
        ),
        (
            "authority_ref",
            cmd::field(&prepared.grant.config, "authority_ref")?.clone(),
        ),
        (
            "owner_configuration",
            cmd::string(&prepared.owner_configuration),
        ),
        ("recorded_at", cmd::string(&instant()?)),
        ("source_path", cmd::string(&source_path)),
        (
            "source",
            cmd::reference(&prepared.body, "record_id", "record_version")?,
        ),
        ("dependencies", cmd::string(&prepared.dependencies)),
        ("files", file_refs(prepared.files.iter())),
        ("grants_admission", JsonValue::Bool(false)),
    ]);
    let receipt_raw = line(&receipt)?;
    let receipt = cmd::parse(&receipt_raw)?;
    prepared
        .files
        .insert("source-create-receipt.json".into(), receipt_raw);
    if prepared
        .files
        .values()
        .try_fold(0usize, |n, raw| n.checked_add(raw.len()))
        .is_none_or(|n| n > MAX_PACKAGE)
    {
        return Err(SourceCommandError::Unsupported(
            "native alignment complete package budget",
        ));
    }
    finish_creation_worker(worker, deadline, cancelled)?;
    prepared.verify_stage_current(deadline, cancelled)?;
    publish_private_text(
        &prepared.context,
        &source_path,
        request,
        &prepared.files,
        || prepared.verify_stage_current(deadline, cancelled),
        || prepared.verify_current(cut, software, components, None, deadline, cancelled),
        None,
        deadline,
        cancelled,
    )?;
    Ok(NativeAlignmentResult {
        receipt,
        replayed: false,
        grants_admission: false,
        aligner_executed: false,
    })
}

fn selected_owner(
    context_path: &Path,
    grant_path: &Path,
    cut: &CorpusCutReader,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(OwnerTextContext, OwnerTextAlignmentSelection)> {
    active(deadline, cancelled)?;
    let schema = RelativePath::parse(CONTEXT_SCHEMA)
        .map_err(|_| SourceCommandError::Invalid("native alignment context schema path"))?;
    let selected = cut
        .read_member(
            cut.current().revision(),
            &schema,
            MAX_CONTRACT as u64,
            deadline,
            cancelled,
        )
        .map_err(|_| SourceCommandError::Conflict("native alignment context contract"))?;
    let (context, _) =
        OwnerTextContext::select(context_path, &selected.raw, worker, deadline, cancelled)?;
    let grant = OwnerTextAlignmentSelection::select(&context, grant_path, deadline, cancelled)?;
    Ok((context, grant))
}

pub(crate) struct NativeAlignmentCliProfile {
    pub(crate) context: OwnerTextContext,
    pub(crate) grant: OwnerTextAlignmentSelection,
    pub(crate) owner_configuration: String,
    pub(crate) delegated_operation: &'static str,
    pub(crate) source_path: String,
    pub(crate) target_exists: bool,
}

/// Descriptive selected owner facts for the opt-in local CLI response. The
/// actual read operation consumes this same protected selection; no second
/// profile can diverge from the owner facts reported in the response.
pub(crate) fn selected_owner_cli_profile(
    context_path: &Path,
    grant_path: &Path,
    cut: &CorpusCutReader,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<NativeAlignmentCliProfile> {
    let (context, grant) =
        selected_owner(context_path, grant_path, cut, worker, deadline, cancelled)?;
    let contracts = selected_contracts(&context, worker, deadline, cancelled)?;
    let owner_configuration = configuration(&context, &grant, &contracts, deadline, cancelled)?;
    let source_path = cmd::text(&grant.config, "source_path")?.to_owned();
    let target = context.private_new_package_target(&source_path)?;
    let target_exists = match target.symlink_metadata() {
        Ok(metadata)
            if metadata.is_dir()
                && metadata.uid() == context.account_uid()
                && metadata.mode() & 0o7777 == 0o700 =>
        {
            true
        }
        Ok(_) => {
            return Err(SourceCommandError::Denied(
                "native alignment CLI target unsafe",
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(_) => return Err(SourceCommandError::Denied("native alignment CLI target")),
    };
    Ok(NativeAlignmentCliProfile {
        delegated_operation: grant.operation,
        context,
        grant,
        owner_configuration,
        source_path,
        target_exists,
    })
}

/// Discovery of this one protected delegation never reads a representation.
pub fn describe_owner_alignment_from_cut(
    context_path: &Path,
    grant_path: &Path,
    cut: &CorpusCutReader,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<NativeAlignmentDescription> {
    let deadline = deadline.min(Instant::now() + Duration::from_secs(60));
    let (context, grant) =
        selected_owner(context_path, grant_path, cut, worker, deadline, cancelled)?;
    describe_owner_alignment_selected(context, grant, worker, deadline, cancelled)
}

pub(crate) fn describe_owner_alignment_selected(
    context: OwnerTextContext,
    grant: OwnerTextAlignmentSelection,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<NativeAlignmentDescription> {
    let target = context.private_new_package_target(cmd::text(&grant.config, "source_path")?)?;
    let exists = match target.symlink_metadata() {
        Ok(metadata)
            if metadata.is_dir()
                && metadata.uid() == context.account_uid()
                && metadata.mode() & 0o7777 == 0o700 =>
        {
            true
        }
        Ok(_) => return Err(SourceCommandError::Denied("native alignment target unsafe")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(_) => {
            return Err(SourceCommandError::Denied(
                "native alignment target observation",
            ));
        }
    };
    finish_creation_worker(worker, deadline, cancelled)?;
    Ok(NativeAlignmentDescription {
        delegated_operation: grant.operation.to_owned(),
        target_exists: exists,
        content_disclosure: "withheld",
        grants_admission: false,
        aligner_executed: false,
    })
}

fn selected_inspection_record(
    context: &OwnerTextContext,
    grant: &OwnerTextAlignmentSelection,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(JsonValue, JsonValue)> {
    let source_path = cmd::text(&grant.config, "source_path")?;
    let target = context.private_new_package_target(source_path)?;
    match target.symlink_metadata() {
        Ok(_) => {
            let home = source_path
                .rsplit_once('/')
                .ok_or(SourceCommandError::Invalid(
                    "native alignment inspection home",
                ))?
                .0;
            let files =
                context.private_package(home, &FILES, MAX_PACKAGE + 2_048, deadline, cancelled)?;
            if files
                .values()
                .try_fold(0usize, |sum, raw| sum.checked_add(raw.len()))
                .is_none_or(|sum| sum > MAX_PACKAGE)
            {
                return Err(SourceCommandError::Unsupported(
                    "native alignment inspected package budget",
                ));
            }
            let raw = files.get(BASENAME).ok_or(SourceCommandError::Conflict(
                "native alignment inspected record absent",
            ))?;
            let body = cmd::parse(raw)?;
            let retained_config =
                cmd::parse(files.get("source-create-owner-configuration.json").ok_or(
                    SourceCommandError::Conflict("native alignment inspected grant absent"),
                )?)?;
            let receipt_raw =
                files
                    .get("source-create-receipt.json")
                    .ok_or(SourceCommandError::Conflict(
                        "native alignment inspected receipt absent",
                    ))?;
            let receipt = cmd::parse(receipt_raw)?;
            let event_raw =
                files
                    .get("source-create-provenance.jsonl")
                    .ok_or(SourceCommandError::Conflict(
                        "native alignment inspected event absent",
                    ))?;
            checked_schema(
                worker,
                &format!("{home}/source-create-provenance.jsonl"),
                event_raw,
                EVENT_SCHEMA,
                deadline,
                cancelled,
            )?;
            if !same(
                cmd::field(&receipt, "files")?,
                &file_refs(
                    files
                        .iter()
                        .filter(|(name, _)| name.as_str() != "source-create-receipt.json"),
                ),
            )? || !same(
                cmd::field(&receipt, "source")?,
                &cmd::reference(&body, "record_id", "record_version")?,
            )? || cmd::text(&receipt, "owner_configuration")?.is_empty()
                || !same(
                    cmd::field(&retained_config, "source_path")?,
                    cmd::field(&grant.config, "source_path")?,
                )?
                || !same(
                    cmd::field(&body, "record_id")?,
                    cmd::field(&grant.config, "record_id")?,
                )?
                || !same(
                    cmd::field(&body, "alignment_id")?,
                    cmd::field(&grant.config, "alignment_id")?,
                )?
                || !same(
                    cmd::field(&body, "native_bindings")?,
                    cmd::field(&grant.config, "native_bindings")?,
                )?
                || line(&receipt)? != *receipt_raw
            {
                return Err(SourceCommandError::Conflict(
                    "native alignment inspected package binding",
                ));
            }
            Ok((record_ref(source_path, &body, raw)?, body))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let selected = cmd::field(&grant.config, "predecessor")?;
            if selected == &JsonValue::Null {
                return Err(SourceCommandError::Conflict(
                    "native alignment inspected record absent",
                ));
            }
            let path = cmd::text(selected, "record_ref")?;
            let raw = context.read(path, MAX_RECORD, deadline, cancelled)?;
            let body = cmd::parse(&raw)?;
            if !same(selected, &record_ref(path, &body, &raw)?)? {
                return Err(SourceCommandError::Conflict(
                    "native alignment predecessor selection",
                ));
            }
            Ok((selected.clone(), body))
        }
        Err(_) => Err(SourceCommandError::Denied(
            "native alignment inspection target unsafe",
        )),
    }
}

/// Inspect the delegated current record or one exact member of its verified
/// immutable ancestry. Only metadata/rights bindings are resolved here.
pub fn inspect_owner_alignment_from_cut(
    context_path: &Path,
    grant_path: &Path,
    exact_version: Option<&JsonValue>,
    cut: &CorpusCutReader,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<NativeAlignmentInspection> {
    let deadline = deadline.min(Instant::now() + Duration::from_secs(60));
    let (context, grant) =
        selected_owner(context_path, grant_path, cut, worker, deadline, cancelled)?;
    inspect_owner_alignment_selected(context, grant, exact_version, worker, deadline, cancelled)
}

pub(crate) fn inspect_owner_alignment_selected(
    mut context: OwnerTextContext,
    grant: OwnerTextAlignmentSelection,
    exact_version: Option<&JsonValue>,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<NativeAlignmentInspection> {
    let publication = context.select_publication(deadline, cancelled)?;
    let (current_ref, body) =
        selected_inspection_record(&context, &grant, worker, deadline, cancelled)?;
    if cmd::field(&body, "record_id")? != cmd::field(&grant.config, "record_id")?
        || cmd::field(&body, "alignment_id")? != cmd::field(&grant.config, "alignment_id")?
        || !same(
            cmd::field(&body, "native_bindings")?,
            cmd::field(&grant.config, "native_bindings")?,
        )?
    {
        return Err(SourceCommandError::Denied(
            "native alignment inspection scope",
        ));
    }
    let resolved = resolve_owner_alignment(
        &mut AlignmentRead {
            context: &mut context,
            grant: &grant,
        },
        worker,
        bindings(&grant, "source")?,
        bindings(&grant, "target")?,
        false,
        deadline,
        cancelled,
    )?;
    let source = side(
        "source",
        bindings(&grant, "source")?,
        &resolved.source,
        truth(cmd::field(
            cmd::field(&grant.config, "tokenization")?,
            "source",
        )?)?,
    )?;
    let target = side(
        "target",
        bindings(&grant, "target")?,
        &resolved.target,
        truth(cmd::field(
            cmd::field(&grant.config, "tokenization")?,
            "target",
        )?)?,
    )?;
    let rights = inherited_rights(&source, &target)?;
    let mut history = History {
        context: &context,
        worker,
        source: &source,
        target: &target,
        rights: &rights,
        selected_bindings: cmd::field(&grant.config, "native_bindings")?,
        deadline,
        cancelled,
        cached: BTreeMap::new(),
        visiting: BTreeSet::new(),
        ancestry: BTreeMap::new(),
        observed: BTreeMap::new(),
        remaining_bytes: 8_388_608,
    };
    history.record(&body, cmd::text(&current_ref, "record_ref")?)?;
    let latest = (current_ref, body);
    let mut selected =
        if exact_version.is_none_or(|expected| same(expected, &latest.0).unwrap_or(false)) {
            Some(latest.clone())
        } else {
            None
        };
    let mut cursor = latest;
    let mut depth = 1usize;
    loop {
        let prior = cmd::field(&cursor.1, "predecessor")?;
        if prior == &JsonValue::Null {
            break;
        }
        let prior = prior.clone();
        let previous = history.reference(&prior, true)?;
        depth += 1;
        if exact_version.is_some_and(|expected| same(expected, &prior).unwrap_or(false)) {
            selected = Some((prior.clone(), previous.clone()));
        }
        cursor = (prior, previous);
    }
    let selected = selected.ok_or(SourceCommandError::Denied(
        "native alignment version outside delegated history",
    ))?;
    drop(history);
    context.verify_publication(&publication, deadline, cancelled)?;
    finish_creation_worker(worker, deadline, cancelled)?;
    let mapping = cmd::field(cmd::field(&selected.1, "claim")?, "mapping")?;
    let summary = cmd::object(vec![
        (
            "granularity",
            cmd::field(&selected.1, "granularity")?.clone(),
        ),
        (
            "correspondence_shape",
            cmd::field(mapping, "correspondence_shape")?.clone(),
        ),
        (
            "order_posture",
            cmd::field(mapping, "order_posture")?.clone(),
        ),
        (
            "source_member_count",
            cmd::number(cmd::array(mapping, "ordered_source_anchor_refs")?.len() as u64),
        ),
        (
            "target_member_count",
            cmd::number(cmd::array(mapping, "ordered_target_anchor_refs")?.len() as u64),
        ),
    ]);
    Ok(NativeAlignmentInspection {
        record_version: cmd::integer(&selected.1, "record_version")?,
        claim_version: cmd::integer(cmd::field(&selected.1, "claim")?, "claim_version")?,
        change_kind: cmd::text(&selected.1, "change_kind")?.to_owned(),
        history_depth: depth,
        inspected_source_sha256: cmd::text(&selected.0, "sha256")?.to_owned(),
        mapping_summary: summary,
        competing_forward_count: cmd::array(&selected.1, "competing_records")?.len(),
        metadata_verified: true,
        content_verified: false,
        grants_admission: false,
    })
}

/// Inspect an exact retained construction by command ID without resuming it.
/// The ordinary create entry resumes a pending plan only after rebuilding
/// the original request against current source, grants and software.
pub fn inspect_owner_alignment_recovery_from_cut(
    context_path: &Path,
    grant_path: &Path,
    target_ref: &str,
    command_id: &str,
    cut: &CorpusCutReader,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<NativeAlignmentRecovery> {
    let deadline = deadline.min(Instant::now() + Duration::from_secs(60));
    let (context, grant) =
        selected_owner(context_path, grant_path, cut, worker, deadline, cancelled)?;
    inspect_owner_alignment_recovery_selected(
        context, grant, target_ref, command_id, worker, deadline, cancelled,
    )
}

pub(crate) fn inspect_owner_alignment_recovery_selected(
    context: OwnerTextContext,
    grant: OwnerTextAlignmentSelection,
    target_ref: &str,
    command_id: &str,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<NativeAlignmentRecovery> {
    if cmd::text(&grant.config, "source_path")? != target_ref {
        return Err(SourceCommandError::Denied(
            "native alignment recovery target",
        ));
    }
    let Some((request, retained_files)) =
        alignment_recovery_request(&context, target_ref, command_id, deadline, cancelled)?
    else {
        let target = context.private_new_package_target(target_ref)?;
        match target.symlink_metadata() {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            _ => {
                return Err(SourceCommandError::Conflict(
                    "native alignment recovery control absent beside target",
                ));
            }
        }
        finish_creation_worker(worker, deadline, cancelled)?;
        return Ok(NativeAlignmentRecovery::Absent);
    };
    let custody =
        observe_private_text(&context, target_ref, &request, &FILES, deadline, cancelled)?;
    let was_committed = match custody {
        PrivateTextCustody::Published(files) if files == retained_files => true,
        PrivateTextCustody::Pending(files) if files == retained_files => false,
        _ => {
            return Err(SourceCommandError::Conflict(
                "native alignment retained control and selected package differ",
            ));
        }
    };
    finish_creation_worker(worker, deadline, cancelled)?;
    Ok(if was_committed {
        NativeAlignmentRecovery::Committed
    } else {
        NativeAlignmentRecovery::RetainedExactPlan
    })
}
