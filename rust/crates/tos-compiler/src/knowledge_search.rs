//! Complete indexed-v2 substring carrier over the selected normalized graph.
//! SQLite owns the gram deduplication and external ordering; the stage owns
//! the private candidate and independently guarded spill namespace. The
//! caller's `StageLimits` supplies cumulative SQLite VM steps, page/output,
//! cache and host-enforced temporary-file caps. `SearchBuildLimits` supplies
//! additional row, posting and work caps. No in-process temp-size sample is
//! claimed to bound a single SQLite statement's external spill.

use crate::{
    Error, Result,
    knowledge_stage::{KnowledgeStage, WritePhase},
};
use rusqlite::{Connection, OptionalExtension, params, params_from_iter};
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
    /// Limits one dedup/copy SQL callback and its Rust batch.
    pub gram_batch_rows: usize,
}

impl SearchBuildLimits {
    fn validate(self) -> Result<()> {
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

struct SourceRow {
    position: i64,
    id: String,
    source_graph: String,
    native_id: Option<String>,
    term_id: String,
    payload_len: i64,
    payload_sha256: Vec<u8>,
    payload: Option<Vec<u8>>,
}

struct Document {
    id_lower: String,
    native_id_lower: String,
    identity_values: String,
    visible_values: String,
    text: String,
    chars: usize,
    digest: Digest256,
    serialization_bytes: usize,
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

fn build_inner(
    stage: &mut KnowledgeStage<'_>,
    limits: SearchBuildLimits,
) -> Result<SearchIndexReceipt> {
    limits.validate()?;
    stage.with_connection(WritePhase::Search, |db| {
        db.execute_batch(SCHEMA).map_err(Error::from)
    })?;
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
        loop {
            let row = stage.with_connection(WritePhase::Search, |db| {
                fetch_next(db, table, after, limits.max_payload_bytes)
            })?;
            let Some(row) = row else {
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
            receipt.document_chars = receipt
                .document_chars
                .checked_add(doc.chars as u64)
                .ok_or(Error::Budget("search document characters"))?;
            stage.with_connection(WritePhase::Search, |db| {
                db.execute(
                    "INSERT INTO search_documents(kind,position,id,source_graph,kind_id,predicate_id,id_lower,native_id_lower,identity_values,visible_values,document_chars,document_digest) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
                    params![kind,row.position,row.id,row.source_graph,
                        if kind == "nodes" { row.term_id.as_str() } else { "" },
                        if kind == "relations" { row.term_id.as_str() } else { "" },
                        doc.id_lower,doc.native_id_lower,doc.identity_values,doc.visible_values,
                        doc.chars as i64,doc.digest.as_bytes().as_slice()],
                )?;
                db.execute("DELETE FROM search_pending_grams", [])?;
                Ok(())
            })?;
            let mut iterator = doc.text.chars();
            let (Some(mut a), Some(mut b)) = (iterator.next(), iterator.next()) else {
                increment_documents(&mut receipt, kind)?;
                continue;
            };
            let mut batch: Vec<Vec<u8>> = Vec::with_capacity(limits.gram_batch_rows);
            for c in iterator {
                let mut gram = String::with_capacity(a.len_utf8() + b.len_utf8() + c.len_utf8());
                gram.push(a);
                gram.push(b);
                gram.push(c);
                charge(&mut receipt.work_bytes, gram.len(), limits)?;
                batch.push(gram.into_bytes());
                if batch.len() == limits.gram_batch_rows {
                    insert_pending(stage, &batch)?;
                    batch.clear();
                }
                a = b;
                b = c;
            }
            if !batch.is_empty() {
                insert_pending(stage, &batch)?;
            }
            let document_postings: u64 = stage.with_connection(WritePhase::Search, |db| {
                Ok(
                    db.query_row("SELECT COUNT(*) FROM search_pending_grams", [], |r| {
                        r.get(0)
                    })?,
                )
            })?;
            receipt.postings = receipt
                .postings
                .checked_add(document_postings)
                .ok_or(Error::Budget("search postings"))?;
            if receipt.postings > limits.max_postings {
                return Err(Error::Budget("search postings"));
            }
            let mut last_gram: Option<Vec<u8>> = None;
            loop {
                let next = stage.with_connection(WritePhase::Search, |db| {
                    copy_pending_page(
                        db,
                        kind,
                        row.position,
                        last_gram.as_deref(),
                        limits.gram_batch_rows,
                    )
                })?;
                let Some(next) = next else {
                    break;
                };
                last_gram = Some(next);
            }
            increment_documents(&mut receipt, kind)?;
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
    }
    // Grouping is an external SQLite index scan under the stage VM and host
    // spill quota. A cap failure poisons and removes the private candidate.
    stage.with_connection(WritePhase::Search, |db| {
        db.execute("INSERT INTO search_gram_stats(kind,n,gram,postings) SELECT kind,n,gram,COUNT(*) FROM search_grams GROUP BY kind,n,gram", [])?;
        db.execute("CREATE INDEX search_document_filter ON search_documents(kind,source_graph,kind_id,predicate_id,position)", [])?;
        db.execute("DROP TABLE search_pending_grams", [])?;
        Ok(())
    })?;
    let (postings, distinct, root) =
        stage.with_connection(WritePhase::Search, |db| verify_and_root(db, &receipt))?;
    if postings != receipt.postings {
        return Err(Error::Invalid("search posting coverage"));
    }
    receipt.distinct_grams = distinct;
    receipt.search_index_root_sha256 = root;
    Ok(receipt)
}

fn increment_documents(receipt: &mut SearchIndexReceipt, kind: &str) -> Result<()> {
    let target = if kind == "nodes" {
        &mut receipt.node_documents
    } else {
        &mut receipt.relation_documents
    };
    *target = target
        .checked_add(1)
        .ok_or(Error::Budget("search document rows"))?;
    Ok(())
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

fn document(
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

fn insert_pending(stage: &mut KnowledgeStage<'_>, batch: &[Vec<u8>]) -> Result<()> {
    stage.with_connection(WritePhase::Search, |db| insert_pending_batch(db, batch))
}

fn insert_pending_batch(db: &mut Connection, batch: &[Vec<u8>]) -> Result<()> {
    if batch.len() > MAX_GRAM_BATCH_ROWS {
        return Err(Error::Budget("search gram batch rows"));
    }
    // Attempted grams were already charged, including duplicates. Avoid
    // spending SQL VM instructions on duplicate keys within this bounded
    // batch; the existing PK still deduplicates across all document batches.
    let mut unique = batch.iter().map(Vec::as_slice).collect::<Vec<_>>();
    unique.sort_unstable();
    unique.dedup();
    if unique.is_empty() {
        return Ok(());
    }
    let transaction = db.transaction()?;
    // One bounded statement avoids restarting a VM program for every gram.
    // Only placeholders and the fixed format gram size enter the SQL text;
    // every borrowed gram remains a bound blob and the PK owns cross-batch dedup.
    let sql = format!(
        "INSERT OR IGNORE INTO search_pending_grams(n,gram) VALUES {}",
        vec![format!("({GRAM_N},?)"); unique.len()].join(",")
    );
    transaction.execute(&sql, params_from_iter(unique))?;
    transaction.commit()?;
    Ok(())
}

fn copy_pending_page(
    db: &mut Connection,
    kind: &str,
    position: i64,
    after: Option<&[u8]>,
    rows_cap: usize,
) -> Result<Option<Vec<u8>>> {
    if rows_cap == 0 || rows_cap > MAX_GRAM_BATCH_ROWS {
        return Err(Error::Budget("search gram page rows"));
    }
    // Keep continuation as a direct (n,gram) primary-key seek. A nullable
    // disjunction can revisit the whole n-prefix on every bounded page.
    let sql = if after.is_some() {
        "SELECT gram FROM search_pending_grams WHERE n=?1 AND gram>?2 ORDER BY gram LIMIT ?3"
    } else {
        "SELECT gram FROM search_pending_grams WHERE n=?1 ORDER BY gram LIMIT ?2"
    };
    let mut statement = db.prepare(sql)?;
    let mut rows = if let Some(after) = after {
        statement.query(params![GRAM_N, after, rows_cap as i64])?
    } else {
        statement.query(params![GRAM_N, rows_cap as i64])?
    };
    let mut grams = Vec::with_capacity(rows_cap);
    while let Some(row) = rows.next()? {
        grams.push(row.get::<_, Vec<u8>>(0)?);
    }
    drop(rows);
    drop(statement);
    if grams.is_empty() {
        return Ok(None);
    }
    // Read only one bounded keyset page before opening its write transaction.
    // Any statement error rolls back this page; with_connection poisons the
    // private stage so previously committed pages cannot become a candidate.
    let transaction = db.transaction()?;
    let copied = if let Some(after) = after {
        transaction.execute(
            "INSERT INTO search_grams(kind,n,gram,position) SELECT ?1,n,gram,?2 FROM search_pending_grams WHERE n=?3 AND gram>?4 ORDER BY gram LIMIT ?5",
            params![kind, position, GRAM_N, after, rows_cap as i64],
        )?
    } else {
        transaction.execute(
            "INSERT INTO search_grams(kind,n,gram,position) SELECT ?1,n,gram,?2 FROM search_pending_grams WHERE n=?3 ORDER BY gram LIMIT ?4",
            params![kind, position, GRAM_N, rows_cap as i64],
        )?
    };
    if copied != grams.len() {
        return Err(Error::Invalid("search gram page copy coverage"));
    }
    transaction.commit()?;
    Ok(grams.pop())
}

fn hash_field(hash: &mut Digest256Hasher, field: &[u8]) {
    hash.update(&(field.len() as u64).to_be_bytes());
    hash.update(field);
}

fn verify_and_root(db: &Connection, expected: &SearchIndexReceipt) -> Result<(u64, u64, String)> {
    let mut hash = Digest256Hasher::new();
    hash_field(&mut hash, b"tos-knowledge-search-index-v1");
    let mut counts = [0u64; 3];
    let mut document_positions = [0u64; 2];
    for (table_index, sql) in [
        "SELECT kind,position,id,source_graph,kind_id,predicate_id,id_lower,native_id_lower,identity_values,visible_values,document_chars,document_digest FROM search_documents ORDER BY kind,position",
        "SELECT kind,n,gram,position FROM search_grams ORDER BY kind,n,gram,position",
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
                let value: i64 = row.get(3)?;
                if (table_index == 1 && value < 0) || (table_index == 2 && value <= 0) {
                    return Err(Error::Invalid("search root posting field"));
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
        || counts[1] != expected.postings
    {
        return Err(Error::Invalid("search root table coverage"));
    }
    let sum: Option<i64> =
        db.query_row("SELECT SUM(postings) FROM search_gram_stats", [], |r| {
            r.get(0)
        })?;
    if sum.unwrap_or(0) < 0 || sum.unwrap_or(0) as u64 != counts[1] {
        return Err(Error::Invalid("search gram stats coverage"));
    }
    let orphans: i64 = db.query_row(
        "SELECT COUNT(*) FROM search_grams g LEFT JOIN search_documents d ON d.kind=g.kind AND d.position=g.position WHERE d.position IS NULL",
        [], |r| r.get(0),
    )?;
    if orphans != 0 {
        return Err(Error::Invalid("search orphan posting"));
    }
    Ok((counts[1], counts[2], hash.finalize().to_hex()))
}

const SCHEMA: &str = r#"
CREATE TABLE search_documents(
 kind TEXT NOT NULL, position INTEGER NOT NULL,id TEXT NOT NULL,
 source_graph TEXT NOT NULL,kind_id TEXT NOT NULL,predicate_id TEXT NOT NULL,
 id_lower TEXT NOT NULL,native_id_lower TEXT NOT NULL,
 identity_values TEXT NOT NULL,visible_values TEXT NOT NULL,
 document_chars INTEGER NOT NULL,document_digest BLOB NOT NULL,
 PRIMARY KEY(kind,position)) WITHOUT ROWID;
CREATE TABLE search_grams(
 kind TEXT NOT NULL,n INTEGER NOT NULL,gram BLOB NOT NULL,position INTEGER NOT NULL,
 PRIMARY KEY(kind,n,gram,position)) WITHOUT ROWID;
CREATE TABLE search_gram_stats(
 kind TEXT NOT NULL,n INTEGER NOT NULL,gram BLOB NOT NULL,postings INTEGER NOT NULL,
 PRIMARY KEY(kind,n,gram)) WITHOUT ROWID;
CREATE TEMP TABLE search_pending_grams(
 n INTEGER NOT NULL,gram BLOB NOT NULL,PRIMARY KEY(n,gram)) WITHOUT ROWID;
"#;

#[cfg(test)]
mod tests {
    use super::*;

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
    fn gram_copy_is_disk_deduped_and_keyset_paged() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch(SCHEMA).unwrap();
        let pending = [
            b"aaa".to_vec(),
            b"bbb".to_vec(),
            b"aaa".to_vec(),
            b"ccc".to_vec(),
        ];
        insert_pending_batch(&mut db, &pending).unwrap();
        // The SQL primary key still owns dedup across separate batches.
        insert_pending_batch(&mut db, &pending[..1]).unwrap();
        let first = copy_pending_page(&mut db, "nodes", 7, None, 2)
            .unwrap()
            .unwrap();
        assert_eq!(first, b"bbb");
        let second = copy_pending_page(&mut db, "nodes", 7, Some(&first), 2)
            .unwrap()
            .unwrap();
        assert_eq!(second, b"ccc");
        assert!(
            copy_pending_page(&mut db, "nodes", 7, Some(&second), 2)
                .unwrap()
                .is_none()
        );
        let count: i64 = db
            .query_row("SELECT COUNT(*) FROM search_grams", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 3);
        db.execute_batch(
            "CREATE TRIGGER refuse_second_posting BEFORE INSERT ON search_grams
             WHEN NEW.position=8 AND NEW.gram=X'626262'
             BEGIN SELECT RAISE(ABORT,'refuse second posting'); END;",
        )
        .unwrap();
        assert!(copy_pending_page(&mut db, "nodes", 8, None, 2).is_err());
        assert!(db.is_autocommit());
        let refused_page_rows: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM search_grams WHERE position=8",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(refused_page_rows, 0);
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
