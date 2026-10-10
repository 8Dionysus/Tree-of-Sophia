//! Physical incidence and canonical sparse positions for the Claim caller.
//! Only addressed seeks are permitted; no recursive or whole-graph fallback.
use super::{
    source_claim_publication_bytes as bytes, source_claim_publication_dependencies as deps,
};
use rusqlite::{OptionalExtension, Transaction, params};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use tos_compiler::{
    Error, Result, local_prepared::PreparedChange, prepared_catalog_semantics as catalog,
};
use tos_foundation::{JsonLimits, JsonMode, parse_json};

pub(super) struct Graph<'a, 'tx> {
    tx: &'a Transaction<'tx>,
    limits: deps::Limits,
    queries: u64,
    rows: u64,
    read: usize,
}
impl<'a, 'tx> Graph<'a, 'tx> {
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
            .ok_or(Error::Budget("Claim closure queries"))?;
        Ok(())
    }
    fn read(&mut self, raw: &str) -> Result<()> {
        self.rows = self
            .rows
            .checked_add(1)
            .filter(|n| *n <= self.limits.max_rows)
            .ok_or(Error::Budget("Claim closure rows"))?;
        self.read = self
            .read
            .checked_add(raw.len())
            .filter(|n| *n <= self.limits.max_read_bytes)
            .ok_or(Error::Budget("Claim closure bytes"))?;
        Ok(())
    }
    pub fn body(&mut self, kind: &str, id: &str) -> Result<(Value, Vec<u8>)> {
        let columns: &[&str] = match kind {
            "node" => &[
                "id",
                "entity_id",
                "native_id",
                "source_graph",
                "kind_id",
                "type_id",
            ],
            "relation" => &[
                "id",
                "native_id",
                "source_graph",
                "from_id",
                "to_id",
                "predicate_id",
                "relation_type_id",
            ],
            _ => return Err(Error::Invalid("Claim closure row kind")),
        };
        self.query()?;
        let protected = columns
            .iter()
            .map(|c| format!("CASE WHEN length(CAST({c} AS BLOB))<=? THEN {c} END"))
            .collect::<Vec<_>>();
        let sql = format!(
            "SELECT {},CASE WHEN length(CAST(json AS BLOB))<=? THEN json END FROM knowledge_{kind}s WHERE id=?",
            protected.join(",")
        );
        let mut values: Vec<rusqlite::types::Value> =
            vec![rusqlite::types::Value::Integer(self.limits.max_row as i64); columns.len() + 1];
        values.push(rusqlite::types::Value::Text(id.to_owned()));
        let (indexed, raw): (Vec<String>, String) =
            self.tx
                .query_row(&sql, rusqlite::params_from_iter(values), |r| {
                    let mut indexed = Vec::new();
                    for i in 0..columns.len() {
                        indexed.push(r.get(i)?);
                    }
                    Ok((indexed, r.get(columns.len())?))
                })?;
        self.read(&raw)?;
        for value in &indexed {
            self.read(value)?;
        }
        self.query()?;
        let key = format!("knowledge_{kind}_digest:{id}");
        let mut statement=self.tx.prepare("SELECT part,CASE WHEN length(CAST(json_chunk AS BLOB))<=1024 THEN json_chunk END FROM edge_meta WHERE key=? ORDER BY part LIMIT 2")?;
        let rows = statement
            .query_map([key], |r| Ok((r.get::<_, u64>(0)?, r.get::<_, String>(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        if rows.len() != 1
            || rows[0].0 != 0
            || bytes::parse(rows[0].1.as_bytes(), 1024)?
                != json!({"sha256":bytes::digest(raw.as_bytes())})
        {
            return Err(Error::Invalid("Claim closure emitted row checksum"));
        }
        let limits = JsonLimits::new(self.limits.max_row, 128, 1_000_000, 4096)
            .map_err(|_| Error::Budget("Claim closure JSON limits"))?;
        let document = parse_json(raw.as_bytes(), JsonMode::PublishedStrict, limits)
            .map_err(|e| Error::Source(e.to_string()))?;
        for (column, found) in columns.iter().zip(indexed) {
            if tos_compiler::local_prepared::index_value(document.root(), column)? != found {
                return Err(Error::Invalid("Claim closure physical identity column"));
            }
        }
        Ok((
            bytes::parse(raw.as_bytes(), self.limits.max_row)?,
            raw.into_bytes(),
        ))
    }
    pub fn incidence(
        &mut self,
        nodes: &BTreeSet<String>,
        maximum: usize,
    ) -> Result<BTreeSet<String>> {
        for (column, index) in [
            ("from_id", "knowledge_relations_from_seek"),
            ("to_id", "knowledge_relations_to_seek"),
        ] {
            self.query()?;
            let mut statement = self.tx.prepare("PRAGMA index_list(knowledge_relations)")?;
            let rows = statement
                .query_map([], |r| Ok((r.get::<_, String>(1)?, r.get::<_, i64>(4)?)))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            if !rows
                .iter()
                .any(|(name, partial)| name == index && *partial == 0)
            {
                return Err(Error::Invalid("Claim incidence physical seek index"));
            }
            drop(statement);
            self.query()?;
            let mut statement = self.tx.prepare(&format!("PRAGMA index_xinfo({index})"))?;
            let rows = statement
                .query_map([], |r| {
                    Ok((
                        r.get::<_, Option<String>>(2)?,
                        r.get::<_, i64>(3)?,
                        r.get::<_, Option<String>>(4)?,
                        r.get::<_, i64>(5)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            let keys: Vec<_> = rows.into_iter().filter(|r| r.3 != 0).collect();
            if keys.len() != 2
                || keys[0].0.as_deref() != Some(column)
                || keys[1].0.as_deref() != Some("id")
                || keys
                    .iter()
                    .any(|r| r.1 != 0 || r.2.as_deref() != Some("BINARY"))
            {
                return Err(Error::Invalid("Claim incidence index order/collation"));
            }
        }
        let mut found = BTreeSet::new();
        for id in nodes {
            for (column, index) in [
                ("from_id", "knowledge_relations_from_seek"),
                ("to_id", "knowledge_relations_to_seek"),
            ] {
                self.query()?;
                let tx = self.tx;
                let mut statement=tx.prepare(&format!("SELECT CASE WHEN length(CAST(id AS BLOB))<=4096 THEN id END FROM knowledge_relations INDEXED BY {index} WHERE {column}=? ORDER BY id LIMIT ?"))?;
                let mut cursor = statement.query(params![id, maximum + 1])?;
                while let Some(row) = cursor.next()? {
                    let id: String = row.get(0)?;
                    self.read(&id)?;
                    found.insert(id);
                    if found.len() > maximum {
                        return Err(Error::Budget("Claim complete incidence"));
                    }
                }
            }
        }
        Ok(found)
    }
    pub fn changes(
        &mut self,
        before_nodes: &BTreeMap<String, Value>,
        before_relations: &BTreeMap<String, Value>,
        after_nodes: &[Value],
        after_relations: &[Value],
    ) -> Result<Vec<PreparedChange>> {
        let mut result = Vec::new();
        for (kind, before, after) in [
            ("node", before_nodes, after_nodes),
            ("relation", before_relations, after_relations),
        ] {
            let mut rows: Vec<_> = after.iter().collect();
            rows.sort_by(|a, b| {
                (a["source_graph"].as_str(), a["id"].as_str())
                    .cmp(&(b["source_graph"].as_str(), b["id"].as_str()))
            });
            let mut inserted: Vec<(Vec<u8>, u64)> = Vec::new();
            for row in rows {
                let id = bytes::text(row, "id")?;
                let old = before.get(id);
                if old
                    .map(|old| bytes::row_digest(old, self.limits.max_row))
                    .transpose()?
                    == Some(bytes::row_digest(row, self.limits.max_row)?)
                {
                    continue;
                }
                let mut position = None;
                if old.is_none() {
                    let order = catalog::order_key(&catalog::CatalogOrder::SourceGraphId(
                        bytes::text(row, "source_graph")?.to_owned(),
                        id.to_owned(),
                    ))?;
                    let mut neighbors = [0, 9_007_199_254_740_991u64];
                    for (i, operator, direction) in [(0, "<", "DESC"), (1, ">", "ASC")] {
                        self.query()?;
                        let sql = format!(
                            "SELECT CASE WHEN length(CAST(c.id AS BLOB))<=4096 THEN c.id END,CASE WHEN length(c.source_order)<=? THEN c.source_order END,p.source_order,CASE WHEN length(CAST(c.row_digest AS BLOB))=64 THEN c.row_digest END,CASE WHEN length(CAST(c.facts_digest AS BLOB))=64 THEN c.facts_digest END,CASE WHEN length(CAST(c.summary AS BLOB))<=? THEN c.summary END,CASE WHEN length(CAST(c.seal AS BLOB))=64 THEN c.seal END FROM catalog_contributors c JOIN prepared_documents p ON p.kind=c.kind AND p.id=c.id WHERE c.kind=? AND c.source_order{operator}? ORDER BY c.source_order {direction} LIMIT 1"
                        );
                        let found: Option<(String, Vec<u8>, u64, String, String, String, String)> =
                            self.tx
                                .query_row(
                                    &sql,
                                    params![self.limits.max_row, self.limits.max_row, kind, order],
                                    |r| {
                                        Ok((
                                            r.get(0)?,
                                            r.get(1)?,
                                            r.get(2)?,
                                            r.get(3)?,
                                            r.get(4)?,
                                            r.get(5)?,
                                            r.get(6)?,
                                        ))
                                    },
                                )
                                .optional()?;
                        if let Some((
                            identity,
                            encoded,
                            token,
                            row_digest,
                            facts_digest,
                            summary,
                            seal,
                        )) = found
                        {
                            self.read(&summary)?;
                            let body = self.body(kind, &identity)?.0;
                            let expected_order =
                                catalog::order_key(&catalog::CatalogOrder::SourceGraphId(
                                    bytes::text(&body, "source_graph")?.to_owned(),
                                    bytes::text(&body, "id")?.to_owned(),
                                ))?;
                            let hex: String = encoded.iter().map(|b| format!("{b:02x}")).collect();
                            let expected_seal = catalog::catalog_digest(&json!([
                                kind,
                                identity,
                                hex,
                                row_digest,
                                facts_digest,
                                summary
                            ]))?;
                            self.query()?;
                            let semantic: u64 = self.tx.query_row(
                                "SELECT source_order FROM semantic_rows WHERE kind=? AND id=?",
                                params![kind, identity],
                                |r| r.get(0),
                            )?;
                            if expected_seal != seal
                                || expected_order != encoded
                                || catalog::catalog_digest(&body)? != row_digest
                                || semantic != token
                            {
                                return Err(Error::Invalid(
                                    "Claim canonical sparse neighbor differs across indexes",
                                ));
                            }
                            neighbors[i] = token;
                        }
                    }
                    let lower = inserted
                        .iter()
                        .filter(|(prior, _)| prior < &order)
                        .map(|(_, token)| *token)
                        .max()
                        .unwrap_or(neighbors[0])
                        .max(neighbors[0]);
                    let upper = neighbors[1];
                    if upper.checked_sub(lower).is_none_or(|gap| gap < 2) {
                        return Err(Error::Budget(
                            "Claim canonical sparse gap requires rebootstrap",
                        ));
                    }
                    let token = lower + (upper - lower) / 2;
                    inserted.push((order, token));
                    position = Some(token);
                }
                let document = parse_json(
                    &bytes::canonical(row, self.limits.max_row)?,
                    JsonMode::PublishedStrict,
                    JsonLimits::new(self.limits.max_row, 128, 1_000_000, 4096)
                        .map_err(|_| Error::Budget("Claim delta JSON limits"))?,
                )
                .map_err(|e| Error::Source(e.to_string()))?;
                result.push(PreparedChange {
                    operation: if old.is_none() { "insert" } else { "update" }.to_owned(),
                    kind: kind.to_owned(),
                    identifier: id.to_owned(),
                    item: Some(document.root().clone()),
                    source_order: position,
                });
            }
        }
        Ok(result)
    }
}
