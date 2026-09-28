//! Complete indexed-v2 substring carrier over the selected normalized graph.
//! One bounded prepared page writes canonical documents and gram-grouped
//! private TEMP run blocks. Bounded merge passes
//! stream those chunks into the selected posting blocks. The stage owns the private
//! candidate and independently guarded spill namespace. The
//! caller's `StageLimits` supplies cumulative SQLite VM steps, page/output,
//! cache and host-enforced temporary-file caps. `SearchBuildLimits` supplies
//! additional row, posting and work caps. No in-process temp-size sample is
//! claimed to bound a single SQLite statement's external spill.

use crate::{
    Error, Result,
    knowledge_posting_codec::{
        MAX_POSTING_DELTA_BYTES, MAX_POSTINGS_PER_BLOCK, decode_posting_block, encode_posting_block,
    },
    knowledge_stage::{KnowledgeStage, WritePhase},
};
use rusqlite::{Connection, OptionalExtension, params};
use std::{
    cmp::{Ordering, Reverse},
    collections::BinaryHeap,
};
use tos_foundation::{
    CanonicalProfile, Digest256, Digest256Hasher, JsonLimits, JsonMode, JsonString, JsonValue,
    canonical_bytes_v1, parse_json, python_lower_unicode16_v1,
};

pub const SEARCH_PROFILE: &str = "tos-python-native-unicode-v1";
const GRAM_N: i64 = 3;
const MAX_GRAM_BATCH_ROWS: usize = 1024;

#[derive(Clone, Copy, Debug)]
pub struct SearchBuildLimits {
    /// Maximum normalized source carrier fetched into memory, in bytes.
    pub max_payload_bytes: usize,
    /// Maximum lowercased search document, in Unicode scalar values.
    pub max_document_chars: usize,
    pub max_document_bytes: usize,
    pub max_rank_field_bytes: usize,
    pub max_postings: u64,
    /// Counts payload reads, serialized/lowercased documents, rank fields and
    /// every attempted gram write (including duplicates).
    pub max_work_bytes: u64,
    /// Limits one prepared document page, cancellation cadence, and merge fan-in.
    /// Private run and selected posting blocks retain their existing 256 cap.
    pub gram_batch_rows: usize,
}

impl SearchBuildLimits {
    pub(crate) fn validate(self) -> Result<()> {
        if self.max_payload_bytes == 0
            || self.max_payload_bytes > 8_000_000
            || self.max_document_chars == 0
            || self.max_document_chars > 8_000_000
            || self.max_document_bytes == 0
            || self.max_document_bytes > 64_000_000
            || self.max_rank_field_bytes == 0
            || self.max_rank_field_bytes > 8_000_000
            || self.max_postings == 0
            || self.max_work_bytes == 0
            || self.gram_batch_rows == 0
            || self.gram_batch_rows > MAX_GRAM_BATCH_ROWS
        {
            return Err(Error::Budget("search build limits"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct SearchIndexReceipt {
    pub profile: &'static str,
    pub node_documents: u64,
    pub relation_documents: u64,
    pub postings: u64,
    pub distinct_grams: u64,
    pub document_chars: u64,
    pub work_bytes: u64,
    /// SHA-256 of length-framed, table-tagged rows in exact PK order. This is
    /// a projection integrity root, never independent source-cut admission.
    pub search_index_root_sha256: String,
}

pub(crate) struct SourceRow {
    pub(crate) position: i64,
    pub(crate) id: String,
    pub(crate) source_graph: String,
    pub(crate) native_id: Option<String>,
    pub(crate) term_id: String,
    pub(crate) payload_len: i64,
    pub(crate) payload_sha256: Vec<u8>,
    pub(crate) payload: Option<Vec<u8>>,
}

pub(crate) struct Document {
    pub(crate) id_lower: String,
    pub(crate) native_id_lower: String,
    pub(crate) identity_values: String,
    pub(crate) visible_values: String,
    pub(crate) text: String,
    pub(crate) chars: usize,
    pub(crate) digest: Digest256,
    pub(crate) serialization_bytes: usize,
}

struct PreparedDocument {
    row: SourceRow,
    doc: Document,
    offsets: Vec<usize>,
}

fn charge(work: &mut u64, amount: usize, limits: SearchBuildLimits) -> Result<()> {
    *work = work
        .checked_add(amount as u64)
        .ok_or(Error::Budget("search work bytes"))?;
    if *work > limits.max_work_bytes {
        return Err(Error::Budget("search work bytes"));
    }
    Ok(())
}

/// Writes only inside the already private `KnowledgeStage`. On every failure
/// after entry, the stage is poisoned and cannot publish via `finish`.
pub fn build_search_index(
    stage: &mut KnowledgeStage<'_>,
    limits: SearchBuildLimits,
) -> Result<SearchIndexReceipt> {
    let result = build_inner(stage, limits);
    if result.is_err() {
        let _ = stage.with_connection::<()>(WritePhase::Search, |_| {
            Err(Error::Invalid("search producer failed"))
        });
    }
    result
}

fn initialize_search_storage(db: &Connection) -> Result<()> {
    // Stage creation fixes this before Catalog first allocates TEMP pages.
    // Search refuses a changed or missing mode rather than attempting to
    // retrofit an already-used TEMP database.
    let mode: i64 = db.query_row("PRAGMA temp.auto_vacuum", [], |row| row.get(0))?;
    if mode != 2 {
        return Err(Error::Invalid("search TEMP reclamation mode"));
    }
    db.execute_batch(SCHEMA)?;
    Ok(())
}

fn retire_search_staging(db: &Connection) -> Result<()> {
    db.execute_batch("DROP TABLE search_run_chunks; PRAGMA temp.incremental_vacuum")?;
    Ok(())
}

fn build_inner(
    stage: &mut KnowledgeStage<'_>,
    limits: SearchBuildLimits,
) -> Result<SearchIndexReceipt> {
    limits.validate()?;
    stage.with_connection(WritePhase::Search, |db| initialize_search_storage(db))?;
    let mut receipt = SearchIndexReceipt {
        profile: SEARCH_PROFILE,
        node_documents: 0,
        relation_documents: 0,
        postings: 0,
        distinct_grams: 0,
        document_chars: 0,
        work_bytes: 0,
        search_index_root_sha256: String::new(),
    };
    for (kind, table) in [
        ("nodes", "knowledge_nodes"),
        ("relations", "knowledge_relations"),
    ] {
        let mut after = -1i64;
        let before_kind_postings = receipt.postings;
        let mut page = Vec::new();
        let mut page_text_bytes = 0usize;
        let mut run_count = 0i64;
        loop {
            if page.len() == limits.gram_batch_rows {
                stage.with_connection_checks(WritePhase::Search, |db, check| {
                    write_document_page(db, check, &page, kind, run_count, limits, &mut receipt)
                })?;
                run_count = run_count
                    .checked_add(1)
                    .ok_or(Error::Budget("search run id"))?;
                page.clear();
                page_text_bytes = 0;
            }
            let row = stage.with_connection(WritePhase::Search, |db| {
                fetch_next(db, table, after, limits.max_payload_bytes)
            })?;
            let Some(mut row) = row else {
                break;
            };
            if row.position
                != after
                    .checked_add(1)
                    .ok_or(Error::Budget("search position"))?
            {
                return Err(Error::Invalid("search source order gap"));
            }
            after = row.position;
            let payload = row
                .payload
                .as_deref()
                .ok_or(Error::Budget("search payload bytes"))?;
            if row.payload_len < 0
                || row.payload_len as usize != payload.len()
                || row.payload_sha256.as_slice() != Digest256::of_bytes(payload).as_bytes()
            {
                return Err(Error::Invalid("search source payload digest/length"));
            }
            charge(&mut receipt.work_bytes, payload.len(), limits)?;
            let doc = document(&row, kind, payload, limits)?;
            charge(&mut receipt.work_bytes, doc.serialization_bytes, limits)?;
            charge(&mut receipt.work_bytes, doc.text.len(), limits)?;
            charge(
                &mut receipt.work_bytes,
                doc.identity_values.len()
                    + doc.visible_values.len()
                    + doc.id_lower.len()
                    + doc.native_id_lower.len(),
                limits,
            )?;
            row.payload = None;
            if !page.is_empty()
                && page_text_bytes
                    .checked_add(doc.text.len())
                    .ok_or(Error::Budget("search document page bytes"))?
                    > limits.max_document_bytes
            {
                stage.with_connection_checks(WritePhase::Search, |db, check| {
                    write_document_page(db, check, &page, kind, run_count, limits, &mut receipt)
                })?;
                run_count = run_count
                    .checked_add(1)
                    .ok_or(Error::Budget("search run id"))?;
                page.clear();
                page_text_bytes = 0;
            }
            let offsets = stage.with_connection_checks(WritePhase::Search, |_, check| {
                prepare_gram_offsets(&doc, limits, &mut receipt.work_bytes, check)
            })?;
            page_text_bytes = page_text_bytes
                .checked_add(doc.text.len())
                .ok_or(Error::Budget("search document page bytes"))?;
            page.push(PreparedDocument { row, doc, offsets });
        }
        if !page.is_empty() {
            stage.with_connection_checks(WritePhase::Search, |db, check| {
                write_document_page(db, check, &page, kind, run_count, limits, &mut receipt)
            })?;
            run_count = run_count
                .checked_add(1)
                .ok_or(Error::Budget("search run id"))?;
            page.clear();
        }
        let normalized_count: u64 = stage.with_connection(WritePhase::Search, |db| {
            let sql = if kind == "nodes" {
                "SELECT COUNT(*) FROM knowledge_nodes"
            } else {
                "SELECT COUNT(*) FROM knowledge_relations"
            };
            Ok(db.query_row(sql, [], |r| r.get(0))?)
        })?;
        let produced = if kind == "nodes" {
            receipt.node_documents
        } else {
            receipt.relation_documents
        };
        if normalized_count != produced {
            return Err(Error::Invalid("search normalized document coverage"));
        }
        stage.with_connection_checks(WritePhase::Search, |db, check| {
            merge_ordered_kind(
                db,
                check,
                kind,
                limits,
                0,
                run_count,
                receipt.postings - before_kind_postings,
                &mut receipt.work_bytes,
            )
        })?;
        stage.with_connection(WritePhase::Search, |db| {
            db.execute_batch("PRAGMA temp.incremental_vacuum")?;
            Ok(())
        })?;
    }
    // Grouping is an external SQLite index scan under the stage VM and host
    // spill quota. A cap failure poisons and removes the private candidate.
    stage.with_connection(WritePhase::Search, |db| {
        retire_search_staging(db)?;
        db.execute("INSERT INTO search_gram_stats(kind,n,gram,postings) SELECT kind,n,gram,SUM(postings) FROM search_posting_blocks GROUP BY kind,n,gram", [])?;
        db.execute("CREATE INDEX search_document_filter ON search_documents(kind,source_graph,kind_id,predicate_id,position)", [])?;
        Ok(())
    })?;
    let (postings, distinct, root) = stage.with_connection(WritePhase::Search, |db| {
        verify_and_root(db, &mut receipt, limits)
    })?;
    if postings != receipt.postings {
        return Err(Error::Invalid("search posting coverage"));
    }
    receipt.distinct_grams = distinct;
    receipt.search_index_root_sha256 = root;
    Ok(receipt)
}

fn fetch_next(db: &Connection, table: &str, after: i64, cap: usize) -> Result<Option<SourceRow>> {
    let sql = match table {
        "knowledge_nodes" => {
            "SELECT source_order,id,source_graph,native_id,kind_id,payload_len,payload_sha256,CASE WHEN payload_len<=?2 AND length(payload)<=?2 THEN payload ELSE NULL END FROM knowledge_nodes WHERE source_order>?1 ORDER BY source_order LIMIT 1"
        }
        "knowledge_relations" => {
            "SELECT source_order,id,source_graph,native_id,predicate_id,payload_len,payload_sha256,CASE WHEN payload_len<=?2 AND length(payload)<=?2 THEN payload ELSE NULL END FROM knowledge_relations WHERE source_order>?1 ORDER BY source_order LIMIT 1"
        }
        _ => return Err(Error::Invalid("search normalized table")),
    };
    db.query_row(sql, params![after, cap as i64], |row| {
        Ok(SourceRow {
            position: row.get(0)?,
            id: row.get(1)?,
            source_graph: row.get(2)?,
            native_id: row.get(3)?,
            term_id: row.get(4)?,
            payload_len: row.get(5)?,
            payload_sha256: row.get(6)?,
            payload: row.get(7)?,
        })
    })
    .optional()
    .map_err(Error::from)
}

pub(crate) fn document(
    row: &SourceRow,
    kind: &str,
    payload: &[u8],
    limits: SearchBuildLimits,
) -> Result<Document> {
    let json_limits = JsonLimits::new(limits.max_payload_bytes, 96, 1_000_000, 4096)
        .map_err(|_| Error::Budget("search JSON parse limits"))?;
    let parsed = parse_json(payload, JsonMode::PublishedStrict, json_limits)
        .map_err(|e| Error::Source(e.to_string()))?;
    let root = parsed.root();
    if root.as_object().is_none() {
        return Err(Error::Invalid("search carrier object"));
    }
    let field = |name| {
        root.object_get(name)
            .and_then(JsonValue::as_str)
            .unwrap_or("")
    };
    if field("id") != row.id
        || field("source_graph") != row.source_graph
        || field(if kind == "nodes" {
            "kind_id"
        } else {
            "predicate_id"
        }) != row.term_id
        || root.object_get("native_id").and_then(JsonValue::as_str) != row.native_id.as_deref()
    {
        return Err(Error::Invalid("search carrier/normalized identity"));
    }
    let compact = canonical_bytes_v1(
        root,
        CanonicalProfile::SourceRecordDigestV1,
        JsonLimits::new(limits.max_document_bytes, 96, 1_000_000, 4096)
            .map_err(|_| Error::Budget("search JSON output limits"))?,
    )
    .map_err(|e| Error::Source(e.to_string()))?;
    let spaced = default_spaced_json(&compact, limits.max_document_bytes)?;
    let serialization_bytes = compact
        .len()
        .checked_add(spaced.len())
        .ok_or(Error::Budget("search serialization bytes"))?;
    let raw = std::str::from_utf8(&spaced).map_err(|_| Error::Invalid("search JSON UTF-8"))?;
    let text = lower(raw, limits.max_document_chars, limits.max_document_bytes)?;
    let chars = text.chars().count();
    let digest = Digest256::of_bytes(text.as_bytes());
    let id_lower = lower(
        &row.id,
        limits.max_payload_bytes,
        limits.max_rank_field_bytes,
    )?;
    let native_id_lower = lower(
        row.native_id.as_deref().unwrap_or(""),
        limits.max_payload_bytes,
        limits.max_rank_field_bytes,
    )?;
    let display = root.object_get("display");
    let primary = if kind == "nodes" {
        &["title"][..]
    } else {
        &["label"][..]
    };
    let visible = if kind == "nodes" {
        &["title", "kind_label", "summary"][..]
    } else {
        &["label", "inverse_label", "statement", "explanation"][..]
    };
    let identity_values = rank_values(display, primary, limits)?;
    let visible_values = rank_values(display, visible, limits)?;
    Ok(Document {
        id_lower,
        native_id_lower,
        identity_values,
        visible_values,
        text,
        chars,
        digest,
        serialization_bytes,
    })
}

fn lower(input: &str, max_input: usize, max_output: usize) -> Result<String> {
    python_lower_unicode16_v1(input, max_input, max_output, max_output)
        .map_err(|e| Error::Source(e.to_string()))
}

fn rank_values(
    display: Option<&JsonValue>,
    fields: &[&str],
    limits: SearchBuildLimits,
) -> Result<String> {
    let mut values = Vec::new();
    for field in fields {
        let Some(value) = display.and_then(|d| d.object_get(field)) else {
            continue;
        };
        if let Some(text) = value.as_str() {
            values.push(JsonValue::String(JsonString::from_utf8(&lower(
                text,
                limits.max_payload_bytes,
                limits.max_rank_field_bytes,
            )?)));
        } else if let Some(entries) = value.as_object() {
            for (_, candidate) in entries {
                if let Some(text) = candidate.as_str() {
                    values.push(JsonValue::String(JsonString::from_utf8(&lower(
                        text,
                        limits.max_payload_bytes,
                        limits.max_rank_field_bytes,
                    )?)));
                }
            }
        }
    }
    let bytes = canonical_bytes_v1(
        &JsonValue::Array(values),
        CanonicalProfile::SourceRecordDigestV1,
        JsonLimits::new(limits.max_rank_field_bytes, 96, 1_000_000, 4096)
            .map_err(|_| Error::Budget("search rank JSON limit"))?,
    )
    .map_err(|e| Error::Source(e.to_string()))?;
    String::from_utf8(bytes).map_err(|_| Error::Invalid("search rank JSON UTF-8"))
}

/// FND emits the exact Python sorted compact JSON. Python's search ABI uses
/// the same writer with default `, ` and `: ` separators. String literals are
/// already escaped, so only separators outside quoted text gain one space.
fn default_spaced_json(compact: &[u8], cap: usize) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(compact.len().min(cap));
    let mut quoted = false;
    let mut escaped = false;
    for &byte in compact {
        if out.len() >= cap {
            return Err(Error::Budget("search document bytes"));
        }
        out.push(byte);
        if quoted {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                quoted = false;
            }
        } else if byte == b'"' {
            quoted = true;
        } else if byte == b',' || byte == b':' {
            if out.len() >= cap {
                return Err(Error::Budget("search document bytes"));
            }
            out.push(b' ');
        }
    }
    if quoted {
        return Err(Error::Invalid("search JSON string close"));
    }
    Ok(out)
}

fn prepare_gram_offsets(
    doc: &Document,
    limits: SearchBuildLimits,
    work_bytes: &mut u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<usize>> {
    check()?;
    if doc.chars > limits.max_document_chars || doc.text.len() > limits.max_document_bytes {
        return Err(Error::Budget("search document bytes/chars"));
    }
    let gram_count = doc.chars.saturating_sub(2);
    gram_count
        .checked_mul(std::mem::size_of::<usize>())
        .ok_or(Error::Budget("search gram offsets"))?;
    let mut offsets = Vec::new();
    offsets
        .try_reserve_exact(gram_count)
        .map_err(|_| Error::Budget("search gram offsets"))?;
    let mut starts = doc.text.char_indices().map(|(offset, _)| offset);
    if let (Some(mut first), Some(mut second)) = (starts.next(), starts.next()) {
        for (attempt, third) in starts.enumerate() {
            if attempt % limits.gram_batch_rows == 0 {
                check()?;
            }
            let end = third
                + doc.text[third..]
                    .chars()
                    .next()
                    .expect("gram third character")
                    .len_utf8();
            charge(work_bytes, end - first, limits)?;
            offsets.push(first);
            first = second;
            second = third;
        }
    }
    if offsets.len() != gram_count {
        return Err(Error::Invalid("search gram character count"));
    }
    check()?;
    offsets.sort_unstable_by(|a, b| gram_slice(&doc.text, *a).cmp(gram_slice(&doc.text, *b)));
    offsets.dedup_by(|a, b| gram_slice(&doc.text, *a) == gram_slice(&doc.text, *b));
    check()?;
    Ok(offsets)
}

fn write_document_page(
    db: &mut Connection,
    check: &dyn Fn() -> Result<()>,
    page: &[PreparedDocument],
    kind: &str,
    run_id: i64,
    limits: SearchBuildLimits,
    receipt: &mut SearchIndexReceipt,
) -> Result<()> {
    limits.validate()?;
    check()?;
    if page.is_empty() || page.len() > limits.gram_batch_rows {
        return Err(Error::Budget("search document page rows"));
    }
    let transaction = db.transaction()?;
    let mut page_postings = 0u64;
    let mut page_chars = 0u64;
    for prepared in page {
        check()?;
        let row = &prepared.row;
        let doc = &prepared.doc;
        transaction.execute(
            "INSERT INTO search_documents(kind,position,id,source_graph,kind_id,predicate_id,id_lower,native_id_lower,identity_values,visible_values,document_chars,document_digest) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            params![kind,row.position,row.id,row.source_graph,
                if kind == "nodes" { row.term_id.as_str() } else { "" },
                if kind == "relations" { row.term_id.as_str() } else { "" },
                doc.id_lower,doc.native_id_lower,doc.identity_values,doc.visible_values,
                doc.chars as i64,doc.digest.as_bytes().as_slice()],
        )?;
        page_chars = page_chars
            .checked_add(doc.chars as u64)
            .ok_or(Error::Budget("search document characters"))?;
    }
    let mut indices = Vec::new();
    indices
        .try_reserve_exact(page.len())
        .map_err(|_| Error::Budget("search page heads"))?;
    indices.resize(page.len(), 0usize);
    let mut heap = BinaryHeap::new();
    heap.try_reserve(page.len())
        .map_err(|_| Error::Budget("search page heads"))?;
    for (reader, prepared) in page.iter().enumerate() {
        if let Some(&offset) = prepared.offsets.first() {
            heap.push(Reverse(PageHead {
                gram: gram_slice(&prepared.doc.text, offset),
                position: prepared.row.position,
                reader,
            }));
        }
    }
    let mut writer = TempRunWriter::new(run_id);
    while let Some(Reverse(head)) = heap.pop() {
        if page_postings % limits.gram_batch_rows as u64 == 0 {
            check()?;
        }
        charge(&mut receipt.work_bytes, 8, limits)?;
        writer.push_gram(
            &transaction,
            head.gram,
            head.position,
            limits,
            &mut receipt.work_bytes,
            check,
        )?;
        page_postings = page_postings
            .checked_add(1)
            .ok_or(Error::Budget("search postings"))?;
        receipt
            .postings
            .checked_add(page_postings)
            .filter(|value| *value <= limits.max_postings)
            .ok_or(Error::Budget("search postings"))?;
        indices[head.reader] += 1;
        let prepared = &page[head.reader];
        if let Some(&offset) = prepared.offsets.get(indices[head.reader]) {
            heap.push(Reverse(PageHead {
                gram: gram_slice(&prepared.doc.text, offset),
                position: prepared.row.position,
                reader: head.reader,
            }));
        }
    }
    writer.finish(&transaction, limits, &mut receipt.work_bytes, check)?;
    let next_postings = receipt
        .postings
        .checked_add(page_postings)
        .filter(|value| *value <= limits.max_postings)
        .ok_or(Error::Budget("search postings"))?;
    let next_documents = (if kind == "nodes" {
        receipt.node_documents
    } else {
        receipt.relation_documents
    })
    .checked_add(page.len() as u64)
    .ok_or(Error::Budget("search document rows"))?;
    let next_chars = receipt
        .document_chars
        .checked_add(page_chars)
        .ok_or(Error::Budget("search document characters"))?;
    check()?;
    transaction.commit()?;
    receipt.postings = next_postings;
    receipt.document_chars = next_chars;
    if kind == "nodes" {
        receipt.node_documents = next_documents;
    } else {
        receipt.relation_documents = next_documents;
    }
    Ok(())
}

fn gram_slice(text: &str, start: usize) -> &[u8] {
    let (third, character) = text[start..]
        .char_indices()
        .nth(2)
        .expect("prepared three-character gram");
    &text.as_bytes()[start..start + third + character.len_utf8()]
}

// A run record is a bounded three-scalar gram and one normalized document
// position. TEMP chunks contain at most gram_batch_rows records; no posting is
// represented by its own SQL row. The zero padding is never serialized.
#[derive(Clone, Copy, Eq, PartialEq)]
struct RunPosting {
    gram: [u8; 12],
    len: u8,
    position: u64,
}

impl RunPosting {
    fn new(gram: &[u8], position: i64) -> Result<Self> {
        if !(3..=12).contains(&gram.len())
            || std::str::from_utf8(gram).ok().map(|s| s.chars().count()) != Some(3)
            || position < 0
        {
            return Err(Error::Invalid("search run posting"));
        }
        let mut bytes = [0; 12];
        bytes[..gram.len()].copy_from_slice(gram);
        Ok(Self {
            gram: bytes,
            len: gram.len() as u8,
            position: position as u64,
        })
    }

    fn gram(&self) -> &[u8] {
        &self.gram[..usize::from(self.len)]
    }
}

impl Ord for RunPosting {
    fn cmp(&self, other: &Self) -> Ordering {
        self.gram()
            .cmp(other.gram())
            .then(self.position.cmp(&other.position))
    }
}

impl PartialOrd for RunPosting {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Eq, PartialEq)]
struct PageHead<'a> {
    gram: &'a [u8],
    position: i64,
    reader: usize,
}
impl Ord for PageHead<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.gram
            .cmp(other.gram)
            .then(self.position.cmp(&other.position))
            .then(self.reader.cmp(&other.reader))
    }
}
impl PartialOrd for PageHead<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

struct TempRunWriter {
    run_id: i64,
    chunk_no: usize,
    gram: Option<RunPosting>,
    pending: Vec<u64>,
}

impl TempRunWriter {
    fn new(run_id: i64) -> Self {
        Self {
            run_id,
            chunk_no: 0,
            gram: None,
            pending: Vec::new(),
        }
    }
    fn push_gram(
        &mut self,
        db: &Connection,
        gram: &[u8],
        position: i64,
        limits: SearchBuildLimits,
        work: &mut u64,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<()> {
        if position < 0 {
            return Err(Error::Invalid("search run posting"));
        }
        if self.gram.is_some_and(|prior| prior.gram() != gram) {
            self.flush(db, limits, work, check)?;
        }
        if self.gram.is_none_or(|prior| prior.gram() != gram) {
            self.gram = Some(RunPosting::new(gram, position)?);
        }
        if self.pending.is_empty() {
            self.pending
                .try_reserve_exact(MAX_POSTINGS_PER_BLOCK)
                .map_err(|_| Error::Budget("search run block positions"))?;
        }
        self.pending.push(position as u64);
        if self.pending.len() == MAX_POSTINGS_PER_BLOCK {
            self.flush(db, limits, work, check)?;
        }
        Ok(())
    }
    fn push_block(
        &mut self,
        db: &Connection,
        block: &RunBlock,
        limits: SearchBuildLimits,
        work: &mut u64,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<()> {
        if self
            .gram
            .is_some_and(|prior| prior.gram() != block.first_key().gram())
        {
            self.flush(db, limits, work, check)?;
        }
        self.gram = Some(block.first_key());
        let mut remaining = block.positions.as_slice();
        while !remaining.is_empty() {
            let take = (MAX_POSTINGS_PER_BLOCK - self.pending.len())
                .min(limits.gram_batch_rows)
                .min(remaining.len());
            if self.pending.capacity() < MAX_POSTINGS_PER_BLOCK {
                self.pending
                    .try_reserve_exact(MAX_POSTINGS_PER_BLOCK)
                    .map_err(|_| Error::Budget("search run block positions"))?;
            }
            self.pending.extend_from_slice(&remaining[..take]);
            remaining = &remaining[take..];
            check()?;
            if self.pending.len() == MAX_POSTINGS_PER_BLOCK {
                self.flush(db, limits, work, check)?;
            }
        }
        Ok(())
    }
    fn flush(
        &mut self,
        db: &Connection,
        limits: SearchBuildLimits,
        work: &mut u64,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<()> {
        if self.pending.is_empty() {
            return Ok(());
        }
        check()?;
        let gram = self.gram.expect("run block gram");
        insert_run_block(
            db,
            self.run_id,
            self.chunk_no,
            gram.gram(),
            &self.pending,
            limits,
            work,
        )?;
        self.chunk_no = self
            .chunk_no
            .checked_add(1)
            .ok_or(Error::Budget("search run chunk number"))?;
        self.pending.clear();
        check()?;
        Ok(())
    }
    fn finish(
        &mut self,
        db: &Connection,
        limits: SearchBuildLimits,
        work: &mut u64,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<()> {
        self.flush(db, limits, work, check)
    }
}

fn insert_run_block(
    db: &Connection,
    run_id: i64,
    chunk_no: usize,
    gram: &[u8],
    positions: &[u64],
    limits: SearchBuildLimits,
    work: &mut u64,
) -> Result<()> {
    let (first, last, count, deltas) = encode_posting_block(positions)?;
    charge(work, gram.len() + 24 + deltas.len(), limits)?;
    let changed = db.execute(
        "INSERT INTO search_run_chunks(run_id,chunk_no,gram,first_position,last_position,postings,deltas) VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![run_id, i64::try_from(chunk_no).map_err(|_| Error::Budget("search run chunk number"))?,
            gram, first as i64, last as i64, i64::from(count), deltas],
    )?;
    if changed != 1 {
        return Err(Error::Invalid("search run chunk insert"));
    }
    Ok(())
}

struct RunBlock {
    gram: [u8; 12],
    len: u8,
    positions: Vec<u64>,
}
impl RunBlock {
    fn first_key(&self) -> RunPosting {
        RunPosting {
            gram: self.gram,
            len: self.len,
            position: self.positions[0],
        }
    }
    fn last_key(&self) -> RunPosting {
        RunPosting {
            gram: self.gram,
            len: self.len,
            position: *self.positions.last().expect("nonempty run block"),
        }
    }
}

struct RunReader {
    run_id: i64,
    next_chunk: i64,
    previous: Option<RunPosting>,
}
impl RunReader {
    fn new(run_id: i64) -> Self {
        Self {
            run_id,
            next_chunk: 0,
            previous: None,
        }
    }
    fn next_block(
        &mut self,
        db: &Connection,
        limits: SearchBuildLimits,
        work: &mut u64,
    ) -> Result<Option<RunBlock>> {
        let row: Option<(Option<Vec<u8>>, i64, i64, i64, Option<Vec<u8>>)> = db.query_row(
            "SELECT CASE WHEN typeof(gram)='blob' AND length(gram) BETWEEN 3 AND 12 THEN gram ELSE NULL END,first_position,last_position,postings,CASE WHEN typeof(deltas)='blob' AND length(deltas)<=?3 THEN deltas ELSE NULL END FROM search_run_chunks WHERE run_id=?1 AND chunk_no=?2",
            params![self.run_id, self.next_chunk, MAX_POSTING_DELTA_BYTES as i64],
            |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?)),
        ).optional()?;
        let Some((gram, first, last, count, deltas)) = row else {
            return Ok(None);
        };
        let gram = gram.ok_or(Error::Invalid("search run gram length"))?;
        let deltas = deltas.ok_or(Error::Invalid("search run delta length"))?;
        if first < 0 || last < 0 || count <= 0 || count > MAX_POSTINGS_PER_BLOCK as i64 {
            return Err(Error::Invalid("search run block shape"));
        }
        let shape = RunPosting::new(&gram, first)?;
        charge(work, gram.len() + 24 + deltas.len(), limits)?;
        let positions = decode_posting_block(first as u64, last as u64, count as u16, &deltas)?;
        charge(work, positions.len() * 8, limits)?;
        let block = RunBlock {
            gram: shape.gram,
            len: shape.len,
            positions,
        };
        if self
            .previous
            .is_some_and(|prior| block.first_key() <= prior)
        {
            return Err(Error::Invalid("search run order"));
        }
        self.previous = Some(block.last_key());
        self.next_chunk = self
            .next_chunk
            .checked_add(1)
            .ok_or(Error::Budget("search run chunk number"))?;
        Ok(Some(block))
    }
}

#[derive(Eq, PartialEq)]
struct RunHead {
    posting: RunPosting,
    reader: usize,
}
impl Ord for RunHead {
    fn cmp(&self, other: &Self) -> Ordering {
        self.posting
            .cmp(&other.posting)
            .then(self.reader.cmp(&other.reader))
    }
}
impl PartialOrd for RunHead {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn merge_group(
    db: &mut Connection,
    check: &dyn Fn() -> Result<()>,
    first_run: i64,
    run_count: usize,
    limits: SearchBuildLimits,
    work: &mut u64,
    mut emit: impl FnMut(&mut Connection, &RunBlock, &mut u64) -> Result<()>,
) -> Result<u64> {
    // Prepared pages cover consecutive source_order ranges. A run block's
    // positions are therefore wholly before or after another page's block
    // for the same gram; merging block heads is equivalent to merging every
    // logical posting, and the strict global check below refuses any breach.
    let mut readers = Vec::new();
    readers
        .try_reserve_exact(run_count)
        .map_err(|_| Error::Budget("search merge readers"))?;
    let mut current = Vec::new();
    current
        .try_reserve_exact(run_count)
        .map_err(|_| Error::Budget("search merge blocks"))?;
    let mut heap = BinaryHeap::new();
    heap.try_reserve(run_count)
        .map_err(|_| Error::Budget("search merge heads"))?;
    for index in 0..run_count {
        let id = first_run
            .checked_add(i64::try_from(index).map_err(|_| Error::Budget("search run id"))?)
            .ok_or(Error::Budget("search run id"))?;
        let mut reader = RunReader::new(id);
        let block = reader.next_block(db, limits, work)?;
        if let Some(block) = &block {
            heap.push(Reverse(RunHead {
                posting: block.first_key(),
                reader: index,
            }));
        }
        readers.push(reader);
        current.push(block);
    }
    let mut previous = None;
    let mut count = 0u64;
    while let Some(Reverse(head)) = heap.pop() {
        check()?;
        let block = current[head.reader].take().expect("heap run block");
        if previous.is_some_and(|prior| block.first_key() <= prior) {
            return Err(Error::Invalid("search merged posting order"));
        }
        charge(work, block.positions.len() * 8, limits)?;
        emit(db, &block, work)?;
        count = count
            .checked_add(block.positions.len() as u64)
            .filter(|value| *value <= limits.max_postings)
            .ok_or(Error::Budget("search postings"))?;
        previous = Some(block.last_key());
        current[head.reader] = readers[head.reader].next_block(db, limits, work)?;
        if let Some(block) = &current[head.reader] {
            heap.push(Reverse(RunHead {
                posting: block.first_key(),
                reader: head.reader,
            }));
        }
    }
    check()?;
    Ok(count)
}

struct FinalBlock {
    gram: [u8; 12],
    len: u8,
    start: usize,
    end: usize,
}
struct FinalWriter {
    gram: Option<RunPosting>,
    positions: Vec<u64>,
    complete_until: usize,
    blocks: Vec<FinalBlock>,
    written: u64,
}

impl FinalWriter {
    fn new() -> Result<Self> {
        let mut positions = Vec::new();
        positions
            .try_reserve_exact(MAX_GRAM_BATCH_ROWS)
            .map_err(|_| Error::Budget("search final position page"))?;
        let mut blocks = Vec::new();
        blocks
            .try_reserve_exact(MAX_GRAM_BATCH_ROWS)
            .map_err(|_| Error::Budget("search final block page"))?;
        Ok(Self {
            gram: None,
            positions,
            complete_until: 0,
            blocks,
            written: 0,
        })
    }
    fn push_block(
        &mut self,
        db: &mut Connection,
        check: &dyn Fn() -> Result<()>,
        kind: &str,
        block: &RunBlock,
        limits: SearchBuildLimits,
        work: &mut u64,
    ) -> Result<()> {
        if self
            .gram
            .is_some_and(|prior| prior.gram() != block.first_key().gram())
        {
            self.complete();
        }
        self.gram = Some(block.first_key());
        let mut remaining = block.positions.as_slice();
        while !remaining.is_empty() {
            if self.positions.len() == MAX_GRAM_BATCH_ROWS {
                self.flush(db, check, kind, limits, work)?;
            }
            let take = (MAX_POSTINGS_PER_BLOCK - (self.positions.len() - self.complete_until))
                .min(MAX_GRAM_BATCH_ROWS - self.positions.len())
                .min(limits.gram_batch_rows)
                .min(remaining.len());
            self.positions.extend_from_slice(&remaining[..take]);
            remaining = &remaining[take..];
            check()?;
            if self.positions.len() - self.complete_until == MAX_POSTINGS_PER_BLOCK {
                self.complete();
            }
        }
        Ok(())
    }
    fn complete(&mut self) {
        if self.positions.len() == self.complete_until {
            return;
        }
        let gram = self.gram.expect("pending final gram");
        self.blocks.push(FinalBlock {
            gram: gram.gram,
            len: gram.len,
            start: self.complete_until,
            end: self.positions.len(),
        });
        self.complete_until = self.positions.len();
    }
    fn flush(
        &mut self,
        db: &mut Connection,
        check: &dyn Fn() -> Result<()>,
        kind: &str,
        limits: SearchBuildLimits,
        work: &mut u64,
    ) -> Result<()> {
        if self.blocks.is_empty() {
            return Ok(());
        }
        check()?;
        let tx = db.transaction()?;
        let mut count = 0u64;
        for block in &self.blocks {
            count = count
                .checked_add(write_posting_block(
                    &tx,
                    kind,
                    &block.gram[..usize::from(block.len)],
                    &self.positions[block.start..block.end],
                    check,
                )?)
                .ok_or(Error::Budget("search ordered postings"))?;
        }
        check()?;
        tx.commit()?;
        self.written = self
            .written
            .checked_add(count)
            .ok_or(Error::Budget("search ordered postings"))?;
        let pending = self.positions.len() - self.complete_until;
        charge(work, pending * 8, limits)?;
        self.positions.drain(..self.complete_until);
        self.complete_until = 0;
        self.blocks.clear();
        Ok(())
    }
    fn finish(
        &mut self,
        db: &mut Connection,
        check: &dyn Fn() -> Result<()>,
        kind: &str,
        limits: SearchBuildLimits,
        work: &mut u64,
    ) -> Result<u64> {
        self.complete();
        self.flush(db, check, kind, limits, work)?;
        Ok(self.written)
    }
}

fn merge_ordered_kind(
    db: &mut Connection,
    check: &dyn Fn() -> Result<()>,
    kind: &str,
    limits: SearchBuildLimits,
    first_run: i64,
    run_count: i64,
    expected: u64,
    work: &mut u64,
) -> Result<()> {
    if first_run < 0 || run_count < 0 {
        return Err(Error::Invalid("search run generation"));
    }
    let fan_in =
        i64::try_from(limits.gram_batch_rows.max(2)).map_err(|_| Error::Budget("search fan in"))?;
    let mut start = first_run;
    let mut count = run_count;
    while count > fan_in {
        let output_start = start
            .checked_add(count)
            .ok_or(Error::Budget("search run id"))?;
        let mut offset = 0i64;
        while offset < count {
            check()?;
            let size = (count - offset).min(fan_in);
            let output_id = output_start
                .checked_add(offset / fan_in)
                .ok_or(Error::Budget("search run id"))?;
            let mut writer = TempRunWriter::new(output_id);
            let group_start = start
                .checked_add(offset)
                .ok_or(Error::Budget("search run id"))?;
            merge_group(
                db,
                check,
                group_start,
                size as usize,
                limits,
                work,
                |db, block, work| writer.push_block(db, block, limits, work, check),
            )?;
            writer.finish(db, limits, work, check)?;
            let group_end = group_start
                .checked_add(size)
                .ok_or(Error::Budget("search run id"))?;
            for id in group_start..group_end {
                retire_run(db, check, id)?;
            }
            offset = offset
                .checked_add(size)
                .ok_or(Error::Budget("search run id"))?;
        }
        start = output_start;
        count = count / fan_in + i64::from(count % fan_in != 0);
    }
    let mut writer = FinalWriter::new()?;
    let copied = merge_group(
        db,
        check,
        start,
        count as usize,
        limits,
        work,
        |db, block, work| writer.push_block(db, check, kind, block, limits, work),
    )?;
    let written = writer.finish(db, check, kind, limits, work)?;
    if copied != expected || written != expected {
        return Err(Error::Invalid("search ordered posting total"));
    }
    let generation_end = start
        .checked_add(count)
        .ok_or(Error::Budget("search run id"))?;
    for id in start..generation_end {
        retire_run(db, check, id)?;
    }
    Ok(())
}

fn retire_run(db: &Connection, check: &dyn Fn() -> Result<()>, id: i64) -> Result<()> {
    let last: Option<i64> = db.query_row(
        "SELECT MAX(chunk_no) FROM search_run_chunks WHERE run_id=?1",
        [id],
        |row| row.get(0),
    )?;
    let Some(last) = last else {
        return Ok(());
    };
    let mut first = 0i64;
    while first <= last {
        check()?;
        let end = first
            .checked_add(MAX_GRAM_BATCH_ROWS as i64 - 1)
            .ok_or(Error::Budget("search run chunk number"))?;
        db.execute(
            "DELETE FROM search_run_chunks WHERE run_id=?1 AND chunk_no BETWEEN ?2 AND ?3",
            params![id, first, end],
        )?;
        first = end
            .checked_add(1)
            .ok_or(Error::Budget("search run chunk number"))?;
    }
    check()?;
    Ok(())
}

fn write_posting_block(
    transaction: &rusqlite::Transaction<'_>,
    kind: &str,
    gram: &[u8],
    positions: &[u64],
    check: &dyn Fn() -> Result<()>,
) -> Result<u64> {
    check()?;
    let (first, last, count, deltas) = encode_posting_block(positions)?;
    let inserted = transaction.execute(
        "INSERT INTO search_posting_blocks(kind,n,gram,last_position,first_position,postings,deltas) VALUES (?1,3,?2,?3,?4,?5,?6)",
        params![kind, gram, last as i64, first as i64, i64::from(count), deltas],
    )?;
    if inserted != 1 {
        return Err(Error::Invalid("search posting block insert"));
    }
    check()?;
    Ok(u64::from(count))
}

fn hash_field(hash: &mut Digest256Hasher, field: &[u8]) {
    hash.update(&(field.len() as u64).to_be_bytes());
    hash.update(field);
}

fn verify_and_root(
    db: &Connection,
    expected: &mut SearchIndexReceipt,
    limits: SearchBuildLimits,
) -> Result<(u64, u64, String)> {
    let mut hash = Digest256Hasher::new();
    hash_field(&mut hash, b"tos-knowledge-search-posting-blocks-v1");
    let mut counts = [0u64; 3];
    let mut document_positions = [0u64; 2];
    let mut logical_postings = 0u64;
    let mut previous_block: Option<(String, Vec<u8>, u64, u16)> = None;
    for (table_index, sql) in [
        "SELECT kind,position,id,source_graph,kind_id,predicate_id,id_lower,native_id_lower,identity_values,visible_values,document_chars,document_digest FROM search_documents ORDER BY kind,position",
        "SELECT kind,n,gram,last_position,first_position,postings,deltas FROM search_posting_blocks ORDER BY kind,n,gram,last_position",
        "SELECT kind,n,gram,postings FROM search_gram_stats ORDER BY kind,n,gram",
    ].iter().enumerate() {
        hash_field(&mut hash, &[table_index as u8]);
        let mut statement = db.prepare(sql)?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            counts[table_index] = counts[table_index]
                .checked_add(1).ok_or(Error::Budget("search root rows"))?;
            let kind: String = row.get(0)?;
            let kind_index = match kind.as_str() {
                "nodes" => 0,
                "relations" => 1,
                _ => return Err(Error::Invalid("search root kind")),
            };
            if table_index == 0 {
                let position: i64 = row.get(1)?;
                if position < 0 || position as u64 != document_positions[kind_index] {
                    return Err(Error::Invalid("search root document position"));
                }
                document_positions[kind_index] = document_positions[kind_index]
                    .checked_add(1).ok_or(Error::Budget("search document positions"))?;
                let digest: Vec<u8> = row.get(11)?;
                let chars: i64 = row.get(10)?;
                if digest.len() != 32 || chars < 0 {
                    return Err(Error::Invalid("search root document digest/chars"));
                }
            } else {
                let n: i64 = row.get(1)?;
                if n != GRAM_N { return Err(Error::Invalid("search root gram size")); }
                let gram: Vec<u8> = row.get(2)?;
                if std::str::from_utf8(&gram).ok().map(|s| s.chars().count()) != Some(GRAM_N as usize) {
                    return Err(Error::Invalid("search root gram code points"));
                }
                if table_index == 1 {
                    let last: i64 = row.get(3)?;
                    let first: i64 = row.get(4)?;
                    let count: i64 = row.get(5)?;
                    let deltas: Vec<u8> = row.get(6)?;
                    if first < 0 || last < 0 || count <= 0 || count > MAX_POSTINGS_PER_BLOCK as i64 {
                        return Err(Error::Invalid("search root block shape"));
                    }
                    let positions = decode_posting_block(first as u64, last as u64, count as u16, &deltas)?;
                    charge(&mut expected.work_bytes, deltas.len(), limits)?;
                    charge(&mut expected.work_bytes, positions.len() * 8, limits)?;
                    charge(&mut expected.work_bytes, gram.len() + 24, limits)?;
                    if positions.iter().any(|position| *position >= document_positions[kind_index]) {
                        return Err(Error::Invalid("search root orphan posting"));
                    }
                    if let Some((prior_kind, prior_gram, prior_last, prior_count)) = &previous_block {
                        if prior_kind == &kind && prior_gram == &gram {
                            if *prior_count as usize != MAX_POSTINGS_PER_BLOCK || first as u64 <= *prior_last {
                                return Err(Error::Invalid("search root block partition"));
                            }
                        }
                    }
                    previous_block = Some((kind.clone(), gram, last as u64, count as u16));
                    logical_postings = logical_postings
                        .checked_add(count as u64)
                        .ok_or(Error::Budget("search root postings"))?;
                } else {
                    let value: i64 = row.get(3)?;
                    if value <= 0 { return Err(Error::Invalid("search root stat count")); }
                }
            }
            for col in 0..row.as_ref().column_count() {
                match row.get_ref(col)? {
                    rusqlite::types::ValueRef::Text(bytes) | rusqlite::types::ValueRef::Blob(bytes) => hash_field(&mut hash, bytes),
                    rusqlite::types::ValueRef::Integer(value) => hash_field(&mut hash, &value.to_be_bytes()),
                    _ => return Err(Error::Invalid("search root SQL value")),
                }
            }
        }
    }
    if document_positions != [expected.node_documents, expected.relation_documents]
        || counts[0]
            != expected
                .node_documents
                .checked_add(expected.relation_documents)
                .ok_or(Error::Budget("search documents"))?
        || logical_postings != expected.postings
    {
        return Err(Error::Invalid("search root table coverage"));
    }
    let mut grouped = db.prepare("SELECT kind,n,gram,SUM(postings) FROM search_posting_blocks GROUP BY kind,n,gram ORDER BY kind,n,gram")?;
    let mut actual = grouped.query([])?;
    let mut stats =
        db.prepare("SELECT kind,n,gram,postings FROM search_gram_stats ORDER BY kind,n,gram")?;
    let mut declared = stats.query([])?;
    while let Some(row) = declared.next()? {
        let group = actual
            .next()?
            .ok_or(Error::Invalid("search gram stats extra"))?;
        let left: (String, i64, Vec<u8>, i64) =
            (row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?);
        let right: (String, i64, Vec<u8>, i64) =
            (group.get(0)?, group.get(1)?, group.get(2)?, group.get(3)?);
        if left != right {
            return Err(Error::Invalid("search gram stats coverage"));
        }
    }
    if actual.next()?.is_some() {
        return Err(Error::Invalid("search gram stats absent"));
    }
    Ok((logical_postings, counts[2], hash.finalize().to_hex()))
}

const SCHEMA: &str = r#"
CREATE TABLE search_documents(
 kind TEXT NOT NULL, position INTEGER NOT NULL,id TEXT NOT NULL,
 source_graph TEXT NOT NULL,kind_id TEXT NOT NULL,predicate_id TEXT NOT NULL,
 id_lower TEXT NOT NULL,native_id_lower TEXT NOT NULL,
 identity_values TEXT NOT NULL,visible_values TEXT NOT NULL,
 document_chars INTEGER NOT NULL,document_digest BLOB NOT NULL,
 PRIMARY KEY(kind,position)) WITHOUT ROWID;
CREATE TABLE search_posting_blocks(
 kind TEXT NOT NULL,n INTEGER NOT NULL,gram BLOB NOT NULL,
 last_position INTEGER NOT NULL,first_position INTEGER NOT NULL,
 postings INTEGER NOT NULL,deltas BLOB NOT NULL,
 PRIMARY KEY(kind,n,gram,last_position)) WITHOUT ROWID;
CREATE TABLE search_gram_stats(
 kind TEXT NOT NULL,n INTEGER NOT NULL,gram BLOB NOT NULL,postings INTEGER NOT NULL,
 PRIMARY KEY(kind,n,gram)) WITHOUT ROWID;
CREATE TEMP TABLE search_run_chunks(
 run_id INTEGER NOT NULL,chunk_no INTEGER NOT NULL,
 gram BLOB NOT NULL,first_position INTEGER NOT NULL,last_position INTEGER NOT NULL,
 postings INTEGER NOT NULL,deltas BLOB NOT NULL,
 PRIMARY KEY(run_id,chunk_no)) WITHOUT ROWID;
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn limits() -> SearchBuildLimits {
        SearchBuildLimits {
            max_payload_bytes: 4096,
            max_document_chars: 4096,
            max_document_bytes: 8192,
            max_rank_field_bytes: 4096,
            max_postings: 1000,
            max_work_bytes: 1_000_000,
            gram_batch_rows: 2,
        }
    }

    #[test]
    fn python_oracle_spaced_json_unicode_and_rank_value_order() {
        // Independent CPython 3.14/Unicode 16 `json.dumps(...,sort_keys=True)`
        // and `_rank_fields` oracle. Input dict rank values are es,de; sorted
        // document keys are de,es. Neither order may be reused for the other.
        let payload = br#"{"id":"N-\u03a3","source_graph":"g","native_id":"Stra\u00dfe","kind_id":"k","display":{"title":{"es":"\u00c1RBOL","de":"Stra\u00dfe"},"summary":"\u039f\u03a3"},"punctuation":"a,b:c"}"#;
        let row = SourceRow {
            position: 0,
            id: "N-Σ".into(),
            source_graph: "g".into(),
            native_id: Some("Straße".into()),
            term_id: "k".into(),
            payload_len: payload.len() as i64,
            payload_sha256: Digest256::of_bytes(payload).as_bytes().to_vec(),
            payload: Some(payload.to_vec()),
        };
        let got = document(&row, "nodes", payload, limits()).expect("oracle carrier");
        assert_eq!(
            got.text,
            "{\"display\": {\"summary\": \"ος\", \"title\": {\"de\": \"straße\", \"es\": \"árbol\"}}, \"id\": \"n-σ\", \"kind_id\": \"k\", \"native_id\": \"straße\", \"punctuation\": \"a,b:c\", \"source_graph\": \"g\"}"
        );
        assert_eq!(got.chars, 169);
        assert_eq!(
            got.digest.to_hex(),
            "39c0f3c2a308c5da0f6aaf3a3d01a6787fdfe20f49ec0566188c41f9c7f1bd68"
        );
        assert_eq!(got.identity_values, "[\"árbol\",\"straße\"]");
        assert_eq!(got.visible_values, "[\"árbol\",\"straße\",\"ος\"]");
    }

    #[test]
    fn ordered_posting_pages_preserve_unicode_and_rollback_boundaries() {
        let read_postings = |db: &Connection| {
            let mut statement = db.prepare("SELECT gram,first_position,last_position,postings,deltas FROM search_posting_blocks ORDER BY kind,n,gram,last_position").unwrap();
            let mut rows = statement.query([]).unwrap();
            let mut postings = Vec::new();
            while let Some(row) = rows.next().unwrap() {
                let gram: Vec<u8> = row.get(0).unwrap();
                let first: i64 = row.get(1).unwrap();
                let last: i64 = row.get(2).unwrap();
                let count: i64 = row.get(3).unwrap();
                let deltas: Vec<u8> = row.get(4).unwrap();
                for position in
                    decode_posting_block(first as u64, last as u64, count as u16, &deltas).unwrap()
                {
                    postings.push((gram.clone(), position as i64));
                }
            }
            postings
        };
        let tick = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("tos-search-ordered-{}-{tick}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let mut db = Connection::open(dir.join("candidate.sqlite3")).unwrap();
        let _vm_used = crate::sqlite_budget::configure(
            &db,
            crate::Limits {
                max_output_bytes: 32 * 1024 * 1024,
                sqlite_cache_kib: 512,
                max_sql_vm_steps: 20_000_000,
                ..crate::Limits::default()
            },
        )
        .unwrap();
        let temp_store: i64 = db
            .query_row("PRAGMA temp_store", [], |row| row.get(0))
            .unwrap();
        assert_eq!(temp_store, 1);
        crate::knowledge_stage::configure_stage_temp_reclamation(&db).unwrap();
        let initial_mode: i64 = db
            .query_row("PRAGMA temp.auto_vacuum", [], |row| row.get(0))
            .unwrap();
        assert_eq!(initial_mode, 2);
        db.execute_batch(
            "SAVEPOINT prior_catalog_temp;
             CREATE TEMP TABLE prior_catalog_rows(value INTEGER);
             INSERT INTO prior_catalog_rows VALUES(1);
             ROLLBACK TO prior_catalog_temp;
             RELEASE prior_catalog_temp;",
        )
        .unwrap();
        initialize_search_storage(&db).unwrap();
        let make_row = |position, id: &str| {
            let payload = format!(
                r#"{{"id":"{id}","source_graph":"g","kind_id":"k","display":{{"title":"aaaaaaaaaá🌳ßá🌳ßá🌳ßbbbccc"}}}}"#
            )
            .into_bytes();
            SourceRow {
                position,
                id: id.into(),
                source_graph: "g".into(),
                native_id: None,
                term_id: "k".into(),
                payload_len: payload.len() as i64,
                payload_sha256: Digest256::of_bytes(&payload).as_bytes().to_vec(),
                payload: Some(payload),
            }
        };
        let mut receipt = SearchIndexReceipt {
            profile: SEARCH_PROFILE,
            node_documents: 0,
            relation_documents: 0,
            postings: 0,
            distinct_grams: 0,
            document_chars: 0,
            work_bytes: 0,
            search_index_root_sha256: String::new(),
        };
        let prepare = |position, id: &str, receipt: &mut SearchIndexReceipt| {
            let mut row = make_row(position, id);
            let doc = document(&row, "nodes", row.payload.as_deref().unwrap(), limits()).unwrap();
            let expected = doc
                .text
                .chars()
                .collect::<Vec<_>>()
                .windows(3)
                .map(|window| window.iter().collect::<String>().into_bytes())
                .collect::<std::collections::BTreeSet<_>>();
            assert!(doc.text.matches("aaa").count() > limits().gram_batch_rows);
            assert!(expected.len() > limits().gram_batch_rows);
            assert!(expected.contains("á🌳ß".as_bytes()));
            assert!(expected.contains(&b"bbb"[..]));
            assert!(
                expected
                    .iter()
                    .take_while(|gram| gram.as_slice() < &b"bbb"[..])
                    .count()
                    > limits().gram_batch_rows
            );
            let offsets =
                prepare_gram_offsets(&doc, limits(), &mut receipt.work_bytes, &|| Ok(())).unwrap();
            assert_eq!(offsets.len(), expected.len());
            row.payload = None;
            (PreparedDocument { row, doc, offsets }, expected)
        };
        let (first, expected_first) = prepare(7, "n7", &mut receipt);
        let (second, expected_second) = prepare(8, "n8", &mut receipt);
        assert!(expected_first.len() >= 8);
        assert_eq!(expected_first.len(), expected_second.len());
        let mut writer_limits = limits();
        writer_limits.gram_batch_rows = (expected_first.len() - 1) / 2;
        assert_eq!(expected_first.len() / writer_limits.gram_batch_rows, 2);
        assert_ne!(expected_first.len() % writer_limits.gram_batch_rows, 0);
        write_document_page(
            &mut db,
            &|| Ok(()),
            &[first, second],
            "nodes",
            7,
            writer_limits,
            &mut receipt,
        )
        .unwrap();
        let expected_total = (expected_first.len() + expected_second.len()) as u64;
        assert_eq!(receipt.postings, expected_total);
        assert_eq!(receipt.node_documents, 2);
        merge_ordered_kind(
            &mut db,
            &|| Ok(()),
            "nodes",
            writer_limits,
            7,
            1,
            expected_total,
            &mut receipt.work_bytes,
        )
        .unwrap();
        let expected_order = expected_first
            .iter()
            .map(|gram| (gram.clone(), 7i64))
            .chain(expected_second.iter().map(|gram| (gram.clone(), 8i64)))
            .collect::<std::collections::BTreeSet<_>>();
        let actual_order = read_postings(&db);
        assert_eq!(actual_order, expected_order.into_iter().collect::<Vec<_>>());
        for (position, expected) in [(7, expected_first), (8, expected_second)] {
            let actual = actual_order
                .iter()
                .filter(|(_, p)| *p == position)
                .map(|(gram, _)| gram.clone())
                .collect::<Vec<_>>();
            assert_eq!(actual, expected.into_iter().collect::<Vec<_>>());
        }
        let repeated = actual_order
            .iter()
            .filter(|(gram, _)| gram == b"aaa")
            .count();
        assert_eq!(repeated, 2);
        let committed_chars = receipt.document_chars;

        // A cap refusal after bounded inserts rolls back its whole document
        // page and cannot advance the committed receipt.
        let (capped, _) = prepare(9, "n9", &mut receipt);
        let mut tight = limits();
        tight.max_postings = receipt.postings + 3;
        assert!(matches!(
            write_document_page(
                &mut db,
                &|| Ok(()),
                &[capped],
                "nodes",
                9,
                tight,
                &mut receipt
            ),
            Err(Error::Budget("search postings"))
        ));
        db.execute_batch(
            "CREATE TEMP TRIGGER refuse_late_staging BEFORE INSERT ON search_run_chunks
             WHEN NEW.run_id=9 AND NEW.chunk_no>0
             BEGIN SELECT RAISE(ABORT,'refuse late staging'); END;",
        )
        .unwrap();
        let (retry, _) = prepare(9, "n9", &mut receipt);
        let (refused, _) = prepare(10, "n10", &mut receipt);
        assert!(
            write_document_page(
                &mut db,
                &|| Ok(()),
                &[retry, refused],
                "nodes",
                9,
                limits(),
                &mut receipt,
            )
            .is_err()
        );
        db.execute_batch("DROP TRIGGER refuse_late_staging")
            .unwrap();
        let (guarded, _) = prepare(11, "n11", &mut receipt);
        let checks = std::cell::Cell::new(0);
        let before_guard_changes = db.total_changes();
        assert!(matches!(
            write_document_page(
                &mut db,
                &|| {
                    checks.set(checks.get() + 1);
                    if checks.get() == 5 {
                        Err(Error::Invalid("fixture late guard"))
                    } else {
                        Ok(())
                    }
                },
                &[guarded],
                "nodes",
                11,
                limits(),
                &mut receipt,
            ),
            Err(Error::Invalid("fixture late guard"))
        ));
        // The injected check is useful only after a real SQL write. SQLite's
        // connection total includes completed INSERTs even when rolled back.
        assert!(db.total_changes() > before_guard_changes + 1);
        assert!(db.is_autocommit());
        let failed_documents: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM search_documents WHERE position IN (9,10,11)",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let failed_staging: i64 = db
            .query_row("SELECT COUNT(*) FROM search_run_chunks", [], |r| r.get(0))
            .unwrap();
        assert_eq!((failed_documents, failed_staging), (0, 0));
        assert_eq!(receipt.postings, expected_total);
        assert_eq!(receipt.node_documents, 2);
        assert_eq!(receipt.document_chars, committed_chars);

        // A failed final block transaction retains earlier committed blocks,
        // while the private source run remains for stage poison/discard.
        let (late, expected_late) = prepare(12, "n12", &mut receipt);
        assert!(expected_late.len() <= MAX_GRAM_BATCH_ROWS);
        write_document_page(
            &mut db,
            &|| Ok(()),
            &[late],
            "nodes",
            12,
            limits(),
            &mut receipt,
        )
        .unwrap();
        db.execute_batch(
            "CREATE TRIGGER refuse_late_final BEFORE INSERT ON search_posting_blocks
             WHEN NEW.first_position=12 AND NEW.gram=X'626262'
             BEGIN SELECT RAISE(ABORT,'refuse late final'); END;",
        )
        .unwrap();
        assert!(
            merge_ordered_kind(
                &mut db,
                &|| Ok(()),
                "nodes",
                limits(),
                12,
                1,
                expected_late.len() as u64,
                &mut receipt.work_bytes,
            )
            .is_err()
        );
        assert!(db.is_autocommit());
        let prior_postings = read_postings(&db)
            .iter()
            .filter(|(_, p)| *p == 7 || *p == 8)
            .count();
        assert_eq!(prior_postings as u64, expected_total);
        let refused_final = read_postings(&db)
            .iter()
            .filter(|(gram, p)| gram == b"bbb" && *p == 12)
            .count();
        let retained_stage: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM search_run_chunks WHERE run_id=12",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(refused_final, 0);
        assert!(retained_stage > 0);
        db.execute_batch("DROP TRIGGER refuse_late_final").unwrap();
        merge_ordered_kind(
            &mut db,
            &|| Ok(()),
            "nodes",
            limits(),
            12,
            1,
            expected_late.len() as u64,
            &mut receipt.work_bytes,
        )
        .unwrap();
        // The real ordered-copy writer must close a full block before the
        // same gram's final one-position block, even across tiny SQL pages.
        for (run_id, range) in [(20, 20..106), (21, 106..192), (22, 192..277)] {
            for (chunk_no, positions) in range
                .collect::<Vec<_>>()
                .chunks(limits().gram_batch_rows)
                .enumerate()
            {
                let block = positions
                    .iter()
                    .map(|position| *position as u64)
                    .collect::<Vec<_>>();
                insert_run_block(
                    &db,
                    run_id,
                    chunk_no,
                    b"zzz",
                    &block,
                    limits(),
                    &mut receipt.work_bytes,
                )
                .unwrap();
            }
        }
        merge_ordered_kind(
            &mut db,
            &|| Ok(()),
            "nodes",
            limits(),
            20,
            3,
            257,
            &mut receipt.work_bytes,
        )
        .unwrap();
        let blocks = db
            .prepare("SELECT first_position,last_position,postings,deltas FROM search_posting_blocks WHERE gram=X'7a7a7a' ORDER BY last_position")
            .unwrap()
            .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?, row.get::<_, Vec<u8>>(3)?)))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(blocks.len(), 2);
        assert_eq!((blocks[0].0, blocks[0].1, blocks[0].2), (20, 275, 256));
        assert_eq!((blocks[1].0, blocks[1].1, blocks[1].2), (276, 276, 1));
        assert_eq!(
            decode_posting_block(20, 275, 256, &blocks[0].3).unwrap(),
            (20..276).collect::<Vec<_>>()
        );
        let mut nonminimal = blocks[0].3.clone();
        nonminimal[0] = 0x81;
        nonminimal.insert(1, 0);
        assert!(decode_posting_block(20, 275, 256, &nonminimal).is_err());
        let mut trailing = blocks[1].3.clone();
        trailing.push(0);
        assert!(decode_posting_block(276, 276, 1, &trailing).is_err());
        let remaining_runs: i64 = db
            .query_row("SELECT COUNT(*) FROM search_run_chunks", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(remaining_runs, 0);
        retire_search_staging(&db).unwrap();
        let freelist: i64 = db
            .query_row("PRAGMA temp.freelist_count", [], |row| row.get(0))
            .unwrap();
        assert_eq!(freelist, 0);
        drop(db);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rank_arrays_are_text_for_pinned_sqlite_json1() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(SCHEMA).unwrap();
        db.execute(
            "INSERT INTO search_documents VALUES
             ('nodes',0,'n','g','k','', 'n','n',?1,?2,3,zeroblob(32))",
            params!["[\"árbol\"]", "[\"árbol\",\"ος\"]"],
        )
        .unwrap();
        let (kind, matches): (String, i64) = db
            .query_row(
                "SELECT typeof(identity_values),
                    (SELECT count(*) FROM json_each(identity_values) WHERE value='árbol')
             FROM search_documents",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(kind, "text");
        assert_eq!(matches, 1);
    }

    #[test]
    fn oversized_document_and_lone_surrogate_refuse() {
        assert!(default_spaced_json(br#"{"a":"b"}"#, 8).is_err());
        let payload =
            br#"{"id":"x","source_graph":"g","kind_id":"k","display":{"title":"\ud800"}}"#;
        let row = SourceRow {
            position: 0,
            id: "x".into(),
            source_graph: "g".into(),
            native_id: None,
            term_id: "k".into(),
            payload_len: payload.len() as i64,
            payload_sha256: Digest256::of_bytes(payload).as_bytes().to_vec(),
            payload: Some(payload.to_vec()),
        };
        assert!(document(&row, "nodes", payload, limits()).is_err());
    }

    #[test]
    fn malformed_or_mismatched_carriers_refuse_before_indexing() {
        let row = SourceRow {
            position: 0,
            id: "n".into(),
            source_graph: "g".into(),
            native_id: None,
            term_id: "k".into(),
            payload_len: 0,
            payload_sha256: vec![0; 32],
            payload: None,
        };
        for payload in [
            br#"{"id":"wrong","source_graph":"g","kind_id":"k"}"#.as_slice(),
            br#"{"id":"n","id":"n","source_graph":"g","kind_id":"k"}"#,
            br#"{"id":"n","source_graph":"g","kind_id":"wrong"}"#,
        ] {
            assert!(document(&row, "nodes", payload, limits()).is_err());
        }
    }
}
