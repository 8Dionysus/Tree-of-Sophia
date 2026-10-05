//! Checked operation-wide reservations for the native source-foundation path.
//!
//! The fixed phase helper validates precomputed slices; the remaining-budget
//! ledger instead admits windows in actual call order and charges each once.
//! Both use the same protected invocation ceilings and absolute deadline.

use super::foundation_entry::{FoundationInvocation, FoundationInvocationBudgets};
use super::foundation_output::SourceFoundationOutputLimits;
use super::foundation_payload::PhysicalPayloadLimits;
use super::foundation_physical::PhysicalSourceLimits;
use super::foundation_reader::FoundationRuleReadLimits;
use super::foundation_rule_diagnostics::SourceFoundationRuleDiagnosticsLimits;
use super::foundation_selection::FoundationSelectionLimits;
use crate::source_command::{SourceCommandError as Error, SourceCommandResult as Result};
use std::mem::size_of;
use std::os::unix::ffi::OsStrExt;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tos_compiler::SourceCatalogInputLimits;
use tos_compiler::knowledge_stage::StageLimits;
use tos_compiler::source_bibliographic::BibliographicLimits;
use tos_compiler::source_witness_catalog::SourceCatalogLimits;
use tos_source_store::{CutReadLimits, ReadLimits};
use tos_validation::executor::{
    BatchBudget, BatchStreamBudget, ExactWorkerIdentity, ExecutorBudget,
};
use tos_validation::item_rules::ItemLimits;
use tos_validation::source_cut::{CutSchemaDiagnosticsLimits, CutWorkerLimits};
use tos_validation::source_foundation_default_rules::SourceFoundationDefaultRulesLimits;
use tos_validation::source_foundation_records::SourceFoundationRecordsLimits;
use tos_validation::source_foundation_schema::{
    SourceFoundationSchemaLimits, SourceFoundationSchemaSet,
};

/// One phase's upper-bound reservation. `source_read_bytes` includes all
/// selected-root reads in that phase, including protected controls and repeat
/// checks; invocation/executable hashing has separate counters. `state_bytes`
/// includes retained phase state; sums are checked because owner reports can
/// remain live together. Output and issue reservations compose into the final
/// CLI result. `tmpfs` covers bytes retained or written in the one selected
/// private stage.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct FoundationPhaseReservation {
    pub source_read_bytes: u64,
    /// Aggregate request/response bytes for selected worker operations in this
    /// phase. This is independent of bytes read from the source filesystem.
    pub worker_wire_bytes: u64,
    pub state_bytes: usize,
    pub issue_count: usize,
    pub output_bytes: usize,
    /// Worker-facing ceiling rounded up for APIs that accept seconds. This is
    /// populated on operation-limit snapshots and stays zero in usage totals.
    pub worker_cpu_seconds: u64,
    /// Exact cumulative worker CPU accounting in microseconds.
    pub worker_cpu_micros: u64,
    pub tmpfs_bytes: u64,
    pub tmpfs_inodes: u64,
}

/// Whether a charged amount was measured from completed work or admitted as
/// a conservative upper bound before work began.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum FoundationChargeBasis {
    #[default]
    Measured,
    AdmittedUpperBound,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FoundationCharge<T> {
    pub amount: T,
    pub basis: FoundationChargeBasis,
}

impl<T> FoundationCharge<T> {
    pub(crate) fn measured(amount: T) -> Self {
        Self {
            amount,
            basis: FoundationChargeBasis::Measured,
        }
    }

    pub(crate) fn admitted_upper_bound(amount: T) -> Self {
        Self {
            amount,
            basis: FoundationChargeBasis::AdmittedUpperBound,
        }
    }
}

/// Incremental usage for one completed or failed window. Every dimension has
/// its own basis because source bytes can be exact while retained state is a
/// conservative admitted bound.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FoundationPhaseUse {
    pub source_read_bytes: FoundationCharge<u64>,
    pub worker_wire_bytes: FoundationCharge<u64>,
    pub state_bytes: FoundationCharge<usize>,
    pub issue_count: FoundationCharge<usize>,
    pub output_bytes: FoundationCharge<usize>,
    pub worker_cpu: FoundationWorkerCpuUse,
    pub tmpfs_bytes: FoundationCharge<u64>,
    pub tmpfs_inodes: FoundationCharge<u64>,
}

impl Default for FoundationPhaseUse {
    fn default() -> Self {
        Self {
            source_read_bytes: FoundationCharge::measured(0),
            worker_wire_bytes: FoundationCharge::measured(0),
            state_bytes: FoundationCharge::measured(0),
            issue_count: FoundationCharge::measured(0),
            output_bytes: FoundationCharge::measured(0),
            worker_cpu: FoundationWorkerCpuUse::MeasuredMicros(0),
            tmpfs_bytes: FoundationCharge::measured(0),
            tmpfs_inodes: FoundationCharge::measured(0),
        }
    }
}

impl FoundationPhaseUse {
    fn amounts(self) -> Result<FoundationPhaseReservation> {
        Ok(FoundationPhaseReservation {
            source_read_bytes: self.source_read_bytes.amount,
            worker_wire_bytes: self.worker_wire_bytes.amount,
            state_bytes: self.state_bytes.amount,
            issue_count: self.issue_count.amount,
            output_bytes: self.output_bytes.amount,
            worker_cpu_seconds: 0,
            worker_cpu_micros: self.worker_cpu.micros()?,
            tmpfs_bytes: self.tmpfs_bytes.amount,
            tmpfs_inodes: self.tmpfs_inodes.amount,
        })
    }
}

/// Worker CPU usage stays precise across windows. Failure reservations use
/// admitted exact microseconds; an integral-second allowance is distinct from
/// the rounded seconds used only for a child RLIMIT. Completed reports supply
/// the exact controller-observed microsecond total.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FoundationWorkerCpuUse {
    MeasuredMicros(u64),
    AdmittedMicros(u64),
    AdmittedSeconds(u64),
}

impl FoundationWorkerCpuUse {
    fn micros(self) -> Result<u64> {
        match self {
            Self::MeasuredMicros(micros) | Self::AdmittedMicros(micros) => Ok(micros),
            Self::AdmittedSeconds(seconds) => seconds
                .checked_mul(1_000_000)
                .ok_or(Error::Unsupported("foundation CPU reservation overflow")),
        }
    }

    fn is_measured(self) -> bool {
        matches!(self, Self::MeasuredMicros(_))
    }
}

/// Snapshot for one serially admitted operation window. `remaining` is the
/// free whole-invocation headroom. `operation_limits` adds only the explicitly
/// reused `already_admitted` baseline so kernels that count retained inputs or
/// previous rows internally receive a truthful ceiling without charging that
/// baseline twice.
pub(crate) struct FoundationBudgetTicket {
    id: u64,
    label: &'static str,
    remaining: FoundationPhaseReservation,
    operation_limits: FoundationPhaseReservation,
    already_admitted: FoundationPhaseReservation,
    remaining_worker_cpu_micros: u64,
}

impl FoundationBudgetTicket {
    pub(crate) fn label(&self) -> &'static str {
        self.label
    }

    pub(crate) fn remaining(&self) -> FoundationPhaseReservation {
        self.remaining
    }

    pub(crate) fn operation_limits(&self) -> FoundationPhaseReservation {
        self.operation_limits
    }

    pub(crate) fn already_admitted(&self) -> FoundationPhaseReservation {
        self.already_admitted
    }

    pub(crate) fn remaining_worker_cpu_micros(&self) -> u64 {
        self.remaining_worker_cpu_micros
    }
}

/// One mutable, serial ledger over the protected invocation's whole resource
/// ceilings. It does not allocate fixed per-phase shares. Callers open a
/// window in actual execution order and charge its measured delta, or the
/// admitted worst case when a phase fails and cannot report its partial cost.
pub(crate) struct FoundationRemainingBudget<'cancel> {
    caps: FoundationInvocationBudgets,
    deadline: Instant,
    cancelled: &'cancel std::sync::atomic::AtomicBool,
    charged: FoundationPhaseReservation,
    measured_charged: FoundationPhaseReservation,
    admitted_charged: FoundationPhaseReservation,
    worker_cpu_cap_micros: u64,
    next_ticket_id: u64,
    open_ticket_id: Option<u64>,
    poisoned: bool,
}

impl<'cancel> FoundationRemainingBudget<'cancel> {
    /// Retain the invocation's actual input and a conservative bound for this
    /// ledger plus the selected worker execution plan. The invocation's
    /// transient parse peak is checked against the cap but is not deducted as
    /// permanent retained state.
    pub(crate) fn from_invocation(invocation: &FoundationInvocation<'cancel>) -> Result<Self> {
        let caps = invocation.budgets;
        let state_cap = usize::try_from(caps.max_state_bytes)
            .map_err(|_| Error::Unsupported("foundation state limit range"))?;
        let issue_cap = usize::try_from(caps.max_issues)
            .map_err(|_| Error::Unsupported("foundation issue limit range"))?;
        let output_cap = usize::try_from(caps.max_output_bytes)
            .map_err(|_| Error::Unsupported("foundation output limit range"))?;
        if issue_cap == 0 || output_cap == 0 {
            return Err(Error::Unsupported(
                "foundation issue and output limits must be positive",
            ));
        }
        let worker_path_bytes = invocation.schema_worker.path.as_os_str().as_bytes().len();
        let plan_state_upper_bound = size_of::<Self>()
            .checked_add(size_of::<FoundationExecutionLimits>())
            .and_then(|bytes| bytes.checked_add(worker_path_bytes))
            .and_then(|bytes| bytes.checked_add(2 * size_of::<usize>()))
            .ok_or(Error::Unsupported(
                "foundation execution-plan state overflow",
            ))?;
        let startup_retained_upper_bound = invocation
            .cost
            .retained_input_bytes
            .checked_add(plan_state_upper_bound)
            .ok_or(Error::Unsupported("foundation startup state overflow"))?;
        let peak_state_with_plan = invocation
            .cost
            .peak_state_upper_bound_bytes
            .checked_add(plan_state_upper_bound)
            .ok_or(Error::Unsupported("foundation peak state overflow"))?;
        if peak_state_with_plan > state_cap || startup_retained_upper_bound > state_cap {
            return Err(Error::Unsupported(
                "foundation startup state exceeds invocation",
            ));
        }
        if invocation.deadline() <= Instant::now() {
            return Err(Error::Unsupported("foundation invocation deadline expired"));
        }
        if invocation.cancellation_flag().load(Ordering::Acquire) {
            return Err(Error::Unsupported("foundation invocation cancelled"));
        }

        let startup_charge = FoundationPhaseUse {
            state_bytes: FoundationCharge::admitted_upper_bound(startup_retained_upper_bound),
            ..FoundationPhaseUse::default()
        };
        let mut ledger = Self {
            caps,
            deadline: invocation.deadline(),
            cancelled: invocation.cancellation_flag(),
            charged: FoundationPhaseReservation::default(),
            measured_charged: FoundationPhaseReservation::default(),
            admitted_charged: FoundationPhaseReservation::default(),
            worker_cpu_cap_micros: caps
                .worker_cpu_seconds
                .checked_mul(1_000_000)
                .ok_or(Error::Unsupported("foundation CPU cap overflow"))?,
            next_ticket_id: 1,
            open_ticket_id: None,
            poisoned: false,
        };
        ledger.charge(startup_charge)?;
        Ok(ledger)
    }

    /// Current cumulative charge. CPU usage is exact in `worker_cpu_micros`;
    /// the seconds field is reserved for worker-facing ceilings, not totals.
    /// Continue only the same selected invocation and cooperative signal.
    pub(crate) fn verify_invocation(&self, invocation: &FoundationInvocation<'_>) -> Result<()> {
        self.check_live()?;
        if self.caps != invocation.budgets
            || self.deadline != invocation.deadline()
            || !std::ptr::eq(self.cancelled, invocation.cancellation_flag())
            || self.open_ticket_id.is_some()
        {
            return Err(Error::Denied(
                "foundation supplied ledger selection differs",
            ));
        }
        Ok(())
    }

    pub(crate) fn charged(&self) -> FoundationPhaseReservation {
        self.charged
    }

    pub(crate) fn caps(&self) -> FoundationInvocationBudgets {
        self.caps
    }

    pub(crate) fn measured_charged(&self) -> FoundationPhaseReservation {
        self.measured_charged
    }

    pub(crate) fn admitted_charged(&self) -> FoundationPhaseReservation {
        self.admitted_charged
    }

    pub(crate) fn remaining_worker_cpu_micros(&self) -> Result<u64> {
        self.worker_cpu_cap_micros
            .checked_sub(self.charged.worker_cpu_micros)
            .ok_or(Error::Unsupported("foundation CPU budget exhausted"))
    }

    /// Open the next serial window. `already_admitted` must be an actual
    /// subset of prior charges; it is exposed in `operation_limits` but does
    /// not reduce remaining headroom again.
    pub(crate) fn begin_window(
        &mut self,
        label: &'static str,
        already_admitted: FoundationPhaseReservation,
    ) -> Result<FoundationBudgetTicket> {
        self.check_live()?;
        if self.open_ticket_id.is_some() {
            return Err(Error::Unsupported("foundation budget window already open"));
        }
        if !reservation_fits(already_admitted, self.charged) {
            return Err(Error::Unsupported(
                "foundation window baseline was not previously charged",
            ));
        }
        let remaining = self.remaining()?;
        let remaining_worker_cpu_micros = self.remaining_worker_cpu_micros()?;
        let mut operation_limits = reservation_add(remaining, already_admitted)
            .ok_or(Error::Unsupported("foundation window limit overflow"))?;
        operation_limits.worker_cpu_micros = remaining_worker_cpu_micros;
        operation_limits.worker_cpu_seconds = seconds_ceiling(remaining_worker_cpu_micros)?;
        let id = self.next_ticket_id;
        self.next_ticket_id = self
            .next_ticket_id
            .checked_add(1)
            .ok_or(Error::Unsupported("foundation window sequence overflow"))?;
        self.open_ticket_id = Some(id);
        Ok(FoundationBudgetTicket {
            id,
            label,
            remaining,
            operation_limits,
            already_admitted,
            remaining_worker_cpu_micros,
        })
    }

    /// Charge actual incremental use exactly once. A measured overrun is
    /// retained as evidence and permanently poisons the ledger.
    pub(crate) fn complete_window(
        &mut self,
        ticket: FoundationBudgetTicket,
        incremental_use: FoundationPhaseUse,
    ) -> Result<()> {
        self.close_ticket(&ticket)?;
        let amounts = match incremental_use.amounts() {
            Ok(amounts) => amounts,
            Err(error) => {
                self.poisoned = true;
                return Err(error);
            }
        };
        let within_budget = reservation_fits(amounts, ticket.remaining);
        if self.charge(incremental_use).is_err() {
            self.poisoned = true;
            return Err(Error::Unsupported("foundation budget charge overflow"));
        }
        if Instant::now() >= self.deadline {
            self.poisoned = true;
            return Err(Error::Unsupported("foundation invocation deadline expired"));
        }
        if self.cancelled.load(Ordering::Acquire) {
            self.poisoned = true;
            return Err(Error::Unsupported("foundation invocation cancelled"));
        }
        if !within_budget {
            self.poisoned = true;
            return Err(Error::Unsupported(
                "foundation phase exceeded invocation budget",
            ));
        }
        Ok(())
    }

    /// Charge the phase's admitted worst case after failure, then poison the
    /// ledger so no later phase can reuse an uncertain remainder.
    pub(crate) fn fail_window(
        &mut self,
        ticket: FoundationBudgetTicket,
        admitted_worst_use: FoundationPhaseUse,
    ) -> Result<()> {
        self.fail_window_with_source_basis(
            ticket,
            admitted_worst_use,
            FoundationChargeBasis::AdmittedUpperBound,
        )
    }

    /// The shared IO owner observed this exact attempted-read term. Preserve
    /// its measured basis while all other unknown terms retain their worst case.
    pub(crate) fn fail_window_with_measured_source_reads(
        &mut self,
        ticket: FoundationBudgetTicket,
        mut admitted_worst_use: FoundationPhaseUse,
        actual_read_bytes: u64,
    ) -> Result<()> {
        admitted_worst_use.source_read_bytes = FoundationCharge::measured(actual_read_bytes);
        self.fail_window_with_source_basis(
            ticket,
            admitted_worst_use,
            FoundationChargeBasis::Measured,
        )
    }

    fn fail_window_with_source_basis(
        &mut self,
        ticket: FoundationBudgetTicket,
        admitted_worst_use: FoundationPhaseUse,
        source_read_basis: FoundationChargeBasis,
    ) -> Result<()> {
        self.close_ticket(&ticket)?;
        self.poisoned = true;
        let mut worst = admitted_worst_use;
        worst.source_read_bytes.basis = source_read_basis;
        worst.worker_wire_bytes.basis = FoundationChargeBasis::AdmittedUpperBound;
        worst.state_bytes.basis = FoundationChargeBasis::AdmittedUpperBound;
        worst.issue_count.basis = FoundationChargeBasis::AdmittedUpperBound;
        worst.output_bytes.basis = FoundationChargeBasis::AdmittedUpperBound;
        worst.tmpfs_bytes.basis = FoundationChargeBasis::AdmittedUpperBound;
        worst.tmpfs_inodes.basis = FoundationChargeBasis::AdmittedUpperBound;
        let amounts = worst.amounts()?;
        let within_budget = reservation_fits(amounts, ticket.remaining);
        self.charge(worst)?;
        if within_budget {
            Ok(())
        } else {
            Err(Error::Unsupported(
                "foundation failed phase worst case exceeds invocation",
            ))
        }
    }

    fn close_ticket(&mut self, ticket: &FoundationBudgetTicket) -> Result<()> {
        self.check_live_or_poisoned()?;
        if self.open_ticket_id != Some(ticket.id) {
            self.poisoned = true;
            self.open_ticket_id = None;
            return Err(Error::Unsupported("foundation budget ticket mismatch"));
        }
        self.open_ticket_id = None;
        Ok(())
    }

    fn check_live(&self) -> Result<()> {
        self.check_live_or_poisoned()?;
        if Instant::now() >= self.deadline {
            return Err(Error::Unsupported("foundation invocation deadline expired"));
        }
        if self.cancelled.load(Ordering::Acquire) {
            return Err(Error::Unsupported("foundation invocation cancelled"));
        }
        Ok(())
    }

    fn check_live_or_poisoned(&self) -> Result<()> {
        if self.poisoned {
            return Err(Error::Unsupported("foundation budget ledger poisoned"));
        }
        Ok(())
    }

    pub(crate) fn remaining(&self) -> Result<FoundationPhaseReservation> {
        self.check_live()?;
        let caps = reservation_caps(self.caps)?;
        let mut remaining = reservation_subtract(caps, self.charged)
            .ok_or(Error::Unsupported("foundation invocation budget exhausted"))?;
        remaining.worker_cpu_seconds = seconds_ceiling(remaining.worker_cpu_micros)?;
        Ok(remaining)
    }

    /// Record only an already-observed terminal source-read suffix. This does
    /// no work, opens no ticket and returns no remaining allowance. Callers
    /// may inspect the measured counter delta even when terminal liveness or
    /// capacity refusal follows the retained charge.
    pub(crate) fn record_terminal_measured_source_read_suffix(&mut self, bytes: u64) -> Result<()> {
        let total = self.charged.source_read_bytes.checked_add(bytes);
        let measured = self.measured_charged.source_read_bytes.checked_add(bytes);
        let (Some(total), Some(measured)) = (total, measured) else {
            self.poisoned = true;
            return Err(Error::Unsupported(
                "foundation terminal source-read accounting overflow",
            ));
        };
        self.charged.source_read_bytes = total;
        self.measured_charged.source_read_bytes = measured;
        if total > self.caps.max_total_read_bytes {
            self.poisoned = true;
            return Err(Error::Unsupported(
                "foundation terminal source reads exceed invocation",
            ));
        }
        if self.open_ticket_id.is_some() {
            self.poisoned = true;
            return Err(Error::Unsupported(
                "foundation terminal source reads with open window",
            ));
        }
        if let Err(error) = self.check_live() {
            self.poisoned = true;
            return Err(error);
        }
        Ok(())
    }

    /// Record a source-owned conservative read envelope after the shared IO
    /// ledger reports its terminal suffix. Preserve the admitted-upper-bound
    /// classification and the exact attempted prefix even if the invocation
    /// has already hit its capacity, deadline, cancellation, or poison fence.
    pub(crate) fn record_terminal_admitted_source_read_upper_bound_suffix(
        &mut self,
        bytes: u64,
    ) -> Result<()> {
        let total = self.charged.source_read_bytes.checked_add(bytes);
        let admitted = self.admitted_charged.source_read_bytes.checked_add(bytes);
        let (Some(total), Some(admitted)) = (total, admitted) else {
            self.poisoned = true;
            return Err(Error::Unsupported(
                "foundation terminal upper-bound source-read accounting overflow",
            ));
        };
        self.charged.source_read_bytes = total;
        self.admitted_charged.source_read_bytes = admitted;
        if total > self.caps.max_total_read_bytes {
            self.poisoned = true;
            return Err(Error::Unsupported(
                "foundation terminal upper-bound reads exceed invocation",
            ));
        }
        if self.open_ticket_id.is_some() {
            self.poisoned = true;
            return Err(Error::Unsupported(
                "foundation terminal upper-bound reads with open window",
            ));
        }
        if let Err(error) = self.check_live() {
            self.poisoned = true;
            return Err(error);
        }
        Ok(())
    }

    /// Retain both measured and admitted-upper-bound parts of one terminal
    /// source-read suffix atomically. This prevents an overflow in either
    /// classification from advancing only half of a caller's ledger witness.
    pub(crate) fn record_terminal_source_read_suffix(
        &mut self,
        measured_bytes: u64,
        admitted_upper_bound_bytes: u64,
    ) -> Result<()> {
        let suffix = measured_bytes.checked_add(admitted_upper_bound_bytes);
        let total = suffix.and_then(|bytes| self.charged.source_read_bytes.checked_add(bytes));
        let measured = self
            .measured_charged
            .source_read_bytes
            .checked_add(measured_bytes);
        let admitted = self
            .admitted_charged
            .source_read_bytes
            .checked_add(admitted_upper_bound_bytes);
        let (Some(total), Some(measured), Some(admitted)) = (total, measured, admitted) else {
            self.poisoned = true;
            return Err(Error::Unsupported(
                "foundation terminal classified source-read accounting overflow",
            ));
        };
        self.charged.source_read_bytes = total;
        self.measured_charged.source_read_bytes = measured;
        self.admitted_charged.source_read_bytes = admitted;
        if total > self.caps.max_total_read_bytes {
            self.poisoned = true;
            return Err(Error::Unsupported(
                "foundation terminal classified source reads exceed invocation",
            ));
        }
        if self.open_ticket_id.is_some() {
            self.poisoned = true;
            return Err(Error::Unsupported(
                "foundation terminal classified source reads with open window",
            ));
        }
        if let Err(error) = self.check_live() {
            self.poisoned = true;
            return Err(error);
        }
        Ok(())
    }

    fn charge(&mut self, usage: FoundationPhaseUse) -> Result<()> {
        let amounts = usage.amounts()?;
        self.charged = reservation_add(self.charged, amounts)
            .ok_or(Error::Unsupported("foundation aggregate usage overflow"))?;
        self.measured_charged = reservation_add(
            self.measured_charged,
            usage_amounts_by_basis(usage, FoundationChargeBasis::Measured)?,
        )
        .ok_or(Error::Unsupported("foundation measured usage overflow"))?;
        self.admitted_charged = reservation_add(
            self.admitted_charged,
            usage_amounts_by_basis(usage, FoundationChargeBasis::AdmittedUpperBound)?,
        )
        .ok_or(Error::Unsupported("foundation admitted usage overflow"))?;
        Ok(())
    }
}

/// Finite maintained phases in their actual default-path order. Records and
/// its conditional Biblio scan share one slice; artifact history/replay has a
/// distinct slice because it retains captured package evidence across rules.
/// The schema slice also covers loaded resource custody and evaluator request,
/// response, and retained-report state; the diagnostic slice covers the final
/// merged schema prose and owner issue stream.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct FoundationPhaseReservations {
    pub capture: FoundationPhaseReservation,
    pub selection: FoundationPhaseReservation,
    pub physical: FoundationPhaseReservation,
    pub payload: FoundationPhaseReservation,
    pub records_and_bibliography: FoundationPhaseReservation,
    pub artifact_history_replay: FoundationPhaseReservation,
    pub default_rules: FoundationPhaseReservation,
    pub catalog_and_persisted: FoundationPhaseReservation,
    pub schema: FoundationPhaseReservation,
    pub diagnostics: FoundationPhaseReservation,
    pub final_custody_and_output: FoundationPhaseReservation,
}

/// Existing builder slot used for one currently open ticket. This preserves
/// each adapter's checked limit constructor without creating phase budgets or
/// imposing an order on the caller's actual invocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FoundationWindowKind {
    Capture,
    Selection,
    Physical,
    Payload,
    RecordsAndBibliography,
    ArtifactHistoryReplay,
    DefaultRules,
    CatalogAndPersisted,
    Schema,
    Diagnostics,
    FinalCustodyAndOutput,
}

/// Caller-shaped batch maxima for the catalog's actual cut-diagnostics-v2
/// worker stream. These are caps for observed catalog work, not source facts.
#[derive(Clone, Copy)]
pub(crate) struct FoundationCatalogWorkerShape {
    pub batch: BatchBudget,
    pub max_chunks: u64,
    pub max_total_units: u64,
    pub max_total_raw_bytes: u64,
    pub max_total_wire_bytes: u64,
    pub max_distinct_selectors: usize,
    pub max_receipts: usize,
    pub max_receipt_bytes: usize,
}

/// Caller-shaped operation limits for a prepared source-cut schema worker.
/// The same `BatchStreamBudget` must cover every Records/Item/Biblio check on
/// that worker; callers must not reset it between families.
#[derive(Clone, Copy)]
pub(crate) struct FoundationCutWorkerShape {
    pub batch: BatchBudget,
    pub max_chunks: u64,
    pub max_total_units: u64,
    pub max_total_raw_bytes: u64,
    pub aggregate_wire_upper_bound_bytes: u64,
    pub max_distinct_selectors: usize,
}

/// Invocation-bound limits and the exact worker identity chosen by the
/// protected launch file. This carries ceilings and reservations only; it
/// contains no observed source facts or completion verdict.
pub(crate) struct FoundationExecutionLimits {
    pub worker: ExactWorkerIdentity,
    pub deadline: Instant,
    pub invocation_budgets: FoundationInvocationBudgets,
    pub reservations: FoundationPhaseReservations,
    selected_ticket_id: Option<u64>,
}

impl FoundationExecutionLimits {
    /// Validate the explicit phase reservations before cloning the selected
    /// worker path or beginning source work. Invocation and executable hashing
    /// have their own measured counters and are not hidden inside the source
    /// byte reservation.
    pub(crate) fn new(
        invocation: &FoundationInvocation<'_>,
        reservations: FoundationPhaseReservations,
    ) -> Result<Self> {
        let budgets = invocation.budgets;
        let phases = [
            reservations.capture,
            reservations.selection,
            reservations.physical,
            reservations.payload,
            reservations.records_and_bibliography,
            reservations.artifact_history_replay,
            reservations.default_rules,
            reservations.catalog_and_persisted,
            reservations.schema,
            reservations.diagnostics,
            reservations.final_custody_and_output,
        ];

        let source_read_bytes = checked_sum_u64(phases.map(|phase| phase.source_read_bytes))
            .ok_or(Error::Unsupported(
                "foundation source read reservation overflow",
            ))?;
        if source_read_bytes > budgets.max_total_read_bytes {
            return Err(Error::Unsupported(
                "foundation source read reservations exceed invocation",
            ));
        }

        let worker_wire_bytes = checked_sum_u64(phases.map(|phase| phase.worker_wire_bytes))
            .ok_or(Error::Unsupported(
                "foundation worker wire reservation overflow",
            ))?;
        if worker_wire_bytes > budgets.max_total_worker_wire_bytes {
            return Err(Error::Unsupported(
                "foundation worker wire reservations exceed invocation",
            ));
        }

        let state_bytes = checked_sum_usize(phases.map(|phase| phase.state_bytes))
            .ok_or(Error::Unsupported("foundation state reservation overflow"))?;
        let worker_path_bytes = invocation.schema_worker.path.as_os_str().as_bytes().len();
        let retained_plan_bytes = size_of::<Self>()
            .checked_add(worker_path_bytes)
            .and_then(|bytes| bytes.checked_add(2 * size_of::<usize>()))
            .ok_or(Error::Unsupported(
                "foundation execution-plan state overflow",
            ))?;
        let complete_state_reservation = invocation
            .cost
            .retained_input_bytes
            .checked_add(retained_plan_bytes)
            .and_then(|bytes| bytes.checked_add(state_bytes))
            .ok_or(Error::Unsupported(
                "foundation aggregate state reservation overflow",
            ))?;
        let selected_state_cap = usize::try_from(budgets.max_state_bytes)
            .map_err(|_| Error::Unsupported("foundation state limit range"))?;
        if complete_state_reservation > selected_state_cap {
            return Err(Error::Unsupported(
                "foundation state reservations exceed invocation",
            ));
        }

        let issue_count = checked_sum_usize(phases.map(|phase| phase.issue_count))
            .ok_or(Error::Unsupported("foundation issue reservation overflow"))?;
        let selected_issue_cap = usize::try_from(budgets.max_issues)
            .map_err(|_| Error::Unsupported("foundation issue limit range"))?;
        if issue_count > selected_issue_cap {
            return Err(Error::Unsupported(
                "foundation issue reservations exceed invocation",
            ));
        }

        let output_bytes = checked_sum_usize(phases.map(|phase| phase.output_bytes))
            .ok_or(Error::Unsupported("foundation output reservation overflow"))?;
        let selected_output_cap = usize::try_from(budgets.max_output_bytes)
            .map_err(|_| Error::Unsupported("foundation output limit range"))?;
        if output_bytes > selected_output_cap {
            return Err(Error::Unsupported(
                "foundation output reservations exceed invocation",
            ));
        }

        let worker_cpu_seconds = checked_sum_u64(phases.map(|phase| phase.worker_cpu_seconds))
            .ok_or(Error::Unsupported(
                "foundation worker CPU reservation overflow",
            ))?;
        if worker_cpu_seconds > budgets.worker_cpu_seconds || worker_cpu_seconds > 3_600 {
            return Err(Error::Unsupported(
                "foundation worker CPU reservations exceed invocation",
            ));
        }

        let worker_cpu_micros = checked_sum_u64(phases.map(|phase| phase.worker_cpu_micros))
            .ok_or(Error::Unsupported(
                "foundation CPU microsecond reservation overflow",
            ))?;
        let worker_cpu_cap_micros = budgets
            .worker_cpu_seconds
            .checked_mul(1_000_000)
            .ok_or(Error::Unsupported("foundation CPU cap overflow"))?;
        if worker_cpu_micros > worker_cpu_cap_micros {
            return Err(Error::Unsupported(
                "foundation worker CPU microsecond reservations exceed invocation",
            ));
        }

        let tmpfs_bytes = checked_sum_u64(phases.map(|phase| phase.tmpfs_bytes))
            .ok_or(Error::Unsupported("foundation tmpfs reservation overflow"))?;
        if tmpfs_bytes > budgets.tmpfs_quota_bytes {
            return Err(Error::Unsupported(
                "foundation tmpfs reservations exceed invocation",
            ));
        }

        let tmpfs_inodes = checked_sum_u64(phases.map(|phase| phase.tmpfs_inodes)).ok_or(
            Error::Unsupported("foundation tmpfs inode reservation overflow"),
        )?;
        if tmpfs_inodes > budgets.tmpfs_inode_limit {
            return Err(Error::Unsupported(
                "foundation tmpfs inode reservations exceed invocation",
            ));
        }

        if budgets.worker_address_space_bytes < 64 * 1024 * 1024
            || budgets.worker_address_space_bytes > 8 * 1024 * 1024 * 1024
        {
            return Err(Error::Unsupported(
                "foundation worker address-space limit unsupported",
            ));
        }
        if invocation.deadline() <= Instant::now() {
            return Err(Error::Unsupported("foundation invocation deadline expired"));
        }

        // The path clone is covered by `retained_plan_bytes` above. The
        // selected worker still verifies and pins this exact image at use.
        let worker = ExactWorkerIdentity {
            absolute_path: invocation.schema_worker.path.clone(),
            sha256: invocation.schema_worker.sha256,
        };

        Ok(Self {
            worker,
            deadline: invocation.deadline(),
            invocation_budgets: budgets,
            reservations,
            selected_ticket_id: None,
        })
    }

    /// Construct one worker-bearing limits value with no fixed phase shares.
    /// Later `select_window` calls only replace the builder ceiling for the
    /// currently open ledger ticket; they never clone the worker identity.
    pub(crate) fn new_whole(invocation: &FoundationInvocation<'_>) -> Result<Self> {
        let plan_bytes = size_of::<Self>()
            .checked_add(invocation.schema_worker.path.as_os_str().as_bytes().len())
            .and_then(|bytes| bytes.checked_add(2 * size_of::<usize>()))
            .ok_or(Error::Unsupported(
                "foundation execution-plan state overflow",
            ))?;
        let peak_with_plan = invocation
            .cost
            .peak_state_upper_bound_bytes
            .checked_add(plan_bytes)
            .ok_or(Error::Unsupported("foundation peak state overflow"))?;
        if peak_with_plan
            > as_usize(
                invocation.budgets.max_state_bytes,
                "foundation state limit range",
            )?
        {
            return Err(Error::Unsupported(
                "foundation execution plan exceeds peak state limit",
            ));
        }
        Self::new(invocation, FoundationPhaseReservations::default())
    }

    /// Select one existing phase slot from the current serial budget ticket.
    /// All other slots are cleared so legacy aggregate helpers see one window,
    /// never eleven copies of the same remaining whole-operation cap.
    pub(crate) fn select_window(
        &mut self,
        ticket: &FoundationBudgetTicket,
        target: FoundationWindowKind,
    ) -> Result<()> {
        if self.selected_ticket_id.is_some()
            || self.reservations != FoundationPhaseReservations::default()
        {
            return Err(Error::Unsupported(
                "foundation execution window must be closed before selection",
            ));
        }
        let limits = ticket.operation_limits();
        if !reservation_fits(limits, reservation_caps(self.invocation_budgets)?)
            || limits.worker_cpu_seconds > 3_600
        {
            return Err(Error::Unsupported(
                "foundation window limits exceed invocation",
            ));
        }
        let mut reservations = FoundationPhaseReservations::default();
        match target {
            FoundationWindowKind::Capture => reservations.capture = limits,
            FoundationWindowKind::Selection => reservations.selection = limits,
            FoundationWindowKind::Physical => reservations.physical = limits,
            FoundationWindowKind::Payload => reservations.payload = limits,
            FoundationWindowKind::RecordsAndBibliography => {
                reservations.records_and_bibliography = limits
            }
            FoundationWindowKind::ArtifactHistoryReplay => {
                reservations.artifact_history_replay = limits
            }
            FoundationWindowKind::DefaultRules => reservations.default_rules = limits,
            FoundationWindowKind::CatalogAndPersisted => {
                reservations.catalog_and_persisted = limits
            }
            FoundationWindowKind::Schema => reservations.schema = limits,
            FoundationWindowKind::Diagnostics => reservations.diagnostics = limits,
            FoundationWindowKind::FinalCustodyAndOutput => {
                reservations.final_custody_and_output = limits
            }
        }
        self.reservations = reservations;
        self.selected_ticket_id = Some(ticket.id);
        Ok(())
    }

    /// Clear the selected builder ceiling while the corresponding ledger
    /// ticket is still open. Call immediately before completing or failing it.
    pub(crate) fn clear_window(&mut self, ticket: &FoundationBudgetTicket) -> Result<()> {
        if self.selected_ticket_id != Some(ticket.id) {
            return Err(Error::Unsupported(
                "foundation execution window ticket mismatch",
            ));
        }
        self.reservations = FoundationPhaseReservations::default();
        self.selected_ticket_id = None;
        Ok(())
    }

    /// Build the common ItemLimits envelope from one named reservation. The
    /// per-member ceiling is intersected with both the protected invocation
    /// cap and this phase's aggregate raw-read cap.
    pub(crate) fn item_limits(
        &self,
        reservation: FoundationPhaseReservation,
    ) -> Result<ItemLimits> {
        let max_total_bytes = reservation.source_read_bytes.min(u64::MAX - 1);
        let max_member_bytes = self
            .invocation_budgets
            .max_member_bytes
            .min(max_total_bytes)
            .min((usize::MAX - 1) as u64);
        let limits = ItemLimits {
            max_member_bytes: as_usize(max_member_bytes, "foundation member limit range")?,
            max_total_bytes,
            max_state_bytes: reservation.state_bytes.min(usize::MAX - 1),
            max_issues: reservation.issue_count.min(usize::MAX - 1),
            deadline: self.deadline,
        };
        if limits.max_member_bytes == 0
            || limits.max_total_bytes == 0
            || limits.max_state_bytes == 0
            || limits.max_issues == 0
        {
            return Err(Error::Unsupported(
                "foundation phase Item reservation must be positive",
            ));
        }
        Ok(limits)
    }

    /// Authored selector capacities are tied to the selected current-member
    /// ceiling. Output-row capacity is bounded by the phase's own retained
    /// state using the selector's smallest charged B-tree row.
    pub(crate) fn selection_limits(&self) -> Result<FoundationSelectionLimits> {
        let reservation = self.reservations.selection;
        let max_selector_documents = as_usize(
            self.invocation_budgets
                .max_current_members
                .min((usize::MAX - 1) as u64),
            "foundation selector document limit range",
        )?;
        let min_row_bytes = 1usize
            .checked_add(size_of::<String>())
            .and_then(|bytes| bytes.checked_add(32 * size_of::<usize>()))
            .ok_or(Error::Unsupported("foundation selector row limit overflow"))?;
        let max_output_entries = reservation.state_bytes / min_row_bytes;
        let max_member_bytes = as_usize(
            self.invocation_budgets
                .max_member_bytes
                .min(reservation.source_read_bytes),
            "foundation selector member limit range",
        )?;
        if max_selector_documents == 0
            || max_output_entries == 0
            || max_member_bytes == 0
            || reservation.source_read_bytes == 0
            || reservation.state_bytes == 0
        {
            return Err(Error::Unsupported(
                "foundation selector reservation must be positive",
            ));
        }
        Ok(FoundationSelectionLimits {
            max_selector_documents,
            max_output_entries,
            max_private_prefixes: max_output_entries,
            max_payload_paths: max_output_entries,
            max_member_bytes: max_member_bytes as u64,
            max_total_read_bytes: reservation.source_read_bytes,
            max_state_bytes: reservation.state_bytes,
            deadline: self.deadline,
        })
    }

    /// Intersect nominal bootstrap count ceilings with this named live window.
    /// Required actual-count callers continue to use `physical_limits`.
    pub(crate) fn physical_limits_from_ceilings(
        &self,
        ceilings: PhysicalSourceLimits,
    ) -> Result<PhysicalSourceLimits> {
        let (path_capacity, inventory_capacity, observation_capacity) = path_count_capacities(
            self.reservations
                .physical
                .state_bytes
                .min(ceilings.max_state_bytes),
        )?;
        let paths = ceilings.max_paths.min(path_capacity);
        let observations = ceilings.max_path_observations.min(observation_capacity);
        self.physical_limits(
            paths,
            ceilings.max_private_prefixes.min(paths),
            ceilings.max_inventory_paths.min(inventory_capacity),
            observations,
            ceilings.max_git_path_queries.min(observations),
        )
    }

    /// Payload bootstrap counts are ceilings, not observed required rows.
    /// The existing actual-count validator still refuses any required excess.
    pub(crate) fn payload_limits_from_ceilings(
        &self,
        ceilings: PhysicalPayloadLimits,
    ) -> Result<PhysicalPayloadLimits> {
        let (file_capacity, _, observation_capacity) = path_count_capacities(
            self.reservations
                .payload
                .state_bytes
                .min(ceilings.max_state_bytes),
        )?;
        self.payload_limits(
            ceilings.max_files.min(file_capacity),
            ceilings.max_observations.min(observation_capacity),
        )
    }

    /// Convert explicit path/work counts selected by the caller into the
    /// physical provider's concrete bounds. Counts are checked against a
    /// state-derived ceiling; this adapter does not infer inventory contents.
    pub(crate) fn physical_limits(
        &self,
        max_paths: usize,
        max_private_prefixes: usize,
        max_inventory_paths: usize,
        max_path_observations: usize,
        max_git_path_queries: usize,
    ) -> Result<PhysicalSourceLimits> {
        let reservation = self.reservations.physical;
        let (max_paths_by_state, max_inventory_by_state, max_observations_by_state) =
            path_count_capacities(reservation.state_bytes)?;
        let max_file_bytes = as_usize(
            self.invocation_budgets
                .max_member_bytes
                .min(reservation.source_read_bytes),
            "foundation physical file limit range",
        )?;
        let max_total_bytes = as_usize(
            reservation.source_read_bytes,
            "foundation physical read limit range",
        )?;
        if max_paths == 0
            || max_paths > max_paths_by_state
            || max_private_prefixes > max_paths
            || max_inventory_paths > max_inventory_by_state
            || max_path_observations == 0
            || max_path_observations > max_observations_by_state
            || max_git_path_queries > max_path_observations
            || max_file_bytes == 0
            || max_total_bytes == 0
            || reservation.state_bytes
                < size_of::<tos_validation::source_foundation_discovery::SourcePhysicalFacts>()
        {
            return Err(Error::Unsupported(
                "foundation physical work exceeds its named reservation",
            ));
        }
        let git_output_bytes = reservation.state_bytes.min(65_536);
        let git_cleanup_grace =
            Duration::from_millis(self.invocation_budgets.operation_wall_ms.min(1_000));
        if git_output_bytes == 0 || git_cleanup_grace.is_zero() {
            return Err(Error::Unsupported(
                "foundation physical helper reservation must be positive",
            ));
        }
        Ok(PhysicalSourceLimits {
            max_paths,
            max_private_prefixes,
            max_inventory_paths,
            max_path_observations,
            max_git_path_queries,
            max_file_bytes,
            max_total_bytes,
            max_state_bytes: reservation.state_bytes,
            git_output_bytes,
            git_cleanup_grace,
        })
    }

    /// Build payload limits for an exact caller-selected path list. Per-path
    /// count is supplied from that list; the observation ceiling is explicit
    /// because owner queries may revisit selected paths.
    pub(crate) fn payload_limits(
        &self,
        max_files: usize,
        max_observations: usize,
    ) -> Result<PhysicalPayloadLimits> {
        let reservation = self.reservations.payload;
        let (max_files_by_state, _, max_observations_by_state) =
            path_count_capacities(reservation.state_bytes)?;
        let max_file_bytes = self
            .invocation_budgets
            .max_member_bytes
            .min(reservation.source_read_bytes);
        if max_files == 0
            || max_files > max_files_by_state
            || max_observations == 0
            || max_observations > max_observations_by_state
            || max_file_bytes == 0
            || reservation.source_read_bytes == 0
            || reservation.state_bytes == 0
        {
            return Err(Error::Unsupported(
                "foundation payload work exceeds its named reservation",
            ));
        }
        Ok(PhysicalPayloadLimits {
            max_files,
            max_observations,
            max_file_bytes,
            max_total_bytes: reservation.source_read_bytes,
            max_state_bytes: reservation.state_bytes,
        })
    }

    /// The record and Item kernels have independent read limits over the same
    /// authenticated cut. Their actual sub-reservations must fit under the
    /// named combined phase; the caller supplies those slices after deriving
    /// the exact selected record/item shape.
    pub(crate) fn records_limits(
        &self,
        operation: FoundationPhaseReservation,
        records: FoundationPhaseReservation,
        items: FoundationPhaseReservation,
        schema_request_state_bytes: usize,
    ) -> Result<SourceFoundationRecordsLimits> {
        let combined = self.reservations.records_and_bibliography;
        ensure_subreservation(operation, combined, "foundation Records operation")?;
        ensure_subreservation(records, operation, "foundation record kernel")?;
        ensure_subreservation(items, operation, "foundation Item kernel")?;
        ensure_sum_fits(
            [records.source_read_bytes, items.source_read_bytes],
            operation.source_read_bytes,
            "foundation record/item read reservations",
        )?;
        ensure_sum_fits(
            [records.worker_wire_bytes, items.worker_wire_bytes],
            operation.worker_wire_bytes,
            "foundation record/item worker wire reservations",
        )?;
        ensure_sum_fits_usize(
            [records.state_bytes, items.state_bytes],
            operation.state_bytes,
            "foundation record/item state reservations",
        )?;
        ensure_sum_fits_usize(
            [records.issue_count, items.issue_count],
            operation.issue_count,
            "foundation record/item issue reservations",
        )?;
        if schema_request_state_bytes == 0
            || schema_request_state_bytes == usize::MAX
            || schema_request_state_bytes > items.state_bytes
        {
            return Err(Error::Unsupported(
                "foundation schema request state exceeds Item reservation",
            ));
        }
        let operation = self.item_limits(operation)?;
        let records = self.item_limits(records)?;
        let items = self.item_limits(items)?;
        Ok(SourceFoundationRecordsLimits {
            operation,
            records,
            items,
            max_schema_resource_bytes: operation.max_total_bytes,
            max_schema_request_state_bytes: schema_request_state_bytes,
        })
    }

    /// Sequential owner kernels receive the original operation ceiling. The
    /// rolling VAL entry narrows Item limits only after actual Records usage;
    /// the same schema quota separately enforces cumulative worker costs.
    pub(crate) fn rolling_records_limits(
        &self,
        operation: FoundationPhaseReservation,
        schema_request_state_bytes: usize,
    ) -> Result<SourceFoundationRecordsLimits> {
        ensure_subreservation(
            operation,
            self.reservations.records_and_bibliography,
            "foundation rolling Records operation",
        )?;
        if schema_request_state_bytes == 0
            || schema_request_state_bytes == usize::MAX
            || schema_request_state_bytes > operation.state_bytes
        {
            return Err(Error::Unsupported(
                "foundation rolling schema request state",
            ));
        }
        let operation = self.item_limits(operation)?;
        Ok(SourceFoundationRecordsLimits {
            operation,
            records: operation,
            items: operation,
            max_schema_resource_bytes: operation.max_total_bytes,
            max_schema_request_state_bytes: schema_request_state_bytes,
        })
    }

    /// Build the one cumulative diagnostics-v2 envelope installed on a
    /// prepared source-cut worker before its first check. `aggregate_wire_`
    /// upper_bound_bytes is the caller's bound for the whole worker lifetime,
    /// including all family reuse; the underlying executor enforces it against
    /// actual request plus reserved response bytes without resetting per call.
    pub(crate) fn cut_worker_stream_budget(
        &self,
        reservation: FoundationPhaseReservation,
        mut shape: FoundationCutWorkerShape,
    ) -> Result<BatchStreamBudget> {
        let wire = shape.aggregate_wire_upper_bound_bytes;
        let wall = self.deadline.saturating_duration_since(Instant::now());
        let worker_cpu = reservation
            .worker_cpu_seconds
            .min(self.invocation_budgets.worker_cpu_seconds)
            .min(3_600);
        let address_space = self.invocation_budgets.worker_address_space_bytes;
        if wall.is_zero()
            || worker_cpu == 0
            || reservation.worker_wire_bytes == 0
            || wire == 0
            || wire == u64::MAX
            || wire > reservation.worker_wire_bytes
            || shape.max_chunks == 0
            || shape.max_chunks == u64::MAX
            || shape.max_total_units == 0
            || shape.max_total_units == u64::MAX
            || shape.max_total_raw_bytes == 0
            || shape.max_total_raw_bytes == u64::MAX
            || shape.max_total_raw_bytes > wire
            || shape.max_distinct_selectors == 0
            || shape.max_distinct_selectors == usize::MAX
            || shape.batch.cpu_seconds == 0
            || shape.batch.max_units == 0
            || shape.batch.max_total_raw_bytes == 0
            || shape.batch.max_total_raw_bytes == usize::MAX
            || shape.batch.total_execution_wall.is_zero()
            || shape.batch.startup_wall.is_zero()
            || shape.batch.per_unit_wall.is_zero()
        {
            return Err(Error::Unsupported(
                "foundation cut-worker wire shape exceeds its named reservation",
            ));
        }
        let wall = wall.min(Duration::from_secs(3_600));
        shape.batch.total_execution_wall = shape.batch.total_execution_wall.min(wall);
        shape.batch.startup_wall = shape
            .batch
            .startup_wall
            .min(shape.batch.total_execution_wall);
        shape.batch.per_unit_wall = shape
            .batch
            .per_unit_wall
            .min(shape.batch.total_execution_wall);
        shape.batch.cleanup_grace = shape
            .batch
            .cleanup_grace
            .min(shape.batch.total_execution_wall);
        shape.batch.cpu_seconds = shape.batch.cpu_seconds.min(worker_cpu);
        shape.batch.address_space_bytes = shape.batch.address_space_bytes.min(address_space);
        shape.batch.max_units = shape.batch.max_units.min(BatchBudget::MAX_UNITS);
        shape.batch.max_total_raw_bytes = shape
            .batch
            .max_total_raw_bytes
            .min(BatchBudget::MAX_RAW_BYTES)
            .min(as_usize(
                shape.max_total_raw_bytes,
                "foundation cut-worker raw limit range",
            )?);
        let budget = BatchStreamBudget {
            batch: shape.batch,
            max_chunks: shape.max_chunks,
            max_total_units: shape.max_total_units,
            max_total_raw_bytes: shape.max_total_raw_bytes,
            total_execution_wall: wall,
            operation_cpu_seconds: worker_cpu,
            operation_address_space_bytes: address_space,
            max_total_wire_bytes: wire,
            max_distinct_selectors: shape.max_distinct_selectors,
        };
        budget
            .validate()
            .map_err(|_| Error::Unsupported("foundation cut-worker stream limits unsupported"))?;
        Ok(budget)
    }

    /// Owner composition runs only after Records and conditional Biblio. Its
    /// owner-issued event-map cap remains an explicit sublimit of this phase.
    pub(crate) fn default_rules_limits(
        &self,
        operation: FoundationPhaseReservation,
        max_event_map_bytes: usize,
    ) -> Result<SourceFoundationDefaultRulesLimits> {
        ensure_subreservation(
            operation,
            self.reservations.default_rules,
            "foundation default-rule operation",
        )?;
        if max_event_map_bytes < 2
            || max_event_map_bytes > operation.state_bytes
            || max_event_map_bytes > operation.output_bytes
        {
            return Err(Error::Unsupported(
                "foundation source-event map exceeds default-rule reservation",
            ));
        }
        Ok(SourceFoundationDefaultRulesLimits {
            operation: self.item_limits(operation)?,
            max_event_map_bytes,
        })
    }

    /// Reader bounds remain a separate named subreservation because auxiliary
    /// physical bytes and the final auxiliary recheck are not authored-cut
    /// membership. The caller owns its exact selected path count.
    pub(crate) fn rule_read_limits(
        &self,
        reservation: FoundationPhaseReservation,
        max_auxiliary_paths: usize,
    ) -> Result<FoundationRuleReadLimits> {
        ensure_subreservation(
            reservation,
            self.reservations.default_rules,
            "foundation rule reader",
        )?;
        let max_member_bytes = as_usize(
            self.invocation_budgets
                .max_member_bytes
                .min(reservation.source_read_bytes),
            "foundation rule reader member limit range",
        )?;
        let minimum_aux_state = max_auxiliary_paths
            .checked_mul(size_of::<(String, Option<(tos_foundation::Digest256, u64)>)>() + 64)
            .ok_or(Error::Unsupported("foundation auxiliary state overflow"))?;
        if max_auxiliary_paths == 0
            || max_member_bytes == 0
            || reservation.source_read_bytes == 0
            || minimum_aux_state > reservation.state_bytes
        {
            return Err(Error::Unsupported(
                "foundation rule reader exceeds its named reservation",
            ));
        }
        Ok(FoundationRuleReadLimits {
            max_member_bytes,
            max_read_bytes: reservation.source_read_bytes,
            max_auxiliary_paths,
            max_auxiliary_state_bytes: reservation.state_bytes,
            deadline: self.deadline,
        })
    }

    /// Compose the final CLI output envelope across all already-reserved
    /// finding phases. Additional wrapper allocation stays in the final
    /// custody/output state slice.
    pub(crate) fn output_limits(&self) -> Result<SourceFoundationOutputLimits> {
        let phases = self.phases();
        let max_issues = checked_sum_usize(phases.map(|phase| phase.issue_count))
            .ok_or(Error::Unsupported("foundation output issue overflow"))?;
        let max_output_bytes = checked_sum_usize(phases.map(|phase| phase.output_bytes))
            .ok_or(Error::Unsupported("foundation output byte overflow"))?;
        let max_state_bytes = self.reservations.final_custody_and_output.state_bytes;
        if max_issues == 0 || max_output_bytes == 0 || max_state_bytes == 0 {
            return Err(Error::Unsupported(
                "foundation final output reservation must be positive",
            ));
        }
        Ok(SourceFoundationOutputLimits {
            max_issues,
            max_output_bytes,
            max_state_bytes,
        })
    }

    /// Stage VM execution and temporary spill use the actual selected SQLite
    /// budget and the catalog phase's reserved private-stage space. The page
    /// caps are caller-derived from the selected catalog input shape.
    pub(crate) fn stage_limits(
        &self,
        max_total_rows: u64,
        max_seek_rows: usize,
        max_seek_bytes: u64,
    ) -> Result<StageLimits> {
        let reservation = self.reservations.catalog_and_persisted;
        if max_total_rows == 0
            || max_seek_rows == 0
            || max_seek_bytes == 0
            || max_seek_bytes > reservation.tmpfs_bytes
            || reservation.tmpfs_bytes == 0
        {
            return Err(Error::Unsupported(
                "foundation catalog stage exceeds its named reservation",
            ));
        }
        let max_row_bytes = as_usize(
            self.invocation_budgets
                .max_member_bytes
                .min(reservation.source_read_bytes)
                .min((8 * 1024 * 1024) as u64),
            "foundation catalog row limit range",
        )?;
        let mut sqlite = tos_compiler::Limits::default();
        sqlite.max_rows = max_total_rows;
        sqlite.max_row_bytes = max_row_bytes;
        sqlite.max_output_bytes = reservation.tmpfs_bytes;
        sqlite.max_work_bytes = reservation.source_read_bytes.min(u64::MAX - 1);
        sqlite.sqlite_cache_kib =
            u32::try_from((self.invocation_budgets.working_ram_bytes / 1024).min(8 * 1024))
                .map_err(|_| Error::Unsupported("foundation SQLite cache limit range"))?;
        sqlite.max_sql_vm_steps = self.invocation_budgets.sqlite_max_vm_steps;
        if max_row_bytes == 0 || sqlite.sqlite_cache_kib == 0 {
            return Err(Error::Unsupported(
                "foundation catalog SQLite reservation must be positive",
            ));
        }
        Ok(StageLimits {
            sqlite,
            max_temp_bytes: reservation.tmpfs_bytes,
            max_seek_rows,
            max_seek_bytes,
        })
    }

    /// The source-catalog planner's member and retained-plan bounds are
    /// derived from the captured-cut cap and the catalog phase; the selected
    /// member ceiling is supplied from the caller's actual planner shape.
    pub(crate) fn catalog_input_limits(
        &self,
        max_selected_members: usize,
    ) -> Result<SourceCatalogInputLimits> {
        let reservation = self.reservations.catalog_and_persisted;
        let max_manifest_members = self
            .invocation_budgets
            .max_current_members
            .min(u64::MAX - 1);
        if max_selected_members == 0
            || max_selected_members > 4096
            || max_selected_members as u64 > max_manifest_members
            || reservation.source_read_bytes == 0
            || reservation.state_bytes == 0
        {
            return Err(Error::Unsupported(
                "foundation catalog input shape exceeds its reservation",
            ));
        }
        Ok(SourceCatalogInputLimits {
            max_manifest_members,
            max_selected_members,
            max_plan_bytes: reservation.state_bytes.min(64 * 1024 * 1024),
            max_work_bytes: reservation.source_read_bytes.min(u64::MAX - 1),
        })
    }

    /// Caller-observed catalog row/file caps are checked against the catalog
    /// phase and the protected per-member limit. This avoids inventing a row
    /// count from the number of manifest files.
    pub(crate) fn catalog_limits(
        &self,
        max_files: u64,
        max_rows: u64,
        max_file_bytes: usize,
        max_row_bytes: usize,
        max_contract_bytes: usize,
        max_output_row_bytes: usize,
    ) -> Result<SourceCatalogLimits> {
        let reservation = self.reservations.catalog_and_persisted;
        let max_phase_member = as_usize(
            self.invocation_budgets
                .max_member_bytes
                .min(reservation.source_read_bytes),
            "foundation catalog member limit range",
        )?;
        if max_files == 0
            || max_files == u64::MAX
            || max_files > self.invocation_budgets.max_current_members
            || max_rows == 0
            || max_rows == u64::MAX
            || max_rows > reservation.source_read_bytes
            || max_file_bytes == 0
            || max_file_bytes > max_phase_member
            || max_row_bytes == 0
            || max_row_bytes > max_file_bytes
            || max_contract_bytes == 0
            || max_contract_bytes > max_file_bytes
            || max_output_row_bytes == 0
            || u64::try_from(max_output_row_bytes)
                .map_or(true, |bytes| bytes > reservation.tmpfs_bytes)
        {
            return Err(Error::Unsupported(
                "foundation catalog work exceeds its named reservation",
            ));
        }
        let limits = SourceCatalogLimits {
            max_files,
            max_rows,
            max_file_bytes,
            max_row_bytes,
            max_contract_bytes,
            max_output_row_bytes,
        };
        limits
            .validate()
            .map_err(|_| Error::Unsupported("foundation catalog limits unsupported"))?;
        Ok(limits)
    }

    /// Biblio construction is conditional and follows the catalog producer.
    /// All row/cohort/output caps are exact caller-provided shape limits and
    /// must remain inside the same catalog-and-persisted reservation.
    pub(crate) fn bibliography_limits(
        &self,
        catalog: SourceCatalogLimits,
        max_claim_cohort_rows: usize,
        max_claim_cohort_bytes: usize,
        max_output_rows: u64,
        max_output_bytes: u64,
    ) -> Result<BibliographicLimits> {
        let reservation = self.reservations.catalog_and_persisted;
        if max_claim_cohort_rows == 0
            || max_claim_cohort_bytes == 0
            || max_claim_cohort_bytes > reservation.state_bytes
            || max_output_rows == 0
            || max_output_rows == u64::MAX
            || max_output_rows > max_output_bytes
            || max_output_bytes == 0
            || max_output_bytes > reservation.tmpfs_bytes
            || max_output_bytes > reservation.output_bytes as u64
        {
            return Err(Error::Unsupported(
                "foundation bibliography exceeds its named reservation",
            ));
        }
        let limits = BibliographicLimits {
            catalog,
            max_claim_cohort_rows,
            max_claim_cohort_bytes,
            max_output_rows,
            max_output_bytes,
            deadline: self.deadline,
        };
        limits
            .validate()
            .map_err(|_| Error::Unsupported("foundation bibliography limits unsupported"))?;
        Ok(limits)
    }

    /// Build the source-catalog worker's finite process and stream envelopes
    /// from one caller-selected batch shape and the catalog phase reservation.
    pub(crate) fn catalog_worker_limits(
        &self,
        mut shape: FoundationCatalogWorkerShape,
    ) -> Result<(
        ExecutorBudget,
        CutWorkerLimits,
        BatchStreamBudget,
        CutSchemaDiagnosticsLimits,
    )> {
        let reservation = self.reservations.catalog_and_persisted;
        let wall = self.deadline.saturating_duration_since(Instant::now());
        let operation_cpu = reservation
            .worker_cpu_seconds
            .min(self.invocation_budgets.worker_cpu_seconds)
            .min(3_600);
        let address_space = self.invocation_budgets.worker_address_space_bytes;
        let total_wire = shape
            .max_total_wire_bytes
            .min(reservation.worker_wire_bytes)
            .min(u64::MAX - 1);
        let total_raw = shape
            .max_total_raw_bytes
            .min(total_wire)
            .min(u64::MAX - 1);
        let total_units = shape.max_total_units.min(u64::MAX - 1);
        let chunks = shape.max_chunks.min(u64::MAX - 1);
        let max_receipts = u64::try_from(shape.max_receipts)
            .map_err(|_| Error::Unsupported("foundation catalog receipt limit range"))?;
        if wall.is_zero()
            || operation_cpu == 0
            || reservation.issue_count == 0
            || reservation.output_bytes == 0
            || reservation.state_bytes == 0
            || shape.max_receipts == 0
            || shape.max_receipt_bytes == 0
            || max_receipts > total_units
            || shape.max_receipt_bytes > reservation.state_bytes
            || chunks == 0
            || total_units == 0
            || total_raw == 0
            || shape.max_total_raw_bytes == u64::MAX
            || shape.max_total_wire_bytes == u64::MAX
            || total_wire == 0
            || reservation.worker_wire_bytes == 0
            || shape.max_distinct_selectors == 0
            || shape.max_distinct_selectors == usize::MAX
            || shape.batch.cpu_seconds == 0
            || shape.batch.max_units == 0
            || shape.batch.max_total_raw_bytes == 0
            || shape.batch.max_total_raw_bytes == usize::MAX
            || shape.batch.total_execution_wall.is_zero()
            || shape.batch.startup_wall.is_zero()
            || shape.batch.per_unit_wall.is_zero()
        {
            return Err(Error::Unsupported(
                "foundation catalog worker shape exceeds its named reservation",
            ));
        }
        let wall = wall.min(Duration::from_secs(3_600));
        shape.batch.total_execution_wall = shape.batch.total_execution_wall.min(wall);
        shape.batch.startup_wall = shape
            .batch
            .startup_wall
            .min(shape.batch.total_execution_wall);
        shape.batch.per_unit_wall = shape
            .batch
            .per_unit_wall
            .min(shape.batch.total_execution_wall);
        shape.batch.cleanup_grace = shape
            .batch
            .cleanup_grace
            .min(shape.batch.total_execution_wall);
        shape.batch.cpu_seconds = shape.batch.cpu_seconds.min(operation_cpu);
        shape.batch.address_space_bytes = shape.batch.address_space_bytes.min(address_space);
        shape.batch.max_units = shape.batch.max_units.min(BatchBudget::MAX_UNITS);
        shape.batch.max_total_raw_bytes = shape
            .batch
            .max_total_raw_bytes
            .min(BatchBudget::MAX_RAW_BYTES)
            .min(as_usize(
                total_raw,
                "foundation catalog raw byte limit range",
            )?);
        let worker = ExecutorBudget {
            execution_wall: wall,
            cleanup_grace: shape.batch.cleanup_grace,
            cpu_seconds: operation_cpu.min(ExecutorBudget::MAX_SCALAR_CPU_SECONDS),
            address_space_bytes: address_space,
        };
        let worker_limits = CutWorkerLimits {
            max_receipts: shape.max_receipts,
            max_receipt_bytes: shape.max_receipt_bytes,
        };
        let stream = BatchStreamBudget {
            batch: shape.batch,
            max_chunks: chunks,
            max_total_units: total_units,
            max_total_raw_bytes: total_raw,
            total_execution_wall: wall,
            operation_cpu_seconds: operation_cpu,
            operation_address_space_bytes: address_space,
            max_total_wire_bytes: total_wire,
            max_distinct_selectors: shape.max_distinct_selectors,
        };
        let receipt_issue_capacity = shape
            .max_receipts
            .checked_mul(tos_validation::executor::schema_diagnostics::MAX_ISSUES_PER_UNIT as usize)
            .ok_or(Error::Unsupported(
                "foundation catalog diagnostic issue capacity overflow",
            ))?;
        let report_byte_capacity = shape
            .max_receipts
            .checked_mul(
                tos_validation::executor::schema_diagnostics::MAX_REPORT_BYTES_PER_UNIT as usize,
            )
            .ok_or(Error::Unsupported(
                "foundation catalog diagnostic report capacity overflow",
            ))?;
        let max_total_report_bytes = reservation
            .output_bytes
            .min(report_byte_capacity)
            .min(total_wire.min(usize::MAX as u64) as usize);
        let max_total_issues = reservation.issue_count.min(receipt_issue_capacity);
        if max_total_report_bytes == 0 || max_total_issues == 0 {
            return Err(Error::Unsupported(
                "foundation catalog diagnostics exceed named output or wire reservation",
            ));
        }
        let diagnostics = CutSchemaDiagnosticsLimits::from_operation_ceilings(
            CutSchemaDiagnosticsLimits {
                max_total_issues,
                max_total_report_bytes,
                max_total_state_bytes: reservation.state_bytes,
            },
            worker_limits.max_receipts,
            stream,
        )
        .map_err(|_| Error::Unsupported("foundation catalog diagnostics operation envelope"))?;
        Ok((worker, worker_limits, stream, diagnostics))
    }

    /// ReadLimits for one exact captured-cut pass. Manifest entry count is an
    /// explicit caller cap; per-member and aggregate bytes are intersected
    /// with this named phase's reservation.
    pub(crate) fn read_limits(
        &self,
        reservation: FoundationPhaseReservation,
        max_manifest_entries: usize,
        max_selected_object_bytes: u64,
    ) -> Result<ReadLimits> {
        let max_manifest_bytes = as_usize(
            self.invocation_budgets
                .max_member_bytes
                .min(reservation.source_read_bytes),
            "foundation manifest byte limit range",
        )?;
        let selected_object_bytes = max_selected_object_bytes
            .min(reservation.source_read_bytes)
            .min(u64::MAX - 1);
        if max_manifest_bytes == 0
            || max_manifest_entries == 0
            || max_manifest_entries == usize::MAX
            || u64::try_from(max_manifest_entries).map_or(true, |entries| {
                entries > self.invocation_budgets.max_current_members
            })
            || selected_object_bytes == 0
            || reservation.source_read_bytes == 0
        {
            return Err(Error::Unsupported(
                "foundation source reader reservation must be positive",
            ));
        }
        let mut json = tos_foundation::JsonLimits::default();
        json.max_bytes = max_manifest_bytes;
        Ok(ReadLimits {
            max_manifest_bytes,
            max_manifest_entries,
            max_selected_object_bytes: selected_object_bytes,
            json,
        })
    }

    /// The capture store intentionally contains the exact current revision,
    /// while the source-store API still requires a finite non-max revision cap.
    pub(crate) fn current_cut_limits(
        &self,
        current_members: u64,
        reservation: FoundationPhaseReservation,
    ) -> Result<CutReadLimits> {
        let max_member_bytes = self
            .invocation_budgets
            .max_member_bytes
            .min(reservation.source_read_bytes)
            .min(u64::MAX - 1);
        let max_members = current_members
            .min(self.invocation_budgets.max_current_members)
            .min(u64::MAX - 1);
        if current_members > self.invocation_budgets.max_current_members
            || max_member_bytes == 0
            || reservation.source_read_bytes == 0
        {
            return Err(Error::Unsupported(
                "foundation captured-cut reservation is invalid",
            ));
        }
        Ok(CutReadLimits {
            max_revisions: 1,
            max_members: max_members.max(1),
            max_total_bytes: reservation.source_read_bytes.min(u64::MAX - 1),
            max_member_bytes,
        })
    }

    /// Construct one schema evaluator envelope from caller-observed cut and
    /// check shapes. Repeated calls must use distinct phase slices; each
    /// returned worker budget is capped by that slice rather than the entire
    /// invocation allowance.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn schema_limits(
        &self,
        resources: FoundationPhaseReservation,
        work: FoundationPhaseReservation,
        max_schema_resources: usize,
        max_schema_resource_bytes: usize,
        max_total_schema_bytes: usize,
        max_checks: usize,
        max_chunks: usize,
        max_instance_bytes: usize,
        max_total_instance_bytes: usize,
        max_total_issues: usize,
        max_total_report_bytes: usize,
        mut batch: BatchBudget,
    ) -> Result<SourceFoundationSchemaLimits> {
        ensure_subreservation(
            work,
            self.reservations.schema,
            "foundation schema worker slice",
        )?;
        let max_work_state = work.state_bytes;
        let max_resource_read = as_usize(
            resources.source_read_bytes,
            "foundation schema source limit range",
        )?;
        let max_member = as_usize(
            self.invocation_budgets.max_member_bytes,
            "foundation schema member limit range",
        )?;
        let batch_wall = self.deadline.saturating_duration_since(Instant::now());
        let worker_cpu = work
            .worker_cpu_seconds
            .min(self.invocation_budgets.worker_cpu_seconds)
            .min(3_600);
        let per_batch_raw = max_total_instance_bytes.min(BatchBudget::MAX_RAW_BYTES);
        let batch_units = batch.max_units.min(BatchBudget::MAX_UNITS);
        if worker_cpu == 0
            || batch_wall.is_zero()
            || batch_units == 0
            || max_schema_resources == 0
            || max_schema_resource_bytes == 0
            || max_schema_resource_bytes > max_member
            || max_schema_resource_bytes > max_resource_read
            || max_total_schema_bytes == 0
            || max_total_schema_bytes > max_resource_read
            || max_checks == 0
            || max_chunks == 0
            || max_instance_bytes == 0
            || max_instance_bytes > max_member
            || max_total_instance_bytes == 0
            || max_total_instance_bytes > max_work_state
            || work.worker_wire_bytes == 0
            || max_total_issues == 0
            || max_total_issues > work.issue_count
            || max_total_report_bytes == 0
            || max_total_report_bytes > work.output_bytes
            || per_batch_raw == 0
        {
            return Err(Error::Unsupported(
                "foundation schema work exceeds its named reservation",
            ));
        }
        batch.total_execution_wall = batch_wall.min(Duration::from_secs(3600));
        batch.startup_wall = batch.startup_wall.min(batch.total_execution_wall);
        batch.per_unit_wall = batch.per_unit_wall.min(batch.total_execution_wall);
        batch.cleanup_grace = batch.cleanup_grace.min(batch.total_execution_wall);
        batch.cpu_seconds = batch.cpu_seconds.min(worker_cpu);
        batch.address_space_bytes = batch
            .address_space_bytes
            .min(self.invocation_budgets.worker_address_space_bytes);
        batch.max_units = batch_units;
        batch.max_total_raw_bytes = batch.max_total_raw_bytes.min(per_batch_raw);
        let total_cpu_capacity = u64::try_from(max_chunks)
            .ok()
            .and_then(|chunks| chunks.checked_mul(batch.cpu_seconds))
            .ok_or(Error::Unsupported(
                "foundation schema worker CPU capacity overflow",
            ))?;
        let max_total_cpu_seconds = worker_cpu.min(total_cpu_capacity);
        let limits = SourceFoundationSchemaLimits {
            max_schema_resources,
            max_schema_resource_bytes,
            max_total_schema_bytes,
            max_checks,
            max_chunks,
            max_total_cpu_seconds,
            max_instance_bytes,
            max_total_instance_bytes,
            max_total_issues,
            max_total_report_bytes,
            max_total_worker_wire_bytes: work.worker_wire_bytes,
            batch,
        };
        if !limits.validate() {
            return Err(Error::Unsupported(
                "foundation schema limits do not fit worker profile",
            ));
        }
        Ok(limits)
    }

    /// Use the already-loaded exact cut schema set as the source of resource
    /// count/byte ceilings when constructing a later diagnostic slice. This
    /// preserves the fact that resource bytes were captured and loaded once.
    pub(crate) fn schema_set_limits(
        &self,
        work: FoundationPhaseReservation,
        schema_set: &SourceFoundationSchemaSet,
        max_checks: usize,
        max_chunks: usize,
        max_instance_bytes: usize,
        max_total_instance_bytes: usize,
        max_total_issues: usize,
        max_total_report_bytes: usize,
        batch: BatchBudget,
    ) -> Result<SourceFoundationSchemaLimits> {
        self.schema_limits(
            self.reservations.schema,
            work,
            schema_set.schema_resource_count(),
            schema_set.schema_bytes().min(as_usize(
                self.invocation_budgets.max_member_bytes,
                "foundation schema member limit range",
            )?),
            schema_set.schema_bytes(),
            max_checks,
            max_chunks,
            max_instance_bytes,
            max_total_instance_bytes,
            max_total_issues,
            max_total_report_bytes,
            batch,
        )
    }

    pub(crate) fn diagnostic_limits(&self) -> Result<SourceFoundationRuleDiagnosticsLimits> {
        let reservation = self.reservations.diagnostics;
        if reservation.issue_count == 0
            || reservation.output_bytes == 0
            || reservation.state_bytes == 0
        {
            return Err(Error::Unsupported(
                "foundation diagnostic reservation must be positive",
            ));
        }
        Ok(SourceFoundationRuleDiagnosticsLimits {
            max_issues: reservation.issue_count,
            max_output_bytes: reservation.output_bytes,
            max_state_bytes: reservation.state_bytes,
        })
    }

    /// The final authenticated authored-member EOF pass is reserved
    /// separately from the initial capture and catalog-owner rechecks.
    pub(crate) fn final_authored_member_read_bytes(&self) -> Result<usize> {
        let bytes = self
            .reservations
            .final_custody_and_output
            .source_read_bytes
            .min((usize::MAX - 1) as u64);
        let bytes = as_usize(bytes, "foundation final source read limit range")?;
        if bytes == 0 {
            return Err(Error::Unsupported(
                "foundation final source custody reservation must be positive",
            ));
        }
        Ok(bytes)
    }

    fn phases(&self) -> [FoundationPhaseReservation; 11] {
        let reservations = self.reservations;
        [
            reservations.capture,
            reservations.selection,
            reservations.physical,
            reservations.payload,
            reservations.records_and_bibliography,
            reservations.artifact_history_replay,
            reservations.default_rules,
            reservations.catalog_and_persisted,
            reservations.schema,
            reservations.diagnostics,
            reservations.final_custody_and_output,
        ]
    }
}

// One capacity formula owns both strict required-count and nominal-ceiling
// adapters. These are the existing modeled retained-state allowances, not RSS.
fn path_count_capacities(state_bytes: usize) -> Result<(usize, usize, usize)> {
    let minimum_path_state = 1usize
        .checked_add(size_of::<String>())
        .and_then(|bytes| bytes.checked_add(32 * size_of::<usize>()))
        .ok_or(Error::Unsupported(
            "foundation physical path limit overflow",
        ))?;
    let inventory_path_state = minimum_path_state
        .checked_mul(3)
        .ok_or(Error::Unsupported("foundation inventory limit overflow"))?;
    Ok((
        state_bytes / minimum_path_state,
        state_bytes / inventory_path_state,
        state_bytes / size_of::<usize>(),
    ))
}

fn as_usize(value: u64, label: &'static str) -> Result<usize> {
    usize::try_from(value).map_err(|_| Error::Unsupported(label))
}

fn seconds_ceiling(micros: u64) -> Result<u64> {
    let seconds = micros / 1_000_000;
    if micros % 1_000_000 == 0 {
        Ok(seconds)
    } else {
        seconds.checked_add(1).ok_or(Error::Unsupported(
            "foundation CPU seconds ceiling overflow",
        ))
    }
}

fn usage_amounts_by_basis(
    usage: FoundationPhaseUse,
    basis: FoundationChargeBasis,
) -> Result<FoundationPhaseReservation> {
    let amount_u64 = |charge: FoundationCharge<u64>| {
        if charge.basis == basis {
            charge.amount
        } else {
            0
        }
    };
    let amount_usize = |charge: FoundationCharge<usize>| {
        if charge.basis == basis {
            charge.amount
        } else {
            0
        }
    };
    let worker_cpu_micros =
        if (basis == FoundationChargeBasis::Measured) == usage.worker_cpu.is_measured() {
            usage.worker_cpu.micros()?
        } else {
            0
        };
    Ok(FoundationPhaseReservation {
        source_read_bytes: amount_u64(usage.source_read_bytes),
        worker_wire_bytes: amount_u64(usage.worker_wire_bytes),
        state_bytes: amount_usize(usage.state_bytes),
        issue_count: amount_usize(usage.issue_count),
        output_bytes: amount_usize(usage.output_bytes),
        worker_cpu_seconds: 0,
        worker_cpu_micros,
        tmpfs_bytes: amount_u64(usage.tmpfs_bytes),
        tmpfs_inodes: amount_u64(usage.tmpfs_inodes),
    })
}

fn reservation_caps(budgets: FoundationInvocationBudgets) -> Result<FoundationPhaseReservation> {
    Ok(FoundationPhaseReservation {
        source_read_bytes: budgets.max_total_read_bytes,
        worker_wire_bytes: budgets.max_total_worker_wire_bytes,
        state_bytes: usize::try_from(budgets.max_state_bytes)
            .map_err(|_| Error::Unsupported("foundation state limit range"))?,
        issue_count: usize::try_from(budgets.max_issues)
            .map_err(|_| Error::Unsupported("foundation issue limit range"))?,
        output_bytes: usize::try_from(budgets.max_output_bytes)
            .map_err(|_| Error::Unsupported("foundation output limit range"))?,
        worker_cpu_seconds: budgets.worker_cpu_seconds,
        worker_cpu_micros: budgets
            .worker_cpu_seconds
            .checked_mul(1_000_000)
            .ok_or(Error::Unsupported("foundation CPU cap overflow"))?,
        tmpfs_bytes: budgets.tmpfs_quota_bytes,
        tmpfs_inodes: budgets.tmpfs_inode_limit,
    })
}

fn reservation_add(
    left: FoundationPhaseReservation,
    right: FoundationPhaseReservation,
) -> Option<FoundationPhaseReservation> {
    Some(FoundationPhaseReservation {
        source_read_bytes: left
            .source_read_bytes
            .checked_add(right.source_read_bytes)?,
        worker_wire_bytes: left
            .worker_wire_bytes
            .checked_add(right.worker_wire_bytes)?,
        state_bytes: left.state_bytes.checked_add(right.state_bytes)?,
        issue_count: left.issue_count.checked_add(right.issue_count)?,
        output_bytes: left.output_bytes.checked_add(right.output_bytes)?,
        worker_cpu_seconds: left
            .worker_cpu_seconds
            .checked_add(right.worker_cpu_seconds)?,
        worker_cpu_micros: left
            .worker_cpu_micros
            .checked_add(right.worker_cpu_micros)?,
        tmpfs_bytes: left.tmpfs_bytes.checked_add(right.tmpfs_bytes)?,
        tmpfs_inodes: left.tmpfs_inodes.checked_add(right.tmpfs_inodes)?,
    })
}

fn reservation_subtract(
    caps: FoundationPhaseReservation,
    charged: FoundationPhaseReservation,
) -> Option<FoundationPhaseReservation> {
    Some(FoundationPhaseReservation {
        source_read_bytes: caps
            .source_read_bytes
            .checked_sub(charged.source_read_bytes)?,
        worker_wire_bytes: caps
            .worker_wire_bytes
            .checked_sub(charged.worker_wire_bytes)?,
        state_bytes: caps.state_bytes.checked_sub(charged.state_bytes)?,
        issue_count: caps.issue_count.checked_sub(charged.issue_count)?,
        output_bytes: caps.output_bytes.checked_sub(charged.output_bytes)?,
        worker_cpu_seconds: caps
            .worker_cpu_seconds
            .checked_sub(charged.worker_cpu_seconds)?,
        worker_cpu_micros: caps
            .worker_cpu_micros
            .checked_sub(charged.worker_cpu_micros)?,
        tmpfs_bytes: caps.tmpfs_bytes.checked_sub(charged.tmpfs_bytes)?,
        tmpfs_inodes: caps.tmpfs_inodes.checked_sub(charged.tmpfs_inodes)?,
    })
}

fn reservation_fits(
    used: FoundationPhaseReservation,
    available: FoundationPhaseReservation,
) -> bool {
    used.source_read_bytes <= available.source_read_bytes
        && used.worker_wire_bytes <= available.worker_wire_bytes
        && used.state_bytes <= available.state_bytes
        && used.issue_count <= available.issue_count
        && used.output_bytes <= available.output_bytes
        && used.worker_cpu_seconds <= available.worker_cpu_seconds
        && used.worker_cpu_micros <= available.worker_cpu_micros
        && used.tmpfs_bytes <= available.tmpfs_bytes
        && used.tmpfs_inodes <= available.tmpfs_inodes
}

fn ensure_subreservation(
    selected: FoundationPhaseReservation,
    parent: FoundationPhaseReservation,
    label: &'static str,
) -> Result<()> {
    if selected.source_read_bytes > parent.source_read_bytes
        || selected.worker_wire_bytes > parent.worker_wire_bytes
        || selected.state_bytes > parent.state_bytes
        || selected.issue_count > parent.issue_count
        || selected.output_bytes > parent.output_bytes
        || selected.worker_cpu_seconds > parent.worker_cpu_seconds
        || selected.worker_cpu_micros > parent.worker_cpu_micros
        || selected.tmpfs_bytes > parent.tmpfs_bytes
        || selected.tmpfs_inodes > parent.tmpfs_inodes
    {
        return Err(Error::Unsupported(label));
    }
    Ok(())
}

fn ensure_sum_fits<const N: usize>(
    values: [u64; N],
    limit: u64,
    label: &'static str,
) -> Result<()> {
    let total = values
        .into_iter()
        .try_fold(0u64, u64::checked_add)
        .ok_or(Error::Unsupported(label))?;
    if total > limit {
        return Err(Error::Unsupported(label));
    }
    Ok(())
}

fn ensure_sum_fits_usize<const N: usize>(
    values: [usize; N],
    limit: usize,
    label: &'static str,
) -> Result<()> {
    let total = values
        .into_iter()
        .try_fold(0usize, usize::checked_add)
        .ok_or(Error::Unsupported(label))?;
    if total > limit {
        return Err(Error::Unsupported(label));
    }
    Ok(())
}

fn checked_sum_u64(values: [u64; 11]) -> Option<u64> {
    values.into_iter().try_fold(0u64, u64::checked_add)
}

fn checked_sum_usize(values: [usize; 11]) -> Option<usize> {
    values.into_iter().try_fold(0usize, usize::checked_add)
}
