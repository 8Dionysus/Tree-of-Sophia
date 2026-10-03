//! Bounded physical payload observations for the source-foundation owner.
//!
//! The caller selects a separate `RouteSources` root that mirrors
//! `ToS/source-witnesses/`. Authored-cut membership is queried only for the
//! payload source-inclusion predicates; it never substitutes for an
//! observation of the selected physical payload root.

use sha1::{Digest as ShaDigest, Sha1};
use std::collections::BTreeMap;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tos_foundation::{Digest256Hasher, RelativePath};
use tos_ops_mechanics_plan::route_cards::{RouteSourceReadHooks, RouteSources};
use tos_source_store::CorpusCutReader;
use tos_validation::item_rules::{ItemPayload, ItemRefusal};
use tos_validation::layer_family_cut::CutLayerPayloadReader;
use tos_validation::layer_family_rules::LayerPayload;
use tos_validation::source_cut::CutPayloadReader;
use tos_validation::source_foundation_discovery::PhysicalPayloadFacts;

const SOURCE_ROOT: &str = "ToS/source-witnesses/";
const MAX_PATH_BYTES: usize = 4096;
const TRANSIENT_STATE_BYTES: usize = std::mem::size_of::<Observation>()
    + std::mem::size_of::<PhysicalPayloadFacts>()
    + std::mem::size_of::<Digest256Hasher>()
    + std::mem::size_of::<Sha1>()
    + std::mem::size_of::<JpegDimensions>()
    + 32 * 1024
    + 64
    + 40
    + 8 * std::mem::size_of::<usize>();

/// Limits for one selected physical payload operation. Aggregate bytes include
/// both the initial stream and the final verification stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalPayloadLimits {
    /// Maximum distinct payload paths, including observed absent/non-file paths.
    pub max_files: usize,
    /// Maximum pre-worker snapshot, reader inspect, and final verification calls.
    pub max_observations: usize,
    /// Per-file cap for the initial stream and final verification stream.
    pub max_file_bytes: u64,
    /// Aggregate streamed bytes, including final verification rereads.
    pub max_total_bytes: u64,
    /// Path/fact state plus one fixed streaming workspace and transient fact.
    pub max_state_bytes: usize,
}

/// Physical payload cost while the adapter is retained, and after successful
/// completion. Counters include partial failed-stream reads internally; a
/// failed consuming finish returns no completion or exported cost receipt.
/// Combine successful `total_bytes_read` with the aggregate byte budget used
/// by the selected repository and artifact providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalPayloadCost {
    pub initial_bytes_read: u64,
    pub final_bytes_read: u64,
    pub total_bytes_read: u64,
    /// Actual root payload returns forwarded into the one original IO ledger.
    pub shared_read_bytes_returned: u64,
    pub observation_calls: usize,
    pub snapshot_paths: usize,
    /// Clone returned to the physical provider; that provider also accounts
    /// the same moved map in its own retained-state total.
    pub snapshot_facts_state_bytes: usize,
    /// Separate final facts map returned after the post-worker recheck.
    pub final_facts_state_bytes: usize,
    /// Snapshot clone plus final facts map, reserved simultaneously with the
    /// adapter's retained observations.
    pub snapshot_duplicate_state_bytes: usize,
    pub retained_state_bytes: usize,
    pub peak_state_bytes: usize,
}

/// Post-worker facts and the cost of their verified initial/final streams.
pub struct PhysicalPayloadCompletion {
    pub facts: BTreeMap<String, PhysicalPayloadFacts>,
    pub cost: PhysicalPayloadCost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileStamp {
    #[cfg(target_os = "linux")]
    dev: u64,
    #[cfg(target_os = "linux")]
    ino: u64,
    #[cfg(target_os = "linux")]
    len: u64,
    #[cfg(target_os = "linux")]
    mode: u32,
    #[cfg(target_os = "linux")]
    mtime: i64,
    #[cfg(target_os = "linux")]
    mtime_nsec: i64,
    #[cfg(target_os = "linux")]
    ctime: i64,
    #[cfg(target_os = "linux")]
    ctime_nsec: i64,
    #[cfg(not(target_os = "linux"))]
    marker: (),
}

#[cfg(target_os = "linux")]
impl FileStamp {
    fn from_metadata(metadata: &std::fs::Metadata) -> Self {
        use std::os::unix::fs::MetadataExt;
        Self {
            dev: metadata.dev(),
            ino: metadata.ino(),
            len: metadata.len(),
            mode: metadata.mode(),
            mtime: metadata.mtime(),
            mtime_nsec: metadata.mtime_nsec(),
            ctime: metadata.ctime(),
            ctime_nsec: metadata.ctime_nsec(),
        }
    }
}

#[cfg(not(target_os = "linux"))]
impl FileStamp {
    fn from_metadata(_: &std::fs::Metadata) -> Self {
        Self { marker: () }
    }
}

#[derive(Debug, Clone)]
struct Observation {
    facts: PhysicalPayloadFacts,
    stamp: Option<FileStamp>,
}

/// Streaming marker reader matching `_jpeg_dimensions` in
/// `scripts/validate_source_witness_foundation.py`. It consumes no retained
/// image bytes and deliberately keeps hashing even after dimensions are known.
#[derive(Debug, Clone, Copy)]
enum JpegState {
    First,
    Second,
    MarkerPrefix,
    MarkerCode,
    LengthHigh(u8),
    LengthLow { marker: u8, high: u8 },
    Skip(u16),
    Frame { bytes: [u8; 5], used: usize },
    Done,
}

#[derive(Debug)]
struct JpegDimensions {
    state: JpegState,
    dimensions: Option<(u64, u64)>,
}

impl JpegDimensions {
    fn new() -> Self {
        Self {
            state: JpegState::First,
            dimensions: None,
        }
    }

    fn feed(&mut self, bytes: &[u8]) {
        const SOF: [u8; 13] = [
            0xc0, 0xc1, 0xc2, 0xc3, 0xc5, 0xc6, 0xc7, 0xc9, 0xca, 0xcb, 0xcd, 0xce, 0xcf,
        ];

        for &byte in bytes {
            self.state = match self.state {
                JpegState::First => {
                    if byte == 0xff {
                        JpegState::Second
                    } else {
                        JpegState::Done
                    }
                }
                JpegState::Second => {
                    if byte == 0xd8 {
                        JpegState::MarkerPrefix
                    } else {
                        JpegState::Done
                    }
                }
                JpegState::MarkerPrefix => {
                    if byte == 0xff {
                        JpegState::MarkerCode
                    } else {
                        JpegState::MarkerPrefix
                    }
                }
                JpegState::MarkerCode => {
                    if byte == 0xff {
                        JpegState::MarkerCode
                    } else if byte == 0xd8 || byte == 0xd9 {
                        JpegState::MarkerPrefix
                    } else {
                        JpegState::LengthHigh(byte)
                    }
                }
                JpegState::LengthHigh(marker) => JpegState::LengthLow { marker, high: byte },
                JpegState::LengthLow { marker, high } => {
                    let segment_length = u16::from_be_bytes([high, byte]);
                    if segment_length < 2 {
                        JpegState::Done
                    } else if SOF.contains(&marker) {
                        JpegState::Frame {
                            bytes: [0; 5],
                            used: 0,
                        }
                    } else {
                        let remaining = segment_length - 2;
                        if remaining == 0 {
                            JpegState::MarkerPrefix
                        } else {
                            JpegState::Skip(remaining)
                        }
                    }
                }
                JpegState::Skip(remaining) => {
                    if remaining == 1 {
                        JpegState::MarkerPrefix
                    } else {
                        JpegState::Skip(remaining - 1)
                    }
                }
                JpegState::Frame {
                    mut bytes,
                    mut used,
                } => {
                    bytes[used] = byte;
                    used += 1;
                    if used == bytes.len() {
                        self.dimensions = Some((
                            u16::from_be_bytes([bytes[3], bytes[4]]) as u64,
                            u16::from_be_bytes([bytes[1], bytes[2]]) as u64,
                        ));
                        JpegState::Done
                    } else {
                        JpegState::Frame { bytes, used }
                    }
                }
                JpegState::Done => JpegState::Done,
            };
        }
    }
}

/// One selected physical payload source, shared by Item and layer readers.
/// Pre-worker snapshot facts are provisional: dependent work may inspect them,
/// but its result cannot be released until `finish_with_cost` has reread and
/// rehashed every observed present regular file and rechecked non-file classes.
pub struct FoundationPayloadSources<'a> {
    sources: &'a mut RouteSources,
    membership: PayloadMembership<'a>,
    original_io: Option<&'a tos_source_store::PinnedSqliteIoBudget>,
    shared_read_bytes_returned: u64,
    deadline: Instant,
    cancelled: &'a AtomicBool,
    limits: PhysicalPayloadLimits,
    bytes_read: u64,
    initial_bytes_read: u64,
    final_bytes_read: u64,
    state_bytes: usize,
    snapshot_facts_state_bytes: usize,
    final_facts_state_bytes: usize,
    snapshot_duplicate_state_bytes: usize,
    snapshot_scratch_state_bytes: usize,
    snapshot_paths: usize,
    snapshot_complete: bool,
    final_verification: bool,
    observations: usize,
    observed: BTreeMap<String, Observation>,
}

#[derive(Clone, Copy)]
enum PayloadMembership<'a> {
    Cut(&'a CorpusCutReader),
    Candidate(&'a dyn tos_validation::record_biblio_cut::SourceCutInput),
}

struct PayloadSharedReadHooks<'budget, 'counter> {
    budget: &'budget tos_source_store::PinnedSqliteIoBudget,
    returned: &'counter mut u64,
    deadline: Instant,
    cancelled: &'budget AtomicBool,
}
impl PayloadSharedReadHooks<'_, '_> {
    fn checkpoint(&self) -> io::Result<()> {
        if self.cancelled.load(Ordering::Acquire) {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "payload cancelled",
            ));
        }
        if Instant::now() >= self.deadline {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "payload deadline",
            ));
        }
        Ok(())
    }
}
impl RouteSourceReadHooks for PayloadSharedReadHooks<'_, '_> {
    fn before_read(&mut self, requested_bytes: u64) -> io::Result<()> {
        self.checkpoint()?;
        self.budget
            .charge_read(requested_bytes)
            .map_err(|_| io::Error::other("candidate payload shared read permit refused"))?;
        self.checkpoint()
    }
    fn read_returned(&mut self, actual_bytes: u64) -> io::Result<()> {
        self.budget
            .record_read_returned(actual_bytes)
            .map_err(|_| io::Error::other("candidate payload shared read return refused"))?;
        *self.returned = self
            .returned
            .checked_add(actual_bytes)
            .ok_or_else(|| io::Error::other("candidate payload shared return overflow"))?;
        self.checkpoint()
    }
}

impl<'a> FoundationPayloadSources<'a> {
    pub(crate) fn shared_io_budget_matches(
        &self,
        original_io: &tos_source_store::PinnedSqliteIoBudget,
    ) -> bool {
        self.original_io
            .is_some_and(|io| io.shares_with(original_io))
    }
    pub(crate) fn forwards_payload_reads_to_shared_io(&self) -> bool {
        self.original_io.is_some()
    }
    pub(crate) fn deadline(&self) -> Instant {
        self.deadline
    }
    /// Observe the selected payload namespace's shared held-root counter while
    /// this adapter owns its mutable route borrow.
    pub(crate) fn root_component_open_count(&self) -> usize {
        self.sources.root_component_open_count()
    }
    /// Narrow a persistent reader before another window uses the shared
    /// invocation budget. Already streamed bytes retain their original charge.
    pub(crate) fn restrict_remaining_budget(
        &mut self,
        additional_read_bytes: u64,
        available_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), ItemRefusal> {
        self.checkpoint(deadline, cancelled)?;
        let ceiling = self
            .bytes_read
            .checked_add(additional_read_bytes)
            .ok_or(ItemRefusal::Budget)?;
        if ceiling > self.limits.max_total_bytes
            || available_state_bytes > self.limits.max_state_bytes
        {
            return Err(ItemRefusal::Source(
                "payload source budget cannot be widened".into(),
            ));
        }
        if self.cost().peak_state_bytes > available_state_bytes {
            return Err(ItemRefusal::Budget);
        }
        self.limits.max_total_bytes = ceiling;
        self.limits.max_state_bytes = available_state_bytes;
        Ok(())
    }
    /// Narrow final read headroom without widening or replacing the already
    /// selected state ceiling used by the payload hashing/custody kernel.
    pub(crate) fn restrict_remaining_read_budget(
        &mut self,
        additional_read_bytes: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), ItemRefusal> {
        let remaining = self
            .limits
            .max_total_bytes
            .checked_sub(self.bytes_read)
            .ok_or(ItemRefusal::Budget)?;
        self.restrict_remaining_budget(
            additional_read_bytes.min(remaining),
            self.limits.max_state_bytes,
            deadline,
            cancelled,
        )
    }

    pub fn new(
        sources: &'a mut RouteSources,
        cut: &'a CorpusCutReader,
        limits: PhysicalPayloadLimits,
        deadline: Instant,
        cancelled: &'a AtomicBool,
    ) -> Result<Self, ItemRefusal> {
        Self::new_membership(
            sources,
            PayloadMembership::Cut(cut),
            None,
            limits,
            deadline,
            cancelled,
        )
    }

    pub(crate) fn new_candidate(
        sources: &'a mut RouteSources,
        input: &'a crate::source_admission_candidate_records::CandidateRecordsInput<'_, '_>,
        original_io: &'a tos_source_store::PinnedSqliteIoBudget,
        limits: PhysicalPayloadLimits,
        deadline: Instant,
        cancelled: &'a AtomicBool,
    ) -> Result<Self, ItemRefusal> {
        if sources.deadline() > deadline {
            return Err(ItemRefusal::Source(
                "candidate payload route deadline extends original invocation".into(),
            ));
        }
        if cancelled.load(Ordering::Acquire) {
            return Err(ItemRefusal::Source("payload source cancelled".into()));
        }
        if Instant::now() >= deadline {
            return Err(ItemRefusal::Deadline);
        }
        if !input.shares_io_budget(original_io) {
            input.abandon();
            return Err(ItemRefusal::Source(
                "candidate payload input does not share original IO".into(),
            ));
        }
        let input: &'a dyn tos_validation::record_biblio_cut::SourceCutInput = input;
        Self::new_membership(
            sources,
            PayloadMembership::Candidate(input),
            Some(original_io),
            limits,
            deadline,
            cancelled,
        )
    }

    fn new_membership(
        sources: &'a mut RouteSources,
        membership: PayloadMembership<'a>,
        original_io: Option<&'a tos_source_store::PinnedSqliteIoBudget>,
        limits: PhysicalPayloadLimits,
        deadline: Instant,
        cancelled: &'a AtomicBool,
    ) -> Result<Self, ItemRefusal> {
        let base_state_bytes = std::mem::size_of::<Self>();
        if limits.max_files == 0
            || limits.max_observations == 0
            || base_state_bytes
                .checked_add(TRANSIENT_STATE_BYTES)
                .is_none_or(|minimum| minimum > limits.max_state_bytes)
        {
            return Err(ItemRefusal::Budget);
        }
        Ok(Self {
            sources,
            membership,
            original_io,
            shared_read_bytes_returned: 0,
            deadline,
            cancelled,
            limits,
            bytes_read: 0,
            initial_bytes_read: 0,
            final_bytes_read: 0,
            state_bytes: base_state_bytes,
            snapshot_facts_state_bytes: 0,
            final_facts_state_bytes: 0,
            snapshot_duplicate_state_bytes: 0,
            snapshot_scratch_state_bytes: 0,
            snapshot_paths: 0,
            snapshot_complete: false,
            final_verification: false,
            observations: 0,
            observed: BTreeMap::new(),
        })
    }

    /// Pre-observe the exact selected physical payload paths before dependent
    /// workers begin. This returns cloned facts but does not consume the
    /// adapter; the reader may later inspect only these selected paths. The
    /// returned map's state and the final facts map are both reserved while
    /// the adapter retains its observations.
    pub fn snapshot_facts(
        &mut self,
        selected_paths: &[String],
    ) -> Result<BTreeMap<String, PhysicalPayloadFacts>, ItemRefusal> {
        self.checkpoint(self.deadline, self.cancelled)?;
        if self.snapshot_complete || !self.observed.is_empty() {
            return Err(ItemRefusal::Source(
                "payload snapshot must be the first and only pre-worker read".into(),
            ));
        }
        if selected_paths.len() > self.limits.max_files {
            return Err(ItemRefusal::BudgetCheck {
                check: "physical-payload-files",
                used: Some(selected_paths.len() as u64),
                limit: Some(self.limits.max_files as u64),
            });
        }

        let mut unique = std::collections::BTreeSet::new();
        let mut scratch_bytes = 0usize;
        let mut snapshot_facts_state_bytes = 0usize;
        let mut final_facts_state_bytes = 0usize;
        let mut retained_rows_bytes = 0usize;
        for path in selected_paths {
            self.checkpoint(self.deadline, self.cancelled)?;
            let _ = Self::path_in_payload_root(path)?;
            if !unique.insert(path.as_str()) {
                return Err(ItemRefusal::Unsupported(
                    "duplicate selected physical payload path".into(),
                ));
            }
            scratch_bytes = scratch_bytes
                .checked_add(std::mem::size_of::<&str>() + 32 * std::mem::size_of::<usize>())
                .ok_or(ItemRefusal::Budget)?;
            // One map is returned to the physical snapshot, and another is
            // reserved for post-worker completion. Each row includes a cloned
            // key, bounded SHA-256/SHA-1 strings, fact value, and B-tree node.
            let row_bytes = snapshot_row_state_bytes(path)?;
            retained_rows_bytes = retained_rows_bytes
                .checked_add(row_bytes)
                .ok_or(ItemRefusal::Budget)?;
            snapshot_facts_state_bytes = snapshot_facts_state_bytes
                .checked_add(row_bytes)
                .ok_or(ItemRefusal::Budget)?;
            final_facts_state_bytes = final_facts_state_bytes
                .checked_add(row_bytes)
                .ok_or(ItemRefusal::Budget)?;
            let reserved = self
                .state_bytes
                .checked_add(retained_rows_bytes)
                .and_then(|bytes| bytes.checked_add(snapshot_facts_state_bytes))
                .and_then(|bytes| bytes.checked_add(final_facts_state_bytes))
                .and_then(|bytes| bytes.checked_add(scratch_bytes))
                .and_then(|bytes| bytes.checked_add(TRANSIENT_STATE_BYTES))
                .ok_or(ItemRefusal::Budget)?;
            if reserved > self.limits.max_state_bytes {
                return Err(ItemRefusal::BudgetCheck {
                    check: "physical-payload-state-bytes",
                    used: Some(reserved as u64),
                    limit: Some(self.limits.max_state_bytes as u64),
                });
            }
        }
        let duplicate_state_bytes = snapshot_facts_state_bytes
            .checked_add(final_facts_state_bytes)
            .ok_or(ItemRefusal::Budget)?;
        let reserved = self
            .state_bytes
            .checked_add(retained_rows_bytes)
            .and_then(|bytes| bytes.checked_add(duplicate_state_bytes))
            .and_then(|bytes| bytes.checked_add(scratch_bytes))
            .and_then(|bytes| bytes.checked_add(TRANSIENT_STATE_BYTES))
            .ok_or(ItemRefusal::Budget)?;
        if reserved > self.limits.max_state_bytes {
            return Err(ItemRefusal::BudgetCheck {
                check: "physical-payload-state-bytes",
                used: Some(reserved as u64),
                limit: Some(self.limits.max_state_bytes as u64),
            });
        }

        self.snapshot_duplicate_state_bytes = duplicate_state_bytes;
        self.snapshot_facts_state_bytes = snapshot_facts_state_bytes;
        self.final_facts_state_bytes = final_facts_state_bytes;
        self.snapshot_scratch_state_bytes = scratch_bytes;
        for path in selected_paths {
            self.checkpoint(self.deadline, self.cancelled)?;
            let _ = self.cached_or_observe(
                path,
                self.limits.max_file_bytes,
                self.deadline,
                self.cancelled,
            )?;
        }
        self.snapshot_complete = true;
        self.snapshot_paths = selected_paths.len();

        let mut facts = BTreeMap::new();
        for path in selected_paths {
            self.checkpoint(self.deadline, self.cancelled)?;
            let observation = self.observed.get(path).ok_or_else(|| {
                ItemRefusal::Source("payload snapshot observation missing".into())
            })?;
            facts.insert(path.clone(), observation.facts.clone());
        }
        self.snapshot_scratch_state_bytes = 0;
        Ok(facts)
    }

    pub fn cost(&self) -> PhysicalPayloadCost {
        PhysicalPayloadCost {
            initial_bytes_read: self.initial_bytes_read,
            final_bytes_read: self.final_bytes_read,
            total_bytes_read: self.bytes_read,
            shared_read_bytes_returned: self.shared_read_bytes_returned,
            observation_calls: self.observations,
            snapshot_paths: self.snapshot_paths,
            snapshot_facts_state_bytes: self.snapshot_facts_state_bytes,
            final_facts_state_bytes: self.final_facts_state_bytes,
            snapshot_duplicate_state_bytes: self.snapshot_duplicate_state_bytes,
            retained_state_bytes: self.state_bytes,
            peak_state_bytes: self
                .state_bytes
                .saturating_add(self.snapshot_duplicate_state_bytes)
                .saturating_add(self.snapshot_scratch_state_bytes)
                .saturating_add(TRANSIENT_STATE_BYTES),
        }
    }

    fn checkpoint(&self, deadline: Instant, cancelled: &AtomicBool) -> Result<(), ItemRefusal> {
        if deadline != self.deadline || !std::ptr::eq(cancelled, self.cancelled) {
            return Err(ItemRefusal::Source(
                "payload source operation context differs".into(),
            ));
        }
        if cancelled.load(Ordering::Relaxed) {
            return Err(ItemRefusal::Source("payload source cancelled".into()));
        }
        if Instant::now() >= deadline {
            return Err(ItemRefusal::Deadline);
        }
        self.sources
            .check()
            .map_err(|error| route_error(error, deadline, cancelled))
    }

    fn path_in_payload_root(path: &str) -> Result<&str, ItemRefusal> {
        if path.len() > MAX_PATH_BYTES {
            return Err(ItemRefusal::BudgetCheck {
                check: "physical-payload-path-bytes",
                used: Some(path.len() as u64),
                limit: Some(MAX_PATH_BYTES as u64),
            });
        }
        let _relative = RelativePath::parse(path)
            .map_err(|_| ItemRefusal::Unsupported("physical payload path".into()))?;
        path.strip_prefix(SOURCE_ROOT)
            .filter(|tail| !tail.is_empty())
            .ok_or_else(|| ItemRefusal::Unsupported("payload outside selected source root".into()))
    }

    fn admit_observation(&mut self, path: &str) -> Result<(), ItemRefusal> {
        self.observations = self
            .observations
            .checked_add(1)
            .filter(|used| *used <= self.limits.max_observations)
            .ok_or(ItemRefusal::BudgetCheck {
                check: "physical-payload-observations",
                used: Some(self.observations.saturating_add(1) as u64),
                limit: Some(self.limits.max_observations as u64),
            })?;
        if !self.observed.contains_key(path) && self.observed.len() >= self.limits.max_files {
            return Err(ItemRefusal::BudgetCheck {
                check: "physical-payload-files",
                used: Some(self.observed.len().saturating_add(1) as u64),
                limit: Some(self.limits.max_files as u64),
            });
        }
        if !self.observed.contains_key(path) {
            // Conservatively charge B-tree node links/slots per retained row.
            let map_node_overhead = 32 * std::mem::size_of::<usize>();
            let entry_bytes = path
                .len()
                .checked_add(std::mem::size_of::<String>())
                .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Observation>()))
                .and_then(|bytes| bytes.checked_add(64 + 40))
                .and_then(|bytes| bytes.checked_add(map_node_overhead))
                .ok_or(ItemRefusal::Budget)?;
            let next_state_bytes =
                self.state_bytes
                    .checked_add(entry_bytes)
                    .ok_or(ItemRefusal::BudgetCheck {
                        check: "physical-payload-state-bytes",
                        used: None,
                        limit: Some(self.limits.max_state_bytes as u64),
                    })?;
            let peak_state_bytes = next_state_bytes.checked_add(TRANSIENT_STATE_BYTES).ok_or(
                ItemRefusal::BudgetCheck {
                    check: "physical-payload-state-bytes",
                    used: None,
                    limit: Some(self.limits.max_state_bytes as u64),
                },
            )?;
            let peak_state_bytes = peak_state_bytes
                .checked_add(self.snapshot_duplicate_state_bytes)
                .and_then(|bytes| bytes.checked_add(self.snapshot_scratch_state_bytes))
                .ok_or(ItemRefusal::BudgetCheck {
                    check: "physical-payload-state-bytes",
                    used: None,
                    limit: Some(self.limits.max_state_bytes as u64),
                })?;
            if peak_state_bytes > self.limits.max_state_bytes {
                return Err(ItemRefusal::BudgetCheck {
                    check: "physical-payload-state-bytes",
                    used: Some(peak_state_bytes as u64),
                    limit: Some(self.limits.max_state_bytes as u64),
                });
            }
            self.state_bytes = next_state_bytes;
        }
        Ok(())
    }

    fn observe(
        &mut self,
        path: &str,
        max_file_bytes: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Observation, ItemRefusal> {
        self.checkpoint(deadline, cancelled)?;
        let physical_path = Self::path_in_payload_root(path)?;
        let mut sha256 = Digest256Hasher::new();
        let mut sha1 = Sha1::new();
        let mut jpeg = JpegDimensions::new();
        let mut callback_refusal = None;
        let before_read = self.bytes_read;
        let mut visit = |chunk: &[u8]| {
            if cancelled.load(Ordering::Relaxed) {
                callback_refusal = Some(ItemRefusal::Source("payload source cancelled".into()));
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "payload cancelled",
                ));
            }
            if Instant::now() >= deadline {
                callback_refusal = Some(ItemRefusal::Deadline);
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "payload deadline",
                ));
            }
            sha256.update(chunk);
            sha1.update(chunk);
            jpeg.feed(chunk);
            Ok(())
        };
        let result = match self.original_io {
            Some(budget) => {
                let mut hooks = PayloadSharedReadHooks {
                    budget,
                    returned: &mut self.shared_read_bytes_returned,
                    deadline,
                    cancelled,
                };
                self.sources.stream_regular_with_hooks(
                    physical_path,
                    max_file_bytes.min(self.limits.max_file_bytes),
                    &mut self.bytes_read,
                    self.limits.max_total_bytes,
                    &mut hooks,
                    &mut visit,
                )
            }
            None => self.sources.stream_regular(
                physical_path,
                max_file_bytes.min(self.limits.max_file_bytes),
                &mut self.bytes_read,
                self.limits.max_total_bytes,
                &mut visit,
            ),
        };
        let streamed = self.bytes_read.saturating_sub(before_read);
        if self.final_verification {
            self.final_bytes_read = self
                .final_bytes_read
                .checked_add(streamed)
                .ok_or(ItemRefusal::Budget)?;
        } else {
            self.initial_bytes_read = self
                .initial_bytes_read
                .checked_add(streamed)
                .ok_or(ItemRefusal::Budget)?;
        }
        let metadata = match result {
            Ok(metadata) => metadata,
            Err(error) => {
                if let Some(refusal) = callback_refusal {
                    return Err(refusal);
                }
                return Err(route_error(error, deadline, cancelled));
            }
        };
        self.checkpoint(deadline, cancelled)?;
        match metadata {
            Some(metadata) => {
                if !metadata.is_file() {
                    return Err(ItemRefusal::Source(
                        "physical payload changed type during stream".into(),
                    ));
                }
                let facts = PhysicalPayloadFacts {
                    exists: true,
                    regular_file: true,
                    symlink: false,
                    git_tracked: None,
                    git_ignored: None,
                    byte_size: Some(metadata.len()),
                    sha256: Some(sha256.finalize().to_hex()),
                    sha1: Some(hex_lower(&sha1.finalize())),
                    jpeg_dimensions: jpeg.dimensions,
                };
                Ok(Observation {
                    facts,
                    stamp: Some(FileStamp::from_metadata(&metadata)),
                })
            }
            None => self.classify_non_file(physical_path, deadline, cancelled),
        }
    }

    fn classify_non_file(
        &mut self,
        path: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Observation, ItemRefusal> {
        self.checkpoint(deadline, cancelled)?;
        let facts = match self.sources.is_dir(path) {
            Ok(true) => PhysicalPayloadFacts {
                exists: true,
                regular_file: false,
                symlink: false,
                git_tracked: None,
                git_ignored: None,
                byte_size: None,
                sha256: None,
                sha1: None,
                jpeg_dimensions: None,
            },
            Ok(false) => match self.sources.exists(path) {
                Ok(false) => PhysicalPayloadFacts {
                    exists: false,
                    regular_file: false,
                    symlink: false,
                    git_tracked: None,
                    git_ignored: None,
                    byte_size: None,
                    sha256: None,
                    sha1: None,
                    jpeg_dimensions: None,
                },
                Ok(true) => {
                    return Err(ItemRefusal::Source(
                        "physical payload changed or is not a regular file".into(),
                    ));
                }
                Err(error)
                    if error.raw_os_error() == Some(rustix::io::Errno::LOOP.raw_os_error()) =>
                {
                    PhysicalPayloadFacts {
                        exists: true,
                        regular_file: false,
                        symlink: true,
                        git_tracked: None,
                        git_ignored: None,
                        byte_size: None,
                        sha256: None,
                        sha1: None,
                        jpeg_dimensions: None,
                    }
                }
                Err(error) => return Err(route_error(error, deadline, cancelled)),
            },
            Err(error) if error.raw_os_error() == Some(rustix::io::Errno::LOOP.raw_os_error()) => {
                PhysicalPayloadFacts {
                    exists: true,
                    regular_file: false,
                    symlink: true,
                    git_tracked: None,
                    git_ignored: None,
                    byte_size: None,
                    sha256: None,
                    sha1: None,
                    jpeg_dimensions: None,
                }
            }
            Err(error) => return Err(route_error(error, deadline, cancelled)),
        };
        self.checkpoint(deadline, cancelled)?;
        Ok(Observation { facts, stamp: None })
    }

    fn cached_or_observe<'s>(
        &'s mut self,
        path: &str,
        max_file_bytes: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<&'s Observation, ItemRefusal> {
        self.checkpoint(deadline, cancelled)?;
        let _ = Self::path_in_payload_root(path)?;
        if self.snapshot_complete && !self.observed.contains_key(path) {
            return Err(ItemRefusal::Unsupported(
                "physical payload path was not in the pre-worker snapshot".into(),
            ));
        }
        self.admit_observation(path)?;
        if self.observed.contains_key(path) {
            let observation = self.observed.get(path).ok_or_else(|| {
                ItemRefusal::Source("payload cached observation unavailable".into())
            })?;
            if observation
                .facts
                .byte_size
                .is_some_and(|size| size > max_file_bytes.min(self.limits.max_file_bytes))
            {
                return Err(ItemRefusal::BudgetCheck {
                    check: "physical-payload-file-bytes",
                    used: observation.facts.byte_size,
                    limit: Some(max_file_bytes.min(self.limits.max_file_bytes)),
                });
            }
            return Ok(observation);
        }
        let observation = self.observe(path, max_file_bytes, deadline, cancelled)?;
        self.observed.insert(path.to_owned(), observation);
        self.observed
            .get(path)
            .ok_or_else(|| ItemRefusal::Source("payload observation insertion failed".into()))
    }

    fn source_member(&self, path: &str) -> Result<bool, ItemRefusal> {
        let relative = RelativePath::parse(path)
            .map_err(|_| ItemRefusal::Unsupported("physical payload source membership".into()))?;
        match self.membership {
            PayloadMembership::Cut(cut) => Ok(cut.current().member(&relative).is_some()),
            PayloadMembership::Candidate(input) => Ok(matches!(
                input.path_presence(path, self.deadline, self.cancelled)?,
                Some(tos_source_store::SourcePresenceV1::File)
            )),
        }
    }

    /// Revalidate all observed physical facts and return them for the
    /// discovery owner. Prefer `finish_with_cost` when aggregate accounting is
    /// required; this compatibility wrapper preserves the facts-only result.
    pub fn finish(self) -> Result<BTreeMap<String, PhysicalPayloadFacts>, ItemRefusal> {
        self.finish_with_cost().map(|completion| completion.facts)
    }

    /// Final post-worker hash/stamp fence, with explicit aggregate cost.
    pub fn finish_with_cost(mut self) -> Result<PhysicalPayloadCompletion, ItemRefusal> {
        let deadline = self.deadline;
        let cancelled = self.cancelled;
        if !self.snapshot_complete {
            return Err(ItemRefusal::Source(
                "physical payload facts were not snapshotted before workers".into(),
            ));
        }
        self.checkpoint(deadline, cancelled)?;
        self.sources
            .verify_root()
            .map_err(|error| route_error(error, deadline, cancelled))?;
        self.final_verification = true;
        // Iterate the selected operands once. Reinserting into the same map
        // being popped would repeatedly select its first key until refusal.
        let selected = std::mem::take(&mut self.observed);
        for (path, original) in selected {
            self.checkpoint(deadline, cancelled)?;
            self.observations = self
                .observations
                .checked_add(1)
                .filter(|used| *used <= self.limits.max_observations)
                .ok_or(ItemRefusal::BudgetCheck {
                    check: "physical-payload-observations",
                    used: Some(self.observations.saturating_add(1) as u64),
                    limit: Some(self.limits.max_observations as u64),
                })?;
            let current = self.observe(&path, self.limits.max_file_bytes, deadline, cancelled)?;
            if current.facts != original.facts || current.stamp != original.stamp {
                return Err(ItemRefusal::Source(
                    "physical payload changed after observation".into(),
                ));
            }
            self.observed.insert(path, original);
        }
        self.sources
            .verify_root()
            .map_err(|error| route_error(error, deadline, cancelled))?;
        self.checkpoint(deadline, cancelled)?;
        let peak_state_bytes = self
            .state_bytes
            .checked_add(self.snapshot_duplicate_state_bytes)
            .and_then(|bytes| bytes.checked_add(TRANSIENT_STATE_BYTES))
            .ok_or(ItemRefusal::Budget)?;
        if peak_state_bytes > self.limits.max_state_bytes {
            return Err(ItemRefusal::BudgetCheck {
                check: "physical-payload-state-bytes",
                used: Some(peak_state_bytes as u64),
                limit: Some(self.limits.max_state_bytes as u64),
            });
        }
        let cost = PhysicalPayloadCost {
            initial_bytes_read: self.initial_bytes_read,
            final_bytes_read: self.final_bytes_read,
            total_bytes_read: self.bytes_read,
            shared_read_bytes_returned: self.shared_read_bytes_returned,
            observation_calls: self.observations,
            snapshot_paths: self.snapshot_paths,
            snapshot_facts_state_bytes: self.snapshot_facts_state_bytes,
            final_facts_state_bytes: self.final_facts_state_bytes,
            snapshot_duplicate_state_bytes: self.snapshot_duplicate_state_bytes,
            retained_state_bytes: self.state_bytes,
            peak_state_bytes,
        };
        let facts = self
            .observed
            .into_iter()
            .map(|(path, observation)| (path, observation.facts))
            .collect();
        Ok(PhysicalPayloadCompletion { facts, cost })
    }
}

impl CutPayloadReader for FoundationPayloadSources<'_> {
    fn inspect(
        &mut self,
        path: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<ItemPayload, ItemRefusal> {
        let facts = &self
            .cached_or_observe(path, self.limits.max_file_bytes, deadline, cancelled)?
            .facts;
        if !facts.exists || !facts.regular_file || facts.symlink {
            return Ok(ItemPayload::Unavailable);
        }
        Ok(ItemPayload::File {
            byte_size: facts
                .byte_size
                .ok_or_else(|| ItemRefusal::Source("payload byte size unavailable".into()))?,
            sha256: facts
                .sha256
                .as_ref()
                .cloned()
                .ok_or_else(|| ItemRefusal::Source("payload SHA-256 unavailable".into()))?,
            excluded_from_source: !self.source_member(path)?,
        })
    }
}

impl CutLayerPayloadReader for FoundationPayloadSources<'_> {
    fn inspect(
        &mut self,
        path: &str,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<LayerPayload, ItemRefusal> {
        let requested = u64::try_from(max_bytes).unwrap_or(u64::MAX);
        let source_member = self.source_member(path)?;
        let facts = &self
            .cached_or_observe(path, requested, deadline, cancelled)?
            .facts;
        if !facts.exists || !facts.regular_file || facts.symlink {
            return Ok(LayerPayload::Unavailable);
        }
        Ok(LayerPayload::File {
            byte_size: facts
                .byte_size
                .ok_or_else(|| ItemRefusal::Source("payload byte size unavailable".into()))?,
            sha256: facts
                .sha256
                .as_ref()
                .cloned()
                .ok_or_else(|| ItemRefusal::Source("payload SHA-256 unavailable".into()))?,
            source_member,
            sha1: facts.sha1.as_ref().cloned(),
            jpeg_dimensions: facts.jpeg_dimensions,
        })
    }
}

fn route_error(error: io::Error, deadline: Instant, cancelled: &AtomicBool) -> ItemRefusal {
    if cancelled.load(Ordering::Relaxed) {
        return ItemRefusal::Source("payload source cancelled".into());
    }
    if Instant::now() >= deadline || error.kind() == io::ErrorKind::TimedOut {
        return ItemRefusal::Deadline;
    }
    if error.kind() == io::ErrorKind::Unsupported {
        return ItemRefusal::Unsupported("physical payload reader unsupported".into());
    }
    if error.kind() == io::ErrorKind::InvalidData {
        match error.to_string().as_str() {
            "physical operand byte bound exceeded"
            | "physical operand aggregate accounting exceeded" => {
                return ItemRefusal::Budget;
            }
            "route lookup operation bound exceeded" => return ItemRefusal::Budget,
            "route operation deadline exceeded" => return ItemRefusal::Deadline,
            _ => (),
        }
    }
    ItemRefusal::Source(format!("physical payload custody: {error}"))
}

fn snapshot_row_state_bytes(path: &str) -> Result<usize, ItemRefusal> {
    path.len()
        .checked_add(std::mem::size_of::<String>())
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<PhysicalPayloadFacts>()))
        .and_then(|bytes| bytes.checked_add(64 + 40))
        .and_then(|bytes| bytes.checked_add(32 * std::mem::size_of::<usize>()))
        .ok_or(ItemRefusal::Budget)
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}
