//! Same prepared candidate schema worker over candidate-fenced native Item input.
//! Resource bytes are checked by the native receiver through that exact input;
//! this adapter creates no candidate revision or admission/completion witness.
use crate::source_admission_candidate_records::CandidateRecordsInput;
use crate::source_admission_spooled_candidate::CandidateFence;
use std::{cell::Cell, mem::size_of, sync::atomic::AtomicBool, time::Instant};
use tos_foundation::{Digest256, SchemaExecutionBinding};
use tos_validation::{
    FormatProfile,
    item_rules::ItemRefusal,
    record_biblio_cut::SourceCutInputWithIdentity,
    source_cut::{
        CandidateCutSchemaDiagnostic, CandidateCutWorkerSchemaExecutor, CandidateSchemaDiagnostic,
        CutPreparedSchemaExecutionBinding, CutPreparedSchemaProtocol,
        CutSchemaDiagnosticsCumulativeCost, CutSchemaExecutor, CutSchemaInputCost,
        InProcessCandidateSchemaExecutor,
    },
    source_foundation_records::{
        SourceFoundationCandidateSchemaBinding, SourceFoundationCandidateSchemaResource,
    },
};

type CandidateDiagnostic = CandidateCutSchemaDiagnostic<CandidateFence>;
type CandidateDiagnosticEnvelope = CandidateSchemaDiagnostic<CandidateFence>;

fn diagnostic_matches_binding(
    report: &CandidateDiagnosticEnvelope,
    identity: &CandidateFence,
    prepared: SchemaExecutionBinding,
    selection: Digest256,
) -> bool {
    report.input_identity() == identity
        && report.execution_binding() == prepared
        && report.contract_selection_sha256() == selection
}

/// The worker-specific controller estimate remains available during migration.
/// Remove it with the worker adapter once the candidate route is in process.
pub(crate) trait CandidateSchemaExecutor:
    SourceFoundationCandidateSchemaBinding<CandidateFence>
{
    fn selected_metadata_state_bytes(&self) -> Option<usize>;
    fn worker_controller_state_upper_bound(
        &self,
        raw_bytes: usize,
        path_bytes: usize,
    ) -> Result<usize, ItemRefusal>;
}

impl CandidateSchemaExecutor for CandidateCutWorkerSchemaExecutor<CandidateFence> {
    fn selected_metadata_state_bytes(&self) -> Option<usize> {
        self.source_resource_metadata_state_bytes()
    }
    fn worker_controller_state_upper_bound(
        &self,
        raw_bytes: usize,
        path_bytes: usize,
    ) -> Result<usize, ItemRefusal> {
        self.diagnostics_v2_controller_state_upper_bound(raw_bytes, path_bytes)
    }
}

impl CandidateSchemaExecutor for InProcessCandidateSchemaExecutor<CandidateFence> {
    fn selected_metadata_state_bytes(&self) -> Option<usize> {
        self.source_resource_metadata_state_bytes()
    }
    fn worker_controller_state_upper_bound(
        &self,
        _raw_bytes: usize,
        _path_bytes: usize,
    ) -> Result<usize, ItemRefusal> {
        Err(ItemRefusal::Unsupported(
            "worker controller is not selected".into(),
        ))
    }
}

const SELECTOR_RETAINED_UPPER_BOUND_BYTES: usize = 16384;

/// State retained by the borrowed binding in addition to the caller's held
/// schema, resource metadata, and report header. The caller reserves this
/// before selecting the dependent callback workspace.
pub(crate) fn binding_retained_state_upper_bound_bytes() -> usize {
    size_of::<CandidateSchemaBinding<'static, 'static, 'static>>()
        + SELECTOR_RETAINED_UPPER_BOUND_BYTES
}

/// Additional adapter state beyond the caller's retained worker/store state.
/// Reserve it before dividing the remaining grant between Records and its
/// later dependent callback; both users consume the same existing envelope.
pub(crate) fn candidate_schema_binding_additional_state_bytes(
    worker: &impl CandidateSchemaExecutor,
) -> Result<usize, ItemRefusal> {
    worker
        .selected_metadata_state_bytes()
        .and_then(|bytes| bytes.checked_add(binding_retained_state_upper_bound_bytes()))
        .ok_or(ItemRefusal::Budget)
}

/// Invoke the maintained Records+Item receiver over the actual candidate input
/// and prepared candidate schema worker. This district report is not whole native
/// admission or a CMD creation-inventory completion. The caller owns the real
/// store, payload resolver, schema protocol envelope and their retained state.
#[allow(clippy::too_many_arguments)]
pub(crate) fn inspect_candidate_records_stored<'store>(
    input: &CandidateRecordsInput<'_, '_>,
    worker: &mut impl CandidateSchemaExecutor,
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
            .ok_or(tos_validation::item_budget_origin!())?;
        input.require_callback_state(
            callback_state,
            max_operation_state_bytes,
            "candidate record-schema callback state",
        )?;
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
    result.map_err(|error| match error {
        // Records owns its internal stage evidence. A legacy budget refusal
        // reaching this seam without it belongs to the adapter's input fences.
        ItemRefusal::Budget => ItemRefusal::BudgetCheck {
            check: "candidate Records adapter input fence",
            used: None,
            limit: None,
        },
        other => other,
    })
}

pub(crate) struct CandidateSchemaBinding<'a, 'input, 'host> {
    input: &'a CandidateRecordsInput<'input, 'host>,
    worker: &'a mut dyn CandidateSchemaExecutor,
    prepared: SchemaExecutionBinding,
    selection_digest: Digest256,
    deadline: Instant,
    cancelled: &'a AtomicBool,
    baseline_state_bytes: usize,
    max_state_bytes: usize,
    failed: Cell<bool>,
}
#[track_caller]
fn refused() -> ItemRefusal {
    let site = std::panic::Location::caller().line();
    ItemRefusal::Source(crate::source_admission_spooled_index::bounded_source_cause(
        "candidate-schema",
        &site.to_string(),
        "candidate schema binding refused",
    ))
}
impl<'a, 'input, 'host> CandidateSchemaBinding<'a, 'input, 'host> {
    /// `retained_state_bytes` includes the caller's held schema/worker/input,
    /// resource metadata, and report header, but excludes callback workspace.
    /// The binding's own retained state is added exactly once.
    /// This constructor performs no parsing, cloning or worker preparation.
    pub(crate) fn new(
        input: &'a CandidateRecordsInput<'input, 'host>,
        worker: &'a mut impl CandidateSchemaExecutor,
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
        worker: &'a mut impl CandidateSchemaExecutor,
        deadline: Instant,
        cancelled: &'a AtomicBool,
        retained_state_bytes: usize,
        max_state_bytes: usize,
    ) -> Result<Self, ItemRefusal> {
        input.verify_invocation(deadline, cancelled)?;
        let prepared = worker.schema_execution_binding().ok_or_else(refused)?;
        let (_, schema_bytes) = worker.selected_resource_totals()?;
        if worker.input_identity() != input.input_identity()
            || prepared.schema_profile() != worker.profile()
            || prepared.schema_set_sha256() != worker.schema_set_digest()
            || usize::try_from(schema_bytes).map_or(true, |bytes| retained_state_bytes < bytes)
            || matches!(prepared, SchemaExecutionBinding::InProcessV1(binding)
                if binding.engine != tos_validation::in_process::ENGINE_ID
                    || binding.input_mode_sha256.is_none())
        {
            return Err(refused());
        }
        let baseline_state_bytes = retained_state_bytes
            .checked_add(candidate_schema_binding_additional_state_bytes(worker)?)
            .filter(|n| *n <= max_state_bytes)
            .ok_or(tos_validation::item_budget_origin!())?;
        let mut previous = String::new();
        worker.for_each_selected_resource(&mut |resource| {
            input.verify_invocation(deadline, cancelled)?;
            if resource.path.is_empty()
                || (!previous.is_empty() && previous.as_str() >= resource.path)
            {
                return Err(refused());
            }
            previous.clear();
            previous.push_str(resource.path);
            Ok(())
        })?;
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
            || self.worker.schema_execution_binding() != Some(self.prepared)
            || self.worker.input_identity() != self.input.input_identity()
            || self.worker.schema_set_digest() != self.prepared.schema_set_sha256()
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
            return Err(tos_validation::item_budget_origin!());
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
            .worker_controller_state_upper_bound(raw.len(), path.len());
        let state = self.finish_result(state)?;
        if self
            .baseline_state_bytes
            .checked_add(state)
            .is_none_or(|n| n > self.max_state_bytes)
        {
            self.failed.set(true);
            self.input.abandon();
            return Err(tos_validation::item_budget_origin!());
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
        if matches!(self.prepared,
            SchemaExecutionBinding::Worker(binding)
                if matches!(binding.protocol, CutPreparedSchemaProtocol::DiagnosticsV2 { .. }))
        {
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

    fn verify_envelope(&self, report: &CandidateDiagnosticEnvelope) -> Result<(), ItemRefusal> {
        self.guard(self.deadline, self.cancelled)?;
        if !diagnostic_matches_binding(
            report,
            self.input.input_identity(),
            self.prepared,
            self.selection_digest,
        ) {
            self.failed.set(true);
            self.input.abandon();
            return Err(refused());
        }
        Ok(())
    }

    fn envelope_state(report: &CandidateDiagnosticEnvelope) -> Option<usize> {
        use tos_validation::executor::schema_diagnostics::{Issue, PathSegment};
        match report {
            CandidateSchemaDiagnostic::Worker(report) => Some(report.retained_state_bytes()),
            CandidateSchemaDiagnostic::InProcess(report) => {
                let diagnostic = report.diagnostic();
                let mut state = size_of::<CandidateDiagnosticEnvelope>()
                    .checked_add(diagnostic.path.capacity())?
                    .checked_add(diagnostic.contract.capacity())?
                    .checked_add(diagnostic.report.root_uri.capacity())?
                    .checked_add(
                        diagnostic
                            .report
                            .issues
                            .capacity()
                            .checked_mul(size_of::<Issue>())?,
                    )?;
                for issue in &diagnostic.report.issues {
                    state = state.checked_add(issue.schema_keyword.capacity())?;
                    for path in [&issue.instance_path, &issue.schema_path] {
                        state = state
                            .checked_add(path.capacity().checked_mul(size_of::<PathSegment>())?)?;
                        for segment in path {
                            if let PathSegment::Property(name) = segment {
                                state = state.checked_add(name.capacity())?;
                            }
                        }
                    }
                }
                Some(state)
            }
        }
    }

    fn check_envelope_state(
        &self,
        report: &CandidateDiagnosticEnvelope,
    ) -> Result<(), ItemRefusal> {
        if Self::envelope_state(report)
            .and_then(|state| self.baseline_state_bytes.checked_add(state))
            .and_then(|state| state.checked_add(size_of::<Option<CandidateDiagnosticEnvelope>>()))
            .is_none_or(|state| state > self.max_state_bytes)
        {
            self.failed.set(true);
            self.input.abandon();
            return Err(tos_validation::item_budget_origin!());
        }
        Ok(())
    }
    fn verify_diagnostic(&self, report: &CandidateDiagnostic) -> Result<(), ItemRefusal> {
        self.guard(self.deadline, self.cancelled)?;
        if report.input_identity() != self.input.input_identity()
            || report.schema_set_sha256() != self.prepared.schema_set_sha256()
            || report.contract_selection_sha256() != self.selection_digest
            || SchemaExecutionBinding::Worker(report.prepared_execution_binding()) != self.prepared
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
        let result = self.worker.take_schema_diagnostic_rejection();
        let report = self.finish_result(result)?;
        // Move only: no second diagnostic vector or report cloning.
        if report.as_ref().is_some_and(|r| {
            self.baseline_state_bytes
                .checked_add(r.retained_state_bytes())
                .and_then(|n| n.checked_add(size_of::<Option<CandidateDiagnostic>>()))
                .is_none_or(|n| n > self.max_state_bytes)
        }) {
            self.failed.set(true);
            self.input.abandon();
            return Err(tos_validation::item_budget_origin!());
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
}
impl CutSchemaExecutor for CandidateSchemaBinding<'_, '_, '_> {
    fn prepared_execution_binding(&self) -> Option<CutPreparedSchemaExecutionBinding> {
        match self.prepared {
            SchemaExecutionBinding::Worker(binding) => Some(binding),
            SchemaExecutionBinding::InProcessV1(_) => None,
        }
    }
    fn schema_execution_binding(&self) -> Option<SchemaExecutionBinding> {
        Some(self.prepared)
    }
    fn schema_profile(&self) -> Option<FormatProfile> {
        Some(self.prepared.schema_profile())
    }
    fn selected_contract_digest(&self, contract: &str) -> Option<Digest256> {
        self.worker.contract_digest(contract)
    }

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
        if matches!(self.prepared,
            SchemaExecutionBinding::Worker(binding)
                if matches!(binding.protocol, CutPreparedSchemaProtocol::DiagnosticsV2 { .. }))
        {
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
    fn check_diagnostic(
        &mut self,
        path: &str,
        raw: &[u8],
        contract: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<CandidateDiagnosticEnvelope, ItemRefusal> {
        self.guard(deadline, cancelled)?;
        self.row(path, raw, contract)?;
        if matches!(self.prepared,
            SchemaExecutionBinding::Worker(binding)
                if matches!(binding.protocol, CutPreparedSchemaProtocol::DiagnosticsV2 { .. }))
        {
            self.diagnostics_state(path, raw)?;
        }
        let result = self
            .worker
            .check_diagnostic(path, raw, contract, deadline, cancelled);
        let report = self.finish_result(result)?;
        self.verify_envelope(&report)?;
        if report.path() != path
            || report.contract() != contract
            || report.input_sha256() != Digest256::of_bytes(raw)
        {
            self.failed.set(true);
            self.input.abandon();
            return Err(refused());
        }
        self.check_envelope_state(&report)?;
        Ok(report)
    }
    fn take_diagnostic_rejection(
        &mut self,
    ) -> Result<Option<CandidateDiagnosticEnvelope>, ItemRefusal> {
        self.guard(self.deadline, self.cancelled)?;
        let result = self.worker.take_diagnostic_rejection();
        let report = self.finish_result(result)?;
        if let Some(report) = &report {
            self.verify_envelope(report)?;
            self.check_envelope_state(report)?;
        }
        Ok(report)
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
    fn profile(&self) -> FormatProfile {
        self.prepared.schema_profile()
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
    fn selected_resource_totals(&self) -> Result<(u64, u64), ItemRefusal> {
        self.guard(self.deadline, self.cancelled)?;
        self.worker.selected_resource_totals()
    }
    fn for_each_selected_resource(
        &self,
        visit: &mut dyn FnMut(
            SourceFoundationCandidateSchemaResource<'_>,
        ) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal> {
        self.guard(self.deadline, self.cancelled)?;
        self.worker.for_each_selected_resource(&mut |resource| {
            self.guard(self.deadline, self.cancelled)?;
            self.finish_result(visit(SourceFoundationCandidateSchemaResource {
                path: resource.path,
                size_bytes: resource.size_bytes,
                sha256: resource.sha256,
            }))?;
            Ok(())
        })?;
        self.guard(self.deadline, self.cancelled)
    }
}

#[cfg(test)]
mod candidate_engine_tests {
    use super::*;
    use std::time::Duration;
    use tos_validation::{SchemaResource, in_process::InputMode};

    #[test]
    fn in_process_candidate_diagnostic_binds_engine_mode_and_fence() {
        let digest = |value: &[u8]| Digest256::of_bytes(value);
        let fence = CandidateFence {
            batch_sha256: digest(b"batch"),
            base_revision: None,
            validator_sha256: digest(b"validator"),
            membership: tos_source_store::SourceMembershipV1 {
                count: 1,
                digest: digest(b"membership"),
            },
            source_bytes: 1,
            retirement_count: 0,
            retirement_digest: digest(b"retirement"),
        };
        let path = "ToS/contracts/example.schema.json";
        let raw = br#"{"$schema":"https://json-schema.org/draft/2020-12/schema","$id":"https://example.test/schema","type":"integer"}"#;
        let deadline = Instant::now() + Duration::from_secs(30);
        let cancelled = AtomicBool::new(false);
        let mut schemas = InProcessCandidateSchemaExecutor::from_selected_source_resources(
            fence,
            vec![(
                path.into(),
                SchemaResource {
                    uri: "https://example.test/schema".into(),
                    raw: raw.to_vec(),
                },
            )],
            FormatProfile::AssertedSourceCandidateV1,
            InputMode::PublishedStrict,
            deadline,
            &cancelled,
        )
        .unwrap();
        let executor: &mut dyn CandidateSchemaExecutor = &mut schemas;
        let binding = executor.schema_execution_binding().unwrap();
        let selection = executor.contract_selection_digest();
        assert!(executor.selected_metadata_state_bytes().unwrap() > 0);
        let report = executor
            .check_diagnostic("input.json", br#""bad""#, path, deadline, &cancelled)
            .unwrap();
        assert!(report.is_invalid());
        assert!(diagnostic_matches_binding(
            &report, &fence, binding, selection
        ));
        assert!(CandidateSchemaBinding::envelope_state(&report).unwrap() > 0);
        let mut different_fence = fence;
        different_fence.batch_sha256 = digest(b"other batch");
        assert!(!diagnostic_matches_binding(
            &report,
            &different_fence,
            binding,
            selection
        ));
        let SchemaExecutionBinding::InProcessV1(mut changed_mode) = binding else {
            panic!("in-process binding expected")
        };
        changed_mode.input_mode_sha256 = Some(InputMode::SerdeNormalized.identity_sha256());
        assert!(!diagnostic_matches_binding(
            &report,
            &fence,
            SchemaExecutionBinding::InProcessV1(changed_mode),
            selection
        ));
    }
}
