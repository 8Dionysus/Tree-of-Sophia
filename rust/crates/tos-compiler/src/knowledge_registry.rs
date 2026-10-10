//! Bounded, exact semantic crosswalk selected from authored registries.
//! Registry lookup does not assess a source row or admit a graph publication.

use crate::{Error, Result, d1_public_capture::CreationState};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use tos_foundation::{Digest256, Digest256Hasher, JsonLimits, JsonMode, parse_json};

const MAX_REGISTRY_BYTES: usize = 4 * 1024 * 1024;
const MAX_TYPES: usize = 4096;
const MAX_MAPPINGS: usize = 16384;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedType<'a> {
    pub type_id: &'a str,
    pub mapped: bool,
}

#[derive(Clone, Debug)]
pub struct KnowledgeRegistry {
    pub entity_sha256: String,
    pub relation_sha256: String,
    pub entity_semantic_digest: String,
    pub relation_semantic_digest: String,
    pub entity_registry_id: String,
    pub relation_registry_id: String,
    pub entity_registry_version: u64,
    pub relation_registry_version: u64,
    fallback_type_id: String,
    fallback_relation_type_id: String,
    entity_types: BTreeMap<String, Vec<String>>,
    relation_types: BTreeSet<String>,
    entity_mappings: BTreeMap<String, BTreeMap<String, String>>,
    relation_mappings: BTreeMap<String, BTreeMap<String, BTreeMap<String, String>>>,
}

fn strict(raw: &[u8]) -> Result<Value> {
    if raw.is_empty() || raw.len() > MAX_REGISTRY_BYTES {
        return Err(Error::Budget("semantic registry bytes"));
    }
    let limits = JsonLimits::new(MAX_REGISTRY_BYTES, 96, 1_000_000, 4096)
        .map_err(|_| Error::Budget("semantic registry JSON limits"))?;
    parse_json(raw, JsonMode::PublishedStrict, limits).map_err(|e| Error::Source(e.to_string()))?;
    serde_json::from_slice(raw).map_err(|_| Error::Invalid("semantic registry JSON"))
}
fn text<'a>(value: &'a Value, name: &str) -> Result<&'a str> {
    value
        .get(name)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.len() <= 4096)
        .ok_or(Error::Invalid("semantic registry text field"))
}
fn version(value: &Value) -> Result<u64> {
    value
        .get("registry_version")
        .and_then(Value::as_u64)
        .filter(|v| *v > 0)
        .ok_or(Error::Invalid("semantic registry version"))
}
fn array<'a>(value: &'a Value, name: &str, cap: usize) -> Result<&'a [Value]> {
    value
        .get(name)
        .and_then(Value::as_array)
        .filter(|a| !a.is_empty() && a.len() <= cap)
        .map(Vec::as_slice)
        .ok_or(Error::Invalid("semantic registry collection"))
}
fn mappings(value: &Value) -> Result<&[Value]> {
    match value.get("source_mappings") {
        None => Ok(&[]),
        Some(Value::Array(rows)) if rows.len() <= MAX_MAPPINGS => Ok(rows),
        _ => Err(Error::Invalid("semantic source mappings")),
    }
}
fn acyclic(parents: &BTreeMap<String, Vec<String>>) -> Result<()> {
    let mut remaining: BTreeMap<&str, usize> = BTreeMap::new();
    let mut children: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (id, upstream) in parents {
        let mut unique = BTreeSet::new();
        for parent in upstream {
            if !parents.contains_key(parent) || !unique.insert(parent.as_str()) {
                return Err(Error::Invalid("missing or duplicate registry parent"));
            }
            children.entry(parent).or_default().push(id);
        }
        remaining.insert(id, upstream.len());
    }
    let mut ready: Vec<&str> = remaining
        .iter()
        .filter_map(|(id, count)| (*count == 0).then_some(*id))
        .collect();
    let mut processed = 0usize;
    while let Some(id) = ready.pop() {
        processed += 1;
        for child in children.get(id).into_iter().flatten() {
            let count = remaining
                .get_mut(child)
                .ok_or(Error::Invalid("registry hierarchy"))?;
            *count -= 1;
            if *count == 0 {
                ready.push(child);
            }
        }
    }
    if processed != parents.len() {
        return Err(Error::Invalid("registry type cycle"));
    }
    Ok(())
}

fn owned_tree_node<K, V>() -> Result<usize> {
    // Match the native Stage's pinned B-tree admission: one complete node per
    // inserted entry includes key/value slots and child links.
    11usize
        .checked_mul(std::mem::size_of::<(K, V)>())
        .and_then(|n| n.checked_add(16 * std::mem::size_of::<usize>()))
        .ok_or(Error::Budget("semantic registry B-tree geometry"))
}

fn owned_text(state: &CreationState<'_>, value: &str) -> Result<String> {
    state.retain(value.len())?;
    state.charge_work(value.len())?;
    Ok(value.to_owned())
}

fn write_usize(hasher: &mut Digest256Hasher, mut value: usize) -> Result<()> {
    let mut digits = [0u8; std::mem::size_of::<usize>() * 3];
    let mut at = digits.len();
    loop {
        at -= 1;
        digits[at] = b'0' + (value % 10) as u8;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    hasher.update(&digits[at..]);
    Ok(())
}

fn stable_digest_owned(value: &Value, state: &CreationState<'_>) -> Result<String> {
    fn decimal_digits(mut value: usize) -> usize {
        let mut digits = 1usize;
        while value >= 10 {
            value /= 10;
            digits += 1;
        }
        digits
    }

    fn write_string(
        hasher: &mut Digest256Hasher,
        value: &str,
        state: &CreationState<'_>,
    ) -> Result<()> {
        let work = value
            .len()
            .checked_add(decimal_digits(value.len()))
            .and_then(|bytes| bytes.checked_add(2))
            .ok_or(Error::Budget("semantic digest work"))?;
        state.charge_work(work)?;
        hasher.update(b"s");
        write_usize(hasher, value.len())?;
        hasher.update(b":");
        hasher.update(value.as_bytes());
        Ok(())
    }

    fn visit(
        value: &Value,
        hasher: &mut Digest256Hasher,
        state: &CreationState<'_>,
        depth: usize,
    ) -> Result<()> {
        if depth > 96 {
            return Err(Error::Budget("semantic digest depth"));
        }
        state.charge_work(std::mem::size_of::<Value>())?;
        match value {
            Value::Null => hasher.update(b"n;"),
            Value::Bool(boolean) => hasher.update(if *boolean { b"b1;" } else { b"b0;" }),
            Value::Number(number) => {
                let mut float = number
                    .as_f64()
                    .filter(|number| number.is_finite())
                    .ok_or(Error::Invalid("non-finite stable digest number"))?;
                if float == 0.0 {
                    float = 0.0;
                }
                state.charge_work(19)?;
                hasher.update(b"d");
                const HEX: &[u8; 16] = b"0123456789abcdef";
                for byte in float.to_be_bytes() {
                    hasher.update(&[HEX[(byte >> 4) as usize], HEX[(byte & 0x0f) as usize]]);
                }
                hasher.update(b";");
            }
            Value::String(string) => write_string(hasher, string, state)?,
            Value::Array(items) => {
                state.charge_work(items.len())?;
                hasher.update(b"a");
                write_usize(hasher, items.len())?;
                hasher.update(b"[");
                for item in items {
                    visit(item, hasher, state, depth + 1)?;
                }
                hasher.update(b"]");
            }
            Value::Object(items) => {
                let key_bytes = items
                    .len()
                    .checked_mul(std::mem::size_of::<&String>())
                    .ok_or(Error::Budget("semantic digest object keys"))?;
                let _key_hold = state.hold(key_bytes)?;
                state.charge_work(key_bytes)?;
                hasher.update(b"o");
                write_usize(hasher, items.len())?;
                hasher.update(b"{");
                let mut keys = Vec::with_capacity(items.len());
                keys.extend(items.keys());
                keys.sort_unstable();
                for key in keys {
                    write_string(hasher, key, state)?;
                    visit(
                        items
                            .get(key)
                            .ok_or(Error::Invalid("stable digest object key"))?,
                        hasher,
                        state,
                        depth + 1,
                    )?;
                }
                hasher.update(b"}");
            }
        }
        Ok(())
    }

    let mut hasher = Digest256Hasher::new();
    visit(value, &mut hasher, state, 0)?;
    state.retain(64)?;
    Ok(hasher.finalize().to_hex())
}

fn acyclic_borrowed(parents: &BTreeMap<&str, Vec<&str>>, state: &CreationState<'_>) -> Result<()> {
    let mut edge_count = 0usize;
    let mut largest_parent_list = 0usize;
    for upstream in parents.values() {
        state.active()?;
        edge_count = edge_count
            .checked_add(upstream.len())
            .ok_or(Error::Budget("semantic registry hierarchy edges"))?;
        largest_parent_list = largest_parent_list.max(upstream.len());
    }
    let remaining_node = owned_tree_node::<&str, usize>()?;
    let child_count_node = remaining_node;
    let children_node = owned_tree_node::<&str, Vec<&str>>()?;
    let seen_node = owned_tree_node::<&str, ()>()?;
    let temporary = parents
        .len()
        .checked_mul(
            remaining_node
                .checked_add(child_count_node)
                .and_then(|n| n.checked_add(children_node))
                .ok_or(Error::Budget("semantic registry hierarchy state"))?,
        )
        .and_then(|n| n.checked_add(edge_count.checked_mul(std::mem::size_of::<&str>())?))
        .and_then(|n| n.checked_add(parents.len().checked_mul(std::mem::size_of::<&str>())?))
        .and_then(|n| n.checked_add(largest_parent_list.checked_mul(seen_node)?))
        .ok_or(Error::Budget("semantic registry hierarchy state"))?;
    let _temporary_hold = state.hold(temporary)?;
    state.charge_work(temporary)?;

    let mut child_counts: BTreeMap<&str, usize> = BTreeMap::new();
    for (id, upstream) in parents {
        state.active()?;
        let mut unique = BTreeSet::new();
        for parent in upstream {
            state.active()?;
            state.charge_work(
                parent
                    .len()
                    .checked_add(id.len())
                    .ok_or(Error::Budget("semantic registry hierarchy work"))?,
            )?;
            if !parents.contains_key(parent) || !unique.insert(*parent) {
                return Err(Error::Invalid("missing or duplicate registry parent"));
            }
            let count = child_counts.entry(parent).or_default();
            *count = count
                .checked_add(1)
                .ok_or(Error::Budget("semantic registry hierarchy count"))?;
        }
    }
    let mut remaining: BTreeMap<&str, usize> = BTreeMap::new();
    for (id, upstream) in parents {
        state.active()?;
        remaining.insert(id, upstream.len());
    }
    let mut children: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (parent, count) in &child_counts {
        state.active()?;
        children.insert(*parent, Vec::with_capacity(*count));
    }
    for (id, upstream) in parents {
        state.active()?;
        for parent in upstream {
            state.active()?;
            children
                .get_mut(parent)
                .ok_or(Error::Invalid("semantic registry hierarchy parent"))?
                .push(id);
        }
    }
    drop(child_counts);
    let mut ready = Vec::with_capacity(remaining.len());
    for (id, count) in &remaining {
        state.active()?;
        if *count == 0 {
            ready.push(*id);
        }
    }
    let mut processed = 0usize;
    while let Some(id) = ready.pop() {
        state.charge_work(id.len())?;
        processed += 1;
        for child in children.get(id).into_iter().flatten() {
            state.active()?;
            let count = remaining
                .get_mut(child)
                .ok_or(Error::Invalid("registry hierarchy"))?;
            *count -= 1;
            if *count == 0 {
                ready.push(child);
            }
        }
    }
    if processed != parents.len() {
        return Err(Error::Invalid("registry type cycle"));
    }
    Ok(())
}

fn acyclic_owned_parents(
    parents: &BTreeMap<String, Vec<String>>,
    state: &CreationState<'_>,
) -> Result<()> {
    let borrowed_node = owned_tree_node::<&str, Vec<&str>>()?;
    let mut bytes = parents
        .len()
        .checked_mul(borrowed_node)
        .ok_or(Error::Budget("semantic registry hierarchy state"))?;
    for upstream in parents.values() {
        state.active()?;
        bytes = bytes
            .checked_add(
                upstream
                    .len()
                    .checked_mul(std::mem::size_of::<&str>())
                    .ok_or(Error::Budget("semantic registry hierarchy state"))?,
            )
            .ok_or(Error::Budget("semantic registry hierarchy state"))?;
    }
    state.charge_work(bytes)?;
    let _borrowed_hold = state.hold(bytes)?;
    let mut borrowed = BTreeMap::new();
    for (id, upstream) in parents {
        state.active()?;
        let mut parent_refs = Vec::with_capacity(upstream.len());
        parent_refs.extend(upstream.iter().map(String::as_str));
        borrowed.insert(id.as_str(), parent_refs);
    }
    acyclic_borrowed(&borrowed, state)
}

impl KnowledgeRegistry {
    pub fn fallback_entity_type_id(&self) -> &str {
        &self.fallback_type_id
    }

    pub fn parse(entity_bytes: &[u8], relation_bytes: &[u8]) -> Result<Self> {
        let entity = strict(entity_bytes)?;
        let relation = strict(relation_bytes)?;
        if text(&entity, "schema_version")? != "tos_semantic_entity_type_registry_v1"
            || text(&relation, "schema_version")? != "tos_semantic_relation_type_registry_v1"
        {
            return Err(Error::Invalid("semantic registry schema"));
        }
        let entity_registry_id = text(&entity, "registry_id")?.to_owned();
        let relation_registry_id = text(&relation, "registry_id")?.to_owned();
        let fallback_type_id = text(&entity, "fallback_type_id")?.to_owned();
        let fallback_relation_type_id = text(&relation, "fallback_relation_type_id")?.to_owned();
        let entity_registry_version = version(&entity)?;
        let relation_registry_version = version(&relation)?;
        let mut entity_types = BTreeMap::new();
        let mut entity_mappings = BTreeMap::new();
        let mut mapping_count = 0usize;
        for row in array(&entity, "types", MAX_TYPES)? {
            let id = text(row, "type_id")?.to_owned();
            let parents = row
                .get("parent_type_ids")
                .and_then(Value::as_array)
                .ok_or(Error::Invalid("entity type parents"))?
                .iter()
                .map(|v| {
                    v.as_str()
                        .filter(|s| !s.is_empty())
                        .map(str::to_owned)
                        .ok_or(Error::Invalid("entity parent id"))
                })
                .collect::<Result<Vec<_>>>()?;
            if entity_types.insert(id.clone(), parents).is_some() {
                return Err(Error::Invalid("duplicate entity type"));
            }
            for mapping in mappings(row)? {
                mapping_count += 1;
                if mapping_count > MAX_MAPPINGS {
                    return Err(Error::Budget("entity mapping count"));
                }
                let source = text(mapping, "source_graph")?;
                let kind = text(mapping, "source_kind_id")?;
                if entity_mappings
                    .entry(source.to_owned())
                    .or_insert_with(BTreeMap::new)
                    .insert(kind.to_owned(), id.clone())
                    .is_some()
                {
                    return Err(Error::Invalid("duplicate entity source mapping"));
                }
            }
        }
        if !entity_types.contains_key(&fallback_type_id) {
            return Err(Error::Invalid("missing entity fallback type"));
        }
        // Kahn traversal visits each declared edge once and does not allocate
        // a recursion stack proportional to hierarchy depth.
        acyclic(&entity_types)?;
        let mut relation_parents = BTreeMap::new();
        let mut relation_mappings = BTreeMap::new();
        mapping_count = 0;
        for row in array(&relation, "relations", MAX_TYPES)? {
            let id = text(row, "relation_type_id")?.to_owned();
            let parents = row
                .get("parent_relation_type_ids")
                .and_then(Value::as_array)
                .ok_or(Error::Invalid("relation type parents"))?
                .iter()
                .map(|v| {
                    v.as_str()
                        .filter(|s| !s.is_empty())
                        .map(str::to_owned)
                        .ok_or(Error::Invalid("relation parent id"))
                })
                .collect::<Result<Vec<_>>>()?;
            if relation_parents.insert(id.clone(), parents).is_some() {
                return Err(Error::Invalid("duplicate relation type"));
            }
            for mapping in mappings(row)? {
                mapping_count += 1;
                if mapping_count > MAX_MAPPINGS {
                    return Err(Error::Budget("relation mapping count"));
                }
                let source = text(mapping, "source_graph")?;
                let predicate = text(mapping, "source_predicate_id")?;
                let scope = text(mapping, "scope")?;
                if relation_mappings
                    .entry(source.to_owned())
                    .or_insert_with(BTreeMap::new)
                    .entry(predicate.to_owned())
                    .or_insert_with(BTreeMap::new)
                    .insert(scope.to_owned(), id.clone())
                    .is_some()
                {
                    return Err(Error::Invalid("duplicate relation source mapping"));
                }
            }
        }
        if !relation_parents.contains_key(&fallback_relation_type_id) {
            return Err(Error::Invalid("missing relation fallback type"));
        }
        acyclic(&relation_parents)?;
        let relation_types = relation_parents.into_keys().collect();
        Ok(Self {
            entity_sha256: Digest256::of_bytes(entity_bytes).to_hex(),
            relation_sha256: Digest256::of_bytes(relation_bytes).to_hex(),
            entity_semantic_digest: crate::knowledge_normalization::stable_digest(&entity)?,
            relation_semantic_digest: crate::knowledge_normalization::stable_digest(&relation)?,
            entity_registry_id,
            relation_registry_id,
            entity_registry_version,
            relation_registry_version,
            fallback_type_id,
            fallback_relation_type_id,
            entity_types,
            relation_types,
            entity_mappings,
            relation_mappings,
        })
    }

    /// Parse the same two authored registries under the serial Whole creation
    /// state. The callback may validate source-row semantics while both
    /// checked input trees are scoped; only the returned registry is retained.
    pub(crate) fn with_parsed_owned<T>(
        entity_bytes: &[u8],
        relation_bytes: &[u8],
        state: &CreationState<'_>,
        operation: impl FnOnce(&Value, &Value, &Self) -> Result<T>,
    ) -> Result<(Self, T)> {
        if entity_bytes.is_empty()
            || relation_bytes.is_empty()
            || entity_bytes.len() > MAX_REGISTRY_BYTES
            || relation_bytes.len() > MAX_REGISTRY_BYTES
        {
            return Err(Error::Budget("semantic registry bytes"));
        }
        let limits = JsonLimits::new(MAX_REGISTRY_BYTES, 96, 1_000_000, 4096)
            .map_err(|_| Error::Budget("semantic registry JSON limits"))?;
        state.charge_work(entity_bytes.len())?;
        state.retain(64)?;
        let entity_sha256 = Digest256::of_bytes(entity_bytes).to_hex();
        state.charge_work(relation_bytes.len())?;
        state.retain(64)?;
        let relation_sha256 = Digest256::of_bytes(relation_bytes).to_hex();
        state.with_serde_owned_value_with_limits(entity_bytes, limits, |entity| {
            state.with_serde_owned_value_with_limits(relation_bytes, limits, |relation| {
                let registry = Self::parse_values_owned(
                    &entity,
                    &relation,
                    entity_sha256,
                    relation_sha256,
                    state,
                )?;
                let checked = operation(&entity, &relation, &registry)?;
                Ok((registry, checked))
            })
        })
    }

    pub(crate) fn parse_with_creation_state(
        entity_bytes: &[u8],
        relation_bytes: &[u8],
        state: &CreationState<'_>,
    ) -> Result<Self> {
        Self::with_parsed_owned(entity_bytes, relation_bytes, state, |_, _, _| Ok(()))
            .map(|(registry, ())| registry)
    }

    fn parse_values_owned(
        entity: &Value,
        relation: &Value,
        entity_sha256: String,
        relation_sha256: String,
        state: &CreationState<'_>,
    ) -> Result<Self> {
        if text(entity, "schema_version")? != "tos_semantic_entity_type_registry_v1"
            || text(relation, "schema_version")? != "tos_semantic_relation_type_registry_v1"
        {
            return Err(Error::Invalid("semantic registry schema"));
        }
        state.retain(std::mem::size_of::<Self>())?;
        let entity_registry_id = owned_text(state, text(entity, "registry_id")?)?;
        let relation_registry_id = owned_text(state, text(relation, "registry_id")?)?;
        let fallback_type_id = owned_text(state, text(entity, "fallback_type_id")?)?;
        let fallback_relation_type_id =
            owned_text(state, text(relation, "fallback_relation_type_id")?)?;
        let entity_registry_version = version(entity)?;
        let relation_registry_version = version(relation)?;
        let entity_node = owned_tree_node::<String, Vec<String>>()?;
        let entity_mapping_outer_node = owned_tree_node::<String, BTreeMap<String, String>>()?;
        let entity_mapping_inner_node = owned_tree_node::<String, String>()?;
        let mut entity_types = BTreeMap::new();
        let mut entity_mappings = BTreeMap::new();
        let mut mapping_count = 0usize;
        for row in array(entity, "types", MAX_TYPES)? {
            state.active()?;
            let source_id = text(row, "type_id")?;
            if entity_types.contains_key(source_id) {
                return Err(Error::Invalid("duplicate entity type"));
            }
            let parent_rows = row
                .get("parent_type_ids")
                .and_then(Value::as_array)
                .ok_or(Error::Invalid("entity type parents"))?;
            let parent_slots = parent_rows
                .len()
                .checked_mul(std::mem::size_of::<String>())
                .ok_or(Error::Budget("semantic registry parent vector"))?;
            state.retain(
                entity_node
                    .checked_add(parent_slots)
                    .ok_or(Error::Budget("semantic registry entity type state"))?,
            )?;
            let mut parents = Vec::with_capacity(parent_rows.len());
            for parent in parent_rows {
                state.active()?;
                let parent = parent
                    .as_str()
                    .filter(|value| !value.is_empty())
                    .ok_or(Error::Invalid("entity parent id"))?;
                parents.push(owned_text(state, parent)?);
            }
            let id = owned_text(state, source_id)?;
            entity_types.insert(id, parents);
            for mapping in mappings(row)? {
                state.active()?;
                mapping_count = mapping_count
                    .checked_add(1)
                    .filter(|count| *count <= MAX_MAPPINGS)
                    .ok_or(Error::Budget("entity mapping count"))?;
                let source = text(mapping, "source_graph")?;
                let kind = text(mapping, "source_kind_id")?;
                if !entity_mappings.contains_key(source) {
                    state.retain(
                        entity_mapping_outer_node
                            .checked_add(source.len())
                            .ok_or(Error::Budget("semantic registry entity mapping state"))?,
                    )?;
                    let source = owned_text(state, source)?;
                    entity_mappings.insert(source, BTreeMap::new());
                }
                let kinds = entity_mappings
                    .get_mut(source)
                    .ok_or(Error::Invalid("semantic registry entity source map"))?;
                if kinds.contains_key(kind) {
                    return Err(Error::Invalid("duplicate entity source mapping"));
                }
                let bytes = entity_mapping_inner_node
                    .checked_add(kind.len())
                    .and_then(|n| n.checked_add(source_id.len()))
                    .ok_or(Error::Budget("semantic registry entity mapping state"))?;
                state.retain(bytes)?;
                let kind = owned_text(state, kind)?;
                let target = owned_text(state, source_id)?;
                kinds.insert(kind, target);
            }
        }
        if !entity_types.contains_key(&fallback_type_id) {
            return Err(Error::Invalid("missing entity fallback type"));
        }
        acyclic_owned_parents(&entity_types, state)?;

        let relation_rows = array(relation, "relations", MAX_TYPES)?;
        let borrowed_node = owned_tree_node::<&str, Vec<&str>>()?;
        let mut relation_parent_bytes = relation_rows
            .len()
            .checked_mul(borrowed_node)
            .ok_or(Error::Budget("semantic registry relation hierarchy"))?;
        for row in relation_rows {
            state.active()?;
            let _ = text(row, "relation_type_id")?;
            let parents = row
                .get("parent_relation_type_ids")
                .and_then(Value::as_array)
                .ok_or(Error::Invalid("relation type parents"))?;
            relation_parent_bytes = relation_parent_bytes
                .checked_add(
                    parents
                        .len()
                        .checked_mul(std::mem::size_of::<&str>())
                        .ok_or(Error::Budget("semantic registry relation parents"))?,
                )
                .ok_or(Error::Budget("semantic registry relation hierarchy"))?;
        }
        state.charge_work(relation_parent_bytes)?;
        let _relation_parent_hold = state.hold(relation_parent_bytes)?;
        let mut relation_parents: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        let mut relation_mappings = BTreeMap::new();
        mapping_count = 0;
        let relation_mapping_outer_node =
            owned_tree_node::<String, BTreeMap<String, BTreeMap<String, String>>>()?;
        let relation_mapping_predicate_node =
            owned_tree_node::<String, BTreeMap<String, String>>()?;
        let relation_mapping_scope_node = owned_tree_node::<String, String>()?;
        for row in relation_rows {
            state.active()?;
            let id = text(row, "relation_type_id")?;
            if relation_parents.contains_key(id) {
                return Err(Error::Invalid("duplicate relation type"));
            }
            let parent_rows = row
                .get("parent_relation_type_ids")
                .and_then(Value::as_array)
                .ok_or(Error::Invalid("relation type parents"))?;
            let mut parents = Vec::with_capacity(parent_rows.len());
            for parent in parent_rows {
                state.active()?;
                parents.push(
                    parent
                        .as_str()
                        .filter(|value| !value.is_empty())
                        .ok_or(Error::Invalid("relation parent id"))?,
                );
            }
            relation_parents.insert(id, parents);
            for mapping in mappings(row)? {
                state.active()?;
                mapping_count = mapping_count
                    .checked_add(1)
                    .filter(|count| *count <= MAX_MAPPINGS)
                    .ok_or(Error::Budget("relation mapping count"))?;
                let source = text(mapping, "source_graph")?;
                let predicate = text(mapping, "source_predicate_id")?;
                let scope = text(mapping, "scope")?;
                if !relation_mappings.contains_key(source) {
                    state.retain(
                        relation_mapping_outer_node
                            .checked_add(source.len())
                            .ok_or(Error::Budget("semantic registry relation mapping state"))?,
                    )?;
                    let source = owned_text(state, source)?;
                    relation_mappings.insert(source, BTreeMap::new());
                }
                let predicates = relation_mappings
                    .get_mut(source)
                    .ok_or(Error::Invalid("semantic registry relation source map"))?;
                if !predicates.contains_key(predicate) {
                    state.retain(
                        relation_mapping_predicate_node
                            .checked_add(predicate.len())
                            .ok_or(Error::Budget("semantic registry relation mapping state"))?,
                    )?;
                    let predicate = owned_text(state, predicate)?;
                    predicates.insert(predicate, BTreeMap::new());
                }
                let scopes = predicates
                    .get_mut(predicate)
                    .ok_or(Error::Invalid("semantic registry relation predicate map"))?;
                if scopes.contains_key(scope) {
                    return Err(Error::Invalid("duplicate relation source mapping"));
                }
                let bytes = relation_mapping_scope_node
                    .checked_add(scope.len())
                    .and_then(|n| n.checked_add(id.len()))
                    .ok_or(Error::Budget("semantic registry relation mapping state"))?;
                state.retain(bytes)?;
                let scope = owned_text(state, scope)?;
                let target = owned_text(state, id)?;
                scopes.insert(scope, target);
            }
        }
        if !relation_parents.contains_key(fallback_relation_type_id.as_str()) {
            return Err(Error::Invalid("missing relation fallback type"));
        }
        acyclic_borrowed(&relation_parents, state)?;
        let relation_type_node = owned_tree_node::<String, ()>()?;
        state.retain(
            relation_parents
                .len()
                .checked_mul(relation_type_node)
                .ok_or(Error::Budget("semantic registry relation type state"))?,
        )?;
        let mut relation_types = BTreeSet::new();
        for id in relation_parents.keys() {
            state.active()?;
            relation_types.insert(owned_text(state, id)?);
        }
        drop(relation_parents);
        drop(_relation_parent_hold);
        let entity_semantic_digest = stable_digest_owned(entity, state)?;
        let relation_semantic_digest = stable_digest_owned(relation, state)?;
        Ok(Self {
            entity_sha256,
            relation_sha256,
            entity_semantic_digest,
            relation_semantic_digest,
            entity_registry_id,
            relation_registry_id,
            entity_registry_version,
            relation_registry_version,
            fallback_type_id,
            fallback_relation_type_id,
            entity_types,
            relation_types,
            entity_mappings,
            relation_mappings,
        })
    }

    pub fn entity(&self, source_graph: &str, native_kind: &str) -> ResolvedType<'_> {
        match self
            .entity_mappings
            .get(source_graph)
            .and_then(|kinds| kinds.get(native_kind))
        {
            Some(id) => ResolvedType {
                type_id: id,
                mapped: true,
            },
            None => ResolvedType {
                type_id: &self.fallback_type_id,
                mapped: false,
            },
        }
    }
    pub fn relation(
        &self,
        source_graph: &str,
        native_predicate: &str,
        scope: &str,
    ) -> ResolvedType<'_> {
        match self
            .relation_mappings
            .get(source_graph)
            .and_then(|predicates| predicates.get(native_predicate))
            .and_then(|scopes| scopes.get(scope))
        {
            Some(id) => ResolvedType {
                type_id: id,
                mapped: true,
            },
            None => ResolvedType {
                type_id: &self.fallback_relation_type_id,
                mapped: false,
            },
        }
    }
    pub fn contains_entity_type(&self, id: &str) -> bool {
        self.entity_types.contains_key(id)
    }
    pub fn contains_relation_type(&self, id: &str) -> bool {
        self.relation_types.contains(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actual_owner_registry_crosswalk_and_unknown_fallback() {
        let entity =
            include_bytes!("../../../../ToS/doctrine/semantic-interchange/entity-types.v1.json");
        let relation =
            include_bytes!("../../../../ToS/doctrine/semantic-interchange/relation-types.v1.json");
        let map = KnowledgeRegistry::parse(entity, relation).unwrap();
        assert!(map.contains_entity_type("tos.entity.unmapped"));
        assert!(map.contains_relation_type("tos.relation.unmapped"));
        assert_eq!(
            map.entity("new-eighth-source", "unregistered-native-kind"),
            ResolvedType {
                type_id: "tos.entity.unmapped",
                mapped: false
            }
        );
        assert_eq!(
            map.relation("new-eighth-source", "unregistered-native-predicate", "edge"),
            ResolvedType {
                type_id: "tos.relation.unmapped",
                mapped: false
            }
        );
        assert!(map.entity_registry_version > 0 && map.relation_registry_version > 0);
    }
}
