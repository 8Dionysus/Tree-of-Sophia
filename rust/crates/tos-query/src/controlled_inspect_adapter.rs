//! The maintained InspectPlan driven by authentic controlled carrier rows.
//! No connection, model constructor, source path or independent clock enters.
use crate::controlled_query_adapter::compiler_query_error;
use crate::search_v2::{SearchKind, SearchV2Error, SearchV2ErrorCode, SelectedQueryVocabulary};
use crate::{
    BoundCmpKnowledge, InspectBudget, InspectCurrentAuthority, InspectNeed, InspectPlan,
    InspectRequest, ObservedInspectCarrier,
};
use tos_compiler::{
    ControlledCarrierSelection, ControlledKnowledgeModel, ControlledQueryHeap, ControlledSearchKind,
};
use tos_foundation::{JsonString, JsonValue, OwnedState};

fn budget() -> SearchV2Error {
    SearchV2Error {
        code: SearchV2ErrorCode::BudgetExceeded,
        message: "controlled inspect budget exceeded",
    }
}
fn corrupt() -> SearchV2Error {
    SearchV2Error {
        code: SearchV2ErrorCode::CorruptSelectedCarrier,
        message: "controlled inspect selected carrier differs",
    }
}
#[derive(Default)]
struct Charges {
    rows: u64,
    decoded: u64,
    vm: u64,
}

fn collect<'hold, 'model, 'state, 'budget, A: InspectCurrentAuthority<'hold> + ?Sized>(
    model: &mut ControlledKnowledgeModel<'model, 'state, 'budget>,
    authority: &mut A,
    sources: &[String],
    kind: SearchKind,
    selected: ControlledCarrierSelection<'_>,
    caps: InspectBudget,
    charges: &mut Charges,
    consulted: &mut Vec<ObservedInspectCarrier>,
    heap: &mut ControlledQueryHeap<'model, 'state, 'budget>,
) -> Result<Vec<JsonValue>, SearchV2Error> {
    let mut values = Vec::new();
    let rows = caps.max_rows.checked_sub(charges.rows).ok_or_else(budget)?;
    let decoded = caps
        .max_decoded_bytes
        .checked_sub(charges.decoded)
        .ok_or_else(budget)?;
    let vm = caps
        .max_read_vm_steps
        .checked_sub(charges.vm)
        .ok_or_else(budget)?;
    let scan = model
        .visit_controlled_carrier_rows(
            match kind {
                SearchKind::Nodes => ControlledSearchKind::Nodes,
                SearchKind::Relations => ControlledSearchKind::Relations,
            },
            sources,
            selected,
            rows,
            decoded,
            vm,
            caps.max_payload_bytes,
            caps.max_field_bytes,
            caps.json,
            heap,
            |source_graph, position, payload_sha256, value, charge, heap| {
                authority.check_selected()?;
                let id = value
                    .object_get("id")
                    .and_then(JsonValue::as_str)
                    .ok_or_else(corrupt)?;
                let payload_state = value.retained_storage_bytes().map_err(|_| budget())?;
                let authorization = heap
                    .with_temporary(
                        payload_state
                            .checked_add(id.len())
                            .and_then(|n| {
                                n.checked_add(std::mem::size_of::<crate::InspectedCarrier>())
                            })
                            .ok_or_else(budget)?,
                        || {
                            authority.authorize_current_borrowed(
                                kind,
                                id,
                                position,
                                payload_sha256,
                                value,
                            )
                        },
                    )
                    .map_err(compiler_query_error)?;
                authorization?;
                // The domain plan clones its collected items once and projects at
                // most one source target per row (<=32 fixed JSON field slots).
                // Reserve those actual row geometries before cloning any item.
                let retained = payload_state
                    .checked_mul(4)
                    .and_then(|n| {
                        n.checked_add(32 * std::mem::size_of::<(JsonString, JsonValue)>())
                    })
                    .and_then(|n| n.checked_add(id.len() + source_graph.len()))
                    .and_then(|n| {
                        n.checked_add(
                            std::mem::size_of::<ObservedInspectCarrier>()
                                + std::mem::size_of::<JsonValue>(),
                        )
                    })
                    .ok_or_else(budget)?;
                heap.retain(retained).map_err(compiler_query_error)?;
                consulted.try_reserve_exact(1).map_err(|_| budget())?;
                values.try_reserve_exact(1).map_err(|_| budget())?;
                consulted.push(ObservedInspectCarrier {
                    kind,
                    id: id.to_owned(),
                    source_graph: source_graph.to_owned(),
                    position,
                    payload_sha256,
                });
                charge(
                    payload_state
                        .checked_add(id.len() + source_graph.len())
                        .ok_or_else(budget)?,
                )
                .map_err(compiler_query_error)?;
                values.push(value.clone());
                Ok::<(), SearchV2Error>(())
            },
        )
        .map_err(compiler_query_error)??;
    charges.rows = charges.rows.checked_add(scan.rows).ok_or_else(budget)?;
    charges.decoded = charges
        .decoded
        .checked_add(scan.decoded_bytes)
        .ok_or_else(budget)?;
    charges.vm = charges.vm.checked_add(scan.vm_steps).ok_or_else(budget)?;
    Ok(values)
}

/// Deliver a complete node/relation packet with exact aliases, complete
/// incident total, source targets, and the maintained Inspect disclosure lease.
pub fn execute_controlled_inspect_response<'hold, A: InspectCurrentAuthority<'hold> + ?Sized>(
    model: &mut ControlledKnowledgeModel<'_, '_, '_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    kind: SearchKind,
    identifier: &str,
    relation_limit: usize,
    caps: InspectBudget,
    deliver: impl FnOnce(&[u8]) -> Result<(), SearchV2Error>,
) -> Result<(), SearchV2Error> {
    if caps.max_rows == 0
        || caps.max_matches == 0
        || caps.max_read_vm_steps == 0
        || caps.max_response_bytes == 0
    {
        return Err(budget());
    }
    model
        .charge_query_work(identifier.len())
        .map_err(compiler_query_error)?;
    bound.check_controlled_model(model)?;
    model
        .check_query_open_vm_admission(caps.max_open_vm_steps)
        .map_err(compiler_query_error)?;
    let revision = bound.require_source_revision()?;
    let source_bytes = bound
        .registered_source_ids()
        .iter()
        .try_fold(0usize, |n, value| {
            n.checked_add(value.len()).ok_or_else(budget)
        })?;
    let metadata = authority.disclosure_metadata_state_upper_bound()?;
    let workspace = metadata
        .checked_add(caps.max_response_bytes)
        .and_then(|n| n.checked_add(identifier.len() + revision.len()))
        .and_then(|n| n.checked_add(source_bytes.checked_mul(10)?))
        .and_then(|n| {
            n.checked_add(bound.registered_source_ids().len().checked_mul(
                std::mem::size_of::<String>()
                    + std::mem::size_of::<JsonValue>()
                    + 2 * std::mem::size_of::<usize>(),
            )?)
        })
        .and_then(|n| {
            n.checked_add(
                std::mem::size_of::<InspectPlan>()
                    + std::mem::size_of::<InspectRequest>()
                    + std::mem::size_of::<Charges>()
                    + 64 * std::mem::size_of::<(JsonString, JsonValue)>()
                    + std::mem::size_of_val(&deliver),
            )
        })
        .ok_or_else(budget)?;
    let mut outcome = Ok(());
    model
        .with_owned_query_workspace(workspace, |model| {
            outcome = (|| {
                let policy = authority.policy_binding();
                let scope = authority.disclosure_scope();
                let operation = if kind == SearchKind::Nodes {
                    crate::NODE_INSPECT_OPERATION
                } else {
                    crate::RELATION_INSPECT_OPERATION
                };
                scope.validate_for(bound, &policy, operation, crate::INSPECT_INTENDED_USE)?;
                authority.check_selected()?;
                let mut heap = model.new_owned_query_heap();
                let request = InspectRequest::new(kind, identifier, relation_limit, caps)?;
                let mut plan = None;
                model
                    .with_owned_query_json(
                        bound.authority_boundary().as_bytes(),
                        caps.json,
                        |boundary| {
                            let retained = boundary.retained_storage_bytes().map_err(|_| {
                                tos_compiler::Error::Budget("controlled inspect boundary state")
                            })?;
                            heap.retain(retained)?;
                            plan = Some(InspectPlan::new(
                                request,
                                revision.to_owned(),
                                boundary.clone(),
                                caps,
                            ));
                            Ok(())
                        },
                    )
                    .map_err(compiler_query_error)?;
                let mut plan = plan.ok_or_else(corrupt)??;
                let mut sources = bound.registered_source_ids().to_vec();
                sources.sort();
                sources.dedup();
                let mut consulted = Vec::new();
                let mut charges = Charges::default();
                while let Some(need) = plan.need() {
                    let need_state = match need {
                        InspectNeed::Lookup { identifier, .. } => identifier.len(),
                        InspectNeed::NodeIncident { ids, .. }
                        | InspectNeed::RelationEndpoints { ids } => ids.iter().try_fold(
                            ids.len()
                                .checked_mul(std::mem::size_of::<String>())
                                .ok_or_else(budget)?,
                            |n, id| n.checked_add(id.len()).ok_or_else(budget),
                        )?,
                    };
                    heap.retain(need_state.checked_mul(4).ok_or_else(budget)?)
                        .map_err(compiler_query_error)?;
                    model
                        .charge_query_work(need_state)
                        .map_err(compiler_query_error)?;
                    let need = need.clone();
                    let probe = authority.abort_probe();
                    let before = charges.decoded;
                    match need {
                        InspectNeed::Lookup {
                            kind,
                            selector,
                            identifier,
                            limit,
                        } => {
                            let limit = limit.checked_add(1).ok_or_else(budget)?;
                            let selected = match selector {
                                "id" => ControlledCarrierSelection::Id {
                                    identifier: &identifier,
                                    limit,
                                },
                                "native_id" => ControlledCarrierSelection::NativeId {
                                    identifier: &identifier,
                                    limit,
                                },
                                "entity_id" => ControlledCarrierSelection::EntityId {
                                    identifier: &identifier,
                                    limit,
                                },
                                _ => return Err(corrupt()),
                            };
                            let rows = collect(
                                model,
                                authority,
                                &sources,
                                kind,
                                selected,
                                caps,
                                &mut charges,
                                &mut consulted,
                                &mut heap,
                            )?;
                            plan.resume_lookup(rows, charges.decoded - before, probe.as_deref())?;
                        }
                        InspectNeed::NodeIncident {
                            ids,
                            relation_limit,
                        } => {
                            let ids = JsonValue::Array(
                                ids.into_iter()
                                    .map(|id| JsonValue::String(JsonString::from_utf8(&id)))
                                    .collect(),
                            );
                            let encoded = heap
                                .canonicalize_owned_query_json(&ids, caps.json)
                                .map_err(compiler_query_error)?;
                            let encoded = std::str::from_utf8(&encoded).map_err(|_| corrupt())?;
                            let (total, vm) = model
                                .controlled_incident_count(
                                    encoded,
                                    caps.max_read_vm_steps
                                        .checked_sub(charges.vm)
                                        .ok_or_else(budget)?,
                                )
                                .map_err(compiler_query_error)?;
                            charges.vm = charges.vm.checked_add(vm).ok_or_else(budget)?;
                            charges.decoded = charges.decoded.checked_add(8).ok_or_else(budget)?;
                            let rows = collect(
                                model,
                                authority,
                                &sources,
                                SearchKind::Relations,
                                ControlledCarrierSelection::Incident {
                                    ids_json: encoded,
                                    limit: relation_limit,
                                },
                                caps,
                                &mut charges,
                                &mut consulted,
                                &mut heap,
                            )?;
                            plan.resume_incident(
                                total,
                                rows,
                                charges.decoded - before,
                                probe.as_deref(),
                            )?;
                        }
                        InspectNeed::RelationEndpoints { ids } => {
                            let mut rows = Vec::new();
                            for id in ids {
                                let mut selected = collect(
                                    model,
                                    authority,
                                    &sources,
                                    SearchKind::Nodes,
                                    ControlledCarrierSelection::Id {
                                        identifier: &id,
                                        limit: 1,
                                    },
                                    caps,
                                    &mut charges,
                                    &mut consulted,
                                    &mut heap,
                                )?;
                                heap.retain(
                                    selected
                                        .len()
                                        .checked_mul(std::mem::size_of::<JsonValue>())
                                        .ok_or_else(budget)?,
                                )
                                .map_err(compiler_query_error)?;
                                rows.try_reserve_exact(selected.len())
                                    .map_err(|_| budget())?;
                                rows.append(&mut selected);
                            }
                            plan.resume_endpoints(
                                rows,
                                charges.decoded - before,
                                probe.as_deref(),
                            )?;
                        }
                    }
                    authority.check_selected()?;
                }
                let packet = plan.into_packet_with_targets(|items, identity, managed| {
                    crate::source_read_projection::controlled_source_read_targets(
                        items,
                        identity,
                        managed,
                        caps.json,
                        |record, limits| {
                            let raw = heap
                                .canonicalize_owned_query_json(record, limits)
                                .map_err(compiler_query_error)?;
                            model
                                .charge_query_work(raw.len())
                                .map_err(compiler_query_error)?;
                            Ok(raw)
                        },
                    )
                })?;
                let body = heap
                    .canonicalize_owned_query_json(&packet, caps.json)
                    .map_err(compiler_query_error)?;
                if body.len() > caps.max_response_bytes {
                    return Err(budget());
                }
                bound.check_controlled_model(model)?;
                authority.check_selected()?;
                let mut lease = authority.acquire_disclosure(&scope, &consulted)?;
                lease.recheck()?;
                deliver(&body)?;
                lease.recheck()
            })();
            Ok(())
        })
        .map_err(compiler_query_error)?;
    outcome
}
