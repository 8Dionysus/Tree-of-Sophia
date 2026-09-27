//! Acyclic private full-knowledge model seal. Source admission and selected
//! publication are separate owner actions; this seal binds derived components.

use crate::{
    Error, KnowledgeRegistry, QueryVocabulary, Result,
    knowledge_catalog_index::CatalogIndexReceipt,
    knowledge_scope::ScopeReceipt,
    knowledge_search::SearchIndexReceipt,
    knowledge_stage::{KnowledgeStage, WritePhase},
};
use rusqlite::params;
use serde_json::Value;
use std::io::{self, Write};
use tos_foundation::{
    CanonicalProfile, Digest256, Digest256Hasher, JsonLimits, JsonMode, canonical_bytes_v1,
    parse_json,
};

pub const KNOWLEDGE_MODEL_ABI: &str = "tos_knowledge_read_model_v2";
const GRAPH_ROOT_DOMAIN: &str = "tos-knowledge-graph-root-v1";

struct CappedWriter {
    bytes: Vec<u8>,
    max: usize,
}
impl Write for CappedWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let next = self
            .bytes
            .len()
            .checked_add(buf.len())
            .ok_or_else(|| io::Error::other("knowledge header overflow"))?;
        if next > self.max {
            return Err(io::Error::other("knowledge header cap"));
        }
        self.bytes.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SealLimits {
    pub max_header_bytes: usize,
}

#[derive(Clone, Debug)]
pub struct KnowledgeSealReceipt {
    pub model_abi: String,
    pub navigation_original_root_sha256: Option<String>,
    pub graph_root_sha256: String,
    pub graph_header_sha256: String,
    pub node_root_sha256: String,
    pub relation_root_sha256: String,
    pub node_count: u64,
    pub relation_count: u64,
    pub catalog_packet_sha256: String,
    pub catalog_index_root_sha256: String,
    pub source_scope_root_sha256: String,
    pub search_index_root_sha256: String,
}

fn canonical(value: &Value, cap: usize) -> Result<Vec<u8>> {
    if cap == 0 || cap > 8 * 1024 * 1024 {
        return Err(Error::Budget("knowledge header cap"));
    }
    // The caller already owns a bounded header value. The strict FND parse
    // rejects duplicate keys and the pinned profile fixes sorted JSON bytes.
    let mut writer = CappedWriter {
        bytes: Vec::new(),
        max: cap,
    };
    serde_json::to_writer(&mut writer, value)
        .map_err(|_| Error::Budget("knowledge header bytes"))?;
    let limits = JsonLimits::new(cap, 96, 1_000_000, 4096)
        .map_err(|_| Error::Budget("knowledge header JSON limits"))?;
    let parsed = parse_json(&writer.bytes, JsonMode::PublishedStrict, limits)
        .map_err(|e| Error::Source(e.to_string()))?;
    canonical_bytes_v1(
        parsed.root(),
        CanonicalProfile::SourceRecordDigestV1,
        limits,
    )
    .map_err(|e| Error::Source(e.to_string()))
}

fn checked_header(header: &Value, nodes: u64, relations: u64) -> Result<&Value> {
    let object = header
        .as_object()
        .ok_or(Error::Invalid("knowledge graph header object"))?;
    let required = [
        "schema",
        "source_revision",
        "normalization_binding",
        "query_properties",
        "counts",
        "authority_boundary",
    ];
    if object.len() != required.len() || required.iter().any(|key| !object.contains_key(*key)) {
        return Err(Error::Invalid("knowledge graph header fields"));
    }
    if header.get("schema").and_then(Value::as_str) != Some("tos_knowledge_graph_v1")
        || header
            .get("source_revision")
            .and_then(Value::as_str)
            .is_none_or(|value| Digest256::from_hex(value).is_err())
        || !header.get("query_properties").is_some_and(Value::is_array)
    {
        return Err(Error::Invalid("knowledge graph header profile"));
    }
    let normalization = header
        .get("normalization_binding")
        .and_then(Value::as_object)
        .ok_or(Error::Invalid("knowledge normalization binding"))?;
    if normalization.len() != 5
        || normalization.get("schema").and_then(Value::as_str)
            != Some("tos_knowledge_graph_normalization_binding_v1")
    {
        return Err(Error::Invalid("knowledge normalization binding profile"));
    }
    for key in [
        "processor_digest",
        "entity_registry_digest",
        "relation_registry_digest",
        "configuration_digest",
    ] {
        if normalization
            .get(key)
            .and_then(Value::as_str)
            .is_none_or(|value| Digest256::from_hex(value).is_err())
        {
            return Err(Error::Invalid("knowledge normalization digest"));
        }
    }
    let counts = header
        .get("counts")
        .and_then(Value::as_object)
        .ok_or(Error::Invalid("knowledge graph counts"))?;
    if counts.get("nodes").and_then(Value::as_u64) != Some(nodes)
        || counts.get("relations").and_then(Value::as_u64) != Some(relations)
        || !counts.get("display_coverage").is_some_and(Value::is_object)
        || !counts.get("semantic_mapping").is_some_and(Value::is_object)
    {
        return Err(Error::Invalid("knowledge graph count mismatch"));
    }
    let authority = header
        .get("authority_boundary")
        .filter(|v| v.is_object())
        .ok_or(Error::Invalid("knowledge graph authority boundary"))?;
    Ok(authority)
}

fn graph_root(
    header_sha: &Digest256,
    nodes: u64,
    relations: u64,
    node_root: &Digest256,
    relation_root: &Digest256,
) -> String {
    let mut hash = Digest256Hasher::new();
    hash.update(&(GRAPH_ROOT_DOMAIN.len() as u64).to_be_bytes());
    hash.update(GRAPH_ROOT_DOMAIN.as_bytes());
    hash.update(header_sha.as_bytes());
    hash.update(&nodes.to_be_bytes());
    hash.update(&relations.to_be_bytes());
    hash.update(node_root.as_bytes());
    hash.update(relation_root.as_bytes());
    hash.finalize().to_hex()
}

fn check_source_counts(
    stage: &mut KnowledgeStage<'_>,
    header: &Value,
    max_sources: usize,
) -> Result<()> {
    let sources = header
        .get("counts")
        .and_then(|counts| counts.get("sources"))
        .and_then(Value::as_object)
        .ok_or(Error::Invalid("knowledge graph source counts"))?;
    if sources.len() > max_sources {
        return Err(Error::Budget("knowledge graph source counts"));
    }
    stage.with_connection(WritePhase::Finalize, |db| {
        let mut statement = db.prepare(
            "SELECT source_graph,expected_node_count FROM source_scope ORDER BY source_graph",
        )?;
        let mut rows = statement.query([])?;
        let mut seen = 0usize;
        let mut populated = 0usize;
        while let Some(row) = rows.next()? {
            let source: String = row.get(0)?;
            let count: i64 = row.get(1)?;
            if count < 0 || seen >= max_sources {
                return Err(Error::Invalid("knowledge source-scope count"));
            }
            seen += 1;
            if count > 0 {
                populated += 1;
                if sources.get(&source).and_then(Value::as_u64) != Some(count as u64) {
                    return Err(Error::Invalid("knowledge graph source count mismatch"));
                }
            } else if sources.contains_key(&source) {
                return Err(Error::Invalid("zero-node source in graph header counts"));
            }
        }
        if seen != max_sources || populated != sources.len() {
            return Err(Error::Invalid("knowledge graph source count closure"));
        }
        Ok(())
    })
}

/// Seal after source scope, catalog and search have all been written. A failed
/// check poisons the private stage, and `finish` repeats core-root verification.
pub fn seal_knowledge_model(
    stage: &mut KnowledgeStage<'_>,
    header: &Value,
    vocabulary: &QueryVocabulary,
    registry: &KnowledgeRegistry,
    scope: &ScopeReceipt,
    catalog: &CatalogIndexReceipt,
    search: &SearchIndexReceipt,
    limits: SealLimits,
) -> Result<KnowledgeSealReceipt> {
    let result = seal_inner(
        stage, header, vocabulary, registry, scope, catalog, search, limits,
    );
    if result.is_err() {
        stage.poison();
    }
    result
}

fn seal_inner(
    stage: &mut KnowledgeStage<'_>,
    header: &Value,
    vocabulary: &QueryVocabulary,
    registry: &KnowledgeRegistry,
    scope: &ScopeReceipt,
    catalog: &CatalogIndexReceipt,
    search: &SearchIndexReceipt,
    limits: SealLimits,
) -> Result<KnowledgeSealReceipt> {
    if limits.max_header_bytes == 0 || limits.max_header_bytes > 1024 * 1024 {
        return Err(Error::Budget("knowledge graph header limit"));
    }
    if registry.entity_registry_id != vocabulary.entity_registry_id
        || registry.relation_registry_id != vocabulary.relation_registry_id
        || catalog.descriptor_sha256 != vocabulary.descriptor_sha256
        || scope.source_count != vocabulary.sources.len()
        || catalog.source_count != scope.source_count as u64
        || search.profile != crate::knowledge_search::SEARCH_PROFILE
    {
        return Err(Error::Invalid("knowledge descriptor/registry binding"));
    }
    let roots = stage.core_roots()?;
    if scope.node_count != roots.nodes
        || scope.relation_count != roots.relations
        || search.node_documents != roots.nodes
        || search.relation_documents != roots.relations
    {
        return Err(Error::Invalid("knowledge component row counts"));
    }
    let authority = checked_header(header, roots.nodes, roots.relations)?;
    check_source_counts(stage, header, vocabulary.sources.len())?;
    let normalization = header
        .get("normalization_binding")
        .ok_or(Error::Invalid("knowledge normalization binding"))?;
    if normalization
        .get("entity_registry_digest")
        .and_then(Value::as_str)
        != Some(registry.entity_semantic_digest.as_str())
        || normalization
            .get("relation_registry_digest")
            .and_then(Value::as_str)
            != Some(registry.relation_semantic_digest.as_str())
    {
        return Err(Error::Invalid("knowledge normalization registry digest"));
    }
    let packet = canonical(header, limits.max_header_bytes)?;
    let authority_packet = canonical(authority, limits.max_header_bytes)?;
    let authority_text = String::from_utf8(authority_packet)
        .map_err(|_| Error::Invalid("knowledge authority UTF-8"))?;
    let header_sha = Digest256::of_bytes(&packet);
    let node_root = Digest256::from_hex(&roots.node_sha256)
        .map_err(|_| Error::Invalid("knowledge node root"))?;
    let relation_root = Digest256::from_hex(&roots.relation_sha256)
        .map_err(|_| Error::Invalid("knowledge relation root"))?;
    for digest in [
        &scope.source_scope_root_sha256,
        &catalog.catalog_packet_sha256,
        &catalog.catalog_index_root_sha256,
        &search.search_index_root_sha256,
        &vocabulary.descriptor_sha256,
        &registry.entity_sha256,
        &registry.relation_sha256,
    ] {
        Digest256::from_hex(digest).map_err(|_| Error::Invalid("knowledge component root"))?;
    }
    let graph_root_sha256 = graph_root(
        &header_sha,
        roots.nodes,
        roots.relations,
        &node_root,
        &relation_root,
    );
    let binding = stage.exact_receipt().binding.clone();
    let navigation = crate::knowledge_navigation_original::verify_stage(
        stage,
        Some(&vocabulary.descriptor_sha256),
    )?;
    let model_abi = if navigation.is_some() {
        crate::KNOWLEDGE_NAVIGATION_MODEL_ABI
    } else {
        KNOWLEDGE_MODEL_ABI
    };
    let mut metadata = vec![
        ("model_abi", model_abi.to_owned()),
        ("descriptor_sha256", vocabulary.descriptor_sha256.clone()),
        (
            "descriptor_version",
            vocabulary.descriptor_version.to_string(),
        ),
        (
            "semantic_primitive_profile",
            vocabulary.semantic_primitive_profile.clone(),
        ),
        ("source_cut", binding.source_cut),
        ("through_commit_seq", binding.through_commit_seq.to_string()),
        ("membership_root", binding.membership_root),
        ("entity_registry_id", registry.entity_registry_id.clone()),
        (
            "entity_registry_version",
            registry.entity_registry_version.to_string(),
        ),
        ("entity_registry_sha256", registry.entity_sha256.clone()),
        (
            "relation_registry_id",
            registry.relation_registry_id.clone(),
        ),
        (
            "relation_registry_version",
            registry.relation_registry_version.to_string(),
        ),
        ("relation_registry_sha256", registry.relation_sha256.clone()),
        ("graph_root_sha256", graph_root_sha256.clone()),
        (
            "catalog_packet_sha256",
            catalog.catalog_packet_sha256.clone(),
        ),
        (
            "catalog_index_root_sha256",
            catalog.catalog_index_root_sha256.clone(),
        ),
        (
            "source_scope_root_sha256",
            scope.source_scope_root_sha256.clone(),
        ),
        (
            "search_index_root_sha256",
            search.search_index_root_sha256.clone(),
        ),
        ("index_generation", binding.index_generation),
        ("route_map_version", binding.route_map_version),
        ("reader_abi", binding.reader_abi),
        ("authority_boundary", authority_text),
        ("node_count", roots.nodes.to_string()),
        ("relation_count", roots.relations.to_string()),
        ("complete", "true".to_owned()),
    ];
    if let Some(r) = &navigation {
        metadata.push((
            "navigation_original_root_sha256",
            r.component_root_sha256.clone(),
        ));
    }
    if metadata.iter().any(|(key, value)| {
        key.len() > 128
            || value.is_empty()
            || value.len()
                > if *key == "authority_boundary" {
                    limits.max_header_bytes
                } else {
                    4096
                }
    }) {
        return Err(Error::Budget("knowledge metadata bytes"));
    }
    stage.with_connection(WritePhase::Finalize, |db| {
        db.execute_batch("SAVEPOINT cmp_knowledge_seal")?;
        let write = (|| {
            db.execute_batch(
                "CREATE TABLE graph_header(\
                singleton INTEGER PRIMARY KEY CHECK(singleton=1),\
                packet_len INTEGER NOT NULL,packet_sha256 BLOB NOT NULL,packet BLOB NOT NULL);\
                CREATE TABLE metadata(key TEXT PRIMARY KEY,value BLOB NOT NULL) WITHOUT ROWID",
            )?;
            db.execute(
                "INSERT INTO graph_header VALUES(1,?1,?2,?3)",
                params![
                    i64::try_from(packet.len())
                        .map_err(|_| Error::Budget("knowledge graph header bytes"))?,
                    &header_sha.as_bytes()[..],
                    &packet,
                ],
            )?;
            let mut insert = db.prepare("INSERT INTO metadata VALUES(?1,?2)")?;
            for (key, value) in metadata {
                insert.execute(params![key, value])?;
            }
            Ok(())
        })();
        if write.is_ok() {
            db.execute_batch("RELEASE cmp_knowledge_seal")?;
        } else {
            db.execute_batch("ROLLBACK TO cmp_knowledge_seal; RELEASE cmp_knowledge_seal")?;
        }
        write
    })?;
    stage.mark_selected_full()?;
    Ok(KnowledgeSealReceipt {
        model_abi: model_abi.into(),
        navigation_original_root_sha256: navigation.map(|r| r.component_root_sha256),
        graph_root_sha256,
        graph_header_sha256: header_sha.to_hex(),
        node_root_sha256: roots.node_sha256,
        relation_root_sha256: roots.relation_sha256,
        node_count: roots.nodes,
        relation_count: roots.relations,
        catalog_packet_sha256: catalog.catalog_packet_sha256.clone(),
        catalog_index_root_sha256: catalog.catalog_index_root_sha256.clone(),
        source_scope_root_sha256: scope.source_scope_root_sha256.clone(),
        search_index_root_sha256: search.search_index_root_sha256.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn header() -> Value {
        json!({
            "schema": "tos_knowledge_graph_v1",
            "source_revision": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "normalization_binding": {
                "schema": "tos_knowledge_graph_normalization_binding_v1",
                "processor_digest": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                "entity_registry_digest": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                "relation_registry_digest": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                "configuration_digest": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
            },
            "query_properties": [],
            "counts": {"nodes": 2, "relations": 1, "sources": {"a": 2},
                "display_coverage": {"node_titles": 2}, "semantic_mapping": {}},
            "authority_boundary": {"is_source": false, "is_canon": false}
        })
    }

    #[test]
    fn preserves_extra_owner_counts_and_refuses_relabelled_root_inputs() {
        let mut value = header();
        checked_header(&value, 2, 1).unwrap();
        assert_ne!(canonical(&value, 1024 * 1024).unwrap(), Vec::<u8>::new());
        value["counts"]["relations"] = json!(2);
        assert!(checked_header(&value, 2, 1).is_err());
        value = header();
        value["normalization_binding"]["processor_digest"] = json!("unpinned");
        assert!(checked_header(&value, 2, 1).is_err());
        value = header();
        value["nodes"] = json!([]);
        assert!(checked_header(&value, 2, 1).is_err());
    }
}
