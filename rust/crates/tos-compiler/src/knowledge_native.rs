//! Native maintained-family assembly in source dependency order, followed by
//! complete title, endpoint, inherited-view and readable-context joins.
//! This writes final private rows; semantic admission and owner selection are
//! independent gates on the returned candidate.

use crate::knowledge_candidates::{candidate_normalizer, prepare_candidate_inputs};
use crate::knowledge_canon_materialize::{
    materialize_canon_nodes, materialize_canon_relations, scan_canon_relations, source_material,
    CanonMaterializeLimits, CanonNormalizer,
};
use crate::knowledge_canon_prepare::{
    clear_canon_prepare, prepare_canon_inputs, CanonPrepareLimits,
};
use crate::knowledge_normalization::SourceRow;
use crate::knowledge_source_claims::{
    claim_context_sources, claim_contexts, clear_source_claim_indices, finalize_source_claims,
    materialize_source_claim_nodes, materialize_source_claim_relations,
    prepare_claim_context_groups, ClaimNormalizeLimits, ClaimNormalizer,
};
use crate::knowledge_source_navigation_prepare::{
    clear_source_navigation_prepare, NavigationJoinClosure,
};
use crate::knowledge_source_navigation_relation::{
    clear_navigation_relation_dependencies, NavigationRelationCompletionProof,
};
use crate::knowledge_stage::{KnowledgeStage, NodeRow, WritePhase};
use crate::*;
use serde_json::Value;
use tos_foundation::{Digest256, Digest256Hasher};

/// Concrete maintained family inputs. Repository root identity/wording are
/// selected source material, never inferred from an ambient checkout path.
pub struct NativeFamilyInputs<'a> {
    /// Explicit maintained prepared projection profile; sealed/D1 callers default false.
    pub prepared_philosophy_projection: bool,
    pub repository_root: Option<RepositoryRootInput<'a>>,
    pub navigation_original: Option<NavigationOriginalInput<'a>>,
    pub philosophy_original: Option<crate::PhilosophyOriginalInput<'a>>,
    pub corpus_original: Option<&'a crate::CapturedCorpusOriginalPlan>,
    pub topology: TopologyLimits,
    pub canon_prepare: CanonPrepareLimits,
    pub canon: CanonMaterializeLimits,
    pub indexed: IndexedLimits,
}

impl NativeFamilyInputs<'_> {
    pub fn bounded_from(limits: NativeProducerLimits) -> Self {
        Self {
            prepared_philosophy_projection: false,
            repository_root: None,
            navigation_original: None,
            philosophy_original: None,
            corpus_original: None,
            topology: TopologyLimits {
                max_rows: limits.finalize.max_rows,
                max_page_rows: limits.finalize.max_page_rows,
                max_row_bytes: limits.finalize.max_row_bytes,
                max_work_bytes: limits.finalize.max_work_bytes,
            },
            canon_prepare: CanonPrepareLimits {
                max_nodes: limits.philosophy_prepare.max_nodes,
                max_packs: limits.philosophy_prepare.max_edges,
                max_edges: limits.philosophy_prepare.max_edges,
                max_node_relations: limits.philosophy_prepare.max_edges,
                max_page_rows: limits.philosophy_prepare.max_page_rows,
                max_row_bytes: limits.philosophy_prepare.max_row_bytes,
                max_work_bytes: limits.philosophy_prepare.max_work_bytes,
            },
            canon: CanonMaterializeLimits {
                max_raw_bytes: limits.philosophy.max_raw_bytes,
                max_output_bytes: limits.philosophy.max_output_bytes,
                max_registry_bytes: limits.philosophy.max_registry_bytes,
                max_page_rows: limits.philosophy.max_page_rows,
                max_page_bytes: limits.finalize.max_page_bytes,
                max_rows: limits.philosophy.max_rows,
                max_work_bytes: limits.philosophy.max_work_bytes,
            },
            indexed: IndexedLimits {
                max_row_bytes: limits.finalize.max_row_bytes,
                max_page_rows: limits.finalize.max_page_rows,
            },
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NativeProducerLimits {
    pub navigation_prepare: NavigationPrepareLimits,
    pub navigation_nodes: NavigationNodeLimits,
    pub navigation_materialize: NavigationMaterializeLimits,
    pub navigation_dependencies: NavigationRelationLimits,
    pub navigation_relations: NavigationRelationNormalizeLimits,
    pub claims_prepare: ClaimPrepareLimits,
    pub claims: ClaimNormalizeLimits,
    pub philosophy_prepare: PhilosophyPrepareLimits,
    pub philosophy: PhilosophyMaterializeLimits,
    pub titles: GlobalTitleLimits,
    pub inherited: InheritedViewLimits,
    pub finalize: NativeFinalizeLimits,
}

/// Adapter implementations accepted by the existing native composition.
/// Consumers use this capability closure, never derive support from input data.
pub const NATIVE_KNOWLEDGE_ADAPTER_PROFILES: &[&str] = &[
    "source-navigation-node-edge-v1",
    "reified-bibliographic-claims-v1",
    "philosophy-node-edge-v1",
    "canon-node-relation-v1",
    "candidate-relation-v1",
    "repository-topology-v1",
    "declared-identity-and-source-ref-joins-v1",
    "indexed-node-edge-v1",
];

#[derive(Clone, Debug)]
pub struct NativeProducerReceipt {
    pub final_rows: NativeFinalizeReceipt,
    pub navigation_original: Option<crate::NavigationOriginalReceipt>,
    pub philosophy_original: Option<crate::PhilosophyOriginalReceipt>,
    pub corpus_original: Option<crate::CorpusOriginalReceipt>,
    pub base_node_root_sha256: String,
    pub endpoint_title_root_sha256: String,
    pub claim_group_root_sha256: String,
    pub placeholder_absence_root_sha256: String,
    pub navigation_nodes: u64,
    pub navigation_relations: u64,
    pub claim_nodes: u64,
    pub claim_relations: u64,
    pub philosophy_nodes: u64,
    pub philosophy_relations: u64,
    pub canon_nodes: u64,
    pub canon_relations: u64,
    pub candidate_relations: u64,
    pub repository_nodes: u64,
    pub repository_relations: u64,
    pub semantic_relations: u64,
    pub indexed_nodes: u64,
    pub indexed_relations: u64,
}

/// Additional native relation families use the same missing-endpoint kernel,
/// but their own source cut and complete edge input root. No path heuristic or
/// fabricated navigation receipt is used to change the endpoint's owner.
fn other_placeholders(
    stage: &mut KnowledgeStage<'_>,
    normalizer: &mut NavigationNodeNormalizer<'_>,
    navigation_source: &str,
    limits: NavigationMaterializeLimits,
) -> Result<String> {
    let entries = stage
        .exact_receipt()
        .collections
        .iter()
        .filter(|entry| entry.collection == "edges" && entry.source_graph != navigation_source)
        .cloned()
        .collect::<Vec<_>>();
    let cut = stage.exact_receipt().binding.source_cut.clone();
    let mut evidence = Digest256Hasher::new();
    evidence.update(b"tos-native-other-placeholder-absence-v1\0");
    let mut placeholders = 0u64;
    let mut work = 0u64;
    let mut order: i64 = stage.with_connection(WritePhase::Sort, |db| {
        Ok(db.query_row(
            "SELECT coalesce(max(source_order)+1,0) FROM knowledge_nodes",
            [],
            |r| r.get(0),
        )?)
    })?;
    for entry in entries {
        if entry.expected_count > limits.max_edges {
            return Err(Error::Budget("native placeholder edges"));
        }
        let mut after = None;
        let mut count = 0u64;
        let mut root = Digest256Hasher::new();
        loop {
            let (seek_rows, _) = stage.input_batch_limits();
            let (write_rows, write_bytes) = stage.write_page_limits();
            let page_rows = limits
                .max_page_rows
                .min(seek_rows)
                .min(write_rows / 2)
                .min(write_bytes as usize / (2 * limits.max_output_bytes));
            let page =
                stage.scan_input(&entry.source_graph, "edges", after.as_deref(), page_rows)?;
            stage.with_write_page(
                WritePhase::Normalized,
                page_rows * 2,
                (page_rows * 2 * limits.max_output_bytes) as u64,
                |stage| {
                    for raw in &page.rows {
                        if raw.payload.len() > limits.max_raw_bytes {
                            return Err(Error::Budget("native placeholder raw bytes"));
                        }
                        count = count
                            .checked_add(1)
                            .ok_or(Error::Budget("native placeholder rows"))?;
                        work = work
                            .checked_add(raw.payload.len() as u64)
                            .ok_or(Error::Budget("native placeholder work"))?;
                        if count > entry.expected_count || work > limits.max_work_bytes {
                            return Err(Error::Budget("native placeholder scan"));
                        }
                        root.update(&(raw.id.len() as u64).to_be_bytes());
                        root.update(raw.id.as_bytes());
                        root.update(Digest256::of_bytes(&raw.payload).as_bytes());
                        let source = SourceRow::parse(&raw.payload, limits.max_raw_bytes)?;
                        for (endpoint, key, source_key) in [
                            (NavigationEndpoint::From, "from_id", "from_source_graph"),
                            (NavigationEndpoint::To, "to_id", "to_source_graph"),
                        ] {
                            let native = source
                                .value()
                                .get(key)
                                .and_then(Value::as_str)
                                .ok_or(Error::Invalid("native placeholder endpoint"))?;
                            let graph = source
                                .value()
                                .get(source_key)
                                .and_then(Value::as_str)
                                .unwrap_or(&entry.source_graph);
                            let id = format!("{graph}:{native}");
                            let exists = stage.with_connection(WritePhase::Sort, |db| {
                                Ok(db.query_row(
                                    "SELECT EXISTS(SELECT 1 FROM knowledge_nodes WHERE id=?1)",
                                    [&id],
                                    |r| r.get::<_, bool>(0),
                                )?)
                            })?;
                            if exists {
                                continue;
                            }
                            let base = normalizer.normalize_relation_endpoint(
                                &raw,
                                &cut,
                                &entry.expected_root_sha256,
                                endpoint,
                            )?;
                            let value = base.value();
                            let bytes = serde_json::to_vec(value)
                                .map_err(|_| Error::Invalid("native placeholder JSON"))?;
                            if bytes.len() > limits.max_output_bytes {
                                return Err(Error::Budget("native placeholder output bytes"));
                            }
                            work = work
                                .checked_add(bytes.len() as u64)
                                .ok_or(Error::Budget("native placeholder output work"))?;
                            placeholders = placeholders
                                .checked_add(1)
                                .ok_or(Error::Budget("native placeholders"))?;
                            if placeholders > limits.max_placeholders
                                || work > limits.max_work_bytes
                            {
                                return Err(Error::Budget("native placeholders"));
                            }
                            let field = |name: &str| {
                                value
                                    .get(name)
                                    .and_then(Value::as_str)
                                    .ok_or(Error::Invalid("native placeholder field"))
                            };
                            stage.insert_node(NodeRow {
                                id: field("id")?,
                                source_graph: field("source_graph")?,
                                native_id: Some(field("native_id")?),
                                entity_id: Some(field("entity_id")?),
                                kind_id: field("kind_id")?,
                                type_id: field("type_id")?,
                                source_order: order,
                                payload: &bytes,
                            })?;
                            order = order
                                .checked_add(1)
                                .ok_or(Error::Budget("native placeholder order"))?;
                            for text in [&entry.source_graph, &raw.id, key, &id] {
                                evidence.update(&(text.len() as u64).to_be_bytes());
                                evidence.update(text.as_bytes());
                            }
                            evidence.update(Digest256::of_bytes(&raw.payload).as_bytes());
                        }
                    }
                    Ok(())
                },
            )?;
            match page.next_id {
                Some(id) => after = Some(id),
                None => break,
            }
        }
        if count != entry.expected_count || root.finalize().to_hex() != entry.expected_root_sha256 {
            return Err(Error::Invalid("native placeholder complete input root"));
        }
    }
    Ok(evidence.finalize().to_hex())
}

fn prepared_family_placeholders(
    stage: &mut KnowledgeStage<'_>,
    normalizer: &mut NavigationNodeNormalizer<'_>,
    canon: &[&crate::knowledge_canon_prepare::CanonPrepareReceipt],
    repository: Option<&RepositoryPrepareReceipt>,
    topology: TopologyLimits,
    canon_limits: CanonMaterializeLimits,
    limits: NavigationMaterializeLimits,
) -> Result<String> {
    use crate::knowledge_stage::SeekRow;
    let cut = stage.exact_receipt().binding.source_cut.clone();
    let mut hash = Digest256Hasher::new();
    hash.update(b"tos-native-prepared-placeholder-absence-v1\0");
    let mut work = 0u64;
    let mut count = 0u64;
    let mut order: i64 = stage.with_connection(WritePhase::Sort, |db| {
        Ok(db.query_row(
            "SELECT coalesce(max(source_order)+1,0) FROM knowledge_nodes",
            [],
            |r| r.get(0),
        )?)
    })?;
    let mut visit = |stage: &mut KnowledgeStage<'_>,
                     graph: &str,
                     source: &SourceRow,
                     root: &str|
     -> Result<()> {
        let payload = serde_json::to_vec(source.value())
            .map_err(|_| Error::Invalid("native prepared edge JSON"))?;
        if payload.len() > limits.max_raw_bytes {
            return Err(Error::Budget("native prepared edge bytes"));
        }
        work = work
            .checked_add(payload.len() as u64)
            .filter(|n| *n <= limits.max_work_bytes)
            .ok_or(Error::Budget("native prepared edge work"))?;
        let id = source
            .value()
            .get("edge_id")
            .and_then(Value::as_str)
            .ok_or(Error::Invalid("native prepared edge identity"))?;
        let raw = SeekRow {
            id: id.into(),
            source_graph: graph.into(),
            source_order: None,
            payload_sha256: Digest256::of_bytes(&payload).to_hex(),
            payload,
        };
        for (endpoint, key, source_key) in [
            (NavigationEndpoint::From, "from_id", "from_source_graph"),
            (NavigationEndpoint::To, "to_id", "to_source_graph"),
        ] {
            let Some(native) = source
                .value()
                .get(key)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
            else {
                continue;
            };
            let graph = source
                .value()
                .get(source_key)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or(graph);
            let identifier = format!("{graph}:{native}");
            let exists: bool = stage.with_connection(WritePhase::Sort, |db| {
                Ok(db.query_row(
                    "SELECT EXISTS(SELECT 1 FROM knowledge_nodes WHERE id=?1)",
                    [&identifier],
                    |r| r.get(0),
                )?)
            })?;
            if exists {
                continue;
            }
            let base = normalizer.normalize_relation_endpoint(&raw, &cut, root, endpoint)?;
            let value = base.value();
            let payload = serde_json::to_vec(value)
                .map_err(|_| Error::Invalid("native prepared placeholder JSON"))?;
            if payload.len() > limits.max_output_bytes {
                return Err(Error::Budget("native prepared placeholder bytes"));
            }
            work = work
                .checked_add(payload.len() as u64)
                .filter(|n| *n <= limits.max_work_bytes)
                .ok_or(Error::Budget("native prepared placeholder work"))?;
            count = count
                .checked_add(1)
                .filter(|n| *n <= limits.max_placeholders)
                .ok_or(Error::Budget("native prepared placeholders"))?;
            let field = |key: &str| {
                value
                    .get(key)
                    .and_then(Value::as_str)
                    .ok_or(Error::Invalid("native prepared placeholder field"))
            };
            stage.insert_node(NodeRow {
                id: field("id")?,
                source_graph: field("source_graph")?,
                native_id: Some(field("native_id")?),
                entity_id: Some(field("entity_id")?),
                kind_id: field("kind_id")?,
                type_id: field("type_id")?,
                source_order: order,
                payload: &payload,
            })?;
            order = order
                .checked_add(1)
                .ok_or(Error::Budget("native prepared placeholder order"))?;
            for part in [graph, id, key, &identifier, root] {
                hash.update(&(part.len() as u64).to_be_bytes());
                hash.update(part.as_bytes());
            }
            hash.update(Digest256::of_bytes(&raw.payload).as_bytes());
        }
        Ok(())
    };
    for prepared in canon {
        let mut after = None;
        loop {
            let (write_rows, write_bytes) = stage.write_page_limits();
            let page_rows = canon_limits
                .max_page_rows
                .min(write_rows / 2)
                .min(write_bytes as usize / (2 * limits.max_output_bytes));
            let rows = scan_canon_relations(
                stage,
                prepared,
                after.as_deref(),
                page_rows,
                canon_limits.max_raw_bytes,
                canon_limits.max_page_bytes,
            )?;
            if rows.is_empty() {
                break;
            }
            stage.with_write_page(
                WritePhase::Normalized,
                page_rows * 2,
                (page_rows * 2 * limits.max_output_bytes) as u64,
                |stage| {
                    for row in &rows {
                        after = Some(row.identity_id.clone());
                        let source = SourceRow::parse(&row.material, canon_limits.max_raw_bytes)?;
                        visit(
                            stage,
                            &prepared.source_graph,
                            &source,
                            &prepared.dependency_root_sha256,
                        )?;
                    }
                    Ok(())
                },
            )?;
        }
    }
    if let Some(prepared) = repository {
        crate::knowledge_repository::scan_repository_placeholder_sources(
            stage,
            prepared,
            topology,
            limits.max_output_bytes,
            |stage, graph, source| visit(stage, graph, source, &prepared.dependency_root_sha256),
        )?;
    }
    Ok(hash.finalize().to_hex())
}

// Every maintained relation family consumes the same complete Claim groups.
// Read the join key from the exact copied source payload, never from a
// similarly named property or an inferred relation endpoint.
fn bind_native_claim_contexts(
    stage: &mut KnowledgeStage<'_>,
    vocabulary: &QueryVocabulary,
    groups: &crate::knowledge_source_claims::ClaimContextReceipt,
    claims: ClaimNormalizeLimits,
    limits: NativeFinalizeLimits,
) -> Result<()> {
    if limits.max_row_bytes == 0 || limits.max_page_rows == 0 {
        return Err(Error::Budget("native Claim join page limits"));
    }
    let mut after = -1i64;
    let mut work = 0u64;
    let mut count = 0u64;
    loop {
        let (write_rows, write_bytes) = stage.write_page_limits();
        let page_rows = (write_bytes as usize / limits.max_row_bytes)
            .min(write_rows)
            .min(limits.max_page_rows);
        let mut exhausted = false;
        let mut write_page = |stage: &mut KnowledgeStage<'_>| -> Result<()> {
            for _ in 0..page_rows {
                // Only one carrier and its derived context are resident at a time.
                let row: Option<(i64, String, String, Vec<u8>, Vec<u8>)> =
            stage.with_connection(WritePhase::Finalize, |db| {
                use rusqlite::OptionalExtension;
                Ok(db.query_row(
                    "SELECT source_order,id,source_graph,CASE WHEN payload_len=length(payload) AND payload_len<=?2 THEN payload ELSE NULL END,payload_sha256 FROM knowledge_relations WHERE source_order>?1 ORDER BY source_order LIMIT 1",
                    rusqlite::params![after, limits.max_row_bytes],
                    |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?)),
                ).optional()?)
            })?;
                let Some((order, id, graph, raw, sha)) = row else {
                    exhausted = true;
                    break;
                };
                count = count
                    .checked_add(1)
                    .ok_or(Error::Budget("native Claim join rows"))?;
                work = work
                    .checked_add(raw.len() as u64)
                    .ok_or(Error::Budget("native Claim join work"))?;
                if count > limits.max_rows || work > limits.max_work_bytes {
                    return Err(Error::Budget("native Claim join limits"));
                }
                if order <= after || sha.as_slice() != Digest256::of_bytes(&raw).as_bytes() {
                    return Err(Error::Invalid("native Claim join row digest/order"));
                }
                after = order;
                if vocabulary.sources.iter().any(|s| {
                    s.source_graph_id == graph && s.adapter_profile == "indexed-node-edge-v1"
                }) {
                    continue;
                }
                let mut value = SourceRow::parse(&raw, limits.max_row_bytes)?
                    .value()
                    .clone();
                if value.get("id").and_then(Value::as_str) != Some(id.as_str())
                    || value.get("source_graph").and_then(Value::as_str) != Some(graph.as_str())
                {
                    return Err(Error::Invalid("native Claim join row identity"));
                }
                let Some(reference) = value
                    .pointer("/source_record/payload/claim_ref")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                else {
                    continue;
                };
                let referenced = claim_contexts(stage, groups, &graph, &reference, claims)?;
                let mut contexts: Vec<Value> = value
                    .pointer("/semantics/assertion_contexts")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter(|c| {
                        c.get("binding_role").and_then(Value::as_str) != Some("referenced-claim")
                    })
                    .cloned()
                    .collect();
                contexts.extend(referenced);
                if contexts.is_empty() {
                    continue;
                }
                let next = Value::Array(contexts);
                if value.pointer("/semantics/assertion_contexts") == Some(&next) {
                    continue;
                }
                value["semantics"]["assertion_contexts"] = next;
                crate::knowledge_normalization::stamp_content_revision(
                    &mut value,
                    limits.max_row_bytes,
                )?;
                let output = serde_json::to_vec(&value)
                    .map_err(|_| Error::Invalid("native Claim join JSON"))?;
                work = work
                    .checked_add(output.len() as u64)
                    .ok_or(Error::Budget("native Claim join work"))?;
                if output.len() > limits.max_row_bytes || work > limits.max_work_bytes {
                    return Err(Error::Budget("native Claim join output"));
                }
                stage.charge_materialized(1, output.len() as u64)?;
                let digest = Digest256::of_bytes(&output);
                let changed = stage.with_connection(WritePhase::Finalize, |db| {
            Ok(db.execute("UPDATE knowledge_relations SET payload_len=?1,payload_sha256=?2,payload=?3 WHERE id=?4 AND payload_sha256=?5",
                rusqlite::params![output.len(),digest.as_bytes().as_slice(),output,id,sha])?)
        })?;
                if changed != 1 {
                    return Err(Error::Invalid("native Claim join concurrent row change"));
                }
            }
            Ok(())
        };
        stage.with_write_page(
            WritePhase::Finalize,
            page_rows,
            limits.max_page_bytes as u64,
            &mut write_page,
        )?;
        if exhausted {
            break;
        }
    }
    Ok(())
}

/// Assemble every source selected by these maintained native adapter profiles.
/// Unknown/extension adapters refuse; absence is never treated as an empty
/// source. The exact input receipt owns complete raw membership and is checked
/// again by `KnowledgeStage::finish` after full component compilation.
pub fn materialize_native_sources(
    stage: &mut KnowledgeStage<'_>,
    registry: &KnowledgeRegistry,
    entity_bytes: &[u8],
    relation_bytes: &[u8],
    vocabulary: &QueryVocabulary,
    descriptor_bytes: &[u8],
    navigation_header: &NavigationHeaderClaim,
    limits: NativeProducerLimits,
) -> Result<NativeProducerReceipt> {
    materialize_native_sources_with_inputs(
        stage,
        registry,
        entity_bytes,
        relation_bytes,
        vocabulary,
        descriptor_bytes,
        navigation_header,
        limits,
        NativeFamilyInputs::bounded_from(limits),
    )
}

pub fn materialize_native_sources_with_inputs(
    stage: &mut KnowledgeStage<'_>,
    registry: &KnowledgeRegistry,
    entity_bytes: &[u8],
    relation_bytes: &[u8],
    vocabulary: &QueryVocabulary,
    descriptor_bytes: &[u8],
    navigation_header: &NavigationHeaderClaim,
    limits: NativeProducerLimits,
    additional: NativeFamilyInputs<'_>,
) -> Result<NativeProducerReceipt> {
    let result = (|| {
        vocabulary.verify_authored_bytes(descriptor_bytes)?;
        let selected = |profile: &str| -> Result<bool> {
            let count = vocabulary
                .sources
                .iter()
                .filter(|s| s.adapter_profile == profile)
                .count();
            if count > 1 {
                return Err(Error::Invalid("duplicate native source family"));
            }
            Ok(count == 1)
        };
        if !selected("source-navigation-node-edge-v1")?
            || vocabulary
                .sources
                .iter()
                .any(|s| !NATIVE_KNOWLEDGE_ADAPTER_PROFILES.contains(&s.adapter_profile.as_str()))
        {
            return Err(Error::Invalid("native source adapter family incomplete"));
        }
        let navigation = prepare_source_navigation(
            stage,
            vocabulary,
            navigation_header,
            limits.navigation_prepare,
        )?;
        let navigation_original = if let Some(original) = additional.navigation_original.as_ref() {
            Some(retain_navigation_original(
                stage,
                vocabulary,
                &navigation,
                navigation_header,
                original.rights,
                original.expected_rights_root_sha256,
                original.limits,
            )?)
        } else {
            None
        };
        let mut nav_nodes = NavigationNodeNormalizer::new(
            registry,
            entity_bytes,
            vocabulary,
            descriptor_bytes,
            limits.navigation_nodes,
        )?;
        let nav_relations = NavigationRelationNormalizer::new(
            registry,
            relation_bytes,
            vocabulary,
            descriptor_bytes,
            limits.navigation_relations,
        )?;
        let nav_base = materialize_navigation_nodes(
            stage,
            &mut nav_nodes,
            &navigation,
            limits.navigation_materialize,
        )?;
        let claims = if selected("reified-bibliographic-claims-v1")? {
            let prepared = prepare_source_claims(stage, vocabulary, limits.claims_prepare)?;
            let normalizer = ClaimNormalizer::new(
                registry,
                entity_bytes,
                relation_bytes,
                vocabulary,
                descriptor_bytes,
                limits.claims,
            )?;
            materialize_source_claim_nodes(stage, &prepared, &normalizer)?;
            Some((prepared, normalizer))
        } else {
            None
        };
        let mut philosophy_original = None;
        let philosophy = if selected("philosophy-node-edge-v1")? {
            let prepared = if additional.prepared_philosophy_projection {
                crate::knowledge_philosophy_prepare::prepare_philosophy_projection(
                    stage,
                    vocabulary,
                    limits.philosophy_prepare,
                )?
            } else {
                prepare_philosophy(stage, vocabulary, limits.philosophy_prepare)?
            };
            if let Some(original) = additional.philosophy_original.as_ref() {
                philosophy_original = Some(crate::retain_philosophy_original(
                    stage, vocabulary, &prepared, original,
                )?);
            }
            let normalizer = PhilosophyNormalizer::new(
                registry,
                entity_bytes,
                relation_bytes,
                vocabulary,
                descriptor_bytes,
                limits.philosophy,
            )?;
            materialize_philosophy_nodes(stage, &normalizer, &prepared)?;
            Some((prepared, normalizer))
        } else {
            None
        };
        if additional.philosophy_original.is_some() && philosophy.is_none() {
            return Err(Error::Invalid(
                "philosophy originals without selected producer",
            ));
        }
        let canon = if selected("canon-node-relation-v1")? {
            let prepared = prepare_canon_inputs(stage, vocabulary, additional.canon_prepare)?;
            let normalizer = CanonNormalizer::new(
                registry,
                entity_bytes,
                relation_bytes,
                vocabulary,
                descriptor_bytes,
                additional.canon,
            )?;
            materialize_canon_nodes(stage, &normalizer, &prepared)?;
            Some((prepared, normalizer))
        } else {
            None
        };
        let candidates = if selected("candidate-relation-v1")? {
            let prepared = prepare_candidate_inputs(stage, vocabulary, additional.canon_prepare)?;
            let normalizer = candidate_normalizer(
                registry,
                entity_bytes,
                relation_bytes,
                vocabulary,
                descriptor_bytes,
                additional.canon,
            )?;
            Some((prepared, normalizer))
        } else {
            None
        };
        let base_normalizer = KnowledgeBaseNormalizer::new(
            registry,
            entity_bytes,
            relation_bytes,
            vocabulary,
            descriptor_bytes,
            BaseNormalizationLimits {
                max_registry_bytes: additional.canon.max_registry_bytes,
                max_output_bytes: limits.finalize.max_row_bytes,
            },
        )?;
        let repository = if selected("repository-topology-v1")? {
            let root = additional
                .repository_root
                .ok_or(Error::Invalid("native repository selected root absent"))?;
            let prepared =
                prepare_repository_topology(stage, vocabulary, root, additional.topology)?;
            materialize_repository_nodes(stage, &prepared, &base_normalizer, additional.topology)?;
            Some(prepared)
        } else {
            if additional.repository_root.is_some() {
                return Err(Error::Invalid("native unselected repository root"));
            }
            None
        };
        let indexed_nodes =
            materialize_registered_indexed_nodes(stage, vocabulary, registry, additional.indexed)?
                .node_count;
        order_native_graph_rows(
            stage,
            limits.finalize.max_rows,
            limits.finalize.max_work_bytes,
        )?;
        let roots = stage.core_roots()?;
        let base = CompleteBaseNodes {
            source_cut: navigation.source_cut.clone(),
            node_count: roots.nodes,
            node_root_sha256: roots.node_sha256,
        };
        let semantic = if selected("declared-identity-and-source-ref-joins-v1")? {
            Some(prepare_semantic_joins(
                stage,
                vocabulary,
                descriptor_bytes,
                &base,
                additional.topology,
            )?)
        } else {
            None
        };
        let placeholder = materialize_navigation_placeholders(
            stage,
            &mut nav_nodes,
            &navigation,
            &base,
            limits.navigation_materialize,
        )?;
        let extra_absence = other_placeholders(
            stage,
            &mut nav_nodes,
            &navigation.source_graph,
            limits.navigation_materialize,
        )?;
        let canon_prepared = canon
            .iter()
            .chain(candidates.iter())
            .map(|(prepared, _)| prepared)
            .collect::<Vec<_>>();
        let prepared_absence = prepared_family_placeholders(
            stage,
            &mut nav_nodes,
            &canon_prepared,
            repository.as_ref(),
            additional.topology,
            additional.canon,
            limits.navigation_materialize,
        )?;
        order_native_graph_rows(
            stage,
            limits.finalize.max_rows,
            limits.finalize.max_work_bytes,
        )?;
        let roots = stage.core_roots()?;
        let titles = prepare_global_titles(
            stage,
            &CompleteBaseNodes {
                source_cut: navigation.source_cut.clone(),
                node_count: roots.nodes,
                node_root_sha256: roots.node_sha256,
            },
            limits.titles,
        )?;
        let contexts = prepare_claim_context_groups(stage, limits.claims)?;
        let dependencies = prepare_navigation_relation_dependencies(
            stage,
            &navigation,
            vocabulary,
            limits.navigation_dependencies,
        )?;
        let relation = materialize_navigation_relations(
            stage,
            &nav_relations,
            &navigation,
            &dependencies,
            &titles.title_root_sha256,
            &contexts.root_sha256,
            limits.navigation_materialize,
            |stage, from, to, reference| {
                let left = endpoint_title(stage, &titles, from, limits.titles.max_title_bytes)?;
                let right = endpoint_title(stage, &titles, to, limits.titles.max_title_bytes)?;
                let group = reference
                    .map(|reference| {
                        claim_contexts(
                            stage,
                            &contexts,
                            &navigation.source_graph,
                            reference,
                            limits.claims,
                        )
                    })
                    .transpose()?
                    .unwrap_or_default();
                Ok((left, right, group))
            },
        )?;
        let (claim_nodes, claim_relations) = if let Some((prepared, normalizer)) = &claims {
            let relations = materialize_source_claim_relations(
                stage, prepared, normalizer, &contexts, &titles,
            )?;
            finalize_source_claims(stage, prepared, normalizer, &contexts)?;
            (prepared.nodes, relations)
        } else {
            (0, 0)
        };
        let (philosophy_nodes, philosophy_relations) =
            if let Some((prepared, normalizer)) = &philosophy {
                let receipt = materialize_philosophy_relations(
                    stage,
                    normalizer,
                    prepared,
                    &navigation.source_cut,
                    &titles.title_root_sha256,
                    |stage, from, to| {
                        Ok((
                            endpoint_title(stage, &titles, from, limits.titles.max_title_bytes)?,
                            endpoint_title(stage, &titles, to, limits.titles.max_title_bytes)?,
                        ))
                    },
                )?;
                (prepared.nodes, receipt.rows)
            } else {
                (0, 0)
            };
        let canon_relations = if let Some((prepared, normalizer)) = &canon {
            materialize_canon_relations(
                stage,
                normalizer,
                prepared,
                &navigation.source_cut,
                &titles.title_root_sha256,
                |stage, from, to| {
                    Ok((
                        endpoint_title(stage, &titles, from, limits.titles.max_title_bytes)?,
                        endpoint_title(stage, &titles, to, limits.titles.max_title_bytes)?,
                    ))
                },
            )?
            .rows
        } else {
            0
        };
        let candidate_relations = if let Some((prepared, normalizer)) = &candidates {
            materialize_canon_relations(
                stage,
                normalizer,
                prepared,
                &navigation.source_cut,
                &titles.title_root_sha256,
                |stage, from, to| {
                    Ok((
                        endpoint_title(stage, &titles, from, limits.titles.max_title_bytes)?,
                        endpoint_title(stage, &titles, to, limits.titles.max_title_bytes)?,
                    ))
                },
            )?
            .rows
        } else {
            0
        };
        let repository_relations = if let Some(prepared) = &repository {
            materialize_repository_relations(
                stage,
                prepared,
                &base_normalizer,
                &navigation.source_cut,
                &titles.title_root_sha256,
                additional.topology,
                |stage, from, to| {
                    Ok((
                        endpoint_title(stage, &titles, from, limits.titles.max_title_bytes)?,
                        endpoint_title(stage, &titles, to, limits.titles.max_title_bytes)?,
                    ))
                },
            )?
        } else {
            0
        };
        let semantic_relations = if let Some(prepared) = &semantic {
            materialize_semantic_relations(
                stage,
                prepared,
                &base_normalizer,
                &navigation.source_cut,
                &titles.title_root_sha256,
                additional.topology,
                |stage, from, to| {
                    Ok((
                        endpoint_title(stage, &titles, from, limits.titles.max_title_bytes)?,
                        endpoint_title(stage, &titles, to, limits.titles.max_title_bytes)?,
                    ))
                },
            )?
        } else {
            0
        };
        let indexed_relations = materialize_registered_indexed_relations(
            stage,
            vocabulary,
            registry,
            additional.indexed,
        )?
        .relation_count;
        order_native_graph_rows(
            stage,
            limits.finalize.max_rows,
            limits.finalize.max_work_bytes,
        )?;
        bind_native_claim_contexts(stage, vocabulary, &contexts, limits.claims, limits.finalize)?;
        let roots = stage.core_roots()?;
        let inherited = prepare_global_inherited_views(
            stage,
            &CompleteRelationSeal {
                source_cut: navigation.source_cut.clone(),
                relation_count: roots.relations,
                relation_root_sha256: roots.relation_sha256,
            },
            limits.inherited,
        )?;
        let final_rows = finalize_native_graph_rows_with_witnesses(
            stage,
            registry,
            entity_bytes,
            &inherited,
            limits.finalize,
            |stage, graph, reference| {
                claim_context_sources(stage, &contexts, graph, reference, limits.claims)
            },
            |stage, relation, graph, id, _native| {
                if let Some((prepared, _)) = canon
                    .iter()
                    .chain(candidates.iter())
                    .find(|(p, _)| p.source_graph == graph)
                {
                    return source_material(
                        stage,
                        prepared,
                        id,
                        relation,
                        limits.finalize.max_row_bytes,
                    )
                    .map(Some);
                }
                if let Some(prepared) = repository.as_ref().filter(|p| p.source_graph == graph) {
                    return repository_material_witness(
                        stage,
                        prepared,
                        relation,
                        id,
                        additional.topology,
                    )
                    .map(Some);
                }
                Ok(None)
            },
        )?;
        // Complete actual rows now own final roots. Cleanup checks raw family
        // coverage and dependency roots again, then removes private joins only.
        let mut absence = Digest256Hasher::new();
        absence.update(
            Digest256::from_hex(&placeholder.placeholder_absence_root_sha256)
                .map_err(|_| Error::Invalid("native placeholder absence root"))?
                .as_bytes(),
        );
        absence.update(
            Digest256::from_hex(&prepared_absence)
                .map_err(|_| Error::Invalid("native prepared absence root"))?
                .as_bytes(),
        );
        absence.update(
            Digest256::from_hex(&extra_absence)
                .map_err(|_| Error::Invalid("native additional absence root"))?
                .as_bytes(),
        );
        let absence = absence.finalize().to_hex();
        let nav_placeholders:u64=stage.with_connection(WritePhase::Finalize,|db|Ok(db.query_row(
            "SELECT count(*) FROM knowledge_nodes WHERE source_graph=?1 AND kind_id='relation-endpoint'",
            [&navigation.source_graph],|r|r.get(0))?))?;
        clear_navigation_relation_dependencies(
            stage,
            &navigation,
            &dependencies,
            &NavigationRelationCompletionProof {
                source_cut: navigation.source_cut.clone(),
                relation_dependency_root_sha256: dependencies.dependency_root_sha256.clone(),
                endpoint_title_root_sha256: titles.title_root_sha256.clone(),
                claim_group_root_sha256: contexts.root_sha256.clone(),
                consumed_edges: relation.edge_count,
            },
        )?;
        clear_source_navigation_prepare(
            stage,
            vocabulary,
            navigation_header,
            &navigation,
            &NavigationJoinClosure {
                source_graph: navigation.source_graph.clone(),
                source_cut: navigation.source_cut.clone(),
                prepared_dependency_root_sha256: navigation.dependency_root_sha256.clone(),
                raw_node_count: nav_base.node_count,
                raw_node_input_root_sha256: nav_base.node_input_root_sha256.clone(),
                raw_edge_count: relation.edge_count,
                raw_edge_input_root_sha256: relation.edge_input_root_sha256.clone(),
                placeholder_count: nav_placeholders,
                placeholder_absence_root_sha256: absence.clone(),
                core_node_count: final_rows.nodes,
                core_node_root_sha256: final_rows.node_root_sha256.clone(),
                core_relation_count: final_rows.relations,
                core_relation_root_sha256: final_rows.relation_root_sha256.clone(),
            },
            limits.navigation_prepare,
        )?;
        if let Some((prepared, _)) = &philosophy {
            clear_philosophy_prepare(
                stage,
                vocabulary,
                prepared,
                limits.philosophy_prepare,
                final_rows.nodes,
                &final_rows.node_root_sha256,
                final_rows.relations,
                &final_rows.relation_root_sha256,
            )?;
        }
        let final_nodes = CompleteBaseNodes {
            source_cut: navigation.source_cut.clone(),
            node_count: final_rows.nodes,
            node_root_sha256: final_rows.node_root_sha256.clone(),
        };
        if let Some(prepared) = &repository {
            clear_repository_topology(
                stage,
                prepared,
                &final_nodes,
                final_rows.relations,
                &final_rows.relation_root_sha256,
                additional.topology,
            )?;
        }
        if let Some(prepared) = &semantic {
            clear_semantic_joins(
                stage,
                prepared,
                &final_nodes,
                final_rows.relations,
                &final_rows.relation_root_sha256,
                additional.topology,
            )?;
        }
        if !canon_prepared.is_empty() {
            for prepared in &canon_prepared {
                let dependency =
                    crate::knowledge_canon_prepare::dependency_root(stage, &prepared.source_graph)?;
                let (nodes,placeholder_nodes,relations):(u64,u64,u64)=stage.with_connection(WritePhase::Finalize,|db|Ok((
                    db.query_row("SELECT count(*) FROM knowledge_nodes WHERE source_graph=?1",[&prepared.source_graph],|r|r.get(0))?,
                    db.query_row("SELECT count(*) FROM knowledge_nodes WHERE source_graph=?1 AND kind_id='relation-endpoint'",[&prepared.source_graph],|r|r.get(0))?,
                    db.query_row("SELECT count(*) FROM knowledge_relations WHERE source_graph=?1",[&prepared.source_graph],|r|r.get(0))?)))?;
                if dependency != prepared.dependency_root_sha256
                    || nodes
                        != prepared
                            .nodes
                            .checked_add(placeholder_nodes)
                            .ok_or(Error::Budget("native canon coverage"))?
                    || relations
                        != prepared
                            .relation_edges
                            .checked_add(prepared.node_relations)
                            .ok_or(Error::Budget("native canon coverage"))?
                {
                    return Err(Error::Invalid("native canon complete family closure"));
                }
            }
            clear_canon_prepare(stage)?;
        }
        // Even a navigation-only profile creates one complete empty group.
        if let Some((prepared, _)) = &claims {
            clear_source_claim_indices(stage, prepared, &contexts, limits.claims)?;
        } else {
            stage.with_connection(WritePhase::Finalize, |db| {
                db.execute_batch("DROP TABLE knowledge_claim_context_groups")?;
                Ok(())
            })?;
        }
        clear_global_titles(stage)?;
        clear_inherited_views(stage)?;
        let corpus_original = additional
            .corpus_original
            .map(|plan| crate::retain_captured_corpus_original(stage, vocabulary, plan))
            .transpose()?;
        Ok(NativeProducerReceipt {
            final_rows,
            navigation_original,
            philosophy_original,
            corpus_original,
            base_node_root_sha256: base.node_root_sha256,
            endpoint_title_root_sha256: titles.title_root_sha256,
            claim_group_root_sha256: contexts.root_sha256,
            placeholder_absence_root_sha256: absence,
            navigation_nodes: nav_base.node_count,
            navigation_relations: relation.edge_count,
            claim_nodes,
            claim_relations,
            philosophy_nodes,
            philosophy_relations,
            canon_nodes: canon.as_ref().map(|(p, _)| p.nodes).unwrap_or(0),
            canon_relations,
            candidate_relations,
            repository_nodes: repository.as_ref().map(|p| p.nodes).unwrap_or(0),
            repository_relations,
            semantic_relations,
            indexed_nodes,
            indexed_relations,
        })
    })();
    if result.is_err() {
        stage.poison();
    }
    result
}
