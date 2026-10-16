//! Maintained source-Claim normalization and bounded stage joins.
//!
//! Prepared bibliographic rows remain the authority for associations. This
//! module writes private base rows and Claim semantics, never admits them.
use crate::knowledge_global_titles::{GlobalTitleReceipt, endpoint_title, verify_global_titles};
use crate::knowledge_normalization::{SourceRow, stable_digest, stamp_content_revision};
use crate::knowledge_philosophy_display::{
    ordinary_philosophy_relation_display, source_navigation_node_display,
};
use crate::knowledge_source_claims_prepare::ClaimPrepareReceipt;
use crate::knowledge_source_navigation_node::{
    assertion_context, attributes, epistemic, normalized_time,
};
use crate::knowledge_stage::{KnowledgeStage, NodeRow, RelationRow, SeekRow, WritePhase};
use crate::{Error, KnowledgeRegistry, QueryVocabulary, Result};
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use tos_foundation::{
    CanonicalProfile, Digest256, Digest256Hasher, JsonLimits, canonical_raw_bytes_v1,
};

macro_rules! processing_fingerprint {
    ($handler:literal, [$($source:expr),+ $(,)?] $(,)?) => {{
        static FINGERPRINT: std::sync::OnceLock<tos_foundation::Digest256> =
            std::sync::OnceLock::new();
        *FINGERPRINT.get_or_init(|| {
            crate::processing_cache::rust_source_fingerprint($handler, &[$($source),+])
        })
    }};
}

const PROFILE: &str = "reified-bibliographic-claims-v1";

fn processing_config(normalizer: &ClaimNormalizer<'_>) -> Result<Value> {
    let digest = stable_digest(&json!({
        "source_graph": &normalizer.source_graph,
        "navigation_graph": &normalizer.navigation_graph,
        "entity_ref": &normalizer.entity_ref,
        "relation_ref": &normalizer.relation_ref,
        "dossier_kinds": &normalizer.dossier_kinds,
        "types": &normalizer.types,
        "relations": &normalizer.relations,
        "limits": {
            "max_raw_bytes": normalizer.limits.max_raw_bytes,
            "max_output_bytes": normalizer.limits.max_output_bytes,
            "max_contexts": normalizer.limits.max_contexts,
        },
    }))?;
    Ok(json!({"sha256":digest}))
}

fn processing_source_input(
    run: &crate::processing_cache::ProcessingRunHandle,
    label: &str,
    graph: &str,
    id: &str,
    bytes: &[u8],
) -> Result<crate::processing_cache::ProcessingValue> {
    run.input(
        &format!("{label}:{graph}:{id}"),
        &json!({"sha256":Digest256::of_bytes(bytes).to_hex(),"bytes":bytes.len()}),
    )
}

fn processing_group(
    run: &crate::processing_cache::ProcessingRunHandle,
    stage: &mut KnowledgeStage<'_>,
    prepared: &ClaimPrepareReceipt,
    receipt: &ClaimContextReceipt,
    reference: &str,
    limits: ClaimNormalizeLimits,
) -> Result<crate::processing_cache::ProcessingValue> {
    let id = format!("claim-context-group:{}:{reference}", prepared.source_graph);
    if let Some(result) = run.result(&id) {
        return Ok(result);
    }
    let contexts = claim_contexts(stage, receipt, &prepared.source_graph, reference, limits)?;
    let owners: Vec<(String, Vec<u8>)> = stage.with_connection(WritePhase::Sort, |db| {
        let mut query = db.prepare(
            "SELECT CASE WHEN typeof(owner_native_id)='text' AND length(CAST(owner_native_id AS BLOB))<=4096 THEN owner_native_id ELSE NULL END, CASE WHEN typeof(owner_sha256)='blob' AND length(owner_sha256)=32 THEN owner_sha256 ELSE NULL END FROM knowledge_claim_context_groups WHERE source_graph=?1 AND claim_ref=?2 ORDER BY encounter LIMIT ?3",
        )?;
        let rows = query.query_map(
            params![prepared.source_graph, reference, limits.max_contexts + 1],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    })?;
    if owners.len() > limits.max_contexts || owners.len() != contexts.len() {
        return Err(Error::Budget("Claim context group dependencies"));
    }
    let mut dependencies = Vec::with_capacity(owners.len());
    for (index, (native_id, owner_sha)) in owners.iter().enumerate() {
        if owner_sha.len() != 32 {
            return Err(Error::Invalid("Claim context source digest"));
        }
        let base_id = format!("claim-node:{}:{native_id}", prepared.source_graph);
        let source = if let Some(node) = run.result(&base_id) {
            node
        } else {
            let held = stage.scoped_raw_by_id(&prepared.source_graph, "nodes", native_id)?;
            let raw = held
                .as_ref()
                .ok_or(Error::Invalid("Claim context source absent"))?;
            if Digest256::of_bytes(&raw.payload).as_bytes().as_slice() != owner_sha.as_slice() {
                return Err(Error::Invalid("Claim context source digest"));
            }
            processing_source_input(
                run,
                "claim-context-source",
                &prepared.source_graph,
                native_id,
                &raw.payload,
            )?
        };
        dependencies.push(source);
        let context_input = run.input(
            &format!(
                "claim-context-value:{}:{reference}:{index}",
                prepared.source_graph
            ),
            &json!({"sha256":stable_digest(&contexts[index])?,"encounter":index}),
        )?;
        dependencies.push(context_input);
    }
    let fingerprint = processing_fingerprint!(
        "knowledge_source_claims::ordered_context_group",
        [
            include_bytes!("knowledge_source_claims.rs"),
            include_bytes!("knowledge_source_claims_prepare.rs"),
            include_bytes!("processing_cache.rs"),
        ],
    );
    run.evaluate_task(
        &id,
        "claim-context-group",
        "knowledge_source_claims::ordered_context_group",
        fingerprint,
        &json!({"source_graph":prepared.source_graph,"claim_ref":reference,"max_contexts":limits.max_contexts}),
        &dependencies,
        || Ok(Value::Array(contexts)),
    )
}

fn processing_endpoint_title(
    run: &crate::processing_cache::ProcessingRunHandle,
    node_id: &str,
    title: &Value,
) -> Result<crate::processing_cache::ProcessingValue> {
    let task_id = format!("endpoint-title:{node_id}");
    if let Some(previous) = run.result(&task_id) {
        if previous.value() != title {
            return Err(Error::Invalid("Claim endpoint title producer mismatch"));
        }
        return Ok(previous);
    }
    let (source, derived) = if let Some(node) = run
        .result(&format!("node:{node_id}"))
        .or_else(|| run.result(&format!("claim-node:{node_id}")))
    {
        let derived = node
            .value()
            .pointer("/display/title")
            .filter(|value| value.is_object())
            .ok_or(Error::Invalid("Claim endpoint node title absent"))?;
        if derived != title {
            return Err(Error::Invalid("Claim endpoint title producer mismatch"));
        }
        let derived = derived.clone();
        (node, derived)
    } else {
        (
            run.input(&format!("source-endpoint-title:{node_id}"), title)?,
            title.clone(),
        )
    };
    let fingerprint = processing_fingerprint!(
        "knowledge_global_titles::endpoint_title",
        [
            include_bytes!("knowledge_global_titles.rs"),
            include_bytes!("knowledge_seal.rs"),
            include_bytes!("processing_cache.rs"),
        ],
    );
    run.evaluate_task(
        &task_id,
        "endpoint-title",
        "knowledge_global_titles::endpoint_title",
        fingerprint,
        &json!({"node_id":node_id}),
        &[source],
        || Ok(derived),
    )
}

fn processing_current_node(
    run: &crate::processing_cache::ProcessingRunHandle,
    stable_id: &str,
    input_id: &str,
    value: &Value,
) -> Result<crate::processing_cache::ProcessingValue> {
    if let Some(previous) = run.latest_output("node", stable_id) {
        if previous.value() != value {
            return Err(Error::Invalid(
                "current Claim endpoint differs from producer",
            ));
        }
        return Ok(previous);
    }
    for task_id in [
        format!("node:{stable_id}"),
        format!("claim-node:{stable_id}"),
    ] {
        if let Some(previous) = run.result(&task_id) {
            if previous.value() == value {
                return Ok(previous);
            }
        }
    }
    let bytes = encode(value, 8 * 1024 * 1024)?;
    run.input(
        input_id,
        &json!({"sha256":Digest256::of_bytes(&bytes).to_hex(),"bytes":bytes.len()}),
    )
}

/// Exact Python bibliography materialization adds its graph layer before
/// normalization. Preserve the original member order for readable copies;
/// this is a derived source carrier, not a replacement owner record.
pub fn ordered_claim_node_material(raw: &[u8], max_bytes: usize) -> Result<Vec<u8>> {
    use tos_foundation::{JsonMode, JsonValue, parse_json};
    let owner = SourceRow::parse(raw, max_bytes)?;
    let mut layers = unique(owner.value().get("graph_layers"));
    layers.push("bibliographic-claim".into());
    layers.sort();
    layers.dedup();
    let limits = JsonLimits {
        max_bytes,
        ..JsonLimits::default()
    };
    let mut ordered = parse_json(raw, JsonMode::PublishedStrict, limits)
        .map_err(|e| Error::Source(e.to_string()))?
        .into_root();
    let addition = serde_json::to_vec(&json!({"graph_layers": layers}))
        .map_err(|_| Error::Invalid("Claim layer material"))?;
    let JsonValue::Object(mut addition) = parse_json(&addition, JsonMode::PublishedStrict, limits)
        .map_err(|e| Error::Source(e.to_string()))?
        .into_root()
    else {
        return Err(Error::Invalid("Claim layer material object"));
    };
    let (key, value) = addition
        .pop()
        .ok_or(Error::Invalid("Claim layer material field"))?;
    let JsonValue::Object(fields) = &mut ordered else {
        return Err(Error::Invalid("Claim owner object"));
    };
    if let Some((_, existing)) = fields
        .iter_mut()
        .find(|(key, _)| key.as_str() == Some("graph_layers"))
    {
        *existing = value;
    } else {
        fields.push((key, value));
    }
    let mut bytes = Vec::new();
    crate::knowledge_readable_context::emit_ordered(&ordered, &mut bytes, max_bytes)?;
    Ok(bytes)
}

/// Preserve original object member order while applying the exact bounded
/// bibliographic relation transform already produced by the normalizer.
/// Only these four declared fields and the two role/form fields may change.
pub fn ordered_claim_relation_material(
    raw: &[u8],
    material: &Value,
    max_bytes: usize,
) -> Result<Vec<u8>> {
    use tos_foundation::{JsonMode, JsonValue, parse_json};
    let original = SourceRow::parse(raw, max_bytes)?;
    let original_object = original
        .value()
        .as_object()
        .ok_or(Error::Invalid("Claim relation source object"))?;
    let material_object = material
        .as_object()
        .ok_or(Error::Invalid("Claim relation material object"))?;
    for (key, value) in original_object {
        if !["predicate_id", "source_ref", "graph_layers", "properties"].contains(&key.as_str())
            && material_object.get(key) != Some(value)
        {
            return Err(Error::Invalid("Claim relation undeclared source mutation"));
        }
    }
    if material_object.keys().any(|key| {
        !original_object.contains_key(key)
            && !["predicate_id", "source_ref", "graph_layers", "properties"].contains(&key.as_str())
    }) {
        return Err(Error::Invalid("Claim relation undeclared source addition"));
    }
    let limits = JsonLimits {
        max_bytes,
        ..JsonLimits::default()
    };
    let mut ordered = parse_json(raw, JsonMode::PublishedStrict, limits)
        .map_err(|e| Error::Source(e.to_string()))?
        .into_root();
    let replace = |target: &mut JsonValue, key: &str, value: &Value| -> Result<()> {
        let raw = encode(&json!({key:value}), max_bytes)?;
        let JsonValue::Object(mut addition) = parse_json(&raw, JsonMode::PublishedStrict, limits)
            .map_err(|e| Error::Source(e.to_string()))?
            .into_root()
        else {
            return Err(Error::Invalid("Claim ordered addition"));
        };
        let (name, value) = addition
            .pop()
            .ok_or(Error::Invalid("Claim ordered addition field"))?;
        let JsonValue::Object(fields) = target else {
            return Err(Error::Invalid("Claim ordered source object"));
        };
        if let Some((_, old)) = fields
            .iter_mut()
            .find(|(name, _)| name.as_str() == Some(key))
        {
            *old = value;
        } else {
            fields.push((name, value));
        }
        Ok(())
    };
    for key in ["predicate_id", "source_ref", "graph_layers"] {
        replace(
            &mut ordered,
            key,
            material
                .get(key)
                .ok_or(Error::Invalid("Claim material transform field"))?,
        )?;
    }
    let props = material
        .get("properties")
        .and_then(Value::as_object)
        .ok_or(Error::Invalid("Claim material properties"))?;
    let source_props = original
        .value()
        .get("properties")
        .and_then(Value::as_object);
    let (role, forms) = match material.get("predicate_id").and_then(Value::as_str) {
        Some("has_normalized_place") => ("spatial_roles", "spatial_literal_forms"),
        Some("has_normalized_agent") => ("agent_roles", "agent_literal_forms"),
        _ => ("", ""),
    };
    if props
        .iter()
        .any(|(k, v)| k != role && k != forms && source_props.and_then(|p| p.get(k)) != Some(v))
        || source_props.is_some_and(|p| p.keys().any(|k| !props.contains_key(k)))
    {
        return Err(Error::Invalid("Claim undeclared property transform"));
    }
    if ordered.object_get("properties").is_none()
        || !ordered
            .object_get("properties")
            .is_some_and(|v| matches!(v, JsonValue::Object(_)))
    {
        replace(&mut ordered, "properties", &json!({}))?;
    }
    let JsonValue::Object(fields) = &mut ordered else {
        return Err(Error::Invalid("Claim ordered object"));
    };
    let target = fields
        .iter_mut()
        .find(|(key, _)| key.as_str() == Some("properties"))
        .map(|(_, v)| v)
        .ok_or(Error::Invalid("Claim ordered properties"))?;
    for key in [role, forms] {
        if !key.is_empty() {
            if let Some(value) = props.get(key) {
                replace(target, key, value)?;
            }
        }
    }
    let mut out = Vec::new();
    crate::knowledge_readable_context::emit_ordered(&ordered, &mut out, max_bytes)?;
    if SourceRow::parse(&out, max_bytes)?.value() != material {
        return Err(Error::Invalid("Claim ordered transform equality"));
    }
    Ok(out)
}

pub(crate) fn claim_relation_material_witness(
    stage: &mut KnowledgeStage<'_>,
    graph: &str,
    id: &str,
    max_bytes: usize,
) -> Result<Vec<u8>> {
    let (native,raw_sha,material,material_sha):(String,Vec<u8>,Vec<u8>,Vec<u8>)=stage.with_connection(WritePhase::Finalize,|db| {
        db.query_row("SELECT native_id,raw_sha256,CASE WHEN material_len=length(material) AND length(material)<=?3 THEN material ELSE NULL END,material_sha256 FROM knowledge_claim_relation_material WHERE source_graph=?1 AND id=?2",params![graph,id,max_bytes as i64],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).map_err(Error::from)
    })?;
    let raw_owner = stage.scoped_raw_by_id(graph, "edges", &native)?;
    let raw = raw_owner
        .as_ref()
        .ok_or(Error::Invalid("Claim material original absent"))?;
    if raw_sha != Digest256::of_bytes(&raw.payload).as_bytes()
        || material_sha != Digest256::of_bytes(&material).as_bytes()
    {
        return Err(Error::Invalid("Claim material original/root digest"));
    }
    Ok(material)
}

#[derive(Clone, Copy, Debug)]
pub struct ClaimNormalizeLimits {
    pub max_raw_bytes: usize,
    pub max_output_bytes: usize,
    pub max_page_rows: usize,
    pub max_contexts: usize,
    pub max_work_bytes: u64,
}
impl ClaimNormalizeLimits {
    pub(crate) fn validate(self) -> Result<()> {
        if self.max_raw_bytes == 0
            || self.max_raw_bytes > 8 * 1024 * 1024
            || self.max_output_bytes == 0
            || self.max_output_bytes > 8 * 1024 * 1024
            || self.max_page_rows == 0
            || self.max_page_rows > 1024
            || self.max_contexts == 0
            || self.max_contexts > 4096
            || self.max_work_bytes == 0
        {
            return Err(Error::Budget("Claim normalization limits"));
        }
        Ok(())
    }
}
fn required<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    text(value.get(key))
        .filter(|s| s.len() <= 4096 && !s.contains('\0'))
        .ok_or(Error::Invalid("Claim carrier required field"))
}
fn text(value: Option<&Value>) -> Option<&str> {
    value?.as_str().map(str::trim).filter(|s| !s.is_empty())
}
fn strings(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().filter(|s| !s.is_empty()))
        .map(str::to_owned)
        .collect()
}
fn unique(value: Option<&Value>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    strings(value)
        .into_iter()
        .filter(|s| seen.insert(s.clone()))
        .collect()
}
fn objects(value: Option<&Value>) -> impl Iterator<Item = &Value> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|v| v.is_object())
}
fn charge(work: &mut u64, bytes: usize, cap: u64) -> Result<()> {
    *work = work
        .checked_add(bytes as u64)
        .filter(|n| *n <= cap)
        .ok_or(Error::Budget("Claim normalization work"))?;
    Ok(())
}
fn root_item(hash: &mut Digest256Hasher, id: &str, digest: &[u8]) {
    hash.update(&(id.len() as u64).to_be_bytes());
    hash.update(id.as_bytes());
    hash.update(digest);
}
fn encode(value: &Value, cap: usize) -> Result<Vec<u8>> {
    struct Writer {
        bytes: Vec<u8>,
        cap: usize,
    }
    impl std::io::Write for Writer {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if bytes.len() > self.cap.saturating_sub(self.bytes.len()) {
                return Err(std::io::Error::other("Claim output byte ceiling"));
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut writer = Writer {
        bytes: Vec::new(),
        cap,
    };
    serde_json::to_writer(&mut writer, value).map_err(|_| Error::Budget("Claim output bytes"))?;
    Ok(writer.bytes)
}

pub struct ClaimNormalizer<'a> {
    registry: &'a KnowledgeRegistry,
    source_graph: String,
    navigation_graph: String,
    dossier_kinds: BTreeSet<String>,
    entity_ref: String,
    relation_ref: String,
    types: BTreeMap<String, Value>,
    relations: BTreeMap<String, Value>,
    limits: ClaimNormalizeLimits,
    processing_run: Option<crate::processing_cache::ProcessingRunHandle>,
}
impl<'a> ClaimNormalizer<'a> {
    pub fn new(
        registry: &'a KnowledgeRegistry,
        entity_bytes: &[u8],
        relation_bytes: &[u8],
        vocabulary: &QueryVocabulary,
        descriptor_bytes: &[u8],
        limits: ClaimNormalizeLimits,
    ) -> Result<Self> {
        limits.validate()?;
        vocabulary.verify_authored_bytes(descriptor_bytes)?;
        if registry.entity_registry_id != vocabulary.entity_registry_id
            || registry.relation_registry_id != vocabulary.relation_registry_id
        {
            return Err(Error::Invalid("Claim descriptor registry identity"));
        }
        let selected = vocabulary
            .sources
            .iter()
            .filter(|s| s.adapter_profile == PROFILE)
            .collect::<Vec<_>>();
        if selected.len() != 1 {
            return Err(Error::Invalid("Claim selected profile"));
        }
        if Digest256::of_bytes(entity_bytes).to_hex() != registry.entity_sha256
            || Digest256::of_bytes(relation_bytes).to_hex() != registry.relation_sha256
        {
            return Err(Error::Invalid("Claim selected registry bytes"));
        }
        let entity = SourceRow::parse(entity_bytes, 4 * 1024 * 1024)?;
        let relation = SourceRow::parse(relation_bytes, 4 * 1024 * 1024)?;
        let descriptor = SourceRow::parse(descriptor_bytes, 1024 * 1024)?;
        let refs = &descriptor.value()["semantic_registry_refs"];
        Ok(Self {
            registry,
            source_graph: selected[0].source_graph_id.clone(),
            navigation_graph: required(&descriptor.value()["identity"], "source_dossier_graph_id")?
                .into(),
            dossier_kinds: strings(descriptor.value()["identity"].get("source_dossier_kinds"))
                .into_iter()
                .collect(),
            entity_ref: required(&refs["entity"], "source_ref")?.into(),
            relation_ref: required(&refs["relation"], "source_ref")?.into(),
            types: objects(entity.value().get("types"))
                .map(|e| Ok((required(e, "type_id")?.to_owned(), e.clone())))
                .collect::<Result<_>>()?,
            relations: objects(relation.value().get("relations"))
                .map(|e| Ok((required(e, "relation_type_id")?.to_owned(), e.clone())))
                .collect::<Result<_>>()?,
            limits,
            processing_run: None,
        })
    }
    pub fn with_processing_run(
        mut self,
        run: crate::processing_cache::ProcessingRunHandle,
    ) -> Self {
        self.processing_run = Some(run);
        self
    }
    fn verify(&self, raw: &SeekRow, prepared: &ClaimPrepareReceipt) -> Result<()> {
        if raw.source_graph != self.source_graph
            || prepared.source_graph != self.source_graph
            || prepared.source_cut.is_empty()
            || prepared.final_graph_rows_written
            || raw.source_order.is_some()
            || Digest256::from_hex(&prepared.dependency_root_sha256).is_err()
            || raw.payload.len() > self.limits.max_raw_bytes
            || Digest256::of_bytes(&raw.payload).to_hex() != raw.payload_sha256
        {
            return Err(Error::Invalid("Claim prepared raw binding"));
        }
        Ok(())
    }
    fn verify_supplied(&self, raw: &SeekRow) -> Result<()> {
        if raw.source_graph != self.source_graph
            || raw.source_order.is_some()
            || raw.id.is_empty()
            || raw.id.len() > 4096
            || raw.payload.len() > self.limits.max_raw_bytes
            || Digest256::of_bytes(&raw.payload).to_hex() != raw.payload_sha256
        {
            return Err(Error::Invalid("Claim supplied raw binding"));
        }
        Ok(())
    }
    fn profile(&self, predicate: &str) -> Result<Value> {
        let resolved = self
            .registry
            .relation(&self.source_graph, predicate, "claim-predicate");
        Ok(self
            .relations
            .get(resolved.type_id)
            .ok_or(Error::Invalid("Claim relation policy"))?
            .get("source_claim_profile")
            .cloned()
            .unwrap_or_else(|| json!({})))
    }
    fn claim_policy_value(&self, predicate: &str) -> Result<Value> {
        let resolved = self
            .registry
            .relation(&self.source_graph, predicate, "claim-predicate");
        Ok(json!({
            "source_graph":self.source_graph,
            "source_predicate_id":predicate,
            "relation_type_id":resolved.type_id,
            "predicate_mapping_status":if resolved.mapped {"mapped"} else {"unmapped"},
            "registry_ref":self.relation_ref,
            "registry_sha256":self.registry.relation_sha256,
            "source_claim_profile":self.profile(predicate)?,
        }))
    }
    fn ancestors(&self, id: &str) -> Result<Vec<String>> {
        let mut seen = BTreeSet::new();
        let mut stack = vec![id.to_owned()];
        while let Some(id) = stack.pop() {
            if !seen.insert(id.clone()) {
                continue;
            }
            let entry = self
                .types
                .get(&id)
                .ok_or(Error::Invalid("Claim type ancestor"))?;
            stack.extend(strings(entry.get("parent_type_ids")));
            if seen.len() > 4096 {
                return Err(Error::Budget("Claim type ancestors"));
            }
        }
        Ok(seen.into_iter().collect())
    }
    /// Normalize one explicitly supplied carrier. This verifies its bytes and
    /// selected owner, but does not prove cut membership or closure admission.
    pub fn normalize_supplied_node(
        &self,
        raw: &SeekRow,
        predicate: Option<&str>,
        dossier: Option<&str>,
    ) -> Result<Value> {
        self.normalize_supplied_node_with_source(raw, predicate, dossier)
            .map(|(value, _)| value)
    }
    fn normalize_supplied_node_with_source(
        &self,
        raw: &SeekRow,
        predicate: Option<&str>,
        dossier: Option<&str>,
    ) -> Result<(Value, Vec<u8>)> {
        self.verify_supplied(raw)?;
        if dossier.is_some_and(|id| id.is_empty() || id.len() > 4096) {
            return Err(Error::Invalid("Claim supplied dossier identifier"));
        }
        let original = SourceRow::parse(&raw.payload, self.limits.max_raw_bytes)?;
        let identity_matches = required(original.value(), "node_id")? == raw.id;
        if !identity_matches {
            return Err(Error::Invalid("Claim node ID"));
        }
        let mut item = original.value().clone();
        let mut layers = unique(item.get("graph_layers"));
        layers.push("bibliographic-claim".into());
        layers.sort();
        layers.dedup();
        item["graph_layers"] = json!(layers);
        let material = encode(&item, self.limits.max_raw_bytes)?;
        let source = SourceRow::parse(&material, self.limits.max_raw_bytes)?;
        let props = &item["properties"];
        let raw_kind = required(&item, "node_kind")?;
        let profile = predicate
            .map(|p| self.profile(p))
            .transpose()?
            .unwrap_or_else(|| json!({}));
        let value = props.get("value");
        let kind = match raw_kind {
            "identity" => text(props.get("identity_kind")).unwrap_or("identity"),
            "provenance_event" => "provenance-event",
            "literal" => match text(profile.get("reader")) {
                Some("historical-temporal-v1" | "document-catalogue-temporal-v1") => {
                    "temporal-assertion"
                }
                Some(
                    "structured-value-v1"
                    | "structured-reference-value-v1"
                    | "identity-transition-v1"
                    | "identity-transition-v2",
                ) => required(&profile, "value_kind")?,
                _ if predicate == Some("provision_activity")
                    || text(value.and_then(|v| v.get("provision_kind"))).is_some() =>
                {
                    "provision-activity"
                }
                _ if matches!(
                    predicate,
                    Some(
                        "historical_dating"
                            | "author_received_finished_copies_on"
                            | "first_publication_chronology"
                            | "official_publication_on"
                            | "printing_completed_in"
                            | "printing_completed_on"
                            | "public_sale_released_on"
                            | "title_page_year"
                    )
                ) =>
                {
                    "temporal-assertion"
                }
                _ if value.is_some_and(|v| {
                    v.get("interval").is_some_and(Value::is_object)
                        || v.get("temporal").is_some_and(Value::is_object)
                        || text(v.get("date")).is_some()
                }) =>
                {
                    "temporal-assertion"
                }
                _ => "literal",
            },
            other => other,
        };
        let resolved = self.registry.entity(&self.source_graph, kind);
        let entry = self
            .types
            .get(resolved.type_id)
            .ok_or(Error::Invalid("Claim entity type"))?;
        let labels = objects(entry.get("source_mappings"))
            .find(|m| {
                text(m.get("source_graph")) == Some(&self.source_graph)
                    && text(m.get("source_kind_id")) == Some(kind)
                    && m.get("labels").is_some_and(|v| {
                        v.is_object() && v.as_object().is_some_and(|o| !o.is_empty())
                    })
            })
            .and_then(|m| m.get("labels"))
            .or_else(|| entry.get("labels"));
        let display =
            source_navigation_node_display(&source, kind, labels, text(entry.get("object_role")))?;
        let attrs = attributes(&item)?;
        let refs = source.source_refs(&[]);
        let mut semantics = json!({"type_ancestors":self.ancestors(resolved.type_id)?});
        if let Some(language) = item.get("multilingual").and_then(Value::as_object) {
            semantics["language_context"] = Value::Object(
                language
                    .iter()
                    .filter(|(k, _)| k.as_str() != "label")
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
            );
        }
        if let Some(context) = assertion_context(&item, &refs)? {
            semantics["assertion_contexts"] = json!([context]);
        }
        if matches!(
            kind,
            "literal" | "temporal-assertion" | "provision-activity"
        ) {
            if let Some(time) = normalized_time(value, "properties.value")? {
                semantics["time"] = time;
            }
            let places = objects(value.and_then(|v| v.get("places")))
                .cloned()
                .collect::<Vec<_>>();
            if !places.is_empty() {
                semantics["space"] = json!({"kind":"claim-scoped-place-mentions","mentions":places,"normalization_status":"source-declared"});
            }
        }
        if kind == "place" {
            semantics["space"] = json!({"kind":"place-identity","place_id":text(props.get("identity_ref")),"identity_status":props.get("identity_status"),"normalization_status":"source-declared"});
        }
        if kind == "claim" {
            semantics["claim"] = json!({"claim_id":text(props.get("claim_ref")),"source_predicate_id":text(props.get("predicate")),"review_status":props.get("review_status"),"epistemic_status":props.get("epistemic_status"),"claim_version":props.get("claim_version")});
        }
        let id = format!("{}:{}", self.source_graph, raw.id);
        let own = match raw_kind {
            "identity" => Some("identity_ref"),
            "claim" => Some("claim_ref"),
            "provenance_event" => Some("event_ref"),
            "review" => Some("review_ref"),
            _ => None,
        };
        let entity = [
            own.and_then(|k| props.get(k)),
            props.get("record_id"),
            item.get("record_id"),
            item.get("node_id"),
        ]
        .into_iter()
        .find_map(|v| text(v).filter(|s| s.starts_with("tos.")))
        .unwrap_or(&id);
        let mut output = json!({"id":id,"entity_id":entity,"native_id":raw.id,"source_graph":self.source_graph,
            "kind_id":kind,"type_id":resolved.type_id,"type_mapping":{"status":if resolved.mapped{"mapped"}else{"unmapped"},"source_kind_id":kind,"registry_ref":self.entity_ref},
            "display":display,"epistemic":epistemic(&item),"graph_layers":layers,"view_ids":unique(item.get("view_ids")),
            "source_refs":refs,"attributes":attrs,"semantics":semantics,"source_record":source.source_record(&attrs)?});
        if let Some(dossier) = dossier {
            output["source_dossier_ref"] = json!(dossier);
        }
        stamp_content_revision(&mut output, self.limits.max_output_bytes)?;
        Ok((output, material))
    }
    pub fn finalize_claim(
        &self,
        claim: &Value,
        subject: &Value,
        object: &Value,
        trace: &Value,
    ) -> Result<Value> {
        let reference = required(trace, "claim_ref")?;
        for (node, key) in [
            (claim, "claim_node_id"),
            (subject, "subject_node_id"),
            (object, "object_node_id"),
        ] {
            if required(node, "id")? != format!("{}:{}", self.source_graph, required(trace, key)?) {
                return Err(Error::Invalid("Claim finalized endpoint binding"));
            }
        }
        let predicate = required(trace, "predicate")?;
        let policy = self
            .registry
            .relation(&self.source_graph, predicate, "claim-predicate");
        let profile = self.profile(predicate)?;
        let mut contract = claim
            .pointer("/semantics/claim")
            .and_then(Value::as_object)
            .cloned()
            .ok_or(Error::Invalid("Claim base semantics absent"))?;
        for (key, value) in [
            ("claim_id", json!(reference)),
            ("source_predicate_id", json!(predicate)),
            ("relation_type_id", json!(policy.type_id)),
            (
                "predicate_mapping_status",
                json!(if policy.mapped { "mapped" } else { "unmapped" }),
            ),
            ("subject_node_id", subject["id"].clone()),
            ("subject_entity_id", subject["entity_id"].clone()),
            ("object_node_id", object["id"].clone()),
            ("object_entity_id", object["entity_id"].clone()),
            (
                "normalized_identity_node_ids",
                json!(
                    strings(trace.get("normalized_identity_node_ids"))
                        .iter()
                        .map(|s| format!("{}:{s}", self.source_graph))
                        .collect::<Vec<_>>()
                ),
            ),
            (
                "evidence_node_ids",
                json!(
                    strings(trace.get("evidence_node_ids"))
                        .iter()
                        .map(|s| format!("{}:{s}", self.source_graph))
                        .collect::<Vec<_>>()
                ),
            ),
            ("review_status", trace["review_status"].clone()),
            ("epistemic_status", trace["epistemic_status"].clone()),
        ] {
            contract.insert(key.into(), value);
        }
        if let Some(members) = trace
            .get("value_member_node_ids")
            .filter(|v| v.as_array().is_some_and(|a| !a.is_empty()))
        {
            contract.insert(
                "value_member_node_ids".into(),
                json!(
                    strings(Some(members))
                        .iter()
                        .map(|s| format!("{}:{s}", self.source_graph))
                        .collect::<Vec<_>>()
                ),
            );
        }
        if text(profile.get("reader")) == Some("document-catalogue-temporal-v1") {
            let source_bytes = encode(
                &claim["attributes"]["source_claim"],
                self.limits.max_output_bytes,
            )?;
            let raw = canonical_raw_bytes_v1(
                &source_bytes,
                CanonicalProfile::SourceRecordDigestV1,
                JsonLimits::new(self.limits.max_output_bytes, 96, 1_000_000, 4096)
                    .map_err(|_| Error::Budget("Claim canonical limits"))?,
            )
            .map_err(|error| Error::Source(error.to_string()))?;
            contract.insert("source_claim_profile".into(), profile);
            contract.insert(
                "source_canonical_json".into(),
                if raw.len() <= 262144 {
                    json!(
                        String::from_utf8(raw)
                            .map_err(|_| Error::Invalid("Claim canonical UTF8"))?
                    )
                } else {
                    Value::Null
                },
            );
        }
        let mut result = claim.clone();
        result["semantics"]["claim"] = Value::Object(contract);
        result["attributes"]["claim_trace"] = trace.clone();
        stamp_content_revision(&mut result, self.limits.max_output_bytes)?;
        Ok(result)
    }
    /// Exact bibliographic enrichment and relation normalization. Endpoint
    /// titles and referenced contexts must come from the complete stage joins.
    pub fn normalize_base_relation(
        &self,
        raw: &SeekRow,
        prepared: &ClaimPrepareReceipt,
        object: &Value,
        target: &Value,
        left_title: &Value,
        right_title: &Value,
        contexts: &[Value],
    ) -> Result<Value> {
        self.verify(raw, prepared)?;
        self.normalize_supplied_relation(
            raw,
            object,
            target,
            left_title,
            right_title,
            contexts,
            None,
        )
    }
    /// Pure supplied-carrier counterpart of the stage-bound relation kernel.
    /// Endpoint and context completeness remain with the selecting caller.
    pub fn normalize_supplied_relation(
        &self,
        raw: &SeekRow,
        object: &Value,
        target: &Value,
        left_title: &Value,
        right_title: &Value,
        contexts: &[Value],
        identity_id: Option<&str>,
    ) -> Result<Value> {
        self.verify_supplied(raw)?;
        if identity_id.is_some_and(|id| id.is_empty() || id.len() > 4096) {
            return Err(Error::Invalid("Claim supplied relation identity"));
        }
        let mut supplied_bytes = 0u64;
        for value in [object, target, left_title, right_title] {
            let bytes = encode(value, self.limits.max_output_bytes)?;
            supplied_bytes = supplied_bytes
                .checked_add(bytes.len() as u64)
                .ok_or(Error::Budget("Claim supplied endpoint bytes"))?;
        }
        if contexts.len() > self.limits.max_contexts {
            return Err(Error::Budget("Claim supplied context count"));
        }
        for context in contexts {
            let bytes = encode(context, self.limits.max_output_bytes)?;
            supplied_bytes = supplied_bytes
                .checked_add(bytes.len() as u64)
                .ok_or(Error::Budget("Claim supplied context bytes"))?;
        }
        if supplied_bytes > self.limits.max_work_bytes {
            return Err(Error::Budget("Claim supplied dependency bytes"));
        }
        let source = SourceRow::parse(&raw.payload, self.limits.max_raw_bytes)?;
        if required(source.value(), "edge_id")? != raw.id
            || contexts.len() > self.limits.max_contexts
        {
            return Err(Error::Invalid("Claim relation carrier"));
        }
        let mut item = source.value().clone();
        let predicate = text(item.get("edge_kind"))
            .unwrap_or("related_to")
            .to_owned();
        item["predicate_id"] = json!(predicate);
        item["source_ref"] = json!(
            text(item.get("source_claim_file_ref"))
                .unwrap_or("ToS/source-witnesses/catalog/claims.jsonl")
        );
        item["graph_layers"] = json!(["bibliographic-claim"]);
        let mut props = item
            .get("properties")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let value = object.pointer("/properties/value");
        let identity = text(target.pointer("/properties/identity_ref"));
        if matches!(
            predicate.as_str(),
            "has_normalized_place" | "has_normalized_agent"
        ) {
            let (collection, reference, role_key, forms_key) =
                if predicate == "has_normalized_place" {
                    (
                        "places",
                        "normalized_place_ref",
                        "spatial_roles",
                        "spatial_literal_forms",
                    )
                } else {
                    (
                        "agents",
                        "normalized_agent_ref",
                        "agent_roles",
                        "agent_literal_forms",
                    )
                };
            let matches = objects(value.and_then(|v| v.get(collection)))
                .filter(|v| text(v.get(reference)) == identity)
                .collect::<Vec<_>>();
            let mut roles = if predicate == "has_normalized_place" {
                strings(props.get(role_key))
            } else {
                Vec::new()
            };
            let mut forms = if predicate == "has_normalized_place" {
                strings(props.get(forms_key))
            } else {
                Vec::new()
            };
            roles.extend(
                matches
                    .iter()
                    .filter_map(|m| text(m.get("role")))
                    .map(str::to_owned),
            );
            forms.extend(
                matches
                    .iter()
                    .filter_map(|m| text(m.get("literal_form")))
                    .map(str::to_owned),
            );
            roles.sort();
            roles.dedup();
            forms.sort();
            forms.dedup();
            props.insert(role_key.into(), json!(roles));
            props.insert(forms_key.into(), json!(forms));
        }
        item["properties"] = Value::Object(props.clone());
        let material = encode(&item, self.limits.max_raw_bytes)?;
        let source = SourceRow::parse(&material, self.limits.max_raw_bytes)?;
        let resolved = self
            .registry
            .relation(&self.source_graph, &predicate, "edge");
        let mut effective = self
            .relations
            .get(resolved.type_id)
            .cloned()
            .ok_or(Error::Invalid("Claim edge relation type"))?;
        let mapping = objects(effective.get("source_mappings"))
            .find(|m| {
                text(m.get("source_graph")) == Some(&self.source_graph)
                    && text(m.get("source_predicate_id")) == Some(&predicate)
                    && text(m.get("scope")) == Some("edge")
                    && m.get("labels")
                        .is_some_and(|v| v.as_object().is_some_and(|o| !o.is_empty()))
            })
            .cloned();
        if let Some(mapping) = mapping {
            effective["labels"] = mapping["labels"].clone();
            effective["definition"] = mapping["definition"].clone();
            effective["source_mappings"] = json!([mapping]);
        }
        let refs = source.source_refs(&[]);
        let mut bound = contexts.to_vec();
        let claim_ref = text(item.get("claim_ref"));
        for context in &bound {
            let fields = &context["fields"];
            if text(context.get("binding_role")) != Some("referenced-claim")
                || fields
                    .get("claim_id")
                    .or_else(|| fields.get("claim_ref"))
                    .and_then(|v| text(v.get("value")))
                    != claim_ref
            {
                return Err(Error::Invalid("Claim edge referenced context key"));
            }
        }
        if let Some(context) = assertion_context(&item, &refs)? {
            bound.insert(0, context);
        }
        let mut semantics = json!({});
        if !bound.is_empty() {
            semantics["assertion_contexts"] = json!(bound);
        }
        if resolved.type_id == "tos.relation.has-normalized-place" {
            semantics["space"] = json!({"roles":props.get("spatial_roles").cloned().unwrap_or_else(||json!([])),"literal_forms":props.get("spatial_literal_forms").cloned().unwrap_or_else(||json!([])),"normalization_status":"source-declared"});
        }
        if resolved.type_id == "tos.relation.has-normalized-agent" {
            semantics["responsibility"] = json!({"roles":props.get("agent_roles").cloned().unwrap_or_else(||json!([])),"literal_forms":props.get("agent_literal_forms").cloned().unwrap_or_else(||json!([])),"normalization_status":"source-declared"});
        }
        let mut attrs = props;
        for (key, value) in item
            .as_object()
            .ok_or(Error::Invalid("Claim relation material"))?
        {
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
        let display = ordinary_philosophy_relation_display(
            &source,
            &predicate,
            left_title,
            right_title,
            &effective,
        )?;
        let from_graph = text(item.get("from_source_graph")).unwrap_or(&self.source_graph);
        let to_graph = text(item.get("to_source_graph")).unwrap_or(&self.source_graph);
        let mut output = json!({"id":format!("{}:{}",self.source_graph,identity_id.unwrap_or(&raw.id)),"native_id":raw.id,"source_graph":self.source_graph,
            "from_id":format!("{}:{}",from_graph,required(&item,"from_id")?),"to_id":format!("{}:{}",to_graph,required(&item,"to_id")?),
            "predicate_id":predicate,"relation_type_id":resolved.type_id,"predicate_mapping":{"status":if resolved.mapped{"mapped"}else{"unmapped"},"source_predicate_id":predicate,"registry_ref":self.relation_ref},
            "display":display,"epistemic":epistemic(&item),"graph_layers":["bibliographic-claim"],"view_ids":unique(item.get("view_ids")),
            "source_refs":refs,"attributes":attrs,"semantics":semantics,"source_record":source.source_record(&attrs)?});
        stamp_content_revision(&mut output, self.limits.max_output_bytes)?;
        Ok(output)
    }
}

struct ClaimNode<'a> {
    value: Value,
    _hold: Option<crate::d1_public_capture::CreationStateHold<'a, 'a>>,
}
impl std::ops::Deref for ClaimNode<'_> {
    type Target = Value;
    fn deref(&self) -> &Value {
        &self.value
    }
}
impl std::ops::DerefMut for ClaimNode<'_> {
    fn deref_mut(&mut self) -> &mut Value {
        &mut self.value
    }
}
fn node<'a>(stage: &mut KnowledgeStage<'a>, id: &str, cap: usize) -> Result<ClaimNode<'a>> {
    if let Some(state) = stage.owned_creation_state() {
        return stage
            .with_node_payload_owned(id, cap, |_, bytes| {
                let (value, hold) =
                    state.serde_scoped_with_limits(bytes, SourceRow::json_limits(cap)?)?;
                if required(&value, "id")? != id {
                    return Err(Error::Invalid("Claim normalized endpoint ID"));
                }
                Ok(ClaimNode {
                    value,
                    _hold: Some(hold),
                })
            })?
            .ok_or(Error::Invalid("Claim normalized endpoint absent/oversize"));
    }
    let (bytes,sha):(Vec<u8>,Vec<u8>)=stage.with_connection(WritePhase::Sort,|db|{
        db.query_row("SELECT CASE WHEN typeof(payload)='blob' AND payload_len=length(payload) AND length(payload)<=?2 THEN payload ELSE NULL END,CASE WHEN typeof(payload_sha256)='blob' AND length(payload_sha256)=32 THEN payload_sha256 ELSE NULL END FROM knowledge_nodes WHERE id=?1",params![id,cap],|r|Ok((r.get(0)?,r.get(1)?)))
            .optional()?.ok_or(Error::Invalid("Claim normalized endpoint absent/oversize"))})?;
    if Digest256::of_bytes(&bytes).as_bytes().as_slice() != sha {
        return Err(Error::Invalid("Claim normalized endpoint SHA"));
    }
    let source = SourceRow::parse(&bytes, cap)?;
    if required(source.value(), "id")? != id {
        return Err(Error::Invalid("Claim normalized endpoint ID"));
    }
    Ok(ClaimNode {
        value: source.value().clone(),
        _hold: None,
    })
}

fn update_node(stage: &mut KnowledgeStage<'_>, value: &Value, cap: usize) -> Result<()> {
    if let Some(state) = stage.owned_creation_state() {
        let id = required(value, "id")?;
        return stage
            .with_node_payload_source_owned(id, cap, |stage, previous, source| {
                let digest = Digest256::of_bytes(previous);
                state.with_json_encoded(value, cap, |bytes| {
                    stage.replace_node_logical_payload_if_current(id, bytes, source, Some(digest))
                })
            })?
            .ok_or(Error::Invalid("Claim update absent"));
    }
    let bytes = encode(value, cap)?;
    let sha = Digest256::of_bytes(&bytes);
    stage.charge_materialized(1, bytes.len() as u64)?;
    stage.with_connection(WritePhase::Normalized, |db| {
        if db.execute(
            "UPDATE knowledge_nodes SET payload_len=?1,payload_sha256=?2,payload=?3 WHERE id=?4",
            params![
                bytes.len(),
                sha.as_bytes().as_slice(),
                bytes,
                required(value, "id")?
            ],
        )? != 1
        {
            return Err(Error::Invalid("Claim update absent"));
        }
        Ok(())
    })
}
fn verify_stage(stage: &KnowledgeStage<'_>, prepared: &ClaimPrepareReceipt) -> Result<()> {
    if prepared.final_graph_rows_written
        || stage.exact_receipt()?.binding.source_cut != prepared.source_cut
        || Digest256::from_hex(&prepared.dependency_root_sha256).is_err()
    {
        return Err(Error::Invalid("Claim stage prepared cut"));
    }
    for (collection, count, root) in [
        ("nodes", prepared.nodes, &prepared.node_input_root_sha256),
        ("edges", prepared.edges, &prepared.edge_input_root_sha256),
        (
            "claim_traces",
            prepared.claim_traces,
            &prepared.trace_input_root_sha256,
        ),
    ] {
        let entries = stage
            .exact_receipt()?
            .collections
            .iter()
            .filter(|r| r.source_graph == prepared.source_graph && r.collection == collection)
            .collect::<Vec<_>>();
        if entries.len() != 1
            || entries[0].expected_count != count
            || entries[0].expected_root_sha256 != *root
            || entries[0].adapter_profile != PROFILE
            || entries[0].input_role != prepared.input_role
        {
            return Err(Error::Invalid("Claim exact prepared collections"));
        }
    }
    Ok(())
}

fn verify_prepared_dependencies(
    stage: &mut KnowledgeStage<'_>,
    prepared: &ClaimPrepareReceipt,
    limits: ClaimNormalizeLimits,
) -> Result<()> {
    verify_stage(stage, prepared)?;
    let mut after = None;
    let mut work = 0;
    let mut traces = 0;
    loop {
        let page = stage.scoped_scan_input(
            &prepared.source_graph,
            "claim_traces",
            after.as_deref(),
            limits.max_page_rows,
        )?;
        for raw in &page.rows {
            charge(&mut work, raw.payload.len(), limits.max_work_bytes)?;
            let source = SourceRow::parse(&raw.payload, limits.max_raw_bytes)?;
            let t = source.value();
            let indexed:(String,String,String,String,Vec<u8>,Vec<u8>)=stage.with_connection(WritePhase::Sort,|db|Ok(db.query_row("SELECT CASE WHEN length(CAST(claim_node_id AS BLOB))<=4096 THEN claim_node_id ELSE NULL END,CASE WHEN length(CAST(subject_node_id AS BLOB))<=4096 THEN subject_node_id ELSE NULL END,CASE WHEN length(CAST(object_node_id AS BLOB))<=4096 THEN object_node_id ELSE NULL END,CASE WHEN length(CAST(predicate_id AS BLOB))<=4096 THEN predicate_id ELSE NULL END,CASE WHEN length(source_claim_sha256)=32 THEN source_claim_sha256 ELSE NULL END,CASE WHEN length(trace_sha256)=32 THEN trace_sha256 ELSE NULL END FROM knowledge_claim_dependencies WHERE claim_ref=?1",[&raw.id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?)))?))?;
            let claim_sha = Digest256::from_hex(required(t, "claim_sha256")?)
                .map_err(|_| Error::Invalid("Claim source digest"))?;
            if indexed.0 != required(t, "claim_node_id")?
                || indexed.1 != required(t, "subject_node_id")?
                || indexed.2 != required(t, "object_node_id")?
                || indexed.3 != required(t, "predicate")?
                || indexed.4 != claim_sha.as_bytes().as_slice()
                || indexed.5 != Digest256::of_bytes(&raw.payload).as_bytes().as_slice()
            {
                return Err(Error::Invalid("Claim prepared trace dependency"));
            }
            traces += 1;
        }
        match page.into_next_id()? {
            Some(next) => after = Some(next),
            None => break,
        }
    }
    let (indexed_traces, indexed_edges): (u64, u64) =
        stage.with_connection(WritePhase::Sort, |db| {
            Ok((
                db.query_row(
                    "SELECT count(*) FROM knowledge_claim_dependencies",
                    [],
                    |r| r.get(0),
                )?,
                db.query_row(
                    "SELECT count(*) FROM knowledge_claim_edge_bindings WHERE listed_by_trace=1",
                    [],
                    |r| r.get(0),
                )?,
            ))
        })?;
    if traces != prepared.claim_traces
        || indexed_traces != traces
        || indexed_edges != prepared.edges
    {
        return Err(Error::Invalid("Claim prepared index count"));
    }
    Ok(())
}

/// Source-owned object references alone bind dossiers; no native-prefix stripping.
fn dossier(
    stage: &mut KnowledgeStage<'_>,
    item: &Value,
    normalizer: &ClaimNormalizer<'_>,
) -> Result<Option<String>> {
    let props = &item["properties"];
    let raw_kind = text(item.get("node_kind"));
    let kind = if raw_kind == Some("identity") {
        text(props.get("identity_kind"))
            .or_else(|| text(props.get("identity_type")))
            .or_else(|| text(props.get("record_type")))
            .or_else(|| {
                text(
                    props
                        .get("source_record")
                        .and_then(|r| r.get("record_type")),
                )
            })
    } else {
        raw_kind
    };
    if !kind.is_some_and(|k| normalizer.dossier_kinds.contains(k)) {
        return Ok(None);
    }
    for candidate in [
        props.get("identity_ref"),
        props.get("source_record").and_then(|r| r.get("record_id")),
        props
            .get("source_record")
            .and_then(|r| r.get("composite_id")),
        props
            .get("source_record")
            .and_then(|r| r.get("artifact_id")),
    ] {
        if let Some(id) = text(candidate) {
            let normalized_id = format!("{}:{id}", normalizer.navigation_graph);
            let exists = stage.with_connection(WritePhase::Sort, |db| {
                Ok(db.query_row(
                    "SELECT EXISTS(SELECT 1 FROM knowledge_nodes WHERE id=?1)",
                    [&normalized_id],
                    |r| r.get::<_, bool>(0),
                )?)
            })?;
            if exists {
                let candidate = node(stage, &normalized_id, normalizer.limits.max_output_bytes)?;
                if text(candidate.get("source_dossier_ref")) == Some(id) {
                    return Ok(Some(id.to_owned()));
                }
            }
        }
    }
    Ok(None)
}

/// Materialize the complete prepared source node cut. The parent must insert
/// source-navigation dossiers first and retains raw_records until readable joins.
pub fn materialize_source_claim_nodes(
    stage: &mut KnowledgeStage<'_>,
    prepared: &ClaimPrepareReceipt,
    normalizer: &ClaimNormalizer<'_>,
) -> Result<u64> {
    let result = (|| {
        verify_prepared_dependencies(stage, prepared, normalizer.limits)?;
        let mut after = None;
        let mut count = 0u64;
        let mut work = 0;
        let mut root = Digest256Hasher::new();
        let mut order: i64 = stage.with_connection(WritePhase::Sort, |db| {
            Ok(db.query_row(
                "SELECT COALESCE(MAX(source_order)+1,0) FROM knowledge_nodes",
                [],
                |r| r.get(0),
            )?)
        })?;
        loop {
            let (page_rows, physical_rows, physical_bytes) = stage.exact_source_write_page_limits(
                normalizer.limits.max_page_rows,
                normalizer.limits.max_output_bytes,
                normalizer.limits.max_raw_bytes,
            )?;
            let page = stage.scoped_scan_input(
                &prepared.source_graph,
                "nodes",
                after.as_deref(),
                page_rows,
            )?;
            stage.with_write_page(WritePhase::Normalized, physical_rows, physical_bytes, |stage| {
            for raw in &page.rows {
                charge(
                    &mut work,
                    raw.payload.len(),
                    normalizer.limits.max_work_bytes,
                )?;
                root_item(
                    &mut root,
                    &raw.id,
                    Digest256::of_bytes(&raw.payload).as_bytes(),
                );
                let predicate:Option<String>=stage.with_connection(WritePhase::Sort,|db|Ok(db.query_row("SELECT predicate_id FROM knowledge_claim_dependencies WHERE object_node_id=?1 ORDER BY claim_ref DESC LIMIT 1",[&raw.id],|r|r.get(0)).optional()?))?;
                let source = SourceRow::parse(&raw.payload, normalizer.limits.max_raw_bytes)?;
                let dossier = dossier(stage, source.value(), normalizer)?;
                normalizer.verify(&raw, prepared)?;
                let material_reservation = normalizer.limits.max_raw_bytes.checked_mul(2)
                    .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Vec<u8>>()))
                    .ok_or(Error::Budget("Claim retained source material state"))?;
                let _material_hold = stage.owned_creation_state()
                    .map(|state| state.hold(material_reservation)).transpose()?;
                let (output, material) = if let Some(run) = &normalizer.processing_run {
                    let source = processing_source_input(
                        run,
                        "claim-node-source",
                        &prepared.source_graph,
                        &raw.id,
                        &raw.payload,
                    )?;
                    let predicate_input = run.input(
                        &format!("claim-node-predicate:{}:{}", prepared.source_graph, raw.id),
                        &json!({"predicate":predicate}),
                    )?;
                    let dossier_input = run.input(
                        &format!("claim-node-dossier:{}:{}", prepared.source_graph, raw.id),
                        &json!({"dossier":dossier}),
                    )?;
                    let config = run.input(
                        &format!("claim-normalizer-config:{}", prepared.source_graph),
                        &processing_config(normalizer)?,
                    )?;
                    let fingerprint = processing_fingerprint!(
                        "knowledge_source_claims::normalize_supplied_node_with_source",
                        [
include_bytes!("knowledge_source_claims.rs"),
                            include_bytes!("knowledge_source_navigation_node.rs"),
                            include_bytes!("knowledge_philosophy_display.rs"),
                            include_bytes!("knowledge_normalization.rs"),
                            include_bytes!("knowledge_seal.rs"),
                            include_bytes!("processing_cache.rs"),
                        ],
                    );
                    let task = run.evaluate_task(
                        &format!("claim-node-material:{}:{}", prepared.source_graph, raw.id),
                        "claim-node-material",
                        "knowledge_source_claims::normalize_supplied_node_with_source",
                        fingerprint,
                        &json!({"source_graph":prepared.source_graph,"native_id":raw.id,"source_cut":prepared.source_cut,"max_raw_bytes":normalizer.limits.max_raw_bytes,"max_output_bytes":normalizer.limits.max_output_bytes}),
                        &[source, predicate_input, dossier_input, config],
                        || {
                            let (output, material) = normalizer
                                .normalize_supplied_node_with_source(
                                    &raw,
                                    predicate.as_deref(),
                                    dossier.as_deref(),
                                )?;
                            let material = String::from_utf8(material)
                                .map_err(|_| Error::Invalid("Claim ordered material UTF8"))?;
                            Ok(json!({"output":output,"material":material}))
                        },
                    )?;
                    let material = task
                        .value()
                        .get("material")
                        .and_then(Value::as_str)
                        .ok_or(Error::Invalid("cached Claim node material"))?
                        .as_bytes()
                        .to_vec();
                    SourceRow::parse(&material, normalizer.limits.max_raw_bytes)?;
                    let node = run.evaluate_task(
                        &format!("claim-node:{}:{}", prepared.source_graph, raw.id),
                        "claim-node",
                        "knowledge_source_claims::normalized_node_projection",
                        fingerprint,
                        &json!({"source_graph":prepared.source_graph,"native_id":raw.id}),
                        std::slice::from_ref(&task),
                        || task.value().get("output").cloned()
                            .ok_or(Error::Invalid("cached Claim node output")),
                    )?;
                    (node.value().clone(), material)
                } else {
                    normalizer.normalize_supplied_node_with_source(
                        &raw,
                        predicate.as_deref(),
                        dossier.as_deref(),
                    )?
                };
                let bytes = encode(&output, normalizer.limits.max_output_bytes)?;
                charge(&mut work, bytes.len(), normalizer.limits.max_work_bytes)?;
                stage.insert_node_with_exact_source(NodeRow {
                    id: required(&output, "id")?,
                    source_graph: &prepared.source_graph,
                    native_id: Some(&raw.id),
                    entity_id: Some(required(&output, "entity_id")?),
                    kind_id: required(&output, "kind_id")?,
                    type_id: required(&output, "type_id")?,
                    source_order: order,
                    payload: &bytes,
                }, &material)?;
                count += 1;
                order = order
                    .checked_add(1)
                    .ok_or(Error::Budget("Claim node order"))?;
            }
            Ok(())
            })?;
            match page.into_next_id()? {
                Some(id) => after = Some(id),
                None => break,
            }
        }
        if count != prepared.nodes || root.finalize().to_hex() != prepared.node_input_root_sha256 {
            return Err(Error::Invalid("Claim normalized node cut root"));
        }
        Ok(count)
    })();
    if result.is_err() {
        stage.poison();
    }
    result
}

#[derive(Clone, Debug)]
pub struct ClaimContextReceipt {
    pub source_cut: String,
    pub contexts: u64,
    pub root_sha256: String,
}
fn context_root(
    stage: &mut KnowledgeStage<'_>,
    limits: ClaimNormalizeLimits,
) -> Result<(u64, String)> {
    stage.with_connection(WritePhase::Sort,|db|{
    let mut hash=Digest256Hasher::new();hash.update(b"tos-claim-context-groups-v1\0");let mut count=0;let mut work=0;
    let mut query=db.prepare("SELECT CASE WHEN typeof(source_graph)='text' AND length(CAST(source_graph AS BLOB))<=4096 THEN source_graph ELSE NULL END, CASE WHEN typeof(claim_ref)='text' AND length(CAST(claim_ref AS BLOB))<=4096 THEN claim_ref ELSE NULL END,CASE WHEN typeof(context_sha256)='blob' AND length(context_sha256)=32 THEN context_sha256 ELSE NULL END,CASE WHEN typeof(owner_native_id)='text' AND length(CAST(owner_native_id AS BLOB))<=4096 THEN owner_native_id ELSE NULL END,CASE WHEN typeof(owner_sha256)='blob' AND length(owner_sha256)=32 THEN owner_sha256 ELSE NULL END,CASE WHEN typeof(context_json)='blob' AND length(context_json)<=?1 THEN context_json ELSE NULL END FROM knowledge_claim_context_groups ORDER BY source_graph,claim_ref,encounter")?;
    let mut rows=query.query([limits.max_output_bytes])?;while let Some(row)=rows.next()?{
        let graph:String=row.get(0)?;let reference:String=row.get(1)?;let sha:Vec<u8>=row.get(2)?;
        let owner:String=row.get(3)?;let owner_sha:Vec<u8>=row.get(4)?;let bytes:Vec<u8>=row.get(5)?;
        charge(&mut work,bytes.len()+graph.len()+reference.len()+owner.len(),limits.max_work_bytes)?;
        let context=SourceRow::parse(&bytes,limits.max_output_bytes)?;
        if Digest256::from_hex(&stable_digest(context.value())?).map_err(|_|Error::Invalid("Claim context digest"))?.as_bytes().as_slice()!=sha {return Err(Error::Invalid("Claim context JSON digest"));}
        root_item(&mut hash,&graph,&[]);root_item(&mut hash,&reference,&sha);root_item(&mut hash,&owner,&owner_sha);count+=1;}
    Ok((count,hash.finalize().to_hex()))})
}

/// Build once after all source base-node passes, before relation materialization.
/// Encounter order is preserved; equivalent context values coalesce by stable digest.
pub fn prepare_claim_context_groups(
    stage: &mut KnowledgeStage<'_>,
    limits: ClaimNormalizeLimits,
) -> Result<ClaimContextReceipt> {
    let result = (|| {
        limits.validate()?;
        stage.create_preparation_tables(crate::knowledge_stage::preparation_schema!(
            table r#"knowledge_claim_context_groups(source_graph TEXT NOT NULL,claim_ref TEXT NOT NULL,encounter INTEGER NOT NULL,context_sha256 BLOB NOT NULL,context_json BLOB NOT NULL,owner_native_id TEXT NOT NULL,owner_sha256 BLOB NOT NULL,PRIMARY KEY(source_graph,claim_ref,context_sha256))"#,
            index r#"knowledge_claim_context_order ON knowledge_claim_context_groups(source_graph,claim_ref,encounter)"#
        ))?;
        let mut after = -1i64;
        let mut work = 0;
        loop {
            let _page_hold = stage.hold_normalized_page(1, limits.max_output_bytes, 0)?;
            let rows: Vec<(i64, String, String, Vec<u8>)> = if let Some(state) =
                stage.owned_creation_state()
            {
                let selected = stage.with_connection(WritePhase::Sort, |db| {
                    let mut query = db.prepare("SELECT source_order,CASE WHEN typeof(id)='text' AND length(CAST(id AS BLOB)) BETWEEN 1 AND 4096 THEN id END,CASE WHEN typeof(source_graph)='text' AND length(CAST(source_graph AS BLOB)) BETWEEN 1 AND 4096 THEN source_graph END,CASE WHEN typeof(native_id)='text' AND length(CAST(native_id AS BLOB)) BETWEEN 1 AND 4096 THEN native_id END,kind_id FROM knowledge_nodes WHERE source_order>?1 AND kind_id IN ('claim','annotation-claim') ORDER BY source_order LIMIT 1")?;
                    let mut rows = query.query([after])?;
                    match rows.next()? {
                        Some(row) => Ok(Some((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?, row.get::<_, String>(4)?))),
                        None => Ok(None),
                    }
                })?;
                let mut selected_rows = Vec::with_capacity(1);
                if let Some((order, id, graph, native, kind)) = selected {
                    if order <= after {
                        return Err(Error::Invalid("Claim group base row order"));
                    }
                    stage
                        .with_node_payload_owned(&id, limits.max_output_bytes, |_, bytes| {
                            state.with_serde_owned_with_limits(
                                bytes,
                                SourceRow::json_limits(limits.max_output_bytes)?,
                                |value| {
                                    if required(value, "id")? != id
                                        || required(value, "kind_id")? != kind
                                        || required(value, "source_graph")? != graph
                                        || required(value, "native_id")? != native
                                    {
                                        return Err(Error::Invalid("Claim group base row binding"));
                                    }
                                    let graph = required(value, "source_graph")?;
                                    let native = required(value, "native_id")?;
                                    if graph.len() > 4096 || native.len() > 4096 {
                                        return Err(Error::Budget(
                                            "Claim group base row identity bytes",
                                        ));
                                    }
                                    state.charge_work(bytes.len())?;
                                    selected_rows.push((
                                        order,
                                        graph.to_owned(),
                                        native.to_owned(),
                                        bytes.to_vec(),
                                    ));
                                    Ok(())
                                },
                            )
                        })?
                        .ok_or(Error::Invalid("Claim group base row disappeared"))?;
                }
                selected_rows
            } else {
                stage.with_connection(WritePhase::Sort,|db|{
                let mut query=db.prepare("SELECT source_order,CASE WHEN typeof(source_graph)='text' AND length(CAST(source_graph AS BLOB))<=4096 THEN source_graph ELSE NULL END,CASE WHEN typeof(native_id)='text' AND length(CAST(native_id AS BLOB))<=4096 THEN native_id ELSE NULL END,CASE WHEN typeof(payload)='blob' AND payload_len=length(payload) AND length(payload)<=?3 THEN payload ELSE NULL END,CASE WHEN typeof(payload_sha256)='blob' AND length(payload_sha256)=32 THEN payload_sha256 ELSE NULL END,CASE WHEN typeof(id)='text' AND length(CAST(id AS BLOB))<=4096 THEN id ELSE NULL END,kind_id FROM knowledge_nodes WHERE source_order>?1 AND kind_id IN ('claim','annotation-claim') ORDER BY source_order LIMIT ?2")?;
                let mut selected=query.query(params![after,limits.max_page_rows,limits.max_output_bytes])?;let mut result=Vec::new();let mut page_work=0;
                while let Some(r)=selected.next()?{
                    let order:i64=r.get(0)?;let graph:String=r.get(1)?;let native:String=r.get(2)?;let bytes:Vec<u8>=r.get(3)?;let sha:Vec<u8>=r.get(4)?;let id:String=r.get(5)?;let kind:String=r.get(6)?;
                    charge(&mut page_work,bytes.len(),limits.max_work_bytes.saturating_sub(work))?;
                    if order<0 || order<=after || Digest256::of_bytes(&bytes).as_bytes().as_slice()!=sha {return Err(Error::Invalid("Claim group base row SHA/order"));}
                    let source=SourceRow::parse(&bytes,limits.max_output_bytes)?;
                    if required(source.value(),"id")?!=id||required(source.value(),"native_id")?!=native||required(source.value(),"source_graph")?!=graph||required(source.value(),"kind_id")?!=kind{return Err(Error::Invalid("Claim group base row binding"));}
                    result.push((order,graph,native,bytes));}Ok(result)})?
            };
            if rows.is_empty() {
                break;
            }
            for (order, graph, native, bytes) in rows {
                after = after.max(order);
                charge(&mut work, bytes.len(), limits.max_work_bytes)?;
                let source = SourceRow::parse_scoped_with_optional_owned_state(
                    &bytes,
                    limits.max_output_bytes,
                    stage.owned_creation_state(),
                )?;
                let owner_row = stage.scoped_raw_by_id(&graph, "nodes", &native)?;
                let owner = owner_row
                    .as_ref()
                    .ok_or(Error::Invalid("Claim group ordered source absent"))?;
                charge(&mut work, owner.payload.len(), limits.max_work_bytes)?;
                let owner_sha = Digest256::of_bytes(&owner.payload);
                for (offset, context) in source
                    .value()
                    .pointer("/semantics/assertion_contexts")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .enumerate()
                {
                    if offset >= limits.max_contexts {
                        return Err(Error::Budget("Claim carrier contexts"));
                    }
                    let fields = &context["fields"];
                    let Some(reference) = fields
                        .get("claim_id")
                        .or_else(|| fields.get("claim_ref"))
                        .and_then(|f| f.get("value"))
                        .and_then(Value::as_str)
                    else {
                        continue;
                    };
                    let mut bound = context.clone();
                    bound["binding_role"] = json!("referenced-claim");
                    let sha = Digest256::from_hex(&stable_digest(&bound)?)
                        .map_err(|_| Error::Invalid("Claim context stable digest"))?;
                    let raw = encode(&bound, limits.max_output_bytes)?;
                    charge(&mut work, raw.len(), limits.max_work_bytes)?;
                    stage.with_connection(WritePhase::Normalized,|db|{db.execute("INSERT OR IGNORE INTO knowledge_claim_context_groups VALUES (?1,?2,?3,?4,?5,?6,?7)",params![graph,reference,order.checked_mul(4096).and_then(|n|n.checked_add(offset as i64)).ok_or(Error::Budget("Claim context encounter"))?,sha.as_bytes().as_slice(),raw,native,owner_sha.as_bytes().as_slice()])?;Ok(())})?;
                }
            }
        }
        let (contexts, root_sha256) = context_root(stage, limits)?;
        Ok(ClaimContextReceipt {
            source_cut: stage.exact_receipt()?.binding.source_cut.clone(),
            contexts,
            root_sha256,
        })
    })();
    if result.is_err() {
        stage.poison();
    }
    result
}
pub fn verify_claim_context_groups(
    stage: &mut KnowledgeStage<'_>,
    receipt: &ClaimContextReceipt,
    limits: ClaimNormalizeLimits,
) -> Result<()> {
    let result = (|| {
        limits.validate()?;
        if stage.exact_receipt()?.binding.source_cut != receipt.source_cut
            || context_root(stage, limits)? != (receipt.contexts, receipt.root_sha256.clone())
        {
            return Err(Error::Invalid("Claim context group root/cut"));
        }
        Ok(())
    })();
    if result.is_err() {
        stage.poison();
    }
    result
}
pub fn claim_contexts(
    stage: &mut KnowledgeStage<'_>,
    receipt: &ClaimContextReceipt,
    graph: &str,
    reference: &str,
    limits: ClaimNormalizeLimits,
) -> Result<Vec<Value>> {
    limits.validate()?;
    if stage.exact_receipt()?.binding.source_cut != receipt.source_cut
        || Digest256::from_hex(&receipt.root_sha256).is_err()
    {
        return Err(Error::Invalid("Claim context lookup receipt"));
    }
    let rows:Vec<Vec<u8>>=stage.with_connection(WritePhase::Sort,|db|{
        let mut q=db.prepare("SELECT CASE WHEN typeof(context_json)='blob' AND length(context_json)<=?4 THEN context_json ELSE NULL END, CASE WHEN typeof(context_sha256)='blob' AND length(context_sha256)=32 THEN context_sha256 ELSE NULL END FROM knowledge_claim_context_groups WHERE source_graph=?1 AND claim_ref=?2 ORDER BY encounter LIMIT ?3")?;
        let mut selected=q.query(params![graph,reference,limits.max_contexts+1,limits.max_output_bytes])?;let mut result=Vec::new();let mut work=0;
        while let Some(row)=selected.next()?{if result.len()>=limits.max_contexts{return Err(Error::Budget("Claim group contexts"));}
            let raw:Vec<u8>=row.get(0)?;let sha:Vec<u8>=row.get(1)?;charge(&mut work,raw.len(),limits.max_output_bytes as u64)?;
            let source=SourceRow::parse(&raw,limits.max_output_bytes)?;
            if Digest256::from_hex(&stable_digest(source.value())?).map_err(|_|Error::Invalid("Claim context digest"))?.as_bytes().as_slice()!=sha{return Err(Error::Invalid("Claim group JSON digest"));}result.push(raw);}
        Ok(result)})?;
    rows.into_iter()
        .map(|raw| {
            Ok(SourceRow::parse(&raw, limits.max_output_bytes)?
                .value()
                .clone())
        })
        .collect()
}

/// Exact ordered owner envelopes for every referenced context. Source keys
/// are retained independently of normalized body order and checked by SHA.
pub fn claim_context_sources(
    stage: &mut KnowledgeStage<'_>,
    receipt: &ClaimContextReceipt,
    graph: &str,
    reference: &str,
    limits: ClaimNormalizeLimits,
) -> Result<Vec<Vec<u8>>> {
    limits.validate()?;
    if stage.exact_receipt()?.binding.source_cut != receipt.source_cut
        || Digest256::from_hex(&receipt.root_sha256).is_err()
    {
        return Err(Error::Invalid("Claim context witness receipt"));
    }
    let owners:Vec<(String,Vec<u8>)>=stage.with_connection(WritePhase::Sort,|db|{let mut q=db.prepare("SELECT CASE WHEN typeof(owner_native_id)='text' AND length(CAST(owner_native_id AS BLOB))<=4096 THEN owner_native_id ELSE NULL END,CASE WHEN typeof(owner_sha256)='blob' AND length(owner_sha256)=32 THEN owner_sha256 ELSE NULL END FROM knowledge_claim_context_groups WHERE source_graph=?1 AND claim_ref=?2 ORDER BY encounter LIMIT ?3")?;
        let rows=q.query_map(params![graph,reference,limits.max_contexts+1],|r|Ok((r.get(0)?,r.get(1)?)))?;Ok(rows.collect::<std::result::Result<_,_>>()?)})?;
    if owners.len() > limits.max_contexts {
        return Err(Error::Budget("Claim context witnesses"));
    }
    let mut work = 0;
    let mut result = Vec::new();
    for (native, sha) in owners {
        let raw_owner = stage.scoped_raw_by_id(graph, "nodes", &native)?;
        let raw = raw_owner
            .as_ref()
            .ok_or(Error::Invalid("Claim context witness absent"))?;
        if Digest256::of_bytes(&raw.payload).as_bytes().as_slice() != sha {
            return Err(Error::Invalid("Claim context witness digest"));
        }
        charge(&mut work, raw.payload.len(), limits.max_work_bytes)?;
        if raw.payload.len() > limits.max_raw_bytes {
            return Err(Error::Budget("Claim context witness bytes"));
        }
        let material = if stage.exact_receipt()?.collections.iter().any(|entry| {
            entry.source_graph == graph
                && entry.collection == "nodes"
                && entry.adapter_profile == PROFILE
        }) {
            ordered_claim_node_material(&raw.payload, limits.max_raw_bytes)?
        } else {
            if let Some(state) = stage.owned_creation_state() {
                state.charge_work(raw.payload.len())?;
            }
            raw.payload.clone()
        };
        charge(&mut work, material.len(), limits.max_work_bytes)?;
        result.push(material);
    }
    Ok(result)
}

/// All endpoint titles must already be globally finalized. The root records
/// that caller obligation; this pass checks exact prepared edge/raw closure.
pub fn materialize_source_claim_relations(
    stage: &mut KnowledgeStage<'_>,
    prepared: &ClaimPrepareReceipt,
    normalizer: &ClaimNormalizer<'_>,
    contexts: &ClaimContextReceipt,
    titles: &GlobalTitleReceipt,
) -> Result<u64> {
    let result = (|| {
        verify_prepared_dependencies(stage, prepared, normalizer.limits)?;
        verify_claim_context_groups(stage, contexts, normalizer.limits)?;
        verify_global_titles(stage, titles, 65536, normalizer.limits.max_work_bytes)?;
        stage.create_preparation_tables(crate::knowledge_stage::preparation_schema!(
            table r#"knowledge_claim_relation_material(source_graph TEXT NOT NULL,id TEXT NOT NULL,native_id TEXT NOT NULL,raw_sha256 BLOB NOT NULL,material_len INTEGER NOT NULL,material_sha256 BLOB NOT NULL,material BLOB NOT NULL,PRIMARY KEY(source_graph,id)) WITHOUT ROWID"#
        ))?;
        let mut after = None;
        let mut count = 0u64;
        let mut root = Digest256Hasher::new();
        let mut work = 0;
        let mut order: i64 = stage.with_connection(WritePhase::Sort, |db| {
            Ok(db.query_row(
                "SELECT COALESCE(MAX(source_order)+1,0) FROM knowledge_relations",
                [],
                |r| r.get(0),
            )?)
        })?;
        loop {
            let (page_rows, physical_rows, physical_bytes) = stage.exact_source_write_page_limits(
                normalizer.limits.max_page_rows,
                normalizer.limits.max_output_bytes,
                normalizer.limits.max_raw_bytes,
            )?;
            let page = stage.scoped_scan_input(
                &prepared.source_graph,
                "edges",
                after.as_deref(),
                page_rows,
            )?;
            stage.with_write_page(WritePhase::Normalized, physical_rows, physical_bytes, |stage| {
            for raw in &page.rows {
                charge(
                    &mut work,
                    raw.payload.len(),
                    normalizer.limits.max_work_bytes,
                )?;
                root_item(
                    &mut root,
                    &raw.id,
                    Digest256::of_bytes(&raw.payload).as_bytes(),
                );
                let source = SourceRow::parse(&raw.payload, normalizer.limits.max_raw_bytes)?;
                let item = source.value();
                let reference = required(item, "claim_ref")?;
                let (object_id,sha):(String,Vec<u8>)=stage.with_connection(WritePhase::Sort,|db|Ok(db.query_row("SELECT d.object_node_id,e.edge_sha256 FROM knowledge_claim_dependencies d JOIN knowledge_claim_edge_bindings e ON d.claim_ref=e.claim_ref WHERE e.edge_id=?1 AND e.claim_ref=?2",params![raw.id,reference],|r|Ok((r.get(0)?,r.get(1)?)))?))?;
                if Digest256::of_bytes(&raw.payload).as_bytes().as_slice() != sha {
                    return Err(Error::Invalid("Claim prepared edge digest"));
                }
                let object_row = stage.scoped_raw_by_id(&prepared.source_graph, "nodes", &object_id)?;
                let object = object_row.as_ref()
                    .ok_or(Error::Invalid("Claim edge object absent"))?;
                let target_row = stage.scoped_raw_by_id(&prepared.source_graph, "nodes", required(item, "to_id")?)?;
                let target = target_row.as_ref()
                    .ok_or(Error::Invalid("Claim edge target absent"))?;
                let object = SourceRow::parse(&object.payload, normalizer.limits.max_raw_bytes)?;
                let target = SourceRow::parse(&target.payload, normalizer.limits.max_raw_bytes)?;
                let from_graph =
                    text(item.get("from_source_graph")).unwrap_or(&prepared.source_graph);
                let to_graph = text(item.get("to_source_graph")).unwrap_or(&prepared.source_graph);
                let left = endpoint_title(
                    stage,
                    titles,
                    &format!("{from_graph}:{}", required(item, "from_id")?),
                    65536,
                )?;
                let right = endpoint_title(
                    stage,
                    titles,
                    &format!("{to_graph}:{}", required(item, "to_id")?),
                    65536,
                )?;
                let group = claim_contexts(
                    stage,
                    contexts,
                    &prepared.source_graph,
                    reference,
                    normalizer.limits,
                )?;
                let (output, material) = if let Some(run) = &normalizer.processing_run {
                    let source_input = processing_source_input(
                        run,
                        "claim-edge-source",
                        &prepared.source_graph,
                        &raw.id,
                        &raw.payload,
                    )?;
                    let object_input = processing_source_input(
                        run,
                        "claim-edge-object",
                        &prepared.source_graph,
                        &object_id,
                        &object_row
                            .as_ref()
                            .ok_or(Error::Invalid("Claim edge object absent"))?
                            .payload,
                    )?;
                    let target_id = required(item, "to_id")?;
                    let target_input = processing_source_input(
                        run,
                        "claim-edge-target",
                        &prepared.source_graph,
                        target_id,
                        &target_row
                            .as_ref()
                            .ok_or(Error::Invalid("Claim edge target absent"))?
                            .payload,
                    )?;
                    let left_id = format!("{from_graph}:{}", required(item, "from_id")?);
                    let right_id = format!("{to_graph}:{target_id}");
                    let left_title = processing_endpoint_title(run, &left_id, &left)?;
                    let right_title = processing_endpoint_title(run, &right_id, &right)?;
                    let group_task = processing_group(
                        run,
                        stage,
                        prepared,
                        contexts,
                        reference,
                        normalizer.limits,
                    )?;
                    let config = run.input(
                        &format!("claim-normalizer-config:{}", prepared.source_graph),
                        &processing_config(normalizer)?,
                    )?;
                    let fingerprint = processing_fingerprint!(
                        "knowledge_source_claims::normalize_base_relation",
                        [
include_bytes!("knowledge_source_claims.rs"),
                            include_bytes!("knowledge_global_titles.rs"),
                            include_bytes!("knowledge_source_claims_prepare.rs"),
                            include_bytes!("knowledge_normalization.rs"),
                            include_bytes!("knowledge_philosophy_display.rs"),
                            include_bytes!("knowledge_seal.rs"),
                            include_bytes!("processing_cache.rs"),
                        ],
                    );
                    let task = run.evaluate_task(
                        &format!("claim-relation:{}:{}", prepared.source_graph, raw.id),
                        "claim-relation",
                        "knowledge_source_claims::normalize_base_relation",
                        fingerprint,
                        &json!({"source_graph":prepared.source_graph,"edge_id":raw.id,"prepared_dependency_root":prepared.dependency_root_sha256,"max_raw_bytes":normalizer.limits.max_raw_bytes,"max_output_bytes":normalizer.limits.max_output_bytes}),
                        &[source_input, object_input, target_input, left_title, right_title, group_task, config],
                        || {
                            let output = normalizer.normalize_base_relation(
                                &raw,
                                prepared,
                                object.value(),
                                target.value(),
                                &left,
                                &right,
                                &group,
                            )?;
                            let material = ordered_claim_relation_material(
                                &raw.payload,
                                output
                                    .pointer("/source_record/payload")
                                    .ok_or(Error::Invalid("Claim normalized source payload"))?,
                                normalizer.limits.max_raw_bytes,
                            )?;
                            let material = String::from_utf8(material)
                                .map_err(|_| Error::Invalid("Claim ordered material UTF8"))?;
                            Ok(json!({"output":output,"material":material}))
                        },
                    )?;
                    let output = task
                        .value()
                        .get("output")
                        .cloned()
                        .ok_or(Error::Invalid("cached Claim relation output"))?;
                    let material = task
                        .value()
                        .get("material")
                        .and_then(Value::as_str)
                        .ok_or(Error::Invalid("cached Claim relation material"))?
                        .as_bytes()
                        .to_vec();
                    SourceRow::parse(&material, normalizer.limits.max_raw_bytes)?;
                    (output, material)
                } else {
                    let output = normalizer.normalize_base_relation(
                        &raw,
                        prepared,
                        object.value(),
                        target.value(),
                        &left,
                        &right,
                        &group,
                    )?;
                    let material = ordered_claim_relation_material(
                        &raw.payload,
                        output
                            .pointer("/source_record/payload")
                            .ok_or(Error::Invalid("Claim normalized source payload"))?,
                        normalizer.limits.max_raw_bytes,
                    )?;
                    (output, material)
                };
                let bytes = encode(&output, normalizer.limits.max_output_bytes)?;
                charge(&mut work, bytes.len(), normalizer.limits.max_work_bytes)?;
                charge(&mut work, material.len(), normalizer.limits.max_work_bytes)?;
                stage.with_connection(WritePhase::Sort,|db|{db.execute("INSERT INTO knowledge_claim_relation_material VALUES (?1,?2,?3,?4,?5,?6,?7)",params![prepared.source_graph,required(&output,"id")?,raw.id,&Digest256::of_bytes(&raw.payload).as_bytes()[..],material.len() as i64,&Digest256::of_bytes(&material).as_bytes()[..],material])?;Ok(())})?;
                stage.insert_relation_with_exact_source(RelationRow {
                    id: required(&output, "id")?,
                    source_graph: &prepared.source_graph,
                    native_id: Some(&raw.id),
                    from_id: required(&output, "from_id")?,
                    to_id: required(&output, "to_id")?,
                    predicate_id: required(&output, "predicate_id")?,
                    relation_type_id: required(&output, "relation_type_id")?,
                    source_order: order,
                    payload: &bytes,
                }, &material)?;
                count += 1;
                order = order
                    .checked_add(1)
                    .ok_or(Error::Budget("Claim relation order"))?;
            }
            Ok(())
            })?;
            match page.into_next_id()? {
                Some(id) => after = Some(id),
                None => break,
            }
        }
        if count != prepared.edges || root.finalize().to_hex() != prepared.edge_input_root_sha256 {
            return Err(Error::Invalid("Claim normalized edge cut root"));
        }
        Ok(count)
    })();
    if result.is_err() {
        stage.poison();
    }
    result
}

/// Complete genuine Claim endpoint contracts and attach exact referenced Claim
/// contexts to literal objects, retaining conflicting contributions in order.
pub fn finalize_source_claims(
    stage: &mut KnowledgeStage<'_>,
    prepared: &ClaimPrepareReceipt,
    normalizer: &ClaimNormalizer<'_>,
    contexts: &ClaimContextReceipt,
) -> Result<u64> {
    let result = (|| {
        verify_prepared_dependencies(stage, prepared, normalizer.limits)?;
        verify_claim_context_groups(stage, contexts, normalizer.limits)?;
        let mut after = None;
        let mut count = 0;
        let mut root = Digest256Hasher::new();
        let mut work = 0;
        let mut latest_literal: HashMap<String, crate::processing_cache::ProcessingValue> =
            HashMap::new();
        loop {
            let (write_rows, write_bytes) = stage.write_page_limits();
            let page_rows = normalizer
                .limits
                .max_page_rows
                .min(write_rows / 2)
                .min(write_bytes as usize / (2 * normalizer.limits.max_output_bytes));
            let page = stage.scoped_scan_input(
                &prepared.source_graph,
                "claim_traces",
                after.as_deref(),
                page_rows,
            )?;
            stage.with_write_page(
                WritePhase::Normalized,
                page_rows * 2,
                (page_rows * 2 * normalizer.limits.max_output_bytes) as u64,
                |stage| {
                    for raw in &page.rows {
                        charge(
                            &mut work,
                            raw.payload.len(),
                            normalizer.limits.max_work_bytes,
                        )?;
                        root_item(
                            &mut root,
                            &raw.id,
                            Digest256::of_bytes(&raw.payload).as_bytes(),
                        );
                        let source =
                            SourceRow::parse(&raw.payload, normalizer.limits.max_raw_bytes)?;
                        let trace = source.value();
                        let mut endpoints = Vec::new();
                        for field in ["claim_node_id", "subject_node_id", "object_node_id"] {
                            endpoints.push(node(
                                stage,
                                &format!("{}:{}", prepared.source_graph, required(trace, field)?),
                                normalizer.limits.max_output_bytes,
                            )?);
                        }
                        let output = if let Some(run) = &normalizer.processing_run {
                            let claim_id = required(trace, "claim_node_id")?;
                            let subject_id = required(trace, "subject_node_id")?;
                            let object_id = required(trace, "object_node_id")?;
                            let stable_ids = [claim_id, subject_id, object_id]
                                .map(|native| format!("{}:{native}", prepared.source_graph));
                            let mut dependencies = Vec::with_capacity(5);
                            for (index, value) in endpoints.iter().enumerate() {
                                dependencies.push(processing_current_node(
                                    run,
                                    &stable_ids[index],
                                    &format!("claim-finalize-node:{}:{}", raw.id, index),
                                    value,
                                )?);
                            }
                            let trace_input = processing_source_input(
                                run,
                                "claim-finalize-trace",
                                &prepared.source_graph,
                                &raw.id,
                                &raw.payload,
                            )?;
                            dependencies.push(trace_input.clone());
                            let predicate = required(trace, "predicate")?;
                            let association = run.input(
                                &format!("claim-association:{}:{}", prepared.source_graph, raw.id),
                                &json!({
                                    "schema":"tos_claim_association_v1",
                                    "source_graph":prepared.source_graph,
                                    "claim_node_id":stable_ids[0],
                                    "subject_node_id":stable_ids[1],
                                    "object_node_id":stable_ids[2],
                                    "predicate_id":predicate,
                                    "prepared_dependency_root_sha256":prepared.dependency_root_sha256,
                                }),
                            )?;
                            let relation_registry = run.input(
                                "source-registry:relations",
                                &json!({
                                    "registry_ref":normalizer.relation_ref,
                                    "sha256":normalizer.registry.relation_sha256,
                                }),
                            )?;
                            let policy_fingerprint = processing_fingerprint!(
                                "knowledge_source_claims::claim_policy",
                                [
include_bytes!("knowledge_source_claims.rs"),
                                    include_bytes!("knowledge_registry.rs"),
                                    include_bytes!("processing_cache.rs"),
                                ],
                            );
                            let expected_policy = normalizer.claim_policy_value(predicate)?;
                            let claim_policy = run.evaluate_task(
                                &format!("claim-policy:{}:{}", prepared.source_graph, raw.id),
                                "claim-policy",
                                "knowledge_source_claims::claim_policy",
                                policy_fingerprint,
                                &json!({"source_graph":prepared.source_graph,"trace_id":raw.id,"predicate_id":predicate}),
                                &[relation_registry, trace_input.clone()],
                                || Ok(expected_policy.clone()),
                            )?;
                            let config = run.input(
                                &format!("claim-normalizer-config:{}", prepared.source_graph),
                                &processing_config(normalizer)?,
                            )?;
                            dependencies.push(config);
                            let fingerprint = processing_fingerprint!(
                                "knowledge_source_claims::finalize_claim",
                                [
include_bytes!("knowledge_source_claims.rs"),
                                    include_bytes!("knowledge_source_claims_prepare.rs"),
                                    include_bytes!("knowledge_normalization.rs"),
                                    include_bytes!("knowledge_seal.rs"),
                                    include_bytes!("processing_cache.rs"),
                                ],
                            );
                            let task = run.evaluate_task(
                                &format!("claim-update:{}:{}", prepared.source_graph, raw.id),
                                "claim-finalization",
                                "knowledge_source_claims::finalize_claim",
                                fingerprint,
                                &json!({"source_graph":prepared.source_graph,"trace_id":raw.id,"dependency_root":prepared.dependency_root_sha256,"max_output_bytes":normalizer.limits.max_output_bytes}),
                                &[dependencies.as_slice(), &[association.clone(), claim_policy.clone()]].concat(),
                                || {
                                    if expected_policy != normalizer.claim_policy_value(predicate)? {
                                        return Err(Error::Invalid("Claim policy producer mismatch"));
                                    }
                                    normalizer.finalize_claim(
                                        &endpoints[0],
                                        &endpoints[1],
                                        &endpoints[2],
                                        trace,
                                    )
                                },
                            )?;
                            let output = task.value().clone();
                            if required(&output, "id")? != stable_ids[0] {
                                return Err(Error::Invalid("cached Claim update identity"));
                            }
                            let object_stable_id = stable_ids[2].clone();
                            if endpoints[2]
                                .pointer("/source_record/payload/node_kind")
                                .and_then(Value::as_str)
                                == Some("literal")
                            {
                                let group = claim_contexts(
                                    stage,
                                    contexts,
                                    &prepared.source_graph,
                                    &raw.id,
                                    normalizer.limits,
                                )?;
                                if !group.is_empty() {
                                    let group_task = processing_group(
                                        run,
                                        stage,
                                        prepared,
                                        contexts,
                                        &raw.id,
                                        normalizer.limits,
                                    )?;
                                    if group_task.value() != &json!(group) {
                                        return Err(Error::Invalid("cached Claim context group differs from stage"));
                                    }
                                    let literal_kind = endpoints[2]
                                        .pointer("/source_record/payload/node_kind")
                                        .and_then(Value::as_str)
                                        .ok_or(Error::Invalid("Claim literal raw node kind absent"))?;
                                    let literal_kind_input = run.input(
                                        &format!("literal-node-kind:{}:{}", prepared.source_graph, object_id),
                                        &json!({"raw_node_kind":literal_kind}),
                                    )?;
                                    let mut contribution_dependencies = vec![processing_current_node(
                                        run,
                                        &object_stable_id,
                                        &format!("literal-claim-current:{}", raw.id),
                                        &endpoints[2],
                                    )?];
                                    contribution_dependencies.push(trace_input.clone());
                                    contribution_dependencies.push(association.clone());
                                    contribution_dependencies.push(literal_kind_input);
                                    if let Some(previous) = latest_literal.get(&object_stable_id) {
                                        contribution_dependencies.push(previous.clone());
                                    }
                                    contribution_dependencies.push(group_task);
                                    let literal_fingerprint = processing_fingerprint!(
                                        "knowledge_source_claims::literal_claim_contribution",
                                        [
include_bytes!("knowledge_source_claims.rs"),
                                            include_bytes!("knowledge_normalization.rs"),
                                            include_bytes!("knowledge_seal.rs"),
                                            include_bytes!("processing_cache.rs"),
                                        ],
                                    );
                                    let contribution = run.evaluate_task(
                                        &format!("literal-claim-contribution:{}:{}", prepared.source_graph, raw.id),
                                        "literal-claim-contribution",
                                        "knowledge_source_claims::literal_claim_contribution",
                                        literal_fingerprint,
                                        &json!({"source_graph":prepared.source_graph,"trace_id":raw.id,"object_id":object_stable_id,"max_contexts":normalizer.limits.max_contexts,"max_output_bytes":normalizer.limits.max_output_bytes}),
                                        &contribution_dependencies,
                                        || {
                                            let mut object = endpoints[2].clone();
                                            let semantics = object["semantics"]
                                                .as_object_mut()
                                                .ok_or(Error::Invalid("Claim literal semantics"))?;
                                            let existing = semantics
                                                .entry("assertion_contexts")
                                                .or_insert_with(|| json!([]))
                                                .as_array_mut()
                                                .ok_or(Error::Invalid("Claim literal contexts"))?;
                                            for context in group {
                                                if !existing.contains(&context) {
                                                    existing.push(context);
                                                }
                                            }
                                            if existing.len() > normalizer.limits.max_contexts {
                                                return Err(Error::Budget("Claim literal accumulated contexts"));
                                            }
                                            stamp_content_revision(&mut object, normalizer.limits.max_output_bytes)?;
                                            Ok(object)
                                        },
                                    )?;
                                    if required(contribution.value(), "id")? != object_stable_id {
                                        return Err(Error::Invalid("cached Claim literal identity"));
                                    }
                                    update_node(stage, contribution.value(), normalizer.limits.max_output_bytes)?;
                                    latest_literal.insert(object_stable_id, contribution);
                                }
                            }
                            output
                        } else {
                            normalizer.finalize_claim(
                                &endpoints[0],
                                &endpoints[1],
                                &endpoints[2],
                                trace,
                            )?
                        };
                        update_node(stage, &output, normalizer.limits.max_output_bytes)?;
                        if normalizer.processing_run.is_none() && endpoints[2]
                            .pointer("/source_record/payload/node_kind")
                            .and_then(Value::as_str)
                            == Some("literal")
                        {
                            let group = claim_contexts(
                                stage,
                                contexts,
                                &prepared.source_graph,
                                &raw.id,
                                normalizer.limits,
                            )?;
                            if !group.is_empty() {
                                let object = &mut endpoints[2];
                                let semantics = object["semantics"]
                                    .as_object_mut()
                                    .ok_or(Error::Invalid("Claim literal semantics"))?;
                                let existing = semantics
                                    .entry("assertion_contexts")
                                    .or_insert_with(|| json!([]))
                                    .as_array_mut()
                                    .ok_or(Error::Invalid("Claim literal contexts"))?;
                                for context in group {
                                    if !existing.contains(&context) {
                                        existing.push(context);
                                    }
                                }
                                if existing.len() > normalizer.limits.max_contexts {
                                    return Err(Error::Budget(
                                        "Claim literal accumulated contexts",
                                    ));
                                }
                                stamp_content_revision(object, normalizer.limits.max_output_bytes)?;
                                update_node(stage, object, normalizer.limits.max_output_bytes)?;
                            }
                        }
                        count += 1;
                    }
                    Ok(())
                },
            )?;
            match page.into_next_id()? {
                Some(id) => after = Some(id),
                None => break,
            }
        }
        if count != prepared.claim_traces
            || root.finalize().to_hex() != prepared.trace_input_root_sha256
        {
            return Err(Error::Invalid("Claim finalization trace cut"));
        }
        Ok(count)
    })();
    if result.is_err() {
        stage.poison();
    }
    result
}

/// Retire private dependencies after relation, inherited-view and readable
/// consumers finish. Counts and exact trace contracts prevent an unfinished
/// Claim pass from being mistaken for a complete selected model.
pub fn clear_source_claim_indices(
    stage: &mut KnowledgeStage<'_>,
    prepared: &ClaimPrepareReceipt,
    contexts: &ClaimContextReceipt,
    limits: ClaimNormalizeLimits,
) -> Result<()> {
    let result = (|| {
        limits.validate()?;
        verify_prepared_dependencies(stage, prepared, limits)?;
        verify_claim_context_groups(stage, contexts, limits)?;
        let (nodes, edges): (u64, u64) = stage.with_connection(WritePhase::Finalize, |db| {
            Ok((
                db.query_row(
                    "SELECT count(*) FROM knowledge_nodes WHERE source_graph=?1",
                    [&prepared.source_graph],
                    |r| r.get(0),
                )?,
                db.query_row(
                    "SELECT count(*) FROM knowledge_relations WHERE source_graph=?1",
                    [&prepared.source_graph],
                    |r| r.get(0),
                )?,
            ))
        })?;
        if nodes != prepared.nodes || edges != prepared.edges {
            return Err(Error::Invalid("Claim cleanup incomplete normalized cut"));
        }
        let mut after = None;
        let mut work = 0;
        loop {
            let page = stage.scoped_scan_input(
                &prepared.source_graph,
                "claim_traces",
                after.as_deref(),
                limits.max_page_rows,
            )?;
            for raw in &page.rows {
                charge(&mut work, raw.payload.len(), limits.max_work_bytes)?;
                let trace = SourceRow::parse(&raw.payload, limits.max_raw_bytes)?;
                let carrier = node(
                    stage,
                    &format!(
                        "{}:{}",
                        prepared.source_graph,
                        required(trace.value(), "claim_node_id")?
                    ),
                    limits.max_output_bytes,
                )?;
                charge(
                    &mut work,
                    encode(&carrier, limits.max_output_bytes)?.len(),
                    limits.max_work_bytes,
                )?;
                if carrier
                    .pointer("/semantics/claim/claim_id")
                    .and_then(Value::as_str)
                    != Some(raw.id.as_str())
                    || carrier.pointer("/attributes/claim_trace") != Some(trace.value())
                {
                    return Err(Error::Invalid("Claim cleanup incomplete final trace"));
                }
            }
            match page.into_next_id()? {
                Some(next) => after = Some(next),
                None => break,
            }
        }
        stage.with_connection(WritePhase::Finalize,|db|{db.execute_batch("DROP TABLE knowledge_claim_context_groups;DROP TABLE knowledge_claim_edge_bindings;DROP TABLE knowledge_claim_dependencies;DROP TABLE IF EXISTS knowledge_claim_relation_material")?;Ok(())})
    })();
    if result.is_err() {
        stage.poison();
    }
    result
}
