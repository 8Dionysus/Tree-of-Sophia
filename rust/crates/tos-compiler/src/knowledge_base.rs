//! Shared bounded base assembly for ordinary owner node/edge carriers.
//! These pure mechanics normalize selected registry mappings and preserve raw
//! source records; source adapters still own input completeness and admission.
use crate::knowledge_normalization::{SourceRow, stamp_content_revision};
use crate::knowledge_philosophy_display::{
    full_owner_relation_display, full_philosophy_node_display,
};
use crate::knowledge_source_navigation_node::{epistemic, normalized_time};
use crate::knowledge_source_navigation_relation::direct_assertion_context;
use crate::{Error, KnowledgeRegistry, QueryVocabulary, Result};
use serde_json::{Map, Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write,
};
use tos_foundation::Digest256;

#[derive(Clone, Copy, Default)]
pub(crate) struct BaseNodeOverrides<'a> {
    pub native_id: Option<&'a str>,
    pub identity_id: Option<&'a str>,
    pub kind_id: Option<&'a str>,
}
#[derive(Clone, Copy)]
pub(crate) struct BaseNormalizationLimits {
    pub max_registry_bytes: usize,
    pub max_output_bytes: usize,
}
pub(crate) struct KnowledgeBaseNormalizer<'a> {
    registry: &'a KnowledgeRegistry,
    sources: BTreeSet<String>,
    entity_registry_ref: String,
    relation_registry_ref: String,
    entities: BTreeMap<String, Value>,
    relations: BTreeMap<String, Value>,
    limits: BaseNormalizationLimits,
}
struct CappedBytes {
    bytes: usize,
    cap: usize,
}
impl Write for CappedBytes {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.bytes = self
            .bytes
            .checked_add(bytes.len())
            .filter(|n| *n <= self.cap)
            .ok_or_else(|| std::io::Error::other("owner base title byte ceiling"))?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn required<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.len() <= 4096 && !s.contains('\0'))
        .ok_or(Error::Invalid("owner base normalized field"))
}
fn text(value: Option<&Value>) -> Option<&str> {
    value?.as_str().map(str::trim).filter(|s| !s.is_empty())
}
fn strings(value: Option<&Value>) -> Vec<Value> {
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
fn truthy(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) | Some(Value::Bool(false)) => false,
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Number(n)) => n.as_f64().is_some_and(|n| n != 0.),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(o)) => !o.is_empty(),
        _ => true,
    }
}
fn attrs(item: &Value, relation: bool) -> Result<Map<String, Value>> {
    let mut output = item
        .get("properties")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let excluded = if relation {
        &[
            "id",
            "edge_id",
            "from_id",
            "to_id",
            "predicate_id",
            "display",
            "properties",
            "source_ref",
            "source_refs",
            "graph_layers",
            "view_ids",
            "from_source_graph",
            "to_source_graph",
        ][..]
    } else {
        &[
            "id",
            "node_id",
            "label",
            "canonical_label",
            "node_type",
            "node_kind",
            "resource_kind",
            "display",
            "multilingual",
            "properties",
            "source_ref",
            "source_refs",
            "graph_layers",
            "view_ids",
        ][..]
    };
    for (key, value) in item
        .as_object()
        .ok_or(Error::Invalid("owner base source object"))?
    {
        if !excluded.contains(&key.as_str()) {
            output.entry(key.clone()).or_insert_with(|| value.clone());
        }
    }
    Ok(output)
}
fn layers(item: &Value) -> Vec<Value> {
    let layers = strings(item.get("graph_layers"));
    if layers.is_empty() {
        text(item.get("layer"))
            .map(|s| vec![json!(s)])
            .unwrap_or_default()
    } else {
        layers
    }
}
fn entries(
    bytes: &[u8],
    cap: usize,
    collection: &str,
    id_field: &str,
) -> Result<BTreeMap<String, Value>> {
    let row = SourceRow::parse(bytes, cap)?;
    let mut entries = BTreeMap::new();
    for entry in row
        .value()
        .get(collection)
        .and_then(Value::as_array)
        .ok_or(Error::Invalid("owner base registry entries"))?
    {
        let id = required(entry, id_field)?.to_owned();
        if entries.insert(id, entry.clone()).is_some() {
            return Err(Error::Invalid("duplicate owner base registry entry"));
        }
    }
    Ok(entries)
}

fn bounded_source_value(value: &Value, cap: usize) -> Result<SourceRow> {
    serde_json::to_writer(&mut CappedBytes { bytes: 0, cap }, value)
        .map_err(|_| Error::Budget("owner base display source bytes"))?;
    let raw = serde_json::to_vec(value).map_err(|_| Error::Invalid("owner base display JSON"))?;
    SourceRow::parse(&raw, cap)
}
fn source_epistemic(item: &Value, default_authority: &str) -> Value {
    let mut value = epistemic(item);
    if text(
        item.get("properties")
            .and_then(|p| p.get("authority_posture")),
    )
    .is_none()
        && text(item.get("authority_layer")).is_none()
    {
        value["authority_layer"] = json!(default_authority);
    }
    value
}
impl<'a> KnowledgeBaseNormalizer<'a> {
    pub(crate) fn new(
        registry: &'a KnowledgeRegistry,
        entity_bytes: &[u8],
        relation_bytes: &[u8],
        vocabulary: &QueryVocabulary,
        descriptor_bytes: &[u8],
        limits: BaseNormalizationLimits,
    ) -> Result<Self> {
        if limits.max_registry_bytes == 0
            || limits.max_registry_bytes > 4 * 1024 * 1024
            || limits.max_output_bytes == 0
            || limits.max_output_bytes > 8 * 1024 * 1024
        {
            return Err(Error::Budget("owner base limits"));
        }
        vocabulary.verify_authored_bytes(descriptor_bytes)?;
        if registry.entity_registry_id != vocabulary.entity_registry_id
            || registry.relation_registry_id != vocabulary.relation_registry_id
        {
            return Err(Error::Invalid("owner base descriptor registry identity"));
        }
        if entity_bytes.len() > limits.max_registry_bytes
            || relation_bytes.len() > limits.max_registry_bytes
            || Digest256::of_bytes(entity_bytes).to_hex() != registry.entity_sha256
            || Digest256::of_bytes(relation_bytes).to_hex() != registry.relation_sha256
        {
            return Err(Error::Invalid("owner base selected registry bytes"));
        }
        let descriptor = SourceRow::parse(descriptor_bytes, 1024 * 1024)?;
        let refs = descriptor
            .value()
            .get("semantic_registry_refs")
            .ok_or(Error::Invalid("owner base registry refs"))?;
        Ok(Self {
            registry,
            sources: vocabulary.registered_source_ids.iter().cloned().collect(),
            entity_registry_ref: required(
                refs.get("entity")
                    .ok_or(Error::Invalid("owner base entity ref"))?,
                "source_ref",
            )?
            .to_owned(),
            relation_registry_ref: required(
                refs.get("relation")
                    .ok_or(Error::Invalid("owner base relation ref"))?,
                "source_ref",
            )?
            .to_owned(),
            entities: entries(entity_bytes, limits.max_registry_bytes, "types", "type_id")?,
            relations: entries(
                relation_bytes,
                limits.max_registry_bytes,
                "relations",
                "relation_type_id",
            )?,
            limits,
        })
    }
    pub(crate) fn normalize_node(
        &self,
        source: &SourceRow,
        source_graph: &str,
        canonical: bool,
        overrides: BaseNodeOverrides<'_>,
    ) -> Result<Value> {
        if !self.sources.contains(source_graph) {
            return Err(Error::Invalid("owner base unregistered source"));
        }
        let default_authority = if canonical { "canon" } else { "derived-export" };
        let item = source.value();
        let native = overrides
            .native_id
            .or_else(|| text(item.get("node_id")))
            .or_else(|| text(item.get("id")))
            .or_else(|| text(item.get("path")))
            .unwrap_or("unnamed");
        let props = item.get("properties").and_then(Value::as_object);
        let p = |key: &str| props.and_then(|props| props.get(key));
        let kind = overrides
            .kind_id
            .or_else(|| text(p("original_node_type")))
            .or_else(|| text(item.get("node_type")))
            .or_else(|| text(item.get("node_kind")))
            .or_else(|| text(item.get("resource_kind")))
            .unwrap_or("knowledge-object");
        let resolved = self.registry.entity(source_graph, kind);
        let type_id = resolved.type_id;
        let entry = self
            .entities
            .get(type_id)
            .ok_or(Error::Invalid("owner base entity type entry"))?;
        let labels = entry
            .get("source_mappings")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .find(|mapping| {
                mapping.get("source_graph").and_then(Value::as_str) == Some(source_graph)
                    && mapping.get("source_kind_id").and_then(Value::as_str) == Some(kind)
                    && truthy(mapping.get("labels"))
            })
            .and_then(|mapping| mapping.get("labels"))
            .or_else(|| entry.get("labels"));
        let mut ancestors = BTreeSet::new();
        let mut frontier = vec![type_id.to_owned()];
        while let Some(id) = frontier.pop() {
            if ancestors.insert(id.clone()) {
                let entry = self
                    .entities
                    .get(&id)
                    .ok_or(Error::Invalid("owner base ancestor entry"))?;
                frontier.extend(
                    entry
                        .get("parent_type_ids")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .map(|v| {
                            v.as_str()
                                .map(str::to_owned)
                                .ok_or(Error::Invalid("owner base parent ID"))
                        })
                        .collect::<Result<Vec<_>>>()?,
                );
            }
        }
        let refs = source.source_refs(&[]);
        let display_owner;
        let display_source = if canonical && props.is_some_and(|p| p.contains_key("node_id")) {
            let properties = props.ok_or(Error::Invalid("canonical retained source"))?;
            if properties.get("node_id") != item.get("node_id")
                || properties.get("node_type") != item.get("node_type")
            {
                return Err(Error::Invalid("canonical retained source identity/type"));
            }
            let mut display_item = properties.clone();
            display_item.insert("properties".into(), Value::Object(properties.clone()));
            display_owner =
                bounded_source_value(&Value::Object(display_item), self.limits.max_output_bytes)?;
            &display_owner
        } else {
            source
        };
        let attributes = attrs(item, false)?;
        let mut semantics = Map::new();
        if let Some(multilingual) = item.get("multilingual").and_then(Value::as_object) {
            semantics.insert(
                "language_context".into(),
                Value::Object(
                    multilingual
                        .iter()
                        .filter(|(k, _)| k.as_str() != "label")
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect(),
                ),
            );
        }
        if let Some(context) = direct_assertion_context(source, self.limits.max_output_bytes)? {
            semantics.insert("assertion_contexts".into(), json!([context]));
        }
        if truthy(p("packet_id")) {
            semantics.insert("annotation".into(),json!({"packet_id":p("packet_id"),"packet_version":p("packet_version"),"content_available":p("content_available"),"publication_posture":p("publication_posture")}));
            if kind == "annotation-claim" {
                semantics.insert("claim".into(),json!({"claim_id":p("claim_id"),"claim_version":p("claim_version"),"proposition":p("proposition"),"review_status":p("claim_status"),"contract_ref":"ToS/contracts/semantic-annotation-packet-v2.schema.json"}));
            }
        }
        let period_field = if props.is_some_and(|p| p.contains_key("period")) {
            "properties.period"
        } else {
            "temporal_context"
        };
        if let Some(time) = normalized_time(
            p("period").or_else(|| item.get("temporal_context")),
            period_field,
        )? {
            semantics.insert("time".into(), time);
        }
        semantics.insert("type_ancestors".into(), json!(ancestors));
        let id = format!(
            "{}:{}",
            source_graph,
            overrides.identity_id.unwrap_or(native)
        );
        let entity = [p("record_id"), item.get("record_id"), item.get("node_id")]
            .into_iter()
            .find_map(|v| text(v).filter(|s| s.starts_with("tos.")))
            .or_else(|| native.starts_with("tos.").then_some(native))
            .unwrap_or(&id);
        let mut value = json!({"id":id,"entity_id":entity,"native_id":native,"source_graph":source_graph,"kind_id":kind,"type_id":type_id,
            "type_mapping":{"status":if type_id != self.registry.fallback_entity_type_id() {"mapped"} else {"unmapped"},"source_kind_id":kind,"registry_ref":self.entity_registry_ref},
            "display":full_philosophy_node_display(display_source,kind,labels,entry.get("object_role").and_then(Value::as_str))?,"epistemic":source_epistemic(item,default_authority),"graph_layers":layers(item),"view_ids":strings(item.get("view_ids")),
            "source_refs":refs,"source_record":source.source_record(&attributes)?,"attributes":attributes,"semantics":semantics});
        if canonical
            && !value["display"]["provenance"]["source_title_available"]
                .as_bool()
                .unwrap_or(false)
        {
            if let Some(label) = text(item.get("label")) {
                value["display"]["title"]["default"] = json!(label);
            }
        }
        stamp_content_revision(&mut value, self.limits.max_output_bytes)?;
        Ok(value)
    }
    pub(crate) fn normalize_relation(
        &self,
        source: &SourceRow,
        source_graph: &str,
        identity: Option<&str>,
        left_title: &Value,
        right_title: &Value,
        default_authority: &str,
    ) -> Result<Value> {
        if !self.sources.contains(source_graph) {
            return Err(Error::Invalid("owner base unregistered source"));
        }
        for title in [left_title, right_title] {
            if !title.is_object() {
                return Err(Error::Invalid("owner base title object"));
            }
            serde_json::to_writer(
                &mut CappedBytes {
                    bytes: 0,
                    cap: self.limits.max_output_bytes,
                },
                title,
            )
            .map_err(|_| Error::Budget("owner base title bytes"))?;
        }
        let native = required(source.value(), "edge_id")?;
        let item = source.value();
        let (from, to) = endpoints(item, source_graph)?;
        let predicate = text(item.get("predicate_id")).unwrap_or("related_to");
        let resolved = self.registry.relation(source_graph, predicate, "edge");
        let type_id = resolved.type_id;
        let mut entry = self
            .relations
            .get(type_id)
            .cloned()
            .ok_or(Error::Invalid("owner base relation type entry"))?;
        let mapping = entry
            .get("source_mappings")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .find(|mapping| {
                mapping.get("source_graph").and_then(Value::as_str) == Some(source_graph)
                    && mapping.get("source_predicate_id").and_then(Value::as_str) == Some(predicate)
                    && mapping.get("scope").and_then(Value::as_str) == Some("edge")
                    && truthy(mapping.get("labels"))
            })
            .cloned();
        if let Some(mapping) = mapping {
            entry["labels"] = mapping["labels"].clone();
            entry["definition"] = mapping.get("definition").cloned().unwrap_or(Value::Null);
            entry["source_mappings"] = json!([mapping]);
        }
        let attributes = attrs(item, true)?;
        let mut semantics = Map::new();
        if let Some(context) = direct_assertion_context(source, self.limits.max_output_bytes)? {
            semantics.insert("assertion_contexts".into(), json!([context]));
        }
        for (special, key, roles, forms) in [
            (
                "tos.relation.has-normalized-place",
                "space",
                "spatial_roles",
                "spatial_literal_forms",
            ),
            (
                "tos.relation.has-normalized-agent",
                "responsibility",
                "agent_roles",
                "agent_literal_forms",
            ),
        ] {
            if type_id == special {
                let p = item.get("properties");
                let sorted = |key| {
                    strings(p.and_then(|p| p.get(key)))
                        .into_iter()
                        .filter_map(|v| v.as_str().map(str::to_owned))
                        .collect::<BTreeSet<_>>()
                };
                semantics.insert(key.into(),json!({"roles":sorted(roles),"literal_forms":sorted(forms),"normalization_status":"source-declared"}));
            }
        }
        let mut value = json!({"id":format!("{}:{}",source_graph,identity.unwrap_or(native)),"native_id":native,"source_graph":source_graph,
            "from_id":from,"to_id":to,"predicate_id":predicate,"relation_type_id":type_id,
            "predicate_mapping":{"status":if resolved.mapped {"mapped"} else {"unmapped"},"source_predicate_id":predicate,"registry_ref":self.relation_registry_ref},
            "display":full_owner_relation_display(source,predicate,left_title,right_title,&entry)?,"epistemic":source_epistemic(item,default_authority),
            "graph_layers":layers(item),"view_ids":strings(item.get("view_ids")),"source_refs":source.source_refs(&[]),
            "source_record":source.source_record(&attributes)?,"attributes":attributes,"semantics":semantics});
        stamp_content_revision(&mut value, self.limits.max_output_bytes)?;
        Ok(value)
    }
}
fn endpoints(item: &Value, source: &str) -> Result<(String, String)> {
    Ok((
        format!(
            "{}:{}",
            text(item.get("from_source_graph")).unwrap_or(source),
            text(item.get("from_id")).unwrap_or("unknown-source")
        ),
        format!(
            "{}:{}",
            text(item.get("to_source_graph")).unwrap_or(source),
            text(item.get("to_id")).unwrap_or("unknown-target")
        ),
    ))
}
