//! Complete source-owned view-card/lens catalog mechanics over authored atlas rows.
use crate::source_philosophy_support::{array, bytes, object, required, strings};
use crate::{Error, Result};
use regex::Regex;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
pub const VIEW_CONTRACT: &str = "ToS/philosophy/graph-workbench/views/view-contracts.json";
pub const LENS_CONTRACT: &str = "ToS/philosophy/graph-workbench/views/lens-review-contracts.json";
pub const VIEW_ROOT: &str = "ToS/philosophy/graph-workbench/views";
pub const LAYERS_SOURCE: &str = "ToS/philosophy/trunk/graph-layers/README.md";
pub const ATLAS_REF: &str = "ToS/derived-exports/philosophy_atlas_projection.min.json";
pub const VIEWS_SCHEMA: &str = "ToS/contracts/philosophy-graph-views.schema.json";
#[derive(Clone, Copy, Debug)]
pub struct ViewLimits {
    pub max_source_bytes: usize,
    pub max_views: usize,
    pub max_material_items: usize,
    pub max_output_bytes: usize,
}
impl Default for ViewLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: 4 * 1024 * 1024,
            max_views: 256,
            max_material_items: 100_000,
            max_output_bytes: 16 * 1024 * 1024,
        }
    }
}
fn lines(text: &str) -> Vec<&str> {
    text.split([
        '\n', '\u{000B}', '\u{000C}', '\u{001c}', '\u{001d}', '\u{001e}', '\u{0085}', '\u{2028}',
        '\u{2029}',
    ])
    .collect()
}
fn section<'a>(text: &'a str, heading: &str) -> Result<Vec<&'a str>> {
    let text = lines(text);
    let start = text
        .iter()
        .position(|line| line.trim() == format!("## {heading}"))
        .ok_or(Error::Invalid("philosophy view missing section"))?
        + 1;
    Ok(text[start..]
        .iter()
        .take_while(|line| !line.starts_with("## "))
        .copied()
        .collect())
}
fn paragraph(lines: &[&str]) -> String {
    lines
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}
fn bullets(lines: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = Vec::new();
    for line in lines {
        let s = line.trim();
        if s.is_empty() {
            continue;
        }
        if let Some(s) = s.strip_prefix("- ") {
            if !current.is_empty() {
                out.push(current.join(" "));
            }
            current = vec![s.trim()];
        } else if !current.is_empty() {
            current.push(s);
        }
    }
    if !current.is_empty() {
        out.push(current.join(" "));
    }
    out
}
fn markdown(raw: Vec<u8>) -> Result<String> {
    Ok(String::from_utf8(raw)
        .map_err(|_| Error::Source("philosophy view UTF-8".into()))?
        .replace("\r\n", "\n")
        .replace('\r', "\n"))
}
pub(crate) fn parse_card(raw: Vec<u8>) -> Result<Value> {
    let text = markdown(raw)?;
    let re = Regex::new(r"(?m)^#\s+(.+)$").map_err(|e| Error::Source(e.to_string()))?;
    let title = re
        .captures(&text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim())
        .ok_or(Error::Invalid("philosophy view H1"))?;
    let future = bullets(&section(&text, "Future Inputs")?);
    if future.is_empty() {
        return Err(Error::Invalid("philosophy view future inputs"));
    }
    Ok(
        json!({"title":title,"lens":paragraph(&section(&text,"Lens")?),"future_inputs":future,"boundary":paragraph(&section(&text,"Boundary")?)}),
    )
}
fn subset(v: &Value, allowed: &BTreeSet<String>) -> Result<()> {
    if strings(v)?.iter().any(|v| !allowed.contains(v)) {
        return Err(Error::Invalid("philosophy view filter/route subset"));
    }
    Ok(())
}
fn projection_sets(nodes: &[Value], edges: &[Value]) -> BTreeMap<String, BTreeSet<String>> {
    let mut p = BTreeMap::new();
    for k in [
        "node_types",
        "predicates",
        "row_fields",
        "node_type_keys",
        "relation_kind_keys",
        "graph_view_ids",
    ] {
        p.insert(k.into(), BTreeSet::new());
    }
    for n in nodes {
        if let Some(t) = n["node_type"].as_str() {
            p.get_mut("node_types").expect("set").insert(t.into());
            match t {
                "master-table-row" => {
                    if let Some(o) = n["properties"].as_object() {
                        p.get_mut("row_fields")
                            .expect("set")
                            .extend(o.keys().cloned());
                    }
                }
                "atlas-node-type" => {
                    if let Some(v) = n["label"].as_str() {
                        p.get_mut("node_type_keys").expect("set").insert(v.into());
                    }
                }
                "atlas-relation-kind" => {
                    if let Some(v) = n["label"].as_str() {
                        p.get_mut("relation_kind_keys")
                            .expect("set")
                            .insert(v.into());
                    }
                }
                "graph-view" => {
                    if let Some(v) = n["node_id"].as_str() {
                        let v = v.strip_prefix("graph-view:").unwrap_or(v);
                        p.get_mut("graph_view_ids").expect("set").insert(v.into());
                    }
                }
                _ => {}
            }
        }
    }
    for e in edges {
        if let Some(t) = e["predicate_id"].as_str() {
            p.get_mut("predicates").expect("set").insert(t.into());
        }
    }
    p
}
pub fn build_views<F>(
    read: &mut F,
    nodes: &[Value],
    edges: &[Value],
    l: ViewLimits,
) -> Result<Value>
where
    F: FnMut(&str) -> Result<Vec<u8>>,
{
    if l.max_source_bytes == 0
        || l.max_source_bytes > 8 * 1024 * 1024
        || l.max_views == 0
        || l.max_views > 1024
        || l.max_material_items == 0
        || l.max_output_bytes == 0
        || l.max_output_bytes > 64 * 1024 * 1024
        || nodes
            .len()
            .checked_add(edges.len())
            .is_none_or(|n| n > l.max_material_items)
    {
        return Err(Error::Budget("philosophy views limits"));
    }
    let source = object(&read(VIEW_CONTRACT)?, l.max_source_bytes)?;
    let lens = object(&read(LENS_CONTRACT)?, l.max_source_bytes)?;
    let manifest = object(
        &read("ToS/philosophy/philosophy.manifest.json")?,
        l.max_source_bytes,
    )?;
    for (k, v) in [
        ("schema_version", "tos_philosophy_graph_view_contracts_v1"),
        ("atlas_projection_ref", ATLAS_REF),
        ("downstream_consumer", "abyss-stack"),
    ] {
        if required(&source, k)? != v {
            return Err(Error::Invalid("philosophy view source contract"));
        }
    }
    for (k, v) in [
        ("schema_version", "tos_philosophy_lens_review_contracts_v1"),
        ("view_contract_ref", VIEW_CONTRACT),
        ("downstream_consumer", "abyss-stack"),
    ] {
        if required(&lens, k)? != v {
            return Err(Error::Invalid("philosophy lens source contract"));
        }
    }
    let requirements = &lens["default_requirements"];
    if !requirements.is_object() || requirements["ui_mcp_payload_mode"] != "cluster-first" {
        return Err(Error::Invalid("philosophy lens requirements"));
    }
    for k in ["minimum_source_ref_expectations", "diagnostics_expected"] {
        strings(&requirements[k])?;
    }
    let raw = read(LAYERS_SOURCE)?;
    if raw.len() > l.max_source_bytes {
        return Err(Error::Budget("philosophy graph layers bytes"));
    }
    let raw = markdown(raw)?;
    let re = Regex::new(r"^\|\s*`([^`]+)`\s*\|\s*(.*?)\s*\|\s*$")
        .map_err(|e| Error::Source(e.to_string()))?;
    let layers = lines(&raw)
        .iter()
        .filter_map(|line| re.captures(line))
        .map(|c| json!({"layer_id":&c[1],"use":c[2].trim(),"source_ref":LAYERS_SOURCE}))
        .collect::<Vec<_>>();
    if layers.is_empty() || layers.len() > l.max_material_items {
        return Err(Error::Budget("philosophy graph layers"));
    }
    let layer_ids = layers
        .iter()
        .map(|v| required(v, "layer_id").map(str::to_owned))
        .collect::<Result<BTreeSet<_>>>()?;
    let routes = strings(&manifest["graph_view_routes"])?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let p = projection_sets(nodes, edges);
    let raw_views = array(&source, "views")?;
    if raw_views.len() > l.max_views {
        return Err(Error::Budget("philosophy source views"));
    }
    let raw_ids = raw_views
        .iter()
        .map(|v| required(v, "view_id").map(str::to_owned))
        .collect::<Result<BTreeSet<_>>>()?;
    let mut reviews = BTreeMap::new();
    let raw_reviews = array(&lens, "views")?;
    if raw_reviews.len() > l.max_views {
        return Err(Error::Budget("philosophy lens reviews"));
    }
    for r in raw_reviews {
        let id = required(r, "view_id")?;
        for k in [
            "review_intent",
            "source_posture",
            "evidence_posture",
            "agent_packet_hint",
        ] {
            required(r, k)?;
        }
        if !r["collapse_rule"].is_object() {
            return Err(Error::Invalid("philosophy lens collapse rule"));
        }
        for k in ["default_cluster_kinds", "expand_to"] {
            strings(&r["collapse_rule"][k])?;
        }
        strings(&r["ordering_hints"])?;
        if reviews.insert(id.to_owned(), r).is_some() {
            return Err(Error::Invalid("philosophy duplicate lens review"));
        }
    }
    if reviews.keys().cloned().collect::<BTreeSet<_>>() != raw_ids {
        return Err(Error::Invalid("philosophy exact lens coverage"));
    }
    let mut sorted = raw_views.iter().collect::<Vec<_>>();
    sorted.sort_by_key(|v| v["order"].as_i64().unwrap_or(0));
    let mut views = Vec::new();
    let mut seen = BTreeSet::new();
    for view in sorted {
        let id = required(view, "view_id")?;
        let route = required(view, "route_card")?;
        if !seen.insert(id.to_owned())
            || !routes.contains(route)
            || route != format!("{VIEW_ROOT}/{id}.graph.md")
            || view["order"].as_i64().is_none()
        {
            return Err(Error::Invalid("philosophy view identity/order/route"));
        }
        subset(&view["graph_layers"], &layer_ids)?;
        if !view["current_projection_filters"].is_object()
            || !view["future_branch_filters"].is_object()
        {
            return Err(Error::Invalid("philosophy view filters"));
        }
        for k in [
            "node_types",
            "predicates",
            "row_fields",
            "node_type_keys",
            "relation_kind_keys",
        ] {
            subset(&view["current_projection_filters"][k], &p[k])?;
        }
        for k in ["node_kinds", "relation_kinds"] {
            strings(&view["future_branch_filters"][k])?;
        }
        strings(&view["group_by"])?;
        let allowed = p["row_fields"]
            .union(&BTreeSet::from([
                "relation_kind".into(),
                "node_type_key".into(),
            ]))
            .cloned()
            .collect::<BTreeSet<_>>();
        subset(&view["sort_fields"], &allowed)?;
        required(view, "layout_hint")?;
        let raw = read(route)?;
        if raw.len() > l.max_source_bytes {
            return Err(Error::Budget("philosophy view card bytes"));
        }
        let card = parse_card(raw)?;
        let review = reviews[id];
        let mut out = json!({"view_id":id,"title":card["title"],"route_card":route,"source_ref":route,"order":view["order"],"lens":card["lens"],"future_inputs":card["future_inputs"],"boundary":card["boundary"]});
        for k in [
            "graph_layers",
            "current_projection_filters",
            "future_branch_filters",
            "layout_hint",
            "group_by",
            "sort_fields",
        ] {
            out[k] = view[k].clone();
        }
        for k in [
            "review_intent",
            "source_posture",
            "evidence_posture",
            "collapse_rule",
            "ordering_hints",
            "agent_packet_hint",
        ] {
            out[k] = review[k].clone();
        }
        views.push(out);
    }
    let mut diagnostics = Vec::new();
    let missing = p["graph_view_ids"]
        .difference(&seen)
        .cloned()
        .collect::<Vec<_>>();
    let extra = seen
        .difference(&p["graph_view_ids"])
        .cloned()
        .collect::<Vec<_>>();
    for (material, prefix) in [
        (missing, "missing graph view contracts: "),
        (
            extra,
            "contracts without atlas projection graph-view nodes: ",
        ),
    ] {
        if !material.is_empty() {
            diagnostics.push(json!({"level":"error","path":VIEW_CONTRACT,"message":format!("{prefix}{}",material.join(", "))}));
        }
    }
    let out = json!({"schema_version":"tos_philosophy_graph_views_v1","schema_ref":VIEWS_SCHEMA,"owner_repo":"Tree-of-Sophia","surface_kind":"derived_philosophy_graph_view_catalog","source_view_contract_ref":VIEW_CONTRACT,"lens_review_contract_ref":LENS_CONTRACT,"source_view_root":VIEW_ROOT,"atlas_projection_ref":ATLAS_REF,"runtime_projection_boundary":{"runtime_owner":"abyss-stack","runtime_scope":["read graph-view filters as ToS-owned display contracts","map view ids to API, MCP, UI, layout, and cache behavior downstream","render and switch graph lenses without writing runtime state back into ToS"],"tos_authority_scope":["view cards and view-contracts.json own ToS graph lens meaning","atlas projection remains the current graph input, not canon","future branch filters guide growth without proving future nodes already exist"]},"validation_refs":["scripts/build_philosophy_graph_views.py","scripts/validate_philosophy_graph_views.py","tests/test_philosophy_graph_views.py"],"counts":{"views":views.len(),"graph_layers":layers.len(),"atlas_projection_graph_views":p["graph_view_ids"].len(),"lens_review_contracts":reviews.len(),"diagnostics":diagnostics.len()},"graph_layers":layers,"default_lens_review_requirements":requirements,"views":views,"diagnostics":diagnostics});
    bytes(&out, l.max_output_bytes)?;
    Ok(out)
}
