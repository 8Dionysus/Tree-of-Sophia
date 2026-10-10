//! Ordered browser path request policy, without graph or source authority.
use wasm_bindgen::prelude::*;
#[wasm_bindgen]
pub struct PathQuerySession {
    phase: u8,
}
#[wasm_bindgen]
impl PathQuerySession {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self { phase: 0 }
    }
    pub fn required(&mut self, length: usize) -> bool {
        if self.phase > 1 {
            return false;
        }
        if length == 0 {
            self.phase = 9;
            return false;
        }
        self.phase += 1;
        true
    }
    pub fn depth(&mut self, value: f64) -> f64 {
        self.phase = 3;
        if value.is_finite() {
            value.trunc().clamp(1.0, 8.0)
        } else {
            6.0
        }
    }
    pub fn direction(&mut self, value: &[u16]) -> bool {
        if self.phase != 3 {
            return false;
        }
        let allowed = value == [111, 117, 116, 103, 111, 105, 110, 103]
            || value == [105, 110, 99, 111, 109, 105, 110, 103]
            || value == [101, 105, 116, 104, 101, 114];
        self.phase = if allowed { 4 } else { 9 };
        allowed
    }
    pub fn alternatives(&mut self, value: f64) -> f64 {
        self.phase = 5;
        if value.is_finite() {
            value.trunc().clamp(1.0, 5.0)
        } else {
            1.0
        }
    }
    pub fn filter_empty(&self, length: usize) -> bool {
        length == 0
    }
    pub fn empty_filter(&self) -> String {
        "__tos_none__".into()
    }
}
