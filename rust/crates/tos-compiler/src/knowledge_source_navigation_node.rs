//! Native, private source-navigation base-node normalization.
//!
//! This produces Python-compatible base nodes for supported owner envelopes.
//! Global claim/context updates and inherited views may change a base row,
//! so this module never writes a final graph row. Unsupported node variants
//! and available historical record bodies fail closed.

use crate::knowledge_normalization::{SourceRow, stamp_content_revision};
use crate::knowledge_philosophy_display::ordinary_philosophy_node_display;
use crate::knowledge_source_navigation_prepare::NavigationPrepareReceipt;
use crate::knowledge_stage::SeekRow;
use crate::{Error, KnowledgeRegistry, QueryVocabulary, Result};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use tos_foundation::Digest256;

const ADAPTER_PROFILE: &str = "source-navigation-node-edge-v1";
const SHARED_ID_GRAMMAR: &str = "^tos\\.[a-z0-9]+(?:[.-][a-z0-9]+)*$";
const RECORD_SCHEMA: &str = "tos_record_version_view_v1";
const MAX_REGISTRY_BYTES: usize = 4 * 1024 * 1024;
const MAX_ADDRESS: u64 = 9_007_199_254_740_991;

#[derive(Clone, Copy, Debug)]
pub struct NavigationNodeLimits {
    pub max_raw_bytes: usize,
    pub max_output_bytes: usize,
    pub max_ancestor_cache_bytes: usize,
}
impl NavigationNodeLimits {
    fn validate(self) -> Result<()> {
        if self.max_raw_bytes == 0
            || self.max_raw_bytes > 8 * 1024 * 1024
            || self.max_output_bytes == 0
            || self.max_output_bytes > 8 * 1024 * 1024
            || self.max_ancestor_cache_bytes == 0
            || self.max_ancestor_cache_bytes > 4 * 1024 * 1024
        {
            return Err(Error::Budget("navigation node normalization limits"));
        }
        Ok(())
    }
}

struct TypeEntry {
    parents: Vec<String>,
    labels: Option<Value>,
    mapping_labels: BTreeMap<String, BTreeMap<String, Value>>,
}

/// The registry bytes must be the independently selected owner bytes used to
/// construct `KnowledgeRegistry`; a matching digest alone is not admission.
pub struct NavigationNodeNormalizer<'a> {
    registry: &'a KnowledgeRegistry,
    source_graph_id: String,
    dossier_kinds: BTreeSet<String>,
    entity_registry_ref: String,
    types: BTreeMap<String, TypeEntry>,
    ancestors: BTreeMap<String, Vec<String>>,
    ancestor_cache_bytes: usize,
    limits: NavigationNodeLimits,
}

fn required<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.len() <= 4096 && !s.contains('\0'))
        .ok_or(Error::Invalid("navigation native node field"))
}
fn text(value: Option<&Value>) -> Option<&str> {
    value?.as_str().map(str::trim).filter(|s| !s.is_empty())
}
fn string_list(value: Option<&Value>) -> Vec<Value> {
    let mut seen = BTreeSet::new();
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().filter(|s| !s.is_empty()))
        .filter(|s| seen.insert((*s).to_owned()))
        .map(|s| Value::String(s.to_owned()))
        .collect()
}
fn source_dossier_id<'a>(kinds: &BTreeSet<String>, kind: &str, native: &'a str) -> Option<&'a str> {
    if kinds.contains(kind) && valid_tos_id(native) {
        Some(native)
    } else {
        None
    }
}
fn valid_tos_id(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("tos.") else {
        return false;
    };
    let mut previous_separator = true;
    for byte in rest.bytes() {
        if byte.is_ascii_lowercase() || byte.is_ascii_digit() {
            previous_separator = false;
        } else if matches!(byte, b'.' | b'-') && !previous_separator {
            previous_separator = true;
        } else {
            return false;
        }
    }
    !previous_separator
}
fn attributes(item: &Value) -> Result<Map<String, Value>> {
    let object = item
        .as_object()
        .ok_or(Error::Invalid("navigation native node object"))?;
    let mut attrs = item
        .get("properties")
        .and_then(Value::as_object)
        .ok_or(Error::Invalid("navigation native properties"))?
        .clone();
    for (key, value) in object {
        if !matches!(
            key.as_str(),
            "id" | "node_id"
                | "label"
                | "canonical_label"
                | "node_type"
                | "node_kind"
                | "resource_kind"
                | "display"
                | "multilingual"
                | "properties"
                | "source_ref"
                | "source_refs"
                | "graph_layers"
                | "view_ids"
        ) && !attrs.contains_key(key)
        {
            attrs.insert(key.clone(), value.clone());
        }
    }
    Ok(attrs)
}
fn epistemic(item: &Value) -> Value {
    let props = item.get("properties").and_then(Value::as_object);
    let p = |key| props.and_then(|p| p.get(key));
    let authority = text(p("authority_posture"))
        .or_else(|| text(item.get("authority_layer")))
        .unwrap_or("derived-export");
    let canon = text(p("canon_status")).or_else(|| text(item.get("status")));
    let review = text(p("review_posture"))
        .or_else(|| text(p("review_status")))
        .or_else(|| text(item.get("review_status")))
        .unwrap_or("not-recorded");
    let confidence = [
        p("confidence"),
        p("master_confidence"),
        item.get("confidence"),
    ]
    .into_iter()
    .flatten()
    .find_map(|v| match v {
        Value::Number(n) if n.as_f64().is_some_and(f64::is_finite) => Some(v.clone()),
        Value::String(s) if !s.trim().is_empty() => Some(Value::String(s.trim().to_owned())),
        _ => None,
    })
    .unwrap_or(Value::Null);
    json!({"authority_layer":authority,"canon_status":canon,"review_posture":review,"confidence":confidence})
}
fn ordinary_envelope(item: &Value) -> Result<()> {
    let object = item
        .as_object()
        .ok_or(Error::Invalid("navigation native owner object"))?;
    if object.keys().any(|key| {
        !matches!(
            key.as_str(),
            "node_id"
                | "node_kind"
                | "label"
                | "source_ref"
                | "identity_status"
                | "properties"
                | "source_refs"
                | "graph_layers"
                | "view_ids"
                | "authority_layer"
                | "status"
                | "review_status"
                | "confidence"
                | "layer"
        )
    }) {
        return Err(Error::Invalid(
            "unsupported navigation ordinary node envelope",
        ));
    }
    let props = item
        .get("properties")
        .and_then(Value::as_object)
        .ok_or(Error::Invalid("navigation native properties"))?;
    if props.keys().any(|key| {
        !matches!(
            key.as_str(),
            "record_id"
                | "preferred_label"
                | "summary"
                | "description"
                | "role"
                | "purpose"
                | "comment"
        )
    }) {
        return Err(Error::Invalid(
            "unsupported navigation ordinary node properties",
        ));
    }
    if matches!(
        required(item, "node_kind")?,
        "claim" | "literal" | "place" | "region" | "temporal-assertion"
    ) {
        return Err(Error::Invalid("unsupported navigation semantic kind"));
    }
    Ok(())
}
fn record_ref_digest(value: &Value) -> Result<String> {
    let raw =
        serde_json::to_vec(value).map_err(|_| Error::Invalid("record version reference JSON"))?;
    Ok(Digest256::of_bytes(&raw).to_hex())
}
fn unavailable_record_view<'a>(item: &'a Value, native: &str) -> Result<&'a Value> {
    let object = item
        .as_object()
        .ok_or(Error::Invalid("record version owner object"))?;
    if object.keys().any(|key| {
        !matches!(
            key.as_str(),
            "node_id" | "node_kind" | "source_ref" | "properties" | "label" | "identity_status"
        )
    }) || required(item, "node_kind")? != "record-version"
        || item
            .get("label")
            .is_some_and(|v| v.as_str() != Some("Exact record version"))
        || item
            .get("identity_status")
            .is_some_and(|v| v.as_str() != Some("not_applicable"))
    {
        return Err(Error::Invalid("record version closed envelope"));
    }
    let props = item
        .get("properties")
        .and_then(Value::as_object)
        .ok_or(Error::Invalid("record version properties"))?;
    if props.len() != 1 {
        return Err(Error::Invalid("record version property coverage"));
    }
    let view = props
        .get("record_version_view")
        .ok_or(Error::Invalid("record version view"))?;
    let view_obj = view
        .as_object()
        .ok_or(Error::Invalid("record version view object"))?;
    if view_obj.len() != 10
        || view_obj.keys().any(|k| {
            !matches!(
                k.as_str(),
                "schema_version"
                    | "record_ref"
                    | "record_kind"
                    | "status"
                    | "reason"
                    | "version_status"
                    | "record"
                    | "provenance"
                    | "grants_current_use"
                    | "performs_assessment"
            )
        })
        || required(view, "schema_version")? != RECORD_SCHEMA
        || !matches!(required(view, "record_kind")?, "claim" | "metadata")
        || !matches!(
            required(view, "status")?,
            "missing" | "stale" | "corrupt" | "access-restricted" | "over-budget"
        )
        || !view.get("record").is_some_and(Value::is_null)
        || !view.get("version_status").is_some_and(Value::is_null)
        || !view
            .get("provenance")
            .and_then(Value::as_object)
            .is_some_and(Map::is_empty)
        || view.get("grants_current_use") != Some(&Value::Bool(false))
        || view.get("performs_assessment") != Some(&Value::Bool(false))
        || required(view, "reason")?.chars().count() > 256
    {
        return Err(Error::Invalid("record version unavailable status"));
    }
    let reference = view
        .get("record_ref")
        .ok_or(Error::Invalid("record version reference"))?;
    let ref_obj = reference
        .as_object()
        .ok_or(Error::Invalid("record version reference object"))?;
    if ref_obj.len() != 3
        || ref_obj
            .keys()
            .any(|k| !matches!(k.as_str(), "id" | "version" | "digest"))
        || !valid_tos_id(required(reference, "id")?)
        || required(reference, "id")?.starts_with("tos.claim.")
            != (required(view, "record_kind")? == "claim")
        || reference
            .get("version")
            .and_then(Value::as_u64)
            .is_none_or(|v| v == 0 || v > MAX_ADDRESS)
        || Digest256::from_prefixed(required(reference, "digest")?).is_err()
        || format!("record-version:{}", record_ref_digest(reference)?) != native
    {
        return Err(Error::Invalid("record version exact reference"));
    }
    Ok(view)
}
fn apply_unavailable_record_view(output: &mut Value, view: &Value) -> Result<()> {
    let version = view
        .get("record_ref")
        .and_then(|r| r.get("version"))
        .and_then(Value::as_u64)
        .ok_or(Error::Invalid("record version number"))?;
    let mut record = Map::new();
    for key in [
        "schema_version",
        "record_ref",
        "record_kind",
        "status",
        "reason",
        "version_status",
        "grants_current_use",
        "performs_assessment",
    ] {
        record.insert(
            key.into(),
            view.get(key)
                .ok_or(Error::Invalid("record version member"))?
                .clone(),
        );
    }
    record.insert(
        "record_pointer".into(),
        Value::String("/attributes/record_version_view/record".into()),
    );
    let record = Value::Object(record);
    let ancestors = output
        .get("semantics")
        .and_then(|s| s.get("type_ancestors"))
        .ok_or(Error::Invalid("record version ancestors"))?
        .clone();
    output["semantics"] = json!({"type_ancestors":ancestors,"record_version":record});
    let display = output
        .get_mut("display")
        .and_then(Value::as_object_mut)
        .ok_or(Error::Invalid("record version display"))?;
    display.insert("title".into(),json!({"default":format!("Exact record version {version}"),
        "ru":format!("Точная версия записи {version}"),"en":format!("Exact record version {version}"),"original":null}));
    display.insert(
        "summary".into(),
        json!({"default":"The exact source wording is not available in this packet.",
        "ru":"Точная исходная формулировка недоступна в этом пакете.",
        "en":"The exact source wording is not available in this packet.","original":null}),
    );
    display.insert("summary_state".into(), Value::String("missing".into()));
    let provenance = display
        .get_mut("provenance")
        .and_then(Value::as_object_mut)
        .ok_or(Error::Invalid("record version provenance"))?;
    provenance.insert(
        "title".into(),
        Value::String("record-version-navigation".into()),
    );
    provenance.insert("source_title_available".into(), Value::Bool(false));
    provenance.insert("record_version".into(), record);
    provenance.insert("summary".into(), Value::String("missing".into()));
    provenance.insert("source_summary_available".into(), Value::Bool(false));
    Ok(())
}

impl<'a> NavigationNodeNormalizer<'a> {
    pub fn new(
        registry: &'a KnowledgeRegistry,
        entity_bytes: &[u8],
        vocabulary: &QueryVocabulary,
        descriptor_bytes: &[u8],
        limits: NavigationNodeLimits,
    ) -> Result<Self> {
        limits.validate()?;
        vocabulary.verify_authored_bytes(descriptor_bytes)?;
        if vocabulary.shared_entity_id_grammars.len() != 1
            || vocabulary.shared_entity_id_grammars[0] != SHARED_ID_GRAMMAR
        {
            return Err(Error::Invalid("navigation shared identity grammar profile"));
        }
        let descriptor = SourceRow::parse(descriptor_bytes, 1024 * 1024)?;
        let identity = descriptor
            .value()
            .get("identity")
            .ok_or(Error::Invalid("navigation descriptor identity"))?;
        let source_graph_id = required(identity, "source_dossier_graph_id")?.to_owned();
        let dossier_kinds = identity
            .get("source_dossier_kinds")
            .and_then(Value::as_array)
            .ok_or(Error::Invalid("navigation descriptor dossier kinds"))?
            .iter()
            .map(|kind| {
                kind.as_str()
                    .map(str::to_owned)
                    .ok_or(Error::Invalid("navigation dossier kind"))
            })
            .collect::<Result<BTreeSet<_>>>()?;
        if dossier_kinds.is_empty()
            || !vocabulary.sources.iter().any(|source| {
                source.source_graph_id == source_graph_id
                    && source.adapter_profile == ADAPTER_PROFILE
            })
        {
            return Err(Error::Invalid("navigation selected dossier owner"));
        }
        let entity_registry_ref = required(
            descriptor
                .value()
                .get("semantic_registry_refs")
                .and_then(|refs| refs.get("entity"))
                .ok_or(Error::Invalid("navigation entity registry descriptor"))?,
            "source_ref",
        )?
        .to_owned();
        if entity_bytes.is_empty()
            || entity_bytes.len() > MAX_REGISTRY_BYTES
            || Digest256::of_bytes(entity_bytes).to_hex() != registry.entity_sha256
        {
            return Err(Error::Invalid("navigation selected entity registry bytes"));
        }
        let source = SourceRow::parse(entity_bytes, MAX_REGISTRY_BYTES)?;
        let entries = source
            .value()
            .get("types")
            .and_then(Value::as_array)
            .ok_or(Error::Invalid("navigation entity registry types"))?;
        let mut types = BTreeMap::new();
        for entry in entries {
            let id = required(entry, "type_id")?.to_owned();
            let parents = entry
                .get("parent_type_ids")
                .and_then(Value::as_array)
                .ok_or(Error::Invalid("navigation type parents"))?
                .iter()
                .map(|v| {
                    v.as_str()
                        .map(str::to_owned)
                        .ok_or(Error::Invalid("navigation parent type"))
                })
                .collect::<Result<Vec<_>>>()?;
            let mut mapping_labels: BTreeMap<String, BTreeMap<String, Value>> = BTreeMap::new();
            for mapping in entry
                .get("source_mappings")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                if let Some(labels) = mapping
                    .get("labels")
                    .filter(|v| v.as_object().is_some_and(|object| !object.is_empty()))
                {
                    mapping_labels
                        .entry(required(mapping, "source_graph")?.to_owned())
                        .or_default()
                        .insert(
                            required(mapping, "source_kind_id")?.to_owned(),
                            labels.clone(),
                        );
                }
            }
            if types
                .insert(
                    id,
                    TypeEntry {
                        parents,
                        labels: entry.get("labels").cloned(),
                        mapping_labels,
                    },
                )
                .is_some()
            {
                return Err(Error::Invalid("duplicate navigation entity type"));
            }
        }
        Ok(Self {
            registry,
            source_graph_id,
            dossier_kinds,
            entity_registry_ref,
            types,
            ancestors: BTreeMap::new(),
            ancestor_cache_bytes: 0,
            limits,
        })
    }
    fn ancestors(&mut self, type_id: &str) -> Result<Vec<String>> {
        if let Some(cached) = self.ancestors.get(type_id) {
            return Ok(cached.clone());
        }
        let mut seen = BTreeSet::new();
        let mut stack = vec![type_id.to_owned()];
        while let Some(id) = stack.pop() {
            if !seen.insert(id.clone()) {
                continue;
            }
            let entry = self
                .types
                .get(&id)
                .ok_or(Error::Invalid("unknown navigation entity type"))?;
            stack.extend(entry.parents.iter().cloned());
            if seen.len() > 4096 {
                return Err(Error::Budget("navigation type ancestors"));
            }
        }
        let values = seen.into_iter().collect::<Vec<_>>();
        let bytes = values
            .iter()
            .try_fold(0usize, |sum, value| sum.checked_add(value.len()))
            .ok_or(Error::Budget("navigation ancestor cache"))?;
        self.ancestor_cache_bytes = self
            .ancestor_cache_bytes
            .checked_add(bytes)
            .filter(|sum| *sum <= self.limits.max_ancestor_cache_bytes)
            .ok_or(Error::Budget("navigation ancestor cache"))?;
        self.ancestors.insert(type_id.to_owned(), values.clone());
        Ok(values)
    }
    /// Exact base `_normalize_node` for the supported source-navigation
    /// envelopes. No global inherited-view or readable-context claim follows.
    pub fn normalize_base(
        &mut self,
        raw: &SeekRow,
        prepared: &NavigationPrepareReceipt,
    ) -> Result<NavigationBaseNode> {
        if prepared.source_graph != self.source_graph_id
            || prepared.source_cut.is_empty()
            || prepared.final_graph_rows_written
            || Digest256::from_hex(&prepared.dependency_root_sha256).is_err()
            || raw.source_graph != prepared.source_graph
            || raw.source_order.is_some()
        {
            return Err(Error::Invalid("navigation selected raw node"));
        }
        if raw.payload.len() > self.limits.max_raw_bytes {
            return Err(Error::Budget("navigation raw node bytes"));
        }
        if Digest256::of_bytes(&raw.payload).to_hex() != raw.payload_sha256 {
            return Err(Error::Invalid("navigation raw node digest"));
        }
        let source = SourceRow::parse(&raw.payload, self.limits.max_raw_bytes)?;
        let item = source.value();
        let native = required(item, "node_id")?;
        if native != raw.id || native.trim() != native {
            return Err(Error::Invalid("navigation native node ID"));
        }
        let kind = required(item, "node_kind")?;
        if required(item, "source_ref")?.trim().is_empty() {
            return Err(Error::Invalid("navigation node source ref"));
        }
        let record = kind == "record-version";
        let view = if record {
            Some(unavailable_record_view(item, native)?)
        } else {
            ordinary_envelope(item)?;
            None
        };
        let resolved = self.registry.entity(&prepared.source_graph, kind);
        let type_id = resolved.type_id.to_owned();
        let mapped = type_id != self.registry.fallback_entity_type_id();
        if record && type_id != "tos.entity.record-version" {
            return Err(Error::Invalid("record version exact type mapping"));
        }
        let ancestors = self.ancestors(&type_id)?;
        let type_entry = self
            .types
            .get(&type_id)
            .ok_or(Error::Invalid("navigation mapped type"))?;
        let labels = type_entry
            .mapping_labels
            .get(&prepared.source_graph)
            .and_then(|m| m.get(kind))
            .or(type_entry.labels.as_ref());
        let display = ordinary_philosophy_node_display(&source, kind, labels)?;
        let attrs = attributes(item)?;
        let source_record = source.source_record(&attrs)?;
        let normalized_id = format!("{}:{native}", prepared.source_graph);
        let entity_id = [
            item.get("properties").and_then(|p| p.get("record_id")),
            item.get("record_id"),
            item.get("node_id"),
        ]
        .into_iter()
        .find_map(|v| text(v).filter(|s| valid_tos_id(s)))
        .unwrap_or(&normalized_id)
        .to_owned();
        let graph_layers = if item.get("graph_layers").and_then(Value::as_array).is_some() {
            string_list(item.get("graph_layers"))
        } else {
            Vec::new()
        };
        let graph_layers = if graph_layers.is_empty() {
            text(item.get("layer"))
                .map(|s| vec![Value::String(s.to_owned())])
                .unwrap_or_default()
        } else {
            graph_layers
        };
        let mut output = json!({
            "id":normalized_id,"entity_id":entity_id,"native_id":native,
            "source_graph":prepared.source_graph,"kind_id":kind,"type_id":type_id,
            "type_mapping":{"status":if mapped {"mapped"} else {"unmapped"},
                "source_kind_id":kind,"registry_ref":self.entity_registry_ref},
            "display":display,"epistemic":epistemic(item),
            "graph_layers":graph_layers,"view_ids":string_list(item.get("view_ids")),
            "source_refs":source.source_refs(&[]),"attributes":attrs,
            "semantics":{"type_ancestors":ancestors},"source_record":source_record,
        });
        if let Some(dossier) = source_dossier_id(&self.dossier_kinds, kind, native) {
            output["source_dossier_ref"] = Value::String(dossier.to_owned());
        }
        if let Some(view) = view {
            apply_unavailable_record_view(&mut output, view)?;
        }
        stamp_content_revision(&mut output, self.limits.max_output_bytes)?;
        let content_revision = required(&output, "content_revision")?.to_owned();
        Ok(NavigationBaseNode {
            value: output,
            source_graph: prepared.source_graph.clone(),
            source_cut: prepared.source_cut.clone(),
            prepared_dependency_root_sha256: prepared.dependency_root_sha256.clone(),
            raw_node_sha256: raw.payload_sha256.clone(),
            entity_registry_sha256: self.registry.entity_sha256.clone(),
            relation_registry_sha256: self.registry.relation_sha256.clone(),
            content_revision,
        })
    }
}

/// Private base carrier. Its content revision is valid only before the
/// all-source `_apply_final_node_changes` pass; no conversion to a final-row
/// candidate or `OrderedKnowledgeSink` is offered here.
pub struct NavigationBaseNode {
    value: Value,
    pub source_graph: String,
    pub source_cut: String,
    pub prepared_dependency_root_sha256: String,
    pub raw_node_sha256: String,
    pub entity_registry_sha256: String,
    pub relation_registry_sha256: String,
    pub content_revision: String,
}
impl NavigationBaseNode {
    pub fn value(&self) -> &Value {
        &self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge_normalization::stable_digest;

    // One independent frozen CPython `knowledge._normalize_node` oracle pair:
    // ordinary Work and an unavailable exact record-version view. Both use
    // the selected authored entity registry; neither has a global join.
    #[test]
    fn python_oracle_native_nodes_preserve_owner_and_registry_semantics() {
        let entity =
            include_bytes!("../../../../ToS/doctrine/semantic-interchange/entity-types.v1.json");
        let relation =
            include_bytes!("../../../../ToS/doctrine/semantic-interchange/relation-types.v1.json");
        let registry = KnowledgeRegistry::parse(entity, relation).unwrap();
        let descriptor = include_bytes!(
            "../../../../ToS/doctrine/semantic-interchange/query-vocabulary.v1.json"
        );
        let document: Value = serde_json::from_slice(descriptor).unwrap();
        let mut adapters = document["sources"]
            .as_array()
            .unwrap()
            .iter()
            .map(|source| source["adapter_profile"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>();
        adapters.push(
            document["extension_adapter_profile"]
                .as_str()
                .unwrap()
                .to_owned(),
        );
        let vocabulary = QueryVocabulary::parse(
            descriptor,
            &adapters.iter().map(String::as_str).collect::<Vec<_>>(),
        )
        .unwrap();
        let limits = NavigationNodeLimits {
            max_raw_bytes: 4096,
            max_output_bytes: 32768,
            max_ancestor_cache_bytes: 32768,
        };
        let mut normalizer =
            NavigationNodeNormalizer::new(&registry, entity, &vocabulary, descriptor, limits)
                .unwrap();
        let prepared = NavigationPrepareReceipt {
            source_graph: "source-navigation".into(),
            input_role: "source-navigation".into(),
            source_cut: "fixture-cut".into(),
            nodes: 2,
            edges: 0,
            endpoint_refs: 0,
            unresolved_endpoint_refs: 0,
            header_claim_sha256: "0".repeat(64),
            header_claim_rights_count: 0,
            node_input_root_sha256: "0".repeat(64),
            edge_input_root_sha256: "0".repeat(64),
            dependency_root_sha256: "1".repeat(64),
            external_dependencies: &[],
            final_graph_rows_written: false,
        };
        let ordinary = json!({"node_id":"tos.work.alpha","node_kind":"work","label":"Alpha",
            "source_ref":"ToS/a.json","identity_status":"not_applicable","properties":{}});
        let reference = json!({"id":"tos.claim.sample","version":1,"digest":format!("sha256:{}","0".repeat(64))});
        let native = format!("record-version:{}", record_ref_digest(&reference).unwrap());
        let view = json!({"schema_version":RECORD_SCHEMA,"record_ref":reference,"record_kind":"claim",
            "status":"missing","reason":"not retained","version_status":null,"record":null,
            "provenance":{},"grants_current_use":false,"performs_assessment":false});
        let version = json!({"node_id":native,"node_kind":"record-version","label":"Exact record version",
            "source_ref":"ToS/claim.json","identity_status":"not_applicable",
            "properties":{"record_version_view":view}});
        for (source, expected_revision, expected_digest, expected_record) in [
            (
                ordinary,
                "ec2eedf9ed7c52f98640396fb018c7f9ec724d231405bafb7120c0755eaa2c68",
                "4d63af04a7f6b7822a3203dcfa0f8d76ffef5e0846124d074e86562a2b0ea0f4",
                "c789751ce807770f867b3d0054c39dc88d88244577c8eeb119d6e176440eb278",
            ),
            (
                version,
                "4099bbcd8dc5d0e59ed2d99070cbf686ac4ade6fb4434fdd25db7894f04fef3d",
                "e86b9cdc82b9117171e2a8dac0ecb1ef045dbf14ad0b051e03e70ecef1f7be98",
                "f7b6c4adcf5980125b1b8e22089cce2bdbd20bfb40616dc7658f7300764da661",
            ),
        ] {
            let native = source["node_id"].as_str().unwrap();
            let payload = serde_json::to_vec(&source).unwrap();
            let row = SeekRow {
                id: native.into(),
                source_graph: "source-navigation".into(),
                source_order: None,
                payload_sha256: Digest256::of_bytes(&payload).to_hex(),
                payload,
            };
            let base = normalizer.normalize_base(&row, &prepared).unwrap();
            let output = base.value();
            assert_eq!(base.content_revision, expected_revision);
            assert_eq!(output["content_revision"], expected_revision);
            assert_eq!(stable_digest(output).unwrap(), expected_digest);
            assert_eq!(output["source_record"]["digest"], expected_record);
            assert_eq!(output["native_id"], native);
            if native == "tos.work.alpha" {
                assert_eq!(output["source_dossier_ref"], native);
                assert_eq!(output["display"]["kind_label"]["ru"], "Произведение");
                assert_eq!(
                    output["semantics"]["type_ancestors"]
                        .as_array()
                        .unwrap()
                        .len(),
                    4
                );
            }
        }
    }
}
