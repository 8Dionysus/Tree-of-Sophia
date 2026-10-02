//! Word request refusal and bounded rank policy; JS coercion and transport stay host-owned.
use wasm_bindgen::prelude::*;
#[wasm_bindgen]
pub struct WordQuerySession {
    phase: u8,
    rank: f64,
}
#[wasm_bindgen]
impl WordQuerySession {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            phase: 0,
            rank: 1.0,
        }
    }
    pub fn query(&mut self, length: usize) -> bool {
        if self.phase != 0 {
            return false;
        }
        self.phase = if length == 0 { 4 } else { 1 };
        length != 0
    }
    pub fn language(&mut self, value: &[u16]) -> bool {
        if self.phase != 1 {
            return false;
        }
        let allowed = value == [100, 101] || value == [114, 117] || value == [101, 110];
        self.phase = if allowed { 2 } else { 4 };
        allowed
    }
    pub fn number(&mut self, value: f64) -> f64 {
        if self.phase != 2 {
            return self.rank;
        }
        self.rank = if value.is_finite() {
            value.trunc().clamp(1.0, 100.0)
        } else {
            1.0
        };
        self.phase = 3;
        self.rank
    }
}
