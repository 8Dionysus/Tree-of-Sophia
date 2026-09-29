//! Explicit, bounded candidate closure. Selection, complete incidence, source
//! admission and publication belong to the caller; no global cut is invented.
#[path = "source_claim_reference_carriers.rs"]
mod reference_carriers;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use tos_compiler::knowledge_normalization::{SourceRow, stamp_content_revision};
use tos_compiler::knowledge_source_claims::{
    ClaimNormalizeLimits, ClaimNormalizer, ordered_claim_node_material,
    ordered_claim_relation_material,
};
use tos_compiler::knowledge_stage::SeekRow;
use tos_compiler::{
    BaseNormalizationLimits, Error, KnowledgeBaseNormalizer, KnowledgeRegistry,
    NavigationNodeLimits, NavigationNodeNormalizer, QueryVocabulary, ReadableContextCarrier,
    ReadableContextCompiler, ReadableContextLimits, Result, ordered_readable_witness,
};
use tos_foundation::{CanonicalProfile, Digest256, JsonLimits, canonical_raw_bytes_v1};

pub struct CandidateNode {
    pub raw: SeekRow,
    /// Exact owner dossier handle, independently selected from declared fields.
    pub dossier_ref: Option<String>,
}
pub struct CandidateRelation {
    pub raw: SeekRow,
    pub identity_id: Option<String>,
}
pub struct RetainedCandidateNode {
    pub normalized: Value,
    /// Original ordered owner material, including declared adapter transforms.
    pub owner_material: Vec<u8>,
}
pub struct ClaimCandidateInput {
    pub nodes: Vec<CandidateNode>,
    pub relations: Vec<CandidateRelation>,
    pub retained_nodes: Vec<RetainedCandidateNode>,
    pub traces: Vec<SeekRow>,
    pub dossier_refs: Vec<String>,
    pub context_node_order: Vec<String>,
    pub normalization_binding: Value,
}
#[derive(Clone, Copy)]
pub struct ClaimCandidateLimits {
    pub max_nodes: usize,
    pub max_retained_nodes: usize,
    pub max_relations: usize,
    pub max_traces: usize,
    pub max_contexts: usize,
    pub max_row_bytes: usize,
    pub max_input_bytes: usize,
    pub max_output_bytes: usize,
}
#[derive(Clone, Copy)]
pub struct ClaimCandidateRegistries<'a> {
    pub entity_bytes: &'a [u8],
    pub relation_bytes: &'a [u8],
    pub descriptor_bytes: &'a [u8],
    pub vocabulary: &'a QueryVocabulary,
    /// Independently admitted current processor/configuration binding.
    pub expected_normalization_binding: &'a Value,
}
pub struct ClaimCandidateOutput {
    pub nodes: Vec<Value>,
    pub relations: Vec<Value>,
    pub input_bytes: usize,
    pub output_bytes: usize,
}
fn identifier<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.len() <= 4096)
        .ok_or(Error::Invalid("Claim candidate identifier"))
}
fn encode(value: &Value, cap: usize) -> Result<Vec<u8>> {
    struct Capped {
        bytes: Vec<u8>,
        cap: usize,
    }
    impl Write for Capped {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self
                .bytes
                .len()
                .checked_add(bytes.len())
                .is_none_or(|n| n > self.cap)
            {
                return Err(std::io::Error::other("Claim candidate row cap"));
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut writer = Capped {
        bytes: Vec::new(),
        cap,
    };
    serde_json::to_writer(&mut writer, value)
        .map_err(|_| Error::Budget("Claim candidate row bytes"))?;
    Ok(writer.bytes)
}
fn charge(total: &mut usize, bytes: usize, cap: usize) -> Result<()> {
    *total = total
        .checked_add(bytes)
        .filter(|n| *n <= cap)
        .ok_or(Error::Budget("Claim candidate aggregate bytes"))?;
    Ok(())
}
fn parsed(raw: &SeekRow, cap: usize) -> Result<Value> {
    if raw.source_order.is_some()
        || raw.id.is_empty()
        || raw.id.len() > 4096
        || raw.source_graph.is_empty()
        || raw.source_graph.len() > 4096
        || Digest256::of_bytes(&raw.payload).to_hex() != raw.payload_sha256
    {
        return Err(Error::Invalid("Claim candidate raw witness"));
    }
    Ok(SourceRow::parse(&raw.payload, cap)?.value().clone())
}
fn verify_content(node: &Value, cap: usize) -> Result<()> {
    let mut stamped = node.clone();
    stamp_content_revision(&mut stamped, cap)?;
    if stamped.get("content_revision") != node.get("content_revision") {
        return Err(Error::Invalid("Claim candidate retained content revision"));
    }
    Ok(())
}
fn validation_digest(value: &Value, cap: usize) -> Result<String> {
    let raw = encode(value, cap)?;
    let canonical = canonical_raw_bytes_v1(
        &raw,
        CanonicalProfile::SourceRecordDigestV1,
        JsonLimits::new(cap, 96, 1_000_000, 4096)
            .map_err(|_| Error::Budget("Claim syntax digest bounds"))?,
    )
    .map_err(|e| Error::Source(e.to_string()))?;
    Ok(Digest256::of_bytes(&canonical).to_hex())
}
fn validate_navigation_carriers(
    raw_claims: &BTreeMap<String, Value>,
    output_ids: &[String],
    registry: &Value,
    entities: &Value,
    cap: usize,
) -> Result<()> {
    let checked: Vec<_> = raw_claims
        .iter()
        .filter(|(native, raw)| {
            raw.get("node_kind").and_then(Value::as_str) != Some("claim")
                || output_ids
                    .iter()
                    .any(|id| id == &format!("source-claims:{native}"))
        })
        .map(|(_, raw)| raw)
        .collect();
    let empty = json!({});
    let endpoint = |identity: &Value| -> &Value {
        let Some(identity) = identity.as_str() else {
            return &empty;
        };
        let mut matches = checked.iter().copied().filter(|raw| {
            raw.get("node_kind").and_then(Value::as_str) == Some("identity")
                && raw
                    .pointer("/properties/identity_ref")
                    .and_then(Value::as_str)
                    == Some(identity)
        });
        let first = matches.next();
        if matches.next().is_none() {
            first.unwrap_or(&empty)
        } else {
            &empty
        }
    };
    for raw in &checked {
        let props = &raw["properties"];
        let Some(descriptor) = props.get("navigation_descriptor") else {
            continue;
        };
        let source = &props["source_claim"];
        let id = identifier(source, "claim_id")?;
        if raw.get("node_kind").and_then(Value::as_str) != Some("claim")
            || source
                .get("claim_version")
                .and_then(Value::as_u64)
                .is_none_or(|v| v < 1)
            || raw.get("node_id").and_then(Value::as_str) != Some(format!("claim:{id}").as_str())
            || props.get("claim_ref").and_then(Value::as_str) != Some(id)
            || raw.get("source_sha256").and_then(Value::as_str)
                != Some(validation_digest(source, cap)?.as_str())
            || !registry
                .get("claim_navigation_template")
                .is_some_and(Value::is_object)
        {
            return Err(Error::Invalid(
                "Claim candidate navigation source/template binding",
            ));
        }
        for field in [
            "claim_version",
            "predicate",
            "subject_ref",
            "object",
            "epistemic_status",
            "review_status",
            "qualifiers",
        ] {
            if validation_digest(props.get(field).unwrap_or(&Value::Null), cap)?
                != validation_digest(source.get(field).unwrap_or(&Value::Null), cap)?
            {
                return Err(Error::Invalid(
                    "Claim candidate navigation source fields differ",
                ));
            }
        }
        let mut target = endpoint(&source["object"]);
        if source["object"].is_object() {
            let mut literals = checked.iter().copied().filter(|node| {
                node.get("node_kind").and_then(Value::as_str) == Some("literal")
                    && node
                        .pointer("/properties/claim_ref")
                        .and_then(Value::as_str)
                        == Some(id)
            });
            if let Some(literal) = literals.next() {
                if literals.next().is_none()
                    && literal.get("source_ref") == raw.get("source_ref")
                    && literal.get("source_line") == raw.get("source_line")
                {
                    target = literal;
                }
            }
        }
        let expected = tos_compiler::source_bibliographic::supplied_claim_navigation_descriptor(
            source,
            endpoint(&source["subject_ref"]),
            target,
            registry,
            entities,
            cap,
        )?
        .ok_or(Error::Invalid(
            "Claim candidate navigation template unavailable",
        ))?;
        if validation_digest(descriptor, cap)? != validation_digest(&expected, cap)? {
            return Err(Error::Invalid(
                "Claim candidate navigation descriptor differs from source syntax",
            ));
        }
    }
    Ok(())
}
pub(super) fn declared_dossier(raw: &Value, graph: &str) -> Option<String> {
    let props = &raw["properties"];
    let kind = raw.get("node_kind").and_then(Value::as_str)?;
    let kind = if kind == "identity" {
        [
            props.get("identity_kind"),
            props.get("identity_type"),
            props.get("record_type"),
            props.pointer("/source_record/record_type"),
        ]
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .find(|s| !s.trim().is_empty())?
        .trim()
    } else {
        kind.trim()
    };
    if !["work", "expression", "edition", "item", "file", "link"].contains(&kind) {
        return None;
    }
    let choices = if graph == "source-navigation" {
        vec![raw.get("node_id")]
    } else {
        vec![
            props.get("identity_ref"),
            props.pointer("/source_record/record_id"),
            props.pointer("/source_record/composite_id"),
            props.pointer("/source_record/artifact_id"),
        ]
    };
    choices
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::trim)
        .find(|s| {
            s.strip_prefix("tos.").is_some_and(|tail| {
                !tail.is_empty()
                    && tail.split(['.', '-']).all(|part| {
                        !part.is_empty()
                            && part
                                .bytes()
                                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
                    })
            })
        })
        .map(str::to_owned)
}
type ContextKey = (String, String);
type ContextGroups = BTreeMap<ContextKey, Vec<Value>>;
type ContextOwners = BTreeMap<ContextKey, Vec<String>>;
fn contexts(
    nodes: &BTreeMap<String, Value>,
    order: &[String],
    cap: usize,
) -> Result<(ContextGroups, ContextOwners)> {
    let expected: BTreeSet<_> = nodes
        .iter()
        .filter(|(_, n)| {
            matches!(
                n.get("kind_id").and_then(Value::as_str),
                Some("claim" | "annotation-claim")
            )
        })
        .map(|(id, _)| id.clone())
        .collect();
    let actual: BTreeSet<_> = order.iter().cloned().collect();
    if actual.len() != order.len() || actual != expected {
        return Err(Error::Invalid("Claim candidate context encounter order"));
    }
    let mut groups = ContextGroups::new();
    let mut owners = ContextOwners::new();
    for id in order {
        let node = &nodes[id];
        let graph = identifier(node, "source_graph")?;
        for context in node
            .pointer("/semantics/assertion_contexts")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let fields = &context["fields"];
            let Some(reference) = fields
                .get("claim_id")
                .or_else(|| fields.get("claim_ref"))
                .and_then(|f| f.get("value"))
                .and_then(Value::as_str)
            else {
                continue;
            };
            if reference.is_empty() || reference.len() > 4096 {
                return Err(Error::Invalid("Claim candidate context reference"));
            }
            let key = (graph.to_owned(), reference.to_owned());
            let mut bound = context.clone();
            bound["binding_role"] = json!("referenced-claim");
            let group = groups.entry(key.clone()).or_default();
            if !group.contains(&bound) {
                if group.len() >= cap {
                    return Err(Error::Budget("Claim candidate contexts"));
                }
                group.push(bound);
                let contributors = owners.entry(key).or_default();
                if !contributors.contains(id) {
                    contributors.push(id.clone());
                }
            }
        }
    }
    Ok((groups, owners))
}
fn append_contexts(node: &mut Value, contributions: &[Value], cap: usize) -> Result<()> {
    if contributions.is_empty() {
        return Ok(());
    }
    let semantics = node["semantics"]
        .as_object_mut()
        .ok_or(Error::Invalid("Claim candidate semantics"))?;
    let existing = semantics
        .entry("assertion_contexts")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .ok_or(Error::Invalid("Claim candidate assertion contexts"))?;
    for contribution in contributions {
        if !existing.contains(contribution) {
            if existing.len() >= cap {
                return Err(Error::Budget("Claim candidate literal contexts"));
            }
            existing.push(contribution.clone());
        }
    }
    Ok(())
}
fn readable(
    node: &mut Value,
    owner: &[u8],
    owners: &ContextOwners,
    witnesses: &BTreeMap<String, Vec<u8>>,
    compiler: Option<&ReadableContextCompiler>,
    limits: ClaimCandidateLimits,
) -> Result<()> {
    let Some(compiler) = compiler else {
        return Ok(());
    };
    let graph = identifier(node, "source_graph")?;
    let mut sources = Vec::new();
    let mut seen = BTreeSet::new();
    for context in node
        .pointer("/semantics/assertion_contexts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if context.get("binding_role").and_then(Value::as_str) != Some("referenced-claim") {
            continue;
        }
        let fields = &context["fields"];
        let reference = fields
            .get("claim_id")
            .or_else(|| fields.get("claim_ref"))
            .and_then(|f| f.get("value"))
            .and_then(Value::as_str)
            .ok_or(Error::Invalid("Claim candidate readable reference"))?;
        for id in owners
            .get(&(graph.to_owned(), reference.to_owned()))
            .into_iter()
            .flatten()
        {
            if seen.insert(id) {
                sources.push(
                    witnesses
                        .get(id)
                        .ok_or(Error::Invalid("Claim context source witness"))?
                        .as_slice(),
                );
            }
        }
    }
    if sources.len() > limits.max_contexts {
        return Err(Error::Budget("Claim readable source count"));
    }
    let raw = encode(node, limits.max_row_bytes)?;
    let witness = ordered_readable_witness(&raw, owner, &sources, limits.max_row_bytes)?;
    match compiler.compile(&raw, Some(&witness))? {
        ReadableContextCarrier::Absent => {
            node.as_object_mut().unwrap().remove("readable_context");
        }
        ReadableContextCarrier::Sidecar(raw) => {
            node["readable_context"] = serde_json::from_slice(&raw)
                .map_err(|_| Error::Invalid("Claim readable sidecar"))?;
        }
    }
    stamp_content_revision(node, limits.max_row_bytes)
}

/// Returns only a supplied candidate, never a receipt for source admission,
/// complete incidence, complete reducers, rights, canon or publication.
pub fn normalize_claim_candidate(
    input: &ClaimCandidateInput,
    selected: ClaimCandidateRegistries<'_>,
    limits: ClaimCandidateLimits,
) -> Result<ClaimCandidateOutput> {
    if limits.max_row_bytes == 0
        || limits.max_row_bytes > 8 * 1024 * 1024
        || limits.max_input_bytes == 0
        || limits.max_output_bytes == 0
        || limits.max_contexts == 0
        || limits.max_contexts > 64
        || input.nodes.len() > limits.max_nodes
        || input.retained_nodes.len() > limits.max_retained_nodes
        || input.relations.len() > limits.max_relations
        || input.traces.len() > limits.max_traces
        || input.context_node_order.len()
            > limits.max_nodes.saturating_add(limits.max_retained_nodes)
        || input.dossier_refs.len() > limits.max_nodes.saturating_add(limits.max_retained_nodes)
    {
        return Err(Error::Budget("Claim candidate limits"));
    }
    let registry = KnowledgeRegistry::parse(selected.entity_bytes, selected.relation_bytes)?;
    let binding = &input.normalization_binding;
    if binding != selected.expected_normalization_binding
        || binding.get("schema").and_then(Value::as_str)
            != Some("tos_knowledge_graph_normalization_binding_v1")
        || binding
            .get("entity_registry_digest")
            .and_then(Value::as_str)
            != Some(&registry.entity_semantic_digest)
        || binding
            .get("relation_registry_digest")
            .and_then(Value::as_str)
            != Some(&registry.relation_semantic_digest)
    {
        return Err(Error::Invalid("Claim candidate normalization binding"));
    }
    let mut input_bytes = 0;
    for bytes in [
        selected.entity_bytes,
        selected.relation_bytes,
        selected.descriptor_bytes,
    ] {
        charge(&mut input_bytes, bytes.len(), limits.max_input_bytes)?;
    }
    for value in [
        binding,
        &json!(input.dossier_refs),
        &json!(input.context_node_order),
    ] {
        charge(
            &mut input_bytes,
            encode(value, limits.max_input_bytes)?.len(),
            limits.max_input_bytes,
        )?;
    }
    let claim = ClaimNormalizer::new(
        &registry,
        selected.entity_bytes,
        selected.relation_bytes,
        selected.vocabulary,
        selected.descriptor_bytes,
        ClaimNormalizeLimits {
            max_raw_bytes: limits.max_row_bytes,
            max_output_bytes: limits.max_row_bytes,
            max_page_rows: 1,
            max_contexts: limits.max_contexts,
            max_work_bytes: limits.max_input_bytes as u64,
        },
    )?;
    let mut navigation = NavigationNodeNormalizer::new(
        &registry,
        selected.entity_bytes,
        selected.vocabulary,
        selected.descriptor_bytes,
        NavigationNodeLimits {
            max_raw_bytes: limits.max_row_bytes,
            max_output_bytes: limits.max_row_bytes,
            max_ancestor_cache_bytes: 4 * 1024 * 1024,
        },
    )?;
    let base = KnowledgeBaseNormalizer::new(
        &registry,
        selected.entity_bytes,
        selected.relation_bytes,
        selected.vocabulary,
        selected.descriptor_bytes,
        BaseNormalizationLimits {
            max_registry_bytes: 4 * 1024 * 1024,
            max_output_bytes: limits.max_row_bytes,
        },
    )?;
    let compiler = ReadableContextCompiler::from_selected_registry_bytes(
        selected.entity_bytes,
        &registry.entity_sha256,
        ReadableContextLimits {
            max_input_bytes: limits.max_row_bytes,
            max_work_bytes: limits.max_input_bytes as u64,
        },
    )?;
    let mut traces = Vec::new();
    let mut trace_refs = BTreeSet::new();
    let mut predicates = BTreeMap::new();
    for raw in &input.traces {
        charge(&mut input_bytes, raw.payload.len(), limits.max_input_bytes)?;
        let trace = parsed(raw, limits.max_row_bytes)?;
        if raw.source_graph != "source-claims"
            || identifier(&trace, "claim_ref")? != raw.id
            || !trace_refs.insert(raw.id.clone())
        {
            return Err(Error::Invalid("Claim candidate trace"));
        }
        for field in [
            "claim_node_id",
            "subject_node_id",
            "object_node_id",
            "predicate",
        ] {
            identifier(&trace, field)?;
        }
        predicates.insert(
            identifier(&trace, "object_node_id")?.to_owned(),
            identifier(&trace, "predicate")?.to_owned(),
        );
        traces.push(trace);
    }
    let dossiers: BTreeSet<_> = input.dossier_refs.iter().collect();
    if dossiers.len() != input.dossier_refs.len() {
        return Err(Error::Invalid("Claim candidate duplicate dossier"));
    }
    if input
        .dossier_refs
        .iter()
        .any(|id| id.is_empty() || id.len() > 4096)
    {
        return Err(Error::Invalid("Claim candidate dossier identifier"));
    }
    let mut nodes = BTreeMap::new();
    let mut witnesses = BTreeMap::new();
    let mut raw_claims = BTreeMap::new();
    let mut expanded_bytes = 0;
    let expanded_cap = limits
        .max_input_bytes
        .checked_add(limits.max_output_bytes)
        .ok_or(Error::Budget("Claim candidate expanded byte cap"))?;
    for retained in &input.retained_nodes {
        let node = &retained.normalized;
        charge(
            &mut input_bytes,
            encode(node, limits.max_row_bytes)?.len(),
            limits.max_input_bytes,
        )?;
        charge(
            &mut input_bytes,
            retained.owner_material.len(),
            limits.max_input_bytes,
        )?;
        verify_content(node, limits.max_row_bytes)?;
        let owner = SourceRow::parse(&retained.owner_material, limits.max_row_bytes)?;
        if node.pointer("/source_record/payload") != Some(owner.value())
            || node
                .pointer("/source_record/digest")
                .and_then(Value::as_str)
                != Some(&owner.stable_digest()?)
        {
            return Err(Error::Invalid("Claim candidate retained source digest"));
        }
        let id = identifier(node, "id")?.to_owned();
        if nodes.insert(id.clone(), node.clone()).is_some() {
            return Err(Error::Invalid("Claim duplicate retained endpoint"));
        }
        witnesses.insert(id, retained.owner_material.clone());
        if node.get("source_graph").and_then(Value::as_str) == Some("source-claims") {
            raw_claims.insert(
                identifier(node, "native_id")?.to_owned(),
                owner.value().clone(),
            );
        }
    }
    let mut output_ids = Vec::new();
    for spec in &input.nodes {
        charge(
            &mut input_bytes,
            spec.raw.payload.len(),
            limits.max_input_bytes,
        )?;
        let raw = parsed(&spec.raw, limits.max_row_bytes)?;
        let native = identifier(&raw, "node_id")?.to_owned();
        if native != spec.raw.id {
            return Err(Error::Invalid("Claim candidate raw node identity"));
        }
        let id = format!("{}:{native}", spec.raw.source_graph);
        if nodes.contains_key(&id) {
            return Err(Error::Invalid("Claim candidate replacement collision"));
        }
        if spec
            .dossier_ref
            .as_ref()
            .is_some_and(|dossier| !dossiers.contains(dossier))
        {
            return Err(Error::Invalid("Claim candidate dossier membership"));
        }
        let admitted_dossier =
            declared_dossier(&raw, &spec.raw.source_graph).filter(|id| dossiers.contains(id));
        if admitted_dossier != spec.dossier_ref {
            return Err(Error::Invalid("Claim candidate declared dossier differs"));
        }
        let (mut node, witness) = match spec.raw.source_graph.as_str() {
            "source-claims" => {
                raw_claims.insert(native.clone(), raw);
                (
                    claim.normalize_supplied_node(
                        &spec.raw,
                        predicates.get(&native).map(String::as_str),
                        spec.dossier_ref.as_deref(),
                    )?,
                    ordered_claim_node_material(&spec.raw.payload, limits.max_row_bytes)?,
                )
            }
            "source-navigation" => (
                navigation
                    .normalize_supplied_node(&spec.raw)?
                    .value()
                    .clone(),
                spec.raw.payload.clone(),
            ),
            _ => return Err(Error::Invalid("Claim candidate node owner")),
        };
        if spec.raw.source_graph == "source-navigation"
            && node
                .get("source_dossier_ref")
                .and_then(Value::as_str)
                .is_some_and(|ref_| !dossiers.iter().any(|id| id.as_str() == ref_))
        {
            node.as_object_mut().unwrap().remove("source_dossier_ref");
            stamp_content_revision(&mut node, limits.max_row_bytes)?;
        }
        charge(
            &mut expanded_bytes,
            encode(&node, limits.max_row_bytes)?.len(),
            expanded_cap,
        )?;
        charge(&mut expanded_bytes, witness.len(), expanded_cap)?;
        witnesses.insert(id.clone(), witness);
        nodes.insert(id.clone(), node);
        output_ids.push(id);
    }
    let (groups, context_owners) =
        contexts(&nodes, &input.context_node_order, limits.max_contexts)?;
    // Reference-value carriers need independent fixed-slot/source-history
    // guards. Base reader support alone cannot admit that candidate family.
    let relation_registry = SourceRow::parse(selected.relation_bytes, 4 * 1024 * 1024)?;
    let entity_registry = SourceRow::parse(selected.entity_bytes, 4 * 1024 * 1024)?;
    validate_navigation_carriers(
        &raw_claims,
        &output_ids,
        relation_registry.value(),
        entity_registry.value(),
        limits.max_row_bytes,
    )?;
    // Parse and charge the complete supplied relation cohort once. The same
    // admitted raw values drive fixed-slot guards and relation normalization.
    let mut raw_edges = Vec::new();
    for spec in &input.relations {
        charge(
            &mut input_bytes,
            spec.raw.payload.len(),
            limits.max_input_bytes,
        )?;
        raw_edges.push(parsed(&spec.raw, limits.max_row_bytes)?);
    }
    reference_carriers::validate(
        &raw_claims,
        &output_ids,
        &traces,
        &raw_edges,
        &nodes,
        &registry,
        entity_registry.value(),
        relation_registry.value(),
        limits.max_row_bytes,
    )?;
    for trace in &traces {
        for field in ["claim_node_id", "subject_node_id", "object_node_id"] {
            if !nodes.contains_key(&format!("source-claims:{}", identifier(trace, field)?)) {
                return Err(Error::Invalid("Claim candidate missing trace endpoint"));
            }
        }
    }
    for id in &output_ids {
        let node = &nodes[id];
        if node.get("source_graph").and_then(Value::as_str) == Some("source-claims")
            && node.get("kind_id").and_then(Value::as_str) == Some("claim")
            && !traces
                .iter()
                .any(|trace| trace.get("claim_node_id") == node.get("native_id"))
        {
            return Err(Error::Invalid("Claim candidate missing governing trace"));
        }
    }
    let mut relations = Vec::new();
    let mut relation_owners = Vec::new();
    let mut relation_ids = BTreeSet::new();
    let mut inherited: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (spec, raw) in input.relations.iter().zip(&raw_edges) {
        if identifier(&raw, "edge_id")? != spec.raw.id {
            return Err(Error::Invalid("Claim candidate edge identity"));
        }
        let graph = &spec.raw.source_graph;
        let endpoint = |end: &str| -> Result<String> {
            let source = raw
                .get(format!("{end}_source_graph"))
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .unwrap_or(graph);
            Ok(format!(
                "{source}:{}",
                identifier(&raw, &format!("{end}_id"))?
            ))
        };
        let left = endpoint("from")?;
        let right = endpoint("to")?;
        let left_node = nodes
            .get(&left)
            .ok_or(Error::Invalid("Claim candidate left endpoint"))?;
        let right_node = nodes
            .get(&right)
            .ok_or(Error::Invalid("Claim candidate right endpoint"))?;
        let reference = raw.get("claim_ref").and_then(Value::as_str);
        let context = reference.and_then(|ref_| groups.get(&(graph.clone(), ref_.to_owned())));
        let mut relation = if graph == "source-claims" {
            let ref_ = reference.ok_or(Error::Invalid("Claim candidate edge governing trace"))?;
            let trace = traces
                .iter()
                .find(|trace| trace.get("claim_ref").and_then(Value::as_str) == Some(ref_))
                .ok_or(Error::Invalid("Claim candidate edge trace absent"))?;
            let object = raw_claims
                .get(identifier(trace, "object_node_id")?)
                .ok_or(Error::Invalid("Claim candidate raw literal endpoint"))?;
            let target = raw_claims
                .get(identifier(&raw, "to_id")?)
                .ok_or(Error::Invalid("Claim candidate raw target endpoint"))?;
            claim.normalize_supplied_relation(
                &spec.raw,
                object,
                target,
                &left_node["display"]["title"],
                &right_node["display"]["title"],
                context.ok_or(Error::Invalid("Claim candidate referenced context absent"))?,
                spec.identity_id.as_deref(),
            )?
        } else {
            let source = SourceRow::parse(&spec.raw.payload, limits.max_row_bytes)?;
            let mut relation = base.normalize_relation(
                &source,
                graph,
                spec.identity_id.as_deref(),
                &left_node["display"]["title"],
                &right_node["display"]["title"],
                if graph == "canon" {
                    "canon"
                } else {
                    "derived-export"
                },
            )?;
            if let Some(context) = context {
                let semantics = relation["semantics"]
                    .as_object_mut()
                    .ok_or(Error::Invalid("Claim candidate relation semantics"))?;
                let direct = semantics
                    .entry("assertion_contexts")
                    .or_insert_with(|| json!([]))
                    .as_array_mut()
                    .ok_or(Error::Invalid("Claim candidate relation contexts"))?;
                if direct.len().saturating_add(context.len()) > limits.max_contexts {
                    return Err(Error::Budget("Claim candidate relation context bytes"));
                }
                direct.extend(context.iter().cloned());
            }
            relation
        };
        stamp_content_revision(&mut relation, limits.max_row_bytes)?;
        if !relation_ids.insert(identifier(&relation, "id")?.to_owned()) {
            return Err(Error::Invalid("Claim candidate duplicate relation"));
        }
        for endpoint in [&left, &right] {
            let views = inherited.entry(endpoint.clone()).or_default();
            views.extend(
                relation
                    .get("view_ids")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned),
            );
        }
        let owner = if graph == "source-claims" {
            ordered_claim_relation_material(
                &spec.raw.payload,
                &relation["source_record"]["payload"],
                limits.max_row_bytes,
            )?
        } else {
            spec.raw.payload.clone()
        };
        charge(
            &mut expanded_bytes,
            encode(&relation, limits.max_row_bytes)?.len(),
            expanded_cap,
        )?;
        charge(&mut expanded_bytes, owner.len(), expanded_cap)?;
        relation_owners.push(owner);
        relations.push(relation);
    }
    // Compute updates against base endpoints, retaining historical last-wins
    // trace behavior; do not mutate retained endpoints as output members.
    let mut updates = BTreeMap::new();
    let mut literals: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for trace in &traces {
        let claim_id = format!("source-claims:{}", identifier(trace, "claim_node_id")?);
        let subject_id = format!("source-claims:{}", identifier(trace, "subject_node_id")?);
        let object_id = format!("source-claims:{}", identifier(trace, "object_node_id")?);
        let update = claim.finalize_claim(
            &nodes[&claim_id],
            &nodes[&subject_id],
            &nodes[&object_id],
            trace,
        )?;
        charge(
            &mut expanded_bytes,
            encode(&update, limits.max_row_bytes)?.len(),
            expanded_cap,
        )?;
        updates.insert(claim_id.clone(), update);
        if nodes[&object_id]
            .pointer("/source_record/payload/node_kind")
            .and_then(Value::as_str)
            == Some("literal")
        {
            let accumulated = literals.entry(object_id).or_default();
            if let Some(group) = groups.get(&(
                "source-claims".into(),
                identifier(trace, "claim_ref")?.to_owned(),
            )) {
                for context in group {
                    if !accumulated.contains(context) {
                        charge(
                            &mut expanded_bytes,
                            encode(context, limits.max_row_bytes)?.len(),
                            expanded_cap,
                        )?;
                        accumulated.push(context.clone());
                    }
                }
            }
        }
    }
    let mut output_nodes = Vec::new();
    let mut output_bytes = 7usize;
    for id in output_ids {
        let mut node = updates.remove(&id).unwrap_or_else(|| nodes[&id].clone());
        if let Some(context) = literals.get(&id) {
            append_contexts(&mut node, context, limits.max_contexts)?;
        }
        if let Some(inherited) = inherited.get(&id).filter(|views| !views.is_empty()) {
            let mut views: BTreeSet<_> = node
                .get("view_ids")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect();
            views.extend(inherited.iter().cloned());
            node["view_ids"] = json!(views);
        }
        stamp_content_revision(&mut node, limits.max_row_bytes)?;
        readable(
            &mut node,
            &witnesses[&id],
            &context_owners,
            &witnesses,
            compiler.as_ref(),
            limits,
        )?;
        charge(
            &mut output_bytes,
            encode(&node, limits.max_row_bytes)?.len() + usize::from(!output_nodes.is_empty()),
            limits.max_output_bytes,
        )?;
        output_nodes.push(node);
    }
    for (index, relation) in relations.iter_mut().enumerate() {
        readable(
            relation,
            &relation_owners[index],
            &context_owners,
            &witnesses,
            compiler.as_ref(),
            limits,
        )?;
        charge(
            &mut output_bytes,
            encode(relation, limits.max_row_bytes)?.len() + usize::from(index > 0),
            limits.max_output_bytes,
        )?;
    }
    Ok(ClaimCandidateOutput {
        nodes: output_nodes,
        relations,
        input_bytes,
        output_bytes,
    })
}
