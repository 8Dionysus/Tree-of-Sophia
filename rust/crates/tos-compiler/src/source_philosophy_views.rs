//! Complete source-owned view-card/lens catalog mechanics over authored atlas rows.
use crate::source_philosophy_support::{
    array, bytes_with_check, object_with_profile_and_check, required,
};
use crate::{Error, Result};
use regex::Regex;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
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
#[derive(Clone, Copy)]
struct ViewRun<'a> {
    deadline: Instant,
    cancelled: &'a AtomicBool,
}

impl ViewRun<'_> {
    fn check(self) -> Result<()> {
        crate::source_philosophy_support::check_run(self.deadline, self.cancelled)
    }
}

fn poll(run: Option<ViewRun<'_>>) -> Result<()> {
    if let Some(run) = run {
        run.check()?;
    }
    Ok(())
}

fn lines<'a>(text: &'a str, run: Option<ViewRun<'_>>) -> Result<Vec<&'a str>> {
    let mut lines = Vec::new();
    for line in text.split([
        '\n', '\u{000B}', '\u{000C}', '\u{001c}', '\u{001d}', '\u{001e}', '\u{0085}', '\u{2028}',
        '\u{2029}',
    ]) {
        poll(run)?;
        lines.push(line);
    }
    Ok(lines)
}
fn section<'a>(text: &'a str, heading: &str, run: Option<ViewRun<'_>>) -> Result<Vec<&'a str>> {
    let text = lines(text, run)?;
    let heading = format!("## {heading}");
    let mut start = None;
    for (index, line) in text.iter().enumerate() {
        poll(run)?;
        if line.trim_matches(crate::source_philosophy_support::source_space) == heading {
            start = Some(index + 1);
            break;
        }
    }
    let start = start.ok_or(Error::Invalid("philosophy view missing section"))?;
    let mut section = Vec::new();
    for line in text.iter().skip(start) {
        poll(run)?;
        if line.starts_with("## ") {
            break;
        }
        section.push(*line);
    }
    Ok(section)
}
fn paragraph(lines: &[&str], run: Option<ViewRun<'_>>) -> Result<String> {
    let mut parts = Vec::new();
    for line in lines {
        poll(run)?;
        let line = line.trim_matches(crate::source_philosophy_support::source_space);
        if !line.is_empty() {
            parts.push(line);
        }
    }
    Ok(parts.join(" "))
}
fn bullets(lines: &[&str], run: Option<ViewRun<'_>>) -> Result<Vec<String>> {
    let mut out = Vec::new();
    let mut current = Vec::new();
    for line in lines {
        poll(run)?;
        let s = line.trim_matches(crate::source_philosophy_support::source_space);
        if s.is_empty() {
            continue;
        }
        if let Some(s) = s.strip_prefix("- ") {
            if !current.is_empty() {
                out.push(current.join(" "));
            }
            current = vec![s.trim_matches(crate::source_philosophy_support::source_space)];
        } else if !current.is_empty() {
            current.push(s);
        }
    }
    if !current.is_empty() {
        out.push(current.join(" "));
    }
    Ok(out)
}
fn markdown(raw: Vec<u8>, run: Option<ViewRun<'_>>) -> Result<String> {
    let text =
        std::str::from_utf8(&raw).map_err(|_| Error::Source("philosophy view UTF-8".into()))?;
    let bytes = text.as_bytes();
    let mut normalized = String::with_capacity(text.len());
    let mut copied_through = 0;
    let mut index = 0;
    while index < bytes.len() {
        if index % (64 * 1024) == 0 {
            poll(run)?;
        }
        if bytes[index] == b'\r' {
            normalized.push_str(&text[copied_through..index]);
            normalized.push('\n');
            index += 1;
            if bytes.get(index) == Some(&b'\n') {
                index += 1;
            }
            copied_through = index;
        } else {
            index += 1;
        }
    }
    normalized.push_str(&text[copied_through..]);
    poll(run)?;
    Ok(normalized)
}
pub(crate) fn parse_card(raw: Vec<u8>) -> Result<Value> {
    parse_card_with_run(raw, None)
}
fn parse_card_with_run(raw: Vec<u8>, run: Option<ViewRun<'_>>) -> Result<Value> {
    poll(run)?;
    let text = markdown(raw, run)?;
    poll(run)?;
    let re =
        Regex::new(r"(?m)^#[\s\x{1c}-\x{1f}]+(.+)$").map_err(|e| Error::Source(e.to_string()))?;
    let title = re
        .captures(&text)
        .and_then(|c| c.get(1))
        .map(|m| {
            m.as_str()
                .trim_matches(crate::source_philosophy_support::source_space)
        })
        .ok_or(Error::Invalid("philosophy view H1"))?;
    poll(run)?;
    let future = bullets(&section(&text, "Future Inputs", run)?, run)?;
    if future.is_empty() {
        return Err(Error::Invalid("philosophy view future inputs"));
    }
    let lens = paragraph(&section(&text, "Lens", run)?, run)?;
    let boundary = paragraph(&section(&text, "Boundary", run)?, run)?;
    poll(run)?;
    Ok(json!({"title":title,"lens":lens,"future_inputs":future,"boundary":boundary}))
}
fn checked_strings(v: &Value, run: Option<ViewRun<'_>>) -> Result<Vec<String>> {
    let values = v
        .as_array()
        .ok_or(Error::Invalid("philosophy string array"))?;
    let mut strings = Vec::with_capacity(values.len());
    for value in values {
        poll(run)?;
        strings.push(
            value
                .as_str()
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .ok_or(Error::Invalid("philosophy string array item"))?,
        );
    }
    Ok(strings)
}
fn subset(v: &Value, allowed: &BTreeSet<String>, run: Option<ViewRun<'_>>) -> Result<()> {
    for value in checked_strings(v, run)? {
        poll(run)?;
        if !allowed.contains(&value) {
            return Err(Error::Invalid("philosophy view filter/route subset"));
        }
    }
    Ok(())
}
fn projection_sets(
    nodes: &[Value],
    edges: &[Value],
    run: Option<ViewRun<'_>>,
) -> Result<BTreeMap<String, BTreeSet<String>>> {
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
        poll(run)?;
        if let Some(t) = n["node_type"].as_str() {
            p.get_mut("node_types").expect("set").insert(t.into());
            match t {
                "master-table-row" => {
                    if let Some(o) = n["properties"].as_object() {
                        for key in o.keys() {
                            poll(run)?;
                            p.get_mut("row_fields").expect("set").insert(key.clone());
                        }
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
        poll(run)?;
        if let Some(t) = e["predicate_id"].as_str() {
            p.get_mut("predicates").expect("set").insert(t.into());
        }
    }
    Ok(p)
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
    build_views_with_input_profile(
        read,
        nodes,
        edges,
        l,
        crate::PhilosophySourceReadProfile::PublishedStrict,
    )
}
/// Same view algorithm with the selected standalone source decoding profile.
pub fn build_views_with_input_profile<F>(
    read: &mut F,
    nodes: &[Value],
    edges: &[Value],
    l: ViewLimits,
    input_profile: crate::PhilosophySourceReadProfile,
) -> Result<Value>
where
    F: FnMut(&str) -> Result<Vec<u8>>,
{
    build_views_impl(read, nodes, edges, l, input_profile, None)
}

/// Same maintained view algorithm with the caller's shared cancellation and
/// deadline. The standalone wrapper keeps its strict-signature APIs above.
pub fn build_views_with_input_profile_guarded<F>(
    read: &mut F,
    nodes: &[Value],
    edges: &[Value],
    l: ViewLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    input_profile: crate::PhilosophySourceReadProfile,
) -> Result<Value>
where
    F: FnMut(&str) -> Result<Vec<u8>>,
{
    build_views_impl(
        read,
        nodes,
        edges,
        l,
        input_profile,
        Some(ViewRun {
            deadline,
            cancelled,
        }),
    )
}

fn build_views_impl<F>(
    read: &mut F,
    nodes: &[Value],
    edges: &[Value],
    l: ViewLimits,
    input_profile: crate::PhilosophySourceReadProfile,
    run: Option<ViewRun<'_>>,
) -> Result<Value>
where
    F: FnMut(&str) -> Result<Vec<u8>>,
{
    poll(run)?;
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
    poll(run)?;
    let source = {
        let mut check = || poll(run);
        object_with_profile_and_check(
            &read(VIEW_CONTRACT)?,
            l.max_source_bytes,
            input_profile,
            &mut check,
        )?
    };
    poll(run)?;
    let lens = {
        let mut check = || poll(run);
        object_with_profile_and_check(
            &read(LENS_CONTRACT)?,
            l.max_source_bytes,
            input_profile,
            &mut check,
        )?
    };
    poll(run)?;
    let manifest = {
        let mut check = || poll(run);
        object_with_profile_and_check(
            &read("ToS/philosophy/philosophy.manifest.json")?,
            l.max_source_bytes,
            input_profile,
            &mut check,
        )?
    };
    poll(run)?;
    for (k, v) in [
        ("schema_version", "tos_philosophy_graph_view_contracts_v1"),
        ("atlas_projection_ref", ATLAS_REF),
        ("downstream_consumer", "abyss-stack"),
    ] {
        poll(run)?;
        if required(&source, k)? != v {
            return Err(Error::Invalid("philosophy view source contract"));
        }
    }
    for (k, v) in [
        ("schema_version", "tos_philosophy_lens_review_contracts_v1"),
        ("view_contract_ref", VIEW_CONTRACT),
        ("downstream_consumer", "abyss-stack"),
    ] {
        poll(run)?;
        if required(&lens, k)? != v {
            return Err(Error::Invalid("philosophy lens source contract"));
        }
    }
    let requirements = &lens["default_requirements"];
    if !requirements.is_object() || requirements["ui_mcp_payload_mode"] != "cluster-first" {
        return Err(Error::Invalid("philosophy lens requirements"));
    }
    for k in ["minimum_source_ref_expectations", "diagnostics_expected"] {
        checked_strings(&requirements[k], run)?;
    }
    poll(run)?;
    let raw = read(LAYERS_SOURCE)?;
    if raw.len() > l.max_source_bytes {
        return Err(Error::Budget("philosophy graph layers bytes"));
    }
    let raw = markdown(raw, run)?;
    let re = Regex::new(r"^\|\s*`([^`]+)`\s*\|\s*(.*?)\s*\|\s*$")
        .map_err(|e| Error::Source(e.to_string()))?;
    poll(run)?;
    let mut layers = Vec::new();
    for line in lines(&raw, run)? {
        poll(run)?;
        if let Some(captures) = re.captures(line) {
            layers.push(json!({"layer_id":&captures[1],"use":captures[2].trim_matches(crate::source_philosophy_support::source_space),"source_ref":LAYERS_SOURCE}));
        }
    }
    if layers.is_empty() || layers.len() > l.max_material_items {
        return Err(Error::Budget("philosophy graph layers"));
    }
    let mut layer_ids = BTreeSet::new();
    for layer in &layers {
        poll(run)?;
        layer_ids.insert(required(layer, "layer_id")?.to_owned());
    }
    let mut routes = BTreeSet::new();
    for route in checked_strings(&manifest["graph_view_routes"], run)? {
        poll(run)?;
        routes.insert(route);
    }
    let p = projection_sets(nodes, edges, run)?;
    let raw_views = array(&source, "views")?;
    if raw_views.len() > l.max_views {
        return Err(Error::Budget("philosophy source views"));
    }
    let mut raw_ids = BTreeSet::new();
    for view in raw_views {
        poll(run)?;
        raw_ids.insert(required(view, "view_id")?.to_owned());
    }
    let mut reviews = BTreeMap::new();
    let raw_reviews = array(&lens, "views")?;
    if raw_reviews.len() > l.max_views {
        return Err(Error::Budget("philosophy lens reviews"));
    }
    for r in raw_reviews {
        poll(run)?;
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
            checked_strings(&r["collapse_rule"][k], run)?;
        }
        checked_strings(&r["ordering_hints"], run)?;
        if reviews.insert(id.to_owned(), r).is_some() {
            return Err(Error::Invalid("philosophy duplicate lens review"));
        }
    }
    poll(run)?;
    let mut review_ids = BTreeSet::new();
    for id in reviews.keys() {
        poll(run)?;
        review_ids.insert(id.clone());
    }
    if review_ids != raw_ids {
        return Err(Error::Invalid("philosophy exact lens coverage"));
    }
    let mut sorted = raw_views.iter().collect::<Vec<_>>();
    poll(run)?;
    sorted.sort_by_key(|v| v["order"].as_i64().unwrap_or(0));
    poll(run)?;
    let mut views = Vec::new();
    let mut seen = BTreeSet::new();
    for view in sorted {
        poll(run)?;
        let id = required(view, "view_id")?;
        let route = required(view, "route_card")?;
        if !seen.insert(id.to_owned())
            || !routes.contains(route)
            || route != format!("{VIEW_ROOT}/{id}.graph.md")
            || view["order"].as_i64().is_none()
        {
            return Err(Error::Invalid("philosophy view identity/order/route"));
        }
        subset(&view["graph_layers"], &layer_ids, run)?;
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
            subset(&view["current_projection_filters"][k], &p[k], run)?;
        }
        for k in ["node_kinds", "relation_kinds"] {
            checked_strings(&view["future_branch_filters"][k], run)?;
        }
        checked_strings(&view["group_by"], run)?;
        let mut allowed = BTreeSet::from(["relation_kind".into(), "node_type_key".into()]);
        for row_field in &p["row_fields"] {
            poll(run)?;
            allowed.insert(row_field.clone());
        }
        subset(&view["sort_fields"], &allowed, run)?;
        required(view, "layout_hint")?;
        poll(run)?;
        let raw = read(route)?;
        if raw.len() > l.max_source_bytes {
            return Err(Error::Budget("philosophy view card bytes"));
        }
        let card = parse_card_with_run(raw, run)?;
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
            poll(run)?;
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
            poll(run)?;
            out[k] = review[k].clone();
        }
        poll(run)?;
        views.push(out);
    }
    let mut diagnostics = Vec::new();
    let mut missing = Vec::new();
    for id in p["graph_view_ids"].difference(&seen) {
        poll(run)?;
        missing.push(id.clone());
    }
    let mut extra = Vec::new();
    for id in seen.difference(&p["graph_view_ids"]) {
        poll(run)?;
        extra.push(id.clone());
    }
    for (material, prefix) in [
        (missing, "missing graph view contracts: "),
        (
            extra,
            "contracts without atlas projection graph-view nodes: ",
        ),
    ] {
        poll(run)?;
        if !material.is_empty() {
            diagnostics.push(json!({"level":"error","path":VIEW_CONTRACT,"message":format!("{prefix}{}",material.join(", "))}));
        }
    }
    let out = json!({"schema_version":"tos_philosophy_graph_views_v1","schema_ref":VIEWS_SCHEMA,"owner_repo":"Tree-of-Sophia","surface_kind":"derived_philosophy_graph_view_catalog","source_view_contract_ref":VIEW_CONTRACT,"lens_review_contract_ref":LENS_CONTRACT,"source_view_root":VIEW_ROOT,"atlas_projection_ref":ATLAS_REF,"runtime_projection_boundary":{"runtime_owner":"abyss-stack","runtime_scope":["read graph-view filters as ToS-owned display contracts","map view ids to API, MCP, UI, layout, and cache behavior downstream","render and switch graph lenses without writing runtime state back into ToS"],"tos_authority_scope":["view cards and view-contracts.json own ToS graph lens meaning","atlas projection remains the current graph input, not canon","future branch filters guide growth without proving future nodes already exist"]},"validation_refs":["rust/crates/tos-compiler/src/source_philosophy_views.rs","rust/crates/tos-ops-mechanics-plan/src/philosophy_products.rs","tests/conformance/rust/compiler_source_cases.rs","docs/validation/validation_lanes.json"],"counts":{"views":views.len(),"graph_layers":layers.len(),"atlas_projection_graph_views":p["graph_view_ids"].len(),"lens_review_contracts":reviews.len(),"diagnostics":diagnostics.len()},"graph_layers":layers,"default_lens_review_requirements":requirements,"views":views,"diagnostics":diagnostics});
    poll(run)?;
    let mut check = || poll(run);
    bytes_with_check(&out, l.max_output_bytes, &mut check)?;
    poll(run)?;
    Ok(out)
}
