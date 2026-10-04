//! Temporary exact filesystem capture for the maintained foundation validator.
//! This authenticates selected bytes/membership, never source, rights or canon.
//! The whole caller holds host isolation and removes its disposable store.

use super::{
    directory, install_manifest, install_manifest_with_limit, manifest, members, object,
    object_with_limit, verify_named_export_chain,
};
use crate::source_admission_candidate::Candidate;
use crate::source_admission_spooled_index::feed_membership;
use crate::source_command::{
    self as cmd, SourceCommandError as Error, SourceCommandResult as Result,
};
use crate::source_creation_store::{IsolatedCreationRoot, MAX_BYTES, MAX_FILES, active};
use serde_json::Value as CandidateValue;
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::mem::size_of;
use std::os::unix::fs::MetadataExt;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{
    Digest256, Digest256Hasher, JsonLimits, JsonString, JsonValue, RelativePath, SourceRevision,
};
use tos_ops_mechanics_plan::route_cards::RouteSources;
use tos_source_store::{
    CorpusCutReader, CorpusReader, CutReadLimits, MemberMetadata, MetadataPublicationEpoch,
    ReadLimits, SourceMembershipV1, StreamedCorpusCutReaderV1,
    has_authored_source_descendants_v1, is_authored_source_path_v1,
};

const CONTROL: &str = "ToS/source-witnesses/.metadata-publication.json";
const CONTROL_READ_RESERVATION: usize = 8192;
const MAX_CAPTURE_RELATIVE_PATH_BYTES: usize = 4096;

pub(crate) struct FoundationCapturedCut {
    cut: FoundationCutBacking,
    membership: SourceMembershipV1,
    source_bytes: u64,
    metadata: Option<BTreeMap<String, MemberMetadata>>,
    epoch: MetadataPublicationEpoch,
    cost: FoundationCaptureCost,
}

enum FoundationCutBacking {
    Resident(CorpusCutReader),
    Streamed(StreamedCorpusCutReaderV1),
}

/// Raw authored-member work only. Publication-control/manifest reads and
/// allocator overhead require separate bounded reservations in the caller.
#[derive(Clone, Copy, Debug, Default)]
pub struct FoundationCaptureCost {
    /// Bytes read directly from the selected live ToS route.
    pub source_read_bytes: usize,
    pub source_recheck_bytes: usize,
    pub immutable_member_eof_bytes: usize,
    /// Present only for candidate-backed capture; this is a subset of the
    /// candidate's monotonic I/O usage and must not be charged twice.
    pub candidate_copy_read_bytes: Option<usize>,
    /// Equal-digest members share one object, so this is a write upper bound.
    pub object_write_upper_bound_bytes: usize,
    pub manifest_write_bytes: usize,
}

/// Explicit independent capture and aggregate recheck allowances. The caller
/// charges these to its whole operation, including parser/manifest state and
/// actual host-backed capture/stage isolation. This carrier grants no isolation.
#[derive(Clone, Copy, Debug)]
pub struct AuthoredDiagnosticCaptureLimits {
    pub read_limits: ReadLimits,
    pub cut_limits: CutReadLimits,
    /// All three existing member passes: capture, immutable EOF, live recheck.
    pub max_capture_member_read_bytes: usize,
    pub max_capture_write_bytes: usize,
    /// All callback rechecks plus the mandatory final live/control recheck.
    pub max_recheck_read_bytes: usize,
    /// Whole callback-owned value/heap plus held capture and the independently
    /// reserved final fence workspace. Construction/manifest transient state
    /// is separately charged by the caller's whole operation.
    pub max_callback_and_fence_state_bytes: usize,
    /// Caller-grounded heap allowance for T and its actual stage/provider/worker
    /// state. This declaration does not enforce opaque heap or process RSS.
    pub callback_owned_heap_state_upper_bound_bytes: usize,
    /// Held across the callback so final source/control checking can run while
    /// T and callback-owned retained state remain live.
    pub final_fence_workspace_upper_bound_bytes: usize,
}

/// Mechanical read accounting, never admission, an Original outcome or a
/// completed native-session receipt. Capture control reads remain an explicit
/// upper bound because the existing capture cost reports raw members only.
#[derive(Clone, Copy, Debug)]
pub struct AuthoredDiagnosticCaptureCost {
    pub capture: FoundationCaptureCost,
    pub callback_recheck_read_bytes: usize,
    pub final_recheck_read_bytes: usize,
    pub capture_control_read_upper_bound_bytes: usize,
    /// Reserved source-level state accounting, never measured process memory.
    pub callback_inline_value_bytes: usize,
    pub callback_owned_heap_state_upper_bound_bytes: usize,
    pub final_fence_workspace_upper_bound_bytes: usize,
    pub final_fence_workspace_lower_bound_bytes: usize,
    pub current_traversal_workspace_upper_bound_bytes: usize,
    pub callback_and_fence_state_upper_bound_bytes: usize,
}

/// Explicit comparison allowances charged inside the callback's independently
/// admitted retained-state budget. The owner enforces cumulative generated reads.
#[derive(Clone, Copy, Debug)]
pub struct AuthoredDiagnosticCatalogueComparisonLimits {
    pub max_file_bytes: usize,
    pub max_total_read_bytes: usize,
    pub max_files: usize,
    pub max_retained_state_bytes: usize,
    pub renderer_owned_state_upper_bound_bytes: usize,
    pub max_overlap_state_bytes: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct AuthoredDiagnosticCatalogueComparisonCost {
    pub generated_read_bytes: usize,
    pub observation_retained_state_bytes: usize,
    pub issue_retained_state_bytes: usize,
    pub overlap_state_upper_bound_bytes: usize,
}

/// Real owner comparison selections retained within the borrowed capture.
/// Construction is private; it exposes no mutable RouteSources or authority.
pub struct AuthoredDiagnosticCatalogueComparison<'guard, 'source> {
    capture: &'guard AuthoredDiagnosticCapture<'source>,
    observation: RefCell<super::foundation_catalog::GeneratedCatalogObservation>,
    issues: Vec<(String, String)>,
    issue_state_bytes: usize,
    overlap_state_upper_bound_bytes: usize,
}
impl AuthoredDiagnosticCatalogueComparison<'_, '_> {
    pub fn issues(&self) -> &[(String, String)] {
        &self.issues
    }
    pub fn cost(&self) -> Result<AuthoredDiagnosticCatalogueComparisonCost> {
        let observation = self
            .observation
            .try_borrow()
            .map_err(|_| Error::Conflict("authored catalogue observation already borrowed"))?;
        Ok(AuthoredDiagnosticCatalogueComparisonCost {
            generated_read_bytes: observation.read_bytes(),
            observation_retained_state_bytes: observation.retained_state_bytes(),
            issue_retained_state_bytes: self.issue_state_bytes,
            overlap_state_upper_bound_bytes: self.overlap_state_upper_bound_bytes,
        })
    }
    /// Recheck the owner's exact generated selections after later callbacks.
    /// Uses the same cumulative cap/clock/cancel; a refusal poisons outer capture.
    pub fn recheck(&self) -> Result<()> {
        let result =
            (|| {
                active(self.capture.deadline, self.capture.cancelled)?;
                if self.capture.poisoned.get() {
                    return Err(Error::Conflict(
                        "authored diagnostic capture previously refused",
                    ));
                }
                let mut observation = self.observation.try_borrow_mut().map_err(|_| {
                    Error::Conflict("authored catalogue observation already borrowed")
                })?;
                let mut sources =
                    self.capture.sources.try_borrow_mut().map_err(|_| {
                        Error::Conflict("authored diagnostic source already borrowed")
                    })?;
                observation.recheck(&mut sources, self.capture.deadline, self.capture.cancelled)
                    .map_err(catalogue_error)
            })();
        if result.is_err() {
            self.capture.poisoned.set(true);
        }
        result
    }
}

/// Borrowed access to one real capture. Its private construction retains the
/// exact mutable RouteSources, actual isolated root and original operation
/// clock through the callback. Callers cannot construct or replace this guard.
pub struct AuthoredDiagnosticCapture<'a> {
    captured: &'a FoundationCapturedCut,
    sources: RefCell<&'a mut RouteSources>,
    isolated: &'a IsolatedCreationRoot,
    read_limits: ReadLimits,
    deadline: Instant,
    cancelled: &'a AtomicBool,
    remaining_recheck_bytes: Cell<usize>,
    observed_recheck_bytes: Cell<usize>,
    poisoned: Cell<bool>,
    pass_upper_bound_bytes: usize,
    callback_owned_heap_state_upper_bound_bytes: usize,
}

impl<'source> AuthoredDiagnosticCapture<'source> {
    pub fn cut(&self) -> &CorpusCutReader {
        self.captured.cut()
    }
    pub fn revision(&self) -> SourceRevision {
        self.captured.revision()
    }
    pub fn membership(&self) -> SourceMembershipV1 {
        self.captured.membership()
    }
    pub fn epoch(&self) -> &MetadataPublicationEpoch {
        self.captured.epoch()
    }
    pub fn capture_cost(&self) -> FoundationCaptureCost {
        self.captured.cost()
    }
    /// Reuse the COMMAND-owned persisted catalogue manifest format after
    /// checking its cold binding against this actual cut and protected epoch.
    /// The caller still verifies the real catalog/stage receipt before using
    /// this pure formatter; formatting grants no source or semantic admission.
    pub fn published_catalogue_manifest(
        &self,
        receipt: &tos_compiler::source_witness_catalog::ColdSourceCatalogReceipt,
        limits: JsonLimits,
    ) -> Result<Vec<u8>> {
        active(self.deadline, self.cancelled)?;
        if self.poisoned.get() {
            return Err(Error::Conflict(
                "authored diagnostic capture recheck previously refused",
            ));
        }
        let binding = &receipt.input_binding;
        let epoch = self.captured.epoch();
        if binding.revision() != self.captured.revision()
            || binding.membership() != self.captured.membership()
            || binding.epoch_token() != epoch.token()
            || binding.epoch_generation() != epoch.generation()
            || binding.epoch_member()
                != epoch
                    .member_binding()
                    .map_err(|_| Error::Invalid("authored diagnostic protected epoch binding"))?
        {
            return Err(Error::Conflict(
                "authored diagnostic cold catalogue receipt binding",
            ));
        }
        super::foundation_catalog::published_manifest(epoch.token(), receipt, limits)
            .map_err(catalogue_error)
    }
    /// Invoke only after finishing the real cold stage and collecting bounded
    /// expected bytes. Reuses COMMAND's live CompareCatalog and protected-epoch
    /// formatter. The returned handle must be rechecked after later callbacks.
    pub fn compare_catalogue_outputs<'guard>(
        &'guard self,
        receipt: &tos_compiler::source_witness_catalog::ColdSourceCatalogReceipt,
        json: JsonLimits,
        limits: AuthoredDiagnosticCatalogueComparisonLimits,
        render: impl FnOnce(
            &mut dyn tos_compiler::source_witness_catalog::SourceCatalogSink,
        ) -> Result<()>,
    ) -> Result<AuthoredDiagnosticCatalogueComparison<'guard, 'source>> {
        let result = (|| {
            for cap in [
                limits.max_file_bytes,
                limits.max_total_read_bytes,
                limits.max_files,
                limits.max_retained_state_bytes,
                limits.max_overlap_state_bytes,
            ] {
                if cap == 0 || cap == usize::MAX {
                    return Err(Error::Invalid(
                        "authored catalogue finite comparison limits",
                    ));
                }
            }
            // Reserve manifest Vec capacity, current raw Vec, owner observations/
            // issues and renderer-owned state BEFORE formatting or comparison.
            let overlap = json
                .max_bytes
                .checked_mul(2)
                .and_then(|n| n.checked_add(limits.max_file_bytes))
                .and_then(|n| n.checked_add(limits.max_retained_state_bytes))
                .and_then(|n| n.checked_add(limits.renderer_owned_state_upper_bound_bytes))
                .and_then(|n| {
                    n.checked_add(
                        super::foundation_catalog::catalogue_comparison_controller_state_bytes(),
                    )
                })
                .and_then(|n| {
                    n.checked_add(size_of::<AuthoredDiagnosticCatalogueComparison<'_, '_>>())
                })
                .filter(|n| {
                    *n <= limits.max_overlap_state_bytes
                        && limits.max_overlap_state_bytes
                            <= self.callback_owned_heap_state_upper_bound_bytes
                })
                .ok_or(Error::Unsupported(
                    "authored catalogue comparison overlap reservation",
                ))?;
            let manifest = self.published_catalogue_manifest(receipt, json)?;
            // Retain the callback's exact command-owned refusal instead of
            // erasing it when crossing the compiler-owned sink interface.
            let mut render_error = None;
            let (issues, observation, issue_state_bytes) =
                super::foundation_catalog::compare_catalogue_outputs(
                    &self.sources,
                    manifest,
                    limits.max_file_bytes,
                    limits.max_total_read_bytes,
                    limits.max_files,
                    limits.max_retained_state_bytes,
                    self.deadline,
                    self.cancelled,
                    |sink| match render(sink) {
                        Ok(()) => Ok(()),
                        Err(error) => {
                            render_error = Some(error);
                            Err(tos_compiler::Error::Invalid("authored catalogue render refused"))
                        }
                    },
                ).map_err(|error| render_error.unwrap_or_else(|| catalogue_error(error)))?;
            Ok(AuthoredDiagnosticCatalogueComparison {
                capture: self,
                observation: RefCell::new(observation),
                issues,
                issue_state_bytes,
                overlap_state_upper_bound_bytes: overlap,
            })
        })();
        if result.is_err() {
            self.poisoned.set(true);
        }
        result
    }
    /// Reuse the exact live/epoch fence under a monotone local allowance.
    /// One final pass remains reserved even when the callback invokes this
    /// through its ColdStageOwner. No allowance or clock is reset here.
    pub fn recheck(&self) -> Result<usize> {
        self.recheck_pass(false)
    }
    fn recheck_pass(&self, final_pass: bool) -> Result<usize> {
        if self.poisoned.get() {
            return Err(Error::Conflict(
                "authored diagnostic capture recheck previously refused",
            ));
        }
        let result = self.recheck_pass_inner(final_pass);
        self.poisoned.set(result.is_err());
        result
    }
    fn recheck_pass_inner(&self, final_pass: bool) -> Result<usize> {
        active(self.deadline, self.cancelled)?;
        let needed = self
            .pass_upper_bound_bytes
            .checked_mul(if final_pass { 1 } else { 2 })
            .ok_or(Error::Unsupported(
                "authored diagnostic recheck reservation overflow",
            ))?;
        if self.remaining_recheck_bytes.get() < needed {
            return Err(Error::Unsupported(
                "authored diagnostic aggregate recheck budget",
            ));
        }
        // Charge the full pass reservation BEFORE I/O, including failure paths.
        self.remaining_recheck_bytes
            .set(self.remaining_recheck_bytes.get() - self.pass_upper_bound_bytes);
        let mut sources = self
            .sources
            .try_borrow_mut()
            .map_err(|_| Error::Conflict("authored diagnostic source already borrowed"))?;
        self.isolated
            .verify_current(self.deadline, self.cancelled)?;
        let bytes = self.captured.recheck_with_control_budget(
            &mut sources,
            self.read_limits,
            self.pass_upper_bound_bytes,
            self.deadline,
            self.cancelled,
        )?;
        self.observed_recheck_bytes.set(
            self.observed_recheck_bytes
                .get()
                .checked_add(bytes)
                .ok_or(Error::Unsupported(
                    "authored diagnostic observed read accounting",
                ))?,
        );
        self.isolated
            .verify_current(self.deadline, self.cancelled)?;
        active(self.deadline, self.cancelled)?;
        Ok(bytes)
    }
}

/// Run a read-only authored diagnostic against the existing exact capture
/// kernel. Only the private isolated store is written. The callback receives
/// no mutable source, capture constructor, arbitrary stage authority or grant.
/// The caller retains host-backed namespace isolation and owns bounded cleanup
/// of its actual newly created IsolatedCreationRoot after this function returns.
/// Initial capture selects and verifies the protected publication epoch and
/// authentic complete immutable EOF. Terminal success additionally requires
/// the same live bytes/membership, epoch, held root and isolated-root custody.
pub fn with_authored_diagnostic_capture<T>(
    sources: &mut RouteSources,
    isolated: &IsolatedCreationRoot,
    validator_sha256: Digest256,
    limits: AuthoredDiagnosticCaptureLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    diagnostic: impl for<'capture> FnOnce(&AuthoredDiagnosticCapture<'capture>) -> Result<T>,
) -> Result<(T, AuthoredDiagnosticCaptureCost)> {
    let deadline = deadline.min(sources.deadline());
    active(deadline, cancelled)?;
    for cap in [
        limits.max_capture_member_read_bytes,
        limits.max_capture_write_bytes,
        limits.max_recheck_read_bytes,
        limits.max_callback_and_fence_state_bytes,
        limits.final_fence_workspace_upper_bound_bytes,
    ] {
        if cap == 0 || cap == usize::MAX {
            return Err(Error::Invalid("authored diagnostic finite capture limits"));
        }
    }
    // Reserve the callback value and declared opaque heap BEFORE running its
    // producer. The caller's actual heap/stage quotas remain a separate owner.
    let callback_state_upper_bound_bytes = size_of::<T>()
        .checked_add(limits.callback_owned_heap_state_upper_bound_bytes)
        .and_then(|bytes| bytes.checked_add(limits.final_fence_workspace_upper_bound_bytes))
        .filter(|bytes| *bytes <= limits.max_callback_and_fence_state_bytes)
        .ok_or(Error::Unsupported(
            "authored diagnostic callback/fence state reservation",
        ))?;
    let current_traversal_workspace_upper_bound_bytes =
        RouteSources::selected_paths_workspace_upper_bound_bytes().map_err(source_error)?;
    if current_traversal_workspace_upper_bound_bytes
        > limits.final_fence_workspace_upper_bound_bytes
    {
        return Err(Error::Unsupported(
            "authored diagnostic changed-current traversal reservation",
        ));
    }
    let captured = capture_bounded_with_write_cap(
        sources,
        isolated,
        validator_sha256,
        limits.read_limits,
        limits.cut_limits,
        limits.max_capture_member_read_bytes,
        limits.max_capture_write_bytes,
        deadline,
        cancelled,
    )?;
    if captured.streamed_cut().is_some() || captured.cost().candidate_copy_read_bytes.is_some() {
        return Err(Error::Invalid(
            "authored diagnostic requires genuine live resident capture",
        ));
    }
    let pass_upper_bound_bytes = captured
        .cost()
        .source_read_bytes
        .checked_add(CONTROL_READ_RESERVATION)
        .filter(|bytes| *bytes <= limits.max_recheck_read_bytes)
        .ok_or(Error::Unsupported(
            "authored diagnostic mandatory final recheck reservation",
        ))?;
    // The current live kernel materializes each raw member before hashing;
    // it does not use a streaming 32KiB buffer. Reuse the owner's existing
    // retained metadata/path/control parse envelope without changing it, then
    // reserve the changed-current traversal worst case, largest actual raw
    // member and guard frame. Captured names never bound changed-current names.
    // Directory traversal and opaque caller state still require independently
    // grounded caller allowances; this is source accounting, never RSS.
    let metadata = captured
        .metadata
        .as_ref()
        .ok_or(Error::Invalid("authored diagnostic live capture metadata"))?;
    let mut file_name_bytes = 0usize;
    let mut largest_member_bytes = 0usize;
    for (path, member) in metadata {
        file_name_bytes = file_name_bytes
            .checked_add(path.len())
            .ok_or(Error::Unsupported(
                "authored diagnostic fence name accounting",
            ))?;
        largest_member_bytes = largest_member_bytes.max(
            usize::try_from(member.size_bytes)
                .map_err(|_| Error::Unsupported("authored diagnostic member size range"))?,
        );
    }
    let held_capture_control_state =
        candidate_capture_retained_state_upper_bound(metadata.len(), file_name_bytes).ok_or(
            Error::Unsupported("authored diagnostic held capture/control reservation"),
        )?;
    let fence_state_lower_bound_bytes = current_traversal_workspace_upper_bound_bytes
        .checked_add(largest_member_bytes)
        .and_then(|bytes| bytes.checked_add(held_capture_control_state))
        .and_then(|bytes| bytes.checked_add(size_of::<AuthoredDiagnosticCapture<'_>>()))
        .filter(|bytes| *bytes <= limits.final_fence_workspace_upper_bound_bytes)
        .ok_or(Error::Unsupported(
            "authored diagnostic final fence workspace reservation",
        ))?;
    let guard = AuthoredDiagnosticCapture {
        captured: &captured,
        sources: RefCell::new(sources),
        isolated,
        // A changed larger member must refuse before raw Vec allocation,
        // preserving the actual captured-largest workspace reservation.
        read_limits: ReadLimits {
            max_selected_object_bytes: limits
                .read_limits
                .max_selected_object_bytes
                .min(largest_member_bytes.max(1) as u64),
            ..limits.read_limits
        },
        deadline,
        cancelled,
        remaining_recheck_bytes: Cell::new(limits.max_recheck_read_bytes),
        observed_recheck_bytes: Cell::new(0),
        poisoned: Cell::new(false),
        pass_upper_bound_bytes,
        callback_owned_heap_state_upper_bound_bytes: limits
            .callback_owned_heap_state_upper_bound_bytes,
    };
    let value = diagnostic(&guard)?;
    let callback_recheck_read_bytes = guard.observed_recheck_bytes.get();
    let final_recheck_read_bytes = guard.recheck_pass(true)?;
    Ok((
        value,
        AuthoredDiagnosticCaptureCost {
            capture: captured.cost(),
            callback_recheck_read_bytes,
            final_recheck_read_bytes,
            capture_control_read_upper_bound_bytes: CONTROL_READ_RESERVATION * 2,
            callback_inline_value_bytes: size_of::<T>(),
            callback_owned_heap_state_upper_bound_bytes: limits
                .callback_owned_heap_state_upper_bound_bytes,
            final_fence_workspace_upper_bound_bytes: limits.final_fence_workspace_upper_bound_bytes,
            final_fence_workspace_lower_bound_bytes: fence_state_lower_bound_bytes,
            current_traversal_workspace_upper_bound_bytes,
            callback_and_fence_state_upper_bound_bytes: callback_state_upper_bound_bytes,
        },
    ))
}

/// The public capture API retains COMMAND's refusal vocabulary. Compiler
/// mechanics supply no new admission; callback-owned errors are kept separately.
fn catalogue_error(error: tos_compiler::Error) -> Error {
    match error {
        tos_compiler::Error::Invalid(message) => Error::Invalid(message),
        tos_compiler::Error::Budget(message)
        | tos_compiler::Error::PreparedUnsupported(message)
        | tos_compiler::Error::ManagedSourceUnsupported(message) => Error::Unsupported(message),
        tos_compiler::Error::Io(_) => Error::Denied("authored catalogue descriptor custody"),
        tos_compiler::Error::Sql(_)
        | tos_compiler::Error::SqlitePhase { .. }
        | tos_compiler::Error::SqliteVmBudget { .. }
        | tos_compiler::Error::Source(_) => Error::Invalid("authored catalogue owner refused"),
    }
}
fn source_error(_: std::io::Error) -> Error {
    Error::Denied("foundation source descriptor custody")
}
fn candidate_error(_: std::io::Error) -> Error {
    Error::Denied("foundation admission candidate custody")
}
fn state(sources: &mut RouteSources) -> Result<Option<JsonValue>> {
    state_with_cost(sources).map(|(value, _)| value)
}
fn state_with_cost(sources: &mut RouteSources) -> Result<(Option<JsonValue>, usize)> {
    let raw = sources
        .protected_control_bytes(CONTROL)
        .map_err(source_error)?;
    let bytes = raw.as_ref().map_or(0, Vec::len);
    Ok((raw.map(|raw| cmd::parse(&raw)).transpose()?, bytes))
}
pub(crate) fn select_epoch_with_cost(
    sources: &mut RouteSources,
) -> Result<(MetadataPublicationEpoch, usize)> {
    let (current_state, bytes) = state_with_cost(sources)?;
    let epoch = MetadataPublicationEpoch::select(current_state)
        .map_err(|_| Error::Conflict("foundation publication selection refused"))?;
    Ok((epoch, bytes))
}
pub(crate) fn verify_epoch_with_cost(
    sources: &mut RouteSources,
    epoch: &MetadataPublicationEpoch,
) -> Result<usize> {
    let (current_state, bytes) = state_with_cost(sources)?;
    epoch
        .verify_current(current_state)
        .map_err(|_| Error::Conflict("foundation publication changed"))?;
    sources.verify_root().map_err(source_error)?;
    Ok(bytes)
}
pub(crate) fn selected(path: &str, directory: bool) -> bool {
    // Source-store eligibility does not admit ignored private bodies. These
    // owner roots keep only their direct authored README in the cut; selected
    // private inputs are observed later under their separate physical custody.
    if let Some(suffix) = private_capture_suffix(path) {
        return if directory {
            suffix.is_empty()
        } else {
            suffix == "/README.md"
        };
    }
    if !directory
        && path.starts_with("ToS/canon/")
        && (path == "ToS/canon/..canonical-human-forms-lock.writer.lock"
            || path.rsplit('/').next() == Some(".node.human-forms.json.writer.lock"))
    {
        return false;
    }
    if directory {
        has_authored_source_descendants_v1(path)
    } else {
        is_authored_source_path_v1(path)
    }
}

fn candidate_member(path: &str, row: &CandidateValue) -> Result<Option<(Digest256, u64, u32)>> {
    if !path.starts_with("ToS/") || !selected(path, false) {
        return Ok(None);
    }
    let fields = row
        .as_object()
        .ok_or(Error::Invalid("foundation candidate member row"))?;
    if fields.len() != 4
        || ["path", "sha256", "size_bytes", "mode"]
            .iter()
            .any(|field| !fields.contains_key(*field))
        || row.get("path").and_then(CandidateValue::as_str) != Some(path)
    {
        return Err(Error::Invalid("foundation candidate member binding"));
    }
    let digest = row
        .get("sha256")
        .and_then(CandidateValue::as_str)
        .and_then(|value| Digest256::from_hex(value).ok())
        .ok_or(Error::Invalid("foundation candidate member digest"))?;
    let size = row
        .get("size_bytes")
        .and_then(CandidateValue::as_u64)
        .ok_or(Error::Invalid("foundation candidate member size"))?;
    let mode = row
        .get("mode")
        .and_then(CandidateValue::as_u64)
        .filter(|mode| matches!(*mode, 0o600 | 0o644 | 0o755))
        .and_then(|mode| u32::try_from(mode).ok())
        .ok_or(Error::Invalid("foundation candidate member mode"))?;
    Ok(Some((digest, size, mode)))
}

fn candidate_path_is_normalized(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\\')
        && !path.chars().any(|character| (character as u32) < 32)
        && path
            .split('/')
            .all(|part| !part.is_empty() && !matches!(part, "." | ".." | ".git"))
}

fn candidate_manifest_upper_bound(
    path_bytes: usize,
    quote_bytes: usize,
    count: usize,
) -> Option<usize> {
    // RelativePath excludes backslashes and control characters. Canonical JSON
    // can therefore add at most one escape byte for each quote in a path.
    512usize
        .checked_add(path_bytes)?
        .checked_add(quote_bytes)?
        .checked_add(count.checked_mul(192)?)
}

fn candidate_capture_retained_state_upper_bound(count: usize, path_bytes: usize) -> Option<usize> {
    let word = size_of::<usize>();
    // The borrowed normalized-member census runs before any capture allocation.
    // These six owned path copies are fresh clone/to_owned buffers; upstream
    // candidate path capacity is accounted by the candidate's separate ledger.
    let path_storage = path_bytes.checked_mul(6)?;
    let member_row = size_of::<MemberMetadata>()
        .checked_mul(3)?
        .checked_add(size_of::<String>().checked_mul(2)?)?
        .checked_add(size_of::<RelativePath>())?
        .checked_add(word.checked_mul(64)?)?;
    let member_state = count.checked_mul(member_row)?.checked_add(path_storage)?;
    let control_visits = JsonLimits::default()
        .max_visits
        .min(CONTROL_READ_RESERVATION);
    let control_state =
        CONTROL_READ_RESERVATION
            .checked_mul(8)?
            .checked_add(control_visits.checked_mul(
                2usize.checked_mul(size_of::<JsonValue>().checked_add(size_of::<JsonString>())?)?,
            )?)?;
    size_of::<FoundationCapturedCut>()
        .checked_add(size_of::<CorpusReader>())
        .and_then(|bytes| bytes.checked_add(control_state))
        .and_then(|bytes| bytes.checked_add(member_state))
}

fn candidate_capture_state_upper_bound(
    count: usize,
    path_bytes: usize,
    max_member_bytes: usize,
    manifest_bytes: usize,
) -> Option<usize> {
    let (retained, candidate_read_path_state, capture_workspace) =
        candidate_capture_state_parts(count, path_bytes, max_member_bytes, manifest_bytes)?;
    retained
        .checked_add(candidate_read_path_state)?
        .checked_add(capture_workspace)
}

fn candidate_capture_state_parts(
    count: usize,
    path_bytes: usize,
    max_member_bytes: usize,
    manifest_bytes: usize,
) -> Option<(usize, usize, usize)> {
    let word = size_of::<usize>();
    let retained = candidate_capture_retained_state_upper_bound(count, path_bytes)?;
    let candidate_read_path_state = count
        .checked_mul(size_of::<String>().checked_add(word.checked_mul(64)?)?)?
        .checked_add(path_bytes)?;
    let visits = count.checked_mul(5)?.checked_add(9)?;
    let manifest_workspace = manifest_bytes
        .checked_mul(8)?
        .checked_add(visits.checked_mul(
            2usize.checked_mul(size_of::<JsonValue>().checked_add(size_of::<JsonString>())?)?,
        )?)?;
    let largest_member_workspace = max_member_bytes.checked_mul(2)?.checked_add(65_536)?;
    Some((
        retained,
        candidate_read_path_state,
        largest_member_workspace
            .max(manifest_workspace)
            .max(tos_compiler::private_tmpfs_stage::PRIVATE_TMPFS_VERIFY_COST.workspace_bytes),
    ))
}

struct CandidateByteSink {
    bytes: Vec<u8>,
    limit: usize,
}

impl Write for CandidateByteSink {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        if self
            .bytes
            .len()
            .checked_add(input.len())
            .is_none_or(|length| length > self.limit)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "candidate member exceeds selected byte limit",
            ));
        }
        self.bytes.extend_from_slice(input);
        Ok(input.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn private_capture_suffix(path: &str) -> Option<&str> {
    for root in [
        "ToS/source-witnesses/access-requests/private",
        "ToS/zarathustra/lived-witness/local-content",
    ] {
        if let Some(suffix) = path.strip_prefix(root)
            && (suffix.is_empty() || suffix.starts_with('/'))
        {
            return Some(suffix);
        }
    }
    let work_path = path.strip_prefix("ToS/source-witnesses/works/")?;
    let (_, gold_path) = work_path.split_once("/gold-sets/")?;
    let (gold_name, relative) = gold_path.split_once('/')?;
    if gold_name.is_empty() {
        return None;
    }
    let suffix = relative.strip_prefix("local-content")?;
    (suffix.is_empty() || suffix.starts_with('/')).then_some(suffix)
}
fn paths(
    sources: &mut RouteSources,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<Vec<String>> {
    active(deadline, cancel)?;
    let mut files = Vec::new();
    for path in sources
        .selected_paths("ToS", &selected)
        .map_err(source_error)?
    {
        active(deadline, cancel)?;
        if sources.is_file(&path).map_err(source_error)? {
            if files.len() >= MAX_FILES {
                return Err(Error::Unsupported("foundation current member count"));
            }
            files.push(path);
        } else if !sources.is_dir(&path).map_err(source_error)? {
            return Err(Error::Denied("foundation authored input is not regular"));
        }
    }
    Ok(files)
}
fn membership(metadata: &BTreeMap<String, MemberMetadata>) -> SourceMembershipV1 {
    let mut hash = Digest256Hasher::new();
    hash.update(b"tos-val-full-membership-v1\0");
    for (path, member) in metadata {
        hash.update(&(path.len() as u64).to_be_bytes());
        hash.update(path.as_bytes());
        hash.update(&member.size_bytes.to_be_bytes());
        hash.update(member.sha256.as_bytes());
    }
    SourceMembershipV1 {
        count: metadata.len() as u64,
        digest: hash.finalize(),
    }
}

impl FoundationCapturedCut {
    pub(crate) fn cut(&self) -> &CorpusCutReader {
        match &self.cut {
            FoundationCutBacking::Resident(cut) => cut,
            FoundationCutBacking::Streamed(_) => {
                panic!("resident source-cut access on streamed foundation capture")
            }
        }
    }
    pub(crate) fn streamed_cut(&self) -> Option<&StreamedCorpusCutReaderV1> {
        match &self.cut {
            FoundationCutBacking::Resident(_) => None,
            FoundationCutBacking::Streamed(cut) => Some(cut),
        }
    }
    pub(crate) fn revision(&self) -> SourceRevision {
        match &self.cut {
            FoundationCutBacking::Resident(cut) => cut.current().revision(),
            FoundationCutBacking::Streamed(cut) => cut.current_revision(),
        }
    }
    pub(crate) fn membership(&self) -> SourceMembershipV1 {
        self.membership
    }
    pub(crate) fn source_bytes(&self) -> u64 {
        self.source_bytes
    }
    pub(crate) fn epoch(&self) -> &MetadataPublicationEpoch {
        &self.epoch
    }

    pub(crate) fn cost(&self) -> FoundationCaptureCost {
        self.cost
    }
    /// Exact independently captured inventory, already bounded and ordered.
    /// It contains authored members only; payload and auxiliary custody are
    /// selected separately by the command owner.
    pub(crate) fn current_paths(&self) -> Vec<String> {
        self.metadata
            .as_ref()
            .map(|metadata| metadata.keys().cloned().collect())
            .unwrap_or_default()
    }
    pub(crate) fn observed_members(&self) -> impl Iterator<Item = (&str, &MemberMetadata)> {
        self.metadata
            .as_ref()
            .into_iter()
            .flat_map(|metadata| metadata.iter())
            .map(|(path, member)| (path.as_str(), member))
    }

    pub(crate) fn member(
        &self,
        path: &RelativePath,
    ) -> io::Result<Option<MemberMetadata>> {
        match &self.cut {
            FoundationCutBacking::Resident(_) => Ok(self
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.get(path.as_str()).cloned())),
            FoundationCutBacking::Streamed(cut) => cut
                .member(self.revision(), path)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "captured member read")),
        }
    }

    pub(crate) fn member_after(
        &self,
        after: Option<&RelativePath>,
    ) -> io::Result<Option<MemberMetadata>> {
        match &self.cut {
            FoundationCutBacking::Resident(_) => Ok(self.metadata.as_ref().and_then(|metadata| {
                match after {
                    Some(after) => metadata
                        .range::<str, _>((std::ops::Bound::Excluded(after.as_str()), std::ops::Bound::Unbounded))
                        .next()
                        .map(|(_, member)| member.clone()),
                    None => metadata.values().next().cloned(),
                }
            })),
            FoundationCutBacking::Streamed(cut) => cut
                .member_after(self.revision(), after)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "captured member cursor")),
        }
    }

    pub(crate) fn read_member(
        &self,
        path: &RelativePath,
        max_bytes: usize,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> io::Result<Vec<u8>> {
        let max_bytes = u64::try_from(max_bytes)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "captured read cap"))?;
        match &self.cut {
            FoundationCutBacking::Resident(cut) => cut
                .read_member(self.revision(), path, max_bytes, deadline, cancel)
                .map(|member| member.raw)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "captured member bytes")),
            FoundationCutBacking::Streamed(cut) => cut
                .read_member(self.revision(), path, max_bytes, deadline, cancel)
                .map(|member| member.raw)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "captured member bytes")),
        }
    }

    /// Visit a sorted current-member cursor without materializing the complete
    /// source path set. The streamed branch checks a true cursor EOF against
    /// the capture's authenticated full-membership receipt.
    pub(crate) fn for_each_member(
        &self,
        deadline: Instant,
        cancel: &AtomicBool,
        mut visit: impl FnMut(&MemberMetadata) -> io::Result<()>,
    ) -> io::Result<()> {
        match &self.cut {
            FoundationCutBacking::Resident(_) => {
                let metadata = self
                    .metadata
                    .as_ref()
                    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "capture metadata absent"))?;
                if membership(metadata) != self.membership
                    || metadata.values().try_fold(0u64, |total, member| {
                        total.checked_add(member.size_bytes)
                    }) != Some(self.source_bytes)
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "captured resident membership changed",
                    ));
                }
                for member in metadata.values() {
                    active(deadline, cancel).map_err(|_| {
                        io::Error::new(io::ErrorKind::Interrupted, "captured member walk interrupted")
                    })?;
                    visit(member)?;
                }
            }
            FoundationCutBacking::Streamed(_) => {
                let mut after = None;
                let mut hash = Digest256Hasher::new();
                hash.update(b"tos-val-full-membership-v1\0");
                let mut count = 0u64;
                let mut observed_bytes = 0u64;
                loop {
                    active(deadline, cancel).map_err(|_| {
                        io::Error::new(io::ErrorKind::Interrupted, "captured member walk interrupted")
                    })?;
                    let next = self.member_after(after.as_ref());
                    active(deadline, cancel).map_err(|_| {
                        io::Error::new(io::ErrorKind::Interrupted, "captured member cursor interrupted")
                    })?;
                    let Some(member) = next? else { break };
                    count = count
                        .checked_add(1)
                        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "member count overflow"))?;
                    observed_bytes = observed_bytes
                        .checked_add(member.size_bytes)
                        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "member byte count overflow"))?;
                    feed_membership(&mut hash, &member.path, member.size_bytes, member.sha256);
                    visit(&member)?;
                    after = Some(member.path);
                }
                active(deadline, cancel).map_err(|_| {
                    io::Error::new(io::ErrorKind::Interrupted, "captured member EOF interrupted")
                })?;
                if count != self.membership.count
                    || (SourceMembershipV1 {
                        count,
                        digest: hash.finalize(),
                    } != self.membership)
                    || observed_bytes != self.source_bytes
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "captured streamed membership did not reach authenticated EOF",
                    ));
                }
            }
        }
        Ok(())
    }

    /// Re-enumerate and rehash actual current files; a participating epoch alone
    /// is not a complete membership certificate. Cut absence says nothing about
    /// external payload bytes, generated catalogs, or software custody.
    pub(crate) fn recheck(
        &self,
        sources: &mut RouteSources,
        limits: ReadLimits,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> Result<()> {
        self.recheck_bounded(sources, limits, MAX_BYTES, deadline, cancel)
            .map(|_| ())
    }

    /// The caller reserves this pass from its remaining whole-operation raw
    /// read allowance before entering it. No independent allowance is reset.
    pub(crate) fn recheck_bounded(
        &self,
        sources: &mut RouteSources,
        limits: ReadLimits,
        max_source_read_bytes: usize,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> Result<usize> {
        self.recheck_inner(sources, limits, max_source_read_bytes, deadline, cancel)
            .map(|(members, _)| members)
    }

    /// Include the protected publication control in one caller-selected
    /// read allowance. Its kernel cap is reserved before any member read.
    pub(crate) fn recheck_with_control_budget(
        &self,
        sources: &mut RouteSources,
        limits: ReadLimits,
        max_total_read_bytes: usize,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> Result<usize> {
        let member_allowance = max_total_read_bytes
            .checked_sub(CONTROL_READ_RESERVATION)
            .ok_or(Error::Unsupported(
                "foundation publication-control read reservation",
            ))?;
        let (members, control) =
            self.recheck_inner(sources, limits, member_allowance, deadline, cancel)?;
        members
            .checked_add(control)
            .ok_or(Error::Unsupported("foundation recheck read accounting"))
    }

    /// Candidate-backed capture has no live authored-source reread. Its sealed
    /// bytes are rechecked through the immutable cut, then the selected
    /// publication control and held root are verified in the same allowance.
    pub(crate) fn recheck_candidate_transport_with_control_budget(
        &self,
        sources: &mut RouteSources,
        max_total_read_bytes: usize,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> Result<usize> {
        if self.cost.candidate_copy_read_bytes.is_none() {
            return Err(Error::Invalid(
                "candidate transport recheck on live-source capture",
            ));
        }
        let member_allowance = max_total_read_bytes
            .checked_sub(CONTROL_READ_RESERVATION)
            .ok_or(Error::Unsupported(
                "foundation publication-control read reservation",
            ))?;
        let mut member_bytes = 0usize;
        match &self.cut {
            FoundationCutBacking::Resident(cut) => {
                let mut stream = cut
                    .stream(self.revision())
                    .map_err(|_| Error::Invalid("foundation candidate immutable stream"))?;
                while let Some(member) = stream
                    .next_member(deadline, cancel)
                    .map_err(|_| Error::Invalid("foundation candidate immutable EOF"))?
                {
                    member_bytes = member_bytes
                        .checked_add(member.raw.len())
                        .filter(|bytes| *bytes <= member_allowance)
                        .ok_or(Error::Unsupported(
                            "foundation candidate immutable read budget",
                        ))?;
                }
                if stream.coverage() != Some(self.membership) {
                    return Err(Error::Conflict(
                        "foundation candidate immutable membership changed",
                    ));
                }
            }
            FoundationCutBacking::Streamed(_) => self
                .for_each_member(deadline, cancel, |metadata| {
                    let raw = self
                        .read_member(
                            &metadata.path,
                            usize::try_from(metadata.size_bytes).map_err(|_| {
                                io::Error::new(io::ErrorKind::InvalidData, "member size range")
                            })?,
                            deadline,
                            cancel,
                        )?;
                    member_bytes = member_bytes
                        .checked_add(raw.len())
                        .filter(|bytes| *bytes <= member_allowance)
                        .ok_or_else(|| {
                            io::Error::new(
                                io::ErrorKind::InvalidData,
                                "foundation candidate immutable read budget",
                            )
                        })?;
                    Ok(())
                })
                .map_err(|_| Error::Invalid("foundation candidate immutable EOF"))?,
        }

        let (current_state, control_bytes) = state_with_cost(sources)?;
        if control_bytes > CONTROL_READ_RESERVATION {
            return Err(Error::Unsupported(
                "foundation publication-control read reservation",
            ));
        }
        self.epoch
            .verify_current(current_state)
            .map_err(|_| Error::Conflict("foundation publication changed"))?;
        sources.verify_root().map_err(source_error)?;
        active(deadline, cancel)?;
        member_bytes
            .checked_add(control_bytes)
            .filter(|bytes| *bytes <= max_total_read_bytes)
            .ok_or(Error::Unsupported(
                "foundation candidate transport read accounting",
            ))
    }

    fn recheck_inner(
        &self,
        sources: &mut RouteSources,
        limits: ReadLimits,
        max_source_read_bytes: usize,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> Result<(usize, usize)> {
        let metadata = self
            .metadata
            .as_ref()
            .ok_or(Error::Invalid("foundation live capture metadata unavailable"))?;
        let current = paths(sources, deadline, cancel)?;
        if !current
            .iter()
            .map(String::as_str)
            .eq(metadata.keys().map(String::as_str))
        {
            return Err(Error::Conflict("foundation live membership changed"));
        }
        let mut read_bytes = 0;
        for path in current {
            active(deadline, cancel)?;
            let (raw, physical) = sources
                .bounded_metadata_bytes(
                    &path,
                    limits.max_selected_object_bytes.min(MAX_BYTES as u64) as usize,
                    &mut read_bytes,
                    max_source_read_bytes.min(MAX_BYTES),
                )
                .map_err(source_error)?;
            let original = &metadata[&path];
            if raw.len() as u64 != original.size_bytes
                || Digest256::of_bytes(&raw) != original.sha256
                || physical.mode() & 0o7777 != original.mode
            {
                return Err(Error::Conflict("foundation live source bytes/mode changed"));
            }
        }
        let (current_state, control_bytes) = state_with_cost(sources)?;
        self.epoch
            .verify_current(current_state)
            .map_err(|_| Error::Conflict("foundation publication changed"))?;
        sources.verify_root().map_err(source_error)?;
        active(deadline, cancel)?;
        Ok((read_bytes, control_bytes))
    }
}

/// Caller-selected real live source into the existing immutable v1 carrier.
/// Empty index claims mean none are asserted: the full validator builds and
/// checks identities/dependencies from these exact source bytes. This ephemeral
/// current capture is not a durable historical/canon admission manifest.
pub(crate) fn capture(
    sources: &mut RouteSources,
    isolated: &IsolatedCreationRoot,
    validator_sha256: Digest256,
    read_limits: ReadLimits,
    cut_limits: CutReadLimits,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<FoundationCapturedCut> {
    capture_bounded(
        sources,
        isolated,
        validator_sha256,
        read_limits,
        cut_limits,
        MAX_BYTES * 3,
        deadline,
        cancel,
    )
}

/// Bound the three raw authored-member passes performed by capture: source
/// materialization, authentic immutable-stream EOF, and initial live recheck.
/// A final invocation EOF pass is reserved separately by the caller.
#[allow(clippy::too_many_arguments)]
pub(crate) fn capture_bounded(
    sources: &mut RouteSources,
    isolated: &IsolatedCreationRoot,
    validator_sha256: Digest256,
    read_limits: ReadLimits,
    cut_limits: CutReadLimits,
    max_source_read_bytes: usize,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<FoundationCapturedCut> {
    let max_capture_write_bytes = (max_source_read_bytes / 3)
        .min(MAX_BYTES)
        .checked_add(read_limits.max_manifest_bytes)
        .ok_or(Error::Unsupported(
            "foundation capture write limit overflow",
        ))?;
    capture_bounded_with_write_cap(
        sources,
        isolated,
        validator_sha256,
        read_limits,
        cut_limits,
        max_source_read_bytes,
        max_capture_write_bytes,
        deadline,
        cancel,
    )
}

/// Capture the authored members of one already-prepared admission candidate.
/// Candidate bytes are copied once into the same immutable v1 carrier used by
/// live capture; this function neither rereads nor attests the live grammar
/// checkout. The caller owns candidate custody and epoch selection.
/// `max_state_bytes` is NET operation-window headroom: the candidate's existing
/// retained state has already been charged by the caller's whole-operation
/// ledger. Capture state and the newly retained read-path set are preflighted
/// together before candidate bytes are copied.
#[allow(clippy::too_many_arguments)]
pub(crate) fn capture_candidate(
    candidate: &Candidate<'_>,
    isolated: &IsolatedCreationRoot,
    epoch: MetadataPublicationEpoch,
    validator_sha256: Digest256,
    read_limits: ReadLimits,
    cut_limits: CutReadLimits,
    max_source_read_bytes: usize,
    max_capture_write_bytes: usize,
    max_state_bytes: usize,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<FoundationCapturedCut> {
    active(deadline, cancel)?;
    candidate.tick().map_err(candidate_error)?;
    read_limits
        .validate()
        .map_err(|_| Error::Invalid("foundation source read limits"))?;
    if cut_limits.max_revisions == 0
        || cut_limits.max_revisions == usize::MAX
        || cut_limits.max_members == 0
        || cut_limits.max_members == u64::MAX
        || cut_limits.max_total_bytes == 0
        || cut_limits.max_total_bytes == u64::MAX
        || cut_limits.max_member_bytes == 0
        || cut_limits.max_member_bytes == u64::MAX
    {
        return Err(Error::Invalid("foundation source cut limits"));
    }
    if max_source_read_bytes == 0 || max_capture_write_bytes == 0 || max_state_bytes == 0 {
        return Err(Error::Unsupported(
            "foundation candidate capture read/write/state budget",
        ));
    }

    // First walk only borrowed metadata. The independent candidate-copy and
    // immutable EOF passes each read one complete selected member set.
    let mut count = 0usize;
    let mut total_member_bytes = 0u64;
    let mut max_member_bytes = 0u64;
    let mut path_bytes = 0usize;
    let mut quote_bytes = 0usize;
    for (path, row) in &candidate.members {
        active(deadline, cancel)?;
        candidate.tick().map_err(candidate_error)?;
        let Some((_, size, _)) = candidate_member(path, row)? else {
            continue;
        };
        if path.len() > MAX_CAPTURE_RELATIVE_PATH_BYTES {
            return Err(Error::Unsupported(
                "foundation candidate relative path byte bound",
            ));
        }
        if !candidate_path_is_normalized(path) {
            return Err(Error::Invalid("foundation candidate relative path"));
        }
        if size > read_limits.max_selected_object_bytes || size > cut_limits.max_member_bytes {
            return Err(Error::Unsupported("foundation candidate member byte bound"));
        }
        count = count
            .checked_add(1)
            .filter(|count| {
                *count <= read_limits.max_manifest_entries
                    && *count as u64 <= cut_limits.max_members
            })
            .ok_or(Error::Unsupported(
                "foundation candidate member/manifest bound",
            ))?;
        total_member_bytes = total_member_bytes
            .checked_add(size)
            .filter(|bytes| *bytes <= cut_limits.max_total_bytes)
            .ok_or(Error::Unsupported(
                "foundation candidate aggregate member bound",
            ))?;
        max_member_bytes = max_member_bytes.max(size);
        path_bytes = path_bytes
            .checked_add(path.len())
            .ok_or(Error::Unsupported("foundation candidate path state bound"))?;
        quote_bytes = quote_bytes
            .checked_add(path.as_bytes().iter().filter(|byte| **byte == b'"').count())
            .ok_or(Error::Unsupported("foundation candidate path state bound"))?;
    }
    if count == 0 {
        return Err(Error::Unsupported(
            "foundation capture member/manifest bound",
        ));
    }
    let total_member_bytes = usize::try_from(total_member_bytes)
        .map_err(|_| Error::Unsupported("foundation candidate member byte range"))?;
    let two_pass_read_bytes = total_member_bytes
        .checked_mul(2)
        .filter(|bytes| *bytes <= max_source_read_bytes)
        .ok_or(Error::Unsupported(
            "foundation candidate aggregate read budget",
        ))?;
    let manifest_upper = candidate_manifest_upper_bound(path_bytes, quote_bytes, count)
        .ok_or(Error::Unsupported("foundation candidate manifest bound"))?;
    let manifest_workspace = manifest_upper
        .min(read_limits.max_manifest_bytes)
        .min(read_limits.json.max_bytes);
    let retained_capture_state = candidate_capture_retained_state_upper_bound(count, path_bytes)
        .ok_or(Error::Unsupported(
            "foundation candidate retained state bound",
        ))?;
    let metadata_peak = candidate_capture_state_upper_bound(
        count,
        path_bytes,
        usize::try_from(max_member_bytes)
            .map_err(|_| Error::Unsupported("foundation candidate member byte range"))?,
        manifest_workspace,
    )
    .ok_or(Error::Unsupported(
        "foundation candidate capture state bound",
    ))?;
    let unique_index_peak = count
        .checked_mul(
            size_of::<Digest256>()
                .checked_add(size_of::<u64>())
                .and_then(|bytes| bytes.checked_add(size_of::<usize>().checked_mul(64)?))
                .ok_or(Error::Unsupported(
                    "foundation candidate digest index state bound",
                ))?,
        )
        .ok_or(Error::Unsupported(
            "foundation candidate digest index state bound",
        ))?;
    if metadata_peak.max(unique_index_peak) > max_state_bytes {
        return Err(Error::Unsupported(
            "foundation candidate capture state budget",
        ));
    }

    // Count unique immutable objects before any candidate copy or stage write.
    let mut unique_objects = BTreeMap::<Digest256, u64>::new();
    let mut unique_object_bytes = 0u64;
    for (path, row) in &candidate.members {
        active(deadline, cancel)?;
        candidate.tick().map_err(candidate_error)?;
        let Some((digest, size, _)) = candidate_member(path, row)? else {
            continue;
        };
        match unique_objects.get(&digest) {
            Some(previous) if *previous != size => {
                return Err(Error::Conflict(
                    "foundation candidate digest has conflicting member sizes",
                ));
            }
            Some(_) => (),
            None => {
                unique_object_bytes =
                    unique_object_bytes
                        .checked_add(size)
                        .ok_or(Error::Unsupported(
                            "foundation candidate object write accounting",
                        ))?;
                unique_objects.insert(digest, size);
            }
        }
    }
    drop(unique_objects);

    // Retain exact candidate metadata for the manifest and sealed cut. No
    // selected path list or duplicate byte map is created.
    let mut metadata = BTreeMap::new();
    for (path, row) in &candidate.members {
        active(deadline, cancel)?;
        candidate.tick().map_err(candidate_error)?;
        let Some((digest, size_bytes, mode)) = candidate_member(path, row)? else {
            continue;
        };
        let relative = RelativePath::parse(path)
            .map_err(|_| Error::Invalid("foundation candidate relative path"))?;
        metadata.insert(
            path.clone(),
            MemberMetadata {
                path: relative,
                sha256: digest,
                size_bytes,
                mode,
            },
        );
    }
    if metadata.len() != count {
        return Err(Error::Conflict("foundation candidate membership changed"));
    }
    let membership = membership(&metadata);
    let encoded = manifest(
        None,
        validator_sha256,
        members(metadata.values().cloned()),
        cmd::object(Vec::new()),
        cmd::object(Vec::new()),
        JsonValue::Array(Vec::new()),
        read_limits,
    )?;
    let object_write_upper_bound_bytes = usize::try_from(unique_object_bytes)
        .map_err(|_| Error::Unsupported("foundation candidate object write range"))?;
    let complete_write_upper_bound = object_write_upper_bound_bytes
        .checked_add(encoded.1.len())
        .filter(|bytes| *bytes <= max_capture_write_bytes)
        .ok_or(Error::Unsupported(
            "foundation candidate capture write budget",
        ))?;
    let (exact_retained_capture_state, candidate_read_path_state, capture_workspace) =
        candidate_capture_state_parts(
            count,
            path_bytes,
            usize::try_from(max_member_bytes)
                .map_err(|_| Error::Unsupported("foundation candidate member byte range"))?,
            encoded.1.len(),
        )
        .ok_or(Error::Unsupported(
            "foundation candidate capture state accounting",
        ))?;
    let exact_metadata_peak = exact_retained_capture_state
        .checked_add(candidate_read_path_state)
        .and_then(|bytes| bytes.checked_add(capture_workspace))
        .ok_or(Error::Unsupported(
            "foundation candidate capture state accounting",
        ))?;
    if exact_metadata_peak > max_state_bytes || complete_write_upper_bound > max_capture_write_bytes
    {
        return Err(Error::Unsupported(
            "foundation candidate capture state/write budget",
        ));
    }
    let candidate_remaining_state = max_state_bytes
        .checked_sub(exact_retained_capture_state)
        .and_then(|bytes| bytes.checked_sub(capture_workspace))
        .ok_or(Error::Unsupported(
            "foundation candidate retained state reservation",
        ))?;
    if candidate_remaining_state < candidate_read_path_state {
        return Err(Error::Unsupported(
            "foundation candidate read-path state reservation",
        ));
    }
    if exact_retained_capture_state != retained_capture_state {
        return Err(Error::Conflict(
            "foundation candidate retained state shape changed",
        ));
    }
    candidate
        // The workspace above coexists only during capture. Keep the
        // candidate's monotone lifetime ceiling at the whole remaining
        // allowance; the checked coexistence bound protects this phase.
        .restrict_remaining_state(max_state_bytes)
        .map_err(candidate_error)?;

    active(deadline, cancel)?;
    candidate.tick().map_err(candidate_error)?;
    let uid = rustix::process::geteuid().as_raw();
    let root = isolated.verify_current(deadline, cancel)?;
    let export = directory(&root, ".foundation-source-cut", uid)?;
    let objects = directory(&export, "objects", uid)?;
    let revisions = directory(&export, "revisions", uid)?;
    let (candidate_revision, encoded_manifest) = encoded;
    let manifest_write_bytes = encoded_manifest.len();
    install_manifest_with_limit(
        &revisions,
        candidate_revision,
        &encoded_manifest,
        uid,
        read_limits.max_manifest_bytes,
        deadline,
        cancel,
    )?;
    drop(encoded_manifest);
    let candidate_io_before = candidate.io_usage();
    for (path, member) in &metadata {
        active(deadline, cancel)?;
        candidate.tick().map_err(candidate_error)?;
        let size = usize::try_from(member.size_bytes)
            .map_err(|_| Error::Unsupported("foundation candidate member byte range"))?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(size)
            .map_err(|_| Error::Unsupported("foundation candidate member allocation"))?;
        let mut sink = CandidateByteSink { bytes, limit: size };
        candidate.copy(path, &mut sink).map_err(candidate_error)?;
        if sink.bytes.len() != size || Digest256::of_bytes(&sink.bytes) != member.sha256 {
            return Err(Error::Conflict("foundation candidate member bytes differ"));
        }
        object_with_limit(
            &objects,
            member.sha256,
            &sink.bytes,
            uid,
            usize::try_from(read_limits.max_selected_object_bytes)
                .map_err(|_| Error::Unsupported("foundation candidate staged byte range"))?,
            deadline,
            cancel,
        )?;
    }
    let candidate_io_after = candidate.io_usage();
    let candidate_copy_read_bytes = candidate_io_after
        .0
        .checked_sub(candidate_io_before.0)
        .and_then(|bytes| usize::try_from(bytes).ok())
        .filter(|bytes| *bytes == total_member_bytes)
        .ok_or(Error::Conflict(
            "foundation candidate copy read accounting differs",
        ))?;
    if candidate_io_after.1 != candidate_io_before.1 {
        return Err(Error::Conflict(
            "foundation candidate copy changed proposal writes",
        ));
    }
    verify_named_export_chain(
        isolated,
        ".foundation-source-cut",
        &export,
        &objects,
        &revisions,
        uid,
        deadline,
        cancel,
    )?;
    let reader =
        CorpusReader::open_existing(&isolated.path().join(".foundation-source-cut"), read_limits)
            .map_err(|_| Error::Invalid("foundation captured reader"))?;
    let cut = reader
        .open_source_cut(candidate_revision, cut_limits, deadline, cancel)
        .map_err(|_| Error::Invalid("foundation captured cut"))?;
    let mut stream = cut
        .stream(candidate_revision)
        .map_err(|_| Error::Invalid("foundation captured stream"))?;
    let mut immutable_member_eof_bytes = 0usize;
    while let Some(member) = stream
        .next_member(deadline, cancel)
        .map_err(|_| Error::Invalid("foundation captured EOF"))?
    {
        immutable_member_eof_bytes = immutable_member_eof_bytes
            .checked_add(member.raw.len())
            .filter(|bytes| *bytes <= total_member_bytes)
            .ok_or(Error::Unsupported(
                "foundation captured EOF read accounting",
            ))?;
    }
    if stream.coverage() != Some(membership) || immutable_member_eof_bytes != total_member_bytes {
        return Err(Error::Conflict(
            "foundation independently captured membership differs",
        ));
    }
    drop(stream);
    if candidate_copy_read_bytes
        .checked_add(immutable_member_eof_bytes)
        .filter(|bytes| *bytes == two_pass_read_bytes)
        .is_none()
    {
        return Err(Error::Conflict(
            "foundation candidate two-pass read accounting differs",
        ));
    }
    Ok(FoundationCapturedCut {
        cut: FoundationCutBacking::Resident(cut),
        membership,
        source_bytes: total_member_bytes as u64,
        metadata: Some(metadata),
        epoch,
        cost: FoundationCaptureCost {
            source_read_bytes: 0,
            source_recheck_bytes: 0,
            immutable_member_eof_bytes,
            candidate_copy_read_bytes: Some(candidate_copy_read_bytes),
            object_write_upper_bound_bytes,
            manifest_write_bytes,
        },
    })
}

/// Keep the capture's file-byte write allowance distinct from its three-pass
/// read allowance. The private stage separately enforces its kernel quota.
#[allow(clippy::too_many_arguments)]
pub(crate) fn capture_bounded_with_write_cap(
    sources: &mut RouteSources,
    isolated: &IsolatedCreationRoot,
    validator_sha256: Digest256,
    read_limits: ReadLimits,
    cut_limits: CutReadLimits,
    max_source_read_bytes: usize,
    max_capture_write_bytes: usize,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<FoundationCapturedCut> {
    active(deadline, cancel)?;
    read_limits
        .validate()
        .map_err(|_| Error::Invalid("foundation source read limits"))?;
    let max_member_pass_bytes = max_source_read_bytes / 3;
    if max_member_pass_bytes == 0 || max_capture_write_bytes == 0 {
        return Err(Error::Unsupported("foundation capture read/write budget"));
    }
    let epoch = MetadataPublicationEpoch::select(state(sources)?)
        .map_err(|_| Error::Conflict("foundation publication selection refused"))?;
    let selected_paths = paths(sources, deadline, cancel)?;
    if selected_paths.len() > read_limits.max_manifest_entries || selected_paths.is_empty() {
        return Err(Error::Unsupported(
            "foundation capture member/manifest bound",
        ));
    }
    let uid = rustix::process::geteuid().as_raw();
    let root = isolated.verify_current(deadline, cancel)?;
    let export = directory(&root, ".foundation-source-cut", uid)?;
    let objects = directory(&export, "objects", uid)?;
    let revisions = directory(&export, "revisions", uid)?;
    let mut metadata = BTreeMap::new();
    let mut read_bytes = 0;
    for path in selected_paths {
        active(deadline, cancel)?;
        let (raw, physical) = sources
            .bounded_metadata_bytes(
                &path,
                read_limits.max_selected_object_bytes.min(MAX_BYTES as u64) as usize,
                &mut read_bytes,
                MAX_BYTES.min(max_member_pass_bytes),
            )
            .map_err(source_error)?;
        if read_bytes > max_capture_write_bytes {
            return Err(Error::Unsupported("foundation capture object write budget"));
        }
        let digest = Digest256::of_bytes(&raw);
        object(&objects, digest, &raw, uid, deadline, cancel)?;
        let relative = RelativePath::parse(&path)
            .map_err(|_| Error::Invalid("foundation authored relative path"))?;
        metadata.insert(
            path,
            MemberMetadata {
                path: relative,
                sha256: digest,
                size_bytes: raw.len() as u64,
                mode: physical.mode() & 0o7777,
            },
        );
    }
    let membership = membership(&metadata);
    let all_member_pass_bytes = read_bytes
        .checked_mul(3)
        .filter(|bytes| *bytes <= max_source_read_bytes)
        .ok_or(Error::Unsupported(
            "foundation capture aggregate member read budget",
        ))?;
    let encoded = manifest(
        None,
        validator_sha256,
        members(metadata.values().cloned()),
        cmd::object(Vec::new()),
        cmd::object(Vec::new()),
        JsonValue::Array(Vec::new()),
        read_limits,
    )?;
    read_bytes
        .checked_add(encoded.1.len())
        .filter(|bytes| *bytes <= max_capture_write_bytes)
        .ok_or(Error::Unsupported(
            "foundation capture manifest write budget",
        ))?;
    install_manifest(&revisions, encoded.0, &encoded.1, uid, deadline, cancel)?;
    verify_named_export_chain(
        isolated,
        ".foundation-source-cut",
        &export,
        &objects,
        &revisions,
        uid,
        deadline,
        cancel,
    )?;
    let reader =
        CorpusReader::open_existing(&isolated.path().join(".foundation-source-cut"), read_limits)
            .map_err(|_| Error::Invalid("foundation captured reader"))?;
    let cut = reader
        .open_source_cut(encoded.0, cut_limits, deadline, cancel)
        .map_err(|_| Error::Invalid("foundation captured cut"))?;
    let mut stream = cut
        .stream(encoded.0)
        .map_err(|_| Error::Invalid("foundation captured stream"))?;
    while stream
        .next_member(deadline, cancel)
        .map_err(|_| Error::Invalid("foundation captured EOF"))?
        .is_some()
    {}
    if stream.coverage() != Some(membership) {
        return Err(Error::Conflict(
            "foundation independently captured membership differs",
        ));
    }
    drop(stream);
    let mut captured = FoundationCapturedCut {
        cut: FoundationCutBacking::Resident(cut),
        membership,
        source_bytes: u64::try_from(read_bytes)
            .map_err(|_| Error::Unsupported("foundation member byte range"))?,
        metadata: Some(metadata),
        epoch,
        cost: FoundationCaptureCost {
            source_read_bytes: read_bytes,
            source_recheck_bytes: 0,
            immutable_member_eof_bytes: read_bytes,
            candidate_copy_read_bytes: None,
            object_write_upper_bound_bytes: read_bytes,
            manifest_write_bytes: encoded.1.len(),
        },
    };
    let remaining = all_member_pass_bytes
        .checked_sub(read_bytes.checked_mul(2).ok_or(Error::Unsupported(
            "foundation capture read accounting overflow",
        ))?)
        .ok_or(Error::Unsupported(
            "foundation capture read accounting underflow",
        ))?;
    captured.cost.source_recheck_bytes =
        captured.recheck_bounded(sources, read_limits, remaining, deadline, cancel)?;
    verify_named_export_chain(
        isolated,
        ".foundation-source-cut",
        &export,
        &objects,
        &revisions,
        uid,
        deadline,
        cancel,
    )?;
    Ok(captured)
}
