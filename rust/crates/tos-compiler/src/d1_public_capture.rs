//! Exact, disposable capture of the three allowlisted public graph projections
//! and selected original-carrier files. Rows live on disk in source encounter
//! order. This is a read-model input, never a source, rights, canon or
//! installed-current grant.

use crate::{Error, Limits, Result, legacy::decode_partition_part, safe_open, sqlite_budget};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use serde::de::{DeserializeSeed, Error as _, MapAccess, SeqAccess, Visitor};
use serde_json::value::RawValue;
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    fmt,
    fs::{self, File},
    io::{self, BufReader, Read, Seek, SeekFrom, Write},
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{Arc, atomic::AtomicU64},
    time::Instant,
};
use tos_foundation::{
    Digest256, Digest256Hasher, JsonLimits, JsonMode, JsonValue, emit_python_compact_json,
    parse_json,
};

const CORPUS: &str = "corpus";
const PHILOSOPHY: &str = "philosophy";
const CLAIMS: &str = "bibliographic";
const PHILOSOPHY_AUDIT_RELATIVE: &str =
    "ToS/philosophy/graph-workbench/review-packets/table-i-post-planting-audit.json";
const MAX_ROOT_BYTES: u64 = 256 * 1024;
pub(crate) const MAX_ROW_BYTES: usize = 8 * 1024 * 1024;
pub(crate) const MAX_HEADER_BYTES: usize = 2 * 1024 * 1024;
const MAX_INDEX_BYTES: usize = 128 * 1024;
const MAX_PART_BYTES: usize = 8 * 1024 * 1024;
const STORED_OVERHEAD: usize = 65536;

#[derive(Clone, Copy, Debug)]
pub struct PublicCaptureLimits {
    pub max_input_bytes: u64,
    pub max_rows: u64,
    pub max_staging_bytes: u64,
    pub max_work_bytes: u64,
    pub max_sql_vm_steps: u64,
    pub sqlite_cache_kib: u32,
}

/// Exact seven-path selector inherited from Reference Core construction.
/// Five files form the whole graph/catalog source identity; the two optional
/// files retain their selected-path presence for isolated header/evidence
/// calls. Paths are inputs, never copied or re-rooted under `root`.
#[derive(Clone, Debug)]
pub struct PublicCaptureInputPaths {
    pub index_path: PathBuf,
    pub philosophy_graph_projection_path: PathBuf,
    pub bibliographic_graph_path: PathBuf,
    pub entity_type_registry_path: PathBuf,
    pub relation_type_registry_path: PathBuf,
    pub philosophy_post_planting_audit_path: PathBuf,
    pub evidence_projection_path: PathBuf,
}

/// One original source carrier needed by a lower-level Core operation. These
/// profiles deliberately omit unrelated graph roles and registries so an
/// index or bibliography read still works on a partial source root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeCaptureRole {
    Corpus,
    Philosophy,
    Bibliographic,
    PhilosophyAudit,
}

fn check_capture_active(
    cancelled: Option<&std::sync::atomic::AtomicBool>,
    deadline: Instant,
) -> Result<()> {
    if cancelled.is_some_and(|value| value.load(std::sync::atomic::Ordering::Relaxed)) {
        return Err(Error::Budget("public D1 capture cancelled"));
    }
    if Instant::now() >= deadline {
        return Err(Error::Budget("public D1 capture deadline"));
    }
    Ok(())
}

impl PublicCaptureInputPaths {
    fn validate(&self) -> Result<()> {
        for path in [
            &self.index_path,
            &self.philosophy_graph_projection_path,
            &self.bibliographic_graph_path,
            &self.entity_type_registry_path,
            &self.relation_type_registry_path,
            &self.philosophy_post_planting_audit_path,
            &self.evidence_projection_path,
        ] {
            if !path.is_absolute() || path.to_str().is_none() {
                return Err(Error::Invalid("selected public D1 input path"));
            }
        }
        Ok(())
    }

    fn core_path(&self, relative: &str) -> Option<&Path> {
        match relative {
            "ToS/derived-exports/tos_corpus_index.min.json" => Some(&self.index_path),
            "ToS/derived-exports/philosophy_graph_projection.min.json" => {
                Some(&self.philosophy_graph_projection_path)
            }
            "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json" => {
                Some(&self.bibliographic_graph_path)
            }
            "ToS/doctrine/semantic-interchange/entity-types.v1.json" => {
                Some(&self.entity_type_registry_path)
            }
            "ToS/doctrine/semantic-interchange/relation-types.v1.json" => {
                Some(&self.relation_type_registry_path)
            }
            _ => None,
        }
    }

    fn optional_path(&self, relative: &str) -> Option<&Path> {
        match relative {
            "ToS/derived-exports/epistemic_evidence_projection.min.json" => {
                Some(&self.evidence_projection_path)
            }
            "ToS/philosophy/graph-workbench/review-packets/table-i-post-planting-audit.json" => {
                Some(&self.philosophy_post_planting_audit_path)
            }
            _ => None,
        }
    }
}

impl PublicCaptureLimits {
    pub(crate) fn validate(self) -> Result<()> {
        if self.max_input_bytes == 0
            || self.max_rows == 0
            || self.max_staging_bytes == 0
            || self.max_work_bytes == 0
            || self.max_sql_vm_steps == 0
            || self.sqlite_cache_kib == 0
        {
            return Err(Error::Budget("public D1 capture limits"));
        }
        Ok(())
    }
    pub(crate) fn sqlite(self) -> Limits {
        Limits {
            max_rows: self.max_rows,
            max_row_bytes: MAX_ROW_BYTES,
            max_output_bytes: self.max_staging_bytes,
            max_work_bytes: self.max_work_bytes,
            sqlite_cache_kib: self.sqlite_cache_kib,
            max_sql_vm_steps: self.max_sql_vm_steps,
        }
    }
}

/// A private source-byte snapshot. No publication is possible from this
/// capture without the full build's final input recheck and output completion.
pub struct PublicCapture {
    root: PathBuf,
    prepared_profile: bool,
    evidence_profile: bool,
    runtime_capture_role: Option<RuntimeCaptureRole>,
    prepared_state: Vec<(PathBuf, PathBuf, u64, u64, i64, i64, i64, i64)>,
    path: PathBuf,
    inode: (u64, u64),
    sources: Vec<SourceFile>,
    partitioned: bool,
    file_state: (u64, u64, u64, i64, i64, i64, i64),
    pub rows: u64,
    work_bytes: Arc<AtomicU64>,
    max_work_bytes: u64,
    deadline: Instant,
    limits: PublicCaptureLimits,
    vm_used: Arc<AtomicU64>,
    cancelled: Arc<std::sync::atomic::AtomicBool>,
}

enum SourceOrigin {
    File(PathBuf),
    Compiled(&'static [u8]),
}
struct SourceFile {
    label: String,
    origin: SourceOrigin,
    digest: Option<Digest256>,
    len: u64,
}

struct PendingCapture<'a> {
    path: &'a Path,
    complete: bool,
}
impl Drop for PendingCapture<'_> {
    fn drop(&mut self) {
        if self.complete {
            return;
        }
        for suffix in ["-journal", "-wal", "-shm", ""] {
            let mut name = self.path.as_os_str().to_os_string();
            name.push(suffix);
            let _ = fs::remove_file(PathBuf::from(name));
        }
    }
}

// Maintained prepare selects these five carriers, including their original
// symlink resolution and path/mtime/size/inode/ctime currentness observations.
const PREPARED_INPUTS: [&str; 5] = [
    "ToS/derived-exports/tos_corpus_index.min.json",
    "ToS/derived-exports/philosophy_graph_projection.min.json",
    "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json",
    "ToS/doctrine/semantic-interchange/entity-types.v1.json",
    "ToS/doctrine/semantic-interchange/relation-types.v1.json",
];
fn prepared_state(root: &Path) -> Result<Vec<(PathBuf, PathBuf, u64, u64, i64, i64, i64, i64)>> {
    PREPARED_INPUTS
        .iter()
        .map(|name| {
            let selected = root.join(name);
            let resolved = fs::canonicalize(&selected)?;
            let m = fs::metadata(&selected)?;
            Ok((
                selected,
                resolved,
                m.len(),
                m.ino(),
                m.mtime(),
                m.mtime_nsec(),
                m.ctime(),
                m.ctime_nsec(),
            ))
        })
        .collect()
}
fn profile_open(path: &Path, cap: u64, prepared: bool) -> Result<File> {
    if prepared {
        safe_open::open_regular(&fs::canonicalize(path)?, cap)
    } else {
        safe_open::open_regular(path, cap)
    }
}

fn checked_add(work: &mut u64, bytes: usize, limit: u64) -> Result<()> {
    *work = work
        .checked_add(bytes as u64)
        .filter(|value| *value <= limit)
        .ok_or(Error::Budget("public D1 capture work"))?;
    Ok(())
}

pub(crate) fn json(raw: &[u8], cap: usize) -> Result<JsonValue> {
    let limits = JsonLimits::new(cap, 96, 1_000_000, 4096)
        .map_err(|_| Error::Budget("public D1 JSON limits"))?;
    Ok(parse_json(raw, JsonMode::PublishedStrict, limits)
        .map_err(|e| Error::Source(e.to_string()))?
        .root()
        .clone())
}

pub(crate) fn compact(value: &JsonValue, cap: usize) -> Result<Vec<u8>> {
    emit_python_compact_json(
        value,
        JsonLimits::new(cap, 96, 1_000_000, 4096)
            .map_err(|_| Error::Budget("public D1 JSON output"))?,
    )
    .map_err(|e| Error::Source(e.to_string()))
}

fn selected_rows(role: &str, collection: &str) -> bool {
    match role {
        "evidence-corpus" => matches!(collection, "nodes" | "relation_edges"),
        "evidence-philosophy" => collection == "views",
        CORPUS => matches!(
            collection,
            "diagnostics"
                | "nodes"
                | "resources"
                | "manifests"
                | "branches"
                | "graph_views"
                | "relation_edges"
                | "relation_packs"
                | "source_navigation/nodes"
                | "source_navigation/edges"
                | "source_navigation/rights"
        ),
        PHILOSOPHY => matches!(
            collection,
            "nodes" | "edges" | "clusters" | "views" | "review_packets" | "graph_layers"
        ),
        CLAIMS => matches!(
            collection,
            "nodes" | "edges" | "claim_traces" | "input_digests"
        ),
        _ => false,
    }
}

fn valid_top_level_collection(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes.next().is_some_and(|first| first.is_ascii_lowercase())
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

struct CaptureWriter<'a> {
    db: &'a Connection,
    role: &'static str,
    row_count: &'a mut u64,
    work: &'a mut u64,
    limits: PublicCaptureLimits,
    ordinals: BTreeMap<String, u64>,
    read_budget: Rc<Cell<usize>>,
    deadline: Instant,
    cancelled: Option<&'a std::sync::atomic::AtomicBool>,
    dynamic_philosophy: bool,
    page_rows: usize,
    page_bytes: usize,
}

// serde_json's RawValue owns a row before the row callback can inspect it.
// Bound bytes delivered to that allocation, including punctuation/whitespace.
// The underlying file reader has a fixed 64 KiB buffer.
struct CaptureReader<'a, R> {
    inner: R,
    remaining: Rc<Cell<usize>>,
    deadline: Instant,
    cancelled: Option<&'a std::sync::atomic::AtomicBool>,
}
impl<R: Read> Read for CaptureReader<'_, R> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if self
            .cancelled
            .is_some_and(|value| value.load(std::sync::atomic::Ordering::Relaxed))
            || Instant::now() >= self.deadline
        {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "public D1 capture cancelled or expired",
            ));
        }
        if output.is_empty() {
            return Ok(0);
        }
        let left = self.remaining.get();
        if left == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "public D1 JSON value read bound",
            ));
        }
        let available = output.len().min(left);
        let n = self.inner.read(&mut output[..available])?;
        self.remaining.set(left - n);
        Ok(n)
    }
}

impl CaptureWriter<'_> {
    fn page_step(&mut self, bytes: usize) -> Result<()> {
        check_capture_active(self.cancelled, self.deadline)?;
        self.page_rows = self
            .page_rows
            .checked_add(1)
            .ok_or(Error::Budget("public D1 capture page rows"))?;
        self.page_bytes = self
            .page_bytes
            .checked_add(bytes)
            .ok_or(Error::Budget("public D1 capture page bytes"))?;
        if self.page_rows >= 128 || self.page_bytes >= 4 * 1024 * 1024 {
            self.db.execute_batch("COMMIT; BEGIN IMMEDIATE")?;
            self.page_rows = 0;
            self.page_bytes = 0;
        }
        Ok(())
    }
    fn collection(&mut self, collection: &str, kind: &str) -> Result<()> {
        check_capture_active(self.cancelled, self.deadline)?;
        checked_add(self.work, collection.len(), self.limits.max_work_bytes)?;
        let source_navigation_object =
            self.role == CORPUS && collection == "source_navigation" && kind == "object";
        let dynamic = self.dynamic_philosophy
            && self.role == PHILOSOPHY
            && valid_top_level_collection(collection);
        if (!selected_rows(self.role, collection) && !source_navigation_object && !dynamic)
            || (!matches!(kind, "array" | "mapping") && !source_navigation_object)
        {
            return Err(Error::Invalid("public D1 collection declaration"));
        }
        self.db.execute(
            "INSERT INTO capture_collections(role,collection,kind) VALUES (?1,?2,?3)",
            params![self.role, collection, kind],
        )?;
        Ok(())
    }
    fn row(
        &mut self,
        collection: &str,
        raw: &[u8],
        source_key: Option<&str>,
        order: &[String],
        allow_non_object: bool,
    ) -> Result<()> {
        check_capture_active(self.cancelled, self.deadline)?;
        let dynamic = self.dynamic_philosophy
            && self.role == PHILOSOPHY
            && valid_top_level_collection(collection);
        if !selected_rows(self.role, collection) && !dynamic {
            return Err(Error::Invalid("public D1 collection outside fixed input"));
        }
        if raw.len() > MAX_ROW_BYTES {
            return Err(Error::Budget("public D1 row bytes"));
        }
        checked_add(self.work, raw.len(), self.limits.max_work_bytes)?;
        let value = json(raw, MAX_ROW_BYTES)?;
        if !allow_non_object && value.as_object().is_none() {
            return Err(Error::Invalid("public D1 row object"));
        }
        let next = self.ordinals.entry(collection.to_owned()).or_default();
        let source_key = source_key
            .map(str::to_owned)
            .unwrap_or_else(|| format!("{:020}", *next));
        if source_key.is_empty() || source_key.len() > 4096 || order.len() > 2 {
            return Err(Error::Budget("public D1 capture row key/order"));
        }
        let encoded = compact(&value, MAX_ROW_BYTES)?;
        let changed = self.db.execute(
            "INSERT INTO capture_rows(role,collection,source_key,ord,sort0,sort1,json,sha256) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![self.role, collection, source_key, *next as i64, order.first().map(String::as_str).unwrap_or(""), order.get(1).map(String::as_str).unwrap_or(""), &encoded, Digest256::of_bytes(&encoded).as_bytes().as_slice()],
        )?;
        if changed != 1 {
            return Err(Error::Invalid("public D1 capture insertion"));
        }
        *next = next
            .checked_add(1)
            .ok_or(Error::Budget("public D1 ordinal"))?;
        *self.row_count = self
            .row_count
            .checked_add(1)
            .filter(|count| *count <= self.limits.max_rows)
            .ok_or(Error::Budget("public D1 capture rows"))?;
        checked_add(
            self.work,
            encoded.len() + source_key.len() + order.iter().map(String::len).sum::<usize>(),
            self.limits.max_work_bytes,
        )?;
        self.page_step(encoded.len())?;
        Ok(())
    }

    fn header(&mut self, path: &str, raw: &[u8]) -> Result<()> {
        if Instant::now() >= self.deadline {
            return Err(Error::Budget("public D1 capture deadline"));
        }
        if raw.len() > MAX_HEADER_BYTES {
            return Err(Error::Budget("public D1 header bytes"));
        }
        let value = json(raw, MAX_HEADER_BYTES)?;
        let encoded = compact(&value, MAX_HEADER_BYTES)?;
        checked_add(
            self.work,
            raw.len() + encoded.len(),
            self.limits.max_work_bytes,
        )?;
        self.db.execute(
            "INSERT INTO capture_headers(role,path,json) VALUES (?1,?2,?3)",
            params![self.role, path, encoded],
        )?;
        self.page_step(raw.len())?;
        Ok(())
    }
}

struct RowsSeed<'a, 'b> {
    writer: &'a mut CaptureWriter<'b>,
    collection: String,
}

impl<'de> DeserializeSeed<'de> for RowsSeed<'_, '_> {
    type Value = ();

    fn deserialize<D: serde::Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> std::result::Result<(), D::Error> {
        struct RowsVisitor<'a, 'b> {
            writer: &'a mut CaptureWriter<'b>,
            collection: String,
        }
        impl<'de> Visitor<'de> for RowsVisitor<'_, '_> {
            type Value = ();
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a public projection row array")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> std::result::Result<(), A::Error> {
                self.writer
                    .collection(&self.collection, "array")
                    .map_err(A::Error::custom)?;
                loop {
                    self.writer.read_budget.set(MAX_ROW_BYTES + 65536);
                    let Some(raw) = seq.next_element::<Box<RawValue>>()? else {
                        break;
                    };
                    let allow_non_object = self.writer.dynamic_philosophy;
                    self.writer
                        .row(
                            &self.collection,
                            raw.get().as_bytes(),
                            None,
                            &[],
                            allow_non_object,
                        )
                        .map_err(A::Error::custom)?;
                }
                Ok(())
            }
        }
        deserializer.deserialize_seq(RowsVisitor {
            writer: self.writer,
            collection: self.collection,
        })
    }
}

struct PhilosophyMemberSeed<'a, 'b> {
    writer: &'a mut CaptureWriter<'b>,
    collection: String,
}

struct CheckedValueSeed;

impl<'de> DeserializeSeed<'de> for CheckedValueSeed {
    type Value = serde_json::Value;

    fn deserialize<D: serde::Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> std::result::Result<Self::Value, D::Error> {
        struct CheckedValueVisitor;

        impl<'de> Visitor<'de> for CheckedValueVisitor {
            type Value = serde_json::Value;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a strict JSON value without duplicate object keys")
            }

            fn visit_bool<E: serde::de::Error>(
                self,
                value: bool,
            ) -> std::result::Result<Self::Value, E> {
                Ok(serde_json::Value::Bool(value))
            }

            fn visit_i64<E: serde::de::Error>(
                self,
                value: i64,
            ) -> std::result::Result<Self::Value, E> {
                Ok(serde_json::Value::Number(value.into()))
            }

            fn visit_u64<E: serde::de::Error>(
                self,
                value: u64,
            ) -> std::result::Result<Self::Value, E> {
                Ok(serde_json::Value::Number(value.into()))
            }

            fn visit_f64<E: serde::de::Error>(
                self,
                value: f64,
            ) -> std::result::Result<Self::Value, E> {
                serde_json::Number::from_f64(value)
                    .map(serde_json::Value::Number)
                    .ok_or_else(|| E::custom("non-finite philosophy projection number"))
            }

            fn visit_str<E: serde::de::Error>(
                self,
                value: &str,
            ) -> std::result::Result<Self::Value, E> {
                Ok(serde_json::Value::String(value.to_owned()))
            }

            fn visit_string<E: serde::de::Error>(
                self,
                value: String,
            ) -> std::result::Result<Self::Value, E> {
                Ok(serde_json::Value::String(value))
            }

            fn visit_none<E: serde::de::Error>(self) -> std::result::Result<Self::Value, E> {
                Ok(serde_json::Value::Null)
            }

            fn visit_unit<E: serde::de::Error>(self) -> std::result::Result<Self::Value, E> {
                Ok(serde_json::Value::Null)
            }

            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                while let Some(value) = seq.next_element_seed(CheckedValueSeed)? {
                    values.push(value);
                }
                Ok(serde_json::Value::Array(values))
            }

            fn visit_map<A: MapAccess<'de>>(
                self,
                mut map: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut fields = serde_json::Map::new();
                let mut seen = BTreeSet::new();
                while let Some(name) = map.next_key::<String>()? {
                    if !seen.insert(name.clone()) {
                        return Err(A::Error::custom(
                            "duplicate philosophy projection object key",
                        ));
                    }
                    let value = map.next_value_seed(CheckedValueSeed)?;
                    fields.insert(name, value);
                }
                Ok(serde_json::Value::Object(fields))
            }
        }

        deserializer.deserialize_any(CheckedValueVisitor)
    }
}

impl<'de> DeserializeSeed<'de> for PhilosophyMemberSeed<'_, '_> {
    type Value = ();

    fn deserialize<D: serde::Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> std::result::Result<(), D::Error> {
        struct MemberVisitor<'a, 'b> {
            writer: &'a mut CaptureWriter<'b>,
            collection: String,
        }

        impl MemberVisitor<'_, '_> {
            fn header<E: serde::de::Error>(
                &mut self,
                value: serde_json::Value,
            ) -> std::result::Result<(), E> {
                let raw = serde_json::to_vec(&value).map_err(E::custom)?;
                self.writer
                    .header(&self.collection, &raw)
                    .map_err(E::custom)
            }
        }

        impl<'de> Visitor<'de> for MemberVisitor<'_, '_> {
            type Value = ();

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a philosophy projection member")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> std::result::Result<(), A::Error> {
                self.writer
                    .collection(&self.collection, "array")
                    .map_err(A::Error::custom)?;
                loop {
                    self.writer.read_budget.set(MAX_ROW_BYTES + 65536);
                    let Some(raw) = seq.next_element::<Box<RawValue>>()? else {
                        break;
                    };
                    self.writer
                        .row(&self.collection, raw.get().as_bytes(), None, &[], true)
                        .map_err(A::Error::custom)?;
                }
                Ok(())
            }

            fn visit_map<A: MapAccess<'de>>(mut self, map: A) -> std::result::Result<(), A::Error> {
                let value = CheckedValueSeed
                    .deserialize(serde::de::value::MapAccessDeserializer::new(map))?;
                self.header(value)
            }

            fn visit_bool<E: serde::de::Error>(
                mut self,
                value: bool,
            ) -> std::result::Result<(), E> {
                self.header(serde_json::Value::Bool(value))
            }

            fn visit_i64<E: serde::de::Error>(mut self, value: i64) -> std::result::Result<(), E> {
                self.header(serde_json::Value::Number(value.into()))
            }

            fn visit_u64<E: serde::de::Error>(mut self, value: u64) -> std::result::Result<(), E> {
                self.header(serde_json::Value::Number(value.into()))
            }

            fn visit_f64<E: serde::de::Error>(mut self, value: f64) -> std::result::Result<(), E> {
                let number = serde_json::Number::from_f64(value)
                    .ok_or_else(|| E::custom("non-finite philosophy projection number"))?;
                self.header(serde_json::Value::Number(number))
            }

            fn visit_str<E: serde::de::Error>(mut self, value: &str) -> std::result::Result<(), E> {
                self.header(serde_json::Value::String(value.to_owned()))
            }

            fn visit_string<E: serde::de::Error>(
                mut self,
                value: String,
            ) -> std::result::Result<(), E> {
                self.header(serde_json::Value::String(value))
            }

            fn visit_none<E: serde::de::Error>(mut self) -> std::result::Result<(), E> {
                self.header(serde_json::Value::Null)
            }

            fn visit_unit<E: serde::de::Error>(mut self) -> std::result::Result<(), E> {
                self.header(serde_json::Value::Null)
            }

            fn visit_some<D: serde::Deserializer<'de>>(
                self,
                deserializer: D,
            ) -> std::result::Result<(), D::Error> {
                deserializer.deserialize_any(self)
            }

            fn visit_newtype_struct<D: serde::Deserializer<'de>>(
                self,
                deserializer: D,
            ) -> std::result::Result<(), D::Error> {
                deserializer.deserialize_any(self)
            }
        }

        deserializer.deserialize_any(MemberVisitor {
            writer: self.writer,
            collection: self.collection,
        })
    }
}

struct ObjectSeed<'a, 'b> {
    writer: &'a mut CaptureWriter<'b>,
    prefix: &'static str,
}

impl<'de> DeserializeSeed<'de> for ObjectSeed<'_, '_> {
    type Value = ();

    fn deserialize<D: serde::Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> std::result::Result<(), D::Error> {
        struct ObjectVisitor<'a, 'b> {
            writer: &'a mut CaptureWriter<'b>,
            prefix: &'static str,
        }
        impl<'de> Visitor<'de> for ObjectVisitor<'_, '_> {
            type Value = ();
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a public projection object")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> std::result::Result<(), A::Error> {
                if self.prefix == "source_navigation" && self.writer.role == CORPUS {
                    self.writer
                        .collection("source_navigation", "object")
                        .map_err(A::Error::custom)?;
                }
                let mut seen = BTreeSet::new();
                loop {
                    self.writer.read_budget.set(MAX_HEADER_BYTES + 65536);
                    let Some(key) = map.next_key::<String>()? else {
                        break;
                    };
                    if key.is_empty() || key.len() > 4096 {
                        return Err(A::Error::custom("public D1 member key bytes"));
                    }
                    checked_add(
                        self.writer.work,
                        key.len(),
                        self.writer.limits.max_work_bytes,
                    )
                    .map_err(A::Error::custom)?;
                    if !seen.insert(key.clone()) {
                        return Err(A::Error::custom("duplicate public projection member"));
                    }
                    let collection = if self.prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{}/{}", self.prefix, key)
                    };
                    if self.writer.role.starts_with("evidence-")
                        && !selected_rows(self.writer.role, &collection)
                        && collection != "schema_version"
                    {
                        self.writer.read_budget.set(self.writer.limits.max_input_bytes.min(usize::MAX as u64) as usize);
                        map.next_value::<serde::de::IgnoredAny>()?;
                        self.writer.read_budget.set(MAX_HEADER_BYTES + 65536);
                    } else if self.prefix.is_empty()
                        && key == "source_navigation"
                        && self.writer.role == CORPUS
                    {
                        map.next_value_seed(ObjectSeed {
                            writer: self.writer,
                            prefix: "source_navigation",
                        })?;
                    } else if collection != "input_digests"
                        && selected_rows(self.writer.role, &collection)
                    {
                        map.next_value_seed(RowsSeed {
                            writer: self.writer,
                            collection,
                        })?;
                    } else if self.writer.dynamic_philosophy && self.prefix.is_empty() {
                        map.next_value_seed(PhilosophyMemberSeed {
                            writer: self.writer,
                            collection,
                        })?;
                    } else {
                        let raw: Box<RawValue> = map.next_value()?;
                        self.writer
                            .header(&collection, raw.get().as_bytes())
                            .map_err(A::Error::custom)?;
                    }
                }
                Ok(())
            }
        }
        deserializer.deserialize_map(ObjectVisitor {
            writer: self.writer,
            prefix: self.prefix,
        })
    }
}

pub(crate) fn source_digest(
    file: &mut File,
    cap: u64,
    mut charge: impl FnMut(usize) -> Result<()>,
) -> Result<(Digest256, u64)> {
    file.seek(SeekFrom::Start(0))?;
    let mut hash = Digest256Hasher::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let size = file.read(&mut buffer)?;
        if size == 0 {
            break;
        }
        total = total
            .checked_add(size as u64)
            .filter(|n| *n <= cap)
            .ok_or(Error::Budget("public D1 source bytes"))?;
        charge(size)?;
        hash.update(&buffer[..size]);
    }
    file.seek(SeekFrom::Start(0))?;
    Ok((hash.finalize(), total))
}

fn strict_value(raw: &[u8], cap: usize) -> Result<serde_json::Value> {
    json(raw, cap)?;
    serde_json::from_slice(raw).map_err(|e| Error::Source(e.to_string()))
}

fn field<'a>(value: &'a serde_json::Value, name: &str) -> Result<&'a str> {
    value
        .get(name)
        .and_then(serde_json::Value::as_str)
        .ok_or(Error::Invalid("public projection descriptor field"))
}

fn number(value: &serde_json::Value, name: &str) -> Result<u64> {
    value
        .get(name)
        .and_then(serde_json::Value::as_u64)
        .ok_or(Error::Invalid("public projection descriptor count"))
}

fn policy(
    role: &str,
    collection: &str,
) -> Option<(&'static [&'static str], &'static [&'static str], bool)> {
    let role = match role {
        "evidence-corpus" => CORPUS,
        "evidence-philosophy" => PHILOSOPHY,
        other => other,
    };
    let entry = match (role, collection) {
        (CORPUS, "diagnostics") => (&[][..], &[][..], false),
        (CORPUS, "nodes") => (&["node_id"][..], &["source_path"][..], false),
        (CORPUS, "resources") | (CORPUS, "manifests") => (&["path"][..], &["path"][..], false),
        (CORPUS, "relation_packs") => (&["pack_id"][..], &["path"][..], false),
        (CORPUS, "relation_edges") => (
            &["pack_id", "edge_id"][..],
            &["pack_id", "edge_id"][..],
            false,
        ),
        (CORPUS, "source_navigation/nodes") => (&["node_id"][..], &["node_id"][..], false),
        (CORPUS, "source_navigation/edges") => (&["edge_id"][..], &["edge_id"][..], false),
        (CORPUS, "source_navigation/rights") => (&["rights_id"][..], &["rights_id"][..], false),
        (PHILOSOPHY, "nodes") => (&["node_id"][..], &["node_id"][..], false),
        (PHILOSOPHY, "edges") => (&["edge_id"][..], &["edge_id"][..], false),
        (PHILOSOPHY, "clusters") => (&["cluster_id"][..], &["cluster_id"][..], false),
        (PHILOSOPHY, "views") => (&["view_id"][..], &["order", "view_id"][..], false),
        (CLAIMS, "nodes") => (&["node_id"][..], &["node_id"][..], false),
        (CLAIMS, "edges") => (&["edge_id"][..], &["edge_id"][..], false),
        (CLAIMS, "claim_traces") => (&["claim_ref"][..], &["claim_ref"][..], false),
        (CLAIMS, "input_digests") => (&[][..], &[][..], true),
        _ => return None,
    };
    Some(entry)
}

fn order_value(value: Option<&JsonValue>) -> Result<String> {
    match value {
        None => Ok("1:".to_owned()),
        Some(JsonValue::Null) => Ok("1:None".to_owned()),
        Some(JsonValue::Bool(value)) => Ok(format!("1:{value}")),
        Some(JsonValue::String(value)) => Ok(format!(
            "1:{}",
            value
                .as_str()
                .ok_or(Error::Invalid("public projection order string"))?
        )),
        Some(JsonValue::Number(number)) if number.lexeme.chars().all(|c| c.is_ascii_digit()) => {
            let decimal = number.lexeme.as_str();
            Ok(format!("0:{:020}:{decimal}", decimal.len()))
        }
        Some(JsonValue::Number(number)) if number.lexeme == "-0" => {
            Ok("0:00000000000000000001:0".to_owned())
        }
        Some(JsonValue::Number(number))
            if number.lexeme.starts_with('-')
                && number.lexeme[1..].bytes().all(|byte| byte.is_ascii_digit()) =>
        {
            Ok(format!("1:{}", number.lexeme))
        }
        _ => Err(Error::Invalid("public projection order value")),
    }
}

fn partition_order(row: &JsonValue, fields: &[String], max_bytes: u64) -> Result<Vec<String>> {
    let mut result = Vec::with_capacity(fields.len());
    let mut total = 0u64;
    for field in fields {
        let order = order_value(row.object_get(field.as_str()))?;
        total = total
            .checked_add(order.len() as u64)
            .filter(|bytes| *bytes <= max_bytes)
            .ok_or(Error::Budget("public projection order bytes"))?;
        result.push(order);
    }
    Ok(result)
}

fn partition_record_key(value: &JsonValue, fields: &[String]) -> Result<String> {
    if fields.len() == 1 {
        return value
            .object_get(fields[0].as_str())
            .and_then(JsonValue::as_str)
            .filter(|key| !key.is_empty() && key.len() <= 4096)
            .map(str::to_owned)
            .ok_or(Error::Invalid("public projection record identity"));
    }
    let mut keys = Vec::with_capacity(fields.len());
    let mut encoded_len = 2usize;
    for field in fields {
        let key = value
            .object_get(field.as_str())
            .and_then(JsonValue::as_str)
            .filter(|key| !key.is_empty() && key.len() <= 4096)
            .ok_or(Error::Invalid("public projection record identity"))?;
        if keys.len() != 0 {
            encoded_len = encoded_len
                .checked_add(1)
                .ok_or(Error::Budget("public projection record identity bytes"))?;
        }
        encoded_len = encoded_len
            .checked_add(2)
            .filter(|bytes| *bytes <= 4096)
            .ok_or(Error::Budget("public projection record identity bytes"))?;
        for byte in key.as_bytes() {
            let escaped = match byte {
                b'"' | b'\\' | b'\x08' | b'\x0c' | b'\n' | b'\r' | b'\t' => 2,
                0x00..=0x1f => 6,
                _ => 1,
            };
            encoded_len = encoded_len
                .checked_add(escaped)
                .filter(|bytes| *bytes <= 4096)
                .ok_or(Error::Budget("public projection record identity bytes"))?;
        }
        keys.push(key);
    }
    if keys.len() == 1 {
        return Ok(keys[0].to_owned());
    }
    serde_json::to_string(&keys).map_err(|e| Error::Source(e.to_string()))
}

fn partition_collection_policy(
    role: &str,
    collection: &str,
    spec: &serde_json::Value,
    dynamic_philosophy: bool,
) -> Result<(Vec<String>, Vec<String>, bool)> {
    let object = spec
        .as_object()
        .ok_or(Error::Invalid("public projection collection spec"))?;
    if object.len() != 3
        || !["key_field", "order_fields", "root"]
            .iter()
            .all(|field| object.contains_key(*field))
    {
        return Err(Error::Invalid("public projection collection spec"));
    }
    if dynamic_philosophy {
        let (key_fields, mapping) = match spec.get("key_field") {
            Some(serde_json::Value::Null) => (Vec::new(), true),
            Some(serde_json::Value::String(field)) if !field.is_empty() => {
                (vec![field.clone()], false)
            }
            Some(serde_json::Value::Array(fields)) => {
                let fields = fields
                    .iter()
                    .map(|field| {
                        field
                            .as_str()
                            .filter(|field| !field.is_empty())
                            .map(str::to_owned)
                            .ok_or(Error::Invalid("public projection key field"))
                    })
                    .collect::<Result<Vec<_>>>()?;
                (fields, false)
            }
            _ => return Err(Error::Invalid("public projection key field")),
        };
        let declared_order = spec
            .get("order_fields")
            .and_then(serde_json::Value::as_array)
            .ok_or(Error::Invalid("public projection ordering fields"))?
            .iter()
            .map(|field| {
                field
                    .as_str()
                    .filter(|field| !field.is_empty())
                    .map(str::to_owned)
                    .ok_or(Error::Invalid("public projection ordering field"))
            })
            .collect::<Result<Vec<_>>>()?;
        if key_fields.is_empty() && !mapping && !declared_order.is_empty() {
            return Err(Error::Invalid("public projection positional ordering"));
        }
        let effective_order = if mapping || key_fields.is_empty() {
            Vec::new()
        } else if declared_order.is_empty() {
            key_fields.clone()
        } else {
            declared_order
        };
        return Ok((key_fields, effective_order, mapping));
    }

    let (key_fields, order_fields, mapping) = policy(role, collection).ok_or(Error::Invalid(
        "public projection collection outside owner policy",
    ))?;
    let expected_key = if mapping {
        serde_json::Value::Null
    } else if key_fields.is_empty() {
        serde_json::json!([])
    } else if key_fields.len() == 1 {
        serde_json::json!(key_fields[0])
    } else {
        serde_json::json!(key_fields)
    };
    let expected_order = serde_json::json!(order_fields);
    if spec.get("key_field") != Some(&expected_key)
        || spec.get("order_fields") != Some(&expected_order)
    {
        return Err(Error::Invalid("public projection owner ordering policy"));
    }
    Ok((
        key_fields.iter().map(|field| (*field).to_owned()).collect(),
        order_fields
            .iter()
            .map(|field| (*field).to_owned())
            .collect(),
        mapping,
    ))
}

fn encode_order_tuple(order: &[String], max_bytes: usize) -> Result<String> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let capacity = order.iter().try_fold(0usize, |total, field| {
        field
            .len()
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(1))
            .and_then(|bytes| total.checked_add(bytes))
            .filter(|bytes| *bytes <= max_bytes)
            .ok_or(Error::Budget("public projection order bytes"))
    })?;
    let mut encoded = String::with_capacity(capacity);
    for field in order {
        for byte in field.as_bytes() {
            encoded.push(HEX[(byte >> 4) as usize] as char);
            encoded.push(HEX[(byte & 0x0f) as usize] as char);
        }
        // Hex contains no exclamation mark, and the terminator sorts before
        // another encoded byte. This preserves lexicographic tuple order.
        encoded.push('!');
    }
    Ok(encoded)
}

fn capture_part(
    root_path: &Path,
    descriptor: &serde_json::Value,
    prefix: &str,
    writer: &mut CaptureWriter<'_>,
    collection: &str,
    key_fields: &[String],
    order_fields: &[String],
    mapping: bool,
    collection_count: u64,
) -> Result<u64> {
    let descriptor_fields = [
        "kind",
        "prefix",
        "path",
        "sha256",
        "size_bytes",
        "decoded_bytes",
        "decoded_sha256",
        "count",
    ];
    let members = descriptor
        .as_object()
        .ok_or(Error::Invalid("public projection descriptor"))?;
    if members.len() != descriptor_fields.len()
        || !descriptor_fields
            .iter()
            .all(|field| members.contains_key(*field))
    {
        return Err(Error::Invalid("public projection descriptor shape"));
    }
    let kind = field(descriptor, "kind")?;
    if kind != "index" && kind != "data" {
        return Err(Error::Invalid("public projection part kind"));
    }
    if field(descriptor, "prefix")? != prefix
        || prefix.len() > 64
        || !prefix
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err(Error::Invalid("public projection part prefix"));
    }
    let stored_sha = field(descriptor, "sha256")?;
    let decoded_sha = field(descriptor, "decoded_sha256")?;
    Digest256::from_hex(stored_sha).map_err(|_| Error::Invalid("public projection part digest"))?;
    Digest256::from_hex(decoded_sha)
        .map_err(|_| Error::Invalid("public projection decoded digest"))?;
    let stored_len = usize::try_from(number(descriptor, "size_bytes")?)
        .map_err(|_| Error::Budget("public projection stored bytes"))?;
    let decoded_len = usize::try_from(number(descriptor, "decoded_bytes")?)
        .map_err(|_| Error::Budget("public projection decoded bytes"))?;
    let cap = if kind == "index" {
        MAX_INDEX_BYTES
    } else {
        MAX_PART_BYTES
    };
    if stored_len > cap + STORED_OVERHEAD || decoded_len > cap {
        return Err(Error::Budget("public projection part bytes"));
    }
    let stem = root_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or(Error::Invalid("public projection stem"))?;
    let suffix = if kind == "index" {
        ".index.json"
    } else {
        ".jsonl.gz"
    };
    let relative = format!("{stem}.parts/{}/{}{}", &stored_sha[..2], stored_sha, suffix);
    if field(descriptor, "path")? != relative {
        return Err(Error::Invalid("public projection part namespace"));
    }
    let path = root_path
        .parent()
        .ok_or(Error::Invalid("public projection parent"))?
        .join(relative);
    let mut file = safe_open::open_regular(&path, stored_len as u64)?;
    let (actual_sha, actual_len) = source_digest(&mut file, stored_len as u64, |n| {
        if Instant::now() >= writer.deadline {
            return Err(Error::Budget("public D1 capture deadline"));
        }
        checked_add(writer.work, n, writer.limits.max_work_bytes)
    })?;
    if actual_len != stored_len as u64 || actual_sha.to_hex() != stored_sha {
        return Err(Error::Invalid("public projection part changed"));
    }
    let mut stored = Vec::with_capacity(stored_len);
    file.take(stored_len as u64 + 1).read_to_end(&mut stored)?;
    if stored.len() != stored_len {
        return Err(Error::Invalid("public projection part length changed"));
    }
    checked_add(writer.work, decoded_len, writer.limits.max_work_bytes)?;
    let decoded = decode_partition_part(
        &stored,
        kind,
        stored_len,
        decoded_len,
        stored_sha,
        decoded_sha,
    )?;
    writer.db.execute(
        "INSERT OR IGNORE INTO capture_sources(path,sha256,size_bytes) VALUES (?1,?2,?3)",
        params![
            path.to_string_lossy().as_ref(),
            actual_sha.as_bytes().as_slice(),
            actual_len as i64
        ],
    )?;
    let expected = number(descriptor, "count")?;
    if kind == "index" {
        let index = strict_value(&decoded, MAX_INDEX_BYTES)?;
        let index_members = index
            .as_object()
            .ok_or(Error::Invalid("public projection index object"))?;
        if index_members.len() != 4
            || !["schema_version", "prefix", "count", "children"]
                .iter()
                .all(|field| index_members.contains_key(*field))
        {
            return Err(Error::Invalid("public projection index shape"));
        }
        if field(&index, "schema_version")? != "tos_projection_partition_index_v1"
            || field(&index, "prefix")? != prefix
            || number(&index, "count")? != expected
        {
            return Err(Error::Invalid("public projection partition index"));
        }
        let children = index
            .get("children")
            .and_then(serde_json::Value::as_object)
            .filter(|children| !children.is_empty() && prefix.len() < 64)
            .ok_or(Error::Invalid("public projection partition children"))?;
        let mut total = 0u64;
        for (digit, child) in children {
            if digit.len() != 1
                || !digit
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            {
                return Err(Error::Invalid("public projection partition digit"));
            }
            total = total
                .checked_add(capture_part(
                    root_path,
                    child,
                    &format!("{prefix}{digit}"),
                    writer,
                    collection,
                    key_fields,
                    order_fields,
                    mapping,
                    collection_count,
                )?)
                .ok_or(Error::Budget("public projection partition count"))?;
        }
        if total != expected {
            return Err(Error::Invalid("public projection partition count"));
        }
        return Ok(total);
    }
    if !decoded.is_empty() && decoded.last() != Some(&b'\n') {
        return Err(Error::Invalid("public projection line feed"));
    }
    let body = if decoded.is_empty() {
        &decoded[..]
    } else {
        &decoded[..decoded.len() - 1]
    };
    let mut count = 0u64;
    let mut previous: Option<String> = None;
    for line in body.split(|b| *b == b'\n').filter(|_| !decoded.is_empty()) {
        if line.is_empty() {
            return Err(Error::Invalid("public projection empty row"));
        }
        let record = json(line, MAX_ROW_BYTES)?;
        let object = record
            .as_object()
            .ok_or(Error::Invalid("public projection row object"))?;
        if object.len() != 2 {
            return Err(Error::Invalid("public projection row shape"));
        }
        let key = record
            .object_get("key")
            .and_then(JsonValue::as_str)
            .filter(|key| !key.is_empty() && key.len() <= 4096)
            .ok_or(Error::Invalid("public projection row key"))?;
        if previous.as_deref().is_some_and(|prior| key <= prior)
            || !Digest256::of_bytes(key.as_bytes())
                .to_hex()
                .starts_with(prefix)
        {
            return Err(Error::Invalid("public projection row order/placement"));
        }
        previous = Some(key.to_owned());
        let value = record
            .object_get("value")
            .ok_or(Error::Invalid("public projection row value"))?;
        let normalized = if mapping {
            &record
        } else {
            if key_fields.is_empty() {
                if key.len() != 20
                    || !key.bytes().all(|b| b.is_ascii_digit())
                    || key
                        .parse::<u64>()
                        .map_err(|_| Error::Invalid("public projection position"))?
                        >= collection_count
                {
                    return Err(Error::Invalid("public projection position"));
                }
            } else if partition_record_key(value, key_fields)? != key {
                return Err(Error::Invalid("public projection row identity"));
            }
            value
        };
        let encoded = compact(&normalized, MAX_ROW_BYTES)?;
        let order = if key_fields.is_empty() {
            vec![key.to_owned()]
        } else {
            partition_order(
                value,
                order_fields,
                writer.limits.max_work_bytes.saturating_sub(*writer.work),
            )?
        };
        let order = if writer.dynamic_philosophy {
            if mapping || key_fields.is_empty() {
                vec![key.to_owned()]
            } else {
                vec![
                    encode_order_tuple(
                        &order,
                        usize::try_from(writer.limits.max_work_bytes.saturating_sub(*writer.work))
                            .unwrap_or(usize::MAX),
                    )?,
                    Digest256::of_bytes(key.as_bytes()).to_hex(),
                ]
            }
        } else {
            order
        };
        let allow_non_object = writer.dynamic_philosophy && key_fields.is_empty() && !mapping;
        writer.row(collection, &encoded, Some(key), &order, allow_non_object)?;
        count = count
            .checked_add(1)
            .ok_or(Error::Budget("public projection row count"))?;
    }
    if count != expected {
        return Err(Error::Invalid("public projection leaf count"));
    }
    Ok(count)
}

fn capture_partitioned(root_path: &Path, raw: &[u8], writer: &mut CaptureWriter<'_>) -> Result<()> {
    let root = strict_value(raw, MAX_ROOT_BYTES as usize)?;
    let object = root
        .as_object()
        .ok_or(Error::Invalid("public projection manifest object"))?;
    if object.len() != 5
        || ![
            "schema_version",
            "logical_schema",
            "header",
            "limits",
            "collections",
        ]
        .iter()
        .all(|key| object.contains_key(*key))
        || field(&root, "schema_version")? != "tos_partitioned_projection_v1"
    {
        return Err(Error::Invalid("public projection manifest shape"));
    }
    let schema = field(&root, "logical_schema")?;
    let expected_schema = match writer.role {
        CORPUS | "evidence-corpus" => "tos_corpus_index_v1",
        PHILOSOPHY | "evidence-philosophy" => {
            if writer.dynamic_philosophy
                && matches!(
                    schema,
                    "tos_philosophy_graph_projection_v1" | "tos_philosophy_graph_projection_v2"
                )
            {
                schema
            } else {
                "tos_philosophy_graph_projection_v2"
            }
        }
        CLAIMS => "tos_source_witness_bibliographic_graph_v1",
        _ => return Err(Error::Invalid("public projection role")),
    };
    if schema != expected_schema || root["header"]["schema_version"].as_str() != Some(schema) {
        return Err(Error::Invalid("public projection logical schema"));
    }
    let limits = root
        .get("limits")
        .and_then(serde_json::Value::as_object)
        .ok_or(Error::Invalid("public projection limits"))?;
    if limits.len() != 4
        || limits.get("root_bytes").and_then(serde_json::Value::as_u64) != Some(MAX_ROOT_BYTES)
        || limits
            .get("index_bytes")
            .and_then(serde_json::Value::as_u64)
            != Some(MAX_INDEX_BYTES as u64)
        || limits.get("part_bytes").and_then(serde_json::Value::as_u64)
            != Some(MAX_PART_BYTES as u64)
        || limits.get("key_bytes").and_then(serde_json::Value::as_u64) != Some(4096)
    {
        return Err(Error::Invalid("public projection limit profile"));
    }
    let exact = json(raw, MAX_ROOT_BYTES as usize)?;
    let header = exact
        .object_get("header")
        .and_then(JsonValue::as_object)
        .ok_or(Error::Invalid("public projection header"))?;
    for (key, value) in header {
        let name = key
            .as_str()
            .ok_or(Error::Invalid("public projection header key"))?;
        if writer.role.starts_with("evidence-") && name != "schema_version" {
            continue;
        }
        if name == "source_navigation" && writer.role == CORPUS {
            let navigation = value
                .as_object()
                .ok_or(Error::Invalid("public projection navigation header"))?;
            for (nested, nested_value) in navigation {
                let nested = nested
                    .as_str()
                    .ok_or(Error::Invalid("public projection navigation key"))?;
                writer.header(
                    &format!("source_navigation/{nested}"),
                    &compact(nested_value, MAX_HEADER_BYTES)?,
                )?;
            }
        } else {
            writer.header(name, &compact(value, MAX_HEADER_BYTES)?)?;
        }
    }
    let collections = root
        .get("collections")
        .and_then(serde_json::Value::as_object)
        .filter(|collections| !collections.is_empty())
        .ok_or(Error::Invalid("public projection collections"))?;
    let required: &[&str] = if writer.dynamic_philosophy {
        &[]
    } else {
        match writer.role {
            "evidence-corpus" => &["nodes", "relation_edges"],
            "evidence-philosophy" => &["views"],
            CORPUS => &[
                "nodes",
                "resources",
                "manifests",
                "relation_packs",
                "relation_edges",
                "source_navigation/nodes",
                "source_navigation/edges",
                "source_navigation/rights",
            ],
            PHILOSOPHY => &["nodes", "edges", "clusters", "views"],
            CLAIMS => &["nodes", "edges", "claim_traces", "input_digests"],
            _ => return Err(Error::Invalid("public projection role")),
        }
    };
    if !required.iter().all(|name| collections.contains_key(*name)) {
        return Err(Error::Invalid(
            "public projection missing maintained collection",
        ));
    }
    for (name, spec) in collections {
        if writer.dynamic_philosophy && !valid_top_level_collection(name) {
            return Err(Error::Invalid("partitioned philosophy collection name"));
        }
        if writer.role.starts_with("evidence-") && !selected_rows(writer.role, name) {
            continue;
        }
        let (key_fields, order_fields, mapping) =
            partition_collection_policy(writer.role, name, spec, writer.dynamic_philosophy)?;
        let header_collision: Option<i64> = writer
            .db
            .query_row(
                "SELECT 1 FROM capture_headers WHERE role=?1 AND path=?2",
                params![writer.role, name],
                |row| row.get(0),
            )
            .optional()?;
        if header_collision.is_some() {
            return Err(Error::Invalid("partitioned collection overlaps header"));
        }
        writer.collection(name, if mapping { "mapping" } else { "array" })?;
        let descriptor = spec
            .get("root")
            .ok_or(Error::Invalid("public projection collection root"))?;
        let count = number(descriptor, "count")?;
        let found = capture_part(
            root_path,
            descriptor,
            "",
            writer,
            name,
            &key_fields,
            &order_fields,
            mapping,
            count,
        )?;
        if found != count {
            return Err(Error::Invalid("public projection collection count"));
        }
    }
    Ok(())
}

fn runtime_companion(label: &str) -> Option<&'static [u8]> {
    match label {
        "ToS/doctrine/semantic-interchange/query-vocabulary.v1.json" => Some(include_bytes!(
            "../../../../ToS/doctrine/semantic-interchange/query-vocabulary.v1.json"
        )),
        "access/contracts/knowledge-api.v1.json" => Some(include_bytes!(
            "../../../../access/contracts/knowledge-api.v1.json"
        )),
        "access/contracts/knowledge-graph.v1.schema.json" => Some(include_bytes!(
            "../../../../access/contracts/knowledge-graph.v1.schema.json"
        )),
        "access/contracts/knowledge-search-indexed.v2.schema.json" => Some(include_bytes!(
            "../../../../access/contracts/knowledge-search-indexed.v2.schema.json"
        )),
        "access/contracts/readable-context.v1.schema.json" => Some(include_bytes!(
            "../../../../access/contracts/readable-context.v1.schema.json"
        )),
        "access/contracts/lens-spec.v1.schema.json" => Some(include_bytes!(
            "../../../../access/contracts/lens-spec.v1.schema.json"
        )),
        "access/contracts/lens-result.v1.schema.json" => Some(include_bytes!(
            "../../../../access/contracts/lens-result.v1.schema.json"
        )),
        "access/contracts/temporal-comparison-request.v1.schema.json" => Some(include_bytes!(
            "../../../../access/contracts/temporal-comparison-request.v1.schema.json"
        )),
        "access/contracts/temporal-comparison-result.v1.schema.json" => Some(include_bytes!(
            "../../../../access/contracts/temporal-comparison-result.v1.schema.json"
        )),
        "access/contracts/source-read.v1.schema.json" => Some(include_bytes!(
            "../../../../access/contracts/source-read.v1.schema.json"
        )),
        "access/contracts/exploration-request.v1.schema.json" => Some(include_bytes!(
            "../../../../access/contracts/exploration-request.v1.schema.json"
        )),
        "access/contracts/exploration-result.v1.schema.json" => Some(include_bytes!(
            "../../../../access/contracts/exploration-result.v1.schema.json"
        )),
        "access/contracts/exploration-request.v2.schema.json" => Some(include_bytes!(
            "../../../../access/contracts/exploration-request.v2.schema.json"
        )),
        "access/contracts/exploration-result.v2.schema.json" => Some(include_bytes!(
            "../../../../access/contracts/exploration-result.v2.schema.json"
        )),
        "ToS/contracts/semantic-entity-type-registry.schema.json" => Some(include_bytes!(
            "../../../../ToS/contracts/semantic-entity-type-registry.schema.json"
        )),
        "ToS/contracts/semantic-relation-type-registry.schema.json" => Some(include_bytes!(
            "../../../../ToS/contracts/semantic-relation-type-registry.schema.json"
        )),
        _ => None,
    }
}

impl PublicCapture {
    /// Logical owned residency: inline owner, actual container capacities and
    /// owned path/string buffers. Shared cancellation belongs to the caller;
    /// borrowed views do not charge this capture again. Allocator bookkeeping
    /// and RSS remain covered by the original hard resource owner.
    pub fn retained_state_upper_bound(&self) -> Result<usize> {
        let mut bytes = std::mem::size_of::<Self>();
        let mut add = |amount: usize| -> Result<()> {
            bytes = bytes
                .checked_add(amount)
                .ok_or(Error::Budget("capture retained state overflow"))?;
            Ok(())
        };
        add(self.root.capacity())?;
        add(self.path.capacity())?;
        add(usize::try_from(self.file_state.2)
            .map_err(|_| Error::Budget("capture retained database size"))?)?;
        add(self
            .prepared_state
            .capacity()
            .checked_mul(std::mem::size_of::<(
                PathBuf,
                PathBuf,
                u64,
                u64,
                i64,
                i64,
                i64,
                i64,
            )>())
            .ok_or(Error::Budget("capture prepared state overflow"))?)?;
        for (selected, resolved, ..) in &self.prepared_state {
            add(selected.capacity())?;
            add(resolved.capacity())?;
        }
        add(self
            .sources
            .capacity()
            .checked_mul(std::mem::size_of::<SourceFile>())
            .ok_or(Error::Budget("capture source state overflow"))?)?;
        for source in &self.sources {
            add(source.label.capacity())?;
            if let SourceOrigin::File(path) = &source.origin {
                add(path.capacity())?;
            }
        }
        // These two counters are created and owned by this capture. Their Arc
        // aliases in views/receipts do not introduce another payload allocation.
        add(std::mem::size_of::<AtomicU64>()
            .checked_mul(2)
            .ok_or(Error::Budget("capture counter state overflow"))?)?;
        Ok(bytes)
    }

    pub fn create(
        root: &Path,
        staging: &Path,
        limits: PublicCaptureLimits,
        deadline: Instant,
    ) -> Result<Self> {
        Self::create_profile(
            root, staging, limits, deadline, false, false, false, None, None, None,
        )
    }

    /// Five maintained prepare input roles. Software contracts are compiled
    /// code companions; optional public-release inputs are not prepare inputs.
    pub(crate) fn create_prepared(
        root: &Path,
        staging: &Path,
        limits: PublicCaptureLimits,
        deadline: Instant,
    ) -> Result<Self> {
        Self::create_profile(
            root, staging, limits, deadline, true, false, false, None, None, None,
        )
    }
    /// Evidence Lens opens only views and canon node/edge collections.
    pub(crate) fn create_evidence(
        root: &Path,
        staging: &Path,
        limits: PublicCaptureLimits,
        deadline: Instant,
    ) -> Result<Self> {
        Self::create_profile(
            root, staging, limits, deadline, false, true, false, None, None, None,
        )
    }

    /// Evidence Lens capture using the same selected Core input paths and
    /// caller-owned operation token. Its own source and route refs remain
    /// rooted at `root`; only the selected corpus/philosophy projections are
    /// redirected by this exact constructor.
    pub(crate) fn create_evidence_selected(
        root: &Path,
        selected: &PublicCaptureInputPaths,
        staging: &Path,
        limits: PublicCaptureLimits,
        deadline: Instant,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<Self> {
        selected.validate()?;
        check_capture_active(Some(cancelled.as_ref()), deadline)?;
        Self::create_profile(
            root,
            staging,
            limits,
            deadline,
            false,
            true,
            false,
            Some(selected),
            Some(cancelled),
            None,
        )
    }
    /// Runtime projection data belongs to the selected source root; executable
    /// contract/vocabulary companions belong to this exact compiled producer.
    pub fn create_runtime(
        root: &Path,
        staging: &Path,
        limits: PublicCaptureLimits,
        deadline: Instant,
    ) -> Result<Self> {
        let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
        Self::create_runtime_selected(
            root,
            &PublicCaptureInputPaths {
                index_path: root.join("ToS/derived-exports/tos_corpus_index.min.json"),
                philosophy_graph_projection_path:
                    root.join("ToS/derived-exports/philosophy_graph_projection.min.json"),
                bibliographic_graph_path: root.join(
                    "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json",
                ),
                entity_type_registry_path: root
                    .join("ToS/doctrine/semantic-interchange/entity-types.v1.json"),
                relation_type_registry_path: root
                    .join("ToS/doctrine/semantic-interchange/relation-types.v1.json"),
                philosophy_post_planting_audit_path: root.join(
                    "ToS/philosophy/graph-workbench/review-packets/table-i-post-planting-audit.json",
                ),
                evidence_projection_path:
                    root.join("ToS/derived-exports/epistemic_evidence_projection.min.json"),
            },
            staging,
            limits,
            deadline,
            cancelled,
        )
    }

    /// Capture Reference Core's exact seven selected paths. The five graph
    /// inputs are required. The selected audit and Evidence Lens projection
    /// are represented even when absent, so existence queries cannot silently
    /// fall back to a different root-relative file.
    pub fn create_runtime_selected(
        root: &Path,
        selected: &PublicCaptureInputPaths,
        staging: &Path,
        limits: PublicCaptureLimits,
        deadline: Instant,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<Self> {
        selected.validate()?;
        check_capture_active(Some(cancelled.as_ref()), deadline)?;
        let capture = Self::create_profile(
            root,
            staging,
            limits,
            deadline,
            false,
            false,
            true,
            Some(selected),
            Some(Arc::clone(&cancelled)),
            None,
        )?;
        let total = capture
            .retained_input_members()?
            .iter()
            .try_fold(0u64, |n, (_, _, len)| {
                n.checked_add(*len)
                    .filter(|bytes| *bytes <= limits.max_input_bytes)
                    .ok_or(Error::Budget("runtime capture aggregate source bytes"))
            })?;
        if total == 0 {
            return Err(Error::Invalid("runtime capture empty input"));
        }
        check_capture_active(Some(cancelled.as_ref()), deadline)?;
        Ok(capture)
    }

    /// Capture one selected original carrier without opening other graph roles,
    /// registries, evidence files or the public ledger. The audit role reads
    /// only its exact selected optional file. The callback below supplies the
    /// same pre/post capture fence as a full completed snapshot.
    pub fn create_runtime_carrier_selected(
        root: &Path,
        selected: &PublicCaptureInputPaths,
        role: RuntimeCaptureRole,
        staging: &Path,
        limits: PublicCaptureLimits,
        deadline: Instant,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<Self> {
        selected.validate()?;
        check_capture_active(Some(cancelled.as_ref()), deadline)?;
        let capture = Self::create_profile(
            root,
            staging,
            limits,
            deadline,
            false,
            false,
            true,
            Some(selected),
            Some(Arc::clone(&cancelled)),
            Some(role),
        )?;
        let selected_label = match role {
            RuntimeCaptureRole::Corpus => "ToS/derived-exports/tos_corpus_index.min.json",
            RuntimeCaptureRole::Philosophy => {
                "ToS/derived-exports/philosophy_graph_projection.min.json"
            }
            RuntimeCaptureRole::Bibliographic => {
                "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json"
            }
            RuntimeCaptureRole::PhilosophyAudit => PHILOSOPHY_AUDIT_RELATIVE,
        };
        if !capture
            .sources
            .iter()
            .any(|source| source.label == selected_label && source.digest.is_some())
        {
            return Err(Error::Invalid("runtime carrier selected input absent"));
        }
        let total =
            capture
                .retained_input_members()?
                .iter()
                .try_fold(0u64, |total, (_, _, len)| {
                    total
                        .checked_add(*len)
                        .filter(|bytes| *bytes <= limits.max_input_bytes)
                        .ok_or(Error::Budget("runtime carrier aggregate source bytes"))
                })?;
        if total == 0 && role != RuntimeCaptureRole::PhilosophyAudit {
            return Err(Error::Invalid("runtime carrier capture empty input"));
        }
        check_capture_active(Some(cancelled.as_ref()), deadline)?;
        Ok(capture)
    }
    fn create_profile(
        root: &Path,
        staging: &Path,
        limits: PublicCaptureLimits,
        deadline: Instant,
        prepared_profile: bool,
        evidence_profile: bool,
        runtime_profile: bool,
        selected_paths: Option<&PublicCaptureInputPaths>,
        cancelled: Option<Arc<std::sync::atomic::AtomicBool>>,
        runtime_capture_role: Option<RuntimeCaptureRole>,
    ) -> Result<Self> {
        limits.validate()?;
        let cancelled =
            cancelled.unwrap_or_else(|| Arc::new(std::sync::atomic::AtomicBool::new(false)));
        let cancelled_ref = Some(cancelled.as_ref());
        let prepared_state = if prepared_profile {
            prepared_state(root)?
        } else {
            Vec::new()
        };
        check_capture_active(cancelled_ref, deadline)?;
        if staging.exists() || staging.is_symlink() {
            return Err(Error::Invalid("public D1 capture staging must be fresh"));
        }
        let mut pending = PendingCapture {
            path: staging,
            complete: false,
        };
        let mut db = Connection::open(staging)?;
        let vm_used = Arc::new(AtomicU64::new(0));
        sqlite_budget::install_progress_until(&db, limits.sqlite(), Arc::clone(&vm_used), deadline);
        db.execute_batch("PRAGMA page_size=4096; PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL; PRAGMA temp_store=FILE;")?;
        let pages = limits.max_staging_bytes / 4096;
        if pages < 16 || pages > i64::MAX as u64 {
            return Err(Error::Budget("public D1 capture page bound"));
        }
        let main_pages: i64 =
            db.query_row(&format!("PRAGMA max_page_count={pages}"), [], |row| {
                row.get(0)
            })?;
        let temp_pages: i64 =
            db.query_row(&format!("PRAGMA temp.max_page_count={pages}"), [], |row| {
                row.get(0)
            })?;
        if main_pages != pages as i64 || temp_pages != pages as i64 {
            return Err(Error::Budget("public D1 capture page admission"));
        }
        db.execute_batch("CREATE TABLE capture_rows(role TEXT NOT NULL,collection TEXT NOT NULL,source_key TEXT NOT NULL,ord INTEGER NOT NULL,sort0 TEXT NOT NULL,sort1 TEXT NOT NULL,json BLOB NOT NULL,sha256 BLOB NOT NULL,PRIMARY KEY(role,collection,source_key)) WITHOUT ROWID; CREATE INDEX capture_rows_order ON capture_rows(role,collection,sort0,sort1,source_key); CREATE TABLE capture_collections(role TEXT NOT NULL,collection TEXT NOT NULL,kind TEXT NOT NULL,PRIMARY KEY(role,collection)) WITHOUT ROWID; CREATE TABLE capture_headers(role TEXT NOT NULL,path TEXT NOT NULL,json BLOB NOT NULL,PRIMARY KEY(role,path)) WITHOUT ROWID; CREATE TABLE capture_sources(path TEXT PRIMARY KEY,sha256 BLOB NOT NULL,size_bytes INTEGER NOT NULL) WITHOUT ROWID;")?;
        let mut rows = 0u64;
        let mut work_bytes = 0u64;
        let mut sources = Vec::new();
        let mut corpus_partitioned = None;
        let mut claims_partitioned = None;
        for (role, relative) in [
            (CORPUS, "ToS/derived-exports/tos_corpus_index.min.json"),
            (
                PHILOSOPHY,
                "ToS/derived-exports/philosophy_graph_projection.min.json",
            ),
            (
                CLAIMS,
                "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json",
            ),
        ] {
            if runtime_capture_role.is_some_and(|selected_role| {
                !matches!(
                    (selected_role, role),
                    (RuntimeCaptureRole::Corpus, CORPUS)
                        | (RuntimeCaptureRole::Philosophy, PHILOSOPHY)
                        | (RuntimeCaptureRole::Bibliographic, CLAIMS)
                )
            }) {
                continue;
            }
            if evidence_profile && role == CLAIMS {
                continue;
            }
            let role = if evidence_profile {
                if role == CORPUS {
                    "evidence-corpus"
                } else {
                    "evidence-philosophy"
                }
            } else {
                role
            };
            let path = selected_paths
                .and_then(|selected| selected.core_path(relative))
                .map(Path::to_owned)
                .unwrap_or_else(|| root.join(relative));
            let mut file = profile_open(&path, limits.max_input_bytes, prepared_profile)?;
            let (digest, len) = source_digest(&mut file, limits.max_input_bytes, |n| {
                check_capture_active(cancelled_ref, deadline)?;
                checked_add(&mut work_bytes, n, limits.max_work_bytes)
            })?;
            check_capture_active(cancelled_ref, deadline)?;
            db.execute_batch("BEGIN IMMEDIATE")?;
            let mut writer = CaptureWriter {
                db: &db,
                role,
                row_count: &mut rows,
                work: &mut work_bytes,
                limits,
                ordinals: BTreeMap::new(),
                read_budget: Rc::new(Cell::new(MAX_HEADER_BYTES + 65536)),
                deadline,
                cancelled: cancelled_ref,
                dynamic_philosophy: role == PHILOSOPHY
                    && runtime_capture_role == Some(RuntimeCaptureRole::Philosophy),
                page_rows: 0,
                page_bytes: 0,
            };
            let mut partitioned = false;
            if len <= MAX_ROOT_BYTES {
                let mut raw = Vec::with_capacity(len as usize);
                (&mut file).take(MAX_ROOT_BYTES + 1).read_to_end(&mut raw)?;
                if raw.len() as u64 != len {
                    return Err(Error::Invalid("public D1 source changed during capture"));
                }
                file.seek(SeekFrom::Start(0))?;
                check_capture_active(cancelled_ref, deadline)?;
                let first = strict_value(&raw, MAX_ROOT_BYTES as usize)?;
                check_capture_active(cancelled_ref, deadline)?;
                if first
                    .get("schema_version")
                    .and_then(serde_json::Value::as_str)
                    == Some("tos_partitioned_projection_v1")
                {
                    capture_partitioned(&path, &raw, &mut writer)?;
                    partitioned = true;
                }
            }
            if !partitioned {
                let buffered = BufReader::with_capacity(64 * 1024, &mut file);
                let bounded = CaptureReader {
                    inner: buffered,
                    remaining: Rc::clone(&writer.read_budget),
                    deadline,
                    cancelled: cancelled_ref,
                };
                let mut deserializer = serde_json::Deserializer::from_reader(bounded);
                ObjectSeed {
                    writer: &mut writer,
                    prefix: "",
                }
                .deserialize(&mut deserializer)
                .map_err(|e| Error::Source(e.to_string()))?;
                writer.read_budget.set(MAX_HEADER_BYTES + 65536);
                deserializer
                    .end()
                    .map_err(|e| Error::Source(e.to_string()))?;
            }
            let stored_schema: Option<Vec<u8>> = db
                .query_row(
                    "SELECT json FROM capture_headers WHERE role=?1 AND path='schema_version'",
                    [role],
                    |row| row.get(0),
                )
                .optional()?;
            let expected_schema = match role {
                CORPUS | "evidence-corpus" => "tos_corpus_index_v1",
                PHILOSOPHY | "evidence-philosophy" => "tos_philosophy_graph_projection_v2",
                CLAIMS => "tos_source_witness_bibliographic_graph_v1",
                _ => return Err(Error::Invalid("public D1 source role")),
            };
            let schema = stored_schema
                .as_deref()
                .map(|raw| json(raw, 4096))
                .transpose()?;
            if runtime_capture_role == Some(RuntimeCaptureRole::Philosophy) {
                if !matches!(
                    schema.as_ref().and_then(JsonValue::as_str),
                    Some(
                        "tos_philosophy_graph_projection_v1" | "tos_philosophy_graph_projection_v2"
                    )
                ) {
                    return Err(Error::Invalid("runtime philosophy source schema"));
                }
            } else if prepared_profile {
                if role == PHILOSOPHY
                    && !matches!(
                        schema.as_ref().and_then(JsonValue::as_str),
                        Some(
                            "tos_philosophy_graph_projection_v1"
                                | "tos_philosophy_graph_projection_v2"
                        )
                    )
                {
                    return Err(Error::Invalid("prepared philosophy source schema"));
                }
            } else if schema.as_ref().and_then(JsonValue::as_str) != Some(expected_schema) {
                return Err(Error::Invalid("public D1 source schema"));
            }
            if matches!(role, CORPUS | "evidence-corpus") {
                corpus_partitioned = Some(partitioned);
            }
            if role == CLAIMS {
                claims_partitioned = Some(partitioned);
            }
            drop(writer);
            db.execute_batch("COMMIT")?;
            check_capture_active(cancelled_ref, deadline)?;
            sources.push(SourceFile {
                label: relative.to_owned(),
                origin: SourceOrigin::File(path),
                digest: Some(digest),
                len,
            });
        }
        if runtime_capture_role == Some(RuntimeCaptureRole::PhilosophyAudit) {
            let path = selected_paths
                .and_then(|selected| selected.optional_path(PHILOSOPHY_AUDIT_RELATIVE))
                .map(Path::to_owned)
                .ok_or(Error::Invalid("selected philosophy audit path absent"))?;
            let mut file = profile_open(&path, limits.max_input_bytes, prepared_profile)?;
            let (digest, len) = source_digest(&mut file, limits.max_input_bytes, |n| {
                check_capture_active(cancelled_ref, deadline)?;
                checked_add(&mut work_bytes, n, limits.max_work_bytes)
            })?;
            check_capture_active(cancelled_ref, deadline)?;
            sources.push(SourceFile {
                label: PHILOSOPHY_AUDIT_RELATIVE.to_owned(),
                origin: SourceOrigin::File(path),
                digest: Some(digest),
                len,
            });
        }
        if !prepared_profile
            && !evidence_profile
            && runtime_capture_role.is_none()
            && corpus_partitioned != claims_partitioned
        {
            return Err(Error::Invalid("public D1 coupled projection storage mode"));
        }
        for relative in [
            "ToS/doctrine/semantic-interchange/entity-types.v1.json",
            "ToS/doctrine/semantic-interchange/relation-types.v1.json",
            "ToS/doctrine/semantic-interchange/query-vocabulary.v1.json",
            "access/contracts/knowledge-api.v1.json",
            "access/contracts/knowledge-graph.v1.schema.json",
            "access/contracts/knowledge-search-indexed.v2.schema.json",
            "access/contracts/readable-context.v1.schema.json",
            "access/contracts/lens-spec.v1.schema.json",
            "access/contracts/lens-result.v1.schema.json",
            "access/contracts/temporal-comparison-request.v1.schema.json",
            "access/contracts/temporal-comparison-result.v1.schema.json",
            "access/contracts/source-read.v1.schema.json",
            "access/contracts/exploration-request.v1.schema.json",
            "access/contracts/exploration-result.v1.schema.json",
            "access/contracts/exploration-request.v2.schema.json",
            "access/contracts/exploration-result.v2.schema.json",
            "ToS/contracts/semantic-entity-type-registry.schema.json",
            "ToS/contracts/semantic-relation-type-registry.schema.json",
        ] {
            if evidence_profile || runtime_capture_role.is_some() {
                continue;
            }
            if prepared_profile
                && !matches!(
                    relative,
                    "ToS/doctrine/semantic-interchange/entity-types.v1.json"
                        | "ToS/doctrine/semantic-interchange/relation-types.v1.json"
                )
            {
                continue;
            }
            if runtime_profile {
                if let Some(raw) = runtime_companion(relative) {
                    if raw.len() as u64 > limits.max_input_bytes {
                        return Err(Error::Budget("runtime compiled companion bytes"));
                    }
                    checked_add(&mut work_bytes, raw.len(), limits.max_work_bytes)?;
                    sources.push(SourceFile {
                        label: relative.to_owned(),
                        origin: SourceOrigin::Compiled(raw),
                        digest: Some(Digest256::of_bytes(raw)),
                        len: raw.len() as u64,
                    });
                    continue;
                }
            }
            let path = selected_paths
                .and_then(|selected| selected.core_path(relative))
                .map(Path::to_owned)
                .unwrap_or_else(|| root.join(relative));
            let mut file = profile_open(&path, limits.max_input_bytes, prepared_profile)?;
            let (digest, len) = source_digest(&mut file, limits.max_input_bytes, |n| {
                check_capture_active(cancelled_ref, deadline)?;
                checked_add(&mut work_bytes, n, limits.max_work_bytes)
            })?;
            check_capture_active(cancelled_ref, deadline)?;
            sources.push(SourceFile {
                label: relative.to_owned(),
                origin: SourceOrigin::File(path),
                digest: Some(digest),
                len,
            });
        }
        if !prepared_profile && !evidence_profile && runtime_capture_role.is_none() {
            for relative in [
                "ToS/derived-exports/epistemic_evidence_projection.min.json",
                "ToS/philosophy/graph-workbench/review-packets/table-i-post-planting-audit.json",
            ] {
                let path = selected_paths
                    .and_then(|selected| selected.optional_path(relative))
                    .map(Path::to_owned)
                    .unwrap_or_else(|| root.join(relative));
                if path.exists() || path.is_symlink() {
                    let mut file = profile_open(&path, limits.max_input_bytes, prepared_profile)?;
                    let (digest, len) = source_digest(&mut file, limits.max_input_bytes, |n| {
                        check_capture_active(cancelled_ref, deadline)?;
                        checked_add(&mut work_bytes, n, limits.max_work_bytes)
                    })?;
                    check_capture_active(cancelled_ref, deadline)?;
                    sources.push(SourceFile {
                        label: relative.to_owned(),
                        origin: SourceOrigin::File(path),
                        digest: Some(digest),
                        len,
                    });
                } else {
                    sources.push(SourceFile {
                        label: relative.to_owned(),
                        origin: SourceOrigin::File(path),
                        digest: None,
                        len: 0,
                    });
                }
            }
            let ledger_relative = "ToS/source-witnesses/access-requests/public-ledger";
            let ledger = root.join(ledger_relative);
            if ledger.exists() {
                if ledger.is_symlink() || !ledger.is_dir() {
                    return Err(Error::Invalid("public D1 ledger directory"));
                }
                let mut names = Vec::new();
                for entry in std::fs::read_dir(&ledger)? {
                    if names.len() == 4096 {
                        return Err(Error::Budget("public D1 ledger membership"));
                    }
                    let name = entry?.file_name();
                    checked_add(
                        &mut work_bytes,
                        name.as_encoded_bytes().len(),
                        limits.max_work_bytes,
                    )?;
                    names.push(name);
                }
                names.sort();
                for name in names {
                    let name = name
                        .to_str()
                        .ok_or(Error::Invalid("public D1 ledger filename"))?;
                    if !name.ends_with(".access-request.json") {
                        continue;
                    }
                    let path = ledger.join(name);
                    let mut file = safe_open::open_regular(&path, 256_000)?;
                    let (digest, len) = source_digest(&mut file, 256_000, |n| {
                        check_capture_active(cancelled_ref, deadline)?;
                        checked_add(&mut work_bytes, n, limits.max_work_bytes)
                    })?;
                    check_capture_active(cancelled_ref, deadline)?;
                    sources.push(SourceFile {
                        label: format!("{ledger_relative}/{name}"),
                        origin: SourceOrigin::File(path),
                        digest: Some(digest),
                        len,
                    });
                }
            }
        }
        if std::fs::metadata(staging)?.len() > limits.max_staging_bytes {
            return Err(Error::Budget("public D1 capture physical bytes"));
        }
        check_capture_active(cancelled_ref, deadline)?;
        drop(db);
        let metadata = fs::symlink_metadata(staging)?;
        if !metadata.file_type().is_file() {
            return Err(Error::Invalid("public D1 capture inode"));
        }
        pending.complete = true;
        Ok(Self {
            root: root.to_owned(),
            prepared_profile,
            evidence_profile,
            runtime_capture_role,
            prepared_state,
            path: staging.to_owned(),
            inode: (metadata.dev(), metadata.ino()),
            file_state: (
                metadata.dev(),
                metadata.ino(),
                metadata.len(),
                metadata.mtime(),
                metadata.mtime_nsec(),
                metadata.ctime(),
                metadata.ctime_nsec(),
            ),
            sources,
            partitioned: corpus_partitioned == Some(true)
                || (prepared_profile && claims_partitioned == Some(true)),
            rows,
            work_bytes: Arc::new(AtomicU64::new(work_bytes)),
            max_work_bytes: limits.max_work_bytes,
            deadline,
            limits,
            vm_used,
            cancelled,
        })
    }

    pub fn charge_work(&self, bytes: u64) -> Result<()> {
        check_capture_active(Some(self.cancelled.as_ref()), self.deadline)?;
        if Instant::now() >= self.deadline {
            return Err(Error::Budget("public D1 build deadline"));
        }
        let mut current = self.work_bytes.load(std::sync::atomic::Ordering::Acquire);
        loop {
            let next = current
                .checked_add(bytes)
                .filter(|value| *value <= self.max_work_bytes)
                .ok_or(Error::Budget("public D1 build work bytes"))?;
            match self.work_bytes.compare_exchange_weak(
                current,
                next,
                std::sync::atomic::Ordering::AcqRel,
                std::sync::atomic::Ordering::Acquire,
            ) {
                Ok(_) => return Ok(()),
                Err(observed) => current = observed,
            }
            check_capture_active(Some(self.cancelled.as_ref()), self.deadline)?;
        }
    }

    /// Lend the exact selected original carrier under this capture's pre/post
    /// source fence. A partial-role capture intentionally has no whole Core
    /// `source_revision`; the view reports that absence explicitly.
    pub fn with_captured_carriers<'a, T>(
        &'a self,
        consume: impl for<'view> FnOnce(
            &'view crate::native_snapshot::CompletedCaptureCarriers<'a>,
        ) -> Result<T>,
    ) -> Result<T> {
        if self.runtime_capture_role.is_none() {
            return Err(Error::Invalid("selected carrier profile required"));
        }
        self.verify_captured_inputs()?;
        let view = crate::native_snapshot::CompletedCaptureCarriers::from_selected_capture(self);
        let result = consume(&view);
        let current = self.verify_captured_inputs();
        match current {
            Err(error) => Err(error),
            Ok(()) => result,
        }
    }

    /// Borrow only the present audit member already owned by this exact capture.
    /// The path is provenance metadata; bytes still pass through `read_input`.
    pub(crate) fn selected_philosophy_audit_path(&self) -> Result<&Path> {
        self.check_custody()?;
        let source = self
            .sources
            .iter()
            .find(|source| source.label == PHILOSOPHY_AUDIT_RELATIVE)
            .filter(|source| source.digest.is_some())
            .ok_or(Error::Invalid("selected philosophy audit input absent"))?;
        match &source.origin {
            SourceOrigin::File(path) => Ok(path),
            SourceOrigin::Compiled(_) => {
                Err(Error::Invalid("selected philosophy audit file required"))
            }
        }
    }

    pub(crate) fn read_selected_philosophy_audit(&self, cap: usize) -> Result<Vec<u8>> {
        self.selected_philosophy_audit_path()?;
        self.read_input(PHILOSOPHY_AUDIT_RELATIVE, cap)?
            .ok_or(Error::Invalid("selected philosophy audit input absent"))
    }

    pub fn work_bytes(&self) -> u64 {
        self.work_bytes.load(std::sync::atomic::Ordering::Acquire)
    }

    pub(crate) fn vm_counter(&self) -> Arc<AtomicU64> {
        Arc::clone(&self.vm_used)
    }

    pub(crate) fn deadline(&self) -> Instant {
        self.deadline
    }

    pub(crate) fn work_counter(&self) -> Arc<AtomicU64> {
        Arc::clone(&self.work_bytes)
    }

    pub(crate) fn max_work_bytes(&self) -> u64 {
        self.max_work_bytes
    }

    pub(crate) fn cancellation(&self) -> &std::sync::atomic::AtomicBool {
        self.cancelled.as_ref()
    }

    pub(crate) fn cancellation_handle(&self) -> Arc<std::sync::atomic::AtomicBool> {
        Arc::clone(&self.cancelled)
    }

    pub(crate) fn runtime_input_paths(&self) -> Result<PublicCaptureInputPaths> {
        fn selected_path(capture: &PublicCapture, label: &str) -> Result<PathBuf> {
            capture
                .sources
                .iter()
                .find(|source| source.label == label)
                .and_then(|source| match &source.origin {
                    SourceOrigin::File(path) => Some(path.clone()),
                    SourceOrigin::Compiled(_) => None,
                })
                .ok_or(Error::Invalid("runtime selected input path absent"))
        }
        Ok(PublicCaptureInputPaths {
            index_path: selected_path(self, "ToS/derived-exports/tos_corpus_index.min.json")?,
            philosophy_graph_projection_path: selected_path(
                self,
                "ToS/derived-exports/philosophy_graph_projection.min.json",
            )?,
            bibliographic_graph_path: selected_path(
                self,
                "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json",
            )?,
            entity_type_registry_path: selected_path(
                self,
                "ToS/doctrine/semantic-interchange/entity-types.v1.json",
            )?,
            relation_type_registry_path: selected_path(
                self,
                "ToS/doctrine/semantic-interchange/relation-types.v1.json",
            )?,
            philosophy_post_planting_audit_path: selected_path(
                self,
                "ToS/philosophy/graph-workbench/review-packets/table-i-post-planting-audit.json",
            )?,
            evidence_projection_path: selected_path(
                self,
                "ToS/derived-exports/epistemic_evidence_projection.min.json",
            )?,
        })
    }

    pub fn check_custody(&self) -> Result<()> {
        check_capture_active(Some(self.cancelled.as_ref()), self.deadline)?;
        let metadata = fs::symlink_metadata(&self.path)?;
        if !metadata.file_type().is_file() || (metadata.dev(), metadata.ino()) != self.inode {
            return Err(Error::Invalid("public D1 private capture replaced"));
        }
        self.charge_work(0)
    }

    pub(crate) fn capture_identity(&self) -> Result<(u64, u64, u64, i64, i64, i64, i64)> {
        self.check_custody()?;
        let metadata = fs::symlink_metadata(&self.path)?;
        let state = (
            metadata.dev(),
            metadata.ino(),
            metadata.len(),
            metadata.mtime(),
            metadata.mtime_nsec(),
            metadata.ctime(),
            metadata.ctime_nsec(),
        );
        if !metadata.file_type().is_file() || state != self.file_state {
            return Err(Error::Invalid("public D1 private capture changed"));
        }
        Ok(state)
    }

    pub(crate) fn read_db(&self) -> Result<Connection> {
        self.check_custody()?;
        let db = Connection::open_with_flags(&self.path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        sqlite_budget::install_progress_until(
            &db,
            self.limits.sqlite(),
            Arc::clone(&self.vm_used),
            self.deadline,
        );
        db.pragma_update(None, "cache_size", -(self.limits.sqlite_cache_kib as i64))?;
        Ok(db)
    }

    pub(crate) fn write_db(&self) -> Result<Connection> {
        self.check_custody()?;
        let db = Connection::open(&self.path)?;
        sqlite_budget::install_progress_until(
            &db,
            self.limits.sqlite(),
            Arc::clone(&self.vm_used),
            self.deadline,
        );
        db.pragma_update(None, "cache_size", -(self.limits.sqlite_cache_kib as i64))?;
        db.execute_batch(
            "PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL; PRAGMA temp_store=FILE",
        )?;
        Ok(db)
    }

    pub fn verify_inputs(&self, limits: PublicCaptureLimits) -> Result<()> {
        check_capture_active(Some(self.cancelled.as_ref()), self.deadline)?;
        self.check_custody()?;
        if self.prepared_profile && prepared_state(&self.root)? != self.prepared_state {
            return Err(Error::Invalid("prepared source state changed"));
        }
        if limits.max_work_bytes != self.max_work_bytes {
            return Err(Error::Invalid("public D1 changed work budget"));
        }
        for source in &self.sources {
            let SourceOrigin::File(path) = &source.origin else {
                continue;
            };
            if source.digest.is_none() {
                if path.exists() || path.is_symlink() {
                    return Err(Error::Invalid(
                        "public D1 optional source appeared during build",
                    ));
                }
                continue;
            }
            let mut file = profile_open(path, limits.max_input_bytes, self.prepared_profile)?;
            let (digest, len) = source_digest(&mut file, limits.max_input_bytes, |n| {
                check_capture_active(Some(self.cancelled.as_ref()), self.deadline)?;
                self.charge_work(n as u64)
            })?;
            if len != source.len || Some(digest) != source.digest {
                return Err(Error::Invalid("public D1 source changed during build"));
            }
        }
        if !self.prepared_profile && !self.evidence_profile && self.runtime_capture_role.is_none() {
            let ledger = self
                .root
                .join("ToS/source-witnesses/access-requests/public-ledger");
            let mut current = BTreeSet::new();
            if ledger.exists() {
                if ledger.is_symlink() || !ledger.is_dir() {
                    return Err(Error::Invalid("public D1 ledger changed"));
                }
                for entry in std::fs::read_dir(&ledger)? {
                    check_capture_active(Some(self.cancelled.as_ref()), self.deadline)?;
                    if current.len() >= 4096 {
                        return Err(Error::Budget("public D1 ledger membership"));
                    }
                    let entry = entry?;
                    let name = entry.file_name();
                    self.charge_work(name.as_encoded_bytes().len() as u64)?;
                    let name = name
                        .to_str()
                        .ok_or(Error::Invalid("public D1 ledger filename"))?;
                    if name.ends_with(".access-request.json") {
                        current.insert(name.to_owned());
                    }
                }
            }
            let captured = self
                .sources
                .iter()
                .filter_map(|source| {
                    source
                        .label
                        .strip_prefix("ToS/source-witnesses/access-requests/public-ledger/")
                })
                .map(str::to_owned)
                .collect::<BTreeSet<_>>();
            if current != captured {
                return Err(Error::Invalid("public D1 ledger membership changed"));
            }
        }
        let db = self.read_db()?;
        let mut statement =
            db.prepare("SELECT path,sha256,size_bytes FROM capture_sources ORDER BY path")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let path: String = row.get(0)?;
            let digest: Vec<u8> = row.get(1)?;
            let len: u64 = row.get(2)?;
            let mut file = safe_open::open_regular(Path::new(&path), limits.max_input_bytes)?;
            let (actual, actual_len) = source_digest(&mut file, limits.max_input_bytes, |n| {
                check_capture_active(Some(self.cancelled.as_ref()), self.deadline)?;
                self.charge_work(n as u64)
            })?;
            if actual_len != len || actual.as_bytes().as_slice() != digest.as_slice() {
                return Err(Error::Invalid("public D1 part changed during build"));
            }
        }
        check_capture_active(Some(self.cancelled.as_ref()), self.deadline)
    }

    pub(crate) fn verify_captured_inputs(&self) -> Result<()> {
        self.verify_inputs(self.limits)
    }

    /// Read only an exact member already selected by this retained capture,
    /// including partition members. No ambient relative-path read is admitted.
    pub fn read_retained_input(&self, label: &str, cap: usize) -> Result<Vec<u8>> {
        self.check_custody()?;
        if self
            .sources
            .iter()
            .any(|source| source.label == label && source.digest.is_some())
        {
            return self
                .read_input(label, cap)?
                .ok_or(Error::Invalid("captured runtime member absent"));
        }
        let selected = Path::new(label);
        let path = if selected.is_absolute() {
            selected.to_owned()
        } else {
            let relative = tos_foundation::RelativePath::parse(label)
                .map_err(|_| Error::Invalid("captured runtime member path"))?;
            self.root.join(relative.as_str())
        };
        let db = self.read_db()?;
        let part: Option<(Vec<u8>, u64)> = db
            .query_row(
                "SELECT sha256,size_bytes FROM capture_sources WHERE path=?1",
                [path
                    .to_str()
                    .ok_or(Error::Invalid("captured runtime path UTF8"))?],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let (expected, len) = part.ok_or(Error::Invalid("captured runtime member absent"))?;
        if len > cap as u64 {
            return Err(Error::Budget("captured runtime member bytes"));
        }
        self.charge_work(len)?;
        let file = safe_open::open_regular(&path, cap as u64)?;
        let mut raw = Vec::with_capacity(len as usize);
        file.take(cap as u64 + 1).read_to_end(&mut raw)?;
        if raw.len() as u64 != len
            || Digest256::of_bytes(&raw).as_bytes().as_slice() != expected.as_slice()
        {
            return Err(Error::Invalid("captured runtime part changed"));
        }
        Ok(raw)
    }

    /// Exact retained closure from the same capture, including authenticated
    /// partition members and typed compiled companions. Optional absence stays
    /// absent; no source tree enumeration is used.
    pub fn retained_input_members(&self) -> Result<Vec<(String, Digest256, u64)>> {
        self.check_custody()?;
        let mut members = BTreeMap::new();
        for source in &self.sources {
            if let Some(digest) = source.digest {
                members.insert(source.label.clone(), (digest, source.len));
            }
        }
        let db = self.read_db()?;
        let mut statement =
            db.prepare("SELECT path,sha256,size_bytes FROM capture_sources ORDER BY path")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            if members.len() >= 65536 {
                return Err(Error::Budget("captured runtime member count"));
            }
            let path: String = row.get(0)?;
            let raw_sha: Vec<u8> = row.get(1)?;
            let len: u64 = row.get(2)?;
            self.charge_work(path.len() as u64 + 40)?;
            let member_path = Path::new(&path);
            let relative = member_path
                .strip_prefix(&self.root)
                .unwrap_or(member_path)
                .to_str()
                .ok_or(Error::Invalid("captured runtime member UTF8"))?
                .to_owned();
            let bytes: [u8; 32] = raw_sha
                .try_into()
                .map_err(|_| Error::Invalid("captured runtime member digest"))?;
            let sha = Digest256::from_bytes(bytes);
            if let Some(previous) = members.insert(relative, (sha, len)) {
                if previous != (sha, len) {
                    return Err(Error::Invalid("captured runtime duplicate member differs"));
                }
            }
        }
        Ok(members
            .into_iter()
            .map(|(path, (sha, len))| (path, sha, len))
            .collect())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Capture the exact five-file physical source-state tuple used by the
    /// public Core graph API. The digest fence is checked on both sides of
    /// stat collection so this tuple cannot describe a different input cut.
    pub fn core_source_state(&self) -> Result<Vec<(String, i64, u64, u64, i64)>> {
        if self.runtime_capture_role.is_some() {
            return Err(Error::Invalid(
                "whole Core source state requires all five inputs",
            ));
        }
        fn collect(capture: &PublicCapture) -> Result<Vec<(String, i64, u64, u64, i64)>> {
            let mut states = Vec::with_capacity(5);
            for label in [
                "ToS/derived-exports/tos_corpus_index.min.json",
                "ToS/derived-exports/philosophy_graph_projection.min.json",
                "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json",
                "ToS/doctrine/semantic-interchange/entity-types.v1.json",
                "ToS/doctrine/semantic-interchange/relation-types.v1.json",
            ] {
                let source = capture
                    .sources
                    .iter()
                    .find(|source| source.label == label && source.digest.is_some())
                    .ok_or(Error::Invalid("public D1 core source-state member"))?;
                let SourceOrigin::File(path) = &source.origin else {
                    return Err(Error::Invalid("public D1 core source-state origin"));
                };
                let resolved = fs::canonicalize(path)?;
                let metadata = fs::metadata(path)?;
                let mtime_ns = metadata
                    .mtime()
                    .checked_mul(1_000_000_000)
                    .and_then(|value| value.checked_add(metadata.mtime_nsec()))
                    .ok_or(Error::Budget("public D1 source mtime"))?;
                let ctime_ns = metadata
                    .ctime()
                    .checked_mul(1_000_000_000)
                    .and_then(|value| value.checked_add(metadata.ctime_nsec()))
                    .ok_or(Error::Budget("public D1 source ctime"))?;
                if metadata.len() != source.len {
                    return Err(Error::Invalid("public D1 source-state size changed"));
                }
                states.push((
                    resolved
                        .to_str()
                        .ok_or(Error::Invalid("public D1 source-state path UTF8"))?
                        .to_owned(),
                    mtime_ns,
                    metadata.len(),
                    metadata.ino(),
                    ctime_ns,
                ));
            }
            Ok(states)
        }
        let before = collect(self)?;
        self.verify_inputs(self.limits)?;
        let after = collect(self)?;
        if before != after {
            return Err(Error::Invalid("public D1 core source state changed"));
        }
        Ok(after)
    }

    /// Physical identity of the complete selected capture closure, including
    /// partition members which do not appear in Reference Core's five-path
    /// public `source_state` tuple. The returned order is canonical absolute
    /// path order; a source edit may change metadata while preserving this
    /// path set, but adding/removing a captured member changes the vector.
    pub fn capture_source_state(&self) -> Result<Vec<(String, i64, u64, u64, i64)>> {
        self.verify_inputs(self.limits)?;
        let mut paths = BTreeSet::new();
        for source in &self.sources {
            if source.digest.is_some()
                && let SourceOrigin::File(path) = &source.origin
            {
                paths.insert(path.clone());
            }
        }
        let db = self.read_db()?;
        let mut statement = db.prepare("SELECT path FROM capture_sources ORDER BY path")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let path: String = row.get(0)?;
            self.charge_work(path.len() as u64)?;
            paths.insert(PathBuf::from(path));
        }
        let mut state = Vec::with_capacity(paths.len());
        for path in paths {
            let resolved = fs::canonicalize(&path)?;
            let metadata = fs::metadata(&path)?;
            let mtime_ns = metadata
                .mtime()
                .checked_mul(1_000_000_000)
                .and_then(|value| value.checked_add(metadata.mtime_nsec()))
                .ok_or(Error::Budget("public D1 capture source mtime"))?;
            let ctime_ns = metadata
                .ctime()
                .checked_mul(1_000_000_000)
                .and_then(|value| value.checked_add(metadata.ctime_nsec()))
                .ok_or(Error::Budget("public D1 capture source ctime"))?;
            state.push((
                resolved
                    .to_str()
                    .ok_or(Error::Invalid("public D1 capture source path UTF8"))?
                    .to_owned(),
                mtime_ns,
                metadata.len(),
                metadata.ino(),
                ctime_ns,
            ));
        }
        state.sort_by(|left, right| left.0.cmp(&right.0));
        self.verify_inputs(self.limits)?;
        Ok(state)
    }

    pub fn source_digest(&self, label: &str) -> Result<Digest256> {
        self.sources
            .iter()
            .find(|source| source.label == label)
            .and_then(|source| source.digest)
            .ok_or(Error::Invalid("public D1 source digest absent"))
    }

    pub(crate) fn source_labels(&self) -> Vec<&str> {
        self.sources
            .iter()
            .filter(|source| source.digest.is_some())
            .map(|source| source.label.as_str())
            .collect()
    }

    pub(crate) fn public_ledger_labels(&self) -> Vec<&str> {
        self.sources
            .iter()
            .filter(|source| {
                source.digest.is_some()
                    && source
                        .label
                        .starts_with("ToS/source-witnesses/access-requests/public-ledger/")
                    && source.label.ends_with(".access-request.json")
            })
            .map(|source| source.label.as_str())
            .collect()
    }

    /// Measured public inputs for the existing completion manifest. Part rows
    /// were admitted by the source-owned partition descriptors during capture;
    /// only their bounded root is exported, never private absolute paths.
    /// Verify the existing public D1 completion pair against this captured
    /// source. This grants no selected reader authority and creates no output.
    /// Call before and after the local comparison using the same capture.
    pub fn verify_public_completion(
        &self,
        runtime: &Path,
        dist: &Path,
        limits: PublicCaptureLimits,
        manifest_bytes: usize,
    ) -> Result<(serde_json::Value, Vec<u8>)> {
        use std::io::Read;
        self.verify_inputs(limits)?;
        if manifest_bytes == 0 || manifest_bytes > MAX_HEADER_BYTES + 1 {
            return Err(Error::Budget("public D1 verifier manifest bytes"));
        }
        let read_marker = |path: &Path| -> Result<Vec<u8>> {
            let mut file = tos_fd_open::open_absolute_regular(path, manifest_bytes as u64)
                .map_err(|_| Error::Invalid("public D1 completion marker"))?;
            let size = file.metadata()?.len();
            if size == 0 || size > manifest_bytes as u64 {
                return Err(Error::Budget("public D1 completion marker bytes"));
            }
            self.charge_work(
                size.checked_mul(3)
                    .ok_or(Error::Budget("public D1 verifier work"))?,
            )?;
            let mut raw = Vec::with_capacity(size as usize);
            std::io::Read::by_ref(&mut file)
                .take(size + 1)
                .read_to_end(&mut raw)?;
            if raw.len() as u64 != size {
                return Err(Error::Invalid("public D1 completion marker changed"));
            }
            Ok(raw)
        };
        let raw = read_marker(&runtime.join("manifest.json"))?;
        if raw != read_marker(&dist.join("__edge/build-manifest.json"))? {
            return Err(Error::Invalid("public D1 completion markers differ"));
        }
        let manifest: serde_json::Value =
            serde_json::from_slice(&raw).map_err(|e| Error::Source(e.to_string()))?;
        let revision = manifest["data_revision"]
            .as_str()
            .filter(|value| {
                value.len() == 64
                    && value
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            })
            .ok_or(Error::Invalid("public D1 completion revision"))?;
        if manifest["schema"] != "tos_cloudflare_edge_build_v1"
            || manifest["read_model_schema"] != "tos_cloudflare_edge_read_model_v9"
        {
            return Err(Error::Invalid("public D1 completion schema"));
        }
        for (name, field) in [
            ("read-model.sql", "sql_bytes"),
            ("read-model.rows.json", "baseline_bytes"),
        ] {
            let size = manifest[field]
                .as_u64()
                .filter(|n| *n > 0)
                .ok_or(Error::Invalid("public D1 completed file size"))?;
            let file = tos_fd_open::open_absolute_regular(&runtime.join(name), size)
                .map_err(|_| Error::Invalid("public D1 completed file"))?;
            if file.metadata()?.len() != size {
                return Err(Error::Invalid("public D1 completed file size"));
            }
            if name == "read-model.rows.json" {
                let mut prefix = Vec::with_capacity(160);
                (&file).take(160).read_to_end(&mut prefix)?;
                let count = prefix.len();
                let expected = format!(
                    "{{\"schema\":\"tos_cloudflare_edge_read_model_v9\",\"revision\":\"{revision}\""
                );
                if !prefix.starts_with(expected.as_bytes()) {
                    return Err(Error::Invalid("public D1 baseline revision"));
                }
                self.charge_work(count as u64)?;
            }
        }
        if manifest["public_input_binding"] != self.manifest_input_binding(manifest_bytes)? {
            return Err(Error::Invalid("public D1 measured input binding differs"));
        }
        self.check_custody()?;
        Ok((manifest, raw))
    }

    pub(crate) fn manifest_input_binding(&self, max_bytes: usize) -> Result<serde_json::Value> {
        #[derive(serde::Serialize)]
        struct Entry<'a> {
            path: &'a str,
            size_bytes: Option<u64>,
            sha256: Option<String>,
        }
        struct Count {
            len: usize,
            max: usize,
            exceeded: bool,
        }
        impl Write for Count {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                let Some(next) = self.len.checked_add(bytes.len()).filter(|n| *n <= self.max)
                else {
                    self.exceeded = true;
                    return Err(io::Error::other("public D1 manifest input binding bytes"));
                };
                self.len = next;
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        self.check_custody()?;
        if max_bytes == 0 || max_bytes > MAX_HEADER_BYTES {
            return Err(Error::Budget("public D1 manifest input binding bytes"));
        }
        const PREFIX: &[u8] = b"{\"sources\":[";
        let pointer_bytes = self
            .sources
            .len()
            .checked_mul(std::mem::size_of::<&SourceFile>())
            .ok_or(Error::Budget("public D1 manifest input state"))?;
        if PREFIX
            .len()
            .checked_add(self.sources.len().saturating_mul(4))
            .is_none_or(|minimum| minimum > max_bytes)
        {
            return Err(Error::Budget("public D1 manifest input binding bytes"));
        }
        self.charge_work(pointer_bytes as u64)?;
        let mut sources = self.sources.iter().collect::<Vec<_>>();
        sources.sort_by(|a, b| a.label.cmp(&b.label));
        if sources
            .windows(2)
            .any(|pair| pair[0].label == pair[1].label)
        {
            return Err(Error::Invalid("public D1 duplicate input label"));
        }
        let mut count = Count {
            len: PREFIX.len(),
            max: max_bytes,
            exceeded: false,
        };
        let mut ledger = Digest256Hasher::new();
        ledger.update(b"tos-public-ledger-membership-v1\0");
        let mut ledger_count = 0u64;
        for (index, source) in sources.iter().enumerate() {
            self.charge_work(source.label.len() as u64 + 64)?;
            if source.digest.is_some()
                && source
                    .label
                    .starts_with("ToS/source-witnesses/access-requests/public-ledger/")
                && source.label.ends_with(".access-request.json")
            {
                ledger.update(&(source.label.len() as u64).to_be_bytes());
                ledger.update(source.label.as_bytes());
                ledger_count = ledger_count
                    .checked_add(1)
                    .ok_or(Error::Budget("public D1 ledger membership count"))?;
            }
            if index != 0 {
                count
                    .write_all(b",")
                    .map_err(|_| Error::Budget("public D1 manifest input binding bytes"))?;
            }
            let entry = Entry {
                path: &source.label,
                size_bytes: source.digest.map(|_| source.len),
                sha256: source.digest.map(|digest| digest.to_hex()),
            };
            serde_json::to_writer(&mut count, &entry).map_err(|e| {
                if count.exceeded {
                    Error::Budget("public D1 manifest input binding bytes")
                } else {
                    Error::Source(e.to_string())
                }
            })?;
        }
        let db = self.read_db()?;
        let mut stmt =
            db.prepare("SELECT path,sha256,size_bytes FROM capture_sources ORDER BY path")?;
        let mut rows = stmt.query([])?;
        let mut parts = Digest256Hasher::new();
        parts.update(b"tos-public-part-closure-v1\0");
        let mut part_count = 0u64;
        while let Some(row) = rows.next()? {
            let absolute: String = row.get(0)?;
            let sha: Vec<u8> = row.get(1)?;
            let size: i64 = row.get(2)?;
            let label = Path::new(&absolute)
                .strip_prefix(&self.root)
                .ok()
                .and_then(Path::to_str)
                .filter(|path| !path.is_empty())
                .ok_or(Error::Invalid("public D1 part logical path"))?;
            if sha.len() != 32 || size < 0 {
                return Err(Error::Invalid("public D1 part binding row"));
            }
            self.charge_work(label.len() as u64 + sha.len() as u64 + 16)?;
            parts.update(&(label.len() as u64).to_be_bytes());
            parts.update(label.as_bytes());
            parts.update(&(size as u64).to_be_bytes());
            parts.update(&sha);
            part_count = part_count
                .checked_add(1)
                .ok_or(Error::Budget("public D1 part binding count"))?;
        }
        let suffix = serde_json::json!({
            "schema":"tos_public_input_binding_v1",
            "public_ledger":{"count":ledger_count,"paths_sha256":ledger.finalize().to_hex()},
            "partitioned":self.partitioned,
            "partition_parts":{"count":part_count,"root_sha256":parts.finalize().to_hex()},
        });
        let suffix = serde_json::to_vec(&suffix).map_err(|e| Error::Source(e.to_string()))?;
        for piece in [
            b"],".as_slice(),
            &suffix[1..suffix.len() - 1],
            b"}".as_slice(),
        ] {
            count
                .write_all(piece)
                .map_err(|_| Error::Budget("public D1 manifest input binding bytes"))?;
        }
        // One exact count admits the raw bytes, bounded parsed Value state and
        // both serialization passes before any growing buffer is allocated.
        let resident = (count.len as u64)
            .checked_mul(4)
            .and_then(|n| n.checked_add((sources.len() as u64).checked_mul(512)?))
            .ok_or(Error::Budget("public D1 manifest input state"))?;
        self.charge_work(resident)?;
        let mut raw = Vec::with_capacity(count.len);
        raw.extend_from_slice(PREFIX);
        for (index, source) in sources.iter().enumerate() {
            if index != 0 {
                raw.push(b',');
            }
            let entry = Entry {
                path: &source.label,
                size_bytes: source.digest.map(|_| source.len),
                sha256: source.digest.map(|digest| digest.to_hex()),
            };
            serde_json::to_writer(&mut raw, &entry).map_err(|e| Error::Source(e.to_string()))?;
        }
        raw.extend_from_slice(b"],");
        raw.extend_from_slice(&suffix[1..suffix.len() - 1]);
        raw.push(b'}');
        if raw.len() != count.len {
            return Err(Error::Invalid("public D1 manifest input serialized size"));
        }
        serde_json::from_slice(&raw).map_err(|e| Error::Source(e.to_string()))
    }

    /// Bind the complete allowlisted public snapshot by stable logical labels.
    /// These digests were measured on capture and are independently rechecked
    /// before completion; transient private paths never enter the revision.
    pub(crate) fn update_revision_sources(&self, hash: &mut Digest256Hasher) -> Result<()> {
        self.check_custody()?;
        for source in &self.sources {
            self.charge_work(source.label.len() as u64 + 41)?;
            hash.update(&(source.label.len() as u64).to_be_bytes());
            hash.update(source.label.as_bytes());
            hash.update(&source.len.to_be_bytes());
            if let Some(digest) = source.digest {
                hash.update(&[1]);
                hash.update(digest.as_bytes());
            } else {
                hash.update(&[0]);
            }
        }
        Ok(())
    }

    pub fn partitioned(&self) -> bool {
        self.partitioned
    }

    pub fn read_input(&self, label: &str, cap: usize) -> Result<Option<Vec<u8>>> {
        if self.prepared_profile {
            let software: Option<&[u8]> = match label {
                "ToS/doctrine/semantic-interchange/query-vocabulary.v1.json" => {
                    Some(include_bytes!(
                        "../../../../ToS/doctrine/semantic-interchange/query-vocabulary.v1.json"
                    ))
                }
                "ToS/contracts/semantic-entity-type-registry.schema.json" => Some(include_bytes!(
                    "../../../../ToS/contracts/semantic-entity-type-registry.schema.json"
                )),
                "ToS/contracts/semantic-relation-type-registry.schema.json" => {
                    Some(include_bytes!(
                        "../../../../ToS/contracts/semantic-relation-type-registry.schema.json"
                    ))
                }
                _ => None,
            };
            if let Some(raw) = software {
                if raw.len() > cap {
                    return Err(Error::Budget("prepared software companion bytes"));
                }
                self.charge_work(raw.len() as u64)?;
                return Ok(Some(raw.to_vec()));
            }
        }
        self.check_custody()?;
        let source = self
            .sources
            .iter()
            .find(|source| source.label == label)
            .ok_or(Error::Invalid("public D1 input outside exact closure"))?;
        let Some(expected) = source.digest else {
            return Ok(None);
        };
        if source.len > cap as u64 {
            return Err(Error::Budget("public D1 input bytes"));
        }
        self.charge_work(source.len)?;
        let mut file = match &source.origin {
            SourceOrigin::Compiled(raw) => return Ok(Some(raw.to_vec())),
            SourceOrigin::File(path) => profile_open(path, cap as u64, self.prepared_profile)?,
        };
        let mut raw = Vec::with_capacity(source.len as usize);
        file.take(cap as u64 + 1).read_to_end(&mut raw)?;
        if raw.len() as u64 != source.len || Digest256::of_bytes(&raw) != expected {
            return Err(Error::Invalid("public D1 input changed"));
        }
        Ok(Some(raw))
    }

    /// The maintained partitioned compiler binds exactly five logical roots;
    /// part hashes are recursively committed by each manifest root. This is
    /// a source revision, not a publication or selected-current receipt.
    pub fn partitioned_source_revision(&self) -> Result<String> {
        if !self.partitioned {
            return Err(Error::Invalid("public D1 partitioned revision mode"));
        }
        let mut bindings = serde_json::Map::new();
        for label in [
            "ToS/derived-exports/tos_corpus_index.min.json",
            "ToS/derived-exports/philosophy_graph_projection.min.json",
            "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json",
            "ToS/doctrine/semantic-interchange/entity-types.v1.json",
            "ToS/doctrine/semantic-interchange/relation-types.v1.json",
        ] {
            let source = self
                .sources
                .iter()
                .find(|source| source.label == label)
                .ok_or(Error::Invalid("public D1 revision input"))?;
            let digest = source
                .digest
                .ok_or(Error::Invalid("public D1 revision missing input"))?;
            bindings.insert(label.to_owned(), serde_json::Value::String(digest.to_hex()));
        }
        let value =
            serde_json::json!({"compiler":"tos_offline_knowledge_v2","snapshot_bindings":bindings});
        let raw = serde_json::to_vec(&value).map_err(|e| Error::Source(e.to_string()))?;
        let value = json(&raw, MAX_HEADER_BYTES)?;
        let canonical = tos_foundation::canonical_bytes_v1(
            &value,
            tos_foundation::CanonicalProfile::SourceRecordDigestV1,
            JsonLimits::new(MAX_HEADER_BYTES, 96, 1_000_000, 4096)
                .map_err(|_| Error::Budget("public D1 revision JSON"))?,
        )
        .map_err(|e| Error::Source(e.to_string()))?;
        Ok(Digest256::of_bytes(&canonical).to_hex())
    }

    fn stable_role(&self, hash: &mut Digest256Hasher, role: &str, prefix: &str) -> Result<()> {
        use crate::knowledge_normalization::{stable_digest_value, write_len, write_string};
        let db = self.read_db()?;
        let mut keys = BTreeSet::new();
        let mut stmt =
            db.prepare("SELECT path FROM capture_headers WHERE role=?1 ORDER BY path")?;
        for item in stmt.query_map([role], |row| row.get::<_, String>(0))? {
            let item = item?;
            self.charge_work(item.len() as u64)?;
            if prefix.is_empty() {
                if let Some((root, _)) = item.split_once('/') {
                    keys.insert(root.to_owned());
                } else {
                    keys.insert(item);
                }
            } else if let Some(nested) = item
                .strip_prefix(prefix)
                .and_then(|path| path.strip_prefix('/'))
            {
                keys.insert(nested.to_owned());
            }
        }
        let mut stmt = db.prepare(
            "SELECT collection FROM capture_collections WHERE role=?1 ORDER BY collection",
        )?;
        for item in stmt.query_map([role], |row| row.get::<_, String>(0))? {
            let item = item?;
            self.charge_work(item.len() as u64)?;
            if prefix.is_empty() {
                if let Some((root, _)) = item.split_once('/') {
                    keys.insert(root.to_owned());
                } else {
                    keys.insert(item);
                }
            } else if let Some(nested) = item
                .strip_prefix(prefix)
                .and_then(|path| path.strip_prefix('/'))
            {
                keys.insert(nested.to_owned());
            }
        }
        hash.update(b"o");
        write_len(hash, keys.len());
        hash.update(b"{");
        for key in keys {
            write_string(hash, &key);
            let path = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}/{key}")
            };
            if path == "source_navigation" {
                self.stable_role(hash, role, "source_navigation")?;
                continue;
            }
            let header: Option<Vec<u8>> = db
                .query_row(
                    "SELECT json FROM capture_headers WHERE role=?1 AND path=?2",
                    params![role, path],
                    |row| row.get(0),
                )
                .optional()?;
            if let Some(raw) = header {
                self.charge_work(raw.len() as u64)?;
                let value: serde_json::Value = strict_value(&raw, MAX_HEADER_BYTES)?;
                stable_digest_value(&value, hash)?;
                continue;
            }
            let count: u64 = db.query_row(
                "SELECT count(*) FROM capture_rows WHERE role=?1 AND collection=?2",
                params![role, path],
                |row| row.get(0),
            )?;
            hash.update(b"a");
            write_len(hash, count as usize);
            hash.update(b"[");
            let actual = self.visit_rows(role, &path, |_, raw| {
                let value: serde_json::Value = strict_value(raw, MAX_ROW_BYTES)?;
                stable_digest_value(&value, hash)
            })?;
            if actual != count {
                return Err(Error::Invalid("public D1 stable revision rows"));
            }
            hash.update(b"]");
        }
        hash.update(b"}");
        Ok(())
    }

    /// The legacy public graph uses Python's full logical-value stable digest.
    /// Emit its container framing over the captured disk rows in owner order,
    /// never rebuilding the 61/30/10 MiB objects in memory.
    pub fn legacy_source_revision(&self) -> Result<String> {
        if self.partitioned {
            return Err(Error::Invalid("public D1 legacy revision mode"));
        }
        self.core_source_revision()
    }

    /// Python Core's exact whole-source revision framing, computed over the
    /// retained logical role documents even when physical rows came from
    /// partitioned projection members.
    pub fn core_source_revision(&self) -> Result<String> {
        if self.runtime_capture_role.is_some() {
            return Err(Error::Invalid(
                "whole Core revision requires all five inputs",
            ));
        }
        use crate::knowledge_normalization::{stable_digest_value, write_len, write_string};
        let mut hash = Digest256Hasher::new();
        hash.update(b"o");
        write_len(&mut hash, 5);
        hash.update(b"{");
        for (name, role) in [
            ("bibliographic_claims", Some(CLAIMS)),
            ("corpus", Some(CORPUS)),
            ("entity_type_registry", None),
            ("philosophy", Some(PHILOSOPHY)),
            ("relation_type_registry", None),
        ] {
            write_string(&mut hash, name);
            match role {
                Some(role) => self.stable_role(&mut hash, role, "")?,
                None => {
                    let label = if name == "entity_type_registry" {
                        "ToS/doctrine/semantic-interchange/entity-types.v1.json"
                    } else {
                        "ToS/doctrine/semantic-interchange/relation-types.v1.json"
                    };
                    let raw = self
                        .read_input(label, 4 * 1024 * 1024)?
                        .ok_or(Error::Invalid("public D1 missing registry"))?;
                    let value: serde_json::Value = strict_value(&raw, 4 * 1024 * 1024)?;
                    stable_digest_value(&value, &mut hash)?;
                }
            }
        }
        hash.update(b"}");
        Ok(hash.finalize().to_hex())
    }

    pub fn header(&self, role: &str, path: &str) -> Result<JsonValue> {
        self.check_custody()?;
        let db = self.read_db()?;
        let raw: Vec<u8> = db.query_row(
            "SELECT json FROM capture_headers WHERE role=?1 AND path=?2",
            params![role, path],
            |row| row.get(0),
        )?;
        self.charge_work(raw.len() as u64)?;
        json(&raw, MAX_HEADER_BYTES)
    }

    /// Reconstruct only the bounded non-row header of one captured projection.
    /// Collection values stay in the disk index and never enter this object.
    pub(crate) fn header_object(
        &self,
        role: &str,
        prefix: &str,
        max_bytes: usize,
    ) -> Result<serde_json::Value> {
        self.check_custody()?;
        if max_bytes == 0 || max_bytes > MAX_HEADER_BYTES {
            return Err(Error::Budget("public D1 header object bytes"));
        }
        let db = self.read_db()?;
        let mut statement = db.prepare(
            "SELECT path,length(json),CASE WHEN length(json) <= ?2 THEN json END
               FROM capture_headers WHERE role=?1 ORDER BY path",
        )?;
        let mut rows = statement.query(params![role, max_bytes as i64])?;
        let mut fields = serde_json::Map::new();
        let mut total = 2usize;
        while let Some(row) = rows.next()? {
            let path: String = row.get(0)?;
            let dynamic_philosophy = self.runtime_capture_role
                == Some(RuntimeCaptureRole::Philosophy)
                && role == PHILOSOPHY;
            let Some(name) = (if prefix.is_empty() {
                (dynamic_philosophy || !path.contains('/')).then_some(path.as_str())
            } else {
                path.strip_prefix(prefix)
                    .and_then(|name| name.strip_prefix('/'))
            }) else {
                continue;
            };
            if name.is_empty() || (!dynamic_philosophy && name.contains('/')) {
                return Err(Error::Invalid("public D1 nested header path"));
            }
            let declared: i64 = row.get(1)?;
            if declared <= 0 || declared as usize > max_bytes {
                return Err(Error::Budget("public D1 header object bytes"));
            }
            // Bound the JSON representation before SQLite copies the BLOB or
            // the serde map allocates its decoded subtree. Six bytes per key
            // byte is a conservative upper bound for JSON escaping.
            let field_bytes = name
                .len()
                .checked_mul(6)
                .and_then(|bytes| bytes.checked_add(3))
                .and_then(|bytes| bytes.checked_add(declared as usize))
                .and_then(|bytes| bytes.checked_add(usize::from(!fields.is_empty())))
                .ok_or(Error::Budget("public D1 header object bytes"))?;
            total = total
                .checked_add(field_bytes)
                .filter(|total| *total <= max_bytes)
                .ok_or(Error::Budget("public D1 header object bytes"))?;
            let raw: Option<Vec<u8>> = row.get(2)?;
            let raw = raw.ok_or(Error::Budget("public D1 header object bytes"))?;
            if raw.len() != declared as usize {
                return Err(Error::Invalid("public D1 header length changed"));
            }
            self.charge_work((path.len() + raw.len()) as u64)?;
            let value = strict_value(&raw, max_bytes)?;
            if fields.insert(name.to_owned(), value).is_some() {
                return Err(Error::Invalid("public D1 duplicate header path"));
            }
        }
        Ok(serde_json::Value::Object(fields))
    }

    /// Declared presence and container kind from the authenticated capture.
    /// An absent collection remains absent; callers must not invent empty rows.
    pub(crate) fn captured_collection_kind(
        &self,
        role: &str,
        collection: &str,
    ) -> Result<Option<String>> {
        self.check_custody()?;
        let dynamic_philosophy = self.runtime_capture_role == Some(RuntimeCaptureRole::Philosophy)
            && role == PHILOSOPHY
            && valid_top_level_collection(collection);
        if !selected_rows(role, collection)
            && !dynamic_philosophy
            && !(role == CORPUS && collection == "source_navigation")
        {
            return Err(Error::Invalid("public D1 collection outside fixed input"));
        }
        let db = self.read_db()?;
        use rusqlite::OptionalExtension;
        let kind: Option<String> = db
            .query_row(
                "SELECT kind FROM capture_collections WHERE role=?1 AND collection=?2",
                params![role, collection],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(value) = &kind {
            self.charge_work(value.len() as u64)?;
        }
        Ok(kind)
    }

    pub(crate) fn captured_collection_names(&self, role: &str) -> Result<Vec<String>> {
        self.check_custody()?;
        if self.runtime_capture_role != Some(RuntimeCaptureRole::Philosophy) || role != PHILOSOPHY {
            return Err(Error::Invalid("dynamic philosophy carrier required"));
        }
        let db = self.read_db()?;
        let mut statement = db.prepare(
            "SELECT collection FROM capture_collections WHERE role=?1 ORDER BY collection",
        )?;
        let mut rows = statement.query([role])?;
        let mut names = Vec::new();
        while let Some(row) = rows.next()? {
            if names.len() >= 4096 {
                return Err(Error::Budget("public D1 captured collection names"));
            }
            let name: String = row.get(0)?;
            if !valid_top_level_collection(&name) {
                return Err(Error::Invalid(
                    "public D1 captured philosophy collection name",
                ));
            }
            self.charge_work(name.len() as u64)?;
            names.push(name);
        }
        Ok(names)
    }

    /// Physical part order is a hash traversal. This cursor restores the
    /// owner's declared logical order from the private disk index; each row is
    /// checked against the digest recorded at capture before it is exposed.
    pub fn visit_rows(
        &self,
        role: &str,
        collection: &str,
        mut sink: impl FnMut(u64, &[u8]) -> Result<()>,
    ) -> Result<u64> {
        self.check_custody()?;
        let dynamic_philosophy = self.runtime_capture_role == Some(RuntimeCaptureRole::Philosophy)
            && role == PHILOSOPHY
            && valid_top_level_collection(collection);
        if !selected_rows(role, collection) && !dynamic_philosophy {
            return Err(Error::Invalid("public D1 collection outside fixed input"));
        }
        let db = self.read_db()?;
        let mut statement = db.prepare("SELECT json,sha256 FROM capture_rows WHERE role=?1 AND collection=?2 ORDER BY sort0,sort1,source_key")?;
        let mut rows = statement.query(params![role, collection])?;
        let mut count = 0u64;
        while let Some(row) = rows.next()? {
            let raw: Vec<u8> = row.get(0)?;
            self.charge_work(raw.len() as u64)?;
            let expected: Vec<u8> = row.get(1)?;
            if raw.len() > MAX_ROW_BYTES
                || Digest256::of_bytes(&raw).as_bytes().as_slice() != expected.as_slice()
            {
                return Err(Error::Invalid("public D1 captured row mismatch"));
            }
            sink(count, &raw)?;
            count = count
                .checked_add(1)
                .ok_or(Error::Budget("public D1 visit rows"))?;
        }
        Ok(count)
    }
}
