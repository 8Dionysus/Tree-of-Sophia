//! Packet-local presentation state. Host retains source objects and observes
//! fresh JS reads/coercions; this state owns string identity, grouping and folds.
use std::collections::{BTreeMap, BTreeSet};
use wasm_bindgen::prelude::*;
const ABSENT: u32 = u32::MAX;
type Key = Vec<u16>;
fn points(value: &[u16]) -> impl Iterator<Item = u32> + '_ {
    std::char::decode_utf16(value.iter().copied()).map(|c| match c {
        Ok(c) => c as u32,
        Err(c) => c.unpaired_surrogate() as u32,
    })
}
fn compare(a: &[u16], b: &[u16]) -> i32 {
    let mut a = points(a);
    let mut b = points(b);
    loop {
        match (a.next(), b.next()) {
            (Some(a), Some(b)) if a != b => return a as i32 - b as i32,
            (Some(_), Some(_)) => {}
            (Some(_), None) => return 1 + a.count() as i32,
            (None, Some(_)) => return -1 - b.count() as i32,
            (None, None) => return 0,
        }
    }
}
#[derive(Default)]
struct Group {
    key: u32,
    entity: u32,
    nodes: Vec<u32>,
    ids: Vec<u32>,
    members: BTreeSet<u32>,
}
#[derive(Clone)]
struct Arc {
    relation: u32,
    from: u32,
    to: u32,
}
#[derive(Clone)]
struct Candidate {
    legs: Vec<u32>,
}
#[wasm_bindgen]
pub struct KnowledgeSceneSession {
    key_index: BTreeMap<Key, u32>,
    groups: Vec<Group>,
    group_index: BTreeMap<u32, u32>,
    by_node: BTreeMap<u32, u32>,
    by_relation: BTreeMap<u32, u32>,
    outgoing: BTreeMap<u32, Vec<u32>>,
    arcs: Vec<Arc>,
    collapsed: Vec<u32>,
    incident: BTreeMap<u32, Vec<u32>>,
    claims: BTreeMap<u32, u32>,
    candidates: BTreeMap<u32, Candidate>,
    reasons: BTreeMap<u32, String>,
    folded: BTreeSet<u32>,
    removed: BTreeSet<u32>,
    detail_vertices: BTreeSet<u32>,
    path_endpoints: BTreeSet<u32>,
    focus_vertex: u32,
    vertex_legs: BTreeMap<u32, BTreeSet<u32>>,
}
#[wasm_bindgen]
impl KnowledgeSceneSession {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            key_index: BTreeMap::new(),
            groups: vec![],
            group_index: BTreeMap::new(),
            by_node: BTreeMap::new(),
            by_relation: BTreeMap::new(),
            outgoing: BTreeMap::new(),
            arcs: vec![],
            collapsed: vec![],
            incident: BTreeMap::new(),
            claims: BTreeMap::new(),
            candidates: BTreeMap::new(),
            reasons: BTreeMap::new(),
            folded: BTreeSet::new(),
            removed: BTreeSet::new(),
            detail_vertices: BTreeSet::new(),
            path_endpoints: BTreeSet::new(),
            focus_vertex: ABSENT,
            vertex_legs: BTreeMap::new(),
        }
    }
    pub fn compare_ids(a: &[u16], b: &[u16]) -> i32 {
        compare(a, b)
    }
    pub fn priority_keys() -> String {
        "source-navigation\ncanon\nsource-claims\nphilosophy\ncandidate-intake\nrepository\nsemantic-interchange".into()
    }
    pub fn priority_delta(delta: f64) -> bool {
        delta != 0.0 && !delta.is_nan()
    }
    pub fn intern(&mut self, units: &[u16]) -> u32 {
        if let Some(id) = self.key_index.get(units) {
            return *id;
        }
        let id = self.key_index.len() as u32;
        self.key_index.insert(units.to_vec(), id);
        id
    }
    pub fn node_key(&mut self, handle: u32, id: u32, key: u32, entity: u32) {
        let group = if let Some(group) = self.group_index.get(&key) {
            *group
        } else {
            let group = self.groups.len() as u32;
            self.groups.push(Group {
                key,
                entity,
                nodes: vec![],
                ids: vec![],
                members: BTreeSet::new(),
            });
            self.group_index.insert(key, group);
            group
        };
        self.by_node.insert(id, group);
        self.groups[group as usize].nodes.push(handle);
    }
    pub fn vertex_prefix(entity: bool) -> String {
        if entity {
            "tos-scene:entity:"
        } else {
            "tos-scene:carrier:"
        }
        .into()
    }
    pub fn entity_prefix() -> String {
        "tos.".into()
    }
    pub fn group_count(&self) -> u32 {
        self.groups.len() as u32
    }
    pub fn group_key(&self, group: u32) -> u32 {
        self.groups[group as usize].key
    }
    pub fn group_entity(&self, group: u32) -> u32 {
        self.groups[group as usize].entity
    }
    pub fn group_nodes(&self, group: u32) -> Vec<u32> {
        self.groups[group as usize].nodes.clone()
    }
    pub fn group_ids(&mut self, group: u32, ids: &[u32]) {
        self.groups[group as usize].ids = ids.to_vec();
        self.groups[group as usize].members = ids.iter().copied().collect();
    }
    pub fn node_ids(&self) -> Vec<u32> {
        self.by_node.keys().copied().collect()
    }
    pub fn vertex_for(&self, id: u32) -> u32 {
        self.by_node.get(&id).copied().unwrap_or(ABSENT)
    }
    pub fn endpoints(&self, from: u32, to: u32) -> Result<(), JsValue> {
        if from == ABSENT || to == ABSENT {
            Err(JsValue::from_str(
                "scene relation endpoint missing from returned packet",
            ))
        } else {
            Ok(())
        }
    }
    pub fn same_vertex(from: u32, to: u32) -> bool {
        from == to
    }
    pub fn arc(&mut self, id: u32, from: u32, to: u32, projects: bool, selected: bool) {
        if from == to && projects && !selected {
            self.collapsed.push(id);
        } else {
            self.arcs.push(Arc {
                relation: id,
                from,
                to,
            });
        }
    }
    pub fn arc_count(&self) -> u32 {
        self.arcs.len() as u32
    }
    pub fn arc_relation(&self, i: u32) -> u32 {
        self.arcs[i as usize].relation
    }
    pub fn arc_from(&self, i: u32) -> u32 {
        self.arcs[i as usize].from
    }
    pub fn arc_to(&self, i: u32) -> u32 {
        self.arcs[i as usize].to
    }
    pub fn collapsed(&self) -> Vec<u32> {
        self.collapsed.clone()
    }
    pub fn relation_record(&mut self, id: u32, handle: u32) {
        self.by_relation.insert(id, handle);
    }
    pub fn relation_handle(&self, id: u32) -> u32 {
        self.by_relation.get(&id).copied().unwrap_or(ABSENT)
    }
    pub fn outgoing_record(&mut self, from: u32, handle: u32) {
        self.outgoing.entry(from).or_default().push(handle);
    }
    pub fn outgoing(&self, id: u32) -> Vec<u32> {
        self.outgoing.get(&id).cloned().unwrap_or_default()
    }
    pub fn index_incident(&mut self) {
        for (i, a) in self.arcs.iter().enumerate() {
            self.incident.entry(a.from).or_default().push(i as u32);
            if a.from != a.to {
                self.incident.entry(a.to).or_default().push(i as u32);
            }
        }
    }
    pub fn claim(&mut self, id: u32, node: u32) {
        self.claims.insert(id, node);
    }
    pub fn claim_ids(&self) -> Vec<u32> {
        self.claims.keys().copied().collect()
    }
    pub fn claim_handle(&self, id: u32) -> u32 {
        self.claims[&id]
    }
    pub fn claim_contract(
        &mut self,
        id: u32,
        subject_string: bool,
        object_string: bool,
        subject: u32,
        object: u32,
    ) -> bool {
        if !subject_string
            || !object_string
            || self.vertex_for(subject) == ABSENT
            || self.vertex_for(object) == ABSENT
        {
            self.reasons.insert(id, "incomplete-claim-contract".into());
            false
        } else {
            true
        }
    }
    pub fn claim_mapping(&mut self, id: u32, mapped: bool, predicate: bool) -> bool {
        if !mapped || !predicate {
            self.reasons.insert(id, "unmapped-claim-predicate".into());
            false
        } else {
            true
        }
    }
    pub fn claim_identity(&mut self, id: u32, subject: u32, object: u32) -> bool {
        let vertex = self.vertex_for(id);
        if vertex == self.vertex_for(subject) || vertex == self.vertex_for(object) {
            self.reasons
                .insert(id, "claim-endpoint-identity-collision".into());
            false
        } else {
            true
        }
    }
    pub fn claim_legs(
        &mut self,
        id: u32,
        subject_count: usize,
        object_count: usize,
        subject_matches: bool,
        object_matches: bool,
    ) -> bool {
        if subject_count != 1 || object_count != 1 || !subject_matches || !object_matches {
            self.reasons
                .insert(id, "incomplete-or-ambiguous-path".into());
            false
        } else {
            true
        }
    }
    pub fn member_shape(&mut self, id: u32, array: bool, length_truthy: bool) -> bool {
        if !array || !length_truthy {
            self.reasons
                .insert(id, "incomplete-value-member-context".into());
            false
        } else {
            true
        }
    }
    pub fn member_strings(&mut self, id: u32, nonstring: bool) -> bool {
        if nonstring {
            self.reasons
                .insert(id, "incomplete-value-member-context".into());
            false
        } else {
            true
        }
    }
    pub fn member_unique(&mut self, id: u32, strict_length_match: bool) -> bool {
        if !strict_length_match {
            self.reasons
                .insert(id, "incomplete-value-member-context".into());
            false
        } else {
            true
        }
    }
    pub fn member_presence(&mut self, id: u32, missing: bool) -> bool {
        if missing {
            self.reasons
                .insert(id, "incomplete-value-member-context".into());
            false
        } else {
            true
        }
    }
    pub fn member_edges(&mut self, id: u32, strict_length_match: bool) -> bool {
        if !strict_length_match {
            self.reasons
                .insert(id, "incomplete-value-member-context".into());
            false
        } else {
            true
        }
    }
    pub fn member_targets(&mut self, id: u32, strict_length_match: bool) -> bool {
        if !strict_length_match {
            self.reasons
                .insert(id, "incomplete-value-member-context".into());
            false
        } else {
            true
        }
    }
    pub fn candidate(&mut self, id: u32, legs: &[u32]) {
        self.candidates.insert(
            id,
            Candidate {
                legs: legs.to_vec(),
            },
        );
    }
    pub fn focus(&mut self, present: bool, id: u32) {
        self.focus_vertex = if present { self.vertex_for(id) } else { ABSENT };
    }
    pub fn incident(&self, group: u32) -> Vec<u32> {
        self.incident.get(&group).cloned().unwrap_or_default()
    }
    pub fn group_contains(&self, group: u32, id: u32) -> bool {
        self.groups[group as usize].members.contains(&id)
    }
    pub fn local_claims(&self, group: u32) -> Vec<u32> {
        self.groups[group as usize]
            .ids
            .iter()
            .copied()
            .filter(|id| self.claims.contains_key(id))
            .collect()
    }
    pub fn vertex_reason(&self, group: u32, selected_relation: bool) -> String {
        if selected_relation {
            "focus-relation".into()
        } else if group == self.focus_vertex {
            "focus-claim".into()
        } else if self.groups[group as usize]
            .ids
            .iter()
            .any(|id| !self.candidates.contains_key(id))
        {
            "mixed-or-incomplete-claim-carriers".into()
        } else {
            String::new()
        }
    }
    pub fn index_vertex_legs(&mut self, group: u32) {
        let legs = self.groups[group as usize]
            .ids
            .iter()
            .filter_map(|id| self.candidates.get(id))
            .flat_map(|c| c.legs.iter().copied())
            .collect();
        self.vertex_legs.insert(group, legs);
    }
    pub fn is_leg(&self, group: u32, relation: u32) -> bool {
        self.vertex_legs[&group].contains(&relation)
    }
    pub fn incident_reason(&self, group: u32, from: u32, detail: bool, to: u32) -> String {
        if !self.groups[group as usize].members.contains(&from) || !detail || to == group {
            "nonfoldable-incident-relation".into()
        } else if to == self.focus_vertex {
            "focus-detail".into()
        } else {
            String::new()
        }
    }
    pub fn accept_vertex(&mut self, group: u32, reason: &str) -> bool {
        let local = self.local_claims(group);
        if !reason.is_empty()
            && (reason != "focus-claim" || local.iter().any(|id| !self.candidates.contains_key(id)))
        {
            for id in local {
                self.reasons.entry(id).or_insert_with(|| reason.into());
            }
            false
        } else {
            if reason.is_empty() {
                self.folded.insert(group);
            }
            true
        }
    }
    pub fn candidate_ids(&self, group: u32) -> Vec<u32> {
        self.groups[group as usize]
            .ids
            .iter()
            .copied()
            .filter(|id| self.candidates.contains_key(id))
            .collect()
    }
    pub fn claim_entity() -> String {
        "tos.entity.claim".into()
    }
    pub fn leg_types() -> String {
        "tos.relation.has-subject\ntos.relation.has-object".into()
    }
    pub fn member_type() -> String {
        "tos.relation.claim-value-member".into()
    }
    pub fn collapse_type() -> String {
        "tos.relation.projects".into()
    }
    pub fn wording_roles() -> String {
        "caption\nstatement\nhover".into()
    }
    pub fn wording_fields() -> String {
        "summary\ntitle".into()
    }
    pub fn shared_schema() -> String {
        "tos_human_form_selection_v2".into()
    }
    pub fn wording_pointer(role: &str, shared: bool) -> String {
        format!(
            "/human_form_selection/roles/{role}{}",
            if shared { "" } else { "/packet" }
        )
    }
    pub fn wording_field_pointer(field: &str) -> String {
        format!("/display_selection/fields/{field}")
    }
    pub fn wording_mode(shared: bool) -> String {
        if shared {
            "claim-with-shared-form-context-v2"
        } else {
            "claim-with-mandatory-context"
        }
        .into()
    }
    pub fn detail_type(units: &[u16]) -> bool {
        units
            .iter()
            .copied()
            .eq("tos.relation.claim-supported-by".encode_utf16())
            || units
                .iter()
                .copied()
                .eq("tos.relation.claim-value-member".encode_utf16())
    }
    pub fn consume(&mut self, id: u32, details: &[u32]) {
        self.removed
            .extend(self.candidates[&id].legs.iter().copied());
        self.removed.extend(details.iter().copied());
    }
    pub fn detail_vertex(&mut self, node: u32) {
        self.detail_vertices.insert(self.vertex_for(node));
    }
    pub fn path_endpoint(&mut self, vertex: u32) {
        self.path_endpoints.insert(vertex);
    }
    pub fn finish_fold(&mut self) {
        let mut endpoints = self.path_endpoints.clone();
        for a in &self.arcs {
            if !self.removed.contains(&a.relation) {
                endpoints.insert(a.from);
                endpoints.insert(a.to);
            }
        }
        let claims: BTreeSet<_> = self.claims.keys().map(|id| self.vertex_for(*id)).collect();
        for vertex in &self.detail_vertices {
            if !endpoints.contains(vertex)
                && *vertex != self.focus_vertex
                && !claims.contains(vertex)
            {
                self.folded.insert(*vertex);
            }
        }
    }
    pub fn retained(&self, i: u32) -> bool {
        !self.removed.contains(&self.arcs[i as usize].relation)
    }
    pub fn folded(&self, group: u32) -> bool {
        self.folded.contains(&group)
    }
    pub fn folded_groups(&self) -> Vec<u32> {
        self.folded.iter().copied().collect()
    }
    pub fn reason_ids(&self) -> Vec<u32> {
        self.reasons.keys().copied().collect()
    }
    pub fn reason(&self, id: u32) -> String {
        self.reasons[&id].clone()
    }
}
