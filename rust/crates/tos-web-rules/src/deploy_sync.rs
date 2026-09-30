//! Existing deploy consumer policy; identity, error coercion and transport stay host-owned.
use wasm_bindgen::prelude::*;
#[wasm_bindgen]
pub struct DeploySyncSession {
    phase: &'static str,
    count: f64,
    mode: &'static str,
    reason: &'static str,
}
#[wasm_bindgen]
impl DeploySyncSession {
    #[wasm_bindgen(constructor)]
    pub fn new(file_choice: bool) -> Self {
        Self {
            phase: if file_choice { "delta-read" } else { "count" },
            count: 0.0,
            mode: "full",
            reason: "",
        }
    }
    pub fn need(&self) -> String {
        self.phase.into()
    }
    pub fn flag(&mut self, value: bool) {
        self.phase = match self.phase {
            "delta-read" => "available",
            "available" => {
                if value {
                    "base"
                } else {
                    "full-count"
                }
            }
            "base" => {
                if value {
                    "target"
                } else {
                    "full-count"
                }
            }
            "target" => {
                if value {
                    self.mode = "delta";
                    "delta-count"
                } else {
                    "full-count"
                }
            }
            "full-count" | "delta-count" => "file-return",
            "maximum-null" => {
                if value {
                    "revision"
                } else {
                    "maximum"
                }
            }
            "revision" => {
                if value {
                    self.reason = "revision-match";
                    "decision-return"
                } else {
                    "remote-truthy"
                }
            }
            "remote-truthy" => {
                self.reason = if value {
                    "revision-changed"
                } else {
                    "database-empty"
                };
                "decision-return"
            }
            _ => self.phase,
        };
    }
    pub fn number(&mut self, is_number: bool, value: f64) {
        let valid = is_number
            && value.is_finite()
            && value.fract() == 0.0
            && value.abs() <= 9_007_199_254_740_991.0
            && value >= 1.0;
        self.phase = match self.phase {
            "count" => {
                if valid {
                    self.count = value;
                    "maximum-null"
                } else {
                    "count-error"
                }
            }
            "maximum" => {
                if valid && self.count <= value {
                    "revision"
                } else {
                    "ceiling-error"
                }
            }
            _ => self.phase,
        };
    }
    pub fn file(&self) -> String {
        if self.mode == "delta" {
            "runtime/read-model.delta.sql"
        } else {
            "runtime/read-model.sql"
        }
        .into()
    }
    pub fn mode(&self) -> String {
        self.mode.into()
    }
    pub fn reason(&self) -> String {
        self.reason.into()
    }
    pub fn required(&self) -> bool {
        self.reason != "revision-match"
    }
}
