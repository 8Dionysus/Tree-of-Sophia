use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use rustix::fs::{
    AtFlags, FlockOperation, Mode, OFlags, flock, fsync, linkat, mkdirat, openat, renameat,
    unlinkat,
};
use rustix::io::Errno;
use tos_fd_open::{OpenError, OpenErrorCode};
use tos_foundation::{Digest256, Digest256Hasher};

use crate::authenticated_tree::AuthenticatedTreeIoLedgerV1;
use crate::error::{Result, SegmentError, SegmentErrorCode as Code};
use crate::format::{self, FrameCoordinate, SegmentLimits};
use crate::generation::{
    GenerationShapeLimits, KeyComparatorV1, PackedPartitionRefV1, describe_placement_partition,
    placement_catalog_shape_root,
};
use crate::journal::{JournalFrame, PinJournal, PinState};
use crate::packed_leaf::PackedPlacementLeafV1;
use crate::placement::PlacementV1;
use crate::selected::{
    GenerationDescriptorV1, GenerationReadLimits, InstalledGenerationV1, check as check_generation,
};

const ROOT_MAGIC: &[u8; 8] = b"TOSROOT2";
const BLOCK_BYTES: usize = 64 * 1024;

fn allocation_upper_bound(bytes: u64, unit: u64) -> Result<u64> {
    if unit == 0 || unit == u64::MAX {
        return Err(SegmentError::new(
            Code::BudgetExceeded,
            "invalid persistent allocation quantum",
        ));
    }
    bytes
        .checked_add(unit - 1)
        .and_then(|n| n.checked_div(unit))
        .and_then(|n| n.checked_mul(unit))
        .and_then(|n| n.checked_add(unit))
        .ok_or_else(|| {
            SegmentError::new(Code::BudgetExceeded, "persistent allocation bound overflow")
        })
}

/// Opaque CMD-provided binding for one proposed owner unit. Storage preserves
/// these bytes; it does not decide their source meaning or authorization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnerBinding {
    pub profile_id: Vec<u8>,
    pub profile_version: Vec<u8>,
    pub subject_key: Vec<u8>,
    pub member_slot: u32,
}

/// A trusted, finite input that reaches EOF immediately after `declared_size`
/// bytes. `seal_segment` reads one extra byte to reject an understated size;
/// `Read` cannot impose a time bound on a pipe, socket or arbitrary callback.
/// The owner adapter must supply a closed/finite source with bounded blocking
/// behavior. This candidate does not accept untrusted live streams directly.
pub struct FrameInput<'a> {
    pub binding: OwnerBinding,
    pub declared_size: u64,
    pub declared_sha256: Digest256,
    pub reader: &'a mut dyn Read,
}

#[derive(Debug)]
struct Inner {
    _root: File,
    staging: File,
    segments: File,
    pins: File,
    attempts: Option<File>,
    leaves: Option<File>,
    generations: Option<File>,
    store_id: [u8; 16],
    domain: Vec<u8>,
    domain_digest: Digest256,
    limits: SegmentLimits,
}

#[derive(Clone, Debug)]
pub struct SegmentStore {
    inner: Arc<Inner>,
}

/// Explicit owner of a pin-directory flock. Close-on-exec only takes effect
/// when a child reaches `exec`; a concurrently forked test process may briefly
/// retain this open-file-description after its owner returns. Unlock on every
/// Rust scope exit so that transient child inheritance cannot extend a lease.
#[derive(Debug)]
pub(crate) struct PinDirectoryLease {
    file: File,
}

impl Drop for PinDirectoryLease {
    fn drop(&mut self) {
        let _ = flock(&self.file, FlockOperation::Unlock);
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ImmutableBlobInstallV1 {
    /// Bytes physically read to verify an already installed object or a
    /// concurrent no-replace install.
    pub read_bytes: u64,
    /// Payload bytes written to the staging file before no-replace install.
    pub written_bytes: u64,
    /// Exact allocated blocks retained by a newly linked immutable object.
    /// Reused objects and losing no-replace races report zero.
    pub allocated_bytes: u64,
}

/// Process-local custody of the exact anchored store that a caller audits.
/// The shared pin lock retains already sealed bytes against STO abort while
/// the caller carries its audit through candidate installation and selection.
/// A matching store ID or copied metadata cannot substitute for this handle;
/// a restored root needs a fresh full physical audit and a new handle.
#[derive(Clone, Debug)]
pub struct AuditedStoreRoot {
    inner: Arc<Inner>,
    _pin_lock: Arc<PinDirectoryLease>,
}

impl PartialEq for AuditedStoreRoot {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl Eq for AuditedStoreRoot {}

impl AuditedStoreRoot {
    pub fn require_store(&self, store: &SegmentStore) -> Result<()> {
        if !Arc::ptr_eq(&self.inner, &store.inner) {
            return Err(SegmentError::new(
                Code::InvalidReceipt,
                "audited store root differs",
            ));
        }
        Ok(())
    }

    pub fn require_installed(&self, installed: &InstalledGenerationV1) -> Result<()> {
        self.require_store(&installed.store)
    }
}

/// Only this crate can construct a verified receipt. Its field accessors are
/// descriptive; passing copied fields to CMD is never sufficient admission.
#[derive(Clone, Debug)]
pub struct ByteDurabilityReceipt {
    inner: Arc<Inner>,
    pin_id: [u8; 16],
    fence_epoch: u64,
    prepare_id: Vec<u8>,
    binding: OwnerBinding,
    segment_digest: Digest256,
    segment_size: u64,
    coordinate: FrameCoordinate,
    segment_dev: u64,
    segment_ino: u64,
    frame_index: u32,
    receipt_id: Digest256,
}

/// Observed local syscall profile; it does not claim storage-stack or
/// power-loss behavior beyond the filesystem's own sync guarantees.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DurabilityClass {
    LinuxFileAndDirectorySyncReopenSha256V1,
}

/// Caller-selected finite budget for pre-commit full-byte verification.
#[derive(Clone, Copy, Debug)]
pub struct VerificationBudget {
    pub max_receipts: usize,
    pub max_segments: usize,
    pub max_total_segment_bytes: u64,
}

/// A local, process-held pin lock and the receipts verified beneath it.
/// The coordinator holds this guard through its short metadata transaction.
/// Dropping it explicitly unlocks even if a concurrently forked child still
/// has the close-on-exec file description; durable sealed pins remain.
#[derive(Debug)]
pub struct VerifiedSealGuard {
    _pin_lock: PinDirectoryLease,
    receipts: Vec<ByteDurabilityReceipt>,
    prepare_id: Vec<u8>,
}

/// Physical custody observed through a durable, exact prepare-ID lookup.
/// None of these states decides whether CMD committed or may abort the attempt.
#[derive(Debug)]
pub enum AttemptRecovery {
    IntentOnly {
        pin_id: [u8; 16],
    },
    Preparing {
        pin_id: [u8; 16],
        fence_epoch: u64,
    },
    Sealed {
        receipts: Vec<ByteDurabilityReceipt>,
    },
    Aborted {
        pin_id: [u8; 16],
        fence_epoch: u64,
    },
}

impl VerifiedSealGuard {
    pub fn receipts(&self) -> &[ByteDurabilityReceipt] {
        &self.receipts
    }
    pub fn prepare_id(&self) -> &[u8] {
        &self.prepare_id
    }
}

impl ByteDurabilityReceipt {
    /// Descriptive persisted coordinate only. CMD must retain the private
    /// receipt through commit; cold readers must recover and verify it.
    pub fn placement(&self) -> PlacementV1 {
        PlacementV1 {
            store_id: self.inner.store_id,
            domain_digest: self.inner.domain_digest,
            pin_id: self.pin_id,
            fence_epoch: self.fence_epoch,
            receipt_id: self.receipt_id,
            segment_digest: self.segment_digest,
            segment_size: self.segment_size,
            frame_index: self.frame_index,
            coordinate: self.coordinate,
        }
    }
    pub fn pin_id(&self) -> [u8; 16] {
        self.pin_id
    }
    pub fn fence_epoch(&self) -> u64 {
        self.fence_epoch
    }
    pub fn store_id(&self) -> [u8; 16] {
        self.inner.store_id
    }
    /// Identity of the exact physical root directory held by this instance.
    /// It distinguishes a same-metadata clone from the selected source store.
    pub fn physical_root_identity(&self) -> Result<(u64, u64)> {
        let metadata = self
            .inner
            ._root
            .metadata()
            .map_err(|error| SegmentError::io("cannot stat held segment root", error))?;
        Ok((metadata.dev(), metadata.ino()))
    }
    pub fn domain_digest(&self) -> Digest256 {
        self.inner.domain_digest
    }
    pub fn custody_domain(&self) -> &[u8] {
        &self.inner.domain
    }
    pub fn prepare_id(&self) -> &[u8] {
        &self.prepare_id
    }
    pub fn binding(&self) -> &OwnerBinding {
        &self.binding
    }
    pub fn segment_digest(&self) -> Digest256 {
        self.segment_digest
    }
    pub fn segment_size(&self) -> u64 {
        self.segment_size
    }
    pub fn coordinate(&self) -> FrameCoordinate {
        self.coordinate
    }
    pub fn frame_index(&self) -> u32 {
        self.frame_index
    }
    pub fn receipt_id(&self) -> Digest256 {
        self.receipt_id
    }
    pub fn staging_id(&self) -> [u8; 16] {
        self.pin_id
    }
    pub fn durability_class(&self) -> DurabilityClass {
        DurabilityClass::LinuxFileAndDirectorySyncReopenSha256V1
    }
}

impl SegmentStore {
    /// Initialize an already existing empty, owner-controlled directory.
    /// This makes no corpus or CMD metadata and accepts no live source bytes.
    pub fn initialize_empty(root: &Path, domain: &[u8], limits: SegmentLimits) -> Result<Self> {
        Self::initialize_empty_root(open_root(root)?, domain, limits)
    }

    /// Initialize an already existing directory held by the caller. This is
    /// the descriptor-relative counterpart to `initialize_empty`: callers
    /// with a private namespace can keep the selected directory inode pinned
    /// instead of resolving its path again.
    pub fn initialize_empty_at(root: &File, domain: &[u8], limits: SegmentLimits) -> Result<Self> {
        Self::initialize_empty_root(held_root(root)?, domain, limits)
    }

    /// Account the exact held-root metadata write under the caller's IO and
    /// persistent-allocation ledger before creating the store header.
    pub fn initialize_empty_at_with_io(
        root: &File,
        domain: &[u8],
        limits: SegmentLimits,
        io: Arc<dyn AuthenticatedTreeIoLedgerV1>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Self> {
        Self::initialize_empty_root_with_io(
            held_root(root)?,
            domain,
            limits,
            Some(io),
            Some(deadline),
            Some(cancelled),
        )
    }

    fn initialize_empty_root(root_fd: File, domain: &[u8], limits: SegmentLimits) -> Result<Self> {
        Self::initialize_empty_root_with_io(root_fd, domain, limits, None, None, None)
    }

    fn initialize_empty_root_with_io(
        root_fd: File,
        domain: &[u8],
        limits: SegmentLimits,
        io: Option<Arc<dyn AuthenticatedTreeIoLedgerV1>>,
        deadline: Option<Instant>,
        cancelled: Option<&AtomicBool>,
    ) -> Result<Self> {
        let limits = limits.validate()?;
        if domain.is_empty() || domain.len() > u16::MAX as usize {
            return Err(SegmentError::new(
                Code::InvalidRoot,
                "invalid custody domain encoding",
            ));
        }
        let mut store_id = [0u8; 16];
        getrandom::fill(&mut store_id)
            .map_err(|_| SegmentError::new(Code::Io, "cannot create store instance ID"))?;
        let mut meta = Vec::with_capacity(8 + 16 + 2 + domain.len() + 32);
        meta.extend_from_slice(ROOT_MAGIC);
        meta.extend_from_slice(&store_id);
        meta.extend_from_slice(&(domain.len() as u16).to_le_bytes());
        meta.extend_from_slice(domain);
        let checksum = Digest256::of_bytes(&meta);
        meta.extend_from_slice(checksum.as_bytes());
        let directory_reservation = if let Some(io) = io.as_deref() {
            let unit = io.allocation_unit_bytes();
            let bytes = unit.checked_mul(6).ok_or_else(|| {
                SegmentError::new(Code::BudgetExceeded, "directory allocation bound overflow")
            })?;
            let upper = allocation_upper_bound(bytes, unit)?;
            if !io.reserve_allocated_bytes(upper) {
                return Err(SegmentError::new(
                    Code::BudgetExceeded,
                    "segment directory allocation refused",
                ));
            }
            Some((io, upper))
        } else {
            None
        };
        let allocation_reservation = if let Some(io) = io.as_deref() {
            let unit = io.allocation_unit_bytes();
            let bytes = meta.len() as u64;
            let upper = allocation_upper_bound(bytes, unit)?;
            if !io.reserve_allocated_bytes(upper) {
                return Err(SegmentError::new(
                    Code::BudgetExceeded,
                    "store metadata allocation refused",
                ));
            }
            if deadline
                .zip(cancelled)
                .is_some_and(|(deadline, cancelled)| check_generation(deadline, cancelled).is_err())
            {
                return Err(SegmentError::new(
                    Code::Cancelled,
                    "store metadata selection expired",
                ));
            }
            Some((io, upper))
        } else {
            None
        };
        for name in [
            "staging",
            "segments",
            "pins",
            "attempts",
            "leaves",
            "generations",
        ] {
            mkdirat(&root_fd, name, Mode::RUSR | Mode::WUSR | Mode::XUSR).map_err(|error| {
                SegmentError::io("cannot initialize segment directory", error.into())
            })?;
        }
        let staging = open_directory(&root_fd, "staging")?;
        let segments = open_directory(&root_fd, "segments")?;
        let pins = open_directory(&root_fd, "pins")?;
        let attempts = open_directory(&root_fd, "attempts")?;
        let leaves = open_directory(&root_fd, "leaves")?;
        let generations = open_directory(&root_fd, "generations")?;
        for directory in [&staging, &segments, &pins, &attempts, &leaves, &generations] {
            fsync(directory)
                .map_err(|error| SegmentError::io("cannot sync segment directory", error.into()))?;
        }
        if let Some((io, reserved)) = directory_reservation {
            let actual = [&staging, &segments, &pins, &attempts, &leaves, &generations]
                .into_iter()
                .try_fold(0u64, |sum, directory| {
                    directory
                        .metadata()
                        .ok()?
                        .blocks()
                        .checked_mul(512)
                        .and_then(|bytes| sum.checked_add(bytes))
                })
                .ok_or_else(|| {
                    SegmentError::new(Code::BudgetExceeded, "directory allocation overflow")
                })?;
            if !io.reconcile_allocated_bytes(reserved, actual) {
                return Err(SegmentError::new(
                    Code::BudgetExceeded,
                    "segment directory allocation reconciliation refused",
                ));
            }
        }
        if let Some(io) = io.as_deref()
            && !io.charge_write(meta.len() as u64)
        {
            return Err(SegmentError::new(
                Code::BudgetExceeded,
                "store metadata IO refused",
            ));
        }
        let mut file = create_exclusive(&root_fd, "store.meta")?;
        file.write_all(&meta)
            .map_err(|error| SegmentError::io("cannot write store metadata", error))?;
        if let Some(io) = io.as_deref()
            && !io.record_write_returned(meta.len() as u64)
        {
            return Err(SegmentError::new(
                Code::BudgetExceeded,
                "store metadata IO return refused",
            ));
        }
        file.sync_all()
            .map_err(|error| SegmentError::io("cannot sync store metadata", error))?;
        if let Some((io, reserved)) = allocation_reservation {
            let actual = file
                .metadata()
                .map_err(|error| SegmentError::io("cannot stat store metadata", error))?
                .blocks()
                .checked_mul(512)
                .ok_or_else(|| {
                    SegmentError::new(Code::BudgetExceeded, "store metadata allocation overflow")
                })?;
            if !io.reconcile_allocated_bytes(reserved, actual) {
                return Err(SegmentError::new(
                    Code::BudgetExceeded,
                    "store metadata allocation reconciliation refused",
                ));
            }
        }
        fsync(&root_fd)
            .map_err(|error| SegmentError::io("cannot sync store directory", error.into()))?;
        Ok(Self {
            inner: Arc::new(Inner {
                _root: root_fd,
                staging,
                segments,
                pins,
                attempts: Some(attempts),
                leaves: Some(leaves),
                generations: Some(generations),
                store_id,
                domain: domain.to_vec(),
                domain_digest: Digest256::of_bytes(domain),
                limits,
            }),
        })
    }

    pub fn open_existing(root: &Path, limits: SegmentLimits) -> Result<Self> {
        Self::open_existing_root(open_root(root)?, limits)
    }

    /// Open an existing store below the caller's held root descriptor.
    pub fn open_existing_at(root: &File, limits: SegmentLimits) -> Result<Self> {
        Self::open_existing_root(held_root(root)?, limits)
    }

    /// Open an existing store through the exact held root while charging its
    /// bounded metadata read to the same caller-owned ledger as tree frames.
    pub fn open_existing_at_with_io(
        root: &File,
        limits: SegmentLimits,
        io: Arc<dyn AuthenticatedTreeIoLedgerV1>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Self> {
        Self::open_existing_root_with_io(
            held_root(root)?,
            limits,
            Some(io),
            Some(deadline),
            Some(cancelled),
        )
    }

    fn open_existing_root(root_fd: File, limits: SegmentLimits) -> Result<Self> {
        Self::open_existing_root_with_io(root_fd, limits, None, None, None)
    }

    fn open_existing_root_with_io(
        root_fd: File,
        limits: SegmentLimits,
        io: Option<Arc<dyn AuthenticatedTreeIoLedgerV1>>,
        deadline: Option<Instant>,
        cancelled: Option<&AtomicBool>,
    ) -> Result<Self> {
        let limits = limits.validate()?;
        let staging = open_directory(&root_fd, "staging")?;
        let segments = open_directory(&root_fd, "segments")?;
        let pins = open_directory(&root_fd, "pins")?;
        // Older physical roots remain readable by exact known placement.
        // Their missing prepare-keyed intent route cannot be used by a new
        // writer or treated as evidence of an absent historical attempt.
        let attempts = match open_directory(&root_fd, "attempts") {
            Ok(attempts) => Some(attempts),
            Err(error) if is_missing(&error) => None,
            Err(error) => return Err(error),
        };
        let leaves = match open_directory(&root_fd, "leaves") {
            Ok(leaves) => Some(leaves),
            Err(error) if is_missing(&error) => None,
            Err(error) => return Err(error),
        };
        let generations = match open_directory(&root_fd, "generations") {
            Ok(generations) => Some(generations),
            Err(error) if is_missing(&error) => None,
            Err(error) => return Err(error),
        };
        let mut meta_file = open_regular(&root_fd, "store.meta")?;
        let before = meta_file
            .metadata()
            .map_err(|error| SegmentError::io("cannot stat store metadata", error))?;
        if !before.is_file() || before.len() < 8 + 16 + 2 + 32 || before.len() > 65_594 {
            return Err(SegmentError::new(
                Code::InvalidRoot,
                "store metadata size differs",
            ));
        }
        let mut meta = Vec::new();
        let meta_len = usize::try_from(before.len()).map_err(|_| {
            SegmentError::new(Code::BudgetExceeded, "store metadata exceeds address space")
        })?;
        meta.try_reserve_exact(meta_len).map_err(|_| {
            SegmentError::new(Code::BudgetExceeded, "store metadata allocation failed")
        })?;
        while meta.len() < meta_len {
            if let (Some(deadline), Some(cancelled)) = (deadline, cancelled) {
                check_generation(deadline, cancelled)?;
            }
            let wanted = (meta_len - meta.len()).min(BLOCK_BYTES);
            if let Some(io) = io.as_deref()
                && !io.charge_read(wanted as u64)
            {
                return Err(SegmentError::new(
                    Code::BudgetExceeded,
                    "store metadata read refused",
                ));
            }
            let mut block = [0u8; BLOCK_BYTES];
            let read = meta_file
                .read(&mut block[..wanted])
                .map_err(|error| SegmentError::io("cannot read store metadata", error))?;
            if let Some(io) = io.as_deref()
                && !io.record_read_returned(read as u64)
            {
                return Err(SegmentError::new(
                    Code::BudgetExceeded,
                    "store metadata read return refused",
                ));
            }
            if read == 0 {
                return Err(SegmentError::new(
                    Code::InvalidRoot,
                    "store metadata ended early",
                ));
            }
            meta.extend_from_slice(&block[..read]);
        }
        if let Some(io) = io.as_deref()
            && !io.charge_read(1)
        {
            return Err(SegmentError::new(
                Code::BudgetExceeded,
                "store metadata EOF read refused",
            ));
        }
        let mut tail = [0u8; 1];
        let tail_read = meta_file
            .read(&mut tail)
            .map_err(|error| SegmentError::io("cannot read store metadata EOF", error))?;
        if let Some(io) = io.as_deref()
            && !io.record_read_returned(tail_read as u64)
        {
            return Err(SegmentError::new(
                Code::BudgetExceeded,
                "store metadata EOF return refused",
            ));
        }
        let stamp = |m: &std::fs::Metadata| {
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
        };
        let selected = open_regular(&root_fd, "store.meta")?;
        if tail_read != 0
            || stamp(
                &meta_file
                    .metadata()
                    .map_err(|error| SegmentError::io("cannot restat store metadata", error))?,
            ) != stamp(&before)
            || stamp(
                &selected
                    .metadata()
                    .map_err(|error| SegmentError::io("cannot verify store metadata", error))?,
            ) != stamp(&before)
        {
            return Err(SegmentError::new(
                Code::CorruptBytes,
                "store metadata EOF or custody differs",
            ));
        }
        if meta.len() < 8 + 16 + 2 + 32 || &meta[..8] != ROOT_MAGIC {
            return Err(SegmentError::new(
                Code::InvalidRoot,
                "store metadata differs",
            ));
        }
        let mut store_id = [0u8; 16];
        store_id.copy_from_slice(&meta[8..24]);
        let domain_len = u16::from_le_bytes([meta[24], meta[25]]) as usize;
        let end = 26usize
            .checked_add(domain_len)
            .and_then(|value| value.checked_add(32))
            .ok_or_else(|| {
                SegmentError::new(Code::InvalidRoot, "store metadata length overflow")
            })?;
        if domain_len == 0
            || meta.len() != end
            || Digest256::of_bytes(&meta[..end - 32]).as_bytes() != &meta[end - 32..]
        {
            return Err(SegmentError::new(
                Code::InvalidRoot,
                "store metadata checksum or length differs",
            ));
        }
        let domain = meta[26..end - 32].to_vec();
        let domain_digest = Digest256::of_bytes(&domain);
        Ok(Self {
            inner: Arc::new(Inner {
                _root: root_fd,
                staging,
                segments,
                pins,
                attempts,
                leaves,
                generations,
                store_id,
                domain,
                domain_digest,
                limits,
            }),
        })
    }

    pub fn store_id(&self) -> [u8; 16] {
        self.inner.store_id
    }
    pub fn domain_digest(&self) -> Digest256 {
        self.inner.domain_digest
    }
    pub fn custody_domain(&self) -> &[u8] {
        &self.inner.domain
    }

    /// Hold the exact anchored store through a caller's physical audit and
    /// subsequent selection decision. This does not itself audit membership
    /// or authorize disclosure.
    pub fn hold_audit_root(&self) -> Result<AuditedStoreRoot> {
        Ok(AuditedStoreRoot {
            inner: self.inner.clone(),
            _pin_lock: Arc::new(self.hold_generation_pin()?),
        })
    }

    /// Install exact packed placement bytes by content digest. This is an
    /// immutable physical leaf, not a selected or complete CMD generation.
    /// Existing roots without a leaves directory stay readable by placement
    /// but cannot install a new leaf implicitly.
    pub fn install_packed_leaf(
        &self,
        leaf: &PackedPlacementLeafV1,
        limits: GenerationShapeLimits,
    ) -> Result<Digest256> {
        if leaf.domain_digest != self.inner.domain_digest {
            return Err(SegmentError::new(
                Code::InvalidReceipt,
                "packed leaf custody domain differs",
            ));
        }
        let raw = leaf.encode(limits)?;
        let digest = Digest256::of_bytes(&raw);
        let leaves = self.inner.leaves.as_ref().ok_or_else(|| {
            SegmentError::new(Code::InvalidRoot, "store lacks immutable leaf directory")
        })?;
        let name = digest.to_hex();
        let mut stage_id = [0u8; 16];
        getrandom::fill(&mut stage_id)
            .map_err(|_| SegmentError::new(Code::Io, "cannot create leaf staging ID"))?;
        let stage_name = format!("{}.part", hex_id(stage_id));
        let mut stage = create_exclusive(leaves, &stage_name)?;
        stage
            .write_all(&raw)
            .map_err(|error| SegmentError::io("cannot write staged packed leaf", error))?;
        stage
            .sync_all()
            .map_err(|error| SegmentError::io("cannot sync staged packed leaf", error))?;
        drop(stage);
        match linkat(
            leaves,
            stage_name.as_str(),
            leaves,
            name.as_str(),
            AtFlags::empty(),
        ) {
            Ok(()) | Err(Errno::EXIST) => {}
            Err(error) => {
                return Err(SegmentError::io(
                    "cannot no-replace install packed leaf",
                    error.into(),
                ));
            }
        }
        fsync(leaves)
            .map_err(|error| SegmentError::io("cannot sync packed leaf directory", error.into()))?;
        unlinkat(leaves, stage_name.as_str(), AtFlags::empty())
            .map_err(|error| SegmentError::io("cannot unlink staged packed leaf", error.into()))?;
        fsync(leaves)
            .map_err(|error| SegmentError::io("cannot sync packed leaf directory", error.into()))?;
        let readback = self.open_packed_leaf(digest, limits)?;
        if &readback != leaf {
            return Err(SegmentError::new(
                Code::CorruptBytes,
                "packed leaf readback differs",
            ));
        }
        open_regular(leaves, &name)?
            .sync_all()
            .map_err(|error| SegmentError::io("cannot sync verified packed leaf", error))?;
        fsync(leaves).map_err(|error| {
            SegmentError::io("cannot sync verified leaf directory", error.into())
        })?;
        Ok(digest)
    }

    /// Cold exact content read for a caller-selected immutable leaf. A leaf
    /// path is derived from SHA-256; this does not authorize a negative key.
    pub fn open_packed_leaf(
        &self,
        digest: Digest256,
        limits: GenerationShapeLimits,
    ) -> Result<PackedPlacementLeafV1> {
        let limits = limits.validate()?;
        let leaves = self.inner.leaves.as_ref().ok_or_else(|| {
            SegmentError::new(Code::InvalidRoot, "store lacks immutable leaf directory")
        })?;
        let mut raw = Vec::new();
        open_regular(leaves, &digest.to_hex())?
            .take(limits.max_leaf_bytes + 1)
            .read_to_end(&mut raw)
            .map_err(|error| SegmentError::io("cannot read packed leaf", error))?;
        if Digest256::of_bytes(&raw) != digest {
            return Err(SegmentError::new(
                Code::CorruptBytes,
                "packed leaf content digest differs",
            ));
        }
        let leaf = PackedPlacementLeafV1::decode(&raw, limits)?;
        if leaf.domain_digest != self.inner.domain_digest {
            return Err(SegmentError::new(
                Code::InvalidReceipt,
                "packed leaf custody domain differs",
            ));
        }
        Ok(leaf)
    }

    /// Offline catalog-shape check. Reopen every exact physical leaf, compare
    /// its canonical semantic rows and cover all declared key intervals.
    /// The caller must independently prove exhaustive CMD history, selected
    /// cut and predicate coverage; this does not admit warm negative reads.
    pub fn verify_packed_catalog_shape(
        &self,
        domain: &[u8],
        namespace: &[u8],
        scope: &[u8],
        key_codec_digest: Digest256,
        comparator: KeyComparatorV1,
        partitions: &[PackedPartitionRefV1],
        limits: GenerationShapeLimits,
    ) -> Result<Digest256> {
        let limits = limits.validate()?;
        if Digest256::of_bytes(domain) != self.inner.domain_digest
            || partitions.is_empty()
            || partitions.len() > limits.max_partitions
        {
            return Err(SegmentError::new(
                Code::InvalidReceipt,
                "packed catalog domain or partition count differs",
            ));
        }
        let mut semantic = Vec::with_capacity(partitions.len());
        for reference in partitions {
            let leaf = self.open_packed_leaf(reference.content_digest, limits)?;
            if leaf.bounds != reference.semantic.bounds {
                return Err(SegmentError::new(
                    Code::InvalidReceipt,
                    "packed leaf interval differs",
                ));
            }
            let described = describe_placement_partition(
                self.inner.domain_digest,
                leaf.bounds,
                leaf.rows.into_iter().map(Ok),
                limits,
            )?;
            if described != reference.semantic {
                return Err(SegmentError::new(
                    Code::InvalidReceipt,
                    "packed leaf semantic rows differ",
                ));
            }
            semantic.push(described);
        }
        let semantic_root = placement_catalog_shape_root(
            domain,
            namespace,
            scope,
            key_codec_digest,
            comparator,
            &semantic,
            limits,
        )?;
        let mut hasher = Digest256Hasher::new();
        hasher.update(b"tos-packed-placement-catalog-v1");
        hasher.update(semantic_root.as_bytes());
        hasher.update(&(partitions.len() as u32).to_le_bytes());
        for reference in partitions {
            hasher.update(reference.content_digest.as_bytes());
        }
        Ok(hasher.finalize())
    }

    /// Cold-install an immutable two-namespace generation candidate. This
    /// checks every referenced physical leaf, but only CMD's independent
    /// complete history/current audit and fenced DB selection can publish it.
    pub fn install_generation_candidate(
        &self,
        descriptor: GenerationDescriptorV1,
        limits: GenerationReadLimits,
    ) -> Result<InstalledGenerationV1> {
        let limits = limits.validate()?;
        let _pin_lock = self.hold_generation_pin()?;
        if descriptor.cut.store_id != self.inner.store_id
            || descriptor.cut.domain_digest != self.inner.domain_digest
        {
            return Err(SegmentError::new(
                Code::InvalidReceipt,
                "generation store/domain differs",
            ));
        }
        for (namespace, catalog) in [
            (b"cmd2.history.v1".as_slice(), &descriptor.history),
            (b"cmd2.current.v1".as_slice(), &descriptor.current),
        ] {
            let root = self.verify_packed_catalog_shape(
                &self.inner.domain,
                namespace,
                b"all",
                catalog.key_codec_digest,
                KeyComparatorV1::RawUnsignedBytes,
                &catalog.partitions,
                limits.shape,
            )?;
            if root != catalog.catalog_root {
                return Err(SegmentError::new(
                    Code::InvalidReceipt,
                    "generation catalog root differs",
                ));
            }
        }
        let raw = descriptor.encode(&self.inner.domain, limits)?;
        let digest = Digest256::of_bytes(&raw);
        let generations = self.inner.generations.as_ref().ok_or_else(|| {
            SegmentError::new(
                Code::InvalidRoot,
                "store lacks immutable generation directory",
            )
        })?;
        let name = digest.to_hex();
        let mut id = [0u8; 16];
        getrandom::fill(&mut id)
            .map_err(|_| SegmentError::new(Code::Io, "cannot create generation staging ID"))?;
        let stage_name = format!("{}.part", hex_id(id));
        let mut stage = create_exclusive(generations, &stage_name)?;
        stage
            .write_all(&raw)
            .map_err(|error| SegmentError::io("cannot write staged generation", error))?;
        stage
            .sync_all()
            .map_err(|error| SegmentError::io("cannot sync staged generation", error))?;
        drop(stage);
        match linkat(
            generations,
            stage_name.as_str(),
            generations,
            name.as_str(),
            AtFlags::empty(),
        ) {
            Ok(()) | Err(Errno::EXIST) => {}
            Err(error) => {
                return Err(SegmentError::io(
                    "cannot no-replace install generation",
                    error.into(),
                ));
            }
        }
        fsync(generations)
            .map_err(|error| SegmentError::io("cannot sync generation directory", error.into()))?;
        unlinkat(generations, stage_name.as_str(), AtFlags::empty())
            .map_err(|error| SegmentError::io("cannot unlink staged generation", error.into()))?;
        fsync(generations)
            .map_err(|error| SegmentError::io("cannot sync generation directory", error.into()))?;
        let opened = self.open_generation_candidate(digest, &descriptor.cut, limits)?;
        if opened.descriptor() != &descriptor {
            return Err(SegmentError::new(
                Code::CorruptBytes,
                "installed generation readback differs",
            ));
        }
        open_regular(generations, &name)?
            .sync_all()
            .map_err(|error| SegmentError::io("cannot sync verified generation", error))?;
        fsync(generations).map_err(|error| {
            SegmentError::io("cannot sync verified generation directory", error.into())
        })?;
        Ok(opened)
    }

    /// Reopen an exact descriptor selected by CMD. The expected cut must come
    /// from CMD's live selected DB row; a caller-supplied digest alone is not
    /// complete-membership or disclosure authority.
    pub fn open_generation_candidate(
        &self,
        digest: Digest256,
        expected_cut: &crate::selected::GenerationCutV1,
        limits: GenerationReadLimits,
    ) -> Result<InstalledGenerationV1> {
        let limits = limits.validate()?;
        let pin_lock = Arc::new(self.hold_generation_pin()?);
        let generations = self.inner.generations.as_ref().ok_or_else(|| {
            SegmentError::new(
                Code::InvalidRoot,
                "store lacks immutable generation directory",
            )
        })?;
        let mut raw = Vec::new();
        open_regular(generations, &digest.to_hex())?
            .take(limits.max_descriptor_bytes as u64 + 1)
            .read_to_end(&mut raw)
            .map_err(|error| SegmentError::io("cannot read generation descriptor", error))?;
        if Digest256::of_bytes(&raw) != digest {
            return Err(SegmentError::new(
                Code::CorruptBytes,
                "generation digest differs",
            ));
        }
        let descriptor = GenerationDescriptorV1::decode(&raw, &self.inner.domain, limits)?;
        if descriptor.cut != *expected_cut || descriptor.cut.store_id != self.inner.store_id {
            return Err(SegmentError::new(
                Code::InvalidReceipt,
                "selected generation cut differs",
            ));
        }
        Ok(InstalledGenerationV1 {
            store: self.clone(),
            digest,
            descriptor,
            pin_lock,
        })
    }

    /// Install one bounded immutable content-addressed object in the existing
    /// generation directory. The caller supplies a format that binds its
    /// domain and kind. A prior object at the same address is reopened and
    /// compared byte-for-byte before reuse.
    pub(crate) fn install_authenticated_blob(
        &self,
        digest: Digest256,
        raw: &[u8],
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<ImmutableBlobInstallV1> {
        self.install_authenticated_blob_accounted(digest, raw, max_bytes, deadline, cancelled, None)
    }

    pub(crate) fn install_authenticated_blob_accounted(
        &self,
        digest: Digest256,
        raw: &[u8],
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
        allocation_ledger: Option<&dyn AuthenticatedTreeIoLedgerV1>,
    ) -> Result<ImmutableBlobInstallV1> {
        check_generation(deadline, cancelled)?;
        if raw.len() > max_bytes || Digest256::of_bytes(raw) != digest {
            return Err(SegmentError::new(
                Code::BudgetExceeded,
                "authenticated object exceeds limit or digest differs",
            ));
        }
        let generations = self.inner.generations.as_ref().ok_or_else(|| {
            SegmentError::new(
                Code::InvalidRoot,
                "store lacks immutable generation directory",
            )
        })?;
        let name = digest.to_hex();
        match open_regular(generations, &name) {
            Ok(file) => {
                let existing = read_bounded_regular(file, max_bytes, deadline, cancelled)?;
                if existing != raw {
                    return Err(SegmentError::new(
                        Code::CorruptBytes,
                        "content-addressed object readback differs",
                    ));
                }
                return Ok(ImmutableBlobInstallV1 {
                    read_bytes: existing.len() as u64,
                    written_bytes: 0,
                    allocated_bytes: 0,
                });
            }
            Err(error) if is_missing(&error) => {}
            Err(error) => return Err(error),
        }

        let allocation_reservation = if let Some(ledger) = allocation_ledger {
            let unit = ledger.allocation_unit_bytes();
            if unit == 0 || unit == u64::MAX {
                return Err(SegmentError::new(
                    Code::BudgetExceeded,
                    "authenticated pack allocation unit is invalid",
                ));
            }
            let bytes = u64::try_from(raw.len()).map_err(|_| {
                SegmentError::new(
                    Code::BudgetExceeded,
                    "authenticated pack size exceeds range",
                )
            })?;
            let upper = bytes
                .checked_add(unit - 1)
                .and_then(|n| n.checked_div(unit))
                .and_then(|n| n.checked_mul(unit))
                .and_then(|n| n.checked_add(unit))
                .ok_or_else(|| {
                    SegmentError::new(
                        Code::BudgetExceeded,
                        "authenticated pack allocation precharge overflow",
                    )
                })?;
            if !ledger.reserve_allocated_bytes(upper) {
                return Err(SegmentError::new(
                    Code::BudgetExceeded,
                    "authenticated pack allocation reservation refused",
                ));
            }
            Some((ledger, upper))
        } else {
            None
        };

        let mut stage_id = [0u8; 16];
        getrandom::fill(&mut stage_id)
            .map_err(|_| SegmentError::new(Code::Io, "cannot create object staging ID"))?;
        let stage_name = format!("{}.part", hex_id(stage_id));
        let generations_before = generations
            .metadata()
            .map_err(|error| SegmentError::io("cannot stat object directory", error))?
            .blocks()
            .checked_mul(512)
            .ok_or_else(|| {
                SegmentError::new(Code::BudgetExceeded, "directory allocation overflow")
            })?;
        let mut stage = create_exclusive(generations, &stage_name)?;
        check_generation(deadline, cancelled)?;
        stage
            .write_all(raw)
            .map_err(|error| SegmentError::io("cannot write authenticated object", error))?;
        stage
            .sync_all()
            .map_err(|error| SegmentError::io("cannot sync authenticated object", error))?;
        let staged_allocated = stage
            .metadata()
            .map_err(|error| SegmentError::io("cannot stat authenticated object", error))?
            .blocks()
            .checked_mul(512)
            .ok_or_else(|| SegmentError::new(Code::BudgetExceeded, "object allocation overflow"))?;
        let installed = match linkat(
            generations,
            stage_name.as_str(),
            generations,
            name.as_str(),
            AtFlags::empty(),
        ) {
            Ok(()) => true,
            Err(Errno::EXIST) => false,
            Err(error) => {
                return Err(SegmentError::io(
                    "cannot no-replace install authenticated object",
                    error.into(),
                ));
            }
        };
        drop(stage);
        fsync(generations).map_err(|error| {
            SegmentError::io("cannot sync authenticated object directory", error.into())
        })?;
        unlinkat(generations, stage_name.as_str(), AtFlags::empty()).map_err(|error| {
            SegmentError::io(
                "cannot unlink authenticated object staging file",
                error.into(),
            )
        })?;
        fsync(generations).map_err(|error| {
            SegmentError::io("cannot sync authenticated object directory", error.into())
        })?;
        let generations_after = generations
            .metadata()
            .map_err(|error| SegmentError::io("cannot restat object directory", error))?
            .blocks()
            .checked_mul(512)
            .ok_or_else(|| {
                SegmentError::new(Code::BudgetExceeded, "directory allocation overflow")
            })?;
        let directory_growth = generations_after.saturating_sub(generations_before);
        check_generation(deadline, cancelled)?;
        let readback = read_bounded_regular(
            open_regular(generations, &name)?,
            max_bytes,
            deadline,
            cancelled,
        )?;
        if readback != raw {
            return Err(SegmentError::new(
                Code::CorruptBytes,
                "authenticated object readback differs",
            ));
        }
        let read_bytes = readback.len() as u64;
        let allocated_bytes = if installed {
            staged_allocated
                .checked_add(directory_growth)
                .ok_or_else(|| {
                    SegmentError::new(
                        Code::BudgetExceeded,
                        "authenticated pack allocated bytes overflow",
                    )
                })?
        } else {
            directory_growth
        };
        if allocation_reservation.is_some_and(|(ledger, reserved)| {
            !ledger.reconcile_allocated_bytes(reserved, allocated_bytes)
        }) {
            return Err(SegmentError::new(
                Code::BudgetExceeded,
                "authenticated pack allocation reconciliation refused",
            ));
        }
        Ok(ImmutableBlobInstallV1 {
            read_bytes,
            written_bytes: raw.len() as u64,
            allocated_bytes: if installed { allocated_bytes } else { 0 },
        })
    }

    /// Allocated blocks in the immutable store's fixed metadata file. Callers
    /// include this only when they created a fresh V2 physical root.
    pub fn store_metadata_allocated_bytes(&self) -> Result<u64> {
        let file = open_regular(&self.inner._root, "store.meta")?;
        file.metadata()
            .map_err(|error| SegmentError::io("cannot stat segment store metadata", error))?
            .blocks()
            .checked_mul(512)
            .ok_or_else(|| SegmentError::new(Code::BudgetExceeded, "store allocation overflow"))
    }

    /// Read exact bounded bytes from the existing content-addressed generation
    /// directory. The enclosing format is verified by the tree/object owner.
    pub(crate) fn read_authenticated_blob(
        &self,
        digest: Digest256,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Vec<u8>> {
        check_generation(deadline, cancelled)?;
        let generations = self.inner.generations.as_ref().ok_or_else(|| {
            SegmentError::new(
                Code::InvalidRoot,
                "store lacks immutable generation directory",
            )
        })?;
        let raw = read_bounded_regular(
            open_regular(generations, &digest.to_hex())?,
            max_bytes,
            deadline,
            cancelled,
        )?;
        if Digest256::of_bytes(&raw) != digest {
            return Err(SegmentError::new(
                Code::CorruptBytes,
                "authenticated object content digest differs",
            ));
        }
        Ok(raw)
    }

    /// Same digest-bound object read with a precharged caller IO ledger. The
    /// immutable file size is observed through its held descriptor before the
    /// exact byte allowance is issued.
    pub(crate) fn read_authenticated_blob_with_io(
        &self,
        digest: Digest256,
        max_bytes: usize,
        io_ledger: Option<&dyn AuthenticatedTreeIoLedgerV1>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Vec<u8>> {
        check_generation(deadline, cancelled)?;
        let generations = self.inner.generations.as_ref().ok_or_else(|| {
            SegmentError::new(
                Code::InvalidRoot,
                "store lacks immutable generation directory",
            )
        })?;
        let file = open_regular(generations, &digest.to_hex())?;
        let metadata = file
            .metadata()
            .map_err(|error| SegmentError::io("cannot stat authenticated object", error))?;
        require_private_generation_file(&metadata)?;
        let declared = usize::try_from(metadata.len()).map_err(|_| {
            SegmentError::new(
                Code::BudgetExceeded,
                "authenticated object exceeds address space",
            )
        })?;
        if declared > max_bytes {
            return Err(SegmentError::new(
                Code::BudgetExceeded,
                "authenticated object exceeds read limit",
            ));
        }
        if io_ledger.is_some_and(|ledger| !ledger.charge_read(declared as u64)) {
            return Err(SegmentError::new(
                Code::BudgetExceeded,
                "authenticated object read reservation refused",
            ));
        }
        let raw = read_bounded_regular(file, max_bytes, deadline, cancelled)?;
        if Digest256::of_bytes(&raw) != digest {
            return Err(SegmentError::new(
                Code::CorruptBytes,
                "authenticated object content digest differs",
            ));
        }
        if io_ledger.is_some_and(|ledger| !ledger.record_read_returned(raw.len() as u64)) {
            return Err(SegmentError::new(
                Code::BudgetExceeded,
                "authenticated object read reconciliation refused",
            ));
        }
        Ok(raw)
    }

    /// Read one bounded frame from a content-addressed immutable generation
    /// object without materializing the rest of its pack. The caller verifies
    /// the frame commitment; full object SHA-256 is checked at install/readback
    /// and explicit cold closure, not on every warm path traversal.
    pub(crate) fn read_authenticated_blob_range(
        &self,
        digest: Digest256,
        offset: u64,
        length: usize,
        max_object_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Vec<u8>> {
        check_generation(deadline, cancelled)?;
        if length == 0 || length > max_object_bytes {
            return Err(SegmentError::new(
                Code::BudgetExceeded,
                "authenticated frame exceeds read limit",
            ));
        }
        let generations = self.inner.generations.as_ref().ok_or_else(|| {
            SegmentError::new(
                Code::InvalidRoot,
                "store lacks immutable generation directory",
            )
        })?;
        let name = digest.to_hex();
        let mut file = open_regular(generations, &name)?;
        let before = file
            .metadata()
            .map_err(|error| SegmentError::io("cannot stat authenticated pack", error))?;
        require_private_generation_file(&before)?;
        let object_len = before.len();
        let end = offset
            .checked_add(length as u64)
            .ok_or_else(|| SegmentError::new(Code::BudgetExceeded, "frame offset overflow"))?;
        if object_len > max_object_bytes as u64 || end > object_len {
            return Err(SegmentError::new(
                Code::CorruptBytes,
                "authenticated frame lies outside its immutable pack",
            ));
        }
        let mut raw = Vec::new();
        raw.try_reserve_exact(length).map_err(|_| {
            SegmentError::new(
                Code::BudgetExceeded,
                "authenticated frame allocation failed",
            )
        })?;
        raw.resize(length, 0);
        file.seek(SeekFrom::Start(offset))
            .map_err(|error| SegmentError::io("cannot seek authenticated frame", error))?;
        for block in raw.chunks_mut(64 * 1024) {
            check_generation(deadline, cancelled)?;
            file.read_exact(block)
                .map_err(|error| SegmentError::io("cannot read authenticated frame", error))?;
        }
        let after = file
            .metadata()
            .map_err(|error| SegmentError::io("cannot restat authenticated pack", error))?;
        require_same_generation_file(&before, &after)?;
        let named_after = open_regular(generations, &name)?
            .metadata()
            .map_err(|error| SegmentError::io("cannot restat authenticated pack path", error))?;
        require_same_generation_file(&before, &named_after)?;
        check_generation(deadline, cancelled)?;
        Ok(raw)
    }

    pub(crate) fn hold_generation_pin(&self) -> Result<PinDirectoryLease> {
        self.lock_pin_dir(FlockOperation::NonBlockingLockShared)
    }

    pub(crate) fn open_packed_leaf_checked(
        &self,
        digest: Digest256,
        limits: GenerationShapeLimits,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<PackedPlacementLeafV1> {
        let limits = limits.validate()?;
        check_generation(deadline, cancelled)?;
        let leaves = self.inner.leaves.as_ref().ok_or_else(|| {
            SegmentError::new(Code::InvalidRoot, "store lacks immutable leaf directory")
        })?;
        let mut file = open_regular(leaves, &digest.to_hex())?;
        let length = file
            .metadata()
            .map_err(|error| SegmentError::io("cannot stat selected leaf", error))?
            .len();
        if length > limits.max_leaf_bytes {
            return Err(SegmentError::new(
                Code::BudgetExceeded,
                "selected leaf exceeds limit",
            ));
        }
        let mut raw = Vec::new();
        let length = usize::try_from(length).map_err(|_| {
            SegmentError::new(Code::BudgetExceeded, "selected leaf exceeds address space")
        })?;
        raw.try_reserve_exact(length).map_err(|_| {
            SegmentError::new(Code::BudgetExceeded, "selected leaf allocation failed")
        })?;
        let mut block = [0u8; BLOCK_BYTES];
        loop {
            check_generation(deadline, cancelled)?;
            let count = file
                .read(&mut block)
                .map_err(|error| SegmentError::io("cannot read selected leaf", error))?;
            if count == 0 {
                break;
            }
            if raw
                .len()
                .checked_add(count)
                .is_none_or(|n| n as u64 > limits.max_leaf_bytes)
            {
                return Err(SegmentError::new(
                    Code::BudgetExceeded,
                    "selected leaf grew beyond limit",
                ));
            }
            raw.extend_from_slice(&block[..count]);
        }
        check_generation(deadline, cancelled)?;
        if Digest256::of_bytes(&raw) != digest {
            return Err(SegmentError::new(
                Code::CorruptBytes,
                "selected leaf digest differs",
            ));
        }
        let leaf = PackedPlacementLeafV1::decode(&raw, limits)?;
        if leaf.domain_digest != self.inner.domain_digest {
            return Err(SegmentError::new(
                Code::InvalidReceipt,
                "selected leaf domain differs",
            ));
        }
        Ok(leaf)
    }

    /// Seal multiple bounded frames under one durable pin. All frames belong
    /// to the same CMD prepare ID but retain independent owner bindings.
    /// Every input must satisfy `FrameInput`'s finite-reader precondition.
    pub fn seal_segment(
        &self,
        prepare_id: &[u8],
        frames: &mut [FrameInput<'_>],
    ) -> Result<Vec<ByteDurabilityReceipt>> {
        self.seal_segment_with_intent(prepare_id, None, frames)
    }

    /// CMD2's fenced one-segment profile. The intent is synced before any
    /// pin or frame bytes. The registered attempt fence is an exact binding,
    /// not a grant to commit; CMD must still recheck it under its DB lock.
    pub fn seal_segment_fenced(
        &self,
        prepare_id: &[u8],
        attempt_fence: u64,
        segment_slot: u32,
        frames: &mut [FrameInput<'_>],
    ) -> Result<Vec<ByteDurabilityReceipt>> {
        if attempt_fence == 0 {
            return Err(SegmentError::new(
                Code::InvalidReceipt,
                "invalid attempt fence",
            ));
        }
        if segment_slot != 0 {
            return Err(SegmentError::new(
                Code::UnsupportedOversized,
                "unsupported multi-segment attempt",
            ));
        }
        self.seal_segment_with_intent(prepare_id, Some((attempt_fence, segment_slot)), frames)
    }

    fn seal_segment_with_intent(
        &self,
        prepare_id: &[u8],
        fenced: Option<(u64, u32)>,
        frames: &mut [FrameInput<'_>],
    ) -> Result<Vec<ByteDurabilityReceipt>> {
        let limits = self.inner.limits;
        if prepare_id.is_empty()
            || prepare_id.len() > u16::MAX as usize
            || frames.is_empty()
            || frames.len() > limits.max_frames as usize
        {
            return Err(SegmentError::new(
                Code::BudgetExceeded,
                "invalid segment frame or prepare count",
            ));
        }
        let mut planned = format::HEADER_BYTES + format::END_BYTES;
        let mut journal_bytes = 166usize
            .checked_add(prepare_id.len())
            .ok_or_else(|| SegmentError::new(Code::BudgetExceeded, "pin journal size overflow"))?;
        let mut member_slots = HashSet::with_capacity(frames.len());
        for frame in frames.iter() {
            if frame.binding.profile_id.is_empty()
                || frame.binding.profile_version.is_empty()
                || frame.binding.subject_key.is_empty()
                || frame.binding.profile_id.len() > u16::MAX as usize
                || frame.binding.profile_version.len() > u16::MAX as usize
                || frame.binding.subject_key.len() > u32::MAX as usize
            {
                return Err(SegmentError::new(
                    Code::InvalidReceipt,
                    "invalid owner binding",
                ));
            }
            if frame.declared_size > limits.max_frame_bytes {
                return Err(SegmentError::new(
                    Code::UnsupportedOversized,
                    "frame needs unsupported large-object route",
                ));
            }
            if !member_slots.insert(frame.binding.member_slot) {
                return Err(SegmentError::new(
                    Code::InvalidReceipt,
                    "duplicate prepare member slot",
                ));
            }
            journal_bytes = journal_bytes
                .checked_add(60)
                .and_then(|size| size.checked_add(frame.binding.profile_id.len()))
                .and_then(|size| size.checked_add(frame.binding.profile_version.len()))
                .and_then(|size| size.checked_add(frame.binding.subject_key.len()))
                .ok_or_else(|| {
                    SegmentError::new(Code::BudgetExceeded, "pin journal size overflow")
                })?;
            if journal_bytes > limits.max_journal_bytes {
                return Err(SegmentError::new(
                    Code::BudgetExceeded,
                    "pin journal exceeds limit",
                ));
            }
            planned = planned
                .checked_add(format::FRAME_HEADER_BYTES)
                .and_then(|value| value.checked_add(frame.declared_size))
                .ok_or_else(|| SegmentError::new(Code::BudgetExceeded, "segment size overflow"))?;
            if planned > limits.max_segment_bytes {
                return Err(SegmentError::new(
                    Code::UnsupportedOversized,
                    "segment exceeds caller profile",
                ));
            }
        }
        let mut pin_id = [0u8; 16];
        getrandom::fill(&mut pin_id)
            .map_err(|_| SegmentError::new(Code::Io, "cannot create pin ID"))?;
        let mut journal = PinJournal {
            state: PinState::Preparing,
            pin_id,
            store_id: self.inner.store_id,
            fence_epoch: 1,
            domain_digest: self.inner.domain_digest,
            segment_digest: None,
            segment_size: 0,
            prepare_id: prepare_id.to_vec(),
            frames: Vec::new(),
        };
        let pin_name = hex_id(pin_id);
        // A crash after sealing but before CMD's attach_ready must leave a
        // prepare-keyed route to this pin. The one-segment-per-prepare first
        // profile refuses reuse; reconciliation precedes any retry.
        match fenced {
            Some((attempt_fence, segment_slot)) => {
                self.write_attempt_intent_fenced(prepare_id, attempt_fence, segment_slot, pin_id)?
            }
            None => self.write_attempt_intent(prepare_id, pin_id)?,
        }
        #[cfg(test)]
        crash_test_barrier("intent-synced", pin_id);
        write_initial_pin(&self.inner.pins, &pin_name, &journal.encode(limits)?)?;
        #[cfg(test)]
        crash_test_barrier("pin-synced", pin_id);
        let stage_name = format!("{pin_name}.part");
        let mut stage = create_exclusive(&self.inner.staging, &stage_name)?;
        let mut segment_hash = Digest256Hasher::new();
        let mut actual = 0u64;
        write_part(
            &mut stage,
            &mut segment_hash,
            &mut actual,
            &format::header(self.inner.domain_digest, frames.len() as u32),
            limits,
        )?;
        let mut coordinates = Vec::with_capacity(frames.len());
        let mut block = [0u8; BLOCK_BYTES];
        for frame in frames.iter_mut() {
            let coordinate = FrameCoordinate {
                header_offset: actual,
                size_bytes: frame.declared_size,
                sha256: frame.declared_sha256,
            };
            write_part(
                &mut stage,
                &mut segment_hash,
                &mut actual,
                &format::frame_header(frame.declared_size, frame.declared_sha256),
                limits,
            )?;
            let mut frame_hash = Digest256Hasher::new();
            let mut remaining = frame.declared_size;
            while remaining > 0 {
                let request =
                    usize::try_from(remaining.min(BLOCK_BYTES as u64)).expect("bounded block");
                let count = frame
                    .reader
                    .read(&mut block[..request])
                    .map_err(|error| SegmentError::io("cannot read proposed frame bytes", error))?;
                if count == 0 {
                    return Err(SegmentError::new(
                        Code::CorruptBytes,
                        "proposed frame truncated",
                    ));
                }
                frame_hash.update(&block[..count]);
                write_part(
                    &mut stage,
                    &mut segment_hash,
                    &mut actual,
                    &block[..count],
                    limits,
                )?;
                remaining -= count as u64;
            }
            let mut extra = [0u8; 1];
            if frame
                .reader
                .read(&mut extra)
                .map_err(|error| SegmentError::io("cannot check proposed frame length", error))?
                != 0
                || frame_hash.finalize() != frame.declared_sha256
            {
                return Err(SegmentError::new(
                    Code::CorruptBytes,
                    "proposed frame digest or length differs",
                ));
            }
            coordinates.push(coordinate);
        }
        write_part(
            &mut stage,
            &mut segment_hash,
            &mut actual,
            format::END,
            limits,
        )?;
        stage
            .sync_all()
            .map_err(|error| SegmentError::io("cannot sync staged segment", error))?;
        drop(stage);
        #[cfg(test)]
        crash_test_barrier("stage-synced", pin_id);
        let segment_digest = segment_hash.finalize();
        let segment_name = segment_digest.to_hex();
        match linkat(
            &self.inner.staging,
            stage_name.as_str(),
            &self.inner.segments,
            segment_name.as_str(),
            AtFlags::empty(),
        ) {
            Ok(()) => {}
            Err(Errno::EXIST) => {
                // A digest-named candidate must be fully verified before reuse.
                let existing = open_regular(&self.inner.segments, &segment_name)?;
                let found = format::verify_whole(
                    existing.try_clone().map_err(|error| {
                        SegmentError::io("cannot clone existing segment", error)
                    })?,
                    segment_digest,
                    actual,
                    self.inner.domain_digest,
                    limits,
                )?;
                if found != coordinates {
                    return Err(SegmentError::new(
                        Code::CorruptBytes,
                        "existing segment frames differ",
                    ));
                }
                existing
                    .sync_all()
                    .map_err(|error| SegmentError::io("cannot sync existing segment", error))?;
            }
            Err(error) => {
                return Err(SegmentError::io(
                    "cannot no-replace install segment",
                    error.into(),
                ));
            }
        }
        #[cfg(test)]
        crash_test_barrier("segment-installed", pin_id);
        fsync(&self.inner.segments)
            .map_err(|error| SegmentError::io("cannot sync segment directory", error.into()))?;
        #[cfg(test)]
        crash_test_barrier("segments-dir-synced", pin_id);
        unlinkat(&self.inner.staging, stage_name.as_str(), AtFlags::empty())
            .map_err(|error| SegmentError::io("cannot unlink staged alias", error.into()))?;
        fsync(&self.inner.staging)
            .map_err(|error| SegmentError::io("cannot sync staging directory", error.into()))?;
        #[cfg(test)]
        crash_test_barrier("staging-dir-synced", pin_id);
        let installed = open_regular(&self.inner.segments, &segment_name)?;
        let metadata = installed
            .metadata()
            .map_err(|error| SegmentError::io("cannot stat installed segment", error))?;
        let found = format::verify_whole(
            installed,
            segment_digest,
            actual,
            self.inner.domain_digest,
            limits,
        )?;
        if found != coordinates {
            return Err(SegmentError::new(
                Code::CorruptBytes,
                "installed segment frames differ",
            ));
        }
        journal.state = PinState::Sealed;
        journal.segment_digest = Some(segment_digest);
        journal.segment_size = actual;
        journal.frames = coordinates
            .into_iter()
            .zip(frames.iter())
            .map(|(coordinate, frame)| JournalFrame {
                coordinate,
                binding: frame.binding.clone(),
            })
            .collect();
        replace_pin(&self.inner.pins, &pin_name, &journal.encode(limits)?)?;
        #[cfg(test)]
        crash_test_barrier("sealed-pin-synced", pin_id);
        Ok(self.receipts_from_journal(journal, metadata.dev(), metadata.ino()))
    }

    /// Cold re-open requires both a sealed durable pin and a complete actual
    /// segment verification. A caller-supplied field packet is never trusted.
    pub fn recover_sealed(&self, pin_id: [u8; 16]) -> Result<Vec<ByteDurabilityReceipt>> {
        let _pin_lock = self.lock_pin_dir(FlockOperation::NonBlockingLockShared)?;
        let journal = self.read_pin(pin_id)?;
        if journal.state != PinState::Sealed {
            return Err(SegmentError::new(Code::InvalidReceipt, "pin is not sealed"));
        }
        let digest = journal
            .segment_digest
            .ok_or_else(|| SegmentError::new(Code::InvalidReceipt, "sealed pin has no segment"))?;
        let file = open_regular(&self.inner.segments, &digest.to_hex())?;
        let metadata = file
            .metadata()
            .map_err(|error| SegmentError::io("cannot stat recovered segment", error))?;
        let found = format::verify_whole(
            file,
            digest,
            journal.segment_size,
            self.inner.domain_digest,
            self.inner.limits,
        )?;
        if found
            != journal
                .frames
                .iter()
                .map(|frame| frame.coordinate)
                .collect::<Vec<_>>()
        {
            return Err(SegmentError::new(
                Code::InvalidReceipt,
                "pin frame map differs from segment",
            ));
        }
        Ok(self.receipts_from_journal(journal, metadata.dev(), metadata.ino()))
    }

    /// Bounded direct lookup by the CMD-registered prepare ID. The durable
    /// intent is written before the pin and segment, so even an interrupted
    /// seal remains discoverable without a marker or directory-wide scan.
    /// A missing pin after an intent is uncertain custody, not abort proof.
    pub fn recover_attempt(&self, prepare_id: &[u8]) -> Result<Option<AttemptRecovery>> {
        let _pin_lock = self.lock_pin_dir(FlockOperation::NonBlockingLockShared)?;
        let Some(pin_id) = self.read_attempt_intent(prepare_id)? else {
            return Ok(None);
        };
        self.recover_attempt_pin(prepare_id, pin_id).map(Some)
    }

    /// Resolve only the exact registered CMD attempt generation and segment
    /// slot. A v1 intent or mismatched fence is an error, never an absent pin.
    pub fn recover_attempt_fenced(
        &self,
        prepare_id: &[u8],
        attempt_fence: u64,
        segment_slot: u32,
    ) -> Result<Option<AttemptRecovery>> {
        let _pin_lock = self.lock_pin_dir(FlockOperation::NonBlockingLockShared)?;
        let Some(pin_id) =
            self.read_attempt_intent_fenced(prepare_id, attempt_fence, segment_slot)?
        else {
            return Ok(None);
        };
        self.recover_attempt_pin(prepare_id, pin_id).map(Some)
    }

    fn recover_attempt_pin(&self, prepare_id: &[u8], pin_id: [u8; 16]) -> Result<AttemptRecovery> {
        let journal = match self.read_pin(pin_id) {
            Ok(journal) => journal,
            Err(error) if is_missing(&error) => {
                return Ok(AttemptRecovery::IntentOnly { pin_id });
            }
            Err(error) => return Err(error),
        };
        if journal.prepare_id != prepare_id {
            return Err(SegmentError::new(
                Code::InvalidReceipt,
                "intent pin prepare differs",
            ));
        }
        Ok(match journal.state {
            PinState::Preparing => AttemptRecovery::Preparing {
                pin_id,
                fence_epoch: journal.fence_epoch,
            },
            PinState::Sealed => AttemptRecovery::Sealed {
                receipts: self.recover_sealed(pin_id)?,
            },
            PinState::Aborted => AttemptRecovery::Aborted {
                pin_id,
                fence_epoch: journal.fence_epoch,
            },
        })
    }

    fn write_attempt_intent(&self, prepare_id: &[u8], pin_id: [u8; 16]) -> Result<()> {
        let raw = encode_attempt_intent(
            self.inner.store_id,
            self.inner.domain_digest,
            prepare_id,
            pin_id,
        )?;
        self.write_intent_raw(prepare_id, &raw)
    }

    fn write_attempt_intent_fenced(
        &self,
        prepare_id: &[u8],
        attempt_fence: u64,
        segment_slot: u32,
        pin_id: [u8; 16],
    ) -> Result<()> {
        let raw = encode_attempt_intent_fenced(
            self.inner.store_id,
            self.inner.domain_digest,
            prepare_id,
            attempt_fence,
            segment_slot,
            pin_id,
        )?;
        self.write_intent_raw(prepare_id, &raw)
    }

    fn write_intent_raw(&self, prepare_id: &[u8], raw: &[u8]) -> Result<()> {
        let name = attempt_name(prepare_id);
        let attempts = self.inner.attempts.as_ref().ok_or_else(|| {
            SegmentError::new(Code::InvalidRoot, "store lacks durable attempt intents")
        })?;
        let mut file = match create_exclusive(attempts, &name) {
            Ok(file) => file,
            Err(error)
                if error
                    .source
                    .as_ref()
                    .is_some_and(|source| source.kind() == std::io::ErrorKind::AlreadyExists) =>
            {
                return Err(SegmentError::new(
                    Code::PinConflict,
                    "prepare already has durable pin intent",
                ));
            }
            Err(error) => return Err(error),
        };
        file.write_all(&raw)
            .map_err(|error| SegmentError::io("cannot write attempt intent", error))?;
        file.sync_all()
            .map_err(|error| SegmentError::io("cannot sync attempt intent", error))?;
        fsync(attempts)
            .map_err(|error| SegmentError::io("cannot sync attempt directory", error.into()))
    }

    fn read_attempt_intent(&self, prepare_id: &[u8]) -> Result<Option<[u8; 16]>> {
        let Some(raw) = self.read_intent_raw(prepare_id)? else {
            return Ok(None);
        };
        decode_attempt_intent(
            &raw,
            self.inner.store_id,
            self.inner.domain_digest,
            prepare_id,
        )
        .map(Some)
    }

    fn read_attempt_intent_fenced(
        &self,
        prepare_id: &[u8],
        attempt_fence: u64,
        segment_slot: u32,
    ) -> Result<Option<[u8; 16]>> {
        if attempt_fence == 0 || segment_slot != 0 {
            return Err(SegmentError::new(
                Code::InvalidReceipt,
                "invalid fenced attempt or unsupported segment slot",
            ));
        }
        let Some(raw) = self.read_intent_raw(prepare_id)? else {
            return Ok(None);
        };
        decode_attempt_intent_fenced(
            &raw,
            self.inner.store_id,
            self.inner.domain_digest,
            prepare_id,
            attempt_fence,
            segment_slot,
        )
        .map(Some)
    }

    fn read_intent_raw(&self, prepare_id: &[u8]) -> Result<Option<Vec<u8>>> {
        if prepare_id.is_empty() || prepare_id.len() > u16::MAX as usize {
            return Err(SegmentError::new(
                Code::InvalidReceipt,
                "invalid prepare ID",
            ));
        }
        let attempts = self.inner.attempts.as_ref().ok_or_else(|| {
            SegmentError::new(Code::InvalidRoot, "store lacks durable attempt intents")
        })?;
        let file = match open_regular(attempts, &attempt_name(prepare_id)) {
            Ok(file) => file,
            Err(error) if is_missing(&error) => return Ok(None),
            Err(error) => return Err(error),
        };
        let mut raw = Vec::new();
        let read_cap = u64::try_from(self.inner.limits.max_journal_bytes)
            .ok()
            .and_then(|value| value.checked_add(141))
            .ok_or_else(|| SegmentError::new(Code::BudgetExceeded, "intent read cap overflow"))?;
        file.take(read_cap)
            .read_to_end(&mut raw)
            .map_err(|error| SegmentError::io("cannot read attempt intent", error))?;
        Ok(Some(raw))
    }

    /// Admission of a stored placement into a warm generation. This cold path
    /// hashes the whole segment once; subsequent selected reads use the
    /// returned private handle and verify only the selected frame.
    pub fn recover_placement(&self, placement: &PlacementV1) -> Result<ByteDurabilityReceipt> {
        if placement.store_id != self.inner.store_id
            || placement.domain_digest != self.inner.domain_digest
        {
            return Err(SegmentError::new(
                Code::InvalidReceipt,
                "placement store/domain differs",
            ));
        }
        let receipts = self.recover_sealed(placement.pin_id)?;
        let receipt = receipts
            .get(placement.frame_index as usize)
            .ok_or_else(|| {
                SegmentError::new(Code::InvalidReceipt, "placement frame index absent")
            })?;
        if receipt.placement() != *placement {
            return Err(SegmentError::new(
                Code::InvalidReceipt,
                "placement differs from sealed pin",
            ));
        }
        Ok(receipt.clone())
    }

    /// Cold-admit a bounded set of physical placements. Every distinct pin
    /// incurs one whole-segment verification, including when many logical
    /// records share a segment; returned receipts preserve caller order.
    /// This is an offline/generation admission path, never a warm query step.
    pub fn recover_placements(
        &self,
        placements: &[PlacementV1],
        budget: VerificationBudget,
    ) -> Result<Vec<ByteDurabilityReceipt>> {
        if placements.is_empty()
            || budget.max_receipts == 0
            || budget.max_segments == 0
            || budget.max_total_segment_bytes == 0
            || budget.max_total_segment_bytes == u64::MAX
            || placements.len() > budget.max_receipts
        {
            return Err(SegmentError::new(
                Code::BudgetExceeded,
                "invalid cold placement budget",
            ));
        }
        let _pin_lock = self.lock_pin_dir(FlockOperation::NonBlockingLockShared)?;
        let mut by_pin: HashMap<[u8; 16], Vec<ByteDurabilityReceipt>> = HashMap::new();
        let mut seen = HashSet::with_capacity(placements.len());
        let mut total_bytes = 0u64;
        let mut result = Vec::with_capacity(placements.len());
        for placement in placements {
            if placement.store_id != self.inner.store_id
                || placement.domain_digest != self.inner.domain_digest
                || !seen.insert(placement.receipt_id)
            {
                return Err(SegmentError::new(
                    Code::InvalidReceipt,
                    "placement store/domain or duplicate receipt differs",
                ));
            }
            if !by_pin.contains_key(&placement.pin_id) {
                if by_pin.len() >= budget.max_segments {
                    return Err(SegmentError::new(
                        Code::BudgetExceeded,
                        "cold placement segment count exceeds budget",
                    ));
                }
                total_bytes = total_bytes
                    .checked_add(placement.segment_size)
                    .ok_or_else(|| {
                        SegmentError::new(
                            Code::BudgetExceeded,
                            "cold placement byte count overflow",
                        )
                    })?;
                if total_bytes > budget.max_total_segment_bytes {
                    return Err(SegmentError::new(
                        Code::BudgetExceeded,
                        "cold placement byte budget exceeded",
                    ));
                }
                by_pin.insert(placement.pin_id, self.recover_sealed(placement.pin_id)?);
            }
            let receipt = by_pin
                .get(&placement.pin_id)
                .and_then(|receipts| receipts.get(placement.frame_index as usize))
                .ok_or_else(|| {
                    SegmentError::new(Code::InvalidReceipt, "placement frame index absent")
                })?;
            if receipt.placement() != *placement {
                return Err(SegmentError::new(
                    Code::InvalidReceipt,
                    "placement differs from sealed pin",
                ));
            }
            result.push(receipt.clone());
        }
        Ok(result)
    }

    /// CMD's same-process durability gate. Only a crate-constructed handle
    /// can enter, and the pinned on-disk bytes are re-read in full at use.
    /// CMD retains the pin until its transaction commits or explicitly aborts.
    pub fn verify_receipt(&self, receipt: &ByteDurabilityReceipt) -> Result<()> {
        let _pin_lock = self.lock_pin_dir(FlockOperation::NonBlockingLockShared)?;
        let journal = self.validated_receipt_journal(receipt)?;
        self.verify_actual_segment(receipt, &journal)
    }

    /// Reverify all exact bytes before CMD opens its short metadata transaction,
    /// retaining a shared pin lock until CMD has resolved that transaction.
    /// One segment is hashed at most once even if it contains many members.
    pub fn verify_and_hold(
        &self,
        receipts: &[ByteDurabilityReceipt],
        budget: VerificationBudget,
    ) -> Result<VerifiedSealGuard> {
        if receipts.is_empty()
            || budget.max_receipts == 0
            || budget.max_segments == 0
            || budget.max_total_segment_bytes == 0
            || budget.max_total_segment_bytes == u64::MAX
            || receipts.len() > budget.max_receipts
        {
            return Err(SegmentError::new(
                Code::BudgetExceeded,
                "invalid receipt verification budget",
            ));
        }
        let pin_lock = self.lock_pin_dir(FlockOperation::NonBlockingLockShared)?;
        let prepare_id = receipts[0].prepare_id.clone();
        let mut seen_receipts = HashSet::with_capacity(receipts.len());
        // This operation retains the shared pin lock: a decoded journal stays
        // valid until its guard resolves. Do not reread all frames per member.
        let mut journals = HashMap::<[u8; 16], PinJournal>::new();
        let mut total_bytes = 0u64;
        for receipt in receipts {
            if receipt.prepare_id != prepare_id || !seen_receipts.insert(receipt.receipt_id) {
                return Err(SegmentError::new(
                    Code::InvalidReceipt,
                    "mixed prepare or duplicate receipt",
                ));
            }
            if !journals.contains_key(&receipt.pin_id) {
                if journals.len() >= budget.max_segments {
                    return Err(SegmentError::new(
                        Code::BudgetExceeded,
                        "too many segments in verification",
                    ));
                }
                let journal = self.validated_receipt_journal(receipt)?;
                total_bytes = total_bytes
                    .checked_add(receipt.segment_size)
                    .ok_or_else(|| {
                        SegmentError::new(Code::BudgetExceeded, "verification byte count overflow")
                    })?;
                if total_bytes > budget.max_total_segment_bytes {
                    return Err(SegmentError::new(
                        Code::BudgetExceeded,
                        "verification byte budget exceeded",
                    ));
                }
                self.verify_actual_segment(receipt, &journal)?;
                journals.insert(receipt.pin_id, journal);
            } else {
                self.validate_receipt_binding(receipt, &journals[&receipt.pin_id])?;
            }
        }
        Ok(VerifiedSealGuard {
            _pin_lock: pin_lock,
            receipts: receipts.to_vec(),
            prepare_id,
        })
    }

    /// CMD2's exact pre-transaction gate for the registered attempt fence.
    /// The outer lock closes the intent-check to guard-acquisition gap;
    /// returned guard continues holding its own fresh shared file description.
    pub fn verify_and_hold_fenced(
        &self,
        prepare_id: &[u8],
        attempt_fence: u64,
        segment_slot: u32,
        receipts: &[ByteDurabilityReceipt],
        budget: VerificationBudget,
    ) -> Result<VerifiedSealGuard> {
        let _pin_lock = self.lock_pin_dir(FlockOperation::NonBlockingLockShared)?;
        let pin_id = self
            .read_attempt_intent_fenced(prepare_id, attempt_fence, segment_slot)?
            .ok_or_else(|| SegmentError::new(Code::InvalidReceipt, "fenced intent absent"))?;
        if receipts.is_empty()
            || receipts
                .iter()
                .any(|receipt| receipt.pin_id != pin_id || receipt.prepare_id != prepare_id)
        {
            return Err(SegmentError::new(
                Code::InvalidReceipt,
                "fenced receipt pin or prepare differs",
            ));
        }
        self.verify_and_hold(receipts, budget)
    }

    fn verify_actual_segment(
        &self,
        receipt: &ByteDurabilityReceipt,
        journal: &PinJournal,
    ) -> Result<()> {
        let file = open_regular(&self.inner.segments, &receipt.segment_digest.to_hex())?;
        let metadata = file
            .metadata()
            .map_err(|error| SegmentError::io("cannot stat receipt segment", error))?;
        if metadata.dev() != receipt.segment_dev || metadata.ino() != receipt.segment_ino {
            return Err(SegmentError::new(
                Code::CorruptBytes,
                "receipt segment identity differs",
            ));
        }
        let found = format::verify_whole(
            file,
            receipt.segment_digest,
            receipt.segment_size,
            self.inner.domain_digest,
            self.inner.limits,
        )?;
        if found
            != journal
                .frames
                .iter()
                .map(|frame| frame.coordinate)
                .collect::<Vec<_>>()
        {
            return Err(SegmentError::new(
                Code::InvalidReceipt,
                "receipt frame map differs",
            ));
        }
        Ok(())
    }

    /// Explicit failed-prepare fence. This retains bytes and journal for
    /// forensic recovery; no garbage collection is implemented here.
    /// Coordinator ownership must serialize abort against metadata commit.
    pub fn abort_uncommitted(
        &self,
        pin_id: [u8; 16],
        prepare_id: &[u8],
        fence_epoch: u64,
    ) -> Result<u64> {
        let _pin_lock = self.lock_pin_dir(FlockOperation::NonBlockingLockExclusive)?;
        // An intent-capable store must resolve the prepare through its v1
        // intent. In particular, a TOSINT2 pin must not be cancelled through
        // this entry point without checking its CMD attempt fence.
        if self.inner.attempts.is_some() {
            let expected_pin = self
                .read_attempt_intent(prepare_id)?
                .ok_or_else(|| SegmentError::new(Code::InvalidReceipt, "attempt intent absent"))?;
            if expected_pin != pin_id {
                return Err(SegmentError::new(
                    Code::InvalidReceipt,
                    "attempt intent pin differs",
                ));
            }
        }
        self.abort_pin_locked(pin_id, prepare_id, fence_epoch)
    }

    /// CMD calls this only after an authoritative DB abort has committed and
    /// all DB locks were released. The argument is the *pre-abort* attempt
    /// fence captured under CMD's attempt-row lock, not its incremented value.
    pub fn abort_uncommitted_fenced(
        &self,
        pin_id: [u8; 16],
        prepare_id: &[u8],
        attempt_fence: u64,
        segment_slot: u32,
        pin_fence_epoch: u64,
    ) -> Result<u64> {
        let _pin_lock = self.lock_pin_dir(FlockOperation::NonBlockingLockExclusive)?;
        let expected_pin = self
            .read_attempt_intent_fenced(prepare_id, attempt_fence, segment_slot)?
            .ok_or_else(|| SegmentError::new(Code::InvalidReceipt, "fenced intent absent"))?;
        if expected_pin != pin_id {
            return Err(SegmentError::new(
                Code::InvalidReceipt,
                "fenced intent pin differs",
            ));
        }
        self.abort_pin_locked(pin_id, prepare_id, pin_fence_epoch)
    }

    fn abort_pin_locked(
        &self,
        pin_id: [u8; 16],
        prepare_id: &[u8],
        fence_epoch: u64,
    ) -> Result<u64> {
        let mut journal = self.read_pin(pin_id)?;
        if journal.state != PinState::Sealed
            || journal.prepare_id != prepare_id
            || journal.fence_epoch != fence_epoch
        {
            return Err(SegmentError::new(
                Code::PinConflict,
                "abort pin fence or owner differs",
            ));
        }
        journal.fence_epoch = journal
            .fence_epoch
            .checked_add(1)
            .ok_or_else(|| SegmentError::new(Code::PinConflict, "pin fence exhausted"))?;
        journal.state = PinState::Aborted;
        replace_pin(
            &self.inner.pins,
            &hex_id(pin_id),
            &journal.encode(self.inner.limits)?,
        )?;
        Ok(journal.fence_epoch)
    }

    /// A selected frame is checked before writing any byte to the caller sink.
    /// The owner adapter must still check current rights before disclosure.
    pub fn read_selected(
        &self,
        receipt: &ByteDurabilityReceipt,
        max_bytes: u64,
        sink: &mut impl Write,
    ) -> Result<u64> {
        let _pin_lock = self.lock_pin_dir(FlockOperation::NonBlockingLockShared)?;
        let journal = self.validated_receipt_journal(receipt)?;
        let file = open_regular(&self.inner.segments, &receipt.segment_digest.to_hex())?;
        let metadata = file
            .metadata()
            .map_err(|error| SegmentError::io("cannot stat selected segment", error))?;
        if metadata.len() != receipt.segment_size
            || metadata.dev() != receipt.segment_dev
            || metadata.ino() != receipt.segment_ino
        {
            return Err(SegmentError::new(
                Code::CorruptBytes,
                "selected segment identity or size differs",
            ));
        }
        format::read_selected(
            file,
            receipt.segment_size,
            receipt.coordinate,
            self.inner.domain_digest,
            journal.frames.len() as u32,
            receipt.frame_index,
            max_bytes.min(self.inner.limits.max_frame_bytes),
            sink,
        )
    }

    fn read_pin(&self, pin_id: [u8; 16]) -> Result<PinJournal> {
        let mut raw = Vec::new();
        open_regular(&self.inner.pins, &hex_id(pin_id))?
            .take(self.inner.limits.max_journal_bytes as u64 + 1)
            .read_to_end(&mut raw)
            .map_err(|error| SegmentError::io("cannot read pin journal", error))?;
        let journal = PinJournal::decode(&raw, self.inner.limits)?;
        if journal.pin_id != pin_id
            || journal.store_id != self.inner.store_id
            || journal.domain_digest != self.inner.domain_digest
        {
            return Err(SegmentError::new(
                Code::InvalidReceipt,
                "pin store/domain identity differs",
            ));
        }
        Ok(journal)
    }

    fn lock_pin_dir(&self, operation: FlockOperation) -> Result<PinDirectoryLease> {
        // A fresh open description is essential: dup/try_clone share flock
        // ownership on Linux and would not exclude a same-process abort.
        let file = tos_fd_open::reopen_directory(&self.inner.pins).map_err(|error| {
            map_fd_open(error, Code::UnsafePath, "cannot reopen pinned directory")
        })?;
        flock(&file, operation).map_err(|error| {
            if error == Errno::AGAIN {
                SegmentError::new(Code::PinConflict, "pin lock held by another operation")
            } else {
                SegmentError::io("cannot lock pin directory", error.into())
            }
        })?;
        Ok(PinDirectoryLease { file })
    }

    fn validated_receipt_journal(&self, receipt: &ByteDurabilityReceipt) -> Result<PinJournal> {
        if !Arc::ptr_eq(&self.inner, &receipt.inner) {
            return Err(SegmentError::new(
                Code::InvalidReceipt,
                "receipt belongs to another store instance",
            ));
        }
        let journal = self.read_pin(receipt.pin_id)?;
        self.validate_receipt_binding(receipt, &journal)?;
        Ok(journal)
    }

    fn validate_receipt_binding(
        &self,
        receipt: &ByteDurabilityReceipt,
        journal: &PinJournal,
    ) -> Result<()> {
        if !Arc::ptr_eq(&self.inner, &receipt.inner) {
            return Err(SegmentError::new(
                Code::InvalidReceipt,
                "receipt belongs to another store instance",
            ));
        }
        if journal.pin_id != receipt.pin_id
            || journal.state != PinState::Sealed
            || journal.fence_epoch != receipt.fence_epoch
            || journal.segment_digest != Some(receipt.segment_digest)
            || journal.segment_size != receipt.segment_size
            || journal.prepare_id != receipt.prepare_id
            || journal
                .frames
                .get(receipt.frame_index as usize)
                .is_none_or(|frame| {
                    frame.coordinate != receipt.coordinate || frame.binding != receipt.binding
                })
            || self.receipt_id(&journal, receipt.frame_index) != receipt.receipt_id
        {
            return Err(SegmentError::new(
                Code::InvalidReceipt,
                "receipt differs from sealed pin",
            ));
        }
        Ok(())
    }

    fn receipt_id(&self, journal: &PinJournal, frame_index: u32) -> Digest256 {
        let mut bytes = Vec::with_capacity(16 + 16 + 8 + 32 + 4);
        bytes.extend_from_slice(&self.inner.store_id);
        bytes.extend_from_slice(&journal.pin_id);
        bytes.extend_from_slice(&journal.fence_epoch.to_le_bytes());
        bytes.extend_from_slice(
            journal
                .segment_digest
                .expect("sealed journal has segment")
                .as_bytes(),
        );
        bytes.extend_from_slice(&frame_index.to_le_bytes());
        Digest256::of_bytes(&bytes)
    }

    fn receipts_from_journal(
        &self,
        journal: PinJournal,
        dev: u64,
        ino: u64,
    ) -> Vec<ByteDurabilityReceipt> {
        let digest = journal.segment_digest.expect("sealed journal has segment");
        journal
            .frames
            .iter()
            .enumerate()
            .map(|(index, frame)| ByteDurabilityReceipt {
                inner: self.inner.clone(),
                pin_id: journal.pin_id,
                fence_epoch: journal.fence_epoch,
                prepare_id: journal.prepare_id.clone(),
                binding: frame.binding.clone(),
                segment_digest: digest,
                segment_size: journal.segment_size,
                coordinate: frame.coordinate,
                segment_dev: dev,
                segment_ino: ino,
                frame_index: index as u32,
                receipt_id: self.receipt_id(&journal, index as u32),
            })
            .collect()
    }
}

fn write_part(
    file: &mut File,
    hasher: &mut Digest256Hasher,
    actual: &mut u64,
    bytes: &[u8],
    limits: SegmentLimits,
) -> Result<()> {
    *actual = actual
        .checked_add(bytes.len() as u64)
        .ok_or_else(|| SegmentError::new(Code::BudgetExceeded, "segment size overflow"))?;
    if *actual > limits.max_segment_bytes {
        return Err(SegmentError::new(
            Code::BudgetExceeded,
            "segment actual bytes exceed limit",
        ));
    }
    file.write_all(bytes)
        .map_err(|error| SegmentError::io("cannot write staged segment", error))?;
    hasher.update(bytes);
    Ok(())
}

fn open_root(path: &Path) -> Result<File> {
    if path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::RootDir))
    {
        return Err(SegmentError::new(
            Code::InvalidRoot,
            "filesystem root is not a segment store",
        ));
    }
    tos_fd_open::open_absolute_directory(path).map_err(|error| {
        map_fd_open(
            error,
            Code::InvalidRoot,
            "cannot securely open segment root",
        )
    })
}

fn held_root(root: &File) -> Result<File> {
    let metadata = root
        .metadata()
        .map_err(|error| SegmentError::io("cannot inspect held segment root", error))?;
    if !metadata.is_dir() {
        return Err(SegmentError::new(
            Code::InvalidRoot,
            "held segment root is not a directory",
        ));
    }
    root.try_clone()
        .map_err(|error| SegmentError::io("cannot retain held segment root", error))
}

fn open_directory(parent: &File, name: &str) -> Result<File> {
    tos_fd_open::open_directory_at(parent, Path::new(name)).map_err(|error| {
        map_fd_open(
            error,
            Code::UnsafePath,
            "cannot securely open segment directory",
        )
    })
}

fn open_regular(parent: &File, name: &str) -> Result<File> {
    tos_fd_open::open_regular_at(parent, Path::new(name))
        .map_err(|error| map_fd_open(error, Code::UnsafePath, "cannot securely open segment file"))
}

fn require_private_generation_file(metadata: &std::fs::Metadata) -> Result<()> {
    if !metadata.is_file()
        || metadata.nlink() != 1
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.mode() & 0o7777 != 0o600
    {
        return Err(SegmentError::new(
            Code::UnsafePath,
            "authenticated pack is not a private regular file",
        ));
    }
    Ok(())
}

fn require_same_generation_file(
    before: &std::fs::Metadata,
    after: &std::fs::Metadata,
) -> Result<()> {
    require_private_generation_file(after)?;
    if before.dev() != after.dev()
        || before.ino() != after.ino()
        || before.len() != after.len()
        || before.uid() != after.uid()
        || before.mode() != after.mode()
        || before.nlink() != after.nlink()
    {
        return Err(SegmentError::new(
            Code::CorruptBytes,
            "authenticated pack custody changed during frame read",
        ));
    }
    Ok(())
}

fn read_bounded_regular(
    mut file: File,
    max_bytes: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Vec<u8>> {
    let declared = file
        .metadata()
        .map_err(|error| SegmentError::io("cannot stat authenticated object", error))?
        .len();
    if declared > max_bytes as u64 {
        return Err(SegmentError::new(
            Code::BudgetExceeded,
            "authenticated object exceeds read limit",
        ));
    }
    let capacity = usize::try_from(declared).map_err(|_| {
        SegmentError::new(
            Code::BudgetExceeded,
            "authenticated object exceeds address space",
        )
    })?;
    let mut raw = Vec::new();
    raw.try_reserve_exact(capacity).map_err(|_| {
        SegmentError::new(
            Code::BudgetExceeded,
            "authenticated object allocation failed",
        )
    })?;
    let mut block = [0u8; 64 * 1024];
    loop {
        check_generation(deadline, cancelled)?;
        let count = file
            .read(&mut block)
            .map_err(|error| SegmentError::io("cannot read authenticated object", error))?;
        if count == 0 {
            break;
        }
        if raw
            .len()
            .checked_add(count)
            .is_none_or(|length| length > max_bytes)
        {
            return Err(SegmentError::new(
                Code::BudgetExceeded,
                "authenticated object grew beyond read limit",
            ));
        }
        raw.extend_from_slice(&block[..count]);
    }
    if raw.len() as u64 != declared {
        return Err(SegmentError::new(
            Code::CorruptBytes,
            "authenticated object length changed during read",
        ));
    }
    Ok(raw)
}

fn create_exclusive(parent: &File, name: &str) -> Result<File> {
    let flags = OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW;
    openat(parent, name, flags, Mode::RUSR | Mode::WUSR)
        .map(File::from)
        .map_err(|error| SegmentError::io("cannot create exclusive segment file", error.into()))
}

fn write_initial_pin(parent: &File, name: &str, raw: &[u8]) -> Result<()> {
    let mut file = create_exclusive(parent, name)?;
    file.write_all(raw)
        .map_err(|error| SegmentError::io("cannot write initial pin", error))?;
    file.sync_all()
        .map_err(|error| SegmentError::io("cannot sync initial pin", error))?;
    fsync(parent).map_err(|error| SegmentError::io("cannot sync pin directory", error.into()))
}

fn replace_pin(parent: &File, name: &str, raw: &[u8]) -> Result<()> {
    let mut temp_id = [0u8; 16];
    getrandom::fill(&mut temp_id)
        .map_err(|_| SegmentError::new(Code::Io, "cannot create pin transition ID"))?;
    let next = format!("{name}.next-{}", hex_id(temp_id));
    let mut file = create_exclusive(parent, &next)?;
    file.write_all(raw)
        .map_err(|error| SegmentError::io("cannot write sealed pin", error))?;
    file.sync_all()
        .map_err(|error| SegmentError::io("cannot sync sealed pin", error))?;
    drop(file);
    renameat(parent, next.as_str(), parent, name)
        .map_err(|error| SegmentError::io("cannot atomically install sealed pin", error.into()))?;
    fsync(parent)
        .map_err(|error| SegmentError::io("cannot sync sealed pin directory", error.into()))
}

fn map_fd_open(error: OpenError, unsafe_code: Code, detail: &'static str) -> SegmentError {
    match error.code {
        OpenErrorCode::InvalidPath | OpenErrorCode::UnsafePath => {
            SegmentError::new(unsafe_code, detail)
        }
        OpenErrorCode::UnsupportedPlatform => {
            SegmentError::new(Code::UnsupportedPlatform, "Linux openat2 unavailable")
        }
        OpenErrorCode::BudgetExceeded => SegmentError::new(Code::BudgetExceeded, detail),
        OpenErrorCode::Io => match error.source {
            Some(source) => SegmentError::io(detail, source),
            None => SegmentError::new(Code::Io, detail),
        },
    }
}

fn hex_id(id: [u8; 16]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(32);
    for byte in id {
        text.push(HEX[(byte >> 4) as usize] as char);
        text.push(HEX[(byte & 15) as usize] as char);
    }
    text
}

const INTENT_MAGIC: &[u8; 8] = b"TOSINT1\0";
const FENCED_INTENT_MAGIC: &[u8; 8] = b"TOSINT2\0";

fn attempt_name(prepare_id: &[u8]) -> String {
    Digest256::of_bytes(prepare_id).to_hex()
}

fn encode_attempt_intent(
    store_id: [u8; 16],
    domain_digest: Digest256,
    prepare_id: &[u8],
    pin_id: [u8; 16],
) -> Result<Vec<u8>> {
    if prepare_id.is_empty() || prepare_id.len() > u16::MAX as usize {
        return Err(SegmentError::new(
            Code::InvalidReceipt,
            "invalid prepare ID",
        ));
    }
    let mut raw = Vec::with_capacity(8 + 16 + 32 + 16 + 2 + prepare_id.len() + 32);
    raw.extend_from_slice(INTENT_MAGIC);
    raw.extend_from_slice(&store_id);
    raw.extend_from_slice(domain_digest.as_bytes());
    raw.extend_from_slice(&pin_id);
    raw.extend_from_slice(&(prepare_id.len() as u16).to_le_bytes());
    raw.extend_from_slice(prepare_id);
    let checksum = Digest256::of_bytes(&raw);
    raw.extend_from_slice(checksum.as_bytes());
    Ok(raw)
}

fn decode_attempt_intent(
    raw: &[u8],
    store_id: [u8; 16],
    domain_digest: Digest256,
    prepare_id: &[u8],
) -> Result<[u8; 16]> {
    let expected_len = 8 + 16 + 32 + 16 + 2 + prepare_id.len() + 32;
    if raw.len() != expected_len
        || &raw[..8] != INTENT_MAGIC
        || &raw[8..24] != store_id.as_slice()
        || &raw[24..56] != domain_digest.as_bytes()
        || u16::from_le_bytes([raw[72], raw[73]]) as usize != prepare_id.len()
        || &raw[74..74 + prepare_id.len()] != prepare_id
        || Digest256::of_bytes(&raw[..raw.len() - 32]).as_bytes() != &raw[raw.len() - 32..]
    {
        return Err(SegmentError::new(
            Code::InvalidReceipt,
            "attempt intent differs",
        ));
    }
    Ok(raw[56..72].try_into().expect("fixed pin ID"))
}

fn encode_attempt_intent_fenced(
    store_id: [u8; 16],
    domain_digest: Digest256,
    prepare_id: &[u8],
    attempt_fence: u64,
    segment_slot: u32,
    pin_id: [u8; 16],
) -> Result<Vec<u8>> {
    if prepare_id.is_empty()
        || prepare_id.len() > u16::MAX as usize
        || attempt_fence == 0
        || segment_slot != 0
    {
        return Err(SegmentError::new(
            Code::InvalidReceipt,
            "invalid fenced attempt identity",
        ));
    }
    let mut raw = Vec::with_capacity(8 + 16 + 32 + 16 + 8 + 4 + 2 + prepare_id.len() + 32);
    raw.extend_from_slice(FENCED_INTENT_MAGIC);
    raw.extend_from_slice(&store_id);
    raw.extend_from_slice(domain_digest.as_bytes());
    raw.extend_from_slice(&pin_id);
    raw.extend_from_slice(&attempt_fence.to_le_bytes());
    raw.extend_from_slice(&segment_slot.to_le_bytes());
    raw.extend_from_slice(&(prepare_id.len() as u16).to_le_bytes());
    raw.extend_from_slice(prepare_id);
    let checksum = Digest256::of_bytes(&raw);
    raw.extend_from_slice(checksum.as_bytes());
    Ok(raw)
}

fn decode_attempt_intent_fenced(
    raw: &[u8],
    store_id: [u8; 16],
    domain_digest: Digest256,
    prepare_id: &[u8],
    attempt_fence: u64,
    segment_slot: u32,
) -> Result<[u8; 16]> {
    if prepare_id.is_empty()
        || prepare_id.len() > u16::MAX as usize
        || attempt_fence == 0
        || segment_slot != 0
    {
        return Err(SegmentError::new(
            Code::InvalidReceipt,
            "invalid fenced attempt identity",
        ));
    }
    let expected_len = 8 + 16 + 32 + 16 + 8 + 4 + 2 + prepare_id.len() + 32;
    if raw.len() != expected_len
        || &raw[..8] != FENCED_INTENT_MAGIC
        || &raw[8..24] != store_id.as_slice()
        || &raw[24..56] != domain_digest.as_bytes()
        || u64::from_le_bytes(raw[72..80].try_into().expect("fixed fence")) != attempt_fence
        || u32::from_le_bytes(raw[80..84].try_into().expect("fixed slot")) != segment_slot
        || u16::from_le_bytes([raw[84], raw[85]]) as usize != prepare_id.len()
        || &raw[86..86 + prepare_id.len()] != prepare_id
        || Digest256::of_bytes(&raw[..raw.len() - 32]).as_bytes() != &raw[raw.len() - 32..]
    {
        return Err(SegmentError::new(
            Code::InvalidReceipt,
            "fenced attempt intent differs",
        ));
    }
    Ok(raw[56..72].try_into().expect("fixed pin ID"))
}

fn is_missing(error: &SegmentError) -> bool {
    error
        .source
        .as_ref()
        .is_some_and(|source| source.kind() == std::io::ErrorKind::NotFound)
}

#[cfg(test)]
fn crash_test_barrier(phase: &str, _pin_id: [u8; 16]) {
    if std::env::var("TOS_SEGMENT_CRASH_AT").ok().as_deref() != Some(phase) {
        return;
    }
    let _ = std::process::Command::new("/usr/bin/kill")
        .arg("-9")
        .arg(std::process::id().to_string())
        .status()
        .expect("send SIGKILL");
    panic!("SIGKILL did not terminate crash child");
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs::{self, OpenOptions};
    use std::io::{Cursor, Seek, SeekFrom, Write};
    use std::os::unix::fs::DirBuilderExt;
    use std::os::unix::fs::symlink;
    use std::os::unix::process::ExitStatusExt;
    use std::path::PathBuf;
    use std::process::Command;
    use std::time::{Duration, Instant};

    use super::*;

    struct PrivateRoot(PathBuf);

    impl PrivateRoot {
        fn new() -> Self {
            let mut id = [0u8; 16];
            getrandom::fill(&mut id).expect("test random ID");
            let path = std::env::temp_dir().join(format!("tos-segment-test-{}", hex_id(id)));
            fs::DirBuilder::new()
                .mode(0o700)
                .create(&path)
                .expect("private test root");
            Self(path)
        }
    }

    impl Drop for PrivateRoot {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove private test root");
        }
    }

    fn limits() -> SegmentLimits {
        SegmentLimits {
            max_segment_bytes: 1024 * 1024,
            max_frame_bytes: 256 * 1024,
            max_frames: 16,
            max_journal_bytes: 64 * 1024,
        }
    }

    fn binding(slot: u32) -> OwnerBinding {
        OwnerBinding {
            profile_id: b"source-item".to_vec(),
            profile_version: b"v1".to_vec(),
            subject_key: format!("opaque-{slot}").into_bytes(),
            member_slot: slot,
        }
    }

    #[test]
    fn packed_leaf_is_immutable_and_cold_recovers_selected_bytes() {
        let root = PrivateRoot::new();
        let store = SegmentStore::initialize_empty(&root.0, b"private-domain", limits()).unwrap();
        let first = b"first exact bytes".to_vec();
        let second = b"second exact bytes".to_vec();
        let mut first_reader = Cursor::new(first.clone());
        let mut second_reader = Cursor::new(second.clone());
        let mut frames = [
            FrameInput {
                binding: binding(0),
                declared_size: first.len() as u64,
                declared_sha256: Digest256::of_bytes(&first),
                reader: &mut first_reader,
            },
            FrameInput {
                binding: binding(1),
                declared_size: second.len() as u64,
                declared_sha256: Digest256::of_bytes(&second),
                reader: &mut second_reader,
            },
        ];
        let receipts = store.seal_segment(b"leaf-prepare", &mut frames).unwrap();
        let leaf = PackedPlacementLeafV1 {
            domain_digest: store.domain_digest(),
            bounds: crate::generation::PartitionBoundsV1 {
                lower_inclusive: None,
                upper_exclusive: None,
            },
            rows: [b"history/1".as_slice(), b"history/2"]
                .into_iter()
                .zip(&receipts)
                .map(
                    |(key, receipt)| crate::generation::PlacementGenerationRowV1 {
                        key: key.to_vec(),
                        logical_digest: receipt.coordinate().sha256,
                        logical_length: receipt.coordinate().size_bytes,
                        placement: receipt.placement(),
                    },
                )
                .collect(),
        };
        let shape = GenerationShapeLimits {
            max_partitions: 2,
            max_rows_per_partition: 4,
            max_key_bytes: 64,
            max_leaf_bytes: 4096,
        };
        let digest = store.install_packed_leaf(&leaf, shape).unwrap();
        assert_eq!(store.install_packed_leaf(&leaf, shape).unwrap(), digest);
        let semantic = describe_placement_partition(
            store.domain_digest(),
            leaf.bounds.clone(),
            leaf.rows.clone().into_iter().map(Ok),
            shape,
        )
        .unwrap();
        let reference = PackedPartitionRefV1 {
            semantic,
            content_digest: digest,
        };
        let catalog = store
            .verify_packed_catalog_shape(
                b"private-domain",
                b"history",
                b"all",
                Digest256::of_bytes(b"cmd-history-key"),
                KeyComparatorV1::RawUnsignedBytes,
                &[reference.clone()],
                shape,
            )
            .unwrap();
        assert_ne!(catalog, digest);
        let mut false_semantic = reference.clone();
        false_semantic.semantic.leaf_digest = Digest256::of_bytes(b"same-count substitution");
        assert_eq!(
            store
                .verify_packed_catalog_shape(
                    b"private-domain",
                    b"history",
                    b"all",
                    Digest256::of_bytes(b"cmd-history-key"),
                    KeyComparatorV1::RawUnsignedBytes,
                    &[false_semantic],
                    shape,
                )
                .unwrap_err()
                .code,
            Code::InvalidReceipt
        );
        drop(store);
        let cold = SegmentStore::open_existing(&root.0, limits()).unwrap();
        let restored = cold.open_packed_leaf(digest, shape).unwrap();
        assert_eq!(restored, leaf);
        assert_eq!(
            cold.verify_packed_catalog_shape(
                b"private-domain",
                b"history",
                b"all",
                Digest256::of_bytes(b"cmd-history-key"),
                KeyComparatorV1::RawUnsignedBytes,
                &[reference],
                shape,
            )
            .unwrap(),
            catalog
        );
        let placements: Vec<_> = restored.rows.iter().map(|row| row.placement).collect();
        let admitted = cold
            .recover_placements(
                &placements,
                VerificationBudget {
                    max_receipts: 2,
                    max_segments: 1,
                    max_total_segment_bytes: 1024 * 1024,
                },
            )
            .unwrap();
        let mut selected = Vec::new();
        cold.read_selected(&admitted[1], 64, &mut selected).unwrap();
        assert_eq!(selected, second);
        let path = root.0.join("leaves").join(digest.to_hex());
        OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .write_all(b"corrupt!")
            .unwrap();
        assert_eq!(
            cold.open_packed_leaf(digest, shape).unwrap_err().code,
            Code::CorruptBytes
        );
    }

    #[test]
    fn two_physical_generations_retain_old_selected_bytes_after_repack() {
        let root = PrivateRoot::new();
        let store = SegmentStore::initialize_empty(&root.0, b"private-domain", limits()).unwrap();
        let exact = b"one historical version".to_vec();
        let padding = b"different physical order".to_vec();
        let mut old_exact = Cursor::new(exact.clone());
        let mut old_padding = Cursor::new(padding.clone());
        let old_receipts = store
            .seal_segment(
                b"old-placement",
                &mut [
                    FrameInput {
                        binding: binding(0),
                        declared_size: exact.len() as u64,
                        declared_sha256: Digest256::of_bytes(&exact),
                        reader: &mut old_exact,
                    },
                    FrameInput {
                        binding: binding(1),
                        declared_size: padding.len() as u64,
                        declared_sha256: Digest256::of_bytes(&padding),
                        reader: &mut old_padding,
                    },
                ],
            )
            .unwrap();
        let mut new_padding = Cursor::new(padding.clone());
        let mut new_exact = Cursor::new(exact.clone());
        let new_receipts = store
            .seal_segment(
                b"new-placement",
                &mut [
                    FrameInput {
                        binding: binding(1),
                        declared_size: padding.len() as u64,
                        declared_sha256: Digest256::of_bytes(&padding),
                        reader: &mut new_padding,
                    },
                    FrameInput {
                        binding: binding(0),
                        declared_size: exact.len() as u64,
                        declared_sha256: Digest256::of_bytes(&exact),
                        reader: &mut new_exact,
                    },
                ],
            )
            .unwrap();
        assert_ne!(old_receipts[0].placement(), new_receipts[1].placement());
        assert_ne!(
            old_receipts[0].segment_digest(),
            new_receipts[1].segment_digest()
        );
        let shape = GenerationShapeLimits {
            max_partitions: 1,
            max_rows_per_partition: 2,
            max_key_bytes: 64,
            max_leaf_bytes: 4096,
        };
        let leaf_for = |receipt: &ByteDurabilityReceipt| PackedPlacementLeafV1 {
            domain_digest: store.domain_digest(),
            bounds: crate::generation::PartitionBoundsV1 {
                lower_inclusive: None,
                upper_exclusive: None,
            },
            rows: vec![crate::generation::PlacementGenerationRowV1 {
                key: b"history/exact-version".to_vec(),
                logical_digest: receipt.coordinate().sha256,
                logical_length: receipt.coordinate().size_bytes,
                placement: receipt.placement(),
            }],
        };
        let old_leaf = leaf_for(&old_receipts[0]);
        let new_leaf = leaf_for(&new_receipts[1]);
        let old_digest = store.install_packed_leaf(&old_leaf, shape).unwrap();
        let new_digest = store.install_packed_leaf(&new_leaf, shape).unwrap();
        assert_ne!(old_digest, new_digest);
        let catalog_for = |leaf: &PackedPlacementLeafV1, content_digest| {
            let semantic = describe_placement_partition(
                store.domain_digest(),
                leaf.bounds.clone(),
                leaf.rows.clone().into_iter().map(Ok),
                shape,
            )
            .unwrap();
            store
                .verify_packed_catalog_shape(
                    b"private-domain",
                    b"history",
                    b"all",
                    Digest256::of_bytes(b"cmd-history-key"),
                    KeyComparatorV1::RawUnsignedBytes,
                    &[PackedPartitionRefV1 {
                        semantic,
                        content_digest,
                    }],
                    shape,
                )
                .unwrap()
        };
        let old_catalog = catalog_for(&old_leaf, old_digest);
        let new_catalog = catalog_for(&new_leaf, new_digest);
        assert_ne!(old_catalog, new_catalog);
        drop(store);
        let cold = SegmentStore::open_existing(&root.0, limits()).unwrap();
        let old_reopened = cold.open_packed_leaf(old_digest, shape).unwrap();
        let new_reopened = cold.open_packed_leaf(new_digest, shape).unwrap();
        assert_eq!(
            old_reopened.rows[0].logical_digest,
            new_reopened.rows[0].logical_digest
        );
        let old_admitted = cold
            .recover_placement(&old_reopened.rows[0].placement)
            .unwrap();
        let new_admitted = cold
            .recover_placement(&new_reopened.rows[0].placement)
            .unwrap();
        for receipt in [&new_admitted, &old_admitted] {
            let mut selected = Vec::new();
            cold.read_selected(receipt, 64, &mut selected).unwrap();
            assert_eq!(selected, exact);
        }
        let new_segment = root
            .0
            .join("segments")
            .join(new_admitted.segment_digest().to_hex());
        OpenOptions::new()
            .write(true)
            .open(new_segment)
            .unwrap()
            .write_all(b"broken!!")
            .unwrap();
        let mut refused = Vec::new();
        assert_eq!(
            cold.read_selected(&new_admitted, 64, &mut refused)
                .unwrap_err()
                .code,
            Code::CorruptBytes
        );
        assert!(refused.is_empty());
        let mut retained_old = Vec::new();
        cold.read_selected(&old_admitted, 64, &mut retained_old)
            .unwrap();
        assert_eq!(retained_old, exact);
    }

    #[test]
    fn installed_generation_reopens_two_complete_bounded_membership_streams() {
        use crate::selected::{
            GenerationCatalogV1, GenerationCutV1, GenerationDescriptorV1, GenerationNamespaceV1,
            GenerationReadLimits,
        };
        use std::sync::atomic::AtomicBool;

        let root = PrivateRoot::new();
        let store = SegmentStore::initialize_empty(&root.0, b"private-domain", limits()).unwrap();
        let old = b"retained exact version".to_vec();
        let new = b"current exact version".to_vec();
        let mut old_reader = Cursor::new(old.clone());
        let mut new_reader = Cursor::new(new.clone());
        let receipts = store
            .seal_segment(
                b"generation-members",
                &mut [
                    FrameInput {
                        binding: binding(0),
                        declared_size: old.len() as u64,
                        declared_sha256: Digest256::of_bytes(&old),
                        reader: &mut old_reader,
                    },
                    FrameInput {
                        binding: binding(1),
                        declared_size: new.len() as u64,
                        declared_sha256: Digest256::of_bytes(&new),
                        reader: &mut new_reader,
                    },
                ],
            )
            .unwrap();
        let shape = GenerationShapeLimits {
            max_partitions: 2,
            max_rows_per_partition: 4,
            max_key_bytes: 64,
            max_leaf_bytes: 4096,
        };
        let read_limits = GenerationReadLimits {
            max_descriptor_bytes: 4096,
            shape,
            max_stream_rows: 4,
            max_stream_key_bytes: 128,
        };
        let row = |key: &[u8], receipt: &ByteDurabilityReceipt| {
            crate::generation::PlacementGenerationRowV1 {
                key: key.to_vec(),
                logical_digest: receipt.coordinate().sha256,
                logical_length: receipt.coordinate().size_bytes,
                placement: receipt.placement(),
            }
        };
        let make_catalog =
            |namespace: &[u8],
             codec: &[u8],
             rows: Vec<crate::generation::PlacementGenerationRowV1>| {
                let bounds = crate::generation::PartitionBoundsV1 {
                    lower_inclusive: None,
                    upper_exclusive: None,
                };
                let leaf = PackedPlacementLeafV1 {
                    domain_digest: store.domain_digest(),
                    bounds: bounds.clone(),
                    rows,
                };
                let content_digest = store.install_packed_leaf(&leaf, shape).unwrap();
                let semantic = describe_placement_partition(
                    store.domain_digest(),
                    bounds,
                    leaf.rows.clone().into_iter().map(Ok),
                    shape,
                )
                .unwrap();
                let partitions = vec![PackedPartitionRefV1 {
                    semantic,
                    content_digest,
                }];
                let key_codec_digest = Digest256::of_bytes(codec);
                let catalog_root = store
                    .verify_packed_catalog_shape(
                        b"private-domain",
                        namespace,
                        b"all",
                        key_codec_digest,
                        KeyComparatorV1::RawUnsignedBytes,
                        &partitions,
                        shape,
                    )
                    .unwrap();
                GenerationCatalogV1 {
                    key_codec_digest,
                    catalog_root,
                    partitions,
                }
            };
        let history = make_catalog(
            b"cmd2.history.v1",
            b"history-key-codec",
            vec![row(b"h/a/1", &receipts[0]), row(b"h/a/2", &receipts[1])],
        );
        let current = make_catalog(
            b"cmd2.current.v1",
            b"current-key-codec",
            vec![row(b"c/a", &receipts[1])],
        );
        let cut = GenerationCutV1 {
            store_id: store.store_id(),
            domain_digest: store.domain_digest(),
            through_seq: 1,
            audit_generation: 7,
            database_oid: 123,
            schema_profile_digest: Digest256::of_bytes(b"profile"),
            state_digest: Digest256::of_bytes(b"audited-state"),
            log_digest: Digest256::of_bytes(b"contiguous-log"),
            historical_members: 2,
            current_members: 1,
            history_membership_root: Digest256::of_bytes(b"cmd-audited-history"),
            current_membership_root: Digest256::of_bytes(b"cmd-audited-current"),
        };
        let selected = store
            .install_generation_candidate(
                GenerationDescriptorV1 {
                    cut: cut.clone(),
                    history,
                    current,
                },
                read_limits,
            )
            .unwrap();
        let descriptor_digest = selected.digest();
        drop(selected);
        drop(store);
        let backup = PrivateRoot::new();
        fs::copy(root.0.join("store.meta"), backup.0.join("store.meta")).unwrap();
        for directory in [
            "staging",
            "segments",
            "pins",
            "attempts",
            "leaves",
            "generations",
        ] {
            let destination = backup.0.join(directory);
            fs::create_dir(&destination).unwrap();
            for entry in fs::read_dir(root.0.join(directory)).unwrap() {
                let entry = entry.unwrap();
                assert!(entry.file_type().unwrap().is_file());
                fs::copy(entry.path(), destination.join(entry.file_name())).unwrap();
            }
        }
        let cold = SegmentStore::open_existing(&backup.0, limits()).unwrap();
        let selected = cold
            .open_generation_candidate(descriptor_digest, &cut, read_limits)
            .unwrap();
        let mut wrong_cut = cut.clone();
        wrong_cut.through_seq = 2;
        assert_eq!(
            cold.open_generation_candidate(descriptor_digest, &wrong_cut, read_limits)
                .unwrap_err()
                .code,
            Code::InvalidReceipt
        );
        let mut wrong_root = cut.clone();
        wrong_root.current_membership_root = Digest256::of_bytes(b"same-count-current-swap");
        assert_eq!(
            cold.open_generation_candidate(descriptor_digest, &wrong_root, read_limits)
                .unwrap_err()
                .code,
            Code::InvalidReceipt
        );
        let cancelled = AtomicBool::new(false);
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut history = selected
            .stream(GenerationNamespaceV1::History, read_limits)
            .unwrap();
        let retained_row = history.next_row(deadline, &cancelled).unwrap().unwrap();
        assert_eq!(retained_row.key, b"h/a/1");
        let retained = cold.recover_placement(&retained_row.placement).unwrap();
        let mut retained_exact = Vec::new();
        cold.read_selected(&retained, old.len() as u64, &mut retained_exact)
            .unwrap();
        assert_eq!(retained_exact, old);
        assert_eq!(
            history.next_row(deadline, &cancelled).unwrap().unwrap().key,
            b"h/a/2"
        );
        assert!(history.next_row(deadline, &cancelled).unwrap().is_none());
        assert_eq!(history.coverage().unwrap().rows, 2);
        let mut current = selected
            .stream(GenerationNamespaceV1::Current, read_limits)
            .unwrap();
        let current_row = current.next_row(deadline, &cancelled).unwrap().unwrap();
        assert_eq!(current_row.key, b"c/a");
        let recovered = cold.recover_placement(&current_row.placement).unwrap();
        let mut restored_exact = Vec::new();
        cold.read_selected(&recovered, new.len() as u64, &mut restored_exact)
            .unwrap();
        assert_eq!(restored_exact, new);
        assert!(current.next_row(deadline, &cancelled).unwrap().is_none());
        assert_eq!(current.coverage().unwrap().rows, 1);
        let mut tiny = read_limits;
        tiny.max_stream_key_bytes = 1;
        let mut refused = selected
            .stream(GenerationNamespaceV1::History, tiny)
            .unwrap();
        assert_eq!(
            refused.next_row(deadline, &cancelled).unwrap_err().code,
            Code::BudgetExceeded
        );
        assert!(refused.coverage().is_none());
        assert_eq!(
            refused.next_row(deadline, &cancelled).unwrap_err().code,
            Code::InvalidReceipt
        );
        cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
        assert_eq!(
            selected
                .stream(GenerationNamespaceV1::History, read_limits)
                .unwrap()
                .next_row(deadline, &cancelled)
                .unwrap_err()
                .code,
            Code::Cancelled
        );
        let expired = Instant::now() - Duration::from_secs(1);
        assert_eq!(
            selected
                .stream(GenerationNamespaceV1::History, read_limits)
                .unwrap()
                .next_row(expired, &AtomicBool::new(false))
                .unwrap_err()
                .code,
            Code::DeadlineExceeded
        );
        cancelled.store(false, std::sync::atomic::Ordering::Relaxed);
        let current_leaf = selected.descriptor().current.partitions[0].content_digest;
        let leaf_path = backup.0.join("leaves").join(current_leaf.to_hex());
        OpenOptions::new()
            .write(true)
            .open(leaf_path)
            .unwrap()
            .write_all(b"broken!!")
            .unwrap();
        let mut corrupted = selected
            .stream(GenerationNamespaceV1::Current, read_limits)
            .unwrap();
        assert_eq!(
            corrupted.next_row(deadline, &cancelled).unwrap_err().code,
            Code::CorruptBytes
        );
        assert!(corrupted.coverage().is_none());
    }

    #[test]
    fn audited_root_guard_rejects_other_store_and_incomplete_same_id_copy() {
        let root = PrivateRoot::new();
        let store = SegmentStore::initialize_empty(&root.0, b"same-domain", limits()).unwrap();
        let bytes = b"audited retained bytes".to_vec();
        let mut reader = Cursor::new(bytes.clone());
        let receipt = store
            .seal_segment(
                b"audited-prepare",
                &mut [FrameInput {
                    binding: binding(0),
                    declared_size: bytes.len() as u64,
                    declared_sha256: Digest256::of_bytes(&bytes),
                    reader: &mut reader,
                }],
            )
            .unwrap()
            .remove(0);
        let guard = store.hold_audit_root().unwrap();
        guard.require_store(&store.clone()).unwrap();
        assert_eq!(
            store
                .abort_uncommitted(
                    receipt.pin_id(),
                    receipt.prepare_id(),
                    receipt.fence_epoch(),
                )
                .unwrap_err()
                .code,
            Code::PinConflict
        );

        let other_root = PrivateRoot::new();
        let other =
            SegmentStore::initialize_empty(&other_root.0, b"same-domain", limits()).unwrap();
        assert_eq!(
            guard.require_store(&other).unwrap_err().code,
            Code::InvalidReceipt
        );

        let copy_root = PrivateRoot::new();
        for directory in [
            "staging",
            "segments",
            "pins",
            "attempts",
            "leaves",
            "generations",
        ] {
            fs::create_dir(copy_root.0.join(directory)).unwrap();
        }
        fs::copy(root.0.join("store.meta"), copy_root.0.join("store.meta")).unwrap();
        let incomplete_copy = SegmentStore::open_existing(&copy_root.0, limits()).unwrap();
        assert_eq!(incomplete_copy.store_id(), store.store_id());
        assert_eq!(incomplete_copy.custody_domain(), store.custody_domain());
        assert_eq!(
            guard.require_store(&incomplete_copy).unwrap_err().code,
            Code::InvalidReceipt
        );
    }

    #[test]
    fn verified_guard_excludes_same_process_abort_until_commit_decision() {
        let root = PrivateRoot::new();
        let store = SegmentStore::initialize_empty(&root.0, b"private-domain", limits()).unwrap();
        let bytes = b"guarded exact bytes".to_vec();
        let mut first_reader = Cursor::new(bytes.clone());
        let mut second_reader = Cursor::new(bytes.clone());
        let mut frames = [
            FrameInput {
                binding: binding(0),
                declared_size: bytes.len() as u64,
                declared_sha256: Digest256::of_bytes(&bytes),
                reader: &mut first_reader,
            },
            FrameInput {
                binding: binding(1),
                declared_size: bytes.len() as u64,
                declared_sha256: Digest256::of_bytes(&bytes),
                reader: &mut second_reader,
            },
        ];
        let receipts = store.seal_segment(b"guard-prepare", &mut frames).unwrap();
        let budget = VerificationBudget {
            max_receipts: 2,
            max_segments: 1,
            max_total_segment_bytes: receipts[0].segment_size(),
        };
        // Later members use the operation's already decoded pin journal, but
        // their individual fence, coordinate and owner binding remain exact.
        for changed in 0..3 {
            let mut invalid = receipts.clone();
            match changed {
                0 => invalid[1].fence_epoch += 1,
                1 => invalid[1].coordinate.sha256 = Digest256::of_bytes(b"substituted frame"),
                _ => invalid[1].binding.member_slot += 1,
            }
            assert_eq!(
                store.verify_and_hold(&invalid, budget).unwrap_err().code,
                Code::InvalidReceipt
            );
        }
        let guard = store.verify_and_hold(&receipts, budget).unwrap();
        assert_eq!(guard.prepare_id(), b"guard-prepare");
        assert_eq!(guard.receipts().len(), 2);
        assert_eq!(
            store
                .abort_uncommitted(receipts[0].pin_id(), b"guard-prepare", 1)
                .unwrap_err()
                .code,
            Code::PinConflict
        );
        let mut selected = Vec::new();
        store
            .read_selected(&receipts[1], 64, &mut selected)
            .unwrap();
        assert_eq!(selected, bytes);
        drop(guard);
        assert_eq!(
            store
                .abort_uncommitted(receipts[0].pin_id(), b"guard-prepare", 1)
                .unwrap(),
            2
        );
        assert_eq!(
            store.verify_and_hold(&receipts, budget).unwrap_err().code,
            Code::InvalidReceipt
        );
    }

    #[test]
    fn pin_directory_lease_unlocks_even_when_open_description_is_duplicated() {
        let root = PrivateRoot::new();
        let store = SegmentStore::initialize_empty(&root.0, b"private-domain", limits()).unwrap();
        let lease = store
            .lock_pin_dir(FlockOperation::NonBlockingLockShared)
            .unwrap();
        // A forked child inherits this same open-file-description until exec;
        // try_clone exercises the lock lifetime without starting a process.
        let inherited = lease.file.try_clone().unwrap();
        drop(lease);
        let exclusive = store
            .lock_pin_dir(FlockOperation::NonBlockingLockExclusive)
            .expect("lease drop explicitly unlocks inherited description");
        drop(exclusive);
        drop(inherited);
    }

    #[test]
    fn fenced_intent_binds_cmd_attempt_and_excludes_abort_under_guard() {
        let root = PrivateRoot::new();
        let store = SegmentStore::initialize_empty(&root.0, b"private-domain", limits()).unwrap();
        let bytes = b"fenced exact bytes".to_vec();
        let mut reader = Cursor::new(bytes.clone());
        let mut frames = [FrameInput {
            binding: binding(0),
            declared_size: bytes.len() as u64,
            declared_sha256: Digest256::of_bytes(&bytes),
            reader: &mut reader,
        }];
        assert_eq!(
            store
                .seal_segment_fenced(b"fenced-prepare", 0, 0, &mut frames)
                .unwrap_err()
                .code,
            Code::InvalidReceipt
        );
        assert_eq!(
            store
                .seal_segment_fenced(b"fenced-prepare", 7, 1, &mut frames)
                .unwrap_err()
                .code,
            Code::UnsupportedOversized
        );
        assert_eq!(fs::read_dir(root.0.join("attempts")).unwrap().count(), 0);
        let receipts = store
            .seal_segment_fenced(b"fenced-prepare", 7, 0, &mut frames)
            .unwrap();
        assert!(matches!(
            store
                .recover_attempt_fenced(b"fenced-prepare", 7, 0)
                .unwrap(),
            Some(AttemptRecovery::Sealed { .. })
        ));
        assert_eq!(
            store
                .recover_attempt_fenced(b"fenced-prepare", 8, 0)
                .unwrap_err()
                .code,
            Code::InvalidReceipt
        );
        assert_eq!(
            store.recover_attempt(b"fenced-prepare").unwrap_err().code,
            Code::InvalidReceipt
        );
        assert_eq!(
            store
                .abort_uncommitted(receipts[0].pin_id(), b"fenced-prepare", 1)
                .unwrap_err()
                .code,
            Code::InvalidReceipt
        );
        assert_eq!(
            store
                .seal_segment(b"fenced-prepare", &mut frames)
                .unwrap_err()
                .code,
            Code::PinConflict
        );
        let budget = VerificationBudget {
            max_receipts: 1,
            max_segments: 1,
            max_total_segment_bytes: receipts[0].segment_size(),
        };
        let guard = store
            .verify_and_hold_fenced(b"fenced-prepare", 7, 0, &receipts, budget)
            .unwrap();
        assert_eq!(guard.receipts().len(), 1);
        assert_eq!(
            store
                .abort_uncommitted_fenced(receipts[0].pin_id(), b"fenced-prepare", 7, 0, 1)
                .unwrap_err()
                .code,
            Code::PinConflict
        );
        drop(guard);
        assert_eq!(
            store
                .abort_uncommitted_fenced(receipts[0].pin_id(), b"fenced-prepare", 8, 0, 1)
                .unwrap_err()
                .code,
            Code::InvalidReceipt
        );
        assert_eq!(
            store
                .abort_uncommitted_fenced(receipts[0].pin_id(), b"fenced-prepare", 7, 0, 1)
                .unwrap(),
            2
        );
        assert!(matches!(
            store
                .recover_attempt_fenced(b"fenced-prepare", 7, 0)
                .unwrap(),
            Some(AttemptRecovery::Aborted { fence_epoch: 2, .. })
        ));
        let mut intent = OpenOptions::new()
            .write(true)
            .open(
                root.0
                    .join("attempts")
                    .join(attempt_name(b"fenced-prepare")),
            )
            .unwrap();
        intent.seek(SeekFrom::Start(72)).unwrap();
        intent.write_all(&8u64.to_le_bytes()).unwrap();
        intent.sync_all().unwrap();
        assert_eq!(
            store
                .recover_attempt_fenced(b"fenced-prepare", 7, 0)
                .unwrap_err()
                .code,
            Code::InvalidReceipt
        );
    }

    #[test]
    fn verification_budget_and_duplicate_receipt_refuse_before_commit() {
        let root = PrivateRoot::new();
        let store = SegmentStore::initialize_empty(&root.0, b"private-domain", limits()).unwrap();
        let bytes = b"budgeted bytes".to_vec();
        let mut reader = Cursor::new(bytes.clone());
        let mut frames = [FrameInput {
            binding: binding(0),
            declared_size: bytes.len() as u64,
            declared_sha256: Digest256::of_bytes(&bytes),
            reader: &mut reader,
        }];
        let receipt = store
            .seal_segment(b"budget-prepare", &mut frames)
            .unwrap()
            .remove(0);
        let too_small = VerificationBudget {
            max_receipts: 1,
            max_segments: 1,
            max_total_segment_bytes: receipt.segment_size() - 1,
        };
        assert_eq!(
            store
                .verify_and_hold(&[receipt.clone()], too_small)
                .unwrap_err()
                .code,
            Code::BudgetExceeded
        );
        let enough = VerificationBudget {
            max_receipts: 2,
            max_segments: 1,
            max_total_segment_bytes: receipt.segment_size(),
        };
        assert_eq!(
            store
                .verify_and_hold(&[receipt.clone(), receipt], enough)
                .unwrap_err()
                .code,
            Code::InvalidReceipt
        );
    }

    #[test]
    fn exact_prepare_intent_recovers_seal_and_retains_abort_fence() {
        let root = PrivateRoot::new();
        let store = SegmentStore::initialize_empty(&root.0, b"private-domain", limits()).unwrap();
        assert!(store.recover_attempt(b"attempt-7").unwrap().is_none());
        let bytes = b"attempt-bound bytes".to_vec();
        let mut reader = Cursor::new(bytes.clone());
        let receipt = store
            .seal_segment(
                b"attempt-7",
                &mut [FrameInput {
                    binding: binding(7),
                    declared_size: bytes.len() as u64,
                    declared_sha256: Digest256::of_bytes(&bytes),
                    reader: &mut reader,
                }],
            )
            .unwrap()
            .remove(0);
        let cold = SegmentStore::open_existing(&root.0, limits()).unwrap();
        let Some(AttemptRecovery::Sealed { receipts }) =
            cold.recover_attempt(b"attempt-7").unwrap()
        else {
            panic!("sealed attempt not found by exact prepare ID");
        };
        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].placement(), receipt.placement());
        let mut retry_reader = Cursor::new(bytes);
        assert_eq!(
            cold.seal_segment(
                b"attempt-7",
                &mut [FrameInput {
                    binding: binding(7),
                    declared_size: receipt.coordinate().size_bytes,
                    declared_sha256: receipt.coordinate().sha256,
                    reader: &mut retry_reader,
                }],
            )
            .unwrap_err()
            .code,
            Code::PinConflict
        );
        assert_eq!(
            cold.abort_uncommitted(receipt.pin_id(), b"attempt-7", 1)
                .unwrap(),
            2
        );
        assert!(matches!(
            cold.recover_attempt(b"attempt-7").unwrap(),
            Some(AttemptRecovery::Aborted { fence_epoch: 2, .. })
        ));
        let mut intent = OpenOptions::new()
            .write(true)
            .open(root.0.join("attempts").join(attempt_name(b"attempt-7")))
            .unwrap();
        intent.write_all(b"X").unwrap();
        intent.sync_all().unwrap();
        assert_eq!(
            cold.recover_attempt(b"attempt-7").unwrap_err().code,
            Code::InvalidReceipt
        );
    }

    #[test]
    fn placement_wire_cold_recovers_exact_member_and_rejects_mismatch() {
        let root = PrivateRoot::new();
        let store = SegmentStore::initialize_empty(&root.0, b"private-domain", limits()).unwrap();
        let same = b"same bytes, separate subjects".to_vec();
        let mut first_reader = Cursor::new(same.clone());
        let mut second_reader = Cursor::new(same.clone());
        let mut frames = [
            FrameInput {
                binding: binding(0),
                declared_size: same.len() as u64,
                declared_sha256: Digest256::of_bytes(&same),
                reader: &mut first_reader,
            },
            FrameInput {
                binding: binding(1),
                declared_size: same.len() as u64,
                declared_sha256: Digest256::of_bytes(&same),
                reader: &mut second_reader,
            },
        ];
        let receipts = store
            .seal_segment(b"placement-prepare", &mut frames)
            .unwrap();
        let wire = receipts[1].placement().encode();
        let placement = PlacementV1::decode(&wire).unwrap();
        assert_eq!(placement, receipts[1].placement());
        assert_eq!(placement.coordinate().sha256, Digest256::of_bytes(&same));
        let cold = SegmentStore::open_existing(&root.0, limits()).unwrap();
        let recovered = cold.recover_placement(&placement).unwrap();
        assert_eq!(recovered.binding(), &binding(1));
        let both = cold
            .recover_placements(
                &[receipts[0].placement(), placement],
                VerificationBudget {
                    max_receipts: 2,
                    max_segments: 1,
                    max_total_segment_bytes: placement.segment_size(),
                },
            )
            .unwrap();
        assert_eq!(both[0].binding(), &binding(0));
        assert_eq!(both[1].binding(), &binding(1));
        assert_eq!(
            cold.recover_placements(
                &[receipts[0].placement(), placement],
                VerificationBudget {
                    max_receipts: 2,
                    max_segments: 1,
                    max_total_segment_bytes: placement.segment_size() - 1,
                },
            )
            .unwrap_err()
            .code,
            Code::BudgetExceeded
        );
        let mut selected = Vec::new();
        cold.read_selected(&recovered, same.len() as u64, &mut selected)
            .unwrap();
        assert_eq!(selected, same);

        let mut bad = wire;
        bad[8] = 2;
        assert_eq!(
            PlacementV1::decode(&bad).unwrap_err().code,
            Code::InvalidFormat
        );
        assert_eq!(
            PlacementV1::decode(&wire[..wire.len() - 1])
                .unwrap_err()
                .code,
            Code::InvalidFormat
        );
        let mut trailing = wire.to_vec();
        trailing.push(0);
        assert_eq!(
            PlacementV1::decode(&trailing).unwrap_err().code,
            Code::InvalidFormat
        );
        let mut wrong_member = placement;
        wrong_member.frame_index = 0;
        assert_eq!(
            cold.recover_placement(&wrong_member).unwrap_err().code,
            Code::InvalidReceipt
        );
        let mut wrong_store = placement;
        wrong_store.store_id = [0; 16];
        assert_eq!(
            cold.recover_placement(&wrong_store).unwrap_err().code,
            Code::InvalidReceipt
        );
    }

    #[test]
    fn physical_backup_copy_restores_into_new_root_and_refuses_corrupt_bytes() {
        let source_root = PrivateRoot::new();
        let backup_root = PrivateRoot::new();
        let source =
            SegmentStore::initialize_empty(&source_root.0, b"backup-domain", limits()).unwrap();
        let bytes = b"retained historical exact bytes".to_vec();
        let mut reader = Cursor::new(bytes.clone());
        let receipt = source
            .seal_segment(
                b"backup-prepare",
                &mut [FrameInput {
                    binding: binding(7),
                    declared_size: bytes.len() as u64,
                    declared_sha256: Digest256::of_bytes(&bytes),
                    reader: &mut reader,
                }],
            )
            .unwrap()
            .remove(0);
        let placement = PlacementV1::decode(&receipt.placement().encode()).unwrap();
        for directory in ["staging", "segments", "pins", "attempts"] {
            fs::create_dir(backup_root.0.join(directory)).unwrap();
        }
        fs::copy(
            source_root.0.join("store.meta"),
            backup_root.0.join("store.meta"),
        )
        .unwrap();
        let segment_name = placement.segment_digest().to_hex();
        fs::copy(
            source_root.0.join("segments").join(&segment_name),
            backup_root.0.join("segments").join(&segment_name),
        )
        .unwrap();
        let pin_name = hex_id(placement.pin_id());
        fs::copy(
            source_root.0.join("pins").join(&pin_name),
            backup_root.0.join("pins").join(&pin_name),
        )
        .unwrap();
        let intent_name = attempt_name(b"backup-prepare");
        fs::copy(
            source_root.0.join("attempts").join(&intent_name),
            backup_root.0.join("attempts").join(&intent_name),
        )
        .unwrap();

        let restored = SegmentStore::open_existing(&backup_root.0, limits()).unwrap();
        let recovered = restored.recover_placement(&placement).unwrap();
        assert!(matches!(
            restored.recover_attempt(b"backup-prepare").unwrap(),
            Some(AttemptRecovery::Sealed { .. })
        ));
        let mut exact = Vec::new();
        restored
            .read_selected(&recovered, bytes.len() as u64, &mut exact)
            .unwrap();
        assert_eq!(exact, bytes);
        let mut file = OpenOptions::new()
            .write(true)
            .open(backup_root.0.join("segments").join(segment_name))
            .unwrap();
        file.seek(SeekFrom::Start(
            placement.coordinate().header_offset + format::FRAME_HEADER_BYTES,
        ))
        .unwrap();
        file.write_all(b"X").unwrap();
        file.sync_all().unwrap();
        assert_eq!(
            restored.recover_placement(&placement).unwrap_err().code,
            Code::CorruptBytes
        );
        let mut disclosed = Vec::new();
        assert_eq!(
            restored
                .read_selected(&recovered, bytes.len() as u64, &mut disclosed)
                .unwrap_err()
                .code,
            Code::CorruptBytes
        );
        assert!(disclosed.is_empty());
    }

    #[test]
    fn synthetic_repack_keeps_old_and_new_exact_physical_placements() {
        let root = PrivateRoot::new();
        let store = SegmentStore::initialize_empty(&root.0, b"repack-domain", limits()).unwrap();
        let original = b"retained exact frame".to_vec();
        let mut source_reader = Cursor::new(original.clone());
        let old = store
            .seal_segment(
                b"original-prepare",
                &mut [FrameInput {
                    binding: binding(0),
                    declared_size: original.len() as u64,
                    declared_sha256: Digest256::of_bytes(&original),
                    reader: &mut source_reader,
                }],
            )
            .unwrap()
            .remove(0);
        let mut verified_old = Vec::new();
        store
            .read_selected(&old, original.len() as u64, &mut verified_old)
            .unwrap();
        let filler = b"different packing".to_vec();
        let mut selected_reader = Cursor::new(verified_old);
        let mut filler_reader = Cursor::new(filler.clone());
        let repacked = store
            .seal_segment(
                b"new-placement-prepare",
                &mut [
                    FrameInput {
                        binding: binding(0),
                        declared_size: original.len() as u64,
                        declared_sha256: Digest256::of_bytes(&original),
                        reader: &mut selected_reader,
                    },
                    FrameInput {
                        binding: binding(1),
                        declared_size: filler.len() as u64,
                        declared_sha256: Digest256::of_bytes(&filler),
                        reader: &mut filler_reader,
                    },
                ],
            )
            .unwrap();
        assert_ne!(old.segment_digest(), repacked[0].segment_digest());
        assert_eq!(old.coordinate(), repacked[0].coordinate());
        let old_placement = old.placement();
        let new_placement = repacked[0].placement();
        assert_ne!(old_placement, new_placement);
        for placement in [old_placement, new_placement] {
            let recovered = store.recover_placement(&placement).unwrap();
            let mut output = Vec::new();
            store
                .read_selected(&recovered, original.len() as u64, &mut output)
                .unwrap();
            assert_eq!(output, original);
        }
    }

    #[test]
    fn sealed_bytes_recover_and_abort_fences_stale_receipts() {
        let root = PrivateRoot::new();
        let store = SegmentStore::initialize_empty(&root.0, b"private-domain", limits()).unwrap();
        let first = b"same exact bytes".to_vec();
        let second = first.clone();
        let mut first_reader = Cursor::new(first.clone());
        let mut second_reader = Cursor::new(second);
        let mut frames = [
            FrameInput {
                binding: binding(0),
                declared_size: first.len() as u64,
                declared_sha256: Digest256::of_bytes(&first),
                reader: &mut first_reader,
            },
            FrameInput {
                binding: binding(1),
                declared_size: first.len() as u64,
                declared_sha256: Digest256::of_bytes(&first),
                reader: &mut second_reader,
            },
        ];
        let receipts = store.seal_segment(b"cmd-prepare-1", &mut frames).unwrap();
        assert_eq!(receipts.len(), 2);
        assert_eq!(receipts[0].custody_domain(), b"private-domain");
        assert_ne!(receipts[0].receipt_id(), receipts[1].receipt_id());
        store.verify_receipt(&receipts[0]).unwrap();
        let cold = SegmentStore::open_existing(&root.0, limits()).unwrap();
        assert_eq!(
            cold.verify_receipt(&receipts[0]).unwrap_err().code,
            Code::InvalidReceipt
        );
        let recovered = cold.recover_sealed(receipts[0].pin_id()).unwrap();
        assert_eq!(recovered[0].receipt_id(), receipts[0].receipt_id());
        let mut selected = Vec::new();
        store
            .read_selected(&receipts[1], first.len() as u64, &mut selected)
            .unwrap();
        assert_eq!(selected, first);
        assert_eq!(
            store
                .abort_uncommitted(receipts[0].pin_id(), b"cmd-prepare-1", 1)
                .unwrap(),
            2
        );
        assert_eq!(
            store.verify_receipt(&receipts[0]).unwrap_err().code,
            Code::InvalidReceipt
        );
        assert_eq!(
            store
                .read_selected(&receipts[1], first.len() as u64, &mut Vec::new())
                .unwrap_err()
                .code,
            Code::InvalidReceipt
        );
    }

    #[test]
    fn selected_same_inode_corruption_fails_closed() {
        let root = PrivateRoot::new();
        let store = SegmentStore::initialize_empty(&root.0, b"private-domain", limits()).unwrap();
        let bytes = b"immutable selected bytes".to_vec();
        let mut reader = Cursor::new(bytes.clone());
        let mut frames = [FrameInput {
            binding: binding(0),
            declared_size: bytes.len() as u64,
            declared_sha256: Digest256::of_bytes(&bytes),
            reader: &mut reader,
        }];
        let receipt = store
            .seal_segment(b"cmd-prepare-2", &mut frames)
            .unwrap()
            .remove(0);
        let path = root
            .0
            .join("segments")
            .join(receipt.segment_digest().to_hex());
        let mut file = OpenOptions::new().write(true).open(path).unwrap();
        file.seek(SeekFrom::Start(
            receipt.coordinate().header_offset + format::FRAME_HEADER_BYTES + 1,
        ))
        .unwrap();
        file.write_all(b"X").unwrap();
        file.sync_all().unwrap();
        assert_eq!(
            store.verify_receipt(&receipt).unwrap_err().code,
            Code::CorruptBytes
        );
        let mut sink = Vec::new();
        assert_eq!(
            store
                .read_selected(&receipt, bytes.len() as u64, &mut sink)
                .unwrap_err()
                .code,
            Code::CorruptBytes
        );
        assert!(sink.is_empty());
    }

    #[test]
    fn oversized_frame_refuses_without_creating_a_pin() {
        let root = PrivateRoot::new();
        let store = SegmentStore::initialize_empty(&root.0, b"private-domain", limits()).unwrap();
        let mut reader = Cursor::new(vec![1u8; 1]);
        let mut frames = [FrameInput {
            binding: binding(0),
            declared_size: limits().max_frame_bytes + 1,
            declared_sha256: Digest256::of_bytes(&[1]),
            reader: &mut reader,
        }];
        assert_eq!(
            store
                .seal_segment(b"cmd-prepare-3", &mut frames)
                .unwrap_err()
                .code,
            Code::UnsupportedOversized
        );
        assert_eq!(fs::read_dir(root.0.join("pins")).unwrap().count(), 0);
    }

    #[test]
    fn initialized_store_remains_anchored_after_root_path_replacement() {
        let parent = PrivateRoot::new();
        let root_path = parent.0.join("store");
        let moved_path = parent.0.join("original-store");
        let decoy_path = parent.0.join("decoy");
        fs::create_dir(&root_path).unwrap();
        fs::create_dir(&decoy_path).unwrap();
        let store =
            SegmentStore::initialize_empty(&root_path, b"private-domain", limits()).unwrap();
        fs::rename(&root_path, &moved_path).unwrap();
        symlink(&decoy_path, &root_path).unwrap();
        let bytes = b"anchored segment".to_vec();
        let mut reader = Cursor::new(bytes.clone());
        let mut frames = [FrameInput {
            binding: binding(0),
            declared_size: bytes.len() as u64,
            declared_sha256: Digest256::of_bytes(&bytes),
            reader: &mut reader,
        }];
        let receipt = store
            .seal_segment(b"anchored-prepare", &mut frames)
            .unwrap()
            .remove(0);
        store.verify_receipt(&receipt).unwrap();
        assert!(
            moved_path
                .join("segments")
                .join(receipt.segment_digest().to_hex())
                .is_file()
        );
        assert_eq!(fs::read_dir(decoy_path).unwrap().count(), 0);
    }

    #[test]
    fn sealed_segment_survives_sigkill_and_cold_reopen() {
        let root = PrivateRoot::new();
        let test_binary = env::current_exe().unwrap();
        let mut child = Command::new(test_binary)
            .arg("--exact")
            .arg("store::tests::seal_then_sigkill_child")
            .arg("--ignored")
            .env("TOS_SEGMENT_CRASH_TEST_ROOT", &root.0)
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("crash child exceeded 30-second bound");
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(status.signal(), Some(9));
        let store = SegmentStore::open_existing(&root.0, limits()).unwrap();
        let Some(AttemptRecovery::Sealed { mut receipts }) =
            store.recover_attempt(b"crash-prepare").unwrap()
        else {
            panic!("sealed prepare absent after crash");
        };
        let receipt = receipts.remove(0);
        store.verify_receipt(&receipt).unwrap();
        let mut bytes = Vec::new();
        store.read_selected(&receipt, 64, &mut bytes).unwrap();
        assert_eq!(bytes, b"sealed before process death");
    }

    #[test]
    #[ignore = "run only as child of sealed_segment_survives_sigkill_and_cold_reopen"]
    fn seal_then_sigkill_child() {
        let root = PathBuf::from(env::var_os("TOS_SEGMENT_CRASH_TEST_ROOT").expect("child root"));
        let store = SegmentStore::initialize_empty(&root, b"private-domain", limits()).unwrap();
        let bytes = b"sealed before process death".to_vec();
        let mut reader = Cursor::new(bytes.clone());
        let mut frames = [FrameInput {
            binding: binding(0),
            declared_size: bytes.len() as u64,
            declared_sha256: Digest256::of_bytes(&bytes),
            reader: &mut reader,
        }];
        store.seal_segment(b"crash-prepare", &mut frames).unwrap();
        let _ = Command::new("/usr/bin/kill")
            .arg("-9")
            .arg(std::process::id().to_string())
            .status()
            .unwrap();
        panic!("SIGKILL did not terminate child");
    }

    #[test]
    fn crash_barriers_never_recover_an_unsealed_receipt() {
        for phase in [
            "intent-synced",
            "pin-synced",
            "stage-synced",
            "segment-installed",
            "segments-dir-synced",
            "staging-dir-synced",
            "sealed-pin-synced",
        ] {
            let root = PrivateRoot::new();
            let mut child = Command::new(env::current_exe().unwrap())
                .arg("--exact")
                .arg("store::tests::seal_at_phase_then_sigkill_child")
                .arg("--ignored")
                .env("TOS_SEGMENT_CRASH_TEST_ROOT", &root.0)
                .env("TOS_SEGMENT_CRASH_AT", phase)
                .spawn()
                .unwrap();
            let deadline = Instant::now() + Duration::from_secs(30);
            let status = loop {
                if let Some(status) = child.try_wait().unwrap() {
                    break status;
                }
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("{phase} crash child exceeded 30-second bound");
                }
                std::thread::sleep(Duration::from_millis(10));
            };
            assert_eq!(status.signal(), Some(9), "{phase} did not SIGKILL");
            let store = SegmentStore::open_existing(&root.0, limits()).unwrap();
            match (
                phase,
                store.recover_attempt(b"phase-crash-prepare").unwrap(),
            ) {
                ("intent-synced", Some(AttemptRecovery::IntentOnly { .. })) => {}
                ("sealed-pin-synced", Some(AttemptRecovery::Sealed { mut receipts })) => {
                    let receipt = receipts.remove(0);
                    store.verify_receipt(&receipt).unwrap();
                    let mut selected = Vec::new();
                    store.read_selected(&receipt, 64, &mut selected).unwrap();
                    assert_eq!(selected, b"phase crash bytes");
                }
                (_, Some(AttemptRecovery::Preparing { .. })) => {}
                (_, state) => panic!("{phase} recovered unexpected state: {state:?}"),
            }
        }
    }

    #[test]
    #[ignore = "run only as child of crash_barriers_never_recover_an_unsealed_receipt"]
    fn seal_at_phase_then_sigkill_child() {
        let root = PathBuf::from(env::var_os("TOS_SEGMENT_CRASH_TEST_ROOT").expect("child root"));
        let store = SegmentStore::initialize_empty(&root, b"private-domain", limits()).unwrap();
        let bytes = b"phase crash bytes".to_vec();
        let mut reader = Cursor::new(bytes.clone());
        let mut frames = [FrameInput {
            binding: binding(0),
            declared_size: bytes.len() as u64,
            declared_sha256: Digest256::of_bytes(&bytes),
            reader: &mut reader,
        }];
        let _ = store
            .seal_segment(b"phase-crash-prepare", &mut frames)
            .unwrap();
        panic!("test barrier did not terminate child");
    }

    #[test]
    fn fenced_intent_crash_barriers_reconcile_without_marker() {
        for phase in ["intent-synced", "pin-synced", "sealed-pin-synced"] {
            let root = PrivateRoot::new();
            let mut child = Command::new(env::current_exe().unwrap())
                .arg("--exact")
                .arg("store::tests::fenced_seal_at_phase_then_sigkill_child")
                .arg("--ignored")
                .env("TOS_SEGMENT_CRASH_TEST_ROOT", &root.0)
                .env("TOS_SEGMENT_CRASH_AT", phase)
                .spawn()
                .unwrap();
            let deadline = Instant::now() + Duration::from_secs(30);
            let status = loop {
                if let Some(status) = child.try_wait().unwrap() {
                    break status;
                }
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("{phase} fenced crash child exceeded 30-second bound");
                }
                std::thread::sleep(Duration::from_millis(10));
            };
            assert_eq!(status.signal(), Some(9), "{phase} did not SIGKILL");
            let store = SegmentStore::open_existing(&root.0, limits()).unwrap();
            match (
                phase,
                store
                    .recover_attempt_fenced(b"fenced-phase-prepare", 7, 0)
                    .unwrap(),
            ) {
                ("intent-synced", Some(AttemptRecovery::IntentOnly { .. })) => {}
                ("pin-synced", Some(AttemptRecovery::Preparing { .. })) => {}
                ("sealed-pin-synced", Some(AttemptRecovery::Sealed { receipts })) => {
                    assert_eq!(receipts.len(), 1);
                    let mut selected = Vec::new();
                    store
                        .read_selected(&receipts[0], 64, &mut selected)
                        .unwrap();
                    assert_eq!(selected, b"fenced phase bytes");
                }
                (_, state) => panic!("{phase} recovered unexpected fenced state: {state:?}"),
            }
        }
    }

    #[test]
    #[ignore = "run only as child of fenced_intent_crash_barriers_reconcile_without_marker"]
    fn fenced_seal_at_phase_then_sigkill_child() {
        let root = PathBuf::from(env::var_os("TOS_SEGMENT_CRASH_TEST_ROOT").expect("child root"));
        let store = SegmentStore::initialize_empty(&root, b"private-domain", limits()).unwrap();
        let bytes = b"fenced phase bytes".to_vec();
        let mut reader = Cursor::new(bytes.clone());
        let mut frames = [FrameInput {
            binding: binding(0),
            declared_size: bytes.len() as u64,
            declared_sha256: Digest256::of_bytes(&bytes),
            reader: &mut reader,
        }];
        let _ = store
            .seal_segment_fenced(b"fenced-phase-prepare", 7, 0, &mut frames)
            .unwrap();
        panic!("fenced test barrier did not terminate child");
    }
}
