//! Maintained Corpus metadata over the authentic cold Original namespace.
use crate::controlled_query_adapter::compiler_query_error;
use crate::controlled_original_reader::{read_original, OriginalReadCharges};
use crate::{BoundCmpKnowledge, InspectBudget, InspectCurrentAuthority};
use crate::search_v2::{SearchV2Error, SearchV2ErrorCode};
use tos_compiler::{ControlledKnowledgeModel, ControlledOriginalCollection, CorpusOriginalCollection};
use tos_foundation::{JsonString, JsonValue, OwnedState};
fn budget() -> SearchV2Error { SearchV2Error { code: SearchV2ErrorCode::BudgetExceeded,
    message: "controlled philosophy status budget exceeded" } }
fn corrupt() -> SearchV2Error { SearchV2Error { code: SearchV2ErrorCode::CorruptSelectedCarrier,
    message: "controlled philosophy status header differs" } }

pub fn execute_controlled_corpus_metadata_response<'hold,
    A: InspectCurrentAuthority<'hold> + ?Sized>(
    model: &mut ControlledKnowledgeModel<'_, '_, '_>, bound: &BoundCmpKnowledge<'_>,
    authority: &mut A, caps: InspectBudget,
    request: &crate::corpus_read::CorpusReadRequest,
    context: &crate::corpus_read::CorpusReadContext, render: bool,
    deliver: impl FnOnce(&[u8]) -> Result<(), SearchV2Error>,
) -> Result<(), SearchV2Error> {
    execute_controlled_corpus_response(model, bound, authority, caps,
        request, context, render, deliver)
}

pub fn execute_controlled_corpus_response<'hold,
    A: InspectCurrentAuthority<'hold> + ?Sized>(
    model: &mut ControlledKnowledgeModel<'_, '_, '_>, bound: &BoundCmpKnowledge<'_>,
    authority: &mut A, caps: InspectBudget,
    request: &crate::corpus_read::CorpusReadRequest,
    context: &crate::corpus_read::CorpusReadContext, render: bool,
    deliver: impl FnOnce(&[u8]) -> Result<(), SearchV2Error>,
) -> Result<(), SearchV2Error> {
    bound.check_controlled_model(model)?;
    model.check_query_open_vm_admission(caps.max_open_vm_steps).map_err(compiler_query_error)?;
    let forecast = authority.disclosure_metadata_state_upper_bound()?;
    let frame = std::mem::size_of_val(&deliver)
        .checked_add(std::mem::size_of::<crate::search_v2::CurrentPolicyBinding>())
        .and_then(|n| n.checked_add(std::mem::size_of::<crate::IndexedDisclosureScope>()))
        .and_then(|n| n.checked_add(std::mem::size_of::<OriginalReadCharges>()))
        .ok_or_else(budget)?;
    let mut result = Ok(());
    model.with_owned_query_workspace(forecast.checked_add(frame)
        .ok_or(tos_compiler::Error::Budget("philosophy metadata frame"))?, |model| {
        result = (|| {
            let policy = authority.policy_binding();
            let scope = authority.disclosure_scope();
            scope.validate_for(bound, &policy, request.operation_id(),
                crate::corpus_read::CORPUS_INTENDED_USE)?;
            let actual = policy.retained_state_bytes().map_err(|_| budget())?
                .checked_add(crate::knowledge_inspect::scope_owned_state(&scope)?)
                .ok_or_else(budget)?;
            if actual > forecast { return Err(budget()); }
            authority.check_selected()?;
            let receipt = model.corpus_original_receipt().ok_or_else(corrupt)?;
            if receipt.descriptor_sha256 != bound.selection().vocabulary.descriptor_sha256.to_hex()
                || receipt.source_cut != bound.selection().source_cut
                || receipt.membership_root != bound.selection().source_membership_root.to_hex() {
                return Err(corrupt());
            }
            let mut heap = model.new_owned_query_heap();
            let receipt_state = model.corpus_original_receipt().ok_or_else(corrupt)?
                .retained_state_bytes().map_err(|_| budget())?;
            heap.retain(receipt_state).map_err(compiler_query_error)?;
            heap.charge_work(receipt_state).map_err(compiler_query_error)?;
            let receipt = model.corpus_original_receipt().ok_or_else(corrupt)?.clone();
            let mut charges = OriginalReadCharges::default();
            let (ordinal, header) = read_original(model, authority,
                ControlledOriginalCollection::Corpus(CorpusOriginalCollection::Header),
                -1, caps, &mut charges, &mut heap)?.ok_or_else(corrupt)?;
            if ordinal != 0 || header.as_object().is_none() { return Err(corrupt()); }
            let header_state = header.retained_storage_bytes().map_err(|_| budget())?;
            let context_state = context.tos_root.len().checked_add(context.index_path.len()).ok_or_else(budget)?;
            // The existing kernel's fixed envelopes plus Original header clones.
            // Each selected row is separately admitted by ControlledCorpusReader
            // before entering its maps/row clones and final packet.
            let fixed = header_state.checked_mul(4)
                .and_then(|n| n.checked_add(context_state.checked_mul(5)?))
                .and_then(|n| n.checked_add(32 * (std::mem::size_of::<JsonString>()
                    + std::mem::size_of::<JsonValue>()) + 5 * 1024)).ok_or_else(budget)?;
            heap.retain(fixed).map_err(compiler_query_error)?;
            heap.charge_work(fixed).map_err(compiler_query_error)?;
            let packet = {
                let mut read = crate::controlled_corpus_reader::ControlledCorpusReader {
                    model, authority, caps, charges: &mut charges, heap: &mut heap,
                };
                crate::corpus_read::compute_controlled_corpus(&mut read, receipt, header,
                    context, request, crate::corpus_read::CorpusReadBudget {
                        inspect: caps, max_work_steps: caps.max_read_vm_steps })?
            };
            let body = if render {
                let mut limits = caps.json;
                limits.max_bytes = limits.max_bytes.min(caps.max_response_bytes);
                let pretty = heap.emit_python_pretty_owned_json(&packet, limits)
                    .map_err(compiler_query_error)?;
                heap.retain(pretty.capacity()).map_err(compiler_query_error)?;
                let text = std::str::from_utf8(&pretty).map_err(|_| corrupt())?;
                // UTF-8 plus a growable UTF-16 buffer and its inline JsonValue.
                let string_state = text.len().checked_mul(1 + 2 * std::mem::size_of::<u16>())
                    .and_then(|n| n.checked_add(std::mem::size_of::<JsonValue>())).ok_or_else(budget)?;
                heap.retain(string_state).map_err(compiler_query_error)?;
                heap.charge_work(text.len()).map_err(compiler_query_error)?;
                let value = JsonValue::String(JsonString::from_utf8(text));
                heap.canonicalize_owned_query_json(&value, caps.json).map_err(compiler_query_error)?
            } else {
                heap.canonicalize_owned_query_json(&packet, caps.json).map_err(compiler_query_error)?
            };
            // The renderer/canonical writer admitted the allocation against the
            // original remainder; transfer actual capacity into the held query
            // before sending, without reserving the response cap twice.
            heap.retain(body.capacity()).map_err(compiler_query_error)?;
            if body.len() > caps.max_response_bytes { return Err(budget()); }
            authority.check_selected()?;
            let mut lease = authority.acquire_disclosure(&scope, &[])?;
            lease.recheck()?;
            model.check_pin().map_err(compiler_query_error)?;
            deliver(&body)?;
            lease.recheck()?;
            model.check_pin().map_err(compiler_query_error)
        })();
        Ok(())
    }).map_err(compiler_query_error)?;
    result
}
