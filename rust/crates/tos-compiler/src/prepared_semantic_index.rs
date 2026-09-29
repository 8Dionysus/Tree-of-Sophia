//! Addressed auxiliary semantic maintenance in one caller-owned transaction.
//! Refusals require rollback of all publication lanes. Invalid mechanical
//! reports remain data; this module grants no semantic admission.
use crate::prepared_semantic_kernel::{Kernel, SemanticCarrier, SemanticLookup, python_eq};
use crate::{
    Error, Result,
    knowledge_normalization::stable_digest,
    local_prepared::{MAX_ADDRESS, SCHEMA, compact as encode, parse},
};
use rusqlite::{Row, Statement, Transaction, types::Value as SqlValue};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
};
use tos_foundation::{Digest256, Digest256Hasher, JsonValue};
const VERSION: &str = "tos_auxiliary_semantic_index_v1";
const COUNTS: [&str; 6] = [
    "registered_node_count",
    "unmapped_node_count",
    "registered_relation_count",
    "unmapped_relation_count",
    "claim_contract_count",
    "cross_layer_relation_count",
];
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticMaintenanceLimits {
    pub max_changes: usize,
    pub max_rows: u64,
    pub max_queries: u64,
    pub max_writes: u64,
    pub max_read_bytes: usize,
    pub max_input_bytes: usize,
    pub max_input_values: usize,
    pub max_row_bytes: usize,
    pub max_output_bytes: usize,
    pub max_output_items: usize,
    pub max_bytes: u64,
}
impl Default for SemanticMaintenanceLimits {
    fn default() -> Self {
        Self {
            max_changes: 4096,
            max_rows: 2_000_000,
            max_queries: 2_000_000,
            max_writes: 2_000_000,
            max_read_bytes: 256 * 1024 * 1024,
            max_input_bytes: 32 * 1024 * 1024,
            max_input_values: 2_000_000,
            max_row_bytes: 1_048_576,
            max_output_bytes: 8 * 1024 * 1024,
            max_output_items: 65536,
            max_bytes: 256 * 1024 * 1024,
        }
    }
}
impl SemanticMaintenanceLimits {
    pub fn validate(self) -> Result<()> {
        if self.max_changes == 0
            || self.max_rows == 0
            || self.max_queries == 0
            || self.max_writes == 0
            || self.max_read_bytes == 0
            || self.max_input_bytes == 0
            || self.max_input_values == 0
            || self.max_row_bytes == 0
            || self.max_output_bytes == 0
            || self.max_output_items == 0
            || self.max_bytes == 0
            || self.max_bytes > 1 << 40
        {
            return Err(Error::Invalid("semantic index positive portable limits"));
        }
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub struct SemanticChange {
    pub operation: String,
    pub kind: String,
    pub identifier: String,
    pub item: Option<JsonValue>,
    pub source_order: Option<u64>,
}
pub trait SemanticRows {
    fn visit(&mut self, kind: &str, sink: &mut dyn FnMut(&JsonValue) -> Result<()>) -> Result<()>;
}
pub fn semantic_projector_sha256() -> String {
    let mut h = Digest256Hasher::new();
    h.update(b"tos-native-maintained-semantic-v1\0");
    h.update(include_bytes!("prepared_semantic_index.rs"));
    h.update(include_bytes!("prepared_semantic_kernel.rs"));
    h.update(include_bytes!("local_prepared.rs"));
    h.update(include_bytes!("d1_public_capture.rs"));
    h.update(include_bytes!("knowledge_normalization.rs"));
    h.update(include_bytes!("../../tos-foundation/src/json.rs"));
    h.update(include_bytes!("../../tos-foundation/src/unicode.rs"));
    h.update(include_bytes!(
        "../../tos-foundation/src/unicode_generated.rs"
    ));
    h.update(include_bytes!("../../tos-foundation/src/digest.rs"));
    h.finalize().to_hex()
}
fn compact(v: &Value, cap: usize) -> Result<String> {
    if v.get("registered_node_count").is_some() && v.get("violations").is_some() {
        return report_raw(v, cap);
    }
    let raw = serde_json::to_string(v).map_err(|e| Error::Source(e.to_string()))?;
    encode(&parse(&raw, cap)?, cap)
}
fn decode(raw: &str) -> Result<Value> {
    serde_json::from_str(raw).map_err(|e| Error::Source(e.to_string()))
}
fn sha(raw: &str) -> String {
    Digest256::of_bytes(raw.as_bytes()).to_hex()
}
fn sv(s: &str) -> SqlValue {
    SqlValue::Text(s.into())
}
fn iv(n: u64) -> Result<SqlValue> {
    Ok(SqlValue::Integer(
        i64::try_from(n).map_err(|_| Error::Budget("semantic SQL bound"))?,
    ))
}
fn st(v: &SqlValue) -> Result<&str> {
    if let SqlValue::Text(s) = v {
        Ok(s)
    } else {
        Err(Error::Invalid("semantic stored text"))
    }
}
fn int(v: &SqlValue) -> Result<i64> {
    if let SqlValue::Integer(n) = v {
        Ok(*n)
    } else {
        Err(Error::Invalid("semantic stored integer"))
    }
}
struct Budget<'a, 't> {
    tx: &'a Transaction<'t>,
    l: SemanticMaintenanceLimits,
    rows: u64,
    queries: u64,
    read: usize,
    input_bytes: usize,
    input_values: usize,
    output_bytes: usize,
    output_items: usize,
    initial: u64,
    meta_raw: BTreeMap<String, String>,
}
impl<'a, 't> Budget<'a, 't> {
    fn new(tx: &'a Transaction<'t>, l: SemanticMaintenanceLimits) -> Result<Self> {
        l.validate()?;
        if tx.is_autocommit() {
            return Err(Error::Invalid("semantic caller-owned transaction"));
        }
        let mut b = Self {
            tx,
            l,
            rows: 0,
            queries: 0,
            read: 0,
            input_bytes: 0,
            input_values: 0,
            output_bytes: 0,
            output_items: 0,
            initial: tx.total_changes(),
            meta_raw: BTreeMap::new(),
        };
        let size = int(&b
            .one("PRAGMA page_size", &[])?
            .ok_or(Error::Invalid("semantic page size"))?[0])? as u64;
        let pages = (int(&b
            .one("PRAGMA max_page_count", &[])?
            .ok_or(Error::Invalid("semantic page cap"))?[0])? as u64)
            .min(l.max_bytes / size);
        if pages < 1
            || int(&b
                .one("PRAGMA page_count", &[])?
                .ok_or(Error::Invalid("semantic page count"))?[0])? as u64
                > pages
        {
            return Err(Error::Budget("semantic whole database bytes"));
        }
        b.execute(&format!("PRAGMA max_page_count={pages}"), &[])?;
        Ok(b)
    }
    fn writes(&self) -> u64 {
        self.tx.total_changes() - self.initial
    }
    fn charge_query(&mut self) -> Result<()> {
        self.queries += 1;
        if self.queries > self.l.max_queries {
            return Err(Error::Budget("semantic operation queries"));
        }
        Ok(())
    }
    fn execute(&mut self, sql: &str, args: &[SqlValue]) -> Result<()> {
        if (sql.starts_with("INSERT ") || sql.starts_with("UPDATE "))
            && self.writes() >= self.l.max_writes
        {
            return Err(Error::Budget("semantic writes before mutation"));
        }
        if let Some(tail) = sql.strip_prefix("DELETE FROM ") {
            let remaining = self.l.max_writes - self.writes();
            let mut probe = args.to_vec();
            probe.push(iv(next_bound(remaining)?)?);
            let found = self.query(&format!("SELECT 1 FROM {tail} LIMIT ?"), &probe)?;
            if found.len() as u64 > remaining {
                return Err(Error::Budget("semantic writes before deletion"));
            }
        }
        self.charge_query()?;
        if sql.starts_with("PRAGMA ") {
            let mut statement = self.tx.prepare(sql)?;
            let mut cursor = statement.query(rusqlite::params_from_iter(args))?;
            while cursor.next()?.is_some() {}
        } else {
            self.tx.execute(sql, rusqlite::params_from_iter(args))?;
        }
        if self.writes() > self.l.max_writes {
            return Err(Error::Budget(
                "semantic operation writes; rollback required",
            ));
        }
        Ok(())
    }
    fn statement(&mut self, sql: &str) -> Result<Statement<'a>> {
        self.charge_query()?;
        let tx = self.tx;
        Ok(tx.prepare(sql)?)
    }
    fn admit_row(&mut self, row: &Row<'_>) -> Result<Vec<SqlValue>> {
        self.rows += 1;
        if self.rows > self.l.max_rows {
            return Err(Error::Budget("semantic operation rows"));
        }
        let mut values = Vec::with_capacity(row.as_ref().column_count());
        for column in 0..row.as_ref().column_count() {
            let value: SqlValue = row.get(column)?;
            self.read = self
                .read
                .checked_add(match &value {
                    SqlValue::Text(v) => v.len(),
                    SqlValue::Blob(v) => v.len(),
                    _ => 0,
                })
                .ok_or(Error::Budget("semantic read bytes"))?;
            if self.read > self.l.max_read_bytes {
                return Err(Error::Budget("semantic operation read bytes"));
            }
            values.push(value)
        }
        Ok(values)
    }
    fn query(&mut self, sql: &str, args: &[SqlValue]) -> Result<Vec<Vec<SqlValue>>> {
        self.charge_query()?;
        let mut stmt = self.tx.prepare(sql)?;
        let columns = stmt.column_count();
        let mut cursor = stmt.query(rusqlite::params_from_iter(args))?;
        let mut out = Vec::new();
        while let Some(row) = cursor.next()? {
            self.rows += 1;
            if self.rows > self.l.max_rows {
                return Err(Error::Budget("semantic operation rows"));
            }
            let mut values = Vec::with_capacity(columns);
            for c in 0..columns {
                let value: SqlValue = row.get(c)?;
                self.read = self
                    .read
                    .checked_add(match &value {
                        SqlValue::Text(s) => s.len(),
                        SqlValue::Blob(s) => s.len(),
                        _ => 0,
                    })
                    .ok_or(Error::Budget("semantic read bytes"))?;
                if self.read > self.l.max_read_bytes {
                    return Err(Error::Budget("semantic operation read bytes"));
                }
                values.push(value)
            }
            out.push(values)
        }
        Ok(out)
    }
    fn one(&mut self, sql: &str, args: &[SqlValue]) -> Result<Option<Vec<SqlValue>>> {
        // Preserve Python next(cursor): only the first row is charged.
        self.charge_query()?;
        let mut stmt = self.tx.prepare(sql)?;
        let columns = stmt.column_count();
        let mut cursor = stmt.query(rusqlite::params_from_iter(args))?;
        let Some(row) = cursor.next()? else {
            return Ok(None);
        };
        self.rows += 1;
        if self.rows > self.l.max_rows {
            return Err(Error::Budget("semantic operation rows"));
        }
        let mut values = Vec::new();
        for c in 0..columns {
            let value: SqlValue = row.get(c)?;
            self.read = self
                .read
                .checked_add(match &value {
                    SqlValue::Text(s) => s.len(),
                    SqlValue::Blob(s) => s.len(),
                    _ => 0,
                })
                .ok_or(Error::Budget("semantic read bytes"))?;
            if self.read > self.l.max_read_bytes {
                return Err(Error::Budget("semantic read bytes"));
            }
            values.push(value)
        }
        Ok(Some(values))
    }
    fn input_json(&mut self, value: &JsonValue) -> Result<()> {
        fn visit(b: &mut Budget<'_, '_>, v: &JsonValue, depth: usize) -> Result<()> {
            b.input_values = b
                .input_values
                .checked_add(1)
                .ok_or(Error::Budget("semantic input visits"))?;
            if b.input_values > b.l.max_input_values || depth > 64 {
                return Err(Error::Budget("semantic input values/depth"));
            }
            let add = match v {
                JsonValue::String(s) => s
                    .as_str()
                    .ok_or(Error::Invalid("semantic JSON Unicode string"))?
                    .chars()
                    .count()
                    .checked_mul(6)
                    .and_then(|n| n.checked_add(2))
                    .ok_or(Error::Budget("semantic input bytes"))?,
                JsonValue::Array(a) => a
                    .len()
                    .checked_add(2)
                    .ok_or(Error::Budget("semantic input bytes"))?,
                JsonValue::Object(o) => o
                    .len()
                    .checked_mul(2)
                    .and_then(|n| n.checked_add(2))
                    .ok_or(Error::Budget("semantic input bytes"))?,
                _ => encode(v, b.l.max_input_bytes)?.len(),
            };
            b.input_bytes = b
                .input_bytes
                .checked_add(add)
                .ok_or(Error::Budget("semantic input bytes"))?;
            match v {
                JsonValue::Array(a) => {
                    for child in a {
                        visit(b, child, depth + 1)?
                    }
                }
                JsonValue::Object(o) => {
                    for (key, child) in o {
                        visit(b, &JsonValue::String(key.clone()), depth + 1)?;
                        visit(b, child, depth + 1)?
                    }
                }
                _ => {}
            }
            if b.input_bytes > b.l.max_input_bytes {
                return Err(Error::Budget("semantic input bytes"));
            }
            Ok(())
        }
        visit(self, value, 0)
    }
    fn text(
        &mut self,
        table: &str,
        column: &str,
        predicate: &str,
        args: &[SqlValue],
        cap: usize,
    ) -> Result<Option<String>> {
        let Some(size) = self.one(
            &format!("SELECT length(CAST({column} AS BLOB)) FROM {table} WHERE {predicate}"),
            args,
        )?
        else {
            return Ok(None);
        };
        let n = int(&size[0])?;
        if n < 0
            || n as usize > cap
            || self
                .read
                .checked_add(n as usize)
                .is_none_or(|n| n > self.l.max_read_bytes)
        {
            return Err(Error::Budget("semantic stored payload bytes"));
        }
        let row = self
            .one(
                &format!("SELECT {column} FROM {table} WHERE {predicate}"),
                args,
            )?
            .ok_or(Error::Invalid("semantic payload disappeared"))?;
        Ok(Some(st(&row[0])?.to_owned()))
    }
    fn diagnostic(&mut self, v: &Value) -> Result<()> {
        self.output_items += 1;
        self.output_bytes = self
            .output_bytes
            .checked_add(compact(v, self.l.max_output_bytes)?.len())
            .ok_or(Error::Budget("semantic kernel output bytes"))?;
        if self.output_items > self.l.max_output_items
            || self.output_bytes > self.l.max_output_bytes
        {
            return Err(Error::Budget("semantic kernel output"));
        }
        Ok(())
    }
    fn metadata(&mut self, key: &str) -> Result<Value> {
        let args = [sv(key)];
        let rows=self.query("SELECT part,length(CAST(json_chunk AS BLOB)) FROM edge_meta WHERE key=? ORDER BY part LIMIT 257",&args)?;
        if rows.is_empty() || rows.len() > 256 {
            return Err(Error::Invalid("semantic exact metadata parts"));
        }
        let mut total = 0usize;
        for (i, r) in rows.iter().enumerate() {
            if int(&r[0])? != i as i64 {
                return Err(Error::Invalid("semantic metadata part order"));
            }
            let n = int(&r[1])?;
            if !(0..=131072).contains(&n) {
                return Err(Error::Budget("semantic metadata chunk"));
            }
            total = total
                .checked_add(n as usize)
                .ok_or(Error::Budget("semantic metadata bytes"))?;
        }
        if total > self.l.max_output_bytes
            || self
                .read
                .checked_add(total)
                .is_none_or(|n| n > self.l.max_read_bytes)
        {
            return Err(Error::Budget("semantic metadata bytes"));
        }
        let mut raw = String::with_capacity(total);
        let semantic_stream_args = &args;
        let mut semantic_stream_statement =
            self.statement("SELECT json_chunk FROM edge_meta WHERE key=? ORDER BY part LIMIT 257")?;
        let mut semantic_stream_cursor = semantic_stream_statement
            .query(rusqlite::params_from_iter(semantic_stream_args.iter()))?;
        while let Some(semantic_stream_row) = semantic_stream_cursor.next()? {
            let row = self.admit_row(semantic_stream_row)?;
            raw.push_str(st(&row[0])?)
        }
        let v = decode(&raw)?;
        if encode(
            &parse(&raw, self.l.max_output_bytes)?,
            self.l.max_output_bytes,
        )? != raw
        {
            return Err(Error::Invalid("semantic metadata framing"));
        }
        if key == "knowledge_reader_top" {
            self.meta_raw.insert(key.into(), raw);
        }
        Ok(v)
    }
    fn metadata_raw(&self, key: &str) -> Result<String> {
        self.meta_raw
            .get(key)
            .cloned()
            .ok_or(Error::Invalid("semantic metadata exact raw absent"))
    }
    fn binding(&mut self, expected: &Value) -> Result<Value> {
        if !expected
            .get("publication_epoch")
            .and_then(Value::as_u64)
            .is_some_and(|n| n <= MAX_ADDRESS)
        {
            return Err(Error::Invalid(
                "semantic selected binding integer epoch ABI",
            ));
        }
        let top = self.metadata("knowledge_reader_top")?;
        validate_top(
            &top,
            &self.metadata_raw("knowledge_reader_top")?,
            self.l.max_output_bytes,
        )?;
        let row = self
            .one(
                "SELECT epoch FROM knowledge_exploration_clock WHERE singleton=1",
                &[],
            )?
            .ok_or(Error::Invalid("semantic publication epoch"))?;
        let epoch = int(&row[0])?;
        if epoch < 0 || epoch as u64 > MAX_ADDRESS || top["read_model_schema"] != SCHEMA {
            return Err(Error::Invalid("semantic prepared schema/epoch"));
        }
        let mut out = json!({"schema":"tos_published_knowledge_snapshot_v1","publication_epoch":epoch,"metadata_sha256":sha(&self.metadata_raw("knowledge_reader_top")?)});
        for key in [
            "read_model_schema",
            "source_revision",
            "data_revision",
            "graph_schema",
            "normalization_binding",
        ] {
            out.as_object_mut().unwrap().insert(
                key.into(),
                top.get(key)
                    .ok_or(Error::Invalid("semantic binding field"))?
                    .clone(),
            );
        }
        if !python_eq(&out, expected)?
            || self.metadata("data_revision")? != json!({"sha256":top["data_revision"]})
        {
            return Err(Error::Invalid("semantic stale/foreign binding"));
        }
        Ok(out)
    }
    fn stored(&mut self, kind: &str, id: &str) -> Result<Option<(Value, JsonValue, String)>> {
        let Some(raw) = self.text(
            &format!("knowledge_{kind}s"),
            "json",
            "id=?",
            &[sv(id)],
            self.l.max_row_bytes,
        )?
        else {
            return Ok(None);
        };
        let digest = sha(&raw);
        if self.metadata(&format!("knowledge_{kind}_digest:{id}"))? != json!({"sha256":digest}) {
            return Err(Error::Invalid("semantic exact row digest"));
        }
        let item = decode(&raw)?;
        if !item.is_object()
            || item["id"] != id
            || encode(&parse(&raw, self.l.max_row_bytes)?, self.l.max_row_bytes)? != raw
            || id.is_empty()
            || canonical(id)? != id
        {
            return Err(Error::Invalid("semantic row framing/canonical identity"));
        }
        Ok(Some((item, parse(&raw, self.l.max_row_bytes)?, digest)))
    }
    fn state(&mut self) -> Result<Value> {
        decode(
            &self
                .text(
                    "semantic_state",
                    "json",
                    "singleton=1",
                    &[],
                    self.l.max_output_bytes,
                )?
                .ok_or(Error::Invalid(
                    "semantic absent; explicit bootstrap required",
                ))?,
        )
    }
    fn put_state(&mut self, state: &Value) -> Result<()> {
        let raw = compact(state, self.l.max_output_bytes)?;
        self.execute("INSERT INTO semantic_state VALUES (1,?) ON CONFLICT(singleton) DO UPDATE SET json=excluded.json",&[sv(&raw)])
    }
}
fn dependencies(
    b: &mut Budget<'_, '_>,
    binding: &Value,
    entity: &Value,
    relation: &Value,
    processor: &str,
) -> Result<Value> {
    let n = &binding["normalization_binding"];
    if !valid_hex(processor)
        || n["processor_digest"] != processor
        || n["entity_registry_digest"] != stable_digest(entity)?
        || n["relation_registry_digest"] != stable_digest(relation)?
    {
        return Err(Error::Invalid(
            "semantic normalization/registry drift; explicit bootstrap required",
        ));
    }
    Ok(json!({"version":VERSION,"normalization":n,"projector":semantic_projector_sha256()}))
}
const DDL: [&str; 13] = [
    "CREATE TABLE semantic_state(singleton INTEGER PRIMARY KEY CHECK(singleton=1),json TEXT NOT NULL)",
    "CREATE TABLE semantic_rows(kind TEXT NOT NULL,id TEXT NOT NULL,source_order INTEGER NOT NULL,digest TEXT NOT NULL,type_id TEXT,entity TEXT,counts TEXT NOT NULL,PRIMARY KEY(kind,id),UNIQUE(kind,source_order))",
    "CREATE INDEX semantic_claim_winner ON semantic_rows(kind,type_id,entity,source_order DESC)",
    "CREATE TABLE semantic_edges(id TEXT PRIMARY KEY,from_id TEXT NOT NULL,to_id TEXT NOT NULL,relation_type TEXT NOT NULL,scope TEXT NOT NULL)",
    "CREATE INDEX semantic_outgoing ON semantic_edges(from_id,relation_type,id)",
    "CREATE TABLE semantic_deps(kind TEXT NOT NULL,id TEXT NOT NULL,dependency_kind TEXT NOT NULL,dependency_id TEXT NOT NULL,PRIMARY KEY(kind,id,dependency_kind,dependency_id)) WITHOUT ROWID",
    "CREATE INDEX semantic_dependents ON semantic_deps(dependency_kind,dependency_id,kind,id)",
    "CREATE TABLE semantic_cardinality(axis TEXT NOT NULL,endpoint TEXT NOT NULL,relation_type TEXT NOT NULL,scope TEXT NOT NULL,n INTEGER NOT NULL,PRIMARY KEY(axis,endpoint,relation_type,scope)) WITHOUT ROWID",
    "CREATE INDEX semantic_cardinality_peak ON semantic_cardinality(axis,endpoint,relation_type,n DESC)",
    "CREATE TABLE semantic_cardinality_errors(axis TEXT NOT NULL,endpoint TEXT NOT NULL,relation_type TEXT NOT NULL,error TEXT NOT NULL,PRIMARY KEY(axis,endpoint,relation_type)) WITHOUT ROWID",
    "CREATE TABLE semantic_diagnostics(kind TEXT NOT NULL,id TEXT NOT NULL,source_order INTEGER NOT NULL,errors TEXT NOT NULL,gaps TEXT NOT NULL,PRIMARY KEY(kind,id)) WITHOUT ROWID",
    "CREATE INDEX semantic_diagnostic_order ON semantic_diagnostics(kind,source_order)",
    "CREATE TABLE semantic_pending(kind TEXT NOT NULL,id TEXT NOT NULL,digest TEXT,source_order INTEGER,PRIMARY KEY(kind,id)) WITHOUT ROWID",
];
type Key = (String, String);
type Check = (Option<String>, Option<i64>);
struct Context<'a, 't, 'r> {
    b: Budget<'a, 't>,
    kernel: Rc<Kernel<'r>>,
    overlay: BTreeMap<Key, Option<Value>>,
    deps: BTreeSet<Key>,
    checked: BTreeMap<Key, Check>,
    exact: BTreeMap<Key, JsonValue>,
}
impl Context<'_, '_, '_> {
    fn row(&mut self, kind: &str, id: &str) -> Result<Option<Value>> {
        let key = (kind.into(), id.into());
        if let Some(v) = self.overlay.get(&key) {
            return Ok(v.clone());
        }
        let stored = self.b.stored(kind, id)?;
        let args = [sv(kind), sv(id)];
        let indexed = self.b.one(
            "SELECT digest,source_order FROM semantic_rows WHERE kind=? AND id=?",
            &args,
        )?;
        let prepared = self.b.one(
            "SELECT source_order FROM prepared_documents WHERE kind=? AND id=?",
            &args,
        )?;
        let Some((item, original, digest)) = stored else {
            if indexed.is_some() || prepared.is_some() {
                return Err(Error::Invalid("semantic carrier disappeared"));
            }
            self.checked.insert(key, (None, None));
            return Ok(None);
        };
        let indexed = indexed.ok_or(Error::Invalid("semantic row index absent"))?;
        let prepared = prepared.ok_or(Error::Invalid("semantic prepared row absent"))?;
        if st(&indexed[0])? != digest || int(&prepared[0])? != int(&indexed[1])? {
            return Err(Error::Invalid("semantic row dependency drift"));
        }
        self.exact.insert(key.clone(), original);
        self.checked
            .insert(key, (Some(digest), Some(int(&indexed[1])?)));
        Ok(Some(item))
    }
    fn carrier(
        &mut self,
        kind: &str,
        id: &str,
        view: Option<Value>,
    ) -> Result<Option<SemanticCarrier>> {
        let Some(view) = view else { return Ok(None) };
        let key = (kind.into(), id.into());
        let original = if self.overlay.contains_key(&key) {
            self.exact.get(&key).cloned()
        } else {
            self.exact.remove(&key)
        }
        .ok_or(Error::Invalid("semantic exact looked-up carrier absent"))?;
        Ok(Some(SemanticCarrier { view, original }))
    }
    fn add(&mut self, kind: &str, item: &Value, order: u64, digest: &str) -> Result<()> {
        if order > MAX_ADDRESS {
            return Err(Error::Invalid("semantic explicit source order"));
        }
        let id = item["id"]
            .as_str()
            .ok_or(Error::Invalid("semantic identity"))?;
        let counts = self.kernel.summarize(kind, item);
        let type_id = if kind == "node" {
            sql_scalar(&item["type_id"])?
        } else {
            SqlValue::Null
        };
        let entity = if kind == "node" {
            sv(&exact_text(
                self.exact
                    .get(&(kind.into(), id.into()))
                    .ok_or(Error::Invalid("semantic exact carrier absent"))?,
                &["entity_id"],
            )?)
        } else {
            SqlValue::Null
        };
        self.b.execute(
            "INSERT INTO semantic_rows VALUES (?,?,?,?,?,?,?)",
            &[
                sv(kind),
                sv(id),
                iv(order)?,
                sv(digest),
                type_id,
                entity,
                sv(&compact(&json!(counts), self.b.l.max_output_bytes)?),
            ],
        )?;
        if kind == "relation" {
            let ty = semantic_string(&item["relation_type_id"])?.unwrap_or("");
            if let Some(entry) = self.kernel.relation_entry(ty) {
                if entry["assertion_mode"] == "reified-claim"
                    && item
                        .get("attributes")
                        .is_some_and(|v| truthy(v) && !v.is_object())
                {
                    return Err(Error::Invalid("semantic attributes object required"));
                }
                let scope = scope(
                    if entry["assertion_mode"] == "reified-claim" {
                        &item["attributes"]["claim_ref"]
                    } else {
                        &Value::Null
                    },
                    self.b.l.max_row_bytes,
                )?;
                self.b.execute(
                    "INSERT INTO semantic_edges VALUES (?,?,?,?,?)",
                    &[
                        sv(id),
                        sv(&exact_text(
                            self.exact
                                .get(&(kind.into(), id.into()))
                                .ok_or(Error::Invalid("semantic exact carrier absent"))?,
                            &["from_id"],
                        )?),
                        sv(&exact_text(
                            self.exact
                                .get(&(kind.into(), id.into()))
                                .ok_or(Error::Invalid("semantic exact carrier absent"))?,
                            &["to_id"],
                        )?),
                        sv(ty),
                        sv(&scope),
                    ],
                )?;
                self.adjust(id, 1)?
            }
        }
        Ok(())
    }
    fn adjust(&mut self, id: &str, adjustment: i64) -> Result<()> {
        let Some(edge) = self.b.one(
            "SELECT from_id,to_id,relation_type,scope FROM semantic_edges WHERE id=?",
            &[sv(id)],
        )?
        else {
            return Ok(());
        };
        for (axis, endpoint) in [("per_subject_max", &edge[0]), ("per_object_max", &edge[1])] {
            let key = [sv(axis), endpoint.clone(), edge[2].clone(), edge[3].clone()];
            let found=self.b.one("SELECT n FROM semantic_cardinality WHERE axis=? AND endpoint=? AND relation_type=? AND scope=?",&key)?;
            let n = found.as_ref().map(|r| int(&r[0])).transpose()?.unwrap_or(0) + adjustment;
            if n < 0 {
                return Err(Error::Invalid("semantic cardinality drift"));
            }
            if n != 0 {
                let mut args = key.to_vec();
                args.push(SqlValue::Integer(n));
                self.b.execute("INSERT INTO semantic_cardinality VALUES (?,?,?,?,?) ON CONFLICT(axis,endpoint,relation_type,scope) DO UPDATE SET n=excluded.n",&args)?
            } else {
                self.b.execute("DELETE FROM semantic_cardinality WHERE axis=? AND endpoint=? AND relation_type=? AND scope=?",&key)?
            }
            let group = &key[..3];
            let peak=self.b.one("SELECT n FROM semantic_cardinality INDEXED BY semantic_cardinality_peak WHERE axis=? AND endpoint=? AND relation_type=? ORDER BY n DESC LIMIT 1",group)?.map(|r|int(&r[0])).transpose()?.unwrap_or(0);
            let entry = self
                .kernel
                .relation_entry(st(&edge[2])?)
                .ok_or(Error::Invalid("semantic indexed relation missing"))?;
            if entry.get("cardinality").is_some_and(|v| !v.is_object()) {
                return Err(Error::Invalid("semantic cardinality object required"));
            }
            let maximum = &entry["cardinality"][axis];
            if numeric_exceeded(peak, maximum)? {
                let error = format!(
                    "{} violates {} {axis}={}",
                    st(endpoint)?,
                    st(&edge[2])?,
                    crate::local_prepared::python_value_string(&parse(
                        &compact(maximum, self.b.l.max_row_bytes)?,
                        self.b.l.max_row_bytes
                    )?)?
                );
                let mut args = group.to_vec();
                args.push(sv(&error));
                self.b.execute("INSERT INTO semantic_cardinality_errors VALUES (?,?,?,?) ON CONFLICT(axis,endpoint,relation_type) DO UPDATE SET error=excluded.error",&args)?
            } else {
                self.b.execute("DELETE FROM semantic_cardinality_errors WHERE axis=? AND endpoint=? AND relation_type=?",group)?
            }
        }
        Ok(())
    }
    fn evaluate(&mut self, kind: &str, id: &str) -> Result<[i64; 6]> {
        let Some(item) = self.row(kind, id)? else {
            return Ok([0; 6]);
        };
        self.deps.clear();
        let mut counts = self.kernel.summarize(kind, &item);
        let kernel = self.kernel.clone();
        let original = self
            .exact
            .get(&(kind.into(), id.into()))
            .ok_or(Error::Invalid("semantic exact carrier absent"))?
            .clone();
        let (errors, gaps, claims) = kernel.evaluate(kind, &item, &original, self)?;
        counts[4] = claims;
        self.b.execute(
            "DELETE FROM semantic_deps WHERE kind=? AND id=?",
            &[sv(kind), sv(id)],
        )?;
        for (dk, di) in &self.deps {
            self.b.execute(
                "INSERT INTO semantic_deps VALUES (?,?,?,?)",
                &[sv(kind), sv(id), sv(dk), sv(di)],
            )?
        }
        self.b.execute(
            "DELETE FROM semantic_diagnostics WHERE kind=? AND id=?",
            &[sv(kind), sv(id)],
        )?;
        if !errors.is_empty() || !gaps.is_empty() {
            let er = compact(&json!(errors), self.b.l.max_output_bytes)?;
            let ga = compact(&json!(gaps), self.b.l.max_output_bytes)?;
            if er.len() + ga.len() > self.b.l.max_output_bytes {
                return Err(Error::Budget("semantic diagnostic output bytes"));
            }
            let order = self
                .b
                .one(
                    "SELECT source_order FROM semantic_rows WHERE kind=? AND id=?",
                    &[sv(kind), sv(id)],
                )?
                .ok_or(Error::Invalid("semantic diagnostics row"))?[0]
                .clone();
            self.b.execute(
                "INSERT INTO semantic_diagnostics VALUES (?,?,?,?,?)",
                &[sv(kind), sv(id), order, sv(&er), sv(&ga)],
            )?
        }
        self.b.execute(
            "UPDATE semantic_rows SET counts=? WHERE kind=? AND id=?",
            &[
                sv(&compact(&json!(counts), self.b.l.max_output_bytes)?),
                sv(kind),
                sv(id),
            ],
        )?;
        self.exact.retain(|key, _| self.overlay.contains_key(key));
        Ok(counts)
    }
}
impl SemanticLookup for Context<'_, '_, '_> {
    fn node(&mut self, id: &str) -> Result<Option<SemanticCarrier>> {
        self.deps.insert(("node".into(), id.into()));
        let result = self.row("node", id)?;
        self.carrier("node", id, result)
    }
    fn claim(&mut self, id: &str) -> Result<Option<SemanticCarrier>> {
        self.deps.insert(("claim".into(), id.into()));
        let row=self.b.one("SELECT id FROM semantic_rows INDEXED BY semantic_claim_winner WHERE kind='node' AND type_id='tos.entity.claim' AND entity=? ORDER BY source_order DESC LIMIT 1",&[sv(id)])?;
        if let Some(row) = row {
            let id = st(&row[0])?;
            let result = self.row("node", id)?;
            self.carrier("node", id, result)
        } else {
            Ok(None)
        }
    }
    fn outgoing(&mut self, id: &str, predicate: &str) -> Result<Vec<Value>> {
        self.deps.insert((
            "outgoing".into(),
            compact(&json!([id, predicate]), self.b.l.max_row_bytes)?,
        ));
        let rows=self.b.query("SELECT to_id FROM semantic_edges INDEXED BY semantic_outgoing WHERE from_id=? AND relation_type=? ORDER BY id LIMIT ?",&[sv(id),sv(predicate),iv(next_bound(self.b.l.max_rows)?)?])?;
        rows.into_iter()
            .map(|r| Ok(json!({"to_id":st(&r[0])?})))
            .collect()
    }
    fn diagnostic(&mut self, v: &Value) -> Result<()> {
        self.b.diagnostic(v)
    }
}
fn scope(v: &Value, cap: usize) -> Result<String> {
    match v {
        Value::Null => compact(&json!(["none", null]), cap),
        Value::String(s) => compact(&json!(["string", s]), cap),
        Value::Bool(b) => compact(&json!(["number", if *b { "1" } else { "0" }]), cap),
        Value::Number(n) => {
            let lexeme = n.to_string();
            if !lexeme.contains(['.', 'e', 'E']) {
                return compact(
                    &json!([
                        "number",
                        if lexeme == "-0" {
                            "0".to_owned()
                        } else {
                            lexeme
                        }
                    ]),
                    cap,
                );
            }
            if let Some(n) = n.as_i64() {
                return compact(&json!(["number", n.to_string()]), cap);
            }
            if let Some(n) = n.as_u64() {
                return compact(&json!(["number", n.to_string()]), cap);
            }
            let f = n
                .as_f64()
                .filter(|n| n.is_finite())
                .ok_or(Error::Invalid("semantic scope finite number"))?;
            let repr = if f.fract() == 0.0 {
                if f == 0.0 {
                    "0".into()
                } else {
                    format!("{f:.0}")
                }
            } else {
                python_float_hex(f)
            };
            compact(&json!(["number", repr]), cap)
        }
        _ => Err(Error::Invalid("semantic unhashable cardinality scope")),
    }
}
fn python_float_hex(f: f64) -> String {
    let bits = f.to_bits();
    let sign = if bits >> 63 != 0 { "-" } else { "" };
    let exponent = ((bits >> 52) & 2047) as i32;
    let fraction = bits & 0xfffffffffffff;
    let (lead, power) = if exponent == 0 {
        (0, -1022)
    } else {
        (1, exponent - 1023)
    };
    format!("{sign}0x{lead}.{fraction:013x}p{power:+}")
}
fn add_counts(total: &mut [i64; 6], value: [i64; 6], sign: i64) -> Result<()> {
    for (i, v) in value.into_iter().enumerate() {
        total[i] = total[i]
            .checked_add(
                v.checked_mul(sign)
                    .ok_or(Error::Budget("semantic count overflow"))?,
            )
            .ok_or(Error::Budget("semantic count overflow"))?
    }
    Ok(())
}
fn counts(v: &Value) -> Result<[i64; 6]> {
    let a = v
        .as_array()
        .filter(|a| a.len() == 6)
        .ok_or(Error::Invalid("semantic state counts"))?;
    let mut out = [0; 6];
    for (i, v) in a.iter().enumerate() {
        out[i] = v.as_i64().ok_or(Error::Invalid("semantic state count"))?
    }
    Ok(out)
}
fn report(b: &mut Budget<'_, '_>, counts: [i64; 6], registry: &[Value]) -> Result<Value> {
    let mut violations = registry.to_vec();
    let mut gaps = Vec::new();
    let mut bytes = compact(&json!(registry), b.l.max_output_bytes)?.len();
    let mut items = violations.len();
    if items > b.l.max_output_items {
        return Err(Error::Budget("semantic registry report items"));
    }
    for kind in ["relation", "node"] {
        let semantic_stream_args = &[sv(kind), iv(next_bound(b.l.max_output_items as u64)?)?];
        let mut semantic_stream_statement=b.statement("SELECT id,length(CAST(errors AS BLOB)),length(CAST(gaps AS BLOB)) FROM semantic_diagnostics INDEXED BY semantic_diagnostic_order WHERE kind=? ORDER BY source_order LIMIT ?")?;
        let mut semantic_stream_cursor = semantic_stream_statement
            .query(rusqlite::params_from_iter(semantic_stream_args.iter()))?;
        while let Some(semantic_stream_row) = semantic_stream_cursor.next()? {
            let r = b.admit_row(semantic_stream_row)?;
            bytes = bytes
                .checked_add(int(&r[1])? as usize + int(&r[2])? as usize)
                .ok_or(Error::Budget("semantic report bytes"))?;
            if bytes > b.l.max_output_bytes {
                return Err(Error::Budget("semantic report bytes"));
            }
            let row = b
                .one(
                    "SELECT errors,gaps FROM semantic_diagnostics WHERE kind=? AND id=?",
                    &[sv(kind), r[0].clone()],
                )?
                .ok_or(Error::Invalid("semantic diagnostics absent"))?;
            let e = decode(st(&row[0])?)?;
            let g = decode(st(&row[1])?)?;
            let e = e.as_array().ok_or(Error::Invalid("semantic errors"))?;
            let g = g.as_array().ok_or(Error::Invalid("semantic gaps"))?;
            items += e.len() + g.len();
            if items > b.l.max_output_items {
                return Err(Error::Budget("semantic report items"));
            }
            violations.extend(e.iter().cloned());
            gaps.extend(g.iter().cloned());
        }
    }
    let semantic_stream_args = &[iv(next_bound(b.l.max_output_items as u64)?)?];
    let mut semantic_stream_statement =
        b.statement("SELECT error FROM semantic_cardinality_errors LIMIT ?")?;
    let mut semantic_stream_cursor =
        semantic_stream_statement.query(rusqlite::params_from_iter(semantic_stream_args.iter()))?;
    while let Some(semantic_stream_row) = semantic_stream_cursor.next()? {
        let r = b.admit_row(semantic_stream_row)?;
        let e = st(&r[0])?;
        items += 1;
        bytes += e.len();
        if items > b.l.max_output_items || bytes > b.l.max_output_bytes {
            return Err(Error::Budget("semantic report output"));
        }
        violations.push(json!(e))
    }
    let sorted: BTreeSet<String> = violations
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .ok_or(Error::Invalid("semantic violation text"))
        })
        .collect::<Result<_>>()?;
    let mut result = json!({"valid":violations.is_empty(),"violations":sorted});
    for (i, key) in COUNTS.into_iter().enumerate() {
        result
            .as_object_mut()
            .unwrap()
            .insert(key.into(), json!(counts[i]));
    }
    result
        .as_object_mut()
        .unwrap()
        .insert("gaps".into(), json!(gaps));
    compact(&result, b.l.max_output_bytes)?;
    Ok(result)
}
fn descriptor(b: &mut Budget<'_, '_>, binding: &Value, digest: &str) -> Result<Value> {
    let raw = b
        .text(
            "prepared_state",
            "descriptor",
            "singleton=1",
            &[],
            b.l.max_input_bytes,
        )?
        .ok_or(Error::Invalid("semantic prepared descriptor absent"))?;
    if binding["data_revision"] != sha(&raw) {
        return Err(Error::Invalid("semantic prepared descriptor digest"));
    }
    let d = decode(&raw)?;
    let header = &d["header"];
    if header["source_revision"] != binding["source_revision"]
        || header["normalization_binding"] != binding["normalization_binding"]
        || sha(&encode(
            jfield(
                &parse(&raw, b.l.max_input_bytes)?,
                &["header", "counts", "semantic_validation"],
            )?,
            b.l.max_output_bytes,
        )?) != digest
    {
        return Err(Error::Invalid(
            "semantic computed report differs from final header",
        ));
    }
    Ok(d)
}
/// Compute complete semantics from exact existing prepared bytes. Existing
/// semantic tables are refused by plain CREATE, never replaced.
pub fn bootstrap_semantic_index_transaction(
    tx: &Transaction<'_>,
    binding: &JsonValue,
    entity: &JsonValue,
    relation: &JsonValue,
    mut ordered_rows: Option<&mut dyn SemanticRows>,
    processor: &str,
    limits: SemanticMaintenanceLimits,
) -> Result<JsonValue> {
    let mut b = Budget::new(tx, limits)?;
    b.input_json(binding)?;
    let binding = view(binding, limits.max_input_bytes)?;
    let binding = b.binding(&binding)?;
    let entity_original = entity;
    let relation_original = relation;
    b.input_json(entity)?;
    b.input_json(relation)?;
    let entity = view(entity, limits.max_input_bytes)?;
    let relation = view(relation, limits.max_input_bytes)?;
    let dep = dependencies(&mut b, &binding, &entity, &relation, processor)?;
    let kernel = Rc::new(Kernel::new(
        &entity,
        &relation,
        &entity_original,
        &relation_original,
    )?);
    let registry = kernel.registry_violations.clone();
    for sql in DDL {
        b.execute(sql, &[])?
    }
    let mut c = Context {
        b,
        kernel,
        overlay: BTreeMap::new(),
        deps: BTreeSet::new(),
        checked: BTreeMap::new(),
        exact: BTreeMap::new(),
    };
    for kind in ["node", "relation"] {
        let mut last = -1i64;
        if let Some(rows) = ordered_rows.as_deref_mut() {
            rows.visit(kind, &mut |original| {
                c.b.input_json(original)?;
                let raw = encode(original, c.b.l.max_row_bytes)?;
                let item = decode(&raw)?;
                let id = item
                    .get("id")
                    .and_then(Value::as_str)
                    .ok_or(Error::Invalid("semantic exact supplied carrier"))?;
                let found =
                    c.b.one(
                        "SELECT source_order FROM prepared_documents WHERE kind=? AND id=?",
                        &[sv(kind), sv(id)],
                    )?
                    .ok_or(Error::Invalid("semantic foreign supplied carrier"))?;
                bootstrap_add(&mut c, kind, id, int(&found[0])?, Some(&raw), &mut last)
            })?;
        } else {
            let mut ids = c.b.query(
                "SELECT id,source_order FROM prepared_documents WHERE kind=? ORDER BY id LIMIT ?",
                &[sv(kind), iv(next_bound(c.b.l.max_rows)?)?],
            )?;
            ids.sort_by_key(|r| {
                if let SqlValue::Integer(n) = r[1] {
                    n
                } else {
                    i64::MIN
                }
            });
            for row in ids {
                bootstrap_add(&mut c, kind, st(&row[0])?, int(&row[1])?, None, &mut last)?;
            }
        }
        let semantic_stream_args = &[sv(kind), iv(next_bound(c.b.l.max_rows)?)?];
        let mut semantic_stream_statement =
            c.b.statement("SELECT id FROM prepared_documents WHERE kind=? ORDER BY id LIMIT ?")?;
        let mut semantic_stream_cursor = semantic_stream_statement
            .query(rusqlite::params_from_iter(semantic_stream_args.iter()))?;
        while let Some(semantic_stream_row) = semantic_stream_cursor.next()? {
            let row = c.b.admit_row(semantic_stream_row)?;
            if c.b
                .one(
                    "SELECT 1 FROM semantic_rows WHERE kind=? AND id=?",
                    &[sv(kind), row[0].clone()],
                )?
                .is_none()
            {
                return Err(Error::Invalid("semantic bootstrap omitted carrier"));
            }
        }
        let semantic_stream_args = &[iv(next_bound(c.b.l.max_rows)?)?];
        let mut semantic_stream_statement = c.b.statement(&format!(
            "SELECT id FROM knowledge_{kind}s ORDER BY id LIMIT ?"
        ))?;
        let mut semantic_stream_cursor = semantic_stream_statement
            .query(rusqlite::params_from_iter(semantic_stream_args.iter()))?;
        while let Some(semantic_stream_row) = semantic_stream_cursor.next()? {
            let row = c.b.admit_row(semantic_stream_row)?;
            if c.b
                .one(
                    "SELECT 1 FROM semantic_rows WHERE kind=? AND id=?",
                    &[sv(kind), row[0].clone()],
                )?
                .is_none()
            {
                return Err(Error::Invalid("semantic carrier absent from prepared map"));
            }
        }
    }
    let mut counts = [0; 6];
    for kind in ["node", "relation"] {
        let semantic_stream_args = &[sv(kind), iv(next_bound(c.b.l.max_rows)?)?];
        let mut semantic_stream_statement =
            c.b.statement("SELECT id FROM semantic_rows WHERE kind=? ORDER BY id LIMIT ?")?;
        let mut semantic_stream_cursor = semantic_stream_statement
            .query(rusqlite::params_from_iter(semantic_stream_args.iter()))?;
        while let Some(semantic_stream_row) = semantic_stream_cursor.next()? {
            let row = c.b.admit_row(semantic_stream_row)?;
            add_counts(&mut counts, c.evaluate(kind, st(&row[0])?)?, 1)?
        }
    }
    let report = report(&mut c.b, counts, &registry)?;
    let digest = sha(&compact(&report, c.b.l.max_output_bytes)?);
    descriptor(&mut c.b, &binding, &digest)?;
    c.b.put_state(&json!({"dependencies":dep,"binding":binding,"pending":false,"counts":counts,"registry_violations":registry,"report_digest":digest}))?;
    parse(
        &compact(&report, limits.max_output_bytes)?,
        limits.max_output_bytes,
    )
}
/// Validate the candidate overlay before the caller publishes prepared rows.
pub fn apply_semantic_delta_transaction(
    tx: &Transaction<'_>,
    expected: &JsonValue,
    new_source_revision: &str,
    changes: &[SemanticChange],
    entity: &JsonValue,
    relation: &JsonValue,
    processor: &str,
    limits: SemanticMaintenanceLimits,
) -> Result<JsonValue> {
    let mut b = Budget::new(tx, limits)?;
    b.input_json(expected)?;
    let expected = view(expected, limits.max_input_bytes)?;
    let binding = b.binding(&expected)?;
    let mut state = b.state()?;
    let entity_original = entity;
    let relation_original = relation;
    b.input_json(entity)?;
    b.input_json(relation)?;
    let entity = view(entity, limits.max_input_bytes)?;
    let relation = view(relation, limits.max_input_bytes)?;
    let dep = dependencies(&mut b, &binding, &entity, &relation, processor)?;
    if state["pending"] != false
        || !python_eq(&state["binding"], &binding)?
        || state["dependencies"] != dep
    {
        return Err(Error::Invalid(
            "semantic pending/stale dependencies; rollback or bootstrap",
        ));
    }
    let epoch = binding["publication_epoch"]
        .as_u64()
        .ok_or(Error::Invalid("semantic epoch"))?;
    if !valid_hex(new_source_revision) || epoch >= MAX_ADDRESS {
        return Err(Error::Invalid("semantic source revision/next epoch"));
    }
    let kernel = Rc::new(Kernel::new(
        &entity,
        &relation,
        &entity_original,
        &relation_original,
    )?);
    let mut c = Context {
        b,
        kernel,
        overlay: BTreeMap::new(),
        deps: BTreeSet::new(),
        checked: BTreeMap::new(),
        exact: BTreeMap::new(),
    };
    let mut pending: Vec<(String, String, Option<Value>, Option<u64>, Option<String>)> = Vec::new();
    let mut affected = BTreeSet::new();
    let mut signals = BTreeSet::new();
    let mut frames = Vec::new();
    let mut seen = BTreeSet::new();
    for change in changes {
        if seen.len() >= limits.max_changes {
            return Err(Error::Budget("semantic delta changes"));
        }
        if !matches!(change.kind.as_str(), "node" | "relation")
            || !matches!(change.operation.as_str(), "insert" | "update" | "delete")
            || !(1..=4096).contains(&change.identifier.chars().count())
            || canonical(&change.identifier)? != change.identifier
        {
            return Err(Error::Invalid("semantic exact canonical change target"));
        }
        let key = (change.kind.clone(), change.identifier.clone());
        if !seen.insert(key.clone()) {
            return Err(Error::Invalid("semantic duplicate target"));
        }
        let found = c.b.one(
            "SELECT source_order FROM prepared_documents WHERE kind=? AND id=?",
            &[sv(&change.kind), sv(&change.identifier)],
        )?;
        if found.is_none() != (change.operation == "insert") {
            return Err(Error::Invalid("semantic operation differs from target"));
        }
        let old = c.row(&change.kind, &change.identifier)?;
        if old.is_none() != (change.operation == "insert") {
            return Err(Error::Invalid("semantic prepared map/carrier differs"));
        }
        let (item, order, digest) = if change.operation == "delete" {
            if change.item.is_some() || change.source_order.is_some() {
                return Err(Error::Invalid("semantic deletion replacement/order"));
            }
            (None, None, None)
        } else {
            let item = change
                .item
                .as_ref()
                .ok_or(Error::Invalid("semantic replacement absent"))?;
            c.b.input_json(item)?;
            let raw = encode(item, limits.max_row_bytes)?;
            let item = decode(&raw)?;
            if !item.is_object() || item["id"] != change.identifier {
                return Err(Error::Invalid("semantic replacement identity"));
            }
            let order = change
                .source_order
                .or(found
                    .as_ref()
                    .map(|r| int(&r[0]))
                    .transpose()?
                    .map(|n| n as u64))
                .ok_or(Error::Invalid("semantic explicit insertion order"))?;
            if order > MAX_ADDRESS {
                return Err(Error::Invalid("semantic order safe integer"));
            }
            (Some(decode(&raw)?), Some(order), Some(sha(&raw)))
        };
        frames.push(json!([
            change.operation,
            change.kind,
            change.identifier,
            if change.operation == "delete" {
                found
                    .as_ref()
                    .map(|r| int(&r[0]))
                    .transpose()?
                    .map(|v| v as u64)
            } else {
                order
            },
            digest
        ]));
        affected.insert(key);
        if change.kind == "node" {
            signals.insert(("node".to_owned(), change.identifier.clone()));
            for (row, is_old) in [(old.as_ref(), true), (item.as_ref(), false)]
                .into_iter()
                .filter_map(|(row, is_old)| row.map(|row| (row, is_old)))
            {
                if row["type_id"] == "tos.entity.claim" {
                    signals.insert((
                        "claim".into(),
                        exact_text(
                            if is_old {
                                c.exact
                                    .get(&(change.kind.clone(), change.identifier.clone()))
                                    .ok_or(Error::Invalid("semantic original old row absent"))?
                            } else {
                                change
                                    .item
                                    .as_ref()
                                    .ok_or(Error::Invalid("semantic candidate original absent"))?
                            },
                            &["entity_id"],
                        )?,
                    ));
                }
            }
        } else {
            for (row, is_old) in [(old.as_ref(), true), (item.as_ref(), false)]
                .into_iter()
                .filter_map(|(row, is_old)| row.map(|row| (row, is_old)))
            {
                signals.insert((
                    "outgoing".into(),
                    compact(
                        &json!([
                            exact_text(
                                if is_old {
                                    c.exact
                                        .get(&(change.kind.clone(), change.identifier.clone()))
                                        .ok_or(Error::Invalid("semantic original old row absent"))?
                                } else {
                                    change.item.as_ref().ok_or(Error::Invalid(
                                        "semantic candidate original absent",
                                    ))?
                                },
                                &["from_id"]
                            )?,
                            semantic_string(&row["relation_type_id"])?
                        ]),
                        limits.max_row_bytes,
                    )?,
                ));
            }
        }
        if let Some(original) = &change.item {
            c.exact.insert(
                (change.kind.clone(), change.identifier.clone()),
                original.clone(),
            );
        }
        pending.push((
            change.kind.clone(),
            change.identifier.clone(),
            item,
            order,
            digest,
        ));
    }
    for (dk, di) in signals {
        let semantic_stream_args = &[sv(&dk), sv(&di), iv(next_bound(limits.max_rows)?)?];
        let mut semantic_stream_statement=c.b.statement("SELECT kind,id FROM semantic_deps INDEXED BY semantic_dependents WHERE dependency_kind=? AND dependency_id=? ORDER BY kind,id LIMIT ?")?;
        let mut semantic_stream_cursor = semantic_stream_statement
            .query(rusqlite::params_from_iter(semantic_stream_args.iter()))?;
        while let Some(semantic_stream_row) = semantic_stream_cursor.next()? {
            let row = c.b.admit_row(semantic_stream_row)?;
            affected.insert((st(&row[0])?.into(), st(&row[1])?.into()));
            if affected.len() as u64 > limits.max_rows {
                return Err(Error::Budget("semantic affected closure"));
            }
        }
    }
    let mut totals = counts(&state["counts"])?;
    for (kind, id) in &affected {
        if let Some(row) = c.b.one(
            "SELECT counts FROM semantic_rows WHERE kind=? AND id=?",
            &[sv(kind), sv(id)],
        )? {
            add_counts(&mut totals, counts(&decode(st(&row[0])?)?)?, -1)?
        }
    }
    for (kind, id, item, _, _) in &pending {
        if kind == "relation" {
            c.adjust(id, -1)?;
            c.b.execute("DELETE FROM semantic_edges WHERE id=?", &[sv(id)])?
        }
        for table in ["semantic_rows", "semantic_deps", "semantic_diagnostics"] {
            c.b.execute(
                &format!("DELETE FROM {table} WHERE kind=? AND id=?"),
                &[sv(kind), sv(id)],
            )?
        }
        c.overlay.insert((kind.clone(), id.clone()), item.clone());
    }
    for (kind, _, item, order, digest) in &pending {
        if let Some(item) = item {
            c.add(
                kind,
                item,
                order.ok_or(Error::Invalid("semantic pending order"))?,
                digest
                    .as_deref()
                    .ok_or(Error::Invalid("semantic pending digest"))?,
            )?
        }
    }
    for (kind, id) in affected {
        add_counts(&mut totals, c.evaluate(&kind, &id)?, 1)?
    }
    let registry = state["registry_violations"]
        .as_array()
        .ok_or(Error::Invalid("semantic registry state"))?;
    let report = report(&mut c.b, totals, registry)?;
    let mut checks = c.checked;
    for (kind, id, _, order, digest) in pending {
        checks.insert((kind, id), (digest, order.map(|n| n as i64)));
    }
    for ((kind, id), (digest, order)) in checks {
        c.b.execute(
            "INSERT INTO semantic_pending VALUES (?,?,?,?)",
            &[
                sv(&kind),
                sv(&id),
                digest.as_deref().map(sv).unwrap_or(SqlValue::Null),
                order.map(SqlValue::Integer).unwrap_or(SqlValue::Null),
            ],
        )?
    }
    let object = state
        .as_object_mut()
        .ok_or(Error::Invalid("semantic state object"))?;
    for (key, value) in [
        ("pending", json!(true)),
        ("counts", json!(totals)),
        ("next_source_revision", json!(new_source_revision)),
        ("next_epoch", json!(epoch + 1)),
        (
            "changes_digest",
            json!(sha(&compact(&json!(frames), limits.max_input_bytes)?)),
        ),
        (
            "report_digest",
            json!(sha(&compact(&report, limits.max_output_bytes)?)),
        ),
    ] {
        object.insert(key.into(), value);
    }
    c.b.put_state(&state)?;
    parse(
        &compact(&report, limits.max_output_bytes)?,
        limits.max_output_bytes,
    )
}
/// Finalize the exact published binding; never commits or selects it.
pub fn verify_semantic_index_binding_transaction(
    tx: &Transaction<'_>,
    new_binding: &JsonValue,
    processor: &str,
    limits: SemanticMaintenanceLimits,
) -> Result<JsonValue> {
    let mut b = Budget::new(tx, limits)?;
    b.input_json(new_binding)?;
    let new_binding_raw = encode(new_binding, limits.max_input_bytes)?;
    let new_binding = decode(&new_binding_raw)?;
    let binding = b.binding(&new_binding)?;
    let mut state = b.state()?;
    let expected = state["binding"].clone();
    if state["dependencies"]["projector"] != semantic_projector_sha256()
        || state["dependencies"]["normalization"]["processor_digest"] != processor
    {
        return Err(Error::Invalid(
            "semantic projector changed; explicit bootstrap required",
        ));
    }
    let pending = state["pending"]
        .as_bool()
        .ok_or(Error::Invalid("semantic pending state"))?;
    if pending {
        if binding["publication_epoch"] != state["next_epoch"]
            || binding["source_revision"] != state["next_source_revision"]
            || binding["normalization_binding"] != expected["normalization_binding"]
        {
            return Err(Error::Invalid("semantic final pending binding differs"));
        }
    } else if !python_eq(&binding, &expected)? {
        return Err(Error::Invalid("semantic final binding differs"));
    }
    let semantic_stream_args = &[iv(next_bound(limits.max_rows)?)?];
    let mut semantic_stream_statement = b.statement(
        "SELECT kind,id,digest,source_order FROM semantic_pending ORDER BY kind,id LIMIT ?",
    )?;
    let mut semantic_stream_cursor =
        semantic_stream_statement.query(rusqlite::params_from_iter(semantic_stream_args.iter()))?;
    while let Some(semantic_stream_row) = semantic_stream_cursor.next()? {
        let row = b.admit_row(semantic_stream_row)?;
        let stored = b.stored(st(&row[0])?, st(&row[1])?)?;
        let found = b.one(
            "SELECT source_order FROM prepared_documents WHERE kind=? AND id=?",
            &row[..2],
        )?;
        let digest = stored.map(|(_, _, d)| sv(&d)).unwrap_or(SqlValue::Null);
        let order = found.map(|r| r[0].clone()).unwrap_or(SqlValue::Null);
        if digest != row[2] || order != row[3] {
            return Err(Error::Invalid("semantic final carrier/order dependency"));
        }
    }
    let digest = state["report_digest"]
        .as_str()
        .ok_or(Error::Invalid("semantic report digest"))?
        .to_owned();
    let desc = descriptor(&mut b, &binding, &digest)?;
    if pending {
        let frames = desc["changes"]
            .as_array()
            .ok_or(Error::Invalid("semantic final changes"))?;
        let mut cut = Vec::new();
        for frame in frames {
            let f = frame
                .as_array()
                .filter(|a| a.len() == 6)
                .ok_or(Error::Invalid("semantic final change framing"))?;
            cut.push(json!([f[0], f[1], f[2], f[4], f[5]]));
        }
        if desc["mode"] != "delta-history"
            || desc["parent_data_revision"] != expected["data_revision"]
            || state["changes_digest"] != sha(&compact(&json!(cut), limits.max_input_bytes)?)
        {
            return Err(Error::Invalid(
                "semantic final changes differ from validated overlay",
            ));
        }
    }
    b.execute("DELETE FROM semantic_pending", &[])?;
    let o = state.as_object_mut().unwrap();
    o.insert("binding".into(), binding.clone());
    o.insert("pending".into(), json!(false));
    for key in ["next_source_revision", "next_epoch", "changes_digest"] {
        o.remove(key);
    }
    b.put_state(&state)?;
    parse(
        &format!(
            "{{\"binding\":{},\"semantic_report_sha256\":{}}}",
            new_binding_raw,
            compact(&json!(digest), limits.max_output_bytes)?
        ),
        limits.max_output_bytes,
    )
}

fn view(v: &JsonValue, cap: usize) -> Result<Value> {
    decode(&encode(v, cap)?)
}
fn jfield<'a>(v: &'a JsonValue, path: &[&str]) -> Result<&'a JsonValue> {
    let mut current = v;
    for key in path {
        let JsonValue::Object(fields) = current else {
            return Err(Error::Invalid("semantic exact JSON object"));
        };
        current = fields
            .iter()
            .find(|(k, _)| k.as_str() == Some(*key))
            .map(|(_, v)| v)
            .ok_or(Error::Invalid("semantic exact JSON field"))?;
    }
    Ok(current)
}
fn report_raw(v: &Value, cap: usize) -> Result<String> {
    let keys = [
        "valid",
        "violations",
        "registered_node_count",
        "unmapped_node_count",
        "registered_relation_count",
        "unmapped_relation_count",
        "claim_contract_count",
        "cross_layer_relation_count",
        "gaps",
    ];
    let mut raw = String::from("{");
    for (i, key) in keys.iter().enumerate() {
        if i > 0 {
            raw.push(',')
        }
        raw.push_str(&serde_json::to_string(key).map_err(|e| Error::Source(e.to_string()))?);
        raw.push(':');
        raw.push_str(&compact(&v[*key], cap)?);
        if raw.len() > cap {
            return Err(Error::Budget("semantic report bytes"));
        }
    }
    raw.push('}');
    if raw.len() > cap {
        return Err(Error::Budget("semantic report bytes"));
    }
    Ok(raw)
}
fn bootstrap_add(
    c: &mut Context<'_, '_, '_>,
    kind: &str,
    id: &str,
    order: i64,
    supplied: Option<&str>,
    last: &mut i64,
) -> Result<()> {
    if order <= *last || order < 0 {
        return Err(Error::Invalid(
            "semantic bootstrap unique increasing source order",
        ));
    }
    *last = order;
    let (item, original, digest) =
        c.b.stored(kind, id)?
            .ok_or(Error::Invalid("semantic bootstrap row absent"))?;
    if supplied.is_some_and(|s| sha(s) != digest) {
        return Err(Error::Invalid(
            "semantic supplied row differs from prepared digest",
        ));
    }
    c.exact.insert((kind.into(), id.into()), original);
    c.add(kind, &item, order as u64, &digest)?;
    c.exact.remove(&(kind.into(), id.into()));
    Ok(())
}

fn canonical(value: &str) -> Result<&str> {
    tos_foundation::python_strip_unicode16_v1(value, value.chars().count())
        .map_err(|e| Error::Source(e.to_string()))
}

fn validate_top(top: &Value, raw: &str, cap: usize) -> Result<()> {
    if !crate::local_prepared_read::exact_top_keys(&parse(raw, cap)?)
        || top["schema"] != "tos_published_knowledge_reader_v2"
        || top["graph_schema"] != "tos_knowledge_graph_v1"
        || top["row_integrity"] != "sha256-emitted-json-v1"
    {
        return Err(Error::Invalid("semantic exact prepared reader metadata"));
    }
    for key in [
        "source_revision",
        "data_revision",
        "catalog_sha256",
        "lens_sha256",
    ] {
        if !top[key].as_str().is_some_and(valid_hex) {
            return Err(Error::Invalid("semantic prepared metadata checksum"));
        }
    }
    let norm = &top["normalization_binding"];
    let keys = [
        "schema",
        "processor_digest",
        "entity_registry_digest",
        "relation_registry_digest",
        "configuration_digest",
    ];
    if !norm
        .as_object()
        .is_some_and(|o| o.len() == keys.len() && keys.iter().all(|k| o.contains_key(*k)))
        || norm["schema"] != "tos_knowledge_graph_normalization_binding_v1"
        || keys[1..]
            .iter()
            .any(|k| !norm[*k].as_str().is_some_and(valid_hex))
    {
        return Err(Error::Invalid("semantic exact normalization metadata"));
    }
    let boundary = &top["authority_boundary"];
    if !boundary.is_object()
        || boundary["source_owner"] != "Tree-of-Sophia"
        || ["is_source", "is_canon", "writes_to_tree"]
            .iter()
            .any(|k| boundary[*k] != false)
    {
        return Err(Error::Invalid("semantic source authority boundary"));
    }
    Ok(())
}
fn valid_hex(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}

fn next_bound(value: u64) -> Result<u64> {
    value
        .checked_add(1)
        .ok_or(Error::Budget("semantic SQL bound overflow"))
}

fn exact_text(original: &JsonValue, path: &[&str]) -> Result<String> {
    let mut current = original;
    for key in path {
        current = current.object_get(key).unwrap_or(&JsonValue::Null);
    }
    crate::local_prepared::python_value_string(current)
}

fn sql_scalar(value: &Value) -> Result<SqlValue> {
    match value {
        Value::Null => Ok(SqlValue::Null),
        Value::String(s) => Ok(sv(s)),
        Value::Bool(v) => Ok(SqlValue::Integer(i64::from(*v))),
        Value::Number(n) => {
            let text = n.to_string();
            if !text.contains(['.', 'e', 'E']) {
                return n
                    .as_i64()
                    .map(SqlValue::Integer)
                    .ok_or(Error::Invalid("semantic SQLite integer range"));
            }
            n.as_f64()
                .filter(|n| n.is_finite())
                .map(SqlValue::Real)
                .ok_or(Error::Invalid("semantic SQLite finite scalar"))
        }
        _ => Err(Error::Invalid(
            "semantic SQLite unsupported compound parameter",
        )),
    }
}

fn numeric_exceeded(peak: i64, maximum: &Value) -> Result<bool> {
    match maximum {
        Value::Null => Ok(false),
        Value::Bool(v) => Ok(peak > i64::from(*v)),
        Value::Number(n) => {
            let lexeme = n.to_string();
            if !lexeme.contains(['.', 'e', 'E']) {
                if let Some(negative) = lexeme.strip_prefix('-') {
                    return Ok(negative.bytes().any(|b| b != b'0') || peak > 0);
                }
                let digits = lexeme.trim_start_matches('0');
                let digits = if digits.is_empty() { "0" } else { digits };
                let peak = peak.to_string();
                return Ok(peak.len() > digits.len()
                    || (peak.len() == digits.len() && peak.as_str() > digits));
            }
            let f = n
                .as_f64()
                .filter(|v| v.is_finite())
                .ok_or(Error::Invalid("semantic finite cardinality maximum"))?;
            Ok(peak as f64 > f)
        }
        _ => Err(Error::Invalid("semantic cardinality comparable maximum")),
    }
}

fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(v) => *v,
        Value::Number(n) => n.as_f64().map(|n| n != 0.0).unwrap_or(true),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

fn semantic_string(v: &Value) -> Result<Option<&str>> {
    let Some(s) = v.as_str() else { return Ok(None) };
    let s = canonical(s)?;
    Ok((!s.is_empty()).then_some(s))
}
