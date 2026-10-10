//! Ordered route probing; transport and original error objects stay host-owned.
use wasm_bindgen::prelude::*;
#[wasm_bindgen]
pub struct RouteCenterSession {
    step: &'static str,
}
#[wasm_bindgen]
impl RouteCenterSession {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self { step: "id-type" }
    }
    pub fn need(&self) -> String {
        self.step.into()
    }
    pub fn observe(&mut self, value: bool) -> Result<(), JsValue> {
        self.step = match self.step {
            "id-type" => {
                if value {
                    "id-trim"
                } else {
                    return Err(JsValue::from_str("route-id"));
                }
            }
            "id-trim" => {
                if value {
                    "relation-inspect"
                } else {
                    return Err(JsValue::from_str("route-id"));
                }
            }
            "relation-inspect" => "relation-compile",
            "relation-compile" => "relation-presence",
            "relation-presence" => {
                if value {
                    "relation-return"
                } else {
                    return Err(JsValue::from_str("route-presence"));
                }
            }
            "relation-error-instance" => {
                if value {
                    "relation-error-status"
                } else {
                    "throw-current"
                }
            }
            "relation-error-status" => {
                if value {
                    "node-compile"
                } else {
                    "throw-current"
                }
            }
            "node-compile" => "node-return",
            "node-error-prior" => {
                if value {
                    "node-error-instance"
                } else {
                    "throw-current"
                }
            }
            "node-error-instance" => {
                if value {
                    "node-error-status"
                } else {
                    "throw-current"
                }
            }
            "node-error-status" => {
                if value {
                    "throw-relation"
                } else {
                    "throw-current"
                }
            }
            _ => return Err(JsValue::from_str("invalid_route_progress")),
        };
        Ok(())
    }
    pub fn failed(&mut self) -> Result<(), JsValue> {
        self.step = match self.step {
            "relation-inspect" | "relation-compile" | "relation-presence" | "relation-return" => {
                "relation-error-instance"
            }
            "node-compile" | "node-return" => "node-error-prior",
            _ => return Err(JsValue::from_str("invalid_route_progress")),
        };
        Ok(())
    }
    pub fn relation_member(equal: bool) -> bool {
        equal
    }
}
