use std::collections::HashSet;
use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path};
use std::sync::Arc;

use rustix::fs::{
    AtFlags, FlockOperation, Mode, OFlags, ResolveFlags, flock, fsync, linkat, mkdirat, openat,
    openat2, renameat, unlinkat,
};
use rustix::io::Errno;
use tos_foundation::{Digest256, Digest256Hasher};

use crate::error::{Result, SegmentError, SegmentErrorCode as Code};
use crate::format::{self, FrameCoordinate, SegmentLimits};
use crate::journal::{JournalFrame, PinJournal, PinState};

const ROOT_MAGIC: &[u8; 8] = b"TOSROOT2";
const BLOCK_BYTES: usize = 64 * 1024;

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
    store_id: [u8; 16],
    domain: Vec<u8>,
    domain_digest: Digest256,
    limits: SegmentLimits,
}

#[derive(Clone, Debug)]
pub struct SegmentStore {
    inner: Arc<Inner>,
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

impl ByteDurabilityReceipt {
    pub fn pin_id(&self) -> [u8; 16] {
        self.pin_id
    }
    pub fn fence_epoch(&self) -> u64 {
        self.fence_epoch
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
        let limits = limits.validate()?;
        if domain.is_empty() || domain.len() > u16::MAX as usize {
            return Err(SegmentError::new(
                Code::InvalidRoot,
                "invalid custody domain encoding",
            ));
        }
        let root_fd = open_root(root)?;
        for name in ["staging", "segments", "pins"] {
            mkdirat(&root_fd, name, Mode::RUSR | Mode::WUSR | Mode::XUSR).map_err(|error| {
                SegmentError::io("cannot initialize segment directory", error.into())
            })?;
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
        let mut file = create_exclusive(&root_fd, "store.meta")?;
        file.write_all(&meta)
            .map_err(|error| SegmentError::io("cannot write store metadata", error))?;
        file.sync_all()
            .map_err(|error| SegmentError::io("cannot sync store metadata", error))?;
        fsync(&root_fd)
            .map_err(|error| SegmentError::io("cannot sync store directory", error.into()))?;
        let staging = open_directory(&root_fd, "staging")?;
        let segments = open_directory(&root_fd, "segments")?;
        let pins = open_directory(&root_fd, "pins")?;
        Ok(Self {
            inner: Arc::new(Inner {
                _root: root_fd,
                staging,
                segments,
                pins,
                store_id,
                domain: domain.to_vec(),
                domain_digest: Digest256::of_bytes(domain),
                limits,
            }),
        })
    }

    pub fn open_existing(root: &Path, limits: SegmentLimits) -> Result<Self> {
        let limits = limits.validate()?;
        let root_fd = open_root(root)?;
        let staging = open_directory(&root_fd, "staging")?;
        let segments = open_directory(&root_fd, "segments")?;
        let pins = open_directory(&root_fd, "pins")?;
        let mut meta = Vec::new();
        open_regular(&root_fd, "store.meta")?
            .take(65_594)
            .read_to_end(&mut meta)
            .map_err(|error| SegmentError::io("cannot read store metadata", error))?;
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

    /// Seal multiple bounded frames under one durable pin. All frames belong
    /// to the same CMD prepare ID but retain independent owner bindings.
    /// Every input must satisfy `FrameInput`'s finite-reader precondition.
    pub fn seal_segment(
        &self,
        prepare_id: &[u8],
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

    /// CMD's same-process durability gate. Only a crate-constructed handle
    /// can enter, and the pinned on-disk bytes are re-read in full at use.
    /// CMD retains the pin until its transaction commits or explicitly aborts.
    pub fn verify_receipt(&self, receipt: &ByteDurabilityReceipt) -> Result<()> {
        let journal = self.validated_receipt_journal(receipt)?;
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
        flock(&self.inner.pins, FlockOperation::LockExclusive)
            .map_err(|error| SegmentError::io("cannot lock pin directory", error.into()))?;
        let result = (|| {
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
        })();
        let unlock = flock(&self.inner.pins, FlockOperation::Unlock);
        if let Err(error) = unlock {
            return Err(SegmentError::io(
                "cannot unlock pin directory",
                error.into(),
            ));
        }
        result
    }

    /// A selected frame is checked before writing any byte to the caller sink.
    /// The owner adapter must still check current rights before disclosure.
    pub fn read_selected(
        &self,
        receipt: &ByteDurabilityReceipt,
        max_bytes: u64,
        sink: &mut impl Write,
    ) -> Result<u64> {
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

    fn validated_receipt_journal(&self, receipt: &ByteDurabilityReceipt) -> Result<PinJournal> {
        if !Arc::ptr_eq(&self.inner, &receipt.inner) {
            return Err(SegmentError::new(
                Code::InvalidReceipt,
                "receipt belongs to another store instance",
            ));
        }
        let journal = self.read_pin(receipt.pin_id)?;
        if journal.state != PinState::Sealed
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
        Ok(journal)
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
    if !path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::RootDir | Component::Normal(_)))
    {
        return Err(SegmentError::new(
            Code::InvalidRoot,
            "segment root must be absolute without parent components",
        ));
    }
    let relative = path
        .strip_prefix("/")
        .map_err(|_| SegmentError::new(Code::InvalidRoot, "segment root is not absolute"))?;
    if relative.as_os_str().is_empty() {
        return Err(SegmentError::new(
            Code::InvalidRoot,
            "filesystem root is not a segment store",
        ));
    }
    let anchor =
        File::open("/").map_err(|error| SegmentError::io("cannot open filesystem root", error))?;
    openat2(
        &anchor,
        relative,
        dir_flags(),
        Mode::empty(),
        resolve_flags(),
    )
    .map(File::from)
    .map_err(|error| {
        map_open(
            error,
            Code::InvalidRoot,
            "cannot securely open segment root",
        )
    })
}

fn dir_flags() -> OFlags {
    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW
}
fn resolve_flags() -> ResolveFlags {
    ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS
}

fn open_directory(parent: &File, name: &str) -> Result<File> {
    openat2(parent, name, dir_flags(), Mode::empty(), resolve_flags())
        .map(File::from)
        .map_err(|error| {
            map_open(
                error,
                Code::UnsafePath,
                "cannot securely open segment directory",
            )
        })
}

fn open_regular(parent: &File, name: &str) -> Result<File> {
    let flags = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK;
    let file: File = openat2(parent, name, flags, Mode::empty(), resolve_flags())
        .map(File::from)
        .map_err(|error| map_open(error, Code::UnsafePath, "cannot securely open segment file"))?;
    if !file
        .metadata()
        .map_err(|error| SegmentError::io("cannot stat opened segment file", error))?
        .is_file()
    {
        return Err(SegmentError::new(
            Code::UnsafePath,
            "opened segment file is not regular",
        ));
    }
    Ok(file)
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

fn map_open(error: Errno, unsafe_code: Code, detail: &'static str) -> SegmentError {
    if error == Errno::NOSYS {
        SegmentError::new(Code::UnsupportedPlatform, "Linux openat2 unavailable")
    } else if matches!(
        error,
        Errno::LOOP | Errno::NOTDIR | Errno::XDEV | Errno::AGAIN
    ) {
        SegmentError::new(unsafe_code, detail)
    } else {
        SegmentError::io(detail, error.into())
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

#[cfg(test)]
fn crash_test_barrier(phase: &str, pin_id: [u8; 16]) {
    use std::io::Write;

    if std::env::var("TOS_SEGMENT_CRASH_AT").ok().as_deref() != Some(phase) {
        return;
    }
    let marker = std::path::PathBuf::from(
        std::env::var_os("TOS_SEGMENT_CRASH_MARKER").expect("crash marker path"),
    );
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(marker)
        .expect("create crash marker");
    file.write_all(hex_id(pin_id).as_bytes())
        .expect("write crash pin ID");
    file.sync_all().expect("sync crash marker");
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
        let pin_text = fs::read_to_string(root.0.join("sealed-pin-id")).unwrap();
        let pin_id: [u8; 16] = std::array::from_fn(|index| {
            u8::from_str_radix(&pin_text[index * 2..index * 2 + 2], 16).unwrap()
        });
        let store = SegmentStore::open_existing(&root.0, limits()).unwrap();
        let receipt = store.recover_sealed(pin_id).unwrap().remove(0);
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
        let receipt = store
            .seal_segment(b"crash-prepare", &mut frames)
            .unwrap()
            .remove(0);
        let mut marker = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(root.join("sealed-pin-id"))
            .unwrap();
        marker
            .write_all(hex_id(receipt.pin_id()).as_bytes())
            .unwrap();
        marker.sync_all().unwrap();
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
            "pin-synced",
            "stage-synced",
            "segment-installed",
            "segments-dir-synced",
            "staging-dir-synced",
            "sealed-pin-synced",
        ] {
            let root = PrivateRoot::new();
            let marker = root.0.join("crash-pin-id");
            let mut child = Command::new(env::current_exe().unwrap())
                .arg("--exact")
                .arg("store::tests::seal_at_phase_then_sigkill_child")
                .arg("--ignored")
                .env("TOS_SEGMENT_CRASH_TEST_ROOT", &root.0)
                .env("TOS_SEGMENT_CRASH_AT", phase)
                .env("TOS_SEGMENT_CRASH_MARKER", &marker)
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
            let pin_text = fs::read_to_string(marker).unwrap();
            let pin_id: [u8; 16] = std::array::from_fn(|index| {
                u8::from_str_radix(&pin_text[index * 2..index * 2 + 2], 16).unwrap()
            });
            let store = SegmentStore::open_existing(&root.0, limits()).unwrap();
            if phase == "sealed-pin-synced" {
                let receipt = store.recover_sealed(pin_id).unwrap().remove(0);
                store.verify_receipt(&receipt).unwrap();
                let mut selected = Vec::new();
                store.read_selected(&receipt, 64, &mut selected).unwrap();
                assert_eq!(selected, b"phase crash bytes");
            } else {
                assert_eq!(
                    store.recover_sealed(pin_id).unwrap_err().code,
                    Code::InvalidReceipt,
                    "{phase} minted a premature receipt"
                );
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
}
