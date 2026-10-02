//! Remaining browser request guards; host coercion, native array callbacks and URL encoding remain transport.
use wasm_bindgen::prelude::*;
#[wasm_bindgen]
pub struct QueryRequestRules;
#[wasm_bindgen]
impl QueryRequestRules {
    pub fn mode(corpus: bool) -> String {
        if corpus { "corpus" } else { "philosophy" }.into()
    }
    pub fn required(length: usize) -> bool {
        length != 0
    }
    pub fn optional(length: usize) -> bool {
        length != 0
    }
    pub fn cursor_action(tag: u8) -> u8 {
        match tag {
            0 => 0,
            1 => 1,
            _ => 2,
        }
    }
    pub fn search_mode_allowed(tag: u8) -> bool {
        tag < 3
    }
    pub fn filter_empty(length: usize) -> bool {
        length == 0
    }
    pub fn empty_filter() -> String {
        "__tos_none__".into()
    }
    pub fn number(field: &str, value: f64) -> f64 {
        let (fallback, maximum) = match field {
            "search-limit" | "gaps-limit" => (20.0, 100.0),
            "knowledge-limit" => (40.0, 100.0),
            "descent-depth" => (8.0, 8.0),
            "source-limit" => (300.0, 300.0),
            "view-corpus" => (100.0, 1000.0),
            "view-philosophy" => (1000.0, 1000.0),
            "neighborhood-depth" => (1.0, 3.0),
            "neighborhood-limit" => (80.0, 300.0),
            "epistemic-limit" => (80.0, 200.0),
            _ => return f64::NAN,
        };
        if value.is_finite() {
            value.trunc().clamp(1.0, maximum)
        } else {
            fallback
        }
    }
}
