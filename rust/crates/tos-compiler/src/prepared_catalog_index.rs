//! Maintained contributor catalog index in one caller-owned transaction.
//! On any refusal the caller must roll back the whole transaction.

use crate::prepared_catalog_semantics::{
    self as semantics, CatalogInputs, CatalogOrder, CatalogRenderLimits, CatalogRow,
};
use crate::{Error, Result};
use flate2::{Compression, bufread::ZlibDecoder, write::ZlibEncoder};
use rusqlite::{OptionalExtension, Transaction, params};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use tos_foundation::{Digest256, Digest256Hasher, JsonValue};

/// Distinct native executable identity; Python auxiliary states are refused.
pub fn catalog_projector_sha256() -> String {
    let mut digest = Digest256Hasher::new();
    digest.update(b"tos-native-maintained-catalog-v1\0");
    for bytes in [
        include_bytes!("prepared_catalog_index.rs").as_slice(),
        include_bytes!("prepared_catalog_semantics.rs").as_slice(),
        include_bytes!("prepared_maintenance.rs").as_slice(),
        include_bytes!("prepared_semantic_kernel.rs").as_slice(),
        include_bytes!("local_prepared.rs").as_slice(),
        include_bytes!("d1_public_capture.rs").as_slice(),
        include_bytes!("knowledge_normalization.rs").as_slice(),
        include_bytes!("../../tos-foundation/src/json.rs").as_slice(),
        include_bytes!("../../tos-foundation/src/unicode.rs").as_slice(),
        include_bytes!("../../tos-foundation/src/unicode_generated.rs").as_slice(),
        include_bytes!("../../tos-foundation/src/digest.rs").as_slice(),
    ] {
        digest.update(bytes);
    }
    digest.finalize().to_hex()
}
pub const SCHEMA: &str = "tos-catalog-index-v1";
const DDL: &[&str] = &[
    "CREATE TABLE catalog_state (singleton INTEGER PRIMARY KEY CHECK(singleton=1), schema TEXT NOT NULL, projector TEXT NOT NULL, binding TEXT NOT NULL, header_digest TEXT NOT NULL, catalog_digest TEXT NOT NULL)",
    "CREATE TABLE catalog_atoms (atom INTEGER PRIMARY KEY, value TEXT NOT NULL UNIQUE)",
    "CREATE TABLE catalog_totals (key INTEGER PRIMARY KEY, n INTEGER NOT NULL CHECK(n>0))",
    "CREATE TABLE catalog_contributors (doc INTEGER PRIMARY KEY, kind TEXT NOT NULL, id TEXT NOT NULL, source_order BLOB NOT NULL, row_digest TEXT NOT NULL, facts_digest TEXT NOT NULL, facts BLOB NOT NULL, summary TEXT NOT NULL, from_id TEXT, to_id TEXT, seal TEXT NOT NULL, UNIQUE(kind,id), UNIQUE(kind,source_order))",
    "CREATE INDEX catalog_from ON catalog_contributors(from_id) WHERE kind='relation'",
    "CREATE INDEX catalog_to ON catalog_contributors(to_id) WHERE kind='relation'",
    "CREATE TABLE catalog_occurrences (bucket INTEGER NOT NULL, value INTEGER NOT NULL, doc INTEGER NOT NULL, source_order BLOB NOT NULL, position INTEGER NOT NULL, PRIMARY KEY(bucket,value,doc)) WITHOUT ROWID",
    "CREATE INDEX catalog_occurrence_doc ON catalog_occurrences(doc)",
    "CREATE INDEX catalog_occurrence_first ON catalog_occurrences(bucket,value,source_order,position)",
    "CREATE TABLE catalog_heads (bucket INTEGER NOT NULL, value INTEGER NOT NULL, doc INTEGER NOT NULL, source_order BLOB NOT NULL, position INTEGER NOT NULL, PRIMARY KEY(bucket,value)) WITHOUT ROWID",
    "CREATE INDEX catalog_head_first ON catalog_heads(bucket,source_order,position)",
];

pub type Counts = BTreeMap<Vec<String>, i64>;
pub type Posts = Vec<(Vec<String>, Value, i64)>;
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogMaintenanceLimits {
    pub max_changes: usize,
    pub max_incident_relations: usize,
    pub max_row_bytes: usize,
    pub max_delta_bytes: usize,
    pub max_catalog_entries: usize,
    pub max_aggregate_bytes: usize,
    pub max_catalog_bytes: usize,
    pub max_index_bytes: u64,
}
impl Default for CatalogMaintenanceLimits {
    fn default() -> Self {
        Self {
            max_changes: 4096,
            max_incident_relations: 16384,
            max_row_bytes: 8 * 1024 * 1024,
            max_delta_bytes: 64 * 1024 * 1024,
            max_catalog_entries: 100000,
            max_aggregate_bytes: 64 * 1024 * 1024,
            max_catalog_bytes: 16 * 1024 * 1024,
            max_index_bytes: 4 * 1024 * 1024 * 1024,
        }
    }
}
impl CatalogMaintenanceLimits {
    pub fn validate(self) -> Result<()> {
        if self.max_changes == 0
            || self.max_incident_relations == 0
            || self.max_incident_relations > (i64::MAX as usize).saturating_sub(1)
            || self.max_row_bytes == 0
            || self.max_delta_bytes == 0
            || self.max_catalog_entries == 0
            || self.max_aggregate_bytes == 0
            || self.max_catalog_bytes == 0
            || self.max_index_bytes == 0
            || self
                .max_row_bytes
                .checked_mul(4)
                .and_then(|size| size.checked_add(1))
                .is_none()
        {
            return Err(Error::Invalid("catalog maintenance limits"));
        }
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub struct CatalogChange {
    pub operation: String,
    pub kind: String,
    pub id: String,
    pub expected_old_digest: Option<String>,
    pub new_item: Option<tos_foundation::JsonValue>,
    pub source_order: Option<CatalogOrder>,
}
#[derive(Clone)]
struct Selected {
    doc: i64,
    order: Vec<u8>,
    row_digest: String,
    facts_digest: String,
    facts: Vec<u8>,
    summary: String,
}
struct Facts {
    digest: String,
    counts: Counts,
    posts: Posts,
    summary: JsonValue,
}

/// The native projector is checked on every addressed operation. It is never
/// borrowed from Python state or supplied as an arbitrary caller verdict.
pub struct CatalogIndex<'a, 'conn> {
    tx: &'a Transaction<'conn>,
    limits: CatalogMaintenanceLimits,
    projector: String,
    spent: usize,
    failed: bool,
}
impl<'a, 'conn> CatalogIndex<'a, 'conn> {
    pub fn new(tx: &'a Transaction<'conn>, limits: CatalogMaintenanceLimits) -> Result<Self> {
        limits.validate()?;
        if tx.is_autocommit() {
            return Err(Error::Invalid("catalog caller transaction required"));
        }
        Ok(Self {
            tx,
            limits,
            projector: catalog_projector_sha256(),
            spent: 0,
            failed: false,
        })
    }
    fn transaction(&self) -> Result<()> {
        if self.failed || self.tx.is_autocommit() {
            return Err(Error::Invalid(
                "catalog failed or absent caller transaction",
            ));
        }
        Ok(())
    }
    fn budget(&mut self, amount: usize) -> Result<()> {
        self.spent = self
            .spent
            .checked_add(amount)
            .ok_or(Error::Budget("catalog selected bytes"))?;
        if self.spent > self.limits.max_delta_bytes {
            return Err(Error::Budget("catalog selected bytes"));
        }
        Ok(())
    }
    fn size(&self) -> Result<()> {
        let pages: u64 = self.tx.query_row("PRAGMA page_count", [], |r| r.get(0))?;
        let page: u64 = self.tx.query_row("PRAGMA page_size", [], |r| r.get(0))?;
        if pages
            .checked_mul(page)
            .ok_or(Error::Budget("catalog database pages"))?
            > self.limits.max_index_bytes
        {
            return Err(Error::Budget("catalog database pages"));
        }
        Ok(())
    }
    fn storage_limit(&self) -> Result<()> {
        self.size()?;
        let page: u64 = self.tx.query_row("PRAGMA page_size", [], |r| r.get(0))?;
        let existing: u64 = self
            .tx
            .query_row("PRAGMA max_page_count", [], |r| r.get(0))?;
        let ceiling = existing.min(self.limits.max_index_bytes / page);
        if ceiling == 0 {
            return Err(Error::Budget("catalog database page ceiling"));
        }
        let actual: u64 =
            self.tx
                .query_row(&format!("PRAGMA max_page_count={ceiling}"), [], |r| {
                    r.get(0)
                })?;
        if actual > ceiling {
            return Err(Error::Budget("catalog database page ceiling"));
        }
        Ok(())
    }
    fn atom(&self, value: &str) -> Result<i64> {
        self.tx.execute(
            "INSERT INTO catalog_atoms(value) VALUES(?) ON CONFLICT(value) DO NOTHING",
            [value],
        )?;
        Ok(self.tx.query_row(
            "SELECT atom FROM catalog_atoms WHERE value=?",
            [value],
            |r| r.get(0),
        )?)
    }
    fn state(&self, inputs: &CatalogInputs) -> Result<(String, String)> {
        self.transaction()?;
        let row:Option<(String,String,String,String,String)>=self.tx.query_row("SELECT schema,projector,binding,header_digest,catalog_digest FROM catalog_state WHERE singleton=1 AND length(schema)<128 AND length(projector)=64 AND length(binding)=64 AND length(header_digest)=64 AND length(catalog_digest)=64",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).optional()?;
        let row = row.ok_or(Error::Invalid("catalog index absent or incompatible"))?;
        if row.0 != SCHEMA || row.1 != self.projector || row.2 != inputs.binding()? {
            return Err(Error::Invalid("catalog executable/owner binding drift"));
        }
        for ddl in DDL {
            let name = ddl
                .split_whitespace()
                .nth(2)
                .ok_or(Error::Invalid("catalog DDL"))?;
            let actual: Option<String> = self
                .tx
                .query_row("SELECT sql FROM sqlite_master WHERE name=?", [name], |r| {
                    r.get(0)
                })
                .optional()?;
            if actual.as_deref() != Some(*ddl) {
                return Err(Error::Invalid("catalog physical schema drift"));
            }
        }
        Ok((row.3, row.4))
    }
    fn seal(
        kind: &str,
        id: &str,
        order: &[u8],
        row: &str,
        facts: &str,
        summary: &str,
    ) -> Result<String> {
        let hex: String = order.iter().map(|b| format!("{b:02x}")).collect();
        semantics::catalog_digest(&json!([kind, id, hex, row, facts, summary]))
    }
    fn endpoint(&self, id: &str) -> Result<JsonValue> {
        let row:Option<(Vec<u8>,String,String,String,String)>=self.tx.query_row("SELECT source_order,row_digest,facts_digest,summary,seal FROM catalog_contributors WHERE kind='node' AND id=? AND length(source_order)<=? AND length(row_digest)=64 AND length(facts_digest)=64 AND length(seal)=64 AND length(CAST(summary AS BLOB))<=?",params![id,self.limits.max_row_bytes,self.limits.max_row_bytes],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).optional()?;
        let row = row.ok_or(Error::Invalid("catalog relation endpoint absent"))?;
        if row.4 != Self::seal("node", id, &row.0, &row.1, &row.2, &row.3)? {
            return Err(Error::Invalid("catalog endpoint seal"));
        }
        crate::local_prepared::parse(&row.3, self.limits.max_row_bytes)
    }
    fn facts(
        &self,
        row: &CatalogRow,
        entries: &tos_foundation::JsonValue,
    ) -> Result<(Facts, usize)> {
        let raw = semantics::encoded_owner(&row.item, self.limits.max_row_bytes)?;
        let (counts, posts, summary) = semantics::row_facts(row, entries)?;
        // Prepared contributor IDs are strings. The maintained endpoint seal
        // includes the lookup identifier, so every non-string endpoint fails
        // even when SQLite affinity could find a superficially matching ID.
        if row.kind == "relation"
            && ["from_id", "to_id"].iter().any(|key| {
                summary
                    .object_get(key)
                    .and_then(JsonValue::as_str)
                    .is_none()
            })
        {
            return Err(Error::Invalid(
                "catalog relation endpoint identity must be a string",
            ));
        }
        let digest = semantics::catalog_owner_digest(&row.item, self.limits.max_row_bytes)?;
        Ok((
            Facts {
                digest,
                counts,
                posts,
                summary,
            },
            raw.len(),
        ))
    }
    fn pack(&mut self, counts: &Counts, posts: &Posts) -> Result<(String, Vec<u8>)> {
        let count_rows: Vec<Value> = counts
            .iter()
            .filter(|(_, n)| **n != 0)
            .map(|(k, n)| json!([k, n]))
            .collect();
        let post_rows: Vec<Value> = posts.iter().map(|(b, v, p)| json!([b, v, p])).collect();
        let raw = semantics::encoded(&json!({"counts":count_rows,"posts":post_rows}))?;
        if raw.len() > self.limits.max_row_bytes * 4 {
            return Err(Error::Budget("catalog facts decoded bytes"));
        }
        self.budget(raw.len())?;
        let digest = Digest256::of_bytes(raw.as_bytes()).to_hex();
        let mut compressor = ZlibEncoder::new(Vec::new(), Compression::new(6));
        compressor
            .write_all(raw.as_bytes())
            .map_err(|_| Error::Invalid("catalog facts compress"))?;
        let blob = compressor
            .finish()
            .map_err(|_| Error::Invalid("catalog facts compress"))?;
        if blob.len() > self.limits.max_row_bytes * 4 {
            return Err(Error::Budget("catalog facts compressed bytes"));
        }
        Ok((digest, blob))
    }
    fn unpack(&mut self, digest: &str, blob: &[u8]) -> Result<(Counts, Posts)> {
        let mut decoder = ZlibDecoder::new(blob);
        let mut raw = Vec::new();
        decoder
            .by_ref()
            .take((self.limits.max_row_bytes * 4 + 1) as u64)
            .read_to_end(&mut raw)
            .map_err(|_| Error::Invalid("catalog facts compressed stream"))?;
        if raw.len() > self.limits.max_row_bytes * 4 || decoder.total_in() != blob.len() as u64 {
            return Err(Error::Budget("catalog facts decoded stream"));
        }
        if Digest256::of_bytes(&raw).to_hex() != digest {
            return Err(Error::Invalid("catalog contributor digest"));
        }
        self.budget(raw.len())?;
        let value: Value =
            serde_json::from_slice(&raw).map_err(|_| Error::Invalid("catalog facts JSON"))?;
        let counts = serde_json::from_value::<Vec<(Vec<String>, i64)>>(value["counts"].clone())
            .map_err(|_| Error::Invalid("catalog counts shape"))?
            .into_iter()
            .collect();
        let posts = serde_json::from_value(value["posts"].clone())
            .map_err(|_| Error::Invalid("catalog posts shape"))?;
        Ok((counts, posts))
    }
    fn head(&self, bucket: i64, value: i64) -> Result<()> {
        let row:Option<(i64,Vec<u8>,i64)>=self.tx.query_row("SELECT doc,source_order,position FROM catalog_occurrences WHERE bucket=? AND value=? ORDER BY source_order,position LIMIT 1",params![bucket,value],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
        if let Some((doc, order, position)) = row {
            self.tx.execute("INSERT INTO catalog_heads VALUES(?,?,?,?,?) ON CONFLICT(bucket,value) DO UPDATE SET doc=excluded.doc,source_order=excluded.source_order,position=excluded.position",params![bucket,value,doc,order,position])?;
        } else {
            self.tx.execute(
                "DELETE FROM catalog_heads WHERE bucket=? AND value=?",
                params![bucket, value],
            )?;
        }
        Ok(())
    }
    fn contributions(
        &self,
        doc: i64,
        order: &[u8],
        before: &Counts,
        after: &Counts,
        before_posts: &Posts,
        after_posts: &Posts,
        old_order: Option<&[u8]>,
    ) -> Result<()> {
        let keys: BTreeSet<_> = before.keys().chain(after.keys()).collect();
        for key in keys {
            let delta =
                after.get(key).copied().unwrap_or(0) - before.get(key).copied().unwrap_or(0);
            if delta == 0 {
                continue;
            }
            let atom = self.atom(&semantics::encoded(&json!(key))?)?;
            let n: Option<i64> = self
                .tx
                .query_row("SELECT n FROM catalog_totals WHERE key=?", [atom], |r| {
                    r.get(0)
                })
                .optional()?;
            let n = n
                .unwrap_or(0)
                .checked_add(delta)
                .ok_or(Error::Budget("catalog aggregate count"))?;
            if n < 0 {
                return Err(Error::Invalid("catalog negative aggregate"));
            }
            if n == 0 {
                self.tx
                    .execute("DELETE FROM catalog_totals WHERE key=?", [atom])?;
            } else {
                self.tx.execute("INSERT INTO catalog_totals VALUES(?,?) ON CONFLICT(key) DO UPDATE SET n=excluded.n",params![atom,n])?;
            }
        }
        // Values are atom text, not an equality comparison over mutable JSON.
        let mut old = BTreeMap::new();
        let mut new = BTreeMap::new();
        for (bucket, value, position) in before_posts {
            old.insert(
                (
                    bucket.clone(),
                    value
                        .as_str()
                        .ok_or(Error::Invalid("catalog post encoded value"))?
                        .to_owned(),
                ),
                *position,
            );
        }
        for (bucket, value, position) in after_posts {
            new.insert(
                (
                    bucket.clone(),
                    value
                        .as_str()
                        .ok_or(Error::Invalid("catalog post encoded value"))?
                        .to_owned(),
                ),
                *position,
            );
        }
        let keys: BTreeSet<_> = old.keys().chain(new.keys()).cloned().collect();
        for key in keys {
            if old.get(&key) == new.get(&key)
                && old.contains_key(&key)
                && new.contains_key(&key)
                && old_order == Some(order)
            {
                continue;
            }
            let bucket = self.atom(&semantics::encoded(&json!(key.0))?)?;
            let value = self.atom(&key.1)?;
            if let Some(position) = new.get(&key) {
                self.tx.execute("INSERT INTO catalog_occurrences VALUES(?,?,?,?,?) ON CONFLICT(bucket,value,doc) DO UPDATE SET source_order=excluded.source_order,position=excluded.position",params![bucket,value,doc,order,position])?;
            } else {
                self.tx.execute(
                    "DELETE FROM catalog_occurrences WHERE bucket=? AND value=? AND doc=?",
                    params![bucket, value, doc],
                )?;
            }
            self.head(bucket, value)?;
        }
        Ok(())
    }
    fn selected(&mut self, kind: &str, id: &str) -> Result<Option<Selected>> {
        let row:Option<(Selected,String,Option<String>,Option<String>)>=self.tx.query_row("SELECT doc,source_order,row_digest,facts_digest,facts,summary,seal,from_id,to_id FROM catalog_contributors WHERE kind=? AND id=? AND length(source_order)<=? AND length(row_digest)=64 AND length(facts_digest)=64 AND length(seal)=64 AND length(facts)<=? AND length(CAST(summary AS BLOB))<=?",params![kind,id,self.limits.max_row_bytes,self.limits.max_row_bytes*4,self.limits.max_row_bytes],|r|Ok((Selected {doc:r.get(0)?,order:r.get(1)?,row_digest:r.get(2)?,facts_digest:r.get(3)?,facts:r.get(4)?,summary:r.get(5)?},r.get(6)?,r.get(7)?,r.get(8)?))).optional()?;
        if let Some((record, seal, from, to)) = row {
            self.budget(
                record
                    .facts
                    .len()
                    .checked_add(record.summary.len())
                    .ok_or(Error::Budget("catalog selected bytes"))?,
            )?;
            if seal
                != Self::seal(
                    kind,
                    id,
                    &record.order,
                    &record.row_digest,
                    &record.facts_digest,
                    &record.summary,
                )?
            {
                return Err(Error::Invalid("catalog selected contributor seal"));
            }
            let summary = crate::local_prepared::parse(&record.summary, self.limits.max_row_bytes)?;
            if from.as_deref() != summary.object_get("from_id").and_then(JsonValue::as_str)
                || to.as_deref() != summary.object_get("to_id").and_then(JsonValue::as_str)
            {
                return Err(Error::Invalid("catalog adjacency binding"));
            }
            Ok(Some(record))
        } else {
            let exists: Option<i64> = self
                .tx
                .query_row(
                    "SELECT 1 FROM catalog_contributors WHERE kind=? AND id=?",
                    params![kind, id],
                    |r| r.get(0),
                )
                .optional()?;
            if exists.is_some() {
                return Err(Error::Invalid("catalog contributor framing or byte budget"));
            }
            Ok(None)
        }
    }
    fn remove(&mut self, record: &Selected) -> Result<()> {
        let (counts, posts) = self.unpack(&record.facts_digest, &record.facts)?;
        self.contributions(
            record.doc,
            &record.order,
            &counts,
            &Counts::new(),
            &posts,
            &Vec::new(),
            None,
        )?;
        self.tx
            .execute("DELETE FROM catalog_contributors WHERE doc=?", [record.doc])?;
        Ok(())
    }
    fn insert_facts(&mut self, kind: &str, id: &str, order: &[u8], facts: &Facts) -> Result<i64> {
        let (digest, blob) = self.pack(&facts.counts, &facts.posts)?;
        let summary =
            semantics::encoded_canonical_owner(&facts.summary, self.limits.max_row_bytes)?;
        let seal = Self::seal(kind, id, order, &facts.digest, &digest, &summary)?;
        self.tx.execute("INSERT INTO catalog_contributors (kind,id,source_order,row_digest,facts_digest,facts,summary,from_id,to_id,seal) VALUES(?,?,?,?,?,?,?,?,?,?)",params![kind,id,order,facts.digest,digest,blob,summary,sql_endpoint(&facts.summary,"from_id")?,sql_endpoint(&facts.summary,"to_id")?,seal])?;
        let doc = self.tx.last_insert_rowid();
        self.contributions(
            doc,
            order,
            &Counts::new(),
            &facts.counts,
            &Vec::new(),
            &facts.posts,
            None,
        )?;
        Ok(doc)
    }
    fn render_unchecked(&self, inputs: &CatalogInputs) -> Result<JsonValue> {
        semantics::render_catalog(
            inputs,
            self.tx,
            true,
            CatalogRenderLimits {
                max_output_entries: self.limits.max_catalog_entries as u64,
                max_aggregate_bytes: self.limits.max_aggregate_bytes,
                max_catalog_bytes: self.limits.max_catalog_bytes,
            },
        )
    }
    pub fn render(&mut self, inputs: &CatalogInputs) -> Result<JsonValue> {
        let result = (|| {
            self.transaction()?;
            self.size()?;
            let (header, digest) = self.state(inputs)?;
            if header != inputs.header_digest()? {
                return Err(Error::Invalid("catalog header digest"));
            }
            let catalog = self.render_unchecked(inputs)?;
            if semantics::catalog_owner_digest(&catalog, self.limits.max_catalog_bytes)? != digest {
                return Err(Error::Invalid("catalog aggregate digest"));
            }
            Ok(catalog)
        })();
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    pub fn bootstrap<I: IntoIterator<Item = Result<CatalogRow>>>(
        &mut self,
        inputs: &CatalogInputs,
        rows: I,
    ) -> Result<JsonValue> {
        let result = (|| {
            self.transaction()?;
            self.storage_limit()?;
            let exists: Option<i64> = self
                .tx
                .query_row(
                    "SELECT 1 FROM sqlite_master WHERE name GLOB 'catalog_*' LIMIT 1",
                    [],
                    |r| r.get(0),
                )
                .optional()?;
            if exists.is_some() {
                return Err(Error::Invalid("catalog bootstrap requires absent index"));
            }
            for ddl in DDL {
                self.tx.execute(ddl, [])?;
            }
            let entries = &inputs.entity_registry;
            let mut relation_phase = false;
            for (index, row) in rows.into_iter().enumerate() {
                let row = row?;
                if row.kind == "relation" {
                    relation_phase = true;
                } else if row.kind != "node" || relation_phase {
                    return Err(Error::Invalid("catalog nodes before relations required"));
                }
                validate_order(inputs, &row)?;
                self.spent = 0;
                let (mut facts, size) = self.facts(&row, entries)?;
                self.budget(size)?;
                add_counts(
                    &mut facts.counts,
                    semantics::route_counts(&row.kind, &facts.summary, |id| {
                        self.endpoint(id).map(Some)
                    })?,
                )?;
                self.insert_facts(
                    &row.kind,
                    &row.id,
                    &semantics::order_key(&row.source_order)?,
                    &facts,
                )?;
                if index % 1024 == 0 {
                    self.size()?;
                }
            }
            let catalog = self.render_unchecked(inputs)?;
            let header = semantics::finalized_header(inputs, &catalog)?;
            self.tx.execute(
                "INSERT INTO catalog_state VALUES(1,?,?,?,?,?)",
                params![
                    SCHEMA,
                    self.projector,
                    inputs.binding()?,
                    semantics::catalog_owner_digest(&header, self.limits.max_catalog_bytes)?,
                    semantics::catalog_owner_digest(&catalog, self.limits.max_catalog_bytes)?
                ],
            )?;
            self.size()?;
            Ok(catalog)
        })();
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    pub fn apply_delta(
        &mut self,
        before: &CatalogInputs,
        after: &CatalogInputs,
        changes: &[CatalogChange],
        expected_catalog_digest: Option<&str>,
    ) -> Result<JsonValue> {
        let result = (|| {
            self.transaction()?;
            self.storage_limit()?;
            let (header, digest) = self.state(before)?;
            if before.header_digest()? != header || after.binding()? != before.binding()? {
                return Err(Error::Invalid("catalog before header/after owner binding"));
            }
            if expected_catalog_digest.is_some_and(|expected| expected != digest) {
                return Err(Error::Invalid("catalog expected before digest"));
            }
            if semantics::catalog_owner_digest(
                &self.render_unchecked(before)?,
                self.limits.max_catalog_bytes,
            )? != digest
            {
                return Err(Error::Invalid("catalog before aggregate digest"));
            }
            if changes.len() > self.limits.max_changes {
                return Err(Error::Budget("catalog changes"));
            }
            self.spent = 0;
            let entries = &after.entity_registry;
            let mut seen = BTreeSet::new();
            let mut selected = Vec::new();
            let mut affected = BTreeSet::new();
            for change in changes {
                if change.id.is_empty()
                    || !matches!(change.kind.as_str(), "node" | "relation")
                    || !matches!(change.operation.as_str(), "insert" | "update" | "delete")
                    || !seen.insert((change.kind.clone(), change.id.clone()))
                {
                    return Err(Error::Invalid("catalog duplicate/invalid change"));
                }
                let record = self.selected(&change.kind, &change.id)?;
                if change.operation == "insert" {
                    if record.is_some()
                        || change.expected_old_digest.is_some()
                        || change.source_order.is_none()
                    {
                        return Err(Error::Invalid("catalog insert absent/order"));
                    }
                } else if record.as_ref().map(|r| r.row_digest.as_str())
                    != change.expected_old_digest.as_deref()
                    || record.is_none()
                {
                    return Err(Error::Invalid("catalog old row digest"));
                }
                let facts = if change.operation == "delete" {
                    if change.new_item.is_some() || change.source_order.is_some() {
                        return Err(Error::Invalid("catalog deletion replacement/order"));
                    }
                    None
                } else {
                    let order = match &change.source_order {
                        Some(order) => order.clone(),
                        None => semantics::order_value(
                            &record
                                .as_ref()
                                .ok_or(Error::Invalid("catalog existing order"))?
                                .order,
                        )?,
                    };
                    let row = CatalogRow {
                        kind: change.kind.clone(),
                        id: change.id.clone(),
                        source_order: order,
                        item: change
                            .new_item
                            .clone()
                            .ok_or(Error::Invalid("catalog replacement"))?,
                    };
                    validate_order(after, &row)?;
                    let (facts, size) = self.facts(&row, entries)?;
                    self.budget(size)?;
                    let count_rows: Vec<Value> =
                        facts.counts.iter().map(|(k, n)| json!([k, n])).collect();
                    let posts: Vec<Value> = facts
                        .posts
                        .iter()
                        .map(|(b, v, p)| json!([b, v, p]))
                        .collect();
                    let summary_size = semantics::encoded_canonical_owner(
                        &facts.summary,
                        self.limits.max_row_bytes,
                    )?
                    .len();
                    let selected_size = semantics::encoded(&json!(count_rows))?
                        .len()
                        .checked_add(semantics::encoded(&json!(posts))?.len())
                        .and_then(|size| size.checked_add(summary_size))
                        .and_then(|size| size.checked_add(4))
                        .ok_or(Error::Budget("catalog selected facts bytes"))?;
                    self.budget(selected_size)?;
                    Some(facts)
                };
                if change.kind == "node" {
                    let old = record
                        .as_ref()
                        .map(|r| {
                            crate::local_prepared::parse(&r.summary, self.limits.max_row_bytes)
                        })
                        .transpose()?;
                    let masks = |v: &JsonValue| {
                        (
                            v.object_get("legacy").cloned(),
                            v.object_get("typed").cloned(),
                        )
                    };
                    if old.as_ref().map(masks) != facts.as_ref().map(|f| masks(&f.summary)) {
                        for column in ["from_id", "to_id"] {
                            let sql = format!(
                                "SELECT id FROM catalog_contributors WHERE kind='relation' AND {column}=? LIMIT ?"
                            );
                            let mut stmt = self.tx.prepare(&sql)?;
                            let mut rows = stmt.query(params![
                                change.id,
                                self.limits.max_incident_relations + 1
                            ])?;
                            while let Some(row) = rows.next()? {
                                affected.insert(row.get::<_, String>(0)?);
                                if affected.len() > self.limits.max_incident_relations {
                                    return Err(Error::Budget("catalog incident closure"));
                                }
                            }
                        }
                    }
                }
                selected.push((change, record, facts));
            }
            for (change, record, facts) in &selected {
                if let Some(record) = record {
                    if facts.is_none()
                        || change
                            .source_order
                            .as_ref()
                            .map(semantics::order_key)
                            .transpose()?
                            .is_some_and(|o| o != record.order)
                    {
                        let mut temporary = vec![0];
                        temporary.extend_from_slice(&record.doc.to_be_bytes());
                        let seal = Self::seal(
                            &change.kind,
                            &change.id,
                            &temporary,
                            &record.row_digest,
                            &record.facts_digest,
                            &record.summary,
                        )?;
                        self.tx.execute(
                            "UPDATE catalog_contributors SET source_order=?,seal=? WHERE doc=?",
                            params![temporary, seal, record.doc],
                        )?;
                    }
                }
            }
            selected.sort_by_key(|(change, _, _)| change.kind != "node");
            for (change, record, facts) in selected {
                if change.kind == "relation" {
                    affected.insert(change.id.clone());
                }
                let Some(mut facts) = facts else {
                    self.remove(&record.ok_or(Error::Invalid("catalog delete selection"))?)?;
                    continue;
                };
                if change.kind == "node" {
                    add_counts(
                        &mut facts.counts,
                        semantics::route_counts("node", &facts.summary, |id| {
                            self.endpoint(id).map(Some)
                        })?,
                    )?;
                }
                let order = match &change.source_order {
                    Some(order) => semantics::order_key(order)?,
                    None => record
                        .as_ref()
                        .ok_or(Error::Invalid("catalog update order"))?
                        .order
                        .clone(),
                };
                if let Some(record) = record {
                    let (old_counts, old_posts) =
                        self.unpack(&record.facts_digest, &record.facts)?;
                    let (digest, blob) = self.pack(&facts.counts, &facts.posts)?;
                    let summary = semantics::encoded_canonical_owner(
                        &facts.summary,
                        self.limits.max_row_bytes,
                    )?;
                    let seal = Self::seal(
                        &change.kind,
                        &change.id,
                        &order,
                        &facts.digest,
                        &digest,
                        &summary,
                    )?;
                    self.tx.execute("UPDATE catalog_contributors SET source_order=?,row_digest=?,facts_digest=?,facts=?,summary=?,from_id=?,to_id=?,seal=? WHERE doc=?",params![order,facts.digest,digest,blob,summary,sql_endpoint(&facts.summary,"from_id")?,sql_endpoint(&facts.summary,"to_id")?,seal,record.doc])?;
                    self.contributions(
                        record.doc,
                        &order,
                        &old_counts,
                        &facts.counts,
                        &old_posts,
                        &facts.posts,
                        Some(&record.order),
                    )?;
                } else {
                    self.insert_facts(&change.kind, &change.id, &order, &facts)?;
                }
            }
            if affected.len() > self.limits.max_incident_relations {
                return Err(Error::Budget("catalog changed relation closure"));
            }
            for id in affected {
                let Some(record) = self.selected("relation", &id)? else {
                    continue;
                };
                let (counts, posts) = self.unpack(&record.facts_digest, &record.facts)?;
                let summary =
                    crate::local_prepared::parse(&record.summary, self.limits.max_row_bytes)?;
                self.endpoint(
                    summary
                        .object_get("from_id")
                        .and_then(JsonValue::as_str)
                        .ok_or(Error::Invalid("catalog final from endpoint"))?,
                )?;
                self.endpoint(
                    summary
                        .object_get("to_id")
                        .and_then(JsonValue::as_str)
                        .ok_or(Error::Invalid("catalog final to endpoint"))?,
                )?;
                let mut after: Counts = counts
                    .iter()
                    .filter(|(k, _)| {
                        !k.get(1)
                            .is_some_and(|m| m == "route-type" || m == "route-predicate")
                    })
                    .map(|(k, v)| (k.clone(), *v))
                    .collect();
                add_counts(
                    &mut after,
                    semantics::route_counts("relation", &summary, |id| {
                        self.endpoint(id).map(Some)
                    })?,
                )?;
                if counts != after {
                    let (digest, blob) = self.pack(&after, &posts)?;
                    let seal = Self::seal(
                        "relation",
                        &id,
                        &record.order,
                        &record.row_digest,
                        &digest,
                        &record.summary,
                    )?;
                    self.tx.execute(
                        "UPDATE catalog_contributors SET facts_digest=?,facts=?,seal=? WHERE doc=?",
                        params![digest, blob, seal, record.doc],
                    )?;
                    self.contributions(
                        record.doc,
                        &record.order,
                        &counts,
                        &after,
                        &Vec::new(),
                        &Vec::new(),
                        None,
                    )?;
                }
            }
            let catalog = self.render_unchecked(after)?;
            let header = semantics::finalized_header(after, &catalog)?;
            self.tx.execute(
                "UPDATE catalog_state SET header_digest=?,catalog_digest=? WHERE singleton=1",
                params![
                    semantics::catalog_owner_digest(&header, self.limits.max_catalog_bytes)?,
                    semantics::catalog_owner_digest(&catalog, self.limits.max_catalog_bytes)?
                ],
            )?;
            self.size()?;
            Ok(catalog)
        })();
        if result.is_err() {
            self.failed = true;
        }
        result
    }
}
fn add_counts(target: &mut Counts, extra: Counts) -> Result<()> {
    for (key, n) in extra {
        let old = target.get(&key).copied().unwrap_or(0);
        target.insert(
            key,
            old.checked_add(n)
                .ok_or(Error::Budget("catalog count overflow"))?,
        );
    }
    Ok(())
}
fn validate_order(inputs: &CatalogInputs, row: &CatalogRow) -> Result<()> {
    if row.id.is_empty() {
        return Err(Error::Invalid("catalog row identity"));
    }
    // CatalogOrder's profile contract is checked by the semantics owner.
    semantics::validate_row_order(inputs, row)
}

fn sql_endpoint<'a>(summary: &'a JsonValue, key: &str) -> Result<Option<&'a str>> {
    match summary.object_get(key) {
        None | Some(JsonValue::Null) => Ok(None),
        Some(JsonValue::String(value)) => Ok(Some(
            value
                .as_str()
                .ok_or(Error::Invalid("catalog endpoint UTF-8"))?,
        )),
        _ => Err(Error::Invalid("catalog SQL endpoint value unsupported")),
    }
}
