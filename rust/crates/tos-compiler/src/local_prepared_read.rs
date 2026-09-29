//! Bounded admission to an independently selected local prepared snapshot.
//!
//! The caller opens the connection, begins and retains ONE transaction, and
//! installs its cumulative SQLite VM meter before admission. This module never
//! changes a pragma, opens a file, installs a progress callback, or performs DDL.
//! `check_current` observes that retained snapshot, not a later WAL generation.
//! File replacement and concurrent publication/ABA require the caller's separate
//! post-transaction current-selection observation; this handle grants no source,
//! rights, publication, semantic, or canon authority.

use rusqlite::{Connection, types::ValueRef};
use std::{cell::Cell, collections::HashSet, fmt};
use tos_foundation::{
    Digest256, FoundationError, FoundationErrorCode, JsonLimits, JsonMode, JsonValue,
    emit_python_compact_json, parse_json,
};

const LOCAL_SCHEMA: &str = "tos_local_prepared_read_model_v1";
const SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const CHUNK_BYTES: usize = 131_072;
const MAX_CHUNKS: usize = 256;
const TOP_BYTES: usize = 65_536;
const NORMALIZATION_KEYS: &[&str] = &[
    "schema",
    "processor_digest",
    "entity_registry_digest",
    "relation_registry_digest",
    "configuration_digest",
];
const BINDING_KEYS: &[&str] = &[
    "schema",
    "publication_epoch",
    "metadata_sha256",
    "read_model_schema",
    "source_revision",
    "data_revision",
    "graph_schema",
    "normalization_binding",
];
const TOP_KEYS: &[&str] = &[
    "schema",
    "read_model_schema",
    "source_revision",
    "data_revision",
    "graph_schema",
    "normalization_binding",
    "catalog_sha256",
    "row_integrity",
    "authority_boundary",
    "lens_sha256",
];

/// Finite read budgets. QRY owns cumulative SQL VM, response and payload work;
/// this admission handle separately meters every schema/metadata row and byte.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedReadLimits {
    pub max_row_bytes: usize,
    pub max_response_bytes: usize,
    pub max_rows: usize,
    pub max_vm_steps: u64,
    pub max_bytes: usize,
}

impl Default for PreparedReadLimits {
    fn default() -> Self {
        Self {
            max_row_bytes: 1_048_576,
            max_response_bytes: 16_777_216,
            max_rows: 4096,
            max_vm_steps: 200_000,
            max_bytes: 16_777_216,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparedReadErrorCode {
    StaleBinding,
    BudgetExceeded,
    Unavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedReadError {
    pub code: PreparedReadErrorCode,
    pub detail: &'static str,
}

impl fmt::Display for PreparedReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.code, self.detail)
    }
}
impl std::error::Error for PreparedReadError {}
pub type Result<T> = std::result::Result<T, PreparedReadError>;

fn unavailable(detail: &'static str) -> PreparedReadError {
    PreparedReadError {
        code: PreparedReadErrorCode::Unavailable,
        detail,
    }
}
fn budget(detail: &'static str) -> PreparedReadError {
    PreparedReadError {
        code: PreparedReadErrorCode::BudgetExceeded,
        detail,
    }
}
fn stale(detail: &'static str) -> PreparedReadError {
    PreparedReadError {
        code: PreparedReadErrorCode::StaleBinding,
        detail,
    }
}
impl From<rusqlite::Error> for PreparedReadError {
    fn from(error: rusqlite::Error) -> Self {
        if matches!(&error, rusqlite::Error::SqliteFailure(code, _)
            if code.code == rusqlite::ffi::ErrorCode::OperationInterrupted
                || code.code == rusqlite::ffi::ErrorCode::TooBig)
        {
            budget("prepared SQLite read budget")
        } else {
            unavailable("configured prepared read model is unavailable or invalid")
        }
    }
}
impl From<FoundationError> for PreparedReadError {
    fn from(error: FoundationError) -> Self {
        if error.code == FoundationErrorCode::BudgetExceeded {
            budget("prepared JSON structural or byte budget")
        } else {
            unavailable("prepared metadata contains invalid published JSON")
        }
    }
}

fn codec_limits(max_bytes: usize) -> JsonLimits {
    JsonLimits {
        max_bytes,
        max_depth: 64,
        max_visits: 300_000,
        max_integer_digits: 4300,
    }
}
fn exact_keys(value: &JsonValue, keys: &[&str]) -> bool {
    let Some(entries) = value.as_object() else {
        return false;
    };
    let mut seen = HashSet::new();
    entries.len() == keys.len()
        && entries.iter().all(|(key, _)| {
            key.as_str()
                .is_some_and(|key| keys.contains(&key) && seen.insert(key))
        })
}
fn text<'a>(value: &'a JsonValue, field: &str) -> Option<&'a str> {
    value.object_get(field)?.as_str()
}
fn hash(value: &JsonValue, field: &str) -> bool {
    text(value, field).is_some_and(|raw| {
        raw.len() == 64
            && raw
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}
fn normalization(value: &JsonValue) -> bool {
    exact_keys(value, NORMALIZATION_KEYS)
        && text(value, "schema") == Some("tos_knowledge_graph_normalization_binding_v1")
        && NORMALIZATION_KEYS[1..].iter().all(|key| hash(value, key))
}
fn validate_binding(value: &JsonValue) -> Result<()> {
    // Emit before cloning: a caller-built value has the same finite structural
    // and Unicode/duplicate-member obligations as parsed published JSON.
    emit_python_compact_json(value, codec_limits(TOP_BYTES))?;
    if !exact_keys(value, BINDING_KEYS)
        || text(value, "schema") != Some("tos_published_knowledge_snapshot_v1")
        || text(value, "read_model_schema") != Some(LOCAL_SCHEMA)
        || text(value, "graph_schema") != Some("tos_knowledge_graph_v1")
        || value
            .object_get("publication_epoch")
            .and_then(JsonValue::as_u64)
            .is_none_or(|epoch| epoch > SAFE_INTEGER)
        || ["metadata_sha256", "source_revision", "data_revision"]
            .iter()
            .any(|key| !hash(value, key))
        || value
            .object_get("normalization_binding")
            .is_none_or(|value| !normalization(value))
    {
        return Err(unavailable(
            "exact independently owner-selected local prepared binding required",
        ));
    }
    Ok(())
}
fn validate_top(value: &JsonValue) -> Result<()> {
    if !exact_keys(value, TOP_KEYS)
        || text(value, "schema") != Some("tos_published_knowledge_reader_v2")
        || text(value, "read_model_schema") != Some(LOCAL_SCHEMA)
        || text(value, "graph_schema") != Some("tos_knowledge_graph_v1")
        || text(value, "row_integrity") != Some("sha256-emitted-json-v1")
        || [
            "source_revision",
            "data_revision",
            "catalog_sha256",
            "lens_sha256",
        ]
        .iter()
        .any(|key| !hash(value, key))
        || value
            .object_get("normalization_binding")
            .is_none_or(|value| !normalization(value))
    {
        return Err(unavailable(
            "invalid local prepared knowledge reader metadata",
        ));
    }
    let boundary = value
        .object_get("authority_boundary")
        .ok_or_else(|| unavailable("missing prepared authority boundary"))?;
    if boundary.as_object().is_none()
        || text(boundary, "source_owner") != Some("Tree-of-Sophia")
        || ["is_source", "is_canon", "writes_to_tree"]
            .iter()
            .any(|key| boundary.object_get(key).and_then(JsonValue::as_bool) != Some(false))
    {
        return Err(unavailable(
            "prepared reader does not preserve source authority boundary",
        ));
    }
    Ok(())
}

/// A checked view into the caller's retained local prepared read transaction.
/// The expected binding is always supplied independently, never inferred as an
/// admission decision from the database. Accessors cannot refresh or select it.
pub struct PreparedReadTransaction<'a> {
    db: &'a Connection,
    binding: JsonValue,
    top: JsonValue,
    limits: PreparedReadLimits,
    rows: Cell<usize>,
    bytes: Cell<usize>,
}

impl<'a> PreparedReadTransaction<'a> {
    pub fn admit(
        db: &'a Connection,
        expected: &JsonValue,
        limits: PreparedReadLimits,
    ) -> Result<Self> {
        if limits.max_row_bytes == 0
            || limits.max_response_bytes == 0
            || limits.max_rows == 0
            || limits.max_vm_steps == 0
            || limits.max_bytes == 0
        {
            return Err(budget(
                "prepared reader limits must all be finite positive integers",
            ));
        }
        if db.is_autocommit() {
            return Err(unavailable(
                "prepared admission requires caller's retained transaction",
            ));
        }
        validate_binding(expected)?;
        let mut view = Self {
            db,
            binding: expected.clone(),
            top: JsonValue::Null,
            limits,
            rows: Cell::new(0),
            bytes: Cell::new(0),
        };
        view.check_schema()?;
        view.top = view.snapshot()?;
        Ok(view)
    }

    pub fn connection(&self) -> &'a Connection {
        self.db
    }
    pub fn binding(&self) -> &JsonValue {
        &self.binding
    }
    pub fn top(&self) -> &JsonValue {
        &self.top
    }
    pub fn limits(&self) -> &PreparedReadLimits {
        &self.limits
    }

    /// Recheck binding/epoch in this transaction, with cumulative admission
    /// budgets. The caller must not COMMIT/ROLLBACK/re-BEGIN while holding it.
    /// This is deliberately not a claim about a newer external WAL snapshot.
    pub fn check_current(&self) -> Result<()> {
        if self.db.is_autocommit() {
            return Err(stale("prepared caller transaction is no longer retained"));
        }
        if self.snapshot()? != self.top {
            return Err(stale(
                "prepared reader header changed within retained transaction",
            ));
        }
        Ok(())
    }

    fn account(&self, bytes: usize) -> Result<()> {
        let rows = self
            .rows
            .get()
            .checked_add(1)
            .ok_or_else(|| budget("prepared row budget"))?;
        let total = self
            .bytes
            .get()
            .checked_add(bytes)
            .ok_or_else(|| budget("prepared byte budget"))?;
        if rows > self.limits.max_rows || total > self.limits.max_bytes {
            return Err(budget("prepared schema/metadata cumulative read budget"));
        }
        self.rows.set(rows);
        self.bytes.set(total);
        Ok(())
    }

    fn schema_text<'r>(&self, row: &'r rusqlite::Row<'_>, column: usize) -> Result<&'r str> {
        let ValueRef::Text(bytes) = row.get_ref(column)? else {
            return Err(unavailable("prepared schema column is not text"));
        };
        if bytes.len() > CHUNK_BYTES.min(self.limits.max_row_bytes) {
            return Err(budget("prepared schema row byte budget"));
        }
        std::str::from_utf8(bytes)
            .map_err(|_| unavailable("prepared schema contains invalid UTF-8"))
    }

    fn metadata(&self, key: &str, maximum: usize) -> Result<(Vec<u8>, JsonValue)> {
        let mut statement = self
            .db
            .prepare("SELECT part,json_chunk FROM edge_meta WHERE key=? ORDER BY part LIMIT 257")?;
        let mut rows = statement.query([key])?;
        let mut raw = Vec::new();
        let mut count = 0;
        while let Some(row) = rows.next()? {
            if count == MAX_CHUNKS {
                return Err(budget("prepared metadata chunk budget"));
            }
            if row.get::<_, i64>(0)? != count as i64 {
                return Err(unavailable(
                    "prepared metadata chunks are incomplete or unordered",
                ));
            }
            let ValueRef::Text(chunk) = row.get_ref(1)? else {
                return Err(unavailable("prepared metadata chunk is not text"));
            };
            if chunk.len() > CHUNK_BYTES
                || raw
                    .len()
                    .checked_add(chunk.len())
                    .is_none_or(|n| n > maximum)
            {
                return Err(budget("prepared metadata byte budget"));
            }
            self.account(chunk.len())?;
            raw.extend_from_slice(chunk);
            count += 1;
        }
        if count == 0 {
            return Err(unavailable("prepared metadata is missing"));
        }
        let value = parse_json(&raw, JsonMode::PublishedStrict, codec_limits(maximum))?.into_root();
        if emit_python_compact_json(&value, codec_limits(maximum))? != raw {
            return Err(unavailable(
                "prepared metadata differs from exact emitted compact JSON framing",
            ));
        }
        Ok((raw, value))
    }

    fn snapshot(&self) -> Result<JsonValue> {
        let (raw, top) =
            self.metadata("knowledge_reader_top", TOP_BYTES.min(self.limits.max_bytes))?;
        validate_top(&top)?;
        let (_, revision) = self.metadata("data_revision", 1024.min(self.limits.max_bytes))?;
        if !exact_keys(&revision, &["sha256"])
            || text(&revision, "sha256") != text(&top, "data_revision")
        {
            return Err(unavailable("prepared reader has incoherent data revision"));
        }
        let mut statement = self
            .db
            .prepare("SELECT epoch FROM knowledge_exploration_clock WHERE singleton=1 LIMIT 2")?;
        let mut clocks = statement.query([])?;
        let row = clocks
            .next()?
            .ok_or_else(|| unavailable("prepared publication clock missing"))?;
        self.account(0)?;
        let epoch: i64 = row.get(0)?;
        if epoch < 0 || epoch as u64 > SAFE_INTEGER || clocks.next()?.is_some() {
            return Err(unavailable("prepared publication clock invalid"));
        }
        if self
            .binding
            .object_get("publication_epoch")
            .and_then(JsonValue::as_u64)
            != Some(epoch as u64)
            || text(&self.binding, "metadata_sha256")
                != Some(Digest256::of_bytes(&raw).to_hex().as_str())
            || [
                "read_model_schema",
                "source_revision",
                "data_revision",
                "graph_schema",
            ]
            .iter()
            .any(|key| text(&self.binding, key) != text(&top, key))
            || NORMALIZATION_KEYS.iter().any(|key| {
                text(
                    self.binding.object_get("normalization_binding").unwrap(),
                    key,
                ) != text(top.object_get("normalization_binding").unwrap(), key)
            })
        {
            return Err(stale(
                "prepared knowledge snapshot differs from independent owner selection",
            ));
        }
        Ok(top)
    }

    fn check_schema(&self) -> Result<()> {
        // Exact base column order/type/nullability/key ordinals. Capability
        // owners separately check search, semantic and other auxiliary stores.
        type Column = (&'static str, &'static str, i64, i64);
        const TABLES: &[(&str, &[Column])] = &[
            (
                "edge_meta",
                &[
                    ("key", "TEXT", 1, 1),
                    ("part", "INTEGER", 1, 2),
                    ("json_chunk", "TEXT", 1, 0),
                ],
            ),
            (
                "knowledge_exploration_clock",
                &[("singleton", "INTEGER", 0, 1), ("epoch", "INTEGER", 1, 0)],
            ),
            (
                "knowledge_nodes",
                &[
                    ("id", "TEXT", 0, 1),
                    ("entity_id", "TEXT", 1, 0),
                    ("native_id", "TEXT", 1, 0),
                    ("source_graph", "TEXT", 1, 0),
                    ("kind_id", "TEXT", 1, 0),
                    ("type_id", "TEXT", 1, 0),
                    ("json", "TEXT", 1, 0),
                ],
            ),
            (
                "knowledge_relations",
                &[
                    ("id", "TEXT", 0, 1),
                    ("native_id", "TEXT", 1, 0),
                    ("source_graph", "TEXT", 1, 0),
                    ("from_id", "TEXT", 1, 0),
                    ("to_id", "TEXT", 1, 0),
                    ("predicate_id", "TEXT", 1, 0),
                    ("relation_type_id", "TEXT", 1, 0),
                    ("json", "TEXT", 1, 0),
                ],
            ),
            (
                "knowledge_lens_order",
                &[
                    ("kind", "TEXT", 1, 1),
                    ("id", "TEXT", 1, 2),
                    ("sort_key", "TEXT", 1, 0),
                    ("from_id", "TEXT", 1, 0),
                    ("to_id", "TEXT", 1, 0),
                ],
            ),
            (
                "prepared_documents",
                &[
                    ("kind", "TEXT", 1, 1),
                    ("id", "TEXT", 1, 2),
                    ("doc_id", "INTEGER", 1, 0),
                    ("source_order", "INTEGER", 1, 0),
                ],
            ),
            (
                "prepared_state",
                &[
                    ("singleton", "INTEGER", 0, 1),
                    ("high_water", "INTEGER", 1, 0),
                    ("max_pages", "INTEGER", 1, 0),
                    ("descriptor", "TEXT", 1, 0),
                ],
            ),
        ];
        for (table, expected) in TABLES {
            let mut statement = self.db.prepare(
                "SELECT name,type,\"notnull\",pk,hidden FROM pragma_table_xinfo(?) ORDER BY cid LIMIT 17")?;
            let mut rows = statement.query([table])?;
            let mut count = 0;
            while let Some(row) = rows.next()? {
                let name = self.schema_text(row, 0)?;
                let ty = self.schema_text(row, 1)?;
                self.account(name.len() + ty.len())?;
                if expected.get(count).is_none_or(|column| {
                    name != column.0
                        || ty != column.1
                        || row.get::<_, i64>(2).ok() != Some(column.2)
                        || row.get::<_, i64>(3).ok() != Some(column.3)
                }) || row.get::<_, i64>(4)? != 0
                {
                    return Err(unavailable("local prepared base table layout differs"));
                }
                count += 1;
            }
            if count != expected.len() {
                return Err(unavailable("local prepared base table unavailable"));
            }
        }
        const INDEXES: &[(&str, &str, &[&str])] = &[
            (
                "knowledge_nodes_native_idx",
                "knowledge_nodes",
                &["native_id"],
            ),
            (
                "knowledge_nodes_entity_idx",
                "knowledge_nodes",
                &["entity_id"],
            ),
            (
                "knowledge_nodes_identity_seek",
                "knowledge_nodes",
                &["entity_id", "id"],
            ),
            (
                "knowledge_relations_native_idx",
                "knowledge_relations",
                &["native_id"],
            ),
            (
                "knowledge_relations_from_seek",
                "knowledge_relations",
                &["from_id", "id"],
            ),
            (
                "knowledge_relations_to_seek",
                "knowledge_relations",
                &["to_id", "id"],
            ),
            (
                "knowledge_lens_order_sort",
                "knowledge_lens_order",
                &["kind", "sort_key", "id"],
            ),
            (
                "knowledge_lens_order_from",
                "knowledge_lens_order",
                &["kind", "from_id", "sort_key", "id"],
            ),
            (
                "knowledge_lens_order_to",
                "knowledge_lens_order",
                &["kind", "to_id", "sort_key", "id"],
            ),
            (
                "knowledge_lens_order_pair",
                "knowledge_lens_order",
                &["kind", "from_id", "to_id", "id"],
            ),
        ];
        for (index, table, expected) in INDEXES {
            let mut statement = self.db.prepare(
                "SELECT \"unique\",partial FROM pragma_index_list(?) WHERE name=? LIMIT 2",
            )?;
            let mut rows = statement.query([table, index])?;
            let row = rows
                .next()?
                .ok_or_else(|| unavailable("local prepared required index missing"))?;
            self.account(0)?;
            if row.get::<_, i64>(0)? != 0 || row.get::<_, i64>(1)? != 0 || rows.next()?.is_some() {
                return Err(unavailable("local prepared required index options differ"));
            }
            self.index_layout(index, expected)?;
        }
        // Point reads use these primary-key indexes as well as the named
        // aliases above. A NOCASE primary key must not silently change identity.
        const PRIMARY_KEYS: &[(&str, &[&str])] = &[
            ("edge_meta", &["key", "part"]),
            ("knowledge_nodes", &["id"]),
            ("knowledge_relations", &["id"]),
            ("knowledge_lens_order", &["kind", "id"]),
            ("prepared_documents", &["kind", "id"]),
        ];
        for (table, expected) in PRIMARY_KEYS {
            let mut statement = self.db.prepare(
                "SELECT name,\"unique\",partial FROM pragma_index_list(?) WHERE origin='pk' LIMIT 2")?;
            let mut rows = statement.query([table])?;
            let row = rows
                .next()?
                .ok_or_else(|| unavailable("local prepared primary index missing"))?;
            let name = self.schema_text(row, 0)?;
            self.account(name.len())?;
            if row.get::<_, i64>(1)? != 1 || row.get::<_, i64>(2)? != 0 {
                return Err(unavailable("local prepared primary index options differ"));
            }
            self.index_layout(name, expected)?;
            if rows.next()?.is_some() {
                return Err(unavailable("local prepared primary index ambiguous"));
            }
        }
        Ok(())
    }

    fn index_layout(&self, index: &str, expected: &[&str]) -> Result<()> {
        let mut statement = self.db.prepare(
            "SELECT name,coll,\"desc\" FROM pragma_index_xinfo(?) WHERE key=1 ORDER BY seqno LIMIT 17")?;
        let mut rows = statement.query([index])?;
        let mut count = 0;
        while let Some(row) = rows.next()? {
            let name = self.schema_text(row, 0)?;
            let coll = self.schema_text(row, 1)?;
            self.account(name.len() + coll.len())?;
            if expected.get(count).is_none_or(|column| name != *column)
                || coll != "BINARY"
                || row.get::<_, i64>(2)? != 0
            {
                return Err(unavailable("local prepared required index layout differs"));
            }
            count += 1;
        }
        if count != expected.len() {
            return Err(unavailable("local prepared required index incomplete"));
        }
        Ok(())
    }
}
