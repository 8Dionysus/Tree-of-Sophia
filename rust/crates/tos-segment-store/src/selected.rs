//! Immutable CMD-bound physical generation descriptors. A descriptor names
//! exact history/current leaves; only CMD can certify an exhaustive source
//! cut and atomically select its digest in coordinator metadata.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use tos_foundation::{Digest256, Digest256Hasher};

use crate::error::{Result, SegmentError, SegmentErrorCode as Code};
use crate::generation::{
    GenerationShapeLimits, KeyComparatorV1, PackedPartitionRefV1, PartitionBoundsV1,
    PlacementGenerationRowV1, PlacementPartitionV1, placement_catalog_shape_root,
};
use crate::store::SegmentStore;

const MAGIC: &[u8; 8] = b"TOSGEN1\0";
const PROFILE: &[u8; 24] = b"complete-private-cmd2-v1";
const NONE: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GenerationNamespaceV1 {
    History,
    Current,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerationCutV1 {
    pub store_id: [u8; 16],
    pub domain_digest: Digest256,
    pub through_seq: u64,
    pub audit_generation: u64,
    pub database_oid: u64,
    pub schema_profile_digest: Digest256,
    pub state_digest: Digest256,
    pub log_digest: Digest256,
    pub historical_members: u64,
    pub current_members: u64,
    /// CMD-derived independent exact `{key,digest,length}` transcripts.
    /// STO binds these claims but cannot issue their completeness proof.
    pub history_membership_root: Digest256,
    pub current_membership_root: Digest256,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerationCatalogV1 {
    pub key_codec_digest: Digest256,
    pub catalog_root: Digest256,
    pub partitions: Vec<PackedPartitionRefV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerationDescriptorV1 {
    pub cut: GenerationCutV1,
    pub history: GenerationCatalogV1,
    pub current: GenerationCatalogV1,
}

#[derive(Clone, Copy, Debug)]
pub struct GenerationReadLimits {
    pub max_descriptor_bytes: usize,
    pub shape: GenerationShapeLimits,
    pub max_stream_rows: u64,
    pub max_stream_key_bytes: u64,
}

impl GenerationReadLimits {
    pub(crate) fn validate(self) -> Result<Self> {
        self.shape.validate()?;
        if self.max_descriptor_bytes < 256
            || self.max_descriptor_bytes == usize::MAX
            || self.max_stream_rows == 0
            || self.max_stream_rows == u64::MAX
            || self.max_stream_key_bytes == 0
            || self.max_stream_key_bytes == u64::MAX
        {
            return Err(SegmentError::new(
                Code::BudgetExceeded,
                "invalid generation read limits",
            ));
        }
        Ok(self)
    }
}

/// A private handle to the exact installed descriptor and current store FD
/// root. It is not a CMD selection or policy capability by itself.
#[derive(Clone, Debug)]
pub struct InstalledGenerationV1 {
    pub(crate) store: SegmentStore,
    pub(crate) digest: Digest256,
    pub(crate) descriptor: GenerationDescriptorV1,
    pub(crate) pin_lock: Arc<std::fs::File>,
}

impl InstalledGenerationV1 {
    pub fn digest(&self) -> Digest256 {
        self.digest
    }
    pub fn descriptor(&self) -> &GenerationDescriptorV1 {
        &self.descriptor
    }

    /// Owned exact lookup in one authenticated leaf. Descriptor membership
    /// certifies placement only; callers retain currentness and rights checks.
    pub fn lookup(
        &self,
        namespace: GenerationNamespaceV1,
        key: &[u8],
        limits: GenerationReadLimits,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<PlacementGenerationRowV1>> {
        let limits = limits.validate()?;
        check(deadline, cancelled)?;
        if key.is_empty() || key.len() > limits.shape.max_key_bytes {
            return Err(budget());
        }
        let catalog = match namespace {
            GenerationNamespaceV1::History => &self.descriptor.history,
            GenerationNamespaceV1::Current => &self.descriptor.current,
        };
        if catalog.partitions.len() > limits.shape.max_partitions {
            return Err(budget());
        }
        let position = catalog.partitions.partition_point(|part| {
            part.semantic
                .bounds
                .lower_inclusive
                .as_deref()
                .is_none_or(|lower| lower <= key)
        });
        let Some(reference) = position
            .checked_sub(1)
            .and_then(|position| catalog.partitions.get(position))
        else {
            return Ok(None);
        };
        if reference
            .semantic
            .bounds
            .upper_exclusive
            .as_deref()
            .is_some_and(|upper| upper <= key)
        {
            return Ok(None);
        }
        let leaf = self.store.open_packed_leaf_checked(
            reference.content_digest,
            limits.shape,
            deadline,
            cancelled,
        )?;
        let described = crate::generation::describe_placement_partition(
            self.store.domain_digest(),
            leaf.bounds.clone(),
            leaf.rows.iter().cloned().map(Ok),
            limits.shape,
        )?;
        if leaf.bounds != reference.semantic.bounds || described != reference.semantic {
            return Err(SegmentError::new(
                Code::InvalidReceipt,
                "selected lookup leaf differs",
            ));
        }
        check(deadline, cancelled)?;
        Ok(leaf
            .rows
            .binary_search_by(|row| row.key.as_slice().cmp(key))
            .ok()
            .map(|position| leaf.rows[position].clone()))
    }

    pub fn stream(
        &self,
        namespace: GenerationNamespaceV1,
        limits: GenerationReadLimits,
    ) -> Result<GenerationRowStreamV1> {
        let limits = limits.validate()?;
        let catalog = match namespace {
            GenerationNamespaceV1::History => self.descriptor.history.clone(),
            GenerationNamespaceV1::Current => self.descriptor.current.clone(),
        };
        let expected_rows = catalog.partitions.iter().try_fold(0u64, |sum, part| {
            sum.checked_add(part.semantic.rows).ok_or_else(budget)
        })?;
        if expected_rows > limits.max_stream_rows {
            return Err(budget());
        }
        let mut transcript = Digest256Hasher::new();
        transcript.update(b"tos-generation-stream-v1\0");
        transcript.update(self.digest.as_bytes());
        transcript.update(&[match namespace {
            GenerationNamespaceV1::History => 1,
            GenerationNamespaceV1::Current => 2,
        }]);
        Ok(GenerationRowStreamV1 {
            store: self.store.clone(),
            descriptor_digest: self.digest,
            namespace,
            catalog,
            limits,
            _pin_lock: self.pin_lock.clone(),
            partition: 0,
            rows: None,
            observed: 0,
            key_bytes: 0,
            transcript,
            done: false,
            failed: false,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenerationCoverageV1 {
    pub descriptor_digest: Digest256,
    pub namespace: GenerationNamespaceV1,
    pub catalog_root: Digest256,
    pub rows: u64,
    pub stream_digest: Digest256,
}

pub struct GenerationRowStreamV1 {
    store: SegmentStore,
    descriptor_digest: Digest256,
    namespace: GenerationNamespaceV1,
    catalog: GenerationCatalogV1,
    limits: GenerationReadLimits,
    _pin_lock: Arc<std::fs::File>,
    partition: usize,
    rows: Option<std::vec::IntoIter<PlacementGenerationRowV1>>,
    observed: u64,
    key_bytes: u64,
    transcript: Digest256Hasher,
    done: bool,
    failed: bool,
}

impl GenerationRowStreamV1 {
    pub fn next_row(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<PlacementGenerationRowV1>> {
        if self.failed {
            return Err(SegmentError::new(
                Code::InvalidReceipt,
                "generation stream already refused",
            ));
        }
        let result = self.next_row_inner(deadline, cancelled);
        if result.is_err() {
            self.failed = true;
        }
        result
    }

    fn next_row_inner(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<PlacementGenerationRowV1>> {
        if self.done {
            return Ok(None);
        }
        check(deadline, cancelled)?;
        loop {
            if let Some(rows) = &mut self.rows {
                if let Some(row) = rows.next() {
                    self.observed = self.observed.checked_add(1).ok_or_else(budget)?;
                    self.key_bytes = self
                        .key_bytes
                        .checked_add(row.key.len() as u64)
                        .ok_or_else(budget)?;
                    if self.observed > self.limits.max_stream_rows
                        || self.key_bytes > self.limits.max_stream_key_bytes
                    {
                        return Err(budget());
                    }
                    self.transcript
                        .update(&(row.key.len() as u32).to_le_bytes());
                    self.transcript.update(&row.key);
                    self.transcript.update(row.logical_digest.as_bytes());
                    self.transcript.update(&row.logical_length.to_le_bytes());
                    self.transcript.update(&row.placement.encode());
                    check(deadline, cancelled)?;
                    return Ok(Some(row));
                }
            }
            self.rows = None;
            let Some(reference) = self.catalog.partitions.get(self.partition) else {
                let expected = self.catalog.partitions.iter().try_fold(0u64, |sum, part| {
                    sum.checked_add(part.semantic.rows).ok_or_else(budget)
                })?;
                if self.observed != expected {
                    return Err(SegmentError::new(
                        Code::InvalidReceipt,
                        "generation stream count differs",
                    ));
                }
                self.done = true;
                return Ok(None);
            };
            check(deadline, cancelled)?;
            let leaf = self.store.open_packed_leaf_checked(
                reference.content_digest,
                self.limits.shape,
                deadline,
                cancelled,
            )?;
            if leaf.bounds != reference.semantic.bounds {
                return Err(SegmentError::new(
                    Code::InvalidReceipt,
                    "selected leaf bounds differ",
                ));
            }
            let described = crate::generation::describe_placement_partition(
                self.store.domain_digest(),
                leaf.bounds,
                leaf.rows.iter().cloned().map(Ok),
                self.limits.shape,
            )?;
            if described != reference.semantic {
                return Err(SegmentError::new(
                    Code::InvalidReceipt,
                    "selected leaf rows differ",
                ));
            }
            self.rows = Some(leaf.rows.into_iter());
            self.partition += 1;
        }
    }

    pub fn coverage(&self) -> Option<GenerationCoverageV1> {
        if !self.done || self.failed {
            return None;
        }
        let mut transcript = self.transcript.clone();
        transcript.update(&self.observed.to_le_bytes());
        Some(GenerationCoverageV1 {
            descriptor_digest: self.descriptor_digest,
            namespace: self.namespace,
            catalog_root: self.catalog.catalog_root,
            rows: self.observed,
            stream_digest: transcript.finalize(),
        })
    }
}

impl GenerationDescriptorV1 {
    pub(crate) fn encode(&self, domain: &[u8], limits: GenerationReadLimits) -> Result<Vec<u8>> {
        let limits = limits.validate()?;
        self.validate(domain, limits.shape)?;
        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(PROFILE);
        out.extend_from_slice(&self.cut.store_id);
        out.extend_from_slice(self.cut.domain_digest.as_bytes());
        for n in [
            self.cut.through_seq,
            self.cut.audit_generation,
            self.cut.database_oid,
        ] {
            out.extend_from_slice(&n.to_le_bytes());
        }
        for digest in [
            self.cut.schema_profile_digest,
            self.cut.state_digest,
            self.cut.log_digest,
        ] {
            out.extend_from_slice(digest.as_bytes());
        }
        out.extend_from_slice(&self.cut.historical_members.to_le_bytes());
        out.extend_from_slice(&self.cut.current_members.to_le_bytes());
        out.extend_from_slice(self.cut.history_membership_root.as_bytes());
        out.extend_from_slice(self.cut.current_membership_root.as_bytes());
        for catalog in [&self.history, &self.current] {
            out.extend_from_slice(catalog.key_codec_digest.as_bytes());
            out.extend_from_slice(catalog.catalog_root.as_bytes());
            out.extend_from_slice(&(catalog.partitions.len() as u32).to_le_bytes());
            for reference in &catalog.partitions {
                let semantic = &reference.semantic;
                for key in [
                    &semantic.bounds.lower_inclusive,
                    &semantic.bounds.upper_exclusive,
                    &semantic.first_key,
                    &semantic.last_key,
                ] {
                    put_key(&mut out, key, limits.max_descriptor_bytes)?;
                }
                out.extend_from_slice(&semantic.rows.to_le_bytes());
                out.extend_from_slice(&semantic.leaf_bytes.to_le_bytes());
                out.extend_from_slice(semantic.leaf_digest.as_bytes());
                out.extend_from_slice(reference.content_digest.as_bytes());
                if out
                    .len()
                    .checked_add(32)
                    .is_none_or(|n| n > limits.max_descriptor_bytes)
                {
                    return Err(budget());
                }
            }
        }
        let digest = Digest256::of_bytes(&out);
        out.extend_from_slice(digest.as_bytes());
        if out.len() > limits.max_descriptor_bytes {
            return Err(budget());
        }
        Ok(out)
    }

    pub(crate) fn decode(raw: &[u8], domain: &[u8], limits: GenerationReadLimits) -> Result<Self> {
        let limits = limits.validate()?;
        if raw.len() > limits.max_descriptor_bytes
            || raw.len() < 8 + 4 + 24 + 16 + 32 + 24 + 96 + 16 + 64 + 2 * 68 + 32
        {
            return Err(invalid());
        }
        let end = raw.len() - 32;
        if &raw[..8] != MAGIC
            || raw[8..10] != 1u16.to_le_bytes()
            || raw[10..12] != [0, 0]
            || &raw[12..36] != PROFILE
            || Digest256::of_bytes(&raw[..end]).as_bytes() != &raw[end..]
        {
            return Err(invalid());
        }
        let mut at = 36;
        let store_id = take(raw, &mut at, 16, end)?.try_into().expect("fixed");
        let domain_digest = digest(take(raw, &mut at, 32, end)?)?;
        let cut = GenerationCutV1 {
            store_id,
            domain_digest,
            through_seq: wide(raw, &mut at, end)?,
            audit_generation: wide(raw, &mut at, end)?,
            database_oid: wide(raw, &mut at, end)?,
            schema_profile_digest: digest(take(raw, &mut at, 32, end)?)?,
            state_digest: digest(take(raw, &mut at, 32, end)?)?,
            log_digest: digest(take(raw, &mut at, 32, end)?)?,
            historical_members: wide(raw, &mut at, end)?,
            current_members: wide(raw, &mut at, end)?,
            history_membership_root: digest(take(raw, &mut at, 32, end)?)?,
            current_membership_root: digest(take(raw, &mut at, 32, end)?)?,
        };
        let history = get_catalog(raw, &mut at, end, limits.shape)?;
        let current = get_catalog(raw, &mut at, end, limits.shape)?;
        if at != end {
            return Err(invalid());
        }
        let result = Self {
            cut,
            history,
            current,
        };
        result.validate(domain, limits.shape)?;
        Ok(result)
    }

    fn validate(&self, domain: &[u8], shape: GenerationShapeLimits) -> Result<()> {
        if self.cut.domain_digest != Digest256::of_bytes(domain)
            || self.cut.database_oid == 0
            || self.cut.historical_members < self.cut.current_members
        {
            return Err(invalid());
        }
        for (catalog, expected, namespace) in [
            (
                &self.history,
                self.cut.historical_members,
                b"cmd2.history.v1".as_slice(),
            ),
            (
                &self.current,
                self.cut.current_members,
                b"cmd2.current.v1".as_slice(),
            ),
        ] {
            let rows = catalog.partitions.iter().try_fold(0u64, |sum, reference| {
                sum.checked_add(reference.semantic.rows).ok_or_else(budget)
            })?;
            if rows != expected {
                return Err(invalid());
            }
            let semantic: Vec<_> = catalog
                .partitions
                .iter()
                .map(|p| p.semantic.clone())
                .collect();
            let root = placement_catalog_shape_root(
                domain,
                namespace,
                b"all",
                catalog.key_codec_digest,
                KeyComparatorV1::RawUnsignedBytes,
                &semantic,
                shape,
            )?;
            let mut hasher = Digest256Hasher::new();
            hasher.update(b"tos-packed-placement-catalog-v1");
            hasher.update(root.as_bytes());
            hasher.update(&(catalog.partitions.len() as u32).to_le_bytes());
            for reference in &catalog.partitions {
                hasher.update(reference.content_digest.as_bytes());
            }
            if hasher.finalize() != catalog.catalog_root {
                return Err(invalid());
            }
        }
        Ok(())
    }
}

fn get_catalog(
    raw: &[u8],
    at: &mut usize,
    end: usize,
    shape: GenerationShapeLimits,
) -> Result<GenerationCatalogV1> {
    let key_codec_digest = digest(take(raw, at, 32, end)?)?;
    let catalog_root = digest(take(raw, at, 32, end)?)?;
    let count = number(raw, at, end)? as usize;
    if count == 0 || count > shape.max_partitions || count > (end - *at) / 80 {
        return Err(invalid());
    }
    let mut partitions = Vec::with_capacity(count);
    for _ in 0..count {
        let bounds = PartitionBoundsV1 {
            lower_inclusive: get_key(raw, at, end, shape.max_key_bytes)?,
            upper_exclusive: get_key(raw, at, end, shape.max_key_bytes)?,
        };
        let first_key = get_key(raw, at, end, shape.max_key_bytes)?;
        let last_key = get_key(raw, at, end, shape.max_key_bytes)?;
        let rows = wide(raw, at, end)?;
        let leaf_bytes = wide(raw, at, end)?;
        let leaf_digest = digest(take(raw, at, 32, end)?)?;
        let content_digest = digest(take(raw, at, 32, end)?)?;
        partitions.push(PackedPartitionRefV1 {
            semantic: PlacementPartitionV1 {
                bounds,
                rows,
                first_key,
                last_key,
                leaf_bytes,
                leaf_digest,
            },
            content_digest,
        });
    }
    Ok(GenerationCatalogV1 {
        key_codec_digest,
        catalog_root,
        partitions,
    })
}

fn put_key(out: &mut Vec<u8>, key: &Option<Vec<u8>>, cap: usize) -> Result<()> {
    match key {
        None => {
            if out.len().checked_add(4).is_none_or(|size| size > cap) {
                return Err(budget());
            }
            out.extend_from_slice(&NONE.to_le_bytes())
        }
        Some(key) => {
            if key.len() >= NONE as usize
                || out
                    .len()
                    .checked_add(4)
                    .and_then(|size| size.checked_add(key.len()))
                    .is_none_or(|size| size > cap)
            {
                return Err(budget());
            }
            out.extend_from_slice(&(key.len() as u32).to_le_bytes());
            out.extend_from_slice(key);
        }
    }
    Ok(())
}

fn get_key(raw: &[u8], at: &mut usize, end: usize, max: usize) -> Result<Option<Vec<u8>>> {
    let length = number(raw, at, end)?;
    if length == NONE {
        return Ok(None);
    }
    if length == 0 || length as usize > max {
        return Err(invalid());
    }
    Ok(Some(take(raw, at, length as usize, end)?.to_vec()))
}

fn take<'a>(raw: &'a [u8], at: &mut usize, size: usize, end: usize) -> Result<&'a [u8]> {
    let next = at.checked_add(size).ok_or_else(invalid)?;
    if next > end {
        return Err(invalid());
    }
    let value = &raw[*at..next];
    *at = next;
    Ok(value)
}
fn number(raw: &[u8], at: &mut usize, end: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        take(raw, at, 4, end)?.try_into().expect("fixed"),
    ))
}
fn wide(raw: &[u8], at: &mut usize, end: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(
        take(raw, at, 8, end)?.try_into().expect("fixed"),
    ))
}
fn digest(raw: &[u8]) -> Result<Digest256> {
    Ok(Digest256::from_bytes(
        raw.try_into().map_err(|_| invalid())?,
    ))
}
pub(crate) fn check(deadline: Instant, cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(SegmentError::new(
            Code::Cancelled,
            "generation read cancelled",
        ));
    }
    if Instant::now() >= deadline {
        return Err(SegmentError::new(
            Code::DeadlineExceeded,
            "generation read timed out",
        ));
    }
    Ok(())
}
fn invalid() -> SegmentError {
    SegmentError::new(Code::InvalidFormat, "generation descriptor differs")
}
fn budget() -> SegmentError {
    SegmentError::new(Code::BudgetExceeded, "generation budget exceeded")
}
