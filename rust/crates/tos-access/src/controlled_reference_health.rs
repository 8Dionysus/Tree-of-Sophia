//! Controlled five-child readiness composition using the existing Access owner.
use std::cell::RefCell;
use std::sync::Arc;
use crate::{AccessError, AccessErrorCode};
use crate::managed_local::{HealthChild, HealthOriginalBudget};
use tos_foundation::{JsonLimits, JsonValue};
use tos_compiler::{ControlledKnowledgeModel, ControlledQueryHeap};
fn failed() -> AccessError { AccessError::new(AccessErrorCode::BudgetExceeded,
    "controlled health Original admission refused") }
fn source_error(error: tos_query::search_v2::SearchV2Error) -> AccessError {
    use tos_query::search_v2::SearchV2ErrorCode as C;
    AccessError::new(match error.code {
        C::BudgetExceeded => AccessErrorCode::BudgetExceeded,
        C::Cancelled => AccessErrorCode::Cancelled,
        C::DeadlineExceeded => AccessErrorCode::DeadlineExceeded,
        C::CorruptSelectedCarrier => AccessErrorCode::CorruptSelectedCarrier,
        C::StaleSelection => AccessErrorCode::StaleSelection,
        _ => AccessErrorCode::Unavailable,
    }, error.message)
}
fn catalog_error(error: tos_query::CatalogError) -> AccessError {
    use tos_query::CatalogErrorCode as C;
    AccessError::new(match error.code {
        C::BudgetExceeded => AccessErrorCode::BudgetExceeded,
        C::Cancelled => AccessErrorCode::Cancelled,
        C::DeadlineExceeded => AccessErrorCode::DeadlineExceeded,
        C::StaleSelection => AccessErrorCode::StaleSelection,
        C::CorruptSelectedCarrier => AccessErrorCode::CorruptSelectedCarrier,
        C::Unauthorized => AccessErrorCode::PolicyDenied,
        C::PolicyBindingUnavailable => AccessErrorCode::Unavailable,
    }, error.message)
}
struct Original<'a, 'context, 'state, 'budget> {
    heap: &'a RefCell<ControlledQueryHeap<'context, 'state, 'budget>>,
}
impl HealthOriginalBudget for Original<'_, '_, '_, '_> {
    fn check(&self) -> Result<(), AccessError> { self.heap.borrow().check().map_err(|_| failed()) }
    fn admit_workspace(&self, bytes: usize) -> Result<(), AccessError> {
        self.heap.borrow_mut().retain(bytes).map_err(|_| failed())
    }
    fn charge_work(&self, units: usize) -> Result<(), AccessError> {
        self.heap.borrow().charge_work(units).map_err(|_| failed())
    }
    fn emit(&self, value: &JsonValue, mut limits: JsonLimits,
        meter: &mut tos_query::InspectVisitMeter) -> Result<Vec<u8>, AccessError> {
        self.check()?; limits.max_visits = limits.max_visits.min(meter.remaining());
        let before = self.heap.borrow().remaining_original_json_visits().map_err(|_| failed())?;
        let body = self.heap.borrow().canonicalize_owned_query_json(value, limits).map_err(|_| failed())?;
        let after = self.heap.borrow().remaining_original_json_visits().map_err(|_| failed())?;
        meter.charge_original_visits(before.checked_sub(after).ok_or_else(failed)?)
            .map_err(|_| failed())?;
        self.heap.borrow_mut().retain(body.capacity()).map_err(|_| failed())?;
        Ok(body)
    }
}
enum Fence<'hold> {
    Inspect(Box<dyn tos_query::InspectDisclosureLease + 'hold>),
    Catalog(Box<dyn tos_query::CatalogDisclosureLease + 'hold>),
}
impl Fence<'_> {
    fn recheck(&mut self) -> Result<(), AccessError> {
        match self {
            Self::Inspect(f) => f.recheck().map_err(source_error),
            Self::Catalog(f) => f.recheck().map_err(catalog_error),
        }
    }
}

pub(crate) fn execute_controlled_reference_health<'model, 'state, 'budget, 'view, 'capture>(
    model: &mut ControlledKnowledgeModel<'model, 'state, 'budget>,
    bound: &tos_query::BoundCmpKnowledge<'_>,
    context: &mut crate::reference_root_query::ReferenceMetadataContext<'view, 'capture>,
    corpus: &tos_query::corpus_read::CorpusReadContext, original_caps: tos_query::InspectBudget,
    probe: Arc<dyn tos_query::AbortProbe>,
    deliver: impl FnOnce(&[u8]) -> tos_compiler::Result<()>,
) -> tos_compiler::Result<()> {
    // The caller owns final transport disclosure inside its original model/cut
    // callback; no packet, JSON, FD or raw authority escapes this function.
    let mut caps = crate::managed_local::partition_health_inspect_budget(original_caps,
        original_caps.max_response_bytes, original_caps.max_decoded_bytes,
        original_caps.max_rows, original_caps.max_read_vm_steps)
        .map_err(|_| tos_compiler::Error::Budget("health aggregate partition"))?;
    caps.max_open_vm_steps = original_caps.max_open_vm_steps;
    let heap = RefCell::new(model.new_owned_query_heap());
    let original = Original { heap: &heap };
    let mut fences: [Option<Fence<'view>>; 5] = std::array::from_fn(|_| None);
    let mut fence_count = 0usize;
    let (body, _) = crate::managed_local::compose_selected_access_health_original(caps,
        original_caps.max_response_bytes, original_caps.json, &probe, Some(&original),
        |child, mut caps, meter| {
        crate::knowledge::check_abort(&probe)?;
        caps.json.max_visits = caps.json.max_visits.min(meter.remaining());
        let before = heap.borrow().remaining_original_json_visits().map_err(|_| failed())?;
        let corpus_request = match &child {
            HealthChild::CorpusView(id) => tos_query::corpus_read::CorpusReadRequest::GraphView {
                view_id: (*id).to_owned(), limit: 1 },
            _ => tos_query::corpus_read::CorpusReadRequest::Status,
        };
        let philosophy_request = match &child {
            HealthChild::PhilosophyView(id) => tos_query::philosophy_read::PhilosophyReadRequest::View {
                view_id: (*id).to_owned(), limit: 1 },
            _ => tos_query::philosophy_read::PhilosophyReadRequest::Views,
        };
        let operation = match &child {
            HealthChild::CorpusSeed | HealthChild::CorpusView(_) =>
                crate::reference_root_query::ReferenceMetadataOperation::for_corpus(&corpus_request),
            HealthChild::PhilosophySeed | HealthChild::PhilosophyView(_) =>
                crate::reference_root_query::ReferenceMetadataOperation::for_philosophy(&philosophy_request),
            HealthChild::Knowledge => crate::reference_root_query::ReferenceMetadataOperation::Catalog,
        };
        let mut output = None; let mut child_failure = None;
        context.with_operation(operation, |authority| {
            let mut parse_failure = None;
            let mut parse = |body: &[u8]| -> Result<(), tos_query::search_v2::SearchV2Error> {
                let result = heap.borrow_mut().with_owned_query_json(body, caps.json, |value, heap| {
                    let bytes = value.retained_storage_bytes().map_err(|_| tos_compiler::Error::Budget("health child state"))?;
                    heap.retain(bytes.checked_mul(4).ok_or(tos_compiler::Error::Budget("health child state"))?)?;
                    heap.charge_work(bytes)?; output = Some(value.clone()); Ok(())
                });
                if result.is_err() { parse_failure = Some(failed()); return Err(tos_query::search_v2::SearchV2Error::new(
                    tos_query::search_v2::SearchV2ErrorCode::BudgetExceeded, "health child Original parser refused")); }
                Ok(())
            };
            let result = match child {
                HealthChild::CorpusSeed => tos_query::execute_controlled_health_seed_response(model, bound,
                    authority, caps, tos_query::ControlledHealthSeed::Corpus, &mut parse).map_err(source_error),
                HealthChild::PhilosophySeed => tos_query::execute_controlled_health_seed_response(model, bound,
                    authority, caps, tos_query::ControlledHealthSeed::Philosophy, &mut parse).map_err(source_error),
                HealthChild::CorpusView(_) => tos_query::execute_controlled_corpus_response(model, bound,
                    authority, caps, &corpus_request, corpus, false, &mut parse).map_err(source_error),
                HealthChild::PhilosophyView(_) => tos_query::execute_controlled_philosophy_domain_response(model, bound,
                    authority, caps, &philosophy_request, false, &mut parse).map_err(source_error),
                HealthChild::Knowledge => tos_query::execute_controlled_knowledge_health_response(model, bound,
                    authority, caps, |body| parse(body).map_err(|e| tos_query::CatalogError {
                        code: tos_query::CatalogErrorCode::BudgetExceeded, message: e.message }))
                    .map_err(catalog_error),
            };
            drop(parse);
            if let Some(error) = parse_failure { child_failure = Some(error); return Ok(()); }
            if let Err(error) = result { child_failure = Some(error); return Ok(()); }
            // Acquire an actual current lease for the retained composite child.
            // The child itself held its own lease while parsing/copying; this
            // grant now covers that same immutable cut through final Reply.
            let fence = if matches!(child, HealthChild::Knowledge) {
                use tos_query::CatalogCurrentAuthority as C;
                let scope = C::disclosure_scope(authority);
                Fence::Catalog(C::acquire_disclosure(authority, &scope,
                    bound.selection().catalog_packet_sha256).map_err(|_| tos_compiler::Error::Invalid("health Catalog hold"))?)
            } else {
                use tos_query::InspectCurrentAuthority as I;
                let scope = I::disclosure_scope(authority);
                Fence::Inspect(I::acquire_disclosure(authority, &scope, &[])
                    .map_err(|_| tos_compiler::Error::Invalid("health Inspect hold"))?)
            };
            if fence_count >= fences.len() { return Err(tos_compiler::Error::Budget("health fence count")); }
            fences[fence_count] = Some(fence); fence_count += 1; Ok(())
        }).map_err(|_| AccessError::new(AccessErrorCode::StaleSelection, "controlled health scope refused"))?;
        let after = heap.borrow().remaining_original_json_visits().map_err(|_| failed())?;
        meter.charge_original_visits(before.checked_sub(after).ok_or_else(failed)?).map_err(|_| failed())?;
        if let Some(error) = child_failure { return Err(error); }
        output.ok_or_else(failed)
    }).map_err(|_| tos_compiler::Error::Invalid("controlled health composition refused"))?;
    for fence in fences.iter_mut().flatten() { fence.recheck().map_err(|_| tos_compiler::Error::Invalid("health final lease"))?; }
    model.check_pin()?; original.check().map_err(|_| tos_compiler::Error::Budget("health Original final check"))?;
    deliver(&body)?;
    for fence in fences.iter_mut().flatten() { fence.recheck().map_err(|_| tos_compiler::Error::Invalid("health final lease"))?; }
    model.check_pin()
}
