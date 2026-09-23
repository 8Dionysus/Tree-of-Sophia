//! The existing synthetic full producer fixture shared by compiler and query
//! integration tests. Enabled only for tests or the `test-fixture` feature.
//! Its owner/quota/custody stubs are not production authority.

use super::*;
use crate::{
    catalog::CatalogLimits,
    knowledge_stage::{
        ExactInputReceipt, InputCollectionReceipt, InputRow, KnowledgeStage, StageIsolation,
        StageLimits, StageOwner, WritePhase,
    },
    materialize_indexed_sources, open_selected_knowledge_model, ColdOpenLimits,
    ExpectedSourceScope, ImmutableKnowledgeCustody, IndexedLimits,
    KnowledgeSelectedExpectation, Limits, SourceBinding,
};
use serde_json::{json, Value};
use std::{
    fs,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use tos_foundation::Digest256Hasher;

struct FixtureOwner;
impl StageOwner for FixtureOwner {
    fn verify_receipt(&self, _: &ExactInputReceipt) -> Result<()> {
        Ok(())
    }
    fn recheck_sealed_cut(&self, _: &ExactInputReceipt) -> Result<()> {
        Ok(())
    }
}
struct FixtureQuota;
impl StageIsolation for FixtureQuota {
    fn verify(&self, _: &Path, _: StageLimits, _: WritePhase) -> Result<()> {
        Ok(())
    }
}
// Synthetic fixture custody only. Production must hold a real immutable
// generation and separately enforce cold temp/heap quotas.
struct FixtureCustody;
impl ImmutableKnowledgeCustody for FixtureCustody {
    fn verify(&self, pinned: &fs::File, expected: &KnowledgeSelectedExpectation) -> Result<()> {
        if expected.owner_receipt_id != "fixture-owner-receipt"
            || pinned.metadata()?.len() != expected.model_size_bytes
        {
            return Err(Error::Invalid("synthetic custody mismatch"));
        }
        Ok(())
    }
    fn verify_cold_resources(&self, _: ColdOpenLimits) -> Result<()> {
        Ok(())
    }
}

pub struct FullKnowledgeFixture {
    pub path: PathBuf,
    pub expectation: KnowledgeSelectedExpectation,
    pub vocabulary: QueryVocabulary,
    pub descriptor_bytes: Vec<u8>,
    pub graph_input_bytes: Vec<u8>,
    custody: FixtureCustody,
}
impl FullKnowledgeFixture {
    pub fn open(&self) -> Result<VerifiedKnowledgeModel<'_>> {
        open_selected_knowledge_model(
            &self.path,
            self.expectation.clone(),
            &self.custody,
            ColdOpenLimits {
                max_file_bytes: 64 * 1024 * 1024,
                max_vm_steps: 100_000_000,
                sqlite_cache_kib: 8192,
                max_rows: 100_000,
                max_work_bytes: 100 * 1024 * 1024,
                max_row_bytes: 1024 * 1024,
                max_metadata_bytes: 256 * 1024,
                max_sources: 2,
            },
        )
    }
}
impl Drop for FullKnowledgeFixture {
    fn drop(&mut self) {
        fs::remove_dir_all(self.path.parent().expect("fixture parent")).ok();
    }
}

fn root(id: &str, payload: &[u8]) -> String {
    roots(&[(id, payload)])
}
fn roots(rows: &[(&str, &[u8])]) -> String {
    let mut hash = Digest256Hasher::new();
    let mut sorted = rows.to_vec();
    sorted.sort_by_key(|(id, _)| *id);
    for (id, payload) in sorted {
        hash.update(&(id.len() as u64).to_be_bytes());
        hash.update(id.as_bytes());
        hash.update(Digest256::of_bytes(payload).as_bytes());
    }
    hash.finalize().to_hex()
}
fn candidate() -> std::path::PathBuf {
    let tick = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir =
        std::env::temp_dir().join(format!("tos-full-eighth-{}-{tick}", std::process::id()));
    fs::create_dir(&dir).unwrap();
    dir.join("candidate.sqlite3")
}

pub fn build_fixture() -> FullKnowledgeFixture {
    let entity_bytes =
        include_bytes!("../../../../ToS/doctrine/semantic-interchange/entity-types.v1.json");
    let relation_bytes =
        include_bytes!("../../../../ToS/doctrine/semantic-interchange/relation-types.v1.json");
    let registry = KnowledgeRegistry::parse(entity_bytes, relation_bytes).unwrap();
    let mut descriptor: Value = serde_json::from_slice(include_bytes!(
        "../../../../ToS/doctrine/semantic-interchange/query-vocabulary.v1.json"
    ))
    .unwrap();
    descriptor["sources"] = json!([
        {"source_graph_id":"eighth", "owner_ref":"owner:eighth",
         "input_role":"source-graph", "adapter_profile":"indexed-node-edge-v1",
         "representative_priority":0},
        {"source_graph_id":"zero", "owner_ref":"owner:zero",
         "input_role":"source-graph", "adapter_profile":"indexed-node-edge-v1",
         "representative_priority":1}
    ]);
    descriptor["identity"]["source_dossier_graph_id"] = json!("eighth");
    let descriptor_bytes = serde_json::to_vec(&descriptor).unwrap();
    let vocabulary =
        QueryVocabulary::parse(&descriptor_bytes, &["indexed-node-edge-v1"]).unwrap();
    let mut divergent = vocabulary.clone();
    divergent.registered_source_ids.push("phantom".into());
    assert!(divergent.verify_authored_bytes(&descriptor_bytes).is_err());
    let node_id = "eighth:node-1";
    let relation_id = "eighth:relation-1";
    let node = serde_json::to_vec(&json!({
        "id":node_id,"native_id":"node-1","entity_id":"external.subject.1",
        "source_graph":"eighth","kind_id":"unknown-owner-kind","type_id":"tos.entity.unmapped",
        "type_mapping":{"status":"unmapped","source_kind_id":"unknown-owner-kind"},
        "content_revision":"0".repeat(64),
        "display":{"kind_label":{"default":"Owner kind"},"title":{"default":"Example"},
            "summary_state":"source","provenance":{"source_summary_available":true}},
        "epistemic":{},"attributes":{},"semantics":{},"graph_layers":[],"view_ids":[],
        "source_refs":["owner:record-1"]
    }))
    .unwrap();
    let extra_node = |id: &str, native: &str, title: &str| {
        serde_json::to_vec(&json!({
            "id":id,"native_id":native,"entity_id":format!("external.subject.{native}"),
            "source_graph":"eighth","kind_id":"unknown-owner-kind","type_id":"tos.entity.unmapped",
            "type_mapping":{"status":"unmapped","source_kind_id":"unknown-owner-kind"},
            "content_revision":"0".repeat(64),
            "display":{"kind_label":{"default":"Owner kind"},"title":{"default":title},
                "summary_state":"source","provenance":{"source_summary_available":true}},
            "epistemic":{},"attributes":{},"semantics":{},"graph_layers":[],"view_ids":[],
            "source_refs":["owner:record-1"]
        })).unwrap()
    };
    let alpha_id = "eighth:alpha";
    let visible_id = "eighth:visible";
    let false_positive_id = "eighth:alp-false-positive";
    let alpha = extra_node(alpha_id, "alpha", "Alpha");
    let visible = extra_node(visible_id, "visible", "Alpha Beta");
    let false_positive = extra_node(false_positive_id, "alp-false-positive", "Alpine");
    let node_rows: [(&str, &[u8]); 4] = [
        (node_id, &node),
        (alpha_id, &alpha),
        (visible_id, &visible),
        (false_positive_id, &false_positive),
    ];
    let node_root = roots(&node_rows);
    let zero_root = roots(&[]);
    let relation = serde_json::to_vec(&json!({
        "id":relation_id,"native_id":"relation-1","from_id":node_id,"to_id":node_id,
        "source_graph":"eighth","predicate_id":"unknown-owner-relation",
        "relation_type_id":"tos.relation.unmapped",
        "predicate_mapping":{"status":"unmapped","source_predicate_id":"unknown-owner-relation"},
        "content_revision":"1".repeat(64),
        "display":{"label":{"default":"Owner relation"},"explanation_state":"source",
            "provenance":{"source_explanation_available":true}},
        "epistemic":{},"attributes":{},"semantics":{},"graph_layers":[],"view_ids":[],
        "source_refs":["owner:record-1"]
    })).unwrap();
    let sealed = ExactInputReceipt {
        binding: SourceBinding {
            owner_profile: "fixture-owner".into(),
            source_cut: "fixture-cut".into(),
            through_commit_seq: 7,
            membership_root: "0".repeat(64),
            index_generation: "fixture-generation".into(),
            route_map_version: "fixture-routes".into(),
            reader_abi: "fixture-reader".into(),
            projection_root_sha256: "1".repeat(64),
            complete: true,
        },
        collections: vec![
            InputCollectionReceipt {
                source_graph: "eighth".into(),
                collection: "nodes".into(),
                input_role: "source-graph".into(),
                adapter_profile: "indexed-node-edge-v1".into(),
                expected_count: 4,
                expected_root_sha256: node_root.clone(),
            },
            InputCollectionReceipt {
                source_graph: "eighth".into(),
                collection: "relations".into(),
                input_role: "source-graph".into(),
                adapter_profile: "indexed-node-edge-v1".into(),
                expected_count: 1,
                expected_root_sha256: root(relation_id, &relation),
            },
            InputCollectionReceipt {
                source_graph: "zero".into(),
                collection: "nodes".into(),
                input_role: "source-graph".into(),
                adapter_profile: "indexed-node-edge-v1".into(),
                expected_count: 0,
                expected_root_sha256: zero_root.clone(),
            },
            InputCollectionReceipt {
                source_graph: "zero".into(),
                collection: "relations".into(),
                input_role: "source-graph".into(),
                adapter_profile: "indexed-node-edge-v1".into(),
                expected_count: 0,
                expected_root_sha256: zero_root.clone(),
            },
        ],
    };
    let path = candidate();
    let owner = FixtureOwner;
    let quota = FixtureQuota;
    let mut stage = KnowledgeStage::create(
        &path,
        StageLimits {
            sqlite: Limits::default(),
            max_temp_bytes: 64 * 1024 * 1024,
            max_seek_rows: 8,
            max_seek_bytes: 1024 * 1024,
        },
        sealed,
        &owner,
        &quota,
    )
    .unwrap();
    for (collection, id, payload) in node_rows
        .into_iter()
        .map(|(id, payload)| ("nodes", id, payload))
        .chain(std::iter::once((
            "relations",
            relation_id,
            relation.as_slice(),
        )))
    {
        stage
            .ingest_input(InputRow {
                source_graph: "eighth",
                collection,
                id,
                payload,
            })
            .unwrap();
    }
    materialize_indexed_sources(
        &mut stage,
        &vocabulary,
        &registry,
        IndexedLimits {
            max_row_bytes: 1024 * 1024,
            max_page_rows: 1,
        },
    )
    .unwrap();
    let header = json!({
        "schema":"tos_knowledge_graph_v1",
        "source_revision":"2".repeat(64),
        "normalization_binding":{
            "schema":"tos_knowledge_graph_normalization_binding_v1",
            "processor_digest":"3".repeat(64),
            "entity_registry_digest":registry.entity_semantic_digest,
            "relation_registry_digest":registry.relation_semantic_digest,
            "configuration_digest":"4".repeat(64)
        },
        "query_properties":[],
        "counts":{"nodes":4,"relations":1,"sources":{"eighth":4},
            "display_coverage":{
                "node_titles":4,"node_summaries":4,"node_summary_states":{"source":4},
                "nodes_without_source_summary":0,
                "relation_labels":1,"relation_statements":1,"relation_explanations":1,
                "relation_explanation_states":{"source":1},
                "relations_without_source_explanation":0
            },
            "semantic_mapping":{
                "mapped_nodes":0,"unmapped_nodes":4,"mapped_relations":0,
                "unmapped_relations":1,"cross_layer_relations":0
            }
        },
        "authority_boundary":{"is_source":false,"is_canon":false,"writes_to_tree":false}
    });
    let full = compile_full_knowledge_components(
        &mut stage,
        &header,
        &registry,
        entity_bytes,
        relation_bytes,
        &[],
        &vocabulary,
        &descriptor_bytes,
        FullKnowledgeLimits {
            scope: ScopeLimits {
                max_sources: 2,
                max_rows: 5,
                max_index_work_bytes: 4096,
            },
            catalog: CatalogLimits::default(),
            catalog_index: CatalogIndexLimits::default(),
            search: SearchBuildLimits {
                max_payload_bytes: 1024 * 1024,
                max_document_chars: 1024 * 1024,
                max_document_bytes: 4 * 1024 * 1024,
                max_rank_field_bytes: 1024 * 1024,
                max_postings: 100_000,
                max_work_bytes: 100 * 1024 * 1024,
                gram_batch_rows: 64,
            },
            seal: SealLimits {
                max_header_bytes: 1024 * 1024,
            },
            max_registry_bytes: 4 * 1024 * 1024,
        },
    )
    .unwrap();
    let private_inode = fs::metadata(&path).unwrap().ino();
    let output = stage.finish().unwrap();
    assert_ne!(fs::metadata(&path).unwrap().ino(), private_inode);
    let raw_table_count: i64 = rusqlite::Connection::open(&path)
        .unwrap()
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='raw_records'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(raw_table_count, 0);
    let expectation = KnowledgeSelectedExpectation {
        model_sha256: output.sqlite_sha256.clone(),
        model_size_bytes: output.sqlite_size_bytes,
        owner_receipt_id: "fixture-owner-receipt".into(),
        model_abi: crate::KNOWLEDGE_MODEL_ABI.into(),
        descriptor_sha256: vocabulary.descriptor_sha256.clone(),
        descriptor_version: vocabulary.descriptor_version,
        semantic_primitive_profile: vocabulary.semantic_primitive_profile.clone(),
        source_cut: output.source_cut,
        through_commit_seq: 7,
        membership_root: output.membership_root,
        entity_registry_id: registry.entity_registry_id.clone(),
        entity_registry_version: registry.entity_registry_version.to_string(),
        entity_registry_sha256: registry.entity_sha256.clone(),
        relation_registry_id: registry.relation_registry_id.clone(),
        relation_registry_version: registry.relation_registry_version.to_string(),
        relation_registry_sha256: registry.relation_sha256.clone(),
        graph_root_sha256: full.seal.graph_root_sha256,
        catalog_packet_sha256: full.catalog.catalog_packet_sha256,
        catalog_index_root_sha256: full.catalog.catalog_index_root_sha256,
        source_scope_root_sha256: full.source_scope.source_scope_root_sha256,
        search_index_root_sha256: full.search.search_index_root_sha256,
        node_count: 4,
        relation_count: 1,
        index_generation: "fixture-generation".into(),
        route_map_version: "fixture-routes".into(),
        reader_abi: "fixture-reader".into(),
        authority_boundary: serde_json::to_string(&header["authority_boundary"]).unwrap(),
        source_scopes: vec![
            ExpectedSourceScope {
                source_graph: "eighth".into(),
                input_role: "source-graph".into(),
                adapter_profile: "indexed-node-edge-v1".into(),
                node_count: 4,
                relation_count: 1,
                node_root_sha256: node_root,
                relation_root_sha256: root(relation_id, &relation),
            },
            ExpectedSourceScope {
                source_graph: "zero".into(),
                input_role: "source-graph".into(),
                adapter_profile: "indexed-node-edge-v1".into(),
                node_count: 0,
                relation_count: 0,
                node_root_sha256: zero_root.clone(),
                relation_root_sha256: zero_root,
            },
        ],
        complete: true,
    };
    let mut graph = header.clone();
    graph["nodes"] = json!([
        serde_json::from_slice::<Value>(&node).unwrap(),
        serde_json::from_slice::<Value>(&alpha).unwrap(),
        serde_json::from_slice::<Value>(&visible).unwrap(),
        serde_json::from_slice::<Value>(&false_positive).unwrap()
    ]);
    graph["relations"] = json!([serde_json::from_slice::<Value>(&relation).unwrap()]);
    FullKnowledgeFixture {
        path,
        expectation,
        vocabulary,
        descriptor_bytes,
        graph_input_bytes: serde_json::to_vec(&graph).unwrap(),
        custody: FixtureCustody,
    }
}
