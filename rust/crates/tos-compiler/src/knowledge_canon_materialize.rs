//! Native canon and candidate producers over exact prepared collections.
//! The complete assembler supplies endpoint titles, placeholders and finalizers.
use crate::knowledge_base::{BaseNodeOverrides, BaseNormalizationLimits, KnowledgeBaseNormalizer};
use crate::knowledge_canon_prepare::{
    CANDIDATE_PROFILE, CANON_PROFILE, CanonPrepareReceipt, canonical_digest, dependency_root,
    required, text,
};
use crate::knowledge_normalization::{SourceRow, stamp_content_revision};
use crate::knowledge_stage::{KnowledgeStage, NodeRow, RelationRow, WritePhase};
use crate::{Error, KnowledgeRegistry, QueryVocabulary, Result};
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};
use tos_foundation::Digest256;

#[derive(Clone, Copy, Debug)]
pub struct CanonMaterializeLimits {
    pub max_raw_bytes: usize,
    pub max_output_bytes: usize,
    pub max_registry_bytes: usize,
    pub max_page_rows: usize,
    pub max_page_bytes: usize,
    pub max_rows: u64,
    pub max_work_bytes: u64,
}
impl CanonMaterializeLimits {
    fn validate(self) -> Result<()> {
        if self.max_raw_bytes == 0
            || self.max_raw_bytes > 8 * 1024 * 1024
            || self.max_output_bytes == 0
            || self.max_output_bytes > 8 * 1024 * 1024
            || self.max_registry_bytes == 0
            || self.max_registry_bytes > 4 * 1024 * 1024
            || self.max_page_rows == 0
            || self.max_page_rows > 1024
            || self.max_rows == 0
            || self.max_work_bytes == 0
            || self.max_page_bytes == 0
            || self.max_page_bytes > 64 * 1024 * 1024
            || self
                .max_page_rows
                .checked_mul(self.max_raw_bytes)
                .is_none_or(|n| n > self.max_page_bytes)
        {
            return Err(Error::Budget("canon materialization limits"));
        }
        Ok(())
    }
}
pub struct CanonNormalizer<'a> {
    source_graph: String,
    profile: String,
    shared: KnowledgeBaseNormalizer<'a>,
    limits: CanonMaterializeLimits,
}
impl<'a> CanonNormalizer<'a> {
    pub fn new(
        registry: &'a KnowledgeRegistry,
        entity_bytes: &[u8],
        relation_bytes: &[u8],
        vocabulary: &QueryVocabulary,
        descriptor_bytes: &[u8],
        limits: CanonMaterializeLimits,
    ) -> Result<Self> {
        Self::family(
            registry,
            entity_bytes,
            relation_bytes,
            vocabulary,
            descriptor_bytes,
            limits,
            CANON_PROFILE,
        )
    }
    pub(crate) fn family(
        registry: &'a KnowledgeRegistry,
        entity_bytes: &[u8],
        relation_bytes: &[u8],
        vocabulary: &QueryVocabulary,
        descriptor_bytes: &[u8],
        limits: CanonMaterializeLimits,
        profile: &str,
    ) -> Result<Self> {
        limits.validate()?;
        vocabulary.verify_authored_bytes(descriptor_bytes)?;
        let sources = vocabulary
            .sources
            .iter()
            .filter(|s| s.adapter_profile == profile)
            .collect::<Vec<_>>();
        if sources.len() != 1
            || registry.entity_registry_id != vocabulary.entity_registry_id
            || registry.relation_registry_id != vocabulary.relation_registry_id
        {
            return Err(Error::Invalid("canon descriptor binding"));
        }
        let shared = KnowledgeBaseNormalizer::new(
            registry,
            entity_bytes,
            relation_bytes,
            vocabulary,
            descriptor_bytes,
            BaseNormalizationLimits {
                max_registry_bytes: limits.max_registry_bytes,
                max_output_bytes: limits.max_output_bytes,
            },
        )?;
        Ok(Self {
            source_graph: sources[0].source_graph_id.clone(),
            profile: profile.into(),
            shared,
            limits,
        })
    }
    pub fn source_graph(&self) -> &str {
        &self.source_graph
    }
    fn bind(&self, stage: &KnowledgeStage<'_>, prepared: &CanonPrepareReceipt) -> Result<()> {
        if prepared.source_graph != self.source_graph
            || prepared.adapter_profile != self.profile
            || prepared.final_graph_rows_written
            || prepared.source_cut != stage.exact_receipt().binding.source_cut
        {
            return Err(Error::Invalid("canon materializer cut"));
        }
        let entries = stage
            .exact_receipt()
            .collections
            .iter()
            .filter(|r| r.source_graph == self.source_graph)
            .collect::<Vec<_>>();
        if entries.len() != prepared.collections.len() {
            return Err(Error::Invalid("canon materializer collection coverage"));
        }
        for c in &prepared.collections {
            let matches = entries
                .iter()
                .filter(|r| r.collection == c.collection)
                .collect::<Vec<_>>();
            if matches.len() != 1
                || matches[0].expected_count != c.count
                || matches[0].expected_root_sha256 != c.root_sha256
                || matches[0].adapter_profile != self.profile
                || matches[0].input_role != prepared.input_role
            {
                return Err(Error::Invalid("canon materializer exact inputs"));
            }
        }
        Ok(())
    }
    pub fn normalize_node(
        &self,
        stage: &mut KnowledgeStage<'_>,
        prepared: &CanonPrepareReceipt,
        native: &str,
    ) -> Result<Value> {
        self.bind(stage, prepared)?;
        if self.profile != CANON_PROFILE {
            return Err(Error::Invalid("candidate family has no nodes"));
        }
        let raw = source_material(
            stage,
            prepared,
            &format!("{}:{native}", self.source_graph),
            false,
            self.limits.max_raw_bytes,
        )?;
        let source = SourceRow::parse(&raw, self.limits.max_raw_bytes)?;
        let digest = canonical_digest(&raw, source.value(), self.limits.max_raw_bytes)?;
        let mut value = self.shared.normalize_node(
            &source,
            &self.source_graph,
            true,
            BaseNodeOverrides::default(),
        )?;
        if let Some(digest) = digest {
            value["attributes"]["source_record"] = source.value()["properties"].clone();
            value["attributes"]["source_sha256"] = json!(digest);
            value["attributes"]["source_file_sha256"] = source
                .value()
                .get("source_sha256")
                .cloned()
                .unwrap_or(Value::Null);
            let map = value["source_record"]["field_map"]
                .as_object_mut()
                .ok_or(Error::Invalid("canon normalized source field map"))?;
            for (key, path) in [
                ("attributes.source_record", "/properties"),
                ("attributes.source_sha256", "/source_record_sha256"),
                ("attributes.source_file_sha256", "/source_sha256"),
            ] {
                map.insert(key.into(), json!(path));
            }
            stamp_content_revision(&mut value, self.limits.max_output_bytes)?;
        }
        Ok(value)
    }
    pub fn normalize_relation(
        &self,
        stage: &mut KnowledgeStage<'_>,
        prepared: &CanonPrepareReceipt,
        identity: &str,
        left: &Value,
        right: &Value,
    ) -> Result<Value> {
        self.bind(stage, prepared)?;
        let material = source_material(
            stage,
            prepared,
            &format!("{}:{identity}", self.source_graph),
            true,
            self.limits.max_raw_bytes,
        )?;
        let source = SourceRow::parse(&material, self.limits.max_raw_bytes)?;
        self.shared.normalize_relation(
            &source,
            &self.source_graph,
            Some(identity),
            left,
            right,
            if self.profile == CANON_PROFILE {
                "canon"
            } else {
                "derived-export"
            },
        )
    }
}
/// Exact ordered source-material witness for the all-source finalizer. Relation
/// lookup uses normalized identity, including pack qualification and synthesized
/// node-local identity. It rechecks the retained origin and material digests.
pub fn source_material(
    stage: &mut KnowledgeStage<'_>,
    prepared: &CanonPrepareReceipt,
    normalized_id: &str,
    relation: bool,
    max_bytes: usize,
) -> Result<Vec<u8>> {
    if max_bytes == 0
        || max_bytes > 8 * 1024 * 1024
        || prepared.source_cut != stage.exact_receipt().binding.source_cut
    {
        return Err(Error::Invalid("canon source-material binding"));
    }
    let prefix = format!("{}:", prepared.source_graph);
    let identity = normalized_id
        .strip_prefix(&prefix)
        .ok_or(Error::Invalid("canon source-material normalized identity"))?;
    if relation {
        let (material,sha,collection,origin,origin_sha):(Vec<u8>,Vec<u8>,String,String,Vec<u8>)=stage.with_connection(WritePhase::Sort,|db|{
            db.query_row("SELECT CASE WHEN length(material)<=?3 THEN material ELSE NULL END,material_sha256,origin_collection,origin_id,origin_sha256 FROM knowledge_canon_proposals WHERE source_graph=?1 AND identity_id=?2",params![prepared.source_graph,identity,max_bytes as i64],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).optional()?.ok_or(Error::Invalid("canon source-material relation absent"))
        })?;
        let raw = stage
            .raw_by_id(&prepared.source_graph, &collection, &origin)?
            .ok_or(Error::Invalid("canon proposal origin absent"))?;
        if material.len() > max_bytes
            || raw.payload.len() > max_bytes
            || sha.as_slice() != Digest256::of_bytes(&material).as_bytes()
            || origin_sha.as_slice() != Digest256::of_bytes(&raw.payload).as_bytes()
        {
            return Err(Error::Invalid("canon proposal source-material digest"));
        }
        Ok(material)
    } else {
        let sha: Vec<u8> = stage.with_connection(WritePhase::Sort, |db| {
            db.query_row(
                "SELECT raw_sha256 FROM knowledge_canon_nodes WHERE source_graph=?1 AND node_id=?2",
                params![prepared.source_graph, identity],
                |r| r.get(0),
            )
            .optional()?
            .ok_or(Error::Invalid("canon source-material node absent"))
        })?;
        let raw = stage
            .raw_by_id(&prepared.source_graph, "nodes", identity)?
            .ok_or(Error::Invalid("canon source-material raw node absent"))?;
        if raw.payload.len() > max_bytes
            || sha.as_slice() != Digest256::of_bytes(&raw.payload).as_bytes()
        {
            return Err(Error::Invalid("canon node source-material digest"));
        }
        Ok(raw.payload)
    }
}
#[derive(Clone, Debug)]
pub struct CanonPreparedRelation {
    pub identity_id: String,
    pub native_id: String,
    pub from_id: String,
    pub to_id: String,
    pub material: Vec<u8>,
}
/// Bounded proposal enumeration supplies the placeholder phase before titles.
pub fn scan_canon_relations(
    stage: &mut KnowledgeStage<'_>,
    prepared: &CanonPrepareReceipt,
    after: Option<&str>,
    max_rows: usize,
    max_bytes: usize,
    max_page_bytes: usize,
) -> Result<Vec<CanonPreparedRelation>> {
    if max_rows == 0
        || max_rows > 1024
        || max_bytes == 0
        || max_bytes > 8 * 1024 * 1024
        || max_page_bytes == 0
        || max_page_bytes > 64 * 1024 * 1024
        || max_rows
            .checked_mul(max_bytes)
            .is_none_or(|n| n > max_page_bytes)
    {
        return Err(Error::Budget("canon relation scan limits"));
    }
    let identities:Vec<(String,String)>=stage.with_connection(WritePhase::Sort,|db| {
        let mut query=db.prepare("SELECT identity_id,native_id FROM knowledge_canon_proposals WHERE source_graph=?1 AND (?2 IS NULL OR identity_id>?2) ORDER BY identity_id LIMIT ?3")?;
        let rows=query.query_map(params![prepared.source_graph,after,max_rows as i64],|r|Ok((r.get(0)?,r.get(1)?)))?;
        rows.collect::<std::result::Result<Vec<_>,_>>().map_err(Error::from)
    })?;
    let mut out = Vec::with_capacity(identities.len());
    for (identity, native) in identities {
        let material = source_material(
            stage,
            prepared,
            &format!("{}:{identity}", prepared.source_graph),
            true,
            max_bytes,
        )?;
        let source = SourceRow::parse(&material, max_bytes)?;
        let item = source.value();
        let from = format!(
            "{}:{}",
            text(item.get("from_source_graph")).unwrap_or(&prepared.source_graph),
            text(item.get("from_id")).unwrap_or("unknown-source")
        );
        let to = format!(
            "{}:{}",
            text(item.get("to_source_graph")).unwrap_or(&prepared.source_graph),
            text(item.get("to_id")).unwrap_or("unknown-target")
        );
        out.push(CanonPreparedRelation {
            identity_id: identity,
            native_id: native,
            from_id: from,
            to_id: to,
            material,
        });
    }
    Ok(out)
}
#[derive(Clone, Debug)]
pub struct CanonMaterializeReceipt {
    pub source_graph: String,
    pub source_cut: String,
    pub rows: u64,
    pub work_bytes: u64,
    pub prepared_dependency_root_sha256: String,
    pub endpoint_title_root_sha256: Option<String>,
    pub finalization_complete: bool,
}
fn materialize<F>(
    stage: &mut KnowledgeStage<'_>,
    normalizer: &CanonNormalizer<'_>,
    prepared: &CanonPrepareReceipt,
    relation: bool,
    title_root: Option<&str>,
    mut titles: F,
) -> Result<CanonMaterializeReceipt>
where
    F: FnMut(&mut KnowledgeStage<'_>, &str, &str) -> Result<(Value, Value)>,
{
    normalizer.bind(stage, prepared)?;
    if dependency_root(stage, &prepared.source_graph)? != prepared.dependency_root_sha256 {
        return Err(Error::Invalid("canon prepared dependency drift"));
    }
    let expected = if relation {
        prepared
            .relation_edges
            .checked_add(prepared.node_relations)
            .ok_or(Error::Budget("canon relations"))?
    } else {
        prepared.nodes
    };
    if expected > normalizer.limits.max_rows {
        return Err(Error::Budget("canon materialize rows"));
    }
    let mut order: i64 = stage.with_connection(WritePhase::Sort, |db| {
        Ok(db.query_row(
            if relation {
                "SELECT coalesce(max(source_order)+1,0) FROM knowledge_relations"
            } else {
                "SELECT coalesce(max(source_order)+1,0) FROM knowledge_nodes"
            },
            [],
            |r| r.get(0),
        )?)
    })?;
    let mut rows = 0u64;
    let mut work = 0u64;
    let mut after = None;
    loop {
        let batch = if relation {
            scan_canon_relations(
                stage,
                prepared,
                after.as_deref(),
                normalizer.limits.max_page_rows,
                normalizer.limits.max_raw_bytes,
                normalizer.limits.max_page_bytes,
            )?
        } else {
            stage
                .scan_input(
                    &prepared.source_graph,
                    "nodes",
                    after.as_deref(),
                    normalizer.limits.max_page_rows,
                )?
                .rows
                .into_iter()
                .map(|r| CanonPreparedRelation {
                    identity_id: r.id.clone(),
                    native_id: r.id,
                    from_id: String::new(),
                    to_id: String::new(),
                    material: r.payload,
                })
                .collect()
        };
        if batch.is_empty() {
            break;
        }
        for row in batch {
            let value = if relation {
                let (left, right) = titles(stage, &row.from_id, &row.to_id)?;
                normalizer.normalize_relation(stage, prepared, &row.identity_id, &left, &right)?
            } else {
                normalizer.normalize_node(stage, prepared, &row.native_id)?
            };
            let bytes =
                serde_json::to_vec(&value).map_err(|_| Error::Invalid("canon normalized JSON"))?;
            rows = rows
                .checked_add(1)
                .ok_or(Error::Budget("canon materialize rows"))?;
            work = work
                .checked_add(row.material.len() as u64)
                .and_then(|n| n.checked_add(bytes.len() as u64))
                .ok_or(Error::Budget("canon materialize work"))?;
            if rows > expected
                || work > normalizer.limits.max_work_bytes
                || bytes.len() > normalizer.limits.max_output_bytes
            {
                return Err(Error::Budget("canon materialize work/bytes"));
            }
            if relation {
                stage.insert_relation(RelationRow {
                    id: required(&value, "id")?,
                    source_graph: &prepared.source_graph,
                    native_id: Some(&row.native_id),
                    from_id: required(&value, "from_id")?,
                    to_id: required(&value, "to_id")?,
                    predicate_id: required(&value, "predicate_id")?,
                    relation_type_id: required(&value, "relation_type_id")?,
                    source_order: order,
                    payload: &bytes,
                })?;
            } else {
                stage.insert_node(NodeRow {
                    id: required(&value, "id")?,
                    source_graph: &prepared.source_graph,
                    native_id: Some(&row.native_id),
                    entity_id: Some(required(&value, "entity_id")?),
                    kind_id: required(&value, "kind_id")?,
                    type_id: required(&value, "type_id")?,
                    source_order: order,
                    payload: &bytes,
                })?;
            }
            order = order
                .checked_add(1)
                .ok_or(Error::Budget("canon materialize order"))?;
            after = Some(row.identity_id);
        }
    }
    if rows != expected {
        return Err(Error::Invalid("canon materialize completeness"));
    }
    Ok(CanonMaterializeReceipt {
        source_graph: prepared.source_graph.clone(),
        source_cut: prepared.source_cut.clone(),
        rows,
        work_bytes: work,
        prepared_dependency_root_sha256: prepared.dependency_root_sha256.clone(),
        endpoint_title_root_sha256: title_root.map(str::to_owned),
        finalization_complete: false,
    })
}
pub fn materialize_canon_nodes(
    stage: &mut KnowledgeStage<'_>,
    normalizer: &CanonNormalizer<'_>,
    prepared: &CanonPrepareReceipt,
) -> Result<CanonMaterializeReceipt> {
    let result = (|| {
        if prepared.adapter_profile != CANON_PROFILE {
            return Err(Error::Invalid("canon node family"));
        }
        materialize(stage, normalizer, prepared, false, None, |_, _, _| {
            Err(Error::Invalid("canon node title lookup"))
        })
    })();
    if result.is_err() {
        stage.poison();
    }
    result
}
pub fn materialize_canon_relations<F>(
    stage: &mut KnowledgeStage<'_>,
    normalizer: &CanonNormalizer<'_>,
    prepared: &CanonPrepareReceipt,
    source_cut: &str,
    title_root_sha256: &str,
    titles: F,
) -> Result<CanonMaterializeReceipt>
where
    F: FnMut(&mut KnowledgeStage<'_>, &str, &str) -> Result<(Value, Value)>,
{
    let result = (|| {
        if source_cut != prepared.source_cut
            || Digest256::from_hex(title_root_sha256).is_err()
            || ![CANON_PROFILE, CANDIDATE_PROFILE].contains(&prepared.adapter_profile.as_str())
        {
            return Err(Error::Invalid("canon selected global title cut"));
        }
        materialize(
            stage,
            normalizer,
            prepared,
            true,
            Some(title_root_sha256),
            titles,
        )
    })();
    if result.is_err() {
        stage.poison();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge_candidates::{
        candidate_normalizer, materialize_candidate_relations, prepare_candidate_inputs,
    };
    use crate::knowledge_canon_prepare::{CanonPrepareLimits, prepare_canon_inputs};
    use crate::knowledge_stage::{
        ExactInputReceipt, InputCollectionReceipt, InputRow, StageIsolation, StageLimits,
        StageOwner,
    };
    use crate::{Limits, SourceBinding};
    use std::{
        fs,
        path::Path,
        time::{SystemTime, UNIX_EPOCH},
    };
    use tos_foundation::Digest256Hasher;
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
    fn fixture_node() -> Value {
        json!({"node_id":"tos.concept.alpha","node_type":"concept","label":"Navigation aid","source_path":"ToS/canon/concept/alpha/node.json","source_ref":"ToS/canon/concept/alpha/node.json","source_sha256":"1".repeat(64),"source_record_sha256":"e7cd7584ab85288daceef3f7e8a3c660fe9c4cb7f19abf7dd7e0173dfea0a9dd","properties":{"schema_version":"tos_canonical_node_v1","node_id":"tos.concept.alpha","node_type":"concept","record_version":1,"name":"Источник Альфа","relations":[null,{"relation":"related_to","target_ref":"tos.concept.beta"}]}})
    }
    #[test]
    fn canonical_retained_source_identity_and_digest_refuse_drift() {
        let source = fixture_node();
        let raw = serde_json::to_vec(&source).unwrap();
        assert!(canonical_digest(&raw, &source, 8192).unwrap().is_some());
        for field in ["node_id", "node_type", "source_record_sha256"] {
            let mut bad = source.clone();
            bad[field] = json!("different");
            let bytes = serde_json::to_vec(&bad).unwrap();
            assert!(canonical_digest(&bytes, &bad, 8192).is_err());
        }
        let mut bad = source;
        bad["properties"]["record_version"] = json!(0);
        let bytes = serde_json::to_vec(&bad).unwrap();
        assert!(canonical_digest(&bytes, &bad, 8192).is_err());
    }
    /// One focused raw-fixture test covers the durable family boundary: exact
    /// collection roots, zero-edge packs, canonical wording, node-local order,
    /// pack identity, candidate views, and finalizer material lookup.
    /// Revision literals come from CMP-canon-candidates-oracle.py (CPython).
    #[test]
    fn python_oracle_raw_canon_and_candidate_native_bases() {
        let entity =
            include_bytes!("../../../../ToS/doctrine/semantic-interchange/entity-types.v1.json");
        let relations =
            include_bytes!("../../../../ToS/doctrine/semantic-interchange/relation-types.v1.json");
        let descriptor = include_bytes!(
            "../../../../ToS/doctrine/semantic-interchange/query-vocabulary.v1.json"
        );
        let vocabulary = QueryVocabulary::parse(
            descriptor,
            &[
                "philosophy-node-edge-v1",
                "canon-node-relation-v1",
                "candidate-relation-v1",
                "source-navigation-node-edge-v1",
                "reified-bibliographic-claims-v1",
                "declared-identity-and-source-ref-joins-v1",
                "repository-topology-v1",
                "indexed-node-edge-v1",
            ],
        )
        .unwrap();
        let registry = KnowledgeRegistry::parse(entity, relations).unwrap();
        let mut rows: Vec<(String, String, String, Vec<u8>)> = Vec::new();
        rows.push((
            "canon".into(),
            "nodes".into(),
            "tos.concept.alpha".into(),
            serde_json::to_vec(&fixture_node()).unwrap(),
        ));
        for (graph, owner, path) in [
            ("canon", "ToS/canon", "ToS/canon/relations/a/edges.csv"),
            (
                "candidate-intake",
                "ToS/candidate-intake",
                "ToS/candidate-intake/a/edges.csv",
            ),
        ] {
            for (pack, count) in [("pack-a", 1), ("pack-empty", 0)] {
                rows.push((graph.into(),"relation_packs".into(),pack.into(),serde_json::to_vec(&json!({"pack_id":pack,"path":path,"owner_branch":owner,"edge_count":count,"sha256":"2".repeat(64)})).unwrap()));
            }
            rows.push((graph.into(),"relation_edges".into(),"pack-a:edge-a".into(),serde_json::to_vec(&json!({"edge_id":"edge-a","pack_id":"pack-a","from_id":"tos.concept.alpha","to_id":"tos.concept.beta","predicate_id":"related_to","owner_branch":owner,"authority_layer":if graph=="canon"{"canon"}else{"candidate-intake"},"status":"source-recorded"})).unwrap()));
        }
        let mut collections = Vec::new();
        for graph in ["canon", "candidate-intake"] {
            for collection in if graph == "canon" {
                vec!["nodes", "relation_packs", "relation_edges"]
            } else {
                vec!["relation_packs", "relation_edges"]
            } {
                let mut selected = rows
                    .iter()
                    .filter(|r| r.0 == graph && r.1 == collection)
                    .collect::<Vec<_>>();
                selected.sort_by(|a, b| a.2.cmp(&b.2));
                let mut hash = Digest256Hasher::new();
                for row in &selected {
                    crate::knowledge_canon_prepare::framed(&mut hash, &row.2);
                    hash.update(Digest256::of_bytes(&row.3).as_bytes());
                }
                collections.push(InputCollectionReceipt {
                    source_graph: graph.into(),
                    collection: collection.into(),
                    input_role: vocabulary
                        .sources
                        .iter()
                        .find(|s| s.source_graph_id == graph)
                        .unwrap()
                        .input_role
                        .clone(),
                    adapter_profile: if graph == "canon" {
                        CANON_PROFILE
                    } else {
                        CANDIDATE_PROFILE
                    }
                    .into(),
                    expected_count: selected.len() as u64,
                    expected_root_sha256: hash.finalize().to_hex(),
                });
            }
        }
        let sealed = ExactInputReceipt {
            binding: SourceBinding {
                owner_profile: "fixture-owner".into(),
                source_cut: "canon-fixture-cut".into(),
                through_commit_seq: 1,
                membership_root: "0".repeat(64),
                index_generation: "fixture-generation".into(),
                route_map_version: "fixture-route".into(),
                reader_abi: "fixture-reader".into(),
                projection_root_sha256: "1".repeat(64),
                complete: true,
            },
            collections,
        };
        let dir = std::env::temp_dir().join(format!(
            "tos-canon-family-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&dir).unwrap();
        let owner = Owner;
        let quota = Quota;
        let mut stage = KnowledgeStage::create(
            &dir.join("candidate.sqlite3"),
            StageLimits {
                sqlite: Limits::default(),
                max_temp_bytes: 64 * 1024 * 1024,
                max_seek_rows: 2,
                max_seek_bytes: 16384,
            },
            sealed,
            &owner,
            &quota,
        )
        .unwrap();
        for (graph, collection, id, payload) in rows {
            stage
                .ingest_input(InputRow {
                    source_graph: &graph,
                    collection: &collection,
                    id: &id,
                    payload: &payload,
                })
                .unwrap();
        }
        let limits = CanonPrepareLimits {
            max_nodes: 8,
            max_packs: 8,
            max_edges: 8,
            max_node_relations: 8,
            max_page_rows: 1,
            max_row_bytes: 8192,
            max_work_bytes: 256 * 1024,
        };
        let canon = prepare_canon_inputs(&mut stage, &vocabulary, limits).unwrap();
        let candidate = prepare_candidate_inputs(&mut stage, &vocabulary, limits).unwrap();
        assert_eq!(
            (
                canon.nodes,
                canon.packs,
                canon.relation_edges,
                canon.node_relations
            ),
            (1, 2, 1, 1)
        );
        assert_eq!(
            (candidate.nodes, candidate.packs, candidate.relation_edges),
            (0, 2, 1)
        );
        assert_eq!(stage.core_roots().unwrap().nodes, 0);
        let ml = CanonMaterializeLimits {
            max_raw_bytes: 8192,
            max_output_bytes: 65536,
            max_registry_bytes: 4 * 1024 * 1024,
            max_page_rows: 1,
            max_page_bytes: 8192,
            max_rows: 16,
            max_work_bytes: 512 * 1024,
        };
        let cn = CanonNormalizer::new(&registry, entity, relations, &vocabulary, descriptor, ml)
            .unwrap();
        let pn = candidate_normalizer(&registry, entity, relations, &vocabulary, descriptor, ml)
            .unwrap();
        let node = cn
            .normalize_node(&mut stage, &canon, "tos.concept.alpha")
            .unwrap();
        assert_eq!(node["display"]["title"]["default"], "Источник Альфа");
        assert_eq!(
            node["content_revision"],
            "73cac16002be670e7e56577584a099fdd7cd778224ef682ec6d2fe42708d02f8"
        );
        materialize_canon_nodes(&mut stage, &cn, &canon).unwrap();
        let left = node["display"]["title"].clone();
        let right = json!({"default":"Бета"});
        let proposals = scan_canon_relations(&mut stage, &canon, None, 2, 8192, 16384).unwrap();
        assert_eq!(
            proposals[0].identity_id,
            "node-relation:506aebc21d2cfd9b99dd1289"
        );
        let local = cn
            .normalize_relation(&mut stage, &canon, &proposals[0].identity_id, &left, &right)
            .unwrap();
        assert_eq!(
            local["content_revision"],
            "fb95572d8cba3574c31650410732fb70650ce701637fc8c5a42408929926f96f"
        );
        let pack = cn
            .normalize_relation(&mut stage, &canon, "pack-a:edge-a", &left, &right)
            .unwrap();
        assert_eq!(pack["id"], "canon:pack-a:edge-a");
        assert_eq!(
            pack["content_revision"],
            "8d98cc7ce0901552668ec3eb7f0c3df7020aa799bcab6f308981c951eafaadc6"
        );
        materialize_canon_relations(
            &mut stage,
            &cn,
            &canon,
            "canon-fixture-cut",
            &"3".repeat(64),
            |_, _, _| Ok((left.clone(), right.clone())),
        )
        .unwrap();
        materialize_candidate_relations(
            &mut stage,
            &pn,
            &candidate,
            "canon-fixture-cut",
            &"3".repeat(64),
            |_, _, _| Ok((left.clone(), right.clone())),
        )
        .unwrap();
        let source = source_material(
            &mut stage,
            &candidate,
            "candidate-intake:pack-a:edge-a",
            true,
            8192,
        )
        .unwrap();
        let material: Value = serde_json::from_slice(&source).unwrap();
        assert_eq!(material["view_ids"], json!(["promotion-flow"]));
        assert_eq!(material["source_ref"], "ToS/candidate-intake/a/edges.csv");
        crate::order_native_graph_rows(&mut stage, 16, 65536).unwrap();
        assert_eq!(stage.core_roots().unwrap().relations, 3);
        drop(stage);
        fs::remove_dir(&dir).unwrap();
    }
}
