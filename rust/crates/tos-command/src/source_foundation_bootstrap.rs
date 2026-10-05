//! Concrete first-phase custody for the maintained source-foundation command.
//!
//! This joins only already selected roots and bounded owner providers. It
//! authenticates neither owner findings nor source admission. The caller keeps
//! this value alive while the callback runs and owns all later worker phases
//! and final EOF ordering.

use super::foundation_capture::{self, FoundationCaptureCost, FoundationCapturedCut};
use super::foundation_entry::{
    FoundationBootstrapClock, FoundationBootstrapCost as EntryBootstrapCost, FoundationInvocation,
    FoundationLaunchArguments, FoundationSelectedRoots,
};
use super::foundation_execution_limits::{
    FoundationBudgetTicket, FoundationCharge, FoundationExecutionLimits,
    FoundationPhaseReservation, FoundationPhaseUse, FoundationRemainingBudget,
    FoundationWindowKind, FoundationWorkerCpuUse,
};
use super::foundation_payload::{
    FoundationPayloadSources, PhysicalPayloadCost, PhysicalPayloadLimits,
};
use super::foundation_physical::{
    FoundationPhysicalSnapshot, PhysicalSourceCost, PhysicalSourceLimits,
};
use super::foundation_selection::{
    self, FoundationPhysicalSelection, FoundationSelectionCost, FoundationSelectionLimits,
};
use crate::source_admission_candidate::Candidate;
use crate::source_command::SourceCommandError;
use crate::source_creation_store::IsolatedCreationRoot;
use std::io;
use std::mem::size_of;
use std::os::unix::ffi::OsStrExt;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};
use std::time::Instant;
use tos_compiler::private_tmpfs_stage::PrivateTmpfsStageIsolation;
use tos_ops_mechanics_plan::route_cards::{MAX_BUDGETED_ROUTE_OPERATIONS, RouteSources};
use tos_source_store::{CutReadLimits, ReadLimits};
use tos_validation::item_rules::ItemRefusal;

const PUBLICATION_CONTROL_READ_CAP_BYTES: usize = 8192;
const CAPTURE_CONTROL_READS: usize = 2;
const CAPTURE_MEMBER_READ_PASSES: usize = 3;
const CAPTURE_TMPFS_FIXED_INODES: usize = 8;
const PAYLOAD_SOURCE_SUFFIX: &str = "ToS/source-witnesses";

/// Caller-derived limits for the first source-foundation phase. These values
/// are checked against the selected invocation reservations before any held
/// root, private stage, or captured source is opened or created.
#[derive(Clone, Copy)]
pub(crate) struct FoundationBootstrapConfig {
    pub read_limits: ReadLimits,
    pub cut_limits: CutReadLimits,
    /// Per-pass authored member ceiling. Bootstrap charges all three passes.
    pub capture_member_bytes_per_pass: usize,
    /// Caller-reserved peak capture state, including source-cut metadata.
    pub capture_state_upper_bound_bytes: usize,
    /// Live full-source path discovery workspace reserved before capture.
    pub capture_discovery_workspace_upper_bound_bytes: usize,
    /// Caller-reserved private-stage write ceiling for capture files.
    pub capture_tmpfs_write_upper_bound_bytes: u64,
    pub selection_limits: FoundationSelectionLimits,
    pub physical_limits: PhysicalSourceLimits,
    pub payload_limits: PhysicalPayloadLimits,
}

/// Error ownership is retained so the command can map bootstrap failures at
/// its boundary without converting typed refusal into a string.
#[derive(Debug)]
pub(crate) enum FoundationBootstrapError {
    Command(SourceCommandError),
    StageSelection(&'static str),
    IsolatedRoot(SourceCommandError),
    RouteRoot(io::Error),
    Capture(SourceCommandError),
    Selection(ItemRefusal),
    Payload(ItemRefusal),
    Physical(ItemRefusal),
    Configuration(&'static str),
}

/// Source-read, state, root-open, and actual owner costs observed by this
/// phase. The three-pass member ceiling and two publication-control ceilings
/// are retained separately; capture's member cost excludes those controls.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct FoundationSourceBootstrapCost {
    pub invocation_after_initial_read: EntryBootstrapCost,
    pub invocation_after_callback: Option<EntryBootstrapCost>,
    pub entry_source_read_bytes: u64,
    pub selected_roots_state_upper_bound_bytes: usize,
    pub payload_root_defaulted: bool,
    pub capture_member_read_upper_bound_bytes: usize,
    pub capture_control_read_upper_bound_bytes: usize,
    pub capture_source_read_upper_bound_bytes: usize,
    pub capture_state_upper_bound_bytes: usize,
    pub capture_tmpfs_write_upper_bound_bytes: u64,
    pub capture_tmpfs_inode_upper_bound: usize,
    /// Full candidate I/O counters already charged through capture, when used.
    /// The admission adapter adopts this baseline rather than charging again.
    pub candidate_io_after_capture: Option<(u64, u64)>,
    pub candidate_state_after_capture: Option<usize>,
    pub capture_after_initial: FoundationCaptureCost,
    pub capture_after_callback: Option<FoundationCaptureCost>,
    pub selection: FoundationSelectionCost,
    pub selection_after_auxiliary_read: Option<FoundationSelectionCost>,
    pub payload_after_snapshot: Option<PhysicalPayloadCost>,
    pub physical_after_snapshot: Option<PhysicalSourceCost>,
    pub physical_after_callback: Option<PhysicalSourceCost>,
    pub shared_route_operations_after_roots: usize,
    pub shared_route_operations_after_capture: usize,
    pub shared_route_operations_after_selection: usize,
    pub shared_route_operations_after_auxiliary_selection: Option<usize>,
    pub shared_route_operations_after_snapshot: usize,
    pub shared_route_operations_after_callback: Option<usize>,
    pub source_root_component_opens_after_roots: usize,
    pub payload_root_component_opens_after_roots: usize,
    pub artifact_root_component_opens_after_roots: Option<usize>,
    pub source_root_component_opens_after_capture: usize,
    pub source_root_component_opens_after_selection: usize,
    pub source_root_component_opens_after_auxiliary_selection: Option<usize>,
    pub source_root_component_opens_after_snapshot: Option<usize>,
    pub payload_root_component_opens_after_snapshot: Option<usize>,
    pub artifact_root_component_opens_after_snapshot: Option<usize>,
    pub source_root_component_opens_after_callback: Option<usize>,
    pub payload_root_component_opens_after_callback: Option<usize>,
    pub artifact_root_component_opens_after_callback: Option<usize>,
    pub whole_budget_after_invocation: FoundationBudgetCost,
    pub whole_budget_after_selected_roots: FoundationBudgetCost,
    pub whole_budget_after_capture: Option<FoundationBudgetCost>,
    pub whole_budget_after_selection: Option<FoundationBudgetCost>,
    pub whole_budget_after_auxiliary_selection: Option<FoundationBudgetCost>,
    pub whole_budget_after_payload_snapshot: Option<FoundationBudgetCost>,
    pub whole_budget_after_physical_snapshot: Option<FoundationBudgetCost>,
    pub whole_budget_after_callback: Option<FoundationBudgetCost>,
}

/// Whole invocation charges split by measured and admitted-upper-bound basis.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct FoundationBudgetCost {
    pub charged: FoundationPhaseReservation,
    pub measured: FoundationPhaseReservation,
    pub admitted_upper_bound: FoundationPhaseReservation,
}

struct FoundationCaptureWindowResult {
    stage: PrivateTmpfsStageIsolation,
    isolated: IsolatedCreationRoot,
    captured: FoundationCapturedCut,
    cost: FoundationCaptureCost,
    member_read_upper_bound_bytes: usize,
    control_read_upper_bound_bytes: usize,
    source_read_upper_bound_bytes: usize,
    state_upper_bound_bytes: usize,
    tmpfs_used_bytes: u64,
    tmpfs_inode_upper_bound: usize,
    candidate_io_after_capture: Option<(u64, u64)>,
    candidate_state_after_capture: Option<usize>,
}

/// Retains real selected roots, execution authority, captured bytes, and the
/// exact selector closure for the complete callback lifetime.
pub(crate) struct FoundationBootstrapInputs<'cancel> {
    pub clock: FoundationBootstrapClock,
    pub launch: FoundationLaunchArguments,
    pub invocation: FoundationInvocation<'cancel>,
    pub execution_limits: FoundationExecutionLimits,
    pub remaining_budget: FoundationRemainingBudget<'cancel>,
    pub selected_roots: FoundationSelectedRoots,
    pub stage: PrivateTmpfsStageIsolation,
    pub isolated: IsolatedCreationRoot,
    pub sources: RouteSources,
    pub payload_sources: RouteSources,
    pub artifact_sources: Option<RouteSources>,
    pub captured: FoundationCapturedCut,
    pub selection: FoundationPhysicalSelection,
    pub cost: FoundationSourceBootstrapCost,
    cancelled: &'cancel AtomicBool,
    config: FoundationBootstrapConfig,
}

/// Short-lived view passed to the owner callback. It gives the caller the
/// actual invocation so it can perform `verify_before_source_eof` after every
/// worker, while the payload adapter remains a separate move-only argument.
pub(crate) struct FoundationBootstrapView<'work, 'cancel, 'signal> {
    pub clock: &'work FoundationBootstrapClock,
    pub launch: &'work FoundationLaunchArguments,
    pub invocation: &'work mut FoundationInvocation<'cancel>,
    pub execution_limits: &'work mut FoundationExecutionLimits,
    pub remaining_budget: &'work mut FoundationRemainingBudget<'cancel>,
    pub selected_roots: &'work FoundationSelectedRoots,
    pub stage: &'work PrivateTmpfsStageIsolation,
    pub isolated: &'work IsolatedCreationRoot,
    pub sources: &'work mut RouteSources,
    pub artifact_sources: Option<&'work mut RouteSources>,
    pub captured: &'work FoundationCapturedCut,
    pub selection: &'work FoundationPhysicalSelection,
    pub physical: &'work mut FoundationPhysicalSnapshot<'cancel, 'signal>,
    pub cancelled: &'cancel AtomicBool,
    pub cost: &'work mut FoundationSourceBootstrapCost,
    // The provider is owned by the caller. Its trait-object type is static;
    // only this view's temporary mutable loan is shortened to `'work`.
    pub history:
        Option<&'work mut (dyn super::foundation_reader::FoundationHistoricalEvidence + 'static)>,
}

/// Borrowed unpublished current source under the one original invocation.
/// Candidate read/write counters remain owned by the original spool ledger.
/// Each observed shared read prefix is adopted once before another independent
/// window; protected controls and unforwarded auxiliary reads are charged here.
pub(crate) struct CandidateFoundationBootstrapInputs<'input, 'cancel> {
    pub clock: FoundationBootstrapClock,
    pub launch: FoundationLaunchArguments,
    pub invocation: FoundationInvocation<'cancel>,
    pub execution_limits: FoundationExecutionLimits,
    pub remaining_budget: FoundationRemainingBudget<'cancel>,
    pub selected_roots: FoundationSelectedRoots,
    pub stage: PrivateTmpfsStageIsolation,
    pub isolated: IsolatedCreationRoot,
    pub sources: RouteSources,
    pub payload_sources: RouteSources,
    pub artifact_sources: Option<RouteSources>,
    pub input: &'input dyn tos_validation::record_biblio_cut::SourceCutInputWithIdentity<
        crate::source_admission_spooled_candidate::CandidateFence,
    >,
    pub coverage: tos_validation::record_biblio_cut::SourceCutInputCoverage,
    pub original_epoch: tos_source_store::MetadataPublicationEpoch,
    original_io: tos_source_store::PinnedSqliteIoBudget,
    pub candidate_io_adopted: (u64, u64),
    remaining_write_bytes: u64,
    pub selection: FoundationPhysicalSelection,
    pub cost: FoundationSourceBootstrapCost,
    cancelled: &'cancel AtomicBool,
    config: FoundationBootstrapConfig,
}
pub(crate) struct CandidateFoundationBootstrapView<'work, 'input, 'cancel, 'signal> {
    pub clock: &'work FoundationBootstrapClock,
    pub launch: &'work FoundationLaunchArguments,
    pub invocation: &'work mut FoundationInvocation<'cancel>,
    pub execution_limits: &'work mut FoundationExecutionLimits,
    pub remaining_budget: &'work mut FoundationRemainingBudget<'cancel>,
    pub selected_roots: &'work FoundationSelectedRoots,
    pub stage: &'work PrivateTmpfsStageIsolation,
    pub isolated: &'work IsolatedCreationRoot,
    pub sources: &'work mut RouteSources,
    pub artifact_sources: Option<&'work mut RouteSources>,
    pub input: &'input dyn tos_validation::record_biblio_cut::SourceCutInputWithIdentity<
        crate::source_admission_spooled_candidate::CandidateFence,
    >,
    pub coverage: &'work tos_validation::record_biblio_cut::SourceCutInputCoverage,
    pub original_epoch: &'work tos_source_store::MetadataPublicationEpoch,
    pub original_io: &'work tos_source_store::PinnedSqliteIoBudget,
    pub candidate_io_adopted: &'work mut (u64, u64),
    pub remaining_write_bytes: &'work mut u64,
    pub selection: &'work FoundationPhysicalSelection,
    pub physical: &'work mut FoundationPhysicalSnapshot<'cancel, 'signal>,
    pub cancelled: &'cancel AtomicBool,
    pub cost: &'work mut FoundationSourceBootstrapCost,
    // The provider is owned by the caller. Its trait-object type is static;
    // only this view's temporary mutable loan is shortened to `'work`.
    pub history:
        Option<&'work mut (dyn super::foundation_reader::FoundationHistoricalEvidence + 'static)>,
}
impl<'cancel> FoundationBootstrapInputs<'cancel> {
    /// Continue from the one real invocation read performed by the entry
    /// adapter. No arguments, roots, budgets, cancellation flag, or clock are
    /// rediscovered here.
    pub(crate) fn prepare(
        clock: FoundationBootstrapClock,
        launch: FoundationLaunchArguments,
        invocation: FoundationInvocation<'cancel>,
        config: FoundationBootstrapConfig,
    ) -> Result<Self, FoundationBootstrapError> {
        Self::prepare_inner(clock, launch, invocation, config, None, None, None, None)
    }

    /// Continue an unpublished candidate under the original already charged
    /// invocation. The held primary root is reused for grammar/software and
    /// auxiliary custody; authored bytes come only from the candidate.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare_candidate(
        clock: FoundationBootstrapClock,
        launch: FoundationLaunchArguments,
        invocation: FoundationInvocation<'cancel>,
        remaining_budget: FoundationRemainingBudget<'cancel>,
        candidate: &Candidate<'_>,
        _git_signal: &AtomicI32,
        primary_sources: RouteSources,
        prior_candidate_io: (u64, u64),
    ) -> Result<Self, FoundationBootstrapError> {
        let config = super::foundation_bootstrap_config::from_candidate_invocation(&invocation)
            .map_err(FoundationBootstrapError::Command)?;
        Self::prepare_inner(
            clock,
            launch,
            invocation,
            config,
            Some(remaining_budget),
            Some(candidate),
            Some(primary_sources),
            Some(prior_candidate_io),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_inner(
        clock: FoundationBootstrapClock,
        launch: FoundationLaunchArguments,
        invocation: FoundationInvocation<'cancel>,
        mut config: FoundationBootstrapConfig,
        supplied_budget: Option<FoundationRemainingBudget<'cancel>>,
        candidate: Option<&Candidate<'_>>,
        primary_sources: Option<RouteSources>,
        prior_candidate_io: Option<(u64, u64)>,
    ) -> Result<Self, FoundationBootstrapError> {
        let cancelled = invocation.cancellation_flag();
        let deadline = invocation.deadline();
        checkpoint(deadline, cancelled).map_err(FoundationBootstrapError::Command)?;
        if launch.arguments.help {
            return Err(FoundationBootstrapError::Configuration(
                "help invocation cannot enter source bootstrap",
            ));
        }
        if invocation.started() != clock.started() || deadline > clock.hard_deadline() {
            return Err(FoundationBootstrapError::Configuration(
                "invocation does not retain the original bootstrap clock",
            ));
        }

        preflight(&invocation, &config)?;
        let provided_budget = supplied_budget.is_some();
        let mut remaining_budget = match supplied_budget {
            Some(budget) => budget,
            None => FoundationRemainingBudget::from_invocation(&invocation)
                .map_err(FoundationBootstrapError::Command)?,
        };
        remaining_budget
            .verify_invocation(&invocation)
            .map_err(FoundationBootstrapError::Command)?;
        let entry_source_read_bytes = invocation
            .cost
            .invocation_read_bytes
            .checked_add(invocation.cost.self_image_read_bytes)
            .ok_or(FoundationBootstrapError::Configuration(
                "entry source-read accounting overflow",
            ))?;
        if !provided_budget {
            charge_entry_source_reads(&mut remaining_budget, entry_source_read_bytes)?;
        }
        let mut execution_limits = FoundationExecutionLimits::new_whole(&invocation)
            .map_err(FoundationBootstrapError::Command)?;
        let whole_budget_after_invocation = budget_cost(&remaining_budget);
        let capture_control_read_upper_bound_bytes = usize::try_from(
            super::foundation_bootstrap_config::capture_control_read_upper_bound_bytes()
                .map_err(FoundationBootstrapError::Command)?,
        )
        .ok()
        .ok_or(FoundationBootstrapError::Configuration(
            "capture control reservation overflow",
        ))?;
        let mut selected_roots = invocation
            .selected_roots(&launch)
            .map_err(FoundationBootstrapError::Command)?;

        // The default mirrors the maintained Python adapter's exact payload
        // subtree. It remains a separate held root and shares the primary
        // route's finite operation allowance.
        let payload_root_was_defaulted = selected_roots.payload_source_root.is_none();
        let payload_root = selected_roots
            .payload_source_root
            .clone()
            .unwrap_or_else(|| selected_roots.repo_root.join(PAYLOAD_SOURCE_SUFFIX));
        selected_roots.payload_source_root = Some(payload_root.clone());

        let selected_roots_state_upper_bound_bytes =
            selected_roots_state_upper_bound(&selected_roots, &payload_root)?;
        charge_admitted_state(
            &mut remaining_budget,
            "selected-root-custody",
            selected_roots_state_upper_bound_bytes,
        )?;
        config = super::foundation_bootstrap_config::narrow_capture_config(
            config,
            remaining_budget
                .remaining()
                .map_err(FoundationBootstrapError::Command)?,
        )
        .map_err(FoundationBootstrapError::Command)?;
        preflight_capture_headroom(&mut remaining_budget, &config)?;
        let whole_budget_after_selected_roots = budget_cost(&remaining_budget);

        checkpoint(deadline, cancelled).map_err(FoundationBootstrapError::Command)?;
        let mut sources = match primary_sources {
            Some(sources) => {
                if sources.selected_root_path() != selected_roots.repo_root
                    || sources.deadline() > deadline
                {
                    return Err(FoundationBootstrapError::Configuration(
                        "candidate primary root selection differs",
                    ));
                }
                sources
                    .verify_root()
                    .map_err(FoundationBootstrapError::RouteRoot)?;
                // The root identity/path binding is retained by the selected
                // admission identity owner; no new route allowance is created.
                sources
            }
            // The complete authored source route performs three bounded
            // membership walks and repeated descriptor-fenced member reads.
            // Keep the generic 100k route default for other callers; this
            // owner uses the explicit finite ceiling and reports actual usage.
            None => RouteSources::new_until_with_operation_limit(
                &selected_roots.repo_root,
                deadline,
                MAX_BUDGETED_ROUTE_OPERATIONS,
            )
            .map_err(FoundationBootstrapError::RouteRoot)?,
        };
        let mut payload_sources =
            RouteSources::new_until_related(&payload_root, deadline, &sources)
                .map_err(FoundationBootstrapError::RouteRoot)?;
        let mut artifact_sources = selected_roots
            .artifact_root
            .as_deref()
            .map(|root| RouteSources::new_until_related(root, deadline, &sources))
            .transpose()
            .map_err(FoundationBootstrapError::RouteRoot)?;
        let shared_route_operations_after_roots = sources.operation_count();
        let source_root_component_opens_after_roots = sources.root_component_open_count();
        let payload_root_component_opens_after_roots = payload_sources.root_component_open_count();
        let artifact_root_component_opens_after_roots = artifact_sources
            .as_ref()
            .map(RouteSources::root_component_open_count);

        let capture = run_budget_window(
            &mut remaining_budget,
            &mut execution_limits,
            "capture",
            FoundationWindowKind::Capture,
            FoundationPhaseReservation::default(),
            |_execution_limits, ticket| {
                let operation_limits = ticket.operation_limits();
                let (read_limits, cut_limits, member_read_upper_bound_bytes) =
                    effective_capture_limits(&invocation, &config, ticket)?;
                let stage = invocation
                    .select_stage()
                    .map_err(FoundationBootstrapError::StageSelection)?;
                let isolated = IsolatedCreationRoot::create(stage.root(), deadline, cancelled)
                    .map_err(FoundationBootstrapError::IsolatedRoot)?;
                let max_capture_write_bytes =
                    usize::try_from(config.capture_tmpfs_write_upper_bound_bytes).map_err(
                        |_| FoundationBootstrapError::Configuration("capture write limit range"),
                    )?;
                let mut candidate_io_after_capture = None;
                let mut candidate_copy_charged_read = 0u64;
                let mut candidate_retained_state_delta = 0usize;
                let mut candidate_state_after_capture = None;
                let captured = match candidate {
                    Some(candidate) => {
                        let before = candidate.io_usage();
                        let before_state = candidate.retained_state_bytes();
                        if prior_candidate_io != Some(before) {
                            return Err(FoundationBootstrapError::Configuration(
                                "candidate prior accounting binding",
                            ));
                        }
                        let (epoch, _) = foundation_capture::select_epoch_with_cost(&mut sources)
                            .map_err(FoundationBootstrapError::Capture)?;
                        let captured = foundation_capture::capture_candidate(
                            candidate,
                            &isolated,
                            epoch,
                            invocation.executable_sha256(),
                            read_limits,
                            cut_limits,
                            member_read_upper_bound_bytes,
                            max_capture_write_bytes,
                            operation_limits
                                .state_bytes
                                .checked_sub(
                                    tos_compiler::private_tmpfs_stage::PRIVATE_TMPFS_SELECT_COST
                                        .retained_bytes,
                                )
                                .ok_or(FoundationBootstrapError::Configuration(
                                    "candidate capture guard state reservation",
                                ))?,
                            deadline,
                            cancelled,
                        )
                        .map_err(FoundationBootstrapError::Capture)?;
                        foundation_capture::verify_epoch_with_cost(&mut sources, captured.epoch())
                            .map_err(FoundationBootstrapError::Capture)?;
                        let after = candidate.io_usage();
                        candidate_copy_charged_read = after.0.checked_sub(before.0).ok_or(
                            FoundationBootstrapError::Configuration(
                                "candidate read accounting regressed",
                            ),
                        )?;
                        if after.1 != before.1 {
                            return Err(FoundationBootstrapError::Configuration(
                                "candidate capture unexpectedly wrote proposal",
                            ));
                        }
                        candidate_io_after_capture = Some(after);
                        let after_state = candidate.retained_state_bytes();
                        candidate_retained_state_delta = after_state
                            .checked_sub(before_state)
                            .ok_or(FoundationBootstrapError::Configuration(
                                "candidate capture state accounting regressed",
                            ))?;
                        candidate_state_after_capture = Some(after_state);
                        captured
                    }
                    None => foundation_capture::capture_bounded_with_write_cap(
                        &mut sources,
                        &isolated,
                        invocation.executable_sha256(),
                        read_limits,
                        cut_limits,
                        member_read_upper_bound_bytes,
                        max_capture_write_bytes,
                        deadline,
                        cancelled,
                    )
                    .map_err(FoundationBootstrapError::Capture)?,
                };
                let capture_cost = captured.cost();
                let actual_member_read_bytes = capture_cost
                    .source_read_bytes
                    .checked_add(capture_cost.immutable_member_eof_bytes)
                    .and_then(|bytes| bytes.checked_add(capture_cost.source_recheck_bytes))
                    .ok_or(FoundationBootstrapError::Configuration(
                        "capture member read accounting overflow",
                    ))?;
                if actual_member_read_bytes > member_read_upper_bound_bytes {
                    return Err(FoundationBootstrapError::Configuration(
                        "capture exceeded its three-pass member reservation",
                    ));
                }
                let actual_capture_writes = capture_cost
                    .object_write_upper_bound_bytes
                    .checked_add(capture_cost.manifest_write_bytes)
                    .ok_or(FoundationBootstrapError::Configuration(
                        "capture write accounting overflow",
                    ))?;
                if u64::try_from(actual_capture_writes).map_or(true, |bytes| {
                    bytes > config.capture_tmpfs_write_upper_bound_bytes
                        || bytes as u64 > operation_limits.tmpfs_bytes
                }) {
                    return Err(FoundationBootstrapError::Configuration(
                        "capture exceeded its private-stage write reservation",
                    ));
                }
                let observed_state =
                    super::foundation_bootstrap_config::observed_capture_state_upper_bound(
                        &captured,
                    )
                    .map_err(FoundationBootstrapError::Command)?;
                let state_upper_bound_bytes = observed_state
                    .retained_upper_bound_bytes
                    .checked_add(candidate_retained_state_delta)
                    .ok_or(FoundationBootstrapError::Configuration(
                        "candidate capture retained state overflow",
                    ))?;
                let capture_peak_state = observed_state
                    .peak_upper_bound_bytes
                    .checked_add(candidate_retained_state_delta)
                    .ok_or(FoundationBootstrapError::Configuration(
                        "candidate capture peak state overflow",
                    ))?;
                if observed_state.peak_upper_bound_bytes > config.capture_state_upper_bound_bytes
                    || capture_peak_state > operation_limits.state_bytes
                {
                    return Err(FoundationBootstrapError::Configuration(
                        "capture observed state exceeds reservation",
                    ));
                }
                checkpoint(deadline, cancelled).map_err(FoundationBootstrapError::Command)?;
                let usage = stage.quota_usage().map_err(|_| {
                    FoundationBootstrapError::Configuration("capture private quota observation")
                })?;
                checkpoint(deadline, cancelled).map_err(FoundationBootstrapError::Command)?;
                if usage.used_bytes > operation_limits.tmpfs_bytes {
                    return Err(FoundationBootstrapError::Configuration(
                        "capture allocated blocks exceed remaining tmpfs",
                    ));
                }
                let tmpfs_inode_upper_bound = usize::try_from(usage.used_inodes).map_err(|_| {
                    FoundationBootstrapError::Configuration("capture inode cost range")
                })?;
                if u64::try_from(tmpfs_inode_upper_bound)
                    .map_or(true, |used| used > operation_limits.tmpfs_inodes)
                {
                    return Err(FoundationBootstrapError::Configuration(
                        "capture exceeded its private-stage inode reservation",
                    ));
                }
                let source_read_upper_bound_bytes = actual_member_read_bytes
                    .checked_add(usize::try_from(candidate_copy_charged_read).map_err(|_| {
                        FoundationBootstrapError::Configuration("candidate capture read cost range")
                    })?)
                    .and_then(|bytes| bytes.checked_add(capture_control_read_upper_bound_bytes))
                    .ok_or(FoundationBootstrapError::Configuration(
                        "capture aggregate reservation overflow",
                    ))?;
                let use_amount = FoundationPhaseUse {
                    source_read_bytes: FoundationCharge::admitted_upper_bound(
                        u64::try_from(source_read_upper_bound_bytes).map_err(|_| {
                            FoundationBootstrapError::Configuration("capture read cost range")
                        })?,
                    ),
                    state_bytes: FoundationCharge::admitted_upper_bound(state_upper_bound_bytes),
                    // Kernel-accounted retained blocks, including the private
                    // capture tree. This is not physical RAM or transient peak.
                    tmpfs_bytes: FoundationCharge::measured(usage.used_bytes),
                    tmpfs_inodes: FoundationCharge::admitted_upper_bound(
                        u64::try_from(tmpfs_inode_upper_bound).map_err(|_| {
                            FoundationBootstrapError::Configuration("capture inode cost range")
                        })?,
                    ),
                    ..FoundationPhaseUse::default()
                };
                Ok((
                    FoundationCaptureWindowResult {
                        stage,
                        isolated,
                        captured,
                        cost: capture_cost,
                        member_read_upper_bound_bytes,
                        control_read_upper_bound_bytes: capture_control_read_upper_bound_bytes,
                        source_read_upper_bound_bytes,
                        state_upper_bound_bytes,
                        tmpfs_used_bytes: usage.used_bytes,
                        tmpfs_inode_upper_bound,
                        candidate_io_after_capture,
                        candidate_state_after_capture,
                    },
                    use_amount,
                ))
            },
        )?;
        let FoundationCaptureWindowResult {
            stage,
            isolated,
            captured,
            cost: capture_after_initial,
            member_read_upper_bound_bytes: capture_member_read_upper_bound_bytes,
            control_read_upper_bound_bytes: capture_control_read_upper_bound_bytes,
            source_read_upper_bound_bytes: capture_source_read_upper_bound_bytes,
            state_upper_bound_bytes: capture_state_upper_bound_bytes,
            tmpfs_used_bytes: capture_tmpfs_write_upper_bound_bytes,
            tmpfs_inode_upper_bound: capture_tmpfs_inode_upper_bound,
            candidate_io_after_capture,
            candidate_state_after_capture,
        } = capture;
        let budget_after_capture = budget_cost(&remaining_budget);
        let shared_route_operations_after_capture = sources.operation_count();
        let source_root_component_opens_after_capture = sources.root_component_open_count();

        let selection = run_budget_window(
            &mut remaining_budget,
            &mut execution_limits,
            "selection",
            FoundationWindowKind::Selection,
            FoundationPhaseReservation::default(),
            |execution_limits, _ticket| {
                let selected_limits = execution_limits
                    .selection_limits()
                    .map_err(FoundationBootstrapError::Command)?;
                let limits =
                    intersect_selection_limits(selected_limits, config.selection_limits, deadline)?;
                let selection = foundation_selection::select(&captured, limits, cancelled)
                    .map_err(FoundationBootstrapError::Selection)?;
                if selection.cost.source_bytes_read > limits.max_total_read_bytes
                    || selection.cost.peak_state_bytes > limits.max_state_bytes
                {
                    return Err(FoundationBootstrapError::Configuration(
                        "selection exceeded its whole-operation window",
                    ));
                }
                let use_amount = FoundationPhaseUse {
                    source_read_bytes: FoundationCharge::measured(selection.cost.source_bytes_read),
                    state_bytes: FoundationCharge::admitted_upper_bound(
                        selection.cost.retained_state_bytes,
                    ),
                    ..FoundationPhaseUse::default()
                };
                Ok((selection, use_amount))
            },
        )?;
        let budget_after_selection = budget_cost(&remaining_budget);
        let mut selection = selection;
        let shared_route_operations_after_selection = sources.operation_count();
        let source_root_component_opens_after_selection = sources.root_component_open_count();

        let cost = FoundationSourceBootstrapCost {
            invocation_after_initial_read: invocation.cost,
            invocation_after_callback: None,
            entry_source_read_bytes,
            selected_roots_state_upper_bound_bytes,
            payload_root_defaulted: payload_root_was_defaulted,
            capture_member_read_upper_bound_bytes,
            capture_control_read_upper_bound_bytes,
            capture_source_read_upper_bound_bytes,
            capture_state_upper_bound_bytes,
            capture_tmpfs_write_upper_bound_bytes,
            capture_tmpfs_inode_upper_bound,
            candidate_io_after_capture,
            candidate_state_after_capture,
            capture_after_initial,
            capture_after_callback: None,
            selection: selection.cost,
            selection_after_auxiliary_read: None,
            payload_after_snapshot: None,
            physical_after_snapshot: None,
            physical_after_callback: None,
            shared_route_operations_after_roots,
            shared_route_operations_after_capture,
            shared_route_operations_after_selection,
            shared_route_operations_after_auxiliary_selection: None,
            shared_route_operations_after_snapshot: 0,
            shared_route_operations_after_callback: None,
            source_root_component_opens_after_roots,
            payload_root_component_opens_after_roots,
            artifact_root_component_opens_after_roots,
            source_root_component_opens_after_capture,
            source_root_component_opens_after_selection,
            source_root_component_opens_after_auxiliary_selection: None,
            source_root_component_opens_after_snapshot: None,
            payload_root_component_opens_after_snapshot: None,
            artifact_root_component_opens_after_snapshot: None,
            source_root_component_opens_after_callback: None,
            payload_root_component_opens_after_callback: None,
            artifact_root_component_opens_after_callback: None,
            whole_budget_after_invocation,
            whole_budget_after_selected_roots,
            whole_budget_after_capture: Some(budget_after_capture),
            whole_budget_after_selection: Some(budget_after_selection),
            whole_budget_after_auxiliary_selection: None,
            whole_budget_after_payload_snapshot: None,
            whole_budget_after_physical_snapshot: None,
            whole_budget_after_callback: None,
        };

        let result = Self {
            clock,
            launch,
            invocation,
            execution_limits,
            remaining_budget,
            selected_roots,
            stage,
            isolated,
            sources,
            payload_sources,
            artifact_sources,
            captured,
            selection,
            cost,
            cancelled,
            config,
        };
        Ok(result)
    }

    /// Construct the one pre-worker payload snapshot and physical snapshot,
    /// then lend them to the caller while the capture and all selected roots
    /// remain alive. `run` is higher-ranked so it cannot return a borrow of a
    /// temporary adapter or physical snapshot.
    pub(crate) fn with_initial_snapshots<'signal, T, E, F>(
        &mut self,
        git_signal: &'signal AtomicI32,
        run: F,
    ) -> Result<T, E>
    where
        F: for<'work> FnOnce(
            FoundationBootstrapView<'work, 'cancel, 'signal>,
            FoundationPayloadSources<'work>,
        ) -> Result<T, E>,
        E: From<FoundationBootstrapError>,
    {
        self.with_initial_snapshots_with_history(git_signal, None, run)
    }

    pub(crate) fn with_initial_snapshots_with_history<'signal, T, E, F>(
        &mut self,
        git_signal: &'signal AtomicI32,
        mut history: Option<
            &mut (dyn super::foundation_reader::FoundationHistoricalEvidence + 'static),
        >,
        run: F,
    ) -> Result<T, E>
    where
        F: for<'work> FnOnce(
            FoundationBootstrapView<'work, 'cancel, 'signal>,
            FoundationPayloadSources<'work>,
        ) -> Result<T, E>,
        E: From<FoundationBootstrapError>,
    {
        let deadline = self.invocation.deadline();
        checkpoint(deadline, self.cancelled)
            .map_err(|error| E::from(FoundationBootstrapError::Command(error)))?;
        let outcome = {
            let clock = &self.clock;
            let launch = &self.launch;
            let invocation = &mut self.invocation;
            let execution_limits = &mut self.execution_limits;
            let remaining_budget = &mut self.remaining_budget;
            let selected_roots = &self.selected_roots;
            let stage = &self.stage;
            let isolated = &self.isolated;
            let sources = &mut self.sources;
            let payload_root = &mut self.payload_sources;
            let artifact_roots = &mut self.artifact_sources;
            let captured = &self.captured;
            let selection = &mut self.selection;
            let cancelled = self.cancelled;
            let config = self.config;
            let cost = &mut self.cost;

            if let Some(history) = history.as_deref() {
                for member in captured.cut().current().members() {
                    checkpoint(deadline, cancelled)
                        .map_err(|error| E::from(FoundationBootstrapError::Command(error)))?;
                    if history.selected(member.path.as_str()) {
                        return Err(E::from(FoundationBootstrapError::Configuration(
                            "historical input overlaps candidate",
                        )));
                    }
                }
            }

            let selection_before_auxiliary = selection.cost;
            let selection_baseline = FoundationPhaseReservation {
                source_read_bytes: selection_before_auxiliary.source_bytes_read,
                state_bytes: selection_before_auxiliary.retained_state_bytes,
                ..FoundationPhaseReservation::default()
            };
            run_budget_window(
                remaining_budget,
                execution_limits,
                "auxiliary-selection",
                FoundationWindowKind::Selection,
                selection_baseline,
                |execution_limits, _ticket| {
                    let selected_limits = execution_limits
                        .selection_limits()
                        .map_err(FoundationBootstrapError::Command)?;
                    let limits = intersect_selection_limits(
                        selected_limits,
                        config.selection_limits,
                        deadline,
                    )?;
                    foundation_selection::read_auxiliary_query_documents(
                        selection, sources, limits, cancelled,
                    )
                    .map_err(FoundationBootstrapError::Selection)?;
                    if selection.cost.peak_state_bytes > limits.max_state_bytes {
                        return Err(FoundationBootstrapError::Configuration(
                            "auxiliary selection exceeded its whole-operation window",
                        ));
                    }
                    let read_delta = selection
                        .cost
                        .source_bytes_read
                        .checked_sub(selection_before_auxiliary.source_bytes_read)
                        .ok_or(FoundationBootstrapError::Configuration(
                            "auxiliary selection read cost regressed",
                        ))?;
                    let state_delta = selection
                        .cost
                        .retained_state_bytes
                        .checked_sub(selection_before_auxiliary.retained_state_bytes)
                        .ok_or(FoundationBootstrapError::Configuration(
                            "auxiliary selection state cost regressed",
                        ))?;
                    Ok((
                        (),
                        FoundationPhaseUse {
                            source_read_bytes: FoundationCharge::measured(read_delta),
                            state_bytes: FoundationCharge::admitted_upper_bound(state_delta),
                            ..FoundationPhaseUse::default()
                        },
                    ))
                },
            )
            .map_err(E::from)?;
            cost.selection_after_auxiliary_read = Some(selection.cost);
            cost.whole_budget_after_auxiliary_selection = Some(budget_cost(remaining_budget));
            cost.shared_route_operations_after_auxiliary_selection =
                Some(sources.operation_count());
            cost.source_root_component_opens_after_auxiliary_selection =
                Some(sources.root_component_open_count());

            let (mut payloads, payload_facts) = run_budget_window(
                remaining_budget,
                execution_limits,
                "payload-snapshot",
                FoundationWindowKind::Payload,
                FoundationPhaseReservation::default(),
                |execution_limits, _ticket| {
                    let derived = execution_limits
                        .payload_limits_from_ceilings(config.payload_limits)
                        .map_err(FoundationBootstrapError::Command)?;
                    let limits = intersect_payload_limits(derived, config.payload_limits)?;
                    let mut payloads = FoundationPayloadSources::new(
                        payload_root,
                        captured.cut(),
                        limits,
                        deadline,
                        cancelled,
                    )
                    .map_err(FoundationBootstrapError::Payload)?;
                    let payload_facts = payloads
                        .snapshot_facts(&selection.payload_paths)
                        .map_err(FoundationBootstrapError::Payload)?;
                    let payload_cost = payloads.cost();
                    let retained_state = payload_cost
                        .retained_state_bytes
                        .checked_add(payload_cost.snapshot_facts_state_bytes)
                        .ok_or(FoundationBootstrapError::Configuration(
                            "payload snapshot state accounting overflow",
                        ))?;
                    if payload_cost.initial_bytes_read > limits.max_total_bytes
                        || payload_cost.peak_state_bytes > limits.max_state_bytes
                        || retained_state > limits.max_state_bytes
                    {
                        return Err(FoundationBootstrapError::Configuration(
                            "payload snapshot exceeded its whole-operation window",
                        ));
                    }
                    let use_amount = FoundationPhaseUse {
                        source_read_bytes: FoundationCharge::measured(
                            payload_cost.initial_bytes_read,
                        ),
                        // The facts map moves into the physical snapshot and
                        // is charged here once; the physical window reuses it
                        // as an already-admitted state baseline.
                        state_bytes: FoundationCharge::admitted_upper_bound(retained_state),
                        ..FoundationPhaseUse::default()
                    };
                    Ok(((payloads, payload_facts), use_amount))
                },
            )
            .map_err(E::from)?;
            let payload_snapshot_cost = payloads.cost();
            cost.payload_after_snapshot = Some(payload_snapshot_cost);
            cost.whole_budget_after_payload_snapshot = Some(budget_cost(remaining_budget));

            let physical_baseline = FoundationPhaseReservation {
                state_bytes: payload_snapshot_cost.snapshot_facts_state_bytes,
                ..FoundationPhaseReservation::default()
            };
            let mut physical = run_budget_window(
                remaining_budget,
                execution_limits,
                "physical-snapshot",
                FoundationWindowKind::Physical,
                physical_baseline,
                |execution_limits, _ticket| {
                    let derived = execution_limits
                        .physical_limits_from_ceilings(config.physical_limits)
                        .map_err(FoundationBootstrapError::Command)?;
                    let limits = intersect_physical_limits(derived, config.physical_limits)?;
                    let mut physical = FoundationPhysicalSnapshot::observe_with_resolved_targets(
                        sources,
                        captured,
                        &selection.authored_paths,
                        payload_facts,
                        &selection.private_paths,
                        &selection.private_prefixes,
                        artifact_roots.as_mut(),
                        &selection.artifact_paths,
                        &selection.resolved_source_directories,
                        limits,
                        deadline,
                        cancelled,
                        git_signal,
                    )
                    .map_err(FoundationBootstrapError::Physical)?;
                    if let Some(history) = history.as_deref_mut() {
                        physical
                            .select_historical(history)
                            .map_err(FoundationBootstrapError::Physical)?;
                    }
                    let physical_cost = physical.cost();
                    let incremental_state = physical_cost
                        .retained_state_bytes
                        .checked_sub(physical_baseline.state_bytes)
                        .ok_or(FoundationBootstrapError::Configuration(
                            "physical snapshot state cost regressed",
                        ))?;
                    if physical_cost.bytes_read > limits.max_total_bytes
                        || physical_cost.retained_state_bytes > limits.max_state_bytes
                    {
                        return Err(FoundationBootstrapError::Configuration(
                            "physical snapshot exceeded its whole-operation window",
                        ));
                    }
                    let use_amount = FoundationPhaseUse {
                        source_read_bytes: FoundationCharge::measured(
                            u64::try_from(physical_cost.bytes_read).map_err(|_| {
                                FoundationBootstrapError::Configuration(
                                    "physical snapshot read cost range",
                                )
                            })?,
                        ),
                        state_bytes: FoundationCharge::admitted_upper_bound(incremental_state),
                        ..FoundationPhaseUse::default()
                    };
                    Ok((physical, use_amount))
                },
            )
            .map_err(E::from)?;
            let initial_physical_cost = physical.cost();
            cost.physical_after_snapshot = Some(initial_physical_cost);
            cost.whole_budget_after_physical_snapshot = Some(budget_cost(remaining_budget));
            cost.shared_route_operations_after_snapshot = sources.operation_count();
            cost.source_root_component_opens_after_snapshot =
                Some(sources.root_component_open_count());
            cost.payload_root_component_opens_after_snapshot =
                Some(payloads.root_component_open_count());
            cost.artifact_root_component_opens_after_snapshot = artifact_roots
                .as_ref()
                .map(|root| root.root_component_open_count());

            let view = FoundationBootstrapView {
                clock,
                launch,
                invocation,
                execution_limits,
                remaining_budget,
                selected_roots,
                stage,
                isolated,
                sources,
                artifact_sources: artifact_roots.as_mut(),
                captured,
                selection,
                physical: &mut physical,
                cancelled,
                cost,
                history,
            };
            let outcome = run(view, payloads);
            let final_physical_cost = physical.cost();
            (outcome, final_physical_cost)
        };

        self.cost.physical_after_callback = Some(outcome.1);
        self.refresh_live_costs();
        outcome.0
    }

    /// Sample held-root counters after the callback's exact EOF/finalization
    /// sequence. Calling this does no new filesystem operation.
    pub(crate) fn refresh_live_costs(&mut self) {
        self.cost.invocation_after_callback = Some(self.invocation.cost);
        self.cost.capture_after_callback = Some(self.captured.cost());
        self.cost.shared_route_operations_after_callback = Some(self.sources.operation_count());
        self.cost.source_root_component_opens_after_callback =
            Some(self.sources.root_component_open_count());
        self.cost.payload_root_component_opens_after_callback =
            Some(self.payload_sources.root_component_open_count());
        self.cost.artifact_root_component_opens_after_callback = self
            .artifact_sources
            .as_ref()
            .map(RouteSources::root_component_open_count);
        self.cost.whole_budget_after_callback = Some(budget_cost(&self.remaining_budget));
    }
}

/// Preparation refusal retains the one moved invocation and its charged
/// ledger/root custody so Native can bill the actual unadopted suffix before
/// reporting terminal failure. No original control or ledger is cloned.
pub(crate) struct CandidateFoundationBootstrapFailure<'cancel> {
    pub error: FoundationBootstrapError,
    pub clock: FoundationBootstrapClock,
    pub launch: FoundationLaunchArguments,
    pub invocation: FoundationInvocation<'cancel>,
    pub remaining_budget: FoundationRemainingBudget<'cancel>,
    pub sources: RouteSources,
}
impl std::fmt::Debug for CandidateFoundationBootstrapFailure<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CandidateFoundationBootstrapFailure")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

impl<'input, 'cancel> CandidateFoundationBootstrapInputs<'input, 'cancel> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare(
        clock: FoundationBootstrapClock,
        launch: FoundationLaunchArguments,
        invocation: FoundationInvocation<'cancel>,
        mut remaining_budget: FoundationRemainingBudget<'cancel>,
        input: &'input crate::source_admission_candidate_records::CandidateRecordsInput<'_, '_>,
        coverage: tos_validation::record_biblio_cut::SourceCutInputCoverage,
        original_io: &tos_source_store::PinnedSqliteIoBudget,
        mut remaining_write_bytes: u64,
        candidate_io_adopted: &mut (u64, u64),
        mut sources: RouteSources,
    ) -> Result<Self, CandidateFoundationBootstrapFailure<'cancel>> {
        let prepared: Result<_, FoundationBootstrapError> = (|| {
            if !input.shares_io_budget(original_io) {
                return Err(FoundationBootstrapError::Configuration(
                    "candidate bootstrap shared IO identity differs",
                ));
            }
            if remaining_write_bytes == u64::MAX {
                return Err(FoundationBootstrapError::Configuration(
                    "candidate bootstrap write headroom is not finite",
                ));
            }
            let entry_io = original_io.snapshot();
            validate_candidate_shared_io_snapshot(&entry_io)?;
            if *candidate_io_adopted
                != (
                    entry_io.read_attempted_bytes,
                    entry_io.write_attempted_bytes,
                )
            {
                return Err(FoundationBootstrapError::Configuration(
                    "candidate bootstrap Native IO adoption differs",
                ));
            }
            let input: &'input dyn tos_validation::record_biblio_cut::SourceCutInputWithIdentity<
                crate::source_admission_spooled_candidate::CandidateFence,
            > = input;
            let cancelled = invocation.cancellation_flag();
            let deadline = invocation.deadline();
            checkpoint(deadline, cancelled).map_err(FoundationBootstrapError::Command)?;
            if launch.arguments.help
                || invocation.started() != clock.started()
                || deadline > clock.hard_deadline()
            {
                return Err(FoundationBootstrapError::Configuration(
                    "candidate bootstrap original invocation",
                ));
            }
            remaining_budget
                .verify_invocation(&invocation)
                .map_err(FoundationBootstrapError::Command)?;
            input
                .verify_current_fence(&coverage, deadline, cancelled)
                .map_err(FoundationBootstrapError::Selection)?;
            adopt_candidate_shared_io_with_budget(
                original_io,
                candidate_io_adopted,
                &mut remaining_write_bytes,
                &mut remaining_budget,
            )?;
            charge_admitted_state(
                &mut remaining_budget,
                "candidate-shared-io-custody",
                size_of::<tos_source_store::PinnedSqliteIoBudget>()
                    + size_of::<(u64, u64)>()
                    + size_of::<u64>(),
            )?;
            let config = super::foundation_bootstrap_config::from_candidate_invocation(&invocation)
                .map_err(FoundationBootstrapError::Command)?;
            let mut execution_limits = FoundationExecutionLimits::new_whole(&invocation)
                .map_err(FoundationBootstrapError::Command)?;
            let mut selected_roots = invocation
                .selected_roots(&launch)
                .map_err(FoundationBootstrapError::Command)?;
            if sources.selected_root_path() != selected_roots.repo_root
                || sources.deadline() > deadline
            {
                return Err(FoundationBootstrapError::Configuration(
                    "candidate primary root selection differs",
                ));
            }
            sources
                .verify_root()
                .map_err(FoundationBootstrapError::RouteRoot)?;
            let payload_root_defaulted = selected_roots.payload_source_root.is_none();
            let payload_root = selected_roots
                .payload_source_root
                .clone()
                .unwrap_or_else(|| selected_roots.repo_root.join(PAYLOAD_SOURCE_SUFFIX));
            selected_roots.payload_source_root = Some(payload_root.clone());
            let selected_roots_state_upper_bound_bytes =
                selected_roots_state_upper_bound(&selected_roots, &payload_root)?;
            let whole_budget_after_invocation = budget_cost(&remaining_budget);
            charge_admitted_state(
                &mut remaining_budget,
                "candidate-selected-roots",
                selected_roots_state_upper_bound_bytes,
            )?;
            let payload_sources =
                RouteSources::new_until_related(&payload_root, deadline, &sources)
                    .map_err(FoundationBootstrapError::RouteRoot)?;
            let artifact_sources = selected_roots
                .artifact_root
                .as_deref()
                .map(|root| RouteSources::new_until_related(root, deadline, &sources))
                .transpose()
                .map_err(FoundationBootstrapError::RouteRoot)?;
            let whole_budget_after_selected_roots = budget_cost(&remaining_budget);
            let (stage, isolated, original_epoch) = run_budget_window(
                &mut remaining_budget,
                &mut execution_limits,
                "candidate-stage-and-original-epoch",
                FoundationWindowKind::Capture,
                FoundationPhaseReservation::default(),
                |_limits, ticket| {
                    let operation = ticket.operation_limits();
                    let guard = tos_compiler::private_tmpfs_stage::PRIVATE_TMPFS_SELECT_COST;
                    let epoch_state = candidate_original_epoch_state_upper_bound()?;
                    let retained_state = guard
                        .retained_bytes
                        .checked_add(size_of::<IsolatedCreationRoot>())
                        .and_then(|n| n.checked_add(epoch_state))
                        .ok_or(FoundationBootstrapError::Configuration(
                            "candidate stage/epoch state overflow",
                        ))?;
                    if operation.state_bytes < guard.workspace_bytes.max(retained_state)
                        || operation.source_read_bytes
                            < guard
                                .read_bytes
                                .checked_add(PUBLICATION_CONTROL_READ_CAP_BYTES as u64)
                                .ok_or(FoundationBootstrapError::Configuration(
                                    "candidate guard read overflow",
                                ))?
                        || operation.tmpfs_inodes < 1
                    {
                        return Err(FoundationBootstrapError::Configuration(
                            "candidate stage/control reservation",
                        ));
                    }
                    let stage = invocation
                        .select_stage()
                        .map_err(FoundationBootstrapError::StageSelection)?;
                    let isolated = IsolatedCreationRoot::create(stage.root(), deadline, cancelled)
                        .map_err(FoundationBootstrapError::IsolatedRoot)?;
                    let (epoch, read) = foundation_capture::select_epoch_with_cost(&mut sources)
                        .map_err(FoundationBootstrapError::Capture)?;
                    Ok((
                        (stage, isolated, epoch),
                        FoundationPhaseUse {
                            source_read_bytes: FoundationCharge::admitted_upper_bound(
                                guard.read_bytes.checked_add(read as u64).ok_or(
                                    FoundationBootstrapError::Configuration(
                                        "candidate control read overflow",
                                    ),
                                )?,
                            ),
                            state_bytes: FoundationCharge::admitted_upper_bound(retained_state),
                            tmpfs_inodes: FoundationCharge::admitted_upper_bound(1),
                            ..FoundationPhaseUse::default()
                        },
                    ))
                },
            )?;
            adopt_candidate_shared_io_with_budget(
                original_io,
                candidate_io_adopted,
                &mut remaining_write_bytes,
                &mut remaining_budget,
            )?;
            let selection = run_candidate_shared_budget_window(
                &mut remaining_budget,
                &mut execution_limits,
                "candidate-selection",
                FoundationWindowKind::Selection,
                FoundationPhaseReservation::default(),
                original_io,
                candidate_io_adopted,
                &mut remaining_write_bytes,
                |limits, _ticket| {
                    let selected = intersect_selection_limits(
                        limits
                            .selection_limits()
                            .map_err(FoundationBootstrapError::Command)?,
                        config.selection_limits,
                        deadline,
                    )?;
                    let selection = foundation_selection::select_candidate(
                        input.source_input(),
                        selected,
                        cancelled,
                    )
                    .map_err(FoundationBootstrapError::Selection)?;
                    let state = selection.cost.retained_state_bytes;
                    Ok((
                        selection,
                        FoundationPhaseUse {
                            state_bytes: FoundationCharge::admitted_upper_bound(state),
                            ..FoundationPhaseUse::default()
                        },
                    ))
                },
            )?;
            input
                .verify_current_fence(&coverage, deadline, cancelled)
                .map_err(FoundationBootstrapError::Selection)?;
            adopt_candidate_shared_io_with_budget(
                original_io,
                candidate_io_adopted,
                &mut remaining_write_bytes,
                &mut remaining_budget,
            )?;
            let cost = FoundationSourceBootstrapCost {
                invocation_after_initial_read: invocation.cost,
                entry_source_read_bytes: invocation
                    .cost
                    .invocation_read_bytes
                    .checked_add(invocation.cost.self_image_read_bytes)
                    .ok_or(FoundationBootstrapError::Configuration(
                        "candidate entry accounting overflow",
                    ))?,
                selected_roots_state_upper_bound_bytes,
                payload_root_defaulted,
                selection: selection.cost,
                shared_route_operations_after_roots: sources.operation_count(),
                source_root_component_opens_after_roots: sources.root_component_open_count(),
                payload_root_component_opens_after_roots: payload_sources
                    .root_component_open_count(),
                artifact_root_component_opens_after_roots: artifact_sources
                    .as_ref()
                    .map(RouteSources::root_component_open_count),
                whole_budget_after_invocation,
                whole_budget_after_selected_roots,
                whole_budget_after_selection: Some(budget_cost(&remaining_budget)),
                ..FoundationSourceBootstrapCost::default()
            };
            Ok((
                execution_limits,
                selected_roots,
                stage,
                isolated,
                payload_sources,
                artifact_sources,
                input,
                coverage,
                original_epoch,
                selection,
                cost,
                cancelled,
                config,
            ))
        })();
        match prepared {
            Ok((
                execution_limits,
                selected_roots,
                stage,
                isolated,
                payload_sources,
                artifact_sources,
                input,
                coverage,
                original_epoch,
                selection,
                cost,
                cancelled,
                config,
            )) => Ok(Self {
                clock,
                launch,
                invocation,
                execution_limits,
                remaining_budget,
                selected_roots,
                stage,
                isolated,
                sources,
                payload_sources,
                artifact_sources,
                input,
                coverage,
                original_epoch,
                original_io: original_io.clone(),
                candidate_io_adopted: *candidate_io_adopted,
                remaining_write_bytes,
                selection,
                cost,
                cancelled,
                config,
            }),
            Err(error) => Err(CandidateFoundationBootstrapFailure {
                error,
                clock,
                launch,
                invocation,
                remaining_budget,
                sources,
            }),
        }
    }
    pub(crate) fn with_initial_candidate_snapshots<'signal, T, E, F>(
        &mut self,
        git_signal: &'signal AtomicI32,
        actual_input: &crate::source_admission_candidate_records::CandidateRecordsInput<'_, '_>,
        run: F,
    ) -> Result<T, E>
    where
        F: for<'work> FnOnce(
            CandidateFoundationBootstrapView<'work, 'input, 'cancel, 'signal>,
            FoundationPayloadSources<'work>,
        ) -> Result<T, E>,
        E: From<FoundationBootstrapError>,
    {
        self.with_initial_candidate_snapshots_with_history(git_signal, actual_input, None, run)
    }

    pub(crate) fn with_initial_candidate_snapshots_with_history<'signal, T, E, F>(
        &mut self,
        git_signal: &'signal AtomicI32,
        actual_input: &crate::source_admission_candidate_records::CandidateRecordsInput<'_, '_>,
        mut history: Option<
            &mut (dyn super::foundation_reader::FoundationHistoricalEvidence + 'static),
        >,
        run: F,
    ) -> Result<T, E>
    where
        F: for<'work> FnOnce(
            CandidateFoundationBootstrapView<'work, 'input, 'cancel, 'signal>,
            FoundationPayloadSources<'work>,
        ) -> Result<T, E>,
        E: From<FoundationBootstrapError>,
    {
        let actual_source: &dyn tos_validation::record_biblio_cut::SourceCutInputWithIdentity<
            crate::source_admission_spooled_candidate::CandidateFence,
        > = actual_input;
        if !std::ptr::addr_eq(self.input, actual_source)
            || !actual_input.shares_io_budget(&self.original_io)
        {
            return Err(E::from(FoundationBootstrapError::Configuration(
                "candidate snapshot input/shared IO loan differs",
            )));
        }
        let deadline = self.invocation.deadline();
        checkpoint(deadline, self.cancelled)
            .map_err(|error| E::from(FoundationBootstrapError::Command(error)))?;
        let outcome = {
            let clock = &self.clock;
            let launch = &self.launch;
            let invocation = &mut self.invocation;
            let execution_limits = &mut self.execution_limits;
            let remaining_budget = &mut self.remaining_budget;
            let selected_roots = &self.selected_roots;
            let stage = &self.stage;
            let isolated = &self.isolated;
            let sources = &mut self.sources;
            let payload_root = &mut self.payload_sources;
            let artifact_roots = &mut self.artifact_sources;
            let input = self.input;
            let coverage = &self.coverage;
            let original_epoch = &self.original_epoch;
            let original_io = &self.original_io;
            let candidate_io_adopted = &mut self.candidate_io_adopted;
            let remaining_write_bytes = &mut self.remaining_write_bytes;
            let selection = &mut self.selection;
            let cancelled = self.cancelled;
            let config = self.config;
            let cost = &mut self.cost;

            adopt_candidate_shared_io_with_budget(
                original_io,
                candidate_io_adopted,
                remaining_write_bytes,
                remaining_budget,
            )
            .map_err(E::from)?;
            if let Some(history) = history.as_deref() {
                input
                    .for_each_current_member_meta(deadline, cancelled, &mut |member| {
                        if history.selected(member.path) {
                            return Err(ItemRefusal::Source(
                                "historical input overlaps candidate".into(),
                            ));
                        }
                        Ok(())
                    })
                    .map_err(|error| E::from(FoundationBootstrapError::Selection(error)))?;
            }

            adopt_candidate_shared_io_with_budget(
                original_io,
                candidate_io_adopted,
                remaining_write_bytes,
                remaining_budget,
            )
            .map_err(E::from)?;
            let selection_before_auxiliary = selection.cost;
            let selection_baseline = FoundationPhaseReservation {
                source_read_bytes: 0,
                state_bytes: selection_before_auxiliary.retained_state_bytes,
                ..FoundationPhaseReservation::default()
            };
            run_budget_window(
                remaining_budget,
                execution_limits,
                "auxiliary-selection",
                FoundationWindowKind::Selection,
                selection_baseline,
                |execution_limits, _ticket| {
                    let selected_limits = execution_limits
                        .selection_limits()
                        .map_err(FoundationBootstrapError::Command)?;
                    let limits = intersect_selection_limits(
                        selected_limits,
                        config.selection_limits,
                        deadline,
                    )?;
                    foundation_selection::read_auxiliary_query_documents_with_remaining_read(
                        selection,
                        sources,
                        limits.max_total_read_bytes,
                        cancelled,
                    )
                    .map_err(FoundationBootstrapError::Selection)?;
                    if selection.cost.peak_state_bytes > limits.max_state_bytes {
                        return Err(FoundationBootstrapError::Configuration(
                            "auxiliary selection exceeded its whole-operation window",
                        ));
                    }
                    let read_delta = selection
                        .cost
                        .source_bytes_read
                        .checked_sub(selection_before_auxiliary.source_bytes_read)
                        .ok_or(FoundationBootstrapError::Configuration(
                            "auxiliary selection read cost regressed",
                        ))?;
                    let state_delta = selection
                        .cost
                        .retained_state_bytes
                        .checked_sub(selection_before_auxiliary.retained_state_bytes)
                        .ok_or(FoundationBootstrapError::Configuration(
                            "auxiliary selection state cost regressed",
                        ))?;
                    Ok((
                        (),
                        FoundationPhaseUse {
                            source_read_bytes: FoundationCharge::measured(read_delta),
                            state_bytes: FoundationCharge::admitted_upper_bound(state_delta),
                            ..FoundationPhaseUse::default()
                        },
                    ))
                },
            )
            .map_err(E::from)?;
            cost.selection_after_auxiliary_read = Some(selection.cost);
            cost.whole_budget_after_auxiliary_selection = Some(budget_cost(remaining_budget));
            cost.shared_route_operations_after_auxiliary_selection =
                Some(sources.operation_count());
            cost.source_root_component_opens_after_auxiliary_selection =
                Some(sources.root_component_open_count());

            adopt_candidate_shared_io_with_budget(
                original_io,
                candidate_io_adopted,
                remaining_write_bytes,
                remaining_budget,
            )
            .map_err(E::from)?;
            let (mut payloads, payload_facts) = run_candidate_shared_budget_window(
                remaining_budget,
                execution_limits,
                "payload-snapshot",
                FoundationWindowKind::Payload,
                FoundationPhaseReservation::default(),
                original_io,
                candidate_io_adopted,
                remaining_write_bytes,
                |execution_limits, _ticket| {
                    let derived = execution_limits
                        .payload_limits_from_ceilings(config.payload_limits)
                        .map_err(FoundationBootstrapError::Command)?;
                    let limits = intersect_payload_limits(derived, config.payload_limits)?;
                    let mut payloads = FoundationPayloadSources::new_candidate(
                        payload_root,
                        actual_input,
                        original_io,
                        limits,
                        deadline,
                        cancelled,
                    )
                    .map_err(FoundationBootstrapError::Payload)?;
                    let payload_facts = payloads
                        .snapshot_facts(&selection.payload_paths)
                        .map_err(FoundationBootstrapError::Payload)?;
                    if !payloads.shared_io_budget_matches(original_io)
                        || !payloads.forwards_payload_reads_to_shared_io()
                    {
                        return Err(FoundationBootstrapError::Configuration(
                            "candidate payload shared IO identity differs",
                        ));
                    }
                    let payload_cost = payloads.cost();
                    let retained_state = payload_cost
                        .retained_state_bytes
                        .checked_add(payload_cost.snapshot_facts_state_bytes)
                        .ok_or(FoundationBootstrapError::Configuration(
                            "payload snapshot state accounting overflow",
                        ))?;
                    if payload_cost.initial_bytes_read > limits.max_total_bytes
                        || payload_cost.peak_state_bytes > limits.max_state_bytes
                        || retained_state > limits.max_state_bytes
                    {
                        return Err(FoundationBootstrapError::Configuration(
                            "payload snapshot exceeded its whole-operation window",
                        ));
                    }
                    let use_amount = FoundationPhaseUse {
                        source_read_bytes: FoundationCharge::measured(
                            payload_cost
                                .initial_bytes_read
                                .checked_sub(payload_cost.shared_read_bytes_returned)
                                .ok_or(FoundationBootstrapError::Configuration(
                                    "candidate payload shared return accounting",
                                ))?,
                        ),
                        // The facts map moves into the physical snapshot and
                        // is charged here once; the physical window reuses it
                        // as an already-admitted state baseline.
                        state_bytes: FoundationCharge::admitted_upper_bound(retained_state),
                        ..FoundationPhaseUse::default()
                    };
                    Ok(((payloads, payload_facts), use_amount))
                },
            )
            .map_err(E::from)?;
            let payload_snapshot_cost = payloads.cost();
            cost.payload_after_snapshot = Some(payload_snapshot_cost);
            cost.whole_budget_after_payload_snapshot = Some(budget_cost(remaining_budget));

            adopt_candidate_shared_io_with_budget(
                original_io,
                candidate_io_adopted,
                remaining_write_bytes,
                remaining_budget,
            )
            .map_err(E::from)?;
            let physical_baseline = FoundationPhaseReservation {
                state_bytes: payload_snapshot_cost.snapshot_facts_state_bytes,
                ..FoundationPhaseReservation::default()
            };
            let mut physical = run_candidate_shared_budget_window(
                remaining_budget,
                execution_limits,
                "physical-snapshot",
                FoundationWindowKind::Physical,
                physical_baseline,
                original_io,
                candidate_io_adopted,
                remaining_write_bytes,
                |execution_limits, _ticket| {
                    let derived = execution_limits
                        .physical_limits_from_ceilings(config.physical_limits)
                        .map_err(FoundationBootstrapError::Command)?;
                    let limits = intersect_physical_limits(derived, config.physical_limits)?;
                    let mut physical =
                        FoundationPhysicalSnapshot::observe_candidate_with_resolved_targets(
                            sources,
                            actual_input,
                            coverage,
                            original_io,
                            &selection.authored_paths,
                            payload_facts,
                            &selection.private_paths,
                            &selection.private_prefixes,
                            artifact_roots.as_mut(),
                            &selection.artifact_paths,
                            &selection.resolved_source_directories,
                            limits,
                            deadline,
                            cancelled,
                            git_signal,
                        )
                        .map_err(FoundationBootstrapError::Physical)?;
                    if let Some(history) = history.as_deref_mut() {
                        physical
                            .select_historical(history)
                            .map_err(FoundationBootstrapError::Physical)?;
                    }
                    if !physical.shared_io_budget_matches(original_io)
                        || !physical.forwards_physical_reads_to_shared_io()
                    {
                        return Err(FoundationBootstrapError::Configuration(
                            "candidate physical shared IO identity differs",
                        ));
                    }
                    let physical_cost = physical.cost();
                    let incremental_state = physical_cost
                        .retained_state_bytes
                        .checked_sub(physical_baseline.state_bytes)
                        .ok_or(FoundationBootstrapError::Configuration(
                            "physical snapshot state cost regressed",
                        ))?;
                    if physical_cost.bytes_read > limits.max_total_bytes
                        || physical_cost.retained_state_bytes > limits.max_state_bytes
                    {
                        return Err(FoundationBootstrapError::Configuration(
                            "physical snapshot exceeded its whole-operation window",
                        ));
                    }
                    let use_amount = FoundationPhaseUse {
                        source_read_bytes: FoundationCharge::measured(
                            u64::try_from(
                                physical_cost
                                    .bytes_read
                                    .checked_sub(physical_cost.shared_read_bytes_returned)
                                    .ok_or(FoundationBootstrapError::Configuration(
                                        "candidate physical shared return accounting",
                                    ))?,
                            )
                            .map_err(|_| {
                                FoundationBootstrapError::Configuration(
                                    "physical snapshot read cost range",
                                )
                            })?,
                        ),
                        state_bytes: FoundationCharge::admitted_upper_bound(incremental_state),
                        ..FoundationPhaseUse::default()
                    };
                    Ok((physical, use_amount))
                },
            )
            .map_err(E::from)?;
            let initial_physical_cost = physical.cost();
            cost.physical_after_snapshot = Some(initial_physical_cost);
            cost.whole_budget_after_physical_snapshot = Some(budget_cost(remaining_budget));
            cost.shared_route_operations_after_snapshot = sources.operation_count();
            cost.source_root_component_opens_after_snapshot =
                Some(sources.root_component_open_count());
            cost.payload_root_component_opens_after_snapshot =
                Some(payloads.root_component_open_count());
            cost.artifact_root_component_opens_after_snapshot = artifact_roots
                .as_ref()
                .map(|root| root.root_component_open_count());

            adopt_candidate_shared_io_with_budget(
                original_io,
                candidate_io_adopted,
                remaining_write_bytes,
                remaining_budget,
            )
            .map_err(E::from)?;
            let view = CandidateFoundationBootstrapView {
                clock,
                launch,
                invocation,
                execution_limits,
                remaining_budget,
                selected_roots,
                stage,
                isolated,
                sources,
                artifact_sources: artifact_roots.as_mut(),
                input,
                coverage,
                original_epoch,
                original_io,
                candidate_io_adopted,
                remaining_write_bytes,
                selection,
                physical: &mut physical,
                cancelled,
                cost,
                history,
            };
            let outcome = run(view, payloads);
            let final_physical_cost = physical.cost();
            (outcome, final_physical_cost)
        };

        self.cost.physical_after_callback = Some(outcome.1);
        self.cost.invocation_after_callback = Some(self.invocation.cost);
        self.cost.whole_budget_after_callback = Some(budget_cost(&self.remaining_budget));
        self.cost.shared_route_operations_after_callback = Some(self.sources.operation_count());
        self.cost.source_root_component_opens_after_callback =
            Some(self.sources.root_component_open_count());
        self.cost.payload_root_component_opens_after_callback =
            Some(self.payload_sources.root_component_open_count());
        self.cost.artifact_root_component_opens_after_callback = self
            .artifact_sources
            .as_ref()
            .map(RouteSources::root_component_open_count);
        outcome.0
    }
}

/// Candidate windows account all hooked IO through the one original ledger.
/// On failure only the shared read term is known exactly; other unknown terms
/// retain the ordinary window's conservative failure law.
fn run_candidate_shared_budget_window<T, F>(
    budget: &mut FoundationRemainingBudget<'_>,
    execution_limits: &mut FoundationExecutionLimits,
    label: &'static str,
    kind: FoundationWindowKind,
    already_admitted: FoundationPhaseReservation,
    original_io: &tos_source_store::PinnedSqliteIoBudget,
    adopted: &mut (u64, u64),
    remaining_write_bytes: &mut u64,
    work: F,
) -> Result<T, FoundationBootstrapError>
where
    F: FnOnce(
        &mut FoundationExecutionLimits,
        &FoundationBudgetTicket,
    ) -> Result<(T, FoundationPhaseUse), FoundationBootstrapError>,
{
    adopt_candidate_shared_io_with_budget(original_io, adopted, remaining_write_bytes, budget)?;
    let ticket = budget
        .begin_window(label, already_admitted)
        .map_err(FoundationBootstrapError::Command)?;
    let worst = admitted_remaining(&ticket);
    if let Err(error) = execution_limits.select_window(&ticket, kind) {
        // No work/read occurred in this window.
        let _ = budget.fail_window_with_measured_source_reads(ticket, worst, 0);
        return Err(FoundationBootstrapError::Command(error));
    }
    let outcome = work(execution_limits, &ticket);
    let clear_result = execution_limits.clear_window(&ticket);
    let usage = original_io.snapshot();
    let read = usage.read_attempted_bytes.checked_sub(adopted.0);
    let write = usage.write_attempted_bytes.checked_sub(adopted.1);
    let remaining_write = write.and_then(|used| remaining_write_bytes.checked_sub(used));
    let Some(read) = read else {
        let _ = budget.fail_window(ticket, worst);
        return Err(FoundationBootstrapError::Configuration(
            "candidate shared read accounting regressed",
        ));
    };
    // This read term is observable even when a denied permit made the shared
    // ledger sticky-failed. Keep its actual attempt debit without refund.
    let accounting = validate_candidate_shared_io_snapshot(&usage).and_then(|()| {
        remaining_write.ok_or(FoundationBootstrapError::Configuration(
            "candidate shared write headroom exceeded",
        ))
    });
    let result = match (outcome, clear_result, accounting) {
        (Ok((value, mut actual)), Ok(()), Ok(_)) => {
            match actual.source_read_bytes.amount.checked_add(read) {
                Some(total) => {
                    actual.source_read_bytes = FoundationCharge::measured(total);
                    budget
                        .complete_window(ticket, actual)
                        .map_err(FoundationBootstrapError::Command)
                        .map(|()| value)
                }
                None => {
                    let _ = budget.fail_window_with_measured_source_reads(ticket, worst, read);
                    Err(FoundationBootstrapError::Configuration(
                        "candidate shared read charge overflow",
                    ))
                }
            }
        }
        (Err(error), _, _) => {
            let _ = budget.fail_window_with_measured_source_reads(ticket, worst, read);
            Err(error)
        }
        (_, Err(error), _) => {
            let _ = budget.fail_window_with_measured_source_reads(ticket, worst, read);
            Err(FoundationBootstrapError::Command(error))
        }
        (_, _, Err(error)) => {
            let _ = budget.fail_window_with_measured_source_reads(ticket, worst, read);
            Err(error)
        }
    };
    // Both close paths retained the shared read debit, including terminal Err.
    adopted.0 = usage.read_attempted_bytes;
    if write.is_some() {
        adopted.1 = usage.write_attempted_bytes;
    }
    if let Some(remaining) = remaining_write {
        *remaining_write_bytes = remaining;
    }
    result
}

fn validate_candidate_shared_io_snapshot(
    usage: &tos_source_store::PinnedSqliteIoSnapshot,
) -> Result<(), FoundationBootstrapError> {
    if usage.failure.is_some()
        || usage.read_permitted_bytes > usage.read_attempted_bytes
        || usage.read_returned_bytes > usage.read_permitted_bytes
        || usage.write_permitted_bytes > usage.write_attempted_bytes
        || usage.write_returned_bytes > usage.write_permitted_bytes
    {
        return Err(FoundationBootstrapError::Configuration(
            "candidate original shared IO accounting refused",
        ));
    }
    Ok(())
}

/// Adopt actual attempted reads from the SAME original ledger before granting
/// another independent window. Native receives these exact adopted counters
/// before it bills any later candidate delta.
pub(crate) fn adopt_candidate_shared_io_with_budget(
    original_io: &tos_source_store::PinnedSqliteIoBudget,
    adopted: &mut (u64, u64),
    remaining_write_bytes: &mut u64,
    budget: &mut FoundationRemainingBudget<'_>,
) -> Result<(), FoundationBootstrapError> {
    let usage = original_io.snapshot();
    validate_candidate_shared_io_snapshot(&usage)?;
    let read = usage.read_attempted_bytes.checked_sub(adopted.0).ok_or(
        FoundationBootstrapError::Configuration("candidate shared read accounting regressed"),
    )?;
    let write = usage.write_attempted_bytes.checked_sub(adopted.1).ok_or(
        FoundationBootstrapError::Configuration("candidate shared write accounting regressed"),
    )?;
    let remaining_write =
        remaining_write_bytes
            .checked_sub(write)
            .ok_or(FoundationBootstrapError::Configuration(
                "candidate shared write headroom exceeded",
            ))?;
    let ticket = budget
        .begin_window(
            "candidate-shared-io-adoption",
            FoundationPhaseReservation::default(),
        )
        .map_err(FoundationBootstrapError::Command)?;
    let debit = budget.complete_window(
        ticket,
        FoundationPhaseUse {
            source_read_bytes: FoundationCharge::measured(read),
            ..FoundationPhaseUse::default()
        },
    );
    // complete_window retains the measured debit even on terminal refusal.
    *adopted = (usage.read_attempted_bytes, usage.write_attempted_bytes);
    *remaining_write_bytes = remaining_write;
    debit.map_err(FoundationBootstrapError::Command)?;
    let remaining = budget
        .remaining()
        .map_err(FoundationBootstrapError::Command)?;
    original_io
        .restrict_remaining_io(remaining.source_read_bytes, remaining_write)
        .map_err(|_| {
            FoundationBootstrapError::Configuration(
                "candidate original shared IO narrowing refused",
            )
        })
}

/// Conservative custody for a present protected control, even when this
/// invocation selects the legacy absent epoch. The maintained parse envelope
/// includes raw/parser nodes; two envelopes cover selected DOM, the token
/// validation clone and simultaneous canonical buffers without assuming a
/// trivial epoch shape. It is admitted BEFORE the protected read.
pub(crate) fn candidate_original_epoch_state_upper_bound() -> Result<usize, FoundationBootstrapError>
{
    super::foundation_bootstrap_config::control_parse_state()
        .map_err(FoundationBootstrapError::Command)?
        .checked_mul(2)
        .and_then(|n| n.checked_add(size_of::<tos_source_store::MetadataPublicationEpoch>()))
        .ok_or(FoundationBootstrapError::Configuration(
            "candidate original epoch state overflow",
        ))
}

/// Verify the same original protected epoch after workers, with its actual
/// retained custody reused as baseline and a separately reserved new parse /
/// token / canonical-comparison workspace. No candidate control is admitted
/// in place of the original selected control.
pub(crate) fn verify_candidate_original_epoch_with_budget(
    sources: &mut RouteSources,
    epoch: &tos_source_store::MetadataPublicationEpoch,
    execution_limits: &mut FoundationExecutionLimits,
    budget: &mut FoundationRemainingBudget<'_>,
) -> Result<(), FoundationBootstrapError> {
    let retained = candidate_original_epoch_state_upper_bound()?;
    run_budget_window(
        budget,
        execution_limits,
        "candidate-original-epoch-eof",
        FoundationWindowKind::Capture,
        FoundationPhaseReservation {
            state_bytes: retained,
            ..FoundationPhaseReservation::default()
        },
        |_limits, ticket| {
            let operation = ticket.operation_limits();
            let peak = retained
                .checked_mul(2)
                .ok_or(FoundationBootstrapError::Configuration(
                    "candidate epoch EOF state overflow",
                ))?;
            if operation.state_bytes < peak
                || operation.source_read_bytes < PUBLICATION_CONTROL_READ_CAP_BYTES as u64
            {
                return Err(FoundationBootstrapError::Configuration(
                    "candidate epoch EOF reservation",
                ));
            }
            let read = foundation_capture::verify_epoch_with_cost(sources, epoch)
                .map_err(FoundationBootstrapError::Capture)?;
            if read > PUBLICATION_CONTROL_READ_CAP_BYTES {
                return Err(FoundationBootstrapError::Configuration(
                    "candidate epoch EOF read bound",
                ));
            }
            Ok((
                (),
                FoundationPhaseUse {
                    source_read_bytes: FoundationCharge::measured(read as u64),
                    ..FoundationPhaseUse::default()
                },
            ))
        },
    )
}

/// Final protected invocation/self-image custody for identity-only or genuine
/// unchanged admission paths. The full evaluator owns this same check itself;
/// callers must not repeat it after a validated Host completion.
pub(crate) fn verify_invocation_with_budget(
    invocation: &mut FoundationInvocation<'_>,
    budget: &mut FoundationRemainingBudget<'_>,
) -> Result<(), FoundationBootstrapError> {
    budget
        .verify_invocation(invocation)
        .map_err(FoundationBootstrapError::Command)?;
    let baseline = FoundationPhaseReservation {
        state_bytes: invocation.cost.retained_input_bytes,
        ..FoundationPhaseReservation::default()
    };
    let ticket = budget
        .begin_window("final-invocation-custody", baseline)
        .map_err(FoundationBootstrapError::Command)?;
    let admitted_failure = admitted_remaining(&ticket);
    let operation = ticket.operation_limits();
    let before = invocation
        .cost
        .invocation_read_bytes
        .checked_add(invocation.cost.self_image_read_bytes)
        .ok_or(FoundationBootstrapError::Configuration(
            "invocation custody prior read overflow",
        ));
    let result =
        (|| {
            let before = before?;
            let image =
                std::fs::metadata("/proc/self/exe").map_err(FoundationBootstrapError::RouteRoot)?;
            let read_upper = image.len().checked_add(1_048_576).ok_or(
                FoundationBootstrapError::Configuration("invocation custody read bound overflow"),
            )?;
            let state_upper = invocation
                .cost
                .retained_input_bytes
                .checked_add(1_048_576 + 65_536)
                .ok_or(FoundationBootstrapError::Configuration(
                    "invocation custody state bound overflow",
                ))?;
            if !image.is_file()
                || image.len() == 0
                || read_upper > operation.source_read_bytes
                || state_upper > operation.state_bytes
            {
                return Err(FoundationBootstrapError::Configuration(
                    "invocation custody exceeds remaining budget",
                ));
            }
            invocation
                .verify_before_source_eof()
                .map_err(FoundationBootstrapError::Command)?;
            let after = invocation
                .cost
                .invocation_read_bytes
                .checked_add(invocation.cost.self_image_read_bytes)
                .ok_or(FoundationBootstrapError::Configuration(
                    "invocation custody read overflow",
                ))?;
            let read = after
                .checked_sub(before)
                .ok_or(FoundationBootstrapError::Configuration(
                    "invocation custody read regressed",
                ))?;
            if read > read_upper {
                return Err(FoundationBootstrapError::Configuration(
                    "invocation custody read exceeded bound",
                ));
            }
            Ok(FoundationPhaseUse {
                source_read_bytes: FoundationCharge::measured(read),
                ..FoundationPhaseUse::default()
            })
        })();
    match result {
        Ok(usage) => budget
            .complete_window(ticket, usage)
            .map_err(FoundationBootstrapError::Command),
        Err(error) => {
            budget
                .fail_window(ticket, admitted_failure)
                .map_err(FoundationBootstrapError::Command)?;
            Err(error)
        }
    }
}

fn budget_cost(budget: &FoundationRemainingBudget<'_>) -> FoundationBudgetCost {
    FoundationBudgetCost {
        charged: budget.charged(),
        measured: budget.measured_charged(),
        admitted_upper_bound: budget.admitted_charged(),
    }
}

fn charge_entry_source_reads(
    budget: &mut FoundationRemainingBudget<'_>,
    bytes: u64,
) -> Result<(), FoundationBootstrapError> {
    let ticket = budget
        .begin_window("entry-inputs", FoundationPhaseReservation::default())
        .map_err(FoundationBootstrapError::Command)?;
    budget
        .complete_window(
            ticket,
            FoundationPhaseUse {
                source_read_bytes: FoundationCharge::measured(bytes),
                ..FoundationPhaseUse::default()
            },
        )
        .map_err(FoundationBootstrapError::Command)
}

fn charge_admitted_state(
    budget: &mut FoundationRemainingBudget<'_>,
    label: &'static str,
    bytes: usize,
) -> Result<(), FoundationBootstrapError> {
    let ticket = budget
        .begin_window(label, FoundationPhaseReservation::default())
        .map_err(FoundationBootstrapError::Command)?;
    budget
        .complete_window(
            ticket,
            FoundationPhaseUse {
                state_bytes: FoundationCharge::admitted_upper_bound(bytes),
                ..FoundationPhaseUse::default()
            },
        )
        .map_err(FoundationBootstrapError::Command)
}

fn preflight_capture_headroom(
    budget: &mut FoundationRemainingBudget<'_>,
    config: &FoundationBootstrapConfig,
) -> Result<(), FoundationBootstrapError> {
    let ticket = budget
        .begin_window("capture-headroom", FoundationPhaseReservation::default())
        .map_err(FoundationBootstrapError::Command)?;
    let remaining = ticket.remaining();
    let required_read_bytes = usize::try_from(
        super::foundation_bootstrap_config::capture_control_read_upper_bound_bytes()
            .map_err(FoundationBootstrapError::Command)?,
    )
    .ok()
    .and_then(|bytes| bytes.checked_add(CAPTURE_MEMBER_READ_PASSES))
    .and_then(|bytes| u64::try_from(bytes).ok())
    .ok_or(FoundationBootstrapError::Configuration(
        "capture headroom read calculation overflow",
    ))?;
    let capture_fits = config.capture_state_upper_bound_bytes <= remaining.state_bytes
        && config.capture_tmpfs_write_upper_bound_bytes <= remaining.tmpfs_bytes
        && remaining.source_read_bytes >= required_read_bytes
        && remaining.tmpfs_inodes
            >= u64::try_from(CAPTURE_TMPFS_FIXED_INODES + 1).map_err(|_| {
                FoundationBootstrapError::Configuration("capture headroom inode range")
            })?;
    budget
        .complete_window(ticket, FoundationPhaseUse::default())
        .map_err(FoundationBootstrapError::Command)?;
    if !capture_fits {
        return Err(FoundationBootstrapError::Configuration(
            "remaining invocation budget cannot admit the capture window",
        ));
    }
    Ok(())
}

/// Logical retained-state upper bound for the selected-root value and held
/// route wrappers. This is a modeled Rust-state charge; it is not an RSS
/// measurement or a claim about allocator metadata or kernel file handles.
fn selected_roots_state_upper_bound(
    selected_roots: &FoundationSelectedRoots,
    payload_root: &std::path::Path,
) -> Result<usize, FoundationBootstrapError> {
    let roots = [
        Some(&selected_roots.repo_root),
        selected_roots.payload_source_root.as_ref(),
        selected_roots.artifact_root.as_ref(),
    ];
    let root_count = roots.iter().filter(|path| path.is_some()).count();
    let selected_path_bytes = roots.iter().try_fold(0usize, |sum, path| {
        sum.checked_add(path.map_or(0, |path| path.as_os_str().as_bytes().len()))
    });
    let selected_path_bytes = selected_path_bytes.ok_or(
        FoundationBootstrapError::Configuration("selected-root path-state overflow"),
    )?;
    let held_path_bytes = selected_path_bytes;
    let route_wrappers = size_of::<RouteSources>().checked_mul(root_count).ok_or(
        FoundationBootstrapError::Configuration("selected-root route-state overflow"),
    )?;
    let shared_operation_arc =
        size_of::<AtomicUsize>()
            .checked_mul(3)
            .ok_or(FoundationBootstrapError::Configuration(
                "selected-root route-state overflow",
            ))?;
    let custody_arc_headers = size_of::<AtomicUsize>()
        .checked_mul(root_count)
        .and_then(|bytes| bytes.checked_mul(2))
        .ok_or(FoundationBootstrapError::Configuration(
            "selected-root custody-state overflow",
        ))?;
    size_of::<FoundationSelectedRoots>()
        .checked_add(route_wrappers)
        .and_then(|bytes| bytes.checked_add(selected_path_bytes))
        .and_then(|bytes| bytes.checked_add(held_path_bytes))
        .and_then(|bytes| bytes.checked_add(payload_root.as_os_str().as_bytes().len()))
        .and_then(|bytes| bytes.checked_add(size_of::<std::path::PathBuf>()))
        .and_then(|bytes| bytes.checked_add(shared_operation_arc))
        .and_then(|bytes| bytes.checked_add(custody_arc_headers))
        .ok_or(FoundationBootstrapError::Configuration(
            "selected-root retained-state overflow",
        ))
}

fn run_budget_window<T, F>(
    budget: &mut FoundationRemainingBudget<'_>,
    execution_limits: &mut FoundationExecutionLimits,
    label: &'static str,
    kind: FoundationWindowKind,
    already_admitted: FoundationPhaseReservation,
    work: F,
) -> Result<T, FoundationBootstrapError>
where
    F: FnOnce(
        &mut FoundationExecutionLimits,
        &FoundationBudgetTicket,
    ) -> Result<(T, FoundationPhaseUse), FoundationBootstrapError>,
{
    let ticket = budget
        .begin_window(label, already_admitted)
        .map_err(FoundationBootstrapError::Command)?;
    let worst = admitted_remaining(&ticket);
    if let Err(error) = execution_limits.select_window(&ticket, kind) {
        let _ = budget.fail_window(ticket, worst);
        return Err(FoundationBootstrapError::Command(error));
    }

    let outcome = work(execution_limits, &ticket);
    let clear_result = execution_limits.clear_window(&ticket);
    match outcome {
        Ok((value, actual)) => {
            if let Err(error) = clear_result {
                let _ = budget.fail_window(ticket, worst);
                return Err(FoundationBootstrapError::Command(error));
            }
            budget
                .complete_window(ticket, actual)
                .map_err(FoundationBootstrapError::Command)?;
            Ok(value)
        }
        Err(error) => {
            let _ = clear_result;
            let _ = budget.fail_window(ticket, worst);
            Err(error)
        }
    }
}

fn admitted_remaining(ticket: &FoundationBudgetTicket) -> FoundationPhaseUse {
    let remaining = ticket.remaining();
    FoundationPhaseUse {
        source_read_bytes: FoundationCharge::admitted_upper_bound(remaining.source_read_bytes),
        worker_wire_bytes: FoundationCharge::admitted_upper_bound(remaining.worker_wire_bytes),
        state_bytes: FoundationCharge::admitted_upper_bound(remaining.state_bytes),
        issue_count: FoundationCharge::admitted_upper_bound(remaining.issue_count),
        output_bytes: FoundationCharge::admitted_upper_bound(remaining.output_bytes),
        // These bootstrap windows perform no selected schema-worker exchange.
        // The bounded read-only Git consumer has its own physical cost route.
        worker_cpu: FoundationWorkerCpuUse::AdmittedSeconds(0),
        tmpfs_bytes: FoundationCharge::admitted_upper_bound(remaining.tmpfs_bytes),
        tmpfs_inodes: FoundationCharge::admitted_upper_bound(remaining.tmpfs_inodes),
    }
}

fn effective_capture_limits(
    invocation: &FoundationInvocation<'_>,
    config: &FoundationBootstrapConfig,
    ticket: &FoundationBudgetTicket,
) -> Result<(ReadLimits, CutReadLimits, usize), FoundationBootstrapError> {
    let remaining = ticket.remaining();
    if config.capture_state_upper_bound_bytes > remaining.state_bytes
        || config.capture_tmpfs_write_upper_bound_bytes > remaining.tmpfs_bytes
    {
        return Err(FoundationBootstrapError::Configuration(
            "capture state or tmpfs write upper bound exceeds remaining invocation budget",
        ));
    }

    let control_bytes = usize::try_from(
        super::foundation_bootstrap_config::capture_control_read_upper_bound_bytes()
            .map_err(FoundationBootstrapError::Command)?,
    )
    .map_err(|_| FoundationBootstrapError::Configuration("capture control reservation overflow"))?;
    let available_read_bytes = usize::try_from(remaining.source_read_bytes)
        .map_err(|_| FoundationBootstrapError::Configuration("capture read limit range"))?;
    let available_member_bytes = available_read_bytes.checked_sub(control_bytes).ok_or(
        FoundationBootstrapError::Configuration(
            "remaining source-read budget cannot reserve capture controls",
        ),
    )?;
    let remaining_pass_limit = available_member_bytes / CAPTURE_MEMBER_READ_PASSES;
    let tmpfs_write_limit = usize::try_from(
        config
            .capture_tmpfs_write_upper_bound_bytes
            .min(remaining.tmpfs_bytes),
    )
    .map_err(|_| FoundationBootstrapError::Configuration("capture tmpfs write limit range"))?;
    // Only materialization writes member objects. The other two passes are
    // immutable/live EOF reads; they do not multiply tmpfs writes. Actual
    // object bytes plus the encoded manifest share the capture write cap.
    let remaining_tmpfs_pass_limit = tmpfs_write_limit;
    let cut_pass_limit = usize::try_from(config.cut_limits.max_total_bytes)
        .map_err(|_| FoundationBootstrapError::Configuration("capture cut read limit range"))?;
    let per_pass_limit = config
        .capture_member_bytes_per_pass
        .min(remaining_pass_limit)
        .min(remaining_tmpfs_pass_limit)
        .min(cut_pass_limit);
    let aggregate_member_limit = per_pass_limit
        .checked_mul(CAPTURE_MEMBER_READ_PASSES)
        .ok_or(FoundationBootstrapError::Configuration(
            "capture member reservation overflow",
        ))?;
    let remaining_inodes = usize::try_from(remaining.tmpfs_inodes)
        .map_err(|_| FoundationBootstrapError::Configuration("capture inode limit range"))?;
    let member_inode_limit = remaining_inodes
        .checked_sub(CAPTURE_TMPFS_FIXED_INODES)
        .ok_or(FoundationBootstrapError::Configuration(
            "remaining tmpfs inode budget cannot reserve capture structure",
        ))?;
    let invocation_member_limit = usize::try_from(invocation.budgets.max_current_members)
        .map_err(|_| FoundationBootstrapError::Configuration("capture member limit range"))?;
    let cut_member_limit = usize::try_from(config.cut_limits.max_members)
        .map_err(|_| FoundationBootstrapError::Configuration("capture cut member limit range"))?;
    let max_manifest_entries = config
        .read_limits
        .max_manifest_entries
        .min(invocation_member_limit)
        .min(cut_member_limit)
        .min(member_inode_limit);
    if per_pass_limit == 0 || max_manifest_entries == 0 {
        return Err(FoundationBootstrapError::Configuration(
            "remaining whole-operation budget cannot admit a capture member",
        ));
    }

    let per_pass_u64 = u64::try_from(per_pass_limit)
        .map_err(|_| FoundationBootstrapError::Configuration("capture member limit range"))?;
    let aggregate_u64 = u64::try_from(aggregate_member_limit)
        .map_err(|_| FoundationBootstrapError::Configuration("capture aggregate limit range"))?;
    let mut read_limits = config.read_limits;
    read_limits.max_manifest_entries = max_manifest_entries;
    read_limits.max_selected_object_bytes = read_limits
        .max_selected_object_bytes
        .min(per_pass_u64)
        .min(invocation.budgets.max_member_bytes);
    let mut cut_limits = config.cut_limits;
    cut_limits.max_members = cut_limits.max_members.min(
        u64::try_from(max_manifest_entries)
            .map_err(|_| FoundationBootstrapError::Configuration("capture member limit range"))?,
    );
    cut_limits.max_total_bytes = cut_limits.max_total_bytes.min(per_pass_u64);
    cut_limits.max_member_bytes = cut_limits
        .max_member_bytes
        .min(per_pass_u64)
        .min(invocation.budgets.max_member_bytes);
    if read_limits.max_selected_object_bytes == 0
        || cut_limits.max_total_bytes == 0
        || cut_limits.max_member_bytes == 0
        || cut_limits.max_members < read_limits.max_manifest_entries as u64
        || cut_limits.max_total_bytes > aggregate_u64
    {
        return Err(FoundationBootstrapError::Configuration(
            "effective capture limits are empty or inconsistent",
        ));
    }
    Ok((read_limits, cut_limits, aggregate_member_limit))
}

fn intersect_selection_limits(
    mut derived: FoundationSelectionLimits,
    requested: FoundationSelectionLimits,
    deadline: Instant,
) -> Result<FoundationSelectionLimits, FoundationBootstrapError> {
    derived.max_selector_documents = derived
        .max_selector_documents
        .min(requested.max_selector_documents);
    derived.max_output_entries = derived.max_output_entries.min(requested.max_output_entries);
    derived.max_private_prefixes = derived
        .max_private_prefixes
        .min(requested.max_private_prefixes);
    derived.max_payload_paths = derived.max_payload_paths.min(requested.max_payload_paths);
    derived.max_member_bytes = derived.max_member_bytes.min(requested.max_member_bytes);
    derived.max_total_read_bytes = derived
        .max_total_read_bytes
        .min(requested.max_total_read_bytes);
    derived.max_state_bytes = derived.max_state_bytes.min(requested.max_state_bytes);
    derived.deadline = deadline;
    if derived.max_selector_documents == 0
        || derived.max_output_entries == 0
        || derived.max_private_prefixes == 0
        || derived.max_payload_paths == 0
        || derived.max_member_bytes == 0
        || derived.max_total_read_bytes == 0
        || derived.max_state_bytes == 0
    {
        return Err(FoundationBootstrapError::Configuration(
            "selection window has no remaining capacity",
        ));
    }
    Ok(derived)
}

fn intersect_payload_limits(
    mut derived: PhysicalPayloadLimits,
    requested: PhysicalPayloadLimits,
) -> Result<PhysicalPayloadLimits, FoundationBootstrapError> {
    derived.max_files = derived.max_files.min(requested.max_files);
    derived.max_observations = derived.max_observations.min(requested.max_observations);
    derived.max_file_bytes = derived.max_file_bytes.min(requested.max_file_bytes);
    derived.max_total_bytes = derived.max_total_bytes.min(requested.max_total_bytes);
    derived.max_state_bytes = derived.max_state_bytes.min(requested.max_state_bytes);
    if derived.max_files == 0
        || derived.max_observations == 0
        || derived.max_file_bytes == 0
        || derived.max_total_bytes == 0
        || derived.max_state_bytes == 0
    {
        return Err(FoundationBootstrapError::Configuration(
            "payload window has no remaining capacity",
        ));
    }
    Ok(derived)
}

fn intersect_physical_limits(
    mut derived: PhysicalSourceLimits,
    requested: PhysicalSourceLimits,
) -> Result<PhysicalSourceLimits, FoundationBootstrapError> {
    derived.max_paths = derived.max_paths.min(requested.max_paths);
    derived.max_private_prefixes = derived
        .max_private_prefixes
        .min(requested.max_private_prefixes);
    derived.max_inventory_paths = derived
        .max_inventory_paths
        .min(requested.max_inventory_paths);
    derived.max_path_observations = derived
        .max_path_observations
        .min(requested.max_path_observations);
    derived.max_git_path_queries = derived
        .max_git_path_queries
        .min(requested.max_git_path_queries);
    derived.max_file_bytes = derived.max_file_bytes.min(requested.max_file_bytes);
    derived.max_total_bytes = derived.max_total_bytes.min(requested.max_total_bytes);
    derived.max_state_bytes = derived.max_state_bytes.min(requested.max_state_bytes);
    derived.git_output_bytes = derived.git_output_bytes.min(requested.git_output_bytes);
    derived.git_cleanup_grace = derived.git_cleanup_grace.min(requested.git_cleanup_grace);
    if derived.max_paths == 0
        || derived.max_private_prefixes == 0
        || derived.max_inventory_paths == 0
        || derived.max_path_observations == 0
        || derived.max_git_path_queries == 0
        || derived.max_file_bytes == 0
        || derived.max_total_bytes == 0
        || derived.max_state_bytes == 0
        || derived.git_output_bytes == 0
        || derived.git_cleanup_grace.is_zero()
    {
        return Err(FoundationBootstrapError::Configuration(
            "physical window has no remaining capacity",
        ));
    }
    Ok(derived)
}

fn checkpoint(deadline: Instant, cancelled: &AtomicBool) -> Result<(), SourceCommandError> {
    if cancelled.load(Ordering::Relaxed) {
        Err(SourceCommandError::Denied(
            "foundation invocation cancelled",
        ))
    } else if Instant::now() >= deadline {
        Err(SourceCommandError::Denied("foundation invocation deadline"))
    } else {
        Ok(())
    }
}

fn preflight(
    invocation: &FoundationInvocation<'_>,
    config: &FoundationBootstrapConfig,
) -> Result<(), FoundationBootstrapError> {
    if config.capture_member_bytes_per_pass == 0
        || config.capture_state_upper_bound_bytes == 0
        || config.capture_state_upper_bound_bytes == usize::MAX
        || config.capture_tmpfs_write_upper_bound_bytes == 0
        || config.capture_tmpfs_write_upper_bound_bytes == u64::MAX
        || config.read_limits.max_selected_object_bytes == 0
        || config.read_limits.max_selected_object_bytes == u64::MAX
        || config.read_limits.max_manifest_entries == 0
        || config.cut_limits.max_members == 0
        || config.cut_limits.max_members == u64::MAX
        || config.cut_limits.max_total_bytes == 0
        || config.cut_limits.max_total_bytes == u64::MAX
        || config.cut_limits.max_member_bytes == 0
        || config.cut_limits.max_member_bytes == u64::MAX
        || config.cut_limits.max_revisions == 0
        || config.cut_limits.max_revisions == usize::MAX
        || config.selection_limits.deadline != invocation.deadline()
        || config.selection_limits.max_selector_documents == 0
        || config.selection_limits.max_output_entries == 0
        || config.selection_limits.max_private_prefixes == 0
        || config.selection_limits.max_payload_paths == 0
        || config.selection_limits.max_member_bytes == 0
        || config.selection_limits.max_total_read_bytes == 0
        || config.selection_limits.max_total_read_bytes == u64::MAX
        || config.selection_limits.max_state_bytes == 0
        || config.selection_limits.max_state_bytes == usize::MAX
        || config.selection_limits.max_member_bytes == u64::MAX
        || config.physical_limits.max_paths == 0
        || config.physical_limits.max_private_prefixes == 0
        || config.physical_limits.max_inventory_paths == 0
        || config.physical_limits.max_path_observations == 0
        || config.physical_limits.max_git_path_queries == 0
        || config.physical_limits.max_file_bytes == 0
        || config.physical_limits.max_total_bytes == 0
        || config.physical_limits.max_state_bytes == 0
        || config.physical_limits.git_output_bytes == 0
        || config.physical_limits.git_output_bytes > 65_536
        || config.physical_limits.git_cleanup_grace.is_zero()
        || config.physical_limits.max_file_bytes == usize::MAX
        || config.payload_limits.max_file_bytes == 0
        || config.payload_limits.max_file_bytes == u64::MAX
        || config.payload_limits.max_files == 0
        || config.payload_limits.max_observations == 0
        || config.payload_limits.max_total_bytes == 0
        || config.payload_limits.max_total_bytes == u64::MAX
        || config.payload_limits.max_state_bytes == 0
        || config.payload_limits.max_state_bytes == usize::MAX
    {
        return Err(FoundationBootstrapError::Configuration(
            "bootstrap provider shapes exceed the protected invocation",
        ));
    }
    Ok(())
}
