//! Maintained default's exact generated catalog comparison and conditional
//! bibliographic construction, over a genuine captured/authenticated stage.
//! No generated graph artifact is required by this default validator.

use super::foundation_candidate_catalog::{FreshCatalogCandidate, FreshCatalogSink};
use super::foundation_capture::FoundationCapturedCut;
use crate::source_admission_index::FreshIndexRowsWriter;
use crate::source_admission_spooled_candidate::CandidateFence;
use crate::source_creation_store::{DisposableCatalogTreeLimits, IsolatedCreationRoot};
use crate::source_forms_compiler::NativeBibliographicForms;
use serde_json::Value;
use std::cell::RefCell;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tos_compiler::knowledge_stage::{
    CandidateExactInputReceipt, CandidateStageOwner, CandidateValidationBinding,
    ColdAuthoredBinding, ColdExactInputReceipt, ColdStageOwner, KnowledgeStage, StageIsolation,
    StageLimits,
};
use tos_compiler::source_bibliographic::{
    BibliographicLimits, BibliographicSourceCut, prepare_candidate_bibliographic_graph_from_input,
    prepare_cold_bibliographic_graph_from_cut,
};
use tos_compiler::source_witness_catalog::{
    ColdSourceCatalogReceipt, SourceCatalogLimits, SourceCatalogProfileObserver,
    SourceCatalogReceipt, SourceCatalogSink, SourceCatalogValidator,
    render_candidate_source_witness_catalog, render_cold_source_witness_catalog,
};
use tos_compiler::{
    Error, Result, SourceCatalogInputLimits, SourceCatalogRenderWorkV1,
    plan_candidate_source_catalog_inputs_with_workspace,
    plan_cold_source_catalog_inputs_with_workspace, prepare_candidate_source_catalog_plan_observed,
    prepare_cold_source_catalog_plan_observed,
};
use tos_foundation::{
    Digest256, JsonEmissionProfile, JsonLimits, JsonMode, JsonString, JsonValue, emit_json_profile,
    parse_json,
};
use tos_ops_mechanics_plan::route_cards::{RouteResolvedTarget, RouteSources};
use tos_source_store::{MetadataPublicationEpoch, ReadLimits};
use tos_validation::executor::{
    BatchStreamBudget, ExactWorkerIdentity, ExecutorBudget, VerifiedWorkerImageHandle,
};
use tos_validation::source_cut::{
    CutSchemaDiagnostic, CutSchemaDiagnosticsLimits, CutWorkerLimits,
    cut_schema_resource_preparation_state_upper_bound,
};

const MANIFEST: &str = "ToS/source-witnesses/catalog/catalog.manifest.json";
const CATALOG_ROOT: &str = "ToS/source-witnesses/catalog";
pub(crate) type Issue = (String, String);

struct CapturedStageOwner<'a, 'r> {
    capture: &'a FoundationCapturedCut,
    sources: &'a RefCell<&'r mut RouteSources>,
    receipt: ColdExactInputReceipt,
    read_limits: ReadLimits,
    limits: BibliographicLimits,
    cancelled: &'a AtomicBool,
    source_recheck_limit_bytes: usize,
    source_recheck_read_bytes: std::cell::Cell<usize>,
    source_recheck_failed: std::cell::Cell<bool>,
}
impl ColdStageOwner for CapturedStageOwner<'_, '_> {
    fn verify_receipt(&self, supplied: &ColdExactInputReceipt) -> Result<()> {
        let selected = &self.receipt;
        if selected.binding != supplied.binding
            || selected.collections.len() != supplied.collections.len()
            || !selected
                .collections
                .iter()
                .zip(&supplied.collections)
                .all(|(a, b)| {
                    a.source_graph == b.source_graph
                        && a.collection == b.collection
                        && a.input_role == b.input_role
                        && a.adapter_profile == b.adapter_profile
                        && a.expected_count == b.expected_count
                        && a.expected_root_sha256 == b.expected_root_sha256
                })
        {
            return Err(Error::Invalid("foundation stage selected receipt differs"));
        }
        Ok(())
    }
    fn recheck_sealed_cut(&self, supplied: &ColdExactInputReceipt) -> Result<()> {
        if self.source_recheck_failed.get() {
            return Err(Error::Invalid(
                "foundation stage source recheck already refused",
            ));
        }
        self.verify_receipt(supplied)?;
        let prior = self.source_recheck_read_bytes.get();
        let remaining = self
            .source_recheck_limit_bytes
            .checked_sub(prior)
            .ok_or(Error::Budget("foundation stage source recheck allowance"))?;
        // A failed pass can have consumed raw bytes without returning cost.
        // Poison before entering it so recovery cannot restore that allowance.
        self.source_recheck_failed.set(true);
        let read = if self.capture.cost().candidate_copy_read_bytes.is_some() {
            self.capture
                .recheck_candidate_transport_with_control_budget(
                    &mut self.sources.borrow_mut(),
                    remaining,
                    self.limits.deadline,
                    self.cancelled,
                )
        } else {
            self.capture.recheck_with_control_budget(
                &mut self.sources.borrow_mut(),
                self.read_limits,
                remaining,
                self.limits.deadline,
                self.cancelled,
            )
        }
        .map_err(|_| {
            Error::Invalid("foundation stage sealed source changed or read budget refused")
        })?;
        self.source_recheck_read_bytes.set(
            prior
                .checked_add(read)
                .ok_or(Error::Budget("foundation stage source recheck accounting"))?,
        );
        self.source_recheck_failed.set(false);
        Ok(())
    }
}

fn typed(v: &Value, limits: JsonLimits) -> Result<JsonValue> {
    struct Count {
        used: usize,
        cap: usize,
    }
    impl std::io::Write for Count {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.used = self
                .used
                .checked_add(bytes.len())
                .filter(|n| *n <= self.cap)
                .ok_or_else(|| std::io::Error::other("catalog metadata byte cap"))?;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    serde_json::to_writer(
        &mut Count {
            used: 0,
            cap: limits.max_bytes,
        },
        v,
    )
    .map_err(|_| Error::Budget("catalog metadata encoding"))?;
    let raw = serde_json::to_vec(v).map_err(|_| Error::Invalid("catalog metadata encoding"))?;
    if raw.len() > limits.max_bytes {
        return Err(Error::Budget("catalog metadata encoding"));
    }
    parse_json(&raw, JsonMode::PublishedStrict, limits)
        .map(|v| v.into_root())
        .map_err(|_| Error::Invalid("catalog metadata decoding"))
}
fn ordered_object(fields: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        fields
            .into_iter()
            .map(|(k, v)| (JsonString::from_utf8(k), v))
            .collect(),
    )
}

/// The compiler's sealed semantic manifest stays unchanged. This actual owner
/// presentation adds only the selected participating epoch and publishes field
/// order from the maintained render_catalog_rows contract.
pub(crate) fn published_manifest<B>(
    original_epoch_token: Option<&str>,
    receipt: &SourceCatalogReceipt<B>,
    limits: JsonLimits,
) -> Result<Vec<u8>> {
    let original = &receipt.manifest;
    let mut fields = Vec::new();
    for field in [
        "schema_version",
        "owner_repo",
        "source_root",
        "generated_by",
        "record_schema_ref",
        "claim_schema_ref",
    ] {
        fields.push((
            field,
            typed(
                original
                    .get(field)
                    .ok_or(Error::Invalid("catalog manifest field"))?,
                limits,
            )?,
        ));
    }
    if let Some(value) = original.get("extension_schema_refs") {
        fields.push(("extension_schema_refs", typed(value, limits)?));
    } else if original["record_files"].as_object().is_some_and(|files| {
        files.keys().any(|kind| {
            ![
                "agent",
                "place",
                "organization",
                "work",
                "expression",
                "edition",
                "collection",
                "item",
                "link",
            ]
            .contains(&kind.as_str())
        })
    }) {
        // Native profile entries deliberately omit source_schema_ref; the
        // maintained renderer still emits this empty array when such a record
        // family (including artifact/composite) is present.
        fields.push(("extension_schema_refs", JsonValue::Array(Vec::new())));
    }
    for field in ["record_files", "claim_file", "counts", "catalog_sha256"] {
        fields.push((
            field,
            typed(
                original
                    .get(field)
                    .ok_or(Error::Invalid("catalog manifest field"))?,
                limits,
            )?,
        ));
    }
    if let Some(token) = original_epoch_token {
        let refs = original["record_files"]
            .as_object()
            .ok_or(Error::Invalid("catalog record files"))?;
        let mut files = Vec::new();
        for value in refs
            .values()
            .chain(std::iter::once(&original["claim_file"]))
        {
            let path = value
                .as_str()
                .ok_or(Error::Invalid("catalog file locator"))?;
            let sha = receipt
                .file_sha256
                .get(path)
                .ok_or(Error::Invalid("catalog sealed file SHA"))?;
            files.push((path, cmd_string(sha)));
        }
        fields.push((
            "selected_metadata_publication",
            ordered_object(vec![
                ("protocol", cmd_string("tos_selected_source_metadata_v1")),
                ("token", cmd_string(token)),
                ("files", ordered_object(files)),
            ]),
        ));
    }
    fields.push((
        "authority_boundary",
        typed(&original["authority_boundary"], limits)?,
    ));
    emit_json_profile(
        &ordered_object(fields),
        JsonEmissionProfile::SourceWitnessCatalogPublishedV3,
        limits,
    )
    .map(|v| v.bytes)
    .map_err(|_| Error::Budget("catalog published manifest bytes"))
}
fn cmd_string(s: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(s))
}

#[derive(PartialEq, Eq)]
struct GeneratedSelection {
    relative_path: String,
    topology_stamp: Digest256,
    endpoint: Option<(u64, u64, u64, u32, u64, i64, i64, i64, i64, Digest256)>,
}

/// Actual generated inputs stay bound through later default phases. This is
/// physical currentness evidence, separate from the compiler's output seal.
pub(crate) struct GeneratedCatalogObservation {
    selections: Vec<(String, GeneratedSelection)>,
    state_bytes: usize,
    read_bytes: usize,
    max_file_bytes: usize,
    max_total_bytes: usize,
    max_files: usize,
    max_state_bytes: usize,
    deadline: Instant,
    cancelled_identity: usize,
}
impl GeneratedCatalogObservation {
    fn reserve(&mut self, path: &str, selection: &GeneratedSelection) -> Result<()> {
        let state = std::mem::size_of::<(String, GeneratedSelection)>()
            .checked_add(path.len())
            .and_then(|n| n.checked_add(selection.relative_path.len()))
            .and_then(|n| n.checked_add(self.state_bytes))
            .ok_or(Error::Budget("generated catalog observation state"))?;
        if self.selections.len() >= self.max_files || state > self.max_state_bytes {
            return Err(Error::Budget("generated catalog observation limits"));
        }
        self.state_bytes = state;
        Ok(())
    }
    pub(crate) fn read_bytes(&self) -> usize {
        self.read_bytes
    }
    pub(crate) fn retained_state_bytes(&self) -> usize {
        self.state_bytes
    }
    /// Narrow the later physical EOF pass to the caller's current whole-run
    /// allowance. Cumulative reads and retained state only move downward.
    pub(crate) fn restrict_remaining_budget(
        &mut self,
        additional_read_bytes: usize,
        available_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<()> {
        self.checkpoint(deadline, cancelled)?;
        if self.state_bytes > available_state_bytes {
            return Err(Error::Budget("generated catalog final state"));
        }
        let total_read_limit = self
            .read_bytes
            .checked_add(additional_read_bytes)
            .ok_or(Error::Budget("generated catalog final read allowance"))?;
        self.max_total_bytes = self.max_total_bytes.min(total_read_limit);
        self.max_state_bytes = self.max_state_bytes.min(available_state_bytes);
        self.checkpoint(deadline, cancelled)
    }

    /// Narrow only the raw-byte allowance for an EOF verification. The
    /// observation's existing state/workspace cap remains in force, since a
    /// caller cannot recover that private cap from the retained facts.
    pub(crate) fn restrict_remaining_read_budget(
        &mut self,
        additional_read_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<()> {
        self.checkpoint(deadline, cancelled)?;
        let total_read_limit = self
            .read_bytes
            .checked_add(additional_read_bytes)
            .ok_or(Error::Budget("generated catalog final read allowance"))?;
        self.max_total_bytes = self.max_total_bytes.min(total_read_limit);
        self.checkpoint(deadline, cancelled)
    }

    fn checkpoint(&self, deadline: Instant, cancelled: &AtomicBool) -> Result<()> {
        if deadline != self.deadline
            || cancelled as *const AtomicBool as usize != self.cancelled_identity
        {
            return Err(Error::Invalid(
                "generated catalog operation context changed",
            ));
        }
        if cancelled.load(Ordering::Relaxed) {
            return Err(Error::Invalid("generated catalog verification cancelled"));
        }
        if Instant::now() >= deadline {
            return Err(Error::Invalid("generated catalog verification deadline"));
        }
        Ok(())
    }

    /// Re-read participating bytes and absent selections under the SAME held
    /// root and total read cap; caller invokes again at whole evaluation EOF.
    pub(crate) fn recheck(
        &mut self,
        sources: &mut RouteSources,
        deadline: std::time::Instant,
        cancelled: &AtomicBool,
    ) -> Result<()> {
        self.checkpoint(deadline, cancelled)?;
        if deadline > sources.deadline() {
            return Err(Error::Invalid(
                "generated catalog clock exceeds whole invocation",
            ));
        }
        for (path, selected) in &self.selections {
            self.checkpoint(deadline, cancelled)?;
            let (current, _) = observe_generated(
                sources,
                path,
                self.max_file_bytes,
                &mut self.read_bytes,
                self.max_total_bytes,
            )?;
            if &current != selected {
                return Err(Error::Invalid("generated catalog input changed"));
            }
        }
        self.checkpoint(deadline, cancelled)
    }
}

fn observe_generated(
    sources: &mut RouteSources,
    path: &str,
    max_file_bytes: usize,
    read_bytes: &mut usize,
    max_total_bytes: usize,
) -> Result<(GeneratedSelection, Option<Vec<u8>>)> {
    use std::os::unix::fs::MetadataExt;
    let (relative_path, before, topology_stamp) = match sources
        .resolve_selected_target(path, Some(CATALOG_ROOT))
        .map_err(|_| Error::Invalid("catalog selected target custody"))?
    {
        RouteResolvedTarget::Inside {
            relative_path,
            metadata,
            topology_stamp,
        } => (relative_path, metadata, topology_stamp),
        RouteResolvedTarget::OutsideSelectedRoot => {
            return Err(Error::Invalid("catalog target outside selected root"));
        }
    };
    let Some(before) = before else {
        return Ok((
            GeneratedSelection {
                relative_path,
                topology_stamp,
                endpoint: None,
            },
            None,
        ));
    };
    if !before.is_file() {
        return Err(Error::Invalid("catalog selected target is not regular"));
    }
    let (raw, read_metadata) = sources
        .bounded_metadata_bytes(&relative_path, max_file_bytes, read_bytes, max_total_bytes)
        .map_err(|_| Error::Budget("catalog physical read custody/budget"))?;
    let stamp = |m: &std::fs::Metadata| {
        (
            m.dev(),
            m.ino(),
            m.len(),
            m.mode(),
            m.nlink(),
            m.mtime(),
            m.mtime_nsec(),
            m.ctime(),
            m.ctime_nsec(),
        )
    };
    match sources
        .resolve_selected_target(path, Some(CATALOG_ROOT))
        .map_err(|_| Error::Invalid("catalog final target custody"))?
    {
        RouteResolvedTarget::Inside {
            relative_path: current_path,
            metadata: Some(current),
            topology_stamp: current_topology,
        } if current_path == relative_path
            && current_topology == topology_stamp
            && stamp(&before) == stamp(&read_metadata)
            && stamp(&current) == stamp(&read_metadata) =>
        {
            ()
        }
        _ => return Err(Error::Invalid("catalog target changed during comparison")),
    }
    let endpoint = (
        read_metadata.dev(),
        read_metadata.ino(),
        read_metadata.len(),
        read_metadata.mode(),
        read_metadata.nlink(),
        read_metadata.mtime(),
        read_metadata.mtime_nsec(),
        read_metadata.ctime(),
        read_metadata.ctime_nsec(),
        Digest256::of_bytes(&raw),
    );
    Ok((
        GeneratedSelection {
            relative_path,
            topology_stamp,
            endpoint: Some(endpoint),
        },
        Some(raw),
    ))
}

/// Bind the fresh renderer outputs to a held candidate root without consulting
/// the ordinary generated catalog tree. Paths and expected digests come only
/// from the successful renderer receipt; the publication manifest is compared
/// byte-for-byte with the exact bytes passed to the sink.
fn observe_fresh_catalog_outputs<B>(
    sources: &mut RouteSources,
    catalog: &SourceCatalogReceipt<B>,
    exact_manifest: &[u8],
    max_file_bytes: usize,
    max_total_bytes: usize,
    max_files: usize,
    max_state_bytes: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<GeneratedCatalogObservation> {
    if catalog.file_sha256.contains_key(MANIFEST) {
        return Err(Error::Invalid(
            "fresh catalog receipt includes manifest as data",
        ));
    }
    let selected_files = catalog
        .file_sha256
        .len()
        .checked_add(1)
        .filter(|count| *count <= max_files)
        .ok_or(Error::Budget("fresh catalog observation file count"))?;
    let _selection_slots = selected_files
        .checked_mul(std::mem::size_of::<(String, GeneratedSelection)>())
        .filter(|bytes| *bytes <= max_state_bytes)
        .ok_or(Error::Budget("fresh catalog observation state"))?;
    let mut observation = GeneratedCatalogObservation {
        selections: Vec::new(),
        state_bytes: 0,
        read_bytes: 0,
        max_file_bytes,
        max_total_bytes,
        max_files,
        max_state_bytes,
        deadline,
        cancelled_identity: cancelled as *const AtomicBool as usize,
    };
    observation
        .selections
        .try_reserve_exact(selected_files)
        .map_err(|_| Error::Budget("fresh catalog observation state"))?;

    let mut observe_expected =
        |path: &str, expected_digest: Option<&str>, expected_bytes: Option<&[u8]>| -> Result<()> {
            observation.checkpoint(deadline, cancelled)?;
            let (selection, raw) = observe_generated(
                sources,
                path,
                max_file_bytes,
                &mut observation.read_bytes,
                max_total_bytes,
            )?;
            let raw = raw.ok_or(Error::Invalid(
                "fresh catalog output missing after sink finish",
            ))?;
            if let Some(expected) = expected_digest {
                if Digest256::of_bytes(&raw).to_hex() != expected {
                    return Err(Error::Invalid("fresh catalog output digest changed"));
                }
            }
            if expected_bytes.is_some_and(|expected| raw.as_slice() != expected) {
                return Err(Error::Invalid("fresh catalog manifest bytes changed"));
            }
            observation.reserve(path, &selection)?;
            observation.selections.push((path.to_owned(), selection));
            observation.checkpoint(deadline, cancelled)?;
            Ok(())
        };

    observe_expected(MANIFEST, None, Some(exact_manifest))?;
    for (path, digest) in &catalog.file_sha256 {
        observe_expected(path, Some(digest), None)?;
    }
    drop(observe_expected);
    Ok(observation)
}

struct CompareCatalog<'a, 'r> {
    cancelled: &'a AtomicBool,
    sources: &'a RefCell<&'r mut RouteSources>,
    manifest: Vec<u8>,
    current: Option<(String, Option<Vec<u8>>, usize, bool)>,
    issues: Vec<Issue>,
    observation: GeneratedCatalogObservation,
    issue_state_bytes: usize,
}
impl<'a, 'r> CompareCatalog<'a, 'r> {
    fn new(
        sources: &'a RefCell<&'r mut RouteSources>,
        manifest: Vec<u8>,
        max_file_bytes: usize,
        max_total_bytes: usize,
        max_files: usize,
        max_state_bytes: usize,
        deadline: Instant,
        cancelled: &'a AtomicBool,
    ) -> Self {
        Self {
            cancelled,
            sources,
            manifest,
            current: None,
            issues: Vec::new(),
            issue_state_bytes: 0,
            observation: GeneratedCatalogObservation {
                selections: Vec::new(),
                state_bytes: 0,
                read_bytes: 0,
                max_file_bytes,
                max_total_bytes,
                max_files,
                max_state_bytes,
                deadline,
                cancelled_identity: cancelled as *const AtomicBool as usize,
            },
        }
    }
    fn actual(&mut self, path: &str) -> Result<Option<Vec<u8>>> {
        self.observation
            .checkpoint(self.observation.deadline, self.cancelled)?;
        let (selection, raw) = observe_generated(
            &mut self.sources.borrow_mut(),
            path,
            self.observation.max_file_bytes,
            &mut self.observation.read_bytes,
            self.observation.max_total_bytes,
        )?;
        self.observation.reserve(path, &selection)?;
        if self
            .observation
            .state_bytes
            .checked_add(self.issue_state_bytes)
            .is_none_or(|n| n > self.observation.max_state_bytes)
        {
            return Err(Error::Budget("catalog comparison retained state"));
        }
        self.observation
            .selections
            .push((path.to_owned(), selection));
        let Some(mut raw) = raw else {
            return Ok(None);
        };
        // Python read_text universal-newline comparison, in the admitted raw allocation.
        std::str::from_utf8(&raw).map_err(|_| Error::Invalid("catalog physical UTF-8"))?;
        let mut read = 0;
        let mut write = 0;
        while read < raw.len() {
            let byte = raw[read];
            if byte == b'\r' {
                raw[write] = b'\n';
                read += usize::from(raw.get(read + 1) == Some(&b'\n'));
            } else {
                raw[write] = byte;
            }
            read += 1;
            write += 1;
        }
        raw.truncate(write);
        Ok(Some(raw))
    }
    fn issue(&mut self, path: &str, missing: bool) -> Result<()> {
        if self.issues.len() >= self.observation.max_files {
            return Err(Error::Budget("catalog comparison issue count"));
        }
        let additional = std::mem::size_of::<Issue>()
            .checked_add(CATALOG_ROOT.len())
            .and_then(|n| n.checked_add(path.len()))
            .and_then(|n| n.checked_add(64))
            .ok_or(Error::Budget("catalog comparison issue state"))?;
        self.issue_state_bytes = self
            .issue_state_bytes
            .checked_add(additional)
            .ok_or(Error::Budget("catalog comparison issue state"))?;
        if self
            .issue_state_bytes
            .checked_add(self.observation.state_bytes)
            .is_none_or(|n| n > self.observation.max_state_bytes)
        {
            return Err(Error::Budget("catalog comparison retained state"));
        }
        self.issues.push((
            CATALOG_ROOT.to_owned(),
            format!(
                "{path}: generated catalog {}",
                if missing {
                    "file is missing"
                } else {
                    "is stale"
                }
            ),
        ));
        Ok(())
    }
}
impl SourceCatalogSink for CompareCatalog<'_, '_> {
    fn begin_file(&mut self, path: &str) -> Result<()> {
        if self.current.is_some() {
            return Err(Error::Invalid("catalog compare overlapping file"));
        }
        let actual = self.actual(path)?;
        self.current = Some((path.to_owned(), actual, 0, false));
        Ok(())
    }
    fn file_bytes(&mut self, bytes: &[u8]) -> Result<()> {
        let (_, actual, offset, stale) = self
            .current
            .as_mut()
            .ok_or(Error::Invalid("catalog compare unopened file"))?;
        let end = offset
            .checked_add(bytes.len())
            .ok_or(Error::Budget("catalog comparison length"))?;
        if let Some(actual) = actual {
            if actual.get(*offset..end) != Some(bytes) {
                *stale = true;
            }
        }
        *offset = end;
        Ok(())
    }
    fn end_file(&mut self, path: &str, sha: &str) -> Result<()> {
        Digest256::from_hex(sha).map_err(|_| Error::Invalid("catalog sealed output SHA"))?;
        let (opened, actual, offset, stale) = self
            .current
            .take()
            .ok_or(Error::Invalid("catalog compare unopened end"))?;
        if opened != path {
            return Err(Error::Invalid("catalog comparison path changed"));
        }
        match actual {
            None => self.issue(path, true)?,
            Some(v) if stale || offset != v.len() => self.issue(path, false)?,
            _ => (),
        }
        Ok(())
    }
    fn addressed_row(&mut self, _: &str, _: &[u8]) -> Result<()> {
        Ok(())
    }
    fn manifest(&mut self, _: &Value) -> Result<()> {
        match self.actual(MANIFEST)? {
            None => self.issue(MANIFEST, true)?,
            Some(v) if v != self.manifest => self.issue(MANIFEST, false)?,
            _ => (),
        }
        Ok(())
    }
}

/// Exact inline comparison controller; heap/workspace terms are admitted separately.
pub(crate) fn catalogue_comparison_controller_state_bytes() -> usize {
    std::mem::size_of::<CompareCatalog<'_, '_>>()
}

/// Compare completed rendered outputs under the same held generated root.
/// The caller admits the moved manifest and renderer workspace before entry;
/// returned issue state is separate from the retained selection observation.
#[allow(clippy::too_many_arguments)]
pub(crate) fn compare_catalogue_outputs(
    sources: &RefCell<&mut RouteSources>,
    manifest: Vec<u8>,
    max_file_bytes: usize,
    max_total_bytes: usize,
    max_files: usize,
    max_state_bytes: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
    render: impl FnOnce(&mut dyn SourceCatalogSink) -> Result<()>,
) -> Result<(Vec<Issue>, GeneratedCatalogObservation, usize)> {
    let mut sink = CompareCatalog::new(
        sources,
        manifest,
        max_file_bytes,
        max_total_bytes,
        max_files,
        max_state_bytes,
        deadline,
        cancelled,
    );
    sink.observation.checkpoint(deadline, cancelled)?;
    if deadline > sources.borrow().deadline() {
        return Err(Error::Invalid("catalog comparison original clock differs"));
    }
    render(&mut sink)?;
    if sink.current.is_some() {
        return Err(Error::Invalid(
            "catalog comparison unfinished rendered file",
        ));
    }
    if !sink
        .observation
        .selections
        .iter()
        .any(|(path, _)| path == MANIFEST)
    {
        return Err(Error::Invalid(
            "catalog comparison rendered manifest absent",
        ));
    }
    sink.observation
        .recheck(&mut sources.borrow_mut(), deadline, cancelled)?;
    Ok((sink.issues, sink.observation, sink.issue_state_bytes))
}

/// Exact maintained tail schema selections. Entry and ClaimEntry are real
/// derived schemas from the selected catalog contract, not the manifest root.
#[derive(Clone, Copy)]
pub(crate) enum PersistedCatalogSchema {
    Manifest,
    Entry,
    ClaimEntry,
}
impl PersistedCatalogSchema {
    fn selected(
        self,
    ) -> tos_validation::source_foundation_schema::SourceFoundationCatalogSchemaTarget {
        use tos_validation::source_foundation_schema::SourceFoundationCatalogSchemaTarget as Target;
        match self {
            Self::Manifest => Target::Manifest,
            Self::Entry => Target::Entry,
            Self::ClaimEntry => Target::ClaimEntry,
        }
    }
    fn report_contract(self) -> &'static str {
        match self {
            Self::Manifest => "catalog_manifest",
            Self::Entry => "catalog_entry",
            Self::ClaimEntry => "catalog_claim_entry",
        }
    }
}
pub(crate) struct PersistedCatalogSchemaRequest {
    pub before_issue: usize,
    pub location: String,
    pub target: PersistedCatalogSchema,
    pub instance: Value,
}
pub(crate) struct PersistedCatalogReport {
    pub issues: Vec<Issue>,
    pub schema_requests: Vec<PersistedCatalogSchemaRequest>,
    pub generated_inputs: GeneratedCatalogObservation,
    pub retained_state_bytes: usize,
}

pub(crate) struct EvaluatedPersistedCatalog {
    pub issues: Vec<Issue>,
    pub generated_inputs: GeneratedCatalogObservation,
    pub diagnostics: Option<tos_validation::source_foundation_schema::SourceFoundationSchemaReport>,
    /// Borrowed request and placement vectors, separate from the already
    /// admitted parsed inputs and retained diagnostics/output state.
    pub input_workspace_bytes: usize,
    /// Parsed-input peak admitted before evaluation. The decoded instances
    /// are dropped after their actual bound diagnostics have been produced.
    pub parsed_input_state_bytes: usize,
}
pub(crate) enum PersistedCatalogEvaluationError {
    Refused(&'static str),
    IncompleteSchema {
        report: tos_validation::source_foundation_schema::SourceFoundationSchemaReport,
        reason: tos_validation::source_foundation_schema::SourceFoundationSchemaFailure,
    },
}

/// Apply the actual maintained tail's selected schema targets, preserving
/// owner encounter positions. An incomplete worker result never yields issues
/// suitable for a completed CLI verdict. The caller must recheck the returned
/// generated-input ledger again at whole-operation EOF.
pub(crate) fn evaluate_persisted_catalog(
    parsed: PersistedCatalogReport,
    schema_set: &tos_validation::source_foundation_schema::SourceFoundationSchemaSet,
    worker: &ExactWorkerIdentity,
    limits: tos_validation::source_foundation_schema::SourceFoundationSchemaLimits,
    deadline: std::time::Instant,
    cancelled: &AtomicBool,
    max_issues: usize,
    max_output_bytes: usize,
    workspace: usize,
) -> std::result::Result<EvaluatedPersistedCatalog, PersistedCatalogEvaluationError> {
    evaluate_persisted_catalog_inner(
        parsed,
        schema_set,
        worker,
        limits,
        deadline,
        cancelled,
        max_issues,
        max_output_bytes,
        workspace,
        None,
        None,
    )
}

pub(crate) fn evaluate_persisted_catalog_with_shared_quota(
    parsed: PersistedCatalogReport,
    schema_set: &tos_validation::source_foundation_schema::SourceFoundationSchemaSet,
    worker: &ExactWorkerIdentity,
    limits: tos_validation::source_foundation_schema::SourceFoundationSchemaLimits,
    deadline: std::time::Instant,
    cancelled: &AtomicBool,
    max_issues: usize,
    max_output_bytes: usize,
    workspace: usize,
    quota: &tos_validation::executor::SharedSchemaWorkerQuota,
) -> std::result::Result<EvaluatedPersistedCatalog, PersistedCatalogEvaluationError> {
    evaluate_persisted_catalog_inner(
        parsed,
        schema_set,
        worker,
        limits,
        deadline,
        cancelled,
        max_issues,
        max_output_bytes,
        workspace,
        Some(quota),
        None,
    )
}

pub(crate) fn evaluate_persisted_catalog_with_shared_quota_and_image(
    parsed: PersistedCatalogReport,
    schema_set: &tos_validation::source_foundation_schema::SourceFoundationSchemaSet,
    image: &VerifiedWorkerImageHandle,
    limits: tos_validation::source_foundation_schema::SourceFoundationSchemaLimits,
    deadline: std::time::Instant,
    cancelled: &AtomicBool,
    max_issues: usize,
    max_output_bytes: usize,
    workspace: usize,
    quota: &tos_validation::executor::SharedSchemaWorkerQuota,
) -> std::result::Result<EvaluatedPersistedCatalog, PersistedCatalogEvaluationError> {
    evaluate_persisted_catalog_inner(
        parsed,
        schema_set,
        image.identity(),
        limits,
        deadline.min(image.operation_deadline()),
        cancelled,
        max_issues,
        max_output_bytes,
        workspace,
        Some(quota),
        Some(image),
    )
}

pub(crate) fn evaluate_persisted_catalog_inner(
    parsed: PersistedCatalogReport,
    schema_set: &tos_validation::source_foundation_schema::SourceFoundationSchemaSet,
    worker: &ExactWorkerIdentity,
    limits: tos_validation::source_foundation_schema::SourceFoundationSchemaLimits,
    deadline: std::time::Instant,
    cancelled: &AtomicBool,
    max_issues: usize,
    max_output_bytes: usize,
    workspace: usize,
    quota: Option<&tos_validation::executor::SharedSchemaWorkerQuota>,
    image: Option<&VerifiedWorkerImageHandle>,
) -> std::result::Result<EvaluatedPersistedCatalog, PersistedCatalogEvaluationError> {
    use super::foundation_cli::{SchemaPlacement, interleave_schema_findings};
    use tos_validation::source_foundation_schema::{
        SourceFoundationCatalogSchemaInput, SourceFoundationSchemaOutcome,
        evaluate_source_foundation_catalog_schema_checks,
    };
    let fail = PersistedCatalogEvaluationError::Refused;
    if std::time::Instant::now() >= deadline || cancelled.load(std::sync::atomic::Ordering::Relaxed)
    {
        return Err(fail("persisted catalog deadline or cancellation"));
    }
    if parsed.schema_requests.is_empty() {
        let bytes = parsed
            .issues
            .iter()
            .try_fold(0usize, |n, (path, message)| {
                n.checked_add(path.len())
                    .and_then(|n| n.checked_add(message.len()))
            })
            .ok_or_else(|| fail("persisted catalog issue byte overflow"))?;
        if parsed.issues.len() > max_issues || bytes > max_output_bytes {
            return Err(fail("persisted catalog issue limits"));
        }
        return Ok(EvaluatedPersistedCatalog {
            issues: parsed.issues,
            generated_inputs: parsed.generated_inputs,
            diagnostics: None,
            input_workspace_bytes: 0,
            parsed_input_state_bytes: parsed.retained_state_bytes,
        });
    }
    let input_workspace = parsed
        .schema_requests
        .len()
        .checked_mul(
            std::mem::size_of::<SourceFoundationCatalogSchemaInput<'_>>()
                + std::mem::size_of::<SchemaPlacement<'_>>(),
        )
        .filter(|bytes| *bytes <= workspace)
        .ok_or_else(|| fail("persisted catalog schema input workspace"))?;
    let checks: Vec<_> = parsed
        .schema_requests
        .iter()
        .map(|request| SourceFoundationCatalogSchemaInput {
            location: &request.location,
            target: request.target.selected(),
            decoded_instance: &request.instance,
        })
        .collect();
    let placements: Vec<_> = parsed
        .schema_requests
        .iter()
        .enumerate()
        .map(|(index, request)| SchemaPlacement {
            before_issue: request.before_issue,
            check_index: index,
            location: &request.location,
            contract: request.target.report_contract(),
            prefix: "",
        })
        .collect();
    let outcome = if let (Some(quota), Some(image)) = (quota, image) {
        tos_validation::source_foundation_schema::evaluate_source_foundation_catalog_schema_checks_with_shared_quota_and_image(
            schema_set, worker, &checks, limits, deadline, cancelled, quota, image,
        )
    } else if let Some(quota) = quota {
        tos_validation::source_foundation_schema::evaluate_source_foundation_catalog_schema_checks_with_shared_quota(
            schema_set, worker, &checks, limits, deadline, cancelled, quota,
        )
    } else {
        evaluate_source_foundation_catalog_schema_checks(
            schema_set, worker, &checks, limits, deadline, cancelled,
        )
    };
    let report = match outcome {
        SourceFoundationSchemaOutcome::Complete(report) => report,
        SourceFoundationSchemaOutcome::Incomplete { report, reason } => {
            return Err(PersistedCatalogEvaluationError::IncompleteSchema { report, reason });
        }
    };
    let issues = interleave_schema_findings(
        parsed.issues,
        &report,
        &placements,
        max_issues,
        max_output_bytes,
        workspace - input_workspace,
    )
    .map_err(fail)?;
    Ok(EvaluatedPersistedCatalog {
        issues,
        generated_inputs: parsed.generated_inputs,
        diagnostics: Some(report),
        input_workspace_bytes: input_workspace,
        parsed_input_state_bytes: parsed.retained_state_bytes,
    })
}

struct PersistedCatalogBudget {
    state: usize,
    issues: usize,
    checks: usize,
    max_state: usize,
    max_issues: usize,
    max_checks: usize,
}
impl PersistedCatalogBudget {
    fn reserve(&mut self, amount: usize) -> Result<()> {
        self.state = self
            .state
            .checked_add(amount)
            .filter(|n| *n <= self.max_state)
            .ok_or(Error::Budget("persisted catalog state"))?;
        Ok(())
    }
    fn issue(
        &mut self,
        output: &mut Vec<Issue>,
        location: &str,
        message: &'static str,
    ) -> Result<()> {
        if self.issues >= self.max_issues {
            return Err(Error::Budget("persisted catalog issues"));
        }
        self.reserve(std::mem::size_of::<Issue>() + location.len() + message.len())?;
        self.issues += 1;
        output.push((location.to_owned(), message.to_owned()));
        Ok(())
    }
    fn parse(&mut self, raw: &[u8]) -> Result<Option<Value>> {
        // Conservative decoded-container workspace is admitted BEFORE parser
        // allocation. Logical retained state includes this upper estimate;
        // caller must separately admit raw read and diagnostic encoder space.
        let workspace = raw
            .len()
            .checked_mul(128)
            .ok_or(Error::Budget("persisted catalog decoded state"))?;
        self.reserve(workspace)?;
        match serde_json::from_slice(raw) {
            Ok(value) => Ok(Some(value)),
            Err(_) => {
                let limits =
                    JsonLimits::new(raw.len().max(1), 128, raw.len().max(1), raw.len().max(1))
                        .map_err(|_| Error::Budget("persisted catalog parser classification"))?;
                match tos_foundation::parse_json_with_state_budget(
                    raw,
                    JsonMode::RequestLastWins,
                    limits,
                    workspace.max(1),
                ) {
                    Err(error) => match error.code {
                        tos_foundation::FoundationErrorCode::NonfiniteFloat => {
                            Err(Error::PreparedUnsupported(
                                "persisted catalog legacy nonfinite JSON representation",
                            ))
                        }
                        tos_foundation::FoundationErrorCode::BudgetExceeded => {
                            Err(Error::Budget("persisted catalog parser classification"))
                        }
                        tos_foundation::FoundationErrorCode::InvalidUtf8
                        | tos_foundation::FoundationErrorCode::InvalidJson
                        | tos_foundation::FoundationErrorCode::InvalidUnicodeScalar
                        | tos_foundation::FoundationErrorCode::InvalidNumber => Ok(None),
                        _ => Err(Error::PreparedUnsupported(
                            "persisted catalog parser category",
                        )),
                    },
                    Ok(_) => Err(Error::PreparedUnsupported(
                        "persisted catalog JSON decoder disagreement",
                    )),
                }
            }
        }
    }
    fn schema(
        &mut self,
        output: &mut Vec<PersistedCatalogSchemaRequest>,
        before_issue: usize,
        location: &str,
        target: PersistedCatalogSchema,
        instance: Value,
    ) -> Result<()> {
        if self.checks >= self.max_checks {
            return Err(Error::Budget("persisted catalog schema count"));
        }
        self.reserve(std::mem::size_of::<PersistedCatalogSchemaRequest>() + location.len())?;
        self.checks += 1;
        output.push(PersistedCatalogSchemaRequest {
            before_issue,
            location: location.to_owned(),
            target,
            instance,
        });
        Ok(())
    }
}

/// Inspect the CURRENT persisted catalog after producer comparison. Extension
/// filenames are ordered authentic profile selections supplied by CMD; values
/// from an untrusted current manifest never select arbitrary physical paths.
/// `entry_schema_present`/`claim_schema_present` are observations of the actual
/// selected contract's object-valued defs, preserving the maintained condition.
pub(crate) fn inspect_persisted_catalog(
    sources: &mut RouteSources,
    extension_files: &[(String, String)],
    entry_schema_present: bool,
    claim_schema_present: bool,
    max_file_bytes: usize,
    max_total_read_bytes: usize,
    max_files: usize,
    max_state_bytes: usize,
    max_issues: usize,
    max_checks: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<PersistedCatalogReport> {
    const BASE_FILES: &[&str] = &[
        "agents.jsonl",
        "places.jsonl",
        "organizations.jsonl",
        "works.jsonl",
        "expressions.jsonl",
        "editions.jsonl",
        "collections.jsonl",
        "items.jsonl",
        "links.jsonl",
    ];
    let mut report = PersistedCatalogReport {
        issues: Vec::new(),
        schema_requests: Vec::new(),
        generated_inputs: GeneratedCatalogObservation {
            selections: Vec::new(),
            state_bytes: 0,
            read_bytes: 0,
            max_file_bytes,
            max_total_bytes: max_total_read_bytes,
            max_files,
            max_state_bytes,
            deadline,
            cancelled_identity: cancelled as *const AtomicBool as usize,
        },
        retained_state_bytes: 0,
    };
    let mut budget = PersistedCatalogBudget {
        state: 0,
        issues: 0,
        checks: 0,
        max_state: max_state_bytes,
        max_issues,
        max_checks,
    };
    let mut read = |path: &str,
                    observation: &mut GeneratedCatalogObservation,
                    budget: &mut PersistedCatalogBudget|
     -> Result<Option<Vec<u8>>> {
        let (selected, raw) = observe_generated(
            sources,
            path,
            max_file_bytes,
            &mut observation.read_bytes,
            max_total_read_bytes,
        )?;
        let prior_state = observation.state_bytes;
        observation.reserve(path, &selected)?;
        budget.reserve(observation.state_bytes - prior_state)?;
        observation.selections.push((path.to_owned(), selected));
        Ok(raw)
    };
    let mut manifest = None;
    match read(MANIFEST, &mut report.generated_inputs, &mut budget)? {
        None => budget.issue(&mut report.issues, MANIFEST, "file is missing")?,
        Some(raw) => match budget.parse(&raw)? {
            None => budget.issue(
                &mut report.issues,
                MANIFEST,
                "cannot read JSON: invalid JSON",
            )?,
            Some(value) if !value.is_object() => {
                budget.issue(&mut report.issues, MANIFEST, "JSON root must be an object")?
            }
            Some(value) => manifest = Some(value),
        },
    }
    let mut filenames = Vec::new();
    if entry_schema_present {
        budget.reserve(
            (BASE_FILES.len() + extension_files.len())
                .checked_mul(std::mem::size_of::<String>())
                .ok_or(Error::Budget("persisted catalog file list"))?,
        )?;
        for name in BASE_FILES {
            budget.reserve(name.len())?;
            filenames.push((*name).to_owned());
        }
        for (kind, filename) in extension_files {
            if filename.len() > 4096
                || filename.contains('/')
                || filename.contains('\\')
                || filename == "."
                || filename == ".."
                || !filename.ends_with(".jsonl")
            {
                return Err(Error::Invalid("selected catalog profile filename"));
            }
            if manifest
                .as_ref()
                .and_then(|m| m.get("record_files"))
                .and_then(Value::as_object)
                .is_some_and(|m| m.contains_key(kind))
            {
                budget.reserve(filename.len())?;
                filenames.push(filename.to_owned());
            }
        }
    }
    if let Some(value) = manifest {
        budget.schema(
            &mut report.schema_requests,
            report.issues.len(),
            MANIFEST,
            PersistedCatalogSchema::Manifest,
            value,
        )?;
    }
    if claim_schema_present {
        budget.reserve(std::mem::size_of::<String>() + "claims.jsonl".len())?;
        filenames.push("claims.jsonl".to_owned());
    }
    for filename in filenames {
        budget.reserve(CATALOG_ROOT.len() + 1 + filename.len())?;
        let path = format!("{CATALOG_ROOT}/{filename}");
        let target = if filename == "claims.jsonl" {
            PersistedCatalogSchema::ClaimEntry
        } else {
            PersistedCatalogSchema::Entry
        };
        let Some(raw) = read(&path, &mut report.generated_inputs, &mut budget)? else {
            budget.issue(&mut report.issues, &path, "file is missing")?;
            continue;
        };
        let Ok(text) = std::str::from_utf8(&raw) else {
            budget.issue(
                &mut report.issues,
                &path,
                "cannot read JSONL: invalid UTF-8",
            )?;
            continue;
        };
        // Python splitlines includes its Unicode separators. The path used by
        // schema checks numbers ACCEPTED objects; load errors number physical
        // lines, exactly as the maintained load-then-enumerate tail does.
        budget.reserve(
            text.len()
                .checked_mul(std::mem::size_of::<&str>())
                .ok_or(Error::Budget("persisted catalog line workspace"))?,
        )?;
        let first_file_schema = report.schema_requests.len();
        let mut accepted = 0usize;
        for (line_index, line) in tos_ops_mechanics_plan::route_cards::splitlines(text)
            .into_iter()
            .enumerate()
        {
            let location_len = path
                .len()
                .checked_add(1 + 20)
                .ok_or(Error::Budget("persisted catalog location"))?;
            budget.reserve(location_len)?;
            let location = format!("{path}:{}", line_index + 1);
            if line
                .chars()
                .all(tos_ops_mechanics_plan::route_cards::python_space)
            {
                budget.issue(
                    &mut report.issues,
                    &location,
                    "blank JSONL line is not allowed",
                )?;
                continue;
            }
            match budget.parse(line.as_bytes())? {
                None => {
                    budget.issue(&mut report.issues, &location, "invalid JSON: invalid JSON")?
                }
                Some(value) if !value.is_object() => budget.issue(
                    &mut report.issues,
                    &location,
                    "JSONL record must be an object",
                )?,
                Some(value) => {
                    accepted += 1;
                    budget.reserve(location_len)?;
                    let schema_location = format!("{path}:{accepted}");
                    budget.schema(
                        &mut report.schema_requests,
                        report.issues.len(),
                        &schema_location,
                        target,
                        value,
                    )?;
                }
            }
        }
        // _load_jsonl emits every parse/root issue for this file before its
        // accepted rows are subsequently schema-validated.
        let after_load = report.issues.len();
        for request in &mut report.schema_requests[first_file_schema..] {
            request.before_issue = after_load;
        }
    }
    report.retained_state_bytes = budget.state;
    Ok(report)
}

/// Validated profile selection from the same compiler contract traversal.
/// This owns no freshness or admission claim; incomplete selection cannot drive
/// the maintained persisted-manifest tail.
pub(crate) struct FoundationCatalogProfiles {
    files: Vec<(String, String)>,
    pub(crate) native_semantic: std::collections::BTreeMap<String, Vec<String>>,
    complete: bool,
    pub retained_state_bytes: usize,
    max_state_bytes: usize,
    max_profiles: usize,
}
impl FoundationCatalogProfiles {
    pub(crate) fn files(&self) -> Result<&[(String, String)]> {
        if !self.complete {
            return Err(Error::Invalid(
                "record catalog profile selection incomplete",
            ));
        }
        Ok(&self.files)
    }
    fn reserve(&mut self, bytes: usize) -> Result<()> {
        self.retained_state_bytes = self
            .retained_state_bytes
            .checked_add(bytes)
            .filter(|total| *total <= self.max_state_bytes)
            .ok_or(Error::Budget("record catalog profile state"))?;
        Ok(())
    }
}
impl SourceCatalogProfileObserver for FoundationCatalogProfiles {
    fn record_profile(&mut self, kind: &str, filename: &str) -> Result<()> {
        if self.complete || self.files.len() >= self.max_profiles {
            return Err(Error::Budget("record catalog profile count"));
        }
        self.reserve(
            std::mem::size_of::<(String, String)>()
                .checked_add(kind.len())
                .and_then(|n| n.checked_add(filename.len()))
                .ok_or(Error::Budget("record catalog profile state"))?,
        )?;
        self.files
            .try_reserve_exact(1)
            .map_err(|_| Error::Budget("record catalog profile state"))?;
        self.files.push((kind.to_owned(), filename.to_owned()));
        Ok(())
    }
    fn native_semantic_identity(&mut self, id: &str, packet_ref: &str) -> Result<()> {
        // Preserve every validated occurrence/version. SQL reservation dedup
        // cannot decide the maintained source-index anchor.
        let new_key = !self.native_semantic.contains_key(id);
        let mut cost = std::mem::size_of::<String>()
            .checked_add(packet_ref.len())
            .ok_or(Error::Budget("native semantic reference state"))?;
        if new_key {
            cost = cost
                .checked_add(std::mem::size_of::<(String, Vec<String>)>())
                .and_then(|n| n.checked_add(id.len()))
                .and_then(|n| n.checked_add(3 * std::mem::size_of::<usize>()))
                .ok_or(Error::Budget("native semantic identity state"))?;
        }
        self.reserve(cost)?;
        if new_key {
            self.native_semantic.insert(id.to_owned(), Vec::new());
        }
        let refs = self.native_semantic.get_mut(id).ok_or(Error::Invalid(
            "native semantic identity observation absent",
        ))?;
        refs.try_reserve_exact(1)
            .map_err(|_| Error::Budget("native semantic reference state"))?;
        refs.push(packet_ref.to_owned());
        Ok(())
    }
    fn completed_record_profiles(&mut self) -> Result<()> {
        if self.complete {
            return Err(Error::Invalid("record catalog profiles already complete"));
        }
        // Maintained dictionary merge gives these adapters priority, even when
        // a valid profile supplied the same kind.
        for (kind, filename) in [
            ("artifact", "artifacts.jsonl"),
            ("composite", "composites.jsonl"),
        ] {
            if let Some(index) = self.files.iter().position(|(selected, _)| selected == kind) {
                self.reserve(filename.len())?;
                self.files[index].1 = filename.to_owned();
            } else {
                self.record_profile(kind, filename)?;
            }
        }
        self.complete = true;
        Ok(())
    }
}

/// Candidate mode sends native semantic row transport directly to the same
/// caller-reserved provider used by the fresh catalog sink. The maintained
/// profile selector and this adapter still decide which rows are emitted; the
/// destination is storage only and grants no source admission.
struct SpoolingCatalogProfileObserver<'profiles, 'rows> {
    profiles: &'profiles mut FoundationCatalogProfiles,
    rows: &'rows mut dyn FreshIndexRowsWriter,
}

impl SourceCatalogProfileObserver for SpoolingCatalogProfileObserver<'_, '_> {
    fn record_profile(&mut self, kind: &str, filename: &str) -> Result<()> {
        <FoundationCatalogProfiles as SourceCatalogProfileObserver>::record_profile(
            self.profiles,
            kind,
            filename,
        )
    }

    fn native_semantic_identity(&mut self, id: &str, packet_ref: &str) -> Result<()> {
        self.rows
            .push_native_semantic(id, packet_ref)
            .map_err(|_| Error::Invalid("native semantic row spool refused"))
    }

    fn completed_record_profiles(&mut self) -> Result<()> {
        <FoundationCatalogProfiles as SourceCatalogProfileObserver>::completed_record_profiles(
            self.profiles,
        )
    }
}

pub(crate) struct FoundationCatalogResult<B = ColdAuthoredBinding> {
    pub(crate) observed_plan_work_bytes: u64,
    pub(crate) profiles: FoundationCatalogProfiles,
    pub(crate) issues: Vec<Issue>,
    pub(crate) catalog: SourceCatalogReceipt<B>,
    pub(crate) bibliographic_constructed: bool,
    /// Actual selected generated-file reads, including the manifest. These
    /// are separate from authored capture/transfer and must enter whole cost.
    pub(crate) generated_read_bytes: usize,
    pub(crate) generated_inputs: GeneratedCatalogObservation,
    pub(crate) source_recheck_read_bytes: usize,
    pub(crate) schema_execution_cost:
        tos_validation::source_cut::CutSchemaDiagnosticsCumulativeCost,
}

pub(crate) enum FoundationCatalogOutcome<B = ColdAuthoredBinding, D = CutSchemaDiagnostic> {
    Complete(FoundationCatalogResult<B>),
    /// Complete authenticated schema rejection; the failed compiler phase
    /// produced no receipt. Independent default owner checks still continue.
    SchemaRejected {
        observed_plan_work_bytes: u64,
        profiles: FoundationCatalogProfiles,
        diagnostic: D,
        bibliographic_phase: bool,
        catalog_issues: Vec<Issue>,
        generated_read_bytes: usize,
        generated_inputs: Option<GeneratedCatalogObservation>,
        source_recheck_read_bytes: usize,
        schema_execution_cost: tos_validation::source_cut::CutSchemaDiagnosticsCumulativeCost,
    },
}

struct CatalogCandidateArtifact<'a> {
    candidate: FreshCatalogCandidate<'a>,
    fresh_sources: RouteSources,
}

struct CatalogKernelOutput<'a, B = ColdAuthoredBinding, D = CutSchemaDiagnostic> {
    outcome: FoundationCatalogOutcome<B, D>,
    candidate: Option<CatalogCandidateArtifact<'a>>,
}

/// Candidate evidence is retained through the host callback. The ordinary
/// outcome continues to describe the exact maintained comparison; the fresh
/// route reader is a distinct root sharing only the primary operation count.
pub(crate) struct FoundationCatalogCandidateOutcome<
    'a,
    B = ColdAuthoredBinding,
    D = CutSchemaDiagnostic,
> {
    pub(crate) outcome: FoundationCatalogOutcome<B, D>,
    pub(crate) candidate: Option<FreshCatalogCandidate<'a>>,
    pub(crate) fresh_sources: Option<RouteSources>,
}

/// All resource profiles and the real isolation guard are caller selected.
/// Conditional graph construction has no generated-file parity requirement:
/// maintained default15708–15720 invokes build_payload only.
fn compare_kernel<'candidate>(
    capture: &FoundationCapturedCut,
    sources: &mut RouteSources,
    candidate: &Path,
    isolation: &dyn StageIsolation,
    stage_limits: StageLimits,
    source_limits: SourceCatalogInputLimits,
    read_limits: ReadLimits,
    limits: BibliographicLimits,
    image: &VerifiedWorkerImageHandle,
    executor: ExecutorBudget,
    worker_limits: CutWorkerLimits,
    operation: BatchStreamBudget,
    diagnostics_limits: CutSchemaDiagnosticsLimits,
    build_bibliographic: bool,
    max_version_files: usize,
    max_version_bytes: usize,
    max_generated_bytes: usize,
    max_generated_files: usize,
    max_generated_state_bytes: usize,
    max_source_recheck_read_bytes: usize,
    quota: &tos_validation::executor::SharedSchemaWorkerQuota,
    candidate_root: Option<(
        &'candidate IsolatedCreationRoot,
        DisposableCatalogTreeLimits,
    )>,
    index_rows: Option<&mut dyn FreshIndexRowsWriter>,
    cancelled: &AtomicBool,
    observe: impl FnOnce(
        &mut KnowledgeStage<'_>,
        &ColdSourceCatalogReceipt,
        SourceCatalogLimits,
    ) -> Result<()>,
) -> Result<CatalogKernelOutput<'candidate>> {
    let binding = ColdAuthoredBinding::from_cut(
        capture.cut(),
        capture.revision(),
        capture.membership(),
        capture.epoch(),
    )?;
    let plan = plan_cold_source_catalog_inputs_with_workspace(
        capture.cut(),
        capture.revision(),
        capture.membership(),
        &binding,
        source_limits,
        limits,
        cancelled,
        max_generated_state_bytes,
    )?;
    let observed_plan_work_bytes = plan.observed_work_bytes();
    // The completed plan remains live while the schema closure is prepared.
    // Its admitted locator ceiling is independent of the parser workspace;
    // reserve both before opening a stage or constructing parsed schema DOMs.
    let preparation_state = cut_schema_resource_preparation_state_upper_bound(
        capture.cut(),
        limits.deadline,
        cancelled,
    )
    .map_err(|_| Error::Budget("catalog schema preparation state"))?;
    let simultaneous_preparation = preparation_state
        .checked_add(source_limits.max_plan_bytes)
        // The owner retains its cloned five collection receipts and fixed binding.
        .and_then(|bytes| bytes.checked_add(5 * 1024))
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<CapturedStageOwner<'_, '_>>()))
        .ok_or(Error::Budget("catalog schema preparation state"))?;
    if simultaneous_preparation > max_generated_state_bytes {
        return Err(Error::Budget("catalog schema preparation state"));
    }
    let sources = RefCell::new(sources);
    let owner = CapturedStageOwner {
        capture,
        sources: &sources,
        receipt: plan.input_receipt(),
        read_limits,
        limits,
        cancelled,
        source_recheck_limit_bytes: max_source_recheck_read_bytes,
        source_recheck_read_bytes: std::cell::Cell::new(0),
        source_recheck_failed: std::cell::Cell::new(false),
    };
    let stage = KnowledgeStage::create_cold_until_with_input_cap(
        candidate,
        stage_limits,
        plan.input_receipt(),
        &owner,
        isolation,
        limits.deadline,
        limits.catalog.max_file_bytes,
    )?;
    let validator = SourceCatalogValidator::from_cut_with_image_diagnostics_v2(
        capture.cut(),
        image,
        executor,
        worker_limits,
        operation,
        diagnostics_limits,
        limits.deadline,
        cancelled,
    )?;
    validator.set_diagnostics_v2_legacy_raw_instance_limit(
        usize::try_from(read_limits.max_selected_object_bytes)
            .map_err(|_| Error::Budget("catalog selected instance byte range"))?
            .min(tos_validation::executor::BatchBudget::MAX_RAW_BYTES),
    )?;
    validator.set_shared_schema_worker_quota(quota.clone())?;
    compare_prepared_kernel(
        stage,
        &validator,
        &sources,
        limits,
        observed_plan_work_bytes,
        capture.epoch().token(),
        build_bibliographic,
        max_generated_bytes,
        max_generated_files,
        max_generated_state_bytes,
        candidate_root,
        index_rows,
        cancelled,
        |stage, validator, profiles, rows| {
            if let Some(rows) = rows {
                let mut observer = SpoolingCatalogProfileObserver { profiles, rows };
                prepare_cold_source_catalog_plan_observed(
                    &plan,
                    capture.cut(),
                    capture.revision(),
                    capture.membership(),
                    stage,
                    validator,
                    limits,
                    &mut observer,
                )
            } else {
                prepare_cold_source_catalog_plan_observed(
                    &plan,
                    capture.cut(),
                    capture.revision(),
                    capture.membership(),
                    stage,
                    validator,
                    limits,
                    profiles,
                )
            }
        },
        |stage, catalog, sink| {
            render_cold_source_witness_catalog(stage, catalog, limits.catalog, sink)
        },
        |stage, catalog, validator| {
            let source = BibliographicSourceCut {
                cut: capture.cut(),
                expected_revision: capture.revision(),
                expected_membership: capture.membership(),
                stage_source_cut: binding.source_cut(),
                max_read_files: max_version_files,
                max_read_bytes: max_version_bytes,
            };
            prepare_cold_bibliographic_graph_from_cut(
                stage,
                catalog,
                validator,
                &mut NativeBibliographicForms,
                limits,
                &source,
            )
            .map(|_| ())
        },
        observe,
        || owner.recheck_sealed_cut(&owner.receipt),
        || owner.source_recheck_read_bytes.get(),
        |stage| stage.finish_cold().map(|_| ()),
        || validator.take_schema_diagnostic_rejection(),
    )
}

/// Object-safe transport into the existing typed renderer; this adds no
/// selection, source identity or publication authority.
struct CatalogSinkRef<'a>(&'a mut dyn SourceCatalogSink);
impl SourceCatalogSink for CatalogSinkRef<'_> {
    fn begin_file(&mut self, path: &str) -> Result<()> {
        self.0.begin_file(path)
    }
    fn file_bytes(&mut self, raw: &[u8]) -> Result<()> {
        self.0.file_bytes(raw)
    }
    fn end_file(&mut self, path: &str, digest: &str) -> Result<()> {
        self.0.end_file(path, digest)
    }
    fn addressed_row(&mut self, collection: &str, raw: &[u8]) -> Result<()> {
        self.0.addressed_row(collection, raw)
    }
    fn manifest(&mut self, manifest: &Value) -> Result<()> {
        self.0.manifest(manifest)
    }
}

/// One maintained render/default/diagnostic kernel for authenticated cold and
/// actual candidate inputs. Callbacks retain their exact typed owner receipts.
#[allow(clippy::too_many_arguments)]
fn compare_prepared_kernel<'candidate, 'stage, B, D>(
    mut stage: KnowledgeStage<'stage>,
    validator: &SourceCatalogValidator<'_>,
    sources: &RefCell<&mut RouteSources>,
    limits: BibliographicLimits,
    observed_plan_work_bytes: u64,
    original_epoch_token: Option<&str>,
    build_bibliographic: bool,
    max_generated_bytes: usize,
    max_generated_files: usize,
    max_generated_state_bytes: usize,
    candidate_root: Option<(
        &'candidate IsolatedCreationRoot,
        DisposableCatalogTreeLimits,
    )>,
    mut index_rows: Option<&mut dyn FreshIndexRowsWriter>,
    cancelled: &AtomicBool,
    mut prepare: impl FnMut(
        &mut KnowledgeStage<'stage>,
        &SourceCatalogValidator<'_>,
        &mut FoundationCatalogProfiles,
        Option<&mut dyn FreshIndexRowsWriter>,
    ) -> Result<SourceCatalogReceipt<B>>,
    mut render: impl FnMut(
        &mut KnowledgeStage<'stage>,
        &SourceCatalogReceipt<B>,
        &mut CatalogSinkRef<'_>,
    ) -> Result<()>,
    mut bibliographic: impl FnMut(
        &mut KnowledgeStage<'stage>,
        &SourceCatalogReceipt<B>,
        &SourceCatalogValidator<'_>,
    ) -> Result<()>,
    observe: impl FnOnce(
        &mut KnowledgeStage<'stage>,
        &SourceCatalogReceipt<B>,
        SourceCatalogLimits,
    ) -> Result<()>,
    mut recheck: impl FnMut() -> Result<()>,
    source_recheck_read_bytes: impl Fn() -> usize,
    finish_stage: impl FnOnce(KnowledgeStage<'stage>) -> Result<()>,
    take_rejection: impl Fn() -> Result<Option<D>>,
) -> Result<CatalogKernelOutput<'candidate, B, D>> {
    let mut profiles = FoundationCatalogProfiles {
        files: Vec::new(),
        native_semantic: std::collections::BTreeMap::new(),
        complete: false,
        retained_state_bytes: std::mem::size_of::<FoundationCatalogProfiles>(),
        max_state_bytes: max_generated_state_bytes,
        max_profiles: 4098,
    };
    if profiles.retained_state_bytes > max_generated_state_bytes {
        return Err(Error::Budget("record catalog profile state"));
    }
    let mut bibliographic_phase = false;
    let mut catalog_issues = Vec::new();
    let mut generated_read_bytes = 0;
    let mut generated_inputs = None;
    let mut candidate_output: Option<CatalogCandidateArtifact<'candidate>> = None;
    let mut result = (|| {
        let catalog = prepare(
            &mut stage,
            validator,
            &mut profiles,
            index_rows.as_mut().map(|rows| &mut **rows as &mut dyn FreshIndexRowsWriter),
        )?;
        let json = JsonLimits::new(limits.catalog.max_output_row_bytes, 96, 1_000_000, 4096)
            .map_err(|_| Error::Budget("catalog manifest JSON limits"))?;
        let manifest = published_manifest(original_epoch_token, &catalog, json)?;
        let available_generated_state = max_generated_state_bytes
            .checked_sub(profiles.retained_state_bytes)
            .ok_or(Error::Budget("catalog shared generated/profile state"))?;
        if let Some((isolated, tree_limits)) = candidate_root {
            let mut sink = if let Some(rows) = index_rows.as_mut().map(|rows| &mut **rows as &mut dyn FreshIndexRowsWriter) {
                FreshCatalogSink::new_with_index_rows(
                    isolated,
                    tree_limits,
                    limits.catalog,
                    &manifest,
                    limits.deadline,
                    cancelled,
                    rows,
                )?
            } else {
                FreshCatalogSink::new(
                    isolated,
                    tree_limits,
                    limits.catalog,
                    &manifest,
                    limits.deadline,
                    cancelled,
                )?
            };
            render(&mut stage, &catalog, &mut CatalogSinkRef(&mut sink))?;
            let candidate = sink.finish()?;
            if !candidate.eof_verified() {
                return Err(Error::Invalid(
                    "fresh catalog output EOF custody incomplete",
                ));
            }
            let primary_sources = sources.borrow();
            let mut fresh_sources = RouteSources::new_until_related(
                candidate.root_path(),
                limits.deadline,
                &**primary_sources,
            )
            .map_err(|_| Error::Invalid("fresh catalog selected route custody"))?;
            drop(primary_sources);
            let observation = observe_fresh_catalog_outputs(
                &mut fresh_sources,
                &catalog,
                &manifest,
                max_generated_bytes,
                max_generated_bytes,
                max_generated_files,
                available_generated_state,
                limits.deadline,
                cancelled,
            )?;
            generated_read_bytes = observation.read_bytes();
            generated_inputs = Some(observation);
            candidate_output = Some(CatalogCandidateArtifact {
                candidate,
                fresh_sources,
            });
        } else {
            let mut sink = CompareCatalog::new(
                &sources,
                manifest,
                max_generated_bytes,
                max_generated_bytes,
                max_generated_files,
                available_generated_state,
                limits.deadline,
                cancelled,
            );
            render(&mut stage, &catalog, &mut CatalogSinkRef(&mut sink))?;
            catalog_issues = std::mem::take(&mut sink.issues);
            generated_inputs = Some(sink.observation);
        }
        if build_bibliographic {
            bibliographic_phase = true;
            bibliographic(&mut stage, &catalog, validator)?;
        }
        validator.finish()?;
        // Observation remains provisional until the original terminal fences succeed.
        observe(&mut stage, &catalog, limits.catalog)?;
        let schema_execution_cost = validator.diagnostics_v2_cumulative_cost()?;
        recheck()?;
        let mut selected_generated = generated_inputs
            .take()
            .ok_or(Error::Invalid("catalog observation absent"))?;
        if let Some(candidate) = candidate_output.as_mut() {
            selected_generated.recheck(&mut candidate.fresh_sources, limits.deadline, cancelled)?;
        } else {
            selected_generated.recheck(&mut sources.borrow_mut(), limits.deadline, cancelled)?;
        }
        generated_read_bytes = selected_generated.read_bytes();
        Ok(FoundationCatalogResult {
            observed_plan_work_bytes,
            profiles: FoundationCatalogProfiles {
                files: std::mem::take(&mut profiles.files),
                native_semantic: std::mem::take(&mut profiles.native_semantic),
                complete: profiles.complete,
                retained_state_bytes: profiles.retained_state_bytes,
                max_state_bytes: profiles.max_state_bytes,
                max_profiles: profiles.max_profiles,
            },
            issues: std::mem::take(&mut catalog_issues),
            catalog,
            bibliographic_constructed: build_bibliographic,
            generated_read_bytes,
            generated_inputs: selected_generated,
            source_recheck_read_bytes: source_recheck_read_bytes(),
            schema_execution_cost,
        })
    })();
    if result.is_err() {
        // No stage escapes this function. Drop closes SQLite and removes the
        // unexported candidate/sidecars/lease using the existing inode guards.
        drop(stage);
        if let Some(diagnostic) = take_rejection()? {
            // Retrieval exposes only complete, fully bound Invalid reports;
            // cleanup/currentness failure still refuses the whole evaluation.
            validator.finish()?;
            let schema_execution_cost = validator.diagnostics_v2_cumulative_cost()?;
            recheck()?;
            if let Some(selected) = generated_inputs.as_mut() {
                if let Some(candidate) = candidate_output.as_mut() {
                    selected.recheck(&mut candidate.fresh_sources, limits.deadline, cancelled)?;
                } else {
                    selected.recheck(&mut sources.borrow_mut(), limits.deadline, cancelled)?;
                }
                generated_read_bytes = selected.read_bytes();
            }
            drop(candidate_output.take());
            let generated_inputs = if candidate_root.is_some() {
                None
            } else {
                generated_inputs
            };
            return Ok(CatalogKernelOutput {
                outcome: FoundationCatalogOutcome::SchemaRejected {
                    observed_plan_work_bytes,
                    profiles,
                    diagnostic,
                    bibliographic_phase,
                    catalog_issues,
                    generated_read_bytes,
                    generated_inputs,
                    source_recheck_read_bytes: source_recheck_read_bytes(),
                    schema_execution_cost,
                },
                candidate: None,
            });
        }
    } else {
        // This consumes/cleans the computational stage without exporting a
        // projection/SQLite receipt. It repeats owner and input-root fences.
        finish_stage(stage)?;
        if let Ok(completed) = &mut result {
            completed.source_recheck_read_bytes = source_recheck_read_bytes();
        }
    }
    let outcome = result.map(FoundationCatalogOutcome::Complete)?;
    if candidate_root.is_some() != candidate_output.is_some() {
        return Err(Error::Invalid("fresh catalog candidate outcome absent"));
    }
    Ok(CatalogKernelOutput {
        outcome,
        candidate: candidate_output,
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn compare_with_owned_catalogue_observation(
    capture: &FoundationCapturedCut,
    sources: &mut RouteSources,
    candidate: &Path,
    isolation: &dyn StageIsolation,
    stage_limits: StageLimits,
    source_limits: SourceCatalogInputLimits,
    read_limits: ReadLimits,
    limits: BibliographicLimits,
    image: &VerifiedWorkerImageHandle,
    executor: ExecutorBudget,
    worker_limits: CutWorkerLimits,
    operation: BatchStreamBudget,
    diagnostics_limits: CutSchemaDiagnosticsLimits,
    build_bibliographic: bool,
    max_version_files: usize,
    max_version_bytes: usize,
    max_generated_bytes: usize,
    max_generated_files: usize,
    max_generated_state_bytes: usize,
    max_source_recheck_read_bytes: usize,
    quota: &tos_validation::executor::SharedSchemaWorkerQuota,
    cancelled: &AtomicBool,
    observe: impl FnOnce(
        &mut KnowledgeStage<'_>,
        &ColdSourceCatalogReceipt,
        SourceCatalogLimits,
    ) -> Result<()>,
) -> Result<FoundationCatalogOutcome> {
    compare_kernel(
        capture,
        sources,
        candidate,
        isolation,
        stage_limits,
        source_limits,
        read_limits,
        limits,
        image,
        executor,
        worker_limits,
        operation,
        diagnostics_limits,
        build_bibliographic,
        max_version_files,
        max_version_bytes,
        max_generated_bytes,
        max_generated_files,
        max_generated_state_bytes,
        max_source_recheck_read_bytes,
        quota,
        None,
        None,
        cancelled,
        observe,
    )
    .map(|output| output.outcome)
}

/// Render the maintained catalog once into a disposable source candidate.
/// Unlike `compare`, this path never uses the authored grammar tree as a stale
/// generated-file oracle. The candidate root and related route reader remain
/// owned by the returned value through host review.
#[allow(clippy::too_many_arguments)]
pub(crate) fn compare_candidate<'candidate, 'rows>(
    capture: &FoundationCapturedCut,
    sources: &mut RouteSources,
    candidate: &Path,
    isolation: &dyn StageIsolation,
    stage_limits: StageLimits,
    source_limits: SourceCatalogInputLimits,
    read_limits: ReadLimits,
    limits: BibliographicLimits,
    image: &VerifiedWorkerImageHandle,
    executor: ExecutorBudget,
    worker_limits: CutWorkerLimits,
    operation: BatchStreamBudget,
    diagnostics_limits: CutSchemaDiagnosticsLimits,
    build_bibliographic: bool,
    max_version_files: usize,
    max_version_bytes: usize,
    max_generated_bytes: usize,
    max_generated_files: usize,
    max_generated_state_bytes: usize,
    max_source_recheck_read_bytes: usize,
    quota: &tos_validation::executor::SharedSchemaWorkerQuota,
    isolated: &'candidate IsolatedCreationRoot,
    tree_limits: DisposableCatalogTreeLimits,
    index_rows: Option<&'rows mut dyn FreshIndexRowsWriter>,
    cancelled: &AtomicBool,
) -> Result<FoundationCatalogCandidateOutcome<'candidate>> {
    let mut output = compare_kernel(
        capture,
        sources,
        candidate,
        isolation,
        stage_limits,
        source_limits,
        read_limits,
        limits,
        image,
        executor,
        worker_limits,
        operation,
        diagnostics_limits,
        build_bibliographic,
        max_version_files,
        max_version_bytes,
        max_generated_bytes,
        max_generated_files,
        max_generated_state_bytes,
        max_source_recheck_read_bytes,
        quota,
        Some((isolated, tree_limits)),
        index_rows,
        cancelled,
        |_, _, _| Ok(()),
    )?;
    if let (FoundationCatalogOutcome::Complete(result), Some(candidate)) =
        (&mut output.outcome, output.candidate.as_mut())
    {
        if result.profiles.complete {
            candidate.candidate.fresh_rows_mut().native_semantic =
                std::mem::take(&mut result.profiles.native_semantic);
        }
    }
    let (candidate, fresh_sources) = output
        .candidate
        .map(|candidate| (Some(candidate.candidate), Some(candidate.fresh_sources)))
        .unwrap_or((None, None));
    Ok(FoundationCatalogCandidateOutcome {
        outcome: output.outcome,
        candidate,
        fresh_sources,
    })
}

struct SpoolCatalogStageOwner<'a> {
    input: &'a dyn tos_validation::record_biblio_cut::SourceCutInputWithIdentity<CandidateFence>,
    receipt: CandidateExactInputReceipt<CandidateFence>,
    deadline: Instant,
    cancelled: &'a AtomicBool,
}
impl CandidateStageOwner<CandidateFence> for SpoolCatalogStageOwner<'_> {
    fn verify_receipt(&self, supplied: &CandidateExactInputReceipt<CandidateFence>) -> Result<()> {
        if self.receipt.binding.input_identity() != supplied.binding.input_identity()
            || self.receipt.binding.coverage() != supplied.binding.coverage()
            || self.receipt.collections.len() != supplied.collections.len()
            || !self
                .receipt
                .collections
                .iter()
                .zip(&supplied.collections)
                .all(|(a, b)| {
                    a.source_graph == b.source_graph
                        && a.collection == b.collection
                        && a.input_role == b.input_role
                        && a.adapter_profile == b.adapter_profile
                        && a.expected_count == b.expected_count
                        && a.expected_root_sha256 == b.expected_root_sha256
                })
        {
            return Err(Error::Invalid(
                "spool catalog exact selected receipt differs",
            ));
        }
        Ok(())
    }
    fn recheck_current_input(
        &self,
        supplied: &CandidateExactInputReceipt<CandidateFence>,
    ) -> Result<()> {
        self.verify_receipt(supplied)?;
        if self.input.input_identity() != supplied.binding.input_identity() {
            return Err(Error::Invalid(
                "spool catalog actual candidate fence differs",
            ));
        }
        self.input
            .source_input()
            .verify_current_fence(supplied.binding.coverage(), self.deadline, self.cancelled)
            .map_err(|e| Error::Source(format!("spool catalog candidate fence:{e:?}")))
    }
}

/// Same catalog/default render algorithm over the actual spooled candidate.
/// The schema worker is the caller's already configured candidate worker. The
/// supplied epoch belongs to the ORIGINAL participating publication context,
/// selected and rechecked through the existing protected-control owner route;
/// no candidate epoch or source revision is constructed here. Physical source
/// reads stay in the candidate's cumulative ledger, including all fence passes.
#[allow(clippy::too_many_arguments)]
pub(crate) fn compare_spooled_candidate<'candidate>(
    input: &dyn tos_validation::record_biblio_cut::SourceCutInputWithIdentity<CandidateFence>,
    coverage: &tos_validation::record_biblio_cut::SourceCutInputCoverage,
    original_epoch: &MetadataPublicationEpoch,
    sources: &mut RouteSources,
    stage_path: &Path,
    isolation: &dyn StageIsolation,
    stage_limits: StageLimits,
    source_limits: SourceCatalogInputLimits,
    limits: BibliographicLimits,
    validator: &SourceCatalogValidator<'_>,
    build_bibliographic: bool,
    max_version_files: usize,
    max_version_bytes: usize,
    max_generated_bytes: usize,
    max_generated_files: usize,
    max_generated_state_bytes: usize,
    isolated: &'candidate IsolatedCreationRoot,
    tree_limits: DisposableCatalogTreeLimits,
    index_rows: &mut dyn FreshIndexRowsWriter,
    cancelled: &AtomicBool,
) -> Result<
    FoundationCatalogCandidateOutcome<
        'candidate,
        CandidateValidationBinding<CandidateFence>,
        tos_validation::source_cut::CandidateCutSchemaDiagnostic<CandidateFence>,
    >,
> {
    // Admit retained plan plus minimal working space before the planner can
    // allocate either. CandidateFence and coverage are fixed Copy values;
    // their identity has no hidden heap or caller-authored revision.
    let initial_receipt_state =
        tos_compiler::CandidateSourceCatalogInputPlan::<CandidateFence>::receipt_state_upper_bound(
            0,
        )?;
    let owner_inline = std::mem::size_of::<SpoolCatalogStageOwner<'_>>()
        .checked_add(std::mem::size_of::<
            CandidateValidationBinding<CandidateFence>,
        >())
        .and_then(|n| n.checked_add(initial_receipt_state))
        .and_then(|n| n.checked_add(4096))
        .ok_or(Error::Budget("spool catalog owner inline state"))?;
    let planner_workspace = max_generated_state_bytes
        .checked_sub(source_limits.max_plan_bytes)
        .and_then(|n| n.checked_sub(owner_inline))
        .filter(|n| *n >= 16 * 1024)
        .ok_or(Error::Budget("spool catalog retained plan/workspace state"))?;
    let mut plan = plan_candidate_source_catalog_inputs_with_workspace(
        input,
        input.input_identity(),
        coverage,
        source_limits,
        limits,
        cancelled,
        max_generated_state_bytes - owner_inline,
    )?;
    let observed_plan_work_bytes = plan.observed_work_bytes();
    // Clone state is measured from the retained owner's actual collection
    // strings BEFORE both owner and stage receipt clones. The original receipt
    // is separately reserved before planning; published binding is fixed.
    let receipt_clones = plan
        .input_receipt_clone_state_bytes(0)?
        .checked_mul(2)
        .ok_or(Error::Budget("spool catalog receipt clones"))?;
    let workspace = planner_workspace
        .checked_sub(receipt_clones)
        .filter(|n| *n >= 16 * 1024)
        .ok_or(Error::Budget(
            "spool catalog retained receipt/workspace state",
        ))?;
    let owner = SpoolCatalogStageOwner {
        input,
        receipt: plan.input_receipt(),
        deadline: limits.deadline,
        cancelled,
    };
    owner.recheck_current_input(&owner.receipt)?;
    let stage = KnowledgeStage::create_candidate_until_with_input_cap(
        stage_path,
        stage_limits,
        plan.input_receipt(),
        &owner,
        isolation,
        limits.deadline,
        limits.catalog.max_file_bytes,
    )?;
    let sources = RefCell::new(sources);
    let read_ledger = RefCell::new(
        tos_compiler::StreamedBibliographicReadLedger::new(
            max_version_files as u64,
            max_version_bytes as u64,
        )?,
    );
    let mut render_work = SourceCatalogRenderWorkV1::default();
    let mut output = compare_prepared_kernel(
        stage,
        validator,
        &sources,
        limits,
        observed_plan_work_bytes,
        original_epoch.token(),
        build_bibliographic,
        max_generated_bytes,
        max_generated_files,
        workspace,
        Some((isolated, tree_limits)),
        Some(index_rows),
        cancelled,
        |stage, validator, profiles, rows| {
            let rows = rows.ok_or(Error::Invalid("spool catalog index callback absent"))?;
            let mut observer = SpoolingCatalogProfileObserver { profiles, rows };
            prepare_candidate_source_catalog_plan_observed(
                &mut plan,
                input,
                stage,
                validator,
                limits,
                &mut observer,
                &mut render_work,
            )
        },
        |stage, catalog, sink| {
            render_candidate_source_witness_catalog::<CandidateFence>(
                stage,
                catalog,
                limits.catalog,
                sink,
            )
        },
        |stage, catalog, validator| {
            prepare_candidate_bibliographic_graph_from_input(
                stage,
                catalog,
                validator,
                &mut NativeBibliographicForms,
                limits,
                input,
                &read_ledger,
                workspace,
            )
            .map(|_| ())
        },
        |_, _, _| Ok(()),
        || owner.recheck_current_input(&owner.receipt),
        // Physical reads are supplied once by the actual SpoolCandidate whole
        // ledger. They must not be counted twice as cold-route recheck reads.
        || 0,
        |mut stage| {
            stage.verify_candidate_inputs::<CandidateFence>()?;
            drop(stage);
            Ok(())
        },
        || validator.take_candidate_schema_diagnostic_rejection::<CandidateFence>(),
    )?;
    if let (FoundationCatalogOutcome::Complete(result), Some(candidate)) =
        (&mut output.outcome, output.candidate.as_mut())
    {
        if result.profiles.complete {
            candidate.candidate.fresh_rows_mut().native_semantic =
                std::mem::take(&mut result.profiles.native_semantic);
        }
    }
    let (candidate, fresh_sources) = output
        .candidate
        .map(|artifact| (Some(artifact.candidate), Some(artifact.fresh_sources)))
        .unwrap_or((None, None));
    owner.recheck_current_input(&owner.receipt)?;
    Ok(FoundationCatalogCandidateOutcome {
        outcome: output.outcome,
        candidate,
        fresh_sources,
    })
}
