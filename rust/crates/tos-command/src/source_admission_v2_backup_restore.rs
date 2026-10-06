//! Finite cold backup/fresh-restore of the private native V2 store.
//! A copied selector is installed last, after the independent copy passes
//! authenticated current/history closure checks. Bytes confer no admission.
use super::source_admission::{AdmissionWorkBudget, active, invalid};
use super::source_admission_packed_objects::{MAX_PACKED_OBJECT_FRAMES_V2, PackedObjectLocationV2};
use super::source_admission_segment_v2::{
    CompactCommitV2, MAX_COMPACT_COMMIT_V2_BYTES, SourceRevisionArtifactV2, SourceRevisionRootsV2,
    SourceRootSetV2, decode_workspace_upper_bound,
};
use super::source_admission_store::AdmissionStore;
use super::source_admission_v2_seen_pack::{
    V2SeenPackSpill, V2SeenPackSpillLimits, V2SeenPackSpillRequest, V2SeenPackSpillRequests,
    history_row_holder_upper_bound,
};
use rustix::fs::{AtFlags, FileType, Mode, OFlags, RawDir, RenameFlags};
use std::{
    fs::{File, Metadata, Permissions},
    io::{self, Read, Write},
    mem::{MaybeUninit, size_of},
    os::unix::{
        ffi::OsStrExt,
        fs::{MetadataExt, PermissionsExt},
    },
    path::Path,
    sync::{Arc, atomic::AtomicBool},
    time::Instant,
};
use tos_foundation::{Digest256, Digest256Hasher, RelativePath};
use tos_segment_store::{
    AuthenticatedTreeCoverageV1, AuthenticatedTreeIoLedgerV1, AuthenticatedTreeLimitsV1,
    AuthenticatedTreePackSetV2, AuthenticatedTreeRowStreamV2, AuthenticatedTreeWorkV1,
    SegmentLimits, SegmentStore,
};
use tos_source_store::{
    CorpusCurrentSelection, CorpusPointerFormat, PinnedSqliteIoBudget, PinnedSqliteSpaceBudget,
    PinnedSqliteSpaceReservation, ReadLimits,
};

const DOMAIN: &[u8] = b"tos-native-admission-source-v2";
const ROOT_BYTES: usize = 65_536;
const BLOCK_BYTES: usize = 65_536;
const NAME_METADATA_READ_GUARD_BYTES: usize = 4096;
const MAX_HELD_ROOT_PATH_BYTES: usize = 4096;
const MAX_HELD_ROOT_PATH_COMPONENTS: usize = 128;

fn rootset_decode_state_upper_bound() -> io::Result<usize> {
    let workspace = decode_workspace_upper_bound(ROOT_BYTES)?;
    let one_value = size_of::<SourceRootSetV2>()
        // A decoded current tuple retains several owned descriptor/key
        // buffers; use the segment owner’s explicit result envelope. The
        // history descriptor is another bounded owned value from this row.
        .checked_add(SourceRevisionRootsV2::retained_state_upper_bound_for_value(
            ROOT_BYTES,
        )?)
        .and_then(|bytes| bytes.checked_add(ROOT_BYTES))
        .ok_or_else(|| invalid("V2 rootset retained state overflow"))?;
    ROOT_BYTES
        .checked_add(workspace)
        .and_then(|bytes| bytes.checked_add(one_value.checked_mul(2)?))
        .ok_or_else(|| invalid("V2 rootset decode state overflow"))
}

fn compact_decode_state_upper_bound() -> io::Result<usize> {
    let raw = MAX_COMPACT_COMMIT_V2_BYTES;
    raw.checked_add(decode_workspace_upper_bound(raw)?)
        .and_then(|bytes| bytes.checked_add(size_of::<CompactCommitV2>().checked_add(raw)?))
        .ok_or_else(|| invalid("V2 compact commit decode state overflow"))
}

#[derive(Clone, Copy)]
pub(crate) struct V2HeldSourceRoot<'a> {
    pub(crate) path: &'a Path,
    pub(crate) held: &'a File,
    pub(crate) identity: (u64, u64),
}

#[derive(Clone, Copy)]
pub(crate) struct V2HeldTargetRoot<'a> {
    pub(crate) artifact_path: &'a Path,
    pub(crate) artifact_held: &'a File,
    pub(crate) artifact_identity: (u64, u64),
    pub(crate) store_path: &'a Path,
    pub(crate) store_relative: &'a RelativePath,
}

#[derive(Clone, Copy)]
pub(crate) struct V2HeldImageRoots<'a> {
    pub(crate) source: V2HeldSourceRoot<'a>,
    pub(crate) target: V2HeldTargetRoot<'a>,
    pub(crate) target_store_identity: (u64, u64),
}

fn identity(file: &File) -> io::Result<(u64, u64)> {
    let metadata = file.metadata()?;
    Ok((metadata.dev(), metadata.ino()))
}

pub(crate) fn debit_root_guard(io: &PinnedSqliteIoBudget) -> io::Result<()> {
    io.charge_read_upper_bound(
        tos_compiler::private_tmpfs_stage::PRIVATE_TMPFS_VERIFY_COST.read_bytes,
    )
    .map_err(invalid)
}

pub(crate) fn debit_name_resolution(io: &PinnedSqliteIoBudget, name: &str) -> io::Result<()> {
    let upper = u64::try_from(
        name.len()
            .checked_add(1)
            .and_then(|bytes| bytes.checked_add(NAME_METADATA_READ_GUARD_BYTES))
            .ok_or_else(|| invalid("V2 path-component guard overflow"))?,
    )
    .map_err(invalid)?;
    io.charge_read_upper_bound(upper).map_err(invalid)
}

fn debit_directory_read(io: &PinnedSqliteIoBudget) -> io::Result<()> {
    // RawDir uses this exact fixed buffer for one getdents refill, including
    // its final EOF probe. This upper is separate from returned file payload.
    io.charge_read_upper_bound(8192).map_err(invalid)
}

fn validate_named_root_path(path: &Path) -> io::Result<()> {
    let bytes = path.as_os_str().as_bytes();
    if !path.is_absolute()
        || bytes.is_empty()
        || bytes.len() > MAX_HELD_ROOT_PATH_BYTES
        || path.components().count() > MAX_HELD_ROOT_PATH_COMPONENTS
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::CurDir | std::path::Component::ParentDir
            )
        })
    {
        return Err(invalid("V2 held root path is not bounded and normalized"));
    }
    Ok(())
}

fn verify_named_root_no_charge(path: &Path, held: &File, expected: (u64, u64)) -> io::Result<()> {
    validate_named_root_path(path)?;
    if identity(held)? != expected {
        return Err(invalid("V2 held root identity differs"));
    }
    let named = tos_fd_open::open_absolute_directory(path).map_err(invalid)?;
    if identity(&named)? != expected {
        return Err(invalid("V2 named root identity differs"));
    }
    Ok(())
}

pub(crate) fn verify_named_root(
    path: &Path,
    held: &File,
    expected: (u64, u64),
    io: &PinnedSqliteIoBudget,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<()> {
    debit_root_guard(io)?;
    active(deadline, cancel)?;
    verify_named_root_no_charge(path, held, expected)?;
    active(deadline, cancel)
}

fn validate_target_relative(path: &RelativePath) -> io::Result<()> {
    if path.as_str().len() > MAX_HELD_ROOT_PATH_BYTES
        || path.as_str().split('/').count() > MAX_HELD_ROOT_PATH_COMPONENTS
    {
        return Err(invalid("V2 selected target relative path exceeds bound"));
    }
    Ok(())
}

fn resolve_target_relative(
    root: &File,
    relative: &RelativePath,
    io: &PinnedSqliteIoBudget,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<File> {
    validate_target_relative(relative)?;
    let mut current = root.try_clone()?;
    for component in relative.as_str().split('/') {
        active(deadline, cancel)?;
        debit_name_resolution(io, component)?;
        current =
            tos_fd_open::open_directory_at(&current, Path::new(component)).map_err(invalid)?;
    }
    Ok(current)
}

pub(crate) fn open_selected_target(
    target: V2HeldTargetRoot<'_>,
    expected_target_identity: Option<(u64, u64)>,
    io: &PinnedSqliteIoBudget,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<File> {
    validate_named_root_path(target.artifact_path)?;
    validate_named_root_path(target.store_path)?;
    validate_target_relative(target.store_relative)?;
    if target.store_path.strip_prefix(target.artifact_path).ok()
        != Some(Path::new(target.store_relative.as_str()))
    {
        return Err(invalid("V2 selected target lexical root binding differs"));
    }
    debit_root_guard(io)?;
    active(deadline, cancel)?;
    verify_named_root_no_charge(
        target.artifact_path,
        target.artifact_held,
        target.artifact_identity,
    )?;
    let opened = resolve_target_relative(
        target.artifact_held,
        target.store_relative,
        io,
        deadline,
        cancel,
    )?;
    let opened_identity = identity(&opened)?;
    if expected_target_identity.is_some_and(|expected| expected != opened_identity) {
        return Err(invalid("V2 selected target identity changed"));
    }
    active(deadline, cancel)?;
    Ok(opened)
}

fn directory_is_empty(
    directory: &File,
    io: &PinnedSqliteIoBudget,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<bool> {
    let mut buffer = [MaybeUninit::uninit(); 8192];
    let mut entries = RawDir::new(directory, &mut buffer);
    loop {
        if entries.is_buffer_empty() {
            active(deadline, cancel)?;
            debit_directory_read(io)?;
        }
        match entries.next() {
            None => return Ok(true),
            Some(Err(error)) => return Err(error.into()),
            Some(Ok(entry)) => {
                let name = entry.file_name().to_str().map_err(invalid)?;
                if name != "." && name != ".." {
                    return Ok(false);
                }
            }
        }
    }
}

/// Bounds are slices of an owning caller's operation, not grants or resets.
#[derive(Clone, Copy)]
pub struct V2ImageLimits {
    pub reader: ReadLimits,
    pub segment: SegmentLimits,
    pub tree: AuthenticatedTreeLimitsV1,
    pub max_history_roots: usize,
    pub max_files: u64,
    pub max_directories: u64,
    pub max_depth: usize,
    pub max_state_bytes: usize,
    pub max_new_allocated_bytes: u64,
    /// Streaming compatibility bytes; separate from pointer/JSON RAM limits.
    pub max_compatibility_manifest_bytes: u64,
    /// Owner-admitted target filesystem allocation quantum; no capacity grant.
    pub allocation_unit_bytes: u64,
}

impl V2ImageLimits {
    fn validate(self) -> io::Result<Self> {
        let (limits, nodes, retained) = self.validate_layout()?;
        let local_set = nodes
            .checked_mul(256)
            .ok_or_else(|| invalid("V2 image state overflow"))?;
        if retained
            .checked_add(local_set)
            .is_none_or(|required| required > limits.max_state_bytes)
        {
            return Err(invalid("V2 image simultaneous state allowance exceeded"));
        }
        Ok(limits)
    }

    pub(crate) fn validate_cold_spill(
        self,
        requests: &V2SeenPackSpillRequests,
    ) -> io::Result<(Self, V2SeenPackSpillLimits, V2SeenPackSpillLimits)> {
        let (limits, nodes, base_state) = self.validate_layout_with_spilled_history()?;
        let retained = base_state
            .checked_add(size_of::<V2ImageColdSpillPlan>())
            .ok_or_else(|| invalid("V2 image cold-spill state overflow"))?;
        let max_tree_nodes = u64::try_from(nodes).map_err(invalid)?;
        let max_history_roots = u64::try_from(limits.max_history_roots).map_err(invalid)?;
        let max_pack_frames = limits.segment.max_frames.min(MAX_PACKED_OBJECT_FRAMES_V2);
        let profile = |request: &V2SeenPackSpillRequest| V2SeenPackSpillLimits {
            max_tree_nodes,
            max_history_roots,
            max_object_rows: limits.tree.max_rows,
            max_pack_frames,
            max_identity_key_bytes: limits.tree.max_key_bytes,
            max_history_row_bytes: SourceRevisionRootsV2::MAX_ENCODED_BYTES,
            cache_bytes: request.cache_bytes,
            max_operation_state_bytes: limits.max_state_bytes,
            retained_operation_state_bytes: retained,
            sqlite_native_overhead_bytes: request.sqlite_native_overhead_bytes,
        };
        let source_profile = profile(&requests.source);
        let target_profile = profile(&requests.target);
        let source_charge = source_profile.state_charge()?;
        let target_charge = target_profile.state_charge()?;
        if retained
            .checked_add(source_charge.max(target_charge))
            .is_none_or(|required| required > limits.max_state_bytes)
        {
            return Err(invalid("V2 cold spill exceeds held image state bill"));
        }
        Ok((limits, source_profile, target_profile))
    }

    fn validate_layout(self) -> io::Result<(Self, usize, usize)> {
        self.validate_layout_inner(false)
    }

    fn validate_layout_with_spilled_history(self) -> io::Result<(Self, usize, usize)> {
        self.validate_layout_inner(true)
    }

    fn validate_layout_inner(self, history_spilled: bool) -> io::Result<(Self, usize, usize)> {
        self.reader.validate().map_err(invalid)?;
        self.segment.validate().map_err(invalid)?;
        if self.max_history_roots == 0
            || self.max_history_roots == usize::MAX
            || self.max_files == 0
            || self.max_files == u64::MAX
            || self.max_directories == 0
            || self.max_directories == u64::MAX
            || self.max_depth == 0
            || self.max_depth > 64
            || self.max_state_bytes == 0
            || self.max_state_bytes == usize::MAX
            || self.max_new_allocated_bytes == 0
            || self.max_new_allocated_bytes == u64::MAX
            || self.max_compatibility_manifest_bytes == 0
            || self.max_compatibility_manifest_bytes == u64::MAX
            || self.allocation_unit_bytes == 0
            || self.allocation_unit_bytes == u64::MAX
            || self.tree.max_nodes == 0
            || self.tree.max_nodes == u64::MAX
            || self.tree.max_rows == 0
            || self.tree.max_rows == u64::MAX
            || self.tree.max_total_bytes == 0
            || self.tree.max_total_bytes == u64::MAX
        {
            return Err(invalid("V2 image finite profile differs"));
        }
        // Shared root/path/history/JSON/copy residency remains charged in both
        // modes. The caller selects either the local-set term or a cold spill.
        let nodes = usize::try_from(self.tree.max_nodes).map_err(invalid)?;
        let path_nodes = self
            .tree
            .max_key_bytes
            .checked_mul(2)
            .and_then(|n| n.checked_add(1))
            .ok_or_else(|| invalid("V2 image state overflow"))?
            .min(nodes);
        let history_resident = if history_spilled {
            0
        } else {
            let per_revision =
                SourceRevisionRootsV2::retained_state_upper_bound_for_value(ROOT_BYTES)?;
            self.max_history_roots
                .checked_mul(per_revision)
                .and_then(|bytes| bytes.checked_add(size_of::<Vec<SourceRevisionRootsV2>>()))
                .ok_or_else(|| invalid("V2 image history state overflow"))?
        };
        // Both source and restored rootsets remain live during their exact
        // comparison. The second parse is the peak: one raw rootset, its
        // caller-reserved workspace, and both retained typed values.
        let rootset_decode = rootset_decode_state_upper_bound()?;
        let compact_decode = compact_decode_state_upper_bound()?;
        // The compatibility history vector is still explicitly precharged
        // above. It also needs one bounded raw row and decoder workspace while
        // each typed value is constructed. Cold mode accounts this peak in the
        // spill bill with the SQLite raw-row clone and typed output.
        let compatibility_history_decode = if history_spilled {
            0
        } else {
            let raw = SourceRevisionRootsV2::MAX_ENCODED_BYTES;
            history_row_holder_upper_bound(raw)?
                .checked_add(decode_workspace_upper_bound(raw)?)
                .ok_or_else(|| invalid("V2 compatibility history decode overflow"))?
        };
        let retained = path_nodes
            .checked_mul(self.tree.max_node_bytes)
            .and_then(|n| n.checked_mul(64))
            .and_then(|n| n.checked_add(history_resident))
            .and_then(|n| n.checked_add(rootset_decode))
            .and_then(|n| n.checked_add(compact_decode))
            .and_then(|n| n.checked_add(compatibility_history_decode))
            .and_then(|n| n.checked_add(size_of::<V2ColdHistoryCursorState>()))
            .and_then(|n| n.checked_add(self.max_depth.checked_mul(8192)?))
            .and_then(|n| n.checked_add(self.reader.max_manifest_bytes.checked_mul(64)?))
            .and_then(|n| n.checked_add(8 * 1024 * 1024 + 2 * BLOCK_BYTES))
            .ok_or_else(|| invalid("V2 image state overflow"))?;
        Ok((self, nodes, retained))
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

#[derive(Default)]
struct Work {
    tree_nodes: u64,
    tree_bytes: u64,
    tree_rows: u64,
    files: u64,
    directories: u64,
    allocated: u64,
    allocation_upper: u64,
    shared_work: Option<AdmissionWorkBudget>,
}

#[derive(Default)]
struct V2ColdHistoryCursorState {
    row_count: u64,
    current_seen: bool,
    after_revision: Option<[u8; 32]>,
    processed: u64,
}
impl Work {
    fn reserve_allocation_upper(&mut self, bytes: u64, limits: V2ImageLimits) -> io::Result<()> {
        let unit = limits.allocation_unit_bytes;
        let upper = bytes
            .checked_add(unit - 1)
            .and_then(|n| n.checked_div(unit))
            .and_then(|n| n.checked_mul(unit))
            .and_then(|n| n.checked_add(unit))
            .ok_or_else(|| invalid("V2 image allocation precharge overflow"))?;
        self.allocation_upper = self
            .allocation_upper
            .checked_add(upper)
            .filter(|n| *n <= limits.max_new_allocated_bytes)
            .ok_or_else(|| invalid("V2 image allocation precharge refused"))?;
        Ok(())
    }
    fn tree_limits(&self, limits: V2ImageLimits) -> io::Result<AuthenticatedTreeLimitsV1> {
        let mut tree = limits.tree;
        tree.max_nodes = tree
            .max_nodes
            .checked_sub(self.tree_nodes)
            .filter(|n| *n > 0)
            .ok_or_else(|| invalid("V2 image cumulative tree work exceeded"))?;
        tree.max_total_bytes = tree
            .max_total_bytes
            .checked_sub(self.tree_bytes)
            .filter(|n| *n > 0)
            .ok_or_else(|| invalid("V2 image cumulative tree bytes exceeded"))?;
        tree.max_rows = tree
            .max_rows
            .checked_sub(self.tree_rows)
            .filter(|n| *n > 0)
            .ok_or_else(|| invalid("V2 image cumulative tree rows exceeded"))?;
        Ok(tree)
    }
    fn record_tree(
        &mut self,
        work: AuthenticatedTreeWorkV1,
        rows: u64,
        limits: V2ImageLimits,
    ) -> io::Result<()> {
        self.tree_nodes = self
            .tree_nodes
            .checked_add(work.read_nodes)
            .filter(|n| *n <= limits.tree.max_nodes)
            .ok_or_else(|| invalid("V2 image cumulative tree work exceeded"))?;
        self.tree_bytes = self
            .tree_bytes
            .checked_add(work.read_bytes)
            .filter(|n| *n <= limits.tree.max_total_bytes)
            .ok_or_else(|| invalid("V2 image cumulative tree bytes exceeded"))?;
        self.tree_rows = self
            .tree_rows
            .checked_add(rows)
            .filter(|n| *n <= limits.tree.max_rows)
            .ok_or_else(|| invalid("V2 image cumulative tree rows exceeded"))?;
        Ok(())
    }

    fn shared_work(&self) -> Option<&AdmissionWorkBudget> {
        self.shared_work.as_ref()
    }
}

fn stamp(m: &Metadata) -> (u64, u64, u64, u32, u32, i64, i64, i64, i64) {
    (
        m.dev(),
        m.ino(),
        m.len(),
        m.mode(),
        m.uid(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    )
}

fn selected_roots(
    store: &AdmissionStore,
    selection: &CorpusCurrentSelection,
    limits: V2ImageLimits,
    io: &PinnedSqliteIoBudget,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<SourceRootSetV2> {
    if selection.format != CorpusPointerFormat::V2 {
        return Err(invalid("V2 image requires a V2 selector"));
    }
    let sha = selection
        .rootset_sha256
        .ok_or_else(|| invalid("V2 image rootset digest absent"))?;
    let raw = store.read_v2_rootset(selection.revision.0, sha, ROOT_BYTES, deadline, cancel, io)?;
    let workspace = decode_workspace_upper_bound(raw.len())?;
    let roots = SourceRootSetV2::decode_with_workspace(&raw, workspace)?;
    if roots.current.revision != selection.revision
        || roots.current.base_revision != selection.previous
    {
        return Err(invalid("V2 image selector and rootset differ"));
    }
    Ok(roots)
}

fn verify_tree_v2(
    segment: &SegmentStore,
    descriptor: &tos_segment_store::AuthenticatedTreeDescriptorV2,
    limits: AuthenticatedTreeLimitsV1,
    tree_io: Arc<dyn AuthenticatedTreeIoLedgerV1>,
    pack_set: Option<&Arc<V2SeenPackSpill>>,
    closure_binding: Digest256,
    shared_work: Option<&AdmissionWorkBudget>,
    deadline: Instant,
    cancel: &AtomicBool,
) -> tos_segment_store::Result<AuthenticatedTreeCoverageV1> {
    if let Some(pack_set) = pack_set {
        let pack_set: Arc<dyn AuthenticatedTreePackSetV2> = pack_set.clone();
        if let Some(shared_work) = shared_work {
            let mut debit = || shared_work.charge(()).is_ok();
            segment.verify_authenticated_tree_v2_with_pack_set_and_work_callback(
                descriptor,
                limits,
                Some(tree_io),
                pack_set,
                closure_binding,
                deadline,
                cancel,
                &mut debit,
            )
        } else {
            segment.verify_authenticated_tree_v2_with_pack_set(
                descriptor,
                limits,
                Some(tree_io),
                pack_set,
                closure_binding,
                deadline,
                cancel,
            )
        }
    } else {
        segment.verify_authenticated_tree_v2_with_io(
            descriptor,
            limits,
            Some(tree_io),
            deadline,
            cancel,
        )
    }
}

fn next_tree_row(
    stream: &mut AuthenticatedTreeRowStreamV2,
    shared_work: Option<&AdmissionWorkBudget>,
    deadline: Instant,
    cancel: &AtomicBool,
) -> tos_segment_store::Result<Option<tos_segment_store::AuthenticatedTreeEntryV1>> {
    if let Some(shared_work) = shared_work {
        let mut debit = || shared_work.charge(()).is_ok();
        stream.next_row_with_work_callback(deadline, cancel, &mut debit)
    } else {
        stream.next_row(deadline, cancel)
    }
}

fn verify_closure(
    store: &AdmissionStore,
    roots: &SourceRootSetV2,
    limits: V2ImageLimits,
    io: &PinnedSqliteIoBudget,
    work: &mut Work,
    closure_binding: Digest256,
    cold_spill: Option<(V2SeenPackSpillRequest, V2SeenPackSpillLimits)>,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<()> {
    let (root, _, _) = store.backup_namespaces()?;
    // The writer's selector helper may initialize a missing namespace. A
    // backup/restore verifier must refuse its absence without mutating it.
    debit_name_resolution(io, "segments-v2")?;
    let _existing =
        tos_fd_open::open_directory_at(root, Path::new("segments-v2")).map_err(invalid)?;
    let tree_io: Arc<dyn AuthenticatedTreeIoLedgerV1> = Arc::new(TreeIo(io.clone()));
    let segment = store.segment_store_v2_with_io(
        DOMAIN,
        limits.segment,
        tree_io.clone(),
        deadline,
        cancel,
    )?;
    if segment.custody_domain() != DOMAIN {
        return Err(invalid("V2 image physical domain differs"));
    }
    roots.validate_store_binding(segment.store_id(), segment.domain_digest())?;
    let max_history_roots = u64::try_from(limits.max_history_roots).map_err(invalid)?;
    if roots.history.entries > max_history_roots {
        return Err(invalid("V2 image retained history bound exceeded"));
    }
    let pack_set = if let Some((request, mut spill_limits)) = cold_spill {
        spill_limits.max_tree_nodes = work.tree_limits(limits)?.max_nodes;
        Some(V2SeenPackSpill::open(
            request.workspace,
            request.request,
            segment.physical_root_identity().map_err(invalid)?,
            segment.store_id(),
            segment.domain_digest(),
            closure_binding,
            spill_limits,
        )?)
    } else {
        None
    };
    let coverage = verify_tree_v2(
        &segment,
        &roots.history,
        work.tree_limits(limits)?,
        tree_io.clone(),
        pack_set.as_ref(),
        closure_binding,
        work.shared_work(),
        deadline,
        cancel,
    )
    .map_err(invalid)?;
    work.record_tree(coverage.work, coverage.entries, limits)?;
    let mut stream = segment
        .stream_authenticated_tree_v2_with_io(
            &roots.history,
            work.tree_limits(limits)?,
            Some(tree_io.clone()),
        )
        .map_err(invalid)?;
    let mut history = if pack_set.is_some() {
        None
    } else {
        Some(Vec::new())
    };
    let mut history_cursor = V2ColdHistoryCursorState::default();
    while let Some(row) =
        next_tree_row(&mut stream, work.shared_work(), deadline, cancel).map_err(invalid)?
    {
        active(deadline, cancel)?;
        history_cursor.row_count = history_cursor
            .row_count
            .checked_add(1)
            .filter(|count| *count <= max_history_roots)
            .ok_or_else(|| invalid("V2 image retained history bound exceeded"))?;
        if history
            .as_ref()
            .is_some_and(|history| history.len() >= limits.max_history_roots)
        {
            return Err(invalid("V2 image retained history bound exceeded"));
        }
        let workspace = decode_workspace_upper_bound(row.value.len())?;
        let revision = SourceRevisionRootsV2::decode_with_workspace(&row.value, workspace)?;
        if row.key.as_slice() != revision.revision.0.as_bytes() {
            return Err(invalid("V2 image history key differs"));
        }
        revision.validate_store_binding(segment.store_id(), segment.domain_digest())?;
        if revision.revision == roots.current.revision {
            if history_cursor.current_seen {
                return Err(invalid("V2 image repeated current history key"));
            }
            if revision != roots.current {
                return Err(invalid("V2 image current history row differs"));
            }
            history_cursor.current_seen = true;
        }
        if let Some(spill) = &pack_set {
            spill.observe_history(
                *revision.revision.0.as_bytes(),
                revision.base_revision.map(|base| *base.0.as_bytes()),
                &row.value,
            )?;
        } else if let Some(history) = &mut history {
            history.try_reserve(1).map_err(invalid)?;
            history.push(revision);
        }
    }
    let coverage = stream
        .coverage()
        .ok_or_else(|| invalid("V2 image history EOF absent"))?;
    work.record_tree(coverage.work, coverage.entries, limits)?;
    drop(stream);
    if !history_cursor.current_seen {
        return Err(invalid(
            "V2 image current absent from authenticated history",
        ));
    }
    if let Some(history) = history {
        // Compatibility mode keeps its explicitly precharged finite vector.
        // Cold mode performs the same missing-base/cycle checks in SQLite.
        for revision in &history {
            let mut next = revision.base_revision;
            let mut hops = 0usize;
            while let Some(base) = next {
                active(deadline, cancel)?;
                hops = hops
                    .checked_add(1)
                    .filter(|n| *n <= history.len())
                    .ok_or_else(|| invalid("V2 image retained history cycle"))?;
                let index = history
                    .binary_search_by_key(&base.0, |row| row.revision.0)
                    .map_err(|_| invalid("V2 image retained history base absent"))?;
                next = history[index].base_revision;
            }
        }
        for revision in history {
            verify_revision_closure(
                store,
                &segment,
                &revision,
                limits,
                io,
                work,
                tree_io.clone(),
                pack_set.as_ref(),
                closure_binding,
                deadline,
                cancel,
            )?;
        }
    } else {
        let spill = pack_set
            .as_ref()
            .ok_or_else(|| invalid("V2 cold history spill is absent"))?;
        spill.seal_history(*roots.current.revision.0.as_bytes(), roots.history.entries)?;
        while let Some(row) = spill.next_history(history_cursor.after_revision)? {
            active(deadline, cancel)?;
            if history_cursor
                .after_revision
                .is_some_and(|previous| row.revision <= previous)
            {
                return Err(invalid("V2 cold history keyset cursor did not advance"));
            }
            let workspace = decode_workspace_upper_bound(row.raw.len())?;
            let revision = SourceRevisionRootsV2::decode_with_workspace(&row.raw, workspace)?;
            if row.revision.as_slice() != revision.revision.0.as_bytes()
                || row.base_revision != revision.base_revision.map(|base| *base.0.as_bytes())
            {
                return Err(invalid("V2 cold history spill row binding differs"));
            }
            revision.validate_store_binding(segment.store_id(), segment.domain_digest())?;
            verify_revision_closure(
                store,
                &segment,
                &revision,
                limits,
                io,
                work,
                tree_io.clone(),
                pack_set.as_ref(),
                closure_binding,
                deadline,
                cancel,
            )?;
            history_cursor.processed = history_cursor
                .processed
                .checked_add(1)
                .filter(|count| *count <= max_history_roots)
                .ok_or_else(|| invalid("V2 cold history processed row bound exceeded"))?;
            history_cursor.after_revision = Some(row.revision);
        }
        spill.finish_history(history_cursor.processed)?;
        if spill.has_packed_revisions() {
            let shared_work = work
                .shared_work()
                .cloned()
                .ok_or_else(|| invalid("V2 packed image lacks the original shared work meter"))?;
            let caller_state = spill.packed_verifier_caller_state_bytes()?;
            spill.verify_packed_payload_segments(
                &segment,
                limits.segment,
                tree_io.clone(),
                limits.max_state_bytes,
                caller_state,
                shared_work,
                deadline,
                cancel,
            )?;
        }
    }
    store.verify_layout()?;
    active(deadline, cancel)
}

#[allow(clippy::too_many_arguments)]
fn verify_revision_closure(
    store: &AdmissionStore,
    segment: &SegmentStore,
    revision: &SourceRevisionRootsV2,
    limits: V2ImageLimits,
    io: &PinnedSqliteIoBudget,
    work: &mut Work,
    tree_io: Arc<dyn AuthenticatedTreeIoLedgerV1>,
    pack_set: Option<&Arc<V2SeenPackSpill>>,
    closure_binding: Digest256,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<()> {
    let packed = revision.objects.as_ref();
    if packed.is_some() && (pack_set.is_none() || work.shared_work().is_none()) {
        return Err(invalid(
            "packed V2 closure requires a cold spill and original shared work meter",
        ));
    }
    let (_, _, revisions) = store.backup_namespaces()?;
    let revision_name = revision.revision.0.to_hex();
    debit_name_resolution(io, &revision_name)?;
    let directory =
        tos_fd_open::open_directory_at(revisions, Path::new(&revision_name)).map_err(invalid)?;
    verify_revision_artifact(&directory, revision, limits, io, deadline, cancel)?;
    for root in [
        &revision.members,
        &revision.identities,
        &revision.dependencies,
        &revision.retirements,
    ] {
        let coverage = verify_tree_v2(
            segment,
            root,
            work.tree_limits(limits)?,
            tree_io.clone(),
            pack_set,
            closure_binding,
            work.shared_work(),
            deadline,
            cancel,
        )
        .map_err(invalid)?;
        work.record_tree(coverage.work, coverage.entries, limits)?;
    }
    if let Some(identity_paths) = revision.identity_paths.as_ref() {
        let coverage = verify_tree_v2(
            segment,
            identity_paths,
            work.tree_limits(limits)?,
            tree_io.clone(),
            pack_set,
            closure_binding,
            work.shared_work(),
            deadline,
            cancel,
        )
        .map_err(invalid)?;
        work.record_tree(coverage.work, coverage.entries, limits)?;
    }
    if let Some(objects_root) = packed {
        let coverage = verify_tree_v2(
            segment,
            objects_root,
            work.tree_limits(limits)?,
            tree_io.clone(),
            pack_set,
            closure_binding,
            work.shared_work(),
            deadline,
            cancel,
        )
        .map_err(invalid)?;
        work.record_tree(coverage.work, coverage.entries, limits)?;
    }
    let (_, objects, _) = store.backup_namespaces()?;
    let revision_id = *revision.revision.0.as_bytes();
    if let (Some(spill), Some(identity_paths)) = (pack_set, revision.identity_paths.as_ref()) {
        let mut identities = segment
            .stream_authenticated_tree_v2_with_io(
                &revision.identities,
                work.tree_limits(limits)?,
                Some(tree_io.clone()),
            )
            .map_err(invalid)?;
        while let Some(row) =
            next_tree_row(&mut identities, work.shared_work(), deadline, cancel).map_err(invalid)?
        {
            active(deadline, cancel)?;
            let id = std::str::from_utf8(&row.key).map_err(invalid)?;
            if id.is_empty() || id.as_bytes().contains(&0) {
                return Err(invalid("V2 identity key encoding differs"));
            }
            let path = std::str::from_utf8(&row.value).map_err(invalid)?;
            tos_foundation::RelativePath::parse(path).map_err(invalid)?;
            spill.observe_identity_binding(revision_id, &row.key, &row.value)?;
        }
        let coverage = identities
            .coverage()
            .ok_or_else(|| invalid("V2 image identities EOF absent"))?;
        work.record_tree(coverage.work, coverage.entries, limits)?;
        drop(identities);

        let mut inverse = segment
            .stream_authenticated_tree_v2_with_io(
                identity_paths,
                work.tree_limits(limits)?,
                Some(tree_io.clone()),
            )
            .map_err(invalid)?;
        while let Some(row) =
            next_tree_row(&mut inverse, work.shared_work(), deadline, cancel).map_err(invalid)?
        {
            active(deadline, cancel)?;
            if !row.value.is_empty() {
                return Err(invalid("V2 identity inverse value is not empty"));
            }
            let separator = row
                .key
                .iter()
                .position(|byte| *byte == 0)
                .ok_or_else(|| invalid("V2 identity inverse delimiter absent"))?;
            let path = std::str::from_utf8(&row.key[..separator]).map_err(invalid)?;
            tos_foundation::RelativePath::parse(path).map_err(invalid)?;
            let id = std::str::from_utf8(&row.key[separator + 1..]).map_err(invalid)?;
            if id.is_empty() || id.as_bytes().contains(&0) {
                return Err(invalid("V2 identity inverse identifier differs"));
            }
            spill.observe_identity_path_key(revision_id, &row.key)?;
        }
        let coverage = inverse
            .coverage()
            .ok_or_else(|| invalid("V2 image identity inverse EOF absent"))?;
        work.record_tree(coverage.work, coverage.entries, limits)?;
        spill.finish_identity_paths_revision(revision_id, revision.identity_count)?;
    }
    let mut members = segment
        .stream_authenticated_tree_v2_with_io(
            &revision.members,
            work.tree_limits(limits)?,
            Some(tree_io.clone()),
        )
        .map_err(invalid)?;
    let mut source_bytes = 0u64;
    while let Some(row) =
        next_tree_row(&mut members, work.shared_work(), deadline, cancel).map_err(invalid)?
    {
        active(deadline, cancel)?;
        let path = std::str::from_utf8(&row.key).map_err(invalid)?;
        tos_foundation::RelativePath::parse(path).map_err(invalid)?;
        if row.value.len() != 44 {
            return Err(invalid("V2 image member tuple length differs"));
        }
        let digest = Digest256::from_bytes(row.value[..32].try_into().map_err(invalid)?);
        let size = u64::from_be_bytes(row.value[32..40].try_into().map_err(invalid)?);
        let mode = u32::from_le_bytes(row.value[40..44].try_into().map_err(invalid)?);
        if mode & !0o777 != 0 {
            return Err(invalid("V2 image member mode differs"));
        }
        source_bytes = source_bytes
            .checked_add(size)
            .ok_or_else(|| invalid("V2 image source byte overflow"))?;
        if let Some(spill) = pack_set.filter(|_| packed.is_some()) {
            spill.expect_packed_object(revision_id, digest, Some(size))?;
        } else {
            verify_object(objects, digest, size, io, deadline, cancel)?;
        }
    }
    if source_bytes != revision.source_bytes {
        return Err(invalid("V2 image source byte count differs"));
    }
    let coverage = members
        .coverage()
        .ok_or_else(|| invalid("V2 image members EOF absent"))?;
    work.record_tree(coverage.work, coverage.entries, limits)?;
    drop(members);
    let mut retirements = segment
        .stream_authenticated_tree_v2_with_io(
            &revision.retirements,
            work.tree_limits(limits)?,
            Some(tree_io.clone()),
        )
        .map_err(invalid)?;
    let mut ordinal = 0u64;
    while let Some(row) =
        next_tree_row(&mut retirements, work.shared_work(), deadline, cancel).map_err(invalid)?
    {
        active(deadline, cancel)?;
        if row.key.as_slice() != ordinal.to_be_bytes() {
            return Err(invalid("V2 image retirement ordinal differs"));
        }
        ordinal = ordinal
            .checked_add(1)
            .ok_or_else(|| invalid("V2 image retirement ordinal overflow"))?;
        let mut raw = row.value.as_slice();
        tuple_path(&mut raw)?;
        let retired_digest = tuple_digest(&mut raw)?;
        tuple_path(&mut raw)?;
        let event_digest = tuple_digest(&mut raw)?;
        let event_size = u64::from_be_bytes(take_tuple(&mut raw, 8)?.try_into().map_err(invalid)?);
        if !raw.is_empty() {
            return Err(invalid("V2 image retirement tuple trailing bytes"));
        }
        if let Some(spill) = pack_set.filter(|_| packed.is_some()) {
            spill.expect_packed_object(revision_id, retired_digest, None)?;
            spill.expect_packed_object(revision_id, event_digest, Some(event_size))?;
        } else {
            let retired_name = retired_digest.to_hex();
            debit_name_resolution(io, &retired_name)?;
            let retired =
                tos_fd_open::open_regular_at(objects, Path::new(&retired_name)).map_err(invalid)?;
            verify_object(
                objects,
                retired_digest,
                retired.metadata()?.len(),
                io,
                deadline,
                cancel,
            )?;
            verify_object(objects, event_digest, event_size, io, deadline, cancel)?;
        }
    }
    let coverage = retirements
        .coverage()
        .ok_or_else(|| invalid("V2 image retirement EOF absent"))?;
    work.record_tree(coverage.work, coverage.entries, limits)?;
    if let Some(objects_root) = packed {
        let spill = pack_set.ok_or_else(|| invalid("V2 packed closure spill is absent"))?;
        let mut extents = segment
            .stream_authenticated_tree_v2_with_io(
                objects_root,
                work.tree_limits(limits)?,
                Some(tree_io.clone()),
            )
            .map_err(invalid)?;
        let mut previous = None;
        while let Some(row) =
            next_tree_row(&mut extents, work.shared_work(), deadline, cancel).map_err(invalid)?
        {
            active(deadline, cancel)?;
            if row.key.len() != 32 {
                return Err(invalid("V2 packed extent digest width differs"));
            }
            let digest = Digest256::from_bytes(row.key.as_slice().try_into().map_err(invalid)?);
            if previous.is_some_and(|last| digest <= last) {
                return Err(invalid("V2 packed extent digest cursor did not advance"));
            }
            previous = Some(digest);
            let location = PackedObjectLocationV2::decode(&row.value)?;
            if location.size > limits.segment.max_frame_bytes
                || location.segment_size > limits.segment.max_segment_bytes
                || location.frame_count > MAX_PACKED_OBJECT_FRAMES_V2.min(limits.segment.max_frames)
            {
                return Err(invalid("V2 packed extent exceeds selected segment limits"));
            }
            spill.observe_packed_extent(revision_id, digest, location)?;
        }
        let coverage = extents
            .coverage()
            .ok_or_else(|| invalid("V2 packed extent EOF absent"))?;
        work.record_tree(coverage.work, coverage.entries, limits)?;
        spill.finish_packed_revision(revision_id)?;
    }
    Ok(())
}

fn verify_revision_artifact(
    directory: &File,
    revision: &SourceRevisionRootsV2,
    limits: V2ImageLimits,
    io: &PinnedSqliteIoBudget,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<()> {
    let artifact = &revision.source_artifact;
    let name = artifact.filename();
    debit_name_resolution(io, name)?;
    let file = tos_fd_open::open_regular_at(directory, Path::new(name)).map_err(invalid)?;
    let size = file.metadata()?.len();
    if size > limits.max_compatibility_manifest_bytes
        || artifact.bytes().is_some_and(|expected| expected != size)
        || matches!(
            artifact,
            SourceRevisionArtifactV2::CompactCommitV2 { .. }
                | SourceRevisionArtifactV2::CompactPackedV2 { .. }
        ) && size > MAX_COMPACT_COMMIT_V2_BYTES as u64
    {
        return Err(invalid("V2 image revision record size differs"));
    }
    drop(file);

    match artifact {
        SourceRevisionArtifactV2::CompactCommitV2 { .. }
        | SourceRevisionArtifactV2::CompactPackedV2 { .. } => {
            let raw = read_and_verify_compact_record(
                directory,
                name,
                artifact.sha256(),
                size,
                io,
                deadline,
                cancel,
            )?;
            let workspace = decode_workspace_upper_bound(raw.len())?;
            let record = CompactCommitV2::decode_with_workspace(&raw, workspace)?;
            if !record.matches_roots(revision) {
                return Err(invalid(
                    "V2 compact commit differs from authenticated history",
                ));
            }
        }
        SourceRevisionArtifactV2::LegacyManifestV1 { .. }
        | SourceRevisionArtifactV2::SnapshotV1 { .. } => {
            verify_file(
                directory,
                name,
                artifact.sha256(),
                size,
                io,
                deadline,
                cancel,
            )?;
        }
    }
    Ok(())
}

fn read_and_verify_compact_record(
    directory: &File,
    name: &str,
    expected_digest: Digest256,
    expected_size: u64,
    io: &PinnedSqliteIoBudget,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<Vec<u8>> {
    let capacity = usize::try_from(expected_size).map_err(invalid)?;
    if capacity == 0 || capacity > MAX_COMPACT_COMMIT_V2_BYTES {
        return Err(invalid("V2 compact commit exceeds its byte profile"));
    }
    debit_name_resolution(io, name)?;
    let mut input = tos_fd_open::open_regular_at(directory, Path::new(name)).map_err(invalid)?;
    let before = input.metadata()?;
    if before.len() != expected_size
        || before.uid() != rustix::process::geteuid().as_raw()
        || before.mode() & 0o222 != 0
    {
        return Err(invalid("V2 compact commit custody differs"));
    }
    let mut raw = Vec::new();
    raw.try_reserve_exact(capacity).map_err(invalid)?;
    let mut block = [0u8; BLOCK_BYTES];
    let mut hash = Digest256Hasher::new();
    let mut remaining = expected_size;
    while remaining > 0 {
        active(deadline, cancel)?;
        let wanted = usize::try_from(remaining.min(BLOCK_BYTES as u64)).map_err(invalid)?;
        io.charge_read(wanted as u64).map_err(invalid)?;
        let read = match input.read(&mut block[..wanted]) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        io.record_read_returned(read as u64).map_err(invalid)?;
        if read == 0 {
            return Err(invalid("V2 compact commit early EOF"));
        }
        hash.update(&block[..read]);
        raw.extend_from_slice(&block[..read]);
        remaining -= read as u64;
    }
    io.charge_read(1).map_err(invalid)?;
    let tail = input.read(&mut block[..1])?;
    io.record_read_returned(tail as u64).map_err(invalid)?;
    debit_name_resolution(io, name)?;
    let named = tos_fd_open::open_regular_at(directory, Path::new(name)).map_err(invalid)?;
    if tail != 0
        || raw.len() != capacity
        || hash.finalize() != expected_digest
        || stamp(&input.metadata()?) != stamp(&before)
        || stamp(&named.metadata()?) != stamp(&before)
    {
        return Err(invalid("V2 compact commit digest or custody changed"));
    }
    Ok(raw)
}

fn take_tuple<'a>(raw: &mut &'a [u8], count: usize) -> io::Result<&'a [u8]> {
    if count > raw.len() {
        return Err(invalid("V2 image row tuple ended early"));
    }
    let (value, rest) = raw.split_at(count);
    *raw = rest;
    Ok(value)
}

fn tuple_path(raw: &mut &[u8]) -> io::Result<()> {
    let count = usize::try_from(u32::from_be_bytes(
        take_tuple(raw, 4)?.try_into().map_err(invalid)?,
    ))
    .map_err(invalid)?;
    let path = std::str::from_utf8(take_tuple(raw, count)?).map_err(invalid)?;
    tos_foundation::RelativePath::parse(path).map_err(invalid)?;
    Ok(())
}

fn tuple_digest(raw: &mut &[u8]) -> io::Result<Digest256> {
    Ok(Digest256::from_bytes(
        take_tuple(raw, 32)?.try_into().map_err(invalid)?,
    ))
}

fn verify_object(
    objects: &File,
    digest: Digest256,
    size: u64,
    io: &PinnedSqliteIoBudget,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<()> {
    let name = digest.to_hex();
    verify_file(objects, &name, digest, size, io, deadline, cancel)
}

fn verify_file(
    directory: &File,
    name: &str,
    digest: Digest256,
    size: u64,
    io: &PinnedSqliteIoBudget,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<()> {
    debit_name_resolution(io, name)?;
    let mut input = tos_fd_open::open_regular_at(directory, Path::new(name)).map_err(invalid)?;
    let before = input.metadata()?;
    if before.len() != size
        || before.uid() != rustix::process::geteuid().as_raw()
        || before.mode() & 0o222 != 0
    {
        return Err(invalid("V2 image referenced object custody differs"));
    }
    let mut block = [0u8; BLOCK_BYTES];
    let mut hash = Digest256Hasher::new();
    let mut remaining = size;
    while remaining > 0 {
        active(deadline, cancel)?;
        let wanted = usize::try_from(remaining.min(BLOCK_BYTES as u64)).map_err(invalid)?;
        io.charge_read(wanted as u64).map_err(invalid)?;
        let read = match input.read(&mut block[..wanted]) {
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            value => value?,
        };
        io.record_read_returned(read as u64).map_err(invalid)?;
        if read == 0 {
            return Err(invalid("V2 image referenced object early EOF"));
        }
        hash.update(&block[..read]);
        remaining -= read as u64;
    }
    io.charge_read(1).map_err(invalid)?;
    let tail = input.read(&mut block[..1])?;
    io.record_read_returned(tail as u64).map_err(invalid)?;
    debit_name_resolution(io, name)?;
    let named = tos_fd_open::open_regular_at(directory, Path::new(name)).map_err(invalid)?;
    if tail != 0
        || hash.finalize() != digest
        || stamp(&input.metadata()?) != stamp(&before)
        || stamp(&named.metadata()?) != stamp(&before)
    {
        return Err(invalid(
            "V2 image referenced object digest or custody changed",
        ));
    }
    Ok(())
}

fn account_allocation(
    file: &File,
    work: &mut Work,
    reservation: &PinnedSqliteSpaceReservation,
) -> io::Result<()> {
    work.allocated = work
        .allocated
        .checked_add(
            file.metadata()?
                .blocks()
                .checked_mul(512)
                .ok_or_else(|| invalid("V2 image allocation overflow"))?,
        )
        .ok_or_else(|| invalid("V2 image allocation overflow"))?;
    reservation
        .update_actual_allocated(work.allocated)
        .map_err(invalid)
}

fn account_growth(
    file: &File,
    previous: &mut u64,
    work: &mut Work,
    reservation: &PinnedSqliteSpaceReservation,
) -> io::Result<()> {
    let current = file
        .metadata()?
        .blocks()
        .checked_mul(512)
        .ok_or_else(|| invalid("V2 image allocation overflow"))?;
    let growth = current
        .checked_sub(*previous)
        .ok_or_else(|| invalid("V2 image allocation unexpectedly shrank"))?;
    work.allocated = work
        .allocated
        .checked_add(growth)
        .ok_or_else(|| invalid("V2 image allocation overflow"))?;
    *previous = current;
    reservation
        .update_actual_allocated(work.allocated)
        .map_err(invalid)
}

fn copy_file(
    source: &File,
    target: &File,
    name: &str,
    target_name: &str,
    io: &PinnedSqliteIoBudget,
    reservation: &PinnedSqliteSpaceReservation,
    work: &mut Work,
    limits: V2ImageLimits,
    block: &mut [u8; BLOCK_BYTES],
    deadline: Instant,
    cancel: &AtomicBool,
    object: bool,
) -> io::Result<()> {
    active(deadline, cancel)?;
    work.files = work
        .files
        .checked_add(1)
        .filter(|n| *n <= limits.max_files)
        .ok_or_else(|| invalid("V2 image file bound exceeded"))?;
    debit_name_resolution(io, name)?;
    let mut input = tos_fd_open::open_regular_at(source, Path::new(name)).map_err(invalid)?;
    let before = input.metadata()?;
    if before.uid() != rustix::process::geteuid().as_raw() || before.mode() & 0o022 != 0 {
        return Err(invalid("V2 image leaf custody differs"));
    }
    work.reserve_allocation_upper(before.len(), limits)?;
    debit_name_resolution(io, target_name)?;
    let mut output = File::from(rustix::fs::openat(
        target,
        target_name,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o600),
    )?);
    let mut allocated = 0;
    let mut hash = Digest256Hasher::new();
    let mut remaining = before.len();
    while remaining > 0 {
        active(deadline, cancel)?;
        let wanted = usize::try_from(remaining.min(BLOCK_BYTES as u64)).map_err(invalid)?;
        io.charge_read(wanted as u64).map_err(invalid)?;
        let read = match input.read(&mut block[..wanted]) {
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            value => value?,
        };
        io.record_read_returned(read as u64).map_err(invalid)?;
        if read == 0 {
            return Err(invalid("V2 image source leaf early EOF"));
        }
        hash.update(&block[..read]);
        let mut offset = 0;
        while offset < read {
            active(deadline, cancel)?;
            io.charge_write((read - offset) as u64).map_err(invalid)?;
            let wrote = match output.write(&block[offset..read]) {
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                value => value?,
            };
            io.record_write_returned(wrote as u64).map_err(invalid)?;
            if wrote == 0 {
                return Err(invalid("V2 image target leaf write stopped"));
            }
            offset += wrote;
            account_growth(&output, &mut allocated, work, reservation)?;
        }
        remaining -= read as u64;
    }
    io.charge_read(1).map_err(invalid)?;
    let tail = input.read(&mut block[..1])?;
    io.record_read_returned(tail as u64).map_err(invalid)?;
    debit_name_resolution(io, name)?;
    let named = tos_fd_open::open_regular_at(source, Path::new(name)).map_err(invalid)?;
    if tail != 0
        || stamp(&input.metadata()?) != stamp(&before)
        || stamp(&named.metadata()?) != stamp(&before)
    {
        return Err(invalid("V2 image source leaf changed"));
    }
    if object && Digest256::from_hex(name).map_err(invalid)? != hash.finalize() {
        return Err(invalid("V2 image source object digest differs"));
    }
    output.set_permissions(Permissions::from_mode(before.mode() & 0o777))?;
    output.sync_all()?;
    account_growth(&output, &mut allocated, work, reservation)
}

fn copy_directory(
    source: &File,
    target: &File,
    depth: usize,
    objects: bool,
    io: &PinnedSqliteIoBudget,
    reservation: &PinnedSqliteSpaceReservation,
    work: &mut Work,
    limits: V2ImageLimits,
    block: &mut [u8; BLOCK_BYTES],
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<()> {
    active(deadline, cancel)?;
    if depth > limits.max_depth {
        return Err(invalid("V2 image directory depth exceeded"));
    }
    work.directories = work
        .directories
        .checked_add(1)
        .filter(|n| *n <= limits.max_directories)
        .ok_or_else(|| invalid("V2 image directory bound exceeded"))?;
    let before = source.metadata()?;
    let mut allocated = target
        .metadata()?
        .blocks()
        .checked_mul(512)
        .ok_or_else(|| invalid("V2 image directory allocation overflow"))?;
    let mut directory_buffer = [MaybeUninit::uninit(); 8192];
    let mut entries = RawDir::new(source, &mut directory_buffer);
    loop {
        active(deadline, cancel)?;
        if entries.is_buffer_empty() {
            debit_directory_read(io)?;
        }
        let entry = match entries.next() {
            None => break,
            Some(entry) => entry?,
        };
        let name = entry
            .file_name()
            .to_str()
            .ok()
            .filter(|name| !name.is_empty() && name.len() <= 255)
            .ok_or_else(|| invalid("V2 image child name differs"))?
            .to_owned();
        if name == "." || name == ".." {
            continue;
        }
        if depth == 0 && (name == "current.json" || name == ".admission.lock") {
            continue;
        }
        debit_name_resolution(io, &name)?;
        let stat = rustix::fs::statat(source, name.as_str(), AtFlags::SYMLINK_NOFOLLOW)?;
        let entry_type = FileType::from_raw_mode(stat.st_mode);
        if entry_type.is_dir() {
            debit_name_resolution(io, &name)?;
            let input =
                tos_fd_open::open_directory_at(source, Path::new(&name)).map_err(invalid)?;
            work.reserve_allocation_upper(limits.allocation_unit_bytes, limits)?;
            debit_name_resolution(io, &name)?;
            rustix::fs::mkdirat(target, name.as_str(), Mode::from_raw_mode(0o700))?;
            account_growth(target, &mut allocated, work, reservation)?;
            debit_name_resolution(io, &name)?;
            let output =
                tos_fd_open::open_directory_at(target, Path::new(&name)).map_err(invalid)?;
            account_allocation(&output, work, reservation)?;
            copy_directory(
                &input,
                &output,
                depth + 1,
                depth == 0 && name == "objects",
                io,
                reservation,
                work,
                limits,
                block,
                deadline,
                cancel,
            )?;
            debit_name_resolution(io, &name)?;
            let named =
                tos_fd_open::open_directory_at(source, Path::new(&name)).map_err(invalid)?;
            if (named.metadata()?.dev(), named.metadata()?.ino())
                != (input.metadata()?.dev(), input.metadata()?.ino())
            {
                return Err(invalid("V2 image source directory replaced"));
            }
        } else if entry_type.is_file() {
            let result = copy_file(
                source,
                target,
                &name,
                &name,
                io,
                reservation,
                work,
                limits,
                block,
                deadline,
                cancel,
                objects,
            );
            account_growth(target, &mut allocated, work, reservation)?;
            result?;
        } else {
            return Err(invalid("V2 image non-regular member refused"));
        }
    }
    if stamp(&source.metadata()?) != stamp(&before) {
        return Err(invalid("V2 image source directory changed"));
    }
    target.sync_all()?;
    account_growth(target, &mut allocated, work, reservation)
}

pub struct V2ImageReceipt {
    pub selection: CorpusCurrentSelection,
    pub copied_files: u64,
    pub copied_directories: u64,
    pub allocated_bytes: u64,
    pub tree_read_nodes: u64,
}

/// Retain this outcome even on failure: created named bytes survive and their
/// reservation must transfer to the terminal cleanup/baseline owner.
pub struct V2ImageOutcome {
    pub result: io::Result<V2ImageReceipt>,
    pub custody: Arc<PinnedSqliteSpaceReservation>,
    pub(crate) held_target_root: Option<File>,
}

struct V2ImageColdSpillPlan {
    source: Option<V2SeenPackSpillRequest>,
    target: Option<V2SeenPackSpillRequest>,
    source_limits: V2SeenPackSpillLimits,
    target_limits: V2SeenPackSpillLimits,
}

impl V2ImageColdSpillPlan {
    fn take_source(&mut self) -> Option<(V2SeenPackSpillRequest, V2SeenPackSpillLimits)> {
        Some((self.source.take()?, self.source_limits))
    }

    fn take_target(&mut self) -> Option<(V2SeenPackSpillRequest, V2SeenPackSpillLimits)> {
        Some((self.target.take()?, self.target_limits))
    }
}

pub fn transfer_image(
    source_path: &Path,
    fresh_target: &Path,
    limits: V2ImageLimits,
    io: &PinnedSqliteIoBudget,
    space: &PinnedSqliteSpaceBudget,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<V2ImageOutcome> {
    transfer_image_inner(
        source_path,
        fresh_target,
        limits,
        io,
        space,
        deadline,
        cancel,
        None,
        None,
        None,
    )
}

/// Cold closure route with separate caller-held auxiliary scratch and
/// independent source/target SQLite requests. Each request is consumed only
/// by its corresponding physical-store verification.
pub fn transfer_image_with_cold_spill(
    source_path: &Path,
    fresh_target: &Path,
    limits: V2ImageLimits,
    io: &PinnedSqliteIoBudget,
    image_space: &PinnedSqliteSpaceBudget,
    auxiliary_space: &PinnedSqliteSpaceBudget,
    requests: V2SeenPackSpillRequests,
    deadline: Instant,
    cancel: &Arc<AtomicBool>,
) -> io::Result<V2ImageOutcome> {
    if auxiliary_space.shares_with(image_space) {
        return Err(invalid(
            "V2 cold spill auxiliary space must be separately held",
        ));
    }
    requests.validate_for_operation(io, auxiliary_space, deadline, cancel)?;
    let (limits, source_limits, target_limits) = limits.validate_cold_spill(&requests)?;
    let plan = V2ImageColdSpillPlan {
        source: Some(requests.source),
        target: Some(requests.target),
        source_limits,
        target_limits,
    };
    transfer_image_inner(
        source_path,
        fresh_target,
        limits,
        io,
        image_space,
        deadline,
        cancel,
        Some(plan),
        None,
        None,
    )
}

pub(crate) fn transfer_image_with_cold_spill_at(
    source_path: &Path,
    source_held: &File,
    source_identity: (u64, u64),
    target: V2HeldTargetRoot<'_>,
    expected_target_identity: (u64, u64),
    limits: V2ImageLimits,
    io: &PinnedSqliteIoBudget,
    image_space: &PinnedSqliteSpaceBudget,
    auxiliary_space: &PinnedSqliteSpaceBudget,
    requests: V2SeenPackSpillRequests,
    shared_work: AdmissionWorkBudget,
    deadline: Instant,
    cancel: &Arc<AtomicBool>,
) -> io::Result<V2ImageOutcome> {
    if auxiliary_space.shares_with(image_space) {
        return Err(invalid(
            "V2 cold spill auxiliary space must be separately held",
        ));
    }
    requests.validate_for_operation(io, auxiliary_space, deadline, cancel)?;
    let (limits, source_limits, target_limits) = limits.validate_cold_spill(&requests)?;
    let target_root =
        open_selected_target(target, Some(expected_target_identity), io, deadline, cancel)?;
    let target_store_identity = identity(&target_root)?;
    drop(target_root);
    let plan = V2ImageColdSpillPlan {
        source: Some(requests.source),
        target: Some(requests.target),
        source_limits,
        target_limits,
    };
    transfer_image_inner(
        source_path,
        target.store_path,
        limits,
        io,
        image_space,
        deadline,
        cancel,
        Some(plan),
        Some(V2HeldImageRoots {
            source: V2HeldSourceRoot {
                path: source_path,
                held: source_held,
                identity: source_identity,
            },
            target,
            target_store_identity,
        }),
        Some(shared_work),
    )
}

fn transfer_image_inner(
    source_path: &Path,
    fresh_target: &Path,
    limits: V2ImageLimits,
    io: &PinnedSqliteIoBudget,
    space: &PinnedSqliteSpaceBudget,
    deadline: Instant,
    cancel: &AtomicBool,
    mut cold: Option<V2ImageColdSpillPlan>,
    held_roots: Option<V2HeldImageRoots<'_>>,
    shared_work: Option<AdmissionWorkBudget>,
) -> io::Result<V2ImageOutcome> {
    let limits = if cold.is_some() {
        limits.validate_layout()?.0
    } else {
        limits.validate()?
    };
    active(deadline, cancel)?;
    let source = if let Some(roots) = held_roots {
        debit_root_guard(io)?;
        active(deadline, cancel)?;
        verify_named_root_no_charge(roots.source.path, roots.source.held, roots.source.identity)?;
        AdmissionStore::open_existing_at_named_with_io(
            roots.source.path,
            roots.source.held,
            io.clone(),
            deadline,
            cancel,
        )?
    } else {
        AdmissionStore::open_existing(source_path, deadline, cancel)?
    };
    debit_name_resolution(io, ".admission.lock")?;
    let _lock = source.lock_for_backup(deadline, cancel)?;
    let selection = source
        .current_selection(limits.reader, deadline, cancel, Some(io))?
        .ok_or_else(|| invalid("V2 image source selection absent"))?;
    let roots = selected_roots(&source, &selection, limits, io, deadline, cancel)?;
    let target = if let Some(roots) = held_roots {
        if roots.target.store_path != fresh_target {
            return Err(invalid("V2 selected target path differs from held binding"));
        }
        open_selected_target(
            roots.target,
            Some(roots.target_store_identity),
            io,
            deadline,
            cancel,
        )?
    } else {
        tos_fd_open::open_absolute_directory(fresh_target).map_err(invalid)?
    };
    let metadata = target.metadata()?;
    active(deadline, cancel)?;
    let empty = directory_is_empty(&target, io, deadline, cancel)?;
    if metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.mode() & 0o077 != 0
        || metadata.blksize() > limits.allocation_unit_bytes
        || !empty
    {
        return Err(invalid("V2 image target is not fresh private directory"));
    }
    let custody = Arc::new(
        space
            .reserve(limits.max_new_allocated_bytes)
            .map_err(invalid)?,
    );
    let result = (|| {
        let mut work = Work {
            shared_work,
            ..Work::default()
        };
        let closure_binding = selection
            .rootset_sha256
            .ok_or_else(|| invalid("V2 image closure selector digest absent"))?;
        verify_closure(
            &source,
            &roots,
            limits,
            io,
            &mut work,
            closure_binding,
            cold.as_mut().and_then(V2ImageColdSpillPlan::take_source),
            deadline,
            cancel,
        )?;
        let (root, _, _) = source.backup_namespaces()?;
        let mut block = [0; BLOCK_BYTES];
        copy_directory(
            root, &target, 0, false, io, &custody, &mut work, limits, &mut block, deadline, cancel,
        )?;
        if let Some(roots) = held_roots {
            let _current_target = open_selected_target(
                roots.target,
                Some(roots.target_store_identity),
                io,
                deadline,
                cancel,
            )?;
            active(deadline, cancel)?;
        }
        let restored = if held_roots.is_some() {
            AdmissionStore::open_existing_at_named_with_io(
                fresh_target,
                &target,
                io.clone(),
                deadline,
                cancel,
            )?
        } else {
            AdmissionStore::open_existing(fresh_target, deadline, cancel)?
        };
        let (restored_root, _, _) = restored.backup_namespaces()?;
        if (
            restored_root.metadata()?.dev(),
            restored_root.metadata()?.ino(),
        ) != (metadata.dev(), metadata.ino())
        {
            return Err(invalid("V2 image target root replaced"));
        }
        let restored_roots = selected_roots(&restored, &selection, limits, io, deadline, cancel)?;
        if restored_roots != roots {
            return Err(invalid("V2 image restored roots differ"));
        }
        verify_closure(
            &restored,
            &restored_roots,
            limits,
            io,
            &mut work,
            closure_binding,
            cold.as_mut().and_then(V2ImageColdSpillPlan::take_target),
            deadline,
            cancel,
        )?;
        if source.current_selection(limits.reader, deadline, cancel, Some(io))?
            != Some(selection.clone())
        {
            return Err(invalid("V2 image source selection changed"));
        }
        let mut root_allocated = target
            .metadata()?
            .blocks()
            .checked_mul(512)
            .ok_or_else(|| invalid("V2 image directory allocation overflow"))?;
        work.reserve_allocation_upper(0, limits)?;
        debit_name_resolution(io, ".admission.lock")?;
        let lock_file = File::from(rustix::fs::openat(
            &target,
            ".admission.lock",
            OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )?);
        lock_file.sync_all()?;
        account_allocation(&lock_file, &mut work, &custody)?;
        account_growth(&target, &mut root_allocated, &mut work, &custody)?;
        let copy_result = copy_file(
            root,
            &target,
            "current.json",
            ".restore-current.tmp",
            io,
            &custody,
            &mut work,
            limits,
            &mut block,
            deadline,
            cancel,
            false,
        );
        account_growth(&target, &mut root_allocated, &mut work, &custody)?;
        copy_result?;
        debit_name_resolution(io, ".restore-current.tmp")?;
        debit_name_resolution(io, "current.json")?;
        rustix::fs::renameat_with(
            &target,
            ".restore-current.tmp",
            &target,
            "current.json",
            RenameFlags::NOREPLACE,
        )?;
        target.sync_all()?;
        account_growth(&target, &mut root_allocated, &mut work, &custody)?;
        if restored.current_selection(limits.reader, deadline, cancel, Some(io))?
            != Some(selection.clone())
        {
            return Err(invalid("V2 image final selection differs"));
        }
        source.verify_layout()?;
        restored.verify_layout()?;
        active(deadline, cancel)?;
        Ok(V2ImageReceipt {
            selection,
            copied_files: work.files,
            copied_directories: work.directories,
            allocated_bytes: work.allocated,
            tree_read_nodes: work.tree_nodes,
        })
    })();
    let result = if let Some(roots) = held_roots {
        let source_fence = verify_named_root(
            roots.source.path,
            roots.source.held,
            roots.source.identity,
            io,
            deadline,
            cancel,
        );
        let target_fence = open_selected_target(
            roots.target,
            Some(roots.target_store_identity),
            io,
            deadline,
            cancel,
        )
        .map(|_| ());
        match result {
            Ok(receipt) => source_fence.and(target_fence).map(|_| receipt),
            Err(error) => Err(error),
        }
    } else {
        result
    };
    Ok(V2ImageOutcome {
        result,
        custody,
        held_target_root: held_roots.map(|_| target),
    })
}
