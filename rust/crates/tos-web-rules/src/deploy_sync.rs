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

/// Revision-query admission and schema choice for the existing deploy transport.
#[wasm_bindgen]
pub struct DeployRevisionSession {
    phase: &'static str,
    choice: &'static str,
}
#[wasm_bindgen]
impl DeployRevisionSession {
    #[wasm_bindgen(constructor)]
    pub fn new(operation: u8) -> Self {
        Self {
            phase: match operation {
                0 => "chunk-column",
                1 => "payload-array",
                2 => "raw-read",
                _ => "table-length",
            },
            choice: "",
        }
    }
    pub fn need(&self) -> String {
        self.phase.into()
    }
    pub fn flag(&mut self, value: bool) {
        self.phase = match self.phase {
            "chunk-column" => {
                if value {
                    "part-column"
                } else {
                    "legacy-column"
                }
            }
            "part-column" => {
                if value {
                    self.choice = "chunk";
                    "query-return"
                } else {
                    "legacy-column"
                }
            }
            "legacy-column" => {
                self.choice = if value { "legacy" } else { "none" };
                "query-return"
            }
            "payload-array" => {
                if value {
                    "failed-results"
                } else {
                    "query-error"
                }
            }
            "failed-results" => {
                if value {
                    "query-error"
                } else {
                    "rows-return"
                }
            }
            "raw-read" => "raw-string",
            "raw-string" => {
                if value {
                    "parse-revision"
                } else {
                    "null-return"
                }
            }
            "parse-revision" => "digest-string",
            "digest-string" => {
                if value {
                    "digest-truthy"
                } else {
                    "null-return"
                }
            }
            "digest-truthy" => {
                if value {
                    "digest-return"
                } else {
                    "null-return"
                }
            }
            "columns-read" => "query-present",
            "query-present" => {
                if value {
                    "execute-revision"
                } else {
                    "schema-error"
                }
            }
            _ => self.phase,
        };
    }
    pub fn table_length(&mut self, is_number: bool, value: f64) {
        self.phase = if is_number && value == 0.0 {
            "null-return"
        } else {
            "columns-read"
        };
    }
    pub fn choice(&self) -> String {
        self.choice.into()
    }
    // Static predicates survive callbacks retained by overridden native array methods.
    pub fn failed_success(is_true: bool) -> bool {
        !is_true
    }
    pub fn results_array(is_array: bool) -> bool {
        is_array
    }
    pub fn string_column(is_string: bool) -> bool {
        is_string
    }
}
