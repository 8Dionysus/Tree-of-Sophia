//! Declared context rules. Source access and exact value cloning stay in the host.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct RecordContextSession {
    state: &'static str,
    tokens: Vec<Vec<u16>>,
}

#[wasm_bindgen]
impl RecordContextSession {
    #[wasm_bindgen(constructor)]
    pub fn new(declared: bool, schema_valid: bool, revision_same: bool, pointers_array: bool) -> Self {
        Self {
            state: if !declared { "not-declared" } else if !schema_valid || !revision_same || !pointers_array { "unavailable" } else { "available" },
            tokens: Vec::new(),
        }
    }

    pub fn state(&self) -> String { self.state.into() }

    pub fn pointer(&mut self, is_string: bool, units: &[u16]) -> bool {
        self.tokens.clear();
        if !is_string || (!units.is_empty() && units[0] != 47) { return false; }
        if units.is_empty() { return true; }
        for part in units[1..].split(|unit| *unit == 47) {
            let mut token = Vec::with_capacity(part.len());
            let mut at = 0;
            while at < part.len() {
                if part[at] == 126 {
                    at += 1;
                    let Some(next) = part.get(at) else { self.tokens.clear(); return false; };
                    match next { 48 => token.push(126), 49 => token.push(47), _ => { self.tokens.clear(); return false; } }
                } else { token.push(part[at]); }
                at += 1;
            }
            self.tokens.push(token);
        }
        true
    }

    pub fn token_count(&self) -> usize { self.tokens.len() }
    pub fn token(&self, index: usize) -> Vec<u16> { self.tokens[index].clone() }
    pub fn array_index(&self, index: usize) -> bool {
        let token = &self.tokens[index];
        token.as_slice() == [48] || (!token.is_empty() && (49..=57).contains(&token[0]) && token[1..].iter().all(|unit| (48..=57).contains(unit)))
    }
    pub fn unavailable(&mut self) { self.state = "incomplete"; }
}
