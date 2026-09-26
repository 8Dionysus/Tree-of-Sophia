//! Bounded assembly of native navigation base rows into the private stage.
//! All-source admission, node finalization and final ordering remain caller
//! obligations. Raw owner rows remain available for readable-context assembly.

use crate::knowledge_normalization::SourceRow;
use crate::knowledge_stage::{KnowledgeStage, NodeRow, RelationRow, SeekRow, WritePhase};
use crate::{
    CompleteBaseNodes, Error, NavigationEndpoint, NavigationNodeNormalizer,
    NavigationPrepareReceipt, NavigationRelationDependencyReceipt, NavigationRelationGlobalInputs,
    NavigationRelationNormalizer, Result,
};
use serde_json::Value;
use tos_foundation::{Digest256, Digest256Hasher};

#[derive(Clone, Copy, Debug)]
pub struct NavigationMaterializeLimits {
    pub max_nodes: u64,
    pub max_edges: u64,
    pub max_placeholders: u64,
    pub max_page_rows: usize,
    pub max_raw_bytes: usize,
    pub max_output_bytes: usize,
    pub max_page_bytes: usize,
    pub max_work_bytes: u64,
}
impl NavigationMaterializeLimits {
    fn validate(self) -> Result<()> {
        if self.max_nodes == 0
            || self.max_edges == 0
            || self.max_placeholders == 0
            || self.max_page_rows == 0
            || self.max_page_rows > 1024
            || self.max_raw_bytes == 0
            || self.max_raw_bytes > 8 * 1024 * 1024
            || self.max_output_bytes == 0
            || self.max_output_bytes > 8 * 1024 * 1024
            || self.max_page_bytes == 0
            || self.max_page_bytes > 64 * 1024 * 1024
            || self
                .max_page_rows
                .checked_mul(self.max_raw_bytes)
                .is_none_or(|n| n > self.max_page_bytes)
            || self.max_work_bytes == 0
        {
            return Err(Error::Budget("navigation materialization limits"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct NavigationNodeMaterializeReceipt {
    pub source_graph: String,
    pub source_cut: String,
    pub node_count: u64,
    pub node_input_root_sha256: String,
    pub prepared_dependency_root_sha256: String,
    pub materialized_node_root_sha256: String,
}
#[derive(Clone, Debug)]
pub struct NavigationPlaceholderReceipt {
    pub source_graph: String,
    pub source_cut: String,
    pub base_node_count: u64,
    pub base_node_root_sha256: String,
    pub edge_count: u64,
    pub edge_input_root_sha256: String,
    pub placeholder_count: u64,
    pub placeholder_absence_root_sha256: String,
    pub materialized_placeholder_root_sha256: String,
}
#[derive(Clone, Debug)]
pub struct NavigationRelationMaterializeReceipt {
    pub source_graph: String,
    pub source_cut: String,
    pub edge_count: u64,
    pub edge_input_root_sha256: String,
    pub prepared_dependency_root_sha256: String,
    pub relation_dependency_root_sha256: String,
    pub endpoint_title_root_sha256: String,
    pub claim_group_root_sha256: String,
    pub materialized_relation_root_sha256: String,
}

fn required<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.len() <= 4096)
        .ok_or(Error::Invalid("navigation materialized row field"))
}
fn root_text(hash: &mut Digest256Hasher, text: &str) {
    hash.update(&(text.len() as u64).to_be_bytes());
    hash.update(text.as_bytes());
}
fn root_item(hash: &mut Digest256Hasher, id: &str, sha: &str) -> Result<()> {
    root_text(hash, id);
    hash.update(
        Digest256::from_hex(sha)
            .map_err(|_| Error::Invalid("navigation materialization digest"))?
            .as_bytes(),
    );
    Ok(())
}
fn charge(work: &mut u64, bytes: usize, limits: NavigationMaterializeLimits) -> Result<()> {
    *work = work
        .checked_add(bytes as u64)
        .filter(|n| *n <= limits.max_work_bytes)
        .ok_or(Error::Budget("navigation materialization work bytes"))?;
    Ok(())
}
fn binding(stage: &KnowledgeStage<'_>, prepared: &NavigationPrepareReceipt) -> Result<()> {
    if prepared.source_cut != stage.exact_receipt().binding.source_cut
        || prepared.final_graph_rows_written
        || Digest256::from_hex(&prepared.dependency_root_sha256).is_err()
    {
        return Err(Error::Invalid(
            "navigation materialization prepared binding",
        ));
    }
    Ok(())
}
fn next_order(stage: &mut KnowledgeStage<'_>, relation: bool) -> Result<i64> {
    stage.with_connection(WritePhase::Sort, |db| {
        let value: Option<i64> = db.query_row(
            if relation {
                "SELECT max(source_order) FROM knowledge_relations"
            } else {
                "SELECT max(source_order) FROM knowledge_nodes"
            },
            [],
            |r| r.get(0),
        )?;
        value
            .unwrap_or(-1)
            .checked_add(1)
            .ok_or(Error::Budget("navigation source order"))
    })
}

// Check length before scan_input allocates its page. The stage's own row cap
// can be broader than this adapter's declared raw/page budget.
fn preflight_input(
    stage: &mut KnowledgeStage<'_>,
    source: &str,
    collection: &str,
    count: u64,
    cap: u64,
    limits: NavigationMaterializeLimits,
) -> Result<()> {
    if count > cap {
        return Err(Error::Budget("navigation registered materialization count"));
    }
    stage.with_connection(WritePhase::Sort, |db| {
        let (actual,bad):(u64,u64)=db.query_row(
            "SELECT count(*),coalesce(sum(CASE WHEN typeof(payload)!='blob' OR payload_len!=length(payload) OR length(payload)>?3 THEN 1 ELSE 0 END),0) FROM raw_records WHERE source_graph=?1 AND collection=?2",
            rusqlite::params![source,collection,limits.max_raw_bytes as i64],|r|Ok((r.get(0)?,r.get(1)?)))?;
        if actual!=count { return Err(Error::Invalid("navigation materialization input coverage")); }
        if bad!=0 {return Err(Error::Budget("navigation materialization raw row bytes"));} Ok(())
    })
}
fn append_node(
    stage: &mut KnowledgeStage<'_>,
    value: &Value,
    order: &mut i64,
    limits: NavigationMaterializeLimits,
    work: &mut u64,
) -> Result<String> {
    let payload =
        serde_json::to_vec(value).map_err(|_| Error::Invalid("navigation node output JSON"))?;
    if payload.len() > limits.max_output_bytes {
        return Err(Error::Budget("navigation node output bytes"));
    }
    charge(work, payload.len(), limits)?;
    stage.insert_node(NodeRow {
        id: required(value, "id")?,
        source_graph: required(value, "source_graph")?,
        native_id: Some(required(value, "native_id")?),
        entity_id: Some(required(value, "entity_id")?),
        kind_id: required(value, "kind_id")?,
        type_id: required(value, "type_id")?,
        source_order: *order,
        payload: &payload,
    })?;
    *order = order
        .checked_add(1)
        .ok_or(Error::Budget("navigation source order"))?;
    Ok(Digest256::of_bytes(&payload).to_hex())
}
struct Walk {
    count: u64,
    hash: Digest256Hasher,
    after: Option<String>,
    work: u64,
}
impl Walk {
    fn new() -> Self {
        Self {
            count: 0,
            hash: Digest256Hasher::new(),
            after: None,
            work: 0,
        }
    }
    fn add(&mut self, raw: &SeekRow, cap: u64, limits: NavigationMaterializeLimits) -> Result<()> {
        if raw.payload.len() > limits.max_raw_bytes {
            return Err(Error::Budget("navigation materialization raw bytes"));
        }
        charge(&mut self.work, raw.payload.len(), limits)?;
        self.count = self
            .count
            .checked_add(1)
            .filter(|n| *n <= cap)
            .ok_or(Error::Budget("navigation materialization row count"))?;
        root_item(&mut self.hash, &raw.id, &raw.payload_sha256)
    }
    fn complete(self, count: u64, root: &str) -> Result<()> {
        if self.count != count || self.hash.finalize().to_hex() != root {
            return Err(Error::Invalid(
                "navigation materialization input count/root",
            ));
        }
        Ok(())
    }
}

pub fn materialize_navigation_nodes(
    stage: &mut KnowledgeStage<'_>,
    normalizer: &mut NavigationNodeNormalizer<'_>,
    prepared: &NavigationPrepareReceipt,
    limits: NavigationMaterializeLimits,
) -> Result<NavigationNodeMaterializeReceipt> {
    let result = (|| {
        limits.validate()?;
        binding(stage, prepared)?;
        preflight_input(
            stage,
            &prepared.source_graph,
            "nodes",
            prepared.nodes,
            limits.max_nodes,
            limits,
        )?;
        let mut order = next_order(stage, false)?;
        let mut walk = Walk::new();
        let mut output = Digest256Hasher::new();
        loop {
            let page = stage.scan_input(
                &prepared.source_graph,
                "nodes",
                walk.after.as_deref(),
                limits.max_page_rows,
            )?;
            for raw in &page.rows {
                walk.add(raw, limits.max_nodes, limits)?;
                let base = normalizer.normalize_base(raw, prepared)?;
                let sha = append_node(stage, base.value(), &mut order, limits, &mut walk.work)?;
                root_item(&mut output, required(base.value(), "id")?, &sha)?;
            }
            match page.next_id {
                Some(next) => walk.after = Some(next),
                None => break,
            }
        }
        walk.complete(prepared.nodes, &prepared.node_input_root_sha256)?;
        Ok(NavigationNodeMaterializeReceipt {
            source_graph: prepared.source_graph.clone(),
            source_cut: prepared.source_cut.clone(),
            node_count: prepared.nodes,
            node_input_root_sha256: prepared.node_input_root_sha256.clone(),
            prepared_dependency_root_sha256: prepared.dependency_root_sha256.clone(),
            materialized_node_root_sha256: output.finalize().to_hex(),
        })
    })();
    if result.is_err() {
        stage.poison();
    }
    result
}

/// Run only after every source has materialized its base nodes. The seal is
/// matched against the entire stage before the first placeholder is added.
pub fn materialize_navigation_placeholders(
    stage: &mut KnowledgeStage<'_>,
    normalizer: &mut NavigationNodeNormalizer<'_>,
    prepared: &NavigationPrepareReceipt,
    complete: &CompleteBaseNodes,
    limits: NavigationMaterializeLimits,
) -> Result<NavigationPlaceholderReceipt> {
    let result = (|| {
        limits.validate()?;
        binding(stage, prepared)?;
        preflight_input(
            stage,
            &prepared.source_graph,
            "edges",
            prepared.edges,
            limits.max_edges,
            limits,
        )?;
        let roots = stage.core_roots()?;
        if complete.source_cut != prepared.source_cut
            || complete.node_count != roots.nodes
            || complete.node_root_sha256 != roots.node_sha256
        {
            return Err(Error::Invalid("navigation placeholder complete base root"));
        }
        let mut absent = Digest256Hasher::new();
        absent.update(b"tos-navigation-global-absence-v1\0");
        root_text(&mut absent, &complete.source_cut);
        root_item(&mut absent, "all-base-nodes", &complete.node_root_sha256)?;
        absent.update(&complete.node_count.to_be_bytes());
        let mut output = Digest256Hasher::new();
        let mut count = 0u64;
        let mut order = next_order(stage, false)?;
        let mut walk = Walk::new();
        loop {
            let page = stage.scan_input(
                &prepared.source_graph,
                "edges",
                walk.after.as_deref(),
                limits.max_page_rows,
            )?;
            for raw in &page.rows {
                walk.add(raw, limits.max_edges, limits)?;
                let source = SourceRow::parse(&raw.payload, limits.max_raw_bytes)?;
                for (endpoint, key, source_key, role) in [
                    (
                        NavigationEndpoint::From,
                        "from_id",
                        "from_source_graph",
                        "from",
                    ),
                    (NavigationEndpoint::To, "to_id", "to_source_graph", "to"),
                ] {
                    let native = required(source.value(), key)?;
                    let graph = source
                        .value()
                        .get(source_key)
                        .and_then(Value::as_str)
                        .unwrap_or(&prepared.source_graph);
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
                    let base = normalizer.normalize_placeholder(raw, prepared, endpoint)?;
                    if required(base.value(), "id")? != id {
                        return Err(Error::Invalid("navigation placeholder endpoint identity"));
                    }
                    count = count
                        .checked_add(1)
                        .filter(|n| *n <= limits.max_placeholders)
                        .ok_or(Error::Budget("navigation placeholder rows"))?;
                    root_item(&mut absent, &raw.id, &raw.payload_sha256)?;
                    root_text(&mut absent, role);
                    root_text(&mut absent, &id);
                    let sha = append_node(stage, base.value(), &mut order, limits, &mut walk.work)?;
                    root_item(&mut output, &id, &sha)?;
                }
            }
            match page.next_id {
                Some(next) => walk.after = Some(next),
                None => break,
            }
        }
        walk.complete(prepared.edges, &prepared.edge_input_root_sha256)?;
        Ok(NavigationPlaceholderReceipt {
            source_graph: prepared.source_graph.clone(),
            source_cut: prepared.source_cut.clone(),
            base_node_count: complete.node_count,
            base_node_root_sha256: complete.node_root_sha256.clone(),
            edge_count: prepared.edges,
            edge_input_root_sha256: prepared.edge_input_root_sha256.clone(),
            placeholder_count: count,
            placeholder_absence_root_sha256: absent.finalize().to_hex(),
            materialized_placeholder_root_sha256: output.finalize().to_hex(),
        })
    })();
    if result.is_err() {
        stage.poison();
    }
    result
}

/// The callback performs exact, bounded lookups in the independently checked
/// global title/Claim indexes. Its root values remain explicit in the receipt.
pub fn materialize_navigation_relations<F>(
    stage: &mut KnowledgeStage<'_>,
    normalizer: &NavigationRelationNormalizer<'_>,
    prepared: &NavigationPrepareReceipt,
    dependency: &NavigationRelationDependencyReceipt,
    endpoint_title_root_sha256: &str,
    claim_group_root_sha256: &str,
    limits: NavigationMaterializeLimits,
    mut global_inputs: F,
) -> Result<NavigationRelationMaterializeReceipt>
where
    F: FnMut(
        &mut KnowledgeStage<'_>,
        &str,
        &str,
        Option<&str>,
    ) -> Result<(Value, Value, Vec<Value>)>,
{
    let result = (|| {
        limits.validate()?;
        binding(stage, prepared)?;
        preflight_input(
            stage,
            &prepared.source_graph,
            "edges",
            prepared.edges,
            limits.max_edges,
            limits,
        )?;
        for root in [endpoint_title_root_sha256, claim_group_root_sha256] {
            Digest256::from_hex(root)
                .map_err(|_| Error::Invalid("navigation materialization global root"))?;
        }
        let mut order = next_order(stage, true)?;
        let mut walk = Walk::new();
        let mut output = Digest256Hasher::new();
        loop {
            let page = stage.scan_input(
                &prepared.source_graph,
                "edges",
                walk.after.as_deref(),
                limits.max_page_rows,
            )?;
            for raw in &page.rows {
                walk.add(raw, limits.max_edges, limits)?;
                let source = SourceRow::parse(&raw.payload, limits.max_raw_bytes)?;
                let item = source.value();
                let address = |key, graph_key| -> Result<String> {
                    Ok(format!(
                        "{}:{}",
                        item.get(graph_key)
                            .and_then(Value::as_str)
                            .unwrap_or(&prepared.source_graph),
                        required(item, key)?
                    ))
                };
                let from = address("from_id", "from_source_graph")?;
                let to = address("to_id", "to_source_graph")?;
                let (left, right, contexts) = global_inputs(
                    stage,
                    &from,
                    &to,
                    item.get("claim_ref").and_then(Value::as_str),
                )?;
                // normalize_indexed performs a second exact raw seek.
                charge(&mut walk.work, raw.payload.len(), limits)?;
                let base = normalizer.normalize_indexed(
                    stage,
                    &raw.id,
                    prepared,
                    dependency,
                    NavigationRelationGlobalInputs {
                        source_cut: &prepared.source_cut,
                        endpoint_title_root_sha256,
                        claim_group_root_sha256,
                        left_title: &left,
                        right_title: &right,
                        referenced_claim_contexts: &contexts,
                    },
                )?;
                let value = base.value();
                let payload = serde_json::to_vec(value)
                    .map_err(|_| Error::Invalid("navigation relation output JSON"))?;
                if payload.len() > limits.max_output_bytes {
                    return Err(Error::Budget("navigation relation output bytes"));
                }
                charge(&mut walk.work, payload.len(), limits)?;
                stage.insert_relation(RelationRow {
                    id: required(value, "id")?,
                    source_graph: required(value, "source_graph")?,
                    native_id: Some(required(value, "native_id")?),
                    from_id: required(value, "from_id")?,
                    to_id: required(value, "to_id")?,
                    predicate_id: required(value, "predicate_id")?,
                    relation_type_id: required(value, "relation_type_id")?,
                    source_order: order,
                    payload: &payload,
                })?;
                order = order
                    .checked_add(1)
                    .ok_or(Error::Budget("navigation source order"))?;
                root_item(
                    &mut output,
                    required(value, "id")?,
                    &Digest256::of_bytes(&payload).to_hex(),
                )?;
            }
            match page.next_id {
                Some(next) => walk.after = Some(next),
                None => break,
            }
        }
        walk.complete(prepared.edges, &prepared.edge_input_root_sha256)?;
        Ok(NavigationRelationMaterializeReceipt {
            source_graph: prepared.source_graph.clone(),
            source_cut: prepared.source_cut.clone(),
            edge_count: prepared.edges,
            edge_input_root_sha256: prepared.edge_input_root_sha256.clone(),
            prepared_dependency_root_sha256: prepared.dependency_root_sha256.clone(),
            relation_dependency_root_sha256: dependency.dependency_root_sha256.clone(),
            endpoint_title_root_sha256: endpoint_title_root_sha256.into(),
            claim_group_root_sha256: claim_group_root_sha256.into(),
            materialized_relation_root_sha256: output.finalize().to_hex(),
        })
    })();
    if result.is_err() {
        stage.poison();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge_stage::{
        ExactInputReceipt, InputCollectionReceipt, InputRow, StageIsolation, StageLimits,
        StageOwner,
    };
    use crate::{
        KnowledgeRegistry, Limits, NavigationHeaderClaim, NavigationNodeLimits,
        NavigationPrepareLimits, NavigationRelationLimits, NavigationRelationNormalizeLimits,
        QueryVocabulary, SourceBinding, prepare_navigation_relation_dependencies,
        prepare_source_navigation,
    };
    use serde_json::json;
    use std::{
        fs,
        path::Path,
        time::{SystemTime, UNIX_EPOCH},
    };
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

    #[test]
    fn native_stage_materialization_preserves_endpoint_identity_and_raw_context() {
        let descriptor = include_bytes!("../tests/fixtures/query-vocabulary.v1.json");
        let document: Value = serde_json::from_slice(descriptor).unwrap();
        let mut profiles = document["sources"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["adapter_profile"].as_str().unwrap())
            .collect::<Vec<_>>();
        profiles.push(document["extension_adapter_profile"].as_str().unwrap());
        let vocabulary = QueryVocabulary::parse(descriptor, &profiles).unwrap();
        let entity =
            include_bytes!("../../../../ToS/doctrine/semantic-interchange/entity-types.v1.json");
        let relation =
            include_bytes!("../../../../ToS/doctrine/semantic-interchange/relation-types.v1.json");
        let registry = KnowledgeRegistry::parse(entity, relation).unwrap();
        // Exact ordinary owner envelopes already exercised by the native node
        // oracle; only the edge's explicit endpoint source changes per case.
        let node=br#"{"node_id":"tos.work.alpha","node_kind":"work","label":"Alpha","source_ref":"ToS/a.json","identity_status":"not_applicable","properties":{}}"#;
        for endpoint_graph in ["source-navigation", "canon"] {
            let edge=serde_json::to_vec(&json!({"edge_id":"e:missing","from_id":"tos.work.alpha",
                "to_id":"tos.work.missing","to_source_graph":endpoint_graph,"predicate_id":"related_to",
                "edge_kind":"authored_source_planting","review_status":"not_applicable","source_refs":["ToS/a.json"]})).unwrap();
            let input_root = |id: &str, raw: &[u8]| {
                let mut h = Digest256Hasher::new();
                root_item(&mut h, id, &Digest256::of_bytes(raw).to_hex()).unwrap();
                h.finalize().to_hex()
            };
            let mut receipt = ExactInputReceipt {
                binding: SourceBinding {
                    owner_profile: "fixture-owner".into(),
                    source_cut: "native-materialize-fixture".into(),
                    through_commit_seq: 1,
                    membership_root: "0".repeat(64),
                    index_generation: "fixture".into(),
                    route_map_version: "fixture".into(),
                    reader_abi: "fixture".into(),
                    projection_root_sha256: "1".repeat(64),
                    complete: true,
                },
                collections: [
                    ("nodes", "tos.work.alpha", node.as_slice()),
                    ("edges", "e:missing", edge.as_slice()),
                ]
                .iter()
                .map(|(collection, id, raw)| InputCollectionReceipt {
                    source_graph: "source-navigation".into(),
                    input_role: "corpus-source-navigation".into(),
                    adapter_profile: "source-navigation-node-edge-v1".into(),
                    collection: (*collection).into(),
                    expected_count: 1,
                    expected_root_sha256: input_root(id, raw),
                })
                .collect(),
            };
            receipt.collections.push(InputCollectionReceipt {
                source_graph: "canon".into(),
                input_role: "corpus-index".into(),
                adapter_profile: "canon-node-relation-v1".into(),
                collection: "nodes".into(),
                expected_count: 0,
                expected_root_sha256: Digest256::of_bytes(b"").to_hex(),
            });
            let dir = std::env::temp_dir().join(format!(
                "tos-navigation-materialize-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&dir).unwrap();
            let mut stage = KnowledgeStage::create(
                &dir.join("private.sqlite3"),
                StageLimits {
                    sqlite: Limits::default(),
                    max_temp_bytes: 64 * 1024 * 1024,
                    max_seek_rows: 1,
                    max_seek_bytes: 4096,
                },
                receipt,
                &Owner,
                &Quota,
            )
            .unwrap();
            for (collection, id, raw) in [
                ("nodes", "tos.work.alpha", node.as_slice()),
                ("edges", "e:missing", edge.as_slice()),
            ] {
                stage
                    .ingest_input(InputRow {
                        source_graph: "source-navigation",
                        collection,
                        id,
                        payload: raw,
                    })
                    .unwrap();
            }
            let raw_json=br#"{"schema_version":"tos_source_navigation_v1","authority_boundary":"fixture","counts":{"nodes":1,"edges":1,"rights":0}}"#.to_vec();
            let header = NavigationHeaderClaim {
                expected_sha256: Digest256::of_bytes(&raw_json).to_hex(),
                raw_json,
            };
            let prepared = prepare_source_navigation(
                &mut stage,
                &vocabulary,
                &header,
                NavigationPrepareLimits {
                    max_nodes: 8,
                    max_edges: 8,
                    max_endpoint_refs: 16,
                    max_page_rows: 1,
                    max_row_bytes: 4096,
                    max_header_bytes: 4096,
                    max_work_bytes: 1024 * 1024,
                },
            )
            .unwrap();
            let mut nodes = NavigationNodeNormalizer::new(
                &registry,
                entity,
                &vocabulary,
                descriptor,
                NavigationNodeLimits {
                    max_raw_bytes: 4096,
                    max_output_bytes: 32768,
                    max_ancestor_cache_bytes: 32768,
                },
            )
            .unwrap();
            let limits = NavigationMaterializeLimits {
                max_nodes: 8,
                max_edges: 8,
                max_placeholders: 8,
                max_page_rows: 1,
                max_raw_bytes: 4096,
                max_output_bytes: 32768,
                max_page_bytes: 4096,
                max_work_bytes: 1024 * 1024,
            };
            let node_receipt =
                materialize_navigation_nodes(&mut stage, &mut nodes, &prepared, limits).unwrap();
            assert_eq!(node_receipt.node_count, 1);
            assert_eq!(
                stage
                    .raw_by_id("source-navigation", "nodes", "tos.work.alpha")
                    .unwrap()
                    .unwrap()
                    .payload,
                node
            );
            let roots = stage.core_roots().unwrap();
            let complete = CompleteBaseNodes {
                source_cut: prepared.source_cut.clone(),
                node_count: roots.nodes,
                node_root_sha256: roots.node_sha256,
            };
            let placeholders = materialize_navigation_placeholders(
                &mut stage, &mut nodes, &prepared, &complete, limits,
            )
            .unwrap();
            assert_eq!(placeholders.placeholder_count, 1);
            let dependency = prepare_navigation_relation_dependencies(
                &mut stage,
                &prepared,
                &vocabulary,
                NavigationRelationLimits {
                    max_edges: 8,
                    max_page_rows: 1,
                    max_raw_bytes: 4096,
                    max_context_bytes: 32768,
                    max_page_bytes: 4096,
                    max_work_bytes: 1024 * 1024,
                },
            )
            .unwrap();
            let relations = NavigationRelationNormalizer::new(
                &registry,
                relation,
                &vocabulary,
                descriptor,
                NavigationRelationNormalizeLimits {
                    max_raw_bytes: 4096,
                    max_output_bytes: 32768,
                    max_registry_bytes: 1024 * 1024,
                    max_claim_contexts: 16,
                    max_global_input_bytes: 32768,
                },
            )
            .unwrap();
            let title_root = "2".repeat(64);
            let claim_root = "3".repeat(64);
            let result = materialize_navigation_relations(
                &mut stage,
                &relations,
                &prepared,
                &dependency,
                &title_root,
                &claim_root,
                limits,
                |stage, from, to, claim| {
                    assert!(claim.is_none());
                    let lookup = |stage: &mut KnowledgeStage<'_>, id: &str| {
                        stage.with_connection(WritePhase::Sort, |db| {
                            let raw: Vec<u8> = db.query_row(
                                "SELECT payload FROM knowledge_nodes WHERE id=?1",
                                [id],
                                |r| r.get(0),
                            )?;
                            let value: Value = serde_json::from_slice(&raw).unwrap();
                            Ok(json!({"display":{"title":value["display"]["title"]}}))
                        })
                    };
                    Ok((lookup(stage, from)?, lookup(stage, to)?, vec![]))
                },
            )
            .unwrap();
            assert_eq!(result.edge_count, 1);
            stage.with_connection(WritePhase::Sort,|db| {
                let (graph,native):(String,String)=db.query_row("SELECT source_graph,native_id FROM knowledge_nodes WHERE kind_id='relation-endpoint'",[],|r|Ok((r.get(0)?,r.get(1)?)))?;
                assert_eq!((graph,native),(endpoint_graph.into(),"tos.work.missing".into()));
                let to:String=db.query_row("SELECT to_id FROM knowledge_relations",[],|r|r.get(0))?;
                assert_eq!(to,format!("{endpoint_graph}:tos.work.missing"));Ok(())
            }).unwrap();
            drop(stage);
            fs::remove_dir_all(dir).unwrap();
        }
    }
}
