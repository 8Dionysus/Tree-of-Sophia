//! Private, caller-selected workspace for one bounded CMD generation audit.
//! Files are unnamed Linux temporary inodes under the supplied directory:
//! no caller entry is replaced, and process death cannot leave an authoritative
//! partial run. These rows remain physical evidence, not a source certificate.

use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;
use std::fs::File;
use std::os::unix::fs::FileExt;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use rustix::fs::{Mode, OFlags, openat};
use tos_foundation::{Digest256, Digest256Hasher};
use tos_segment_store::{PlacementGenerationRowV1, PlacementV1};

use super::{DurableError, DurableResult};

const IO_BLOCK: usize = 64 * 1024;

#[derive(Clone, Copy, Debug)]
pub struct ColdWorkspaceLimits {
    pub max_scratch_written_bytes: u64,
    pub max_run_bytes: usize,
    pub max_runs: usize,
    pub merge_fan_in: usize,
    pub max_rows: u64,
    pub max_key_bytes: usize,
}

impl ColdWorkspaceLimits {
    fn check(self) -> DurableResult<Self> {
        if self.max_scratch_written_bytes == 0
            || self.max_scratch_written_bytes == u64::MAX
            || self.max_run_bytes < 512
            || self.max_runs == 0
            || self.merge_fan_in < 2
            || self.merge_fan_in > 64
            || self.max_rows == 0
            || self.max_rows == u64::MAX
            || self.max_key_bytes == 0
            || self.max_key_bytes > u32::MAX as usize
        {
            return Err(DurableError::Refused("invalid cold workspace limits"));
        }
        Ok(self)
    }
}

#[derive(Clone, Debug)]
pub struct PrivateGenerationWorkspace {
    directory: Arc<File>,
    limits: ColdWorkspaceLimits,
    written: Arc<Mutex<u64>>,
}

impl PrivateGenerationWorkspace {
    pub fn open(directory: &Path, limits: ColdWorkspaceLimits) -> DurableResult<Self> {
        let limits = limits.check()?;
        let directory = tos_fd_open::open_absolute_directory(directory)
            .map_err(|_| DurableError::Refused("unsafe cold workspace directory"))?;
        Ok(Self {
            directory: Arc::new(directory),
            limits,
            written: Arc::new(Mutex::new(0)),
        })
    }

    pub fn limits(&self) -> ColdWorkspaceLimits {
        self.limits
    }

    pub(crate) fn collector(&self) -> RunCollector {
        RunCollector {
            workspace: self.clone(),
            pending: Vec::new(),
            pending_bytes: 0,
            runs: Vec::new(),
            rows: 0,
        }
    }

    pub(crate) fn ordered_rows(&self) -> DurableResult<OrderedRows> {
        Ok(OrderedRows {
            writer: ScratchWriter::new(self)?,
            previous: Vec::new(),
            rows: 0,
            max_rows: self.limits.max_rows,
        })
    }

    fn new_file(&self) -> DurableResult<File> {
        // TMPFILE is deliberately required. Falling back to a named path
        // would make crash cleanup and caller ownership a different contract.
        openat(
            &*self.directory,
            ".",
            OFlags::TMPFILE | OFlags::RDWR | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )
        .map(File::from)
        .map_err(|_| DurableError::Refused("unnamed cold workspace file unavailable"))
    }

    fn charge(&self, bytes: usize) -> DurableResult<()> {
        let mut written = self
            .written
            .lock()
            .map_err(|_| DurableError::Corrupt("cold workspace accounting poisoned"))?;
        let next = written
            .checked_add(bytes as u64)
            .ok_or(DurableError::Refused("cold workspace byte count overflow"))?;
        if next > self.limits.max_scratch_written_bytes {
            return Err(DurableError::Refused("cold workspace byte budget exceeded"));
        }
        *written = next;
        Ok(())
    }
}

pub(crate) struct RunCollector {
    workspace: PrivateGenerationWorkspace,
    pending: Vec<PlacementGenerationRowV1>,
    pending_bytes: usize,
    runs: Vec<ScratchRows>,
    rows: u64,
}

pub(crate) struct OrderedRows {
    writer: ScratchWriter,
    previous: Vec<u8>,
    rows: u64,
    max_rows: u64,
}
impl OrderedRows {
    pub(crate) fn push(&mut self, row: PlacementGenerationRowV1) -> DurableResult<()> {
        if !self.previous.is_empty() && row.key <= self.previous {
            return Err(DurableError::Corrupt("ordered membership key differs"));
        }
        self.rows = self
            .rows
            .checked_add(1)
            .ok_or(DurableError::Refused("ordered membership count overflow"))?;
        if self.rows > self.max_rows {
            return Err(DurableError::Refused(
                "ordered membership row budget exceeded",
            ));
        }
        self.writer.push(&row)?;
        self.previous = row.key;
        Ok(())
    }
    pub(crate) fn finish(self) -> DurableResult<ScratchRows> {
        let result = self.writer.finish()?;
        if result.count != self.rows {
            return Err(DurableError::Corrupt("ordered membership count differs"));
        }
        Ok(result)
    }
}

impl RunCollector {
    pub(crate) fn push(&mut self, row: PlacementGenerationRowV1) -> DurableResult<()> {
        let size = wire_size(&row, self.workspace.limits.max_key_bytes)?;
        if size > self.workspace.limits.max_run_bytes {
            return Err(DurableError::Refused("membership row exceeds sort run"));
        }
        if self
            .pending_bytes
            .checked_add(size)
            .is_none_or(|next| next > self.workspace.limits.max_run_bytes)
        {
            self.flush()?;
        }
        self.rows = self
            .rows
            .checked_add(1)
            .ok_or(DurableError::Refused("membership row count overflow"))?;
        if self.rows > self.workspace.limits.max_rows {
            return Err(DurableError::Refused("membership row budget exceeded"));
        }
        self.pending_bytes += size;
        self.pending.push(row);
        Ok(())
    }

    fn flush(&mut self) -> DurableResult<()> {
        if self.pending.is_empty() {
            return Ok(());
        }
        if self.runs.len() >= self.workspace.limits.max_runs {
            return Err(DurableError::Refused("membership sort run budget exceeded"));
        }
        self.pending.sort_unstable_by(|a, b| a.key.cmp(&b.key));
        if self
            .pending
            .windows(2)
            .any(|pair| pair[0].key == pair[1].key)
        {
            return Err(DurableError::Corrupt("duplicate membership key"));
        }
        let mut writer = ScratchWriter::new(&self.workspace)?;
        for row in &self.pending {
            writer.push(row)?;
        }
        self.runs.push(writer.finish()?);
        self.pending.clear();
        self.pending_bytes = 0;
        Ok(())
    }

    pub(crate) fn finish(
        mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<ScratchRows> {
        check(deadline, cancelled)?;
        self.flush()?;
        if self.runs.is_empty() {
            return ScratchWriter::new(&self.workspace)?.finish();
        }
        while self.runs.len() > 1 {
            let mut next = Vec::new();
            for group in self.runs.chunks(self.workspace.limits.merge_fan_in) {
                check(deadline, cancelled)?;
                next.push(merge_group(&self.workspace, group, deadline, cancelled)?);
            }
            self.runs = next;
        }
        let result = self.runs.pop().expect("nonempty runs");
        if result.count != self.rows {
            return Err(DurableError::Corrupt("membership merge row count differs"));
        }
        Ok(result)
    }
}

fn merge_group(
    workspace: &PrivateGenerationWorkspace,
    group: &[ScratchRows],
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<ScratchRows> {
    let mut readers: Vec<_> = group.iter().map(ScratchRows::reader).collect();
    let mut queue = BinaryHeap::new();
    for (index, reader) in readers.iter_mut().enumerate() {
        if let Some(row) = reader.next()? {
            queue.push(Reverse(HeapRow { row, index }));
        }
    }
    let mut writer = ScratchWriter::new(workspace)?;
    let mut previous = Vec::new();
    while let Some(Reverse(HeapRow { row, index })) = queue.pop() {
        check(deadline, cancelled)?;
        if !previous.is_empty() && row.key <= previous {
            return Err(DurableError::Corrupt("membership merge key order differs"));
        }
        previous.clone_from(&row.key);
        writer.push(&row)?;
        if let Some(next) = readers[index].next()? {
            queue.push(Reverse(HeapRow { row: next, index }));
        }
    }
    for reader in &mut readers {
        reader.finish()?;
    }
    writer.finish()
}

fn check(deadline: Instant, cancelled: &AtomicBool) -> DurableResult<()> {
    if cancelled.load(AtomicOrdering::Relaxed) || Instant::now() >= deadline {
        Err(DurableError::Refused("cold workspace deadline exceeded"))
    } else {
        Ok(())
    }
}

struct HeapRow {
    row: PlacementGenerationRowV1,
    index: usize,
}
impl PartialEq for HeapRow {
    fn eq(&self, other: &Self) -> bool {
        self.row.key == other.row.key && self.index == other.index
    }
}
impl Eq for HeapRow {}
impl PartialOrd for HeapRow {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for HeapRow {
    fn cmp(&self, other: &Self) -> Ordering {
        self.row
            .key
            .cmp(&other.row.key)
            .then(self.index.cmp(&other.index))
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ScratchRows {
    file: Arc<File>,
    bytes: u64,
    count: u64,
    digest: Digest256,
    max_key_bytes: usize,
}
impl ScratchRows {
    pub(crate) fn count(&self) -> u64 {
        self.count
    }
    pub(crate) fn reader(&self) -> ScratchReader {
        ScratchReader {
            source: self.clone(),
            offset: 0,
            count: 0,
            digest: Digest256Hasher::new(),
            done: false,
            buffer: vec![0; IO_BLOCK],
            available: 0,
            position: 0,
        }
    }
}

struct ScratchWriter {
    file: File,
    workspace: PrivateGenerationWorkspace,
    offset: u64,
    count: u64,
    digest: Digest256Hasher,
    buffer: Vec<u8>,
}
impl ScratchWriter {
    fn new(workspace: &PrivateGenerationWorkspace) -> DurableResult<Self> {
        Ok(Self {
            file: workspace.new_file()?,
            workspace: workspace.clone(),
            offset: 0,
            count: 0,
            digest: Digest256Hasher::new(),
            buffer: Vec::with_capacity(IO_BLOCK),
        })
    }
    fn push(&mut self, row: &PlacementGenerationRowV1) -> DurableResult<()> {
        let size = wire_size(row, self.workspace.limits.max_key_bytes)?;
        let mut raw = Vec::with_capacity(size);
        raw.extend_from_slice(&(row.key.len() as u32).to_le_bytes());
        raw.extend_from_slice(&row.key);
        raw.extend_from_slice(row.logical_digest.as_bytes());
        raw.extend_from_slice(&row.logical_length.to_le_bytes());
        raw.extend_from_slice(&row.placement.encode());
        self.workspace.charge(raw.len())?;
        if self
            .buffer
            .len()
            .checked_add(raw.len())
            .is_none_or(|n| n > IO_BLOCK)
        {
            self.flush()?;
        }
        if raw.len() > IO_BLOCK {
            write_all_at(&self.file, &raw, self.offset)?;
        } else {
            self.buffer.extend_from_slice(&raw);
        }
        self.offset = self
            .offset
            .checked_add(raw.len() as u64)
            .ok_or(DurableError::Refused("workspace length overflow"))?;
        self.count += 1;
        self.digest.update(&raw);
        Ok(())
    }
    fn flush(&mut self) -> DurableResult<()> {
        if !self.buffer.is_empty() {
            let start = self.offset - self.buffer.len() as u64;
            write_all_at(&self.file, &self.buffer, start)?;
            self.buffer.clear();
        }
        Ok(())
    }
    fn finish(mut self) -> DurableResult<ScratchRows> {
        self.flush()?;
        // Unnamed scratch is process-local and re-created from a fresh audit
        // after a crash. Only final STO leaves/descriptors enter durability.
        Ok(ScratchRows {
            file: Arc::new(self.file),
            bytes: self.offset,
            count: self.count,
            digest: self.digest.finalize(),
            max_key_bytes: self.workspace.limits.max_key_bytes,
        })
    }
}

pub(crate) struct ScratchReader {
    source: ScratchRows,
    offset: u64,
    count: u64,
    digest: Digest256Hasher,
    done: bool,
    buffer: Vec<u8>,
    available: usize,
    position: usize,
}
impl ScratchReader {
    pub(crate) fn next(&mut self) -> DurableResult<Option<PlacementGenerationRowV1>> {
        if self.done {
            return Ok(None);
        }
        if self.count == self.source.count {
            self.finish()?;
            return Ok(None);
        }
        let mut length = [0u8; 4];
        self.read_exact(&mut length)?;
        let key_len = u32::from_le_bytes(length) as usize;
        if key_len == 0 || key_len > self.source.max_key_bytes {
            return Err(DurableError::Corrupt("workspace membership key malformed"));
        }
        let size = 4 + key_len + 32 + 8 + PlacementV1::ENCODED_BYTES;
        if self
            .offset
            .checked_add((size - 4) as u64)
            .is_none_or(|end| end > self.source.bytes)
        {
            return Err(DurableError::Corrupt("workspace membership row truncated"));
        }
        let mut raw = vec![0u8; size];
        raw[..4].copy_from_slice(&length);
        self.read_exact(&mut raw[4..])?;
        self.digest.update(&raw);
        self.count += 1;
        let key = raw[4..4 + key_len].to_vec();
        let at = 4 + key_len;
        let logical_digest = Digest256::from_bytes(raw[at..at + 32].try_into().expect("fixed"));
        let logical_length = u64::from_le_bytes(raw[at + 32..at + 40].try_into().expect("fixed"));
        let placement = PlacementV1::decode(&raw[at + 40..])?;
        if placement.coordinate().sha256 != logical_digest
            || placement.coordinate().size_bytes != logical_length
        {
            return Err(DurableError::Corrupt(
                "workspace membership placement differs",
            ));
        }
        Ok(Some(PlacementGenerationRowV1 {
            key,
            logical_digest,
            logical_length,
            placement,
        }))
    }
    fn read_exact(&mut self, mut destination: &mut [u8]) -> DurableResult<()> {
        while !destination.is_empty() {
            if self.position == self.available {
                if self.offset >= self.source.bytes {
                    return Err(DurableError::Corrupt("cold workspace premature EOF"));
                }
                let count = (self.source.bytes - self.offset).min(IO_BLOCK as u64) as usize;
                self.available = self
                    .source
                    .file
                    .read_at(&mut self.buffer[..count], self.offset)
                    .map_err(|_| DurableError::Refused("cold workspace read failed"))?;
                if self.available == 0 {
                    return Err(DurableError::Corrupt("cold workspace premature EOF"));
                }
                self.position = 0;
            }
            let count = destination.len().min(self.available - self.position);
            destination[..count]
                .copy_from_slice(&self.buffer[self.position..self.position + count]);
            self.position += count;
            self.offset += count as u64;
            destination = &mut destination[count..];
        }
        Ok(())
    }
    pub(crate) fn finish(&mut self) -> DurableResult<()> {
        if self.count != self.source.count
            || self.offset != self.source.bytes
            || self.digest.clone().finalize() != self.source.digest
        {
            return Err(DurableError::Corrupt("workspace membership EOF differs"));
        }
        self.done = true;
        Ok(())
    }
}

fn wire_size(row: &PlacementGenerationRowV1, max_key_bytes: usize) -> DurableResult<usize> {
    if row.key.is_empty()
        || row.key.len() > max_key_bytes
        || row.logical_digest != row.placement.coordinate().sha256
        || row.logical_length != row.placement.coordinate().size_bytes
    {
        return Err(DurableError::Corrupt("workspace membership row differs"));
    }
    Ok(4 + row.key.len() + 32 + 8 + PlacementV1::ENCODED_BYTES)
}

fn write_all_at(file: &File, mut raw: &[u8], mut offset: u64) -> DurableResult<()> {
    while !raw.is_empty() {
        let count = file
            .write_at(raw, offset)
            .map_err(|_| DurableError::Refused("cold workspace write failed"))?;
        if count == 0 {
            return Err(DurableError::Refused("cold workspace write returned zero"));
        }
        raw = &raw[count..];
        offset += count as u64;
    }
    Ok(())
}
