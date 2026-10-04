//! Full immutable runtime-data snapshot production. This produces derived
//! selection facts; it neither admits authored source nor issues a live grant.
use crate::{
    Error, ExpectedSourceScope, FullKnowledgeLimits, KnowledgeSelectedExpectation,
    NATIVE_KNOWLEDGE_ADAPTER_PROFILES, NativeFamilyInputs, NativeProducerLimits, QueryVocabulary,
    Result, SourceBinding,
    d1_public_capture::{PublicCapture, PublicCaptureLimits},
    d1_public_graph::{
        PublicRepositoryRoot, PublicStageOwner, captured_input_roots, ingest_family_rows,
        prepare_family_rows,
    },
    d1_public_header::build_native_snapshot_header,
    d1_public_lens_specs::saved_lenses,
    d1_public_semantics::{validate_native_snapshot_semantics, validate_public_current_registries},
    knowledge_source_navigation_prepare::NavigationHeaderClaim,
    knowledge_stage::{ExactInputReceipt, KnowledgeStage, StageIsolation, StageLimits, WritePhase},
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{self, Read, Write},
    os::{
        fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd},
        unix::fs::{FileExt, MetadataExt},
    },
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};
use tos_foundation::{Digest256, Digest256Hasher, JsonLimits, JsonMode, JsonValue, parse_json};

const CORE_STATE_ROLE: &str = "tos-native-core-snapshot-state-v1";
const CORE_STATE_ISSUER: &str = "tos-compiler/native_snapshot.rs";
const CORE_STATE_REQUIRED_SEALS: libc::c_int =
    libc::F_SEAL_WRITE | libc::F_SEAL_GROW | libc::F_SEAL_SHRINK | libc::F_SEAL_SEAL;
const CORE_SOURCE_REFS: [(&str, &str); 5] = [
    ("corpus", "ToS/derived-exports/tos_corpus_index.min.json"),
    (
        "philosophy",
        "ToS/derived-exports/philosophy_graph_projection.min.json",
    ),
    (
        "bibliographic_claims",
        "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json",
    ),
    (
        "entity_type_registry",
        "ToS/doctrine/semantic-interchange/entity-types.v1.json",
    ),
    (
        "relation_type_registry",
        "ToS/doctrine/semantic-interchange/relation-types.v1.json",
    ),
];

fn check_snapshot_active(cancelled: &AtomicBool, deadline: Instant) -> Result<()> {
    if cancelled.load(Ordering::Acquire) {
        return Err(Error::Budget("native whole snapshot cancelled"));
    }
    if Instant::now() >= deadline {
        return Err(Error::Budget("native whole snapshot deadline"));
    }
    Ok(())
}

/// Issued only after the complete captured families, components and final
/// source check succeed. No constructor accepts an arbitrary candidate receipt.
pub struct CompletedNativeSnapshot {
    path: PathBuf,
    stage_model_identity: SnapshotModelIdentity,
    stage_limits: StageLimits,
    capture_identity: (u64, u64, u64, i64, i64, i64, i64),
    stage: crate::knowledge_stage::StageReceipt,
    full: crate::FullKnowledgeReceipt,
    expectation: KnowledgeSelectedExpectation,
    producer: crate::knowledge_native::NativeProducerReceipt,
    semantic_report: serde_json::Value,
    declaration_sha256: Digest256,
    source_revision: String,
    descriptor: Vec<u8>,
    vocabulary: QueryVocabulary,
    entity: Vec<u8>,
    relation: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SnapshotModelIdentity {
    device: u64,
    inode: u64,
    size: u64,
    mtime_seconds: i64,
    mtime_nanoseconds: i64,
    ctime_seconds: i64,
    ctime_nanoseconds: i64,
}

/// Implemented by the selected host resource owner. It must recheck a real
/// process-scoped hard-memory/swap boundary and the admitted cold-work envelope.
/// For the Linux source route, that means the held current cgroup-v2 scope has a
/// finite `memory.max` no greater than `working_ram_bytes`, `memory.swap.max`
/// equal to zero, and enough headroom for the retained stage model, sealed copy,
/// the remaining unallocated copy bytes, the producer's bounded 64 KiB copy
/// buffer while that buffer is live, and the cold SQLite working set. The stage
/// ticket's `working_ram_bytes` field alone is only metadata. The producer
/// supplies copy-phase values itself: before and during copy,
/// `additional_copy_bytes` is the exact remaining source size and
/// `copy_buffer_bytes` is 65,536; after the copy is held, additional copy
/// bytes are zero. The hold must remain valid for the entire callback and
/// recheck on every method call.
pub trait NativeColdOpenResourceHold: Send + Sync {
    fn verify_cold_open(
        &self,
        cold: crate::ColdOpenLimits,
        process: crate::NativeProcessLimits,
        working_ram_bytes: u64,
        stage_model_bytes: u64,
        sealed_model_bytes: u64,
        additional_copy_bytes: u64,
        copy_buffer_bytes: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<()>;
}

/// Explicit resident-memory and transfer ceilings for the whole Core
/// graph/catalog export. These limits apply only to this opt-in whole result;
/// normal selected query routes remain page-bounded.
#[derive(Clone, Copy, Debug)]
pub struct NativeWholeSnapshotLimits {
    pub max_rows: u64,
    pub max_row_bytes: usize,
    pub max_graph_bytes: usize,
    pub max_catalog_bytes: usize,
    pub max_catalog_inputs_bytes: usize,
    /// Hard cap for the private producer-issued state packet, including the
    /// full current graph and exact captured source baselines.
    pub max_state_bytes: usize,
    pub json: JsonLimits,
}

/// Actual whole-function output extracted from the same staged native model
/// and capture as its completed producer. Rows are normalized producer rows;
/// `catalog_inputs` contains detached copies of the exact captured registries,
/// header and saved lenses when explicitly requested.
pub struct NativeKnowledgeSnapshot {
    pub graph: serde_json::Value,
    pub catalog: serde_json::Value,
    pub catalog_inputs: Option<crate::prepared_catalog_semantics::CatalogInputs>,
    pub source_revision: String,
    pub source_state: Vec<(String, i64, u64, u64, i64)>,
    pub source_inputs: Vec<(String, String, u64)>,
    pub graph_root_sha256: String,
    pub catalog_sha256: String,
    /// Present only for a retained graph/snapshot publication. One-shot
    /// snapshots leave this empty so they cannot become an addressed parent.
    pub state: Option<ProducerIssuedCoreSnapshotState>,
}

/// The native producer's sealed, opaque source-baseline descriptor. There is
/// no constructor from caller-provided bytes; only a completed snapshot
/// capture can issue one. The app-side identity map remains responsible for
/// tying a published graph object to this descriptor across calls.
pub struct ProducerIssuedCoreSnapshotState {
    file: File,
    size_bytes: u64,
    receipt_sha256: String,
    source_revision: String,
}

impl ProducerIssuedCoreSnapshotState {
    pub fn as_fd(&self) -> BorrowedFd<'_> {
        self.file.as_fd()
    }

    pub fn size_bytes(&self) -> u64 {
        self.size_bytes
    }

    pub fn receipt_sha256(&self) -> &str {
        &self.receipt_sha256
    }

    pub fn source_revision(&self) -> &str {
        &self.source_revision
    }
}

/// A state descriptor after schema, issuer receipt, kernel seals, current
/// root and all source-carrier identities have been checked by the native
/// importer. Its fields stay private so transition code uses only this gate.
pub struct ImportedCoreSnapshotState {
    _file: File,
    root: PathBuf,
    capture_identity: (u64, u64, u64, i64, i64, i64, i64),
    source_state: Vec<(String, i64, u64, u64, i64)>,
    capture_source_state: Vec<(String, i64, u64, u64, i64)>,
    source_revision: String,
    graph: serde_json::Value,
    catalog: serde_json::Value,
    source_inputs: std::collections::BTreeMap<String, serde_json::Value>,
    input_sha256: std::collections::BTreeMap<String, String>,
    graph_root_sha256: String,
    catalog_sha256: String,
    receipt_sha256: String,
}

impl ImportedCoreSnapshotState {
    pub fn source_revision(&self) -> &str {
        &self.source_revision
    }

    pub fn graph_root_sha256(&self) -> &str {
        &self.graph_root_sha256
    }

    pub fn catalog_sha256(&self) -> &str {
        &self.catalog_sha256
    }

    pub fn receipt_sha256(&self) -> &str {
        &self.receipt_sha256
    }
}

/// Prior graph/catalog result that passed the native same-capture physical and
/// logical currentness test. The SDK can preserve object identity and the
/// existing descriptor when this value is returned; it must not retain this
/// detached result as a new state authority.
pub struct ReusedNativeKnowledgeSnapshot {
    pub graph: serde_json::Value,
    pub catalog: serde_json::Value,
    pub source_revision: String,
    pub source_state: Vec<(String, i64, u64, u64, i64)>,
    pub source_inputs: Vec<(String, String, u64)>,
    pub graph_root_sha256: String,
    pub catalog_sha256: String,
    pub state_receipt_sha256: String,
}

#[derive(Clone, Copy, Debug)]
pub struct NativeAddressedUpdate<'a> {
    pub source_graph: &'a str,
    pub source_id: &'a str,
    pub source_record: &'a serde_json::Value,
    pub source_revision: &'a str,
    pub expected_parent_revision: Option<&'a str>,
    pub return_report: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct NativeAddressedUpdateReport {
    pub value: serde_json::Value,
}

struct ValidatedAddressedSourceDelta {
    normalized_id: String,
    before_content_revision: serde_json::Value,
    incident_relation_ids: Vec<String>,
    previous_node_count: usize,
    previous_relation_count: usize,
    source_transition: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CoreSnapshotStatePacket {
    schema_version: String,
    role: String,
    issuer: String,
    root: String,
    capture_identity: (u64, u64, u64, i64, i64, i64, i64),
    source_state: Vec<(String, i64, u64, u64, i64)>,
    capture_source_state: Vec<(String, i64, u64, u64, i64)>,
    source_revision: String,
    graph_root_sha256: String,
    catalog_sha256: String,
    graph: serde_json::Value,
    catalog: serde_json::Value,
    source_inputs: std::collections::BTreeMap<String, serde_json::Value>,
    input_sha256: std::collections::BTreeMap<String, String>,
    receipt_sha256: String,
}

#[derive(Serialize)]
struct CoreSnapshotStatePacketView<'a> {
    schema_version: &'a str,
    role: &'a str,
    issuer: &'a str,
    root: &'a str,
    capture_identity: &'a (u64, u64, u64, i64, i64, i64, i64),
    source_state: &'a [(String, i64, u64, u64, i64)],
    capture_source_state: &'a [(String, i64, u64, u64, i64)],
    source_revision: &'a str,
    graph_root_sha256: &'a str,
    catalog_sha256: &'a str,
    graph: &'a serde_json::Value,
    catalog: &'a serde_json::Value,
    source_inputs: &'a std::collections::BTreeMap<String, serde_json::Value>,
    input_sha256: &'a std::collections::BTreeMap<String, String>,
    receipt_sha256: &'a str,
}

impl<'a> From<(&'a CoreSnapshotStatePacket, &'a str)> for CoreSnapshotStatePacketView<'a> {
    fn from((packet, receipt_sha256): (&'a CoreSnapshotStatePacket, &'a str)) -> Self {
        Self {
            schema_version: &packet.schema_version,
            role: &packet.role,
            issuer: &packet.issuer,
            root: &packet.root,
            capture_identity: &packet.capture_identity,
            source_state: &packet.source_state,
            capture_source_state: &packet.capture_source_state,
            source_revision: &packet.source_revision,
            graph_root_sha256: &packet.graph_root_sha256,
            catalog_sha256: &packet.catalog_sha256,
            graph: &packet.graph,
            catalog: &packet.catalog,
            source_inputs: &packet.source_inputs,
            input_sha256: &packet.input_sha256,
            receipt_sha256,
        }
    }
}

impl NativeWholeSnapshotLimits {
    fn validate(self) -> Result<()> {
        if self.max_rows == 0
            || self.max_row_bytes == 0
            || self.max_graph_bytes == 0
            || self.max_catalog_bytes == 0
            || self.max_catalog_inputs_bytes == 0
            || self.max_state_bytes == 0
            || self.max_row_bytes > i64::MAX as usize
            || self.max_catalog_bytes > i64::MAX as usize
            || self.max_state_bytes > i64::MAX as usize
            || self.json.max_bytes == 0
            || self.json.max_visits == 0
        {
            return Err(Error::Budget("native whole snapshot limits"));
        }
        Ok(())
    }
}

struct StateDigestWriter<'a> {
    hasher: tos_foundation::Digest256Hasher,
    written: usize,
    max_bytes: usize,
    deadline: Instant,
    cancelled: &'a AtomicBool,
}

impl Write for StateDigestWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.cancelled.load(Ordering::Acquire) || Instant::now() >= self.deadline {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "state serialization cancelled",
            ));
        }
        let next = self
            .written
            .checked_add(bytes.len())
            .filter(|next| *next <= self.max_bytes)
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "state serialization cap"))?;
        self.hasher.update(bytes);
        self.written = next;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn state_receipt_digest(
    view: &CoreSnapshotStatePacketView<'_>,
    max_bytes: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(String, usize)> {
    check_snapshot_active(cancelled, deadline)?;
    let mut writer = StateDigestWriter {
        hasher: Digest256Hasher::new(),
        written: 0,
        max_bytes,
        deadline,
        cancelled,
    };
    serde_json::to_writer(&mut writer, view)
        .map_err(|_| Error::Budget("native snapshot state receipt serialization"))?;
    check_snapshot_active(cancelled, deadline)?;
    Ok((writer.hasher.finalize().to_hex(), writer.written))
}

struct StateFileWriter<'a> {
    file: &'a mut File,
    written: usize,
    max_bytes: usize,
    deadline: Instant,
    cancelled: &'a AtomicBool,
}

impl Write for StateFileWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.cancelled.load(Ordering::Acquire) || Instant::now() >= self.deadline {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "state export cancelled",
            ));
        }
        let next = self
            .written
            .checked_add(bytes.len())
            .filter(|next| *next <= self.max_bytes)
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "state export cap"))?;
        let count = self.file.write(bytes)?;
        self.written = self
            .written
            .checked_add(count)
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "state export byte overflow"))?;
        if self.written > self.max_bytes || self.written > next {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "state export write bound",
            ));
        }
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

fn state_packet_view<'a>(
    root: &'a str,
    capture_identity: &'a (u64, u64, u64, i64, i64, i64, i64),
    source_state: &'a [(String, i64, u64, u64, i64)],
    capture_source_state: &'a [(String, i64, u64, u64, i64)],
    source_revision: &'a str,
    graph_root_sha256: &'a str,
    catalog_sha256: &'a str,
    graph: &'a serde_json::Value,
    catalog: &'a serde_json::Value,
    source_inputs: &'a std::collections::BTreeMap<String, serde_json::Value>,
    input_sha256: &'a std::collections::BTreeMap<String, String>,
    receipt_sha256: &'a str,
) -> CoreSnapshotStatePacketView<'a> {
    CoreSnapshotStatePacketView {
        schema_version: "tos_native_core_snapshot_state_v1",
        role: CORE_STATE_ROLE,
        issuer: CORE_STATE_ISSUER,
        root,
        capture_identity,
        source_state,
        capture_source_state,
        source_revision,
        graph_root_sha256,
        catalog_sha256,
        graph,
        catalog,
        source_inputs,
        input_sha256,
        receipt_sha256,
    }
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn source_values_from_capture(
    capture: &PublicCapture,
    source_revision: &str,
    max_bytes: usize,
    json: JsonLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(
    std::collections::BTreeMap<String, serde_json::Value>,
    std::collections::BTreeMap<String, String>,
)> {
    let mut inputs = std::collections::BTreeMap::new();
    let mut hashes = std::collections::BTreeMap::new();
    let mut total = 0usize;
    let mut visits = json.max_visits;
    check_snapshot_active(cancelled, deadline)?;
    let view = CompletedCaptureCarriers {
        capture,
        source_revision: Some(source_revision),
    };
    let budget = crate::native_snapshot_carriers::CapturedCarrierReadBudget {
        max_rows: capture.rows.max(1),
        max_input_bytes: max_bytes as u64,
        max_output_bytes: max_bytes,
        json,
    };
    for (name, request) in [
        (
            "corpus",
            crate::native_snapshot_carriers::CapturedCarrierRequest::CorpusIndex,
        ),
        (
            "bibliographic_claims",
            crate::native_snapshot_carriers::CapturedCarrierRequest::BibliographicGraph,
        ),
    ] {
        let raw = crate::native_snapshot_carriers::read_complete_captured_carrier(
            &view, request, budget, deadline, cancelled,
        )?;
        total = total
            .checked_add(raw.len())
            .filter(|bytes| *bytes <= max_bytes)
            .ok_or(Error::Budget("native snapshot state input bytes"))?;
        let value = strict_snapshot_value(&raw, max_bytes, json, &mut visits, cancelled, deadline)?;
        hashes.insert(
            name.to_owned(),
            source_value_digest(&value, json, cancelled, deadline, Some(capture))?,
        );
        inputs.insert(name.to_owned(), value);
    }
    check_snapshot_active(cancelled, deadline)?;
    let philosophy_header = capture.header_object("philosophy", "", max_bytes)?;
    let header_raw = serde_json::to_vec(&philosophy_header)
        .map_err(|_| Error::Invalid("native snapshot philosophy header"))?;
    total = total
        .checked_add(header_raw.len())
        .filter(|bytes| *bytes <= max_bytes)
        .ok_or(Error::Budget("native snapshot state input bytes"))?;
    let mut philosophy = strict_snapshot_value(
        &header_raw,
        max_bytes,
        json,
        &mut visits,
        cancelled,
        deadline,
    )?;
    for collection in [
        "nodes",
        "edges",
        "clusters",
        "views",
        "review_packets",
        "graph_layers",
    ] {
        match capture
            .captured_collection_kind("philosophy", collection)?
            .as_deref()
        {
            None => continue,
            Some("array") => {}
            _ => return Err(Error::Invalid("native snapshot philosophy collection kind")),
        }
        let mut rows = Vec::new();
        let actual = capture.visit_rows("philosophy", collection, |ordinal, raw| {
            check_snapshot_active(cancelled, deadline)?;
            if ordinal != rows.len() as u64 || rows.len() as u64 >= budget.max_rows {
                return Err(Error::Budget("native snapshot philosophy source rows"));
            }
            total = total
                .checked_add(raw.len())
                .filter(|bytes| *bytes <= max_bytes)
                .ok_or(Error::Budget("native snapshot state input bytes"))?;
            rows.push(strict_snapshot_value(
                raw,
                max_bytes,
                json,
                &mut visits,
                cancelled,
                deadline,
            )?);
            Ok(())
        })?;
        if actual != rows.len() as u64 {
            return Err(Error::Invalid("native snapshot philosophy source EOF"));
        }
        if philosophy
            .as_object_mut()
            .ok_or(Error::Invalid("native snapshot philosophy header object"))?
            .insert(collection.to_owned(), serde_json::Value::Array(rows))
            .is_some()
        {
            return Err(Error::Invalid(
                "native snapshot philosophy collection collision",
            ));
        }
    }
    hashes.insert(
        "philosophy".into(),
        source_value_digest(&philosophy, json, cancelled, deadline, Some(capture))?,
    );
    inputs.insert("philosophy".into(), philosophy);
    for (name, path) in CORE_SOURCE_REFS.iter().skip(3) {
        check_snapshot_active(cancelled, deadline)?;
        let raw = capture
            .read_input(path, max_bytes)?
            .ok_or(Error::Invalid("native snapshot state registry absent"))?;
        total = total
            .checked_add(raw.len())
            .filter(|bytes| *bytes <= max_bytes)
            .ok_or(Error::Budget("native snapshot state input bytes"))?;
        let value = strict_snapshot_value(&raw, max_bytes, json, &mut visits, cancelled, deadline)?;
        if value.as_object().is_none() {
            return Err(Error::Invalid("native snapshot state registry object"));
        }
        hashes.insert(
            (*name).to_owned(),
            source_value_digest(&value, json, cancelled, deadline, Some(capture))?,
        );
        inputs.insert((*name).to_owned(), value);
    }
    if inputs.len() != CORE_SOURCE_REFS.len() || hashes.len() != CORE_SOURCE_REFS.len() {
        return Err(Error::Invalid("native snapshot state source closure"));
    }
    check_snapshot_active(cancelled, deadline)?;
    Ok((inputs, hashes))
}

fn addressed_collection<'a>(
    inputs: &'a mut std::collections::BTreeMap<String, serde_json::Value>,
    source_graph: &str,
) -> Result<&'a mut Vec<serde_json::Value>> {
    let (document, field) = match source_graph {
        "philosophy" => ("philosophy", "nodes"),
        "canon" => ("corpus", "nodes"),
        "source-navigation" => ("corpus", "source_navigation/nodes"),
        "source-claims" => ("bibliographic_claims", "nodes"),
        _ => return Err(Error::Invalid("unsupported native addressed source graph")),
    };
    let value = inputs
        .get_mut(document)
        .ok_or(Error::Invalid("native addressed source document absent"))?;
    if source_graph == "source-navigation" {
        let navigation = value
            .get_mut("source_navigation")
            .and_then(serde_json::Value::as_object_mut)
            .ok_or(Error::Invalid("native addressed navigation object absent"))?;
        return navigation
            .get_mut("nodes")
            .and_then(serde_json::Value::as_array_mut)
            .ok_or(Error::Invalid("native addressed source nodes absent"));
    }
    value
        .get_mut(field)
        .and_then(serde_json::Value::as_array_mut)
        .ok_or(Error::Invalid("native addressed source nodes absent"))
}

fn addressed_collection_ref<'a>(
    inputs: &'a std::collections::BTreeMap<String, serde_json::Value>,
    source_graph: &str,
) -> Result<&'a Vec<serde_json::Value>> {
    let (document, field) = match source_graph {
        "philosophy" => ("philosophy", "nodes"),
        "canon" => ("corpus", "nodes"),
        "source-navigation" => ("corpus", "source_navigation/nodes"),
        "source-claims" => ("bibliographic_claims", "nodes"),
        _ => return Err(Error::Invalid("unsupported native addressed source graph")),
    };
    let value = inputs
        .get(document)
        .ok_or(Error::Invalid("native addressed source document absent"))?;
    if source_graph == "source-navigation" {
        return value
            .get("source_navigation")
            .and_then(serde_json::Value::as_object)
            .and_then(|navigation| navigation.get("nodes"))
            .and_then(serde_json::Value::as_array)
            .ok_or(Error::Invalid("native addressed navigation nodes absent"));
    }
    value
        .get(field)
        .and_then(serde_json::Value::as_array)
        .ok_or(Error::Invalid("native addressed source nodes absent"))
}

fn addressed_record_matches(record: &serde_json::Value, source_id: &str) -> bool {
    ["node_id", "id", "path"]
        .iter()
        .any(|field| record.get(*field).and_then(serde_json::Value::as_str) == Some(source_id))
}

fn validate_addressed_transition(
    previous: &ImportedCoreSnapshotState,
    capture: &PublicCapture,
    update: NativeAddressedUpdate<'_>,
    max_bytes: usize,
    json: JsonLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<ValidatedAddressedSourceDelta> {
    if !matches!(
        update.source_graph,
        "philosophy" | "canon" | "source-navigation" | "source-claims"
    ) || update.source_id.trim().is_empty()
        || update.source_record.as_object().is_none()
        || !valid_sha256(update.source_revision)
    {
        return Err(Error::Invalid("native addressed update request"));
    }
    let parent_revision = previous
        .graph
        .get("source_revision")
        .and_then(serde_json::Value::as_str)
        .ok_or(Error::Invalid("native addressed parent graph revision"))?;
    let expected_parent = update.expected_parent_revision.unwrap_or(parent_revision);
    if expected_parent != parent_revision || previous.source_revision != parent_revision {
        return Err(Error::Invalid("native addressed parent revision stale"));
    }
    let previous_graph_root = previous
        .graph
        .get("graph_root_sha256")
        .and_then(serde_json::Value::as_str);
    if previous_graph_root.is_some_and(|value| value != previous.graph_root_sha256) {
        return Err(Error::Invalid("native addressed parent graph root"));
    }
    let expected_root = std::fs::canonicalize(&previous.root)?;
    let current_root = std::fs::canonicalize(capture.root())?;
    if expected_root != current_root {
        return Err(Error::Invalid("native addressed source root changed"));
    }
    let current_state = capture.core_source_state()?;
    if previous.source_state.len() != current_state.len()
        || previous
            .source_state
            .iter()
            .zip(&current_state)
            .any(|(before, after)| before.0 != after.0)
    {
        return Err(Error::Invalid(
            "native addressed selected input paths changed",
        ));
    }
    let previous_capture_paths = previous
        .capture_source_state
        .iter()
        .map(|source| source.0.as_str())
        .collect::<Vec<_>>();
    let current_capture_state = capture.capture_source_state()?;
    let current_capture_paths = current_capture_state
        .iter()
        .map(|source| source.0.as_str())
        .collect::<Vec<_>>();
    if previous_capture_paths != current_capture_paths {
        return Err(Error::Invalid(
            "native addressed captured input paths changed",
        ));
    }
    let current_revision = capture.core_source_revision()?;
    if update.source_revision != current_revision {
        return Err(Error::Invalid("native addressed target source revision"));
    }
    let (current_inputs, _) = source_values_from_capture(
        capture,
        &current_revision,
        max_bytes,
        json,
        deadline,
        cancelled,
    )?;
    if previous.source_inputs.len() != current_inputs.len()
        || previous.source_inputs.keys().ne(current_inputs.keys())
    {
        return Err(Error::Invalid("native addressed source closure changed"));
    }
    let mut expected_inputs = previous.source_inputs.clone();
    let before_matches = {
        let records = addressed_collection(&mut expected_inputs, update.source_graph)?;
        records
            .iter()
            .enumerate()
            .filter_map(|(position, record)| {
                addressed_record_matches(record, update.source_id).then_some(position)
            })
            .collect::<Vec<_>>()
    };
    let after_matches = addressed_collection_ref(&current_inputs, update.source_graph)?
        .iter()
        .enumerate()
        .filter_map(|(position, record)| {
            addressed_record_matches(record, update.source_id).then_some(position)
        })
        .collect::<Vec<_>>();
    if before_matches.len() != 1 || after_matches.len() != 1 {
        return Err(Error::Invalid(
            "native addressed source must retain one record",
        ));
    }
    let position = before_matches[0];
    if position != after_matches[0] {
        return Err(Error::Invalid("native addressed source record moved"));
    }
    let record_digest = source_value_digest(
        update.source_record,
        json,
        cancelled,
        deadline,
        Some(capture),
    )?;
    let current_record_digest = source_value_digest(
        &addressed_collection_ref(&current_inputs, update.source_graph)?[position],
        json,
        cancelled,
        deadline,
        Some(capture),
    )?;
    if record_digest != current_record_digest {
        return Err(Error::Invalid(
            "native addressed replacement differs from owner source",
        ));
    }
    addressed_collection(&mut expected_inputs, update.source_graph)?[position] =
        update.source_record.clone();
    for (name, expected) in &expected_inputs {
        let current = current_inputs
            .get(name)
            .ok_or(Error::Invalid("native addressed current source absent"))?;
        if source_value_digest(expected, json, cancelled, deadline, Some(capture))?
            != source_value_digest(current, json, cancelled, deadline, Some(capture))?
        {
            return Err(Error::Invalid(
                "native addressed source transition has extra changes",
            ));
        }
    }
    let previous_nodes = previous
        .graph
        .get("nodes")
        .and_then(serde_json::Value::as_array)
        .ok_or(Error::Invalid("native addressed previous graph nodes"))?;
    let previous_relations = previous
        .graph
        .get("relations")
        .and_then(serde_json::Value::as_array)
        .ok_or(Error::Invalid("native addressed previous graph relations"))?;
    let native_address = format!("{}:{}", update.source_graph, update.source_id);
    let old_nodes = previous_nodes
        .iter()
        .filter(|node| {
            node.get("source_graph").and_then(serde_json::Value::as_str)
                == Some(update.source_graph)
                && (node.get("id").and_then(serde_json::Value::as_str) == Some(update.source_id)
                    || node.get("native_id").and_then(serde_json::Value::as_str)
                        == Some(update.source_id)
                    || node.get("id").and_then(serde_json::Value::as_str)
                        == Some(native_address.as_str()))
        })
        .collect::<Vec<_>>();
    if old_nodes.len() != 1 {
        return Err(Error::Invalid("native addressed normalized source address"));
    }
    let normalized_id = old_nodes[0]
        .get("id")
        .and_then(serde_json::Value::as_str)
        .ok_or(Error::Invalid("native addressed normalized node ID"))?
        .to_owned();
    let before_content_revision = old_nodes[0]
        .get("content_revision")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let mut incident_relation_ids = std::collections::BTreeSet::new();
    for relation in previous_relations {
        if relation.as_object().is_none() {
            return Err(Error::Invalid("native addressed previous relation object"));
        }
        if relation.get("from_id").and_then(serde_json::Value::as_str)
            == Some(normalized_id.as_str())
            || relation.get("to_id").and_then(serde_json::Value::as_str)
                == Some(normalized_id.as_str())
        {
            incident_relation_ids.insert(
                relation
                    .get("id")
                    .and_then(serde_json::Value::as_str)
                    .ok_or(Error::Invalid("native addressed previous relation ID"))?
                    .to_owned(),
            );
        }
    }
    Ok(ValidatedAddressedSourceDelta {
        normalized_id,
        before_content_revision,
        incident_relation_ids: incident_relation_ids.into_iter().collect(),
        previous_node_count: previous_nodes.len(),
        previous_relation_count: previous_relations.len(),
        source_transition: serde_json::json!({
            "mode":"exact-single-record-delta",
            "source_graph":update.source_graph,
            "source_id":update.source_id,
            "changed_records":1,
            "source_scan":"complete-direct-carrier-set",
        }),
    })
}

fn native_graph_structural_report(
    graph: &serde_json::Value,
    max_rows: u64,
) -> Result<serde_json::Value> {
    let nodes = graph
        .get("nodes")
        .and_then(serde_json::Value::as_array)
        .ok_or(Error::Invalid("native addressed result graph nodes"))?;
    let relations = graph
        .get("relations")
        .and_then(serde_json::Value::as_array)
        .ok_or(Error::Invalid("native addressed result graph relations"))?;
    if nodes.len().saturating_add(relations.len()) as u64 > max_rows {
        return Err(Error::Budget("native addressed structural report rows"));
    }
    let mut violations = Vec::new();
    let mut node_ids = std::collections::BTreeSet::new();
    for node in nodes {
        let Some(id) = node.get("id").and_then(serde_json::Value::as_str) else {
            return Err(Error::Invalid("native addressed result node ID"));
        };
        if !node_ids.insert(id.to_owned()) && violations.len() < 20 {
            violations.push(format!("duplicate knowledge node id: {id}"));
        }
    }
    let mut relation_ids = std::collections::BTreeSet::new();
    for relation in relations {
        let Some(id) = relation.get("id").and_then(serde_json::Value::as_str) else {
            return Err(Error::Invalid("native addressed result relation ID"));
        };
        if !relation_ids.insert(id.to_owned()) && violations.len() < 20 {
            violations.push(format!("duplicate knowledge relation id: {id}"));
        }
        for endpoint in ["from_id", "to_id"] {
            let target = relation
                .get(endpoint)
                .and_then(serde_json::Value::as_str)
                .ok_or(Error::Invalid("native addressed result endpoint"))?;
            if !node_ids.contains(target) && violations.len() < 20 {
                violations.push(format!(
                    "relation {id} has unresolved normalized endpoint {target}"
                ));
            }
        }
    }
    violations.sort();
    violations.dedup();
    Ok(serde_json::json!({
        "valid": violations.is_empty(),
        "violations": violations,
        "checked_nodes": nodes.len(),
        "checked_relations": relations.len(),
    }))
}

fn build_native_addressed_report(
    previous: &ImportedCoreSnapshotState,
    completed: &CompletedNativeSnapshot,
    snapshot: &NativeKnowledgeSnapshot,
    delta: &ValidatedAddressedSourceDelta,
    update: NativeAddressedUpdate<'_>,
    source_rows: u64,
    max_rows: u64,
) -> Result<NativeAddressedUpdateReport> {
    let nodes = snapshot
        .graph
        .get("nodes")
        .and_then(serde_json::Value::as_array)
        .ok_or(Error::Invalid("native addressed successor nodes"))?;
    let relations = snapshot
        .graph
        .get("relations")
        .and_then(serde_json::Value::as_array)
        .ok_or(Error::Invalid("native addressed successor relations"))?;
    let successor_matches = nodes
        .iter()
        .filter(|node| {
            node.get("source_graph").and_then(serde_json::Value::as_str)
                == Some(update.source_graph)
                && node.get("id").and_then(serde_json::Value::as_str)
                    == Some(delta.normalized_id.as_str())
        })
        .count();
    if successor_matches != 1 {
        return Err(Error::Invalid("native addressed normalized ID changed"));
    }
    let structural = native_graph_structural_report(&snapshot.graph, max_rows)?;
    if structural.get("valid") != Some(&serde_json::Value::Bool(true)) {
        return Err(Error::Invalid(
            "native addressed full structural validation",
        ));
    }
    let address = serde_json::json!({
        "source_graph": update.source_graph,
        "source_id": update.source_id,
        "normalized_id": delta.normalized_id,
    });
    let lineage = serde_json::json!({
        "schema": "tos_addressed_update_lineage_v1",
        "processor": "tos-addressed-update-v1",
        "operation": "replace",
        "parent_source_revision": previous.source_revision,
        "address": address,
        "before_content_revision": delta.before_content_revision,
        "replacement_source_digest": crate::knowledge_normalization::stable_digest(update.source_record)?,
        "incident_relation_ids": delta.incident_relation_ids,
    });
    let lineage_revision = crate::knowledge_normalization::stable_digest(&lineage)?;
    let recomputed_nodes = nodes
        .iter()
        .map(|node| {
            node.get("id")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
                .ok_or(Error::Invalid("native addressed recomputed node ID"))
        })
        .collect::<Result<Vec<_>>>()?;
    let recomputed_relations = relations
        .iter()
        .map(|relation| {
            relation
                .get("id")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
                .ok_or(Error::Invalid("native addressed recomputed relation ID"))
        })
        .collect::<Result<Vec<_>>>()?;
    let report = serde_json::json!({
        "schema": "tos_addressed_update_report_v1",
        "processor": "tos-addressed-update-v1",
        "source_revision": snapshot.source_revision,
        "source_revision_mode": "provided-exact",
        "lineage_revision": lineage_revision,
        "address": address,
        "recomputed": {"nodes":recomputed_nodes,"relations":recomputed_relations},
        "reused": {"nodes":0,"relations":0},
        "source_transition": delta.source_transition,
        "input_traversal": {
            "submitted_source_records":1,
            "retained_relation_payloads_read":delta.previous_relation_count,
            "source_records_indexed":source_rows,
            "source_assembly":"complete-native-source-reassembly",
            "source_scan":"complete-direct-carrier-set",
            "normalized_nodes_scanned":delta.previous_node_count.saturating_add(nodes.len().saturating_mul(2)),
            "normalized_relations_scanned":delta.previous_relation_count.saturating_add(relations.len().saturating_mul(2)),
            "bounded_recompute":"complete-native-rebuild",
            "global_validation":"full-snapshot-scan",
        },
        "validation": {
            "local":{"node_ids":[delta.normalized_id],"relation_ids":delta.incident_relation_ids},
            "global":"semantic",
            "structural":structural,
            "semantic":completed.semantic_report,
        },
        "snapshot":{"previous_snapshot_mutated":false},
        "native_build":"complete-native-source-reassembly",
        "is_semantic_acceptance":false,
    });
    Ok(NativeAddressedUpdateReport { value: report })
}

fn source_value_digest(
    value: &serde_json::Value,
    limits: JsonLimits,
    cancelled: &AtomicBool,
    deadline: Instant,
    capture: Option<&PublicCapture>,
) -> Result<String> {
    check_snapshot_active(cancelled, deadline)?;
    let raw = serde_json::to_vec(value)
        .map_err(|_| Error::Invalid("native snapshot source value encoding"))?;
    if let Some(capture) = capture {
        capture.charge_work(raw.len() as u64)?;
    }
    check_snapshot_active(cancelled, deadline)?;
    let parsed = parse_json(&raw, JsonMode::PublishedStrict, limits)
        .map_err(|_| Error::Invalid("native snapshot source value JSON"))?;
    let canonical = tos_foundation::canonical_bytes_v1(
        parsed.root(),
        tos_foundation::CanonicalProfile::SourceRecordDigestV1,
        limits,
    )
    .map_err(|_| Error::Invalid("native snapshot source value canonicalization"))?;
    if let Some(capture) = capture {
        capture.charge_work(canonical.len() as u64)?;
    }
    check_snapshot_active(cancelled, deadline)?;
    Ok(Digest256::of_bytes(&canonical).to_hex())
}

fn memfd_file(label: &str) -> Result<File> {
    use std::ffi::CString;
    let name =
        CString::new(label).map_err(|_| Error::Invalid("native snapshot state memfd name"))?;
    let raw =
        unsafe { libc::memfd_create(name.as_ptr(), libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING) };
    if raw < 0 {
        return Err(Error::Source(io::Error::last_os_error().to_string()));
    }
    Ok(unsafe { File::from_raw_fd(raw) })
}

fn validate_state_fd(fd: BorrowedFd<'_>, max_bytes: usize) -> Result<(File, u64)> {
    if max_bytes == 0 || max_bytes > i64::MAX as usize {
        return Err(Error::Budget("native snapshot state descriptor cap"));
    }
    let raw_fd = fd.as_raw_fd();
    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstat(raw_fd, &mut stat) } != 0 {
        return Err(Error::Source(io::Error::last_os_error().to_string()));
    }
    if (stat.st_mode & libc::S_IFMT) != libc::S_IFREG || stat.st_size <= 0 {
        return Err(Error::Invalid("native snapshot state descriptor type/size"));
    }
    let size = stat.st_size as u64;
    if size > max_bytes as u64 {
        return Err(Error::Budget("native snapshot state descriptor bytes"));
    }
    let seals = unsafe { libc::fcntl(raw_fd, libc::F_GET_SEALS) };
    if seals != CORE_STATE_REQUIRED_SEALS {
        return Err(Error::Invalid("native snapshot state descriptor seals"));
    }
    let mut fs: libc::statfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstatfs(raw_fd, &mut fs) } != 0 {
        return Err(Error::Source(io::Error::last_os_error().to_string()));
    }
    // Linux memfd uses shmem/tmpfs. Requiring this, alongside exact seals,
    // rejects ordinary files that happen to contain a copied state packet.
    if fs.f_type as u64 != 0x0102_1994 {
        return Err(Error::Invalid("native snapshot state is not a memfd"));
    }
    let owned = fd
        .try_clone_to_owned()
        .map_err(|error| Error::Source(error.to_string()))?;
    Ok((File::from(owned), size))
}

fn read_state_file(
    file: &File,
    size: u64,
    cap: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Vec<u8>> {
    let size = usize::try_from(size).map_err(|_| Error::Budget("native snapshot state size"))?;
    if size == 0 || size > cap {
        return Err(Error::Budget("native snapshot state bytes"));
    }
    let mut raw = vec![0; size];
    let mut offset = 0usize;
    while offset < size {
        check_snapshot_active(cancelled, deadline)?;
        let count = file
            .read_at(&mut raw[offset..], offset as u64)
            .map_err(|error| Error::Source(error.to_string()))?;
        if count == 0 {
            return Err(Error::Invalid("native snapshot state descriptor truncated"));
        }
        offset = offset
            .checked_add(count)
            .ok_or(Error::Budget("native snapshot state read"))?;
    }
    Ok(raw)
}

fn issue_state_fd(
    capture: &PublicCapture,
    source_revision: &str,
    graph: &serde_json::Value,
    catalog: &serde_json::Value,
    graph_root_sha256: &str,
    catalog_sha256: &str,
    max_bytes: usize,
    json: JsonLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<ProducerIssuedCoreSnapshotState> {
    if !valid_sha256(source_revision)
        || !valid_sha256(graph_root_sha256)
        || !valid_sha256(catalog_sha256)
        || !capture.root().is_absolute()
    {
        return Err(Error::Invalid("native snapshot state producer binding"));
    }
    check_snapshot_active(cancelled, deadline)?;
    let (source_inputs, input_sha256) = source_values_from_capture(
        capture,
        source_revision,
        max_bytes,
        json,
        deadline,
        cancelled,
    )?;
    let root = capture
        .root()
        .to_str()
        .ok_or(Error::Invalid("native snapshot state root UTF8"))?;
    let capture_identity = capture.capture_identity()?;
    let source_state = capture.core_source_state()?;
    let capture_source_state = capture.capture_source_state()?;
    let empty_receipt = "";
    let unsigned_view = state_packet_view(
        root,
        &capture_identity,
        &source_state,
        &capture_source_state,
        source_revision,
        graph_root_sha256,
        catalog_sha256,
        graph,
        catalog,
        &source_inputs,
        &input_sha256,
        empty_receipt,
    );
    let (receipt_sha256, unsigned_bytes) =
        state_receipt_digest(&unsigned_view, max_bytes, deadline, cancelled)?;
    let mut file = memfd_file("tos-native-core-snapshot-state-v1")?;
    let packet_view = state_packet_view(
        root,
        &capture_identity,
        &source_state,
        &capture_source_state,
        source_revision,
        graph_root_sha256,
        catalog_sha256,
        graph,
        catalog,
        &source_inputs,
        &input_sha256,
        &receipt_sha256,
    );
    let final_bytes = {
        let mut writer = StateFileWriter {
            file: &mut file,
            written: 0,
            max_bytes,
            deadline,
            cancelled,
        };
        serde_json::to_writer(&mut writer, &packet_view)
            .map_err(|_| Error::Budget("native snapshot state output bytes"))?;
        writer.written
    };
    if final_bytes == 0 || final_bytes > max_bytes {
        return Err(Error::Budget("native snapshot state bytes"));
    }
    capture.charge_work(
        u64::try_from(unsigned_bytes)
            .ok()
            .and_then(|bytes| bytes.checked_add(final_bytes as u64))
            .ok_or(Error::Budget("native snapshot state work bytes"))?,
    )?;
    file.sync_all()
        .map_err(|error| Error::Source(error.to_string()))?;
    if unsafe {
        libc::fcntl(
            file.as_raw_fd(),
            libc::F_ADD_SEALS,
            CORE_STATE_REQUIRED_SEALS,
        )
    } != 0
    {
        return Err(Error::Source(io::Error::last_os_error().to_string()));
    }
    let (checked, size) = validate_state_fd(file.as_fd(), max_bytes)?;
    if size != final_bytes as u64 {
        return Err(Error::Invalid("native snapshot state descriptor size"));
    }
    drop(checked);
    Ok(ProducerIssuedCoreSnapshotState {
        file,
        size_bytes: size,
        receipt_sha256,
        source_revision: source_revision.to_owned(),
    })
}

/// Import a sealed state descriptor supplied through the authenticated
/// producer-owned SCM_RIGHTS/private registry route. Its receipt is an
/// unkeyed integrity digest and does not authenticate origin by itself;
/// caller identity remains a separate host-side graph-identity CAS.
pub fn import_core_snapshot_state(
    fd: BorrowedFd<'_>,
    max_bytes: usize,
    json: JsonLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<ImportedCoreSnapshotState> {
    check_snapshot_active(cancelled, deadline)?;
    let (file, size) = validate_state_fd(fd, max_bytes)?;
    let raw = read_state_file(&file, size, max_bytes, deadline, cancelled)?;
    let mut packet_limits = json;
    packet_limits.max_bytes = packet_limits.max_bytes.max(max_bytes);
    packet_limits.max_visits = packet_limits.max_visits.saturating_mul(8);
    if raw.len() > packet_limits.max_bytes {
        return Err(Error::Budget("native snapshot state JSON bytes"));
    }
    let parsed = parse_json(&raw, JsonMode::PublishedStrict, packet_limits)
        .map_err(|_| Error::Invalid("native snapshot state JSON"))?;
    drop(parsed);
    let packet: CoreSnapshotStatePacket =
        serde_json::from_slice(&raw).map_err(|_| Error::Invalid("native snapshot state packet"))?;
    if packet.schema_version != "tos_native_core_snapshot_state_v1"
        || packet.role != CORE_STATE_ROLE
        || packet.issuer != CORE_STATE_ISSUER
        || !Path::new(&packet.root).is_absolute()
        || !valid_sha256(&packet.source_revision)
        || !valid_sha256(&packet.graph_root_sha256)
        || !valid_sha256(&packet.catalog_sha256)
        || !valid_sha256(&packet.receipt_sha256)
        || packet
            .graph
            .get("schema")
            .and_then(serde_json::Value::as_str)
            != Some("tos_knowledge_graph_v1")
        || packet
            .graph
            .get("source_revision")
            .and_then(serde_json::Value::as_str)
            != Some(packet.source_revision.as_str())
        || packet.catalog.as_object().is_none()
        || packet.source_inputs.len() != CORE_SOURCE_REFS.len()
        || packet.input_sha256.len() != CORE_SOURCE_REFS.len()
        || packet.source_state.len() != CORE_SOURCE_REFS.len()
        || packet.capture_source_state.len() < packet.source_state.len()
        || CORE_SOURCE_REFS.iter().any(|(name, _)| {
            packet
                .source_inputs
                .get(*name)
                .and_then(serde_json::Value::as_object)
                .is_none()
                || packet
                    .input_sha256
                    .get(*name)
                    .is_none_or(|digest| !valid_sha256(digest))
        })
        || packet.source_state.iter().any(|(path, _, size, inode, _)| {
            !Path::new(path).is_absolute() || *size == 0 || *inode == 0
        })
        || packet
            .capture_source_state
            .iter()
            .any(|(path, _, size, inode, _)| {
                !Path::new(path).is_absolute() || *size == 0 || *inode == 0
            })
        || packet.source_state.iter().any(|source| {
            !packet
                .capture_source_state
                .iter()
                .any(|captured| captured == source)
        })
    {
        return Err(Error::Invalid(
            "native snapshot state issuer/schema/closure",
        ));
    }
    let empty_receipt = "";
    let unsigned_view: CoreSnapshotStatePacketView<'_> = (&packet, empty_receipt).into();
    let (receipt, _) = state_receipt_digest(&unsigned_view, max_bytes, deadline, cancelled)?;
    if receipt != packet.receipt_sha256 {
        return Err(Error::Invalid("native snapshot state receipt"));
    }
    for (name, _) in CORE_SOURCE_REFS {
        let value = packet
            .source_inputs
            .get(name)
            .ok_or(Error::Invalid("native snapshot state source input"))?;
        let digest = source_value_digest(value, json, cancelled, deadline, None)?;
        if packet.input_sha256.get(name) != Some(&digest) {
            return Err(Error::Invalid("native snapshot state source digest"));
        }
    }
    let root = PathBuf::from(packet.root);
    if packet
        .graph
        .get("source_revision")
        .and_then(serde_json::Value::as_str)
        != Some(packet.source_revision.as_str())
    {
        return Err(Error::Invalid("native snapshot state graph revision"));
    }
    check_snapshot_active(cancelled, deadline)?;
    Ok(ImportedCoreSnapshotState {
        _file: file,
        root,
        capture_identity: packet.capture_identity,
        source_state: packet.source_state,
        capture_source_state: packet.capture_source_state,
        source_revision: packet.source_revision,
        graph: packet.graph,
        catalog: packet.catalog,
        source_inputs: packet.source_inputs,
        input_sha256: packet.input_sha256,
        graph_root_sha256: packet.graph_root_sha256,
        catalog_sha256: packet.catalog_sha256,
        receipt_sha256: packet.receipt_sha256,
    })
}

/// Reuse a host-retained graph/catalog pair only when the authenticated prior
/// state and this exact completed capture still describe identical physical
/// and logical source cuts. `None` means the caller must run the real builder;
/// this gate never creates a replacement state descriptor.
pub fn reuse_native_knowledge_snapshot_if_current(
    state_fd: BorrowedFd<'_>,
    capture: &PublicCapture,
    max_state_bytes: usize,
    json: JsonLimits,
    include_catalog_inputs: bool,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Option<ReusedNativeKnowledgeSnapshot>> {
    if include_catalog_inputs {
        return Ok(None);
    }
    check_snapshot_active(cancelled, deadline)?;
    let previous =
        import_core_snapshot_state(state_fd, max_state_bytes, json, deadline, cancelled)?;
    if std::fs::canonicalize(&previous.root)? != std::fs::canonicalize(capture.root())? {
        return Ok(None);
    }
    capture.verify_captured_inputs()?;
    let source_state = capture.core_source_state()?;
    let capture_source_state = capture.capture_source_state()?;
    if previous.source_state != source_state
        || previous.capture_source_state != capture_source_state
    {
        return Ok(None);
    }
    let source_revision = capture.core_source_revision()?;
    if source_revision != previous.source_revision {
        return Ok(None);
    }
    check_snapshot_active(cancelled, deadline)?;
    capture.verify_captured_inputs()?;
    if capture.core_source_state()? != source_state
        || capture.capture_source_state()? != capture_source_state
    {
        return Ok(None);
    }
    let source_inputs = capture
        .retained_input_members()?
        .into_iter()
        .map(|(path, digest, bytes)| (path, digest.to_hex(), bytes))
        .collect();
    Ok(Some(ReusedNativeKnowledgeSnapshot {
        graph: previous.graph,
        catalog: previous.catalog,
        source_revision,
        source_state,
        source_inputs,
        graph_root_sha256: previous.graph_root_sha256,
        catalog_sha256: previous.catalog_sha256,
        state_receipt_sha256: previous.receipt_sha256,
    }))
}

fn strict_snapshot_value(
    raw: &[u8],
    max_bytes: usize,
    json: JsonLimits,
    remaining_visits: &mut usize,
    cancelled: &AtomicBool,
    deadline: Instant,
) -> Result<serde_json::Value> {
    check_snapshot_active(cancelled, deadline)?;
    if raw.is_empty() || raw.len() > max_bytes || raw.len() > json.max_bytes {
        return Err(Error::Budget("native whole snapshot JSON bytes"));
    }
    let mut limits = json;
    limits.max_bytes = limits.max_bytes.min(max_bytes);
    limits.max_visits = limits.max_visits.min(*remaining_visits);
    if limits.max_visits == 0 {
        return Err(Error::Budget("native whole snapshot JSON visits"));
    }
    let parsed = parse_json(raw, JsonMode::PublishedStrict, limits)
        .map_err(|error| Error::Source(error.to_string()))?;
    *remaining_visits = (*remaining_visits)
        .checked_sub(parsed.visits())
        .ok_or(Error::Budget("native whole snapshot JSON visits"))?;
    drop(parsed);
    check_snapshot_active(cancelled, deadline)?;
    let value =
        serde_json::from_slice(raw).map_err(|_| Error::Invalid("native whole snapshot JSON"))?;
    check_snapshot_active(cancelled, deadline)?;
    Ok(value)
}

fn foundation_snapshot_value(
    raw: &[u8],
    max_bytes: usize,
    json: JsonLimits,
    remaining_visits: &mut usize,
    cancelled: &AtomicBool,
    deadline: Instant,
) -> Result<JsonValue> {
    check_snapshot_active(cancelled, deadline)?;
    if raw.is_empty() || raw.len() > max_bytes || raw.len() > json.max_bytes {
        return Err(Error::Budget("native catalog input JSON bytes"));
    }
    let mut limits = json;
    limits.max_bytes = limits.max_bytes.min(max_bytes);
    limits.max_visits = limits.max_visits.min(*remaining_visits);
    if limits.max_visits == 0 {
        return Err(Error::Budget("native catalog input JSON visits"));
    }
    let parsed = parse_json(raw, JsonMode::PublishedStrict, limits)
        .map_err(|error| Error::Source(error.to_string()))?;
    *remaining_visits = (*remaining_visits)
        .checked_sub(parsed.visits())
        .ok_or(Error::Budget("native catalog input JSON visits"))?;
    check_snapshot_active(cancelled, deadline)?;
    Ok(parsed.into_root())
}

struct BoundedSnapshotWriter {
    written: usize,
    max_bytes: usize,
}

impl Write for BoundedSnapshotWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let next = self
            .written
            .checked_add(bytes.len())
            .filter(|next| *next <= self.max_bytes)
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "native snapshot output cap"))?;
        self.written = next;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn staged_rows(
    db: &rusqlite::Connection,
    capture: &PublicCapture,
    table: &'static str,
    max_rows: u64,
    max_row_bytes: usize,
    max_total_bytes: usize,
    json: JsonLimits,
    remaining_visits: &mut usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Vec<serde_json::Value>> {
    if !matches!(table, "knowledge_nodes" | "knowledge_relations") {
        return Err(Error::Invalid("native whole snapshot row table"));
    }
    let sql = format!(
        "SELECT id,source_graph,payload_len,payload_sha256,
                CASE WHEN payload_len BETWEEN 1 AND ?1
                     AND length(payload)=payload_len THEN payload END
           FROM {table} ORDER BY id"
    );
    let mut statement = db.prepare(&sql)?;
    let mut rows = statement.query([max_row_bytes as i64])?;
    let mut result = Vec::new();
    let mut bytes = 0usize;
    while let Some(row) = rows.next()? {
        check_snapshot_active(cancelled, deadline)?;
        let id: String = row.get(0)?;
        let source_graph: String = row.get(1)?;
        let declared: i64 = row.get(2)?;
        let expected: Vec<u8> = row.get(3)?;
        let raw: Option<Vec<u8>> = row.get(4)?;
        let raw = raw.ok_or(Error::Budget("native whole snapshot row bytes"))?;
        capture.charge_work(raw.len() as u64)?;
        if result.len() as u64 >= max_rows
            || declared <= 0
            || declared as usize != raw.len()
            || raw.len() > max_row_bytes
            || expected.len() != 32
            || Digest256::of_bytes(&raw).as_bytes().as_slice() != expected.as_slice()
        {
            return Err(Error::Invalid("native whole snapshot row receipt"));
        }
        let value = strict_snapshot_value(
            &raw,
            max_row_bytes,
            json,
            remaining_visits,
            cancelled,
            deadline,
        )?;
        if value.get("id").and_then(serde_json::Value::as_str) != Some(id.as_str())
            || value
                .get("source_graph")
                .and_then(serde_json::Value::as_str)
                != Some(source_graph.as_str())
        {
            return Err(Error::Invalid("native whole snapshot row identity"));
        }
        let encoded = serde_json::to_vec(&value)
            .map_err(|_| Error::Invalid("native whole snapshot row encoding"))?;
        bytes = bytes
            .checked_add(encoded.len())
            .filter(|next| *next <= max_total_bytes)
            .ok_or(Error::Budget("native whole snapshot graph bytes"))?;
        result.push(value);
    }
    Ok(result)
}

fn whole_snapshot_in_stage(
    stage: &mut KnowledgeStage<'_>,
    capture: &PublicCapture,
    header: &serde_json::Value,
    entity_raw: &[u8],
    relation_raw: &[u8],
    lenses: &[serde_json::Value],
    full: &crate::FullKnowledgeReceipt,
    include_catalog_inputs: bool,
    limits: NativeWholeSnapshotLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(
    serde_json::Value,
    serde_json::Value,
    Option<crate::prepared_catalog_semantics::CatalogInputs>,
)> {
    limits.validate()?;
    if header.as_object().is_none()
        || header.get("nodes").is_some()
        || header.get("relations").is_some()
    {
        return Err(Error::Invalid("native whole snapshot header"));
    }
    let header_bytes = serde_json::to_vec(header)
        .map_err(|_| Error::Invalid("native whole snapshot header encoding"))?;
    if header_bytes.len() > limits.max_graph_bytes.min(limits.json.max_bytes) {
        return Err(Error::Budget("native whole snapshot header bytes"));
    }
    let graph_rows = limits.max_rows;
    let mut remaining_visits = limits.json.max_visits;
    let graph_header = strict_snapshot_value(
        &header_bytes,
        limits.max_graph_bytes,
        limits.json,
        &mut remaining_visits,
        cancelled,
        deadline,
    )?;
    let (nodes, relations, catalog_raw) = stage.with_connection(WritePhase::Finalize, |db| {
        check_snapshot_active(cancelled, deadline)?;
        let node_count: u64 =
            db.query_row("SELECT count(*) FROM knowledge_nodes", [], |row| row.get(0))?;
        let relation_count: u64 =
            db.query_row("SELECT count(*) FROM knowledge_relations", [], |row| {
                row.get(0)
            })?;
        if !node_count
            .checked_add(relation_count)
            .is_some_and(|count| count <= graph_rows)
        {
            return Err(Error::Budget("native whole snapshot row count"));
        }
        let catalog_packet = db.query_row(
            "SELECT packet_len,packet_sha256,
                    CASE WHEN packet_len BETWEEN 1 AND ?2 AND length(packet)=packet_len
                         THEN packet END
               FROM catalog_index_meta WHERE descriptor_sha256=?1",
            rusqlite::params![
                full.catalog.descriptor_sha256,
                limits.max_catalog_bytes as i64
            ],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, Option<Vec<u8>>>(2)?,
                ))
            },
        )?;
        let (declared, expected, raw) = catalog_packet;
        let raw = raw.ok_or(Error::Budget("native whole snapshot catalog bytes"))?;
        capture.charge_work(raw.len() as u64)?;
        if declared < 0
            || declared as usize != raw.len()
            || raw.len() > limits.max_catalog_bytes
            || expected.len() != 32
            || Digest256::of_bytes(&raw).as_bytes().as_slice() != expected.as_slice()
            || Digest256::of_bytes(&raw).to_hex() != full.catalog.catalog_packet_sha256
        {
            return Err(Error::Invalid("native whole snapshot catalog receipt"));
        }
        let nodes = staged_rows(
            db,
            capture,
            "knowledge_nodes",
            graph_rows,
            limits.max_row_bytes,
            limits.max_graph_bytes.saturating_sub(header_bytes.len()),
            limits.json,
            &mut remaining_visits,
            deadline,
            cancelled,
        )?;
        let node_bytes = nodes.iter().try_fold(0usize, |sum, node| {
            let raw = serde_json::to_vec(node)
                .map_err(|_| Error::Invalid("native whole snapshot node encoding"))?;
            sum.checked_add(raw.len())
                .filter(|bytes| *bytes <= limits.max_graph_bytes.saturating_sub(header_bytes.len()))
                .ok_or(Error::Budget("native whole snapshot graph bytes"))
        })?;
        let relations = staged_rows(
            db,
            capture,
            "knowledge_relations",
            graph_rows.saturating_sub(nodes.len() as u64),
            limits.max_row_bytes,
            limits
                .max_graph_bytes
                .saturating_sub(header_bytes.len().saturating_add(node_bytes)),
            limits.json,
            &mut remaining_visits,
            deadline,
            cancelled,
        )?;
        Ok((nodes, relations, raw))
    })?;
    let mut graph = graph_header;
    let graph_object = graph
        .as_object_mut()
        .ok_or(Error::Invalid("native whole snapshot header object"))?;
    if graph_object
        .insert("nodes".into(), serde_json::Value::Array(nodes))
        .is_some()
        || graph_object
            .insert("relations".into(), serde_json::Value::Array(relations))
            .is_some()
    {
        return Err(Error::Invalid("native whole snapshot row header collision"));
    }
    let catalog = strict_snapshot_value(
        &catalog_raw,
        limits.max_catalog_bytes,
        limits.json,
        &mut remaining_visits,
        cancelled,
        deadline,
    )?;
    let catalog_inputs = if include_catalog_inputs {
        let header_input = serde_json::to_vec(header)
            .map_err(|_| Error::Invalid("native catalog header encoding"))?;
        let mut input_bytes = header_input
            .len()
            .checked_add(entity_raw.len())
            .and_then(|bytes| bytes.checked_add(relation_raw.len()))
            .ok_or(Error::Budget("native catalog input bytes"))?;
        if input_bytes > limits.max_catalog_inputs_bytes {
            return Err(Error::Budget("native catalog input bytes"));
        }
        let header = foundation_snapshot_value(
            &header_input,
            limits.max_catalog_inputs_bytes,
            limits.json,
            &mut remaining_visits,
            cancelled,
            deadline,
        )?;
        let entity_registry = foundation_snapshot_value(
            entity_raw,
            limits.max_catalog_inputs_bytes,
            limits.json,
            &mut remaining_visits,
            cancelled,
            deadline,
        )?;
        let relation_registry = foundation_snapshot_value(
            relation_raw,
            limits.max_catalog_inputs_bytes,
            limits.json,
            &mut remaining_visits,
            cancelled,
            deadline,
        )?;
        let mut input_lenses = Vec::with_capacity(lenses.len());
        for lens in lenses {
            let raw = serde_json::to_vec(lens)
                .map_err(|_| Error::Invalid("native catalog lens encoding"))?;
            input_bytes = input_bytes
                .checked_add(raw.len())
                .filter(|bytes| *bytes <= limits.max_catalog_inputs_bytes)
                .ok_or(Error::Budget("native catalog input bytes"))?;
            input_lenses.push(foundation_snapshot_value(
                &raw,
                limits.max_catalog_inputs_bytes,
                limits.json,
                &mut remaining_visits,
                cancelled,
                deadline,
            )?);
        }
        let inputs = crate::prepared_catalog_semantics::CatalogInputs {
            header,
            entity_registry,
            relation_registry,
            lenses: input_lenses,
            source_order_profile:
                crate::prepared_catalog_semantics::SourceOrderProfile::OwnerSequence,
        };
        inputs.validate()?;
        let _ = inputs.binding()?;
        Some(inputs)
    } else {
        None
    };
    let mut writer = BoundedSnapshotWriter {
        written: 0,
        max_bytes: limits.max_graph_bytes.min(limits.json.max_bytes),
    };
    serde_json::to_writer(&mut writer, &graph)
        .map_err(|_| Error::Budget("native whole snapshot graph bytes"))?;
    check_snapshot_active(cancelled, deadline)?;
    capture.charge_work(writer.written as u64)?;
    Ok((graph, catalog, catalog_inputs))
}

/// Short-lived, read-only access to the exact carriers used by a completed
/// native snapshot. Constructed only inside `with_capture_carriers`; the
/// callback's higher-ranked borrow prevents retaining the view beyond its
/// currentness fence.
/// Registry members are selected by producer code from the retained capture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectedKnowledgeRegistry {
    EntityTypes,
    RelationTypes,
}

pub struct CompletedCaptureCarriers<'a> {
    capture: &'a PublicCapture,
    source_revision: Option<&'a str>,
}
impl CompletedCaptureCarriers<'_> {
    /// Full captures carry the five-source Core revision. A role-only carrier
    /// capture has no whole-source revision and reports `None`.
    pub fn source_revision(&self) -> Option<&str> {
        self.source_revision
    }

    /// Recheck the exact held source, partition closure and private capture
    /// inode under the original capture deadline and cancellation token.
    /// Borrowers use this at disclosure boundaries; a revision string alone
    /// does not establish currentness.
    pub fn verify_current(&self) -> Result<()> {
        self.capture.verify_captured_inputs()
    }

    pub fn header(&self, role: &str, path: &str) -> Result<JsonValue> {
        self.capture.header(role, path)
    }

    pub fn header_object(
        &self,
        role: &str,
        prefix: &str,
        max_bytes: usize,
    ) -> Result<serde_json::Value> {
        self.capture.header_object(role, prefix, max_bytes)
    }

    pub fn captured_collection_kind(&self, role: &str, collection: &str) -> Result<Option<String>> {
        self.capture.captured_collection_kind(role, collection)
    }

    pub fn captured_collection_names(&self, role: &str) -> Result<Vec<String>> {
        self.capture.captured_collection_names(role)
    }

    pub fn visit_rows(
        &self,
        role: &str,
        collection: &str,
        sink: impl FnMut(u64, &[u8]) -> Result<()>,
    ) -> Result<u64> {
        self.capture.visit_rows(role, collection, sink)
    }

    pub fn retained_inputs(&self) -> Result<Vec<(String, String, u64)>> {
        self.capture
            .retained_input_members()?
            .into_iter()
            .map(|(path, digest, bytes)| Ok((path, digest.to_hex(), bytes)))
            .collect()
    }

    pub fn charge_work(&self, bytes: u64) -> Result<()> {
        self.capture.charge_work(bytes)
    }

    /// Provenance path of the present audit member held by this capture.
    /// The borrow confers no independent read authority outside this view.
    pub fn philosophy_audit_path(&self) -> Result<&Path> {
        self.capture.selected_philosophy_audit_path()
    }

    /// Read the exact captured registry through the original work and custody
    /// ledger. A role-only capture lacking that member is refused.
    pub fn read_registry(
        &self,
        registry: SelectedKnowledgeRegistry,
        max_bytes: usize,
    ) -> Result<Vec<u8>> {
        let label = match registry {
            SelectedKnowledgeRegistry::EntityTypes => {
                "ToS/doctrine/semantic-interchange/entity-types.v1.json"
            }
            SelectedKnowledgeRegistry::RelationTypes => {
                "ToS/doctrine/semantic-interchange/relation-types.v1.json"
            }
        };
        self.capture.read_retained_input(label, max_bytes)
    }

    pub fn read_philosophy_audit(&self, max_bytes: usize) -> Result<Vec<u8>> {
        self.capture.read_selected_philosophy_audit(max_bytes)
    }
}

impl<'a> CompletedCaptureCarriers<'a> {
    pub(crate) fn from_selected_capture(capture: &'a PublicCapture) -> Self {
        Self {
            capture,
            source_revision: None,
        }
    }
}

fn snapshot_model_identity(file: &File) -> Result<SnapshotModelIdentity> {
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(Error::Invalid("native snapshot model file type"));
    }
    Ok(SnapshotModelIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
        size: metadata.len(),
        mtime_seconds: metadata.mtime(),
        mtime_nanoseconds: metadata.mtime_nsec(),
        ctime_seconds: metadata.ctime(),
        ctime_nanoseconds: metadata.ctime_nsec(),
    })
}

fn snapshot_model_path_identity(path: &Path, expected_size: u64) -> Result<SnapshotModelIdentity> {
    let file = crate::safe_open::open_regular(path, expected_size)?;
    let held = snapshot_model_identity(&file)?;
    let named = fs::symlink_metadata(path)?;
    let named_identity = SnapshotModelIdentity {
        device: named.dev(),
        inode: named.ino(),
        size: named.len(),
        mtime_seconds: named.mtime(),
        mtime_nanoseconds: named.mtime_nsec(),
        ctime_seconds: named.ctime(),
        ctime_nanoseconds: named.ctime_nsec(),
    };
    if !named.file_type().is_file() || held != named_identity || held.size != expected_size {
        return Err(Error::Invalid("native snapshot model path rebound"));
    }
    Ok(held)
}

fn verify_snapshot_model_memfd(file: &File, expected: SnapshotModelIdentity) -> Result<()> {
    if snapshot_model_identity(file)? != expected {
        return Err(Error::Invalid(
            "native snapshot model memfd identity changed",
        ));
    }
    let seals = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GET_SEALS) };
    if seals != CORE_STATE_REQUIRED_SEALS {
        return Err(Error::Invalid("native snapshot model memfd seals"));
    }
    let mut fs: libc::statfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstatfs(file.as_raw_fd(), &mut fs) } != 0 {
        return Err(Error::Io(io::Error::last_os_error()));
    }
    if fs.f_type as u64 != 0x0102_1994 {
        return Err(Error::Invalid("native snapshot model is not a memfd"));
    }
    Ok(())
}

fn same_cold_open_limits(left: crate::ColdOpenLimits, right: crate::ColdOpenLimits) -> bool {
    left.max_file_bytes == right.max_file_bytes
        && left.max_vm_steps == right.max_vm_steps
        && left.sqlite_cache_kib == right.sqlite_cache_kib
        && left.max_rows == right.max_rows
        && left.max_work_bytes == right.max_work_bytes
        && left.max_row_bytes == right.max_row_bytes
        && left.max_metadata_bytes == right.max_metadata_bytes
        && left.max_sources == right.max_sources
}

struct SnapshotModelMemfdCustody<'a> {
    file: &'a File,
    identity: SnapshotModelIdentity,
    expected: &'a KnowledgeSelectedExpectation,
    cold: crate::ColdOpenLimits,
    process: crate::NativeProcessLimits,
    working_ram_bytes: u64,
    stage_model_bytes: u64,
    resource_hold: &'a dyn NativeColdOpenResourceHold,
    deadline: Instant,
    cancelled: &'a AtomicBool,
}

impl crate::ImmutableKnowledgeCustody for SnapshotModelMemfdCustody<'_> {
    fn verify(&self, pinned: &File, expected: &KnowledgeSelectedExpectation) -> Result<()> {
        if expected.model_sha256 != self.expected.model_sha256
            || expected.model_size_bytes != self.expected.model_size_bytes
            || expected.owner_receipt_id != self.expected.owner_receipt_id
        {
            return Err(Error::Invalid(
                "native snapshot selected expectation changed",
            ));
        }
        let pinned_identity = snapshot_model_identity(pinned)?;
        if pinned_identity != self.identity {
            return Err(Error::Invalid("native snapshot selected model FD changed"));
        }
        verify_snapshot_model_memfd(pinned, self.identity)?;
        crate::ImmutableKnowledgeCustody::verify_cold_resources(self, self.cold)
    }

    fn verify_cold_resources(&self, limits: crate::ColdOpenLimits) -> Result<()> {
        if !same_cold_open_limits(limits, self.cold) {
            return Err(Error::Invalid("native snapshot cold limits changed"));
        }
        check_snapshot_active(self.cancelled, self.deadline)?;
        crate::native_knowledge_selection::verify_native_process_limits(self.process)?;
        verify_snapshot_model_memfd(self.file, self.identity)?;
        self.resource_hold.verify_cold_open(
            self.cold,
            self.process,
            self.working_ram_bytes,
            self.stage_model_bytes,
            self.identity.size,
            0,
            0,
            self.deadline,
            self.cancelled,
        )?;
        check_snapshot_active(self.cancelled, self.deadline)
    }
}

/// Short-lived borrow of the actual checked Evidence Lens projection file.
/// The holder keeps its independently selected source closure alive and
/// rechecks it before and after every consumer callback.
pub struct CompletedEvidenceProjectionView<'a> {
    pub(crate) evidence: &'a crate::epistemic_evidence::CompletedEvidenceProjection,
}
impl CompletedEvidenceProjectionView<'_> {
    pub fn raw(&self) -> &[u8] {
        self.evidence.raw()
    }

    pub fn source_revision(&self) -> &str {
        self.evidence.source_revision()
    }

    pub fn charge_work(&self, bytes: u64) -> Result<()> {
        self.evidence.charge_work(bytes)
    }
}

impl CompletedNativeSnapshot {
    /// Consume unused producer diagnostics before a Reference query callback.
    /// The exact descriptor, receipts, model identity and source bindings remain
    /// unchanged; Graph/Snapshot/addressed report producers do not take this
    /// transition and retain their existing diagnostic values.
    pub fn into_reference_query_delivery(mut self) -> Self {
        self.semantic_report = serde_json::Value::Null;
        self.vocabulary.discard_non_query_policy_caches();
        self
    }

    /// This owner's logical retained state, including the distinct original
    /// Stage model. A sealed model lent by with_verified_model is charged by
    /// that model owner, not charged again here.
    pub fn retained_query_state_upper_bound(&self) -> Result<usize> {
        use tos_foundation::{OwnedState, checked_state_add};
        if !self.semantic_report.is_null() {
            return Err(Error::Invalid("query delivery diagnostics still retained"));
        }
        let mut bytes = std::mem::size_of::<Self>();
        for amount in [
            std::mem::size_of::<SnapshotModelMemfdCustody<'_>>(),
            std::mem::size_of::<File>(),
        ] {
            bytes = checked_state_add(bytes, amount)
                .map_err(|_| Error::Budget("completed query cold custody state"))?;
        }
        macro_rules! charge { ($($field:ident),*) => { $(
            bytes = checked_state_add(bytes, self.$field.owned_heap_bytes()
                .map_err(|_| Error::Budget("completed query retained state"))?)
                .map_err(|_| Error::Budget("completed query retained state"))?;
        )* }; }
        charge!(
            path,
            stage,
            full,
            expectation,
            producer,
            source_revision,
            descriptor,
            entity,
            relation
        );
        bytes = checked_state_add(bytes, self.vocabulary.query_delivery_heap_bytes()?)
            .map_err(|_| Error::Budget("completed query vocabulary state"))?;
        bytes = checked_state_add(
            bytes,
            usize::try_from(self.stage_model_identity.size)
                .map_err(|_| Error::Budget("completed query stage size"))?,
        )
        .map_err(|_| Error::Budget("completed query stage state"))?;
        Ok(bytes)
    }

    pub fn artifact_path(&self) -> &Path {
        &self.path
    }
    pub fn stage(&self) -> &crate::knowledge_stage::StageReceipt {
        &self.stage
    }
    pub fn components(&self) -> &crate::FullKnowledgeReceipt {
        &self.full
    }
    pub fn expectation(&self) -> &KnowledgeSelectedExpectation {
        &self.expectation
    }
    pub fn producer(&self) -> &crate::knowledge_native::NativeProducerReceipt {
        &self.producer
    }
    pub fn declaration_sha256(&self) -> Digest256 {
        self.declaration_sha256
    }
    pub fn source_revision(&self) -> &str {
        &self.source_revision
    }

    /// Lend exact original projection carriers only while their completed
    /// snapshot binding and physical source inputs remain current. The callback
    /// must build a disposable value; publication belongs to a separate owner.
    pub fn with_capture_carriers<'a, T>(
        &'a self,
        capture: &'a PublicCapture,
        consume: impl for<'view> FnOnce(&'view CompletedCaptureCarriers<'a>) -> Result<T>,
    ) -> Result<T> {
        self.check_capture_binding(capture)?;
        let view = CompletedCaptureCarriers {
            capture,
            source_revision: Some(&self.source_revision),
        };
        let result = consume(&view);
        let current = self.check_capture_binding(capture);
        match current {
            Err(error) => Err(error),
            Ok(()) => result,
        }
    }

    /// Lend the exact finished Evidence Lens output only when its independent
    /// source/output closure is bound to this exact completed graph capture.
    pub fn with_evidence_projection<'a, T>(
        &'a self,
        capture: &'a PublicCapture,
        evidence: &'a crate::epistemic_evidence::CompletedEvidenceProjection,
        consume: impl for<'view> FnOnce(&'view CompletedEvidenceProjectionView<'a>) -> Result<T>,
    ) -> Result<T> {
        self.check_capture_binding(capture)?;
        evidence.verify_binding(capture, &self.source_revision)?;
        let view = CompletedEvidenceProjectionView { evidence };
        let result = consume(&view);
        let evidence_current = evidence.verify_current();
        let capture_current = self.check_capture_binding(capture);
        match (evidence_current, capture_current) {
            (Err(error), _) => Err(error),
            (_, Err(error)) => Err(error),
            (Ok(()), Ok(())) => result,
        }
    }

    /// Open this exact completed model through the maintained selected cold
    /// verifier while preserving the producer's capture and resource holds.
    /// The callback cannot retain the model borrow or its anonymous sealed
    /// SQLite copy. Its host resource hold must verify the real hard-memory,
    /// swap and process limits; stage-ticket numbers alone are not sufficient.
    #[allow(clippy::too_many_arguments)]
    pub fn with_verified_model<T>(
        &self,
        capture: &PublicCapture,
        isolation: &dyn StageIsolation,
        cold: crate::ColdOpenLimits,
        process: crate::NativeProcessLimits,
        working_ram_bytes: u64,
        resource_hold: &dyn NativeColdOpenResourceHold,
        deadline: Instant,
        cancelled: &AtomicBool,
        consume: impl for<'model> FnOnce(
            &mut crate::VerifiedKnowledgeModel<'model>,
            &QueryVocabulary,
            &[u8],
        ) -> Result<T>,
    ) -> Result<T> {
        if deadline != capture.deadline()
            || !std::ptr::eq(cancelled, capture.cancellation())
            || working_ram_bytes == 0
            || process.address_space_bytes == 0
            || process.file_size_bytes < self.expectation.model_size_bytes
            || process.address_space_bytes > working_ram_bytes
            || process.address_space_bytes >= libc::RLIM_INFINITY as u64
            || process.file_size_bytes >= libc::RLIM_INFINITY as u64
            || cold.max_file_bytes < self.expectation.model_size_bytes
        {
            return Err(Error::Invalid("native selected cold-open admission"));
        }
        crate::knowledge_selected::validate(&self.expectation, cold)?;
        check_snapshot_active(cancelled, deadline)?;
        crate::native_knowledge_selection::verify_native_process_limits(process)?;
        crate::native_knowledge_selection::verify_native_file_size_limit(
            self.expectation.model_size_bytes,
        )?;
        self.check_capture_binding(capture)?;
        self.verify_stage_model(isolation, deadline, cancelled)?;
        resource_hold.verify_cold_open(
            cold,
            process,
            working_ram_bytes,
            self.stage_model_identity.size,
            self.expectation.model_size_bytes,
            self.expectation.model_size_bytes,
            65_536,
            deadline,
            cancelled,
        )?;
        let model_copy = self.sealed_model_copy(
            capture,
            isolation,
            cold,
            process,
            working_ram_bytes,
            resource_hold,
            deadline,
            cancelled,
        )?;
        let model_identity = snapshot_model_identity(&model_copy)?;
        verify_snapshot_model_memfd(&model_copy, model_identity)?;
        let custody = SnapshotModelMemfdCustody {
            file: &model_copy,
            identity: model_identity,
            expected: &self.expectation,
            cold,
            process,
            working_ram_bytes,
            stage_model_bytes: self.stage_model_identity.size,
            resource_hold,
            deadline,
            cancelled,
        };
        let mut model = crate::knowledge_selected::open_selected_knowledge_model_pinned(
            model_copy.try_clone()?,
            self.expectation.clone(),
            &custody,
            cold,
        )?;
        let result = consume(&mut model, &self.vocabulary, &self.descriptor);
        drop(model);
        let current = (|| {
            check_snapshot_active(cancelled, deadline)?;
            self.check_capture_binding(capture)?;
            self.verify_stage_model(isolation, deadline, cancelled)?;
            crate::ImmutableKnowledgeCustody::verify(&custody, &model_copy, &self.expectation)?;
            Ok(())
        })();
        match current {
            Err(error) => Err(error),
            Ok(()) => result,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn sealed_model_copy(
        &self,
        capture: &PublicCapture,
        isolation: &dyn StageIsolation,
        cold: crate::ColdOpenLimits,
        process: crate::NativeProcessLimits,
        working_ram_bytes: u64,
        resource_hold: &dyn NativeColdOpenResourceHold,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<File> {
        self.verify_stage_model(isolation, deadline, cancelled)?;
        resource_hold.verify_cold_open(
            cold,
            process,
            working_ram_bytes,
            self.stage_model_identity.size,
            self.expectation.model_size_bytes,
            self.expectation.model_size_bytes,
            65_536,
            deadline,
            cancelled,
        )?;
        let mut source =
            crate::safe_open::open_regular(&self.path, self.expectation.model_size_bytes)?;
        if snapshot_model_identity(&source)? != self.stage_model_identity {
            return Err(Error::Invalid("native snapshot model stage inode changed"));
        }
        let mut copy = memfd_file("tos-native-core-selected-model-v1")?;
        let mut hasher = tos_foundation::Digest256Hasher::new();
        let mut total = 0u64;
        let mut buffer = [0u8; 65_536];
        loop {
            check_snapshot_active(cancelled, deadline)?;
            let remaining = self
                .expectation
                .model_size_bytes
                .checked_sub(total)
                .ok_or(Error::Budget("native selected model copy bytes"))?;
            let read_limit = usize::try_from(remaining.saturating_add(1))
                .unwrap_or(usize::MAX)
                .min(buffer.len());
            let count = source.read(&mut buffer[..read_limit])?;
            if count == 0 {
                break;
            }
            capture.charge_work(count as u64)?;
            let next_total = total
                .checked_add(count as u64)
                .ok_or(Error::Budget("native selected model copy bytes"))?;
            if next_total > self.expectation.model_size_bytes {
                return Err(Error::Invalid(
                    "native selected model source grew during copy",
                ));
            }
            resource_hold.verify_cold_open(
                cold,
                process,
                working_ram_bytes,
                self.stage_model_identity.size,
                self.expectation.model_size_bytes,
                remaining,
                buffer.len() as u64,
                deadline,
                cancelled,
            )?;
            hasher.update(&buffer[..count]);
            copy.write_all(&buffer[..count])?;
            total = next_total;
        }
        if total != self.expectation.model_size_bytes
            || hasher.finalize().to_hex() != self.expectation.model_sha256
            || snapshot_model_identity(&source)? != self.stage_model_identity
            || snapshot_model_path_identity(&self.path, self.expectation.model_size_bytes)?
                != self.stage_model_identity
        {
            return Err(Error::Invalid(
                "native selected model source changed or digest differs",
            ));
        }
        copy.sync_all()?;
        if unsafe {
            libc::fcntl(
                copy.as_raw_fd(),
                libc::F_ADD_SEALS,
                CORE_STATE_REQUIRED_SEALS,
            )
        } != 0
        {
            return Err(Error::Io(io::Error::last_os_error()));
        }
        let identity = snapshot_model_identity(&copy)?;
        verify_snapshot_model_memfd(&copy, identity)?;
        resource_hold.verify_cold_open(
            cold,
            process,
            working_ram_bytes,
            self.stage_model_identity.size,
            identity.size,
            0,
            65_536,
            deadline,
            cancelled,
        )?;
        check_snapshot_active(cancelled, deadline)?;
        Ok(copy)
    }

    fn verify_stage_model(
        &self,
        isolation: &dyn StageIsolation,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<()> {
        check_snapshot_active(cancelled, deadline)?;
        if self.stage.sqlite_sha256 != self.expectation.model_sha256
            || self.stage.sqlite_size_bytes != self.expectation.model_size_bytes
        {
            return Err(Error::Invalid(
                "native snapshot stage/model receipt mismatch",
            ));
        }
        isolation.verify(&self.path, self.stage_limits, WritePhase::Finalize)?;
        if snapshot_model_path_identity(&self.path, self.expectation.model_size_bytes)?
            != self.stage_model_identity
        {
            return Err(Error::Invalid(
                "native snapshot stage model identity changed",
            ));
        }
        check_snapshot_active(cancelled, deadline)
    }

    fn check_capture_binding(&self, capture: &PublicCapture) -> Result<()> {
        if capture.capture_identity()? != self.capture_identity
            || !self.expectation.complete
            || self.stage.source_cut != format!("native-projection:{}", self.source_revision)
            || self.expectation.source_cut != self.stage.source_cut
            || self.expectation.owner_receipt_id
                != format!(
                    "native-snapshot:{}:{}",
                    self.source_revision,
                    self.declaration_sha256.to_hex()
                )
        {
            return Err(Error::Invalid("native snapshot capture binding"));
        }
        capture.verify_captured_inputs()
    }
    /// After the host copies the finished model once to its fresh filesystem
    /// destination, seal that exact copy and serialize independently produced
    /// roots. No fields are inferred by opening the candidate SQLite model.
    pub fn selection_for_copied_model(
        &self,
        copied_model: &Path,
        paths: crate::NativeSelectionPaths,
        cold: crate::ColdOpenLimits,
        process: crate::NativeProcessLimits,
        max_bytes: usize,
    ) -> Result<crate::NativeKnowledgeSelection> {
        let measurement = crate::prepare_native_knowledge_artifact(copied_model, &self.stage)?;
        crate::NativeKnowledgeSelection::from_producer(
            paths,
            crate::NativeSelectionProducer {
                stage: self.stage.clone(),
                seal: self.full.seal.clone(),
                navigation_original: self.producer.navigation_original.clone(),
                philosophy_original: self.producer.philosophy_original.clone(),
                corpus_original: self.producer.corpus_original.clone(),
                managed_source: None,
                managed_source_v2: None,
            },
            self.expectation.clone(),
            measurement,
            cold,
            process,
            &self.descriptor,
            &self.entity,
            &self.relation,
            NATIVE_KNOWLEDGE_ADAPTER_PROFILES,
            max_bytes,
        )
    }

    pub fn descriptor(&self) -> &[u8] {
        &self.descriptor
    }
    pub fn entity_registry(&self) -> &[u8] {
        &self.entity
    }
    pub fn relation_registry(&self) -> &[u8] {
        &self.relation
    }
}

/// Check and retain the actual finished Evidence Lens carrier under its
/// maintained owner while binding it to the same complete capture cut.
pub fn check_completed_evidence_projection(
    capture: &PublicCapture,
    staging: &Path,
    limits: PublicCaptureLimits,
    deadline: std::time::Instant,
) -> Result<crate::epistemic_evidence::CompletedEvidenceProjection> {
    crate::epistemic_evidence::check_completed(capture, staging, limits, deadline)
}

#[derive(Clone, Copy, Debug)]
pub struct NativeSnapshotLimits {
    pub capture: PublicCaptureLimits,
    pub stage: StageLimits,
    pub native: NativeProducerLimits,
    pub full: FullKnowledgeLimits,
    pub originals: crate::CorpusOriginalSourceLimits,
    pub max_transfer_work_bytes: u64,
    pub max_declaration_bytes: usize,
}
fn input(capture: &PublicCapture, path: &str) -> Result<Vec<u8>> {
    capture
        .read_input(path, 4 * 1024 * 1024)?
        .ok_or(Error::Invalid("native snapshot required captured input"))
}

/// Reuse the actual maintained capture and full component pipeline. The host
/// retains its kernel-backed StageIsolation for the capture, stage and spill
/// lifetime; caller retains the exact declaration bytes through disclosure.
/// The declaration is the software-owned runtime-data allowlist, never a
/// source-owner grant. Candidate must be a fresh private path.
pub fn build_native_snapshot_from_capture(
    capture: &PublicCapture,
    candidate: &Path,
    declaration_raw: &[u8],
    isolation: &dyn StageIsolation,
    limits: NativeSnapshotLimits,
) -> Result<CompletedNativeSnapshot> {
    let cancelled = capture.cancellation();
    let deadline = capture.deadline();
    build_native_snapshot_from_capture_inner(
        capture,
        candidate,
        declaration_raw,
        isolation,
        limits,
        None,
        deadline,
        cancelled,
    )
    .map(|(completed, _)| completed)
}

/// Produce the explicit whole Core snapshot from the same successful native
/// stage as its selected model receipt. This opt-in route materializes the
/// complete public graph and compiled catalog under caller-provided bounds;
/// optional `CatalogInputs` are detached copies of the actual inputs used by
/// that same compilation. It does not retain a mutable source transition
/// baseline or grant addressed-update authority by itself.
pub fn build_native_knowledge_snapshot_from_capture(
    capture: &PublicCapture,
    candidate: &Path,
    declaration_raw: &[u8],
    isolation: &dyn StageIsolation,
    limits: NativeSnapshotLimits,
    whole_limits: NativeWholeSnapshotLimits,
    include_catalog_inputs: bool,
    retain_state: bool,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(CompletedNativeSnapshot, NativeKnowledgeSnapshot)> {
    let (completed, whole) = build_native_snapshot_from_capture_inner(
        capture,
        candidate,
        declaration_raw,
        isolation,
        limits,
        Some((whole_limits, include_catalog_inputs, retain_state)),
        deadline,
        cancelled,
    )?;
    whole
        .map(|snapshot| (completed, snapshot))
        .ok_or(Error::Invalid("native whole snapshot output absent"))
}

/// Apply one exact owner-supplied record replacement to a producer-issued
/// snapshot baseline. Source state and graph identity arrive separately: the
/// host must first resolve `previous_graph is published_graph` to this exact
/// retained descriptor. The kernel then verifies the sealed baseline, full
/// source delta and target revision before rebuilding through the maintained
/// native pipeline. The successor descriptor is issued only after all normal
/// completion/currentness fences pass.
pub fn build_native_addressed_knowledge_snapshot_from_capture(
    previous_state_fd: BorrowedFd<'_>,
    capture: &PublicCapture,
    candidate: &Path,
    declaration_raw: &[u8],
    isolation: &dyn StageIsolation,
    limits: NativeSnapshotLimits,
    whole_limits: NativeWholeSnapshotLimits,
    include_catalog_inputs: bool,
    update: NativeAddressedUpdate<'_>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(
    CompletedNativeSnapshot,
    NativeKnowledgeSnapshot,
    Option<NativeAddressedUpdateReport>,
)> {
    if deadline != capture.deadline() || !std::ptr::eq(cancelled, capture.cancellation()) {
        return Err(Error::Invalid("native addressed admission context changed"));
    }
    check_snapshot_active(cancelled, deadline)?;
    whole_limits.validate()?;
    let previous = import_core_snapshot_state(
        previous_state_fd,
        whole_limits.max_state_bytes,
        whole_limits.json,
        deadline,
        cancelled,
    )?;
    let delta = validate_addressed_transition(
        &previous,
        capture,
        update,
        whole_limits.max_state_bytes,
        whole_limits.json,
        deadline,
        cancelled,
    )?;
    let (completed, snapshot) = build_native_knowledge_snapshot_from_capture(
        capture,
        candidate,
        declaration_raw,
        isolation,
        limits,
        whole_limits,
        include_catalog_inputs,
        true,
        deadline,
        cancelled,
    )?;
    if snapshot.source_revision != update.source_revision || snapshot.state.is_none() {
        return Err(Error::Invalid("native addressed successor binding"));
    }
    let report = if update.return_report {
        Some(build_native_addressed_report(
            &previous,
            &completed,
            &snapshot,
            &delta,
            update,
            capture.rows,
            whole_limits.max_rows,
        )?)
    } else {
        None
    };
    Ok((completed, snapshot, report))
}

#[allow(clippy::too_many_arguments)]
fn build_native_snapshot_from_capture_inner(
    capture: &PublicCapture,
    candidate: &Path,
    declaration_raw: &[u8],
    isolation: &dyn StageIsolation,
    limits: NativeSnapshotLimits,
    whole_mode: Option<(NativeWholeSnapshotLimits, bool, bool)>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(CompletedNativeSnapshot, Option<NativeKnowledgeSnapshot>)> {
    if deadline != capture.deadline() || !std::ptr::eq(cancelled, capture.cancellation()) {
        return Err(Error::Invalid("native snapshot admission context changed"));
    }
    check_snapshot_active(cancelled, deadline)?;
    if let Some((whole_limits, _, _)) = whole_mode {
        whole_limits.validate()?;
    }
    let capture_identity = capture.capture_identity()?;
    let source_state_before = capture.core_source_state()?;
    let capture_source_state_before = capture.capture_source_state()?;
    if limits.max_declaration_bytes == 0
        || limits.max_declaration_bytes > 1024 * 1024
        || declaration_raw.is_empty()
        || declaration_raw.len() > limits.max_declaration_bytes
        || limits.max_transfer_work_bytes == 0
    {
        return Err(Error::Budget("native snapshot declaration/transfer limits"));
    }
    let declaration = parse_json(
        declaration_raw,
        JsonMode::PublishedStrict,
        JsonLimits::new(limits.max_declaration_bytes, 64, 65536, 4096)
            .map_err(|_| Error::Budget("native snapshot declaration JSON"))?,
    )
    .map_err(|e| Error::Source(e.to_string()))?;
    check_snapshot_active(cancelled, deadline)?;
    if declaration
        .root()
        .object_get("schema_version")
        .and_then(|v| v.as_str())
        != Some("tos_access_runtime_data_allowlist_v1")
    {
        return Err(Error::Invalid("native snapshot runtime declaration"));
    }
    capture.check_custody()?;
    let declaration_sha256 = Digest256::of_bytes(declaration_raw);
    let source_revision = capture.core_source_revision()?;
    check_snapshot_active(cancelled, deadline)?;
    let entity = input(
        capture,
        "ToS/doctrine/semantic-interchange/entity-types.v1.json",
    )?;
    let relation = input(
        capture,
        "ToS/doctrine/semantic-interchange/relation-types.v1.json",
    )?;
    let descriptor = input(
        capture,
        "ToS/doctrine/semantic-interchange/query-vocabulary.v1.json",
    )?;
    let registry = validate_public_current_registries(capture, &entity, &relation)?;
    let vocabulary = QueryVocabulary::parse(&descriptor, NATIVE_KNOWLEDGE_ADAPTER_PROFILES)?;
    prepare_family_rows(capture, limits.capture)?;
    let (collections, membership_root, projection_root) =
        captured_input_roots(capture, &vocabulary)?;
    // The chosen original corpus/phi connector selects the existing V5 ABI.
    // Original receipts remain distinct from normalized projections.
    let abi = crate::KNOWLEDGE_CORPUS_MODEL_ABI;
    let binding = SourceBinding {
        owner_profile: "tos-native-projection-snapshot-v1".into(),
        source_cut: format!("native-projection:{source_revision}"),
        through_commit_seq: 0,
        membership_root,
        index_generation: format!("{abi}:{}", declaration_sha256.to_hex()),
        route_map_version: "tos-access-runtime-data-v1".into(),
        reader_abi: abi.into(),
        projection_root_sha256: projection_root.to_hex(),
        complete: true,
    };
    check_snapshot_active(cancelled, deadline)?;
    let originals = crate::native_snapshot_originals::prepare(
        capture,
        &binding,
        &vocabulary,
        limits.originals,
        deadline,
        cancelled,
    )?;
    check_snapshot_active(cancelled, deadline)?;
    if originals.expected_model_abi() != abi {
        return Err(Error::Invalid("native snapshot original component ABI"));
    }
    let receipt = ExactInputReceipt {
        binding: binding.clone(),
        collections,
    };
    let owner = PublicStageOwner {
        capture,
        receipt: receipt.clone(),
    };
    let mut stage = KnowledgeStage::create_captured_native_snapshot(
        candidate,
        limits.stage,
        receipt,
        &owner,
        isolation,
        capture.vm_counter(),
        capture.work_counter(),
        capture.cancellation_handle(),
        capture.max_work_bytes(),
        capture.deadline(),
    )?;
    check_snapshot_active(cancelled, deadline)?;
    ingest_family_rows(&mut stage, capture, limits.max_transfer_work_bytes)?;
    check_snapshot_active(cancelled, deadline)?;
    let nav_raw =
        serde_json::to_vec(&capture.header_object("corpus", "source_navigation", 1024 * 1024)?)
            .map_err(|e| Error::Source(e.to_string()))?;
    let nav = NavigationHeaderClaim {
        expected_sha256: Digest256::of_bytes(&nav_raw).to_hex(),
        raw_json: nav_raw,
    };
    let repository =
        PublicRepositoryRoot::captured_native_projection(capture, &stage, &source_revision)?;
    let borrowed_originals = originals.borrowed();
    let mut families = borrowed_originals.family_inputs(limits.native);
    families.repository_root = Some(repository.input());
    let producer = crate::materialize_native_sources_with_inputs(
        &mut stage,
        &registry,
        &entity,
        &relation,
        &vocabulary,
        &descriptor,
        &nav,
        limits.native,
        families,
    )?;
    check_snapshot_active(cancelled, deadline)?;
    let semantics =
        validate_native_snapshot_semantics(&mut stage, capture, &registry, &entity, &relation)?;
    check_snapshot_active(cancelled, deadline)?;
    let (processor, configuration) =
        crate::d1_public_build::processor_binding(&repository, &descriptor, &entity, &relation)?;
    let header = build_native_snapshot_header(
        &mut stage,
        capture,
        &registry,
        &entity,
        &source_revision,
        processor,
        configuration,
        &semantics,
    )?;
    check_snapshot_active(cancelled, deadline)?;
    let lenses = saved_lenses(capture)?;
    let full = crate::compile_full_knowledge_components(
        &mut stage,
        &header,
        &registry,
        &entity,
        &relation,
        &lenses,
        &vocabulary,
        &descriptor,
        limits.full,
    )?;
    check_snapshot_active(cancelled, deadline)?;
    if full.seal.model_abi != abi || full.seal.managed_source_root_sha256.is_some() {
        return Err(Error::Invalid("native snapshot actual component ABI"));
    }
    let source_scopes = stage.with_connection(WritePhase::Finalize, |db| {
        let mut statement = db.prepare("SELECT source_graph,input_role,adapter_profile,expected_node_count,expected_relation_count,lower(hex(node_root_sha256)),lower(hex(relation_root_sha256)) FROM source_scope ORDER BY source_graph")?;
        let result = statement.query_map([], |r| Ok(ExpectedSourceScope { source_graph:r.get(0)?, input_role:r.get(1)?, adapter_profile:r.get(2)?, node_count:r.get(3)?, relation_count:r.get(4)?, node_root_sha256:r.get(5)?, relation_root_sha256:r.get(6)? }))?
            .collect::<std::result::Result<Vec<_>,_>>()?;
        Ok(result)
    })?;
    let whole_parts = if let Some((whole_limits, include_catalog_inputs, _)) = whole_mode {
        Some(whole_snapshot_in_stage(
            &mut stage,
            capture,
            &header,
            &entity,
            &relation,
            &lenses,
            &full,
            include_catalog_inputs,
            whole_limits,
            deadline,
            cancelled,
        )?)
    } else {
        None
    };
    let output = stage.finish()?;
    check_snapshot_active(cancelled, deadline)?;
    let completed_path = fs::canonicalize(candidate)?;
    isolation.verify(&completed_path, limits.stage, WritePhase::Finalize)?;
    let stage_model_identity =
        snapshot_model_path_identity(&completed_path, output.sqlite_size_bytes)?;
    if stage_model_identity.size != output.sqlite_size_bytes {
        return Err(Error::Invalid("native snapshot completed model receipt"));
    }
    capture.verify_inputs(limits.capture)?;
    let expectation = KnowledgeSelectedExpectation {
        model_sha256: output.sqlite_sha256.clone(),
        model_size_bytes: output.sqlite_size_bytes,
        owner_receipt_id: format!(
            "native-snapshot:{}:{}",
            source_revision,
            declaration_sha256.to_hex()
        ),
        model_abi: full.seal.model_abi.clone(),
        managed_source_root_sha256: None,
        descriptor_sha256: vocabulary.descriptor_sha256.clone(),
        descriptor_version: vocabulary.descriptor_version,
        semantic_primitive_profile: vocabulary.semantic_primitive_profile.clone(),
        source_cut: output.source_cut.clone(),
        through_commit_seq: binding.through_commit_seq,
        membership_root: output.membership_root.clone(),
        entity_registry_id: registry.entity_registry_id.clone(),
        entity_registry_version: registry.entity_registry_version.to_string(),
        entity_registry_sha256: registry.entity_sha256.clone(),
        relation_registry_id: registry.relation_registry_id.clone(),
        relation_registry_version: registry.relation_registry_version.to_string(),
        relation_registry_sha256: registry.relation_sha256.clone(),
        graph_root_sha256: full.seal.graph_root_sha256.clone(),
        navigation_original_root_sha256: full.seal.navigation_original_root_sha256.clone(),
        philosophy_original_root_sha256: full.seal.philosophy_original_root_sha256.clone(),
        corpus_original_root_sha256: full.seal.corpus_original_root_sha256.clone(),
        catalog_packet_sha256: full.catalog.catalog_packet_sha256.clone(),
        catalog_index_root_sha256: full.catalog.catalog_index_root_sha256.clone(),
        source_scope_root_sha256: full.source_scope.source_scope_root_sha256.clone(),
        search_index_root_sha256: full.search.search_index_root_sha256.clone(),
        node_count: output.node_rows,
        relation_count: output.relation_rows,
        index_generation: binding.index_generation,
        route_map_version: binding.route_map_version,
        reader_abi: binding.reader_abi,
        authority_boundary: serde_json::to_string(&header["authority_boundary"])
            .map_err(|e| Error::Source(e.to_string()))?,
        source_scopes,
        complete: true,
    };
    let completed = CompletedNativeSnapshot {
        path: completed_path,
        stage_model_identity,
        stage_limits: limits.stage,
        capture_identity,
        stage: output,
        full,
        expectation,
        producer,
        semantic_report: semantics,
        declaration_sha256,
        source_revision,
        descriptor,
        vocabulary,
        entity,
        relation,
    };
    let whole = if let Some((graph, catalog, catalog_inputs)) = whole_parts {
        let retain_state = whole_mode.is_some_and(|(_, _, retain_state)| retain_state);
        let source_state = source_state_before;
        let source_inputs = capture
            .retained_input_members()?
            .into_iter()
            .map(|(path, digest, bytes)| (path, digest.to_hex(), bytes))
            .collect();
        let state = if retain_state {
            Some(issue_state_fd(
                capture,
                &completed.source_revision,
                &graph,
                &catalog,
                &completed.full.seal.graph_root_sha256,
                &completed.full.catalog.catalog_packet_sha256,
                whole_mode
                    .map(|(limits, _, _)| limits.max_state_bytes)
                    .ok_or(Error::Invalid("native snapshot state limit absent"))?,
                whole_mode
                    .map(|(limits, _, _)| limits.json)
                    .ok_or(Error::Invalid("native snapshot state JSON limit absent"))?,
                deadline,
                cancelled,
            )?)
        } else {
            None
        };
        capture.verify_inputs(limits.capture)?;
        if capture.capture_identity()? != capture_identity
            || capture.core_source_state()? != source_state
            || capture.capture_source_state()? != capture_source_state_before
        {
            return Err(Error::Invalid(
                "native snapshot capture changed before whole return",
            ));
        }
        Some(NativeKnowledgeSnapshot {
            graph,
            catalog,
            catalog_inputs,
            source_revision: completed.source_revision.clone(),
            source_state,
            source_inputs,
            graph_root_sha256: completed.full.seal.graph_root_sha256.clone(),
            catalog_sha256: completed.full.catalog.catalog_packet_sha256.clone(),
            state,
        })
    } else {
        capture.verify_inputs(limits.capture)?;
        if capture.capture_identity()? != capture_identity
            || capture.core_source_state()? != source_state_before
            || capture.capture_source_state()? != capture_source_state_before
        {
            return Err(Error::Invalid(
                "native snapshot capture changed before return",
            ));
        }
        None
    };
    Ok((completed, whole))
}
