//! Maintained native validation for the standalone Tree-of-Sophia source tree.
//!
//! This is a mechanical check only. It issues no source, rights, semantic,
//! canon, publication, or selected-reader authority. The source mode borrows
//! the existing captured native snapshot construction and query/schema owners.

use flate2::read::MultiGzDecoder;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{Read, Write},
    os::unix::fs::MetadataExt,
    path::{Component, Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
use tos_foundation::{
    Digest256, Digest256Hasher, JsonLimits, JsonMode, JsonValue, emit_value_preserved_json,
    parse_json,
};
use tos_query::{AbortProbe, AbortReason};

const REPORT_SCHEMA: &str = "tos_standalone_validation_v1";
const QUERY_OPERATIONS: &[&str] = &[
    "tos.status",
    "tos.snapshot",
    "tos.search",
    "tos.knowledge.search",
    "tos_philosophy_graph_scale_rows",
    "tos.source-gaps.search",
    "tos.source.descend",
    "tos.dossier.inspect",
    "tos.view.open",
    "tos.node.inspect",
    "tos.neighborhood",
    "tos.epistemic.inspect",
    "tos.path.find",
    "tos.zarathustra.word-analysis.prepare",
    "tos_philosophy_graph_lens_packet",
    "tos.zarathustra.word_analysis.public-capability",
    "tos.zarathustra.reading.public-capability",
];
const KNOWLEDGE_OPERATIONS: &[&str] = &[
    "tos.knowledge.catalog",
    "tos.knowledge.contracts",
    "tos.knowledge.search",
    "tos.knowledge.search.capabilities",
    "tos.knowledge.node.inspect",
    "tos.knowledge.relation.inspect",
    "tos.knowledge.temporal.compare",
    "tos.knowledge.focus",
    "tos.lens.open",
    "tos.lens.compile",
    "tos.source.read.contracts",
    "tos.source.read.capabilities",
    "tos.source.handle.discover",
    "tos.source.record.read",
];
const PAGE_COMMANDS: &[&str] = &[
    "tos.page.context",
    "tos.page.open-view",
    "tos.page.search",
    "tos.page.knowledge-search",
    "tos.page.find-source-gaps",
    "tos.page.prepare-word-analysis",
    "tos.page.select",
    "tos.page.inspect-selection",
    "tos.page.show-neighborhood",
    "tos.page.start-path",
    "tos.page.find-path",
    "tos.page.reroute-without-selection",
    "tos.page.inspect-epistemic",
    "tos.page.compare-readings",
    "tos.page.research-workspace",
    "tos.page.add-research-note",
    "tos.page.add-session-hypothesis",
    "tos.page.stage-proposal",
    "tos.page.exclude-selected-edge",
    "tos.page.save-route-comparison",
    "tos.page.workspace-undo",
    "tos.page.workspace-redo",
    "tos.page.workspace-export",
    "tos.page.workspace-import",
    "tos.page.clear-focus",
    "tos.page.cancel",
];
const TEN_ACCESS_SCHEMAS: &[&str] = &[
    "access/contracts/knowledge-graph.v1.schema.json",
    "access/contracts/knowledge-search-indexed.v2.schema.json",
    "access/contracts/lens-spec.v1.schema.json",
    "access/contracts/lens-result.v1.schema.json",
    "access/contracts/temporal-comparison-request.v1.schema.json",
    "access/contracts/temporal-comparison-result.v1.schema.json",
    "access/contracts/exploration-request.v1.schema.json",
    "access/contracts/exploration-result.v1.schema.json",
    "access/contracts/exploration-request.v2.schema.json",
    "access/contracts/exploration-result.v2.schema.json",
];
const NATIVE_ADDITIONAL_SCHEMAS: &[&str] = &[
    "ToS/contracts/semantic-entity-type-registry.schema.json",
    "ToS/contracts/semantic-relation-type-registry.schema.json",
    "access/contracts/readable-context.v1.schema.json",
    "access/contracts/source-read.v1.schema.json",
];
const ALL_PROGRAM_SCHEMAS: &[&str] = &[
    "access/contracts/knowledge-graph.v1.schema.json",
    "access/contracts/knowledge-search-indexed.v2.schema.json",
    "access/contracts/lens-spec.v1.schema.json",
    "access/contracts/lens-result.v1.schema.json",
    "access/contracts/temporal-comparison-request.v1.schema.json",
    "access/contracts/temporal-comparison-result.v1.schema.json",
    "access/contracts/exploration-request.v1.schema.json",
    "access/contracts/exploration-result.v1.schema.json",
    "access/contracts/exploration-request.v2.schema.json",
    "access/contracts/exploration-result.v2.schema.json",
    "ToS/contracts/semantic-entity-type-registry.schema.json",
    "ToS/contracts/semantic-relation-type-registry.schema.json",
    "access/contracts/readable-context.v1.schema.json",
    "access/contracts/source-read.v1.schema.json",
];
const SOFTWARE_CONTRACT_PATHS: &[&str] = &[
    "access/contracts/runtime-manifest.v1.json",
    "access/profiles/abyssos.v1.json",
    "access/contracts/web-actions.v1.json",
    "access/contracts/query-operations.v1.json",
    "access/contracts/knowledge-api.v1.json",
    "access/contracts/epistemic-packet.v1.schema.json",
    "ToS/contracts/epistemic-evidence-projection.schema.json",
    "access/contracts/evidence-lens-packet.v1.schema.json",
    "access/contracts/page-commands.v1.json",
    "access/contracts/research-workspace.v1.schema.json",
    "access/contracts/runtime-data.v1.json",
];
const REQUIRED_RUNTIME_SUBJECTS: &[&str] = &[
    "ToS/derived-exports/epistemic_evidence_projection.min.json",
    "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json",
    "ToS/doctrine/semantic-interchange/entity-types.v1.json",
    "ToS/doctrine/semantic-interchange/relation-types.v1.json",
    "ToS/contracts/semantic-entity-type-registry.schema.json",
    "ToS/contracts/semantic-relation-type-registry.schema.json",
];
const CAPTURED_INPUT_SUBJECTS: &[&str] = &[
    "tos-corpus-index",
    "tos-philosophy-graph",
    "tos-source-witness-bibliographic-claim-graph",
    "tos-semantic-entity-type-registry",
    "tos-semantic-relation-type-registry",
];
const COMPILED_QUERY_STORE_ID: &str = "tos-compiled-query-store";
const COMPILED_QUERY_STORE_OUTPUT: &str = "ToS/derived-exports/runtime/knowledge.sqlite3";
const COMPILED_QUERY_STORE_BUILDER: &str = "tos_access.knowledge_compile";
const BLOCKED_CODE_MARKERS: &[&[u8]] = &[b"/srv/AbyssOS", b"/srv/abyss-machine"];
const PARTITION_ROOT_BYTES: u64 = 256 * 1024;
const PARTITION_INDEX_BYTES: usize = 128 * 1024;
const PARTITION_DATA_BYTES: usize = 8 * 1024 * 1024;
const PARTITION_KEY_BYTES: usize = 4096;
const PARTITION_STORED_OVERHEAD: usize = 65_536;
// These count input work, not process RSS. The admitted working-RAM ticket
// must separately cover up to 64 retained index ancestors, the root plus its
// metadata clone, and one stored/decoded data-part overlap.
const SCHEMA_RESOURCE_BYTES: usize = 256 * 1024;
const SOFTWARE_BYTES: u64 = 64 * 1024 * 1024;
const SOFTWARE_ROWS: u64 = 100_000;
const SOFTWARE_FILE_BYTES: u64 = 16 * 1024 * 1024;
const SOFTWARE_SECONDS: u64 = 45;
const MAX_BUILD_SECONDS: u64 = 2 * 60 * 60;
const SOURCE_JSON_DEPTH: usize = 96;
const SOURCE_JSON_VISITS: usize = 1_000_000;

#[derive(Clone, Copy)]
struct Stamp {
    dev: u64,
    ino: u64,
    len: u64,
    mtime: i64,
    mtime_nsec: i64,
    ctime: i64,
    ctime_nsec: i64,
}
impl From<&fs::Metadata> for Stamp {
    fn from(value: &fs::Metadata) -> Self {
        Self {
            dev: value.dev(),
            ino: value.ino(),
            len: value.len(),
            mtime: value.mtime(),
            mtime_nsec: value.mtime_nsec(),
            ctime: value.ctime(),
            ctime_nsec: value.ctime_nsec(),
        }
    }
}
impl PartialEq for Stamp {
    fn eq(&self, other: &Self) -> bool {
        (
            self.dev,
            self.ino,
            self.len,
            self.mtime,
            self.mtime_nsec,
            self.ctime,
            self.ctime_nsec,
        ) == (
            other.dev,
            other.ino,
            other.len,
            other.mtime,
            other.mtime_nsec,
            other.ctime,
            other.ctime_nsec,
        )
    }
}
impl Eq for Stamp {}

struct Meter {
    max_bytes: u64,
    max_rows: u64,
    bytes: u64,
    rows: u64,
    work: u64,
    max_work: u64,
    deadline: Instant,
    root_path: PathBuf,
    root_stamp: Stamp,
    root_fd: File,
    held: BTreeMap<String, Stamp>,
    absent: BTreeSet<String>,
}
impl Meter {
    fn new(root: &Path, max_bytes: u64, max_rows: u64, deadline: Instant) -> Result<Self, String> {
        if max_bytes == 0 || max_rows == 0 || Instant::now() >= deadline {
            return Err("standalone validation limits required".into());
        }
        let root = fs::canonicalize(root).map_err(|_| "selected source root unavailable")?;
        let root_fd = tos_fd_open::open_absolute_directory(&root)
            .map_err(|_| "selected source root is not a no-symlink directory")?;
        let root_stamp = Stamp::from(&root_fd.metadata().map_err(|_| "source root stat failed")?);
        let max_work = max_bytes
            .checked_mul(4)
            .ok_or("standalone work envelope overflow")?;
        Ok(Self {
            max_bytes,
            max_rows,
            bytes: 0,
            rows: 0,
            work: 0,
            max_work,
            deadline,
            root_path: root,
            root_stamp,
            root_fd,
            held: BTreeMap::new(),
            absent: BTreeSet::new(),
        })
    }
    fn tick(&mut self, work: u64) -> Result<(), String> {
        if Instant::now() >= self.deadline {
            return Err("standalone validation deadline exceeded".into());
        }
        self.work = self
            .work
            .checked_add(work)
            .filter(|value| *value <= self.max_work)
            .ok_or("standalone validation work envelope exceeded")?;
        Ok(())
    }
    fn preflight_external(&self, rows: u64, bytes: u64) -> Result<(), String> {
        self.check_deadline()?;
        if rows > self.remaining_rows() {
            return Err("standalone validation row envelope exceeded".into());
        }
        if bytes > self.remaining_bytes() {
            return Err("standalone validation cumulative byte envelope exceeded".into());
        }
        Ok(())
    }
    fn check_deadline(&self) -> Result<(), String> {
        if Instant::now() >= self.deadline {
            return Err("standalone validation deadline exceeded".into());
        }
        Ok(())
    }
    fn hold(&mut self, relative: &str, stamp: Stamp) -> Result<(), String> {
        if self.absent.contains(relative) {
            return Err("optional source member appeared during validation".into());
        }
        if let Some(previous) = self.held.get(relative) {
            if *previous != stamp {
                return Err("source member changed between validation reads".into());
            }
        } else {
            self.held.insert(relative.to_owned(), stamp);
        }
        Ok(())
    }
    fn charge(&mut self, relative: &str, bytes: usize, expected: Stamp) -> Result<(), String> {
        self.tick(bytes as u64 + 1)?;
        self.bytes = self
            .bytes
            .checked_add(bytes as u64)
            .filter(|value| *value <= self.max_bytes)
            .ok_or("standalone validation cumulative byte envelope exceeded")?;
        self.rows = self
            .rows
            .checked_add(1)
            .filter(|value| *value <= self.max_rows)
            .ok_or("standalone validation row envelope exceeded")?;
        let path = self.root_path.join(relative);
        let file = tos_fd_open::open_absolute_regular(&path, bytes as u64)
            .map_err(|_| "source member changed while it was read")?;
        let stamp = Stamp::from(&file.metadata().map_err(|_| "source member stat failed")?);
        if stamp != expected {
            return Err("source member changed after it was read".into());
        }
        self.hold(relative, stamp)
    }
    fn read(&mut self, relative: &str, cap: usize) -> Result<Vec<u8>, String> {
        self.tick(1)?;
        if !safe_relative(relative) {
            return Err("source contract path is unsafe".into());
        }
        let path = self.root_path.join(relative);
        let mut file = tos_fd_open::open_absolute_regular(&path, cap as u64)
            .map_err(|_| "required source member is missing, linked, or oversized")?;
        let before = Stamp::from(&file.metadata().map_err(|_| "source member stat failed")?);
        if before.len > cap as u64 {
            return Err("source member exceeds its selected byte envelope".into());
        }
        // Reserve the observed file length plus the one-byte growth/EOF probe
        // before allocating or reading it. Only the exact bytes read are
        // charged after the post-read identity checks.
        let read_bound = before
            .len
            .checked_add(1)
            .ok_or("source member read bound overflow")?;
        self.preflight_external(1, read_bound)?;
        let mut bytes = Vec::with_capacity(before.len as usize);
        (&mut file)
            .take(read_bound)
            .read_to_end(&mut bytes)
            .map_err(|_| "source member read failed")?;
        let after = Stamp::from(&file.metadata().map_err(|_| "source member stat failed")?);
        let named = fs::symlink_metadata(&path).map_err(|_| "source member name disappeared")?;
        if bytes.len() as u64 != before.len
            || before != after
            || !named.is_file()
            || named.file_type().is_symlink()
            || Stamp::from(&named) != before
        {
            return Err("source member changed while it was read".into());
        }
        self.charge(relative, bytes.len(), after)?;
        Ok(bytes)
    }
    fn account_external(&mut self, rows: u64, bytes: u64) -> Result<(), String> {
        self.tick(
            rows.checked_add(bytes)
                .ok_or("standalone external work envelope overflow")?,
        )?;
        self.rows = self
            .rows
            .checked_add(rows)
            .filter(|value| *value <= self.max_rows)
            .ok_or("standalone validation row envelope exceeded")?;
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .filter(|value| *value <= self.max_bytes)
            .ok_or("standalone validation cumulative byte envelope exceeded")?;
        Ok(())
    }
    fn json(&mut self, relative: &str, cap: usize) -> Result<(Vec<u8>, Value), String> {
        let raw = self.read(relative, cap)?;
        let limits = JsonLimits::new(cap, SOURCE_JSON_DEPTH, SOURCE_JSON_VISITS, 4_300)
            .map_err(|_| "source JSON limits invalid")?;
        parse_json(&raw, JsonMode::PublishedStrict, limits)
            .map_err(|_| format!("invalid strict JSON source: {relative}"))?;
        let value = serde_json::from_slice(&raw)
            .map_err(|_| format!("source JSON representation unsupported: {relative}"))?;
        Ok((raw, value))
    }
    fn verify(&mut self) -> Result<(), String> {
        self.tick(1)?;
        let root_now =
            fs::symlink_metadata(&self.root_path).map_err(|_| "source root disappeared")?;
        if !root_now.is_dir()
            || root_now.file_type().is_symlink()
            || Stamp::from(&root_now) != self.root_stamp
            || Stamp::from(
                &self
                    .root_fd
                    .metadata()
                    .map_err(|_| "source root descriptor lost")?,
            ) != self.root_stamp
        {
            return Err("source root changed during standalone validation".into());
        }
        for (relative, expected) in &self.held {
            self.check_deadline()?;
            let path = self.root_path.join(relative);
            let file = tos_fd_open::open_absolute_regular(&path, expected.len)
                .map_err(|_| "source member changed before validation completed")?;
            if Stamp::from(
                &file
                    .metadata()
                    .map_err(|_| "source member recheck failed")?,
            ) != *expected
            {
                return Err("source member changed before validation completed".into());
            }
        }
        for relative in &self.absent {
            self.check_deadline()?;
            let path = self.root_path.join(relative);
            match fs::symlink_metadata(path) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                _ => return Err("optional source member appeared during validation".into()),
            }
        }
        Ok(())
    }
    fn remaining_bytes(&self) -> u64 {
        self.max_bytes.saturating_sub(self.bytes)
    }
    fn remaining_rows(&self) -> u64 {
        self.max_rows.saturating_sub(self.rows)
    }
}

fn strict_json_value(raw: &[u8], cap: usize) -> Result<Value, String> {
    let limits = JsonLimits::new(cap, SOURCE_JSON_DEPTH, SOURCE_JSON_VISITS, 4_300)
        .map_err(|_| "projection JSON limits invalid")?;
    parse_json(raw, JsonMode::PublishedStrict, limits)
        .map_err(|_| "projection contains invalid strict JSON")?;
    serde_json::from_slice(raw).map_err(|_| "projection JSON representation unsupported".into())
}

fn exact_object_keys(value: &Value, keys: &[&str]) -> bool {
    value.as_object().is_some_and(|object| {
        object.len() == keys.len() && keys.iter().all(|key| object.contains_key(*key))
    })
}

fn projection_collection_name(value: &str) -> bool {
    !value.is_empty()
        && value.split('/').all(|part| {
            let mut bytes = part.bytes();
            bytes.next().is_some_and(|first| first.is_ascii_lowercase())
                && bytes
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        })
}

fn insert_projection_collection(header: &mut Value, name: &str) -> Result<(), String> {
    let mut current = header;
    let mut parts = name.split('/').peekable();
    while let Some(part) = parts.next() {
        let object = current
            .as_object_mut()
            .ok_or("projection collection overlaps scalar metadata")?;
        if parts.peek().is_none() {
            if object.contains_key(part) {
                return Err("projection collection already exists in metadata".into());
            }
            object.insert(part.to_owned(), Value::Null);
        } else {
            let child = object.entry(part.to_owned()).or_insert_with(|| json!({}));
            if !child.is_object() {
                return Err("projection collection overlaps scalar metadata".into());
            }
            current = child;
        }
    }
    Ok(())
}

fn projection_key_field(value: &Value) -> Result<(), String> {
    match value {
        Value::Null => Ok(()),
        Value::String(field) if !field.is_empty() => Ok(()),
        Value::Array(fields)
            if fields
                .iter()
                .all(|field| field.as_str().is_some_and(|value| !value.is_empty())) =>
        {
            Ok(())
        }
        _ => Err("projection collection key field is invalid".into()),
    }
}

fn projection_limits_are_exact(value: &Value) -> bool {
    exact_object_keys(
        value,
        &["root_bytes", "index_bytes", "part_bytes", "key_bytes"],
    ) && value.get("root_bytes").and_then(Value::as_u64) == Some(PARTITION_ROOT_BYTES)
        && value.get("index_bytes").and_then(Value::as_u64) == Some(PARTITION_INDEX_BYTES as u64)
        && value.get("part_bytes").and_then(Value::as_u64) == Some(PARTITION_DATA_BYTES as u64)
        && value.get("key_bytes").and_then(Value::as_u64) == Some(PARTITION_KEY_BYTES as u64)
}

struct ProjectionPart {
    kind: String,
    prefix: String,
    relative: String,
    stored_bytes: usize,
    decoded_bytes: usize,
    count: u64,
    digest: Digest256,
    decoded_digest: Digest256,
}

fn projection_part(
    root_relative: &str,
    descriptor: &Value,
    expected_prefix: &str,
) -> Result<ProjectionPart, String> {
    if !exact_object_keys(
        descriptor,
        &[
            "kind",
            "prefix",
            "path",
            "sha256",
            "size_bytes",
            "decoded_bytes",
            "decoded_sha256",
            "count",
        ],
    ) {
        return Err("projection part descriptor shape is invalid".into());
    }
    let kind = descriptor
        .get("kind")
        .and_then(Value::as_str)
        .filter(|value| matches!(*value, "data" | "index"))
        .ok_or("projection part kind is invalid")?;
    let prefix = descriptor
        .get("prefix")
        .and_then(Value::as_str)
        .ok_or("projection part prefix is invalid")?;
    if prefix != expected_prefix
        || prefix.len() > 64
        || !prefix
            .bytes()
            .all(|value| value.is_ascii_digit() || (b'a'..=b'f').contains(&value))
    {
        return Err("projection part prefix is invalid".into());
    }
    let digest_text = descriptor
        .get("sha256")
        .and_then(Value::as_str)
        .filter(|value| value.len() == 64)
        .ok_or("projection part digest is invalid")?;
    let digest =
        Digest256::from_hex(digest_text).map_err(|_| "projection part digest is invalid")?;
    let decoded_digest_text = descriptor
        .get("decoded_sha256")
        .and_then(Value::as_str)
        .filter(|value| value.len() == 64)
        .ok_or("projection decoded digest is invalid")?;
    let decoded_digest = Digest256::from_hex(decoded_digest_text)
        .map_err(|_| "projection decoded digest is invalid")?;
    let stored_u64 = descriptor
        .get("size_bytes")
        .and_then(Value::as_u64)
        .ok_or("projection stored byte count is invalid")?;
    let decoded_u64 = descriptor
        .get("decoded_bytes")
        .and_then(Value::as_u64)
        .ok_or("projection decoded byte count is invalid")?;
    let count = descriptor
        .get("count")
        .and_then(Value::as_u64)
        .ok_or("projection item count is invalid")?;
    let stored_bytes = usize::try_from(stored_u64)
        .map_err(|_| "projection stored byte count exceeds platform size")?;
    let decoded_bytes = usize::try_from(decoded_u64)
        .map_err(|_| "projection decoded byte count exceeds platform size")?;
    let decoded_cap = if kind == "index" {
        PARTITION_INDEX_BYTES
    } else {
        PARTITION_DATA_BYTES
    };
    if decoded_bytes > decoded_cap
        || stored_bytes > decoded_cap.saturating_add(PARTITION_STORED_OVERHEAD)
    {
        return Err("projection part exceeds its format byte bounds".into());
    }
    let suffix = if kind == "index" {
        ".index.json"
    } else {
        ".jsonl.gz"
    };
    let stem = Path::new(root_relative)
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or("partition root filename is invalid")?;
    let parent = Path::new(root_relative)
        .parent()
        .unwrap_or_else(|| Path::new(""));
    let descriptor_relative = PathBuf::from(format!("{stem}.parts"))
        .join(&digest_text[..2])
        .join(format!("{digest_text}{suffix}"));
    let descriptor_relative = descriptor_relative
        .to_str()
        .filter(|value| safe_relative(value))
        .ok_or("projection part path is invalid")?;
    if descriptor.get("path").and_then(Value::as_str) != Some(descriptor_relative) {
        return Err("projection part path is outside the exact content-addressed namespace".into());
    }
    let physical_path = parent.join(descriptor_relative);
    let expected_relative = physical_path
        .to_str()
        .filter(|value| safe_relative(value))
        .ok_or("projection physical part path is invalid")?;
    Ok(ProjectionPart {
        kind: kind.to_owned(),
        prefix: prefix.to_owned(),
        relative: expected_relative.to_owned(),
        stored_bytes,
        decoded_bytes,
        count,
        digest,
        decoded_digest,
    })
}

fn projection_record_key_valid(
    value: &Value,
    key_field: &Value,
    key: &str,
    sequence_count: u64,
) -> bool {
    match key_field {
        Value::Null => true,
        Value::String(field) => {
            value
                .as_object()
                .and_then(|row| row.get(field))
                .and_then(Value::as_str)
                == Some(key)
        }
        Value::Array(fields) if fields.is_empty() => {
            key.len() == 20
                && key.bytes().all(|byte| byte.is_ascii_digit())
                && key
                    .parse::<u64>()
                    .is_ok_and(|position| position < sequence_count)
        }
        Value::Array(fields) => {
            let Some(row) = value.as_object() else {
                return false;
            };
            let mut values = Vec::with_capacity(fields.len());
            for field in fields {
                let Some(value) = field
                    .as_str()
                    .and_then(|field| row.get(field))
                    .and_then(Value::as_str)
                    .filter(|value| {
                        !value.is_empty() && value.as_bytes().len() <= PARTITION_KEY_BYTES
                    })
                else {
                    return false;
                };
                values.push(value);
            }
            serde_json::to_string(&values).is_ok_and(|value| value == key)
        }
        _ => false,
    }
}

fn for_each_projection_line(
    raw: &[u8],
    mut visit: impl FnMut(&[u8]) -> Result<(), String>,
) -> Result<(), String> {
    let mut start = 0usize;
    let mut at = 0usize;
    while at < raw.len() {
        if matches!(raw[at], b'\n' | b'\r') {
            visit(&raw[start..at])?;
            if raw[at] == b'\r' && raw.get(at + 1) == Some(&b'\n') {
                at += 2;
            } else {
                at += 1;
            }
            start = at;
        } else {
            at += 1;
        }
    }
    if start < raw.len() {
        visit(&raw[start..])?;
    }
    Ok(())
}

fn walk_projection_part(
    meter: &mut Meter,
    root_relative: &str,
    collection_name: &str,
    key_field: &Value,
    descriptor: &Value,
    expected_prefix: &str,
    sequence_count: Option<u64>,
    seen: &mut BTreeSet<String>,
    closure_members: &mut usize,
) -> Result<u64, String> {
    meter.tick(1)?;
    let part = projection_part(root_relative, descriptor, expected_prefix)?;
    if seen.insert(part.relative.clone()) {
        // `closure_paths` yields each physical path once, while continuing to
        // traverse each collection reference and revalidate its row identity.
        *closure_members = closure_members
            .checked_add(1)
            .ok_or("projection closure member count overflow")?;
    }
    let stored = meter.read(&part.relative, part.stored_bytes)?;
    if stored.len() != part.stored_bytes || Digest256::of_bytes(&stored) != part.digest {
        return Err("projection stored part identity mismatch".into());
    }
    let decoded_probe = (part.decoded_bytes as u64)
        .checked_add(1)
        .ok_or("projection decoded read bound overflow")?;
    meter.preflight_external(0, decoded_probe)?;
    let decoded = if part.kind == "index" {
        if stored.len() != part.decoded_bytes {
            return Err("projection index size differs from decoded size".into());
        }
        stored
    } else {
        let mut decoder = MultiGzDecoder::new(stored.as_slice());
        let mut decoded = Vec::with_capacity(part.decoded_bytes);
        decoder
            .by_ref()
            .take(part.decoded_bytes as u64 + 1)
            .read_to_end(&mut decoded)
            .map_err(|_| "projection data part gzip stream is invalid")?;
        decoded
    };
    if decoded.len() != part.decoded_bytes || Digest256::of_bytes(&decoded) != part.decoded_digest {
        return Err("projection decoded part identity mismatch".into());
    }
    meter.account_external(0, part.decoded_bytes as u64)?;
    let sequence_count = sequence_count.unwrap_or(part.count);
    if part.kind == "index" {
        if part.prefix.len() >= 64 {
            return Err("projection partition tree exceeds prefix depth".into());
        }
        let index = strict_json_value(&decoded, PARTITION_INDEX_BYTES)?;
        if !exact_object_keys(&index, &["schema_version", "prefix", "count", "children"])
            || index.get("schema_version").and_then(Value::as_str)
                != Some("tos_projection_partition_index_v1")
            || index.get("prefix").and_then(Value::as_str) != Some(part.prefix.as_str())
            || index.get("count").and_then(Value::as_u64) != Some(part.count)
        {
            return Err("projection partition directory identity is invalid".into());
        }
        let children = index
            .get("children")
            .and_then(Value::as_object)
            .filter(|children| !children.is_empty())
            .ok_or("projection partition directory children are invalid")?;
        let mut child_names: Vec<_> = children.keys().collect();
        child_names.sort();
        let mut count = 0u64;
        for digit in child_names {
            if digit.len() != 1
                || !digit
                    .bytes()
                    .all(|value| value.is_ascii_digit() || (b'a'..=b'f').contains(&value))
            {
                return Err("projection partition branch digit is invalid".into());
            }
            let child_prefix = format!("{}{digit}", part.prefix);
            let child = children
                .get(digit)
                .ok_or("projection partition child disappeared")?;
            count = count
                .checked_add(walk_projection_part(
                    meter,
                    root_relative,
                    collection_name,
                    key_field,
                    child,
                    &child_prefix,
                    Some(sequence_count),
                    seen,
                    closure_members,
                )?)
                .ok_or("projection partition count overflow")?;
        }
        if count != part.count {
            return Err("projection partition child count mismatch".into());
        }
        Ok(count)
    } else {
        let mut count = 0u64;
        let mut previous: Option<String> = None;
        for_each_projection_line(&decoded, |line| {
            meter.check_deadline()?;
            meter.account_external(1, 0)?;
            if line.len() > PARTITION_DATA_BYTES {
                return Err("projection record exceeds its part byte bound".into());
            }
            let record = strict_json_value(line, PARTITION_DATA_BYTES)?;
            if !exact_object_keys(&record, &["key", "value"]) {
                return Err("projection row shape is invalid".into());
            }
            let key = record
                .get("key")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty() && value.as_bytes().len() <= PARTITION_KEY_BYTES)
                .ok_or("projection row key is invalid")?;
            if previous.as_deref().is_some_and(|prior| key <= prior) {
                return Err("projection partition keys are duplicate or unsorted".into());
            }
            previous = Some(key.to_owned());
            if !Digest256::of_bytes(key.as_bytes())
                .to_hex()
                .starts_with(&part.prefix)
            {
                return Err("projection row is in the wrong partition".into());
            }
            if !projection_record_key_valid(
                record
                    .get("value")
                    .ok_or("projection row value is missing")?,
                key_field,
                key,
                sequence_count,
            ) {
                return Err("projection row key differs from its collection identity".into());
            }
            count = count
                .checked_add(1)
                .ok_or("projection data row count overflow")?;
            if count > part.count {
                return Err("projection data row count exceeds its descriptor".into());
            }
            Ok(())
        })?;
        if count != part.count {
            return Err("projection data row count mismatch".into());
        }
        let _ = collection_name;
        Ok(count)
    }
}

fn verify_partitioned_projection(
    meter: &mut Meter,
    root_relative: &str,
    root_raw: &[u8],
) -> Result<usize, String> {
    // Keep partition mechanics computational only: the allowlist remains the
    // source of candidate paths, and Meter owns every read and final recheck.
    meter.tick(1)?;
    if root_raw.len() > PARTITION_ROOT_BYTES as usize {
        return Err("partition root exceeds its format byte bound".into());
    }
    let root = strict_json_value(root_raw, PARTITION_ROOT_BYTES as usize)?;
    if !exact_object_keys(
        &root,
        &[
            "schema_version",
            "logical_schema",
            "header",
            "limits",
            "collections",
        ],
    ) || root.get("schema_version").and_then(Value::as_str)
        != Some("tos_partitioned_projection_v1")
        || !projection_limits_are_exact(root.get("limits").ok_or("partition limits are missing")?)
    {
        return Err("partition root shape or limits are invalid".into());
    }
    let header = root
        .get("header")
        .filter(|header| header.is_object())
        .ok_or("partition metadata header is invalid")?;
    let logical_schema = root
        .get("logical_schema")
        .and_then(Value::as_str)
        .filter(|schema| !schema.is_empty())
        .ok_or("partition logical schema is invalid")?;
    if header.get("schema_version").and_then(Value::as_str) != Some(logical_schema) {
        return Err("partition logical schema differs from metadata header".into());
    }
    let collections = root
        .get("collections")
        .and_then(Value::as_object)
        .filter(|collections| !collections.is_empty())
        .ok_or("partition collection declarations are invalid")?;
    let mut checked_header = header.clone();
    let mut collection_names: Vec<_> = collections.keys().collect();
    collection_names.sort();
    let mut seen = BTreeSet::from([root_relative.to_owned()]);
    let mut closure_members = 1usize;
    for name in collection_names {
        meter.tick(1)?;
        if !projection_collection_name(name) {
            return Err("partition collection name is invalid".into());
        }
        let spec = collections
            .get(name)
            .ok_or("partition collection disappeared")?;
        if !exact_object_keys(spec, &["key_field", "order_fields", "root"]) {
            return Err("partition collection descriptor shape is invalid".into());
        }
        let key_field = spec
            .get("key_field")
            .ok_or("partition key field is missing")?;
        projection_key_field(key_field)?;
        let order_fields = spec
            .get("order_fields")
            .and_then(Value::as_array)
            .ok_or("partition order fields are invalid")?;
        if order_fields
            .iter()
            .any(|field| field.as_str().is_none_or(str::is_empty))
            || (key_field.as_array().is_some_and(Vec::is_empty) && !order_fields.is_empty())
        {
            return Err("partition order fields are invalid".into());
        }
        insert_projection_collection(&mut checked_header, name)?;
        walk_projection_part(
            meter,
            root_relative,
            name,
            key_field,
            spec.get("root")
                .ok_or("partition collection root is missing")?,
            "",
            None,
            &mut seen,
            &mut closure_members,
        )?;
    }
    // `checked_header` mirrors ProjectionReader's collision check while
    // keeping the manifest's logical metadata private to this diagnostic.
    let _ = checked_header;
    if seen.is_empty() {
        return Err("partition closure is empty".into());
    }
    Ok(closure_members)
}

fn safe_relative(value: &str) -> bool {
    !value.is_empty()
        && !Path::new(value).is_absolute()
        && Path::new(value)
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
        && !value.contains(['*', '?', '[', ']'])
}
fn string_set(value: &Value, field: &str) -> Result<BTreeSet<String>, String> {
    let rows = value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("source contract {field} must be an array"))?;
    let mut result = BTreeSet::new();
    for item in rows {
        let text = item
            .as_str()
            .filter(|text| !text.is_empty())
            .ok_or_else(|| format!("source contract {field} must contain non-empty strings"))?;
        if !result.insert(text.to_owned()) {
            return Err(format!("source contract {field} contains duplicates"));
        }
    }
    Ok(result)
}
fn expected_set(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}
fn exact_ids(
    document: &Value,
    array_name: &str,
    key: &str,
    expected: &[&str],
) -> Result<(), String> {
    let rows = document
        .get(array_name)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("source contract {array_name} must be an array"))?;
    let mut ids = BTreeSet::new();
    for row in rows {
        let value = row
            .get(key)
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("source contract {array_name} identity is missing"))?;
        if !ids.insert(value.to_owned()) {
            return Err(format!("source contract {array_name} repeats an identity"));
        }
    }
    if ids != expected_set(expected) {
        return Err(format!("source contract {array_name} identity set drift"));
    }
    Ok(())
}
fn parse_schema_id(raw: &[u8], cap: usize) -> Result<String, String> {
    let limits = JsonLimits::new(cap, SOURCE_JSON_DEPTH, SOURCE_JSON_VISITS, 4_300)
        .map_err(|_| "schema JSON limits invalid")?;
    let parsed = parse_json(raw, JsonMode::PublishedStrict, limits)
        .map_err(|_| "software schema source is invalid JSON")?;
    parsed
        .root()
        .object_get("$id")
        .and_then(JsonValue::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| "software schema source has no $id".into())
}
fn schema_set_digest(resources: &BTreeMap<String, Vec<u8>>) -> Digest256 {
    let mut hash = Digest256Hasher::new();
    hash.update(b"tos-schema-set-v1\0");
    for (uri, raw) in resources {
        hash.update(&(uri.len() as u64).to_be_bytes());
        hash.update(uri.as_bytes());
        hash.update(Digest256::of_bytes(raw).as_bytes());
    }
    hash.finalize()
}
fn json_limits(max_bytes: usize) -> Result<JsonLimits, String> {
    JsonLimits::new(max_bytes, SOURCE_JSON_DEPTH, SOURCE_JSON_VISITS, 4_300)
        .map_err(|_| "standalone JSON limits invalid".into())
}
fn embedded_contracts_match(
    meter: &mut Meter,
    schemas: &tos_query::source_diagnostic_validation::SourceDiagnosticSchemas,
) -> Result<(), String> {
    let mut resources = BTreeMap::new();
    for path in ALL_PROGRAM_SCHEMAS {
        let raw = meter.read(path, SOFTWARE_FILE_BYTES as usize)?;
        let uri = parse_schema_id(&raw, SOFTWARE_FILE_BYTES as usize)?;
        if resources.insert(uri, raw).is_some() {
            return Err("source schema IDs must be unique".into());
        }
    }
    if resources.len() != TEN_ACCESS_SCHEMAS.len() + NATIVE_ADDITIONAL_SCHEMAS.len()
        || schema_set_digest(&resources) != schemas.schema_set_sha256()
    {
        return Err("source schema resources differ from the compiled native schema set".into());
    }
    Ok(())
}

struct SoftwareFacts {
    schemas: tos_query::source_diagnostic_validation::SourceDiagnosticSchemas,
    allowlist: Value,
    allowlist_raw: Vec<u8>,
    schema_bytes: usize,
}
fn validate_software_contracts(meter: &mut Meter) -> Result<SoftwareFacts, String> {
    let mut documents = BTreeMap::new();
    for path in SOFTWARE_CONTRACT_PATHS {
        let (_, value) = meter.json(path, SOFTWARE_FILE_BYTES as usize)?;
        documents.insert(*path, value);
    }
    let schemas =
        tos_query::source_diagnostic_validation::SourceDiagnosticSchemas::from_program_contracts(
            SCHEMA_RESOURCE_BYTES,
        )
        .map_err(|_| "compiled Draft 2020-12 software schema set is invalid")?;
    embedded_contracts_match(meter, &schemas)?;

    for path in TEN_ACCESS_SCHEMAS {
        if !documents.contains_key(path) {
            let (_, value) = meter.json(path, SOFTWARE_FILE_BYTES as usize)?;
            documents.insert(*path, value);
        }
    }
    let runtime = &documents["access/contracts/runtime-manifest.v1.json"];
    if runtime.get("authority_owner").and_then(Value::as_str) != Some("Tree-of-Sophia") {
        return Err("runtime authority owner must be Tree-of-Sophia".into());
    }
    let profiles = runtime
        .get("runtime_profiles")
        .and_then(Value::as_array)
        .ok_or("runtime manifest profiles are missing")?;
    if !profiles.iter().any(|item| {
        item.get("profile_id").and_then(Value::as_str) == Some("standalone")
            && item.get("requires_abyssos") == Some(&Value::Bool(false))
    }) {
        return Err("standalone profile must not require AbyssOS".into());
    }
    let components = runtime
        .get("components")
        .and_then(Value::as_array)
        .ok_or("runtime manifest components are missing")?;
    if !components.iter().any(|item| {
        item.get("component_id").and_then(Value::as_str) == Some("knowledge-lens-engine")
            && item.get("required") == Some(&Value::Bool(true))
            && item.get("posture").and_then(Value::as_str) == Some("read-only-derived-composition")
    }) {
        return Err("runtime manifest must require the read-only knowledge lens engine".into());
    }
    if runtime.get("integration_posture")
        != Some(&json!({
            "state":"paused", "scope":["abyssos"], "default_profile":"standalone",
            "external_activation":"disabled", "unfreeze_requires":"explicit ToS operator command"
        }))
    {
        return Err("AbyssOS integration posture must remain explicitly paused".into());
    }
    if documents["access/profiles/abyssos.v1.json"]
        .get("availability")
        .and_then(Value::as_str)
        != Some("paused")
    {
        return Err("AbyssOS access profile must remain paused".into());
    }
    let migration = &documents["access/contracts/web-actions.v1.json"];
    if migration.get("status").and_then(Value::as_str) != Some("superseded")
        || string_set(migration, "superseded_by")?
            != expected_set(&["query-operations.v1.json", "page-commands.v1.json"])
    {
        return Err("web action migration marker must route to split contracts".into());
    }
    exact_ids(
        &documents["access/contracts/query-operations.v1.json"],
        "operations",
        "operation_id",
        QUERY_OPERATIONS,
    )?;
    let api = &documents["access/contracts/knowledge-api.v1.json"];
    let api_rows = api
        .get("operations")
        .and_then(Value::as_array)
        .ok_or("knowledge API operation list is missing")?;
    let mut api_ids = BTreeSet::new();
    for row in api_rows {
        let id = row
            .get("operation_id")
            .and_then(Value::as_str)
            .ok_or("knowledge API operation ID missing")?;
        if !api_ids.insert(id.to_owned()) {
            return Err("knowledge API operation IDs are not unique".into());
        }
    }
    if api_ids != expected_set(KNOWLEDGE_OPERATIONS) {
        return Err("knowledge operation contract drift".into());
    }
    let compile_operation = api_rows
        .iter()
        .find(|row| row.get("operation_id").and_then(Value::as_str) == Some("tos.lens.compile"))
        .ok_or("knowledge lens compile operation is missing")?;
    if compile_operation
        .pointer("/http/method")
        .and_then(Value::as_str)
        != Some("POST")
        || !compile_operation
            .get("post_semantics")
            .and_then(Value::as_str)
            .is_some_and(|value| value.contains("creates no server state"))
    {
        return Err("knowledge lens compile must remain a read-only structured query".into());
    }
    let epistemic = &documents["access/contracts/epistemic-packet.v1.schema.json"];
    if epistemic
        .pointer("/properties/schema/const")
        .and_then(Value::as_str)
        != Some("tos_philosophy_epistemic_packet_v1")
        || epistemic.pointer("/properties/authority_boundary/properties")
            != Some(&json!({
                "is_source":{"const":false}, "is_canon":{"const":false},
                "is_semantic_truth":{"const":false}, "is_rights_clearance":{"const":false}
            }))
    {
        return Err("epistemic packet authority boundary must fail closed".into());
    }
    if documents["ToS/contracts/epistemic-evidence-projection.schema.json"]
        .pointer("/properties/schema_version/const")
        .and_then(Value::as_str)
        != Some("tos_epistemic_evidence_projection_v1")
    {
        return Err("Evidence Lens projection schema identity drift".into());
    }
    if documents["access/contracts/evidence-lens-packet.v1.schema.json"]
        .pointer("/properties/schema/const")
        .and_then(Value::as_str)
        != Some("tos_evidence_lens_packet_v1")
    {
        return Err("Evidence Lens packet schema identity drift".into());
    }
    exact_ids(
        &documents["access/contracts/page-commands.v1.json"],
        "commands",
        "command_id",
        PAGE_COMMANDS,
    )?;
    let workspace = &documents["access/contracts/research-workspace.v1.schema.json"];
    if workspace
        .pointer("/properties/schema/const")
        .and_then(Value::as_str)
        != Some("tos_research_workspace_session_v1")
        || workspace.pointer("/$defs/posture/properties")
            != Some(&json!({
                "session_hypothesis":{"const":true}, "source":{"const":false},
                "reviewed":{"const":false}, "canon":{"const":false}
            }))
    {
        return Err("research hypotheses must remain explicitly outside ToS authority".into());
    }
    let exploration_capabilities = crate::exploration_contracts::runtime_capabilities(None);
    if exploration_capabilities
        .object_get("runtime")
        .and_then(JsonValue::as_str)
        != Some("local")
        || exploration_capabilities.object_get("restart_survival") != Some(&JsonValue::Bool(false))
    {
        return Err("local exploration capability contract drift".into());
    }
    let (allowlist_raw, allowlist) = {
        let path = "access/contracts/runtime-data.v1.json";
        let raw = meter.read(path, SOFTWARE_FILE_BYTES as usize)?;
        if raw != tos_compiler::native_snapshot_manifest::RUNTIME_DATA_DECLARATION {
            return Err("runtime data declaration differs from the compiled product input".into());
        }
        let limits = json_limits(SOFTWARE_FILE_BYTES as usize)?;
        parse_json(&raw, JsonMode::PublishedStrict, limits)
            .map_err(|_| "runtime data allowlist JSON is invalid")?;
        let value =
            serde_json::from_slice(&raw).map_err(|_| "runtime data allowlist shape invalid")?;
        (raw, value)
    };
    let subjects = allowlist
        .get("subjects")
        .and_then(Value::as_array)
        .ok_or("runtime allowlist subjects must be a list")?;
    let source_paths: BTreeSet<String> = subjects
        .iter()
        .map(|item| {
            item.get("source_path")
                .and_then(Value::as_str)
                .filter(|path| safe_relative(path))
                .map(ToOwned::to_owned)
                .ok_or("runtime allowlist source path is invalid")
        })
        .collect::<Result<_, _>>()?;
    if !expected_set(REQUIRED_RUNTIME_SUBJECTS).is_subset(&source_paths) {
        return Err("runtime allowlist is missing required constructor inputs".into());
    }
    if source_paths
        .iter()
        .any(|path| path.contains("lexical-search") || path.contains("/payload/"))
    {
        return Err("runtime allowlist admits an explicitly excluded subject".into());
    }
    Ok(SoftwareFacts {
        schema_bytes: schemas.resource_bytes(),
        schemas,
        allowlist,
        allowlist_raw,
    })
}

fn scan_source(meter: &mut Meter) -> Result<(), String> {
    const ROOTS: &[&str] = &[
        "access/src/tos_access",
        "access/contracts",
        "access/profiles",
        "access/packaging",
        "access/web/src",
    ];
    let mut paths = vec![
        "access/pyproject.toml".to_owned(),
        "access/web/index.html".to_owned(),
    ];
    let mut visited_entries = 0u64;
    for relative in ROOTS {
        let start = meter.root_path.join(relative);
        if !start.exists() {
            continue;
        }
        let mut pending = vec![start];
        while let Some(directory) = pending.pop() {
            meter.tick(1)?;
            let metadata = fs::symlink_metadata(&directory)
                .map_err(|_| "owned source directory disappeared")?;
            if metadata.file_type().is_symlink() {
                return Err("owned source tree contains a symlink".into());
            }
            if !metadata.is_dir() {
                continue;
            }
            let entries =
                fs::read_dir(&directory).map_err(|_| "owned source directory is unreadable")?;
            let mut children = Vec::new();
            for entry in entries {
                meter.tick(1)?;
                visited_entries = visited_entries
                    .checked_add(1)
                    .filter(|count| *count <= meter.max_rows)
                    .ok_or("owned source entry envelope exceeded")?;
                children.push(entry.map_err(|_| "owned source entry unreadable")?.path());
            }
            children.sort();
            for path in children.into_iter().rev() {
                let meta =
                    fs::symlink_metadata(&path).map_err(|_| "owned source entry disappeared")?;
                if meta.is_dir() && !meta.file_type().is_symlink() {
                    pending.push(path);
                } else if meta.is_file() && !meta.file_type().is_symlink() {
                    paths.push(
                        path.strip_prefix(&meter.root_path)
                            .map_err(|_| "owned source escaped selected root")?
                            .to_string_lossy()
                            .replace('\\', "/"),
                    );
                } else if meta.file_type().is_symlink() {
                    return Err("owned source tree contains a symlink".into());
                }
            }
        }
    }
    paths.sort();
    paths.dedup();
    for relative in paths {
        if relative.split('/').any(|part| part == "runtime_data") {
            continue;
        }
        let path = Path::new(&relative);
        let suffix = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("");
        if !matches!(
            suffix,
            "py" | "json" | "toml" | "ts" | "js" | "mjs" | "html"
        ) {
            continue;
        }
        let payload = meter.read(&relative, SOFTWARE_FILE_BYTES as usize)?;
        if BLOCKED_CODE_MARKERS
            .iter()
            .any(|marker| payload.windows(marker.len()).any(|chunk| chunk == *marker))
        {
            return Err("hard-coded host path in owned access source".into());
        }
    }
    Ok(())
}

fn validate_runtime_allowlist(
    meter: &mut Meter,
    allowlist: &Value,
) -> Result<(Vec<String>, usize), String> {
    let subjects = allowlist
        .get("subjects")
        .and_then(Value::as_array)
        .ok_or("runtime allowlist subjects must be a list")?;
    let mut by_id = BTreeMap::<String, &Value>::new();
    let mut source_paths = BTreeSet::new();
    for item in subjects {
        let id = item
            .get("subject_id")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or("runtime allowlist subject ID is invalid")?;
        let path = item
            .get("source_path")
            .and_then(Value::as_str)
            .filter(|value| safe_relative(value))
            .ok_or("runtime allowlist source path is invalid")?;
        if by_id.insert(id.to_owned(), item).is_some() {
            return Err("runtime allowlist subject IDs repeat".into());
        }
        source_paths.insert(path.to_owned());
        if item
            .get("required")
            .is_some_and(|value| !value.is_boolean())
        {
            return Err("runtime allowlist required flag is not boolean".into());
        }
    }
    let compiled = allowlist
        .get("compiled_subjects")
        .and_then(Value::as_array)
        .ok_or("runtime allowlist compiled_subjects must be a list")?;
    let compiled_keys = expected_set(&[
        "subject_id",
        "output_path",
        "builder_module",
        "required_when",
        "input_subject_ids",
        "consumer_roles",
        "identity_rule",
        "authority",
    ]);
    let mut seen_ids = BTreeSet::new();
    let mut seen_outputs = BTreeSet::new();
    let mut compiled_by_id = Vec::new();
    for spec in compiled {
        let object = spec
            .as_object()
            .ok_or("compiled subject must be an object")?;
        if object.keys().cloned().collect::<BTreeSet<_>>() != compiled_keys {
            return Err("compiled subject field shape drift".into());
        }
        let id = spec
            .get("subject_id")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or("compiled subject ID invalid")?;
        if by_id.contains_key(id) || !seen_ids.insert(id.to_owned()) {
            return Err("compiled subject ID collides or repeats".into());
        }
        let output = spec
            .get("output_path")
            .and_then(Value::as_str)
            .filter(|value| safe_relative(value))
            .ok_or("compiled subject output path invalid")?;
        if source_paths.contains(output) || !seen_outputs.insert(output.to_owned()) {
            return Err("compiled subject output path collides or repeats".into());
        }
        let module = spec
            .get("builder_module")
            .and_then(Value::as_str)
            .filter(|value| {
                !value.is_empty()
                    && value.split('.').all(|part| {
                        let mut chars = part.chars();
                        chars
                            .next()
                            .is_some_and(|first| first == '_' || first.is_alphabetic())
                            && chars.all(|ch| ch == '_' || ch.is_alphanumeric())
                    })
            })
            .ok_or("compiled subject builder module invalid")?;
        let _ = module;
        if spec.get("required_when").and_then(Value::as_str)
            != Some("partitioned_projection_inputs")
        {
            return Err("compiled subject required_when unsupported".into());
        }
        let inputs = unique_nonempty_strings(spec, "input_subject_ids")?;
        if inputs.is_empty() || inputs.iter().any(|input| !by_id.contains_key(input)) {
            return Err("compiled subject inputs are empty or unknown".into());
        }
        if id != COMPILED_QUERY_STORE_ID
            || output != COMPILED_QUERY_STORE_OUTPUT
            || module != COMPILED_QUERY_STORE_BUILDER
            || inputs != expected_set(CAPTURED_INPUT_SUBJECTS)
        {
            return Err("compiled subject differs from the native captured-input owner".into());
        }
        if unique_nonempty_strings(spec, "consumer_roles")?.is_empty() {
            return Err("compiled subject consumer roles are empty".into());
        }
        for field in ["identity_rule", "authority"] {
            if !spec
                .get(field)
                .and_then(Value::as_str)
                .is_some_and(|value| !value.trim().is_empty())
            {
                return Err("compiled subject identity or authority is empty".into());
            }
        }
        compiled_by_id.push((id.to_owned(), inputs));
    }
    let mut partitioned = Vec::new();
    let mut closure_member_count = 0usize;
    for (id, item) in &by_id {
        let relative = item["source_path"]
            .as_str()
            .ok_or("source subject path missing")?;
        let path = meter.root_path.join(relative);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => metadata,
            Ok(_) => return Err("runtime subject is not a regular unlinked file".into()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if item.get("required").and_then(Value::as_bool) == Some(true) {
                    return Err("required runtime subject is missing".into());
                }
                meter.absent.insert(relative.to_owned());
                continue;
            }
            Err(_) => return Err("runtime subject metadata is unavailable".into()),
        };
        if metadata.len() <= PARTITION_ROOT_BYTES {
            let raw = meter.read(relative, PARTITION_ROOT_BYTES as usize)?;
            // Keep the maintained Python marker semantics: ordinary files are
            // not parsed here; only a successfully decoded root marker selects
            // the owner partition-closure route.
            let marker_limits = JsonLimits::new(
                PARTITION_ROOT_BYTES as usize,
                128,
                SOURCE_JSON_VISITS,
                4_300,
            )
            .map_err(|_| "partition marker JSON limits invalid")?;
            // Python's maintained marker probe uses ordinary json.loads:
            // duplicate members are last-wins and NaN/Infinity are accepted
            // here, then the strict owner reader rejects them for a marked
            // root. Preserve that route distinction without making the root
            // publishable under this compatibility parse.
            let marked = parse_json(&raw, JsonMode::LegacyPythonObserved, marker_limits)
                .ok()
                .and_then(|value| {
                    value
                        .root()
                        .object_get("schema_version")
                        .and_then(JsonValue::as_str)
                        .map(ToOwned::to_owned)
                })
                .is_some_and(|value| value == "tos_partitioned_projection_v1");
            if marked {
                closure_member_count = closure_member_count
                    .checked_add(verify_partitioned_projection(meter, relative, &raw)?)
                    .ok_or("partition closure member count overflow")?;
                partitioned.push(id.to_owned());
            }
        } else {
            meter.tick(1)?;
            let file = tos_fd_open::open_absolute_regular(&path, u64::MAX)
                .map_err(|_| "runtime subject is not a safe regular file")?;
            let stamp = Stamp::from(&file.metadata().map_err(|_| "runtime subject stat failed")?);
            meter.hold(relative, stamp)?;
        }
    }
    if !partitioned.is_empty() {
        let policy = allowlist
            .get("partitioned_subject_policy")
            .and_then(Value::as_object)
            .ok_or("partitioned inputs require an exact subject policy")?;
        let policy_keys: BTreeSet<String> = [
            "format",
            "inclusion",
            "discovery_glob_allowed",
            "source_authority",
        ]
        .iter()
        .map(|value| (*value).to_owned())
        .collect();
        if policy.keys().cloned().collect::<BTreeSet<_>>() != policy_keys
            || policy.get("format").and_then(Value::as_str) != Some("tos_partitioned_projection_v1")
            || policy.get("inclusion").and_then(Value::as_str)
                != Some("exact-verified-manifest-closure")
            || policy.get("discovery_glob_allowed") != Some(&Value::Bool(false))
            || !policy
                .get("source_authority")
                .and_then(Value::as_str)
                .is_some_and(|value| !value.trim().is_empty())
        {
            return Err("partitioned subject policy differs from the maintained contract".into());
        }
        let active: Vec<_> = compiled_by_id
            .iter()
            .filter(|(_, inputs)| inputs.iter().any(|input| partitioned.contains(input)))
            .collect();
        if active.len() != 1 {
            return Err("partitioned inputs require exactly one active compiled subject".into());
        }
    }
    Ok((partitioned, closure_member_count))
}
fn unique_nonempty_strings(value: &Value, field: &str) -> Result<BTreeSet<String>, String> {
    let array = value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("compiled subject {field} must be an array"))?;
    let mut result = BTreeSet::new();
    for item in array {
        let value = item
            .as_str()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("compiled subject {field} item invalid"))?;
        if !result.insert(value.to_owned()) {
            return Err(format!("compiled subject {field} repeats a value"));
        }
    }
    Ok(result)
}

struct DeadlineAbort(Instant);
impl AbortProbe for DeadlineAbort {
    fn reason(&self) -> Option<AbortReason> {
        (Instant::now() >= self.0).then_some(AbortReason::DeadlineExceeded)
    }
}
struct QueryFacts {
    source_revision: String,
    node_count: u64,
    relation_count: u64,
    schema_nodes_checked: u64,
    schema_relations_checked: u64,
    query_rows: u64,
    query_decoded_bytes: u64,
    query_response_bytes: usize,
    native_contract_count: usize,
    registry_bytes: usize,
}
fn stage_predicates(
    capture: &tos_compiler::PublicCapture,
    isolation: &tos_compiler::private_tmpfs_stage::PrivateTmpfsStageIsolation,
    limits: tos_compiler::native_snapshot::NativeSnapshotLimits,
    declaration_raw: &[u8],
    schemas: &tos_query::source_diagnostic_validation::SourceDiagnosticSchemas,
    max_rows: u64,
    row_bytes: usize,
    decoded_bytes: u64,
    response_bytes: usize,
    deadline: Instant,
) -> Result<QueryFacts, String> {
    let candidate = fresh_stage_path(isolation.root(), "tos-standalone-candidate")?;
    let mut facts = None;
    tos_compiler::native_snapshot::validate_native_snapshot_from_capture(
        capture,
        &candidate,
        declaration_raw,
        isolation,
        limits,
        |stage, full, header, source_revision, descriptor_raw| {
            let initial_budget = tos_query::source_diagnostic_validation::SourceDiagnosticQueryLimits::from_envelope(
                max_rows,
                row_bytes,
                decoded_bytes,
                response_bytes,
                limits.capture.max_sql_vm_steps,
                Arc::new(DeadlineAbort(deadline)),
            )
            .map_err(|_| tos_compiler::Error::Budget("standalone query envelope"))?;
            let read = initial_budget.read_budget();
            let source_header = header
                .get("counts")
                .and_then(Value::as_object)
                .ok_or(tos_compiler::Error::Invalid("standalone knowledge counts"))?;
            validate_semantic_header(source_header, full.seal.node_count, full.seal.relation_count)
                .map_err(|_| tos_compiler::Error::Invalid("standalone semantic/display counts"))?;

            let entity_path = "ToS/doctrine/semantic-interchange/entity-types.v1.json";
            let relation_path = "ToS/doctrine/semantic-interchange/relation-types.v1.json";
            let entity = capture
                .read_retained_input(entity_path, row_bytes)
                .map_err(|_| tos_compiler::Error::Budget("standalone entity registry bytes"))?;
            let relation = capture
                .read_retained_input(relation_path, row_bytes)
                .map_err(|_| tos_compiler::Error::Budget("standalone relation registry bytes"))?;
            let registry_bytes = entity
                .len()
                .checked_add(relation.len())
                .ok_or(tos_compiler::Error::Budget("standalone registry byte sum"))?;
            schemas.validate_registries(
                [&entity, &relation],
                read.json,
                usize::try_from(decoded_bytes).unwrap_or(usize::MAX),
            )?;
            // Source digests are supplied by the authentic capture's retained
            // source receipt; the wrapper never manufactures expected identity
            // from the bytes it is about to compare.
            let expected_entity = capture
                .source_digest(entity_path)
                .map_err(|_| tos_compiler::Error::Invalid("standalone entity registry receipt"))?;
            let expected_relation = capture
                .source_digest(relation_path)
                .map_err(|_| tos_compiler::Error::Invalid("standalone relation registry receipt"))?;
            let bundle = tos_query::knowledge_contracts::build_knowledge_contract_bundle(
                [&entity, &relation],
                [expected_entity, expected_relation],
                tos_query::knowledge_contracts::KnowledgeContractBudget {
                    max_input_bytes: usize::try_from(decoded_bytes).unwrap_or(usize::MAX),
                    max_registry_bytes: row_bytes,
                    max_response_bytes: response_bytes,
                    json: read.json,
                },
            )
            .map_err(|_| tos_compiler::Error::Invalid("standalone knowledge contract bundle"))?;
            let contracts = bundle
                .object_get("contracts")
                .and_then(JsonValue::as_object)
                .ok_or(tos_compiler::Error::Invalid("standalone contract bundle shape"))?;
            let contract_ids: BTreeSet<String> = contracts
                .iter()
                .map(|(key, _)| key.as_str().map(ToOwned::to_owned)
                    .ok_or(tos_compiler::Error::Invalid("standalone bundle key")))
                .collect::<tos_compiler::Result<_>>()?;
            let expected_native = expected_set(&[
                "api", "knowledge_graph", "knowledge_search_indexed", "readable_context",
                "lens_spec", "lens_result", "temporal_comparison_request",
                "temporal_comparison_result", "source_read", "entity_type_registry_schema",
                "relation_type_registry_schema", "entity_type_registry", "relation_type_registry",
            ]);
            if bundle.object_get("schema").and_then(JsonValue::as_str)
                != Some("tos_knowledge_contract_bundle_v1")
                || contract_ids != expected_native
            {
                return Err(tos_compiler::Error::Invalid("standalone native contract set"));
            }
            let registry_rows = 2u64;
            let query_rows = max_rows
                .checked_sub(registry_rows)
                .ok_or(tos_compiler::Error::Budget("standalone registry row envelope"))?;
            let registry_bytes_u64 = u64::try_from(registry_bytes)
                .map_err(|_| tos_compiler::Error::Budget("standalone registry byte envelope"))?;
            let query_decoded_bytes = decoded_bytes
                .checked_sub(registry_bytes_u64)
                .ok_or(tos_compiler::Error::Budget("standalone registry byte envelope"))?;
            let query_budget = tos_query::source_diagnostic_validation::SourceDiagnosticQueryLimits::from_envelope(
                query_rows,
                row_bytes,
                query_decoded_bytes,
                response_bytes,
                limits.capture.max_sql_vm_steps,
                Arc::new(DeadlineAbort(deadline)),
            )
            .map_err(|_| tos_compiler::Error::Budget("standalone query envelope"))?;
            let query = tos_query::source_diagnostic_validation::validate_stage(
                stage,
                full,
                header,
                source_revision,
                descriptor_raw,
                schemas,
                query_budget,
            )?;
            check_query_report(&query, schemas, read.json, deadline)
                .map_err(|_| tos_compiler::Error::Invalid("standalone native query assertions"))?;
            let packet = query.packet();
            let packet_count = |field: &str| -> tos_compiler::Result<u64> {
                packet
                    .object_get(field)
                    .and_then(JsonValue::as_u64)
                    .ok_or(tos_compiler::Error::Invalid("standalone native report count"))
            };
            if packet_count("schema_nodes_checked")? != full.seal.node_count
                || packet_count("schema_relations_checked")? != full.seal.relation_count
            {
                return Err(tos_compiler::Error::Invalid(
                    "standalone all-row graph schema count",
                ));
            }
            let query_facts = QueryFacts {
                source_revision: source_revision.to_owned(),
                node_count: full.seal.node_count,
                relation_count: full.seal.relation_count,
                schema_nodes_checked: packet_count("schema_nodes_checked")?,
                schema_relations_checked: packet_count("schema_relations_checked")?,
                query_rows: query
                    .rows()
                    .checked_add(registry_rows)
                    .ok_or(tos_compiler::Error::Budget("standalone total query rows"))?,
                query_decoded_bytes: query
                    .decoded_bytes()
                    .checked_add(registry_bytes_u64)
                    .ok_or(tos_compiler::Error::Budget("standalone total query bytes"))?,
                query_response_bytes: query.response_bytes(),
                native_contract_count: contract_ids.len(),
                registry_bytes,
            };
            facts = Some(query_facts);
            Ok(())
        },
    )
    .map_err(|_| "native source5 snapshot validation refused")?;
    facts.ok_or_else(|| "native source5 validation returned no facts".into())
}
fn verify_captured_input_roots(
    capture: &tos_compiler::PublicCapture,
    allowlist: &Value,
) -> Result<usize, String> {
    let subjects = allowlist
        .get("subjects")
        .and_then(Value::as_array)
        .ok_or("runtime allowlist subjects must be a list")?;
    let paths: BTreeMap<&str, &str> = subjects
        .iter()
        .map(|item| {
            let id = item
                .get("subject_id")
                .and_then(Value::as_str)
                .ok_or("runtime subject ID missing")?;
            let path = item
                .get("source_path")
                .and_then(Value::as_str)
                .ok_or("runtime subject path missing")?;
            Ok((id, path))
        })
        .collect::<Result<_, &str>>()?;
    let members = capture
        .retained_input_members()
        .map_err(|_| "native capture retained-member receipt unavailable")?;
    let member_paths: BTreeSet<&str> = members.iter().map(|(path, _, _)| path.as_str()).collect();
    for id in CAPTURED_INPUT_SUBJECTS {
        let path = paths
            .get(id)
            .copied()
            .ok_or("native captured input is absent from the selected runtime allowlist")?;
        if !member_paths.contains(path) {
            return Err("native capture does not retain an exact declared input root".into());
        }
        capture
            .source_digest(path)
            .map_err(|_| "native capture input digest receipt unavailable")?;
    }
    Ok(members.len())
}
fn validate_semantic_header(
    counts: &serde_json::Map<String, Value>,
    nodes: u64,
    relations: u64,
) -> Result<(), String> {
    let semantic = counts
        .get("semantic_mapping")
        .ok_or("semantic mapping counts absent")?;
    if semantic.get("unmapped_nodes").and_then(Value::as_u64) != Some(0)
        || semantic.get("unmapped_relations").and_then(Value::as_u64) != Some(0)
    {
        return Err("unmapped production vocabulary".into());
    }
    let validation = counts
        .get("semantic_validation")
        .ok_or("semantic validation counts absent")?;
    if validation.get("valid") != Some(&Value::Bool(true))
        || !validation
            .get("violations")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty)
    {
        return Err("semantic registry invariants failed".into());
    }
    let coverage = counts
        .get("display_coverage")
        .ok_or("display coverage absent")?;
    for (field, expected) in [
        ("node_titles", nodes),
        ("node_summaries", nodes),
        ("relation_labels", relations),
        ("relation_statements", relations),
        ("relation_explanations", relations),
    ] {
        if coverage.get(field).and_then(Value::as_u64) != Some(expected) {
            return Err("normalized knowledge graph display coverage incomplete".into());
        }
    }
    Ok(())
}
fn value_bytes(value: &JsonValue, limits: JsonLimits) -> Result<Vec<u8>, String> {
    emit_value_preserved_json(value, limits)
        .map_err(|_| "diagnostic packet encoding exceeded cap".into())
}
fn validate_value(
    schemas: &tos_query::source_diagnostic_validation::SourceDiagnosticSchemas,
    schema: tos_query::source_diagnostic_validation::SourceDiagnosticSchema,
    value: &JsonValue,
    limits: JsonLimits,
    deadline: Instant,
) -> Result<usize, String> {
    if Instant::now() >= deadline {
        return Err("standalone query validation deadline exceeded".into());
    }
    let raw = value_bytes(value, limits)?;
    schemas
        .validate_raw(schema, &raw, limits)
        .map_err(|_| "native diagnostic packet schema invalid")?;
    if Instant::now() >= deadline {
        return Err("standalone query validation deadline exceeded".into());
    }
    Ok(raw.len())
}
fn check_query_report(
    report: &tos_query::source_diagnostic_validation::SourceDiagnosticQueryReport,
    schemas: &tos_query::source_diagnostic_validation::SourceDiagnosticSchemas,
    json: JsonLimits,
    deadline: Instant,
) -> Result<(), String> {
    let packet = report.packet();
    if packet.object_get("schema").and_then(JsonValue::as_str)
        != Some("tos_source_diagnostic_query_report_v1")
    {
        return Err("native source query report schema differs".into());
    }
    if packet.object_get("nodes").and_then(JsonValue::as_u64) == Some(0) {
        return Err("normalized knowledge graph has no node for exploration validation".into());
    }
    let catalog = packet
        .object_get("catalog")
        .ok_or("native source catalog absent")?;
    let lenses = catalog
        .object_get("lenses")
        .and_then(JsonValue::as_array)
        .ok_or("native source catalog lenses absent")?;
    let mut validated_bytes = 0usize;
    for lens in lenses {
        validated_bytes = validated_bytes
            .checked_add(validate_value(
                schemas,
                tos_query::source_diagnostic_validation::SourceDiagnosticSchema::LensSpec,
                lens,
                json,
                deadline,
            )?)
            .ok_or("native lens schema byte count overflow")?;
    }
    for (field, schema) in [
        (
            "lens_request",
            tos_query::source_diagnostic_validation::SourceDiagnosticSchema::LensSpec,
        ),
        (
            "lens",
            tos_query::source_diagnostic_validation::SourceDiagnosticSchema::LensResult,
        ),
    ] {
        let value = packet
            .object_get(field)
            .ok_or("native lens probe packet absent")?;
        validated_bytes = validated_bytes
            .checked_add(validate_value(schemas, schema, value, json, deadline)?)
            .ok_or("native lens probe byte count overflow")?;
    }
    if let Some(request) = packet.object_get("exploration_request") {
        validated_bytes += validate_value(
            schemas,
            tos_query::source_diagnostic_validation::SourceDiagnosticSchema::ExplorationRequestV1,
            request,
            json,
            deadline,
        )?;
        let result = packet
            .object_get("exploration")
            .ok_or("native first exploration packet absent")?;
        validated_bytes += validate_value(
            schemas,
            tos_query::source_diagnostic_validation::SourceDiagnosticSchema::ExplorationResultV1,
            result,
            json,
            deadline,
        )?;
        if let Some(resumed) = packet.object_get("exploration_resumed") {
            validated_bytes += validate_value(
                schemas,
                tos_query::source_diagnostic_validation::SourceDiagnosticSchema::ExplorationResultV1,
                resumed,
                json,
                deadline,
            )?;
        }
        let cursor = packet
            .object_get("exploration")
            .and_then(|result| result.object_get("page"))
            .and_then(|page| page.object_get("next_cursor"))
            .ok_or("native first exploration cursor field absent")?;
        if matches!(cursor, JsonValue::Null) != packet.object_get("exploration_resumed").is_none() {
            return Err("native v1 exploration resume did not match its cursor".into());
        }
    } else {
        return Err("native first exploration packet absent".into());
    }
    if packet.object_get("node_origin").is_none()
        || (packet.object_get("relations").and_then(JsonValue::as_u64) != Some(0)
            && packet.object_get("relation_origin").is_none())
    {
        return Err("native typed node/relation exploration probes are incomplete".into());
    }
    for field in ["node_origin", "relation_origin"] {
        let Some(origin) = packet.object_get(field) else {
            continue;
        };
        let request = origin
            .object_get("request")
            .ok_or("typed exploration request absent")?;
        let result = origin
            .object_get("result")
            .ok_or("typed exploration result absent")?;
        validated_bytes += validate_value(
            schemas,
            tos_query::source_diagnostic_validation::SourceDiagnosticSchema::ExplorationRequestV2,
            request,
            json,
            deadline,
        )?;
        validated_bytes += validate_value(
            schemas,
            tos_query::source_diagnostic_validation::SourceDiagnosticSchema::ExplorationResultV2,
            result,
            json,
            deadline,
        )?;
        if result.object_get("status").and_then(JsonValue::as_str) != Some("complete")
            || !result
                .object_get("page")
                .and_then(|page| page.object_get("primary_node_ids"))
                .and_then(JsonValue::as_array)
                .is_some_and(Vec::is_empty)
            || !result
                .object_get("page")
                .and_then(|page| page.object_get("primary_relation_ids"))
                .and_then(JsonValue::as_array)
                .is_some_and(Vec::is_empty)
        {
            return Err("zero-depth typed exploration must return origin context only".into());
        }
    }
    if validated_bytes > json.max_bytes {
        return Err("native diagnostic schema validation bytes exceeded".into());
    }
    Ok(())
}

fn fresh_stage_path(root: &Path, stem: &str) -> Result<PathBuf, String> {
    let path = root.join(format!("{stem}-{}.sqlite3", std::process::id()));
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(path),
        _ => Err("private source validation path is not fresh".into()),
    }
}
fn validate_source(
    root: &Path,
    quota: u64,
    inodes: u64,
    ram: u64,
    max_build_seconds: u64,
    query_rows: u64,
    row_bytes: usize,
    decoded_bytes: u64,
    response_bytes: usize,
) -> Result<Value, String> {
    if !(600..=MAX_BUILD_SECONDS).contains(&max_build_seconds)
        || query_rows == 0
        || row_bytes == 0
        || decoded_bytes == 0
        || response_bytes == 0
    {
        return Err("source validation requires explicit positive finite limits".into());
    }
    let limits =
        tos_compiler::native_snapshot_manifest::portable_native_snapshot_limits(max_build_seconds)
            .map_err(|_| "native source validation profile is unavailable")?;
    let started = Instant::now();
    let deadline = started
        .checked_add(Duration::from_secs(max_build_seconds))
        .ok_or("source validation deadline arithmetic overflow")?;
    // The sealed host ticket must be selected before source capture, SQLite,
    // or any writer. Its values come from the admitted whole-run envelope.
    let isolation =
        tos_compiler::private_tmpfs_stage::PrivateTmpfsStageIsolation::select_from_environment(
            quota, inodes, ram,
        )
        .map_err(|_| "matching private stage ticket is required")?;
    let root = fs::canonicalize(root).map_err(|_| "selected source root unavailable")?;
    let mut meter = Meter::new(&root, decoded_bytes, query_rows, deadline)?;
    let software = validate_software_contracts(&mut meter)?;
    scan_source(&mut meter)?;
    let (partitioned, partitioned_closure_members) =
        validate_runtime_allowlist(&mut meter, &software.allowlist)?;
    if Instant::now() >= deadline {
        return Err("source validation deadline exceeded before capture".into());
    }
    let capture_path = fresh_stage_path(isolation.root(), "tos-standalone-capture")?;
    let capture =
        tos_compiler::PublicCapture::create_runtime(&root, &capture_path, limits.capture, deadline)
            .map_err(|_| "native source capture refused".to_owned())?;
    let retained_member_count = verify_captured_input_roots(&capture, &software.allowlist)?;
    let facts = stage_predicates(
        &capture,
        &isolation,
        limits,
        &software.allowlist_raw,
        &software.schemas,
        meter.remaining_rows(),
        row_bytes,
        meter.remaining_bytes(),
        response_bytes,
        deadline,
    )?;
    meter.account_external(facts.query_rows, facts.query_decoded_bytes)?;
    let executable = std::env::current_exe().map_err(|_| "native program path unavailable")?;
    let program_directory = executable
        .parent()
        .ok_or("native program directory unavailable")?;
    let doctor = crate::doctor::doctor_report(&root, "standalone", true, program_directory)
        .map_err(|_| "standalone verify diagnostic failed")?;
    if doctor.object_get("ok") != Some(&JsonValue::Bool(true)) {
        return Err("standalone verify profile failed".into());
    }
    let doctor_raw = emit_value_preserved_json(
        &doctor,
        JsonLimits {
            max_bytes: 65_536,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| "standalone verify report exceeds cap")?;
    let doctor_value: Value = serde_json::from_slice(&doctor_raw)
        .map_err(|_| "standalone verify report encoding failed")?;
    capture
        .verify_inputs(limits.capture)
        .map_err(|_| "native source inputs changed during validation")?;
    meter.verify()?;
    Ok(json!({
        "schema_version": REPORT_SCHEMA,
        "ok": true,
        "mode": "source",
        "data_validated": true,
        "partitioned_subjects": partitioned,
        "checks": {
            "validation_meter_rows": meter.rows,
            "validation_meter_bytes": meter.bytes,
            "software_contracts": true,
            "source_marker_scan": true,
            "source5_full_validation": true,
            "all_graph_schema_rows": facts.schema_nodes_checked + facts.schema_relations_checked,
            "native_contract_count": facts.native_contract_count,
            "query_rows": facts.query_rows,
            "query_decoded_bytes": facts.query_decoded_bytes,
            "query_response_bytes": facts.query_response_bytes,
            "registry_bytes": facts.registry_bytes,
            "native_capture_input_roots": CAPTURED_INPUT_SUBJECTS.len(),
            "native_capture_retained_members": retained_member_count,
            "partitioned_closure_members": partitioned_closure_members,
            "schema_resources": software.schema_bytes,
            "source_revision": facts.source_revision,
            "nodes": facts.node_count,
            "relations": facts.relation_count,
        },
        "doctor": doctor_value,
    }))
}

fn validate_software(root: &Path) -> Result<Value, String> {
    let deadline = Instant::now()
        .checked_add(Duration::from_secs(SOFTWARE_SECONDS))
        .ok_or("software validation deadline arithmetic overflow")?;
    let mut meter = Meter::new(root, SOFTWARE_BYTES, SOFTWARE_ROWS, deadline)?;
    let facts = validate_software_contracts(&mut meter)?;
    scan_source(&mut meter)?;
    meter.verify()?;
    Ok(json!({
        "schema_version": REPORT_SCHEMA,
        "ok": true,
        "mode": "software",
        "data_validated": false,
        "checks": {
            "validation_meter_rows": meter.rows,
            "validation_meter_bytes": meter.bytes,
            "software_contracts": true,
            "source_marker_scan": true,
            "schema_resources": facts.schema_bytes,
        }
    }))
}

#[derive(Default)]
struct Args {
    root: Option<PathBuf>,
    software: bool,
    source: bool,
    json: bool,
    quota: Option<u64>,
    inodes: Option<u64>,
    ram: Option<u64>,
    max_build_seconds: Option<u64>,
    query_rows: Option<u64>,
    query_row_bytes: Option<usize>,
    query_decoded_bytes: Option<u64>,
    query_response_bytes: Option<usize>,
}
fn parse_u64(value: &str, name: &str) -> Result<u64, String> {
    value
        .parse()
        .map_err(|_| format!("invalid integer for {name}"))
}
fn parse_usize(value: &str, name: &str) -> Result<usize, String> {
    value
        .parse()
        .map_err(|_| format!("invalid integer for {name}"))
}
fn matching_args(args: &[String]) -> Option<usize> {
    let mut at = 0;
    while let Some(option) = args.get(at) {
        let (key, inline) = option
            .split_once('=')
            .map_or((option.as_str(), None), |(key, value)| (key, Some(value)));
        if key != "--root" {
            break;
        }
        if inline.is_some() {
            at += 1;
            continue;
        }
        if args.get(at + 1).map(String::as_str) == Some("validate-standalone") {
            // The route token in the value slot is a missing --root operand;
            // claim it so this dispatcher reports the malformed invocation.
            return Some(at + 1);
        }
        at = at.saturating_add(2);
    }
    (args.get(at).map(String::as_str) == Some("validate-standalone")).then_some(at)
}
fn parse_args(args: &[String], at: usize) -> Result<Args, String> {
    let mut parsed = Args::default();
    let mut seen = BTreeSet::new();
    let mut index = 0;
    while index < at {
        let arg = &args[index];
        let (key, inline) = arg
            .split_once('=')
            .map_or((arg.as_str(), None), |(key, value)| (key, Some(value)));
        if key != "--root" {
            return Err("validate-standalone only accepts an explicit --root selector".into());
        }
        let value = if let Some(value) = inline {
            value
        } else {
            index += 1;
            args.get(index).map(String::as_str).unwrap_or("")
        };
        if value.is_empty() || !seen.insert(key) {
            return Err("--root requires one explicit absolute path".into());
        }
        parsed.root = Some(PathBuf::from(value));
        index += 1;
    }
    index = at + 1;
    while index < args.len() {
        let arg = &args[index];
        let (key, inline) = arg
            .split_once('=')
            .map_or((arg.as_str(), None), |(key, value)| (key, Some(value)));
        if key == "--json" {
            if inline.is_some() || !seen.insert(key) {
                return Err("--json may appear once without a value".into());
            }
            parsed.json = true;
            index += 1;
            continue;
        }
        if matches!(key, "--software" | "--source") {
            if inline.is_some() || !seen.insert(key) {
                return Err("validation mode flag may appear once without a value".into());
            }
            if key == "--software" {
                parsed.software = true;
            } else {
                parsed.source = true;
            }
            index += 1;
            continue;
        }
        let value = if let Some(value) = inline {
            value
        } else {
            index += 1;
            args.get(index).map(String::as_str).unwrap_or("")
        };
        if value.is_empty() || !seen.insert(key) {
            return Err(format!("{key} requires one value"));
        }
        match key {
            "--tmpfs-quota-bytes" => parsed.quota = Some(parse_u64(value, key)?),
            "--tmpfs-inode-limit" => parsed.inodes = Some(parse_u64(value, key)?),
            "--working-ram-bytes" => parsed.ram = Some(parse_u64(value, key)?),
            "--max-build-seconds" => parsed.max_build_seconds = Some(parse_u64(value, key)?),
            "--max-query-rows" => parsed.query_rows = Some(parse_u64(value, key)?),
            "--max-query-row-bytes" => parsed.query_row_bytes = Some(parse_usize(value, key)?),
            "--max-query-decoded-bytes" => {
                parsed.query_decoded_bytes = Some(parse_u64(value, key)?)
            }
            "--max-query-response-bytes" => {
                parsed.query_response_bytes = Some(parse_usize(value, key)?)
            }
            _ => return Err(format!("unsupported validate-standalone option: {key}")),
        }
        index += 1;
    }
    if !parsed.json || parsed.software == parsed.source {
        return Err(
            "validate-standalone requires --json and exactly one of --software or --source".into(),
        );
    }
    let root = parsed
        .root
        .as_ref()
        .ok_or("validate-standalone requires --root ABS")?;
    if !root.is_absolute()
        || root
            .components()
            .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
    {
        return Err("--root must be an absolute normalized path".into());
    }
    if parsed.software && seen.len() != 3 {
        return Err("software mode accepts only --root and --json --software".into());
    }
    if parsed.source {
        if [
            parsed.quota,
            parsed.inodes,
            parsed.ram,
            parsed.max_build_seconds,
            parsed.query_rows,
            parsed.query_row_bytes,
            parsed.query_decoded_bytes,
            parsed.query_response_bytes,
        ]
        .iter()
        .any(Option::is_none)
        {
            return Err(
                "source mode requires the complete native resource and query envelope".into(),
            );
        }
    }
    Ok(parsed)
}

/// Dispatch only this exact maintained route; unrelated commands are declined.
pub fn run_if_requested(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Option<i32> {
    let at = matching_args(args)?;
    let result = (|| {
        let parsed = parse_args(args, at)?;
        let root = parsed.root.as_deref().ok_or("source root unavailable")?;
        let report = if parsed.software {
            validate_software(root)?
        } else {
            validate_source(
                root,
                parsed.quota.ok_or("tmpfs quota missing")?,
                parsed.inodes.ok_or("tmpfs inode limit missing")?,
                parsed.ram.ok_or("working RAM limit missing")?,
                parsed.max_build_seconds.ok_or("build seconds missing")?,
                parsed.query_rows.ok_or("query row limit missing")?,
                parsed
                    .query_row_bytes
                    .ok_or("query row byte limit missing")?,
                parsed
                    .query_decoded_bytes
                    .ok_or("query decoded byte limit missing")?,
                parsed
                    .query_response_bytes
                    .ok_or("query response byte limit missing")?,
            )?
        };
        let mut bytes =
            serde_json::to_vec(&report).map_err(|_| "standalone report encoding failed")?;
        if bytes.len() > 65_536 {
            return Err("standalone report exceeds output envelope".into());
        }
        bytes.push(b'\n');
        stdout
            .write_all(&bytes)
            .and_then(|_| stdout.flush())
            .map_err(|_| "standalone report write failed")?;
        Ok::<i32, String>(0)
    })();
    Some(match result {
        Ok(code) => code,
        Err(error) => {
            let _ = writeln!(stderr, "standalone_validation_failed: {error}");
            1
        }
    })
}
