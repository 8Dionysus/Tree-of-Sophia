//! Maintained Philosophy Status over the authentic cold Original header.
use crate::controlled_original_reader::{OriginalReadCharges, read_original};
use crate::controlled_query_adapter::compiler_query_error;
use crate::search_v2::{SearchV2Error, SearchV2ErrorCode};
use crate::{BoundCmpKnowledge, InspectBudget, InspectCurrentAuthority};
use tos_compiler::{
    ControlledKnowledgeModel, ControlledOriginalCollection, PhilosophyOriginalCollection,
};
use tos_foundation::{JsonString, JsonValue, OwnedState};
fn budget() -> SearchV2Error {
    SearchV2Error {
        code: SearchV2ErrorCode::BudgetExceeded,
        message: "controlled philosophy status budget exceeded",
    }
}
fn corrupt() -> SearchV2Error {
    SearchV2Error {
        code: SearchV2ErrorCode::CorruptSelectedCarrier,
        message: "controlled philosophy status header differs",
    }
}

pub fn execute_controlled_philosophy_status_response<
    'hold,
    A: InspectCurrentAuthority<'hold> + ?Sized,
>(
    model: &mut ControlledKnowledgeModel<'_, '_, '_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    caps: InspectBudget,
    deliver: impl FnOnce(&[u8]) -> Result<(), SearchV2Error>,
) -> Result<(), SearchV2Error> {
    execute_controlled_philosophy_status_response_render(
        model, bound, authority, caps, false, deliver,
    )
}

pub fn execute_controlled_philosophy_status_response_render<
    'hold,
    A: InspectCurrentAuthority<'hold> + ?Sized,
>(
    model: &mut ControlledKnowledgeModel<'_, '_, '_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    caps: InspectBudget,
    render: bool,
    deliver: impl FnOnce(&[u8]) -> Result<(), SearchV2Error>,
) -> Result<(), SearchV2Error> {
    execute_controlled_philosophy_metadata_response_render(
        model,
        bound,
        authority,
        caps,
        &crate::philosophy_read::PhilosophyReadRequest::Status,
        render,
        deliver,
    )
}

pub fn execute_controlled_philosophy_metadata_response_render<
    'hold,
    A: InspectCurrentAuthority<'hold> + ?Sized,
>(
    model: &mut ControlledKnowledgeModel<'_, '_, '_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    caps: InspectBudget,
    request: &crate::philosophy_read::PhilosophyReadRequest,
    render: bool,
    deliver: impl FnOnce(&[u8]) -> Result<(), SearchV2Error>,
) -> Result<(), SearchV2Error> {
    if !matches!(
        request,
        crate::philosophy_read::PhilosophyReadRequest::Status
            | crate::philosophy_read::PhilosophyReadRequest::Layers
            | crate::philosophy_read::PhilosophyReadRequest::Snapshot
    ) {
        return Err(corrupt());
    }
    bound.check_controlled_model(model)?;
    model
        .check_query_open_vm_admission(caps.max_open_vm_steps)
        .map_err(compiler_query_error)?;
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
            let mut heap = model.new_owned_query_heap();
            let mut charges = OriginalReadCharges::default();
            let (ordinal, header) = read_original(model, authority,
                ControlledOriginalCollection::Philosophy(PhilosophyOriginalCollection::Header),
                -1, caps, &mut charges, &mut heap)?.ok_or_else(corrupt)?;
            if ordinal != 0 || header.as_object().is_none() { return Err(corrupt()); }
            // Status clones disjoint header fields once. The two array filters
            // also materialize borrowed reference vectors; account their exact
            // source capacities separately from the output's 13 fields and
            // two possible fallback fields.
            let lists = ["views", "graph_layers"].iter().try_fold(0usize, |n, key| {
                n.checked_add(header.object_get(key).and_then(JsonValue::as_array)
                    .map_or(0, |a| a.len())).ok_or_else(budget)
            })?;
            let literals = concat!("layer_counts", "tos_philosophy_mcp_layers_v1", "tos_philosophy_mcp_snapshot_v1",
                "Tree-of-Sophia owns snapshot semantics; MCP serves fingerprints for review and diff routing.", "schema", "tos_philosophy_mcp_status_v1", "projection_exists",
                "tos_root", "Tree-of-Sophia", "projection_path", "ToS/derived-exports/philosophy_graph_projection.min.json",
                "owner_repo", "surface_kind", "counts", "views", "graph_layers", "visibility_model",
                "snapshot_review", "runtime_projection_boundary", "runtime_owner", "abyss-stack",
                "missing_state", "ToS philosophy graph projection is not present at this MCP path",
                "authority_note", "Tree-of-Sophia owns philosophy meaning; this MCP packet is a Tree-of-Sophia standalone access aid.").len();
            let bytes = header.retained_storage_bytes().map_err(|_| budget())?
                .checked_add(lists.checked_mul(2 * std::mem::size_of::<&JsonValue>()).ok_or_else(budget)?)
                .and_then(|n| n.checked_add((13 + 13 + 2 + 2) * std::mem::size_of::<(JsonString, JsonValue)>()))
                .and_then(|n| n.checked_add(literals.checked_mul(1 + 2 * std::mem::size_of::<u16>())?)).ok_or_else(budget)?;
            heap.retain(bytes).map_err(compiler_query_error)?;
            model.charge_query_work(bytes).map_err(compiler_query_error)?;
            let packet = crate::philosophy_read::controlled_header_metadata(&header, request).ok_or_else(corrupt)?;
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
