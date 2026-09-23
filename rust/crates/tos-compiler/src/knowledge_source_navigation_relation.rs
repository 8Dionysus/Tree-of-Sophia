//! Exact direct assertion context and private relation dependency addresses.
//!
//! Full Python relation normalization additionally consumes global endpoint
//! titles and Claim groups from all base nodes. This phase refuses to turn
//! prepared source-navigation edges into final graph rows without those joins.

use crate::knowledge_normalization::{SourceRow, stable_digest, stamp_content_revision};
use crate::knowledge_philosophy_display::ordinary_philosophy_relation_display;
use crate::knowledge_source_navigation_prepare::NavigationPrepareReceipt;
use crate::knowledge_stage::{KnowledgeStage, SeekRow, WritePhase};
use crate::{Error, KnowledgeRegistry, QueryVocabulary, Result};
use rusqlite::{OptionalExtension, params};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use tos_foundation::{Digest256, Digest256Hasher};

const PROFILE: &str = "source-navigation-node-edge-v1";
const ASSERTION_FIELDS: &[&str] = &[
    "claim_id",
    "claim_ref",
    "claim_version",
    "claim_type",
    "assertion_layer",
    "subject_ref",
    "predicate",
    "object",
    "proposition",
    "qualifiers",
    "polarity",
    "negated",
    "condition",
    "conditions",
    "attribution",
    "scope",
    "temporal_context",
    "spatial_context",
    "epistemic_status",
    "review_status",
    "claim_status",
    "review_refs",
    "reviews",
    "assessment_refs",
    "confidence",
    "maker",
    "method_ref",
    "evidence_refs",
    "counterevidence_refs",
    "alternative_claim_refs",
    "competing_claim_refs",
    "supersedes_claim_ref",
    "provenance_event_ref",
    "visibility",
];
const ASSERTION_TRIGGERS: &[&str] = &[
    "claim_id",
    "claim_ref",
    "claim_version",
    "claim_type",
    "assertion_layer",
    "proposition",
    "qualifiers",
    "polarity",
    "negated",
    "condition",
    "conditions",
    "attribution",
    "epistemic_status",
    "review_status",
    "claim_status",
    "review_refs",
    "reviews",
    "assessment_refs",
    "evidence_refs",
    "counterevidence_refs",
    "alternative_claim_refs",
    "competing_claim_refs",
    "supersedes_claim_ref",
    "provenance_event_ref",
];

#[derive(Clone, Copy, Debug)]
pub struct NavigationRelationLimits {
    pub max_edges: u64,
    pub max_page_rows: usize,
    pub max_raw_bytes: usize,
    pub max_context_bytes: usize,
    pub max_page_bytes: u64,
    pub max_work_bytes: u64,
}
impl NavigationRelationLimits {
    fn validate(self) -> Result<()> {
        if self.max_edges == 0
            || self.max_page_rows == 0
            || self.max_page_rows > 1024
            || self.max_raw_bytes == 0
            || self.max_raw_bytes > 8 * 1024 * 1024
            || self.max_context_bytes == 0
            || self.max_context_bytes > 8 * 1024 * 1024
            || self.max_page_bytes == 0
            || self.max_page_bytes > 64 * 1024 * 1024
            || self.max_work_bytes == 0
            || (self.max_page_rows as u64)
                .checked_mul(self.max_raw_bytes as u64)
                .is_none_or(|worst| worst > self.max_page_bytes)
        {
            return Err(Error::Budget("navigation relation preparation limits"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct NavigationRelationDependencyReceipt {
    pub source_graph: String,
    pub source_cut: String,
    pub edges: u64,
    pub direct_contexts: u64,
    pub referenced_claim_keys: u64,
    pub edge_input_root_sha256: String,
    pub dependency_root_sha256: String,
    pub final_relation_rows_written: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct NavigationRelationNormalizeLimits {
    pub max_raw_bytes: usize,
    pub max_output_bytes: usize,
    pub max_registry_bytes: usize,
    pub max_claim_contexts: usize,
    pub max_global_input_bytes: usize,
}
impl NavigationRelationNormalizeLimits {
    fn validate(self) -> Result<()> {
        if self.max_raw_bytes == 0
            || self.max_raw_bytes > 8 * 1024 * 1024
            || self.max_output_bytes == 0
            || self.max_output_bytes > 8 * 1024 * 1024
            || self.max_registry_bytes == 0
            || self.max_registry_bytes > 4 * 1024 * 1024
            || self.max_claim_contexts == 0
            || self.max_claim_contexts > 4096
            || self.max_global_input_bytes == 0
            || self.max_global_input_bytes > 8 * 1024 * 1024
        {
            return Err(Error::Budget("navigation relation normalizer limits"));
        }
        Ok(())
    }
}
struct ByteCounter {
    bytes: usize,
    cap: usize,
}
impl Write for ByteCounter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.bytes = self
            .bytes
            .checked_add(buf.len())
            .filter(|n| *n <= self.cap)
            .ok_or_else(|| std::io::Error::other("navigation global dependency byte ceiling"))?;
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn check_global_bytes(value: &Value, counter: &mut ByteCounter) -> Result<()> {
    serde_json::to_writer(counter, value)
        .map_err(|_| Error::Budget("navigation global dependency bytes"))
}

/// Exact global inputs must be supplied by the all-source title/Claim join.
/// Their root strings are obligations carried by the private base relation,
/// not proof that this module has completed that global join.
pub struct NavigationRelationGlobalInputs<'a> {
    pub source_cut: &'a str,
    pub endpoint_title_root_sha256: &'a str,
    pub claim_group_root_sha256: &'a str,
    pub left_title: &'a Value,
    pub right_title: &'a Value,
    pub referenced_claim_contexts: &'a [Value],
}

pub struct NavigationBaseRelation {
    value: Value,
    pub source_cut: String,
    pub prepared_dependency_root_sha256: String,
    pub relation_dependency_root_sha256: String,
    pub raw_edge_sha256: String,
    pub relation_registry_sha256: String,
    pub endpoint_title_root_sha256: String,
    pub claim_group_root_sha256: String,
    pub content_revision: String,
}
impl NavigationBaseRelation {
    pub fn value(&self) -> &Value {
        &self.value
    }
}

pub struct NavigationRelationNormalizer<'a> {
    registry: &'a KnowledgeRegistry,
    source_graph: String,
    relation_registry_ref: String,
    types: BTreeMap<String, Value>,
    limits: NavigationRelationNormalizeLimits,
}

impl<'a> NavigationRelationNormalizer<'a> {
    pub fn new(
        registry: &'a KnowledgeRegistry,
        relation_bytes: &[u8],
        vocabulary: &QueryVocabulary,
        descriptor_bytes: &[u8],
        limits: NavigationRelationNormalizeLimits,
    ) -> Result<Self> {
        limits.validate()?;
        vocabulary.verify_authored_bytes(descriptor_bytes)?;
        if relation_bytes.is_empty()
            || relation_bytes.len() > limits.max_registry_bytes
            || Digest256::of_bytes(relation_bytes).to_hex() != registry.relation_sha256
        {
            return Err(Error::Invalid(
                "navigation selected relation registry bytes",
            ));
        }
        let descriptor = SourceRow::parse(descriptor_bytes, 1024 * 1024)?;
        let source_graph = required(
            descriptor
                .value()
                .get("identity")
                .ok_or(Error::Invalid("navigation descriptor identity"))?,
            "source_dossier_graph_id",
        )?
        .to_owned();
        let sources: Vec<_> = vocabulary
            .sources
            .iter()
            .filter(|s| s.adapter_profile == PROFILE)
            .collect();
        if sources.len() != 1 || sources[0].source_graph_id != source_graph {
            return Err(Error::Invalid("navigation selected relation profile"));
        }
        let relation_registry_ref = required(
            descriptor
                .value()
                .get("semantic_registry_refs")
                .and_then(|v| v.get("relation"))
                .ok_or(Error::Invalid("navigation relation registry descriptor"))?,
            "source_ref",
        )?
        .to_owned();
        let registry_source = SourceRow::parse(relation_bytes, limits.max_registry_bytes)?;
        let entries = registry_source
            .value()
            .get("relations")
            .and_then(Value::as_array)
            .ok_or(Error::Invalid("navigation relation registry entries"))?;
        let mut types = BTreeMap::new();
        for entry in entries {
            let id = required(entry, "relation_type_id")?.to_owned();
            if types.insert(id, entry.clone()).is_some() {
                return Err(Error::Invalid("duplicate navigation relation type"));
            }
        }
        Ok(Self {
            registry,
            source_graph,
            relation_registry_ref,
            types,
            limits,
        })
    }

    fn effective_type(&self, type_id: &str, predicate: &str) -> Result<Value> {
        let mut entry = self
            .types
            .get(type_id)
            .cloned()
            .ok_or(Error::Invalid("navigation relation type entry"))?;
        let mapping = entry
            .get("source_mappings")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .find(|mapping| {
                mapping.get("source_graph").and_then(Value::as_str)
                    == Some(self.source_graph.as_str())
                    && mapping.get("source_predicate_id").and_then(Value::as_str) == Some(predicate)
                    && mapping.get("scope").and_then(Value::as_str) == Some("edge")
                    && mapping.get("labels").is_some_and(|v| {
                        !v.is_null() && v.as_object().is_some_and(|o| !o.is_empty())
                    })
            })
            .cloned();
        if let Some(mapping) = mapping {
            let object = entry
                .as_object_mut()
                .ok_or(Error::Invalid("navigation relation type object"))?;
            object.insert("labels".into(), mapping["labels"].clone());
            object.insert(
                "definition".into(),
                mapping.get("definition").cloned().unwrap_or(Value::Null),
            );
            object.insert("source_mappings".into(), Value::Array(vec![mapping]));
        }
        Ok(entry)
    }

    /// Produce only a private Python-compatible base relation for supported
    /// ordinary source-navigation edge envelopes. The two global root values
    /// remain explicit obligations; no final candidate conversion exists.
    pub fn normalize_base(
        &self,
        raw: &SeekRow,
        prepared: &NavigationPrepareReceipt,
        dependency: &NavigationRelationDependencyReceipt,
        global: NavigationRelationGlobalInputs<'_>,
    ) -> Result<NavigationBaseRelation> {
        if raw.source_graph != self.source_graph
            || raw.source_order.is_some()
            || prepared.source_graph != self.source_graph
            || dependency.source_graph != self.source_graph
            || prepared.source_cut != dependency.source_cut
            || prepared.source_cut != global.source_cut
            || prepared.final_graph_rows_written
            || dependency.final_relation_rows_written
            || dependency.edge_input_root_sha256 != prepared.edge_input_root_sha256
            || Digest256::from_hex(&dependency.dependency_root_sha256).is_err()
            || Digest256::from_hex(global.endpoint_title_root_sha256).is_err()
            || Digest256::from_hex(global.claim_group_root_sha256).is_err()
            || raw.payload.len() > self.limits.max_raw_bytes
            || Digest256::of_bytes(&raw.payload).to_hex() != raw.payload_sha256
        {
            return Err(Error::Invalid("navigation private relation binding"));
        }
        let source = SourceRow::parse(&raw.payload, self.limits.max_raw_bytes)?;
        let item = source.value();
        let native = required(item, "edge_id")?;
        if native != raw.id || native.trim() != native || item.get("display").is_some() {
            return Err(Error::Invalid("unsupported navigation relation carrier"));
        }
        let from = required(item, "from_id")?.trim();
        let to = required(item, "to_id")?.trim();
        let from_graph = normalized_source(item, "from_source_graph", &self.source_graph);
        let to_graph = normalized_source(item, "to_source_graph", &self.source_graph);
        let from_id = format!("{from_graph}:{from}");
        let to_id = format!("{to_graph}:{to}");
        let predicate = required(item, "predicate_id")?.trim();
        let resolved = self
            .registry
            .relation(&self.source_graph, predicate, "edge");
        let type_id = resolved.type_id;
        let effective = self.effective_type(type_id, predicate)?;
        if !global.left_title.is_object()
            || !global.right_title.is_object()
            || global.referenced_claim_contexts.len() > self.limits.max_claim_contexts
        {
            return Err(Error::Invalid("navigation global title/context values"));
        }
        let mut global_bytes = ByteCounter {
            bytes: 0,
            cap: self.limits.max_global_input_bytes,
        };
        check_global_bytes(global.left_title, &mut global_bytes)?;
        check_global_bytes(global.right_title, &mut global_bytes)?;
        for context in global.referenced_claim_contexts {
            check_global_bytes(context, &mut global_bytes)?;
        }
        let claim_ref = item.get("claim_ref").and_then(Value::as_str);
        if claim_ref.is_none() && !global.referenced_claim_contexts.is_empty() {
            return Err(Error::Invalid("navigation unkeyed referenced contexts"));
        }
        let mut seen_contexts = BTreeSet::new();
        for context in global.referenced_claim_contexts {
            if !seen_contexts.insert(stable_digest(context)?) {
                return Err(Error::Invalid(
                    "duplicate navigation referenced Claim context",
                ));
            }
            let fields = context
                .get("fields")
                .and_then(Value::as_object)
                .ok_or(Error::Invalid("navigation referenced Claim context"))?;
            let ref_value = fields
                .get("claim_id")
                .or_else(|| fields.get("claim_ref"))
                .and_then(|f| f.get("value"))
                .and_then(Value::as_str);
            if context.get("binding_role").and_then(Value::as_str) != Some("referenced-claim")
                || ref_value != claim_ref
            {
                return Err(Error::Invalid("navigation referenced Claim key"));
            }
        }
        let mut contexts = global.referenced_claim_contexts.to_vec();
        if let Some(direct) = direct_assertion_context(&source, self.limits.max_output_bytes)? {
            contexts.insert(0, direct);
        }
        let mut attrs = item
            .get("properties")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let object = item
            .as_object()
            .ok_or(Error::Invalid("navigation relation carrier object"))?;
        for (key, value) in object {
            if !matches!(
                key.as_str(),
                "id" | "edge_id"
                    | "from_id"
                    | "to_id"
                    | "predicate_id"
                    | "display"
                    | "properties"
                    | "source_ref"
                    | "source_refs"
                    | "graph_layers"
                    | "view_ids"
                    | "from_source_graph"
                    | "to_source_graph"
            ) && !attrs.contains_key(key)
            {
                attrs.insert(key.clone(), value.clone());
            }
        }
        let mut semantics = Map::new();
        if !contexts.is_empty() {
            semantics.insert("assertion_contexts".into(), Value::Array(contexts));
        }
        let properties = item.get("properties").and_then(Value::as_object);
        if type_id == "tos.relation.has-normalized-place" {
            semantics.insert("space".into(),json!({"roles":sorted_strings(properties.and_then(|p|p.get("spatial_roles"))),
                "literal_forms":sorted_strings(properties.and_then(|p|p.get("spatial_literal_forms"))),
                "normalization_status":"source-declared"}));
        }
        if type_id == "tos.relation.has-normalized-agent" {
            semantics.insert("responsibility".into(),json!({"roles":sorted_strings(properties.and_then(|p|p.get("agent_roles"))),
                "literal_forms":sorted_strings(properties.and_then(|p|p.get("agent_literal_forms"))),
                "normalization_status":"source-declared"}));
        }
        let display = ordinary_philosophy_relation_display(
            &source,
            predicate,
            global.left_title,
            global.right_title,
            &effective,
        )?;
        let layers = ordered_strings(item.get("graph_layers"));
        let layers = if layers.is_empty() {
            item.get("layer")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| vec![s.to_owned()])
                .unwrap_or_default()
        } else {
            layers
        };
        let source_record = source.source_record(&attrs)?;
        let mut value = json!({"id":format!("{}:{native}",self.source_graph),"native_id":native,
            "source_graph":self.source_graph,"from_id":from_id,"to_id":to_id,
            "predicate_id":predicate,"relation_type_id":type_id,
            "predicate_mapping":{"status":if resolved.mapped{"mapped"}else{"unmapped"},
                "source_predicate_id":predicate,"registry_ref":self.relation_registry_ref},
            "display":display,"epistemic":relation_epistemic(item),
            "graph_layers":layers,"view_ids":ordered_strings(item.get("view_ids")),
            "source_refs":source.source_refs(&[]),"attributes":attrs,
            "semantics":semantics,"source_record":source_record});
        stamp_content_revision(&mut value, self.limits.max_output_bytes)?;
        let content_revision = required(&value, "content_revision")?.to_owned();
        Ok(NavigationBaseRelation {
            value,
            source_cut: prepared.source_cut.clone(),
            prepared_dependency_root_sha256: prepared.dependency_root_sha256.clone(),
            relation_dependency_root_sha256: dependency.dependency_root_sha256.clone(),
            raw_edge_sha256: raw.payload_sha256.clone(),
            relation_registry_sha256: self.registry.relation_sha256.clone(),
            endpoint_title_root_sha256: global.endpoint_title_root_sha256.into(),
            claim_group_root_sha256: global.claim_group_root_sha256.into(),
            content_revision,
        })
    }

    /// Seek the sealed raw edge and its prepared dependency row by exact ID.
    /// Global titles/Claim groups remain explicit external proof obligations.
    pub fn normalize_indexed(
        &self,
        stage: &mut KnowledgeStage<'_>,
        native_id: &str,
        prepared: &NavigationPrepareReceipt,
        dependency: &NavigationRelationDependencyReceipt,
        global: NavigationRelationGlobalInputs<'_>,
    ) -> Result<NavigationBaseRelation> {
        if native_id.is_empty()
            || native_id.len() > 4096
            || native_id.trim() != native_id
            || stage.exact_receipt().binding.source_cut != prepared.source_cut
        {
            return Err(Error::Invalid("navigation indexed relation ID/cut"));
        }
        let raw = stage
            .raw_by_id(&prepared.source_graph, "edges", native_id)?
            .ok_or(Error::Invalid("navigation indexed raw edge absent"))?;
        let indexed:(Vec<u8>,String,String,Option<String>,Option<Vec<u8>>)=stage.with_connection(WritePhase::Sort,|db| {
            db.query_row("SELECT raw_sha256,from_id,to_id,claim_ref,direct_context_json FROM knowledge_navigation_relation_dependencies WHERE edge_id=?1",
                [native_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?)))
                .optional()?.ok_or(Error::Invalid("navigation indexed relation dependency absent"))
        })?;
        if indexed.0.as_slice() != Digest256::of_bytes(&raw.payload).as_bytes() {
            return Err(Error::Invalid("navigation indexed relation raw SHA"));
        }
        let value = SourceRow::parse(&raw.payload, self.limits.max_raw_bytes)?;
        let item = value.value();
        let from = format!(
            "{}:{}",
            normalized_source(item, "from_source_graph", &self.source_graph),
            required(item, "from_id")?.trim()
        );
        let to = format!(
            "{}:{}",
            normalized_source(item, "to_source_graph", &self.source_graph),
            required(item, "to_id")?.trim()
        );
        let claim = item.get("claim_ref").and_then(Value::as_str);
        let direct = direct_assertion_context(&value, self.limits.max_output_bytes)?;
        let indexed_context = indexed
            .4
            .as_deref()
            .map(|bytes| {
                serde_json::from_slice::<Value>(bytes)
                    .map_err(|_| Error::Invalid("navigation indexed direct context JSON"))
            })
            .transpose()?;
        if indexed.1 != from
            || indexed.2 != to
            || indexed.3.as_deref() != claim
            || indexed_context != direct
        {
            return Err(Error::Invalid(
                "navigation indexed relation dependency mismatch",
            ));
        }
        self.normalize_base(&raw, prepared, dependency, global)
    }
}

fn ordered_strings(value: Option<&Value>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for value in value.and_then(Value::as_array).into_iter().flatten() {
        if let Some(s) = value.as_str().filter(|s| !s.is_empty()) {
            if seen.insert(s.to_owned()) {
                result.push(s.to_owned());
            }
        }
    }
    result
}
fn sorted_strings(value: Option<&Value>) -> Vec<String> {
    ordered_strings(value)
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
fn relation_epistemic(item: &Value) -> Value {
    let props = item.get("properties").and_then(Value::as_object);
    let p = |key| props.and_then(|p| p.get(key));
    let text = |v: Option<&Value>| {
        v.and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
    };
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
        Value::String(s) if !s.trim().is_empty() => Some(Value::String(s.trim().into())),
        _ => None,
    })
    .unwrap_or(Value::Null);
    json!({"authority_layer":authority,"canon_status":canon,"review_posture":review,"confidence":confidence})
}

fn required<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.len() <= 4096 && !s.contains('\0'))
        .ok_or(Error::Invalid("navigation relation required field"))
}
fn normalized_source<'a>(value: &'a Value, key: &str, fallback: &'a str) -> &'a str {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(fallback)
}
fn root_text(hash: &mut Digest256Hasher, value: &str) {
    hash.update(&(value.len() as u64).to_be_bytes());
    hash.update(value.as_bytes());
}
fn root_item(hash: &mut Digest256Hasher, id: &str, sha: &[u8; 32]) {
    root_text(hash, id);
    hash.update(sha);
}

/// Python `_assertion_context` over one exact carrier. The three layers are
/// visited in Python priority order: outer, properties, embedded source_claim.
/// Missing/null/false/empty values remain distinct; conflicts retain both.
pub fn direct_assertion_context(
    row: &SourceRow,
    max_context_bytes: usize,
) -> Result<Option<Value>> {
    if max_context_bytes == 0 || max_context_bytes > 8 * 1024 * 1024 {
        return Err(Error::Budget("navigation assertion context bytes"));
    }
    let item = row.value();
    let outer = item
        .as_object()
        .ok_or(Error::Invalid("navigation assertion carrier"))?;
    let properties = item.get("properties").and_then(Value::as_object);
    let embedded = properties
        .and_then(|p| p.get("source_claim"))
        .and_then(Value::as_object);
    let mut layers: Vec<(&Map<String, Value>, &str)> = vec![(outer, "")];
    if let Some(p) = properties {
        layers.push((p, "/properties"));
    }
    if let Some(e) = embedded {
        layers.push((e, "/properties/source_claim"));
    }
    if !layers.iter().any(|(layer, _)| {
        ASSERTION_TRIGGERS
            .iter()
            .any(|key| layer.contains_key(*key))
    }) {
        return Ok(None);
    }
    let mut fields = Map::new();
    let mut conflicts = Vec::new();
    for (layer, prefix) in layers {
        for key in ASSERTION_FIELDS {
            let Some(value) = layer.get(*key) else {
                continue;
            };
            let entry = json!({"value":value,"source_pointer":format!("{prefix}/{key}")});
            if let Some(prior) = fields.get(*key) {
                if stable_digest(
                    prior
                        .get("value")
                        .ok_or(Error::Invalid("navigation assertion prior"))?,
                )? != stable_digest(value)?
                {
                    conflicts.push(
                        json!({"field":key,"lower_priority":prior,"higher_priority":entry.clone()}),
                    );
                }
            }
            fields.insert((*key).to_owned(), entry);
        }
    }
    let context = json!({"schema_version":"tos_assertion_context_v1","binding_role":"carrier",
        "source_record_digest":row.stable_digest()?,"source_refs":row.source_refs(&[]),
        "fields":fields,"conflicts":conflicts,
        "interpretation":"source-declared-not-semantic-assessment"});
    let serialized =
        serde_json::to_vec(&context).map_err(|_| Error::Invalid("navigation assertion JSON"))?;
    if serialized.len() > max_context_bytes {
        return Err(Error::Budget("navigation assertion context bytes"));
    }
    Ok(Some(context))
}

struct DependencyRow {
    edge_id: String,
    sha: [u8; 32],
    from_id: String,
    to_id: String,
    claim_ref: Option<String>,
    context_json: Option<Vec<u8>>,
}
fn create_table(stage: &mut KnowledgeStage<'_>) -> Result<()> {
    stage.with_connection(WritePhase::Schema, |db| {
        db.execute_batch(
            r#"
CREATE TABLE knowledge_navigation_relation_dependencies(
 edge_id TEXT PRIMARY KEY, raw_sha256 BLOB NOT NULL CHECK(length(raw_sha256)=32),
 from_id TEXT NOT NULL, to_id TEXT NOT NULL, claim_ref TEXT,
 direct_context_json BLOB);
CREATE INDEX knowledge_navigation_relation_claim_seek
 ON knowledge_navigation_relation_dependencies(claim_ref,edge_id);
"#,
        )?;
        Ok(())
    })
}
fn verify_prepared(
    stage: &mut KnowledgeStage<'_>,
    prepared: &NavigationPrepareReceipt,
    vocabulary: &QueryVocabulary,
    limits: NavigationRelationLimits,
) -> Result<()> {
    let matches: Vec<_> = vocabulary
        .sources
        .iter()
        .filter(|source| source.adapter_profile == PROFILE)
        .collect();
    if matches.len() != 1
        || matches[0].source_graph_id != prepared.source_graph
        || matches[0].input_role != prepared.input_role
        || stage.exact_receipt().binding.source_cut != prepared.source_cut
        || prepared.final_graph_rows_written
        || prepared.edges > limits.max_edges
        || Digest256::from_hex(&prepared.dependency_root_sha256).is_err()
    {
        return Err(Error::Invalid("navigation relation selected preparation"));
    }
    let entry = stage
        .exact_receipt()
        .collections
        .iter()
        .find(|entry| entry.source_graph == prepared.source_graph && entry.collection == "edges")
        .ok_or(Error::Invalid("navigation relation edge registration"))?;
    if entry.adapter_profile != PROFILE
        || entry.input_role != prepared.input_role
        || entry.expected_count != prepared.edges
        || entry.expected_root_sha256 != prepared.edge_input_root_sha256
    {
        return Err(Error::Invalid("navigation relation edge receipt"));
    }
    Ok(())
}
fn verify_indexed_edge(
    stage: &mut KnowledgeStage<'_>,
    source: &str,
    row: &SeekRow,
    value: &Value,
    from_id: &str,
    to_id: &str,
) -> Result<()> {
    stage.with_connection(WritePhase::Sort, |db| {
        let indexed: Option<Vec<u8>>=db.query_row("SELECT raw_sha256 FROM knowledge_navigation_edges WHERE edge_id=?1",[&row.id],|r|r.get(0)).optional()?;
        if indexed.as_deref()!=Some(&Digest256::of_bytes(&row.payload).as_bytes()[..]) {
            return Err(Error::Invalid("navigation relation prepared edge digest"));
        }
        for (role,expected) in [("from",from_id),("to",to_id)] {
            let actual: Option<(String,String)>=db.query_row("SELECT source_graph,native_id FROM knowledge_navigation_endpoints WHERE edge_id=?1 AND endpoint_role=?2",
                params![row.id,role],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
            let (graph,native)=actual.ok_or(Error::Invalid("navigation relation prepared endpoint"))?;
            let expected_graph=normalized_source(value,if role=="from"{"from_source_graph"}else{"to_source_graph"},source);
            if graph!=expected_graph || native!=expected {return Err(Error::Invalid("navigation relation endpoint mismatch"));}
        }
        Ok(())
    })
}
fn prepare_inner(
    stage: &mut KnowledgeStage<'_>,
    prepared: &NavigationPrepareReceipt,
    vocabulary: &QueryVocabulary,
    limits: NavigationRelationLimits,
) -> Result<NavigationRelationDependencyReceipt> {
    limits.validate()?;
    verify_prepared(stage, prepared, vocabulary, limits)?;
    create_table(stage)?;
    let mut after: Option<String> = None;
    let mut count = 0u64;
    let mut work = 0u64;
    let mut direct = 0u64;
    let mut claims = 0u64;
    let mut root = Digest256Hasher::new();
    loop {
        let page = stage.scan_input(
            &prepared.source_graph,
            "edges",
            after.as_deref(),
            limits.max_page_rows,
        )?;
        let page_bytes = page.rows.iter().try_fold(0u64, |sum, row| {
            sum.checked_add(row.payload.len() as u64)
                .ok_or(Error::Budget("navigation relation page bytes"))
        })?;
        if page_bytes > limits.max_page_bytes {
            return Err(Error::Budget("navigation relation page bytes"));
        }
        let mut rows = Vec::with_capacity(page.rows.len());
        for raw in &page.rows {
            count = count
                .checked_add(1)
                .ok_or(Error::Budget("navigation relation rows"))?;
            if count > limits.max_edges {
                return Err(Error::Budget("navigation relation rows"));
            }
            work = work
                .checked_add(raw.payload.len() as u64)
                .ok_or(Error::Budget("navigation relation work"))?;
            if work > limits.max_work_bytes {
                return Err(Error::Budget("navigation relation work"));
            }
            let digest = Digest256::of_bytes(&raw.payload);
            if digest.to_hex() != raw.payload_sha256 {
                return Err(Error::Invalid("navigation relation raw digest"));
            }
            root_item(&mut root, &raw.id, digest.as_bytes());
            let source = SourceRow::parse(&raw.payload, limits.max_raw_bytes)?;
            let value = source.value();
            if required(value, "edge_id")? != raw.id {
                return Err(Error::Invalid("navigation relation edge identity"));
            }
            let from = required(value, "from_id")?;
            let to = required(value, "to_id")?;
            verify_indexed_edge(stage, &prepared.source_graph, raw, value, from, to)?;
            let from_graph = normalized_source(value, "from_source_graph", &prepared.source_graph);
            let to_graph = normalized_source(value, "to_source_graph", &prepared.source_graph);
            let from_id = format!("{from_graph}:{from}");
            let to_id = format!("{to_graph}:{to}");
            let context = direct_assertion_context(&source, limits.max_context_bytes)?;
            let context_json = context
                .map(|v| {
                    serde_json::to_vec(&v)
                        .map_err(|_| Error::Invalid("navigation relation context JSON"))
                })
                .transpose()?;
            if context_json.is_some() {
                direct = direct
                    .checked_add(1)
                    .ok_or(Error::Budget("navigation direct contexts"))?;
            }
            let claim_ref = value
                .get("claim_ref")
                .and_then(Value::as_str)
                .map(str::to_owned);
            if claim_ref.is_some() {
                if claim_ref.as_ref().is_some_and(|value| value.len() > 4096) {
                    return Err(Error::Budget("navigation claim reference bytes"));
                }
                claims = claims
                    .checked_add(1)
                    .ok_or(Error::Budget("navigation claim keys"))?;
            }
            rows.push(DependencyRow {
                edge_id: raw.id.clone(),
                sha: *digest.as_bytes(),
                from_id,
                to_id,
                claim_ref,
                context_json,
            });
        }
        stage.with_connection(WritePhase::Normalized, |db| {
            let tx=db.transaction()?;
            for row in rows {
                tx.execute("INSERT INTO knowledge_navigation_relation_dependencies VALUES (?1,?2,?3,?4,?5,?6)",
                    params![row.edge_id,&row.sha[..],row.from_id,row.to_id,row.claim_ref,row.context_json])?;
            }
            tx.commit()?;Ok(())
        })?;
        match page.next_id {
            Some(next) => after = Some(next),
            None => break,
        }
    }
    if count != prepared.edges || root.finalize().to_hex() != prepared.edge_input_root_sha256 {
        return Err(Error::Invalid("navigation relation complete input root"));
    }
    let dependency_root_sha256=stage.with_connection(WritePhase::Sort, |db| {
        let mut hash=Digest256Hasher::new();hash.update(b"tos-navigation-relation-dependencies-v1\0");
        root_text(&mut hash,&prepared.source_cut);root_text(&mut hash,&prepared.dependency_root_sha256);
        let mut statement=db.prepare("SELECT edge_id,raw_sha256,from_id,to_id,claim_ref,direct_context_json FROM knowledge_navigation_relation_dependencies ORDER BY edge_id")?;
        let mut rows=statement.query([])?;let mut seen=0u64;
        while let Some(row)=rows.next()? {
            let id:String=row.get(0)?;let sha:Vec<u8>=row.get(1)?;
            if sha.len()!=32 {return Err(Error::Invalid("navigation relation dependency digest"));}
            let mut digest=[0u8;32];digest.copy_from_slice(&sha);root_item(&mut hash,&id,&digest);
            for index in 2..4 {let v:String=row.get(index)?;root_text(&mut hash,&v);}
            let claim:Option<String>=row.get(4)?;
            hash.update(&[u8::from(claim.is_some())]);if let Some(v)=claim {root_text(&mut hash,&v);}
            let context:Option<Vec<u8>>=row.get(5)?;
            hash.update(&[u8::from(context.is_some())]);if let Some(v)=context {hash.update(Digest256::of_bytes(&v).as_bytes());}
            seen+=1;
        }
        if seen!=count {return Err(Error::Invalid("navigation relation dependency count"));}
        Ok(hash.finalize().to_hex())
    })?;
    Ok(NavigationRelationDependencyReceipt {
        source_graph: prepared.source_graph.clone(),
        source_cut: prepared.source_cut.clone(),
        edges: count,
        direct_contexts: direct,
        referenced_claim_keys: claims,
        edge_input_root_sha256: prepared.edge_input_root_sha256.clone(),
        dependency_root_sha256,
        final_relation_rows_written: false,
    })
}
/// Bind every exact prepared edge to its two normalized endpoint addresses,
/// optional referenced Claim key, and lossless direct assertion context.
/// This remains private until a global base-node title/Claim join completes.
pub fn prepare_navigation_relation_dependencies(
    stage: &mut KnowledgeStage<'_>,
    prepared: &NavigationPrepareReceipt,
    vocabulary: &QueryVocabulary,
    limits: NavigationRelationLimits,
) -> Result<NavigationRelationDependencyReceipt> {
    let result = prepare_inner(stage, prepared, vocabulary, limits);
    if result.is_err() {
        stage.poison();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge_source_navigation_prepare::{
        NavigationHeaderClaim, NavigationPrepareLimits, prepare_source_navigation,
    };
    use crate::knowledge_stage::{
        ExactInputReceipt, InputCollectionReceipt, InputRow, StageIsolation, StageLimits,
        StageOwner,
    };
    use crate::{Limits, RegisteredSource, SourceBinding};
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    const CONTEXT_DIGEST: &str = "bc72f2d1fbe31f49452c56772497c24de221db2a2c13ba2b3045b54225f54647";
    const EDGE: &str = r#"{"edge_id":"e:claim","from_id":"n:a","to_id":"n:missing","predicate_id":"links","edge_kind":"source_record_link","review_status":"source-recorded","claim_ref":"c1","source_refs":["Z","A","A"],"properties":{"claim_ref":"c2","claim_status":false,"source_claim":{"claim_ref":"c3","qualifiers":[]}}}"#;
    const NODE: &str = r#"{"node_id":"n:a","node_kind":"work","label":"Alpha","source_ref":"ToS/a.json","identity_status":"not_applicable","properties":{}}"#;
    struct Owner;
    impl StageOwner for Owner {
        fn verify_receipt(&self, _: &ExactInputReceipt) -> Result<()> {
            Ok(())
        }
        fn recheck_sealed_cut(&self, _: &ExactInputReceipt) -> Result<()> {
            Ok(())
        }
    }
    struct Quota;
    impl StageIsolation for Quota {
        fn verify(&self, _: &Path, _: StageLimits, _: WritePhase) -> Result<()> {
            Ok(())
        }
    }
    fn vocabulary() -> QueryVocabulary {
        QueryVocabulary {
            descriptor_sha256: "0".repeat(64),
            descriptor_version: 1,
            sources: vec![RegisteredSource {
                source_graph_id: "navigation-fixture".into(),
                owner_ref: "ToS/source-witnesses/AGENTS.md".into(),
                input_role: "source-navigation".into(),
                adapter_profile: PROFILE.into(),
                representative_priority: 0,
            }],
            registered_source_ids: vec!["navigation-fixture".into()],
            extension_adapter_profile: "indexed-node-edge-v1".into(),
            entity_registry_id: "entities".into(),
            relation_registry_id: "relations".into(),
            semantic_primitive_profile: "fixture".into(),
            shared_entity_id_grammars: vec![],
            overview_route_ids: vec![],
        }
    }
    fn input_root(id: &str, payload: &str) -> String {
        let mut hash = Digest256Hasher::new();
        root_item(
            &mut hash,
            id,
            Digest256::of_bytes(payload.as_bytes()).as_bytes(),
        );
        hash.finalize().to_hex()
    }
    fn receipt() -> ExactInputReceipt {
        ExactInputReceipt {
            binding: SourceBinding {
                owner_profile: "fixture-owner".into(),
                source_cut: "sealed-navigation".into(),
                through_commit_seq: 1,
                membership_root: "0".repeat(64),
                index_generation: "gen-1".into(),
                route_map_version: "routes-1".into(),
                reader_abi: "reader-1".into(),
                projection_root_sha256: "1".repeat(64),
                complete: true,
            },
            collections: vec![
                InputCollectionReceipt {
                    source_graph: "navigation-fixture".into(),
                    collection: "nodes".into(),
                    input_role: "source-navigation".into(),
                    adapter_profile: PROFILE.into(),
                    expected_count: 1,
                    expected_root_sha256: input_root("n:a", NODE),
                },
                InputCollectionReceipt {
                    source_graph: "navigation-fixture".into(),
                    collection: "edges".into(),
                    input_role: "source-navigation".into(),
                    adapter_profile: PROFILE.into(),
                    expected_count: 1,
                    expected_root_sha256: input_root("e:claim", EDGE),
                },
            ],
        }
    }
    fn header() -> NavigationHeaderClaim {
        let raw_json=b"{\"schema_version\":\"tos_source_navigation_v1\",\"authority_boundary\":\"fixture detached header\",\"counts\":{\"nodes\":1,\"edges\":1,\"rights\":0}}".to_vec();
        NavigationHeaderClaim {
            expected_sha256: Digest256::of_bytes(&raw_json).to_hex(),
            raw_json,
        }
    }
    fn prep_limits() -> NavigationPrepareLimits {
        NavigationPrepareLimits {
            max_nodes: 4,
            max_edges: 4,
            max_endpoint_refs: 8,
            max_page_rows: 2,
            max_row_bytes: 2048,
            max_header_bytes: 2048,
            max_work_bytes: 65536,
        }
    }
    fn limits() -> NavigationRelationLimits {
        NavigationRelationLimits {
            max_edges: 4,
            max_page_rows: 2,
            max_raw_bytes: 2048,
            max_context_bytes: 4096,
            max_page_bytes: 4096,
            max_work_bytes: 65536,
        }
    }
    fn path() -> PathBuf {
        let tick = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "tos-navigation-relation-{}-{tick}",
            std::process::id()
        ));
        fs::create_dir(&dir).unwrap();
        dir.join("private.sqlite3")
    }
    fn run(tamper: bool) -> Result<NavigationRelationDependencyReceipt> {
        let owner = Owner;
        let quota = Quota;
        let candidate = path();
        let mut stage = KnowledgeStage::create(
            &candidate,
            StageLimits {
                sqlite: Limits::default(),
                max_temp_bytes: 64 * 1024 * 1024,
                max_seek_rows: 2,
                max_seek_bytes: 4096,
            },
            receipt(),
            &owner,
            &quota,
        )?;
        let result = (|| {
            for (collection, id, payload) in [("nodes", "n:a", NODE), ("edges", "e:claim", EDGE)] {
                stage.ingest_input(InputRow {
                    source_graph: "navigation-fixture",
                    collection,
                    id,
                    payload: payload.as_bytes(),
                })?;
            }
            let mut prepared =
                prepare_source_navigation(&mut stage, &vocabulary(), &header(), prep_limits())?;
            if tamper {
                prepared.edge_input_root_sha256 = "0".repeat(64);
            }
            let dependency = prepare_navigation_relation_dependencies(
                &mut stage,
                &prepared,
                &vocabulary(),
                limits(),
            )?;
            let (from,to,claim,context):(String,String,Option<String>,Option<Vec<u8>>)=stage.with_connection(WritePhase::Sort,|db| {
                Ok(db.query_row("SELECT from_id,to_id,claim_ref,direct_context_json FROM knowledge_navigation_relation_dependencies WHERE edge_id='e:claim'",[],
                    |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?)
            })?;
            assert_eq!(
                (from.as_str(), to.as_str(), claim.as_deref()),
                (
                    "navigation-fixture:n:a",
                    "navigation-fixture:n:missing",
                    Some("c1")
                )
            );
            let context: Value = serde_json::from_slice(&context.unwrap()).unwrap();
            assert_eq!(stable_digest(&context)?, CONTEXT_DIGEST);
            let roots = stage.core_roots()?;
            assert_eq!((roots.nodes, roots.relations), (0, 0));
            Ok(dependency)
        })();
        drop(stage);
        fs::remove_dir(candidate.parent().unwrap()).unwrap();
        result
    }
    #[test]
    fn python_oracle_context_and_indexed_dependency_addresses() {
        let source = SourceRow::parse(EDGE.as_bytes(), 2048).unwrap();
        let context = direct_assertion_context(&source, 4096).unwrap().unwrap();
        assert_eq!(stable_digest(&context).unwrap(), CONTEXT_DIGEST);
        assert_eq!(context["source_refs"], json!(["A", "Z"]));
        assert_eq!(context["conflicts"].as_array().unwrap().len(), 2);
        assert_eq!(context["fields"]["claim_ref"]["value"], "c3");
        let prepared = run(false).unwrap();
        assert_eq!(
            (
                prepared.edges,
                prepared.direct_contexts,
                prepared.referenced_claim_keys
            ),
            (1, 1, 1)
        );
        assert!(!prepared.final_relation_rows_written);
    }
    #[test]
    fn missing_trigger_and_incomplete_prepared_cut_refuse_or_remain_absent() {
        let raw = br#"{"subject_ref":"s","predicate":"p","object":null,"properties":{}}"#;
        let source = SourceRow::parse(raw, 2048).unwrap();
        assert!(direct_assertion_context(&source, 4096).unwrap().is_none());
        assert!(run(true).is_err());
        let edge = SourceRow::parse(EDGE.as_bytes(), 2048).unwrap();
        assert!(direct_assertion_context(&edge, 16).is_err());
    }
    #[test]
    fn python_oracle_native_relation_requires_explicit_global_inputs() {
        let descriptor = include_bytes!("../tests/fixtures/query-vocabulary.v1.json");
        let entity =
            include_bytes!("../../../../ToS/doctrine/semantic-interchange/entity-types.v1.json");
        let relation =
            include_bytes!("../../../../ToS/doctrine/semantic-interchange/relation-types.v1.json");
        let document: Value = serde_json::from_slice(descriptor).unwrap();
        let mut adapters = document["sources"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["adapter_profile"].as_str().unwrap().to_owned())
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
        let registry = KnowledgeRegistry::parse(entity, relation).unwrap();
        let normalizer = NavigationRelationNormalizer::new(
            &registry,
            relation,
            &vocabulary,
            descriptor,
            NavigationRelationNormalizeLimits {
                max_raw_bytes: 2048,
                max_output_bytes: 32768,
                max_registry_bytes: 4 * 1024 * 1024,
                max_claim_contexts: 4,
                max_global_input_bytes: 4096,
            },
        )
        .unwrap();
        let mut prepared = NavigationPrepareReceipt {
            source_graph: "source-navigation".into(),
            input_role: "corpus-source-navigation".into(),
            source_cut: "sealed-navigation".into(),
            nodes: 1,
            edges: 1,
            endpoint_refs: 2,
            unresolved_endpoint_refs: 1,
            header_claim_sha256: "0".repeat(64),
            header_claim_rights_count: 0,
            node_input_root_sha256: "0".repeat(64),
            edge_input_root_sha256: input_root("e:claim", EDGE),
            dependency_root_sha256: "1".repeat(64),
            external_dependencies: &[],
            final_graph_rows_written: false,
        };
        let dependency = NavigationRelationDependencyReceipt {
            source_graph: prepared.source_graph.clone(),
            source_cut: prepared.source_cut.clone(),
            edges: 1,
            direct_contexts: 1,
            referenced_claim_keys: 1,
            edge_input_root_sha256: prepared.edge_input_root_sha256.clone(),
            dependency_root_sha256: "2".repeat(64),
            final_relation_rows_written: false,
        };
        let raw = SeekRow {
            id: "e:claim".into(),
            source_graph: "source-navigation".into(),
            source_order: None,
            payload: EDGE.as_bytes().to_vec(),
            payload_sha256: Digest256::of_bytes(EDGE.as_bytes()).to_hex(),
        };
        let left = json!({"default":"Alpha","ru":"Альфа","en":"Alpha"});
        let right = json!({"default":"Missing","ru":"Отсутствует","en":"Missing"});
        let referenced = json!({"schema_version":"tos_assertion_context_v1","binding_role":"referenced-claim",
            "source_record_digest":"39c6ab2c51c9ae96ccb96200d616d45872088b490adbdac03fd35cb75cd51e2c",
            "source_refs":["ToS/source_home.manifest.json"],"fields":{"claim_id":{"value":"c1","source_pointer":"/claim_id"}},
            "conflicts":[],"interpretation":"source-declared-not-semantic-assessment"});
        let roots = ("3".repeat(64), "4".repeat(64));
        let global = NavigationRelationGlobalInputs {
            source_cut: &prepared.source_cut,
            endpoint_title_root_sha256: &roots.0,
            claim_group_root_sha256: &roots.1,
            left_title: &left,
            right_title: &right,
            referenced_claim_contexts: std::slice::from_ref(&referenced),
        };
        let result = normalizer
            .normalize_base(&raw, &prepared, &dependency, global)
            .unwrap();
        assert_eq!(result.value()["relation_type_id"], "tos.relation.unmapped");
        assert_eq!(
            result.value()["display"]["statement"]["ru"],
            "Альфа — неотображённое исходное отношение → Отсутствует."
        );
        assert_eq!(
            result.content_revision,
            "5a6293667c42d81f2e5d96b08156751354b1459c4d5efe6401715efeac9ef4c4"
        );
        assert_eq!(
            stable_digest(result.value()).unwrap(),
            "304ac31fdc696462ba6fce0d096d65c79d6b56ee610baa36badf55891f2244e6"
        );
        prepared.source_cut = "wrong-cut".into();
        let invalid = NavigationRelationGlobalInputs {
            source_cut: "sealed-navigation",
            endpoint_title_root_sha256: &roots.0,
            claim_group_root_sha256: &roots.1,
            left_title: &left,
            right_title: &right,
            referenced_claim_contexts: std::slice::from_ref(&referenced),
        };
        assert!(
            normalizer
                .normalize_base(&raw, &prepared, &dependency, invalid)
                .is_err()
        );
    }
}
