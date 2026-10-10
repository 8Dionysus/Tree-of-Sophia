//! Finite saved LensSpecs selected by the maintained public view declarations.

use crate::{
    Error, Result,
    d1_public_capture::{CreationState, MAX_ROW_BYTES, PublicCapture},
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
// Fixed maintained LensSpec shape. Dynamic source text changes only the
// existing scalar slots; the controlled route admits the actual parsed shape
// and every replacement before its string copy.
const SPEC_TEMPLATE: &[u8] = br#"{
    "schema_version":"tos_lens_spec_v1","lens_id":"",
    "title":{"default":"","ru":null,"en":null,"original":null},
    "description":{"default":"","ru":null,"en":null,"original":null},
    "language":"auto","detail":"full","path_query":[],"explain":false,
    "pagination":null,"sources":[""],
    "seed":{"focus_node_id":null,"node_ids":[],"text_query":""},
    "node_query":{"enabled":true,"match":"all","filters":[{"field":"view_ids","op":"contains","value":""}]},
    "relation_query":{"enabled":true,"match":"all","filters":[{"field":"view_ids","op":"contains","value":""}]},
    "traversal":{"depth":0,"direction":"either","predicate_ids":[],"profile":"all"},
    "composition":{"endpoint_policy":"both","group_by":[],"sort_nodes":[{"field":"id","direction":"asc"}],"sort_relations":[{"field":"id","direction":"asc"}]},
    "presentation":{"layout":"","color_by":"kind_id","lane_by":"","size_by":null,"inspector_fields":["display","epistemic","source_refs"]},
    "limits":{"nodes":1000,"relations":2000,"groups":100}}
"#;
fn spec_inner(
    row: &Value,
    id: &str,
    source: &str,
    title: &str,
    description: &str,
    lane: &str,
    state: Option<&CreationState<'_>>,
) -> Result<Value> {
    let mut output = match state {
        Some(state) => state.serde_owned(SPEC_TEMPLATE, SPEC_TEMPLATE.len())?,
        None => serde_json::from_slice(SPEC_TEMPLATE)
            .map_err(|_| Error::Invalid("maintained LensSpec template"))?,
    };
    let _layout_hold = state
        .map(|s| {
            s.hold(
                row.get("layout_hint")
                    .and_then(Value::as_str)
                    .map_or(0, str::len),
            )
        })
        .transpose()?;
    let layout = layout(row);
    if let Some(state) = state {
        state.retain(
            id.len()
                .checked_mul(3)
                .and_then(|n| n.checked_add(source.len()))
                .and_then(|n| n.checked_add(title.len()))
                .and_then(|n| n.checked_add(description.len()))
                .and_then(|n| n.checked_add(lane.len()))
                .and_then(|n| n.checked_add(layout.len()))
                .ok_or(Error::Budget("owned LensSpec scalar state"))?,
        )?;
    }
    output["lens_id"] = Value::String(id.to_owned());
    output["title"]["default"] = Value::String(title.to_owned());
    output["description"]["default"] = Value::String(description.to_owned());
    output["sources"][0] = Value::String(source.to_owned());
    output["node_query"]["filters"][0]["value"] = Value::String(id.to_owned());
    output["relation_query"]["filters"][0]["value"] = Value::String(id.to_owned());
    output["presentation"]["layout"] = Value::String(layout.to_owned());
    output["presentation"]["lane_by"] = Value::String(lane.to_owned());
    Ok(output)
}
fn spec(row: &Value, id: &str, source: &str, title: &str, description: &str, lane: &str) -> Value {
    spec_inner(row, id, source, title, description, lane, None)
        .expect("maintained LensSpec template")
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

pub(crate) fn saved_lenses_owned(
    capture: &PublicCapture,
    state: &CreationState<'_>,
) -> Result<Vec<Value>> {
    let mut result = BTreeMap::<String, Value>::new();
    for (role, collection) in [("philosophy", "views"), ("corpus", "graph_views")] {
        capture.visit_rows(role, collection, |_, raw| {
            if raw.len() > MAX_ROW_BYTES {
                return Err(Error::Budget("public D1 view row bytes"));
            }
            let limits = tos_foundation::JsonLimits::new(MAX_ROW_BYTES, 96, 1_000_000, 4096)
                .map_err(|_| Error::Budget("owned LensSpec row JSON"))?;
            state.with_serde_owned_with_limits(raw, limits, |row| {
                let Some(id) = text(row, "view_id") else {
                    return Ok(());
                };
                let (source, lane, description_key) = if role == "philosophy" {
                    if result.len() >= 63
                        || id.len() > 128
                        || !id
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
                    {
                        return Err(Error::Budget("public D1 saved lens IDs"));
                    }
                    ("philosophy", "epistemic.canon_status", "review_intent")
                } else {
                    let source = match id {
                        "corpus-topology" => "repository",
                        "route-graph" | "promotion-flow" => "canon",
                        _ => return Ok(()),
                    };
                    if result.contains_key(id) {
                        return Ok(());
                    }
                    (source, "epistemic.authority_layer", "purpose")
                };
                // The optional humanization word list is bounded by the same
                // authored ID; these temporary owners do not persist per row.
                let temp = id
                    .len()
                    .max(4)
                    .checked_mul(std::mem::size_of::<&str>() + 1)
                    .and_then(|n| n.checked_add(64 + id.len()))
                    .ok_or(Error::Budget("owned LensSpec fallback state"))?;
                let _fallback_hold = state.hold(temp)?;
                let title = text(row, "title")
                    .map(std::borrow::Cow::Borrowed)
                    .unwrap_or_else(|| std::borrow::Cow::Owned(humanize(id)));
                let description = text(row, description_key)
                    .map(std::borrow::Cow::Borrowed)
                    .unwrap_or_else(|| {
                        std::borrow::Cow::Owned(if role == "philosophy" {
                            format!("Source-owned ToS lens {id}.")
                        } else {
                            format!("ToS corpus lens {id}.")
                        })
                    });
                let value = spec_inner(row, id, source, &title, &description, lane, Some(state))?;
                // A fresh BTree node can contain eleven entries and twelve
                // child pointers; reserve one complete node before insertion.
                let node = 11 * std::mem::size_of::<(String, Value)>()
                    + 12 * std::mem::size_of::<usize>()
                    + 64;
                state.retain(
                    node.checked_add(id.len())
                        .ok_or(Error::Budget("owned LensSpec map state"))?,
                )?;
                if result.insert(id.to_owned(), value).is_some() {
                    return Err(Error::Invalid("public D1 duplicate philosophy view"));
                }
                Ok(())
            })
        })?;
    }
    state.retain(
        result
            .len()
            .checked_mul(std::mem::size_of::<Value>())
            .ok_or(Error::Budget("owned LensSpec output slots"))?,
    )?;
    let mut output = Vec::with_capacity(result.len());
    output.extend(result.into_values());
    Ok(output)
}
