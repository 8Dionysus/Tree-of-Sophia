//! Full deterministic graph projection, lens membership, clusters and review material.
use crate::source_philosophy_multilingual::Multilingual;
use crate::source_philosophy_support::check_run;
use crate::source_philosophy_support::{
    array, bytes_with_check, digest_with_check, fallback, required, sha1_hex, string, string_set,
    truth,
};
use crate::source_philosophy_views::{ATLAS_REF, LENS_CONTRACT, VIEW_CONTRACT};
use crate::{Error, Result};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Instant;
/// Opt-in operator diagnostics only: at most 64 bounded metadata records per worker.
/// No authored content, source identity, result bytes, or assertion is changed.
pub fn graph_phase_mark(phase: &'static str, view_id: Option<&str>) {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    static START: OnceLock<Instant> = OnceLock::new();
    static RECORDS: AtomicUsize = AtomicUsize::new(0);
    if !*ENABLED
        .get_or_init(|| std::env::var_os("TOS_PHI_GRAPH_PHASE_TRACE").is_some_and(|v| v == "1"))
        || RECORDS.fetch_add(1, Ordering::Relaxed) >= 64
    {
        return;
    }
    let elapsed = START.get_or_init(Instant::now).elapsed().as_millis();
    let view = view_id.filter(|id| id.len() <= 128).unwrap_or("");
    eprintln!("phi-graph-phase elapsed_ms={elapsed} phase={phase} view={view:?}");
}

fn predicate_layers(key: &str) -> BTreeSet<String> {
    let layers: &[&str] = match key {
        "belongs_to_genre" => &["conceptual-relation"],
        "canonized_by" => &["canonical-relation", "transmission-relation"],
        "commented_by" => &["transmission-relation"],
        "contested_by" => &["conceptual-relation", "evidence-relation"],
        "contains_dossier" => &["source-relation"],
        "contains_row" => &["source-relation"],
        "contains_table" => &["source-relation"],
        "contains_view" => &["source-relation"],
        "develops_concept" => &["conceptual-relation"],
        "figure_anchor" => &["historical-relation", "source-relation"],
        "fragments_preserved_by" => &["evidence-relation", "transmission-relation"],
        "has_atlas" => &["source-relation"],
        "has_candidate_node" => &["candidate-relation"],
        "has_node_type_pressure" => &["candidate-relation", "evidence-relation"],
        "has_prepared_dossier" => &["evidence-relation", "source-relation"],
        "has_relation_pressure" => &["candidate-relation", "evidence-relation"],
        "has_section" => &["source-relation"],
        "has_view_section" => &["source-relation"],
        "influences" => &["conceptual-relation", "historical-relation"],
        "institutionalized_in" => &["historical-relation"],
        "polemicizes_with" => &["conceptual-relation", "evidence-relation"],
        "preserved_in" => &["evidence-relation", "transmission-relation"],
        "preserves_in" => &["evidence-relation", "transmission-relation"],
        "receives_from" => &["transmission-relation"],
        "survives_as" => &[
            "evidence-relation",
            "historical-relation",
            "transmission-relation",
        ],
        "translated_into" => &["transmission-relation"],
        "transforms_concept" => &["conceptual-relation"],
        "transmits_to" => &["transmission-relation"],
        "uncertain_relation" => &["evidence-relation"],
        "uses_language" => &["source-relation"],
        "uses_script" => &["source-relation"],
        _ => &["source-relation"],
    };
    layers.iter().map(|s| s.to_string()).collect()
}
fn type_layers(key: &str) -> BTreeSet<String> {
    let layers: &[&str] = match key {
        "atlas" => &["source-relation"],
        "atlas-node-type" => &["source-relation"],
        "atlas-relation-kind" => &["source-relation"],
        "atlas-section" => &["source-relation"],
        "candidate-endpoint" => &["candidate-relation", "evidence-relation"],
        "candidate-node" => &["candidate-relation"],
        "domain-root" => &["source-relation"],
        "graph-view" => &["source-relation"],
        "master-table" => &["source-relation"],
        "master-table-row" => &["historical-relation", "source-relation"],
        "prepared-dossier" => &["evidence-relation", "source-relation"],
        "view-section" => &["source-relation"],
        _ => &["source-relation"],
    };
    layers.iter().map(|s| s.to_string()).collect()
}
pub const GRAPH_SCHEMA: &str = "ToS/contracts/philosophy-graph-projection.schema.json";
pub const GRAPH_REF: &str = "ToS/derived-exports/philosophy_graph_projection.min.json";
pub const VIEWS_REF: &str = "ToS/derived-exports/philosophy_graph_views.min.json";
pub const CLUSTER_CONTRACT: &str = "ToS/philosophy/graph-workbench/clusters/cluster-contracts.json";
pub const REVIEW_CONTRACT: &str =
    "ToS/philosophy/graph-workbench/review-packets/review-packet-contract.json";
#[derive(Clone, Copy, Debug)]
pub struct GraphLimits {
    pub max_nodes: usize,
    pub max_edges: usize,
    pub max_views: usize,
    pub max_clusters: usize,
    pub max_material_bytes: usize,
    pub max_work_units: u64,
}
impl Default for GraphLimits {
    fn default() -> Self {
        Self {
            max_nodes: 100_000,
            max_edges: 200_000,
            max_views: 256,
            max_clusters: 10_000,
            max_material_bytes: 256 * 1024 * 1024,
            max_work_units: 100_000_000,
        }
    }
}
struct Work<'a> {
    units: u64,
    limit: u64,
    deadline: Instant,
    cancelled: &'a AtomicBool,
    material_bytes: usize,
    material_limit: usize,
}
impl Work<'_> {
    fn reserve(&mut self, value: &Value) -> Result<()> {
        check_run(self.deadline, self.cancelled)?;
        let remaining = self
            .material_limit
            .checked_sub(self.material_bytes)
            .ok_or(Error::Budget("philosophy graph material"))?;
        if remaining == 0 {
            return Err(Error::Budget("philosophy graph material"));
        }
        let deadline = self.deadline;
        let cancelled = self.cancelled;
        let mut check = || check_run(deadline, cancelled);
        self.material_bytes = self
            .material_bytes
            .checked_add(bytes_with_check(value, remaining, &mut check)?.len())
            .ok_or(Error::Budget("philosophy graph material"))?;
        Ok(())
    }
    fn charge(&mut self, n: usize) -> Result<()> {
        check_run(self.deadline, self.cancelled)?;
        self.units = self
            .units
            .checked_add(n as u64)
            .ok_or(Error::Budget("philosophy graph work"))?;
        if self.units > self.limit {
            return Err(Error::Budget("philosophy graph work"));
        }
        Ok(())
    }
}
fn node_layers(n: &Value) -> BTreeSet<String> {
    let mut layers = type_layers(n["node_type"].as_str().unwrap_or(""));
    let p = &n["properties"];
    if p["canon_status"] == "pre-canon"
        || p["authority_posture"]
            .as_str()
            .is_some_and(|s| s.ends_with("_candidate"))
    {
        layers.insert("candidate-relation".into());
    }
    if !fallback(&p["priority"], "")
        .trim_matches(crate::source_philosophy_support::source_space)
        .is_empty()
    {
        layers.insert("evidence-relation".into());
    }
    layers
}
fn edge_layers(e: &Value) -> BTreeSet<String> {
    let mut layers = predicate_layers(e["predicate_id"].as_str().unwrap_or(""));
    let p = &e["properties"];
    if p["canon_status"] == "pre-canon"
        || p["authority_posture"]
            .as_str()
            .is_some_and(|s| s.ends_with("_candidate"))
    {
        layers.insert("candidate-relation".into());
    }
    if p["endpoint_resolution"] == "unresolved"
        || matches!(
            fallback(&p["confidence"], "")
                .trim_matches(crate::source_philosophy_support::source_space)
                .to_lowercase()
                .as_str(),
            "низкий" | "низкая" | "низкое" | "low"
        )
    {
        layers.insert("evidence-relation".into());
    }
    layers
}
fn node_matches(n: &Value, f: &Value) -> bool {
    let kind = n["node_type"].as_str().unwrap_or("");
    let labels = string_set(&f["node_type_keys"]);
    let relations = string_set(&f["relation_kind_keys"]);
    let label = n["label"].as_str().unwrap_or("");
    if kind == "atlas-node-type" && !labels.is_empty() {
        return labels.contains(label);
    }
    if kind == "atlas-relation-kind" && !relations.is_empty() {
        return relations.contains(label);
    }
    if string_set(&f["node_types"]).contains(kind) {
        return true;
    }
    if kind == "candidate-node" {
        return n["properties"]["original_node_type"]
            .as_str()
            .is_some_and(|k| labels.contains(k));
    }
    if kind == "candidate-endpoint" {
        return string_set(&f["node_types"]).contains(kind);
    }
    false
}
fn refs(items: &[&Value], nested: bool) -> BTreeSet<String> {
    refs_checked(items, nested, &mut || Ok(())).expect("no-op source-ref checkpoint")
}
fn refs_checked(
    items: &[&Value],
    nested: bool,
    check: &mut impl FnMut() -> Result<()>,
) -> Result<BTreeSet<String>> {
    let mut out = BTreeSet::new();
    for item in items {
        check()?;
        if let Some(source_ref) = item["source_ref"].as_str().filter(|s| !s.is_empty()) {
            out.insert(source_ref.to_owned());
        }
        if nested {
            if let Some(source_refs) = item["source_refs"].as_array() {
                for source_ref in source_refs {
                    check()?;
                    if let Some(source_ref) = source_ref.as_str().filter(|s| !s.is_empty()) {
                        out.insert(source_ref.to_owned());
                    }
                }
            }
        }
    }
    Ok(out)
}
fn projection_node(
    n: &Value,
    views: &BTreeSet<String>,
    layers: &BTreeSet<String>,
    multi: &Multilingual,
) -> Result<Value> {
    let p = if n["properties"].is_object() {
        n["properties"].clone()
    } else {
        json!({})
    };
    let multilingual = if n["multilingual"].is_object() {
        n["multilingual"].clone()
    } else {
        let mut props = p.clone();
        props["node_type"] = n["node_type"].clone();
        multi.label(required(n, "label")?, required(n, "source_ref")?, &props)?
    };
    Ok(
        json!({"node_id":n["node_id"],"label":n["label"],"multilingual":multilingual,"node_type":n["node_type"],"graph_layers":layers,"view_ids":views,"source_ref":n["source_ref"],"properties":p}),
    )
}
fn projection_edge(e: &Value, views: &BTreeSet<String>, layers: &BTreeSet<String>) -> Value {
    json!({"edge_id":e["edge_id"],"from_id":e["from_id"],"to_id":e["to_id"],"predicate_id":e["predicate_id"],"graph_layers":layers,"view_ids":views,"source_ref":e["source_ref"],"properties":if e["properties"].is_object(){e["properties"].clone()}else{json!({})}})
}
#[derive(Default)]
struct Membership {
    views: BTreeSet<String>,
    layers: BTreeSet<String>,
}
struct ViewMaterial {
    header: Value,
    nodes: Vec<String>,
    edges: Vec<String>,
}
fn field(n: &Value, key: &str) -> Option<String> {
    let v = if let Some(k) = key.strip_prefix("properties.") {
        &n["properties"][k]
    } else {
        match key {
            "label" | "node_type" | "source_ref" => &n[key],
            _ => return None,
        }
    };
    let text = match v {
        Value::Null | Value::Array(_) | Value::Object(_) => return None,
        Value::Bool(b) => b.to_string(),
        _ => string(v),
    };
    let text = text.trim_matches(crate::source_philosophy_support::source_space);
    if text.is_empty() {
        None
    } else {
        Some(text.to_owned())
    }
}
fn cluster_member(n: &Value, key: &Value) -> bool {
    let kinds = string_set(&key["node_types"]);
    if !kinds.is_empty() && !kinds.contains(n["node_type"].as_str().unwrap_or("")) {
        return false;
    }
    let Some(value) = field(n, key["field"].as_str().unwrap_or("")) else {
        return false;
    };
    let allowed = string_set(&key["allowed_values"]);
    allowed.is_empty() || allowed.contains(&value)
}
fn cluster_contract_guard(c: &Value) -> Result<()> {
    for (key, value) in [
        (
            "schema_version",
            "tos_philosophy_graph_cluster_contracts_v1",
        ),
        ("projection_ref", GRAPH_REF),
        ("downstream_consumer", "abyss-stack"),
    ] {
        if required(c, key)? != value {
            return Err(Error::Invalid("philosophy cluster contract"));
        }
    }
    Ok(())
}
fn build_clusters(
    c: &Value,
    nodes: &[Value],
    edges: &[Value],
    multi: &Multilingual,
    l: GraphLimits,
    w: &mut Work,
) -> Result<(Vec<Value>, Vec<Value>)> {
    cluster_contract_guard(c)?;
    let mut clusters = Vec::new();
    let mut unresolved = Vec::new();
    let families = array(c, "cluster_families")?;
    if families.len() > l.max_clusters {
        return Err(Error::Budget("philosophy cluster families"));
    }
    for family in families {
        let kind = required(family, "cluster_kind")?;
        let label = required(family, "label")?;
        let current = array(family, "current_member_keys")?;
        let future = array(family, "future_member_keys")?;
        let mut family_count = 0;
        if current.len() > l.max_clusters || future.len() > l.max_clusters {
            return Err(Error::Budget("philosophy cluster keys"));
        }
        for key in current {
            let key_field = required(key, "field")?;
            let mut groups: BTreeMap<String, Vec<&Value>> = BTreeMap::new();
            w.charge(nodes.len())?;
            for n in nodes {
                check_run(w.deadline, w.cancelled)?;
                if !string_set(&n["view_ids"]).is_empty() && cluster_member(n, key) {
                    if let Some(value) = field(n, key_field) {
                        groups.entry(value).or_default().push(n);
                    }
                }
            }
            for (value, member_nodes) in groups {
                let node_ids = member_nodes
                    .iter()
                    .map(|n| required(n, "node_id").map(str::to_owned))
                    .collect::<Result<BTreeSet<_>>>()?;
                w.charge(edges.len())?;
                let member_edges = edges
                    .iter()
                    .filter(|e| {
                        !string_set(&e["view_ids"]).is_empty()
                            && (node_ids.contains(e["from_id"].as_str().unwrap_or(""))
                                || node_ids.contains(e["to_id"].as_str().unwrap_or("")))
                            && (key_field != "source_ref" || refs(&[e], true).contains(&value))
                    })
                    .collect::<Vec<_>>();
                let mut all = member_nodes.clone();
                all.extend(member_edges.iter().copied());
                let source_refs = refs(&all, true);
                let layers = all
                    .iter()
                    .flat_map(|v| string_set(&v["graph_layers"]))
                    .collect::<BTreeSet<_>>();
                let views = all
                    .iter()
                    .flat_map(|v| string_set(&v["view_ids"]))
                    .collect::<BTreeSet<_>>();
                let cluster_label = format!("{label}: {value}");
                let id = format!(
                    "cluster:{kind}:{}",
                    &sha1_hex(&format!("{kind}|{key_field}|{value}"))[..12]
                );
                let out = json!({"cluster_id":id,"cluster_kind":kind,"label":cluster_label,"multilingual":multi.label(&cluster_label,CLUSTER_CONTRACT,&json!({"cluster_kind":kind,"member_key":key_field,"member_value":value}))?,"member_key":key_field,"member_value":value,"member_node_ids":node_ids,"member_edge_ids":member_edges.iter().map(|e|required(e,"edge_id").map(str::to_owned)).collect::<Result<BTreeSet<_>>>()?,"view_ids":views,"graph_layers":layers,"source_ref":CLUSTER_CONTRACT,"source_refs":source_refs,"properties":{"family_label":label,"review_use":fallback(&family["review_use"],""),"future_member_keys":future.iter().map(string).collect::<Vec<_>>(),"member_count":member_nodes.len(),"edge_count":member_edges.len()}});
                w.reserve(&out)?;
                clusters.push(out);
                family_count += 1;
                if clusters.len() > l.max_clusters {
                    return Err(Error::Budget("philosophy clusters"));
                }
            }
        }
        if family_count == 0 {
            let surface = json!({"surface_id":format!("unresolved-cluster-family:{kind}"),"kind":"unresolved-cluster-family","cluster_kind":kind,"source_ref":CLUSTER_CONTRACT,"message":"current projection has no source-owned member key coverage for this cluster family yet","future_member_keys":future.iter().map(string).collect::<Vec<_>>()});
            w.reserve(&surface)?;
            unresolved.push(surface);
        }
    }
    clusters.sort_by(|a, b| a["cluster_id"].as_str().cmp(&b["cluster_id"].as_str()));
    Ok((clusters, unresolved))
}
fn layer_counts(
    layers: &[Value],
    views: &[Value],
    nodes: &[&Value],
    edges: &[&Value],
    clusters: &[&Value],
    include_views: bool,
) -> Result<Vec<Value>> {
    layer_counts_checked(
        layers,
        views,
        nodes,
        edges,
        clusters,
        include_views,
        &mut || Ok(()),
    )
}
fn layer_counts_checked(
    layers: &[Value],
    views: &[Value],
    nodes: &[&Value],
    edges: &[&Value],
    clusters: &[&Value],
    include_views: bool,
    check: &mut impl FnMut() -> Result<()>,
) -> Result<Vec<Value>> {
    let mut out = Vec::new();
    for layer in layers {
        check()?;
        let id = required(layer, "layer_id")?;
        let n = filter_layer(nodes, id, check)?;
        let e = filter_layer(edges, id, check)?;
        let c = filter_layer(clusters, id, check)?;
        let mut all = n.clone();
        all.extend(e.iter().copied());
        all.extend(c.iter().copied());
        let source_ref_count = refs_checked(&all, true, check)?.len();
        let mut row = json!({"layer_id":id,"node_count":n.len(),"edge_count":e.len(),"cluster_count":c.len(),"source_ref_count":source_ref_count});
        if include_views {
            let mut view_count = 0;
            for view in views {
                check()?;
                if assertion_string_set(&view["graph_layers"], check)?.contains(id) {
                    view_count += 1;
                }
            }
            row["view_count"] = json!(view_count);
        }
        out.push(row);
    }
    Ok(out)
}
fn filter_layer<'a>(
    items: &[&'a Value],
    layer_id: &str,
    check: &mut impl FnMut() -> Result<()>,
) -> Result<Vec<&'a Value>> {
    let mut out = Vec::new();
    for item in items {
        check()?;
        if assertion_string_set(&item["graph_layers"], check)?.contains(layer_id) {
            out.push(*item);
        }
    }
    Ok(out)
}
fn stable_items(items: &[&Value], key: &str) -> Result<Vec<Value>> {
    stable_items_checked(items, key, &mut || Ok(()))
}
fn stable_items_checked(
    items: &[&Value],
    key: &str,
    check: &mut impl FnMut() -> Result<()>,
) -> Result<Vec<Value>> {
    let mut sorted = Vec::with_capacity(items.len());
    for item in items {
        check()?;
        sorted.push(*item);
    }
    sorted.sort_by(|a, b| a[key].as_str().cmp(&b[key].as_str()));
    let mut out = Vec::with_capacity(sorted.len());
    for item in sorted {
        check()?;
        out.push(item.clone());
    }
    Ok(out)
}
fn fingerprint_checked(
    v: &ViewMaterial,
    nodes: &[&Value],
    edges: &[&Value],
    clusters: &[&Value],
    max: usize,
    check: &mut impl FnMut() -> Result<()>,
) -> Result<String> {
    let mut cluster_ids = BTreeSet::new();
    for cluster in clusters {
        check()?;
        cluster_ids.insert(required(cluster, "cluster_id")?.to_owned());
    }
    let material = json!({
        "view_id":v.header["view_id"],
        "node_ids":v.nodes,
        "edge_ids":v.edges,
        "cluster_ids":cluster_ids,
        "nodes":stable_items_checked(nodes,"node_id",check)?,
        "edges":stable_items_checked(edges,"edge_id",check)?,
        "clusters":stable_items_checked(clusters,"cluster_id",check)?,
        "graph_layers":v.header["graph_layers"],
        "source_refs":v.header["source_refs"]
    });
    check()?;
    let result = digest_with_check(&material, max, check)?;
    check()?;
    Ok(result)
}
fn integer_limit(v: &Value, default: usize, max: usize) -> Result<usize> {
    let value = if truth(v) {
        v.as_u64()
            .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
            .ok_or(Error::Invalid("philosophy review integer limit"))?
    } else {
        default as u64
    };
    let value = usize::try_from(value).map_err(|_| Error::Budget("philosophy review limit"))?;
    if value > max {
        return Err(Error::Budget("philosophy review limit"));
    }
    Ok(value)
}
fn pressure(nodes: &[&Value]) -> Value {
    let mut count: BTreeMap<String, u64> = BTreeMap::new();
    for n in nodes {
        let key = match n["node_type"].as_str() {
            Some("master-table-row") => {
                Some(field(n, "properties.status").unwrap_or("missing".into()))
            }
            Some("candidate-node") => {
                Some(field(n, "properties.canon_status").unwrap_or("pre-canon".into()))
            }
            _ => None,
        };
        if let Some(k) = key {
            *count.entry(k).or_default() += 1;
        }
    }
    json!(count)
}
fn degree_counts(edges: &[&Value]) -> BTreeMap<String, (usize, usize)> {
    let mut degrees = BTreeMap::new();
    let mut order = 0;
    for e in edges {
        for key in ["from_id", "to_id"] {
            let id = fallback(&e[key], "");
            if !id.is_empty() {
                let entry = degrees.entry(id).or_insert_with(|| {
                    let n = order;
                    order += 1;
                    (0, n)
                });
                entry.0 += 1;
            }
        }
    }
    degrees
}
fn review_material(
    views: &[ViewMaterial],
    nodes: &[Value],
    edges: &[Value],
    layers: &[Value],
    clusters: &[Value],
    unresolved: &[Value],
    contract: &Value,
    l: GraphLimits,
    w: &mut Work,
) -> Result<(Vec<Value>, Value)> {
    for (key, value) in [
        (
            "schema_version",
            "tos_philosophy_graph_review_packet_contract_v1",
        ),
        ("projection_ref", GRAPH_REF),
        ("downstream_consumer", "abyss-stack"),
    ] {
        if required(contract, key)? != value {
            return Err(Error::Invalid("philosophy review packet contract"));
        }
    }
    let limits = &contract["default_limits"];
    if !limits.is_object() {
        return Err(Error::Invalid("philosophy review default limits"));
    }
    let threshold = integer_limit(
        &contract["dense_hub_degree_threshold"],
        24,
        l.max_edges.saturating_mul(2),
    )?;
    let changed = if contract["changed_subgraph"].is_object() {
        contract["changed_subgraph"].clone()
    } else {
        json!({"available":false,"reason":"not declared"})
    };
    let nmap = nodes
        .iter()
        .map(|n| Ok((required(n, "node_id")?.to_owned(), n)))
        .collect::<Result<BTreeMap<_, _>>>()?;
    let emap = edges
        .iter()
        .map(|e| Ok((required(e, "edge_id")?.to_owned(), e)))
        .collect::<Result<BTreeMap<_, _>>>()?;
    let mut packets = Vec::new();
    let mut fingerprints = Vec::new();
    for view in views {
        let id = required(&view.header, "view_id")?;
        let vn = view
            .nodes
            .iter()
            .map(|id| {
                nmap.get(id)
                    .copied()
                    .ok_or(Error::Invalid("philosophy view node closure"))
            })
            .collect::<Result<Vec<_>>>()?;
        let ve = view
            .edges
            .iter()
            .map(|id| {
                emap.get(id)
                    .copied()
                    .ok_or(Error::Invalid("philosophy view edge closure"))
            })
            .collect::<Result<Vec<_>>>()?;
        w.charge(vn.len() + ve.len() + clusters.len())?;
        let vc = clusters
            .iter()
            .filter(|c| string_set(&c["view_ids"]).contains(id))
            .collect::<Vec<_>>();
        let degrees = degree_counts(&ve);
        let mut ordered = degrees.iter().collect::<Vec<_>>();
        ordered.sort_by_key(|(_, (degree, order))| (std::cmp::Reverse(*degree), *order));
        let dense=ordered.into_iter().filter(|(id,(degree,_))|*degree>=threshold&&nmap.contains_key(id.as_str())).take(integer_limit(&limits["dense_hubs"],12,l.max_nodes)?).map(|(id,(degree,_))|json!({"node_id":id,"label":fallback(&nmap[id]["label"],id),"degree":degree,"source_ref":fallback(&nmap[id]["source_ref"],"")})).collect::<Vec<_>>();
        let isolated=vn.iter().filter(|n|!degrees.contains_key(n["node_id"].as_str().unwrap_or(""))).take(integer_limit(&limits["isolated_nodes"],20,l.max_nodes)?).map(|n|json!({"node_id":n["node_id"],"label":fallback(&n["label"],n["node_id"].as_str().unwrap_or("")),"source_ref":fallback(&n["source_ref"],"")})).collect::<Vec<_>>();
        let kinds = string_set(&view.header["collapse_rule"]["default_cluster_kinds"]);
        let unresolved = unresolved
            .iter()
            .filter(|s| {
                s["cluster_kind"]
                    .as_str()
                    .is_some_and(|k| kinds.contains(k))
            })
            .take(integer_limit(
                &limits["unresolved_diagnostics"],
                20,
                l.max_clusters,
            )?)
            .cloned()
            .collect::<Vec<_>>();
        let mut weak = Vec::new();
        for (kind, items, key) in [
            ("node", &vn, "node_id"),
            ("edge", &ve, "edge_id"),
            ("cluster", &vc, "cluster_id"),
        ] {
            for item in items {
                if !truth(&item["source_ref"]) && !truth(&item["source_refs"]) {
                    weak.push(json!({"item_id":item[key],"item_type":kind}));
                }
            }
        }
        weak.truncate(integer_limit(
            &limits["weak_source_refs"],
            20,
            l.max_nodes + l.max_edges + l.max_clusters,
        )?);
        let mut sorted = vc.clone();
        sorted.sort_by(|a, b| {
            a["cluster_kind"]
                .as_str()
                .cmp(&b["cluster_kind"].as_str())
                .then_with(|| {
                    b["member_node_ids"]
                        .as_array()
                        .map_or(0, Vec::len)
                        .cmp(&a["member_node_ids"].as_array().map_or(0, Vec::len))
                })
                .then_with(|| a["label"].as_str().cmp(&b["label"].as_str()))
        });
        let summaries=sorted.iter().take(integer_limit(&limits["cluster_summaries"],12,l.max_clusters)?).map(|c|json!({"cluster_id":c["cluster_id"],"cluster_kind":c["cluster_kind"],"label":c["label"],"node_count":c["member_node_ids"].as_array().map_or(0,Vec::len),"edge_count":c["member_edge_ids"].as_array().map_or(0,Vec::len),"source_ref_count":c["source_refs"].as_array().map_or(0,Vec::len)})).collect::<Vec<_>>();
        let deadline = w.deadline;
        let cancelled = w.cancelled;
        graph_phase_mark("producer_fingerprint.begin", Some(id));
        let fingerprint =
            fingerprint_checked(view, &vn, &ve, &vc, l.max_material_bytes, &mut || {
                check_run(deadline, cancelled)
            })?;
        graph_phase_mark("producer_fingerprint.end", Some(id));
        let mut changed = changed.clone();
        changed["snapshot_mode"] = json!("current-view-fingerprint");
        changed["current_view_fingerprint"] = json!(fingerprint);
        let packet = json!({"packet_id":format!("review-packet:{id}"),"view_id":id,"review_intent":fallback(&view.header["review_intent"],""),"active_filters":view.header["filters_applied"],"counts":{"nodes":vn.len(),"edges":ve.len(),"source_refs":view.header["source_refs"].as_array().map_or(0,Vec::len),"clusters":vc.len(),"weak_source_refs":weak.len(),"unresolved_diagnostics":unresolved.len(),"suspicious_dense_hubs":dense.len(),"isolated_nodes":isolated.len()},"layer_counts":layer_counts(layers,&[],&vn,&ve,&vc,false)?,"cluster_summaries":summaries,"weak_source_refs":weak,"unresolved_diagnostics":unresolved,"suspicious_dense_hubs":dense,"isolated_nodes":isolated,"candidate_to_canon_pressure":pressure(&vn),"changed_subgraph":changed,"recommended_human_review_route":fallback(&view.header["route_card"],""),"source_refs":view.header["source_refs"]});
        w.reserve(&packet)?;
        packets.push(packet);
        fingerprints.push(json!({"view_id":id,"fingerprint":fingerprint,"node_count":vn.len(),"edge_count":ve.len(),"cluster_count":vc.len(),"source_ref_count":view.header["source_refs"].as_array().map_or(0,Vec::len)}));
    }
    let nl = nodes
        .iter()
        .filter(|n| string_set(&n["view_ids"]).is_empty())
        .collect::<Vec<_>>();
    let el = edges
        .iter()
        .filter(|e| string_set(&e["view_ids"]).is_empty())
        .collect::<Vec<_>>();
    let material = json!({"node_ids":nodes.iter().map(|n|required(n,"node_id").map(str::to_owned)).collect::<Result<BTreeSet<_>>>()?,"edge_ids":edges.iter().map(|e|required(e,"edge_id").map(str::to_owned)).collect::<Result<BTreeSet<_>>>()?,"cluster_ids":clusters.iter().map(|c|required(c,"cluster_id").map(str::to_owned)).collect::<Result<BTreeSet<_>>>()?,"view_fingerprints":fingerprints,"unlensed_records":{"nodes":stable_items(&nl,"node_id")?,"edges":stable_items(&el,"edge_id")?}});
    let mut check = || check_run(w.deadline, w.cancelled);
    let projection_fingerprint = digest_with_check(&material, l.max_material_bytes, &mut check)?;
    let count_fingerprint = digest_with_check(
        &json!({"views":views.len(),"nodes":nodes.len(),"edges":edges.len(),"clusters":clusters.len()}),
        l.max_material_bytes,
        &mut check,
    )?;
    let snapshot = json!({"snapshot_schema_version":"tos_philosophy_graph_projection_snapshot_v1","current_snapshot":{"projection_fingerprint":projection_fingerprint,"count_fingerprint":count_fingerprint,"view_fingerprints":fingerprints},"diff_route":{"mode":"fingerprint-ready","changed_subgraph_available":false,"previous_snapshot_ref":null,"next_route":"compare current_snapshot against a previous reviewed philosophy graph projection snapshot"}});
    Ok((packets, snapshot))
}
pub fn build_graph(
    atlas: &Value,
    catalog: &Value,
    view_contract: &Value,
    cluster_contract: &Value,
    review_contract: &Value,
    multi: &Multilingual,
    l: GraphLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Value> {
    if l.max_nodes == 0
        || l.max_edges == 0
        || l.max_views == 0
        || l.max_views > 1024
        || l.max_clusters == 0
        || l.max_material_bytes == 0
        || l.max_material_bytes > 512 * 1024 * 1024
        || l.max_work_units == 0
    {
        return Err(Error::Budget("philosophy graph limits"));
    }
    if catalog["atlas_projection_ref"] != ATLAS_REF
        || view_contract["atlas_projection_ref"] != ATLAS_REF
        || catalog["lens_review_contract_ref"] != LENS_CONTRACT
    {
        return Err(Error::Invalid("philosophy graph derived route refs"));
    }
    let atlas_nodes = array(atlas, "nodes")?;
    let atlas_edges = array(atlas, "edges")?;
    let raw_views = array(catalog, "views")?;
    let graph_layers = array(catalog, "graph_layers")?;
    if atlas_nodes.len() > l.max_nodes
        || atlas_edges.len() > l.max_edges
        || raw_views.len() > l.max_views
        || graph_layers.len() > l.max_views
    {
        return Err(Error::Budget("philosophy graph inputs"));
    }
    for item in atlas_nodes.iter().chain(atlas_edges.iter()) {
        crate::source_philosophy_atlas::validate_authored_context(
            item,
            8 * 1024 * 1024,
            deadline,
            cancelled,
        )?;
    }
    let nmap = atlas_nodes
        .iter()
        .map(|n| Ok((required(n, "node_id")?.to_owned(), n)))
        .collect::<Result<BTreeMap<_, _>>>()?;
    let emap = atlas_edges
        .iter()
        .map(|e| Ok((required(e, "edge_id")?.to_owned(), e)))
        .collect::<Result<BTreeMap<_, _>>>()?;
    if nmap.len() != atlas_nodes.len() || emap.len() != atlas_edges.len() {
        return Err(Error::Invalid("philosophy graph duplicate atlas identity"));
    }
    let mut membership: BTreeMap<String, Membership> = BTreeMap::new();
    let mut edge_membership: BTreeMap<String, Membership> = BTreeMap::new();
    let mut views = Vec::new();
    let mut diagnostics = Vec::new();
    let mut work = Work {
        units: 0,
        limit: l.max_work_units,
        deadline,
        cancelled,
        material_bytes: 0,
        material_limit: l.max_material_bytes,
    };
    for view in raw_views {
        let id = required(view, "view_id")?;
        let filters = &view["current_projection_filters"];
        if !filters.is_object() {
            return Err(Error::Invalid("philosophy graph filters"));
        }
        work.charge(nmap.len() + atlas_edges.len())?;
        let selected = nmap
            .iter()
            .filter(|(_, n)| node_matches(n, filters))
            .map(|(id, _)| id.clone())
            .collect::<BTreeSet<_>>();
        let allowed = string_set(&filters["predicates"])
            .union(&string_set(&filters["relation_kind_keys"]))
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut vedges = atlas_edges
            .iter()
            .filter(|e| {
                allowed.contains(e["predicate_id"].as_str().unwrap_or(""))
                    && (selected.contains(e["from_id"].as_str().unwrap_or(""))
                        || selected.contains(e["to_id"].as_str().unwrap_or("")))
            })
            .collect::<Vec<_>>();
        vedges.sort_by(|a, b| a["edge_id"].as_str().cmp(&b["edge_id"].as_str()));
        let mut endpoints = selected.clone();
        for e in &vedges {
            endpoints.insert(required(e, "from_id")?.into());
            endpoints.insert(required(e, "to_id")?.into());
        }
        let vnodes = endpoints
            .iter()
            .filter_map(|id| nmap.get(id).map(|n| (id.clone(), *n)))
            .collect::<Vec<_>>();
        let mut node_layers_map = vnodes
            .iter()
            .map(|(id, n)| (id.clone(), node_layers(n)))
            .collect::<BTreeMap<_, _>>();
        let mut edge_layers_map = BTreeMap::new();
        for e in &vedges {
            let layers = edge_layers(e);
            edge_layers_map.insert(required(e, "edge_id")?.to_owned(), layers.clone());
            for key in ["from_id", "to_id"] {
                if let Some(n) = node_layers_map.get_mut(required(e, key)?) {
                    n.extend(layers.iter().cloned());
                }
            }
        }
        for (nid, n) in &vnodes {
            let m = membership.entry(nid.clone()).or_default();
            m.views.insert(id.into());
            m.layers.extend(
                node_layers_map
                    .get(nid)
                    .cloned()
                    .unwrap_or_else(|| node_layers(n)),
            );
        }
        for e in &vedges {
            let eid = required(e, "edge_id")?;
            let m = edge_membership.entry(eid.into()).or_default();
            m.views.insert(id.into());
            m.layers.extend(
                edge_layers_map
                    .get(eid)
                    .cloned()
                    .unwrap_or_else(|| edge_layers(e)),
            );
        }
        let mut vd = Vec::new();
        if vnodes.is_empty() {
            vd.push(json!({"level":"warning","path":fallback(&view["source_ref"],&fallback(&view["route_card"],VIEW_CONTRACT)),"message":"view filters currently select no atlas projection nodes"}));
        }
        diagnostics.extend(vd.iter().cloned());
        let mut header = json!({"view_id":id,"title":view["title"],"source_ref":view["source_ref"],"route_card":view["route_card"],"order":view["order"],"layout_hint":view["layout_hint"],"graph_layers":string_set(&view["graph_layers"]),"filters_applied":filters,"future_branch_filters":view["future_branch_filters"],"review_intent":view["review_intent"],"source_posture":view["source_posture"],"evidence_posture":view["evidence_posture"],"collapse_rule":view["collapse_rule"],"ordering_hints":view["ordering_hints"],"agent_packet_hint":view["agent_packet_hint"],"diagnostics":vd});
        let mut all = vnodes.iter().map(|(_, n)| *n).collect::<Vec<_>>();
        all.extend(vedges.iter().copied());
        header["source_refs"] = json!(refs(&all, false));
        work.reserve(&json!({"header":header,"node_ids":vnodes.iter().map(|(id,_)| id).collect::<Vec<_>>(),"edge_ids":vedges.iter().map(|e| &e["edge_id"]).collect::<Vec<_>>() }))?;
        views.push(ViewMaterial {
            header,
            nodes: vnodes.iter().map(|(id, _)| id.clone()).collect(),
            edges: vedges
                .iter()
                .map(|e| required(e, "edge_id").map(str::to_owned))
                .collect::<Result<Vec<_>>>()?,
        });
    }
    // Global source return retains candidates outside every lens without changing
    // any authored lens membership or cluster denominator.
    for (id, n) in &nmap {
        if n["node_type"] == "candidate-node" {
            membership.entry(id.clone()).or_insert_with(|| Membership {
                views: BTreeSet::new(),
                layers: node_layers(n),
            });
        }
    }
    for e in atlas_edges {
        let id = required(e, "edge_id")?;
        if !id.starts_with("edge:candidate-relation:") {
            continue;
        }
        edge_membership
            .entry(id.into())
            .or_insert_with(|| Membership {
                views: BTreeSet::new(),
                layers: edge_layers(e),
            });
        for key in ["from_id", "to_id"] {
            let id = required(e, key)?;
            let n = nmap.get(id).ok_or(Error::Invalid(
                "philosophy authored relation atlas endpoint",
            ))?;
            membership.entry(id.into()).or_insert_with(|| Membership {
                views: BTreeSet::new(),
                layers: node_layers(n),
            });
        }
    }
    let mut nodes = Vec::new();
    for (id, member) in &membership {
        let node = projection_node(nmap[id], &member.views, &member.layers, multi)?;
        work.reserve(&node)?;
        nodes.push(node);
    }
    let mut edges = Vec::new();
    for (id, member) in &edge_membership {
        let edge = projection_edge(emap[id], &member.views, &member.layers);
        work.reserve(&edge)?;
        edges.push(edge);
    }
    let (clusters, unresolved) =
        build_clusters(cluster_contract, &nodes, &edges, multi, l, &mut work)?;
    let headers = views.iter().map(|v| v.header.clone()).collect::<Vec<_>>();
    let layer_counts = layer_counts(
        graph_layers,
        &headers,
        &nodes.iter().collect::<Vec<_>>(),
        &edges.iter().collect::<Vec<_>>(),
        &clusters.iter().collect::<Vec<_>>(),
        true,
    )?;
    let (review_packets, snapshot) = review_material(
        &views,
        &nodes,
        &edges,
        graph_layers,
        &clusters,
        &unresolved,
        review_contract,
        l,
        &mut work,
    )?;
    let node_refs = views.iter().map(|v| v.nodes.len()).sum::<usize>();
    let edge_refs = views.iter().map(|v| v.edges.len()).sum::<usize>();
    let mut exported = Vec::new();
    for v in views {
        let mut header = v.header;
        header["node_ids"] = json!(v.nodes);
        header["edge_ids"] = json!(v.edges);
        exported.push(header);
    }
    let rules = &cluster_contract["collapse_rules"];
    if !rules.is_object() {
        return Err(Error::Invalid("philosophy cluster collapse rules"));
    }
    let out = json!({"schema_version":"tos_philosophy_graph_projection_v2","schema_ref":GRAPH_SCHEMA,"owner_repo":"Tree-of-Sophia","surface_kind":"derived_philosophy_graph_projection","source_refs":{"atlas_projection_ref":ATLAS_REF,"graph_view_catalog_ref":VIEWS_REF,"source_view_contract_ref":VIEW_CONTRACT,"lens_review_contract_ref":LENS_CONTRACT,"cluster_contract_ref":CLUSTER_CONTRACT,"review_packet_contract_ref":REVIEW_CONTRACT},"content_language_contract":multi.content_language_contract()?,"runtime_projection_boundary":{"runtime_owner":"abyss-stack","runtime_scope":["serve this projection through API, MCP, UI, layout, and cache behavior","materialize this projection into Neo4j as a rebuildable cache","render switchable graph lenses without writing runtime state back into ToS"],"tos_authority_scope":["atlas projection and graph-view catalog remain the source-owned inputs","this projection is generated and reproducible, not canon","source_ref fields route every projected node and edge back to ToS-owned surfaces"]},"validation_refs":["rust/crates/tos-compiler/src/source_philosophy_graph.rs","rust/crates/tos-ops-mechanics-plan/src/philosophy_products.rs","tests/conformance/rust/compiler_source_cases.rs","docs/validation/validation_lanes.json"],"counts":{"views":exported.len(),"graph_layers":graph_layers.len(),"nodes":nodes.len(),"edges":edges.len(),"source_refs":refs(&nodes.iter().chain(edges.iter()).collect::<Vec<_>>(),false).len(),"diagnostics":diagnostics.len(),"clusters":clusters.len(),"review_packets":review_packets.len(),"unresolved_review_surfaces":unresolved.len(),"view_node_references":node_refs,"view_edge_references":edge_refs,"unlensed_nodes":nodes.iter().filter(|v|string_set(&v["view_ids"]).is_empty()).count(),"unlensed_edges":edges.iter().filter(|v|string_set(&v["view_ids"]).is_empty()).count()},"visibility_model":{"default_payload_mode":"cluster-first","default_depth":1,"default_limit":integer_limit(&rules["runtime_payload_limit"],200,l.max_nodes+l.max_edges)?,"layer_ids":graph_layers.iter().map(|l|required(l,"layer_id").map(str::to_owned)).collect::<Result<Vec<_>>>()?,"expand_returns":rules.get("expand_returns").cloned().unwrap_or(json!([])),"cluster_contract_ref":CLUSTER_CONTRACT,"review_packet_contract_ref":REVIEW_CONTRACT,"lens_review_contract_ref":LENS_CONTRACT},"snapshot_review":snapshot,"graph_layers":graph_layers,"layer_counts":layer_counts,"views":exported,"nodes":nodes,"edges":edges,"clusters":clusters,"review_packets":review_packets,"unresolved_review_surfaces":unresolved,"diagnostics":diagnostics});
    validate_cross_refs(&out)?;
    let mut check = || check_run(deadline, cancelled);
    bytes_with_check(&out, l.max_material_bytes, &mut check)?;
    Ok(out)
}
fn validate_cross_refs(v: &Value) -> Result<()> {
    for (name, key) in [
        ("graph_layers", "layer_id"),
        ("views", "view_id"),
        ("nodes", "node_id"),
        ("edges", "edge_id"),
        ("clusters", "cluster_id"),
        ("review_packets", "packet_id"),
        ("unresolved_review_surfaces", "surface_id"),
    ] {
        let mut seen = BTreeSet::new();
        for item in array(v, name)? {
            if !seen.insert(required(item, key)?) {
                return Err(Error::Invalid(
                    "philosophy graph duplicate material identity",
                ));
            }
        }
    }
    let layers = array(v, "graph_layers")?
        .iter()
        .map(|l| required(l, "layer_id").map(str::to_owned))
        .collect::<Result<BTreeSet<_>>>()?;
    let views = array(v, "views")?
        .iter()
        .map(|l| required(l, "view_id").map(str::to_owned))
        .collect::<Result<BTreeSet<_>>>()?;
    let nodes = array(v, "nodes")?
        .iter()
        .map(|l| required(l, "node_id").map(str::to_owned))
        .collect::<Result<BTreeSet<_>>>()?;
    let edges = array(v, "edges")?
        .iter()
        .map(|l| required(l, "edge_id").map(str::to_owned))
        .collect::<Result<BTreeSet<_>>>()?;
    for name in ["nodes", "edges"] {
        for item in array(v, name)? {
            required(item, "source_ref")?;
            if !string_set(&item["graph_layers"]).is_subset(&layers) {
                return Err(Error::Invalid("philosophy graph layer closure"));
            }
            if name == "edges"
                && (!nodes.contains(required(item, "from_id")?)
                    || !nodes.contains(required(item, "to_id")?))
            {
                return Err(Error::Invalid("philosophy graph endpoint closure"));
            }
        }
    }
    for view in array(v, "views")? {
        if !string_set(&view["graph_layers"]).is_subset(&layers)
            || !string_set(&view["node_ids"]).is_subset(&nodes)
            || !string_set(&view["edge_ids"]).is_subset(&edges)
            || (string_set(&view["node_ids"]).is_empty() && array(view, "diagnostics")?.is_empty())
        {
            return Err(Error::Invalid("philosophy graph view closure"));
        }
    }
    for c in array(v, "clusters")? {
        if !string_set(&c["member_node_ids"]).is_subset(&nodes)
            || !string_set(&c["member_edge_ids"]).is_subset(&edges)
            || !string_set(&c["graph_layers"]).is_subset(&layers)
            || !string_set(&c["view_ids"]).is_subset(&views)
            || string_set(&c["source_refs"]).is_empty()
        {
            return Err(Error::Invalid("philosophy graph cluster closure"));
        }
    }
    for p in array(v, "review_packets")? {
        if !views.contains(required(p, "view_id")?) {
            return Err(Error::Invalid("philosophy review packet view"));
        }
    }
    Ok(())
}

fn assertion_strings(
    v: &Value,
    key: &str,
    check: &mut impl FnMut() -> Result<()>,
) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for item in array(v, key)? {
        check()?;
        out.push(
            item.as_str()
                .map(str::to_owned)
                .ok_or(Error::Invalid("philosophy graph validator string array"))?,
        );
    }
    Ok(out)
}

fn assertion_id_map<'a>(
    items: &'a [Value],
    key: &str,
    check: &mut impl FnMut() -> Result<()>,
) -> Result<BTreeMap<String, &'a Value>> {
    let mut out = BTreeMap::new();
    for item in items {
        check()?;
        let id = required(item, key)?.to_owned();
        out.insert(id, item);
    }
    Ok(out)
}

fn assertion_string_set(
    value: &Value,
    check: &mut impl FnMut() -> Result<()>,
) -> Result<BTreeSet<String>> {
    let mut out = BTreeSet::new();
    if let Some(values) = value.as_array() {
        for value in values {
            check()?;
            if let Some(value) = value.as_str().filter(|value| !value.is_empty()) {
                out.insert(value.to_owned());
            }
        }
    }
    Ok(out)
}

fn assertion_map_keys<T>(
    values: &BTreeMap<String, T>,
    check: &mut impl FnMut() -> Result<()>,
) -> Result<BTreeSet<String>> {
    let mut out = BTreeSet::new();
    for key in values.keys() {
        check()?;
        out.insert(key.clone());
    }
    Ok(out)
}

fn assertion_set_is_subset(
    values: &BTreeSet<String>,
    expected: &BTreeSet<String>,
    check: &mut impl FnMut() -> Result<()>,
) -> Result<bool> {
    for value in values {
        check()?;
        if !expected.contains(value) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn assertion_sets_equal(
    left: &BTreeSet<String>,
    right: &BTreeSet<String>,
    check: &mut impl FnMut() -> Result<()>,
) -> Result<bool> {
    if left.len() != right.len() {
        return Ok(false);
    }
    assertion_set_is_subset(left, right, check)
}

fn assertion_materials<'a>(
    ids: &[String],
    by_id: &BTreeMap<String, &'a Value>,
    error: &'static str,
    check: &mut impl FnMut() -> Result<()>,
) -> Result<Vec<&'a Value>> {
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        check()?;
        out.push(by_id.get(id).copied().ok_or(Error::Invalid(error))?);
    }
    Ok(out)
}

fn assertion_clusters_for_view<'a>(
    clusters: &'a [Value],
    view_id: &str,
    check: &mut impl FnMut() -> Result<()>,
) -> Result<Vec<&'a Value>> {
    let mut out = Vec::new();
    for cluster in clusters {
        check()?;
        if assertion_string_set(&cluster["view_ids"], check)?.contains(view_id) {
            out.push(cluster);
        }
    }
    Ok(out)
}

fn assertion_int(value: &Value) -> Result<i128> {
    if !truth(value) {
        return Ok(0);
    }
    match value {
        Value::Bool(value) => Ok(i128::from(u8::from(*value))),
        Value::Number(number) => {
            if let Some(value) = number.as_i64() {
                Ok(i128::from(value))
            } else if let Some(value) = number.as_u64() {
                Ok(i128::from(value))
            } else if let Some(value) = number.as_f64() {
                if value.is_finite() && value >= i128::MIN as f64 && value <= i128::MAX as f64 {
                    Ok(value as i128)
                } else {
                    Err(Error::Invalid("philosophy graph validator integer"))
                }
            } else {
                Err(Error::Invalid("philosophy graph validator integer"))
            }
        }
        Value::String(value) => value
            .trim_matches(crate::source_philosophy_support::source_space)
            .parse::<i128>()
            .map_err(|_| Error::Invalid("philosophy graph validator integer")),
        _ => Err(Error::Invalid("philosophy graph validator integer")),
    }
}

fn assertion_equal_number(value: &Value, expected: u64) -> bool {
    value.as_u64() == Some(expected)
        || value.as_i64() == Some(expected as i64)
        || value.as_f64() == Some(expected as f64)
}

/// Runs the maintained graph-projection validator assertions on a parsed
/// current payload. Schema validation and canonical rebuild parity belong to
/// the owner adapter; fingerprint and layer-count mechanics stay shared with
/// the native producer above.
pub fn validate_graph_assertions(
    current: &Value,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<()> {
    let mut check = || check_run(deadline, cancelled);
    check()?;
    let counts = current
        .get("counts")
        .filter(|value| value.is_object())
        .ok_or(Error::Invalid("philosophy graph validator counts"))?;
    if !assertion_equal_number(&counts["views"], 11) {
        return Err(Error::Invalid("philosophy graph validator view count"));
    }
    if !assertion_equal_number(&counts["graph_layers"], 7) {
        return Err(Error::Invalid("philosophy graph validator layer count"));
    }
    for key in [
        "nodes",
        "edges",
        "clusters",
        "view_node_references",
        "view_edge_references",
    ] {
        check()?;
        if assertion_int(&counts[key])? == 0 {
            return Err(Error::Invalid("philosophy graph validator material count"));
        }
    }
    if !assertion_equal_number(&counts["review_packets"], 11) {
        return Err(Error::Invalid(
            "philosophy graph validator review packet count",
        ));
    }
    if !assertion_equal_number(&counts["diagnostics"], 0) {
        return Err(Error::Invalid("philosophy graph validator diagnostics"));
    }

    if current["runtime_projection_boundary"]["runtime_owner"] != "abyss-stack" {
        return Err(Error::Invalid("philosophy graph validator runtime owner"));
    }
    let visibility = current
        .get("visibility_model")
        .filter(|value| value.is_object())
        .ok_or(Error::Invalid(
            "philosophy graph validator visibility model",
        ))?;
    if visibility["default_payload_mode"] != "cluster-first" {
        return Err(Error::Invalid("philosophy graph validator payload mode"));
    }
    let graph_layers = array(current, "graph_layers")?;
    let mut graph_layer_ids = BTreeSet::new();
    for layer in graph_layers {
        check()?;
        graph_layer_ids.insert(required(layer, "layer_id")?.to_owned());
    }
    if !assertion_sets_equal(
        &assertion_string_set(&visibility["layer_ids"], &mut check)?,
        &graph_layer_ids,
        &mut check,
    )? {
        return Err(Error::Invalid(
            "philosophy graph validator visibility layers",
        ));
    }

    let snapshot = current
        .get("snapshot_review")
        .filter(|value| value.is_object())
        .ok_or(Error::Invalid("philosophy graph validator snapshot review"))?;
    let current_snapshot = snapshot
        .get("current_snapshot")
        .filter(|value| value.is_object())
        .ok_or(Error::Invalid(
            "philosophy graph validator current snapshot",
        ))?;
    if snapshot["snapshot_schema_version"] != "tos_philosophy_graph_projection_snapshot_v1" {
        return Err(Error::Invalid("philosophy graph validator snapshot schema"));
    }
    let projection_fingerprint =
        current_snapshot["projection_fingerprint"]
            .as_str()
            .ok_or(Error::Invalid(
                "philosophy graph validator projection fingerprint",
            ))?;
    if projection_fingerprint.len() != 64 {
        return Err(Error::Invalid(
            "philosophy graph validator projection fingerprint",
        ));
    }
    let view_fingerprints = array(current_snapshot, "view_fingerprints")?;
    if view_fingerprints.len() != 11
        || !assertion_equal_number(&counts["views"], view_fingerprints.len() as u64)
    {
        return Err(Error::Invalid(
            "philosophy graph validator view fingerprints",
        ));
    }
    if snapshot["diff_route"]["mode"] != "fingerprint-ready" {
        return Err(Error::Invalid("philosophy graph validator diff route"));
    }

    let nodes = array(current, "nodes")?;
    let edges = array(current, "edges")?;
    let clusters = array(current, "clusters")?;
    let node_by_id = assertion_id_map(nodes, "node_id", &mut check)?;
    let edge_by_id = assertion_id_map(edges, "edge_id", &mut check)?;
    let node_ids = assertion_map_keys(&node_by_id, &mut check)?;
    let edge_ids = assertion_map_keys(&edge_by_id, &mut check)?;
    for edge in edges {
        check()?;
        if !node_ids.contains(required(edge, "from_id")?)
            || !node_ids.contains(required(edge, "to_id")?)
        {
            return Err(Error::Invalid("philosophy graph validator edge endpoint"));
        }
        if !truth(&edge["source_ref"]) {
            return Err(Error::Invalid("philosophy graph validator edge source ref"));
        }
    }
    for cluster in clusters {
        check()?;
        if !truth(&cluster["source_refs"]) {
            return Err(Error::Invalid(
                "philosophy graph validator cluster source refs",
            ));
        }
        if !assertion_set_is_subset(
            &assertion_string_set(&cluster["member_node_ids"], &mut check)?,
            &node_ids,
            &mut check,
        )? || !assertion_set_is_subset(
            &assertion_string_set(&cluster["member_edge_ids"], &mut check)?,
            &edge_ids,
            &mut check,
        )? {
            return Err(Error::Invalid("philosophy graph validator cluster members"));
        }
    }

    let view_rows = array(current, "views")?;
    let view_by_id = assertion_id_map(view_rows, "view_id", &mut check)?;
    let views = assertion_map_keys(&view_by_id, &mut check)?;
    let chronology = view_by_id
        .get("chronology")
        .copied()
        .ok_or(Error::Invalid("philosophy graph validator chronology view"))?;
    if chronology["layout_hint"] != "timeline-lanes"
        || !truth(&chronology["node_ids"])
        || !truth(&chronology["edge_ids"])
    {
        return Err(Error::Invalid("philosophy graph validator chronology view"));
    }
    let source_evidence = view_by_id
        .get("source-evidence")
        .copied()
        .ok_or(Error::Invalid(
            "philosophy graph validator source evidence view",
        ))?;
    if !assertion_string_set(&source_evidence["graph_layers"], &mut check)?
        .contains("evidence-relation")
    {
        return Err(Error::Invalid(
            "philosophy graph validator source evidence layer",
        ));
    }
    for view in view_by_id.values() {
        check()?;
        if !truth(&view["source_refs"]) {
            return Err(Error::Invalid(
                "philosophy graph validator view source refs",
            ));
        }
        if !truth(&view["review_intent"]) {
            return Err(Error::Invalid(
                "philosophy graph validator view review intent",
            ));
        }
        if !assertion_set_is_subset(
            &assertion_string_set(&view["node_ids"], &mut check)?,
            &node_ids,
            &mut check,
        )? || !assertion_set_is_subset(
            &assertion_string_set(&view["edge_ids"], &mut check)?,
            &edge_ids,
            &mut check,
        )? {
            return Err(Error::Invalid("philosophy graph validator view membership"));
        }
    }

    let mut fingerprint_by_view = BTreeMap::new();
    for item in view_fingerprints {
        check()?;
        if item.is_object() {
            if let Some(view_id) = item["view_id"].as_str() {
                fingerprint_by_view.insert(view_id.to_owned(), item["fingerprint"].clone());
            }
        }
    }
    let mut expected_fingerprints = BTreeMap::new();
    let max_fingerprint_bytes = 512 * 1024 * 1024;
    for (view_id, view) in &view_by_id {
        check()?;
        let node_id_list = assertion_strings(view, "node_ids", &mut check)?;
        let edge_id_list = assertion_strings(view, "edge_ids", &mut check)?;
        let mut sorted_node_ids = node_id_list.clone();
        let mut sorted_edge_ids = edge_id_list.clone();
        sorted_node_ids.sort();
        sorted_edge_ids.sort();
        check()?;
        let view_nodes = assertion_materials(
            &node_id_list,
            &node_by_id,
            "philosophy graph validator view node",
            &mut check,
        )?;
        let view_edges = assertion_materials(
            &edge_id_list,
            &edge_by_id,
            "philosophy graph validator view edge",
            &mut check,
        )?;
        let view_clusters = assertion_clusters_for_view(clusters, view_id, &mut check)?;
        let view_material = ViewMaterial {
            header: json!({
                "view_id": view_id,
                "graph_layers": view["graph_layers"],
                "source_refs": view["source_refs"],
            }),
            nodes: sorted_node_ids,
            edges: sorted_edge_ids,
        };
        graph_phase_mark("assertion_fingerprint.begin", Some(view_id.as_str()));
        let expected_fingerprint = fingerprint_checked(
            &view_material,
            &view_nodes,
            &view_edges,
            &view_clusters,
            max_fingerprint_bytes,
            &mut check,
        )?;
        graph_phase_mark("assertion_fingerprint.end", Some(view_id.as_str()));
        if fingerprint_by_view.get(view_id).and_then(Value::as_str)
            != Some(expected_fingerprint.as_str())
        {
            return Err(Error::Invalid(
                "philosophy graph validator view fingerprint",
            ));
        }
        expected_fingerprints.insert(view_id.clone(), expected_fingerprint);
    }

    let packet_rows = array(current, "review_packets")?;
    let packet_by_view = assertion_id_map(packet_rows, "view_id", &mut check)?;
    if !assertion_sets_equal(
        &assertion_map_keys(&packet_by_view, &mut check)?,
        &views,
        &mut check,
    )? {
        return Err(Error::Invalid("philosophy graph validator packet/view set"));
    }
    for (view_id, packet) in &packet_by_view {
        check()?;
        if packet["changed_subgraph"]["current_view_fingerprint"].as_str()
            != expected_fingerprints.get(view_id).map(String::as_str)
        {
            return Err(Error::Invalid(
                "philosophy graph validator packet fingerprint",
            ));
        }
        let view = view_by_id
            .get(view_id)
            .copied()
            .ok_or(Error::Invalid("philosophy graph validator packet view"))?;
        let node_id_list = assertion_strings(view, "node_ids", &mut check)?;
        let edge_id_list = assertion_strings(view, "edge_ids", &mut check)?;
        let view_nodes = assertion_materials(
            &node_id_list,
            &node_by_id,
            "philosophy graph validator packet node",
            &mut check,
        )?;
        let view_edges = assertion_materials(
            &edge_id_list,
            &edge_by_id,
            "philosophy graph validator packet edge",
            &mut check,
        )?;
        let view_clusters = assertion_clusters_for_view(clusters, view_id, &mut check)?;
        let expected_layer_counts = layer_counts_checked(
            graph_layers,
            &[],
            &view_nodes,
            &view_edges,
            &view_clusters,
            false,
            &mut check,
        )?;
        if packet.get("layer_counts") != Some(&json!(expected_layer_counts)) {
            return Err(Error::Invalid(
                "philosophy graph validator packet layer counts",
            ));
        }
    }

    let canon_packet = packet_by_view
        .get("canon-promotion")
        .copied()
        .ok_or(Error::Invalid("philosophy graph validator canon packet"))?;
    if !canon_packet
        .as_object()
        .is_some_and(|packet| packet.contains_key("candidate_to_canon_pressure"))
    {
        return Err(Error::Invalid("philosophy graph validator canon pressure"));
    }
    if fallback(
        &canon_packet["changed_subgraph"]["current_view_fingerprint"],
        "",
    )
    .len()
        != 64
    {
        return Err(Error::Invalid(
            "philosophy graph validator current fingerprint",
        ));
    }
    if !truth(&canon_packet["recommended_human_review_route"]) {
        return Err(Error::Invalid(
            "philosophy graph validator human review route",
        ));
    }
    check()?;
    Ok(())
}
