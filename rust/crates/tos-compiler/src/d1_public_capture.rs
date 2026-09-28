//! Exact, disposable capture of the three allowlisted public graph projections.
//! Rows live on disk in source encounter order. This is a read-model input,
//! never a source, rights, canon or installed-current grant.

use crate::{Error, Limits, Result, legacy::decode_partition_part, safe_open, sqlite_budget};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use serde::de::{DeserializeSeed, Error as _, MapAccess, SeqAccess, Visitor};
use serde_json::value::RawValue;
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    fmt,
    fs::{self, File},
    io::{self, BufReader, Read, Seek, SeekFrom},
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
    path: PathBuf,
    inode: (u64, u64),
    sources: Vec<SourceFile>,
    partitioned: bool,
    pub rows: u64,
    work_bytes: Rc<Cell<u64>>,
    max_work_bytes: u64,
    deadline: Instant,
    limits: PublicCaptureLimits,
    vm_used: Arc<AtomicU64>,
}

struct SourceFile {
    label: String,
    path: PathBuf,
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

struct CaptureWriter<'a> {
    db: &'a Connection,
    role: &'static str,
    row_count: &'a mut u64,
    work: &'a mut u64,
    limits: PublicCaptureLimits,
    ordinals: BTreeMap<String, u64>,
    read_budget: Rc<Cell<usize>>,
    deadline: Instant,
    page_rows: usize,
    page_bytes: usize,
}

// serde_json's RawValue owns a row before the row callback can inspect it.
// Bound bytes delivered to that allocation, including punctuation/whitespace.
// The underlying file reader has a fixed 64 KiB buffer.
struct CaptureReader<R> {
    inner: R,
    remaining: Rc<Cell<usize>>,
}
impl<R: Read> Read for CaptureReader<R> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
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
        if !selected_rows(self.role, collection) || !matches!(kind, "array" | "mapping") {
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
    ) -> Result<()> {
        if Instant::now() >= self.deadline {
            return Err(Error::Budget("public D1 capture deadline"));
        }
        if !selected_rows(self.role, collection) {
            return Err(Error::Invalid("public D1 collection outside fixed input"));
        }
        if raw.len() > MAX_ROW_BYTES {
            return Err(Error::Budget("public D1 row bytes"));
        }
        checked_add(self.work, raw.len(), self.limits.max_work_bytes)?;
        let value = json(raw, MAX_ROW_BYTES)?;
        if value.as_object().is_none() {
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
                    self.writer
                        .row(&self.collection, raw.get().as_bytes(), None, &[])
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
                    if self.prefix.is_empty()
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

fn source_digest(
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
        _ => Err(Error::Invalid("public projection order value")),
    }
}

fn partition_order(row: &JsonValue, fields: &[&str]) -> Result<Vec<String>> {
    fields
        .iter()
        .map(|field| order_value(row.object_get(field)))
        .collect()
}

fn partition_record_key(value: &JsonValue, fields: &[&str]) -> Result<String> {
    let mut keys = Vec::with_capacity(fields.len());
    for field in fields {
        let key = value
            .object_get(field)
            .and_then(JsonValue::as_str)
            .filter(|key| !key.is_empty() && key.len() <= 4096)
            .ok_or(Error::Invalid("public projection record identity"))?;
        keys.push(key);
    }
    if keys.len() == 1 {
        return Ok(keys[0].to_owned());
    }
    serde_json::to_string(&keys).map_err(|e| Error::Source(e.to_string()))
}

fn capture_part(
    root_path: &Path,
    descriptor: &serde_json::Value,
    prefix: &str,
    writer: &mut CaptureWriter<'_>,
    collection: &str,
    key_fields: &[&str],
    order_fields: &[&str],
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
            partition_order(value, order_fields)?
        };
        writer.row(collection, &encoded, Some(key), &order)?;
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
        CORPUS => "tos_corpus_index_v1",
        PHILOSOPHY => "tos_philosophy_graph_projection_v2",
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
    let required: &[&str] = match writer.role {
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
    };
    if !required.iter().all(|name| collections.contains_key(*name)) {
        return Err(Error::Invalid(
            "public projection missing maintained collection",
        ));
    }
    for (name, spec) in collections {
        let (key_fields, order_fields, mapping) = policy(writer.role, name).ok_or(
            Error::Invalid("public projection collection outside owner policy"),
        )?;
        writer.collection(name, if mapping { "mapping" } else { "array" })?;
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
        let spec_object = spec
            .as_object()
            .ok_or(Error::Invalid("public projection collection spec"))?;
        if spec_object.len() != 3
            || spec.get("key_field") != Some(&expected_key)
            || spec.get("order_fields") != Some(&expected_order)
        {
            return Err(Error::Invalid("public projection owner ordering policy"));
        }
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
            key_fields,
            order_fields,
            mapping,
            count,
        )?;
        if found != count {
            return Err(Error::Invalid("public projection collection count"));
        }
    }
    Ok(())
}

impl PublicCapture {
    pub fn create(
        root: &Path,
        staging: &Path,
        limits: PublicCaptureLimits,
        deadline: Instant,
    ) -> Result<Self> {
        limits.validate()?;
        if Instant::now() >= deadline {
            return Err(Error::Budget("public D1 capture deadline"));
        }
        if staging.exists() || staging.is_symlink() {
            return Err(Error::Invalid("public D1 capture staging must be fresh"));
        }
        let mut pending = PendingCapture {
            path: staging,
            complete: false,
        };
        let mut db = Connection::open(staging)?;
        let vm_used = Arc::new(AtomicU64::new(0));
        sqlite_budget::install_progress(&db, limits.sqlite(), Arc::clone(&vm_used));
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
            let path = root.join(relative);
            let mut file = safe_open::open_regular(&path, limits.max_input_bytes)?;
            let (digest, len) = source_digest(&mut file, limits.max_input_bytes, |n| {
                if Instant::now() >= deadline {
                    return Err(Error::Budget("public D1 capture deadline"));
                }
                checked_add(&mut work_bytes, n, limits.max_work_bytes)
            })?;
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
                let first = strict_value(&raw, MAX_ROOT_BYTES as usize)?;
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
            let stored_schema: Vec<u8> = db.query_row(
                "SELECT json FROM capture_headers WHERE role=?1 AND path='schema_version'",
                [role],
                |row| row.get(0),
            )?;
            let expected_schema = match role {
                CORPUS => "tos_corpus_index_v1",
                PHILOSOPHY => "tos_philosophy_graph_projection_v2",
                CLAIMS => "tos_source_witness_bibliographic_graph_v1",
                _ => return Err(Error::Invalid("public D1 source role")),
            };
            if json(&stored_schema, 4096)?.as_str() != Some(expected_schema) {
                return Err(Error::Invalid("public D1 source schema"));
            }
            if role == CORPUS {
                corpus_partitioned = Some(partitioned);
            }
            if role == CLAIMS {
                claims_partitioned = Some(partitioned);
            }
            drop(writer);
            db.execute_batch("COMMIT")?;
            sources.push(SourceFile {
                label: relative.to_owned(),
                path,
                digest: Some(digest),
                len,
            });
        }
        if corpus_partitioned != claims_partitioned {
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
            let path = root.join(relative);
            let mut file = safe_open::open_regular(&path, limits.max_input_bytes)?;
            let (digest, len) = source_digest(&mut file, limits.max_input_bytes, |n| {
                if Instant::now() >= deadline {
                    return Err(Error::Budget("public D1 capture deadline"));
                }
                checked_add(&mut work_bytes, n, limits.max_work_bytes)
            })?;
            sources.push(SourceFile {
                label: relative.to_owned(),
                path,
                digest: Some(digest),
                len,
            });
        }
        for relative in [
            "ToS/derived-exports/epistemic_evidence_projection.min.json",
            "ToS/philosophy/graph-workbench/review-packets/table-i-post-planting-audit.json",
        ] {
            let path = root.join(relative);
            if path.exists() || path.is_symlink() {
                let mut file = safe_open::open_regular(&path, limits.max_input_bytes)?;
                let (digest, len) = source_digest(&mut file, limits.max_input_bytes, |n| {
                    if Instant::now() >= deadline {
                        return Err(Error::Budget("public D1 capture deadline"));
                    }
                    checked_add(&mut work_bytes, n, limits.max_work_bytes)
                })?;
                sources.push(SourceFile {
                    label: relative.to_owned(),
                    path,
                    digest: Some(digest),
                    len,
                });
            } else {
                sources.push(SourceFile {
                    label: relative.to_owned(),
                    path,
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
                    if Instant::now() >= deadline {
                        return Err(Error::Budget("public D1 capture deadline"));
                    }
                    checked_add(&mut work_bytes, n, limits.max_work_bytes)
                })?;
                sources.push(SourceFile {
                    label: format!("{ledger_relative}/{name}"),
                    path,
                    digest: Some(digest),
                    len,
                });
            }
        }
        if std::fs::metadata(staging)?.len() > limits.max_staging_bytes {
            return Err(Error::Budget("public D1 capture physical bytes"));
        }
        drop(db);
        let metadata = fs::symlink_metadata(staging)?;
        if !metadata.file_type().is_file() {
            return Err(Error::Invalid("public D1 capture inode"));
        }
        pending.complete = true;
        Ok(Self {
            root: root.to_owned(),
            path: staging.to_owned(),
            inode: (metadata.dev(), metadata.ino()),
            sources,
            partitioned: corpus_partitioned == Some(true),
            rows,
            work_bytes: Rc::new(Cell::new(work_bytes)),
            max_work_bytes: limits.max_work_bytes,
            deadline,
            limits,
            vm_used,
        })
    }

    pub fn charge_work(&self, bytes: u64) -> Result<()> {
        if Instant::now() >= self.deadline {
            return Err(Error::Budget("public D1 build deadline"));
        }
        let next = self
            .work_bytes
            .get()
            .checked_add(bytes)
            .filter(|value| *value <= self.max_work_bytes)
            .ok_or(Error::Budget("public D1 build work bytes"))?;
        self.work_bytes.set(next);
        Ok(())
    }

    pub fn work_bytes(&self) -> u64 {
        self.work_bytes.get()
    }

    pub(crate) fn vm_counter(&self) -> Arc<AtomicU64> {
        Arc::clone(&self.vm_used)
    }

    pub(crate) fn work_counter(&self) -> Rc<Cell<u64>> {
        Rc::clone(&self.work_bytes)
    }

    pub(crate) fn max_work_bytes(&self) -> u64 {
        self.max_work_bytes
    }

    pub fn check_custody(&self) -> Result<()> {
        let metadata = fs::symlink_metadata(&self.path)?;
        if !metadata.file_type().is_file() || (metadata.dev(), metadata.ino()) != self.inode {
            return Err(Error::Invalid("public D1 private capture replaced"));
        }
        self.charge_work(0)
    }

    pub(crate) fn read_db(&self) -> Result<Connection> {
        self.check_custody()?;
        let db = Connection::open_with_flags(&self.path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        sqlite_budget::install_progress(&db, self.limits.sqlite(), Arc::clone(&self.vm_used));
        db.pragma_update(None, "cache_size", -(self.limits.sqlite_cache_kib as i64))?;
        Ok(db)
    }

    pub(crate) fn write_db(&self) -> Result<Connection> {
        self.check_custody()?;
        let db = Connection::open(&self.path)?;
        sqlite_budget::install_progress(&db, self.limits.sqlite(), Arc::clone(&self.vm_used));
        db.pragma_update(None, "cache_size", -(self.limits.sqlite_cache_kib as i64))?;
        db.execute_batch(
            "PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL; PRAGMA temp_store=FILE",
        )?;
        Ok(db)
    }

    pub fn verify_inputs(&self, limits: PublicCaptureLimits) -> Result<()> {
        self.check_custody()?;
        if limits.max_work_bytes != self.max_work_bytes {
            return Err(Error::Invalid("public D1 changed work budget"));
        }
        for source in &self.sources {
            if source.digest.is_none() {
                if source.path.exists() || source.path.is_symlink() {
                    return Err(Error::Invalid(
                        "public D1 optional source appeared during build",
                    ));
                }
                continue;
            }
            let mut file = safe_open::open_regular(&source.path, limits.max_input_bytes)?;
            let (digest, len) = source_digest(&mut file, limits.max_input_bytes, |n| {
                self.charge_work(n as u64)
            })?;
            if len != source.len || Some(digest) != source.digest {
                return Err(Error::Invalid("public D1 source changed during build"));
            }
        }
        let ledger = self
            .root
            .join("ToS/source-witnesses/access-requests/public-ledger");
        let mut current = BTreeSet::new();
        if ledger.exists() {
            if ledger.is_symlink() || !ledger.is_dir() {
                return Err(Error::Invalid("public D1 ledger changed"));
            }
            for entry in std::fs::read_dir(&ledger)? {
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
                self.charge_work(n as u64)
            })?;
            if actual_len != len || actual.as_bytes().as_slice() != digest.as_slice() {
                return Err(Error::Invalid("public D1 part changed during build"));
            }
        }
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn root(&self) -> &Path {
        &self.root
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
        let mut file = safe_open::open_regular(&source.path, cap as u64)?;
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
        use crate::knowledge_normalization::{stable_digest_value, write_len, write_string};
        if self.partitioned {
            return Err(Error::Invalid("public D1 legacy revision mode"));
        }
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
        let mut statement =
            db.prepare("SELECT path,json FROM capture_headers WHERE role=?1 ORDER BY path")?;
        let mut rows = statement.query([role])?;
        let mut fields = serde_json::Map::new();
        let mut total = 0usize;
        while let Some(row) = rows.next()? {
            let path: String = row.get(0)?;
            let Some(name) = (if prefix.is_empty() {
                (!path.contains('/')).then_some(path.as_str())
            } else {
                path.strip_prefix(prefix)
                    .and_then(|name| name.strip_prefix('/'))
            }) else {
                continue;
            };
            if name.is_empty() || name.contains('/') {
                return Err(Error::Invalid("public D1 nested header path"));
            }
            let raw: Vec<u8> = row.get(1)?;
            total = total
                .checked_add(path.len() + raw.len())
                .filter(|total| *total <= max_bytes)
                .ok_or(Error::Budget("public D1 header object bytes"))?;
            self.charge_work((path.len() + raw.len()) as u64)?;
            let value = strict_value(&raw, max_bytes)?;
            if fields.insert(name.to_owned(), value).is_some() {
                return Err(Error::Invalid("public D1 duplicate header path"));
            }
        }
        Ok(serde_json::Value::Object(fields))
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
        if !selected_rows(role, collection) {
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
