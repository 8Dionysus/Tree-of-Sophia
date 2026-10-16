//! Adapter into the existing Corpus kernel; Original selectors remain owned
//! by the compiler's maintained scalar-index planner.
use crate::controlled_original_reader::{OriginalReadCharges, read_corpus_selected_original};
use crate::controlled_query_adapter::compiler_query_error;
use crate::corpus_read::CorpusOriginalRead;
use crate::search_v2::{SearchV2Error, SearchV2ErrorCode};
use crate::{InspectBudget, InspectCurrentAuthority};
use tos_compiler::{
    ControlledKnowledgeModel, ControlledQueryHeap, CorpusOriginalCollection, CorpusOriginalReceipt,
    CorpusOriginalSelector,
};
use tos_foundation::JsonValue;

pub(crate) struct ControlledCorpusReader<'a, 'model, 'state, 'budget, A: ?Sized> {
    pub model: &'a mut ControlledKnowledgeModel<'model, 'state, 'budget>,
    pub authority: &'a mut A,
    pub caps: InspectBudget,
    pub charges: &'a mut OriginalReadCharges,
    pub heap: &'a mut ControlledQueryHeap<'model, 'state, 'budget>,
}
fn budget() -> SearchV2Error {
    SearchV2Error::new(
        SearchV2ErrorCode::BudgetExceeded,
        "controlled Corpus kernel budget exceeded",
    )
}
fn corrupt() -> SearchV2Error {
    SearchV2Error::new(
        SearchV2ErrorCode::CorruptSelectedCarrier,
        "controlled Corpus receipt differs",
    )
}
impl<'hold, A: InspectCurrentAuthority<'hold> + ?Sized> CorpusOriginalRead
    for ControlledCorpusReader<'_, '_, '_, '_, A>
{
    fn check_interrupt(&mut self) -> Result<(), SearchV2Error> {
        self.authority.check_selected()?;
        self.heap.check().map_err(compiler_query_error)?;
        self.model.check_pin().map_err(compiler_query_error)
    }
    fn charge_kernel_work(&mut self, steps: usize) -> Result<(), SearchV2Error> {
        self.check_interrupt()?;
        self.heap.charge_work(steps).map_err(compiler_query_error)
    }
    fn corpus_row(
        &mut self,
        receipt: &CorpusOriginalReceipt,
        collection: CorpusOriginalCollection,
        selector: &CorpusOriginalSelector,
        after: Option<u64>,
    ) -> Result<Option<(u64, JsonValue)>, SearchV2Error> {
        self.check_interrupt()?;
        let actual = self.model.corpus_original_receipt().ok_or_else(corrupt)?;
        if receipt.descriptor_sha256 != actual.descriptor_sha256
            || receipt.source_cut != actual.source_cut
            || receipt.membership_root != actual.membership_root
            || receipt.header_sha256 != actual.header_sha256
        {
            return Err(corrupt());
        }
        let after = after
            .map(i64::try_from)
            .transpose()
            .map_err(|_| budget())?
            .unwrap_or(-1);
        let row = read_corpus_selected_original(
            self.model,
            self.authority,
            collection,
            selector,
            after,
            self.caps,
            self.charges,
            self.heap,
        )?;
        if let Some((ordinal, value)) = row {
            // Before entering the existing kernel, hold its bounded row clones,
            // ID/ref maps and output frames in the same original heap. Holds
            // remain until final Reply and cannot replenish an Original clock.
            let bytes = value
                .retained_storage_bytes()
                .map_err(|_| budget())?
                .checked_mul(8)
                .and_then(|n| n.checked_add(8 * std::mem::size_of::<JsonValue>()))
                .ok_or_else(budget)?;
            self.heap.retain(bytes).map_err(compiler_query_error)?;
            self.heap.charge_work(bytes).map_err(compiler_query_error)?;
            Ok(Some((
                u64::try_from(ordinal).map_err(|_| corrupt())?,
                value,
            )))
        } else {
            Ok(None)
        }
    }
}
