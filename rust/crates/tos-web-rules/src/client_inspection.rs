//! Validation of the browser's already delivered inspection packet.
//! The host keeps exact source objects, equality indexes, localization and I/O.
use wasm_bindgen::prelude::*;
fn require(value: bool, code: &str) -> Result<(), JsValue> {
    if value {
        Ok(())
    } else {
        Err(JsValue::from_str(code))
    }
}
fn revision(units: &[u16]) -> bool {
    units.len() == 64
        && units
            .iter()
            .all(|unit| (48..=57).contains(unit) || (97..=102).contains(unit))
}
#[wasm_bindgen]
pub struct ClientInspectionSession;
#[wasm_bindgen]
impl ClientInspectionSession {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self
    }
    pub fn revision_length(&self, length: usize) -> Result<(), JsValue> {
        require(length == 64, "missing_revision")
    }
    pub fn content_revision_length(&self, length: usize) -> Result<(), JsValue> {
        require(length == 64, "item")
    }
    pub fn revision(&self, units: &[u16]) -> Result<(), JsValue> {
        require(revision(units), "missing_revision")
    }
    pub fn expected_revision(&self, same: bool) -> Result<(), JsValue> {
        require(same, "revision")
    }
    pub fn schema(&self, same: bool) -> Result<(), JsValue> {
        require(same, "schema")
    }
    pub fn items(&self, array: bool) -> Result<(), JsValue> {
        require(array, "items")
    }
    pub fn identity(&self, valid: bool) -> Result<(), JsValue> {
        require(valid, "item")
    }
    pub fn display(&self, present: bool) -> Result<(), JsValue> {
        require(present, "item")
    }
    pub fn content_revision(&self, units: &[u16]) -> Result<(), JsValue> {
        require(revision(units), "item")
    }
    pub fn source_refs(&self, valid: bool) -> Result<(), JsValue> {
        require(valid, "item")
    }
    pub fn exact_match(&self, found: bool) -> Result<(), JsValue> {
        require(found, "match")
    }
    pub fn endpoints(&self, complete: bool) -> Result<(), JsValue> {
        require(complete, "endpoints")
    }
}
