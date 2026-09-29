//! WebMCP presentation rules. Source objects, native callbacks, tool
//! definitions, registration and final wire serialization stay in the host.
use wasm_bindgen::prelude::*;
#[wasm_bindgen]
pub struct WebMcpRules {}
#[wasm_bindgen]
impl WebMcpRules {
    pub fn identity_action(is_string: bool) -> u32 {
        u32::from(is_string)
    }
    pub fn text_action(field: &str, is_string: bool, length: usize) -> u32 {
        if !is_string {
            0
        } else if length > Self::text_maximum(field) {
            2
        } else {
            1
        }
    }
    pub fn text_prefix(field: &str) -> usize {
        Self::text_maximum(field) - 1
    }
    pub fn text_finish(prefix: &[u16]) -> Vec<u16> {
        let mut text = prefix.to_vec();
        text.push(0x2026);
        text
    }
    pub fn array_bound(field: &str) -> i32 {
        Self::array_size(field)
    }
    pub fn array_tail(field: &str) -> i32 {
        Self::array_size(field)
    }
    pub fn neighborhood_scope(page: bool) -> String {
        if page {
            "exploration-page"
        } else {
            "bounded-neighborhood"
        }
        .into()
    }
    pub fn path_finding(found: bool) -> String {
        if found {
            "route found and shown on the page"
        } else {
            "no route found within the requested bounds"
        }
        .into()
    }
    pub fn path_actions(found: bool) -> String {
        if found {
            "save the route for comparison\ninspect its uncertain edges"
        } else {
            "widen the route bounds\nrestore an excluded edge"
        }
        .into()
    }
    pub fn knowledge_next_action(is_boolean: bool, truthy: bool) -> String {
        if Self::strict_true(is_boolean, truthy) {
            "invoke this tool again with next_cursor"
        } else {
            "select one returned stable id"
        }
        .into()
    }
    pub fn word_next_action(is_boolean: bool, truthy: bool) -> String {
        if Self::strict_true(is_boolean, truthy) {
            "perform the source-bound analysis and preserve citations"
        } else {
            "install the local source-bound provider"
        }
        .into()
    }
    pub fn strict_true(is_boolean: bool, truthy: bool) -> bool {
        is_boolean && truthy
    }
    pub fn optional_text(kind: &str, nonempty: bool) -> u32 {
        if nonempty {
            0
        } else {
            match kind {
                "undefined" => 1,
                "null" => 2,
                "unresolved" => 3,
                _ => unreachable!(),
            }
        }
    }
    pub fn shape_array_needed(kind: &str, truthy: bool, is_object: bool) -> bool {
        kind == "knowledge-item" && truthy && is_object
    }
    pub fn shape(kind: &str, truthy: bool, is_object: bool, is_array: bool) -> u32 {
        match kind {
            "knowledge-item" => u32::from(truthy && is_object && !is_array),
            "knowledge-page" => u32::from(truthy && is_object),
            "evidence" | "path" => {
                if truthy {
                    1
                } else {
                    2
                }
            }
            "selection"
            | "page-selected"
            | "reading-selection"
            | "workspace-hypothesis"
            | "workspace-route"
            | "workspace-proposal"
            | "word-source"
            | "neighborhood-page"
            | "excluded-edge" => u32::from(truthy),
            _ => unreachable!(),
        }
    }
    pub fn knowledge_limit(number: f64) -> f64 {
        let selected = if number == 0.0 || number.is_nan() {
            6.0
        } else {
            number
        };
        selected.trunc().min(6.0).max(1.0)
    }
}
impl WebMcpRules {
    fn text_maximum(field: &str) -> usize {
        match field {
            "compactSelectionResult.text.0" => 24,
            "compactSelectionResult.text.1" => 48,
            "compactSelectionResult.text.2" => 120,
            "compactSelectionResult.text.3" => 100,
            "compactSelectionResult.text.4" => 48,
            "compactSelectionResult.text.5" => 48,
            "compactSelectionResult.text.6" => 48,
            "compactSelectionResult.text.7" => 48,
            "compactPageContext.text.0" => 96,
            "compactPageContext.text.1" => 24,
            "compactPageContext.text.2" => 48,
            "compactPageContext.text.3" => 120,
            "compactSearchResult.text.0" => 160,
            "compactSearchResult.text.1" => 48,
            "compactSearchResult.text.2" => 80,
            "compactSearchResult.text.3" => 56,
            "compactSearchResult.text.4" => 72,
            "compactKnowledgeSearchResult.text.0" => 48,
            "compactKnowledgeSearchResult.text.1" => 120,
            "compactKnowledgeSearchResult.text.2" => 80,
            "compactKnowledgeSearchResult.text.3" => 160,
            "compactKnowledgeSearchResult.text.4" => 96,
            "compactSourceGapResult.text.0" => 160,
            "compactSourceGapResult.text.1" => 160,
            "compactSourceGapResult.text.2" => 64,
            "compactSourceGapResult.text.3" => 180,
            "compactSourceGapResult.text.4" => 300,
            "compactNeighborhoodResult.text.0" => 120,
            "compactNeighborhoodResult.text.1" => 72,
            "compactNeighborhoodResult.text.2" => 36,
            "compactReadingComparison.text.0" => 48,
            "compactReadingComparison.text.1" => 120,
            "compactReadingComparison.text.2" => 96,
            "compactReadingComparison.text.3" => 48,
            "compactReadingComparison.text.4" => 120,
            "compactReadingComparison.text.5" => 180,
            "compactWorkspaceMutation.text.0" => 96,
            "compactWorkspaceMutation.text.1" => 96,
            "compactWorkspaceMutation.text.2" => 140,
            "compactWorkspaceMutation.text.3" => 96,
            "compactWorkspaceMutation.text.4" => 140,
            "compactWorkspaceMutation.text.5" => 48,
            "compactWorkspaceMutation.text.6" => 180,
            "compactWorkspaceMutation.text.7" => 96,
            "compactWorkspaceRead.text.0" => 80,
            "compactWorkspaceRead.text.1" => 96,
            "compactWorkspaceRead.text.2" => 120,
            "compactWorkspaceRead.text.3" => 80,
            "compactWorkspaceRead.text.4" => 72,
            "compactWorkspaceRead.text.5" => 80,
            "compactWorkspaceRead.text.6" => 96,
            "compactWorkspaceRead.text.7" => 48,
            "compactWordAnalysisResult.text.0" => 180,
            "compactWordAnalysisResult.text.1" => 96,
            "compactWordAnalysisResult.text.2" => 24,
            "compactWordAnalysisResult.text.3" => 160,
            "compactWordAnalysisResult.text.4" => 180,
            _ => unreachable!("unknown maintained WebMCP text field"),
        }
    }
    fn array_size(field: &str) -> i32 {
        match field {
            "compactSelectionResult.array.0" => 3,
            "compactPageContext.array.0" => 3,
            "compactPageContext.array.1" => 16,
            "compactPageContext.array.2" => 16,
            "compactPageContext.array.3" => 8,
            "compactSearchResult.array.0" => 6,
            "compactKnowledgeSearchResult.array.0" => 6,
            "compactKnowledgeSearchResult.array.1" => 3,
            "compactSourceGapResult.array.0" => 6,
            "compactPathResult.array.0" => 10,
            "compactNeighborhoodResult.array.0" => 6,
            "compactNeighborhoodResult.array.1" => 12,
            "compactReadingComparison.array.0" => 4,
            "compactReadingComparison.array.1" => 2,
            "compactReadingComparison.array.2" => 4,
            "compactWorkspaceRead.array.0" => -1,
            "compactWorkspaceRead.array.1" => -2,
            "compactWorkspaceRead.array.2" => -3,
            "compactWorkspaceRead.array.3" => -2,
            "compactWorkspaceRead.array.4" => -1,
            "compactWorkspaceRead.array.5" => -3,
            _ => unreachable!("unknown maintained WebMCP preview field"),
        }
    }
}

#[wasm_bindgen]
pub struct WebMcpResultChoice {
    stage: u32,
    count: u32,
    mode: &'static str,
    done: bool,
}
#[wasm_bindgen]
impl WebMcpResultChoice {
    #[wasm_bindgen(constructor)]
    pub fn new(policy: &str) -> Self {
        let (count, mode) = match policy {
            "selected-kind" => (2, "truthy"),
            "item-kind" => (2, "truthy"),
            "search-posture" => (3, "truthy"),
            "knowledge-kind" => (3, "truthy"),
            "knowledge-subtitle" => (3, "truthy"),
            "gap-posture" => (2, "truthy"),
            "result-count" => (2, "truthy"),
            "path-first" => (2, "truthy"),
            "array-preview-0" => (3, "array"),
            "array-preview-1" => (3, "array"),
            "array-preview-2" => (3, "array"),
            "array-preview-3" => (3, "array"),
            "array-preview-4" => (3, "array"),
            "array-preview-5" => (3, "array"),
            "array-preview-6" => (3, "array"),
            "array-preview-7" => (3, "array"),
            "array-preview-8" => (3, "array"),
            "array-preview-9" => (3, "array"),
            "array-preview-10" => (3, "array"),
            "array-preview-11" => (3, "array"),
            "array-preview-12" => (3, "array"),
            "array-preview-13" => (3, "array"),
            "array-preview-14" => (3, "array"),
            "array-preview-15" => (3, "array"),
            "array-preview-16" => (3, "array"),
            "array-preview-17" => (3, "array"),
            "array-preview-18" => (3, "array"),
            "array-preview-19" => (3, "array"),
            "array-preview-20" => (3, "array"),
            "array-preview-21" => (3, "array"),
            "array-preview-22" => (3, "array"),
            "array-preview-23" => (3, "array"),
            "array-preview-24" => (3, "array"),
            "array-preview-25" => (3, "array"),
            "array-preview-26" => (3, "array"),
            "array-preview-27" => (3, "array"),
            "array-preview-28" => (3, "array"),
            "search-count" => (2, "count"),
            "path-count" => (3, "truthy"),
            "neighborhood-id" => (2, "truthy"),
            "neighborhood-label" => (2, "truthy"),
            "neighbor-id" => (2, "truthy"),
            "neighbor-label" => (2, "truthy"),
            "reading-count" => (2, "truthy"),
            "workspace-summary" => (2, "truthy"),
            "workspace-changed" => (4, "nullish"),
            "proposal-status" => (2, "truthy"),
            "workspace-lens" => (2, "truthy"),
            "word-occurrence" => (2, "truthy"),
            "word-surface" => (2, "truthy"),
            "word-task-schema" => (2, "truthy"),
            "path-actions" => (2, "truthy"),
            "knowledge-page" => (4, "page"),
            "knowledge-cursor" => (4, "cursor"),
            _ => unreachable!("unknown maintained WebMCP choice"),
        };
        Self {
            stage: 0,
            count,
            mode,
            done: false,
        }
    }
    pub fn need(&self) -> u32 {
        self.stage
    }
    pub fn done(&self) -> bool {
        self.done
    }
    pub fn observe(&mut self, truthy: bool, nullish: bool, is_object: bool, is_null: bool) {
        if self.stage == self.count - 1 {
            self.done = true;
            return;
        }
        match self.mode {
            "array" => {
                if self.stage == 1 {
                    self.done = true;
                } else {
                    unreachable!("array shape needs native array observation")
                }
            }
            "count" => {
                if is_null {
                    self.done = true;
                } else {
                    self.stage += 1;
                }
            }
            "truthy" => {
                if truthy {
                    self.done = true;
                } else {
                    self.stage += 1;
                }
            }
            "nullish" => {
                if !nullish {
                    self.done = true;
                } else {
                    self.stage += 1;
                }
            }
            "page" => match self.stage {
                0 => self.stage = if truthy { 1 } else { 3 },
                1 => self.stage = if is_object { 2 } else { 3 },
                2 => self.done = true,
                _ => unreachable!(),
            },
            "cursor" => match self.stage {
                0 => self.stage = if !nullish && is_object { 3 } else { 1 },
                1 => self.stage = if truthy { 2 } else { 3 },
                2 => self.done = true,
                _ => unreachable!(),
            },
            _ => unreachable!(),
        }
    }
    pub fn needs_array(&self) -> bool {
        self.mode == "array" && self.stage == 0
    }
    pub fn observe_array(&mut self, array: bool) {
        self.stage = if array { 1 } else { 2 };
    }
    pub fn observe_string(&mut self, is_string: bool, truthy: bool) {
        if self.mode == "cursor" && self.stage == 0 {
            self.stage = if is_string { 1 } else { 3 };
        } else {
            self.observe(truthy, false, false, false);
        }
    }
}

#[wasm_bindgen]
pub struct WebMcpToolSession {
    phase: &'static str,
}
#[wasm_bindgen]
impl WebMcpToolSession {
    #[wasm_bindgen(constructor)]
    pub fn new(selected: bool) -> Self {
        Self {
            phase: if selected { "base" } else { "done" },
        }
    }
    pub fn need(&self) -> String {
        self.phase.into()
    }
    pub fn literal(&self) -> String {
        match self.phase {
            "evidence-mode-first" | "philosophy-mode" => "philosophy",
            "evidence-mode-corpus" => "corpus",
            "evidence-view" => "route-graph",
            "evidence-kind-node" | "node-kind" => "node",
            "evidence-kind-edge" | "mutation-kind" | "reroute-kind" => "edge",
            _ => unreachable!("tool phase has no string equality"),
        }
        .into()
    }
    pub fn emitted(&mut self) {
        self.phase = match self.phase {
            "base" => "evidence-mode-first",
            "evidence-tools" => "mutation-kind",
            "mutation-tools" => "philosophy-mode",
            "neighborhood-tool" => "start-available",
            "start-tool" => "find-available",
            "find-tool" => "reroute-kind",
            "reroute-tool" => "done",
            _ => unreachable!("tool phase has no emission"),
        };
    }
    pub fn observe_optional(&mut self, truthy: bool, nullish: bool) {
        assert_eq!(self.phase, "evidence-available");
        self.phase = if nullish {
            "evidence-reroutable"
        } else if truthy {
            "evidence-tools"
        } else {
            "mutation-kind"
        };
    }
    pub fn observe_truthy(&mut self, truthy: bool) {
        self.phase = match self.phase {
            "mutation-from" => {
                if truthy {
                    "mutation-to"
                } else {
                    "philosophy-mode"
                }
            }
            "mutation-to" => {
                if truthy {
                    "mutation-tools"
                } else {
                    "philosophy-mode"
                }
            }
            "path-start" => {
                if truthy {
                    "path-distinct"
                } else {
                    "reroute-kind"
                }
            }
            "reroute-from" => {
                if truthy {
                    "reroute-to"
                } else {
                    "done"
                }
            }
            "reroute-to" => {
                if truthy {
                    "reroute-tool"
                } else {
                    "done"
                }
            }
            _ => unreachable!("tool phase has no truthiness observation"),
        };
    }
    pub fn observe_equal(&mut self, equal: bool) {
        self.phase = match self.phase {
            "evidence-mode-first" => {
                if equal {
                    "evidence-kind-node"
                } else {
                    "evidence-mode-corpus"
                }
            }
            "evidence-mode-corpus" => {
                if equal {
                    "evidence-view"
                } else {
                    "done"
                }
            }
            "evidence-view" => {
                if equal {
                    "evidence-kind-node"
                } else {
                    "done"
                }
            }
            "evidence-kind-node" => {
                if equal {
                    "evidence-available"
                } else {
                    "evidence-kind-edge"
                }
            }
            "evidence-kind-edge" => {
                if equal {
                    "evidence-available"
                } else {
                    "mutation-kind"
                }
            }
            "evidence-reroutable" => {
                if equal {
                    "mutation-kind"
                } else {
                    "evidence-tools"
                }
            }
            "mutation-kind" => {
                if equal {
                    "mutation-reroutable"
                } else {
                    "philosophy-mode"
                }
            }
            "mutation-reroutable" => {
                if equal {
                    "philosophy-mode"
                } else {
                    "mutation-from"
                }
            }
            "philosophy-mode" => {
                if equal {
                    "layers"
                } else {
                    "done"
                }
            }
            "layers" => {
                if equal {
                    "done"
                } else {
                    "predicates"
                }
            }
            "predicates" => {
                if equal {
                    "done"
                } else {
                    "node-kind"
                }
            }
            "node-kind" => {
                if equal {
                    "neighborhood-tool"
                } else {
                    "reroute-kind"
                }
            }
            "start-available" => {
                if equal {
                    "find-available"
                } else {
                    "start-tool"
                }
            }
            "find-available" => {
                if equal {
                    "reroute-kind"
                } else {
                    "path-start"
                }
            }
            "path-distinct" => {
                if equal {
                    "reroute-kind"
                } else {
                    "find-tool"
                }
            }
            "reroute-kind" => {
                if equal {
                    "reroute-reroutable"
                } else {
                    "done"
                }
            }
            "reroute-reroutable" => {
                if equal {
                    "done"
                } else {
                    "reroute-from"
                }
            }
            _ => unreachable!("tool phase has no equality observation"),
        };
    }
}

/// Pure routing selector state. Native Set construction/membership and raw
/// opaque return values stay in the page-command host adapter.
#[wasm_bindgen]
pub struct WebMcpViewSelectorSession {
    phase: &'static str,
}
#[wasm_bindgen]
impl WebMcpViewSelectorSession {
    #[wasm_bindgen(constructor)]
    pub fn new(mode: &str, truthy: bool) -> Self {
        Self {
            phase: match mode {
                "known" => {
                    if truthy {
                        "known-membership"
                    } else {
                        "empty-error"
                    }
                }
                "reload" => "selected",
                _ => unreachable!(),
            },
        }
    }
    pub fn need(&self) -> String {
        self.phase.into()
    }
    pub fn value(&mut self, truthy: bool) {
        self.phase = match self.phase {
            "selected" => {
                if truthy {
                    "selected-membership"
                } else {
                    "graph"
                }
            }
            "graph" => {
                if truthy {
                    "graph-membership"
                } else {
                    "empty"
                }
            }
            _ => unreachable!(),
        };
    }
    pub fn membership(&mut self, present: bool) {
        self.phase = match self.phase {
            "known-membership" => {
                if present {
                    "done"
                } else {
                    "unknown-error"
                }
            }
            "selected-membership" => {
                if present {
                    "selected-result"
                } else {
                    "graph"
                }
            }
            "graph-membership" => {
                if present {
                    "graph-result"
                } else {
                    "empty"
                }
            }
            _ => unreachable!(),
        };
    }
    pub fn empty_label() -> String {
        "(empty)".into()
    }
}
