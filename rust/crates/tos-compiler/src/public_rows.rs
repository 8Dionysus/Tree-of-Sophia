use tos_foundation::{JsonNumber, JsonNumberKind, JsonString, JsonValue};

#[cfg(feature = "source-tools")]
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

fn int(value: usize) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: value.to_string(),
    })
}
fn entry(name: &str, value: JsonValue) -> (JsonString, JsonValue) {
    (JsonString::from_utf8(name), value)
}
/// Compact a validated philosophy view for its public carrier. Counts describe
/// the full view; a read adapter may append a bounded set of node/edge IDs.
/// Callers own original-row validation, count selection, input/work limits and
/// custody. This operation clones only the already bounded metadata fields.
pub fn project_private_philosophy_view_row(
    value: &JsonValue,
    node_count: usize,
    edge_count: usize,
) -> JsonValue {
    let Some(source) = value.as_object() else {
        return value.clone();
    };
    let mut fields = source
        .iter()
        .filter(|(key, _)| {
            !matches!(
                key.as_str(),
                Some("node_ids" | "edge_ids" | "nodes" | "edges" | "node_count" | "edge_count")
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    fields.push(entry("node_count", int(node_count)));
    fields.push(entry("edge_count", int(edge_count)));
    JsonValue::Object(fields)
}
