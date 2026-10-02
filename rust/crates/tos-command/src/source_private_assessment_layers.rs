//! Source-only adapter for the maintained confidential native TextLayer
//! (v5) and page-image OCR (v6) assessment comparisons.  It selects no review,
//! writes no journal, and grants no publication, disclosure, or canon right.

use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_sign_native::{
    NativeInput, NativeReadKind, ResolvedDerivedTextSource, ResolvedInitialTextSource,
    ResolvedOwnerTextLayer, resolve_derived_owner_text_source, resolve_initial_owner_text_source,
    resolve_owner_text_layer,
};
use crate::source_text_layer_payload::{
    AcquiredMember, PayloadIdentity, file_identity, owned_file, parents, read_acquired_epub_member,
    verify_acquired_file,
};
use crate::source_text_layer_xml::{extract_xhtml_text, validate_extraction_profile};
use crate::source_text_owner::{
    OwnerTextContext, OwnerTextInitialLayerSelection, normalized_absolute,
};
use crate::source_text_owner_ocr;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonValue};
use tos_source_store::CorpusCutReader;
use tos_validation::assessment::{
    AssessmentLayerQualityObservation, AssessmentRecordInput, MAX_RECORD_BYTES,
};
use tos_validation::source_cut::CutWorkerSchemaExecutor;

const LAYER_SCHEMA: &str = "ToS/contracts/source-text-layer.schema.json";
const BINDING_SCHEMA: &str = "ToS/contracts/native-text-layer-binding.schema.json";
const TEXT_COMPARISON_SCHEMA: &str = "ToS/contracts/native-text-layer-comparison.schema.json";
const DERIVATION_COMPARISON_SCHEMA: &str =
    "ToS/contracts/native-text-layer-derivation-comparison.schema.json";
const PAGE_COMPARISON_SCHEMA: &str = "ToS/contracts/native-page-ocr-comparison.schema.json";
const ANCHOR_SCHEMA: &str = "ToS/contracts/source-anchor-v2.schema.json";
const CORPUS_SCHEMA: &str = "ToS/contracts/corpus-record.schema.json";
const MANIFEST_SCHEMA: &str = "ToS/contracts/source-item-manifest.schema.json";
const RIGHTS_SCHEMA: &str = "ToS/contracts/rights-record.schema.json";
const MAX_LAYERS_V5: usize = 8;
const MAX_LINEAGE: usize = 16;
const MAX_ORIGINAL_V5: u64 = 16 * 1024 * 1024;
const MAX_ORIGINAL_V6: u64 = 128 * 1024 * 1024;
const MAX_SOURCE_BYTES_V5: usize = 16 * 1024 * 1024;
const MAX_SOURCE_FILES: usize = 128;
const MAX_PAGE_IMAGE: u64 = 10 * 1024 * 1024;
const MAX_PAGE_PIXELS: u64 = 12_000_000;
const MAX_OCR_TEXT_BYTES: usize = 131_072;
const RETAINED_PAGE_PROFILE: &str = "tos_retained_page_ocr_image_comparison_v1";
const SYNTHETIC_PNG_PROFILE: &str = "tos_operator_synthetic_png_ocr_image_comparison_v1";
const INITIAL_CONFIG: &str = "tos_local_text_layer_create_owner_v1";
const DERIVED_CONFIG: &str = "tos_local_text_layer_derive_owner_v1";
const OWNER_OCR_CONFIG: &str = "tos_local_text_layer_record_owner_ocr_v1";
const OWNER_PAGE_OCR_CONFIG: &str = "tos_local_text_layer_record_owner_page_ocr_v1";
const DERIVED_OPERATIONS: [&str; 4] = [
    "text-layer.correct",
    "text-layer.normalize",
    "text-layer.record-transcription",
    "text-layer.record-ocr",
];
const MAX_SELECTION_BYTES: usize = MAX_RECORD_BYTES;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LayerQualityDependency {
    pub(crate) layer_id: String,
    pub(crate) use_name: String,
}

/// Pure configuration closure needed before the journal takes any subject
/// lock. It authenticates shape and selected IDs only; source relationships
/// are resolved later from exact current records.
#[derive(Debug, Clone)]
pub(crate) struct OwnerLocalLayerPreflight {
    pub(crate) layer_ids: BTreeSet<String>,
    pub(crate) lock_subject_ids: BTreeSet<String>,
    pub(crate) quality_dependencies: BTreeMap<String, Vec<LayerQualityDependency>>,
    pub(crate) selection_digest: String,
}

#[derive(Debug, Clone)]
pub(crate) struct SelectedOwnerLayer {
    /// Exact current owner record envelope selected by the layer binding.
    pub(crate) layer_record: AssessmentRecordInput,
    /// Exact layer and corpus source binding selected by the protected row.
    pub(crate) binding: JsonValue,
    /// Exact comparison envelope, absent when the independently selected
    /// grant is metadata-only.
    pub(crate) comparison_record: Option<AssessmentRecordInput>,
    pub(crate) scope: JsonValue,
    pub(crate) read_ready: bool,
    /// A comparison can make a source visible to a later human assessment;
    /// it never contains a positive assessment decision of its own.
    pub(crate) positive_use_allowed: bool,
    pub(crate) comparison_limits: Vec<String>,
    pub(crate) profile_contract_digests: BTreeMap<String, String>,
    pub(crate) source_snapshot: String,
}

#[derive(Debug, Clone)]
pub(crate) struct PrivateAssessmentLayers {
    pub(crate) layers: BTreeMap<String, SelectedOwnerLayer>,
    pub(crate) source_records: Vec<AssessmentRecordInput>,
    pub(crate) source_snapshot: String,
    pub(crate) profile_contract_digests: BTreeMap<String, String>,
    selected_input_bytes: u64,
    provided_input_bytes: u64,
    selection_digest: String,
    context_snapshot: String,
    currentness: Vec<CurrentnessPin>,
}

#[derive(Debug, Clone)]
enum CurrentnessPin {
    Epub {
        config: JsonValue,
        config_raw: Vec<u8>,
        config_path: PathBuf,
        entry: JsonValue,
        identity: PayloadIdentity,
        member_sha256: String,
    },
    Original {
        config: JsonValue,
        entry: JsonValue,
        identity: PayloadIdentity,
    },
    Image {
        access: JsonValue,
        input: JsonValue,
        identity: PayloadIdentity,
        sha256: String,
    },
    OwnerOcr {
        material: JsonValue,
        scope: JsonValue,
        language: String,
        page: bool,
        evidence_dir: PathBuf,
        copied: BTreeMap<String, Vec<u8>>,
        uid: u32,
    },
}

struct TargetLayer {
    binding: JsonValue,
    origin_id: String,
    raw: Vec<u8>,
    layer: JsonValue,
    record: AssessmentRecordInput,
    reference: String,
    config_ref: String,
    config_path: PathBuf,
    config_raw: Vec<u8>,
    config: JsonValue,
    policy_raw: Vec<u8>,
    policy: JsonValue,
    scope: JsonValue,
    subject: JsonValue,
    selection: JsonValue,
}

struct ImageRead {
    raw: Vec<u8>,
    identity: PayloadIdentity,
}

struct SelectedLayerAnchor {
    reference: JsonValue,
    raw_bytes: usize,
}

struct PreparedLayer {
    target: TargetLayer,
    source_manifest: JsonValue,
    scope_kind: String,
    page_anchor: Option<SelectedLayerAnchor>,
    profile_contract_digests: BTreeMap<String, String>,
}

impl PrivateAssessmentLayers {
    pub(crate) fn records(&self) -> Vec<AssessmentRecordInput> {
        let mut records = self.source_records.clone();
        for selected in self.layers.values() {
            records.push(selected.layer_record.clone());
            if let Some(comparison) = &selected.comparison_record {
                records.push(comparison.clone());
            }
        }
        records
    }

    /// Incremental bytes consumed by this adapter. The caller has already
    /// charged the owner configuration, selected context and source records.
    pub(crate) fn input_bytes(&self) -> u64 {
        self.selected_input_bytes
    }

    /// Canonical input buffers supplied by the caller and excluded from
    /// `input_bytes()` to avoid charging the same envelopes twice.
    pub(crate) fn provided_input_bytes(&self) -> u64 {
        self.provided_input_bytes
    }

    /// Revalidate the retained owner and third-root file evidence after the
    /// caller's review locks have been acquired. Every private read made by
    /// this adapter remains held by `OwnerTextContext`; third-root and image
    /// inputs are reopened and compared to their selected identities here.
    pub(crate) fn verify_current(
        &self,
        owner: &OwnerTextContext,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        if owner.snapshot(deadline, cancelled)?.to_prefixed() != self.context_snapshot {
            return Err(SourceCommandError::Conflict(
                "private assessment native layer owner context changed",
            ));
        }
        let uid = owner.account_uid();
        for pin in &self.currentness {
            match pin {
                CurrentnessPin::Epub {
                    config,
                    config_raw,
                    config_path,
                    entry,
                    identity,
                    member_sha256,
                } => {
                    validate_payload_grant(cmd::field(config, "source_access")?, MAX_ORIGINAL_V5)?;
                    let reread = OwnerTextInitialLayerSelection {
                        config: config.clone(),
                        raw: config_raw.clone(),
                        path: config_path.clone(),
                    };
                    let current =
                        read_acquired_epub_member(owner, &reread, entry, deadline, cancelled)?;
                    validate_payload_grant(cmd::field(config, "source_access")?, MAX_ORIGINAL_V5)?;
                    if current.identity != *identity
                        || Digest256::of_bytes(&current.raw).to_hex() != *member_sha256
                    {
                        return Err(SourceCommandError::Conflict(
                            "private assessment EPUB source member changed",
                        ));
                    }
                }
                CurrentnessPin::Original {
                    config,
                    entry,
                    identity,
                } => {
                    validate_payload_grant(cmd::field(config, "source_access")?, MAX_ORIGINAL_V6)?;
                    let current = verify_acquired_file(owner, config, entry, deadline, cancelled)?;
                    validate_payload_grant(cmd::field(config, "source_access")?, MAX_ORIGINAL_V6)?;
                    if current != *identity {
                        return Err(SourceCommandError::Conflict(
                            "private assessment original source file changed",
                        ));
                    }
                }
                CurrentnessPin::Image {
                    access,
                    input,
                    identity,
                    sha256,
                } => {
                    let current = read_exact_png(access, input, uid, deadline, cancelled)?;
                    validate_image_grant(access)?;
                    if current.identity != *identity
                        || Digest256::of_bytes(&current.raw).to_hex() != *sha256
                    {
                        return Err(SourceCommandError::Conflict(
                            "private assessment retained image changed",
                        ));
                    }
                }
                CurrentnessPin::OwnerOcr {
                    material,
                    scope,
                    language,
                    page,
                    evidence_dir,
                    copied,
                    uid,
                } => {
                    source_text_owner_ocr::validate_material(material, *page)?;
                    cmd::validate_expiry(
                        cmd::text(material, "expires_at")?,
                        &crate::source_serialization::instant()?,
                    )?;
                    source_text_owner_ocr::verify_record(
                        material,
                        scope,
                        language,
                        *page,
                        evidence_dir,
                        copied,
                        *uid,
                        deadline,
                        cancelled,
                    )?;
                    cmd::validate_expiry(
                        cmd::text(material, "expires_at")?,
                        &crate::source_serialization::instant()?,
                    )?;
                }
            }
        }
        if owner.snapshot(deadline, cancelled)?.to_prefixed() != self.context_snapshot {
            return Err(SourceCommandError::Conflict(
                "private assessment native layer owner context changed during verification",
            ));
        }
        let current = source_snapshot(
            owner,
            &self.layers,
            &self.currentness,
            &self.source_records,
            &self.selection_digest,
            &self.profile_contract_digests,
            deadline,
            cancelled,
        )?;
        if current != self.source_snapshot {
            return Err(SourceCommandError::Conflict(
                "private assessment native layer source snapshot changed",
            ));
        }
        Ok(())
    }
}

impl SelectedOwnerLayer {
    pub(crate) fn quality_observation(&self, required: bool) -> AssessmentLayerQualityObservation {
        AssessmentLayerQualityObservation {
            source_comparison_required: required,
            source_comparison_present: self.comparison_record.is_some(),
            positive_use_allowed: self.positive_use_allowed,
        }
    }
}

/// Validate the entire v5/v6 native-layer and quality-dependency selection
/// without opening owner, corpus, payload, or image paths.
pub(crate) fn preflight_owner_local_layers(
    config: &JsonValue,
    subjects: &JsonValue,
    schema_version: &str,
) -> SourceCommandResult<OwnerLocalLayerPreflight> {
    let v6 = schema_version == "tos_local_assessment_owner_v6";
    if !matches!(
        schema_version,
        "tos_local_assessment_owner_v5" | "tos_local_assessment_owner_v6"
    ) || cmd::text(config, "schema_version")? != schema_version
        || !cmd::same(cmd::field(config, "subjects")?, subjects)?
    {
        return Err(SourceCommandError::Invalid(
            "private assessment layer preflight profile",
        ));
    }
    let selections = cmd::array(config, "native_text_layers")?;
    if selections.len() > if v6 { 1 } else { MAX_LAYERS_V5 }
        || cmd::canonical(&cmd::object(vec![
            ("selections", JsonValue::Array(selections.to_vec())),
            ("subjects", subjects.clone()),
        ]))?
        .len()
            > MAX_SELECTION_BYTES
    {
        return Err(SourceCommandError::Invalid(
            "private assessment native layer preflight budget",
        ));
    }
    let mut original_bytes = 0u64;
    let mut layer_ids = BTreeSet::new();
    let mut layer_record_paths = BTreeSet::new();
    for selection in selections {
        validate_selection(selection, subjects, v6, &mut layer_ids, &mut original_bytes)?;
        let record_ref = cmd::text(
            cmd::field(cmd::field(selection, "binding")?, "text_layer")?,
            "record_ref",
        )?;
        if Path::new(record_ref)
            .file_name()
            .and_then(|name| name.to_str())
            != Some("source-text-layer.v1.json")
            || !layer_record_paths.insert(record_ref.to_owned())
        {
            return Err(SourceCommandError::Invalid(
                "private assessment layer path repeats or has another package identity",
            ));
        }
    }
    if original_bytes > if v6 { MAX_ORIGINAL_V6 } else { MAX_ORIGINAL_V5 } {
        return Err(SourceCommandError::Invalid(
            "private assessment native original-byte budget",
        ));
    }
    let dependencies = cmd::field(config, "quality_dependencies")?;
    let entries = dependencies.as_object().ok_or(SourceCommandError::Invalid(
        "private assessment quality dependency map",
    ))?;
    if entries.len() > tos_validation::assessment::MAX_ASSESSMENTS {
        return Err(SourceCommandError::Invalid(
            "private assessment quality dependency subject budget",
        ));
    }
    let subject_map = subjects.as_object().ok_or(SourceCommandError::Invalid(
        "private assessment layer subjects",
    ))?;
    let mut quality_dependencies = BTreeMap::new();
    let mut lock_subject_ids = layer_ids.clone();
    for (subject_key, values) in entries {
        let subject_id = subject_key
            .as_str()
            .ok_or(SourceCommandError::Invalid(
                "private assessment quality subject identity",
            ))?
            .to_owned();
        if !subject_map
            .iter()
            .any(|(key, _)| key.as_str() == Some(&subject_id))
        {
            return Err(SourceCommandError::Invalid(
                "private assessment quality dependency subject is unconfigured",
            ));
        }
        let rows = values.as_array().ok_or(SourceCommandError::Invalid(
            "private assessment quality dependency rows",
        ))?;
        if rows.len() > MAX_LAYERS_V5 {
            return Err(SourceCommandError::Invalid(
                "private assessment quality dependency row budget",
            ));
        }
        let mut seen = BTreeSet::new();
        let mut selected = Vec::with_capacity(rows.len());
        for row in rows {
            cmd::exact_keys(row, &["layer_id", "use"])?;
            let layer_id = cmd::text(row, "layer_id")?.to_owned();
            let use_name = cmd::text(row, "use")?.to_owned();
            if !cmd::nonblank(&layer_id)
                || !seen.insert(layer_id.clone())
                || !layer_ids.contains(&layer_id)
                || ![
                    "text-layer:citation",
                    "text-layer:linguistic-analysis",
                    "text-layer:semantic-analysis",
                    "text-layer:search-projection",
                ]
                .contains(&use_name.as_str())
            {
                return Err(SourceCommandError::Invalid(
                    "private assessment quality dependency binding",
                ));
            }
            selected.push(LayerQualityDependency { layer_id, use_name });
        }
        lock_subject_ids.insert(subject_id.clone());
        quality_dependencies.insert(subject_id, selected);
    }
    let selection_digest = Digest256::of_bytes(&cmd::canonical(&cmd::object(vec![
        ("selections", JsonValue::Array(selections.to_vec())),
        ("subjects", subjects.clone()),
        ("quality_dependencies", dependencies.clone()),
    ]))?)
    .to_prefixed();
    Ok(OwnerLocalLayerPreflight {
        layer_ids,
        lock_subject_ids,
        quality_dependencies,
        selection_digest,
    })
}

/// Select exact v5/v6 layer comparison evidence from the protected owner
/// profile. `source_records` are the already selected v4 source envelopes;
/// layer bytes, configurations, policies and payloads are independently
/// resolved through the retained `OwnerTextContext` and current grants here.
pub(crate) fn select_owner_local_layers(
    owner: &mut OwnerTextContext,
    selected_context: &JsonValue,
    config: &JsonValue,
    source_records: &[AssessmentRecordInput],
    cut: &CorpusCutReader,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PrivateAssessmentLayers> {
    if cut.current().revision() != worker.source_revision() {
        return Err(SourceCommandError::Conflict(
            "private assessment layer worker and source cut differ",
        ));
    }
    context_selection_matches(owner, selected_context)?;
    let schema_version = cmd::text(config, "schema_version")?;
    let v6 = schema_version == "tos_local_assessment_owner_v6";
    if !matches!(
        schema_version,
        "tos_local_assessment_owner_v5" | "tos_local_assessment_owner_v6"
    ) {
        return Err(SourceCommandError::Unsupported(
            "private assessment native layers require owner profile v5 or v6",
        ));
    }
    let subjects = cmd::field(config, "subjects")?;
    let preflight = preflight_owner_local_layers(config, subjects, schema_version)?;
    let selections = cmd::array(config, "native_text_layers")?;
    if selections.len() > if v6 { 1 } else { MAX_LAYERS_V5 } {
        return Err(SourceCommandError::Invalid(
            "private assessment native layer selection count",
        ));
    }
    if cmd::canonical(config)?.len() > MAX_RECORD_BYTES {
        return Err(SourceCommandError::Invalid(
            "private assessment native layer owner configuration budget",
        ));
    }
    if source_records.len() > tos_validation::assessment::MAX_ASSESSMENTS {
        return Err(SourceCommandError::Invalid(
            "private assessment selected source record budget",
        ));
    }
    let mut seen = BTreeSet::new();
    let mut original_bytes = 0u64;
    for selection in selections {
        validate_selection(selection, subjects, v6, &mut seen, &mut original_bytes)?;
    }
    if original_bytes > if v6 { MAX_ORIGINAL_V6 } else { MAX_ORIGINAL_V5 } {
        return Err(SourceCommandError::Invalid(
            "private assessment native original-byte budget",
        ));
    }
    let context_snapshot = owner.snapshot(deadline, cancelled)?.to_prefixed();
    for selection in selections {
        let payload = cmd::field(selection, "payload_access")?;
        if payload == &JsonValue::Null {
            continue;
        }
        let root = normalized_absolute(cmd::text(payload, "payload_root")?)?;
        let overlaps = |other: &Path| root.starts_with(other) || other.starts_with(&root);
        if overlaps(owner.public_root()) || overlaps(owner.private_root()) {
            return Err(SourceCommandError::Denied(
                "private assessment acquired payload needs a distinct third root",
            ));
        }
    }
    let mut currentness = Vec::new();
    let mut layers = BTreeMap::new();
    let mut selected_contracts = BTreeMap::new();
    let mut selected_input_bytes = 0u64;
    let mut provided_input_bytes = 0u64;
    add_byte_cost(&mut provided_input_bytes, cmd::canonical(config)?.len())?;
    add_byte_cost(
        &mut provided_input_bytes,
        cmd::canonical(selected_context)?.len(),
    )?;
    for record in source_records {
        add_byte_cost(&mut provided_input_bytes, record.envelope.len())?;
    }
    let mut prepared = Vec::with_capacity(selections.len());
    // Match the maintained adapter's admission order: resolve every layer,
    // source manifest, current rights closure and anchor before any content,
    // original payload, or page image is read.
    for selection in selections {
        crate::source_creation_store::active(deadline, cancelled)?;
        let target = read_target(owner, worker, selection, subjects, deadline, cancelled)?;
        let (source_manifest, source_metadata_bytes) =
            validate_selected_source_metadata(owner, worker, &target, deadline, cancelled)?;
        let rights_bytes = validate_current_layer_rights(
            owner,
            worker,
            &target,
            &source_manifest,
            deadline,
            cancelled,
        )?;
        add_u64_cost(&mut selected_input_bytes, source_metadata_bytes)?;
        add_u64_cost(&mut selected_input_bytes, rights_bytes)?;
        add_byte_cost(&mut selected_input_bytes, target.raw.len())?;
        add_byte_cost(&mut selected_input_bytes, target.config_raw.len())?;
        add_byte_cost(&mut selected_input_bytes, target.policy_raw.len())?;
        let mut digests = BTreeMap::new();
        remember_contract(worker, &mut selected_contracts, LAYER_SCHEMA)?;
        remember_contract(worker, &mut digests, LAYER_SCHEMA)?;
        remember_contract(worker, &mut selected_contracts, BINDING_SCHEMA)?;
        remember_contract(worker, &mut digests, BINDING_SCHEMA)?;
        for contract in [CORPUS_SCHEMA, MANIFEST_SCHEMA, RIGHTS_SCHEMA] {
            remember_contract(worker, &mut selected_contracts, contract)?;
            remember_contract(worker, &mut digests, contract)?;
        }
        let scope_kind =
            cmd::text(cmd::field(selection, "source_access")?, "read_scope")?.to_owned();
        if !v6 {
            let anchor_bytes =
                validate_target_anchors(owner, worker, &target, deadline, cancelled)?;
            add_byte_cost(&mut selected_input_bytes, anchor_bytes)?;
            remember_contract(worker, &mut selected_contracts, ANCHOR_SCHEMA)?;
            remember_contract(worker, &mut digests, ANCHOR_SCHEMA)?;
        }
        let page_anchor = if v6 {
            let anchor = validate_page_layer_profile(
                owner, worker, &target, selection, deadline, cancelled,
            )?;
            remember_contract(worker, &mut selected_contracts, ANCHOR_SCHEMA)?;
            remember_contract(worker, &mut digests, ANCHOR_SCHEMA)?;
            add_byte_cost(&mut selected_input_bytes, anchor.raw_bytes)?;
            Some(anchor)
        } else {
            None
        };
        if scope_kind != "metadata_only" {
            let comparison_schema = if v6 {
                PAGE_COMPARISON_SCHEMA
            } else if is_initial_extraction(&target.config)? {
                TEXT_COMPARISON_SCHEMA
            } else {
                DERIVATION_COMPARISON_SCHEMA
            };
            remember_contract(worker, &mut selected_contracts, comparison_schema)?;
            remember_contract(worker, &mut digests, comparison_schema)?;
        }
        prepared.push(PreparedLayer {
            target,
            source_manifest,
            scope_kind,
            page_anchor,
            profile_contract_digests: digests,
        });
    }

    for mut layer in prepared {
        let target = &mut layer.target;
        let selection = target.selection.clone();
        let source_manifest = &layer.source_manifest;
        let scope_kind = layer.scope_kind.as_str();
        let payload_access = cmd::field(&selection, "payload_access")?;
        let mut digests = layer.profile_contract_digests;
        let (comparison_record, read_ready, positive_use_allowed, source_snapshot) = if scope_kind
            == "metadata_only"
        {
            (None, false, false, context_snapshot.clone())
        } else if v6 {
            let (comparison, pins, snapshot_value, used_contracts, compared_bytes) =
                compare_page_ocr(
                    owner,
                    worker,
                    target,
                    payload_access,
                    &selection,
                    source_manifest,
                    layer
                        .page_anchor
                        .as_ref()
                        .ok_or(SourceCommandError::Invalid(
                            "private assessment page anchor absent",
                        ))?,
                    deadline,
                    cancelled,
                )?;
            add_u64_cost(&mut selected_input_bytes, compared_bytes)?;
            add_byte_cost(&mut selected_input_bytes, comparison.envelope.len())?;
            currentness.extend(pins);
            for (name, digest) in used_contracts {
                selected_contracts.insert(name.clone(), digest.clone());
                digests.insert(name, digest);
            }
            (Some(comparison), true, false, snapshot_value)
        } else if is_initial_extraction(&target.config)? {
            let (comparison, pins, snapshot_value, compared_bytes) = compare_initial_extraction(
                owner,
                worker,
                target,
                payload_access,
                deadline,
                cancelled,
            )?;
            add_u64_cost(&mut selected_input_bytes, compared_bytes)?;
            add_byte_cost(&mut selected_input_bytes, comparison.envelope.len())?;
            currentness.extend(pins);
            let ready = comparison_match(&comparison);
            (Some(comparison), ready, ready, snapshot_value)
        } else {
            let (comparison, read_ready, pins, snapshot_value, compared_bytes) =
                compare_derived_layer(owner, worker, target, payload_access, deadline, cancelled)?;
            add_u64_cost(&mut selected_input_bytes, compared_bytes)?;
            add_byte_cost(&mut selected_input_bytes, comparison.envelope.len())?;
            currentness.extend(pins);
            (Some(comparison), read_ready, read_ready, snapshot_value)
        };
        // Initial extraction permits positive review only on an exact text
        // match. Derived and image comparisons are source-visible evidence;
        // their own mismatch/fidelity fields never become quality verdicts.
        if let Some(comparison) = &comparison_record {
            if scope_kind == "metadata_only" {
                add_byte_cost(&mut selected_input_bytes, comparison.envelope.len())?;
            }
            let schema = if v6 {
                PAGE_COMPARISON_SCHEMA
            } else if is_initial_extraction(&target.config)? {
                TEXT_COMPARISON_SCHEMA
            } else {
                DERIVATION_COMPARISON_SCHEMA
            };
            validate_schema(
                worker,
                &target.reference,
                comparison,
                schema,
                deadline,
                cancelled,
            )?;
        }
        let selected = SelectedOwnerLayer {
            layer_record: target.record.clone(),
            binding: target.binding.clone(),
            comparison_record,
            scope: target.scope.clone(),
            read_ready,
            positive_use_allowed,
            comparison_limits: if v6 {
                page_comparison_limits(cmd::text(&selection, "comparison_profile")?)
            } else {
                Vec::new()
            },
            profile_contract_digests: digests,
            source_snapshot,
        };
        if layers
            .insert(cmd::text(&target.layer, "layer_id")?.to_owned(), selected)
            .is_some()
        {
            return Err(SourceCommandError::Invalid(
                "private assessment native layer identity repeats",
            ));
        }
    }
    add_u64_cost(
        &mut selected_input_bytes,
        currentness_byte_cost(&currentness)?,
    )?;
    for (name, expected) in &selected_contracts {
        let path = tos_foundation::RelativePath::parse(name)
            .map_err(|_| SourceCommandError::Invalid("private assessment contract path"))?;
        let raw = cut
            .read_member(
                cut.current().revision(),
                &path,
                MAX_RECORD_BYTES as u64,
                deadline,
                cancelled,
            )
            .map_err(|_| {
                SourceCommandError::Conflict("private assessment selected contract changed")
            })?
            .raw;
        if Digest256::of_bytes(&raw).to_prefixed() != *expected {
            return Err(SourceCommandError::Conflict(
                "private assessment selected contract digest differs",
            ));
        }
        add_byte_cost(&mut selected_input_bytes, raw.len())?;
    }
    let source_snapshot = source_snapshot(
        owner,
        &layers,
        &currentness,
        source_records,
        &preflight.selection_digest,
        &selected_contracts,
        deadline,
        cancelled,
    )?;
    Ok(PrivateAssessmentLayers {
        layers,
        source_records: source_records.to_vec(),
        source_snapshot,
        profile_contract_digests: selected_contracts,
        selected_input_bytes,
        provided_input_bytes,
        selection_digest: preflight.selection_digest,
        context_snapshot,
        currentness,
    })
}

fn context_selection_matches(
    owner: &OwnerTextContext,
    selected: &JsonValue,
) -> SourceCommandResult<()> {
    cmd::exact_keys(
        selected,
        &[
            "schema_version",
            "store_id",
            "public_root",
            "private_root",
            "private_prefix",
        ],
    )?;
    let prefix = owner
        .private_identity_home()
        .strip_prefix(owner.private_root())
        .ok()
        .and_then(Path::to_str)
        .map(|value| format!("{}/", value.trim_end_matches('/')))
        .ok_or(SourceCommandError::Denied(
            "private assessment selected context prefix",
        ))?;
    let store_id =
        prefix
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .ok_or(SourceCommandError::Denied(
                "private assessment selected store identity",
            ))?;
    if cmd::text(selected, "schema_version")? != "tos_owner_local_source_context_v1"
        || cmd::text(selected, "store_id")? != store_id
        || cmd::text(selected, "public_root")? != owner.public_root().to_str().unwrap_or("")
        || cmd::text(selected, "private_root")? != owner.private_root().to_str().unwrap_or("")
        || cmd::text(selected, "private_prefix")? != prefix
    {
        return Err(SourceCommandError::Conflict(
            "private assessment owner context selection differs",
        ));
    }
    Ok(())
}

fn validate_selection(
    selection: &JsonValue,
    subjects: &JsonValue,
    v6: bool,
    seen: &mut BTreeSet<String>,
    original_bytes: &mut u64,
) -> SourceCommandResult<()> {
    cmd::exact_keys(
        selection,
        if v6 {
            &[
                "binding",
                "origin_id",
                "source_access",
                "payload_access",
                "comparison_profile",
                "image_access",
                "disclosure_access",
            ]
        } else {
            &["binding", "origin_id", "source_access", "payload_access"]
        },
    )?;
    let binding = cmd::field(selection, "binding")?;
    cmd::exact_keys(
        binding,
        &["schema_version", "text_layer", "source_record_refs"],
    )?;
    let target = cmd::field(binding, "text_layer")?;
    cmd::exact_keys(
        target,
        &["record_ref", "record_sha256", "layer_id", "layer_version"],
    )?;
    cmd::exact_keys(
        cmd::field(binding, "source_record_refs")?,
        &["work", "expression", "edition", "item"],
    )?;
    let layer_id = cmd::text(target, "layer_id")?;
    if cmd::text(binding, "schema_version")? != "tos_native_text_layer_binding_v1"
        || !valid_layer_id(layer_id)
        || cmd::integer(target, "layer_version")? == 0
        || !lower_sha(cmd::text(target, "record_sha256")?)
        || !safe_metadata_ref(cmd::text(target, "record_ref")?)
        || !cmd::nonblank(cmd::text(selection, "origin_id")?)
        || !seen.insert(layer_id.to_owned())
    {
        return Err(SourceCommandError::Invalid(
            "private assessment native layer binding identity",
        ));
    }
    for kind in ["work", "expression", "edition", "item"] {
        let reference = cmd::text(cmd::field(binding, "source_record_refs")?, kind)?;
        if !safe_metadata_ref(reference)
            || Path::new(reference)
                .file_name()
                .and_then(|name| name.to_str())
                != Some(format!("{kind}.json").as_str())
        {
            return Err(SourceCommandError::Invalid(
                "private assessment native source locator",
            ));
        }
    }
    let access = cmd::field(selection, "source_access")?;
    cmd::exact_keys(access, &["read_scope", "access_allowed", "authority_ref"])?;
    let read_scope = cmd::text(access, "read_scope")?;
    if !matches!(read_scope, "metadata_only" | "exact_owner_local")
        || cmd::field(access, "access_allowed")? != &JsonValue::Bool(true)
        || !cmd::nonblank(cmd::text(access, "authority_ref")?)
    {
        return Err(SourceCommandError::Denied(
            "private assessment native layer source access",
        ));
    }
    let payload = cmd::field(selection, "payload_access")?;
    if read_scope == "metadata_only" {
        if payload != &JsonValue::Null {
            return Err(SourceCommandError::Denied(
                "metadata-only native layer selection carries payload access",
            ));
        }
    } else {
        validate_payload_grant(payload, if v6 { MAX_ORIGINAL_V6 } else { MAX_ORIGINAL_V5 })?;
        *original_bytes = original_bytes
            .checked_add(cmd::integer(payload, "byte_size")?)
            .ok_or(SourceCommandError::Invalid(
                "private assessment native original byte overflow",
            ))?;
    }
    let subject = subjects
        .object_get(layer_id)
        .ok_or(SourceCommandError::Denied(
            "private assessment native layer subject is absent",
        ))?;
    cmd::exact_keys(
        subject,
        &[
            "record",
            "assertion_layer",
            "risk",
            "languages",
            "maker_id",
            "requested_use",
            "access_allowed",
        ],
    )?;
    let record = cmd::field(subject, "record")?;
    cmd::exact_keys(record, &["id", "version", "digest"])?;
    if cmd::text(record, "id")? != layer_id
        || cmd::integer(record, "version")? != cmd::integer(target, "layer_version")?
        || !prefixed_sha(cmd::text(record, "digest")?)
        || cmd::text(subject, "assertion_layer")? != "textual_observation"
        || !["low", "moderate", "high"].contains(&cmd::text(subject, "risk")?)
        || ![
            "text-layer:citation",
            "text-layer:linguistic-analysis",
            "text-layer:semantic-analysis",
            "text-layer:search-projection",
        ]
        .contains(&cmd::text(subject, "requested_use")?)
        || cmd::field(subject, "access_allowed")? != &JsonValue::Bool(true)
        || cmd::array(subject, "languages")?.len() != 1
        || !cmd::nonblank(cmd::array(subject, "languages")?[0].as_str().unwrap_or(""))
        || !cmd::nonblank(cmd::text(subject, "maker_id")?)
    {
        return Err(SourceCommandError::Denied(
            "private assessment native layer subject scope",
        ));
    }
    if v6 {
        let profile = cmd::text(selection, "comparison_profile")?;
        if ![RETAINED_PAGE_PROFILE, SYNTHETIC_PNG_PROFILE].contains(&profile) {
            return Err(SourceCommandError::Unsupported(
                "private assessment page OCR comparison profile",
            ));
        }
        let image = cmd::field(selection, "image_access")?;
        let disclosure = cmd::field(selection, "disclosure_access")?;
        if profile == RETAINED_PAGE_PROFILE && disclosure != &JsonValue::Null {
            return Err(SourceCommandError::Denied(
                "retained historical page OCR cannot authorize disclosure",
            ));
        }
        if read_scope == "metadata_only" {
            if image != &JsonValue::Null || disclosure != &JsonValue::Null {
                return Err(SourceCommandError::Denied(
                    "metadata-only page OCR selection carries image or disclosure access",
                ));
            }
        } else {
            validate_image_grant(image)?;
            if profile == SYNTHETIC_PNG_PROFILE {
                if cmd::integer(image, "page_number")? != 1
                    || cmd::integer(payload, "byte_size")? > MAX_PAGE_IMAGE
                {
                    return Err(SourceCommandError::Denied(
                        "synthetic OCR comparison exceeds one whole PNG profile",
                    ));
                }
                if disclosure != &JsonValue::Null {
                    validate_disclosure_for_selection(disclosure, target, image)?;
                }
            }
        }
    }
    Ok(())
}

fn validate_payload_grant(grant: &JsonValue, max_bytes: u64) -> SourceCommandResult<()> {
    cmd::exact_keys(
        grant,
        &[
            "read_scope",
            "access_allowed",
            "authority_ref",
            "expires_at",
            "payload_root",
            "byte_size",
        ],
    )?;
    if cmd::text(grant, "read_scope")? != "exact_acquired_file"
        || cmd::field(grant, "access_allowed")? != &JsonValue::Bool(true)
        || !cmd::nonblank(cmd::text(grant, "authority_ref")?)
        || cmd::integer(grant, "byte_size")? == 0
        || cmd::integer(grant, "byte_size")? > max_bytes
    {
        return Err(SourceCommandError::Denied(
            "private assessment exact acquired-file grant",
        ));
    }
    let now = crate::source_serialization::instant()?;
    cmd::validate_expiry(cmd::text(grant, "expires_at")?, &now)?;
    normalized_absolute(cmd::text(grant, "payload_root")?)?;
    Ok(())
}

fn validate_image_grant(image: &JsonValue) -> SourceCommandResult<()> {
    cmd::exact_keys(
        image,
        &[
            "read_scope",
            "access_allowed",
            "authority_ref",
            "expires_at",
            "path",
            "byte_size",
            "sha256",
            "page_number",
            "source_file_ref",
            "source_file_sha256",
            "processing_boundary",
            "width_pixels",
            "height_pixels",
        ],
    )?;
    if cmd::text(image, "read_scope")? != "exact_retained_page"
        || cmd::field(image, "access_allowed")? != &JsonValue::Bool(true)
        || !cmd::nonblank(cmd::text(image, "authority_ref")?)
        || cmd::text(image, "processing_boundary")? != "local_only"
        || cmd::integer(image, "byte_size")? == 0
        || cmd::integer(image, "byte_size")? > MAX_PAGE_IMAGE
        || !lower_sha(cmd::text(image, "sha256")?)
        || !(1..=10_000).contains(&cmd::integer(image, "page_number")?)
        || cmd::integer(image, "width_pixels")? == 0
        || cmd::integer(image, "height_pixels")? == 0
        || cmd::integer(image, "width_pixels")?
            .checked_mul(cmd::integer(image, "height_pixels")?)
            .is_none_or(|pixels| pixels > MAX_PAGE_PIXELS)
        || !lower_sha(cmd::text(image, "source_file_sha256")?)
        || cmd::text(image, "source_file_ref")?
            != format!(
                "tos.file.sha256.{}",
                cmd::text(image, "source_file_sha256")?
            )
    {
        return Err(SourceCommandError::Denied(
            "private assessment exact retained image grant",
        ));
    }
    cmd::validate_expiry(
        cmd::text(image, "expires_at")?,
        &crate::source_serialization::instant()?,
    )?;
    normalized_absolute(cmd::text(image, "path")?)?;
    Ok(())
}

fn validate_disclosure(disclosure: &JsonValue, target: &JsonValue) -> SourceCommandResult<()> {
    cmd::exact_keys(
        disclosure,
        &[
            "allowed",
            "authority_ref",
            "expires_at",
            "read_scope",
            "basis",
            "processing_boundary",
            "source_file_ref",
            "source_file_sha256",
            "image_sha256",
            "layer_record_sha256",
        ],
    )?;
    if cmd::field(disclosure, "allowed")? != &JsonValue::Bool(true)
        || !cmd::nonblank(cmd::text(disclosure, "authority_ref")?)
        || cmd::text(disclosure, "read_scope")? != "exact_source_image_and_ocr"
        || cmd::text(disclosure, "basis")? != "operator_created_synthetic_source"
        || cmd::text(disclosure, "processing_boundary")? != "current_assistant_session"
        || cmd::text(disclosure, "layer_record_sha256")? != cmd::text(target, "record_sha256")?
        || !lower_sha(cmd::text(disclosure, "source_file_sha256")?)
        || !lower_sha(cmd::text(disclosure, "image_sha256")?)
    {
        return Err(SourceCommandError::Denied(
            "private assessment synthetic image disclosure grant scope",
        ));
    }
    cmd::validate_expiry(
        cmd::text(disclosure, "expires_at")?,
        &crate::source_serialization::instant()?,
    )?;
    Ok(())
}

fn validate_disclosure_for_selection(
    disclosure: &JsonValue,
    target: &JsonValue,
    image: &JsonValue,
) -> SourceCommandResult<()> {
    validate_disclosure(disclosure, target)?;
    if cmd::text(disclosure, "source_file_ref")? != cmd::text(image, "source_file_ref")?
        || cmd::text(disclosure, "source_file_sha256")? != cmd::text(image, "source_file_sha256")?
        || cmd::text(disclosure, "image_sha256")? != cmd::text(image, "sha256")?
    {
        return Err(SourceCommandError::Denied(
            "private assessment disclosure grant differs from its selected image",
        ));
    }
    Ok(())
}

fn read_target(
    owner: &OwnerTextContext,
    worker: &mut CutWorkerSchemaExecutor,
    selection: &JsonValue,
    subjects: &JsonValue,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<TargetLayer> {
    let binding = cmd::field(selection, "binding")?.clone();
    let target = cmd::field(&binding, "text_layer")?;
    let reference = cmd::text(target, "record_ref")?.to_owned();
    let layer_raw = owner.read(&reference, MAX_RECORD_BYTES, deadline, cancelled)?;
    if Digest256::of_bytes(&layer_raw).to_hex() != cmd::text(target, "record_sha256")? {
        return Err(SourceCommandError::Conflict(
            "private assessment native TextLayer selected digest differs",
        ));
    }
    let layer = cmd::parse(&layer_raw)?;
    validate_schema_payload(
        worker,
        &reference,
        &layer,
        LAYER_SCHEMA,
        deadline,
        cancelled,
    )?;
    validate_layer_metadata(&layer_raw, &reference, deadline, cancelled)?;
    if cmd::field(&layer, "layer_id")? != cmd::field(target, "layer_id")?
        || cmd::field(&layer, "layer_version")? != cmd::field(target, "layer_version")?
    {
        return Err(SourceCommandError::Conflict(
            "private assessment native TextLayer id/version differs",
        ));
    }
    let maker = cmd::field(cmd::field(&layer, "derivation")?, "maker")?;
    let config_ref = cmd::text(maker, "configuration_ref")?.to_owned();
    let expected_config = format!(
        "{}/source-create-owner-configuration.json",
        reference
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid(
                "private assessment native layer package reference",
            ))?
            .0
    );
    if config_ref != expected_config {
        return Err(SourceCommandError::Conflict(
            "private assessment native layer configuration locator differs",
        ));
    }
    let config_raw = owner.read(&config_ref, MAX_RECORD_BYTES, deadline, cancelled)?;
    if Digest256::of_bytes(&config_raw).to_hex() != cmd::text(maker, "configuration_digest")? {
        return Err(SourceCommandError::Conflict(
            "private assessment native layer configuration digest differs",
        ));
    }
    let config = cmd::parse(&config_raw)?;
    let policy_ref = cmd::text(cmd::field(&layer, "editorial_policy")?, "policy_ref")?;
    let policy_raw = owner.read(policy_ref, MAX_RECORD_BYTES, deadline, cancelled)?;
    if Digest256::of_bytes(&policy_raw).to_hex()
        != cmd::text(cmd::field(&layer, "editorial_policy")?, "policy_sha256")?
    {
        return Err(SourceCommandError::Conflict(
            "private assessment native layer policy digest differs",
        ));
    }
    let policy = cmd::parse(&policy_raw)?;
    if cmd::canonical(cmd::field(&config, "policy")?)? != cmd::canonical(&policy)? {
        return Err(SourceCommandError::Conflict(
            "private assessment native layer retained policy differs",
        ));
    }
    let scope = layer_source_scope(&layer)?;
    if cmd::canonical(cmd::field(&config, "source_scope")?)? != cmd::canonical(&scope)?
        || cmd::canonical(cmd::field(&config, "source_record_refs")?)?
            != cmd::canonical(cmd::field(&binding, "source_record_refs")?)?
        || cmd::text(&config, "source_path")? != reference
        || cmd::field(&config, "language")?
            != cmd::field(cmd::field(&layer, "representation")?, "language")?
    {
        return Err(SourceCommandError::Conflict(
            "private assessment native layer retained source configuration differs",
        ));
    }
    let subject = subjects
        .object_get(cmd::text(&layer, "layer_id")?)
        .ok_or(SourceCommandError::Denied(
            "private assessment native layer subject is absent",
        ))?
        .clone();
    let record = envelope(
        cmd::text(&layer, "layer_id")?,
        cmd::field(&layer, "layer_version")?,
        &layer,
        cmd::text(selection, "origin_id")?,
    )?;
    let expected_ref = record_reference(&layer)?;
    if cmd::canonical(cmd::field(&subject, "record")?)? != cmd::canonical(&expected_ref)?
        || cmd::field(&subject, "languages")?
            != &JsonValue::Array(vec![
                cmd::field(cmd::field(&layer, "representation")?, "language")?.clone(),
            ])
        || cmd::text(&subject, "maker_id")? != cmd::text(maker, "agent_ref")?
    {
        return Err(SourceCommandError::Denied(
            "private assessment native subject differs from source-owned layer",
        ));
    }
    let mut config_path = owner.private_root().join(&config_ref);
    if !config_ref.starts_with("ToS/source-witnesses/owner-local/") {
        config_path = owner.public_root().join(&config_ref);
    }
    Ok(TargetLayer {
        binding,
        origin_id: cmd::text(selection, "origin_id")?.to_owned(),
        raw: layer_raw,
        layer,
        record,
        reference,
        config_ref,
        config_path,
        config_raw,
        config,
        policy_raw,
        policy,
        scope,
        subject,
        selection: selection.clone(),
    })
}

fn validate_layer_metadata(
    raw: &[u8],
    reference: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    use tos_validation::text_metadata_rules::{
        self as rules, TextMetadataLimits, TextMetadataState,
    };
    let report = rules::inspect_source_text_layer_metadata(
        raw,
        reference,
        TextMetadataLimits {
            max_packet_bytes: 1_048_576,
            max_state_bytes: 8_388_608,
            max_issues: 128,
            deadline,
        },
        cancelled,
    )
    .map_err(|_| {
        SourceCommandError::Unsupported(
            "private assessment TextLayer metadata predicate unavailable",
        )
    })?;
    if report.packet_digest != Digest256::of_bytes(raw).to_hex()
        || report.scope != "owner-metadata-predicates-only"
    {
        return Err(SourceCommandError::Conflict(
            "private assessment TextLayer metadata predicate input differs",
        ));
    }
    match report.state {
        TextMetadataState::CheckedMetadata if report.issues.is_empty() => Ok(()),
        TextMetadataState::Unsupported => Err(SourceCommandError::Unsupported(
            "private assessment TextLayer metadata predicate profile",
        )),
        _ => Err(SourceCommandError::Invalid(
            "private assessment TextLayer metadata violates owner predicates",
        )),
    }
}

fn validate_selected_source_metadata(
    owner: &OwnerTextContext,
    worker: &mut CutWorkerSchemaExecutor,
    target: &TargetLayer,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(JsonValue, u64)> {
    let refs = cmd::field(&target.config, "source_record_refs")?;
    let digests = cmd::field(&target.config, "source_record_sha256")?;
    if !cmd::same(refs, cmd::field(&target.binding, "source_record_refs")?)? {
        return Err(SourceCommandError::Conflict(
            "private assessment selected source-record bindings differ",
        ));
    }
    let mut item = None;
    let mut byte_cost = 0u64;
    for kind in ["work", "expression", "edition", "item"] {
        let reference = cmd::text(refs, kind)?;
        let raw = owner.read(reference, MAX_RECORD_BYTES, deadline, cancelled)?;
        byte_cost = byte_cost
            .checked_add(raw.len() as u64)
            .ok_or(SourceCommandError::Invalid(
                "private assessment source metadata byte overflow",
            ))?;
        if Digest256::of_bytes(&raw).to_hex() != cmd::text(digests, kind)? {
            return Err(SourceCommandError::Conflict(
                "private assessment selected source-record digest differs",
            ));
        }
        crate::source_text_layer_entry::checked_schema(
            worker,
            reference,
            &raw,
            CORPUS_SCHEMA,
            deadline,
            cancelled,
        )?;
        let record = cmd::parse(&raw)?;
        if cmd::text(&record, "record_type")? != kind
            || cmd::text(&record, "record_id")? != cmd::text(&target.scope, &format!("{kind}_ref"))?
        {
            return Err(SourceCommandError::Conflict(
                "private assessment source-record identity differs",
            ));
        }
        if kind == "item" {
            item = Some(record);
        }
    }
    let item = item.ok_or(SourceCommandError::Invalid(
        "private assessment selected Item record absent",
    ))?;
    let manifest_ref = cmd::text(&item, "item_manifest_ref")?;
    let manifest_raw = owner.read(manifest_ref, MAX_RECORD_BYTES, deadline, cancelled)?;
    byte_cost =
        byte_cost
            .checked_add(manifest_raw.len() as u64)
            .ok_or(SourceCommandError::Invalid(
                "private assessment source metadata byte overflow",
            ))?;
    if Digest256::of_bytes(&manifest_raw).to_hex() != cmd::text(&target.config, "manifest_sha256")?
    {
        return Err(SourceCommandError::Conflict(
            "private assessment selected Item manifest digest differs",
        ));
    }
    crate::source_text_layer_entry::checked_schema(
        worker,
        manifest_ref,
        &manifest_raw,
        MANIFEST_SCHEMA,
        deadline,
        cancelled,
    )?;
    let manifest = cmd::parse(&manifest_raw)?;
    if cmd::text(&manifest, "item_id")? != cmd::text(&target.scope, "item_ref")?
        || cmd::text(&manifest, "visibility")? != "local_only"
    {
        return Err(SourceCommandError::Denied(
            "private assessment exact source manifest is not local-only",
        ));
    }
    let entries = cmd::array(&manifest, "payload_files")?
        .iter()
        .filter(|entry| entry.object_get("file_id") == target.scope.object_get("file_ref"))
        .collect::<Vec<_>>();
    if entries.len() != 1
        || cmd::field(entries[0], "sha256")? != cmd::field(&target.scope, "file_sha256")?
    {
        return Err(SourceCommandError::Conflict(
            "private assessment source File is not unique in the selected manifest",
        ));
    }
    Ok((manifest, byte_cost))
}

fn validate_target_anchors(
    owner: &OwnerTextContext,
    worker: &mut CutWorkerSchemaExecutor,
    target: &TargetLayer,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<usize> {
    let anchors = cmd::array(cmd::field(&target.layer, "source_binding")?, "anchors")?;
    if anchors.is_empty() || anchors.len() > MAX_SOURCE_FILES {
        return Err(SourceCommandError::Invalid(
            "private assessment source anchor count",
        ));
    }
    let mut seen = BTreeSet::new();
    let mut byte_cost = 0usize;
    for reference in anchors {
        let path = cmd::text(reference, "anchor_record_ref")?;
        let expected = cmd::text(reference, "anchor_record_sha256")?;
        if !safe_metadata_ref(path) || !seen.insert(path.to_owned()) {
            return Err(SourceCommandError::Invalid(
                "private assessment source anchor locator repeats",
            ));
        }
        let raw = owner.read(path, MAX_RECORD_BYTES, deadline, cancelled)?;
        if Digest256::of_bytes(&raw).to_hex() != expected {
            return Err(SourceCommandError::Conflict(
                "private assessment source anchor digest differs",
            ));
        }
        crate::source_text_layer_entry::checked_schema(
            worker,
            path,
            &raw,
            ANCHOR_SCHEMA,
            deadline,
            cancelled,
        )?;
        let anchor = cmd::parse(&raw)?;
        let endpoint = cmd::field(&anchor, "target")?;
        if cmd::field(&anchor, "anchor_id")? != cmd::field(reference, "anchor_id")?
            || cmd::text(endpoint, "item_id")? != cmd::text(&target.scope, "item_ref")?
            || cmd::text(endpoint, "file_id")? != cmd::text(&target.scope, "file_ref")?
            || cmd::text(endpoint, "file_sha256")? != cmd::text(&target.scope, "file_sha256")?
        {
            return Err(SourceCommandError::Conflict(
                "private assessment source anchor target differs",
            ));
        }
        byte_cost = byte_cost
            .checked_add(raw.len())
            .ok_or(SourceCommandError::Invalid(
                "private assessment source anchor byte cost overflow",
            ))?;
    }
    Ok(byte_cost)
}

fn validate_current_layer_rights(
    owner: &OwnerTextContext,
    worker: &mut CutWorkerSchemaExecutor,
    target: &TargetLayer,
    manifest: &JsonValue,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<u64> {
    let representation = cmd::field(&target.layer, "representation")?;
    let rights = cmd::array(representation, "rights_record_refs")?;
    let configured = cmd::field(
        cmd::field(&target.config, "derivation_access")?,
        "rights_record_refs",
    )?;
    if rights.is_empty()
        || rights.len() > MAX_SOURCE_FILES
        || !cmd::same(&JsonValue::Array(rights.to_vec()), configured)?
    {
        return Err(SourceCommandError::Conflict(
            "private assessment TextLayer rights closure differs from configuration",
        ));
    }
    let exact = [
        cmd::text(&target.layer, "layer_id")?,
        cmd::text(representation, "content_file_id")?,
    ];
    let mut relevant = exact.to_vec();
    for name in ["work_ref", "expression_ref", "edition_ref", "item_ref"] {
        relevant.push(cmd::text(&target.scope, name)?);
    }
    let mut records = Vec::with_capacity(rights.len());
    let mut byte_cost = 0u64;
    let mut seen = BTreeSet::new();
    let manifest_rights_ref = cmd::text(manifest, "rights_ref")?;
    let mut item_file_rights = false;
    for row in rights {
        cmd::exact_keys(row, &["ref", "sha256"])?;
        let reference = cmd::text(row, "ref")?;
        let expected = cmd::text(row, "sha256")?;
        if !safe_metadata_ref(reference) || !lower_sha(expected) || !seen.insert(reference) {
            return Err(SourceCommandError::Invalid(
                "private assessment TextLayer rights reference",
            ));
        }
        let raw = owner.read(reference, MAX_RECORD_BYTES, deadline, cancelled)?;
        byte_cost = byte_cost
            .checked_add(raw.len() as u64)
            .ok_or(SourceCommandError::Invalid(
                "private assessment rights byte overflow",
            ))?;
        if Digest256::of_bytes(&raw).to_hex() != expected {
            return Err(SourceCommandError::Conflict(
                "private assessment current rights record digest differs",
            ));
        }
        crate::source_text_layer_entry::checked_schema(
            worker,
            reference,
            &raw,
            RIGHTS_SCHEMA,
            deadline,
            cancelled,
        )?;
        let record = cmd::parse(&raw)?;
        let scope_refs = cmd::array(&record, "scope_refs")?;
        let intersects = |ids: &[&str]| {
            ids.iter()
                .any(|id| scope_refs.iter().any(|value| value.as_str() == Some(id)))
        };
        if !intersects(&relevant) {
            return Err(SourceCommandError::Denied(
                "private assessment rights record addresses another source scope",
            ));
        }
        if reference == manifest_rights_ref {
            if !intersects(&[
                cmd::text(&target.scope, "item_ref")?,
                cmd::text(&target.scope, "file_ref")?,
            ]) {
                return Err(SourceCommandError::Denied(
                    "private assessment Item rights omit its exact File",
                ));
            }
            item_file_rights = true;
        }
        records.push(record);
    }
    if !item_file_rights {
        return Err(SourceCommandError::Conflict(
            "private assessment TextLayer omits current Item/File rights",
        ));
    }
    let mut decisions = Vec::new();
    for record in &records {
        if ["superseded", "legal_review_requested"].contains(&cmd::text(record, "review_status")?)
            || ["permission_denied", "conflicting_evidence"]
                .contains(&cmd::text(record, "assessment_status")?)
        {
            return Err(SourceCommandError::Denied(
                "private assessment TextLayer rights are inactive or denied",
            ));
        }
        let assessments = record
            .object_get("layer_assessments")
            .and_then(JsonValue::as_array)
            .unwrap_or(&[]);
        let mut scoped = false;
        for assessment in assessments {
            let scope_refs = cmd::array(assessment, "scope_refs")?;
            if exact
                .iter()
                .any(|id| scope_refs.iter().any(|value| value.as_str() == Some(id)))
            {
                decisions.push(assessment);
                scoped = true;
            }
        }
        if !scoped
            && exact.iter().any(|id| {
                cmd::array(record, "scope_refs")
                    .ok()
                    .is_some_and(|refs| refs.iter().any(|v| v.as_str() == Some(id)))
            })
        {
            decisions.push(record);
        }
    }
    if decisions.is_empty() {
        decisions.extend(records.iter());
    }
    for decision in decisions {
        if !["local_research_only", "allowed"].contains(&cmd::text(decision, "derivative_posture")?)
            || ["permission_denied", "conflicting_evidence"]
                .contains(&cmd::text(decision, "assessment_status")?)
            || ["superseded", "legal_review_requested"]
                .contains(&cmd::text(decision, "review_status")?)
        {
            return Err(SourceCommandError::Denied(
                "private assessment current local research rights are not unconditional",
            ));
        }
    }
    Ok(byte_cost)
}

fn validate_page_layer_profile(
    owner: &OwnerTextContext,
    worker: &mut CutWorkerSchemaExecutor,
    target: &TargetLayer,
    selection: &JsonValue,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<SelectedLayerAnchor> {
    let profile = cmd::text(selection, "comparison_profile")?;
    let retained = match profile {
        RETAINED_PAGE_PROFILE => true,
        SYNTHETIC_PNG_PROFILE => false,
        _ => {
            return Err(SourceCommandError::Unsupported(
                "private assessment page OCR comparison profile",
            ));
        }
    };
    let layer = &target.layer;
    let config = &target.config;
    let derivation = cmd::field(layer, "derivation")?;
    let maker = cmd::field(derivation, "maker")?;
    let representation = cmd::field(layer, "representation")?;
    let operation = if retained {
        "text-layer.record-owner-page-ocr"
    } else {
        "text-layer.record-owner-ocr"
    };
    let config_schema = if retained {
        OWNER_PAGE_OCR_CONFIG
    } else {
        OWNER_OCR_CONFIG
    };
    let expected_input_kind = if retained {
        "retained_pdf_page"
    } else {
        "acquired_file"
    };
    let target_maker = cmd::object(
        ["maker_type", "agent_ref", "method", "version"]
            .iter()
            .map(|key| Ok((*key, cmd::field(maker, key)?.clone())))
            .collect::<SourceCommandResult<Vec<_>>>()?,
    );
    let config_maker = cmd::field(config, "maker")?;
    if cmd::text(derivation, "method")? != "ocr"
        || !cmd::array(derivation, "input_layers")?.is_empty()
        || !cmd::same(
            cmd::field(derivation, "change_payload")?,
            &cmd::object(vec![("kind", cmd::string("none"))]),
        )?
        || cmd::text(layer, "layer_role")? != "raw_ocr"
        || cmd::text(representation, "character_normalization")? != "none"
        || cmd::text(representation, "content_visibility")? != "local_only"
        || cmd::field(representation, "publication_authorized")? != &JsonValue::Bool(false)
        || cmd::integer(cmd::field(representation, "text_scope")?, "start")? != 0
        || cmd::array(cmd::field(layer, "source_binding")?, "anchors")?.len() != 1
        || cmd::text(maker, "maker_type")? != "software"
        || cmd::text(maker, "configuration_ref")? != target.config_ref
        || cmd::text(config, "schema_version")? != config_schema
        || cmd::array(config, "allowed_operations")? != [cmd::string(operation)]
        || cmd::text(cmd::field(config, "input")?, "kind")? != expected_input_kind
        || !cmd::same(cmd::field(config, "source_scope")?, &target.scope)?
        || !cmd::same(
            cmd::field(config, "source_record_refs")?,
            cmd::field(&target.binding, "source_record_refs")?,
        )?
        || cmd::text(config, "source_path")? != target.reference
        || !cmd::same(cmd::field(config, "policy")?, &target.policy)?
        || !cmd::same(&target_maker, config_maker)?
        || cmd::field(config, "language")? != cmd::field(representation, "language")?
        || !cmd::same(
            cmd::field(
                cmd::field(config, "derivation_access")?,
                "rights_record_refs",
            )?,
            cmd::field(representation, "rights_record_refs")?,
        )?
        || cmd::field(cmd::field(config, "identities")?, "layer_id")?
            != cmd::field(layer, "layer_id")?
        || cmd::field(cmd::field(config, "identities")?, "provenance_event_id")?
            != cmd::field(layer, "provenance_event_ref")?
    {
        return Err(SourceCommandError::Unsupported(
            "private assessment v6 requires the exact retained-page or synthetic owner OCR layer",
        ));
    }
    let material = cmd::field(config, "material")?;
    source_text_owner_ocr::validate_material(material, retained)?;
    cmd::validate_expiry(
        cmd::text(material, "expires_at")?,
        &crate::source_serialization::instant()?,
    )?;
    if cmd::field(material, "content_sha256")? != cmd::field(representation, "content_sha256")?
        || cmd::field(material, "byte_size")? != cmd::field(representation, "byte_size")?
    {
        return Err(SourceCommandError::Conflict(
            "private assessment owner OCR grant differs from TextLayer content",
        ));
    }
    let input = if retained {
        let input = cmd::field(material, "input_representation")?;
        source_text_owner_ocr::validate_page_binding(input)?;
        input.clone()
    } else {
        let image = cmd::field(selection, "image_access")?;
        if image != &JsonValue::Null {
            cmd::object(vec![
                (
                    "schema_version",
                    cmd::string("tos_operator_synthetic_png_input_binding_v1"),
                ),
                (
                    "source_file_ref",
                    cmd::field(&target.scope, "file_ref")?.clone(),
                ),
                (
                    "source_file_sha256",
                    cmd::field(&target.scope, "file_sha256")?.clone(),
                ),
                (
                    "input_file_ref",
                    cmd::field(&target.scope, "file_ref")?.clone(),
                ),
                (
                    "input_sha256",
                    cmd::field(&target.scope, "file_sha256")?.clone(),
                ),
                (
                    "input_bytes",
                    cmd::field(cmd::field(config, "source_access")?, "byte_size")?.clone(),
                ),
                ("media_type", cmd::string("image/png")),
                ("page_number", cmd::number(1)),
                ("page_index_origin", cmd::number(1)),
                ("width_pixels", cmd::field(image, "width_pixels")?.clone()),
                ("height_pixels", cmd::field(image, "height_pixels")?.clone()),
            ])
        } else {
            cmd::object(vec![
                (
                    "schema_version",
                    cmd::string("tos_operator_synthetic_png_input_binding_v1"),
                ),
                (
                    "source_file_ref",
                    cmd::field(&target.scope, "file_ref")?.clone(),
                ),
                (
                    "source_file_sha256",
                    cmd::field(&target.scope, "file_sha256")?.clone(),
                ),
                (
                    "input_file_ref",
                    cmd::field(&target.scope, "file_ref")?.clone(),
                ),
                (
                    "input_sha256",
                    cmd::field(&target.scope, "file_sha256")?.clone(),
                ),
                (
                    "input_bytes",
                    cmd::field(cmd::field(config, "source_access")?, "byte_size")?.clone(),
                ),
                ("media_type", cmd::string("image/png")),
                ("page_number", cmd::number(1)),
                ("page_index_origin", cmd::number(1)),
                ("width_pixels", JsonValue::Null),
                ("height_pixels", JsonValue::Null),
            ])
        }
    };
    if retained {
        let source_file_ref = cmd::text(&target.scope, "file_ref")?;
        let source_file_sha = cmd::text(&target.scope, "file_sha256")?;
        if cmd::text(&input, "source_file_ref")? != source_file_ref
            || cmd::text(&input, "source_file_sha256")? != source_file_sha
        {
            return Err(SourceCommandError::Conflict(
                "private assessment retained OCR input differs from source File",
            ));
        }
    } else if cmd::text(&input, "source_file_ref")? != cmd::text(&target.scope, "file_ref")?
        || cmd::text(&input, "source_file_sha256")? != cmd::text(&target.scope, "file_sha256")?
    {
        return Err(SourceCommandError::Conflict(
            "private assessment synthetic OCR input differs from source File",
        ));
    }
    let anchor_ref = &cmd::array(cmd::field(layer, "source_binding")?, "anchors")?[0];
    let anchor_path = cmd::text(anchor_ref, "anchor_record_ref")?;
    let anchor_raw = owner.read(anchor_path, MAX_RECORD_BYTES, deadline, cancelled)?;
    if Digest256::of_bytes(&anchor_raw).to_hex() != cmd::text(anchor_ref, "anchor_record_sha256")? {
        return Err(SourceCommandError::Conflict(
            "private assessment exact OCR source anchor digest differs",
        ));
    }
    crate::source_text_layer_entry::checked_schema(
        worker,
        anchor_path,
        &anchor_raw,
        ANCHOR_SCHEMA,
        deadline,
        cancelled,
    )?;
    let anchor = cmd::parse(&anchor_raw)?;
    if retained {
        source_text_owner_ocr::validate_page_anchor(&anchor, &input, &target.scope)?;
    } else {
        validate_synthetic_anchor(&anchor, &input, &target.scope)?;
    }
    let image_grant = cmd::field(selection, "image_access")?;
    if image_grant != &JsonValue::Null {
        validate_image_matches_input(image_grant, &input, &target.scope)?;
    }
    let disclosure = cmd::field(selection, "disclosure_access")?;
    if disclosure != &JsonValue::Null {
        if retained {
            return Err(SourceCommandError::Denied(
                "historical retained PDF pages cannot authorize disclosure",
            ));
        }
        validate_disclosure(disclosure, cmd::field(&target.binding, "text_layer")?)?;
        if cmd::text(disclosure, "source_file_ref")? != cmd::text(&target.scope, "file_ref")?
            || cmd::text(disclosure, "source_file_sha256")?
                != cmd::text(&target.scope, "file_sha256")?
            || cmd::text(disclosure, "image_sha256")? != cmd::text(&input, "input_sha256")?
        {
            return Err(SourceCommandError::Denied(
                "private assessment disclosure grant addresses another synthetic PNG",
            ));
        }
    }
    Ok(SelectedLayerAnchor {
        reference: anchor_ref.clone(),
        raw_bytes: anchor_raw.len(),
    })
}

fn validate_synthetic_anchor(
    anchor: &JsonValue,
    selected: &JsonValue,
    scope: &JsonValue,
) -> SourceCommandResult<()> {
    let target = cmd::field(anchor, "target")?;
    let expression = cmd::field(cmd::field(anchor, "selector_payload")?, "expression")?;
    let envelope = cmd::field(expression, "selector")?;
    let region = cmd::field(envelope, "selector")?;
    let state = cmd::field(envelope, "state")?;
    let width_mismatch = selected
        .object_get("width_pixels")
        .and_then(JsonValue::as_u64)
        .is_some_and(|width| cmd::integer(region, "source_width").ok() != Some(width));
    let height_mismatch = selected
        .object_get("height_pixels")
        .and_then(JsonValue::as_u64)
        .is_some_and(|height| cmd::integer(region, "source_height").ok() != Some(height));
    if cmd::text(expression, "mode")? != "single"
        || cmd::text(target, "item_id")? != cmd::text(scope, "item_ref")?
        || cmd::text(target, "file_id")? != cmd::text(scope, "file_ref")?
        || cmd::text(target, "file_sha256")? != cmd::text(scope, "file_sha256")?
        || cmd::text(target, "media_type")? != "image/png"
        || cmd::text(state, "state_type")? != "digest_state"
        || cmd::text(state, "representation_sha256")? != cmd::text(scope, "file_sha256")?
        || cmd::text(state, "media_type")? != "image/png"
        || cmd::text(region, "type")? != "page_region"
        || cmd::field(region, "page_identity")?
            != &cmd::object(vec![("page_number", cmd::number(1))])
        || cmd::text(region, "coordinate_space")? != "pixels"
        || cmd::integer(region, "x")? != 0
        || cmd::integer(region, "y")? != 0
        || cmd::integer(region, "width")? != cmd::integer(region, "source_width")?
        || cmd::integer(region, "height")? != cmd::integer(region, "source_height")?
        || width_mismatch
        || height_mismatch
    {
        return Err(SourceCommandError::Conflict(
            "private assessment synthetic OCR anchor is not the exact whole PNG",
        ));
    }
    Ok(())
}

fn validate_image_matches_input(
    image: &JsonValue,
    input: &JsonValue,
    scope: &JsonValue,
) -> SourceCommandResult<()> {
    if cmd::integer(image, "byte_size")? != cmd::integer(input, "input_bytes")?
        || cmd::text(image, "sha256")? != cmd::text(input, "input_sha256")?
        || cmd::integer(image, "page_number")? != cmd::integer(input, "page_number")?
        || cmd::integer(image, "width_pixels")? != cmd::integer(input, "width_pixels")?
        || cmd::integer(image, "height_pixels")? != cmd::integer(input, "height_pixels")?
        || cmd::text(image, "source_file_ref")? != cmd::text(scope, "file_ref")?
        || cmd::text(image, "source_file_sha256")? != cmd::text(scope, "file_sha256")?
    {
        return Err(SourceCommandError::Denied(
            "private assessment exact image grant addresses another source representation",
        ));
    }
    Ok(())
}

fn page_comparison_limits(profile: &str) -> Vec<String> {
    let limits: &[&str] = if profile == RETAINED_PAGE_PROFILE {
        &[
            "Quality review is limited to this exact OCR layer and retained page image; no full-document or translation-source admission.",
            "Historical PDF-page rendering remains unsigned retained provenance; current fixity is not a new render or independent page-fidelity attestation.",
            "Local source comparison grants no model/server disclosure, publication, diplomatic fidelity or canon authority.",
        ]
    } else {
        &[
            "Comparison concerns one operator-declared synthetic PNG and its exact authenticated OCR, not a historical witness.",
            "The protected issuer declares synthetic-source ownership; software checks exact bytes and grant scope, not authorship.",
            "An exact current source-disclosure grant permits only this image and OCR in the current assistant session; no publication or canon authority.",
        ]
    };
    limits.iter().map(|limit| (*limit).to_owned()).collect()
}

fn lower_sha(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn valid_layer_id(value: &str) -> bool {
    let Some(suffix) = value.strip_prefix("tos.text-layer.") else {
        return false;
    };
    !suffix.is_empty()
        && suffix.split(['.', '-']).all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
}

fn prefixed_sha(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(lower_sha)
}

fn safe_metadata_ref(value: &str) -> bool {
    if !cmd::nonblank(value)
        || value.contains('\\')
        || value.contains('\0')
        || !value.starts_with("ToS/source-witnesses/")
        || !value.ends_with(".json")
    {
        return false;
    }
    let mut components = Path::new(value).components();
    if !matches!(components.next(), Some(std::path::Component::Normal(_)))
        || !matches!(components.next(), Some(std::path::Component::Normal(_)))
    {
        return false;
    }
    components.all(|part| match part {
        std::path::Component::Normal(name) => name
            .to_str()
            .is_some_and(|name| !matches!(name, "payload" | "local-content" | "catalog")),
        _ => false,
    })
}

fn is_initial_extraction(config: &JsonValue) -> SourceCommandResult<bool> {
    Ok(cmd::text(config, "schema_version")? == INITIAL_CONFIG)
}

fn comparison_match(comparison: &AssessmentRecordInput) -> bool {
    cmd::parse(&comparison.envelope)
        .ok()
        .and_then(|record| cmd::field(&record, "payload").ok().cloned())
        .and_then(|payload| payload.object_get("deterministic_match").cloned())
        == Some(JsonValue::Bool(true))
}

fn remember_contract(
    worker: &CutWorkerSchemaExecutor,
    contracts: &mut BTreeMap<String, String>,
    name: &str,
) -> SourceCommandResult<()> {
    let digest = worker
        .contract_digest(name)
        .ok_or(SourceCommandError::Conflict(
            "private assessment selected comparison contract absent from source cut",
        ))?
        .to_prefixed();
    if contracts
        .insert(name.to_owned(), digest.clone())
        .is_some_and(|previous| previous != digest)
    {
        return Err(SourceCommandError::Conflict(
            "private assessment selected comparison contract changed",
        ));
    }
    Ok(())
}

fn validate_schema(
    worker: &mut CutWorkerSchemaExecutor,
    reference: &str,
    record: &AssessmentRecordInput,
    schema: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let envelope = cmd::parse(&record.envelope)?;
    let payload = cmd::field(&envelope, "payload")?;
    validate_schema_payload(worker, reference, payload, schema, deadline, cancelled)
}

fn validate_schema_payload(
    worker: &mut CutWorkerSchemaExecutor,
    reference: &str,
    payload: &JsonValue,
    schema: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let raw = cmd::canonical(payload)?;
    match worker.check_reusing_scalar(reference, &raw, schema, deadline, cancelled) {
        Ok(true) => Ok(()),
        Ok(false) => Err(SourceCommandError::Invalid(
            "private assessment layer comparison schema",
        )),
        Err(reason) => Err(SourceCommandError::SchemaExecution {
            path: reference.to_owned(),
            root: schema.to_owned(),
            reason,
        }),
    }
}

fn layer_source_scope(layer: &JsonValue) -> SourceCommandResult<JsonValue> {
    let binding = cmd::field(layer, "source_binding")?;
    Ok(cmd::object(vec![
        ("work_ref", cmd::field(binding, "work_ref")?.clone()),
        (
            "expression_ref",
            cmd::field(binding, "expression_ref")?.clone(),
        ),
        ("edition_ref", cmd::field(binding, "edition_ref")?.clone()),
        ("item_ref", cmd::field(binding, "item_ref")?.clone()),
        ("file_ref", cmd::field(binding, "source_file_ref")?.clone()),
        (
            "file_sha256",
            cmd::field(binding, "source_file_sha256")?.clone(),
        ),
    ]))
}

fn envelope(
    id: &str,
    version: &JsonValue,
    payload: &JsonValue,
    origin_id: &str,
) -> SourceCommandResult<AssessmentRecordInput> {
    if !cmd::nonblank(id) || !cmd::nonblank(origin_id) {
        return Err(SourceCommandError::Invalid(
            "private assessment native comparison identity",
        ));
    }
    let version =
        version
            .as_u64()
            .filter(|value| *value > 0)
            .ok_or(SourceCommandError::Invalid(
                "private assessment native comparison version",
            ))?;
    let raw = cmd::canonical(payload)?;
    if raw.len() > MAX_RECORD_BYTES {
        return Err(SourceCommandError::Invalid(
            "private assessment native comparison record budget",
        ));
    }
    let value = cmd::object(vec![
        ("id", cmd::string(id)),
        ("version", cmd::number(version)),
        ("payload", payload.clone()),
        ("origin_id", cmd::string(origin_id)),
    ]);
    Ok(AssessmentRecordInput {
        envelope: cmd::canonical(&value)?,
    })
}

fn record_reference(layer: &JsonValue) -> SourceCommandResult<JsonValue> {
    Ok(cmd::object(vec![
        ("id", cmd::string(cmd::text(layer, "layer_id")?)),
        (
            "version",
            cmd::number(cmd::integer(layer, "layer_version")?),
        ),
        (
            "digest",
            cmd::string(&cmd::record_digest(layer)?.to_prefixed()),
        ),
    ]))
}

fn input_fixity(inputs: &[NativeInput]) -> SourceCommandResult<Vec<JsonValue>> {
    let mut rows = BTreeMap::<(String, String), String>::new();
    for input in inputs {
        let category = if input.kind == NativeReadKind::Content {
            "content"
        } else {
            "metadata"
        };
        let key = (input.reference.clone(), category.to_owned());
        let digest = input.raw_sha256.to_hex();
        if rows
            .insert(key, digest.clone())
            .is_some_and(|previous| previous != digest)
        {
            return Err(SourceCommandError::Conflict(
                "private assessment native comparison input changed",
            ));
        }
    }
    if rows.len() > MAX_SOURCE_FILES {
        return Err(SourceCommandError::Invalid(
            "private assessment native comparison source-file budget",
        ));
    }
    Ok(rows
        .into_iter()
        .map(|((reference, category), digest)| {
            cmd::object(vec![
                ("ref", cmd::string(&reference)),
                ("category", cmd::string(&category)),
                ("sha256", cmd::string(&digest)),
            ])
        })
        .collect())
}

fn input_byte_cost(inputs: &[NativeInput]) -> SourceCommandResult<u64> {
    inputs.iter().try_fold(0u64, |total, input| {
        total
            .checked_add(input.raw_size as u64)
            .ok_or(SourceCommandError::Invalid(
                "private assessment source input byte cost overflow",
            ))
    })
}

fn add_byte_cost(total: &mut u64, amount: usize) -> SourceCommandResult<()> {
    *total = total
        .checked_add(amount as u64)
        .ok_or(SourceCommandError::Invalid(
            "private assessment selected input byte cost overflow",
        ))?;
    Ok(())
}

fn add_u64_cost(total: &mut u64, amount: u64) -> SourceCommandResult<()> {
    *total = total
        .checked_add(amount)
        .ok_or(SourceCommandError::Invalid(
            "private assessment selected input byte cost overflow",
        ))?;
    Ok(())
}

fn currentness_byte_cost(pins: &[CurrentnessPin]) -> SourceCommandResult<u64> {
    pins.iter().try_fold(0u64, |total, pin| {
        let amount = match pin {
            CurrentnessPin::Epub { identity, .. } | CurrentnessPin::Original { identity, .. } => {
                identity.file.4
            }
            CurrentnessPin::Image { identity, .. } => identity.file.4,
            CurrentnessPin::OwnerOcr { copied, .. } => {
                copied.values().try_fold(0u64, |sum, raw| {
                    sum.checked_add(raw.len() as u64)
                        .ok_or(SourceCommandError::Invalid(
                            "private assessment OCR evidence byte cost overflow",
                        ))
                })?
            }
        };
        total.checked_add(amount).ok_or(SourceCommandError::Invalid(
            "private assessment currentness byte cost overflow",
        ))
    })
}

fn append_fixity(
    rows: &mut Vec<JsonValue>,
    reference: &str,
    category: &str,
    digest: &str,
) -> SourceCommandResult<()> {
    if !lower_sha(digest) {
        return Err(SourceCommandError::Invalid(
            "private assessment source fixity digest",
        ));
    }
    let value = cmd::object(vec![
        ("ref", cmd::string(reference)),
        ("category", cmd::string(category)),
        ("sha256", cmd::string(digest)),
    ]);
    if rows.iter().any(|previous| previous == &value) {
        return Ok(());
    }
    rows.push(value);
    if rows.len() > MAX_SOURCE_FILES {
        return Err(SourceCommandError::Invalid(
            "private assessment native comparison source-file budget",
        ));
    }
    Ok(())
}

fn source_snapshot(
    owner: &OwnerTextContext,
    layers: &BTreeMap<String, SelectedOwnerLayer>,
    currentness: &[CurrentnessPin],
    source_records: &[AssessmentRecordInput],
    selection_digest: &str,
    profile_contract_digests: &BTreeMap<String, String>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<String> {
    let context = owner.snapshot(deadline, cancelled)?.to_prefixed();
    let layer_rows = layers
        .iter()
        .map(|(layer_id, selected)| {
            let comparison_digest = selected
                .comparison_record
                .as_ref()
                .map_or_else(String::new, |record| {
                    Digest256::of_bytes(&record.envelope).to_hex()
                });
            cmd::object(vec![
                ("layer_id", cmd::string(layer_id)),
                (
                    "record_sha256",
                    cmd::string(&Digest256::of_bytes(&selected.layer_record.envelope).to_hex()),
                ),
                ("comparison_sha256", cmd::string(&comparison_digest)),
                (
                    "profile_contract_digests",
                    cmd::object(
                        selected
                            .profile_contract_digests
                            .iter()
                            .map(|(name, digest)| (name.as_str(), cmd::string(digest)))
                            .collect(),
                    ),
                ),
                (
                    "comparison_limits",
                    JsonValue::Array(
                        selected
                            .comparison_limits
                            .iter()
                            .map(|limit| cmd::string(limit))
                            .collect(),
                    ),
                ),
                ("layer_snapshot", cmd::string(&selected.source_snapshot)),
            ])
        })
        .collect::<Vec<_>>();
    let pin_rows = currentness
        .iter()
        .map(|pin| -> SourceCommandResult<JsonValue> {
            match pin {
                CurrentnessPin::Epub {
                    identity,
                    member_sha256,
                    entry,
                    ..
                } => Ok(cmd::object(vec![
                    ("kind", cmd::string("epub_member")),
                    ("identity", cmd::string(&format!("{:?}", identity))),
                    ("member_sha256", cmd::string(member_sha256)),
                    ("entry", cmd::string(cmd::text(entry, "file_id")?)),
                ])),
                CurrentnessPin::Original {
                    identity, entry, ..
                } => Ok(cmd::object(vec![
                    ("kind", cmd::string("original_file")),
                    ("identity", cmd::string(&format!("{:?}", identity))),
                    ("entry", cmd::string(cmd::text(entry, "file_id")?)),
                ])),
                CurrentnessPin::Image {
                    identity,
                    sha256,
                    access,
                    ..
                } => Ok(cmd::object(vec![
                    ("kind", cmd::string("image")),
                    ("identity", cmd::string(&format!("{:?}", identity))),
                    ("sha256", cmd::string(sha256)),
                    ("path", cmd::string(cmd::text(access, "path")?)),
                ])),
                CurrentnessPin::OwnerOcr {
                    copied, material, ..
                } => {
                    let evidence = copied
                        .iter()
                        .map(|(name, raw)| {
                            (
                                name.as_str(),
                                cmd::string(&Digest256::of_bytes(raw).to_hex()),
                            )
                        })
                        .collect();
                    let evidence_digest =
                        Digest256::of_bytes(&cmd::canonical(&cmd::object(evidence))?);
                    Ok(cmd::object(vec![
                        ("kind", cmd::string("owner_ocr")),
                        (
                            "receipt_sha256",
                            cmd::string(cmd::text(material, "receipt_sha256")?),
                        ),
                        ("evidence_sha256", cmd::string(&evidence_digest.to_hex())),
                    ]))
                }
            }
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    let source_rows = source_records
        .iter()
        .map(|record| cmd::string(&Digest256::of_bytes(&record.envelope).to_hex()))
        .collect::<Vec<_>>();
    let contract_rows = profile_contract_digests
        .iter()
        .map(|(name, digest)| (name.as_str(), cmd::string(digest)))
        .collect();
    let state = cmd::object(vec![
        ("context", cmd::string(&context)),
        ("selection_digest", cmd::string(selection_digest)),
        ("source_records", JsonValue::Array(source_rows)),
        ("profile_contract_digests", cmd::object(contract_rows)),
        ("layers", JsonValue::Array(layer_rows)),
        ("currentness", JsonValue::Array(pin_rows)),
    ]);
    Ok(Digest256::of_bytes(&cmd::canonical(&state)?).to_prefixed())
}

fn read_whole_text_layer(
    owner: &mut OwnerTextContext,
    worker: &mut CutWorkerSchemaExecutor,
    binding: &JsonValue,
    expected_layer: &JsonValue,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(String, ResolvedOwnerTextLayer)> {
    let resolved = resolve_owner_text_layer(owner, worker, binding, deadline, cancelled)?;
    if cmd::canonical(&resolved.layer)? != cmd::canonical(expected_layer)? {
        return Err(SourceCommandError::Conflict(
            "private assessment exact TextLayer changed during selection",
        ));
    }
    let representation = cmd::field(expected_layer, "representation")?;
    let raw = resolved.raw.clone();
    let text = std::str::from_utf8(&raw)
        .map_err(|_| SourceCommandError::Invalid("private assessment TextLayer UTF-8"))?
        .to_owned();
    let text_scope = cmd::field(representation, "text_scope")?;
    if cmd::integer(text_scope, "start")? != 0
        || cmd::integer(text_scope, "end")? != text.chars().count() as u64
    {
        return Err(SourceCommandError::Invalid(
            "private assessment comparison needs the whole TextLayer representation",
        ));
    }
    Ok((text, resolved))
}

fn layer_source_payload_ref(
    source_record_refs: &JsonValue,
    entry: &JsonValue,
) -> SourceCommandResult<String> {
    let item = Path::new(cmd::text(source_record_refs, "item")?);
    let parent = item.parent().ok_or(SourceCommandError::Invalid(
        "private assessment Item metadata parent",
    ))?;
    Ok(parent
        .join(cmd::text(entry, "relative_path")?)
        .to_str()
        .ok_or(SourceCommandError::Invalid(
            "private assessment source payload reference UTF-8",
        ))?
        .to_owned())
}

fn validate_initial_anchor(
    owner: &OwnerTextContext,
    worker: &mut CutWorkerSchemaExecutor,
    target: &TargetLayer,
    entry: &JsonValue,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    let anchors = cmd::array(cmd::field(&target.layer, "source_binding")?, "anchors")?;
    if anchors.len() != 1 {
        return Err(SourceCommandError::Unsupported(
            "private assessment EPUB extraction requires one exact source anchor",
        ));
    }
    let target_anchor = &anchors[0];
    let reference = cmd::text(target_anchor, "anchor_record_ref")?;
    let raw = owner.read(reference, MAX_RECORD_BYTES, deadline, cancelled)?;
    if Digest256::of_bytes(&raw).to_hex() != cmd::text(target_anchor, "anchor_record_sha256")? {
        return Err(SourceCommandError::Conflict(
            "private assessment exact source anchor digest differs",
        ));
    }
    let anchor = cmd::parse(&raw)?;
    crate::source_text_layer_entry::checked_schema(
        worker,
        reference,
        &raw,
        ANCHOR_SCHEMA,
        deadline,
        cancelled,
    )?;
    let config = &target.config;
    let member = cmd::field(config, "member")?.clone();
    let selector = cmd::field(config, "selector")?.clone();
    let scope = cmd::field(config, "source_scope")?;
    let file_ref = cmd::text(scope, "file_ref")?;
    let file_sha = cmd::text(scope, "file_sha256")?;
    let payload_ref = layer_source_payload_ref(cmd::field(config, "source_record_refs")?, entry)?;
    let selector_payload = cmd::object(vec![
        ("kind", cmd::string("selector_expression")),
        (
            "expression",
            cmd::object(vec![
                ("mode", cmd::string("refinement_chain")),
                (
                    "steps",
                    JsonValue::Array(vec![
                        cmd::object(vec![
                            (
                                "state",
                                cmd::object(vec![
                                    ("state_type", cmd::string("digest_state")),
                                    ("representation_ref", cmd::string(&payload_ref)),
                                    ("representation_sha256", cmd::string(file_sha)),
                                    ("media_type", cmd::string("application/epub+zip")),
                                ]),
                            ),
                            (
                                "selector",
                                cmd::object(vec![
                                    ("type", cmd::string("container_member")),
                                    ("member_path", cmd::field(&member, "member_path")?.clone()),
                                    (
                                        "member_sha256",
                                        cmd::field(&member, "member_sha256")?.clone(),
                                    ),
                                    ("member_media_type", cmd::string("application/xhtml+xml")),
                                ]),
                            ),
                        ]),
                        cmd::object(vec![
                            (
                                "state",
                                cmd::object(vec![
                                    ("state_type", cmd::string("digest_state")),
                                    (
                                        "representation_ref",
                                        cmd::field(&member, "member_path")?.clone(),
                                    ),
                                    (
                                        "representation_sha256",
                                        cmd::field(&member, "member_sha256")?.clone(),
                                    ),
                                    ("media_type", cmd::string("application/xhtml+xml")),
                                ]),
                            ),
                            ("selector", selector.clone()),
                        ]),
                    ]),
                ),
            ]),
        ),
    ]);
    let identities = cmd::field(config, "identities")?;
    let maker = cmd::field(cmd::field(&target.layer, "derivation")?, "maker")?;
    let expected_method = cmd::object(
        ["maker_type", "method", "version"]
            .iter()
            .map(|key| Ok((*key, cmd::field(maker, key)?.clone())))
            .collect::<SourceCommandResult<Vec<_>>>()?,
    );
    let mut expected_method = expected_method;
    cmd::set(
        &mut expected_method,
        "agent_ref",
        cmd::field(maker, "agent_ref")?.clone(),
    )?;
    let mut selector_method = expected_method.clone();
    if let JsonValue::Object(fields) = &mut selector_method {
        fields.retain(|(key, _)| key.as_str() != Some("agent_ref"));
    }
    let expected_payload = cmd::object(vec![
        ("kind", cmd::string("selector_expression")),
        (
            "expression",
            cmd::field(&selector_payload, "expression")?.clone(),
        ),
    ]);
    if cmd::canonical(cmd::field(&anchor, "selector_payload")?)?
        != cmd::canonical(&expected_payload)?
        || cmd::field(&anchor, "anchor_id")? != cmd::field(identities, "anchor_id")?
        || cmd::field(&anchor, "passage_id")? != cmd::field(identities, "passage_id")?
        || cmd::field(&anchor, "provenance_event_ref")?
            != cmd::field(identities, "provenance_event_id")?
        || cmd::canonical(cmd::field(&anchor, "target")?)?
            != cmd::canonical(&cmd::object(vec![
                ("item_id", cmd::field(scope, "item_ref")?.clone()),
                ("file_id", cmd::string(file_ref)),
                ("file_sha256", cmd::string(file_sha)),
                ("media_type", cmd::string("application/epub+zip")),
            ]))?
        || cmd::canonical(cmd::field(&anchor, "selector_method")?)?
            != cmd::canonical(&selector_method)?
    {
        return Err(SourceCommandError::Conflict(
            "private assessment exact extraction anchor differs",
        ));
    }
    Ok(anchor)
}

fn validate_initial_layer_profile(target: &TargetLayer) -> SourceCommandResult<()> {
    let layer = &target.layer;
    let config = &target.config;
    let derivation = cmd::field(layer, "derivation")?;
    let maker = cmd::field(derivation, "maker")?;
    let representation = cmd::field(layer, "representation")?;
    let target_maker = cmd::object(
        ["maker_type", "agent_ref", "method", "version"]
            .iter()
            .map(|key| Ok((*key, cmd::field(maker, key)?.clone())))
            .collect::<SourceCommandResult<Vec<_>>>()?,
    );
    let config_maker = cmd::field(config, "maker")?;
    if cmd::text(config, "schema_version")? != INITIAL_CONFIG
        || cmd::text(derivation, "method")? != "structural_extraction"
        || !cmd::array(derivation, "input_layers")?.is_empty()
        || cmd::canonical(cmd::field(derivation, "change_payload")?)?
            != cmd::canonical(&cmd::object(vec![("kind", cmd::string("none"))]))?
        || cmd::text(layer, "layer_role")? != "machine_transcription"
        || cmd::text(representation, "character_normalization")? != "none"
        || cmd::text(representation, "content_visibility")? != "local_only"
        || cmd::field(representation, "publication_authorized")? != &JsonValue::Bool(false)
        || cmd::integer(cmd::field(representation, "text_scope")?, "start")? != 0
        || cmd::array(cmd::field(layer, "source_binding")?, "anchors")?.len() != 1
        || cmd::text(maker, "maker_type")? != "software"
        || cmd::canonical(&target_maker)? != cmd::canonical(config_maker)?
        || cmd::text(config, "source_path")? != target.reference
        || cmd::canonical(cmd::field(config, "source_scope")?)? != cmd::canonical(&target.scope)?
        || cmd::canonical(cmd::field(config, "source_record_refs")?)?
            != cmd::canonical(cmd::field(&target.binding, "source_record_refs")?)?
        || cmd::field(config, "language")? != cmd::field(representation, "language")?
        || cmd::canonical(cmd::field(config, "policy")?)? != cmd::canonical(&target.policy)?
        || cmd::canonical(
            cmd::field(config, "derivation_access")?
                .object_get("rights_record_refs")
                .ok_or(SourceCommandError::Invalid(
                    "private assessment extraction rights config",
                ))?,
        )? != cmd::canonical(cmd::field(representation, "rights_record_refs")?)?
    {
        return Err(SourceCommandError::Unsupported(
            "private assessment layer is outside the exact EPUB extraction profile",
        ));
    }
    let identities = cmd::field(config, "identities")?;
    if cmd::field(identities, "layer_id")? != cmd::field(layer, "layer_id")?
        || cmd::field(identities, "provenance_event_id")?
            != cmd::field(layer, "provenance_event_ref")?
    {
        return Err(SourceCommandError::Conflict(
            "private assessment extraction configuration identities differ",
        ));
    }
    validate_extraction_profile(
        cmd::field(config, "selector")?,
        cmd::field(config, "policy")?,
    )?;
    Ok(())
}

fn compare_initial_extraction(
    owner: &mut OwnerTextContext,
    worker: &mut CutWorkerSchemaExecutor,
    target: &TargetLayer,
    payload_access: &JsonValue,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(AssessmentRecordInput, Vec<CurrentnessPin>, String, u64)> {
    validate_payload_grant(payload_access, MAX_ORIGINAL_V5)?;
    validate_initial_layer_profile(target)?;
    let mut access_config = target.config.clone();
    cmd::set(&mut access_config, "source_access", payload_access.clone())?;
    let grant = OwnerTextInitialLayerSelection {
        config: access_config.clone(),
        raw: target.config_raw.clone(),
        path: target.config_path.clone(),
    };
    let source: ResolvedInitialTextSource =
        resolve_initial_owner_text_source(owner, worker, &grant, deadline, cancelled)?;
    validate_initial_anchor(
        owner,
        worker,
        target,
        &source.payload_entry,
        deadline,
        cancelled,
    )?;
    let (representation_text, resolved_layer) = read_whole_text_layer(
        owner,
        worker,
        &target.binding,
        &target.layer,
        deadline,
        cancelled,
    )?;
    let selector = validate_extraction_profile(
        cmd::field(&target.config, "selector")?,
        cmd::field(&target.config, "policy")?,
    )?;
    let acquired: AcquiredMember =
        read_acquired_epub_member(owner, &grant, &source.payload_entry, deadline, cancelled)?;
    validate_payload_grant(payload_access, MAX_ORIGINAL_V5)?;
    let expected_text = extract_xhtml_text(&acquired.raw, selector, deadline, cancelled)?;
    validate_payload_grant(payload_access, MAX_ORIGINAL_V5)?;
    let member_utf8 = std::str::from_utf8(&acquired.raw)
        .map_err(|_| SourceCommandError::Invalid("private assessment XHTML UTF-8"))?;
    if acquired.raw.len() > MAX_SOURCE_BYTES_V5
        || member_utf8.len() > 1_048_576
        || expected_text.len() > 1_048_576
        || representation_text.len() > 1_048_576
    {
        return Err(SourceCommandError::Unsupported(
            "private assessment EPUB comparison record budget",
        ));
    }
    let source_inputs = [resolved_layer.inputs, source.inputs].concat();
    let source_input_bytes = input_byte_cost(&source_inputs)?;
    let mut fixity = input_fixity(&source_inputs)?;
    let payload_ref = layer_source_payload_ref(
        cmd::field(&target.config, "source_record_refs")?,
        &source.payload_entry,
    )?;
    append_fixity(
        &mut fixity,
        &payload_ref,
        "original_payload",
        cmd::text(&target.scope, "file_sha256")?,
    )?;
    append_fixity(
        &mut fixity,
        &format!(
            "{}!/{}",
            payload_ref,
            cmd::text(cmd::field(&target.config, "member")?, "member_path")?
        ),
        "source_member",
        cmd::text(cmd::field(&target.config, "member")?, "member_sha256")?,
    )?;
    let mut body = cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_native_text_layer_comparison_v1"),
        ),
        ("comparison_version", cmd::number(1)),
        ("layer", record_reference(&target.layer)?),
        ("source_scope", target.scope.clone()),
        ("member", cmd::field(&target.config, "member")?.clone()),
        ("selector", cmd::field(&target.config, "selector")?.clone()),
        ("source_member_utf8", cmd::string(member_utf8)),
        ("expected_text", cmd::string(&expected_text)),
        ("representation_text", cmd::string(&representation_text)),
        (
            "representation",
            cmd::field(&target.layer, "representation")?.clone(),
        ),
        ("editorial_policy", target.policy.clone()),
        (
            "maker",
            cmd::field(cmd::field(&target.layer, "derivation")?, "maker")?.clone(),
        ),
        (
            "configuration_sha256",
            cmd::string(cmd::text(
                cmd::field(cmd::field(&target.layer, "derivation")?, "maker")?,
                "configuration_digest",
            )?),
        ),
        ("input_fixity", JsonValue::Array(fixity)),
        (
            "deterministic_match",
            JsonValue::Bool(expected_text == representation_text),
        ),
        ("visibility", cmd::string("local_only")),
        ("publication_authorized", JsonValue::Bool(false)),
        ("performs_semantic_assessment", JsonValue::Bool(false)),
    ]);
    let comparison_id = format!(
        "tos.text-comparison.sha256.{}",
        Digest256::of_bytes(&cmd::canonical(&body)?).to_hex()
    );
    cmd::set(&mut body, "comparison_id", cmd::string(&comparison_id))?;
    let comparison = envelope(&comparison_id, &cmd::number(1), &body, &target.origin_id)?;
    let source_file_identity = format!("{:?}", acquired.identity);
    let pin = CurrentnessPin::Epub {
        config: access_config,
        config_raw: target.config_raw.clone(),
        config_path: target.config_path.clone(),
        entry: source.payload_entry,
        identity: acquired.identity,
        member_sha256: Digest256::of_bytes(&acquired.raw).to_hex(),
    };
    let layer_snapshot = Digest256::of_bytes(&cmd::canonical(&cmd::object(vec![
        (
            "owner_context",
            cmd::string(&owner.snapshot(deadline, cancelled)?.to_prefixed()),
        ),
        (
            "comparison_sha256",
            cmd::string(&Digest256::of_bytes(&comparison.envelope).to_hex()),
        ),
        ("source_file", cmd::string(&source_file_identity)),
    ]))?)
    .to_prefixed();
    let input_cost = source_input_bytes
        .checked_add(acquired.raw.len() as u64)
        .ok_or(SourceCommandError::Invalid(
            "private assessment EPUB comparison byte cost overflow",
        ))?;
    Ok((comparison, vec![pin], layer_snapshot, input_cost))
}

const XHTML_DEFAULT_POLICY: &str = r#"{
  "schema_version":"tos_xhtml_text_extraction_policy_v1",
  "method":"tos.xhtml.character-data.v1",
  "input_media_type":"application/xhtml+xml",
  "encoding":"UTF-8-strict",
  "xml_version":"1.0",
  "namespace":"http://www.w3.org/1999/xhtml",
  "carriage_returns":"reject-input-profile-limitation",
  "entity_and_character_references":"reject-all-ampersands",
  "doctype_and_processing_instructions":"reject-except-xml-declaration",
  "comments":"omit-bounded-markup-preserve-surrounding-character-data",
  "cdata":"literal-character-data-under-member-and-text-budgets",
  "markup_token_max_bytes":16384,
  "attributes_per_start_tag_max":128,
  "selector_schemes":["tos.xhtml.element-ordinal.v1","tos.xhtml.element-id.v1"],
  "ordinal_basis":"one-based-document-order-exact-namespace-and-local-name",
  "selected_root_elements":["a","abbr","b","bdi","bdo","blockquote","cite","code","dfn","em","h1","h2","h3","h4","h5","h6","i","kbd","li","mark","p","pre","q","s","samp","small","span","strong","sub","sup","time","u","var"],
  "inline_elements":["a","abbr","b","bdi","bdo","cite","code","dfn","em","i","kbd","mark","q","s","samp","small","span","strong","sub","sup","time","u","var"],
  "line_break_element":"br-to-one-LF",
  "text_nodes":"concatenate-in-document-order-without-selected-root-tail",
  "whitespace":"preserve-no-trim-collapse-or-inserted-block-separators",
  "unicode_normalization":"none",
  "attributes":"XML-1.0-parsed-identifiers-only-not-rendered",
  "unknown_selected_markup":"reject",
  "quality_assessment":"not-performed",
  "uncertainty":"none-recorded-is-not-reviewed-absence"
}"#;

fn read_lineage_layer(
    owner: &OwnerTextContext,
    worker: &mut CutWorkerSchemaExecutor,
    binding: &JsonValue,
    origin_id: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<TargetLayer> {
    let selected = cmd::field(binding, "text_layer")?;
    let reference = cmd::text(selected, "record_ref")?.to_owned();
    if !safe_metadata_ref(&reference) {
        return Err(SourceCommandError::Denied(
            "private assessment lineage TextLayer locator",
        ));
    }
    let raw = owner.read(&reference, MAX_RECORD_BYTES, deadline, cancelled)?;
    if Digest256::of_bytes(&raw).to_hex() != cmd::text(selected, "record_sha256")? {
        return Err(SourceCommandError::Conflict(
            "private assessment exact lineage TextLayer digest differs",
        ));
    }
    let layer = cmd::parse(&raw)?;
    crate::source_text_layer_entry::checked_schema(
        worker,
        &reference,
        &raw,
        LAYER_SCHEMA,
        deadline,
        cancelled,
    )?;
    validate_layer_metadata(&raw, &reference, deadline, cancelled)?;
    if cmd::field(&layer, "layer_id")? != cmd::field(selected, "layer_id")?
        || cmd::field(&layer, "layer_version")? != cmd::field(selected, "layer_version")?
    {
        return Err(SourceCommandError::Conflict(
            "private assessment lineage TextLayer identity differs",
        ));
    }
    let maker = cmd::field(cmd::field(&layer, "derivation")?, "maker")?;
    let config_ref = cmd::text(maker, "configuration_ref")?.to_owned();
    let expected_config = format!(
        "{}/source-create-owner-configuration.json",
        reference
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid(
                "private assessment lineage package locator",
            ))?
            .0
    );
    if config_ref != expected_config {
        return Err(SourceCommandError::Conflict(
            "private assessment lineage configuration locator differs",
        ));
    }
    let config_raw = owner.read(&config_ref, MAX_RECORD_BYTES, deadline, cancelled)?;
    if Digest256::of_bytes(&config_raw).to_hex() != cmd::text(maker, "configuration_digest")? {
        return Err(SourceCommandError::Conflict(
            "private assessment lineage configuration digest differs",
        ));
    }
    let config = cmd::parse(&config_raw)?;
    let policy_ref = cmd::text(cmd::field(&layer, "editorial_policy")?, "policy_ref")?;
    let policy_raw = owner.read(policy_ref, MAX_RECORD_BYTES, deadline, cancelled)?;
    if Digest256::of_bytes(&policy_raw).to_hex()
        != cmd::text(cmd::field(&layer, "editorial_policy")?, "policy_sha256")?
    {
        return Err(SourceCommandError::Conflict(
            "private assessment lineage editorial policy digest differs",
        ));
    }
    let policy = cmd::parse(&policy_raw)?;
    if !cmd::same(cmd::field(&config, "policy")?, &policy)? {
        return Err(SourceCommandError::Conflict(
            "private assessment lineage policy differs from its configuration",
        ));
    }
    let scope = layer_source_scope(&layer)?;
    if !cmd::same(cmd::field(&config, "source_scope")?, &scope)?
        || !cmd::same(
            cmd::field(&config, "source_record_refs")?,
            cmd::field(binding, "source_record_refs")?,
        )?
        || cmd::text(&config, "source_path")? != reference
        || cmd::field(&config, "language")?
            != cmd::field(cmd::field(&layer, "representation")?, "language")?
    {
        return Err(SourceCommandError::Conflict(
            "private assessment lineage source configuration differs",
        ));
    }
    let config_path = if config_ref.starts_with("ToS/source-witnesses/owner-local/") {
        owner.private_root().join(&config_ref)
    } else {
        owner.public_root().join(&config_ref)
    };
    let record = envelope(
        cmd::text(&layer, "layer_id")?,
        cmd::field(&layer, "layer_version")?,
        &layer,
        origin_id,
    )?;
    Ok(TargetLayer {
        binding: binding.clone(),
        origin_id: origin_id.to_owned(),
        raw,
        layer,
        record,
        reference,
        config_ref,
        config_path,
        config_raw,
        config,
        policy_raw,
        policy,
        scope,
        subject: JsonValue::Null,
        selection: JsonValue::Null,
    })
}

fn source_view_from_anchor(
    anchor: &JsonValue,
    source_scope: &JsonValue,
    payload_ref: &str,
) -> SourceCommandResult<(JsonValue, JsonValue)> {
    let payload = cmd::field(anchor, "selector_payload")?;
    let expression = cmd::field(payload, "expression")?;
    let steps = cmd::array(expression, "steps")?;
    if cmd::text(payload, "kind")? != "selector_expression"
        || cmd::text(expression, "mode")? != "refinement_chain"
        || steps.len() != 2
    {
        return Err(SourceCommandError::Unsupported(
            "private assessment derived source view requires exact XHTML refinement anchor",
        ));
    }
    let first = &steps[0];
    let member_selector = cmd::field(first, "selector")?;
    cmd::exact_keys(
        member_selector,
        &["type", "member_path", "member_sha256", "member_media_type"],
    )?;
    if cmd::text(member_selector, "type")? != "container_member"
        || cmd::text(member_selector, "member_media_type")? != "application/xhtml+xml"
    {
        return Err(SourceCommandError::Unsupported(
            "private assessment derived source view member media type",
        ));
    }
    let member = cmd::object(vec![
        (
            "member_path",
            cmd::field(member_selector, "member_path")?.clone(),
        ),
        (
            "member_sha256",
            cmd::field(member_selector, "member_sha256")?.clone(),
        ),
    ]);
    let member_path = cmd::text(&member, "member_path")?;
    let member_sha = cmd::text(&member, "member_sha256")?;
    let expected_first = cmd::object(vec![
        (
            "state",
            cmd::object(vec![
                ("state_type", cmd::string("digest_state")),
                ("representation_ref", cmd::string(payload_ref)),
                (
                    "representation_sha256",
                    cmd::field(source_scope, "file_sha256")?.clone(),
                ),
                ("media_type", cmd::string("application/epub+zip")),
            ]),
        ),
        (
            "selector",
            cmd::object(vec![
                ("type", cmd::string("container_member")),
                ("member_path", cmd::string(member_path)),
                ("member_sha256", cmd::string(member_sha)),
                ("member_media_type", cmd::string("application/xhtml+xml")),
            ]),
        ),
    ]);
    let second = &steps[1];
    let selector = cmd::field(second, "selector")?.clone();
    let expected_second = cmd::object(vec![
        (
            "state",
            cmd::object(vec![
                ("state_type", cmd::string("digest_state")),
                ("representation_ref", cmd::string(member_path)),
                ("representation_sha256", cmd::string(member_sha)),
                ("media_type", cmd::string("application/xhtml+xml")),
            ]),
        ),
        ("selector", selector.clone()),
    ]);
    if !cmd::same(first, &expected_first)? || !cmd::same(second, &expected_second)? {
        return Err(SourceCommandError::Unsupported(
            "private assessment derived source view anchor is outside exact XHTML profile",
        ));
    }
    let default_policy = cmd::parse(XHTML_DEFAULT_POLICY.as_bytes())?;
    validate_extraction_profile(&selector, &default_policy)?;
    Ok((member, selector))
}

fn compare_derived_layer(
    owner: &mut OwnerTextContext,
    worker: &mut CutWorkerSchemaExecutor,
    target: &mut TargetLayer,
    payload_access: &JsonValue,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(
    AssessmentRecordInput,
    bool,
    Vec<CurrentnessPin>,
    String,
    u64,
)> {
    validate_payload_grant(payload_access, MAX_ORIGINAL_V5)?;
    let mut lineage = Vec::<TargetLayer>::new();
    let mut current = read_lineage_layer(
        owner,
        worker,
        &target.binding,
        &target.origin_id,
        deadline,
        cancelled,
    )?;
    let expected_scope = current.scope.clone();
    let expected_source_binding = cmd::field(&current.layer, "source_binding")?.clone();
    let mut seen = BTreeSet::new();
    let root_is_initial;
    let root_source_access;
    let mut payload_entry;
    let mut root_source_inputs = Vec::new();
    let mut lineage_inputs = Vec::<NativeInput>::new();
    loop {
        if lineage.len() >= MAX_LINEAGE {
            return Err(SourceCommandError::Invalid(
                "private assessment derived layer lineage depth",
            ));
        }
        let layer_id = cmd::text(&current.layer, "layer_id")?.to_owned();
        if !seen.insert(layer_id) {
            return Err(SourceCommandError::Invalid(
                "private assessment derived layer lineage cycle",
            ));
        }
        if !cmd::same(&current.scope, &expected_scope)?
            || !cmd::same(
                cmd::field(&current.layer, "source_binding")?,
                &expected_source_binding,
            )?
        {
            return Err(SourceCommandError::Conflict(
                "private assessment derived layer lineage source scope differs",
            ));
        }
        let representation = cmd::field(&current.layer, "representation")?;
        if cmd::text(representation, "content_visibility")? != "local_only"
            || cmd::field(representation, "publication_authorized")? != &JsonValue::Bool(false)
            || cmd::integer(cmd::field(representation, "text_scope")?, "start")? != 0
        {
            return Err(SourceCommandError::Unsupported(
                "private assessment derived lineage needs a private whole-layer representation",
            ));
        }
        let (representation_text, resolved) = read_whole_text_layer(
            owner,
            worker,
            &current.binding,
            &current.layer,
            deadline,
            cancelled,
        )?;
        lineage_inputs.extend(resolved.inputs);
        if representation_text.len() > 1_048_576 {
            return Err(SourceCommandError::Unsupported(
                "private assessment derived lineage record text budget",
            ));
        }
        current.selection = cmd::object(vec![(
            "representation_text",
            cmd::string(&representation_text),
        )]);
        lineage.push(current);
        let last = lineage.last().ok_or(SourceCommandError::Invalid(
            "private assessment derived layer lineage absent",
        ))?;
        let config = &last.config;
        let config_schema = cmd::text(config, "schema_version")?;
        if config_schema == INITIAL_CONFIG {
            validate_initial_layer_profile(last)?;
            root_is_initial = true;
            let mut access_config = config.clone();
            cmd::set(&mut access_config, "source_access", payload_access.clone())?;
            let grant = OwnerTextInitialLayerSelection {
                config: access_config.clone(),
                raw: last.config_raw.clone(),
                path: last.config_path.clone(),
            };
            let source =
                resolve_initial_owner_text_source(owner, worker, &grant, deadline, cancelled)?;
            validate_initial_anchor(
                owner,
                worker,
                last,
                &source.payload_entry,
                deadline,
                cancelled,
            )?;
            payload_entry = source.payload_entry;
            root_source_inputs = source.inputs;
            root_source_access = access_config;
            break;
        }
        if config_schema != DERIVED_CONFIG {
            return Err(SourceCommandError::Unsupported(
                "private assessment v5 lineage supports only extraction and ordinary supplied/derived profiles",
            ));
        }
        let operations = cmd::array(config, "allowed_operations")?;
        if operations.len() != 1 {
            return Err(SourceCommandError::Unsupported(
                "private assessment derived operation cardinality",
            ));
        }
        let operation = operations[0].as_str().ok_or(SourceCommandError::Invalid(
            "private assessment derived operation",
        ))?;
        if !DERIVED_OPERATIONS.contains(&operation) {
            return Err(SourceCommandError::Unsupported(
                "private assessment v5 derived operation profile",
            ));
        }
        let input = cmd::field(config, "input")?;
        if operation == "text-layer.correct" || operation == "text-layer.normalize" {
            if cmd::text(input, "kind")? != "text_layer" {
                return Err(SourceCommandError::Conflict(
                    "private assessment transform lineage lacks exact TextLayer predecessor",
                ));
            }
            let predecessor_binding = cmd::field(input, "binding")?;
            if !cmd::same(
                cmd::field(predecessor_binding, "source_record_refs")?,
                cmd::field(config, "source_record_refs")?,
            )? {
                return Err(SourceCommandError::Conflict(
                    "private assessment derived predecessor source records differ",
                ));
            }
            let predecessor = read_lineage_layer(
                owner,
                worker,
                predecessor_binding,
                &target.origin_id,
                deadline,
                cancelled,
            )?;
            current = predecessor;
            continue;
        }
        if cmd::text(input, "kind")? == "text_layer" {
            return Err(SourceCommandError::Conflict(
                "private assessment supplied-result lineage unexpectedly selects a TextLayer",
            ));
        }
        root_is_initial = false;
        let mut access_config = config.clone();
        cmd::set(&mut access_config, "source_access", payload_access.clone())?;
        let source: ResolvedDerivedTextSource =
            resolve_derived_owner_text_source(owner, worker, &access_config, deadline, cancelled)?;
        payload_entry = source.payload_entry;
        root_source_inputs = source.inputs;
        root_source_access = access_config;
        break;
    }
    if lineage.is_empty() {
        return Err(SourceCommandError::Invalid(
            "private assessment derived lineage lacks an origin",
        ));
    }
    let root = lineage.last().ok_or(SourceCommandError::Invalid(
        "private assessment derived origin absent",
    ))?;
    let (member, selector, source_policy) = if root_is_initial {
        (
            cmd::field(&root.config, "member")?.clone(),
            cmd::field(&root.config, "selector")?.clone(),
            cmd::field(&root.config, "policy")?.clone(),
        )
    } else {
        let anchors = cmd::array(cmd::field(&root.layer, "source_binding")?, "anchors")?;
        if anchors.len() != 1 {
            return Err(SourceCommandError::Unsupported(
                "private assessment supplied origin requires one exact source anchor",
            ));
        }
        let anchor_target = &anchors[0];
        let anchor_ref = cmd::text(anchor_target, "anchor_record_ref")?;
        let anchor_raw = owner.read(anchor_ref, MAX_RECORD_BYTES, deadline, cancelled)?;
        if Digest256::of_bytes(&anchor_raw).to_hex()
            != cmd::text(anchor_target, "anchor_record_sha256")?
        {
            return Err(SourceCommandError::Conflict(
                "private assessment supplied source anchor digest differs",
            ));
        }
        let anchor = cmd::parse(&anchor_raw)?;
        let payload_ref = layer_source_payload_ref(
            cmd::field(&root.config, "source_record_refs")?,
            &payload_entry,
        )?;
        let (member, selector) = source_view_from_anchor(&anchor, &root.scope, &payload_ref)?;
        let source_policy = cmd::parse(XHTML_DEFAULT_POLICY.as_bytes())?;
        (member, selector, source_policy)
    };
    let selector_token = validate_extraction_profile(&selector, &source_policy)?;
    let mut payload_config = root_source_access.clone();
    cmd::set(&mut payload_config, "member", member.clone())?;
    let grant = OwnerTextInitialLayerSelection {
        config: payload_config.clone(),
        raw: root.config_raw.clone(),
        path: root.config_path.clone(),
    };
    let acquired = read_acquired_epub_member(owner, &grant, &payload_entry, deadline, cancelled)?;
    validate_payload_grant(payload_access, MAX_ORIGINAL_V5)?;
    if acquired.raw.len() > MAX_SOURCE_BYTES_V5 {
        return Err(SourceCommandError::Unsupported(
            "private assessment derived original source byte budget",
        ));
    }
    let source_member = std::str::from_utf8(&acquired.raw)
        .map_err(|_| SourceCommandError::Invalid("private assessment derived XHTML UTF-8"))?;
    if source_member.len() > 1_048_576 {
        return Err(SourceCommandError::Unsupported(
            "private assessment derived source view record budget",
        ));
    }
    let source_text = extract_xhtml_text(&acquired.raw, selector_token, deadline, cancelled)?;
    validate_payload_grant(payload_access, MAX_ORIGINAL_V5)?;
    if source_text.len() > 1_048_576 {
        return Err(SourceCommandError::Unsupported(
            "private assessment derived source text record budget",
        ));
    }
    let mut fixity_inputs = root_source_inputs;
    fixity_inputs.extend(lineage_inputs);
    let source_input_bytes = input_byte_cost(&fixity_inputs)?;
    let mut fixity = input_fixity(&fixity_inputs)?;
    let payload_ref = layer_source_payload_ref(
        cmd::field(&root.config, "source_record_refs")?,
        &payload_entry,
    )?;
    append_fixity(
        &mut fixity,
        &payload_ref,
        "original_payload",
        cmd::text(&root.scope, "file_sha256")?,
    )?;
    append_fixity(
        &mut fixity,
        &format!("{}!/{}", payload_ref, cmd::text(&member, "member_path")?),
        "source_member",
        cmd::text(&member, "member_sha256")?,
    )?;
    let target_text = lineage
        .first()
        .and_then(|node| node.selection.object_get("representation_text"))
        .and_then(JsonValue::as_str)
        .ok_or(SourceCommandError::Invalid(
            "private assessment derived target representation text absent",
        ))?;
    let lineage_rows = lineage
        .iter()
        .rev()
        .map(|node| {
            let operation = if cmd::text(&node.config, "schema_version")? == INITIAL_CONFIG {
                "text-layer.extract"
            } else {
                cmd::array(&node.config, "allowed_operations")?
                    .first()
                    .and_then(JsonValue::as_str)
                    .ok_or(SourceCommandError::Invalid(
                        "private assessment derived lineage operation absent",
                    ))?
            };
            let reported_producer = cmd::field(&node.config, "material")
                .ok()
                .and_then(|material| material.object_get("reported_maker"))
                .cloned()
                .unwrap_or(JsonValue::Null);
            let representation_text = node
                .selection
                .object_get("representation_text")
                .cloned()
                .ok_or(SourceCommandError::Invalid(
                    "private assessment derived lineage text absent",
                ))?;
            Ok(cmd::object(vec![
                ("record", record_reference(&node.layer)?),
                ("record_ref", cmd::string(&node.reference)),
                ("record_payload", node.layer.clone()),
                ("representation_text", representation_text),
                ("editorial_policy", node.policy.clone()),
                (
                    "configuration_sha256",
                    cmd::string(cmd::text(
                        cmd::field(cmd::field(&node.layer, "derivation")?, "maker")?,
                        "configuration_digest",
                    )?),
                ),
                ("operation", cmd::string(operation)),
                ("reported_producer", reported_producer),
            ]))
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    let source_view = cmd::object(vec![
        ("member", member),
        ("selector", selector),
        ("source_member_utf8", cmd::string(source_member)),
        ("selected_text", cmd::string(&source_text)),
        ("policy", source_policy),
        (
            "method",
            cmd::string("bounded-XHTML-character-data-not-textual-quality"),
        ),
    ]);
    let mut body = cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_native_text_layer_derivation_comparison_v1"),
        ),
        ("comparison_version", cmd::number(1)),
        ("layer", record_reference(&target.layer)?),
        ("source_scope", target.scope.clone()),
        ("source_view", source_view),
        ("lineage", JsonValue::Array(lineage_rows)),
        ("input_fixity", JsonValue::Array(fixity)),
        ("transformation_integrity", JsonValue::Bool(true)),
        (
            "source_text_equals_output",
            JsonValue::Bool(source_text == target_text),
        ),
        (
            "positive_use_boundary",
            cmd::string("source-visible-assessment-required-not-byte-equality"),
        ),
        (
            "provider_execution",
            cmd::string("not-attested-by-comparison"),
        ),
        ("inherited_quality", cmd::string("not-transferred")),
        ("visibility", cmd::string("local_only")),
        ("publication_authorized", JsonValue::Bool(false)),
        ("performs_semantic_assessment", JsonValue::Bool(false)),
    ]);
    if cmd::canonical(&body)?.len() > MAX_RECORD_BYTES {
        return Err(SourceCommandError::Unsupported(
            "private assessment derived comparison aggregate record budget",
        ));
    }
    let comparison_id = format!(
        "tos.text-comparison.sha256.{}",
        Digest256::of_bytes(&cmd::canonical(&body)?).to_hex()
    );
    cmd::set(&mut body, "comparison_id", cmd::string(&comparison_id))?;
    let comparison = envelope(&comparison_id, &cmd::number(1), &body, &target.origin_id)?;
    let source_file_identity = format!("{:?}", acquired.identity);
    let pin = CurrentnessPin::Epub {
        config: payload_config,
        config_raw: root.config_raw.clone(),
        config_path: root.config_path.clone(),
        entry: payload_entry,
        identity: acquired.identity,
        member_sha256: Digest256::of_bytes(&acquired.raw).to_hex(),
    };
    let layer_snapshot = Digest256::of_bytes(&cmd::canonical(&cmd::object(vec![
        (
            "owner_context",
            cmd::string(&owner.snapshot(deadline, cancelled)?.to_prefixed()),
        ),
        (
            "comparison_sha256",
            cmd::string(&Digest256::of_bytes(&comparison.envelope).to_hex()),
        ),
        ("source_file", cmd::string(&source_file_identity)),
    ]))?)
    .to_prefixed();
    let input_cost = source_input_bytes
        .checked_add(acquired.raw.len() as u64)
        .ok_or(SourceCommandError::Invalid(
            "private assessment derived comparison byte cost overflow",
        ))?;
    Ok((comparison, true, vec![pin], layer_snapshot, input_cost))
}

fn read_exact_png(
    access: &JsonValue,
    input: &JsonValue,
    uid: u32,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<ImageRead> {
    validate_image_grant(access)?;
    let path = normalized_absolute(cmd::text(access, "path")?)?;
    let limit = cmd::integer(access, "byte_size")?;
    let before_parents = parents(&path, uid, deadline, cancelled)?;
    let mut file = tos_fd_open::open_absolute_regular(&path, MAX_PAGE_IMAGE).map_err(|_| {
        SourceCommandError::Denied("private assessment exact PNG is absent or unsafe")
    })?;
    let before = owned_file(&file, uid)?;
    if before.len() != limit || before.len() > MAX_PAGE_IMAGE {
        return Err(SourceCommandError::Conflict(
            "private assessment exact PNG size differs from its grant",
        ));
    }
    let mut raw = Vec::with_capacity(before.len() as usize);
    Read::by_ref(&mut file)
        .take(MAX_PAGE_IMAGE + 1)
        .read_to_end(&mut raw)
        .map_err(|_| SourceCommandError::Denied("private assessment exact PNG read"))?;
    if raw.len() as u64 != limit {
        return Err(SourceCommandError::Conflict(
            "private assessment exact PNG changed while reading",
        ));
    }
    let after = owned_file(&file, uid)?;
    let current = tos_fd_open::open_absolute_regular(&path, MAX_PAGE_IMAGE).map_err(|_| {
        SourceCommandError::Conflict("private assessment exact PNG path was replaced")
    })?;
    let at_path = owned_file(&current, uid)?;
    let after_parents = parents(&path, uid, deadline, cancelled)?;
    let image_sha = Digest256::of_bytes(&raw).to_hex();
    if file_identity(&before) != file_identity(&after)
        || file_identity(&before) != file_identity(&at_path)
        || before_parents != after_parents
        || image_sha != cmd::text(access, "sha256")?
        || image_sha != cmd::text(input, "input_sha256")?
        || raw.len() < 33
        || raw
            .get(..8)
            .is_none_or(|signature| signature != b"\x89PNG\r\n\x1a\n")
        || raw.get(12..16).is_none_or(|kind| kind != b"IHDR")
        || u32::from_be_bytes(
            raw.get(16..20)
                .ok_or(SourceCommandError::Invalid("private assessment PNG width"))?
                .try_into()
                .map_err(|_| SourceCommandError::Invalid("private assessment PNG width"))?,
        ) as u64
            != cmd::integer(access, "width_pixels")?
        || u32::from_be_bytes(
            raw.get(20..24)
                .ok_or(SourceCommandError::Invalid("private assessment PNG height"))?
                .try_into()
                .map_err(|_| SourceCommandError::Invalid("private assessment PNG height"))?,
        ) as u64
            != cmd::integer(access, "height_pixels")?
        || raw.get(24..26).is_none_or(|kind| kind != b"\x08\x02")
        || cmd::integer(input, "width_pixels")? != cmd::integer(access, "width_pixels")?
        || cmd::integer(input, "height_pixels")? != cmd::integer(access, "height_pixels")?
    {
        return Err(SourceCommandError::Conflict(
            "private assessment exact image bytes or dimensions changed",
        ));
    }
    Ok(ImageRead {
        raw,
        identity: PayloadIdentity {
            file: file_identity(&before),
            parents: before_parents,
        },
    })
}

fn compare_page_ocr(
    owner: &mut OwnerTextContext,
    worker: &mut CutWorkerSchemaExecutor,
    target: &TargetLayer,
    payload_access: &JsonValue,
    selection: &JsonValue,
    source_manifest: &JsonValue,
    anchor: &SelectedLayerAnchor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(
    AssessmentRecordInput,
    Vec<CurrentnessPin>,
    String,
    BTreeMap<String, String>,
    u64,
)> {
    let retained = cmd::text(selection, "comparison_profile")? == RETAINED_PAGE_PROFILE;
    let config = &target.config;
    let material = cmd::field(config, "material")?;
    let access_grant = payload_access;
    validate_payload_grant(access_grant, MAX_ORIGINAL_V6)?;
    let mut current_config = config.clone();
    cmd::set(&mut current_config, "source_access", access_grant.clone())?;
    let source: ResolvedDerivedTextSource =
        resolve_derived_owner_text_source(owner, worker, &current_config, deadline, cancelled)?;
    let entry = source.payload_entry.clone();
    let expected_media = if retained {
        "application/pdf"
    } else {
        "image/png"
    };
    let matching_entries = cmd::array(source_manifest, "payload_files")?
        .iter()
        .filter(|row| row.object_get("file_id") == target.scope.object_get("file_ref"))
        .collect::<Vec<_>>();
    if matching_entries.len() != 1
        || cmd::text(&entry, "media_type")? != expected_media
        || cmd::text(matching_entries[0], "media_type")? != expected_media
        || !cmd::same(&entry, matching_entries[0])?
        || cmd::integer(&entry, "byte_size")? != cmd::integer(access_grant, "byte_size")?
    {
        return Err(SourceCommandError::Conflict(
            "private assessment acquired image source differs from its current manifest grant",
        ));
    }
    let source_identity =
        verify_acquired_file(owner, &current_config, &entry, deadline, cancelled)?;
    validate_payload_grant(access_grant, MAX_ORIGINAL_V6)?;
    let selected_input = if retained {
        cmd::field(material, "input_representation")?.clone()
    } else {
        let image = cmd::field(selection, "image_access")?;
        cmd::object(vec![
            (
                "schema_version",
                cmd::string("tos_operator_synthetic_png_input_binding_v1"),
            ),
            (
                "source_file_ref",
                cmd::field(&target.scope, "file_ref")?.clone(),
            ),
            (
                "source_file_sha256",
                cmd::field(&target.scope, "file_sha256")?.clone(),
            ),
            (
                "input_file_ref",
                cmd::field(&target.scope, "file_ref")?.clone(),
            ),
            (
                "input_sha256",
                cmd::field(&target.scope, "file_sha256")?.clone(),
            ),
            (
                "input_bytes",
                cmd::field(cmd::field(config, "source_access")?, "byte_size")?.clone(),
            ),
            ("media_type", cmd::string("image/png")),
            ("page_number", cmd::number(1)),
            ("page_index_origin", cmd::number(1)),
            ("width_pixels", cmd::field(image, "width_pixels")?.clone()),
            ("height_pixels", cmd::field(image, "height_pixels")?.clone()),
        ])
    };
    let image_access = cmd::field(selection, "image_access")?;
    validate_image_matches_input(image_access, &selected_input, &target.scope)?;
    let image = read_exact_png(
        image_access,
        &selected_input,
        owner.account_uid(),
        deadline,
        cancelled,
    )?;
    validate_image_grant(image_access)?;
    if retained
        && (cmd::integer(&selected_input, "input_bytes")? != image.identity.file.4
            || cmd::text(&selected_input, "input_file_ref")?
                != format!(
                    "tos.file.sha256.{}",
                    cmd::text(&selected_input, "input_sha256")?
                ))
    {
        return Err(SourceCommandError::Conflict(
            "private assessment retained page image differs from OCR input binding",
        ));
    }
    if !retained && cmd::integer(&selected_input, "input_bytes")? != source_identity.file.4 {
        return Err(SourceCommandError::Conflict(
            "private assessment synthetic source PNG size differs from its retained configuration",
        ));
    }
    let image_width = cmd::integer(image_access, "width_pixels")?;
    let image_height = cmd::integer(image_access, "height_pixels")?;
    if image_width
        .checked_mul(image_height)
        .is_none_or(|n| n > MAX_PAGE_PIXELS)
    {
        return Err(SourceCommandError::Invalid(
            "private assessment exact PNG exceeds pixel budget",
        ));
    }
    let representation = cmd::field(&target.layer, "representation")?;
    let content_raw = owner.read(
        cmd::text(representation, "content_ref")?,
        MAX_OCR_TEXT_BYTES,
        deadline,
        cancelled,
    )?;
    if Digest256::of_bytes(&content_raw).to_hex() != cmd::text(representation, "content_sha256")?
        || Digest256::of_bytes(&content_raw).to_hex() != cmd::text(material, "content_sha256")?
        || content_raw.len() as u64 != cmd::integer(material, "byte_size")?
    {
        return Err(SourceCommandError::Conflict(
            "private assessment owner OCR representation bytes differ from its grant",
        ));
    }
    let representation_text = std::str::from_utf8(&content_raw)
        .map_err(|_| SourceCommandError::Invalid("private assessment owner OCR UTF-8"))?;
    if representation_text.is_empty()
        || representation_text.len() > MAX_OCR_TEXT_BYTES
        || cmd::integer(cmd::field(representation, "text_scope")?, "start")? != 0
        || cmd::integer(cmd::field(representation, "text_scope")?, "end")?
            != representation_text.chars().count() as u64
    {
        return Err(SourceCommandError::Invalid(
            "private assessment image comparison requires whole exact OCR text",
        ));
    }
    let package = target
        .reference
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .ok_or(SourceCommandError::Invalid(
            "private assessment owner OCR package reference",
        ))?;
    let evidence_dir = owner.private_root().join(package);
    let mut copied = BTreeMap::new();
    for (name, limit) in [
        ("owner-ocr-receipt.json", 128 * 1024),
        ("owner-ocr-signature.sigstore.json", 64 * 1024),
        ("owner-ocr-signer.pub", 4096),
    ] {
        let raw = owner.read(&format!("{package}/{name}"), limit, deadline, cancelled)?;
        copied.insert(name.to_owned(), raw);
    }
    source_text_owner_ocr::verify_record(
        material,
        &target.scope,
        cmd::text(representation, "language")?,
        retained,
        &evidence_dir,
        &copied,
        owner.account_uid(),
        deadline,
        cancelled,
    )?;
    source_text_owner_ocr::validate_material(material, retained)?;
    cmd::validate_expiry(
        cmd::text(material, "expires_at")?,
        &crate::source_serialization::instant()?,
    )?;
    let evidence_bytes = copied.values().try_fold(0u64, |sum, raw| {
        sum.checked_add(raw.len() as u64)
            .ok_or(SourceCommandError::Invalid(
                "private assessment OCR evidence byte cost overflow",
            ))
    })?;
    let receipt = cmd::parse(copied.get("owner-ocr-receipt.json").ok_or(
        SourceCommandError::Invalid("private assessment owner OCR receipt absent"),
    )?)?;
    let disclosure = cmd::field(selection, "disclosure_access")?.clone();
    let disclosed = disclosure != JsonValue::Null;
    if disclosed {
        validate_disclosure(&disclosure, cmd::field(&target.binding, "text_layer")?)?;
    }
    let boundary = if disclosed {
        "current_assistant_session"
    } else {
        "local_only"
    };
    let source_ref = layer_source_payload_ref(cmd::field(config, "source_record_refs")?, &entry)?;
    let source_input_bytes = input_byte_cost(&source.inputs)?;
    let mut fixity = input_fixity(&source.inputs)?;
    append_fixity(
        &mut fixity,
        &target.reference,
        "metadata",
        &Digest256::of_bytes(&target.raw).to_hex(),
    )?;
    append_fixity(
        &mut fixity,
        &target.config_ref,
        "metadata",
        &Digest256::of_bytes(&target.config_raw).to_hex(),
    )?;
    let policy_ref = cmd::text(cmd::field(&target.layer, "editorial_policy")?, "policy_ref")?;
    append_fixity(
        &mut fixity,
        policy_ref,
        "metadata",
        &Digest256::of_bytes(&target.policy_raw).to_hex(),
    )?;
    append_fixity(
        &mut fixity,
        cmd::text(&anchor.reference, "anchor_record_ref")?,
        "metadata",
        cmd::text(&anchor.reference, "anchor_record_sha256")?,
    )?;
    append_fixity(
        &mut fixity,
        cmd::text(representation, "content_ref")?,
        "content",
        cmd::text(representation, "content_sha256")?,
    )?;
    for rights in cmd::array(representation, "rights_record_refs")? {
        append_fixity(
            &mut fixity,
            cmd::text(rights, "ref")?,
            "metadata",
            cmd::text(rights, "sha256")?,
        )?;
    }
    for (name, raw) in &copied {
        append_fixity(
            &mut fixity,
            &format!("{package}/{name}"),
            "metadata",
            &Digest256::of_bytes(raw).to_hex(),
        )?;
    }
    append_fixity(
        &mut fixity,
        &source_ref,
        "original_payload",
        cmd::text(&target.scope, "file_sha256")?,
    )?;
    append_fixity(
        &mut fixity,
        cmd::text(image_access, "path")?,
        "retained_image",
        cmd::text(&selected_input, "input_sha256")?,
    )?;
    let mut body = cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_native_page_ocr_comparison_v1"),
        ),
        ("comparison_version", cmd::number(1)),
        (
            "comparison_profile",
            cmd::string(cmd::text(selection, "comparison_profile")?),
        ),
        ("layer", record_reference(&target.layer)?),
        ("source_scope", target.scope.clone()),
        ("source_anchor", anchor.reference.clone()),
        ("input_representation", selected_input.clone()),
        (
            "source_image",
            cmd::object(vec![
                ("path", cmd::field(image_access, "path")?.clone()),
                ("media_type", cmd::string("image/png")),
                (
                    "sha256",
                    cmd::field(&selected_input, "input_sha256")?.clone(),
                ),
                (
                    "byte_size",
                    cmd::field(&selected_input, "input_bytes")?.clone(),
                ),
                (
                    "width_pixels",
                    cmd::field(&selected_input, "width_pixels")?.clone(),
                ),
                (
                    "height_pixels",
                    cmd::field(&selected_input, "height_pixels")?.clone(),
                ),
                (
                    "page_number",
                    cmd::field(&selected_input, "page_number")?.clone(),
                ),
                ("page_index_origin", cmd::number(1)),
                ("processing_boundary", cmd::string(boundary)),
                ("model_disclosure_authorized", JsonValue::Bool(disclosed)),
            ]),
        ),
        ("representation_text", cmd::string(representation_text)),
        ("representation", representation.clone()),
        ("editorial_policy", cmd::field(config, "policy")?.clone()),
        (
            "maker",
            cmd::field(cmd::field(&target.layer, "derivation")?, "maker")?.clone(),
        ),
        (
            "configuration_sha256",
            cmd::field(
                cmd::field(cmd::field(&target.layer, "derivation")?, "maker")?,
                "configuration_digest",
            )?
            .clone(),
        ),
        ("input_fixity", JsonValue::Array(fixity)),
        (
            "owner_execution",
            cmd::object(vec![
                (
                    "receipt_sha256",
                    cmd::field(material, "receipt_sha256")?.clone(),
                ),
                ("owner", cmd::field(&receipt, "owner")?.clone()),
                (
                    "input_verification",
                    receipt
                        .object_get("input_verification")
                        .cloned()
                        .unwrap_or(JsonValue::Null),
                ),
            ]),
        ),
        ("disclosure_access", disclosure),
        ("deterministic_text_match", JsonValue::Null),
        ("source_visible_judgment", cmd::string("not_performed")),
        (
            "historical_render_fidelity",
            cmd::string(if retained {
                "not_independently_assessed"
            } else {
                "not_applicable"
            }),
        ),
        (
            "limits",
            JsonValue::Array(
                page_comparison_limits(cmd::text(selection, "comparison_profile")?)
                    .iter()
                    .map(|limit| cmd::string(limit))
                    .collect(),
            ),
        ),
        ("visibility", cmd::string("local_only")),
        ("publication_authorized", JsonValue::Bool(false)),
        ("performs_semantic_assessment", JsonValue::Bool(false)),
    ]);
    let comparison_id = format!(
        "tos.text-comparison.sha256.{}",
        Digest256::of_bytes(&cmd::canonical(&body)?).to_hex()
    );
    cmd::set(&mut body, "comparison_id", cmd::string(&comparison_id))?;
    let comparison = envelope(&comparison_id, &cmd::number(1), &body, &target.origin_id)?;
    validate_schema(
        worker,
        &target.reference,
        &comparison,
        PAGE_COMPARISON_SCHEMA,
        deadline,
        cancelled,
    )?;
    let original_identity = format!("{:?}", source_identity);
    let image_identity = format!("{:?}", image.identity);
    let layer_snapshot = Digest256::of_bytes(&cmd::canonical(&cmd::object(vec![
        (
            "owner_context",
            cmd::string(&owner.snapshot(deadline, cancelled)?.to_prefixed()),
        ),
        (
            "comparison_sha256",
            cmd::string(&Digest256::of_bytes(&comparison.envelope).to_hex()),
        ),
        ("original_identity", cmd::string(&original_identity)),
        ("image_identity", cmd::string(&image_identity)),
        ("source_snapshot", cmd::string(&source.input_snapshot)),
    ]))?)
    .to_prefixed();
    let pins = vec![
        CurrentnessPin::Original {
            config: current_config,
            entry: entry.clone(),
            identity: source_identity.clone(),
        },
        CurrentnessPin::Image {
            access: image_access.clone(),
            input: selected_input.clone(),
            identity: image.identity.clone(),
            sha256: Digest256::of_bytes(&image.raw).to_hex(),
        },
        CurrentnessPin::OwnerOcr {
            material: material.clone(),
            scope: target.scope.clone(),
            language: cmd::text(representation, "language")?.to_owned(),
            page: retained,
            evidence_dir,
            copied,
            uid: owner.account_uid(),
        },
    ];
    let contracts = [(
        PAGE_COMPARISON_SCHEMA,
        worker.contract_digest(PAGE_COMPARISON_SCHEMA),
    )]
    .into_iter()
    .filter_map(|(name, digest)| digest.map(|digest| (name.to_owned(), digest.to_prefixed())))
    .collect();
    let input_cost = source_input_bytes
        .checked_add(content_raw.len() as u64)
        .and_then(|value| value.checked_add(evidence_bytes))
        .ok_or(SourceCommandError::Invalid(
            "private assessment page OCR input byte cost overflow",
        ))?;
    Ok((comparison, pins, layer_snapshot, contracts, input_cost))
}
