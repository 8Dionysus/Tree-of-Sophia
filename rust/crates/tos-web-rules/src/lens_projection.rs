//! Browser presentation allocation, not source meaning or a camera admission.
//! Opaque IDs/previous vectors remain in native host indexes. ICU, JS coercion,
//! trig and slice operations are demanded by this narrow policy.
use wasm_bindgen::prelude::*;
fn progress() -> JsValue {
    JsValue::from_str("invalid_lens_projection_progress")
}
#[wasm_bindgen]
pub struct LensSortSession {
    step: &'static str,
    b_focus: bool,
    b_degree: f64,
    result: f64,
}
#[wasm_bindgen]
impl LensSortSession {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            step: "b-focus",
            b_focus: false,
            b_degree: 0.0,
            result: 0.0,
        }
    }
    pub fn need(&self) -> String {
        self.step.into()
    }
    pub fn result(&self) -> f64 {
        self.result
    }
    pub fn focus(&mut self, value: bool) -> Result<(), JsValue> {
        match self.step {
            "b-focus" => {
                self.b_focus = value;
                self.step = "a-focus";
            }
            "a-focus" => {
                self.result = (self.b_focus as i32 - value as i32) as f64;
                self.step = if self.result != 0.0 {
                    "done"
                } else {
                    "b-degree"
                };
            }
            _ => return Err(progress()),
        }
        Ok(())
    }
    pub fn degree(&mut self, truthy: bool, value: f64) -> Result<(), JsValue> {
        let number = if truthy { value } else { 0.0 };
        match self.step {
            "b-degree" => {
                self.b_degree = number;
                self.step = "a-degree";
            }
            "a-degree" => {
                self.result = self.b_degree - number;
                self.step = if self.result != 0.0 && !self.result.is_nan() {
                    "done"
                } else {
                    "locale"
                };
            }
            _ => return Err(progress()),
        }
        Ok(())
    }
}
#[wasm_bindgen]
pub struct LensNodeSession {
    step: &'static str,
    next_slot: f64,
    allocated_slot: f64,
    hash: u32,
    group: u8,
}
#[wasm_bindgen]
impl LensNodeSession {
    #[wasm_bindgen(constructor)]
    pub fn new(next_slot: f64) -> Self {
        Self {
            step: "scan-slot",
            next_slot,
            allocated_slot: 0.0,
            hash: 2166136261,
            group: 1,
        }
    }
    pub fn need(&self) -> String {
        self.step.into()
    }
    pub fn next_slot(&self) -> f64 {
        self.next_slot
    }
    pub fn allocated_slot(&self) -> f64 {
        self.allocated_slot
    }
    pub fn scan_slot(&mut self, member: bool) -> Result<(), JsValue> {
        if self.step != "scan-slot" {
            return Err(progress());
        }
        if member {
            self.next_slot += 1.0;
        } else {
            self.step = "old-slot";
        }
        Ok(())
    }
    pub fn old_slot(&mut self, null: bool, undefined: bool) -> Result<bool, JsValue> {
        if self.step != "old-slot" {
            return Err(progress());
        }
        let allocated = null || undefined;
        if allocated {
            self.allocated_slot = self.next_slot;
            self.next_slot += 1.0;
        }
        self.step = "occupied-add";
        Ok(allocated)
    }
    pub fn advance(&mut self) -> Result<(), JsValue> {
        self.step = match self.step {
            "occupied-add" => "hash",
            "angle" => "table",
            "fallback-x" => "fallback-y",
            "fallback-y" => "fallback-z",
            "fallback-z" => "prefix",
            "prefix" => "main",
            "clone-source" => "volume",
            "clone-pos" => "restore-target",
            "clone-target" => "done",
            _ => return Err(progress()),
        };
        Ok(())
    }
    pub fn hash_value(&self) -> u32 {
        self.hash
    }
    pub fn hash_word(&mut self, word: i32) -> Result<(), JsValue> {
        if self.step != "hash" {
            return Err(progress());
        }
        self.hash = (word as u32).wrapping_mul(16777619);
        Ok(())
    }
    pub fn hash_done(&mut self) -> Result<(), JsValue> {
        if self.step != "hash" {
            return Err(progress());
        }
        self.step = "angle";
        Ok(())
    }
    pub fn table_candidate(&mut self, truthy: bool) -> Result<(), JsValue> {
        if self.step != "table" {
            return Err(progress());
        }
        self.step = if truthy { "prefix" } else { "fallback-x" };
        Ok(())
    }
    pub fn main_flag(&mut self, index: f64) -> Result<bool, JsValue> {
        if self.step != "main" {
            return Err(progress());
        }
        self.step = "above";
        Ok(index < 8.0)
    }
    pub fn above_flag(&mut self, remainder: f64) -> Result<bool, JsValue> {
        if self.step != "above" {
            return Err(progress());
        }
        self.step = "group-id";
        Ok(remainder == 1.0)
    }
    pub fn group_focus(&mut self, equal: bool) -> Result<(), JsValue> {
        if self.step != "group-id" {
            return Err(progress());
        }
        self.step = if equal {
            self.group = 1;
            "restore-p"
        } else {
            "kind-agent"
        };
        Ok(())
    }
    pub fn kind_length(&self, string: bool, length: usize) -> bool {
        string && length == if self.step == "kind-agent" { 5 } else { 10 }
    }
    pub fn kind(&mut self, units: &[u16]) -> Result<(), JsValue> {
        let equal = units.iter().copied().eq(if self.step == "kind-agent" {
            "agent"
        } else {
            "expression"
        }
        .encode_utf16());
        self.kind_result(equal)
    }
    pub fn kind_mismatch(&mut self) -> Result<(), JsValue> {
        self.kind_result(false)
    }
    pub fn group(&self) -> u8 {
        self.group
    }
    /// 0 retain candidate; 1 retain automatic position; 2 clone position next.
    pub fn restore_candidate(&mut self, truthy: bool) -> Result<u8, JsValue> {
        let (next, clone) = match self.step {
            "restore-p" => ("restore-source", None),
            "restore-source" => ("volume", Some("clone-source")),
            "restore-pos" => ("restore-target", Some("clone-pos")),
            "restore-target" => ("done", Some("clone-target")),
            _ => return Err(progress()),
        };
        if truthy {
            self.step = next;
            Ok(0)
        } else if let Some(clone) = clone {
            self.step = clone;
            Ok(2)
        } else {
            self.step = next;
            Ok(1)
        }
    }
    pub fn volume_candidate(&mut self, null: bool, undefined: bool) -> Result<bool, JsValue> {
        if self.step != "volume" {
            return Err(progress());
        }
        self.step = "restore-pos";
        Ok(!(null || undefined))
    }
    pub fn angle_multiplier(&self) -> f64 {
        2.399963229728653
    }
    pub fn above_cycle(&self) -> f64 {
        3.0
    }
    pub fn x_base(&self) -> f64 {
        260.0
    }
    pub fn x_cycle(&self) -> f64 {
        4.0
    }
    pub fn x_spacing(&self) -> f64 {
        58.0
    }
    pub fn y_base(&self) -> f64 {
        160.0
    }
    pub fn y_cycle(&self) -> f64 {
        3.0
    }
    pub fn y_spacing(&self) -> f64 {
        47.0
    }
    pub fn fallback_z(&self) -> f64 {
        -480.0 + f64::from(self.hash % 740)
    }
    pub fn degree_next(truthy: bool, value: f64) -> f64 {
        if truthy { value + 1.0 } else { 1.0 }
    }
    pub fn slot_table() -> String {
        "[[0,5,70],[-124,-134,-190],[143,-97,210],[151,91,-45],[-106,135,185],[-230,-47,95],[-54,-70,300],[63,157,245],[-29,74,-225],[-403,100,-460],[-474,-2,-335],[-309,184,-350],[-506,141,-560],[-365,-18,-420],[346,17,-140],[412,-98,-315],[490,65,-275],[344,157,120],[470,202,-410]]".into()
    }
}
impl LensNodeSession {
    fn kind_result(&mut self, equal: bool) -> Result<(), JsValue> {
        self.step = match self.step {
            "kind-agent" => {
                if equal {
                    self.group = 0;
                    "restore-p"
                } else {
                    "kind-expression"
                }
            }
            "kind-expression" => {
                self.group = if equal { 2 } else { 1 };
                "restore-p"
            }
            _ => return Err(progress()),
        };
        Ok(())
    }
}
