//! Source-visible semantic checks for witness text collation.

use crate::assessment::owner_values_equal;
use crate::item_rules::ItemRefusal;
use serde_json::Value;

fn json_truthy(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => false,
        Some(Value::Bool(value)) => *value,
        Some(Value::Number(value)) => value.as_f64().is_some_and(|number| number != 0.0),
        Some(Value::String(value)) => !value.is_empty(),
        Some(Value::Array(value)) => !value.is_empty(),
        Some(Value::Object(value)) => !value.is_empty(),
    }
}

fn python_duplicate_count(values: &[Value]) -> Result<bool, ItemRefusal> {
    for right in 0..values.len() {
        for left in 0..right {
            if owner_values_equal(&values[left], &values[right])? {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn python_contains(values: &[Value], needle: &Value) -> Result<bool, ItemRefusal> {
    for value in values {
        if owner_values_equal(value, needle)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Shared semantic closure for an already bounded, schema-checked collation packet.
pub fn witness_text_collation_semantic_messages(
    packet: &Value,
) -> Result<Vec<String>, ItemRefusal> {
    let rows = |field: &str| -> Vec<&Value> {
        packet
            .get(field)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|row| row.is_object())
            .collect()
    };
    let witnesses = rows("witnesses");
    let collations = rows("collations");
    let reviews = rows("reviews");
    let witness_ids: Vec<_> = witnesses
        .iter()
        .map(|row| row.get("witness_id").cloned().unwrap_or(Value::Null))
        .collect();
    let witness_ordinals: Vec<_> = witnesses
        .iter()
        .map(|row| row.get("ordinal").cloned().unwrap_or(Value::Null))
        .collect();
    let expressions: Vec<_> = witnesses
        .iter()
        .map(|row| row.get("expression_ref").cloned().unwrap_or(Value::Null))
        .collect();
    let review_ids: Vec<_> = reviews
        .iter()
        .map(|row| row.get("review_id").cloned().unwrap_or(Value::Null))
        .collect();
    let mut messages = Vec::new();
    if python_duplicate_count(&witness_ids)? {
        messages.push("duplicate witness identity".into());
    }
    if python_duplicate_count(&witness_ordinals)? {
        messages.push("duplicate witness ordinal".into());
    }
    if python_duplicate_count(&expressions)? {
        messages.push("collation witnesses must retain distinct Expression identities".into());
    }
    let known_witnesses = witness_ids;
    let known_reviews = review_ids;
    let known_collations: Vec<_> = collations
        .iter()
        .filter_map(|row| row.get("collation_id").filter(|value| value.is_string()))
        .cloned()
        .collect();
    let mut accepted_ids = Vec::<Value>::new();
    for row in &collations {
        let status = row.get("status").and_then(Value::as_str);
        let decided = matches!(
            status,
            Some("accepted" | "accepted_with_limits" | "rejected" | "superseded")
        );
        let accepted = matches!(status, Some("accepted" | "accepted_with_limits"));
        for reference in row
            .get("ordered_witness_refs")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if !python_contains(&known_witnesses, reference)? {
                messages.push("collation references an unknown witness".into());
            }
        }
        let review_refs = row
            .get("review_refs")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for reference in &review_refs {
            if !python_contains(&known_reviews, reference)? {
                messages.push("collation references an unknown review".into());
            }
        }
        if decided && review_refs.is_empty() {
            messages.push("decided collation lacks human review".into());
        }
        if accepted && row.pointer("/maker/maker_kind").and_then(Value::as_str) != Some("human") {
            messages.push("non-human collation acceptance".into());
        }
        if owner_values_equal(
            row.get("collation_id").unwrap_or(&Value::Null),
            row.get("supersedes_collation_ref").unwrap_or(&Value::Null),
        )? {
            messages.push("collation cannot supersede itself".into());
        }
        if owner_values_equal(
            row.get("claim_id").unwrap_or(&Value::Null),
            row.get("supersedes_claim_ref").unwrap_or(&Value::Null),
        )? {
            messages.push("collation claim cannot supersede itself".into());
        }
        if let Some(boundary) = row.get("interpretive_boundary").and_then(Value::as_object) {
            if boundary.values().any(|value| value != &Value::Bool(false)) {
                messages.push("collation interpretive boundary widened".into());
            }
        }
        if accepted {
            if let Some(id) = row.get("collation_id") {
                accepted_ids.push(id.clone());
            }
        }
    }
    for projection in rows("projections") {
        let references = projection
            .get("source_collation_refs")
            .and_then(Value::as_array)
            .into_iter()
            .flatten();
        let mut unknown = false;
        let mut unaccepted = false;
        for reference in references {
            if !python_contains(&known_collations, reference)? {
                unknown = true;
            }
            if !python_contains(&accepted_ids, reference)? {
                unaccepted = true;
            }
        }
        if unknown {
            messages.push("projection references an unknown collation".into());
        }
        if unaccepted {
            messages.push("projection uses a collation that is not accepted".into());
        }
    }
    let rights = packet.get("rights_and_visibility");
    if json_truthy(rights.and_then(|value| value.get("private_source_used")))
        && json_truthy(rights.and_then(|value| value.get("publication_authorized")))
    {
        messages.push("private-source collation authorizes publication".into());
    }
    Ok(messages)
}
