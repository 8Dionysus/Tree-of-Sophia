//! Maintained MCP compositions over the same original graph and work meter.
use super::*;
use crate::knowledge_lens_spec::{py_string, truthy};
fn unique(rows: &[&JsonValue], key: &str, w: &mut Work<'_>) -> Result<JsonValue, SearchV2Error> {
    let mut result = BTreeSet::new();
    for row in rows {
        w.step()?;
        if let Some(value) = get(row, key).as_str().filter(|v| !v.is_empty()) {
            result.insert(value.to_owned());
        }
    }
    Ok(texts(result))
}
pub(super) fn contracts<'a>(
    g: &Graph<'a>,
    nodes: &'a [JsonValue],
    edges: &'a [JsonValue],
    w: &mut Work<'_>,
) -> Result<JsonValue, SearchV2Error> {
    let mut views = vec![];
    for view in objs(get(g.header, "views")) {
        w.step()?;
        // Charge the source cohorts before selection scans or cluster sorting.
        for _ in nodes
            .iter()
            .chain(edges.iter())
            .chain(arr(get(g.header, "clusters")).iter())
            .chain(arr(get(view, "graph_layers")).iter())
        {
            w.step()?;
        }
        let (n, e) = g.view_rows(view, nodes, edges);
        let view_id = if truthy(get(view, "view_id")) {
            py_string(get(view, "view_id"))
        } else {
            String::new()
        };
        let mut clusters = g.clusters(Some(&view_id), None);
        clusters.truncate(1_000_000);
        let layers = arr(get(view, "graph_layers"))
            .iter()
            .filter(|v| truthy(v))
            .map(|v| py_string(v))
            .collect::<Vec<_>>();
        views.push(object(vec![
            ("schema", text("tos_philosophy_mcp_view_contract_v1")),
            ("view_id", get(view, "view_id").clone()),
            ("route_card", get(view, "route_card").clone()),
            ("layout_hint", get(view, "layout_hint").clone()),
            ("graph_layers", texts(layers)),
            ("node_kinds", unique(&n, "node_type", w)?),
            ("edge_predicates", unique(&e, "predicate_id", w)?),
            ("cluster_kinds", unique(&clusters, "cluster_kind", w)?),
            ("node_count", number(n.len())),
            ("edge_count", number(e.len())),
            ("cluster_count", number(clusters.len())),
            (
                "source_view_contract_ref",
                get(get(g.header, "source_refs"), "source_view_contract_ref").clone(),
            ),
        ]));
    }
    let mut refs = vec![];
    if let Some(fields) = get(g.header, "source_refs").as_object() {
        for (key, value) in fields {
            w.step()?;
            if value.as_str().is_some_and(|v| !v.is_empty()) {
                refs.push((
                    key.as_str().ok_or_else(|| {
                        failure(
                            SearchV2ErrorCode::CorruptSelectedCarrier,
                            "non-Unicode philosophy source reference key",
                        )
                    })?,
                    value.clone(),
                ));
            }
        }
    }
    Ok(object(vec![
        ("schema", text("tos_philosophy_mcp_contracts_v1")),
        ("source_contract_refs", object(refs)),
        (
            "runtime_contract",
            object(vec![
                ("runtime_owner", text("Tree-of-Sophia")),
                ("source_owner", text("Tree-of-Sophia")),
                (
                    "packet_shape",
                    text("bounded MCP resources and tools over ToS derived exports"),
                ),
                (
                    "limits",
                    texts(
                        [
                            "no writeback",
                            "no canon promotion",
                            "MCP packets are access aids, not source authority",
                        ]
                        .map(str::to_owned),
                    ),
                ),
            ]),
        ),
        ("views", values(views)),
        ("node_kinds", unique(&g.nodes, "node_type", w)?),
        ("edge_predicates", unique(&g.edges, "predicate_id", w)?),
        (
            "graph_layers",
            unique(&objs(get(g.header, "graph_layers")), "layer_id", w)?,
        ),
        (
            "cluster_kinds",
            unique(&objs(get(g.header, "clusters")), "cluster_kind", w)?,
        ),
        (
            "runtime_projection_boundary",
            metadata(g.header, "runtime_projection_boundary"),
        ),
        (
            "authority_note",
            text("Tree-of-Sophia owns graph meaning; MCP exposes the access-plane contract only."),
        ),
    ]))
}
pub(super) fn packet<'a>(
    g: &Graph<'a>,
    nodes: &'a [JsonValue],
    edges: &'a [JsonValue],
    query: &str,
    view: Option<&str>,
    limit: usize,
    w: &mut Work<'_>,
) -> Result<JsonValue, SearchV2Error> {
    w.step()?;
    let search = if query.is_empty() {
        object(vec![
            ("result_count", number(0)),
            ("results", values(vec![])),
        ])
    } else {
        public::search(g, nodes, edges, query, limit, w)?
    };
    let compact = if let Some(id) = view.filter(|v| !v.is_empty()) {
        let v = compute_on_graph(
            g,
            nodes,
            edges,
            &PhilosophyReadRequest::View {
                view_id: id.to_owned(),
                limit,
            },
            w,
        )?;
        object(
            [
                "view",
                "nodes",
                "edges",
                "clusters",
                "review_packet",
                "source_refs",
            ]
            .into_iter()
            .map(|k| (k, get(&v, k).clone()))
            .collect(),
        )
    } else {
        JsonValue::Null
    };
    Ok(object(vec![
        ("schema", text("tos_philosophy_mcp_packet_v1")),
        ("query", text(query)),
        ("view_id", view.map(text).unwrap_or(JsonValue::Null)),
        ("result_count", get(&search, "result_count").clone()),
        ("results", get(&search, "results").clone()),
        ("view", compact),
        ("counts", metadata(g.header, "counts")),
        (
            "runtime_projection_boundary",
            metadata(g.header, "runtime_projection_boundary"),
        ),
        (
            "authority_note",
            text("Packets are access aids; ToS owns meaning and Neo4j/UI/MCP remain projections."),
        ),
    ]))
}
pub(super) fn lens<'a>(
    g: &Graph<'a>,
    nodes: &'a [JsonValue],
    edges: &'a [JsonValue],
    view: &str,
    limit: usize,
    w: &mut Work<'_>,
) -> Result<JsonValue, SearchV2Error> {
    let packet = packet(g, nodes, edges, "", Some(view), limit, w)?;
    w.step()?;
    Ok(object(vec![
        ("schema", text("tos_philosophy_mcp_lens_packet_v1")),
        ("view_id", text(view)),
        ("packet", packet),
        ("review_packet", g.review(view)?.clone()),
        (
            "authority_note",
            text(
                "Lens packets are compact review slices; ToS source_ref surfaces remain authoritative.",
            ),
        ),
    ]))
}

/// The maintained context tool returns its complete public packet. Evidence
/// Lens keeps using the internal context, so its existing wrapper is unchanged.
pub(super) fn epistemic<'a>(
    g: &Graph<'a>,
    nodes: &'a [JsonValue],
    edges: &'a [JsonValue],
    request: &EvidenceRequest,
    w: &mut Work<'_>,
) -> Result<JsonValue, SearchV2Error> {
    let context = evidence::phi_context(g, nodes, edges, request, w)?;
    w.step()?;
    let counts = object(vec![
        (
            "challenge_relations",
            number(arr(get(&context, "challenge_relations")).len()),
        ),
        (
            "available_challenge_relations",
            get(get(&context, "coverage"), "available_challenge_relations").clone(),
        ),
        (
            "context_relations",
            number(arr(get(&context, "context_relations")).len()),
        ),
        (
            "neighbor_nodes",
            number(arr(get(&context, "neighbor_nodes")).len()),
        ),
        (
            "source_refs",
            number(arr(get(&context, "source_refs")).len()),
        ),
    ]);
    let mut fields = context
        .as_object()
        .ok_or_else(|| {
            failure(
                SearchV2ErrorCode::CorruptSelectedCarrier,
                "invalid philosophy epistemic context",
            )
        })?
        .to_vec();
    let extra = object(vec![
        ("schema", text("tos_philosophy_epistemic_packet_v1")),
        ("item_id", text(&request.item_id)),
        (
            "view_id",
            request
                .view_id
                .as_deref()
                .map(text)
                .unwrap_or(JsonValue::Null),
        ),
        (
            "authority_boundary",
            object(vec![
                ("is_source", JsonValue::Bool(false)),
                ("is_canon", JsonValue::Bool(false)),
                ("is_semantic_truth", JsonValue::Bool(false)),
                ("is_rights_clearance", JsonValue::Bool(false)),
            ]),
        ),
        ("counts", counts),
        (
            "challenge_predicates",
            texts(BTreeSet::from([
                "contested_by".to_owned(),
                "uncertain_relation".to_owned(),
                "polemicizes_with".to_owned(),
            ])),
        ),
        (
            "runtime_projection_boundary",
            metadata(g.header, "runtime_projection_boundary"),
        ),
        (
            "authority_note",
            text(
                "This packet exposes projected challenge signals and source-return routes. A contested_by, uncertain_relation, or polemicizes_with candidate is not adjudicated counterevidence; ToS source, claim, review, rights, and canon owners remain authoritative.",
            ),
        ),
    ]);
    fields.extend(extra.as_object().unwrap().iter().cloned());
    Ok(JsonValue::Object(fields))
}
