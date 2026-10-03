//! Finite saved LensSpecs selected by the maintained public view declarations.

use crate::{
    Error, Result,
    d1_public_capture::{MAX_ROW_BYTES, PublicCapture},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn text<'a>(row: &'a Value, key: &str) -> Option<&'a str> {
    row.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}
fn humanize(id: &str) -> String {
    id.split(|c| c == '-' || c == '_' || c == '.')
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}
fn layout(row: &Value) -> &'static str {
    let hint = row
        .get("layout_hint")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_ascii_lowercase();
    for (needle, layout) in [
        ("timeline", "timeline"),
        ("flow", "flow"),
        ("corridor", "flow"),
        ("dag", "evidence"),
        ("evidence", "evidence"),
        ("semantic", "semantic"),
        ("infrastructure", "infrastructure"),
        ("layered", "hierarchical"),
        ("radial", "radial"),
        ("matrix", "matrix"),
    ] {
        if hint.contains(needle) {
            return layout;
        }
    }
    "organic"
}
fn spec(row: &Value, id: &str, source: &str, title: &str, description: &str, lane: &str) -> Value {
    json!({
        "schema_version":"tos_lens_spec_v1", "lens_id":id,
        "title":{"default":title,"ru":null,"en":null,"original":null},
        "description":{"default":description,"ru":null,"en":null,"original":null},
        "language":"auto", "detail":"full", "path_query":[], "explain":false,
        "pagination":null, "sources":[source],
        "seed":{"focus_node_id":null,"node_ids":[],"text_query":""},
        "node_query":{"enabled":true,"match":"all","filters":[{"field":"view_ids","op":"contains","value":id}]},
        "relation_query":{"enabled":true,"match":"all","filters":[{"field":"view_ids","op":"contains","value":id}]},
        "traversal":{"depth":0,"direction":"either","predicate_ids":[],"profile":"all"},
        "composition":{"endpoint_policy":"both","group_by":[],"sort_nodes":[{"field":"id","direction":"asc"}],
            "sort_relations":[{"field":"id","direction":"asc"}]},
        "presentation":{"layout":layout(row),"color_by":"kind_id","lane_by":lane,
            "size_by":null,"inspector_fields":["display","epistemic","source_refs"]},
        "limits":{"nodes":1000,"relations":2000,"groups":100}
    })
}
pub(crate) fn saved_lenses(capture: &PublicCapture) -> Result<Vec<Value>> {
    let mut result = BTreeMap::<String, Value>::new();
    capture.visit_rows("philosophy", "views", |_, raw| {
        if raw.len() > MAX_ROW_BYTES {
            return Err(Error::Budget("public D1 view row bytes"));
        }
        let row: Value = serde_json::from_slice(raw).map_err(|e| Error::Source(e.to_string()))?;
        if let Some(id) = text(&row, "view_id") {
            if result.len() >= 63
                || id.len() > 128
                || !id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
            {
                return Err(Error::Budget("public D1 saved lens IDs"));
            }
            let title = text(&row, "title")
                .map(str::to_owned)
                .unwrap_or_else(|| humanize(id));
            let description = text(&row, "review_intent")
                .map(str::to_owned)
                .unwrap_or_else(|| format!("Source-owned ToS lens {id}."));
            if result
                .insert(
                    id.to_owned(),
                    spec(
                        &row,
                        id,
                        "philosophy",
                        &title,
                        &description,
                        "epistemic.canon_status",
                    ),
                )
                .is_some()
            {
                return Err(Error::Invalid("public D1 duplicate philosophy view"));
            }
        }
        Ok(())
    })?;
    capture.visit_rows("corpus", "graph_views", |_, raw| {
        if raw.len() > MAX_ROW_BYTES {
            return Err(Error::Budget("public D1 corpus view row bytes"));
        }
        let row: Value = serde_json::from_slice(raw).map_err(|e| Error::Source(e.to_string()))?;
        let Some(id) = text(&row, "view_id") else {
            return Ok(());
        };
        let source = match id {
            "corpus-topology" => "repository",
            "route-graph" | "promotion-flow" => "canon",
            _ => return Ok(()),
        };
        if result.contains_key(id) {
            return Ok(());
        }
        let title = text(&row, "title")
            .map(str::to_owned)
            .unwrap_or_else(|| humanize(id));
        let description = text(&row, "purpose")
            .map(str::to_owned)
            .unwrap_or_else(|| format!("ToS corpus lens {id}."));
        result.insert(
            id.to_owned(),
            spec(
                &row,
                id,
                source,
                &title,
                &description,
                "epistemic.authority_layer",
            ),
        );
        Ok(())
    })?;
    Ok(result.into_values().collect())
}
