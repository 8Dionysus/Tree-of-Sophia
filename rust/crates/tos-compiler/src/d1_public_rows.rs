//! Exact source-projection rows for the disposable v9 D1 transport. The
//! capture owns source order; this module never rebuilds a projection in RAM.

use crate::{
    Error, Result,
    d1_public_capture::{MAX_ROW_BYTES, PublicCapture, compact, json},
    d1_public_sql::{MAX_ROW_VALUE_BYTES, SqlSink, chunks, quote, quote_len},
};
use rusqlite::OptionalExtension;
use std::{collections::BTreeMap, path::Path};
use tos_foundation::{
    Digest256, JsonNumber, JsonNumberKind, JsonString, JsonValue, python_lower_unicode16_v1,
};

pub(crate) fn portable(value: &mut JsonValue, root: &str) {
    match value {
        JsonValue::String(text) => {
            if let Some(current) = text.as_str() {
                let replacement = if current == root {
                    Some("Tree-of-Sophia")
                } else {
                    current.strip_prefix(root).and_then(|s| s.strip_prefix('/'))
                };
                if let Some(value) = replacement {
                    *text = JsonString::from_utf8(value);
                }
            }
        }
        JsonValue::Array(items) => {
            for item in items {
                portable(item, root)
            }
        }
        JsonValue::Object(items) => {
            for (_, item) in items {
                portable(item, root)
            }
        }
        _ => {}
    }
}

fn source_value(capture: &PublicCapture, raw: &[u8], root: &str) -> Result<JsonValue> {
    capture.charge_work(raw.len() as u64)?;
    let mut value = json(raw, MAX_ROW_BYTES)?;
    portable(&mut value, root);
    Ok(value)
}
fn text<'a>(value: &'a JsonValue, key: &str) -> Option<&'a str> {
    value.object_get(key).and_then(JsonValue::as_str)
}
fn default_text<'a>(value: &'a JsonValue, key: &str) -> &'a str {
    text(value, key).unwrap_or("")
}
fn identity(value: &JsonValue, fallback: &str) -> String {
    for key in [
        "node_id",
        "edge_id",
        "cluster_id",
        "view_id",
        "resource_id",
        "manifest_id",
        "pack_id",
        "id",
        "path",
        "layer_id",
    ] {
        if let Some(id) = text(value, key).filter(|id| !id.is_empty()) {
            return id.to_owned();
        }
    }
    fallback.to_owned()
}
fn array_strings<'a>(value: &'a JsonValue, key: &str) -> impl Iterator<Item = &'a str> {
    value
        .object_get(key)
        .and_then(JsonValue::as_array)
        .into_iter()
        .flatten()
        .filter_map(JsonValue::as_str)
        .filter(|value| !value.is_empty())
}
pub(crate) fn encoded(capture: &PublicCapture, value: &JsonValue) -> Result<String> {
    let raw = compact(value, MAX_ROW_BYTES)?;
    capture.charge_work(raw.len() as u64)?;
    String::from_utf8(raw).map_err(|_| Error::Invalid("public D1 compact JSON UTF-8"))
}
pub(crate) fn preflight_large_fields(capture: &PublicCapture, values: &[&str]) -> Result<()> {
    let mut total = 1024usize;
    for value in values {
        total = total
            .checked_add(quote_len(value)?)
            .ok_or(Error::Budget("D1 SQL row value bytes"))?;
    }
    if total > MAX_ROW_VALUE_BYTES {
        return Err(Error::Budget("D1 SQL row value bytes"));
    }
    capture.charge_work(total as u64)
}
pub(crate) fn lower_search(capture: &PublicCapture, input: &str) -> Result<String> {
    if quote_len(input)?
        .checked_add(1024)
        .ok_or(Error::Budget("D1 SQL row value bytes"))?
        > MAX_ROW_VALUE_BYTES
    {
        return Err(Error::Budget("D1 SQL row value bytes"));
    }
    capture.charge_work(input.len() as u64)?;
    let value = python_lower_unicode16_v1(
        input,
        MAX_ROW_BYTES,
        crate::d1_public_sql::MAX_ROW_VALUE_BYTES,
        crate::d1_public_sql::MAX_ROW_VALUE_BYTES,
    )
    .map_err(|error| Error::Source(error.to_string()))?;
    capture.charge_work(value.len() as u64)?;
    Ok(value)
}
fn int(value: usize) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: value.to_string(),
    })
}
fn entry(name: &str, value: JsonValue) -> (JsonString, JsonValue) {
    (JsonString::from_utf8(name), value)
}
fn mask(value: &JsonValue, key: &str, positions: &BTreeMap<String, u32>) -> Result<u64> {
    let mut mask = 0u64;
    for id in array_strings(value, key) {
        if let Some(position) = positions.get(id) {
            mask |= 1u64
                .checked_shl(*position)
                .ok_or(Error::Budget("public D1 mask positions"))?;
        }
    }
    if mask > i64::MAX as u64 {
        return Err(Error::Budget("public D1 SQLite mask"));
    }
    Ok(mask)
}
fn insert_large(
    sink: &mut SqlSink,
    table: &str,
    columns: &[&str],
    values: &[String],
    selector: &str,
    large: &[(&str, &str)],
) -> Result<()> {
    sink.insert_chunked(table, columns, values, selector, large)
}
pub(crate) fn quoted(capture: &PublicCapture, value: &str) -> Result<String> {
    let length = quote_len(value)?;
    if length > MAX_ROW_VALUE_BYTES {
        return Err(Error::Budget("public D1 quoted field bytes"));
    }
    capture.charge_work(length as u64)?;
    quote(value)
}
fn nullable(capture: &PublicCapture, value: Option<&str>) -> Result<String> {
    value
        .map(|value| quoted(capture, value))
        .transpose()
        .map(|value| value.unwrap_or_else(|| "NULL".to_owned()))
}
fn positions(
    capture: &PublicCapture,
    collection: &str,
    key: &str,
    root: &str,
) -> Result<BTreeMap<String, u32>> {
    let mut result = BTreeMap::new();
    capture.visit_rows("philosophy", collection, |position, raw| {
        let value = source_value(capture, raw, root)?;
        let id = default_text(&value, key);
        if !id.is_empty() {
            let position =
                u32::try_from(position).map_err(|_| Error::Budget("public D1 mask positions"))?;
            if position >= 63 || result.insert(id.to_owned(), position).is_some() {
                return Err(Error::Invalid("public D1 philosophy view/layer identity"));
            }
        }
        Ok(())
    })?;
    Ok(result)
}

#[derive(Default)]
pub(crate) struct SourceSqlCounts {
    pub philosophy_nodes: u64,
    pub philosophy_edges: u64,
    pub corpus_items: u64,
    pub corpus_edges: u64,
    pub corpus_packs: u64,
    pub cluster_node_memberships: u64,
    pub cluster_edge_memberships: u64,
    pub navigation_nodes: u64,
    pub navigation_edges: u64,
    pub navigation_rights: u64,
    pub navigation_payload_chunks: u64,
}

/// The caller passes its resolved repository root and owns the single SQL
/// sink/output budget. Source row bytes are rechecked by PublicCapture.
pub(crate) fn emit_philosophy(
    capture: &PublicCapture,
    sink: &mut SqlSink,
    root: &Path,
    counts: &mut SourceSqlCounts,
) -> Result<()> {
    let root = root
        .to_str()
        .ok_or(Error::Invalid("public D1 root UTF-8"))?;
    let views = positions(capture, "views", "view_id", root)?;
    let layers = positions(capture, "graph_layers", "layer_id", root)?;
    capture.visit_rows("philosophy", "nodes", |order, raw| {
        let item = source_value(capture, raw, root)?;
        let item_json = encoded(capture, &item)?;
        let item_id = default_text(&item, "node_id");
        let search = lower_search(capture, &item_json)?;
        preflight_large_fields(capture, &[&item_json, &search])?;
        let values = vec![
            quoted(capture, item_id)?,
            order.to_string(),
            mask(&item, "view_ids", &views)?.to_string(),
            mask(&item, "graph_layers", &layers)?.to_string(),
            quoted(capture, &item_json)?,
            quoted(capture, &search)?,
        ];
        insert_large(
            sink,
            "philosophy_nodes_next",
            &[
                "id",
                "ord",
                "view_mask",
                "layer_mask",
                "json",
                "search_text",
            ],
            &values,
            &format!("id={}", quoted(capture, item_id)?),
            &[("json", &item_json), ("search_text", &search)],
        )?;
        counts.philosophy_nodes += 1;
        Ok(())
    })?;
    capture.visit_rows("philosophy", "edges", |order, raw| {
        let item = source_value(capture, raw, root)?;
        let item_json = encoded(capture, &item)?;
        let item_id = default_text(&item, "edge_id");
        let search = lower_search(capture, &item_json)?;
        preflight_large_fields(capture, &[&item_json, &search])?;
        let values = vec![
            quoted(capture, item_id)?,
            order.to_string(),
            quoted(capture, default_text(&item, "from_id"))?,
            quoted(capture, default_text(&item, "to_id"))?,
            quoted(capture, default_text(&item, "predicate_id"))?,
            mask(&item, "view_ids", &views)?.to_string(),
            mask(&item, "graph_layers", &layers)?.to_string(),
            quoted(capture, &item_json)?,
            quoted(capture, &search)?,
        ];
        insert_large(
            sink,
            "philosophy_edges_next",
            &[
                "id",
                "ord",
                "from_id",
                "to_id",
                "predicate_id",
                "view_mask",
                "layer_mask",
                "json",
                "search_text",
            ],
            &values,
            &format!("id={}", quoted(capture, item_id)?),
            &[("json", &item_json), ("search_text", &search)],
        )?;
        counts.philosophy_edges += 1;
        Ok(())
    })?;
    for collection in ["views", "clusters", "review_packets", "graph_layers"] {
        capture.visit_rows("philosophy", collection, |order, raw| {
            let item = source_value(capture, raw, root)?;
            let mut auxiliary = item.clone();
            if let JsonValue::Object(ref mut fields) = auxiliary {
                let mut node_count = 0usize;
                let mut edge_count = 0usize;
                let mut diagnostics = 0usize;
                if collection == "views" {
                    node_count = array_strings(&item, "node_ids").count();
                    edge_count = array_strings(&item, "edge_ids").count();
                    fields.retain(|(key, _)| {
                        !matches!(
                            key.as_str(),
                            Some("node_ids" | "edge_ids" | "nodes" | "edges")
                        )
                    });
                    fields.push(entry("node_count", int(node_count)));
                    fields.push(entry("edge_count", int(edge_count)));
                } else if collection == "clusters" {
                    node_count = array_strings(&item, "member_node_ids").count();
                    edge_count = array_strings(&item, "member_edge_ids").count();
                    fields.retain(|(key, _)| {
                        !matches!(key.as_str(), Some("member_node_ids" | "member_edge_ids"))
                    });
                    fields.push(entry("member_node_count", int(node_count)));
                    fields.push(entry("member_edge_count", int(edge_count)));
                } else if collection == "review_packets" {
                    diagnostics = item
                        .object_get("unresolved_diagnostics")
                        .and_then(JsonValue::as_array)
                        .map_or(0, |items| items.len());
                    fields.retain(|(key, _)| key.as_str() != Some("unresolved_diagnostics"));
                    fields.push(entry("unresolved_diagnostic_count", int(diagnostics)));
                }
            }
            let compact_item = encoded(capture, &auxiliary)?;
            let search = lower_search(capture, &compact_item)?;
            preflight_large_fields(capture, &[&compact_item, &search])?;
            let item_id = identity(&item, &format!("{collection}:{order}"));
            let values = vec![
                quoted(capture, collection)?,
                order.to_string(),
                quoted(capture, &item_id)?,
                quoted(capture, &compact_item)?,
                quoted(capture, &search)?,
            ];
            insert_large(
                sink,
                "philosophy_aux_next",
                &["collection", "ord", "id", "json", "search_text"],
                &values,
                &format!(
                    "collection={} AND ord={order}",
                    quoted(capture, collection)?
                ),
                &[("json", &compact_item), ("search_text", &search)],
            )?;
            if collection == "review_packets" {
                let view = default_text(&item, "view_id");
                let whole = encoded(capture, &item)?;
                preflight_large_fields(capture, &[&whole])?;
                insert_large(
                    sink,
                    "philosophy_review_packets_next",
                    &["view_id", "json"],
                    &[quoted(capture, view)?, quoted(capture, &whole)?],
                    &format!("view_id={}", quoted(capture, view)?),
                    &[("json", &whole)],
                )?;
            }
            if collection == "clusters" {
                let cluster_id = default_text(&item, "cluster_id");
                let whole = encoded(capture, &item)?;
                let sort_key = format!(
                    "{}\u{241f}{}",
                    default_text(&item, "cluster_kind"),
                    default_text(&item, "label")
                );
                let view_mask = mask(&item, "view_ids", &views)?.to_string();
                let layer_mask = mask(&item, "graph_layers", &layers)?.to_string();
                for (part, chunk) in chunks(&whole).enumerate() {
                    sink.insert(
                        "philosophy_clusters_next",
                        &[
                            "id",
                            "ord",
                            "sort_key",
                            "view_mask",
                            "layer_mask",
                            "part",
                            "json_chunk",
                        ],
                        &[
                            quoted(capture, cluster_id)?,
                            order.to_string(),
                            quoted(capture, &sort_key)?,
                            view_mask.clone(),
                            layer_mask.clone(),
                            part.to_string(),
                            quoted(capture, chunk)?,
                        ],
                    )?;
                }
                for (field, member_field, table) in [
                    (
                        "member_node_ids",
                        "node_id",
                        "philosophy_cluster_nodes_next",
                    ),
                    (
                        "member_edge_ids",
                        "edge_id",
                        "philosophy_cluster_edges_next",
                    ),
                ] {
                    for (member_order, member) in array_strings(&item, field).enumerate() {
                        let membership = JsonValue::Object(vec![
                            entry(
                                "cluster_id",
                                item.object_get("cluster_id")
                                    .cloned()
                                    .unwrap_or(JsonValue::Null),
                            ),
                            entry(
                                "source_ref",
                                item.object_get("source_ref")
                                    .cloned()
                                    .unwrap_or(JsonValue::Null),
                            ),
                            entry(
                                "source_refs",
                                item.object_get("source_refs")
                                    .cloned()
                                    .unwrap_or(JsonValue::Array(Vec::new())),
                            ),
                            entry(
                                member_field,
                                JsonValue::String(JsonString::from_utf8(member)),
                            ),
                        ]);
                        let membership_json = encoded(capture, &membership)?;
                        preflight_large_fields(capture, &[&membership_json])?;
                        sink.insert(
                            table,
                            &[
                                "cluster_id",
                                "cluster_ord",
                                "sort_key",
                                "member_ord",
                                "item_id",
                                "view_mask",
                                "layer_mask",
                                "json",
                            ],
                            &[
                                quoted(capture, cluster_id)?,
                                order.to_string(),
                                quoted(capture, &sort_key)?,
                                member_order.to_string(),
                                quoted(capture, member)?,
                                view_mask.clone(),
                                layer_mask.clone(),
                                quoted(capture, &membership_json)?,
                            ],
                        )?;
                        let counter = if member_field == "node_id" {
                            &mut counts.cluster_node_memberships
                        } else {
                            &mut counts.cluster_edge_memberships
                        };
                        *counter = counter
                            .checked_add(1)
                            .ok_or(Error::Budget("public D1 cluster memberships"))?;
                    }
                }
            }
            Ok(())
        })?;
    }
    Ok(())
}

pub(crate) fn emit_corpus(
    capture: &PublicCapture,
    sink: &mut SqlSink,
    root: &Path,
    counts: &mut SourceSqlCounts,
) -> Result<()> {
    let root = root
        .to_str()
        .ok_or(Error::Invalid("public D1 root UTF-8"))?;
    for collection in ["nodes", "resources", "manifests", "branches", "graph_views"] {
        capture.visit_rows("corpus", collection, |order, raw| {
            let item = source_value(capture, raw, root)?;
            let item_json = encoded(capture, &item)?;
            let search = lower_search(capture, &item_json)?;
            preflight_large_fields(capture, &[&item_json, &search])?;
            let id = identity(&item, &format!("{collection}:{order}"));
            let values = vec![
                quoted(capture, collection)?,
                order.to_string(),
                quoted(capture, &id)?,
                nullable(capture, text(&item, "resource_kind"))?,
                nullable(capture, text(&item, "owner_branch"))?,
                quoted(capture, &item_json)?,
                quoted(capture, &search)?,
            ];
            insert_large(
                sink,
                "corpus_items_next",
                &[
                    "collection",
                    "ord",
                    "id",
                    "resource_kind",
                    "owner_branch",
                    "json",
                    "search_text",
                ],
                &values,
                &format!(
                    "collection={} AND ord={order}",
                    quoted(capture, collection)?
                ),
                &[("json", &item_json), ("search_text", &search)],
            )?;
            counts.corpus_items = counts
                .corpus_items
                .checked_add(1)
                .ok_or(Error::Budget("public D1 corpus items"))?;
            Ok(())
        })?;
    }
    capture.visit_rows("corpus", "relation_packs", |order, raw| {
        let item = source_value(capture, raw, root)?;
        let id = default_text(&item, "pack_id");
        let item_json = encoded(capture, &item)?;
        preflight_large_fields(capture, &[&item_json])?;
        insert_large(
            sink,
            "corpus_packs_next",
            &["id", "ord", "json"],
            &[
                quoted(capture, id)?,
                order.to_string(),
                quoted(capture, &item_json)?,
            ],
            &format!("id={}", quoted(capture, id)?),
            &[("json", &item_json)],
        )?;
        counts.corpus_packs = counts
            .corpus_packs
            .checked_add(1)
            .ok_or(Error::Budget("public D1 corpus packs"))?;
        Ok(())
    })?;
    let pack_db = capture.read_db()?;
    let mut pack_path = pack_db.prepare("SELECT path FROM public_pack_paths WHERE pack_id=?1")?;
    capture.visit_rows("corpus", "relation_edges", |order, raw| {
        let mut item = source_value(capture, raw, root)?;
        if default_text(&item, "source_ref").is_empty() {
            if let Some(pack_id) = text(&item, "pack_id") {
                let source: Option<String> = pack_path
                    .query_row([pack_id], |row| row.get(0))
                    .optional()?;
                if let Some(source) = source {
                    capture.charge_work(source.len() as u64)?;
                    if let JsonValue::Object(fields) = &mut item {
                        if let Some((_, value)) = fields
                            .iter_mut()
                            .find(|(key, _)| key.as_str() == Some("source_ref"))
                        {
                            *value = JsonValue::String(JsonString::from_utf8(&source));
                        } else {
                            fields.push(entry(
                                "source_ref",
                                JsonValue::String(JsonString::from_utf8(&source)),
                            ));
                        }
                    }
                    portable(&mut item, root);
                }
            }
        }
        let item_json = encoded(capture, &item)?;
        preflight_large_fields(capture, &[&item_json])?;
        let id = text(&item, "edge_id")
            .filter(|id| !id.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| format!("corpus-edge:{order}"));
        insert_large(
            sink,
            "corpus_edges_next",
            &[
                "id",
                "ord",
                "from_id",
                "to_id",
                "pack_id",
                "owner_branch",
                "json",
            ],
            &[
                quoted(capture, &id)?,
                order.to_string(),
                quoted(capture, default_text(&item, "from_id"))?,
                quoted(capture, default_text(&item, "to_id"))?,
                nullable(capture, text(&item, "pack_id"))?,
                nullable(capture, text(&item, "owner_branch"))?,
                quoted(capture, &item_json)?,
            ],
            &format!("ord={order}"),
            &[("json", &item_json)],
        )?;
        counts.corpus_edges = counts
            .corpus_edges
            .checked_add(1)
            .ok_or(Error::Budget("public D1 corpus edges"))?;
        Ok(())
    })?;
    Ok(())
}

fn navigation_selection(item: &JsonValue, field: &str, keys: &[&str]) -> JsonValue {
    match item.object_get(field) {
        Some(JsonValue::Object(fields)) if !keys.is_empty() => JsonValue::Object(
            keys.iter()
                .filter_map(|name| {
                    fields
                        .iter()
                        .find(|(key, _)| key.as_str() == Some(*name))
                        .cloned()
                })
                .collect(),
        ),
        Some(JsonValue::Array(items)) if keys.is_empty() => JsonValue::Array(items.clone()),
        _ if keys.is_empty() => JsonValue::Array(Vec::new()),
        _ => JsonValue::Object(Vec::new()),
    }
}

/// Project one already-addressed private source-navigation value through the
/// same row/overflow/digest kernel used by the full D1 producer.  The caller
/// owns the snapshot and order proof; this function only derives carrier rows.
pub fn project_private_navigation_row(
    kind: &str,
    ordinal: i64,
    item: &mut JsonValue,
    repo_root: &str,
) -> Result<Vec<crate::d1::D1RowTransition>> {
    use crate::d1::{D1Cell as C, D1RowTransition as T, D1Table as D};

    if ordinal < 0 || !matches!(kind, "nodes" | "edges" | "rights") {
        return Err(Error::Invalid("private navigation row identity/order"));
    }
    portable(item, repo_root);
    let json = String::from_utf8(compact(item, MAX_ROW_BYTES)?)
        .map_err(|_| Error::Invalid("private navigation row UTF-8"))?;
    let text = |key: &str| {
        item.object_get(key)
            .and_then(JsonValue::as_str)
            .unwrap_or("")
    };
    let (id, table, payload_table, selection, mut values, base_columns, selection_column) =
        match kind {
            "nodes" => (
                text("node_id"),
                D::SourceNavigationNodes,
                D::SourceNavigationNodePayload,
                String::from_utf8(compact(
                    &navigation_selection(item, "properties", &["packet_id", "access_status"]),
                    MAX_ROW_BYTES,
                )?)
                .map_err(|_| Error::Invalid("private navigation properties UTF-8"))?,
                vec![
                    text("node_id").to_owned(),
                    ordinal.to_string(),
                    text("node_kind").to_owned(),
                    text("source_ref").to_owned(),
                    text("label").to_owned(),
                    text("identity_status").to_owned(),
                ],
                vec![
                    "node_id",
                    "ord",
                    "node_kind",
                    "source_ref",
                    "label",
                    "identity_status",
                ],
                "properties_json",
            ),
            "edges" => (
                text("edge_id"),
                D::SourceNavigationEdges,
                D::SourceNavigationEdgePayload,
                String::from_utf8(compact(
                    &navigation_selection(item, "source_refs", &[]),
                    MAX_ROW_BYTES,
                )?)
                .map_err(|_| Error::Invalid("private navigation source refs UTF-8"))?,
                vec![
                    text("edge_id").to_owned(),
                    ordinal.to_string(),
                    text("from_id").to_owned(),
                    text("to_id").to_owned(),
                    text("edge_kind").to_owned(),
                    text("predicate_id").to_owned(),
                    text("review_status").to_owned(),
                ],
                vec![
                    "edge_id",
                    "ord",
                    "from_id",
                    "to_id",
                    "edge_kind",
                    "predicate_id",
                    "review_status",
                ],
                "source_refs_json",
            ),
            "rights" => (
                text("rights_id"),
                D::SourceNavigationRights,
                D::SourceNavigationRightsPayload,
                String::from_utf8(compact(
                    &navigation_selection(item, "scope_refs", &[]),
                    MAX_ROW_BYTES,
                )?)
                .map_err(|_| Error::Invalid("private navigation scope refs UTF-8"))?,
                vec![text("rights_id").to_owned(), ordinal.to_string()],
                vec!["rights_id", "ord"],
                "scope_refs_json",
            ),
            _ => unreachable!(),
        };
    if id.is_empty() || id.len() > 4096 {
        return Err(Error::Invalid("private navigation row key"));
    }
    let quote_len = |value: &str| -> Result<usize> { crate::d1_public_sql::quote_len(value) };
    let base = base_columns.iter().try_fold(1024usize, |sum, value| {
        sum.checked_add(quote_len(value)?)
            .ok_or(Error::Budget("private navigation row bytes"))
    })?;
    let inline = base
        .checked_add(quote_len(&selection)?)
        .and_then(|size| size.checked_add(quote_len(&json).ok()?))
        .is_some_and(|size| size <= crate::d1_public_sql::MAX_ROW_VALUE_BYTES);
    let selection_retained = base
        .checked_add(quote_len(&selection)?)
        .and_then(|size| size.checked_add(2))
        .is_some_and(|size| size <= crate::d1_public_sql::MAX_ROW_VALUE_BYTES);
    if !inline
        && !base
            .checked_add(4)
            .is_some_and(|size| size <= crate::d1_public_sql::MAX_ROW_VALUE_BYTES)
    {
        return Err(Error::Budget("private navigation selection bytes"));
    }
    let retained_selection = if inline || selection_retained {
        selection
    } else {
        String::new()
    };
    let retained_json = if inline { json.clone() } else { String::new() };
    let mut row = base_columns
        .iter()
        .enumerate()
        .map(|(index, value)| {
            if index == 1 {
                C::Integer(ordinal)
            } else {
                C::Text((*value).to_owned())
            }
        })
        .collect::<Vec<_>>();
    row.push(C::Text(retained_selection));
    row.push(C::Text(retained_json));
    let mut result = vec![T {
        table,
        before: None,
        after: Some(row),
    }];
    if !inline {
        let chunks = crate::d1_public_sql::chunks(&json)
            .enumerate()
            .map(|(part, chunk)| T {
                table: payload_table,
                before: None,
                after: Some(vec![
                    C::Text(id.to_owned()),
                    C::Integer(part as i64),
                    C::Text(chunk.to_owned()),
                ]),
            })
            .collect::<Vec<_>>();
        result.extend(chunks);
    }
    let digest_key = format!(
        "source_navigation_row_digest:{kind}:{}",
        tos_foundation::Digest256::of_bytes(id.as_bytes()).to_hex()
    );
    let digest_row = format!(
        "{{\"sha256\":\"{}\"}}",
        tos_foundation::Digest256::of_bytes(json.as_bytes()).to_hex()
    );
    result.push(T {
        table: D::EdgeMeta,
        before: None,
        after: Some(vec![
            C::Text(digest_key),
            C::Integer(0),
            C::Text(digest_row),
        ]),
    });
    let _ = selection_column;
    Ok(result)
}

fn bounded_quote(capture: &PublicCapture, value: &str) -> Result<String> {
    quoted(capture, value)
}

fn emit_navigation_digest(
    capture: &PublicCapture,
    sink: &mut SqlSink,
    kind: &str,
    id: &str,
    json: &str,
) -> Result<()> {
    let key = format!(
        "source_navigation_row_digest:{kind}:{}",
        Digest256::of_bytes(id.as_bytes()).to_hex()
    );
    let digest = format!(
        "{{\"sha256\":\"{}\"}}",
        Digest256::of_bytes(json.as_bytes()).to_hex()
    );
    capture.charge_work(key.len() as u64 + digest.len() as u64)?;
    sink.insert(
        "edge_meta_next",
        &["key", "part", "json_chunk"],
        &[
            bounded_quote(capture, &key)?,
            "0".to_owned(),
            bounded_quote(capture, &digest)?,
        ],
    )
}

/// Exact public navigation and rights rows, including lossless overflow and
/// a digest over the JSON bytes actually emitted to the D1 carrier.
pub(crate) fn emit_navigation(
    capture: &PublicCapture,
    sink: &mut SqlSink,
    root: &Path,
    counts: &mut SourceSqlCounts,
) -> Result<()> {
    let root = root
        .to_str()
        .ok_or(Error::Invalid("public D1 root UTF-8"))?;
    for kind in ["nodes", "edges", "rights"] {
        capture.visit_rows(
            "corpus",
            &format!("source_navigation/{kind}"),
            |order, raw| {
                let item = source_value(capture, raw, root)?;
                let json = encoded(capture, &item)?;
                let (
                    table,
                    payload_table,
                    id_field,
                    columns,
                    selection,
                    selection_field,
                    mut values,
                ) = match kind {
                    "nodes" => {
                        let selection = encoded(
                            capture,
                            &navigation_selection(
                                &item,
                                "properties",
                                &["packet_id", "access_status"],
                            ),
                        )?;
                        (
                            "source_navigation_nodes_next",
                            "source_navigation_node_payload_next",
                            "node_id",
                            &[
                                "node_id",
                                "ord",
                                "node_kind",
                                "source_ref",
                                "label",
                                "identity_status",
                                "properties_json",
                                "json",
                            ][..],
                            selection,
                            "properties_json",
                            vec![
                                bounded_quote(capture, default_text(&item, "node_id"))?,
                                order.to_string(),
                                bounded_quote(capture, default_text(&item, "node_kind"))?,
                                bounded_quote(capture, default_text(&item, "source_ref"))?,
                                bounded_quote(capture, default_text(&item, "label"))?,
                                bounded_quote(capture, default_text(&item, "identity_status"))?,
                            ],
                        )
                    }
                    "edges" => {
                        let selection =
                            encoded(capture, &navigation_selection(&item, "source_refs", &[]))?;
                        (
                            "source_navigation_edges_next",
                            "source_navigation_edge_payload_next",
                            "edge_id",
                            &[
                                "edge_id",
                                "ord",
                                "from_id",
                                "to_id",
                                "edge_kind",
                                "predicate_id",
                                "review_status",
                                "source_refs_json",
                                "json",
                            ][..],
                            selection,
                            "source_refs_json",
                            vec![
                                bounded_quote(capture, default_text(&item, "edge_id"))?,
                                order.to_string(),
                                bounded_quote(capture, default_text(&item, "from_id"))?,
                                bounded_quote(capture, default_text(&item, "to_id"))?,
                                bounded_quote(capture, default_text(&item, "edge_kind"))?,
                                bounded_quote(capture, default_text(&item, "predicate_id"))?,
                                bounded_quote(capture, default_text(&item, "review_status"))?,
                            ],
                        )
                    }
                    "rights" => {
                        let selection =
                            encoded(capture, &navigation_selection(&item, "scope_refs", &[]))?;
                        (
                            "source_navigation_rights_next",
                            "source_navigation_rights_payload_next",
                            "rights_id",
                            &["rights_id", "ord", "scope_refs_json", "json"][..],
                            selection,
                            "scope_refs_json",
                            vec![
                                bounded_quote(capture, default_text(&item, "rights_id"))?,
                                order.to_string(),
                            ],
                        )
                    }
                    _ => return Err(Error::Invalid("public D1 navigation kind")),
                };
                let id = default_text(&item, id_field);
                if id.is_empty() || id.len() > 4096 {
                    return Err(Error::Invalid("public D1 navigation identity"));
                }
                let base = values.iter().try_fold(1024usize, |sum, value| {
                    sum.checked_add(value.len())
                        .ok_or(Error::Budget("public D1 navigation row bytes"))
                })?;
                let selection_len = quote_len(&selection)?;
                let json_len = quote_len(&json)?;
                let inline = base
                    .checked_add(selection_len)
                    .and_then(|n| n.checked_add(json_len))
                    .is_some_and(|n| n <= MAX_ROW_VALUE_BYTES);
                let selection_retained = base
                    .checked_add(selection_len)
                    .and_then(|n| n.checked_add(2))
                    .is_some_and(|n| n <= MAX_ROW_VALUE_BYTES);
                if !inline
                    && !base
                        .checked_add(4)
                        .is_some_and(|n| n <= MAX_ROW_VALUE_BYTES)
                {
                    return Err(Error::Budget("public D1 navigation selection fields"));
                }
                values.push(if inline || selection_retained {
                    bounded_quote(capture, &selection)?
                } else {
                    "''".to_owned()
                });
                values.push(if inline {
                    bounded_quote(capture, &json)?
                } else {
                    "''".to_owned()
                });
                if inline {
                    let selector = format!("{id_field}={}", bounded_quote(capture, id)?);
                    sink.insert_chunked(
                        table,
                        columns,
                        &values,
                        &selector,
                        &[(selection_field, &selection), ("json", &json)],
                    )?;
                } else {
                    sink.insert(table, columns, &values)?;
                    for (part, chunk) in chunks(&json).enumerate() {
                        sink.insert(
                            payload_table,
                            &["id", "part", "json_chunk"],
                            &[
                                bounded_quote(capture, id)?,
                                part.to_string(),
                                bounded_quote(capture, chunk)?,
                            ],
                        )?;
                        counts.navigation_payload_chunks = counts
                            .navigation_payload_chunks
                            .checked_add(1)
                            .ok_or(Error::Budget("public D1 navigation payload chunks"))?;
                    }
                }
                emit_navigation_digest(capture, sink, kind, id, &json)?;
                let counter = match kind {
                    "nodes" => &mut counts.navigation_nodes,
                    "edges" => &mut counts.navigation_edges,
                    _ => &mut counts.navigation_rights,
                };
                *counter = counter
                    .checked_add(1)
                    .ok_or(Error::Budget("public D1 navigation rows"))?;
                Ok(())
            },
        )?;
    }
    Ok(())
}
