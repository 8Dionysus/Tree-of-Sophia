//! Native source-to-projection coverage observation.
//!
//! This tool reports only the exact public source-catalog identities and the
//! fields present on normalized graph carriers. It never assesses meaning,
//! admits source, or mutates an owner surface.

use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{self, Read, Write},
    os::fd::AsRawFd,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};
use tos_compiler::source_witness_catalog::{
    SourceCatalogLimits, SourceRootClaimProfiles, SourceRootNativeMetadataReader,
};
use tos_foundation::{
    CanonicalProfile, Digest256, Digest256Hasher, JsonEmissionProfile, JsonLimits, JsonMode,
    JsonNumber, JsonNumberKind, JsonString, JsonValue, canonical_feed_digest_v1,
    canonical_raw_bytes_v1, emit_json_profile, parse_json,
};
use tos_query::source_diagnostic::Limits;
use tos_source_store::MetadataPublicationEpoch;

const SOURCE_HOME: &str = "ToS/source-witnesses";
const CATALOG_HOME: &str = "ToS/source-witnesses/catalog";
const PUBLICATION_REF: &str = "ToS/source-witnesses/.metadata-publication.json";
const COMPOSITE_SCHEMA: &str = "ToS/contracts/scholarly-composite-witness.schema.json";
const INPUT_CAP: u64 = 256 * 1024 * 1024;
const REQUEST_CAP: usize = 16 * 1024 * 1024;
const MAX_ROWS: u64 = 1_000_000;
const MAX_CONTRACT_FILES: u64 = 4096;
const MAX_SELECTED_FILES: usize = 16_384;
const MAX_NATIVE_PACKETS: usize = 1024;
const MAX_NATIVE_PACKET_BYTES: usize = 8 * 1024 * 1024;
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

const BASE_FAMILIES: [(&str, &str); 9] = [
    ("agent", "agents.jsonl"),
    ("place", "places.jsonl"),
    ("organization", "organizations.jsonl"),
    ("work", "works.jsonl"),
    ("expression", "expressions.jsonl"),
    ("edition", "editions.jsonl"),
    ("collection", "collections.jsonl"),
    ("item", "items.jsonl"),
    ("link", "links.jsonl"),
];
const ADAPTED_FAMILIES: [(&str, &str, &str); 2] = [
    ("artifact", "artifact-witness.json", "artifacts.jsonl"),
    ("composite", "composite-witness.json", "composites.jsonl"),
];
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
                max_state: MAX_STATE_BYTES,
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

    fn collect_contract_paths(&mut self, deadline: Instant) -> Result<Vec<String>, String> {
        let root = self.open_dir_ref("ToS/contracts")?;
        let mut selected = Vec::new();
        let mut select = |path: &str, kind: &fs::FileType| {
            if tos_compiler::source_witness_catalog::is_root_contract_path(path) {
                if !kind.is_file() {
                    return Err("source projection contract selector is not a regular file".into());
                }
                return Ok(Some(SourceCandidateKind::Contract));
            }
            Ok(None)
        };
        let mut skip = |_path: &str| false;
        self.walk_tree(
            root,
            "ToS/contracts".into(),
            0,
            MAX_CONTRACT_FILES as usize,
            "source projection contract-file budget exceeded",
            &mut select,
            &mut skip,
            &mut selected,
        )?;
        if Instant::now() >= deadline {
            return Err("source projection original deadline exceeded".into());
        }
        selected.sort_by(|left, right| left.0.cmp(&right.0));
        Ok(selected
            .into_iter()
            .map(|(path, candidate)| {
                debug_assert!(matches!(candidate, SourceCandidateKind::Contract));
                path
            })
            .collect())
    }

    fn collect_source_paths(
        &mut self,
        profiles: &SourceRootClaimProfiles,
    ) -> Result<Vec<(String, SourceCandidateKind)>, String> {
        let selected_names = profiles
            .source_basenames()
            .map_err(|error| error.to_string())?;
        let claim_names = profiles
            .claim_source_basenames()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        let mut record_kinds = BTreeMap::<String, String>::new();
        for (kind, _catalog_filename) in BASE_FAMILIES {
            record_kinds.insert(format!("{kind}.json"), kind.to_owned());
        }
        profiles
            .visit_record_profiles(|kind, basename, _catalog_filename| {
                if let Some(previous) = record_kinds.insert(basename.to_owned(), kind.to_owned()) {
                    if previous != kind {
                        return Err(tos_compiler::Error::Invalid(
                            "root coverage duplicate source basename",
                        ));
                    }
                }
                Ok(())
            })
            .map_err(|error| error.to_string())?;
        if record_kinds
            .keys()
            .any(|name| !selected_names.contains(name))
            || claim_names
                .iter()
                .any(|name| !selected_names.contains(name))
        {
            return Err("source projection owner basename selection is inconsistent".into());
        }
        let root = self.open_dir_ref(SOURCE_HOME)?;
        let mut selected = Vec::new();
        let mut select = |path: &str, kind: &fs::FileType| {
            let Some(basename) = path.rsplit('/').next() else {
                return Ok(None);
            };
            let native = basename.starts_with("semantic-annotation") && basename.ends_with(".json");
            let adapted = if basename == "artifact-witness.json"
                && path.starts_with(&format!("{SOURCE_HOME}/artifacts/"))
            {
                Some("artifact")
            } else if basename == "composite-witness.json"
                && path.starts_with(&format!("{SOURCE_HOME}/scholarly-composites/"))
            {
                Some("composite")
            } else {
                None
            };
            let ordinary = record_kinds.get(basename).cloned();
            let claim = claim_names.contains(basename);
            let candidate = if native {
                if path
                    .split('/')
                    .any(|part| matches!(part, "payload" | "local-content" | "catalog"))
                {
                    None
                } else {
                    Some(SourceCandidateKind::Native)
                }
            } else if let Some(kind) = adapted {
                Some(SourceCandidateKind::Record(kind.to_owned()))
            } else if let Some(kind) = ordinary {
                Some(SourceCandidateKind::Record(kind))
            } else if claim {
                Some(SourceCandidateKind::Claim)
            } else {
                None
            };
            if let Some(candidate) = candidate.as_ref() {
                if !native && !public_source_path(path) {
                    return Err(
                        "source projection selected path is outside the public metadata route"
                            .into(),
                    );
                }
                if native
                    && path.split('/').any(|part| {
                        matches!(part, "private" | "owner-local") || part.starts_with('.')
                    })
                {
                    return Err(
                        "source projection native identity locator is not public metadata".into(),
                    );
                }
                if !kind.is_file() {
                    return Err(
                        "source projection selected path is not a regular metadata file".into(),
                    );
                }
            }
            Ok(candidate)
        };
        let mut skip = |_path: &str| false;
        self.walk_tree(
            root,
            SOURCE_HOME.into(),
            0,
            MAX_SELECTED_FILES,
            "source projection selected-file budget exceeded",
            &mut select,
            &mut skip,
            &mut selected,
        )?;
        selected.sort_by(|left, right| left.0.cmp(&right.0));
        Ok(selected)
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

impl SourceRootNativeMetadataReader for RootFence {
    fn read_held_metadata(
        &mut self,
        reference: &str,
        max_bytes: usize,
    ) -> tos_compiler::Result<Option<Vec<u8>>> {
        if !reference.starts_with("ToS/")
            || reference.len() > 4096
            || reference.contains(['\\', '\0'])
            || reference.split('/').any(|part| {
                part.is_empty()
                    || part == "."
                    || part == ".."
                    || part == "catalog"
                    || part == "owner-local"
                    || part == "payload"
                    || part == "local-content"
                    || part == "private"
                    || part.starts_with('.')
            })
        {
            return Err(tos_compiler::Error::Invalid(
                "root source projection native metadata path",
            ));
        }
        self.read_optional(reference, max_bytes, 8)
            .map_err(tos_compiler::Error::Source)
    }
}

fn walk_contract_paths(fence: &mut RootFence, deadline: Instant) -> Result<Vec<String>, String> {
    fence.collect_contract_paths(deadline)
}

fn source_limits(max_rows: u64) -> SourceCatalogLimits {
    SourceCatalogLimits {
        max_files: MAX_CONTRACT_FILES,
        max_rows,
        max_file_bytes: MAX_SOURCE_FILE_BYTES,
        max_row_bytes: MAX_SOURCE_ROW_BYTES,
        max_contract_bytes: 16 * 1024 * 1024,
        max_output_row_bytes: MAX_OUTPUT_ROW_BYTES,
    }
}

fn read_contract_file(
    fence: &mut RootFence,
    reference: &str,
    cap: usize,
) -> Result<Vec<u8>, String> {
    fence.read_required(reference, cap, 16)
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

fn object(fields: Vec<(String, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        fields
            .into_iter()
            .map(|(key, value)| (JsonString::from_utf8(&key), value))
            .collect(),
    )
}
fn jstring(value: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(value))
}
fn jnumber(value: u64) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: value.to_string(),
    })
}
fn jarray(values: impl IntoIterator<Item = JsonValue>) -> JsonValue {
    JsonValue::Array(values.into_iter().collect())
}

struct CatalogCurrentness {
    catalog_sha256: String,
    record_count: u64,
    claim_count: u64,
    kinds: Vec<String>,
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

fn manifest_bytes(
    family_order: &[(String, String)],
    counts: &BTreeMap<String, u64>,
    claim_count: u64,
    object_count: u64,
    catalog_sha256: &str,
    extension_schema_refs: &BTreeSet<String>,
    has_extension_family: bool,
    publication_token: Option<&str>,
    output_hashes: &[(String, String)],
) -> Result<Vec<u8>, String> {
    let record_files = object(
        family_order
            .iter()
            .map(|(kind, filename)| (kind.clone(), jstring(&format!("{CATALOG_HOME}/{filename}"))))
            .collect(),
    );
    let mut count_fields = family_order
        .iter()
        .map(|(kind, _)| (kind.clone(), jnumber(*counts.get(kind).unwrap_or(&0))))
        .collect::<Vec<_>>();
    count_fields.push(("object_total".into(), jnumber(object_count)));
    count_fields.push(("claim".into(), jnumber(claim_count)));
    count_fields.push(("total".into(), jnumber(object_count + claim_count)));
    let mut fields = vec![
        (
            "schema_version".into(),
            jstring("tos_source_witness_catalog_v3"),
        ),
        ("owner_repo".into(), jstring("Tree-of-Sophia")),
        ("source_root".into(), jstring(SOURCE_HOME)),
        (
            "generated_by".into(),
            jstring("scripts/build_source_witness_catalog.py"),
        ),
        (
            "record_schema_ref".into(),
            jstring("ToS/contracts/corpus-record.schema.json"),
        ),
        (
            "claim_schema_ref".into(),
            jstring("ToS/contracts/claim-packet.schema.json"),
        ),
    ];
    if has_extension_family || !extension_schema_refs.is_empty() {
        fields.push((
            "extension_schema_refs".into(),
            jarray(extension_schema_refs.iter().map(|value| jstring(value))),
        ));
    }
    fields.extend([
        ("record_files".into(), record_files),
        (
            "claim_file".into(),
            jstring(&format!("{CATALOG_HOME}/claims.jsonl")),
        ),
        ("counts".into(), object(count_fields)),
        ("catalog_sha256".into(), jstring(catalog_sha256)),
    ]);
    if let Some(token) = publication_token {
        let files = object(
            output_hashes
                .iter()
                .map(|(path, digest)| (path.clone(), jstring(digest)))
                .collect(),
        );
        fields.push((
            "selected_metadata_publication".into(),
            object(vec![
                (
                    "protocol".into(),
                    jstring("tos_selected_source_metadata_v1"),
                ),
                ("token".into(), jstring(token)),
                ("files".into(), files),
            ]),
        ));
    }
    fields.push((
        "authority_boundary".into(),
        jstring("This generated catalog provides navigation to the tracked object and claim records that own its contents."),
    ));
    let encoded = emit_json_profile(
        &object(fields),
        JsonEmissionProfile::SourceWitnessCatalogPublishedV3,
        json_limits(MAX_OUTPUT_FILE_BYTES),
    )
    .map_err(|error| error.to_string())?;
    Ok(encoded.bytes)
}

fn verify_expected_file(
    fence: &mut RootFence,
    path: &str,
    expected: &[u8],
) -> Result<String, String> {
    let actual = fence.read_required(path, MAX_OUTPUT_FILE_BYTES, 2)?;
    if actual != expected {
        return Err("public source catalog is stale; rebuild it through its owner".into());
    }
    Ok(Digest256::of_bytes(expected).to_hex())
}

fn build_and_verify_catalog(
    fence: &mut RootFence,
    profiles: &SourceRootClaimProfiles,
    records: &mut BTreeMap<String, Vec<SourceEntry>>,
    claims: &mut Vec<SourceEntry>,
    publication: &MetadataPublicationEpoch,
) -> Result<CatalogCurrentness, String> {
    let mut profile_families = Vec::<(String, String)>::new();
    profiles
        .visit_record_profiles(|kind, _basename, filename| {
            if records.contains_key(kind) {
                profile_families.push((kind.to_owned(), filename.to_owned()));
            }
            Ok(())
        })
        .map_err(|error| error.to_string())?;
    let mut family_order = BASE_FAMILIES
        .iter()
        .map(|(kind, filename)| (kind.to_string(), filename.to_string()))
        .collect::<Vec<_>>();
    for (kind, filename) in profile_families {
        if family_order.iter().any(|(existing, _)| existing == &kind) {
            return Err("source projection record family conflicts with the base catalog".into());
        }
        family_order.push((kind, filename));
    }
    for (kind, _, filename) in ADAPTED_FAMILIES {
        if records.get(kind).is_some_and(|entries| !entries.is_empty())
            && !family_order.iter().any(|(existing, _)| existing == kind)
        {
            family_order.push((kind.to_owned(), filename.to_owned()));
        }
    }
    claims.sort_by(|left, right| left.identity.cmp(&right.identity));
    let mut object_count = 0u64;
    let mut claim_count = 0u64;
    let mut counts = BTreeMap::new();
    let mut extensions = BTreeSet::new();
    let mut catalog_hash = Digest256Hasher::new();
    let mut output_hashes = Vec::<(String, String)>::new();
    for (kind, filename) in &family_order {
        let family_entries = records.entry(kind.clone()).or_default();
        family_entries.sort_by(|left, right| left.identity.cmp(&right.identity));
        let mut file_bytes = Vec::new();
        for entry in family_entries.iter() {
            fence.meter.checkpoint()?;
            let row = canonical_bytes(&entry.catalog_entry, MAX_OUTPUT_ROW_BYTES)?;
            if row.len() > MAX_OUTPUT_ROW_BYTES {
                return Err("source projection catalog output row budget exceeded".into());
            }
            fence
                .meter
                .charge_state((row.len() as u64).saturating_mul(2))?;
            if file_bytes.len() > MAX_OUTPUT_FILE_BYTES.saturating_sub(row.len() + 1) {
                return Err("source projection catalog output file budget exceeded".into());
            }
            catalog_hash.update(kind.as_bytes());
            catalog_hash.update(b"\0");
            catalog_hash.update(&row);
            catalog_hash.update(b"\n");
            file_bytes.extend_from_slice(&row);
            file_bytes.push(b'\n');
            if let Some(schema) = entry
                .catalog_entry
                .get("source_schema_ref")
                .and_then(Value::as_str)
            {
                extensions.insert(schema.to_owned());
            }
        }
        let count = family_entries.len() as u64;
        object_count = object_count
            .checked_add(count)
            .ok_or("source projection object count overflow")?;
        counts.insert(kind.clone(), count);
        let path = format!("{CATALOG_HOME}/{filename}");
        let digest = verify_expected_file(fence, &path, &file_bytes)?;
        output_hashes.push((path, digest));
    }
    catalog_hash.update(b"claim\0");
    let mut claims_bytes = Vec::new();
    for entry in claims.iter() {
        fence.meter.checkpoint()?;
        let row = canonical_bytes(&entry.catalog_entry, MAX_OUTPUT_ROW_BYTES)?;
        if claims_bytes.len() > MAX_OUTPUT_FILE_BYTES.saturating_sub(row.len() + 1) {
            return Err("source projection claim catalog output file budget exceeded".into());
        }
        fence
            .meter
            .charge_state((row.len() as u64).saturating_mul(2))?;
        catalog_hash.update(&row);
        catalog_hash.update(b"\n");
        claims_bytes.extend_from_slice(&row);
        claims_bytes.push(b'\n');
        if let Some(schema) = entry
            .catalog_entry
            .get("source_schema_ref")
            .and_then(Value::as_str)
        {
            extensions.insert(schema.to_owned());
        }
    }
    claim_count = claims.len() as u64;
    counts.insert("claim".into(), claim_count);
    let catalog_sha256 = catalog_hash.finalize().to_hex();
    let claim_path = format!("{CATALOG_HOME}/claims.jsonl");
    let claim_digest = verify_expected_file(fence, &claim_path, &claims_bytes)?;
    output_hashes.push((claim_path, claim_digest));
    let has_extension_family = family_order
        .iter()
        .any(|(kind, _)| !BASE_FAMILIES.iter().any(|(base, _)| base == kind));
    let manifest = manifest_bytes(
        &family_order,
        &counts,
        claim_count,
        object_count,
        &catalog_sha256,
        &extensions,
        has_extension_family,
        publication.token(),
        &output_hashes,
    )?;
    verify_expected_file(
        fence,
        &format!("{CATALOG_HOME}/catalog.manifest.json"),
        &manifest,
    )?;
    let kinds = family_order.iter().map(|(kind, _)| kind.clone()).collect();
    Ok(CatalogCurrentness {
        catalog_sha256,
        record_count: object_count,
        claim_count,
        kinds,
    })
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
    max_input_bytes: u64,
    max_rows: u64,
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
    let mut fence = RootFence::open(root, max_input_bytes, max_rows, deadline)?;
    let publication = selected_publication(&mut fence)?;
    let contract_paths = walk_contract_paths(&mut fence, deadline)?;
    let limits = source_limits(max_rows);
    let profiles = SourceRootClaimProfiles::load_from_held_reader(
        contract_paths,
        |reference, cap| {
            read_contract_file(&mut fence, reference, cap).map_err(tos_compiler::Error::Source)
        },
        limits,
    )
    .map_err(|error| {
        format!("public source catalog schema/profile inventory is incomplete: {error}")
    })?;
    let candidates = fence.collect_source_paths(&profiles)?;
    let cancelled = AtomicBool::new(false);
    let mut native_ids = BTreeSet::new();
    let mut native_bytes = 0usize;
    let mut native_packets = 0usize;
    for (reference, candidate) in &candidates {
        if !matches!(candidate, SourceCandidateKind::Native) {
            continue;
        }
        native_packets += 1;
        if native_packets > MAX_NATIVE_PACKETS {
            return Err("source projection native identity packet budget exceeded".into());
        }
        let remaining = MAX_NATIVE_PACKET_BYTES
            .checked_sub(native_bytes)
            .filter(|remaining| *remaining > 0)
            .ok_or("source projection native identity byte budget exceeded")?;
        let raw = fence.read_required(reference, remaining.min(MAX_SOURCE_ROW_BYTES), 8)?;
        native_bytes = native_bytes
            .checked_add(raw.len())
            .filter(|bytes| *bytes <= MAX_NATIVE_PACKET_BYTES)
            .ok_or("source projection native identity byte budget exceeded")?;
        profiles
            .visit_native_semantic_identities(&raw, reference, |identity, _private_ref| {
                fence
                    .meter
                    .charge_state(identity.len() as u64 + 64)
                    .map_err(tos_compiler::Error::Source)?;
                native_ids.insert(identity.to_owned());
                Ok(())
            })
            .map_err(|error| format!("private native identity inventory is incomplete: {error}"))?;
    }

    let mut source_hashes = BTreeMap::<String, String>::new();
    let mut object_seen = BTreeSet::<String>::new();
    let mut claim_seen = BTreeSet::<String>::new();
    let mut by_kind = BTreeMap::<String, Vec<SourceEntry>>::new();
    let mut claims = Vec::<SourceEntry>::new();
    for (reference, candidate) in candidates {
        fence.meter.checkpoint()?;
        match candidate {
            SourceCandidateKind::Native => continue,
            SourceCandidateKind::Contract => {
                return Err("source projection contract path escaped its selector".into());
            }
            SourceCandidateKind::Record(kind) => {
                let raw = fence.read_required(&reference, MAX_SOURCE_FILE_BYTES, 12)?;
                let digest = Digest256::of_bytes(&raw).to_hex();
                remember_source_hash(&mut fence.meter, &mut source_hashes, &reference, &digest)?;
                let validated = profiles
                    .validate_record(&raw, &reference, &mut fence, deadline, &cancelled)
                    .map_err(|error| {
                        format!("public source record validation is incomplete: {error}")
                    })?;
                let identity = string(&validated.catalog_entry, "record_id")?.to_owned();
                if native_ids.contains(&identity) || !object_seen.insert(identity.clone()) {
                    return Err("duplicate source record identity or native reservation".into());
                }
                fence.meter.charge_row()?;
                let adapter = if kind == "artifact"
                    || validated
                        .catalog_entry
                        .get("source_schema_ref")
                        .and_then(Value::as_str)
                        == Some(COMPOSITE_SCHEMA)
                {
                    "native-witness"
                } else {
                    "source-record"
                };
                by_kind.entry(kind.clone()).or_default().push(SourceEntry {
                    identity,
                    record: validated.source,
                    catalog_entry: validated.catalog_entry,
                    source_ref: reference,
                    source_file_sha256: digest,
                    kind,
                    adapter: adapter.into(),
                    source_line: None,
                });
            }
            SourceCandidateKind::Claim => {
                let raw = fence.read_required(&reference, MAX_CLAIM_FILE_BYTES, 12)?;
                let digest = Digest256::of_bytes(&raw).to_hex();
                remember_source_hash(&mut fence.meter, &mut source_hashes, &reference, &digest)?;
                let remaining_rows = fence.meter.max_rows.saturating_sub(fence.meter.rows);
                let rows = claim_rows(&raw, remaining_rows)?;
                fence
                    .meter
                    .charge_state((rows.len() as u64).saturating_mul(32))?;
                for (line, raw_line) in rows {
                    if raw_line.len() > MAX_SOURCE_ROW_BYTES {
                        return Err("source projection claim row byte budget exceeded".into());
                    }
                    let validated = profiles
                        .validate_claim(raw_line, &reference, line)
                        .map_err(|error| {
                            format!("public source Claim validation is incomplete: {error}")
                        })?;
                    let identity = string(&validated.catalog_entry, "claim_id")?.to_owned();
                    if !claim_seen.insert(identity.clone()) {
                        return Err("duplicate public source Claim identity".into());
                    }
                    fence.meter.charge_row()?;
                    claims.push(SourceEntry {
                        identity,
                        record: validated.source,
                        catalog_entry: validated.catalog_entry,
                        source_ref: reference.clone(),
                        source_file_sha256: digest.clone(),
                        kind: "claim".into(),
                        adapter: "reified-claim".into(),
                        source_line: Some(line),
                    });
                }
            }
        }
    }
    if fence.meter.rows > max_rows {
        return Err("source projection catalog row budget exceeded".into());
    }
    let total_entries = by_kind.values().map(Vec::len).sum::<usize>() + claims.len();
    fence
        .meter
        .charge_state((total_entries as u64).saturating_mul(256))?;
    let mut object_entries = by_kind.into_values().flatten().collect::<Vec<_>>();
    object_entries.sort_by(|left, right| left.identity.cmp(&right.identity));
    claims.sort_by(|left, right| left.identity.cmp(&right.identity));
    let mut all_entries = object_entries;
    all_entries.extend(claims);
    let (mut object_entries, mut claim_entries) = (Vec::new(), Vec::new());
    for entry in all_entries {
        if entry.kind == "claim" {
            claim_entries.push(entry);
        } else {
            object_entries.push(entry);
        }
    }
    object_entries.sort_by(|left: &SourceEntry, right| left.identity.cmp(&right.identity));
    claim_entries.sort_by(|left: &SourceEntry, right| left.identity.cmp(&right.identity));
    let mut catalog_records = BTreeMap::<String, Vec<SourceEntry>>::new();
    for entry in object_entries.iter().cloned() {
        catalog_records
            .entry(entry.kind.clone())
            .or_default()
            .push(entry);
    }
    let currentness = build_and_verify_catalog(
        &mut fence,
        &profiles,
        &mut catalog_records,
        &mut claim_entries,
        &publication,
    )?;
    if currentness.record_count != object_entries.len() as u64
        || currentness.claim_count != claim_entries.len() as u64
    {
        return Err("source projection catalog count differs from full source enumeration".into());
    }
    for entry in &object_entries {
        if source_hashes.get(&entry.source_ref) != Some(&entry.source_file_sha256) {
            return Err("source projection source file digest is inconsistent".into());
        }
    }
    for entry in &claim_entries {
        if source_hashes.get(&entry.source_ref) != Some(&entry.source_file_sha256) {
            return Err("source projection Claim file digest is inconsistent".into());
        }
    }
    let kinds = currentness.kinds.clone();
    let mut entries = object_entries;
    entries.extend(claim_entries);
    verify_source_snapshot(&mut fence, &publication)?;
    Ok((
        fence,
        publication,
        entries,
        source_hashes,
        currentness.kinds.clone(),
        currentness,
    ))
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
    graph: Option<&str>,
    input_bytes: u64,
    max_rows: u64,
    deadline: Instant,
    rows: bool,
    output: &mut dyn Write,
) -> Result<(), String> {
    let (mut fence, publication, entries, source_hashes, kinds, currentness) =
        load_source_catalog(root, input_bytes, max_rows, deadline)?;
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
    let mut input = None;
    let mut observe = false;
    let mut rows = false;
    let mut max_input_bytes = INPUT_CAP;
    let mut max_rows = MAX_ROWS;
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
            "--graph" => graph = Some(value.as_str()),
            "--input" => input = Some(value.as_str()),
            "--max-input-bytes" => {
                max_input_bytes = value
                    .parse::<u64>()
                    .map_err(|_| "invalid source projection input byte budget")?
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
        if rows || root.is_some() || graph.is_some() {
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
        graph,
        max_input_bytes,
        max_rows,
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
            "source-projection-coverage --root ABS [--graph ABS_JSON|-] [--rows] [--max-input-bytes N --max-rows N --max-seconds N]\nsource-projection-coverage --observe-record --input ABS|-\nA stream without its terminal summary is incomplete. No assessment, admission or source mutation."
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
