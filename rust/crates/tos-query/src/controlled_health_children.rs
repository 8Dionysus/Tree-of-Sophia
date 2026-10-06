//! Genuine native health child packets over the same controlled Original owner.
use crate::controlled_query_adapter::{compiler_query_error, execute_controlled_catalog_payload_response};
use crate::controlled_original_reader::{read_original, OriginalReadCharges};
use crate::{BoundCmpKnowledge, InspectCurrentAuthority, InspectBudget};
use crate::search_v2::{SearchV2Error, SearchV2ErrorCode};
use tos_compiler::{ControlledKnowledgeModel, ControlledOriginalCollection,
    CorpusOriginalCollection, PhilosophyOriginalCollection};
use tos_foundation::{JsonValue, OwnedState};
fn budget() -> SearchV2Error { SearchV2Error::new(SearchV2ErrorCode::BudgetExceeded,
    "controlled health child budget exceeded") }
fn corrupt() -> SearchV2Error { SearchV2Error::new(SearchV2ErrorCode::CorruptSelectedCarrier,
    "controlled health Original binding differs") }

#[derive(Clone, Copy)]
pub enum ControlledHealthSeed { Corpus, Philosophy }

pub fn execute_controlled_health_seed_response<'hold, A: InspectCurrentAuthority<'hold> + ?Sized>(
    model: &mut ControlledKnowledgeModel<'_, '_, '_>, bound: &BoundCmpKnowledge<'_>,
    authority: &mut A, caps: InspectBudget, seed: ControlledHealthSeed,
    deliver: impl FnOnce(&[u8]) -> Result<(), SearchV2Error>,
) -> Result<(), SearchV2Error> {
    bound.check_controlled_model(model)?;
    model.check_query_open_vm_admission(caps.max_open_vm_steps).map_err(compiler_query_error)?;
    let metadata = authority.disclosure_metadata_state_upper_bound()?;
    let mut result = Ok(());
    model.with_owned_query_workspace(metadata.checked_add(std::mem::size_of_val(&deliver))
        .ok_or(tos_compiler::Error::Budget("health seed frame")).map_err(compiler_query_error)?, |model| {
        result = (|| {
            let policy = authority.policy_binding(); let scope = authority.disclosure_scope();
            let (operation, intended, collection) = match seed {
                ControlledHealthSeed::Corpus => {
                    let r = model.corpus_original_receipt().ok_or_else(corrupt)?;
                    if r.descriptor_sha256 != bound.selection().vocabulary.descriptor_sha256.to_hex()
                        || r.source_cut != bound.selection().source_cut
                        || r.membership_root != bound.selection().source_membership_root.to_hex() { return Err(corrupt()); }
                    ("tos_corpus_status", crate::corpus_read::CORPUS_INTENDED_USE,
                        ControlledOriginalCollection::Corpus(CorpusOriginalCollection::Header))
                }
                ControlledHealthSeed::Philosophy => {
                    let r = model.philosophy_original_receipt().ok_or_else(corrupt)?;
                    if r.descriptor_sha256 != bound.selection().vocabulary.descriptor_sha256.to_hex()
                        || r.source_cut != bound.selection().source_cut
                        || r.membership_root != bound.selection().source_membership_root.to_hex()
                        || r.source_graph != bound.source_for_adapter("philosophy-node-edge-v1").ok_or_else(corrupt)? { return Err(corrupt()); }
                    ("tos_philosophy_graph_views", crate::philosophy_read::PHILOSOPHY_INTENDED_USE,
                        ControlledOriginalCollection::Philosophy(PhilosophyOriginalCollection::Header))
                }
            };
            scope.validate_for(bound, &policy, operation, intended)?;
            if policy.retained_state_bytes().map_err(|_| budget())?
                .checked_add(crate::knowledge_inspect::scope_owned_state(&scope)?).ok_or_else(budget)? > metadata { return Err(budget()); }
            authority.check_selected()?;
            let mut heap = model.new_owned_query_heap(); let mut charges = OriginalReadCharges::default();
            let (ordinal, header) = read_original(model, authority, collection, -1,
                caps, &mut charges, &mut heap)?.ok_or_else(corrupt)?;
            if ordinal != 0 || header.as_object().is_none() { return Err(corrupt()); }
            // Header-derived strings, first-view identity and fixed seed envelope.
            let state = header.retained_storage_bytes().map_err(|_| budget())?
                .checked_mul(4).and_then(|n| n.checked_add(16 * std::mem::size_of::<JsonValue>() + 1024))
                .ok_or_else(budget)?;
            heap.retain(state).map_err(compiler_query_error)?;
            heap.charge_work(state).map_err(compiler_query_error)?;
            let packet = match seed {
                ControlledHealthSeed::Corpus => {
                    let expected = model.corpus_original_receipt().ok_or_else(corrupt)?.collections
                        .iter().find(|c| c.collection == "graph_views").ok_or_else(corrupt)?.rows;
                    let mut views = Vec::new(); let mut after = -1;
                    for expected_ordinal in 0..expected {
                        let (ordinal, row) = read_original(model, authority,
                            ControlledOriginalCollection::Corpus(CorpusOriginalCollection::GraphViews),
                            after, caps, &mut charges, &mut heap)?.ok_or_else(corrupt)?;
                        if u64::try_from(ordinal).map_err(|_| corrupt())? != expected_ordinal { return Err(corrupt()); }
                        after = ordinal;
                        heap.retain(2 * std::mem::size_of::<JsonValue>()).map_err(compiler_query_error)?;
                        views.push(row);
                    }
                    crate::corpus_read::controlled_health_seed(&header, &views, caps.max_field_bytes)?
                }
                ControlledHealthSeed::Philosophy => {
                    let mut interrupt = || { heap.check().map_err(compiler_query_error)?;
                        heap.charge_work(1).map_err(compiler_query_error) };
                    crate::philosophy_read::controlled_health_seed(&header, caps.max_field_bytes,
                        caps.max_read_vm_steps, &mut interrupt)?
                }
            };
            let body = heap.canonicalize_owned_query_json(&packet, caps.json).map_err(compiler_query_error)?;
            heap.retain(body.capacity()).map_err(compiler_query_error)?;
            if body.len() > caps.max_response_bytes { return Err(budget()); }
            let mut lease = authority.acquire_disclosure(&scope, &[])?;
            lease.recheck()?; model.check_pin().map_err(compiler_query_error)?;
            deliver(&body)?;
            lease.recheck()?; model.check_pin().map_err(compiler_query_error)
        })(); Ok(())
    }).map_err(compiler_query_error)?;
    result
}

pub fn execute_controlled_knowledge_health_response<'hold, A>(
    model: &mut ControlledKnowledgeModel<'_, '_, '_>, bound: &BoundCmpKnowledge<'_>,
    authority: &mut A, caps: InspectBudget,
    deliver: impl FnOnce(&[u8]) -> Result<(), crate::CatalogError>,
) -> Result<(), crate::CatalogError>
where A: crate::CatalogCurrentAuthority<'hold> + InspectCurrentAuthority<'hold> {
    use crate::{CatalogError, CatalogErrorCode};
    let failed = || CatalogError { code: CatalogErrorCode::BudgetExceeded,
        message: "controlled health metadata owner refused" };
    let map = |e| CatalogError { code: match e {
        tos_compiler::Error::Budget(_) | tos_compiler::Error::SqliteVmBudget {..} => CatalogErrorCode::BudgetExceeded,
        tos_compiler::Error::Invalid(_) => CatalogErrorCode::CorruptSelectedCarrier,
        _ => CatalogErrorCode::PolicyBindingUnavailable,
    }, message: "controlled health metadata owner refused" };
    bound.check_controlled_model(model).map_err(|_| failed())?;
    model.check_query_open_vm_admission(caps.max_open_vm_steps).map_err(map)?;
    let mut heap = model.new_owned_query_heap(); let mut header = None;
    let scan = model.with_controlled_header_scan(caps.max_payload_bytes, caps.max_decoded_bytes,
        caps.max_read_vm_steps, caps.json, |raw| {
        heap.with_owned_query_json(raw, caps.json, |value, heap| {
            let state = value.retained_storage_bytes().map_err(|_| tos_compiler::Error::Budget("health header state"))?;
            heap.retain(state)?; heap.charge_work(state)?; header = Some(value.clone()); Ok(())
        })
    }).map_err(map)?;
    let mut header = header.ok_or_else(failed)?;
    let budget = crate::CatalogBudget { max_open_vm_steps: caps.max_open_vm_steps,
        max_read_vm_steps: caps.max_read_vm_steps.checked_sub(scan.vm_steps).ok_or_else(failed)?,
        max_packet_bytes: caps.max_response_bytes,
        max_decoded_bytes: usize::try_from(caps.max_decoded_bytes.checked_sub(scan.decoded_bytes)
            .ok_or_else(failed)?).map_err(|_| failed())?, json: caps.json };
    execute_controlled_catalog_payload_response(model, bound, authority, budget, |_, catalog| {
        let state = header.retained_storage_bytes().map_err(|_| failed())?
            .checked_add(catalog.retained_storage_bytes().map_err(|_| failed())?)
            .and_then(|n| n.checked_mul(4)).and_then(|n| n.checked_add(64 * std::mem::size_of::<JsonValue>() + 4096))
            .ok_or_else(failed)?;
        heap.retain(state).map_err(map)?; heap.charge_work(state).map_err(map)?;
        let packet = crate::knowledge_catalog::controlled_knowledge_health_metadata(bound, &mut header, catalog, budget)?;
        let body = heap.canonicalize_owned_query_json(&packet, caps.json).map_err(map)?;
        heap.retain(body.capacity()).map_err(map)?;
        if body.len() > caps.max_response_bytes { return Err(failed()); }
        deliver(&body)
    })
}
