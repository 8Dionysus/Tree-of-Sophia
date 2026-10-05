//! Maintained Philosophy Status over the authentic cold Original header.
use crate::controlled_query_adapter::compiler_query_error;
use crate::controlled_original_reader::{read_original, OriginalReadCharges};
use crate::{BoundCmpKnowledge, InspectBudget, InspectCurrentAuthority};
use crate::search_v2::{SearchV2Error, SearchV2ErrorCode};
use tos_compiler::{ControlledKnowledgeModel, ControlledOriginalCollection, PhilosophyOriginalCollection};
use tos_foundation::{JsonString, JsonValue, OwnedState};
fn budget() -> SearchV2Error { SearchV2Error { code: SearchV2ErrorCode::BudgetExceeded,
    message: "controlled philosophy status budget exceeded" } }
fn corrupt() -> SearchV2Error { SearchV2Error { code: SearchV2ErrorCode::CorruptSelectedCarrier,
    message: "controlled philosophy status header differs" } }

pub fn execute_controlled_philosophy_domain_response<'hold,
    A: InspectCurrentAuthority<'hold> + ?Sized>(
    model: &mut ControlledKnowledgeModel<'_, '_, '_>, bound: &BoundCmpKnowledge<'_>,
    authority: &mut A, caps: InspectBudget,
    request: &crate::philosophy_read::PhilosophyReadRequest, render: bool,
    deliver: impl FnOnce(&[u8]) -> Result<(), SearchV2Error>,
) -> Result<(), SearchV2Error> {
    match request {
        crate::philosophy_read::PhilosophyReadRequest::Views => (),
        crate::philosophy_read::PhilosophyReadRequest::View {view_id, limit}
            if !view_id.is_empty() && view_id.len() <= caps.max_field_bytes && (1..=1000).contains(limit) => (),
        _ => return Err(SearchV2Error::new(SearchV2ErrorCode::InvalidRequest,
            "controlled Philosophy domain request unavailable")),
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
        .ok_or(tos_compiler::Error::Budget("philosophy metadata frame")).map_err(compiler_query_error)?, |model| {
        result = (|| {
            let policy = authority.policy_binding();
            let scope = authority.disclosure_scope();
            scope.validate_for(bound, &policy, request.operation_id(),
                crate::philosophy_read::PHILOSOPHY_INTENDED_USE)?;
            let actual = policy.retained_state_bytes().map_err(|_| budget())?
                .checked_add(crate::knowledge_inspect::scope_owned_state(&scope)?)
                .ok_or_else(budget)?;
            if actual > forecast { return Err(budget()); }
            authority.check_selected()?;
            let receipt = model.philosophy_original_receipt().ok_or_else(corrupt)?;
            if receipt.source_graph != bound.source_for_adapter("philosophy-node-edge-v1")
                .ok_or_else(corrupt)? || receipt.descriptor_sha256 != bound.selection().vocabulary.descriptor_sha256.to_hex()
                || receipt.source_cut != bound.selection().source_cut
                || receipt.membership_root != bound.selection().source_membership_root.to_hex() {
                return Err(corrupt());
            }
            let expected_nodes = receipt.nodes;
            let expected_edges = receipt.edges;
            let mut heap = model.new_owned_query_heap();
            let mut charges = OriginalReadCharges::default();
            let (ordinal, header) = read_original(model, authority,
                ControlledOriginalCollection::Philosophy(PhilosophyOriginalCollection::Header),
                -1, caps, &mut charges, &mut heap)?.ok_or_else(corrupt)?;
            if ordinal != 0 || header.as_object().is_none() { return Err(corrupt()); }
            let mut read_rows = |collection, count: u64| -> Result<Vec<JsonValue>, SearchV2Error> {
                let mut output = Vec::new(); let mut after = -1;
                for expected in 0..count {
                    let (ordinal, row) = read_original(model, authority,
                        ControlledOriginalCollection::Philosophy(collection), after, caps,
                        &mut charges, &mut heap)?.ok_or_else(corrupt)?;
                    if u64::try_from(ordinal).map_err(|_| corrupt())? != expected { return Err(corrupt()); }
                    after = ordinal;
                    heap.retain(2 * std::mem::size_of::<JsonValue>()).map_err(compiler_query_error)?;
                    output.push(row);
                }
                Ok(output)
            };
            let nodes = read_rows(PhilosophyOriginalCollection::Nodes, expected_nodes)?;
            let edges = read_rows(PhilosophyOriginalCollection::Edges, expected_edges)?;
            drop(read_rows);
            // Original row clones remain held separately. Before Graph builds
            // its borrowed ID maps and the View kernel clones its output,
            // admit workspace from the actual complete input geometry.
            let input = header.retained_storage_bytes().map_err(|_| budget())?
                .checked_add(nodes.retained_state_bytes().map_err(|_| budget())?)
                .and_then(|n| n.checked_add(edges.retained_state_bytes().ok()?))
                .ok_or_else(budget)?;
            let frames = expected_nodes.checked_add(expected_edges)
                .and_then(|n| n.checked_mul(4 * std::mem::size_of::<(usize, usize, usize)>() as u64))
                .and_then(|n| usize::try_from(n).ok()).ok_or_else(budget)?;
            let workspace = input.checked_mul(8).and_then(|n| n.checked_add(frames))
                .and_then(|n| n.checked_add(32 * (std::mem::size_of::<JsonString>()
                    + std::mem::size_of::<JsonValue>()) + 5 * 1024)).ok_or_else(budget)?;
            heap.retain(workspace).map_err(compiler_query_error)?;
            heap.charge_work(workspace).map_err(compiler_query_error)?;
            let mut interrupt = || {
                heap.check().map_err(compiler_query_error)?;
                heap.charge_work(1).map_err(compiler_query_error)
            };
            let packet = crate::philosophy_read::compute_philosophy_read(&header, &nodes,
                &edges, request, caps.max_read_vm_steps, &mut interrupt)?;
            drop(interrupt);
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
