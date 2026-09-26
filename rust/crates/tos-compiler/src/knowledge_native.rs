//! Complete native navigation/Claim/philosophy assembly, in source dependency
//! order. Other adapter families refuse until their own producer is present.
//! This writes final private rows; semantic admission and owner selection are
//! independent gates on the returned candidate.

use crate::knowledge_normalization::SourceRow;
use crate::knowledge_source_claims::{
    ClaimNormalizeLimits, ClaimNormalizer, claim_context_sources, claim_contexts,
    clear_source_claim_indices, finalize_source_claims, materialize_source_claim_nodes,
    materialize_source_claim_relations, prepare_claim_context_groups,
};
use crate::knowledge_source_navigation_prepare::{
    NavigationJoinClosure, clear_source_navigation_prepare,
};
use crate::knowledge_source_navigation_relation::{
    NavigationRelationCompletionProof, clear_navigation_relation_dependencies,
};
use crate::knowledge_stage::{KnowledgeStage, NodeRow, WritePhase};
use crate::*;
use serde_json::Value;
use tos_foundation::{Digest256, Digest256Hasher};

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

#[derive(Clone, Debug)]
pub struct NativeProducerReceipt {
    pub final_rows: NativeFinalizeReceipt,
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
            let page = stage.scan_input(
                &entry.source_graph,
                "edges",
                after.as_deref(),
                limits.max_page_rows,
            )?;
            for raw in page.rows {
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
                    if placeholders > limits.max_placeholders || work > limits.max_work_bytes {
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
            || vocabulary.sources.iter().any(|s| {
                !matches!(
                    s.adapter_profile.as_str(),
                    "source-navigation-node-edge-v1"
                        | "reified-bibliographic-claims-v1"
                        | "philosophy-node-edge-v1"
                )
            })
        {
            return Err(Error::Invalid("native source adapter family incomplete"));
        }
        let navigation = prepare_source_navigation(
            stage,
            vocabulary,
            navigation_header,
            limits.navigation_prepare,
        )?;
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
        let philosophy = if selected("philosophy-node-edge-v1")? {
            let prepared = prepare_philosophy(stage, vocabulary, limits.philosophy_prepare)?;
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
        order_native_graph_rows(
            stage,
            limits.finalize.max_rows,
            limits.finalize.max_work_bytes,
        )?;
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
        let final_rows = finalize_native_graph_rows(
            stage,
            registry,
            entity_bytes,
            &inherited,
            limits.finalize,
            |stage, graph, reference| {
                claim_context_sources(stage, &contexts, graph, reference, limits.claims)
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
        Ok(NativeProducerReceipt {
            final_rows,
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
        })
    })();
    if result.is_err() {
        stage.poison();
    }
    result
}
