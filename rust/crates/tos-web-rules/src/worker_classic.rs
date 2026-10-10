//! Explicit compiled-D1 Worker compatibility profile. Numeric keys represent
//! host-interned exact JS string identity. This profile has no selected native
//! publication, current grant, corpus admission, or whole-graph input.
//! Numeric limits are the existing HTTP integer-bounded caller values. Physical
//! rows are decoded JSON; arbitrary accessor/method overrides are not admitted.
use std::collections::{BTreeMap, BTreeSet};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct WorkerNeighborhood {
    depth: usize,
    limit: usize,
    level: usize,
    root: u32,
    selected: BTreeSet<u32>,
    discovery: Vec<u32>,
    frontier: Vec<u32>,
    candidates: BTreeMap<u32, (u32, u32)>,
    edges: Vec<u32>,
    edge_ids: BTreeSet<u32>,
}
#[wasm_bindgen]
impl WorkerNeighborhood {
    #[wasm_bindgen(constructor)]
    pub fn new(root: u32, depth: usize, limit: usize) -> Self {
        Self {
            depth,
            limit,
            level: 0,
            root,
            selected: BTreeSet::from([root]),
            discovery: vec![root],
            frontier: vec![root],
            candidates: BTreeMap::new(),
            edges: vec![],
            edge_ids: BTreeSet::new(),
        }
    }
    pub fn active(&self) -> bool {
        self.level < self.depth
            && !self.frontier.is_empty()
            && self.discovery.len() - 1 < self.limit
    }
    pub fn frontier(&self) -> Vec<u32> {
        self.frontier.clone()
    }
    pub fn observe_edge(&mut self, id: u32, from: u32, to: u32, row: u32) {
        for (a, b) in [(from, to), (to, from)] {
            if self.frontier.contains(&a) && !self.selected.contains(&b) {
                self.candidates.entry(b).or_insert((id, row));
            }
        }
    }
    pub fn candidates(&self) -> Vec<u32> {
        self.candidates.keys().copied().collect()
    }
    /// The host supplies SQL ord then its native localeCompare tie order.
    pub fn finish_level(&mut self, allowed: &[u32]) {
        let mut next = vec![];
        for id in allowed {
            if self.discovery.len() - 1 >= self.limit {
                break;
            }
            if let Some((edge, row)) = self.candidates.get(id) {
                self.selected.insert(*id);
                self.discovery.push(*id);
                next.push(*id);
                if self.edge_ids.insert(*edge) {
                    self.edges.push(*row);
                }
            }
        }
        self.candidates.clear();
        self.frontier = next;
        self.level += 1;
    }
    pub fn selected(&self) -> Vec<u32> {
        self.discovery.clone()
    }
    pub fn neighbors(&self) -> Vec<u32> {
        self.discovery
            .iter()
            .copied()
            .filter(|id| *id != self.root)
            .collect()
    }
    pub fn edges(&self) -> Vec<u32> {
        self.edges.clone()
    }
    pub fn enclosed_needed(&self) -> bool {
        self.edges.len() < self.limit
    }
    pub fn observe_enclosed(&mut self, id: u32, row: u32) {
        if self.edges.len() < self.limit && self.edge_ids.insert(id) {
            self.edges.push(row);
        }
    }
}
#[derive(Clone)]
struct PathState {
    current: u32,
    nodes: Vec<u32>,
    edges: Vec<u32>,
    directions: Vec<u32>,
}
#[wasm_bindgen]
pub struct WorkerPath {
    target: u32,
    depth: usize,
    direction: u8,
    alternatives: usize,
    excluded: BTreeSet<u32>,
    queue: Vec<PathState>,
    completed: Vec<PathState>,
    adjacency: BTreeMap<u32, Vec<(u32, u32, u32)>>,
    explored: usize,
    enqueued: usize,
    max_frontier: usize,
    truncated: bool,
}
#[wasm_bindgen]
impl WorkerPath {
    #[wasm_bindgen(constructor)]
    pub fn new(
        from: u32,
        to: u32,
        depth: usize,
        direction: u8,
        alternatives: usize,
        excluded: &[u32],
    ) -> Self {
        Self {
            target: to,
            depth,
            direction,
            alternatives,
            excluded: excluded.iter().copied().collect(),
            queue: vec![PathState {
                current: from,
                nodes: vec![from],
                edges: vec![],
                directions: vec![],
            }],
            completed: vec![],
            adjacency: BTreeMap::new(),
            explored: 0,
            enqueued: 1,
            max_frontier: 1,
            truncated: false,
        }
    }
    pub fn active(&mut self) -> bool {
        if self.explored >= 50_000
            && !self.queue.is_empty()
            && self.completed.len() < self.alternatives
        {
            self.truncated = true;
            false
        } else {
            !self.queue.is_empty() && self.completed.len() < self.alternatives
        }
    }
    pub fn current_ids(&self) -> Vec<u32> {
        let mut seen = BTreeSet::new();
        self.queue
            .iter()
            .filter_map(|s| seen.insert(s.current).then_some(s.current))
            .collect()
    }
    pub fn observe_edge(&mut self, id: u32, from: u32, to: u32) {
        if self.excluded.contains(&id) {
            return;
        }
        if self.direction == 0 || self.direction == 2 {
            self.adjacency.entry(from).or_default().push((to, id, 0));
        }
        if (self.direction == 1 || self.direction == 2) && (from != to || self.direction == 1) {
            self.adjacency.entry(to).or_default().push((from, id, 1));
        }
    }
    pub fn finish_level(&mut self) {
        let mut next = vec![];
        for state in &self.queue {
            if self.explored >= 50_000 || self.completed.len() >= self.alternatives {
                break;
            }
            self.explored += 1;
            if state.current == self.target {
                self.completed.push(state.clone());
                continue;
            }
            if state.edges.len() >= self.depth {
                continue;
            }
            for (neighbor, edge, direction) in
                self.adjacency.get(&state.current).into_iter().flatten()
            {
                if state.nodes.contains(neighbor) {
                    continue;
                }
                if self.enqueued >= 50_000 || next.len() >= 5_000 {
                    self.truncated = true;
                    break;
                }
                let mut s = state.clone();
                s.current = *neighbor;
                s.nodes.push(*neighbor);
                s.edges.push(*edge);
                s.directions.push(*direction);
                next.push(s);
                self.enqueued += 1;
            }
        }
        self.queue = next;
        self.max_frontier = self.max_frontier.max(self.queue.len());
        self.adjacency.clear();
    }
    pub fn path_count(&self) -> usize {
        self.completed.len()
    }
    pub fn path_nodes(&self, index: usize) -> Vec<u32> {
        self.completed[index].nodes.clone()
    }
    pub fn path_edges(&self, index: usize) -> Vec<u32> {
        self.completed[index].edges.clone()
    }
    pub fn path_directions(&self, index: usize) -> Vec<u32> {
        self.completed[index].directions.clone()
    }
    pub fn explored(&self) -> usize {
        self.explored
    }
    pub fn enqueued(&self) -> usize {
        self.enqueued
    }
    pub fn max_frontier(&self) -> usize {
        self.max_frontier
    }
    pub fn truncated(&self) -> bool {
        self.truncated
    }
}

/// Shared profile-local selection over physical row indexes. Host supplies only
/// exact identity keys; all membership, incidence and capacity choices are here.
#[wasm_bindgen]
pub struct WorkerGraph {
    available: BTreeSet<u32>,
    selected: BTreeSet<u32>,
    edges: Vec<u32>,
    limit: usize,
}
#[wasm_bindgen]
impl WorkerGraph {
    #[wasm_bindgen(constructor)]
    pub fn new(limit: usize) -> Self {
        Self {
            available: BTreeSet::new(),
            selected: BTreeSet::new(),
            edges: vec![],
            limit,
        }
    }
    pub fn observe_node(&mut self, id: u32) {
        self.available.insert(id);
    }
    pub fn observe_edge(&mut self, from: u32, to: u32, row: u32) -> bool {
        if self.edges.len() >= self.limit {
            return false;
        }
        if !self.available.contains(&from) || !self.available.contains(&to) {
            return false;
        }
        let additions = BTreeSet::from([from, to])
            .difference(&self.selected)
            .count();
        if self.selected.len() + additions > self.limit {
            return false;
        }
        self.selected.insert(from);
        self.selected.insert(to);
        self.edges.push(row);
        true
    }
    pub fn edge_capacity(&self) -> bool {
        self.edges.len() < self.limit
    }
    pub fn node_capacity(&self) -> bool {
        self.selected.len() < self.limit
    }
    pub fn fill_node(&mut self, id: u32, nonempty: bool) {
        if self.selected.len() < self.limit && nonempty {
            self.selected.insert(id);
        }
    }
    pub fn selected(&self, id: u32) -> bool {
        self.selected.contains(&id)
    }
    pub fn edge_rows(&self) -> Vec<u32> {
        self.edges.clone()
    }
}
#[wasm_bindgen]
pub struct WorkerEpistemic {
    limit: usize,
    capacity: usize,
    available: usize,
    challenges: Vec<u32>,
    challenge_ids: BTreeSet<u32>,
    candidates: Vec<(u32, u32)>,
    context: Vec<u32>,
    related: BTreeSet<u32>,
}
#[wasm_bindgen]
impl WorkerEpistemic {
    #[wasm_bindgen(constructor)]
    pub fn new(limit: usize, selected_edge_context: bool) -> Self {
        Self {
            limit,
            capacity: limit.saturating_sub(usize::from(selected_edge_context)),
            available: 0,
            challenges: vec![],
            challenge_ids: BTreeSet::new(),
            candidates: vec![],
            context: vec![],
            related: BTreeSet::new(),
        }
    }
    pub fn challenge(predicate: &str) -> bool {
        matches!(
            predicate,
            "contested_by" | "uncertain_relation" | "polemicizes_with"
        )
    }
    pub fn observe_relation(&mut self, id: u32, row: u32, challenge: bool) {
        self.candidates.push((id, row));
        if challenge {
            self.available += 1;
            if self.challenges.len() < self.capacity {
                self.challenges.push(row);
                self.challenge_ids.insert(id);
            }
        }
    }
    pub fn finish_relations(&mut self) {
        self.context = self
            .candidates
            .iter()
            .filter(|(id, _)| !self.challenge_ids.contains(id))
            .take(self.limit.saturating_sub(self.challenges.len()))
            .map(|(_, row)| *row)
            .collect();
    }
    pub fn challenge_rows(&self) -> Vec<u32> {
        self.challenges.clone()
    }
    pub fn context_rows(&self) -> Vec<u32> {
        self.context.clone()
    }
    pub fn available(&self) -> usize {
        self.available
    }
    pub fn challenge_state(&self) -> String {
        if self.challenges.len() < self.available {
            "projected_signals_truncated"
        } else if self.available > 0 {
            "projected_signals"
        } else {
            "none_in_projection_scope"
        }
        .into()
    }
    pub fn observe_endpoint(&mut self, id: u32, nonempty: bool) {
        if nonempty {
            self.related.insert(id);
        }
    }
    pub fn neighbor(&self, id: u32, selected_node: bool, selection: u32) -> bool {
        !selected_node || id != selection
    }
    pub fn corpus_neighbor(&self, id: u32, selected_node: bool, selection: u32) -> bool {
        (!selected_node || id != selection) && self.related.contains(&id)
    }
    pub fn corpus_candidate(id: u32, selection: u32, from: u32, to: u32, nodes: &[u32]) -> bool {
        id == selection || nodes.contains(&from) || nodes.contains(&to)
    }
}
#[wasm_bindgen]
pub struct WorkerSet {
    seen: BTreeSet<u32>,
    order: Vec<u32>,
}
#[wasm_bindgen]
impl WorkerSet {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            seen: BTreeSet::new(),
            order: vec![],
        }
    }
    pub fn add(&mut self, id: u32, eligible: bool) -> bool {
        if eligible && self.seen.insert(id) {
            self.order.push(id);
            true
        } else {
            false
        }
    }
    pub fn has(&self, id: u32) -> bool {
        self.seen.contains(&id)
    }
    pub fn remove(&mut self, id: u32) {
        self.seen.remove(&id);
        self.order.retain(|value| *value != id);
    }
    pub fn values(&self) -> Vec<u32> {
        self.order.clone()
    }
}
#[wasm_bindgen]
pub struct WorkerEvidence {
    scene: Option<u32>,
    selection: u32,
    mode: u8,
}
#[wasm_bindgen]
impl WorkerEvidence {
    #[wasm_bindgen(constructor)]
    pub fn new(mode: u8, selection: u32) -> Self {
        Self {
            scene: None,
            selection,
            mode,
        }
    }
    pub fn route(&mut self, scene: u32, mode: &str, items: &[u32]) {
        if self.scene.is_none()
            && mode
                == if self.mode == 0 {
                    "philosophy"
                } else {
                    "corpus"
                }
            && items.contains(&self.selection)
        {
            self.scene = Some(scene);
        }
    }
    pub fn scene(&self) -> f64 {
        self.scene.map(|value| value as f64).unwrap_or(-1.0)
    }
    pub fn curated(&self) -> bool {
        self.scene.is_some()
    }
    pub fn field_fallback(length: usize) -> bool {
        length == 0
    }
    pub fn conclusion_record(present: bool, is_object: bool) -> bool {
        present && is_object
    }
    pub fn canon_layer(layer: &str) -> bool {
        layer == "canon"
    }
    pub fn coverage_posture() -> String {
        "curated-route".into()
    }
    /// Output-only fallback; no source fields, graph or packet are serialized.
    pub fn fallback(canon: bool) -> String {
        let mut output=r#"{"finding":"No curated Evidence Lens route is published for this selection.","finding_ru":"Для выбранного объекта ещё не опубликован курируемый маршрут Evidence Lens.","posture":"projection-only","conclusion":{"can_conclude":false,"canon_membership":CANON,"claim_evidence_closed":false,"allowed":["inspect the projection context and its source-return references"],"allowed_ru":["исследовать контекст проекции и её ссылки возврата к источникам"],"not_allowed":["infer evidence closure from projection membership"],"not_allowed_ru":["выводить доказательную замкнутость из присутствия в проекции"]},"gaps":["curated source, review, rights, and claim/evidence routes"],"gaps_ru":["курируемые маршруты к source, review, rights и claim/evidence"]}"#.to_owned();
        output = output.replace("CANON", if canon { "true" } else { "false" });
        output
    }
    /// Ordinary numeric tally belongs to Rust. The host retains original JS
    /// coercion for inherited nonnumeric raw table slots.
    pub fn count_route(&self, current: f64) -> f64 {
        current + 1.0
    }
}

#[wasm_bindgen]
pub struct WorkerClusters {
    nodes: BTreeSet<u32>,
    edges: BTreeSet<u32>,
}
#[wasm_bindgen]
impl WorkerClusters {
    #[wasm_bindgen(constructor)]
    pub fn new(nodes: &[u32], edges: &[u32]) -> Self {
        Self {
            nodes: nodes.iter().copied().collect(),
            edges: edges.iter().copied().collect(),
        }
    }
    pub fn node(&self, id: u32) -> bool {
        self.nodes.contains(&id)
    }
    pub fn edge(&self, id: u32) -> bool {
        self.edges.contains(&id)
    }
    pub fn retain(nodes: usize, edges: usize) -> bool {
        nodes != 0 || edges != 0
    }
}

#[wasm_bindgen]
pub struct WorkerEndpoints {
    ids: BTreeSet<u32>,
    order: Vec<u32>,
    refs: BTreeMap<u32, BTreeSet<u32>>,
}
#[wasm_bindgen]
impl WorkerEndpoints {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            ids: BTreeSet::new(),
            order: vec![],
            refs: BTreeMap::new(),
        }
    }
    pub fn endpoint(&mut self, id: u32, nonempty: bool) {
        if nonempty && self.ids.insert(id) {
            self.order.push(id);
            self.refs.insert(id, BTreeSet::new());
        }
    }
    pub fn reference(&mut self, id: u32, reference: u32, nonempty: bool) {
        if nonempty {
            if let Some(refs) = self.refs.get_mut(&id) {
                refs.insert(reference);
            }
        }
    }
    pub fn ids(&self) -> Vec<u32> {
        self.order.clone()
    }
    pub fn contains(&self, id: u32) -> bool {
        self.ids.contains(&id)
    }
    pub fn references(&self, id: u32) -> Vec<u32> {
        self.refs
            .get(&id)
            .map(|refs| refs.iter().copied().collect())
            .unwrap_or_default()
    }
    pub fn graph_mode(view: &str) -> u8 {
        match view {
            "corpus-topology" => 0,
            "route-graph" => 1,
            "promotion-flow" => 2,
            _ => 3,
        }
    }
}
#[wasm_bindgen]
pub struct WorkerSearch {
    limit: usize,
    count: usize,
}
#[wasm_bindgen]
impl WorkerSearch {
    // Native ECMAScript string intrinsics remain host observations. Rust owns
    // their maintained query-normalization order; no Python Unicode substitute.
    pub fn needle_order() -> Vec<u8> {
        vec![0, 1]
    }

    #[wasm_bindgen(constructor)]
    pub fn new(limit: usize) -> Self {
        Self { limit, count: 0 }
    }
    pub fn remaining(&self) -> usize {
        self.limit.saturating_sub(self.count)
    }
    pub fn observed(&mut self, count: usize) {
        self.count += count;
    }
}
#[wasm_bindgen]
pub struct WorkerHealth {}
#[wasm_bindgen]
impl WorkerHealth {
    pub fn schema(schema: &str) -> bool {
        schema == "tos_knowledge_graph_v1"
    }
    pub fn expected(is_number: bool) -> bool {
        is_number
    }
    pub fn coverage(expected: f64, actual: f64, is_number: bool) -> bool {
        is_number && expected == actual
    }
}

#[wasm_bindgen]
pub struct WorkerProfile {}
#[wasm_bindgen]
impl WorkerProfile {
    pub fn direction(value: &str) -> u8 {
        match value {
            "outgoing" => 0,
            "incoming" => 1,
            "either" => 2,
            _ => 3,
        }
    }
    pub fn priority(left_selected: bool, right_selected: bool) -> i32 {
        i32::from(!left_selected) - i32::from(!right_selected)
    }
    pub fn same(left: u32, right: u32) -> bool {
        left == right
    }
    pub fn different(left: u32, right: u32) -> bool {
        left != right
    }
    pub fn outside_view(row: i32, view: i32) -> bool {
        row & view == 0
    }
    pub fn endpoints_missing(count: usize, same: bool) -> bool {
        count < 2 && !same
    }
    pub fn node_fetch_limit(limit: usize) -> usize {
        limit.saturating_mul(2).max(limit)
    }
    pub fn edge_fetch_limit(limit: usize) -> usize {
        limit.saturating_mul(8).max(1000).min(13_307)
    }
    pub fn supported_corpus_evidence(view: &str) -> bool {
        view == "route-graph"
    }
    pub fn missing(count: usize) -> bool {
        count == 0
    }
    pub fn challenge_predicates() -> String {
        r#"["contested_by","polemicizes_with","uncertain_relation"]"#.into()
    }
    /// Fixed output-only SQL collection descriptors, no source packet input.
    pub fn search_collections() -> String {
        r#"[{"name":"views","table":"philosophy_aux","where":"collection = 'views'"},{"name":"nodes","table":"philosophy_nodes","where":"1 = 1"},{"name":"edges","table":"philosophy_edges","where":"1 = 1"},{"name":"clusters","table":"philosophy_aux","where":"collection = 'clusters'"},{"name":"review_packets","table":"philosophy_aux","where":"collection = 'review_packets'"},{"name":"graph_layers","table":"philosophy_aux","where":"collection = 'graph_layers'"}]"#.into()
    }
}
