//! Bounded, supplied-neighborhood direct-carrier replacement candidates.
//!
//! This route consumes only caller-supplied rows. It does not prove complete
//! incidence, source admission, global semantics, catalog state or publication.

use crate::knowledge_base::ADDRESS_UPDATE_SOURCE_GRAPHS;
use crate::knowledge_normalization::{
    SourceRow, stable_digest, stamp_addressed_candidate_content_revision,
};
use crate::knowledge_readable_context::{
    ReadableContextCarrier, ReadableContextCompiler, ReadableContextLimits,
};
use crate::knowledge_source_claims::{ClaimNormalizeLimits, ClaimNormalizer};
use crate::knowledge_source_navigation_node::{NavigationNodeLimits, NavigationNodeNormalizer};
use crate::knowledge_source_navigation_relation::direct_assertion_context_addressed_candidate;
use crate::knowledge_stage::SeekRow;
use crate::{
    BaseNodeOverrides, BaseNormalizationLimits, Error, KnowledgeBaseNormalizer, KnowledgeRegistry,
    NATIVE_KNOWLEDGE_ADAPTER_PROFILES, QueryVocabulary, Result,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use tos_foundation::{Digest256, Digest256Hasher};

const DEFAULT_INPUT_BYTES: usize = 16 * 1024 * 1024;
const DEFAULT_OUTPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_RELATIONS: usize = 1024;
const MAX_ENDPOINTS: usize = 2048;
#[cfg(test)]
const QUERY_VOCABULARY: &[u8] =
    include_bytes!("../../../../ToS/doctrine/semantic-interchange/query-vocabulary.v1.json");

/// Current normalization inputs supplied by the capture or prepare owner.
/// `processor_digest` is the producer's exact current software binding;
/// byte slices are the exact descriptor and registries used for this run.
#[derive(Clone, Copy, Debug)]
pub struct AddressedNormalizationContext<'a> {
    pub processor_digest: &'a str,
    pub descriptor_bytes: &'a [u8],
    pub entity_registry_bytes: &'a [u8],
    pub relation_registry_bytes: &'a [u8],
}

#[derive(Clone, Copy, Debug)]
pub struct AddressedReplacementLimits {
    pub max_relations: usize,
    pub max_endpoints: usize,
    pub max_input_bytes: usize,
    pub max_output_bytes: usize,
}
impl Default for AddressedReplacementLimits {
    fn default() -> Self {
        Self {
            max_relations: MAX_RELATIONS,
            max_endpoints: MAX_ENDPOINTS,
            max_input_bytes: DEFAULT_INPUT_BYTES,
            max_output_bytes: DEFAULT_OUTPUT_BYTES,
        }
    }
}
impl AddressedReplacementLimits {
    fn validate(self) -> Result<()> {
        if self.max_input_bytes == 0 || self.max_output_bytes == 0 {
            return Err(Error::Budget("addressed replacement limits"));
        }
        Ok(())
    }
}

struct CountWriter {
    bytes: usize,
    cap: usize,
}
impl std::io::Write for CountWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.bytes = self
            .bytes
            .checked_add(bytes.len())
            .filter(|next| *next <= self.cap)
            .ok_or_else(|| std::io::Error::other("addressed replacement byte budget"))?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn add(total: &mut usize, amount: usize, cap: usize) -> Result<()> {
    *total = total
        .checked_add(amount)
        .filter(|next| *next <= cap)
        .ok_or(Error::Budget("addressed replacement byte budget"))?;
    Ok(())
}

struct CappedDigestWriter {
    bytes: usize,
    cap: usize,
    digest: Digest256Hasher,
}
impl std::io::Write for CappedDigestWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.bytes = self
            .bytes
            .checked_add(bytes.len())
            .filter(|next| *next <= self.cap)
            .ok_or_else(|| std::io::Error::other("addressed context digest byte budget"))?;
        self.digest.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn validation_digest(value: &Value, cap: usize) -> Result<String> {
    let mut writer = CappedDigestWriter {
        bytes: 0,
        cap,
        digest: Digest256Hasher::new(),
    };
    serde_json::to_writer(&mut writer, value)
        .map_err(|_| Error::Budget("addressed context digest bytes"))?;
    Ok(writer.digest.finalize().to_hex())
}

/// Exact compact UTF-8 serde JSON size, with a bounded recursion depth.
fn json_size(value: &Value, remaining: usize, depth: usize) -> Result<usize> {
    if depth > 64 || remaining == 0 {
        return Err(Error::Budget("addressed replacement JSON depth/bytes"));
    }
    match value {
        Value::Array(items) => {
            let mut bytes = 2usize;
            if bytes > remaining || items.len() > remaining.saturating_sub(bytes).saturating_add(1)
            {
                return Err(Error::Budget("addressed replacement byte budget"));
            }
            for (index, item) in items.iter().enumerate() {
                if index != 0 {
                    add(&mut bytes, 1, remaining)?;
                }
                let size = json_size(item, remaining - bytes, depth + 1)?;
                add(&mut bytes, size, remaining)?;
            }
            Ok(bytes)
        }
        Value::Object(fields) => {
            let mut bytes = 2usize;
            if bytes > remaining || fields.len() > remaining.saturating_sub(bytes).saturating_add(1)
            {
                return Err(Error::Budget("addressed replacement byte budget"));
            }
            for (index, (key, item)) in fields.iter().enumerate() {
                if index != 0 {
                    add(&mut bytes, 1, remaining)?;
                }
                let mut writer = CountWriter {
                    bytes: 0,
                    cap: remaining.saturating_sub(bytes),
                };
                serde_json::to_writer(&mut writer, key)
                    .map_err(|_| Error::Budget("addressed replacement byte budget"))?;
                add(&mut bytes, writer.bytes, remaining)?;
                add(&mut bytes, 1, remaining)?;
                let size = json_size(item, remaining - bytes, depth + 1)?;
                add(&mut bytes, size, remaining)?;
            }
            Ok(bytes)
        }
        scalar => {
            let mut writer = CountWriter {
                bytes: 0,
                cap: remaining,
            };
            serde_json::to_writer(&mut writer, scalar)
                .map_err(|_| Error::Budget("addressed replacement byte budget"))?;
            Ok(writer.bytes)
        }
    }
}

fn array_size(values: &[Value], cap: usize) -> Result<usize> {
    let mut bytes = 2usize;
    if bytes > cap || values.len() > cap.saturating_sub(bytes).saturating_add(1) {
        return Err(Error::Budget("addressed replacement byte budget"));
    }
    for (index, value) in values.iter().enumerate() {
        if index != 0 {
            add(&mut bytes, 1, cap)?;
        }
        // The supplied row arrays are children of the accounting envelope,
        // so their entries begin at the same depth as Python's bounded helper.
        add(&mut bytes, json_size(value, cap - bytes, 2)?, cap)?;
    }
    Ok(bytes)
}

fn input_size(
    prior: &Value,
    replacement: &Value,
    relations: &[Value],
    endpoints: &[Value],
    binding: &Value,
    entities: &Value,
    relation_registry: &Value,
    current: &AddressedNormalizationContext<'_>,
    cap: usize,
) -> Result<usize> {
    if cap < 8 {
        return Err(Error::Budget("addressed replacement byte budget"));
    }
    // The top-level array contributes two brackets and six separators.
    let mut total = 2 + 6;
    for value in [prior, replacement] {
        add(&mut total, json_size(value, cap - total, 1)?, cap)?;
    }
    add(&mut total, array_size(relations, cap - total)?, cap)?;
    add(&mut total, array_size(endpoints, cap - total)?, cap)?;
    for value in [binding, entities, relation_registry] {
        add(&mut total, json_size(value, cap - total, 1)?, cap)?;
    }
    for raw in [
        current.descriptor_bytes,
        current.entity_registry_bytes,
        current.relation_registry_bytes,
    ] {
        add(&mut total, raw.len(), cap)?;
    }
    Ok(total)
}

fn text(value: Option<&Value>) -> Option<&str> {
    value?
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn row_identity(row: &Value) -> Result<(&str, &str, &str)> {
    let source =
        text(row.get("source_graph")).ok_or(Error::Invalid("addressed row source graph"))?;
    let id = text(row.get("id")).ok_or(Error::Invalid("addressed row normalized ID"))?;
    let native = text(row.get("native_id")).ok_or(Error::Invalid("addressed row native ID"))?;
    if !id.starts_with(&format!("{source}:")) || id.len() == source.len() + 1 {
        return Err(Error::Invalid("addressed row normalized/source identity"));
    }
    Ok((source, id, native))
}

fn payload_id_matches(payload: &Value, native: &str, relation: bool) -> bool {
    let fields: &[&str] = if relation {
        &["edge_id", "id"]
    } else {
        &["node_id", "id", "path"]
    };
    fields
        .iter()
        .any(|field| text(payload.get(field)) == Some(native))
}

fn verify_source_row(row: &Value, relation: bool, input_cap: usize) -> Result<()> {
    let (_, id, native) = row_identity(row)?;
    let payload = row
        .pointer("/source_record/payload")
        .filter(|value| value.is_object())
        .ok_or(Error::Invalid("addressed row source payload"))?;
    let source_digest = stable_digest(payload)?;
    if row.pointer("/source_record/digest").and_then(Value::as_str) != Some(source_digest.as_str())
    {
        return Err(Error::Invalid("addressed row corrupt source digest"));
    }
    if !payload_id_matches(payload, native, relation) {
        return Err(Error::Invalid(
            "addressed row native identity differs from source payload",
        ));
    }
    let revision = text(row.get("content_revision"))
        .ok_or(Error::Invalid("addressed row content revision absent"))?;
    let mut stamped = row.clone();
    stamp_addressed_candidate_content_revision(&mut stamped, input_cap)?;
    if stamped.get("content_revision").and_then(Value::as_str) != Some(revision) {
        return Err(Error::Invalid("addressed row corrupt content revision"));
    }
    Ok(())
}

fn truthy(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) | Some(Value::Bool(false)) => false,
        Some(Value::String(value)) => !value.is_empty(),
        Some(Value::Number(value)) => value.as_f64().is_some_and(|value| value != 0.0),
        Some(Value::Array(value)) => !value.is_empty(),
        Some(Value::Object(value)) => !value.is_empty(),
        _ => true,
    }
}

fn valid_dossier_ref(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("tos.") else {
        return false;
    };
    let mut segment_has_text = false;
    for byte in rest.bytes() {
        if byte.is_ascii_lowercase() || byte.is_ascii_digit() {
            segment_has_text = true;
        } else if matches!(byte, b'.' | b'-') && segment_has_text {
            segment_has_text = false;
        } else {
            return false;
        }
    }
    segment_has_text
}

fn source_dossier_candidate(
    item: &Value,
    source_graph: &str,
    dossier_kinds: &BTreeSet<String>,
) -> Option<String> {
    if !matches!(source_graph, "source-navigation" | "source-claims") {
        return None;
    }
    let properties = item.get("properties").filter(|value| value.is_object());
    let mut kind = text(item.get("node_kind"));
    if kind == Some("identity") {
        let source = properties.and_then(|value| value.get("source_record"));
        kind = text(properties.and_then(|value| value.get("identity_kind")))
            .or_else(|| text(properties.and_then(|value| value.get("identity_type"))))
            .or_else(|| text(properties.and_then(|value| value.get("record_type"))))
            .or_else(|| text(source.and_then(|value| value.get("record_type"))));
    }
    if !kind.is_some_and(|kind| dossier_kinds.contains(kind)) {
        return None;
    }
    let source_record = properties.and_then(|value| value.get("source_record"));
    let candidates: Vec<Option<&Value>> = if source_graph == "source-navigation" {
        vec![item.get("node_id")]
    } else {
        vec![
            properties.and_then(|value| value.get("identity_ref")),
            source_record.and_then(|value| value.get("record_id")),
            source_record.and_then(|value| value.get("composite_id")),
            source_record.and_then(|value| value.get("artifact_id")),
        ]
    };
    candidates
        .into_iter()
        .flatten()
        .filter_map(|candidate| text(Some(candidate)))
        .find(|candidate| valid_dossier_ref(candidate))
        .map(str::to_owned)
}

fn selected_owner_inputs(
    entity_value: &Value,
    relation_value: &Value,
    current: &AddressedNormalizationContext<'_>,
    max_input_bytes: usize,
    max_output_bytes: usize,
) -> Result<(KnowledgeRegistry, Value, QueryVocabulary)> {
    let selected_entities = SourceRow::parse_addressed_candidate(
        current.entity_registry_bytes,
        max_input_bytes,
        max_output_bytes,
    )?
    .value()
    .clone();
    let selected_relations = SourceRow::parse_addressed_candidate(
        current.relation_registry_bytes,
        max_input_bytes,
        max_output_bytes,
    )?
    .value()
    .clone();
    if &selected_entities != entity_value || &selected_relations != relation_value {
        return Err(Error::Invalid(
            "addressed registry values differ from current normalization bytes",
        ));
    }
    let descriptor = SourceRow::parse_addressed_candidate(
        current.descriptor_bytes,
        max_input_bytes,
        max_output_bytes,
    )?
    .value()
    .clone();
    let registry = KnowledgeRegistry::parse(
        current.entity_registry_bytes,
        current.relation_registry_bytes,
    )?;
    let mut adapters = descriptor
        .get("sources")
        .and_then(Value::as_array)
        .ok_or(Error::Invalid("addressed source registrations"))?
        .iter()
        .filter_map(|source| source.get("adapter_profile").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    if let Some(extension) = descriptor
        .get("extension_adapter_profile")
        .and_then(Value::as_str)
    {
        adapters.insert(extension);
    }
    if adapters
        .iter()
        .any(|adapter| !NATIVE_KNOWLEDGE_ADAPTER_PROFILES.contains(adapter))
    {
        return Err(Error::Invalid("addressed unsupported selected adapter"));
    }
    let vocabulary = QueryVocabulary::parse(
        current.descriptor_bytes,
        &adapters.into_iter().collect::<Vec<_>>(),
    )?;
    Ok((registry, descriptor, vocabulary))
}

fn row_title(row: &Value) -> Result<Value> {
    row.pointer("/display/title")
        .filter(|title| title.is_object())
        .cloned()
        .ok_or(Error::Invalid("addressed endpoint title"))
}

fn attach_readable_context(
    compiler: Option<&ReadableContextCompiler>,
    row: &mut Value,
    output_cap: usize,
) -> Result<()> {
    let Some(compiler) = compiler else {
        row.as_object_mut()
            .ok_or(Error::Invalid("addressed normalized row object"))?
            .remove("readable_context");
        return Ok(());
    };
    let raw = serde_json::to_vec(row).map_err(|_| Error::Invalid("addressed normalized JSON"))?;
    if raw.len() > output_cap {
        return Err(Error::Budget("addressed normalized row bytes"));
    }
    match compiler.compile(&raw, Some(&raw))? {
        ReadableContextCarrier::Absent => {
            row.as_object_mut()
                .ok_or(Error::Invalid("addressed normalized row object"))?
                .remove("readable_context");
        }
        ReadableContextCarrier::Sidecar(sidecar) => {
            row["readable_context"] = serde_json::from_slice(&sidecar)
                .map_err(|_| Error::Invalid("addressed readable context sidecar"))?;
        }
    }
    stamp_addressed_candidate_content_revision(row, output_cap)
}

fn reserve_output(total: &mut usize, row: &Value, cap: usize) -> Result<()> {
    add(total, json_size(row, cap - *total, 0)?, cap)
}

/// Recompute one direct source carrier and only the supplied incident rows.
/// Registry binding digests and row revisions are self-consistency checks, not
/// independently admitted provenance. No source transition or publication is
/// inferred from this neighborhood-local result.
#[allow(clippy::too_many_arguments)]
pub fn replace_direct_carrier_candidate(
    prior_carrier: &Value,
    replacement_record: &Value,
    incident_relations: &[Value],
    endpoint_nodes: &[Value],
    normalization_binding: &Value,
    current_normalizer: &AddressedNormalizationContext<'_>,
    entity_registry: &Value,
    relation_registry: &Value,
    limits: AddressedReplacementLimits,
) -> Result<Value> {
    limits.validate()?;
    if incident_relations.len() > limits.max_relations
        || endpoint_nodes.len() > limits.max_endpoints
    {
        return Err(Error::Budget("addressed replacement row count"));
    }
    if !prior_carrier.is_object()
        || !replacement_record.is_object()
        || !normalization_binding.is_object()
        || !entity_registry.is_object()
        || !relation_registry.is_object()
    {
        return Err(Error::Invalid("addressed explicit JSON objects required"));
    }
    let input_bytes = input_size(
        prior_carrier,
        replacement_record,
        incident_relations,
        endpoint_nodes,
        normalization_binding,
        entity_registry,
        relation_registry,
        current_normalizer,
        limits.max_input_bytes,
    )?;
    let (source_graph, carrier_id, carrier_native) = row_identity(prior_carrier)?;
    if !ADDRESS_UPDATE_SOURCE_GRAPHS.contains(&source_graph) {
        return Err(Error::Invalid(
            "addressed update supports only direct source carriers",
        ));
    }
    let payload_ids = ["node_id", "id", "path"]
        .iter()
        .filter_map(|field| text(replacement_record.get(*field)))
        .collect::<BTreeSet<_>>();
    if !payload_ids.contains(carrier_native) {
        return Err(Error::Invalid("addressed replacement native ID differs"));
    }
    verify_source_row(prior_carrier, false, limits.max_input_bytes)?;
    let processor_digest = Digest256::from_hex(current_normalizer.processor_digest)
        .map_err(|_| Error::Invalid("addressed current processor digest"))?;
    let (registry, descriptor, vocabulary) = selected_owner_inputs(
        entity_registry,
        relation_registry,
        current_normalizer,
        limits.max_input_bytes,
        limits.max_output_bytes,
    )?;
    if source_graph == "source-claims" || source_graph == "source-navigation" {
        let dossier_kinds = descriptor
            .pointer("/identity/source_dossier_kinds")
            .and_then(Value::as_array)
            .ok_or(Error::Invalid("addressed dossier kinds"))?
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        if source_dossier_candidate(replacement_record, source_graph, &dossier_kinds).as_deref()
            != text(prior_carrier.get("source_dossier_ref"))
        {
            return Err(Error::Invalid("addressed source dossier binding changed"));
        }
        if matches!(
            text(replacement_record.get("node_kind")),
            Some("claim" | "record-version")
        ) || matches!(
            text(prior_carrier.get("kind_id")),
            Some("claim" | "record-version")
        ) {
            return Err(Error::Invalid(
                "addressed claim and record-version carrier requires owner assembly",
            ));
        }
    }
    if !prior_carrier.get("semantics").is_some_and(Value::is_object) {
        return Err(Error::Invalid("addressed prior carrier semantics"));
    }
    if truthy(prior_carrier.pointer("/semantics/claim")) {
        return Err(Error::Invalid(
            "addressed claim-enriched carrier requires owner assembly",
        ));
    }

    let source_graphs = vocabulary.registered_source_ids().to_vec();
    let base = KnowledgeBaseNormalizer::new_for_addressed_candidate(
        &registry,
        current_normalizer.entity_registry_bytes,
        current_normalizer.relation_registry_bytes,
        &vocabulary,
        current_normalizer.descriptor_bytes,
        &source_graphs,
        BaseNormalizationLimits {
            max_registry_bytes: 4 * 1024 * 1024,
            max_output_bytes: limits.max_output_bytes,
        },
    )?;
    let expected_binding =
        crate::native_normalization_binding::NativeNormalizationBinding::from_current_inputs(
            processor_digest,
            &registry,
            current_normalizer.descriptor_bytes,
            current_normalizer.entity_registry_bytes,
            current_normalizer.relation_registry_bytes,
        )
        .to_value();
    if &expected_binding != normalization_binding {
        return Err(Error::Invalid("addressed normalization binding changed"));
    }
    let context = ReadableContextCompiler::from_selected_registry_bytes_for_addressed_candidate(
        current_normalizer.entity_registry_bytes,
        &registry.entity_sha256,
        ReadableContextLimits {
            max_input_bytes: limits.max_output_bytes,
            max_work_bytes: (limits.max_output_bytes as u64).saturating_mul(32),
        },
    )?;
    let identity_id = {
        let prefix = format!("{source_graph}:");
        let tail = carrier_id
            .strip_prefix(&prefix)
            .ok_or(Error::Invalid("addressed carrier normalized ID prefix"))?;
        (tail != carrier_native).then_some(tail)
    };
    let replacement_bytes = serde_json::to_vec(replacement_record)
        .map_err(|_| Error::Invalid("addressed replacement JSON"))?;
    let replacement_base = match source_graph {
        "source-navigation" => {
            let mut normalizer = NavigationNodeNormalizer::new_for_addressed_candidate(
                &registry,
                current_normalizer.entity_registry_bytes,
                &vocabulary,
                current_normalizer.descriptor_bytes,
                NavigationNodeLimits {
                    max_raw_bytes: limits.max_input_bytes,
                    max_output_bytes: limits.max_output_bytes,
                    max_ancestor_cache_bytes: 4 * 1024 * 1024,
                },
            )?;
            let payload_sha256 = Digest256::of_bytes(&replacement_bytes).to_hex();
            let raw = SeekRow {
                id: carrier_native.to_owned(),
                source_graph: source_graph.to_owned(),
                source_order: None,
                payload: replacement_bytes,
                payload_sha256,
            };
            let normalized = normalizer.normalize_addressed_candidate_node(&raw)?;
            normalized.value().clone()
        }
        "source-claims" => {
            let dossier = text(prior_carrier.get("source_dossier_ref"));
            let normalizer = ClaimNormalizer::new_for_addressed_candidate(
                &registry,
                current_normalizer.entity_registry_bytes,
                current_normalizer.relation_registry_bytes,
                &vocabulary,
                current_normalizer.descriptor_bytes,
                ClaimNormalizeLimits {
                    max_raw_bytes: limits.max_input_bytes,
                    max_output_bytes: limits.max_output_bytes,
                    max_page_rows: 1,
                    max_contexts: 4096,
                    max_work_bytes: (limits.max_input_bytes as u64).saturating_mul(32),
                },
            )?;
            let raw = SeekRow {
                id: carrier_native.to_owned(),
                source_graph: source_graph.to_owned(),
                source_order: None,
                payload_sha256: Digest256::of_bytes(&replacement_bytes).to_hex(),
                payload: replacement_bytes,
            };
            normalizer.normalize_addressed_candidate_node(&raw, dossier)?
        }
        _ => {
            let replacement_source = SourceRow::parse_addressed_candidate(
                &replacement_bytes,
                limits.max_input_bytes,
                limits.max_output_bytes,
            )?;
            base.normalize_node(
                &replacement_source,
                source_graph,
                source_graph == "canon",
                BaseNodeOverrides {
                    native_id: Some(carrier_native),
                    identity_id,
                    kind_id: None,
                },
            )?
        }
    };
    if text(replacement_base.get("id")) != Some(carrier_id) {
        return Err(Error::Invalid(
            "addressed replacement changed normalized address",
        ));
    }

    let mut nodes = BTreeMap::new();
    nodes.insert(carrier_id.to_owned(), &replacement_base);
    for endpoint in endpoint_nodes {
        let (_, id, _) = row_identity(endpoint)?;
        if nodes.contains_key(id) {
            return Err(Error::Invalid("addressed duplicate supplied endpoint row"));
        }
        verify_source_row(endpoint, false, limits.max_input_bytes)?;
        nodes.insert(id.to_owned(), endpoint);
    }
    let mut seen_relations = BTreeSet::new();
    let mut updated_relations = Vec::new();
    let mut output_bytes = 0usize;
    for relation in incident_relations {
        let (relation_graph, relation_id, relation_native) = row_identity(relation)?;
        let payload = relation
            .pointer("/source_record/payload")
            .filter(|value| value.is_object())
            .ok_or(Error::Invalid("addressed incident relation source payload"))?;
        if !seen_relations.insert(relation_id.to_owned()) {
            return Err(Error::Invalid("addressed duplicate incident relation"));
        }
        let from = text(relation.get("from_id"))
            .ok_or(Error::Invalid("addressed relation from endpoint"))?;
        let to =
            text(relation.get("to_id")).ok_or(Error::Invalid("addressed relation to endpoint"))?;
        if from != carrier_id && to != carrier_id {
            return Err(Error::Invalid(
                "addressed supplied relation is not incident",
            ));
        }
        if !nodes.contains_key(from) || !nodes.contains_key(to) {
            return Err(Error::Invalid(
                "addressed incident relation lacks endpoint row",
            ));
        }
        verify_source_row(relation, true, limits.max_input_bytes)?;
        let source_row_bytes = serde_json::to_vec(payload)
            .map_err(|_| Error::Invalid("addressed relation source JSON"))?;
        let source = SourceRow::parse_addressed_candidate(
            &source_row_bytes,
            limits.max_input_bytes,
            limits.max_output_bytes,
        )?;
        let old_contexts = relation
            .pointer("/semantics/assertion_contexts")
            .map(|value| {
                value
                    .as_array()
                    .ok_or(Error::Invalid("addressed relation assertion contexts"))
            })
            .transpose()?;
        let direct_context =
            direct_assertion_context_addressed_candidate(&source, limits.max_output_bytes)?;
        if let Some(old_contexts) = old_contexts {
            for old in old_contexts {
                let direct_matches = match &direct_context {
                    Some(direct) => {
                        validation_digest(old, limits.max_input_bytes)?
                            == validation_digest(direct, limits.max_input_bytes)?
                    }
                    None => false,
                };
                if !direct_matches {
                    return Err(Error::Invalid(
                        "addressed relation has referenced claim context",
                    ));
                }
            }
        }
        let prefix = format!("{relation_graph}:");
        let relation_tail = relation_id
            .strip_prefix(&prefix)
            .ok_or(Error::Invalid("addressed relation normalized ID prefix"))?;
        let relation_identity = (relation_tail != relation_native).then_some(relation_tail);
        let left = row_title(
            nodes
                .get(from)
                .ok_or(Error::Invalid("addressed from node"))?,
        )?;
        let right = row_title(nodes.get(to).ok_or(Error::Invalid("addressed to node"))?)?;
        let mut normalized = base.normalize_addressed_relation(
            &source,
            relation_graph,
            relation_native,
            relation_identity,
            &left,
            &right,
            if relation_graph == "canon" {
                "canon"
            } else {
                "derived-export"
            },
        )?;
        if text(normalized.get("id")) != Some(relation_id)
            || text(normalized.get("from_id")) != Some(from)
            || text(normalized.get("to_id")) != Some(to)
        {
            return Err(Error::Invalid(
                "addressed relation changed normalized address/endpoints",
            ));
        }
        attach_readable_context(context.as_ref(), &mut normalized, limits.max_output_bytes)?;
        reserve_output(&mut output_bytes, &normalized, limits.max_output_bytes)?;
        updated_relations.push(normalized);
    }

    let mut inherited_views = updated_relations
        .iter()
        .flat_map(|relation| {
            relation
                .get("view_ids")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_owned)
        })
        .collect::<BTreeSet<_>>();
    inherited_views.extend(
        replacement_base
            .get("view_ids")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned),
    );
    let mut replacement_node = replacement_base;
    replacement_node["view_ids"] = json!(inherited_views.into_iter().collect::<Vec<_>>());
    attach_readable_context(
        context.as_ref(),
        &mut replacement_node,
        limits.max_output_bytes,
    )?;
    if text(replacement_node.get("id")) != Some(carrier_id) {
        return Err(Error::Invalid(
            "addressed finalized carrier changed normalized address",
        ));
    }
    reserve_output(
        &mut output_bytes,
        &replacement_node,
        limits.max_output_bytes,
    )?;
    for row in std::iter::once(&replacement_node).chain(updated_relations.iter()) {
        verify_final_revision(row, limits.max_output_bytes)?;
    }
    let endpoint_ids = endpoint_nodes
        .iter()
        .map(|row| {
            row.get("id")
                .cloned()
                .ok_or(Error::Invalid("addressed endpoint ID"))
        })
        .collect::<Result<Vec<_>>>()?;
    let relation_ids = incident_relations
        .iter()
        .map(|row| {
            row.get("id")
                .cloned()
                .ok_or(Error::Invalid("addressed relation ID"))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(json!({
        "schema":"tos_direct_carrier_replacement_candidate_v1",
        "node":replacement_node,
        "relations":updated_relations,
        "scope":{
            "kind":"supplied-neighborhood-local-only",
            "carrier_id":carrier_id,
            "supplied_relation_ids":relation_ids,
            "supplied_endpoint_ids":endpoint_ids,
            "complete_incidence_verified":false,
            "source_transition_verified":false,
            "global_semantics_validated":false,
            "catalog_updated":false,
            "publication_current":false,
            "published":false
        },
        "accounting":{
            "input_bytes":input_bytes,
            "output_row_bytes":output_bytes,
            "relations":incident_relations.len(),
            "endpoints":endpoint_nodes.len()
        },
        "is_semantic_acceptance":false
    }))
}

fn verify_final_revision(row: &Value, cap: usize) -> Result<()> {
    let revision = text(row.get("content_revision"))
        .ok_or(Error::Invalid("addressed result content revision absent"))?;
    let mut stamped = row.clone();
    stamp_addressed_candidate_content_revision(&mut stamped, cap)?;
    if stamped.get("content_revision").and_then(Value::as_str) != Some(revision) {
        return Err(Error::Invalid("addressed result content revision mismatch"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[derive(Clone)]
    struct Fixture {
        prior: Value,
        replacement: Value,
        relation: Value,
        endpoint: Value,
        binding: Value,
        processor_digest: String,
        descriptor_bytes: Vec<u8>,
        entity_bytes: Vec<u8>,
        relation_bytes: Vec<u8>,
        entities: Value,
        relations: Value,
    }

    impl Fixture {
        fn current_normalizer(&self) -> AddressedNormalizationContext<'_> {
            AddressedNormalizationContext {
                processor_digest: &self.processor_digest,
                descriptor_bytes: &self.descriptor_bytes,
                entity_registry_bytes: &self.entity_bytes,
                relation_registry_bytes: &self.relation_bytes,
            }
        }
    }

    fn fixture() -> Fixture {
        let entities: Value = serde_json::from_slice(ENTITY_REGISTRY_REF_BYTES).unwrap();
        let relations: Value = serde_json::from_slice(RELATION_REGISTRY_REF_BYTES).unwrap();
        let entity_bytes = ENTITY_REGISTRY_REF_BYTES.to_vec();
        let relation_bytes = RELATION_REGISTRY_REF_BYTES.to_vec();
        let registry = KnowledgeRegistry::parse(&entity_bytes, &relation_bytes).unwrap();
        let descriptor_bytes = QUERY_VOCABULARY.to_vec();
        let vocabulary =
            QueryVocabulary::parse(&descriptor_bytes, NATIVE_KNOWLEDGE_ADAPTER_PROFILES).unwrap();
        let sources = vocabulary.registered_source_ids().to_vec();
        let normalizer = KnowledgeBaseNormalizer::new_for_addressed_candidate(
            &registry,
            &entity_bytes,
            &relation_bytes,
            &vocabulary,
            &descriptor_bytes,
            &sources,
            BaseNormalizationLimits {
                max_registry_bytes: 4 * 1024 * 1024,
                max_output_bytes: DEFAULT_OUTPUT_BYTES,
            },
        )
        .unwrap();
        let make_node = |node_id: &str, label: &str| {
            let raw = json!({
                "node_id":node_id,
                "node_kind":"concept",
                "label":label,
                "source_ref":"fixture:philosophy",
                "properties":{"summary":format!("{label} summary")}
            });
            let bytes = serde_json::to_vec(&raw).unwrap();
            let source = SourceRow::parse_addressed_candidate(
                &bytes,
                DEFAULT_INPUT_BYTES,
                DEFAULT_OUTPUT_BYTES,
            )
            .unwrap();
            normalizer
                .normalize_node(&source, "philosophy", false, BaseNodeOverrides::default())
                .unwrap()
        };
        let prior = make_node("a", "Alpha");
        let endpoint = make_node("b", "Beta");
        let replacement = json!({
            "node_id":"a",
            "node_kind":"concept",
            "label":"Changed Alpha",
            "source_ref":"fixture:philosophy",
            "properties":{"summary":"Changed Alpha summary"}
        });
        let edge = json!({
            "edge_id":"edge-a-b",
            "from_id":"a",
            "to_id":"b",
            "predicate_id":"develops_concept",
            "source_ref":"fixture:philosophy",
            "view_ids":["inherited-only"],
            "properties":{}
        });
        let edge_bytes = serde_json::to_vec(&edge).unwrap();
        let source = SourceRow::parse_addressed_candidate(
            &edge_bytes,
            DEFAULT_INPUT_BYTES,
            DEFAULT_OUTPUT_BYTES,
        )
        .unwrap();
        let relation = normalizer
            .normalize_addressed_relation(
                &source,
                "philosophy",
                "edge-a-b",
                None,
                &row_title(&prior).unwrap(),
                &row_title(&endpoint).unwrap(),
                "derived-export",
            )
            .unwrap();
        let processor_digest = Digest256::of_bytes(b"fixture native processor").to_hex();
        let binding =
            crate::native_normalization_binding::NativeNormalizationBinding::from_current_inputs(
                Digest256::from_hex(&processor_digest).unwrap(),
                &registry,
                &descriptor_bytes,
                &entity_bytes,
                &relation_bytes,
            )
            .to_value();
        Fixture {
            prior,
            replacement,
            relation,
            endpoint,
            binding,
            processor_digest,
            descriptor_bytes,
            entity_bytes,
            relation_bytes,
            entities,
            relations,
        }
    }

    const ENTITY_REGISTRY_REF_BYTES: &[u8] =
        include_bytes!("../../../../ToS/doctrine/semantic-interchange/entity-types.v1.json");
    const RELATION_REGISTRY_REF_BYTES: &[u8] =
        include_bytes!("../../../../ToS/doctrine/semantic-interchange/relation-types.v1.json");

    fn run(fixture: &Fixture, limits: AddressedReplacementLimits) -> Result<Value> {
        replace_direct_carrier_candidate(
            &fixture.prior,
            &fixture.replacement,
            std::slice::from_ref(&fixture.relation),
            std::slice::from_ref(&fixture.endpoint),
            &fixture.binding,
            &fixture.current_normalizer(),
            &fixture.entities,
            &fixture.relations,
            limits,
        )
    }

    #[test]
    fn bounded_candidate_recomputes_only_supplied_neighborhood() {
        let fixture = fixture();
        let candidate = run(&fixture, AddressedReplacementLimits::default()).unwrap();
        let framed = serde_json::to_vec(&json!([
            fixture.prior.clone(),
            fixture.replacement.clone(),
            [fixture.relation.clone()],
            [fixture.endpoint.clone()],
            fixture.binding.clone(),
            fixture.entities.clone(),
            fixture.relations.clone()
        ]))
        .unwrap();
        let exact_input_bytes = framed.len()
            + fixture.descriptor_bytes.len()
            + fixture.entity_bytes.len()
            + fixture.relation_bytes.len();
        assert_eq!(candidate["accounting"]["input_bytes"], exact_input_bytes);
        assert_eq!(candidate["node"]["id"], "philosophy:a");
        assert_eq!(
            candidate["node"]["display"]["title"]["default"],
            "Changed Alpha"
        );
        assert!(
            candidate["node"]["view_ids"]
                .as_array()
                .unwrap()
                .contains(&json!("inherited-only"))
        );
        assert!(
            candidate["relations"][0]["display"]["statement"]["default"]
                .as_str()
                .unwrap()
                .contains("Changed Alpha")
        );
        assert_eq!(
            candidate["scope"]["kind"],
            "supplied-neighborhood-local-only"
        );
        for field in [
            "complete_incidence_verified",
            "source_transition_verified",
            "global_semantics_validated",
            "catalog_updated",
            "publication_current",
            "published",
        ] {
            assert_eq!(candidate["scope"][field], false);
        }
        assert_eq!(candidate["is_semantic_acceptance"], false);
        verify_final_revision(&candidate["node"], DEFAULT_OUTPUT_BYTES).unwrap();
        verify_final_revision(&candidate["relations"][0], DEFAULT_OUTPUT_BYTES).unwrap();
    }

    #[test]
    fn candidate_rejects_corrupt_source_and_content_revisions_and_missing_endpoint() {
        let mut fixture = fixture();
        fixture.relation["source_record"]["payload"]["unknown"] = json!(true);
        assert!(run(&fixture, AddressedReplacementLimits::default()).is_err());

        let mut fixture = fixture();
        fixture.prior["display"]["title"]["default"] = json!("changed without revision");
        assert!(run(&fixture, AddressedReplacementLimits::default()).is_err());

        let fixture = fixture();
        assert!(
            replace_direct_carrier_candidate(
                &fixture.prior,
                &fixture.replacement,
                std::slice::from_ref(&fixture.relation),
                &[],
                &fixture.binding,
                &fixture.current_normalizer(),
                &fixture.entities,
                &fixture.relations,
                AddressedReplacementLimits::default(),
            )
            .is_err()
        );
    }

    #[test]
    fn exact_normalization_binding_is_recomputed() {
        let fixture = fixture();
        for field in [
            "processor_digest",
            "configuration_digest",
            "entity_registry_digest",
            "relation_registry_digest",
        ] {
            let mut changed = fixture.clone();
            changed.binding[field] = json!("0".repeat(64));
            assert!(run(&changed, AddressedReplacementLimits::default()).is_err());
        }
        let mut changed_registry = fixture;
        changed_registry.entities["types"][0]["labels"]["en"] = json!("changed");
        assert!(run(&changed_registry, AddressedReplacementLimits::default()).is_err());
    }

    #[test]
    fn current_normalizer_processor_and_exact_descriptor_are_required() {
        let fixture = fixture();
        let mut changed_processor = fixture.clone();
        changed_processor.processor_digest = "1".repeat(64);
        assert!(run(&changed_processor, AddressedReplacementLimits::default()).is_err());

        let mut changed_descriptor = fixture.clone();
        changed_descriptor.descriptor_bytes.push(b' ');
        assert!(run(&changed_descriptor, AddressedReplacementLimits::default()).is_err());

        let mut changed_registry_bytes = fixture;
        changed_registry_bytes.entity_bytes.push(b' ');
        assert!(
            run(
                &changed_registry_bytes,
                AddressedReplacementLimits::default()
            )
            .is_err()
        );
    }

    #[test]
    fn empty_incidence_stays_unverified_and_candidate_row_cap_is_isolated() {
        let fixture = fixture();
        let candidate = replace_direct_carrier_candidate(
            &fixture.prior,
            &fixture.replacement,
            &[],
            &[],
            &fixture.binding,
            &fixture.current_normalizer(),
            &fixture.entities,
            &fixture.relations,
            AddressedReplacementLimits::default(),
        )
        .unwrap();
        assert!(candidate["relations"].as_array().unwrap().is_empty());
        assert_eq!(candidate["scope"]["complete_incidence_verified"], false);
        assert_eq!(candidate["scope"]["published"], false);
        assert!(
            !candidate["node"]["view_ids"]
                .as_array()
                .unwrap()
                .contains(&json!("inherited-only"))
        );

        let row = br#"{"node_id":"bounded"}"#;
        const CANDIDATE_ROW_BUDGET: usize = 16 * 1024 * 1024;
        assert!(SourceRow::parse(row, CANDIDATE_ROW_BUDGET).is_err());
        assert!(
            SourceRow::parse_addressed_candidate(row, CANDIDATE_ROW_BUDGET, CANDIDATE_ROW_BUDGET,)
                .is_ok()
        );
    }

    #[test]
    fn exact_accounting_boundaries_and_detached_inputs_hold() {
        let fixture = fixture();
        let before = (
            fixture.prior.clone(),
            fixture.replacement.clone(),
            fixture.relation.clone(),
            fixture.endpoint.clone(),
            fixture.entities.clone(),
            fixture.relations.clone(),
        );
        let mut result = run(&fixture, AddressedReplacementLimits::default()).unwrap();
        let accounting = &result["accounting"];
        let exact = AddressedReplacementLimits {
            max_relations: accounting["relations"].as_u64().unwrap() as usize,
            max_endpoints: accounting["endpoints"].as_u64().unwrap() as usize,
            max_input_bytes: accounting["input_bytes"].as_u64().unwrap() as usize,
            max_output_bytes: accounting["output_row_bytes"].as_u64().unwrap() as usize,
        };
        assert_eq!(run(&fixture, exact).unwrap(), result);
        assert!(
            run(
                &fixture,
                AddressedReplacementLimits {
                    max_relations: exact.max_relations - 1,
                    ..exact
                }
            )
            .is_err()
        );
        assert!(
            run(
                &fixture,
                AddressedReplacementLimits {
                    max_endpoints: exact.max_endpoints - 1,
                    ..exact
                }
            )
            .is_err()
        );
        assert!(
            run(
                &fixture,
                AddressedReplacementLimits {
                    max_input_bytes: exact.max_input_bytes - 1,
                    ..exact
                }
            )
            .is_err()
        );
        let below_output = AddressedReplacementLimits {
            max_output_bytes: exact.max_output_bytes - 1,
            ..exact
        };
        assert!(run(&fixture, below_output).is_err());

        let mut larger_prior = fixture.clone();
        larger_prior.prior["input_only_padding"] = json!("x".repeat(64 * 1024));
        stamp_addressed_candidate_content_revision(&mut larger_prior.prior, DEFAULT_INPUT_BYTES)
            .unwrap();
        let independent_caps = AddressedReplacementLimits {
            max_output_bytes: exact.max_output_bytes,
            ..AddressedReplacementLimits::default()
        };
        assert!(run(&larger_prior, independent_caps).is_ok());
        assert_eq!(
            before,
            (
                fixture.prior.clone(),
                fixture.replacement.clone(),
                fixture.relation.clone(),
                fixture.endpoint.clone(),
                fixture.entities.clone(),
                fixture.relations.clone(),
            )
        );
        result["node"]["source_record"]["payload"]["properties"]["summary"] =
            json!("consumer mutation");
        assert_ne!(
            result["node"]["source_record"]["payload"]["properties"]["summary"],
            fixture.replacement["properties"]["summary"]
        );
        assert_eq!(
            fixture.replacement["properties"]["summary"],
            "Changed Alpha summary"
        );
    }
}
