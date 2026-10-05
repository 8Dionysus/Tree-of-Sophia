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
    if !matches!(request, crate::corpus_read::CorpusReadRequest::Status
        | crate::corpus_read::CorpusReadRequest::Summary | crate::corpus_read::CorpusReadRequest::GraphViews) {
        return Err(corrupt());
    }
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
            let expected_views = receipt.collections.iter().find(|c| c.collection == "graph_views")
                .ok_or_else(corrupt)?.rows;
            let expected_branches = receipt.collections.iter().find(|c| c.collection == "branches")
                .ok_or_else(corrupt)?.rows;
            let mut heap = model.new_owned_query_heap();
            let mut charges = OriginalReadCharges::default();
            let (ordinal, header) = read_original(model, authority,
                ControlledOriginalCollection::Corpus(CorpusOriginalCollection::Header),
                -1, caps, &mut charges, &mut heap)?.ok_or_else(corrupt)?;
            if ordinal != 0 || header.as_object().is_none() { return Err(corrupt()); }
            let mut read_rows = |collection| -> Result<Vec<JsonValue>, SearchV2Error> {
                let mut rows = Vec::new(); let mut after = -1;
                while let Some((ordinal, value)) = read_original(model, authority,
                    ControlledOriginalCollection::Corpus(collection), after, caps, &mut charges, &mut heap)? {
                    if ordinal <= after { return Err(corrupt()); }
                    after = ordinal;
                    heap.retain(2 * std::mem::size_of::<JsonValue>()).map_err(compiler_query_error)?;
                    rows.push(value);
                }
                Ok(rows)
            };
            let views = read_rows(CorpusOriginalCollection::GraphViews)?;
            if views.len() as u64 != expected_views { return Err(corrupt()); }
            let branches = if matches!(request, crate::corpus_read::CorpusReadRequest::Summary) {
                read_rows(CorpusOriginalCollection::Branches)?
            } else { Vec::new() };
            drop(read_rows);
            if matches!(request, crate::corpus_read::CorpusReadRequest::Summary)
                && branches.len() as u64 != expected_branches { return Err(corrupt()); }
            // Same supported-view predicate as the maintained Corpus reader.
            let views = views.into_iter().filter(|v| matches!(v.object_get("view_id")
                .and_then(JsonValue::as_str), Some("corpus-topology" | "route-graph" | "promotion-flow")))
                .collect::<Vec<_>>();
            let inputs = header.retained_storage_bytes().map_err(|_| budget())?
                .checked_add(views.retained_state_bytes().map_err(|_| budget())?)
                .and_then(|n| n.checked_add(branches.retained_state_bytes().ok()?))
                .and_then(|n| n.checked_add(context.tos_root.len()))
                .and_then(|n| n.checked_add(context.index_path.len())).ok_or_else(budget)?;
            let literals = concat!("schema", "tos_corpus_mcp_summary_v1", "tos_corpus_mcp_status_v1",
                "tos_corpus_mcp_graph_views_v1", "index_exists", "tos_root", "index_path", "owner_repo",
                "surface_kind", "counts", "graph_views", "authority_order", "runtime_projection_boundary",
                "status", "branches").len();
            let bytes = inputs.checked_mul(3)
                .and_then(|n| n.checked_add(2 * 20 * std::mem::size_of::<(JsonString, JsonValue)>()))
                .and_then(|n| n.checked_add(literals.checked_mul(1 + 2 * std::mem::size_of::<u16>())?))
                .ok_or_else(budget)?;
            heap.retain(bytes).map_err(compiler_query_error)?;
            heap.charge_work(bytes).map_err(compiler_query_error)?;
            let packet = crate::corpus_read::controlled_metadata(&header, context, views, branches, request)?;
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
