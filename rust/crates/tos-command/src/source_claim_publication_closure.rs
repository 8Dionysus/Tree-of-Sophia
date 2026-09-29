//! Complete one-hop incidence and context closure for the maintained initial Claim caller.
use super::{
    source_claim_publication_assembly::AssembledAddition,
    source_claim_publication_bytes as bytes,
    source_claim_publication_context::{self, Context},
    source_claim_publication_dependencies as deps,
    source_claim_publication_graph::Graph,
    source_claim_publication_normalize as normalization,
    source_claim_publication_roots::{Change, Roots},
};
use normalization::{
    CandidateNode, CandidateRelation, ClaimCandidateInput, ClaimCandidateLimits,
    ClaimCandidateRegistries, RetainedCandidateNode,
};
use rusqlite::Transaction;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use tos_compiler::{Error, Result, knowledge_stage::SeekRow, local_prepared::PreparedChange};
use tos_foundation::{JsonLimits, JsonMode, emit_value_preserved_json, parse_json};
pub(super) struct Closure {
    pub changes: Vec<PreparedChange>,
    pub raw_changes: Vec<Change>,
    pub context_ids: Vec<String>,
    pub nodes: BTreeMap<String, Value>,
    pub normalized_nodes: usize,
    pub normalized_relations: usize,
}
fn role(graph: &str) -> Result<&'static str> {
    match graph {
        "source-claims" => Ok("bibliographic-claims"),
        "source-navigation" => Ok("source-navigation"),
        _ => Err(Error::Invalid("Claim raw graph profile")),
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
fn owner_material(raw: &[u8], maximum: usize) -> Result<Vec<u8>> {
    let limits = JsonLimits::new(maximum, 128, 1_000_000, 4096)
        .map_err(|_| Error::Budget("Claim retained material limits"))?;
    let document = parse_json(raw, JsonMode::PublishedStrict, limits)
        .map_err(|e| Error::Source(e.to_string()))?;
    let payload = document
        .root()
        .object_get("source_record")
        .and_then(|v| v.object_get("payload"))
        .ok_or(Error::Invalid("Claim retained owner payload"))?;
    emit_value_preserved_json(payload, limits).map_err(|e| Error::Source(e.to_string()))
}
fn by_id(rows: &[Value]) -> Result<BTreeMap<String, Value>> {
    let mut result = BTreeMap::new();
    for row in rows {
        if result
            .insert(bytes::text(row, "id")?.to_owned(), row.clone())
            .is_some()
        {
            return Err(Error::Invalid("Claim normalized duplicate"));
        }
    }
    Ok(result)
}
fn candidate_nodes(
    rows: &BTreeMap<(String, String), (Value, Vec<u8>)>,
    dossiers: &[String],
) -> Vec<CandidateNode> {
    rows.iter()
        .map(|((graph, key), (row, raw))| CandidateNode {
            raw: seek(graph, key, raw.clone()),
            dossier_ref: normalization::declared_dossier(row, graph)
                .filter(|d| dossiers.contains(d)),
        })
        .collect()
}
fn candidate_relations(
    rows: &BTreeMap<String, (String, Value, Vec<u8>, Option<String>)>,
) -> Vec<CandidateRelation> {
    rows.values()
        .map(|(graph, row, raw, identity)| CandidateRelation {
            raw: seek(graph, row["edge_id"].as_str().unwrap_or(""), raw.clone()),
            identity_id: identity.clone(),
        })
        .collect()
}
fn traces(rows: &BTreeMap<String, Vec<u8>>) -> Vec<SeekRow> {
    rows.iter()
        .map(|(id, raw)| seek("source-claims", id, raw.clone()))
        .collect()
}
fn retained(rows: &BTreeMap<String, (Value, Vec<u8>)>) -> Vec<RetainedCandidateNode> {
    rows.values()
        .map(|(normalized, owner_material)| RetainedCandidateNode {
            normalized: normalized.clone(),
            owner_material: owner_material.clone(),
        })
        .collect()
}
#[allow(clippy::too_many_arguments)]
pub(super) fn normalize(
    tx: &Transaction<'_>,
    roots: &mut Roots,
    addition: &AssembledAddition,
    context_state: &Value,
    limits: ClaimCandidateLimits,
    dependency_limits: deps::Limits,
    registries: &ClaimCandidateRegistries<'_>,
) -> Result<Closure> {
    let mut graph = Graph::new(tx, dependency_limits);
    let mut context = Context::new(tx, dependency_limits);
    let dossiers: Vec<String> = context_state["dossier_refs"]
        .as_array()
        .ok_or(Error::Invalid("Claim dossier refs"))?
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .ok_or(Error::Invalid("Claim dossier ref"))
        })
        .collect::<Result<_>>()?;
    let mut old_nodes = BTreeMap::new();
    let mut new_nodes = BTreeMap::new();
    let mut raw_changes = Vec::new();
    for ((source, key), row) in &addition.nodes {
        let old = roots.get_with_material(role(source)?, "nodes", key)?;
        if let Some((value, raw)) = old {
            if bytes::row_digest(&value, limits.max_row_bytes)?
                != bytes::row_digest(&row.value, limits.max_row_bytes)?
            {
                return Err(Error::Invalid("Claim changed shared source carrier"));
            }
            old_nodes.insert((source.clone(), key.clone()), (value, raw));
        } else {
            raw_changes.push(Change {
                collection: "nodes".to_owned(),
                key: key.clone(),
                before_sha256: None,
                after: row.value.clone(),
            });
        }
        new_nodes.insert(
            (source.clone(), key.clone()),
            (row.value.clone(), row.raw.clone()),
        );
    }
    let affected: BTreeSet<String> = addition
        .nodes
        .keys()
        .map(|(g, id)| format!("{g}:{id}"))
        .collect();
    if affected.len() > limits.max_nodes {
        return Err(Error::Budget("Claim affected nodes"));
    }
    let incident = graph.incidence(&affected, limits.max_relations)?;
    let mut prior_relations = BTreeMap::new();
    let mut prior_specs = BTreeMap::new();
    for id in incident {
        let (row, raw) = graph.body("relation", &id)?;
        let source = bytes::text(&row, "source_graph")?.to_owned();
        let native = bytes::text(&row, "native_id")?;
        let identity = id
            .strip_prefix(&(source.clone() + ":"))
            .filter(|s| *s != native)
            .map(str::to_owned);
        let material = owner_material(&raw, limits.max_row_bytes)?;
        let value = bytes::parse(&material, limits.max_row_bytes)?;
        prior_specs.insert(id.clone(), (source, value, material, identity));
        prior_relations.insert(id, row);
    }
    let mut specs = prior_specs.clone();
    for ((source, key), row) in &addition.edges {
        if roots.get(role(source)?, "edges", key)?.is_some() {
            return Err(Error::Invalid("Claim new raw relation exists"));
        }
        let id = format!("{source}:{key}");
        if specs
            .insert(
                id,
                (source.clone(), row.value.clone(), row.raw.clone(), None),
            )
            .is_some()
        {
            return Err(Error::Invalid("Claim relation collides with incidence"));
        }
        raw_changes.push(Change {
            collection: "edges".to_owned(),
            key: key.clone(),
            before_sha256: None,
            after: row.value.clone(),
        });
    }
    if specs.len() > limits.max_relations {
        return Err(Error::Budget("Claim complete incidence"));
    }
    let mut new_traces = BTreeMap::new();
    for row in &addition.traces {
        let id = bytes::text(&row.value, "claim_ref")?.to_owned();
        if roots
            .get("bibliographic-claims", "claim_traces", &id)?
            .is_some()
            || new_traces.insert(id.clone(), row.raw.clone()).is_some()
        {
            return Err(Error::Invalid("Claim new trace exists"));
        }
        raw_changes.push(Change {
            collection: "claim_traces".to_owned(),
            key: id,
            before_sha256: None,
            after: row.value.clone(),
        });
    }
    let mut support = BTreeSet::new();
    let mut references = BTreeSet::new();
    for (source, row, _, _) in specs.values() {
        for end in ["from", "to"] {
            let g = row
                .get(format!("{end}_source_graph"))
                .and_then(Value::as_str)
                .unwrap_or(source);
            let id = bytes::text(row, &format!("{end}_id"))?;
            support.insert(format!("{g}:{id}"));
        }
        if let Some(reference) = row["claim_ref"].as_str() {
            references.insert((source.clone(), reference.to_owned()));
        }
    }
    let mut old_traces = BTreeMap::new();
    for (source, reference) in references {
        if source == "source-claims" && !new_traces.contains_key(&reference) {
            if old_traces.len() + new_traces.len() >= limits.max_traces {
                return Err(Error::Budget("Claim governing traces"));
            }
            let (_, raw) = roots
                .get_with_material("bibliographic-claims", "claim_traces", &reference)?
                .ok_or(Error::Invalid("Claim retained governing trace missing"))?;
            old_traces.insert(reference.clone(), raw);
        }
        if source == "source-claims" && new_traces.contains_key(&reference) {
            if context.head(&source, &reference)?.is_some() {
                return Err(Error::Invalid("Claim context occupied"));
            }
        } else {
            support.extend(context.members(&source, &reference, limits.max_nodes)?);
        }
    }
    for raw in old_traces.values().chain(new_traces.values()) {
        let trace = bytes::parse(raw, limits.max_row_bytes)?;
        for field in ["claim_node_id", "subject_node_id", "object_node_id"] {
            support.insert(format!("source-claims:{}", bytes::text(&trace, field)?));
        }
    }
    support.retain(|id| !affected.contains(id));
    if support.len() + affected.len() > limits.max_nodes {
        return Err(Error::Budget("Claim endpoint/context closure"));
    }
    let mut retained_nodes = BTreeMap::new();
    for id in support {
        let (row, raw) = graph.body("node", &id)?;
        retained_nodes.insert(id, (row, owner_material(&raw, limits.max_row_bytes)?));
    }
    let mut previous_nodes = BTreeMap::new();
    for (source, key) in old_nodes.keys() {
        let id = format!("{source}:{key}");
        previous_nodes.insert(id.clone(), graph.body("node", &id)?.0);
    }
    let mut positions = BTreeMap::new();
    for (id, row) in retained_nodes
        .iter()
        .map(|(id, (row, _))| (id, row))
        .chain(previous_nodes.iter())
    {
        if ["claim", "annotation-claim"].contains(&row["kind_id"].as_str().unwrap_or("")) {
            let position = context.position(row, limits.max_nodes)?;
            if positions.insert(position, id.clone()).is_some() {
                return Err(Error::Invalid("Claim repeated context position"));
            }
        }
    }
    let order: Vec<String> = positions.into_values().collect();
    let before = normalization::normalize_claim_candidate(
        ClaimCandidateInput {
            nodes: candidate_nodes(&old_nodes, &dossiers),
            relations: candidate_relations(&prior_specs),
            retained_nodes: retained(&retained_nodes),
            traces: traces(&old_traces),
            dossier_refs: dossiers.clone(),
            context_node_order: order.clone(),
            normalization_binding: registries.expected_normalization_binding.clone(),
        },
        registries,
        limits,
    )?;
    if by_id(&before.nodes)? != previous_nodes || by_id(&before.relations)? != prior_relations {
        return Err(Error::Invalid(
            "Claim closure does not reproduce prepared predecessor",
        ));
    }
    let mut context_ids = Vec::new();
    for raw in new_traces.values() {
        let row = bytes::parse(raw, limits.max_row_bytes)?;
        context_ids.push(format!(
            "source-claims:{}",
            bytes::text(&row, "claim_node_id")?
        ));
    }
    let mut after_order = order;
    after_order.extend(context_ids.clone());
    let mut all_traces = old_traces;
    all_traces.extend(new_traces);
    let after = normalization::normalize_claim_candidate(
        ClaimCandidateInput {
            nodes: candidate_nodes(&new_nodes, &dossiers),
            relations: candidate_relations(&specs),
            retained_nodes: retained(&retained_nodes),
            traces: traces(&all_traces),
            dossier_refs: dossiers,
            context_node_order: after_order,
            normalization_binding: registries.expected_normalization_binding.clone(),
        },
        registries,
        limits,
    )?;
    for row in &after.nodes {
        let id = bytes::text(row, "id")?;
        let keys = context::keys(row)?;
        if context_ids.iter().any(|v| v == id) {
            if keys
                != vec![(
                    "source-claims".to_owned(),
                    bytes::text(row, "entity_id")?.to_owned(),
                )]
            {
                return Err(Error::Invalid("Claim independent singleton context"));
            }
        } else if keys != context::keys(previous_nodes.get(id).unwrap_or(&json!({})))? {
            return Err(Error::Invalid("Claim changed other context"));
        }
    }
    let changes = graph.changes(
        &previous_nodes,
        &prior_relations,
        &after.nodes,
        &after.relations,
    )?;
    let nodes = by_id(&after.nodes)?;
    Ok(Closure {
        changes,
        raw_changes,
        context_ids,
        nodes,
        normalized_nodes: after.nodes.len(),
        normalized_relations: after.relations.len(),
    })
}
