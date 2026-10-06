//! Source-owned packed payload extents for native V2 admission.
//!
//! The object tree is an authenticated revision root. Its value points into an
//! immutable, accounted segment frame; raw digest files remain the legacy
//! interpretation only.

use std::{
    cell::RefCell,
    collections::VecDeque,
    fs::File,
    io::{self, Read},
    mem::{size_of, size_of_val},
    os::unix::fs::FileExt,
    sync::{Arc, atomic::AtomicBool},
    time::Instant,
};
use tos_foundation::Digest256;
use tos_segment_store::{
    AuthenticatedTreeDeltaV1, AuthenticatedTreeDescriptorV2, AuthenticatedTreeEntryV1,
    AuthenticatedTreeIoLedgerV1, AuthenticatedTreeLimitsV1, AuthenticatedTreeWorkV1,
    ByteDurabilityReceipt, FrameInput, OwnerBinding, SegmentOperationLimitsV1,
    SegmentOperationWorkV1, SegmentStore,
};

use super::source_admission_segment_v2::OBJECT_EXTENTS_KIND;

const EXTENT_MAGIC: &[u8; 8] = b"TOSOBJX2";
const EXTENT_VERSION: u16 = 2;
const EXTENT_VALUE_BYTES: usize = 76;
const BLOCK_BYTES: u64 = 64 * 1024;
pub(crate) const MAX_PACKED_OBJECT_FRAMES_V2: u32 = 64;
const OBJECT_PROFILE_ID: &[u8] = b"tos-source-object-v2";
const OBJECT_PROFILE_VERSION: &[u8] = b"2";

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

/// Exact frame location committed by one revision's object extent tree.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PackedObjectLocationV2 {
    pub(crate) size: u64,
    pub(crate) segment_digest: Digest256,
    pub(crate) segment_size: u64,
    pub(crate) frame_index: u32,
    pub(crate) frame_count: u32,
    pub(crate) header_offset: u64,
}

impl PackedObjectLocationV2 {
    pub(crate) fn encode(self) -> io::Result<[u8; EXTENT_VALUE_BYTES]> {
        self.validate()?;
        let mut raw = [0u8; EXTENT_VALUE_BYTES];
        raw[..8].copy_from_slice(EXTENT_MAGIC);
        raw[8..10].copy_from_slice(&EXTENT_VERSION.to_le_bytes());
        raw[10..12].copy_from_slice(&0u16.to_le_bytes());
        raw[12..20].copy_from_slice(&self.size.to_be_bytes());
        raw[20..52].copy_from_slice(self.segment_digest.as_bytes());
        raw[52..60].copy_from_slice(&self.segment_size.to_be_bytes());
        raw[60..64].copy_from_slice(&self.frame_index.to_be_bytes());
        raw[64..68].copy_from_slice(&self.frame_count.to_be_bytes());
        raw[68..76].copy_from_slice(&self.header_offset.to_be_bytes());
        Ok(raw)
    }

    pub(crate) fn decode(raw: &[u8]) -> io::Result<Self> {
        if raw.len() != EXTENT_VALUE_BYTES
            || &raw[..8] != EXTENT_MAGIC
            || u16::from_le_bytes(
                raw[8..10]
                    .try_into()
                    .map_err(|_| invalid("extent version width differs"))?,
            ) != EXTENT_VERSION
            || raw[10..12] != [0, 0]
        {
            return Err(invalid("packed object extent wire differs"));
        }
        let result = Self {
            size: u64::from_be_bytes(
                raw[12..20]
                    .try_into()
                    .map_err(|_| invalid("extent size width differs"))?,
            ),
            segment_digest: Digest256::from_bytes(
                raw[20..52]
                    .try_into()
                    .map_err(|_| invalid("extent digest width differs"))?,
            ),
            segment_size: u64::from_be_bytes(
                raw[52..60]
                    .try_into()
                    .map_err(|_| invalid("extent segment size width differs"))?,
            ),
            frame_index: u32::from_be_bytes(
                raw[60..64]
                    .try_into()
                    .map_err(|_| invalid("extent frame index width differs"))?,
            ),
            frame_count: u32::from_be_bytes(
                raw[64..68]
                    .try_into()
                    .map_err(|_| invalid("extent frame count width differs"))?,
            ),
            header_offset: u64::from_be_bytes(
                raw[68..76]
                    .try_into()
                    .map_err(|_| invalid("extent offset width differs"))?,
            ),
        };
        result.validate()?;
        Ok(result)
    }

    fn validate(self) -> io::Result<()> {
        if self.segment_size == 0 || self.frame_count == 0 || self.frame_index >= self.frame_count {
            return Err(invalid("packed object frame coordinates differ"));
        }
        Ok(())
    }
}

/// One seekable, finite payload slice. Slice sources share a single held pack
/// descriptor and use positional reads, so a 100K-row cursor does not open
/// 100K files or disturb another row's file position.
pub(crate) enum PackedObjectSourceReaderV2 {
    File(File),
    Slice {
        file: Arc<File>,
        offset: u64,
        size: u64,
        position: u64,
    },
}

impl Read for PackedObjectSourceReaderV2 {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::File(file) => file.read(output),
            Self::Slice {
                file,
                offset,
                size,
                position,
            } => {
                if *position >= *size || output.is_empty() {
                    return Ok(0);
                }
                let wanted = usize::try_from((*size - *position).min(output.len() as u64))
                    .map_err(|_| invalid("packed source slice read range differs"))?;
                let at = offset
                    .checked_add(*position)
                    .ok_or_else(|| invalid("packed source slice offset overflow"))?;
                let read = file.read_at(&mut output[..wanted], at)?;
                *position = position
                    .checked_add(read as u64)
                    .ok_or_else(|| invalid("packed source slice cursor overflow"))?;
                Ok(read)
            }
        }
    }
}

/// Digest-ordered object row consumed by the full authenticated extent build.
/// Exactly one source form is present: a new finite payload, or a location
/// already authenticated by the selected base revision.
pub(crate) struct PackedObjectSourceV2 {
    pub(crate) digest: Digest256,
    pub(crate) size: u64,
    pub(crate) source: Option<PackedObjectSourceReaderV2>,
    pub(crate) existing: Option<PackedObjectLocationV2>,
}

impl PackedObjectSourceV2 {
    pub(crate) fn from_file(digest: Digest256, size: u64, file: File) -> Self {
        Self {
            digest,
            size,
            source: Some(PackedObjectSourceReaderV2::File(file)),
            existing: None,
        }
    }

    pub(crate) fn from_slice(digest: Digest256, size: u64, file: Arc<File>, offset: u64) -> Self {
        Self {
            digest,
            size,
            source: Some(PackedObjectSourceReaderV2::Slice {
                file,
                offset,
                size,
                position: 0,
            }),
            existing: None,
        }
    }

    pub(crate) fn retain_existing(
        digest: Digest256,
        size: u64,
        location: PackedObjectLocationV2,
    ) -> Self {
        Self {
            digest,
            size,
            source: None,
            existing: Some(location),
        }
    }
}

/// One digest-keyed COW change. `payload: None` removes an unreachable extent;
/// `Some` seals a new payload and installs its new extent value.
pub(crate) struct PackedObjectChangeV2 {
    pub(crate) digest: Digest256,
    pub(crate) payload: Option<PackedObjectSourceV2>,
}

impl PackedObjectChangeV2 {
    pub(crate) fn upsert(source: PackedObjectSourceV2) -> Self {
        Self {
            digest: source.digest,
            payload: Some(source),
        }
    }

    pub(crate) fn remove(digest: Digest256) -> Self {
        Self {
            digest,
            payload: None,
        }
    }

    pub(crate) fn delete(digest: Digest256) -> Self {
        Self::remove(digest)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct PackedObjectLimitsV2 {
    pub(crate) tree_limits: AuthenticatedTreeLimitsV1,
    /// Physical bounds selected by the enclosing invocation.
    pub(crate) segment_limits: tos_segment_store::SegmentLimits,
    pub(crate) max_working_state_bytes: usize,
    /// All retained caller-owned state live across object tree/frame work.
    pub(crate) caller_live_state_bytes: usize,
    /// Shared invocation work capacity; operation limits never exceed it.
    pub(crate) max_work_units: u64,
    /// Maximum unique extent rows admitted by the selected input/candidate.
    pub(crate) max_objects: u64,
    /// Maximum delta rows left in the enclosing invocation-wide tree-row
    /// budget; for readers and full builds this remains a finite structural
    /// ceiling.
    pub(crate) max_delta_rows: u64,
    /// Producer-selected maximum frames per immutable pack.
    pub(crate) max_pack_frames: u32,
}

impl PackedObjectLimitsV2 {
    fn validate(self) -> io::Result<Self> {
        self.segment_limits
            .validate()
            .map_err(|_| invalid("packed object segment limits differ"))?;
        if self.max_working_state_bytes == 0
            || self.max_working_state_bytes == usize::MAX
            || self.caller_live_state_bytes >= self.max_working_state_bytes
            || self.tree_limits.max_nodes == 0
            || self.tree_limits.max_nodes == u64::MAX
            || self.tree_limits.max_total_bytes == 0
            || self.tree_limits.max_total_bytes == u64::MAX
            || self.tree_limits.max_value_bytes < EXTENT_VALUE_BYTES
            || self.max_work_units == 0
            || self.max_work_units == u64::MAX
            || self.max_objects == u64::MAX
            || self.max_delta_rows == u64::MAX
            || self.max_pack_frames == 0
            || self.max_pack_frames == u32::MAX
        {
            return Err(invalid("packed object limits are not finite"));
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PackedObjectBuildWorkV2 {
    /// Standalone build work, or cumulative work for a COW apply.
    pub(crate) tree_work: AuthenticatedTreeWorkV1,
    pub(crate) segment_work: SegmentOperationWorkV1,
    pub(crate) object_rows: u64,
    pub(crate) payload_bytes: u64,
}

pub(crate) struct PackedObjectWriterV2;

impl PackedObjectWriterV2 {
    pub(crate) fn build<I>(
        segment: &SegmentStore,
        rows: I,
        limits: PackedObjectLimitsV2,
        io: Arc<dyn AuthenticatedTreeIoLedgerV1>,
        deadline: Instant,
        cancelled: &AtomicBool,
        debit_work: &mut dyn FnMut() -> bool,
    ) -> io::Result<(AuthenticatedTreeDescriptorV2, PackedObjectBuildWorkV2)>
    where
        I: IntoIterator<Item = io::Result<PackedObjectSourceV2>>,
    {
        let limits = limits.validate()?;
        if limits.max_pack_frames > limits.segment_limits.max_frames {
            return Err(invalid(
                "packed object pack-frame cap exceeds segment limits",
            ));
        }
        let batch_cap = frame_batch_cap(limits.segment_limits, limits)?;
        let batch_state = batch_state_bytes(batch_cap)?;
        // Tree callbacks and iterator-driven sealing run serially against the
        // same invocation debit; neither receives an independent work grant.
        let shared_debit = RefCell::new(debit_work);
        let mut frame_debit = || (*shared_debit.borrow_mut())();
        let mut tree_debit = || (*shared_debit.borrow_mut())();
        let batch_state = batch_state
            .checked_add(size_of_val(&shared_debit))
            .and_then(|bytes| bytes.checked_add(size_of_val(&frame_debit)))
            .and_then(|bytes| bytes.checked_add(size_of_val(&tree_debit)))
            .ok_or_else(|| invalid("packed callback state overflow"))?;
        let additional_live = limits
            .caller_live_state_bytes
            .checked_add(size_of::<PackedObjectBuildWorkV2>())
            .and_then(|bytes| bytes.checked_add(size_of::<ExtentTreeRows<'_, I::IntoIter>>()))
            .and_then(|bytes| bytes.checked_add(batch_state))
            .ok_or_else(|| invalid("packed object builder state overflow"))?;
        if additional_live >= limits.max_working_state_bytes {
            return Err(invalid("packed object builder exceeds selected state"));
        }
        let mut work = PackedObjectBuildWorkV2::default();
        let rows = ExtentTreeRows::new(
            segment.clone(),
            rows.into_iter(),
            limits,
            io.clone(),
            batch_cap,
            batch_state,
            deadline,
            cancelled,
            &mut frame_debit,
            &mut work,
        )?;
        let (descriptor, tree_work) = segment
            .build_authenticated_tree_v2_with_work_and_io_and_state_and_callback(
                OBJECT_EXTENTS_KIND,
                rows,
                limits.tree_limits,
                Some(io),
                limits.max_working_state_bytes,
                additional_live,
                deadline,
                cancelled,
                &mut tree_debit,
            )
            .map_err(|_| invalid("packed object extent tree build failed"))?;
        work.tree_work = tree_work;
        if descriptor.entries > limits.max_objects {
            return Err(invalid("packed object extent count exceeds selected cap"));
        }
        Ok((descriptor, work))
    }

    pub(crate) fn apply_delta<I>(
        segment: &SegmentStore,
        old: &AuthenticatedTreeDescriptorV2,
        changes: I,
        limits: PackedObjectLimitsV2,
        io: Arc<dyn AuthenticatedTreeIoLedgerV1>,
        initial_tree_work: AuthenticatedTreeWorkV1,
        deadline: Instant,
        cancelled: &AtomicBool,
        debit_work: &mut dyn FnMut() -> bool,
    ) -> io::Result<(AuthenticatedTreeDescriptorV2, PackedObjectBuildWorkV2)>
    where
        I: IntoIterator<Item = io::Result<PackedObjectChangeV2>>,
    {
        let limits = limits.validate()?;
        if limits.max_pack_frames > limits.segment_limits.max_frames {
            return Err(invalid(
                "packed object pack-frame cap exceeds segment limits",
            ));
        }
        if old.kind.as_slice() != OBJECT_EXTENTS_KIND {
            return Err(invalid("packed object delta root kind differs"));
        }
        let batch_cap = frame_batch_cap(limits.segment_limits, limits)?;
        let batch_state = batch_state_bytes(batch_cap)?;
        // Tree callbacks and iterator-driven sealing run serially against the
        // same invocation debit; neither receives an independent work grant.
        let shared_debit = RefCell::new(debit_work);
        let mut frame_debit = || (*shared_debit.borrow_mut())();
        let mut tree_debit = || (*shared_debit.borrow_mut())();
        let batch_state = batch_state
            .checked_add(size_of_val(&shared_debit))
            .and_then(|bytes| bytes.checked_add(size_of_val(&frame_debit)))
            .and_then(|bytes| bytes.checked_add(size_of_val(&tree_debit)))
            .ok_or_else(|| invalid("packed callback state overflow"))?;
        let additional_live = limits
            .caller_live_state_bytes
            .checked_add(size_of::<PackedObjectBuildWorkV2>())
            .and_then(|bytes| bytes.checked_add(size_of::<ExtentDeltaRows<'_, I::IntoIter>>()))
            .and_then(|bytes| bytes.checked_add(batch_state))
            .ok_or_else(|| invalid("packed object delta state overflow"))?;
        if additional_live >= limits.max_working_state_bytes {
            return Err(invalid("packed object delta exceeds selected state"));
        }
        let mut work = PackedObjectBuildWorkV2::default();
        let changes = ExtentDeltaRows::new(
            segment.clone(),
            changes.into_iter(),
            limits,
            io.clone(),
            batch_cap,
            batch_state,
            deadline,
            cancelled,
            &mut frame_debit,
            &mut work,
        )?;
        let (descriptor, tree_work) = segment
            .apply_authenticated_tree_delta_v2_with_work_and_io_and_state_cumulative_and_callback(
                old,
                changes,
                limits.tree_limits,
                Some(io),
                initial_tree_work,
                limits.max_working_state_bytes,
                additional_live,
                deadline,
                cancelled,
                &mut tree_debit,
            )
            .map_err(|_| invalid("packed object extent COW failed"))?;
        work.tree_work = tree_work;
        if descriptor.entries > limits.max_objects {
            return Err(invalid("packed object extent count exceeds selected cap"));
        }
        Ok((descriptor, work))
    }
}

fn frame_batch_cap(
    segment_limits: tos_segment_store::SegmentLimits,
    limits: PackedObjectLimitsV2,
) -> io::Result<usize> {
    let fixed = segment_limits
        .max_journal_bytes
        .checked_mul(3)
        .and_then(|bytes| bytes.checked_add(2 * 64 * 1024))
        .and_then(|bytes| bytes.checked_add(limits.caller_live_state_bytes))
        .ok_or_else(|| invalid("packed segment state bound overflow"))?;
    let per_frame = size_of::<PackedObjectSourceV2>()
        .checked_add(size_of::<ByteDurabilityReceipt>())
        .and_then(|bytes| bytes.checked_add(size_of::<FrameInput<'static>>()))
        .and_then(|bytes| bytes.checked_add(size_of::<AuthenticatedTreeEntryV1>() + 512))
        .ok_or_else(|| invalid("packed frame row state overflow"))?;
    let available = limits
        .max_working_state_bytes
        .checked_sub(fixed)
        .ok_or_else(|| invalid("packed segment fixed state exceeds selected state"))?;
    let count = usize::try_from(segment_limits.max_frames.min(limits.max_pack_frames))
        .map_err(|_| invalid("packed segment frame count exceeds address space"))?
        .min(available / per_frame);
    if count == 0 {
        return Err(invalid(
            "selected state cannot hold one packed object frame",
        ));
    }
    Ok(count)
}

fn batch_state_bytes(count: usize) -> io::Result<usize> {
    count
        .checked_mul(
            size_of::<PackedObjectSourceV2>()
                .checked_add(size_of::<ByteDurabilityReceipt>())
                .and_then(|bytes| bytes.checked_add(size_of::<FrameInput<'static>>()))
                .and_then(|bytes| bytes.checked_add(size_of::<AuthenticatedTreeEntryV1>() + 512))
                .ok_or_else(|| invalid("packed frame row state overflow"))?,
        )
        .ok_or_else(|| invalid("packed batch state overflow"))
}

struct ExtentTreeRows<'a, I> {
    segment: SegmentStore,
    rows: I,
    limits: PackedObjectLimitsV2,
    io: Arc<dyn AuthenticatedTreeIoLedgerV1>,
    batch_cap: usize,
    batch_state: usize,
    deadline: Instant,
    cancelled: &'a AtomicBool,
    debit_work: &'a mut dyn FnMut() -> bool,
    work: &'a mut PackedObjectBuildWorkV2,
    batch: Vec<PackedObjectSourceV2>,
    ready: VecDeque<AuthenticatedTreeEntryV1>,
    pending: Option<PackedObjectSourceV2>,
    last: Option<[u8; 32]>,
    seen_rows: u64,
    done: bool,
}

impl<'a, I> ExtentTreeRows<'a, I>
where
    I: Iterator<Item = io::Result<PackedObjectSourceV2>>,
{
    #[allow(clippy::too_many_arguments)]
    fn new(
        segment: SegmentStore,
        rows: I,
        limits: PackedObjectLimitsV2,
        io: Arc<dyn AuthenticatedTreeIoLedgerV1>,
        batch_cap: usize,
        batch_state: usize,
        deadline: Instant,
        cancelled: &'a AtomicBool,
        debit_work: &'a mut dyn FnMut() -> bool,
        work: &'a mut PackedObjectBuildWorkV2,
    ) -> io::Result<Self> {
        let mut batch = Vec::new();
        batch
            .try_reserve_exact(batch_cap)
            .map_err(|_| invalid("packed object batch reservation failed"))?;
        let mut ready = VecDeque::new();
        ready
            .try_reserve(batch_cap)
            .map_err(|_| invalid("packed object extent row reservation failed"))?;
        Ok(Self {
            segment,
            rows,
            limits,
            io,
            batch_cap,
            batch_state,
            deadline,
            cancelled,
            debit_work,
            work,
            batch,
            ready,
            pending: None,
            last: None,
            seen_rows: 0,
            done: false,
        })
    }

    fn next_source(&mut self) -> io::Result<Option<(PackedObjectSourceV2, bool)>> {
        if let Some(row) = self.pending.take() {
            return Ok(Some((row, true)));
        }
        match self.rows.next() {
            Some(row) => row.map(|row| Some((row, false))),
            None => Ok(None),
        }
    }

    fn accept_order(&mut self, source: &PackedObjectSourceV2) -> io::Result<()> {
        let key = *source.digest.as_bytes();
        if self.last.is_some_and(|prior| prior >= key) {
            return Err(invalid(
                "packed object rows are not strictly digest ordered",
            ));
        }
        self.seen_rows = self
            .seen_rows
            .checked_add(1)
            .filter(|rows| *rows <= self.limits.max_objects)
            .ok_or_else(|| invalid("packed object row count exceeds selected cap"))?;
        if source.source.is_some() == source.existing.is_some() {
            return Err(invalid("packed object row has ambiguous source"));
        }
        if let Some(location) = source.existing {
            if location.size != source.size || location.frame_count > self.limits.max_pack_frames {
                return Err(invalid("retained packed object size differs"));
            }
        } else if source.size > self.limits.segment_limits.max_frame_bytes {
            return Err(invalid("packed object exceeds selected frame size"));
        }
        self.last = Some(key);
        Ok(())
    }

    fn fill(&mut self) -> io::Result<()> {
        if self.done || !self.ready.is_empty() {
            return Ok(());
        }
        loop {
            let Some((mut source, was_checked)) = self.next_source()? else {
                self.done = true;
                if self.batch.is_empty() {
                    return Ok(());
                }
                self.seal_batch()?;
                return Ok(());
            };
            if !was_checked {
                self.accept_order(&source)?;
            }
            if let Some(location) = source.existing.take() {
                if !self.batch.is_empty() {
                    self.pending = Some(source);
                    self.seal_batch()?;
                    return Ok(());
                }
                self.push_extent(source.digest, location)?;
                return Ok(());
            }
            self.batch.push(source);
            if self.batch.len() == self.batch_cap {
                self.seal_batch()?;
                return Ok(());
            }
        }
    }

    fn seal_batch(&mut self) -> io::Result<()> {
        if self.batch.is_empty() {
            return Ok(());
        }
        let mut frames = Vec::new();
        frames
            .try_reserve_exact(self.batch.len())
            .map_err(|_| invalid("packed frame descriptor allocation failed"))?;
        for (slot, source) in self.batch.iter_mut().enumerate() {
            let reader = source
                .source
                .as_mut()
                .ok_or_else(|| invalid("new packed object payload is absent"))?;
            frames.push(FrameInput {
                binding: OwnerBinding {
                    profile_id: OBJECT_PROFILE_ID.to_vec(),
                    profile_version: OBJECT_PROFILE_VERSION.to_vec(),
                    subject_key: source.digest.as_bytes().to_vec(),
                    member_slot: u32::try_from(slot)
                        .map_err(|_| invalid("packed frame slot exceeds range"))?,
                },
                declared_size: source.size,
                declared_sha256: source.digest,
                reader,
            });
        }
        let operation_limits = seal_operation_limits(
            self.limits.segment_limits,
            self.limits,
            self.batch_state,
            self.work.segment_work.work_units,
        )?;
        let prepare_id = random_prepare_id(&self.io)?;
        let mut segment_work = SegmentOperationWorkV1::default();
        let receipts = self
            .segment
            .seal_segment_accounted(
                &prepare_id,
                &mut frames,
                self.io.clone(),
                operation_limits,
                self.deadline,
                self.cancelled,
                self.debit_work,
                &mut segment_work,
            )
            .map_err(|_| invalid("packed object segment seal failed"))?;
        add_segment_work(&mut self.work.segment_work, segment_work)?;
        if self.work.segment_work.work_units > self.limits.max_work_units {
            return Err(invalid("packed object writer work-unit cap exceeded"));
        }
        if receipts.len() != self.batch.len() {
            return Err(invalid("packed segment receipt count differs"));
        }
        let frame_count = u32::try_from(self.batch.len())
            .map_err(|_| invalid("packed segment frame count exceeds range"))?;
        for (slot, (source, receipt)) in self.batch.drain(..).zip(receipts.iter()).enumerate() {
            validate_receipt(&source, receipt, slot, frame_count)?;
            let location = PackedObjectLocationV2 {
                size: source.size,
                segment_digest: receipt.segment_digest(),
                segment_size: receipt.segment_size(),
                frame_index: receipt.frame_index(),
                frame_count,
                header_offset: receipt.coordinate().header_offset,
            };
            Self::push_extent_row(
                &mut self.ready,
                self.batch_cap,
                self.work,
                source.digest,
                location,
            )?;
            self.work.payload_bytes = self
                .work
                .payload_bytes
                .checked_add(source.size)
                .ok_or_else(|| invalid("packed object payload byte count overflow"))?;
        }
        Ok(())
    }

    fn push_extent(
        &mut self,
        digest: Digest256,
        location: PackedObjectLocationV2,
    ) -> io::Result<()> {
        Self::push_extent_row(&mut self.ready, self.batch_cap, self.work, digest, location)
    }

    fn push_extent_row(
        ready: &mut VecDeque<AuthenticatedTreeEntryV1>,
        batch_cap: usize,
        work: &mut PackedObjectBuildWorkV2,
        digest: Digest256,
        location: PackedObjectLocationV2,
    ) -> io::Result<()> {
        let value = location.encode()?.to_vec();
        ready.push_back(AuthenticatedTreeEntryV1 {
            key: digest.as_bytes().to_vec(),
            value,
        });
        if ready.len() > batch_cap {
            return Err(invalid("packed extent row buffer exceeds selected state"));
        }
        work.object_rows = work
            .object_rows
            .checked_add(1)
            .ok_or_else(|| invalid("packed object row count overflow"))?;
        Ok(())
    }
}

impl<I> Iterator for ExtentTreeRows<'_, I>
where
    I: Iterator<Item = io::Result<PackedObjectSourceV2>>,
{
    type Item = tos_segment_store::Result<AuthenticatedTreeEntryV1>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(row) = self.ready.pop_front() {
            return Some(Ok(row));
        }
        match self.fill() {
            Err(_) => Some(Err(tos_segment_store::SegmentError::new(
                tos_segment_store::SegmentErrorCode::InvalidFormat,
                "packed object extent source failed",
            ))),
            Ok(()) => self.ready.pop_front().map(Ok),
        }
    }
}

struct ExtentDeltaRows<'a, I> {
    segment: SegmentStore,
    changes: I,
    limits: PackedObjectLimitsV2,
    io: Arc<dyn AuthenticatedTreeIoLedgerV1>,
    batch_cap: usize,
    batch_state: usize,
    deadline: Instant,
    cancelled: &'a AtomicBool,
    debit_work: &'a mut dyn FnMut() -> bool,
    work: &'a mut PackedObjectBuildWorkV2,
    batch: Vec<PackedObjectSourceV2>,
    ready: VecDeque<AuthenticatedTreeDeltaV1>,
    pending: Option<PackedObjectChangeV2>,
    last: Option<[u8; 32]>,
    seen_rows: u64,
    done: bool,
}

impl<'a, I> ExtentDeltaRows<'a, I>
where
    I: Iterator<Item = io::Result<PackedObjectChangeV2>>,
{
    #[allow(clippy::too_many_arguments)]
    fn new(
        segment: SegmentStore,
        changes: I,
        limits: PackedObjectLimitsV2,
        io: Arc<dyn AuthenticatedTreeIoLedgerV1>,
        batch_cap: usize,
        batch_state: usize,
        deadline: Instant,
        cancelled: &'a AtomicBool,
        debit_work: &'a mut dyn FnMut() -> bool,
        work: &'a mut PackedObjectBuildWorkV2,
    ) -> io::Result<Self> {
        let mut batch = Vec::new();
        batch
            .try_reserve_exact(batch_cap)
            .map_err(|_| invalid("packed delta batch reservation failed"))?;
        let mut ready = VecDeque::new();
        ready
            .try_reserve(batch_cap)
            .map_err(|_| invalid("packed delta row reservation failed"))?;
        Ok(Self {
            segment,
            changes,
            limits,
            io,
            batch_cap,
            batch_state,
            deadline,
            cancelled,
            debit_work,
            work,
            batch,
            ready,
            pending: None,
            last: None,
            seen_rows: 0,
            done: false,
        })
    }

    fn next_change(&mut self) -> io::Result<Option<(PackedObjectChangeV2, bool)>> {
        if let Some(change) = self.pending.take() {
            return Ok(Some((change, true)));
        }
        match self.changes.next() {
            Some(change) => change.map(|change| Some((change, false))),
            None => Ok(None),
        }
    }

    fn fill(&mut self) -> io::Result<()> {
        if self.done || !self.ready.is_empty() {
            return Ok(());
        }
        loop {
            let Some((change, was_checked)) = self.next_change()? else {
                self.done = true;
                if !self.batch.is_empty() {
                    self.seal_batch()?;
                }
                return Ok(());
            };
            let key = *change.digest.as_bytes();
            if !was_checked {
                if self.last.is_some_and(|prior| prior >= key) {
                    return Err(invalid(
                        "packed object delta rows are not strictly digest ordered",
                    ));
                }
                self.last = Some(key);
                self.seen_rows = self
                    .seen_rows
                    .checked_add(1)
                    .filter(|rows| *rows <= self.limits.max_delta_rows)
                    .ok_or_else(|| invalid("packed object delta row count exceeds selected cap"))?;
            }
            match change.payload {
                None => {
                    if !self.batch.is_empty() {
                        self.pending = Some(PackedObjectChangeV2 {
                            digest: change.digest,
                            payload: None,
                        });
                        self.seal_batch()?;
                        return Ok(());
                    }
                    self.ready.push_back(AuthenticatedTreeDeltaV1 {
                        key: key.to_vec(),
                        value: None,
                    });
                    self.work.object_rows = self
                        .work
                        .object_rows
                        .checked_add(1)
                        .ok_or_else(|| invalid("packed delta row count overflow"))?;
                    return Ok(());
                }
                Some(source) => {
                    if source.digest != change.digest
                        || source.existing.is_some()
                        || source.source.is_none()
                        || source.size > self.limits.segment_limits.max_frame_bytes
                    {
                        return Err(invalid("packed object upsert payload differs"));
                    }
                    self.batch.push(source);
                    if self.batch.len() == self.batch_cap {
                        self.seal_batch()?;
                        return Ok(());
                    }
                }
            }
        }
    }

    fn seal_batch(&mut self) -> io::Result<()> {
        if self.batch.is_empty() {
            return Ok(());
        }
        let mut frames = Vec::new();
        frames
            .try_reserve_exact(self.batch.len())
            .map_err(|_| invalid("packed delta frame allocation failed"))?;
        for (slot, source) in self.batch.iter_mut().enumerate() {
            let reader = source
                .source
                .as_mut()
                .ok_or_else(|| invalid("packed delta payload is absent"))?;
            frames.push(FrameInput {
                binding: OwnerBinding {
                    profile_id: OBJECT_PROFILE_ID.to_vec(),
                    profile_version: OBJECT_PROFILE_VERSION.to_vec(),
                    subject_key: source.digest.as_bytes().to_vec(),
                    member_slot: u32::try_from(slot)
                        .map_err(|_| invalid("packed delta slot exceeds range"))?,
                },
                declared_size: source.size,
                declared_sha256: source.digest,
                reader,
            });
        }
        let operation_limits = seal_operation_limits(
            self.limits.segment_limits,
            self.limits,
            self.batch_state,
            self.work.segment_work.work_units,
        )?;
        let prepare_id = random_prepare_id(&self.io)?;
        let mut segment_work = SegmentOperationWorkV1::default();
        let receipts = self
            .segment
            .seal_segment_accounted(
                &prepare_id,
                &mut frames,
                self.io.clone(),
                operation_limits,
                self.deadline,
                self.cancelled,
                self.debit_work,
                &mut segment_work,
            )
            .map_err(|_| invalid("packed object delta segment seal failed"))?;
        add_segment_work(&mut self.work.segment_work, segment_work)?;
        if self.work.segment_work.work_units > self.limits.max_work_units {
            return Err(invalid("packed object delta work-unit cap exceeded"));
        }
        if receipts.len() != self.batch.len() {
            return Err(invalid("packed delta receipt count differs"));
        }
        let frame_count = u32::try_from(self.batch.len())
            .map_err(|_| invalid("packed delta frame count exceeds range"))?;
        for (slot, (source, receipt)) in self.batch.drain(..).zip(receipts.iter()).enumerate() {
            validate_receipt(&source, receipt, slot, frame_count)?;
            let location = PackedObjectLocationV2 {
                size: source.size,
                segment_digest: receipt.segment_digest(),
                segment_size: receipt.segment_size(),
                frame_index: receipt.frame_index(),
                frame_count,
                header_offset: receipt.coordinate().header_offset,
            };
            self.ready.push_back(AuthenticatedTreeDeltaV1 {
                key: source.digest.as_bytes().to_vec(),
                value: Some(location.encode()?.to_vec()),
            });
            self.work.object_rows = self
                .work
                .object_rows
                .checked_add(1)
                .ok_or_else(|| invalid("packed delta row count overflow"))?;
            self.work.payload_bytes = self
                .work
                .payload_bytes
                .checked_add(source.size)
                .ok_or_else(|| invalid("packed delta payload count overflow"))?;
        }
        Ok(())
    }
}

impl<I> Iterator for ExtentDeltaRows<'_, I>
where
    I: Iterator<Item = io::Result<PackedObjectChangeV2>>,
{
    type Item = tos_segment_store::Result<AuthenticatedTreeDeltaV1>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(row) = self.ready.pop_front() {
            return Some(Ok(row));
        }
        match self.fill() {
            Err(_) => Some(Err(tos_segment_store::SegmentError::new(
                tos_segment_store::SegmentErrorCode::InvalidFormat,
                "packed object delta source failed",
            ))),
            Ok(()) => self.ready.pop_front().map(Ok),
        }
    }
}

fn validate_receipt(
    source: &PackedObjectSourceV2,
    receipt: &ByteDurabilityReceipt,
    slot: usize,
    frame_count: u32,
) -> io::Result<()> {
    if receipt.binding().profile_id.as_slice() != OBJECT_PROFILE_ID
        || receipt.binding().profile_version.as_slice() != OBJECT_PROFILE_VERSION
        || receipt.binding().subject_key.as_slice() != source.digest.as_bytes()
        || receipt.binding().member_slot as usize != slot
        || receipt.coordinate().sha256 != source.digest
        || receipt.coordinate().size_bytes != source.size
        || receipt.frame_index() as usize != slot
        || frame_count == 0
        || receipt.segment_size() == 0
    {
        return Err(invalid("packed object segment receipt differs"));
    }
    Ok(())
}

fn seal_operation_limits(
    segment: tos_segment_store::SegmentLimits,
    limits: PackedObjectLimitsV2,
    batch_state: usize,
    already_used_work_units: u64,
) -> io::Result<SegmentOperationLimitsV1> {
    let caller_live_state_bytes = limits
        .caller_live_state_bytes
        .checked_add(batch_state)
        .ok_or_else(|| invalid("packed segment caller state overflow"))?;
    if caller_live_state_bytes >= limits.max_working_state_bytes {
        return Err(invalid(
            "packed segment caller state exceeds selected slice",
        ));
    }
    let max_work_bytes = segment
        .max_segment_bytes
        .checked_mul(4)
        .and_then(|bytes| {
            u64::try_from(segment.max_journal_bytes)
                .ok()?
                .checked_mul(8)
                .and_then(|journal| bytes.checked_add(journal))
        })
        .filter(|bytes| *bytes > 0 && *bytes < u64::MAX)
        .ok_or_else(|| invalid("packed segment work-byte bound overflow"))?;
    let chunks = segment
        .max_segment_bytes
        .checked_add(BLOCK_BYTES - 1)
        .and_then(|bytes| bytes.checked_div(BLOCK_BYTES))
        .and_then(|bytes| bytes.checked_mul(8))
        .ok_or_else(|| invalid("packed segment work-unit bound overflow"))?;
    let operation_work_units = chunks
        .checked_add(
            u64::from(segment.max_frames)
                .checked_mul(16)
                .ok_or_else(|| invalid("packed frame work bound overflow"))?,
        )
        .and_then(|units| units.checked_add(128))
        .filter(|units| *units > 0 && *units < u64::MAX)
        .ok_or_else(|| invalid("packed segment work-unit bound overflow"))?;
    let max_work_units = limits
        .max_work_units
        .checked_sub(already_used_work_units)
        .filter(|units| *units > 0)
        .map(|remaining| remaining.min(operation_work_units))
        .ok_or_else(|| invalid("packed segment shared work-unit budget exhausted"))?;
    Ok(SegmentOperationLimitsV1 {
        max_working_state_bytes: limits.max_working_state_bytes,
        caller_live_state_bytes,
        max_work_bytes,
        max_work_units,
    })
}

fn random_prepare_id(io: &Arc<dyn AuthenticatedTreeIoLedgerV1>) -> io::Result<[u8; 16]> {
    let mut random = File::open("/dev/urandom")?;
    if !io.charge_read(16) {
        return Err(invalid("packed prepare ID entropy charge refused"));
    }
    let mut id = [0u8; 16];
    random.read_exact(&mut id)?;
    if !io.record_read_returned(16) {
        return Err(invalid("packed prepare ID entropy accounting refused"));
    }
    Ok(id)
}

fn add_segment_work(
    into: &mut SegmentOperationWorkV1,
    next: SegmentOperationWorkV1,
) -> io::Result<()> {
    into.read_bytes = into
        .read_bytes
        .checked_add(next.read_bytes)
        .ok_or_else(|| invalid("packed segment read count overflow"))?;
    into.read_upper_bound_bytes = into
        .read_upper_bound_bytes
        .checked_add(next.read_upper_bound_bytes)
        .ok_or_else(|| invalid("packed segment read-guard count overflow"))?;
    into.write_bytes = into
        .write_bytes
        .checked_add(next.write_bytes)
        .ok_or_else(|| invalid("packed segment write count overflow"))?;
    into.allocation_reserved_bytes = into
        .allocation_reserved_bytes
        .checked_add(next.allocation_reserved_bytes)
        .ok_or_else(|| invalid("packed segment allocation reservation overflow"))?;
    into.allocated_bytes = into
        .allocated_bytes
        .checked_add(next.allocated_bytes)
        .ok_or_else(|| invalid("packed segment allocation overflow"))?;
    into.work_units = into
        .work_units
        .checked_add(next.work_units)
        .ok_or_else(|| invalid("packed segment work count overflow"))?;
    into.work_bytes = into
        .work_bytes
        .checked_add(next.work_bytes)
        .ok_or_else(|| invalid("packed segment work bytes overflow"))?;
    Ok(())
}

/// Authenticated object-index reader. Tree lookup work and physical frame IO
/// share the original ledger, deadline, and caller work debit.
pub(crate) struct PackedObjectReaderV2<'a> {
    segment: &'a SegmentStore,
    descriptor: &'a AuthenticatedTreeDescriptorV2,
    limits: PackedObjectLimitsV2,
    io: Arc<dyn AuthenticatedTreeIoLedgerV1>,
    deadline: Instant,
    cancelled: &'a AtomicBool,
    caller_retained_state_bytes: usize,
    debit_work: &'a mut dyn FnMut() -> bool,
    tree_work: AuthenticatedTreeWorkV1,
    segment_work: SegmentOperationWorkV1,
    last_lookup: Option<(Digest256, PackedObjectLocationV2)>,
}

impl<'a> PackedObjectReaderV2<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        segment: &'a SegmentStore,
        descriptor: &'a AuthenticatedTreeDescriptorV2,
        limits: PackedObjectLimitsV2,
        io: Arc<dyn AuthenticatedTreeIoLedgerV1>,
        deadline: Instant,
        cancelled: &'a AtomicBool,
        additional_caller_state_bytes: usize,
        debit_work: &'a mut dyn FnMut() -> bool,
    ) -> io::Result<Self> {
        let limits = limits.validate()?;
        if descriptor.kind.as_slice() != OBJECT_EXTENTS_KIND {
            return Err(invalid("packed object descriptor kind differs"));
        }
        if descriptor.entries > limits.max_objects
            || limits.max_pack_frames > limits.segment_limits.max_frames
        {
            return Err(invalid("packed object root exceeds selected cardinality"));
        }
        let caller_retained_state_bytes = limits
            .caller_live_state_bytes
            .checked_add(additional_caller_state_bytes)
            .ok_or_else(|| invalid("packed object reader state overflow"))?;
        let fixed_lookup = limits
            .tree_limits
            .max_node_bytes
            .checked_mul(64)
            .and_then(|bytes| {
                usize::try_from(limits.tree_limits.max_value_bytes)
                    .ok()?
                    .checked_mul(4)
                    .and_then(|value_bytes| bytes.checked_add(value_bytes))
            })
            .and_then(|bytes| bytes.checked_add(2048))
            .ok_or_else(|| invalid("packed object lookup state overflow"))?;
        if caller_retained_state_bytes
            .checked_add(fixed_lookup)
            .is_none_or(|bytes| bytes > limits.max_working_state_bytes)
        {
            return Err(invalid("packed object reader exceeds selected state"));
        }
        Ok(Self {
            segment,
            descriptor,
            limits,
            io,
            deadline,
            cancelled,
            caller_retained_state_bytes,
            debit_work,
            tree_work: AuthenticatedTreeWorkV1::default(),
            segment_work: SegmentOperationWorkV1::default(),
            last_lookup: None,
        })
    }

    pub(crate) fn lookup(
        &mut self,
        digest: Digest256,
        expected_size: Option<u64>,
    ) -> io::Result<Option<PackedObjectLocationV2>> {
        self.lookup_with_work(digest, expected_size)
            .map(|(location, _)| location)
    }

    pub(crate) fn lookup_with_work(
        &mut self,
        digest: Digest256,
        expected_size: Option<u64>,
    ) -> io::Result<(Option<PackedObjectLocationV2>, AuthenticatedTreeWorkV1)> {
        let row_bytes = self
            .limits
            .tree_limits
            .max_node_bytes
            .checked_mul(64)
            .and_then(|bytes| bytes.checked_add(EXTENT_VALUE_BYTES * 4 + 2048))
            .ok_or_else(|| invalid("packed object extent lookup state overflow"))?;
        if self
            .limits
            .max_working_state_bytes
            .checked_sub(self.caller_retained_state_bytes)
            .is_none_or(|bytes| bytes < row_bytes)
        {
            return Err(invalid("packed object extent lookup state exhausted"));
        }
        let mut tree_limits = self.limits.tree_limits;
        let used_nodes = self
            .tree_work
            .read_nodes
            .checked_add(self.tree_work.written_nodes)
            .ok_or_else(|| invalid("packed object cumulative node count overflow"))?;
        let used_bytes = self
            .tree_work
            .read_bytes
            .checked_add(self.tree_work.written_bytes)
            .ok_or_else(|| invalid("packed object cumulative tree byte count overflow"))?;
        tree_limits.max_nodes = tree_limits
            .max_nodes
            .checked_sub(used_nodes)
            .filter(|nodes| *nodes > 0)
            .ok_or_else(|| invalid("packed object cumulative node limit exhausted"))?;
        tree_limits.max_total_bytes = tree_limits
            .max_total_bytes
            .checked_sub(used_bytes)
            .filter(|bytes| *bytes > 0)
            .ok_or_else(|| invalid("packed object cumulative tree byte limit exhausted"))?;
        let (value, work) = self
            .segment
            .lookup_authenticated_tree_v2_with_work_and_io_and_callback(
                self.descriptor,
                digest.as_bytes(),
                tree_limits,
                Some(self.io.clone()),
                self.deadline,
                self.cancelled,
                self.debit_work,
            )
            .map_err(|_| invalid("packed object extent lookup failed"))?;
        let location = value
            .as_deref()
            .map(PackedObjectLocationV2::decode)
            .transpose()?;
        if location.is_some_and(|location| {
            location.size > self.limits.segment_limits.max_frame_bytes
                || location.frame_count > self.limits.max_pack_frames
                || expected_size.is_some_and(|size| size != location.size)
        }) {
            return Err(invalid("packed object extent size differs"));
        }
        self.tree_work = add_tree_work(self.tree_work, work)?;
        self.last_lookup = location.map(|found| (digest, found));
        Ok((location, work))
    }

    pub(crate) fn tree_work(&self) -> AuthenticatedTreeWorkV1 {
        self.tree_work
    }

    pub(crate) fn read_exact(
        &mut self,
        location: &PackedObjectLocationV2,
        digest: Digest256,
        size: u64,
        sink: &mut dyn io::Write,
    ) -> io::Result<SegmentOperationWorkV1> {
        self.read_exact_with_caller_state(location, digest, size, 0, sink)
    }

    pub(crate) fn read_exact_with_caller_state(
        &mut self,
        location: &PackedObjectLocationV2,
        digest: Digest256,
        size: u64,
        caller_retained_state_bytes: usize,
        sink: &mut dyn io::Write,
    ) -> io::Result<SegmentOperationWorkV1> {
        if location.size != size
            || size > self.limits.segment_limits.max_frame_bytes
            || location.frame_count > self.limits.max_pack_frames
            || self.last_lookup != Some((digest, *location))
        {
            return Err(invalid("packed object frame size differs"));
        }
        self.last_lookup = None;
        let reader_state = size_of::<Self>()
            .checked_add(size_of::<PackedObjectLocationV2>() + EXTENT_VALUE_BYTES * 4)
            .and_then(|bytes| bytes.checked_add(self.caller_retained_state_bytes))
            .and_then(|bytes| bytes.checked_add(caller_retained_state_bytes))
            .ok_or_else(|| invalid("packed object frame state overflow"))?;
        let max_work_units = self
            .limits
            .max_work_units
            .checked_sub(self.segment_work.work_units)
            .filter(|units| *units > 0)
            .ok_or_else(|| invalid("packed object shared work-unit budget exhausted"))?;
        let operation_limits = SegmentOperationLimitsV1 {
            max_working_state_bytes: self.limits.max_working_state_bytes,
            caller_live_state_bytes: reader_state,
            max_work_bytes: size
                .checked_add(512)
                .filter(|bytes| *bytes > 0 && *bytes < u64::MAX)
                .ok_or_else(|| invalid("packed frame byte-work bound overflow"))?,
            max_work_units,
        };
        let mut work = SegmentOperationWorkV1::default();
        self.segment
            .read_packed_frame_accounted(
                location.segment_digest,
                location.segment_size,
                location.header_offset,
                digest,
                size,
                location.frame_index,
                location.frame_count,
                size,
                self.io.clone(),
                operation_limits,
                self.deadline,
                self.cancelled,
                self.debit_work,
                &mut work,
                sink,
            )
            .map_err(|_| invalid("packed object frame verification failed"))?;
        add_segment_work(&mut self.segment_work, work)?;
        if self.segment_work.work_units > self.limits.max_work_units {
            return Err(invalid("packed object shared work-unit budget exceeded"));
        }
        Ok(work)
    }
}

fn add_tree_work(
    mut current: AuthenticatedTreeWorkV1,
    next: AuthenticatedTreeWorkV1,
) -> io::Result<AuthenticatedTreeWorkV1> {
    current.read_nodes = current
        .read_nodes
        .checked_add(next.read_nodes)
        .ok_or_else(|| invalid("packed object tree node overflow"))?;
    current.written_nodes = current
        .written_nodes
        .checked_add(next.written_nodes)
        .ok_or_else(|| invalid("packed object tree node overflow"))?;
    current.read_bytes = current
        .read_bytes
        .checked_add(next.read_bytes)
        .ok_or_else(|| invalid("packed object tree byte overflow"))?;
    current.written_bytes = current
        .written_bytes
        .checked_add(next.written_bytes)
        .ok_or_else(|| invalid("packed object tree byte overflow"))?;
    current.allocated_bytes = current
        .allocated_bytes
        .checked_add(next.allocated_bytes)
        .ok_or_else(|| invalid("packed object tree allocation overflow"))?;
    Ok(current)
}
