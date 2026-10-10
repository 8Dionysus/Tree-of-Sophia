//! Disk-backed deterministic ordering of already normalized carriers. The
//! specialized source adapters still own their semantics and global joins.

use crate::{
    Error, Result,
    knowledge_stage::{KnowledgeStage, WritePhase},
};
use rusqlite::params;
use serde_json::Value;
use tos_foundation::{Digest256, JsonLimits, JsonMode, parse_json};

#[derive(Clone, Copy, Debug)]
pub struct OrderedCandidateLimits {
    pub max_rows: u64,
    pub max_work_bytes: u64,
    pub max_row_bytes: usize,
}
impl OrderedCandidateLimits {
    fn validate(self) -> Result<()> {
        if self.max_rows == 0
            || self.max_work_bytes == 0
            || self.max_row_bytes == 0
            || self.max_row_bytes > 8 * 1024 * 1024
        {
            return Err(Error::Budget("ordered candidate limits"));
        }
        Ok(())
    }
}

pub struct NormalizedNodeCandidate<'a> {
    pub id: &'a str,
    pub source_graph: &'a str,
    pub native_id: Option<&'a str>,
    pub entity_id: Option<&'a str>,
    pub kind_id: &'a str,
    pub type_id: &'a str,
    pub payload: &'a [u8],
}
pub struct NormalizedRelationCandidate<'a> {
    pub id: &'a str,
    pub source_graph: &'a str,
    pub native_id: Option<&'a str>,
    pub from_id: &'a str,
    pub to_id: &'a str,
    pub predicate_id: &'a str,
    pub relation_type_id: &'a str,
    pub payload: &'a [u8],
}
#[derive(Clone, Debug)]
pub struct OrderedCandidateReceipt {
    pub node_count: u64,
    pub relation_count: u64,
    pub intermediate_work_bytes: u64,
}

pub struct OrderedKnowledgeSink<'s, 'o> {
    stage: &'s mut KnowledgeStage<'o>,
    limits: OrderedCandidateLimits,
    node_count: u64,
    relation_count: u64,
    work_bytes: u64,
    finished: bool,
}

fn required<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty() && text.len() <= 4096)
        .ok_or(Error::Invalid("ordered candidate payload field"))
}
fn valid_id(value: &str) -> Result<()> {
    if value.is_empty() || value.len() > 4096 || value.as_bytes().contains(&0) {
        return Err(Error::Invalid("ordered candidate ID"));
    }
    Ok(())
}
fn verify_payload(raw: &[u8], cap: usize, fields: &[(&str, &str)]) -> Result<()> {
    if raw.is_empty() || raw.len() > cap {
        return Err(Error::Budget("ordered candidate payload bytes"));
    }
    let limits = JsonLimits::new(cap, 96, 1_000_000, 4096)
        .map_err(|_| Error::Budget("ordered candidate JSON limits"))?;
    parse_json(raw, JsonMode::PublishedStrict, limits).map_err(|e| Error::Source(e.to_string()))?;
    let value: Value =
        serde_json::from_slice(raw).map_err(|_| Error::Invalid("ordered candidate JSON"))?;
    for (field, expected) in fields {
        if required(&value, field)? != *expected {
            return Err(Error::Invalid("ordered candidate column/payload mismatch"));
        }
    }
    Ok(())
}

impl<'s, 'o> OrderedKnowledgeSink<'s, 'o> {
    /// The host-isolated stage owns all temp/output quota and SQLite VM caps.
    /// This creates private candidate tables with global unique IDs and a
    /// `(source_graph,id)` index used by the final external order pass.
    pub fn begin(
        stage: &'s mut KnowledgeStage<'o>,
        limits: OrderedCandidateLimits,
    ) -> Result<Self> {
        let setup: Result<()> = (|| {
            limits.validate()?;
            stage.create_preparation_tables(PREPARATION_SCHEMA)
        })();
        if setup.is_err() {
            stage.poison();
        }
        setup?;
        Ok(Self {
            stage,
            limits,
            node_count: 0,
            relation_count: 0,
            work_bytes: 0,
            finished: false,
        })
    }
    fn preflight(&mut self, source_graph: &str, id: &str, payload: &[u8]) -> Result<()> {
        valid_id(source_graph)?;
        valid_id(id)?;
        if !self.stage.registered_source(source_graph) {
            return Err(Error::Invalid("ordered candidate unregistered source"));
        }
        let next_rows = self
            .node_count
            .checked_add(self.relation_count)
            .and_then(|n| n.checked_add(1))
            .ok_or(Error::Budget("ordered candidate rows"))?;
        let next_work = self
            .work_bytes
            .checked_add(payload.len() as u64)
            .ok_or(Error::Budget("ordered candidate work bytes"))?;
        if next_rows > self.limits.max_rows || next_work > self.limits.max_work_bytes {
            return Err(Error::Budget("ordered candidate rows/work bytes"));
        }
        self.stage.charge(payload)?;
        self.work_bytes = next_work;
        Ok(())
    }
    pub fn push_node(&mut self, row: NormalizedNodeCandidate<'_>) -> Result<()> {
        let result = self.push_node_inner(row);
        if result.is_err() {
            self.stage.poison();
        }
        result
    }
    fn push_node_inner(&mut self, row: NormalizedNodeCandidate<'_>) -> Result<()> {
        for field in [row.kind_id, row.type_id]
            .into_iter()
            .chain(row.native_id)
            .chain(row.entity_id)
        {
            valid_id(field)?;
        }
        verify_payload(
            row.payload,
            self.limits.max_row_bytes,
            &[
                ("id", row.id),
                ("source_graph", row.source_graph),
                ("kind_id", row.kind_id),
                ("type_id", row.type_id),
            ],
        )?;
        self.preflight(row.source_graph, row.id, row.payload)?;
        let digest = Digest256::of_bytes(row.payload);
        self.stage.with_connection(WritePhase::Normalized, |db| {
            db.execute(
                "INSERT INTO knowledge_node_candidates VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                params![
                    row.id,
                    row.source_graph,
                    row.native_id,
                    row.entity_id,
                    row.kind_id,
                    row.type_id,
                    i64::try_from(row.payload.len())
                        .map_err(|_| Error::Budget("ordered candidate row bytes"))?,
                    &digest.as_bytes()[..],
                    row.payload
                ],
            )?;
            Ok(())
        })?;
        self.node_count += 1;
        Ok(())
    }
    pub fn push_relation(&mut self, row: NormalizedRelationCandidate<'_>) -> Result<()> {
        let result = self.push_relation_inner(row);
        if result.is_err() {
            self.stage.poison();
        }
        result
    }
    fn push_relation_inner(&mut self, row: NormalizedRelationCandidate<'_>) -> Result<()> {
        for field in [
            row.from_id,
            row.to_id,
            row.predicate_id,
            row.relation_type_id,
        ]
        .into_iter()
        .chain(row.native_id)
        {
            valid_id(field)?;
        }
        verify_payload(
            row.payload,
            self.limits.max_row_bytes,
            &[
                ("id", row.id),
                ("source_graph", row.source_graph),
                ("from_id", row.from_id),
                ("to_id", row.to_id),
                ("predicate_id", row.predicate_id),
                ("relation_type_id", row.relation_type_id),
            ],
        )?;
        self.preflight(row.source_graph, row.id, row.payload)?;
        let digest = Digest256::of_bytes(row.payload);
        self.stage.with_connection(WritePhase::Normalized, |db| {
            db.execute(
                "INSERT INTO knowledge_relation_candidates VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
                params![
                    row.id,
                    row.source_graph,
                    row.native_id,
                    row.from_id,
                    row.to_id,
                    row.predicate_id,
                    row.relation_type_id,
                    i64::try_from(row.payload.len())
                        .map_err(|_| Error::Budget("ordered candidate row bytes"))?,
                    &digest.as_bytes()[..],
                    row.payload
                ],
            )?;
            Ok(())
        })?;
        self.relation_count += 1;
        Ok(())
    }
    pub fn finish(mut self) -> Result<OrderedCandidateReceipt> {
        let result = self.finish_inner();
        if result.is_err() {
            self.stage.poison();
        }
        result
    }
    fn finish_inner(&mut self) -> Result<OrderedCandidateReceipt> {
        let counts = self.stage.with_connection(WritePhase::Sort, |db| {
            let mut totals = Vec::with_capacity(2);
            for table in ["knowledge_node_candidates", "knowledge_relation_candidates"] {
                let sql = format!("SELECT count(*),coalesce(sum(payload_len),0) FROM {table}");
                let pair: (i64, i64) = db.query_row(&sql, [], |r| Ok((r.get(0)?, r.get(1)?)))?;
                totals.push(pair);
            }
            Ok(totals)
        })?;
        let (nodes, node_bytes) = counts[0];
        let (relations, relation_bytes) = counts[1];
        if nodes < 0
            || relations < 0
            || node_bytes < 0
            || relation_bytes < 0
            || nodes as u64 != self.node_count
            || relations as u64 != self.relation_count
        {
            return Err(Error::Invalid("ordered candidate count mismatch"));
        }
        self.stage.charge_materialized(
            self.node_count
                .checked_add(self.relation_count)
                .ok_or(Error::Budget("ordered rows"))?,
            (node_bytes as u64)
                .checked_add(relation_bytes as u64)
                .ok_or(Error::Budget("ordered work bytes"))?,
        )?;
        self.stage.with_connection(WritePhase::Sort, |db| {
            db.execute_batch("SAVEPOINT cmp_ordered_final")?;
            let result: Result<()> = (|| {
                let n = db.execute(NODE_FINAL, [])?;
                let r = db.execute(RELATION_FINAL, [])?;
                if n as u64 != self.node_count || r as u64 != self.relation_count {
                    return Err(Error::Invalid("ordered final copy counts"));
                }
                db.execute_batch(
                    "DROP TABLE knowledge_node_candidates; DROP TABLE knowledge_relation_candidates",
                )?;
                Ok(())
            })();
            if result.is_ok() {
                db.execute_batch("RELEASE cmp_ordered_final")?;
            } else {
                db.execute_batch("ROLLBACK TO cmp_ordered_final; RELEASE cmp_ordered_final")?;
            }
            result
        })?;
        self.finished = true;
        Ok(OrderedCandidateReceipt {
            node_count: self.node_count,
            relation_count: self.relation_count,
            intermediate_work_bytes: self.work_bytes,
        })
    }
}

impl Drop for OrderedKnowledgeSink<'_, '_> {
    fn drop(&mut self) {
        if !self.finished {
            self.stage.poison();
        }
    }
}

pub(crate) const PREPARATION_SCHEMA: crate::knowledge_stage::PreparationSchema = crate::knowledge_stage::preparation_schema!(
    table r#"knowledge_node_candidates(
 id TEXT PRIMARY KEY,source_graph TEXT NOT NULL,native_id TEXT,entity_id TEXT,
 kind_id TEXT NOT NULL,type_id TEXT NOT NULL,payload_len INTEGER NOT NULL,
 payload_sha256 BLOB NOT NULL,payload BLOB NOT NULL) WITHOUT ROWID"#,
    index r#"knowledge_node_candidates_order ON knowledge_node_candidates(source_graph,id)"#,
    table r#"knowledge_relation_candidates(
 id TEXT PRIMARY KEY,source_graph TEXT NOT NULL,native_id TEXT,
 from_id TEXT NOT NULL,to_id TEXT NOT NULL,predicate_id TEXT NOT NULL,
 relation_type_id TEXT NOT NULL,payload_len INTEGER NOT NULL,
 payload_sha256 BLOB NOT NULL,payload BLOB NOT NULL) WITHOUT ROWID"#,
    index r#"knowledge_relation_candidates_order ON knowledge_relation_candidates(source_graph,id)"#,
);

const NODE_FINAL: &str = "INSERT INTO knowledge_nodes
 SELECT id,source_graph,native_id,entity_id,kind_id,type_id,
 ROW_NUMBER() OVER (ORDER BY source_graph,id)-1,payload_len,payload_sha256,payload
 FROM knowledge_node_candidates ORDER BY source_graph,id";
const RELATION_FINAL: &str = "INSERT INTO knowledge_relations
 SELECT id,source_graph,native_id,from_id,to_id,predicate_id,relation_type_id,
 ROW_NUMBER() OVER (ORDER BY source_graph,id)-1,payload_len,payload_sha256,payload
 FROM knowledge_relation_candidates ORDER BY source_graph,id";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Limits, SourceBinding,
        knowledge_stage::{
            ExactInputReceipt, InputCollectionReceipt, StageIsolation, StageLimits, StageOwner,
        },
    };
    use std::{
        fs,
        path::Path,
        time::{SystemTime, UNIX_EPOCH},
    };

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
    fn path() -> std::path::PathBuf {
        let tick = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("tos-ordered-{}-{tick}", std::process::id()));
        fs::create_dir(&dir).unwrap();
        dir.join("candidate.sqlite3")
    }
    #[test]
    fn disk_sort_sets_dense_order_and_duplicate_failure_poisons_stage() {
        let owner = Owner;
        let quota = Quota;
        let receipt = ExactInputReceipt {
            binding: SourceBinding {
                owner_profile: "fixture".into(),
                source_cut: "cut".into(),
                through_commit_seq: 1,
                membership_root: "0".repeat(64),
                index_generation: "g".into(),
                route_map_version: "r".into(),
                reader_abi: "a".into(),
                projection_root_sha256: "1".repeat(64),
                complete: true,
            },
            collections: vec![InputCollectionReceipt {
                source_graph: "graph".into(),
                collection: "empty".into(),
                input_role: "fixture".into(),
                adapter_profile: "fixture".into(),
                expected_count: 0,
                expected_root_sha256: Digest256::of_bytes(b"").to_hex(),
            }],
        };
        let candidate = path();
        let mut stage = KnowledgeStage::create(
            &candidate,
            StageLimits {
                sqlite: Limits::default(),
                max_temp_bytes: 64 * 1024 * 1024,
                max_seek_rows: 8,
                max_seek_bytes: 1024,
            },
            receipt.clone(),
            &owner,
            &quota,
        )
        .unwrap();
        let mut sink = OrderedKnowledgeSink::begin(
            &mut stage,
            OrderedCandidateLimits {
                max_rows: 3,
                max_work_bytes: 4096,
                max_row_bytes: 1024,
            },
        )
        .unwrap();
        for id in ["b", "a"] {
            let raw = serde_json::to_vec(&serde_json::json!({
                "id":id,"source_graph":"graph","kind_id":"kind","type_id":"type"
            }))
            .unwrap();
            sink.push_node(NormalizedNodeCandidate {
                id,
                source_graph: "graph",
                native_id: None,
                entity_id: None,
                kind_id: "kind",
                type_id: "type",
                payload: &raw,
            })
            .unwrap();
        }
        let relation = br#"{"id":"r","source_graph":"graph","from_id":"a","to_id":"b","predicate_id":"p","relation_type_id":"t"}"#;
        sink.push_relation(NormalizedRelationCandidate {
            id: "r",
            source_graph: "graph",
            native_id: None,
            from_id: "a",
            to_id: "b",
            predicate_id: "p",
            relation_type_id: "t",
            payload: relation,
        })
        .unwrap();
        let copied = sink.finish().unwrap();
        assert_eq!((copied.node_count, copied.relation_count), (2, 1));
        stage.finish().unwrap();
        let db = rusqlite::Connection::open(&candidate).unwrap();
        let ordered: Vec<String> = db
            .prepare("SELECT id FROM knowledge_nodes ORDER BY source_order")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(|row| row.unwrap())
            .collect();
        assert_eq!(ordered, vec!["a".to_owned(), "b".to_owned()]);
        drop(db);
        fs::remove_dir_all(candidate.parent().unwrap()).unwrap();

        let duplicate = path();
        let mut stage = KnowledgeStage::create(
            &duplicate,
            StageLimits {
                sqlite: Limits::default(),
                max_temp_bytes: 64 * 1024 * 1024,
                max_seek_rows: 8,
                max_seek_bytes: 1024,
            },
            receipt,
            &owner,
            &quota,
        )
        .unwrap();
        let mut sink = OrderedKnowledgeSink::begin(
            &mut stage,
            OrderedCandidateLimits {
                max_rows: 2,
                max_work_bytes: 4096,
                max_row_bytes: 1024,
            },
        )
        .unwrap();
        let raw = br#"{"id":"a","source_graph":"graph","kind_id":"kind","type_id":"type"}"#;
        for attempt in 0..2 {
            let result = sink.push_node(NormalizedNodeCandidate {
                id: "a",
                source_graph: "graph",
                native_id: None,
                entity_id: None,
                kind_id: "kind",
                type_id: "type",
                payload: raw,
            });
            if attempt == 0 {
                result.unwrap();
            } else {
                assert!(result.is_err());
            }
        }
        drop(sink);
        assert!(stage.finish().is_err());
        assert!(!duplicate.exists());
        fs::remove_dir_all(duplicate.parent().unwrap()).unwrap();
    }
}
