//! The maintained native LensPlan driven by authenticated controlled rows.
use crate::controlled_query_adapter::compiler_query_error;
use crate::controlled_inspect_adapter::{collect_with_sizes, Charges};
use crate::controlled_lens_budget::ControlledLensOriginalBudget;
use crate::lens_plan::{LensPlan, LensNeed, LensReply, LensCandidate, LensCandidateCursor,
    LensCandidatePage};
use crate::knowledge_lens_spec::{LensVocabulary, normalize_lens_spec};
use crate::{BoundCmpKnowledge, InspectCurrentAuthority, ObservedInspectCarrier};
use crate::knowledge_lens::{LensBudget, LENS_OPERATION, LENS_INTENDED_USE};
use crate::search_v2::{SearchKind, SearchV2Error, SearchV2ErrorCode, SelectedQueryVocabulary};
use std::cell::RefCell;
use tos_compiler::{ControlledKnowledgeModel, ControlledCarrierSelection,
    ControlledLensKeySelection, ControlledSearchKind};
use tos_foundation::{JsonValue, JsonString, OwnedState};
fn budget() -> SearchV2Error { SearchV2Error { code: SearchV2ErrorCode::BudgetExceeded,
    message: "controlled Lens budget exceeded" } }
fn corrupt() -> SearchV2Error { SearchV2Error { code: SearchV2ErrorCode::CorruptSelectedCarrier,
    message: "controlled native Lens carrier differs" } }
fn kind(value: SearchKind) -> ControlledSearchKind { match value {
    SearchKind::Nodes => ControlledSearchKind::Nodes,
    SearchKind::Relations => ControlledSearchKind::Relations,
} }

pub fn execute_controlled_lens_response<'hold, A: InspectCurrentAuthority<'hold> + ?Sized>(
    model: &mut ControlledKnowledgeModel<'_, '_, '_>, bound: &BoundCmpKnowledge<'_>,
    authority: &mut A, spec: &JsonValue, limits: LensBudget,
    deliver: impl FnOnce(&[u8]) -> Result<(), SearchV2Error>,
) -> Result<(), SearchV2Error> {
    execute_controlled_lens_request_response(model, bound, authority,
        ControlledLensRequest::Compile(spec), limits, deliver)
}

#[derive(Clone, Copy)]
pub enum ControlledLensRequest<'a> {
    Compile(&'a JsonValue),
    Focus(&'a crate::knowledge_focus::KnowledgeFocusRequest),
    Stored(&'a str),
}

pub fn execute_controlled_lens_request_response<'hold, A: InspectCurrentAuthority<'hold> + ?Sized>(
    model: &mut ControlledKnowledgeModel<'_, '_, '_>, bound: &BoundCmpKnowledge<'_>,
    authority: &mut A, request: ControlledLensRequest<'_>, limits: LensBudget,
    deliver: impl FnOnce(&[u8]) -> Result<(), SearchV2Error>,
) -> Result<(), SearchV2Error> {
    let caps = limits.inspect;
    bound.check_controlled_model(model)?;
    model.check_query_open_vm_admission(caps.max_open_vm_steps).map_err(compiler_query_error)?;
    let metadata = authority.disclosure_metadata_state_upper_bound()?;
    let (operation, intended, input) = match request {
        ControlledLensRequest::Compile(spec) => (LENS_OPERATION, LENS_INTENDED_USE,
            spec.retained_storage_bytes().map_err(|_| budget())?),
        ControlledLensRequest::Focus(focus) => (crate::knowledge_lens::FOCUS_OPERATION,
            crate::knowledge_lens::FOCUS_INTENDED_USE,
            focus.retained_state_bytes().map_err(|_| budget())?),
        ControlledLensRequest::Stored(identifier) => {
            if identifier.is_empty() || identifier.chars().count() > 4096 {
                return Err(crate::knowledge_lens_spec::invalid("stored lens identifier is required and bounded"));
            }
            (crate::knowledge_lens::STORED_LENS_OPERATION,
                crate::knowledge_lens::STORED_LENS_INTENDED_USE, identifier.len())
        }
    };
    let frame = metadata.checked_add(input.checked_mul(3).ok_or_else(budget)?)
        .and_then(|n| n.checked_add(std::mem::size_of::<LensPlan<'_>>()
            + std::mem::size_of::<Charges>() + std::mem::size_of_val(&deliver)))
        .ok_or_else(budget)?;
    let mut result = Ok(());
    model.with_owned_query_workspace(frame, |model| {
        result = (|| {
            let policy = authority.policy_binding();
            let scope = authority.disclosure_scope();
            scope.validate_for(bound, &policy, operation, intended)?;
            let actual = policy.retained_state_bytes().map_err(|_| budget())?
                .checked_add(crate::knowledge_inspect::scope_owned_state(&scope)?).ok_or_else(budget)?;
            if actual > metadata { return Err(budget()); }
            authority.check_selected()?;
            let heap = RefCell::new(model.new_owned_query_heap());
            let mut header = None;
            let header_scan = model.with_controlled_header_scan(caps.max_payload_bytes, caps.max_decoded_bytes,
                caps.max_read_vm_steps, caps.json, |raw| {
                heap.borrow_mut().with_owned_query_json(raw, caps.json, |value, heap| {
                    let bytes = value.retained_storage_bytes().map_err(|_| tos_compiler::Error::Budget("Lens header state"))?;
                    heap.retain(bytes)?;
                    heap.charge_work(bytes)?;
                    header = Some(value.clone());
                    Ok(())
                })
            }).map_err(compiler_query_error)?;
            let header = header.ok_or_else(corrupt)?;
            // Match the existing RegexBuilder program/DFA ceilings. The same
            // original owner admits these before vocabulary compilation.
            let grammars = bound.descriptor().object_get("identity")
                .and_then(|v| v.object_get("shared_entity_id_grammars"))
                .and_then(JsonValue::as_array).ok_or_else(corrupt)?;
            let regex_state = grammars.len().checked_mul(2 * 1_048_576)
                .and_then(|n| n.checked_add(header.retained_storage_bytes().ok()?))
                .ok_or_else(budget)?;
            heap.borrow_mut().retain(regex_state).map_err(compiler_query_error)?;
            heap.borrow().charge_work(regex_state).map_err(compiler_query_error)?;
            let vocabulary = LensVocabulary::from_selected(bound, &header)?;
            let mut catalog_scan = None;
            let spec = match request {
                ControlledLensRequest::Compile(spec) => spec.clone(),
                ControlledLensRequest::Focus(focus) => {
                    // The fixed Focus constructor has fewer than 80 values/keys.
                    // Admit their frames, fixed literals, selected-source strings,
                    // and four uses of the identifier before its owned spec exists.
                    let sources = vocabulary.sources.iter().try_fold(0usize,
                        |n, s| n.checked_add(s.len()).ok_or_else(budget))?;
                    let generated = input.checked_mul(4)
                        .and_then(|n| n.checked_add(sources))
                        .and_then(|n| n.checked_add(1024))
                        .and_then(|n| n.checked_mul(1 + 2 * std::mem::size_of::<u16>()))
                        .and_then(|n| n.checked_add(80 * (std::mem::size_of::<JsonValue>()
                            + std::mem::size_of::<JsonString>())))
                        .ok_or_else(budget)?;
                    heap.borrow_mut().retain(generated).map_err(compiler_query_error)?;
                    heap.borrow().charge_work(generated).map_err(compiler_query_error)?;
                    crate::knowledge_focus::focus_lens_spec(focus, &vocabulary)?
                }
                ControlledLensRequest::Stored(identifier) => {
                    let mut catalog = None;
                    let mut authorizing = Ok(());
                    let scan = model.with_controlled_catalog_scan(caps.max_payload_bytes,
                        usize::try_from(caps.max_decoded_bytes.checked_sub(header_scan.decoded_bytes)
                            .ok_or_else(budget)?).map_err(|_| budget())?,
                        caps.max_read_vm_steps.checked_sub(header_scan.vm_steps).ok_or_else(budget)?,
                        caps.json, |raw, value| {
                        authorizing = (|| {
                            authority.authorize_catalog_current(bound,
                                bound.selection().catalog_packet_sha256,
                                bound.selection().catalog_index_root_sha256)?;
                            heap.borrow().charge_work(raw.len()).map_err(compiler_query_error)?;
                            bound.validate_catalog_identity(value, caps.json)?;
                            let bytes = value.retained_storage_bytes().map_err(|_| budget())?;
                            heap.borrow_mut().retain(bytes).map_err(compiler_query_error)?;
                            heap.borrow().charge_work(raw.len()).map_err(compiler_query_error)?;
                            catalog = Some(value.clone());
                            Ok(())
                        })();
                        Ok(())
                    }).map_err(compiler_query_error)?;
                    authorizing?;
                    catalog_scan = Some(scan);
                    let catalog = catalog.ok_or_else(corrupt)?;
                    heap.borrow_mut().retain(catalog.retained_storage_bytes().map_err(|_| budget())?)
                        .map_err(compiler_query_error)?;
                    crate::knowledge_lens_spec::stored_lens_spec(&catalog, identifier)?
                }
            };
            heap.borrow_mut().retain(spec.retained_storage_bytes().map_err(|_| budget())?
                .checked_mul(3).ok_or_else(budget)?).map_err(compiler_query_error)?;
            let public = normalize_lens_spec(&spec, &vocabulary)?;
            let public_state = public.retained_storage_bytes().map_err(|_| budget())?;
            heap.borrow_mut().retain(public_state.checked_mul(3).ok_or_else(budget)?)
                .map_err(compiler_query_error)?;
            let sources = public.object_get("sources").and_then(JsonValue::as_array)
                .ok_or_else(corrupt)?;
            let mut source_ids = sources.iter().map(|v| v.as_str().map(str::to_owned)
                .ok_or_else(corrupt)).collect::<Result<Vec<_>,_>>()?;
            source_ids.sort(); source_ids.dedup();
            let registered = bound.registered_source_ids();
            let registered_bytes = registered.iter().try_fold(
                registered.len().checked_mul(std::mem::size_of::<String>()).ok_or_else(budget)?,
                |n, value| n.checked_add(value.len()).ok_or_else(budget))?;
            heap.borrow_mut().retain(registered_bytes.checked_mul(2).ok_or_else(budget)?)
                .map_err(compiler_query_error)?;
            let mut all_source_ids = registered.to_vec();
            all_source_ids.sort(); all_source_ids.dedup();
            let geometry_json = heap.borrow().canonicalize_owned_query_json(
                &JsonValue::Array(all_source_ids.iter().map(|s| JsonValue::String(JsonString::from_utf8(s))).collect()),
                caps.json).map_err(compiler_query_error)?;
            heap.borrow_mut().retain(geometry_json.capacity()).map_err(compiler_query_error)?;
            let geometry_json = std::str::from_utf8(&geometry_json).map_err(|_| corrupt())?;
            let source_json = heap.borrow().canonicalize_owned_query_json(
                &JsonValue::Array(source_ids.iter().map(|s| JsonValue::String(JsonString::from_utf8(s))).collect()),
                caps.json).map_err(compiler_query_error)?;
            heap.borrow_mut().retain(source_json.capacity()).map_err(compiler_query_error)?;
            let source_json = std::str::from_utf8(&source_json).map_err(|_| corrupt())?;
            let mut charges = Charges { rows: header_scan.rows.checked_add(3).ok_or_else(budget)?,
                decoded: header_scan.decoded_bytes.checked_add(24).ok_or_else(budget)?, vm: header_scan.vm_steps };
            if let Some(scan) = catalog_scan {
                charges.rows = charges.rows.checked_add(scan.rows).ok_or_else(budget)?;
                charges.decoded = charges.decoded.checked_add(scan.decoded_bytes).ok_or_else(budget)?;
                charges.vm = charges.vm.checked_add(scan.vm_steps).ok_or_else(budget)?;
            }
            if charges.rows > caps.max_rows || charges.decoded > caps.max_decoded_bytes { return Err(budget()); }
            let (nodes, vm) = model.controlled_lens_scope_count(ControlledSearchKind::Nodes,
                source_json, caps.max_read_vm_steps.checked_sub(charges.vm).ok_or_else(budget)?).map_err(compiler_query_error)?;
            charges.vm = charges.vm.checked_add(vm).ok_or_else(budget)?;
            let (relations, vm) = model.controlled_lens_scope_count(ControlledSearchKind::Relations,
                source_json, caps.max_read_vm_steps.checked_sub(charges.vm).ok_or_else(budget)?)
                .map_err(compiler_query_error)?;
            charges.vm = charges.vm.checked_add(vm).ok_or_else(budget)?;
            let (id_length, vm) = model.controlled_lens_key_geometry(geometry_json,
                caps.max_read_vm_steps.checked_sub(charges.vm).ok_or_else(budget)?)
                .map_err(compiler_query_error)?;
            charges.vm = charges.vm.checked_add(vm).ok_or_else(budget)?;
            if id_length > caps.max_field_bytes { return Err(budget()); }
            // Actual selected ID geometry bounds thin candidate/adjacency/path
            // frames. Parsed-row clones are separately admitted by the shared
            // controlled collector before every resume; no full graph is loaded.
            let count = usize::try_from(nodes.checked_add(relations).ok_or_else(budget)?)
                .map_err(|_| budget())?.min(limits.max_candidates);
            let key_frame = id_length.checked_add(all_source_ids.iter().map(String::len).max().unwrap_or(0))
                .and_then(|n| n.checked_mul(4))
                .and_then(|n| n.checked_add(std::mem::size_of::<LensCandidate>()
                    + std::mem::size_of::<ObservedInspectCarrier>()
                    + 4 * std::mem::size_of::<String>())).ok_or_else(budget)?;
            let path_queries = public.object_get("path_query").and_then(JsonValue::as_array)
                .ok_or_else(corrupt)?;
            let path_frames = if path_queries.is_empty() { 0 } else {
                usize::try_from(nodes.checked_mul(relations).ok_or_else(budget)?)
                    .map_err(|_| budget())?.min(limits.max_path_steps)
            };
            let adjacent_frames = usize::try_from(relations).map_err(|_| budget())?
                .min(limits.max_adjacency_rows);
            let thin = count.checked_add(adjacent_frames)
                .and_then(|n| n.checked_add(path_frames))
                .and_then(|n| n.checked_mul(key_frame))
                .and_then(|n| n.checked_add(public_state.checked_mul(3)?))
                .ok_or_else(budget)?;
            let publication_keys = concat!("schema", "tos_selected_lens_continuation_v1",
                "operation_id", "carrier_layer", "intended_use", "selected_model_receipt_id",
                "source_cut", "through_commit_seq", "source_membership_root", "descriptor_sha256",
                "selected_index_sha256", "catalog_packet_sha256", "catalog_index_root_sha256",
                "policy_issuer_ref", "policy_receipt_id", "policy_scope", "policy_epoch",
                "withdrawal_generation").len();
            let publication_state = crate::knowledge_inspect::scope_owned_state(&scope)?
                .checked_add(publication_keys + 5 * 64 + 20)
                .and_then(|n| n.checked_mul(1 + 2 * std::mem::size_of::<u16>()))
                .and_then(|n| n.checked_add(2 * 17 * std::mem::size_of::<(JsonString,JsonValue)>()))
                .ok_or_else(budget)?;
            heap.borrow_mut().retain(publication_state).map_err(compiler_query_error)?;
            let publication = crate::knowledge_lens::lens_continuation_binding(bound, &scope);
            let guard = ControlledLensOriginalBudget { heap: &heap };
            let mut plan = LensPlan::native_with_original(public, vocabulary,
                bound.require_source_revision()?, header.object_get("authority_boundary")
                    .cloned().ok_or_else(corrupt)?, publication, limits, (nodes,relations),
                authority.abort_probe(), &guard, thin)?;
            let mut consulted = Vec::new();
            while !plan.advance()? {
                authority.check_selected()?;
                model.check_pin().map_err(compiler_query_error)?;
                let need = plan.need().ok_or_else(corrupt)?;
                let reply = match &*need {
                    LensNeed::ExactRows { kind, ids, .. } => {
                        let mut values = Vec::new();
                        for id in ids { values.extend(collect_with_sizes(model, authority, &all_source_ids,
                            *kind, ControlledCarrierSelection::Id { identifier: id, limit: 1 },
                            caps, &mut charges, &mut consulted, &mut heap.borrow_mut())?); }
                        let (rows,raw_bytes) = values.into_iter().unzip();
                        LensReply::Rows {rows,raw_bytes}
                    }
                    LensNeed::LookupRows { field, identifier, limit } => {
                        let selected = match field.as_str() {
                            "id" => ControlledCarrierSelection::Id { identifier, limit: *limit },
                            "native_id" => ControlledCarrierSelection::NativeId { identifier, limit: *limit },
                            "entity_id" => ControlledCarrierSelection::EntityId { identifier, limit: *limit },
                            _ => return Err(corrupt()),
                        };
                        let (rows,raw_bytes) = collect_with_sizes(model, authority, &all_source_ids,
                            SearchKind::Nodes, selected, caps, &mut charges, &mut consulted,
                            &mut heap.borrow_mut())?.into_iter().unzip();
                        LensReply::Rows {rows,raw_bytes}
                    }
                    LensNeed::CandidateIds { kind: selected_kind, sources, after, limit, .. } => {
                        let (source,position) = match after { None => ("",-1),
                            Some(LensCandidateCursor::SourceOrder {source,position}) => (source.as_str(),*position),
                            _ => return Err(corrupt()) };
                        if sources.len() != source_ids.len() || sources.iter().any(|s| !source_ids.contains(s)) { return Err(corrupt()); }
                        let mut keys = Vec::new();
                        let scan = model.with_controlled_lens_keys(ControlledLensKeySelection::Candidates {
                            kind: kind(*selected_kind), sources_json: source_json,
                            after_source: source, after_position: position }, *limit, caps.max_field_bytes,
                            caps.max_decoded_bytes.checked_sub(charges.decoded).ok_or_else(budget)?,
                            caps.max_read_vm_steps.checked_sub(charges.vm).ok_or_else(budget)?, |rows| {
                                for row in rows { keys.push(LensCandidate { id: row.id.clone(),
                                    source: Some(row.source_graph.clone()), position: Some(i64::try_from(row.position)
                                        .map_err(|_| corrupt())?) }); }
                                Ok::<_,SearchV2Error>(())
                            }).map_err(compiler_query_error)??;
                        charges.rows = charges.rows.checked_add(scan.rows).ok_or_else(budget)?;
                        charges.decoded = charges.decoded.checked_add(scan.decoded_bytes).ok_or_else(budget)?;
                        charges.vm = charges.vm.checked_add(scan.vm_steps).ok_or_else(budget)?;
                        LensReply::Candidates(LensCandidatePage {rows:keys})
                    }
                    LensNeed::IdentityIds { identifier, sources, after, limit } => {
                        if sources.len() != source_ids.len() || sources.iter().any(|s| !source_ids.contains(s)) { return Err(corrupt()); }
                        let mut ids = Vec::new();
                        let scan = model.with_controlled_lens_keys(ControlledLensKeySelection::Identity {
                            identifier, sources_json: source_json, after }, *limit, caps.max_field_bytes,
                            caps.max_decoded_bytes.checked_sub(charges.decoded).ok_or_else(budget)?,
                            caps.max_read_vm_steps.checked_sub(charges.vm).ok_or_else(budget)?,
                            |rows| { ids.extend(rows.iter().map(|r|r.id.clone())); Ok::<_,SearchV2Error>(()) })
                            .map_err(compiler_query_error)??;
                        charges.rows = charges.rows.checked_add(scan.rows).ok_or_else(budget)?;
                        charges.decoded = charges.decoded.checked_add(scan.decoded_bytes).ok_or_else(budget)?;
                        charges.vm = charges.vm.checked_add(scan.vm_steps).ok_or_else(budget)?;
                        LensReply::Ids(ids)
                    }
                    LensNeed::IncidentIds { identifier, after, limit } => {
                        let mut ids = Vec::new();
                        let scan = model.with_controlled_lens_keys(ControlledLensKeySelection::Incident {
                            identifier, after }, *limit, caps.max_field_bytes,
                            caps.max_decoded_bytes.checked_sub(charges.decoded).ok_or_else(budget)?,
                            caps.max_read_vm_steps.checked_sub(charges.vm).ok_or_else(budget)?,
                            |rows| { ids.extend(rows.iter().map(|r|r.id.clone())); Ok::<_,SearchV2Error>(()) })
                            .map_err(compiler_query_error)??;
                        charges.rows = charges.rows.checked_add(scan.rows).ok_or_else(budget)?;
                        charges.decoded = charges.decoded.checked_add(scan.decoded_bytes).ok_or_else(budget)?;
                        charges.vm = charges.vm.checked_add(scan.vm_steps).ok_or_else(budget)?;
                        LensReply::Ids(ids)
                    }
                    _ => return Err(corrupt()),
                };
                if charges.rows > caps.max_rows { return Err(budget()); }
                drop(need);
                plan.resume(reply)?;
            }
            let packet = plan.finish()?;
            let body = heap.borrow().canonicalize_owned_query_json(&packet,caps.json)
                .map_err(compiler_query_error)?;
            if body.len() > caps.max_response_bytes { return Err(budget()); }
            heap.borrow_mut().retain(body.capacity()).map_err(compiler_query_error)?;
            authority.check_selected()?;
            let mut lease = authority.acquire_disclosure(&scope,&consulted)?;
            lease.recheck()?; model.check_pin().map_err(compiler_query_error)?;
            deliver(&body)?;
            lease.recheck()?; model.check_pin().map_err(compiler_query_error)
        })();
        Ok(())
    }).map_err(compiler_query_error)?;
    result
}
