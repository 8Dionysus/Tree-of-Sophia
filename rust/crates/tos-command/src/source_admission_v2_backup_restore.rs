//! Finite cold backup/fresh-restore of the private native V2 store.
//! A copied selector is installed last, after the independent copy passes
//! authenticated current/history closure checks. Bytes confer no admission.
use super::source_admission::{active, invalid};
use super::source_admission_segment_v2::{SourceRevisionRootsV2, SourceRootSetV2};
use super::source_admission_store::AdmissionStore;
use super::source_admission_v2_seen_pack::{
    V2SeenPackSpill, V2SeenPackSpillLimits, V2SeenPackSpillRequest, V2SeenPackSpillRequests,
};
use rustix::fs::{Mode, OFlags, RenameFlags};
use std::{
    fs::{File, Metadata, Permissions},
    io::{self, Read, Write},
    mem::size_of,
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, PermissionsExt},
    },
    path::Path,
    sync::{Arc, atomic::AtomicBool},
    time::Instant,
};
use tos_foundation::{Digest256, Digest256Hasher};
use tos_segment_store::{
    AuthenticatedTreeCoverageV1, AuthenticatedTreeIoLedgerV1, AuthenticatedTreeLimitsV1,
    AuthenticatedTreePackSetV2, AuthenticatedTreeWorkV1, SegmentLimits, SegmentStore,
};
use tos_source_store::{
    CorpusCurrentSelection, CorpusPointerFormat, PinnedSqliteIoBudget, PinnedSqliteSpaceBudget,
    PinnedSqliteSpaceReservation, ReadLimits,
};

const DOMAIN: &[u8] = b"tos-native-admission-source-v2";
const ROOT_BYTES: usize = 65_536;
const BLOCK_BYTES: usize = 65_536;

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
        let (limits, nodes, base_state) = self.validate_layout()?;
        let retained = base_state
            .checked_add(size_of::<V2ImageColdSpillPlan>())
            .ok_or_else(|| invalid("V2 image cold-spill state overflow"))?;
        let max_tree_nodes = u64::try_from(nodes).map_err(invalid)?;
        let profile = |request: &V2SeenPackSpillRequest| V2SeenPackSpillLimits {
            max_tree_nodes,
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
        let retained = path_nodes
            .checked_mul(self.tree.max_node_bytes)
            .and_then(|n| n.checked_mul(64))
            .and_then(|n| {
                n.checked_add(
                    self.max_history_roots
                        .checked_mul(ROOT_BYTES.checked_mul(4)?)?,
                )
            })
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
    files: u64,
    directories: u64,
    allocated: u64,
    allocation_upper: u64,
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
        Ok(tree)
    }
    fn record_tree(
        &mut self,
        work: AuthenticatedTreeWorkV1,
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
        Ok(())
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
    let roots = SourceRootSetV2::decode(&raw)?;
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
    deadline: Instant,
    cancel: &AtomicBool,
) -> tos_segment_store::Result<AuthenticatedTreeCoverageV1> {
    if let Some(pack_set) = pack_set {
        let pack_set: Arc<dyn AuthenticatedTreePackSetV2> = pack_set.clone();
        segment.verify_authenticated_tree_v2_with_pack_set(
            descriptor,
            limits,
            Some(tree_io),
            pack_set,
            closure_binding,
            deadline,
            cancel,
        )
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
    if roots.history.entries > limits.max_history_roots as u64 {
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
        deadline,
        cancel,
    )
    .map_err(invalid)?;
    work.record_tree(coverage.work, limits)?;
    let mut stream = segment
        .stream_authenticated_tree_v2_with_io(
            &roots.history,
            work.tree_limits(limits)?,
            Some(tree_io.clone()),
        )
        .map_err(invalid)?;
    let mut history = Vec::new();
    let mut current_seen = false;
    while let Some(row) = stream.next_row(deadline, cancel).map_err(invalid)? {
        active(deadline, cancel)?;
        if history.len() >= limits.max_history_roots {
            return Err(invalid("V2 image retained history bound exceeded"));
        }
        let revision = SourceRevisionRootsV2::decode(&row.value)?;
        if row.key.as_slice() != revision.revision.0.as_bytes() {
            return Err(invalid("V2 image history key differs"));
        }
        revision.validate_store_binding(segment.store_id(), segment.domain_digest())?;
        if revision.revision == roots.current.revision {
            roots.verify_current_history_row(&row.key, &row.value)?;
            if current_seen {
                return Err(invalid("V2 image repeated current history key"));
            }
            current_seen = true;
        }
        history.try_reserve(1).map_err(invalid)?;
        history.push(revision);
    }
    let coverage = stream
        .coverage()
        .ok_or_else(|| invalid("V2 image history EOF absent"))?;
    work.record_tree(coverage.work, limits)?;
    drop(stream);
    if !current_seen {
        return Err(invalid(
            "V2 image current absent from authenticated history",
        ));
    }
    // Preserve every retained predecessor, with no missing base or cycle. The
    // authenticated history stream is ordered by the raw revision digest.
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
        let (_, _, revisions) = store.backup_namespaces()?;
        let directory =
            tos_fd_open::open_directory_at(revisions, Path::new(&revision.revision.0.to_hex()))
                .map_err(invalid)?;
        let snapshot = tos_fd_open::open_regular_at(&directory, Path::new("snapshot.json"))
            .map_err(invalid)?;
        let snapshot_size = snapshot.metadata()?.len();
        if snapshot_size > limits.max_compatibility_manifest_bytes {
            return Err(invalid("V2 image compatibility manifest bound exceeded"));
        }
        verify_file(
            &directory,
            "snapshot.json",
            revision.manifest_sha256,
            snapshot_size,
            io,
            deadline,
            cancel,
        )?;
        for root in [
            &revision.members,
            &revision.identities,
            &revision.dependencies,
            &revision.retirements,
        ] {
            let coverage = verify_tree_v2(
                &segment,
                root,
                work.tree_limits(limits)?,
                tree_io.clone(),
                pack_set.as_ref(),
                closure_binding,
                deadline,
                cancel,
            )
            .map_err(invalid)?;
            work.record_tree(coverage.work, limits)?;
        }
        let (_, objects, _) = store.backup_namespaces()?;
        let mut members = segment
            .stream_authenticated_tree_v2_with_io(
                &revision.members,
                work.tree_limits(limits)?,
                Some(tree_io.clone()),
            )
            .map_err(invalid)?;
        let mut source_bytes = 0u64;
        while let Some(row) = members.next_row(deadline, cancel).map_err(invalid)? {
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
            verify_object(objects, digest, size, io, deadline, cancel)?;
        }
        if source_bytes != revision.source_bytes {
            return Err(invalid("V2 image source byte count differs"));
        }
        let coverage = members
            .coverage()
            .ok_or_else(|| invalid("V2 image members EOF absent"))?;
        work.record_tree(coverage.work, limits)?;
        drop(members);
        let mut retirements = segment
            .stream_authenticated_tree_v2_with_io(
                &revision.retirements,
                work.tree_limits(limits)?,
                Some(tree_io.clone()),
            )
            .map_err(invalid)?;
        let mut ordinal = 0u64;
        while let Some(row) = retirements.next_row(deadline, cancel).map_err(invalid)? {
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
            let event_size =
                u64::from_be_bytes(take_tuple(&mut raw, 8)?.try_into().map_err(invalid)?);
            if !raw.is_empty() {
                return Err(invalid("V2 image retirement tuple trailing bytes"));
            }
            let retired =
                tos_fd_open::open_regular_at(objects, Path::new(&retired_digest.to_hex()))
                    .map_err(invalid)?;
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
        let coverage = retirements
            .coverage()
            .ok_or_else(|| invalid("V2 image retirement EOF absent"))?;
        work.record_tree(coverage.work, limits)?;
    }
    store.verify_layout()?;
    active(deadline, cancel)
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
    let mut input = tos_fd_open::open_regular_at(source, Path::new(name)).map_err(invalid)?;
    let before = input.metadata()?;
    if before.uid() != rustix::process::geteuid().as_raw() || before.mode() & 0o022 != 0 {
        return Err(invalid("V2 image leaf custody differs"));
    }
    work.reserve_allocation_upper(before.len(), limits)?;
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
    for entry in std::fs::read_dir(format!("/proc/self/fd/{}", source.as_raw_fd()))? {
        active(deadline, cancel)?;
        let entry = entry?;
        let name = entry.file_name();
        let name = name
            .to_str()
            .filter(|n| !n.is_empty() && n.len() <= 255)
            .ok_or_else(|| invalid("V2 image child name differs"))?;
        if depth == 0 && (name == "current.json" || name == ".admission.lock") {
            continue;
        }
        if entry.file_type()?.is_dir() {
            let input = tos_fd_open::open_directory_at(source, Path::new(name)).map_err(invalid)?;
            work.reserve_allocation_upper(limits.allocation_unit_bytes, limits)?;
            rustix::fs::mkdirat(target, name, Mode::from_raw_mode(0o700))?;
            account_growth(target, &mut allocated, work, reservation)?;
            let output =
                tos_fd_open::open_directory_at(target, Path::new(name)).map_err(invalid)?;
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
            let named = tos_fd_open::open_directory_at(source, Path::new(name)).map_err(invalid)?;
            if (named.metadata()?.dev(), named.metadata()?.ino())
                != (input.metadata()?.dev(), input.metadata()?.ino())
            {
                return Err(invalid("V2 image source directory replaced"));
            }
        } else if entry.file_type()?.is_file() {
            let result = copy_file(
                source,
                target,
                name,
                name,
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
) -> io::Result<V2ImageOutcome> {
    let limits = if cold.is_some() {
        limits.validate_layout()?.0
    } else {
        limits.validate()?
    };
    active(deadline, cancel)?;
    let source = AdmissionStore::open_existing(source_path, deadline, cancel)?;
    let _lock = source.lock_for_backup(deadline, cancel)?;
    let selection = source
        .current_selection(limits.reader, deadline, cancel, Some(io))?
        .ok_or_else(|| invalid("V2 image source selection absent"))?;
    let roots = selected_roots(&source, &selection, limits, io, deadline, cancel)?;
    let target = tos_fd_open::open_absolute_directory(fresh_target).map_err(invalid)?;
    let metadata = target.metadata()?;
    if metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.mode() & 0o077 != 0
        || metadata.blksize() > limits.allocation_unit_bytes
        || std::fs::read_dir(format!("/proc/self/fd/{}", target.as_raw_fd()))?
            .next()
            .transpose()?
            .is_some()
    {
        return Err(invalid("V2 image target is not fresh private directory"));
    }
    let custody = Arc::new(
        space
            .reserve(limits.max_new_allocated_bytes)
            .map_err(invalid)?,
    );
    let result = (|| {
        let mut work = Work::default();
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
        let restored = AdmissionStore::open_existing(fresh_target, deadline, cancel)?;
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
    Ok(V2ImageOutcome { result, custody })
}
