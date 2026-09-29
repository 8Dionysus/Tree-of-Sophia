//! Ordered final browser packet contracts. The host retains source objects,
//! property access, native collection equality and asynchronous transport.
use wasm_bindgen::prelude::*;
fn refusal(code: &str) -> JsValue {
    JsValue::from_str(code)
}
#[wasm_bindgen]
pub struct ClientPacketSession {
    step: &'static str,
    mode: u8,
    expected: bool,
    previous: bool,
    v2: bool,
    relation: bool,
    paused: bool,
    limited: bool,
    partition_relation: bool,
    endpoint_to: bool,
}
#[wasm_bindgen]
impl ClientPacketSession {
    /// 0 revision, 1 area, 2 lens, 3 exploration, 4 search request,
    /// 5 search response, 6 initial exploration request identity.
    /// Item validation uses the existing inspection core.
    #[wasm_bindgen(constructor)]
    pub fn new(mode: u8, expected: bool, previous: bool, relation: bool) -> Self {
        Self {
            step: match mode {
                2 => "lens-schema",
                3 => "exploration-schema",
                4 => "cursor-null",
                6 => "initial-request",
                _ => "revision",
            },
            mode,
            expected,
            previous,
            v2: false,
            relation,
            paused: false,
            limited: false,
            partition_relation: false,
            endpoint_to: false,
        }
    }
    pub fn need(&self) -> String {
        self.step.into()
    }
    pub fn relation_partition(&self) -> bool {
        self.partition_relation
    }
    pub fn origin_relation(&self) -> bool {
        self.relation
    }
    pub fn endpoint_to(&self) -> bool {
        self.endpoint_to
    }
    pub fn v2(&self) -> bool {
        self.v2
    }
    pub fn text_length(&self, string: bool, length: usize) -> Result<(), JsValue> {
        if !string || length > 64 {
            return Err(refusal(self.code()));
        }
        Ok(())
    }
    pub fn text(&mut self, units: &[u16]) -> Result<(), JsValue> {
        let eq = |text: &str| units.iter().copied().eq(text.encode_utf16());
        let digest = || {
            units.len() == 64
                && units
                    .iter()
                    .all(|u| (48..=57).contains(u) || (97..=102).contains(u))
        };
        let (valid, next) = match self.step {
            "lens-schema" => (eq("tos_lens_result_v1"), "revision"),

            "revision" => (
                digest(),
                if self.expected {
                    "expected"
                } else {
                    self.after_revision()
                },
            ),
            "v1-schema" => (eq("tos_exploration_result_v1"), "writes"),
            "search-schema" => (
                eq(if self.relation {
                    "tos_knowledge_search_indexed_v2"
                } else {
                    "tos_knowledge_search_compressed_v3"
                }),
                "search-nodes-length",
            ),
            "snapshot" => (digest(), if self.v2 { "execution" } else { "status" }),
            "execution" => (
                eq("tos-exploration-execution-v6") || eq("tos-exploration-d1-execution-v6"),
                "status",
            ),
            "status" => {
                self.paused = eq("paused");
                self.limited = eq("limit_reached");
                (
                    self.paused || self.limited || eq("complete"),
                    if self.v2 { "origin" } else { "v1-focus" },
                )
            }
            "origin-kind" => {
                self.relation = eq("relation");
                (self.relation || eq("node"), "origin-revision")
            }
            "origin-revision" => (digest(), "origin-id-type"),
            "query-schema" => (eq("tos_exploration_request_v2"), "query-revision"),
            "page-scope" => (eq("resumable-neighborhood"), "returned-nodes"),
            "counts-scope" => (
                eq("cumulative-discovered-not-global-total"),
                "inclusion-authority",
            ),
            "inclusion-authority" => (eq("query-execution-not-semantic-proof"), "cursor-status"),
            "cursor-digest" => (digest(), self.after_cursor()),
            "limit-reason" => (
                eq("session_nodes") || eq("session_relations"),
                "partition-start",
            ),
            "origin-inclusion" => (
                eq("origin"),
                if self.relation {
                    "endpoint-start"
                } else {
                    "previous"
                },
            ),
            "endpoint-inclusion" => (eq("origin-endpoint"), "endpoint-next"),
            "prior-status" => (eq("paused"), "prior-source"),
            _ => return Err(refusal("invalid_packet_progress")),
        };
        self.accept(valid, next)
    }
    pub fn select_v2(&mut self, value: bool) -> Result<(), JsValue> {
        if self.step != "exploration-schema" {
            return Err(refusal("invalid_packet_progress"));
        }
        self.v2 = value;
        self.step = "revision";
        Ok(())
    }
    pub fn strict_digest(&self) -> bool {
        self.step == "origin-revision"
            || self.v2 && matches!(self.step, "snapshot" | "cursor-digest")
    }
    pub fn digest_type(&self, string: bool) -> Result<(), JsValue> {
        if self.strict_digest() && !string {
            Err(refusal(self.code()))
        } else {
            Ok(())
        }
    }
    pub fn member_missing(member: bool) -> bool {
        !member
    }
    pub fn observe(&mut self, value: bool) -> Result<(), JsValue> {
        let (valid, next) = match self.step {
            "initial-request" => {
                self.step = if self.previous {
                    "done"
                } else {
                    "request-match"
                };
                return Ok(());
            }
            "request-match" => (value, "done"),
            "cursor-null" => {
                self.step = if value {
                    "request-limit-type"
                } else {
                    "request-cursor-type"
                };
                return Ok(());
            }
            "request-cursor-type" => (value, "request-cursor-length"),
            "request-mode" => (value, "request-limit-type"),
            "expected" => (value, self.after_revision()),
            "authority-source" => (value, "authority-canon"),
            "authority-canon" => (value, "authority-writes"),
            "authority-writes" => (value, "nodes-array"),
            "nodes-array" => (value, "relations-array"),
            "relations-array" => (value, "nodes-length"),
            "node-items" => (true, "relation-items"),
            "relation-items" => (
                true,
                if self.mode == 5 {
                    "done"
                } else {
                    "area-endpoints"
                },
            ),
            "area-endpoints" => (!value, "area-focus"),
            "area-focus" => {
                self.step = if value {
                    "area-focus-member"
                } else {
                    self.after_area()
                };
                return Ok(());
            }
            "area-focus-member" => (value, self.after_area()),
            "exploration-snapshots" => (true, if self.v2 { "writes" } else { "v1-schema" }),
            "writes" => (value, "snapshot"),
            "v1-focus" => (value, "page-integer"),
            "origin" => (value, "origin-kind"),
            "origin-id-type" => (value, "origin-id-truthy"),
            "origin-id-truthy" => (value, "query"),
            "query" => (value, "query-schema"),
            "query-revision" => (value, "query-origin"),
            "query-origin" => (value, "query-origin-fields"),
            "query-origin-fields" => (!value, "focus-own"),
            "focus-own" => (!value, "page-integer"),
            "returned-nodes" => (value, "returned-relations"),
            "returned-relations" => (
                value,
                if self.v2 {
                    "work-integer"
                } else {
                    "v1-primary-array"
                },
            ),
            "v1-primary-array" => (value, "v1-context-array"),
            "v1-context-array" => (value, "v1-total"),
            "v1-total" => (value, "v1-unique"),
            "v1-unique" => (value, "v1-members"),
            "v1-members" => (!value, "counts-scope"),
            "cursor-status" => {
                self.paused = value;
                self.step = if value {
                    "cursor-digest"
                } else {
                    "cursor-null-value"
                };
                return Ok(());
            }
            "cursor-null-value" => (value, self.after_cursor()),
            "limit-status" => {
                self.limited = value;
                self.step = if value { "limit-reason" } else { "limit-null" };
                return Ok(());
            }
            "limit-null" => (value, "partition-start"),
            "partition-start" => (true, "primary-array"),
            "primary-array" => (value, "context-array"),
            "context-array" => (value, "partition-total"),
            "partition-total" => (value, "partition-unique"),
            "partition-unique" => (value, "partition-members"),
            "partition-members" => (!value, "partition-inclusion"),
            "partition-inclusion" => (value, "inclusion-count"),
            "inclusion-count" => (value, "inclusion-keys"),
            "inclusion-keys" => (!value, "partition-next"),
            "partition-next" => {
                self.step = if self.partition_relation {
                    "origin-item"
                } else {
                    self.partition_relation = true;
                    "partition-start"
                };
                return Ok(());
            }
            "origin-item" => (value, "origin-item-revision"),
            "origin-item-revision" => (value, "origin-branch"),
            "origin-branch" => {
                self.relation = !value;
                self.step = if value {
                    "origin-no-endpoints"
                } else {
                    "context-relation-count"
                };
                return Ok(());
            }
            "origin-no-endpoints" => (!value, "context-relations-empty"),
            "context-relations-empty" => (!value, "origin-node-context"),
            "origin-node-context" => (value, "origin-inclusion"),
            "context-relation-count" => (value, "context-relation-id"),
            "context-relation-id" => (value, "origin-inclusion"),
            "endpoint-start" => (true, "endpoint"),
            "endpoint" => (value, "endpoint-node"),
            "endpoint-node" => (value, "endpoint-id"),
            "endpoint-id" => (value, "endpoint-revision"),
            "endpoint-revision" => (value, "endpoint-entity"),
            "endpoint-entity" => (value, "endpoint-context"),
            "endpoint-context" => (value, "endpoint-inclusion"),
            "endpoint-next" => {
                self.step = if self.endpoint_to {
                    "previous"
                } else {
                    self.endpoint_to = true;
                    "endpoint-start"
                };
                return Ok(());
            }
            "previous" => {
                self.step = if self.previous {
                    if self.v2 {
                        "prior-schema"
                    } else {
                        "prior-snapshot"
                    }
                } else {
                    self.after_prior()
                };
                return Ok(());
            }
            "prior-schema" => (value, "prior-status"),
            "prior-source" => (value, "prior-snapshot"),
            "prior-snapshot" => (
                value,
                if self.v2 {
                    "prior-execution"
                } else {
                    "prior-focus"
                },
            ),
            "prior-execution" => (value, "prior-page"),
            "prior-focus" => (value, "prior-page"),
            "prior-page" => (
                value,
                if self.v2 {
                    "prior-cursor-status"
                } else {
                    "prior-query-json"
                },
            ),
            "prior-cursor-status" => {
                self.step = if value {
                    "prior-cursor-different"
                } else {
                    "prior-origin"
                };
                return Ok(());
            }
            "prior-cursor-different" => (!value, "prior-origin"),
            "prior-origin" => (value, "prior-query"),
            "prior-query" | "prior-query-json" => (value, self.after_prior()),
            "scene" => (value, "scene-equal"),
            "scene-equal" => (value, "done"),
            "search-cursor" => (value, "search-limit"),
            "search-limit" => (value, "search-more-type"),
            "search-more-type" => (value, "search-more"),
            "search-more" => {
                self.step = if value {
                    "search-next-type"
                } else {
                    "search-next-null"
                };
                return Ok(());
            }
            "search-next-type" => (value, "search-next-truthy"),
            "search-next-truthy" | "search-next-null" => (value, "search-writes"),
            "search-writes" => (value, "node-items"),
            _ => return Err(refusal("invalid_packet_progress")),
        };
        self.accept(valid, next)
    }
    pub fn integer(&mut self, number: bool, value: f64) -> Result<(), JsValue> {
        let safe = self.v2 || self.step == "request-limit-type" || self.step == "work-integer";
        let valid = number
            && value.is_finite()
            && value.fract() == 0.0
            && (!safe || value.abs() <= 9007199254740991.0);
        let next = match self.step {
            "page-integer" => "page-positive",
            "work-integer" => "work-lower",
            "request-limit-type" => "request-limit",
            _ => return Err(refusal("invalid_packet_progress")),
        };
        self.accept(valid, next)
    }
    pub fn number(&mut self, value: f64) -> Result<(), JsValue> {
        let (valid, next) = match self.step {
            "request-cursor-length" => (value <= 65536.0, "request-mode"),
            "request-limit" => (
                value.is_finite() && value.fract() == 0.0 && (1.0..=6.0).contains(&value),
                "done",
            ),
            "nodes-length" => (
                !(value > if self.v2 { 302.0 } else { 40.0 }),
                "relations-length",
            ),
            "relations-length" => (!(value > if self.v2 { 101.0 } else { 80.0 }), "node-items"),
            "page-positive" => (!(value < 1.0), "page-scope"),
            "work-lower" => (!(value < 0.0), "work-upper"),
            "work-upper" => (!(value > 512.0), "counts-scope"),
            _ => return Err(refusal("invalid_packet_progress")),
        };
        self.accept(valid, next)
    }
    pub fn search_length(&mut self, value: f64, limit: f64) -> Result<(), JsValue> {
        let next = match self.step {
            "search-nodes-length" => "search-relations-length",
            "search-relations-length" => "search-cursor",
            _ => return Err(refusal("invalid_packet_progress")),
        };
        self.accept(!(value > limit), next)
    }
}
impl ClientPacketSession {
    fn code(&self) -> &'static str {
        match self.step {
            "revision" => "missing_revision",
            "request-match" => "request_area",
            "expected" => "revision",
            "lens-schema" => "lens",
            "authority-source" | "authority-canon" | "authority-writes" => "area",
            "nodes-array" | "relations-array" | "nodes-length" | "relations-length" => "budget",
            "area-endpoints" => "endpoints",
            "area-focus-member" => "focus",
            s if s.starts_with("prior-") => "revision",
            _ if self.mode == 4 || self.mode == 5 => "search",
            _ => "exploration",
        }
    }
    fn accept(&mut self, valid: bool, next: &'static str) -> Result<(), JsValue> {
        if !valid {
            return Err(refusal(self.code()));
        }
        self.step = next;
        Ok(())
    }
    fn after_revision(&self) -> &'static str {
        match self.mode {
            0 => "done",
            5 => "search-schema",
            _ => "authority-source",
        }
    }
    fn after_area(&self) -> &'static str {
        if self.mode == 3 {
            "exploration-snapshots"
        } else {
            "done"
        }
    }
    fn after_cursor(&self) -> &'static str {
        if self.v2 { "limit-status" } else { "previous" }
    }
    fn after_prior(&self) -> &'static str {
        if self.v2 { "scene" } else { "done" }
    }
}

/// Exact browser JSON equality, retaining native every callback behavior.
/// It compares observations only; it never parses or serializes source values.
#[wasm_bindgen]
pub struct ClientJsonSession {
    step: &'static str,
    result: u8,
}
#[wasm_bindgen]
impl ClientJsonSession {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            step: "identity",
            result: 0,
        }
    }
    pub fn need(&self) -> String {
        self.step.into()
    }
    pub fn result(&self) -> u8 {
        self.result
    }
    pub fn observe(&mut self, value: bool) -> Result<(), JsValue> {
        let next = match self.step {
            "identity" => {
                if value {
                    self.result = 1;
                    "done"
                } else {
                    "left-null"
                }
            }
            "left-null" => {
                if value {
                    "done"
                } else {
                    "right-null"
                }
            }
            "right-null" => {
                if value {
                    "done"
                } else {
                    "left-object"
                }
            }
            "left-object" => {
                if value {
                    "right-object"
                } else {
                    "done"
                }
            }
            "right-object" => {
                if value {
                    "left-array-first"
                } else {
                    "done"
                }
            }
            "left-array-first" => {
                if value {
                    "left-array-second"
                } else {
                    "right-array-first"
                }
            }
            "right-array-first" => {
                if value {
                    "left-array-second"
                } else {
                    "object-keys"
                }
            }
            "left-array-second" => {
                if value {
                    "right-array-second"
                } else {
                    "done"
                }
            }
            "right-array-second" => {
                if value {
                    "array-length"
                } else {
                    "done"
                }
            }
            "array-length" => {
                if value {
                    "array-every"
                } else {
                    "done"
                }
            }
            "object-keys" => "object-key-count",
            "object-key-count" => {
                if value {
                    "object-every"
                } else {
                    "done"
                }
            }
            "array-every" | "object-every" => {
                self.result = 2;
                "done"
            }
            _ => return Err(refusal("invalid_json_comparison_progress")),
        };
        self.step = next;
        Ok(())
    }
    /// The request normalizer treats exactly these two fields as sets.
    pub fn request_set_key(units: &[u16]) -> bool {
        units.iter().copied().eq("sources".encode_utf16())
            || units.iter().copied().eq("predicate_ids".encode_utf16())
    }
    pub fn request_key_length(length: usize) -> bool {
        length == 7 || length == 13
    }
    pub fn string_element(string: bool) -> bool {
        string
    }
}
#[wasm_bindgen]
pub struct ClientSelectorSession {
    step: &'static str,
    result: u8,
}
#[wasm_bindgen]
impl ClientSelectorSession {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            step: "requested-array",
            result: 0,
        }
    }
    pub fn need(&self) -> String {
        self.step.into()
    }
    pub fn result(&self) -> u8 {
        self.result
    }
    pub fn observe(&mut self, value: bool) -> Result<(), JsValue> {
        let next = match self.step {
            "requested-array" => "actual-array",
            "actual-array" => "strings",
            "strings" => {
                if !value {
                    self.result = 2;
                }
                "actual-unique"
            }
            "actual-unique" => "requested-count",
            "requested-count" => "members",
            "members" => {
                self.result = 2;
                "done"
            }
            _ => return Err(refusal("invalid_selector_comparison_progress")),
        };
        self.step = if value || self.step == "members" {
            next
        } else {
            "done"
        };
        Ok(())
    }
}

#[wasm_bindgen]
pub struct ClientMaterialSession {
    step: &'static str,
    claim: bool,
    relation: bool,
    versions: bool,
}
#[wasm_bindgen]
impl ClientMaterialSession {
    #[wasm_bindgen(constructor)]
    pub fn new(claim: bool, versions: bool) -> Self {
        Self {
            step: if claim { "language" } else { "kind" },
            claim,
            relation: false,
            versions,
        }
    }
    pub fn need(&self) -> String {
        self.step.into()
    }
    pub fn relation(&self) -> bool {
        self.relation
    }
    pub fn kind(&mut self, string: bool, units: &[u16]) -> Result<(), JsValue> {
        let node = units.iter().copied().eq("node".encode_utf16());
        self.relation = units.iter().copied().eq("relation".encode_utf16());
        if self.step != "kind" || !string || !(node || self.relation) {
            return Err(refusal("material_request"));
        }
        self.step = "id-type";
        Ok(())
    }
    pub fn observe(&mut self, value: bool) -> Result<(), JsValue> {
        let (valid, next) = match self.step {
            "id-type" => (value, "id-truthy"),
            "id-truthy" => (value, "language"),
            "language" => (
                value,
                if self.claim {
                    "reference"
                } else if self.relation {
                    "relation-truthy"
                } else {
                    "spec"
                },
            ),
            "relation-truthy" => {
                self.step = if value { "relation-id" } else { "inspect" };
                return Ok(());
            }
            "relation-id" => {
                self.step = if value { "expected" } else { "inspect" };
                return Ok(());
            }
            "expected" => {
                self.step = if value { "spec" } else { "inspect" };
                return Ok(());
            }
            "inspect" | "reference" => (true, "spec"),
            "spec" => (true, "compile"),
            "compile" => (
                true,
                if self.claim {
                    "claim-nodes-count"
                } else {
                    "match"
                },
            ),
            "match" => (value, "content-required"),
            "content-required" => {
                self.step = if value { "content-equal" } else { "allowed" };
                return Ok(());
            }
            "content-equal" => (value, "allowed"),
            "allowed" => (true, "scope-node-count"),
            "scope-node-count" => (value, "scope-node-members"),
            "scope-node-members" => (!value, "scope-relation-count"),
            "scope-relation-count" => (value, "forms"),
            "forms" => (true, "done"),
            "claim-nodes-count" => (value, "claim-nodes-every"),
            "claim-nodes-every" => (value, "claim-relations-count"),
            "claim-relations-count" => (value, "claim-relations-every"),
            "claim-relations-every" => (value, "claim-items"),
            "item-version" => (value, "item-human"),
            "item-human" => (true, "item-readable"),
            "item-readable" => (true, "claim-items"),
            "path" => (value, "closure"),
            "closure" => (true, "path-id"),
            "path-id" => (value, "path-relation-type"),
            "path-relation-type" => (value, "path-nodes-json"),
            "path-nodes-json" => (value, "path-relations-json"),
            "path-relations-json" => (value, "closure-nodes-count"),
            "closure-nodes-count" => (value, "closure-nodes-every"),
            "closure-nodes-every" => (value, "closure-relations-count"),
            "closure-relations-count" => (value, "closure-relations-every"),
            "closure-relations-every" => (value, "done"),
            _ => return Err(refusal("invalid_material_progress")),
        };
        if !valid {
            return Err(refusal(match self.step {
                "id-type" | "id-truthy" | "language" => "material_request",
                "match" => "match",
                "content-equal" | "item-version" => "revision",
                s if s.starts_with("path") || s.starts_with("closure") => "form",
                _ => "material_scope",
            }));
        }
        self.step = next;
        Ok(())
    }
    pub fn begin_item(&mut self) -> Result<(), JsValue> {
        if self.step != "claim-items" {
            return Err(refusal("invalid_material_progress"));
        }
        self.step = if self.versions {
            "item-version"
        } else {
            "item-human"
        };
        Ok(())
    }
    pub fn finish_items(&mut self) -> Result<(), JsValue> {
        if self.step != "claim-items" {
            return Err(refusal("invalid_material_progress"));
        }
        self.step = "path";
        Ok(())
    }
    pub fn kind_length(string: bool, length: usize) -> bool {
        string && (length == 4 || length == 8)
    }
}
