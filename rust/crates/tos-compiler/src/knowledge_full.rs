//! Ordered full-model component build over an already normalized private
//! stage. This is a FullOnly job; adapters and owner semantic admission are
//! prerequisites, not hidden fallback behavior.

use crate::{
    Error, KnowledgeRegistry, QueryVocabulary, Result,
    catalog::{CatalogLimits, CatalogReceipt, compile_catalog},
    knowledge_catalog_index::{CatalogIndexLimits, CatalogIndexReceipt, materialize_catalog},
    knowledge_scope::{ScopeLimits, ScopeReceipt, write_source_scope},
    knowledge_seal::{KnowledgeSealReceipt, SealLimits, seal_knowledge_model},
    knowledge_search::{SearchBuildLimits, SearchIndexReceipt, build_search_index},
    knowledge_stage::{KnowledgeStage, WritePhase},
};
use serde_json::Value;
use tos_foundation::{Digest256, JsonLimits, JsonMode, parse_json};

#[derive(Clone, Copy, Debug)]
pub struct FullKnowledgeLimits {
    pub scope: ScopeLimits,
    pub catalog: CatalogLimits,
    pub catalog_index: CatalogIndexLimits,
    pub search: SearchBuildLimits,
    pub seal: SealLimits,
    pub max_registry_bytes: usize,
}

#[derive(Clone, Debug)]
pub struct FullKnowledgeReceipt {
    pub source_scope: ScopeReceipt,
    pub catalog: CatalogIndexReceipt,
    pub search: SearchIndexReceipt,
    pub seal: KnowledgeSealReceipt,
}

fn registry_value(raw: &[u8], expected_sha256: &str, cap: usize) -> Result<Value> {
    if cap == 0 || cap > 4 * 1024 * 1024 || raw.is_empty() || raw.len() > cap {
        return Err(Error::Budget("full knowledge registry bytes"));
    }
    if Digest256::of_bytes(raw).to_hex() != expected_sha256 {
        return Err(Error::Invalid("full knowledge registry byte root"));
    }
    let limits = JsonLimits::new(cap, 96, 1_000_000, 4096)
        .map_err(|_| Error::Budget("full knowledge registry JSON limits"))?;
    parse_json(raw, JsonMode::PublishedStrict, limits).map_err(|e| Error::Source(e.to_string()))?;
    serde_json::from_slice(raw).map_err(|_| Error::Invalid("full knowledge registry JSON"))
}

/// Materialize all local read components in dependency order, then seal their
/// roots. The caller must provide complete final normalized rows and owner
/// semantic validation before this call; it must call `stage.finish` and use
/// a separate selected-publication authority afterward. Any failure poisons
/// the stage and prevents `finish` from yielding a candidate receipt.
pub fn compile_full_knowledge_components(
    stage: &mut KnowledgeStage<'_>,
    graph_header: &Value,
    registry: &KnowledgeRegistry,
    entity_registry_bytes: &[u8],
    relation_registry_bytes: &[u8],
    saved_lenses: &[Value],
    vocabulary: &QueryVocabulary,
    descriptor_bytes: &[u8],
    limits: FullKnowledgeLimits,
) -> Result<FullKnowledgeReceipt> {
    let result = compile_inner(
        stage,
        graph_header,
        registry,
        entity_registry_bytes,
        relation_registry_bytes,
        saved_lenses,
        vocabulary,
        descriptor_bytes,
        limits,
    );
    if result.is_err() {
        stage.poison();
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn compile_inner(
    stage: &mut KnowledgeStage<'_>,
    graph_header: &Value,
    registry: &KnowledgeRegistry,
    entity_registry_bytes: &[u8],
    relation_registry_bytes: &[u8],
    saved_lenses: &[Value],
    vocabulary: &QueryVocabulary,
    descriptor_bytes: &[u8],
    limits: FullKnowledgeLimits,
) -> Result<FullKnowledgeReceipt> {
    vocabulary.verify_authored_bytes(descriptor_bytes)?;
    let entity = registry_value(
        entity_registry_bytes,
        &registry.entity_sha256,
        limits.max_registry_bytes,
    )?;
    let relation = registry_value(
        relation_registry_bytes,
        &registry.relation_sha256,
        limits.max_registry_bytes,
    )?;
    let source_scope = write_source_scope(stage, vocabulary, limits.scope)?;
    let packet: CatalogReceipt = stage.with_connection(WritePhase::Catalog, |db| {
        compile_catalog(
            db,
            graph_header,
            &entity,
            &relation,
            saved_lenses,
            vocabulary,
            descriptor_bytes,
            limits.catalog,
        )
    })?;
    let catalog = materialize_catalog(stage, &packet, vocabulary, limits.catalog_index)?;
    let search = build_search_index(stage, limits.search)?;
    let seal = seal_knowledge_model(
        stage,
        graph_header,
        vocabulary,
        registry,
        &source_scope,
        &catalog,
        &search,
        limits.seal,
    )?;
    Ok(FullKnowledgeReceipt {
        source_scope,
        catalog,
        search,
        seal,
    })
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use crate::knowledge_full_fixture::build_fixture;

    #[test]
    fn eighth_source_private_producer_opens_one_complete_selected_model() {
        let fixture = build_fixture();
        let mut selected = fixture.open().unwrap();
        assert_eq!(selected.selection().node_count, 4);
        assert_eq!(selected.source_revision(), "2".repeat(64));
        assert!(selected.open_vm_steps() > 0);
        let fork = selected.fork_reader_with_vm_budget(100_000_000).unwrap();
        assert_eq!(fork.source_revision(), selected.source_revision());
        assert_eq!(
            selected
                .connection()
                .query_row("SELECT count(*) FROM search_documents", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            5
        );
        assert_eq!(
            selected
                .connection()
                .query_row(
                    "SELECT postings FROM search_gram_stats WHERE kind='nodes' AND n=3 AND gram=?1",
                    [b"alp".as_slice()],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            3
        );
        assert_eq!(
            selected
                .connection()
                .query_row(
                    "SELECT expected_node_count FROM source_scope WHERE source_graph='zero'",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
        selected.check_pin().unwrap();
    }

    #[test]
    fn navigation_original_selected_custody_preserves_bytes_and_refuses_tamper() {
        use crate::knowledge_full_fixture::{
            build_fixture, build_native_fixture_with_navigation_original,
        };
        use tos_foundation::Digest256;
        let mut fixture = build_native_fixture_with_navigation_original();
        {
            let mut model = fixture.open().unwrap();
            let receipt = model.navigation_original_receipt().unwrap().clone();
            assert_eq!(
                model.selection().model_abi,
                crate::KNOWLEDGE_NAVIGATION_MODEL_ABI
            );
            assert_eq!(
                receipt.descriptor_sha256,
                fixture.vocabulary.descriptor_sha256
            );
            assert_eq!(receipt.source_cut, model.selection().source_cut);
            assert_eq!(receipt.membership_root, model.selection().membership_root);
            assert_eq!(receipt.rights, 1);
            let first = model
                .navigation_original_page(None, 100_000, 1, 65536, 65536)
                .unwrap();
            assert_eq!(first.rows.len(), 1);
            assert_eq!(first.rows[0].0, -1);
            assert_eq!(
                Digest256::of_bytes(&first.rows[0].1).to_hex(),
                receipt.header_sha256
            );
            let second = model
                .navigation_original_page(first.next_ordinal, 100_000, 1, 65536, 65536)
                .unwrap();
            assert_eq!(second.rows[0].0, 0);
            assert_eq!(second.rows[0].1.as_slice(),br#"{ "rights_id":"fixture-original-declaration", "visibility":"public_metadata_only", "assessment_status":"unknown", "restrictions":[] }"#);
            let tail = model
                .navigation_original_page(second.next_ordinal, 100_000, 1, 65536, 65536)
                .unwrap();
            assert!(tail.rows.is_empty());
            assert!(tail.next_ordinal.is_none());
            assert!(first.vm_steps > 0 && second.vm_steps > 0);
            assert_eq!(
                first.decoded_bytes + second.decoded_bytes,
                receipt.total_bytes
            );
            assert_eq!(
                crate::navigation_original_rights_root(&[&second.rows[0].1]),
                receipt.rights_root_sha256
            );
            let mut fork = model.fork_reader_with_vm_budget(100_000).unwrap();
            assert_eq!(
                fork.navigation_original_receipt()
                    .unwrap()
                    .component_root_sha256,
                receipt.component_root_sha256
            );
            assert_eq!(
                fork.navigation_original_page(Some(-1), 100_000, 1, 65536, 65536)
                    .unwrap()
                    .rows[0]
                    .1,
                second.rows[0].1
            );
            assert!(
                model
                    .navigation_original_page(None, 1, 1, 65536, 65536)
                    .is_err()
            );
            assert!(
                model
                    .navigation_original_page(None, 100_000, 2, 65536, 65536)
                    .is_err()
            );
        }
        // Deliberately rehash the synthetic expected carrier after tampering:
        // refusal must come from the inner component, not only whole-file SHA.
        let changed = br#"{"rights_id":"fixture-original-declaration","visibility":"private"}"#;
        let db = rusqlite::Connection::open(&fixture.path).unwrap();
        db.execute("UPDATE navigation_original_rows SET packet_len=?1,packet_sha256=?2,packet=?3 WHERE ordinal=0",rusqlite::params![changed.len() as i64,Digest256::of_bytes(changed).as_bytes().as_slice(),changed.as_slice()]).unwrap();
        drop(db);
        let raw = std::fs::read(&fixture.path).unwrap();
        fixture.expectation.model_sha256 = Digest256::of_bytes(&raw).to_hex();
        fixture.expectation.model_size_bytes = raw.len() as u64;
        assert!(fixture.open().is_err());
        let legacy = build_fixture();
        let mut old = legacy.open().unwrap();
        assert_eq!(old.selection().model_abi, crate::KNOWLEDGE_MODEL_ABI);
        assert!(old.navigation_original_receipt().is_err());
        assert!(
            old.navigation_original_page(None, 100_000, 1, 65536, 65536)
                .is_err()
        );
    }

    #[test]
    fn native_raw_claim_navigation_produces_complete_selected_graph() {
        let fixture = crate::knowledge_full_fixture::build_native_fixture();
        let graph: serde_json::Value = serde_json::from_slice(&fixture.graph_input_bytes).unwrap();
        assert_eq!(graph["counts"]["nodes"], 7);
        assert_eq!(graph["counts"]["relations"], 6);
        let source: serde_json::Value = serde_json::from_slice(include_bytes!(
            "../../../../access/tests/fixtures/knowledge-contract/temporal-jenseits-date.json"
        ))
        .unwrap();
        let trace = &source["claim_traces"][0];
        let source_claim = source["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["node_id"] == trace["claim_node_id"])
            .unwrap();
        let source_graph = &fixture
            .vocabulary
            .sources
            .iter()
            .find(|source| source.adapter_profile == "reified-bibliographic-claims-v1")
            .unwrap()
            .source_graph_id;
        let claim = graph["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| {
                node["source_graph"] == source_graph.as_str()
                    && node["native_id"] == trace["claim_node_id"]
            })
            .unwrap();
        assert_eq!(
            claim.pointer("/attributes/source_claim"),
            source_claim.pointer("/properties/source_claim")
        );
        assert_eq!(claim.pointer("/attributes/claim_trace"), Some(trace));
        // Frozen maintained Python _normalize_node +
        // _claim_finalization_value oracle over this historical transport.
        // Its registry reader is historical-temporal-v1; the two source
        // fields below belong only to document-catalogue-temporal-v1.
        let contract = claim.pointer("/semantics/claim").unwrap();
        assert_eq!(
            crate::knowledge_normalization::stable_digest(contract).unwrap(),
            "e1482ee857ecf29770bd002ab31a699ad1ac711549730e8a42eea486eff2f4b9"
        );
        assert!(contract.get("source_claim_profile").is_none());
        assert!(contract.get("source_canonical_json").is_none());
        assert_eq!(claim["view_ids"], serde_json::json!(["native-fixture"]));
        assert!(claim.get("readable_context").is_some());
        let mut selected = fixture.open().unwrap();
        assert_eq!(selected.selection().node_count, 7);
        assert_eq!(selected.selection().relation_count, 6);
        assert_eq!(selected.selection().model_abi, crate::KNOWLEDGE_MODEL_ABI);
        selected.check_pin().unwrap();
        if let Some(path) = std::env::var_os("TOS_CMP_NATIVE_GRAPH_EXPORT") {
            std::fs::write(path, &fixture.graph_input_bytes).unwrap();
        }
    }
}
