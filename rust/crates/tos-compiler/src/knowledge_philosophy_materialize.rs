//! Native philosophy base producer over the prepared, exact source cut.
//! Global placeholders, inherited views and readable context remain assembly
//! obligations; rows emitted here are private bases, never semantic admission.

use crate::knowledge_base::{BaseNodeOverrides, BaseNormalizationLimits, KnowledgeBaseNormalizer};
use crate::knowledge_normalization::SourceRow;
use crate::knowledge_philosophy_prepare::dependency_root;
use crate::knowledge_stage::{KnowledgeStage, NodeRow, RelationRow, SeekRow, WritePhase};
use crate::{Error, KnowledgeRegistry, PhilosophyPrepareReceipt, QueryVocabulary, Result};
use rusqlite::OptionalExtension;
use serde_json::Value;
use tos_foundation::Digest256;

const PROFILE: &str = "philosophy-node-edge-v1";

#[derive(Clone, Copy, Debug)]
pub struct PhilosophyMaterializeLimits {
    pub max_raw_bytes: usize,
    pub max_output_bytes: usize,
    pub max_registry_bytes: usize,
    pub max_page_rows: usize,
    pub max_rows: u64,
    pub max_work_bytes: u64,
}
impl PhilosophyMaterializeLimits {
    pub(crate) fn validate(self) -> Result<()> {
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
        {
            return Err(Error::Budget("philosophy materialization limits"));
        }
        Ok(())
    }
}

/// Exact normalized endpoint titles selected by the global title owner.
/// A root binds that owner's complete title universe, not a two-row inference.
pub struct PhilosophyRelationGlobalInputs<'a> {
    pub source_cut: &'a str,
    pub endpoint_title_root_sha256: &'a str,
    pub left_title: &'a Value,
    pub right_title: &'a Value,
}

pub struct PhilosophyBase {
    value: Value,
    ordered_source_raw: Vec<u8>,
    pub source_cut: String,
    pub prepared_dependency_root_sha256: String,
    pub raw_sha256: String,
    pub entity_registry_sha256: String,
    pub relation_registry_sha256: String,
    pub endpoint_title_root_sha256: Option<String>,
}
impl PhilosophyBase {
    pub fn value(&self) -> &Value {
        &self.value
    }
    pub fn ordered_source_raw(&self) -> &[u8] {
        &self.ordered_source_raw
    }
}

pub struct PhilosophyNormalizer<'a> {
    registry: &'a KnowledgeRegistry,
    source_graph: String,
    shared: KnowledgeBaseNormalizer<'a>,
    limits: PhilosophyMaterializeLimits,
}

fn required<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.len() <= 4096 && !s.contains('\0'))
        .ok_or(Error::Invalid("philosophy normalized field"))
}
impl<'a> PhilosophyNormalizer<'a> {
    pub fn new(
        registry: &'a KnowledgeRegistry,
        entity_bytes: &[u8],
        relation_bytes: &[u8],
        vocabulary: &QueryVocabulary,
        descriptor_bytes: &[u8],
        limits: PhilosophyMaterializeLimits,
    ) -> Result<Self> {
        Self::new_with_optional_owned_state(
            registry,
            entity_bytes,
            relation_bytes,
            vocabulary,
            descriptor_bytes,
            limits,
            None,
        )
    }
    pub(crate) fn new_with_optional_owned_state(
        registry: &'a KnowledgeRegistry,
        entity_bytes: &[u8],
        relation_bytes: &[u8],
        vocabulary: &QueryVocabulary,
        descriptor_bytes: &[u8],
        limits: PhilosophyMaterializeLimits,
        state: Option<&'a crate::d1_public_capture::CreationState<'a>>,
    ) -> Result<Self> {
        limits.validate()?;
        vocabulary.verify_authored_bytes(descriptor_bytes)?;
        if registry.entity_registry_id != vocabulary.entity_registry_id
            || registry.relation_registry_id != vocabulary.relation_registry_id
        {
            return Err(Error::Invalid("philosophy descriptor registry identity"));
        }
        if entity_bytes.len() > limits.max_registry_bytes
            || relation_bytes.len() > limits.max_registry_bytes
            || Digest256::of_bytes(entity_bytes).to_hex() != registry.entity_sha256
            || Digest256::of_bytes(relation_bytes).to_hex() != registry.relation_sha256
        {
            return Err(Error::Invalid("philosophy selected registry bytes"));
        }
        let sources: Vec<_> = vocabulary
            .sources
            .iter()
            .filter(|s| s.adapter_profile == PROFILE)
            .collect();
        if sources.len() != 1 {
            return Err(Error::Invalid("philosophy selected materializer"));
        }
        let base_limits = BaseNormalizationLimits {
            max_registry_bytes: limits.max_registry_bytes,
            max_output_bytes: limits.max_output_bytes,
        };
        let shared = match state {
            Some(state) => KnowledgeBaseNormalizer::new_with_owned_state(
                registry,
                entity_bytes,
                relation_bytes,
                vocabulary,
                descriptor_bytes,
                base_limits,
                state,
            )?,
            None => KnowledgeBaseNormalizer::new(
                registry,
                entity_bytes,
                relation_bytes,
                vocabulary,
                descriptor_bytes,
                base_limits,
            )?,
        };
        if let Some(state) = state {
            state.retain(sources[0].source_graph_id.len())?;
            state.charge_work(sources[0].source_graph_id.len())?;
        }
        Ok(Self {
            registry,
            source_graph: sources[0].source_graph_id.clone(),
            shared,
            limits,
        })
    }
    fn bind(&self, stage: &KnowledgeStage<'_>, prepared: &PhilosophyPrepareReceipt) -> Result<()> {
        if prepared.source_graph != self.source_graph
            || prepared.final_graph_rows_written
            || prepared.source_cut != stage.exact_receipt()?.binding.source_cut
        {
            return Err(Error::Invalid("philosophy materializer selected cut"));
        }
        Digest256::from_hex(&prepared.dependency_root_sha256)
            .map_err(|_| Error::Invalid("philosophy preparation root"))?;
        for (collection, count, root) in [
            ("nodes", prepared.nodes, &prepared.node_input_root_sha256),
            ("edges", prepared.edges, &prepared.edge_input_root_sha256),
        ] {
            let registered = stage
                .exact_receipt()?
                .collections
                .iter()
                .find(|r| r.source_graph == self.source_graph && r.collection == collection)
                .ok_or(Error::Invalid("philosophy registered materializer input"))?;
            if registered.expected_count != count
                || &registered.expected_root_sha256 != root
                || registered.adapter_profile != PROFILE
                || registered.input_role != prepared.input_role
            {
                return Err(Error::Invalid("philosophy prepared input receipt"));
            }
        }
        Ok(())
    }
    fn raw(
        &self,
        stage: &mut KnowledgeStage<'_>,
        prepared: &PhilosophyPrepareReceipt,
        native: &str,
        relation: bool,
    ) -> Result<SeekRow> {
        let raw = stage
            .raw_by_id(
                &self.source_graph,
                if relation { "edges" } else { "nodes" },
                native,
            )?
            .ok_or(Error::Invalid("missing philosophy raw row"))?;
        self.verify_raw(stage, prepared, &raw, relation)?;
        Ok(raw)
    }
    fn verify_raw(
        &self,
        stage: &mut KnowledgeStage<'_>,
        prepared: &PhilosophyPrepareReceipt,
        raw: &SeekRow,
        relation: bool,
    ) -> Result<()> {
        let native = &raw.id;
        self.bind(stage, prepared)?;
        let sql = if relation {
            "SELECT raw_sha256 FROM knowledge_philosophy_edges WHERE edge_id=?1"
        } else {
            "SELECT raw_sha256 FROM knowledge_philosophy_nodes WHERE node_id=?1"
        };
        let sha: Vec<u8> = stage.with_connection(WritePhase::Sort, |db| {
            db.query_row(sql, [native], |r| r.get(0))
                .optional()?
                .ok_or(Error::Invalid("missing philosophy prepared row"))
        })?;
        if raw.payload.len() > self.limits.max_raw_bytes
            || sha != Digest256::of_bytes(&raw.payload).as_bytes()
            || raw.payload_sha256 != Digest256::of_bytes(&raw.payload).to_hex()
        {
            return Err(Error::Invalid("philosophy prepared raw binding"));
        }
        Ok(())
    }
    fn base(
        &self,
        value: Value,
        raw: SeekRow,
        prepared: &PhilosophyPrepareReceipt,
        title_root: Option<String>,
    ) -> PhilosophyBase {
        PhilosophyBase {
            value,
            ordered_source_raw: raw.payload,
            source_cut: prepared.source_cut.clone(),
            prepared_dependency_root_sha256: prepared.dependency_root_sha256.clone(),
            raw_sha256: raw.payload_sha256,
            entity_registry_sha256: self.registry.entity_sha256.clone(),
            relation_registry_sha256: self.registry.relation_sha256.clone(),
            endpoint_title_root_sha256: title_root,
        }
    }
    pub fn normalize_node(
        &self,
        stage: &mut KnowledgeStage<'_>,
        prepared: &PhilosophyPrepareReceipt,
        native: &str,
    ) -> Result<PhilosophyBase> {
        let raw = self.raw(stage, prepared, native, false)?;
        let source = SourceRow::parse(&raw.payload, self.limits.max_raw_bytes)?;
        if required(source.value(), "node_id")? != native {
            return Err(Error::Invalid("philosophy raw node identity"));
        }
        let value = self.shared.normalize_node(
            &source,
            &self.source_graph,
            false,
            BaseNodeOverrides::default(),
        )?;
        Ok(self.base(value, raw, prepared, None))
    }
    pub fn relation_endpoints(
        &self,
        stage: &mut KnowledgeStage<'_>,
        prepared: &PhilosophyPrepareReceipt,
        native: &str,
    ) -> Result<(String, String)> {
        let raw = self.raw(stage, prepared, native, true)?;
        let source = SourceRow::parse(&raw.payload, self.limits.max_raw_bytes)?;
        endpoints(source.value(), &self.source_graph)
    }
    pub fn normalize_relation(
        &self,
        stage: &mut KnowledgeStage<'_>,
        prepared: &PhilosophyPrepareReceipt,
        native: &str,
        global: PhilosophyRelationGlobalInputs<'_>,
    ) -> Result<PhilosophyBase> {
        if global.source_cut != prepared.source_cut
            || Digest256::from_hex(global.endpoint_title_root_sha256).is_err()
        {
            return Err(Error::Invalid("philosophy global title receipt"));
        }
        let raw = self.raw(stage, prepared, native, true)?;
        let source = SourceRow::parse(&raw.payload, self.limits.max_raw_bytes)?;
        if required(source.value(), "edge_id")? != native {
            return Err(Error::Invalid("philosophy raw edge identity"));
        }
        let value = self.shared.normalize_relation(
            &source,
            &self.source_graph,
            None,
            global.left_title,
            global.right_title,
            "derived-export",
        )?;
        Ok(self.base(
            value,
            raw,
            prepared,
            Some(global.endpoint_title_root_sha256.into()),
        ))
    }
}

fn text(value: Option<&Value>) -> Option<&str> {
    value?.as_str().map(str::trim).filter(|s| !s.is_empty())
}
fn endpoints(item: &Value, source: &str) -> Result<(String, String)> {
    Ok((
        format!(
            "{}:{}",
            text(item.get("from_source_graph")).unwrap_or(source),
            required(item, "from_id")?.trim()
        ),
        format!(
            "{}:{}",
            text(item.get("to_source_graph")).unwrap_or(source),
            required(item, "to_id")?.trim()
        ),
    ))
}

#[derive(Clone, Debug)]
pub struct PhilosophyMaterializeReceipt {
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
    normalizer: &PhilosophyNormalizer<'_>,
    prepared: &PhilosophyPrepareReceipt,
    relation: bool,
    title_root: Option<&str>,
    mut titles: F,
) -> Result<PhilosophyMaterializeReceipt>
where
    F: FnMut(&mut KnowledgeStage<'_>, &str, &str) -> Result<(Value, Value)>,
{
    normalizer.bind(stage, prepared)?;
    if dependency_root(stage)? != prepared.dependency_root_sha256 {
        return Err(Error::Invalid("philosophy materialization prepared root"));
    }
    let expected = if relation {
        prepared.edges
    } else {
        prepared.nodes
    };
    if expected > normalizer.limits.max_rows {
        return Err(Error::Budget("philosophy materialization rows"));
    }
    let mut after = None;
    let mut rows = 0u64;
    let mut work_bytes = 0u64;
    let mut order: i64 = stage.with_connection(WritePhase::Sort, |db| {
        db.query_row(
            if relation {
                "SELECT coalesce(max(source_order)+1,0) FROM knowledge_relations"
            } else {
                "SELECT coalesce(max(source_order)+1,0) FROM knowledge_nodes"
            },
            [],
            |r| r.get(0),
        )
        .map_err(Error::from)
    })?;
    loop {
        let (page_rows, physical_rows, physical_bytes) = stage.exact_source_write_page_limits(
            normalizer.limits.max_page_rows,
            normalizer.limits.max_output_bytes,
            normalizer.limits.max_raw_bytes,
        )?;
        let page = stage.scoped_scan_input(
            &normalizer.source_graph,
            if relation { "edges" } else { "nodes" },
            after.as_deref(),
            page_rows,
        )?;
        stage.with_write_page(
            WritePhase::Normalized,
            physical_rows,
            physical_bytes,
            |stage| {
                for raw in &page.rows {
                    normalizer.verify_raw(stage, prepared, raw, relation)?;
                    let state = stage.owned_creation_state();
                    let source = SourceRow::parse_scoped_with_optional_owned_state(
                        &raw.payload,
                        normalizer.limits.max_raw_bytes,
                        state,
                    )?;
                    if required(source.value(), if relation { "edge_id" } else { "node_id" })?
                        != raw.id
                    {
                        return Err(Error::Invalid("philosophy raw row identity"));
                    }
                    let endpoint_titles = if relation {
                        let root = title_root
                            .ok_or(Error::Invalid("philosophy missing global title root"))?;
                        Digest256::from_hex(root)
                            .map_err(|_| Error::Invalid("philosophy global title receipt"))?;
                        let (from, to) = endpoints(source.value(), &normalizer.source_graph)?;
                        Some(titles(stage, &from, &to)?)
                    } else {
                        None
                    };
                    let mut write = |value: &Value, bytes: &[u8]| -> Result<()> {
                        work_bytes = work_bytes
                            .checked_add(raw.payload.len() as u64)
                            .and_then(|n| n.checked_add(bytes.len() as u64))
                            .ok_or(Error::Budget("philosophy materialization work"))?;
                        rows = rows
                            .checked_add(1)
                            .ok_or(Error::Budget("philosophy materialization rows"))?;
                        if work_bytes > normalizer.limits.max_work_bytes || rows > expected {
                            return Err(Error::Budget("philosophy materialization work/rows"));
                        }
                        if relation {
                            stage.insert_relation_with_exact_source(
                                RelationRow {
                                    id: required(value, "id")?,
                                    source_graph: &normalizer.source_graph,
                                    native_id: Some(&raw.id),
                                    from_id: required(value, "from_id")?,
                                    to_id: required(value, "to_id")?,
                                    predicate_id: required(value, "predicate_id")?,
                                    relation_type_id: required(value, "relation_type_id")?,
                                    source_order: order,
                                    payload: &bytes,
                                },
                                &raw.payload,
                            )?;
                        } else {
                            stage.insert_node_with_exact_source(
                                NodeRow {
                                    id: required(value, "id")?,
                                    source_graph: &normalizer.source_graph,
                                    native_id: Some(&raw.id),
                                    entity_id: Some(required(value, "entity_id")?),
                                    kind_id: required(value, "kind_id")?,
                                    type_id: required(value, "type_id")?,
                                    source_order: order,
                                    payload: &bytes,
                                },
                                &raw.payload,
                            )?;
                        }
                        Ok(())
                    };
                    if let Some(state) = state {
                        normalizer.shared.ensure_same_owned_state(state)?;
                        if let Some((left, right)) = &endpoint_titles {
                            normalizer.shared.with_normalized_relation_owned(
                                &source,
                                &normalizer.source_graph,
                                None,
                                left,
                                right,
                                "derived-export",
                                normalizer.limits.max_output_bytes,
                                &mut write,
                            )?;
                        } else {
                            normalizer.shared.with_normalized_node_owned(
                                &source,
                                &normalizer.source_graph,
                                false,
                                BaseNodeOverrides::default(),
                                normalizer.limits.max_output_bytes,
                                &mut write,
                            )?;
                        }
                    } else {
                        let value = if let Some((left, right)) = &endpoint_titles {
                            normalizer.shared.normalize_relation(
                                &source,
                                &normalizer.source_graph,
                                None,
                                left,
                                right,
                                "derived-export",
                            )?
                        } else {
                            normalizer.shared.normalize_node(
                                &source,
                                &normalizer.source_graph,
                                false,
                                BaseNodeOverrides::default(),
                            )?
                        };
                        let bytes = serde_json::to_vec(&value)
                            .map_err(|_| Error::Invalid("philosophy normalized serialization"))?;
                        write(&value, &bytes)?;
                    }
                    order = order
                        .checked_add(1)
                        .ok_or(Error::Budget("philosophy materialization order"))?;
                }
                Ok(())
            },
        )?;
        match page.into_next_id()? {
            Some(next) => after = Some(next),
            None => break,
        }
    }
    if rows != expected {
        return Err(Error::Invalid("philosophy materialization completeness"));
    }
    Ok(PhilosophyMaterializeReceipt {
        source_graph: normalizer.source_graph.clone(),
        source_cut: prepared.source_cut.clone(),
        rows,
        work_bytes,
        prepared_dependency_root_sha256: prepared.dependency_root_sha256.clone(),
        endpoint_title_root_sha256: title_root.map(str::to_owned),
        finalization_complete: false,
    })
}
/// Append a bounded private node base pass. Parent must globally reorder before
/// sealing and finalize inherited views/readable context over all sources.
pub fn materialize_philosophy_nodes(
    stage: &mut KnowledgeStage<'_>,
    normalizer: &PhilosophyNormalizer<'_>,
    prepared: &PhilosophyPrepareReceipt,
) -> Result<PhilosophyMaterializeReceipt> {
    let result = materialize(stage, normalizer, prepared, false, None, |_, _, _| {
        Err(Error::Invalid("unexpected philosophy title lookup"))
    });
    if result.is_err() {
        stage.poison();
    }
    result
}
/// The lookup resolves complete global titles including parent-owned missing
/// endpoint placeholders; this producer never invents a title or placeholder.
pub fn materialize_philosophy_relations<F>(
    stage: &mut KnowledgeStage<'_>,
    normalizer: &PhilosophyNormalizer<'_>,
    prepared: &PhilosophyPrepareReceipt,
    source_cut: &str,
    title_root_sha256: &str,
    titles: F,
) -> Result<PhilosophyMaterializeReceipt>
where
    F: FnMut(&mut KnowledgeStage<'_>, &str, &str) -> Result<(Value, Value)>,
{
    let result = (|| {
        if source_cut != prepared.source_cut || Digest256::from_hex(title_root_sha256).is_err() {
            return Err(Error::Invalid("philosophy selected global title cut"));
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
