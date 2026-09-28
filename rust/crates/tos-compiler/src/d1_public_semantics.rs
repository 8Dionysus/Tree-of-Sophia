//! The public graph's semantic transport check over the final normalized
//! rows. The registries and indexed Stage own the bounded lookups; no graph
//! carrier or duplicate endpoint map is retained in process memory.

use crate::{
    Error, KnowledgeRegistry, Result,
    d1_public_capture::{MAX_HEADER_BYTES, MAX_ROW_BYTES, PublicCapture, json as strict_json},
    knowledge_stage::{KnowledgeStage, WritePhase},
};
use rusqlite::{OptionalExtension, Statement, params};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use tos_foundation::Digest256;

fn field<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.')
        .try_fold(value, |value, part| value.get(part))
}
fn string<'a>(value: &'a Value, path: &str) -> Option<&'a str> {
    field(value, path)?
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
}
fn strings<'a>(value: &'a Value, path: &str) -> impl Iterator<Item = &'a str> {
    field(value, path)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|text| !text.is_empty())
}
fn member<'a>(value: &'a Value, path: &str) -> &'a Value {
    field(value, path).unwrap_or(&Value::Null)
}
fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_some_and(|number| number != 0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(value) => !value.is_empty(),
        Value::Object(value) => !value.is_empty(),
    }
}
fn admitted_row_len(capture: &PublicCapture, len: i64) -> Result<usize> {
    if len < 0 {
        return Err(Error::Invalid("public D1 semantic row length"));
    }
    let length = usize::try_from(len).map_err(|_| Error::Budget("public D1 semantic row bytes"))?;
    if length > MAX_ROW_BYTES {
        return Err(Error::Budget("public D1 semantic row bytes"));
    }
    // Reserve the selected copy, digest and two decoders before row.get
    // materializes the BLOB. The Stage CASE returns NULL for bad lengths.
    capture.charge_work(
        (length as u64)
            .checked_mul(4)
            .ok_or(Error::Budget("public D1 semantic row work"))?,
    )?;
    Ok(length)
}
fn check_row(len: usize, digest: &[u8], raw: Option<Vec<u8>>) -> Result<Value> {
    let raw = raw.ok_or(Error::Invalid("public D1 semantic row length"))?;
    if raw.len() > MAX_ROW_BYTES
        || len != raw.len()
        || digest != Digest256::of_bytes(&raw).as_bytes()
    {
        return Err(Error::Invalid("public D1 semantic row digest"));
    }
    strict_json(&raw, MAX_ROW_BYTES)?;
    serde_json::from_slice(&raw).map_err(|error| Error::Source(error.to_string()))
}
fn node(statement: &mut Statement<'_>, capture: &PublicCapture, id: &str) -> Result<Option<Value>> {
    capture.charge_work(id.len() as u64)?;
    let mut rows = statement.query(params![id, MAX_ROW_BYTES as i64])?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };
    let len = admitted_row_len(capture, row.get(0)?)?;
    let digest: Vec<u8> = row.get(1)?;
    Ok(Some(check_row(len, &digest, row.get(2)?)?))
}
fn node_identity(
    statement: &mut Statement<'_>,
    capture: &PublicCapture,
    id: &str,
) -> Result<Option<(String, String)>> {
    capture.charge_work(id.len() as u64)?;
    let found: Option<(String, String)> = statement
        .query_row([id], |row| Ok((row.get(0)?, row.get(1)?)))
        .optional()?;
    if let Some((type_id, entity_id)) = &found {
        capture.charge_work((type_id.len() + entity_id.len()) as u64)?;
    }
    Ok(found)
}
fn claim_edge(
    statement: &mut Statement<'_>,
    capture: &PublicCapture,
    id: &str,
    predicate: &str,
    target: &str,
) -> Result<()> {
    capture.charge_work((id.len() + predicate.len()) as u64)?;
    let mut rows = statement.query(params![id, predicate])?;
    let first: Option<String> = rows.next()?.map(|row| row.get(0)).transpose()?;
    let second = rows.next()?.is_some();
    capture.charge_work(first.as_ref().map_or(0, String::len) as u64)?;
    if second || first.as_deref() != Some(target) {
        return Err(Error::Invalid("public D1 Claim incidence"));
    }
    Ok(())
}
fn supporting_claim(
    statement: &mut Statement<'_>,
    capture: &PublicCapture,
    claim_ref: &str,
) -> Result<Option<String>> {
    capture.charge_work(claim_ref.len() as u64)?;
    let mut rows = statement.query([claim_ref])?;
    let mut last: Option<(String, String)> = None;
    while let Some(row) = rows.next()? {
        // Python's final nodes are ordered by (source_graph,id), and its
        // claims_by_entity assignment retains the last matching Claim.
        // Select that one while streaming the existing entity index rather
        // than asking SQLite to materialize a per-lookup sorter.
        let source_graph: String = row.get(0)?;
        let id: String = row.get(1)?;
        capture.charge_work((source_graph.len() + id.len() + 16) as u64)?;
        if last
            .as_ref()
            .is_none_or(|old| (&source_graph, &id) > (&old.0, &old.1))
        {
            last = Some((source_graph, id));
        }
    }
    Ok(last.map(|(_, id)| id))
}
fn cardinality(
    statement: &mut Statement<'_>,
    capture: &PublicCapture,
    endpoint: &str,
    relation_type: &str,
    scope: Option<&str>,
    maximum: u64,
    scoped: bool,
) -> Result<()> {
    let limit = maximum
        .checked_add(1)
        .unwrap_or(u64::MAX)
        .min(i64::MAX as u64) as i64;
    capture.charge_work(
        (endpoint.len() + relation_type.len() + scope.map_or(0, str::len) + 32) as u64,
    )?;
    let mut rows = statement.query(params![
        endpoint,
        relation_type,
        if scoped { MAX_ROW_BYTES as i64 } else { limit }
    ])?;
    let mut count = 0u64;
    while let Some(row) = rows.next()? {
        if scoped {
            // Every candidate from the existing endpoint index is admitted
            // before copying/parsing its assertion context; no hidden SQL
            // JSON scan over unmetered blobs or graph-sized incidence table.
            let len = admitted_row_len(capture, row.get(0)?)?;
            let digest: Vec<u8> = row.get(1)?;
            let value = check_row(len, &digest, row.get(2)?)?;
            if member(&value, "attributes.claim_ref").as_str() != scope {
                continue;
            }
        } else {
            capture.charge_work(8)?;
        }
        count = count
            .checked_add(1)
            .ok_or(Error::Budget("public D1 cardinality count"))?;
        if count > maximum {
            return Err(Error::Invalid("public D1 scoped relation cardinality"));
        }
    }
    Ok(())
}
fn push_gap(
    capture: &PublicCapture,
    gaps: &mut Vec<Value>,
    live_bytes: &mut usize,
    id: &str,
    kind: &str,
) -> Result<()> {
    // Six bytes per input byte bounds JSON escaping (including \u00XX),
    // while the fixed allowance covers the two Value strings and map slots.
    let bytes = id
        .len()
        .checked_add(kind.len())
        .and_then(|n| n.checked_mul(6))
        .and_then(|n| n.checked_add(64))
        .ok_or(Error::Budget("public D1 semantic report bytes"))?;
    *live_bytes = live_bytes
        .checked_add(bytes)
        .filter(|n| *n <= MAX_HEADER_BYTES)
        .ok_or(Error::Budget("public D1 semantic report bytes"))?;
    capture.charge_work(bytes as u64)?;
    gaps.push(json!({"id":id,"kind":kind}));
    Ok(())
}
fn registry_entries<'a>(
    registry: &'a Value,
    key: &str,
    id: &str,
    capture: &PublicCapture,
) -> Result<BTreeMap<String, &'a Value>> {
    let rows = registry
        .get(key)
        .and_then(Value::as_array)
        .ok_or(Error::Invalid("public D1 semantic registry collection"))?;
    if rows.len() > 4096 {
        return Err(Error::Budget("public D1 semantic registry entries"));
    }
    let mut entries = BTreeMap::new();
    for row in rows {
        let name = string(row, id).ok_or(Error::Invalid("public D1 semantic registry ID"))?;
        capture.charge_work((name.len() + 32) as u64)?;
        if entries.insert(name.to_owned(), row).is_some() {
            return Err(Error::Invalid("public D1 duplicate semantic registry ID"));
        }
    }
    Ok(entries)
}
fn language_tag(value: &str) -> bool {
    let mut parts = value.split('-');
    let first = parts.next().unwrap_or("");
    let ordinary = (2..=8).contains(&first.len()) && first.bytes().all(|b| b.is_ascii_alphabetic());
    let private = matches!(first, "i" | "I" | "x" | "X");
    if !ordinary && !private {
        return false;
    }
    let mut suffixes = 0usize;
    for part in parts {
        suffixes += 1;
        if !(1..=8).contains(&part.len()) || !part.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return false;
        }
    }
    ordinary || suffixes > 0
}
fn property_field(field: &str) -> bool {
    const NODE_FIELDS: &[&str] = &[
        "id",
        "entity_id",
        "native_id",
        "source_dossier_ref",
        "source_graph",
        "kind_id",
        "type_id",
        "type_mapping.status",
        "type_mapping.source_kind_id",
        "display.title.default",
        "display.title.ru",
        "display.title.en",
        "display.kind_label.default",
        "display.summary.default",
        "display.summary.ru",
        "display.summary.en",
        "display.summary_state",
        "epistemic.authority_layer",
        "epistemic.canon_status",
        "epistemic.review_posture",
        "epistemic.confidence",
        "graph_layers",
        "view_ids",
        "source_refs",
    ];
    if NODE_FIELDS.contains(&field) {
        return true;
    }
    let mut parts = field.split('.');
    let prefix = parts.next();
    let display = parts.next();
    let language = parts.next();
    if parts.next().is_none()
        && prefix == Some("display")
        && matches!(display, Some("title" | "kind_label" | "summary"))
        && language.is_some_and(|language| {
            matches!(language, "default" | "original") || language_tag(language)
        })
    {
        return true;
    }
    let Some(tail) = field
        .strip_prefix("attributes.")
        .or_else(|| field.strip_prefix("semantics."))
    else {
        return false;
    };
    (1..=128).contains(&tail.len())
        && tail.as_bytes()[0].is_ascii_alphanumeric()
        && tail
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
        && !tail
            .split('.')
            .any(|part| matches!(part, "__proto__" | "prototype" | "constructor"))
}
fn supersession_chain(
    entries: &BTreeMap<String, &Value>,
    field: &str,
    capture: &PublicCapture,
) -> Result<()> {
    for start in entries.keys() {
        let mut seen = BTreeSet::new();
        let mut current = start.as_str();
        loop {
            capture.charge_work((current.len() + 16) as u64)?;
            if !seen.insert(current) {
                return Err(Error::Invalid("public D1 registry supersession cycle"));
            }
            let Some(next) = entries
                .get(current)
                .and_then(|entry| entry.get(field))
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
            else {
                break;
            };
            if !entries.contains_key(next) {
                return Err(Error::Invalid("public D1 registry missing supersession"));
            }
            current = next;
        }
    }
    Ok(())
}
fn template_labels(value: &Value, languages: &BTreeSet<&str>) -> bool {
    let Some(labels) = value.as_object() else {
        return false;
    };
    labels.len() == languages.len()
        && languages.iter().all(|language| {
            labels
                .get(*language)
                .and_then(Value::as_str)
                .is_some_and(|label| !label.trim().is_empty() && label.chars().count() <= 256)
        })
}
fn json_integer(value: &Value) -> Option<i128> {
    // Python's current registry predicate uses isinstance(value, int), which
    // includes booleans for this comparison.
    value
        .as_bool()
        .map(|value| i128::from(u8::from(value)))
        .or_else(|| value.as_i64().map(i128::from))
        .or_else(|| value.as_u64().map(i128::from))
}
fn current_claim_template(relation: &Value, capture: &PublicCapture) -> Result<()> {
    let Some(template) = relation.get("claim_navigation_template") else {
        return Ok(());
    };
    let invalid = || Error::Invalid("public D1 claim navigation template");
    let object = template.as_object().ok_or_else(invalid)?;
    const FIELDS: &[&str] = &[
        "template_id",
        "template_version",
        "reader",
        "purpose",
        "owner_ref",
        "default_language",
        "max_output_bytes",
        "marker",
        "status_labels",
        "renderings",
    ];
    if !(FIELDS.len()..=FIELDS.len() + 1).contains(&object.len())
        || FIELDS.iter().any(|field| !object.contains_key(*field))
        || object
            .keys()
            .any(|key| !FIELDS.contains(&key.as_str()) && key.as_str() != "object_label_adapters")
    {
        return Err(invalid());
    }
    let id = template
        .get("template_id")
        .and_then(Value::as_str)
        .ok_or_else(invalid)?;
    let suffix = id
        .strip_prefix("tos.navigation-template.")
        .ok_or_else(invalid)?;
    if suffix.is_empty()
        || suffix.split(['.', '-']).any(|part| {
            part.is_empty()
                || !part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
        || template.get("reader").and_then(Value::as_str) != Some("claim-navigation-v1")
        || template.get("purpose").and_then(Value::as_str) != Some("claim-navigation-only")
        || template.get("owner_ref").and_then(Value::as_str) != Some("ToS/doctrine/HUMAN_FORMS.md")
        || !template["max_output_bytes"]
            .as_u64()
            .is_some_and(|n| (128..=16384).contains(&n))
    {
        return Err(invalid());
    }
    let version = template["template_version"]
        .as_u64()
        .filter(|n| *n > 0)
        .ok_or_else(invalid)?;
    if let Some(adapters) = object.get("object_label_adapters") {
        let adapters = adapters
            .as_array()
            .filter(|a| (1..=2).contains(&a.len()))
            .ok_or_else(invalid)?;
        let mut seen = BTreeSet::new();
        for adapter in adapters {
            let name = adapter.as_str().ok_or_else(invalid)?;
            if !matches!(
                name,
                "historical-time-source-wording-v1" | "document-catalogue-time-source-wording-v1"
            ) || !seen.insert(name)
                || version < 2
                || (name == "document-catalogue-time-source-wording-v1" && version < 3)
            {
                return Err(invalid());
            }
        }
    }
    let renderings = template["renderings"]
        .as_object()
        .filter(|r| (1..=16).contains(&r.len()))
        .ok_or_else(invalid)?;
    let statuses = template["status_labels"]
        .as_object()
        .filter(|s| s.len() == 2)
        .ok_or_else(invalid)?;
    let mut languages = BTreeSet::new();
    let mut folded = BTreeSet::new();
    for language in renderings.keys() {
        capture.charge_work((language.len() + 16) as u64)?;
        if language.len() > 64
            || !language_tag(language)
            || matches!(
                language.to_ascii_lowercase().as_str(),
                "default" | "original" | "auto"
            )
            || !folded.insert(language.to_ascii_lowercase())
        {
            return Err(invalid());
        }
        languages.insert(language.as_str());
    }
    if !template
        .get("default_language")
        .and_then(Value::as_str)
        .is_some_and(|v| languages.contains(v))
        || !template_labels(&template["marker"], &languages)
    {
        return Err(invalid());
    }
    for (key, expected) in [
        (
            "epistemic_status",
            &[
                "observed",
                "inferred",
                "reported",
                "interpreted",
                "uncertain",
                "disputed",
            ][..],
        ),
        (
            "review_status",
            &[
                "unreviewed",
                "accepted",
                "accepted_with_limits",
                "rejected",
                "ambiguous",
                "deferred",
                "superseded",
            ][..],
        ),
    ] {
        let labels = statuses
            .get(key)
            .and_then(Value::as_object)
            .filter(|v| v.len() == expected.len())
            .ok_or_else(invalid)?;
        for name in expected {
            if !labels
                .get(*name)
                .is_some_and(|value| template_labels(value, &languages))
            {
                return Err(invalid());
            }
        }
    }
    const SLOTS: &[&str] = &[
        "claim-marker",
        "subject-label",
        "predicate-label",
        "object-label",
        "declared-epistemic-status",
        "declared-review-status",
    ];
    for parts in renderings.values() {
        let parts = parts
            .as_array()
            .filter(|p| (6..=32).contains(&p.len()))
            .ok_or_else(invalid)?;
        if parts[0].as_object().is_none_or(|p| {
            p.len() != 1 || p.get("slot").and_then(Value::as_str) != Some("claim-marker")
        }) {
            return Err(invalid());
        }
        let mut seen = BTreeSet::new();
        for part in parts {
            capture.charge_work(32)?;
            let object = part
                .as_object()
                .filter(|p| p.len() == 1)
                .ok_or_else(invalid)?;
            if let Some(slot) = object.get("slot") {
                let slot = slot.as_str().ok_or_else(invalid)?;
                if !SLOTS.contains(&slot) || !seen.insert(slot) {
                    return Err(invalid());
                }
            } else if !object
                .get("literal")
                .and_then(Value::as_str)
                .is_some_and(|literal| (1..=256).contains(&literal.chars().count()))
            {
                return Err(invalid());
            }
        }
        if seen.len() != SLOTS.len() {
            return Err(invalid());
        }
    }
    Ok(())
}
/// Validate only the current selected registries used by this full public
/// build. Historical registry migration checks belong to their source owner.
pub(crate) fn validate_public_current_registries(
    capture: &PublicCapture,
    entity_bytes: &[u8],
    relation_bytes: &[u8],
) -> Result<KnowledgeRegistry> {
    const MAX_REGISTRY_BYTES: usize = 4 * 1024 * 1024;
    if entity_bytes.len() > MAX_REGISTRY_BYTES || relation_bytes.len() > MAX_REGISTRY_BYTES {
        return Err(Error::Budget("public D1 semantic registry bytes"));
    }
    let total = (entity_bytes.len() as u64)
        .checked_add(relation_bytes.len() as u64)
        .ok_or(Error::Budget("public D1 semantic registry work"))?;
    // The existing parser performs its strict selected decode/hierarchy and
    // mapping checks; the serde view below supplies the remaining current
    // owner predicates. Both finite passes use the same capture deadline/work.
    capture.charge_work(
        total
            .checked_mul(4)
            .ok_or(Error::Budget("public D1 semantic registry work"))?,
    )?;
    let registry = KnowledgeRegistry::parse(entity_bytes, relation_bytes)?;
    let entity: Value = serde_json::from_slice(entity_bytes)
        .map_err(|_| Error::Invalid("public D1 entity registry JSON"))?;
    let relation: Value = serde_json::from_slice(relation_bytes)
        .map_err(|_| Error::Invalid("public D1 relation registry JSON"))?;
    let entities = registry_entries(&entity, "types", "type_id", capture)?;
    let relations = registry_entries(&relation, "relations", "relation_type_id", capture)?;
    if let Some(presentation) = entity.get("context_presentation").filter(|v| !v.is_null()) {
        crate::knowledge_readable_context::validate_current_context_presentation(presentation)?;
    }
    let properties = entity
        .get("property_definitions")
        .and_then(Value::as_array)
        .ok_or(Error::Invalid("public D1 property registry"))?;
    let mut property_ids = BTreeSet::new();
    for definition in properties {
        capture.charge_work(64)?;
        let id = definition
            .get("property_id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or(Error::Invalid("public D1 property ID"))?;
        let path = definition
            .get("field")
            .and_then(Value::as_str)
            .ok_or(Error::Invalid("public D1 property field"))?;
        let owners = member(definition, "applies_to")
            .as_array()
            .ok_or(Error::Invalid("public D1 property owners"))?;
        if !property_ids.insert(id)
            || !property_field(path)
            || owners.iter().any(|owner| {
                owner
                    .as_str()
                    .is_none_or(|owner| !entities.contains_key(owner))
            })
        {
            return Err(Error::Invalid("public D1 property registry contract"));
        }
    }
    for entry in entities.values() {
        capture.charge_work(32)?;
        if truthy(member(entry, "abstract"))
            && member(entry, "source_mappings")
                .as_array()
                .is_some_and(|v| !v.is_empty())
        {
            return Err(Error::Invalid("public D1 abstract entity mapping"));
        }
    }
    supersession_chain(&entities, "supersedes_type_id", capture)?;
    supersession_chain(&relations, "supersedes_relation_type_id", capture)?;
    for (id, entry) in &relations {
        capture.charge_work((id.len() + 64) as u64)?;
        if truthy(member(entry, "abstract"))
            && member(entry, "source_mappings")
                .as_array()
                .is_some_and(|v| !v.is_empty())
        {
            return Err(Error::Invalid("public D1 abstract relation mapping"));
        }
        for field in ["domain_type_ids", "range_type_ids"] {
            let endpoints = member(entry, field)
                .as_array()
                .ok_or(Error::Invalid("public D1 registry endpoint list"))?;
            for endpoint in endpoints {
                let endpoint = endpoint
                    .as_str()
                    .ok_or(Error::Invalid("public D1 registry endpoint type"))?;
                capture.charge_work(endpoint.len() as u64)?;
                if !entities.contains_key(endpoint) {
                    return Err(Error::Invalid("public D1 registry endpoint type"));
                }
            }
        }
        for parent in strings(entry, "parent_relation_type_ids") {
            if !relations.contains_key(parent) {
                return Err(Error::Invalid("public D1 relation parent"));
            }
        }
        if let Some(inverse) = string(entry, "inverse_relation_type_id") {
            let counterpart = relations
                .get(inverse)
                .ok_or(Error::Invalid("public D1 relation inverse"))?;
            if string(counterpart, "inverse_relation_type_id") != Some(id.as_str()) {
                return Err(Error::Invalid("public D1 relation inverse reciprocity"));
            }
        }
        for (minimum, maximum) in [
            ("per_subject_min", "per_subject_max"),
            ("per_object_min", "per_object_max"),
        ] {
            if let (Some(low), Some(high)) = (
                json_integer(member(entry, &format!("cardinality.{minimum}"))),
                json_integer(member(entry, &format!("cardinality.{maximum}"))),
            ) {
                if low > high {
                    return Err(Error::Invalid("public D1 relation cardinality registry"));
                }
            }
        }
    }
    current_claim_template(&relation, capture)?;
    Ok(registry)
}
fn is_a(
    type_id: &str,
    allowed: &Value,
    entities: &BTreeMap<String, &Value>,
    capture: &PublicCapture,
) -> Result<bool> {
    let Some(allowed) = allowed.as_array() else {
        return Ok(false);
    };
    capture.charge_work(type_id.len() as u64)?;
    let mut current = vec![type_id.to_owned()];
    let mut seen = std::collections::BTreeSet::new();
    while let Some(id) = current.pop() {
        capture.charge_work((id.len() + allowed.len() + 16) as u64)?;
        if allowed
            .iter()
            .any(|item| item.as_str() == Some(id.as_str()))
        {
            return Ok(true);
        }
        if !seen.insert(id.clone()) {
            continue;
        }
        if let Some(entry) = entities.get(&id) {
            capture.charge_work(
                strings(entry, "parent_type_ids")
                    .map(str::len)
                    .sum::<usize>() as u64,
            )?;
            current.extend(strings(entry, "parent_type_ids").map(str::to_owned));
        }
    }
    Ok(false)
}
fn mapped_node_type<'a>(registry: &'a KnowledgeRegistry, value: &Value) -> &'a str {
    registry
        .entity(
            string(value, "source_graph").unwrap_or(""),
            string(value, "type_mapping.source_kind_id").unwrap_or(""),
        )
        .type_id
}
fn mapped_relation_type<'a>(registry: &'a KnowledgeRegistry, value: &Value) -> &'a str {
    registry
        .relation(
            string(value, "source_graph").unwrap_or(""),
            string(value, "predicate_mapping.source_predicate_id").unwrap_or(""),
            "edge",
        )
        .type_id
}
fn exact_ref(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    if object.len() != 3
        || !object.contains_key("id")
        || !object.contains_key("version")
        || !object.contains_key("digest")
        || value
            .get("id")
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
        || !member(value, "version")
            .as_u64()
            .is_some_and(|number| (1..=9_007_199_254_740_991).contains(&number))
    {
        return false;
    }
    value
        .get("digest")
        .and_then(Value::as_str)
        .is_some_and(|digest| {
            digest.len() == 71
                && digest.starts_with("sha256:")
                && digest[7..]
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
}
fn exact_record_digest(capture: &PublicCapture, value: &Value) -> Result<String> {
    let raw =
        serde_json::to_vec(value).map_err(|_| Error::Invalid("public D1 exact record JSON"))?;
    if raw.len() > MAX_ROW_BYTES {
        return Err(Error::Budget("public D1 exact record bytes"));
    }
    capture.charge_work(raw.len() as u64)?;
    Ok(Digest256::of_bytes(&raw).to_hex())
}
fn metadata_history_refs<'a>(capture: &PublicCapture, node: &'a Value) -> Result<&'a [Value]> {
    let history = member(node, "attributes.record_history");
    let record_value = member(node, "attributes.source_record");
    let (Some(history), Some(record)) = (history.as_object(), record_value.as_object()) else {
        return Err(Error::Invalid("public D1 metadata history envelope"));
    };
    const FIELDS: [&str; 10] = [
        "schema_version",
        "status",
        "reason",
        "record_id",
        "current_ref",
        "refs",
        "provenance",
        "grants_current_use",
        "performs_assessment",
        "writes_to_source",
    ];
    if history.len() != FIELDS.len()
        || FIELDS.iter().any(|name| !history.contains_key(*name))
        || string(member(node, "attributes"), "record_history.schema_version")
            != Some("tos_metadata_record_history_v1")
        || string(member(node, "attributes"), "record_history.status") != Some("available")
        || !string(member(node, "attributes"), "record_history.reason")
            .is_some_and(|reason| reason.len() <= 256)
        || [
            "grants_current_use",
            "performs_assessment",
            "writes_to_source",
        ]
        .iter()
        .any(|name| history.get(*name) != Some(&Value::Bool(false)))
        || history
            .get("provenance")
            .and_then(Value::as_object)
            .is_none_or(|value| value.is_empty())
    {
        return Err(Error::Invalid("public D1 metadata history fields"));
    }
    let identity = match string(record_value, "schema_version") {
        Some("tos_canonical_node_v1") => "node_id",
        Some("tos_scholarly_composite_witness_v1") => "composite_id",
        Some("tos_artifact_source_witness_v1" | "tos_artifact_source_witness_v2") => "artifact_id",
        _ => "record_id",
    };
    let id =
        string(record_value, identity).ok_or(Error::Invalid("public D1 metadata record ID"))?;
    let version = member(record_value, "record_version")
        .as_u64()
        .filter(|version| (1..=9_007_199_254_740_991).contains(version))
        .ok_or(Error::Invalid("public D1 metadata record version"))?;
    if string(node, "entity_id") != Some(id)
        || string(member(node, "attributes"), "record_history.record_id") != Some(id)
    {
        return Err(Error::Invalid("public D1 metadata record identity"));
    }
    let current = json!({
        "id":id,"version":version,
        "digest":format!("sha256:{}",exact_record_digest(capture, member(node, "attributes.source_record"))?)
    });
    let declared = member(node, "attributes.record_history.current_ref");
    if !exact_ref(declared)
        || exact_record_digest(capture, &current)? != exact_record_digest(capture, declared)?
    {
        return Err(Error::Invalid("public D1 metadata current ref"));
    }
    let refs = history
        .get("refs")
        .and_then(Value::as_array)
        .filter(|refs| (1..=129).contains(&refs.len()))
        .ok_or(Error::Invalid("public D1 metadata history refs"))?;
    let mut previous = None;
    for reference in refs {
        let next = member(reference, "version")
            .as_u64()
            .ok_or(Error::Invalid("public D1 metadata history version"))?;
        if !exact_ref(reference)
            || string(reference, "id") != Some(id)
            || previous.is_some_and(|old: u64| next != old.saturating_add(1))
        {
            return Err(Error::Invalid("public D1 metadata history sequence"));
        }
        previous = Some(next);
    }
    if refs.last() != Some(&current) {
        return Err(Error::Invalid("public D1 metadata history head"));
    }
    Ok(refs)
}
fn endpoint_contract(
    from: Option<&(String, String)>,
    to: Option<&(String, String)>,
    relation_type: &str,
    fallback_relation: &str,
    relation_entry: &Value,
    entities: &BTreeMap<String, &Value>,
    capture: &PublicCapture,
) -> Result<()> {
    if relation_type == fallback_relation {
        return Ok(());
    }
    let (Some(from), Some(to)) = (from, to) else {
        return Err(Error::Invalid("public D1 unresolved normalized endpoint"));
    };
    if !is_a(
        &from.0,
        member(relation_entry, "domain_type_ids"),
        entities,
        capture,
    )? || !is_a(
        &to.0,
        member(relation_entry, "range_type_ids"),
        entities,
        capture,
    )? {
        return Err(Error::Invalid("public D1 semantic endpoint type"));
    }
    Ok(())
}
fn property_type(value: &Value, kind: &str) -> bool {
    match kind {
        "string" => value.is_string(),
        "boolean" => value.is_boolean(),
        "number" => value.is_number(),
        "string-array" => value
            .as_array()
            .is_some_and(|items| items.iter().all(Value::is_string)),
        _ => false,
    }
}
fn check_node(
    value: &Value,
    registry: &KnowledgeRegistry,
    entities: &BTreeMap<String, &Value>,
    properties: &[Value],
    capture: &PublicCapture,
) -> Result<()> {
    let id = string(value, "id").ok_or(Error::Invalid("public D1 node ID"))?;
    let type_id = string(value, "type_id").ok_or(Error::Invalid("public D1 node type"))?;
    let entry = entities
        .get(type_id)
        .ok_or(Error::Invalid("public D1 unregistered node type"))?;
    if truthy(member(entry, "abstract"))
        || type_id != mapped_node_type(registry, value)
        || string(value, "type_mapping.status")
            != Some(if type_id == registry.fallback_entity_type_id() {
                "unmapped"
            } else {
                "mapped"
            })
        || string(value, "type_mapping.source_kind_id").is_none()
        || string(value, "entity_id").is_none()
        || strings(value, "source_refs").next().is_none()
    {
        return Err(Error::Invalid("public D1 node semantic mapping/source"));
    }
    capture.charge_work(properties.len() as u64)?;
    for definition in properties {
        let inherited = truthy(member(definition, "inherited"));
        let applies = if inherited {
            is_a(type_id, member(definition, "applies_to"), entities, capture)?
        } else {
            strings(definition, "applies_to").any(|owner| owner == type_id)
        };
        if !applies {
            continue;
        }
        let path = string(definition, "field").ok_or(Error::Invalid("public D1 property field"))?;
        match field(value, path) {
            None | Some(Value::Null) if truthy(member(definition, "required")) => {
                return Err(Error::Invalid("public D1 required node property"));
            }
            None | Some(Value::Null) => continue,
            Some(field) if property_type(field, string(definition, "value_type").unwrap_or("")) => {
                ()
            }
            Some(_) => return Err(Error::Invalid("public D1 node property type")),
        }
    }
    match type_id {
        "tos.entity.temporal-assertion"
            if !matches!(
                string(value, "semantics.time.normalization_status"),
                Some("structured-source" | "source-literal-parsed" | "source-literal-unparsed")
            ) =>
        {
            return Err(Error::Invalid("public D1 temporal source value"));
        }
        "tos.entity.place" if string(value, "semantics.space.kind") != Some("place-identity") => {
            return Err(Error::Invalid("public D1 place identity"));
        }
        "tos.entity.navigation-region"
            if string(value, "semantics.space.kind") != Some("navigation-region") =>
        {
            return Err(Error::Invalid("public D1 navigation region marker"));
        }
        _ => (),
    }
    if id.len() > 4096 {
        return Err(Error::Budget("public D1 semantic node ID"));
    }
    Ok(())
}

/// A valid report is produced only after every graph row and assertion is
/// checked. Invalid rows refuse the disposable build before any completion
/// marker; they are never converted to a fabricated `valid: true` packet.
pub(crate) fn validate_public_semantics(
    stage: &mut KnowledgeStage<'_>,
    capture: &PublicCapture,
    registry: &KnowledgeRegistry,
    entity_bytes: &[u8],
    relation_bytes: &[u8],
) -> Result<Value> {
    if !stage.public_build() {
        return Err(Error::Invalid("public D1 semantic registry binding"));
    }
    capture.charge_work(
        (entity_bytes.len() as u64)
            .checked_add(relation_bytes.len() as u64)
            .and_then(|n| n.checked_mul(2))
            .ok_or(Error::Budget("public D1 semantic registry work"))?,
    )?;
    if Digest256::of_bytes(entity_bytes).to_hex() != registry.entity_sha256
        || Digest256::of_bytes(relation_bytes).to_hex() != registry.relation_sha256
    {
        return Err(Error::Invalid("public D1 semantic registry binding"));
    }
    let entity: Value = serde_json::from_slice(entity_bytes)
        .map_err(|_| Error::Invalid("public D1 entity registry JSON"))?;
    let relation: Value = serde_json::from_slice(relation_bytes)
        .map_err(|_| Error::Invalid("public D1 relation registry JSON"))?;
    let entities = registry_entries(&entity, "types", "type_id", capture)?;
    let relations = registry_entries(&relation, "relations", "relation_type_id", capture)?;
    let properties = entity
        .get("property_definitions")
        .and_then(Value::as_array)
        .ok_or(Error::Invalid("public D1 property registry"))?;
    let fallback_relation = string(&relation, "fallback_relation_type_id")
        .ok_or(Error::Invalid("public D1 relation fallback"))?;
    let mut registered_nodes = 0u64;
    let mut unmapped_nodes = 0u64;
    let mut registered_relations = 0u64;
    let mut unmapped_relations = 0u64;
    let mut claim_count = 0u64;
    let mut cross_layer = 0u64;
    let mut relation_gaps = Vec::<Value>::new();
    let mut claim_gaps = Vec::<Value>::new();
    let mut live_gap_bytes = 256usize;
    stage.with_connection(WritePhase::Finalize, |db| {
        let mut node_lookup = db.prepare("SELECT payload_len,payload_sha256,CASE WHEN payload_len BETWEEN 0 AND ?2 AND length(payload)=payload_len THEN payload ELSE NULL END FROM knowledge_nodes WHERE id=?1")?;
        let mut identity_lookup = db.prepare("SELECT type_id,entity_id FROM knowledge_nodes WHERE id=?1")?;
        let mut claim_edges = db.prepare("SELECT to_id FROM knowledge_relations WHERE from_id=?1 AND relation_type_id=?2 LIMIT 2")?;
        let mut supporting_lookup = db.prepare("SELECT source_graph,id FROM knowledge_nodes WHERE entity_id=?1 AND type_id='tos.entity.claim'")?;
        // Existing endpoint indexes carry the bounded checks. A scoped check
        // reads each candidate under the cumulative work budget; no private
        // graph-sized TEMP table, sorter, journal or Rust map is created.
        let mut subject_plain = db.prepare("SELECT 1 FROM knowledge_relations WHERE from_id=?1 AND relation_type_id=?2 LIMIT ?3")?;
        let mut object_plain = db.prepare("SELECT 1 FROM knowledge_relations WHERE to_id=?1 AND relation_type_id=?2 LIMIT ?3")?;
        let mut subject_scoped = db.prepare("SELECT payload_len,payload_sha256,CASE WHEN payload_len BETWEEN 0 AND ?3 AND length(payload)=payload_len THEN payload ELSE NULL END FROM knowledge_relations WHERE from_id=?1 AND relation_type_id=?2")?;
        let mut object_scoped = db.prepare("SELECT payload_len,payload_sha256,CASE WHEN payload_len BETWEEN 0 AND ?3 AND length(payload)=payload_len THEN payload ELSE NULL END FROM knowledge_relations WHERE to_id=?1 AND relation_type_id=?2")?;
        {
            let mut statement = db.prepare("SELECT payload_len,payload_sha256,CASE WHEN payload_len BETWEEN 0 AND ?1 AND length(payload)=payload_len THEN payload ELSE NULL END,type_id,entity_id,source_graph FROM knowledge_nodes ORDER BY source_order")?;
            let mut rows = statement.query([MAX_ROW_BYTES as i64])?;
            while let Some(row) = rows.next()? {
                let len = admitted_row_len(capture, row.get(0)?)?;
                let digest: Vec<u8> = row.get(1)?;
                let value = check_row(len, &digest, row.get(2)?)?;
                let stored_type: String = row.get(3)?;
                let stored_entity: Option<String> = row.get(4)?;
                let stored_source: String = row.get(5)?;
                capture.charge_work((stored_type.len() + stored_entity.as_ref().map_or(0, String::len) + stored_source.len()) as u64)?;
                if string(&value, "type_id") != Some(stored_type.as_str())
                    || string(&value, "entity_id") != stored_entity.as_deref()
                    || string(&value, "source_graph") != Some(stored_source.as_str())
                {
                    return Err(Error::Invalid("public D1 indexed node identity"));
                }
                check_node(&value, registry, &entities, properties, capture)?;
                let type_id = string(&value, "type_id").unwrap_or("");
                registered_nodes += u64::from(entities.contains_key(type_id));
                unmapped_nodes += u64::from(type_id == registry.fallback_entity_type_id());
                if type_id == "tos.entity.claim" {
                    claim_count += 1;
                    let id = string(&value, "id").ok_or(Error::Invalid("public D1 Claim ID"))?;
                    let claim = member(&value, "semantics.claim");
                    for (predicate, target) in [
                        ("tos.relation.has-subject", "subject_node_id"),
                        ("tos.relation.has-object", "object_node_id"),
                    ] {
                        let target = string(claim, target).ok_or(Error::Invalid("public D1 Claim endpoint"))?;
                        claim_edge(&mut claim_edges, capture, id, predicate, target)?;
                    }
                    let subject = string(claim, "subject_node_id").ok_or(Error::Invalid("public D1 Claim subject"))?;
                    let object = string(claim, "object_node_id").ok_or(Error::Invalid("public D1 Claim object"))?;
                    let relation_type = string(claim, "relation_type_id").unwrap_or(fallback_relation);
                    let relation_entry = relations.get(relation_type).ok_or(Error::Invalid("public D1 Claim relation type"))?;
                    let left = node_identity(&mut identity_lookup, capture, subject)?;
                    let right = node_identity(&mut identity_lookup, capture, object)?;
                    endpoint_contract(left.as_ref(), right.as_ref(), relation_type, fallback_relation, relation_entry, &entities, capture)?;
                    let mut evidence = strings(claim, "evidence_node_ids").peekable();
                    if evidence.peek().is_none() {
                        push_gap(capture, &mut claim_gaps, &mut live_gap_bytes, id, "claim-evidence-not-projected")?;
                    }
                    for evidence_id in evidence {
                        if node_identity(&mut identity_lookup, capture, evidence_id)?.is_none() {
                            return Err(Error::Invalid("public D1 unresolved Claim evidence"));
                        }
                    }
                }
            }
        }
        {
            let mut statement = db.prepare("SELECT payload_len,payload_sha256,CASE WHEN payload_len BETWEEN 0 AND ?1 AND length(payload)=payload_len THEN payload ELSE NULL END,relation_type_id,from_id,to_id,source_graph FROM knowledge_relations ORDER BY source_order")?;
            let mut rows = statement.query([MAX_ROW_BYTES as i64])?;
            while let Some(row) = rows.next()? {
                let len = admitted_row_len(capture, row.get(0)?)?;
                let digest: Vec<u8> = row.get(1)?;
                let value = check_row(len, &digest, row.get(2)?)?;
                let stored_type: String = row.get(3)?;
                let stored_from: String = row.get(4)?;
                let stored_to: String = row.get(5)?;
                let stored_source: String = row.get(6)?;
                capture.charge_work((stored_type.len() + stored_from.len() + stored_to.len() + stored_source.len()) as u64)?;
                if string(&value, "relation_type_id") != Some(stored_type.as_str())
                    || string(&value, "from_id") != Some(stored_from.as_str())
                    || string(&value, "to_id") != Some(stored_to.as_str())
                    || string(&value, "source_graph") != Some(stored_source.as_str())
                {
                    return Err(Error::Invalid("public D1 indexed relation identity"));
                }
                let id = string(&value, "id").ok_or(Error::Invalid("public D1 relation ID"))?;
                let relation_type = string(&value, "relation_type_id").ok_or(Error::Invalid("public D1 relation type"))?;
                let entry = relations.get(relation_type).ok_or(Error::Invalid("public D1 unregistered relation type"))?;
                registered_relations += 1;
                unmapped_relations += u64::from(relation_type == fallback_relation);
                cross_layer += u64::from(string(&value, "source_graph") == Some("semantic-interchange"));
                if truthy(member(entry, "abstract"))
                    || relation_type != mapped_relation_type(registry, &value)
                    || string(&value, "predicate_mapping.status") != Some(if relation_type == fallback_relation { "unmapped" } else { "mapped" })
                    || string(&value, "predicate_mapping.source_predicate_id").is_none()
                    || (truthy(member(entry, "evidence_required")) && strings(&value, "source_refs").next().is_none())
                    || (member(entry, "review_requirement").as_str() != Some("none") && string(&value, "epistemic.review_posture").is_none())
                {
                    return Err(Error::Invalid("public D1 relation semantic mapping/evidence"));
                }
                if string(&value, "epistemic.review_posture") == Some("not-recorded") {
                    push_gap(capture, &mut relation_gaps, &mut live_gap_bytes, id, "review-not-recorded")?;
                }
                let from_id = string(&value, "from_id").ok_or(Error::Invalid("public D1 relation from"))?;
                let to_id = string(&value, "to_id").ok_or(Error::Invalid("public D1 relation to"))?;
                let left = node_identity(&mut identity_lookup, capture, from_id)?;
                let right = node_identity(&mut identity_lookup, capture, to_id)?;
                endpoint_contract(left.as_ref(), right.as_ref(), relation_type, fallback_relation, entry, &entities, capture)?;
                let claim_ref = string(&value, "attributes.claim_ref").unwrap_or("");
                let scope = if string(entry, "assertion_mode") == Some("reified-claim") {
                    member(&value, "attributes.claim_ref").as_str()
                } else {
                    None
                };
                for (endpoint, field, plain, scoped) in [
                    (from_id, "per_subject_max", &mut subject_plain, &mut subject_scoped),
                    (to_id, "per_object_max", &mut object_plain, &mut object_scoped),
                ] {
                    if let Some(maximum) = member(entry, &format!("cardinality.{field}")).as_u64() {
                        let assertion_scoped = string(entry, "assertion_mode") == Some("reified-claim");
                        cardinality(if assertion_scoped { scoped } else { plain }, capture, endpoint, relation_type, scope, maximum, assertion_scoped)?;
                    }
                }
                if string(entry, "assertion_mode") == Some("reified-claim") {
                    let supporting = supporting_claim(&mut supporting_lookup, capture, claim_ref)?.ok_or(Error::Invalid("public D1 unresolved supporting Claim"))?;
                    if left.as_ref().is_some_and(|left| left.0 != "tos.entity.claim") {
                        let claim = node(&mut node_lookup, capture, &supporting)?.ok_or(Error::Invalid("public D1 missing supporting Claim"))?;
                        if left.as_ref().map(|item| item.1.as_str()) != string(&claim, "semantics.claim.subject_entity_id")
                            || right.as_ref().map(|item| item.1.as_str()) != string(&claim, "semantics.claim.object_entity_id") {
                            return Err(Error::Invalid("public D1 supporting Claim endpoints"));
                        }
                    }
                }
                if relation_type == "tos.relation.projects"
                    && left.as_ref().map(|item| &item.1) != right.as_ref().map(|item| &item.1) {
                    return Err(Error::Invalid("public D1 projection entity identity"));
                }
                if relation_type == "tos.relation.same-as" {
                    let left = left.as_ref().ok_or(Error::Invalid("public D1 same-as left"))?;
                    let right = right.as_ref().ok_or(Error::Invalid("public D1 same-as right"))?;
                    if !(is_a(&left.0, &Value::Array(vec![Value::String(right.0.clone())]), &entities, capture)?
                        || is_a(&right.0, &Value::Array(vec![Value::String(left.0.clone())]), &entities, capture)?)
                        || !matches!(string(&value, "epistemic.review_posture"), Some("accepted" | "verified" | "reviewed_equivalence"))
                        || strings(&value, "source_refs").next().is_none()
                    {
                        return Err(Error::Invalid("public D1 same-as evidence/review"));
                    }
                    let review_id = string(&value, "attributes.review_node_id").ok_or(Error::Invalid("public D1 same-as review ID"))?;
                    let review = node(&mut node_lookup, capture, review_id)?.ok_or(Error::Invalid("public D1 same-as review"))?;
                    let supporting = supporting_claim(&mut supporting_lookup, capture, claim_ref)?.ok_or(Error::Invalid("public D1 same-as Claim"))?;
                    let claim_node = node(&mut node_lookup, capture, &supporting)?
                        .ok_or(Error::Invalid("public D1 same-as Claim node"))?;
                    let claim = member(&claim_node, "semantics.claim");
                    let pair = [left.1.as_str(), right.1.as_str()];
                    let expected = [string(claim, "subject_entity_id"), string(claim, "object_entity_id")];
                    if !is_a(string(&review, "type_id").unwrap_or(""), &json!(["tos.entity.review"]), &entities, capture)?
                        || string(&review, "attributes.claim_ref") != string(claim, "claim_id")
                        || member(&review, "attributes.claim_version") != member(claim, "claim_version")
                        || string(&review, "attributes.decision") != Some("accepted")
                        || string(claim, "relation_type_id") != Some(relation_type)
                        || member(claim, "claim_version").is_null()
                        || strings(claim, "evidence_node_ids").next().is_none()
                        || !((expected[0] == Some(pair[0]) && expected[1] == Some(pair[1]))
                            || (expected[0] == Some(pair[1]) && expected[1] == Some(pair[0])))
                    {
                        return Err(Error::Invalid("public D1 same-as exact Claim"));
                    }
                    for evidence in strings(claim, "evidence_node_ids") {
                        let type_id = node_identity(&mut identity_lookup, capture, evidence)?.ok_or(Error::Invalid("public D1 same-as evidence"))?.0;
                        if !is_a(&type_id, &json!(["tos.entity.evidence"]), &entities, capture)? {
                            return Err(Error::Invalid("public D1 same-as evidence type"));
                        }
                    }
                }
                if relation_type == "tos.relation.promotion-basis-version" {
                    let left = node(&mut node_lookup, capture, from_id)?.ok_or(Error::Invalid("public D1 promotion Sign"))?;
                    let right = node(&mut node_lookup, capture, to_id)?.ok_or(Error::Invalid("public D1 promotion Version"))?;
                    let candidate = member(&left, "attributes.source_record.promotion_basis.candidate");
                    let reference = member(&right, "semantics.record_version.record_ref");
                    if !exact_ref(candidate) || !exact_ref(reference)
                        || exact_record_digest(capture, candidate)? != exact_record_digest(capture, reference)? {
                        return Err(Error::Invalid("public D1 promotion basis exact version"));
                    }
                }
                if relation_type == "tos.relation.has-record-version" {
                    let right = node(&mut node_lookup, capture, to_id)?.ok_or(Error::Invalid("public D1 record Version"))?;
                    let reference = member(&right, "semantics.record_version.record_ref");
                    if string(&right, "semantics.record_version.record_kind") != Some("metadata") || !exact_ref(reference) {
                        return Err(Error::Invalid("public D1 record history reference"));
                    }
                    let left = node(&mut node_lookup, capture, from_id)?.ok_or(Error::Invalid("public D1 record source"))?;
                    let history = metadata_history_refs(capture, &left)?;
                    let reference_digest = exact_record_digest(capture, reference)?;
                    let mut found = false;
                    for candidate in history {
                        if exact_ref(candidate)
                            && exact_record_digest(capture, candidate)? == reference_digest
                        {
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        return Err(Error::Invalid("public D1 record history exact member"));
                    }
                }
            }
        }
        Ok(())
    })?;
    relation_gaps.extend(claim_gaps);
    capture.charge_work((live_gap_bytes as u64).saturating_add(256))?;
    let mut report = serde_json::Map::new();
    report.insert("valid".into(), Value::Bool(true));
    report.insert("violations".into(), Value::Array(Vec::new()));
    for (name, count) in [
        ("registered_node_count", registered_nodes),
        ("unmapped_node_count", unmapped_nodes),
        ("registered_relation_count", registered_relations),
        ("unmapped_relation_count", unmapped_relations),
        ("claim_contract_count", claim_count),
        ("cross_layer_relation_count", cross_layer),
    ] {
        report.insert(name.into(), Value::from(count));
    }
    report.insert("gaps".into(), Value::Array(relation_gaps));
    Ok(Value::Object(report))
}
