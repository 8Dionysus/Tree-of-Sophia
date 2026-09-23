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
}
