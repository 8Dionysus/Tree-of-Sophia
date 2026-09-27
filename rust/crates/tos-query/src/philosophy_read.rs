//! Maintained philosophy read rules over exact original logical projection inputs.
//! Selected disclosure requires the held-owner adapter. Source diagnostics use
//! the separate bounded View-only entry and convey no selected runtime grant.
use crate::knowledge_inspect::{Reader, execute_selected_carrier_packet};
use crate::search_v2::{SearchV2Error, SearchV2ErrorCode};
use crate::source_read_projection::{object, text};
use crate::{
    AbortProbe, AbortReason, BoundCmpKnowledge, DisclosableInspect, InspectBudget,
    InspectCurrentAuthority,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use tos_compiler::{
    PhilosophyOriginalCollection, PhilosophyOriginalReceipt, VerifiedKnowledgeModel,
};
use tos_foundation::{
    CanonicalProfile, FoundationErrorCode, JsonMode, JsonNumber, JsonNumberKind, JsonString,
    JsonValue, canonical_bytes_v1, parse_json,
};

pub const PHILOSOPHY_INTENDED_USE: &str = "read_only_public_philosophy_projection_v1";
pub const PHILOSOPHY_CARRIER_LAYER: &str = crate::knowledge_packet::INDEXED_SEARCH_CARRIER_LAYER;

/// Whole maintained compatibility reads. Existing inspect bounds govern the
/// retained originals, SQL work and final packet; this is not a scale API.
#[derive(Clone, Copy, Debug)]
pub struct PhilosophyReadBudget {
    pub inspect: InspectBudget,
    pub max_work_steps: u64,
}

/// Materialize one monolithic source view for doctor/verify, using the same
/// maintained View algorithm as selected reads. The caller selects and bounds
/// the source file; this function neither opens files nor establishes custody,
/// publication, current policy or a disclosure lease.
///
/// `inspect.json` bounds the input parse, `max_decoded_bytes` bounds raw input,
/// `max_rows` includes base and all inline-view node/edge entries, and
/// `max_response_bytes` bounds the complete canonical packet. `max_work_steps`
/// counts the existing kernel's logical steps; SQL/open VM fields are unused.
/// Parsing and emission are bounded synchronous phases, with probe checks at
/// their boundaries and during the kernel's existing work steps. Last-member
/// wins matches the diagnostic source reader's json.loads, not source admission.
pub fn compute_source_philosophy_view_diagnostic(
    raw_graph: &[u8],
    view_id: &str,
    budget: PhilosophyReadBudget,
    probe: &dyn AbortProbe,
) -> Result<Vec<u8>, SearchV2Error> {
    if view_id.is_empty() || view_id.len() > budget.inspect.max_field_bytes {
        return Err(invalid());
    }
    let request = PhilosophyReadRequest::View {
        view_id: view_id.to_owned(),
        limit: 1000,
    };
    request.validate(budget.inspect)?;
    if budget.max_work_steps == 0
        || budget.inspect.max_rows == 0
        || budget.inspect.max_response_bytes == 0
    {
        return Err(invalid());
    }
    let exhausted = || {
        failure(
            SearchV2ErrorCode::BudgetExceeded,
            "source philosophy diagnostic budget exceeded",
        )
    };
    let mut interrupt = || match probe.reason() {
        Some(AbortReason::Cancelled) => Err(failure(
            SearchV2ErrorCode::Cancelled,
            "source philosophy diagnostic cancelled",
        )),
        Some(AbortReason::DeadlineExceeded) => Err(failure(
            SearchV2ErrorCode::DeadlineExceeded,
            "source philosophy diagnostic deadline exceeded",
        )),
        None => Ok(()),
    };
    interrupt()?;
    if u64::try_from(raw_graph.len()).map_err(|_| exhausted())? > budget.inspect.max_decoded_bytes {
        return Err(exhausted());
    }
    let document =
        parse_json(raw_graph, JsonMode::RequestLastWins, budget.inspect.json).map_err(|error| {
            if error.code == FoundationErrorCode::BudgetExceeded {
                exhausted()
            } else {
                invalid()
            }
        })?;
    interrupt()?;
    let graph = document.root();
    if graph.as_object().is_none() {
        return Err(invalid());
    }
    if !matches!(
        get(graph, "schema_version").as_str(),
        Some("tos_philosophy_graph_projection_v1" | "tos_philosophy_graph_projection_v2")
    ) {
        return Err(failure(
            SearchV2ErrorCode::UnsupportedProfile,
            "source philosophy diagnostic requires a monolithic graph",
        ));
    }
    let nodes = arr(get(graph, "nodes"));
    let edges = arr(get(graph, "edges"));
    let mut rows = 0u64;
    let mut charge = |count: usize| -> Result<(), SearchV2Error> {
        rows = rows
            .checked_add(u64::try_from(count).map_err(|_| exhausted())?)
            .ok_or_else(exhausted)?;
        if rows > budget.inspect.max_rows {
            return Err(exhausted());
        }
        interrupt()
    };
    charge(nodes.len())?;
    charge(edges.len())?;
    for view in arr(get(graph, "views")) {
        if view.as_object().is_some() {
            charge(arr(get(view, "nodes")).len())?;
            charge(arr(get(view, "edges")).len())?;
        }
    }
    let packet = compute_philosophy_read(
        graph,
        nodes,
        edges,
        &request,
        budget.max_work_steps,
        &mut interrupt,
    )?;
    interrupt()?;
    let mut output_limits = budget.inspect.json;
    output_limits.max_bytes = budget.inspect.max_response_bytes;
    let body = canonical_bytes_v1(
        &packet,
        CanonicalProfile::SourceRecordDigestV1,
        output_limits,
    )
    .map_err(|error| {
        if error.code == FoundationErrorCode::BudgetExceeded {
            exhausted()
        } else {
            invalid()
        }
    })?;
    interrupt()?;
    Ok(body)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhilosophyDirection {
    Outgoing,
    Incoming,
    Either,
}
#[derive(Clone, Debug)]
pub enum PhilosophyReadRequest {
    Node {
        node_id: String,
    },
    Edge {
        edge_id: String,
    },
    Neighborhood {
        node_id: String,
        depth: usize,
        limit: usize,
        layers: Vec<String>,
        predicates: Vec<String>,
    },
    Path {
        from_id: String,
        to_id: String,
        layers: Vec<String>,
        predicates: Vec<String>,
        max_depth: usize,
        direction: PhilosophyDirection,
        view_id: Option<String>,
        excluded_edge_ids: Vec<String>,
        alternative_limit: usize,
    },
    View {
        view_id: String,
        limit: usize,
    },
    Views,
    Layers,
    Clusters {
        view_id: Option<String>,
        cluster_kind: Option<String>,
        limit: usize,
    },
    Review {
        view_id: String,
    },
    Snapshot,
    Unresolved {
        view_id: Option<String>,
    },
}
impl PhilosophyReadRequest {
    /// Canonical operation where maintained, otherwise the exact existing
    /// maintained MCP selector. The intended use binds the philosophy mode.
    pub fn operation_id(&self) -> &'static str {
        match self {
            Self::Node { .. } => "tos.node.inspect",
            Self::Edge { .. } => "tos_philosophy_graph_edge",
            Self::Neighborhood { .. } => "tos.neighborhood",
            Self::Path { .. } => "tos.path.find",
            Self::View { .. } => "tos.view.open",
            Self::Views => "tos_philosophy_graph_views",
            Self::Layers => "tos_philosophy_graph_layers",
            Self::Clusters { .. } => "tos_philosophy_graph_clusters",
            Self::Review { .. } => "tos_philosophy_graph_review_packet",
            Self::Snapshot => "tos.snapshot",
            Self::Unresolved { .. } => "tos_philosophy_graph_unresolved",
        }
    }
    fn validate(&self, budget: InspectBudget) -> Result<(), SearchV2Error> {
        let id = |s: &str| !s.is_empty() && s.len() <= budget.max_field_bytes;
        let optional =
            |s: &Option<String>| s.as_ref().is_none_or(|s| s.len() <= budget.max_field_bytes);
        let filters = |v: &[String]| {
            v.len() <= budget.max_matches && v.iter().all(|s| s.len() <= budget.max_field_bytes)
        };
        let valid = match self {
            Self::Node { node_id } => id(node_id),
            Self::Edge { edge_id } => id(edge_id),
            Self::Neighborhood {
                node_id,
                depth,
                limit,
                layers,
                predicates,
            } => {
                id(node_id)
                    && (1..=3).contains(depth)
                    && (1..=300).contains(limit)
                    && filters(layers)
                    && filters(predicates)
            }
            Self::Path {
                from_id,
                to_id,
                layers,
                predicates,
                max_depth,
                view_id,
                excluded_edge_ids,
                alternative_limit,
                ..
            } => {
                id(from_id)
                    && id(to_id)
                    && filters(layers)
                    && filters(predicates)
                    && filters(excluded_edge_ids)
                    && optional(view_id)
                    && (1..=8).contains(max_depth)
                    && (1..=5).contains(alternative_limit)
            }
            Self::View { view_id, limit } => id(view_id) && (1..=1000).contains(limit),
            Self::Clusters {
                view_id,
                cluster_kind,
                limit,
            } => optional(view_id) && optional(cluster_kind) && (1..=1000).contains(limit),
            Self::Review { view_id } => id(view_id),
            Self::Unresolved { view_id } => optional(view_id),
            Self::Views | Self::Layers | Self::Snapshot => true,
        };
        if valid { Ok(()) } else { Err(invalid()) }
    }
}

fn original_rows<'hold, A: InspectCurrentAuthority<'hold> + ?Sized>(
    read: &mut Reader<'_, '_, A>,
    receipt: &PhilosophyOriginalReceipt,
    collection: PhilosophyOriginalCollection,
    count: u64,
) -> Result<Vec<JsonValue>, SearchV2Error> {
    let mut rows = Vec::new();
    let mut after = None;
    for ordinal in 0..count {
        let (actual, row) = read
            .philosophy_row(receipt, collection, after)?
            .ok_or_else(|| {
                failure(
                    SearchV2ErrorCode::CorruptSelectedCarrier,
                    "selected philosophy original coverage incomplete",
                )
            })?;
        if actual != ordinal || row.as_object().is_none() {
            return Err(failure(
                SearchV2ErrorCode::CorruptSelectedCarrier,
                "selected philosophy original order differs",
            ));
        }
        rows.push(row);
        after = Some(actual);
    }
    Ok(rows)
}

/// Exact selected originals, current projection authorization and one held
/// final packet. This grants no source record, Item payload or text access.
pub fn execute_selected_philosophy<'hold, A: InspectCurrentAuthority<'hold> + ?Sized>(
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    request: &PhilosophyReadRequest,
    budget: PhilosophyReadBudget,
) -> Result<DisclosableInspect<'hold>, SearchV2Error> {
    request.validate(budget.inspect)?;
    if budget.max_work_steps == 0 {
        return Err(failure(
            SearchV2ErrorCode::BudgetExceeded,
            "invalid philosophy work budget",
        ));
    }
    execute_selected_carrier_packet(
        model,
        bound,
        authority,
        request.operation_id(),
        PHILOSOPHY_INTENDED_USE,
        budget.inspect,
        |read| {
            let receipt = bound_original_receipt(read, bound)?;
            let mut header =
                original_rows(read, &receipt, PhilosophyOriginalCollection::Header, 1)?;
            let nodes = original_rows(
                read,
                &receipt,
                PhilosophyOriginalCollection::Nodes,
                receipt.nodes,
            )?;
            let edges = original_rows(
                read,
                &receipt,
                PhilosophyOriginalCollection::Edges,
                receipt.edges,
            )?;
            compute_philosophy_read(
                &header.remove(0),
                &nodes,
                &edges,
                request,
                budget.max_work_steps,
                &mut || read.check_interrupt(),
            )
        },
    )
}
// These are maintained path-packet semantics, not additional storage limits.
const PATH_STATE_LIMIT: usize = 50_000;
const PATH_FRONTIER_LIMIT: usize = 5_000;
fn get<'a>(v: &'a JsonValue, k: &str) -> &'a JsonValue {
    v.object_get(k).unwrap_or(&JsonValue::Null)
}
fn s(v: &JsonValue) -> &str {
    v.as_str().unwrap_or("")
}
fn arr(v: &JsonValue) -> &[JsonValue] {
    v.as_array().unwrap_or(&[])
}
fn objs(v: &JsonValue) -> Vec<&JsonValue> {
    arr(v).iter().filter(|v| v.as_object().is_some()).collect()
}
fn strings(v: &JsonValue) -> BTreeSet<String> {
    arr(v)
        .iter()
        .filter_map(JsonValue::as_str)
        .map(str::to_owned)
        .collect()
}
fn texts(v: impl IntoIterator<Item = String>) -> JsonValue {
    JsonValue::Array(v.into_iter().map(|v| text(&v)).collect())
}
fn number(n: usize) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: n.to_string(),
    })
}
fn values(v: impl IntoIterator<Item = JsonValue>) -> JsonValue {
    JsonValue::Array(v.into_iter().collect())
}
fn copies<'a>(v: impl IntoIterator<Item = &'a JsonValue>) -> JsonValue {
    values(v.into_iter().cloned())
}
fn failure(code: SearchV2ErrorCode, message: &'static str) -> SearchV2Error {
    SearchV2Error { code, message }
}
fn unknown() -> SearchV2Error {
    failure(
        SearchV2ErrorCode::UnknownIdentifier,
        "unknown selected philosophy object",
    )
}
fn invalid() -> SearchV2Error {
    failure(
        SearchV2ErrorCode::InvalidRequest,
        "invalid philosophy request",
    )
}
fn metadata(header: &JsonValue, key: &str) -> JsonValue {
    header
        .object_get(key)
        .cloned()
        .unwrap_or_else(|| object(vec![]))
}
fn replace(value: &JsonValue, remove: &[&str], fields: Vec<(&str, JsonValue)>) -> JsonValue {
    let mut pairs = value
        .as_object()
        .unwrap_or(&[])
        .iter()
        .filter(|(k, _)| {
            !remove.contains(&k.as_str().unwrap_or(""))
                && !fields.iter().any(|(name, _)| Some(*name) == k.as_str())
        })
        .cloned()
        .collect::<Vec<_>>();
    pairs.extend(
        fields
            .into_iter()
            .map(|(k, v)| (JsonString::from_utf8(k), v)),
    );
    JsonValue::Object(pairs)
}
fn source_refs<'a>(items: impl IntoIterator<Item = &'a JsonValue>) -> JsonValue {
    let mut refs = BTreeSet::new();
    for item in items {
        let r = s(get(item, "source_ref"));
        if !r.is_empty() {
            refs.insert(r.to_owned());
        }
        for r in arr(get(item, "source_refs")) {
            if let Some(r) = r.as_str().filter(|r| !r.is_empty()) {
                refs.insert(r.to_owned());
            }
        }
    }
    texts(refs)
}
fn layer_allowed(item: &JsonValue, filter: &BTreeSet<String>) -> bool {
    filter.is_empty() || !strings(get(item, "graph_layers")).is_disjoint(filter)
}
fn predicate_allowed(item: &JsonValue, filter: &BTreeSet<String>) -> bool {
    filter.is_empty()
        || get(item, "predicate_id")
            .as_str()
            .is_some_and(|v| filter.contains(v))
}
struct Work<'a> {
    remaining: u64,
    interrupt: &'a mut dyn FnMut() -> Result<(), SearchV2Error>,
}
impl Work<'_> {
    fn step(&mut self) -> Result<(), SearchV2Error> {
        self.remaining = self.remaining.checked_sub(1).ok_or_else(|| {
            failure(
                SearchV2ErrorCode::BudgetExceeded,
                "philosophy work budget exceeded",
            )
        })?;
        (self.interrupt)()
    }
}
struct Graph<'a> {
    header: &'a JsonValue,
    nodes: Vec<&'a JsonValue>,
    edges: Vec<&'a JsonValue>,
}
impl<'a> Graph<'a> {
    fn new(
        header: &'a JsonValue,
        nodes: &'a [JsonValue],
        edges: &'a [JsonValue],
        w: &mut Work<'_>,
    ) -> Result<Self, SearchV2Error> {
        let mut result = Self {
            header,
            nodes: vec![],
            edges: vec![],
        };
        let mut seen_n = BTreeSet::new();
        let mut seen_e = BTreeSet::new();
        let mut append = |items: &'a [JsonValue],
                          key: &str,
                          seen: &mut BTreeSet<String>,
                          out: &mut Vec<&'a JsonValue>|
         -> Result<(), SearchV2Error> {
            for item in items {
                w.step()?;
                let id = s(get(item, key));
                if item.as_object().is_some() && !id.is_empty() && seen.insert(id.to_owned()) {
                    out.push(item);
                }
            }
            Ok(())
        };
        append(nodes, "node_id", &mut seen_n, &mut result.nodes)?;
        append(edges, "edge_id", &mut seen_e, &mut result.edges)?;
        for view in objs(get(header, "views")) {
            append(
                arr(get(view, "nodes")),
                "node_id",
                &mut seen_n,
                &mut result.nodes,
            )?;
            append(
                arr(get(view, "edges")),
                "edge_id",
                &mut seen_e,
                &mut result.edges,
            )?;
        }
        Ok(result)
    }
    fn view(&self, id: &str) -> Result<&'a JsonValue, SearchV2Error> {
        objs(get(self.header, "views"))
            .into_iter()
            .find(|v| s(get(v, "view_id")) == id)
            .ok_or_else(unknown)
    }
    // Inline rows override references for this view, exactly as maintained owner.
    fn view_rows(
        &self,
        view: &'a JsonValue,
        base_nodes: &'a [JsonValue],
        base_edges: &'a [JsonValue],
    ) -> (Vec<&'a JsonValue>, Vec<&'a JsonValue>) {
        let n = objs(get(view, "nodes"));
        let e = objs(get(view, "edges"));
        if !n.is_empty() || !e.is_empty() {
            return (n, e);
        }
        let ni = strings(get(view, "node_ids"));
        let ei = strings(get(view, "edge_ids"));
        (
            base_nodes
                .iter()
                .filter(|v| v.as_object().is_some() && ni.contains(s(get(v, "node_id"))))
                .collect(),
            base_edges
                .iter()
                .filter(|v| v.as_object().is_some() && ei.contains(s(get(v, "edge_id"))))
                .collect(),
        )
    }
    fn node(&self, id: &str) -> Result<&'a JsonValue, SearchV2Error> {
        self.nodes
            .iter()
            .copied()
            .find(|v| s(get(v, "node_id")) == id)
            .ok_or_else(unknown)
    }
    fn review(&self, id: &str) -> Result<&'a JsonValue, SearchV2Error> {
        objs(get(self.header, "review_packets"))
            .into_iter()
            .find(|v| s(get(v, "view_id")) == id)
            .ok_or_else(unknown)
    }
    fn clusters(&self, view: Option<&str>, kind: Option<&str>) -> Vec<&'a JsonValue> {
        let mut c = objs(get(self.header, "clusters"))
            .into_iter()
            .filter(|v| {
                view.is_none_or(|id| id.is_empty() || strings(get(v, "view_ids")).contains(id))
                    && kind.is_none_or(|kind| kind.is_empty() || s(get(v, "cluster_kind")) == kind)
            })
            .collect::<Vec<_>>();
        c.sort_by_key(|v| (s(get(v, "cluster_kind")), s(get(v, "label"))));
        c
    }
}

fn list(header: &JsonValue, key: &str) -> JsonValue {
    header
        .object_get(key)
        .cloned()
        .unwrap_or_else(|| values([]))
}
fn bounded_graph<'a>(
    nodes: &[&'a JsonValue],
    edges: &[&'a JsonValue],
    limit: usize,
    w: &mut Work<'_>,
) -> Result<(Vec<&'a JsonValue>, Vec<&'a JsonValue>), SearchV2Error> {
    let by_id = nodes
        .iter()
        .map(|n| s(get(n, "node_id")))
        .collect::<BTreeSet<_>>();
    let mut ids = BTreeSet::new();
    let mut selected_edges = vec![];
    for e in edges {
        w.step()?;
        let a = s(get(e, "from_id"));
        let b = s(get(e, "to_id"));
        if !by_id.contains(a) || !by_id.contains(b) {
            continue;
        }
        let additions = BTreeSet::from([a, b]).difference(&ids).count();
        if ids.len() + additions > limit {
            continue;
        }
        ids.insert(a);
        ids.insert(b);
        selected_edges.push(*e);
        if selected_edges.len() >= limit {
            break;
        }
    }
    for n in nodes {
        w.step()?;
        if ids.len() >= limit {
            break;
        }
        let id = s(get(n, "node_id"));
        if !id.is_empty() {
            ids.insert(id);
        }
    }
    Ok((
        nodes
            .iter()
            .copied()
            .filter(|n| ids.contains(s(get(n, "node_id"))))
            .collect(),
        selected_edges,
    ))
}
fn bounded_clusters(
    clusters: &[&JsonValue],
    nodes: &[&JsonValue],
    edges: &[&JsonValue],
    w: &mut Work<'_>,
) -> Result<Vec<JsonValue>, SearchV2Error> {
    let ni = nodes
        .iter()
        .map(|n| s(get(n, "node_id")))
        .collect::<BTreeSet<_>>();
    let ei = edges
        .iter()
        .map(|e| s(get(e, "edge_id")))
        .collect::<BTreeSet<_>>();
    let mut result = vec![];
    for c in clusters {
        w.step()?;
        let n = arr(get(c, "member_node_ids"))
            .iter()
            .filter_map(JsonValue::as_str)
            .collect::<Vec<_>>();
        let e = arr(get(c, "member_edge_ids"))
            .iter()
            .filter_map(JsonValue::as_str)
            .collect::<Vec<_>>();
        let selected_n = n
            .iter()
            .filter(|id| ni.contains(**id))
            .map(|id| (*id).to_owned())
            .collect::<Vec<_>>();
        let selected_e = e
            .iter()
            .filter(|id| ei.contains(**id))
            .map(|id| (*id).to_owned())
            .collect::<Vec<_>>();
        if selected_n.is_empty() && selected_e.is_empty() {
            continue;
        }
        let properties = get(c, "properties");
        let mut fields = vec![];
        if properties.object_get("member_count").is_some() {
            fields.push(("member_count", number(selected_n.len())));
        }
        if properties.object_get("edge_count").is_some() {
            fields.push(("edge_count", number(selected_e.len())));
        }
        let properties = replace(properties, &[], fields);
        result.push(replace(
            c,
            &[],
            vec![
                ("member_node_ids", texts(selected_n)),
                ("member_edge_ids", texts(selected_e)),
                ("available_member_node_count", number(n.len())),
                ("available_member_edge_count", number(e.len())),
                ("properties", properties),
            ],
        ));
    }
    Ok(result)
}
/// Only a selected original adapter may finalize this into a held packet.
pub(crate) fn compute_philosophy_read(
    header: &JsonValue,
    base_nodes: &[JsonValue],
    base_edges: &[JsonValue],
    request: &PhilosophyReadRequest,
    max_steps: u64,
    interrupt: &mut dyn FnMut() -> Result<(), SearchV2Error>,
) -> Result<JsonValue, SearchV2Error> {
    let mut w = Work {
        remaining: max_steps,
        interrupt,
    };
    let graph = Graph::new(header, base_nodes, base_edges, &mut w)?;
    let boundary = || metadata(header, "runtime_projection_boundary");
    match request {
        PhilosophyReadRequest::Node { node_id } => {
            let node = graph.node(node_id)?;
            let edges = graph
                .edges
                .iter()
                .copied()
                .filter(|e| s(get(e, "from_id")) == node_id || s(get(e, "to_id")) == node_id)
                .collect::<Vec<_>>();
            Ok(object(vec![
                ("schema", text("tos_philosophy_mcp_node_v1")),
                ("node_id", text(node_id)),
                ("node", node.clone()),
                ("related_edges", copies(edges.iter().copied())),
                (
                    "source_refs",
                    source_refs(std::iter::once(node).chain(edges)),
                ),
                (
                    "authority_note",
                    text(
                        "Node source_ref stays authoritative in Tree-of-Sophia; MCP exposes an access packet only.",
                    ),
                ),
            ]))
        }
        PhilosophyReadRequest::Edge { edge_id } => {
            let edge = graph
                .edges
                .iter()
                .copied()
                .find(|e| s(get(e, "edge_id")) == edge_id)
                .ok_or_else(unknown)?;
            let ids = BTreeSet::from([s(get(edge, "from_id")), s(get(edge, "to_id"))]);
            let nodes = graph
                .nodes
                .iter()
                .copied()
                .filter(|n| ids.contains(s(get(n, "node_id"))))
                .collect::<Vec<_>>();
            Ok(object(vec![
                ("schema", text("tos_philosophy_mcp_edge_v1")),
                ("edge_id", text(edge_id)),
                ("edge", edge.clone()),
                ("endpoints", copies(nodes.iter().copied())),
                (
                    "source_refs",
                    source_refs(nodes.into_iter().chain(std::iter::once(edge))),
                ),
                (
                    "authority_note",
                    text(
                        "Edge source_ref stays authoritative in Tree-of-Sophia; MCP exposes an access packet only.",
                    ),
                ),
            ]))
        }
        PhilosophyReadRequest::Neighborhood {
            node_id,
            depth,
            limit,
            layers,
            predicates,
        } => {
            if !(1..=3).contains(depth) || !(1..=300).contains(limit) {
                return Err(invalid());
            }
            let node = graph.node(node_id)?;
            let lf = layers.iter().cloned().collect::<BTreeSet<_>>();
            let pf = predicates.iter().cloned().collect::<BTreeSet<_>>();
            let node_order = graph
                .nodes
                .iter()
                .enumerate()
                .map(|(i, n)| (s(get(n, "node_id")), i))
                .collect::<BTreeMap<_, _>>();
            let by_id = graph
                .nodes
                .iter()
                .map(|n| (s(get(n, "node_id")), *n))
                .collect::<BTreeMap<_, _>>();
            let allowed = graph
                .nodes
                .iter()
                .filter(|n| s(get(n, "node_id")) == node_id || layer_allowed(n, &lf))
                .map(|n| s(get(n, "node_id")))
                .collect::<BTreeSet<_>>();
            let edges = graph
                .edges
                .iter()
                .copied()
                .filter(|e| {
                    layer_allowed(e, &lf)
                        && predicate_allowed(e, &pf)
                        && allowed.contains(s(get(e, "from_id")))
                        && allowed.contains(s(get(e, "to_id")))
                })
                .collect::<Vec<_>>();
            let mut selected = BTreeSet::from([node_id.as_str()]);
            let mut discovery = vec![node_id.as_str()];
            let mut frontier = vec![node_id.as_str()];
            let mut retained = vec![];
            let mut selected_edges = BTreeSet::new();
            for _ in 0..*depth {
                let mut candidates = BTreeMap::new();
                for e in &edges {
                    w.step()?;
                    let a = s(get(e, "from_id"));
                    let b = s(get(e, "to_id"));
                    if frontier.contains(&a) && !selected.contains(b) {
                        candidates.entry(b).or_insert(*e);
                    }
                    if frontier.contains(&b) && !selected.contains(a) {
                        candidates.entry(a).or_insert(*e);
                    }
                }
                let mut candidates = candidates.into_iter().collect::<Vec<_>>();
                candidates.sort_by_key(|(id, _)| {
                    (node_order.get(id).copied().unwrap_or(node_order.len()), *id)
                });
                let mut next = vec![];
                for (id, e) in candidates {
                    w.step()?;
                    if discovery.len() - 1 >= *limit {
                        break;
                    }
                    selected.insert(id);
                    discovery.push(id);
                    next.push(id);
                    if selected_edges.insert(s(get(e, "edge_id"))) {
                        retained.push(e);
                    }
                }
                frontier = next;
                if frontier.is_empty() || discovery.len() - 1 >= *limit {
                    break;
                }
            }
            let neighbors = discovery
                .iter()
                .skip(1)
                .filter_map(|id| by_id.get(id).copied())
                .collect::<Vec<_>>();
            for e in edges {
                w.step()?;
                if retained.len() >= *limit {
                    break;
                }
                let id = s(get(e, "edge_id"));
                if !selected_edges.contains(id)
                    && selected.contains(s(get(e, "from_id")))
                    && selected.contains(s(get(e, "to_id")))
                {
                    retained.push(e);
                    selected_edges.insert(id);
                }
            }
            Ok(object(vec![
                ("schema", text("tos_philosophy_mcp_neighborhood_v1")),
                ("node", node.clone()),
                ("neighbors", copies(neighbors.iter().copied())),
                ("edges", copies(retained.iter().copied())),
                ("depth", number(*depth)),
                ("layers", texts(lf)),
                ("predicates", texts(pf)),
                ("limit", number(*limit)),
                (
                    "source_refs",
                    source_refs(std::iter::once(node).chain(neighbors).chain(retained)),
                ),
                ("runtime_projection_boundary", boundary()),
            ]))
        }
        PhilosophyReadRequest::Path {
            from_id,
            to_id,
            layers,
            predicates,
            max_depth,
            direction,
            view_id,
            excluded_edge_ids,
            alternative_limit,
        } => {
            graph.node(from_id)?;
            graph.node(to_id)?;
            if !(1..=8).contains(max_depth) || !(1..=5).contains(alternative_limit) {
                return Err(invalid());
            }
            let lf = layers.iter().cloned().collect::<BTreeSet<_>>();
            let pf = predicates.iter().cloned().collect::<BTreeSet<_>>();
            let excluded = excluded_edge_ids
                .iter()
                .filter(|s| !s.is_empty())
                .cloned()
                .collect::<BTreeSet<_>>();
            let by_id = graph
                .nodes
                .iter()
                .map(|n| (s(get(n, "node_id")), *n))
                .collect::<BTreeMap<_, _>>();
            let (nodes, mut edges) =
                if let Some(id) = view_id.as_deref().filter(|id| !id.is_empty()) {
                    graph.view_rows(graph.view(id)?, base_nodes, base_edges)
                } else {
                    (graph.nodes.clone(), graph.edges.clone())
                };
            let ids = nodes
                .iter()
                .map(|n| s(get(n, "node_id")))
                .collect::<BTreeSet<_>>();
            edges.sort_by_key(|e| {
                (
                    s(get(e, "edge_id")),
                    s(get(e, "from_id")),
                    s(get(e, "to_id")),
                )
            });
            let mut adjacency: BTreeMap<&str, Vec<(&str, &JsonValue, &str)>> = BTreeMap::new();
            for e in edges {
                w.step()?;
                let a = s(get(e, "from_id"));
                let b = s(get(e, "to_id"));
                if !layer_allowed(e, &lf)
                    || !predicate_allowed(e, &pf)
                    || excluded.contains(s(get(e, "edge_id")))
                    || !ids.contains(a)
                    || !ids.contains(b)
                {
                    continue;
                }
                if *direction != PhilosophyDirection::Incoming {
                    adjacency.entry(a).or_default().push((b, e, "forward"));
                }
                if *direction != PhilosophyDirection::Outgoing
                    && (a != b || *direction == PhilosophyDirection::Incoming)
                {
                    adjacency.entry(b).or_default().push((a, e, "reverse"));
                }
            }
            let mut queue = VecDeque::from([(
                from_id.as_str(),
                vec![from_id.as_str()],
                Vec::<&JsonValue>::new(),
                Vec::<JsonValue>::new(),
            )]);
            let mut paths = vec![];
            let mut all_sources = vec![];
            let mut explored = 0;
            let mut enqueued = 1;
            let mut max_frontier = 1;
            let mut truncated = false;
            let mut primary_nodes = vec![];
            let mut primary_edges = vec![];
            while !queue.is_empty() && paths.len() < *alternative_limit {
                w.step()?;
                if explored >= PATH_STATE_LIMIT {
                    truncated = true;
                    break;
                }
                let (current, np, ep, traversal) = queue.pop_front().unwrap();
                explored += 1;
                if current == to_id {
                    let nodes = np.iter().map(|id| by_id[id]).collect::<Vec<_>>();
                    if paths.is_empty() {
                        primary_nodes = nodes.clone();
                        primary_edges = ep.clone();
                    }
                    let path_refs = source_refs(nodes.iter().copied().chain(ep.iter().copied()));
                    all_sources.extend(nodes.iter().copied());
                    all_sources.extend(ep.iter().copied());
                    paths.push(object(vec![
                        ("path_index", number(paths.len())),
                        ("node_ids", texts(np.into_iter().map(str::to_owned))),
                        (
                            "edge_ids",
                            texts(ep.iter().map(|e| s(get(e, "edge_id")).to_owned())),
                        ),
                        ("nodes", copies(nodes)),
                        ("edges", copies(ep)),
                        ("traversal", values(traversal)),
                        ("source_refs", path_refs),
                    ]));
                    continue;
                }
                if ep.len() >= *max_depth {
                    continue;
                }
                for (neighbor, e, orientation) in adjacency.get(current).into_iter().flatten() {
                    w.step()?;
                    if np.contains(neighbor) {
                        continue;
                    }
                    if enqueued >= PATH_STATE_LIMIT || queue.len() >= PATH_FRONTIER_LIMIT {
                        truncated = true;
                        break;
                    }
                    let mut nn = np.clone();
                    nn.push(neighbor);
                    let mut ee = ep.clone();
                    ee.push(e);
                    let mut tt = traversal.clone();
                    tt.push(object(vec![
                        ("edge_id", get(e, "edge_id").clone()),
                        ("from_node_id", text(current)),
                        ("to_node_id", text(neighbor)),
                        ("edge_direction", text(orientation)),
                    ]));
                    queue.push_back((neighbor, nn, ee, tt));
                    enqueued += 1;
                    max_frontier = max_frontier.max(queue.len());
                }
            }
            Ok(object(vec![
                ("schema", text("tos_philosophy_mcp_path_v2")),
                ("from_id", text(from_id)),
                ("to_id", text(to_id)),
                ("found", JsonValue::Bool(!paths.is_empty())),
                ("path_count", number(paths.len())),
                ("paths", values(paths)),
                ("nodes", copies(primary_nodes)),
                ("edges", copies(primary_edges)),
                ("max_depth", number(*max_depth)),
                (
                    "direction",
                    text(match direction {
                        PhilosophyDirection::Outgoing => "outgoing",
                        PhilosophyDirection::Incoming => "incoming",
                        PhilosophyDirection::Either => "either",
                    }),
                ),
                (
                    "view_id",
                    view_id.as_deref().map(text).unwrap_or(JsonValue::Null),
                ),
                ("excluded_edge_ids", texts(excluded)),
                ("alternative_limit", number(*alternative_limit)),
                ("exploration_truncated", JsonValue::Bool(truncated)),
                ("explored_state_count", number(explored)),
                ("enqueued_state_count", number(enqueued)),
                ("frontier_limit", number(PATH_FRONTIER_LIMIT)),
                ("max_frontier_size", number(max_frontier)),
                ("layers", texts(lf)),
                ("predicates", texts(pf)),
                ("source_refs", source_refs(all_sources)),
                ("runtime_projection_boundary", boundary()),
                (
                    "authority_note",
                    text("Tree-of-Sophia owns graph meaning; MCP serves a bounded path packet."),
                ),
            ]))
        }
        PhilosophyReadRequest::View { view_id, limit } => {
            if !(1..=1000).contains(limit) {
                return Err(invalid());
            }
            let view = graph.view(view_id)?;
            let (all_nodes, all_edges) = graph.view_rows(view, base_nodes, base_edges);
            let (nodes, edges) = bounded_graph(&all_nodes, &all_edges, *limit, &mut w)?;
            let clusters =
                bounded_clusters(&graph.clusters(Some(view_id), None), &nodes, &edges, &mut w)?
                    .into_iter()
                    .take(*limit)
                    .collect::<Vec<_>>();
            let bounded_view = replace(
                view,
                &["nodes", "edges"],
                vec![
                    (
                        "node_ids",
                        texts(nodes.iter().map(|n| s(get(n, "node_id")).to_owned())),
                    ),
                    (
                        "edge_ids",
                        texts(edges.iter().map(|e| s(get(e, "edge_id")).to_owned())),
                    ),
                ],
            );
            let review = object(vec![
                ("schema", text("tos_philosophy_mcp_review_packet_v1")),
                ("packet", graph.review(view_id)?.clone()),
                ("runtime_projection_boundary", boundary()),
                (
                    "authority_note",
                    text(
                        "Tree-of-Sophia owns review packet semantics; MCP serves the compact access packet.",
                    ),
                ),
            ]);
            Ok(object(vec![
                ("schema", text("tos_philosophy_mcp_view_v1")),
                ("view", bounded_view),
                ("node_count", number(nodes.len())),
                ("edge_count", number(edges.len())),
                ("available_node_count", number(all_nodes.len())),
                ("available_edge_count", number(all_edges.len())),
                ("limit", number(*limit)),
                ("nodes", copies(nodes)),
                ("edges", copies(edges)),
                ("clusters", values(clusters)),
                ("review_packet", review),
                ("source_refs", list(view, "source_refs")),
                ("runtime_projection_boundary", boundary()),
            ]))
        }
        PhilosophyReadRequest::Views => {
            let mut counts = BTreeMap::<&str, usize>::new();
            for c in objs(get(header, "clusters")) {
                w.step()?;
                for id in arr(get(c, "view_ids")).iter().filter_map(JsonValue::as_str) {
                    *counts.entry(id).or_default() += 1;
                }
            }
            let mut views = vec![];
            for v in objs(get(header, "views")) {
                w.step()?;
                let (n, e) = graph.view_rows(v, base_nodes, base_edges);
                views.push(object(vec![
                    ("view_id", get(v, "view_id").clone()),
                    ("title", get(v, "title").clone()),
                    ("layout_hint", get(v, "layout_hint").clone()),
                    ("graph_layers", list(v, "graph_layers")),
                    ("node_count", number(n.len())),
                    ("edge_count", number(e.len())),
                    (
                        "cluster_count",
                        number(counts.get(s(get(v, "view_id"))).copied().unwrap_or(0)),
                    ),
                    ("review_intent", get(v, "review_intent").clone()),
                    ("collapse_rule", metadata(v, "collapse_rule")),
                    ("source_ref", get(v, "source_ref").clone()),
                    ("route_card", get(v, "route_card").clone()),
                ]));
            }
            Ok(object(vec![
                ("schema", text("tos_philosophy_mcp_views_v1")),
                ("views", values(views)),
                ("counts", metadata(header, "counts")),
                ("graph_layers", list(header, "graph_layers")),
                ("layer_counts", list(header, "layer_counts")),
                ("visibility_model", metadata(header, "visibility_model")),
                ("runtime_projection_boundary", boundary()),
            ]))
        }
        PhilosophyReadRequest::Layers => Ok(object(vec![
            ("schema", text("tos_philosophy_mcp_layers_v1")),
            ("graph_layers", list(header, "graph_layers")),
            ("layer_counts", list(header, "layer_counts")),
            ("visibility_model", metadata(header, "visibility_model")),
            ("runtime_projection_boundary", boundary()),
        ])),
        PhilosophyReadRequest::Clusters {
            view_id,
            cluster_kind,
            limit,
        } => {
            if !(1..=1000).contains(limit) {
                return Err(invalid());
            }
            let clusters = graph
                .clusters(view_id.as_deref(), cluster_kind.as_deref())
                .into_iter()
                .take(*limit)
                .collect::<Vec<_>>();
            Ok(object(vec![
                ("schema", text("tos_philosophy_mcp_clusters_v1")),
                (
                    "view_id",
                    view_id.as_deref().map(text).unwrap_or(JsonValue::Null),
                ),
                (
                    "cluster_kind",
                    cluster_kind.as_deref().map(text).unwrap_or(JsonValue::Null),
                ),
                ("cluster_count", number(clusters.len())),
                ("clusters", copies(clusters.iter().copied())),
                ("counts", metadata(header, "counts")),
                ("source_refs", source_refs(clusters)),
                ("runtime_projection_boundary", boundary()),
            ]))
        }
        PhilosophyReadRequest::Review { view_id } => Ok(object(vec![
            ("schema", text("tos_philosophy_mcp_review_packet_v1")),
            ("packet", graph.review(view_id)?.clone()),
            ("runtime_projection_boundary", boundary()),
            (
                "authority_note",
                text(
                    "Tree-of-Sophia owns review packet semantics; MCP serves the compact access packet.",
                ),
            ),
        ])),
        PhilosophyReadRequest::Snapshot => Ok(object(vec![
            ("schema", text("tos_philosophy_mcp_snapshot_v1")),
            ("snapshot_review", metadata(header, "snapshot_review")),
            ("runtime_projection_boundary", boundary()),
            (
                "authority_note",
                text(
                    "Tree-of-Sophia owns snapshot semantics; MCP serves fingerprints for review and diff routing.",
                ),
            ),
        ])),
        PhilosophyReadRequest::Unresolved { view_id } => {
            let unresolved = if let Some(id) = view_id.as_deref().filter(|id| !id.is_empty()) {
                objs(get(graph.review(id)?, "unresolved_diagnostics"))
            } else {
                objs(get(header, "unresolved_review_surfaces"))
            };
            Ok(object(vec![
                ("schema", text("tos_philosophy_mcp_unresolved_v1")),
                (
                    "view_id",
                    view_id.as_deref().map(text).unwrap_or(JsonValue::Null),
                ),
                ("unresolved_count", number(unresolved.len())),
                ("unresolved", copies(unresolved)),
                ("runtime_projection_boundary", boundary()),
            ]))
        }
    }
}

fn bound_original_receipt<'hold, A: InspectCurrentAuthority<'hold> + ?Sized>(
    read: &mut Reader<'_, '_, A>,
    bound: &BoundCmpKnowledge<'_>,
) -> Result<PhilosophyOriginalReceipt, SearchV2Error> {
    let receipt = read.philosophy_receipt()?;
    let source = bound
        .source_for_adapter("philosophy-node-edge-v1")
        .ok_or_else(|| {
            failure(
                SearchV2ErrorCode::Unavailable,
                "selected philosophy source unavailable",
            )
        })?;
    if receipt.profile != tos_compiler::PHILOSOPHY_ORIGINAL_PROFILE
        || receipt.source_graph != source
        || receipt.descriptor_sha256 != bound.selection().vocabulary.descriptor_sha256.to_hex()
        || receipt.source_cut != bound.selection().source_cut
        || receipt.membership_root != bound.selection().source_membership_root.to_hex()
    {
        return Err(failure(
            SearchV2ErrorCode::CorruptSelectedCarrier,
            "selected philosophy original binding differs",
        ));
    }
    Ok(receipt)
}

/// Metadata projection for site defaults, not the complete maintained Views
/// packet. Only the exact original Header is consulted under the existing
/// philosophy views scope and current held original-component authority.
pub fn execute_selected_philosophy_view_ids<'hold, A: InspectCurrentAuthority<'hold> + ?Sized>(
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    budget: PhilosophyReadBudget,
) -> Result<DisclosableInspect<'hold>, SearchV2Error> {
    if budget.max_work_steps == 0 {
        return Err(invalid());
    }
    execute_selected_carrier_packet(
        model,
        bound,
        authority,
        PhilosophyReadRequest::Views.operation_id(),
        PHILOSOPHY_INTENDED_USE,
        budget.inspect,
        |read| {
            let receipt = bound_original_receipt(read, bound)?;
            let header =
                original_rows(read, &receipt, PhilosophyOriginalCollection::Header, 1)?.remove(0);
            let mut interrupt = || read.check_interrupt();
            let mut work = Work {
                remaining: budget.max_work_steps,
                interrupt: &mut interrupt,
            };
            let mut views = vec![];
            for view in arr(get(&header, "views")) {
                work.step()?;
                if view.as_object().is_none()
                    || !crate::knowledge_lens_spec::truthy(get(view, "view_id"))
                {
                    continue;
                }
                let id = crate::knowledge_lens_spec::py_string(get(view, "view_id"));
                if id.len() > budget.inspect.max_field_bytes {
                    return Err(failure(
                        SearchV2ErrorCode::BudgetExceeded,
                        "philosophy view identity exceeds field budget",
                    ));
                }
                views.push(object(vec![("view_id", text(&id))]));
            }
            Ok(object(vec![("views", values(views))]))
        },
    )
}
