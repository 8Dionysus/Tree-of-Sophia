//! Bounded physical/Git observations for the source-foundation owner.
//!
//! Build this snapshot before schema/stage workers, lend `facts()` only while
//! they run, then call the matching recheck method after every worker has
//! ended. Captured authored bytes are rechecked by `FoundationCapturedCut`;
//! borrowed candidate bytes retain their own `SourceCutInputCoverage` fence
//! and stay independent of working-tree physical facts. Payload bytes are
//! owned by the separate payload adapter and must be reverified after workers.
//! An absent artifact provider means unknown artifact facts, never an observed
//! absence.

use super::foundation_capture::FoundationCapturedCut;
use crate::source_admission_candidate_records::CandidateRecordsInput;
use crate::source_admission_spooled_candidate::CandidateFence;
use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Component, Path};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::time::{Duration, Instant};
use tos_foundation::{Digest256, RelativePath};
use tos_ops_mechanics_plan::route_cards::{
    RouteResolvedTarget, RouteSourceReadHooks, RouteSources,
};
use tos_source_store::PinnedSqliteIoBudget;
use tos_validation::item_rules::ItemRefusal;
use tos_validation::record_biblio_cut::{SourceCutInputCoverage, SourceCutInputWithIdentity};
use tos_validation::source_foundation_discovery::{
    GitPathFacts, PhysicalPathFacts, PhysicalPayloadFacts, PhysicalResolvedTargetFacts,
    SourcePhysicalFacts,
};

const MAX_PATH_BYTES: usize = 4096;
const MAX_PATH_COMPONENTS: usize = 128;
const RESOLVER_WORKSPACE_BYTES: usize = 64 * 1024;
const MAP_NODE_OVERHEAD: usize = 32 * std::mem::size_of::<usize>();

/// Explicit bounds for one selected-repository physical snapshot. Byte totals
/// cover authored-independent private and artifact reads, including their
/// final verification reads; captured authored bytes are already accounted for
/// by `FoundationCapturedCut`, and mirrored payload bytes by its adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalSourceLimits {
    /// Distinct exact paths across the ToS root and the selected artifact root.
    pub max_paths: usize,
    /// Exact private inventory prefixes; overlapping prefixes are rejected.
    pub max_private_prefixes: usize,
    /// Descendant files and directories across all private-prefix walks, per pass.
    pub max_inventory_paths: usize,
    /// Logical physical-path observations across the initial and final passes.
    pub max_path_observations: usize,
    /// Exact Git helper calls across the initial and final passes.
    pub max_git_path_queries: usize,
    /// Per-file bound for explicit private and artifact file reads.
    pub max_file_bytes: usize,
    /// Aggregate private and artifact bytes, including final verification reads.
    pub max_total_bytes: usize,
    /// Retained facts, cloned path keys, and bounded transient workspace.
    pub max_state_bytes: usize,
    /// Output bound passed to the maintained Git custody helper.
    pub git_output_bytes: usize,
    /// Reserved child cleanup time passed to the maintained Git custody helper.
    pub git_cleanup_grace: Duration,
}

/// Caller-visible work charged to the physical-source observation. Root-open
/// counters are cumulative `RouteSources` values sampled at this adapter's
/// entry, after the initial snapshot, and after final verification; they
/// include the held-root custody opens as well as later root rechecks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalSourceCost {
    pub bytes_read: usize,
    /// Payload bytes returned by physical reads already forwarded to the
    /// retained shared candidate ledger. The outer candidate debit excludes
    /// only this amount after matching that exact ledger identity.
    pub shared_read_bytes_returned: usize,
    pub path_observations: usize,
    pub git_path_queries: usize,
    pub retained_state_bytes: usize,
    pub source_root_component_open_count_initial: usize,
    pub source_root_component_open_count_after_observation: usize,
    pub source_root_component_open_count_final: Option<usize>,
    pub artifact_root_component_open_count_initial: Option<usize>,
    pub artifact_root_component_open_count_after_observation: Option<usize>,
    pub artifact_root_component_open_count_final: Option<usize>,
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
    nlink: u64,
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
    fn from_metadata(metadata: &fs::Metadata) -> Self {
        use std::os::unix::fs::MetadataExt;
        Self {
            dev: metadata.dev(),
            ino: metadata.ino(),
            len: metadata.len(),
            mode: metadata.mode(),
            nlink: metadata.nlink(),
            mtime: metadata.mtime(),
            mtime_nsec: metadata.mtime_nsec(),
            ctime: metadata.ctime(),
            ctime_nsec: metadata.ctime_nsec(),
        }
    }
}

#[cfg(not(target_os = "linux"))]
impl FileStamp {
    fn from_metadata(_: &fs::Metadata) -> Self {
        Self { marker: () }
    }
}

#[derive(Debug, Clone)]
struct PathObservation {
    facts: PhysicalPathFacts,
    stamp: Option<FileStamp>,
    resolved_stamp: Option<FileStamp>,
}

struct SharedPhysicalReadHooks<'budget, 'returned, 'cancel, 'signal> {
    budget: &'budget PinnedSqliteIoBudget,
    returned_bytes: &'returned mut usize,
    deadline: Instant,
    cancelled: &'cancel AtomicBool,
    git_signal: &'signal AtomicI32,
}

impl RouteSourceReadHooks for SharedPhysicalReadHooks<'_, '_, '_, '_> {
    fn before_read(&mut self, requested_bytes: u64) -> io::Result<()> {
        physical_read_checkpoint(self.deadline, self.cancelled, self.git_signal)?;
        self.budget.charge_read(requested_bytes).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("shared physical read budget refused: {error}"),
            )
        })?;
        physical_read_checkpoint(self.deadline, self.cancelled, self.git_signal)
    }

    fn read_returned(&mut self, actual_bytes: u64) -> io::Result<()> {
        self.budget
            .record_read_returned(actual_bytes)
            .map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("shared physical read ledger rejected returned bytes: {error}"),
                )
            })?;
        let actual_bytes = usize::try_from(actual_bytes).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "shared physical returned byte accounting overflow",
            )
        })?;
        *self.returned_bytes = self
            .returned_bytes
            .checked_add(actual_bytes)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "shared physical returned byte accounting overflow",
                )
            })?;
        physical_read_checkpoint(self.deadline, self.cancelled, self.git_signal)
    }
}

#[derive(Debug)]
struct ResolvedObservation {
    facts: PhysicalResolvedTargetFacts,
    metadata: Option<fs::Metadata>,
    stamp: Option<FileStamp>,
}

/// Physical/Git facts plus the operation context needed to recheck them after
/// dependent diagnostic workers complete. The selected artifact root is an
/// optional, separately held `RouteSources`; no ambient root is discovered.
pub struct FoundationPhysicalSnapshot<'cancel, 'signal> {
    facts: SourcePhysicalFacts,
    historical_originals: BTreeMap<String, PhysicalPathFacts>,
    repo_paths: BTreeSet<String>,
    authored_capture_paths: BTreeSet<String>,
    candidate_member_paths: BTreeSet<String>,
    private_declared_paths: BTreeSet<String>,
    private_prefixes: Vec<String>,
    git_paths: BTreeSet<String>,
    private_stamps: BTreeMap<String, FileStamp>,
    private_resolved_stamps: BTreeMap<String, FileStamp>,
    authored_stamps: BTreeMap<String, FileStamp>,
    authored_resolved_stamps: BTreeMap<String, FileStamp>,
    resolved_source_directories: BTreeMap<String, String>,
    artifact_requested_paths: BTreeSet<String>,
    artifact_stamps: BTreeMap<String, FileStamp>,
    artifact_resolved_stamps: BTreeMap<String, FileStamp>,
    artifact_root_stamp: Option<FileStamp>,
    artifact_provider_selected: bool,
    git_root_present: bool,
    inventory_entry_counts: BTreeMap<String, usize>,
    inventory_entries_total: usize,
    limits: PhysicalSourceLimits,
    deadline: Instant,
    cancelled: &'cancel AtomicBool,
    git_signal: &'signal AtomicI32,
    candidate_identity: Option<CandidateFence>,
    candidate_io_budget: Option<PinnedSqliteIoBudget>,
    bytes_read: usize,
    shared_read_bytes_returned: usize,
    path_observations: usize,
    git_path_queries: usize,
    state_bytes: usize,
    source_root_component_open_count_initial: usize,
    source_root_component_open_count_after_observation: usize,
    source_root_component_open_count_final: Option<usize>,
    artifact_root_component_open_count_initial: Option<usize>,
    artifact_root_component_open_count_after_observation: Option<usize>,
    artifact_root_component_open_count_final: Option<usize>,
}

impl<'cancel, 'signal> FoundationPhysicalSnapshot<'cancel, 'signal> {
    pub(crate) fn deadline(&self) -> Instant {
        self.deadline
    }
    /// Preserve earlier observations while reducing the allowance available
    /// to a later recheck after other invocation windows have spent resources.
    pub(crate) fn restrict_remaining_budget(
        &mut self,
        additional_read_bytes: usize,
        available_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), ItemRefusal> {
        if deadline != self.deadline || !std::ptr::eq(cancelled, self.cancelled) {
            return Err(ItemRefusal::Source(
                "physical source operation context differs".into(),
            ));
        }
        if cancelled.load(Ordering::Relaxed)
            || self.git_signal.load(Ordering::Relaxed) != 0
            || Instant::now() >= deadline
        {
            return Err(check_context(deadline, cancelled, self.git_signal));
        }
        let ceiling = self
            .bytes_read
            .checked_add(additional_read_bytes)
            .ok_or(ItemRefusal::Budget)?;
        if ceiling > self.limits.max_total_bytes
            || available_state_bytes > self.limits.max_state_bytes
        {
            return Err(ItemRefusal::Source(
                "physical source budget cannot be widened".into(),
            ));
        }
        if self.state_bytes > available_state_bytes {
            return Err(ItemRefusal::Budget);
        }
        self.limits.max_total_bytes = ceiling;
        self.limits.max_state_bytes = available_state_bytes;
        Ok(())
    }
    /// Narrow only final read headroom; keep the already selected state
    /// ceiling so the existing fixed hashing workspace remains available.
    pub(crate) fn restrict_remaining_read_budget(
        &mut self,
        additional_read_bytes: usize,
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
    /// Observe captured authored metadata, explicit private paths/prefixes,
    /// payload Git posture, and caller-selected artifact paths. All Git facts
    /// come from the actual selected ToS repository's maintained read-only Git
    /// helper; source-cut membership is never treated as Git evidence.
    #[allow(clippy::too_many_arguments)]
    pub fn observe(
        sources: &mut RouteSources,
        captured: &FoundationCapturedCut,
        authored_paths: &[String],
        payloads: BTreeMap<String, PhysicalPayloadFacts>,
        private_paths: &[String],
        private_prefixes: &[String],
        mut artifact_sources: Option<&mut RouteSources>,
        artifact_paths: &[String],
        limits: PhysicalSourceLimits,
        deadline: Instant,
        cancelled: &'cancel AtomicBool,
        git_signal: &'signal AtomicI32,
    ) -> Result<Self, ItemRefusal> {
        Self::observe_with_resolved_targets(
            sources,
            captured,
            authored_paths,
            payloads,
            private_paths,
            private_prefixes,
            artifact_sources,
            artifact_paths,
            &BTreeMap::new(),
            limits,
            deadline,
            cancelled,
            git_signal,
        )
    }

    /// Observe physical targets through caller-selected, explicit directory
    /// capabilities. Each ToS key maps to one selected repository-relative
    /// directory; the adapter never widens that capability to a common root.
    /// Artifact paths resolve under their separately selected held root.
    #[allow(clippy::too_many_arguments)]
    pub fn observe_with_resolved_targets(
        sources: &mut RouteSources,
        captured: &FoundationCapturedCut,
        authored_paths: &[String],
        payloads: BTreeMap<String, PhysicalPayloadFacts>,
        private_paths: &[String],
        private_prefixes: &[String],
        mut artifact_sources: Option<&mut RouteSources>,
        artifact_paths: &[String],
        resolved_source_directories: &BTreeMap<String, String>,
        limits: PhysicalSourceLimits,
        deadline: Instant,
        cancelled: &'cancel AtomicBool,
        git_signal: &'signal AtomicI32,
    ) -> Result<Self, ItemRefusal> {
        Self::observe_kernel(
            sources,
            Some(captured),
            BTreeSet::new(),
            None,
            authored_paths,
            payloads,
            private_paths,
            private_prefixes,
            artifact_sources,
            artifact_paths,
            resolved_source_directories,
            limits,
            deadline,
            cancelled,
            git_signal,
        )
    }

    /// Observe selected physical targets for the concrete borrowed candidate
    /// adapter. Before any candidate or root read, require its wrapped spool
    /// to share the caller's original ledger. The candidate fence authenticates
    /// its own source stream; physical facts remain independent working-tree
    /// observations and need not equal candidate bytes.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn observe_candidate_with_resolved_targets(
        sources: &mut RouteSources,
        candidate: &CandidateRecordsInput<'_, '_>,
        coverage: &SourceCutInputCoverage,
        original_io: &PinnedSqliteIoBudget,
        authored_paths: &[String],
        payloads: BTreeMap<String, PhysicalPayloadFacts>,
        private_paths: &[String],
        private_prefixes: &[String],
        artifact_sources: Option<&mut RouteSources>,
        artifact_paths: &[String],
        resolved_source_directories: &BTreeMap<String, String>,
        limits: PhysicalSourceLimits,
        deadline: Instant,
        cancelled: &'cancel AtomicBool,
        git_signal: &'signal AtomicI32,
    ) -> Result<Self, ItemRefusal> {
        if !candidate.shares_io_budget(original_io) {
            candidate.abandon();
            return Err(ItemRefusal::Source(
                "candidate physical input does not share the original IO ledger".into(),
            ));
        }
        validate_limits(limits)?;
        candidate
            .source_input()
            .verify_current_fence(coverage, deadline, cancelled)?;
        let identity = *candidate.input_identity();
        let mut candidate_member_paths = BTreeSet::new();
        let mut candidate_member_state = 0usize;
        for path in authored_paths {
            if candidate
                .source_input()
                .path_presence(path, deadline, cancelled)?
                == Some(tos_source_store::SourcePresenceV1::File)
            {
                charge_path_value(path, &mut candidate_member_state, limits.max_state_bytes)?;
                candidate_member_paths.insert(path.clone());
            }
        }
        candidate
            .source_input()
            .verify_current_fence(coverage, deadline, cancelled)?;
        let mut snapshot = Self::observe_kernel(
            sources,
            None,
            candidate_member_paths,
            Some(original_io),
            authored_paths,
            payloads,
            private_paths,
            private_prefixes,
            artifact_sources,
            artifact_paths,
            resolved_source_directories,
            limits,
            deadline,
            cancelled,
            git_signal,
        )?;
        candidate
            .source_input()
            .verify_current_fence(coverage, deadline, cancelled)?;
        snapshot.candidate_identity = Some(identity);
        Ok(snapshot)
    }

    #[allow(clippy::too_many_arguments)]
    fn observe_kernel(
        sources: &mut RouteSources,
        captured: Option<&FoundationCapturedCut>,
        candidate_member_paths: BTreeSet<String>,
        candidate_io_budget: Option<&PinnedSqliteIoBudget>,
        authored_paths: &[String],
        payloads: BTreeMap<String, PhysicalPayloadFacts>,
        private_paths: &[String],
        private_prefixes: &[String],
        mut artifact_sources: Option<&mut RouteSources>,
        artifact_paths: &[String],
        resolved_source_directories: &BTreeMap<String, String>,
        limits: PhysicalSourceLimits,
        deadline: Instant,
        cancelled: &'cancel AtomicBool,
        git_signal: &'signal AtomicI32,
    ) -> Result<Self, ItemRefusal> {
        let source_root_component_open_count_initial = sources.root_component_open_count();
        let artifact_root_component_open_count_initial = artifact_sources
            .as_deref()
            .map(RouteSources::root_component_open_count);
        validate_limits(limits)?;
        checkpoint(sources, deadline, cancelled, git_signal)?;
        sources
            .verify_root()
            .map_err(|error| route_error(error, deadline, cancelled, git_signal))?;

        let artifact_requested_paths = collect_artifact_paths(artifact_paths, limits)?;
        let artifact_count = artifact_requested_paths.len();
        let mut state_bytes = std::mem::size_of::<SourcePhysicalFacts>()
            .checked_add(std::mem::size_of::<
                FoundationPhysicalSnapshot<'cancel, 'signal>,
            >())
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<PhysicalSourceCost>()))
            .ok_or(ItemRefusal::Budget)?;
        for path in &candidate_member_paths {
            charge_path_value(path, &mut state_bytes, limits.max_state_bytes)?;
        }
        if resolved_source_directories.len() > limits.max_paths {
            return Err(budget(
                "physical-source-resolution-roots",
                resolved_source_directories.len(),
                limits.max_paths,
            ));
        }
        let mut owned_resolved_source_directories = BTreeMap::new();
        for (path, directory) in resolved_source_directories {
            validate_repo_path(path)?;
            validate_repo_path(directory)?;
            if !path_within(path, directory) {
                return Err(ItemRefusal::Unsupported(
                    "physical resolution path outside selected directory".into(),
                ));
            }
            charge_path_value(path, &mut state_bytes, limits.max_state_bytes)?;
            charge_path_value(directory, &mut state_bytes, limits.max_state_bytes)?;
            owned_resolved_source_directories.insert(path.clone(), directory.clone());
        }
        for path in &artifact_requested_paths {
            charge_path_value(path, &mut state_bytes, limits.max_state_bytes)?;
        }
        charge_payload_map(&payloads, &mut state_bytes, limits.max_state_bytes)?;
        let mut repo_paths = BTreeSet::new();
        let mut git_paths = BTreeSet::new();
        let mut private_declared_paths = BTreeSet::new();
        let mut authored_capture_paths = BTreeSet::new();
        let mut prefixes = Vec::new();
        let mut facts = SourcePhysicalFacts {
            git_available: None,
            payloads,
            authored_git: BTreeMap::new(),
            authored_paths: BTreeMap::new(),
            artifact_paths: BTreeMap::new(),
            private_files: None,
            private_inventories: BTreeMap::new(),
            private_paths: BTreeMap::new(),
        };

        let mut bytes_read = 0usize;
        let mut shared_read_bytes_returned = 0usize;
        let mut path_observations = 0usize;
        let mut authored_stamps = BTreeMap::new();
        let mut authored_resolved_stamps = BTreeMap::new();
        let mut private_stamps = BTreeMap::new();
        let mut private_resolved_stamps = BTreeMap::new();
        let mut authored_requested = BTreeSet::new();
        for path in authored_paths {
            validate_repo_path(path)?;
            insert_path(
                &mut repo_paths,
                path,
                artifact_count,
                &mut state_bytes,
                limits,
            )?;
            insert_path(
                &mut git_paths,
                path,
                artifact_count,
                &mut state_bytes,
                limits,
            )?;
            if authored_requested.insert(path.clone()) {
                charge_path_value(path, &mut state_bytes, limits.max_state_bytes)?;
                charge_map_entry(
                    path,
                    std::mem::size_of::<GitPathFacts>(),
                    0,
                    &mut state_bytes,
                    limits.max_state_bytes,
                )?;
                facts.authored_git.insert(
                    path.clone(),
                    GitPathFacts {
                        tracked: None,
                        ignored: None,
                    },
                );
            }
        }
        if let Some(captured) = captured {
            for (path, member) in captured.observed_members() {
                if !authored_requested.contains(path) {
                    continue;
                }
                checkpoint(sources, deadline, cancelled, git_signal)?;
                let selected_directory = owned_resolved_source_directories
                    .get(path)
                    .map(String::as_str);
                let resolved_observation = if let Some(selected_directory) = selected_directory {
                    Some(observe_path(
                        sources,
                        path,
                        false,
                        Some(selected_directory),
                        true,
                        &mut bytes_read,
                        &mut path_observations,
                        &mut state_bytes,
                        limits,
                        deadline,
                        cancelled,
                        git_signal,
                    )?)
                } else {
                    charge_path_observation(&mut path_observations, limits.max_path_observations)?;
                    None
                };
                let metadata = sources
                    .metadata(path)
                    .map_err(|error| route_error(error, deadline, cancelled, git_signal))?
                    .ok_or_else(|| {
                        ItemRefusal::Source("captured authored path disappeared".into())
                    })?;
                if !metadata.is_file()
                    || metadata.len() != member.size_bytes
                    || file_mode(&metadata) != Some(member.mode)
                {
                    return Err(ItemRefusal::Source(
                        "captured authored physical metadata changed".into(),
                    ));
                }
                let stamp = FileStamp::from_metadata(&metadata);
                if let Some(observation) = resolved_observation.as_ref() {
                    if !observation.facts.regular_file
                        || observation.facts.directory
                        || observation.facts.symlink
                        || observation.stamp != Some(stamp)
                    {
                        return Err(ItemRefusal::Source(
                            "captured authored resolved path changed type".into(),
                        ));
                    }
                    if let Some(resolved_stamp) = observation.resolved_stamp {
                        charge_map_entry(
                            path,
                            std::mem::size_of::<FileStamp>(),
                            0,
                            &mut state_bytes,
                            limits.max_state_bytes,
                        )?;
                        authored_resolved_stamps.insert(path.to_owned(), resolved_stamp);
                    }
                }
                if authored_capture_paths.insert(path.to_owned()) {
                    charge_path_value(path, &mut state_bytes, limits.max_state_bytes)?;
                }
                charge_map_entry(
                    path,
                    std::mem::size_of::<PhysicalPathFacts>(),
                    64usize
                        .checked_add(
                            resolved_observation
                                .as_ref()
                                .map(|observation| physical_dynamic_bytes(&observation.facts))
                                .transpose()?
                                .unwrap_or_default(),
                        )
                        .ok_or(ItemRefusal::Budget)?,
                    &mut state_bytes,
                    limits.max_state_bytes,
                )?;
                facts.authored_paths.insert(
                    path.to_owned(),
                    PhysicalPathFacts {
                        exists: true,
                        regular_file: true,
                        directory: false,
                        symlink: false,
                        resolved_target: resolved_observation
                            .and_then(|observation| observation.facts.resolved_target),
                        git_tracked: None,
                        git_ignored: None,
                        file_mode: Some(member.mode),
                        byte_size: Some(member.size_bytes),
                        sha256: Some(member.sha256.to_hex()),
                    },
                );
                charge_map_entry(
                    path,
                    std::mem::size_of::<FileStamp>(),
                    0,
                    &mut state_bytes,
                    limits.max_state_bytes,
                )?;
                authored_stamps.insert(path.to_owned(), stamp);
            }
        }

        for path in &authored_requested {
            if authored_capture_paths.contains(path) {
                continue;
            }
            let selected_directory = owned_resolved_source_directories
                .get(path)
                .map(String::as_str);
            let observation = observe_path_with_shared(
                sources,
                path,
                true,
                selected_directory,
                selected_directory.is_some(),
                &mut bytes_read,
                &mut path_observations,
                &mut state_bytes,
                limits,
                deadline,
                cancelled,
                git_signal,
                candidate_io_budget,
                &mut shared_read_bytes_returned,
            )?;
            if let Some(stamp) = observation.stamp {
                charge_map_entry(
                    path,
                    std::mem::size_of::<FileStamp>(),
                    0,
                    &mut state_bytes,
                    limits.max_state_bytes,
                )?;
                authored_stamps.insert(path.clone(), stamp);
            }
            if let Some(stamp) = observation.resolved_stamp {
                charge_map_entry(
                    path,
                    std::mem::size_of::<FileStamp>(),
                    0,
                    &mut state_bytes,
                    limits.max_state_bytes,
                )?;
                authored_resolved_stamps.insert(path.clone(), stamp);
            }
            charge_map_entry(
                path,
                std::mem::size_of::<PhysicalPathFacts>(),
                physical_dynamic_bytes(&observation.facts)?,
                &mut state_bytes,
                limits.max_state_bytes,
            )?;
            facts.authored_paths.insert(path.clone(), observation.facts);
        }

        for path in facts.payloads.keys() {
            validate_repo_path(path)?;
            insert_path(
                &mut repo_paths,
                path,
                artifact_count,
                &mut state_bytes,
                limits,
            )?;
            insert_path(
                &mut git_paths,
                path,
                artifact_count,
                &mut state_bytes,
                limits,
            )?;
        }

        for path in private_paths {
            validate_repo_path(path)?;
            insert_path(
                &mut repo_paths,
                path,
                artifact_count,
                &mut state_bytes,
                limits,
            )?;
            insert_path(
                &mut git_paths,
                path,
                artifact_count,
                &mut state_bytes,
                limits,
            )?;
            if private_declared_paths.insert(path.clone()) {
                charge_path_value(path, &mut state_bytes, limits.max_state_bytes)?;
            }
        }
        if owned_resolved_source_directories.keys().any(|path| {
            !authored_requested.contains(path) && !private_declared_paths.contains(path)
        }) {
            return Err(ItemRefusal::Unsupported(
                "physical resolution requested for an unselected source path".into(),
            ));
        }
        for path in &private_declared_paths {
            let inventory_directory = private_prefixes
                .iter()
                .find(|prefix| path_within(path, prefix))
                .map(String::as_str);
            let requested_directory = owned_resolved_source_directories
                .get(path)
                .map(String::as_str);
            if requested_directory.is_some()
                && inventory_directory.is_some()
                && requested_directory != inventory_directory
            {
                return Err(ItemRefusal::Unsupported(
                    "private path resolution scope differs from its selected inventory".into(),
                ));
            }
            let selected_directory = requested_directory.or(inventory_directory);
            let observation = observe_path_with_shared(
                sources,
                path,
                true,
                selected_directory,
                selected_directory.is_some(),
                &mut bytes_read,
                &mut path_observations,
                &mut state_bytes,
                limits,
                deadline,
                cancelled,
                git_signal,
                candidate_io_budget,
                &mut shared_read_bytes_returned,
            )?;
            if let Some(stamp) = observation.stamp {
                charge_map_entry(
                    path,
                    std::mem::size_of::<FileStamp>(),
                    0,
                    &mut state_bytes,
                    limits.max_state_bytes,
                )?;
                private_stamps.insert(path.clone(), stamp);
            }
            if let Some(stamp) = observation.resolved_stamp {
                charge_map_entry(
                    path,
                    std::mem::size_of::<FileStamp>(),
                    0,
                    &mut state_bytes,
                    limits.max_state_bytes,
                )?;
                private_resolved_stamps.insert(path.clone(), stamp);
            }
            put_private_fact(
                &mut facts.private_paths,
                path,
                observation.facts,
                &mut state_bytes,
                limits.max_state_bytes,
            )?;
        }

        if private_prefixes.len() > limits.max_private_prefixes {
            return Err(budget(
                "physical-private-prefix-count",
                private_prefixes.len(),
                limits.max_private_prefixes,
            ));
        }
        for prefix in private_prefixes {
            validate_repo_path(prefix)?;
            if prefixes
                .iter()
                .any(|old: &String| paths_overlap(old, prefix))
            {
                return Err(ItemRefusal::Unsupported(
                    "overlapping private inventory prefixes".into(),
                ));
            }
            insert_path(
                &mut repo_paths,
                prefix,
                artifact_count,
                &mut state_bytes,
                limits,
            )?;
            insert_path(
                &mut git_paths,
                prefix,
                artifact_count,
                &mut state_bytes,
                limits,
            )?;
            charge_path_value(prefix, &mut state_bytes, limits.max_state_bytes)?;
            prefixes.push(prefix.clone());
        }
        prefixes.sort();

        let mut private_inventories = BTreeMap::new();
        let mut inventory_entry_counts = BTreeMap::new();
        let mut inventory_entries_total = 0usize;
        let mut all_private_files = BTreeSet::new();
        let mut inventory_complete = true;
        for prefix in &prefixes {
            let root_observation = observe_path(
                sources,
                prefix,
                false,
                None,
                false,
                &mut bytes_read,
                &mut path_observations,
                &mut state_bytes,
                limits,
                deadline,
                cancelled,
                git_signal,
            )?;
            if let Some(stamp) = root_observation.stamp {
                charge_map_entry(
                    prefix,
                    std::mem::size_of::<FileStamp>(),
                    0,
                    &mut state_bytes,
                    limits.max_state_bytes,
                )?;
                private_stamps.insert(prefix.clone(), stamp);
            }
            let root_is_directory = root_observation.facts.exists
                && root_observation.facts.directory
                && !root_observation.facts.symlink;
            put_private_fact(
                &mut facts.private_paths,
                prefix,
                root_observation.facts.clone(),
                &mut state_bytes,
                limits.max_state_bytes,
            )?;
            if !root_observation.facts.exists {
                insert_inventory(
                    &mut private_inventories,
                    prefix,
                    Vec::new(),
                    &mut state_bytes,
                    limits.max_state_bytes,
                )?;
                put_inventory_count(
                    &mut inventory_entry_counts,
                    prefix,
                    0,
                    &mut state_bytes,
                    limits.max_state_bytes,
                )?;
                continue;
            }
            if !root_is_directory {
                inventory_complete = false;
                put_inventory_count(
                    &mut inventory_entry_counts,
                    prefix,
                    0,
                    &mut state_bytes,
                    limits.max_state_bytes,
                )?;
                continue;
            }
            let walk = enumerate_private_prefix(
                sources,
                prefix,
                &mut repo_paths,
                &mut git_paths,
                &mut facts.private_paths,
                &mut private_stamps,
                &mut private_resolved_stamps,
                &mut all_private_files,
                &mut path_observations,
                &mut state_bytes,
                &mut inventory_entries_total,
                artifact_count,
                limits,
                deadline,
                cancelled,
                git_signal,
            )?;
            put_inventory_count(
                &mut inventory_entry_counts,
                prefix,
                walk.entries_seen,
                &mut state_bytes,
                limits.max_state_bytes,
            )?;
            match walk.files {
                Some(files) => {
                    insert_inventory(
                        &mut private_inventories,
                        prefix,
                        files,
                        &mut state_bytes,
                        limits.max_state_bytes,
                    )?;
                }
                None => inventory_complete = false,
            }
        }
        facts.private_inventories = private_inventories;
        if inventory_complete && !prefixes.is_empty() {
            let mut all_files = Vec::with_capacity(all_private_files.len());
            for path in all_private_files {
                charge_vec_slot(&mut state_bytes, limits.max_state_bytes)?;
                all_files.push(path);
            }
            facts.private_files = Some(all_files);
        }

        let artifact_provider_selected = artifact_sources.is_some();
        let mut artifact_root_stamp = None;
        let mut artifact_stamps = BTreeMap::new();
        let mut artifact_resolved_stamps = BTreeMap::new();
        if let Some(artifact_root) = artifact_sources.as_deref_mut() {
            checkpoint(artifact_root, deadline, cancelled, git_signal)?;
            artifact_root
                .verify_root()
                .map_err(|error| route_error(error, deadline, cancelled, git_signal))?;
            let metadata = artifact_root
                .metadata(".")
                .map_err(|error| route_error(error, deadline, cancelled, git_signal))?
                .ok_or_else(|| ItemRefusal::Source("selected artifact root disappeared".into()))?;
            if !metadata.is_dir() {
                return Err(ItemRefusal::Source(
                    "selected artifact root is not a directory".into(),
                ));
            }
            let stamp = FileStamp::from_metadata(&metadata);
            artifact_root_stamp = Some(stamp);
            for path in &artifact_requested_paths {
                let observation = observe_path_with_shared(
                    artifact_root,
                    path,
                    true,
                    None,
                    true,
                    &mut bytes_read,
                    &mut path_observations,
                    &mut state_bytes,
                    limits,
                    deadline,
                    cancelled,
                    git_signal,
                    candidate_io_budget,
                    &mut shared_read_bytes_returned,
                )?;
                charge_map_entry(
                    path,
                    std::mem::size_of::<PhysicalPathFacts>(),
                    physical_dynamic_bytes(&observation.facts)?,
                    &mut state_bytes,
                    limits.max_state_bytes,
                )?;
                facts.artifact_paths.insert(path.clone(), observation.facts);
                if let Some(stamp) = observation.stamp {
                    charge_map_entry(
                        path,
                        std::mem::size_of::<FileStamp>(),
                        0,
                        &mut state_bytes,
                        limits.max_state_bytes,
                    )?;
                    artifact_stamps.insert(path.clone(), stamp);
                }
                if let Some(stamp) = observation.resolved_stamp {
                    charge_map_entry(
                        path,
                        std::mem::size_of::<FileStamp>(),
                        0,
                        &mut state_bytes,
                        limits.max_state_bytes,
                    )?;
                    artifact_resolved_stamps.insert(path.clone(), stamp);
                }
            }
        }

        for physical in facts
            .authored_paths
            .values()
            .chain(facts.private_paths.values())
        {
            if let Some(PhysicalResolvedTargetFacts::InsideSelectedRoot {
                relative_target, ..
            }) = physical.resolved_target.as_ref()
            {
                validate_repo_path(relative_target)?;
                insert_path(
                    &mut repo_paths,
                    relative_target,
                    artifact_count,
                    &mut state_bytes,
                    limits,
                )?;
                insert_path(
                    &mut git_paths,
                    relative_target,
                    artifact_count,
                    &mut state_bytes,
                    limits,
                )?;
            }
        }

        let git_root_present = git_root_present(sources, deadline, cancelled, git_signal)?;
        preflight_final_cost(
            &facts,
            &authored_capture_paths,
            &private_declared_paths,
            &prefixes,
            &inventory_entry_counts,
            &artifact_requested_paths,
            artifact_provider_selected,
            path_observations,
            bytes_read,
            git_paths.len(),
            git_root_present,
            state_bytes,
            limits,
        )?;
        let mut git_path_queries = 0usize;
        let git_available = query_git_paths(
            sources,
            &git_paths,
            git_root_present,
            &mut git_path_queries,
            limits,
            deadline,
            cancelled,
            git_signal,
            |path, tracked, ignored| apply_git_facts(&mut facts, path, tracked, ignored),
        )?;
        facts.git_available = git_available;
        sources
            .verify_root()
            .map_err(|error| route_error(error, deadline, cancelled, git_signal))?;
        checkpoint(sources, deadline, cancelled, git_signal)?;
        if let Some(artifact_root) = artifact_sources.as_deref_mut() {
            artifact_root
                .verify_root()
                .map_err(|error| route_error(error, deadline, cancelled, git_signal))?;
            checkpoint(artifact_root, deadline, cancelled, git_signal)?;
        }

        let source_root_component_open_count_after_observation =
            sources.root_component_open_count();
        let artifact_root_component_open_count_after_observation = artifact_sources
            .as_deref()
            .map(RouteSources::root_component_open_count);
        Ok(Self {
            facts,
            historical_originals: BTreeMap::new(),
            repo_paths,
            authored_capture_paths,
            candidate_member_paths,
            private_declared_paths,
            private_prefixes: prefixes,
            git_paths,
            private_stamps,
            private_resolved_stamps,
            authored_stamps,
            authored_resolved_stamps,
            resolved_source_directories: owned_resolved_source_directories,
            artifact_requested_paths,
            artifact_stamps,
            artifact_resolved_stamps,
            artifact_root_stamp,
            artifact_provider_selected,
            git_root_present,
            inventory_entry_counts,
            inventory_entries_total,
            limits,
            deadline,
            cancelled,
            git_signal,
            candidate_identity: None,
            candidate_io_budget: candidate_io_budget.cloned(),
            bytes_read,
            shared_read_bytes_returned,
            path_observations,
            git_path_queries,
            state_bytes,
            source_root_component_open_count_initial,
            source_root_component_open_count_after_observation,
            source_root_component_open_count_final: None,
            artifact_root_component_open_count_initial,
            artifact_root_component_open_count_after_observation,
            artifact_root_component_open_count_final: None,
        })
    }

    pub fn facts(&self) -> &SourcePhysicalFacts {
        &self.facts
    }

    /// Overlay only already selected path observations with authentic
    /// historical facts. Original primary-root facts stay bound for EOF;
    /// historical roots have their own final provider recheck.
    pub(crate) fn select_historical(
        &mut self,
        history: &mut dyn super::foundation_reader::FoundationHistoricalEvidence,
    ) -> Result<(), ItemRefusal> {
        for (path, facts) in self
            .facts
            .authored_paths
            .iter_mut()
            .chain(self.facts.private_paths.iter_mut())
        {
            reader_history_checkpoint(self.deadline, self.cancelled)?;
            if !history.selected(path) {
                continue;
            }
            if self.authored_capture_paths.contains(path)
                || self.candidate_member_paths.contains(path)
                || self.historical_originals.contains_key(path)
            {
                return Err(ItemRefusal::Source(
                    "historical physical input overlaps candidate".into(),
                ));
            }
            let remaining_read = self
                .limits
                .max_total_bytes
                .checked_sub(self.bytes_read)
                .ok_or(ItemRefusal::Budget)?;
            // Reserve cloned baseline and the bounded SHA/returned-facts
            // workspace before the provider allocates or hashes anything.
            charge_map_entry(
                path,
                std::mem::size_of::<PhysicalPathFacts>(),
                physical_dynamic_bytes(facts)?,
                &mut self.state_bytes,
                self.limits.max_state_bytes,
            )?;
            reserve_state(
                &mut self.state_bytes,
                64usize
                    .checked_add(std::mem::size_of::<PhysicalPathFacts>())
                    .ok_or(ItemRefusal::Budget)?,
                self.limits.max_state_bytes,
            )?;
            let remaining_state = self
                .limits
                .max_state_bytes
                .checked_sub(self.state_bytes)
                .ok_or(ItemRefusal::Budget)?;
            let before = history.usage();
            let selected = history.physical(
                path,
                remaining_read.min(self.limits.max_file_bytes) as u64,
                remaining_state,
                self.deadline,
                self.cancelled,
            );
            let after = history.usage();
            let read_delta =
                usize::try_from(after.0.checked_sub(before.0).ok_or(ItemRefusal::Budget)?)
                    .map_err(|_| ItemRefusal::Budget)?;
            self.bytes_read = self
                .bytes_read
                .checked_add(read_delta)
                .filter(|bytes| *bytes <= self.limits.max_total_bytes)
                .ok_or(ItemRefusal::Budget)?;
            reserve_state(
                &mut self.state_bytes,
                after.1.checked_sub(before.1).ok_or(ItemRefusal::Budget)?,
                self.limits.max_state_bytes,
            )?;
            let selected = selected
                .map_err(|_| ItemRefusal::Source("historical physical custody refused".into()))?
                .ok_or_else(|| {
                    ItemRefusal::Source("selected historical physical input missing".into())
                })?;
            if !selected.exists
                || !selected.regular_file
                || selected.directory
                || selected.symlink
                || selected.resolved_target.is_some()
                || selected.git_tracked.is_some()
                || selected.git_ignored.is_some()
                || selected.file_mode.is_none()
                || selected.byte_size.is_none()
                || selected
                    .sha256
                    .as_deref()
                    .is_none_or(|digest| Digest256::from_hex(digest).is_err())
            {
                return Err(ItemRefusal::Source(
                    "historical physical input binding differs".into(),
                ));
            }
            self.historical_originals
                .insert(path.clone(), facts.clone());
            *facts = selected;
            if let Some(git) = self.facts.authored_git.get_mut(path) {
                git.tracked = None;
                git.ignored = None;
            }
        }
        reader_history_checkpoint(self.deadline, self.cancelled)
    }

    pub fn bytes_read(&self) -> usize {
        self.bytes_read
    }

    pub fn path_observations(&self) -> usize {
        self.path_observations
    }

    pub fn git_path_queries(&self) -> usize {
        self.git_path_queries
    }

    pub fn state_bytes(&self) -> usize {
        self.state_bytes
    }

    /// True only when this candidate snapshot retained the exact original
    /// cumulative read ledger supplied at construction.
    pub fn shared_io_budget_matches(&self, budget: &PinnedSqliteIoBudget) -> bool {
        self.candidate_identity.is_some()
            && self
                .candidate_io_budget
                .as_ref()
                .is_some_and(|retained| retained.shares_with(budget))
    }

    /// Candidate snapshots forward every bounded physical content read through
    /// their retained shared ledger; captured snapshots retain the legacy route.
    pub fn forwards_physical_reads_to_shared_io(&self) -> bool {
        self.candidate_identity.is_some() && self.candidate_io_budget.is_some()
    }

    pub fn cost(&self) -> PhysicalSourceCost {
        PhysicalSourceCost {
            bytes_read: self.bytes_read,
            shared_read_bytes_returned: self.shared_read_bytes_returned,
            path_observations: self.path_observations,
            git_path_queries: self.git_path_queries,
            retained_state_bytes: self.state_bytes,
            source_root_component_open_count_initial: self.source_root_component_open_count_initial,
            source_root_component_open_count_after_observation: self
                .source_root_component_open_count_after_observation,
            source_root_component_open_count_final: self.source_root_component_open_count_final,
            artifact_root_component_open_count_initial: self
                .artifact_root_component_open_count_initial,
            artifact_root_component_open_count_after_observation: self
                .artifact_root_component_open_count_after_observation,
            artifact_root_component_open_count_final: self.artifact_root_component_open_count_final,
        }
    }

    pub fn paths_observed(&self) -> usize {
        self.repo_paths.len() + self.artifact_requested_paths.len()
    }

    /// Recheck physical/Git facts for a captured-source snapshot. Candidate
    /// snapshots must use `recheck_candidate` so their original source fence
    /// is also verified. The caller separately rechecks captured bytes or the
    /// candidate fence, publication epoch, and payload adapter as applicable.
    pub fn recheck(
        &mut self,
        sources: &mut RouteSources,
        artifact_sources: Option<&mut RouteSources>,
    ) -> Result<(), ItemRefusal> {
        if self.candidate_identity.is_some() || self.candidate_io_budget.is_some() {
            return Err(ItemRefusal::Source(
                "candidate physical snapshot requires its source fence".into(),
            ));
        }
        self.recheck_observations(sources, artifact_sources)
    }

    /// Recheck physical observations and the same candidate coverage after
    /// dependent workers have stopped. Candidate bytes remain owned by the
    /// candidate adapter; this verifies its original fence without comparing
    /// those bytes to the selected working tree.
    pub(crate) fn recheck_candidate(
        &mut self,
        sources: &mut RouteSources,
        artifact_sources: Option<&mut RouteSources>,
        candidate: &CandidateRecordsInput<'_, '_>,
        coverage: &SourceCutInputCoverage,
    ) -> Result<(), ItemRefusal> {
        let Some(original_io) = self.candidate_io_budget.as_ref() else {
            candidate.abandon();
            return Err(ItemRefusal::Source(
                "candidate physical source identity changed".into(),
            ));
        };
        if !candidate.shares_io_budget(original_io)
            || self.candidate_identity != Some(*candidate.input_identity())
        {
            candidate.abandon();
            return Err(ItemRefusal::Source(
                "candidate physical source identity changed".into(),
            ));
        }
        candidate
            .source_input()
            .verify_current_fence(coverage, self.deadline, self.cancelled)?;
        self.recheck_observations(sources, artifact_sources)?;
        candidate
            .source_input()
            .verify_current_fence(coverage, self.deadline, self.cancelled)
    }

    fn recheck_observations(
        &mut self,
        sources: &mut RouteSources,
        mut artifact_sources: Option<&mut RouteSources>,
    ) -> Result<(), ItemRefusal> {
        checkpoint(sources, self.deadline, self.cancelled, self.git_signal)?;
        sources
            .verify_root()
            .map_err(|error| route_error(error, self.deadline, self.cancelled, self.git_signal))?;

        if artifact_sources.is_some() != self.artifact_provider_selected {
            return Err(ItemRefusal::Source(
                "selected artifact provider changed after observation".into(),
            ));
        }
        if let Some(artifact_root) = artifact_sources.as_deref_mut() {
            recheck_artifacts(self, artifact_root)?;
        }

        for path in self.facts.authored_paths.keys() {
            // Captured authored bytes have their own exact-cut recheck. A
            // candidate's bytes are held in its independent spool, so every
            // selected working-tree path is still hashed as a physical fact.
            let hash_file =
                self.candidate_identity.is_some() || !self.authored_capture_paths.contains(path);
            let selected_directory = self
                .resolved_source_directories
                .get(path)
                .map(String::as_str);
            let current = observe_path_with_shared(
                sources,
                path,
                hash_file,
                selected_directory,
                selected_directory.is_some(),
                &mut self.bytes_read,
                &mut self.path_observations,
                &mut self.state_bytes,
                self.limits,
                self.deadline,
                self.cancelled,
                self.git_signal,
                self.candidate_io_budget.as_ref(),
                &mut self.shared_read_bytes_returned,
            )?;
            let expected = self
                .historical_originals
                .get(path)
                .or_else(|| self.facts.authored_paths.get(path))
                .ok_or_else(|| ItemRefusal::Source("authored path snapshot missing".into()))?;
            let physical_match = if hash_file {
                same_physical_path(expected, &current.facts)
            } else {
                same_physical_metadata(expected, &current.facts)
            };
            if !physical_match
                || self.authored_stamps.get(path).copied() != current.stamp
                || self.authored_resolved_stamps.get(path).copied() != current.resolved_stamp
            {
                return Err(ItemRefusal::Source(
                    "selected authored physical facts changed after observation".into(),
                ));
            }
        }

        for path in &self.private_declared_paths {
            let expected = self
                .historical_originals
                .get(path)
                .or_else(|| self.facts.private_paths.get(path))
                .ok_or_else(|| ItemRefusal::Source("private path snapshot missing".into()))?;
            let inventory_directory = self
                .private_prefixes
                .iter()
                .find(|prefix| path_within(path, prefix))
                .map(String::as_str);
            let requested_directory = self
                .resolved_source_directories
                .get(path)
                .map(String::as_str);
            if requested_directory.is_some()
                && inventory_directory.is_some()
                && requested_directory != inventory_directory
            {
                return Err(ItemRefusal::Source(
                    "private path resolution scope changed after observation".into(),
                ));
            }
            let selected_directory = requested_directory.or(inventory_directory);
            let current = observe_path_with_shared(
                sources,
                path,
                true,
                selected_directory,
                selected_directory.is_some(),
                &mut self.bytes_read,
                &mut self.path_observations,
                &mut self.state_bytes,
                self.limits,
                self.deadline,
                self.cancelled,
                self.git_signal,
                self.candidate_io_budget.as_ref(),
                &mut self.shared_read_bytes_returned,
            )?;
            if !same_physical_path(expected, &current.facts)
                || self.private_stamps.get(path).copied() != current.stamp
                || self.private_resolved_stamps.get(path).copied() != current.resolved_stamp
            {
                return Err(ItemRefusal::Source(
                    "selected private path changed after observation".into(),
                ));
            }
        }

        for index in 0..self.private_prefixes.len() {
            let prefix = self.private_prefixes[index].clone();
            let transient = prefix.len() + std::mem::size_of::<String>();
            if self
                .state_bytes
                .checked_add(transient)
                .is_none_or(|peak| peak > self.limits.max_state_bytes)
            {
                return Err(budget(
                    "physical-source-state-bytes",
                    self.state_bytes.saturating_add(transient),
                    self.limits.max_state_bytes,
                ));
            }
            recheck_private_prefix(self, sources, &prefix)?;
        }

        let now_has_git =
            git_root_present(sources, self.deadline, self.cancelled, self.git_signal)?;
        if now_has_git != self.git_root_present {
            return Err(ItemRefusal::Source(
                "selected repository Git availability changed".into(),
            ));
        }
        let mut git_available = None;
        query_git_paths(
            sources,
            &self.git_paths,
            now_has_git,
            &mut self.git_path_queries,
            self.limits,
            self.deadline,
            self.cancelled,
            self.git_signal,
            |path, tracked, ignored| {
                let expected = self
                    .historical_originals
                    .get(path)
                    .map(|facts| (facts.git_tracked, facts.git_ignored))
                    .or_else(|| expected_git_values(&self.facts, path));
                if expected != Some((tracked, ignored)) {
                    return Err(ItemRefusal::Source(
                        "selected repository Git path facts changed".into(),
                    ));
                }
                if tracked.is_some() || ignored.is_some() {
                    git_available = Some(true);
                }
                Ok(())
            },
        )?;
        if !now_has_git {
            git_available = Some(false);
        }
        if git_available != self.facts.git_available {
            return Err(ItemRefusal::Source(
                "selected repository Git availability changed".into(),
            ));
        }

        sources
            .verify_root()
            .map_err(|error| route_error(error, self.deadline, self.cancelled, self.git_signal))?;
        checkpoint(sources, self.deadline, self.cancelled, self.git_signal)?;
        self.source_root_component_open_count_final = Some(sources.root_component_open_count());
        self.artifact_root_component_open_count_final = artifact_sources
            .as_deref()
            .map(RouteSources::root_component_open_count);
        Ok(())
    }
}

fn recheck_artifacts(
    snapshot: &mut FoundationPhysicalSnapshot<'_, '_>,
    sources: &mut RouteSources,
) -> Result<(), ItemRefusal> {
    checkpoint(
        sources,
        snapshot.deadline,
        snapshot.cancelled,
        snapshot.git_signal,
    )?;
    sources.verify_root().map_err(|error| {
        route_error(
            error,
            snapshot.deadline,
            snapshot.cancelled,
            snapshot.git_signal,
        )
    })?;
    let root_metadata = sources
        .metadata(".")
        .map_err(|error| {
            route_error(
                error,
                snapshot.deadline,
                snapshot.cancelled,
                snapshot.git_signal,
            )
        })?
        .ok_or_else(|| ItemRefusal::Source("selected artifact root disappeared".into()))?;
    if !root_metadata.is_dir()
        || snapshot.artifact_root_stamp != Some(FileStamp::from_metadata(&root_metadata))
    {
        return Err(ItemRefusal::Source(
            "selected artifact root changed after observation".into(),
        ));
    }
    for path in &snapshot.artifact_requested_paths {
        let observation = observe_path_with_shared(
            sources,
            path,
            true,
            None,
            true,
            &mut snapshot.bytes_read,
            &mut snapshot.path_observations,
            &mut snapshot.state_bytes,
            snapshot.limits,
            snapshot.deadline,
            snapshot.cancelled,
            snapshot.git_signal,
            snapshot.candidate_io_budget.as_ref(),
            &mut snapshot.shared_read_bytes_returned,
        )?;
        let expected = snapshot
            .facts
            .artifact_paths
            .get(path)
            .ok_or_else(|| ItemRefusal::Source("artifact path snapshot missing".into()))?;
        if !same_physical_path(expected, &observation.facts)
            || snapshot.artifact_stamps.get(path).copied() != observation.stamp
            || snapshot.artifact_resolved_stamps.get(path).copied() != observation.resolved_stamp
        {
            return Err(ItemRefusal::Source(
                "selected artifact path changed after observation".into(),
            ));
        }
    }
    sources.verify_root().map_err(|error| {
        route_error(
            error,
            snapshot.deadline,
            snapshot.cancelled,
            snapshot.git_signal,
        )
    })?;
    checkpoint(
        sources,
        snapshot.deadline,
        snapshot.cancelled,
        snapshot.git_signal,
    )
}

fn recheck_private_prefix(
    snapshot: &mut FoundationPhysicalSnapshot<'_, '_>,
    sources: &mut RouteSources,
    prefix: &str,
) -> Result<(), ItemRefusal> {
    let expected_prefix = snapshot
        .facts
        .private_paths
        .get(prefix)
        .ok_or_else(|| ItemRefusal::Source("private prefix snapshot missing".into()))?;
    let now = observe_path(
        sources,
        prefix,
        false,
        None,
        false,
        &mut snapshot.bytes_read,
        &mut snapshot.path_observations,
        &mut snapshot.state_bytes,
        snapshot.limits,
        snapshot.deadline,
        snapshot.cancelled,
        snapshot.git_signal,
    )?;
    if !same_physical_metadata(expected_prefix, &now.facts)
        || snapshot.private_stamps.get(prefix).copied() != now.stamp
    {
        return Err(ItemRefusal::Source(
            "private inventory prefix changed after observation".into(),
        ));
    }

    let Some(expected_files) = snapshot.facts.private_inventories.get(prefix) else {
        // A prefix which was inaccessible, linked, or over budget remains an
        // explicit unsupported observation. It is never upgraded to empty.
        // Still recheck every descendant entry already observed before the
        // inventory became incomplete; this fences partial walks and explicit
        // link facts without pretending to know the omitted inventory.
        let expected_paths = &snapshot.facts.private_paths;
        let expected_stamps = &snapshot.private_stamps;
        let expected_resolved_stamps = &snapshot.private_resolved_stamps;
        for (path, expected) in expected_paths {
            if path == prefix || !path_within(path, prefix) || !expected.exists {
                continue;
            }
            checkpoint(
                sources,
                snapshot.deadline,
                snapshot.cancelled,
                snapshot.git_signal,
            )?;
            let observation = observe_path(
                sources,
                path,
                false,
                Some(prefix),
                true,
                &mut snapshot.bytes_read,
                &mut snapshot.path_observations,
                &mut snapshot.state_bytes,
                snapshot.limits,
                snapshot.deadline,
                snapshot.cancelled,
                snapshot.git_signal,
            )?;
            if !same_physical_metadata(expected, &observation.facts)
                || expected_stamps.get(path).copied() != observation.stamp
                || expected_resolved_stamps.get(path).copied() != observation.resolved_stamp
            {
                return Err(ItemRefusal::Source(
                    "partial private inventory entry changed after observation".into(),
                ));
            }
        }
        return Ok(());
    };
    if !expected_prefix.exists {
        if !expected_files.is_empty() {
            return Err(ItemRefusal::Source(
                "missing private prefix has nonempty inventory".into(),
            ));
        }
        return Ok(());
    }
    if !expected_prefix.exists || !expected_prefix.directory || expected_prefix.symlink {
        return Err(ItemRefusal::Source(
            "private inventory root changed type after observation".into(),
        ));
    }

    let current_expected_entries = snapshot
        .inventory_entry_counts
        .get(prefix)
        .copied()
        .unwrap_or_default();
    let remaining = snapshot.limits.max_inventory_paths.saturating_sub(
        snapshot
            .inventory_entries_total
            .saturating_sub(current_expected_entries),
    );
    let local_count = Cell::new(0usize);
    let local_state = Cell::new(0usize);
    let over_paths = Cell::new(false);
    let over_state = Cell::new(false);
    let stopped = Cell::new(false);
    let paths = sources
        .selected_physical_paths(prefix, &|path, _directory| {
            if snapshot.cancelled.load(Ordering::Relaxed)
                || snapshot.git_signal.load(Ordering::Relaxed) != 0
                || Instant::now() >= snapshot.deadline
            {
                stopped.set(true);
                return false;
            }
            // The callback checks the selected root too, but the returned
            // inventory contains only descendants. The prefix has already
            // been observed and priced separately above.
            if path == prefix {
                return true;
            }
            let next = local_count.get().saturating_add(1);
            if next > remaining {
                over_paths.set(true);
                return false;
            }
            let cost = transient_walk_cost(path);
            let state = local_state.get().saturating_add(cost);
            if snapshot
                .state_bytes
                .checked_add(state)
                .is_none_or(|peak| peak > snapshot.limits.max_state_bytes)
            {
                over_state.set(true);
                return false;
            }
            local_count.set(next);
            local_state.set(state);
            true
        })
        .map_err(|error| {
            route_error(
                error,
                snapshot.deadline,
                snapshot.cancelled,
                snapshot.git_signal,
            )
        })?;
    if stopped.get() {
        return Err(check_context(
            snapshot.deadline,
            snapshot.cancelled,
            snapshot.git_signal,
        ));
    }
    if over_state.get() {
        return Err(budget(
            "physical-private-inventory-state-bytes",
            snapshot.state_bytes.saturating_add(local_state.get()),
            snapshot.limits.max_state_bytes,
        ));
    }
    if over_paths.get() || paths.len() != local_count.get() {
        return Err(budget(
            "physical-private-inventory-paths",
            remaining.saturating_add(1),
            snapshot.limits.max_inventory_paths,
        ));
    }

    let mut current_files = Vec::new();
    if paths.len()
        != snapshot
            .inventory_entry_counts
            .get(prefix)
            .copied()
            .unwrap_or_default()
    {
        return Err(ItemRefusal::Source(
            "private inventory entry count changed after observation".into(),
        ));
    }
    for path in paths {
        checkpoint(
            sources,
            snapshot.deadline,
            snapshot.cancelled,
            snapshot.git_signal,
        )?;
        let mut observation_state_bytes = snapshot
            .state_bytes
            .checked_add(local_state.get())
            .ok_or(ItemRefusal::Budget)?;
        let observation = observe_path(
            sources,
            &path,
            false,
            Some(prefix),
            true,
            &mut snapshot.bytes_read,
            &mut snapshot.path_observations,
            &mut observation_state_bytes,
            snapshot.limits,
            snapshot.deadline,
            snapshot.cancelled,
            snapshot.git_signal,
        )?;
        let expected_path = snapshot
            .historical_originals
            .get(&path)
            .or_else(|| snapshot.facts.private_paths.get(&path))
            .ok_or_else(|| ItemRefusal::Source("private inventory path snapshot missing".into()))?;
        if !same_physical_metadata(expected_path, &observation.facts)
            || snapshot.private_stamps.get(&path).copied() != observation.stamp
            || snapshot.private_resolved_stamps.get(&path).copied() != observation.resolved_stamp
        {
            return Err(ItemRefusal::Source(
                "private inventory path changed after observation".into(),
            ));
        }
        if inventory_is_file(&observation.facts) {
            current_files.push(path);
        } else if !observation.facts.exists {
            return Err(ItemRefusal::Source(
                "private inventory entry changed type after observation".into(),
            ));
        }
    }
    if &current_files != expected_files {
        return Err(ItemRefusal::Source(
            "private inventory membership changed after observation".into(),
        ));
    }
    Ok(())
}

#[derive(Debug)]
struct InventoryWalk {
    files: Option<Vec<String>>,
    entries_seen: usize,
}

fn enumerate_private_prefix(
    sources: &mut RouteSources,
    prefix: &str,
    repo_paths: &mut BTreeSet<String>,
    git_paths: &mut BTreeSet<String>,
    private_facts: &mut BTreeMap<String, PhysicalPathFacts>,
    private_stamps: &mut BTreeMap<String, FileStamp>,
    private_resolved_stamps: &mut BTreeMap<String, FileStamp>,
    all_private_files: &mut BTreeSet<String>,
    path_observations: &mut usize,
    state_bytes: &mut usize,
    inventory_entries_total: &mut usize,
    artifact_count: usize,
    limits: PhysicalSourceLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    git_signal: &AtomicI32,
) -> Result<InventoryWalk, ItemRefusal> {
    if limits.max_inventory_paths == 0 {
        return Ok(InventoryWalk {
            files: None,
            entries_seen: 0,
        });
    }
    let remaining = limits
        .max_inventory_paths
        .saturating_sub(*inventory_entries_total);
    let local_count = Cell::new(0usize);
    let local_state = Cell::new(0usize);
    let over_paths = Cell::new(false);
    let over_state = Cell::new(false);
    let stopped = Cell::new(false);
    let baseline_state = *state_bytes;
    let paths_result = sources.selected_physical_paths(prefix, &|path, _directory| {
        if cancelled.load(Ordering::Relaxed)
            || git_signal.load(Ordering::Relaxed) != 0
            || Instant::now() >= deadline
        {
            stopped.set(true);
            return false;
        }
        // Root admission is not a returned descendant. Its existing physical
        // observation owns that cost; count only this inventory's entries.
        if path == prefix {
            return true;
        }
        let next = local_count.get().saturating_add(1);
        if next > remaining {
            over_paths.set(true);
            return false;
        }
        let state = local_state.get().saturating_add(transient_walk_cost(path));
        if baseline_state
            .checked_add(state)
            .is_none_or(|peak| peak > limits.max_state_bytes)
        {
            over_state.set(true);
            return false;
        }
        local_count.set(next);
        local_state.set(state);
        true
    });
    if stopped.get() {
        return Err(check_context(deadline, cancelled, git_signal));
    }
    if over_state.get() {
        return Err(budget(
            "physical-private-inventory-state-bytes",
            baseline_state.saturating_add(local_state.get()),
            limits.max_state_bytes,
        ));
    }
    if over_paths.get() {
        return Err(budget(
            "physical-private-inventory-paths",
            inventory_entries_total
                .saturating_add(local_count.get())
                .saturating_add(1),
            limits.max_inventory_paths,
        ));
    }
    let mut paths = match paths_result {
        Ok(paths) => paths,
        Err(error) if inventory_unavailable(&error) => {
            *inventory_entries_total = (*inventory_entries_total)
                .checked_add(local_count.get())
                .ok_or(ItemRefusal::Budget)?;
            return Ok(InventoryWalk {
                files: None,
                entries_seen: local_count.get(),
            });
        }
        Err(error) => return Err(route_error(error, deadline, cancelled, git_signal)),
    };
    if paths.len() != local_count.get() {
        return Err(ItemRefusal::Budget);
    }
    let entries_seen = paths.len();
    *inventory_entries_total = (*inventory_entries_total)
        .checked_add(paths.len())
        .ok_or(ItemRefusal::Budget)?;

    let mut files = Vec::new();
    let mut complete = true;
    for path in paths.drain(..) {
        checkpoint(sources, deadline, cancelled, git_signal)?;
        let mut unused_bytes_read = 0usize;
        let mut observation_state_bytes = state_bytes
            .checked_add(local_state.get())
            .ok_or(ItemRefusal::Budget)?;
        let observation = observe_path(
            sources,
            &path,
            false,
            Some(prefix),
            true,
            &mut unused_bytes_read,
            path_observations,
            &mut observation_state_bytes,
            limits,
            deadline,
            cancelled,
            git_signal,
        )?;
        if !observation.facts.exists {
            return Err(ItemRefusal::Source(
                "private inventory entry changed during enumeration".into(),
            ));
        }
        if matches!(
            observation.facts.resolved_target.as_ref(),
            Some(PhysicalResolvedTargetFacts::OutsideSelectedRoot)
        ) {
            // The physical link name is known, but its target lies outside the
            // selected inventory capability. Keep the inventory unavailable.
            complete = false;
        }
        insert_path(repo_paths, &path, artifact_count, state_bytes, limits)?;
        insert_path(git_paths, &path, artifact_count, state_bytes, limits)?;
        put_private_fact(
            private_facts,
            &path,
            observation.facts.clone(),
            state_bytes,
            limits.max_state_bytes,
        )?;
        if let Some(stamp) = observation.stamp {
            charge_map_entry(
                &path,
                std::mem::size_of::<FileStamp>(),
                0,
                state_bytes,
                limits.max_state_bytes,
            )?;
            private_stamps.insert(path.clone(), stamp);
        }
        if let Some(stamp) = observation.resolved_stamp {
            charge_map_entry(
                &path,
                std::mem::size_of::<FileStamp>(),
                0,
                state_bytes,
                limits.max_state_bytes,
            )?;
            private_resolved_stamps.insert(path.clone(), stamp);
        }
        if inventory_is_file(&observation.facts) {
            if !all_private_files.contains(&path) {
                charge_path_value(&path, state_bytes, limits.max_state_bytes)?;
                all_private_files.insert(path.clone());
            }
            charge_path_value(&path, state_bytes, limits.max_state_bytes)?;
            files.push(path);
        }
    }
    Ok(InventoryWalk {
        files: complete.then_some(files),
        entries_seen,
    })
}

fn inventory_is_file(facts: &PhysicalPathFacts) -> bool {
    facts.regular_file
        || matches!(
            facts.resolved_target.as_ref(),
            Some(PhysicalResolvedTargetFacts::InsideSelectedRoot {
                exists: true,
                regular_file: true,
                ..
            })
        )
}

#[allow(clippy::too_many_arguments)]
fn observe_path(
    sources: &mut RouteSources,
    path: &str,
    hash_file: bool,
    selected_directory: Option<&str>,
    resolve_target: bool,
    bytes_read: &mut usize,
    path_observations: &mut usize,
    state_bytes: &mut usize,
    limits: PhysicalSourceLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    git_signal: &AtomicI32,
) -> Result<PathObservation, ItemRefusal> {
    let mut shared_read_bytes_returned = 0usize;
    observe_path_with_shared(
        sources,
        path,
        hash_file,
        selected_directory,
        resolve_target,
        bytes_read,
        path_observations,
        state_bytes,
        limits,
        deadline,
        cancelled,
        git_signal,
        None,
        &mut shared_read_bytes_returned,
    )
}

fn observe_path_with_shared(
    sources: &mut RouteSources,
    path: &str,
    hash_file: bool,
    selected_directory: Option<&str>,
    resolve_target: bool,
    bytes_read: &mut usize,
    path_observations: &mut usize,
    state_bytes: &mut usize,
    limits: PhysicalSourceLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    git_signal: &AtomicI32,
    shared_io_budget: Option<&PinnedSqliteIoBudget>,
    shared_read_bytes_returned: &mut usize,
) -> Result<PathObservation, ItemRefusal> {
    charge_path_observation(path_observations, limits.max_path_observations)?;
    checkpoint(sources, deadline, cancelled, git_signal)?;
    #[cfg(not(target_os = "linux"))]
    if resolve_target {
        return Err(ItemRefusal::Unsupported(
            "bounded physical target resolution requires Linux descriptor custody".into(),
        ));
    }

    #[cfg(target_os = "linux")]
    let resolved = if resolve_target {
        Some(resolve_target_once(
            sources,
            path,
            selected_directory,
            *state_bytes,
            path_observations,
            limits,
            deadline,
            cancelled,
            git_signal,
        )?)
    } else {
        None
    };

    let direct_metadata = match sources.metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if resolve_target && same_resolution_error(&error) => None,
        Err(error) => return Err(route_error(error, deadline, cancelled, git_signal)),
    };
    let target_metadata = resolved.as_ref().and_then(|value| value.metadata.as_ref());
    let metadata = direct_metadata.as_ref().or(target_metadata);
    let content_metadata = if resolved.is_some() {
        target_metadata
    } else {
        direct_metadata.as_ref()
    };

    let outside_selected_root = resolved.as_ref().is_some_and(|value| {
        matches!(
            value.facts,
            PhysicalResolvedTargetFacts::OutsideSelectedRoot
        )
    });
    if outside_selected_root && hash_file {
        return Err(ItemRefusal::Unsupported(
            "selected physical target escapes its held root".into(),
        ));
    }

    if let Some(metadata) = metadata {
        if !metadata.is_file() && !metadata.is_dir() && !metadata.file_type().is_symlink() {
            return Err(ItemRefusal::Source(
                "selected physical path is not a regular file, directory, or symlink".into(),
            ));
        }
    }

    let source_stamp = direct_metadata
        .as_ref()
        .or(target_metadata)
        .map(FileStamp::from_metadata);
    let mut facts = if let Some(metadata) = metadata {
        physical_from_metadata(metadata, None)
    } else if outside_selected_root {
        physical_kind(true, false, true)
    } else {
        physical_kind(false, false, false)
    };

    let should_hash = hash_file && content_metadata.is_some_and(|metadata| metadata.is_file());
    let mut target_hash = None;
    let mut outer_hash = None;
    let mut resolved_stamp = resolved.as_ref().and_then(|value| value.stamp);
    if should_hash {
        let metadata = content_metadata.ok_or(ItemRefusal::Budget)?;
        let target_path = resolved
            .as_ref()
            .and_then(|value| match &value.facts {
                PhysicalResolvedTargetFacts::InsideSelectedRoot {
                    relative_target, ..
                } => Some(relative_target.as_str()),
                _ => None,
            })
            .unwrap_or(path);
        let transient = usize::try_from(metadata.len()).map_err(|_| ItemRefusal::Budget)?;
        let target_workspace =
            resolved_target_dynamic_bytes(resolved.as_ref().map(|observation| &observation.facts))?;
        let metadata_workspace = if resolved.is_some() {
            target_workspace
                .checked_mul(2)
                .and_then(|bytes| bytes.checked_add(128))
                .ok_or(ItemRefusal::Budget)?
        } else {
            64
        };
        let peak = state_bytes
            .checked_add(transient)
            .and_then(|value| value.checked_add(metadata_workspace))
            .and_then(|value| value.checked_add(RESOLVER_WORKSPACE_BYTES))
            .ok_or(ItemRefusal::Budget)?;
        if peak > limits.max_state_bytes {
            return Err(budget(
                "physical-source-state-bytes",
                peak,
                limits.max_state_bytes,
            ));
        }
        let read_result = if let Some(budget) = shared_io_budget {
            let mut hooks = SharedPhysicalReadHooks {
                budget,
                returned_bytes: shared_read_bytes_returned,
                deadline,
                cancelled,
                git_signal,
            };
            sources.bounded_metadata_bytes_with_hooks(
                target_path,
                limits.max_file_bytes,
                bytes_read,
                limits.max_total_bytes,
                &mut hooks,
            )
        } else {
            sources.bounded_metadata_bytes(
                target_path,
                limits.max_file_bytes,
                bytes_read,
                limits.max_total_bytes,
            )
        };
        let (raw, content_metadata) =
            read_result.map_err(|error| route_error(error, deadline, cancelled, git_signal))?;
        let content_stamp = FileStamp::from_metadata(&content_metadata);
        if resolved.is_some() {
            if resolved_stamp != Some(content_stamp) {
                return Err(ItemRefusal::Source(
                    "selected resolved target changed while hashing".into(),
                ));
            }
            let after = resolve_target_once(
                sources,
                path,
                selected_directory,
                *state_bytes,
                path_observations,
                limits,
                deadline,
                cancelled,
                git_signal,
            )?;
            if !same_resolved_target_identity(
                &resolved.as_ref().ok_or(ItemRefusal::Budget)?.facts,
                &after.facts,
            ) || after.stamp != Some(content_stamp)
            {
                return Err(ItemRefusal::Source(
                    "selected resolved target changed during content read".into(),
                ));
            }
            resolved_stamp = after.stamp;
        } else if source_stamp != Some(content_stamp) {
            return Err(ItemRefusal::Source(
                "selected physical path changed while hashing".into(),
            ));
        }
        let digest = Digest256::of_bytes(&raw).to_hex();
        if direct_metadata
            .as_ref()
            .is_none_or(|metadata| !metadata.file_type().is_symlink())
        {
            outer_hash = Some(digest.clone());
        }
        if resolved.is_some() {
            target_hash = Some(digest);
        }
        checkpoint(sources, deadline, cancelled, git_signal)?;
    }

    if let Some(observation) = resolved {
        facts.resolved_target = Some(match observation.facts {
            PhysicalResolvedTargetFacts::InsideSelectedRoot {
                relative_target,
                topology_stamp,
                exists,
                regular_file,
                directory,
                byte_size,
                ..
            } => PhysicalResolvedTargetFacts::InsideSelectedRoot {
                relative_target,
                topology_stamp,
                exists,
                regular_file,
                directory,
                byte_size,
                sha256: target_hash,
                git_tracked: None,
                git_ignored: None,
            },
            outside @ PhysicalResolvedTargetFacts::OutsideSelectedRoot => outside,
            PhysicalResolvedTargetFacts::Unknown => PhysicalResolvedTargetFacts::Unknown,
        });
    }
    facts.sha256 = outer_hash;
    checkpoint(sources, deadline, cancelled, git_signal)?;
    Ok(PathObservation {
        facts,
        stamp: source_stamp,
        resolved_stamp,
    })
}

fn reader_history_checkpoint(deadline: Instant, cancelled: &AtomicBool) -> Result<(), ItemRefusal> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(ItemRefusal::Source(
            "historical physical observation cancelled".into(),
        ));
    }
    if Instant::now() >= deadline {
        return Err(ItemRefusal::Deadline);
    }
    Ok(())
}

pub(crate) fn physical_from_metadata(
    metadata: &fs::Metadata,
    sha256: Option<String>,
) -> PhysicalPathFacts {
    PhysicalPathFacts {
        exists: true,
        regular_file: metadata.is_file(),
        directory: metadata.is_dir(),
        symlink: metadata.file_type().is_symlink(),
        resolved_target: None,
        git_tracked: None,
        git_ignored: None,
        file_mode: file_mode(metadata),
        byte_size: Some(metadata.len()),
        sha256,
    }
}

fn physical_kind(exists: bool, regular_file: bool, symlink: bool) -> PhysicalPathFacts {
    PhysicalPathFacts {
        exists,
        regular_file,
        directory: false,
        symlink,
        resolved_target: None,
        git_tracked: None,
        git_ignored: None,
        file_mode: None,
        byte_size: None,
        sha256: None,
    }
}

#[cfg(target_os = "linux")]
fn file_mode(metadata: &fs::Metadata) -> Option<u32> {
    use std::os::unix::fs::MetadataExt;
    Some(metadata.mode() & 0o7777)
}

#[cfg(not(target_os = "linux"))]
fn file_mode(_: &fs::Metadata) -> Option<u32> {
    None
}

fn same_physical_path(expected: &PhysicalPathFacts, current: &PhysicalPathFacts) -> bool {
    same_physical_metadata(expected, current)
        && expected.sha256 == current.sha256
        && same_resolved_target_hash(
            expected.resolved_target.as_ref(),
            current.resolved_target.as_ref(),
        )
}

fn same_physical_metadata(expected: &PhysicalPathFacts, current: &PhysicalPathFacts) -> bool {
    expected.exists == current.exists
        && expected.regular_file == current.regular_file
        && expected.directory == current.directory
        && expected.symlink == current.symlink
        && expected.file_mode == current.file_mode
        && expected.byte_size == current.byte_size
        && match (
            expected.resolved_target.as_ref(),
            current.resolved_target.as_ref(),
        ) {
            (Some(expected), Some(current)) => same_resolved_target_identity(expected, current),
            (None, None) => true,
            _ => false,
        }
}

fn same_resolved_target_identity(
    expected: &PhysicalResolvedTargetFacts,
    current: &PhysicalResolvedTargetFacts,
) -> bool {
    match (expected, current) {
        (
            PhysicalResolvedTargetFacts::InsideSelectedRoot {
                relative_target: expected_path,
                topology_stamp: expected_topology,
                exists: expected_exists,
                regular_file: expected_file,
                directory: expected_directory,
                byte_size: expected_size,
                ..
            },
            PhysicalResolvedTargetFacts::InsideSelectedRoot {
                relative_target: current_path,
                topology_stamp: current_topology,
                exists: current_exists,
                regular_file: current_file,
                directory: current_directory,
                byte_size: current_size,
                ..
            },
        ) => {
            expected_path == current_path
                && expected_topology == current_topology
                && expected_exists == current_exists
                && expected_file == current_file
                && expected_directory == current_directory
                && expected_size == current_size
        }
        (
            PhysicalResolvedTargetFacts::OutsideSelectedRoot,
            PhysicalResolvedTargetFacts::OutsideSelectedRoot,
        ) => true,
        (PhysicalResolvedTargetFacts::Unknown, PhysicalResolvedTargetFacts::Unknown) => true,
        _ => false,
    }
}

fn same_resolved_target_hash(
    expected: Option<&PhysicalResolvedTargetFacts>,
    current: Option<&PhysicalResolvedTargetFacts>,
) -> bool {
    match (expected, current) {
        (
            Some(PhysicalResolvedTargetFacts::InsideSelectedRoot {
                sha256: expected_hash,
                ..
            }),
            Some(PhysicalResolvedTargetFacts::InsideSelectedRoot {
                sha256: current_hash,
                ..
            }),
        ) => expected_hash == current_hash,
        (
            Some(PhysicalResolvedTargetFacts::OutsideSelectedRoot),
            Some(PhysicalResolvedTargetFacts::OutsideSelectedRoot),
        )
        | (
            Some(PhysicalResolvedTargetFacts::Unknown),
            Some(PhysicalResolvedTargetFacts::Unknown),
        )
        | (None, None) => true,
        _ => false,
    }
}

fn physical_dynamic_bytes(facts: &PhysicalPathFacts) -> Result<usize, ItemRefusal> {
    let outer = facts.sha256.as_ref().map_or(0, String::len);
    let target = match facts.resolved_target.as_ref() {
        Some(PhysicalResolvedTargetFacts::InsideSelectedRoot {
            relative_target,
            topology_stamp,
            sha256,
            ..
        }) => relative_target
            .len()
            .checked_add(topology_stamp.len())
            .and_then(|bytes| bytes.checked_add(sha256.as_ref().map_or(0, String::len)))
            .ok_or(ItemRefusal::Budget)?,
        _ => 0,
    };
    outer.checked_add(target).ok_or(ItemRefusal::Budget)
}

#[cfg(target_os = "linux")]
fn same_resolution_error(error: &io::Error) -> bool {
    matches!(
        error.raw_os_error(),
        Some(code) if code == rustix::io::Errno::LOOP.raw_os_error()
            || code == rustix::io::Errno::NOTDIR.raw_os_error()
    )
}

#[cfg(not(target_os = "linux"))]
fn same_resolution_error(_: &io::Error) -> bool {
    false
}

#[cfg(target_os = "linux")]
fn resolve_target_once(
    sources: &mut RouteSources,
    path: &str,
    selected_directory: Option<&str>,
    state_bytes: usize,
    path_observations: &mut usize,
    limits: PhysicalSourceLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    git_signal: &AtomicI32,
) -> Result<ResolvedObservation, ItemRefusal> {
    let peak = state_bytes
        .checked_add(RESOLVER_WORKSPACE_BYTES)
        .ok_or(ItemRefusal::Budget)?;
    if peak > limits.max_state_bytes {
        return Err(budget(
            "physical-source-resolver-workspace",
            peak,
            limits.max_state_bytes,
        ));
    }
    charge_path_observation(path_observations, limits.max_path_observations)?;
    checkpoint(sources, deadline, cancelled, git_signal)?;
    let resolved = sources
        .resolve_selected_target(path, selected_directory)
        .map_err(|error| route_error(error, deadline, cancelled, git_signal))?;
    checkpoint(sources, deadline, cancelled, git_signal)?;
    match resolved {
        RouteResolvedTarget::OutsideSelectedRoot => Ok(ResolvedObservation {
            facts: PhysicalResolvedTargetFacts::OutsideSelectedRoot,
            metadata: None,
            stamp: None,
        }),
        RouteResolvedTarget::Inside {
            relative_path,
            metadata,
            topology_stamp,
        } => {
            if relative_path.is_empty()
                || relative_path.len() > MAX_PATH_BYTES
                || relative_path.contains('\0')
                || relative_path.contains('\n')
                || relative_path.contains('\r')
            {
                return Err(ItemRefusal::Unsupported(
                    "resolved physical relative path".into(),
                ));
            }
            if let Some(metadata) = metadata.as_ref() {
                if !metadata.is_file() && !metadata.is_dir() {
                    return Err(ItemRefusal::Source(
                        "resolved physical target is not a regular file or directory".into(),
                    ));
                }
            }
            let exists = metadata.is_some();
            let regular_file = metadata.as_ref().is_some_and(fs::Metadata::is_file);
            let directory = metadata.as_ref().is_some_and(fs::Metadata::is_dir);
            let byte_size = metadata.as_ref().map(fs::Metadata::len);
            Ok(ResolvedObservation {
                facts: PhysicalResolvedTargetFacts::InsideSelectedRoot {
                    relative_target: relative_path,
                    topology_stamp: topology_stamp.to_hex(),
                    exists,
                    regular_file,
                    directory,
                    byte_size,
                    sha256: None,
                    git_tracked: None,
                    git_ignored: None,
                },
                stamp: metadata.as_ref().map(FileStamp::from_metadata),
                metadata,
            })
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn resolve_target_once(
    _: &mut RouteSources,
    _: &str,
    _: Option<&str>,
    _: usize,
    _: &mut usize,
    _: PhysicalSourceLimits,
    _: Instant,
    _: &AtomicBool,
    _: &AtomicI32,
) -> Result<ResolvedObservation, ItemRefusal> {
    Err(ItemRefusal::Unsupported(
        "bounded physical target resolution requires Linux descriptor custody".into(),
    ))
}

fn collect_artifact_paths(
    paths: &[String],
    limits: PhysicalSourceLimits,
) -> Result<BTreeSet<String>, ItemRefusal> {
    let mut unique = BTreeSet::new();
    let mut total_bytes = 0usize;
    for path in paths {
        validate_relative_path(path)?;
        if unique.insert(path.clone()) {
            total_bytes = total_bytes
                .checked_add(path.len() + std::mem::size_of::<String>() + MAP_NODE_OVERHEAD)
                .ok_or(ItemRefusal::Budget)?;
        }
    }
    if unique.len() > limits.max_paths {
        return Err(budget(
            "physical-source-distinct-paths",
            unique.len(),
            limits.max_paths,
        ));
    }
    if total_bytes > limits.max_state_bytes {
        return Err(budget(
            "physical-source-state-bytes",
            total_bytes,
            limits.max_state_bytes,
        ));
    }
    Ok(unique)
}

#[allow(clippy::too_many_arguments)]
fn preflight_final_cost(
    facts: &SourcePhysicalFacts,
    authored_capture_paths: &BTreeSet<String>,
    private_declared_paths: &BTreeSet<String>,
    private_prefixes: &[String],
    inventory_entry_counts: &BTreeMap<String, usize>,
    artifact_paths: &BTreeSet<String>,
    artifact_provider_selected: bool,
    initial_observations: usize,
    initial_bytes: usize,
    git_path_count: usize,
    git_root_present: bool,
    retained_state_bytes: usize,
    limits: PhysicalSourceLimits,
) -> Result<(), ItemRefusal> {
    let known_inventory_entries = private_prefixes.iter().try_fold(0usize, |sum, prefix| {
        let count = facts
            .private_paths
            .iter()
            .filter(|(path, physical)| {
                path.as_str() != prefix.as_str() && path_within(path, prefix) && physical.exists
            })
            .count();
        sum.checked_add(count).ok_or(ItemRefusal::Budget)
    })?;
    let base_final_observations = facts
        .authored_paths
        .len()
        .checked_add(private_declared_paths.len())
        .and_then(|count| count.checked_add(private_prefixes.len()))
        .and_then(|count| count.checked_add(known_inventory_entries))
        .and_then(|count| {
            count.checked_add(if artifact_provider_selected {
                artifact_paths.len()
            } else {
                0
            })
        })
        .ok_or(ItemRefusal::Budget)?;
    let mut resolution_observations = 0usize;
    for (path, physical) in &facts.authored_paths {
        if !authored_capture_paths.contains(path) {
            resolution_observations = resolution_observations
                .checked_add(resolved_final_calls(physical))
                .ok_or(ItemRefusal::Budget)?;
        } else if matches!(
            physical.resolved_target.as_ref(),
            Some(PhysicalResolvedTargetFacts::InsideSelectedRoot { .. })
        ) {
            resolution_observations = resolution_observations
                .checked_add(resolved_final_calls(physical))
                .ok_or(ItemRefusal::Budget)?;
        }
    }
    for path in private_declared_paths {
        if let Some(physical) = facts.private_paths.get(path) {
            resolution_observations = resolution_observations
                .checked_add(resolved_final_calls(physical))
                .ok_or(ItemRefusal::Budget)?;
        }
    }
    for prefix in private_prefixes {
        for (path, physical) in &facts.private_paths {
            if physical.exists && path != prefix && path_within(path, prefix) {
                resolution_observations = resolution_observations
                    .checked_add(resolved_final_calls(physical))
                    .ok_or(ItemRefusal::Budget)?;
            }
        }
    }
    if artifact_provider_selected {
        for path in artifact_paths {
            if let Some(physical) = facts.artifact_paths.get(path) {
                resolution_observations = resolution_observations
                    .checked_add(resolved_final_calls(physical))
                    .ok_or(ItemRefusal::Budget)?;
            }
        }
    }
    let final_observations = base_final_observations
        .checked_add(resolution_observations)
        .ok_or(ItemRefusal::Budget)?;
    let total_observations = initial_observations
        .checked_add(final_observations)
        .ok_or(ItemRefusal::Budget)?;
    if total_observations > limits.max_path_observations {
        return Err(budget(
            "physical-source-path-observations",
            total_observations,
            limits.max_path_observations,
        ));
    }

    let mut final_bytes = 0usize;
    for (path, physical) in &facts.authored_paths {
        if !authored_capture_paths.contains(path) {
            if let Some(size) = content_read_size(physical) {
                final_bytes = checked_add_file_bytes(final_bytes, Some(size))?;
            }
        }
    }
    for path in private_declared_paths {
        if let Some(physical) = facts.private_paths.get(path) {
            if let Some(size) = content_read_size(physical) {
                final_bytes = checked_add_file_bytes(final_bytes, Some(size))?;
            }
        }
    }
    if artifact_provider_selected {
        for path in artifact_paths {
            if let Some(physical) = facts.artifact_paths.get(path) {
                if let Some(size) = content_read_size(physical) {
                    final_bytes = checked_add_file_bytes(final_bytes, Some(size))?;
                }
            }
        }
    }
    let total_bytes = initial_bytes
        .checked_add(final_bytes)
        .ok_or(ItemRefusal::Budget)?;
    if total_bytes > limits.max_total_bytes {
        return Err(budget(
            "physical-source-aggregate-bytes",
            total_bytes,
            limits.max_total_bytes,
        ));
    }

    let mut max_hash_workspace = 0usize;
    for (path, physical) in &facts.authored_paths {
        if !authored_capture_paths.contains(path) {
            if let Some(size) = content_read_size(physical) {
                max_hash_workspace = max_hash_workspace.max(hash_workspace(physical, size)?);
            }
        }
    }
    for path in private_declared_paths {
        if let Some(physical) = facts.private_paths.get(path) {
            if let Some(size) = content_read_size(physical) {
                max_hash_workspace = max_hash_workspace.max(hash_workspace(physical, size)?);
            }
        }
    }
    if artifact_provider_selected {
        for path in artifact_paths {
            if let Some(physical) = facts.artifact_paths.get(path) {
                if let Some(size) = content_read_size(physical) {
                    max_hash_workspace = max_hash_workspace.max(hash_workspace(physical, size)?);
                }
            }
        }
    }
    let mut max_inventory_workspace = 0usize;
    for prefix in private_prefixes
        .iter()
        .filter(|prefix| facts.private_inventories.contains_key(*prefix))
    {
        let mut workspace = prefix
            .len()
            .checked_add(std::mem::size_of::<String>())
            .ok_or(ItemRefusal::Budget)?;
        for (path, physical) in &facts.private_paths {
            if physical.exists && path != prefix && path_within(path, prefix) {
                workspace = workspace
                    .checked_add(transient_walk_cost(path))
                    .ok_or(ItemRefusal::Budget)?;
            }
        }
        let workspace = if inventory_entry_counts
            .get(prefix)
            .copied()
            .unwrap_or_default()
            > 0
        {
            workspace
                .checked_add(RESOLVER_WORKSPACE_BYTES)
                .ok_or(ItemRefusal::Budget)?
        } else {
            workspace
        };
        max_inventory_workspace = max_inventory_workspace.max(workspace);
    }
    let resolver_workspace = if resolution_observations > 0 {
        RESOLVER_WORKSPACE_BYTES
    } else {
        0
    };
    let final_state_peak = retained_state_bytes
        .checked_add(
            max_hash_workspace
                .max(max_inventory_workspace)
                .max(resolver_workspace)
                .max(limits.git_output_bytes.saturating_mul(2)),
        )
        .ok_or(ItemRefusal::Budget)?;
    if final_state_peak > limits.max_state_bytes {
        return Err(budget(
            "physical-source-final-state-bytes",
            final_state_peak,
            limits.max_state_bytes,
        ));
    }

    if git_root_present {
        let total_queries = git_path_count.checked_mul(2).ok_or(ItemRefusal::Budget)?;
        if total_queries > limits.max_git_path_queries {
            return Err(budget(
                "physical-source-git-path-queries",
                total_queries,
                limits.max_git_path_queries,
            ));
        }
    }
    Ok(())
}

fn checked_add_file_bytes(total: usize, bytes: Option<u64>) -> Result<usize, ItemRefusal> {
    total
        .checked_add(
            usize::try_from(bytes.ok_or(ItemRefusal::Budget)?).map_err(|_| ItemRefusal::Budget)?,
        )
        .ok_or(ItemRefusal::Budget)
}

fn resolved_final_calls(facts: &PhysicalPathFacts) -> usize {
    match facts.resolved_target.as_ref() {
        Some(PhysicalResolvedTargetFacts::InsideSelectedRoot { sha256, .. }) => {
            1 + usize::from(sha256.is_some())
        }
        Some(PhysicalResolvedTargetFacts::OutsideSelectedRoot) => 1,
        Some(PhysicalResolvedTargetFacts::Unknown) | None => 0,
    }
}

fn content_read_size(facts: &PhysicalPathFacts) -> Option<u64> {
    match facts.resolved_target.as_ref() {
        Some(PhysicalResolvedTargetFacts::InsideSelectedRoot {
            sha256: Some(_),
            byte_size,
            ..
        }) => *byte_size,
        _ if facts.sha256.is_some() => facts.byte_size,
        _ => None,
    }
}

fn resolved_target_dynamic_bytes(
    target: Option<&PhysicalResolvedTargetFacts>,
) -> Result<usize, ItemRefusal> {
    match target {
        Some(PhysicalResolvedTargetFacts::InsideSelectedRoot {
            relative_target,
            topology_stamp,
            sha256,
            ..
        }) => relative_target
            .len()
            .checked_add(topology_stamp.len())
            .and_then(|bytes| bytes.checked_add(sha256.as_ref().map_or(0, String::len)))
            .ok_or(ItemRefusal::Budget),
        _ => Ok(0),
    }
}

fn hash_workspace(facts: &PhysicalPathFacts, size: u64) -> Result<usize, ItemRefusal> {
    let size = usize::try_from(size).map_err(|_| ItemRefusal::Budget)?;
    let target_dynamic = resolved_target_dynamic_bytes(facts.resolved_target.as_ref())?;
    let metadata_workspace = if target_dynamic == 0 {
        64
    } else {
        target_dynamic
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(128 + RESOLVER_WORKSPACE_BYTES))
            .ok_or(ItemRefusal::Budget)?
    };
    size.checked_add(metadata_workspace)
        .ok_or(ItemRefusal::Budget)
}

fn validate_limits(limits: PhysicalSourceLimits) -> Result<(), ItemRefusal> {
    if limits.max_paths == 0
        || limits.max_path_observations == 0
        || limits.max_file_bytes == 0
        || limits.max_total_bytes == 0
        || limits.max_state_bytes < std::mem::size_of::<SourcePhysicalFacts>()
        || limits.git_output_bytes == 0
        || limits.git_output_bytes > 65_536
        || limits.git_cleanup_grace.is_zero()
    {
        return Err(ItemRefusal::Budget);
    }
    Ok(())
}

fn validate_repo_path(path: &str) -> Result<(), ItemRefusal> {
    validate_relative_path(path)?;
    if !path.starts_with("ToS/") {
        return Err(ItemRefusal::Unsupported("physical source path".into()));
    }
    RelativePath::parse(path)
        .map(|_| ())
        .map_err(|_| ItemRefusal::Unsupported("physical source path".into()))
}

fn validate_relative_path(path: &str) -> Result<(), ItemRefusal> {
    if path.is_empty()
        || path.len() > MAX_PATH_BYTES
        || path.contains('\0')
        || path.contains('\n')
        || path.contains('\r')
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        || Path::new(path).components().count() > MAX_PATH_COMPONENTS
        || Path::new(path)
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ItemRefusal::Unsupported("physical relative path".into()));
    }
    Ok(())
}

fn paths_overlap(a: &str, b: &str) -> bool {
    a == b
        || b.strip_prefix(a).is_some_and(|tail| tail.starts_with('/'))
        || a.strip_prefix(b).is_some_and(|tail| tail.starts_with('/'))
}

fn path_within(path: &str, root: &str) -> bool {
    path == root
        || (root != "."
            && path
                .strip_prefix(root)
                .is_some_and(|tail| tail.starts_with('/')))
}

fn budget(check: &'static str, used: usize, limit: usize) -> ItemRefusal {
    ItemRefusal::BudgetCheck {
        check,
        used: Some(used as u64),
        limit: Some(limit as u64),
    }
}

fn reserve_state(used: &mut usize, additional: usize, limit: usize) -> Result<(), ItemRefusal> {
    let next = used
        .checked_add(additional)
        .ok_or(ItemRefusal::BudgetCheck {
            check: "physical-source-state-bytes",
            used: None,
            limit: Some(limit as u64),
        })?;
    if next > limit {
        return Err(budget("physical-source-state-bytes", next, limit));
    }
    *used = next;
    Ok(())
}

fn charge_path_value(path: &str, state: &mut usize, limit: usize) -> Result<(), ItemRefusal> {
    let bytes = path
        .len()
        .checked_add(std::mem::size_of::<String>())
        .and_then(|value| value.checked_add(MAP_NODE_OVERHEAD))
        .ok_or(ItemRefusal::Budget)?;
    reserve_state(state, bytes, limit)
}

fn charge_map_entry(
    path: &str,
    value_bytes: usize,
    dynamic_bytes: usize,
    state: &mut usize,
    limit: usize,
) -> Result<(), ItemRefusal> {
    let bytes = path
        .len()
        .checked_add(std::mem::size_of::<String>())
        .and_then(|value| value.checked_add(value_bytes))
        .and_then(|value| value.checked_add(dynamic_bytes))
        .and_then(|value| value.checked_add(MAP_NODE_OVERHEAD))
        .ok_or(ItemRefusal::Budget)?;
    reserve_state(state, bytes, limit)
}

fn insert_path(
    set: &mut BTreeSet<String>,
    path: &str,
    other_root_count: usize,
    state: &mut usize,
    limits: PhysicalSourceLimits,
) -> Result<bool, ItemRefusal> {
    if set.contains(path) {
        return Ok(false);
    }
    let next_count = set
        .len()
        .checked_add(other_root_count)
        .and_then(|count| count.checked_add(1))
        .ok_or(ItemRefusal::Budget)?;
    if next_count > limits.max_paths {
        return Err(budget(
            "physical-source-distinct-paths",
            next_count,
            limits.max_paths,
        ));
    }
    charge_path_value(path, state, limits.max_state_bytes)?;
    set.insert(path.to_owned());
    Ok(true)
}

fn charge_payload_map(
    payloads: &BTreeMap<String, PhysicalPayloadFacts>,
    state: &mut usize,
    limit: usize,
) -> Result<(), ItemRefusal> {
    for (path, facts) in payloads {
        let dynamic = facts
            .sha256
            .as_ref()
            .map_or(0, String::len)
            .checked_add(facts.sha1.as_ref().map_or(0, String::len))
            .ok_or(ItemRefusal::Budget)?;
        charge_map_entry(
            path,
            std::mem::size_of::<PhysicalPayloadFacts>(),
            dynamic,
            state,
            limit,
        )?;
    }
    Ok(())
}

fn put_private_fact(
    map: &mut BTreeMap<String, PhysicalPathFacts>,
    path: &str,
    facts: PhysicalPathFacts,
    state: &mut usize,
    limit: usize,
) -> Result<(), ItemRefusal> {
    if let Some(existing) = map.get_mut(path) {
        if !same_physical_metadata(existing, &facts) {
            return Err(ItemRefusal::Source(
                "private path observations disagree".into(),
            ));
        }
        // An explicit private path carries a content hash; inventory-only
        // metadata may enrich only fields absent from that exact snapshot.
        if existing.sha256.is_none() {
            existing.file_mode = facts.file_mode;
            existing.byte_size = facts.byte_size;
        }
        return Ok(());
    }
    charge_map_entry(
        path,
        std::mem::size_of::<PhysicalPathFacts>(),
        physical_dynamic_bytes(&facts)?,
        state,
        limit,
    )?;
    map.insert(path.to_owned(), facts);
    Ok(())
}

fn insert_inventory(
    map: &mut BTreeMap<String, Vec<String>>,
    prefix: &str,
    files: Vec<String>,
    state: &mut usize,
    limit: usize,
) -> Result<(), ItemRefusal> {
    charge_map_entry(prefix, std::mem::size_of::<Vec<String>>(), 0, state, limit)?;
    if map.insert(prefix.to_owned(), files).is_some() {
        return Err(ItemRefusal::Source(
            "duplicate private inventory prefix".into(),
        ));
    }
    Ok(())
}

fn put_inventory_count(
    map: &mut BTreeMap<String, usize>,
    prefix: &str,
    count: usize,
    state: &mut usize,
    limit: usize,
) -> Result<(), ItemRefusal> {
    charge_map_entry(prefix, std::mem::size_of::<usize>(), 0, state, limit)?;
    if map.insert(prefix.to_owned(), count).is_some() {
        return Err(ItemRefusal::Source(
            "duplicate private inventory prefix count".into(),
        ));
    }
    Ok(())
}

fn charge_vec_slot(state: &mut usize, limit: usize) -> Result<(), ItemRefusal> {
    reserve_state(state, std::mem::size_of::<String>(), limit)
}

fn transient_walk_cost(path: &str) -> usize {
    path.len()
        .saturating_add(std::mem::size_of::<String>())
        .saturating_add(MAP_NODE_OVERHEAD)
        .saturating_mul(3)
}

fn charge_path_observation(used: &mut usize, limit: usize) -> Result<(), ItemRefusal> {
    let next = used.checked_add(1).ok_or(ItemRefusal::Budget)?;
    if next > limit {
        return Err(budget("physical-source-path-observations", next, limit));
    }
    *used = next;
    Ok(())
}

fn checkpoint(
    sources: &RouteSources,
    deadline: Instant,
    cancelled: &AtomicBool,
    git_signal: &AtomicI32,
) -> Result<(), ItemRefusal> {
    if cancelled.load(Ordering::Relaxed) || git_signal.load(Ordering::Relaxed) != 0 {
        return Err(ItemRefusal::Source(
            "physical source operation cancelled".into(),
        ));
    }
    if Instant::now() >= deadline {
        return Err(ItemRefusal::Deadline);
    }
    sources
        .check()
        .map_err(|error| route_error(error, deadline, cancelled, git_signal))
}

fn physical_read_checkpoint(
    deadline: Instant,
    cancelled: &AtomicBool,
    git_signal: &AtomicI32,
) -> io::Result<()> {
    if cancelled.load(Ordering::Relaxed) || git_signal.load(Ordering::Relaxed) != 0 {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "physical source operation cancelled",
        ));
    }
    if Instant::now() >= deadline {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "physical source operation deadline exceeded",
        ));
    }
    Ok(())
}

fn route_error(
    error: io::Error,
    deadline: Instant,
    cancelled: &AtomicBool,
    git_signal: &AtomicI32,
) -> ItemRefusal {
    if cancelled.load(Ordering::Relaxed) || git_signal.load(Ordering::Relaxed) != 0 {
        return ItemRefusal::Source("physical source operation cancelled".into());
    }
    if Instant::now() >= deadline || error.kind() == io::ErrorKind::TimedOut {
        return ItemRefusal::Deadline;
    }
    if error.kind() == io::ErrorKind::Unsupported {
        return ItemRefusal::Unsupported("physical source operation unsupported".into());
    }
    if error.kind() == io::ErrorKind::InvalidData {
        match error.to_string().as_str() {
            value @ ("route lookup operation bound exceeded"
            | "route discovery entry bound exceeded"
            | "operand aggregate byte accounting exceeded"
            | "operand input byte bound exceeded"
            | "operand aggregate byte accounting overflow"
            | "shared physical returned byte accounting overflow"
            | "foundation Git output bound"
            | "foundation Git FD census bound"
            | "foundation Git status bound") => {
                // Preserve the fixed owner check through bootstrap/finalizers.
                // Paths and arbitrary IO text remain outside this public code.
                let check = match value {
                    "route lookup operation bound exceeded" => "physical-route-lookup-operations",
                    "route discovery entry bound exceeded" => "physical-route-discovery-entries",
                    "operand aggregate byte accounting exceeded" => {
                        "physical-route-total-read-bytes"
                    }
                    "operand input byte bound exceeded" => "physical-route-member-read-bytes",
                    "operand aggregate byte accounting overflow" => {
                        "physical-route-read-counter-overflow"
                    }
                    "shared physical returned byte accounting overflow" => {
                        "physical-route-shared-return-overflow"
                    }
                    "foundation Git output bound" => "physical-git-output-bytes",
                    "foundation Git FD census bound" => "physical-git-fd-census",
                    _ => "physical-git-status",
                };
                return ItemRefusal::BudgetCheck {
                    check,
                    used: None,
                    limit: None,
                };
            }
            value if value.starts_with("shared physical read budget refused:") => {
                return ItemRefusal::BudgetCheck {
                    check: "physical-route-shared-read-budget",
                    used: None,
                    limit: None,
                };
            }
            "foundation Git deadline" | "route operation deadline exceeded" => {
                return ItemRefusal::Deadline;
            }
            _ => (),
        }
    }
    ItemRefusal::Source(format!("selected physical custody: {error}"))
}

fn inventory_unavailable(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::PermissionDenied
        || error.kind() == io::ErrorKind::NotFound
        || error.raw_os_error() == Some(rustix::io::Errno::LOOP.raw_os_error())
        || error.to_string() == "route discovery refuses symlinks"
}

fn check_context(deadline: Instant, cancelled: &AtomicBool, git_signal: &AtomicI32) -> ItemRefusal {
    if cancelled.load(Ordering::Relaxed) || git_signal.load(Ordering::Relaxed) != 0 {
        ItemRefusal::Source("physical source operation cancelled".into())
    } else if Instant::now() >= deadline {
        ItemRefusal::Deadline
    } else {
        ItemRefusal::Source("physical source operation interrupted".into())
    }
}

fn git_root_present(
    sources: &mut RouteSources,
    deadline: Instant,
    cancelled: &AtomicBool,
    git_signal: &AtomicI32,
) -> Result<bool, ItemRefusal> {
    checkpoint(sources, deadline, cancelled, git_signal)?;
    let present = sources
        .exists(".git")
        .map_err(|error| route_error(error, deadline, cancelled, git_signal))?;
    checkpoint(sources, deadline, cancelled, git_signal)?;
    Ok(present)
}

fn query_git_paths<F>(
    sources: &mut RouteSources,
    paths: &BTreeSet<String>,
    git_root_present: bool,
    queries: &mut usize,
    limits: PhysicalSourceLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    git_signal: &AtomicI32,
    mut consume: F,
) -> Result<Option<bool>, ItemRefusal>
where
    F: FnMut(&str, Option<bool>, Option<bool>) -> Result<(), ItemRefusal>,
{
    if !git_root_present {
        for path in paths {
            consume(path, None, None)?;
        }
        return Ok(Some(false));
    }
    let next_queries = queries
        .checked_add(paths.len())
        .ok_or(ItemRefusal::Budget)?;
    if next_queries > limits.max_git_path_queries {
        return Err(budget(
            "physical-source-git-path-queries",
            next_queries,
            limits.max_git_path_queries,
        ));
    }
    let mut any_observed = false;
    for path in paths {
        checkpoint(sources, deadline, cancelled, git_signal)?;
        *queries = queries.checked_add(1).ok_or(ItemRefusal::Budget)?;
        let (tracked, ignored) = sources
            .foundation_git_path_facts(
                path,
                limits.git_output_bytes,
                limits.git_cleanup_grace,
                git_signal,
            )
            .map_err(|error| route_error(error, deadline, cancelled, git_signal))?;
        checkpoint(sources, deadline, cancelled, git_signal)?;
        any_observed |= tracked.is_some() || ignored.is_some();
        consume(path, tracked, ignored)?;
    }
    Ok(any_observed.then_some(true))
}

fn apply_git_facts(
    facts: &mut SourcePhysicalFacts,
    path: &str,
    tracked: Option<bool>,
    ignored: Option<bool>,
) -> Result<(), ItemRefusal> {
    if let Some(value) = facts.authored_git.get_mut(path) {
        value.tracked = tracked;
        value.ignored = ignored;
    }
    if let Some(value) = facts.payloads.get_mut(path) {
        value.git_tracked = tracked;
        value.git_ignored = ignored;
    }
    if let Some(value) = facts.authored_paths.get_mut(path) {
        value.git_tracked = tracked;
        value.git_ignored = ignored;
    }
    if let Some(value) = facts.private_paths.get_mut(path) {
        value.git_tracked = tracked;
        value.git_ignored = ignored;
    }
    for value in facts
        .authored_paths
        .values_mut()
        .chain(facts.private_paths.values_mut())
    {
        if let Some(PhysicalResolvedTargetFacts::InsideSelectedRoot {
            relative_target,
            git_tracked,
            git_ignored,
            ..
        }) = value.resolved_target.as_mut()
        {
            if relative_target == path {
                *git_tracked = tracked;
                *git_ignored = ignored;
            }
        }
    }
    Ok(())
}

fn expected_git_values(
    facts: &SourcePhysicalFacts,
    path: &str,
) -> Option<(Option<bool>, Option<bool>)> {
    facts
        .authored_git
        .get(path)
        .map(|value| (value.tracked, value.ignored))
        .or_else(|| {
            facts
                .payloads
                .get(path)
                .map(|value| (value.git_tracked, value.git_ignored))
        })
        .or_else(|| {
            facts
                .authored_paths
                .get(path)
                .map(|value| (value.git_tracked, value.git_ignored))
        })
        .or_else(|| {
            facts
                .private_paths
                .get(path)
                .map(|value| (value.git_tracked, value.git_ignored))
        })
        .or_else(|| {
            facts
                .authored_paths
                .values()
                .chain(facts.private_paths.values())
                .find_map(|value| match value.resolved_target.as_ref() {
                    Some(PhysicalResolvedTargetFacts::InsideSelectedRoot {
                        relative_target,
                        git_tracked,
                        git_ignored,
                        ..
                    }) if relative_target == path => Some((*git_tracked, *git_ignored)),
                    _ => None,
                })
        })
}
