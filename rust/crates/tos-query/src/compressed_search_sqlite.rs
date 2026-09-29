//! Storage-v3 indexed seeks and incremental exact verification. No writer,
//! graph load, alternate profile, or full aggregate preflight is used here.
use crate::compressed_search_state::*;
use rusqlite::{Connection, Row, ToSql};
use std::sync::{
    Arc,
    atomic::{AtomicU8, AtomicU64, Ordering},
};
use tos_compiler::local_prepared::{PreparedReadLimits, PreparedReadTransaction};
use tos_foundation::{Digest256, Digest256Hasher, JsonMode, JsonValue, parse_json};

const MAX_ROW_BYTES: usize = 1_900_000;
const MAX_DOCUMENT_BYTES: u64 = 8 * 1024 * 1024;
const CHUNK: usize = 32_768;

pub(crate) struct Read<'a> {
    pub db: &'a Connection,
    pub limits: PreparedReadLimits,
    pub rows: usize,
    pub bytes: usize,
    steps: Arc<AtomicU64>,
    owner_rows: usize,
    owner_bytes: usize,
    owner_statements: u64,
    abort: Option<Arc<dyn crate::AbortProbe>>,
    abort_reason: Arc<AtomicU8>,
}
impl<'a> Read<'a> {
    pub fn new(db: &'a Connection, limits: PreparedReadLimits) -> Result<Self> {
        Self::with_abort(db, limits, None)
    }
    pub fn with_abort(
        db: &'a Connection,
        limits: PreparedReadLimits,
        abort: Option<Arc<dyn crate::AbortProbe>>,
    ) -> Result<Self> {
        if db.is_autocommit() {
            return Err(unavailable(
                "an already-open prepared read transaction is required",
            ));
        }
        let steps = Arc::new(AtomicU64::new(0));
        let observed = steps.clone();
        let max = limits.max_vm_steps;
        let abort_reason = Arc::new(AtomicU8::new(0));
        let observed_reason = abort_reason.clone();
        let progress_abort = abort.clone();
        db.progress_handler(
            100,
            Some(move || {
                if let Some(reason) = progress_abort
                    .as_deref()
                    .and_then(crate::AbortProbe::reason)
                {
                    observed_reason.store(
                        match reason {
                            crate::AbortReason::Cancelled => 1,
                            crate::AbortReason::DeadlineExceeded => 2,
                        },
                        Ordering::Relaxed,
                    );
                    return true;
                }
                observed
                    .fetch_add(100, Ordering::Relaxed)
                    .saturating_add(100)
                    > max
            }),
        );
        Ok(Self {
            db,
            limits,
            rows: 0,
            bytes: 0,
            steps,
            owner_rows: 0,
            owner_bytes: 0,
            owner_statements: 0,
            abort,
            abort_reason,
        })
    }
    pub fn steps(&self) -> u64 {
        self.steps.load(Ordering::Relaxed)
    }
    pub fn check_abort(&self) -> Result<()> {
        let reason = match self.abort_reason.load(Ordering::Relaxed) {
            1 => Some(crate::AbortReason::Cancelled),
            2 => Some(crate::AbortReason::DeadlineExceeded),
            _ => self.abort.as_deref().and_then(crate::AbortProbe::reason),
        };
        match reason {
            Some(crate::AbortReason::Cancelled) => Err(err(
                CompressedSearchErrorCode::Cancelled,
                "compressed search cancelled",
            )),
            Some(crate::AbortReason::DeadlineExceeded) => Err(err(
                CompressedSearchErrorCode::DeadlineExceeded,
                "compressed search deadline exceeded",
            )),
            None => Ok(()),
        }
    }
    pub fn abort_probe(&self) -> Option<&dyn crate::AbortProbe> {
        self.abort.as_deref()
    }
    pub fn reset_owner(&mut self) {
        self.owner_rows = 0;
        self.owner_bytes = 0;
        self.owner_statements = 0;
    }
    fn sql_error(&self, error: rusqlite::Error) -> CompressedSearchError {
        self.check_abort().err().unwrap_or_else(|| sql_error(error))
    }
    pub fn absorb_owner(&mut self, view: &PreparedReadTransaction<'_>) -> Result<()> {
        self.check_abort()?;
        let rows = view.read_rows();
        let bytes = view.read_bytes();
        let statements = view.statement_count();
        self.rows = self
            .rows
            .checked_add(
                rows.checked_sub(self.owner_rows)
                    .ok_or_else(|| unavailable("prepared admission row counters regressed"))?,
            )
            .ok_or_else(|| budget("prepared row counter overflow"))?;
        self.charge_bytes(
            bytes
                .checked_sub(self.owner_bytes)
                .ok_or_else(|| unavailable("prepared admission byte counters regressed"))?,
        )?;
        let statement_steps = statements
            .checked_sub(self.owner_statements)
            .and_then(|n| n.checked_mul(100))
            .ok_or_else(|| budget("prepared statement counter overflow"))?;
        let used = self
            .steps
            .fetch_add(statement_steps, Ordering::Relaxed)
            .saturating_add(statement_steps);
        self.owner_rows = rows;
        self.owner_bytes = bytes;
        self.owner_statements = statements;
        if used > self.limits.max_vm_steps {
            return Err(budget("prepared inspection exceeds its SQLite work budget"));
        }
        if self.rows > self.limits.max_rows {
            return Err(budget("prepared inspection exceeds its row budget"));
        }
        Ok(())
    }
    pub fn charge_bytes(&mut self, bytes: usize) -> Result<()> {
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .ok_or_else(|| budget("prepared byte counter overflow"))?;
        if self.bytes > self.limits.max_bytes {
            return Err(budget("prepared inspection exceeds its byte budget"));
        }
        Ok(())
    }
    pub fn query<T, F>(
        &mut self,
        sql: &str,
        args: &[&dyn ToSql],
        tracked: bool,
        mut f: F,
    ) -> Result<Vec<T>>
    where
        F: FnMut(&Row<'_>) -> rusqlite::Result<T>,
    {
        self.check_abort()?;
        if self
            .steps
            .fetch_add(100, Ordering::Relaxed)
            .saturating_add(100)
            > self.limits.max_vm_steps
        {
            return Err(budget("prepared inspection exceeds its SQLite work budget"));
        }
        let mut statement = self.db.prepare(sql).map_err(|e| self.sql_error(e))?;
        let mut rows = statement.query(args).map_err(|e| self.sql_error(e))?;
        let mut result = Vec::new();
        while let Some(row) = rows.next().map_err(|e| self.sql_error(e))? {
            // Every returned SQL row consumes the shared request allowance.
            // Search BLOB/chunk payload bytes are charged explicitly by their
            // owner below; `tracked` affects only automatic TEXT byte charging.
            self.rows = self
                .rows
                .checked_add(1)
                .ok_or_else(|| budget("prepared row counter overflow"))?;
            if self.rows > self.limits.max_rows {
                return Err(budget("prepared inspection exceeds its row budget"));
            }
            if tracked {
                let mut size = 0usize;
                for i in 0..row.as_ref().column_count() {
                    if let rusqlite::types::ValueRef::Text(v) = row.get_ref(i).map_err(sql_error)? {
                        size = size
                            .checked_add(v.len())
                            .ok_or_else(|| budget("prepared byte counter overflow"))?;
                    }
                }
                self.charge_bytes(size)?;
            }
            result.push(f(row).map_err(sql_error)?);
        }
        self.check_abort()?;
        Ok(result)
    }
    pub fn one<T, F>(
        &mut self,
        sql: &str,
        args: &[&dyn ToSql],
        tracked: bool,
        f: F,
    ) -> Result<Option<T>>
    where
        F: FnMut(&Row<'_>) -> rusqlite::Result<T>,
    {
        let mut rows = self.query(sql, args, tracked, f)?;
        if rows.len() > 1 {
            return Err(unavailable("selected prepared address is not unique"));
        }
        Ok(rows.pop())
    }
}
impl Drop for Read<'_> {
    fn drop(&mut self) {
        self.db.progress_handler(0, None::<fn() -> bool>);
    }
}
pub(crate) fn sql_error(error: rusqlite::Error) -> CompressedSearchError {
    match &error {
        rusqlite::Error::SqliteFailure(e, _)
            if matches!(
                e.code,
                rusqlite::ffi::ErrorCode::OperationInterrupted
                    | rusqlite::ffi::ErrorCode::DiskFull
                    | rusqlite::ffi::ErrorCode::TooBig
            ) =>
        {
            budget(error.to_string())
        }
        _ => unavailable(error.to_string()),
    }
}

#[derive(Default, Debug)]
pub(crate) struct Work {
    pub candidates: u64,
    pub operations: usize,
    pub verification_bytes: usize,
    pub blocks_decoded: u64,
    pub posting_entries_read: u64,
    pub metadata_bytes: usize,
    pub metadata_rows: u64,
    pub metadata_probes: u64,
    pub directory_probes: u64,
    pub response_bytes: usize,
}
impl Work {
    pub fn json(&self) -> JsonValue {
        object(vec![
            ("candidates", number(self.candidates)),
            ("operations", number(self.operations as u64)),
            ("verification_bytes", number(self.verification_bytes as u64)),
            ("blocks_decoded", number(self.blocks_decoded)),
            ("posting_entries_read", number(self.posting_entries_read)),
            ("metadata_bytes", number(self.metadata_bytes as u64)),
            ("metadata_rows", number(self.metadata_rows)),
            ("metadata_probes", number(self.metadata_probes)),
            ("directory_probes", number(self.directory_probes)),
            ("response_bytes", number(self.response_bytes as u64)),
        ])
    }
}
pub(crate) fn check_header(
    read: &mut Read<'_>,
    binding: &JsonValue,
    work: &mut Work,
) -> Result<(String, Vec<u8>)> {
    let sizes=read.one("SELECT CASE WHEN typeof(header)='text' THEN length(CAST(header AS BLOB)) ELSE -1 END, CASE WHEN typeof(cursor_key)='blob' THEN length(cursor_key) ELSE -1 END FROM search_header WHERE singleton=1 LIMIT 2",&[],false,|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?)))?.ok_or_else(||unavailable("missing search header"))?;
    if !(1..=65_536).contains(&sizes.0) || sizes.1 != 32 {
        return Err(unavailable("invalid or oversized stored search header"));
    }
    let (header, key) = read
        .one(
            "SELECT header,cursor_key FROM search_header WHERE singleton=1 LIMIT 2",
            &[],
            false,
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?)),
        )?
        .ok_or_else(|| unavailable("missing search header"))?;
    work.metadata_probes += 1;
    work.metadata_rows += 1;
    work.metadata_bytes += (sizes.0 + sizes.1) as usize;
    let expected = tos_compiler::local_prepared_search::header(binding)
        .map_err(|e| unavailable(e.to_string()))?;
    if header != expected {
        return Err(err(
            CompressedSearchErrorCode::StaleBinding,
            "search store snapshot/algorithm binding mismatch; restart query",
        ));
    }
    Ok((header, key))
}

#[derive(Clone, Debug)]
struct Partial {
    address: u64,
    stage: String,
    field: u64,
    offset: u64,
    rank: u8,
}
impl Partial {
    fn new(address: u64) -> Self {
        Self {
            address,
            stage: "identity".into(),
            field: 0,
            offset: 0,
            rank: 3,
        }
    }
    fn json(&self) -> JsonValue {
        object(vec![
            ("doc_id", number(self.address)),
            ("stage", string(&self.stage)),
            ("field", number(self.field)),
            ("offset", number(self.offset)),
            ("rank", number(self.rank as u64)),
        ])
    }
    fn decode(value: &JsonValue) -> Result<Self> {
        require_keys(value, &["doc_id", "stage", "field", "offset", "rank"])?;
        let stage = get(value, "stage")?
            .as_str()
            .filter(|s| ["identity", "visible", "full", "matched"].contains(s))
            .ok_or_else(|| cursor_error("invalid partial search stage"))?
            .to_owned();
        Ok(Self {
            address: integer(get(value, "doc_id")?, 1, MAX_ADDRESS)?,
            stage,
            field: integer(get(value, "field")?, 0, MAX_DOCUMENT_BYTES)?,
            offset: integer(get(value, "offset")?, 0, MAX_DOCUMENT_BYTES)?,
            rank: integer(get(value, "rank")?, 0, 3)? as u8,
        })
    }
}
struct InnerState {
    query: String,
    phase: u8,
    after: u64,
    partial: Option<Partial>,
    expires: u64,
}
impl InnerState {
    fn json(&self) -> JsonValue {
        object(vec![
            ("query", string(&self.query)),
            ("phase", number(self.phase as u64)),
            ("after", number(self.after)),
            (
                "partial",
                self.partial
                    .as_ref()
                    .map(|p| p.json())
                    .unwrap_or(JsonValue::Null),
            ),
            ("expires", number(self.expires)),
        ])
    }
}
pub(crate) struct Match {
    pub address: u64,
    pub id: JsonValue,
    pub rank: u8,
}
pub(crate) struct InnerPage {
    pub matches: Vec<Match>,
    pub has_more: bool,
    pub cursor: Option<JsonValue>,
    pub work: Work,
}

fn term(
    read: &mut Read<'_>,
    kind: &str,
    phase: u8,
    needle: &str,
    full_bound: bool,
) -> Result<Option<u64>> {
    let chars: Vec<char> = needle.chars().collect();
    let n = chars.len().min(3);
    let (width, keys): (usize, Vec<Vec<u8>>) = if needle.is_empty() {
        (0, vec![Vec::new()])
    } else if phase == 0 {
        (0, vec![needle.as_bytes().to_vec()])
    } else if phase == 1 {
        (n, vec![chars[..n].iter().collect::<String>().into_bytes()])
    } else {
        let keys: std::collections::BTreeSet<_> = chars
            .windows(n)
            .map(|g| g.iter().collect::<String>().into_bytes())
            .collect();
        (n, keys.into_iter().collect())
    };
    let mut best: Option<(u64, Vec<u8>, u64)> = None;
    for key in keys {
        let row=read.one("SELECT term_id,posting_count FROM search_terms WHERE kind=?1 AND plane=?2 AND n=?3 AND term_key=?4 LIMIT 2",&[&kind,&(phase as i64),&(width as i64),&key],false,|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?)))?;
        let Some((id, count)) = row else {
            return Ok(None);
        };
        if id < 1 || count < 0 {
            return Err(unavailable("invalid selected search term"));
        }
        if count == 0 {
            return Ok(None);
        }
        let candidate = (count as u64, key, id as u64);
        if best.as_ref().is_none_or(|b| candidate < *b) {
            best = Some(candidate);
        }
    }
    let best = best.ok_or_else(|| unavailable("missing query term seed"))?;
    if full_bound && !needle.is_empty() && phase < 3 {
        let Some(full) = term(read, kind, 3, needle, false)? else {
            return Ok(None);
        };
        let count = read
            .one(
                "SELECT posting_count FROM search_terms WHERE term_id=?1 LIMIT 2",
                &[&(full as i64)],
                false,
                |r| r.get::<_, i64>(0),
            )?
            .filter(|c| *c > 0)
            .ok_or_else(|| unavailable("invalid serialized search term"))?;
        if (count as u64) < best.0 {
            return Ok(Some(full));
        }
    }
    Ok(Some(best.2))
}

/// One block at a time. Fence bytes are never fetched or materialized; the
/// exact last consumed carrier key drives the next indexed directory seek.
struct Candidates {
    term: u64,
    after: u64,
    first: bool,
    finished: bool,
    last_key: Vec<u8>,
    block: Vec<u64>,
    index: usize,
}
enum Candidate {
    Address(u64),
    Exhausted,
    Budget,
}
impl Candidates {
    fn new(
        read: &mut Read<'_>,
        term: u64,
        after: u64,
        kind: &str,
        work: &mut Work,
    ) -> Result<Self> {
        let mut last_key = Vec::new();
        if after != 0 {
            let len=read.one("SELECT CASE WHEN typeof(sort_key)='blob' THEN length(sort_key) ELSE -1 END FROM search_documents WHERE doc_id=?1 AND kind=?2 LIMIT 2",&[&(after as i64),&kind],false,|r|r.get::<_,i64>(0))?.filter(|n|*n>=0&&*n<=MAX_ROW_BYTES as i64).ok_or_else(||unavailable("invalid cursor predecessor metadata"))?;
            work.metadata_probes += 1;
            last_key = read
                .one(
                    "SELECT sort_key FROM search_documents WHERE doc_id=?1 LIMIT 2",
                    &[&(after as i64)],
                    false,
                    |r| r.get::<_, Vec<u8>>(0),
                )?
                .ok_or_else(|| unavailable("missing cursor predecessor key"))?;
            work.metadata_bytes += len as usize;
            work.metadata_rows += 1;
        }
        Ok(Self {
            term,
            after,
            first: true,
            finished: false,
            last_key,
            block: Vec::new(),
            index: 0,
        })
    }
    fn next(
        &mut self,
        read: &mut Read<'_>,
        work: &mut Work,
        max_bytes: usize,
    ) -> Result<Candidate> {
        if self.index < self.block.len() {
            let address = self.block[self.index];
            self.index += 1;
            return Ok(Candidate::Address(address));
        }
        if self.finished {
            return Ok(Candidate::Exhausted);
        }
        let (suffix, order) = if self.first && self.after != 0 {
            ("lower_fence<=?2", "DESC")
        } else if self.first {
            ("1", "ASC")
        } else {
            ("lower_fence>?2", "ASC")
        };
        let base = format!(
            "FROM search_blocks INDEXED BY search_blocks_nonempty WHERE term_id=?1 AND {suffix} AND posting_count>0 ORDER BY lower_fence {order} LIMIT 1"
        );
        let term = self.term as i64;
        let args: Vec<&dyn ToSql> = if self.first && self.after == 0 {
            vec![&term]
        } else {
            vec![&term, &self.last_key]
        };
        let len = read.one(
            &format!(
                "SELECT CASE WHEN typeof(payload)='blob' THEN length(payload) ELSE -1 END {base}"
            ),
            &args,
            false,
            |r| r.get::<_, i64>(0),
        )?;
        work.directory_probes += 1;
        let Some(len) = len else {
            if self.first {
                return Err(unavailable(
                    "selected term or cursor predecessor has no posting block",
                ));
            }
            self.finished = true;
            return Ok(Candidate::Exhausted);
        };
        if !(1..=2048).contains(&len) {
            return Err(unavailable("invalid posting block size"));
        }
        if work.metadata_bytes.saturating_add(len as usize) > max_bytes {
            return Ok(Candidate::Budget);
        }
        let payload = read
            .one(&format!("SELECT payload {base}"), &args, false, |r| {
                r.get::<_, Vec<u8>>(0)
            })?
            .ok_or_else(|| unavailable("posting block disappeared"))?;
        work.metadata_bytes += payload.len();
        work.blocks_decoded += 1;
        self.block = tos_compiler::local_prepared_search::decode_search_postings(&payload)
            .map_err(|e| unavailable(e.to_string()))?;
        if self.block.is_empty() {
            return Err(unavailable(
                "nonempty posting directory contains an empty block",
            ));
        }
        work.posting_entries_read += self.block.len() as u64;
        self.index = 0;
        if self.first && self.after != 0 {
            self.index = self
                .block
                .iter()
                .position(|a| *a == self.after)
                .ok_or_else(|| {
                    unavailable("cursor predecessor is not a member of its posting block")
                })?
                + 1;
        }
        self.first = false;
        // A predecessor may be the last block address; seek the next block.
        self.next(read, work, max_bytes)
    }
}
fn read_text(
    read: &mut Read<'_>,
    address: u64,
    category: &str,
    field: u64,
    mut start: usize,
    mut length: usize,
) -> Result<Vec<u8>> {
    let mut bytes = Vec::with_capacity(length);
    while length > 0 {
        let size = length.min(CHUNK - start % CHUNK);
        let value=read.one("SELECT substr(payload,?1,?2) FROM search_text_chunks WHERE doc_id=?3 AND category=?4 AND field=?5 AND chunk=?6 LIMIT 2",&[&((start%CHUNK+1)as i64),&(size as i64),&(address as i64),&category,&(field as i64),&((start/CHUNK)as i64)],false,|r|r.get::<_,Vec<u8>>(0))?.ok_or_else(||unavailable("missing search text chunk"))?;
        if value.len() != size {
            return Err(unavailable("truncated search text chunk"));
        }
        bytes.extend_from_slice(&value);
        start += size;
        length -= size;
    }
    Ok(bytes)
}
fn verify(
    read: &mut Read<'_>,
    needle: &[u8],
    state: &mut Partial,
    work: &mut Work,
    limits: PublishedSearchLimits,
    phase: Option<u8>,
) -> Result<Option<bool>> {
    if state.stage == "matched" {
        return Ok(Some(true));
    }
    while work.operations < limits.candidate_budget {
        let category = state.stage.clone();
        if category == "full" && phase.is_some_and(|p| state.rank != p) {
            return Ok(Some(false));
        }
        let row=read.one("SELECT field,byte_length FROM search_values WHERE doc_id=?1 AND category=?2 AND field>=?3 ORDER BY field LIMIT 1",&[&(state.address as i64),&category,&(state.field as i64)],false,|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?)))?;
        work.operations += 1;
        let Some((field, length)) = row else {
            if category == "identity" {
                if phase.is_some_and(|p| (p <= 1 || state.rank <= 1) && state.rank != p) {
                    return Ok(Some(false));
                }
                state.stage = if state.rank > 1 { "visible" } else { "full" }.into();
                state.field = 0;
                state.offset = 0;
            } else if category == "visible" {
                state.stage = "full".into();
                state.field = 0;
                state.offset = 0;
            } else {
                return Ok(Some(false));
            }
            continue;
        };
        if field < 0
            || field > 8192
            || length < 0
            || length as u64 > MAX_DOCUMENT_BYTES
            || state.offset > length as u64
        {
            return Err(unavailable("invalid search value descriptor"));
        }
        let field = field as u64;
        let length = length as usize;
        let remaining = limits
            .verification_bytes
            .saturating_sub(work.verification_bytes);
        if category == "identity" {
            let size = length.min(needle.len());
            if size > remaining {
                return Ok(None);
            }
            let prefix = read_text(read, state.address, &category, field, 0, size)?;
            work.verification_bytes += size;
            if prefix == needle {
                state.rank = state.rank.min(if length == needle.len() { 0 } else { 1 });
            }
            state.field = field + 1;
            state.offset = 0;
            continue;
        }
        if length == 0 {
            state.field = field + 1;
            state.offset = 0;
            continue;
        }
        let offset = state.offset as usize;
        let overlap = offset.min(needle.len().saturating_sub(1));
        if remaining <= overlap {
            return Ok(None);
        }
        let size = CHUNK.min(length - offset).min(remaining - overlap);
        if size == 0 {
            return Err(unavailable("invalid partial verification offset"));
        }
        let bytes = read_text(
            read,
            state.address,
            &category,
            field,
            offset - overlap,
            size + overlap,
        )?;
        work.verification_bytes += bytes.len();
        if bytes.windows(needle.len()).any(|w| w == needle) {
            if category == "full" {
                return Ok(Some(true));
            }
            state.rank = 2;
            state.stage = "full".into();
            state.field = 0;
            state.offset = 0;
            continue;
        }
        let offset = offset + size;
        state.field = if offset == length { field + 1 } else { field };
        state.offset = if offset == length { 0 } else { offset as u64 };
    }
    Ok(None)
}

pub(crate) fn query_kind(
    read: &mut Read<'_>,
    binding: &JsonValue,
    kind: &str,
    needle: &str,
    filters: &JsonValue,
    limit: usize,
    cursor: Option<&JsonValue>,
    limits: PublishedSearchLimits,
    now: u64,
) -> Result<InnerPage> {
    let initial = cursor.is_none();
    let mut work = Work::default();
    let (header, key) = check_header(read, binding, &mut work)?;
    let frame = sorted_default(
        &JsonValue::Array(vec![string(kind), string(needle), filters.clone()]),
        65_536,
    )?;
    let mut hash = Digest256Hasher::new();
    hash.update(header.as_bytes());
    hash.update(b"\0full-text-bound-v1\0");
    hash.update(&frame);
    let query_hash = hash.finalize().to_hex();
    let mut hash = Digest256Hasher::new();
    hash.update(header.as_bytes());
    hash.update(b"\0");
    hash.update(&frame);
    let legacy_hash = hash.finalize().to_hex();
    let (mut state, full_bound) = if let Some(cursor) = cursor {
        require_keys(cursor, &["state", "mac"])?;
        let value = get(cursor, "state")?;
        let bytes = sorted_default(value, 4096)
            .map_err(|_| cursor_error("invalid inner search cursor length"))?;
        if !constant_time_equal(&hmac(&key, b"", &bytes), hex(get(cursor, "mac")?)?) {
            return Err(cursor_error("invalid search cursor integrity"));
        }
        require_keys(value, &["query", "phase", "after", "partial", "expires"])?;
        let query = hex(get(value, "query")?)?.to_owned();
        let full_bound = query != legacy_hash;
        if query != query_hash && full_bound {
            return Err(cursor_error("search cursor query/snapshot mismatch"));
        }
        let expires = integer(get(value, "expires")?, 1, MAX_ADDRESS)?;
        if expires <= now {
            return Err(err(
                CompressedSearchErrorCode::CursorExpired,
                "search cursor expired",
            ));
        }
        let partial = get(value, "partial")?;
        (
            InnerState {
                query,
                phase: integer(
                    get(value, "phase")?,
                    if needle.is_empty() { 3 } else { 0 },
                    3,
                )? as u8,
                after: integer(get(value, "after")?, 0, MAX_ADDRESS)?,
                partial: if partial.is_null() {
                    None
                } else {
                    Some(Partial::decode(partial)?)
                },
                expires,
            },
            full_bound,
        )
    } else {
        (
            InnerState {
                query: query_hash,
                phase: if needle.is_empty() { 3 } else { 0 },
                after: 0,
                partial: None,
                expires: now
                    .checked_add(900)
                    .filter(|v| *v <= MAX_ADDRESS)
                    .ok_or_else(|| invalid("invalid search clock"))?,
            },
            true,
        )
    };
    let mut matches = Vec::new();
    let mut match_bytes = 0usize;
    while state.phase < 4 && matches.len() < limit && work.operations < limits.candidate_budget {
        let Some(term) = term(read, kind, state.phase, needle, full_bound)? else {
            state.phase += 1;
            state.after = 0;
            state.partial = None;
            continue;
        };
        let mut candidates = Candidates::new(read, term, state.after, kind, &mut work)?;
        let mut exhausted = false;
        loop {
            match candidates.next(read, &mut work, limits.metadata_bytes)? {
                Candidate::Exhausted => {
                    exhausted = true;
                    break;
                }
                Candidate::Budget => break,
                Candidate::Address(address) => {
                    if work.operations >= limits.candidate_budget {
                        break;
                    }
                    let lengths=read.one("SELECT CASE WHEN typeof(identifier)='blob' THEN length(identifier) ELSE -1 END,CASE WHEN typeof(sort_key)='blob' THEN length(sort_key) ELSE -1 END,CASE WHEN typeof(filters)='blob' THEN length(filters) ELSE -1 END FROM search_documents WHERE doc_id=?1 AND kind=?2 LIMIT 2",&[&(address as i64),&kind],false,|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,i64>(2)?)))?.ok_or_else(||unavailable("posting references missing document metadata"))?;
                    work.metadata_probes += 1;
                    work.operations += 1;
                    work.candidates += 1;
                    if lengths.0 < 0
                        || lengths.1 < 0
                        || lengths.2 < 0
                        || lengths
                            .0
                            .saturating_add(lengths.1)
                            .saturating_add(lengths.2)
                            > MAX_ROW_BYTES as i64
                    {
                        return Err(unavailable(
                            "posting references oversized document metadata",
                        ));
                    }
                    let size = (lengths.0 + lengths.1 + lengths.2) as usize;
                    if work.metadata_bytes.saturating_add(size) > limits.metadata_bytes {
                        break;
                    }
                    let(identifier,key,raw_filters)=read.one("SELECT identifier,sort_key,filters FROM search_documents WHERE doc_id=?1 LIMIT 2",&[&(address as i64)],false,|r|Ok((r.get::<_,Vec<u8>>(0)?,r.get::<_,Vec<u8>>(1)?,r.get::<_,Vec<u8>>(2)?)))?.ok_or_else(||unavailable("missing search document metadata"))?;
                    work.metadata_bytes += size;
                    work.metadata_rows += 1;
                    candidates.last_key = key;
                    let doc_filters = parse_json(
                        &raw_filters,
                        JsonMode::PublishedStrict,
                        json_limits(MAX_ROW_BYTES),
                    )
                    .map_err(|_| unavailable("invalid stored document filters"))?
                    .into_root();
                    if doc_filters.as_object().is_none() {
                        return Err(unavailable("invalid stored document filters"));
                    }
                    let filtered = filters
                        .as_object()
                        .ok_or_else(|| invalid("invalid per-kind filters"))?
                        .iter()
                        .any(|(field, values)| {
                            let values = values.as_array().unwrap_or(&[]);
                            !values.is_empty()
                                && !values.iter().any(|v| {
                                    doc_filters
                                        .object_get(field.as_str().unwrap_or(""))
                                        .is_some_and(|a| a == v)
                                })
                        });
                    if filtered {
                        state.after = address;
                        state.partial = None;
                    } else {
                        let mut partial = state
                            .partial
                            .take()
                            .unwrap_or_else(|| Partial::new(address));
                        if partial.address != address {
                            return Err(unavailable("cursor document no longer matches posting"));
                        }
                        let found = if needle.is_empty() {
                            Some(true)
                        } else {
                            verify(
                                read,
                                needle.as_bytes(),
                                &mut partial,
                                &mut work,
                                limits,
                                full_bound.then_some(state.phase),
                            )?
                        };
                        let Some(found) = found else {
                            state.partial = Some(partial);
                            break;
                        };
                        if found && partial.rank == state.phase {
                            let id = parse_json(
                                &identifier,
                                JsonMode::PublishedStrict,
                                json_limits(MAX_ROW_BYTES),
                            )
                            .map_err(|_| unavailable("invalid stored search identity"))?
                            .into_root();
                            let size = sorted_default(
                                &object(vec![
                                    ("doc_id", number(address)),
                                    ("id", id.clone()),
                                    ("rank", number(state.phase as u64)),
                                ]),
                                MAX_ROW_BYTES + 8192,
                            )?
                            .len()
                                + 2;
                            if match_bytes.saturating_add(size).saturating_add(4096)
                                > 4 * 1024 * 1024
                            {
                                partial.stage = "matched".into();
                                partial.field = 0;
                                partial.offset = 0;
                                state.partial = Some(partial);
                                break;
                            }
                            matches.push(Match {
                                address,
                                id,
                                rank: state.phase,
                            });
                            match_bytes += size;
                        }
                        state.after = address;
                        state.partial = None;
                    }
                    if matches.len() >= limit || work.operations >= limits.candidate_budget {
                        break;
                    }
                }
            }
        }
        if exhausted {
            state.phase += 1;
            state.after = 0;
            state.partial = None;
        } else {
            break;
        }
    }
    let has_more = state.phase < 4;
    let cursor = if has_more {
        let value = state.json();
        Some(object(vec![
            ("state", value.clone()),
            (
                "mac",
                string(&hmac(&key, b"", &sorted_default(&value, 4096)?)),
            ),
        ]))
    } else {
        None
    };
    let matches_json = JsonValue::Array(
        matches
            .iter()
            .map(|m| {
                object(vec![
                    ("doc_id", number(m.address)),
                    ("id", m.id.clone()),
                    ("rank", number(m.rank as u64)),
                ])
            })
            .collect(),
    );
    for _ in 0..4 {
        let value = object(vec![
            ("schema", string(SCHEMA)),
            ("matches", matches_json.clone()),
            ("returned_count", number(matches.len() as u64)),
            (
                "total_matching",
                if initial && !has_more {
                    number(matches.len() as u64)
                } else {
                    JsonValue::Null
                },
            ),
            ("has_more", JsonValue::Bool(has_more)),
            ("next_cursor", cursor.clone().unwrap_or(JsonValue::Null)),
            ("work", work.json()),
        ]);
        let size = sorted_default(&value, 4 * 1024 * 1024)?.len();
        if work.response_bytes == size {
            break;
        }
        work.response_bytes = size;
    }
    Ok(InnerPage {
        matches,
        has_more,
        cursor,
        work,
    })
}
