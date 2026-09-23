//! Materialize an owner-registered `indexed-node-edge-v1` source. This
//! adapter indexes complete already-normalized carriers; it does not invent
//! source meaning or implement other registered source adapters.

use crate::{
    Error, KnowledgeRegistry, QueryVocabulary, Result,
    knowledge_stage::{ExactInputReceipt, KnowledgeStage, NodeRow, RelationRow},
};
use serde_json::Value;
use std::collections::BTreeSet;
use tos_foundation::{Digest256, JsonLimits, JsonMode, parse_json};

const PROFILE: &str = "indexed-node-edge-v1";

#[derive(Clone, Copy, Debug)]
pub struct IndexedLimits {
    pub max_row_bytes: usize,
    pub max_page_rows: usize,
}
impl IndexedLimits {
    fn validate(self) -> Result<()> {
        if self.max_row_bytes == 0
            || self.max_row_bytes > 8 * 1024 * 1024
            || self.max_page_rows == 0
            || self.max_page_rows > 4096
        {
            return Err(Error::Budget("indexed adapter limits"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct IndexedReceipt {
    pub source_count: usize,
    pub node_count: u64,
    pub relation_count: u64,
}

fn required<'a>(row: &'a Value, name: &str) -> Result<&'a str> {
    row.get(name)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.len() <= 4096)
        .ok_or(Error::Invalid("indexed carrier field"))
}
fn parse_carrier(raw: &[u8], cap: usize) -> Result<Value> {
    if raw.len() > cap {
        return Err(Error::Budget("indexed carrier row bytes"));
    }
    let limits = JsonLimits::new(cap, 96, 1_000_000, 4096)
        .map_err(|_| Error::Budget("indexed carrier JSON limits"))?;
    parse_json(raw, JsonMode::PublishedStrict, limits).map_err(|e| Error::Source(e.to_string()))?;
    serde_json::from_slice(raw).map_err(|_| Error::Invalid("indexed carrier JSON"))
}
fn provenance(row: &Value) -> Result<()> {
    let refs = row
        .get("source_refs")
        .and_then(Value::as_array)
        .filter(|r| !r.is_empty())
        .ok_or(Error::Invalid("indexed carrier source refs"))?;
    let mut seen = BTreeSet::new();
    for reference in refs {
        let text = reference
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 4096)
            .ok_or(Error::Invalid("indexed carrier source ref"))?;
        if !seen.insert(text) {
            return Err(Error::Invalid("duplicate indexed source ref"));
        }
    }
    for name in ["display", "epistemic", "attributes", "semantics"] {
        if !row.get(name).is_some_and(Value::is_object) {
            return Err(Error::Invalid("indexed carrier envelope"));
        }
    }
    for name in ["graph_layers", "view_ids"] {
        if !row.get(name).is_some_and(Value::is_array) {
            return Err(Error::Invalid("indexed carrier lists"));
        }
    }
    Digest256::from_hex(required(row, "content_revision")?)
        .map_err(|_| Error::Invalid("indexed content revision"))?;
    Ok(())
}

fn checked_node<'a>(
    row: &'a Value,
    source: &str,
    id: &str,
    registry: &KnowledgeRegistry,
) -> Result<(&'a str, &'a str, &'a str, &'a str)> {
    if required(row, "id")? != id || required(row, "source_graph")? != source {
        return Err(Error::Invalid("indexed node identity/registration"));
    }
    provenance(row)?;
    let native = required(row, "native_id")?;
    let entity = required(row, "entity_id")?;
    let kind = required(row, "kind_id")?;
    let type_id = required(row, "type_id")?;
    let resolved = registry.entity(source, kind);
    if type_id != resolved.type_id
        || row
            .get("type_mapping")
            .and_then(|m| m.get("status"))
            .and_then(Value::as_str)
            != Some(if resolved.mapped {
                "mapped"
            } else {
                "unmapped"
            })
        || row
            .get("type_mapping")
            .and_then(|m| m.get("source_kind_id"))
            .and_then(Value::as_str)
            != Some(kind)
    {
        return Err(Error::Invalid("indexed node semantic crosswalk"));
    }
    Ok((native, entity, kind, type_id))
}
fn checked_relation<'a>(
    row: &'a Value,
    source: &str,
    id: &str,
    registry: &KnowledgeRegistry,
) -> Result<(&'a str, &'a str, &'a str, &'a str, &'a str)> {
    if required(row, "id")? != id || required(row, "source_graph")? != source {
        return Err(Error::Invalid("indexed relation identity/registration"));
    }
    provenance(row)?;
    let native = required(row, "native_id")?;
    let from_id = required(row, "from_id")?;
    let to_id = required(row, "to_id")?;
    let predicate = required(row, "predicate_id")?;
    let relation_type = required(row, "relation_type_id")?;
    let resolved = registry.relation(source, predicate, "edge");
    if relation_type != resolved.type_id
        || row
            .get("predicate_mapping")
            .and_then(|m| m.get("status"))
            .and_then(Value::as_str)
            != Some(if resolved.mapped {
                "mapped"
            } else {
                "unmapped"
            })
        || row
            .get("predicate_mapping")
            .and_then(|m| m.get("source_predicate_id"))
            .and_then(Value::as_str)
            != Some(predicate)
    {
        return Err(Error::Invalid("indexed relation semantic crosswalk"));
    }
    Ok((native, from_id, to_id, predicate, relation_type))
}

fn registered_pair(receipt: &ExactInputReceipt, source: &str, role: &str) -> Result<()> {
    for collection in ["nodes", "relations"] {
        let matches: Vec<_> = receipt
            .collections
            .iter()
            .filter(|entry| entry.source_graph == source && entry.collection == collection)
            .collect();
        if matches.len() != 1
            || matches[0].adapter_profile != PROFILE
            || matches[0].input_role != role
        {
            return Err(Error::Invalid("indexed input collection registration"));
        }
    }
    if receipt.collections.iter().any(|entry| {
        entry.source_graph == source
            && entry.collection != "nodes"
            && entry.collection != "relations"
    }) {
        return Err(Error::Invalid("unknown indexed input collection"));
    }
    Ok(())
}

/// Complete named indexed-source materialization. Caller has already staged
/// every raw row from the independent sealed input receipt. The stage itself
/// verifies count/root and endpoint closure at `finish`; this function cannot
/// derive source completeness from its own output.
pub fn materialize_indexed_sources(
    stage: &mut KnowledgeStage<'_>,
    vocabulary: &QueryVocabulary,
    registry: &KnowledgeRegistry,
    limits: IndexedLimits,
) -> Result<IndexedReceipt> {
    let result = materialize_indexed_sources_inner(stage, vocabulary, registry, limits);
    if result.is_err() {
        stage.poison();
    }
    result
}

fn materialize_indexed_sources_inner(
    stage: &mut KnowledgeStage<'_>,
    vocabulary: &QueryVocabulary,
    registry: &KnowledgeRegistry,
    limits: IndexedLimits,
) -> Result<IndexedReceipt> {
    limits.validate()?;
    let receipt = stage.exact_receipt().clone();
    if registry.entity_registry_id != vocabulary.entity_registry_id
        || registry.relation_registry_id != vocabulary.relation_registry_id
    {
        return Err(Error::Invalid("indexed registry selection mismatch"));
    }
    let mut registered = BTreeSet::new();
    for source in &vocabulary.sources {
        if source.adapter_profile != PROFILE {
            return Err(Error::Invalid(
                "source adapter not implemented by indexed materializer",
            ));
        }
        registered_pair(&receipt, &source.source_graph_id, &source.input_role)?;
        registered.insert(source.source_graph_id.as_str());
    }
    if receipt.collections.len() != vocabulary.sources.len() * 2
        || receipt
            .collections
            .iter()
            .any(|entry| !registered.contains(entry.source_graph.as_str()))
    {
        return Err(Error::Invalid("indexed source coverage incomplete"));
    }
    let mut ordered = vocabulary.sources.iter().collect::<Vec<_>>();
    ordered.sort_by(|a, b| a.source_graph_id.cmp(&b.source_graph_id));
    let mut node_count = 0u64;
    let mut relation_count = 0u64;
    for source in &ordered {
        let mut after: Option<String> = None;
        loop {
            let page = stage.scan_input(
                &source.source_graph_id,
                "nodes",
                after.as_deref(),
                limits.max_page_rows,
            )?;
            for raw in page.rows {
                let row = parse_carrier(&raw.payload, limits.max_row_bytes)?;
                let (native, entity, kind, type_id) =
                    checked_node(&row, &source.source_graph_id, &raw.id, registry)?;
                let source_order =
                    i64::try_from(node_count).map_err(|_| Error::Budget("knowledge node order"))?;
                stage.insert_node(NodeRow {
                    id: &raw.id,
                    source_graph: &source.source_graph_id,
                    native_id: Some(native),
                    entity_id: Some(entity),
                    kind_id: kind,
                    type_id,
                    source_order,
                    payload: &raw.payload,
                })?;
                node_count += 1;
            }
            match page.next_id {
                Some(next) => after = Some(next),
                None => break,
            }
        }
    }
    for source in &ordered {
        let mut after: Option<String> = None;
        loop {
            let page = stage.scan_input(
                &source.source_graph_id,
                "relations",
                after.as_deref(),
                limits.max_page_rows,
            )?;
            for raw in page.rows {
                let row = parse_carrier(&raw.payload, limits.max_row_bytes)?;
                let (native, from_id, to_id, predicate, relation_type) =
                    checked_relation(&row, &source.source_graph_id, &raw.id, registry)?;
                let source_order = i64::try_from(relation_count)
                    .map_err(|_| Error::Budget("knowledge relation order"))?;
                stage.insert_relation(RelationRow {
                    id: &raw.id,
                    source_graph: &source.source_graph_id,
                    native_id: Some(native),
                    from_id,
                    to_id,
                    predicate_id: predicate,
                    relation_type_id: relation_type,
                    source_order,
                    payload: &raw.payload,
                })?;
                relation_count += 1;
            }
            match page.next_id {
                Some(next) => after = Some(next),
                None => break,
            }
        }
    }
    Ok(IndexedReceipt {
        source_count: vocabulary.sources.len(),
        node_count,
        relation_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Limits, ScopeLimits, SourceBinding,
        knowledge_stage::{
            InputCollectionReceipt, InputRow, StageIsolation, StageLimits, StageOwner, WritePhase,
        },
        write_source_scope,
    };
    use std::{
        fs,
        path::Path,
        time::{SystemTime, UNIX_EPOCH},
    };
    use tos_foundation::Digest256Hasher;

    struct FixtureOwner;
    impl StageOwner for FixtureOwner {
        fn verify_receipt(&self, receipt: &ExactInputReceipt) -> Result<()> {
            if receipt.binding.source_cut != "indexed-fixture-cut" {
                return Err(Error::Invalid("fixture cut"));
            }
            Ok(())
        }
        fn recheck_sealed_cut(&self, receipt: &ExactInputReceipt) -> Result<()> {
            self.verify_receipt(receipt)
        }
    }
    // Synthetic small-row test guard. Production must supply an independently
    // quota-backed StageIsolation; this test does not certify host spill limits.
    struct FixtureIsolation;
    impl StageIsolation for FixtureIsolation {
        fn verify(&self, _: &Path, _: StageLimits, _: WritePhase) -> Result<()> {
            Ok(())
        }
    }
    fn root(id: &str, raw: &[u8]) -> String {
        let mut hash = Digest256Hasher::new();
        hash.update(&(id.len() as u64).to_be_bytes());
        hash.update(id.as_bytes());
        hash.update(Digest256::of_bytes(raw).as_bytes());
        hash.finalize().to_hex()
    }
    fn receipt(node: &[u8], relation: &[u8]) -> ExactInputReceipt {
        let collection = |name: &str, id: &str, raw: &[u8]| InputCollectionReceipt {
            source_graph: "eighth".into(),
            collection: name.into(),
            input_role: "source-graph".into(),
            adapter_profile: PROFILE.into(),
            expected_count: 1,
            expected_root_sha256: root(id, raw),
        };
        ExactInputReceipt {
            binding: SourceBinding {
                owner_profile: "fixture-owner".into(),
                source_cut: "indexed-fixture-cut".into(),
                through_commit_seq: 7,
                membership_root: "0".repeat(64),
                index_generation: "fixture-generation".into(),
                route_map_version: "fixture-routes".into(),
                reader_abi: "fixture-reader".into(),
                projection_root_sha256: "1".repeat(64),
                complete: true,
            },
            collections: vec![
                collection("nodes", "eighth:node-1", node),
                collection("relations", "eighth:relation-1", relation),
            ],
        }
    }
    fn eighth_vocabulary() -> QueryVocabulary {
        let raw = include_bytes!(
            "../../../../ToS/doctrine/semantic-interchange/query-vocabulary.v1.json"
        );
        let mut document: Value = serde_json::from_slice(raw).unwrap();
        document["sources"] = serde_json::json!([{
            "source_graph_id": "eighth", "owner_ref": "owner:eighth",
            "input_role": "source-graph", "adapter_profile": PROFILE,
            "representative_priority": 0
        }]);
        document["identity"]["source_dossier_graph_id"] = Value::String("eighth".into());
        QueryVocabulary::parse(&serde_json::to_vec(&document).unwrap(), &[PROFILE]).unwrap()
    }
    fn candidate(label: &str) -> std::path::PathBuf {
        let tick = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("tos-indexed-{label}-{}-{tick}", std::process::id()));
        fs::create_dir(&dir).unwrap();
        dir.join("candidate.sqlite3")
    }

    #[test]
    fn eighth_source_unknown_vocabulary_uses_declared_registry_fallback() {
        let entity =
            include_bytes!("../../../../ToS/doctrine/semantic-interchange/entity-types.v1.json");
        let relation =
            include_bytes!("../../../../ToS/doctrine/semantic-interchange/relation-types.v1.json");
        let registry = KnowledgeRegistry::parse(entity, relation).unwrap();
        let node = serde_json::json!({
            "id":"eighth:node-1", "native_id":"node-1", "entity_id":"external.subject.1",
            "source_graph":"eighth", "kind_id":"unknown-owner-kind", "type_id":"tos.entity.unmapped",
            "type_mapping":{"status":"unmapped","source_kind_id":"unknown-owner-kind"},
            "content_revision":"0".repeat(64), "display":{}, "epistemic":{}, "attributes":{},
            "semantics":{}, "graph_layers":[], "view_ids":[], "source_refs":["owner:record-1"]
        });
        let (native, entity, kind, type_id) =
            checked_node(&node, "eighth", "eighth:node-1", &registry).unwrap();
        assert_eq!(
            (native, entity, kind, type_id),
            (
                "node-1",
                "external.subject.1",
                "unknown-owner-kind",
                "tos.entity.unmapped"
            )
        );
        let mut forged = node.clone();
        forged["type_id"] = Value::String("tos.entity.concept".into());
        assert!(checked_node(&forged, "eighth", "eighth:node-1", &registry).is_err());
    }

    #[test]
    fn eighth_registered_source_materializes_actual_node_and_relation() {
        let entity =
            include_bytes!("../../../../ToS/doctrine/semantic-interchange/entity-types.v1.json");
        let relation_registry =
            include_bytes!("../../../../ToS/doctrine/semantic-interchange/relation-types.v1.json");
        let registry = KnowledgeRegistry::parse(entity, relation_registry).unwrap();
        let vocabulary = eighth_vocabulary();
        let node = serde_json::to_vec(&serde_json::json!({
            "id":"eighth:node-1", "native_id":"node-1", "entity_id":"external.subject.1",
            "source_graph":"eighth", "kind_id":"unknown-owner-kind", "type_id":"tos.entity.unmapped",
            "type_mapping":{"status":"unmapped","source_kind_id":"unknown-owner-kind"},
            "content_revision":"0".repeat(64), "display":{}, "epistemic":{}, "attributes":{},
            "semantics":{}, "graph_layers":[], "view_ids":[], "source_refs":["owner:record-1"]
        })).unwrap();
        let relation = serde_json::to_vec(&serde_json::json!({
            "id":"eighth:relation-1", "native_id":"relation-1",
            "from_id":"eighth:node-1", "to_id":"eighth:node-1",
            "source_graph":"eighth", "predicate_id":"unknown-owner-relation",
            "relation_type_id":"tos.relation.unmapped",
            "predicate_mapping":{"status":"unmapped","source_predicate_id":"unknown-owner-relation"},
            "content_revision":"1".repeat(64), "display":{}, "epistemic":{}, "attributes":{},
            "semantics":{}, "graph_layers":[], "view_ids":[], "source_refs":["owner:record-1"]
        })).unwrap();
        let sealed = receipt(&node, &relation);
        let path = candidate("complete");
        let owner = FixtureOwner;
        let isolation = FixtureIsolation;
        let mut stage = KnowledgeStage::create(
            &path,
            StageLimits {
                sqlite: Limits::default(),
                max_temp_bytes: 64 * 1024 * 1024,
                max_seek_rows: 8,
                max_seek_bytes: 1024 * 1024,
            },
            sealed.clone(),
            &owner,
            &isolation,
        )
        .unwrap();
        stage
            .ingest_input(InputRow {
                source_graph: "eighth",
                collection: "nodes",
                id: "eighth:node-1",
                payload: &node,
            })
            .unwrap();
        stage
            .ingest_input(InputRow {
                source_graph: "eighth",
                collection: "relations",
                id: "eighth:relation-1",
                payload: &relation,
            })
            .unwrap();
        let indexed = materialize_indexed_sources(
            &mut stage,
            &vocabulary,
            &registry,
            IndexedLimits {
                max_row_bytes: 1024 * 1024,
                max_page_rows: 1,
            },
        )
        .unwrap();
        assert_eq!(
            (
                indexed.source_count,
                indexed.node_count,
                indexed.relation_count
            ),
            (1, 1, 1)
        );
        let scope = write_source_scope(
            &mut stage,
            &vocabulary,
            ScopeLimits {
                max_sources: 2,
                max_rows: 4,
                max_index_work_bytes: 4096,
            },
        )
        .unwrap();
        assert_eq!(
            (scope.source_count, scope.node_count, scope.relation_count),
            (1, 1, 1)
        );
        let finished = stage.finish().unwrap();
        assert_eq!(
            (
                finished.input_rows,
                finished.node_rows,
                finished.relation_rows
            ),
            (2, 1, 1)
        );
        let db = rusqlite::Connection::open(&path).unwrap();
        let actual: (String,String) = db.query_row(
            "SELECT n.id,r.id FROM knowledge_nodes n JOIN knowledge_relations r ON r.from_id=n.id",
            [], |row| Ok((row.get(0)?,row.get(1)?))).unwrap();
        assert_eq!(actual, ("eighth:node-1".into(), "eighth:relation-1".into()));
        let scope_rows: i64 = db
            .query_row(
                "SELECT count(*) FROM source_scope WHERE source_graph='eighth'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(scope_rows, 1);
        drop(db);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn missing_registered_input_collection_refuses_before_materialization() {
        let entity =
            include_bytes!("../../../../ToS/doctrine/semantic-interchange/entity-types.v1.json");
        let relation =
            include_bytes!("../../../../ToS/doctrine/semantic-interchange/relation-types.v1.json");
        let registry = KnowledgeRegistry::parse(entity, relation).unwrap();
        let vocabulary = eighth_vocabulary();
        let mut sealed = receipt(b"{}", b"{}");
        sealed.collections.pop();
        let path = candidate("missing");
        let owner = FixtureOwner;
        let isolation = FixtureIsolation;
        let mut stage = KnowledgeStage::create(
            &path,
            StageLimits {
                sqlite: Limits::default(),
                max_temp_bytes: 64 * 1024 * 1024,
                max_seek_rows: 8,
                max_seek_bytes: 1024 * 1024,
            },
            sealed.clone(),
            &owner,
            &isolation,
        )
        .unwrap();
        assert!(
            materialize_indexed_sources(
                &mut stage,
                &vocabulary,
                &registry,
                IndexedLimits {
                    max_row_bytes: 1024 * 1024,
                    max_page_rows: 1
                }
            )
            .is_err()
        );
        assert!(stage.finish().is_err());
        assert!(!path.exists());
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
