//! Source-compatible semantic validation kernels for the prepared index.
//!
//! Registry problems are retained as report values. They do not prevent row
//! evaluation: this layer computes mechanical diagnostics and never admits
//! semantic meaning.

use crate::{Error, Result};
use serde_json::{Value, json};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Deref;
use std::rc::Rc;
use tos_foundation::{JsonString, JsonValue};

const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Clone, Debug)]
pub(crate) struct SemanticCarrier {
    pub(crate) view: Value,
    pub(crate) original: JsonValue,
}

impl Deref for SemanticCarrier {
    type Target = Value;

    fn deref(&self) -> &Self::Target {
        &self.view
    }
}

pub(crate) trait SemanticLookup {
    fn node(&mut self, id: &str) -> Result<Option<SemanticCarrier>>;
    fn claim(&mut self, id: &str) -> Result<Option<SemanticCarrier>>;
    fn outgoing(&mut self, id: &str, predicate: &str) -> Result<Vec<Value>>;
    fn diagnostic(&mut self, value: &Value) -> Result<()>;
}

/// Python `str` for a scalar or order-independent `serde_json::Value`.
/// Callers with compound owner input pass its original `JsonValue` instead.
pub(crate) fn python_str(value: &Value) -> Result<String> {
    let raw = serde_json::to_vec(value).map_err(|error| Error::Source(error.to_string()))?;
    let parsed = crate::d1_public_capture::json(&raw, raw.len().max(1))?;
    crate::local_prepared::python_value_string(&parsed)
}

fn python_repr_value(value: &Value) -> Result<String> {
    let raw = serde_json::to_vec(value).map_err(|error| Error::Source(error.to_string()))?;
    let parsed = crate::d1_public_capture::json(&raw, raw.len().max(1))?;
    python_repr_raw(&parsed)
}

fn python_repr_raw(value: &JsonValue) -> Result<String> {
    if let JsonValue::String(_) = value {
        let wrapped = JsonValue::Array(vec![value.clone()]);
        let representation = crate::local_prepared::python_value_string(&wrapped)?;
        return representation
            .strip_prefix('[')
            .and_then(|value| value.strip_suffix(']'))
            .map(str::to_owned)
            .ok_or(Error::Invalid("Python string representation"));
    }
    crate::local_prepared::python_value_string(value)
}

fn python_string_repr(value: &str) -> Result<String> {
    python_repr_value(&Value::String(value.to_owned()))
}

fn python_raw_str(value: &JsonValue) -> Result<String> {
    crate::local_prepared::python_value_string(value)
}

fn python_raw_repr(value: &JsonValue) -> Result<String> {
    python_repr_raw(value)
}

fn python_iter_values(value: &JsonValue) -> Result<Vec<JsonValue>> {
    match value {
        JsonValue::Array(values) => Ok(values.clone()),
        JsonValue::String(value) => Ok(value
            .as_str()
            .ok_or(Error::Invalid("semantic evidence string"))?
            .chars()
            .map(|character| JsonValue::String(JsonString::from_utf8(&character.to_string())))
            .collect()),
        JsonValue::Object(values) => Ok(values
            .iter()
            .map(|(key, _)| JsonValue::String(key.clone()))
            .collect()),
        JsonValue::Null | JsonValue::Bool(_) | JsonValue::Number(_) => {
            Err(Error::Invalid("semantic evidence ids are not iterable"))
        }
    }
}

fn python_strip(value: &str) -> &str {
    tos_foundation::python_strip_unicode16_v1(value, value.chars().count()).unwrap_or(value)
}

fn string(value: Option<&Value>) -> Option<&str> {
    value
        .and_then(Value::as_str)
        .map(python_strip)
        .filter(|text| !text.is_empty())
}

fn strings(value: Option<&Value>) -> Vec<&str> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|value| !value.is_empty())
        .collect()
}

fn objects(value: Option<&Value>) -> Vec<&Value> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(Value::is_object)
        .collect()
}

fn truthy(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => false,
        Some(Value::Bool(value)) => *value,
        Some(Value::Number(value)) => {
            value.as_f64().map(|value| value != 0.0).unwrap_or_else(|| {
                value
                    .to_string()
                    .bytes()
                    .any(|byte| (b'1'..=b'9').contains(&byte))
            })
        }
        Some(Value::String(value)) => !value.is_empty(),
        Some(Value::Array(value)) => !value.is_empty(),
        Some(Value::Object(value)) => !value.is_empty(),
    }
}

fn python_or_object<'a>(
    value: Option<&'a Value>,
    label: &'static str,
) -> Result<Option<&'a Value>> {
    let Some(value) = value else { return Ok(None) };
    if !truthy(Some(value)) {
        return Ok(None);
    }
    if value.is_object() {
        Ok(Some(value))
    } else {
        Err(Error::Invalid(label))
    }
}

fn exact_int(value: Option<&Value>) -> Option<i64> {
    match value {
        Some(Value::Number(value)) => value
            .as_i64()
            .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok())),
        Some(Value::Bool(value)) => Some(i64::from(*value)),
        _ => None,
    }
}

fn python_tuple2(first: &str, second: &str) -> Result<String> {
    Ok(format!(
        "({}, {})",
        python_string_repr(first)?,
        python_string_repr(second)?
    ))
}

fn python_tuple3(first: &str, second: &str, third: &str) -> Result<String> {
    Ok(format!(
        "({}, {}, {})",
        python_string_repr(first)?,
        python_string_repr(second)?,
        python_string_repr(third)?
    ))
}

fn registry_items<'a>(registry: &'a Value, key: &str) -> Vec<&'a Value> {
    objects(registry.get(key))
}

fn ordered_registry_entries<'a>(
    registry: &'a Value,
    collection: &str,
    id_field: &str,
    trim_id: bool,
) -> Vec<(String, &'a Value)> {
    let mut positions = BTreeMap::<String, usize>::new();
    let mut ordered = Vec::<(String, &'a Value)>::new();
    for item in registry_items(registry, collection) {
        let Some(raw_id) = item.get(id_field) else {
            continue;
        };
        if string(Some(raw_id)).is_none() {
            continue;
        }
        let id = if trim_id {
            string(Some(raw_id)).unwrap().to_owned()
        } else {
            raw_id.as_str().unwrap().to_owned()
        };
        if let Some(index) = positions.get(&id).copied() {
            // Python dict assignment replaces the value without moving its
            // original insertion position.
            ordered[index].1 = item;
        } else {
            positions.insert(id.clone(), ordered.len());
            ordered.push((id, item));
        }
    }
    ordered
}

fn entity_registry_indexes<'a>(
    registry: &'a Value,
) -> (
    BTreeMap<String, &'a Value>,
    BTreeMap<(String, String), String>,
    String,
) {
    let mut entries = BTreeMap::new();
    let mut mappings = BTreeMap::new();
    let ordered = ordered_registry_entries(registry, "types", "type_id", false);
    for (type_id, entry) in &ordered {
        entries.insert(type_id.clone(), *entry);
    }
    for (type_id, entry) in &ordered {
        for mapping in objects(entry.get("source_mappings")) {
            if let (Some(source_graph), Some(source_kind_id)) = (
                string(mapping.get("source_graph")),
                string(mapping.get("source_kind_id")),
            ) {
                mappings.insert(
                    (source_graph.to_owned(), source_kind_id.to_owned()),
                    type_id.clone(),
                );
            }
        }
    }
    let fallback = string(registry.get("fallback_type_id"))
        .unwrap_or("tos.entity.unmapped")
        .to_owned();
    (entries, mappings, fallback)
}

fn relation_registry_indexes<'a>(
    registry: &'a Value,
) -> (
    BTreeMap<String, &'a Value>,
    BTreeMap<(String, String, String), String>,
    String,
) {
    let mut entries = BTreeMap::new();
    let mut mappings = BTreeMap::new();
    let ordered = ordered_registry_entries(registry, "relations", "relation_type_id", false);
    for (type_id, entry) in &ordered {
        entries.insert(type_id.clone(), *entry);
    }
    for (type_id, entry) in &ordered {
        for mapping in objects(entry.get("source_mappings")) {
            if let (Some(source_graph), Some(source_predicate_id), Some(scope)) = (
                string(mapping.get("source_graph")),
                string(mapping.get("source_predicate_id")),
                string(mapping.get("scope")),
            ) {
                mappings.insert(
                    (
                        source_graph.to_owned(),
                        source_predicate_id.to_owned(),
                        scope.to_owned(),
                    ),
                    type_id.clone(),
                );
            }
        }
    }
    let fallback = string(registry.get("fallback_relation_type_id"))
        .unwrap_or("tos.relation.unmapped")
        .to_owned();
    (entries, mappings, fallback)
}

#[derive(Clone)]
pub(crate) struct Kernel<'a> {
    entity_registry: &'a Value,
    relation_registry: &'a Value,
    entity_registry_original: &'a JsonValue,
    relation_registry_original: &'a JsonValue,
    entity_entries: Rc<BTreeMap<String, &'a Value>>,
    entity_mappings: Rc<BTreeMap<(String, String), String>>,
    fallback_type_id: String,
    relation_entries: Rc<BTreeMap<String, &'a Value>>,
    relation_mappings: Rc<BTreeMap<(String, String, String), String>>,
    fallback_relation_type_id: String,
    applicable_properties: Rc<RefCell<BTreeMap<Option<String>, Vec<&'a Value>>>>,
    pub(crate) registry_violations: Vec<Value>,
}

impl<'a> Kernel<'a> {
    pub(crate) fn new(
        entity_registry: &'a Value,
        relation_registry: &'a Value,
        entity_registry_original: &'a JsonValue,
        relation_registry_original: &'a JsonValue,
    ) -> Result<Self> {
        let report = validate_semantic_registries(
            entity_registry,
            relation_registry,
            entity_registry_original,
            relation_registry_original,
        )?;
        let registry_violations = report
            .get("violations")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let (entity_entries, entity_mappings, fallback_type_id) =
            entity_registry_indexes(entity_registry);
        let (relation_entries, relation_mappings, fallback_relation_type_id) =
            relation_registry_indexes(relation_registry);
        Ok(Self {
            entity_registry,
            relation_registry,
            entity_registry_original,
            relation_registry_original,
            entity_entries: Rc::new(entity_entries),
            entity_mappings: Rc::new(entity_mappings),
            fallback_type_id,
            relation_entries: Rc::new(relation_entries),
            relation_mappings: Rc::new(relation_mappings),
            fallback_relation_type_id,
            applicable_properties: Rc::new(RefCell::new(BTreeMap::new())),
            registry_violations,
        })
    }

    pub(crate) fn relation_entry(&self, id: &str) -> Option<&Value> {
        self.relation_entries.get(id).copied()
    }

    pub(crate) fn summarize(&self, kind: &str, item: &Value) -> [i64; 6] {
        let mut counts = [0; 6];
        if kind == "node" {
            counts[0] = i64::from(
                string(item.get("type_id")).is_some_and(|id| self.entity_entries.contains_key(id)),
            );
            counts[1] = i64::from(item.get("type_id").is_some_and(|value| {
                python_eq(value, &Value::String(self.fallback_type_id.clone())).unwrap_or(false)
            }));
        } else if kind == "relation" {
            counts[2] = i64::from(
                string(item.get("relation_type_id"))
                    .is_some_and(|id| self.relation_entries.contains_key(id)),
            );
            counts[3] = i64::from(item.get("relation_type_id").is_some_and(|value| {
                python_eq(
                    value,
                    &Value::String(self.fallback_relation_type_id.clone()),
                )
                .unwrap_or(false)
            }));
            counts[5] = i64::from(item.get("source_graph").is_some_and(|value| {
                python_eq(value, &Value::String("semantic-interchange".to_owned())).unwrap_or(false)
            }));
        }
        counts
    }

    pub(crate) fn evaluate(
        &self,
        kind: &str,
        item: &Value,
        original: &JsonValue,
        lookup: &mut impl SemanticLookup,
    ) -> Result<(Vec<Value>, Vec<Value>, i64)> {
        let mut violations = Vec::new();
        let mut gaps = Vec::new();
        let mut claim_count = 0;
        match kind {
            "node" => {
                self.validate_node(item, original, lookup, &mut violations)?;
                if item.get("type_id").is_some_and(|value| {
                    python_eq(value, &Value::String("tos.entity.claim".to_owned())).unwrap_or(false)
                }) {
                    let (mut errors, missing, count) =
                        self.validate_claim(item, original, lookup)?;
                    for error in &errors {
                        // The Python wrapper extends the shared diagnostics
                        // list after the Claim-local diagnostics list charged
                        // each error once.
                        lookup.diagnostic(error)?;
                    }
                    violations.append(&mut errors);
                    gaps.extend(missing);
                    claim_count = count;
                }
            }
            "relation" => {
                let (errors, missing) = self.validate_relation(item, original, lookup)?;
                violations.extend(errors);
                gaps.extend(missing);
                if string(item.get("id")).unwrap_or("<missing relation id>")
                    == "<missing relation id>"
                    && string(item.get("relation_type_id"))
                        .is_some_and(|id| self.relation_entries.contains_key(id))
                {
                    add_violation(
                        lookup,
                        &mut violations,
                        "duplicate or missing relation id <missing relation id>".to_owned(),
                    )?;
                }
            }
            _ => return Err(Error::Invalid("semantic item kind")),
        }
        Ok((violations, gaps, claim_count))
    }

    fn validate_node(
        &self,
        node: &Value,
        original: &JsonValue,
        lookup: &mut impl SemanticLookup,
        violations: &mut Vec<Value>,
    ) -> Result<()> {
        let node_id = string(node.get("id"));
        let type_id = string(node.get("type_id"));
        let mapping = node
            .get("type_mapping")
            .filter(|v| v.is_object())
            .unwrap_or(&Value::Null);
        let Some(node_id) = node_id else {
            add_violation(lookup, violations, "knowledge node has no id".to_owned())?;
            return Ok(());
        };
        if !type_id.is_some_and(|id| self.entity_entries.contains_key(id)) {
            let type_repr = match type_id {
                Some(id) => python_string_repr(id)?,
                None => "None".to_owned(),
            };
            add_violation(
                lookup,
                violations,
                format!("node {node_id} uses unregistered type {type_repr}"),
            )?;
        } else if type_id
            .and_then(|id| self.entity_entries.get(id))
            .is_some_and(|entry| truthy(entry.get("abstract")))
        {
            add_violation(
                lookup,
                violations,
                format!(
                    "node {node_id} instantiates abstract type {}",
                    type_id.unwrap()
                ),
            )?;
        }
        let source_graph = python_original_path(original, "source_graph")?;
        let source_kind_id = python_original_path(original, "type_mapping.source_kind_id")?;
        let expected_type = self
            .entity_mappings
            .get(&(source_graph, source_kind_id))
            .map(String::as_str)
            .unwrap_or(&self.fallback_type_id);
        if type_id != Some(expected_type) {
            add_violation(
                lookup,
                violations,
                format!("node {node_id} disagrees with its registered source type mapping"),
            )?;
        }
        let expected_status = if type_id == Some(&self.fallback_type_id) {
            "unmapped"
        } else {
            "mapped"
        };
        if mapping.get("status").and_then(Value::as_str) != Some(expected_status) {
            add_violation(
                lookup,
                violations,
                format!("node {node_id} has inconsistent type mapping status"),
            )?;
        }
        if string(mapping.get("source_kind_id")).is_none() {
            add_violation(
                lookup,
                violations,
                format!("node {node_id} does not preserve source_kind_id"),
            )?;
        }
        if string(node.get("entity_id")).is_none() {
            add_violation(
                lookup,
                violations,
                format!("node {node_id} has no stable entity_id or representation fallback"),
            )?;
        }
        if strings(node.get("source_refs")).is_empty() {
            add_violation(
                lookup,
                violations,
                format!("node {node_id} has no source_refs"),
            )?;
        }
        for definition in self.applicable_properties(type_id)? {
            let field = definition
                .get("field")
                .and_then(Value::as_str)
                .ok_or(Error::Invalid("semantic property field"))?;
            let value = field_value(node, field).filter(|value| !value.is_null());
            if value.is_none() {
                if truthy(definition.get("required")) {
                    let property_id = self.property_diagnostic_id(definition)?;
                    add_violation(
                        lookup,
                        violations,
                        format!("node {node_id} lacks required property {property_id}"),
                    )?;
                }
                continue;
            }
            let value = value.unwrap();
            let valid = match definition.get("value_type").and_then(Value::as_str) {
                Some("string") => value.is_string(),
                Some("boolean") => value.is_boolean(),
                Some("number") => value.is_number(),
                Some("string-array") => value
                    .as_array()
                    .is_some_and(|values| values.iter().all(Value::is_string)),
                _ => false,
            };
            if !valid {
                let property_id = self.property_diagnostic_id(definition)?;
                add_violation(
                    lookup,
                    violations,
                    format!("node {node_id} has invalid property {property_id}"),
                )?;
            }
        }
        let semantics = node
            .get("semantics")
            .filter(|v| v.is_object())
            .unwrap_or(&Value::Null);
        match type_id {
            Some("tos.entity.temporal-assertion") => {
                let time = semantics
                    .get("time")
                    .filter(|v| v.is_object())
                    .unwrap_or(&Value::Null);
                if !matches!(
                    time.get("normalization_status").and_then(Value::as_str),
                    Some("structured-source" | "source-literal-parsed" | "source-literal-unparsed")
                ) {
                    add_violation(
                        lookup,
                        violations,
                        format!(
                            "temporal assertion {node_id} is not backed by an explicit source time value"
                        ),
                    )?;
                }
            }
            Some("tos.entity.place") => {
                let space = semantics
                    .get("space")
                    .filter(|v| v.is_object())
                    .unwrap_or(&Value::Null);
                if space.get("kind").and_then(Value::as_str) != Some("place-identity") {
                    add_violation(
                        lookup,
                        violations,
                        format!("Place {node_id} is missing place-identity semantics"),
                    )?;
                }
            }
            Some("tos.entity.navigation-region") => {
                let space = semantics
                    .get("space")
                    .filter(|v| v.is_object())
                    .unwrap_or(&Value::Null);
                if space.get("kind").and_then(Value::as_str) != Some("navigation-region") {
                    add_violation(
                        lookup,
                        violations,
                        format!("navigation Region {node_id} is missing its non-Place marker"),
                    )?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn applicable_properties(&self, type_id: Option<&str>) -> Result<Vec<&Value>> {
        let cache_key = type_id.map(str::to_owned);
        if let Some(values) = self.applicable_properties.borrow().get(&cache_key) {
            return Ok(values.clone());
        }
        let definitions = match self.entity_registry.get("property_definitions") {
            None => Vec::new(),
            Some(Value::Array(values)) => values.iter().collect::<Vec<_>>(),
            Some(Value::String(_)) | Some(Value::Object(_)) => {
                return Err(Error::Invalid(
                    "semantic property definitions are not object rows",
                ));
            }
            Some(Value::Null | Value::Bool(_) | Value::Number(_)) => {
                return Err(Error::Invalid(
                    "semantic property definitions are not iterable",
                ));
            }
        };
        if definitions.iter().any(|definition| !definition.is_object()) {
            return Err(Error::Invalid(
                "semantic property definition is not an object",
            ));
        }
        let values = definitions
            .into_iter()
            .map(|definition| {
                property_applies_to(type_id, definition, &self.entity_entries)
                    .map(|applies| applies.then_some(definition))
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        self.applicable_properties
            .borrow_mut()
            .insert(cache_key, values.clone());
        Ok(values)
    }

    fn original_property_definition(&self, definition: &Value) -> Option<&JsonValue> {
        let index = self
            .entity_registry
            .get("property_definitions")?
            .as_array()?
            .iter()
            .position(|candidate| std::ptr::eq(candidate, definition))?;
        self.entity_registry_original
            .object_get("property_definitions")?
            .as_array()?
            .get(index)
    }

    fn property_diagnostic_id(&self, definition: &Value) -> Result<String> {
        let original = self
            .original_property_definition(definition)
            .ok_or(Error::Invalid("semantic property original carrier"))?;
        original
            .object_get("property_id")
            .map(python_raw_str)
            .transpose()?
            .ok_or(Error::Invalid("semantic property id"))
    }

    fn validate_endpoints(
        &self,
        owner_id: &str,
        relation_type_id: &str,
        from_node: Option<&Value>,
        to_node: Option<&Value>,
        lookup: &mut impl SemanticLookup,
        violations: &mut Vec<Value>,
    ) -> Result<()> {
        let Some(entry) = self.relation_entries.get(relation_type_id) else {
            let relation_type_repr = python_string_repr(relation_type_id)?;
            add_violation(
                lookup,
                violations,
                format!("{owner_id} uses unregistered relation type {relation_type_repr}"),
            )?;
            return Ok(());
        };
        if relation_type_id == self.fallback_relation_type_id {
            return Ok(());
        }
        if from_node.is_none() || to_node.is_none() {
            add_violation(
                lookup,
                violations,
                format!("{owner_id} has an unresolved normalized endpoint"),
            )?;
            return Ok(());
        }
        let from_type = string(from_node.and_then(|v| v.get("type_id")));
        let to_type = string(to_node.and_then(|v| v.get("type_id")));
        if !type_is_a(
            from_type,
            &strings(entry.get("domain_type_ids")),
            &self.entity_entries,
        ) {
            let domain = registry_raw_entry(
                self.relation_registry_original,
                "relations",
                "relation_type_id",
                relation_type_id,
            )
            .map(|entry| python_original_path(entry, "domain_type_ids"))
            .transpose()?
            .unwrap_or(python_str(
                entry.get("domain_type_ids").unwrap_or(&Value::Null),
            )?);
            let from_repr = match from_type {
                Some(value) => python_string_repr(value)?,
                None => "None".to_owned(),
            };
            add_violation(
                lookup,
                violations,
                format!("{owner_id} domain {from_repr} is outside {domain}"),
            )?;
        }
        if !type_is_a(
            to_type,
            &strings(entry.get("range_type_ids")),
            &self.entity_entries,
        ) {
            let range = registry_raw_entry(
                self.relation_registry_original,
                "relations",
                "relation_type_id",
                relation_type_id,
            )
            .map(|entry| python_original_path(entry, "range_type_ids"))
            .transpose()?
            .unwrap_or(python_str(
                entry.get("range_type_ids").unwrap_or(&Value::Null),
            )?);
            let to_repr = match to_type {
                Some(value) => python_string_repr(value)?,
                None => "None".to_owned(),
            };
            add_violation(
                lookup,
                violations,
                format!("{owner_id} range {to_repr} is outside {range}"),
            )?;
        }
        Ok(())
    }

    fn validate_relation(
        &self,
        relation: &Value,
        original: &JsonValue,
        lookup: &mut impl SemanticLookup,
    ) -> Result<(Vec<Value>, Vec<Value>)> {
        let mut violations = Vec::new();
        let mut gaps = Vec::new();
        let relation_id = string(relation.get("id")).unwrap_or("<missing relation id>");
        let relation_type_id = string(relation.get("relation_type_id"));
        let mapping = relation
            .get("predicate_mapping")
            .filter(|v| v.is_object())
            .unwrap_or(&Value::Null);
        let Some(relation_type_id) = relation_type_id else {
            add_violation(
                lookup,
                &mut violations,
                format!("relation {relation_id} uses unregistered type None"),
            )?;
            return Ok((violations, gaps));
        };
        let Some(entry) = self.relation_entries.get(relation_type_id) else {
            let relation_type_repr = python_string_repr(relation_type_id)?;
            add_violation(
                lookup,
                &mut violations,
                format!("relation {relation_id} uses unregistered type {relation_type_repr}"),
            )?;
            return Ok((violations, gaps));
        };
        if truthy(entry.get("abstract")) {
            add_violation(
                lookup,
                &mut violations,
                format!("relation {relation_id} instantiates abstract type {relation_type_id}"),
            )?;
        }
        if truthy(entry.get("evidence_required")) && strings(relation.get("source_refs")).is_empty()
        {
            add_violation(
                lookup,
                &mut violations,
                format!("relation {relation_id} has no evidence-bearing source ref"),
            )?;
        }
        let epistemic = python_or_object(
            relation.get("epistemic"),
            "semantic relation epistemic object",
        )?;
        let posture = epistemic.and_then(|epistemic| epistemic.get("review_posture"));
        if entry.get("review_requirement") != Some(&Value::String("none".to_owned()))
            && !truthy(posture)
        {
            add_violation(
                lookup,
                &mut violations,
                format!("relation {relation_id} has no recorded review state"),
            )?;
        }
        if posture.and_then(Value::as_str) == Some("not-recorded") {
            add_gap(lookup, &mut gaps, relation_id, "review-not-recorded")?;
        }
        let expected_status = if relation_type_id == self.fallback_relation_type_id {
            "unmapped"
        } else {
            "mapped"
        };
        if mapping.get("status").and_then(Value::as_str) != Some(expected_status) {
            add_violation(
                lookup,
                &mut violations,
                format!("relation {relation_id} has inconsistent predicate mapping status"),
            )?;
        }
        if string(mapping.get("source_predicate_id")).is_none() {
            add_violation(
                lookup,
                &mut violations,
                format!("relation {relation_id} does not preserve source_predicate_id"),
            )?;
        }
        let map_key = (
            python_original_path(original, "source_graph")?,
            python_original_path(original, "predicate_mapping.source_predicate_id")?,
            "edge".to_owned(),
        );
        let expected_type = self
            .relation_mappings
            .get(&map_key)
            .map(String::as_str)
            .unwrap_or(&self.fallback_relation_type_id);
        if relation_type_id != expected_type {
            add_violation(
                lookup,
                &mut violations,
                format!("relation {relation_id} disagrees with its registered source mapping"),
            )?;
        }
        let left_id = python_original_path(original, "from_id")?;
        let right_id = python_original_path(original, "to_id")?;
        let left = lookup.node(&left_id)?;
        let right = lookup.node(&right_id)?;
        self.validate_endpoints(
            relation_id,
            relation_type_id,
            left.as_ref().map(|carrier| &carrier.view),
            right.as_ref().map(|carrier| &carrier.view),
            lookup,
            &mut violations,
        )?;
        if entry.get("assertion_mode").and_then(Value::as_str) == Some("reified-claim") {
            let _attributes = python_or_object(
                relation.get("attributes"),
                "semantic relation attributes object",
            )?;
            let claim_ref = python_original_path(original, "attributes.claim_ref")?;
            let supporting_claim = lookup.claim(&claim_ref)?;
            if supporting_claim.is_none() {
                add_violation(
                    lookup,
                    &mut violations,
                    format!("relation {relation_id} lacks a resolved supporting Claim"),
                )?;
            } else if let (Some(left), Some(right), Some(supporting_claim)) =
                (left.as_ref(), right.as_ref(), supporting_claim.as_ref())
            {
                if left.get("type_id").and_then(Value::as_str) != Some("tos.entity.claim") {
                    let contract = get_path(&supporting_claim.view, "semantics.claim");
                    if !python_optional_eq(
                        left.get("entity_id"),
                        contract.and_then(|v| v.get("subject_entity_id")),
                    ) || !python_optional_eq(
                        right.get("entity_id"),
                        contract.and_then(|v| v.get("object_entity_id")),
                    ) {
                        add_violation(
                            lookup,
                            &mut violations,
                            format!(
                                "relation {relation_id} disagrees with supporting Claim endpoints"
                            ),
                        )?;
                    }
                }
            }
        }
        if relation_type_id == "tos.relation.projects" {
            if let (Some(left), Some(right)) = (left.as_ref(), right.as_ref()) {
                if !python_optional_eq(left.get("entity_id"), right.get("entity_id")) {
                    add_violation(
                        lookup,
                        &mut violations,
                        format!(
                            "projection relation {relation_id} connects different entity_id values"
                        ),
                    )?;
                }
            }
        }
        if relation_type_id == "tos.relation.promotion-basis-version" {
            if let (Some(left), Some(right)) = (left.as_ref(), right.as_ref()) {
                let candidate =
                    get_path(left, "attributes.source_record.promotion_basis.candidate");
                let reference = get_path(right, "semantics.record_version.record_ref");
                if !exact_form_ref(candidate)
                    || !exact_form_ref(reference)
                    || exact_record_digest(candidate)? != exact_record_digest(reference)?
                {
                    add_violation(
                        lookup,
                        &mut violations,
                        format!(
                            "promotion basis relation {relation_id} differs from the exact Sign candidate"
                        ),
                    )?;
                }
            }
        }
        if relation_type_id == "tos.relation.has-record-version" {
            if let (Some(left), Some(right)) = (left.as_ref(), right.as_ref()) {
                let references = metadata_history_refs(left)?;
                let version = get_path(right, "semantics.record_version");
                let reference = version.and_then(|v| v.get("record_ref"));
                let matches = references.as_ref().is_some_and(|references| {
                    references.iter().any(|candidate| {
                        exact_form_ref(reference)
                            && exact_record_digest(reference).ok()
                                == exact_record_digest(Some(candidate)).ok()
                    })
                });
                if references.is_none()
                    || version.is_none_or(|v| {
                        v.get("record_kind").and_then(Value::as_str) != Some("metadata")
                    })
                    || !exact_form_ref(reference)
                    || !matches
                {
                    add_violation(
                        lookup,
                        &mut violations,
                        format!(
                            "record history relation {relation_id} is not bound to the exact source history"
                        ),
                    )?;
                }
            }
        }
        if relation_type_id == "tos.relation.same-as" {
            let _attributes = python_or_object(
                relation.get("attributes"),
                "semantic relation attributes object",
            )?;
            if let (Some(left), Some(right)) = (left.as_ref(), right.as_ref()) {
                let left_type = python_original_path(&left.original, "type_id")?;
                let right_type = python_original_path(&right.original, "type_id")?;
                if !type_is_a(
                    Some(&left_type),
                    &[right_type.as_str()],
                    &self.entity_entries,
                ) && !type_is_a(
                    Some(&right_type),
                    &[left_type.as_str()],
                    &self.entity_entries,
                ) {
                    add_violation(
                        lookup,
                        &mut violations,
                        format!(
                            "same_as relation {relation_id} connects incompatible entity types"
                        ),
                    )?;
                }
            }
            if strings(relation.get("source_refs")).is_empty() {
                add_violation(
                    lookup,
                    &mut violations,
                    format!("same_as relation {relation_id} has no evidence-bearing source ref"),
                )?;
            }
            if !matches!(
                posture.and_then(Value::as_str),
                Some("accepted" | "verified" | "reviewed_equivalence")
            ) {
                add_violation(
                    lookup,
                    &mut violations,
                    format!("same_as relation {relation_id} lacks accepted review posture"),
                )?;
            }
            let claim_ref = python_original_path(original, "attributes.claim_ref")?;
            let claim_node = lookup.claim(&claim_ref)?;
            let claim = claim_node
                .as_ref()
                .and_then(|v| get_path(&v.view, "semantics.claim"));
            let review_id = python_original_path(original, "attributes.review_node_id")?;
            let review = lookup.node(&review_id)?;
            let review_data = review.as_ref().and_then(|v| v.view.get("attributes"));
            let exact_pair = match (left.as_ref(), right.as_ref(), claim) {
                (Some(left), Some(right), Some(claim)) => python_pair_set_eq(
                    left.get("entity_id").unwrap_or(&Value::Null),
                    right.get("entity_id").unwrap_or(&Value::Null),
                    claim.get("subject_entity_id").unwrap_or(&Value::Null),
                    claim.get("object_entity_id").unwrap_or(&Value::Null),
                )?,
                _ => false,
            };
            let reviewed_exact_claim = match (review.as_ref(), review_data, claim) {
                (Some(review), Some(data), Some(claim)) => {
                    let review_type = python_original_path(&review.original, "type_id")?;
                    type_is_a(
                        Some(&review_type),
                        &["tos.entity.review"],
                        &self.entity_entries,
                    ) && python_optional_eq(data.get("claim_ref"), claim.get("claim_id"))
                        && python_optional_eq(data.get("claim_version"), claim.get("claim_version"))
                        && data.get("decision").and_then(Value::as_str) == Some("accepted")
                }
                _ => false,
            };
            let evidence_value = claim.and_then(|value| value.get("evidence_node_ids"));
            let has_evidence = evidence_value.is_some_and(|value| truthy(Some(value)));
            let raw_evidence = claim_node.as_ref().and_then(|carrier| {
                original_path(&carrier.original, "semantics.claim.evidence_node_ids")
            });
            let eligible = exact_pair
                && claim
                    .and_then(|v| v.get("relation_type_id"))
                    .and_then(Value::as_str)
                    == Some(relation_type_id)
                && claim
                    .and_then(|v| v.get("claim_version"))
                    .is_some_and(|version| !version.is_null())
                && reviewed_exact_claim
                && has_evidence;
            let mut resolved_evidence = false;
            if eligible {
                let evidence_ids = raw_evidence
                    .map(python_iter_values)
                    .transpose()?
                    .unwrap_or_default();
                resolved_evidence = true;
                for evidence_id in evidence_ids {
                    let Some(id) = evidence_id.as_str() else {
                        if matches!(evidence_id, JsonValue::Array(_) | JsonValue::Object(_)) {
                            return Err(Error::Invalid("unhashable same-as evidence id"));
                        }
                        resolved_evidence = false;
                        break;
                    };
                    if lookup.node(id)?.is_none() {
                        resolved_evidence = false;
                        break;
                    }
                    let evidence = lookup.node(id)?;
                    let Some(evidence) = evidence else {
                        resolved_evidence = false;
                        break;
                    };
                    let evidence_type = python_original_path(&evidence.original, "type_id")?;
                    if !type_is_a(
                        Some(&evidence_type),
                        &["tos.entity.evidence"],
                        &self.entity_entries,
                    ) {
                        resolved_evidence = false;
                        break;
                    }
                }
            }
            if !eligible || !resolved_evidence {
                add_violation(
                    lookup,
                    &mut violations,
                    format!(
                        "same_as relation {relation_id} lacks resolved evidence and exact-version review"
                    ),
                )?;
            }
        }
        Ok((violations, gaps))
    }

    fn validate_claim(
        &self,
        node: &Value,
        original: &JsonValue,
        lookup: &mut impl SemanticLookup,
    ) -> Result<(Vec<Value>, Vec<Value>, i64)> {
        let mut violations = Vec::new();
        let mut gaps = Vec::new();
        let semantics = node
            .get("semantics")
            .filter(|v| v.is_object())
            .unwrap_or(&Value::Null);
        let claim = semantics.get("claim").filter(|v| v.is_object());
        if claim.is_none_or(|claim| {
            !truthy(claim.get("subject_node_id")) || !truthy(claim.get("object_node_id"))
        }) {
            add_violation(
                lookup,
                &mut violations,
                format!(
                    "claim {} lacks its subject/object contract",
                    python_original_path(original, "id")?
                ),
            )?;
            return Ok((violations, gaps, 0));
        }
        let claim = claim.unwrap();
        let node_id = python_original_path(original, "id")?;
        for (predicate, field) in [
            ("tos.relation.has-subject", "subject_node_id"),
            ("tos.relation.has-object", "object_node_id"),
        ] {
            let edges = lookup.outgoing(&node_id, predicate)?;
            if edges.len() != 1 || !python_optional_eq(edges[0].get("to_id"), claim.get(field)) {
                add_violation(
                    lookup,
                    &mut violations,
                    format!("claim {node_id} must have exactly one consistent {predicate}"),
                )?;
            }
        }
        let raw_evidence = original_path(original, "semantics.claim.evidence_node_ids");
        let evidence_ids = raw_evidence
            .map(python_iter_values)
            .transpose()?
            .unwrap_or_default();
        for evidence_id in &evidence_ids {
            let found = if let Some(id) = evidence_id.as_str() {
                lookup.node(id)?.is_some()
            } else if matches!(evidence_id, JsonValue::Array(_) | JsonValue::Object(_)) {
                return Err(Error::Invalid("unhashable Claim evidence id"));
            } else {
                false
            };
            if !found {
                let evidence_repr = python_raw_str(evidence_id)?;
                add_violation(
                    lookup,
                    &mut violations,
                    format!("claim {node_id} has unresolved evidence {evidence_repr}"),
                )?;
            }
        }
        if !claim
            .get("evidence_node_ids")
            .is_some_and(|value| truthy(Some(value)))
        {
            add_gap(lookup, &mut gaps, &node_id, "claim-evidence-not-projected")?;
        }
        let relation_type_id =
            string(claim.get("relation_type_id")).unwrap_or(&self.fallback_relation_type_id);
        let subject_id = python_original_path(original, "semantics.claim.subject_node_id")?;
        let object_id = python_original_path(original, "semantics.claim.object_node_id")?;
        let subject = lookup.node(&subject_id)?;
        let object = lookup.node(&object_id)?;
        self.validate_endpoints(
            &format!(
                "claim {}",
                python_original_path(original, "semantics.claim.claim_id")?
            ),
            relation_type_id,
            subject.as_ref().map(|carrier| &carrier.view),
            object.as_ref().map(|carrier| &carrier.view),
            lookup,
            &mut violations,
        )?;
        Ok((violations, gaps, 1))
    }
}

fn add_violation(
    lookup: &mut impl SemanticLookup,
    out: &mut Vec<Value>,
    message: String,
) -> Result<()> {
    let value = Value::String(message);
    lookup.diagnostic(&value)?;
    out.push(value);
    Ok(())
}

fn add_gap(
    lookup: &mut impl SemanticLookup,
    out: &mut Vec<Value>,
    id: &str,
    kind: &str,
) -> Result<()> {
    let value = json!({"id": id, "kind": kind});
    lookup.diagnostic(&value)?;
    out.push(value);
    Ok(())
}

fn field_value<'a>(item: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.')
        .try_fold(item, |value, part| value.get(part))
}

fn get_path<'a>(item: &'a Value, path: &str) -> Option<&'a Value> {
    field_value(item, path)
}

fn original_path<'a>(item: &'a JsonValue, path: &str) -> Option<&'a JsonValue> {
    path.split('.')
        .try_fold(item, |value, part| value.object_get(part))
}

fn python_original_path(item: &JsonValue, path: &str) -> Result<String> {
    original_path(item, path)
        .map(python_raw_str)
        .transpose()
        .map(|value| value.unwrap_or_else(|| "None".to_owned()))
}

fn registry_raw_entry<'a>(
    registry: &'a JsonValue,
    collection: &str,
    id_field: &str,
    identifier: &str,
) -> Option<&'a JsonValue> {
    registry
        .object_get(collection)?
        .as_array()?
        .iter()
        .rev()
        .find(|entry| {
            entry
                .object_get(id_field)
                .and_then(JsonValue::as_str)
                .is_some_and(|value| python_strip(value) == identifier)
        })
}

/// Python equality for JSON carriers, including the bool/int/float numeric
/// equivalences used by the source validator.
pub(crate) fn python_eq(left: &Value, right: &Value) -> Result<bool> {
    fn equal(left: &Value, right: &Value) -> bool {
        fn integer_text(value: &Value) -> Option<String> {
            let text = match value {
                Value::Bool(true) => "1".to_owned(),
                Value::Bool(false) => "0".to_owned(),
                Value::Number(number) => number.to_string(),
                _ => return None,
            };
            if text.contains(['.', 'e', 'E']) {
                return None;
            }
            Some(normalize_integer(&text))
        }

        fn normalize_integer(value: &str) -> String {
            let (negative, digits) = value
                .strip_prefix('-')
                .map(|digits| (true, digits))
                .unwrap_or((false, value));
            let digits = digits.trim_start_matches('0');
            if digits.is_empty() {
                "0".to_owned()
            } else if negative {
                format!("-{digits}")
            } else {
                digits.to_owned()
            }
        }

        fn float_value(value: &Value) -> Option<f64> {
            match value {
                Value::Number(number) if number.to_string().contains(['.', 'e', 'E']) => {
                    number.as_f64()
                }
                _ => None,
            }
        }

        fn numeric_equal(left: &Value, right: &Value) -> Option<bool> {
            if let (Some(left), Some(right)) = (integer_text(left), integer_text(right)) {
                return Some(left == right);
            }
            let left_integer = integer_text(left);
            let right_integer = integer_text(right);
            let left_float = float_value(left);
            let right_float = float_value(right);
            match (left_integer, right_integer, left_float, right_float) {
                (Some(integer), None, _, Some(float))
                    if float.is_finite() && float.fract() == 0.0 =>
                {
                    Some(integer == normalize_integer(&format!("{float:.0}")))
                }
                (None, Some(integer), Some(float), _)
                    if float.is_finite() && float.fract() == 0.0 =>
                {
                    Some(normalize_integer(&format!("{float:.0}")) == integer)
                }
                (None, None, Some(left), Some(right)) => Some(left == right),
                _ => None,
            }
        }

        if let Some(equal) = numeric_equal(left, right) {
            return equal;
        }
        match (left, right) {
            (Value::Null, Value::Null) => true,
            (Value::String(left), Value::String(right)) => left == right,
            (Value::Array(left), Value::Array(right)) => {
                left.len() == right.len()
                    && left
                        .iter()
                        .zip(right)
                        .all(|(left, right)| equal(left, right))
            }
            (Value::Object(left), Value::Object(right)) => {
                left.len() == right.len()
                    && left
                        .iter()
                        .all(|(key, left)| right.get(key).is_some_and(|right| equal(left, right)))
            }
            _ => false,
        }
    }

    Ok(equal(left, right))
}

fn python_optional_eq(left: Option<&Value>, right: Option<&Value>) -> bool {
    python_eq(left.unwrap_or(&Value::Null), right.unwrap_or(&Value::Null)).unwrap_or(false)
}

fn python_integer_text(value: &Value) -> Option<String> {
    let text = match value {
        Value::Bool(true) => "1".to_owned(),
        Value::Bool(false) => "0".to_owned(),
        Value::Number(number) => number.to_string(),
        _ => return None,
    };
    if text.contains(['.', 'e', 'E']) {
        return None;
    }
    Some(normalize_integer_text(&text))
}

fn normalize_integer_text(value: &str) -> String {
    let (negative, digits) = value
        .strip_prefix('-')
        .map(|digits| (true, digits))
        .unwrap_or((false, value));
    let digits = digits.trim_start_matches('0');
    if digits.is_empty() {
        "0".to_owned()
    } else if negative {
        format!("-{digits}")
    } else {
        digits.to_owned()
    }
}

fn compare_integer_text(left: &str, right: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let left = normalize_integer_text(left);
    let right = normalize_integer_text(right);
    let left_negative = left.starts_with('-');
    let right_negative = right.starts_with('-');
    match (left_negative, right_negative) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        (false, false) => left.len().cmp(&right.len()).then_with(|| left.cmp(&right)),
        (true, true) => {
            let left_digits = &left[1..];
            let right_digits = &right[1..];
            right_digits
                .len()
                .cmp(&left_digits.len())
                .then_with(|| right_digits.cmp(left_digits))
        }
    }
}

fn python_pair_set_eq(
    left_first: &Value,
    left_second: &Value,
    right_first: &Value,
    right_second: &Value,
) -> Result<bool> {
    if [left_first, left_second, right_first, right_second]
        .iter()
        .any(|value| matches!(value, Value::Array(_) | Value::Object(_)))
    {
        return Err(Error::Invalid("unhashable same-as entity identity"));
    }
    let left_duplicate = python_eq(left_first, left_second)?;
    let right_duplicate = python_eq(right_first, right_second)?;
    if left_duplicate != right_duplicate {
        return Ok(false);
    }
    let contains = |value: &Value, first: &Value, second: &Value, duplicate: bool| {
        python_eq(value, first).unwrap_or(false)
            || (!duplicate && python_eq(value, second).unwrap_or(false))
    };
    Ok(
        contains(left_first, right_first, right_second, right_duplicate)
            && contains(left_second, right_first, right_second, right_duplicate),
    )
}

fn type_is_a(
    type_id: Option<&str>,
    allowed_type_ids: &[&str],
    entity_entries: &BTreeMap<String, &Value>,
) -> bool {
    let Some(type_id) = type_id else { return false };
    let allowed: BTreeSet<&str> = allowed_type_ids.iter().copied().collect();
    let mut frontier = vec![type_id];
    let mut seen = BTreeSet::new();
    while let Some(current) = frontier.pop() {
        if allowed.contains(current) {
            return true;
        }
        if !seen.insert(current) {
            continue;
        }
        if let Some(entry) = entity_entries.get(current) {
            frontier.extend(strings(entry.get("parent_type_ids")));
        }
    }
    false
}

fn property_applies_to(
    type_id: Option<&str>,
    definition: &Value,
    entity_entries: &BTreeMap<String, &Value>,
) -> Result<bool> {
    let applies = definition
        .get("applies_to")
        .unwrap_or(&Value::Array(Vec::new()));
    if truthy(definition.get("inherited")) {
        let Some(type_id) = type_id else {
            return Ok(false);
        };
        let allowed_storage = match applies {
            Value::Array(values) => {
                if values
                    .iter()
                    .any(|value| matches!(value, Value::Array(_) | Value::Object(_)))
                {
                    return Err(Error::Invalid("unhashable inherited property owner"));
                }
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            }
            Value::Object(values) => values.keys().cloned().collect::<Vec<_>>(),
            Value::String(values) => values
                .chars()
                .map(|value| value.to_string())
                .collect::<Vec<_>>(),
            Value::Null | Value::Bool(_) | Value::Number(_) => {
                return Err(Error::Invalid("inherited property owners are not iterable"));
            }
        };
        let allowed = allowed_storage
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        return Ok(type_is_a(Some(type_id), &allowed, entity_entries));
    }
    let owner_value = type_id
        .map(|value| Value::String(value.to_owned()))
        .unwrap_or(Value::Null);
    match applies {
        Value::Array(values) => {
            for value in values {
                if python_eq(&owner_value, value)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        Value::Object(values) => Ok(type_id.is_some_and(|type_id| values.contains_key(type_id))),
        Value::String(values) => {
            let Some(type_id) = type_id else {
                return Err(Error::Invalid(
                    "property owner membership requires a string",
                ));
            };
            Ok(values.contains(type_id))
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {
            Err(Error::Invalid("property owners do not support membership"))
        }
    }
}

fn language_key(value: &str) -> bool {
    fn alphanumeric_nonempty(value: &str) -> bool {
        !value.is_empty() && value.bytes().all(|b| b.is_ascii_alphanumeric())
    }
    let mut parts = value.split('-');
    let Some(first) = parts.next() else {
        return false;
    };
    if !first.bytes().all(|b| b.is_ascii_alphabetic()) {
        return false;
    }
    let rest: Vec<&str> = parts.collect();
    if matches!(first, "i" | "I" | "x" | "X") {
        first.len() == 1
            && !rest.is_empty()
            && rest
                .iter()
                .all(|part| part.len() <= 8 && alphanumeric_nonempty(part))
    } else {
        (2..=8).contains(&first.len())
            && rest
                .iter()
                .all(|part| part.len() <= 8 && alphanumeric_nonempty(part))
    }
}

fn labels(value: Option<&Value>, languages: &BTreeSet<String>, maximum: usize) -> bool {
    let Some(object) = value.and_then(Value::as_object) else {
        return false;
    };
    object.len() == languages.len()
        && languages
            .iter()
            .all(|language| object.contains_key(language))
        && object.values().all(|label| {
            label.as_str().is_some_and(|label| {
                !python_strip(label).is_empty() && label.chars().count() <= maximum
            })
        })
}

fn exact_object_fields(value: &Value, fields: &[&str]) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    object.len() == fields.len() && fields.iter().all(|field| object.contains_key(*field))
}

fn validate_vocabulary(registry: &Value, previous: Option<&Value>) -> Vec<String> {
    let value = registry.get("context_presentation");
    let old = previous.and_then(|previous| previous.get("context_presentation"));
    if value.is_none_or(Value::is_null) {
        return if old.is_some_and(|value| !value.is_null()) {
            vec!["context presentation cannot remove its historical owner contract".to_owned()]
        } else {
            Vec::new()
        };
    }
    let value = value.unwrap();
    let fields = [
        "schema_version",
        "presentation_id",
        "presentation_version",
        "owner_ref",
        "purpose",
        "default_language",
        "languages",
        "max_output_bytes",
        "max_entries",
        "record_schema_versions",
        "field_rules",
        "unclassified",
    ];
    let finite = exact_object_fields(value, &fields)
        && value.get("schema_version").and_then(Value::as_str)
            == Some("tos_context_presentation_v1")
        && value.get("presentation_id").and_then(Value::as_str)
            == Some("tos.context-presentation.governing")
        && value.get("owner_ref").and_then(Value::as_str) == Some("ToS/doctrine/HUMAN_FORMS.md")
        && value.get("purpose").and_then(Value::as_str)
            == Some("source-context-reading-not-assessment")
        && integer_exact(value.get("presentation_version"), 1, u64::MAX)
        && integer_exact(value.get("max_output_bytes"), 1024, 32_768)
        && integer_exact(value.get("max_entries"), 1, 256);
    if !finite {
        return vec!["context presentation violates its finite owner contract".to_owned()];
    }
    let languages = value.get("languages").and_then(Value::as_array);
    let Some(languages) = languages.filter(|values| !values.is_empty() && values.len() <= 8) else {
        return vec!["context presentation has invalid label languages".to_owned()];
    };
    let mut language_set = BTreeSet::new();
    let mut folded = BTreeSet::new();
    for language in languages {
        let Some(language) = language.as_str() else {
            return vec!["context presentation has invalid label languages".to_owned()];
        };
        if !language_key(language)
            || matches!(
                language.to_ascii_lowercase().as_str(),
                "default" | "original" | "auto"
            )
            || !folded.insert(language.to_ascii_lowercase())
        {
            return vec!["context presentation has invalid label languages".to_owned()];
        }
        language_set.insert(language.to_owned());
    }
    if !value
        .get("default_language")
        .and_then(Value::as_str)
        .is_some_and(|v| language_set.contains(v))
    {
        return vec!["context presentation has invalid label languages".to_owned()];
    }
    let schemas = value
        .get("record_schema_versions")
        .and_then(Value::as_array);
    let Some(schemas) = schemas.filter(|values| !values.is_empty() && values.len() <= 128) else {
        return vec!["context presentation has invalid source-schema selectors".to_owned()];
    };
    let mut schema_set = BTreeSet::new();
    for schema in schemas {
        let Some(schema) = schema.as_str() else {
            return vec!["context presentation has invalid source-schema selectors".to_owned()];
        };
        let valid = schema
            .strip_prefix("tos_")
            .and_then(|suffix| suffix.rsplit_once("_v"))
            .is_some_and(|(body, version)| {
                !body.is_empty()
                    && body
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
                    && !version.is_empty()
                    && version.bytes().all(|b| b.is_ascii_digit())
            });
        if !valid || !schema_set.insert(schema) {
            return vec!["context presentation has invalid source-schema selectors".to_owned()];
        }
    }
    let unknown = value.get("unclassified");
    if !exact_object_fields(unknown.unwrap_or(&Value::Null), &["label", "explanation"])
        || !labels(unknown.and_then(|v| v.get("label")), &language_set, 1024)
        || !labels(
            unknown.and_then(|v| v.get("explanation")),
            &language_set,
            1024,
        )
    {
        return vec!["context presentation has invalid unclassified explanation".to_owned()];
    }
    let Some(rules) = value
        .get("field_rules")
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty() && values.len() <= 128)
    else {
        return vec!["context presentation field rules are not bounded".to_owned()];
    };
    let targets = [
        "record",
        "assertion",
        "language-context",
        "subject-assessment",
        "assessment-snapshot",
    ];
    let technical = [
        "schema_version",
        "record_id",
        "record_version",
        "claim_id",
        "claim_version",
        "source_record_digest",
        "source_sha256",
        "record_sha256",
        "journal_revision",
        "owner_snapshot",
        "journal_batches",
    ];
    let mut routed = BTreeSet::new();
    let mut errors = Vec::new();
    for rule in rules {
        let fields = [
            "field",
            "targets",
            "category",
            "label",
            "explanation",
            "value_labels",
        ];
        let field = rule.get("field").and_then(Value::as_str);
        let field_valid = field.is_some_and(|field| {
            field.len() <= 96
                && field.bytes().next().is_some_and(|b| b.is_ascii_lowercase())
                && field
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        });
        let rule_targets = rule.get("targets").and_then(Value::as_array);
        let target_valid = rule_targets.is_some_and(|items| {
            let mut local = BTreeSet::new();
            !items.is_empty()
                && items.iter().all(|target| {
                    target
                        .as_str()
                        .is_some_and(|target| targets.contains(&target) && local.insert(target))
                })
        });
        let category = rule.get("category").and_then(Value::as_str);
        if !exact_object_fields(rule, &fields)
            || !field_valid
            || !target_valid
            || !matches!(category, Some("governing" | "technical"))
            || !labels(rule.get("label"), &language_set, 1024)
            || (!rule.get("explanation").is_some_and(Value::is_null)
                && !labels(rule.get("explanation"), &language_set, 1024))
        {
            errors.push("context presentation has an invalid field rule".to_owned());
            continue;
        }
        if category == Some("technical") && !field.is_some_and(|field| technical.contains(&field)) {
            errors.push("context presentation cannot hide a nonmechanical field".to_owned());
        }
        if let Some(value_labels) = rule.get("value_labels").and_then(Value::as_object) {
            if value_labels.is_empty()
                || value_labels.len() > 64
                || value_labels.iter().any(|(key, label)| {
                    key.is_empty()
                        || key.chars().count() > 128
                        || !labels(Some(label), &language_set, 1024)
                })
            {
                errors.push("context presentation has an invalid field rule".to_owned());
                continue;
            }
        } else if !rule.get("value_labels").is_some_and(Value::is_null) {
            errors.push("context presentation has an invalid field rule".to_owned());
            continue;
        }
        if let (Some(field), Some(rule_targets)) = (field, rule_targets) {
            for target in rule_targets.iter().filter_map(Value::as_str) {
                if !routed.insert((target.to_owned(), field.to_owned())) {
                    errors.push("context presentation has ambiguous field rules".to_owned());
                }
            }
        }
    }
    if let Some(old) = old.filter(|old| old.is_object()) {
        if old != value
            && integer_exact(value.get("presentation_version"), 1, u64::MAX)
            && exact_int(old.get("presentation_version")).is_some_and(|version| {
                value["presentation_version"].as_u64().unwrap_or(0) <= version as u64
            })
        {
            errors
                .push("changed context presentation must increase presentation_version".to_owned());
        }
        for key in ["schema_version", "presentation_id", "owner_ref", "purpose"] {
            if old.get(key) != value.get(key) {
                errors.push("context presentation cannot repurpose its owner identity".to_owned());
                break;
            }
        }
    }
    errors
}

fn integer_exact(value: Option<&Value>, minimum: u64, maximum: u64) -> bool {
    match value {
        Some(Value::Number(number)) if number.is_u64() => number
            .as_u64()
            .is_some_and(|value| (minimum..=maximum).contains(&value)),
        Some(Value::Number(number)) if number.is_i64() => number
            .as_i64()
            .is_some_and(|value| value >= 0 && (minimum..=maximum).contains(&(value as u64))),
        _ => false,
    }
}

fn valid_navigation_template_id(value: &str) -> bool {
    let Some(suffix) = value.strip_prefix("tos.navigation-template.") else {
        return false;
    };
    !suffix.is_empty()
        && suffix.split(['.', '-']).all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
        && suffix
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-')
}

fn validate_claim_navigation_template(registry: &Value) -> Vec<String> {
    let Some(template) = registry.get("claim_navigation_template") else {
        return Vec::new();
    };
    let invalid =
        || vec!["claim navigation template violates its finite source contract".to_owned()];
    let required = [
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
    let Some(object) = template.as_object() else {
        return invalid();
    };
    if required.iter().any(|field| !object.contains_key(*field))
        || object
            .keys()
            .any(|key| !required.contains(&key.as_str()) && key != "object_label_adapters")
        || !template
            .get("template_id")
            .and_then(Value::as_str)
            .is_some_and(valid_navigation_template_id)
        || !integer_exact(template.get("template_version"), 1, u64::MAX)
        || template.get("reader").and_then(Value::as_str) != Some("claim-navigation-v1")
        || template.get("purpose").and_then(Value::as_str) != Some("claim-navigation-only")
        || template.get("owner_ref").and_then(Value::as_str) != Some("ToS/doctrine/HUMAN_FORMS.md")
        || !integer_exact(template.get("max_output_bytes"), 128, 16_384)
    {
        return invalid();
    }
    let version = template
        .get("template_version")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if let Some(adapters) = template.get("object_label_adapters") {
        let Some(adapters) = adapters
            .as_array()
            .filter(|items| (1..=2).contains(&items.len()))
        else {
            return invalid();
        };
        let allowed = [
            "historical-time-source-wording-v1",
            "document-catalogue-time-source-wording-v1",
        ];
        let mut seen = BTreeSet::new();
        if adapters.iter().any(|adapter| {
            !adapter
                .as_str()
                .is_some_and(|value| allowed.contains(&value) && seen.insert(value))
        }) || (adapters
            .iter()
            .any(|value| value == "document-catalogue-time-source-wording-v1")
            && version < 3)
            || version < 2
        {
            return invalid();
        }
    }
    let Some(renderings) = template
        .get("renderings")
        .and_then(Value::as_object)
        .filter(|v| (1..=16).contains(&v.len()))
    else {
        return invalid();
    };
    let Some(statuses) = template.get("status_labels").and_then(Value::as_object) else {
        return invalid();
    };
    let status_keys: [(&str, &[&str]); 2] = [
        (
            "epistemic_status",
            &[
                "observed",
                "inferred",
                "reported",
                "interpreted",
                "uncertain",
                "disputed",
            ],
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
            ],
        ),
    ];
    if statuses.len() != 2
        || status_keys.iter().any(|(key, allowed)| {
            statuses
                .get(*key)
                .and_then(Value::as_object)
                .is_none_or(|values| {
                    values.len() != allowed.len()
                        || allowed.iter().any(|value| !values.contains_key(**value))
                })
        })
    {
        return invalid();
    }
    let languages: BTreeSet<String> = renderings.keys().cloned().collect();
    let mut folded = BTreeSet::new();
    if languages.iter().any(|language| {
        language.chars().count() > 64
            || !language_key(language)
            || matches!(
                language.to_ascii_lowercase().as_str(),
                "default" | "original" | "auto"
            )
            || !folded.insert(language.to_ascii_lowercase())
    }) || !template
        .get("default_language")
        .and_then(Value::as_str)
        .is_some_and(|value| languages.contains(value))
    {
        return invalid();
    }
    let mut label_values = vec![template.get("marker")];
    for (key, _) in &status_keys {
        if let Some(values) = statuses.get(*key).and_then(Value::as_object) {
            label_values.extend(values.values().map(Some));
        }
    }
    if label_values
        .into_iter()
        .any(|label| !labels(label, &languages, 256))
    {
        return invalid();
    }
    let slots = [
        "claim-marker",
        "subject-label",
        "predicate-label",
        "object-label",
        "declared-epistemic-status",
        "declared-review-status",
    ];
    for parts in renderings.values() {
        let Some(parts) = parts
            .as_array()
            .filter(|parts| (6..=32).contains(&parts.len()))
        else {
            return invalid();
        };
        if parts.first().is_none_or(|part| {
            !exact_object_fields(part, &["slot"])
                || part.get("slot").and_then(Value::as_str) != Some("claim-marker")
        }) {
            return invalid();
        }
        let mut occurrences = BTreeMap::<String, usize>::new();
        for part in parts {
            if exact_object_fields(part, &["slot"]) {
                let Some(slot) = part
                    .get("slot")
                    .and_then(Value::as_str)
                    .filter(|slot| slots.contains(slot))
                else {
                    return invalid();
                };
                *occurrences.entry(slot.to_owned()).or_default() += 1;
            } else if exact_object_fields(part, &["literal"]) {
                if !part
                    .get("literal")
                    .and_then(Value::as_str)
                    .is_some_and(|literal| (1..=256).contains(&literal.chars().count()))
                {
                    return invalid();
                }
            } else {
                return invalid();
            }
        }
        if slots.iter().any(|slot| occurrences.get(*slot) != Some(&1)) {
            return invalid();
        }
    }
    Vec::new()
}

fn acyclic_hierarchy(
    entries: &BTreeMap<String, &Value>,
    parent_field: &str,
    label: &str,
) -> Vec<String> {
    let graph = entries
        .iter()
        .map(|(identifier, entry)| {
            (
                identifier.clone(),
                strings(entry.get(parent_field))
                    .into_iter()
                    .map(str::to_owned)
                    .collect(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    acyclic_links(&graph, label)
}

fn acyclic_links(graph: &BTreeMap<String, Vec<String>>, label: &str) -> Vec<String> {
    let mut violations = Vec::new();
    let mut states = BTreeMap::<String, u8>::new();
    for identifier in graph.keys() {
        if states.get(identifier) == Some(&2) {
            continue;
        }
        states.insert(identifier.clone(), 1);
        let mut stack = vec![(identifier.clone(), graph[identifier].clone())];
        let mut offsets = vec![0usize];
        while let Some((current, parents)) = stack.last() {
            let offset = offsets.last_mut().unwrap();
            if *offset >= parents.len() {
                states.insert(current.clone(), 2);
                stack.pop();
                offsets.pop();
                continue;
            }
            let parent = parents[*offset].to_owned();
            *offset += 1;
            if !graph.contains_key(&parent) {
                violations.push(format!(
                    "{label} {current} references missing parent {parent}"
                ));
            } else if states.get(&parent) == Some(&1) {
                let mut cycle = stack.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>();
                cycle.push(parent);
                violations.push(format!(
                    "{label} hierarchy contains a cycle: {}",
                    cycle.join(" -> ")
                ));
            } else if states.get(&parent) != Some(&2) {
                states.insert(parent.clone(), 1);
                stack.push((parent.clone(), graph[&parent].clone()));
                offsets.push(0);
            }
        }
    }
    violations
}

/// Mechanical registry report equivalent to `knowledge.validate_semantic_registries`.
/// Registry violations remain report data; malformed ownership values that
/// make the Python source itself raise retain a bounded error result.
pub(crate) fn validate_semantic_registries(
    entity_registry: &Value,
    relation_registry: &Value,
    entity_original: &JsonValue,
    relation_original: &JsonValue,
) -> Result<Value> {
    let mut violations = validate_vocabulary(entity_registry, None);
    let entity_rows = registry_items(entity_registry, "types");
    let relation_rows = registry_items(relation_registry, "relations");
    let mut entity_entries = BTreeMap::<String, &Value>::new();
    let mut relation_entries = BTreeMap::<String, &Value>::new();
    let mut entity_order = Vec::<String>::new();
    let mut relation_order = Vec::<String>::new();
    for entry in entity_rows {
        let Some(identifier) = string(entry.get("type_id")) else {
            violations.push("entity registry contains an entry without type_id".to_owned());
            continue;
        };
        if entity_entries.contains_key(identifier) {
            violations.push(format!("duplicate entity type_id {identifier}"));
        } else {
            entity_order.push(identifier.to_owned());
        }
        entity_entries.insert(identifier.to_owned(), entry);
    }
    for entry in relation_rows {
        let Some(identifier) = string(entry.get("relation_type_id")) else {
            violations
                .push("relation registry contains an entry without relation_type_id".to_owned());
            continue;
        };
        if relation_entries.contains_key(identifier) {
            violations.push(format!("duplicate relation_type_id {identifier}"));
        } else {
            relation_order.push(identifier.to_owned());
        }
        relation_entries.insert(identifier.to_owned(), entry);
    }
    violations.extend(validate_claim_navigation_template(relation_registry));

    let definitions = match entity_registry.get("property_definitions") {
        None => Vec::new(),
        Some(Value::Array(definitions)) => {
            if definitions.iter().any(|definition| !definition.is_object()) {
                return Err(Error::Invalid(
                    "semantic property definition is not an object",
                ));
            }
            definitions.iter().collect::<Vec<_>>()
        }
        Some(Value::String(_) | Value::Object(_)) => {
            return Err(Error::Invalid(
                "semantic property definitions are not object rows",
            ));
        }
        Some(Value::Null | Value::Bool(_) | Value::Number(_)) => {
            return Err(Error::Invalid(
                "semantic property definitions are not iterable",
            ));
        }
    };
    let raw_definitions = match original_path(entity_original, "property_definitions") {
        None => Vec::new(),
        Some(JsonValue::Array(definitions)) => definitions.iter().collect::<Vec<_>>(),
        Some(JsonValue::String(_) | JsonValue::Object(_)) => {
            return Err(Error::Invalid(
                "semantic property definitions are not object rows",
            ));
        }
        Some(JsonValue::Null | JsonValue::Bool(_) | JsonValue::Number(_)) => {
            return Err(Error::Invalid(
                "semantic property definitions are not iterable",
            ));
        }
    };
    let mut properties_seen = Vec::<Value>::new();
    for (index, definition) in definitions.into_iter().enumerate() {
        let raw_definition = raw_definitions
            .get(index)
            .copied()
            .unwrap_or(&JsonValue::Null);
        let identifier = definition
            .get("property_id")
            .cloned()
            .unwrap_or(Value::Null);
        if matches!(identifier, Value::Array(_) | Value::Object(_)) {
            return Err(Error::Invalid("unhashable semantic property_id"));
        }
        let duplicate = properties_seen
            .iter()
            .map(|seen| python_eq(seen, &identifier))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .any(|equal| equal);
        if !truthy(Some(&identifier)) || duplicate {
            let raw_identifier = raw_definition
                .object_get("property_id")
                .unwrap_or(&JsonValue::Null);
            violations.push(format!(
                "duplicate or missing property_id {}",
                python_raw_str(raw_identifier)?
            ));
        }
        properties_seen.push(identifier.clone());
        let field = raw_definition
            .object_get("field")
            .map(python_raw_str)
            .transpose()?
            .unwrap_or_default();
        let identifier_text = raw_definition
            .object_get("property_id")
            .map(python_raw_str)
            .transpose()?
            .unwrap_or_else(|| "None".to_owned());
        if !allowed_field(&field, "node") {
            violations.push(format!(
                "property {identifier_text} has an unsupported query field"
            ));
        }
        let Some(raw_applies_to) = raw_definition.object_get("applies_to") else {
            continue;
        };
        match raw_applies_to {
            JsonValue::Array(owners) => {
                for owner in owners {
                    if matches!(owner, JsonValue::Array(_) | JsonValue::Object(_)) {
                        return Err(Error::Invalid("unhashable semantic property owner"));
                    }
                    if !owner
                        .as_str()
                        .is_some_and(|owner| entity_entries.contains_key(owner))
                    {
                        violations.push(format!(
                            "property {identifier_text} refers to unregistered type {}",
                            python_raw_str(owner)?
                        ));
                    }
                }
            }
            JsonValue::String(owners) => {
                for owner in owners.chars() {
                    let text = owner.to_string();
                    if !entity_entries.contains_key(&text) {
                        violations.push(format!(
                            "property {identifier_text} refers to unregistered type {text}"
                        ));
                    }
                }
            }
            JsonValue::Object(owners) => {
                for (owner, _) in owners {
                    let Some(text) = owner.as_str() else {
                        return Err(Error::Invalid("semantic surrogate property owner"));
                    };
                    if !entity_entries.contains_key(text) {
                        violations.push(format!(
                            "property {identifier_text} refers to unregistered type {text}"
                        ));
                    }
                }
            }
            JsonValue::Null | JsonValue::Bool(_) | JsonValue::Number(_) => {
                return Err(Error::Invalid(
                    "semantic property applies_to is not iterable",
                ));
            }
        }
    }

    let fallback_type = string(entity_registry.get("fallback_type_id"));
    let fallback_relation = string(relation_registry.get("fallback_relation_type_id"));
    if !fallback_type.is_some_and(|id| entity_entries.contains_key(id)) {
        let repr = match fallback_type {
            Some(identifier) => python_string_repr(identifier)?,
            None => "None".to_owned(),
        };
        violations.push(format!("entity fallback {repr} is not registered"));
    }
    if !fallback_relation.is_some_and(|id| relation_entries.contains_key(id)) {
        let repr = match fallback_relation {
            Some(identifier) => python_string_repr(identifier)?,
            None => "None".to_owned(),
        };
        violations.push(format!("relation fallback {repr} is not registered"));
    }
    violations.extend(acyclic_hierarchy(
        &entity_entries,
        "parent_type_ids",
        "entity",
    ));
    violations.extend(acyclic_hierarchy(
        &relation_entries,
        "parent_relation_type_ids",
        "relation",
    ));
    for (entries, field, label) in [
        (&entity_entries, "supersedes_type_id", "entity supersession"),
        (
            &relation_entries,
            "supersedes_relation_type_id",
            "relation supersession",
        ),
    ] {
        let links = entries
            .iter()
            .map(|(key, entry)| {
                let previous = if truthy(entry.get(field)) {
                    strings(entry.get(field))
                        .into_iter()
                        .map(str::to_owned)
                        .collect()
                } else {
                    Vec::new()
                };
                (key.clone(), previous)
            })
            .collect::<BTreeMap<String, Vec<String>>>();
        violations.extend(acyclic_links(&links, label));
        for (key, entry) in entries {
            if truthy(entry.get("abstract")) && truthy(entry.get("source_mappings")) {
                violations.push(format!(
                    "abstract {label} {key} cannot map source instances"
                ));
            }
        }
    }

    let mut entity_mappings = BTreeMap::<(String, String), String>::new();
    for identifier in &entity_order {
        let entry = entity_entries[identifier];
        for mapping in objects(entry.get("source_mappings")) {
            let source_graph = string(mapping.get("source_graph")).unwrap_or("");
            let source_kind_id = string(mapping.get("source_kind_id")).unwrap_or("");
            let key = (source_graph.to_owned(), source_kind_id.to_owned());
            if source_graph.is_empty() || source_kind_id.is_empty() {
                violations.push(format!("entity mapping on {identifier} is incomplete"));
                continue;
            }
            if let Some(prior) = entity_mappings.get(&key) {
                violations.push(format!(
                    "duplicate entity source mapping {} on {prior} and {identifier}",
                    python_tuple2(&key.0, &key.1)?
                ));
            }
            entity_mappings.insert(key, identifier.clone());
        }
    }

    let mut relation_mappings = BTreeMap::<(String, String, String), String>::new();
    for identifier in &relation_order {
        let entry = relation_entries[identifier];
        for endpoint_type in [
            strings(entry.get("domain_type_ids")),
            strings(entry.get("range_type_ids")),
        ]
        .concat()
        {
            if !entity_entries.contains_key(endpoint_type) {
                violations.push(format!(
                    "relation {identifier} references missing entity type {endpoint_type}"
                ));
            }
        }
        for parent in strings(entry.get("parent_relation_type_ids")) {
            if !relation_entries.contains_key(parent) {
                violations.push(format!(
                    "relation {identifier} references missing parent {parent}"
                ));
            }
        }
        if let Some(inverse) = string(entry.get("inverse_relation_type_id")) {
            if !relation_entries.contains_key(inverse) {
                violations.push(format!(
                    "relation {identifier} references missing inverse {inverse}"
                ));
            } else if string(relation_entries[inverse].get("inverse_relation_type_id"))
                != Some(identifier.as_str())
            {
                violations.push(format!(
                    "relation inverse {identifier} -> {inverse} is not reciprocal"
                ));
            }
        }
        let cardinality = entry
            .get("cardinality")
            .filter(|value| value.is_object())
            .unwrap_or(&Value::Null);
        for (minimum_key, maximum_key) in [
            ("per_subject_min", "per_subject_max"),
            ("per_object_min", "per_object_max"),
        ] {
            let minimum = cardinality.get(minimum_key);
            let maximum = cardinality.get(maximum_key);
            if let (Some(minimum), Some(maximum)) = (minimum, maximum) {
                if integer_like(minimum)
                    .zip(integer_like(maximum))
                    .is_some_and(|(minimum, maximum)| {
                        compare_integer_text(&minimum, &maximum) == std::cmp::Ordering::Greater
                    })
                {
                    violations.push(format!(
                        "relation {identifier} has {minimum_key} greater than {maximum_key}"
                    ));
                }
            }
        }
        for mapping in objects(entry.get("source_mappings")) {
            let source_graph = string(mapping.get("source_graph")).unwrap_or("");
            let source_predicate_id = string(mapping.get("source_predicate_id")).unwrap_or("");
            let scope = string(mapping.get("scope")).unwrap_or("");
            let key = (
                source_graph.to_owned(),
                source_predicate_id.to_owned(),
                scope.to_owned(),
            );
            if source_graph.is_empty() || source_predicate_id.is_empty() || scope.is_empty() {
                violations.push(format!("relation mapping on {identifier} is incomplete"));
                continue;
            }
            if let Some(prior) = relation_mappings.get(&key) {
                violations.push(format!(
                    "duplicate relation source mapping {} on {prior} and {identifier}",
                    python_tuple3(&key.0, &key.1, &key.2)?
                ));
            }
            relation_mappings.insert(key, identifier.clone());
        }
    }
    let violations = violations.into_iter().collect::<BTreeSet<_>>();
    Ok(json!({
        "valid": violations.is_empty(),
        "violations": violations,
        "entity_type_count": entity_entries.len(),
        "relation_type_count": relation_entries.len(),
        "entity_mapping_count": entity_mappings.len(),
        "relation_mapping_count": relation_mappings.len(),
    }))
}

fn integer_like(value: &Value) -> Option<String> {
    python_integer_text(value)
}

fn allowed_field(field: &str, kind: &str) -> bool {
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
    const RELATION_FIELDS: &[&str] = &[
        "id",
        "native_id",
        "source_graph",
        "from_id",
        "to_id",
        "predicate_id",
        "relation_type_id",
        "predicate_mapping.status",
        "predicate_mapping.source_predicate_id",
        "display.label.default",
        "display.label.ru",
        "display.label.en",
        "display.statement.default",
        "display.explanation.default",
        "display.explanation_state",
        "epistemic.authority_layer",
        "epistemic.canon_status",
        "epistemic.review_posture",
        "epistemic.confidence",
        "graph_layers",
        "view_ids",
        "source_refs",
    ];
    const UNSAFE: &[&str] = &["__proto__", "prototype", "constructor"];
    let fields = if kind == "node" {
        NODE_FIELDS
    } else {
        RELATION_FIELDS
    };
    if fields.contains(&field) {
        return true;
    }
    let parts: Vec<&str> = field.split('.').collect();
    let display_fields: &[&str] = if kind == "node" {
        &["title", "kind_label", "summary"]
    } else {
        &["label", "inverse_label", "statement", "explanation"]
    };
    if parts.len() == 3
        && parts[0] == "display"
        && display_fields.contains(&parts[1])
        && form_key(parts[2])
    {
        return true;
    }
    let Some((root, rest)) = field.split_once('.') else {
        return false;
    };
    if !["attributes", "semantics"].contains(&root)
        || rest.is_empty()
        || rest.len() > 128
        || !rest.as_bytes()[0].is_ascii_alphanumeric()
        || !rest.as_bytes()[0].is_ascii()
        || !rest
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'.' || b == b'-')
    {
        return false;
    }
    !field.split('.').any(|part| UNSAFE.contains(&part))
}

fn form_key(key: &str) -> bool {
    key == "default" || key == "original" || language_key(key)
}

pub(crate) fn semantic_cardinality_violation(
    endpoint: &str,
    relation_type: &str,
    maximum_key: &str,
    peak: i64,
    relation_entry: &Value,
) -> Result<Option<String>> {
    let maximum = relation_entry
        .get("cardinality")
        .and_then(|value| value.get(maximum_key));
    if let Some(maximum) = maximum {
        let Some(order) = compare_python_number(peak, maximum) else {
            return Err(Error::Invalid("semantic cardinality maximum"));
        };
        if order == std::cmp::Ordering::Greater {
            return Ok(Some(format!(
                "{endpoint} violates {relation_type} {maximum_key}={}",
                python_str(maximum)?
            )));
        }
        Ok(None)
    } else {
        Ok(None)
    }
}

fn compare_python_number(left: i64, right: &Value) -> Option<std::cmp::Ordering> {
    let left = left.to_string();
    if let Some(right) = python_integer_text(right) {
        return Some(compare_integer_text(&left, &right));
    }
    let right = match right {
        Value::Number(number) => number.as_f64()?,
        _ => return None,
    };
    if !right.is_finite() {
        return None;
    }
    let floor = format!("{:.0}", right.floor());
    let comparison = compare_integer_text(&left, &floor);
    if comparison == std::cmp::Ordering::Equal && right.fract() != 0.0 {
        Some(std::cmp::Ordering::Less)
    } else {
        Some(comparison)
    }
}

fn exact_form_ref(value: Option<&Value>) -> bool {
    let Some(value) = value else { return false };
    exact_object_fields(value, &["id", "version", "digest"])
        && value
            .get("id")
            .and_then(Value::as_str)
            .is_some_and(|id| !id.is_empty())
        && integer_exact(value.get("version"), 1, MAX_SAFE_INTEGER)
        && value
            .get("digest")
            .and_then(Value::as_str)
            .is_some_and(|digest| {
                digest.strip_prefix("sha256:").is_some_and(|hex| {
                    hex.len() == 64
                        && hex
                            .bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                })
            })
}

fn exact_record_digest(value: Option<&Value>) -> Result<String> {
    let value = value.ok_or(Error::Invalid("semantic exact record"))?;
    let raw = serde_json::to_vec(value).map_err(|error| Error::Source(error.to_string()))?;
    let cap = raw.len().max(1);
    let json = crate::d1_public_capture::json(&raw, cap)?;
    let canonical = crate::d1_public_capture::compact(&json, cap)?;
    Ok(tos_foundation::Digest256::of_bytes(&canonical).to_hex())
}

fn native_metadata_identity(record: &Value) -> Result<Option<&'static str>> {
    if record.get("schema_version").and_then(Value::as_str) == Some("tos_canonical_node_v1") {
        let kind = record.get("node_type").and_then(Value::as_str);
        let kinds = [
            "source",
            "concept",
            "principle",
            "lineage",
            "event",
            "state",
            "support",
            "context",
            "analogy",
            "synthesis",
        ];
        let id = record.get("node_id").and_then(Value::as_str);
        let version = integer_exact(record.get("record_version"), 1, MAX_SAFE_INTEGER);
        let valid_id = id.is_some_and(|id| {
            let Some(rest) = id.strip_prefix("tos.") else {
                return false;
            };
            let Some((prefix, tail)) = rest.split_once('.') else {
                return false;
            };
            Some(prefix) == kind
                && !tail.is_empty()
                && tail.split(['.', '-']).all(|part| {
                    !part.is_empty()
                        && part
                            .bytes()
                            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
                })
        });
        if record.get("record_id").is_some()
            || !kind.is_some_and(|kind| kinds.contains(&kind))
            || !valid_id
            || version.is_none()
        {
            return Err(Error::Invalid("native canonical identity"));
        }
        return Ok(Some("node_id"));
    }
    let schema = record.get("schema_version").and_then(Value::as_str);
    let field = match schema {
        Some("tos_scholarly_composite_witness_v1") => Some("composite_id"),
        Some("tos_artifact_source_witness_v1" | "tos_artifact_source_witness_v2") => {
            Some("artifact_id")
        }
        _ => None,
    };
    if let Some(field) = field {
        let id = record.get(field).and_then(Value::as_str);
        let prefix = if field == "composite_id" {
            "tos.composite."
        } else {
            "tos.artifact."
        };
        if record.get("record_id").is_some() || !id.is_some_and(|id| id.starts_with(prefix)) {
            return Err(Error::Invalid("native metadata identity"));
        }
    }
    Ok(field)
}

fn metadata_history_refs(node: &Value) -> Result<Option<Vec<Value>>> {
    let attributes = node
        .get("attributes")
        .filter(|v| truthy(Some(v)))
        .unwrap_or(&Value::Null);
    let history = attributes.get("record_history");
    let record = attributes.get("source_record");
    let identity_field = if let Some(record) = record.filter(|record| record.is_object()) {
        match native_metadata_identity(record) {
            Ok(Some(field)) => field,
            Ok(None) => "record_id",
            Err(_) => return Ok(None),
        }
    } else {
        "record_id"
    };
    let fields = [
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
    let Some(history) = history else {
        return Ok(None);
    };
    let valid = exact_object_fields(history, &fields)
        && history.get("schema_version").and_then(Value::as_str)
            == Some("tos_metadata_record_history_v1")
        && history.get("status").and_then(Value::as_str) == Some("available")
        && history
            .get("reason")
            .and_then(Value::as_str)
            .is_some_and(|reason| (1..=256).contains(&reason.chars().count()))
        && [
            "grants_current_use",
            "performs_assessment",
            "writes_to_source",
        ]
        .iter()
        .all(|key| history.get(*key) == Some(&Value::Bool(false)))
        && history
            .get("provenance")
            .and_then(Value::as_object)
            .is_some_and(|value| !value.is_empty())
        && record.is_some_and(Value::is_object)
        && python_optional_eq(
            record.and_then(|record| record.get(identity_field)),
            node.get("entity_id"),
        )
        && integer_exact(
            record.and_then(|record| record.get("record_version")),
            0,
            MAX_SAFE_INTEGER,
        )
        .is_some()
        && python_optional_eq(
            history.get("record_id"),
            record.and_then(|record| record.get(identity_field)),
        )
        && exact_form_ref(history.get("current_ref"));
    if !valid {
        return Ok(None);
    }
    let refs = history.get("refs").and_then(Value::as_array);
    let Some(refs) = refs.filter(|refs| (1..=129).contains(&refs.len())) else {
        return Ok(None);
    };
    let record = record.unwrap();
    let current = json!({
        "id": record.get(identity_field).unwrap(),
        "version": record.get("record_version").unwrap(),
        "digest": format!("sha256:{}", exact_record_digest(Some(record))?),
    });
    if exact_record_digest(history.get("current_ref"))? != exact_record_digest(Some(&current))? {
        return Ok(None);
    }
    let current_id = current.get("id");
    let mut previous_version = None;
    for reference in refs {
        if !exact_form_ref(Some(reference)) || !python_optional_eq(reference.get("id"), current_id)
        {
            return Ok(None);
        }
        let version = reference
            .get("version")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        if previous_version.is_some_and(|previous| version != previous + 1) {
            return Ok(None);
        }
        previous_version = Some(version);
    }
    if exact_record_digest(refs.last())? != exact_record_digest(Some(&current))? {
        return Ok(None);
    }
    Ok(Some(refs.clone()))
}
