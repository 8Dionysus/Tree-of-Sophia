//! Worker source-navigation projection over original row handles.
//! No payload custody, permission grant, native carrier admission or new budget.
use std::collections::{HashMap, HashSet, VecDeque};
use wasm_bindgen::prelude::*;
fn bib(p: &str) -> bool {
    matches!(p, "has_expression" | "embodied_by" | "exemplified_by")
}
fn link(p: &str) -> bool {
    matches!(
        p,
        "described_by" | "metadata_at" | "downloadable_at" | "rights_statement_at"
    )
}
#[derive(Clone)]
struct Edge {
    id: u32,
    row: u32,
    from: u32,
    to: u32,
    kind: String,
}
#[wasm_bindgen]
pub struct WorkerSourceWalk {
    root: u32,
    empty: u32,
    paths: Vec<(Vec<u32>, Vec<u32>)>,
    references: Vec<u32>,
    reference_seen: HashSet<u32>,
    limit: usize,
    max_depth: f64,
    dossier: bool,
    phase: u8,
    step: &'static str,
    nodes: Vec<u32>,
    kinds: HashMap<u32, String>,
    depths: HashMap<u32, u32>,
    edges: Vec<Edge>,
    edge_positions: HashMap<u32, usize>,
    queue: VecDeque<(u32, u32)>,
    visited: HashSet<u32>,
    roots: Vec<u32>,
    current: u32,
    depth: u32,
    pending: Option<Edge>,
    truncated: bool,
}
#[wasm_bindgen]
impl WorkerSourceWalk {
    #[wasm_bindgen(constructor)]
    pub fn new(
        root: u32,
        empty: u32,
        kind: &str,
        dossier: bool,
        max_depth: f64,
        limit: usize,
    ) -> Self {
        let phase = if !dossier {
            0
        } else if kind == "link" {
            1
        } else {
            2
        };
        let mut s = Self {
            root,
            empty,
            paths: vec![],
            references: vec![],
            reference_seen: HashSet::new(),
            limit,
            max_depth,
            dossier,
            phase,
            step: "",
            nodes: vec![root],
            kinds: HashMap::from([(root, kind.into())]),
            depths: HashMap::from([(root, 0)]),
            edges: vec![],
            edge_positions: HashMap::new(),
            queue: VecDeque::from([(root, 0)]),
            visited: HashSet::new(),
            roots: vec![],
            current: root,
            depth: 0,
            pending: None,
            truncated: false,
        };
        s.advance();
        s
    }
    pub fn semantic_predicates() -> String {
        "[\"has_expression\",\"embodied_by\",\"exemplified_by\",\"described_by\",\"metadata_at\",\"downloadable_at\",\"rights_statement_at\"]".into()
    }
    pub fn packet_filtered(value: &str) -> bool {
        !value.is_empty()
    }
    pub fn supported(kind: &str) -> bool {
        kind == "work" || kind == "link"
    }
    pub fn need(&self) -> String {
        self.step.into()
    }
    pub fn current(&self) -> u32 {
        self.current
    }
    pub fn incoming(&self) -> bool {
        matches!(self.phase, 1 | 3 | 4)
    }
    pub fn semantic(&self) -> bool {
        self.phase == 2
    }
    pub fn sorting_ids(&self) -> Vec<u32> {
        self.roots.clone()
    }
    pub fn sorted(&mut self, ids: &[u32]) {
        self.queue = ids.iter().map(|&id| (id, 0)).collect();
        self.visited.clear();
        if self.phase == 3 {
            self.roots.clear();
        }
        self.advance();
    }
    pub fn contains(&self, id: u32) -> bool {
        self.kinds.contains_key(&id)
    }
    pub fn target(&self) -> u32 {
        let e = self.pending.as_ref().unwrap();
        if matches!(self.phase, 1 | 3 | 4) {
            e.from
        } else {
            e.to
        }
    }
    pub fn edge(
        &mut self,
        id: u32,
        row: u32,
        from: u32,
        to: u32,
        kind: &str,
        predicate: &str,
    ) -> bool {
        let current_kind = self
            .kinds
            .get(&self.current)
            .map(String::as_str)
            .unwrap_or("");
        let eligible = match self.phase {
            0 => to != self.empty,
            1 => {
                kind == "evidence_claim"
                    && if current_kind == "link" {
                        link(predicate)
                    } else {
                        bib(predicate)
                    }
            }
            2 => true,
            3 => kind == "authored_source_planting",
            4 => {
                kind == "authored_branch_hierarchy"
                    || (current_kind == "source_planting"
                        && kind == "authored_source_planting"
                        && predicate == "has_source_planting")
            }
            _ => false,
        };
        if !eligible {
            return false;
        }
        let edge = Edge {
            id,
            row,
            from,
            to,
            kind: kind.into(),
        };
        let target = if matches!(self.phase, 1 | 3 | 4) {
            from
        } else {
            to
        };
        self.pending = Some(edge);
        if self.kinds.contains_key(&target) {
            self.admit_pending(target, None);
            false
        } else {
            true
        }
    }
    pub fn retained(&self, id: u32, row: u32) -> bool {
        self.edge_positions
            .get(&id)
            .is_some_and(|&index| self.edges[index].row == row)
    }
    pub fn loaded(&mut self, exists: bool, kind: &str) {
        let target = self.target();
        if exists {
            self.admit_pending(target, Some(kind.into()));
        } else {
            self.pending = None;
        }
    }
    pub fn finish_edges(&mut self) {
        self.advance();
    }
    pub fn node_ids(&self) -> Vec<u32> {
        self.nodes.clone()
    }
    pub fn node_depth(&self, id: u32) -> u32 {
        *self.depths.get(&id).unwrap_or(&0)
    }
    pub fn edge_rows(&self) -> Vec<u32> {
        self.edges.iter().map(|e| e.row).collect()
    }
    pub fn truncated(&self) -> bool {
        self.truncated
    }
    pub fn chain_kinds() -> String {
        "[\"branch\",\"era\",\"region\",\"tradition\",\"source_planting\",\"work\",\"expression\",\"edition\",\"item\",\"file\",\"link\"]".into()
    }
    pub fn kind_matches(&self, id: u32, kind: &str) -> bool {
        self.kinds.get(&id).is_some_and(|v| v == kind)
    }
    pub fn decision_ids(&self) -> Vec<u32> {
        if self.kinds.get(&self.root).is_some_and(|k| k == "link") {
            let mut ids = vec![];
            for e in &self.edges {
                if e.to == self.root && e.kind == "evidence_claim" && !ids.contains(&e.from) {
                    ids.push(e.from);
                }
            }
            ids
        } else {
            vec![self.root]
        }
    }
    pub fn prepare_paths(&mut self, eras: &[u32], ordered_rows: &[u32]) {
        self.paths = eras
            .iter()
            .filter_map(|&era| self.path(era, ordered_rows))
            .collect();
    }
    pub fn path_count(&self) -> usize {
        self.paths.len()
    }
    pub fn path_nodes(&self, index: usize) -> Vec<u32> {
        self.paths[index].0.clone()
    }
    pub fn path_edges(&self, index: usize) -> Vec<u32> {
        self.paths[index].1.clone()
    }
    pub fn reference(&mut self, id: u32, nonempty: bool) {
        if nonempty && self.reference_seen.insert(id) {
            self.references.push(id);
        }
    }
    pub fn references(&self) -> Vec<u32> {
        self.references.clone()
    }
}
impl WorkerSourceWalk {
    fn advance(&mut self) {
        loop {
            if let Some((id, depth)) = self.queue.pop_front() {
                if self.phase == 0 {
                    if f64::from(depth) >= self.max_depth {
                        continue;
                    }
                } else {
                    if !self.visited.insert(id) {
                        continue;
                    }
                    if self.phase == 1 && self.kind_matches(id, "work") {
                        self.roots.push(id);
                        continue;
                    }
                }
                self.current = id;
                self.depth = depth;
                self.step = "edges";
                return;
            }
            match self.phase {
                1 => {
                    if self.roots.is_empty() {
                        self.roots = self
                            .nodes
                            .iter()
                            .copied()
                            .filter(|id| !self.kind_matches(*id, "link"))
                            .collect();
                    }
                    self.phase = 2;
                    self.step = "sort";
                    return;
                }
                2 => {
                    self.roots = self
                        .nodes
                        .iter()
                        .copied()
                        .filter(|id| self.kind_matches(*id, "work"))
                        .collect();
                    self.phase = 3;
                    self.step = "sort";
                    return;
                }
                3 => {
                    self.phase = 4;
                    self.visited.clear();
                    self.queue = std::mem::take(&mut self.roots)
                        .into_iter()
                        .map(|id| (id, 0))
                        .collect();
                }
                _ => {
                    self.step = "done";
                    return;
                }
            }
        }
    }
    fn admit_pending(&mut self, target: u32, kind: Option<String>) {
        let edge = self.pending.take().unwrap();
        let fresh = !self.kinds.contains_key(&target);
        if fresh {
            if self.nodes.len() >= self.limit {
                self.truncated = true;
                return;
            }
            self.nodes.push(target);
            self.kinds.insert(target, kind.unwrap());
            self.depths.insert(target, self.depth + 1);
        }
        if let Some(&index) = self.edge_positions.get(&edge.id) {
            if self.dossier {
                self.edges[index] = edge;
            }
        } else {
            self.edge_positions.insert(edge.id, self.edges.len());
            self.edges.push(edge);
        }
        match self.phase {
            0 => {
                if fresh {
                    self.queue.push_back((target, self.depth + 1));
                }
            }
            1 | 2 | 4 => self.queue.push_back((target, 0)),
            3 => self.roots.push(target),
            _ => {}
        }
    }
    fn path(&self, era: u32, rows: &[u32]) -> Option<(Vec<u32>, Vec<u32>)> {
        let by_row: HashMap<u32, &Edge> = self.edges.iter().map(|edge| (edge.row, edge)).collect();
        let mut outgoing: HashMap<u32, Vec<&Edge>> = HashMap::new();
        for row in rows {
            if let Some(&edge) = by_row.get(row) {
                outgoing.entry(edge.from).or_default().push(edge);
            }
        }
        let mut queue = VecDeque::from([(era, vec![era], vec![])]);
        let mut seen = HashSet::from([era]);
        while let Some((current, nodes, edges)) = queue.pop_front() {
            if current == self.root {
                return Some((nodes, edges));
            }
            for edge in outgoing.get(&current).into_iter().flatten() {
                if edge.to != self.empty && seen.insert(edge.to) {
                    let mut n = nodes.clone();
                    n.push(edge.to);
                    let mut e = edges.clone();
                    e.push(edge.row);
                    queue.push_back((edge.to, n, e));
                }
            }
        }
        None
    }
}
#[derive(Clone)]
struct Right {
    row: u32,
    source: u32,
    nonempty: bool,
    scopes: HashSet<u32>,
    explicit: bool,
    assessment: String,
    root: bool,
    layer: bool,
    status: String,
    posture: String,
    review: String,
}
#[derive(Clone)]
struct Member {
    item: u32,
    file: u32,
    refs: HashSet<u32>,
    legacy: bool,
    valid: bool,
    sources: HashSet<u32>,
    manifests: HashSet<u32>,
}
fn rights_id(units: &[u16]) -> bool {
    let prefix: Vec<u16> = "tos.rights.".encode_utf16().collect();
    if !units.starts_with(&prefix) {
        return false;
    }
    let body = &units[prefix.len()..];
    !body.is_empty()
        && body.split(|c| *c == 46 || *c == 45).all(|p| {
            !p.is_empty()
                && p.iter()
                    .all(|c| (97..=122).contains(c) || (48..=57).contains(c))
        })
}
fn layer_id(units: &[u16]) -> bool {
    let prefix_length = "tos.rights.".len();
    rights_id(units)
        && units[prefix_length..]
            .windows(7)
            .enumerate()
            .any(|(index, part)| index > 0 && part == [46, 108, 97, 121, 101, 114, 46])
}

#[wasm_bindgen]
pub struct WorkerSourceRights {
    records: Vec<Right>,
    components: HashMap<u32, String>,
    members: Vec<Member>,
    owners: HashMap<u32, Vec<u32>>,
    statuses: HashSet<String>,
    filtered_cache: Option<HashSet<u32>>,
    legacy_files: Vec<u32>,
    legacy_seen: HashSet<u32>,
}
#[wasm_bindgen]
impl WorkerSourceRights {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            records: vec![],
            components: HashMap::new(),
            members: vec![],
            owners: HashMap::new(),
            statuses: HashSet::new(),
            filtered_cache: None,
            legacy_files: vec![],
            legacy_seen: HashSet::new(),
        }
    }
    pub fn component(&mut self, id: u32, kind: &str) {
        self.components.insert(id, kind.into());
    }
    pub fn record(
        &mut self,
        row: u32,
        source: u32,
        nonempty: bool,
        scopes: &[u32],
        explicit: bool,
        assessment: &str,
        id: &[u16],
        status: &str,
        posture: &str,
        review: &str,
    ) {
        self.records.push(Right {
            row,
            source,
            nonempty,
            scopes: scopes.iter().copied().collect(),
            explicit,
            assessment: assessment.into(),
            root: rights_id(id),
            layer: layer_id(id),
            status: status.into(),
            posture: posture.into(),
            review: review.into(),
        });
    }
    pub fn membership(
        &mut self,
        item: u32,
        file: u32,
        kind: &str,
        predicate: &str,
        array: bool,
        raw_length: usize,
        sources: &[u32],
        contexts_present: bool,
        contexts_array: bool,
        contexts_length: usize,
    ) -> i32 {
        if kind != "authored_item_manifest"
            || predicate != "has_file"
            || self.components.get(&item).is_none_or(|k| k != "item")
            || self.components.get(&file).is_none_or(|k| k != "file")
        {
            return -1;
        }
        let source_set: HashSet<u32> = sources.iter().copied().collect();
        let mut valid = array
            && sources.len() == raw_length
            && source_set.len() == sources.len()
            && !sources.is_empty();
        if !contexts_present {
            valid = valid && sources.len() == 1;
        } else if !contexts_array || contexts_length == 0 {
            valid = false;
        }
        let index = self.members.len();
        self.members.push(Member {
            item,
            file,
            refs: HashSet::new(),
            legacy: !contexts_present,
            valid,
            sources: source_set,
            manifests: HashSet::new(),
        });
        index as i32
    }
    pub fn context(
        &mut self,
        index: usize,
        record: bool,
        manifest: u32,
        manifest_nonempty: bool,
        right: u32,
        right_nonempty: bool,
        legacy_empty: bool,
    ) {
        let m = &mut self.members[index];
        if !record {
            m.valid = false;
            return;
        }
        if !manifest_nonempty || !m.sources.contains(&manifest) || !m.manifests.insert(manifest) {
            m.valid = false;
        }
        if right_nonempty {
            m.refs.insert(right);
        } else if legacy_empty {
            m.legacy = true;
        } else {
            m.valid = false;
        }
    }
    pub fn finish_membership(
        &mut self,
        index: usize,
        contexts_present: bool,
        contexts_array: bool,
        contexts_length: usize,
    ) {
        let m = &mut self.members[index];
        if contexts_present && contexts_array && contexts_length > 0 && m.manifests != m.sources {
            m.valid = false;
        }
        if m.refs.len() > 1 || !m.refs.is_empty() && m.legacy {
            m.valid = false;
        }
    }
    pub fn incoming_owner(&mut self, file: u32, item: u32, kind: &str, predicate: &str) {
        if kind == "authored_item_manifest" && predicate == "has_file" {
            self.owners.entry(file).or_default().push(item);
        }
    }
    pub fn aggregate_rows(&self) -> Vec<u32> {
        aggregate_rows(&self.records)
            .into_iter()
            .map(|r| r.row)
            .collect()
    }
    pub fn filtered_rows(&mut self) -> Vec<u32> {
        let rows: Vec<u32> = self.filtered().into_iter().map(|r| r.row).collect();
        self.filtered_cache = Some(rows.iter().copied().collect());
        rows
    }
    pub fn intersects_component(&self, scopes: &[u32]) -> bool {
        scopes.iter().any(|id| self.components.contains_key(id))
    }
    pub fn observe_legacy_file(
        &mut self,
        file: u32,
        kind: &str,
        predicate: &str,
        array: bool,
        length: usize,
        valid_contexts: usize,
    ) {
        if kind == "authored_item_manifest"
            && predicate == "has_file"
            && !(array && length > 0 && length == valid_contexts)
            && self.legacy_seen.insert(file)
        {
            self.legacy_files.push(file);
        }
    }
    pub fn legacy_file_ids(&self) -> Vec<u32> {
        self.legacy_files.clone()
    }
    pub fn context_ref(record: bool, reference_nonempty: bool) -> bool {
        record && reference_nonempty
    }
    pub fn status(&mut self, value: &str) {
        self.statuses.insert(value.into());
    }
    pub fn summary(&self, decision: &[u32], links: usize) -> String {
        let scopes: HashSet<u32> = decision.iter().copied().collect();
        let filtered = self.filtered();
        let records: Vec<Right> = filtered
            .iter()
            .filter(|r| r.scopes.iter().any(|id| scopes.contains(id)))
            .map(|r| (*r).clone())
            .collect();
        let aggregates = aggregate_rows(&records);
        let positive = aggregates.iter().filter(|r| positive(r)).count();
        let reviewed = aggregates
            .iter()
            .filter(|r| {
                positive(r) && matches!(r.review.as_str(), "accepted" | "accepted_with_limits")
            })
            .count();
        let access = if self.statuses.contains("open_download") {
            "downloadable"
        } else if self.statuses.contains("open_view") {
            "viewable"
        } else if self.statuses.contains("metadata_only") {
            "metadata_only"
        } else if ["restricted", "login_required", "unavailable"]
            .iter()
            .any(|s| self.statuses.contains(*s))
        {
            "restricted_or_unavailable"
        } else {
            "unknown"
        };
        let posture = if reviewed > 0 {
            "reviewed_reuse_route"
        } else if positive > 0 {
            "candidate_requires_human_review"
        } else if !records.is_empty() {
            "not_cleared"
        } else {
            "unknown"
        };
        let mut gaps = vec![];
        if records.is_empty() {
            gaps.push("no associated public rights record");
        } else if aggregates.is_empty() {
            gaps.push("no unambiguous aggregate rights assessment");
        }
        if positive > 0 && reviewed == 0 {
            gaps.push("positive rights route exists but has no accepted human review");
        }
        if links == 0 {
            gaps.push("no first-class associated Link record");
        }
        format!(
            "{{\"technical_access\":\"{access}\",\"rights_posture\":\"{posture}\",\"human_review_required\":{},\"can_conclude_legal_openness\":{},\"availability_is_license\":false,\"rights_scope_refs\":null,\"gaps\":[{}]}}",
            reviewed == 0,
            reviewed > 0,
            gaps.iter()
                .map(|s| format!("\"{s}\""))
                .collect::<Vec<_>>()
                .join(",")
        )
    }
}
fn positive(r: &Right) -> bool {
    matches!(r.status.as_str(), "licensed" | "public_domain_reviewed")
        && matches!(
            r.posture.as_str(),
            "authorized" | "authorized_with_conditions"
        )
}
fn aggregate_rows(records: &[Right]) -> Vec<&Right> {
    let mut groups: Vec<Vec<&Right>> = vec![];
    let mut positions = HashMap::new();
    for r in records {
        if !r.nonempty {
            continue;
        }
        let index = *positions.entry(r.source).or_insert_with(|| {
            groups.push(vec![]);
            groups.len() - 1
        });
        groups[index].push(r);
    }
    let mut result = vec![];
    for group in groups {
        if group.iter().any(|r| r.explicit) {
            if group
                .iter()
                .any(|r| !matches!(r.assessment.as_str(), "aggregate" | "layer"))
            {
                continue;
            }
            let roots: Vec<_> = group
                .iter()
                .filter(|r| r.assessment == "aggregate")
                .copied()
                .collect();
            if roots.len() == 1 {
                result.push(roots[0]);
            }
        } else {
            let roots: Vec<_> = group
                .iter()
                .filter(|r| r.root && !r.layer)
                .copied()
                .collect();
            let layers = group.iter().filter(|r| r.layer).count();
            if roots.len() == 1 && roots.len() + layers == group.len() {
                result.push(roots[0]);
            }
        }
    }
    result
}
impl WorkerSourceRights {
    fn filtered(&self) -> Vec<&Right> {
        if let Some(cache) = &self.filtered_cache {
            return self
                .records
                .iter()
                .filter(|r| cache.contains(&r.row))
                .collect();
        }

        let files: HashSet<u32> = self
            .components
            .iter()
            .filter(|(_, kind)| *kind == "file")
            .map(|(id, _)| *id)
            .collect();
        if files.is_empty() {
            return self.records.iter().collect();
        }
        let bib_ids: HashSet<u32> = self
            .components
            .iter()
            .filter(|(_, kind)| matches!(kind.as_str(), "work" | "expression" | "edition"))
            .map(|(id, _)| *id)
            .collect();
        let mut member_counts: HashMap<(u32, u32), usize> = HashMap::new();
        for member in &self.members {
            *member_counts.entry((member.file, member.item)).or_default() += 1;
        }
        self.records
            .iter()
            .filter(|record| {
                let file_scopes: Vec<_> = files
                    .iter()
                    .filter(|id| record.scopes.contains(id))
                    .copied()
                    .collect();
                if file_scopes.is_empty() || bib_ids.iter().any(|id| record.scopes.contains(id)) {
                    return true;
                }
                file_scopes.iter().all(|file| {
                    self.members.iter().filter(|m| m.file == *file).any(|m| {
                        if !m.valid
                            || member_counts.get(&(m.file, m.item)).copied().unwrap_or(0) > 1
                        {
                            return false;
                        }
                        if !m.refs.is_empty() {
                            return m.refs.contains(&record.source);
                        }
                        if !m.legacy || !record.scopes.contains(&m.item) {
                            return false;
                        }
                        let Some(owners) = self.owners.get(file) else {
                            return false;
                        };
                        if owners.len() != 1 || owners[0] != m.item {
                            return false;
                        }
                        let matching: Vec<_> = self
                            .records
                            .iter()
                            .filter(|r| r.scopes.contains(&m.item) && r.scopes.contains(file))
                            .collect();
                        let sources: HashSet<u32> = matching
                            .iter()
                            .filter(|r| r.nonempty)
                            .map(|r| r.source)
                            .collect();
                        sources.len() == 1
                            && matching.iter().all(|r| r.nonempty)
                            && sources.contains(&record.source)
                    })
                })
            })
            .collect()
    }
}
