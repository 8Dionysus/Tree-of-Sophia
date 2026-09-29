//! Explicit local prepared projection reader. An independently supplied binding
//! selects a snapshot; this adapter grants no source, rights or publication
//! authority and never falls back to a release/compatibility graph.
use crate::{
    AccessError, AccessErrorCode, AccessExecutor, AccessProfile, DisclosureFence,
    KnowledgeOperation, KnowledgeRequest, Params, PreparedPacket,
};
use rusqlite::{Connection, OpenFlags, limits::Limit};
use std::{
    fs,
    io::Read,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tos_compiler::local_prepared::PreparedReadLimits;
use tos_foundation::{
    JsonLimits, JsonMode, JsonNumber, JsonNumberKind, JsonString, JsonValue,
    emit_python_compact_json, parse_json,
};
use tos_query::{
    AbortProbe,
    compressed_search::{
        CompressedSearchError, CompressedSearchErrorCode, CompressedSearchRequest,
        PreparedSearchSession, PublishedSearchLimits,
    },
};

pub const PREPARED_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const REQUEST_BYTES: usize = 65_536;
enum LocalRequest {
    Search(CompressedSearchRequest),
    SearchCapabilities,
    Catalog,
    Inspect {
        kind: tos_query::search_v2::SearchKind,
        identifier: String,
        relation_limit: usize,
    },
    Lens(JsonValue),
}
fn error(code: AccessErrorCode, message: &'static str) -> AccessError {
    AccessError::new(code, message)
}
fn unavailable() -> AccessError {
    error(
        AccessErrorCode::Unavailable,
        "explicit local prepared publication unavailable",
    )
}
fn query_error(e: CompressedSearchError) -> AccessError {
    let code = match e.code {
        CompressedSearchErrorCode::InvalidRequest | CompressedSearchErrorCode::CursorInvalid => {
            AccessErrorCode::InvalidRequest
        }
        CompressedSearchErrorCode::StaleBinding => AccessErrorCode::StaleSelection,
        CompressedSearchErrorCode::CursorExpired => AccessErrorCode::CursorExpired,
        CompressedSearchErrorCode::BudgetExceeded => AccessErrorCode::BudgetExceeded,
        CompressedSearchErrorCode::Cancelled => AccessErrorCode::Cancelled,
        CompressedSearchErrorCode::DeadlineExceeded => AccessErrorCode::DeadlineExceeded,
        CompressedSearchErrorCode::Unavailable => AccessErrorCode::Unavailable,
    };
    error(code, "selected compressed search refused")
}
fn limits(bytes: usize) -> JsonLimits {
    JsonLimits {
        max_bytes: bytes,
        max_depth: 96,
        max_visits: 1_000_000,
        max_integer_digits: 4096,
    }
}
fn string(s: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(s))
}
fn number(n: u64) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: n.to_string(),
    })
}
fn object(fields: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        fields
            .into_iter()
            .map(|(k, v)| (JsonString::from_utf8(k), v))
            .collect(),
    )
}
fn compact(value: &JsonValue, cap: usize) -> Result<Vec<u8>, AccessError> {
    emit_python_compact_json(value, limits(cap)).map_err(|_| {
        error(
            AccessErrorCode::BudgetExceeded,
            "prepared packet framing byte budget exceeded",
        )
    })
}
pub fn profile() -> AccessProfile {
    let p = AccessProfile::new(REQUEST_BYTES, PREPARED_RESPONSE_BYTES, REQUEST_BYTES);
    let frame = crate::mcp::tool_result_frame_byte_bound(p.max_response_bytes, p.max_request_bytes)
        .expect("fixed prepared frame arithmetic");
    p.with_mcp_frame_budget(frame)
        .with_query_timeout(std::time::Duration::from_secs(5))
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FileState {
    dev: u64,
    ino: u64,
    size: u64,
    modified: (i64, i64),
    changed: (i64, i64),
}
fn state(path: &Path) -> Result<FileState, AccessError> {
    let m = fs::symlink_metadata(path).map_err(|_| unavailable())?;
    if !m.is_file() {
        return Err(unavailable());
    }
    Ok(FileState {
        dev: m.dev(),
        ino: m.ino(),
        size: m.len(),
        modified: (m.mtime(), m.mtime_nsec()),
        changed: (m.ctime(), m.ctime_nsec()),
    })
}
fn optional_state(path: &Path) -> Result<Option<FileState>, AccessError> {
    match fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(unavailable()),
        Ok(_) => state(path).map(Some),
    }
}
fn wal_path(path: &Path) -> PathBuf {
    let mut p = path.as_os_str().to_os_string();
    p.push("-wal");
    PathBuf::from(p)
}
struct CurrentFence {
    path: PathBuf,
    main: FileState,
    wal: Option<FileState>,
    probe: Arc<dyn AbortProbe>,
}
impl DisclosureFence for CurrentFence {
    fn recheck(&mut self) -> Result<(), AccessError> {
        crate::knowledge::check_abort(&self.probe)?;
        if state(&self.path)? != self.main || optional_state(&wal_path(&self.path))? != self.wal {
            return Err(error(
                AccessErrorCode::StaleSelection,
                "prepared publication changed before transport delivery",
            ));
        }
        // A pathname/WAL observation is weaker than a release-owner held lock.
        // This explicit local profile does not manufacture such a holder.
        crate::knowledge::check_abort(&self.probe)
    }
}
struct Selection {
    path: PathBuf,
    binding: JsonValue,
    read: PreparedReadLimits,
    reading: Option<crate::reading::ReadingLocalExecutor>,
}
pub struct PreparedLocalExecutor {
    selected: Arc<Selection>,
}
impl PreparedLocalExecutor {
    pub fn open(
        path: PathBuf,
        binding_path: PathBuf,
        root: Option<PathBuf>,
    ) -> Result<Self, AccessError> {
        if !path.is_absolute() || !binding_path.is_absolute() {
            return Err(error(
                AccessErrorCode::InvalidRequest,
                "prepared publication/binding require absolute paths",
            ));
        }
        let mut file = tos_fd_open::open_absolute_regular(&binding_path, REQUEST_BYTES as u64)
            .map_err(|_| unavailable())?;
        let mut raw = Vec::new();
        file.by_ref()
            .take(REQUEST_BYTES as u64 + 1)
            .read_to_end(&mut raw)
            .map_err(|_| unavailable())?;
        if raw.len() > REQUEST_BYTES {
            return Err(error(
                AccessErrorCode::BudgetExceeded,
                "prepared binding byte budget exceeded",
            ));
        }
        let binding = parse_json(&raw, JsonMode::PublishedStrict, limits(REQUEST_BYTES))
            .map_err(|_| {
                error(
                    AccessErrorCode::InvalidRequest,
                    "prepared binding JSON invalid",
                )
            })?
            .into_root();
        let reading = root
            .map(crate::reading::ReadingLocalExecutor::open)
            .transpose()?;
        let read = PreparedReadLimits {
            max_response_bytes: PREPARED_RESPONSE_BYTES,
            ..PreparedReadLimits::default()
        };
        // Binding validation and publication observation remain in the metered
        // request, rather than scanning the database at adapter construction.
        Ok(Self {
            selected: Arc::new(Selection {
                path,
                binding,
                read,
                reading,
            }),
        })
    }
    fn read(
        &self,
        request: LocalRequest,
        probe: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket<'static>, AccessError> {
        crate::knowledge::check_abort(&probe)?;
        let s = &self.selected;
        let before = state(&s.path)?;
        // FD opening checks the absolute regular path; SQLite retains the actual
        // pathname so its maintained WAL snapshot semantics are preserved.
        let pinned =
            tos_fd_open::open_absolute_regular(&s.path, u64::MAX).map_err(|_| unavailable())?;
        let db = Connection::open_with_flags(
            &s.path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|_| unavailable())?;
        db.busy_timeout(Duration::from_millis(100))
            .map_err(|_| unavailable())?;
        db.set_limit(
            Limit::SQLITE_LIMIT_LENGTH,
            (s.read.max_row_bytes.max(131_072) + 4096)
                .try_into()
                .map_err(|_| unavailable())?,
        )
        .map_err(|_| unavailable())?;
        db.pragma_update(None, "query_only", true)
            .map_err(|_| unavailable())?;
        if state(&s.path)? != before
            || pinned.metadata().map_err(|_| unavailable())?.ino() != before.ino
        {
            return Err(error(
                AccessErrorCode::StaleSelection,
                "prepared publication changed while opening",
            ));
        }
        db.execute_batch("BEGIN").map_err(|_| unavailable())?;
        let mut session =
            PreparedSearchSession::new_with_abort(&db, s.read, Some(Arc::clone(&probe)))
                .map_err(query_error)?;
        let result = if let LocalRequest::Search(request) = &request {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| unavailable())?
                .as_secs();
            session
                .search(
                    &s.binding,
                    request.clone(),
                    PublishedSearchLimits::default(),
                    now,
                )
                .map_err(query_error)?
        } else if let LocalRequest::Inspect {
            kind,
            identifier,
            relation_limit,
        } = &request
        {
            session
                .inspect(&s.binding, *kind, identifier, *relation_limit)
                .map_err(AccessError::from)?
        } else if let LocalRequest::Lens(spec) = &request {
            session.lens(&s.binding, spec).map_err(AccessError::from)?
        } else if matches!(request, LocalRequest::Catalog) {
            session.catalog(&s.binding).map_err(query_error)?
        } else {
            let compressed = session
                .capability(&s.binding, PublishedSearchLimits::default())
                .map_err(query_error)?;
            object(vec![
                ("schema", string("tos_knowledge_search_capabilities_v1")),
                ("default_mode", string("legacy")),
                ("explicit_mode_required", JsonValue::Bool(true)),
                ("writes_to_tree", JsonValue::Bool(false)),
                (
                    "modes",
                    object(vec![
                        (
                            "legacy",
                            object(vec![
                                ("available", JsonValue::Bool(false)),
                                ("schema", string("tos_knowledge_search_v1")),
                                ("verification", string("engine-selection-only")),
                                ("pagination", string("offset")),
                            ]),
                        ),
                        (
                            "indexed",
                            object(vec![
                                ("available", JsonValue::Bool(false)),
                                ("schema", string("tos_knowledge_search_indexed_v2")),
                                ("verification", string("engine-selection-only")),
                                ("pagination", string("cursor")),
                                ("min_normalized_query_code_points", number(3)),
                            ]),
                        ),
                        ("compressed", compressed),
                    ]),
                ),
            ])
        };
        crate::knowledge::check_abort(&probe)?;
        if let LocalRequest::Search(request) = &request {
            continuation_fits(request, &result)?;
        }
        let body = compact(&result, PREPARED_RESPONSE_BYTES)?;
        // End the operation snapshot; a new BEGIN sees a concurrent WAL commit
        // or epoch ABA. The same session meters both observations cumulatively.
        let observed_wal = optional_state(&wal_path(&s.path))?;
        db.execute_batch("COMMIT; BEGIN")
            .map_err(|_| unavailable())?;
        session.recheck_binding(&s.binding).map_err(query_error)?;
        db.execute_batch("COMMIT").map_err(|_| unavailable())?;
        drop(session);
        if state(&s.path)? != before {
            return Err(error(
                AccessErrorCode::StaleSelection,
                "prepared publication changed during query",
            ));
        }
        if optional_state(&wal_path(&s.path))? != observed_wal {
            return Err(error(
                AccessErrorCode::StaleSelection,
                "prepared WAL changed during current binding observation",
            ));
        }
        let fence = CurrentFence {
            path: s.path.clone(),
            main: before,
            wal: observed_wal,
            probe,
        };
        drop(db);
        drop(pinned);
        Ok(PreparedPacket {
            body,
            fence: Box::new(fence),
        })
    }
}
impl AccessExecutor for PreparedLocalExecutor {
    fn source_descend_available(&self) -> bool {
        false
    }
    fn source_descend(
        &self,
        _: Params,
        _: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket<'static>, AccessError> {
        Err(unavailable())
    }
    fn knowledge_search_compressed_available(&self) -> bool {
        true
    }
    fn knowledge_search_compressed(
        &self,
        request: CompressedSearchRequest,
        probe: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket<'static>, AccessError> {
        self.read(LocalRequest::Search(request), probe)
    }
    fn reading_search(
        &self,
        request: tos_query::reading_search::ReadingSearchRequest,
        probe: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket<'static>, AccessError> {
        match self.selected.reading.as_ref() {
            Some(reading) => reading.reading_search(request, probe),
            None => crate::reading::unavailable_packet(probe),
        }
    }
    fn knowledge_available(&self, operation: KnowledgeOperation) -> bool {
        matches!(
            operation,
            KnowledgeOperation::SearchCapabilities
                | KnowledgeOperation::Catalog
                | KnowledgeOperation::Node
                | KnowledgeOperation::Relation
                | KnowledgeOperation::Lens
        )
    }
    fn knowledge(
        &self,
        request: KnowledgeRequest,
        probe: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket<'static>, AccessError> {
        match request {
            KnowledgeRequest::SearchCapabilities => {
                self.read(LocalRequest::SearchCapabilities, probe)
            }
            KnowledgeRequest::Catalog => self.read(LocalRequest::Catalog, probe),
            KnowledgeRequest::Node {
                node_id,
                relation_limit,
            } => self.read(
                LocalRequest::Inspect {
                    kind: tos_query::search_v2::SearchKind::Nodes,
                    identifier: node_id,
                    relation_limit,
                },
                probe,
            ),
            KnowledgeRequest::Relation { relation_id } => self.read(
                LocalRequest::Inspect {
                    kind: tos_query::search_v2::SearchKind::Relations,
                    identifier: relation_id,
                    relation_limit: 0,
                },
                probe,
            ),
            KnowledgeRequest::Lens(spec) => self.read(LocalRequest::Lens(spec), probe),
            _ => Err(unavailable()),
        }
    }
}
fn percent_length(s: &str) -> usize {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
                1
            } else {
                3
            }
        })
        .sum()
}
fn continuation_fits(
    request: &CompressedSearchRequest,
    packet: &JsonValue,
) -> Result<(), AccessError> {
    let next = packet
        .object_get("page")
        .and_then(|v| v.object_get("next_cursor"));
    let cursor = match next {
        Some(JsonValue::Null) => return Ok(()),
        Some(v) => v.as_str().ok_or_else(unavailable)?,
        None => return Err(unavailable()),
    };
    let list = |values: &[String]| JsonValue::Array(values.iter().map(|v| string(v)).collect());
    let args = object(vec![
        ("mode", string("compressed")),
        ("query", string(&request.query)),
        ("sources", list(&request.sources)),
        ("kind_ids", list(&request.kind_ids)),
        ("predicate_ids", list(&request.predicate_ids)),
        ("limit", number(request.limit as u64)),
        ("cursor", string(cursor)),
    ]);
    let envelope = object(vec![
        ("jsonrpc", string("2.0")),
        ("id", number(1)),
        ("method", string("tools/call")),
        (
            "params",
            object(vec![
                ("name", string("tos_knowledge_search")),
                ("arguments", args),
            ]),
        ),
    ]);
    let mcp = compact(&envelope, REQUEST_BYTES)?
        .len()
        .checked_add(1)
        .ok_or_else(unavailable)?;
    let mut cli = "knowledgesearch--modecompressed--limit--cursor"
        .len()
        .checked_add(request.limit.to_string().len())
        .and_then(|n| n.checked_add(request.query.len()))
        .and_then(|n| n.checked_add(cursor.len()))
        .ok_or_else(unavailable)?;
    let mut http = "GET /api/knowledge/search?mode=compressed&query=&limit=&cursor= HTTP/1.1\r\nHost: localhost\r\n\r\n"
        .len().checked_add(percent_length(&request.query))
        .and_then(|n| n.checked_add(request.limit.to_string().len()))
        .and_then(|n| n.checked_add(percent_length(cursor))).ok_or_else(unavailable)?;
    for (key, flag, values) in [
        ("sources", "--sources", &request.sources),
        ("kind_ids", "--kind", &request.kind_ids),
        ("predicate_ids", "--predicate", &request.predicate_ids),
    ] {
        if values.is_empty() {
            continue;
        }
        // The maintained HTTP filter grammar uses one comma-separated value.
        // A literal comma inside a filter has no faithful HTTP representation.
        if values.iter().any(|v| v.contains(',')) {
            return Err(error(
                AccessErrorCode::BudgetExceeded,
                "compressed continuation filters cannot fit maintained HTTP grammar",
            ));
        }
        http = http
            .checked_add(2 + key.len())
            .and_then(|n| n.checked_add(percent_length(&values.join(","))))
            .ok_or_else(unavailable)?;
        if key == "sources" {
            cli = cli.checked_add(flag.len()).ok_or_else(unavailable)?;
        }
        for value in values {
            let flag_bytes = if key == "sources" { 0 } else { flag.len() };
            cli = cli
                .checked_add(flag_bytes)
                .and_then(|n| n.checked_add(value.len()))
                .ok_or_else(unavailable)?;
        }
    }
    if mcp > REQUEST_BYTES || cli > REQUEST_BYTES || http > REQUEST_BYTES {
        return Err(error(
            AccessErrorCode::BudgetExceeded,
            "compressed continuation cannot fit native request framing",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn continuation_framing_retains_query_and_filters_under_the_actual_request_cap() {
        let packet = |cursor: &str| {
            object(vec![(
                "page",
                object(vec![("next_cursor", string(cursor))]),
            )])
        };
        let request = CompressedSearchRequest {
            query: "query".into(),
            sources: vec!["philosophy".into()],
            kind_ids: vec!["concept".into()],
            limit: 1,
            ..Default::default()
        };
        assert!(continuation_fits(&request, &packet(&"x".repeat(36_000))).is_ok());
        assert_eq!(
            continuation_fits(&request, &packet(&"x".repeat(65_530)))
                .unwrap_err()
                .code,
            AccessErrorCode::BudgetExceeded
        );
        let oversized = CompressedSearchRequest {
            query: "z".repeat(35_000),
            ..request
        };
        assert_eq!(
            continuation_fits(&oversized, &packet(&"x".repeat(36_000)))
                .unwrap_err()
                .code,
            AccessErrorCode::BudgetExceeded
        );
    }
}
