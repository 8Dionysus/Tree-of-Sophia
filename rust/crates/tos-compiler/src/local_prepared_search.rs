//! Exact local prepared compressed-search storage v3 writer.
//!
//! The caller owns the open main-database transaction, connection, VM/deadline
//! policy, journal/WAL reservation and rollback on *any* error. This module never
//! begins, commits, rolls back, closes, attaches or creates SQLite TEMP objects.
//! The pager cap covers the entire main database; pager settings are not SQL
//! data and are not promised to roll back. No corpus, digest or source is selected.
use crate::{Error, Result};
use rusqlite::{Connection, OptionalExtension, Params, params};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs::File,
    io::Read,
    time::Instant,
};
use tos_foundation::{
    CanonicalProfile, Digest256Hasher, JsonLimits, JsonNumber, JsonNumberKind, JsonString,
    JsonValue, canonical_bytes_v1, python_lower_unicode16_v1,
};

pub const SCHEMA: &str = "tos_knowledge_search_compressed_v3";
pub const ALGORITHM: &str = "python-lower-json-default-order-v1";
pub const STORAGE_VERSION: u64 = 3;
pub const MAX_ADDRESS: u64 = (1 << 53) - 1;
pub const BLOCK_SIZE: usize = 256;
pub const CHUNK_SIZE: usize = 32768;
pub const MAX_DOCUMENT_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_ROW_BYTES: usize = 1_900_000;
pub const MAX_TERMS: usize = 200_000;
pub const MAX_HEADER_BYTES: usize = 65536;
const MAX_VALUES: usize = 8192;
const MAX_CACHE_BLOCKS: usize = 8192;
const MAX_CACHE_BYTES: usize = 32 * 1024 * 1024;
const REVERSE_DOMAIN: &[u8] = b"tos-search-reverse-uvarint-v1\0";

/// Six data tables and one header, byte-for-byte column/index contracts of
/// access/src/tos_access/compressed_search_store.py. Execute one DDL at a time.
pub const DDL: &[&str] = &[
    "CREATE TABLE search_header (singleton INTEGER PRIMARY KEY CHECK(singleton=1), header TEXT NOT NULL, cursor_key BLOB NOT NULL, high_water INTEGER NOT NULL, max_pages INTEGER NOT NULL)",
    "CREATE TABLE search_documents (doc_id INTEGER PRIMARY KEY, kind TEXT NOT NULL, identifier BLOB NOT NULL, sort_key BLOB NOT NULL, filters BLOB NOT NULL, UNIQUE(kind,sort_key), UNIQUE(kind,identifier))",
    "CREATE TABLE search_values (doc_id INTEGER NOT NULL, category TEXT NOT NULL, field INTEGER NOT NULL, byte_length INTEGER NOT NULL, PRIMARY KEY(doc_id,category,field)) WITHOUT ROWID",
    "CREATE TABLE search_text_chunks (doc_id INTEGER NOT NULL, category TEXT NOT NULL, field INTEGER NOT NULL, chunk INTEGER NOT NULL, payload BLOB NOT NULL, PRIMARY KEY(doc_id,category,field,chunk)) WITHOUT ROWID",
    "CREATE TABLE search_terms (term_id INTEGER PRIMARY KEY, kind TEXT NOT NULL, plane INTEGER NOT NULL, n INTEGER NOT NULL, term_key BLOB NOT NULL, posting_count INTEGER NOT NULL, UNIQUE(kind,plane,n,term_key))",
    "CREATE TABLE search_blocks (term_id INTEGER NOT NULL, lower_fence BLOB NOT NULL, posting_count INTEGER NOT NULL, payload BLOB NOT NULL, PRIMARY KEY(term_id,lower_fence)) WITHOUT ROWID",
    "CREATE INDEX search_blocks_nonempty ON search_blocks(term_id,lower_fence) WHERE posting_count>0",
    "CREATE TABLE search_document_terms (doc_id INTEGER PRIMARY KEY, term_count INTEGER NOT NULL, payload BLOB NOT NULL, digest BLOB NOT NULL)",
];

#[derive(Clone, Debug)]
pub struct PreparedSearchDocument {
    pub doc_id: u64,
    pub kind: String,
    pub identifier: JsonValue,
    pub source_order: u64,
    pub searchable: String,
    pub identities: Vec<String>,
    pub visible: Vec<String>,
    pub filters: JsonValue,
}
impl PreparedSearchDocument {
    /// Exact normalized-object preparation; display dictionaries retain their
    /// source encounter order. Unicode is the pinned foundation Unicode 16.
    pub fn from_item(doc_id: u64, kind: &str, item: &JsonValue, source_order: u64) -> Result<Self> {
        address(doc_id)?;
        order(source_order)?;
        valid_kind(kind)?;
        if item.as_object().is_none() {
            return Err(Error::Invalid("prepared search object"));
        }
        let searchable = lower(
            std::str::from_utf8(&default_json(item, MAX_DOCUMENT_BYTES)?)
                .map_err(|_| Error::Invalid("search JSON UTF-8"))?,
        )?;
        let identifier = item.object_get("id").unwrap_or(&JsonValue::Null).clone();
        let native = item.object_get("native_id").unwrap_or(&JsonValue::Null);
        let fields: &[&str] = if kind == "relation" {
            &["label", "inverse_label", "statement", "explanation"]
        } else {
            &["title", "kind_label", "summary"]
        };
        let display = item.object_get("display");
        let mut identities = vec![
            lower(&python_str_or_empty(&identifier)?)?,
            lower(&python_str_or_empty(native)?)?,
        ];
        identities.extend(display_values(display, fields[0])?);
        let mut visible = Vec::new();
        for field in fields {
            visible.extend(display_values(display, field)?);
        }
        let filters = JsonValue::Object(
            [
                "source_graph",
                "kind_id",
                "predicate_id",
                "type_id",
                "relation_type_id",
            ]
            .into_iter()
            .map(|key| {
                (
                    JsonString::from_utf8(key),
                    item.object_get(key).unwrap_or(&JsonValue::Null).clone(),
                )
            })
            .collect(),
        );
        let document = Self {
            doc_id,
            kind: kind.to_owned(),
            identifier,
            source_order,
            searchable,
            identities,
            visible,
            filters,
        };
        document.metadata()?;
        Ok(document)
    }
    fn metadata(&self) -> Result<(Vec<u8>, Vec<u8>, Vec<u8>)> {
        address(self.doc_id)?;
        valid_kind(&self.kind)?;
        if self
            .identities
            .len()
            .checked_add(self.visible.len())
            .is_none_or(|n| n > MAX_VALUES)
        {
            return Err(Error::Budget("search document value count"));
        }
        let mut text_bytes = 0usize;
        for value in std::iter::once(&self.searchable)
            .chain(&self.identities)
            .chain(&self.visible)
        {
            if value.len() > MAX_DOCUMENT_BYTES {
                return Err(Error::Budget("search document value bytes"));
            }
            text_bytes = text_bytes
                .checked_add(value.len())
                .filter(|n| *n <= 4 * MAX_DOCUMENT_BYTES)
                .ok_or(Error::Budget("search aggregate document bytes"))?;
        }
        let key = order_key(&self.identifier, self.source_order)?;
        let identifier = default_json(&self.identifier, MAX_ROW_BYTES)?;
        let filters = default_json(&self.filters, MAX_ROW_BYTES)?;
        if key
            .len()
            .checked_add(identifier.len())
            .and_then(|n| n.checked_add(filters.len()))
            .is_none_or(|n| n > MAX_ROW_BYTES)
        {
            return Err(Error::Budget("search document metadata bytes"));
        }
        Ok((key, identifier, filters))
    }
}
#[derive(Clone, Debug)]
pub enum SearchChange {
    Insert(PreparedSearchDocument),
    Update(PreparedSearchDocument),
    Delete { doc_id: u64 },
}

/// Search-only DML counters. `mutations` charges total_changes deltas (including
/// triggers) with at least one per write. DDL/index pages are covered by pager
/// caps, not represented as DML rows. Retained/workspace bytes are accounted
/// payload, never RSS. The owner's independent carrier writes are not included.
#[derive(Clone, Debug, Default)]
pub struct SearchWriteReport {
    pub mutations: u64,
    pub blocks_written: u64,
    pub payload_bytes_written: u64,
    pub database_bytes: u64,
    pub write_calls: u64,
    pub documents_consumed: u64,
    pub memberships: u64,
    pub term_lookups: u64,
    pub block_reads: u64,
    pub document_key_reads: u64,
    pub reverse_reads: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub cache_evictions: u64,
    pub peak_cached_blocks: usize,
    pub peak_cached_bytes: usize,
    pub peak_key_workspace_bytes: usize,
    pub peak_document_term_bytes: usize,
    pub ddl_statements: u64,
    pub setup_micros: u64,
    pub elapsed_micros: u64,
    /// Always zero: the writer creates no TEMP table/index or sorter query.
    pub temp_objects_created: u64,
    /// Always zero: transaction, file and process locks belong to the caller.
    pub locks_acquired: u64,
}

/// Sorted Python default JSON separators, numeric spelling and UTF-8, without
/// changing source dictionary encounter order in the input object.
pub fn default_json(value: &JsonValue, max_bytes: usize) -> Result<Vec<u8>> {
    let limits = JsonLimits::new(max_bytes, 96, 1_000_000, 4300)
        .map_err(|_| Error::Budget("search JSON limits"))?;
    let compact = canonical_bytes_v1(value, CanonicalProfile::SourceRecordDigestV1, limits)
        .map_err(|e| Error::Source(e.to_string()))?;
    let mut output = Vec::new();
    output
        .try_reserve(compact.len())
        .map_err(|_| Error::Budget("search JSON allocation"))?;
    let (mut string, mut escape) = (false, false);
    for byte in compact {
        if output.len() >= max_bytes {
            return Err(Error::Budget("search JSON bytes"));
        }
        output.push(byte);
        if string {
            if escape {
                escape = false;
            } else if byte == b'\\' {
                escape = true;
            } else if byte == b'"' {
                string = false;
            }
        } else if byte == b'"' {
            string = true;
        } else if byte == b',' || byte == b':' {
            if output.len() >= max_bytes {
                return Err(Error::Budget("search JSON bytes"));
            }
            output.push(b' ');
        }
    }
    Ok(output)
}
pub fn header(binding: &JsonValue) -> Result<String> {
    if binding.as_object().is_none_or(|fields| fields.is_empty()) {
        return Err(Error::Invalid("explicit nonempty search snapshot binding"));
    }
    // Refuse oversized selected input before cloning it into the header.
    default_json(binding, MAX_HEADER_BYTES)?;
    let root = JsonValue::Object(vec![
        (JsonString::from_utf8("schema"), text(SCHEMA)),
        (
            JsonString::from_utf8("storage_version"),
            integer(STORAGE_VERSION),
        ),
        (JsonString::from_utf8("algorithm"), text(ALGORITHM)),
        (JsonString::from_utf8("unicode_version"), text("16.0.0")),
        (JsonString::from_utf8("snapshot"), binding.clone()),
    ]);
    String::from_utf8(default_json(&root, MAX_HEADER_BYTES)?)
        .map_err(|_| Error::Invalid("search header UTF-8"))
}
fn text(value: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(value))
}
fn integer(value: u64) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: value.to_string(),
    })
}
fn valid_kind(kind: &str) -> Result<()> {
    if matches!(kind, "node" | "relation") {
        Ok(())
    } else {
        Err(Error::Invalid("search document kind"))
    }
}
fn address(value: u64) -> Result<()> {
    if (1..=MAX_ADDRESS).contains(&value) {
        Ok(())
    } else {
        Err(Error::Invalid("search document address"))
    }
}
fn order(value: u64) -> Result<()> {
    if value <= MAX_ADDRESS {
        Ok(())
    } else {
        Err(Error::Invalid("search source order"))
    }
}
fn lower(input: &str) -> Result<String> {
    python_lower_unicode16_v1(
        input,
        MAX_DOCUMENT_BYTES,
        MAX_DOCUMENT_BYTES,
        MAX_DOCUMENT_BYTES,
    )
    .map_err(|e| Error::Source(e.to_string()))
}
fn display_values(display: Option<&JsonValue>, field: &str) -> Result<Vec<String>> {
    let mut values = Vec::new();
    if let Some(value) = display.and_then(|v| v.object_get(field)) {
        if let Some(s) = value.as_str() {
            values.push(lower(s)?);
        } else if let Some(fields) = value.as_object() {
            for (_, value) in fields {
                if let Some(s) = value.as_str() {
                    if values.len() >= MAX_VALUES {
                        return Err(Error::Budget("search display value count"));
                    }
                    values.push(lower(s)?);
                }
            }
        }
    }
    Ok(values)
}
/// Normalized carrier IDs are scalar JSON values. Unsupported container IDs
/// are refused explicitly rather than inventing a Python repr/ordering.
fn python_str_or_empty(value: &JsonValue) -> Result<String> {
    if !crate::truthy(value) {
        return Ok(String::new());
    }
    match value {
        JsonValue::String(s) => s
            .as_str()
            .map(str::to_owned)
            .ok_or(Error::Invalid("search lone surrogate identifier")),
        JsonValue::Bool(true) => Ok("True".to_owned()),
        JsonValue::Number(_) => String::from_utf8(default_json(value, MAX_DOCUMENT_BYTES)?)
            .map_err(|_| Error::Invalid("search numeric identifier")),
        JsonValue::Null | JsonValue::Bool(false) => Ok(String::new()),
        JsonValue::Array(_) | JsonValue::Object(_) => Err(Error::Invalid(
            "search normalized identifier must be scalar",
        )),
    }
}
pub fn order_key(identifier: &JsonValue, source_order: u64) -> Result<Vec<u8>> {
    order(source_order)?;
    let lowered = lower(&python_str_or_empty(identifier)?)?;
    let mut key = Vec::new();
    for ch in lowered.chars() {
        if key.len() > MAX_ROW_BYTES - 14 {
            return Err(Error::Budget("search order key bytes"));
        }
        let symbol = (ch as u32 + 1).to_be_bytes();
        key.extend_from_slice(&symbol[1..]);
    }
    key.extend_from_slice(&[0, 0, 0]);
    key.extend_from_slice(&source_order.to_be_bytes());
    Ok(key)
}

fn push_varint(mut value: u64, payload: &mut Vec<u8>) {
    while value >= 128 {
        payload.push((value as u8 & 127) | 128);
        value >>= 7;
    }
    payload.push(value as u8);
}
pub fn encode_search_postings(addresses: &[u64]) -> Result<Vec<u8>> {
    if addresses.len() > BLOCK_SIZE {
        return Err(Error::Invalid("search posting block count"));
    }
    let mut payload = Vec::with_capacity(addresses.len() * 8);
    let mut previous = 0i64;
    let mut seen = BTreeSet::new();
    for &id in addresses {
        address(id)?;
        if !seen.insert(id) {
            return Err(Error::Invalid("duplicate search posting address"));
        }
        let delta = id as i64 - previous;
        let zigzag = if delta >= 0 {
            2 * delta as u64
        } else {
            (-delta as u64) * 2 - 1
        };
        push_varint(zigzag, &mut payload);
        previous = id as i64;
    }
    Ok(payload)
}
/// Canonical storage-v3 signed zigzag frame; at most 256 numeric addresses.
pub fn decode_search_postings(payload: &[u8]) -> Result<Vec<u64>> {
    if payload.len() > BLOCK_SIZE * 8 {
        return Err(Error::Invalid("oversized search posting payload"));
    }
    let mut result = Vec::new();
    let (mut value, mut shift, mut previous) = (0u64, 0u32, 0i64);
    let mut seen = BTreeSet::new();
    for &byte in payload {
        value |= u64::from(byte & 127) << shift;
        if byte & 128 != 0 {
            shift += 7;
            if shift > 56 {
                return Err(Error::Invalid("search posting varint overflow"));
            }
            continue;
        }
        if shift != 0 && byte == 0 {
            return Err(Error::Invalid("noncanonical search posting varint"));
        }
        let delta = if value & 1 == 1 {
            -(value as i64 / 2) - 1
        } else {
            (value / 2) as i64
        };
        previous = previous
            .checked_add(delta)
            .filter(|v| *v >= 1 && *v as u64 <= MAX_ADDRESS)
            .ok_or(Error::Invalid("search posting address overflow"))?;
        if result.len() >= BLOCK_SIZE || !seen.insert(previous as u64) {
            return Err(Error::Invalid("search posting count/duplicate"));
        }
        result.push(previous as u64);
        value = 0;
        shift = 0;
    }
    if shift != 0 {
        return Err(Error::Invalid("truncated search posting varint"));
    }
    Ok(result)
}
fn reverse_digest(doc_id: u64, kind: &str, count: usize, payload: &[u8]) -> Result<[u8; 32]> {
    address(doc_id)?;
    valid_kind(kind)?;
    if count == 0 || count > MAX_TERMS {
        return Err(Error::Invalid("search reverse count"));
    }
    let mut hash = Digest256Hasher::new();
    hash.update(REVERSE_DOMAIN);
    hash.update(&doc_id.to_be_bytes());
    hash.update(kind.as_bytes());
    hash.update(&[0]);
    hash.update(&(count as u32).to_be_bytes());
    hash.update(payload);
    Ok(*hash.finalize().as_bytes())
}
pub fn encode_search_reverse(
    doc_id: u64,
    kind: &str,
    terms: &[u64],
) -> Result<(Vec<u8>, [u8; 32])> {
    if terms.is_empty() || terms.len() > MAX_TERMS {
        return Err(Error::Invalid("search reverse terms"));
    }
    let mut payload = Vec::with_capacity(terms.len());
    let mut previous = 0;
    for &term in terms {
        if term <= previous || term > i64::MAX as u64 {
            return Err(Error::Invalid("search reverse term order"));
        }
        push_varint(term - previous, &mut payload);
        previous = term;
    }
    let digest = reverse_digest(doc_id, kind, terms.len(), &payload)?;
    Ok((payload, digest))
}
pub fn decode_search_reverse(
    doc_id: u64,
    kind: &str,
    count: usize,
    payload: &[u8],
    digest: &[u8],
) -> Result<Vec<u64>> {
    if count == 0
        || count > MAX_TERMS
        || payload.len() < count
        || payload.len() > count * 9
        || digest.len() != 32
    {
        return Err(Error::Invalid("search reverse frame bounds"));
    }
    let expected = reverse_digest(doc_id, kind, count, payload)?;
    let mismatch = digest
        .iter()
        .zip(expected)
        .fold(0u8, |diff, (&a, b)| diff | (a ^ b));
    if mismatch != 0 {
        return Err(Error::Invalid("search reverse digest"));
    }
    let mut terms = Vec::with_capacity(count);
    let (mut value, mut shift, mut previous) = (0u64, 0u32, 0u64);
    for &byte in payload {
        value |= u64::from(byte & 127) << shift;
        if byte & 128 != 0 {
            shift += 7;
            if shift > 56 {
                return Err(Error::Invalid("search reverse varint overflow"));
            }
            continue;
        }
        if value == 0 || (shift != 0 && byte == 0) {
            return Err(Error::Invalid("search reverse noncanonical varint"));
        }
        previous = previous
            .checked_add(value)
            .filter(|v| *v <= i64::MAX as u64)
            .ok_or(Error::Invalid("search reverse term overflow"))?;
        if terms.len() >= count {
            return Err(Error::Invalid("search reverse excess terms"));
        }
        terms.push(previous);
        value = 0;
        shift = 0;
    }
    if shift != 0 || terms.len() != count {
        return Err(Error::Invalid("search reverse truncated frame"));
    }
    Ok(terms)
}

type Term = (u8, u8, Vec<u8>);
fn document_terms(document: &PreparedSearchDocument) -> Result<BTreeSet<Term>> {
    let mut terms = BTreeSet::from([(3, 0, Vec::new())]);
    for value in &document.identities {
        let mut boundaries: Vec<usize> = value.char_indices().map(|(i, _)| i).take(769).collect();
        if value.chars().count() <= 768 {
            terms.insert((0, 0, value.as_bytes().to_vec()));
        }
        boundaries.push(value.len());
        for n in 1..=3 {
            if let Some(&end) = boundaries.get(n) {
                terms.insert((1, n as u8, value.as_bytes()[..end].to_vec()));
            }
        }
    }
    for (plane, values) in [
        (2, &document.visible[..]),
        (3, std::slice::from_ref(&document.searchable)),
    ] {
        for value in values {
            // Three-codepoint rolling window avoids an unbounded input-position
            // array while preserving Python codepoint grams exactly.
            let mut starts = VecDeque::with_capacity(4);
            for (offset, _) in value
                .char_indices()
                .chain(std::iter::once((value.len(), '\0')))
            {
                for (distance, &start) in starts.iter().rev().enumerate() {
                    terms.insert((
                        plane,
                        (distance + 1) as u8,
                        value.as_bytes()[start..offset].to_vec(),
                    ));
                    if terms.len() > MAX_TERMS {
                        return Err(Error::Budget("search document distinct terms"));
                    }
                }
                starts.push_back(offset);
                if starts.len() > 3 {
                    starts.pop_front();
                }
            }
        }
    }
    if terms.len() > MAX_TERMS {
        return Err(Error::Budget("search document distinct terms"));
    }
    Ok(terms)
}

struct CachedBlock {
    term: u64,
    fence: Vec<u8>,
    upper: Option<Vec<u8>>,
    addresses: Vec<u64>,
    last_key: Option<Vec<u8>>,
    dirty: bool,
}
impl CachedBlock {
    fn bytes(&self) -> usize {
        64 + self.fence.len()
            + self.upper.as_ref().map_or(0, Vec::len)
            + self.last_key.as_ref().map_or(0, Vec::len)
            + self.addresses.len() * 8
    }
}
struct Writer<'a> {
    db: &'a Connection,
    maximum: u64,
    bootstrap: bool,
    report: SearchWriteReport,
    cache: BTreeMap<u64, CachedBlock>,
    lru: VecDeque<u64>,
    cached_bytes: usize,
}
impl<'a> Writer<'a> {
    fn new(db: &'a Connection, maximum: u64, bootstrap: bool) -> Result<Self> {
        if maximum == 0 || maximum > if bootstrap { MAX_ADDRESS } else { 20_000_000 } {
            return Err(Error::Invalid("search mutation limit"));
        }
        Ok(Self {
            db,
            maximum,
            bootstrap,
            report: SearchWriteReport::default(),
            cache: BTreeMap::new(),
            lru: VecDeque::new(),
            cached_bytes: 0,
        })
    }
    fn write<P: Params>(&mut self, sql: &str, parameters: P) -> Result<usize> {
        let before = self.db.total_changes();
        let changed = self.db.execute(sql, parameters)?;
        self.report.write_calls += 1;
        self.report.mutations = self
            .report
            .mutations
            .checked_add(self.db.total_changes().saturating_sub(before).max(1))
            .filter(|n| *n <= self.maximum)
            .ok_or(Error::Budget(
                "search mutation cap; abort owner transaction",
            ))?;
        Ok(changed)
    }
    fn block(&mut self, term: u64, fence: &[u8], addresses: &[u64]) -> Result<()> {
        let payload = encode_search_postings(addresses)?;
        self.write(
            "INSERT OR REPLACE INTO search_blocks VALUES (?1,?2,?3,?4)",
            params![term, fence, addresses.len(), payload],
        )?;
        self.report.blocks_written += 1;
        self.report.payload_bytes_written += payload.len() as u64;
        Ok(())
    }
    fn keys(&mut self, addresses: &[u64]) -> Result<BTreeMap<u64, Vec<u8>>> {
        if addresses.len() > BLOCK_SIZE + 1 {
            return Err(Error::Invalid("search key workspace address count"));
        }
        let mut keys = BTreeMap::new();
        let mut bytes = 0;
        for &id in addresses {
            let length: Option<i64> = self
                .db
                .query_row(
                    "SELECT length(sort_key) FROM search_documents WHERE doc_id=?1",
                    [id],
                    |r| r.get(0),
                )
                .optional()?;
            if length.is_none_or(|n| n < 11 || n > MAX_ROW_BYTES as i64) {
                return Err(Error::Invalid(
                    "missing/oversized search posting document key",
                ));
            }
            let key: Vec<u8> = self.db.query_row(
                "SELECT sort_key FROM search_documents WHERE doc_id=?1",
                [id],
                |r| r.get(0),
            )?;
            self.report.document_key_reads += 1;
            bytes += key.len();
            keys.insert(id, key);
        }
        self.report.peak_key_workspace_bytes = self.report.peak_key_workspace_bytes.max(bytes);
        Ok(keys)
    }
    fn read_block(&mut self, term: u64, key: &[u8]) -> Result<(Vec<u8>, Vec<u64>)> {
        let probe: Option<(i64, i64, i64)> = self.db.query_row("SELECT length(lower_fence),length(payload),posting_count FROM search_blocks WHERE term_id=?1 AND lower_fence<=?2 ORDER BY lower_fence DESC LIMIT 1", params![term, key], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional()?;
        if let Some((fence_len, payload_len, count)) = probe {
            if !(0..=MAX_ROW_BYTES as i64).contains(&fence_len)
                || !(0..=(BLOCK_SIZE * 8) as i64).contains(&payload_len)
                || !(0..=BLOCK_SIZE as i64).contains(&count)
            {
                return Err(Error::Invalid("search block frame bounds"));
            }
            let (fence, payload): (Vec<u8>, Vec<u8>) = self.db.query_row("SELECT lower_fence,payload FROM search_blocks WHERE term_id=?1 AND lower_fence<=?2 ORDER BY lower_fence DESC LIMIT 1", params![term, key], |r| Ok((r.get(0)?, r.get(1)?)))?;
            self.report.block_reads += 1;
            let addresses = decode_search_postings(&payload)?;
            if addresses.len() != count as usize {
                return Err(Error::Invalid("search block count mismatch"));
            }
            Ok((fence, addresses))
        } else {
            Ok((Vec::new(), Vec::new()))
        }
    }
    fn flush_block(&mut self, block: &mut CachedBlock) -> Result<()> {
        if block.dirty {
            self.block(block.term, &block.fence, &block.addresses)?;
            block.dirty = false;
        }
        Ok(())
    }
    fn take_block(&mut self, term: u64, key: &[u8]) -> Result<CachedBlock> {
        if let Some(mut block) = self.cache.remove(&term) {
            self.cached_bytes -= block.bytes();
            if let Some(position) = self.lru.iter().position(|id| *id == term) {
                self.lru.remove(position);
            }
            if block.fence.as_slice() <= key
                && block
                    .upper
                    .as_ref()
                    .is_none_or(|upper| key < upper.as_slice())
            {
                self.report.cache_hits += 1;
                return Ok(block);
            }
            self.flush_block(&mut block)?;
        }
        self.report.cache_misses += 1;
        let (fence, addresses) = self.read_block(term, key)?;
        let upper_len: Option<i64> = self.db.query_row("SELECT length(lower_fence) FROM search_blocks WHERE term_id=?1 AND lower_fence>?2 ORDER BY lower_fence LIMIT 1", params![term, fence], |r| r.get(0)).optional()?;
        if upper_len.is_some_and(|n| n < 0 || n > MAX_ROW_BYTES as i64) {
            return Err(Error::Invalid("search upper fence bytes"));
        }
        let upper = self.db.query_row("SELECT lower_fence FROM search_blocks WHERE term_id=?1 AND lower_fence>?2 ORDER BY lower_fence LIMIT 1", params![term, fence], |r| r.get(0)).optional()?;
        let last_key = if let Some(&last) = addresses.last() {
            self.keys(&[last])?.remove(&last)
        } else {
            None
        };
        Ok(CachedBlock {
            term,
            fence,
            upper,
            addresses,
            last_key,
            dirty: false,
        })
    }
    fn retain(&mut self, mut block: CachedBlock) -> Result<()> {
        let bytes = block.bytes();
        if bytes > MAX_CACHE_BYTES {
            self.flush_block(&mut block)?;
            return Ok(());
        }
        while !self.cache.is_empty()
            && (self.cache.len() >= MAX_CACHE_BLOCKS || self.cached_bytes + bytes > MAX_CACHE_BYTES)
        {
            let oldest = self
                .lru
                .pop_front()
                .ok_or(Error::Invalid("search cache LRU closure"))?;
            let mut previous = self
                .cache
                .remove(&oldest)
                .ok_or(Error::Invalid("search cache entry closure"))?;
            self.cached_bytes -= previous.bytes();
            self.report.cache_evictions += 1;
            self.flush_block(&mut previous)?;
        }
        self.cached_bytes += bytes;
        self.lru.push_back(block.term);
        self.cache.insert(block.term, block);
        self.report.peak_cached_blocks = self.report.peak_cached_blocks.max(self.cache.len());
        self.report.peak_cached_bytes = self.report.peak_cached_bytes.max(self.cached_bytes);
        Ok(())
    }
    fn membership(&mut self, term: u64, id: u64, key: &[u8], insert: bool) -> Result<()> {
        self.report.memberships += 1;
        if self.bootstrap {
            if !insert {
                return Err(Error::Invalid("bootstrap search deletion"));
            }
            let mut block = self.take_block(term, key)?;
            if block.addresses.contains(&id) {
                return Err(Error::Invalid("duplicate search membership"));
            }
            block.addresses.push(id);
            let mut keys = None;
            if block
                .last_key
                .as_ref()
                .is_none_or(|last| last.as_slice() < key)
            {
                block.last_key = Some(key.to_vec());
            } else {
                let map = self.keys(&block.addresses)?;
                block.addresses.sort_by(|a, b| map[a].cmp(&map[b]));
                block.last_key = block.addresses.last().map(|last| map[last].clone());
                keys = Some(map);
            }
            block.dirty = true;
            if block.addresses.len() > BLOCK_SIZE {
                let middle = block.addresses.len() / 2;
                let map = match keys {
                    Some(map) => map,
                    None => self.keys(&[block.addresses[middle], block.addresses[middle - 1]])?,
                };
                let pivot = map[&block.addresses[middle]].clone();
                let mut left = CachedBlock {
                    term,
                    fence: block.fence,
                    upper: Some(pivot.clone()),
                    addresses: block.addresses[..middle].to_vec(),
                    last_key: Some(map[&block.addresses[middle - 1]].clone()),
                    dirty: true,
                };
                let mut right = CachedBlock {
                    term,
                    fence: pivot.clone(),
                    upper: block.upper,
                    addresses: block.addresses[middle..].to_vec(),
                    last_key: block.last_key,
                    dirty: true,
                };
                self.flush_block(&mut left)?;
                self.flush_block(&mut right)?;
                block = if key >= pivot.as_slice() { right } else { left };
            }
            self.retain(block)?;
        } else {
            let (fence, mut addresses) = self.read_block(term, key)?;
            if insert {
                if addresses.contains(&id) {
                    return Err(Error::Invalid("duplicate search membership"));
                }
                addresses.push(id);
                let keys = self.keys(&addresses)?;
                addresses.sort_by(|a, b| keys[a].cmp(&keys[b]));
                if addresses.len() > BLOCK_SIZE {
                    let middle = addresses.len() / 2;
                    self.block(term, &fence, &addresses[..middle])?;
                    self.block(term, &keys[&addresses[middle]], &addresses[middle..])?;
                } else {
                    self.block(term, &fence, &addresses)?;
                }
            } else {
                let position = addresses
                    .iter()
                    .position(|v| *v == id)
                    .ok_or(Error::Invalid("missing search deletion membership"))?;
                addresses.remove(position);
                self.block(term, &fence, &addresses)?;
            }
        }
        let changed = self.write(if insert { "UPDATE search_terms SET posting_count=posting_count+1 WHERE term_id=?1" } else { "UPDATE search_terms SET posting_count=posting_count-1 WHERE term_id=?1 AND posting_count>0" }, [term])?;
        if changed != 1 {
            return Err(Error::Invalid("search term count update closure"));
        }
        Ok(())
    }
    fn reverse(&mut self, id: u64, kind: &str) -> Result<BTreeSet<u64>> {
        let probe: Option<(i64, i64, i64, String, String, String)> = self.db.query_row("SELECT term_count,length(payload),length(digest),typeof(term_count),typeof(payload),typeof(digest) FROM search_document_terms WHERE doc_id=?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))).optional()?;
        let (count, bytes, digest_bytes, count_type, payload_type, digest_type) =
            probe.ok_or(Error::Invalid("missing search reverse frame"))?;
        if !(1..=MAX_TERMS as i64).contains(&count)
            || bytes < count
            || bytes > count * 9
            || digest_bytes != 32
            || count_type != "integer"
            || payload_type != "blob"
            || digest_type != "blob"
        {
            return Err(Error::Invalid("search reverse frame bounds"));
        }
        let (payload, digest): (Vec<u8>, Vec<u8>) = self.db.query_row(
            "SELECT payload,digest FROM search_document_terms WHERE doc_id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let terms = decode_search_reverse(id, kind, count as usize, &payload, &digest)?;
        self.report.reverse_reads += 1;
        let mut sentinel = false;
        for &term in &terms {
            let row: Option<(String, i64, i64, bool)> = self
                .db
                .query_row(
                    "SELECT kind,plane,n,term_key=x'' FROM search_terms WHERE term_id=?1",
                    [term],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                )
                .optional()?;
            let (term_kind, plane, n, empty) =
                row.ok_or(Error::Invalid("reverse references missing search term"))?;
            if term_kind != kind {
                return Err(Error::Invalid("reverse references wrong-kind search term"));
            }
            sentinel |= plane == 3 && n == 0 && empty;
        }
        if !sentinel {
            return Err(Error::Invalid("reverse omits search all-document term"));
        }
        Ok(terms.into_iter().collect())
    }
    fn old_document(&self, id: u64) -> Result<Option<(String, Vec<u8>)>> {
        let probe: Option<(i64, i64)> = self
            .db
            .query_row(
                "SELECT length(kind),length(sort_key) FROM search_documents WHERE doc_id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((kind_len, key_len)) = probe {
            if !matches!(kind_len, 4 | 8) || !(11..=MAX_ROW_BYTES as i64).contains(&key_len) {
                return Err(Error::Invalid("old search document bounds"));
            }
            let row: (String, Vec<u8>) = self.db.query_row(
                "SELECT kind,sort_key FROM search_documents WHERE doc_id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            valid_kind(&row.0)?;
            Ok(Some(row))
        } else {
            Ok(None)
        }
    }
    fn save_values(&mut self, document: &PreparedSearchDocument) -> Result<()> {
        for (category, values) in [
            ("identity", &document.identities[..]),
            ("visible", &document.visible[..]),
            ("full", std::slice::from_ref(&document.searchable)),
        ] {
            for (field, value) in values.iter().enumerate() {
                self.write(
                    "INSERT INTO search_values VALUES (?1,?2,?3,?4)",
                    params![document.doc_id, category, field, value.len()],
                )?;
                for (chunk, payload) in value.as_bytes().chunks(CHUNK_SIZE).enumerate() {
                    self.write(
                        "INSERT INTO search_text_chunks VALUES (?1,?2,?3,?4,?5)",
                        params![document.doc_id, category, field, chunk, payload],
                    )?;
                }
            }
        }
        Ok(())
    }
    fn replace(&mut self, document: &PreparedSearchDocument, insert: bool) -> Result<()> {
        let (key, identifier, filters) = document.metadata()?;
        let terms = document_terms(document)?;
        self.report.peak_document_term_bytes = self
            .report
            .peak_document_term_bytes
            .max(terms.iter().map(|(_, _, key)| key.len() + 2).sum());
        let same: Option<u64> = self
            .db
            .query_row(
                "SELECT doc_id FROM search_documents WHERE kind=?1 AND identifier=?2",
                params![document.kind, identifier],
                |r| r.get(0),
            )
            .optional()?;
        if same.is_some_and(|id| id != document.doc_id) {
            return Err(Error::Invalid("duplicate exact search source identity"));
        }
        let old = self.old_document(document.doc_id)?;
        if insert == old.is_some() {
            return Err(Error::Invalid("search insert/update existence mismatch"));
        }
        let old_ids = match old.as_ref() {
            Some((kind, _)) => self.reverse(document.doc_id, kind)?,
            None => BTreeSet::new(),
        };
        if insert
            && self
                .db
                .query_row(
                    "SELECT 1 FROM search_document_terms WHERE doc_id=?1",
                    [document.doc_id],
                    |_| Ok(()),
                )
                .optional()?
                .is_some()
        {
            return Err(Error::Invalid("orphan search reverse frame"));
        }
        let mut new_ids = BTreeSet::new();
        for (plane, n, term_key) in terms {
            self.report.term_lookups += 1;
            let existing: Option<u64> = self.db.query_row("SELECT term_id FROM search_terms WHERE kind=?1 AND plane=?2 AND n=?3 AND term_key=?4", params![document.kind, plane, n, term_key], |r| r.get(0)).optional()?;
            let term = if let Some(term) = existing {
                term
            } else {
                self.write("INSERT INTO search_terms(kind,plane,n,term_key,posting_count) VALUES (?1,?2,?3,?4,0)", params![document.kind, plane, n, term_key])?;
                let term = self.db.last_insert_rowid();
                if term <= 0 {
                    return Err(Error::Invalid("search term dictionary address"));
                }
                term as u64
            };
            new_ids.insert(term);
        }
        let moved = old
            .as_ref()
            .is_some_and(|(kind, old_key)| kind != &document.kind || old_key != &key);
        if let Some((_, old_key)) = &old {
            for &term in &old_ids {
                if moved || !new_ids.contains(&term) {
                    self.membership(term, document.doc_id, old_key, false)?;
                }
            }
            self.write(
                "DELETE FROM search_values WHERE doc_id=?1",
                [document.doc_id],
            )?;
            self.write(
                "DELETE FROM search_text_chunks WHERE doc_id=?1",
                [document.doc_id],
            )?;
            self.write("UPDATE search_documents SET kind=?1,identifier=?2,sort_key=?3,filters=?4 WHERE doc_id=?5", params![document.kind, identifier, key, filters, document.doc_id])?;
        } else {
            self.write(
                "INSERT INTO search_documents VALUES (?1,?2,?3,?4,?5)",
                params![document.doc_id, document.kind, identifier, key, filters],
            )?;
        }
        self.save_values(document)?;
        for &term in &new_ids {
            if moved || !old_ids.contains(&term) {
                self.membership(term, document.doc_id, &key, true)?;
            }
        }
        if insert
            || old.as_ref().is_some_and(|(kind, _)| kind != &document.kind)
            || old_ids != new_ids
        {
            let ids: Vec<u64> = new_ids.into_iter().collect();
            let (payload, digest) = encode_search_reverse(document.doc_id, &document.kind, &ids)?;
            if insert {
                self.write(
                    "INSERT INTO search_document_terms VALUES (?1,?2,?3,?4)",
                    params![document.doc_id, ids.len(), payload, &digest[..]],
                )?;
            } else {
                self.write("UPDATE search_document_terms SET term_count=?1,payload=?2,digest=?3 WHERE doc_id=?4", params![ids.len(), payload, &digest[..], document.doc_id])?;
            }
        }
        self.report.documents_consumed += 1;
        Ok(())
    }
    fn delete(&mut self, id: u64) -> Result<()> {
        let (kind, key) = self
            .old_document(id)?
            .ok_or(Error::Invalid("search delete targets missing document"))?;
        let terms = self.reverse(id, &kind)?;
        for term in terms {
            self.membership(term, id, &key, false)?;
        }
        for sql in [
            "DELETE FROM search_document_terms WHERE doc_id=?1",
            "DELETE FROM search_values WHERE doc_id=?1",
            "DELETE FROM search_text_chunks WHERE doc_id=?1",
            "DELETE FROM search_documents WHERE doc_id=?1",
        ] {
            self.write(sql, [id])?;
        }
        self.report.documents_consumed += 1;
        Ok(())
    }
    fn flush(&mut self) -> Result<()> {
        while let Some(term) = self.lru.pop_front() {
            let mut block = self
                .cache
                .remove(&term)
                .ok_or(Error::Invalid("search flush cache closure"))?;
            self.cached_bytes -= block.bytes();
            self.flush_block(&mut block)?;
        }
        Ok(())
    }
}

fn require_transaction(db: &Connection) -> Result<()> {
    if db.is_autocommit() {
        Err(Error::Invalid(
            "already-open caller search transaction required",
        ))
    } else {
        Ok(())
    }
}
fn page_cap(db: &Connection, requested: u64) -> Result<u64> {
    let current: u64 = db.query_row("PRAGMA max_page_count", [], |r| r.get(0))?;
    let cap = current.min(requested);
    let pages: u64 = db.query_row("PRAGMA page_count", [], |r| r.get(0))?;
    if cap == 0 || pages > cap {
        return Err(Error::Budget(
            "existing whole main database exceeds search cap",
        ));
    }
    db.pragma_update(None, "max_page_count", cap)?;
    let actual: u64 = db.query_row("PRAGMA max_page_count", [], |r| r.get(0))?;
    if actual != cap {
        return Err(Error::Budget("whole main database search cap not enforced"));
    }
    Ok(cap)
}
fn cursor_key() -> Result<[u8; 32]> {
    let mut key = [0; 32];
    // Kernel entropy is read afresh at each publication. No deterministic key,
    // clock, digest, source binding or old incarnation is an entropy fallback.
    File::open("/dev/urandom")?.read_exact(&mut key)?;
    Ok(key)
}
fn database_bytes(db: &Connection) -> Result<u64> {
    let pages: u64 = db.query_row("PRAGMA page_count", [], |r| r.get(0))?;
    let size: u64 = db.query_row("PRAGMA page_size", [], |r| r.get(0))?;
    pages
        .checked_mul(size)
        .ok_or(Error::Budget("search database byte count"))
}

pub fn initialize<I>(
    db: &Connection,
    binding: &JsonValue,
    documents: I,
    max_mutations: u64,
    max_bytes: u64,
) -> Result<SearchWriteReport>
where
    I: IntoIterator<Item = Result<PreparedSearchDocument>>,
{
    initialize_with(db, binding, max_mutations, max_bytes, |sink| {
        for document in documents {
            sink(document?)?;
        }
        Ok(())
    })
}
/// Callback producer permits fallible streaming carrier writes in the exact
/// same transaction. Search cap is set before the callback can write carriers.
pub fn initialize_with(
    db: &Connection,
    binding: &JsonValue,
    max_mutations: u64,
    max_bytes: u64,
    produce: impl FnOnce(&mut dyn FnMut(PreparedSearchDocument) -> Result<()>) -> Result<()>,
) -> Result<SearchWriteReport> {
    require_transaction(db)?;
    let started = Instant::now();
    let framed = header(binding)?;
    if !(65536..=(1 << 40)).contains(&max_bytes) {
        return Err(Error::Invalid("search whole-file byte limit"));
    }
    let mut writer = Writer::new(db, max_mutations, true)?;
    let page_size: u64 = db.query_row("PRAGMA page_size", [], |r| r.get(0))?;
    let max_pages = page_cap(db, max_bytes / page_size)?;
    for ddl in DDL {
        db.execute(ddl, [])?;
        writer.report.ddl_statements += 1;
    }
    writer.report.setup_micros = started.elapsed().as_micros().min(u64::MAX as u128) as u64;
    let mut high_water = 0u64;
    produce(&mut |document| {
        writer.replace(&document, true)?;
        high_water = high_water.max(document.doc_id);
        Ok(())
    })?;
    writer.flush()?;
    writer.write(
        "INSERT INTO search_header VALUES (1,?1,?2,?3,?4)",
        params![framed, &cursor_key()?[..], high_water, max_pages],
    )?;
    writer.report.database_bytes = database_bytes(db)?;
    writer.report.elapsed_micros = started.elapsed().as_micros().min(u64::MAX as u128) as u64;
    Ok(writer.report)
}

pub fn apply_delta<I>(
    db: &Connection,
    expected_binding: &JsonValue,
    new_binding: &JsonValue,
    changes: I,
    max_mutations: u64,
) -> Result<SearchWriteReport>
where
    I: IntoIterator<Item = Result<SearchChange>>,
{
    apply_delta_with(db, expected_binding, new_binding, max_mutations, |sink| {
        for change in changes {
            sink(change?)?;
        }
        Ok(())
    })
}
pub fn apply_delta_with(
    db: &Connection,
    expected_binding: &JsonValue,
    new_binding: &JsonValue,
    max_mutations: u64,
    produce: impl FnOnce(&mut dyn FnMut(SearchChange) -> Result<()>) -> Result<()>,
) -> Result<SearchWriteReport> {
    require_transaction(db)?;
    let started = Instant::now();
    let expected = header(expected_binding)?;
    let new = header(new_binding)?;
    if expected == new {
        return Err(Error::Invalid("search delta requires new snapshot binding"));
    }
    let mut writer = Writer::new(db, max_mutations, false)?;
    let probe: Option<(i64, i64, String, String)> = db.query_row("SELECT length(CAST(header AS BLOB)),length(cursor_key),typeof(header),typeof(cursor_key) FROM search_header WHERE singleton=1", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).optional()?;
    if probe.as_ref().is_none_or(|(n, key, ht, kt)| {
        *n < 1 || *n > MAX_HEADER_BYTES as i64 || *key != 32 || ht != "text" || kt != "blob"
    }) {
        return Err(Error::Invalid("missing/invalid search header"));
    }
    let (stored, mut high_water, stored_cap): (String, u64, u64) = db.query_row(
        "SELECT header,high_water,max_pages FROM search_header WHERE singleton=1",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    if stored != expected {
        return Err(Error::Invalid("stale search snapshot binding"));
    }
    if high_water > MAX_ADDRESS || stored_cap == 0 {
        return Err(Error::Invalid("search allocation/page cap"));
    }
    let max_pages = page_cap(db, stored_cap)?;
    writer.report.setup_micros = started.elapsed().as_micros().min(u64::MAX as u128) as u64;
    let mut seen = BTreeSet::new();
    produce(&mut |change| {
        let id = match &change {
            SearchChange::Insert(doc) | SearchChange::Update(doc) => doc.doc_id,
            SearchChange::Delete { doc_id } => *doc_id,
        };
        address(id)?;
        if seen.len() as u64 >= max_mutations || !seen.insert(id) {
            return Err(Error::Invalid("duplicate/over-limit search delta target"));
        }
        match change {
            SearchChange::Insert(document) => {
                if id <= high_water {
                    return Err(Error::Invalid(
                        "search insert reuses/nonmonotonically allocates address",
                    ));
                }
                high_water = id;
                writer.replace(&document, true)?;
            }
            SearchChange::Update(document) => writer.replace(&document, false)?,
            SearchChange::Delete { doc_id } => writer.delete(doc_id)?,
        }
        Ok(())
    })?;
    writer.write("UPDATE search_header SET header=?1,high_water=?2,cursor_key=?3,max_pages=?4 WHERE singleton=1", params![new, high_water, &cursor_key()?[..], max_pages])?;
    writer.report.database_bytes = database_bytes(db)?;
    writer.report.elapsed_micros = started.elapsed().as_micros().min(u64::MAX as u128) as u64;
    Ok(writer.report)
}
