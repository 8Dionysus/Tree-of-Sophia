//! Exact three-Original reads for maintained domain kernels. Custody remains
//! with the cold owner; authorization remains with the live Inspect owner.
use crate::controlled_query_adapter::compiler_query_error;
use crate::{InspectBudget, InspectCurrentAuthority};
use crate::search_v2::{SearchV2Error, SearchV2ErrorCode};
use tos_compiler::{ControlledKnowledgeModel, ControlledOriginalCollection,
    ControlledOriginalReceipt, ControlledQueryHeap};
use tos_foundation::JsonValue;

fn budget() -> SearchV2Error {
    SearchV2Error { code: SearchV2ErrorCode::BudgetExceeded,
        message: "controlled Original read budget exceeded" }
}
fn corrupt() -> SearchV2Error {
    SearchV2Error { code: SearchV2ErrorCode::CorruptSelectedCarrier,
        message: "controlled Original namespace differs" }
}

/// Per-operation windows over the original aggregate ledgers. These values
/// only narrow a query; they cannot reset or replenish its original owners.
#[derive(Default)]
pub(crate) struct OriginalReadCharges {
    pub rows: u64,
    pub decoded: u64,
    pub vm: u64,
}

pub(crate) fn read_original<'hold, 'model, 'state, 'budget,
    A: InspectCurrentAuthority<'hold> + ?Sized>(
    model: &mut ControlledKnowledgeModel<'model, 'state, 'budget>,
    authority: &mut A,
    collection: ControlledOriginalCollection,
    after: i64,
    caps: InspectBudget,
    charges: &mut OriginalReadCharges,
    heap: &mut ControlledQueryHeap<'model, 'state, 'budget>,
) -> Result<Option<(i64, JsonValue)>, SearchV2Error> {
    read_original_selected(model, authority, collection, None, after, caps, charges, heap)
}

pub(crate) fn read_corpus_selected_original<'hold, 'model, 'state, 'budget,
    A: InspectCurrentAuthority<'hold> + ?Sized>(
    model: &mut ControlledKnowledgeModel<'model, 'state, 'budget>, authority: &mut A,
    collection: tos_compiler::CorpusOriginalCollection,
    selector: &tos_compiler::CorpusOriginalSelector, after: i64, caps: InspectBudget,
    charges: &mut OriginalReadCharges,
    heap: &mut ControlledQueryHeap<'model, 'state, 'budget>,
) -> Result<Option<(i64, JsonValue)>, SearchV2Error> {
    read_original_selected(model, authority, ControlledOriginalCollection::Corpus(collection),
        Some(selector), after, caps, charges, heap)
}

fn read_original_selected<'hold, 'model, 'state, 'budget,
    A: InspectCurrentAuthority<'hold> + ?Sized>(
    model: &mut ControlledKnowledgeModel<'model, 'state, 'budget>,
    authority: &mut A,
    collection: ControlledOriginalCollection,
    selector: Option<&tos_compiler::CorpusOriginalSelector>,
    after: i64,
    caps: InspectBudget,
    charges: &mut OriginalReadCharges,
    heap: &mut ControlledQueryHeap<'model, 'state, 'budget>,
) -> Result<Option<(i64, JsonValue)>, SearchV2Error> {
    authority.check_selected()?;
    if charges.rows >= caps.max_rows { return Err(budget()); }
    let decoded = caps.max_decoded_bytes.checked_sub(charges.decoded).ok_or_else(budget)?;
    let vm = caps.max_read_vm_steps.checked_sub(charges.vm).ok_or_else(budget)?;
    let mut output = None;
    let mut failure = None;
    let consume = |receipt, ordinal, digest, raw: &[u8], value: &JsonValue| {
            let result = (|| {
                authority.check_selected()?;
                match (receipt, collection) {
                    (ControlledOriginalReceipt::Navigation(receipt), ControlledOriginalCollection::Navigation) =>
                        authority.authorize_navigation_original_current(receipt, ordinal, raw, digest)?,
                    (ControlledOriginalReceipt::Philosophy(receipt), ControlledOriginalCollection::Philosophy(collection)) =>
                        authority.authorize_philosophy_original_current(receipt, collection,
                            u64::try_from(ordinal).map_err(|_| corrupt())?, raw, digest)?,
                    (ControlledOriginalReceipt::Corpus(receipt), ControlledOriginalCollection::Corpus(collection)) =>
                        authority.authorize_corpus_original_current(receipt, collection,
                            u64::try_from(ordinal).map_err(|_| corrupt())?, raw, digest)?,
                    _ => return Err(corrupt()),
                }
                authority.check_selected()?;
                let retained = value.retained_storage_bytes().map_err(|_| budget())?
                    .checked_add(std::mem::size_of::<(i64, JsonValue)>()).ok_or_else(budget)?;
                // The owner loan holds the source and parser; this separately
                // admits the one retained domain input copy before cloning it.
                heap.retain(retained).map_err(compiler_query_error)?;
                heap.charge_work(raw.len()).map_err(compiler_query_error)?;
                Ok((ordinal, value.clone()))
            })();
            match result { Ok(row) => output = Some(row), Err(error) => failure = Some(error) }
            Ok(())
        };
    let scan = match (collection, selector) {
        (ControlledOriginalCollection::Corpus(collection), Some(selector)) =>
            model.with_controlled_corpus_selected_row_receipt(collection, selector, after,
                caps.max_payload_bytes, decoded, vm, caps.json, consume),
        (_, None) => model.with_controlled_original_row_receipt(collection, after,
            caps.max_payload_bytes, decoded, vm, caps.json, consume),
        _ => return Err(corrupt()),
    }.map_err(compiler_query_error)?;
    charges.rows = charges.rows.checked_add(u64::from(scan.ordinal.is_some())).ok_or_else(budget)?;
    charges.decoded = charges.decoded.checked_add(scan.decoded_bytes).ok_or_else(budget)?;
    charges.vm = charges.vm.checked_add(scan.vm_steps).ok_or_else(budget)?;
    if let Some(error) = failure { return Err(error); }
    authority.check_selected()?;
    model.check_pin().map_err(compiler_query_error)?;
    Ok(output)
}
