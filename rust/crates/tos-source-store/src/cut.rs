//! Complete traversal of the existing immutable source-corpus carrier.
//!
//! Manifest membership and index claims are authenticated by the exact revision
//! digest, not by a new source registry. This reader grants no source admission,
//! current rights, semantic assessment, or authority to publish retained bytes.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use tos_foundation::{Digest256, Digest256Hasher, RelativePath, SourceRevision};

use crate::{
    CorpusDescriptor, CorpusReader, MemberMetadata, Result, RetirementMetadata, Snapshot,
    StoreError, StoreErrorCode,
};

#[derive(Clone, Copy, Debug)]
pub struct CutReadLimits {
    pub max_revisions: usize,
    pub max_members: u64,
    pub max_total_bytes: u64,
    pub max_member_bytes: u64,
}

#[derive(Debug)]
pub struct SourceMemberV1 {
    pub path: RelativePath,
    pub raw: Vec<u8>,
    pub revision: SourceRevision,
    /// Exact manifest index claims; the source rule must verify their meaning.
    pub stable_ids: Vec<String>,
}

#[derive(Debug)]
pub struct RetiredSourceMemberV1 {
    pub revision: SourceRevision,
    pub metadata: RetirementMetadata,
    pub raw: Vec<u8>,
    pub event_raw: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceMembershipV1 {
    pub count: u64,
    pub digest: Digest256,
}

/// Versioned full-membership commitment for an authenticated V2 member tree.
/// It commits the selected tree and aggregate size without claiming the
/// canonical ordered-row digest retained by V1.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceMembershipV2 {
    pub count: u64,
    pub source_bytes: u64,
    pub members_tree_commitment: Digest256,
    pub commitment: Digest256,
}

impl SourceMembershipV2 {
    pub fn from_members_tree(
        count: u64,
        source_bytes: u64,
        members_tree_commitment: Digest256,
    ) -> Self {
        let mut hasher = Digest256Hasher::new();
        hasher.update(b"tos-val-full-membership-v2\0");
        hasher.update(&count.to_be_bytes());
        hasher.update(&source_bytes.to_be_bytes());
        hasher.update(members_tree_commitment.as_bytes());
        Self {
            count,
            source_bytes,
            members_tree_commitment,
            commitment: hasher.finalize(),
        }
    }
}

/// Physical source format selected by the caller. The V2 form retains its
/// authenticated member-tree commitment separately from the V1 semantic
/// membership digest; those digests describe different layers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceCutFormat {
    CorpusSnapshotV1,
    NativeAdmissionV2,
}

/// Exact current selection that anchors a reusable source-read session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceCutSelection {
    pub format: SourceCutFormat,
    pub current_revision: SourceRevision,
    pub rootset_sha256: Option<Digest256>,
}

/// Typed proof target for one ordered member stream. `row_digest` is the
/// source membership digest over exact path, length and content-digest rows.
/// V2 additionally carries the authenticated tree commitment that binds its
/// membership index. A V2 tree commitment is never treated as `row_digest`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceCutMembership {
    CorpusSnapshotV1(SourceMembershipV1),
    NativeAdmissionV2 {
        rows: SourceMembershipV1,
        members_tree_commitment: Digest256,
    },
}

impl SourceCutMembership {
    pub fn rows(self) -> SourceMembershipV1 {
        match self {
            Self::CorpusSnapshotV1(rows) | Self::NativeAdmissionV2 { rows, .. } => rows,
        }
    }

    pub fn format(self) -> SourceCutFormat {
        match self {
            Self::CorpusSnapshotV1(_) => SourceCutFormat::CorpusSnapshotV1,
            Self::NativeAdmissionV2 { .. } => SourceCutFormat::NativeAdmissionV2,
        }
    }
}

/// Bounded locator for one member of the exact retained source chain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceCutRevision {
    pub format: SourceCutFormat,
    pub revision: SourceRevision,
    pub base_revision: Option<SourceRevision>,
    pub validator_sha256: Digest256,
    pub member_count: u64,
    /// Authenticated sum of the selected source member sizes. A V2 caller
    /// obtains this from the exact revision root rather than scanning all
    /// member rows merely to preflight a bounded successor.
    pub source_bytes: u64,
    pub identity_count: u64,
    pub dependency_source_count: u64,
    pub dependency_count: u64,
    pub retirement_count: u64,
    pub membership: SourceCutMembership,
}

/// One addressed physical-path predicate retained by a selected operation.
/// `expected_presence` includes absence and materialized-directory witnesses;
/// a file witness also binds its exact digest, size, and source mode.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceCutMemberWitness {
    pub path: RelativePath,
    pub expected_presence: Option<SourcePresenceV1>,
    pub expected_member: Option<SourceCutMemberTuple>,
    /// `None` means no selected file was present; `Some([])` is the exact
    /// authenticated inverse-index result for a present file with no IDs.
    pub expected_indexed_ids: Option<Vec<String>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceCutMemberTuple {
    pub sha256: Digest256,
    pub size_bytes: u64,
    pub mode: u32,
}

/// A uniqueness or absence predicate for one exact indexed identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceCutIdentityWitness {
    pub id: String,
    pub expected_path: Option<RelativePath>,
}

/// Exact immediate-child set used by a source selector such as record-home
/// history/forms discovery. Comparing only the selected child would miss a
/// newly added sibling that changes the owner's selection predicate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceCutDirectoryWitness {
    pub path: RelativePath,
    /// Distinguishes an absent namespace from an existing materialized
    /// directory. Empty directories are generally not representable in the
    /// snapshot membership format, but callers must preserve that distinction
    /// if a source transport exposes it.
    pub expected_presence: Option<SourcePresenceV1>,
    pub expected_children: Vec<(String, bool)>,
}

/// Exact authenticated outgoing dependency set consulted by an owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceCutDependencyWitness {
    pub path: RelativePath,
    pub expected_targets: Vec<RelativePath>,
}

/// Observed authenticated object-tree reference count for one exact digest.
/// Absence is `None`; a present count must be nonzero. This is read evidence,
/// not object custody, admission, or an authorization to publish a successor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceCutObjectRefcountWitness {
    pub digest: Digest256,
    pub expected_count: Option<u64>,
}

/// The exact finite source predicates observed by one V2 owner operation.
/// The caller retains this value for currentness checks and, when a transaction
/// requires recovery, the owning publisher persists this same logical witness
/// set with its transaction evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceCutReadsetV1 {
    pub base_revision: SourceRevision,
    pub members: Vec<SourceCutMemberWitness>,
    pub identities: Vec<SourceCutIdentityWitness>,
    pub directories: Vec<SourceCutDirectoryWitness>,
    pub dependencies: Vec<SourceCutDependencyWitness>,
    pub object_refcounts: Vec<SourceCutObjectRefcountWitness>,
}

impl SourceCutReadsetV1 {
    pub fn new(base_revision: SourceRevision) -> Self {
        Self {
            base_revision,
            members: Vec::new(),
            identities: Vec::new(),
            directories: Vec::new(),
            dependencies: Vec::new(),
            object_refcounts: Vec::new(),
        }
    }

    /// Upper bound for the logical witness allocations retained by the
    /// caller. Owner adapters include any enclosing wrapper and transient
    /// output state separately.
    pub fn retained_state_upper_bound(&self) -> Option<usize> {
        let mut state = std::mem::size_of::<Self>()
            .checked_add(
                self.members
                    .capacity()
                    .checked_mul(std::mem::size_of::<SourceCutMemberWitness>())?,
            )?
            .checked_add(
                self.identities
                    .capacity()
                    .checked_mul(std::mem::size_of::<SourceCutIdentityWitness>())?,
            )?
            .checked_add(
                self.directories
                    .capacity()
                    .checked_mul(std::mem::size_of::<SourceCutDirectoryWitness>())?,
            )?
            .checked_add(
                self.dependencies
                    .capacity()
                    .checked_mul(std::mem::size_of::<SourceCutDependencyWitness>())?,
            )?
            .checked_add(
                self.object_refcounts
                    .capacity()
                    .checked_mul(std::mem::size_of::<SourceCutObjectRefcountWitness>())?,
            )?;
        for witness in &self.members {
            state = state
                .checked_add(witness.path.as_str().len().saturating_mul(4))?
                .checked_add(witness.expected_indexed_ids.as_ref().map_or(0, |ids| {
                    ids.capacity()
                        .checked_mul(std::mem::size_of::<String>())
                        .unwrap_or(usize::MAX)
                }))?;
            if let Some(ids) = &witness.expected_indexed_ids {
                for id in ids {
                    state = state.checked_add(id.capacity())?;
                }
            }
        }
        for witness in &self.identities {
            state = state.checked_add(witness.id.capacity())?.checked_add(
                witness
                    .expected_path
                    .as_ref()
                    .map_or(0, |path| path.as_str().len().saturating_mul(4)),
            )?;
        }
        for witness in &self.directories {
            state = state
                .checked_add(witness.path.as_str().len().saturating_mul(4))?
                .checked_add(
                    witness
                        .expected_children
                        .capacity()
                        .checked_mul(std::mem::size_of::<(String, bool)>())?,
                )?;
            for (name, _) in &witness.expected_children {
                state = state.checked_add(name.capacity())?;
            }
        }
        for witness in &self.dependencies {
            state = state
                .checked_add(witness.path.as_str().len().saturating_mul(4))?
                .checked_add(
                    witness
                        .expected_targets
                        .capacity()
                        .checked_mul(std::mem::size_of::<RelativePath>())?,
                )?;
            for path in &witness.expected_targets {
                state = state.checked_add(path.as_str().len().saturating_mul(4))?;
            }
        }
        Some(state)
    }
}

/// A metadata-only ordered member cursor. Completion is available only after
/// the caller observes EOF and the exact row count and semantic digest match.
pub trait SourceCutMetadataStream {
    fn expectation(&self) -> SourceCutMembership;
    fn coverage(&self) -> Option<SourceCutMembership>;
    fn next_metadata(
        &mut self,
        caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<MemberMetadata>>;
}

/// An exact ordered member cursor. Implementations retain one selected source
/// session and account each metadata seek and payload read through its ledger.
pub trait SourceCutMemberStream {
    fn expectation(&self) -> SourceCutMembership;
    fn coverage(&self) -> Option<SourceCutMembership>;
    fn next_member(
        &mut self,
        caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<SourceMemberV1>>;
}

/// Lazily follows the selected revision's exact base chain. Implementations
/// report complete coverage only after the caller observes its terminal EOF.
pub trait SourceCutRevisionStream {
    fn complete(&self) -> bool;
    fn next_revision(
        &mut self,
        caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<SourceCutRevision>>;
}

/// Shared read-only source boundary for bounded V1 and V2 callers. Methods
/// take `&mut self` so a V2 implementation cannot bypass its cumulative read
/// ledger by issuing independent point readers.
pub trait SourceCutRead {
    fn selection(&self) -> SourceCutSelection;
    fn revision_stream<'a>(
        &'a mut self,
        caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Box<dyn SourceCutRevisionStream + 'a>>;
    fn revision(
        &mut self,
        revision: SourceRevision,
        caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<SourceCutRevision>>;
    fn member(
        &mut self,
        revision: SourceRevision,
        path: &RelativePath,
        caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<MemberMetadata>>;
    /// List immediate children of one exact selected directory path. The
    /// operation is bounded by the caller's child cap and does not imply a
    /// complete-source traversal.
    fn list_directory(
        &mut self,
        revision: SourceRevision,
        path: &RelativePath,
        max_entries: usize,
        caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<Vec<(String, bool)>>>;
    fn identity_path(
        &mut self,
        revision: SourceRevision,
        id: &str,
        caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<RelativePath>>;
    fn indexed_ids_for_path(
        &mut self,
        revision: SourceRevision,
        path: &RelativePath,
        caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<Vec<String>>>;
    fn indexed_dependencies(
        &mut self,
        revision: SourceRevision,
        path: &RelativePath,
        caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<Vec<RelativePath>>>;
    fn presence(
        &mut self,
        revision: SourceRevision,
        path: &RelativePath,
        caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<SourcePresenceV1>>;
    fn read_member(
        &mut self,
        revision: SourceRevision,
        path: &RelativePath,
        max_bytes: u64,
        caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<SourceMemberV1>;
    fn read_retirement(
        &mut self,
        revision: SourceRevision,
        index: usize,
        max_bytes: u64,
        caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<RetiredSourceMemberV1>;
    fn metadata_stream<'a>(
        &'a mut self,
        revision: SourceRevision,
        caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Box<dyn SourceCutMetadataStream + 'a>>;
    fn member_stream<'a>(
        &'a mut self,
        revision: SourceRevision,
        caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Box<dyn SourceCutMemberStream + 'a>>;
    /// Observe an actual authenticated object reference-count index. A provider
    /// lacking that index must refuse rather than infer a count from members.
    fn object_refcount(
        &mut self,
        _revision: SourceRevision,
        _digest: Digest256,
        _caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<u64>> {
        check_time(deadline, cancelled)?;
        Err(StoreError::new(
            StoreErrorCode::UnsupportedFormat,
            "selected source cut has no authenticated object refcount lookup",
        ))
    }
    fn verify_current_fence(&mut self, deadline: Instant, cancelled: &AtomicBool) -> Result<()>;
    /// Re-select current and prove only this operation's exact positive and
    /// negative physical witnesses. V2 implementations may accept a newer
    /// root when every witness is unchanged; V1 continues to require its full
    /// current-fence rules at the caller's owning transaction boundary.
    fn verify_readset_current(
        &mut self,
        base_revision: SourceRevision,
        members: &[SourceCutMemberWitness],
        identities: &[SourceCutIdentityWitness],
        directories: &[SourceCutDirectoryWitness],
        dependencies: &[SourceCutDependencyWitness],
        object_refcounts: &[SourceCutObjectRefcountWitness],
        caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<SourceCutSelection>;
}

/// A file is present only when listed in this exact snapshot. Directory
/// existence describes the source validator's materialized namespace, not the
/// external payload filesystem or a physical directory in the object store.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourcePresenceV1 {
    File,
    MaterializedDirectory,
}

#[derive(Debug)]
pub struct CorpusCutReader {
    reader: CorpusReader,
    snapshots: Vec<Snapshot>, // current first, then its exact retained bases
    limits: CutReadLimits,
}

impl CorpusReader {
    /// Open the exact current revision and *all* of its retained base chain.
    /// Missing bases, cycles, unsupported membership or budgets refuse the cut;
    /// a caller cannot choose a shorter chain and claim complete history.
    pub fn open_source_cut(
        &self,
        current: SourceRevision,
        limits: CutReadLimits,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<CorpusCutReader> {
        if limits.max_revisions == 0
            || limits.max_revisions == usize::MAX
            || limits.max_members == 0
            || limits.max_members == u64::MAX
            || limits.max_total_bytes == 0
            || limits.max_total_bytes == u64::MAX
            || limits.max_member_bytes == 0
            || limits.max_member_bytes == u64::MAX
        {
            return Err(refusal("invalid source cut budget"));
        }
        let mut snapshots = Vec::new();
        let mut visited = BTreeSet::new();
        let mut next = Some(current);
        let mut members = 0u64;
        let mut bytes = 0u64;
        while let Some(revision) = next {
            check_time(deadline, cancelled)?;
            if snapshots.len() >= limits.max_revisions || !visited.insert(revision.0) {
                return Err(refusal("source history chain exceeds budget or cycles"));
            }
            let snapshot = self.load_exact(revision)?;
            for member in snapshot.members() {
                check_time(deadline, cancelled)?;
                if !is_authored_source_path_v1(&member.path.as_str().to_owned()) {
                    return Err(StoreError::new(
                        StoreErrorCode::InvalidMemberIndex,
                        "member is outside the source admission carrier",
                    ));
                }
                members = members
                    .checked_add(1)
                    .ok_or_else(|| refusal("source member count overflow"))?;
                bytes = bytes
                    .checked_add(member.size_bytes)
                    .ok_or_else(|| refusal("source byte count overflow"))?;
                if members > limits.max_members
                    || bytes > limits.max_total_bytes
                    || member.size_bytes > limits.max_member_bytes
                {
                    return Err(refusal("source cut exceeds declared budget"));
                }
            }
            for retired in snapshot.retirements() {
                if !is_authored_source_path_v1(retired.path.as_str())
                    || !is_authored_source_path_v1(retired.event_ref.as_str())
                {
                    return Err(StoreError::new(
                        StoreErrorCode::InvalidRetirementIndex,
                        "retirement is outside source carrier",
                    ));
                }
                // Reserve a finite worst case for the unknown v1 retired
                // object length before exposing any retirement bytes.
                members = members
                    .checked_add(2)
                    .ok_or_else(|| refusal("retirement member count overflow"))?;
                bytes = bytes
                    .checked_add(limits.max_member_bytes)
                    .and_then(|n| n.checked_add(retired.event_size_bytes))
                    .ok_or_else(|| refusal("retirement byte count overflow"))?;
                if members > limits.max_members
                    || bytes > limits.max_total_bytes
                    || retired.event_size_bytes > limits.max_member_bytes
                {
                    return Err(refusal("retirement cut exceeds declared budget"));
                }
            }
            next = snapshot.base_revision();
            snapshots.push(snapshot);
        }
        check_time(deadline, cancelled)?;
        Ok(CorpusCutReader {
            reader: self.clone(),
            snapshots,
            limits,
        })
    }
}

impl CorpusCutReader {
    pub fn current(&self) -> &Snapshot {
        &self.snapshots[0]
    }
    /// Current is index zero. Retained revisions keep original paths and
    /// identities; they never contribute current ID ownership by inference.
    pub fn revisions(&self) -> impl Iterator<Item = &Snapshot> {
        self.snapshots.iter()
    }
    /// Exact retirement-ledger member and its separate event carrier. These
    /// bytes are retained evidence, never members of the current ID universe.
    pub fn read_retirement(
        &self,
        revision: SourceRevision,
        index: usize,
        max_bytes: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<RetiredSourceMemberV1> {
        check_time(deadline, cancelled)?;
        let snapshot = self
            .snapshots
            .iter()
            .find(|s| s.revision() == revision)
            .ok_or_else(|| {
                StoreError::new(
                    StoreErrorCode::MissingRevision,
                    "revision is outside opened source cut",
                )
            })?;
        let metadata = snapshot.retirements().get(index).ok_or_else(|| {
            StoreError::new(
                StoreErrorCode::MissingMember,
                "retirement is outside exact source revision",
            )
        })?;
        let cap = max_bytes.min(self.limits.max_member_bytes);
        let mut source = TimedStage {
            raw: Vec::new(),
            deadline,
            cancelled,
        };
        self.reader
            .read_retirement_object(snapshot, metadata.sha256, None, cap, &mut source)?;
        check_time(deadline, cancelled)?;
        let mut event = TimedStage {
            raw: Vec::new(),
            deadline,
            cancelled,
        };
        self.reader.read_retirement_object(
            snapshot,
            metadata.event_sha256,
            Some(metadata.event_size_bytes),
            cap,
            &mut event,
        )?;
        check_time(deadline, cancelled)?;
        Ok(RetiredSourceMemberV1 {
            revision,
            metadata: metadata.clone(),
            raw: source.raw,
            event_raw: event.raw,
        })
    }
    /// Random exact companion lookup under the same anchored cut. The rule
    /// executor owns its aggregate lookup budget; every individual read remains
    /// capped and private until complete digest verification.
    pub fn read_member(
        &self,
        revision: SourceRevision,
        path: &RelativePath,
        max_bytes: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<SourceMemberV1> {
        check_time(deadline, cancelled)?;
        let snapshot = self
            .snapshots
            .iter()
            .find(|s| s.revision() == revision)
            .ok_or_else(|| {
                StoreError::new(
                    StoreErrorCode::MissingRevision,
                    "revision is outside opened source cut",
                )
            })?;
        let metadata = snapshot.member(path).ok_or_else(|| {
            StoreError::new(
                StoreErrorCode::MissingMember,
                "member is outside exact source revision",
            )
        })?;
        let descriptor = CorpusDescriptor {
            revision,
            path: path.clone(),
            sha256: metadata.sha256,
            size_bytes: metadata.size_bytes,
            mode: metadata.mode,
        };
        let mut stage = TimedStage {
            raw: Vec::new(),
            deadline,
            cancelled,
        };
        self.reader.read_selected(
            snapshot,
            &descriptor,
            max_bytes.min(self.limits.max_member_bytes),
            &mut stage,
        )?;
        check_time(deadline, cancelled)?;
        Ok(SourceMemberV1 {
            path: path.clone(),
            raw: stage.raw,
            revision,
            stable_ids: snapshot.ids_for_path(path).map(str::to_owned).collect(),
        })
    }
    pub fn presence(
        &self,
        revision: SourceRevision,
        path: &RelativePath,
    ) -> Option<SourcePresenceV1> {
        let snapshot = self.snapshots.iter().find(|s| s.revision() == revision)?;
        if snapshot.member(path).is_some() {
            return Some(SourcePresenceV1::File);
        }
        let prefix = format!("{}/", path.as_str());
        snapshot
            .members()
            .any(|m| m.path.as_str().starts_with(&prefix))
            .then_some(SourcePresenceV1::MaterializedDirectory)
    }
    /// Each revision has its own strictly path-ordered stream and EOF root.
    /// Retained revisions must be validated under their frozen profile, rather
    /// than merged into the current rule runner or renamed with path suffixes.
    pub fn stream(&self, revision: SourceRevision) -> Result<SourceMemberStreamV1<'_>> {
        let snapshot = self
            .snapshots
            .iter()
            .find(|s| s.revision() == revision)
            .ok_or_else(|| {
                StoreError::new(
                    StoreErrorCode::MissingRevision,
                    "revision is outside opened source cut",
                )
            })?;
        let mut expected = Digest256Hasher::new();
        expected.update(b"tos-val-full-membership-v1\0");
        for member in snapshot.members() {
            feed_member(
                &mut expected,
                &member.path.as_str().to_owned(),
                member.size_bytes,
                member.sha256,
            );
        }
        Ok(SourceMemberStreamV1 {
            cut: self,
            snapshot,
            last: None,
            count: 0,
            bytes: 0,
            actual: {
                let mut h = Digest256Hasher::new();
                h.update(b"tos-val-full-membership-v1\0");
                h
            },
            expected: SourceMembershipV1 {
                count: snapshot.member_count() as u64,
                digest: expected.finalize(),
            },
            complete: false,
            failed: false,
        })
    }
}

pub struct SourceMemberStreamV1<'a> {
    cut: &'a CorpusCutReader,
    snapshot: &'a Snapshot,
    last: Option<RelativePath>,
    count: u64,
    bytes: u64,
    actual: Digest256Hasher,
    expected: SourceMembershipV1,
    complete: bool,
    failed: bool,
}

impl SourceMemberStreamV1<'_> {
    pub fn expectation(&self) -> SourceMembershipV1 {
        self.expected
    }
    pub fn coverage(&self) -> Option<SourceMembershipV1> {
        self.complete.then_some(self.expected)
    }
    pub fn next_member(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<SourceMemberV1>> {
        if self.failed {
            return Err(refusal("source stream already refused"));
        }
        let result = self.next_inner(deadline, cancelled);
        if result.is_err() {
            self.failed = true;
            self.complete = false;
        }
        result
    }
    fn next_inner(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<SourceMemberV1>> {
        check_time(deadline, cancelled)?;
        if self.complete {
            return Ok(None);
        }
        let Some(metadata) = self.snapshot.member_after(self.last.as_ref()) else {
            if self.count != self.expected.count
                || self.actual.clone().finalize() != self.expected.digest
            {
                return Err(StoreError::new(
                    StoreErrorCode::DescriptorMismatch,
                    "source stream membership differs",
                ));
            }
            self.complete = true;
            return Ok(None);
        };
        let next_bytes = self
            .bytes
            .checked_add(metadata.size_bytes)
            .ok_or_else(|| refusal("source stream byte count overflow"))?;
        if self.count >= self.cut.limits.max_members || next_bytes > self.cut.limits.max_total_bytes
        {
            return Err(refusal("source stream exceeds declared budget"));
        }
        let member = self.cut.read_member(
            self.snapshot.revision(),
            &metadata.path,
            self.cut.limits.max_member_bytes,
            deadline,
            cancelled,
        )?;
        feed_member(
            &mut self.actual,
            metadata.path.as_str(),
            member.raw.len() as u64,
            Digest256::of_bytes(&member.raw),
        );
        self.last = Some(metadata.path.clone());
        self.count += 1;
        self.bytes = next_bytes;
        Ok(Some(member))
    }
}

struct TimedStage<'a> {
    raw: Vec<u8>,
    deadline: Instant,
    cancelled: &'a AtomicBool,
}
impl Write for TimedStage<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.cancelled.load(Ordering::Relaxed) || Instant::now() >= self.deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "source stream cancelled or expired",
            ));
        }
        self.raw.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn check_time(deadline: Instant, cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
        Err(refusal("source read cancelled or expired"))
    } else {
        Ok(())
    }
}
fn refusal(detail: &'static str) -> StoreError {
    StoreError::new(StoreErrorCode::BudgetExceeded, detail)
}
fn feed_member(h: &mut Digest256Hasher, path: &str, length: u64, digest: Digest256) {
    feed_source_membership_v1(h, path, length, digest);
}

/// Add one ordered row to the V1 semantic membership transcript. Physical V2
/// tree commitments remain separate and must not be passed as row digests.
pub fn feed_source_membership_v1(
    h: &mut Digest256Hasher,
    path: &str,
    length: u64,
    digest: Digest256,
) {
    h.update(&(path.len() as u64).to_be_bytes());
    h.update(path.as_bytes());
    h.update(&length.to_be_bytes());
    h.update(digest.as_bytes());
}

struct CorpusCutMemberStream<'a> {
    inner: SourceMemberStreamV1<'a>,
    expected: SourceCutMembership,
}

impl SourceCutMemberStream for CorpusCutMemberStream<'_> {
    fn expectation(&self) -> SourceCutMembership {
        self.expected
    }

    fn coverage(&self) -> Option<SourceCutMembership> {
        self.inner.coverage().map(|_| self.expected)
    }

    fn next_member(
        &mut self,
        _caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<SourceMemberV1>> {
        self.inner.next_member(deadline, cancelled)
    }
}

struct SnapshotMetadataStream<'a> {
    snapshot: &'a Snapshot,
    max_members: u64,
    max_total_bytes: u64,
    expected: SourceCutMembership,
    last: Option<RelativePath>,
    count: u64,
    bytes: u64,
    actual: Digest256Hasher,
    complete: bool,
    failed: bool,
}

struct SnapshotRevisionStream<'a> {
    snapshots: &'a [Snapshot],
    index: usize,
    complete: bool,
    failed: bool,
}

impl SourceCutRevisionStream for SnapshotRevisionStream<'_> {
    fn complete(&self) -> bool {
        self.complete
    }

    fn next_revision(
        &mut self,
        _caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<SourceCutRevision>> {
        if self.failed {
            return Err(cut_refusal("source revision stream already refused"));
        }
        let result = (|| {
            check_time(deadline, cancelled)?;
            let Some(snapshot) = self.snapshots.get(self.index) else {
                self.complete = true;
                return Ok(None);
            };
            let revision = cut_revision(snapshot)?;
            self.index = self
                .index
                .checked_add(1)
                .ok_or_else(|| cut_refusal("source revision stream count overflow"))?;
            check_time(deadline, cancelled)?;
            Ok(Some(revision))
        })();
        if result.is_err() {
            self.failed = true;
            self.complete = false;
        }
        result
    }
}

impl SourceCutMetadataStream for SnapshotMetadataStream<'_> {
    fn expectation(&self) -> SourceCutMembership {
        self.expected
    }

    fn coverage(&self) -> Option<SourceCutMembership> {
        self.complete.then_some(self.expected)
    }

    fn next_metadata(
        &mut self,
        _caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<MemberMetadata>> {
        if self.failed {
            return Err(cut_refusal("source metadata stream already refused"));
        }
        let result = self.next_metadata_inner(deadline, cancelled);
        if result.is_err() {
            self.failed = true;
            self.complete = false;
        }
        result
    }
}

impl SnapshotMetadataStream<'_> {
    fn next_metadata_inner(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<MemberMetadata>> {
        check_time(deadline, cancelled)?;
        if self.complete {
            return Ok(None);
        }
        let Some(metadata) = self.snapshot.member_after(self.last.as_ref()) else {
            let expected = self.expected.rows();
            if self.count != expected.count || self.actual.clone().finalize() != expected.digest {
                return Err(StoreError::new(
                    StoreErrorCode::DescriptorMismatch,
                    "source metadata stream membership differs",
                ));
            }
            self.complete = true;
            return Ok(None);
        };
        let next_count = self
            .count
            .checked_add(1)
            .filter(|count| *count <= self.max_members)
            .ok_or_else(|| cut_refusal("source metadata stream exceeds member budget"))?;
        let next_bytes = self
            .bytes
            .checked_add(metadata.size_bytes)
            .filter(|bytes| *bytes <= self.max_total_bytes)
            .ok_or_else(|| cut_refusal("source metadata stream exceeds byte budget"))?;
        feed_member(
            &mut self.actual,
            metadata.path.as_str(),
            metadata.size_bytes,
            metadata.sha256,
        );
        self.last = Some(metadata.path.clone());
        self.count = next_count;
        self.bytes = next_bytes;
        Ok(Some(metadata.clone()))
    }
}

fn cut_membership(snapshot: &Snapshot) -> SourceMembershipV1 {
    let mut digest = Digest256Hasher::new();
    digest.update(b"tos-val-full-membership-v1\0");
    for member in snapshot.members() {
        feed_member(
            &mut digest,
            member.path.as_str(),
            member.size_bytes,
            member.sha256,
        );
    }
    SourceMembershipV1 {
        count: snapshot.member_count() as u64,
        digest: digest.finalize(),
    }
}

fn cut_revision(snapshot: &Snapshot) -> Result<SourceCutRevision> {
    let mut dependency_source_count = 0u64;
    let mut dependency_count = 0u64;
    for member in snapshot.members() {
        if let Some(values) = snapshot.indexed_dependencies(&member.path) {
            dependency_source_count = dependency_source_count
                .checked_add(1)
                .ok_or_else(|| cut_refusal("source dependency source count overflow"))?;
            dependency_count =
                dependency_count
                    .checked_add(u64::try_from(values.len()).map_err(|_| {
                        cut_refusal("source dependency count exceeds address space")
                    })?)
                    .ok_or_else(|| cut_refusal("source dependency count overflow"))?;
        }
    }
    Ok(SourceCutRevision {
        format: SourceCutFormat::CorpusSnapshotV1,
        revision: snapshot.revision(),
        base_revision: snapshot.base_revision(),
        validator_sha256: snapshot.validator_sha256(),
        member_count: snapshot.member_count() as u64,
        source_bytes: snapshot.members().try_fold(0u64, |sum, member| {
            sum.checked_add(member.size_bytes)
                .ok_or_else(|| cut_refusal("source member byte aggregate overflow"))
        })?,
        identity_count: snapshot.identity_count() as u64,
        dependency_source_count,
        dependency_count,
        retirement_count: snapshot.retirement_count() as u64,
        membership: SourceCutMembership::CorpusSnapshotV1(cut_membership(snapshot)),
    })
}

impl SourceCutRead for CorpusCutReader {
    fn selection(&self) -> SourceCutSelection {
        SourceCutSelection {
            format: SourceCutFormat::CorpusSnapshotV1,
            current_revision: self.snapshots[0].revision(),
            rootset_sha256: None,
        }
    }

    fn revision_stream<'a>(
        &'a mut self,
        _caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Box<dyn SourceCutRevisionStream + 'a>> {
        check_time(deadline, cancelled)?;
        Ok(Box::new(SnapshotRevisionStream {
            snapshots: &self.snapshots,
            index: 0,
            complete: false,
            failed: false,
        }))
    }

    fn revision(
        &mut self,
        revision: SourceRevision,
        _caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<SourceCutRevision>> {
        check_time(deadline, cancelled)?;
        Ok(self
            .snapshots
            .iter()
            .find(|snapshot| snapshot.revision() == revision)
            .map(cut_revision)
            .transpose()?)
    }

    fn member(
        &mut self,
        revision: SourceRevision,
        path: &RelativePath,
        _caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<MemberMetadata>> {
        check_time(deadline, cancelled)?;
        Ok(self
            .snapshots
            .iter()
            .find(|snapshot| snapshot.revision() == revision)
            .and_then(|snapshot| snapshot.member(path))
            .cloned())
    }

    fn list_directory(
        &mut self,
        revision: SourceRevision,
        path: &RelativePath,
        max_entries: usize,
        _caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<Vec<(String, bool)>>> {
        check_time(deadline, cancelled)?;
        if max_entries == 0 || max_entries == usize::MAX {
            return Err(StoreError::new(
                StoreErrorCode::BudgetExceeded,
                "source directory child budget is not finite",
            ));
        }
        let Some(snapshot) = self
            .snapshots
            .iter()
            .find(|snapshot| snapshot.revision() == revision)
        else {
            return Ok(None);
        };
        let prefix = format!("{}/", path.as_str());
        let mut children = BTreeMap::<String, bool>::new();
        for member in snapshot.members() {
            check_time(deadline, cancelled)?;
            let Some(tail) = member.path.as_str().strip_prefix(&prefix) else {
                continue;
            };
            let (name, is_directory) = match tail.split_once('/') {
                Some((name, _)) => (name, true),
                None => (tail, false),
            };
            if name.is_empty() {
                continue;
            }
            if children
                .insert(name.to_owned(), is_directory)
                .is_some_and(|prior| prior != is_directory)
            {
                return Err(StoreError::new(
                    StoreErrorCode::DescriptorMismatch,
                    "source path is both a file and directory",
                ));
            }
            if children.len() > max_entries {
                return Err(StoreError::new(
                    StoreErrorCode::BudgetExceeded,
                    "source directory child budget exceeded",
                ));
            }
        }
        check_time(deadline, cancelled)?;
        if children.is_empty() {
            Ok(None)
        } else {
            Ok(Some(children.into_iter().collect()))
        }
    }

    fn identity_path(
        &mut self,
        revision: SourceRevision,
        id: &str,
        _caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<RelativePath>> {
        check_time(deadline, cancelled)?;
        Ok(self
            .snapshots
            .iter()
            .find(|snapshot| snapshot.revision() == revision)
            .and_then(|snapshot| snapshot.identity_path(id))
            .cloned())
    }

    fn indexed_ids_for_path(
        &mut self,
        revision: SourceRevision,
        path: &RelativePath,
        _caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<Vec<String>>> {
        check_time(deadline, cancelled)?;
        let Some(snapshot) = self
            .snapshots
            .iter()
            .find(|snapshot| snapshot.revision() == revision)
        else {
            return Ok(None);
        };
        Ok(snapshot.member(path).map(|_| {
            snapshot
                .indexed_ids_for_path(path)
                .map(str::to_owned)
                .collect()
        }))
    }

    fn indexed_dependencies(
        &mut self,
        revision: SourceRevision,
        path: &RelativePath,
        _caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<Vec<RelativePath>>> {
        check_time(deadline, cancelled)?;
        let Some(snapshot) = self
            .snapshots
            .iter()
            .find(|snapshot| snapshot.revision() == revision)
        else {
            return Ok(None);
        };
        Ok(snapshot.member(path).and_then(|_| {
            snapshot
                .indexed_dependencies(path)
                .map(|values| values.to_vec())
        }))
    }

    fn presence(
        &mut self,
        revision: SourceRevision,
        path: &RelativePath,
        _caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<SourcePresenceV1>> {
        check_time(deadline, cancelled)?;
        Ok(CorpusCutReader::presence(self, revision, path))
    }

    fn read_member(
        &mut self,
        revision: SourceRevision,
        path: &RelativePath,
        max_bytes: u64,
        _caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<SourceMemberV1> {
        CorpusCutReader::read_member(self, revision, path, max_bytes, deadline, cancelled)
    }

    fn read_retirement(
        &mut self,
        revision: SourceRevision,
        index: usize,
        max_bytes: u64,
        _caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<RetiredSourceMemberV1> {
        CorpusCutReader::read_retirement(self, revision, index, max_bytes, deadline, cancelled)
    }

    fn metadata_stream<'a>(
        &'a mut self,
        revision: SourceRevision,
        _caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Box<dyn SourceCutMetadataStream + 'a>> {
        check_time(deadline, cancelled)?;
        let snapshot = self
            .snapshots
            .iter()
            .find(|snapshot| snapshot.revision() == revision)
            .ok_or_else(|| {
                StoreError::new(
                    StoreErrorCode::MissingRevision,
                    "revision is outside opened source cut",
                )
            })?;
        let mut expected = Digest256Hasher::new();
        expected.update(b"tos-val-full-membership-v1\0");
        for member in snapshot.members() {
            feed_member(
                &mut expected,
                member.path.as_str(),
                member.size_bytes,
                member.sha256,
            );
        }
        Ok(Box::new(SnapshotMetadataStream {
            snapshot,
            max_members: self.limits.max_members,
            max_total_bytes: self.limits.max_total_bytes,
            expected: SourceCutMembership::CorpusSnapshotV1(SourceMembershipV1 {
                count: snapshot.member_count() as u64,
                digest: expected.finalize(),
            }),
            last: None,
            count: 0,
            bytes: 0,
            actual: {
                let mut actual = Digest256Hasher::new();
                actual.update(b"tos-val-full-membership-v1\0");
                actual
            },
            complete: false,
            failed: false,
        }))
    }

    fn member_stream<'a>(
        &'a mut self,
        revision: SourceRevision,
        _caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Box<dyn SourceCutMemberStream + 'a>> {
        let inner = CorpusCutReader::stream(self, revision)?;
        check_time(deadline, cancelled)?;
        let expected = SourceCutMembership::CorpusSnapshotV1(inner.expectation());
        Ok(Box::new(CorpusCutMemberStream { inner, expected }))
    }

    fn verify_current_fence(&mut self, deadline: Instant, cancelled: &AtomicBool) -> Result<()> {
        check_time(deadline, cancelled)?;
        if self.reader.select_current()? != Some(self.snapshots[0].revision()) {
            return Err(StoreError::new(
                StoreErrorCode::RevisionMismatch,
                "current source selection advanced",
            ));
        }
        check_time(deadline, cancelled)
    }

    fn verify_readset_current(
        &mut self,
        base_revision: SourceRevision,
        members: &[SourceCutMemberWitness],
        identities: &[SourceCutIdentityWitness],
        directories: &[SourceCutDirectoryWitness],
        dependencies: &[SourceCutDependencyWitness],
        object_refcounts: &[SourceCutObjectRefcountWitness],
        caller_retained_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<SourceCutSelection> {
        check_time(deadline, cancelled)?;
        if !object_refcounts.is_empty() {
            return Err(StoreError::new(
                StoreErrorCode::UnsupportedFormat,
                "V1 source cut cannot verify object refcount witnesses",
            ));
        }
        self.verify_current_fence(deadline, cancelled)?;
        if self.selection().current_revision != base_revision {
            return Err(StoreError::new(
                StoreErrorCode::RevisionMismatch,
                "source readset base revision changed",
            ));
        }
        for witness in members {
            check_time(deadline, cancelled)?;
            if self.presence(
                base_revision,
                &witness.path,
                caller_retained_state_bytes,
                deadline,
                cancelled,
            )? != witness.expected_presence
            {
                return Err(StoreError::new(
                    StoreErrorCode::DescriptorMismatch,
                    "source readset path presence changed",
                ));
            }
            let actual = self.member(
                base_revision,
                &witness.path,
                caller_retained_state_bytes,
                deadline,
                cancelled,
            )?;
            let expected = witness.expected_member.map(|tuple| MemberMetadata {
                path: witness.path.clone(),
                sha256: tuple.sha256,
                size_bytes: tuple.size_bytes,
                mode: tuple.mode,
            });
            if actual != expected {
                return Err(StoreError::new(
                    StoreErrorCode::DescriptorMismatch,
                    "source readset member binding changed",
                ));
            }
            if self.indexed_ids_for_path(
                base_revision,
                &witness.path,
                caller_retained_state_bytes,
                deadline,
                cancelled,
            )? != witness.expected_indexed_ids
            {
                return Err(StoreError::new(
                    StoreErrorCode::DescriptorMismatch,
                    "source readset inverse identity coverage changed",
                ));
            }
        }
        for witness in identities {
            check_time(deadline, cancelled)?;
            if self.identity_path(
                base_revision,
                &witness.id,
                caller_retained_state_bytes,
                deadline,
                cancelled,
            )? != witness.expected_path
            {
                return Err(StoreError::new(
                    StoreErrorCode::DescriptorMismatch,
                    "source readset identity binding changed",
                ));
            }
        }
        for witness in directories {
            check_time(deadline, cancelled)?;
            if self.presence(
                base_revision,
                &witness.path,
                caller_retained_state_bytes,
                deadline,
                cancelled,
            )? != witness.expected_presence
            {
                return Err(StoreError::new(
                    StoreErrorCode::DescriptorMismatch,
                    "source readset directory presence changed",
                ));
            }
            let children = self
                .list_directory(
                    base_revision,
                    &witness.path,
                    witness
                        .expected_children
                        .len()
                        .checked_add(1)
                        .ok_or_else(|| cut_refusal("source directory witness cap overflow"))?,
                    caller_retained_state_bytes,
                    deadline,
                    cancelled,
                )?
                .unwrap_or_default();
            if children != witness.expected_children {
                return Err(StoreError::new(
                    StoreErrorCode::DescriptorMismatch,
                    "source readset directory selection changed",
                ));
            }
        }
        for witness in dependencies {
            check_time(deadline, cancelled)?;
            if self.indexed_dependencies(
                base_revision,
                &witness.path,
                caller_retained_state_bytes,
                deadline,
                cancelled,
            )? != Some(witness.expected_targets.clone())
            {
                return Err(StoreError::new(
                    StoreErrorCode::DescriptorMismatch,
                    "source readset dependency closure changed",
                ));
            }
        }
        Ok(self.selection())
    }
}

fn cut_refusal(detail: &'static str) -> StoreError {
    StoreError::new(StoreErrorCode::BudgetExceeded, detail)
}
/// Existing authored-cut path eligibility only; no filesystem completeness or
/// byte-read authority is inferred from a true result.
pub fn is_authored_source_path_v1(path: &str) -> bool {
    path.starts_with("ToS/")
        && has_authored_source_descendants_v1(path)
        && (!(path.starts_with("ToS/derived-exports/")
            || path.starts_with("ToS/source-witnesses/catalog/"))
            || path.ends_with(".md"))
}

/// Directory traversal classification for the existing authored-cut law.
/// Weak output parents may contain authored Markdown companions. This grants
/// neither file membership, completeness nor a read permission.
pub fn has_authored_source_descendants_v1(path: &str) -> bool {
    (path == "ToS" || path.starts_with("ToS/"))
        && !path
            .split('/')
            .any(|p| matches!(p, ".git" | "payload" | "owner-local"))
}
