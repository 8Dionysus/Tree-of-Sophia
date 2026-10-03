//! Same prepared candidate schema worker over candidate-fenced native Item input.
//! Resource bytes are checked by the native receiver through that exact input;
//! this adapter creates no candidate revision or admission/completion witness.
use crate::source_admission_candidate_records::CandidateRecordsInput;
use crate::source_admission_spooled_candidate::CandidateFence;
use std::{cell::Cell, mem::size_of, sync::atomic::AtomicBool, time::Instant};
use tos_foundation::Digest256;
use tos_validation::{
    FormatProfile,
    item_rules::ItemRefusal,
    record_biblio_cut::SourceCutInputWithIdentity,
    source_cut::{
        CandidateCutSchemaDiagnostic, CandidateCutWorkerSchemaExecutor,
        CutPreparedSchemaExecutionBinding, CutPreparedSchemaProtocol,
        CutSchemaDiagnosticsCumulativeCost, CutSchemaExecutor, CutSchemaInputCost,
    },
    source_foundation_records::{
        SourceFoundationCandidateSchemaBinding, SourceFoundationCandidateSchemaResource,
    },
};

type CandidateDiagnostic = CandidateCutSchemaDiagnostic<CandidateFence>;

/// Invoke the maintained Records+Item receiver over the actual candidate input
/// and prepared candidate schema worker. This district report is not whole native
/// admission or a CMD creation-inventory completion. The caller owns the real
/// store, payload resolver, schema protocol envelope and their retained state.
#[allow(clippy::too_many_arguments)]
pub(crate) fn inspect_candidate_records_stored<'store>(
    input: &CandidateRecordsInput<'_, '_>,
    worker: &mut CandidateCutWorkerSchemaExecutor<CandidateFence>,
    limits: tos_validation::source_foundation_records::SourceFoundationRecordsLimits,
    require_local_payloads: bool,
    cancelled: &AtomicBool,
    record_executor: &mut tos_validation::record_biblio_cut::BiblioRecordExecutor,
    physical_facts: &tos_validation::source_foundation_discovery::SourcePhysicalFacts,
    payloads: &mut impl tos_validation::source_cut::CutPayloadReader,
    fact_budget: tos_validation::record_biblio_cut::SourceCutRecordFactBudget,
    page_budget: tos_validation::source_foundation_records::SourceFoundationRecordsPageBudget,
    store: &'store mut dyn tos_validation::source_foundation_records::SourceFoundationRecordsStore,
    retained_state_bytes: usize,
    max_operation_state_bytes: usize,
) -> Result<
    tos_validation::source_foundation_records::SourceFoundationRecordsStreamedReport<
        'store,
        CandidateFence,
    >,
    ItemRefusal,
> {
    let result = (move || {
        // Consume the held store reference once; the returned report borrows it.
        let store = store;
        let deadline = limits.operation.deadline;
        input.verify_invocation(deadline, cancelled)?;
        let mut binding = CandidateSchemaBinding::new(
            input,
            worker,
            deadline,
            cancelled,
            retained_state_bytes,
            max_operation_state_bytes,
        )?;
        let callback_state = binding
            .baseline_state_bytes
            .checked_add(limits.operation.max_state_bytes)
            .filter(|state| *state <= max_operation_state_bytes)
            .ok_or(ItemRefusal::Budget)?;
        input.require_callback_state(callback_state, max_operation_state_bytes)?;
        let report = tos_validation::source_foundation_records::inspect_source_foundation_records_from_input_stored(
            input,
            &mut binding,
            limits,
            require_local_payloads,
            cancelled,
            record_executor,
            physical_facts,
            payloads,
            fact_budget,
            page_budget,
            store,
        )?;
        input.verify_invocation(deadline, cancelled)?;
        Ok(report)
    })();
    if result.is_err() {
        input.abandon();
    }
    result
}

pub(crate) struct CandidateSchemaBinding<'a, 'input, 'host> {
    input: &'a CandidateRecordsInput<'input, 'host>,
    worker: &'a mut CandidateCutWorkerSchemaExecutor<CandidateFence>,
    prepared: CutPreparedSchemaExecutionBinding,
    selection_digest: Digest256,
    deadline: Instant,
    cancelled: &'a AtomicBool,
    baseline_state_bytes: usize,
    max_state_bytes: usize,
    failed: Cell<bool>,
}
fn refused() -> ItemRefusal {
    ItemRefusal::Source("candidate schema binding refused".into())
}
impl<'a, 'input, 'host> CandidateSchemaBinding<'a, 'input, 'host> {
    /// `retained_state_bytes` includes the caller's actual schema/worker/input
    /// state. The selector's retained resource metadata is added separately.
    /// This constructor performs no parsing, cloning or worker preparation.
    pub(crate) fn new(
        input: &'a CandidateRecordsInput<'input, 'host>,
        worker: &'a mut CandidateCutWorkerSchemaExecutor<CandidateFence>,
        deadline: Instant,
        cancelled: &'a AtomicBool,
        retained_state_bytes: usize,
        max_state_bytes: usize,
    ) -> Result<Self, ItemRefusal> {
        let result = Self::new_inner(
            input,
            worker,
            deadline,
            cancelled,
            retained_state_bytes,
            max_state_bytes,
        );
        if result.is_err() {
            input.abandon();
        }
        result
    }
    fn new_inner(
        input: &'a CandidateRecordsInput<'input, 'host>,
        worker: &'a mut CandidateCutWorkerSchemaExecutor<CandidateFence>,
        deadline: Instant,
        cancelled: &'a AtomicBool,
        retained_state_bytes: usize,
        max_state_bytes: usize,
    ) -> Result<Self, ItemRefusal> {
        input.verify_invocation(deadline, cancelled)?;
        let prepared = worker.prepared_execution_binding();
        if worker.input_identity() != input.input_identity()
            || prepared.schema_profile != worker.profile()
            || prepared.schema_set_sha256 != worker.schema_set_digest()
            || retained_state_bytes < worker.schema_bytes()
        {
            return Err(refused());
        }
        let baseline_state_bytes = retained_state_bytes
            .checked_add(
                worker
                    .source_resource_metadata_state_bytes()
                    .ok_or(ItemRefusal::Budget)?,
            )
            .and_then(|n| n.checked_add(size_of::<Self>() + 16384))
            .filter(|n| *n <= max_state_bytes)
            .ok_or(ItemRefusal::Budget)?;
        let mut previous: Option<&str> = None;
        for resource in worker.source_resources() {
            input.verify_invocation(deadline, cancelled)?;
            if resource.path.is_empty() || previous.is_some_and(|p| p >= resource.path) {
                return Err(refused());
            }
            previous = Some(resource.path);
        }
        input.verify_invocation(deadline, cancelled)?;
        let selection_digest = worker.contract_selection_digest();
        Ok(Self {
            input,
            worker,
            prepared,
            selection_digest,
            deadline,
            cancelled,
            baseline_state_bytes,
            max_state_bytes,
            failed: Cell::new(false),
        })
    }
    fn guard(&self, deadline: Instant, cancelled: &AtomicBool) -> Result<(), ItemRefusal> {
        if self.failed.get()
            || deadline != self.deadline
            || !std::ptr::eq(cancelled, self.cancelled)
            || self.worker.prepared_execution_binding() != self.prepared
            || self.worker.input_identity() != self.input.input_identity()
            || self.worker.schema_set_digest() != self.prepared.schema_set_sha256
            || self.worker.contract_selection_digest() != self.selection_digest
        {
            self.failed.set(true);
            self.input.abandon();
            return Err(refused());
        }
        if let Err(error) = self.input.verify_invocation(deadline, cancelled) {
            self.failed.set(true);
            self.input.abandon();
            return Err(error);
        }
        Ok(())
    }
    fn row(&self, path: &str, raw: &[u8], contract: &str) -> Result<(), ItemRefusal> {
        self.guard(self.deadline, self.cancelled)?;
        if self.worker.contract_digest(contract).is_none()
            || path
                .len()
                .checked_add(contract.len())
                .and_then(|n| n.checked_mul(16))
                .and_then(|n| n.checked_add(raw.len().checked_mul(4)?))
                .and_then(|n| n.checked_add(self.baseline_state_bytes))
                .is_none_or(|n| n > self.max_state_bytes)
        {
            self.failed.set(true);
            self.input.abandon();
            return Err(ItemRefusal::Budget);
        }
        Ok(())
    }
    fn finish_result<T>(&self, result: Result<T, ItemRefusal>) -> Result<T, ItemRefusal> {
        match result {
            Err(error) => {
                self.failed.set(true);
                self.input.abandon();
                Err(error)
            }
            Ok(value) => {
                self.guard(self.deadline, self.cancelled)?;
                Ok(value)
            }
        }
    }
    fn diagnostics_state(&self, path: &str, raw: &[u8]) -> Result<(), ItemRefusal> {
        self.guard(self.deadline, self.cancelled)?;
        let state = self
            .worker
            .diagnostics_v2_controller_state_upper_bound(raw.len(), path.len());
        let state = self.finish_result(state)?;
        if self
            .baseline_state_bytes
            .checked_add(state)
            .is_none_or(|n| n > self.max_state_bytes)
        {
            self.failed.set(true);
            self.input.abandon();
            return Err(ItemRefusal::Budget);
        }
        self.guard(self.deadline, self.cancelled)
    }
    /// Fragment validation remains the prepared worker's existing root-URI law.
    /// Its bool projection retains actual invalid V2 reports for the same drain.
    pub(crate) fn check_with_fragment(
        &mut self,
        path: &str,
        raw: &[u8],
        contract: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<bool, ItemRefusal> {
        self.guard(deadline, cancelled)?;
        self.row(path, raw, contract)?;
        if matches!(
            self.prepared.protocol,
            CutPreparedSchemaProtocol::DiagnosticsV2 { .. }
        ) {
            self.diagnostics_state(path, raw)?;
        }
        let result = self.worker.check(path, raw, contract, deadline, cancelled);
        self.finish_result(result)
    }
    /// The caller configured this exact V2 worker/quota before wrapping it.
    /// This method refuses on a scalar worker; it never enables a protocol.
    pub(crate) fn check_diagnostics_v2(
        &mut self,
        path: &str,
        raw: &[u8],
        contract: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<CandidateDiagnostic, ItemRefusal> {
        self.guard(deadline, cancelled)?;
        self.row(path, raw, contract)?;
        self.diagnostics_state(path, raw)?;
        let result = self
            .worker
            .check_diagnostics_v2(path, raw, contract, deadline, cancelled);
        let report = self.finish_result(result)?;
        self.verify_diagnostic(&report)?;
        Ok(report)
    }
    fn verify_diagnostic(&self, report: &CandidateDiagnostic) -> Result<(), ItemRefusal> {
        self.guard(self.deadline, self.cancelled)?;
        if report.input_identity() != self.input.input_identity()
            || report.schema_set_sha256() != self.prepared.schema_set_sha256
            || report.contract_selection_sha256() != self.selection_digest
            || report.prepared_execution_binding() != self.prepared
        {
            self.failed.set(true);
            self.input.abandon();
            return Err(refused());
        }
        Ok(())
    }
    pub(crate) fn take_schema_diagnostic_rejection(
        &mut self,
    ) -> Result<Option<CandidateDiagnostic>, ItemRefusal> {
        self.guard(self.deadline, self.cancelled)?;
        let report = self.worker.take_schema_diagnostic_rejection();
        // Move only: no second diagnostic vector or report cloning.
        if report.as_ref().is_some_and(|r| {
            self.baseline_state_bytes
                .checked_add(r.retained_state_bytes())
                .and_then(|n| n.checked_add(size_of::<Option<CandidateDiagnostic>>()))
                .is_none_or(|n| n > self.max_state_bytes)
        }) {
            self.failed.set(true);
            self.input.abandon();
            return Err(ItemRefusal::Budget);
        }
        if let Some(value) = &report {
            self.verify_diagnostic(value)?;
        }
        self.finish_result(Ok(report))
    }
    pub(crate) fn diagnostic_execution_count(&self) -> Result<usize, ItemRefusal> {
        self.guard(self.deadline, self.cancelled)?;
        self.finish_result(self.worker.diagnostic_execution_count())
    }
    pub(crate) fn diagnostics_v2_cumulative_cost(
        &self,
    ) -> Result<CutSchemaDiagnosticsCumulativeCost, ItemRefusal> {
        self.guard(self.deadline, self.cancelled)?;
        self.finish_result(self.worker.diagnostics_v2_cumulative_cost())
    }
    pub(crate) fn prepared_execution_binding(
        &self,
    ) -> Result<CutPreparedSchemaExecutionBinding, ItemRefusal> {
        self.guard(self.deadline, self.cancelled)?;
        Ok(self.prepared)
    }
}
impl CutSchemaExecutor for CandidateSchemaBinding<'_, '_, '_> {
    fn check(
        &mut self,
        path: &str,
        raw: &[u8],
        contract: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<bool, ItemRefusal> {
        self.check_with_fragment(path, raw, contract, deadline, cancelled)
    }
    fn check_reusing_scalar(
        &mut self,
        path: &str,
        raw: &[u8],
        contract: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<bool, ItemRefusal> {
        self.guard(deadline, cancelled)?;
        self.row(path, raw, contract)?;
        if matches!(
            self.prepared.protocol,
            CutPreparedSchemaProtocol::DiagnosticsV2 { .. }
        ) {
            self.diagnostics_state(path, raw)?;
        }
        let result = self
            .worker
            .check_reusing_scalar(path, raw, contract, deadline, cancelled);
        self.finish_result(result)
    }
    fn schema_input_cost(
        &self,
        path: &str,
        raw: &[u8],
        contract: &str,
        ordinal: u64,
    ) -> Result<CutSchemaInputCost, ItemRefusal> {
        self.row(path, raw, contract)?;
        self.finish_result(self.worker.schema_input_cost(path, raw, contract, ordinal))
    }
    fn finish(&mut self, deadline: Instant, cancelled: &AtomicBool) -> Result<(), ItemRefusal> {
        self.guard(deadline, cancelled)?;
        let result = self.worker.finish(deadline, cancelled);
        self.finish_result(result)
    }
}
impl SourceFoundationCandidateSchemaBinding<CandidateFence> for CandidateSchemaBinding<'_, '_, '_> {
    fn check_with_fragment(
        &mut self,
        path: &str,
        raw: &[u8],
        contract: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<bool, ItemRefusal> {
        CandidateSchemaBinding::check_with_fragment(self, path, raw, contract, deadline, cancelled)
    }
    fn check_diagnostics_v2(
        &mut self,
        path: &str,
        raw: &[u8],
        contract: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<CandidateDiagnostic, ItemRefusal> {
        CandidateSchemaBinding::check_diagnostics_v2(self, path, raw, contract, deadline, cancelled)
    }
    fn take_schema_diagnostic_rejection(
        &mut self,
    ) -> Result<Option<CandidateDiagnostic>, ItemRefusal> {
        CandidateSchemaBinding::take_schema_diagnostic_rejection(self)
    }
    fn diagnostic_execution_count(&self) -> Result<usize, ItemRefusal> {
        CandidateSchemaBinding::diagnostic_execution_count(self)
    }
    fn diagnostics_v2_cumulative_cost(
        &self,
    ) -> Result<CutSchemaDiagnosticsCumulativeCost, ItemRefusal> {
        CandidateSchemaBinding::diagnostics_v2_cumulative_cost(self)
    }
    fn input_identity(&self) -> &CandidateFence {
        self.input.input_identity()
    }
    fn prepared_execution_binding(&self) -> CutPreparedSchemaExecutionBinding {
        self.prepared
    }
    fn profile(&self) -> FormatProfile {
        self.prepared.schema_profile
    }
    fn schema_set_digest(&self) -> Digest256 {
        self.worker.schema_set_digest()
    }
    fn contract_selection_digest(&self) -> Digest256 {
        self.selection_digest
    }
    fn contract_digest(&self, contract: &str) -> Option<Digest256> {
        self.worker.contract_digest(contract)
    }
    fn for_each_selected_resource(
        &self,
        visit: &mut dyn FnMut(
            SourceFoundationCandidateSchemaResource<'_>,
        ) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal> {
        self.guard(self.deadline, self.cancelled)?;
        for resource in self.worker.source_resources() {
            self.guard(self.deadline, self.cancelled)?;
            self.finish_result(visit(SourceFoundationCandidateSchemaResource {
                path: resource.path,
                size_bytes: resource.size_bytes,
                sha256: resource.sha256,
            }))?;
        }
        self.guard(self.deadline, self.cancelled)
    }
}
