//! Source dossier metadata validation, not rights or license adjudication.
use wasm_bindgen::prelude::*;
fn reference(v: &[u16]) -> bool {
    if !v.starts_with(&[116, 111, 115, 46]) {
        return false;
    }
    let mut part = false;
    for u in &v[4..] {
        if (97..=122).contains(u) || (48..=57).contains(u) {
            part = true;
        } else if (*u == 46 || *u == 45) && part {
            part = false;
        } else {
            return false;
        }
    }
    part
}
fn refuse(code: &str) -> JsValue {
    JsValue::from_str(code)
}
#[wasm_bindgen]
pub struct SourceDossierSession {
    step: &'static str,
    array: usize,
    request: bool,
}
impl SourceDossierSession {
    fn code(&self) -> &'static str {
        if self.request {
            "request"
        } else if self.step.starts_with("chain") {
            "chain"
        } else {
            "dossier"
        }
    }
}
#[wasm_bindgen]
impl SourceDossierSession {
    #[wasm_bindgen(constructor)]
    pub fn new(request: bool) -> Self {
        Self {
            step: if request { "request-ref" } else { "schema" },
            array: 0,
            request,
        }
    }
    pub fn need(&self) -> String {
        self.step.into()
    }
    pub fn reference_length(&self, string: bool, length: usize) -> Result<(), JsValue> {
        if string && length > 0 && length <= 2048 {
            Ok(())
        } else {
            Err(refuse(self.code()))
        }
    }
    pub fn reference(&mut self, units: &[u16]) -> Result<(), JsValue> {
        if !reference(units) {
            return Err(refuse(self.code()));
        }
        self.step = if self.request {
            "request-limit"
        } else {
            "object-id"
        };
        Ok(())
    }
    pub fn request_limit(&mut self, number: bool, value: f64) -> Result<(), JsValue> {
        if !number || !value.is_finite() || value.fract() != 0.0 || !(1.0..=64.0).contains(&value) {
            return Err(refuse("request"));
        }
        self.step = "done";
        Ok(())
    }
    pub fn text_length(&self, string: bool, length: usize) -> Result<(), JsValue> {
        let valid = string
            && if self.step == "schema" {
                length == 21
            } else {
                length <= 10
            };
        if valid {
            Ok(())
        } else {
            Err(refuse("dossier"))
        }
    }
    pub fn text(&mut self, units: &[u16]) -> Result<(), JsValue> {
        let valid = if self.step == "schema" {
            units
                .iter()
                .copied()
                .eq("tos_source_dossier_v1".encode_utf16())
        } else {
            ["work", "expression", "edition", "item", "file", "link"]
                .iter()
                .any(|v| units.iter().copied().eq(v.encode_utf16()))
        };
        if !valid {
            return Err(refuse("dossier"));
        }
        self.step = if self.step == "schema" {
            "expected-ref"
        } else {
            "summary"
        };
        Ok(())
    }
    pub fn record_type(&self, nonnull: bool, object: bool) -> Result<bool, JsValue> {
        let valid = nonnull && object;
        if !valid && matches!(self.step, "object" | "authority-record") {
            return Err(refuse(self.code()));
        }
        Ok(valid)
    }
    pub fn record_array(&mut self, array: bool) -> Result<bool, JsValue> {
        if matches!(self.step, "object" | "authority-record") {
            if array {
                return Err(refuse(self.code()));
            }
            self.step = if self.step == "object" {
                "node-id"
            } else {
                "chain-truthy"
            };
        }
        Ok(!array)
    }
    pub fn license(&mut self, boolean: bool, value: bool) -> Result<(), JsValue> {
        if self.step != "license-false" || !boolean || value {
            return Err(refuse("dossier"));
        }
        self.step = "array-value";
        Ok(())
    }
    pub fn array_location(&self) -> String {
        if self.array < 2 { "summary" } else { "packet" }.into()
    }
    pub fn observe(&mut self, value: bool) -> Result<(), JsValue> {
        let next = match self.step {
            "object-id" => "object",
            "node-id" => "kind",
            "summary" => "technical-type",
            "technical-type" => "technical-truthy",
            "technical-truthy" => "posture-type",
            "posture-type" => "posture-truthy",
            "posture-truthy" => "review-type",
            "review-type" => "legal-type",
            "legal-type" => "license-false",
            "array-every" => {
                self.array += 1;
                if self.array < 6 {
                    "array-value"
                } else {
                    "truncated"
                }
            }
            "truncated" => "authority-type",
            "authority-type" => {
                self.step = if value {
                    "authority-length"
                } else {
                    "authority-record"
                };
                return Ok(());
            }
            "chain-truthy" => "chain-type",
            "chain-type" => "chain-array",
            "chain-array" => {
                if value {
                    return Err(refuse("chain"));
                }
                "chain-values"
            }
            "chain-values" => {
                if value {
                    return Err(refuse("chain"));
                }
                "done"
            }
            _ => return Err(refuse("invalid_dossier_progress")),
        };
        if !value && !matches!(self.step, "chain-array" | "chain-values") {
            return Err(refuse(self.code()));
        }
        self.step = next;
        Ok(())
    }
    pub fn authority_length(&mut self, value: f64) {
        self.step = if value > 0.0 {
            "chain-truthy"
        } else {
            "authority-record"
        };
    }
    pub fn array_field(&self) -> String {
        [
            "rights_scope_refs",
            "gaps",
            "relations",
            "rights",
            "tree_paths",
            "source_refs",
        ][self.array]
            .into()
    }
    pub fn array_type(&mut self, array: bool) -> Result<(), JsValue> {
        if !array {
            return Err(refuse("dossier"));
        }
        self.step = "array-length";
        Ok(())
    }
    pub fn array_length(&mut self, length: f64) -> Result<(), JsValue> {
        if !(length <= if self.array == 5 { 1024.0 } else { 64.0 }) {
            return Err(refuse("dossier"));
        }
        self.step = "array-every";
        Ok(())
    }
    pub fn element_kind(&self) -> String {
        if self.array < 2 || self.array == 5 {
            "string"
        } else {
            "record"
        }
        .into()
    }
    pub fn record_element_type(nonnull: bool, object: bool) -> bool {
        nonnull && object
    }
    pub fn record_element_array(array: bool) -> bool {
        !array
    }
    pub fn string_element(string: bool, length: usize) -> bool {
        string && length > 0 && length <= 2048
    }
    pub fn chain_array_type(array: bool) -> bool {
        array
    }
    pub fn chain_array_length(length: f64) -> bool {
        length <= 64.0
    }
    pub fn chain_result(every: bool) -> bool {
        !every
    }
}
