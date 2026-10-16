//! Exact one-hop predecessor/successor closure for descriptive Agent publication.
//! Source material remains ordered; the existing candidate normalizer owns all
//! normalization, and Graph owns canonical sparse insertion allocation.
use super::{
    source_claim_publication_assembly::OrderedRow,
    source_claim_publication_bytes as bytes,
    source_claim_publication_context::{self as context, Context},
    source_claim_publication_dependencies as deps,
    source_claim_publication_graph::Graph,
    source_claim_publication_normalize as norm,
    source_claim_publication_roots::{Change, Roots},
};
use norm::{
    CandidateNode, CandidateRelation, ClaimCandidateInput, ClaimCandidateLimits,
    ClaimCandidateRegistries, RetainedCandidateNode,
};
use rusqlite::Transaction;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use tos_compiler::{Error, Result, knowledge_stage::SeekRow, local_prepared::PreparedChange};
use tos_foundation::{JsonLimits, JsonMode, emit_value_preserved_json, parse_json};

pub(super) struct Cohort {
    pub nodes: BTreeMap<(String, String), OrderedRow>,
    pub edges: BTreeMap<(String, String), OrderedRow>,
    pub traces: BTreeMap<String, OrderedRow>,
}
pub(super) struct Closure {
    pub changes: Vec<PreparedChange>,
    pub raw_changes: BTreeMap<String, Vec<Change>>,
    pub normalized_nodes: usize,
    pub normalized_relations: usize,
    pub affected_nodes: usize,
    pub changed_nodes: usize,
    pub changed_relations: usize,
}
fn role(graph: &str) -> Result<&'static str> {
    match graph {
        "source-claims" => Ok("bibliographic-claims"),
        "source-navigation" => Ok("source-navigation"),
        _ => Err(Error::Invalid("Agent raw graph profile")),
    }
}
fn seek(graph: &str, id: &str, raw: Vec<u8>) -> SeekRow {
    SeekRow {
        id: id.to_owned(),
        source_graph: graph.to_owned(),
        source_order: None,
        payload_sha256: bytes::digest(&raw),
        payload: raw,
    }
}
fn material(raw: &[u8], maximum: usize) -> Result<Vec<u8>> {
    let limits = JsonLimits::new(maximum, 128, 1_000_000, 4096)
        .map_err(|_| Error::Budget("Agent retained material limits"))?;
    let doc = parse_json(raw, JsonMode::PublishedStrict, limits)
        .map_err(|e| Error::Source(e.to_string()))?;
    let payload = doc
        .root()
        .object_get("source_record")
        .and_then(|v| v.object_get("payload"))
        .ok_or(Error::Invalid("Agent retained owner payload"))?;
    emit_value_preserved_json(payload, limits).map_err(|e| Error::Source(e.to_string()))
}
fn check_row(row: &OrderedRow, maximum: usize) -> Result<()> {
    if bytes::parse(&row.raw, maximum)? != row.value {
        return Err(Error::Invalid(
            "Agent ordered material differs from cohort value",
        ));
    }
    Ok(())
}
fn by_id(rows: &[Value]) -> Result<BTreeMap<String, Value>> {
    let mut out = BTreeMap::new();
    for row in rows {
        if out
            .insert(bytes::text(row, "id")?.to_owned(), row.clone())
            .is_some()
        {
            return Err(Error::Invalid("Agent normalized duplicate identity"));
        }
    }
    Ok(out)
}
type Specs = BTreeMap<String, (String, Value, Vec<u8>, Option<String>)>;
fn candidate_nodes(
    rows: &BTreeMap<(String, String), OrderedRow>,
    selected: &BTreeSet<String>,
    materials: &BTreeMap<(String, String), Vec<u8>>,
    dossiers: &[String],
) -> Vec<CandidateNode> {
    rows.iter()
        .filter(|((g, id), _)| selected.contains(&format!("{g}:{id}")))
        .map(|((g, id), row)| CandidateNode {
            raw: seek(g, id, materials[&(g.clone(), id.clone())].clone()),
            dossier_ref: norm::declared_dossier(&row.value, g).filter(|d| dossiers.contains(d)),
        })
        .collect()
}
fn candidate_relations(rows: &Specs) -> Vec<CandidateRelation> {
    rows.values()
        .map(|(g, row, raw, identity)| CandidateRelation {
            raw: seek(g, row["edge_id"].as_str().unwrap_or(""), raw.clone()),
            identity_id: identity.clone(),
        })
        .collect()
}
fn retained(rows: &BTreeMap<String, (Value, Vec<u8>)>) -> Vec<RetainedCandidateNode> {
    rows.values()
        .map(|(v, raw)| RetainedCandidateNode {
            normalized: v.clone(),
            owner_material: raw.clone(),
        })
        .collect()
}
fn traces(rows: &BTreeMap<String, Vec<u8>>) -> Vec<SeekRow> {
    rows.iter()
        .map(|(id, raw)| seek("source-claims", id, raw.clone()))
        .collect()
}
#[allow(clippy::too_many_arguments)]
pub(super) fn normalize(
    tx: &Transaction<'_>,
    roots: &mut Roots,
    old: &Cohort,
    new: &Cohort,
    context_state: &Value,
    limits: ClaimCandidateLimits,
    dependency_limits: deps::Limits,
    registries: &ClaimCandidateRegistries<'_>,
) -> Result<Closure> {
    if old.nodes.keys().any(|k| !new.nodes.contains_key(k))
        || old.edges.keys().any(|k| !new.edges.contains_key(k))
        || old.traces.keys().ne(new.traces.keys())
    {
        return Err(Error::Invalid(
            "Agent descriptive publication cannot delete topology or alter traces",
        ));
    }
    for cohort in [old, new] {
        let mut input = 0usize;
        if cohort.nodes.len() > limits.max_nodes
            || cohort.edges.len() > limits.max_relations
            || cohort.traces.len() > limits.max_traces
        {
            return Err(Error::Budget("Agent cohort limits"));
        }
        for row in cohort
            .nodes
            .values()
            .chain(cohort.edges.values())
            .chain(cohort.traces.values())
        {
            input = input
                .checked_add(row.raw.len())
                .filter(|n| *n <= limits.max_input_bytes)
                .ok_or(Error::Budget("Agent ordered cohort bytes"))?;
            check_row(row, limits.max_row_bytes)?;
        }
    }
    for (id, row) in &old.traces {
        if bytes::row_digest(&row.value, limits.max_row_bytes)?
            != bytes::row_digest(&new.traces[id].value, limits.max_row_bytes)?
        {
            return Err(Error::Invalid(
                "Agent descriptive publication changed Claim trace",
            ));
        }
    }
    let mut changed = BTreeSet::new();
    let mut before_material = BTreeMap::new();
    let mut after_material = BTreeMap::new();
    let mut raw_changes: BTreeMap<String, Vec<Change>> = BTreeMap::new();
    for (collection, before, after) in [
        ("nodes", &old.nodes, &new.nodes),
        ("edges", &old.edges, &new.edges),
    ] {
        for ((g, id), row) in after {
            let selected = roots.get_with_material(role(g)?, collection, id)?;
            let prior = before.get(&(g.clone(), id.clone()));
            if selected
                .as_ref()
                .map(|(v, _)| bytes::row_digest(v, limits.max_row_bytes))
                .transpose()?
                != prior
                    .map(|v| bytes::row_digest(&v.value, limits.max_row_bytes))
                    .transpose()?
            {
                return Err(Error::Invalid(
                    "Agent cohort differs from selected raw predecessor",
                ));
            }
            if collection == "nodes" {
                if let Some((_, raw)) = &selected {
                    before_material.insert((g.clone(), id.clone()), raw.clone());
                }
            }
            let before_sha = prior
                .map(|v| bytes::row_digest(&v.value, limits.max_row_bytes))
                .transpose()?;
            if before_sha.as_ref() == Some(&bytes::row_digest(&row.value, limits.max_row_bytes)?) {
                if collection == "nodes" {
                    after_material.insert(
                        (g.clone(), id.clone()),
                        selected
                            .as_ref()
                            .ok_or(Error::Invalid("Agent unchanged source absent"))?
                            .1
                            .clone(),
                    );
                }
                continue;
            }
            if collection == "nodes" {
                after_material.insert((g.clone(), id.clone()), row.raw.clone());
            }
            if collection == "nodes" {
                changed.insert(format!("{g}:{id}"));
            }
            raw_changes
                .entry(role(g)?.to_owned())
                .or_default()
                .push(Change {
                    collection: collection.to_owned(),
                    key: id.clone(),
                    before_sha256: before_sha,
                    after: row.value.clone(),
                });
        }
    }
    let mut trace_rows = BTreeMap::new();
    for (id, row) in &old.traces {
        let (selected, raw) = roots
            .get_with_material("bibliographic-claims", "claim_traces", id)?
            .ok_or(Error::Invalid("Agent retained trace absent"))?;
        if bytes::row_digest(&selected, limits.max_row_bytes)?
            != bytes::row_digest(&row.value, limits.max_row_bytes)?
        {
            return Err(Error::Invalid(
                "Agent trace differs from selected predecessor",
            ));
        }
        trace_rows.insert(id.clone(), raw);
        let key = (
            "source-claims".to_owned(),
            bytes::text(&row.value, "object_node_id")?.to_owned(),
        );
        if changed.contains(&format!(
            "source-claims:{}",
            bytes::text(&row.value, "claim_node_id")?
        )) {
            if let Some(literal) = old
                .nodes
                .get(&key)
                .filter(|v| v.value["node_kind"] == "literal")
            {
                if literal.value["properties"]["claim_ref"] != *id {
                    return Err(Error::Invalid(
                        "Agent literal context ownership is not Claim scoped",
                    ));
                }
                changed.insert(format!("{}:{}", key.0, key.1));
            }
        }
    }
    if changed.len() > limits.max_nodes {
        return Err(Error::Budget("Agent changed nodes"));
    }
    let mut graph = Graph::new(tx, dependency_limits);
    let mut context = Context::new(tx, dependency_limits);
    let mut prior_relations = BTreeMap::new();
    let mut prior_specs = Specs::new();
    for id in graph.incidence(&changed, limits.max_relations)? {
        let (row, raw) = graph.body("relation", &id)?;
        let g = bytes::text(&row, "source_graph")?.to_owned();
        let native = bytes::text(&row, "native_id")?;
        let identity = id
            .strip_prefix(&(g.clone() + ":"))
            .filter(|s| *s != native)
            .map(str::to_owned);
        let raw = material(&raw, limits.max_row_bytes)?;
        prior_specs.insert(
            id.clone(),
            (g, bytes::parse(&raw, limits.max_row_bytes)?, raw, identity),
        );
        prior_relations.insert(id, row);
    }
    let mut specs = prior_specs.clone();
    for ((g, id), row) in &new.edges {
        let key = (g.clone(), id.clone());
        if old
            .edges
            .get(&key)
            .map(|v| bytes::row_digest(&v.value, limits.max_row_bytes))
            .transpose()?
            == Some(bytes::row_digest(&row.value, limits.max_row_bytes)?)
        {
            continue;
        }
        let identity = format!("{g}:{id}");
        if let Some(prior) = specs.get_mut(&identity) {
            prior.1 = row.value.clone();
            prior.2 = row.raw.clone();
        } else {
            if old.edges.contains_key(&key) {
                return Err(Error::Invalid(
                    "Agent changed edge outside complete incidence",
                ));
            }
            specs.insert(
                identity,
                (g.clone(), row.value.clone(), row.raw.clone(), None),
            );
        }
    }
    if specs.len() > limits.max_relations {
        return Err(Error::Budget("Agent successor incidence"));
    }
    let mut support = BTreeSet::new();
    let mut references = BTreeSet::new();
    for (g, row, _, _) in specs.values() {
        for end in ["from", "to"] {
            let source = row
                .get(format!("{end}_source_graph"))
                .and_then(Value::as_str)
                .unwrap_or(g);
            support.insert(format!(
                "{source}:{}",
                bytes::text(row, &format!("{end}_id"))?
            ));
        }
        if let Some(id) = row["claim_ref"].as_str() {
            references.insert((g.clone(), id.to_owned()));
        }
    }
    for (g, id) in &references {
        if g == "source-claims" && !trace_rows.contains_key(id) {
            let (_, raw) = roots
                .get_with_material("bibliographic-claims", "claim_traces", id)?
                .ok_or(Error::Invalid("Agent governing trace absent"))?;
            trace_rows.insert(id.clone(), raw);
        }
        support.extend(context.members(g, id, limits.max_nodes)?);
    }
    if trace_rows.len() > limits.max_traces {
        return Err(Error::Budget("Agent governing traces"));
    }
    for raw in trace_rows.values() {
        let row = bytes::parse(raw, limits.max_row_bytes)?;
        for field in ["claim_node_id", "subject_node_id", "object_node_id"] {
            support.insert(format!("source-claims:{}", bytes::text(&row, field)?));
        }
    }
    support.retain(|id| !changed.contains(id));
    if support.len() + changed.len() > limits.max_nodes {
        return Err(Error::Budget("Agent endpoint/context closure"));
    }
    let mut retained_nodes = BTreeMap::new();
    for id in support {
        let (row, raw) = graph.body("node", &id)?;
        retained_nodes.insert(id, (row, material(&raw, limits.max_row_bytes)?));
    }
    let mut previous_nodes = BTreeMap::new();
    for (g, id) in old
        .nodes
        .keys()
        .filter(|(g, id)| changed.contains(&format!("{g}:{id}")))
    {
        let key = format!("{g}:{id}");
        previous_nodes.insert(key.clone(), graph.body("node", &key)?.0);
    }
    let mut positions = BTreeMap::new();
    for (id, row) in retained_nodes
        .iter()
        .map(|(id, (row, _))| (id, row))
        .chain(previous_nodes.iter())
    {
        if ["claim", "annotation-claim"].contains(&row["kind_id"].as_str().unwrap_or("")) {
            if positions
                .insert(context.position(row, limits.max_nodes)?, id.clone())
                .is_some()
            {
                return Err(Error::Invalid("Agent repeated context position"));
            }
        }
    }
    let order: Vec<_> = positions.into_values().collect();
    let dossiers = context_state["dossier_refs"]
        .as_array()
        .ok_or(Error::Invalid("Agent dossier refs"))?
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .ok_or(Error::Invalid("Agent dossier ref"))
        })
        .collect::<Result<Vec<_>>>()?;
    let before = norm::normalize_claim_candidate(
        &ClaimCandidateInput {
            nodes: candidate_nodes(&old.nodes, &changed, &before_material, &dossiers),
            relations: candidate_relations(&prior_specs),
            retained_nodes: retained(&retained_nodes),
            traces: traces(&trace_rows),
            dossier_refs: dossiers.clone(),
            context_node_order: order.clone(),
            normalization_binding: registries.expected_normalization_binding.clone(),
        },
        *registries,
        limits,
    )?;
    if by_id(&before.nodes)? != previous_nodes || by_id(&before.relations)? != prior_relations {
        return Err(Error::Invalid(
            "Agent closure does not reproduce exact prepared predecessor",
        ));
    }
    let after = norm::normalize_claim_candidate(
        &ClaimCandidateInput {
            nodes: candidate_nodes(&new.nodes, &changed, &after_material, &dossiers),
            relations: candidate_relations(&specs),
            retained_nodes: retained(&retained_nodes),
            traces: traces(&trace_rows),
            dossier_refs: dossiers,
            context_node_order: order,
            normalization_binding: registries.expected_normalization_binding.clone(),
        },
        *registries,
        limits,
    )?;
    for row in &after.nodes {
        let prior = previous_nodes.get(bytes::text(row, "id")?);
        if context::keys(row)? != prior.map(context::keys).transpose()?.unwrap_or_default() {
            return Err(Error::Invalid(
                "Agent descriptive correction changed context membership",
            ));
        }
    }
    for row in &after.relations {
        if let Some(prior) = prior_relations.get(bytes::text(row, "id")?) {
            if ["from_id", "to_id", "view_ids", "relation_type_id"]
                .iter()
                .any(|key| row.get(*key) != prior.get(*key))
            {
                return Err(Error::Invalid(
                    "Agent descriptive correction changed existing topology/views",
                ));
            }
        }
    }
    let changes = graph.changes(
        &previous_nodes,
        &prior_relations,
        &after.nodes,
        &after.relations,
    )?;
    Ok(Closure {
        affected_nodes: changed.len(),
        changed_nodes: changes.iter().filter(|v| v.kind == "node").count(),
        changed_relations: changes.iter().filter(|v| v.kind == "relation").count(),
        changes,
        raw_changes,
        normalized_nodes: after.nodes.len(),
        normalized_relations: after.relations.len(),
    })
}
