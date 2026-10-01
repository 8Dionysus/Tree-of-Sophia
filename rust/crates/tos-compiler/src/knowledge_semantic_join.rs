//! Declared identity and exact source-reference joins over the complete base
//! graph. These are projection/grounding routes, never identity assertions.

use crate::knowledge_base::KnowledgeBaseNormalizer;
use crate::knowledge_global_titles::CompleteBaseNodes;
use crate::knowledge_normalization::{SourceRow, stable_digest};
use crate::knowledge_repository::{
    TopologyLimits, bytes, charge, required, root_item, source_for, strings, text,
};
use crate::knowledge_stage::{KnowledgeStage, RelationRow, WritePhase};
use crate::{Error, QueryVocabulary, Result};
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use tos_foundation::{Digest256, Digest256Hasher};

const PROFILE: &str = "declared-identity-and-source-ref-joins-v1";

#[derive(Clone, Debug)]
pub struct SemanticJoinReceipt {
    pub source_graph: String,
    pub source_cut: String,
    pub descriptor_sha256: String,
    pub base_nodes: u64,
    pub base_node_root_sha256: String,
    pub relations: u64,
    pub dependency_root_sha256: String,
}

fn optional_source(vocab: &QueryVocabulary, profile: &str) -> Result<Option<String>> {
    let mut matches = vocab
        .sources
        .iter()
        .filter(|s| s.adapter_profile == profile);
    let source = matches.next().map(|s| s.source_graph_id.clone());
    if matches.next().is_some() {
        return Err(Error::Invalid("ambiguous semantic join source"));
    }
    Ok(source)
}

// The maintained join rule uses the declared persistent namespace. Derive
// that prefix from the selected grammar, and refuse unimplemented grammar
// shapes instead of silently changing the owner rule.
fn identity_prefix(vocab: &QueryVocabulary) -> Result<String> {
    if vocab.shared_entity_id_grammars.len() != 1 {
        return Err(Error::Invalid("semantic identity grammar coverage"));
    }
    let grammar = &vocab.shared_entity_id_grammars[0];
    let literal = grammar
        .strip_prefix('^')
        .and_then(|s| s.strip_suffix("[a-z0-9]+(?:[.-][a-z0-9]+)*$"))
        .ok_or(Error::Invalid("unsupported semantic identity grammar"))?;
    let prefix = literal.replace("\\.", ".");
    if prefix.is_empty()
        || !prefix
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
    {
        return Err(Error::Invalid("semantic identity namespace"));
    }
    Ok(prefix)
}

// Computational pair rule shared by the admitted stage and supplied whole
// metadata caller. It carries no stage, cut, receipt or admission authority.
#[allow(clippy::too_many_arguments)]
fn shared_identity_pair(
    left: &SourceRow,
    right: &SourceRow,
    left_id: &str,
    right_id: &str,
    entity: &str,
    claims: &str,
    nav: &str,
    entity_ref: &str,
    prefix: &str,
) -> Result<Option<Value>> {
    if !entity.starts_with(prefix) {
        return Ok(None);
    }
    assessed_parity(left.value(), right.value())?;
    let mut refs = strings(left.value().get("source_refs"));
    refs.extend(strings(right.value().get("source_refs")));
    refs.insert(entity_ref.into());
    let digest = stable_digest(&json!([left_id, right_id]))?;
    Ok(Some(
        json!({"edge_id":format!("projects:{entity}:{}",&digest[..16]),"from_id":required(left.value(),"native_id")?,"from_source_graph":claims,"to_id":required(right.value(),"native_id")?,"to_source_graph":nav,"predicate_id":"projects","source_refs":refs,"graph_layers":["semantic-interchange"],"properties":{"derivation":"shared-persistent-tos-entity-id","entity_id":entity,"relation_label":"projects","note":"Связь соединяет две проекции одного объявленного устойчивого ToS ID и не создаёт новое утверждение same_as."}}),
    ))
}

/// A bounded computational renderer over two supplied normalized carriers.
/// The caller proves complete cohort ownership and source admission separately.
pub fn render_supplied_shared_identity_pair(
    left: &SourceRow,
    right: &SourceRow,
    vocab: &QueryVocabulary,
    descriptor_bytes: &[u8],
    max_row_bytes: usize,
) -> Result<Option<Value>> {
    if max_row_bytes == 0 || max_row_bytes > 8_388_608 {
        return Err(Error::Budget("supplied shared identity row bytes"));
    }
    if descriptor_bytes.is_empty() || descriptor_bytes.len() > 1_048_576 {
        return Err(Error::Budget("supplied shared identity descriptor bytes"));
    }
    vocab.verify_authored_bytes(descriptor_bytes)?;
    let limits = TopologyLimits {
        max_rows: 1,
        max_page_rows: 1,
        max_row_bytes,
        max_work_bytes: 1,
    };
    bytes(left.value(), limits)?;
    bytes(right.value(), limits)?;
    let claims = optional_source(vocab, "reified-bibliographic-claims-v1")?
        .ok_or(Error::Invalid("supplied shared identity claims source"))?;
    let nav = optional_source(vocab, "source-navigation-node-edge-v1")?
        .ok_or(Error::Invalid("supplied shared identity navigation source"))?;
    for (row, graph) in [(left, &claims), (right, &nav)] {
        if required(row.value(), "source_graph")? != graph
            || required(row.value(), "id")?
                != format!("{graph}:{}", required(row.value(), "native_id")?)
        {
            return Err(Error::Invalid(
                "supplied shared identity normalized identity",
            ));
        }
    }
    let entity = required(left.value(), "entity_id")?;
    if entity != required(right.value(), "entity_id")? {
        return Ok(None);
    }
    let descriptor = SourceRow::parse(descriptor_bytes, 1_048_576)?;
    let entity_ref = required(
        &descriptor.value()["semantic_registry_refs"]["entity"],
        "source_ref",
    )?;
    let result = shared_identity_pair(
        left,
        right,
        required(left.value(), "id")?,
        required(right.value(), "id")?,
        entity,
        &claims,
        &nav,
        entity_ref,
        &identity_prefix(vocab)?,
    )?;
    if let Some(value) = &result {
        bytes(value, limits)?;
    }
    Ok(result)
}

fn exact_form(value: &Value) -> bool {
    value.is_object()
        && text(value.get("id")).is_some()
        && value
            .get("version")
            .and_then(Value::as_u64)
            .is_some_and(|n| n > 0)
        && value
            .get("digest")
            .and_then(Value::as_str)
            .is_some_and(|d| Digest256::from_prefixed(d).is_ok())
}
fn assessed_parity(left: &Value, right: &Value) -> Result<()> {
    let attrs = [left.get("attributes"), right.get("attributes")];
    let forms = attrs.map(|a| a.and_then(|a| a.get("human_forms")));
    let mut selected = BTreeSet::new();
    for collection in forms {
        for packet in collection.and_then(Value::as_array).into_iter().flatten() {
            if packet.is_object() && packet.get("assessment_snapshot").is_some() {
                let form = packet
                    .get("form")
                    .ok_or(Error::Invalid("semantic assessed form"))?;
                if !exact_form(form) {
                    return Err(Error::Invalid("semantic assessed form reference"));
                }
                selected.insert(required(form, "id")?.to_owned());
            }
        }
    }
    if selected.is_empty() {
        return Ok(());
    }
    let arrays = forms.map(|f| f.and_then(Value::as_array));
    let [Some(first), Some(second)] = arrays else {
        return Err(Error::Invalid("semantic assessed forms disagreement"));
    };
    for key in ["source_record", "source_claim"] {
        if attrs[0].and_then(|a| a.get(key)) != attrs[1].and_then(|a| a.get(key)) {
            return Err(Error::Invalid("semantic assessed source disagreement"));
        }
    }
    for id in selected {
        let matches = |forms: &[Value]| {
            forms
                .iter()
                .enumerate()
                .filter(|(_, p)| {
                    p.is_object()
                        && p.pointer("/form/id").and_then(Value::as_str) == Some(id.as_str())
                })
                .map(|(index, _)| index)
                .collect::<Vec<_>>()
        };
        let left = matches(first);
        let right = matches(second);
        if left.len() != 1 || right.len() != 1 || first[left[0]] != second[right[0]] {
            return Err(Error::Invalid("semantic assessed snapshot disagreement"));
        }
    }
    Ok(())
}

fn insert(
    stage: &mut KnowledgeStage<'_>,
    value: Value,
    limits: TopologyLimits,
    count: &mut u64,
    work: &mut u64,
) -> Result<()> {
    let raw = bytes(&value, limits)?;
    charge(work, raw.len(), limits)?;
    *count = count
        .checked_add(1)
        .filter(|n| *n <= limits.max_rows)
        .ok_or(Error::Budget("semantic prepared relations"))?;
    let sha = Digest256::of_bytes(&raw);
    stage.with_connection(WritePhase::Sort, |db| {
        db.execute(
            "INSERT INTO knowledge_semantic_material VALUES (?1,?2,?3,?4)",
            params![
                required(&value, "edge_id")?,
                raw.len() as i64,
                &sha.as_bytes()[..],
                raw
            ],
        )?;
        Ok(())
    })
}

fn page(
    stage: &mut KnowledgeStage<'_>,
    after: &str,
    limits: TopologyLimits,
) -> Result<Vec<(String, Vec<u8>)>> {
    stage.with_connection(WritePhase::Sort, |db| {
        let mut stmt = db.prepare("SELECT id,CASE WHEN material_len=length(material) AND length(material)<=?2 THEN material ELSE NULL END,material_sha256 FROM knowledge_semantic_material WHERE id>?1 ORDER BY id LIMIT ?3")?;
        let mut rows = stmt.query(params![after,limits.max_row_bytes as i64,limits.max_page_rows as i64])?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            let raw: Vec<u8> = row.get::<_,Option<Vec<u8>>>(1)?.ok_or(Error::Budget("semantic prepared bytes"))?;
            if row.get::<_,Vec<u8>>(2)? != Digest256::of_bytes(&raw).as_bytes() { return Err(Error::Invalid("semantic prepared SHA")); }
            out.push((row.get(0)?,raw));
        }
        Ok(out)
    })
}

fn root(
    stage: &mut KnowledgeStage<'_>,
    receipt: &SemanticJoinReceipt,
    limits: TopologyLimits,
) -> Result<(u64, String)> {
    let mut hash = Digest256Hasher::new();
    hash.update(b"tos-semantic-join-v1\0");
    for value in [
        &receipt.source_graph,
        &receipt.source_cut,
        &receipt.descriptor_sha256,
        &receipt.base_node_root_sha256,
    ] {
        root_item(&mut hash, value, b"");
    }
    hash.update(&receipt.base_nodes.to_be_bytes());
    let mut after = String::new();
    let mut count = 0u64;
    let mut work = 0;
    loop {
        let rows = page(stage, &after, limits)?;
        if rows.is_empty() {
            break;
        }
        for (id, raw) in rows {
            charge(&mut work, raw.len(), limits)?;
            count += 1;
            if count > limits.max_rows {
                return Err(Error::Budget("semantic dependency rows"));
            }
            root_item(&mut hash, &id, Digest256::of_bytes(&raw).as_bytes());
            after = id;
        }
    }
    Ok((count, hash.finalize().to_hex()))
}

/// The exact owner receipt explicitly registers an empty `join_scope`
/// collection for this derived family. Its actual dependencies are the
/// complete base-node seal and same-cut upstream source collections.
pub fn prepare_semantic_joins(
    stage: &mut KnowledgeStage<'_>,
    vocab: &QueryVocabulary,
    descriptor_bytes: &[u8],
    base: &CompleteBaseNodes,
    limits: TopologyLimits,
) -> Result<SemanticJoinReceipt> {
    let result = (|| {
        limits.validate()?;
        if limits
            .max_page_rows
            .checked_mul(limits.max_row_bytes)
            .and_then(|n| n.checked_mul(2))
            .is_none_or(|n| n > 64 * 1024 * 1024)
        {
            return Err(Error::Budget("semantic pair page bytes"));
        }
        vocab.verify_authored_bytes(descriptor_bytes)?;
        let source = source_for(vocab, PROFILE)?;
        let registration = vocab
            .sources
            .iter()
            .find(|s| s.source_graph_id == source)
            .unwrap();
        let roots = stage.core_roots()?;
        if base.source_cut != stage.exact_receipt()?.binding.source_cut
            || base.node_count != roots.nodes
            || base.node_root_sha256 != roots.node_sha256
        {
            return Err(Error::Invalid("semantic complete base node closure"));
        }
        let coverage = stage
            .exact_receipt()?
            .collections
            .iter()
            .filter(|e| e.source_graph == source)
            .collect::<Vec<_>>();
        if coverage.len() != 1
            || coverage[0].collection != "join_scope"
            || coverage[0].adapter_profile != PROFILE
            || coverage[0].input_role != registration.input_role
            || coverage[0].expected_count != 0
            || coverage[0].expected_root_sha256 != Digest256::of_bytes(b"").to_hex()
        {
            return Err(Error::Invalid("semantic join scope registration"));
        }
        let descriptor = SourceRow::parse(descriptor_bytes, 1024 * 1024)?;
        let entity_ref = required(
            &descriptor.value()["semantic_registry_refs"]["entity"],
            "source_ref",
        )?;
        let relation_ref = required(
            &descriptor.value()["semantic_registry_refs"]["relation"],
            "source_ref",
        )?;
        let prefix = identity_prefix(vocab)?;
        let navigation = optional_source(vocab, "source-navigation-node-edge-v1")?;
        let claims = optional_source(vocab, "reified-bibliographic-claims-v1")?;
        let canon = optional_source(vocab, "canon-node-relation-v1")?;
        stage.with_connection(WritePhase::Schema,|db| {db.execute_batch("CREATE TABLE knowledge_semantic_material(id TEXT PRIMARY KEY,material_len INTEGER NOT NULL,material_sha256 BLOB NOT NULL,material BLOB NOT NULL) WITHOUT ROWID; CREATE TABLE knowledge_semantic_canon_path(path TEXT PRIMARY KEY,node_id TEXT NOT NULL) WITHOUT ROWID;")?;Ok(())})?;
        let mut count = 0;
        let mut work = 0;
        if let (Some(claims), Some(nav)) = (&claims, &navigation) {
            let mut after = (String::new(), String::new());
            loop {
                let pairs=stage.with_connection(WritePhase::Sort,|db| {
                    let mut stmt=db.prepare("SELECT l.id,r.id,l.entity_id,CASE WHEN l.payload_len=length(l.payload) AND length(l.payload)<=?5 THEN l.payload ELSE NULL END,CASE WHEN r.payload_len=length(r.payload) AND length(r.payload)<=?5 THEN r.payload ELSE NULL END,l.payload_sha256,r.payload_sha256 FROM knowledge_nodes l JOIN knowledge_nodes r ON l.entity_id=r.entity_id WHERE l.source_graph=?1 AND r.source_graph=?2 AND (l.id>?3 OR (l.id=?3 AND r.id>?4)) ORDER BY l.id,r.id LIMIT ?6")?;
                    let mut rows=stmt.query(params![claims,nav,after.0,after.1,limits.max_row_bytes as i64,limits.max_page_rows as i64])?;let mut out=Vec::new();
                    while let Some(row)=rows.next()? {out.push((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?,row.get::<_,Option<Vec<u8>>>(3)?.ok_or(Error::Budget("semantic pair bytes"))?,row.get::<_,Option<Vec<u8>>>(4)?.ok_or(Error::Budget("semantic pair bytes"))?,row.get::<_,Vec<u8>>(5)?,row.get::<_,Vec<u8>>(6)?));}Ok(out)
                })?;
                if pairs.is_empty() {
                    break;
                }
                for (left_id, right_id, entity, left, right, left_sha, right_sha) in pairs {
                    after = (left_id.clone(), right_id.clone());
                    charge(&mut work, left.len() + right.len(), limits)?;
                    if left_sha != Digest256::of_bytes(&left).as_bytes()
                        || right_sha != Digest256::of_bytes(&right).as_bytes()
                    {
                        return Err(Error::Invalid("semantic base pair digest"));
                    }
                    if !entity.starts_with(&prefix) {
                        continue;
                    }
                    let left = SourceRow::parse(&left, limits.max_row_bytes)?;
                    let right = SourceRow::parse(&right, limits.max_row_bytes)?;
                    let pair = shared_identity_pair(
                        &left, &right, &left_id, &right_id, &entity, claims, nav, entity_ref,
                        &prefix,
                    )?
                    .ok_or(Error::Invalid("semantic admitted pair prefix"))?;
                    insert(stage, pair, limits, &mut count, &mut work)?;
                }
            }
        }
        if let (Some(canon), Some(nav)) = (&canon, &navigation) {
            let mut after = None;
            loop {
                let rows =
                    stage.scan_input(canon, "nodes", after.as_deref(), limits.max_page_rows)?;
                for raw in rows.rows {
                    charge(&mut work, raw.payload.len(), limits)?;
                    let row = SourceRow::parse(&raw.payload, limits.max_row_bytes)?;
                    if let (Some(path), Some(id)) = (
                        text(row.value().get("source_path")),
                        text(row.value().get("node_id")),
                    ) {
                        stage.with_connection(WritePhase::Sort, |db| {
                            db.execute(
                                "INSERT INTO knowledge_semantic_canon_path VALUES (?1,?2)",
                                params![path, id],
                            )?;
                            Ok(())
                        })?;
                    }
                }
                match rows.next_id {
                    Some(id) => after = Some(id),
                    None => break,
                }
            }
            let mut after = None;
            loop {
                let rows =
                    stage.scan_input(nav, "nodes", after.as_deref(), limits.max_page_rows)?;
                for raw in rows.rows {
                    charge(&mut work, raw.payload.len(), limits)?;
                    let row = SourceRow::parse(&raw.payload, limits.max_row_bytes)?;
                    let item = row.value();
                    let Some(native) = text(item.get("node_id")) else {
                        continue;
                    };
                    let props = item.get("properties");
                    let mut refs = strings(props.and_then(|p| p.get("source_refs")));
                    refs.extend(strings(
                        props
                            .and_then(|p| p.get("source_record"))
                            .and_then(|r| r.get("source_refs")),
                    ));
                    for reference in refs {
                        let target = stage.with_connection(WritePhase::Sort, |db| {
                            db.query_row(
                                "SELECT node_id FROM knowledge_semantic_canon_path WHERE path=?1",
                                [&reference],
                                |r| r.get::<_, String>(0),
                            )
                            .optional()
                            .map_err(Error::from)
                        })?;
                        if let Some(target) = target {
                            let digest = Digest256::of_bytes(
                                format!("{native}\0{reference}\0{target}").as_bytes(),
                            )
                            .to_hex();
                            let mut source_refs =
                                row.source_refs(&[]).into_iter().collect::<BTreeSet<_>>();
                            source_refs.insert(reference.clone());
                            source_refs.insert(relation_ref.into());
                            insert(
                                stage,
                                json!({"edge_id":format!("grounded-in:{}",&digest[..24]),"from_id":native,"from_source_graph":nav,"to_id":target,"to_source_graph":canon,"predicate_id":"grounded_in","source_refs":source_refs,"graph_layers":["semantic-interchange"],"properties":{"derivation":"authored-source-record-source-ref","relation_label":"grounded in","review_status":"source-recorded","declared_source_ref":reference,"note":"Связь построена только из точной авторской source_refs-ссылки на canon node; она не утверждает same_as."}}),
                                limits,
                                &mut count,
                                &mut work,
                            )?;
                        }
                    }
                }
                match rows.next_id {
                    Some(id) => after = Some(id),
                    None => break,
                }
            }
        }
        let mut receipt = SemanticJoinReceipt {
            source_graph: source,
            source_cut: base.source_cut.clone(),
            descriptor_sha256: vocab.descriptor_sha256.clone(),
            base_nodes: base.node_count,
            base_node_root_sha256: base.node_root_sha256.clone(),
            relations: count,
            dependency_root_sha256: String::new(),
        };
        let (actual, hash) = root(stage, &receipt, limits)?;
        if actual != count {
            return Err(Error::Invalid("semantic prepared completeness"));
        }
        receipt.dependency_root_sha256 = hash;
        Ok(receipt)
    })();
    if result.is_err() {
        stage.poison();
    }
    result
}

fn verify(
    stage: &mut KnowledgeStage<'_>,
    receipt: &SemanticJoinReceipt,
    limits: TopologyLimits,
) -> Result<()> {
    limits.validate()?;
    if receipt.source_cut != stage.exact_receipt()?.binding.source_cut {
        return Err(Error::Invalid("semantic join cut"));
    }
    let (count, hash) = root(stage, receipt, limits)?;
    if count != receipt.relations || hash != receipt.dependency_root_sha256 {
        return Err(Error::Invalid("semantic join dependency root"));
    }
    Ok(())
}
pub fn materialize_semantic_relations<F>(
    stage: &mut KnowledgeStage<'_>,
    receipt: &SemanticJoinReceipt,
    normalizer: &KnowledgeBaseNormalizer<'_>,
    title_source_cut: &str,
    title_root_sha256: &str,
    limits: TopologyLimits,
    mut titles: F,
) -> Result<u64>
where
    F: FnMut(&mut KnowledgeStage<'_>, &str, &str) -> Result<(Value, Value)>,
{
    let result = (|| {
        verify(stage, receipt, limits)?;
        if title_source_cut != receipt.source_cut || Digest256::from_hex(title_root_sha256).is_err()
        {
            return Err(Error::Invalid("semantic title closure"));
        }
        let mut order: i64 = stage.with_connection(WritePhase::Sort, |db| {
            Ok(db.query_row(
                "SELECT coalesce(max(source_order)+1,0) FROM knowledge_relations",
                [],
                |r| r.get(0),
            )?)
        })?;
        let mut after = String::new();
        let mut count = 0;
        let mut work = 0;
        loop {
            let rows = page(stage, &after, limits)?;
            if rows.is_empty() {
                break;
            }
            stage.with_write_page(
                WritePhase::Normalized,
                limits.max_page_rows,
                (limits.max_page_rows * limits.max_row_bytes) as u64,
                |stage| {
                    for (id, raw) in rows {
                        after = id;
                        let source = SourceRow::parse(&raw, limits.max_row_bytes)?;
                        let item = source.value();
                        let from = format!(
                            "{}:{}",
                            required(item, "from_source_graph")?,
                            required(item, "from_id")?
                        );
                        let to = format!(
                            "{}:{}",
                            required(item, "to_source_graph")?,
                            required(item, "to_id")?
                        );
                        let (left, right) = titles(stage, &from, &to)?;
                        let value = normalizer.normalize_relation(
                            &source,
                            &receipt.source_graph,
                            None,
                            &left,
                            &right,
                            "derived-export",
                        )?;
                        let payload = bytes(&value, limits)?;
                        charge(&mut work, raw.len() + payload.len(), limits)?;
                        stage.insert_relation(RelationRow {
                            id: required(&value, "id")?,
                            source_graph: &receipt.source_graph,
                            native_id: Some(required(&value, "native_id")?),
                            from_id: required(&value, "from_id")?,
                            to_id: required(&value, "to_id")?,
                            predicate_id: required(&value, "predicate_id")?,
                            relation_type_id: required(&value, "relation_type_id")?,
                            source_order: order,
                            payload: &payload,
                        })?;
                        order += 1;
                        count += 1;
                    }
                    Ok(())
                },
            )?;
        }
        if count != receipt.relations {
            return Err(Error::Invalid("semantic final relations"));
        }
        Ok(count)
    })();
    if result.is_err() {
        stage.poison();
    }
    result
}
pub fn clear_semantic_joins(
    stage: &mut KnowledgeStage<'_>,
    receipt: &SemanticJoinReceipt,
    final_nodes: &CompleteBaseNodes,
    final_relation_count: u64,
    final_relation_root: &str,
    limits: TopologyLimits,
) -> Result<()> {
    let result = (|| {
        verify(stage, receipt, limits)?;
        let roots = stage.core_roots()?;
        if final_nodes.source_cut != receipt.source_cut
            || roots.nodes != final_nodes.node_count
            || roots.node_sha256 != final_nodes.node_root_sha256
            || roots.relations != final_relation_count
            || roots.relation_sha256 != final_relation_root
        {
            return Err(Error::Invalid("semantic final graph closure"));
        }
        let count: u64 = stage.with_connection(WritePhase::Finalize, |db| {
            Ok(db.query_row(
                "SELECT count(*) FROM knowledge_relations WHERE source_graph=?1",
                [&receipt.source_graph],
                |r| r.get(0),
            )?)
        })?;
        if count != receipt.relations {
            return Err(Error::Invalid("semantic final family coverage"));
        }
        stage.with_connection(WritePhase::Finalize, |db| {
            db.execute_batch(
                "DROP TABLE knowledge_semantic_material; DROP TABLE knowledge_semantic_canon_path;",
            )?;
            Ok(())
        })
    })();
    if result.is_err() {
        stage.poison();
    }
    result
}
