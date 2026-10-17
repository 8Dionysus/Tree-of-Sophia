//! Sandboxed pure compiler child for an exact selected Source Navigation cut.
//!
//! Host owner/configuration/PG selection and final publication remain in the
//! parent. This process accepts only a bounded immutable export and returns a
//! candidate plus producer-issued descriptive receipts to the admitted stage.

use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{Read, Write},
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
use tos_compiler::{
    ColdOpenLimits, KnowledgeSelectedExpectation, NativeFsVerityMeasurement, NativeProcessLimits,
    NavigationOriginalReceipt, SourceBinding, SourceCatalogInputLimits, SourceCatalogRenderWorkV1,
    knowledge_stage::{
        ExactInputReceipt, InputCollectionReceipt, KnowledgeStage, StageOwner, StageReceipt,
    },
    managed_source::ManagedSourceProofV2,
    plan_source_catalog_inputs, render_source_bibliographic_plan_with_work,
    source_bibliographic::{BibliographicLimits, BibliographicSourceCut},
    source_witness_catalog::SourceCatalogValidator,
};
use tos_foundation::{Digest256, RelativePath, SourceRevision};
use tos_source_store::{
    CorpusCutReader, CorpusReader, CutReadLimits, ReadLimits, SourceMembershipV1,
};
use tos_validation::{
    executor::{BatchBudget, BatchStreamBudget, ExactWorkerIdentity, ExecutorBudget},
    source_cut::{CutSchemaDiagnosticsLimits, CutWorkerLimits},
};

use crate::source_command::{SourceCommandError, SourceCommandResult};

const REQUEST_SCHEMA: &str = "tos_source_navigation_model_compile_request_v2";
pub(crate) const MAX_SOURCE_HISTORY_REVISIONS: usize = 4;
// The host budget also covers assessment, selected export and publication.
// The isolated compiler keeps its own smaller execution ceiling.
pub(crate) const INITIAL_MAX_TOTAL_MS: u64 = 900_000;
pub(crate) const CHILD_MAX_TOTAL_MS: u64 = 600_000;
const RESULT_SCHEMA: &str = "tos_source_navigation_model_compile_result_v1";
const REQUEST_MAX_BYTES: usize = 8 * 1024 * 1024;
const RESPONSE_MAX_BYTES: usize = 8 * 1024 * 1024;
const MODEL_CANDIDATE_LEAF: &str = "source-navigation-model.sqlite";
const CATALOG_CANDIDATE_LEAF: &str = "source-navigation-catalog.sqlite";
const NORMALIZATION_INPUT_RELATIVE: &str = "inputs/knowledge_normalization.rs";
const ENTITY_REGISTRY_PATH: &str = "ToS/doctrine/semantic-interchange/entity-types.v1.json";
const RELATION_REGISTRY_PATH: &str = "ToS/doctrine/semantic-interchange/relation-types.v1.json";
const QUERY_VOCABULARY_PATH: &str = "ToS/doctrine/semantic-interchange/query-vocabulary.v1.json";
// The process parent accepts only these fixed, non-sensitive child phase names.
pub(super) const SOURCE_NAVIGATION_PHASE_STAGES: &[&str] = &[
    "input_cut_verification",
    "catalog_planning",
    "schema_validation",
    "catalog_render",
    "navigation_projection",
    "managed_model_compile",
    "artifact_publication",
    "candidate_finalization",
];

struct CompilePhaseTrace {
    started: Instant,
}

impl CompilePhaseTrace {
    fn new() -> Self {
        Self {
            started: Instant::now(),
        }
    }

    fn phase(&self, stage: &'static str) -> CompilePhaseGuard<'_> {
        self.write_marker(stage, "begin");
        CompilePhaseGuard { trace: self, stage }
    }

    fn write_marker(&self, stage: &'static str, edge: &'static str) {
        let elapsed_ms = u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let stderr = std::io::stderr();
        let mut output = stderr.lock();
        let _ = writeln!(
            output,
            "tos_source_navigation_phase_v1 stage={stage} edge={edge} elapsed_ms={elapsed_ms}"
        );
        let _ = output.flush();
    }
}

struct CompilePhaseGuard<'a> {
    trace: &'a CompilePhaseTrace,
    stage: &'static str,
}

impl Drop for CompilePhaseGuard<'_> {
    fn drop(&mut self) {
        self.trace.write_marker(self.stage, "end");
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceNavigationModelCompileRequestV2 {
    pub schema_version: String,
    pub read_only_input_root: PathBuf,
    pub source_cut_root: PathBuf,
    /// Initial bootstrap anchor; this may differ from the derived export.
    pub bootstrap_source_revision: String,
    pub bootstrap_membership_count: u64,
    pub bootstrap_membership_sha256: String,
    /// Exact immutable managed export selected by the host parent.
    pub export_revision: String,
    /// Exact retained parser limits derived by the source-current-cut owner.
    pub source_read_limits: SourceNavigationCompileReadLimitsV1,
    /// Exact finite retained-cut bounds derived by the source-current-cut owner.
    pub source_cut_limits: SourceNavigationCompileCutLimitsV1,
    pub membership_count: u64,
    pub membership_sha256: String,
    pub source_binding: SourceBinding,
    pub source_proof_v2: ManagedSourceProofV2,
    pub schema_worker: SourceNavigationCompileWorkerV1,
    pub normalization_processor: SourceNavigationCompileProcessorV1,
    /// Exact separately admitted persistent scratch store, never the final owner path.
    pub artifact_staging_root: PathBuf,
    pub configuration_sha256: String,
    /// Digest of this exact executable image, independently remeasured inside
    /// the child because the dumpable=0 boundary hides `/proc/<pid>/exe` from
    /// the host parent.
    pub native_executable_sha256: String,
    pub operation_budget_ms: u64,
    pub model_limits: SourceNavigationCompileModelLimitsV1,
    pub producer_limits: SourceNavigationCompileProducerLimitsV1,
    pub cold_limits: ColdOpenLimits,
    pub process_limits: NativeProcessLimits,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceNavigationCompileReadLimitsV1 {
    pub max_manifest_bytes: usize,
    pub max_manifest_entries: usize,
    pub max_selected_object_bytes: u64,
    pub json_max_bytes: usize,
    pub json_max_depth: usize,
    pub json_max_visits: usize,
    pub json_max_integer_digits: usize,
}

impl From<ReadLimits> for SourceNavigationCompileReadLimitsV1 {
    fn from(limits: ReadLimits) -> Self {
        Self {
            max_manifest_bytes: limits.max_manifest_bytes,
            max_manifest_entries: limits.max_manifest_entries,
            max_selected_object_bytes: limits.max_selected_object_bytes,
            json_max_bytes: limits.json.max_bytes,
            json_max_depth: limits.json.max_depth,
            json_max_visits: limits.json.max_visits,
            json_max_integer_digits: limits.json.max_integer_digits,
        }
    }
}

impl SourceNavigationCompileReadLimitsV1 {
    fn checked(self) -> SourceCommandResult<ReadLimits> {
        let default_json = tos_foundation::JsonLimits::default();
        if self.max_manifest_bytes == 0
            || self.max_manifest_bytes > crate::source_current_cut::INITIAL_EXPORT_MANIFEST_BYTES
            || self.max_manifest_entries == 0
            || self.max_manifest_entries
                > crate::source_current_cut::INITIAL_EXPORT_MANIFEST_ENTRIES
            || self.max_selected_object_bytes == 0
            || self.max_selected_object_bytes
                > crate::source_current_cut::INITIAL_EXPORT_HISTORY_BYTES
            || self.json_max_bytes == 0
            || self.json_max_bytes > crate::source_current_cut::INITIAL_EXPORT_MANIFEST_BYTES
            || self.json_max_depth == 0
            || self.json_max_depth > default_json.max_depth
            || self.json_max_visits == 0
            || self.json_max_visits > crate::source_current_cut::INITIAL_EXPORT_JSON_VISITS
            || self.json_max_integer_digits == 0
            || self.json_max_integer_digits > default_json.max_integer_digits
        {
            return Err(SourceCommandError::Invalid(
                "source-navigation retained read limits outside bounded history envelope",
            ));
        }
        let limits = ReadLimits {
            max_manifest_bytes: self.max_manifest_bytes,
            max_manifest_entries: self.max_manifest_entries,
            max_selected_object_bytes: self.max_selected_object_bytes,
            json: tos_foundation::JsonLimits {
                max_bytes: self.json_max_bytes,
                max_depth: self.json_max_depth,
                max_visits: self.json_max_visits,
                max_integer_digits: self.json_max_integer_digits,
            },
        };
        limits.validate().map_err(|_| {
            SourceCommandError::Invalid("source-navigation retained read limits are invalid")
        })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceNavigationCompileCutLimitsV1 {
    pub max_revisions: usize,
    pub max_members: u64,
    pub max_total_bytes: u64,
    pub max_member_bytes: u64,
}

impl From<CutReadLimits> for SourceNavigationCompileCutLimitsV1 {
    fn from(limits: CutReadLimits) -> Self {
        Self {
            max_revisions: limits.max_revisions,
            max_members: limits.max_members,
            max_total_bytes: limits.max_total_bytes,
            max_member_bytes: limits.max_member_bytes,
        }
    }
}

impl SourceNavigationCompileCutLimitsV1 {
    fn checked(self) -> SourceCommandResult<CutReadLimits> {
        if self.max_revisions == 0
            || self.max_revisions > MAX_SOURCE_HISTORY_REVISIONS
            || self.max_members == 0
            || self.max_members > crate::source_current_cut::INITIAL_EXPORT_MANIFEST_ENTRIES as u64
            || self.max_total_bytes == 0
            || self.max_total_bytes > crate::source_current_cut::INITIAL_EXPORT_HISTORY_BYTES
            || self.max_member_bytes == 0
            || self.max_member_bytes > crate::source_current_cut::INITIAL_EXPORT_HISTORY_BYTES
        {
            return Err(SourceCommandError::Invalid(
                "source-navigation retained cut limits outside bounded history envelope",
            ));
        }
        Ok(CutReadLimits {
            max_revisions: self.max_revisions,
            max_members: self.max_members,
            max_total_bytes: self.max_total_bytes,
            max_member_bytes: self.max_member_bytes,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceNavigationCompileWorkerV1 {
    pub absolute_path: PathBuf,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceNavigationCompileProcessorV1 {
    pub absolute_path: PathBuf,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceNavigationCompileModelLimitsV1 {
    pub max_total_ms: u64,
    pub host_export_bytes: u64,
    pub host_export_inodes: u64,
    pub tmpfs_bytes: u64,
    pub tmpfs_inodes: u64,
    pub working_ram_bytes: u64,
    pub max_source_members: u64,
    pub max_source_total_bytes: u64,
    pub max_source_member_bytes: u64,
    pub max_plan_bytes: usize,
    pub max_plan_work_bytes: u64,
    pub max_model_bytes: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceNavigationCompileProducerLimitsV1 {
    pub max_events: u64,
    pub schema_execution_wall_ms: u64,
    pub schema_cleanup_grace_ms: u64,
    pub schema_cpu_seconds: u64,
    pub schema_address_space_bytes: u64,
    pub schema_max_receipts: u64,
    pub schema_max_receipt_bytes: u64,
    pub event_limits: SourceNavigationCompileEventLimitsV1,
    pub aggregate_limits: SourceNavigationCompileAggregateLimitsV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceNavigationCompileEventLimitsV1 {
    pub max_dependencies: u64,
    pub max_current_rows: u64,
    pub max_row_bytes: u64,
    pub max_total_bytes: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceNavigationCompileAggregateLimitsV1 {
    pub max_total_rows: u64,
    pub max_total_encoded_bytes: u64,
    pub max_total_dependency_bytes: u64,
    pub max_total_work_units: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceNavigationModelCompileResultV1 {
    pub schema_version: String,
    /// Hash of the exact bounded stdin request, independently checked by the launcher.
    pub raw_request_sha256: String,
    pub candidate_leaf: String,
    pub stage: StageReceipt,
    pub expectation: KnowledgeSelectedExpectation,
    pub navigation_original: NavigationOriginalReceipt,
    pub artifact_fs_verity: NativeFsVerityMeasurement,
    pub catalog_work: SourceNavigationCatalogWorkV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceNavigationCatalogWorkV1 {
    pub bootstrap_members_verified: u64,
    pub bootstrap_bytes_verified: u64,
    pub export_members_verified: u64,
    pub export_bytes_verified: u64,
    pub planned_selected_members: u64,
    pub plan_observed_work_bytes: u64,
    pub rendered_source_members: u64,
    pub rendered_source_payload_bytes: u64,
    pub staged_input_rows: u64,
    pub staged_input_payload_bytes: u64,
    pub planning_elapsed_ms: u64,
    pub rendering_elapsed_ms: u64,
    pub model_elapsed_ms: u64,
}

struct CompileStageOwner<'a> {
    cut: &'a CorpusCutReader,
    export_revision: SourceRevision,
    bootstrap_revision: SourceRevision,
    export_membership: SourceMembershipV1,
    bootstrap_membership: SourceMembershipV1,
    binding: CompileStageBinding<'a>,
    expected_collections: Option<&'a [InputCollectionReceipt]>,
    deadline: Instant,
    cancelled: &'a AtomicBool,
}

#[derive(Clone, Copy)]
enum CompileStageBinding<'a> {
    ExactExport(&'a SourceBinding),
    ManagedModel {
        export_binding: &'a SourceBinding,
        proof: &'a ManagedSourceProofV2,
        max_header_bytes: usize,
    },
}

fn binding_matches(left: &SourceBinding, right: &SourceBinding) -> bool {
    left.owner_profile == right.owner_profile
        && left.source_cut == right.source_cut
        && left.through_commit_seq == right.through_commit_seq
        && left.membership_root == right.membership_root
        && left.index_generation == right.index_generation
        && left.route_map_version == right.route_map_version
        && left.reader_abi == right.reader_abi
        && left.projection_root_sha256 == right.projection_root_sha256
        && left.complete == right.complete
}

fn collections_match(left: &[InputCollectionReceipt], right: &[InputCollectionReceipt]) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            left.source_graph == right.source_graph
                && left.collection == right.collection
                && left.input_role == right.input_role
                && left.adapter_profile == right.adapter_profile
                && left.expected_count == right.expected_count
                && left.expected_root_sha256 == right.expected_root_sha256
        })
}

impl CompileStageOwner<'_> {
    fn check(&self, receipt: &ExactInputReceipt) -> tos_compiler::Result<()> {
        let changed =
            || tos_compiler::Error::Invalid("source-navigation selected cut or receipt changed");
        if crate::source_creation_store::active(self.deadline, self.cancelled).is_err() {
            return Err(changed());
        }
        let receipt_binding_matches = match self.binding {
            CompileStageBinding::ExactExport(expected) => {
                binding_matches(&receipt.binding, expected)
            }
            CompileStageBinding::ManagedModel {
                export_binding,
                proof,
                max_header_bytes,
            } => {
                let expected = tos_compiler::managed_agent_producer::managed_model_receipt_binding(
                    export_binding,
                    proof,
                    &receipt.collections,
                    max_header_bytes,
                )
                .map_err(|_| {
                    tos_compiler::Error::Invalid("source-navigation managed model receipt binding")
                })?;
                binding_matches(&receipt.binding, &expected)
            }
        };
        if crate::source_creation_store::active(self.deadline, self.cancelled).is_err()
            || !receipt_binding_matches
            || self
                .expected_collections
                .is_some_and(|expected| !collections_match(&receipt.collections, expected))
            || self.cut.current().revision() != self.export_revision
            || self
                .cut
                .stream(self.export_revision)
                .map_err(|_| tos_compiler::Error::Invalid("selected source export unavailable"))?
                .expectation()
                != self.export_membership
            || self
                .cut
                .stream(self.bootstrap_revision)
                .map_err(|_| tos_compiler::Error::Invalid("selected bootstrap anchor unavailable"))?
                .expectation()
                != self.bootstrap_membership
        {
            return Err(changed());
        }
        Ok(())
    }
}

impl StageOwner for CompileStageOwner<'_> {
    fn verify_receipt(&self, receipt: &ExactInputReceipt) -> tos_compiler::Result<()> {
        self.check(receipt)
    }

    fn recheck_sealed_cut(&self, receipt: &ExactInputReceipt) -> tos_compiler::Result<()> {
        self.check(receipt)
    }
}

fn active(deadline: Instant, cancelled: &AtomicBool) -> SourceCommandResult<()> {
    if Instant::now() >= deadline || cancelled.load(Ordering::Acquire) {
        return Err(SourceCommandError::Unsupported(
            "source-navigation compile deadline or cancellation",
        ));
    }
    Ok(())
}

fn canonical_absolute_directory(path: &Path) -> SourceCommandResult<File> {
    if !path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::CurDir | std::path::Component::ParentDir
            )
        })
        || std::fs::canonicalize(path).ok().as_deref() != Some(path)
    {
        return Err(SourceCommandError::Denied(
            "source-navigation compile directory path scope",
        ));
    }
    tos_fd_open::open_absolute_directory(path)
        .map_err(|_| SourceCommandError::Denied("source-navigation compile directory unavailable"))
}

fn read_normalization_input(
    path: &Path,
    input_root: &Path,
    selected_size: u64,
    selected_sha256: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<u8>> {
    active(deadline, cancelled)?;
    let expected_parent = input_root.join("inputs");
    if path.parent() != Some(expected_parent.as_path())
        || path.file_name().and_then(|name| name.to_str()) != Some("knowledge_normalization.rs")
        || path.strip_prefix(input_root).ok().and_then(Path::to_str)
            != Some(NORMALIZATION_INPUT_RELATIVE)
        || selected_size == 0
        || selected_size > 8 * 1024 * 1024
    {
        return Err(SourceCommandError::Denied(
            "selected normalization input path or size differs",
        ));
    }
    let expected = Digest256::from_hex(selected_sha256)
        .map_err(|_| SourceCommandError::Invalid("selected normalization input digest"))?;
    let mut file = tos_fd_open::open_absolute_regular(path, 8 * 1024 * 1024)
        .map_err(|_| SourceCommandError::Conflict("selected normalization input unavailable"))?;
    let metadata = file
        .metadata()
        .map_err(|_| SourceCommandError::Invalid("selected normalization input metadata"))?;
    let uid = rustix::process::geteuid().as_raw();
    if !metadata.is_file()
        || metadata.uid() != uid
        || metadata.mode() & 0o7777 != 0o400
        || metadata.len() != selected_size
    {
        return Err(SourceCommandError::Conflict(
            "selected normalization input custody differs",
        ));
    }
    let capacity = usize::try_from(selected_size)
        .map_err(|_| SourceCommandError::Invalid("selected normalization input size"))?;
    let mut raw = Vec::with_capacity(capacity);
    (&mut file)
        .take(selected_size + 1)
        .read_to_end(&mut raw)
        .map_err(|_| SourceCommandError::Invalid("selected normalization input read"))?;
    active(deadline, cancelled)?;
    let after = file
        .metadata()
        .map_err(|_| SourceCommandError::Invalid("selected normalization input readback"))?;
    if raw.len() as u64 != selected_size
        || Digest256::of_bytes(&raw) != expected
        || after.dev() != metadata.dev()
        || after.ino() != metadata.ino()
        || after.len() != metadata.len()
        || after.mtime() != metadata.mtime()
        || after.mtime_nsec() != metadata.mtime_nsec()
    {
        return Err(SourceCommandError::Conflict(
            "selected normalization input changed during read",
        ));
    }
    Ok(raw)
}

fn read_source_member(
    cut: &CorpusCutReader,
    revision: SourceRevision,
    path: &str,
    cap: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<u8>> {
    let path = RelativePath::parse(path)
        .map_err(|_| SourceCommandError::Invalid("selected source member path"))?;
    cut.read_member(revision, &path, cap, deadline, cancelled)
        .map(|member| member.raw)
        .map_err(|_| SourceCommandError::Conflict("selected source member unavailable"))
}

fn verify_membership_bodies(
    cut: &CorpusCutReader,
    revision: SourceRevision,
    expected: SourceMembershipV1,
    max_members: u64,
    max_total_bytes: u64,
    max_member_bytes: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(u64, u64)> {
    let mut stream = cut
        .stream(revision)
        .map_err(|_| SourceCommandError::Conflict("selected source membership unavailable"))?;
    if stream.expectation() != expected {
        return Err(SourceCommandError::Conflict(
            "selected source membership differs",
        ));
    }
    let mut count = 0u64;
    let mut bytes = 0u64;
    while let Some(member) = stream
        .next_member(deadline, cancelled)
        .map_err(|_| SourceCommandError::Conflict("selected source body verification failed"))?
    {
        count = count
            .checked_add(1)
            .filter(|value| *value <= max_members)
            .ok_or(SourceCommandError::Unsupported(
                "selected source member count cap",
            ))?;
        bytes = bytes
            .checked_add(member.raw.len() as u64)
            .filter(|value| *value <= max_total_bytes)
            .ok_or(SourceCommandError::Unsupported(
                "selected source body byte cap",
            ))?;
        if member.raw.len() as u64 > max_member_bytes {
            return Err(SourceCommandError::Unsupported(
                "selected source member byte cap",
            ));
        }
    }
    if count != expected.count || stream.coverage() != Some(expected) {
        return Err(SourceCommandError::Conflict(
            "selected source stream was incomplete",
        ));
    }
    Ok((count, bytes))
}

fn strict_value(raw: &[u8]) -> SourceCommandResult<serde_json::Value> {
    let parsed = crate::source_command::parse(raw)?;
    serde_json::from_slice(&crate::source_command::canonical(&parsed)?)
        .map_err(|_| SourceCommandError::Invalid("selected source registry encoding"))
}

fn stage_file_identity(parent: &File, leaf: &str) -> SourceCommandResult<(u64, u64)> {
    let file = tos_fd_open::open_regular_at(parent, Path::new(leaf))
        .map_err(|_| SourceCommandError::Conflict("private compiler candidate unavailable"))?;
    let metadata = file
        .metadata()
        .map_err(|_| SourceCommandError::Invalid("private compiler candidate metadata"))?;
    if !metadata.is_file() || metadata.uid() != rustix::process::geteuid().as_raw() {
        return Err(SourceCommandError::Conflict(
            "private compiler candidate ownership differs",
        ));
    }
    Ok((metadata.dev(), metadata.ino()))
}

fn remove_stage_file(
    isolated: &crate::source_creation_store::IsolatedCreationRoot,
    leaf: &str,
    expected_identity: (u64, u64),
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    active(deadline, cancelled)?;
    let parent = isolated.verify_current(deadline, cancelled)?;
    let file = tos_fd_open::open_regular_at(&parent, Path::new(leaf))
        .map_err(|_| SourceCommandError::Conflict("private compiler candidate changed"))?;
    let metadata = file
        .metadata()
        .map_err(|_| SourceCommandError::Invalid("private compiler candidate metadata"))?;
    if (metadata.dev(), metadata.ino()) != expected_identity {
        return Err(SourceCommandError::Conflict(
            "private compiler candidate identity changed",
        ));
    }
    rustix::fs::unlinkat(&parent, leaf, rustix::fs::AtFlags::empty())
        .map_err(|_| SourceCommandError::Conflict("private compiler candidate cleanup failed"))
}

/// Strict, bounded process entry used by the private-stage launcher.
pub(crate) fn run_stdin() -> SourceCommandResult<()> {
    let mut raw = Vec::new();
    std::io::stdin()
        .lock()
        .take(REQUEST_MAX_BYTES as u64 + 1)
        .read_to_end(&mut raw)
        .map_err(|_| SourceCommandError::Invalid("source-navigation compile request read"))?;
    if raw.len() > REQUEST_MAX_BYTES {
        return Err(SourceCommandError::Invalid(
            "source-navigation compile request exceeds byte cap",
        ));
    }
    let document = crate::source_command::parse(&raw)?;
    let canonical = crate::source_command::canonical(&document)?;
    let request: SourceNavigationModelCompileRequestV2 = serde_json::from_slice(&canonical)
        .map_err(|_| SourceCommandError::Invalid("source-navigation compile request schema"))?;
    if request.schema_version != REQUEST_SCHEMA {
        return Err(SourceCommandError::Invalid(
            "source-navigation compile request schema version",
        ));
    }
    let request_sha256 = tos_foundation::Digest256::of_bytes(&raw).to_hex();
    let result = compile(&request, &request_sha256)?;
    let mut response = serde_json::to_vec(&result)
        .map_err(|_| SourceCommandError::Invalid("source-navigation compile response encoding"))?;
    if response.len() >= RESPONSE_MAX_BYTES {
        return Err(SourceCommandError::Invalid(
            "source-navigation compile response exceeds byte cap",
        ));
    }
    response.push(b'\n');
    std::io::stdout()
        .lock()
        .write_all(&response)
        .map_err(|_| SourceCommandError::Invalid("source-navigation compile response write"))
}

/// Compile from the immutable, selected export and return a candidate plus
/// producer-issued receipts. Final-owner publication belongs to the parent.
pub(crate) fn compile(
    request: &SourceNavigationModelCompileRequestV2,
    raw_request_sha256: &str,
) -> SourceCommandResult<SourceNavigationModelCompileResultV1> {
    let phase_trace = CompilePhaseTrace::new();
    if request.schema_version != REQUEST_SCHEMA {
        return Err(SourceCommandError::Invalid(
            "source-navigation compile request schema",
        ));
    }
    let request_digest = Digest256::from_hex(raw_request_sha256)
        .map_err(|_| SourceCommandError::Invalid("source-navigation compile request digest"))?;
    let model = request.model_limits;
    validate_model_limits(model)?;
    if request.operation_budget_ms == 0 || request.operation_budget_ms > CHILD_MAX_TOTAL_MS {
        return Err(SourceCommandError::Invalid(
            "source-navigation operation budget",
        ));
    }
    let operation_ms = request.operation_budget_ms.min(model.max_total_ms);
    let deadline = Instant::now()
        .checked_add(Duration::from_millis(operation_ms))
        .ok_or(SourceCommandError::Invalid(
            "source-navigation operation deadline",
        ))?;
    let cancelled = AtomicBool::new(false);
    active(deadline, &cancelled)?;
    let isolation =
        tos_compiler::private_tmpfs_stage::PrivateTmpfsStageIsolation::select_from_environment(
            model.tmpfs_bytes,
            model.tmpfs_inodes,
            model.working_ram_bytes,
        )
        .map_err(|error| compiler_failure("private-stage-selection", error))?;
    if isolation.resource_limits()
        != (
            model.tmpfs_bytes,
            model.tmpfs_inodes,
            model.working_ram_bytes,
        )
    {
        return Err(SourceCommandError::Denied(
            "selected private model stage limits differ",
        ));
    }
    isolation
        .verify_persistent_store(&request.artifact_staging_root)
        .map_err(|error| compiler_failure("persistent-stage-selection", error))?;
    request
        .cold_limits
        .validate()
        .map_err(|_| SourceCommandError::Invalid("source-navigation selected cold limits"))?;
    request
        .process_limits
        .verify_current()
        .map_err(|_| SourceCommandError::Denied("selected native process limits differ"))?;

    let configuration_digest = Digest256::from_hex(&request.configuration_sha256)
        .map_err(|_| SourceCommandError::Invalid("source-navigation configuration digest"))?;
    let expected_executable = Digest256::from_prefixed(&request.native_executable_sha256)
        .map_err(|_| SourceCommandError::Invalid("source-navigation native executable digest"))?;
    let current_executable = crate::source_serialization::observe_executable(deadline, &cancelled)?
        .current_digest(deadline, &cancelled)?;
    if current_executable != expected_executable {
        return Err(SourceCommandError::Denied(
            "source-navigation child executable differs",
        ));
    }

    let input_cut_phase = phase_trace.phase("input_cut_verification");
    if !request.read_only_input_root.is_absolute()
        || !request.source_cut_root.is_absolute()
        || request.source_cut_root != request.read_only_input_root.join(".managed-source-cut")
        || request.normalization_processor.absolute_path
            != request
                .read_only_input_root
                .join(NORMALIZATION_INPUT_RELATIVE)
    {
        return Err(SourceCommandError::Denied(
            "source-navigation compile input path scope",
        ));
    }
    let input_root = canonical_absolute_directory(&request.read_only_input_root)?;
    let source_root = canonical_absolute_directory(&request.source_cut_root)?;
    let input_metadata = input_root
        .metadata()
        .map_err(|_| SourceCommandError::Invalid("source-navigation input root metadata"))?;
    let source_metadata = source_root
        .metadata()
        .map_err(|_| SourceCommandError::Invalid("source-navigation source root metadata"))?;
    let uid = rustix::process::geteuid().as_raw();
    if !input_metadata.is_dir()
        || input_metadata.uid() != uid
        || input_metadata.mode() & 0o7777 != 0o700
        || !source_metadata.is_dir()
        || source_metadata.uid() != uid
        || source_metadata.mode() & 0o7777 != 0o700
    {
        return Err(SourceCommandError::Denied(
            "source-navigation input root custody differs",
        ));
    }
    let normalization_raw = read_normalization_input(
        &request.normalization_processor.absolute_path,
        &request.read_only_input_root,
        request.normalization_processor.size_bytes,
        &request.normalization_processor.sha256,
        deadline,
        &cancelled,
    )?;

    let export_revision = SourceRevision(
        Digest256::from_hex(&request.export_revision)
            .map_err(|_| SourceCommandError::Invalid("source-navigation export revision"))?,
    );
    let bootstrap_revision = SourceRevision(
        Digest256::from_hex(&request.bootstrap_source_revision)
            .map_err(|_| SourceCommandError::Invalid("source-navigation bootstrap revision"))?,
    );
    let export_membership = SourceMembershipV1 {
        count: request.membership_count,
        digest: Digest256::from_hex(&request.membership_sha256)
            .map_err(|_| SourceCommandError::Invalid("source-navigation export membership"))?,
    };
    let bootstrap_membership = SourceMembershipV1 {
        count: request.bootstrap_membership_count,
        digest: Digest256::from_hex(&request.bootstrap_membership_sha256)
            .map_err(|_| SourceCommandError::Invalid("source-navigation bootstrap membership"))?,
    };
    let proof = &request.source_proof_v2;
    proof
        .validate()
        .map_err(|_| SourceCommandError::Invalid("source-navigation managed source proof"))?;
    if request.membership_count == 0
        || request.bootstrap_membership_count == 0
        || proof.delta.is_some()
        || proof.generation.bootstrap_source_revision != request.bootstrap_source_revision
        || proof.generation.bootstrap_membership_sha256 != request.bootstrap_membership_sha256
        || proof.generation.bootstrap_members != request.bootstrap_membership_count
        || proof.initial_export_source_revision != request.export_revision
        || proof.initial_export_membership_sha256 != request.membership_sha256
        || proof.initial_export_members != request.membership_count
        || request.source_binding.owner_profile != "tos-managed-source-navigation-v2"
        || request.source_binding.source_cut != request.export_revision
        || request.source_binding.through_commit_seq != proof.generation.through_commit_seq
        || request.source_binding.membership_root != request.membership_sha256
        || request.source_binding.index_generation != proof.generation.installed_generation_sha256
        || request.source_binding.route_map_version != "tos-source-navigation-managed-current-v2"
        || request.source_binding.reader_abi
            != tos_foundation::KNOWLEDGE_MODEL_ABI_MANAGED_NAVIGATION_V2
        || request.source_binding.projection_root_sha256
            != proof.generation.inventory_projection_sha256
        || !request.source_binding.complete
    {
        return Err(SourceCommandError::Denied(
            "source-navigation source proof or binding differs",
        ));
    }

    // The immutable corpus and the selected catalogue are different scopes.
    // The plan checks its actual dependency closure against its own envelope.
    let max_catalog_members = usize::try_from(model.max_source_members)
        .map_err(|_| SourceCommandError::Invalid("source-navigation manifest member limit"))?
        .min(SourceCatalogInputLimits::MAX_SELECTED_MEMBERS);
    let read_limits = request.source_read_limits.checked()?;
    let corpus =
        CorpusReader::open_existing(&request.source_cut_root, read_limits).map_err(|_| {
            SourceCommandError::Conflict("selected immutable source export unavailable")
        })?;
    let cut = corpus
        .open_source_cut(
            export_revision,
            request.source_cut_limits.checked()?,
            deadline,
            &cancelled,
        )
        .map_err(|_| {
            SourceCommandError::Conflict("selected immutable source revision unavailable")
        })?;
    if cut.current().revision() != export_revision
        || cut
            .stream(export_revision)
            .map(|stream| stream.expectation())
            .ok()
            != Some(export_membership)
        || cut
            .stream(bootstrap_revision)
            .map(|stream| stream.expectation())
            .ok()
            != Some(bootstrap_membership)
    {
        return Err(SourceCommandError::Conflict(
            "selected source revision anchors differ",
        ));
    }
    let (bootstrap_members_verified, bootstrap_bytes_verified) = verify_membership_bodies(
        &cut,
        bootstrap_revision,
        bootstrap_membership,
        model.max_source_members,
        model.max_source_total_bytes,
        model.max_source_member_bytes,
        deadline,
        &cancelled,
    )?;
    let (export_members_verified, export_bytes_verified) = verify_membership_bodies(
        &cut,
        export_revision,
        export_membership,
        model.max_source_members,
        model.max_source_total_bytes,
        model.max_source_member_bytes,
        deadline,
        &cancelled,
    )?;
    if export_members_verified != export_membership.count
        || export_bytes_verified > model.max_source_total_bytes
        || bootstrap_members_verified > model.max_source_members
        || bootstrap_bytes_verified > model.max_source_total_bytes
    {
        return Err(SourceCommandError::Unsupported(
            "selected source export exceeds bounded envelope",
        ));
    }

    drop(input_cut_phase);

    let worker_path = &request.schema_worker.absolute_path;
    if !worker_path.is_absolute()
        || worker_path.components().any(|component| {
            matches!(
                component,
                std::path::Component::CurDir | std::path::Component::ParentDir
            )
        })
    {
        return Err(SourceCommandError::Invalid(
            "source-navigation schema worker path",
        ));
    }
    let schema_worker_identity = ExactWorkerIdentity {
        absolute_path: worker_path.clone(),
        sha256: Digest256::from_hex(&request.schema_worker.sha256)
            .map_err(|_| SourceCommandError::Invalid("source-navigation schema worker digest"))?,
    };
    let producer = request.producer_limits;
    let event_limits = producer.event_limits;
    let aggregate = producer.aggregate_limits;
    bounded_positive(producer.max_events, 256, "producer event cap")?;
    let _event_dependencies = bounded_positive(
        event_limits.max_dependencies,
        4_096,
        "producer dependency cap",
    )? as usize;
    let _event_rows = bounded_positive(event_limits.max_current_rows, 65_536, "producer row cap")?;
    let _event_row_bytes = bounded_positive(
        event_limits.max_row_bytes,
        8 * 1024 * 1024,
        "producer row byte cap",
    )? as usize;
    let _event_total_bytes = bounded_positive(
        event_limits.max_total_bytes,
        512 * 1024 * 1024,
        "producer total byte cap",
    )?;
    let schema_wall_ms = bounded_positive(
        producer.schema_execution_wall_ms,
        300_000,
        "schema execution cap",
    )?;
    let schema_cleanup_ms = bounded_positive(
        producer.schema_cleanup_grace_ms,
        60_000,
        "schema cleanup cap",
    )?;
    let schema_cpu_seconds = bounded_positive(producer.schema_cpu_seconds, 60, "schema CPU cap")?;
    let schema_address_space = bounded_positive(
        producer.schema_address_space_bytes,
        8 * 1024 * 1024 * 1024,
        "schema address cap",
    )?;
    let _event_schema_max_receipts = usize::try_from(bounded_positive(
        producer.schema_max_receipts,
        100_000,
        "schema receipt count cap",
    )?)
    .map_err(|_| SourceCommandError::Invalid("schema receipt count cap"))?;
    let _event_schema_max_receipt_bytes = usize::try_from(bounded_positive(
        producer.schema_max_receipt_bytes,
        64 * 1024 * 1024,
        "schema receipt byte cap",
    )?)
    .map_err(|_| SourceCommandError::Invalid("schema receipt byte cap"))?;
    let _max_total_rows =
        bounded_positive(aggregate.max_total_rows, u64::MAX - 1, "aggregate row cap")?;
    let _max_total_encoded_bytes = bounded_positive(
        aggregate.max_total_encoded_bytes,
        u64::MAX - 1,
        "aggregate encoded byte cap",
    )?;
    let _max_total_dependency_bytes = bounded_positive(
        aggregate.max_total_dependency_bytes,
        u64::MAX - 1,
        "aggregate dependency byte cap",
    )?;
    let _max_total_work_units = bounded_positive(
        aggregate.max_total_work_units,
        u64::MAX - 1,
        "aggregate work cap",
    )?;

    let mut snapshot = tos_compiler::native_snapshot_manifest::portable_native_snapshot_limits(
        model.max_total_ms.min(CHILD_MAX_TOTAL_MS).div_ceil(1_000),
    )
    .map_err(|_| SourceCommandError::Unsupported("production source-navigation model limits"))?;
    snapshot.stage.sqlite.max_output_bytes = snapshot
        .stage
        .sqlite
        .max_output_bytes
        .min(model.max_model_bytes);
    snapshot.stage.sqlite.max_work_bytes = snapshot
        .stage
        .sqlite
        .max_work_bytes
        .min(model.max_plan_work_bytes);
    snapshot.stage.max_temp_bytes = snapshot.stage.max_temp_bytes.min(model.tmpfs_bytes);
    snapshot.full.scope.max_index_work_bytes = snapshot
        .full
        .scope
        .max_index_work_bytes
        .min(model.max_plan_work_bytes);
    snapshot.full.catalog.max_aggregate_bytes = snapshot
        .full
        .catalog
        .max_aggregate_bytes
        .min(model.max_plan_work_bytes as usize);
    snapshot.full.search.max_work_bytes = snapshot
        .full
        .search
        .max_work_bytes
        .min(model.max_plan_work_bytes);
    snapshot.full.max_registry_bytes = snapshot
        .full
        .max_registry_bytes
        .min(model.max_source_member_bytes as usize);
    snapshot
        .stage
        .validate()
        .map_err(|_| SourceCommandError::Invalid("narrowed source-navigation stage limits"))?;
    let source_file_bytes = usize::try_from(model.max_source_member_bytes.min(16 * 1024 * 1024))
        .map_err(|_| SourceCommandError::Invalid("selected source member byte cap"))?;
    let source_row_bytes = source_file_bytes.min(1024 * 1024).max(1);
    let initial_rows = request
        .cold_limits
        .max_rows
        .min(snapshot.stage.sqlite.max_rows);
    // The selected consumer owns the emitted-row envelope. A provenance row
    // can exceed its source document after reification; the input/parser cap
    // and the old 32 KiB small-export ceiling do not bound that output.
    let max_output_row_bytes = request.cold_limits.max_row_bytes.min(4 * 1024 * 1024);
    let claim_rows = usize::try_from(initial_rows.min(16_384))
        .map_err(|_| SourceCommandError::Invalid("selected bibliographic row cap"))?
        .min((128 * 1024 * 1024) / max_output_row_bytes)
        .max(1);
    let bibliography = BibliographicLimits {
        catalog: tos_compiler::source_witness_catalog::SourceCatalogLimits {
            max_files: model.max_source_members,
            max_rows: initial_rows,
            max_file_bytes: source_file_bytes.max(1),
            max_row_bytes: source_row_bytes,
            max_contract_bytes: usize::try_from(model.max_source_total_bytes.min(16 * 1024 * 1024))
                .map_err(|_| SourceCommandError::Invalid("selected contract byte cap"))?
                .max(1),
            max_output_row_bytes,
        },
        max_claim_cohort_rows: claim_rows,
        max_claim_cohort_bytes: claim_rows * max_output_row_bytes,
        max_output_rows: initial_rows,
        max_output_bytes: model.max_plan_work_bytes.min(128 * 1024 * 1024).max(1),
        deadline,
    };
    bibliography.validate().map_err(|_| {
        SourceCommandError::Invalid("selected source-navigation bibliography limits")
    })?;
    let catalog_input_limits = SourceCatalogInputLimits {
        max_manifest_members: model.max_source_members,
        max_selected_members: max_catalog_members,
        max_plan_bytes: model.max_plan_bytes,
        max_work_bytes: model.max_plan_work_bytes,
    };
    catalog_input_limits
        .validate(bibliography)
        .map_err(|_| SourceCommandError::Invalid("selected source catalog plan limits"))?;

    let plan_start = Instant::now();
    let catalog_plan = {
        let _phase = phase_trace.phase("catalog_planning");
        plan_source_catalog_inputs(
            &cut,
            export_revision,
            export_membership,
            &request.source_binding,
            catalog_input_limits,
            bibliography,
            &cancelled,
        )
        .map_err(|error| compiler_failure("catalog-planning", error))?
    };
    let catalog_input_receipt = catalog_plan.input_receipt();
    let planning_elapsed_ms = elapsed_ms(plan_start);
    let schema_phase = phase_trace.phase("schema_validation");
    let schema_operation_wall = deadline.saturating_duration_since(Instant::now());
    let schema_duration = schema_operation_wall.min(Duration::from_millis(schema_wall_ms));
    if schema_duration.is_zero() {
        return Err(SourceCommandError::Unsupported(
            "source-navigation schema deadline exhausted",
        ));
    }
    let schema_budget = ExecutorBudget {
        execution_wall: schema_duration,
        cleanup_grace: Duration::from_millis(
            schema_cleanup_ms.min(ExecutorBudget::MAX_SCALAR_CLEANUP_GRACE_MILLIS),
        ),
        cpu_seconds: schema_cpu_seconds,
        address_space_bytes: schema_address_space,
    };
    // Initial preparation validates a complete selected catalogue. The
    // producer's per-event receipt/window caps describe later incremental
    // advances and cannot bound this different operation. Keep the initial
    // schema work within the selected cold row and model plan caps. Schema
    // per-unit execution limits remain separate from aggregate initial work.
    let initial_units = initial_rows
        .checked_add(model.max_source_members)
        .ok_or(SourceCommandError::Invalid("initial schema unit overflow"))?;
    let initial_receipts = usize::try_from(initial_units)
        .map_err(|_| SourceCommandError::Invalid("initial schema receipt cap"))?;
    let initial_receipt_bytes = model.max_plan_bytes.min(
        usize::try_from(model.max_plan_work_bytes)
            .map_err(|_| SourceCommandError::Invalid("initial schema receipt byte cap"))?,
    );
    let schema_limits = CutWorkerLimits {
        max_receipts: initial_receipts,
        max_receipt_bytes: initial_receipt_bytes,
    };
    let batch_wall = schema_duration.min(Duration::from_secs(3_600));
    let batch_units = initial_receipts.min(BatchBudget::MAX_UNITS).max(1);
    let batch_raw_bytes = usize::try_from(
        model
            .max_plan_work_bytes
            .min(BatchBudget::MAX_RAW_BYTES as u64),
    )
    .map_err(|_| SourceCommandError::Invalid("source-navigation schema input cap"))?
    .max(1);
    let schema_work = BatchStreamBudget {
        batch: tos_validation::executor::BatchBudget {
            total_execution_wall: batch_wall,
            startup_wall: batch_wall.min(Duration::from_secs(1)),
            per_unit_wall: batch_wall,
            cleanup_grace: Duration::from_millis(
                schema_cleanup_ms.min(ExecutorBudget::MAX_SCALAR_CLEANUP_GRACE_MILLIS),
            ),
            cpu_seconds: schema_cpu_seconds,
            address_space_bytes: schema_address_space,
            max_units: batch_units,
            max_total_raw_bytes: batch_raw_bytes,
        },
        max_chunks: initial_units,
        max_total_units: initial_units,
        max_total_raw_bytes: model.max_plan_work_bytes,
        total_execution_wall: schema_operation_wall,
        operation_cpu_seconds: u64::try_from(schema_operation_wall.as_millis())
            .map_err(|_| SourceCommandError::Invalid("initial schema CPU cap"))?
            .div_ceil(1_000),
        operation_address_space_bytes: schema_address_space,
        max_total_wire_bytes: model.max_plan_work_bytes,
        max_distinct_selectors: usize::try_from(model.max_source_members)
            .map_err(|_| SourceCommandError::Invalid("initial schema selector cap"))?,
    };
    schema_work.validate().map_err(|_| {
        SourceCommandError::Invalid("selected source-navigation schema work budget")
    })?;
    // Whole selected source packets use the existing bounded diagnostics-v2
    // input lane. The scalar probe's one-MiB ceiling describes ordinary rows.
    let diagnostics = CutSchemaDiagnosticsLimits::from_operation_ceilings(
        CutSchemaDiagnosticsLimits {
            max_total_issues: 8192,
            max_total_report_bytes: initial_receipt_bytes,
            max_total_state_bytes: usize::try_from(model.max_plan_work_bytes)
                .map_err(|_| SourceCommandError::Invalid("initial schema state cap"))?,
        },
        initial_receipts,
        schema_work,
    )
    .map_err(|_| SourceCommandError::Invalid("initial schema diagnostics limits"))?;
    let validator = SourceCatalogValidator::from_cut_diagnostics_v2(
        &cut,
        &schema_worker_identity,
        schema_budget,
        schema_limits,
        schema_work,
        diagnostics,
        deadline,
        &cancelled,
    )
    .map_err(|error| compiler_failure("schema-preparation", error))?;
    validator
        .set_diagnostics_v2_legacy_raw_instance_limit(source_file_bytes)
        .map_err(|error| compiler_failure("schema-source-packet-limit", error))?;

    drop(schema_phase);

    let catalog_render_phase = phase_trace.phase("catalog_render");
    let isolated = crate::source_creation_store::IsolatedCreationRoot::create(
        isolation.root(),
        deadline,
        &cancelled,
    )?;
    let isolated_dir = isolated.verify_current(deadline, &cancelled)?;
    let catalog_candidate = isolated.path().join(CATALOG_CANDIDATE_LEAF);
    let catalog_owner = CompileStageOwner {
        cut: &cut,
        export_revision,
        bootstrap_revision,
        export_membership,
        bootstrap_membership,
        binding: CompileStageBinding::ExactExport(&request.source_binding),
        expected_collections: Some(&catalog_input_receipt.collections),
        deadline,
        cancelled: &cancelled,
    };
    let mut catalog_stage = KnowledgeStage::create_until_with_input_cap(
        &catalog_candidate,
        snapshot.stage,
        catalog_input_receipt.clone(),
        &catalog_owner,
        &isolation,
        deadline,
        bibliography.catalog.max_file_bytes,
    )
    .map_err(|error| compiler_failure("catalog-stage", error))?;
    let render_start = Instant::now();
    let mut render_work = SourceCatalogRenderWorkV1::default();
    let catalog_candidate_receipt = render_source_bibliographic_plan_with_work(
        &catalog_plan,
        &cut,
        export_revision,
        export_membership,
        &mut catalog_stage,
        &validator,
        &mut crate::source_forms_compiler::NativeBibliographicForms,
        bibliography,
        catalog_input_limits.max_selected_members,
        catalog_input_limits.max_work_bytes.min(64 * 1024 * 1024) as usize,
        &mut render_work,
    )
    .map_err(|error| compiler_failure("catalog-rendering", error))?;
    let rendering_elapsed_ms = elapsed_ms(render_start);
    drop(catalog_render_phase);

    let navigation_phase = phase_trace.phase("navigation_projection");
    let entities_raw = read_source_member(
        &cut,
        export_revision,
        ENTITY_REGISTRY_PATH,
        model.max_source_member_bytes,
        deadline,
        &cancelled,
    )?;
    let relation_raw = read_source_member(
        &cut,
        export_revision,
        RELATION_REGISTRY_PATH,
        model.max_source_member_bytes,
        deadline,
        &cancelled,
    )?;
    let descriptor_raw = read_source_member(
        &cut,
        export_revision,
        QUERY_VOCABULARY_PATH,
        model.max_source_member_bytes,
        deadline,
        &cancelled,
    )?;
    let entities = strict_value(&entities_raw)?;
    let mut descriptor_value = strict_value(&descriptor_raw)?;
    let descriptor_sources = descriptor_value
        .get_mut("sources")
        .and_then(serde_json::Value::as_array_mut)
        .ok_or(SourceCommandError::Invalid(
            "selected query vocabulary source registrations",
        ))?;
    let before_sources = descriptor_sources.len();
    descriptor_sources.retain(|entry| {
        entry
            .get("source_graph_id")
            .and_then(serde_json::Value::as_str)
            == Some("source-navigation")
    });
    if before_sources == 0
        || descriptor_sources.len() != 1
        || descriptor_sources[0]
            .get("adapter_profile")
            .and_then(serde_json::Value::as_str)
            != Some("source-navigation-node-edge-v1")
    {
        return Err(SourceCommandError::Conflict(
            "selected query vocabulary source-navigation registration differs",
        ));
    }
    let descriptor_reencoded = serde_json::to_vec(&descriptor_value)
        .map_err(|_| SourceCommandError::Invalid("selected query vocabulary encoding"))?;
    let descriptor_source = crate::source_command::parse(&descriptor_reencoded)?;
    let projected_descriptor_raw = crate::source_command::canonical(&descriptor_source)?;
    let vocabulary = tos_compiler::QueryVocabulary::parse(
        &projected_descriptor_raw,
        tos_compiler::NATIVE_KNOWLEDGE_ADAPTER_PROFILES,
    )
    .map_err(|error| compiler_failure("query-vocabulary", error))?;
    if vocabulary.sources.len() != 1
        || vocabulary.sources[0].source_graph_id != "source-navigation"
        || vocabulary.sources[0].adapter_profile != "source-navigation-node-edge-v1"
    {
        return Err(SourceCommandError::Conflict(
            "query vocabulary source isolation failed",
        ));
    }
    let source_cut = BibliographicSourceCut {
        cut: &cut,
        expected_revision: export_revision,
        expected_membership: export_membership,
        stage_source_cut: &request.source_binding.source_cut,
        max_read_files: catalog_input_limits.max_selected_members,
        max_read_bytes: catalog_input_limits.max_work_bytes.min(64 * 1024 * 1024) as usize,
    };
    let navigation = tos_compiler::source_navigation_source::project_source_navigation_from_cut(
        &mut catalog_stage,
        &catalog_candidate_receipt.catalog,
        &source_cut,
        &validator,
        &entities,
        &mut crate::source_forms_compiler::NativeBibliographicForms,
        bibliography,
    )
    .map_err(|error| compiler_failure("navigation-projection", error))?;
    drop(catalog_stage);
    drop(navigation_phase);

    let original_max = tos_compiler::NavigationOriginalLimits::maximum();
    let original_limits = tos_compiler::NavigationOriginalLimits {
        max_rows: request.cold_limits.max_rows.min(original_max.max_rows),
        max_row_bytes: request
            .cold_limits
            .max_row_bytes
            .min(original_max.max_row_bytes)
            .min(usize::try_from(model.max_source_member_bytes).unwrap_or(usize::MAX)),
        max_total_bytes: request
            .cold_limits
            .max_work_bytes
            .min(original_max.max_total_bytes)
            .min(model.max_source_total_bytes),
    };
    original_limits
        .validate()
        .map_err(|_| SourceCommandError::Invalid("selected source-navigation original limits"))?;
    let model_leaf = MODEL_CANDIDATE_LEAF;
    let model_candidate = isolated.path().join(model_leaf);
    let model_owner = CompileStageOwner {
        cut: &cut,
        export_revision,
        bootstrap_revision,
        export_membership,
        bootstrap_membership,
        binding: CompileStageBinding::ManagedModel {
            export_binding: &request.source_binding,
            proof,
            max_header_bytes: snapshot.full.seal.max_header_bytes,
        },
        expected_collections: None,
        deadline,
        cancelled: &cancelled,
    };
    let model_start = Instant::now();
    let managed_model_phase = phase_trace.phase("managed_model_compile");
    let proof_for_header = proof.clone();
    let completed = tos_compiler::managed_source::prepare_managed_agent_selected_model(
        &catalog_plan,
        &navigation,
        &validator,
        proof.clone(),
        &model_candidate,
        request.source_binding.clone(),
        snapshot.stage,
        &model_owner,
        &isolation,
        &entities_raw,
        &relation_raw,
        &projected_descriptor_raw,
        tos_compiler::NATIVE_KNOWLEDGE_ADAPTER_PROFILES,
        snapshot.native,
        original_limits,
        snapshot.full,
        &[],
        format!("sha256:{}", configuration_digest.to_hex()),
        |stage, registry| {
            tos_compiler::managed_navigation_header_with_binding(
                stage,
                registry,
                &entities_raw,
                &proof_for_header,
                Digest256::of_bytes(&normalization_raw),
                configuration_digest,
            )
        },
    )
    .map_err(|error| compiler_failure("managed-model-compilation", error))?;
    let model_elapsed_ms = elapsed_ms(model_start);
    drop(managed_model_phase);

    let artifact_phase = phase_trace.phase("artifact_publication");
    let model_candidate_identity = stage_file_identity(&isolated_dir, model_leaf)?;
    let stage = completed.stage().clone();
    let expectation = completed.expectation().clone();
    if expectation.model_size_bytes == 0
        || expectation.model_size_bytes > model.max_model_bytes
        || request.cold_limits.max_file_bytes < expectation.model_size_bytes
        || request.cold_limits.max_file_bytes > model.max_model_bytes
        || expectation.owner_receipt_id != format!("sha256:{}", configuration_digest.to_hex())
        || stage.sqlite_sha256 != expectation.model_sha256
        || stage.sqlite_size_bytes != expectation.model_size_bytes
    {
        remove_stage_file(
            &isolated,
            model_leaf,
            model_candidate_identity,
            deadline,
            &cancelled,
        )?;
        return Err(SourceCommandError::Conflict(
            "selected cold model envelope does not fit compiled model",
        ));
    }
    isolation
        .verify_persistent_store(&request.artifact_staging_root)
        .map_err(|error| compiler_failure("persistent-stage-recheck", error))?;
    let artifact_fs_verity =
        super::source_navigation_model_artifact::publish_completed_native_model_artifact(
            completed.artifact_path(),
            &request.artifact_staging_root,
            MODEL_CANDIDATE_LEAF,
            completed.stage(),
            model.max_model_bytes,
            deadline,
            &cancelled,
        )
        .map_err(|error| match error {
            super::source_navigation_model_artifact::NativeModelArtifactPublishError::Source(
                error,
            ) => error,
            super::source_navigation_model_artifact::NativeModelArtifactPublishError::Compiler(
                error,
            ) => compiler_failure("staged-artifact-publication", error),
        })?;
    drop(artifact_phase);

    let candidate_finalization_phase = phase_trace.phase("candidate_finalization");
    let navigation_original = completed.navigation_original().clone();
    drop(completed);
    remove_stage_file(
        &isolated,
        model_leaf,
        model_candidate_identity,
        deadline,
        &cancelled,
    )?;
    isolated.cleanup_empty(deadline, &cancelled)?;

    let catalog_work = SourceNavigationCatalogWorkV1 {
        bootstrap_members_verified,
        bootstrap_bytes_verified,
        export_members_verified,
        export_bytes_verified,
        planned_selected_members: u64::try_from(catalog_plan.selected_member_count())
            .map_err(|_| SourceCommandError::Invalid("source-navigation plan member count"))?,
        plan_observed_work_bytes: catalog_plan.observed_work_bytes(),
        rendered_source_members: render_work.source_members_returned,
        rendered_source_payload_bytes: render_work.source_payload_bytes_returned,
        staged_input_rows: render_work.input_rows_staged,
        staged_input_payload_bytes: render_work.input_payload_bytes_staged,
        planning_elapsed_ms,
        rendering_elapsed_ms,
        model_elapsed_ms,
    };
    let result = SourceNavigationModelCompileResultV1 {
        schema_version: RESULT_SCHEMA.into(),
        raw_request_sha256: request_digest.to_hex(),
        candidate_leaf: MODEL_CANDIDATE_LEAF.into(),
        stage,
        expectation,
        navigation_original,
        artifact_fs_verity,
        catalog_work,
    };
    drop(candidate_finalization_phase);
    Ok(result)
}

fn compiler_failure(stage: &'static str, error: tos_compiler::Error) -> SourceCommandError {
    SourceCommandError::DeniedWithReason(format!(
        "source-navigation {stage}: {}",
        crate::source_command::public_compiler_reason(&error),
    ))
}

fn elapsed_ms(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn bounded_positive(value: u64, maximum: u64, message: &'static str) -> SourceCommandResult<u64> {
    if value == 0 || value > maximum {
        return Err(SourceCommandError::Invalid(message));
    }
    Ok(value)
}

fn validate_model_limits(limits: SourceNavigationCompileModelLimitsV1) -> SourceCommandResult<()> {
    if !(1_000..=INITIAL_MAX_TOTAL_MS).contains(&limits.max_total_ms)
        || !(16 * 1024 * 1024..=2 * 1024 * 1024 * 1024).contains(&limits.host_export_bytes)
        || !(32..=1_000_000).contains(&limits.host_export_inodes)
        || !(64 * 1024 * 1024..=8 * 1024 * 1024 * 1024).contains(&limits.tmpfs_bytes)
        || !(128..=1_000_000).contains(&limits.tmpfs_inodes)
        || !(512 * 1024 * 1024..=16 * 1024 * 1024 * 1024).contains(&limits.working_ram_bytes)
        || limits.max_source_members == 0
        || limits.max_source_members > limits.host_export_inodes
        || limits.max_source_total_bytes == 0
        || limits.max_source_total_bytes > limits.host_export_bytes
        || limits.max_source_member_bytes == 0
        || limits.max_source_member_bytes > limits.max_source_total_bytes
        || limits.max_source_member_bytes > limits.working_ram_bytes / 8
        || limits.max_plan_bytes == 0
        || limits.max_plan_bytes > 64 * 1024 * 1024
        || limits.max_plan_work_bytes == 0
        || limits.max_plan_work_bytes > 512 * 1024 * 1024
        || limits.max_model_bytes == 0
        || limits.max_model_bytes > 512 * 1024 * 1024
    {
        return Err(SourceCommandError::Invalid(
            "source-navigation model limits outside supported envelope",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod history_limit_tests {
    use super::*;

    #[test]
    fn bootstrap_contract_initial_export_history_is_not_a_command_batch() {
        let history = SourceNavigationCompileCutLimitsV1 {
            max_revisions: 2,
            max_members: 19_034,
            max_total_bytes: 549_726_458,
            max_member_bytes: 16_734_381,
        };
        assert!(history.checked().is_ok());
        let manifest = SourceNavigationCompileReadLimitsV1 {
            max_manifest_bytes: 10 * 1024 * 1024,
            max_manifest_entries: 40_000,
            max_selected_object_bytes: 16_734_381,
            json_max_bytes: 10 * 1024 * 1024,
            json_max_depth: 64,
            json_max_visits: 3_000_000,
            json_max_integer_digits: 4_300,
        };
        assert!(manifest.checked().is_ok());
        assert!(history.max_members > crate::source_creation_store::MAX_FILES as u64);
        assert!(history.max_total_bytes > crate::source_creation_store::MAX_BYTES as u64);
        assert!(
            SourceNavigationCompileCutLimitsV1 {
                max_total_bytes: crate::source_current_cut::INITIAL_EXPORT_HISTORY_BYTES + 1,
                ..history
            }
            .checked()
            .is_err()
        );
    }

    #[test]
    fn retained_history_limits_are_separate_from_current_source_limits_and_bounded() {
        let retained = SourceNavigationCompileCutLimitsV1 {
            max_revisions: 2,
            max_members: 516,
            max_total_bytes: 5_063_774,
            max_member_bytes: 344_358,
        };
        let cut = retained.checked().expect("bounded retained history");
        let read_limits = SourceNavigationCompileReadLimitsV1 {
            max_manifest_bytes: 1_048_576,
            max_manifest_entries: 516,
            max_selected_object_bytes: 2_531_887,
            json_max_bytes: 1_048_576,
            json_max_depth: 64,
            json_max_visits: 300_000,
            json_max_integer_digits: 4_300,
        }
        .checked()
        .expect("bounded retained manifest limits");
        let current_max_members = 258;
        let current_max_bytes = 2_531_887;
        assert_eq!(cut.max_members, 516);
        assert_eq!(cut.max_total_bytes, 5_063_774);
        assert_eq!(read_limits.max_manifest_entries, 516);
        assert_eq!(read_limits.max_selected_object_bytes, 2_531_887);
        assert!(cut.max_members > current_max_members);
        assert!(cut.max_total_bytes > current_max_bytes);

        let over_history_members = SourceNavigationCompileCutLimitsV1 {
            max_members: crate::source_current_cut::INITIAL_EXPORT_MANIFEST_ENTRIES as u64 + 1,
            ..retained
        };
        assert!(over_history_members.checked().is_err());
        let over_history_bytes = SourceNavigationCompileCutLimitsV1 {
            max_total_bytes: crate::source_current_cut::INITIAL_EXPORT_HISTORY_BYTES + 1,
            ..retained
        };
        assert!(over_history_bytes.checked().is_err());
        let over_history_revisions = SourceNavigationCompileCutLimitsV1 {
            max_revisions: MAX_SOURCE_HISTORY_REVISIONS + 1,
            ..retained
        };
        assert!(over_history_revisions.checked().is_err());
        let over_history_manifest_entries = SourceNavigationCompileReadLimitsV1 {
            max_manifest_entries: crate::source_current_cut::INITIAL_EXPORT_MANIFEST_ENTRIES + 1,
            max_manifest_bytes: 1_048_576,
            max_selected_object_bytes: 2_531_887,
            json_max_bytes: 1_048_576,
            json_max_depth: 64,
            json_max_visits: 300_000,
            json_max_integer_digits: 4_300,
        };
        assert!(over_history_manifest_entries.checked().is_err());
    }
}
