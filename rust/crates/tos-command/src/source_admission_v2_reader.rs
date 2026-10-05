//! Bounded exact-revision point reads from the private native V2 store.
//! These observations confer no admission, rights or currentness after the
//! selected immutable session. Strict legacy V1 readers remain unchanged.
use super::source_admission::{active, invalid};
use super::source_admission_segment_v2::{
    SourceRevisionRootsV2, SourceRootSetV2, decode_workspace_upper_bound,
};
use super::source_admission_store::AdmissionStore;
use std::{
    fs::File,
    io::{self, Read},
    mem::size_of,
    os::unix::fs::MetadataExt,
    path::Path,
    sync::{Arc, atomic::AtomicBool},
    time::Instant,
};
use tos_foundation::{Digest256, Digest256Hasher, RelativePath, SourceRevision};
use tos_segment_store::{
    AuthenticatedTreeDescriptorV2, AuthenticatedTreeEntryV1, AuthenticatedTreeIoLedgerV1,
    AuthenticatedTreeLimitsV1, AuthenticatedTreeWorkV1, SegmentLimits, SegmentStore,
};
use tos_source_store::{
    CorpusCurrentSelection, CorpusPointerFormat, PinnedSqliteIoBudget, ReadLimits,
    SourceMembershipV1,
};

const DOMAIN: &[u8] = b"tos-native-admission-source-v2";
const ROOT_BYTES: usize = 65_536;
const BLOCK_BYTES: usize = 65_536;

#[derive(Clone, Copy)]
pub struct V2PointReadLimits {
    pub pointer: ReadLimits,
    pub segment: SegmentLimits,
    pub tree: AuthenticatedTreeLimitsV1,
    pub max_object_bytes: usize,
    /// Includes caller-retained data, session roots, decode/transient frame
    /// overlap and one returned object. No independently reset state grant.
    pub max_state_bytes: usize,
    pub caller_retained_state_bytes: usize,
}

impl V2PointReadLimits {
    fn validate(mut self) -> io::Result<Self> {
        // V2 mutable selection is a fixed, shallow pointer record. Do not let
        // the inherited V1 snapshot profile turn that small read into a large
        // parser-state reservation. Every value is narrowed to the original
        // protected limits; this never raises a caller-selected cap.
        self.pointer.max_manifest_bytes = self.pointer.max_manifest_bytes.min(4 * 1024);
        self.pointer.json.max_bytes = self.pointer.json.max_bytes.min(4 * 1024);
        self.pointer.json.max_depth = self.pointer.json.max_depth.min(8);
        self.pointer.json.max_visits = self.pointer.json.max_visits.min(64);
        self.pointer.json.max_integer_digits = self.pointer.json.max_integer_digits.min(64);
        self.pointer.validate().map_err(invalid)?;
        self.segment.validate().map_err(invalid)?;
        if self.max_object_bytes == 0
            || self.max_object_bytes == usize::MAX
            || self.max_state_bytes == 0
            || self.max_state_bytes == usize::MAX
            || self.tree.max_nodes == 0
            || self.tree.max_nodes == u64::MAX
            || self.tree.max_total_bytes == 0
            || self.tree.max_total_bytes == u64::MAX
            || self.tree.max_value_bytes == 0
            || self.tree.max_value_bytes > SourceRevisionRootsV2::MAX_ENCODED_BYTES
        {
            return Err(invalid("V2 point reader finite profile differs"));
        }
        let required = self.base_state_bytes()?;
        if required > self.max_state_bytes {
            return Err(invalid("V2 point simultaneous state allowance exceeded"));
        }
        Ok(self)
    }

    fn base_state_bytes(self) -> io::Result<usize> {
        let history_value_bytes = usize::try_from(self.tree.max_value_bytes)
            .map_err(|_| invalid("V2 history value bound exceeds address space"))?;
        let rootset_result = SourceRootSetV2::retained_state_upper_bound_for_value(ROOT_BYTES)?;
        let history_result =
            SourceRevisionRootsV2::retained_state_upper_bound_for_value(history_value_bytes)?;
        let history_decode_workspace = decode_workspace_upper_bound(history_value_bytes)?;
        let rootset_peak = ROOT_BYTES
            .checked_add(decode_workspace_upper_bound(ROOT_BYTES)?)
            .and_then(|n| n.checked_add(rootset_result))
            .ok_or_else(|| invalid("V2 rootset state overflow"))?;
        // A history read retains the selected rootset while parsing one row,
        // then keeps one cache copy beside the caller's returned value.
        let history_peak = rootset_result
            .checked_add(history_value_bytes)
            .and_then(|n| n.checked_add(history_decode_workspace))
            .and_then(|n| n.checked_add(history_result.checked_mul(2)?))
            .ok_or_else(|| invalid("V2 history state overflow"))?;
        let selected_record_peak = rootset_peak.max(history_peak);
        let auxiliary_peak = 8usize
            .checked_mul(1024 * 1024)
            .and_then(|n| n.checked_add(BLOCK_BYTES))
            .ok_or_else(|| invalid("V2 point auxiliary state overflow"))?;
        self.caller_retained_state_bytes
            .checked_add(
                self.pointer
                    .max_manifest_bytes
                    .checked_mul(64)
                    .ok_or_else(|| invalid("V2 point state overflow"))?,
            )
            .and_then(|n| n.checked_add(self.tree.max_node_bytes.checked_mul(64)?))
            .and_then(|n| n.checked_add(self.max_object_bytes.checked_mul(2)?))
            .and_then(|n| n.checked_add(selected_record_peak))
            .and_then(|n| n.checked_add(auxiliary_peak))
            .ok_or_else(|| invalid("V2 point state overflow"))
    }
}

struct TreeIo(PinnedSqliteIoBudget);
impl AuthenticatedTreeIoLedgerV1 for TreeIo {
    fn charge_read(&self, n: u64) -> bool {
        self.0.charge_read(n).is_ok()
    }
    fn record_read_returned(&self, n: u64) -> bool {
        self.0.record_read_returned(n).is_ok()
    }
    fn charge_write(&self, n: u64) -> bool {
        self.0.charge_write(n).is_ok()
    }
    fn record_write_returned(&self, n: u64) -> bool {
        self.0.record_write_returned(n).is_ok()
    }
}

pub struct V2MemberObservation {
    pub revision: SourceRevision,
    pub path: RelativePath,
    pub sha256: Digest256,
    pub size_bytes: u64,
    pub source_mode: u32,
    pub bytes: Vec<u8>,
}

/// Exact authenticated membership tuple without reading the payload object.
/// Warm COW callers use this to compare unchanged members while keeping the
/// object read reserved for an actual consumer request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct V2MemberTupleObservation {
    pub revision: SourceRevision,
    pub path: RelativePath,
    pub sha256: Digest256,
    pub size_bytes: u64,
    pub source_mode: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum V2RootKind {
    Members,
    Identities,
    Dependencies,
    Retirements,
    History,
}

/// Read-only evidence that one exact batch/base/validator tuple is already
/// present in the currently selected authenticated history. The digest names
/// the current rootset that proved the retained row; it is not represented as
/// the historical publication's original pointer digest or receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AcceptedV2Publication {
    pub(crate) revision: SourceRevision,
    pub(crate) base_revision: Option<SourceRevision>,
    pub(crate) batch_sha256: Digest256,
    pub(crate) validator_sha256: Digest256,
    pub(crate) membership_v1: SourceMembershipV1,
    pub(crate) source_bytes: u64,
    pub(crate) member_count: u64,
    pub(crate) identity_count: u64,
    pub(crate) dependency_source_count: u64,
    pub(crate) dependency_count: u64,
    pub(crate) retirement_count: u64,
    pub(crate) source_artifact: super::source_admission_segment_v2::SourceRevisionArtifactV2,
    pub(crate) history_proof_rootset_sha256: Digest256,
}

/// One pinned immutable selected cut. Caller-owned IO/deadline/cancellation
/// are shared by all its lookups and actual object reads.
pub struct V2ReadSession {
    store: AdmissionStore,
    segment: SegmentStore,
    roots: SourceRootSetV2,
    history_roots_cache: Option<SourceRevisionRootsV2>,
    selection: CorpusCurrentSelection,
    limits: V2PointReadLimits,
    io: PinnedSqliteIoBudget,
    tree_io: Arc<dyn AuthenticatedTreeIoLedgerV1>,
    deadline: Instant,
    cancel: Arc<AtomicBool>,
    read_nodes: u64,
    tree_read_bytes: u64,
    failed: bool,
    observation: Option<V2MemberObservation>,
}

impl V2ReadSession {
    pub fn open(
        path: &Path,
        limits: V2PointReadLimits,
        original_io: PinnedSqliteIoBudget,
        deadline: Instant,
        cancel: Arc<AtomicBool>,
    ) -> io::Result<Self> {
        let store =
            AdmissionStore::open_existing_with_io(path, original_io.clone(), deadline, &cancel)?;
        Self::open_store(store, limits, original_io, deadline, cancel)
    }

    /// Open from an exact held root while checking its normalized name before
    /// and after the reader is constructed. Physical reads stay rooted at the
    /// supplied descriptor even if an unrelated path is later replaced.
    pub fn open_at_named(
        path: &Path,
        held_root: &File,
        limits: V2PointReadLimits,
        original_io: PinnedSqliteIoBudget,
        deadline: Instant,
        cancel: Arc<AtomicBool>,
    ) -> io::Result<Self> {
        Self::open_at_named_with_io(path, held_root, limits, original_io, deadline, cancel)
    }

    pub fn open_at_named_with_io(
        path: &Path,
        held_root: &File,
        limits: V2PointReadLimits,
        original_io: PinnedSqliteIoBudget,
        deadline: Instant,
        cancel: Arc<AtomicBool>,
    ) -> io::Result<Self> {
        let store = AdmissionStore::open_existing_at_named_with_io(
            path,
            held_root,
            original_io.clone(),
            deadline,
            &cancel,
        )?;
        Self::open_store(store, limits, original_io, deadline, cancel)
    }

    fn open_store(
        store: AdmissionStore,
        limits: V2PointReadLimits,
        original_io: PinnedSqliteIoBudget,
        deadline: Instant,
        cancel: Arc<AtomicBool>,
    ) -> io::Result<Self> {
        let limits = limits.validate()?;
        active(deadline, &cancel)?;
        let selection = store
            .current_selection(limits.pointer, deadline, &cancel, Some(&original_io))?
            .ok_or_else(|| invalid("V2 point selected revision absent"))?;
        if selection.format != CorpusPointerFormat::V2 {
            return Err(invalid("V2 point reader requires explicit V2 selection"));
        }
        let digest = selection
            .rootset_sha256
            .ok_or_else(|| invalid("V2 point rootset digest absent"))?;
        let raw = store.read_v2_rootset(
            selection.revision.0,
            digest,
            ROOT_BYTES,
            deadline,
            &cancel,
            &original_io,
        )?;
        let root_decode_workspace = decode_workspace_upper_bound(raw.len())?;
        let roots = SourceRootSetV2::decode_with_workspace(&raw, root_decode_workspace)?;
        if roots.current.revision != selection.revision
            || roots.current.base_revision != selection.previous
        {
            return Err(invalid("V2 point selector and roots differ"));
        }
        let (root, _, _) = store.backup_namespaces()?;
        let _existing =
            tos_fd_open::open_directory_at(root, Path::new("segments-v2")).map_err(invalid)?;
        let tree_io: Arc<dyn AuthenticatedTreeIoLedgerV1> = Arc::new(TreeIo(original_io.clone()));
        let segment = store.segment_store_v2_with_io(
            DOMAIN,
            limits.segment,
            tree_io.clone(),
            deadline,
            &cancel,
        )?;
        if segment.custody_domain() != DOMAIN {
            return Err(invalid("V2 point physical domain differs"));
        }
        roots.validate_store_binding(segment.store_id(), segment.domain_digest())?;
        let mut session = Self {
            store,
            segment,
            roots,
            history_roots_cache: None,
            selection,
            limits,
            io: original_io,
            tree_io,
            deadline,
            cancel,
            read_nodes: 0,
            tree_read_bytes: 0,
            failed: false,
            observation: None,
        };
        let history = session.roots.history.clone();
        let key = *session.selection.revision.0.as_bytes();
        let row = session
            .lookup(&history, &key)?
            .ok_or_else(|| invalid("V2 point current history row absent"))?;
        let history_decode_workspace = decode_workspace_upper_bound(row.len())?;
        session
            .roots
            .verify_current_history_row(&key, &row, history_decode_workspace)?;
        session.store.verify_layout()?;
        active(deadline, &session.cancel)?;
        Ok(session)
    }

    pub fn selected_revision(&self) -> SourceRevision {
        self.selection.revision
    }

    pub(crate) fn selected_selection(&self) -> CorpusCurrentSelection {
        self.selection.clone()
    }

    pub(crate) fn current_roots(&self) -> &SourceRevisionRootsV2 {
        &self.roots.current
    }

    pub(crate) fn current_rootset(&self) -> &SourceRootSetV2 {
        &self.roots
    }

    pub(crate) fn segment_store(&self) -> &SegmentStore {
        &self.segment
    }

    pub(crate) fn selected_rootset_sha256(&self) -> io::Result<Digest256> {
        self.selection
            .rootset_sha256
            .ok_or_else(|| invalid("V2 selected rootset digest absent"))
    }

    pub(crate) fn accumulated_tree_work(&self) -> AuthenticatedTreeWorkV1 {
        AuthenticatedTreeWorkV1 {
            read_nodes: self.read_nodes,
            read_bytes: self.tree_read_bytes,
            ..AuthenticatedTreeWorkV1::default()
        }
    }

    /// Actual retained reader values that remain live when a CMD COW writer
    /// reuses this selected session. Numeric profile headroom is checked by
    /// the caller before it clones the old descriptors or starts a delta.
    fn held_wrapper_state_bytes(&self) -> io::Result<usize> {
        let segment_state = self.segment.retained_heap_state_bytes().map_err(invalid)?;
        let tree_io_state = size_of::<TreeIo>()
            .checked_add(2 * size_of::<usize>())
            .ok_or_else(|| invalid("V2 reader IO wrapper state overflow"))?;
        size_of::<Self>()
            .checked_add(self.store.retained_path_capacity())
            .and_then(|bytes| bytes.checked_add(segment_state))
            .and_then(|bytes| bytes.checked_add(tree_io_state))
            .ok_or_else(|| invalid("V2 reader wrapper state overflow"))
    }

    pub(crate) fn retained_live_state_bytes(&self) -> io::Result<usize> {
        let roots_state = self.roots.retained_state_bytes()?;
        let mut retained = self
            .held_wrapper_state_bytes()?
            .checked_add(roots_state)
            .ok_or_else(|| invalid("V2 reader retained state overflow"))?;
        if let Some(cached) = &self.history_roots_cache {
            retained = retained
                .checked_add(cached.retained_state_bytes()?)
                .ok_or_else(|| invalid("V2 reader retained state overflow"))?;
        }
        if let Some(observation) = &self.observation {
            let observation_state = size_of::<V2MemberObservation>()
                .checked_add(observation.path.as_str().len())
                .and_then(|bytes| bytes.checked_add(observation.bytes.capacity()))
                .ok_or_else(|| invalid("V2 reader observation state overflow"))?;
            retained = retained
                .checked_add(observation_state)
                .ok_or_else(|| invalid("V2 reader observation state overflow"))?;
        }
        Ok(retained)
    }

    pub(crate) fn retained_history_count(&self) -> u64 {
        self.roots.history.entries
    }

    /// Recover an exact accepted transaction after a caller loses its reply.
    /// A match is returned only after the selected history reaches EOF, the
    /// immutable source-record file passes digest/size/EOF/stamp checks, and
    /// the selected mutable pointer still names this session's rootset.
    pub(crate) fn find_accepted_batch(
        &mut self,
        batch_sha256: Digest256,
        base_revision: Option<SourceRevision>,
        validator_sha256: Digest256,
        max_artifact_bytes: u64,
    ) -> io::Result<Option<AcceptedV2Publication>> {
        if self.failed {
            return Err(invalid("V2 point session already refused"));
        }
        let result = (|| {
            if max_artifact_bytes == 0 || max_artifact_bytes == u64::MAX {
                return Err(invalid("accepted V2 artifact read bound is invalid"));
            }
            let proof_rootset = self
                .selection
                .rootset_sha256
                .ok_or_else(|| invalid("accepted V2 history proof digest is absent"))?;
            let accepted_state = std::mem::size_of::<AcceptedV2Publication>()
                .checked_add(std::mem::size_of::<Option<[u8; 32]>>())
                .and_then(|bytes| bytes.checked_add(1024))
                .ok_or_else(|| invalid("accepted V2 result state overflow"))?;
            if self
                .limits
                .base_state_bytes()?
                .checked_add(accepted_state)
                .is_none_or(|bytes| bytes > self.limits.max_state_bytes)
            {
                return Err(invalid(
                    "accepted V2 lookup exceeds the original reader state slice",
                ));
            }
            let caller_result_state = self
                .history_root_result_state_upper_bound()?
                .checked_add(accepted_state)
                .ok_or_else(|| invalid("accepted V2 caller result state overflow"))?;
            let mut after: Option<[u8; 32]> = None;
            let mut accepted: Option<AcceptedV2Publication> = None;
            let mut ambiguous = false;
            let mut observed = 0u64;
            loop {
                let next = self.next_history_roots_after_with_retained(
                    after.as_ref().map(|key| key.as_slice()),
                    caller_result_state,
                    accepted_state,
                )?;
                let Some((roots, _raw_bytes)) = next else {
                    break;
                };
                observed = observed
                    .checked_add(1)
                    .filter(|count| *count <= self.roots.history.entries)
                    .ok_or_else(|| invalid("accepted V2 history row count exceeded"))?;
                let revision_key = *roots.revision.0.as_bytes();
                if roots.base_revision == base_revision
                    && roots.batch_sha256 == Some(batch_sha256)
                    && roots.validator_sha256 == validator_sha256
                {
                    if accepted.is_some() {
                        ambiguous = true;
                    } else {
                        let artifact = roots.source_artifact.clone();
                        if artifact
                            .bytes()
                            .is_none_or(|bytes| bytes == 0 || bytes > max_artifact_bytes)
                        {
                            return Err(invalid(
                                "accepted V2 history artifact exceeds selected read bound",
                            ));
                        }
                        accepted = Some(AcceptedV2Publication {
                            revision: roots.revision,
                            base_revision: roots.base_revision,
                            batch_sha256,
                            validator_sha256,
                            membership_v1: roots.membership_v1,
                            source_bytes: roots.source_bytes,
                            member_count: roots.member_count,
                            identity_count: roots.identity_count,
                            dependency_source_count: roots.dependency_source_count,
                            dependency_count: roots.dependency_count,
                            retirement_count: roots.retirement_count,
                            source_artifact: artifact,
                            history_proof_rootset_sha256: proof_rootset,
                        });
                    }
                }
                after = Some(revision_key);
            }
            if observed != self.roots.history.entries {
                return Err(invalid("accepted V2 history EOF count differs"));
            }
            // The lookup is deliberately not a prefix hit: only a completed
            // ordered walk and a stable selected pointer can recover success.
            self.verify_current_fence()?;
            if ambiguous {
                return Err(invalid(
                    "accepted V2 batch tuple is ambiguous in retained history",
                ));
            }
            if let Some(accepted) = accepted {
                self.store.verify_v2_source_artifact(
                    accepted.revision.0,
                    &accepted.source_artifact,
                    max_artifact_bytes,
                    self.deadline,
                    &self.cancel,
                    &self.io,
                )?;
                self.verify_current_fence()?;
                return Ok(Some(accepted));
            }
            Ok(None)
        })();
        if result.is_err() {
            self.failed = true;
        }
        result
    }

    pub(crate) fn history_root_result_state_upper_bound(&self) -> io::Result<usize> {
        SourceRevisionRootsV2::retained_state_upper_bound_for_value(
            self.limits.tree.max_value_bytes,
        )
    }

    pub(crate) fn identity_lookup_result_state_upper_bound(&self) -> io::Result<usize> {
        usize::try_from(self.limits.tree.max_value_bytes)
            .map_err(|_| invalid("V2 identity value bound exceeds address space"))?
            .checked_mul(4)
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<RelativePath>() + 1024))
            .ok_or_else(|| invalid("V2 identity result state overflow"))
    }

    pub(crate) fn shares_io_budget(&self, io: &PinnedSqliteIoBudget) -> bool {
        self.io.shares_with(io)
    }

    pub(crate) fn io_budget(&self) -> &PinnedSqliteIoBudget {
        &self.io
    }

    pub(crate) fn declared_retained_state_bytes(&self) -> io::Result<(usize, usize)> {
        self.limits
            .base_state_bytes()?
            .checked_add(self.held_wrapper_state_bytes()?)
            .map(|state| (state, 0))
            .ok_or_else(|| invalid("V2 point retained state overflow"))
    }

    pub(crate) fn roots_for_revision(
        &mut self,
        revision: SourceRevision,
    ) -> io::Result<Option<SourceRevisionRootsV2>> {
        self.revision_roots(revision)
    }

    pub(crate) fn next_history_roots_after(
        &mut self,
        after_revision: Option<&[u8]>,
        caller_result_state_bytes: usize,
    ) -> io::Result<Option<(SourceRevisionRootsV2, usize)>> {
        self.next_history_roots_after_with_retained(after_revision, caller_result_state_bytes, 0)
    }

    fn next_history_roots_after_with_retained(
        &mut self,
        after_revision: Option<&[u8]>,
        caller_result_state_bytes: usize,
        additional_caller_retained_state_bytes: usize,
    ) -> io::Result<Option<(SourceRevisionRootsV2, usize)>> {
        let selected_revision = self.selection.revision;
        let Some(row) = self.next_row_after_with_retained(
            selected_revision,
            V2RootKind::History,
            None,
            None,
            after_revision,
            additional_caller_retained_state_bytes,
        )?
        else {
            return Ok(None);
        };
        let result_state_upper =
            SourceRevisionRootsV2::retained_state_upper_bound_for_value(row.value.len())?;
        if result_state_upper > caller_result_state_bytes {
            return Err(invalid("V2 history root exceeds caller retained state"));
        }
        let workspace = decode_workspace_upper_bound(row.value.len())?;
        // Drop the previous one-entry cache before allocating the new parsed
        // value/cache pair. The session base-state reservation covers the
        // returned value and its one cache copy simultaneously.
        self.history_roots_cache = None;
        let roots = SourceRevisionRootsV2::decode_with_workspace(&row.value, workspace)?;
        if row.key.as_slice() != roots.revision.0.as_bytes() {
            return Err(invalid("V2 history key and revision differ"));
        }
        roots.validate_store_binding(self.segment.store_id(), self.segment.domain_digest())?;
        if roots.revision == self.roots.current.revision {
            self.roots
                .verify_current_history_row(&row.key, &row.value, workspace)?;
        } else {
            let result_state = roots.retained_state_bytes()?;
            if result_state > caller_result_state_bytes {
                return Err(invalid("V2 history root exceeds caller retained state"));
            }
            self.history_roots_cache = Some(roots.clone());
        }
        let raw_bytes = row.value.len();
        Ok(Some((roots, raw_bytes)))
    }

    pub(crate) fn identity_path(
        &mut self,
        revision: SourceRevision,
        id: &str,
    ) -> io::Result<Option<RelativePath>> {
        if id.is_empty() || id.len() > self.limits.tree.max_key_bytes {
            return Err(invalid("V2 base identity key exceeds profile"));
        }
        let Some(roots) = self.revision_roots(revision)? else {
            return Ok(None);
        };
        let Some(path) = self.lookup(&roots.identities, id.as_bytes())? else {
            return Ok(None);
        };
        RelativePath::parse(std::str::from_utf8(&path).map_err(invalid)?)
            .map_err(invalid)
            .map(Some)
    }

    /// Explicit mutable-pointer fence for a caller that requires a current
    /// view; exact-revision reads themselves use the retained immutable cut.
    pub fn verify_current_fence(&self) -> io::Result<()> {
        if self.store.current_selection(
            self.limits.pointer,
            self.deadline,
            &self.cancel,
            Some(&self.io),
        )? != Some(self.selection.clone())
        {
            return Err(invalid("V2 point current selection advanced"));
        }
        self.store.verify_layout()?;
        active(self.deadline, &self.cancel)
    }

    /// Return one row from an authenticated, ordered key interval. The
    /// Patricia seek prunes every child whose authenticated bounds cannot
    /// contain a row after `after_exclusive`; callers can continue by passing
    /// the returned key without restarting a full stream. The caller's point
    /// state profile supplies the transient node-stack allowance, and this
    /// session charges all actual node/byte work cumulatively.
    pub fn next_row_after(
        &mut self,
        revision: SourceRevision,
        kind: V2RootKind,
        lower_inclusive: Option<&[u8]>,
        upper_exclusive: Option<&[u8]>,
        after_exclusive: Option<&[u8]>,
    ) -> io::Result<Option<AuthenticatedTreeEntryV1>> {
        self.next_row_after_with_retained(
            revision,
            kind,
            lower_inclusive,
            upper_exclusive,
            after_exclusive,
            0,
        )
    }

    fn next_row_after_with_retained(
        &mut self,
        revision: SourceRevision,
        kind: V2RootKind,
        lower_inclusive: Option<&[u8]>,
        upper_exclusive: Option<&[u8]>,
        after_exclusive: Option<&[u8]>,
        additional_caller_retained_state_bytes: usize,
    ) -> io::Result<Option<AuthenticatedTreeEntryV1>> {
        if self.failed {
            return Err(invalid("V2 point session already refused"));
        }
        self.observation = None;
        let result = (|| {
            active(self.deadline, &self.cancel)?;
            let root = if kind == V2RootKind::History {
                if revision != self.selection.revision {
                    return Err(invalid("V2 history cursor revision differs"));
                }
                self.roots.history.clone()
            } else {
                let Some(roots) = self.revision_roots(revision)? else {
                    return Ok(None);
                };
                match kind {
                    V2RootKind::Members => roots.members,
                    V2RootKind::Identities => roots.identities,
                    V2RootKind::Dependencies => roots.dependencies,
                    V2RootKind::Retirements => roots.retirements,
                    V2RootKind::History => unreachable!(),
                }
            };
            let mut limits = self.limits.tree;
            limits.max_nodes = limits
                .max_nodes
                .checked_sub(self.read_nodes)
                .filter(|n| *n > 0)
                .ok_or_else(|| invalid("V2 point cumulative node allowance exceeded"))?;
            limits.max_total_bytes = limits
                .max_total_bytes
                .checked_sub(self.tree_read_bytes)
                .filter(|n| *n > 0)
                .ok_or_else(|| invalid("V2 point cumulative tree bytes exceeded"))?;
            let range_state = self
                .limits
                .max_state_bytes
                .checked_sub(self.limits.base_state_bytes()?)
                .and_then(|bytes| bytes.checked_sub(additional_caller_retained_state_bytes))
                .filter(|n| *n > 0)
                .ok_or_else(|| invalid("V2 point range state allowance absent"))?;
            let (row, work) = self
                .segment
                .lookup_authenticated_tree_v2_after_with_work_and_io(
                    &root,
                    lower_inclusive,
                    upper_exclusive,
                    after_exclusive,
                    limits,
                    range_state,
                    Some(self.tree_io.clone()),
                    self.deadline,
                    &self.cancel,
                )
                .map_err(invalid)?;
            self.record_work(work.read_nodes, work.read_bytes)?;
            self.store.verify_layout()?;
            active(self.deadline, &self.cancel)?;
            Ok(row)
        })();
        if result.is_err() {
            self.failed = true;
        }
        result
    }

    /// Seek one exact-revision member under the same cumulative node/IO
    /// ledger. The caller includes all still-live census and previous-result
    /// state in `caller_retained_state_bytes`; the newly returned path and
    /// transient raw row are reserved before the authenticated seek.
    pub(crate) fn next_member_tuple_after(
        &mut self,
        revision: SourceRevision,
        after_exclusive: Option<&[u8]>,
        caller_retained_state_bytes: usize,
    ) -> io::Result<Option<V2MemberTupleObservation>> {
        if self.failed {
            return Err(invalid("V2 point session already refused"));
        }
        self.observation = None;
        let result = (|| {
            let result_state = self
                .limits
                .tree
                .max_key_bytes
                .checked_mul(2)
                .and_then(|bytes| bytes.checked_add(size_of::<V2MemberTupleObservation>()))
                .and_then(|bytes| bytes.checked_add(256))
                .and_then(|bytes| bytes.checked_add(caller_retained_state_bytes))
                .ok_or_else(|| invalid("V2 member cursor retained state overflow"))?;
            if self
                .limits
                .base_state_bytes()?
                .checked_add(result_state)
                .is_none_or(|bytes| bytes >= self.limits.max_state_bytes)
            {
                return Err(invalid("V2 member cursor original state slice exhausted"));
            }
            if self.revision_roots(revision)?.is_none() {
                return Err(invalid("V2 member cursor original revision absent"));
            }
            let Some(row) = self.next_row_after_with_retained(
                revision,
                V2RootKind::Members,
                None,
                None,
                after_exclusive,
                result_state,
            )?
            else {
                return Ok(None);
            };
            let path = RelativePath::parse(std::str::from_utf8(&row.key).map_err(invalid)?)
                .map_err(invalid)?;
            decode_member_tuple(revision, path, &row.value, self.limits.max_object_bytes).map(Some)
        })();
        if result.is_err() {
            self.failed = true;
        }
        result
    }

    /// Point-read only the authenticated 44-byte member tuple. This does not
    /// read or verify the referenced payload object.
    pub fn member_tuple(
        &mut self,
        revision: SourceRevision,
        path: &RelativePath,
    ) -> io::Result<Option<V2MemberTupleObservation>> {
        if self.failed {
            return Err(invalid("V2 point session already refused"));
        }
        self.observation = None;
        let result = (|| {
            let Some(roots) = self.revision_roots(revision)? else {
                return Ok(None);
            };
            self.member_tuple_from_roots(&roots, path)
        })();
        if result.is_err() {
            self.failed = true;
        }
        result
    }

    fn lookup(
        &mut self,
        root: &AuthenticatedTreeDescriptorV2,
        key: &[u8],
    ) -> io::Result<Option<Vec<u8>>> {
        if self.failed {
            return Err(invalid("V2 point session already refused"));
        }
        active(self.deadline, &self.cancel)?;
        let mut limits = self.limits.tree;
        limits.max_nodes = limits
            .max_nodes
            .checked_sub(self.read_nodes)
            .filter(|n| *n > 0)
            .ok_or_else(|| invalid("V2 point cumulative node allowance exceeded"))?;
        limits.max_total_bytes = limits
            .max_total_bytes
            .checked_sub(self.tree_read_bytes)
            .filter(|n| *n > 0)
            .ok_or_else(|| invalid("V2 point cumulative tree bytes exceeded"))?;
        let lookup = self
            .segment
            .lookup_authenticated_tree_v2_with_work_and_io(
                root,
                key,
                limits,
                Some(self.tree_io.clone()),
                self.deadline,
                &self.cancel,
            )
            .map_err(invalid);
        let (value, work) = match lookup {
            Ok(value) => value,
            Err(error) => {
                self.failed = true;
                return Err(error);
            }
        };
        self.record_work(work.read_nodes, work.read_bytes)?;
        self.store.verify_layout()?;
        active(self.deadline, &self.cancel)?;
        Ok(value)
    }

    fn record_work(&mut self, nodes: u64, bytes: u64) -> io::Result<()> {
        self.read_nodes = self
            .read_nodes
            .checked_add(nodes)
            .filter(|n| *n <= self.limits.tree.max_nodes)
            .ok_or_else(|| invalid("V2 point cumulative node allowance exceeded"))?;
        self.tree_read_bytes = self
            .tree_read_bytes
            .checked_add(bytes)
            .filter(|n| *n <= self.limits.tree.max_total_bytes)
            .ok_or_else(|| invalid("V2 point cumulative tree bytes exceeded"))?;
        Ok(())
    }

    fn revision_roots(
        &mut self,
        revision: SourceRevision,
    ) -> io::Result<Option<SourceRevisionRootsV2>> {
        if revision == self.roots.current.revision {
            return Ok(Some(self.roots.current.clone()));
        }
        if let Some(cached) = self
            .history_roots_cache
            .as_ref()
            .filter(|roots| roots.revision == revision)
        {
            return Ok(Some(cached.clone()));
        }
        let history = self.roots.history.clone();
        let Some(raw) = self.lookup(&history, revision.0.as_bytes())? else {
            return Ok(None);
        };
        let workspace = decode_workspace_upper_bound(raw.len())?;
        // Replacing the one-entry cache must not overlap an obsolete cached
        // revision with both the decoded row and its new cached copy.
        self.history_roots_cache = None;
        let roots = SourceRevisionRootsV2::decode_with_workspace(&raw, workspace)?;
        if roots.revision != revision {
            return Err(invalid("V2 point history revision differs"));
        }
        roots.validate_store_binding(self.segment.store_id(), self.segment.domain_digest())?;
        self.history_roots_cache = Some(roots.clone());
        Ok(Some(roots))
    }

    pub fn read_member(
        &mut self,
        revision: SourceRevision,
        path: &RelativePath,
    ) -> io::Result<Option<&V2MemberObservation>> {
        // Keep one owned observation. A live borrowed result prevents another
        // mutable read; explicit caller copies belong to its retained state.
        self.observation = None;
        let Some(roots) = self.revision_roots(revision)? else {
            return Ok(None);
        };
        self.observation = self.read_from_roots(&roots, path)?;
        Ok(self.observation.as_ref())
    }

    pub fn read_identity(
        &mut self,
        revision: SourceRevision,
        id: &str,
    ) -> io::Result<Option<&V2MemberObservation>> {
        self.observation = None;
        if id.is_empty() || id.len() > self.limits.tree.max_key_bytes {
            return Err(invalid("V2 point identity key exceeds profile"));
        }
        let Some(roots) = self.revision_roots(revision)? else {
            return Ok(None);
        };
        let Some(path) = self.lookup(&roots.identities, id.as_bytes())? else {
            return Ok(None);
        };
        let path =
            RelativePath::parse(std::str::from_utf8(&path).map_err(invalid)?).map_err(invalid)?;
        self.observation = Some(
            self.read_from_roots(&roots, &path)?
                .ok_or_else(|| invalid("V2 point identity member absent"))?,
        );
        Ok(self.observation.as_ref())
    }

    fn read_from_roots(
        &mut self,
        roots: &SourceRevisionRootsV2,
        path: &RelativePath,
    ) -> io::Result<Option<V2MemberObservation>> {
        let Some(member) = self.member_tuple_from_roots(roots, path)? else {
            return Ok(None);
        };
        let digest = member.sha256;
        let size = member.size_bytes;
        let mode = member.source_mode;
        let (_, objects, _) = self.store.backup_namespaces()?;
        let name = digest.to_hex();
        let mut input = tos_fd_open::open_regular_at(objects, Path::new(&name)).map_err(invalid)?;
        let before = stamp(&input)?;
        let metadata = input.metadata()?;
        if metadata.uid() != rustix::process::geteuid().as_raw()
            || metadata.mode() & 0o222 != 0
            || metadata.len() != size
        {
            return Err(invalid("V2 point object custody differs"));
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(usize::try_from(size).map_err(invalid)?)
            .map_err(invalid)?;
        let mut block = [0u8; BLOCK_BYTES];
        let mut hash = Digest256Hasher::new();
        let mut remaining = size;
        while remaining > 0 {
            active(self.deadline, &self.cancel)?;
            let wanted = usize::try_from(remaining.min(BLOCK_BYTES as u64)).map_err(invalid)?;
            self.io.charge_read(wanted as u64).map_err(invalid)?;
            let read = match input.read(&mut block[..wanted]) {
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                value => value?,
            };
            self.io.record_read_returned(read as u64).map_err(invalid)?;
            if read == 0 {
                return Err(invalid("V2 point object early EOF"));
            }
            bytes.extend_from_slice(&block[..read]);
            hash.update(&block[..read]);
            remaining -= read as u64;
        }
        self.io.charge_read(1).map_err(invalid)?;
        let tail = input.read(&mut block[..1])?;
        self.io.record_read_returned(tail as u64).map_err(invalid)?;
        let named = tos_fd_open::open_regular_at(objects, Path::new(&name)).map_err(invalid)?;
        if tail != 0
            || hash.finalize() != digest
            || stamp(&input)? != before
            || stamp(&named)? != before
        {
            return Err(invalid("V2 point object digest or EOF differs"));
        }
        self.store.verify_layout()?;
        active(self.deadline, &self.cancel)?;
        Ok(Some(V2MemberObservation {
            revision: member.revision,
            path: member.path,
            sha256: digest,
            size_bytes: size,
            source_mode: mode,
            bytes,
        }))
    }

    fn member_tuple_from_roots(
        &mut self,
        roots: &SourceRevisionRootsV2,
        path: &RelativePath,
    ) -> io::Result<Option<V2MemberTupleObservation>> {
        let Some(row) = self.lookup(&roots.members, path.as_str().as_bytes())? else {
            return Ok(None);
        };
        decode_member_tuple(
            roots.revision,
            path.clone(),
            &row,
            self.limits.max_object_bytes,
        )
    }
}

/// One maintained decoder for point and keyset member observations.
fn decode_member_tuple(
    revision: SourceRevision,
    path: RelativePath,
    row: &[u8],
    max_object_bytes: usize,
) -> io::Result<V2MemberTupleObservation> {
    if row.len() != 44 {
        return Err(invalid("V2 point member tuple width differs"));
    }
    let digest = Digest256::from_bytes(row[..32].try_into().map_err(invalid)?);
    let size = u64::from_be_bytes(row[32..40].try_into().map_err(invalid)?);
    let mode = u32::from_le_bytes(row[40..44].try_into().map_err(invalid)?);
    if mode & !0o777 != 0 || size > max_object_bytes as u64 {
        return Err(invalid("V2 point object or mode exceeds profile"));
    }
    Ok(V2MemberTupleObservation {
        revision,
        path,
        sha256: digest,
        size_bytes: size,
        source_mode: mode,
    })
}

fn stamp(file: &File) -> io::Result<(u64, u64, u64, u32, u32, i64, i64, i64, i64)> {
    let m = file.metadata()?;
    Ok((
        m.dev(),
        m.ino(),
        m.len(),
        m.mode(),
        m.uid(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    ))
}
