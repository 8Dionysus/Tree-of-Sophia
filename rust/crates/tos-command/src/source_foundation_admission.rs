//! Admission adapter over the same protected invocation and complete FND path.
//! No public proof constructor, alternate validator, or bootstrap clock exists.
use crate::source_admission::{AdmissionLimits, active, invalid};
use crate::source_admission_candidate::{Candidate, CandidateLimits};
use crate::source_admission_index::{self, CandidateInput, Index, IndexLimits};
use crate::source_admission_spooled_candidate::{SpoolCandidate, SpoolLimits};
use crate::source_admission_spooled_index::{CandidateRecordsReportVerified, SpoolIndexLimits};
use crate::source_admission_spooled_manifest::ManifestStreamLimits;
use crate::source_creation_store::IsolatedCreationRoot;
use crate::source_current_cut::foundation_cli::ValidationProfile;
use crate::source_current_cut::{
    foundation_bootstrap::{FoundationBootstrapInputs, verify_invocation_with_budget},
    foundation_entry::{
        self, FoundationBootstrapClock, FoundationInvocation, FoundationLaunchArguments,
    },
    foundation_execution_limits::{
        FoundationCharge, FoundationPhaseReservation, FoundationPhaseUse, FoundationRemainingBudget,
    },
    foundation_orchestrator,
    foundation_reader::FoundationHistoricalEvidence,
};
use crate::source_foundation_admission_history::{
    HistoryEvidence, HistoryLimits, HistorySelection,
};
use crate::source_foundation_admission_identity::{GrammarIdentity, IdentityLimits};
use serde_json::Value;
use std::{
    ffi::OsString,
    fs::File,
    io::{self, Write},
    mem::size_of,
    os::unix::{ffi::OsStrExt, fs::MetadataExt},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicI32},
    },
    time::Instant,
};
use tos_compiler::private_tmpfs_stage::{
    PRIVATE_TMPFS_SELECT_COST, PRIVATE_TMPFS_VERIFY_COST, PrivateTmpfsStageIsolation,
};
use tos_foundation::{Digest256, JsonLimits};
use tos_ops_mechanics_plan::route_cards::RouteSources;
use tos_segment_store::{AuthenticatedTreeLimitsV1, SegmentLimits};
use tos_source_store::ReadLimits;
use tos_source_store::{
    CutReadLimits, PinnedSqliteAuxLimits, PinnedSqliteAuxRequest, PinnedSqliteIoBudget,
    PinnedSqliteIoFailure, PinnedSqliteIoSnapshot, PinnedSqliteSpaceBudget,
    StreamedCutReadLimitsV1,
};

/// Only this typed error carries path-free owner refusal text. Other IO errors
/// may contain private paths or diagnostics and are not printed.
#[derive(Debug)]
pub(crate) struct NativeValidationRefusal(pub(crate) String);
impl NativeValidationRefusal {
    pub(crate) const MAX_PUBLIC_REASON_BYTES: usize = 192;
}
impl std::fmt::Display for NativeValidationRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for NativeValidationRefusal {}

/// The invocation joins two distinct read authorities. Keep their raw
/// snapshots beside aggregate counters; an aggregate has no single ceiling.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct NativeInvocationIoSnapshot {
    aggregate: PinnedSqliteIoSnapshot,
    spool: PinnedSqliteIoSnapshot,
    segment_v2: Option<PinnedSqliteIoSnapshot>,
}

/// Pre-publication refusal owns its original cause before terminal accounting.
/// Only the reviewed validator reason and fixed operation labels reach output.
#[derive(Debug)]
pub(crate) struct NativeSpoolRefusal {
    primary: io::Error,
    phase: &'static str,
    primary_io: NativeInvocationIoSnapshot,
    terminal_io: NativeInvocationIoSnapshot,
    accounting_failed: bool,
    cleanup_failed: bool,
    output_failed: bool,
}
impl std::fmt::Display for NativeSpoolRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("native spooled admission refused")
    }
}
impl std::error::Error for NativeSpoolRefusal {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.primary)
    }
}
#[derive(serde::Serialize)]
pub(crate) struct NativeSpoolRefusalPacket<'a> {
    schema_version: &'static str,
    publication_state: &'static str,
    phase: &'static str,
    primary_error_kind: &'static str,
    primary_error_sha256: Option<String>,
    native_validation_reason: Option<&'a str>,
    primary_io: NativeSpoolIoPacket,
    terminal_io: NativeSpoolIoPacket,
    accounting_failed: bool,
    cleanup_failed: bool,
    output_failed: bool,
}
#[derive(serde::Serialize)]
struct NativeSpoolIoPacket {
    read_attempted_bytes: u64,
    read_upper_bound_attempted_bytes: u64,
    read_permitted_bytes: u64,
    read_returned_bytes: u64,
    write_attempted_bytes: u64,
    write_permitted_bytes: u64,
    write_returned_bytes: u64,
    first_failure: Option<&'static str>,
    spool_reads: NativeSpoolReadPacket,
    segment_v2_reads: Option<NativeSpoolReadPacket>,
}
#[derive(serde::Serialize)]
struct NativeSpoolReadPacket {
    limit_bytes: Option<u64>,
    limit_source: Option<&'static str>,
    limit_line: Option<u32>,
    attempted_bytes: u64,
    upper_bound_attempted_bytes: u64,
    permitted_bytes: u64,
    returned_bytes: u64,
    local_failure: Option<&'static str>,
    effective_failure: Option<&'static str>,
}
fn io_failure_name(failure: PinnedSqliteIoFailure) -> &'static str {
    match failure {
        PinnedSqliteIoFailure::ReadLimit => "read_limit",
        PinnedSqliteIoFailure::WriteLimit => "write_limit",
        PinnedSqliteIoFailure::Deadline => "deadline",
        PinnedSqliteIoFailure::Cancelled => "cancelled",
        PinnedSqliteIoFailure::FileLimit => "file_limit",
        PinnedSqliteIoFailure::SpaceLimit => "space_limit",
        PinnedSqliteIoFailure::Io => "io",
    }
}
impl From<PinnedSqliteIoSnapshot> for NativeSpoolReadPacket {
    fn from(value: PinnedSqliteIoSnapshot) -> Self {
        Self {
            limit_bytes: value.read_limit_bytes,
            // Rust source basename only: never emit an absolute build path.
            limit_source: value
                .read_limit_origin
                .and_then(|origin| origin.file().rsplit('/').next()),
            limit_line: value.read_limit_origin.map(|origin| origin.line()),
            attempted_bytes: value.read_attempted_bytes,
            upper_bound_attempted_bytes: value.read_upper_bound_attempted_bytes,
            permitted_bytes: value.read_permitted_bytes,
            returned_bytes: value.read_returned_bytes,
            local_failure: value.local_failure.map(io_failure_name),
            effective_failure: value.failure.map(io_failure_name),
        }
    }
}
impl From<NativeInvocationIoSnapshot> for NativeSpoolIoPacket {
    fn from(value: NativeInvocationIoSnapshot) -> Self {
        let aggregate = value.aggregate;
        Self {
            read_attempted_bytes: aggregate.read_attempted_bytes,
            read_upper_bound_attempted_bytes: aggregate.read_upper_bound_attempted_bytes,
            read_permitted_bytes: aggregate.read_permitted_bytes,
            read_returned_bytes: aggregate.read_returned_bytes,
            write_attempted_bytes: aggregate.write_attempted_bytes,
            write_permitted_bytes: aggregate.write_permitted_bytes,
            write_returned_bytes: aggregate.write_returned_bytes,
            first_failure: aggregate.failure.map(io_failure_name),
            spool_reads: value.spool.into(),
            segment_v2_reads: value.segment_v2.map(Into::into),
        }
    }
}
// Correlate an opaque IO cause without exporting private paths or text.
// Formatting is bounded and incomplete output never produces a fingerprint.
fn bounded_error_sha256(error: &io::Error) -> Option<String> {
    struct Fingerprint {
        hash: tos_foundation::Digest256Hasher,
        remaining: usize,
    }
    impl std::fmt::Write for Fingerprint {
        fn write_str(&mut self, text: &str) -> std::fmt::Result {
            self.remaining = self
                .remaining
                .checked_sub(text.len())
                .ok_or(std::fmt::Error)?;
            self.hash.update(text.as_bytes());
            Ok(())
        }
    }
    let mut sink = Fingerprint {
        hash: tos_foundation::Digest256Hasher::new(),
        remaining: 4096,
    };
    std::fmt::write(&mut sink, format_args!("{error}")).ok()?;
    let digest = sink.hash.finalize().to_hex();
    Some(digest)
}
impl NativeSpoolRefusal {
    pub(crate) fn retain(
        primary: io::Error,
        phase: &'static str,
        primary_io: NativeInvocationIoSnapshot,
        terminal_io: NativeInvocationIoSnapshot,
        accounting_failed: bool,
        cleanup_failed: bool,
    ) -> Self {
        // Keep the first phase/cause/snapshot when the CLI adds terminal checks.
        if primary.get_ref().is_some_and(|cause| cause.is::<Self>()) {
            let cause = primary.into_inner().expect("checked typed spool refusal");
            let mut refusal = *cause.downcast::<Self>().expect("checked spool owner");
            refusal.terminal_io = terminal_io;
            refusal.accounting_failed |= accounting_failed;
            refusal.cleanup_failed |= cleanup_failed;
            return refusal;
        }
        Self {
            primary,
            phase,
            primary_io,
            terminal_io,
            accounting_failed,
            cleanup_failed,
            output_failed: false,
        }
    }
    pub(crate) fn with_output_refused(mut self) -> Self {
        self.output_failed = true;
        self
    }
    pub(crate) fn packet(&self) -> NativeSpoolRefusalPacket<'_> {
        let reason = self
            .primary
            .get_ref()
            .and_then(|cause| cause.downcast_ref::<NativeValidationRefusal>())
            .map(|reason| reason.0.as_str())
            .filter(|reason| reason.len() <= NativeValidationRefusal::MAX_PUBLIC_REASON_BYTES);
        NativeSpoolRefusalPacket {
            schema_version: "tos_native_spooled_admission_refusal_v1",
            publication_state: "not_committed",
            phase: self.phase,
            primary_error_kind: match self.primary.kind() {
                io::ErrorKind::InvalidData => "invalid_data",
                io::ErrorKind::InvalidInput => "invalid_input",
                io::ErrorKind::PermissionDenied => "permission_denied",
                io::ErrorKind::TimedOut => "timed_out",
                io::ErrorKind::Interrupted => "interrupted",
                io::ErrorKind::NotFound => "not_found",
                io::ErrorKind::AlreadyExists => "already_exists",
                _ => "other",
            },
            primary_error_sha256: bounded_error_sha256(&self.primary),
            native_validation_reason: reason,
            primary_io: self.primary_io.into(),
            terminal_io: self.terminal_io.into(),
            accounting_failed: self.accounting_failed,
            cleanup_failed: self.cleanup_failed,
            output_failed: self.output_failed,
        }
    }
}

/// Unforgeable-in-crate completion witness consumed by the spooled index
/// adapter. Its constructor remains inside this validator module so a caller
/// cannot turn an index-only pass into a validated source view.
pub(crate) struct NativeAdmissionComplete {
    records: CandidateRecordsReportVerified,
    index_profile: SpoolIndexLimits,
    original_io: PinnedSqliteIoBudget,
    original_epoch: tos_source_store::MetadataPublicationEpoch,
    original_workspace: File,
    original_space: PinnedSqliteSpaceBudget,
    segment_v2: Option<NativeSegmentV2Budget>,
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
}
impl NativeAdmissionComplete {
    // Called only after the whole candidate route and final guards succeed.
    // The report is moved from the authenticated index callback, never rebuilt.
    fn new(
        records: CandidateRecordsReportVerified,
        index_profile: SpoolIndexLimits,
        original_io: &PinnedSqliteIoBudget,
        original_epoch: tos_source_store::MetadataPublicationEpoch,
        original_workspace: File,
        original_space: PinnedSqliteSpaceBudget,
        segment_v2: Option<NativeSegmentV2Budget>,
        deadline: Instant,
        cancelled: Arc<AtomicBool>,
    ) -> Self {
        Self {
            records,
            index_profile,
            original_io: original_io.clone(),
            original_epoch,
            original_workspace,
            original_space,
            segment_v2,
            deadline,
            cancelled,
        }
    }
    pub(crate) fn original_workspace(&self) -> &File {
        &self.original_workspace
    }
    pub(crate) fn original_space(&self) -> &PinnedSqliteSpaceBudget {
        &self.original_space
    }
    pub(crate) fn deadline(&self) -> Instant {
        self.deadline
    }
    pub(crate) fn cancelled(&self) -> &Arc<AtomicBool> {
        &self.cancelled
    }
    pub(crate) fn original_epoch(&self) -> &tos_source_store::MetadataPublicationEpoch {
        &self.original_epoch
    }
    pub(crate) fn records(&self) -> &CandidateRecordsReportVerified {
        &self.records
    }
    pub(crate) fn index_profile(&self) -> SpoolIndexLimits {
        self.index_profile
    }
    pub(crate) fn original_io(&self) -> &PinnedSqliteIoBudget {
        &self.original_io
    }
    pub(crate) fn segment_v2(&self) -> Option<&NativeSegmentV2Budget> {
        self.segment_v2.as_ref()
    }
}

/// Explicit invocation-scoped incremental persistent and tree-IO profile.
/// These ledgers are independent of the TMPFS SQLite workspace and are carried
/// only by the authentic NativeAdmissionComplete token.
#[derive(Clone)]
pub(crate) struct NativeSegmentV2Budget {
    pub(crate) max_allocated_bytes: u64,
    pub(crate) max_frame_bytes: u64,
    pub(crate) allocation_space: PinnedSqliteSpaceBudget,
    pub(crate) allocation_reservation: Arc<tos_source_store::PinnedSqliteSpaceReservation>,
    pub(crate) allocation_unit_bytes: u64,
    pub(crate) allocation_accountant: Arc<super::source_admission_segment_v2::NativeV2TreeIo>,
    pub(crate) io: PinnedSqliteIoBudget,
    pub(crate) max_working_state_bytes: usize,
    pub(crate) tree_limits: AuthenticatedTreeLimitsV1,
}

fn command(error: crate::source_command::SourceCommandError) -> io::Error {
    // Only explicitly reviewed static owner reasons may cross the installed
    // CLI boundary. In particular, SchemaExecution carries private paths.
    if matches!(
        &error,
        crate::source_command::SourceCommandError::Denied(
            "configuration ancestor ownership/write boundary"
        )
    ) {
        return io::Error::new(
            io::ErrorKind::PermissionDenied,
            NativeValidationRefusal("protected invocation ancestor ownership boundary".into()),
        );
    }
    invalid(format!("native admission foundation boundary: {error:?}"))
}
fn count(value: u64) -> io::Result<usize> {
    usize::try_from(value).map_err(|_| invalid("admission limit exceeds address space"))
}
struct Prepared<'c> {
    clock: FoundationBootstrapClock,
    launch: FoundationLaunchArguments,
    invocation: FoundationInvocation<'c>,
    ledger: FoundationRemainingBudget<'c>,
    sources: RouteSources,
}

/// Resource selection is made by the protected invocation. Missing selection
/// means resident-v1; native-v4 also requires the original shared cancellation
/// owner and a finite profile from that invocation's remaining limits.
pub(crate) enum PreparedAdmissionExecution {
    Resident,
    Spooled(PreparedSpooledExecution),
}

pub(crate) struct PreparedSpooledExecution {
    pub(crate) workspace_root: IsolatedCreationRoot,
    pub(crate) workspace: File,
    pub(crate) request: PinnedSqliteAuxRequest,
    pub(crate) candidate_limits: SpoolLimits,
    pub(crate) index_limits: SpoolIndexLimits,
    pub(crate) defaults_limits: SpoolIndexLimits,
    pub(crate) manifest_limits: ManifestStreamLimits,
    pub(crate) streamed_cut_limits: StreamedCutReadLimitsV1,
    pub(crate) max_index_allocated_bytes: u64,
    pub(crate) max_manifest_allocated_bytes: u64,
    pub(crate) v2_case: Option<foundation_entry::FoundationV2CaseSelection>,
    pub(crate) v2_source_root: Option<PreparedV2StoreRoot>,
    pub(crate) v2_target_root: Option<PreparedV2ArtifactRoot>,
    pub(crate) v2_allocation_accountant:
        Option<Arc<super::source_admission_segment_v2::NativeV2TreeIo>>,
    /// Numeric metadata-only reader profile selected before the native
    /// completion witness. It carries no publication or allocation authority.
    pub(crate) v2_base_read_limits: Option<super::source_admission_v2_reader::V2PointReadLimits>,
}

pub(crate) struct PreparedV2StoreRoot {
    pub(crate) path: PathBuf,
    pub(crate) held: File,
    pub(crate) identity: (u64, u64),
}

pub(crate) struct PreparedV2ArtifactRoot {
    pub(crate) path: PathBuf,
    pub(crate) held: File,
    pub(crate) identity: (u64, u64),
}

// The existing IO control envelope is charged before either the early issuer
// or ordinary spooled factory allocates its original shared control blocks.
const SPOOLED_IO_CONTROL_STATE_BYTES: usize = 4096;

struct GeneratedInputPrelude {
    declaration: Option<crate::source_admission_indexed_input::HeldIndexedInputDeclarationV1>,
    io: PinnedSqliteIoBudget,
    aggregate_write: PinnedSqliteIoBudget,
    identity_binding: Option<String>,
}

pub(crate) struct NativeSourceValidator<'c> {
    prepared: Option<Prepared<'c>>,
    evaluated: Option<FoundationBootstrapInputs<'c>>,
    grammar: GrammarIdentity,
    validation_profile: ValidationProfile,
    record_selection: Option<Arc<tos_validation::source_record_selection::SourceRecordSelection>>,
    record_selection_held: Option<crate::source_text_owner::HeldOwnerFile>,
    record_selection_binding: Option<Value>,
    generated_input: Option<GeneratedInputPrelude>,
    history: Option<HistoryEvidence>,
    history_usage: (u64, usize),
    identity: Digest256,
    deadline: Instant,
    cancel: &'c AtomicBool,
    git_signal: &'c AtomicI32,
    write_cap: u64,
    candidate_io: (u64, u64),
    candidate_state: usize,
    batch_charged: bool,
    store_authority: Option<PrivateTmpfsStageIsolation>,
    shared_cancel: Option<Arc<AtomicBool>>,
    spooled_route_selected: bool,
    execution_resources_taken: bool,
    // Exact protected profiles and original shared IO authority retained for
    // the whole borrowed validator after the resource DTO moves to its caller.
    spooled_profile: Option<(SpoolIndexLimits, SpoolIndexLimits, PinnedSqliteIoBudget)>,
    spooled_workspace: Option<(File, PinnedSqliteSpaceBudget)>,
    // Declared after the retained workspace FD; the root also holds this Arc.
    _spooled_persistent_root_allocation:
        Option<Arc<tos_source_store::PinnedSqliteSpaceReservation>>,
    segment_v2_profile: Option<NativeSegmentV2Budget>,
    segment_v2_io_accounted: (u64, u64),
    segment_v2_read_upper_accounted: u64,
    spooled_read_upper_accounted: u64,
    spooled_write_cap: Option<u64>,
}
/// The only constructor is the successful complete owner callback below.
pub(crate) struct ValidatedCandidate(Index);
impl ValidatedCandidate {
    pub(crate) fn into_index(self) -> Index {
        self.0
    }
}
fn debit(
    ledger: &mut FoundationRemainingBudget<'_>,
    label: &'static str,
    read: u64,
    state: usize,
) -> io::Result<()> {
    let ticket = ledger
        .begin_window(label, FoundationPhaseReservation::default())
        .map_err(command)?;
    ledger
        .complete_window(
            ticket,
            FoundationPhaseUse {
                source_read_bytes: FoundationCharge::measured(read),
                state_bytes: FoundationCharge::admitted_upper_bound(state),
                ..FoundationPhaseUse::default()
            },
        )
        .map_err(command)
}

fn debit_spooled_profile(
    ledger: &mut FoundationRemainingBudget<'_>,
    read: u64,
    state: usize,
    tmpfs_bytes: u64,
    tmpfs_inodes: u64,
) -> io::Result<()> {
    let ticket = ledger
        .begin_window(
            "admission-spooled-profile",
            FoundationPhaseReservation::default(),
        )
        .map_err(command)?;
    ledger
        .complete_window(
            ticket,
            FoundationPhaseUse {
                source_read_bytes: FoundationCharge::admitted_upper_bound(read),
                state_bytes: FoundationCharge::admitted_upper_bound(state),
                tmpfs_bytes: FoundationCharge::admitted_upper_bound(tmpfs_bytes),
                tmpfs_inodes: FoundationCharge::admitted_upper_bound(tmpfs_inodes),
                ..FoundationPhaseUse::default()
            },
        )
        .map_err(command)
}

fn sqlite_aux_limits(partition: u64, max_live_aux: usize) -> Option<PinnedSqliteAuxLimits> {
    let main = partition / 2;
    let aux = partition / 8;
    if main == 0 || aux == 0 || max_live_aux == 0 || max_live_aux == usize::MAX {
        return None;
    }
    Some(PinnedSqliteAuxLimits {
        main_logical_bytes: main,
        main_allocated_bytes: main,
        temp_db_logical_bytes: aux,
        temp_db_allocated_bytes: aux,
        main_journal_logical_bytes: aux,
        main_journal_allocated_bytes: aux,
        temp_journal_logical_bytes: aux,
        temp_journal_allocated_bytes: aux,
        other_aux_aggregate_logical_bytes: aux,
        other_aux_aggregate_allocated_bytes: aux,
        max_live_aux,
    })
}

impl<'c> NativeSourceValidator<'c> {
    /// Borrow the invocation's already-selected V2 physical read/copy bounds
    /// for a protected observation case, including recovery of an accepted
    /// result. This is numeric profile access only: it does not mint a
    /// `NativeAdmissionComplete` witness, grant publication authority, or
    /// authorize a successor write. Publication still requires the genuine
    /// completed index issued by the native validation path.
    pub(crate) fn prepared_v2_read_case_profile(&self) -> io::Result<&NativeSegmentV2Budget> {
        self.segment_v2_profile
            .as_ref()
            .ok_or_else(|| invalid("prepared V2 read/copy profile absent"))
    }

    pub(crate) fn prepare(
        clock: FoundationBootstrapClock,
        args: &[OsString],
        cancel: &'c AtomicBool,
        git_signal: &'c AtomicI32,
        select_output: impl FnOnce(u64, Instant),
    ) -> io::Result<Self> {
        let mut foundation_args = Vec::new();
        let mut captures = Vec::new();
        let mut restored_roots = Vec::new();
        let mut position = 0;
        while position < args.len() {
            let option = &args[position];
            position += 1;
            if option == "--historical-capture" || option == "--historical-root" {
                let selected = args
                    .get(position)
                    .ok_or_else(|| invalid("historical evidence selection requires a path"))?;
                position += 1;
                if option == "--historical-capture" {
                    captures.push(PathBuf::from(selected));
                } else {
                    restored_roots.push(PathBuf::from(selected));
                }
            } else {
                foundation_args.push(option.clone());
            }
        }
        if captures.len() != restored_roots.len() {
            return Err(invalid("historical captures require paired restored roots"));
        }
        let launch = foundation_entry::parse_launch_arguments(&foundation_args).map_err(command)?;
        let invocation =
            foundation_entry::read_invocation(&clock, &launch, cancel).map_err(command)?;
        select_output(invocation.budgets.max_output_bytes, invocation.deadline());
        if launch.arguments.help || launch.arguments.selected_lab.is_some() {
            return Err(invalid(
                "admission requires complete default foundation validation",
            ));
        }
        // Identity-only selection does not create or write a corpus store.
        // Candidate construction below requires the explicit write authority.
        let write_cap = invocation.budgets.max_admission_write_bytes.unwrap_or(0);
        let deadline = invocation.deadline();
        let mut ledger =
            FoundationRemainingBudget::from_invocation(&invocation).map_err(command)?;
        let entry_read = invocation
            .cost
            .invocation_read_bytes
            .checked_add(invocation.cost.self_image_read_bytes)
            .ok_or_else(|| invalid("admission bootstrap read overflow"))?;
        debit(&mut ledger, "admission-entry", entry_read, 0)?;
        let selected = invocation.selected_roots(&launch).map_err(command)?;
        let mut sources = RouteSources::new_until(Path::new(&selected.repo_root), deadline)?;
        let (record_selection, record_selection_held, record_selection_binding) =
            if let Some(path) = &launch.arguments.record_selection_manifest {
                use std::os::unix::ffi::OsStrExt;
                use tos_validation::source_record_selection::{
                    SelectionLimits, SourceRecordSelection, selection_state_upper_bound,
                };
                let remaining = ledger.remaining().map_err(command)?;
                let custody_bound = path
                    .as_os_str()
                    .as_bytes()
                    .len()
                    .checked_mul(3)
                    .and_then(|n| {
                        n.checked_add(size_of::<crate::source_text_owner::HeldOwnerFile>())
                            .and_then(|n| {
                                n.checked_add(
                                    size_of::<Arc<SourceRecordSelection>>()
                                        + 2 * size_of::<usize>(),
                                )
                            })
                    })
                    .ok_or_else(|| invalid("record selection custody state overflow"))?;
                let model_state = remaining
                    .state_bytes
                    .checked_sub(custody_bound)
                    .ok_or_else(|| invalid("record selection custody state bound"))?;
                let mut low = 0usize;
                let mut high = count(invocation.budgets.max_member_bytes)?
                    .min(count(remaining.source_read_bytes / 2)?);
                // Invert the model owner's existing simultaneous state bound;
                // no copied parser ratio or physical-file/row count assumption.
                while low < high {
                    let middle = low + (high - low).div_ceil(2);
                    if selection_state_upper_bound(middle).is_ok_and(|state| state < model_state) {
                        low = middle;
                    } else {
                        high = middle - 1;
                    }
                }
                if low == 0 {
                    return Err(invalid("record selection manifest budget"));
                }
                let ticket = ledger
                    .begin_window(
                        "record-selection-manifest-read",
                        FoundationPhaseReservation::default(),
                    )
                    .map_err(command)?;
                ledger
                    .complete_window(
                        ticket,
                        FoundationPhaseUse {
                            source_read_bytes: FoundationCharge::admitted_upper_bound(
                                (low as u64)
                                    .checked_mul(2)
                                    .ok_or_else(|| invalid("record selection read overflow"))?,
                            ),
                            ..FoundationPhaseUse::default()
                        },
                    )
                    .map_err(command)?;
                let (held, raw) = crate::source_text_owner::select_held_file(
                    path,
                    unsafe { libc::geteuid() },
                    false,
                    low,
                    deadline,
                    cancel,
                )
                .map_err(command)?;
                let retained = selection_state_upper_bound(raw.len())
                    .map_err(|_| invalid("record selection model state"))?;
                let verify_state = model_state
                    .checked_sub(retained)
                    .filter(|n| *n > 0)
                    .ok_or_else(|| invalid("record selection verification state"))?;
                let selection = SourceRecordSelection::parse(
                    &raw,
                    SelectionLimits {
                        max_manifest_bytes: low,
                        max_records: raw.len(),
                        max_slots: raw.len(),
                        max_roots: raw.len(),
                        max_owned_state_bytes: model_state,
                        max_row_bytes: count(invocation.budgets.max_member_bytes)?,
                        max_verify_state_bytes: verify_state,
                    },
                )
                .map_err(|error| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        NativeValidationRefusal(
                            crate::source_admission_spooled_index::receiver_refusal(error)
                                .to_string(),
                        ),
                    )
                })?;
                let binding = selection.binding().map_err(|error| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        NativeValidationRefusal(
                            crate::source_admission_spooled_index::receiver_refusal(error)
                                .to_string(),
                        ),
                    )
                })?;
                let state = selection
                    .charged_state_bytes()
                    .checked_add(
                        held.retained_state_bytes()
                            .ok_or_else(|| invalid("record selection custody state overflow"))?,
                    )
                    .and_then(|n| {
                        n.checked_add(
                            size_of::<Arc<SourceRecordSelection>>() + 2 * size_of::<usize>(),
                        )
                    })
                    .ok_or_else(|| invalid("record selection state overflow"))?;
                drop(raw);
                debit(&mut ledger, "record-selection-retained-state", 0, state)?;
                (Some(Arc::new(selection)), Some(held), Some(binding))
            } else {
                (None, None, None)
            };
        let mut generated_input = if let Some(root) = &launch.arguments.indexed_input_root {
            if !matches!(
                invocation.admission_representation(),
                foundation_entry::FoundationAdmissionRepresentation::NativeV4SegmentV2
            ) {
                return Err(invalid(
                    "generated record closure requires native-v4-segment-v2",
                ));
            }
            let selection = record_selection
                .as_ref()
                .ok_or_else(|| invalid("generated input requires finite authored selection"))?;
            let meter_state = SPOOLED_IO_CONTROL_STATE_BYTES.checked_add(crate::source_admission::AdmissionWorkBudget::retained_allocation_upper_bound_bytes())
                .ok_or_else(|| invalid("generated input original meter state overflow"))?;
            debit(
                &mut ledger,
                "generated-input-original-meters",
                0,
                meter_state,
            )?;
            let remaining = ledger.remaining().map_err(command)?;
            let aggregate_write = PinnedSqliteIoBudget::new(remaining.source_read_bytes, write_cap)
                .map_err(invalid)?;
            let io = PinnedSqliteIoBudget::new_with_shared_write_authority(
                remaining.source_read_bytes,
                write_cap,
                aggregate_write.clone(),
            )
            .map_err(invalid)?;
            // The maintained initial-cut selector uses the original read ceiling
            // as its work authority. Its later phase selection only narrows this
            // same owner; every early descriptor/aux row remains in the prefix.
            let work =
                crate::source_admission::AdmissionWorkBudget::new(remaining.source_read_bytes)?;
            let declaration =
                crate::source_admission_indexed_input::open_indexed_input_declaration_v1(
                    root,
                    count(invocation.budgets.max_member_bytes)?
                        .min(crate::source_admission_indexed_input::DESCRIPTOR_MAX_BYTES),
                    count(invocation.budgets.max_member_bytes)?
                        .min(crate::source_admission_indexed_input::PROFILE_SIDECAR_MAX_BYTES_V1),
                    &io,
                    deadline,
                    cancel,
                    &work,
                    selection,
                    remaining.state_bytes,
                );
            let usage = io.snapshot();
            let measured = usage
                .read_attempted_bytes
                .checked_sub(usage.read_upper_bound_attempted_bytes)
                .ok_or_else(|| invalid("generated input attempted read classification"))?;
            ledger
                .record_terminal_source_read_suffix(
                    measured,
                    usage.read_upper_bound_attempted_bytes,
                )
                .map_err(command)?;
            let declaration = declaration?;
            debit(
                &mut ledger,
                "generated-input-held-declaration",
                0,
                declaration.retained_state_bytes_v1()?,
            )?;
            Some(GeneratedInputPrelude {
                declaration: Some(declaration),
                io,
                aggregate_write,
                identity_binding: None,
            })
        } else {
            None
        };
        let remaining = ledger.remaining().map_err(command)?;
        let limits = IdentityLimits {
            max_read_bytes: remaining.source_read_bytes,
            max_member_bytes: count(invocation.budgets.max_member_bytes)?,
            max_members: count(invocation.budgets.max_current_members)?,
            max_discovery_entries: count(invocation.budgets.max_current_members)?,
            max_state_bytes: remaining.state_bytes,
        };
        let validation_profile = launch.arguments.validation_profile;
        if validation_profile.scope != tos_validation::source_foundation_default_rules::SourceFoundationDefaultRuleScope::FullAudit
            && !matches!(invocation.admission_representation(), foundation_entry::FoundationAdmissionRepresentation::NativeV4 | foundation_entry::FoundationAdmissionRepresentation::NativeV4SegmentV2) {
            return Err(invalid("selected-source validation requires native-v4 admission"));
        }
        let mut grammar = GrammarIdentity::select(
            &mut sources,
            invocation.executable_sha256(),
            invocation.schema_worker.sha256,
            limits,
            deadline,
            cancel,
            validation_profile,
        )?;
        debit(
            &mut ledger,
            "admission-grammar-identity",
            grammar.read_bytes(),
            grammar.retained_state_bytes(),
        )?;
        if let Some(binding) = &record_selection_binding {
            grammar.bind_record_selection(
                binding,
                ledger.remaining().map_err(command)?.state_bytes,
                cancel,
            )?;
        }
        if let Some(early) = &mut generated_input {
            let binding = grammar.bind_generated_input(
                early.declaration.as_ref().expect("generated issuer held"),
                ledger.remaining().map_err(command)?.state_bytes,
                cancel,
            )?;
            let binding = String::from_utf8(binding).map_err(invalid)?;
            debit(
                &mut ledger,
                "generated-input-identity-binding",
                0,
                binding
                    .capacity()
                    .checked_add(size_of::<String>())
                    .ok_or_else(|| invalid("generated binding retained state overflow"))?,
            )?;
            early.identity_binding = Some(binding);
        }
        let history = if captures.is_empty() {
            None
        } else {
            let pairs: Vec<_> = captures
                .into_iter()
                .zip(restored_roots)
                .map(|(capture, restored)| HistorySelection { capture, restored })
                .collect();
            let available = ledger.remaining().map_err(command)?;
            let caps = invocation.budgets;
            let member_bytes = count(caps.max_member_bytes.min(available.source_read_bytes))?;
            let mut json = JsonLimits::default();
            json.max_bytes = member_bytes;
            let history = HistoryEvidence::select(
                &sources,
                &pairs,
                HistoryLimits {
                    metadata: ReadLimits {
                        max_manifest_bytes: member_bytes,
                        max_manifest_entries: member_bytes.min(json.max_visits),
                        max_selected_object_bytes: caps
                            .max_member_bytes
                            .min(available.source_read_bytes),
                        json,
                    },
                    max_packs: count(caps.max_current_members)?,
                    max_read_bytes: available.source_read_bytes,
                    max_state_bytes: available.state_bytes,
                    max_member_bytes: member_bytes,
                },
                deadline,
                cancel,
            )?;
            debit(
                &mut ledger,
                "admission-historical-selection",
                history.read_bytes(),
                history.retained_state_bytes(),
            )?;
            Some(history)
        };
        let identity = match &history {
            Some(history) => grammar.bind_history(
                history.original_rows(),
                ledger.remaining().map_err(command)?.state_bytes,
                cancel,
            )?,
            None => grammar.digest(),
        };
        let history_usage = history
            .as_ref()
            .map(|h| (h.read_bytes(), h.retained_state_bytes()))
            .unwrap_or_default();
        let spooled_route_selected = matches!(
            invocation.admission_representation(),
            foundation_entry::FoundationAdmissionRepresentation::NativeV4
                | foundation_entry::FoundationAdmissionRepresentation::NativeV4SegmentV2
        );
        Ok(Self {
            prepared: Some(Prepared {
                clock,
                launch,
                invocation,
                ledger,
                sources,
            }),
            evaluated: None,
            grammar,
            validation_profile,
            record_selection,
            record_selection_held,
            record_selection_binding,
            history,
            history_usage,
            identity,
            deadline,
            cancel,
            git_signal,
            write_cap,
            candidate_io: generated_input
                .as_ref()
                .map(|early| {
                    let usage = early.io.snapshot();
                    (usage.read_attempted_bytes, usage.write_attempted_bytes)
                })
                .unwrap_or((0, 0)),
            candidate_state: 0,
            batch_charged: false,
            store_authority: None,
            shared_cancel: None,
            spooled_route_selected,
            execution_resources_taken: false,
            spooled_profile: None,
            spooled_workspace: None,
            _spooled_persistent_root_allocation: None,
            segment_v2_profile: None,
            segment_v2_io_accounted: (0, 0),
            segment_v2_read_upper_accounted: 0,
            spooled_read_upper_accounted: generated_input
                .as_ref()
                .map(|early| early.io.snapshot().read_upper_bound_attempted_bytes)
                .unwrap_or(0),
            generated_input,
            spooled_write_cap: None,
        })
    }

    /// Shared CLI entry retains the caller's original cancellation Arc. This
    /// cannot synthesize shared cancellation from the legacy borrowed flag.
    pub(crate) fn prepare_shared_cancel(
        clock: FoundationBootstrapClock,
        args: &[OsString],
        cancelled: &'c Arc<AtomicBool>,
        git_signal: &'c AtomicI32,
        select_output: impl FnOnce(u64, Instant),
    ) -> io::Result<Self> {
        let mut prepared =
            Self::prepare(clock, args, cancelled.as_ref(), git_signal, select_output)?;
        if !std::ptr::eq(prepared.cancel, Arc::as_ptr(cancelled)) {
            return Err(invalid("shared admission cancellation identity changed"));
        }
        prepared.shared_cancel = Some(Arc::clone(cancelled));
        Ok(prepared)
    }
    /// Identity-only and unchanged admissions do not enter Host. They still
    /// finalize the same protected invocation and held grammar before output.
    pub(crate) fn finalize_without_evaluation(&mut self) -> io::Result<()> {
        let prepared = self
            .prepared
            .as_mut()
            .ok_or_else(|| invalid("admission fast path has no prepared invocation"))?;
        verify_invocation_with_budget(&mut prepared.invocation, &mut prepared.ledger)
            .map_err(|_| invalid("admission final invocation custody refused"))?;
        let remaining = prepared.ledger.remaining().map_err(command)?;
        let read = self.grammar.recheck(
            &mut prepared.sources,
            remaining.source_read_bytes,
            remaining.state_bytes,
            self.cancel,
        )?;
        debit(&mut prepared.ledger, "admission-grammar-final", read, 0)?;
        if let Some(history) = &mut self.history {
            let available = prepared.ledger.remaining().map_err(command)?;
            history.recheck(
                available.source_read_bytes,
                available.state_bytes,
                self.cancel,
            )?;
            let usage = (history.read_bytes(), history.retained_state_bytes());
            let reads = usage
                .0
                .checked_sub(self.history_usage.0)
                .ok_or_else(|| invalid("historical read accounting regressed"))?;
            let state = usage
                .1
                .checked_sub(self.history_usage.1)
                .ok_or_else(|| invalid("historical state accounting regressed"))?;
            debit(
                &mut prepared.ledger,
                "admission-historical-final",
                reads,
                state,
            )?;
            self.history_usage = usage;
        }
        Ok(())
    }
    pub(crate) fn identity(&self) -> Digest256 {
        self.identity
    }
    pub(crate) fn deadline(&self) -> Instant {
        self.deadline
    }
    /// The protected whole-run declaration, separate from logical state and
    /// worker address space. Host admission and observed process fit remain
    /// prerequisites owned by the original launcher/controller.
    pub(crate) fn declared_working_ram_bytes(&self) -> io::Result<u64> {
        let invocation = if let Some(prepared) = &self.prepared {
            &prepared.invocation
        } else if let Some(evaluated) = &self.evaluated {
            &evaluated.invocation
        } else {
            return Err(invalid("admission invocation is unavailable"));
        };
        Ok(invocation.budgets.working_ram_bytes)
    }
    fn ledger(&self) -> io::Result<&FoundationRemainingBudget<'c>> {
        if let Some(p) = &self.prepared {
            Ok(&p.ledger)
        } else if let Some(p) = &self.evaluated {
            Ok(&p.remaining_budget)
        } else {
            Err(invalid("admission validator already refused"))
        }
    }
    fn ledger_mut(&mut self) -> io::Result<&mut FoundationRemainingBudget<'c>> {
        if let Some(p) = &mut self.prepared {
            Ok(&mut p.ledger)
        } else if let Some(p) = &mut self.evaluated {
            Ok(&mut p.remaining_budget)
        } else {
            Err(invalid("admission validator already refused"))
        }
    }
    /// Persistent admission writes require the launcher-selected store, not
    /// merely a logical write allowance or the temporary validator stage.
    /// Select before batch/candidate allocation so retained custody participates
    /// in every subsequent remaining-state calculation.
    pub(crate) fn bind_store_authority(&mut self, store: &Path) -> io::Result<()> {
        active(self.deadline, self.cancel)?;
        if self.store_authority.is_some() {
            return Err(invalid("admission store authority already selected"));
        }
        if self.write_cap == 0 {
            return Err(invalid(
                "admission requires explicit persistent write budget",
            ));
        }
        self.preflight_store_guard(
            PRIVATE_TMPFS_SELECT_COST.read_bytes,
            PRIVATE_TMPFS_SELECT_COST.workspace_bytes,
        )?;
        let selected = self
            .prepared
            .as_ref()
            .ok_or_else(|| invalid("store authority requires prepared invocation"))?
            .invocation
            .select_stage();
        self.charge_store_guard(
            PRIVATE_TMPFS_SELECT_COST.read_bytes,
            PRIVATE_TMPFS_SELECT_COST.retained_bytes,
        )?;
        let selected = selected.map_err(invalid)?;
        // Selection just checked the kernel and held/named store identity.
        // Associate the CLI path without repeating that same successful read.
        if selected.persistent_store() != Some(store) {
            return Err(invalid(
                "requested admission store differs from selected authority",
            ));
        }
        self.store_authority = Some(selected);
        Ok(())
    }
    fn preflight_store_guard(&self, read: u64, workspace: usize) -> io::Result<()> {
        active(self.deadline, self.cancel)?;
        let remaining = self.ledger()?.remaining().map_err(command)?;
        if read > remaining.source_read_bytes || workspace > remaining.state_bytes {
            return Err(invalid(
                "admission store custody exceeds remaining operation budget",
            ));
        }
        Ok(())
    }
    fn charge_store_guard(&mut self, read: u64, retained: usize) -> io::Result<()> {
        let ledger = self.ledger_mut()?;
        let ticket = ledger
            .begin_window(
                "admission-store-custody",
                FoundationPhaseReservation::default(),
            )
            .map_err(command)?;
        ledger
            .complete_window(
                ticket,
                FoundationPhaseUse {
                    // These are source-owned conservative whole-guard bounds, not
                    // measurements of procfs or filesystem traffic.
                    source_read_bytes: FoundationCharge::admitted_upper_bound(read),
                    state_bytes: FoundationCharge::admitted_upper_bound(retained),
                    ..FoundationPhaseUse::default()
                },
            )
            .map_err(command)
    }
    fn account_v2_root_guard(
        &mut self,
        io_upper_bound: u64,
        foundation_upper_bound: u64,
        retained: usize,
    ) -> io::Result<()> {
        self.preflight_store_guard(foundation_upper_bound, retained)?;
        let io = self
            .segment_v2_profile
            .as_ref()
            .ok_or_else(|| invalid("V2 root guard lacks selected IO profile"))?
            .io
            .clone();
        let io_result = io.charge_read_upper_bound(io_upper_bound).map_err(invalid);
        let io_snapshot = io.snapshot();

        let before = self.ledger()?.admitted_charged().source_read_bytes;
        let foundation_result = self.charge_store_guard(foundation_upper_bound, retained);
        let after = self.ledger()?.admitted_charged().source_read_bytes;
        let charged = after
            .checked_sub(before)
            .ok_or_else(|| invalid("V2 root guard admission accounting regressed"))?;
        let terminal_result = if charged < foundation_upper_bound {
            self.ledger_mut()?
                .record_terminal_admitted_source_read_upper_bound_suffix(
                    foundation_upper_bound - charged,
                )
                .map_err(command)
        } else {
            Ok(())
        };
        let admitted = self
            .ledger()?
            .admitted_charged()
            .source_read_bytes
            .checked_sub(before)
            .ok_or_else(|| invalid("V2 root guard terminal accounting regressed"))?;
        if admitted == foundation_upper_bound {
            // This root-open envelope is already charged above as an admitted
            // upper bound. Exclude its matching physical-ledger prefix from
            // the later measured suffix exactly once.
            self.segment_v2_io_accounted = (
                io_snapshot.read_attempted_bytes,
                io_snapshot.write_attempted_bytes,
            );
            self.segment_v2_read_upper_accounted = io_snapshot.read_upper_bound_attempted_bytes;
        }
        io_result?;
        foundation_result?;
        terminal_result
    }
    pub(crate) fn verify_store_authority(&mut self, store: &Path) -> io::Result<()> {
        self.preflight_store_guard(
            PRIVATE_TMPFS_VERIFY_COST.read_bytes,
            PRIVATE_TMPFS_VERIFY_COST.workspace_bytes,
        )?;
        let result = self
            .store_authority
            .as_ref()
            .ok_or_else(|| invalid("persistent admission store authority absent"))?
            .verify_persistent_store(store);
        // Charge attempted guards too. Workspace is temporary; the retained
        // guard was already charged at selection and is never charged twice.
        self.charge_store_guard(PRIVATE_TMPFS_VERIFY_COST.read_bytes, 0)?;
        result.map_err(|error| invalid(format!("admission store authority refused: {error:?}")))
    }
    /// Remove only the invocation's empty verified workspace root. Reuse the
    /// existing private-TMPFS verification allowance and charge the attempted
    /// custody guard even when the unlink/fsync refuses.
    pub(crate) fn cleanup_spooled_workspace(
        &mut self,
        isolated: &IsolatedCreationRoot,
        cancelled: &AtomicBool,
    ) -> io::Result<()> {
        if !std::ptr::eq(self.cancel, cancelled)
            || self
                .shared_cancel
                .as_ref()
                .is_some_and(|owner| !std::ptr::eq(cancelled, Arc::as_ptr(owner)))
        {
            return Err(invalid("spooled cleanup cancellation owner differs"));
        }
        self.preflight_store_guard(PRIVATE_TMPFS_VERIFY_COST.read_bytes, 0)?;
        let cleanup = isolated
            .cleanup_empty(self.deadline, cancelled)
            .map_err(command);
        let charged = self.charge_store_guard(PRIVATE_TMPFS_VERIFY_COST.read_bytes, 0);
        match (cleanup, charged) {
            (Err(error), _) => Err(error),
            (Ok(()), Err(error)) => Err(error),
            (Ok(()), Ok(())) => Ok(()),
        }
    }
    pub(crate) fn candidate_limits(&self) -> io::Result<CandidateLimits> {
        active(self.deadline, self.cancel)?;
        if self.write_cap == 0 {
            return Err(invalid(
                "admission requires explicit persistent write budget",
            ));
        }
        let remaining = self.ledger()?.remaining().map_err(command)?;
        let caps = self.ledger()?.caps();
        let members = count(caps.max_current_members)?;
        let bytes = count(caps.max_member_bytes.min(remaining.source_read_bytes))?;
        let mut json = JsonLimits::default();
        json.max_bytes = bytes;
        Ok(CandidateLimits {
            admission: AdmissionLimits {
                max_batch_bytes: bytes,
                max_members: members,
                max_member_bytes: caps.max_member_bytes.min(remaining.source_read_bytes),
                max_source_bytes: remaining.source_read_bytes,
                json,
            },
            reader: ReadLimits {
                max_manifest_bytes: bytes,
                // Index entries also include identities, edges and retirements.
                max_manifest_entries: bytes.min(json.max_visits),
                max_selected_object_bytes: caps.max_member_bytes.min(remaining.source_read_bytes),
                json,
            },
            max_state_bytes: remaining.state_bytes,
            // History cardinality is independent of current membership. Its
            // retained keys provide a lower bound per entry; the Candidate
            // still charges actual map nodes, strings and read workspace.
            max_history_revisions: remaining.state_bytes / std::mem::size_of::<Digest256>(),
            max_history_identities: remaining.state_bytes / std::mem::size_of::<(String, String)>(),
            max_read_bytes: remaining.source_read_bytes,
            max_write_bytes: self.write_cap,
        })
    }

    /// Produce a one-use, invocation-derived spooled profile. The old
    /// borrowed-cancel constructor is deliberately resident-only. The shared
    /// entry requires a retained original Arc, persistent-store custody, and
    /// sufficient *remaining* protected read/state/tmpfs ceilings before it
    /// creates a private workspace or SQLite budgets.
    pub(crate) fn prepared_execution_resources(
        &mut self,
    ) -> io::Result<PreparedAdmissionExecution> {
        if self.execution_resources_taken {
            return Err(invalid("admission execution resources already consumed"));
        }
        self.execution_resources_taken = true;
        if !self.spooled_route_selected {
            return Ok(PreparedAdmissionExecution::Resident);
        }
        if self.shared_cancel.is_none() {
            return Err(invalid(
                "selected spooled route omitted original cancellation owner",
            ));
        }
        if self.prepared.is_none() || self.evaluated.is_some() {
            return Err(invalid(
                "spooled resources require a fresh prepared invocation",
            ));
        }
        active(self.deadline, self.cancel)?;
        let cancelled = self
            .shared_cancel
            .as_ref()
            .cloned()
            .ok_or_else(|| invalid("spooled cancellation owner absent"))?;
        if !std::ptr::eq(self.cancel, Arc::as_ptr(&cancelled))
            || cancelled.load(std::sync::atomic::Ordering::Acquire)
        {
            return Err(invalid("spooled cancellation owner changed or cancelled"));
        }

        let remaining = self.ledger()?.remaining().map_err(command)?;
        let caps = self.ledger()?.caps();
        let v2_source_root_state_bytes = if self.prepared.as_ref().is_some_and(|prepared| {
            prepared.invocation.admission_representation()
                == foundation_entry::FoundationAdmissionRepresentation::NativeV4SegmentV2
        }) {
            let path = self
                .store_authority
                .as_ref()
                .and_then(PrivateTmpfsStageIsolation::persistent_store)
                .ok_or_else(|| invalid("V2 profile lacks selected persistent store"))?;
            size_of::<PreparedV2StoreRoot>()
                .checked_add(path.as_os_str().as_bytes().len())
                .ok_or_else(|| invalid("V2 source root retained-state overflow"))?
        } else {
            0
        };
        let v2_case_state_bytes = self
            .prepared
            .as_ref()
            .and_then(|prepared| prepared.invocation.v2_case())
            .map(|case| case.retained_state_bytes())
            .transpose()
            .map_err(command)?
            .unwrap_or(0);
        let v2_target_root_state_bytes = if v2_case_state_bytes != 0 {
            let root_path = self
                .prepared
                .as_ref()
                .and_then(|prepared| prepared.invocation.artifact_root.as_ref())
                .ok_or_else(|| invalid("V2 case artifact root absent"))?;
            size_of::<PreparedV2ArtifactRoot>()
                .checked_add(root_path.as_os_str().as_bytes().len())
                .ok_or_else(|| invalid("V2 target root retained-state overflow"))?
        } else {
            0
        };
        let case_clone_preflight = size_of::<PreparedSpooledExecution>()
            .checked_add(v2_case_state_bytes)
            .and_then(|n| n.checked_add(v2_source_root_state_bytes))
            .and_then(|n| n.checked_add(v2_target_root_state_bytes))
            // Conservative fixed control-state envelope includes the shared
            // write root, both local IoState blocks and their Arc headers;
            // size_of::<PinnedSqliteIoBudget>() above counts only its handle.
            .and_then(|n| n.checked_add(SPOOLED_IO_CONTROL_STATE_BYTES))
            .ok_or_else(|| invalid("V2 case clone state overflow"))?;
        if case_clone_preflight > remaining.state_bytes {
            return Err(invalid("V2 case clone exceeds remaining state"));
        }
        let v2_case = self
            .prepared
            .as_ref()
            .and_then(|prepared| prepared.invocation.v2_case().cloned());
        let mut candidate_limits = match self.candidate_limits() {
            Ok(limits) => limits,
            Err(_) => return Err(invalid("selected spooled candidate limits refused")),
        };
        let write_remaining = self
            .write_cap
            .checked_sub(self.candidate_io.1)
            .ok_or_else(|| invalid("persistent admission write accounting regressed"))?;
        let segment_v2_total_store_bytes = self
            .prepared
            .as_ref()
            .and_then(|prepared| {
                (prepared.invocation.admission_representation()
                    == foundation_entry::FoundationAdmissionRepresentation::NativeV4SegmentV2)
                    .then_some(prepared.invocation.budgets.max_admission_store_bytes)
            })
            .flatten();
        let segment_v2_store_bytes = self
            .prepared
            .as_ref()
            .and_then(|prepared| {
                (prepared.invocation.admission_representation()
                    == foundation_entry::FoundationAdmissionRepresentation::NativeV4SegmentV2)
                    .then(|| prepared.invocation.source_store_allocation_bytes())
            })
            .flatten();
        if let Some(case) = v2_case.as_ref() {
            if case
                .source_store_bytes
                .checked_add(case.target_store_bytes)
                .and_then(|bytes| bytes.checked_add(case.sqlite_store_bytes))
                != segment_v2_total_store_bytes
                || Some(case.source_store_bytes) != segment_v2_store_bytes
                || case.state_bytes > remaining.state_bytes
            {
                return Err(invalid(
                    "V2 case source/target profile differs from remaining invocation",
                ));
            }
        }
        // Allocation limits own resident physical bytes. Candidate and V2
        // writes instead debit one original cumulative-write authority.
        let segment_v2_write_cap = if segment_v2_store_bytes.is_some() {
            write_remaining
        } else {
            0
        };
        let candidate_write_cap = write_remaining;
        if remaining.source_read_bytes == 0
            || remaining.state_bytes < 64 * 1024
            || remaining.tmpfs_bytes < 512 * 1024
            || remaining.tmpfs_inodes < 8
            || candidate_write_cap == 0
            || candidate_limits.max_read_bytes == u64::MAX
            || write_remaining == u64::MAX
        {
            return Err(invalid(
                "selected spooled profile exceeds remaining invocation resources",
            ));
        }
        candidate_limits.max_write_bytes = candidate_write_cap;
        if candidate_limits.admission.validate().is_err() {
            return Err(invalid(
                "selected spooled profile exceeds remaining invocation resources",
            ));
        }

        // Verify only the already-selected persistent authority. The stage is
        // never reselected from the environment here.
        let selected_store = self
            .store_authority
            .as_ref()
            .and_then(PrivateTmpfsStageIsolation::persistent_store)
            .ok_or_else(|| invalid("spooled profile requires selected persistent store"))?;
        self.preflight_store_guard(
            PRIVATE_TMPFS_VERIFY_COST.read_bytes,
            PRIVATE_TMPFS_VERIFY_COST.workspace_bytes,
        )?;
        let store_verify_result = self
            .store_authority
            .as_ref()
            .ok_or_else(|| invalid("private stage authority disappeared"))?
            .verify_persistent_store(selected_store);
        self.charge_store_guard(PRIVATE_TMPFS_VERIFY_COST.read_bytes, 0)?;
        store_verify_result
            .map_err(|error| invalid(format!("admission store authority refused: {error:?}")))?;
        let usage_result = self
            .store_authority
            .as_ref()
            .ok_or_else(|| invalid("private stage authority disappeared"))?
            .quota_usage()
            .map_err(|error| invalid(format!("private stage quota check refused: {error:?}")));
        self.charge_store_guard(PRIVATE_TMPFS_VERIFY_COST.read_bytes, 0)?;
        let usage = usage_result?;
        active(self.deadline, self.cancel)?;

        // Reserve the still-future invocation store guards before any shared
        // candidate/parser IO: profile debit, post-create verification,
        // prepublication verification, final store recheck and empty-root
        // cleanup. The eventual shared ledger is narrowed to this exact
        // remaining source-read envelope before its first parser read.
        const FUTURE_STORE_GUARDS: u64 = 5;
        let reserved_store_reads = PRIVATE_TMPFS_VERIFY_COST
            .read_bytes
            .checked_mul(FUTURE_STORE_GUARDS)
            .ok_or_else(|| invalid("spooled store-guard reserve overflow"))?;
        let read_before_profile = self
            .ledger()?
            .remaining()
            .map_err(command)?
            .source_read_bytes;
        let spooled_io_read_cap = read_before_profile
            .checked_sub(reserved_store_reads)
            .filter(|bytes| *bytes != 0 && *bytes < u64::MAX)
            .ok_or_else(|| invalid("spooled IO leaves no room for final store guards"))?;
        let segment_v2_read_cap = if segment_v2_store_bytes.is_some() {
            let base_tree_read_cap = (spooled_io_read_cap / 4).min(64 * 1024 * 1024);
            let case_read_cap = match v2_case.as_ref() {
                Some(case) => case
                    .point_tree_bytes
                    .checked_mul(2)
                    .and_then(|n| n.checked_add(case.tree_bytes))
                    .ok_or_else(|| invalid("V2 case read profile overflow"))?,
                None => 0,
            };
            base_tree_read_cap
                .checked_add(case_read_cap)
                .filter(|cap| *cap < spooled_io_read_cap)
                .ok_or_else(|| invalid("V2 case leaves no candidate read allowance"))?
        } else {
            0
        };
        let candidate_read_cap = spooled_io_read_cap
            .checked_sub(segment_v2_read_cap)
            .filter(|bytes| *bytes != 0)
            .ok_or_else(|| invalid("V2 profile leaves no candidate read allowance"))?;
        let original_io_read_cap = candidate_limits.max_read_bytes;
        let cap_usize = count(candidate_read_cap)?;
        candidate_limits.max_read_bytes = candidate_limits.max_read_bytes.min(candidate_read_cap);
        candidate_limits.admission.max_batch_bytes =
            candidate_limits.admission.max_batch_bytes.min(cap_usize);
        candidate_limits.admission.max_member_bytes = candidate_limits
            .admission
            .max_member_bytes
            .min(candidate_read_cap);
        candidate_limits.admission.max_source_bytes = candidate_limits
            .admission
            .max_source_bytes
            .min(candidate_read_cap);
        candidate_limits.admission.json.max_bytes =
            candidate_limits.admission.json.max_bytes.min(cap_usize);
        candidate_limits.reader.max_manifest_bytes =
            candidate_limits.reader.max_manifest_bytes.min(cap_usize);
        candidate_limits.reader.max_selected_object_bytes = candidate_limits
            .reader
            .max_selected_object_bytes
            .min(candidate_read_cap);
        candidate_limits.reader.json.max_bytes =
            candidate_limits.reader.json.max_bytes.min(cap_usize);
        candidate_limits.validate()?;

        let quota_bytes = caps
            .tmpfs_quota_bytes
            .checked_sub(usage.used_bytes)
            .ok_or_else(|| invalid("private stage byte usage exceeds selected quota"))?;
        let quota_inodes = caps
            .tmpfs_inode_limit
            .checked_sub(usage.used_inodes)
            .ok_or_else(|| invalid("private stage inode usage exceeds selected quota"))?;
        let physical_available = remaining.tmpfs_bytes.min(quota_bytes);
        let inode_available = remaining.tmpfs_inodes.min(quota_inodes);

        // Half of the exact remaining private-stage envelope is offered to
        // the SQLite/candidate/publication family; the other half remains for
        // the actual compiler/render and final operation custody. Each disk
        // consumer gets a disjoint fraction of this shared physical ledger.
        let declared_profile = physical_available / 2;
        const ROOT_METADATA_BOUND: u64 = 4096;
        const MIN_SQLITE_PARTITION: u64 = 256 * 1024;
        if declared_profile <= ROOT_METADATA_BOUND {
            return Err(invalid(
                "selected spooled profile exceeds remaining invocation resources",
            ));
        }
        // Opt-in provider placement is an explicit slice of the ORIGINAL
        // persistent envelope. Absence leaves the historical TMPFS route intact.
        let persistent_sqlite_bytes = v2_case.as_ref().map_or(0, |case| case.sqlite_store_bytes);
        let persistent_root_bound = if persistent_sqlite_bytes != 0 {
            v2_case
                .as_ref()
                .ok_or_else(|| invalid("persistent SQLite case absent"))?
                .allocation_unit_bytes
                .checked_mul(2)
                .ok_or_else(|| invalid("persistent SQLite root metadata overflow"))?
        } else {
            0
        };
        let sqlite_space = if persistent_sqlite_bytes != 0 {
            persistent_sqlite_bytes
                .checked_sub(persistent_root_bound)
                .filter(|bytes| *bytes != 0)
                .ok_or_else(|| invalid("persistent SQLite slice leaves no provider space"))?
        } else {
            declared_profile - ROOT_METADATA_BOUND
        };
        let candidate_partition = sqlite_space / 4;
        let index_partition = sqlite_space / 4;
        let reader_partition = sqlite_space / 4;
        let manifest_partition = sqlite_space / 8;
        let defaults_partition = sqlite_space / 8;
        if candidate_partition < MIN_SQLITE_PARTITION
            || index_partition < MIN_SQLITE_PARTITION
            || reader_partition < MIN_SQLITE_PARTITION
            || defaults_partition < MIN_SQLITE_PARTITION
            || manifest_partition == 0
        {
            return Err(invalid(
                "selected spooled profile exceeds remaining invocation resources",
            ));
        }

        // Keep room for the private directory and five simultaneously
        // retained main-file handles. Candidate, native index and defaults
        // auxiliary ceilings split this same remaining inode slice.
        let inode_profile = inode_available / 2;
        let auxiliary_slots = inode_profile.saturating_sub(6) / 3;
        let max_live_aux = usize::try_from(auxiliary_slots.min(16)).unwrap_or(16);
        if max_live_aux == 0 {
            return Err(invalid(
                "selected spooled profile exceeds remaining invocation resources",
            ));
        }
        // Keep the already-selected consumer ceilings. The filesystem's
        // measured root quota cost must fit beside all five retained main
        // handles and all three auxiliary families in this same profile.
        let retained_consumer_inodes = u64::try_from(max_live_aux)
            .ok()
            .and_then(|aux| aux.checked_mul(3))
            .and_then(|aux| aux.checked_add(5))
            .ok_or_else(|| invalid("spooled inode consumer reservation overflow"))?;
        let root_inode_bound = inode_profile
            .checked_sub(retained_consumer_inodes)
            .filter(|bound| *bound != 0)
            .ok_or_else(|| invalid("spooled inode profile leaves no root allowance"))?;
        let Some(candidate_sqlite) = sqlite_aux_limits(candidate_partition, max_live_aux) else {
            return Err(invalid(
                "selected spooled profile exceeds remaining invocation resources",
            ));
        };
        let Some(index_sqlite) = sqlite_aux_limits(index_partition, max_live_aux) else {
            return Err(invalid(
                "selected spooled profile exceeds remaining invocation resources",
            ));
        };

        let Some(defaults_sqlite) = sqlite_aux_limits(defaults_partition, max_live_aux) else {
            return Err(invalid(
                "selected defaults scope exceeds remaining invocation resources",
            ));
        };

        // The existing creation plan has three auxiliary families plus one
        // strict-reader main and one manifest main. Their simultaneous ceilings
        // must fit ONLY the SQL slice, even though the ledger also owns the
        // distinct source and target reservations.
        let family_upper = |limits: PinnedSqliteAuxLimits| {
            limits
                .main_allocated_bytes
                .checked_add(limits.temp_db_allocated_bytes)
                .and_then(|n| n.checked_add(limits.main_journal_allocated_bytes))
                .and_then(|n| n.checked_add(limits.temp_journal_allocated_bytes))
                .and_then(|n| n.checked_add(limits.other_aux_aggregate_allocated_bytes))
        };
        let sqlite_consumer_upper = family_upper(candidate_sqlite)
            .and_then(|n| n.checked_add(family_upper(index_sqlite)?))
            .and_then(|n| n.checked_add(family_upper(defaults_sqlite)?))
            .and_then(|n| n.checked_add(reader_partition))
            .and_then(|n| n.checked_add(manifest_partition))
            .ok_or_else(|| invalid("spooled consumer allocation overflow"))?;
        if sqlite_consumer_upper > sqlite_space {
            return Err(invalid("spooled consumers exceed selected SQL slice"));
        }

        let root_len = self
            .store_authority
            .as_ref()
            .ok_or_else(|| invalid("private stage authority disappeared"))?
            .root()
            .as_os_str()
            .len();
        let state_bound = size_of::<PreparedSpooledExecution>()
            .checked_add(size_of::<
                Option<(SpoolIndexLimits, SpoolIndexLimits, PinnedSqliteIoBudget)>,
            >())
            .and_then(|n| n.checked_add(size_of::<Option<(File, PinnedSqliteSpaceBudget)>>()))
            .and_then(|n| n.checked_add(size_of::<IsolatedCreationRoot>()))
            .and_then(|n| n.checked_add(size_of::<PinnedSqliteIoBudget>()))
            .and_then(|n| n.checked_add(size_of::<PinnedSqliteSpaceBudget>()))
            .and_then(|n| n.checked_add(size_of::<Option<NativeSegmentV2Budget>>()))
            .and_then(|n| n.checked_add(v2_case_state_bytes))
            .and_then(|n| n.checked_add(v2_source_root_state_bytes))
            .and_then(|n| n.checked_add(v2_target_root_state_bytes))
            .and_then(|n| n.checked_add(root_len.checked_mul(2)?))
            .and_then(|n| n.checked_add(4096))
            .ok_or_else(|| invalid("spooled profile retained state overflow"))?;
        let profile_inodes = inode_profile;
        let profile_bytes = declared_profile;
        let segment_v2_state_bytes = if segment_v2_store_bytes.is_some() {
            v2_case
                .as_ref()
                .map(|case| case.state_bytes)
                .unwrap_or_else(|| (remaining.state_bytes / 4).min(32 * 1024 * 1024))
        } else {
            0
        };
        let profile_state_bytes = state_bound
            .checked_add(segment_v2_state_bytes)
            .ok_or_else(|| invalid("V2 retained-state reservation overflow"))?;
        if profile_state_bytes > remaining.state_bytes
            || profile_bytes > remaining.tmpfs_bytes
            || profile_inodes > remaining.tmpfs_inodes
        {
            return Err(invalid(
                "selected spooled profile exceeds remaining invocation resources",
            ));
        }
        let segment_v2_allocation = if let (Some(max_allocated_bytes), Some(total_store_bytes)) =
            (segment_v2_store_bytes, segment_v2_total_store_bytes)
        {
            let space = PinnedSqliteSpaceBudget::new(total_store_bytes).map_err(invalid)?;
            let reservation = Arc::new(space.reserve(max_allocated_bytes).map_err(invalid)?);
            Some((space, reservation))
        } else if segment_v2_store_bytes.is_some() || segment_v2_total_store_bytes.is_some() {
            return Err(invalid("V2 persistent allocation selection differs"));
        } else {
            None
        };
        let candidate_state = remaining
            .state_bytes
            .checked_sub(profile_state_bytes)
            .filter(|bytes| *bytes != 0)
            .ok_or_else(|| invalid("spooled profile leaves no candidate state"))?;
        candidate_limits.max_state_bytes = candidate_limits.max_state_bytes.min(candidate_state);
        candidate_limits.max_history_revisions = candidate_limits
            .max_history_revisions
            .min(candidate_state / size_of::<Digest256>());
        candidate_limits.max_history_identities = candidate_limits
            .max_history_identities
            .min(candidate_state / size_of::<(String, String)>());
        let state = candidate_limits.max_state_bytes;
        let row_state = state / 16;
        let cache_bytes = (8 * 1024 * 1024usize).min(state / 32);
        if row_state == 0
            || cache_bytes == 0
            || cache_bytes > row_state
            || candidate_limits.max_history_revisions == 0
            || candidate_limits.max_history_identities == 0
        {
            return Err(invalid(
                "selected spooled profile exceeds remaining invocation resources",
            ));
        }
        let max_manifest_allocated_bytes = manifest_partition;
        let manifest_bytes = candidate_limits.reader.max_manifest_bytes.min(
            usize::try_from(max_manifest_allocated_bytes)
                .map_err(|_| invalid("spooled manifest allocation bound exceeds address space"))?,
        );
        if manifest_bytes == 0 {
            return Err(invalid(
                "selected spooled profile exceeds remaining invocation resources",
            ));
        }
        let max_members = candidate_limits.admission.max_members;
        let max_manifest_entries = u64::try_from(
            candidate_limits
                .reader
                .max_manifest_entries
                .min(manifest_bytes),
        )
        .map_err(|_| invalid("spooled manifest entry bound exceeds range"))?;
        let max_member_bytes = candidate_limits.admission.max_member_bytes;
        let max_total_bytes = candidate_limits.admission.max_source_bytes;
        let max_revisions = candidate_limits.max_history_revisions;
        if max_revisions == 0 || max_members == 0 || max_member_bytes == 0 || max_total_bytes == 0 {
            return Err(invalid(
                "selected spooled profile exceeds remaining invocation resources",
            ));
        }
        let streamed_cut_limits = StreamedCutReadLimitsV1 {
            cut: CutReadLimits {
                max_revisions,
                max_members: u64::try_from(max_members)
                    .map_err(|_| invalid("spooled member count exceeds range"))?,
                max_total_bytes,
                max_member_bytes,
            },
            manifest_json: candidate_limits.admission.json,
            max_manifest_entries: usize::try_from(max_manifest_entries)
                .map_err(|_| invalid("spooled manifest entry count exceeds address space"))?,
            max_index_bytes: reader_partition,
            max_manifest_row_bytes: usize::try_from(max_member_bytes.min(usize::MAX as u64 - 1))
                .map_err(|_| invalid("spooled member byte limit exceeds range"))?,
            sqlite_cache_bytes: cache_bytes,
        };
        let manifest_limits = ManifestStreamLimits {
            max_manifest_bytes: u64::try_from(manifest_bytes)
                .map_err(|_| invalid("spooled manifest bytes exceed u64"))?,
            row_json: candidate_limits.admission.json,
        };

        // Reserve the profile, directory metadata and inode ceiling before
        // creating the workspace or the ledger control blocks.
        debit_spooled_profile(
            self.ledger_mut()?,
            PRIVATE_TMPFS_VERIFY_COST.read_bytes,
            profile_state_bytes,
            profile_bytes,
            profile_inodes,
        )?;
        active(self.deadline, self.cancel)?;
        let authority = self
            .store_authority
            .as_ref()
            .ok_or_else(|| invalid("private stage authority disappeared"))?;
        let aggregate_write = match &self.generated_input {
            Some(early) => {
                early
                    .aggregate_write
                    .restrict_remaining_io(original_io_read_cap, write_remaining)
                    .map_err(invalid)?;
                early.aggregate_write.clone()
            }
            None => {
                PinnedSqliteIoBudget::new(original_io_read_cap, write_remaining).map_err(invalid)?
            }
        };
        let io_budget = match &self.generated_input {
            Some(early) => early.io.clone(),
            None => PinnedSqliteIoBudget::new_with_shared_write_authority(
                original_io_read_cap,
                candidate_limits.max_write_bytes,
                aggregate_write.clone(),
            )
            .map_err(invalid)?,
        };
        io_budget
            .restrict_remaining_io(
                candidate_limits.max_read_bytes,
                candidate_limits.max_write_bytes,
            )
            .map_err(invalid)?;
        let persistent_parent = if persistent_sqlite_bytes != 0 {
            let parent = authority
                .persistent_store()
                .ok_or_else(|| invalid("persistent SQLite original store absent"))?;
            let held = authority
                .persistent_store_custody(parent)
                .map_err(invalid)?
                .try_clone()?;
            let before = held
                .metadata()?
                .blocks()
                .checked_mul(512)
                .ok_or_else(|| invalid("persistent SQLite parent allocation overflow"))?;
            Some((held, before))
        } else {
            None
        };
        let persistent_root_allocation = if persistent_sqlite_bytes != 0 {
            let (space, _) = segment_v2_allocation
                .as_ref()
                .ok_or_else(|| invalid("persistent SQLite original allocation absent"))?;
            Some(Arc::new(
                space.reserve(persistent_root_bound).map_err(invalid)?,
            ))
        } else {
            None
        };
        let isolated = if persistent_sqlite_bytes != 0 {
            let parent = authority
                .persistent_store()
                .ok_or_else(|| invalid("persistent SQLite original store absent"))?;
            let held = authority
                .persistent_store_custody(parent)
                .map_err(invalid)?;
            // This is a bounded metadata-work permit, not returned payload.
            io_budget
                .charge_write(persistent_root_bound)
                .map_err(invalid)?;
            IsolatedCreationRoot::create_with_held_parent(
                parent,
                held,
                Arc::clone(
                    persistent_root_allocation
                        .as_ref()
                        .ok_or_else(|| invalid("persistent SQLite root custody absent"))?,
                ),
                self.deadline,
                &cancelled,
            )
            .map_err(command)?
        } else {
            IsolatedCreationRoot::create(authority.root(), self.deadline, &cancelled)
                .map_err(command)?
        };
        let workspace_result =
            (|| -> io::Result<(File, PinnedSqliteIoBudget, PinnedSqliteSpaceBudget, File)> {
                let workspace = isolated
                    .verify_current(self.deadline, &cancelled)
                    .map_err(command)?;
                active(self.deadline, self.cancel)?;
                self.preflight_store_guard(
                    PRIVATE_TMPFS_VERIFY_COST.read_bytes,
                    PRIVATE_TMPFS_VERIFY_COST.workspace_bytes,
                )?;
                let after_usage_result = self
                    .store_authority
                    .as_ref()
                    .ok_or_else(|| invalid("private stage authority disappeared"))?
                    .quota_usage()
                    .map_err(|error| {
                        invalid(format!(
                            "private stage post-create check refused: {error:?}"
                        ))
                    });
                self.charge_store_guard(PRIVATE_TMPFS_VERIFY_COST.read_bytes, 0)?;
                let after_usage = after_usage_result?;
                if persistent_sqlite_bytes == 0
                    && (after_usage.used_bytes < usage.used_bytes
                        || after_usage
                            .used_bytes
                            .checked_sub(usage.used_bytes)
                            .is_none_or(|n| n > ROOT_METADATA_BOUND)
                        || after_usage.used_inodes < usage.used_inodes
                        || after_usage
                            .used_inodes
                            .checked_sub(usage.used_inodes)
                            .is_none_or(|n| n == 0 || n > root_inode_bound))
                {
                    // Only fixed labels and numeric quota observations cross
                    // the public refusal boundary; paths and raw IO stay sealed.
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        crate::source_command::SourceCommandError::DeniedWithReason(format!(
                            "private spooled workspace creation exceeded its precharge: \
                             before_used_bytes={} before_used_inodes={} \
                             after_used_bytes={} after_used_inodes={} \
                             root_metadata_bound_bytes={} root_inode_bound={} \
                             inode_profile={} retained_consumer_inodes={} \
                             selected_tmpfs_quota_bytes={} selected_tmpfs_inode_limit={} \
                             bytes_decreased={} inodes_decreased={} \
                             bytes_excess={} root_inode_delta_invalid={}",
                            usage.used_bytes,
                            usage.used_inodes,
                            after_usage.used_bytes,
                            after_usage.used_inodes,
                            ROOT_METADATA_BOUND,
                            root_inode_bound,
                            inode_profile,
                            retained_consumer_inodes,
                            caps.tmpfs_quota_bytes,
                            caps.tmpfs_inode_limit,
                            after_usage.used_bytes < usage.used_bytes,
                            after_usage.used_inodes < usage.used_inodes,
                            after_usage
                                .used_bytes
                                .checked_sub(usage.used_bytes)
                                .is_some_and(|n| n > ROOT_METADATA_BOUND),
                            after_usage
                                .used_inodes
                                .checked_sub(usage.used_inodes)
                                .is_some_and(|n| n == 0 || n > root_inode_bound),
                        )),
                    ));
                }
                active(self.deadline, self.cancel)?;

                let space_budget = if persistent_sqlite_bytes != 0 {
                    let (space, _) = segment_v2_allocation.as_ref().ok_or_else(|| {
                        invalid("persistent SQLite original allocation disappeared")
                    })?;
                    let allocation = persistent_root_allocation
                        .as_ref()
                        .ok_or_else(|| invalid("persistent SQLite root custody disappeared"))?;
                    let metadata = workspace.metadata()?;
                    let (held_parent, before) = persistent_parent
                        .as_ref()
                        .ok_or_else(|| invalid("persistent SQLite parent custody disappeared"))?;
                    let after = held_parent
                        .metadata()?
                        .blocks()
                        .checked_mul(512)
                        .ok_or_else(|| invalid("persistent SQLite parent allocation overflow"))?;
                    let parent_delta = after
                        .checked_sub(*before)
                        .ok_or_else(|| invalid("persistent SQLite parent allocation regressed"))?;
                    let actual = metadata
                        .blocks()
                        .checked_mul(512)
                        .and_then(|bytes| bytes.checked_add(parent_delta))
                        .ok_or_else(|| invalid("persistent SQLite root allocation overflow"))?;
                    allocation
                        .update_actual_allocated(actual)
                        .map_err(invalid)?;
                    // Per-family limits partition only sqlite_space; the source
                    // reservation already consumes its distinct original slice.
                    space.clone()
                } else {
                    PinnedSqliteSpaceBudget::new(sqlite_space).map_err(invalid)?
                };
                let retained_workspace = workspace.try_clone()?;
                Ok((
                    workspace,
                    io_budget.clone(),
                    space_budget,
                    retained_workspace,
                ))
            })();
        let (workspace, io_budget, space_budget, retained_workspace) = match workspace_result {
            Ok(prepared) => prepared,
            Err(error) => {
                self.cleanup_spooled_workspace(&isolated, cancelled.as_ref())?;
                return Err(error);
            }
        };
        self.segment_v2_profile = match (segment_v2_store_bytes, segment_v2_allocation) {
            (Some(max_allocated_bytes), Some((allocation_space, allocation_reservation))) => {
                let io = PinnedSqliteIoBudget::new_with_shared_write_authority(
                    segment_v2_read_cap,
                    segment_v2_write_cap,
                    aggregate_write.clone(),
                )
                .map_err(invalid)?;
                let allocation_unit_bytes = v2_case
                    .as_ref()
                    .map(|case| case.allocation_unit_bytes)
                    .unwrap_or(65_536);
                let allocation_accountant =
                    super::source_admission_segment_v2::NativeV2TreeIo::new(
                        io.clone(),
                        Arc::clone(&allocation_reservation),
                        max_allocated_bytes,
                        allocation_unit_bytes,
                        segment_v2_state_bytes,
                    )?;
                let max_node_bytes = 64 * 1024usize;
                let max_total_bytes = segment_v2_read_cap
                    .checked_add(segment_v2_write_cap)
                    .filter(|bytes| *bytes != 0 && *bytes < u64::MAX)
                    .ok_or_else(|| invalid("V2 tree IO bound overflow"))?;
                // Row and node counts cap finite work, not resident row state.
                // The builder keeps bounded stack/pack state independently;
                // actual physical bytes remain under the shared IO ledger.
                let max_rows = v2_case
                    .as_ref()
                    .map(|case| case.tree_rows)
                    .unwrap_or(max_total_bytes);
                let max_nodes = v2_case
                    .as_ref()
                    .map(|case| case.tree_nodes)
                    .unwrap_or(max_total_bytes);
                if max_rows == 0 || max_nodes == 0 {
                    return Err(invalid("V2 finite row/node work profile is empty"));
                }
                let max_frame_bytes = max_allocated_bytes
                    .min(candidate_limits.admission.max_member_bytes)
                    .max(1);
                Some(NativeSegmentV2Budget {
                    max_allocated_bytes,
                    max_frame_bytes,
                    allocation_space,
                    allocation_reservation,
                    allocation_unit_bytes,
                    allocation_accountant,
                    io,
                    max_working_state_bytes: segment_v2_state_bytes,
                    tree_limits: AuthenticatedTreeLimitsV1 {
                        max_key_bytes: 4096,
                        max_value_bytes: 16 * 1024,
                        max_kind_bytes: 64,
                        max_node_bytes,
                        max_children: 16,
                        max_nodes,
                        max_total_bytes,
                        max_rows,
                    },
                })
            }
            (None, None) => None,
            _ => return Err(invalid("V2 persistent allocation selection differs")),
        };
        let request = PinnedSqliteAuxRequest {
            limits: candidate_sqlite,
            io_budget,
            space_budget,
            deadline: self.deadline,
            cancelled,
        };
        let candidate_limits = SpoolLimits {
            candidate: candidate_limits,
            max_row_state_bytes: row_state,
            sqlite_cache_bytes: cache_bytes,
        };
        let index_limits = SpoolIndexLimits {
            sqlite: index_sqlite,
            cache_bytes,
            max_row_state_bytes: row_state,
        };

        let defaults_limits = SpoolIndexLimits {
            sqlite: defaults_sqlite,
            cache_bytes,
            max_row_state_bytes: row_state,
        };
        self._spooled_persistent_root_allocation = persistent_root_allocation;
        self.spooled_workspace = Some((retained_workspace, request.space_budget.clone()));
        self.spooled_profile = Some((index_limits, defaults_limits, request.io_budget.clone()));
        self.spooled_write_cap = Some(write_remaining);
        let v2_allocation_accountant = self
            .segment_v2_profile
            .as_ref()
            .map(|profile| Arc::clone(&profile.allocation_accountant));
        let v2_base_read_limits = match self.segment_v2_profile.as_ref() {
            Some(profile) => Some(super::source_admission_v2_reader::V2PointReadLimits {
                pointer: candidate_limits.candidate.reader,
                segment: SegmentLimits {
                    max_segment_bytes: profile.max_allocated_bytes,
                    max_frame_bytes: profile.max_frame_bytes,
                    max_frames: u32::try_from(profile.tree_limits.max_nodes.min(u32::MAX as u64))
                        .map_err(|_| invalid("V2 base segment frame limit exceeds range"))?
                        .max(1),
                    max_journal_bytes: profile
                        .max_working_state_bytes
                        .min(4 * 1024 * 1024)
                        .max(128),
                },
                tree: profile.tree_limits,
                // This session authenticates metadata rows for native base
                // validation; payload reads use the separately selected case
                // profile after publication.
                max_object_bytes: 1,
                max_state_bytes: profile.max_working_state_bytes,
                caller_retained_state_bytes: 0,
            }),
            None => None,
        };
        let v2_source_root = if v2_allocation_accountant.is_some() {
            let path = self
                .store_authority
                .as_ref()
                .and_then(PrivateTmpfsStageIsolation::persistent_store)
                .ok_or_else(|| invalid("V2 profile lacks selected persistent store"))?;
            const V2_SOURCE_ROOT_METADATA_BOUND: u64 = 4096;
            let path_bytes = u64::try_from(path.as_os_str().as_bytes().len())
                .map_err(|_| invalid("V2 source root path exceeds finite range"))?;
            let io_upper = PRIVATE_TMPFS_SELECT_COST
                .read_bytes
                .checked_add(path_bytes)
                .and_then(|bytes| bytes.checked_add(V2_SOURCE_ROOT_METADATA_BOUND))
                .ok_or_else(|| invalid("V2 source root guard bound overflow"))?;
            self.account_v2_root_guard(
                io_upper,
                io_upper,
                PRIVATE_TMPFS_SELECT_COST.retained_bytes,
            )?;
            let path = self
                .store_authority
                .as_ref()
                .and_then(PrivateTmpfsStageIsolation::persistent_store)
                .ok_or_else(|| invalid("V2 selected persistent store disappeared"))?;
            let held = self
                .store_authority
                .as_ref()
                .ok_or_else(|| invalid("V2 source root custody disappeared"))?
                .persistent_store_custody(path)
                .map_err(invalid)?
                .try_clone()?;
            let metadata = held.metadata()?;
            if !metadata.is_dir()
                || metadata.uid() != rustix::process::geteuid().as_raw()
                || metadata.mode() & 0o022 != 0
            {
                return Err(invalid("V2 source root custody differs"));
            }
            Some(PreparedV2StoreRoot {
                path: path.to_path_buf(),
                identity: (metadata.dev(), metadata.ino()),
                held,
            })
        } else {
            None
        };
        let v2_target_root = if v2_case.is_some() {
            let path = self
                .prepared
                .as_ref()
                .and_then(|prepared| prepared.invocation.artifact_root.as_ref())
                .ok_or_else(|| invalid("V2 case artifact root absent"))?
                .clone();
            // Bound path resolution and metadata observation before touching
            // the selected namespace; the V2 IO ledger records this as an
            // upper bound, not returned payload bytes.
            const V2_TARGET_ROOT_METADATA_BOUND: u64 = 4096;
            let path_bytes = u64::try_from(path.as_os_str().as_bytes().len())
                .map_err(|_| invalid("V2 target root path exceeds finite range"))?;
            let root_open_upper = path_bytes
                .checked_add(V2_TARGET_ROOT_METADATA_BOUND)
                .ok_or_else(|| invalid("V2 target root guard bound overflow"))?;
            self.account_v2_root_guard(root_open_upper, root_open_upper, 0)?;
            active(self.deadline, self.cancel)?;
            let held = tos_fd_open::open_absolute_directory(&path).map_err(invalid)?;
            let metadata = held.metadata()?;
            let expected_uid = self
                .prepared
                .as_ref()
                .ok_or_else(|| invalid("foundation invocation disappeared"))?
                .invocation
                .uid();
            if !metadata.is_dir() || metadata.uid() != expected_uid || metadata.mode() & 0o022 != 0
            {
                return Err(invalid("V2 selected artifact root custody differs"));
            }
            Some(PreparedV2ArtifactRoot {
                path,
                identity: (metadata.dev(), metadata.ino()),
                held,
            })
        } else {
            None
        };

        Ok(PreparedAdmissionExecution::Spooled(
            PreparedSpooledExecution {
                workspace_root: isolated,
                workspace,
                request,
                candidate_limits,
                index_limits,
                defaults_limits,
                manifest_limits,
                streamed_cut_limits,
                max_index_allocated_bytes: reader_partition,
                max_manifest_allocated_bytes,
                v2_case,
                v2_source_root,
                v2_target_root,
                v2_allocation_accountant,
                v2_base_read_limits,
            },
        ))
    }
    pub(crate) fn account_candidate(&mut self, candidate: &Candidate<'_>) -> io::Result<()> {
        candidate.tick()?;
        let usage = candidate.io_usage();
        if usage.1 > self.write_cap {
            return Err(invalid("admission persistent write budget exceeded"));
        }
        let mut read = usage
            .0
            .checked_sub(self.candidate_io.0)
            .ok_or_else(|| invalid("candidate read accounting regressed"))?;
        usage
            .1
            .checked_sub(self.candidate_io.1)
            .ok_or_else(|| invalid("candidate write accounting regressed"))?;
        if !self.batch_charged {
            read = read
                .checked_add(candidate.batch_bytes_read())
                .ok_or_else(|| invalid("batch read accounting overflow"))?;
        }
        let retained = candidate.retained_state_bytes();
        let state = retained
            .checked_sub(self.candidate_state)
            .ok_or_else(|| invalid("candidate state accounting regressed"))?;
        debit(self.ledger_mut()?, "admission-candidate-io", read, state)?;
        self.candidate_state = retained;
        self.candidate_io = usage;
        self.batch_charged = true;
        let remaining = self.ledger()?.remaining().map_err(command)?;
        candidate.restrict_remaining_state(remaining.state_bytes)?;
        candidate.restrict_remaining_io(remaining.source_read_bytes, self.write_cap - usage.1)
    }
    /// Borrow the exact protected profiles and original shared IO authority.
    fn selected_spooled_profile(
        &self,
        index_limits: SpoolIndexLimits,
    ) -> io::Result<&(SpoolIndexLimits, SpoolIndexLimits, PinnedSqliteIoBudget)> {
        let profile = self
            .spooled_profile
            .as_ref()
            .filter(|profile| self.spooled_route_selected && profile.0 == index_limits)
            .ok_or_else(|| invalid("spooled index profile differs from protected invocation"))?;
        Ok(profile)
    }

    /// Adopt the original shared spool counters into the Foundation ledger.
    /// Batch reads already use this same physical ledger, so the first sample
    /// includes them once. Sampling never spends the SQLite IO budget again.
    pub(crate) fn account_spooled_candidate(
        &mut self,
        candidate: &SpoolCandidate<'_>,
    ) -> io::Result<()> {
        let result = self.account_spooled_candidate_inner(candidate);
        match result {
            Err(primary) if self.prepared.is_some() => Err(self.retain_spooled_refusal(
                candidate,
                primary,
                "native-v4 candidate accounting",
            )),
            other => other,
        }
    }

    fn account_spooled_candidate_inner(
        &mut self,
        candidate: &SpoolCandidate<'_>,
    ) -> io::Result<()> {
        if !self.spooled_route_selected
            || self
                .spooled_profile
                .as_ref()
                .is_none_or(|profile| !candidate.shares_io_budget(&profile.2))
            || !candidate.matches_invocation(self.deadline, self.cancel)
            || candidate.fence()?.validator_sha256 != self.identity
        {
            candidate.abandon();
            return Err(invalid("spooled candidate original invocation differs"));
        }
        let usage = candidate.io_usage()?;
        if usage.read_permitted_bytes > usage.read_attempted_bytes
            || usage.read_returned_bytes > usage.read_permitted_bytes
            || usage.write_permitted_bytes > usage.write_attempted_bytes
            || usage.write_returned_bytes > usage.write_permitted_bytes
            || usage.write_attempted_bytes > self.write_cap
        {
            candidate.abandon();
            return Err(invalid("spooled candidate physical IO accounting refused"));
        }
        self.account_spooled_terminal_io(candidate)?;
        let retained = candidate.own_retained_state_upper_bound_bytes()?;
        let state = retained
            .checked_sub(self.candidate_state)
            .ok_or_else(|| invalid("spooled candidate state accounting regressed"))?;
        let before_state = self.ledger()?.admitted_charged().state_bytes;
        let charged = debit(
            self.ledger_mut()?,
            "admission-spooled-candidate-state",
            0,
            state,
        );
        if self
            .ledger()?
            .admitted_charged()
            .state_bytes
            .checked_sub(before_state)
            == Some(state)
        {
            self.candidate_state = retained;
        }
        self.batch_charged = true;
        charged?;
        if usage.failure.is_some() {
            candidate.abandon();
            return Err(invalid("spooled candidate physical IO accounting refused"));
        }
        let remaining = self.ledger()?.remaining().map_err(command)?;
        // The held baseline has just been charged. The candidate's API takes
        // baseline plus transient headroom, not that headroom alone.
        let total_owned = retained
            .checked_add(remaining.state_bytes)
            .ok_or_else(|| invalid("spooled candidate state ceiling overflow"))?;
        candidate.restrict_remaining_state(total_owned)?;
        candidate.restrict_remaining_io(
            remaining.source_read_bytes,
            self.write_cap - usage.write_attempted_bytes,
        )
    }
    /// The returned manifest is caller-owned, outside Candidate's retained
    /// state. Charge it once before allocating the receipt; keep the earlier
    /// index debit conservatively rather than inventing a state refund.
    pub(crate) fn account_manifest(
        &mut self,
        candidate: &Candidate<'_>,
        manifest: &Value,
    ) -> io::Result<()> {
        debit(
            self.ledger_mut()?,
            "admission-manifest",
            0,
            Candidate::manifest_state_bytes(manifest)?,
        )?;
        let remaining = self.ledger()?.remaining().map_err(command)?;
        candidate.restrict_remaining_state(remaining.state_bytes)
    }
    /// Account an attempted suffix even when the original physical ledger is
    /// sticky-failed. This grants no further read or completion authority.
    fn account_spooled_terminal_io(&mut self, candidate: &SpoolCandidate<'_>) -> io::Result<()> {
        let original = &self
            .spooled_profile
            .as_ref()
            .ok_or_else(|| invalid("spooled original budget absent"))?
            .2;
        if !candidate.shares_io_budget(original) {
            return Err(invalid("spooled terminal original budget differs"));
        }
        self.account_spooled_terminal_budget()
    }

    /// Check the distinct local read ledgers and their common original write
    /// authority selected by this invocation.
    pub(crate) fn verify_spooled_v2_io(
        &self,
        spool: &PinnedSqliteIoBudget,
        v2: &PinnedSqliteIoBudget,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> io::Result<()> {
        active(self.deadline, self.cancel)?;
        self.ledger()?;
        if deadline != self.deadline
            || !std::ptr::eq(cancel, self.cancel)
            || !self.spooled_route_selected
            || !spool.shares_write_authority_with(v2)
            || self
                .spooled_profile
                .as_ref()
                .is_none_or(|profile| !profile.2.shares_with(spool))
            || self
                .segment_v2_profile
                .as_ref()
                .is_none_or(|profile| !profile.io.shares_with(v2))
        {
            return Err(invalid("spooled/V2 original invocation IO binding differs"));
        }
        Ok(())
    }

    /// Observe both selected local read ledgers and their single original
    /// cumulative-write authority for runtime refusal evidence.
    pub(crate) fn spooled_invocation_io_snapshot(&self) -> io::Result<NativeInvocationIoSnapshot> {
        let spool = &self
            .spooled_profile
            .as_ref()
            .ok_or_else(|| invalid("spooled original budget absent"))?
            .2;
        let spool_snapshot = spool.snapshot();
        let mut usage = spool_snapshot;
        let mut segment_snapshot = None;
        let writes = spool.shared_write_snapshot();
        if let Some(v2) = self.segment_v2_profile.as_ref() {
            if !spool.shares_write_authority_with(&v2.io) {
                return Err(invalid("spooled/V2 original write authority differs"));
            }
            let segment = v2.io.snapshot();
            segment_snapshot = Some(segment);
            usage.read_attempted_bytes = usage
                .read_attempted_bytes
                .checked_add(segment.read_attempted_bytes)
                .ok_or_else(|| invalid("invocation read snapshot overflow"))?;
            usage.read_permitted_bytes = usage
                .read_permitted_bytes
                .checked_add(segment.read_permitted_bytes)
                .ok_or_else(|| invalid("invocation read snapshot overflow"))?;
            usage.read_returned_bytes = usage
                .read_returned_bytes
                .checked_add(segment.read_returned_bytes)
                .ok_or_else(|| invalid("invocation read snapshot overflow"))?;
            usage.read_upper_bound_attempted_bytes = usage
                .read_upper_bound_attempted_bytes
                .checked_add(segment.read_upper_bound_attempted_bytes)
                .ok_or_else(|| invalid("invocation read snapshot overflow"))?;
            usage.failure = usage.failure.or(segment.failure);
        }
        usage.write_attempted_bytes = writes.write_attempted_bytes;
        usage.write_permitted_bytes = writes.write_permitted_bytes;
        usage.write_returned_bytes = writes.write_returned_bytes;
        usage.failure = writes.failure.or(usage.failure);
        // Aggregate counters are arithmetic observations, not another read
        // authority. Its distinct read ceilings remain in the raw snapshots.
        usage.read_limit_bytes = None;
        usage.read_limit_origin = None;
        usage.local_failure = None;
        Ok(NativeInvocationIoSnapshot {
            aggregate: usage,
            spool: spool_snapshot,
            segment_v2: segment_snapshot,
        })
    }

    /// Reserve caller-held source-operation state on the existing invocation
    /// ledger before an external bounded preparation allocates it. The debit
    /// remains monotonic through candidate validation and publication.
    pub(crate) fn reserve_spooled_external_state(
        &mut self,
        bytes: usize,
        io: &PinnedSqliteIoBudget,
    ) -> io::Result<()> {
        active(self.deadline, self.cancel)?;
        // The capacity producer reads its authored selection through the
        // selected V2 slice; other preparations use the candidate spool slice.
        // Both retain state on this invocation's ledger. Require exact IO
        // identity with one of its issued slices, never merely shared writes.
        let selected_io = self
            .spooled_profile
            .as_ref()
            .is_some_and(|profile| profile.2.shares_with(io))
            || self
                .segment_v2_profile
                .as_ref()
                .is_some_and(|profile| profile.io.shares_with(io));
        if bytes == 0 || bytes == usize::MAX || !self.spooled_route_selected || !selected_io {
            return Err(invalid("spooled external state original owner differs"));
        }
        debit(
            self.ledger_mut()?,
            "admission-spooled-external-state",
            0,
            bytes,
        )
    }

    /// Account the same original physical ledger when a spooled operation
    /// refuses before a candidate exists or after publication has retired it.
    /// This carries no new IO authority; it only records the attempted suffix.
    pub(crate) fn account_spooled_external_io(&mut self) -> io::Result<()> {
        if !self.spooled_route_selected
            || self
                .shared_cancel
                .as_ref()
                .is_none_or(|cancelled| !std::ptr::eq(self.cancel, Arc::as_ptr(cancelled)))
        {
            return Err(invalid("spooled external accounting owner differs"));
        }
        self.account_spooled_terminal_budget()
    }

    fn account_spooled_terminal_budget(&mut self) -> io::Result<()> {
        let original = &self
            .spooled_profile
            .as_ref()
            .ok_or_else(|| invalid("spooled original budget absent"))?
            .2;
        let usage = original.snapshot();
        let aggregate_write = original.shared_write_snapshot();
        let selected_write_cap = self
            .spooled_write_cap
            .ok_or_else(|| invalid("spooled selected write authority absent"))?;
        let candidate_read = usage
            .read_attempted_bytes
            .checked_sub(self.candidate_io.0)
            .ok_or_else(|| invalid("spooled terminal read accounting regressed"))?;
        let spool_upper = usage
            .read_upper_bound_attempted_bytes
            .checked_sub(self.spooled_read_upper_accounted)
            .ok_or_else(|| invalid("spooled terminal upper-bound accounting regressed"))?;
        let candidate_read = candidate_read
            .checked_sub(spool_upper)
            .ok_or_else(|| invalid("spooled upper-bound suffix exceeds attempted suffix"))?;
        let _candidate_write_suffix = usage
            .write_attempted_bytes
            .checked_sub(self.candidate_io.1)
            .ok_or_else(|| invalid("spooled terminal write accounting regressed"))?;
        let segment_v2_usage = self
            .segment_v2_profile
            .as_ref()
            .map(|profile| profile.io.snapshot());
        let segment_v2_read_attempted = match segment_v2_usage {
            Some(segment) => segment
                .read_attempted_bytes
                .checked_sub(self.segment_v2_io_accounted.0)
                .ok_or_else(|| invalid("V2 terminal read accounting regressed"))?,
            None if self.segment_v2_io_accounted == (0, 0) => 0,
            None => return Err(invalid("V2 terminal IO profile disappeared")),
        };
        let segment_v2_upper = match segment_v2_usage {
            Some(segment) => segment
                .read_upper_bound_attempted_bytes
                .checked_sub(self.segment_v2_read_upper_accounted)
                .ok_or_else(|| invalid("V2 terminal upper-bound read accounting regressed"))?,
            None if self.segment_v2_read_upper_accounted == 0 => 0,
            None => return Err(invalid("V2 terminal upper-bound profile disappeared")),
        };
        let segment_v2_read = segment_v2_read_attempted
            .checked_sub(segment_v2_upper)
            .ok_or_else(|| invalid("V2 upper-bound read suffix exceeds attempted suffix"))?;
        let read = candidate_read
            .checked_add(segment_v2_read)
            .ok_or_else(|| invalid("spooled terminal read suffix overflow"))?;
        let upper = spool_upper
            .checked_add(segment_v2_upper)
            .ok_or_else(|| invalid("spooled/V2 upper-bound suffix overflow"))?;
        let measured_before = self.ledger()?.measured_charged().source_read_bytes;
        let upper_before = self.ledger()?.admitted_charged().source_read_bytes;
        let classified = self
            .ledger_mut()?
            .record_terminal_source_read_suffix(read, upper);
        let measured_after = self.ledger()?.measured_charged().source_read_bytes;
        let upper_after = self.ledger()?.admitted_charged().source_read_bytes;
        let measured_recorded = measured_after.checked_sub(measured_before);
        let upper_recorded = upper_after.checked_sub(upper_before);
        // Both classifications must be retained before their shared physical
        // attempted prefix advances. Arithmetic overflow in either ledger
        // leaves the old witness intact for a later exact refusal.
        if measured_recorded == Some(read) && upper_recorded == Some(upper) {
            self.candidate_io = (usage.read_attempted_bytes, usage.write_attempted_bytes);
            self.spooled_read_upper_accounted = usage.read_upper_bound_attempted_bytes;
            if let Some(segment) = segment_v2_usage {
                self.segment_v2_io_accounted =
                    (segment.read_attempted_bytes, segment.write_attempted_bytes);
                self.segment_v2_read_upper_accounted = segment.read_upper_bound_attempted_bytes;
            }
        }
        classified.map_err(command)?;
        if usage.read_permitted_bytes > usage.read_attempted_bytes
            || usage.read_returned_bytes > usage.read_permitted_bytes
            || usage.write_permitted_bytes > usage.write_attempted_bytes
            || usage.write_returned_bytes > usage.write_permitted_bytes
            || aggregate_write.write_attempted_bytes > selected_write_cap
            || aggregate_write.write_permitted_bytes > aggregate_write.write_attempted_bytes
            || aggregate_write.write_returned_bytes > aggregate_write.write_permitted_bytes
            || aggregate_write.failure.is_some()
            || usage
                .write_attempted_bytes
                .checked_add(segment_v2_usage.map_or(0, |segment| segment.write_attempted_bytes))
                != Some(aggregate_write.write_attempted_bytes)
            || usage
                .write_permitted_bytes
                .checked_add(segment_v2_usage.map_or(0, |segment| segment.write_permitted_bytes))
                != Some(aggregate_write.write_permitted_bytes)
            || usage
                .write_returned_bytes
                .checked_add(segment_v2_usage.map_or(0, |segment| segment.write_returned_bytes))
                != Some(aggregate_write.write_returned_bytes)
            || usage.failure.is_some()
        {
            return Err(invalid("spooled terminal physical IO accounting refused"));
        }
        if let Some(segment) = segment_v2_usage
            && (segment.read_permitted_bytes > segment.read_attempted_bytes
                || segment.read_returned_bytes > segment.read_permitted_bytes
                || segment.write_permitted_bytes > segment.write_attempted_bytes
                || segment.write_returned_bytes > segment.write_permitted_bytes
                || segment.failure.is_some())
        {
            return Err(invalid("V2 terminal physical IO accounting refused"));
        }
        Ok(())
    }

    fn retain_spooled_refusal(
        &mut self,
        candidate: &SpoolCandidate<'_>,
        primary: io::Error,
        phase: &'static str,
    ) -> io::Error {
        let before = self.spooled_invocation_io_snapshot();
        let accounting = self.account_spooled_terminal_io(candidate);
        if accounting.is_ok() {
            return primary;
        }
        // Failed phases may poison the ledger. Retain the primary cause and
        // the accounting failure separately, as the outer CLI already does.
        match (before, self.spooled_invocation_io_snapshot()) {
            (Ok(before), Ok(after)) => io::Error::other(NativeSpoolRefusal::retain(
                primary, phase, before, after, true, false,
            )),
            _ => primary,
        }
    }

    pub(crate) fn validate_spooled<'candidate>(
        &mut self,
        candidate: &'candidate SpoolCandidate<'_>,
        selected_index: SpoolIndexLimits,
    ) -> io::Result<crate::source_admission_spooled_index::IndexView<'candidate>> {
        let result = self.validate_spooled_inner(candidate, selected_index);
        // Early raw/history/metadata refusals can happen before bootstrap owns
        // a phase window. Preserve that attempted suffix under the original
        // ledger as well; never regrant a completed view after an error.
        match result {
            Err(primary) if self.prepared.is_some() => Err(self.retain_spooled_refusal(
                candidate,
                primary,
                "native-v4 whole foundation validation",
            )),
            other => other,
        }
    }

    fn validate_spooled_inner<'candidate>(
        &mut self,
        candidate: &'candidate SpoolCandidate<'_>,
        selected_index: SpoolIndexLimits,
    ) -> io::Result<crate::source_admission_spooled_index::IndexView<'candidate>> {
        use crate::source_admission_candidate_records::CandidateRecordsInput;
        use crate::source_current_cut::foundation_bootstrap::{
            CandidateFoundationBootstrapInputs, adopt_candidate_shared_io_with_budget,
            verify_candidate_original_epoch_with_budget,
        };
        let (_, defaults, original_io) = self.selected_spooled_profile(selected_index)?;
        let defaults = *defaults;
        let original_io = original_io.clone();
        let (workspace, space) = self
            .spooled_workspace
            .as_ref()
            .ok_or_else(|| invalid("selected spooled workspace authority absent"))?;
        let cancelled = self
            .shared_cancel
            .as_ref()
            .ok_or_else(|| invalid("selected shared cancellation authority absent"))?;
        candidate.verify_prepared_request(
            workspace,
            &original_io,
            space,
            self.deadline,
            cancelled,
        )?;
        self.account_spooled_candidate(candidate)?;
        let selected = &self
            .prepared
            .as_ref()
            .ok_or_else(|| invalid("candidate foundation phase already consumed"))?
            .invocation
            .budgets;
        let max_member = count(selected.max_member_bytes)?;
        let max_members = count(selected.max_current_members)?;
        // Every edge endpoint is a current member: bind checks membership,
        // references resolve that table or check the literal member, and the
        // semantic first pass checks every replay path. edge skips self pairs
        // and deduplicates ordered pairs. This is a structural ceiling only.
        let max_edges = max_members
            .checked_mul(max_members.saturating_sub(1))
            .map(|n| n.max(1))
            .filter(|n| *n != usize::MAX)
            .ok_or_else(|| invalid("native dependency structural ceiling overflow"))?;
        let working_ram = count(selected.working_ram_bytes)?;
        let base = candidate.borrowed_base_declared_retained_state_bytes()?;
        let base_state = base
            .0
            .checked_add(base.1)
            .ok_or_else(|| invalid("borrowed base declared state overflow"))?;
        self.account_spooled_candidate(candidate)?;
        // Find the genuine largest current row through an indexed bounded
        // metadata pass. Raw coverage is earned separately below.
        let mut after = None;
        let mut observed_max = 1usize;
        let mut observed_max_locator = 0usize;
        while let Some(member) =
            candidate.member_after_bounded(after.as_ref(), selected_index.max_row_state_bytes)?
        {
            let bytes = count(member.size_bytes)?;
            if bytes > max_member {
                return Err(invalid("candidate member exceeds selected source ceiling"));
            }
            observed_max = observed_max.max(bytes);
            observed_max_locator = observed_max_locator.max(member.path.as_str().len());
            after = Some(member.path);
        }
        drop(after);
        self.account_spooled_candidate(candidate)?;
        let remaining = self.ledger()?.remaining().map_err(command)?;
        let mut operation_state = remaining.state_bytes;
        // The original operation remainder bounds the simultaneous callback
        // plus raw-member overlap. The spool's selected row ceiling continues
        // to bound local raw/SQL workspace separately; a schema constructor is
        // not a SQL row allocation.
        let mut input_state = operation_state;
        let raw_state = observed_max
            .checked_mul(4)
            .and_then(|n| n.checked_add(16384))
            .ok_or_else(|| invalid("candidate raw callback state overflow"))?;
        // for_each_verified_member retains the previous locator (16x), then
        // preallocates the current SQL row (16x + 1024) and checks the current
        // locator overlap (32x). Reserve their simultaneous upper bound from
        // these actual selected paths before lending any remainder to callback.
        // Otherwise its raw + callback baseline consumes input_state exactly,
        // leaving zero for the first metadata row even under a valid profile.
        let metadata_state = observed_max_locator
            .checked_mul(48)
            .and_then(|n| n.checked_add(1024))
            .ok_or_else(|| invalid("candidate member metadata state overflow"))?;
        let callback_reserve = raw_state
            .checked_add(metadata_state)
            .ok_or_else(|| invalid("candidate raw/member callback state overflow"))?;
        let mut callback_state = input_state
            .checked_sub(callback_reserve)
            .ok_or_else(|| invalid("candidate raw/member callback exceeds selected state"))?;
        let held_declared = candidate
            .own_retained_state_upper_bound_bytes()?
            .checked_add(base_state)
            .and_then(|n| n.checked_add(selected_index.cache_bytes))
            .and_then(|n| n.checked_add(defaults.cache_bytes))
            .and_then(|n| n.checked_add(operation_state))
            .and_then(|n| n.checked_add(size_of::<NativeAdmissionComplete>()))
            .ok_or_else(|| invalid("native simultaneous declared state overflow"))?;
        if held_declared > working_ram {
            return Err(invalid("native declared working envelope exceeded"));
        }
        // Opaque SQLite heap/pins and process fit retain the original external
        // Host/controller prerequisite; nominal cache is not an RSS bound.
        self.verify_record_selection()?;
        // A local shared owner keeps the immutable model alive while mutable
        // validator accounting advances; this clones no manifest rows.
        let record_selection = self.record_selection.clone();
        let generated_selection = if self.generated_input.is_some() {
            use crate::source_admission_generated_selection::GeneratedCandidateSelectionV1;
            use crate::source_capacity_workload::{
                WeightedScaleGeneratedAllV1, WeightedScaleTemplateSetV1,
            };
            use tos_validation::record_biblio_cut::GeneratedSourceSelection;
            let issued = candidate
                .selected_generated_declaration_v1()?
                .ok_or_else(|| invalid("generated candidate issued declaration absent"))?;
            let finite = record_selection
                .as_ref()
                .ok_or_else(|| invalid("generated candidate finite selection absent"))?;
            let root = self
                .prepared
                .as_ref()
                .and_then(|prepared| prepared.launch.arguments.repo_root.as_deref())
                .ok_or_else(|| invalid("generated template source root absent"))?;
            let work = candidate.admission_work_budget()?;
            // This is the unallocated callback remainder, not total process RAM.
            // The finite model and Native baseline were already debited above.
            let templates = WeightedScaleTemplateSetV1::load_from_selected_declaration_accounted(
                root,
                issued,
                &original_io,
                self.deadline,
                self.cancel,
                &work,
                input_state,
                0,
            )?;
            let fence = candidate.fence()?;
            let provider = WeightedScaleGeneratedAllV1::from_selected_declaration(
                issued,
                templates,
                finite.clone(),
                fence,
                input_state,
                0,
                work,
                self.deadline,
                self.cancel,
            )?;
            let wrapper = GeneratedCandidateSelectionV1::new(provider, fence)?;
            let owned = wrapper
                .retained_state_bytes()
                .checked_add(2 * size_of::<usize>())
                .ok_or_else(|| invalid("generated selection Arc state overflow"))?;
            debit(
                self.ledger_mut()?,
                "generated-candidate-retained-state",
                0,
                owned,
            )?;
            operation_state = self.ledger()?.remaining().map_err(command)?.state_bytes;
            input_state = operation_state;
            callback_state = input_state
                .checked_sub(callback_reserve)
                .ok_or_else(|| invalid("generated callback exceeds remaining selected state"))?;
            Some(Arc::new(wrapper))
        } else {
            None
        };
        let input =
            CandidateRecordsInput::new(candidate, observed_max, input_state, callback_state)
                .map_err(|_| invalid("candidate callback source profile refused"))?
                .with_record_selection(record_selection)
                .with_generated_selection(generated_selection);
        if let Some(history) = &mut self.history {
            history.bind_candidate_io_budget(&input, &original_io)?;
        }
        let coverage = self.grammar.verify_candidate_input(&input, self.cancel)?;
        if let Some(history) = &self.history {
            history.verify_candidate_input(&input, &coverage, self.cancel)?;
        }
        self.account_spooled_candidate(candidate)?;
        let prepared = self
            .prepared
            .take()
            .ok_or_else(|| invalid("candidate foundation phase already consumed"))?;
        let remaining_write = self
            .write_cap
            .checked_sub(self.candidate_io.1)
            .ok_or_else(|| invalid("candidate persistent write ceiling exceeded"))?;
        let preparation = CandidateFoundationBootstrapInputs::prepare(
            prepared.clock,
            prepared.launch,
            prepared.invocation,
            prepared.ledger,
            &input,
            coverage,
            &original_io,
            remaining_write,
            &mut self.candidate_io,
            prepared.sources,
        );
        let mut inputs = match preparation {
            Ok(inputs) => inputs,
            Err(failed) => {
                self.prepared = Some(Prepared {
                    clock: failed.clock,
                    launch: failed.launch,
                    invocation: failed.invocation,
                    ledger: failed.remaining_budget,
                    sources: failed.sources,
                });
                // The outer validator accounts this restored failed phase
                // without replacing its primary refusal with a finalizer error.
                let reason =
                    foundation_orchestrator::FoundationOrchestratorError::Bootstrap(failed.error)
                        .public_reason();
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    NativeValidationRefusal(reason),
                ));
            }
        };
        let mut json = JsonLimits::default();
        json.max_bytes = max_member;
        let json_state = observed_max
            .checked_mul(8)
            .ok_or_else(|| invalid("native JSON workspace overflow"))?;
        let native_limits = IndexLimits {
            max_edges,
            max_state_bytes: operation_state,
        };
        let validation_scope = self.validation_profile.scope;
        let history = self
            .history
            .as_mut()
            .map(|h| h as &mut dyn FoundationHistoricalEvidence);
        let result = inputs
            .with_initial_candidate_snapshots_with_history(
                self.git_signal,
                &input,
                history,
                |view, payloads| {
                    foundation_orchestrator::evaluate_spooled_admission(
                        view,
                        payloads,
                        &input,
                        candidate,
                        selected_index,
                        native_limits,
                        defaults,
                        json,
                        json_state,
                        callback_state,
                        base_state,
                        validation_scope,
                    )
                },
            )
            .map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    NativeValidationRefusal(error.public_reason()),
                )
            });
        let result = result.and_then(|earned| {
            let final_coverage = self.grammar.verify_candidate_input(&input, self.cancel)?;
            if let Some(history) = &self.history {
                history.verify_candidate_input(&input, &final_coverage, self.cancel)?;
            }
            let mut write = self
                .write_cap
                .checked_sub(inputs.candidate_io_adopted.1)
                .ok_or_else(|| invalid("candidate persistent write ceiling exceeded"))?;
            adopt_candidate_shared_io_with_budget(
                &original_io,
                &mut inputs.candidate_io_adopted,
                &mut write,
                &mut inputs.remaining_budget,
            )
            .map_err(|_| invalid("candidate final raw IO adoption refused"))?;
            verify_candidate_original_epoch_with_budget(
                &mut inputs.sources,
                &inputs.original_epoch,
                &mut inputs.execution_limits,
                &mut inputs.remaining_budget,
            )
            .map_err(|_| invalid("candidate final original epoch refused"))?;
            Ok(earned)
        });
        self.candidate_io = inputs.candidate_io_adopted;
        let original_epoch = inputs.original_epoch;
        self.prepared = Some(Prepared {
            clock: inputs.clock,
            launch: inputs.launch,
            invocation: inputs.invocation,
            ledger: inputs.remaining_budget,
            sources: inputs.sources,
        });
        let result = result.and_then(|earned| {
            self.verify_record_selection()?;
            Ok(earned)
        });
        // Attempted suffixes remain visible even if any final fence failed.
        let terminal = self.account_spooled_terminal_io(candidate);
        let (sink, records) = result?;
        terminal?;
        self.account_spooled_candidate(candidate)?;
        let (workspace, space) = self
            .spooled_workspace
            .take()
            .ok_or_else(|| invalid("selected spooled workspace authority absent"))?;
        let cancelled = self
            .shared_cancel
            .as_ref()
            .ok_or_else(|| invalid("selected shared cancellation authority absent"))?
            .clone();
        let complete = NativeAdmissionComplete::new(
            records,
            selected_index,
            &original_io,
            original_epoch,
            workspace,
            space,
            self.segment_v2_profile.clone(),
            self.deadline,
            cancelled,
        );
        let finished = sink.finish(complete);
        // finish authenticates the index membership EOF under the same ledger;
        // its attempted suffix is charged on both success and refusal.
        let terminal = self.account_spooled_terminal_io(candidate);
        let view = finished?;
        terminal?;
        // The joined candidate phases already charged runtime history attempts
        // and retained state. Keep the owner totals for later custody checks.
        if let Some(history) = &self.history {
            self.history_usage = (history.read_bytes(), history.retained_state_bytes());
        }
        Ok(view)
    }

    pub(crate) fn validate(&mut self, candidate: &Candidate<'_>) -> io::Result<ValidatedCandidate> {
        if self.spooled_route_selected {
            return Err(invalid(
                "native-v4 cannot use resident candidate validation",
            ));
        }
        self.grammar.verify_candidate(candidate)?;
        if let Some(history) = &self.history {
            history.verify_candidate(candidate)?;
        }
        self.account_candidate(candidate)?;
        let prepared = self
            .prepared
            .take()
            .ok_or_else(|| invalid("candidate foundation phase already consumed"))?;
        let mut inputs = FoundationBootstrapInputs::prepare_candidate(
            prepared.clock,
            prepared.launch,
            prepared.invocation,
            prepared.ledger,
            candidate,
            self.git_signal,
            prepared.sources,
            self.candidate_io,
        )
        .map_err(|_| invalid("candidate foundation bootstrap refused"))?;
        let baseline = inputs
            .cost
            .candidate_io_after_capture
            .ok_or_else(|| invalid("candidate capture accounting absent"))?;
        if baseline.0 < self.candidate_io.0
            || baseline.1 < self.candidate_io.1
            || baseline.1 > self.write_cap
        {
            return Err(invalid("candidate capture accounting invalid"));
        }
        self.candidate_io = baseline;
        self.candidate_state = inputs
            .cost
            .candidate_state_after_capture
            .ok_or_else(|| invalid("candidate capture state accounting absent"))?;
        let available = inputs.remaining_budget.remaining().map_err(command)?;
        candidate
            .restrict_remaining_io(available.source_read_bytes, self.write_cap - baseline.1)?;
        let mut json = JsonLimits::default();
        json.max_bytes = count(inputs.invocation.budgets.max_member_bytes)?;
        let history = self
            .history
            .as_mut()
            .map(|h| h as &mut dyn FoundationHistoricalEvidence);
        let write_cap = self.write_cap;
        let index = inputs
            .with_initial_snapshots_with_history(self.git_signal, history, move |view, payloads| {
                foundation_orchestrator::evaluate_admission(
                    view,
                    payloads,
                    move |fresh, schemas, available: FoundationPhaseReservation| {
                        candidate.restrict_remaining_io(
                            available.source_read_bytes,
                            write_cap
                                .checked_sub(candidate.io_usage().1)
                                .ok_or_else(|| invalid("persistent write budget exceeded"))?,
                        )?;
                        // The growing index coexists with the candidate's
                        // read-set and one decoded document. Reserve their
                        // shared peak before either consumer allocates; only
                        // actual retained state is debited on completion.
                        let read_set = candidate.unread_path_state_bytes()?;
                        let parser_and_index = available
                            .state_bytes
                            .checked_sub(read_set)
                            .and_then(|n| n.checked_sub(std::mem::size_of::<Index>()))
                            .ok_or_else(|| {
                                invalid("candidate index workspace exceeds state budget")
                            })?;
                        let (scratch, json, json_state_bytes) =
                            candidate.index_scratch_state_bytes(json, parser_and_index)?;
                        let workspace = read_set
                            .checked_add(scratch)
                            .ok_or_else(|| invalid("candidate index workspace overflow"))?;
                        let index_state = available
                            .state_bytes
                            .checked_sub(workspace)
                            .filter(|n| *n >= std::mem::size_of::<Index>())
                            .ok_or_else(|| {
                                invalid("candidate index and document exceed shared state budget")
                            })?;
                        // Keep the monotone operation bound: workspace is a
                        // temporary peak reservation, not a permanent debit.
                        candidate.restrict_remaining_state(available.state_bytes)?;
                        let index_limits = IndexLimits {
                            max_edges: index_state / std::mem::size_of::<String>(),
                            max_state_bytes: index_state,
                        };
                        let before_io = candidate.io_usage();
                        let before_state = candidate.retained_state_bytes();
                        let mut read = |path: &str, cap| candidate.read(path, cap);
                        let mut verify = |path: &str| candidate.verify(path);
                        let mut check = || candidate.tick();
                        let mut source = CandidateInput {
                            members: &candidate.members,
                            read: &mut read,
                            verify_member: &mut verify,
                            check: &mut check,
                            json,
                            json_state_bytes,
                        };
                        source_admission_index::validate_retirements(
                            &mut source,
                            &candidate.new_retirements,
                            candidate.base.as_ref(),
                            schemas,
                        )?;
                        let (index, index_state) = source_admission_index::build_index_accounted(
                            &mut source,
                            &fresh,
                            candidate.base_index.as_ref(),
                            index_limits,
                            schemas,
                        )?;
                        let read_delta = candidate
                            .io_usage()
                            .0
                            .checked_sub(before_io.0)
                            .ok_or_else(|| invalid("candidate index read accounting regressed"))?;
                        let retained_delta = candidate
                            .retained_state_bytes()
                            .checked_sub(before_state)
                            .and_then(|n| n.checked_add(index_state))
                            .ok_or_else(|| invalid("candidate index state accounting overflow"))?;
                        Ok((
                            index,
                            FoundationPhaseUse {
                                source_read_bytes: FoundationCharge::measured(read_delta),
                                state_bytes: FoundationCharge::admitted_upper_bound(retained_delta),
                                ..FoundationPhaseUse::default()
                            },
                        ))
                    },
                )
            })
            // Preserve the joined owner's bounded, source-free phase label;
            // discarding it turns every real integration refusal into the
            // same opaque error at the installed admission entrypoint.
            .map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    NativeValidationRefusal(error.public_reason()),
                )
            })?;
        // Host's callback window already charged index reads and retained
        // state. Adopt its successful terminal Candidate counters exactly once.
        self.candidate_io = candidate.io_usage();
        self.candidate_state = candidate.retained_state_bytes();
        // Host debits the provider deltas and final historical custody in the
        // same phase ledger. Adopt its terminal totals without a second debit.
        if let Some(history) = &self.history {
            self.history_usage = (history.read_bytes(), history.retained_state_bytes());
        }
        let remaining = inputs.remaining_budget.remaining().map_err(command)?;
        let read = self.grammar.recheck(
            &mut inputs.sources,
            remaining.source_read_bytes,
            remaining.state_bytes,
            self.cancel,
        )?;
        debit(
            &mut inputs.remaining_budget,
            "admission-grammar-final",
            read,
            0,
        )?;
        // Host owns the charged invocation/worker EOF verification; do not
        // repeat its successful image hash here. Grammar recheck above is new IO.
        candidate.tick()?;
        self.evaluated = Some(inputs);
        self.account_candidate(candidate)?;
        Ok(ValidatedCandidate(index))
    }
    pub(crate) fn take_indexed_input_declaration_v1(
        &mut self,
    ) -> io::Result<Option<crate::source_admission_indexed_input::HeldIndexedInputDeclarationV1>>
    {
        let Some(early) = &mut self.generated_input else {
            return Ok(None);
        };
        early
            .declaration
            .as_ref()
            .ok_or_else(|| invalid("generated input declaration already consumed"))?
            .verify_v1(&early.io, self.deadline, self.cancel)?;
        early
            .declaration
            .take()
            .map(Some)
            .ok_or_else(|| invalid("generated input declaration already consumed"))
    }
    pub(crate) fn remaining_output_bytes(&self) -> io::Result<usize> {
        Ok(self.ledger()?.remaining().map_err(command)?.output_bytes)
    }
    fn verify_record_selection(&mut self) -> io::Result<()> {
        let Some(held) = &self.record_selection_held else {
            return Ok(());
        };
        let read = u64::try_from(held.size_bytes())
            .ok()
            .and_then(|n| n.checked_mul(2))
            .ok_or_else(|| invalid("record selection verification read overflow"))?;
        let verify_state = usize::try_from(read)
            .ok()
            .and_then(|n| n.checked_add(size_of::<crate::source_text_owner::HeldOwnerFile>()))
            .ok_or_else(|| invalid("record selection custody verification state overflow"))?;
        if verify_state > self.ledger()?.remaining().map_err(command)?.state_bytes {
            return Err(invalid("record selection custody verification state bound"));
        }
        let ticket = self
            .ledger_mut()?
            .begin_window(
                "record-selection-custody",
                FoundationPhaseReservation::default(),
            )
            .map_err(command)?;
        self.ledger_mut()?
            .complete_window(
                ticket,
                FoundationPhaseUse {
                    source_read_bytes: FoundationCharge::admitted_upper_bound(read),
                    ..FoundationPhaseUse::default()
                },
            )
            .map_err(command)?;
        let held = self
            .record_selection_held
            .as_ref()
            .expect("selection custody held");
        crate::source_text_owner::verify_held_file(
            held,
            unsafe { libc::geteuid() },
            self.deadline,
            self.cancel,
        )
        .map_err(command)
    }
    pub(crate) fn write_receipt(
        &mut self,
        value: &Value,
        writer: &mut dyn Write,
    ) -> io::Result<()> {
        active(self.deadline, self.cancel)?;
        self.verify_record_selection()?;
        let object = value
            .as_object()
            .ok_or_else(|| invalid("admission receipt must be an object"))?;
        if object.contains_key("validation_profile_id")
            || object.contains_key("validation_profile_declaration_sha256")
            || object.contains_key("validation_input_scope")
            || object.contains_key("record_selection")
            || object.contains_key("generated_input_identity_binding_json")
            || object.contains_key("generated_input_identity_binding_sha256")
        {
            return Err(invalid(
                "admission receipt cannot override validator profile",
            ));
        }
        #[derive(serde::Serialize)]
        struct ProfileReceipt<'a> {
            #[serde(flatten)]
            receipt: &'a Value,
            validation_profile_id: &'a str,
            validation_input_scope: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            record_selection: Option<&'a Value>,
            // Exact owner-serialized canonical binding, retained once and
            // borrowed here. It includes the issued tuple, not physical EOF.
            #[serde(skip_serializing_if = "Option::is_none")]
            generated_input_identity_binding_json: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            generated_input_identity_binding_sha256: Option<String>,
            validation_profile_declaration_sha256: String,
        }
        let ticket = self
            .ledger_mut()?
            .begin_window("admission-receipt", FoundationPhaseReservation::default())
            .map_err(command)?;
        let cap = ticket.remaining().output_bytes;
        let value = ProfileReceipt {
            receipt: value,
            validation_profile_id: self.validation_profile.id,
            validation_input_scope: self.validation_profile.input_scope,
            record_selection: self.record_selection_binding.as_ref(),
            generated_input_identity_binding_json: self
                .generated_input
                .as_ref()
                .and_then(|early| early.identity_binding.as_deref()),
            generated_input_identity_binding_sha256: self
                .generated_input
                .as_ref()
                .and_then(|early| early.identity_binding.as_ref())
                .map(|binding| Digest256::of_bytes(binding.as_bytes()).to_hex()),
            validation_profile_declaration_sha256: self
                .validation_profile
                .declaration_sha256
                .to_hex(),
        };
        // Same serializer counts before emitting; no unbounded intermediate Vec.
        let mut count = ReceiptWriter {
            inner: None,
            count: 0,
            cap,
            deadline: self.deadline,
            cancel: self.cancel,
        };
        serde_json::to_writer(&mut count, &value).map_err(invalid)?;
        count.write_all(b"\n")?;
        let mut output = ReceiptWriter {
            inner: Some(writer),
            count: 0,
            cap,
            deadline: self.deadline,
            cancel: self.cancel,
        };
        serde_json::to_writer(&mut output, &value).map_err(invalid)?;
        output.write_all(b"\n")?;
        output.flush()?;
        let bytes = output.count;
        self.ledger_mut()?
            .complete_window(
                ticket,
                FoundationPhaseUse {
                    output_bytes: FoundationCharge::measured(bytes),
                    ..FoundationPhaseUse::default()
                },
            )
            .map_err(command)
    }
}
struct ReceiptWriter<'a> {
    inner: Option<&'a mut dyn Write>,
    count: usize,
    cap: usize,
    deadline: Instant,
    cancel: &'a AtomicBool,
}
impl Write for ReceiptWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        active(self.deadline, self.cancel)?;
        if bytes.len() > self.cap.saturating_sub(self.count) {
            return Err(invalid("admission receipt exceeds output budget"));
        }
        let n = match &mut self.inner {
            Some(writer) => writer.write(bytes)?,
            None => bytes.len(),
        };
        if n > bytes.len() {
            return Err(invalid("admission receipt writer returned invalid count"));
        }
        self.count += n;
        active(self.deadline, self.cancel)?;
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        active(self.deadline, self.cancel)?;
        if let Some(writer) = &mut self.inner {
            writer.flush()?;
        }
        active(self.deadline, self.cancel)
    }
}

#[cfg(test)]
mod refusal_transport_tests {
    use super::*;

    #[test]
    fn opaque_error_fingerprint_is_bounded_and_does_not_export_text() {
        let text = "/private/sentinel: publication failed";
        assert_eq!(
            bounded_error_sha256(&io::Error::other(text)),
            Some(tos_foundation::Digest256::of_bytes(text.as_bytes()).to_hex())
        );
        assert!(bounded_error_sha256(&io::Error::other("x".repeat(4097))).is_none());
        let snapshot = NativeInvocationIoSnapshot::default();
        let refusal = NativeSpoolRefusal::retain(
            io::Error::other(text),
            "native-v4 corpus publication",
            snapshot,
            snapshot,
            false,
            false,
        );
        let encoded = serde_json::to_string(&refusal.packet()).unwrap();
        assert!(!encoded.contains("private/sentinel"));
        assert!(
            serde_json::from_str::<Value>(&encoded).unwrap()["native_validation_reason"].is_null()
        );
    }
}
