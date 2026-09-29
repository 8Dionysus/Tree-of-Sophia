//! Ordered browser inspection validation. Source access, equality indexes,
//! localization and asynchronous transport remain in the host.
use wasm_bindgen::prelude::*;
#[wasm_bindgen]
pub struct ClientInspectionSession {
    step: &'static str,
    node: bool,
    relation: bool,
    expected: bool,
    content_expected: bool,
    endpoint_items: bool,
}
#[wasm_bindgen]
impl ClientInspectionSession {
    #[wasm_bindgen(constructor)]
    pub fn new(node: bool, relation: bool, expected: bool, content_expected: bool) -> Self {
        Self {
            step: "revision",
            node,
            relation,
            expected,
            content_expected,
            endpoint_items: false,
        }
    }
    pub fn need(&self) -> String {
        self.step.into()
    }
    pub fn text_length(&self, length: usize) -> Result<(), JsValue> {
        if length == 64 {
            Ok(())
        } else {
            Err(JsValue::from_str(if self.step == "revision" {
                "missing_revision"
            } else {
                "item"
            }))
        }
    }
    pub fn schema_length(&self, is_string: bool, length: usize) -> Result<(), JsValue> {
        let expected = if self.node {
            "tos_knowledge_node_packet_v1"
        } else {
            "tos_knowledge_relation_packet_v1"
        };
        if self.step == "schema" && is_string && length == expected.len() {
            Ok(())
        } else {
            Err(JsValue::from_str("schema"))
        }
    }
    pub fn text(&mut self, units: &[u16]) -> Result<(), JsValue> {
        if self.step == "schema" {
            let expected = if self.node {
                "tos_knowledge_node_packet_v1"
            } else {
                "tos_knowledge_relation_packet_v1"
            };
            if !units.iter().copied().eq(expected.encode_utf16()) {
                return Err(JsValue::from_str("schema"));
            }
            self.step = "matches";
            return Ok(());
        }
        let code = if self.step == "revision" {
            "missing_revision"
        } else {
            "item"
        };
        if !matches!(self.step, "revision" | "content-revision")
            || units.len() != 64
            || !units
                .iter()
                .all(|u| (48..=57).contains(u) || (97..=102).contains(u))
        {
            return Err(JsValue::from_str(code));
        }
        self.step = if self.step == "revision" {
            if self.expected { "expected" } else { "schema" }
        } else {
            "refs-array"
        };
        Ok(())
    }
    pub fn observe(&mut self, value: bool) -> Result<(), JsValue> {
        let (valid, code, next) = match self.step {
            "expected" => (value, "revision", "schema"),
            "matches" | "endpoints" => (value, "items", "item-next"),
            "item-next" => {
                self.step = if value {
                    "item-truthy"
                } else if self.endpoint_items {
                    "from-endpoint"
                } else {
                    "exact"
                };
                return Ok(());
            }
            "item-truthy" => (value, "item", "id-type"),
            "id-type" => (value, "item", "id-truthy"),
            "id-truthy" => (value, "item", "id-duplicate"),
            "id-duplicate" => (!value, "item", "display"),
            "display" => (
                value,
                "item",
                if self.node || self.endpoint_items {
                    "localized-title"
                } else {
                    "localized-label"
                },
            ),
            "localized-title" | "localized-label" => (value, "item", "content-revision"),
            "refs-array" => (value, "item", "refs-length"),
            "refs-length" => (value, "item", "refs"),
            "refs" => (!value, "item", "add-id"),
            "add-id" => (value, "item", "item-next"),
            "exact" => (
                value,
                "match",
                if self.content_expected {
                    "content-expected"
                } else if self.relation {
                    "endpoints"
                } else {
                    "done"
                },
            ),
            "content-expected" => (
                value,
                "revision",
                if self.relation { "endpoints" } else { "done" },
            ),
            "from-endpoint" => (value, "endpoints", "to-endpoint"),
            "to-endpoint" => (value, "endpoints", "done"),
            _ => return Err(JsValue::from_str("invalid_inspection_progress")),
        };
        if !valid {
            return Err(JsValue::from_str(code));
        }
        if self.step == "endpoints" {
            self.endpoint_items = true;
        }
        self.step = next;
        Ok(())
    }
    pub fn ref_invalid(string: bool, truthy: bool) -> bool {
        !string || !truthy
    }
}
