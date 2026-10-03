//! Existing agent_context_* complete groups and singleton Claim finalizer.
//! Positions preserve reducer order; they grant no global authored ordering.
use super::{
    source_claim_publication_bytes as bytes, source_claim_publication_dependencies as deps,
};
use rusqlite::{OptionalExtension, Transaction, params};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use tos_compiler::{Error, Result};

const MAX_ORDER: u64 = 9_007_199_254_740_991;
const DDL: [&str; 4] = [
    "CREATE TABLE agent_context_state(singleton INTEGER PRIMARY KEY CHECK(singleton=1),json TEXT NOT NULL,sha256 TEXT NOT NULL)",
    "CREATE TABLE agent_context_nodes(id TEXT PRIMARY KEY,position INTEGER NOT NULL UNIQUE,keys_json TEXT NOT NULL,seal TEXT NOT NULL) WITHOUT ROWID",
    "CREATE TABLE agent_context_refs(source_graph TEXT NOT NULL,claim_ref TEXT NOT NULL,id TEXT NOT NULL,position INTEGER NOT NULL,PRIMARY KEY(source_graph,claim_ref,id)) WITHOUT ROWID",
    "CREATE TABLE agent_context_heads(source_graph TEXT NOT NULL,claim_ref TEXT NOT NULL,n INTEGER NOT NULL,xor_sha256 TEXT NOT NULL,sum_sha256 TEXT NOT NULL,seal TEXT NOT NULL,PRIMARY KEY(source_graph,claim_ref)) WITHOUT ROWID",
];
pub(super) fn keys(node: &Value) -> Result<Vec<(String, String)>> {
    if !["claim", "annotation-claim"].contains(&node["kind_id"].as_str().unwrap_or("")) {
        return Ok(Vec::new());
    }
    let graph = bytes::text(node, "source_graph")?;
    let contexts = node["semantics"]["assertion_contexts"]
        .as_array()
        .ok_or(Error::Invalid("Claim context values"))?;
    let mut keys = BTreeSet::new();
    for context in contexts {
        let fields = &context["fields"];
        if let Some(reference) = fields
            .get("claim_id")
            .or_else(|| fields.get("claim_ref"))
            .and_then(|v| v["value"].as_str())
        {
            if reference.is_empty() || reference.len() > 4096 {
                return Err(Error::Invalid("Claim context identifier"));
            }
            keys.insert((graph.to_owned(), reference.to_owned()));
        }
    }
    Ok(keys.into_iter().collect())
}
pub(super) struct Context<'a, 'tx> {
    tx: &'a Transaction<'tx>,
    limits: deps::Limits,
    queries: u64,
    rows: u64,
    read: usize,
}
impl<'a, 'tx> Context<'a, 'tx> {
    pub fn new(tx: &'a Transaction<'tx>, limits: deps::Limits) -> Self {
        Self {
            tx,
            limits,
            queries: 0,
            rows: 0,
            read: 0,
        }
    }
    fn query(&mut self) -> Result<()> {
        self.queries = self
            .queries
            .checked_add(1)
            .filter(|n| *n <= self.limits.max_queries)
            .ok_or(Error::Budget("Claim context queries"))?;
        Ok(())
    }
    fn read(&mut self, raw: &str) -> Result<()> {
        self.rows = self
            .rows
            .checked_add(1)
            .filter(|n| *n <= self.limits.max_rows)
            .ok_or(Error::Budget("Claim context rows"))?;
        self.read = self
            .read
            .checked_add(raw.len())
            .filter(|n| *n <= self.limits.max_read_bytes)
            .ok_or(Error::Budget("Claim context bytes"))?;
        Ok(())
    }
    pub fn state(&mut self, binding: &Value, source_digest: &str) -> Result<Value> {
        for sql in DDL {
            self.query()?;
            let name = sql
                .split_whitespace()
                .nth(2)
                .unwrap()
                .split('(')
                .next()
                .unwrap();
            let actual:Option<String>=self.tx.query_row("SELECT CASE WHEN length(CAST(sql AS BLOB))<=4096 THEN sql END FROM sqlite_master WHERE name=?",[name],|r|r.get(0)).optional()?;
            if actual.as_deref() != Some(sql) {
                return Err(Error::Invalid("Claim context physical schema"));
            }
            self.read(sql)?;
        }
        self.query()?;
        if self.tx.query_row("SELECT 1 FROM sqlite_master WHERE type='trigger' AND tbl_name IN ('agent_context_state','agent_context_nodes','agent_context_refs','agent_context_heads') LIMIT 1",[],|r|r.get::<_,i64>(0)).optional()?.is_some(){
            return Err(Error::Invalid("Claim context trigger mask"));
        }
        self.query()?;
        if self
            .tx
            .query_row("PRAGMA journal_mode", [], |r| r.get::<_, String>(0))?
            != "wal"
        {
            return Err(Error::Invalid("Claim publication WAL profile"));
        }
        self.query()?;
        let (raw,sha):(Option<String>,String)=self.tx.query_row("SELECT CASE WHEN length(CAST(json AS BLOB))<=? THEN json END,CASE WHEN length(CAST(sha256 AS BLOB))=64 THEN sha256 END FROM agent_context_state WHERE singleton=1",
            [self.limits.max_state],|r|Ok((r.get(0)?,r.get(1)?)))?;
        let raw = raw.ok_or(Error::Budget("Claim context state bytes"))?;
        self.read(&raw)?;
        if bytes::digest(raw.as_bytes()) != sha {
            return Err(Error::Invalid("Claim context state checksum"));
        }
        let state = bytes::parse(raw.as_bytes(), self.limits.max_state)?;
        if state["schema"] != "tos_agent_publication_context_index_v1"
            || state["binding"] != *binding
            || state["source_inputs_sha256"] != source_digest
        {
            return Err(Error::Invalid("Claim context selected binding"));
        }
        Ok(state)
    }
    pub fn head(
        &mut self,
        graph: &str,
        reference: &str,
    ) -> Result<Option<(u64, [u8; 32], [u8; 32])>> {
        self.query()?;
        let found:Option<(u64,String,String,String)>=self.tx.query_row("SELECT n,CASE WHEN length(CAST(xor_sha256 AS BLOB))=64 THEN xor_sha256 END,CASE WHEN length(CAST(sum_sha256 AS BLOB))=64 THEN sum_sha256 END,CASE WHEN length(CAST(seal AS BLOB))=64 THEN seal END FROM agent_context_heads WHERE source_graph=? AND claim_ref=?",
            params![graph,reference],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
        let Some((count, xor, sum, seal)) = found else {
            return Ok(None);
        };
        self.read(&xor)?;
        self.read(&sum)?;
        self.read(&seal)?;
        if count == 0
            || count > MAX_ORDER
            || seal
                != bytes::row_digest(
                    &json!([graph, reference, count, xor, sum]),
                    self.limits.max_row,
                )?
        {
            return Err(Error::Invalid("Claim context head checksum"));
        }
        Ok(Some((count, deps::decode(&xor)?, deps::decode(&sum)?)))
    }
    pub fn members(&mut self, graph: &str, reference: &str, maximum: usize) -> Result<Vec<String>> {
        let Some(expected) = self.head(graph, reference)? else {
            return Ok(Vec::new());
        };
        if expected.0 > maximum as u64 {
            return Err(Error::Budget("Claim complete context contributors"));
        }
        self.query()?;
        let tx = self.tx;
        let mut stmt=tx.prepare("SELECT CASE WHEN length(CAST(id AS BLOB))<=4096 THEN id END,position FROM agent_context_refs WHERE source_graph=? AND claim_ref=? ORDER BY position LIMIT ?")?;
        let mut cursor = stmt.query(params![graph, reference, maximum + 1])?;
        let mut members = Vec::new();
        let mut xor = [0; 32];
        let mut sum = [0; 32];
        while let Some(row) = cursor.next()? {
            if members.len() >= maximum {
                return Err(Error::Budget("Claim complete context count"));
            }
            let id: String = row.get(0)?;
            let position: u64 = row.get(1)?;
            self.read(&id)?;
            if position > MAX_ORDER {
                return Err(Error::Invalid("Claim context position"));
            }
            let contribution = deps::decode(&bytes::row_digest(
                &json!([id, position]),
                self.limits.max_row,
            )?)?;
            deps::aggregate(&mut xor, &mut sum, &contribution);
            members.push(id);
        }
        if (members.len() as u64, xor, sum) != expected {
            return Err(Error::Invalid("Claim complete context group checksum"));
        }
        Ok(members)
    }
    pub fn position(&mut self, node: &Value, maximum: usize) -> Result<u64> {
        let id = bytes::text(node, "id")?;
        self.query()?;
        let (position,raw,seal):(u64,Option<String>,String)=self.tx.query_row("SELECT position,CASE WHEN length(CAST(keys_json AS BLOB))<=? THEN keys_json END,CASE WHEN length(CAST(seal AS BLOB))=64 THEN seal END FROM agent_context_nodes WHERE id=?",
            params![self.limits.max_row,id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
        let raw = raw.ok_or(Error::Budget("Claim context keys bytes"))?;
        self.read(&raw)?;
        let keys = keys(node)?;
        if position > MAX_ORDER
            || seal != bytes::row_digest(&json!([id, position, raw]), self.limits.max_row)?
            || bytes::parse(raw.as_bytes(), self.limits.max_row)? != json!(keys)
        {
            return Err(Error::Invalid("Claim retained context contributor"));
        }
        for (graph, reference) in keys {
            if !self
                .members(&graph, &reference, maximum)?
                .iter()
                .any(|member| member == id)
            {
                return Err(Error::Invalid(
                    "Claim retained contributor omitted from group",
                ));
            }
        }
        Ok(position)
    }
    pub fn rebind(&mut self, mut state: Value, binding: &Value, digest: &str) -> Result<()> {
        let before = bytes::row_digest(&state, self.limits.max_state)?;
        state["binding"] = binding.clone();
        state["source_inputs_sha256"] = json!(digest);
        let raw = bytes::canonical(&state, self.limits.max_state)?;
        self.query()?;
        if self.tx.execute(
            "UPDATE agent_context_state SET json=?,sha256=? WHERE singleton=1 AND sha256=?",
            params![
                std::str::from_utf8(&raw).map_err(|_| Error::Invalid("Claim context UTF8"))?,
                bytes::digest(&raw),
                before
            ],
        )? != 1
        {
            return Err(Error::Invalid("Claim reviewed context CAS"));
        }
        if self.state(binding, digest)? != state {
            return Err(Error::Invalid("Claim reviewed context readback"));
        }
        Ok(())
    }
    pub fn append(
        &mut self,
        mut state: Value,
        new_binding: &Value,
        after_digest: &str,
        ids: &[String],
        nodes: &BTreeMap<String, Value>,
    ) -> Result<()> {
        self.query()?;
        let last: Option<u64> = self
            .tx
            .query_row(
                "SELECT position FROM agent_context_nodes ORDER BY position DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()?;
        let mut next = last
            .map(|n| n.checked_add(1))
            .unwrap_or(Some(0))
            .ok_or(Error::Budget("Claim singleton position exhausted"))?;
        for id in ids {
            if next > MAX_ORDER {
                return Err(Error::Budget("Claim singleton position exhausted"));
            }
            let node = nodes
                .get(id)
                .ok_or(Error::Invalid("Claim singleton candidate absent"))?;
            let keys = keys(node)?;
            if keys.len() != 1
                || keys[0]
                    != (
                        "source-claims".to_owned(),
                        bytes::text(node, "entity_id")?.to_owned(),
                    )
            {
                return Err(Error::Invalid("Claim singleton group identity"));
            }
            let (graph, reference) = &keys[0];
            if self.head(graph, reference)?.is_some() {
                return Err(Error::Invalid("Claim singleton context occupied"));
            }
            let raw = String::from_utf8(bytes::canonical(&json!(keys), self.limits.max_row)?)
                .map_err(|_| Error::Invalid("Claim keys UTF8"))?;
            self.query()?;
            self.tx.execute(
                "INSERT INTO agent_context_nodes VALUES (?,?,?,?)",
                params![
                    id,
                    next,
                    raw,
                    bytes::row_digest(&json!([id, next, raw]), self.limits.max_row)?
                ],
            )?;
            self.query()?;
            self.tx.execute(
                "INSERT INTO agent_context_refs VALUES (?,?,?,?)",
                params![graph, reference, id, next],
            )?;
            let contribution = bytes::row_digest(&json!([id, next]), self.limits.max_row)?;
            let seal = bytes::row_digest(
                &json!([graph, reference, 1, contribution, contribution]),
                self.limits.max_row,
            )?;
            self.query()?;
            self.tx.execute(
                "INSERT INTO agent_context_heads VALUES (?,?,?,?,?,?)",
                params![graph, reference, 1, contribution, contribution, seal],
            )?;
            next = next
                .checked_add(1)
                .ok_or(Error::Budget("Claim context position"))?;
        }
        state["binding"] = new_binding.clone();
        state["source_inputs_sha256"] = json!(after_digest);
        let raw = bytes::canonical(&state, self.limits.max_state)?;
        self.query()?;
        if self.tx.execute(
            "UPDATE agent_context_state SET json=?,sha256=? WHERE singleton=1",
            params![
                std::str::from_utf8(&raw)
                    .map_err(|_| Error::Invalid("Claim context state UTF8"))?,
                bytes::digest(&raw)
            ],
        )? != 1
        {
            return Err(Error::Invalid("Claim context singleton finalization"));
        }
        Ok(())
    }
}
