//! Import the private typed snapshot frame used by returning in-process SQLite
//! callers. The frame is a custody transport, not a SQLite file or authority
//! receipt. Only fixed ToS table profiles are materialized; other ordinary
//! tables are parsed and fingerprinted as inert opaque records.

#[path = "typed_snapshot_encoder.rs"]
mod encoder;
pub(crate) use encoder::{
    EncodeBudget, ReadStatement, ReadView, encode as encode_borrowed, encode_view, reserve_schema,
};

use rusqlite::{
    Connection, TransactionBehavior, params_from_iter,
    types::{Value as SqlValue, ValueRef},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::Read,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};
use tos_foundation::{Digest256, Digest256Hasher};

const MAGIC: &[u8; 8] = b"TOSLSNP1";
const HEADER_BYTES: usize = 26;
const FOOTER_BYTES: usize = 32;
const MAX_COLUMNS: usize = 4096;
const MAX_INDEXES: usize = 4096;
const MAX_SCHEMA_OBJECTS: usize = 65_536;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Role {
    D1 = 1,
    Prepared = 2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DatabaseEncoding {
    Utf8 = 1,
    Utf16Le = 2,
    Utf16Be = 3,
}

impl DatabaseEncoding {
    fn parse(tag: u8) -> Result<Self, String> {
        match tag {
            1 => Ok(Self::Utf8),
            2 => Ok(Self::Utf16Le),
            3 => Ok(Self::Utf16Be),
            _ => Err("typed snapshot database encoding tag".into()),
        }
    }
    fn pragma(self) -> &'static str {
        match self {
            Self::Utf8 => "UTF-8",
            Self::Utf16Le => "UTF-16le",
            Self::Utf16Be => "UTF-16be",
        }
    }
}

#[derive(Clone, Copy)]
struct ColumnSpec {
    name: &'static str,
    ty: &'static str,
    not_null: bool,
    primary_key: u16,
    unique: bool,
}

macro_rules! col {
    ($name:literal, $ty:literal, $not_null:literal, $pk:literal) => {
        ColumnSpec {
            name: $name,
            ty: $ty,
            not_null: $not_null,
            primary_key: $pk,
            unique: false,
        }
    };
    ($name:literal, $ty:literal, $not_null:literal, $pk:literal, unique) => {
        ColumnSpec {
            name: $name,
            ty: $ty,
            not_null: $not_null,
            primary_key: $pk,
            unique: true,
        }
    };
}

struct TableSpec {
    id: u16,
    name: &'static str,
    columns: &'static [ColumnSpec],
    check: &'static str,
}

const D1_TABLES: &[TableSpec] = &[
    TableSpec {
        id: 0x0001,
        name: "knowledge_nodes",
        check: "",
        columns: &[
            col!("id", "TEXT", false, 1),
            col!("entity_id", "TEXT", true, 0),
            col!("native_id", "TEXT", true, 0),
            col!("source_graph", "TEXT", true, 0),
            col!("kind_id", "TEXT", true, 0),
            col!("type_id", "TEXT", true, 0),
            col!("title_text", "TEXT", true, 0),
            col!("summary_text", "TEXT", true, 0),
            col!("search_text", "TEXT", true, 0),
            col!("json", "TEXT", true, 0),
        ],
    },
    TableSpec {
        id: 0x0002,
        name: "knowledge_relations",
        check: "",
        columns: &[
            col!("id", "TEXT", false, 1),
            col!("native_id", "TEXT", true, 0),
            col!("source_graph", "TEXT", true, 0),
            col!("from_id", "TEXT", true, 0),
            col!("to_id", "TEXT", true, 0),
            col!("predicate_id", "TEXT", true, 0),
            col!("relation_type_id", "TEXT", true, 0),
            col!("label_text", "TEXT", true, 0),
            col!("explanation_text", "TEXT", true, 0),
            col!("search_text", "TEXT", true, 0),
            col!("json", "TEXT", true, 0),
        ],
    },
    TableSpec {
        id: 0x0003,
        name: "knowledge_search_documents",
        check: "",
        columns: &[
            col!("kind", "TEXT", true, 1),
            col!("position", "INTEGER", true, 2),
            col!("id", "TEXT", true, 0),
            col!("source_graph", "TEXT", true, 0),
            col!("kind_id", "TEXT", true, 0),
            col!("predicate_id", "TEXT", true, 0),
            col!("id_lower", "TEXT", true, 0),
            col!("native_id_lower", "TEXT", true, 0),
            col!("identity_values", "TEXT", true, 0),
            col!("visible_values", "TEXT", true, 0),
            col!("document_chars", "INTEGER", true, 0),
            col!("document_digest", "TEXT", true, 0),
        ],
    },
    TableSpec {
        id: 0x0004,
        name: "knowledge_search_grams",
        check: "",
        columns: &[
            col!("kind", "TEXT", true, 1),
            col!("n", "INTEGER", true, 2),
            col!("gram", "TEXT", true, 3),
            col!("position", "INTEGER", true, 4),
        ],
    },
    TableSpec {
        id: 0x0005,
        name: "knowledge_search_gram_stats",
        check: "",
        columns: &[
            col!("kind", "TEXT", true, 1),
            col!("n", "INTEGER", true, 2),
            col!("gram", "TEXT", true, 3),
            col!("postings", "INTEGER", true, 0),
        ],
    },
    TableSpec {
        id: 0x0006,
        name: "knowledge_lens_order",
        check: "",
        columns: &[
            col!("kind", "TEXT", true, 1),
            col!("id", "TEXT", true, 2),
            col!("sort_key", "TEXT", true, 0),
            col!("from_id", "TEXT", true, 0),
            col!("to_id", "TEXT", true, 0),
        ],
    },
    TableSpec {
        id: 0x0007,
        name: "source_navigation_nodes",
        check: "",
        columns: &[
            col!("node_id", "TEXT", false, 1),
            col!("ord", "INTEGER", true, 0),
            col!("node_kind", "TEXT", true, 0),
            col!("source_ref", "TEXT", true, 0),
            col!("label", "TEXT", true, 0),
            col!("identity_status", "TEXT", true, 0),
            col!("properties_json", "TEXT", true, 0),
            col!("json", "TEXT", true, 0),
        ],
    },
    TableSpec {
        id: 0x0008,
        name: "source_navigation_node_payload",
        check: "",
        columns: &[
            col!("id", "TEXT", true, 1),
            col!("part", "INTEGER", true, 2),
            col!("json_chunk", "TEXT", true, 0),
        ],
    },
    TableSpec {
        id: 0x0009,
        name: "source_navigation_edges",
        check: "",
        columns: &[
            col!("edge_id", "TEXT", false, 1),
            col!("ord", "INTEGER", true, 0),
            col!("from_id", "TEXT", true, 0),
            col!("to_id", "TEXT", true, 0),
            col!("edge_kind", "TEXT", true, 0),
            col!("predicate_id", "TEXT", true, 0),
            col!("review_status", "TEXT", true, 0),
            col!("source_refs_json", "TEXT", true, 0),
            col!("json", "TEXT", true, 0),
        ],
    },
    TableSpec {
        id: 0x000a,
        name: "source_navigation_edge_payload",
        check: "",
        columns: &[
            col!("id", "TEXT", true, 1),
            col!("part", "INTEGER", true, 2),
            col!("json_chunk", "TEXT", true, 0),
        ],
    },
    TableSpec {
        id: 0x000b,
        name: "source_navigation_rights",
        check: "",
        columns: &[
            col!("rights_id", "TEXT", false, 1),
            col!("ord", "INTEGER", true, 0),
            col!("scope_refs_json", "TEXT", true, 0),
            col!("json", "TEXT", true, 0),
        ],
    },
    TableSpec {
        id: 0x000c,
        name: "source_navigation_rights_payload",
        check: "",
        columns: &[
            col!("id", "TEXT", true, 1),
            col!("part", "INTEGER", true, 2),
            col!("json_chunk", "TEXT", true, 0),
        ],
    },
    TableSpec {
        id: 0x000d,
        name: "edge_meta",
        check: "",
        columns: &[
            col!("key", "TEXT", true, 1),
            col!("part", "INTEGER", true, 2),
            col!("json_chunk", "TEXT", true, 0),
        ],
    },
    TableSpec {
        id: 0x000e,
        name: "knowledge_exploration_clock",
        check: "CHECK(singleton=1)",
        columns: &[
            col!("singleton", "INTEGER", false, 1),
            col!("epoch", "INTEGER", true, 0),
        ],
    },
    TableSpec {
        id: 0x0010,
        name: "knowledge_compact_lens",
        check: "",
        columns: &[
            col!("kind", "TEXT", true, 1),
            col!("id", "TEXT", true, 2),
            col!("source_sha256", "TEXT", true, 0),
            col!("seed_sha256", "TEXT", true, 0),
            col!("json", "TEXT", true, 0),
        ],
    },
    TableSpec {
        id: 0x0011,
        name: "knowledge_compact_lens_state",
        check: "CHECK(singleton=1 AND valid IN (0,1))",
        columns: &[
            col!("singleton", "INTEGER", false, 1),
            col!("schema", "TEXT", true, 0),
            col!("binding", "TEXT", true, 0),
            col!("valid", "INTEGER", true, 0),
        ],
    },
    TableSpec {
        id: 0x0012,
        name: "knowledge_lens_memberships",
        check: "",
        columns: &[
            col!("kind", "TEXT", true, 1),
            col!("field", "TEXT", true, 2),
            col!("value", "TEXT", true, 3),
            col!("id", "TEXT", true, 4),
            col!("sort_key", "TEXT", true, 0),
        ],
    },
    TableSpec {
        id: 0x0013,
        name: "knowledge_lens_membership_state",
        check: "CHECK(singleton=1 AND valid IN (0,1))",
        columns: &[
            col!("singleton", "INTEGER", false, 1),
            col!("schema", "TEXT", true, 0),
            col!("binding", "TEXT", true, 0),
            col!("valid", "INTEGER", true, 0),
        ],
    },
];

const PREPARED_TABLES: &[TableSpec] = &[
    TableSpec {
        id: 0x0020,
        name: "edge_meta",
        check: "",
        columns: &[
            col!("key", "TEXT", true, 1),
            col!("part", "INTEGER", true, 2),
            col!("json_chunk", "TEXT", true, 0),
        ],
    },
    TableSpec {
        id: 0x0021,
        name: "knowledge_exploration_clock",
        check: "CHECK(singleton=1)",
        columns: &[
            col!("singleton", "INTEGER", false, 1),
            col!("epoch", "INTEGER", true, 0),
        ],
    },
    TableSpec {
        id: 0x0022,
        name: "knowledge_lens_order",
        check: "",
        columns: &[
            col!("kind", "TEXT", true, 1),
            col!("id", "TEXT", true, 2),
            col!("sort_key", "TEXT", true, 0),
            col!("from_id", "TEXT", true, 0),
            col!("to_id", "TEXT", true, 0),
        ],
    },
    TableSpec {
        id: 0x0023,
        name: "prepared_documents",
        check: "",
        columns: &[
            col!("kind", "TEXT", true, 1),
            col!("id", "TEXT", true, 2),
            col!("doc_id", "INTEGER", true, 0, unique),
            col!("source_order", "INTEGER", true, 0),
        ],
    },
    TableSpec {
        id: 0x0024,
        name: "prepared_state",
        check: "CHECK(singleton=1)",
        columns: &[
            col!("singleton", "INTEGER", false, 1),
            col!("high_water", "INTEGER", true, 0),
            col!("max_pages", "INTEGER", true, 0),
            col!("descriptor", "TEXT", true, 0),
        ],
    },
    TableSpec {
        id: 0x0025,
        name: "knowledge_nodes",
        check: "",
        columns: &[
            col!("id", "TEXT", false, 1),
            col!("entity_id", "TEXT", true, 0),
            col!("native_id", "TEXT", true, 0),
            col!("source_graph", "TEXT", true, 0),
            col!("kind_id", "TEXT", true, 0),
            col!("type_id", "TEXT", true, 0),
            col!("json", "TEXT", true, 0),
        ],
    },
    TableSpec {
        id: 0x0026,
        name: "knowledge_relations",
        check: "",
        columns: &[
            col!("id", "TEXT", false, 1),
            col!("native_id", "TEXT", true, 0),
            col!("source_graph", "TEXT", true, 0),
            col!("from_id", "TEXT", true, 0),
            col!("to_id", "TEXT", true, 0),
            col!("predicate_id", "TEXT", true, 0),
            col!("relation_type_id", "TEXT", true, 0),
            col!("json", "TEXT", true, 0),
        ],
    },
    TableSpec {
        id: 0x0027,
        name: "prepared_source_state",
        check: "CHECK(singleton=1)",
        columns: &[
            col!("singleton", "INTEGER", false, 1),
            col!("binding", "TEXT", true, 0),
            col!("inputs", "TEXT", true, 0),
            col!("sha256", "TEXT", true, 0),
        ],
    },
];

#[derive(Clone)]
struct ColumnEvidence {
    name: String,
    declared_type: String,
    not_null: bool,
    primary_key: u16,
    hidden: u8,
    default: Option<Vec<u8>>,
}

#[derive(Clone)]
struct IndexKey {
    cid: i32,
    name: Option<String>,
    descending: bool,
    collation: String,
    key: bool,
}

#[derive(Clone)]
struct IndexEvidence {
    name: String,
    unique: bool,
    origin: u8,
    partial: bool,
    keys: Vec<IndexKey>,
}

#[derive(Clone)]
struct TableEvidence {
    id: u16,
    name: String,
    present: bool,
    flags: u8,
    columns: Vec<ColumnEvidence>,
    indexes: Vec<IndexEvidence>,
    row_count: u64,
    wire_range: std::ops::Range<usize>,
}

struct SchemaObject {
    kind: u8,
    name: String,
    table_name: String,
    sql: Option<String>,
}

pub(crate) struct ImportedFrame {
    pub(crate) connection: Connection,
    pub(crate) guard: File,
    pub(crate) selected_path: PathBuf,
    pub(crate) selected_identity: (u64, u64),
    pub(crate) inventory: Value,
    pub(crate) frame_sha256: String,
    pub(crate) frame_bytes: u64,
}

struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
    allocations: &'a mut SchemaAllocationBudget,
}

struct SchemaAllocationBudget {
    limit: u64,
    used: u64,
}

impl SchemaAllocationBudget {
    fn new(limit: u64) -> Self {
        Self { limit, used: 0 }
    }

    fn charge(&mut self, bytes: usize) -> Result<(), String> {
        let bytes = u64::try_from(bytes).map_err(|_| "typed snapshot schema allocation")?;
        self.used = self
            .used
            .checked_add(bytes)
            .filter(|used| *used <= self.limit)
            .ok_or("typed snapshot schema allocation budget")?;
        Ok(())
    }

    fn charge_vec<T>(&mut self, count: usize) -> Result<(), String> {
        count
            .checked_mul(std::mem::size_of::<T>())
            .ok_or_else(|| "typed snapshot schema allocation overflow".to_owned())
            .and_then(|bytes| self.charge(bytes))
    }
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8], allocations: &'a mut SchemaAllocationBudget) -> Self {
        Self {
            bytes,
            position: 0,
            allocations,
        }
    }
    fn charge(&mut self, bytes: usize) -> Result<(), String> {
        self.allocations.charge(bytes)
    }
    fn charge_vec<T>(&mut self, count: usize) -> Result<(), String> {
        self.allocations.charge_vec::<T>(count)
    }
    fn take(&mut self, count: usize) -> Result<&'a [u8], String> {
        let end = self
            .position
            .checked_add(count)
            .ok_or("typed snapshot offset overflow")?;
        let value = self
            .bytes
            .get(self.position..end)
            .ok_or("truncated typed snapshot frame")?;
        self.position = end;
        Ok(value)
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn i32(&mut self) -> Result<i32, String> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn i64(&mut self) -> Result<i64, String> {
        Ok(i64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn text(&mut self, count: usize) -> Result<String, String> {
        let bytes = self.take(count)?;
        self.charge(count.saturating_add(std::mem::size_of::<String>()))?;
        std::str::from_utf8(bytes)
            .map(str::to_owned)
            .map_err(|_| "typed snapshot schema text is not UTF-8".into())
    }
    fn sized_text16(&mut self) -> Result<String, String> {
        let n = self.u16()? as usize;
        self.text(n)
    }
    fn optional_text16(&mut self) -> Result<Option<String>, String> {
        let n = self.u16()?;
        if n == u16::MAX {
            Ok(None)
        } else {
            self.text(n as usize).map(Some)
        }
    }
    fn optional_bytes32(&mut self) -> Result<Option<Vec<u8>>, String> {
        let n = self.u32()?;
        if n == u32::MAX {
            Ok(None)
        } else {
            let count = n as usize;
            self.charge(count.saturating_add(std::mem::size_of::<Vec<u8>>()))?;
            self.take(count).map(|bytes| Some(bytes.to_vec()))
        }
    }
}

fn tables(role: Role) -> &'static [TableSpec] {
    match role {
        Role::D1 => D1_TABLES,
        Role::Prepared => PREPARED_TABLES,
    }
}

fn sqlite_record_length_limit(frame_bytes: u64, role: Role) -> Result<i32, String> {
    // SQLITE_LIMIT_LENGTH covers both individual values and SQLite's encoded
    // table/index records. The typed frame already bounds every source cell
    // payload in a record; add enough header space for the widest owner table
    // plus an implicit rowid in a secondary index. Per-cell semantic limits
    // remain enforced by sql_value/skip_cell before any row is imported.
    const SQLITE_VARINT_MAX_BYTES: u64 = 9;
    let indexed_columns = tables(role)
        .iter()
        .map(|table| table.columns.len())
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| "typed snapshot SQLite record columns".to_owned())?;
    let header_bytes = u64::try_from(indexed_columns)
        .ok()
        .and_then(|columns| columns.checked_mul(SQLITE_VARINT_MAX_BYTES))
        .and_then(|bytes| bytes.checked_add(SQLITE_VARINT_MAX_BYTES))
        .ok_or_else(|| "typed snapshot SQLite record header".to_owned())?;
    let required = frame_bytes
        .checked_add(header_bytes)
        .ok_or_else(|| "typed snapshot SQLite record length".to_owned())?;
    // sqlite3_limit takes a signed 32-bit request. SQLite also clamps this to
    // the build's hard length limit; source values still have their stricter
    // per-cell limits and the whole frame remains independently bounded.
    i32::try_from(required.min(i32::MAX as u64))
        .map_err(|_| "typed snapshot SQLite record length".into())
}

fn role_label(role: Role) -> &'static str {
    match role {
        Role::D1 => "d1",
        Role::Prepared => "prepared",
    }
}

fn quote(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

fn create_table(db: &Connection, spec: &TableSpec) -> Result<(), String> {
    let mut definitions = Vec::with_capacity(spec.columns.len() + 2);
    for column in spec.columns {
        let mut definition = format!("{} {}", quote(column.name), column.ty);
        if column.not_null {
            definition.push_str(" NOT NULL");
        }
        if column.unique {
            definition.push_str(" UNIQUE");
        }
        definitions.push(definition);
    }
    let mut primary = spec
        .columns
        .iter()
        .filter(|column| column.primary_key > 0)
        .collect::<Vec<_>>();
    primary.sort_by_key(|column| column.primary_key);
    if !primary.is_empty() {
        definitions.push(format!(
            "PRIMARY KEY ({})",
            primary
                .iter()
                .map(|column| quote(column.name))
                .collect::<Vec<_>>()
                .join(",")
        ));
    }
    if !spec.check.is_empty() {
        definitions.push(spec.check.to_owned());
    }
    let sql = format!(
        "CREATE TABLE {} ({})",
        quote(spec.name),
        definitions.join(",")
    );
    db.execute_batch(&sql)
        .map_err(|error| format!("typed snapshot owner table schema: {error}"))
}

fn expected_ids(role: Role) -> &'static [u16] {
    match role {
        Role::D1 => &[
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 18, 19,
        ],
        Role::Prepared => &[32, 33, 34, 35, 36, 37, 38, 39],
    }
}

fn parse_index(cursor: &mut Cursor<'_>) -> Result<IndexEvidence, String> {
    let name = cursor.sized_text16()?;
    let unique = match cursor.u8()? {
        0 => false,
        1 => true,
        _ => return Err("typed snapshot index unique flag".into()),
    };
    let origin = cursor.u8()?;
    if origin > 2 {
        return Err("typed snapshot index origin".into());
    }
    let partial = match cursor.u8()? {
        0 => false,
        1 => true,
        _ => return Err("typed snapshot partial flag".into()),
    };
    let count = cursor.u16()? as usize;
    if count > MAX_COLUMNS || count > cursor.bytes.len().saturating_sub(cursor.position) / 9 {
        return Err("typed snapshot index column count".into());
    }
    cursor.charge_vec::<IndexKey>(count)?;
    let mut keys = Vec::with_capacity(count);
    for _ in 0..count {
        let cid = cursor.i32()?;
        let name = cursor.optional_text16()?;
        let descending = match cursor.u8()? {
            0 => false,
            1 => true,
            _ => return Err("typed snapshot index direction".into()),
        };
        let collation = cursor.sized_text16()?;
        let key = match cursor.u8()? {
            0 => false,
            1 => true,
            _ => return Err("typed snapshot index key flag".into()),
        };
        keys.push(IndexKey {
            cid,
            name,
            descending,
            collation,
            key,
        });
    }
    Ok(IndexEvidence {
        name,
        unique,
        origin,
        partial,
        keys,
    })
}

fn validate_columns(spec: &TableSpec, columns: &[ColumnEvidence]) -> Result<(), String> {
    if columns.len() != spec.columns.len() {
        return Err(format!("typed snapshot {} column count", spec.name));
    }
    for (actual, expected) in columns.iter().zip(spec.columns) {
        if actual.name != expected.name
            || !actual.declared_type.eq_ignore_ascii_case(expected.ty)
            || actual.not_null != expected.not_null
            || actual.primary_key != expected.primary_key
            || actual.hidden != 0
            || actual.default.is_some()
        {
            return Err(format!("typed snapshot {} column profile", spec.name));
        }
    }
    Ok(())
}

fn index_keys_match_primary(index: &IndexEvidence, spec: &TableSpec, expected: usize) -> bool {
    let mut key_index = 0usize;
    for key in index.keys.iter().filter(|key| key.key) {
        let ordinal = match u16::try_from(key_index + 1) {
            Ok(ordinal) => ordinal,
            Err(_) => return false,
        };
        let Some(column) = spec
            .columns
            .iter()
            .find(|column| column.primary_key == ordinal)
        else {
            return false;
        };
        let Some(column_index) = spec
            .columns
            .iter()
            .position(|item| item.name == column.name)
        else {
            return false;
        };
        if key.cid != column_index as i32
            || key.name.as_deref() != Some(column.name)
            || key.descending
            || key.collation != "BINARY"
        {
            return false;
        }
        key_index += 1;
    }
    key_index == expected
}

fn index_keys_match_names(index: &IndexEvidence, spec: &TableSpec, expected: &[&str]) -> bool {
    let mut key_index = 0usize;
    for key in index.keys.iter().filter(|key| key.key) {
        let Some(name) = expected.get(key_index) else {
            return false;
        };
        let Some(column_index) = spec.columns.iter().position(|column| column.name == *name) else {
            return false;
        };
        if key.cid != column_index as i32
            || key.name.as_deref() != Some(*name)
            || key.descending
            || key.collation != "BINARY"
        {
            return false;
        }
        key_index += 1;
    }
    key_index == expected.len()
}

fn validate_owner_indexes(spec: &TableSpec, indexes: &[IndexEvidence]) -> Result<(), String> {
    let primary_key_count = spec
        .columns
        .iter()
        .filter(|column| column.primary_key > 0)
        .count();
    let rowid_alias = primary_key_count == 1
        && spec
            .columns
            .iter()
            .any(|column| column.primary_key == 1 && column.ty == "INTEGER");
    let expected_unique_count = spec.columns.iter().filter(|column| column.unique).count();
    let mut primary_key_indexes = 0usize;
    let mut unique_indexes = 0usize;

    for index in indexes {
        if index.name.is_empty() || index.keys.is_empty() || index.keys.len() > MAX_COLUMNS {
            return Err("typed snapshot index evidence".into());
        }
        match index.origin {
            0 => {}
            1 => {
                unique_indexes += 1;
                if !index.unique || index.partial {
                    return Err(format!("typed snapshot {} unique-index flags", spec.name));
                }
                let mut matching_unique = 0usize;
                for column in spec.columns.iter().filter(|column| column.unique) {
                    matching_unique +=
                        usize::from(index_keys_match_names(index, spec, &[column.name]));
                }
                if matching_unique != 1 {
                    return Err(format!("typed snapshot {} unique index profile", spec.name));
                }
            }
            2 => {
                primary_key_indexes += 1;
                if rowid_alias
                    || !index.unique
                    || index.partial
                    || !index_keys_match_primary(index, spec, primary_key_count)
                {
                    return Err(format!(
                        "typed snapshot {} primary-key index profile",
                        spec.name
                    ));
                }
            }
            _ => return Err("typed snapshot index origin".into()),
        }
    }

    if primary_key_count == 0
        || (rowid_alias && primary_key_indexes != 0)
        || (!rowid_alias && primary_key_indexes != 1)
        || unique_indexes != expected_unique_count
    {
        return Err(format!(
            "typed snapshot {} owner index inventory",
            spec.name
        ));
    }
    Ok(())
}

fn required_search_index(
    spec: &TableSpec,
    indexes: &[IndexEvidence],
    name: &str,
    columns: &[&str],
    unique: bool,
) -> bool {
    indexes.iter().any(|index| {
        index.name == name
            && index.unique == unique
            && index.origin == 0
            && !index.partial
            && index_keys_match_names(index, spec, columns)
    })
}

fn install_search_indexes(
    db: &Connection,
    spec: &TableSpec,
    indexes: &[IndexEvidence],
) -> Result<(), String> {
    if spec.name != "knowledge_search_documents" {
        return Ok(());
    }
    if required_search_index(
        spec,
        indexes,
        "knowledge_search_address_id_idx",
        &["kind", "id"],
        true,
    ) {
        db.execute_batch("CREATE UNIQUE INDEX knowledge_search_address_id_idx ON knowledge_search_documents(kind,id)")
            .map_err(|error| format!("typed snapshot search-address index: {error}"))?;
    }
    if required_search_index(
        spec,
        indexes,
        "knowledge_search_address_tie_idx",
        &["kind", "id_lower", "position"],
        false,
    ) {
        db.execute_batch("CREATE INDEX knowledge_search_address_tie_idx ON knowledge_search_documents(kind,id_lower,position)")
            .map_err(|error| format!("typed snapshot search-tie index: {error}"))?;
    }
    Ok(())
}

fn storage_name(tag: u8) -> Result<&'static str, String> {
    match tag {
        0 => Ok("null"),
        1 => Ok("integer"),
        2 => Ok("real"),
        3 => Ok("text"),
        4 => Ok("blob"),
        _ => Err("typed snapshot storage tag".into()),
    }
}

fn utf16_unit(bytes: &[u8], offset: usize, encoding: DatabaseEncoding) -> u16 {
    let pair = [bytes[offset], bytes[offset + 1]];
    match encoding {
        DatabaseEncoding::Utf16Le => u16::from_le_bytes(pair),
        DatabaseEncoding::Utf16Be => u16::from_be_bytes(pair),
        DatabaseEncoding::Utf8 => unreachable!("UTF-16 decoder used for UTF-8"),
    }
}

fn utf16_text_len(bytes: &[u8], encoding: DatabaseEncoding) -> Result<usize, String> {
    if bytes.len() % 2 != 0 {
        return Err("typed snapshot odd-length UTF-16 text".into());
    }
    let mut offset = 0usize;
    let mut output_bytes = 0usize;
    while offset < bytes.len() {
        let first = utf16_unit(bytes, offset, encoding);
        let (scalar, consumed) = if (0xd800..=0xdbff).contains(&first) {
            let next_offset = offset
                .checked_add(2)
                .ok_or("typed snapshot UTF-16 offset overflow")?;
            if next_offset >= bytes.len() {
                return Err("typed snapshot unpaired UTF-16 high surrogate".into());
            }
            let second = utf16_unit(bytes, next_offset, encoding);
            if !(0xdc00..=0xdfff).contains(&second) {
                return Err("typed snapshot unpaired UTF-16 high surrogate".into());
            }
            let high = u32::from(first - 0xd800);
            let low = u32::from(second - 0xdc00);
            (0x1_0000 + (high << 10) + low, 4)
        } else if (0xdc00..=0xdfff).contains(&first) {
            return Err("typed snapshot unpaired UTF-16 low surrogate".into());
        } else {
            (u32::from(first), 2)
        };
        let character = char::from_u32(scalar)
            .ok_or_else(|| "typed snapshot invalid UTF-16 scalar".to_owned())?;
        output_bytes = output_bytes
            .checked_add(character.len_utf8())
            .ok_or("typed snapshot decoded text length overflow")?;
        offset = offset
            .checked_add(consumed)
            .ok_or("typed snapshot UTF-16 offset overflow")?;
    }
    Ok(output_bytes)
}

fn decode_utf16_text(
    allocations: &mut SchemaAllocationBudget,
    bytes: &[u8],
    encoding: DatabaseEncoding,
) -> Result<String, String> {
    if !matches!(
        encoding,
        DatabaseEncoding::Utf16Le | DatabaseEncoding::Utf16Be
    ) {
        return Err("typed snapshot UTF-16 decoder encoding".into());
    }
    if bytes.len() % 2 != 0 {
        return Err("typed snapshot odd-length UTF-16 text".into());
    }
    let units = bytes.len() / 2;
    // Charge both bounded scans and the maximum UTF-8 buffer before decoding
    // or allocating. A BMP unit needs at most three UTF-8 bytes; a surrogate
    // pair needs four bytes for two units.
    let decode_work = bytes
        .len()
        .checked_mul(2)
        .ok_or("typed snapshot UTF-16 work overflow")?;
    let max_output = units
        .checked_mul(3)
        .ok_or("typed snapshot UTF-16 output overflow")?;
    let charged = decode_work
        .checked_add(max_output)
        .and_then(|value| value.checked_add(std::mem::size_of::<String>()))
        .ok_or("typed snapshot UTF-16 allocation overflow")?;
    allocations.charge(charged)?;

    let output_len = utf16_text_len(bytes, encoding)?;
    let mut output = String::new();
    output
        .try_reserve_exact(output_len)
        .map_err(|_| "typed snapshot UTF-16 text allocation".to_owned())?;
    let mut offset = 0usize;
    while offset < bytes.len() {
        let first = utf16_unit(bytes, offset, encoding);
        let (scalar, consumed) = if (0xd800..=0xdbff).contains(&first) {
            let second = utf16_unit(bytes, offset + 2, encoding);
            let high = u32::from(first - 0xd800);
            let low = u32::from(second - 0xdc00);
            (0x1_0000 + (high << 10) + low, 4)
        } else {
            (u32::from(first), 2)
        };
        output.push(
            char::from_u32(scalar)
                .ok_or_else(|| "typed snapshot invalid UTF-16 scalar".to_owned())?,
        );
        offset += consumed;
    }
    Ok(output)
}

fn sql_value(
    cursor: &mut Cursor<'_>,
    max_cell_bytes: usize,
    encoding: DatabaseEncoding,
) -> Result<(u8, SqlValue), String> {
    let tag = cursor.u8()?;
    let value = match tag {
        0 => SqlValue::Null,
        1 => SqlValue::Integer(cursor.i64()?),
        2 => SqlValue::Real(f64::from_bits(cursor.u64()?)),
        3 | 4 => {
            let length =
                usize::try_from(cursor.u64()?).map_err(|_| "typed snapshot cell length")?;
            if length > max_cell_bytes {
                return Err("typed snapshot owner cell byte limit".into());
            }
            if tag == 3 && encoding != DatabaseEncoding::Utf8 {
                // Charge the retained source carrier before copying it. The
                // decoder separately charges its scans and UTF-8 output.
                cursor.charge(length)?;
            }
            let bytes = cursor.take(length)?.to_vec();
            // Keep TEXT in its source database encoding until the owner table
            // profile is known. UTF-8 can use SQLite's same-encoding cast;
            // UTF-16 must be decoded before binding, since SQLite casts a BLOB
            // as UTF-8 and would replace malformed UTF-8 bytes.
            SqlValue::Blob(bytes)
        }
        _ => return Err("typed snapshot storage tag".into()),
    };
    Ok((tag, value))
}

fn skip_cell(cursor: &mut Cursor<'_>, max_cell_bytes: usize) -> Result<(), String> {
    let tag = cursor.u8()?;
    match tag {
        0 => Ok(()),
        1 | 2 => {
            cursor.take(8)?;
            Ok(())
        }
        3 | 4 => {
            let length =
                usize::try_from(cursor.u64()?).map_err(|_| "typed snapshot cell length")?;
            if length > max_cell_bytes {
                return Err("typed snapshot owner cell byte limit".into());
            }
            cursor.take(length)?;
            Ok(())
        }
        _ => Err("typed snapshot storage tag".into()),
    }
}

fn expected_tag(column: &ColumnSpec, tag: u8) -> bool {
    tag == 0 || (column.ty == "TEXT" && tag == 3) || (column.ty == "INTEGER" && tag == 1)
}

fn compare_returned(
    row: &rusqlite::Row<'_>,
    offset: usize,
    expected_tag: u8,
    value: &SqlValue,
) -> Result<(), String> {
    let storage: String = row.get(offset).map_err(|error| error.to_string())?;
    if storage != storage_name(expected_tag)? {
        return Err("typed snapshot imported storage class changed".into());
    }
    let actual = row.get_ref(offset + 1).map_err(|error| error.to_string())?;
    let equal = match (expected_tag, actual, value) {
        (0, ValueRef::Null, SqlValue::Null) => true,
        (1, ValueRef::Integer(actual), SqlValue::Integer(expected)) => actual == *expected,
        (2, ValueRef::Real(actual), SqlValue::Real(expected)) => {
            actual.to_bits() == expected.to_bits()
        }
        (3, ValueRef::Blob(actual), SqlValue::Blob(expected)) => actual == expected.as_slice(),
        (4, ValueRef::Blob(actual), SqlValue::Blob(expected)) => actual == expected.as_slice(),
        _ => false,
    };
    if !equal {
        return Err("typed snapshot imported cell differs from transported value".into());
    }
    Ok(())
}

fn import_row(
    db: &Connection,
    encoding: DatabaseEncoding,
    allocations: &mut SchemaAllocationBudget,
    spec: &TableSpec,
    rowid: Option<i64>,
    cells: &[(u8, SqlValue)],
    columns: &[ColumnEvidence],
) -> Result<(), String> {
    if cells.len() != spec.columns.len() {
        return Err("typed snapshot row shape".into());
    }
    let primary_key_count = spec
        .columns
        .iter()
        .filter(|column| column.primary_key > 0)
        .count();
    let rowid_alias = primary_key_count == 1
        && spec
            .columns
            .iter()
            .any(|column| column.primary_key == 1 && column.ty == "INTEGER");
    if rowid_alias {
        let index = spec
            .columns
            .iter()
            .position(|column| column.primary_key == 1)
            .ok_or("typed snapshot rowid key")?;
        if cells[index].0 != 1
            || cells[index].1 != SqlValue::Integer(rowid.ok_or("typed snapshot rowid absent")?)
        {
            return Err("typed snapshot integer primary-key rowid differs".into());
        }
    }
    for ((tag, value), column) in cells.iter().zip(spec.columns) {
        if !expected_tag(column, *tag) || (column.not_null && *tag == 0) {
            return Err(format!("typed snapshot {} storage profile", spec.name));
        }
        if *tag == 3 && !matches!(value, SqlValue::Blob(_)) {
            return Err("typed snapshot raw text carrier".into());
        }
    }

    let mut insert_names = Vec::with_capacity(spec.columns.len() + 1);
    let mut values_sql = Vec::with_capacity(spec.columns.len() + 1);
    let import_rowid = rowid.is_some() && !rowid_alias;
    if import_rowid {
        insert_names.push("rowid".to_owned());
        values_sql.push("?1".to_owned());
    }
    for (index, column) in spec.columns.iter().enumerate() {
        insert_names.push(quote(column.name));
        let bind = index + usize::from(import_rowid) + 1;
        values_sql.push(
            if column.ty == "TEXT" && encoding == DatabaseEncoding::Utf8 {
                format!("CAST(?{bind} AS TEXT)")
            } else {
                format!("?{bind}")
            },
        );
    }
    let returning = spec
        .columns
        .iter()
        .map(|column| {
            let name = quote(column.name);
            if column.ty == "TEXT" {
                format!("typeof({name}),CAST({name} AS BLOB)")
            } else {
                format!("typeof({name}),{name}")
            }
        })
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "INSERT INTO {} ({}) VALUES ({}) RETURNING {}",
        quote(spec.name),
        insert_names.join(","),
        values_sql.join(","),
        returning
    );
    let mut bind_values = Vec::with_capacity(cells.len() + usize::from(import_rowid));
    if import_rowid {
        bind_values.push(SqlValue::Integer(rowid.unwrap()));
    }
    for (column, (tag, value)) in spec.columns.iter().zip(cells) {
        bind_values.push(if *tag == 3 {
            match value {
                SqlValue::Blob(bytes) if encoding == DatabaseEncoding::Utf8 => {
                    SqlValue::Blob(bytes.clone())
                }
                SqlValue::Blob(bytes) => {
                    SqlValue::Text(decode_utf16_text(allocations, bytes, encoding)?)
                }
                _ => return Err("typed snapshot text bytes".into()),
            }
        } else {
            value.clone()
        });
        if column.ty == "TEXT" && *tag != 0 && *tag != 3 {
            return Err("typed snapshot text storage class".into());
        }
    }
    let mut statement = db
        .prepare(&sql)
        .map_err(|error| format!("typed snapshot insert plan: {error}"))?;
    let mut rows = statement
        .query(params_from_iter(bind_values.iter()))
        .map_err(|error| format!("typed snapshot insert: {error}"))?;
    let returned = rows
        .next()
        .map_err(|error| error.to_string())?
        .ok_or("typed snapshot insert returned no row")?;
    for (index, ((tag, value), column)) in cells.iter().zip(spec.columns).enumerate() {
        if !columns.iter().any(|evidence| evidence.name == column.name) {
            return Err("typed snapshot column evidence changed".into());
        }
        compare_returned(returned, index * 2, *tag, value)?;
    }
    if rows.next().map_err(|error| error.to_string())?.is_some() {
        return Err("typed snapshot insert returned multiple rows".into());
    }
    Ok(())
}

fn parse_table(
    cursor: &mut Cursor<'_>,
    role_tables: &[TableSpec],
    expected_id: u16,
    encoding: DatabaseEncoding,
    db: &Connection,
    max_cell_bytes: usize,
) -> Result<TableEvidence, String> {
    let start = cursor.position;
    let id = cursor.u16()?;
    let present = match cursor.u8()? {
        0 => false,
        1 => true,
        _ => return Err("typed snapshot presence flag".into()),
    };
    let flags = cursor.u8()?;
    if flags & !0x07 != 0 {
        return Err("typed snapshot table flags".into());
    }
    let name = cursor.sized_text16()?;
    let spec = role_tables
        .iter()
        .find(|table| table.id == expected_id)
        .ok_or("typed snapshot table registry")?;
    if id != expected_id || name != spec.name {
        return Err("typed snapshot known table identity".into());
    }
    let column_count = cursor.u16()? as usize;
    let index_count = cursor.u16()? as usize;
    let row_count = cursor.u64()?;
    if !present {
        if flags != 0 || column_count != 0 || index_count != 0 || row_count != 0 {
            return Err("typed snapshot absent-table record".into());
        }
        return Ok(TableEvidence {
            id,
            name,
            present,
            flags,
            columns: Vec::new(),
            indexes: Vec::new(),
            row_count,
            wire_range: start..cursor.position,
        });
    }
    if flags & 1 != 0 || flags & 2 != 0 || flags & 4 == 0 {
        return Err(format!(
            "typed snapshot {} table storage profile",
            spec.name
        ));
    }
    if column_count > MAX_COLUMNS
        || column_count != spec.columns.len()
        || index_count > MAX_INDEXES
        || column_count > cursor.bytes.len().saturating_sub(cursor.position) / 10
        || index_count > cursor.bytes.len().saturating_sub(cursor.position) / 12
        || row_count > cursor.bytes.len() as u64
    {
        return Err(format!("typed snapshot {} descriptor bounds", spec.name));
    }

    cursor.charge_vec::<ColumnEvidence>(column_count)?;
    let mut columns = Vec::with_capacity(column_count);
    for _ in 0..column_count {
        let name = cursor.sized_text16()?;
        let declared_type = cursor.sized_text16()?;
        let not_null = match cursor.u8()? {
            0 => false,
            1 => true,
            _ => return Err("typed snapshot not-null flag".into()),
        };
        let primary_key = cursor.u16()?;
        let hidden = cursor.u8()?;
        let default = cursor.optional_bytes32()?;
        columns.push(ColumnEvidence {
            name,
            declared_type,
            not_null,
            primary_key,
            hidden,
            default,
        });
    }
    validate_columns(spec, &columns)?;
    cursor.charge_vec::<IndexEvidence>(index_count)?;
    let mut indexes = Vec::with_capacity(index_count);
    for _ in 0..index_count {
        indexes.push(parse_index(cursor)?);
    }
    validate_owner_indexes(spec, &indexes)?;
    create_table(db, spec)?;

    for _ in 0..row_count {
        let rowid = if flags & 4 != 0 {
            Some(cursor.i64()?)
        } else {
            None
        };
        let mut cells = Vec::with_capacity(column_count);
        for column in spec.columns {
            let (tag, value) = sql_value(cursor, max_cell_bytes, encoding)?;
            if !expected_tag(column, tag) {
                return Err(format!("typed snapshot {} column storage class", spec.name));
            }
            cells.push((tag, value));
        }
        import_row(
            db,
            encoding,
            cursor.allocations,
            spec,
            rowid,
            &cells,
            &columns,
        )?;
    }
    install_search_indexes(db, spec, &indexes)?;
    Ok(TableEvidence {
        id,
        name,
        present,
        flags,
        columns,
        indexes,
        row_count,
        wire_range: start..cursor.position,
    })
}

fn parse_opaque_table(
    cursor: &mut Cursor<'_>,
    max_cell_bytes: usize,
) -> Result<TableEvidence, String> {
    let start = cursor.position;
    let id = cursor.u16()?;
    let present = match cursor.u8()? {
        0 => false,
        1 => true,
        _ => return Err("typed snapshot opaque presence".into()),
    };
    let flags = cursor.u8()?;
    if id != u16::MAX || !present || flags & !0x07 != 0 || (flags & 1 != 0) == (flags & 4 != 0) {
        return Err("typed snapshot opaque table header".into());
    }
    let name = cursor.sized_text16()?;
    if name.is_empty() {
        return Err("typed snapshot opaque table name".into());
    }
    let column_count = cursor.u16()? as usize;
    let index_count = cursor.u16()? as usize;
    let row_count = cursor.u64()?;
    if column_count == 0
        || column_count > MAX_COLUMNS
        || index_count > MAX_INDEXES
        || row_count > cursor.bytes.len() as u64
    {
        return Err("typed snapshot opaque descriptor bounds".into());
    }
    cursor.charge_vec::<ColumnEvidence>(column_count)?;
    let mut columns = Vec::with_capacity(column_count);
    for _ in 0..column_count {
        let name = cursor.sized_text16()?;
        let declared_type = cursor.sized_text16()?;
        let not_null = match cursor.u8()? {
            0 => false,
            1 => true,
            _ => return Err("typed snapshot opaque not-null flag".into()),
        };
        let primary_key = cursor.u16()?;
        let hidden = cursor.u8()?;
        let default = cursor.optional_bytes32()?;
        columns.push(ColumnEvidence {
            name,
            declared_type,
            not_null,
            primary_key,
            hidden,
            default,
        });
    }
    cursor.charge_vec::<IndexEvidence>(index_count)?;
    let mut indexes = Vec::with_capacity(index_count);
    for _ in 0..index_count {
        indexes.push(parse_index(cursor)?);
    }
    for _ in 0..row_count {
        if flags & 4 != 0 {
            let _ = cursor.i64()?;
        }
        for _ in 0..column_count {
            skip_cell(cursor, max_cell_bytes)?;
        }
    }
    Ok(TableEvidence {
        id,
        name,
        present,
        flags,
        columns,
        indexes,
        row_count,
        wire_range: start..cursor.position,
    })
}

fn schema_object(cursor: &mut Cursor<'_>) -> Result<SchemaObject, String> {
    let kind = cursor.u8()?;
    if !(1..=4).contains(&kind) {
        return Err("typed snapshot schema object kind".into());
    }
    let name = cursor.sized_text16()?;
    let table_name = cursor.sized_text16()?;
    let sql = cursor
        .optional_bytes32()?
        .map(|raw| {
            String::from_utf8(raw).map_err(|_| "typed snapshot schema SQL is not UTF-8".to_owned())
        })
        .transpose()?;
    if name.is_empty() || table_name.is_empty() {
        return Err("typed snapshot schema object name".into());
    }
    if kind == 1
        && sql
            .as_deref()
            .is_some_and(|sql| contains_ascii_phrase(sql, "CREATE VIRTUAL TABLE"))
    {
        return Err("typed snapshot virtual tables are unsupported".into());
    }
    Ok(SchemaObject {
        kind,
        name,
        table_name,
        sql,
    })
}

fn contains_ascii_phrase(haystack: &str, phrase: &str) -> bool {
    haystack
        .as_bytes()
        .windows(phrase.len())
        .any(|window| window.eq_ignore_ascii_case(phrase.as_bytes()))
}

fn contains_ascii_word(haystack: &str, word: &str) -> bool {
    haystack
        .as_bytes()
        .windows(word.len())
        .enumerate()
        .any(|(index, window)| {
            if !window.eq_ignore_ascii_case(word.as_bytes()) {
                return false;
            }
            let identifier = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_';
            let before_is_identifier = index > 0 && identifier(haystack.as_bytes()[index - 1]);
            let after = index + word.len();
            let after_is_identifier =
                after < haystack.len() && identifier(haystack.as_bytes()[after]);
            !before_is_identifier && !after_is_identifier
        })
}

fn object_kind_name(kind: u8) -> &'static str {
    match kind {
        1 => "table",
        2 => "index",
        3 => "trigger",
        _ => "view",
    }
}

// WITHOUT ROWID primary keys are the table b-tree itself. SQLite exposes
// their index_list/index_xinfo descriptors but no separate sqlite_schema row.
// Only this structurally complete descriptor can be absent from that inventory.
fn implicit_without_rowid_primary(table: &TableEvidence, index: &IndexEvidence) -> bool {
    if table.id != u16::MAX
        || !matches!(table.flags, 1 | 3)
        || index.origin != 2
        || !index.unique
        || index.partial
        || index.keys.is_empty()
        || table.indexes.iter().filter(|item| item.origin == 2).count() != 1
    {
        return false;
    }
    let prefix = format!("sqlite_autoindex_{}_", table.name);
    let Some(suffix) = index.name.strip_prefix(&prefix) else {
        return false;
    };
    if suffix.is_empty()
        || !suffix.bytes().all(|byte| byte.is_ascii_digit())
        || suffix.parse::<u64>().ok().is_none_or(|number| number == 0)
    {
        return false;
    }
    let primary_count = table
        .columns
        .iter()
        .filter(|column| column.primary_key > 0)
        .count();
    if primary_count == 0 || table.columns.iter().any(|column| column.hidden != 0) {
        return false;
    }
    let mut seen_columns = BTreeSet::new();
    let mut primary_ordinal = 0usize;
    let mut payload_started = false;
    for key in &index.keys {
        let Ok(cid) = usize::try_from(key.cid) else {
            return false;
        };
        let Some(column) = table.columns.get(cid) else {
            return false;
        };
        if !seen_columns.insert(cid)
            || key.name.as_deref() != Some(column.name.as_str())
            || key.collation.is_empty()
        {
            return false;
        }
        if key.key {
            if payload_started {
                return false;
            }
            primary_ordinal += 1;
            if usize::from(column.primary_key) != primary_ordinal || !column.not_null {
                return false;
            }
        } else {
            payload_started = true;
            if column.primary_key != 0 {
                return false;
            }
        }
    }
    primary_ordinal == primary_count && seen_columns.len() == table.columns.len()
}

fn validate_schema_objects(
    objects: &[SchemaObject],
    tables: &[TableEvidence],
) -> Result<(), String> {
    let mut table_names = BTreeSet::new();
    let mut index_names = BTreeSet::new();
    let mut evidenced_index_names = BTreeSet::new();
    let mut descriptor_index_names = BTreeSet::new();
    let mut previous: Option<(&str, &str)> = None;
    for object in objects {
        let key = (object_kind_name(object.kind), object.name.as_str());
        if previous.is_some_and(|prior| prior >= key) {
            return Err("typed snapshot schema object order or uniqueness".into());
        }
        previous = Some(key);
        if object.kind == 1 {
            if object.table_name != object.name {
                return Err("typed snapshot table schema identity differs".into());
            }
            if tables.iter().any(|table| {
                table.id != u16::MAX
                    && table.present
                    && table.name.as_str() == object.name.as_str()
                    && object
                        .sql
                        .as_deref()
                        .is_some_and(|sql| contains_ascii_word(sql, "COLLATE"))
            }) {
                return Err("typed snapshot owner table collation is unsupported".into());
            }
            table_names.insert(object.name.as_str());
        }
        if object.kind == 2 {
            index_names.insert((object.name.as_str(), object.table_name.as_str()));
        }
        if object.kind == 2 && object.sql.is_none() && !object.name.starts_with("sqlite_autoindex_")
        {
            return Err("typed snapshot null index SQL".into());
        }
        if object.kind != 2
            && object.sql.is_none()
            && !(object.kind == 1 && object.name == "sqlite_sequence")
        {
            return Err("typed snapshot null schema SQL".into());
        }
    }
    if index_names
        .iter()
        .any(|(_, table_name)| !table_names.contains(*table_name))
    {
        return Err("typed snapshot index references absent table".into());
    }
    let expected_table_count = tables.iter().filter(|table| table.present).count();
    if expected_table_count != table_names.len()
        || tables
            .iter()
            .filter(|table| table.present)
            .any(|table| !table_names.contains(table.name.as_str()))
    {
        return Err("typed snapshot schema table inventory differs".into());
    }
    for table in tables.iter().filter(|table| table.present) {
        for index in &table.indexes {
            let key = (index.name.as_str(), table.name.as_str());
            if !descriptor_index_names.insert(key) {
                return Err("typed snapshot duplicate index descriptor".into());
            }
            if index_names.contains(&key) {
                evidenced_index_names.insert(key);
            } else if !implicit_without_rowid_primary(table, index) {
                return Err("typed snapshot index schema inventory differs".into());
            }
        }
    }
    if evidenced_index_names != index_names {
        return Err("typed snapshot index schema evidence differs".into());
    }
    Ok(())
}

fn hex(digest: Digest256) -> String {
    digest.to_hex()
}

fn build_inventory(
    input_field: &str,
    encoding: DatabaseEncoding,
    frame_bytes: u64,
    frame_sha: String,
    schema_sha: String,
    opaque_sha: String,
    opaque: Vec<Value>,
) -> Value {
    json!({
        "input_field": input_field,
        "database_encoding": encoding.pragma(),
        "frame_bytes": frame_bytes,
        "frame_sha256": frame_sha,
        "schema_objects_sha256": schema_sha,
        "opaque_tables_sha256": opaque_sha,
        "opaque_tables": opaque,
    })
}

pub(crate) fn import(
    path: &Path,
    role: Role,
    input_field: &str,
    remaining_frame_bytes: u64,
    schema_allocation_bytes: u64,
    vm_steps: u64,
    max_cell_bytes: usize,
) -> Result<ImportedFrame, String> {
    let mut guard = tos_fd_open::open_absolute_regular(path, remaining_frame_bytes)
        .map_err(|error| format!("open typed snapshot frame: {error}"))?;
    let metadata = guard.metadata().map_err(|error| error.to_string())?;
    let frame_bytes = metadata.len();
    if frame_bytes < (HEADER_BYTES + FOOTER_BYTES) as u64 || frame_bytes > remaining_frame_bytes {
        return Err("typed snapshot frame byte budget".into());
    }
    let selected_identity = (metadata.dev(), metadata.ino());
    let read_limit = frame_bytes
        .checked_add(1)
        .ok_or("typed snapshot frame length overflow")?;
    let mut raw = Vec::with_capacity(
        usize::try_from(frame_bytes).map_err(|_| "typed snapshot frame length")?,
    );
    guard
        .by_ref()
        .take(read_limit)
        .read_to_end(&mut raw)
        .map_err(|error| error.to_string())?;
    if raw.len() as u64 != frame_bytes {
        return Err("typed snapshot frame changed while reading".into());
    }
    let current = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if path.is_symlink()
        || !current.is_file()
        || (current.dev(), current.ino()) != selected_identity
    {
        return Err("typed snapshot selected file identity changed".into());
    }
    let frame_sha = hex(Digest256::of_bytes(&raw));
    let trailer_at = raw.len() - FOOTER_BYTES;
    let expected_trailer = Digest256::of_bytes(&raw[..trailer_at]);
    if raw[trailer_at..] != expected_trailer.as_bytes()[..] {
        return Err("typed snapshot frame checksum".into());
    }
    let mut schema_allocation = SchemaAllocationBudget::new(schema_allocation_bytes);
    let mut cursor = Cursor::new(&raw[..trailer_at], &mut schema_allocation);
    if cursor.take(MAGIC.len())? != MAGIC {
        return Err("typed snapshot magic".into());
    }
    if cursor.u8()? != role as u8 {
        return Err("typed snapshot role".into());
    }
    let encoding = DatabaseEncoding::parse(cursor.u8()?)?;
    let known_count = cursor.u16()? as usize;
    let opaque_count = cursor.u16()? as usize;
    let schema_count = cursor.u32()? as usize;
    let declared_total = cursor.u64()?;
    let role_tables = tables(role);
    if known_count != role_tables.len()
        || schema_count > MAX_SCHEMA_OBJECTS
        || declared_total != frame_bytes
        || opaque_count > schema_count
        || schema_count > raw.len() / 7
    {
        return Err("typed snapshot header bounds".into());
    }
    if cursor.position != HEADER_BYTES {
        return Err("typed snapshot header layout".into());
    }

    let mut db = Connection::open_in_memory().map_err(|error| error.to_string())?;
    let max_sqlite_record = sqlite_record_length_limit(frame_bytes, role)?;
    db.set_limit(
        rusqlite::limits::Limit::SQLITE_LIMIT_LENGTH,
        max_sqlite_record,
    )
    .map_err(|error| error.to_string())?;
    db.set_limit(rusqlite::limits::Limit::SQLITE_LIMIT_SQL_LENGTH, 1_000_000)
        .map_err(|error| error.to_string())?;
    let mut vm_used = 0u64;
    db.progress_handler(
        1000,
        Some(move || {
            vm_used = vm_used.saturating_add(1000);
            vm_used > vm_steps
        }),
    );
    db.pragma_update(None, "encoding", encoding.pragma())
        .map_err(|error| format!("typed snapshot database encoding: {error}"))?;
    require_memory_temp_store(&db)?;

    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| format!("typed snapshot import transaction: {error}"))?;
    let evidence_capacity = known_count
        .checked_add(opaque_count)
        .ok_or("typed snapshot evidence count overflow")?;
    cursor.charge_vec::<TableEvidence>(evidence_capacity)?;
    let mut evidence = Vec::with_capacity(evidence_capacity);
    for expected_id in expected_ids(role) {
        evidence.push(parse_table(
            &mut cursor,
            role_tables,
            *expected_id,
            encoding,
            &tx,
            max_cell_bytes,
        )?);
    }
    let mut opaque_hasher = Digest256Hasher::new();
    cursor.charge_vec::<Value>(opaque_count)?;
    let mut opaque_inventory = Vec::with_capacity(opaque_count);
    let mut previous_opaque_name: Option<String> = None;
    for _ in 0..opaque_count {
        let table = parse_opaque_table(
            &mut cursor,
            usize::try_from(remaining_frame_bytes).unwrap_or(usize::MAX),
        )?;
        if previous_opaque_name
            .as_deref()
            .is_some_and(|prior| prior.as_bytes() >= table.name.as_bytes())
        {
            return Err("typed snapshot opaque table order".into());
        }
        cursor.charge(table.name.len())?;
        previous_opaque_name = Some(table.name.clone());
        let raw_record = raw
            .get(table.wire_range.clone())
            .ok_or("typed snapshot opaque record range")?;
        opaque_hasher.update(raw_record);
        cursor.charge(512usize.saturating_add(table.name.len()))?;
        opaque_inventory.push(json!({
            "name_sha256": digest_text(table.name.as_bytes()),
            "row_count": table.row_count,
            "logical_sha256": digest_text(raw_record),
        }));
        evidence.push(table);
    }
    let schema_start = cursor.position;
    cursor.charge_vec::<SchemaObject>(schema_count)?;
    let mut objects = Vec::with_capacity(schema_count);
    for _ in 0..schema_count {
        objects.push(schema_object(&mut cursor)?);
    }
    let schema_end = cursor.position;
    let schema_sha = digest_text(&raw[schema_start..schema_end]);
    if cursor.position != trailer_at {
        return Err("typed snapshot trailing frame bytes".into());
    }
    cursor.charge(
        schema_count
            .saturating_add(evidence.len())
            .saturating_mul(128),
    )?;
    validate_schema_objects(&objects, &evidence)?;

    if evidence
        .iter()
        .filter(|table| table.id == u16::MAX)
        .any(|table| role_tables.iter().any(|known| known.name == table.name))
    {
        return Err("typed snapshot opaque table shadows owner table".into());
    }
    tx.commit()
        .map_err(|error| format!("typed snapshot import commit: {error}"))?;
    let installed_encoding: String = db
        .query_row("PRAGMA encoding", [], |row| row.get(0))
        .map_err(|error| error.to_string())?;
    if installed_encoding != encoding.pragma() {
        return Err("typed snapshot imported database encoding differs".into());
    }
    Ok(ImportedFrame {
        connection: db,
        guard,
        selected_path: path.to_owned(),
        selected_identity,
        inventory: build_inventory(
            input_field,
            encoding,
            frame_bytes,
            frame_sha.clone(),
            schema_sha,
            hex(opaque_hasher.finalize()),
            opaque_inventory,
        ),
        frame_sha256: frame_sha,
        frame_bytes,
    })
}

fn digest_text(bytes: &[u8]) -> String {
    Digest256::of_bytes(bytes).to_hex()
}

fn require_memory_temp_store(db: &Connection) -> Result<(), String> {
    let can_force_memory: i64 = db.query_row(
        "SELECT sqlite_compileoption_used('TEMP_STORE=1') OR sqlite_compileoption_used('TEMP_STORE=2') OR sqlite_compileoption_used('TEMP_STORE=3')",
        [], |row| row.get(0),
    ).map_err(|error| format!("SQLite temp-store compile mode unavailable: {error}"))?;
    if can_force_memory != 1 {
        return Err("SQLite temp-store compile mode cannot prove memory placement".into());
    }
    db.execute_batch("PRAGMA temp_store=MEMORY;")
        .map_err(|error| error.to_string())?;
    let mode: i64 = db
        .query_row("PRAGMA temp_store", [], |row| row.get(0))
        .map_err(|error| error.to_string())?;
    if mode != 2 {
        return Err("SQLite temp-store memory placement did not take effect".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs::OpenOptions,
        io::Write,
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT_FRAME: AtomicU64 = AtomicU64::new(0);

    fn text16(output: &mut Vec<u8>, value: &str) {
        let bytes = value.as_bytes();
        output.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
        output.extend_from_slice(bytes);
    }

    fn empty_prepared_state_frame(encoding: u8) -> Vec<u8> {
        let mut frame = Vec::new();
        frame.extend_from_slice(MAGIC);
        frame.push(Role::Prepared as u8);
        frame.push(encoding);
        frame.extend_from_slice(&(PREPARED_TABLES.len() as u16).to_le_bytes());
        frame.extend_from_slice(&0u16.to_le_bytes());
        frame.extend_from_slice(&1u32.to_le_bytes());
        frame.extend_from_slice(&0u64.to_le_bytes()); // filled after records

        for id in expected_ids(Role::Prepared) {
            let spec = PREPARED_TABLES
                .iter()
                .find(|table| table.id == *id)
                .unwrap();
            let present = spec.name == "prepared_state";
            frame.extend_from_slice(&id.to_le_bytes());
            frame.push(u8::from(present));
            frame.push(if present { 4 } else { 0 }); // ordinary rowid table
            text16(&mut frame, spec.name);
            if present {
                frame.extend_from_slice(&4u16.to_le_bytes()); // columns
                frame.extend_from_slice(&0u16.to_le_bytes()); // indexes
                frame.extend_from_slice(&0u64.to_le_bytes()); // rows
                for (name, ty, not_null, primary_key) in [
                    ("singleton", "INTEGER", false, 1u16),
                    ("high_water", "INTEGER", true, 0),
                    ("max_pages", "INTEGER", true, 0),
                    ("descriptor", "TEXT", true, 0),
                ] {
                    text16(&mut frame, name);
                    text16(&mut frame, ty);
                    frame.push(u8::from(not_null));
                    frame.extend_from_slice(&primary_key.to_le_bytes());
                    frame.push(0); // not generated/hidden
                    frame.extend_from_slice(&u32::MAX.to_le_bytes()); // no default
                }
            } else {
                frame.extend_from_slice(&0u16.to_le_bytes());
                frame.extend_from_slice(&0u16.to_le_bytes());
                frame.extend_from_slice(&0u64.to_le_bytes());
            }
        }
        frame.push(1); // table schema object
        text16(&mut frame, "prepared_state");
        text16(&mut frame, "prepared_state");
        let sql = b"CREATE TABLE prepared_state(singleton INTEGER PRIMARY KEY CHECK(singleton=1),high_water INTEGER NOT NULL,max_pages INTEGER NOT NULL,descriptor TEXT NOT NULL)";
        frame.extend_from_slice(&(sql.len() as u32).to_le_bytes());
        frame.extend_from_slice(sql);
        let total = frame.len() + FOOTER_BYTES;
        frame[18..26].copy_from_slice(&(total as u64).to_le_bytes());
        let checksum = Digest256::of_bytes(&frame);
        frame.extend_from_slice(checksum.as_bytes());
        frame
    }

    fn frame_file(bytes: &[u8]) -> PathBuf {
        let id = NEXT_FRAME.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "tos-typed-snapshot-{}-{id}.frame",
            std::process::id()
        ));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        file.write_all(bytes).unwrap();
        file.sync_all().unwrap();
        path
    }

    #[test]
    fn owner_index_profile_keeps_primary_and_unique_keys_exact() {
        let spec = &PREPARED_TABLES[3]; // prepared_documents
        let indexes = vec![
            IndexEvidence {
                name: "sqlite_autoindex_prepared_documents_1".into(),
                unique: true,
                origin: 2,
                partial: false,
                keys: vec![
                    IndexKey {
                        cid: 0,
                        name: Some("kind".into()),
                        descending: false,
                        collation: "BINARY".into(),
                        key: true,
                    },
                    IndexKey {
                        cid: 1,
                        name: Some("id".into()),
                        descending: false,
                        collation: "BINARY".into(),
                        key: true,
                    },
                ],
            },
            IndexEvidence {
                name: "sqlite_autoindex_prepared_documents_2".into(),
                unique: true,
                origin: 1,
                partial: false,
                keys: vec![IndexKey {
                    cid: 2,
                    name: Some("doc_id".into()),
                    descending: false,
                    collation: "BINARY".into(),
                    key: true,
                }],
            },
        ];
        assert!(validate_owner_indexes(spec, &indexes).is_ok());

        let mut wrong_primary_key = indexes.clone();
        wrong_primary_key[0].keys[0].cid = 1;
        assert!(validate_owner_indexes(spec, &wrong_primary_key).is_err());

        let mut partial_unique = indexes.clone();
        partial_unique[1].partial = true;
        assert!(validate_owner_indexes(spec, &partial_unique).is_err());

        let mut extra_unique = indexes;
        extra_unique.push(IndexEvidence {
            name: "prepared_documents_source_order_uq".into(),
            unique: true,
            origin: 1,
            partial: false,
            keys: vec![IndexKey {
                cid: 3,
                name: Some("source_order".into()),
                descending: false,
                collation: "BINARY".into(),
                key: true,
            }],
        });
        assert!(validate_owner_indexes(spec, &extra_unique).is_err());
    }

    #[test]
    fn sqlite_length_limit_allows_record_header_beyond_cell_payload() {
        let frame_bytes = 512u64;
        let limit = sqlite_record_length_limit(frame_bytes, Role::Prepared).unwrap();
        assert!(u64::try_from(limit).unwrap() > frame_bytes);

        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE captured(key TEXT NOT NULL,part INTEGER NOT NULL,json_chunk TEXT NOT NULL,PRIMARY KEY(key,part))",
        )
        .unwrap();
        let payload = "x".repeat(frame_bytes as usize);
        db.set_limit(
            rusqlite::limits::Limit::SQLITE_LIMIT_LENGTH,
            frame_bytes as i32,
        )
        .unwrap();
        assert!(
            db.execute(
                "INSERT INTO captured VALUES(?1,?2,?3)",
                rusqlite::params!["revision", 0i64, &payload],
            )
            .is_err()
        );

        db.set_limit(rusqlite::limits::Limit::SQLITE_LIMIT_LENGTH, limit)
            .unwrap();
        assert_eq!(
            db.execute(
                "INSERT INTO captured VALUES(?1,?2,?3)",
                rusqlite::params!["revision", 0i64, payload],
            )
            .unwrap(),
            1
        );
    }

    #[test]
    fn without_rowid_implicit_primary_keeps_schema_inventory_exact() {
        let column = |name: &str, primary_key| ColumnEvidence {
            name: name.into(),
            declared_type: "TEXT".into(),
            not_null: true,
            primary_key,
            hidden: 0,
            default: None,
        };
        let key = |cid, name: &str, is_primary| IndexKey {
            cid,
            name: Some(name.into()),
            descending: false,
            collation: "BINARY".into(),
            key: is_primary,
        };
        let mut table = TableEvidence {
            id: u16::MAX,
            name: "opaque".into(),
            present: true,
            flags: 1,
            columns: vec![column("id", 1), column("value", 0)],
            indexes: vec![IndexEvidence {
                name: "sqlite_autoindex_opaque_1".into(),
                unique: true,
                origin: 2,
                partial: false,
                keys: vec![key(0, "id", true), key(1, "value", false)],
            }],
            row_count: 0,
            wire_range: 0..0,
        };
        let objects = vec![SchemaObject {
            kind: 1,
            name: "opaque".into(),
            table_name: "opaque".into(),
            sql: Some("CREATE TABLE opaque(id TEXT PRIMARY KEY,value TEXT) WITHOUT ROWID".into()),
        }];
        assert!(validate_schema_objects(&objects, &[table.clone()]).is_ok());
        let mut strict = table.clone();
        strict.flags = 3; // STRICT does not add a separate primary-key schema row.
        assert!(validate_schema_objects(&objects, &[strict]).is_ok());
        for (flags, origin, unique, partial) in [
            (4, 2, true, false),
            (1, 0, true, false),
            (1, 1, true, false),
            (1, 2, false, false),
            (1, 2, true, true),
        ] {
            let mut wrong = table.clone();
            wrong.flags = flags;
            wrong.indexes[0].origin = origin;
            wrong.indexes[0].unique = unique;
            wrong.indexes[0].partial = partial;
            assert!(validate_schema_objects(&objects, &[wrong]).is_err());
        }
        let mut wrong = table.clone();
        wrong.indexes[0].keys[0].name = Some("value".into());
        assert!(validate_schema_objects(&objects, &[wrong]).is_err());
        let mut wrong = table.clone();
        wrong.indexes[0].keys.swap(0, 1);
        assert!(validate_schema_objects(&objects, &[wrong]).is_err());
        let mut wrong = table.clone();
        wrong.indexes[0].keys.pop();
        assert!(validate_schema_objects(&objects, &[wrong]).is_err());
        let mut wrong = table.clone();
        wrong.indexes.push(wrong.indexes[0].clone());
        assert!(validate_schema_objects(&objects, &[wrong]).is_err());
        table.indexes.push(IndexEvidence {
            name: "opaque_value_idx".into(),
            unique: false,
            origin: 0,
            partial: false,
            keys: vec![key(1, "value", true)],
        });
        assert!(validate_schema_objects(&objects, &[table.clone()]).is_err());
        let mut complete = objects;
        complete.insert(
            0,
            SchemaObject {
                kind: 2,
                name: "opaque_value_idx".into(),
                table_name: "opaque".into(),
                sql: Some("CREATE INDEX opaque_value_idx ON opaque(value)".into()),
            },
        );
        assert!(validate_schema_objects(&complete, &[table.clone()]).is_ok());
        complete.insert(
            1,
            SchemaObject {
                kind: 2,
                name: "orphan_idx".into(),
                table_name: "opaque".into(),
                sql: Some("CREATE INDEX orphan_idx ON opaque(value)".into()),
            },
        );
        assert!(validate_schema_objects(&complete, &[table]).is_err());
    }

    #[test]
    fn utf16_raw_carrier_is_charged_before_copy() {
        let mut wire = vec![3];
        wire.extend_from_slice(&2u64.to_le_bytes());
        wire.extend_from_slice(&[0x41, 0x00]);
        let mut allocations = SchemaAllocationBudget::new(1);
        let mut cursor = Cursor::new(&wire, &mut allocations);
        assert!(sql_value(&mut cursor, 2, DatabaseEncoding::Utf16Le).is_err());
        assert_eq!(cursor.position, 9);
    }

    #[test]
    fn owner_table_collation_is_refused_without_copying_sql() {
        let table = TableEvidence {
            id: 0x0024,
            name: "prepared_state".into(),
            present: true,
            flags: 4,
            columns: Vec::new(),
            indexes: Vec::new(),
            row_count: 0,
            wire_range: 0..0,
        };
        let object = SchemaObject {
            kind: 1,
            name: "prepared_state".into(),
            table_name: "prepared_state".into(),
            sql: Some("CREATE TABLE prepared_state(value TEXT COLLATE NOCASE)".into()),
        };
        assert!(validate_schema_objects(&[object], &[table]).is_err());
        assert!(contains_ascii_word("CREATE TABLE collated_value(value TEXT)", "COLLATE") == false);
    }

    #[test]
    fn prepared_import_preserves_absent_versus_empty_tables() {
        let bytes = empty_prepared_state_frame(DatabaseEncoding::Utf8 as u8);
        let path = frame_file(&bytes);
        let imported = import(
            &path,
            Role::Prepared,
            "after_prepared_database",
            bytes.len() as u64,
            64 * 1024 * 1024,
            1_000_000,
            32 * 1024 * 1024,
        )
        .unwrap();
        let rows: i64 = imported
            .connection
            .query_row("SELECT count(*) FROM prepared_state", [], |row| row.get(0))
            .unwrap();
        assert_eq!(rows, 0);
        assert!(
            imported
                .connection
                .query_row("SELECT count(*) FROM edge_meta", [], |row| row
                    .get::<_, i64>(0))
                .is_err()
        );
        assert_eq!(imported.inventory["input_field"], "after_prepared_database");
        assert_eq!(imported.inventory["database_encoding"], "UTF-8");
        drop(imported);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn malformed_encoding_and_truncated_frames_are_refused() {
        let mut invalid_tag = empty_prepared_state_frame(0);
        let trailer_at = invalid_tag.len() - FOOTER_BYTES;
        let checksum = Digest256::of_bytes(&invalid_tag[..trailer_at]);
        invalid_tag[trailer_at..].copy_from_slice(checksum.as_bytes());
        let path = frame_file(&invalid_tag);
        assert!(
            import(
                &path,
                Role::Prepared,
                "after_prepared_database",
                invalid_tag.len() as u64,
                64 * 1024 * 1024,
                1_000_000,
                32 * 1024 * 1024,
            )
            .is_err()
        );
        fs::remove_file(path).unwrap();

        let valid = empty_prepared_state_frame(DatabaseEncoding::Utf8 as u8);
        let truncated = &valid[..valid.len() - 1];
        let path = frame_file(truncated);
        assert!(
            import(
                &path,
                Role::Prepared,
                "after_prepared_database",
                truncated.len() as u64,
                64 * 1024 * 1024,
                1_000_000,
                32 * 1024 * 1024,
            )
            .is_err()
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn utf16_text_binding_retains_bom_and_nul_bytes_and_rejects_invalid_sequences() {
        for encoding in [DatabaseEncoding::Utf16Le, DatabaseEncoding::Utf16Be] {
            let db = Connection::open_in_memory().unwrap();
            db.pragma_update(None, "encoding", encoding.pragma())
                .unwrap();
            db.execute_batch("CREATE TABLE raw_text(value TEXT NOT NULL)")
                .unwrap();
            let raw = match encoding {
                DatabaseEncoding::Utf16Le => {
                    vec![0xff, 0xfe, 0x41, 0x00, 0x00, 0x00, 0x3d, 0xd8, 0x00, 0xde]
                }
                DatabaseEncoding::Utf16Be => {
                    vec![0xfe, 0xff, 0x00, 0x41, 0x00, 0x00, 0xd8, 0x3d, 0xde, 0x00]
                }
                DatabaseEncoding::Utf8 => unreachable!(),
            };
            let mut allocations = SchemaAllocationBudget::new(1024);
            let text = decode_utf16_text(&mut allocations, &raw, encoding).unwrap();
            let (storage, returned): (String, Vec<u8>) = db
                .query_row(
                    "INSERT INTO raw_text VALUES(?1) RETURNING typeof(value),CAST(value AS BLOB)",
                    [SqlValue::Text(text)],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            assert_eq!(storage, "text");
            assert_eq!(returned, raw);
        }

        let mut allocations = SchemaAllocationBudget::new(1024);
        assert!(decode_utf16_text(&mut allocations, &[0x41], DatabaseEncoding::Utf16Le).is_err());
        assert!(
            decode_utf16_text(&mut allocations, &[0x00, 0xd8], DatabaseEncoding::Utf16Le).is_err()
        );
        assert!(
            decode_utf16_text(&mut allocations, &[0xdc, 0x00], DatabaseEncoding::Utf16Be).is_err()
        );
    }
}
