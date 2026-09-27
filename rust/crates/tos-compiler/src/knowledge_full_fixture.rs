//! The existing synthetic full producer fixture shared by compiler and query
//! integration tests. Enabled only for tests or the `test-fixture` feature.
//! Its owner/quota/custody stubs are not production authority.

use super::*;
use crate::knowledge_source_claims::ClaimNormalizeLimits;
use crate::{
    ColdOpenLimits, ExpectedSourceScope, ImmutableKnowledgeCustody, IndexedLimits,
    KnowledgeSelectedExpectation, Limits, SourceBinding,
    catalog::CatalogLimits,
    knowledge_stage::{
        ExactInputReceipt, InputCollectionReceipt, InputRow, KnowledgeStage, StageIsolation,
        StageLimits, StageOwner, WritePhase,
    },
    materialize_indexed_sources, open_selected_knowledge_model,
};
use serde_json::{Value, json};
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

/// Engineering caps for this finite software fixture only. Existing cold work
/// is 100MiB, model/per-file ceiling 64MiB and cache 8MiB; a finite 1GiB AS
/// envelope leaves bounded decoder/SQLite/service headroom. This is not host
/// aggregate admission or an aggregate temporary-file quota.
pub const NATIVE_SOFTWARE_FIXTURE_PROCESS_LIMITS: crate::NativeProcessLimits =
    crate::NativeProcessLimits {
        address_space_bytes: 1024 * 1024 * 1024,
        file_size_bytes: 64 * 1024 * 1024,
    };

pub struct FullKnowledgeFixture {
    pub path: PathBuf,
    pub expectation: KnowledgeSelectedExpectation,
    pub vocabulary: QueryVocabulary,
    pub descriptor_bytes: Vec<u8>,
    pub graph_input_bytes: Vec<u8>,
    pub stage_receipt: crate::knowledge_stage::StageReceipt,
    pub seal_receipt: crate::KnowledgeSealReceipt,
    pub navigation_original: Option<crate::NavigationOriginalReceipt>,
    pub philosophy_original: Option<crate::PhilosophyOriginalReceipt>,
    pub corpus_original: Option<crate::CorpusOriginalReceipt>,
    entity_registry_bytes: Vec<u8>,
    relation_registry_bytes: Vec<u8>,
    producer_limits: FullKnowledgeLimits,
    custody: FixtureCustody,
}
impl FullKnowledgeFixture {
    /// Original caller inputs already bound by compile_full_knowledge_components.
    /// The selected consumer still checks exact registry identity/version/SHA.
    pub fn entity_registry_bytes(&self) -> &[u8] {
        &self.entity_registry_bytes
    }
    pub fn relation_registry_bytes(&self) -> &[u8] {
        &self.relation_registry_bytes
    }
    pub fn registry_originals(&self) -> [&[u8]; 2] {
        [&self.entity_registry_bytes, &self.relation_registry_bytes]
    }
    /// Existing cold fixture limits, shared with its actual native companion.
    /// Per-table row guards cover the declared producer workload, never counts
    /// observed after a successful build. Physical/VM/total work caps are fixed.
    pub fn cold_limits(&self) -> ColdOpenLimits {
        let declared = self.producer_limits;
        let max_rows = declared
            .scope
            .max_rows
            .max(declared.scope.max_sources as u64)
            .max(declared.search.max_postings)
            .max(declared.catalog.max_catalog_entries)
            .max(declared.catalog_index.max_index_rows);
        ColdOpenLimits {
            max_file_bytes: 64 * 1024 * 1024,
            max_vm_steps: 100_000_000,
            sqlite_cache_kib: 8192,
            max_rows,
            max_work_bytes: 100 * 1024 * 1024,
            max_row_bytes: 1024 * 1024,
            max_metadata_bytes: 256 * 1024,
            max_sources: self.vocabulary.sources.len(),
        }
    }
    pub fn open(&self) -> Result<VerifiedKnowledgeModel<'_>> {
        open_selected_knowledge_model(
            &self.path,
            self.expectation.clone(),
            &self.custody,
            self.cold_limits(),
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
    let dir = std::env::temp_dir().join(format!("tos-full-eighth-{}-{tick}", std::process::id()));
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
    let vocabulary = QueryVocabulary::parse(&descriptor_bytes, &["indexed-node-edge-v1"]).unwrap();
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
        }))
        .unwrap()
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
    }))
    .unwrap();
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
    finish_fixture(
        stage,
        path,
        registry,
        entity_bytes,
        relation_bytes,
        vocabulary,
        descriptor_bytes,
        header,
        &[],
        None,
        None,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn finish_fixture(
    stage: KnowledgeStage<'_>,
    path: PathBuf,
    registry: KnowledgeRegistry,
    entity_bytes: &[u8],
    relation_bytes: &[u8],
    vocabulary: QueryVocabulary,
    descriptor_bytes: Vec<u8>,
    header: Value,
    saved_lenses: &[Value],
    navigation_original: Option<crate::NavigationOriginalReceipt>,
    philosophy_original: Option<crate::PhilosophyOriginalReceipt>,
    corpus_original: Option<crate::CorpusOriginalReceipt>,
) -> FullKnowledgeFixture {
    let limits = FullKnowledgeLimits {
        scope: ScopeLimits {
            max_sources: vocabulary.sources.len(),
            max_rows: 1000,
            max_index_work_bytes: 1024 * 1024,
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
    };
    finish_fixture_with_limits(
        stage,
        path,
        registry,
        entity_bytes,
        relation_bytes,
        vocabulary,
        descriptor_bytes,
        header,
        saved_lenses,
        navigation_original,
        philosophy_original,
        corpus_original,
        limits,
    )
}

/// Complete the existing finite fixture from genuine already frozen native
/// inputs. This never substitutes synthetic source rows or grants admission;
/// owner/cold custody stubs remain the explicit software-conformance profile.
#[allow(clippy::too_many_arguments)]
pub fn finish_native_source_fixture(
    mut stage: KnowledgeStage<'_>,
    path: PathBuf,
    entity_bytes: &[u8],
    relation_bytes: &[u8],
    vocabulary: QueryVocabulary,
    descriptor_bytes: Vec<u8>,
    navigation_header: &NavigationHeaderClaim,
    additional: NativeFamilyInputs<'_>,
    full_limits: FullKnowledgeLimits,
) -> FullKnowledgeFixture {
    let native_limits =
        native_fixture_limits(full_limits.scope.max_rows, full_limits.scope.max_rows);
    let registry = KnowledgeRegistry::parse(entity_bytes, relation_bytes).unwrap();
    let native = materialize_native_sources_with_inputs(
        &mut stage,
        &registry,
        entity_bytes,
        relation_bytes,
        &vocabulary,
        &descriptor_bytes,
        navigation_header,
        native_limits,
        additional,
    )
    .unwrap();
    let mut header = native_fixture_header(&mut stage, &registry, entity_bytes);
    // This software fixture derives header pins from the actual native output
    // and compiled entry source. It does not claim a source admission or the
    // digest of a complete production compiler installation.
    let proof = native
        .corpus_original
        .as_ref()
        .and_then(|receipt| receipt.origin.native_producer.as_ref())
        .expect("native source fixture requires its actual corpus producer proof");
    header["source_revision"] = json!(proof.source_revision);
    header["normalization_binding"]["processor_digest"] =
        json!(Digest256::of_bytes(include_bytes!("knowledge_native.rs")).to_hex());
    header["normalization_binding"]["configuration_digest"] =
        json!(Digest256::of_bytes(&descriptor_bytes).to_hex());
    finish_fixture_with_limits(
        stage,
        path,
        registry,
        entity_bytes,
        relation_bytes,
        vocabulary,
        descriptor_bytes,
        header,
        &[],
        native.navigation_original,
        native.philosophy_original,
        native.corpus_original,
        full_limits,
    )
}

#[allow(clippy::too_many_arguments)]
fn finish_fixture_with_limits(
    mut stage: KnowledgeStage<'_>,
    path: PathBuf,
    registry: KnowledgeRegistry,
    entity_bytes: &[u8],
    relation_bytes: &[u8],
    vocabulary: QueryVocabulary,
    descriptor_bytes: Vec<u8>,
    header: Value,
    saved_lenses: &[Value],
    navigation_original: Option<crate::NavigationOriginalReceipt>,
    philosophy_original: Option<crate::PhilosophyOriginalReceipt>,
    corpus_original: Option<crate::CorpusOriginalReceipt>,
    limits: FullKnowledgeLimits,
) -> FullKnowledgeFixture {
    let source_binding = stage.exact_receipt().binding.clone();
    let full = compile_full_knowledge_components(
        &mut stage,
        &header,
        &registry,
        entity_bytes,
        relation_bytes,
        saved_lenses,
        &vocabulary,
        &descriptor_bytes,
        limits,
    )
    .unwrap();
    let (source_scopes,graph)=stage.with_connection(WritePhase::Finalize,|db| {
        let mut statement=db.prepare("SELECT source_graph,input_role,adapter_profile,expected_node_count,expected_relation_count,lower(hex(node_root_sha256)),lower(hex(relation_root_sha256)) FROM source_scope ORDER BY source_graph")?;
        let scopes=statement.query_map([],|r|Ok(ExpectedSourceScope {source_graph:r.get(0)?,input_role:r.get(1)?,adapter_profile:r.get(2)?,
            node_count:r.get(3)?,relation_count:r.get(4)?,node_root_sha256:r.get(5)?,relation_root_sha256:r.get(6)?}))?
            .collect::<std::result::Result<Vec<_>,_>>()?;
        let mut graph=header.clone();
        for (table,key) in [("knowledge_nodes","nodes"),("knowledge_relations","relations")] {
            let mut statement=db.prepare(&format!("SELECT payload FROM {table} ORDER BY source_order"))?;
            let raw=statement.query_map([],|r|r.get::<_,Vec<u8>>(0))?.collect::<std::result::Result<Vec<_>,_>>()?;
            let values=raw.iter().map(|raw|serde_json::from_slice::<Value>(raw).unwrap()).collect::<Vec<_>>();
            graph[key]=json!(values);
        }
        Ok((scopes,graph))
    }).unwrap();
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
        model_abi: full.seal.model_abi.clone(),
        managed_source_root_sha256: full.seal.managed_source_root_sha256.clone(),
        navigation_original_root_sha256: full.seal.navigation_original_root_sha256.clone(),
        philosophy_original_root_sha256: full.seal.philosophy_original_root_sha256.clone(),
        corpus_original_root_sha256: full.seal.corpus_original_root_sha256.clone(),
        descriptor_sha256: vocabulary.descriptor_sha256.clone(),
        descriptor_version: vocabulary.descriptor_version,
        semantic_primitive_profile: vocabulary.semantic_primitive_profile.clone(),
        source_cut: output.source_cut.clone(),
        through_commit_seq: source_binding.through_commit_seq,
        membership_root: output.membership_root.clone(),
        entity_registry_id: registry.entity_registry_id.clone(),
        entity_registry_version: registry.entity_registry_version.to_string(),
        entity_registry_sha256: registry.entity_sha256.clone(),
        relation_registry_id: registry.relation_registry_id.clone(),
        relation_registry_version: registry.relation_registry_version.to_string(),
        relation_registry_sha256: registry.relation_sha256.clone(),
        graph_root_sha256: full.seal.graph_root_sha256.clone(),
        catalog_packet_sha256: full.catalog.catalog_packet_sha256,
        catalog_index_root_sha256: full.catalog.catalog_index_root_sha256,
        source_scope_root_sha256: full.source_scope.source_scope_root_sha256,
        search_index_root_sha256: full.search.search_index_root_sha256,
        node_count: output.node_rows,
        relation_count: output.relation_rows,
        index_generation: source_binding.index_generation,
        route_map_version: source_binding.route_map_version,
        reader_abi: source_binding.reader_abi,
        authority_boundary: serde_json::to_string(&header["authority_boundary"]).unwrap(),
        source_scopes,
        complete: true,
    };
    FullKnowledgeFixture {
        path,
        expectation,
        vocabulary,
        descriptor_bytes,
        graph_input_bytes: serde_json::to_vec(&graph).unwrap(),
        stage_receipt: output,
        seal_receipt: full.seal,
        navigation_original,
        philosophy_original,
        corpus_original,
        entity_registry_bytes: entity_bytes.to_vec(),
        relation_registry_bytes: relation_bytes.to_vec(),
        producer_limits: limits,
        custody: FixtureCustody,
    }
}

/// The same selected fixture route, now starting from maintained public raw
/// Claim bytes and a bounded navigation owner carrier for its exact subject.
/// No pre-normalized Claim, time envelope or final row is a test input.
pub fn build_native_fixture() -> FullKnowledgeFixture {
    build_native_fixture_inner(false, None, None, None)
}
/// Existing native raw fixture with one explicitly synthetic rights declaration
/// retained through normal assembler/seal/cold-open. This grants no authority.
pub fn build_native_fixture_with_navigation_original() -> FullKnowledgeFixture {
    build_native_fixture_inner(true, None, None, None)
}
/// Caller supplies complete original owner fixture packets. They traverse the
/// same raw ingestion, native normalization, catalog, seal and cold-open path.
/// Header counts and exact packet identities are verified by the normal producer.
pub fn build_native_fixture_with_navigation_inputs(
    header: &[u8],
    nodes: &[&[u8]],
    edges: &[&[u8]],
    rights: &[&[u8]],
) -> FullKnowledgeFixture {
    build_native_fixture_inner(true, Some((header, nodes, edges, rights)), None, None)
}
use crate::knowledge_philosophy_prepare::{
    PHILOSOPHY_FIXTURE_0, PHILOSOPHY_FIXTURE_1, PHILOSOPHY_FIXTURE_2,
};
/// One finite synthetic software fixture, using the unchanged existing native
/// preparation inputs; no authored-growth, rights or review admission is claimed.
pub fn build_native_fixture_with_philosophy_original() -> FullKnowledgeFixture {
    let nodes = [
        PHILOSOPHY_FIXTURE_0.as_bytes(),
        PHILOSOPHY_FIXTURE_1.as_bytes(),
    ];
    let edges = [PHILOSOPHY_FIXTURE_2.as_bytes()];
    let nv = nodes
        .iter()
        .map(|r| serde_json::from_slice::<Value>(r).unwrap())
        .collect::<Vec<_>>();
    let ev = edges
        .iter()
        .map(|r| serde_json::from_slice::<Value>(r).unwrap())
        .collect::<Vec<_>>();
    let node_ids = nv.iter().map(|v| v["node_id"].clone()).collect::<Vec<_>>();
    let edge_ids = ev.iter().map(|v| v["edge_id"].clone()).collect::<Vec<_>>();
    let mut views = Vec::new();
    for v in ev.iter().chain(&nv) {
        for id in v["view_ids"].as_array().unwrap() {
            if !views.contains(id) {
                views.push(id.clone());
            }
        }
    }
    let refs = nv
        .iter()
        .chain(&ev)
        .map(|v| v["source_ref"].clone())
        .collect::<Vec<_>>();
    let source = ev[0]["source_ref"].clone();
    let view_records=views.iter().map(|id|json!({"view_id":id,"title":id,"graph_layers":["philosophy"],"node_ids":node_ids,"edge_ids":edge_ids,"source_refs":refs,"source_ref":source})).collect::<Vec<_>>();
    let review = views
        .iter()
        .map(|id| json!({"view_id":id,"unresolved_diagnostics":[]}))
        .collect::<Vec<_>>();
    let header = json!({"schema_version":"tos_philosophy_graph_projection_v2","counts":{"nodes":nodes.len(),"edges":edges.len(),"views":views.len(),"clusters":1},
      "graph_layers":[{"layer_id":"philosophy","label":"Philosophy"}],"layer_counts":[{"layer_id":"philosophy","nodes":nodes.len(),"edges":edges.len()}],
      "visibility_model":{},"runtime_projection_boundary":{"is_source_authority":false,"writes_to_tree":false,"scope":"synthetic-native-philosophy-fixture"},
      "views":view_records,"clusters":[{"cluster_id":"fixture-phi-pair","cluster_kind":"pair","label":"Pair","view_ids":views,"member_node_ids":node_ids,"member_edge_ids":edge_ids,"source_ref":source,"properties":{"member_count":nodes.len(),"edge_count":edges.len()}}],
      "review_packets":review,"snapshot_review":{"snapshot_schema_version":"tos_philosophy_graph_projection_snapshot_v1"},"unresolved_review_surfaces":[],"source_refs":{}});
    let raw = serde_json::to_vec(&header).unwrap();
    build_native_fixture_with_philosophy_inputs(&raw, &nodes, &edges)
}
/// Exact ordered phi owner input through the existing native assembler and cold opener.
pub fn build_native_fixture_with_philosophy_inputs(
    header: &[u8],
    nodes: &[&[u8]],
    edges: &[&[u8]],
) -> FullKnowledgeFixture {
    build_native_fixture_inner(false, None, Some((header, nodes, edges)), None)
}
/// Reuse a real existing software capture of unchanged public corpus inputs.
/// Abbreviated compatibility rows are never passed off as native canon inputs.
pub fn build_native_fixture_with_captured_corpus(
    capture_root: &std::path::Path,
    restored_root: &std::path::Path,
    source_git_commit: &str,
    source_git_tree: &str,
    capture_manifest_sha256: &str,
    source_path: &str,
) -> FullKnowledgeFixture {
    let cancelled = std::sync::atomic::AtomicBool::new(false);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let capture = tos_source_store::SoftwareCaptureReader::open(
        capture_root,
        restored_root,
        tos_source_store::SoftwareCaptureSelectionV1 {
            source_git_commit: source_git_commit.into(),
            source_git_tree: source_git_tree.into(),
            capture_manifest_sha256: Digest256::from_hex(capture_manifest_sha256).unwrap(),
        },
        tos_source_store::ReadLimits {
            max_manifest_bytes: 2 * 1024 * 1024,
            max_manifest_entries: 512,
            max_selected_object_bytes: 16 * 1024 * 1024,
            json: tos_foundation::JsonLimits::default(),
        },
        deadline,
        &cancelled,
    )
    .unwrap();
    let path = tos_foundation::RelativePath::parse(source_path).unwrap();
    build_native_fixture_inner(
        false,
        None,
        None,
        Some((&capture, &path, deadline, &cancelled)),
    )
}
type PhilosophyFixtureInputs<'a> = (&'a [u8], &'a [&'a [u8]], &'a [&'a [u8]]);
type NavigationFixtureInputs<'a> = (&'a [u8], &'a [&'a [u8]], &'a [&'a [u8]], &'a [&'a [u8]]);
fn build_native_fixture_inner(
    retain_original: bool,
    originals: Option<NavigationFixtureInputs<'_>>,
    philosophy_originals: Option<PhilosophyFixtureInputs<'_>>,
    captured_corpus: Option<(
        &tos_source_store::SoftwareCaptureReader,
        &tos_foundation::RelativePath,
        std::time::Instant,
        &std::sync::atomic::AtomicBool,
    )>,
) -> FullKnowledgeFixture {
    let entity_bytes =
        include_bytes!("../../../../ToS/doctrine/semantic-interchange/entity-types.v1.json");
    let relation_bytes =
        include_bytes!("../../../../ToS/doctrine/semantic-interchange/relation-types.v1.json");
    let registry = KnowledgeRegistry::parse(entity_bytes, relation_bytes).unwrap();
    let mut descriptor: Value =
        serde_json::from_slice(include_bytes!("../tests/fixtures/query-vocabulary.v1.json"))
            .unwrap();
    descriptor["sources"]
        .as_array_mut()
        .unwrap()
        .retain(|source| {
            matches!(
                source["adapter_profile"].as_str(),
                Some("source-navigation-node-edge-v1" | "reified-bibliographic-claims-v1")
            ) || philosophy_originals.is_some()
                && source["adapter_profile"] == "philosophy-node-edge-v1"
        });
    let descriptor_bytes = serde_json::to_vec(&descriptor).unwrap();
    let vocabulary = QueryVocabulary::parse(
        &descriptor_bytes,
        &[
            "source-navigation-node-edge-v1",
            "reified-bibliographic-claims-v1",
            "indexed-node-edge-v1",
            "philosophy-node-edge-v1",
        ],
    )
    .unwrap();
    let fixture: Value = serde_json::from_slice(include_bytes!(
        "../../../../access/tests/fixtures/knowledge-contract/temporal-jenseits-date.json"
    ))
    .unwrap();
    let subject = fixture["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["node_kind"] == "identity")
        .unwrap();
    let claim = fixture["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["node_kind"] == "claim")
        .unwrap();
    let subject_id = subject["properties"]["record_id"].as_str().unwrap();
    let nav = json!({"node_id":subject_id,"node_kind":"identity","label":subject["properties"]["preferred_label"],
        "source_ref":subject["source_ref"],"identity_status":subject["properties"]["identity_status"],"properties":subject["properties"]});
    let nav_edge = json!({"edge_id":"native-claim-subject","from_id":subject_id,"to_id":claim["node_id"],
        "to_source_graph":"source-claims","predicate_id":"has_claim","edge_kind":"declared-claim-navigation",
        "review_status":"source-recorded","source_refs":[subject["source_ref"]],"properties":{},"view_ids":["native-fixture"]});
    let mut rows = Vec::<(String, String, String, Vec<u8>)>::new();
    for (collection, field) in [
        ("nodes", "node_id"),
        ("edges", "edge_id"),
        ("claim_traces", "claim_ref"),
    ] {
        for row in fixture[collection].as_array().unwrap() {
            rows.push((
                "source-claims".into(),
                collection.into(),
                row[field].as_str().unwrap().into(),
                serde_json::to_vec(row).unwrap(),
            ));
        }
    }
    rows.push((
        "source-navigation".into(),
        "nodes".into(),
        subject_id.into(),
        serde_json::to_vec(&nav).unwrap(),
    ));
    rows.push((
        "source-navigation".into(),
        "edges".into(),
        "native-claim-subject".into(),
        serde_json::to_vec(&nav_edge).unwrap(),
    ));
    if let Some((_, nodes, edges, _)) = originals {
        let source = vocabulary
            .sources
            .iter()
            .find(|s| s.adapter_profile == "source-navigation-node-edge-v1")
            .unwrap();
        rows.retain(|row| row.0 != source.source_graph_id);
        for (collection, id_field, packets) in
            [("nodes", "node_id", nodes), ("edges", "edge_id", edges)]
        {
            for raw in packets {
                let value: Value = serde_json::from_slice(raw).unwrap();
                rows.push((
                    source.source_graph_id.clone(),
                    collection.into(),
                    value[id_field].as_str().unwrap().into(),
                    raw.to_vec(),
                ));
            }
        }
    }
    if let Some((_, nodes, edges)) = philosophy_originals {
        let source = vocabulary
            .sources
            .iter()
            .find(|s| s.adapter_profile == "philosophy-node-edge-v1")
            .unwrap();
        for (collection, key, packets) in [("nodes", "node_id", nodes), ("edges", "edge_id", edges)]
        {
            for raw in packets {
                let v: Value = serde_json::from_slice(raw).unwrap();
                rows.push((
                    source.source_graph_id.clone(),
                    collection.into(),
                    v[key].as_str().unwrap().into(),
                    raw.to_vec(),
                ));
            }
        }
    }
    let mut collections = Vec::new();
    for source in &vocabulary.sources {
        let names: &[&str] = if source.adapter_profile == "reified-bibliographic-claims-v1" {
            &["nodes", "edges", "claim_traces"]
        } else {
            &["nodes", "edges"]
        };
        for collection in names {
            let subset = rows
                .iter()
                .filter(|row| row.0 == source.source_graph_id && row.1 == *collection)
                .map(|row| (row.2.as_str(), row.3.as_slice()))
                .collect::<Vec<_>>();
            collections.push(InputCollectionReceipt {
                source_graph: source.source_graph_id.clone(),
                collection: (*collection).into(),
                input_role: source.input_role.clone(),
                adapter_profile: source.adapter_profile.clone(),
                expected_count: subset.len() as u64,
                expected_root_sha256: roots(&subset),
            });
        }
    }
    let exact = ExactInputReceipt {
        binding: SourceBinding {
            owner_profile: "fixture-owner".into(),
            source_cut: "native-fixture-cut".into(),
            through_commit_seq: 7,
            membership_root: "0".repeat(64),
            index_generation: "fixture-generation".into(),
            route_map_version: "fixture-routes".into(),
            reader_abi: "fixture-reader".into(),
            projection_root_sha256: "1".repeat(64),
            complete: true,
        },
        collections,
    };
    let path = candidate();
    let owner = FixtureOwner;
    let quota = FixtureQuota;
    let mut stage = KnowledgeStage::create(
        &path,
        StageLimits {
            sqlite: Limits::default(),
            max_temp_bytes: 64 * 1024 * 1024,
            max_seek_rows: 2,
            max_seek_bytes: 1024 * 1024,
        },
        exact,
        &owner,
        &quota,
    )
    .unwrap();
    for (source, collection, id, payload) in &rows {
        stage
            .ingest_input(InputRow {
                source_graph: source,
                collection,
                id,
                payload,
            })
            .unwrap();
    }
    let header = json!({"schema_version":"tos_source_navigation_v1","authority_boundary":"derived fixture, source semantics and admission retained upstream",
        "counts":{"nodes":1,"edges":1,"rights":u64::from(retain_original)}});
    let raw_json = originals.map_or_else(
        || serde_json::to_vec(&header).unwrap(),
        |(header, _, _, _)| header.to_vec(),
    );
    let navigation_header = NavigationHeaderClaim {
        expected_sha256: Digest256::of_bytes(&raw_json).to_hex(),
        raw_json,
    };
    let native_limits = native_fixture_limits(100, 200);
    let mut additional = NativeFamilyInputs::bounded_from(native_limits);
    let original_right = br#"{ "rights_id":"fixture-original-declaration", "visibility":"public_metadata_only", "assessment_status":"unknown", "restrictions":[] }"#;
    let default_rights: [&[u8]; 1] = [original_right];
    let original_rights = originals.map_or(default_rights.as_slice(), |(_, _, _, rights)| rights);
    let original_root = navigation_original_rights_root(original_rights);
    if retain_original {
        additional.navigation_original = Some(NavigationOriginalInput {
            rights: original_rights,
            expected_rights_root_sha256: &original_root,
            limits: NavigationOriginalLimits {
                max_rows: 200,
                max_row_bytes: 65536,
                max_total_bytes: 262144,
            },
        });
    }
    let phi_header_sha =
        philosophy_originals.map(|(header, _, _)| Digest256::of_bytes(header).to_hex());
    let phi_nodes_root = philosophy_originals.map(|(_, nodes, _)| {
        crate::philosophy_original_rows_root(crate::PhilosophyOriginalCollection::Nodes, nodes)
    });
    let phi_edges_root = philosophy_originals.map(|(_, _, edges)| {
        crate::philosophy_original_rows_root(crate::PhilosophyOriginalCollection::Edges, edges)
    });
    if let Some((header, nodes, edges)) = philosophy_originals {
        additional.philosophy_original = Some(crate::PhilosophyOriginalInput {
            header,
            nodes,
            edges,
            expected_header_sha256: phi_header_sha.as_deref().unwrap(),
            expected_nodes_root_sha256: phi_nodes_root.as_deref().unwrap(),
            expected_edges_root_sha256: phi_edges_root.as_deref().unwrap(),
            limits: NavigationOriginalLimits {
                max_rows: 200,
                max_row_bytes: 65536,
                max_total_bytes: 262144,
            },
        });
    }
    let mut native = materialize_native_sources_with_inputs(
        &mut stage,
        &registry,
        entity_bytes,
        relation_bytes,
        &vocabulary,
        &descriptor_bytes,
        &navigation_header,
        native_limits,
        additional,
    )
    .unwrap();
    if let Some((capture, path, deadline, cancelled)) = captured_corpus {
        native.corpus_original = Some(
            crate::retain_captured_corpus_original_from_capture(
                &mut stage,
                capture,
                path,
                &vocabulary,
                crate::CorpusOriginalSourceLimits {
                    originals: NavigationOriginalLimits {
                        max_rows: 200,
                        max_row_bytes: 65536,
                        max_total_bytes: 262144,
                    },
                    max_members: 128,
                    max_work_bytes: 16 * 1024 * 1024,
                },
                deadline,
                cancelled,
            )
            .unwrap(),
        );
    }
    let graph_header = native_fixture_header(&mut stage, &registry, entity_bytes);
    // One versioned LensSpec travels through the real catalog producer. Its
    // source scope comes from this fixture's selected owner descriptor.
    // Query tests retrieve and execute the stored packet, rather than a
    // consumer-side substitute with a hard-coded maintained source list.
    let saved_lenses = [json!({
        "schema_version":"tos_lens_spec_v1",
        "lens_id":"synthetic-indexed",
        "sources":vocabulary.registered_source_ids,
        "explain":true,
        "composition":{"endpoint_policy":"independent","group_by":["source_graph"]},
        "limits":{"nodes":200,"relations":400,"groups":100}
    })];
    finish_fixture(
        stage,
        path,
        registry,
        entity_bytes,
        relation_bytes,
        vocabulary,
        descriptor_bytes,
        graph_header,
        &saved_lenses,
        native.navigation_original,
        native.philosophy_original,
        native.corpus_original,
    )
}

// Existing software fixture engineering profile. Source composition uses its
// independently declared row envelope; per-row/cache/work limits stay shared.
fn native_fixture_limits(row_limit: u64, final_limit: u64) -> NativeProducerLimits {
    NativeProducerLimits {
        navigation_prepare: NavigationPrepareLimits {
            max_nodes: row_limit,
            max_edges: row_limit,
            max_endpoint_refs: row_limit.saturating_mul(2),
            max_page_rows: 2,
            max_row_bytes: 262144,
            max_header_bytes: 65536,
            max_work_bytes: 32 * 1024 * 1024,
        },
        navigation_nodes: NavigationNodeLimits {
            max_raw_bytes: 262144,
            max_output_bytes: 1048576,
            max_ancestor_cache_bytes: 1048576,
        },
        navigation_materialize: NavigationMaterializeLimits {
            max_nodes: row_limit,
            max_edges: row_limit,
            max_placeholders: row_limit,
            max_page_rows: 2,
            max_raw_bytes: 262144,
            max_output_bytes: 1048576,
            max_page_bytes: 524288,
            max_work_bytes: 32 * 1024 * 1024,
        },
        navigation_dependencies: NavigationRelationLimits {
            max_edges: row_limit,
            max_page_rows: 2,
            max_raw_bytes: 262144,
            max_context_bytes: 1048576,
            max_page_bytes: 524288,
            max_work_bytes: 32 * 1024 * 1024,
        },
        navigation_relations: NavigationRelationNormalizeLimits {
            max_raw_bytes: 262144,
            max_output_bytes: 1048576,
            max_registry_bytes: 4 * 1024 * 1024,
            max_claim_contexts: 64,
            max_global_input_bytes: 1048576,
        },
        claims_prepare: ClaimPrepareLimits {
            max_nodes: row_limit,
            max_edges: row_limit,
            max_claims: row_limit,
            max_page_rows: 2,
            max_row_bytes: 262144,
            max_work_bytes: 32 * 1024 * 1024,
        },
        claims: ClaimNormalizeLimits {
            max_raw_bytes: 262144,
            max_output_bytes: 1048576,
            max_page_rows: 2,
            max_contexts: 64,
            max_work_bytes: 64 * 1024 * 1024,
        },
        philosophy_prepare: PhilosophyPrepareLimits {
            max_nodes: row_limit,
            max_edges: row_limit,
            max_edge_view_bindings: row_limit,
            max_page_rows: 2,
            max_row_bytes: 262144,
            max_work_bytes: 32 * 1024 * 1024,
        },
        philosophy: PhilosophyMaterializeLimits {
            max_raw_bytes: 262144,
            max_output_bytes: 1048576,
            max_registry_bytes: 4 * 1024 * 1024,
            max_page_rows: 2,
            max_rows: row_limit,
            max_work_bytes: 32 * 1024 * 1024,
        },
        titles: GlobalTitleLimits {
            max_nodes: row_limit,
            max_page_rows: 2,
            max_page_bytes: 2 * 1048576,
            max_node_bytes: 1048576,
            max_title_bytes: 65536,
            max_work_bytes: 32 * 1024 * 1024,
        },
        inherited: InheritedViewLimits {
            max_relations: row_limit,
            max_endpoint_evidence_rows: row_limit.saturating_mul(2),
            max_view_tokens: row_limit,
            max_page_rows: 2,
            max_page_bytes: 2 * 1048576,
            max_row_bytes: 1048576,
            max_work_bytes: 32 * 1024 * 1024,
        },
        finalize: NativeFinalizeLimits {
            max_rows: final_limit,
            max_page_rows: 2,
            max_page_bytes: 2 * 1048576,
            max_row_bytes: 1048576,
            max_view_ids_per_node: 64,
            max_context_sources: 64,
            max_work_bytes: 64 * 1024 * 1024,
        },
    }
}

fn native_fixture_header(
    stage: &mut KnowledgeStage<'_>,
    registry: &KnowledgeRegistry,
    entity_bytes: &[u8],
) -> Value {
    let roots = stage.core_roots().unwrap();
    let (
        sources,
        node_states,
        relation_states,
        mapped_nodes,
        mapped_relations,
        missing_node_summary,
        missing_relation_explanation,
    ) = stage
        .with_connection(WritePhase::Finalize, |db| {
            let mut sources = std::collections::BTreeMap::<String, u64>::new();
            let mut states = [
                std::collections::BTreeMap::<String, u64>::new(),
                std::collections::BTreeMap::<String, u64>::new(),
            ];
            let mut mapped = [0u64; 2];
            let mut missing = [0u64; 2];
            for (i, table) in ["knowledge_nodes", "knowledge_relations"]
                .iter()
                .enumerate()
            {
                let mut statement = db.prepare(&format!(
                    "SELECT source_graph,payload FROM {table} ORDER BY source_order"
                ))?;
                let mut rows = statement.query([])?;
                while let Some(row) = rows.next()? {
                    let source: String = row.get(0)?;
                    let raw: Vec<u8> = row.get(1)?;
                    let value: Value = serde_json::from_slice(&raw).unwrap();
                    if i == 0 {
                        *sources.entry(source).or_default() += 1;
                    }
                    let state = if i == 0 {
                        "summary_state"
                    } else {
                        "explanation_state"
                    };
                    *states[i]
                        .entry(value["display"][state].as_str().unwrap().into())
                        .or_default() += 1;
                    let mapping = if i == 0 {
                        "type_mapping"
                    } else {
                        "predicate_mapping"
                    };
                    if value[mapping]["status"] == "mapped" {
                        mapped[i] += 1;
                    }
                    let source = if i == 0 {
                        "source_summary_available"
                    } else {
                        "source_explanation_available"
                    };
                    if value["display"]["provenance"][source] == false {
                        missing[i] += 1;
                    }
                }
            }
            let [nodes, relations] = states;
            Ok((
                sources, nodes, relations, mapped[0], mapped[1], missing[0], missing[1],
            ))
        })
        .unwrap();
    let entity: Value = serde_json::from_slice(entity_bytes).unwrap();
    let query_properties = entity["property_definitions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|definition| {
            let mut packet = serde_json::Map::new();
            for field in [
                "property_id",
                "field",
                "value_type",
                "applies_to",
                "inherited",
                "operators",
            ] {
                if let Some(value) = definition.get(field) {
                    packet.insert(field.into(), value.clone());
                }
            }
            Value::Object(packet)
        })
        .collect::<Vec<_>>();
    json!({"schema":"tos_knowledge_graph_v1","source_revision":"2".repeat(64),
        "normalization_binding":{"schema":"tos_knowledge_graph_normalization_binding_v1","processor_digest":"3".repeat(64),
            "entity_registry_digest":registry.entity_semantic_digest,"relation_registry_digest":registry.relation_semantic_digest,"configuration_digest":"4".repeat(64)},
        "query_properties":query_properties,"counts":{"nodes":roots.nodes,"relations":roots.relations,"sources":sources,
            "display_coverage":{"node_titles":roots.nodes,"node_summaries":roots.nodes,"node_summary_states":node_states,"nodes_without_source_summary":missing_node_summary,
                "relation_labels":roots.relations,"relation_statements":roots.relations,"relation_explanations":roots.relations,"relation_explanation_states":relation_states,
                "relations_without_source_explanation":missing_relation_explanation},
            "semantic_mapping":{"mapped_nodes":mapped_nodes,"unmapped_nodes":roots.nodes-mapped_nodes,"mapped_relations":mapped_relations,
                "unmapped_relations":roots.relations-mapped_relations,"cross_layer_relations":0}},
        "authority_boundary":{"is_source":false,"is_canon":false,"writes_to_tree":false}})
}
