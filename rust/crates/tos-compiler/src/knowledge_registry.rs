//! Bounded, exact semantic crosswalk selected from authored registries.
//! Registry lookup does not assess a source row or admit a graph publication.

use crate::{Error, Result};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use tos_foundation::{Digest256, JsonLimits, JsonMode, parse_json};

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
    pub entity_registry_id: String,
    pub relation_registry_id: String,
    pub entity_registry_version: u64,
    pub relation_registry_version: u64,
    fallback_type_id: String,
    fallback_relation_type_id: String,
    entity_types: BTreeMap<String, Vec<String>>,
    relation_types: BTreeSet<String>,
    entity_mappings: BTreeMap<(String, String), String>,
    relation_mappings: BTreeMap<(String, String, String), String>,
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

impl KnowledgeRegistry {
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
                let key = (
                    text(mapping, "source_graph")?.to_owned(),
                    text(mapping, "source_kind_id")?.to_owned(),
                );
                if entity_mappings.insert(key, id.clone()).is_some() {
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
                let key = (
                    text(mapping, "source_graph")?.to_owned(),
                    text(mapping, "source_predicate_id")?.to_owned(),
                    text(mapping, "scope")?.to_owned(),
                );
                if relation_mappings.insert(key, id.clone()).is_some() {
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
            .get(&(source_graph.to_owned(), native_kind.to_owned()))
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
        match self.relation_mappings.get(&(
            source_graph.to_owned(),
            native_predicate.to_owned(),
            scope.to_owned(),
        )) {
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
