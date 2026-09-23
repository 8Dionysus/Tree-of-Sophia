//! Full, bounded catalog reduction over normalized knowledge rows.
//!
//! The caller supplies one sealed graph header, registries and authored vocabulary.
//! This module only creates a derived candidate. It does not select a publication.

use crate::{Error, QueryVocabulary, Result};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Map, Value, json};
use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};
use tos_foundation::Digest256;

const SCHEMA: &str = "tos_knowledge_catalog_v1";
const NODE: &str = "node";
const RELATION: &str = "relation";

#[derive(Clone, Copy, Debug)]
pub struct CatalogLimits {
    pub max_rows: u64,
    pub max_row_bytes: usize,
    pub max_catalog_bytes: usize,
    pub max_catalog_entries: u64,
    pub max_staging_pages: u64,
}

impl Default for CatalogLimits {
    fn default() -> Self {
        Self {
            max_rows: 10_000_000,
            max_row_bytes: 8 * 1024 * 1024,
            max_catalog_bytes: 16 * 1024 * 1024,
            max_catalog_entries: 100_000,
            max_staging_pages: 1_000_000,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CatalogReceipt {
    pub catalog: Value,
    pub sha256: String,
    pub node_count: u64,
    pub relation_count: u64,
}

struct ByteBudget {
    used: Cell<usize>,
    max: usize,
}

impl ByteBudget {
    fn new(max: usize) -> Self {
        Self {
            used: Cell::new(0),
            max,
        }
    }
    fn charge(&self, bytes: usize) -> Result<()> {
        let next = self
            .used
            .get()
            .checked_add(bytes)
            .ok_or(Error::Budget("catalog decoded bytes"))?;
        if next > self.max {
            return Err(Error::Budget("catalog decoded bytes"));
        }
        self.used.set(next);
        Ok(())
    }
}

struct BoundedWriter {
    bytes: Vec<u8>,
    max: usize,
}

impl Write for BoundedWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let next = self
            .bytes
            .len()
            .checked_add(buf.len())
            .ok_or_else(|| io::Error::other("catalog byte overflow"))?;
        if next > self.max {
            return Err(io::Error::other("catalog byte cap"));
        }
        self.bytes.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn input_preflight(
    header: &Value,
    entity: &Value,
    relation: &Value,
    lenses: &[Value],
    descriptor: &[u8],
    max: usize,
) -> Result<()> {
    if descriptor.len() > max {
        return Err(Error::Budget("catalog input bytes"));
    }
    let mut writer = BoundedWriter {
        bytes: Vec::new(),
        max: max - descriptor.len(),
    };
    serde_json::to_writer(&mut writer, &(header, entity, relation, lenses))
        .map_err(|_| Error::Budget("catalog input bytes"))?;
    Ok(())
}

#[derive(Clone)]
struct Route {
    id: String,
    kinds: Vec<String>,
    predicates: Vec<String>,
    types: Vec<String>,
    relation_types: Vec<String>,
}

fn text<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or(Error::Invalid("catalog required string"))
}

fn array<'a>(v: &'a Value, key: &str) -> Result<&'a [Value]> {
    v.get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or(Error::Invalid("catalog required array"))
}

fn strings(v: &Value, key: &str) -> Result<Vec<String>> {
    array(v, key)?
        .iter()
        .map(|x| {
            x.as_str()
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .ok_or(Error::Invalid("catalog string array"))
        })
        .collect()
}

fn path<'a>(v: &'a Value, name: &str) -> Option<&'a Value> {
    name.split('.')
        .try_fold(v, |at, key| at.as_object()?.get(key))
}

fn py_string(v: &Value) -> String {
    match v {
        Value::Null => "None".into(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::String(s) => s.clone(),
        _ => v.to_string(),
    }
}

fn value_kind(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(n) if n.is_i64() || n.is_u64() => "integer",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn ordered_strings(v: &Value, key: &str) -> Result<Vec<String>> {
    strings(v, key)
}

fn registry_entries(registry: &Value, field: &str, key: &str) -> Result<BTreeMap<String, Value>> {
    let mut out = BTreeMap::new();
    for entry in array(registry, field)? {
        let id = text(entry, key)?.to_owned();
        if out.insert(id, entry.clone()).is_some() {
            return Err(Error::Invalid("duplicate catalog registry ID"));
        }
    }
    Ok(out)
}

fn type_is_a(id: &str, allowed: &[String], entries: &BTreeMap<String, Value>) -> bool {
    let mut todo = vec![id.to_owned()];
    let mut seen = BTreeSet::new();
    while let Some(item) = todo.pop() {
        if allowed.iter().any(|x| x == &item) {
            return true;
        }
        if !seen.insert(item.clone()) {
            continue;
        }
        if let Some(parents) = entries
            .get(&item)
            .and_then(|e| e.get("parent_type_ids"))
            .and_then(Value::as_array)
        {
            todo.extend(parents.iter().filter_map(Value::as_str).map(str::to_owned));
        }
    }
    false
}

fn count(db: &Connection, kind: &str, metric: &str, a: &str, b: &str, c: &str) -> Result<()> {
    db.execute(
        "INSERT INTO temp.cmp_catalog_counts(kind,metric,a,b,c,n) VALUES(?1,?2,?3,?4,?5,1)
        ON CONFLICT(kind,metric,a,b,c) DO UPDATE SET n=n+1",
        params![kind, metric, a, b, c],
    )?;
    Ok(())
}

fn post(
    db: &Connection,
    kind: &str,
    metric: &str,
    field: &str,
    value: &Value,
    order: i64,
    position: usize,
) -> Result<()> {
    let encoded = serde_json::to_string(value).map_err(|_| Error::Invalid("catalog post JSON"))?;
    db.execute("INSERT INTO temp.cmp_catalog_posts(kind,metric,field,value,source_order,position)
        VALUES(?1,?2,?3,?4,?5,?6)
        ON CONFLICT(kind,metric,field,value) DO UPDATE SET
        source_order=MIN(source_order,excluded.source_order),
        position=CASE WHEN excluded.source_order<source_order THEN excluded.position ELSE position END",
        params![kind,metric,field,encoded,order,position as i64])?;
    Ok(())
}

fn first_values(
    db: &Connection,
    kind: &str,
    metric: &str,
    field: &str,
    limit: usize,
    budget: &ByteBudget,
) -> Result<Vec<Value>> {
    let sql_limit = i64::try_from(limit).map_err(|_| Error::Budget("catalog SQL limit"))?;
    let mut stmt = db.prepare(
        "SELECT value FROM temp.cmp_catalog_posts
        WHERE kind=?1 AND metric=?2 AND field=?3 ORDER BY source_order,position LIMIT ?4",
    )?;
    let rows = stmt.query_map(params![kind, metric, field, sql_limit], |r| {
        r.get::<_, String>(0)
    })?;
    let mut out = Vec::new();
    for v in rows {
        let raw = v?;
        budget.charge(raw.len().saturating_add(32))?;
        out.push(
            serde_json::from_str(&raw).map_err(|_| Error::Invalid("catalog stored post JSON"))?,
        );
    }
    Ok(out)
}

fn python_casefold(value: &str) -> Result<String> {
    // FND must supply a complete versioned Unicode casefold table. Until then
    // this producer accepts only the domain where ASCII lowercase is exact.
    if !value.is_ascii() {
        return Err(Error::Invalid("unsupported Unicode catalog facet casefold"));
    }
    Ok(value.to_ascii_lowercase())
}

fn facet_names(vocab: &Value, kind: &str) -> Result<Vec<String>> {
    let catalog = vocab
        .get("catalog")
        .ok_or(Error::Invalid("catalog vocabulary missing"))?;
    let fields = strings(catalog, "facets")?;
    Ok(fields
        .into_iter()
        .map(|f| {
            if kind == RELATION {
                match f.as_str() {
                    "kind_id" => "predicate_id".into(),
                    "type_id" => "relation_type_id".into(),
                    "type_mapping.status" => "predicate_mapping.status".into(),
                    _ => f,
                }
            } else {
                f
            }
        })
        .collect())
}

fn attribute_allowed(field: &str) -> bool {
    let Some(suffix) = field.strip_prefix("attributes.") else {
        return false;
    };
    let bytes = suffix.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 128
        && bytes[0].is_ascii_alphanumeric()
        && bytes
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || matches!(*b, b'_' | b'.' | b'-'))
        && !suffix
            .split('.')
            .any(|part| matches!(part, "__proto__" | "prototype" | "constructor"))
}

fn form_key(key: &str) -> bool {
    if matches!(key, "default" | "original") {
        return true;
    }
    let mut parts = key.split('-');
    let first = parts.next().unwrap_or("");
    if first.eq_ignore_ascii_case("i") || first.eq_ignore_ascii_case("x") {
        let tail: Vec<&str> = parts.collect();
        return !tail.is_empty()
            && tail.iter().all(|p| {
                !p.is_empty() && p.len() <= 8 && p.bytes().all(|b| b.is_ascii_alphanumeric())
            });
    }
    (2..=8).contains(&first.len())
        && first.bytes().all(|b| b.is_ascii_alphabetic())
        && parts
            .all(|p| !p.is_empty() && p.len() <= 8 && p.bytes().all(|b| b.is_ascii_alphanumeric()))
}

fn surface(db: &Connection, kind: &str, item: &Value, order: i64, facets: &[String]) -> Result<()> {
    fn attributes(db: &Connection, kind: &str, at: &Value, prefix: &str, order: i64) -> Result<()> {
        let Some(obj) = at.as_object() else {
            return Ok(());
        };
        for (key, value) in obj {
            let field = format!("{prefix}.{key}");
            if !attribute_allowed(&field) {
                continue;
            }
            count(db, kind, "attribute", &field, "", "")?;
            count(
                db,
                kind,
                "attribute-value-type",
                &field,
                value_kind(value),
                "",
            )?;
            if let Some(source) = item_source(db, kind, order)? {
                count(db, kind, "attribute-source", &field, &source, "")?;
            }
            let members: Vec<&Value> = match value {
                Value::Array(a) => a.iter().collect(),
                _ => vec![value],
            };
            let mut examples = BTreeSet::new();
            for (position, candidate) in members.iter().enumerate() {
                if value.is_array() {
                    count(
                        db,
                        kind,
                        "attribute-array-type",
                        &field,
                        value_kind(candidate),
                        "",
                    )?;
                }
                if candidate.is_null()
                    || candidate.is_array()
                    || candidate.is_object()
                    || examples.len() == 5
                {
                    continue;
                }
                let serialized = serde_json::to_string(candidate)
                    .map_err(|_| Error::Invalid("attribute example"))?;
                if serialized.chars().count() <= 180 && examples.insert(serialized) {
                    post(db, kind, "examples", &field, candidate, order, position)?;
                }
            }
            if value.is_object() {
                attributes(db, kind, value, &field, order)?;
            }
        }
        Ok(())
    }
    attributes(
        db,
        kind,
        item.get("attributes").unwrap_or(&Value::Null),
        "attributes",
        order,
    )?;
    if let Some(display) = item.get("display").and_then(Value::as_object) {
        for (field, forms) in display {
            let allowed = if kind == NODE {
                matches!(field.as_str(), "title" | "kind_label" | "summary")
            } else {
                matches!(
                    field.as_str(),
                    "label" | "inverse_label" | "statement" | "explanation"
                )
            };
            if !allowed {
                continue;
            }
            if let Some(forms) = forms.as_object() {
                for (language, value) in forms {
                    if form_key(language) && value.as_str().is_some_and(|s| !s.is_empty()) {
                        count(
                            db,
                            kind,
                            "display",
                            &format!("display.{field}.{language}"),
                            "",
                            "",
                        )?;
                    }
                }
            }
        }
    }
    for field in facets {
        let raw = path(item, field).unwrap_or(&Value::Null);
        let values: Vec<&Value> = match raw {
            Value::Array(a) => a.iter().collect(),
            _ => vec![raw],
        };
        let mut seen = BTreeSet::new();
        for (position, value) in values.iter().enumerate() {
            if value.is_null() {
                continue;
            }
            let s = py_string(value);
            if s.is_empty() {
                continue;
            }
            count(db, kind, "facet", field, &s, "")?;
            if seen.insert(s.clone()) {
                post(
                    db,
                    kind,
                    "facet-order",
                    field,
                    &Value::String(s),
                    order,
                    position,
                )?;
            }
        }
    }
    Ok(())
}

fn item_source(db: &Connection, kind: &str, order: i64) -> Result<Option<String>> {
    let table = if kind == NODE {
        "knowledge_nodes"
    } else {
        "knowledge_relations"
    };
    let sql = format!("SELECT source_graph FROM {table} WHERE source_order=?1");
    db.query_row(&sql, [order], |r| r.get(0))
        .optional()
        .map_err(Error::from)
}

fn routes(vocab: &Value) -> Result<Vec<Route>> {
    let raw = array(
        vocab
            .get("overview")
            .ok_or(Error::Invalid("overview vocabulary"))?,
        "routes",
    )?;
    raw.iter()
        .map(|r| {
            Ok(Route {
                id: text(r, "route_id")?.into(),
                kinds: ordered_strings(r, "candidate_kind_ids")?,
                predicates: ordered_strings(r, "confirming_predicate_ids")?,
                types: ordered_strings(r, "candidate_type_ids")?,
                relation_types: ordered_strings(r, "confirming_relation_type_ids")?,
            })
        })
        .collect()
}

fn stage(db: &Connection, limits: CatalogLimits) -> Result<()> {
    if limits.max_rows == 0
        || limits.max_row_bytes == 0
        || limits.max_catalog_bytes == 0
        || limits.max_catalog_entries == 0
        || limits.max_staging_pages == 0
    {
        return Err(Error::Budget("catalog limits"));
    }
    let existing: u64 = db.query_row("PRAGMA temp.max_page_count", [], |r| r.get(0))?;
    let ceiling = existing.min(limits.max_staging_pages);
    if ceiling == 0 {
        return Err(Error::Budget("catalog staging pages"));
    }
    let actual: u64 = db.query_row(&format!("PRAGMA temp.max_page_count={ceiling}"), [], |r| {
        r.get(0)
    })?;
    if actual > ceiling {
        return Err(Error::Budget("catalog staging page ceiling"));
    }
    db.execute_batch("CREATE TEMP TABLE cmp_catalog_counts(kind TEXT,metric TEXT,a TEXT,b TEXT,c TEXT,n INTEGER NOT NULL,
        PRIMARY KEY(kind,metric,a,b,c)) WITHOUT ROWID;
        CREATE TEMP TABLE cmp_catalog_posts(kind TEXT,metric TEXT,field TEXT,value TEXT,source_order INTEGER,position INTEGER,
        PRIMARY KEY(kind,metric,field,value)) WITHOUT ROWID;
        CREATE INDEX cmp_catalog_posts_first ON cmp_catalog_posts(kind,metric,field,source_order,position);
        CREATE TEMP TABLE cmp_catalog_node_routes(id TEXT PRIMARY KEY,legacy TEXT NOT NULL,typed TEXT NOT NULL) WITHOUT ROWID;")?;
    Ok(())
}

fn ensure_row(
    item: &Value,
    id: &str,
    source: &str,
    raw: &[u8],
    stored_len: i64,
    stored_sha: &[u8],
    limits: CatalogLimits,
) -> Result<()> {
    if raw.len() > limits.max_row_bytes || stored_len < 0 || raw.len() != stored_len as usize {
        return Err(Error::Budget("catalog row bytes"));
    }
    if stored_sha != &Digest256::of_bytes(raw).as_bytes()[..] {
        return Err(Error::Invalid("catalog row digest"));
    }
    if text(item, "id")? != id || text(item, "source_graph")? != source {
        return Err(Error::Invalid("catalog row identity"));
    }
    if !item.get("source_refs").is_some_and(Value::is_array) {
        return Err(Error::Invalid("catalog row source refs"));
    }
    Ok(())
}

fn valid_order(
    previous: &mut Option<(String, String, i64)>,
    source: &str,
    id: &str,
    order: i64,
) -> Result<()> {
    if order < 0
        || previous
            .as_ref()
            .is_some_and(|(s, i, n)| (source, id) <= (s.as_str(), i.as_str()) || order <= *n)
    {
        return Err(Error::Invalid("catalog source order"));
    }
    *previous = Some((source.into(), id.into(), order));
    Ok(())
}

fn ingest_nodes(
    db: &Connection,
    vocab: &Value,
    entity_entries: &BTreeMap<String, Value>,
    fallback_type: &str,
    route_defs: &[Route],
    limits: CatalogLimits,
) -> Result<u64> {
    let facets = facet_names(vocab, NODE)?;
    let mut stmt = db.prepare(
        "SELECT id,source_graph,kind_id,type_id,source_order,length(payload),payload,payload_len,payload_sha256
        FROM knowledge_nodes ORDER BY source_order",
    )?;
    let mut rows = stmt.query([])?;
    let registered: BTreeSet<String> = array(vocab, "sources")?
        .iter()
        .map(|s| text(s, "source_graph_id").map(str::to_owned))
        .collect::<Result<_>>()?;
    let mut prior = None;
    let mut n = 0u64;
    while let Some(r) = rows.next()? {
        n = n.checked_add(1).ok_or(Error::Budget("catalog rows"))?;
        if n > limits.max_rows {
            return Err(Error::Budget("catalog rows"));
        }
        let actual_len: i64 = r.get(5)?;
        if actual_len < 0 || actual_len as u64 > limits.max_row_bytes as u64 {
            return Err(Error::Budget("catalog row bytes"));
        }
        let (id, source, kind, type_id, order, raw, len, sha): (
            String,
            String,
            String,
            String,
            i64,
            Vec<u8>,
            i64,
            Vec<u8>,
        ) = (
            r.get(0)?,
            r.get(1)?,
            r.get(2)?,
            r.get(3)?,
            r.get(4)?,
            r.get(6)?,
            r.get(7)?,
            r.get(8)?,
        );
        valid_order(&mut prior, &source, &id, order)?;
        if !registered.contains(&source) {
            return Err(Error::Invalid("unregistered catalog node source"));
        }
        let item: Value =
            serde_json::from_slice(&raw).map_err(|_| Error::Invalid("catalog node JSON"))?;
        ensure_row(&item, &id, &source, &raw, len, &sha, limits)?;
        if text(&item, "kind_id")? != kind || text(&item, "type_id")? != type_id {
            return Err(Error::Invalid("catalog node columns"));
        }
        if !entity_entries.contains_key(&type_id) && type_id != fallback_type {
            return Err(Error::Invalid("node type bypasses registry fallback"));
        }
        surface(db, NODE, &item, order, &facets)?;
        count(db, NODE, "total", "", "", "")?;
        count(db, NODE, "group", &kind, "", "")?;
        count(db, NODE, "type", &type_id, "", "")?;
        count(db, NODE, "group-type", &kind, &type_id, "")?;
        let status = path(&item, "type_mapping.status")
            .map(py_string)
            .unwrap_or_else(|| "None".into());
        count(db, NODE, "group-status", &kind, &status, "")?;
        if matches!(status.as_str(), "mapped" | "unmapped") {
            count(db, NODE, "mapping", &status, "", "")?;
        }
        post(
            db,
            NODE,
            "representative",
            &kind,
            path(&item, "display.kind_label").ok_or(Error::Invalid("node kind label"))?,
            order,
            0,
        )?;
        count(db, NODE, "source", &source, "", "")?;
        let state = path(&item, "display.summary_state")
            .map(py_string)
            .unwrap_or_else(|| "None".into());
        count(db, NODE, "summary-state", &state, "", "")?;
        if path(&item, "display.provenance.source_summary_available") == Some(&Value::Bool(false)) {
            count(db, NODE, "without-source", "", "", "")?;
        }
        if let Some(claim_type) = path(&item, "semantics.claim.relation_type_id")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        {
            count(db, NODE, "claim-type", claim_type, "", "")?;
        }
        let mut legacy = Vec::new();
        let mut typed = Vec::new();
        for route in route_defs {
            if route.kinds.iter().any(|k| k == &kind) {
                legacy.push(route.id.clone());
            }
            if !route.types.is_empty() && type_is_a(&type_id, &route.types, entity_entries) {
                typed.push(route.id.clone());
                count(db, NODE, "route-type", &route.id, &type_id, "")?;
            }
        }
        db.execute(
            "INSERT INTO temp.cmp_catalog_node_routes VALUES(?1,?2,?3)",
            params![
                id,
                serde_json::to_string(&legacy).unwrap(),
                serde_json::to_string(&typed).unwrap()
            ],
        )?;
    }
    Ok(n)
}

fn endpoint_routes(db: &Connection, id: &str) -> Result<(Vec<String>, Vec<String>)> {
    let row: Option<(String, String)> = db
        .query_row(
            "SELECT legacy,typed FROM temp.cmp_catalog_node_routes WHERE id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let (legacy, typed) = row.ok_or(Error::Invalid("catalog relation endpoint absent"))?;
    Ok((
        serde_json::from_str(&legacy).map_err(|_| Error::Invalid("catalog endpoint routes"))?,
        serde_json::from_str(&typed).map_err(|_| Error::Invalid("catalog endpoint routes"))?,
    ))
}

fn ingest_relations(
    db: &Connection,
    vocab: &Value,
    route_defs: &[Route],
    relation_entries: &BTreeMap<String, Value>,
    fallback_type: &str,
    cross_source_ids: &BTreeSet<String>,
    limits: CatalogLimits,
) -> Result<u64> {
    let facets = facet_names(vocab, RELATION)?;
    let mut stmt=db.prepare("SELECT id,source_graph,from_id,to_id,predicate_id,relation_type_id,source_order,length(payload),payload,payload_len,payload_sha256
        FROM knowledge_relations ORDER BY source_order")?;
    let mut rows = stmt.query([])?;
    let registered: BTreeSet<String> = array(vocab, "sources")?
        .iter()
        .map(|s| text(s, "source_graph_id").map(str::to_owned))
        .collect::<Result<_>>()?;
    let mut prior = None;
    let mut n = 0u64;
    while let Some(r) = rows.next()? {
        n = n.checked_add(1).ok_or(Error::Budget("catalog rows"))?;
        if n > limits.max_rows {
            return Err(Error::Budget("catalog rows"));
        }
        let actual_len: i64 = r.get(7)?;
        if actual_len < 0 || actual_len as u64 > limits.max_row_bytes as u64 {
            return Err(Error::Budget("catalog row bytes"));
        }
        let (id, source, from, to, predicate, type_id, order, raw, len, sha): (
            String,
            String,
            String,
            String,
            String,
            String,
            i64,
            Vec<u8>,
            i64,
            Vec<u8>,
        ) = (
            r.get(0)?,
            r.get(1)?,
            r.get(2)?,
            r.get(3)?,
            r.get(4)?,
            r.get(5)?,
            r.get(6)?,
            r.get(8)?,
            r.get(9)?,
            r.get(10)?,
        );
        valid_order(&mut prior, &source, &id, order)?;
        if !registered.contains(&source) {
            return Err(Error::Invalid("unregistered catalog relation source"));
        }
        let item: Value =
            serde_json::from_slice(&raw).map_err(|_| Error::Invalid("catalog relation JSON"))?;
        ensure_row(&item, &id, &source, &raw, len, &sha, limits)?;
        for (key, expected) in [
            ("from_id", &from),
            ("to_id", &to),
            ("predicate_id", &predicate),
            ("relation_type_id", &type_id),
        ] {
            if text(&item, key)? != expected {
                return Err(Error::Invalid("catalog relation columns"));
            }
        }
        if !relation_entries.contains_key(&type_id) && type_id != fallback_type {
            return Err(Error::Invalid("relation type bypasses registry fallback"));
        }
        surface(db, RELATION, &item, order, &facets)?;
        count(db, RELATION, "total", "", "", "")?;
        count(db, RELATION, "group", &predicate, "", "")?;
        count(db, RELATION, "type", &type_id, "", "")?;
        count(db, RELATION, "group-type", &predicate, &type_id, "")?;
        let status = path(&item, "predicate_mapping.status")
            .map(py_string)
            .unwrap_or_else(|| "None".into());
        count(db, RELATION, "group-status", &predicate, &status, "")?;
        if matches!(status.as_str(), "mapped" | "unmapped") {
            count(db, RELATION, "mapping", &status, "", "")?;
        }
        post(
            db,
            RELATION,
            "representative",
            &predicate,
            path(&item, "display.label").ok_or(Error::Invalid("relation label"))?,
            order,
            0,
        )?;
        let state = path(&item, "display.explanation_state")
            .map(py_string)
            .unwrap_or_else(|| "None".into());
        count(db, RELATION, "explanation-state", &state, "", "")?;
        if path(&item, "display.provenance.source_explanation_available")
            == Some(&Value::Bool(false))
        {
            count(db, RELATION, "without-source", "", "", "")?;
        }
        if cross_source_ids.contains(&source) {
            count(db, RELATION, "cross-layer", "", "", "")?;
        }
        let (left_l, left_t) = endpoint_routes(db, &from)?;
        let (right_l, right_t) = endpoint_routes(db, &to)?;
        for route in route_defs {
            if route.predicates.iter().any(|v| v == &predicate)
                && (left_l.contains(&route.id) || right_l.contains(&route.id))
            {
                count(db, RELATION, "route-predicate", &route.id, &predicate, "")?;
            }
            if route.relation_types.iter().any(|v| v == &type_id)
                && (left_t.contains(&route.id) || right_t.contains(&route.id))
            {
                count(db, RELATION, "route-type", &route.id, &type_id, "")?;
            }
        }
    }
    Ok(n)
}

fn top_counts(
    db: &Connection,
    kind: &str,
    metric: &str,
    budget: &ByteBudget,
) -> Result<BTreeMap<String, u64>> {
    let mut stmt = db.prepare(
        "SELECT a,n FROM temp.cmp_catalog_counts
        WHERE kind=?1 AND metric=?2 AND b='' AND c='' ORDER BY a",
    )?;
    let rows = stmt.query_map(params![kind, metric], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, u64>(1)?))
    })?;
    let mut out = BTreeMap::new();
    for row in rows {
        let (key, n) = row?;
        budget.charge(key.len().saturating_add(32))?;
        out.insert(key, n);
    }
    Ok(out)
}

fn sub_counts(
    db: &Connection,
    kind: &str,
    metric: &str,
    a: &str,
    budget: &ByteBudget,
) -> Result<BTreeMap<String, u64>> {
    let mut stmt = db.prepare(
        "SELECT b,n FROM temp.cmp_catalog_counts
        WHERE kind=?1 AND metric=?2 AND a=?3 AND c='' ORDER BY b",
    )?;
    let rows = stmt.query_map(params![kind, metric, a], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, u64>(1)?))
    })?;
    let mut out = BTreeMap::new();
    for row in rows {
        let (key, n) = row?;
        budget.charge(key.len().saturating_add(32))?;
        out.insert(key, n);
    }
    Ok(out)
}

fn field_catalog(db: &Connection, kind: &str, metric: &str, budget: &ByteBudget) -> Result<Value> {
    let fields = top_counts(db, kind, metric, budget)?;
    if metric == "display" {
        return Ok(Value::Array(
            fields
                .into_iter()
                .map(|(field, n)| json!({"field":field,"available_item_count":n}))
                .collect(),
        ));
    }
    let mut out = Vec::new();
    for (field, n) in fields {
        let value_types = sub_counts(db, kind, "attribute-value-type", &field, budget)?;
        let array_item_types = sub_counts(db, kind, "attribute-array-type", &field, budget)?;
        let sources: Vec<_> = sub_counts(db, kind, "attribute-source", &field, budget)?
            .into_keys()
            .collect();
        let examples = first_values(db, kind, "examples", &field, 5, budget)?;
        out.push(
            json!({"field":field,"item_count":n,"value_types":value_types,
            "array_item_types":array_item_types,"sources":sources,"examples":examples}),
        );
    }
    Ok(Value::Array(out))
}

fn facets(
    db: &Connection,
    vocab: &Value,
    kind: &str,
    max_entries: u64,
    budget: &ByteBudget,
) -> Result<Value> {
    let mut result = Map::new();
    for field in facet_names(vocab, kind)? {
        let item_counts = sub_counts(db, kind, "facet", &field, budget)?;
        let limit =
            usize::try_from(max_entries).map_err(|_| Error::Budget("catalog facet limit"))?;
        let mut values = first_values(db, kind, "facet-order", &field, limit, budget)?;
        let mut decorated = Vec::with_capacity(values.len());
        for value in values.drain(..) {
            let s = value
                .as_str()
                .ok_or(Error::Invalid("catalog facet value"))?
                .to_owned();
            decorated.push((python_casefold(&s)?, s));
        }
        decorated.sort_by(|a, b| a.0.cmp(&b.0));
        let rows = decorated
            .into_iter()
            .map(|(_, value)| {
                json!({"count":item_counts.get(&value).copied().unwrap_or(0),
            "value":value})
            })
            .collect();
        result.insert(field, Value::Array(rows));
    }
    Ok(Value::Object(result))
}

fn registry_metadata(
    registry: &Value,
    entries: &BTreeMap<String, Value>,
    type_counts: &BTreeMap<String, u64>,
    fallback_key: &str,
    kind: &str,
    node_count: u64,
    claim_counts: Option<&BTreeMap<String, u64>>,
) -> Result<Value> {
    let fallback = text(registry, fallback_key)?;
    let mut rows = Vec::new();
    for (id, entry) in entries {
        let mut object = entry
            .as_object()
            .ok_or(Error::Invalid("registry entry object"))?
            .clone();
        let instance = type_counts.get(id).copied().unwrap_or(0);
        if kind == NODE {
            object.insert("instance_count".into(), json!(instance));
        } else {
            let claims = claim_counts.and_then(|m| m.get(id)).copied().unwrap_or(0);
            object.insert("edge_instance_count".into(), json!(instance));
            object.insert("claim_instance_count".into(), json!(claims));
            object.insert("instance_count".into(), json!(instance + claims));
        }
        rows.push(Value::Object(object));
    }
    let mapped = node_count.saturating_sub(type_counts.get(fallback).copied().unwrap_or(0));
    let key_mapped = if kind == NODE {
        "mapped_instance_count"
    } else {
        "mapped_edge_instance_count"
    };
    let key_unmapped = if kind == NODE {
        "unmapped_instance_count"
    } else {
        "unmapped_edge_instance_count"
    };
    let mut out = Map::new();
    out.insert(
        "registry_id".into(),
        registry.get("registry_id").cloned().unwrap_or(Value::Null),
    );
    out.insert(
        "registry_version".into(),
        registry
            .get("registry_version")
            .cloned()
            .unwrap_or(Value::Null),
    );
    out.insert(
        "source_refs".into(),
        registry.get("source_refs").cloned().unwrap_or(json!([])),
    );
    out.insert(fallback_key.into(), Value::String(fallback.into()));
    out.insert(key_mapped.into(), json!(mapped));
    out.insert(
        key_unmapped.into(),
        json!(type_counts.get(fallback).copied().unwrap_or(0)),
    );
    out.insert("entries".into(), Value::Array(rows));
    Ok(Value::Object(out))
}

fn group_catalog(
    db: &Connection,
    kind: &str,
    relation_entries: &BTreeMap<String, Value>,
    budget: &ByteBudget,
) -> Result<Value> {
    let groups = top_counts(db, kind, "group", budget)?;
    let mut out = Vec::new();
    for (group, n) in groups {
        let types: Vec<String> = sub_counts(db, kind, "group-type", &group, budget)?
            .into_keys()
            .collect();
        let statuses: Vec<String> = sub_counts(db, kind, "group-status", &group, budget)?
            .into_keys()
            .collect();
        let display = first_values(db, kind, "representative", &group, 1, budget)?
            .into_iter()
            .next()
            .ok_or(Error::Invalid("catalog representative absent"))?;
        let mut object = Map::new();
        object.insert(
            if kind == NODE {
                "kind_id"
            } else {
                "predicate_id"
            }
            .into(),
            Value::String(group),
        );
        object.insert("display".into(), display);
        object.insert("count".into(), json!(n));
        object.insert(
            if kind == NODE {
                "type_ids"
            } else {
                "relation_type_ids"
            }
            .into(),
            json!(types),
        );
        object.insert("mapping_statuses".into(), json!(statuses));
        if kind == RELATION {
            let mut defs = Vec::new();
            for type_id in &types {
                if let Some(entry) = relation_entries.get(type_id) {
                    defs.push(json!({"relation_type_id":type_id,
                        "labels":entry.get("labels"),"definition":entry.get("definition"),
                        "domain_type_ids":entry.get("domain_type_ids"),"range_type_ids":entry.get("range_type_ids")}));
                }
            }
            object.insert("semantic_definitions".into(), Value::Array(defs));
        }
        out.push(Value::Object(object));
    }
    Ok(Value::Array(out))
}

fn route_catalog(
    db: &Connection,
    route_defs: &[Route],
    entity_entries: &BTreeMap<String, Value>,
    budget: &ByteBudget,
) -> Result<Value> {
    let kinds = top_counts(db, NODE, "group", budget)?;
    let mut out = Vec::new();
    for route in route_defs {
        let available_kinds: Vec<String> = route
            .kinds
            .iter()
            .filter(|k| kinds.get(*k).copied().unwrap_or(0) > 0)
            .cloned()
            .collect();
        let typed = sub_counts(db, NODE, "route-type", &route.id, budget)?;
        let legacy = sub_counts(db, RELATION, "route-predicate", &route.id, budget)?;
        let typed_relations = sub_counts(db, RELATION, "route-type", &route.id, budget)?;
        let semantic_mode = !entity_entries.is_empty() && !route.types.is_empty();
        let available = if semantic_mode {
            !typed.is_empty()
        } else {
            !available_kinds.is_empty()
        };
        let availability = if available {
            "available"
        } else {
            "not_projected"
        };
        let has_confirmation = if semantic_mode {
            !typed_relations.is_empty()
        } else {
            !legacy.is_empty()
        };
        let expects = if semantic_mode {
            !route.relation_types.is_empty()
        } else {
            !route.predicates.is_empty()
        };
        let readiness = if !available {
            "not_projected"
        } else if expects && !has_confirmation {
            "kind_only"
        } else {
            "confirmed"
        };
        let note = if !available {
            "No trustworthy nodes of these kinds are projected; the backend will not synthesize them from unrelated predicates."
        } else if readiness == "kind_only" {
            "Kinds are projected, but no confirming relation currently establishes the requested contextual role."
        } else if !route.predicates.is_empty() {
            "Kinds select candidate entities; predicates establish contextual roles such as authorship."
        } else {
            "Kinds are source-derived candidates and retain their exact kind_id on every node."
        };
        let node_count: u64 = if semantic_mode {
            typed.values().sum()
        } else {
            available_kinds
                .iter()
                .map(|k| kinds.get(k).copied().unwrap_or(0))
                .sum()
        };
        out.push(json!({"route_id":route.id,"candidate_kind_ids":route.kinds,
            "available_kind_ids":available_kinds,"confirming_predicate_ids":route.predicates,
            "available_confirming_predicate_ids":legacy.keys().collect::<Vec<_>>(),
            "confirming_relation_count":legacy.values().sum::<u64>(),"candidate_type_ids":route.types,
            "available_type_ids":typed.keys().collect::<Vec<_>>(),"confirming_relation_type_ids":route.relation_types,
            "available_confirming_relation_type_ids":typed_relations.keys().collect::<Vec<_>>(),
            "semantic_confirming_relation_count":typed_relations.values().sum::<u64>(),
            "node_count":node_count,"availability":availability,"role_readiness":readiness,"note":note}));
    }
    Ok(Value::Array(out))
}

fn presentation(entity_registry: &Value) -> Result<Value> {
    let Some(value) = entity_registry.get("context_presentation") else {
        return Ok(Value::Null);
    };
    let encoded =
        serde_json::to_vec(value).map_err(|_| Error::Invalid("context presentation JSON"))?;
    Ok(json!({"id":text(value,"presentation_id")?,
        "version":value.get("presentation_version").ok_or(Error::Invalid("presentation version"))?,
        "source_ref":"ToS/doctrine/semantic-interchange/entity-types.v1.json",
        "digest":format!("sha256:{}",Digest256::of_bytes(&encoded).to_hex()),"payload":value}))
}

fn capabilities(
    db: &Connection,
    vocab: &Value,
    max_entries: u64,
    budget: &ByteBudget,
) -> Result<Value> {
    let overview = vocab
        .get("overview")
        .ok_or(Error::Invalid("catalog overview"))?;
    let filters = vocab
        .get("filters")
        .ok_or(Error::Invalid("catalog filters"))?;
    let sources: Vec<String> = array(vocab, "sources")?
        .iter()
        .map(|s| text(s, "source_graph_id").map(str::to_owned))
        .collect::<Result<_>>()?;
    Ok(json!({
        "execution_version":"tos-lens-execution-v7",
        "property_filters":{"selector":"property_id","scope":"node-query-and-path-node-query",
            "binding":"same-graph-snapshot","field_and_property_id":"mutually-exclusive",
            "unknown_value":"does-not-match-except-exists-false","outside_applicable_type":"does-not-match",
            "unknown_property":"error","operators":"declared-per-property",
            "string_comparison":"exact-codepoints-no-casefold-or-normalization",
            "units_and_languages":"source-declared-no-implicit-conversion"},
        "path_query":{"conditions":4,"steps_per_condition":4,"quantifiers":["exists","not_exists"],
            "combination":"all","scope":"node-selector-roots-and-selected-sources","walks_may_revisit_nodes":true},
        "inclusion":{"request_field":"explain","authority":"query-execution-not-semantic-proof"},
        "pagination":{"request_field":"pagination","scope":"bounded-lens-result",
            "snapshot_bound":true,"historical_snapshot_retention":false,"reexecutes_bounded_lens":true,
            "maximum_primary_nodes":100,"maximum_relations":100,"context_endpoints_may_repeat":true,
            "changed_query_or_snapshot_http_status":409},
        "neighborhood_profiles":[
            {"profile":"overview","definition":"Bibliographic and conceptual overview; dense text units, anchors and record-maker/provenance links are inspected separately. Shared record production does not establish semantic proximity. Source-filtered carriers of one declared ToS entity expand at zero distance before a relation hop, within node budgets.",
                "identity_expansion":"declared-tos-entity-id-zero-distance",
                "excluded_predicates":overview.get("excluded_predicate_ids"),
                "excluded_relation_type_ids":overview.get("excluded_relation_type_ids")},
            {"profile":"all","definition":"All declared relation kinds, including detailed text structure; result limits still apply.","excluded_predicates":[]}],
        "sources":sources,"filter_operators":filters.get("operators"),
        "operator_value_contracts":{"eq":"scalar","neq":"scalar","in":"scalar-or-scalar-array",
            "contains":"scalar-or-scalar-array","prefix":"string","exists":"boolean",
            "gt":"number","gte":"number","lt":"number","lte":"number"},
        "node_fields":{let mut v=strings(filters,"node_fields")?;v.sort();v},
        "relation_fields":{let mut v=strings(filters,"relation_fields")?;v.sort();v},
        "human_languages":{"key_pattern":"^(?:[A-Za-z]{2,8}(?:-[A-Za-z0-9]{1,8})*|[iIxX](?:-[A-Za-z0-9]{1,8})+)$(?![\\s\\S])",
            "reserved_roles":["default","original"],"registration_verified":false,
            "node_fields":field_catalog(db,NODE,"display",budget)?,"relation_fields":field_catalog(db,RELATION,"display",budget)?,
            "fallback_order":["default","ru","en","original","remaining-keys-sorted"],
            "boundary":"Availability is not translation, semantic quality, or interface-language equivalence."},
        "attribute_field_pattern":"^(?:attributes|semantics)\\.[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$",
        "node_attribute_fields":field_catalog(db,NODE,"attribute",budget)?,
        "relation_attribute_fields":field_catalog(db,RELATION,"attribute",budget)?,
        "facets":{"nodes":facets(db,vocab,NODE,max_entries,budget)?,"relations":facets(db,vocab,RELATION,max_entries,budget)?},
        "layouts":["auto","organic","timeline","flow","evidence","semantic","infrastructure","hierarchical","radial","matrix"],
        "endpoint_policies":["both","either","independent"],
        "focus":{"seed_field":"seed.focus_node_id","resolution_order":["id","entity_id","unique_native_id"],
            "shared_entity_id_resolution":"source-priority-then-node-id","ambiguous_native_id":"rejected",
            "default_depth":1,"default_direction":"either","default_layout":"radial"},
        "maximums":{"filters_per_item_kind":32,"traversal_depth":5,"nodes":1000,"relations":2000,"groups":200}
    }))
}

fn render(
    db: &Connection,
    header: &Value,
    entity_registry: &Value,
    relation_registry: &Value,
    lenses: &[Value],
    vocab: &Value,
    routes: &[Route],
    entity_entries: &BTreeMap<String, Value>,
    relation_entries: &BTreeMap<String, Value>,
    node_count: u64,
    relation_count: u64,
    max_entries: u64,
    budget: &ByteBudget,
) -> Result<Value> {
    let node_types = top_counts(db, NODE, "type", budget)?;
    let relation_types = top_counts(db, RELATION, "type", budget)?;
    let claim_types = top_counts(db, NODE, "claim-type", budget)?;
    let mut caps = capabilities(db, vocab, max_entries, budget)?;
    caps.as_object_mut()
        .ok_or(Error::Invalid("catalog capabilities"))?
        .insert(
            "entity_routes".into(),
            route_catalog(db, routes, entity_entries, budget)?,
        );
    let contract_refs = json!({
        "public_bundle":"/api/knowledge/contracts","knowledge_api":"access/contracts/knowledge-api.v1.json",
        "lens_spec":"access/contracts/lens-spec.v1.schema.json","lens_result":"access/contracts/lens-result.v1.schema.json",
        "temporal_comparison_request":"access/contracts/temporal-comparison-request.v1.schema.json",
        "temporal_comparison_result":"access/contracts/temporal-comparison-result.v1.schema.json",
        "knowledge_graph":"access/contracts/knowledge-graph.v1.schema.json",
        "readable_context":"access/contracts/readable-context.v1.schema.json",
        "entity_type_registry_schema":"ToS/contracts/semantic-entity-type-registry.schema.json",
        "relation_type_registry_schema":"ToS/contracts/semantic-relation-type-registry.schema.json",
        "entity_type_registry":"ToS/doctrine/semantic-interchange/entity-types.v1.json",
        "relation_type_registry":"ToS/doctrine/semantic-interchange/relation-types.v1.json"});
    let counts = header.get("counts").cloned().unwrap_or(json!({}));
    if let Some(n) = counts.get("nodes").and_then(Value::as_u64) {
        if n != node_count {
            return Err(Error::Invalid("catalog graph node count"));
        }
    }
    if let Some(n) = counts.get("relations").and_then(Value::as_u64) {
        if n != relation_count {
            return Err(Error::Invalid("catalog graph relation count"));
        }
    }
    Ok(
        json!({"schema":SCHEMA,"source_revision":header.get("source_revision"),
        "context_presentation":presentation(entity_registry)?,"contract_refs":contract_refs,
        "counts":counts,"node_kinds":group_catalog(db,NODE,relation_entries,budget)?,
        "predicates":group_catalog(db,RELATION,relation_entries,budget)?,
        "semantic_registries":{
            "properties":entity_registry.get("property_definitions").cloned().unwrap_or(json!([])),
            "entity_types":registry_metadata(entity_registry,entity_entries,&node_types,
                "fallback_type_id",NODE,node_count,None)?,
            "relation_types":registry_metadata(relation_registry,relation_entries,&relation_types,
                "fallback_relation_type_id",RELATION,relation_count,Some(&claim_types))?},
        "lenses":lenses,"capabilities":caps,
        "authority_boundary":header.get("authority_boundary").cloned().unwrap_or(json!({}))}),
    )
}

/// Reduce a full normalized graph into a private catalog candidate. The caller
/// verifies the sealed source cut and registry roots before invoking this.
/// All source rows are read in order; the only in-memory maps are the bounded
/// authored registries and the bounded final catalog.
pub fn compile_catalog(
    db: &mut Connection,
    graph_header: &Value,
    entity_registry: &Value,
    relation_registry: &Value,
    saved_lenses: &[Value],
    vocabulary: &QueryVocabulary,
    authored_descriptor: &[u8],
    limits: CatalogLimits,
) -> Result<CatalogReceipt> {
    if Digest256::of_bytes(authored_descriptor).to_hex() != vocabulary.descriptor_sha256 {
        return Err(Error::Invalid("catalog vocabulary digest"));
    }
    input_preflight(
        graph_header,
        entity_registry,
        relation_registry,
        saved_lenses,
        authored_descriptor,
        limits.max_catalog_bytes,
    )?;
    let descriptor: Value = serde_json::from_slice(authored_descriptor)
        .map_err(|_| Error::Invalid("catalog vocabulary JSON"))?;
    if text(entity_registry, "registry_id")? != vocabulary.entity_registry_id
        || text(relation_registry, "registry_id")? != vocabulary.relation_registry_id
    {
        return Err(Error::Invalid("catalog registry identity"));
    }
    if descriptor
        .get("catalog")
        .and_then(|v| v.get("canonical_order"))
        .and_then(Value::as_str)
        != Some("source-graph-id-v1")
    {
        return Err(Error::Invalid("catalog source order profile"));
    }
    let entity_entries = registry_entries(entity_registry, "types", "type_id")?;
    let relation_entries = registry_entries(relation_registry, "relations", "relation_type_id")?;
    let route_defs = routes(&descriptor)?;
    if route_defs
        .iter()
        .map(|r| &r.id)
        .ne(vocabulary.overview_route_ids.iter())
    {
        return Err(Error::Invalid("catalog vocabulary route mismatch"));
    }
    let original_temp_page_ceiling: u64 =
        db.query_row("PRAGMA temp.max_page_count", [], |r| r.get(0))?;
    db.execute_batch("SAVEPOINT cmp_catalog_build")?;
    let result = (|| {
        stage(db, limits)?;
        let nodes = ingest_nodes(
            db,
            &descriptor,
            &entity_entries,
            text(entity_registry, "fallback_type_id")?,
            &route_defs,
            limits,
        )?;
        let cross_source_ids: BTreeSet<String> = array(&descriptor, "sources")?
            .iter()
            .filter(|source| {
                source.get("input_role").and_then(Value::as_str)
                    == Some("derived-cross-source-join")
            })
            .map(|source| text(source, "source_graph_id").map(str::to_owned))
            .collect::<Result<_>>()?;
        let relations = ingest_relations(
            db,
            &descriptor,
            &route_defs,
            &relation_entries,
            text(relation_registry, "fallback_relation_type_id")?,
            &cross_source_ids,
            CatalogLimits {
                max_rows: limits
                    .max_rows
                    .checked_sub(nodes)
                    .ok_or(Error::Budget("catalog rows"))?,
                ..limits
            },
        )?;
        if nodes
            .checked_add(relations)
            .ok_or(Error::Budget("catalog rows"))?
            > limits.max_rows
        {
            return Err(Error::Budget("catalog rows"));
        }
        let entries: u64 = db.query_row(
            "SELECT
            (SELECT count(*) FROM temp.cmp_catalog_counts)+
            (SELECT count(*) FROM temp.cmp_catalog_posts)+
            (SELECT count(*) FROM temp.cmp_catalog_node_routes)",
            [],
            |r| r.get(0),
        )?;
        if entries > limits.max_catalog_entries {
            return Err(Error::Budget("catalog aggregate entries"));
        }
        let aggregate_bytes: i64 = db.query_row(
            "SELECT
            coalesce((SELECT sum(length(CAST(kind AS BLOB))+length(CAST(metric AS BLOB))+
                length(CAST(a AS BLOB))+length(CAST(b AS BLOB))+length(CAST(c AS BLOB))+32)
                FROM temp.cmp_catalog_counts),0)+
            coalesce((SELECT sum(length(CAST(kind AS BLOB))+length(CAST(metric AS BLOB))+
                length(CAST(field AS BLOB))+length(CAST(value AS BLOB))+32)
                FROM temp.cmp_catalog_posts),0)+
            coalesce((SELECT sum(length(CAST(id AS BLOB))+length(CAST(legacy AS BLOB))+
                length(CAST(typed AS BLOB))+32) FROM temp.cmp_catalog_node_routes),0)",
            [],
            |r| r.get(0),
        )?;
        let reserved = entries
            .checked_mul(32)
            .ok_or(Error::Budget("catalog aggregate bytes"))?;
        if aggregate_bytes < 0
            || (aggregate_bytes as u64)
                .checked_add(reserved)
                .ok_or(Error::Budget("catalog aggregate bytes"))?
                > limits.max_catalog_bytes as u64
        {
            return Err(Error::Budget("catalog aggregate bytes"));
        }
        let budget = ByteBudget::new(limits.max_catalog_bytes);
        let catalog = render(
            db,
            graph_header,
            entity_registry,
            relation_registry,
            saved_lenses,
            &descriptor,
            &route_defs,
            &entity_entries,
            &relation_entries,
            nodes,
            relations,
            limits.max_catalog_entries,
            &budget,
        )?;
        let mut writer = BoundedWriter {
            bytes: Vec::new(),
            max: limits.max_catalog_bytes,
        };
        serde_json::to_writer(&mut writer, &catalog)
            .map_err(|_| Error::Budget("catalog output bytes"))?;
        let bytes = writer.bytes;
        let pages: u64 = db.query_row("PRAGMA temp.page_count", [], |r| r.get(0))?;
        if pages > limits.max_staging_pages {
            return Err(Error::Budget("catalog staging pages"));
        }
        Ok(CatalogReceipt {
            catalog,
            sha256: Digest256::of_bytes(&bytes).to_hex(),
            node_count: nodes,
            relation_count: relations,
        })
    })();
    let rollback = db.execute_batch("ROLLBACK TO cmp_catalog_build; RELEASE cmp_catalog_build");
    let restored: rusqlite::Result<u64> = db.query_row(
        &format!("PRAGMA temp.max_page_count={original_temp_page_ceiling}"),
        [],
        |r| r.get(0),
    );
    rollback?;
    if restored? != original_temp_page_ceiling {
        return Err(Error::Budget("catalog staging page ceiling restore"));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    const VOCAB: &[u8] = include_bytes!("../tests/fixtures/query-vocabulary.v1.json");
    const ADAPTERS: &[&str] = &[
        "philosophy-node-edge-v1",
        "canon-node-relation-v1",
        "candidate-relation-v1",
        "source-navigation-node-edge-v1",
        "reified-bibliographic-claims-v1",
        "declared-identity-and-source-ref-joins-v1",
        "repository-topology-v1",
        "indexed-node-edge-v1",
    ];

    fn insert_node(db: &Connection, id: &str, kind: &str, type_id: &str, order: i64) {
        let item = json!({"id":id,"source_graph":"canon","kind_id":kind,"type_id":type_id,
            "type_mapping":{"status":"mapped"},"source_refs":["ToS/canon/example.json"],
            "display":{"kind_label":{"default":kind},"title":{"default":id},
                "summary_state":"source","provenance":{"source_summary_available":true}},
            "epistemic":{"authority_layer":"canon","canon_status":"accepted","review_posture":"accepted"},
            "graph_layers":["canon"],"view_ids":["route-graph"]});
        let raw = serde_json::to_vec(&item).unwrap();
        db.execute(
            "INSERT INTO knowledge_nodes VALUES(?1,'canon',?2,?3,?4,?5,?6,?7)",
            params![
                id,
                kind,
                type_id,
                order,
                &raw,
                raw.len() as i64,
                Digest256::of_bytes(&raw).as_bytes().as_slice()
            ],
        )
        .unwrap();
    }

    fn fixture(kind: &str) -> (Connection, Value, Value, Value, QueryVocabulary) {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE knowledge_nodes(id TEXT PRIMARY KEY,source_graph TEXT,kind_id TEXT,type_id TEXT,
            source_order INTEGER,payload BLOB,payload_len INTEGER,payload_sha256 BLOB);
            CREATE TABLE knowledge_relations(id TEXT PRIMARY KEY,source_graph TEXT,from_id TEXT,to_id TEXT,
            predicate_id TEXT,relation_type_id TEXT,source_order INTEGER,payload BLOB,payload_len INTEGER,payload_sha256 BLOB);").unwrap();
        insert_node(&db, "canon:a", "work", "tos.entity.work", 0);
        insert_node(&db, "canon:b", kind, "tos.entity.concept", 1);
        let edge = json!({"id":"canon:r","source_graph":"canon","from_id":"canon:a","to_id":"canon:b",
            "predicate_id":"has_expression","relation_type_id":"tos.relation.has-expression",
            "predicate_mapping":{"status":"mapped"},"source_refs":["ToS/canon/relation.json"],
            "display":{"label":{"default":"has expression"},"explanation_state":"source",
                "provenance":{"source_explanation_available":true}},
            "epistemic":{"authority_layer":"canon"},"graph_layers":["canon"],"view_ids":["route-graph"]});
        let raw = serde_json::to_vec(&edge).unwrap();
        db.execute(
            "INSERT INTO knowledge_relations VALUES('canon:r','canon','canon:a','canon:b',
            'has_expression','tos.relation.has-expression',0,?1,?2,?3)",
            params![
                &raw,
                raw.len() as i64,
                Digest256::of_bytes(&raw).as_bytes().as_slice()
            ],
        )
        .unwrap();
        let entity = json!({"registry_id":"tos.semantic.entity-types","registry_version":1,
            "fallback_type_id":"tos.entity.unmapped","source_refs":[],"property_definitions":[],
            "types":[{"type_id":"tos.entity.work","parent_type_ids":[]},
                {"type_id":"tos.entity.concept","parent_type_ids":[]},
                {"type_id":"tos.entity.unmapped","parent_type_ids":[]}]});
        let relation = json!({"registry_id":"tos.semantic.relation-types","registry_version":1,
            "fallback_relation_type_id":"tos.relation.unmapped","source_refs":[],
            "relations":[{"relation_type_id":"tos.relation.has-expression","labels":{"default":"has expression"},
                "definition":"fixture","domain_type_ids":[],"range_type_ids":[]},
                {"relation_type_id":"tos.relation.unmapped"}]});
        let header = json!({"source_revision":"fixture-revision","counts":{"nodes":2,"relations":1},
            "authority_boundary":{"is_source":false}});
        let vocab = QueryVocabulary::parse(VOCAB, ADAPTERS).unwrap();
        (db, header, entity, relation, vocab)
    }

    #[test]
    fn full_catalog_is_deterministic_and_preserves_route_confirmation() {
        let (mut db, header, entity, relation, vocab) = fixture("concept");
        let initial_page_ceiling: u64 = db
            .query_row("PRAGMA temp.max_page_count", [], |r| r.get(0))
            .unwrap();
        let a = compile_catalog(
            &mut db,
            &header,
            &entity,
            &relation,
            &[],
            &vocab,
            VOCAB,
            CatalogLimits::default(),
        )
        .unwrap();
        let b = compile_catalog(
            &mut db,
            &header,
            &entity,
            &relation,
            &[],
            &vocab,
            VOCAB,
            CatalogLimits::default(),
        )
        .unwrap();
        assert_eq!(a.sha256, b.sha256);
        let final_page_ceiling: u64 = db
            .query_row("PRAGMA temp.max_page_count", [], |r| r.get(0))
            .unwrap();
        assert_eq!(initial_page_ceiling, final_page_ceiling);
        assert_eq!((a.node_count, a.relation_count), (2, 1));
        assert_eq!(
            a.catalog["capabilities"]["facets"]["nodes"]["kind_id"][0]["value"],
            "concept"
        );
        let routes = a.catalog["capabilities"]["entity_routes"]
            .as_array()
            .unwrap();
        let work = routes.iter().find(|r| r["route_id"] == "work").unwrap();
        assert_eq!(work["role_readiness"], "confirmed");
        assert_eq!(work["semantic_confirming_relation_count"], 1);
        assert_eq!(
            a.catalog["semantic_registries"]["entity_types"]["mapped_instance_count"],
            2
        );
    }

    #[test]
    fn non_ascii_casefold_requires_pinned_fnd_primitive() {
        for kind in ["Straße", "ς"] {
            let (mut db, header, entity, relation, vocab) = fixture(kind);
            let error = compile_catalog(
                &mut db,
                &header,
                &entity,
                &relation,
                &[],
                &vocab,
                VOCAB,
                CatalogLimits::default(),
            )
            .unwrap_err();
            assert!(error.to_string().contains("casefold"));
        }
    }

    #[test]
    fn hostile_aggregate_count_refuses_before_render() {
        let (mut db, header, entity, relation, vocab) = fixture("concept");
        let limits = CatalogLimits {
            max_catalog_entries: 1,
            ..CatalogLimits::default()
        };
        let error = compile_catalog(
            &mut db,
            &header,
            &entity,
            &relation,
            &[],
            &vocab,
            VOCAB,
            limits,
        )
        .unwrap_err();
        assert!(error.to_string().contains("catalog aggregate entries"));
    }

    #[test]
    fn combined_row_limit_refuses_before_relation_payload_decode() {
        let (mut db, header, entity, relation, vocab) = fixture("concept");
        db.execute(
            "UPDATE knowledge_relations SET payload=zeroblob(200000) WHERE id='canon:r'",
            [],
        )
        .unwrap();
        let limits = CatalogLimits {
            max_rows: 2,
            max_row_bytes: 1024,
            ..CatalogLimits::default()
        };
        let error = compile_catalog(
            &mut db,
            &header,
            &entity,
            &relation,
            &[],
            &vocab,
            VOCAB,
            limits,
        )
        .unwrap_err();
        assert!(error.to_string().contains("catalog rows"));
    }

    #[test]
    fn oversized_blob_refuses_on_sql_length_before_decode() {
        let (mut db, header, entity, relation, vocab) = fixture("concept");
        db.execute(
            "UPDATE knowledge_nodes SET payload=zeroblob(200000) WHERE id='canon:a'",
            [],
        )
        .unwrap();
        let limits = CatalogLimits {
            max_row_bytes: 1024,
            ..CatalogLimits::default()
        };
        let error = compile_catalog(
            &mut db,
            &header,
            &entity,
            &relation,
            &[],
            &vocab,
            VOCAB,
            limits,
        )
        .unwrap_err();
        assert!(error.to_string().contains("catalog row bytes"));
    }

    #[test]
    fn large_facet_value_refuses_before_render() {
        let (mut db, header, entity, relation, vocab) = fixture(&"x".repeat(200_000));
        let limits = CatalogLimits {
            max_catalog_bytes: 64 * 1024,
            ..CatalogLimits::default()
        };
        let error = compile_catalog(
            &mut db,
            &header,
            &entity,
            &relation,
            &[],
            &vocab,
            VOCAB,
            limits,
        )
        .unwrap_err();
        assert!(error.to_string().contains("catalog aggregate bytes"));
    }

    #[test]
    fn unregistered_source_and_tampered_carrier_refuse() {
        let (mut db, header, entity, relation, vocab) = fixture("concept");
        db.execute(
            "UPDATE knowledge_nodes SET source_graph='unknown' WHERE id='canon:b'",
            [],
        )
        .unwrap();
        assert!(
            compile_catalog(
                &mut db,
                &header,
                &entity,
                &relation,
                &[],
                &vocab,
                VOCAB,
                CatalogLimits::default()
            )
            .is_err()
        );
        db.execute("UPDATE knowledge_nodes SET source_graph='canon',payload_sha256=zeroblob(32) WHERE id='canon:b'",[]).unwrap();
        assert!(
            compile_catalog(
                &mut db,
                &header,
                &entity,
                &relation,
                &[],
                &vocab,
                VOCAB,
                CatalogLimits::default()
            )
            .is_err()
        );
    }
}
