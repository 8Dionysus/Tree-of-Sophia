//! Exact maintained catalog row contributions and transaction-local rendering.
//! Reads the prepared catalog index only; no graph scan, publication or admission.
use crate::{Error, Result};
use rusqlite::{Transaction, params};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use tos_foundation::{CanonicalProfile, Digest256, JsonLimits, JsonMode, JsonString, JsonValue};
const NODE: &str = "node";
const RELATION: &str = "relation";
const SCHEMA: &str = "tos_knowledge_catalog_v1";
pub const PROJECTOR_VERSION: &str = "tos-exact-catalog-contributions-v1";
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceOrderProfile {
    #[serde(rename = "owner-sequence-v1")]
    OwnerSequence,
    #[serde(rename = "source-graph-id-v1")]
    SourceGraphId,
}
impl SourceOrderProfile {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OwnerSequence => "owner-sequence-v1",
            Self::SourceGraphId => "source-graph-id-v1",
        }
    }
}
#[derive(Clone, Debug)]
pub struct CatalogInputs {
    pub header: JsonValue,
    pub entity_registry: JsonValue,
    pub relation_registry: JsonValue,
    pub lenses: Vec<JsonValue>,
    pub source_order_profile: SourceOrderProfile,
}
impl CatalogInputs {
    pub fn validate(&self) -> Result<()> {
        if self.header.as_object().is_none()
            || self.header.object_get("nodes").is_some()
            || self.header.object_get("relations").is_some()
        {
            return Err(Error::Invalid(
                "catalog header must exclude row collections",
            ));
        }
        Ok(())
    }
    pub fn binding(&self) -> Result<String> {
        self.binding_with_owned_state(None)
    }
    pub(crate) fn binding_with_owned_state(
        &self,
        state: Option<&crate::d1_public_capture::CreationState<'_>>,
    ) -> Result<String> {
        self.validate()?;
        if let Some(state) = state {
            let literal_bytes = [PROJECTOR_VERSION, self.source_order_profile.as_str()]
                .iter()
                .try_fold(0usize, |n, text| {
                    n.checked_add(
                        text.encode_utf16().count().max(4) * 2 * std::mem::size_of::<u16>(),
                    )
                    .and_then(|n| n.checked_add(text.len()))
                    .ok_or(Error::Budget("owned catalog binding literals"))
                })?;
            state.retain(
                self.lenses
                    .len()
                    .checked_mul(std::mem::size_of::<JsonValue>())
                    .and_then(|n| n.checked_add(6 * std::mem::size_of::<JsonValue>()))
                    .and_then(|n| n.checked_add(literal_bytes))
                    .ok_or(Error::Budget("owned catalog binding containers"))?,
            )?;
            for value in std::iter::once(&self.entity_registry)
                .chain(std::iter::once(&self.relation_registry))
                .chain(self.lenses.iter())
                .chain(self.header.object_get("normalization_binding").into_iter())
            {
                let upper = value
                    .retained_storage_bytes()
                    .map_err(|_| Error::Budget("owned catalog binding clones"))?;
                state.retain(upper)?;
                state.charge_work(upper)?;
            }
        }
        let binding = JsonValue::Array(vec![
            JsonValue::String(JsonString::from_utf8(PROJECTOR_VERSION)),
            JsonValue::String(JsonString::from_utf8(self.source_order_profile.as_str())),
            self.entity_registry.clone(),
            self.relation_registry.clone(),
            JsonValue::Array(self.lenses.clone()),
            self.header
                .object_get("normalization_binding")
                .cloned()
                .unwrap_or(JsonValue::Null),
        ]);
        if let Some(state) = state {
            let limits = JsonLimits::new(16 * 1024 * 1024, 96, 1_000_000, 4096)
                .map_err(|_| Error::Budget("owned catalog binding limits"))?;
            let encoded = state.encode_foundation_canonical_with_limits(&binding, limits)?;
            state.retain(64)?;
            state.charge_work(encoded.len())?;
            Ok(Digest256::of_bytes(&encoded).to_hex())
        } else {
            catalog_owner_digest(&binding, 16 * 1024 * 1024)
        }
    }
    pub fn header_digest(&self) -> Result<String> {
        catalog_owner_digest(&self.header, 16 * 1024 * 1024)
    }
}
pub fn encoded_owner(value: &JsonValue, cap: usize) -> Result<String> {
    let limits = JsonLimits::new(cap, 96, 1_000_000, 4096)
        .map_err(|_| Error::Budget("catalog owner JSON limits"))?;
    String::from_utf8(
        tos_foundation::emit_python_compact_json(value, limits)
            .map_err(|_| Error::Budget("catalog owner JSON bytes"))?,
    )
    .map_err(|_| Error::Invalid("catalog owner UTF-8"))
}
pub fn encoded_canonical_owner(value: &JsonValue, cap: usize) -> Result<String> {
    let limits = JsonLimits::new(cap, 96, 1_000_000, 4096)
        .map_err(|_| Error::Budget("catalog owner JSON limits"))?;
    String::from_utf8(
        tos_foundation::canonical_bytes_v1(value, CanonicalProfile::SourceRecordDigestV1, limits)
            .map_err(|_| Error::Budget("catalog owner JSON bytes"))?,
    )
    .map_err(|_| Error::Invalid("catalog owner UTF-8"))
}
pub fn catalog_owner_digest(value: &JsonValue, cap: usize) -> Result<String> {
    Ok(Digest256::of_bytes(encoded_canonical_owner(value, cap)?.as_bytes()).to_hex())
}
fn owner_view(value: &JsonValue, cap: usize) -> Result<Value> {
    serde_json::from_str(&encoded_owner(value, cap)?)
        .map_err(|_| Error::Invalid("catalog owner view JSON"))
}
fn owner_from(value: &Value, cap: usize) -> Result<JsonValue> {
    let raw = serde_json::to_string(value).map_err(|_| Error::Invalid("catalog owner JSON"))?;
    if raw.len() > cap {
        return Err(Error::Budget("catalog owner JSON bytes"));
    }
    tos_foundation::parse_json(
        raw.as_bytes(),
        JsonMode::PublishedStrict,
        JsonLimits::new(cap, 96, 1_000_000, 4096)
            .map_err(|_| Error::Budget("catalog owner limits"))?,
    )
    .map(|d| d.into_root())
    .map_err(|_| Error::Invalid("catalog owner JSON"))
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CatalogOrder {
    OwnerSequence(u64),
    SourceGraphId(String, String),
}
#[derive(Clone, Debug)]
pub struct CatalogRow {
    pub kind: String,
    pub id: String,
    pub source_order: CatalogOrder,
    pub item: JsonValue,
}
pub type Counts = BTreeMap<Vec<String>, i64>;
pub type Posts = Vec<(Vec<String>, Value, i64)>;
#[derive(Clone, Copy, Debug)]
pub struct CatalogRenderLimits {
    pub max_output_entries: u64,
    pub max_aggregate_bytes: usize,
    pub max_catalog_bytes: usize,
}
impl Default for CatalogRenderLimits {
    fn default() -> Self {
        Self {
            max_output_entries: 100_000,
            max_aggregate_bytes: 16 * 1024 * 1024,
            max_catalog_bytes: 16 * 1024 * 1024,
        }
    }
}
pub fn encoded(value: &Value) -> Result<String> {
    let raw = serde_json::to_vec(value).map_err(|_| Error::Invalid("catalog JSON"))?;
    let doc = tos_foundation::parse_json(
        &raw,
        tos_foundation::JsonMode::PublishedStrict,
        tos_foundation::JsonLimits::default(),
    )
    .map_err(|_| Error::Invalid("catalog JSON"))?;
    let bytes = tos_foundation::canonical_bytes_v1(
        doc.root(),
        tos_foundation::CanonicalProfile::SourceRecordDigestV1,
        tos_foundation::JsonLimits::default(),
    )
    .map_err(|_| Error::Invalid("catalog JSON"))?;
    String::from_utf8(bytes).map_err(|_| Error::Invalid("catalog UTF-8"))
}
pub fn catalog_digest(value: &Value) -> Result<String> {
    Ok(Digest256::of_bytes(encoded(value)?.as_bytes()).to_hex())
}
pub fn order_key(value: &CatalogOrder) -> Result<Vec<u8>> {
    match value {
        CatalogOrder::OwnerSequence(n) if *n < 1u64 << 63 => {
            let mut out = vec![b'I'];
            out.extend(n.to_be_bytes());
            Ok(out)
        }
        CatalogOrder::SourceGraphId(a, b) => {
            let mut out = vec![b'T'];
            for part in [a, b] {
                for byte in part.bytes() {
                    out.push(byte);
                    if byte == 0 {
                        out.push(255);
                    }
                }
                out.extend([0, 0]);
            }
            Ok(out)
        }
        _ => Err(Error::Invalid(
            "catalog order requires nonnegative 63-bit integer",
        )),
    }
}
pub fn order_value(raw: &[u8]) -> Result<CatalogOrder> {
    if raw.first() == Some(&b'I') && raw.len() == 9 {
        let n = u64::from_be_bytes(
            raw[1..]
                .try_into()
                .map_err(|_| Error::Invalid("catalog order"))?,
        );
        return Ok(CatalogOrder::OwnerSequence(n));
    }
    if raw.first() != Some(&b'T') {
        return Err(Error::Invalid("stored catalog order"));
    }
    let mut parts = Vec::new();
    let mut current = Vec::new();
    let mut i = 1;
    while i < raw.len() {
        if raw[i] != 0 {
            current.push(raw[i]);
            i += 1;
        } else if raw.get(i + 1) == Some(&255) {
            current.push(0);
            i += 2;
        } else if raw.get(i + 1) == Some(&0) {
            parts.push(
                String::from_utf8(std::mem::take(&mut current))
                    .map_err(|_| Error::Invalid("catalog order UTF-8"))?,
            );
            i += 2;
        } else {
            return Err(Error::Invalid("catalog tuple escape"));
        }
    }
    if parts.len() != 2 || !current.is_empty() {
        return Err(Error::Invalid("catalog tuple components"));
    }
    Ok(CatalogOrder::SourceGraphId(
        parts.remove(0),
        parts.remove(0),
    ))
}
struct ByteBudget {
    used: Cell<usize>,
    max: usize,
}
impl ByteBudget {
    fn new(max: usize) -> Self {
        Self {
            used: Cell::new(0),
            max,
        }
    }
    fn charge(&self, n: usize) -> Result<()> {
        let next = self
            .used
            .get()
            .checked_add(n)
            .ok_or(Error::Budget("catalog decoded bytes"))?;
        if next > self.max {
            return Err(Error::Budget("catalog decoded bytes"));
        }
        self.used.set(next);
        Ok(())
    }
}
struct CatalogView<'a> {
    db: &'a Transaction<'a>,
    counts: Counts,
    groups: BTreeMap<Vec<String>, BTreeMap<String, u64>>,
    max_entries: u64,
}
fn scalar_counts(
    view: &CatalogView<'_>,
    prefix: &[&str],
    budget: &ByteBudget,
) -> Result<BTreeMap<String, u64>> {
    let key = prefix.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let _ = budget;
    let group = view.groups.get(&key);
    Ok(group.cloned().unwrap_or_default())
}
fn top_counts(
    view: &CatalogView<'_>,
    kind: &str,
    metric: &str,
    budget: &ByteBudget,
) -> Result<BTreeMap<String, u64>> {
    scalar_counts(view, &[kind, metric], budget)
}
fn sub_counts(
    view: &CatalogView<'_>,
    kind: &str,
    metric: &str,
    a: &str,
    budget: &ByteBudget,
) -> Result<BTreeMap<String, u64>> {
    scalar_counts(view, &[kind, metric, a], budget)
}
fn first_values(
    view: &CatalogView<'_>,
    kind: &str,
    metric: &str,
    field: &str,
    limit: Option<usize>,
    budget: &ByteBudget,
) -> Result<Vec<Value>> {
    let maximum = match limit {
        Some(n) => u64::try_from(n).map_err(|_| Error::Budget("catalog SQL limit"))?,
        None => view.max_entries,
    };
    let fetch = maximum
        .checked_add(u64::from(limit.is_none()))
        .ok_or(Error::Budget("catalog SQL limit"))?;
    let bucket = encoded(&json!([kind, metric, field]))?;
    let mut stmt=view.db.prepare("SELECT CASE WHEN length(CAST(v.value AS BLOB))<=?1 THEN v.value ELSE NULL END FROM catalog_heads h JOIN catalog_atoms b ON b.atom=h.bucket JOIN catalog_atoms v ON v.atom=h.value WHERE b.value=?2 ORDER BY h.source_order,h.position LIMIT ?3")?;
    let mut rows = stmt.query(params![
        i64::try_from(budget.max).map_err(|_| Error::Budget("catalog SQL byte limit"))?,
        bucket,
        i64::try_from(fetch).map_err(|_| Error::Budget("catalog SQL limit"))?
    ])?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        if out.len() as u64 >= maximum {
            return Err(Error::Budget("catalog ordered values"));
        }
        let raw: Option<String> = row.get(0)?;
        let raw = raw.ok_or(Error::Budget("catalog aggregate atom bytes"))?;
        budget.charge(raw.len())?;
        out.push(serde_json::from_str(&raw).map_err(|_| Error::Invalid("catalog post JSON"))?);
    }
    Ok(out)
}
#[derive(Clone)]
struct Route {
    id: String,
    kinds: Vec<String>,
    predicates: Vec<String>,
    types: Vec<String>,
    relation_types: Vec<String>,
}

fn text<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or(Error::Invalid("catalog required string"))
}

fn array<'a>(v: &'a Value, key: &str) -> Result<&'a [Value]> {
    v.get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or(Error::Invalid("catalog required array"))
}

fn strings(v: &Value, key: &str) -> Result<Vec<String>> {
    array(v, key)?
        .iter()
        .map(|x| {
            x.as_str()
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .ok_or(Error::Invalid("catalog string array"))
        })
        .collect()
}

fn path<'a>(v: &'a Value, name: &str) -> Option<&'a Value> {
    name.split('.')
        .try_fold(v, |at, key| at.as_object()?.get(key))
}

fn value_kind(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(n) if !n.to_string().contains(['.', 'e', 'E']) => "integer",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn ordered_strings(v: &Value, key: &str) -> Result<Vec<String>> {
    strings(v, key)
}

fn registry_entries(registry: &Value, field: &str, key: &str) -> Result<BTreeMap<String, Value>> {
    let mut out = BTreeMap::new();
    for entry in registry
        .get(field)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|e| e.is_object())
    {
        let Some(id) = entry
            .get(key)
            .and_then(Value::as_str)
            .filter(|s| !python_strip(s).is_empty())
            .map(str::to_owned)
        else {
            continue;
        };
        out.insert(id, entry.clone());
    }
    Ok(out)
}

fn type_is_a(id: &str, allowed: &[String], entries: &BTreeMap<String, Value>) -> bool {
    let mut todo = vec![id.to_owned()];
    let mut seen = BTreeSet::new();
    while let Some(item) = todo.pop() {
        if allowed.iter().any(|x| x == &item) {
            return true;
        }
        if !seen.insert(item.clone()) {
            continue;
        }
        if let Some(parents) = entries
            .get(&item)
            .and_then(|e| e.get("parent_type_ids"))
            .and_then(Value::as_array)
        {
            todo.extend(parents.iter().filter_map(Value::as_str).map(str::to_owned));
        }
    }
    false
}

fn python_casefold(value: &str, _budget: &ByteBudget) -> Result<String> {
    tos_foundation::python_casefold_unicode16_v1(
        value,
        value.chars().count(),
        value.len().saturating_mul(3).max(1),
        value.len().saturating_mul(3).max(1),
    )
    .map_err(|_| Error::Budget("catalog facet casefold bytes"))
}
fn facet_names(vocab: &Value, kind: &str) -> Result<Vec<String>> {
    let catalog = vocab
        .get("catalog")
        .ok_or(Error::Invalid("catalog vocabulary missing"))?;
    let fields = strings(catalog, "facets")?;
    Ok(fields
        .into_iter()
        .map(|f| {
            if kind == RELATION {
                match f.as_str() {
                    "kind_id" => "predicate_id".into(),
                    "type_id" => "relation_type_id".into(),
                    "type_mapping.status" => "predicate_mapping.status".into(),
                    _ => f,
                }
            } else {
                f
            }
        })
        .collect())
}

fn attribute_allowed(field: &str) -> bool {
    let Some(suffix) = field.strip_prefix("attributes.") else {
        return false;
    };
    let bytes = suffix.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 128
        && bytes[0].is_ascii_alphanumeric()
        && bytes
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || matches!(*b, b'_' | b'.' | b'-'))
        && !suffix
            .split('.')
            .any(|part| matches!(part, "__proto__" | "prototype" | "constructor"))
}

fn form_key(key: &str) -> bool {
    if matches!(key, "default" | "original") {
        return true;
    }
    let mut parts = key.split('-');
    let first = parts.next().unwrap_or("");
    if first.eq_ignore_ascii_case("i") || first.eq_ignore_ascii_case("x") {
        let tail: Vec<&str> = parts.collect();
        return !tail.is_empty()
            && tail.iter().all(|p| {
                !p.is_empty() && p.len() <= 8 && p.bytes().all(|b| b.is_ascii_alphanumeric())
            });
    }
    (2..=8).contains(&first.len())
        && first.bytes().all(|b| b.is_ascii_alphabetic())
        && parts
            .all(|p| !p.is_empty() && p.len() <= 8 && p.bytes().all(|b| b.is_ascii_alphanumeric()))
}

fn field_catalog(
    db: &CatalogView<'_>,
    kind: &str,
    metric: &str,
    budget: &ByteBudget,
) -> Result<Value> {
    let fields = top_counts(db, kind, metric, budget)?;
    if metric == "display" {
        return Ok(Value::Array(
            fields
                .into_iter()
                .map(|(field, n)| json!({"field":field,"available_item_count":n}))
                .collect(),
        ));
    }
    let mut out = Vec::new();
    for (field, n) in fields {
        let value_types = sub_counts(db, kind, "attribute-value-type", &field, budget)?;
        let array_item_types = sub_counts(db, kind, "attribute-array-type", &field, budget)?;
        let sources: Vec<_> = sub_counts(db, kind, "attribute-source", &field, budget)?
            .into_keys()
            .collect();
        let examples = first_values(db, kind, "examples", &field, Some(5), budget)?;
        out.push(
            json!({"field":field,"item_count":n,"value_types":value_types,
            "array_item_types":array_item_types,"sources":sources,"examples":examples}),
        );
    }
    Ok(Value::Array(out))
}

fn facets(
    db: &CatalogView<'_>,
    vocab: &Value,
    kind: &str,
    max_entries: u64,
    budget: &ByteBudget,
) -> Result<Value> {
    let mut result = Map::new();
    for field in facet_names(vocab, kind)? {
        let item_counts = sub_counts(db, kind, "facet", &field, budget)?;
        let _ = max_entries;
        let mut values = first_values(db, kind, "facet-order", &field, None, budget)?;
        let mut decorated = Vec::with_capacity(values.len());
        for value in values.drain(..) {
            let s = value
                .as_str()
                .ok_or(Error::Invalid("catalog facet value"))?
                .to_owned();
            decorated.push((python_casefold(&s, budget)?, s));
        }
        decorated.sort_by(|a, b| a.0.cmp(&b.0));
        let rows = decorated
            .into_iter()
            .map(|(_, value)| {
                json!({"count":item_counts.get(&value).copied().unwrap_or(0),
            "value":value})
            })
            .collect();
        result.insert(field, Value::Array(rows));
    }
    Ok(Value::Object(result))
}

fn registry_metadata(
    registry: &Value,
    entries: &BTreeMap<String, Value>,
    type_counts: &BTreeMap<String, u64>,
    fallback_key: &str,
    kind: &str,
    node_count: u64,
    claim_counts: Option<&BTreeMap<String, u64>>,
) -> Result<Value> {
    let fallback_owned = registry
        .get(fallback_key)
        .and_then(Value::as_str)
        .map(python_strip)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            if kind == NODE {
                "tos.entity.unmapped".into()
            } else {
                "tos.relation.unmapped".into()
            }
        });
    let fallback = fallback_owned.as_str();
    let mut rows = Vec::new();
    for (id, entry) in entries {
        let mut object = entry
            .as_object()
            .ok_or(Error::Invalid("registry entry object"))?
            .clone();
        let instance = type_counts.get(id).copied().unwrap_or(0);
        if kind == NODE {
            object.insert("instance_count".into(), json!(instance));
        } else {
            let claims = claim_counts.and_then(|m| m.get(id)).copied().unwrap_or(0);
            object.insert("edge_instance_count".into(), json!(instance));
            object.insert("claim_instance_count".into(), json!(claims));
            object.insert("instance_count".into(), json!(instance + claims));
        }
        rows.push(Value::Object(object));
    }
    let mapped = node_count.saturating_sub(type_counts.get(fallback).copied().unwrap_or(0));
    let key_mapped = if kind == NODE {
        "mapped_instance_count"
    } else {
        "mapped_edge_instance_count"
    };
    let key_unmapped = if kind == NODE {
        "unmapped_instance_count"
    } else {
        "unmapped_edge_instance_count"
    };
    let mut out = Map::new();
    out.insert(
        "registry_id".into(),
        registry.get("registry_id").cloned().unwrap_or(Value::Null),
    );
    out.insert(
        "registry_version".into(),
        registry
            .get("registry_version")
            .cloned()
            .unwrap_or(Value::Null),
    );
    out.insert(
        "source_refs".into(),
        Value::Array(
            registry
                .get("source_refs")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter(|v| v.as_str().is_some_and(|s| !s.is_empty()))
                .cloned()
                .collect(),
        ),
    );
    out.insert(fallback_key.into(), Value::String(fallback.into()));
    out.insert(key_mapped.into(), json!(mapped));
    out.insert(
        key_unmapped.into(),
        json!(type_counts.get(fallback).copied().unwrap_or(0)),
    );
    out.insert("entries".into(), Value::Array(rows));
    Ok(Value::Object(out))
}

fn group_catalog(
    db: &CatalogView<'_>,
    kind: &str,
    relation_entries: &BTreeMap<String, Value>,
    budget: &ByteBudget,
) -> Result<Value> {
    let groups = top_counts(db, kind, "group", budget)?;
    let mut out = Vec::new();
    for (group, n) in groups {
        let types: Vec<String> = sub_counts(db, kind, "group-type", &group, budget)?
            .into_keys()
            .collect();
        let statuses: Vec<String> = sub_counts(db, kind, "group-status", &group, budget)?
            .into_keys()
            .collect();
        let display = first_values(db, kind, "representative", &group, Some(1), budget)?
            .into_iter()
            .next()
            .ok_or(Error::Invalid("catalog representative absent"))?;
        let mut object = Map::new();
        object.insert(
            if kind == NODE {
                "kind_id"
            } else {
                "predicate_id"
            }
            .into(),
            Value::String(group),
        );
        object.insert("display".into(), display);
        object.insert("count".into(), json!(n));
        object.insert(
            if kind == NODE {
                "type_ids"
            } else {
                "relation_type_ids"
            }
            .into(),
            json!(types),
        );
        object.insert("mapping_statuses".into(), json!(statuses));
        if kind == RELATION {
            let mut defs = Vec::new();
            for type_id in &types {
                if let Some(entry) = relation_entries.get(type_id) {
                    defs.push(json!({"relation_type_id":type_id,
                        "labels":entry.get("labels"),"definition":entry.get("definition"),
                        "domain_type_ids":entry.get("domain_type_ids"),"range_type_ids":entry.get("range_type_ids")}));
                }
            }
            object.insert("semantic_definitions".into(), Value::Array(defs));
        }
        out.push(Value::Object(object));
    }
    Ok(Value::Array(out))
}

fn route_catalog(
    db: &CatalogView<'_>,
    route_defs: &[Route],
    entity_entries: &BTreeMap<String, Value>,
    budget: &ByteBudget,
) -> Result<Value> {
    let kinds = top_counts(db, NODE, "group", budget)?;
    let mut out = Vec::new();
    for route in route_defs {
        let available_kinds: Vec<String> = route
            .kinds
            .iter()
            .filter(|k| kinds.get(*k).copied().unwrap_or(0) > 0)
            .cloned()
            .collect();
        let typed = sub_counts(db, NODE, "route-type", &route.id, budget)?;
        let legacy = sub_counts(db, RELATION, "route-predicate", &route.id, budget)?;
        let typed_relations = sub_counts(db, RELATION, "route-type", &route.id, budget)?;
        let semantic_mode = !entity_entries.is_empty() && !route.types.is_empty();
        let available = if semantic_mode {
            !typed.is_empty()
        } else {
            !available_kinds.is_empty()
        };
        let availability = if available {
            "available"
        } else {
            "not_projected"
        };
        let has_confirmation = if semantic_mode {
            !typed_relations.is_empty()
        } else {
            !legacy.is_empty()
        };
        let expects = if semantic_mode {
            !route.relation_types.is_empty()
        } else {
            !route.predicates.is_empty()
        };
        let readiness = if !available {
            "not_projected"
        } else if expects && !has_confirmation {
            "kind_only"
        } else {
            "confirmed"
        };
        let note = if !available {
            "No trustworthy nodes of these kinds are projected; the backend will not synthesize them from unrelated predicates."
        } else if readiness == "kind_only" {
            "Kinds are projected, but no confirming relation currently establishes the requested contextual role."
        } else if !route.predicates.is_empty() {
            "Kinds select candidate entities; predicates establish contextual roles such as authorship."
        } else {
            "Kinds are source-derived candidates and retain their exact kind_id on every node."
        };
        let node_count: u64 = if semantic_mode {
            typed.values().sum()
        } else {
            available_kinds
                .iter()
                .map(|k| kinds.get(k).copied().unwrap_or(0))
                .sum()
        };
        out.push(json!({"route_id":route.id,"candidate_kind_ids":route.kinds,
            "available_kind_ids":available_kinds,"confirming_predicate_ids":route.predicates,
            "available_confirming_predicate_ids":legacy.keys().collect::<Vec<_>>(),
            "confirming_relation_count":legacy.values().sum::<u64>(),"candidate_type_ids":route.types,
            "available_type_ids":typed.keys().collect::<Vec<_>>(),"confirming_relation_type_ids":route.relation_types,
            "available_confirming_relation_type_ids":typed_relations.keys().collect::<Vec<_>>(),
            "semantic_confirming_relation_count":typed_relations.values().sum::<u64>(),
            "node_count":node_count,"availability":availability,"role_readiness":readiness,"note":note}));
    }
    Ok(Value::Array(out))
}

fn presentation(entity_registry: &Value) -> Result<Value> {
    let Some(value) = entity_registry
        .get("context_presentation")
        .filter(|v| !v.is_null())
    else {
        return Ok(Value::Null);
    };
    crate::knowledge_readable_context::validate_current_context_presentation(value)?;
    let encoded = encoded(value)?;
    Ok(json!({"id":text(value,"presentation_id")?,
        "version":value.get("presentation_version").ok_or(Error::Invalid("presentation version"))?,
        "source_ref":"ToS/doctrine/semantic-interchange/entity-types.v1.json",
        "digest":format!("sha256:{}",Digest256::of_bytes(encoded.as_bytes()).to_hex()),"payload":value}))
}

fn capabilities(
    db: &CatalogView<'_>,
    vocab: &Value,
    max_entries: u64,
    budget: &ByteBudget,
) -> Result<Value> {
    let overview = vocab
        .get("overview")
        .ok_or(Error::Invalid("catalog overview"))?;
    let filters = vocab
        .get("filters")
        .ok_or(Error::Invalid("catalog filters"))?;
    let sources: Vec<String> = array(vocab, "sources")?
        .iter()
        .map(|s| text(s, "source_graph_id").map(str::to_owned))
        .collect::<Result<_>>()?;
    let mut node_fields = strings(filters, "node_fields")?;
    node_fields.sort();
    let mut relation_fields = strings(filters, "relation_fields")?;
    relation_fields.sort();
    Ok(json!({
        "execution_version":"tos-lens-execution-v7",
        "property_filters":{"selector":"property_id","scope":"node-query-and-path-node-query",
            "binding":"same-graph-snapshot","field_and_property_id":"mutually-exclusive",
            "unknown_value":"does-not-match-except-exists-false","outside_applicable_type":"does-not-match",
            "unknown_property":"error","operators":"declared-per-property",
            "string_comparison":"exact-codepoints-no-casefold-or-normalization",
            "units_and_languages":"source-declared-no-implicit-conversion"},
        "path_query":{"conditions":4,"steps_per_condition":4,"quantifiers":["exists","not_exists"],
            "combination":"all","scope":"node-selector-roots-and-selected-sources","walks_may_revisit_nodes":true},
        "inclusion":{"request_field":"explain","authority":"query-execution-not-semantic-proof"},
        "pagination":{"request_field":"pagination","scope":"bounded-lens-result",
            "snapshot_bound":true,"historical_snapshot_retention":false,"reexecutes_bounded_lens":true,
            "maximum_primary_nodes":100,"maximum_relations":100,"context_endpoints_may_repeat":true,
            "changed_query_or_snapshot_http_status":409},
        "neighborhood_profiles":[
            {"profile":"overview","definition":"Bibliographic and conceptual overview; dense text units, anchors and record-maker/provenance links are inspected separately. Shared record production does not establish semantic proximity. Source-filtered carriers of one declared ToS entity expand at zero distance before a relation hop, within node budgets.",
                "identity_expansion":"declared-tos-entity-id-zero-distance",
                "excluded_predicates":overview.get("excluded_predicate_ids"),
                "excluded_relation_type_ids":overview.get("excluded_relation_type_ids")},
            {"profile":"all","definition":"All declared relation kinds, including detailed text structure; result limits still apply.","excluded_predicates":[]}],
        "sources":sources,"filter_operators":filters.get("operators"),
        "operator_value_contracts":{"eq":"scalar","neq":"scalar","in":"scalar-or-scalar-array",
            "contains":"scalar-or-scalar-array","prefix":"string","exists":"boolean",
            "gt":"number","gte":"number","lt":"number","lte":"number"},
        "node_fields":node_fields,
        "relation_fields":relation_fields,
        "human_languages":{"key_pattern":"^(?:[A-Za-z]{2,8}(?:-[A-Za-z0-9]{1,8})*|[iIxX](?:-[A-Za-z0-9]{1,8})+)$(?![\\s\\S])",
            "reserved_roles":["default","original"],"registration_verified":false,
            "node_fields":field_catalog(db,NODE,"display",budget)?,"relation_fields":field_catalog(db,RELATION,"display",budget)?,
            "fallback_order":["default","ru","en","original","remaining-keys-sorted"],
            "boundary":"Availability is not translation, semantic quality, or interface-language equivalence."},
        "attribute_field_pattern":"^(?:attributes|semantics)\\.[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$",
        "node_attribute_fields":field_catalog(db,NODE,"attribute",budget)?,
        "relation_attribute_fields":field_catalog(db,RELATION,"attribute",budget)?,
        "facets":{"nodes":facets(db,vocab,NODE,max_entries,budget)?,"relations":facets(db,vocab,RELATION,max_entries,budget)?},
        "layouts":["auto","organic","timeline","flow","evidence","semantic","infrastructure","hierarchical","radial","matrix"],
        "endpoint_policies":["both","either","independent"],
        "focus":{"seed_field":"seed.focus_node_id","resolution_order":["id","entity_id","unique_native_id"],
            "shared_entity_id_resolution":"source-priority-then-node-id","ambiguous_native_id":"rejected",
            "default_depth":1,"default_direction":"either","default_layout":"radial"},
        "maximums":{"filters_per_item_kind":32,"traversal_depth":5,"nodes":1000,"relations":2000,"groups":200}
    }))
}

fn render(
    db: &CatalogView<'_>,
    header: &Value,
    entity_registry: &Value,
    relation_registry: &Value,
    lenses: &[Value],
    vocab: &Value,
    routes: &[Route],
    entity_entries: &BTreeMap<String, Value>,
    relation_entries: &BTreeMap<String, Value>,
    node_count: u64,
    relation_count: u64,
    max_entries: u64,
    budget: &ByteBudget,
) -> Result<Value> {
    let node_types = top_counts(db, NODE, "type", budget)?;
    let relation_types = top_counts(db, RELATION, "type", budget)?;
    let claim_types = top_counts(db, NODE, "claim-type", budget)?;
    let mut caps = capabilities(db, vocab, max_entries, budget)?;
    caps.as_object_mut()
        .ok_or(Error::Invalid("catalog capabilities"))?
        .insert(
            "entity_routes".into(),
            route_catalog(db, routes, entity_entries, budget)?,
        );
    let contract_refs = json!({
        "public_bundle":"/api/knowledge/contracts","knowledge_api":"access/contracts/knowledge-api.v1.json",
        "lens_spec":"access/contracts/lens-spec.v1.schema.json","lens_result":"access/contracts/lens-result.v1.schema.json",
        "temporal_comparison_request":"access/contracts/temporal-comparison-request.v1.schema.json",
        "temporal_comparison_result":"access/contracts/temporal-comparison-result.v1.schema.json",
        "knowledge_graph":"access/contracts/knowledge-graph.v1.schema.json",
        "readable_context":"access/contracts/readable-context.v1.schema.json",
        "entity_type_registry_schema":"ToS/contracts/semantic-entity-type-registry.schema.json",
        "relation_type_registry_schema":"ToS/contracts/semantic-relation-type-registry.schema.json",
        "entity_type_registry":"ToS/doctrine/semantic-interchange/entity-types.v1.json",
        "relation_type_registry":"ToS/doctrine/semantic-interchange/relation-types.v1.json"});
    let counts = header.get("counts").cloned().unwrap_or(json!({}));
    let packet = json!({"schema":SCHEMA,"source_revision":header.get("source_revision"),
        "context_presentation":presentation(entity_registry)?,"contract_refs":contract_refs,
        "counts":counts,"node_kinds":group_catalog(db,NODE,relation_entries,budget)?,
        "predicates":group_catalog(db,RELATION,relation_entries,budget)?,
        "semantic_registries":{
            "properties":entity_registry.get("property_definitions").cloned().unwrap_or(json!([])),
            "entity_types":registry_metadata(entity_registry,entity_entries,&node_types,
                "fallback_type_id",NODE,node_count,None)?,
            "relation_types":registry_metadata(relation_registry,relation_entries,&relation_types,
                "fallback_relation_type_id",RELATION,relation_count,Some(&claim_types))?},
        "lenses":lenses,"capabilities":caps,
        "authority_boundary":header.get("authority_boundary").cloned().unwrap_or(json!({}))});
    Ok(packet)
}

fn vocabulary() -> Result<Value> {
    serde_json::from_str(VOCABULARY).map_err(|_| Error::Invalid("catalog vocabulary"))
}
pub fn entity_entries(registry: &Value) -> Result<BTreeMap<String, Value>> {
    registry_entries(registry, "types", "type_id")
}
fn add(counts: &mut Counts, kind: &str, metric: &str, keys: &[&str]) {
    let key = std::iter::once(kind)
        .chain(std::iter::once(metric))
        .chain(keys.iter().copied())
        .map(str::to_owned)
        .collect();
    *counts.entry(key).or_insert(0) += 1;
}
fn push(
    posts: &mut Posts,
    kind: &str,
    metric: &str,
    key: &str,
    value: Value,
    position: usize,
) -> Result<()> {
    posts.push((
        vec![kind.into(), metric.into(), key.into()],
        Value::String(encoded(&value)?),
        i64::try_from(position).map_err(|_| Error::Budget("catalog position"))?,
    ));
    Ok(())
}
fn required<'a>(item: &'a Value, key: &str) -> Result<&'a Value> {
    item.get(key)
        .ok_or(Error::Invalid("catalog missing row field"))
}
fn node_summary(
    item: &JsonValue,
    entries: &BTreeMap<String, Value>,
    defs: &[Route],
) -> Result<JsonValue> {
    let kind = owner_required(item, "kind_id")?;
    let type_id = owner_required(item, "type_id")?;
    let type_string = if owner_truthy(type_id) {
        crate::local_prepared::python_value_string(type_id)?
    } else {
        String::new()
    };
    let legacy = defs
        .iter()
        .filter(|r| r.kinds.iter().any(|s| kind.as_str() == Some(s.as_str())))
        .map(|r| JsonValue::String(JsonString::from_utf8(&r.id)))
        .collect();
    let typed = defs
        .iter()
        .filter(|r| !r.types.is_empty() && type_is_a(&type_string, &r.types, entries))
        .map(|r| JsonValue::String(JsonString::from_utf8(&r.id)))
        .collect();
    Ok(JsonValue::Object(vec![
        (JsonString::from_utf8("kind_id"), kind.clone()),
        (JsonString::from_utf8("type_id"), type_id.clone()),
        (JsonString::from_utf8("legacy"), JsonValue::Array(legacy)),
        (JsonString::from_utf8("typed"), JsonValue::Array(typed)),
    ]))
}
pub fn row_facts(row: &CatalogRow, registry: &JsonValue) -> Result<(Counts, Posts, JsonValue)> {
    let kind = row.kind.as_str();
    let decoded = owner_view(&row.item, 16 * 1024 * 1024)?;
    let item = &decoded;
    let registry = owner_view(registry, 16 * 1024 * 1024)?;
    if !matches!(kind, NODE | RELATION)
        || row.id.is_empty()
        || item.get("id").and_then(Value::as_str) != Some(row.id.as_str())
        || !item.is_object()
    {
        return Err(Error::Invalid("catalog row identity"));
    }
    order_key(&row.source_order)?;
    let mut counts = Counts::new();
    let mut posts = Posts::new();
    let vocab = vocabulary()?;
    fn attributes(
        at: &Value,
        prefix: &str,
        source: &str,
        kind: &str,
        counts: &mut Counts,
        posts: &mut Posts,
    ) -> Result<()> {
        let Some(obj) = at.as_object() else {
            return Ok(());
        };
        let mut keys = obj.keys().collect::<Vec<_>>();
        keys.sort();
        for key in keys {
            let value = &obj[key];
            let field = format!("{prefix}.{key}");
            if !attribute_allowed(&field) {
                continue;
            }
            add(counts, kind, "attribute", &[&field]);
            add(
                counts,
                kind,
                "attribute-value-type",
                &[&field, value_kind(value)],
            );
            if !source.is_empty() {
                add(counts, kind, "attribute-source", &[&field, &source]);
            }
            let members = value
                .as_array()
                .map(|a| a.iter().collect::<Vec<_>>())
                .unwrap_or(vec![value]);
            let mut seen = BTreeSet::new();
            for (position, candidate) in members.into_iter().enumerate() {
                if value.is_array() {
                    add(
                        counts,
                        kind,
                        "attribute-array-type",
                        &[&field, value_kind(candidate)],
                    );
                }
                if candidate.is_null()
                    || candidate.is_object()
                    || candidate.is_array()
                    || seen.len() == 5
                {
                    continue;
                }
                let key = encoded(candidate)?;
                if legacy_scalar_json(candidate)?.chars().count() <= 180 && seen.insert(key) {
                    push(posts, kind, "examples", &field, candidate.clone(), position)?;
                }
            }
            if value.is_object() {
                attributes(value, &field, source, kind, counts, posts)?;
            }
        }
        Ok(())
    }
    let source = row
        .item
        .object_get("source_graph")
        .filter(|v| owner_truthy(v))
        .map(crate::local_prepared::python_value_string)
        .transpose()?
        .unwrap_or_default();
    attributes(
        item.get("attributes").unwrap_or(&Value::Null),
        "attributes",
        &source,
        kind,
        &mut counts,
        &mut posts,
    )?;
    if let Some(display) = item.get("display").and_then(Value::as_object) {
        for (field, forms) in display {
            let allowed = if kind == NODE {
                matches!(field.as_str(), "title" | "kind_label" | "summary")
            } else {
                matches!(
                    field.as_str(),
                    "label" | "inverse_label" | "statement" | "explanation"
                )
            };
            if !allowed {
                continue;
            }
            if let Some(forms) = forms.as_object() {
                for (language, text) in forms {
                    if form_key(language)
                        && text.as_str().is_some_and(|s| !python_strip(s).is_empty())
                    {
                        add(
                            &mut counts,
                            kind,
                            "display",
                            &[&format!("display.{field}.{language}")],
                        );
                    }
                }
            }
        }
    }
    let (g, t, m, label, state, provenance) = if kind == NODE {
        (
            "kind_id",
            "type_id",
            "type_mapping",
            "kind_label",
            "summary_state",
            "source_summary_available",
        )
    } else {
        (
            "predicate_id",
            "relation_type_id",
            "predicate_mapping",
            "label",
            "explanation_state",
            "source_explanation_available",
        )
    };
    let group = crate::local_prepared::python_value_string(owner_required(&row.item, g)?)?;
    let type_id = crate::local_prepared::python_value_string(owner_required(&row.item, t)?)?;
    let status = crate::local_prepared::python_value_string(
        owner_field(&row.item, &format!("{m}.status")).unwrap_or(&JsonValue::Null),
    )?;
    add(&mut counts, kind, "total", &[]);
    add(&mut counts, kind, "group", &[&group]);
    add(&mut counts, kind, "type", &[&type_id]);
    add(&mut counts, kind, "group-type", &[&group, &type_id]);
    add(&mut counts, kind, "group-status", &[&group, &status]);
    let representative = row
        .item
        .object_get("display")
        .and_then(|d| d.object_get(label))
        .ok_or(Error::Invalid("catalog row representative"))?;
    posts.push((
        vec![kind.into(), "representative".into(), group.clone()],
        Value::String(encoded_owner(representative, 16 * 1024 * 1024)?),
        0,
    ));
    for field in facet_names(&vocab, kind)? {
        let raw = owner_field(&row.item, &field).unwrap_or(&JsonValue::Null);
        let values = raw
            .as_array()
            .map(|a| a.iter().collect::<Vec<_>>())
            .unwrap_or(vec![raw]);
        let mut seen = BTreeSet::new();
        for (position, value) in values.into_iter().enumerate() {
            if value.is_null() {
                continue;
            }
            let text = crate::local_prepared::python_value_string(value)?;
            if text.is_empty() {
                continue;
            }
            add(&mut counts, kind, "facet", &[&field, &text]);
            if seen.insert(text.clone()) {
                push(
                    &mut posts,
                    kind,
                    "facet-order",
                    &field,
                    json!(text),
                    position,
                )?;
            }
        }
    }
    let state_value = crate::local_prepared::python_value_string(
        owner_field(&row.item, &format!("display.{state}")).unwrap_or(&JsonValue::Null),
    )?;
    add(
        &mut counts,
        kind,
        if kind == NODE {
            "summary-state"
        } else {
            "explanation-state"
        },
        &[&state_value],
    );
    if path(item, &format!("display.provenance.{provenance}")) == Some(&Value::Bool(false)) {
        add(&mut counts, kind, "without-source", &[]);
    }
    if matches!(status.as_str(), "mapped" | "unmapped") {
        add(&mut counts, kind, "mapping", &[&status]);
    }
    let summary = if kind == NODE {
        add(
            &mut counts,
            kind,
            "source",
            &[&crate::local_prepared::python_value_string(
                owner_required(&row.item, "source_graph")?,
            )?],
        );
        if let Some(claim) = path(item, "semantics.claim.relation_type_id")
            .and_then(Value::as_str)
            .filter(|s| !python_strip(s).is_empty())
        {
            let claim = python_strip(claim);
            add(&mut counts, kind, "claim-type", &[&claim]);
        }
        node_summary(&row.item, &entity_entries(&registry)?, &routes(&vocab)?)?
    } else {
        if required(item, "source_graph")?.as_str() == Some("semantic-interchange") {
            add(&mut counts, kind, "cross-layer", &[]);
        }
        let mut summary = Vec::new();
        for key in ["from_id", "to_id", "predicate_id", "relation_type_id"] {
            summary.push((
                JsonString::from_utf8(key),
                owner_required(&row.item, key)?.clone(),
            ));
        }
        JsonValue::Object(summary)
    };
    Ok((counts, posts, summary))
}
pub fn route_counts<F>(kind: &str, summary: &JsonValue, mut endpoint: F) -> Result<Counts>
where
    F: FnMut(&str) -> Result<Option<JsonValue>>,
{
    let mut counts = Counts::new();
    if kind == NODE {
        for route in summary
            .object_get("typed")
            .and_then(JsonValue::as_array)
            .into_iter()
            .flatten()
        {
            let type_id =
                crate::local_prepared::python_value_string(owner_required(summary, "type_id")?)?;
            add(
                &mut counts,
                kind,
                "route-type",
                &[
                    route.as_str().ok_or(Error::Invalid("catalog route"))?,
                    &type_id,
                ],
            );
        }
        return Ok(counts);
    }
    let left = endpoint(&crate::local_prepared::python_value_string(
        owner_required(summary, "from_id")?,
    )?)?;
    let right = endpoint(&crate::local_prepared::python_value_string(
        owner_required(summary, "to_id")?,
    )?)?;
    for route in routes(&vocabulary()?)? {
        let contains = |name: &str| {
            [&left, &right].iter().any(|s| {
                s.as_ref()
                    .and_then(|s| s.object_get(name))
                    .and_then(JsonValue::as_array)
                    .is_some_and(|a| a.iter().any(|v| v.as_str() == Some(route.id.as_str())))
            })
        };
        let predicate = owner_required(summary, "predicate_id")?;
        let type_id = owner_required(summary, "relation_type_id")?;
        if route
            .predicates
            .iter()
            .any(|s| predicate.as_str() == Some(s.as_str()))
            && contains("legacy")
        {
            let predicate = crate::local_prepared::python_value_string(predicate)?;
            add(
                &mut counts,
                kind,
                "route-predicate",
                &[&route.id, &predicate],
            );
        }
        if route
            .relation_types
            .iter()
            .any(|s| type_id.as_str() == Some(s.as_str()))
            && contains("typed")
        {
            let type_id = crate::local_prepared::python_value_string(type_id)?;
            add(&mut counts, kind, "route-type", &[&route.id, &type_id]);
        }
    }
    Ok(counts)
}
fn total(view: &CatalogView<'_>, kind: &str, metric: &str) -> u64 {
    view.counts
        .get(&vec![kind.into(), metric.into()])
        .copied()
        .unwrap_or(0) as u64
}
fn graph_counts(view: &CatalogView<'_>, original: &Value, budget: &ByteBudget) -> Result<Value> {
    let mut result = original.clone();
    let n = total(view, NODE, "total");
    let r = total(view, RELATION, "total");
    let get = |kind: &str, metric: &str, status: &str| {
        view.counts
            .get(&vec![kind.into(), metric.into(), status.into()])
            .copied()
            .unwrap_or(0)
    };
    let replacements = json!({"nodes":n,"relations":r,"sources":top_counts(view,NODE,"source",budget)?,"display_coverage":{"node_titles":n,"node_summaries":n,"node_summary_states":top_counts(view,NODE,"summary-state",budget)?,"nodes_without_source_summary":total(view,NODE,"without-source"),"relation_labels":r,"relation_statements":r,"relation_explanations":r,"relation_explanation_states":top_counts(view,RELATION,"explanation-state",budget)?,"relations_without_source_explanation":total(view,RELATION,"without-source")},"semantic_mapping":{"mapped_nodes":get(NODE,"mapping","mapped"),"unmapped_nodes":get(NODE,"mapping","unmapped"),"mapped_relations":get(RELATION,"mapping","mapped"),"unmapped_relations":get(RELATION,"mapping","unmapped"),"cross_layer_relations":total(view,RELATION,"cross-layer")}});
    if let Some(object) = result.as_object_mut() {
        for (key, value) in object {
            if let Some(new) = replacements.get(key) {
                if matches!(key.as_str(), "display_coverage" | "semantic_mapping")
                    && value.is_object()
                {
                    for (child, v) in value.as_object_mut().unwrap() {
                        if let Some(n) = new.get(child) {
                            *v = n.clone();
                        }
                    }
                } else {
                    *value = new.clone();
                }
            }
        }
    }
    Ok(result)
}
pub fn finalized_header(inputs: &CatalogInputs, catalog: &JsonValue) -> Result<JsonValue> {
    inputs.validate()?;
    if catalog.object_get("schema").and_then(JsonValue::as_str) != Some(SCHEMA)
        || catalog
            .object_get("source_revision")
            .unwrap_or(&JsonValue::Null)
            != inputs
                .header
                .object_get("source_revision")
                .unwrap_or(&JsonValue::Null)
    {
        return Err(Error::Invalid("catalog result draft source header"));
    }
    let empty = JsonValue::Object(Vec::new());
    if catalog_owner_digest(
        catalog.object_get("authority_boundary").unwrap_or(&empty),
        16 * 1024 * 1024,
    )? != catalog_owner_digest(
        inputs
            .header
            .object_get("authority_boundary")
            .unwrap_or(&empty),
        16 * 1024 * 1024,
    )? {
        return Err(Error::Invalid("catalog authority boundary mismatch"));
    }
    let mut header = inputs.header.clone();
    if inputs.header.object_get("counts").is_some() {
        let counts = catalog
            .object_get("counts")
            .ok_or(Error::Invalid("catalog counts"))?
            .clone();
        owner_set(&mut header, "counts", counts)?;
    } else if catalog.object_get("counts") != Some(&empty) {
        return Err(Error::Invalid("catalog invented counts"));
    }
    Ok(header)
}
pub fn render_catalog(
    inputs: &CatalogInputs,
    db: &Transaction<'_>,
    derive_counts: bool,
    limits: CatalogRenderLimits,
) -> Result<JsonValue> {
    inputs.validate()?;
    if limits.max_output_entries == 0
        || limits.max_aggregate_bytes == 0
        || limits.max_catalog_bytes == 0
    {
        return Err(Error::Budget("catalog limits"));
    }
    let budget = ByteBudget::new(limits.max_aggregate_bytes);
    let mut counts = Counts::new();
    let mut stmt=db.prepare("SELECT CASE WHEN length(CAST(a.value AS BLOB))<=?1 THEN a.value ELSE NULL END,t.n FROM catalog_totals t JOIN catalog_atoms a ON a.atom=t.key LIMIT ?2")?;
    let mut rows = stmt.query(params![
        i64::try_from(limits.max_aggregate_bytes)
            .map_err(|_| Error::Budget("catalog SQL byte limit"))?,
        i64::try_from(
            limits
                .max_output_entries
                .checked_add(1)
                .ok_or(Error::Budget("catalog aggregate entries"))?
        )
        .map_err(|_| Error::Budget("catalog SQL entry limit"))?
    ])?;
    let mut selected = 0u64;
    while let Some(row) = rows.next()? {
        selected += 1;
        if selected > limits.max_output_entries {
            return Err(Error::Budget("catalog aggregate entries"));
        }
        let raw: Option<String> = row.get(0)?;
        let raw = raw.ok_or(Error::Budget("catalog aggregate atom bytes"))?;
        budget.charge(raw.len())?;
        let key: Vec<String> =
            serde_json::from_str(&raw).map_err(|_| Error::Invalid("catalog count key"))?;
        let n: i64 = row.get(1)?;
        if n <= 0 {
            return Err(Error::Invalid("catalog count"));
        }
        counts.insert(key, n);
    }
    let mut groups: BTreeMap<Vec<String>, BTreeMap<String, u64>> = BTreeMap::new();
    for (key, n) in &counts {
        if key.len() >= 3 {
            groups
                .entry(key[..key.len() - 1].to_vec())
                .or_default()
                .insert(key.last().unwrap().clone(), *n as u64);
        }
    }
    let view = CatalogView {
        db,
        counts,
        groups,
        max_entries: limits.max_output_entries,
    };
    let mut header = owner_view(&inputs.header, usize::MAX)?;
    if derive_counts {
        header["counts"] =
            graph_counts(&view, header.get("counts").unwrap_or(&json!({})), &budget)?;
    }
    let vocab = vocabulary()?;
    let entity = owner_view(&inputs.entity_registry, usize::MAX)?;
    let relation = owner_view(&inputs.relation_registry, usize::MAX)?;
    let lenses = inputs
        .lenses
        .iter()
        .map(|v| owner_view(v, usize::MAX))
        .collect::<Result<Vec<_>>>()?;
    let output = render(
        &view,
        &header,
        &entity,
        &relation,
        &lenses,
        &vocab,
        &routes(&vocab)?,
        &entity_entries(&entity)?,
        &registry_entries(&relation, "relations", "relation_type_id")?,
        total(&view, NODE, "total"),
        total(&view, RELATION, "total"),
        limits.max_output_entries,
        &budget,
    )?;
    let output = ordered_catalog(inputs, db, &output, derive_counts, limits)?;
    let raw = encoded_owner(&output, limits.max_catalog_bytes)?;
    if raw.len() > limits.max_catalog_bytes {
        return Err(Error::Budget("catalog output bytes"));
    }
    Ok(output)
}

const VOCABULARY: &str = r###"{"catalog":{"canonical_order":"source-graph-id-v1","facets":["source_graph","kind_id","type_id","type_mapping.status","epistemic.authority_layer","epistemic.canon_status","epistemic.review_posture","graph_layers","view_ids"]},"overview":{"excluded_predicate_ids":["anchored_in","annotation_member","has_anchor","has_text_unit"],"excluded_relation_type_ids":["tos.relation.generated-by","tos.relation.made-by"],"routes":[{"route_id":"concept","candidate_kind_ids":["concept","principle"],"confirming_predicate_ids":[],"candidate_type_ids":["tos.entity.concept","tos.entity.principle"],"confirming_relation_type_ids":[]},{"route_id":"author","candidate_kind_ids":["agent"],"confirming_predicate_ids":["authored_by"],"candidate_type_ids":["tos.entity.agent"],"confirming_relation_type_ids":["tos.relation.authored-by"]},{"route_id":"work","candidate_kind_ids":["work"],"confirming_predicate_ids":["authored_by","has_expression"],"candidate_type_ids":["tos.entity.work"],"confirming_relation_type_ids":["tos.relation.authored-by","tos.relation.has-expression"]},{"route_id":"word","candidate_kind_ids":["lexeme","word","token","word-occurrence"],"confirming_predicate_ids":["occurs_in","expresses_concept"],"candidate_type_ids":[],"confirming_relation_type_ids":[]},{"route_id":"tradition","candidate_kind_ids":["tradition","school_tradition"],"confirming_predicate_ids":[],"candidate_type_ids":["tos.entity.tradition","tos.entity.school-tradition"],"confirming_relation_type_ids":[]},{"route_id":"place","candidate_kind_ids":["place","region"],"confirming_predicate_ids":[],"candidate_type_ids":["tos.entity.place"],"confirming_relation_type_ids":["tos.relation.has-normalized-place"]},{"route_id":"source-object","candidate_kind_ids":["work","expression","edition","item","file","source_witness"],"confirming_predicate_ids":[],"candidate_type_ids":["tos.entity.intellectual-object","tos.entity.source-witness"],"confirming_relation_type_ids":[]}]},"filters":{"operators":["eq","neq","in","contains","prefix","exists","gt","gte","lt","lte"],"dotted_field_profile":"safe-attributes-and-semantics-v1","node_fields":["id","entity_id","native_id","source_dossier_ref","source_graph","kind_id","type_id","type_mapping.status","type_mapping.source_kind_id","display.title.default","display.title.ru","display.title.en","display.kind_label.default","display.summary.default","display.summary.ru","display.summary.en","display.summary_state","epistemic.authority_layer","epistemic.canon_status","epistemic.review_posture","epistemic.confidence","graph_layers","view_ids","source_refs"],"relation_fields":["id","native_id","source_graph","from_id","to_id","predicate_id","relation_type_id","predicate_mapping.status","predicate_mapping.source_predicate_id","display.label.default","display.label.ru","display.label.en","display.statement.default","display.explanation.default","display.explanation_state","epistemic.authority_layer","epistemic.canon_status","epistemic.review_posture","epistemic.confidence","graph_layers","view_ids","source_refs"],"property_definitions":"selected-entity-registry-property-definitions; preserve-applies-to-inherited-value-type-operators","unknown_field":"reject","unknown_property_id":"reject"},"sources":[{"source_graph_id":"philosophy","owner_ref":"ToS/philosophy/AGENTS.md","input_role":"philosophy-graph","adapter_profile":"philosophy-node-edge-v1","representative_priority":3},{"source_graph_id":"canon","owner_ref":"ToS/canon/AGENTS.md","input_role":"corpus-index","adapter_profile":"canon-node-relation-v1","representative_priority":1},{"source_graph_id":"candidate-intake","owner_ref":"ToS/candidate-intake/AGENTS.md","input_role":"corpus-index","adapter_profile":"candidate-relation-v1","representative_priority":4},{"source_graph_id":"source-navigation","owner_ref":"ToS/source-witnesses/AGENTS.md","input_role":"corpus-source-navigation","adapter_profile":"source-navigation-node-edge-v1","representative_priority":0},{"source_graph_id":"source-claims","owner_ref":"ToS/source-witnesses/AGENTS.md","input_role":"bibliographic-claims","adapter_profile":"reified-bibliographic-claims-v1","representative_priority":2},{"source_graph_id":"semantic-interchange","owner_ref":"ToS/doctrine/semantic-interchange/README.md","input_role":"derived-cross-source-join","adapter_profile":"declared-identity-and-source-ref-joins-v1","representative_priority":6},{"source_graph_id":"repository","owner_ref":"ToS/source_home.manifest.json","input_role":"corpus-topology","adapter_profile":"repository-topology-v1","representative_priority":5}]}"###;

fn python_strip(value: &str) -> String {
    tos_foundation::python_strip_unicode16_v1(value, value.chars().count())
        .unwrap_or(value)
        .to_owned()
}
pub fn validate_row_order(inputs: &CatalogInputs, row: &CatalogRow) -> Result<()> {
    order_key(&row.source_order)?;
    match (&inputs.source_order_profile, &row.source_order) {
        (SourceOrderProfile::OwnerSequence, CatalogOrder::OwnerSequence(_)) => Ok(()),
        (SourceOrderProfile::SourceGraphId, CatalogOrder::SourceGraphId(source, id))
            if row
                .item
                .object_get("source_graph")
                .and_then(JsonValue::as_str)
                == Some(source.as_str())
                && row.id == *id =>
        {
            Ok(())
        }
        _ => Err(Error::Invalid("catalog row source order profile")),
    }
}
fn legacy_scalar_json(value: &Value) -> Result<String> {
    encoded(value)
}
fn preserve_counts(original: &JsonValue, new: &Value) -> Result<JsonValue> {
    fn merge(original: &JsonValue, new: &Value, path: &str) -> Result<JsonValue> {
        if matches!(
            path,
            "sources"
                | "display_coverage.node_summary_states"
                | "display_coverage.relation_explanation_states"
        ) {
            return owner_from(new, 16 * 1024 * 1024);
        }
        if owner_view(original, 16 * 1024 * 1024)? == *new {
            return Ok(original.clone());
        }
        if let (JsonValue::Object(fields), Some(obj)) = (original, new.as_object()) {
            let mut out = Vec::new();
            let mut seen = BTreeSet::new();
            for (key, value) in fields {
                if let Some(name) = key.as_str() {
                    if let Some(next) = obj.get(name) {
                        let child = if path.is_empty() {
                            name.to_owned()
                        } else {
                            format!("{path}.{name}")
                        };
                        out.push((key.clone(), merge(value, next, &child)?));
                        seen.insert(name.to_owned());
                    }
                }
            }
            for (name, value) in obj {
                if !seen.contains(name) {
                    out.push((
                        JsonString::from_utf8(name),
                        owner_from(value, 16 * 1024 * 1024)?,
                    ));
                }
            }
            Ok(JsonValue::Object(out))
        } else {
            owner_from(new, 16 * 1024 * 1024)
        }
    }
    merge(original, new, "")
}

fn routes(vocab: &Value) -> Result<Vec<Route>> {
    let raw = array(
        vocab
            .get("overview")
            .ok_or(Error::Invalid("overview vocabulary"))?,
        "routes",
    )?;
    raw.iter()
        .map(|r| {
            Ok(Route {
                id: text(r, "route_id")?.into(),
                kinds: ordered_strings(r, "candidate_kind_ids")?,
                predicates: ordered_strings(r, "confirming_predicate_ids")?,
                types: ordered_strings(r, "candidate_type_ids")?,
                relation_types: ordered_strings(r, "confirming_relation_type_ids")?,
            })
        })
        .collect()
}

fn owner_field<'a>(value: &'a JsonValue, path: &str) -> Option<&'a JsonValue> {
    path.split('.')
        .try_fold(value, |at, key| at.object_get(key))
}
// Maintained Python catalog constructors are insertion ordered. Reduction maps
// are disposable views; this boundary reconstructs those constructors and
// restores every owner payload before emitting the packet.
fn owner_set(object: &mut JsonValue, key: &str, value: JsonValue) -> Result<()> {
    let JsonValue::Object(fields) = object else {
        return Err(Error::Invalid("catalog owner object"));
    };
    if let Some((_, old)) = fields
        .iter_mut()
        .find(|(name, _)| name.as_str() == Some(key))
    {
        *old = value;
    } else {
        fields.push((JsonString::from_utf8(key), value));
    }
    Ok(())
}
fn owner_mut<'a>(value: &'a mut JsonValue, key: &str) -> Result<&'a mut JsonValue> {
    let JsonValue::Object(fields) = value else {
        return Err(Error::Invalid("catalog owner object"));
    };
    fields
        .iter_mut()
        .find(|(name, _)| name.as_str() == Some(key))
        .map(|(_, value)| value)
        .ok_or(Error::Invalid("catalog owner member"))
}
fn owner_order_at(value: &mut JsonValue, path: &[&str], keys: &[&str]) -> Result<()> {
    if let Some((head, tail)) = path.split_first() {
        if *head == "*" {
            let JsonValue::Array(items) = value else {
                return Err(Error::Invalid("catalog ordered array"));
            };
            for item in items {
                owner_order_at(item, tail, keys)?;
            }
            return Ok(());
        }
        if let JsonValue::Array(items) = value {
            let index = head
                .parse::<usize>()
                .map_err(|_| Error::Invalid("catalog ordered array index"))?;
            let item = items
                .get_mut(index)
                .ok_or(Error::Invalid("catalog ordered array index"))?;
            return owner_order_at(item, tail, keys);
        }
        let child = owner_mut(value, head)?;
        if child.is_null() {
            return Ok(());
        }
        return owner_order_at(child, tail, keys);
    }
    if value.is_null() {
        return Ok(());
    }
    let JsonValue::Object(fields) = value else {
        return Err(Error::Invalid("catalog ordered object"));
    };
    let mut remaining = std::mem::take(fields);
    let mut ordered = Vec::with_capacity(remaining.len());
    for key in keys {
        let index = remaining
            .iter()
            .position(|(name, _)| name.as_str() == Some(key))
            .ok_or(Error::Invalid("catalog constructor field"))?;
        ordered.push(remaining.remove(index));
    }
    if !remaining.is_empty() {
        return Err(Error::Invalid("catalog constructor extra field"));
    }
    *fields = ordered;
    Ok(())
}
fn owner_registry_entries(
    registry: &JsonValue,
    field: &str,
    key: &str,
) -> BTreeMap<String, JsonValue> {
    let mut entries = BTreeMap::new();
    for item in registry
        .object_get(field)
        .and_then(JsonValue::as_array)
        .into_iter()
        .flatten()
    {
        if let Some(id) = item
            .object_get(key)
            .and_then(JsonValue::as_str)
            .filter(|s| !python_strip(s).is_empty())
        {
            entries.insert(id.to_owned(), item.clone());
        }
    }
    entries
}
fn owner_entry_counts(
    registry: &JsonValue,
    field: &str,
    key: &str,
    rendered: &JsonValue,
    count_keys: &[&str],
) -> Result<JsonValue> {
    let entries = owner_registry_entries(registry, field, key);
    let mut result = Vec::new();
    for row in rendered
        .as_array()
        .ok_or(Error::Invalid("catalog registry rows"))?
    {
        let id = row
            .object_get(key)
            .and_then(JsonValue::as_str)
            .ok_or(Error::Invalid("catalog registry ID"))?;
        let mut entry = entries
            .get(id)
            .ok_or(Error::Invalid("catalog registry original entry"))?
            .clone();
        for count_key in count_keys {
            owner_set(
                &mut entry,
                count_key,
                row.object_get(count_key)
                    .ok_or(Error::Invalid("catalog registry count"))?
                    .clone(),
            )?;
        }
        result.push(entry);
    }
    Ok(JsonValue::Array(result))
}
fn owner_representatives(
    output: &mut JsonValue,
    db: &Transaction<'_>,
    kind: &str,
    collection: &str,
    group_key: &str,
    cap: usize,
) -> Result<()> {
    let JsonValue::Array(groups) = owner_mut(output, collection)? else {
        return Err(Error::Invalid("catalog groups"));
    };
    // This bounded reread restores owner member order. Its bytes were charged
    // once by the logical first_values selection, as in maintained _SQLView.
    let mut stmt=db.prepare("SELECT CASE WHEN length(CAST(v.value AS BLOB))<=?1 THEN v.value ELSE NULL END FROM catalog_heads h JOIN catalog_atoms b ON b.atom=h.bucket JOIN catalog_atoms v ON v.atom=h.value WHERE b.value=?2 ORDER BY h.source_order,h.position LIMIT 1")?;
    for group in groups {
        let id = group
            .object_get(group_key)
            .and_then(JsonValue::as_str)
            .ok_or(Error::Invalid("catalog group ID"))?;
        let bucket = encoded(&json!([kind, "representative", id]))?;
        let raw: Option<String> = stmt.query_row(
            params![
                i64::try_from(cap).map_err(|_| Error::Budget("catalog representative limit"))?,
                bucket
            ],
            |r| r.get(0),
        )?;
        let raw = raw.ok_or(Error::Budget("catalog representative bytes"))?;
        if raw.len() > cap {
            return Err(Error::Budget("catalog representative bytes"));
        }
        let value = tos_foundation::parse_json(
            raw.as_bytes(),
            JsonMode::PublishedStrict,
            JsonLimits::new(cap, 96, 1_000_000, 4096)
                .map_err(|_| Error::Budget("catalog representative limits"))?,
        )
        .map_err(|_| Error::Invalid("catalog representative JSON"))?
        .into_root();
        owner_set(group, "display", value)?;
    }
    Ok(())
}
fn owner_semantic_definitions(output: &mut JsonValue, registry: &JsonValue) -> Result<()> {
    let entries = owner_registry_entries(registry, "relations", "relation_type_id");
    let JsonValue::Array(groups) = owner_mut(output, "predicates")? else {
        return Err(Error::Invalid("catalog predicates"));
    };
    for group in groups {
        let types = group
            .object_get("relation_type_ids")
            .and_then(JsonValue::as_array)
            .ok_or(Error::Invalid("catalog relation type IDs"))?;
        let mut result = Vec::new();
        for id in types {
            if let Some(entry) = id.as_str().and_then(|id| entries.get(id)) {
                let mut fields = vec![(JsonString::from_utf8("relation_type_id"), id.clone())];
                for key in ["labels", "definition", "domain_type_ids", "range_type_ids"] {
                    fields.push((
                        JsonString::from_utf8(key),
                        entry.object_get(key).cloned().unwrap_or(JsonValue::Null),
                    ));
                }
                result.push(JsonValue::Object(fields));
            }
        }
        owner_set(group, "semantic_definitions", JsonValue::Array(result))?;
    }
    Ok(())
}
fn ordered_catalog(
    inputs: &CatalogInputs,
    db: &Transaction<'_>,
    packet: &Value,
    derive_counts: bool,
    limits: CatalogRenderLimits,
) -> Result<JsonValue> {
    let mut output = owner_from(packet, limits.max_catalog_bytes)?;
    owner_order_at(
        &mut output,
        &[],
        &[
            "schema",
            "source_revision",
            "context_presentation",
            "contract_refs",
            "counts",
            "node_kinds",
            "predicates",
            "semantic_registries",
            "lenses",
            "capabilities",
            "authority_boundary",
        ],
    )?;
    owner_order_at(
        &mut output,
        &["contract_refs"],
        &[
            "public_bundle",
            "knowledge_api",
            "lens_spec",
            "lens_result",
            "temporal_comparison_request",
            "temporal_comparison_result",
            "knowledge_graph",
            "readable_context",
            "entity_type_registry_schema",
            "relation_type_registry_schema",
            "entity_type_registry",
            "relation_type_registry",
        ],
    )?;
    owner_order_at(
        &mut output,
        &["semantic_registries"],
        &["properties", "entity_types", "relation_types"],
    )?;
    owner_order_at(
        &mut output,
        &["semantic_registries", "entity_types"],
        &[
            "registry_id",
            "registry_version",
            "source_refs",
            "fallback_type_id",
            "mapped_instance_count",
            "unmapped_instance_count",
            "entries",
        ],
    )?;
    owner_order_at(
        &mut output,
        &["semantic_registries", "relation_types"],
        &[
            "registry_id",
            "registry_version",
            "source_refs",
            "fallback_relation_type_id",
            "mapped_edge_instance_count",
            "unmapped_edge_instance_count",
            "entries",
        ],
    )?;
    owner_order_at(
        &mut output,
        &["capabilities"],
        &[
            "execution_version",
            "property_filters",
            "path_query",
            "inclusion",
            "pagination",
            "neighborhood_profiles",
            "sources",
            "filter_operators",
            "operator_value_contracts",
            "node_fields",
            "relation_fields",
            "human_languages",
            "attribute_field_pattern",
            "node_attribute_fields",
            "relation_attribute_fields",
            "facets",
            "layouts",
            "endpoint_policies",
            "focus",
            "entity_routes",
            "maximums",
        ],
    )?;
    owner_order_at(
        &mut output,
        &["capabilities", "property_filters"],
        &[
            "selector",
            "scope",
            "binding",
            "field_and_property_id",
            "unknown_value",
            "outside_applicable_type",
            "unknown_property",
            "operators",
            "string_comparison",
            "units_and_languages",
        ],
    )?;
    owner_order_at(
        &mut output,
        &["capabilities", "path_query"],
        &[
            "conditions",
            "steps_per_condition",
            "quantifiers",
            "combination",
            "scope",
            "walks_may_revisit_nodes",
        ],
    )?;
    owner_order_at(
        &mut output,
        &["capabilities", "inclusion"],
        &["request_field", "authority"],
    )?;
    owner_order_at(
        &mut output,
        &["capabilities", "pagination"],
        &[
            "request_field",
            "scope",
            "snapshot_bound",
            "historical_snapshot_retention",
            "reexecutes_bounded_lens",
            "maximum_primary_nodes",
            "maximum_relations",
            "context_endpoints_may_repeat",
            "changed_query_or_snapshot_http_status",
        ],
    )?;
    owner_order_at(
        &mut output,
        &["capabilities", "neighborhood_profiles", "0"],
        &[
            "profile",
            "definition",
            "identity_expansion",
            "excluded_predicates",
            "excluded_relation_type_ids",
        ],
    )?;
    owner_order_at(
        &mut output,
        &["capabilities", "neighborhood_profiles", "1"],
        &["profile", "definition", "excluded_predicates"],
    )?;
    owner_order_at(
        &mut output,
        &["capabilities", "operator_value_contracts"],
        &[
            "eq", "neq", "in", "contains", "prefix", "exists", "gt", "gte", "lt", "lte",
        ],
    )?;
    owner_order_at(
        &mut output,
        &["capabilities", "human_languages"],
        &[
            "key_pattern",
            "reserved_roles",
            "registration_verified",
            "node_fields",
            "relation_fields",
            "fallback_order",
            "boundary",
        ],
    )?;
    owner_order_at(
        &mut output,
        &["capabilities", "facets"],
        &["nodes", "relations"],
    )?;
    owner_order_at(
        &mut output,
        &["capabilities", "focus"],
        &[
            "seed_field",
            "resolution_order",
            "shared_entity_id_resolution",
            "ambiguous_native_id",
            "default_depth",
            "default_direction",
            "default_layout",
        ],
    )?;
    owner_order_at(
        &mut output,
        &["capabilities", "maximums"],
        &[
            "filters_per_item_kind",
            "traversal_depth",
            "nodes",
            "relations",
            "groups",
        ],
    )?;
    owner_order_at(
        &mut output,
        &["context_presentation"],
        &["id", "version", "source_ref", "digest", "payload"],
    )?;
    owner_order_at(
        &mut output,
        &["node_kinds", "*"],
        &[
            "kind_id",
            "display",
            "count",
            "type_ids",
            "mapping_statuses",
        ],
    )?;
    owner_order_at(
        &mut output,
        &["predicates", "*"],
        &[
            "predicate_id",
            "display",
            "count",
            "relation_type_ids",
            "mapping_statuses",
            "semantic_definitions",
        ],
    )?;
    owner_order_at(
        &mut output,
        &["predicates", "*", "semantic_definitions", "*"],
        &[
            "relation_type_id",
            "labels",
            "definition",
            "domain_type_ids",
            "range_type_ids",
        ],
    )?;
    owner_order_at(
        &mut output,
        &["capabilities", "entity_routes", "*"],
        &[
            "route_id",
            "candidate_kind_ids",
            "available_kind_ids",
            "confirming_predicate_ids",
            "available_confirming_predicate_ids",
            "confirming_relation_count",
            "candidate_type_ids",
            "available_type_ids",
            "confirming_relation_type_ids",
            "available_confirming_relation_type_ids",
            "semantic_confirming_relation_count",
            "node_count",
            "availability",
            "role_readiness",
            "note",
        ],
    )?;
    owner_order_at(
        &mut output,
        &["capabilities", "node_attribute_fields", "*"],
        &[
            "field",
            "item_count",
            "value_types",
            "array_item_types",
            "sources",
            "examples",
        ],
    )?;
    owner_order_at(
        &mut output,
        &["capabilities", "human_languages", "node_fields", "*"],
        &["field", "available_item_count"],
    )?;
    owner_order_at(
        &mut output,
        &["capabilities", "relation_attribute_fields", "*"],
        &[
            "field",
            "item_count",
            "value_types",
            "array_item_types",
            "sources",
            "examples",
        ],
    )?;
    owner_order_at(
        &mut output,
        &["capabilities", "human_languages", "relation_fields", "*"],
        &["field", "available_item_count"],
    )?;
    owner_order_at(
        &mut output,
        &["capabilities", "facets", "nodes"],
        &[
            "source_graph",
            "kind_id",
            "type_id",
            "type_mapping.status",
            "epistemic.authority_layer",
            "epistemic.canon_status",
            "epistemic.review_posture",
            "graph_layers",
            "view_ids",
        ],
    )?;
    owner_order_at(
        &mut output,
        &["capabilities", "facets", "nodes", "source_graph", "*"],
        &["value", "count"],
    )?;
    owner_order_at(
        &mut output,
        &["capabilities", "facets", "nodes", "kind_id", "*"],
        &["value", "count"],
    )?;
    owner_order_at(
        &mut output,
        &["capabilities", "facets", "nodes", "type_id", "*"],
        &["value", "count"],
    )?;
    owner_order_at(
        &mut output,
        &[
            "capabilities",
            "facets",
            "nodes",
            "type_mapping.status",
            "*",
        ],
        &["value", "count"],
    )?;
    owner_order_at(
        &mut output,
        &[
            "capabilities",
            "facets",
            "nodes",
            "epistemic.authority_layer",
            "*",
        ],
        &["value", "count"],
    )?;
    owner_order_at(
        &mut output,
        &[
            "capabilities",
            "facets",
            "nodes",
            "epistemic.canon_status",
            "*",
        ],
        &["value", "count"],
    )?;
    owner_order_at(
        &mut output,
        &[
            "capabilities",
            "facets",
            "nodes",
            "epistemic.review_posture",
            "*",
        ],
        &["value", "count"],
    )?;
    owner_order_at(
        &mut output,
        &["capabilities", "facets", "nodes", "graph_layers", "*"],
        &["value", "count"],
    )?;
    owner_order_at(
        &mut output,
        &["capabilities", "facets", "nodes", "view_ids", "*"],
        &["value", "count"],
    )?;
    owner_order_at(
        &mut output,
        &["capabilities", "facets", "relations"],
        &[
            "source_graph",
            "predicate_id",
            "relation_type_id",
            "predicate_mapping.status",
            "epistemic.authority_layer",
            "epistemic.canon_status",
            "epistemic.review_posture",
            "graph_layers",
            "view_ids",
        ],
    )?;
    owner_order_at(
        &mut output,
        &["capabilities", "facets", "relations", "source_graph", "*"],
        &["value", "count"],
    )?;
    owner_order_at(
        &mut output,
        &["capabilities", "facets", "relations", "predicate_id", "*"],
        &["value", "count"],
    )?;
    owner_order_at(
        &mut output,
        &[
            "capabilities",
            "facets",
            "relations",
            "relation_type_id",
            "*",
        ],
        &["value", "count"],
    )?;
    owner_order_at(
        &mut output,
        &[
            "capabilities",
            "facets",
            "relations",
            "predicate_mapping.status",
            "*",
        ],
        &["value", "count"],
    )?;
    owner_order_at(
        &mut output,
        &[
            "capabilities",
            "facets",
            "relations",
            "epistemic.authority_layer",
            "*",
        ],
        &["value", "count"],
    )?;
    owner_order_at(
        &mut output,
        &[
            "capabilities",
            "facets",
            "relations",
            "epistemic.canon_status",
            "*",
        ],
        &["value", "count"],
    )?;
    owner_order_at(
        &mut output,
        &[
            "capabilities",
            "facets",
            "relations",
            "epistemic.review_posture",
            "*",
        ],
        &["value", "count"],
    )?;
    owner_order_at(
        &mut output,
        &["capabilities", "facets", "relations", "graph_layers", "*"],
        &["value", "count"],
    )?;
    owner_order_at(
        &mut output,
        &["capabilities", "facets", "relations", "view_ids", "*"],
        &["value", "count"],
    )?;
    owner_set(
        &mut output,
        "source_revision",
        inputs
            .header
            .object_get("source_revision")
            .cloned()
            .unwrap_or(JsonValue::Null),
    )?;
    owner_set(
        &mut output,
        "authority_boundary",
        inputs
            .header
            .object_get("authority_boundary")
            .cloned()
            .unwrap_or(JsonValue::Object(Vec::new())),
    )?;
    owner_set(
        &mut output,
        "lenses",
        JsonValue::Array(inputs.lenses.clone()),
    )?;
    let original_counts = inputs
        .header
        .object_get("counts")
        .cloned()
        .unwrap_or(JsonValue::Object(Vec::new()));
    let counts = if derive_counts {
        preserve_counts(
            &original_counts,
            packet
                .get("counts")
                .ok_or(Error::Invalid("catalog counts"))?,
        )?
    } else {
        original_counts
    };
    owner_set(&mut output, "counts", counts)?;
    owner_representatives(
        &mut output,
        db,
        NODE,
        "node_kinds",
        "kind_id",
        limits.max_catalog_bytes.min(limits.max_aggregate_bytes),
    )?;
    owner_representatives(
        &mut output,
        db,
        RELATION,
        "predicates",
        "predicate_id",
        limits.max_catalog_bytes.min(limits.max_aggregate_bytes),
    )?;
    owner_semantic_definitions(&mut output, &inputs.relation_registry)?;
    if let Some(presentation) = inputs
        .entity_registry
        .object_get("context_presentation")
        .filter(|v| !v.is_null())
    {
        owner_set(
            owner_mut(&mut output, "context_presentation")?,
            "payload",
            presentation.clone(),
        )?;
    }
    let registries = owner_mut(&mut output, "semantic_registries")?;
    owner_set(
        registries,
        "properties",
        inputs
            .entity_registry
            .object_get("property_definitions")
            .cloned()
            .unwrap_or(JsonValue::Array(Vec::new())),
    )?;
    let entity_types = owner_mut(registries, "entity_types")?;
    let entity_entries = owner_entry_counts(
        &inputs.entity_registry,
        "types",
        "type_id",
        entity_types
            .object_get("entries")
            .ok_or(Error::Invalid("catalog entity entries"))?,
        &["instance_count"],
    )?;
    owner_set(entity_types, "entries", entity_entries)?;
    let relation_types = owner_mut(registries, "relation_types")?;
    let relation_entries = owner_entry_counts(
        &inputs.relation_registry,
        "relations",
        "relation_type_id",
        relation_types
            .object_get("entries")
            .ok_or(Error::Invalid("catalog relation entries"))?,
        &[
            "edge_instance_count",
            "claim_instance_count",
            "instance_count",
        ],
    )?;
    owner_set(relation_types, "entries", relation_entries)?;
    Ok(output)
}

fn owner_required<'a>(value: &'a JsonValue, key: &str) -> Result<&'a JsonValue> {
    value
        .object_get(key)
        .ok_or(Error::Invalid("catalog missing row field"))
}
fn owner_truthy(value: &JsonValue) -> bool {
    match value {
        JsonValue::Null => false,
        JsonValue::Bool(value) => *value,
        JsonValue::String(value) => !value.units().is_empty(),
        JsonValue::Array(value) => !value.is_empty(),
        JsonValue::Object(value) => !value.is_empty(),
        JsonValue::Number(value) => value
            .lexeme
            .parse::<f64>()
            .map(|n| n != 0.0)
            .unwrap_or(true),
    }
}
