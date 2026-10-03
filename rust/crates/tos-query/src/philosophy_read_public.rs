//! Maintained public projection packets over the selected original graph.
use super::*;
const PROJECTION_PATH: &str = "ToS/derived-exports/philosophy_graph_projection.min.json";
const TABLES: [&str; 5] = [
    "nodes",
    "edges",
    "clusters",
    "cluster-node-memberships",
    "cluster-edge-memberships",
];
fn lower(value: &str, w: &mut Work<'_>) -> Result<String, SearchV2Error> {
    let chars = value.chars().count();
    for _ in 0..chars {
        w.step()?;
    }
    if value.is_empty() {
        return Ok(String::new());
    }
    tos_foundation::python_lower_unicode16_v1(
        value,
        chars,
        chars.saturating_mul(3),
        value.len().saturating_mul(3),
    )
    .map_err(|_| {
        failure(
            SearchV2ErrorCode::BudgetExceeded,
            "philosophy lowercase budget",
        )
    })
}
fn contains(value: &JsonValue, needle: &str, w: &mut Work<'_>) -> Result<bool, SearchV2Error> {
    w.step()?;
    match value {
        JsonValue::String(_) => Ok(lower(s(value), w)?.contains(needle)),
        JsonValue::Array(values) => {
            for value in values {
                if contains(value, needle, w)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        JsonValue::Object(values) => {
            for (_, value) in values {
                if contains(value, needle, w)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        _ => Ok(false),
    }
}
pub(super) fn status(header: &JsonValue) -> JsonValue {
    object(vec![
        ("schema", text("tos_philosophy_mcp_status_v1")),
        ("projection_exists", JsonValue::Bool(true)),
        ("tos_root", text("Tree-of-Sophia")),
        ("projection_path", text(PROJECTION_PATH)),
        ("owner_repo", get(header, "owner_repo").clone()),
        ("surface_kind", get(header, "surface_kind").clone()),
        ("counts", metadata(header, "counts")),
        (
            "views",
            values(
                objs(get(header, "views"))
                    .into_iter()
                    .map(|v| get(v, "view_id").clone()),
            ),
        ),
        (
            "graph_layers",
            values(
                objs(get(header, "graph_layers"))
                    .into_iter()
                    .filter(|v| !s(get(v, "layer_id")).is_empty())
                    .map(|v| get(v, "layer_id").clone()),
            ),
        ),
        ("visibility_model", metadata(header, "visibility_model")),
        ("snapshot_review", metadata(header, "snapshot_review")),
        (
            "runtime_projection_boundary",
            header
                .object_get("runtime_projection_boundary")
                .cloned()
                .unwrap_or_else(|| {
                    object(vec![
                        ("runtime_owner", text("abyss-stack")),
                        (
                            "missing_state",
                            text("ToS philosophy graph projection is not present at this MCP path"),
                        ),
                    ])
                }),
        ),
        (
            "authority_note",
            text(
                "Tree-of-Sophia owns philosophy meaning; this MCP packet is a Tree-of-Sophia standalone access aid.",
            ),
        ),
    ])
}
pub(super) fn search<'a>(
    graph: &Graph<'a>,
    base_nodes: &'a [JsonValue],
    base_edges: &'a [JsonValue],
    query: &str,
    limit: usize,
    w: &mut Work<'_>,
) -> Result<JsonValue, SearchV2Error> {
    let lower = lower(query, w)?;
    let needle = if lower.is_empty() {
        ""
    } else {
        tos_foundation::python_strip_unicode16_v1(&lower, lower.chars().count())
            .map_err(|_| invalid())?
    };
    let collections = [
        ("views", objs(get(graph.header, "views"))),
        ("nodes", graph.nodes.clone()),
        ("edges", graph.edges.clone()),
        ("clusters", objs(get(graph.header, "clusters"))),
        ("review_packets", objs(get(graph.header, "review_packets"))),
        ("graph_layers", objs(get(graph.header, "graph_layers"))),
    ];
    let mut results = Vec::new();
    'collections: for (name, rows) in collections {
        for row in rows {
            w.step()?;
            if !needle.is_empty() && !contains(row, needle, w)? {
                continue;
            }
            let item = match name {
                "views" => {
                    let (nodes, edges) = graph.view_rows(row, base_nodes, base_edges);
                    replace(
                        row,
                        &["nodes", "edges", "node_ids", "edge_ids"],
                        vec![
                            ("node_count", number(nodes.len())),
                            ("edge_count", number(edges.len())),
                        ],
                    )
                }
                "clusters" => replace(
                    row,
                    &["member_node_ids", "member_edge_ids"],
                    vec![
                        (
                            "member_node_count",
                            number(arr(get(row, "member_node_ids")).len()),
                        ),
                        (
                            "member_edge_count",
                            number(arr(get(row, "member_edge_ids")).len()),
                        ),
                    ],
                ),
                "review_packets" => replace(
                    row,
                    &["unresolved_diagnostics"],
                    vec![(
                        "unresolved_diagnostic_count",
                        number(arr(get(row, "unresolved_diagnostics")).len()),
                    )],
                ),
                _ => row.clone(),
            };
            results.push(object(vec![("collection", text(name)), ("item", item)]));
            if results.len() >= limit {
                break 'collections;
            }
        }
    }
    Ok(object(vec![
        ("schema", text("tos_philosophy_mcp_search_v1")),
        ("query", text(query)),
        ("result_count", number(results.len())),
        ("results", values(results)),
        (
            "authority_note",
            text(
                "Tree-of-Sophia owns philosophy meaning; this MCP search result is an access-plane packet.",
            ),
        ),
    ]))
}
fn scale_rows<'a>(
    graph: &Graph<'a>,
    base_nodes: &'a [JsonValue],
    base_edges: &'a [JsonValue],
    table: &str,
    view_id: Option<&str>,
    layers: &BTreeSet<String>,
    w: &mut Work<'_>,
) -> Result<Vec<JsonValue>, SearchV2Error> {
    let (nodes, edges, clusters) = if let Some(id) = view_id.filter(|id| !id.is_empty()) {
        let view = graph.view(id)?;
        let (nodes, edges) = graph.view_rows(view, base_nodes, base_edges);
        (nodes, edges, graph.clusters(Some(id), None))
    } else {
        (
            graph.nodes.clone(),
            graph.edges.clone(),
            objs(get(graph.header, "clusters")),
        )
    };
    let mut selected_nodes = Vec::new();
    for node in nodes {
        w.step()?;
        if layer_allowed(node, layers) {
            selected_nodes.push(node);
        }
    }
    let nodes = selected_nodes;
    let ids = nodes
        .iter()
        .map(|v| s(get(v, "node_id")))
        .collect::<BTreeSet<_>>();
    let mut selected_edges = Vec::new();
    for edge in edges {
        w.step()?;
        if layer_allowed(edge, layers)
            && ids.contains(s(get(edge, "from_id")))
            && ids.contains(s(get(edge, "to_id")))
        {
            selected_edges.push(edge);
        }
    }
    let edges = selected_edges;
    let edge_ids = edges
        .iter()
        .map(|v| s(get(v, "edge_id")))
        .collect::<BTreeSet<_>>();
    let mut selected_clusters = Vec::new();
    for cluster in clusters {
        w.step()?;
        if layer_allowed(cluster, layers) {
            selected_clusters.push(cluster);
        }
    }
    let clusters = selected_clusters;
    let clusters = bounded_clusters(&clusters, &nodes, &edges, w)?;
    match table {
        "nodes" => Ok(nodes.into_iter().cloned().collect()),
        "edges" => Ok(edges.into_iter().cloned().collect()),
        "clusters" => Ok(clusters),
        "cluster-node-memberships" | "cluster-edge-memberships" => {
            let node = table == "cluster-node-memberships";
            let mut rows = Vec::new();
            for cluster in &clusters {
                for id in arr(get(
                    cluster,
                    if node {
                        "member_node_ids"
                    } else {
                        "member_edge_ids"
                    },
                ))
                .iter()
                .filter_map(JsonValue::as_str)
                {
                    w.step()?;
                    if !(if node {
                        ids.contains(id)
                    } else {
                        edge_ids.contains(id)
                    }) {
                        continue;
                    }
                    rows.push(object(vec![
                        ("cluster_id", get(cluster, "cluster_id").clone()),
                        (if node { "node_id" } else { "edge_id" }, text(id)),
                        ("source_ref", get(cluster, "source_ref").clone()),
                        (
                            "source_refs",
                            cluster
                                .object_get("source_refs")
                                .cloned()
                                .unwrap_or_else(|| values([])),
                        ),
                    ]));
                }
            }
            Ok(rows)
        }
        _ => Err(unknown()),
    }
}
pub(super) fn scale<'a>(
    graph: &Graph<'a>,
    base_nodes: &'a [JsonValue],
    base_edges: &'a [JsonValue],
    table: Option<&str>,
    view_id: Option<&str>,
    layers: &[String],
    page: Option<(usize, usize)>,
    w: &mut Work<'_>,
) -> Result<JsonValue, SearchV2Error> {
    let layers = layers.iter().cloned().collect::<BTreeSet<_>>();
    if let Some(table) = table {
        let rows = scale_rows(graph, base_nodes, base_edges, table, view_id, &layers, w)?;
        let total = rows.len();
        let (offset, limit) = page.unwrap_or((0, total));
        let rows = rows
            .into_iter()
            .skip(offset)
            .take(limit)
            .collect::<Vec<_>>();
        let count = rows.len();
        let next = offset.saturating_add(count);
        return Ok(object(vec![
            ("schema", text("tos_philosophy_mcp_scale_rows_v1")),
            ("table", text(table)),
            ("view_id", view_id.map(text).unwrap_or(JsonValue::Null)),
            ("layers", texts(layers.iter().cloned())),
            ("offset", number(offset)),
            ("limit", number(limit)),
            ("row_count", number(count)),
            ("total_row_count", number(total)),
            (
                "next_offset",
                if next < total {
                    number(next)
                } else {
                    JsonValue::Null
                },
            ),
            ("rows", values(rows)),
            ("source_projection_ref", text(PROJECTION_PATH)),
            (
                "authority_note",
                text(
                    "Scale rows are MCP navigation packets; ToS derived exports remain authoritative.",
                ),
            ),
        ]));
    }
    let mut tables = Vec::new();
    for table in TABLES {
        let rows = scale_rows(graph, base_nodes, base_edges, table, view_id, &layers, w)?;
        tables.push((
            JsonString::from_utf8(table),
            object(vec![
                ("row_count", number(rows.len())),
                ("packet_route", text("tos_philosophy_graph_scale_rows")),
                (
                    "packet_route_args",
                    object(vec![
                        ("table", text(table)),
                        ("view_id", view_id.map(text).unwrap_or(JsonValue::Null)),
                        ("layers", texts(layers.iter().cloned())),
                    ]),
                ),
            ]),
        ));
    }
    Ok(object(vec![
        ("schema", text("tos_philosophy_mcp_scale_manifest_v1")),
        ("view_id", view_id.map(text).unwrap_or(JsonValue::Null)),
        ("layers", texts(layers)),
        ("tables", JsonValue::Object(tables)),
        ("source_projection_ref", text(PROJECTION_PATH)),
        (
            "runtime_projection_boundary",
            metadata(graph.header, "runtime_projection_boundary"),
        ),
        (
            "authority_note",
            text(
                "Scale manifests are MCP navigation packets; ToS derived exports remain authoritative.",
            ),
        ),
    ]))
}
