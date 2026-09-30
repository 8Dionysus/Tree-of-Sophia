//! Private, disk-backed inherited-view join over a complete normalized relation cut.
//!
//! Python `_prepare_inherited_views` receives every normalized relation after
//! relation assembly, and before final node revision stamping. This phase does
//! not create nodes, relations, or a selected output file.

use crate::knowledge_stage::{KnowledgeStage, WritePhase};
use crate::{Error, Result, knowledge_normalization::SourceRow};
use rusqlite::params;
use serde_json::Value;
use std::collections::BTreeSet;
use tos_foundation::{Digest256, Digest256Hasher};

#[derive(Clone, Debug)]
pub struct CompleteRelationSeal {
    /// The separately checked all-source producer receipt owns this value.
    pub source_cut: String,
    pub relation_count: u64,
    pub relation_root_sha256: String,
}

#[derive(Clone, Copy, Debug)]
pub struct InheritedViewLimits {
    pub max_relations: u64,
    pub max_endpoint_evidence_rows: u64,
    pub max_view_tokens: u64,
    pub max_page_rows: usize,
    pub max_page_bytes: u64,
    pub max_row_bytes: usize,
    pub max_work_bytes: u64,
}
impl InheritedViewLimits {
    pub(crate) fn validate(self) -> Result<()> {
        if self.max_relations == 0
            || self.max_endpoint_evidence_rows == 0
            || self.max_view_tokens == 0
            || self.max_page_rows == 0
            || self.max_page_rows > 1024
            || self.max_page_bytes == 0
            || self.max_page_bytes > 64 * 1024 * 1024
            || self.max_row_bytes == 0
            || self.max_row_bytes > 8 * 1024 * 1024
            || self.max_work_bytes == 0
            || self
                .max_page_rows
                .checked_mul(self.max_row_bytes)
                .is_none_or(|bytes| bytes as u64 > self.max_page_bytes)
        {
            return Err(Error::Budget("inherited view join limits"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct InheritedViewReceipt {
    pub source_cut: String,
    pub relation_count: u64,
    pub relation_root_sha256: String,
    /// Includes endpoint/relation pairs whose contribution has no views.
    pub endpoint_evidence_rows: u64,
    pub inherited_view_rows: u64,
    pub dependency_root_sha256: String,
    pub final_graph_rows_written: bool,
}

struct RelationInput {
    id: String,
    source_graph: String,
    from_id: String,
    to_id: String,
    order: i64,
    payload: Vec<u8>,
    payload_sha: [u8; 32],
}

fn read_page(
    stage: &mut KnowledgeStage<'_>,
    after: Option<(&str, &str)>,
    limits: InheritedViewLimits,
) -> Result<Vec<RelationInput>> {
    stage.with_connection(WritePhase::Sort, |db| {
        let mut statement = db.prepare(if after.is_some() {
            "SELECT id,source_graph,from_id,to_id,source_order,payload,payload_sha256
             FROM knowledge_relations WHERE
             (source_graph>?1 OR (source_graph=?1 AND id>?2))
             AND length(payload)<=?3 AND payload_len=length(payload)
             ORDER BY source_graph,id LIMIT ?4"
        } else {
            "SELECT id,source_graph,from_id,to_id,source_order,payload,payload_sha256
             FROM knowledge_relations WHERE length(payload)<=?1 AND payload_len=length(payload)
             ORDER BY source_graph,id LIMIT ?2"
        })?;
        let mut rows = match after {
            Some((source, id)) => statement.query(params![
                source,
                id,
                limits.max_row_bytes as i64,
                limits.max_page_rows as i64
            ])?,
            None => statement.query(params![
                limits.max_row_bytes as i64,
                limits.max_page_rows as i64
            ])?,
        };
        let mut result = Vec::new();
        let mut bytes = 0u64;
        while let Some(row) = rows.next()? {
            let payload: Vec<u8> = row.get(5)?;
            bytes = bytes
                .checked_add(payload.len() as u64)
                .ok_or(Error::Budget("inherited page bytes"))?;
            if bytes > limits.max_page_bytes {
                return Err(Error::Budget("inherited page bytes"));
            }
            let sha: Vec<u8> = row.get(6)?;
            if sha.len() != 32 {
                return Err(Error::Invalid("inherited relation digest size"));
            }
            let mut payload_sha = [0u8; 32];
            payload_sha.copy_from_slice(&sha);
            result.push(RelationInput {
                id: row.get(0)?,
                source_graph: row.get(1)?,
                from_id: row.get(2)?,
                to_id: row.get(3)?,
                order: row.get(4)?,
                payload,
                payload_sha,
            });
        }
        Ok(result)
    })
}

fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= 4096)
        .ok_or(Error::Invalid("inherited normalized relation field"))
}
fn root_text(hash: &mut Digest256Hasher, text: &str) {
    hash.update(&(text.len() as u64).to_be_bytes());
    hash.update(text.as_bytes());
}
fn root_item(hash: &mut Digest256Hasher, id: &str, sha: &[u8; 32]) {
    root_text(hash, id);
    hash.update(sha);
}

fn create_tables(stage: &mut KnowledgeStage<'_>) -> Result<()> {
    stage.with_connection(WritePhase::Schema, |db| {
        db.execute_batch(
            r#"
CREATE TABLE knowledge_global_inherited_endpoint_evidence(
 endpoint_id TEXT NOT NULL, relation_id TEXT NOT NULL,
 PRIMARY KEY(endpoint_id,relation_id));
CREATE INDEX knowledge_global_inherited_relation
 ON knowledge_global_inherited_endpoint_evidence(relation_id,endpoint_id);
CREATE TABLE knowledge_global_inherited_views(
 endpoint_id TEXT NOT NULL, view_id TEXT NOT NULL,
 PRIMARY KEY(endpoint_id,view_id));
"#,
        )?;
        Ok(())
    })
}

fn insert_relation(
    stage: &mut KnowledgeStage<'_>,
    relation: &RelationInput,
    views: &BTreeSet<String>,
) -> Result<()> {
    stage.with_connection(WritePhase::Normalized, |db| {
        for endpoint in [&relation.from_id, &relation.to_id]
            .into_iter()
            .collect::<BTreeSet<_>>()
        {
            db.execute(
                "INSERT INTO knowledge_global_inherited_endpoint_evidence VALUES (?1,?2)",
                params![endpoint, relation.id],
            )?;
            for view in views {
                db.execute(
                    "INSERT OR IGNORE INTO knowledge_global_inherited_views VALUES (?1,?2)",
                    params![endpoint, view],
                )?;
            }
        }
        Ok(())
    })
}

fn index_root(
    stage: &mut KnowledgeStage<'_>,
    seal: &CompleteRelationSeal,
) -> Result<(u64, u64, String)> {
    stage.with_connection(WritePhase::Sort, |db| {
        let mut hash = Digest256Hasher::new();
        hash.update(b"tos-global-inherited-views-v1\0");
        root_text(&mut hash,&seal.source_cut);
        root_text(&mut hash,&seal.relation_root_sha256);
        root_text(&mut hash,"endpoint_evidence");
        let mut evidence_count = 0u64;
        let mut statement = db.prepare("SELECT endpoint_id,relation_id FROM knowledge_global_inherited_endpoint_evidence ORDER BY endpoint_id,relation_id")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let endpoint: String = row.get(0)?; let relation: String = row.get(1)?;
            root_text(&mut hash,&endpoint); root_text(&mut hash,&relation);
            evidence_count = evidence_count.checked_add(1).ok_or(Error::Budget("inherited evidence rows"))?;
        }
        root_text(&mut hash,"views");
        let mut view_count = 0u64;
        let mut statement = db.prepare("SELECT endpoint_id,view_id FROM knowledge_global_inherited_views ORDER BY endpoint_id,view_id")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let endpoint: String = row.get(0)?; let view: String = row.get(1)?;
            root_text(&mut hash,&endpoint); root_text(&mut hash,&view);
            view_count = view_count.checked_add(1).ok_or(Error::Budget("inherited view rows"))?;
        }
        Ok((evidence_count,view_count,hash.finalize().to_hex()))
    })
}

fn prepare_inner(
    stage: &mut KnowledgeStage<'_>,
    seal: &CompleteRelationSeal,
    limits: InheritedViewLimits,
) -> Result<InheritedViewReceipt> {
    limits.validate()?;
    Digest256::from_hex(&seal.relation_root_sha256)
        .map_err(|_| Error::Invalid("inherited relation seal root"))?;
    if seal.source_cut != stage.exact_receipt().binding.source_cut
        || seal.relation_count > limits.max_relations
    {
        return Err(Error::Invalid("inherited complete relation seal"));
    }
    create_tables(stage)?;
    let mut after: Option<(String, String)> = None;
    let mut count = 0u64;
    let mut work = 0u64;
    let mut view_tokens = 0u64;
    let mut endpoint_evidence_limit_count = 0u64;
    let mut hash = Digest256Hasher::new();
    loop {
        let page = read_page(
            stage,
            after.as_ref().map(|(s, i)| (s.as_str(), i.as_str())),
            limits,
        )?;
        if page.is_empty() {
            break;
        }
        let page_len = page.len();
        stage.with_write_page(
            WritePhase::Normalized,
            page_len,
            limits.max_page_bytes,
            |stage| {
                for relation in &page {
                    count = count
                        .checked_add(1)
                        .ok_or(Error::Budget("inherited relation count"))?;
                    if count > limits.max_relations {
                        return Err(Error::Budget("inherited relation count"));
                    }
                    if relation.order < 0 || relation.order as u64 != count - 1 {
                        return Err(Error::Invalid("inherited relation source order"));
                    }
                    work = work
                        .checked_add(relation.payload.len() as u64)
                        .ok_or(Error::Budget("inherited work bytes"))?;
                    if work > limits.max_work_bytes {
                        return Err(Error::Budget("inherited work bytes"));
                    }
                    if Digest256::of_bytes(&relation.payload).as_bytes() != &relation.payload_sha {
                        return Err(Error::Invalid("inherited relation payload digest"));
                    }
                    root_item(&mut hash, &relation.id, &relation.payload_sha);
                    let parsed = SourceRow::parse(&relation.payload, limits.max_row_bytes)?;
                    let value = parsed.value();
                    for (field, expected) in [
                        ("id", relation.id.as_str()),
                        ("source_graph", relation.source_graph.as_str()),
                        ("from_id", relation.from_id.as_str()),
                        ("to_id", relation.to_id.as_str()),
                    ] {
                        if text(value, field)? != expected {
                            return Err(Error::Invalid(
                                "inherited relation column/payload mismatch",
                            ));
                        }
                    }
                    let view_values = value
                        .get("view_ids")
                        .and_then(Value::as_array)
                        .ok_or(Error::Invalid("inherited normalized relation view list"))?;
                    view_tokens = view_tokens
                        .checked_add(view_values.len() as u64)
                        .ok_or(Error::Budget("inherited view tokens"))?;
                    if view_tokens > limits.max_view_tokens {
                        return Err(Error::Budget("inherited view tokens"));
                    }
                    let mut views = BTreeSet::new();
                    for view in view_values {
                        let text = view
                            .as_str()
                            .filter(|v| !v.is_empty() && v.len() <= 4096)
                            .ok_or(Error::Invalid("inherited normalized relation view member"))?;
                        views.insert(text.to_owned());
                    }
                    endpoint_evidence_limit_count = endpoint_evidence_limit_count
                        .checked_add(if relation.from_id == relation.to_id {
                            1
                        } else {
                            2
                        })
                        .ok_or(Error::Budget("inherited endpoint evidence"))?;
                    if endpoint_evidence_limit_count > limits.max_endpoint_evidence_rows {
                        return Err(Error::Budget("inherited endpoint evidence"));
                    }
                    insert_relation(stage, &relation, &views)?;
                    after = Some((relation.source_graph.clone(), relation.id.clone()));
                }
                Ok(())
            },
        )?;
        if page_len < limits.max_page_rows {
            break;
        }
    }
    if count != seal.relation_count || hash.finalize().to_hex() != seal.relation_root_sha256 {
        return Err(Error::Invalid(
            "inherited complete relation count/root mismatch",
        ));
    }
    let (evidence, views, root) = index_root(stage, seal)?;
    if evidence != endpoint_evidence_limit_count
        || views
            > limits
                .max_view_tokens
                .checked_mul(2)
                .ok_or(Error::Budget("inherited indexed view rows"))?
    {
        return Err(Error::Invalid("inherited indexed join counts"));
    }
    Ok(InheritedViewReceipt {
        source_cut: seal.source_cut.clone(),
        relation_count: count,
        relation_root_sha256: seal.relation_root_sha256.clone(),
        endpoint_evidence_rows: evidence,
        inherited_view_rows: views,
        dependency_root_sha256: root,
        final_graph_rows_written: false,
    })
}

/// Build an endpoint-view index only after an independently sealed all-source
/// relation producer supplies the exact complete count and row root.
pub fn prepare_global_inherited_views(
    stage: &mut KnowledgeStage<'_>,
    seal: &CompleteRelationSeal,
    limits: InheritedViewLimits,
) -> Result<InheritedViewReceipt> {
    let result = prepare_inner(stage, seal, limits);
    if result.is_err() {
        stage.poison();
    }
    result
}

/// Exact, ordered inherited views for one normalized stable endpoint ID.
/// The caller must keep this index private and invoke `clear_inherited_views`
/// after final node assembly, before the stage can be selected.
pub fn endpoint_inherited_views(
    stage: &mut KnowledgeStage<'_>,
    endpoint_id: &str,
    max_views: usize,
) -> Result<Vec<String>> {
    if endpoint_id.is_empty() || endpoint_id.len() > 4096 || max_views == 0 || max_views > 4096 {
        return Err(Error::Budget("inherited endpoint lookup"));
    }
    stage.with_connection(WritePhase::Sort, |db| {
        let mut statement = db.prepare("SELECT view_id FROM knowledge_global_inherited_views WHERE endpoint_id=?1 ORDER BY view_id LIMIT ?2")?;
        let mut rows = statement.query(params![endpoint_id,(max_views as i64)+1])?;
        let mut result = Vec::new();
        while let Some(row)=rows.next()? {
            if result.len()==max_views { return Err(Error::Budget("inherited endpoint view count")); }
            result.push(row.get(0)?);
        }
        Ok(result)
    })
}

/// Explicitly erase private join tables after all final node reads. The
/// compiler owner's sanitized-output gate must also reject any remaining
/// preparatory tables or `raw_records` before selected publication.
pub fn clear_inherited_views(stage: &mut KnowledgeStage<'_>) -> Result<()> {
    stage.with_connection(WritePhase::Finalize, |db| {
        db.execute_batch("DROP TABLE knowledge_global_inherited_views; DROP TABLE knowledge_global_inherited_endpoint_evidence;")?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge_stage::{
        ExactInputReceipt, InputCollectionReceipt, RelationRow, StageIsolation, StageLimits,
        StageOwner,
    };
    use crate::{Limits, SourceBinding};
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    const ROOT: &str = "79c80123314225ea91f982eaa81c937990779fa21dc4da3c833d84270ee465d4";
    const INDEX_ROOT: &str = "9c0f657c58192bc6ad6814f1338c640c9c1dc1efb2450c8cae412e9f73174036";
    type FixtureRow = (
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        &'static str,
    );
    const ROWS: &[FixtureRow] = &[
        (
            "canon:e1",
            "canon",
            "canon:a",
            "philosophy:n:a",
            r#"{"id":"canon:e1","source_graph":"canon","from_id":"canon:a","to_id":"philosophy:n:a","view_ids":["route","atlas","atlas"]}"#,
        ),
        (
            "philosophy:e2",
            "philosophy",
            "philosophy:n:a",
            "philosophy:n:a",
            r#"{"id":"philosophy:e2","source_graph":"philosophy","from_id":"philosophy:n:a","to_id":"philosophy:n:a","view_ids":[]}"#,
        ),
        (
            "philosophy:e3",
            "philosophy",
            "philosophy:n:a",
            "philosophy:n:b",
            r#"{"id":"philosophy:e3","source_graph":"philosophy","from_id":"philosophy:n:a","to_id":"philosophy:n:b","view_ids":["route","public"]}"#,
        ),
    ];
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
    fn receipt() -> ExactInputReceipt {
        ExactInputReceipt {
            binding: SourceBinding {
                owner_profile: "fixture-owner".into(),
                source_cut: "sealed-all-relations".into(),
                through_commit_seq: 1,
                membership_root: "0".repeat(64),
                index_generation: "gen-1".into(),
                route_map_version: "routes-1".into(),
                reader_abi: "reader-1".into(),
                projection_root_sha256: "1".repeat(64),
                complete: true,
            },
            collections: ["canon", "philosophy"]
                .iter()
                .map(|source| InputCollectionReceipt {
                    source_graph: (*source).into(),
                    collection: "fixture".into(),
                    input_role: "test".into(),
                    adapter_profile: "test".into(),
                    expected_count: 0,
                    expected_root_sha256: Digest256::of_bytes(b"").to_hex(),
                })
                .collect(),
        }
    }
    fn limits() -> InheritedViewLimits {
        InheritedViewLimits {
            max_relations: 8,
            max_endpoint_evidence_rows: 16,
            max_view_tokens: 16,
            max_page_rows: 2,
            max_page_bytes: 4096,
            max_row_bytes: 2048,
            max_work_bytes: 16 * 1024,
        }
    }
    fn path() -> PathBuf {
        let tick = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("tos-global-views-{}-{tick}", std::process::id()));
        fs::create_dir(&dir).unwrap();
        dir.join("private.sqlite3")
    }
    fn run(
        rows: &[FixtureRow],
        seal: CompleteRelationSeal,
        limits: InheritedViewLimits,
    ) -> Result<(InheritedViewReceipt, Vec<String>)> {
        let owner = Owner;
        let quota = Quota;
        let candidate = path();
        let mut stage = KnowledgeStage::create(
            &candidate,
            StageLimits {
                sqlite: Limits::default(),
                max_temp_bytes: 64 * 1024 * 1024,
                max_seek_rows: 2,
                max_seek_bytes: 4096,
            },
            receipt(),
            &owner,
            &quota,
        )?;
        let result = (|| {
            for (order, (id, source, from, to, payload)) in rows.iter().enumerate() {
                stage.insert_relation(RelationRow {
                    id,
                    source_graph: source,
                    native_id: None,
                    from_id: from,
                    to_id: to,
                    predicate_id: "related_to",
                    relation_type_id: "tos.relation.unmapped",
                    source_order: order as i64,
                    payload: payload.as_bytes(),
                })?;
            }
            let before = stage.core_roots()?;
            let joined = prepare_global_inherited_views(&mut stage, &seal, limits)?;
            let views = endpoint_inherited_views(&mut stage, "philosophy:n:a", 16)?;
            if rows.len() == 3 {
                let empty_relation_evidence: i64 = stage.with_connection(WritePhase::Sort, |db| {
                    Ok(db.query_row("SELECT count(*) FROM knowledge_global_inherited_endpoint_evidence WHERE endpoint_id='philosophy:n:a' AND relation_id='philosophy:e2'",[],|r|r.get(0))?)
                })?;
                assert_eq!(empty_relation_evidence, 1);
            }
            let after = stage.core_roots()?;
            assert_eq!(
                (before.nodes, before.relations, before.relation_sha256),
                (after.nodes, after.relations, after.relation_sha256)
            );
            clear_inherited_views(&mut stage)?;
            Ok((joined, views))
        })();
        drop(stage);
        fs::remove_dir(candidate.parent().unwrap()).unwrap();
        result
    }
    fn seal() -> CompleteRelationSeal {
        CompleteRelationSeal {
            source_cut: "sealed-all-relations".into(),
            relation_count: 3,
            relation_root_sha256: ROOT.into(),
        }
    }
    // CPython hashlib independently hashes the exact fixture bytes; Python
    // `_prepare_inherited_views` independently yields the expected view sets.
    #[test]
    fn python_oracle_all_source_views_and_empty_self_loop() {
        let (joined, views) = run(ROWS, seal(), limits()).unwrap();
        assert_eq!(joined.relation_root_sha256, ROOT);
        assert_eq!(
            (joined.endpoint_evidence_rows, joined.inherited_view_rows),
            (5, 7)
        );
        assert_eq!(joined.dependency_root_sha256, INDEX_ROOT);
        assert_eq!(
            views,
            vec!["atlas".to_owned(), "public".to_owned(), "route".to_owned()]
        );
        assert!(!joined.final_graph_rows_written);
    }
    #[test]
    fn incomplete_malformed_and_budgeted_relation_cuts_refuse() {
        let mut incomplete = ROWS.to_vec();
        incomplete.pop();
        assert!(run(&incomplete, seal(), limits()).is_err());
        let mut malformed = ROWS.to_vec();
        malformed[0].4 = r#"{"id":"canon:e1","source_graph":"canon","from_id":"wrong","to_id":"philosophy:n:a","view_ids":["route"]}"#;
        assert!(
            run(
                &malformed,
                CompleteRelationSeal {
                    relation_root_sha256: raw_root(&malformed),
                    ..seal()
                },
                limits()
            )
            .is_err()
        );
        let mut small = limits();
        small.max_view_tokens = 3;
        assert!(run(ROWS, seal(), small).is_err());
        let mut small_page = limits();
        small_page.max_page_bytes = 4095;
        assert!(run(ROWS, seal(), small_page).is_err());
        let mut bad_seal = seal();
        bad_seal.source_cut = "different".into();
        assert!(run(ROWS, bad_seal, limits()).is_err());
        let mut out_of_order = ROWS.to_vec();
        out_of_order.swap(0, 1);
        assert!(run(&out_of_order, seal(), limits()).is_err());
        let mut duplicate = ROWS.to_vec();
        duplicate.push(ROWS[0]);
        assert!(run(&duplicate, seal(), limits()).is_err());
        let mut invalid_views = ROWS.to_vec();
        invalid_views[0].4 = r#"{"id":"canon:e1","source_graph":"canon","from_id":"canon:a","to_id":"philosophy:n:a","view_ids":[1]}"#;
        assert!(
            run(
                &invalid_views,
                CompleteRelationSeal {
                    relation_root_sha256: raw_root(&invalid_views),
                    ..seal()
                },
                limits()
            )
            .is_err()
        );
    }
    fn raw_root(rows: &[FixtureRow]) -> String {
        let mut hash = Digest256Hasher::new();
        for (id, _, _, _, payload) in rows {
            root_item(
                &mut hash,
                id,
                Digest256::of_bytes(payload.as_bytes()).as_bytes(),
            );
        }
        hash.finalize().to_hex()
    }
}
