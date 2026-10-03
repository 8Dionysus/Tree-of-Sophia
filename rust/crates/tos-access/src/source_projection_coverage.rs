//! Native source-to-projection coverage observation.
//!
//! This tool reports only the exact public source-catalog identities and the
//! fields present on normalized graph carriers. It never assesses meaning,
//! admits source, or mutates an owner surface.

use crate::source_projection_catalog_capture::{CaptureObservationLimits, observe_owned_catalogue};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{self, Read, Write},
    os::fd::AsRawFd,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, AtomicI32},
    time::{Duration, Instant},
};
use tos_foundation::{
    CanonicalProfile, Digest256, Digest256Hasher, JsonLimits, JsonMode, JsonString, JsonValue,
    canonical_feed_digest_v1, canonical_raw_bytes_v1, parse_json,
};
use tos_query::source_diagnostic::Limits;
use tos_source_store::MetadataPublicationEpoch;

const SOURCE_HOME: &str = "ToS/source-witnesses";
const CATALOG_HOME: &str = "ToS/source-witnesses/catalog";
const PUBLICATION_REF: &str = "ToS/source-witnesses/.metadata-publication.json";
const INPUT_CAP: u64 = 256 * 1024 * 1024;
const REQUEST_CAP: usize = 16 * 1024 * 1024;
const MAX_ROWS: u64 = 1_000_000;
const MAX_SELECTED_FILES: usize = 16_384;
const MAX_CLAIM_FILE_BYTES: usize = 16 * 1024 * 1024;
const MAX_SOURCE_FILE_BYTES: usize = 16 * 1024 * 1024;
const MAX_SOURCE_ROW_BYTES: usize = 1024 * 1024;
const MAX_SOURCE_HASH_MAP_BYTES: usize = 256 * 1024 * 1024;
const MAX_OUTPUT_ROW_BYTES: usize = 4 * 1024 * 1024;
const MAX_OUTPUT_FILE_BYTES: usize = 64 * 1024 * 1024;
const MAX_PACKET_BYTES: usize = 64 * 1024 * 1024;
const MAX_STATE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_WALK_ENTRIES: u64 = 1_000_000;
const MAX_WALK_DEPTH: usize = 128;

const LIMITATIONS: [&str; 8] = [
    "Only catalog-owned public metadata identities are enumerated, not all ToS corpus resources.",
    "Private native semantic packets, payloads and owner-local material are outside this report.",
    "Unknown uncatalogued families require their source owner; absence does not imply restriction or falsity.",
    "Direct/adapted mapping means exact retained JSON fields and source return, not semantic understanding.",
    "Source-file hashes bind observed bytes; a JSON carrier does not preserve file formatting.",
    "Historical versions, form quality, assessment, rights and canon require their separate owner routes.",
    "The report compares the selected snapshot. Generated artifacts, runtime and deployment have their own revision and currentness checks.",
    "Boundary rechecks do not provide an atomic snapshot against arbitrary non-cooperating editors.",
];

#[derive(Clone, Debug)]
pub struct SourceEntry {
    pub identity: String,
    pub record: Value,
    pub catalog_entry: Value,
    pub source_ref: String,
    pub source_file_sha256: String,
    pub kind: String,
    pub adapter: String,
    pub source_line: Option<u64>,
}

#[derive(Default)]
struct IdentityObservation {
    carriers: Vec<Value>,
}

#[derive(Default)]
struct Totals {
    kinds: BTreeMap<String, BTreeMap<String, u64>>,
    identities: u64,
}

fn canonical_digest(value: &Value) -> Result<String, String> {
    let raw = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    canonical_raw_bytes_v1(
        &raw,
        CanonicalProfile::SourceRecordDigestV1,
        JsonLimits {
            max_bytes: MAX_SOURCE_ROW_BYTES,
            max_depth: 96,
            max_visits: 1_000_000,
            max_integer_digits: 4_300,
        },
    )
    .map(|bytes| Digest256::of_bytes(&bytes).to_hex())
    .map_err(|error| error.to_string())
}

fn json_string_encoded_len(value: &str) -> Result<usize, String> {
    value.chars().try_fold(2usize, |length, ch| {
        let encoded = match ch {
            '"' | '\\' | '\u{0008}' | '\u{0009}' | '\u{000a}' | '\u{000c}' | '\u{000d}' => 2,
            ch if ch <= '\u{001f}' => 6,
            ch => ch.len_utf8(),
        };
        length
            .checked_add(encoded)
            .ok_or_else(|| "source projection source-hash map size overflow".to_owned())
    })
}

fn source_hash_map_json_len(
    hashes: &BTreeMap<String, String>,
    meter: &mut Meter,
) -> Result<(usize, u64), String> {
    let mut length = 2usize;
    let mut max_fragment = 0usize;
    for (index, (path, digest)) in hashes.iter().enumerate() {
        meter.tick((path.len() as u64).saturating_add(digest.len() as u64))?;
        max_fragment = max_fragment.max(path.len()).max(digest.len());
        if index > 0 {
            length = length
                .checked_add(1)
                .ok_or("source projection source-hash map size overflow")?;
        }
        let path_len = json_string_encoded_len(path)?;
        let digest_len = json_string_encoded_len(digest)?;
        length = length
            .checked_add(path_len)
            .and_then(|value| value.checked_add(1))
            .and_then(|value| value.checked_add(digest_len))
            .ok_or("source projection source-hash map size overflow")?;
    }
    if length > MAX_SOURCE_HASH_MAP_BYTES {
        return Err("source projection source-hash map byte budget exceeded".into());
    }
    let fragment_state = (max_fragment as u64)
        .checked_mul(3)
        .and_then(|bytes| bytes.checked_add(256))
        .ok_or("source projection retained-state estimate overflow")?;
    Ok((length, fragment_state))
}

fn reserve_source_hash_map_digest(
    meter: &mut Meter,
    hashes: &BTreeMap<String, String>,
) -> Result<usize, String> {
    let (encoded_len, fragment_state) = source_hash_map_json_len(hashes, meter)?;
    meter.charge_state(fragment_state)?;
    Ok(encoded_len)
}

fn feed_source_hash_fragment(
    value: &str,
    hasher: &mut Digest256Hasher,
    written: &mut usize,
    visits: &mut usize,
) -> Result<(), String> {
    let fragment = JsonValue::String(JsonString::from_utf8(value));
    canonical_feed_digest_v1(
        &fragment,
        CanonicalProfile::SourceRecordDigestV1,
        JsonLimits {
            max_bytes: MAX_SOURCE_HASH_MAP_BYTES,
            max_depth: 96,
            max_visits: 1_000_000,
            max_integer_digits: 4_300,
        },
        hasher,
        written,
        visits,
        0,
    )
    .map_err(|error| error.to_string())
}

fn feed_source_hash_punctuation(
    bytes: &[u8],
    hasher: &mut Digest256Hasher,
    written: &mut usize,
) -> Result<(), String> {
    *written = written
        .checked_add(bytes.len())
        .filter(|length| *length <= MAX_SOURCE_HASH_MAP_BYTES)
        .ok_or("source projection source-hash map byte budget exceeded")?;
    hasher.update(bytes);
    Ok(())
}

fn source_hash_map_digest(
    hashes: &BTreeMap<String, String>,
    expected_encoded_len: usize,
    meter: &mut Meter,
    deadline: Instant,
) -> Result<String, String> {
    if Instant::now() >= deadline {
        return Err("source projection original deadline exceeded".into());
    }
    let mut hasher = Digest256Hasher::new();
    let (mut written, mut visits) = (0usize, 0usize);
    feed_source_hash_punctuation(b"{", &mut hasher, &mut written)?;
    for (index, (path, digest)) in hashes.iter().enumerate() {
        meter.tick(1)?;
        if index > 0 {
            feed_source_hash_punctuation(b",", &mut hasher, &mut written)?;
        }
        feed_source_hash_fragment(path, &mut hasher, &mut written, &mut visits)?;
        feed_source_hash_punctuation(b":", &mut hasher, &mut written)?;
        feed_source_hash_fragment(digest, &mut hasher, &mut written, &mut visits)?;
    }
    feed_source_hash_punctuation(b"}", &mut hasher, &mut written)?;
    if Instant::now() >= deadline {
        return Err("source projection original deadline exceeded".into());
    }
    if written != expected_encoded_len {
        return Err(
            "source projection source-hash map size changed during canonicalization".into(),
        );
    }
    Ok(hasher.finalize().to_hex())
}

fn remember_source_hash(
    meter: &mut Meter,
    hashes: &mut BTreeMap<String, String>,
    path: &str,
    digest: &str,
) -> Result<(), String> {
    if let Some(previous) = hashes.get(path) {
        if previous != digest {
            return Err("source projection source path changed digest during enumeration".into());
        }
        return Ok(());
    }
    let retained = (path.len() as u64)
        .checked_add(digest.len() as u64)
        .and_then(|bytes| bytes.checked_add(128))
        .ok_or("source projection retained-state estimate overflow")?;
    meter.tick((path.len() as u64).saturating_add(digest.len() as u64))?;
    meter.charge_state(retained)?;
    hashes.insert(path.to_owned(), digest.to_owned());
    Ok(())
}

fn canonical_bytes(value: &Value, cap: usize) -> Result<Vec<u8>, String> {
    let raw = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    canonical_raw_bytes_v1(
        &raw,
        CanonicalProfile::SourceRecordDigestV1,
        JsonLimits {
            max_bytes: cap,
            max_depth: 96,
            max_visits: 1_000_000,
            max_integer_digits: 4_300,
        },
    )
    .map_err(|error| error.to_string())
}

fn string<'a>(value: &'a Value, name: &str) -> Result<&'a str, String> {
    value
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("source projection carrier needs string {name}"))
}

fn compare_carrier(entry: &SourceEntry, item: &Value) -> Result<Value, String> {
    if !item.is_object() {
        return Err("source projection carrier must be a JSON object".into());
    }
    let carrier_id = string(item, "id")?;
    let field = if entry.kind == "claim" {
        "source_claim"
    } else {
        "source_record"
    };
    let attributes = item.get("attributes").and_then(Value::as_object);
    let raw = attributes.and_then(|attributes| attributes.get(field));
    let supplied = raw.and_then(Value::as_object);
    let source = entry
        .record
        .as_object()
        .ok_or("source projection record must be a JSON object")?;
    let mut missing = Vec::new();
    let mut added = Vec::new();
    let mut changed = Vec::new();
    if let Some(raw) = supplied {
        missing.extend(source.keys().filter(|key| !raw.contains_key(*key)).cloned());
        added.extend(raw.keys().filter(|key| !source.contains_key(*key)).cloned());
        for key in source.keys().filter(|key| raw.contains_key(*key)) {
            if canonical_digest(&source[key])? != canonical_digest(&raw[key])? {
                changed.push(key.clone());
            }
        }
    }
    missing.sort();
    added.sort();
    changed.sort();
    let exact = supplied.is_some() && missing.is_empty() && added.is_empty() && changed.is_empty();
    let relation = item.get("from_id").is_some();
    let mapping_name = if relation {
        "predicate_mapping"
    } else {
        "type_mapping"
    };
    let mapped = item
        .get(mapping_name)
        .and_then(Value::as_object)
        .and_then(|mapping| mapping.get("status"))
        == Some(&json!("mapped"));
    let source_return_present = item
        .get("source_refs")
        .and_then(Value::as_array)
        .is_some_and(|refs| {
            refs.iter()
                .any(|value| value.as_str() == Some(&entry.source_ref))
        });
    Ok(json!({
        "id": carrier_id,
        "source_graph": item.get("source_graph").cloned().unwrap_or(Value::Null),
        "kind": if relation { "relation" } else { "node" },
        "raw_record_state": if exact { "exact" } else if supplied.is_some() { "different" } else { "not-provided" },
        "mapping_state": if mapped { "mapped" } else { "unmapped" },
        "source_return_present": source_return_present,
        "missing_fields": missing,
        "added_fields": added,
        "changed_fields": changed,
        "record_pointer": if supplied.is_some() { json!(format!("/attributes/{field}")) } else { Value::Null },
    }))
}

fn finish_observation(entry: &SourceEntry, observed: Vec<Value>) -> Result<Value, String> {
    let source = entry
        .record
        .as_object()
        .ok_or("source projection record must be a JSON object")?;
    let state = if observed.is_empty() {
        "missing-from-projection"
    } else if observed
        .iter()
        .any(|item| item["raw_record_state"] == "different")
    {
        "conflicting-record-carriers"
    } else if observed.iter().any(|item| {
        item["raw_record_state"] == "exact"
            && item["mapping_state"] == "mapped"
            && item["source_return_present"] == true
    }) {
        if entry.adapter == "native-witness" {
            "mapped-through-adapter"
        } else {
            "mapped-directly"
        }
    } else {
        "requires-clarification"
    };
    Ok(json!({
        "identity": entry.identity,
        "kind": entry.kind,
        "state": state,
        "adapter": entry.adapter,
        "source_ref": entry.source_ref,
        "source_line": entry.source_line,
        "record_digest": format!("sha256:{}", canonical_digest(&entry.record)?),
        "field_count": source.len(),
        "carriers": observed,
        "next_owner": "ToS/source-witnesses",
        "next_action": if state.starts_with("mapped-") {
            "retain-source-and-review-separate-content-form-and-admission-gaps"
        } else {
            "inspect-exact-source-and-projection-mapping-with-the-source-owner"
        },
    }))
}

/// Preserve the legacy imported callback while all comparison rules execute in Rust.
pub fn observe_record(
    identity: &str,
    record: &Value,
    source_ref: &str,
    candidates: &[Value],
    kind: &str,
    adapter: &str,
    source_line: Option<u64>,
) -> Result<Value, String> {
    if !record.is_object() {
        return Err("source projection record must be a JSON object".into());
    }
    let entry = SourceEntry {
        identity: identity.to_owned(),
        record: record.clone(),
        catalog_entry: Value::Null,
        source_ref: source_ref.to_owned(),
        source_file_sha256: String::new(),
        kind: kind.to_owned(),
        adapter: adapter.to_owned(),
        source_line,
    };
    let mut sorted = candidates.iter().collect::<Vec<_>>();
    sorted.sort_by(|left, right| {
        left.get("id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .cmp(right.get("id").and_then(Value::as_str).unwrap_or(""))
    });
    let mut observed = Vec::with_capacity(sorted.len());
    for item in sorted {
        observed.push(compare_carrier(&entry, item)?);
    }
    finish_observation(&entry, observed)
}

fn carrier_identities(item: &Value, known: &BTreeSet<String>) -> BTreeSet<String> {
    let mut declared = BTreeSet::new();
    for field in ["entity_id", "native_id", "id"] {
        if let Some(identity) = item.get(field).and_then(Value::as_str) {
            if known.contains(identity) {
                declared.insert(identity.to_owned());
            }
        }
    }
    if let Some(attributes) = item.get("attributes").and_then(Value::as_object) {
        for field in ["record_id", "identity_ref", "claim_id", "claim_ref"] {
            if let Some(identity) = attributes.get(field).and_then(Value::as_str) {
                if known.contains(identity) {
                    declared.insert(identity.to_owned());
                }
            }
        }
    }
    declared
}

fn add_carrier(
    item: &Value,
    entries: &BTreeMap<String, SourceEntry>,
    known: &BTreeSet<String>,
    candidates: &mut BTreeMap<String, IdentityObservation>,
    meter: &mut Meter,
) -> Result<(), String> {
    if !item.is_object() {
        return Err("source projection carrier must be a JSON object".into());
    }
    let identities = carrier_identities(item, known);
    for identity in identities {
        let entry = entries
            .get(&identity)
            .ok_or("source projection identity disappeared")?;
        let observed = compare_carrier(entry, item)?;
        let estimate = serde_json::to_vec(&observed)
            .map_err(|error| error.to_string())?
            .len()
            .checked_mul(4)
            .and_then(|bytes| bytes.checked_add(256))
            .ok_or("source projection retained state overflow")?;
        meter.charge_state(estimate as u64)?;
        candidates
            .entry(identity)
            .or_default()
            .carriers
            .push(observed);
    }
    Ok(())
}

fn observe_entry(entry: &SourceEntry, candidates: &[Value]) -> Result<(Value, String), String> {
    let mut carriers = candidates.to_vec();
    carriers.sort_by(|left, right| {
        left.get("id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .cmp(right.get("id").and_then(Value::as_str).unwrap_or(""))
    });
    let row = finish_observation(entry, carriers)?;
    let state = row["state"]
        .as_str()
        .ok_or("source projection observation has no state")?
        .to_owned();
    Ok((
        json!({
            "schema_version": "tos_source_projection_coverage_row_v1",
            "source_revision": Value::Null,
            "observation": {
                "identity": row["identity"],
                "kind": row["kind"],
                "state": row["state"],
                "adapter": row["adapter"],
                "source_ref": row["source_ref"],
                "source_line": row["source_line"],
                "record_digest": row["record_digest"],
                "source_file_sha256": entry.source_file_sha256,
                "field_count": row["field_count"],
                "carriers": row["carriers"],
                "next_owner": row["next_owner"],
                "next_action": row["next_action"],
            }
        }),
        state,
    ))
}

fn report_summary<F>(
    entries: &[SourceEntry],
    source_files: usize,
    files_digest: &str,
    kinds: &[String],
    source_revision: &Value,
    catalog_sha256: &str,
    candidates: &BTreeMap<String, IdentityObservation>,
    rows: bool,
    deadline: Instant,
    before_terminal: F,
    output: &mut dyn Write,
) -> Result<(), String>
where
    F: FnOnce() -> Result<(), String>,
{
    let mut totals = Totals::default();
    for kind in kinds {
        totals.kinds.entry(kind.clone()).or_default();
    }
    totals.kinds.entry("claim".into()).or_default();
    let mut object_count = 0u64;
    let mut claim_count = 0u64;
    for entry in entries {
        if Instant::now() >= deadline {
            return Err("source projection original deadline exceeded".into());
        }
        if entry.kind == "claim" {
            claim_count = claim_count.checked_add(1).ok_or("claim count overflow")?;
        } else {
            object_count = object_count.checked_add(1).ok_or("object count overflow")?;
        }
        let carriers = candidates
            .get(&entry.identity)
            .map(|set| set.carriers.as_slice())
            .unwrap_or(&[]);
        let (mut row, state) = observe_entry(entry, carriers)?;
        row["source_revision"] = source_revision.clone();
        *totals
            .kinds
            .entry(entry.kind.clone())
            .or_default()
            .entry(state)
            .or_default() += 1;
        totals.identities = totals
            .identities
            .checked_add(1)
            .ok_or("source identity count overflow")?;
        if rows {
            emit(output, &row)?;
        }
    }
    let groups = totals
        .kinds
        .into_iter()
        .map(|(kind, states)| {
            let total = states.values().copied().sum::<u64>();
            json!({"kind":kind,"source_identities":total,"states":states})
        })
        .collect::<Vec<_>>();
    before_terminal()?;
    if Instant::now() >= deadline {
        return Err("source projection original deadline exceeded".into());
    }
    emit(
        output,
        &json!({
            "schema_version":"tos_source_projection_coverage_v1",
            "scope":"current-public-source-witness-catalog",
            "enumeration_complete":true,
            "source_revision":source_revision,
            "catalog_sha256":catalog_sha256,
            "source_files_digest":format!("sha256:{files_digest}"),
            "objects":object_count,
            "claims":claim_count,
            "source_identities":totals.identities,
            "source_files":source_files,
            "groups":groups,
            "limitations":LIMITATIONS,
            "performs_assessment":false,
            "grants_admission":false,
            "writes_to_source":false,
        }),
    )
}

fn emit(output: &mut dyn Write, value: &Value) -> Result<(), String> {
    let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    if bytes.len() > MAX_PACKET_BYTES {
        return Err("source projection output packet byte budget exceeded".into());
    }
    output
        .write_all(&bytes)
        .and_then(|_| output.write_all(b"\n"))
        .and_then(|_| output.flush())
        .map_err(|error| error.to_string())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Stamp {
    dev: u64,
    ino: u64,
    mode: u32,
    len: u64,
    mtime: i64,
    mtime_nsec: i64,
    ctime: i64,
    ctime_nsec: i64,
}
impl Stamp {
    fn read(metadata: &fs::Metadata) -> Self {
        Self {
            dev: metadata.dev(),
            ino: metadata.ino(),
            mode: metadata.mode(),
            len: metadata.len(),
            mtime: metadata.mtime(),
            mtime_nsec: metadata.mtime_nsec(),
            ctime: metadata.ctime(),
            ctime_nsec: metadata.ctime_nsec(),
        }
    }
}

struct FileFence {
    stamp: Stamp,
    sha256: String,
    cap: usize,
}

struct Meter {
    deadline: Instant,
    max_input: u64,
    input: u64,
    max_state: u64,
    state: u64,
    max_work: u64,
    work: u64,
    max_rows: u64,
    rows: u64,
}
impl Meter {
    fn checkpoint(&self) -> Result<(), String> {
        if Instant::now() >= self.deadline {
            return Err("source projection original deadline exceeded".into());
        }
        Ok(())
    }
    fn tick(&mut self, amount: u64) -> Result<(), String> {
        self.checkpoint()?;
        self.work = self
            .work
            .checked_add(amount)
            .filter(|work| *work <= self.max_work)
            .ok_or("source projection work budget exceeded")?;
        Ok(())
    }
    fn charge_read(&mut self, bytes: u64, state_factor: u64) -> Result<(), String> {
        self.checkpoint()?;
        self.input = self
            .input
            .checked_add(bytes)
            .filter(|total| *total <= self.max_input)
            .ok_or("source projection selected-input byte budget exceeded")?;
        let estimate = bytes
            .checked_mul(state_factor)
            .ok_or("source projection retained-state estimate overflow")?;
        self.charge_state(estimate)
    }
    fn charge_state(&mut self, bytes: u64) -> Result<(), String> {
        self.checkpoint()?;
        self.state = self
            .state
            .checked_add(bytes)
            .filter(|total| *total <= self.max_state)
            .ok_or("source projection retained-state budget exceeded")?;
        Ok(())
    }
    fn charge_row(&mut self) -> Result<(), String> {
        self.rows = self
            .rows
            .checked_add(1)
            .filter(|rows| *rows <= self.max_rows)
            .ok_or("source projection catalog row budget exceeded")?;
        self.tick(1)
    }
    fn charge_external_usage(&mut self, bytes: u64, rows: u64) -> Result<(), String> {
        self.input = self
            .input
            .checked_add(bytes)
            .filter(|total| *total <= self.max_input)
            .ok_or("source projection selected-input byte budget exceeded")?;
        self.charge_state(
            bytes
                .checked_mul(4)
                .ok_or("source projection retained-state estimate overflow")?,
        )?;
        self.rows = self
            .rows
            .checked_add(rows)
            .filter(|total| *total <= self.max_rows)
            .ok_or("source projection row budget exceeded")?;
        self.tick(rows)
    }
    fn remaining_input(&self) -> u64 {
        self.max_input.saturating_sub(self.input)
    }
}

#[derive(Clone)]
enum SourceCandidateKind {
    Record(String),
    Claim,
    Native,
    Contract,
}

struct RootFence {
    root: File,
    meter: Meter,
    files: BTreeMap<String, FileFence>,
    absent: BTreeSet<String>,
    directories: BTreeMap<String, Stamp>,
    directory_entries: u64,
}
impl RootFence {
    fn open(
        root: &Path,
        input_bytes: u64,
        max_rows: u64,
        max_state_bytes: u64,
        deadline: Instant,
    ) -> Result<Self, String> {
        if !root.is_absolute()
            || root.components().any(|part| {
                !matches!(
                    part,
                    std::path::Component::RootDir | std::path::Component::Normal(_)
                )
            })
        {
            return Err("source projection --root must be a normalized absolute directory".into());
        }
        let root_fd = tos_fd_open::open_absolute_directory(root)
            .map_err(|_| "source projection root is not a safe directory")?;
        Ok(Self {
            root: root_fd,
            meter: Meter {
                deadline,
                max_input: input_bytes,
                input: 0,
                max_state: max_state_bytes,
                state: 0,
                max_work: 100_000_000,
                work: 0,
                max_rows,
                rows: 0,
            },
            files: BTreeMap::new(),
            absent: BTreeSet::new(),
            directories: BTreeMap::new(),
            directory_entries: 0,
        })
    }

    fn open_dir_ref(&self, reference: &str) -> Result<File, String> {
        let mut directory = self
            .root
            .try_clone()
            .map_err(|_| "source projection root descriptor unavailable")?;
        if reference.is_empty() {
            return Ok(directory);
        }
        for part in path_parts(reference)? {
            directory = tos_fd_open::open_directory_at(&directory, Path::new(part))
                .map_err(|_| "source projection selected directory changed or is unsafe")?;
        }
        Ok(directory)
    }

    fn split_parent(reference: &str) -> Result<(&str, &str), String> {
        validate_relative_ref(reference)?;
        reference
            .rsplit_once('/')
            .ok_or("source projection selected file has no parent")
    }

    fn names_in(&mut self, directory: &File) -> Result<Vec<(String, fs::FileType)>, String> {
        let path = PathBuf::from(format!("/proc/self/fd/{}", directory.as_raw_fd()));
        let entries =
            fs::read_dir(path).map_err(|_| "source projection directory cannot be enumerated")?;
        let mut names = Vec::new();
        for entry in entries {
            self.meter.checkpoint()?;
            let entry = entry.map_err(|_| "source projection directory entry cannot be read")?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| "source projection contains a non-UTF-8 path component")?;
            let kind = entry
                .file_type()
                .map_err(|_| "source projection directory entry type cannot be read")?;
            self.directory_entries = self
                .directory_entries
                .checked_add(1)
                .filter(|count| *count <= MAX_WALK_ENTRIES)
                .ok_or("source projection directory-entry budget exceeded")?;
            self.meter.charge_state(name.len() as u64 + 128)?;
            self.meter.tick(1)?;
            names.push((name, kind));
        }
        let sort_work = (names.len() as u64)
            .checked_mul(usize::BITS as u64 - names.len().max(1).leading_zeros() as u64)
            .ok_or("source projection directory sort budget overflow")?;
        self.meter.tick(sort_work)?;
        names.sort_by(|left, right| left.0.cmp(&right.0));
        Ok(names)
    }

    fn read_optional(
        &mut self,
        reference: &str,
        cap: usize,
        state_factor: u64,
    ) -> Result<Option<Vec<u8>>, String> {
        let (parent_ref, leaf) = Self::split_parent(reference)?;
        let parent = self.open_dir_ref(parent_ref)?;
        let before_parent = parent
            .metadata()
            .map_err(|_| "source projection selected parent cannot be stated")?;
        let parent_stamp = Stamp::read(&before_parent);
        self.record_directory(parent_ref, &parent, parent_stamp)?;
        let file = match tos_fd_open::open_regular_at(&parent, Path::new(leaf)) {
            Ok(file) => file,
            Err(error)
                if error
                    .source
                    .as_ref()
                    .is_some_and(|source| source.kind() == io::ErrorKind::NotFound) =>
            {
                let after_parent = Stamp::read(
                    &parent
                        .metadata()
                        .map_err(|_| "source projection selected parent cannot be restated")?,
                );
                if parent_stamp != after_parent {
                    return Err("source projection parent changed during file selection".into());
                }
                if self.files.contains_key(reference) {
                    return Err("source projection selected file disappeared".into());
                }
                self.absent.insert(reference.to_owned());
                return Ok(None);
            }
            Err(_) => {
                return Err("source projection selected file is not a safe regular file".into());
            }
        };
        if self.absent.contains(reference) {
            return Err("source projection selected absent file appeared".into());
        }
        let before = file
            .metadata()
            .map_err(|_| "source projection selected file cannot be stated")?;
        let before_stamp = Stamp::read(&before);
        if before_stamp.len > cap as u64 {
            return Err("source projection selected file byte budget exceeded".into());
        }
        self.meter.charge_read(before_stamp.len, state_factor)?;
        let reserve = usize::try_from(before_stamp.len)
            .map_err(|_| "source projection selected file size does not fit memory")?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(reserve)
            .map_err(|_| "source projection selected file allocation refused")?;
        (&file)
            .take(cap as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "source projection selected file cannot be read")?;
        if bytes.len() > cap {
            return Err("source projection selected file byte budget exceeded".into());
        }
        let after_stamp = Stamp::read(
            &file
                .metadata()
                .map_err(|_| "source projection selected file cannot be restated")?,
        );
        if before_stamp != after_stamp || bytes.len() as u64 != before_stamp.len {
            return Err("source projection selected file changed during read".into());
        }
        let after_parent = Stamp::read(
            &parent
                .metadata()
                .map_err(|_| "source projection selected parent cannot be restated")?,
        );
        if parent_stamp != after_parent {
            return Err("source projection parent changed during file read".into());
        }
        let sha256 = Digest256::of_bytes(&bytes).to_hex();
        if let Some(previous) = self.files.get(reference) {
            if previous.stamp != before_stamp || previous.sha256 != sha256 {
                return Err("source projection selected file changed between reads".into());
            }
        } else {
            self.files.insert(
                reference.to_owned(),
                FileFence {
                    stamp: before_stamp,
                    sha256,
                    cap,
                },
            );
        }
        Ok(Some(bytes))
    }

    fn read_required(
        &mut self,
        reference: &str,
        cap: usize,
        state_factor: u64,
    ) -> Result<Vec<u8>, String> {
        self.read_optional(reference, cap, state_factor)?
            .ok_or_else(|| "source projection selected file is missing".into())
    }

    fn record_directory(
        &mut self,
        reference: &str,
        directory: &File,
        stamp: Stamp,
    ) -> Result<(), String> {
        self.meter.tick(1)?;
        if let Some(previous) = self.directories.insert(reference.to_owned(), stamp) {
            if previous != stamp {
                return Err("source projection directory changed during selection".into());
            }
        }
        let _ = directory;
        Ok(())
    }

    fn walk_tree<F, S>(
        &mut self,
        directory: File,
        relative_dir: String,
        depth: usize,
        selected_limit: usize,
        selected_limit_error: &'static str,
        select: &mut F,
        skip_subtree: &mut S,
        selected: &mut Vec<(String, SourceCandidateKind)>,
    ) -> Result<(), String>
    where
        F: FnMut(&str, &fs::FileType) -> Result<Option<SourceCandidateKind>, String>,
        S: FnMut(&str) -> bool,
    {
        self.meter.checkpoint()?;
        if depth > MAX_WALK_DEPTH {
            return Err("source projection directory depth budget exceeded".into());
        }
        let before = Stamp::read(
            &directory
                .metadata()
                .map_err(|_| "source projection selected directory cannot be stated")?,
        );
        let entries = self.names_in(&directory)?;
        for (name, kind) in entries {
            self.meter.checkpoint()?;
            let path = if relative_dir.is_empty() {
                name.clone()
            } else {
                format!("{relative_dir}/{name}")
            };
            if path == "ToS/source-witnesses/owner-local" {
                return Err("reserved owner-local metadata home blocks source coverage".into());
            }
            let candidate = select(&path, &kind)?;
            match (kind.is_dir(), kind.is_file(), kind.is_symlink()) {
                (true, _, _) => {
                    let child = tos_fd_open::open_directory_at(&directory, Path::new(&name))
                        .map_err(|_| "source projection selected directory is unsafe")?;
                    let child_stamp =
                        Stamp::read(&child.metadata().map_err(
                            |_| "source projection selected directory cannot be stated",
                        )?);
                    if skip_subtree(&path) {
                        self.record_directory(&path, &child, child_stamp)?;
                        continue;
                    }
                    self.walk_tree(
                        child,
                        path,
                        depth + 1,
                        selected_limit,
                        selected_limit_error,
                        select,
                        skip_subtree,
                        selected,
                    )?;
                }
                (false, true, false) => {
                    if let Some(candidate) = candidate {
                        if selected.len() >= selected_limit {
                            return Err(selected_limit_error.into());
                        }
                        self.meter.charge_state(path.len() as u64 + 128)?;
                        selected.push((path, candidate));
                    }
                }
                (false, false, true) => {
                    if candidate.is_some() {
                        return Err("source projection selected file is a symlink".into());
                    }
                }
                _ => {
                    if candidate.is_some() {
                        return Err("source projection selected file is not regular".into());
                    }
                }
            }
        }
        let after = Stamp::read(
            &directory
                .metadata()
                .map_err(|_| "source projection selected directory cannot be restated")?,
        );
        if before != after {
            return Err("source projection directory changed during selection".into());
        }
        self.record_directory(&relative_dir, &directory, after)
    }

    fn bind_metadata_directories(&mut self) -> Result<(), String> {
        for home in ["ToS/contracts", SOURCE_HOME] {
            let directory = self.open_dir_ref(home)?;
            let mut selected = Vec::new();
            let mut no_selection = |_path: &str, _kind: &fs::FileType| Ok(None);
            let mut skip = |path: &str| {
                path.split('/').any(|part| {
                    matches!(
                        part,
                        "payload" | "private" | "local-content" | "catalog" | "owner-local"
                    )
                })
            };
            self.walk_tree(
                directory,
                home.to_owned(),
                0,
                MAX_SELECTED_FILES,
                "coverage directory fence",
                &mut no_selection,
                &mut skip,
                &mut selected,
            )?;
        }
        Ok(())
    }

    fn verify_currentness(&mut self) -> Result<(), String> {
        let files = self
            .files
            .iter()
            .map(|(path, fence)| (path.clone(), fence.cap, fence.stamp, fence.sha256.clone()))
            .collect::<Vec<_>>();
        for (path, cap, stamp, sha256) in files {
            self.meter.tick(1)?;
            let raw = self
                .read_optional(&path, cap, 2)?
                .ok_or_else(|| "source projection selected file disappeared".to_owned())?;
            if Digest256::of_bytes(&raw).to_hex() != sha256
                || self
                    .files
                    .get(&path)
                    .is_none_or(|fence| fence.stamp != stamp)
            {
                return Err("source projection selected file changed during observation".into());
            }
        }
        let absent = self.absent.iter().cloned().collect::<Vec<_>>();
        for path in absent {
            if self.read_optional(&path, 8192, 1)?.is_some() {
                return Err("source projection selected absent file appeared".into());
            }
        }
        let directories = self
            .directories
            .iter()
            .map(|(path, stamp)| (path.clone(), *stamp))
            .collect::<Vec<_>>();
        for (path, expected) in directories {
            self.meter.tick(1)?;
            let directory = self.open_dir_ref(&path)?;
            let actual = Stamp::read(
                &directory
                    .metadata()
                    .map_err(|_| "source projection selected directory cannot be restated")?,
            );
            if actual != expected {
                return Err(
                    "source projection selected directory changed during observation".into(),
                );
            }
        }
        Ok(())
    }
}

fn path_parts(reference: &str) -> Result<Vec<&str>, String> {
    validate_relative_ref(reference)?;
    Ok(reference.split('/').collect())
}

fn validate_relative_ref(reference: &str) -> Result<(), String> {
    if reference.is_empty()
        || reference.len() > 4096
        || reference.starts_with('/')
        || reference.contains(['\\', '\0'])
        || reference
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err("source projection selected path is not normalized".into());
    }
    Ok(())
}

fn public_source_path(reference: &str) -> bool {
    reference.starts_with("ToS/")
        && reference.len() <= 4096
        && !reference.contains(['\\', '\0'])
        && reference.split('/').all(|part| {
            !part.is_empty()
                && !part.starts_with('.')
                && !matches!(
                    part,
                    "catalog" | "payload" | "private" | "owner-local" | "local-content"
                )
        })
}

fn json_limits(cap: usize) -> JsonLimits {
    JsonLimits {
        max_bytes: cap,
        max_depth: 96,
        max_visits: 1_000_000,
        max_integer_digits: 4_300,
    }
}

fn parse_source_json(raw: &[u8], cap: usize) -> Result<Value, String> {
    let document = parse_json(raw, JsonMode::PublishedStrict, json_limits(cap))
        .map_err(|error| error.to_string())?;
    let preserved = tos_foundation::emit_value_preserved_json(document.root(), json_limits(cap))
        .map_err(|error| error.to_string())?;
    serde_json::from_slice(&preserved).map_err(|error| error.to_string())
}

fn read_input(path: &str, cap: usize, deadline: Instant) -> Result<Vec<u8>, String> {
    if path != "-" {
        let file = tos_fd_open::open_absolute_regular(Path::new(path), cap as u64)
            .map_err(|_| "source projection input is not a safe regular file")?;
        let before = Stamp::read(
            &file
                .metadata()
                .map_err(|_| "source projection input cannot be stated")?,
        );
        if before.len > cap as u64 {
            return Err("source projection input byte budget exceeded".into());
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(before.len as usize)
            .map_err(|_| "source projection input allocation refused")?;
        let mut chunk = [0u8; 65_536];
        loop {
            if Instant::now() >= deadline {
                return Err("source projection input deadline exceeded".into());
            }
            let read = (&file)
                .take(cap as u64 + 1 - bytes.len() as u64)
                .read(&mut chunk)
                .map_err(|_| "source projection input cannot be read")?;
            if read == 0 {
                break;
            }
            if read > cap.saturating_sub(bytes.len()) {
                return Err("source projection input byte budget exceeded".into());
            }
            bytes.extend_from_slice(&chunk[..read]);
        }
        if Stamp::read(
            &file
                .metadata()
                .map_err(|_| "source projection input cannot be restated")?,
        ) != before
        {
            return Err("source projection input changed during read".into());
        }
        return Ok(bytes);
    }
    let mut stdin = io::stdin();
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 65_536];
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or("source projection input deadline exceeded")?;
        let mut pollfd = libc::pollfd {
            fd: 0,
            events: libc::POLLIN,
            revents: 0,
        };
        let millis = remaining.as_millis().clamp(1, i32::MAX as u128) as i32;
        let ready = unsafe { libc::poll(&mut pollfd, 1, millis) };
        if ready == 0 {
            return Err("source projection input deadline exceeded".into());
        }
        if ready < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error.to_string());
        }
        let read = stdin.read(&mut chunk).map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        if read > cap.saturating_sub(bytes.len()) {
            return Err("source projection input byte budget exceeded".into());
        }
        bytes.extend_from_slice(&chunk[..read]);
    }
    Ok(bytes)
}

fn required<'a>(value: &'a Value, name: &str) -> Result<&'a Value, String> {
    value
        .get(name)
        .ok_or_else(|| format!("source projection request missing {name}"))
}

fn execute_observe(input: &Value, output: &mut dyn Write) -> Result<(), String> {
    let identity = string(input, "identity")?;
    let source_ref = string(input, "source_ref")?;
    let kind = string(input, "kind")?;
    let adapter = string(input, "adapter")?;
    let candidates = required(input, "candidates")?
        .as_array()
        .ok_or("source projection candidates must be an array")?;
    let source_line = input.get("source_line").and_then(Value::as_u64);
    emit(
        output,
        &observe_record(
            identity,
            required(input, "record")?,
            source_ref,
            candidates,
            kind,
            adapter,
            source_line,
        )?,
    )
}

struct CatalogCurrentness {
    catalog_sha256: String,
}

fn selected_publication(fence: &mut RootFence) -> Result<MetadataPublicationEpoch, String> {
    let raw = fence.read_optional(PUBLICATION_REF, 8192, 8)?;
    let state = raw
        .as_deref()
        .map(|bytes| {
            parse_json(bytes, JsonMode::PublishedStrict, json_limits(8192))
                .map(|document| document.into_root())
                .map_err(|error| error.to_string())
        })
        .transpose()?;
    MetadataPublicationEpoch::select(state)
        .map_err(|error| format!("selected metadata publication is incomplete: {error}"))
}

fn verify_publication_current(
    fence: &mut RootFence,
    publication: &MetadataPublicationEpoch,
) -> Result<(), String> {
    let current_raw = fence.read_optional(PUBLICATION_REF, 8192, 2)?;
    let current_state = current_raw
        .as_deref()
        .map(|bytes| {
            parse_json(bytes, JsonMode::PublishedStrict, json_limits(8192))
                .map(|document| document.into_root())
                .map_err(|error| error.to_string())
        })
        .transpose()?;
    publication
        .verify_current(current_state)
        .map_err(|error| format!("selected metadata publication changed: {error}"))
}

fn verify_source_snapshot(
    fence: &mut RootFence,
    publication: &MetadataPublicationEpoch,
) -> Result<(), String> {
    fence.verify_currentness()?;
    verify_publication_current(fence, publication)
}

fn claim_rows(raw: &[u8], max_rows: u64) -> Result<Vec<(u64, &[u8])>, String> {
    let text = std::str::from_utf8(raw).map_err(|_| "source projection claim file is not UTF-8")?;
    let mut rows = Vec::new();
    let mut line_start = 0usize;
    let mut line_number = 1u64;
    let mut chars = text.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        let is_separator = matches!(
            ch,
            '\n' | '\r'
                | '\u{000b}'
                | '\u{000c}'
                | '\u{001c}'
                | '\u{001d}'
                | '\u{001e}'
                | '\u{0085}'
                | '\u{2028}'
                | '\u{2029}'
        );
        if !is_separator {
            continue;
        }
        let mut end = index + ch.len_utf8();
        if ch == '\r' && chars.peek().is_some_and(|(_, next)| *next == '\n') {
            let (next_index, _) = chars.next().expect("peeked CRLF continuation");
            end = next_index + 1;
        }
        let line = &text[line_start..index];
        if !line.trim().is_empty() {
            if rows.len() as u64 >= max_rows {
                return Err("source projection catalog row budget exceeded".into());
            }
            rows.push((line_number, &raw[line_start..index]));
        }
        line_number = line_number
            .checked_add(1)
            .ok_or("source projection Claim line overflow")?;
        line_start = end;
    }
    let line = &text[line_start..];
    if !line.trim().is_empty() {
        if rows.len() as u64 >= max_rows {
            return Err("source projection catalog row budget exceeded".into());
        }
        rows.push((line_number, &raw[line_start..]));
    }
    Ok(rows)
}

fn load_source_catalog(
    root: &Path,
    invocation: &str,
    max_input_bytes: u64,
    max_rows: u64,
    max_state_bytes: u64,
    deadline: Instant,
) -> Result<
    (
        RootFence,
        MetadataPublicationEpoch,
        Vec<SourceEntry>,
        BTreeMap<String, String>,
        Vec<String>,
        CatalogCurrentness,
    ),
    String,
> {
    use tos_command::source_current_cut::foundation_command::{
        SourceFoundationCatalogueObservationLimits, run_with_owned_catalogue_observation,
    };
    let mut fence = RootFence::open(root, max_input_bytes, max_rows, max_state_bytes, deadline)?;
    let publication = selected_publication(&mut fence)?;
    fence.bind_metadata_directories()?;
    let cancelled = AtomicBool::new(false);
    let git_signal = AtomicI32::new(0);
    let native_args = [
        std::ffi::OsString::from("--repo-root"),
        root.as_os_str().to_owned(),
        std::ffi::OsString::from("--invocation"),
        std::ffi::OsString::from(invocation),
    ];
    let mut entries = Vec::new();
    let mut hashes = BTreeMap::new();
    let mut complete = None;
    let mut captured_error = None;
    let observation_limits = CaptureObservationLimits {
        max_addressed_rows: max_rows,
        max_addressed_bytes: usize::try_from(max_input_bytes)
            .map_err(|_| "coverage address byte range")?,
        max_source_read_bytes: max_input_bytes,
        max_state_bytes: usize::try_from(max_state_bytes)
            .map_err(|_| "coverage retained state byte range")?,
        deadline,
    };
    let result = run_with_owned_catalogue_observation(
        &native_args,
        &cancelled,
        &git_signal,
        &mut io::sink(),
        &mut io::sink(),
        SourceFoundationCatalogueObservationLimits {
            max_stage_read_bytes: max_input_bytes,
            // RootFence and catalogue observer retain state simultaneously.
            // Reservation is deducted from the unchanged invocation grant.
            max_state_bytes: usize::try_from(max_state_bytes)
                .map_err(|_| "coverage retained state byte range")?
                .checked_mul(2)
                .ok_or("coverage shared state reservation overflow")?,
        },
        |stage, receipt, catalogue_limits| {
            let mut populate = || -> Result<(), String> {
                // Bind the actual held contract inventory too, so a later edit
                // cannot keep identical source rows while changing their rules.
                let mut after = None;
                loop {
                    let page = stage
                        .scan_input(
                            tos_compiler::source_witness_catalog::CATALOG_SOURCE,
                            tos_compiler::source_witness_catalog::CONTRACT_FILES,
                            after.as_deref(),
                            1,
                        )
                        .map_err(|e| e.to_string())?;
                    for row in page.rows {
                        fence.meter.charge_read(row.payload.len() as u64, 8)?;
                        let root_bytes = fence.read_required(&row.id, MAX_SOURCE_FILE_BYTES, 8)?;
                        if Digest256::of_bytes(&root_bytes).to_hex() != row.payload_sha256 {
                            return Err(
                                "coverage root contracts differ from genuine captured catalogue"
                                    .into(),
                            );
                        }
                    }
                    after = page.next_id;
                    if after.is_none() {
                        break;
                    }
                }

                observe_owned_catalogue(
                    stage,
                    receipt,
                    catalogue_limits,
                    observation_limits,
                    |row| {
                        let mut append = || -> Result<usize, String> {
                            fence.meter.charge_row()?;
                            if !hashes.contains_key(row.source_ref) {
                                // Stage and root reads are separate physical work.
                                fence.meter.charge_read(row.source_file_bytes, 8)?;
                                if !public_source_path(row.source_ref) {
                                    return Err("coverage catalogue selected nonpublic path".into());
                                }
                                let raw = fence.read_required(
                                    row.source_ref,
                                    MAX_SOURCE_FILE_BYTES,
                                    8,
                                )?;
                                let digest = Digest256::of_bytes(&raw).to_hex();
                                if digest != row.source_file_sha256.to_hex() {
                                    return Err(
                                        "coverage root differs from captured source file".into()
                                    );
                                }
                                remember_source_hash(
                                    &mut fence.meter,
                                    &mut hashes,
                                    row.source_ref,
                                    &digest,
                                )?;
                            }
                            let source_bytes =
                                canonical_bytes(&row.source_record, MAX_SOURCE_FILE_BYTES)?;
                            let retained = source_bytes
                                .len()
                                .checked_add(
                                    canonical_bytes(row.catalogue_entry, MAX_OUTPUT_ROW_BYTES)?
                                        .len(),
                                )
                                .and_then(|n| n.checked_mul(128))
                                .and_then(|n| {
                                    n.checked_add(
                                        row.identity.len()
                                            + row.source_ref.len()
                                            + row.kind.len()
                                            + 1024,
                                    )
                                })
                                .ok_or("coverage callback retained state overflow")?;
                            if retained > row.retained_state_budget {
                                return Err("coverage shared callback state budget exceeded".into());
                            }
                            fence.meter.charge_state(retained as u64)?;
                            let adapter = if row.kind == "claim" {
                                "reified-claim"
                            } else if matches!(row.kind, "artifact" | "composite") {
                                "native-witness"
                            } else {
                                "source-record"
                            };
                            entries.push(SourceEntry {
                                identity: row.identity.to_owned(),
                                record: row.source_record,
                                catalog_entry: row.catalogue_entry.clone(),
                                source_ref: row.source_ref.to_owned(),
                                source_file_sha256: row.source_file_sha256.to_hex(),
                                kind: row.kind.to_owned(),
                                adapter: adapter.to_owned(),
                                source_line: row.source_line,
                            });
                            Ok(retained)
                        };
                        append().map_err(tos_compiler::Error::Source)
                    },
                )
                .map_err(|e| format!("coverage genuine catalogue observation: {e}"))?;
                let files = receipt.manifest["record_files"]
                    .as_object()
                    .ok_or("coverage genuine catalogue record files")?;
                let kinds = files.keys().cloned().collect::<Vec<_>>();
                let currentness = CatalogCurrentness {
                    catalog_sha256: string(&receipt.manifest, "catalog_sha256")?.to_owned(),
                };
                for reference in receipt.file_sha256.keys() {
                    fence.read_required(reference, MAX_OUTPUT_FILE_BYTES, 2)?;
                }
                fence.read_required(
                    &format!("{CATALOG_HOME}/catalog.manifest.json"),
                    MAX_OUTPUT_FILE_BYTES,
                    2,
                )?;
                complete = Some((kinds, currentness));
                Ok(())
            };
            populate().map_err(|error| {
                captured_error = Some(error.clone());
                tos_compiler::Error::Source(error)
            })
        },
    );
    if let Some(error) = captured_error {
        return Err(error);
    }
    if result.map_err(|e| format!("coverage native foundation refused: {e:?}"))? != 0 {
        return Err("coverage native foundation did not complete".into());
    }
    let (kinds, currentness) =
        complete.ok_or("coverage native foundation produced no complete catalogue observation")?;
    verify_source_snapshot(&mut fence, &publication)?;
    Ok((fence, publication, entries, hashes, kinds, currentness))
}

fn open_store(
    root: &Path,
    max_input_bytes: u64,
    max_rows: u64,
    deadline: Instant,
) -> Result<tos_query::source_diagnostic::LegacyStore, String> {
    crate::coverage::open_knowledge_store(
        root,
        Limits {
            max_input_bytes,
            max_json_bytes: MAX_SOURCE_ROW_BYTES,
            max_rows,
            max_work_steps: 100_000_000,
            max_sql_vm_steps: 100_000_000,
            sqlite_cache_kib: 8192,
        },
        deadline,
    )
}

fn graph_revision(graph: &Value) -> Result<Value, String> {
    let revision = required(graph, "source_revision")?;
    if !revision.is_string() {
        return Err("source projection graph needs a string source_revision".into());
    }
    Ok(revision.clone())
}

fn execute_imported_graph(
    graph: &Value,
    entries: &[SourceEntry],
    source_hashes: &BTreeMap<String, String>,
    kinds: &[String],
    catalog_sha256: &str,
    fence: &mut RootFence,
    publication: &MetadataPublicationEpoch,
    deadline: Instant,
    rows: bool,
    output: &mut dyn Write,
) -> Result<(), String> {
    let revision = graph_revision(graph)?;
    let node_list = required(graph, "nodes")?
        .as_array()
        .ok_or("source projection graph nodes must be an array")?;
    let relation_list = required(graph, "relations")?
        .as_array()
        .ok_or("source projection graph relations must be an array")?;
    let by_identity = entries
        .iter()
        .cloned()
        .map(|entry| (entry.identity.clone(), entry))
        .collect::<BTreeMap<_, _>>();
    let known = by_identity.keys().cloned().collect::<BTreeSet<_>>();
    let mut candidates = BTreeMap::<String, IdentityObservation>::new();
    for item in node_list.iter().chain(relation_list.iter()) {
        fence.meter.charge_row()?;
        add_carrier(
            item,
            &by_identity,
            &known,
            &mut candidates,
            &mut fence.meter,
        )?;
    }
    verify_source_snapshot(fence, publication)?;
    let source_hash_map_encoded_len =
        reserve_source_hash_map_digest(&mut fence.meter, source_hashes)?;
    let files_digest = source_hash_map_digest(
        source_hashes,
        source_hash_map_encoded_len,
        &mut fence.meter,
        deadline,
    )?;
    report_summary(
        entries,
        source_hashes.len(),
        &files_digest,
        kinds,
        &revision,
        catalog_sha256,
        &candidates,
        rows,
        deadline,
        || verify_source_snapshot(fence, publication),
        output,
    )
}

fn execute_native_store(
    entries: &[SourceEntry],
    source_hashes: &BTreeMap<String, String>,
    kinds: &[String],
    catalog_sha256: &str,
    root: &Path,
    fence: &mut RootFence,
    publication: &MetadataPublicationEpoch,
    deadline: Instant,
    rows: bool,
    output: &mut dyn Write,
) -> Result<(), String> {
    let remaining_input = fence.meter.remaining_input();
    if remaining_input == 0 {
        return Err(
            "source projection selected-input byte budget exhausted before graph read".into(),
        );
    }
    let remaining_rows = fence.meter.max_rows.saturating_sub(fence.meter.rows);
    if remaining_rows == 0 {
        return Err("source projection row budget exhausted before graph read".into());
    }
    let mut store = open_store(root, remaining_input, remaining_rows, deadline)?;
    let revision = store
        .graph_header
        .get("source_revision")
        .filter(|value| value.is_string())
        .cloned()
        .ok_or("native source diagnostic graph header has no string source_revision")?;
    let by_identity = entries
        .iter()
        .cloned()
        .map(|entry| (entry.identity.clone(), entry))
        .collect::<BTreeMap<_, _>>();
    let known = by_identity.keys().cloned().collect::<BTreeSet<_>>();
    let mut candidates = BTreeMap::<String, IdentityObservation>::new();
    let mut observed_rows = 0u64;
    let counts = store
        .visit_knowledge_carriers(|item| {
            observed_rows = observed_rows.checked_add(1).ok_or_else(|| {
                tos_query::source_diagnostic::DiagnosticError(
                    "source projection graph row overflow".into(),
                )
            })?;
            add_carrier(
                item,
                &by_identity,
                &known,
                &mut candidates,
                &mut fence.meter,
            )
            .map_err(tos_query::source_diagnostic::DiagnosticError)
        })
        .map_err(|error| error.to_string())?;
    if counts.0.checked_add(counts.1) != Some(observed_rows) {
        return Err("source projection graph enumeration count mismatch".into());
    }
    let (charged_bytes, charged_rows) = store.resource_usage();
    fence
        .meter
        .charge_external_usage(charged_bytes, charged_rows)?;
    store
        .verify_currentness()
        .map_err(|error| error.to_string())?;
    fence.verify_currentness()?;
    let source_hash_map_encoded_len =
        reserve_source_hash_map_digest(&mut fence.meter, source_hashes)?;
    let files_digest = source_hash_map_digest(
        source_hashes,
        source_hash_map_encoded_len,
        &mut fence.meter,
        deadline,
    )?;
    report_summary(
        entries,
        source_hashes.len(),
        &files_digest,
        kinds,
        &revision,
        catalog_sha256,
        &candidates,
        rows,
        deadline,
        || {
            store
                .verify_currentness()
                .map_err(|error| error.to_string())?;
            verify_source_snapshot(fence, publication)
        },
        output,
    )
}

fn execute_root(
    root: &Path,
    invocation: &str,
    graph: Option<&str>,
    input_bytes: u64,
    max_rows: u64,
    max_state_bytes: u64,
    deadline: Instant,
    rows: bool,
    output: &mut dyn Write,
) -> Result<(), String> {
    let (mut fence, publication, entries, source_hashes, kinds, currentness) =
        load_source_catalog(root, invocation, input_bytes, max_rows, max_state_bytes, deadline)?;
    if let Some(graph_path) = graph {
        let remaining = fence.meter.remaining_input();
        if remaining == 0 {
            return Err(
                "source projection selected-input byte budget exhausted before graph input".into(),
            );
        }
        let cap = usize::try_from(remaining.min(usize::MAX as u64))
            .map_err(|_| "source projection graph input cap does not fit memory")?;
        let raw = read_input(graph_path, cap, deadline)?;
        fence.meter.charge_read(raw.len() as u64, 4)?;
        let graph_value = parse_source_json(&raw, cap)?;
        execute_imported_graph(
            &graph_value,
            &entries,
            &source_hashes,
            &kinds,
            &currentness.catalog_sha256,
            &mut fence,
            &publication,
            deadline,
            rows,
            output,
        )
    } else {
        execute_native_store(
            &entries,
            &source_hashes,
            &kinds,
            &currentness.catalog_sha256,
            root,
            &mut fence,
            &publication,
            deadline,
            rows,
            output,
        )
    }
}

fn execute(args: &[String], output: &mut dyn Write) -> Result<(), String> {
    let mut root = None;
    let mut graph = None;
    let mut invocation = None;
    let mut input = None;
    let mut observe = false;
    let mut rows = false;
    let mut max_input_bytes = INPUT_CAP;
    let mut max_rows = MAX_ROWS;
    let mut max_state_bytes = MAX_STATE_BYTES;
    let mut max_seconds = 120u64;
    let mut seen = BTreeSet::new();
    let mut index = 1;
    while index < args.len() {
        let flag = args[index].as_str();
        index += 1;
        if flag == "--rows" {
            if !seen.insert(flag.to_owned()) {
                return Err("duplicate source projection --rows option".into());
            }
            rows = true;
            continue;
        }
        if flag == "--observe-record" {
            if !seen.insert(flag.to_owned()) {
                return Err("duplicate source projection --observe-record option".into());
            }
            observe = true;
            continue;
        }
        let value = args
            .get(index)
            .ok_or("source projection option needs a value")?;
        index += 1;
        if !seen.insert(flag.to_owned()) {
            return Err(format!("duplicate source projection option {flag}"));
        }
        match flag {
            "--root" => root = Some(PathBuf::from(value)),
            "--invocation" => invocation = Some(value.as_str()),
            "--graph" => graph = Some(value.as_str()),
            "--input" => input = Some(value.as_str()),
            "--max-input-bytes" => {
                max_input_bytes = value
                    .parse::<u64>()
                    .map_err(|_| "invalid source projection input byte budget")?
            }
            "--max-state-bytes" => {
                max_state_bytes = value.parse::<u64>()
                    .map_err(|_| "invalid source projection retained state budget")?
            }
            "--max-rows" => {
                max_rows = value
                    .parse::<u64>()
                    .map_err(|_| "invalid source projection row budget")?
            }
            "--max-seconds" => {
                max_seconds = value
                    .parse::<u64>()
                    .map_err(|_| "invalid source projection deadline")?
            }
            _ => return Err(format!("unknown source projection option {flag}")),
        }
    }
    if max_input_bytes == 0
        || max_input_bytes > INPUT_CAP
        || max_state_bytes == 0
        || max_state_bytes > MAX_STATE_BYTES
        || max_rows == 0
        || max_rows > MAX_ROWS
        || max_seconds == 0
        || max_seconds > 3600
    {
        return Err("source projection budget outside supported envelope".into());
    }
    let deadline = Instant::now()
        .checked_add(Duration::from_secs(max_seconds))
        .ok_or("source projection deadline arithmetic")?;
    if observe {
        if rows || root.is_some() || graph.is_some() || invocation.is_some() {
            return Err("--observe-record does not accept root, graph or rows options".into());
        }
        let path = input.ok_or("--observe-record requires --input")?;
        let value = parse_source_json(&read_input(path, REQUEST_CAP, deadline)?, REQUEST_CAP)?;
        return execute_observe(&value, output);
    }
    if input.is_some() || root.is_none() {
        return Err(
            "source-projection-coverage requires --root; --input is only for --observe-record"
                .into(),
        );
    }
    execute_root(
        root.as_deref().expect("checked root"),
        invocation
            .ok_or("source-projection-coverage requires protected native --invocation ABS")?,
        graph,
        max_input_bytes,
        max_rows,
        max_state_bytes,
        deadline,
        rows,
        output,
    )
}

pub fn run_if_requested(
    args: &[String],
    output: &mut dyn Write,
    error: &mut dyn Write,
) -> Option<i32> {
    if !args
        .first()
        .is_some_and(|arg| arg == "source-projection-coverage")
    {
        return None;
    }
    if args.len() == 2 && matches!(args[1].as_str(), "--help" | "-h") {
        let _ = writeln!(
            output,
            "source-projection-coverage --root ABS --invocation ABS [--graph ABS_JSON|-] [--rows] [--max-input-bytes N --max-rows N --max-state-bytes N --max-seconds N]\nsource-projection-coverage --observe-record --input ABS|-\nA stream without its terminal summary is incomplete. No assessment, admission or source mutation."
        );
        return Some(0);
    }
    Some(match execute(args, output) {
        Ok(()) => 0,
        Err(message) => {
            let _ = writeln!(error, "source_projection_coverage_refused: {message}");
            2
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_and_null_fields_participate_in_exact_projection_comparison() {
        let record = json!({
            "record_id":"tos.agent.synthetic",
            "unknown_extension":{"word":"λόγος","explicit_null":null,"flag":true}
        });
        let source_ref = "ToS/source-witnesses/agents/synthetic/agent.json";
        let exact = json!({
            "id":"carrier:exact",
            "source_refs":[source_ref],
            "type_mapping":{"status":"mapped"},
            "attributes":{"source_record":record.clone()}
        });
        let mut changed = record.clone();
        changed["unknown_extension"]["explicit_null"] = json!(false);
        let different = json!({
            "id":"carrier:different",
            "source_refs":[source_ref],
            "type_mapping":{"status":"mapped"},
            "attributes":{"source_record":changed}
        });
        let row = observe_record(
            "tos.agent.synthetic",
            &record,
            source_ref,
            &[exact, different],
            "agent",
            "source-record",
            None,
        )
        .expect("source values are comparable");
        assert_eq!(row["state"], "conflicting-record-carriers");
        assert_eq!(row["carriers"][0]["raw_record_state"], "different");
        assert_eq!(row["carriers"][1]["raw_record_state"], "exact");
    }

    #[test]
    fn carrier_mapping_and_source_return_remain_independent() {
        let record = json!({"claim_id":"tos.claim.synthetic","evidence":[null]});
        let item = json!({
            "id":"claim:synthetic",
            "source_refs":["elsewhere"],
            "type_mapping":{"status":"mapped"},
            "attributes":{"source_claim":record.clone()}
        });
        let row = observe_record(
            "tos.claim.synthetic",
            &record,
            "ToS/source-witnesses/relations/synthetic/source-claims.jsonl",
            &[item],
            "claim",
            "reified-claim",
            Some(3),
        )
        .expect("source values are comparable");
        assert_eq!(row["state"], "requires-clarification");
        assert_eq!(row["carriers"][0]["source_return_present"], false);
        assert_eq!(
            row["carriers"][0]["record_pointer"],
            "/attributes/source_claim"
        );
    }

    #[test]
    fn claim_rows_preserve_splitlines_numbers_and_unicode_blank_lines() {
        let raw =
            "  \r\n{\"claim_id\":\"first\"}\r\n\u{00a0}\n{\"claim_id\":\"second\"}".as_bytes();
        let rows = claim_rows(raw, 2).expect("bounded Claims are split");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].0, 2);
        assert_eq!(rows[0].1, br#"{"claim_id":"first"}"#);
        assert_eq!(rows[1].0, 4);
        assert_eq!(rows[1].1, br#"{"claim_id":"second"}"#);
        assert!(claim_rows(raw, 1).is_err());
    }

    #[test]
    fn streamed_source_file_digest_matches_the_sorted_canonical_map() {
        let hashes = BTreeMap::from([
            (
                "ToS/source-witnesses/agents/α/\"agent.json\n".to_owned(),
                "01".repeat(32),
            ),
            (
                "ToS/source-witnesses/agents/😀/agent.json".to_owned(),
                "fe".repeat(32),
            ),
        ]);
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut meter = Meter {
            deadline,
            max_input: INPUT_CAP,
            input: 0,
            max_state: MAX_STATE_BYTES,
            state: 0,
            max_work: 100_000_000,
            work: 0,
            max_rows: MAX_ROWS,
            rows: 0,
        };
        let encoded_len = reserve_source_hash_map_digest(&mut meter, &hashes)
            .expect("small hash map has a bounded canonical encoding");
        let streamed = source_hash_map_digest(&hashes, encoded_len, &mut meter, deadline)
            .expect("streamed hash map is canonical");
        let expected_value = serde_json::to_value(&hashes).expect("hash map converts to JSON");
        assert_eq!(streamed, canonical_digest(&expected_value).unwrap());
    }
}
