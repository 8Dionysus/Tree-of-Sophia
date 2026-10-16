//! Borrowed bridge into the existing original compiler heap. No State,
//! deadline, Work/VM counter, selected model, or source authority is created.
use crate::controlled_query_adapter::compiler_query_error;
use crate::lens_plan::OriginalLensBudget;
use crate::search_v2::SearchV2Error;
use std::cell::RefCell;
use tos_compiler::ControlledQueryHeap;
use tos_foundation::{JsonLimits, JsonValue};

pub(crate) struct ControlledLensOriginalBudget<'a, 'context, 'state, 'budget> {
    pub(crate) heap: &'a RefCell<ControlledQueryHeap<'context, 'state, 'budget>>,
}
impl OriginalLensBudget for ControlledLensOriginalBudget<'_, '_, '_, '_> {
    fn check(&self) -> Result<(), SearchV2Error> {
        self.heap.borrow().check().map_err(compiler_query_error)
    }
    fn charge_work(&self, units: usize) -> Result<(), SearchV2Error> {
        self.heap
            .borrow()
            .charge_work(units)
            .map_err(compiler_query_error)
    }
    fn admit_workspace(&self, bytes: usize) -> Result<(), SearchV2Error> {
        self.heap
            .borrow_mut()
            .retain(bytes)
            .map_err(compiler_query_error)
    }
    fn canonicalize(
        &self,
        value: &JsonValue,
        limits: JsonLimits,
    ) -> Result<Vec<u8>, SearchV2Error> {
        self.heap
            .borrow()
            .canonicalize_owned_query_json(value, limits)
            .map_err(compiler_query_error)
    }
}
