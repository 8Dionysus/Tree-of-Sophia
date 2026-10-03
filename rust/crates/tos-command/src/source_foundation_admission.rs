//! Admission adapter over the same protected invocation and complete FND path.
//! No public proof constructor, alternate validator, or bootstrap clock exists.
use crate::source_admission::{AdmissionLimits, active, invalid};
use crate::source_admission_candidate::{Candidate, CandidateLimits};
use crate::source_admission_index::{self, CandidateInput, Index, IndexLimits};
use crate::source_admission_spooled_candidate::SpoolLimits;
use crate::source_admission_spooled_index::SpoolIndexLimits;
use crate::source_admission_spooled_manifest::ManifestStreamLimits;
use crate::source_creation_store::IsolatedCreationRoot;
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
    path::{Path, PathBuf},
    sync::{Arc, atomic::{AtomicBool, AtomicI32}},
    time::Instant,
};
use tos_compiler::private_tmpfs_stage::{
    PRIVATE_TMPFS_SELECT_COST, PRIVATE_TMPFS_VERIFY_COST, PrivateTmpfsStageIsolation,
};
use tos_foundation::{Digest256, JsonLimits};
use tos_ops_mechanics_plan::route_cards::RouteSources;
use tos_source_store::ReadLimits;
use tos_source_store::{
    CutReadLimits, PinnedSqliteAuxLimits, PinnedSqliteAuxRequest, PinnedSqliteIoBudget,
    PinnedSqliteSpaceBudget, StreamedCutReadLimitsV1,
};

/// Only this typed error carries the joined owner's public static phase label.
/// Other IO errors may contain private paths or diagnostics and are not printed.
#[derive(Debug)]
pub(crate) struct NativeValidationRefusal(pub(crate) &'static str);
impl std::fmt::Display for NativeValidationRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for NativeValidationRefusal {}

/// Unforgeable-in-crate completion witness consumed by the spooled index
/// adapter. Its constructor remains inside this validator module so a caller
/// cannot turn an index-only pass into a validated source view.
pub(crate) struct NativeAdmissionComplete(());
impl NativeAdmissionComplete {
    fn new() -> Self {
        Self(())
    }
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
            NativeValidationRefusal("protected invocation ancestor ownership boundary"),
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

/// Resource selection is made by the protected invocation. The legacy entry
/// remains resident; the shared-cancel entry may receive a bounded spooled
/// profile derived solely from that invocation's remaining limits.
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
    pub(crate) manifest_limits: ManifestStreamLimits,
    pub(crate) streamed_cut_limits: StreamedCutReadLimitsV1,
    pub(crate) max_index_allocated_bytes: u64,
    pub(crate) max_manifest_allocated_bytes: u64,
}

pub(crate) struct NativeSourceValidator<'c> {
    prepared: Option<Prepared<'c>>,
    evaluated: Option<FoundationBootstrapInputs<'c>>,
    grammar: GrammarIdentity,
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
        .begin_window("admission-spooled-profile", FoundationPhaseReservation::default())
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
        let remaining = ledger.remaining().map_err(command)?;
        let limits = IdentityLimits {
            max_read_bytes: remaining.source_read_bytes,
            max_member_bytes: count(invocation.budgets.max_member_bytes)?,
            max_members: count(invocation.budgets.max_current_members)?,
            max_discovery_entries: count(invocation.budgets.max_current_members)?,
            max_state_bytes: remaining.state_bytes,
        };
        let grammar = GrammarIdentity::select(
            &mut sources,
            invocation.executable_sha256(),
            invocation.schema_worker.sha256,
            limits,
            deadline,
            cancel,
        )?;
        debit(
            &mut ledger,
            "admission-grammar-identity",
            grammar.read_bytes(),
            grammar.retained_state_bytes(),
        )?;
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
                        max_manifest_entries: count(caps.max_current_members)?,
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
            history,
            history_usage,
            identity,
            deadline,
            cancel,
            git_signal,
            write_cap,
            candidate_io: (0, 0),
            candidate_state: 0,
            batch_charged: false,
            store_authority: None,
            shared_cancel: None,
            spooled_route_selected: false,
            execution_resources_taken: false,
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
        let mut prepared = Self::prepare(
            clock,
            args,
            cancelled.as_ref(),
            git_signal,
            select_output,
        )?;
        if !std::ptr::eq(prepared.cancel, Arc::as_ptr(cancelled)) {
            return Err(invalid("shared admission cancellation identity changed"));
        }
        prepared.shared_cancel = Some(Arc::clone(cancelled));
        prepared.spooled_route_selected = true;
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
                max_manifest_entries: members,
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
        if !self.spooled_route_selected || self.shared_cancel.is_none() {
            return Ok(PreparedAdmissionExecution::Resident);
        }
        if self.prepared.is_none() || self.evaluated.is_some() {
            return Err(invalid("spooled resources require a fresh prepared invocation"));
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
        let mut candidate_limits = match self.candidate_limits() {
            Ok(limits) => limits,
            Err(_) => return Ok(PreparedAdmissionExecution::Resident),
        };
        let write_remaining = self
            .write_cap
            .checked_sub(self.candidate_io.1)
            .ok_or_else(|| invalid("persistent admission write accounting regressed"))?;
        if remaining.source_read_bytes == 0
            || remaining.state_bytes < 64 * 1024
            || remaining.tmpfs_bytes < 512 * 1024
            || remaining.tmpfs_inodes < 8
            || write_remaining == 0
            || candidate_limits.max_read_bytes == u64::MAX
            || write_remaining == u64::MAX
        {
            return Ok(PreparedAdmissionExecution::Resident);
        }
        candidate_limits.max_write_bytes = write_remaining;
        if candidate_limits.admission.validate().is_err() {
            return Ok(PreparedAdmissionExecution::Resident);
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
            return Ok(PreparedAdmissionExecution::Resident);
        }
        let sqlite_space = declared_profile - ROOT_METADATA_BOUND;
        let candidate_partition = sqlite_space / 4;
        let index_partition = sqlite_space / 4;
        let reader_partition = sqlite_space / 4;
        let manifest_partition = sqlite_space / 8;
        if candidate_partition < MIN_SQLITE_PARTITION
            || index_partition < MIN_SQLITE_PARTITION
            || reader_partition < MIN_SQLITE_PARTITION
            || manifest_partition == 0
        {
            return Ok(PreparedAdmissionExecution::Resident);
        }

        // Keep room for the private directory and the four simultaneously
        // retained main-file handles. Candidate and native index auxiliary
        // ceilings split the remaining inode slice instead of each receiving
        // the full invocation count.
        let inode_profile = inode_available / 2;
        let auxiliary_slots = inode_profile.saturating_sub(5) / 2;
        let max_live_aux = usize::try_from(auxiliary_slots.min(16)).unwrap_or(16);
        if max_live_aux == 0 {
            return Ok(PreparedAdmissionExecution::Resident);
        }
        let Some(candidate_sqlite) = sqlite_aux_limits(candidate_partition, max_live_aux)
        else {
            return Ok(PreparedAdmissionExecution::Resident);
        };
        let Some(index_sqlite) = sqlite_aux_limits(index_partition, max_live_aux) else {
            return Ok(PreparedAdmissionExecution::Resident);
        };

        let state = remaining.state_bytes;
        let row_state = state / 16;
        let cache_bytes = (8 * 1024 * 1024usize).min(state / 32);
        if row_state == 0 || cache_bytes == 0 || cache_bytes > row_state {
            return Ok(PreparedAdmissionExecution::Resident);
        }
        let max_manifest_allocated_bytes = manifest_partition;
        let manifest_bytes = candidate_limits
            .reader
            .max_manifest_bytes
            .min(max_manifest_allocated_bytes);
        if manifest_bytes == 0 {
            return Ok(PreparedAdmissionExecution::Resident);
        }
        let max_members = candidate_limits.admission.max_members;
        let max_manifest_entries = max_members;
        let max_member_bytes = candidate_limits.admission.max_member_bytes;
        let max_total_bytes = candidate_limits.admission.max_source_bytes;
        let max_revisions = candidate_limits.max_history_revisions;
        if max_revisions == 0
            || max_members == 0
            || max_member_bytes == 0
            || max_total_bytes == 0
        {
            return Ok(PreparedAdmissionExecution::Resident);
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
            max_manifest_entries,
            max_index_bytes: reader_partition,
            max_manifest_row_bytes: usize::try_from(max_member_bytes.min(usize::MAX as u64 - 1))
                .map_err(|_| invalid("spooled member byte limit exceeds range"))?,
            sqlite_cache_bytes: cache_bytes,
        };
        let manifest_limits = ManifestStreamLimits {
            max_manifest_bytes: manifest_bytes,
            row_json: candidate_limits.admission.json,
        };

        let root_len = self
            .store_authority
            .as_ref()
            .ok_or_else(|| invalid("private stage authority disappeared"))?
            .root()
            .as_os_str()
            .len();
        let state_bound = size_of::<PreparedSpooledExecution>()
            .checked_add(size_of::<IsolatedCreationRoot>())
            .and_then(|n| n.checked_add(size_of::<PinnedSqliteIoBudget>()))
            .and_then(|n| n.checked_add(size_of::<PinnedSqliteSpaceBudget>()))
            .and_then(|n| n.checked_add(root_len.checked_mul(2)?))
            .and_then(|n| n.checked_add(4096))
            .ok_or_else(|| invalid("spooled profile retained state overflow"))?;
        let profile_inodes = inode_profile;
        let profile_bytes = declared_profile;
        if state_bound > remaining.state_bytes
            || profile_bytes > remaining.tmpfs_bytes
            || profile_inodes > remaining.tmpfs_inodes
        {
            return Ok(PreparedAdmissionExecution::Resident);
        }

        // Reserve the profile, directory metadata and inode ceiling before
        // creating the workspace or the ledger control blocks.
        debit_spooled_profile(
            self.ledger_mut()?,
            PRIVATE_TMPFS_VERIFY_COST.read_bytes,
            state_bound,
            profile_bytes,
            profile_inodes,
        )?;
        active(self.deadline, self.cancel)?;
        let authority = self
            .store_authority
            .as_ref()
            .ok_or_else(|| invalid("private stage authority disappeared"))?;
        let isolated = IsolatedCreationRoot::create(authority.root(), self.deadline, &cancelled)
            .map_err(command)?;
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
            .map_err(|error| invalid(format!("private stage post-create check refused: {error:?}")));
        self.charge_store_guard(PRIVATE_TMPFS_VERIFY_COST.read_bytes, 0)?;
        let after_usage = after_usage_result?;
        if after_usage.used_bytes < usage.used_bytes
            || after_usage
                .used_bytes
                .checked_sub(usage.used_bytes)
                .is_none_or(|n| n > ROOT_METADATA_BOUND)
            || after_usage.used_inodes < usage.used_inodes
            || after_usage
                .used_inodes
                .checked_sub(usage.used_inodes)
                .is_none_or(|n| n > 1)
        {
            return Err(invalid("private spooled workspace creation exceeded its precharge"));
        }
        active(self.deadline, self.cancel)?;

        let io_budget = PinnedSqliteIoBudget::new(
            candidate_limits.max_read_bytes,
            candidate_limits.max_write_bytes,
        )
        .map_err(invalid)?;
        let space_budget = PinnedSqliteSpaceBudget::new(sqlite_space).map_err(invalid)?;
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

        Ok(PreparedAdmissionExecution::Spooled(PreparedSpooledExecution {
            workspace_root: isolated,
            workspace,
            request,
            candidate_limits,
            index_limits,
            manifest_limits,
            streamed_cut_limits,
            max_index_allocated_bytes: reader_partition,
            max_manifest_allocated_bytes,
        }))
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
    pub(crate) fn validate(&mut self, candidate: &Candidate<'_>) -> io::Result<ValidatedCandidate> {
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
    pub(crate) fn write_receipt(
        &mut self,
        value: &Value,
        writer: &mut dyn Write,
    ) -> io::Result<()> {
        active(self.deadline, self.cancel)?;
        let ticket = self
            .ledger_mut()?
            .begin_window("admission-receipt", FoundationPhaseReservation::default())
            .map_err(command)?;
        let cap = ticket.remaining().output_bytes;
        // Same serializer counts before emitting; no unbounded intermediate Vec.
        let mut count = ReceiptWriter {
            inner: None,
            count: 0,
            cap,
            deadline: self.deadline,
            cancel: self.cancel,
        };
        serde_json::to_writer(&mut count, value).map_err(invalid)?;
        count.write_all(b"\n")?;
        let mut output = ReceiptWriter {
            inner: Some(writer),
            count: 0,
            cap,
            deadline: self.deadline,
            cancel: self.cancel,
        };
        serde_json::to_writer(&mut output, value).map_err(invalid)?;
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
