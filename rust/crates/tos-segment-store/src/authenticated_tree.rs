//! Domain- and kind-bound addressable trees for internal warm projections.
//!
//! This module stores only opaque ordered key/value rows. Its commitments prove
//! the exact bytes and tree shape presented to it; CMD still owns source-cut
//! completeness, currentness, rights, and disclosure decisions.

use std::alloc::Layout;
use std::collections::HashSet;
use std::ops::Deref;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use tos_foundation::{Digest256, Digest256Hasher};

use crate::authenticated_pack_set_v2::AuthenticatedTreePackSetV2;
use crate::error::{Result, SegmentError, SegmentErrorCode as Code};
use crate::generation::PlacementGenerationRowV1;
use crate::placement::PlacementV1;
use crate::selected::check;
use crate::store::{ImmutableBlobInstallV1, PinDirectoryLease, SegmentStore};

const NODE_MAGIC: &[u8; 8] = b"TOSATN1\0";
const DESCRIPTOR_MAGIC: &[u8; 8] = b"TOSATD1\0";
const OBJECT_MAGIC: &[u8; 8] = b"TOSATO1\0";
const PLACEMENT_ROW_MAGIC: &[u8; 8] = b"TOSATR1\0";
const PACKED_DESCRIPTOR_MAGIC: &[u8; 8] = b"TOSATD2\0";
const PACKED_FRAME_MAGIC: &[u8; 8] = b"TOSANF2\0";
const PACKED_FRAME_VERSION: u16 = 1;
const AUTHENTICATED_PACK_MAX_BYTES: usize = 4 * 1024 * 1024;
const MAX_CHILD_NIBBLES: usize = 16;
const MAX_KEY_BYTES: usize = 4096;

/// Per-call limits. `max_rows` caps rows consumed by this call: build input,
/// delta input, or a complete stream/verification. Warm lookup is instead
/// bounded by actual nodes and bytes it reads. There is no global tree-row cap.
#[derive(Clone, Copy, Debug)]
pub struct AuthenticatedTreeLimitsV1 {
    pub max_key_bytes: usize,
    pub max_value_bytes: usize,
    pub max_kind_bytes: usize,
    pub max_node_bytes: usize,
    pub max_children: usize,
    pub max_nodes: u64,
    pub max_total_bytes: u64,
    pub max_rows: u64,
}

impl AuthenticatedTreeLimitsV1 {
    fn validate(self) -> Result<Self> {
        if self.max_key_bytes == 0
            || self.max_key_bytes > MAX_KEY_BYTES
            || self.max_value_bytes == 0
            || self.max_value_bytes > u32::MAX as usize
            || self.max_kind_bytes == 0
            || self.max_kind_bytes > u16::MAX as usize
            || self.max_node_bytes < 128
            || self.max_node_bytes == usize::MAX
            || self.max_children == 0
            || self.max_children > MAX_CHILD_NIBBLES
            || self.max_nodes == 0
            || self.max_nodes == u64::MAX
            || self.max_total_bytes == 0
            || self.max_total_bytes == u64::MAX
            || self.max_rows == 0
            || self.max_rows == u64::MAX
        {
            return Err(budget("invalid authenticated tree limits"));
        }
        Ok(self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticatedTreeEntryV1 {
    pub key: Vec<u8>,
    pub value: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticatedTreeDeltaV1 {
    pub key: Vec<u8>,
    /// `None` removes this exact key; `Some` inserts or replaces its value.
    pub value: Option<Vec<u8>>,
}

/// Invocation-owned cumulative IO ledger for one authenticated-tree operation.
/// The store charges the maximum exact frame/payload before touching physical
/// bytes, then reconciles the bytes the operation actually returned. This is
/// accounting only; it carries no source authorization.
pub trait AuthenticatedTreeIoLedgerV1: Send + Sync {
    fn charge_read(&self, bytes: u64) -> bool;
    /// Debit a source-owned conservative name/metadata guard on the same
    /// cumulative read ceiling without classifying it as returned payload.
    /// Adapters backed by a ledger with separate upper-bound accounting should
    /// override this method; the default preserves compatibility for existing
    /// non-classifying ledgers.
    fn charge_read_upper_bound(&self, bytes: u64) -> bool {
        self.charge_read(bytes)
    }
    fn record_read_returned(&self, bytes: u64) -> bool;
    fn charge_write(&self, bytes: u64) -> bool;
    fn record_write_returned(&self, bytes: u64) -> bool;
    /// Reserve a conservative persistent-allocation upper bound before a new
    /// immutable pack is staged. Non-owning readers may keep the default.
    fn reserve_allocated_bytes(&self, _bytes: u64) -> bool {
        true
    }
    /// Reconcile one staged file's observed allocated blocks against its
    /// prior reservation. This does not grant filesystem capacity.
    fn reconcile_allocated_bytes(&self, reserved: u64, actual: u64) -> bool {
        actual <= reserved
    }
    /// Conservative physical allocation quantum used for the pre-write bound.
    fn allocation_unit_bytes(&self) -> u64 {
        65_536
    }
}

fn charge_tree_read(ledger: Option<&dyn AuthenticatedTreeIoLedgerV1>, bytes: u64) -> Result<()> {
    if ledger.is_some_and(|ledger| !ledger.charge_read(bytes)) {
        return Err(budget("authenticated tree read IO reservation refused"));
    }
    Ok(())
}

fn record_tree_read(ledger: Option<&dyn AuthenticatedTreeIoLedgerV1>, bytes: u64) -> Result<()> {
    if ledger.is_some_and(|ledger| !ledger.record_read_returned(bytes)) {
        return Err(budget("authenticated tree read IO reconciliation refused"));
    }
    Ok(())
}

fn charge_tree_write(ledger: Option<&dyn AuthenticatedTreeIoLedgerV1>, bytes: u64) -> Result<()> {
    if ledger.is_some_and(|ledger| !ledger.charge_write(bytes)) {
        return Err(budget("authenticated tree write IO reservation refused"));
    }
    Ok(())
}

fn record_tree_write(ledger: Option<&dyn AuthenticatedTreeIoLedgerV1>, bytes: u64) -> Result<()> {
    if ledger.is_some_and(|ledger| !ledger.record_write_returned(bytes)) {
        return Err(budget("authenticated tree write IO reconciliation refused"));
    }
    Ok(())
}

/// A content address and authenticated subtree summary. The compressed
/// Patricia prefix is derived from these key bounds, so no flat partition
/// table or extra prefix vector is retained in a descriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticatedTreeNodeRefV1 {
    pub digest: Digest256,
    pub entries: u64,
    pub min_key: Vec<u8>,
    pub max_key: Vec<u8>,
}

/// Constant-size root descriptor apart from bounded `kind` and root key bounds.
/// It can be embedded in the CMD-owned V2 descriptor or stored as an immutable
/// opaque object. A descriptor alone is not a selection or policy capability.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticatedTreeDescriptorV1 {
    pub store_id: [u8; 16],
    pub domain_digest: Digest256,
    pub kind: Vec<u8>,
    pub root: Option<AuthenticatedTreeNodeRefV1>,
    pub entries: u64,
    pub commitment: Digest256,
}

impl AuthenticatedTreeDescriptorV1 {
    /// Canonical bounded wire form for embedding in a CMD-owned descriptor.
    pub fn encode(&self, max_bytes: usize) -> Result<Vec<u8>> {
        validate_descriptor_shape(self, max_bytes)?;
        let encoded_len = descriptor_encoded_len(self)?;
        if encoded_len > max_bytes {
            return Err(budget("authenticated descriptor exceeds limit"));
        }
        let mut raw = Vec::new();
        raw.try_reserve_exact(encoded_len)
            .map_err(|_| budget("authenticated descriptor allocation failed"))?;
        raw.extend_from_slice(DESCRIPTOR_MAGIC);
        raw.extend_from_slice(&1u16.to_le_bytes());
        raw.extend_from_slice(&0u16.to_le_bytes());
        raw.extend_from_slice(&self.store_id);
        raw.extend_from_slice(self.domain_digest.as_bytes());
        raw.extend_from_slice(&(self.kind.len() as u16).to_le_bytes());
        raw.extend_from_slice(&self.kind);
        raw.extend_from_slice(&self.entries.to_le_bytes());
        match &self.root {
            None => raw.push(0),
            Some(root) => {
                raw.push(1);
                encode_node_ref(&mut raw, root)?;
            }
        }
        raw.extend_from_slice(self.commitment.as_bytes());
        if raw.len() != encoded_len {
            return Err(SegmentError::new(
                Code::InvalidFormat,
                "authenticated descriptor encoding length differs",
            ));
        }
        Ok(raw)
    }

    /// Decode one canonical bounded wire form and verify its root commitment.
    pub fn decode(raw: &[u8], max_bytes: usize) -> Result<Self> {
        if raw.len() > max_bytes {
            return Err(budget("authenticated descriptor exceeds limit"));
        }
        let mut reader = Reader::new(raw);
        if reader.take(8)? != DESCRIPTOR_MAGIC || reader.u16()? != 1 || reader.u16()? != 0 {
            return Err(invalid("authenticated descriptor header differs"));
        }
        let store_id = reader.array::<16>()?;
        let domain_digest = digest_from_raw(reader.take(32)?)?;
        let kind_len = reader.u16()? as usize;
        if kind_len == 0 || kind_len > max_bytes {
            return Err(invalid("authenticated descriptor kind differs"));
        }
        let kind = reader.take(kind_len)?.to_vec();
        let entries = reader.u64()?;
        let root = match reader.u8()? {
            0 => None,
            1 => Some(decode_node_ref(&mut reader, max_bytes)?),
            _ => return Err(invalid("authenticated descriptor root tag differs")),
        };
        let commitment = digest_from_raw(reader.take(32)?)?;
        reader.finish()?;
        let descriptor = Self {
            store_id,
            domain_digest,
            kind,
            root,
            entries,
            commitment,
        };
        validate_descriptor_shape(&descriptor, max_bytes)?;
        if descriptor_root_commitment(
            descriptor.store_id,
            descriptor.domain_digest,
            &descriptor.kind,
            descriptor.root.as_ref(),
            descriptor.entries,
        ) != descriptor.commitment
        {
            return Err(SegmentError::new(
                Code::CorruptBytes,
                "authenticated descriptor commitment differs",
            ));
        }
        if descriptor.encode(max_bytes)?.as_slice() != raw {
            return Err(invalid("authenticated descriptor is not canonical"));
        }
        Ok(descriptor)
    }
}

/// Physical locator for a canonical semantic node. Pack coordinates affect
/// storage only; they are excluded from `AuthenticatedTreeNodeRefV1::digest`
/// and therefore from the tree commitment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticatedTreeLocatorV2 {
    pub pack_digest: Digest256,
    pub pack_id: [u8; 16],
    pub offset: u64,
    pub frame_len: u32,
    pub frame_sha256: Digest256,
}

/// Versioned tree descriptor. A missing physical root means the exact legacy
/// V1 digest-addressed tree; a present root selects the packed V2 reader.
/// Deref keeps semantic proof fields (`commitment`, `entries`, etc.) stable for
/// CMD callers while the separately encoded locator binds physical custody.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticatedTreeDescriptorV2 {
    pub semantic: AuthenticatedTreeDescriptorV1,
    pub physical_root: Option<AuthenticatedTreeLocatorV2>,
}

impl Deref for AuthenticatedTreeDescriptorV2 {
    type Target = AuthenticatedTreeDescriptorV1;

    fn deref(&self) -> &Self::Target {
        &self.semantic
    }
}

impl AuthenticatedTreeDescriptorV2 {
    pub fn from_legacy(semantic: AuthenticatedTreeDescriptorV1) -> Self {
        Self {
            semantic,
            physical_root: None,
        }
    }

    /// Preserve old descriptor bytes exactly when wrapping a legacy root.
    /// Packed roots use a distinct strict wire frame that carries the physical
    /// locator without changing the canonical semantic descriptor.
    pub fn encode(&self, max_bytes: usize) -> Result<Vec<u8>> {
        let Some(locator) = &self.physical_root else {
            return self.semantic.encode(max_bytes);
        };
        validate_locator(locator)?;
        if self.semantic.root.is_none() || self.semantic.entries == 0 {
            return Err(invalid("packed descriptor has no semantic root"));
        }
        let semantic = self.semantic.encode(max_bytes)?;
        let total = 8usize
            .checked_add(2 + 2 + 4)
            .and_then(|len| len.checked_add(semantic.len()))
            .and_then(|len| len.checked_add(locator_encoded_len()))
            .ok_or_else(|| budget("packed descriptor size overflow"))?;
        if total > max_bytes {
            return Err(budget("packed descriptor exceeds limit"));
        }
        let mut raw = Vec::new();
        raw.try_reserve_exact(total)
            .map_err(|_| budget("packed descriptor allocation failed"))?;
        raw.extend_from_slice(PACKED_DESCRIPTOR_MAGIC);
        raw.extend_from_slice(&1u16.to_le_bytes());
        raw.extend_from_slice(&0u16.to_le_bytes());
        raw.extend_from_slice(
            &u32::try_from(semantic.len())
                .map_err(|_| budget("semantic descriptor is too long"))?
                .to_le_bytes(),
        );
        raw.extend_from_slice(&semantic);
        encode_locator(&mut raw, locator);
        if raw.len() != total {
            return Err(invalid("packed descriptor encoding length differs"));
        }
        Ok(raw)
    }

    /// Decode packed descriptors strictly, while accepting exact historical
    /// V1 descriptor bytes for old internal cuts.
    pub fn decode(raw: &[u8], max_bytes: usize) -> Result<Self> {
        if raw.len() > max_bytes {
            return Err(budget("packed descriptor exceeds limit"));
        }
        if raw.starts_with(DESCRIPTOR_MAGIC) {
            return AuthenticatedTreeDescriptorV1::decode(raw, max_bytes).map(Self::from_legacy);
        }
        let mut reader = Reader::new(raw);
        if reader.take(8)? != PACKED_DESCRIPTOR_MAGIC || reader.u16()? != 1 || reader.u16()? != 0 {
            return Err(invalid("packed descriptor header differs"));
        }
        let semantic_len = reader.u32()? as usize;
        if semantic_len > max_bytes {
            return Err(budget("semantic descriptor exceeds limit"));
        }
        let semantic =
            AuthenticatedTreeDescriptorV1::decode(reader.take(semantic_len)?, max_bytes)?;
        let physical_root = decode_locator(&mut reader)?;
        reader.finish()?;
        if semantic.root.is_none() || semantic.entries == 0 {
            return Err(invalid("packed descriptor has no semantic root"));
        }
        let descriptor = Self {
            semantic,
            physical_root: Some(physical_root),
        };
        if descriptor.encode(max_bytes)?.as_slice() != raw {
            return Err(invalid("packed descriptor is not canonical"));
        }
        Ok(descriptor)
    }
}

/// Actual content-node I/O performed by one source operation.
/// For packed V2, `read_bytes` includes validated frames copied from the
/// bounded active in-memory writer chunk during a multi-key delta; callers
/// must not label that counter as physical disk I/O. Pack install/readback and
/// explicit cold closure bytes are charged separately as actual storage I/O.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AuthenticatedTreeWorkV1 {
    pub read_nodes: u64,
    pub read_bytes: u64,
    pub written_nodes: u64,
    pub written_bytes: u64,
    /// Allocated blocks retained by newly linked content-addressed packs.
    /// This is separate from logical read/write I/O and is zero for reused
    /// packs. The caller must reserve this persistent space before writing.
    pub allocated_bytes: u64,
}

impl AuthenticatedTreeWorkV1 {
    fn total_nodes(self) -> u64 {
        self.read_nodes.saturating_add(self.written_nodes)
    }

    fn total_bytes(self) -> u64 {
        self.read_bytes.saturating_add(self.written_bytes)
    }

    fn charge_read(&mut self, bytes: usize, limits: AuthenticatedTreeLimitsV1) -> Result<()> {
        self.read_nodes = self
            .read_nodes
            .checked_add(1)
            .ok_or_else(|| budget("tree node counter overflow"))?;
        self.read_bytes = self
            .read_bytes
            .checked_add(bytes as u64)
            .ok_or_else(|| budget("tree read byte counter overflow"))?;
        check_work(*self, limits)
    }

    fn charge_install(
        &mut self,
        install: ImmutableBlobInstallV1,
        limits: AuthenticatedTreeLimitsV1,
    ) -> Result<()> {
        if install.read_bytes > 0 {
            self.read_nodes = self
                .read_nodes
                .checked_add(1)
                .ok_or_else(|| budget("tree node counter overflow"))?;
            self.read_bytes = self
                .read_bytes
                .checked_add(install.read_bytes)
                .ok_or_else(|| budget("tree read byte counter overflow"))?;
        }
        if install.written_bytes > 0 {
            self.written_nodes = self
                .written_nodes
                .checked_add(1)
                .ok_or_else(|| budget("tree node counter overflow"))?;
            self.written_bytes = self
                .written_bytes
                .checked_add(install.written_bytes)
                .ok_or_else(|| budget("tree write byte counter overflow"))?;
        }
        self.allocated_bytes = self
            .allocated_bytes
            .checked_add(install.allocated_bytes)
            .ok_or_else(|| budget("tree allocated-byte counter overflow"))?;
        check_work(*self, limits)
    }

    fn charge_pack_install(
        &mut self,
        install: ImmutableBlobInstallV1,
        frame_count: u64,
        limits: AuthenticatedTreeLimitsV1,
    ) -> Result<()> {
        if install.read_bytes > 0 {
            self.read_nodes = self
                .read_nodes
                .checked_add(frame_count)
                .ok_or_else(|| budget("tree pack read node counter overflow"))?;
            self.read_bytes = self
                .read_bytes
                .checked_add(install.read_bytes)
                .ok_or_else(|| budget("tree pack read byte counter overflow"))?;
        }
        if install.written_bytes > 0 {
            self.written_nodes = self
                .written_nodes
                .checked_add(frame_count)
                .ok_or_else(|| budget("tree pack write node counter overflow"))?;
            self.written_bytes = self
                .written_bytes
                .checked_add(install.written_bytes)
                .ok_or_else(|| budget("tree pack write byte counter overflow"))?;
        }
        check_work(*self, limits)
    }

    fn charge_pack_read(
        &mut self,
        bytes: usize,
        frame_count: u64,
        limits: AuthenticatedTreeLimitsV1,
    ) -> Result<()> {
        self.read_nodes = self
            .read_nodes
            .checked_add(frame_count)
            .ok_or_else(|| budget("tree pack closure node counter overflow"))?;
        self.read_bytes = self
            .read_bytes
            .checked_add(bytes as u64)
            .ok_or_else(|| budget("tree pack closure byte counter overflow"))?;
        check_work(*self, limits)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedTreeCoverageV1 {
    pub descriptor_commitment: Digest256,
    pub entries: u64,
    pub stream_digest: Digest256,
    pub work: AuthenticatedTreeWorkV1,
}

/// Streaming traversal retains at most the bounded nodes on one Patricia path.
pub struct AuthenticatedTreeRowStreamV1 {
    store: SegmentStore,
    descriptor: AuthenticatedTreeDescriptorV1,
    limits: AuthenticatedTreeLimitsV1,
    _pin_lock: Arc<PinDirectoryLease>,
    stack: Vec<StreamFrame>,
    started: bool,
    observed: u64,
    transcript: Digest256Hasher,
    work: AuthenticatedTreeWorkV1,
    done: bool,
    failed: bool,
}

impl AuthenticatedTreeRowStreamV1 {
    /// Return one exact key/value row. `None` is available only after the full
    /// ordered tree has been traversed and its declared count has matched.
    pub fn next_row(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<AuthenticatedTreeEntryV1>> {
        if self.failed {
            return Err(SegmentError::new(
                Code::InvalidReceipt,
                "authenticated stream already refused",
            ));
        }
        if self.done {
            return Ok(None);
        }
        let result = self.next_row_inner(deadline, cancelled);
        if result.is_err() {
            self.failed = true;
        }
        result
    }

    pub fn coverage(&self) -> Option<AuthenticatedTreeCoverageV1> {
        if !self.done || self.failed {
            return None;
        }
        let mut transcript = self.transcript.clone();
        Some(AuthenticatedTreeCoverageV1 {
            descriptor_commitment: self.descriptor.commitment,
            entries: self.observed,
            stream_digest: transcript.finalize(),
            work: self.work,
        })
    }

    fn next_row_inner(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<AuthenticatedTreeEntryV1>> {
        loop {
            check(deadline, cancelled)?;
            if !self.started {
                self.started = true;
                if let Some(root) = self.descriptor.root.clone() {
                    let node = load_node(
                        &self.store,
                        &self.descriptor,
                        &root,
                        self.limits,
                        &mut self.work,
                        deadline,
                        cancelled,
                    )?;
                    self.stack.push(StreamFrame::new(root, node));
                    continue;
                }
            }
            let Some(frame) = self.stack.last_mut() else {
                if self.observed != self.descriptor.entries {
                    return Err(SegmentError::new(
                        Code::InvalidReceipt,
                        "authenticated stream count differs",
                    ));
                }
                self.done = true;
                return Ok(None);
            };
            if let Some(entry) = frame.value.take() {
                self.observed = self
                    .observed
                    .checked_add(1)
                    .ok_or_else(|| budget("tree row counter overflow"))?;
                if self.observed > self.limits.max_rows {
                    return Err(budget("authenticated stream row limit exceeded"));
                }
                self.transcript
                    .update(&(entry.key.len() as u64).to_le_bytes());
                self.transcript.update(&entry.key);
                self.transcript
                    .update(&(entry.value.len() as u64).to_le_bytes());
                self.transcript.update(&entry.value);
                return Ok(Some(entry));
            }
            if frame.next_child < frame.node.children.len() {
                let child = frame.node.children[frame.next_child].1.clone();
                frame.next_child += 1;
                let node = load_node(
                    &self.store,
                    &self.descriptor,
                    &child,
                    self.limits,
                    &mut self.work,
                    deadline,
                    cancelled,
                )?;
                self.stack.push(StreamFrame::new(child, node));
                continue;
            }
            self.stack.pop();
        }
    }
}

struct StreamFrame {
    _reference: AuthenticatedTreeNodeRefV1,
    node: TreeNode,
    value: Option<AuthenticatedTreeEntryV1>,
    next_child: usize,
}

impl StreamFrame {
    fn new(reference: AuthenticatedTreeNodeRefV1, node: TreeNode) -> Self {
        Self {
            _reference: reference,
            value: node.value.clone(),
            node,
            next_child: 0,
        }
    }
}

/// V2 streaming traversal over packed physical frames. It retains only one
/// semantic node per Patricia path level and never materializes a partition.
pub struct AuthenticatedTreeRowStreamV2 {
    store: SegmentStore,
    descriptor: AuthenticatedTreeDescriptorV2,
    limits: AuthenticatedTreeLimitsV1,
    io_ledger: Option<Arc<dyn AuthenticatedTreeIoLedgerV1>>,
    _pin_lock: Arc<PinDirectoryLease>,
    stack: Vec<StreamFrameV2>,
    state_limit: Option<usize>,
    base_state_bytes: usize,
    retained_node_bytes: usize,
    started: bool,
    observed: u64,
    transcript: Digest256Hasher,
    work: AuthenticatedTreeWorkV1,
    pack_capture: PackCaptureV2,
    done: bool,
    failed: bool,
}

enum PackCaptureV2 {
    None,
    Local(HashSet<Digest256>),
    External(Arc<dyn AuthenticatedTreePackSetV2>),
}

impl PackCaptureV2 {
    fn observe(&mut self, digest: Digest256) -> Result<()> {
        match self {
            Self::None => Ok(()),
            Self::Local(digests) => {
                digests
                    .try_reserve(1)
                    .map_err(|_| budget("cold pack receipt allocation failed"))?;
                digests.insert(digest);
                Ok(())
            }
            Self::External(set) => set.observe(digest),
        }
    }

    fn take_local(&mut self) -> Result<HashSet<Digest256>> {
        match std::mem::replace(self, Self::None) {
            Self::Local(digests) => Ok(digests),
            _ => Err(SegmentError::new(
                Code::InvalidReceipt,
                "local cold pack inventory is unavailable",
            )),
        }
    }
}

impl AuthenticatedTreeRowStreamV2 {
    pub fn next_row(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<AuthenticatedTreeEntryV1>> {
        self.next_row_inner_with_callback(deadline, cancelled, None)
    }

    /// Return one exact row while charging the caller's existing cumulative
    /// work callback before every node visit in this traversal step.
    pub fn next_row_with_work_callback(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
        callback: &mut dyn FnMut() -> bool,
    ) -> Result<Option<AuthenticatedTreeEntryV1>> {
        self.next_row_inner_with_callback(deadline, cancelled, Some(callback))
    }

    fn next_row_inner_with_callback(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
        mut shared_work: Option<&mut dyn FnMut() -> bool>,
    ) -> Result<Option<AuthenticatedTreeEntryV1>> {
        if self.failed {
            return Err(SegmentError::new(
                Code::InvalidReceipt,
                "packed authenticated stream already refused",
            ));
        }
        if self.done {
            return Ok(None);
        }
        let result = self.next_row_inner(deadline, cancelled, &mut shared_work);
        if result.is_err() {
            self.failed = true;
        }
        result
    }

    pub fn coverage(&self) -> Option<AuthenticatedTreeCoverageV1> {
        if !self.done || self.failed {
            return None;
        }
        let mut transcript = self.transcript.clone();
        Some(AuthenticatedTreeCoverageV1 {
            descriptor_commitment: self.descriptor.commitment,
            entries: self.observed,
            stream_digest: transcript.finalize(),
            work: self.work,
        })
    }

    fn node_workspace(&self) -> Result<usize> {
        self.limits.max_node_bytes.checked_mul(64)
            .and_then(|n| n.checked_add(65_536))
            .ok_or_else(|| budget("packed stream node workspace overflow"))
    }

    /// Complete a row walk's physical pack verification without walking the
    /// same tree again. Rows may be inspected privately before this succeeds;
    /// a cold-closure receipt requires this terminal verification too.
    pub fn finish_pack_verification_with_work_callback(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
        shared_work: &mut dyn FnMut() -> bool,
    ) -> Result<AuthenticatedTreeCoverageV1> {
        let result = (|| {
            if !self.done || self.failed {
                return Err(invalid("packed verification requires successful stream EOF"));
            }
            let PackCaptureV2::External(pack_set) = &self.pack_capture else {
                return Err(invalid("packed verification requires its captured pack set"));
            };
            let pack_set = pack_set.clone();
            let mut verify = |digest: Digest256| {
                check(deadline, cancelled)?;
                if !shared_work() {
                    return Err(budget("authenticated tree shared work refused"));
                }
                let remaining = remaining_bytes(self.work, self.limits)?;
                if remaining == 0 {
                    return Err(budget("authenticated tree byte budget exceeded"));
                }
                self.check_state(AUTHENTICATED_PACK_MAX_BYTES)?;
                let raw = self.store.read_authenticated_blob_with_io(
                    digest,
                    AUTHENTICATED_PACK_MAX_BYTES.min(remaining),
                    self.io_ledger.as_deref(),
                    deadline,
                    cancelled,
                )?;
                let frame_count = scan_packed_chunk(&raw)?;
                self.work.charge_pack_read(raw.len(), frame_count, self.limits)
            };
            pack_set.verify_pending(&mut verify)?;
            drop(verify);
            check(deadline, cancelled)?;
            self.coverage().ok_or_else(|| invalid("packed authenticated tree coverage unavailable"))
        })();
        self.failed |= result.is_err();
        result
    }

    fn check_state(&self, extra: usize) -> Result<()> {
        let Some(limit) = self.state_limit else { return Ok(()) };
        let bytes = self.stack.capacity().checked_mul(std::mem::size_of::<StreamFrameV2>())
            .and_then(|n| n.checked_add(self.base_state_bytes))
            .and_then(|n| n.checked_add(self.retained_node_bytes))
            .and_then(|n| n.checked_add(extra))
            .ok_or_else(|| budget("packed stream state overflow"))?;
        if bytes > limit { return Err(budget("packed stream state allowance exceeded")); }
        Ok(())
    }

    fn push_node(&mut self, loaded: LoadedTreeNodeV2) -> Result<()> {
        let retained_state_bytes = loaded_node_retained_state(&loaded)?;
        self.check_state(retained_state_bytes)?;
        if self.stack.len() == self.stack.capacity() {
            // Includes old and replacement buffers while reallocating.
            let slots = self.stack.len().checked_add(1)
                .and_then(|n| n.checked_mul(std::mem::size_of::<StreamFrameV2>()))
                .and_then(|n| n.checked_add(retained_state_bytes))
                .ok_or_else(|| budget("packed stream stack state overflow"))?;
            self.check_state(slots)?;
            self.stack.try_reserve_exact(1)
                .map_err(|_| budget("packed stream stack allocation failed"))?;
            self.check_state(retained_state_bytes)?;
        }
        self.retained_node_bytes = self.retained_node_bytes.checked_add(retained_state_bytes)
            .ok_or_else(|| budget("packed stream retained state overflow"))?;
        self.stack.push(StreamFrameV2 { loaded, next_child: 0, retained_state_bytes });
        Ok(())
    }

    fn next_row_inner(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
        shared_work: &mut Option<&mut dyn FnMut() -> bool>,
    ) -> Result<Option<AuthenticatedTreeEntryV1>> {
        loop {
            check(deadline, cancelled)?;
            if !self.started {
                self.started = true;
                self.check_state(self.node_workspace()?)?;
                if let Some(root) = descriptor_root_handle(&self.descriptor)? {
                    let loaded = load_node_v2(
                        &self.store,
                        &self.descriptor,
                        &root,
                        self.limits,
                        &mut self.work,
                        None,
                        self.io_ledger.as_deref(),
                        deadline,
                        cancelled,
                        shared_work,
                    )?;
                    if let Some(digest) = loaded.physical_pack_digest {
                        self.pack_capture.observe(digest)?;
                    }
                    drop(root);
                    self.push_node(loaded)?;
                    continue;
                }
            }
            if self.stack.last().is_some_and(|frame| frame.loaded.node.value.is_none()
                && frame.next_child < frame.loaded.node.children.len()) {
                self.check_state(self.node_workspace()?)?;
            }
            let Some(frame) = self.stack.last_mut() else {
                if self.observed != self.descriptor.entries {
                    return Err(SegmentError::new(
                        Code::InvalidReceipt,
                        "packed authenticated stream count differs",
                    ));
                }
                self.done = true;
                return Ok(None);
            };
            if let Some(entry) = frame.loaded.node.value.take() {
                self.observed = self
                    .observed
                    .checked_add(1)
                    .ok_or_else(|| budget("packed stream row counter overflow"))?;
                if self.observed > self.limits.max_rows {
                    return Err(budget("packed authenticated stream row limit exceeded"));
                }
                self.transcript
                    .update(&(entry.key.len() as u64).to_le_bytes());
                self.transcript.update(&entry.key);
                self.transcript
                    .update(&(entry.value.len() as u64).to_le_bytes());
                self.transcript.update(&entry.value);
                return Ok(Some(entry));
            }
            if frame.next_child < frame.loaded.node.children.len() {
                let child_index = frame.next_child;
                let child = frame.loaded.node.children[child_index].1.clone();
                let locator = frame
                    .loaded
                    .child_locators
                    .get(child_index)
                    .cloned()
                    .ok_or_else(|| invalid("packed child locator is missing"))?;
                frame.next_child += 1;
                let child_handle = TreeHandleV2 {
                    reference: child,
                    locator,
                };
                let loaded = load_node_v2(
                    &self.store,
                    &self.descriptor,
                    &child_handle,
                    self.limits,
                    &mut self.work,
                    None,
                    self.io_ledger.as_deref(),
                    deadline,
                    cancelled,
                    shared_work,
                )?;
                if let Some(digest) = loaded.physical_pack_digest {
                    self.pack_capture.observe(digest)?;
                }
                drop(child_handle);
                self.push_node(loaded)?;
                continue;
            }
            let removed = self.stack.pop().expect("checked stream frame");
            self.retained_node_bytes = self.retained_node_bytes
                .checked_sub(removed.retained_state_bytes)
                .ok_or_else(|| budget("packed stream retained state regressed"))?;
        }
    }
}

struct StreamFrameV2 {
    loaded: LoadedTreeNodeV2,
    next_child: usize,
    retained_state_bytes: usize,
}


impl SegmentStore {
    /// Cold-build one immutable Patricia tree from strict raw-byte order.
    pub fn build_authenticated_tree_v1<I>(
        &self,
        kind: &[u8],
        rows: I,
        limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<AuthenticatedTreeDescriptorV1>
    where
        I: IntoIterator<Item = Result<AuthenticatedTreeEntryV1>>,
    {
        self.build_authenticated_tree_v1_with_work(kind, rows, limits, deadline, cancelled)
            .map(|(descriptor, _)| descriptor)
    }

    pub fn build_authenticated_tree_v1_with_work<I>(
        &self,
        kind: &[u8],
        rows: I,
        limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(AuthenticatedTreeDescriptorV1, AuthenticatedTreeWorkV1)>
    where
        I: IntoIterator<Item = Result<AuthenticatedTreeEntryV1>>,
    {
        let limits = limits.validate()?;
        validate_kind(kind, limits)?;
        check(deadline, cancelled)?;
        let _pin_lock = self.hold_generation_pin()?;
        let mut cursor = BuildCursor::new(rows.into_iter());
        let mut work = AuthenticatedTreeWorkV1::default();
        let mut frames = Vec::new();
        frames
            .try_reserve_exact(1)
            .map_err(|_| budget("authenticated builder stack allocation failed"))?;
        frames.push(BuildFrame::default());
        // Rows held by open terminal frames are the only unflushed payloads.
        // Cap their sum by the operation's byte envelope so a long prefix
        // ladder cannot retain an unbounded amount of row data in memory.
        let mut pending_payload_bytes = 0u64;

        loop {
            check(deadline, cancelled)?;
            // `take` advances BuildCursor's previous-key guard, so preserve
            // the old path key for the suffix frames being closed below.
            let previous_key = cursor.previous.clone();
            let Some(row) = cursor.take(limits, deadline, cancelled)? else {
                break;
            };
            let key_depth = row
                .key
                .len()
                .checked_mul(2)
                .ok_or_else(|| budget("tree key depth overflow"))?;
            let common = previous_key
                .as_deref()
                .map_or(0, |previous| common_prefix_nibbles(previous, &row.key));
            if common > key_depth || frames.len() < common.saturating_add(1) {
                return Err(invalid("authenticated builder prefix stack differs"));
            }

            while frames.len() > common + 1 {
                check(deadline, cancelled)?;
                let frame = frames
                    .pop()
                    .ok_or_else(|| invalid("authenticated builder stack disappeared"))?;
                let child = close_build_frame(
                    self,
                    kind,
                    frame,
                    limits,
                    &mut work,
                    &mut pending_payload_bytes,
                    deadline,
                    cancelled,
                )?;
                if let Some(child) = child {
                    let parent_index = frames
                        .len()
                        .checked_sub(1)
                        .ok_or_else(|| invalid("authenticated builder parent is missing"))?;
                    let old_key = previous_key
                        .as_deref()
                        .ok_or_else(|| invalid("authenticated builder previous key is missing"))?;
                    let edge = nibble_at(old_key, parent_index)
                        .ok_or_else(|| invalid("authenticated builder parent edge is missing"))?;
                    attach_build_child(
                        frames.get_mut(parent_index).ok_or_else(|| {
                            invalid("authenticated builder parent frame is missing")
                        })?,
                        edge,
                        child,
                        limits,
                    )?;
                }
            }
            if frames.len() != common + 1 {
                return Err(invalid("authenticated builder common-prefix frame differs"));
            }

            if key_depth < frames.len() - 1 {
                return Err(invalid("authenticated builder key prefix regressed"));
            }
            let added = u64::try_from(row.key.len())
                .ok()
                .and_then(|key_bytes| {
                    u64::try_from(row.value.len())
                        .ok()
                        .and_then(|value_bytes| key_bytes.checked_add(value_bytes))
                })
                .ok_or_else(|| budget("authenticated pending row size overflow"))?;
            pending_payload_bytes = pending_payload_bytes
                .checked_add(added)
                .ok_or_else(|| budget("authenticated pending payload overflow"))?;
            if pending_payload_bytes > limits.max_total_bytes {
                return Err(budget(
                    "authenticated pending payload exceeds operation byte envelope",
                ));
            }

            let needed = key_depth
                .checked_add(1)
                .and_then(|length| length.checked_sub(frames.len()))
                .ok_or_else(|| invalid("authenticated builder stack length differs"))?;
            frames
                .try_reserve_exact(needed)
                .map_err(|_| budget("authenticated builder stack allocation failed"))?;
            for depth in frames.len()..=key_depth {
                check(deadline, cancelled)?;
                if nibble_at(&row.key, depth - 1).is_none() {
                    return Err(invalid("authenticated builder path nibble is missing"));
                }
                frames.push(BuildFrame::default());
            }
            let terminal = frames
                .get_mut(key_depth)
                .ok_or_else(|| invalid("authenticated builder terminal frame is missing"))?;
            if terminal.value.is_some() {
                return Err(SegmentError::new(
                    Code::InvalidFormat,
                    "authenticated builder encountered duplicate key",
                ));
            }
            terminal.value = Some(row);
        }

        while frames.len() > 1 {
            check(deadline, cancelled)?;
            let frame = frames
                .pop()
                .ok_or_else(|| invalid("authenticated builder stack disappeared"))?;
            let child = close_build_frame(
                self,
                kind,
                frame,
                limits,
                &mut work,
                &mut pending_payload_bytes,
                deadline,
                cancelled,
            )?;
            if let Some(child) = child {
                let parent_index = frames
                    .len()
                    .checked_sub(1)
                    .ok_or_else(|| invalid("authenticated builder parent is missing"))?;
                let old_key = cursor
                    .previous
                    .as_deref()
                    .ok_or_else(|| invalid("authenticated builder previous key is missing"))?;
                let edge = nibble_at(old_key, parent_index)
                    .ok_or_else(|| invalid("authenticated builder parent edge is missing"))?;
                attach_build_child(
                    frames
                        .get_mut(parent_index)
                        .ok_or_else(|| invalid("authenticated builder parent frame is missing"))?,
                    edge,
                    child,
                    limits,
                )?;
            }
        }
        let root_frame = frames
            .pop()
            .ok_or_else(|| invalid("authenticated builder root frame is missing"))?;
        let root = close_build_frame(
            self,
            kind,
            root_frame,
            limits,
            &mut work,
            &mut pending_payload_bytes,
            deadline,
            cancelled,
        )?;
        if pending_payload_bytes != 0 {
            return Err(invalid(
                "authenticated builder left pending terminal payload",
            ));
        }
        let entries = cursor.consumed;
        let descriptor = make_descriptor(self, kind, root, entries)?;
        Ok((descriptor, work))
    }

    /// Apply a strict ordered delta. Every changed node is a new immutable
    /// object; untouched child addresses and all historical roots are retained.
    pub fn apply_authenticated_tree_delta_v1<I>(
        &self,
        old: &AuthenticatedTreeDescriptorV1,
        changes: I,
        limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<AuthenticatedTreeDescriptorV1>
    where
        I: IntoIterator<Item = Result<AuthenticatedTreeDeltaV1>>,
    {
        self.apply_authenticated_tree_delta_v1_with_work(old, changes, limits, deadline, cancelled)
            .map(|(descriptor, _)| descriptor)
    }

    pub fn apply_authenticated_tree_delta_v1_with_work<I>(
        &self,
        old: &AuthenticatedTreeDescriptorV1,
        changes: I,
        limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(AuthenticatedTreeDescriptorV1, AuthenticatedTreeWorkV1)>
    where
        I: IntoIterator<Item = Result<AuthenticatedTreeDeltaV1>>,
    {
        let limits = limits.validate()?;
        validate_descriptor_for_store(self, old, limits)?;
        check(deadline, cancelled)?;
        let _pin_lock = self.hold_generation_pin()?;
        let mut work = AuthenticatedTreeWorkV1::default();
        let mut root = old.root.clone();
        let mut previous: Option<Vec<u8>> = None;
        let mut changed_rows = 0u64;
        for change in changes {
            check(deadline, cancelled)?;
            let change = change?;
            validate_key_value(&change.key, change.value.as_deref(), limits)?;
            if previous
                .as_deref()
                .is_some_and(|key| change.key.as_slice() <= key)
            {
                return Err(SegmentError::new(
                    Code::InvalidFormat,
                    "authenticated delta keys are not strictly ordered",
                ));
            }
            changed_rows = changed_rows
                .checked_add(1)
                .ok_or_else(|| budget("tree delta row counter overflow"))?;
            if changed_rows > limits.max_rows {
                return Err(budget("authenticated delta row limit exceeded"));
            }
            previous = Some(change.key.clone());
            root = update_one(
                self, old, root, change, limits, &mut work, deadline, cancelled,
            )?;
        }
        let entries = root.as_ref().map_or(0, |reference| reference.entries);
        let descriptor = make_descriptor(self, &old.kind, root, entries)?;
        Ok((descriptor, work))
    }

    /// Exact lookup; absence is returned only after every node on the relevant
    /// authenticated path has been reopened and checked.
    pub fn lookup_authenticated_tree_v1(
        &self,
        descriptor: &AuthenticatedTreeDescriptorV1,
        key: &[u8],
        limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<Vec<u8>>> {
        self.lookup_authenticated_tree_v1_with_work(descriptor, key, limits, deadline, cancelled)
            .map(|(value, _)| value)
    }

    pub fn lookup_authenticated_tree_v1_with_work(
        &self,
        descriptor: &AuthenticatedTreeDescriptorV1,
        key: &[u8],
        limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(Option<Vec<u8>>, AuthenticatedTreeWorkV1)> {
        let limits = limits.validate()?;
        validate_descriptor_for_store(self, descriptor, limits)?;
        validate_key_value(key, None, limits)?;
        check(deadline, cancelled)?;
        let _pin_lock = self.hold_generation_pin()?;
        let mut work = AuthenticatedTreeWorkV1::default();
        let Some(mut reference) = descriptor.root.clone() else {
            return Ok((None, work));
        };
        loop {
            let node = load_node(
                self, descriptor, &reference, limits, &mut work, deadline, cancelled,
            )?;
            let prefix_len = node_prefix_nibbles(&node);
            if !key_matches_prefix(key, &node.min_key, prefix_len) {
                return Ok((None, work));
            }
            let key_nibbles = key
                .len()
                .checked_mul(2)
                .ok_or_else(|| budget("tree key depth overflow"))?;
            if key_nibbles == prefix_len {
                return Ok((node.value.map(|entry| entry.value), work));
            }
            let Some(edge) = nibble_at(key, prefix_len) else {
                return Ok((None, work));
            };
            let Some((_, child)) = node
                .children
                .iter()
                .find(|(child_edge, _)| *child_edge == edge)
            else {
                return Ok((None, work));
            };
            reference = child.clone();
        }
    }

    /// Open a bounded ordered stream. Full cold traversal limits are checked as
    /// rows and nodes are consumed, not by comparing against root size at open.
    pub fn stream_authenticated_tree_v1(
        &self,
        descriptor: &AuthenticatedTreeDescriptorV1,
        limits: AuthenticatedTreeLimitsV1,
    ) -> Result<AuthenticatedTreeRowStreamV1> {
        let limits = limits.validate()?;
        validate_descriptor_for_store(self, descriptor, limits)?;
        if descriptor.entries > limits.max_rows {
            return Err(budget("authenticated stream row limit exceeded"));
        }
        let mut transcript = Digest256Hasher::new();
        transcript.update(b"tos-authenticated-tree-stream-v1\0");
        transcript.update(descriptor.commitment.as_bytes());
        Ok(AuthenticatedTreeRowStreamV1 {
            store: self.clone(),
            descriptor: descriptor.clone(),
            limits,
            _pin_lock: Arc::new(self.hold_generation_pin()?),
            stack: Vec::new(),
            started: false,
            observed: 0,
            transcript,
            work: AuthenticatedTreeWorkV1::default(),
            done: false,
            failed: false,
        })
    }

    /// Traverse and verify every addressed node under an explicit cold budget.
    pub fn verify_authenticated_tree_v1(
        &self,
        descriptor: &AuthenticatedTreeDescriptorV1,
        limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<AuthenticatedTreeCoverageV1> {
        let mut stream = self.stream_authenticated_tree_v1(descriptor, limits)?;
        while stream.next_row(deadline, cancelled)?.is_some() {}
        stream.coverage().ok_or_else(|| {
            SegmentError::new(
                Code::InvalidReceipt,
                "authenticated tree coverage unavailable",
            )
        })
    }

    /// Store bounded CMD-owned descriptor bytes with exact custody-domain and
    /// kind framing. `max_bytes` covers the complete encoded object.
    pub fn install_authenticated_object_v1(
        &self,
        kind: &[u8],
        payload: &[u8],
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Digest256> {
        self.install_authenticated_object_v1_with_work(
            kind, payload, max_bytes, deadline, cancelled,
        )
        .map(|(digest, _)| digest)
    }

    pub fn install_authenticated_object_v1_with_work(
        &self,
        kind: &[u8],
        payload: &[u8],
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(Digest256, AuthenticatedTreeWorkV1)> {
        if kind.is_empty() || kind.len() > u16::MAX as usize || payload.len() > u64::MAX as usize {
            return Err(budget("authenticated object kind or payload exceeds limit"));
        }
        check(deadline, cancelled)?;
        let raw = encode_authenticated_object(
            self.store_id(),
            self.domain_digest(),
            kind,
            payload,
            max_bytes,
        )?;
        let digest = Digest256::of_bytes(&raw);
        let _pin_lock = self.hold_generation_pin()?;
        let install =
            self.install_authenticated_blob(digest, &raw, max_bytes, deadline, cancelled)?;
        let work = work_from_install(install);
        Ok((digest, work))
    }

    pub fn read_authenticated_object_v1(
        &self,
        kind: &[u8],
        digest: Digest256,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Vec<u8>> {
        self.read_authenticated_object_v1_with_work(kind, digest, max_bytes, deadline, cancelled)
            .map(|(payload, _)| payload)
    }

    pub fn read_authenticated_object_v1_with_work(
        &self,
        kind: &[u8],
        digest: Digest256,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(Vec<u8>, AuthenticatedTreeWorkV1)> {
        if kind.is_empty() || kind.len() > u16::MAX as usize {
            return Err(invalid("authenticated object kind differs"));
        }
        check(deadline, cancelled)?;
        let _pin_lock = self.hold_generation_pin()?;
        let raw = self.read_authenticated_blob(digest, max_bytes, deadline, cancelled)?;
        let payload =
            decode_authenticated_object(&raw, self.store_id(), self.domain_digest(), kind)?;
        let work = AuthenticatedTreeWorkV1 {
            read_nodes: 1,
            read_bytes: raw.len() as u64,
            written_nodes: 0,
            written_bytes: 0,
            allocated_bytes: 0,
        };
        Ok((payload, work))
    }
}

#[derive(Debug)]
struct PackSealStateV2 {
    pack_id: [u8; 16],
    digest: OnceLock<Digest256>,
}

#[derive(Clone, Debug)]
struct PackedHandleV2 {
    state: Arc<PackSealStateV2>,
    offset: u64,
    frame_len: u32,
    frame_sha256: Digest256,
}

#[derive(Clone, Debug)]
enum TreeLocatorV2 {
    Legacy(Digest256),
    Packed(PackedHandleV2),
}

#[derive(Clone, Debug)]
struct TreeHandleV2 {
    reference: AuthenticatedTreeNodeRefV1,
    locator: TreeLocatorV2,
}

struct LoadedTreeNodeV2 {
    node: TreeNode,
    child_locators: Vec<TreeLocatorV2>,
    physical_pack_digest: Option<Digest256>,
}

struct BuildFrameV2 {
    value: Option<AuthenticatedTreeEntryV1>,
    children: Vec<(u8, TreeHandleV2)>,
}

impl Default for BuildFrameV2 {
    fn default() -> Self {
        Self {
            value: None,
            children: Vec::new(),
        }
    }
}

struct ActivePackV2 {
    state: Arc<PackSealStateV2>,
    raw: Vec<u8>,
    frame_count: u64,
}

struct PackWriterV2<'a, 'shared> {
    store: &'a SegmentStore,
    limits: AuthenticatedTreeLimitsV1,
    pack_cap: usize,
    work: &'a mut AuthenticatedTreeWorkV1,
    io_ledger: Option<Arc<dyn AuthenticatedTreeIoLedgerV1>>,
    active: Option<ActivePackV2>,
    state_limit: Option<CowStateLimitV2>,
    delta_live_state_bytes: usize,
    root_live_state_bytes: usize,
    deadline: Instant,
    cancelled: &'a AtomicBool,
    shared_work: Option<&'shared mut dyn FnMut() -> bool>,
}

#[derive(Clone, Copy)]
struct CowStateLimitV2 {
    maximum_bytes: usize,
    additional_live_bytes: usize,
    old_descriptor_bytes: usize,
}

impl<'a, 'shared> PackWriterV2<'a, 'shared> {
    fn new(
        store: &'a SegmentStore,
        limits: AuthenticatedTreeLimitsV1,
        pack_cap: usize,
        work: &'a mut AuthenticatedTreeWorkV1,
        io_ledger: Option<Arc<dyn AuthenticatedTreeIoLedgerV1>>,
        deadline: Instant,
        cancelled: &'a AtomicBool,
        shared_work: Option<&'shared mut dyn FnMut() -> bool>,
    ) -> Self {
        Self {
            store,
            limits,
            pack_cap,
            work,
            io_ledger,
            active: None,
            state_limit: None,
            delta_live_state_bytes: 0,
            root_live_state_bytes: 0,
            deadline,
            cancelled,
            shared_work,
        }
    }

    fn debit_shared_work(&mut self) -> Result<()> {
        if let Some(charge) = self.shared_work.as_mut() {
            if !(**charge)() {
                return Err(budget("authenticated tree shared work refused"));
            }
        }
        Ok(())
    }

    fn with_state_limit(mut self, limit: CowStateLimitV2) -> Self {
        self.state_limit = Some(limit);
        self
    }

    fn check_cow_state(&self, transient_bytes: usize) -> Result<()> {
        let Some(limit) = self.state_limit else {
            return Ok(());
        };
        let store_heap_state = self.store.retained_heap_state_bytes()?;
        let seal_allocation_state = pack_seal_arc_allocation_state_bytes()?;
        let active_bytes = match self.active.as_ref() {
            Some(active) => std::mem::size_of::<ActivePackV2>()
                .checked_add(active.raw.capacity())
                .and_then(|bytes| bytes.checked_add(seal_allocation_state))
                .ok_or_else(|| budget("authenticated COW active pack state overflow"))?,
            None => 0,
        };
        let total = std::mem::size_of::<Self>()
            .checked_add(std::mem::size_of::<AuthenticatedTreeWorkV1>())
            .and_then(|n| n.checked_add(std::mem::size_of::<SegmentStore>()))
            .and_then(|n| n.checked_add(store_heap_state))
            .and_then(|n| n.checked_add(limit.additional_live_bytes))
            .and_then(|n| n.checked_add(limit.old_descriptor_bytes))
            .and_then(|n| n.checked_add(self.delta_live_state_bytes))
            .and_then(|n| n.checked_add(self.root_live_state_bytes))
            .and_then(|n| n.checked_add(active_bytes))
            .and_then(|n| n.checked_add(transient_bytes))
            .ok_or_else(|| budget("authenticated COW state bound overflow"))?;
        if total > limit.maximum_bytes {
            return Err(budget("authenticated COW state profile exceeded"));
        }
        Ok(())
    }

    fn persist(
        &mut self,
        kind: &[u8],
        node: TreeNode,
        child_locators: &[TreeLocatorV2],
        child_locator_capacity: usize,
    ) -> Result<TreeHandleV2> {
        check(self.deadline, self.cancelled)?;
        self.debit_shared_work()?;
        validate_node(&node, self.limits)?;
        if child_locators.len() != node.children.len() {
            return Err(invalid("packed child locator count differs"));
        }
        for ((_, child), locator) in node.children.iter().zip(child_locators) {
            if let TreeLocatorV2::Legacy(digest) = locator {
                if *digest != child.digest {
                    return Err(invalid("legacy child locator digest differs"));
                }
            }
        }
        let encoded_len = node_encoded_len(kind, &node)?;
        let candidate_bound = encoded_len
            .checked_add(45)
            .and_then(|n| n.checked_add(child_locators.len().checked_mul(93)?))
            .ok_or_else(|| budget("authenticated COW frame state overflow"))?;
        let persist_scratch = encoded_len
            .checked_add(
                candidate_bound
                    .checked_mul(2)
                    .ok_or_else(|| budget("authenticated COW frame state overflow"))?,
            )
            .and_then(|n| n.checked_add(node.min_key.len()))
            .and_then(|n| n.checked_add(node.max_key.len()))
            .and_then(|n| n.checked_add(std::mem::size_of::<TreeNode>()))
            .and_then(|n| n.checked_add(64 * 1024))
            .ok_or_else(|| budget("authenticated COW persist state overflow"))?;
        let per_child_state = std::mem::size_of::<TreeLocatorV2>()
            .checked_add(pack_seal_arc_allocation_state_bytes()?)
            .ok_or_else(|| budget("authenticated COW persist state overflow"))?;
        let persist_scratch = persist_scratch
            .checked_add(
                child_locators
                    .len()
                    .checked_mul(per_child_state)
                    .ok_or_else(|| budget("authenticated COW persist state overflow"))?,
            )
            .ok_or_else(|| budget("authenticated COW persist state overflow"))?;
        let node_state = tree_node_state_bytes(&node)?
            .checked_add(tree_locator_slice_state_bytes(
                child_locators,
                child_locator_capacity,
            )?)
            .ok_or_else(|| budget("authenticated COW node state overflow"))?;
        self.check_cow_state(
            node_state
                .checked_add(persist_scratch)
                .ok_or_else(|| budget("authenticated COW persist state overflow"))?,
        )?;
        let canonical = encode_node(
            self.store.store_id(),
            self.store.domain_digest(),
            kind,
            &node,
            self.limits,
        )?;
        if canonical.len() > self.limits.max_node_bytes {
            return Err(budget("authenticated node exceeds byte limit"));
        }
        let digest = Digest256::of_bytes(&canonical);
        let mut reference = AuthenticatedTreeNodeRefV1 {
            digest,
            entries: node.entries,
            min_key: node.min_key.clone(),
            max_key: node.max_key.clone(),
        };
        let mut candidate = self.encode_candidate(&canonical, child_locators)?;
        let current_len = self.active.as_ref().map_or(0, |pack| pack.raw.len());
        if current_len
            .checked_add(candidate.len())
            .is_none_or(|len| len > self.pack_cap)
        {
            if current_len == 0 {
                return Err(budget("authenticated node frame exceeds pack limit"));
            }
            self.flush()?;
            candidate = self.encode_candidate(&canonical, child_locators)?;
            if candidate.len() > self.pack_cap {
                return Err(budget("authenticated node frame exceeds pack limit"));
            }
        }
        let (offset, active_len, active_frames) = self
            .active
            .as_ref()
            .map(|active| (active.raw.len(), active.raw.len(), active.frame_count))
            .ok_or_else(|| invalid("authenticated pack writer has no active chunk"))?;
        let offset = u64::try_from(offset)
            .map_err(|_| budget("authenticated pack offset exceeds address space"))?;
        let frame_len = u32::try_from(candidate.len())
            .map_err(|_| budget("authenticated node frame is too long"))?;
        let frame_sha256 = Digest256::of_bytes(&candidate);
        let future_bytes = (candidate.len() as u64)
            .checked_add(active_len as u64)
            .and_then(|bytes| bytes.checked_mul(2))
            .ok_or_else(|| budget("authenticated pack byte reservation overflow"))?;
        let future_nodes = active_frames
            .checked_add(1)
            .and_then(|nodes| nodes.checked_mul(2))
            .ok_or_else(|| budget("authenticated pack node reservation overflow"))?;
        if self
            .work
            .total_bytes()
            .checked_add(future_bytes)
            .is_none_or(|bytes| bytes > self.limits.max_total_bytes)
            || self
                .work
                .total_nodes()
                .checked_add(future_nodes)
                .is_none_or(|nodes| nodes > self.limits.max_nodes)
        {
            return Err(budget("authenticated pack exceeds operation budget"));
        }
        let (current_capacity, requested_capacity) = self
            .active
            .as_ref()
            .map(|active| {
                let requested = active
                    .raw
                    .len()
                    .checked_add(candidate.len())
                    .ok_or_else(|| budget("authenticated pack capacity overflow"))?;
                Ok((active.raw.capacity(), requested))
            })
            .ok_or_else(|| invalid("authenticated pack writer has no active chunk"))??;
        let anticipated_capacity = current_capacity.max(requested_capacity);
        self.check_cow_state(anticipated_capacity.saturating_sub(current_capacity))?;
        self.active
            .as_mut()
            .ok_or_else(|| invalid("authenticated pack writer has no active chunk"))?
            .raw
            .try_reserve_exact(candidate.len())
            .map_err(|_| budget("authenticated pack allocation failed"))?;
        self.check_cow_state(0)?;
        let active = self
            .active
            .as_mut()
            .ok_or_else(|| invalid("authenticated pack writer lost its active chunk"))?;
        active.raw.extend_from_slice(&candidate);
        active.frame_count = active
            .frame_count
            .checked_add(1)
            .ok_or_else(|| budget("authenticated pack frame counter overflow"))?;
        // Keep ownership explicit in the semantic summary; it does not include
        // the physical coordinates returned below.
        reference.min_key = node.min_key;
        reference.max_key = node.max_key;
        Ok(TreeHandleV2 {
            reference,
            locator: TreeLocatorV2::Packed(PackedHandleV2 {
                state: active.state.clone(),
                offset,
                frame_len,
                frame_sha256,
            }),
        })
    }

    fn encode_candidate(
        &mut self,
        canonical: &[u8],
        child_locators: &[TreeLocatorV2],
    ) -> Result<Vec<u8>> {
        if self.active.is_none() {
            let mut pack_id = [0u8; 16];
            getrandom::fill(&mut pack_id)
                .map_err(|_| SegmentError::new(Code::Io, "cannot create pack ID"))?;
            self.active = Some(ActivePackV2 {
                state: Arc::new(PackSealStateV2 {
                    pack_id,
                    digest: OnceLock::new(),
                }),
                raw: Vec::new(),
                frame_count: 0,
            });
        }
        let active = self
            .active
            .as_ref()
            .ok_or_else(|| invalid("authenticated pack writer has no active chunk"))?;
        let offset = u64::try_from(active.raw.len())
            .map_err(|_| budget("authenticated pack offset exceeds address space"))?;
        encode_packed_frame(active.state.pack_id, offset, canonical, child_locators)
    }

    fn flush(&mut self) -> Result<()> {
        check(self.deadline, self.cancelled)?;
        self.check_cow_state(64 * 1024)?;
        let Some(active) = self.active.take() else {
            return Ok(());
        };
        if active.raw.is_empty() {
            return Err(invalid("authenticated pack is empty"));
        }
        let pending_bytes = u64::try_from(active.raw.len())
            .ok()
            .and_then(|bytes| bytes.checked_mul(2))
            .ok_or_else(|| budget("authenticated pack byte reservation overflow"))?;
        let pending_nodes = active
            .frame_count
            .checked_mul(2)
            .ok_or_else(|| budget("authenticated pack node reservation overflow"))?;
        if self
            .work
            .total_bytes()
            .checked_add(pending_bytes)
            .is_none_or(|bytes| bytes > self.limits.max_total_bytes)
            || self
                .work
                .total_nodes()
                .checked_add(pending_nodes)
                .is_none_or(|nodes| nodes > self.limits.max_nodes)
        {
            return Err(budget("authenticated pack exceeds operation budget"));
        }
        let digest = Digest256::of_bytes(&active.raw);
        let io_ledger = self.io_ledger.as_deref();
        let payload_bytes = active.raw.len() as u64;
        charge_tree_read(io_ledger, payload_bytes)?;
        charge_tree_write(io_ledger, payload_bytes)?;
        let install = self.store.install_authenticated_blob_accounted(
            digest,
            &active.raw,
            self.pack_cap,
            self.deadline,
            self.cancelled,
            io_ledger,
        )?;
        record_tree_read(io_ledger, install.read_bytes)?;
        record_tree_write(io_ledger, install.written_bytes)?;
        self.work
            .charge_pack_install(install, active.frame_count, self.limits)?;
        active
            .state
            .digest
            .set(digest)
            .map_err(|_| invalid("authenticated pack digest was already sealed"))?;
        check(self.deadline, self.cancelled)
    }

    fn finish(&mut self) -> Result<()> {
        self.flush()
    }
}

impl SegmentStore {
    pub fn build_authenticated_tree_v2<I>(
        &self,
        kind: &[u8],
        rows: I,
        limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<AuthenticatedTreeDescriptorV2>
    where
        I: IntoIterator<Item = Result<AuthenticatedTreeEntryV1>>,
    {
        self.build_authenticated_tree_v2_with_work(kind, rows, limits, deadline, cancelled)
            .map(|(descriptor, _)| descriptor)
    }

    pub fn build_authenticated_tree_v2_with_work<I>(
        &self,
        kind: &[u8],
        rows: I,
        limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(AuthenticatedTreeDescriptorV2, AuthenticatedTreeWorkV1)>
    where
        I: IntoIterator<Item = Result<AuthenticatedTreeEntryV1>>,
    {
        self.build_authenticated_tree_v2_with_work_and_io(
            kind, rows, limits, None, deadline, cancelled,
        )
    }

    /// Budget-aware form used by CMD admission. Every physical pack read/write
    /// is charged before the store touches it and reconciled after return.
    pub fn build_authenticated_tree_v2_with_work_and_io<I>(
        &self,
        kind: &[u8],
        rows: I,
        limits: AuthenticatedTreeLimitsV1,
        io_ledger: Option<Arc<dyn AuthenticatedTreeIoLedgerV1>>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(AuthenticatedTreeDescriptorV2, AuthenticatedTreeWorkV1)>
    where
        I: IntoIterator<Item = Result<AuthenticatedTreeEntryV1>>,
    {
        self.build_authenticated_tree_v2_with_pack_cap(
            kind,
            rows,
            limits,
            AUTHENTICATED_PACK_MAX_BYTES,
            io_ledger,
            None,
            None,
            deadline,
            cancelled,
        )
    }

    /// State-limited bulk build form. `additional_live_state_bytes` covers
    /// caller-owned state and dynamic heap retained by the row iterator; this
    /// method accounts its cursor, frame stack, copied keys, child handles,
    /// node construction and pack writer before allocation or growth.
    pub fn build_authenticated_tree_v2_with_work_and_io_and_state<I>(
        &self,
        kind: &[u8],
        rows: I,
        limits: AuthenticatedTreeLimitsV1,
        io_ledger: Option<Arc<dyn AuthenticatedTreeIoLedgerV1>>,
        max_working_state_bytes: usize,
        additional_live_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(AuthenticatedTreeDescriptorV2, AuthenticatedTreeWorkV1)>
    where
        I: IntoIterator<Item = Result<AuthenticatedTreeEntryV1>>,
    {
        self.build_authenticated_tree_v2_with_pack_cap(
            kind,
            rows,
            limits,
            AUTHENTICATED_PACK_MAX_BYTES,
            io_ledger,
            Some((max_working_state_bytes, additional_live_state_bytes)),
            None,
            deadline,
            cancelled,
        )
    }

    /// State-limited bulk build that also debits the caller's shared
    /// invocation work meter before each tree-node visit or node creation.
    pub fn build_authenticated_tree_v2_with_work_and_io_and_state_and_callback<I>(
        &self,
        kind: &[u8],
        rows: I,
        limits: AuthenticatedTreeLimitsV1,
        io_ledger: Option<Arc<dyn AuthenticatedTreeIoLedgerV1>>,
        max_working_state_bytes: usize,
        additional_live_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
        callback: &mut dyn FnMut() -> bool,
    ) -> Result<(AuthenticatedTreeDescriptorV2, AuthenticatedTreeWorkV1)>
    where
        I: IntoIterator<Item = Result<AuthenticatedTreeEntryV1>>,
    {
        self.build_authenticated_tree_v2_with_pack_cap(
            kind,
            rows,
            limits,
            AUTHENTICATED_PACK_MAX_BYTES,
            io_ledger,
            Some((max_working_state_bytes, additional_live_state_bytes)),
            Some(callback),
            deadline,
            cancelled,
        )
    }

    fn build_authenticated_tree_v2_with_pack_cap<I>(
        &self,
        kind: &[u8],
        rows: I,
        limits: AuthenticatedTreeLimitsV1,
        pack_cap: usize,
        io_ledger: Option<Arc<dyn AuthenticatedTreeIoLedgerV1>>,
        state_limit: Option<(usize, usize)>,
        mut shared_work: Option<&mut dyn FnMut() -> bool>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(AuthenticatedTreeDescriptorV2, AuthenticatedTreeWorkV1)>
    where
        I: IntoIterator<Item = Result<AuthenticatedTreeEntryV1>>,
    {
        if pack_cap == 0 || pack_cap > AUTHENTICATED_PACK_MAX_BYTES {
            return Err(budget("authenticated pack limit is invalid"));
        }
        let limits = limits.validate()?;
        validate_kind(kind, limits)?;
        check(deadline, cancelled)?;
        let _pin_lock = self.hold_generation_pin()?;
        let mut work = AuthenticatedTreeWorkV1::default();
        let mut writer = PackWriterV2::new(
            self,
            limits,
            pack_cap,
            &mut work,
            io_ledger,
            deadline,
            cancelled,
            shared_work.take(),
        );
        if let Some((maximum_bytes, additional_live_bytes)) = state_limit {
            writer = writer.with_state_limit(CowStateLimitV2 {
                maximum_bytes,
                additional_live_bytes,
                old_descriptor_bytes: 0,
            });
            writer.check_cow_state(0)?;
        }
        let mut cursor = BuildCursor::new(rows.into_iter());
        let mut frames = Vec::new();
        check_build_state(&mut writer, &frames, &cursor, limits, 0)?;
        let initial_frame_bytes = std::mem::size_of::<BuildFrameV2>();
        check_build_state(&mut writer, &frames, &cursor, limits, initial_frame_bytes)?;
        frames
            .try_reserve_exact(1)
            .map_err(|_| budget("packed authenticated builder stack allocation failed"))?;
        check_build_state(&mut writer, &frames, &cursor, limits, 0)?;
        frames.push(BuildFrameV2::default());
        check_build_state(&mut writer, &frames, &cursor, limits, 0)?;
        let mut pending_payload_bytes = 0u64;
        loop {
            check(deadline, cancelled)?;
            check_build_state(&mut writer, &frames, &cursor, limits, limits.max_key_bytes)?;
            let previous_key = cursor.previous.clone();
            let previous_key_bytes = previous_key.as_ref().map_or(0, Vec::capacity);
            check_build_state(&mut writer, &frames, &cursor, limits, previous_key_bytes)?;
            let next_row_upper = limits
                .max_key_bytes
                .checked_mul(2)
                .and_then(|bytes| bytes.checked_add(limits.max_value_bytes))
                .and_then(|bytes| bytes.checked_add(previous_key_bytes))
                .ok_or_else(|| budget("packed builder row state overflow"))?;
            check_build_state(&mut writer, &frames, &cursor, limits, next_row_upper)?;
            let Some(row) = cursor.take(limits, deadline, cancelled)? else {
                break;
            };
            let row_state = previous_key_bytes
                .checked_add(entry_state_bytes(&row)?)
                .ok_or_else(|| budget("packed builder row state overflow"))?;
            check_build_state(&mut writer, &frames, &cursor, limits, row_state)?;
            let key_depth = row
                .key
                .len()
                .checked_mul(2)
                .ok_or_else(|| budget("tree key depth overflow"))?;
            let common = previous_key
                .as_deref()
                .map_or(0, |previous| common_prefix_nibbles(previous, &row.key));
            if common > key_depth || frames.len() < common.saturating_add(1) {
                return Err(invalid("packed builder prefix stack differs"));
            }
            while frames.len() > common + 1 {
                check(deadline, cancelled)?;
                let frame = frames
                    .pop()
                    .ok_or_else(|| invalid("packed builder stack disappeared"))?;
                let retained_builder_state = build_frame_stack_state_bytes(&frames)?
                    .checked_add(build_cursor_state_bytes(&cursor)?)
                    .and_then(|bytes| bytes.checked_add(row_state))
                    .ok_or_else(|| budget("packed builder retained state overflow"))?;
                if let Some(child) = close_build_frame_v2(
                    &mut writer,
                    kind,
                    frame,
                    limits,
                    retained_builder_state,
                    &mut pending_payload_bytes,
                    deadline,
                    cancelled,
                )? {
                    let parent_index = frames
                        .len()
                        .checked_sub(1)
                        .ok_or_else(|| invalid("packed builder parent is missing"))?;
                    let old_key = previous_key
                        .as_deref()
                        .ok_or_else(|| invalid("packed builder previous key is missing"))?;
                    let edge = nibble_at(old_key, parent_index)
                        .ok_or_else(|| invalid("packed builder parent edge is missing"))?;
                    let retained_builder_state = build_frame_stack_state_bytes(&frames)?
                        .checked_add(build_cursor_state_bytes(&cursor)?)
                        .and_then(|bytes| bytes.checked_add(row_state))
                        .ok_or_else(|| budget("packed builder retained state overflow"))?;
                    let parent = frames
                        .get_mut(parent_index)
                        .ok_or_else(|| invalid("packed builder parent frame is missing"))?;
                    attach_build_child_v2(
                        &mut writer,
                        parent,
                        edge,
                        child,
                        limits,
                        retained_builder_state,
                    )?;
                    check_build_state(&mut writer, &frames, &cursor, limits, row_state)?;
                }
            }
            if frames.len() != common + 1 {
                return Err(invalid("packed builder common-prefix frame differs"));
            }
            if key_depth < frames.len() - 1 {
                return Err(invalid("packed builder key prefix regressed"));
            }
            let added = u64::try_from(row.key.len())
                .ok()
                .and_then(|key| {
                    u64::try_from(row.value.len())
                        .ok()
                        .and_then(|value| key.checked_add(value))
                })
                .ok_or_else(|| budget("packed pending row size overflow"))?;
            pending_payload_bytes = pending_payload_bytes
                .checked_add(added)
                .ok_or_else(|| budget("packed pending payload overflow"))?;
            if pending_payload_bytes > limits.max_total_bytes {
                return Err(budget(
                    "packed pending payload exceeds operation byte envelope",
                ));
            }
            let needed = key_depth
                .checked_add(1)
                .and_then(|length| length.checked_sub(frames.len()))
                .ok_or_else(|| invalid("packed builder stack length differs"))?;
            let requested_capacity = frames
                .len()
                .checked_add(needed)
                .ok_or_else(|| budget("packed builder stack capacity overflow"))?;
            let requested_bytes = requested_capacity
                .checked_mul(std::mem::size_of::<BuildFrameV2>())
                .ok_or_else(|| budget("packed builder stack state overflow"))?;
            let current_bytes = frames
                .capacity()
                .checked_mul(std::mem::size_of::<BuildFrameV2>())
                .ok_or_else(|| budget("packed builder stack state overflow"))?;
            check_build_state(
                &mut writer,
                &frames,
                &cursor,
                limits,
                row_state
                    .checked_add(requested_bytes.saturating_sub(current_bytes))
                    .ok_or_else(|| budget("packed builder stack state overflow"))?,
            )?;
            frames
                .try_reserve_exact(needed)
                .map_err(|_| budget("packed builder stack allocation failed"))?;
            check_build_state(&mut writer, &frames, &cursor, limits, row_state)?;
            for depth in frames.len()..=key_depth {
                check(deadline, cancelled)?;
                if nibble_at(&row.key, depth - 1).is_none() {
                    return Err(invalid("packed builder path nibble is missing"));
                }
                frames.push(BuildFrameV2::default());
            }
            let terminal = frames
                .get_mut(key_depth)
                .ok_or_else(|| invalid("packed builder terminal frame is missing"))?;
            if terminal.value.is_some() {
                return Err(SegmentError::new(
                    Code::InvalidFormat,
                    "packed builder encountered duplicate key",
                ));
            }
            terminal.value = Some(row);
            check_build_state(&mut writer, &frames, &cursor, limits, previous_key_bytes)?;
        }

        while frames.len() > 1 {
            check(deadline, cancelled)?;
            let frame = frames
                .pop()
                .ok_or_else(|| invalid("packed builder stack disappeared"))?;
            let retained_builder_state = build_frame_stack_state_bytes(&frames)?
                .checked_add(build_cursor_state_bytes(&cursor)?)
                .ok_or_else(|| budget("packed builder retained state overflow"))?;
            if let Some(child) = close_build_frame_v2(
                &mut writer,
                kind,
                frame,
                limits,
                retained_builder_state,
                &mut pending_payload_bytes,
                deadline,
                cancelled,
            )? {
                let parent_index = frames
                    .len()
                    .checked_sub(1)
                    .ok_or_else(|| invalid("packed builder parent is missing"))?;
                let old_key = cursor
                    .previous
                    .as_deref()
                    .ok_or_else(|| invalid("packed builder previous key is missing"))?;
                let edge = nibble_at(old_key, parent_index)
                    .ok_or_else(|| invalid("packed builder parent edge is missing"))?;
                let retained_builder_state = build_frame_stack_state_bytes(&frames)?
                    .checked_add(build_cursor_state_bytes(&cursor)?)
                    .ok_or_else(|| budget("packed builder retained state overflow"))?;
                let parent = frames
                    .get_mut(parent_index)
                    .ok_or_else(|| invalid("packed builder parent frame is missing"))?;
                attach_build_child_v2(
                    &mut writer,
                    parent,
                    edge,
                    child,
                    limits,
                    retained_builder_state,
                )?;
                check_build_state(&mut writer, &frames, &cursor, limits, 0)?;
            }
        }
        let root_frame = frames
            .pop()
            .ok_or_else(|| invalid("packed builder root frame is missing"))?;
        let retained_builder_state = build_frame_stack_state_bytes(&frames)?
            .checked_add(build_cursor_state_bytes(&cursor)?)
            .ok_or_else(|| budget("packed builder retained state overflow"))?;
        let root = close_build_frame_v2(
            &mut writer,
            kind,
            root_frame,
            limits,
            retained_builder_state,
            &mut pending_payload_bytes,
            deadline,
            cancelled,
        )?;
        if pending_payload_bytes != 0 {
            return Err(invalid("packed builder left pending terminal payload"));
        }
        let entries = cursor.consumed;
        let root_state = root
            .as_ref()
            .map(tree_handle_state_bytes)
            .transpose()?
            .unwrap_or(0);
        writer.root_live_state_bytes =
            build_cursor_state_bytes(&cursor)?
                .checked_add(root_state)
                .ok_or_else(|| budget("packed builder root state overflow"))?;
        let descriptor_overhead = std::mem::size_of::<AuthenticatedTreeDescriptorV2>()
            .checked_add(kind.len())
            .and_then(|bytes| bytes.checked_add(limits.max_key_bytes.checked_mul(2)?))
            .ok_or_else(|| budget("packed builder descriptor state overflow"))?;
        writer.check_cow_state(descriptor_overhead)?;
        writer.finish()?;
        let (semantic_root, physical_root) = match root {
            Some(handle) => {
                let locator = public_locator(&handle.locator)?;
                (Some(handle.reference), Some(locator))
            }
            None => (None, None),
        };
        let semantic = make_descriptor(self, kind, semantic_root, entries)?;
        drop(writer);
        Ok((
            AuthenticatedTreeDescriptorV2 {
                semantic,
                physical_root,
            },
            work,
        ))
    }

    pub fn apply_authenticated_tree_delta_v2<I>(
        &self,
        old: &AuthenticatedTreeDescriptorV2,
        changes: I,
        limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<AuthenticatedTreeDescriptorV2>
    where
        I: IntoIterator<Item = Result<AuthenticatedTreeDeltaV1>>,
    {
        self.apply_authenticated_tree_delta_v2_with_work(old, changes, limits, deadline, cancelled)
            .map(|(descriptor, _)| descriptor)
    }

    pub fn apply_authenticated_tree_delta_v2_with_work<I>(
        &self,
        old: &AuthenticatedTreeDescriptorV2,
        changes: I,
        limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(AuthenticatedTreeDescriptorV2, AuthenticatedTreeWorkV1)>
    where
        I: IntoIterator<Item = Result<AuthenticatedTreeDeltaV1>>,
    {
        self.apply_authenticated_tree_delta_v2_with_work_and_io(
            old, changes, limits, None, deadline, cancelled,
        )
    }

    /// Budget-aware COW delta form used by the source admission owner.
    pub fn apply_authenticated_tree_delta_v2_with_work_and_io<I>(
        &self,
        old: &AuthenticatedTreeDescriptorV2,
        changes: I,
        limits: AuthenticatedTreeLimitsV1,
        io_ledger: Option<Arc<dyn AuthenticatedTreeIoLedgerV1>>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(AuthenticatedTreeDescriptorV2, AuthenticatedTreeWorkV1)>
    where
        I: IntoIterator<Item = Result<AuthenticatedTreeDeltaV1>>,
    {
        self.apply_authenticated_tree_delta_v2_with_pack_cap_and_state(
            old,
            changes,
            limits,
            AUTHENTICATED_PACK_MAX_BYTES,
            io_ledger,
            AuthenticatedTreeWorkV1::default(),
            None,
            None,
            deadline,
            cancelled,
        )
    }

    /// State-limited COW delta form. `additional_live_state_bytes` is the
    /// caller's already retained rootset, source cursor, and candidate state;
    /// this operation adds its own decoded-node, ancestor, delta-row, and pack
    /// writer peaks before loading, cloning, or growing those structures.
    pub fn apply_authenticated_tree_delta_v2_with_work_and_io_and_state<I>(
        &self,
        old: &AuthenticatedTreeDescriptorV2,
        changes: I,
        limits: AuthenticatedTreeLimitsV1,
        io_ledger: Option<Arc<dyn AuthenticatedTreeIoLedgerV1>>,
        max_working_state_bytes: usize,
        additional_live_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(AuthenticatedTreeDescriptorV2, AuthenticatedTreeWorkV1)>
    where
        I: IntoIterator<Item = Result<AuthenticatedTreeDeltaV1>>,
    {
        self.apply_authenticated_tree_delta_v2_with_pack_cap_and_state(
            old,
            changes,
            limits,
            AUTHENTICATED_PACK_MAX_BYTES,
            io_ledger,
            AuthenticatedTreeWorkV1::default(),
            Some((max_working_state_bytes, additional_live_state_bytes)),
            None,
            deadline,
            cancelled,
        )
    }

    /// COW form for a sequence of root-family deltas that keeps the caller's
    /// already-spent authenticated tree work in the same finite operation cap.
    pub fn apply_authenticated_tree_delta_v2_with_work_and_io_and_state_cumulative<I>(
        &self,
        old: &AuthenticatedTreeDescriptorV2,
        changes: I,
        limits: AuthenticatedTreeLimitsV1,
        io_ledger: Option<Arc<dyn AuthenticatedTreeIoLedgerV1>>,
        initial_work: AuthenticatedTreeWorkV1,
        max_working_state_bytes: usize,
        additional_live_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(AuthenticatedTreeDescriptorV2, AuthenticatedTreeWorkV1)>
    where
        I: IntoIterator<Item = Result<AuthenticatedTreeDeltaV1>>,
    {
        self.apply_authenticated_tree_delta_v2_with_pack_cap_and_state(
            old,
            changes,
            limits,
            AUTHENTICATED_PACK_MAX_BYTES,
            io_ledger,
            initial_work,
            Some((max_working_state_bytes, additional_live_state_bytes)),
            None,
            deadline,
            cancelled,
        )
    }

    /// State-limited cumulative COW form that debits the caller's original
    /// work meter before each old-node visit and new-node creation.
    pub fn apply_authenticated_tree_delta_v2_with_work_and_io_and_state_cumulative_and_callback<I>(
        &self,
        old: &AuthenticatedTreeDescriptorV2,
        changes: I,
        limits: AuthenticatedTreeLimitsV1,
        io_ledger: Option<Arc<dyn AuthenticatedTreeIoLedgerV1>>,
        initial_work: AuthenticatedTreeWorkV1,
        max_working_state_bytes: usize,
        additional_live_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
        callback: &mut dyn FnMut() -> bool,
    ) -> Result<(AuthenticatedTreeDescriptorV2, AuthenticatedTreeWorkV1)>
    where
        I: IntoIterator<Item = Result<AuthenticatedTreeDeltaV1>>,
    {
        self.apply_authenticated_tree_delta_v2_with_pack_cap_and_state(
            old,
            changes,
            limits,
            AUTHENTICATED_PACK_MAX_BYTES,
            io_ledger,
            initial_work,
            Some((max_working_state_bytes, additional_live_state_bytes)),
            Some(callback),
            deadline,
            cancelled,
        )
    }

    fn apply_authenticated_tree_delta_v2_with_pack_cap_and_state<I>(
        &self,
        old: &AuthenticatedTreeDescriptorV2,
        changes: I,
        limits: AuthenticatedTreeLimitsV1,
        pack_cap: usize,
        io_ledger: Option<Arc<dyn AuthenticatedTreeIoLedgerV1>>,
        initial_work: AuthenticatedTreeWorkV1,
        state_limit: Option<(usize, usize)>,
        mut shared_work: Option<&mut dyn FnMut() -> bool>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(AuthenticatedTreeDescriptorV2, AuthenticatedTreeWorkV1)>
    where
        I: IntoIterator<Item = Result<AuthenticatedTreeDeltaV1>>,
    {
        if pack_cap == 0 || pack_cap > AUTHENTICATED_PACK_MAX_BYTES {
            return Err(budget("authenticated pack limit is invalid"));
        }
        let limits = limits.validate()?;
        validate_descriptor_for_store(self, &old.semantic, limits)?;
        validate_descriptor_v2_shape(old)?;
        if initial_work.total_nodes() > limits.max_nodes
            || initial_work.total_bytes() > limits.max_total_bytes
        {
            return Err(budget(
                "authenticated cumulative tree work is already exceeded",
            ));
        }
        check(deadline, cancelled)?;
        let _pin_lock = self.hold_generation_pin()?;
        let mut work = initial_work;
        let mut writer = PackWriterV2::new(
            self,
            limits,
            pack_cap,
            &mut work,
            io_ledger,
            deadline,
            cancelled,
            shared_work.take(),
        );
        if let Some((maximum_bytes, additional_live_bytes)) = state_limit {
            let old_descriptor_bytes = tree_descriptor_state_bytes(old)?;
            writer = writer.with_state_limit(CowStateLimitV2 {
                maximum_bytes,
                additional_live_bytes,
                old_descriptor_bytes,
            });
            writer.check_cow_state(tree_descriptor_handle_clone_state_bytes(old)?)?;
        }
        let mut root = descriptor_root_handle(old)?;
        if writer.state_limit.is_some() {
            writer.root_live_state_bytes = root
                .as_ref()
                .map(tree_handle_state_bytes)
                .transpose()?
                .unwrap_or(0);
            writer.check_cow_state(0)?;
        }
        let mut previous: Option<Vec<u8>> = None;
        let mut changed_rows = 0u64;
        for change in changes {
            check(deadline, cancelled)?;
            let change = change?;
            validate_key_value(&change.key, change.value.as_deref(), limits)?;
            if previous
                .as_deref()
                .is_some_and(|key| change.key.as_slice() <= key)
            {
                return Err(SegmentError::new(
                    Code::InvalidFormat,
                    "authenticated delta keys are not strictly ordered",
                ));
            }
            changed_rows = changed_rows
                .checked_add(1)
                .ok_or_else(|| budget("tree delta row counter overflow"))?;
            if changed_rows > limits.max_rows {
                return Err(budget("authenticated delta row limit exceeded"));
            }
            if writer.state_limit.is_some() {
                let row_state = delta_row_state_bytes(&change)?;
                let previous_capacity = previous.as_ref().map_or(0, Vec::capacity);
                let new_previous_capacity = change.key.len();
                writer.delta_live_state_bytes = row_state
                    .checked_add(previous_capacity)
                    .and_then(|n| n.checked_add(new_previous_capacity))
                    .ok_or_else(|| budget("authenticated delta row state overflow"))?;
                writer.check_cow_state(0)?;
            }
            let mut next_previous = Vec::new();
            next_previous
                .try_reserve_exact(change.key.len())
                .map_err(|_| budget("authenticated delta key allocation failed"))?;
            next_previous.extend_from_slice(&change.key);
            let next_previous_capacity = next_previous.capacity();
            previous = Some(next_previous);
            if writer.state_limit.is_some() {
                let row_state = delta_row_state_bytes(&change)?;
                writer.delta_live_state_bytes = row_state
                    .checked_add(next_previous_capacity)
                    .ok_or_else(|| budget("authenticated delta row state overflow"))?;
                writer.check_cow_state(0)?;
            }
            root = update_one_v2(
                self,
                old,
                root,
                change,
                limits,
                &mut writer,
                deadline,
                cancelled,
            )?;
            if writer.state_limit.is_some() {
                writer.root_live_state_bytes = root
                    .as_ref()
                    .map(tree_handle_state_bytes)
                    .transpose()?
                    .unwrap_or(0);
                writer.delta_live_state_bytes = previous.as_ref().map_or(0, Vec::capacity);
                writer.check_cow_state(0)?;
            }
        }
        writer.finish()?;
        if writer.state_limit.is_some() {
            let output_state = std::mem::size_of::<AuthenticatedTreeDescriptorV2>()
                .checked_add(old.kind.capacity())
                .and_then(|n| {
                    n.checked_add(
                        root.as_ref()
                            .map(|handle| tree_reference_state_bytes(&handle.reference).ok())
                            .flatten()?,
                    )
                })
                .and_then(|n| n.checked_add(std::mem::size_of::<AuthenticatedTreeLocatorV2>()))
                .ok_or_else(|| budget("authenticated COW result state overflow"))?;
            writer.check_cow_state(output_state)?;
        }
        let entries = root.as_ref().map_or(0, |handle| handle.reference.entries);
        let semantic_root = root.as_ref().map(|handle| handle.reference.clone());
        let physical_root = match root.as_ref().map(|handle| &handle.locator) {
            Some(TreeLocatorV2::Packed(_)) => Some(public_locator(
                &root
                    .as_ref()
                    .ok_or_else(|| invalid("packed root disappeared"))?
                    .locator,
            )?),
            Some(TreeLocatorV2::Legacy(_)) | None => None,
        };
        let semantic = make_descriptor(self, &old.kind, semantic_root, entries)?;
        drop(writer);
        Ok((
            AuthenticatedTreeDescriptorV2 {
                semantic,
                physical_root,
            },
            work,
        ))
    }

    #[cfg(test)]
    fn apply_authenticated_tree_delta_v2_with_pack_cap<I>(
        &self,
        old: &AuthenticatedTreeDescriptorV2,
        changes: I,
        limits: AuthenticatedTreeLimitsV1,
        pack_cap: usize,
        io_ledger: Option<Arc<dyn AuthenticatedTreeIoLedgerV1>>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(AuthenticatedTreeDescriptorV2, AuthenticatedTreeWorkV1)>
    where
        I: IntoIterator<Item = Result<AuthenticatedTreeDeltaV1>>,
    {
        self.apply_authenticated_tree_delta_v2_with_pack_cap_and_state(
            old,
            changes,
            limits,
            pack_cap,
            io_ledger,
            AuthenticatedTreeWorkV1::default(),
            None,
            None,
            deadline,
            cancelled,
        )
    }

    pub fn lookup_authenticated_tree_v2(
        &self,
        descriptor: &AuthenticatedTreeDescriptorV2,
        key: &[u8],
        limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<Vec<u8>>> {
        self.lookup_authenticated_tree_v2_with_work(descriptor, key, limits, deadline, cancelled)
            .map(|(value, _)| value)
    }

    pub fn lookup_authenticated_tree_v2_with_work(
        &self,
        descriptor: &AuthenticatedTreeDescriptorV2,
        key: &[u8],
        limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(Option<Vec<u8>>, AuthenticatedTreeWorkV1)> {
        self.lookup_authenticated_tree_v2_with_work_and_io(
            descriptor, key, limits, None, deadline, cancelled,
        )
    }

    pub fn lookup_authenticated_tree_v2_with_work_and_io(
        &self,
        descriptor: &AuthenticatedTreeDescriptorV2,
        key: &[u8],
        limits: AuthenticatedTreeLimitsV1,
        io_ledger: Option<Arc<dyn AuthenticatedTreeIoLedgerV1>>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(Option<Vec<u8>>, AuthenticatedTreeWorkV1)> {
        self.lookup_authenticated_tree_v2_with_work_and_io_inner(
            descriptor, key, limits, io_ledger, deadline, cancelled, None,
        )
    }

    /// Point lookup variant that charges the caller's existing cumulative
    /// work callback immediately before each authenticated node visit. It does
    /// not clone or wrap that callback in the Send+Sync IO ledger.
    pub fn lookup_authenticated_tree_v2_with_work_and_io_and_callback(
        &self,
        descriptor: &AuthenticatedTreeDescriptorV2,
        key: &[u8],
        limits: AuthenticatedTreeLimitsV1,
        io_ledger: Option<Arc<dyn AuthenticatedTreeIoLedgerV1>>,
        deadline: Instant,
        cancelled: &AtomicBool,
        callback: &mut dyn FnMut() -> bool,
    ) -> Result<(Option<Vec<u8>>, AuthenticatedTreeWorkV1)> {
        self.lookup_authenticated_tree_v2_with_work_and_io_inner(
            descriptor,
            key,
            limits,
            io_ledger,
            deadline,
            cancelled,
            Some(callback),
        )
    }

    fn lookup_authenticated_tree_v2_with_work_and_io_inner(
        &self,
        descriptor: &AuthenticatedTreeDescriptorV2,
        key: &[u8],
        limits: AuthenticatedTreeLimitsV1,
        io_ledger: Option<Arc<dyn AuthenticatedTreeIoLedgerV1>>,
        deadline: Instant,
        cancelled: &AtomicBool,
        mut shared_work: Option<&mut dyn FnMut() -> bool>,
    ) -> Result<(Option<Vec<u8>>, AuthenticatedTreeWorkV1)> {
        let limits = limits.validate()?;
        validate_descriptor_for_store(self, &descriptor.semantic, limits)?;
        validate_descriptor_v2_shape(descriptor)?;
        validate_key_value(key, None, limits)?;
        check(deadline, cancelled)?;
        let _pin_lock = self.hold_generation_pin()?;
        let mut work = AuthenticatedTreeWorkV1::default();
        let Some(mut handle) = descriptor_root_handle(descriptor)? else {
            return Ok((None, work));
        };
        loop {
            let loaded = load_node_v2(
                self,
                descriptor,
                &handle,
                limits,
                &mut work,
                None,
                io_ledger.as_deref(),
                deadline,
                cancelled,
                &mut shared_work,
            )?;
            let node = loaded.node;
            let prefix_len = node_prefix_nibbles(&node);
            if !key_matches_prefix(key, &node.min_key, prefix_len) {
                return Ok((None, work));
            }
            let key_nibbles = key
                .len()
                .checked_mul(2)
                .ok_or_else(|| budget("tree key depth overflow"))?;
            if key_nibbles == prefix_len {
                return Ok((node.value.map(|entry| entry.value), work));
            }
            let Some(edge) = nibble_at(key, prefix_len) else {
                return Ok((None, work));
            };
            let Some(index) = node
                .children
                .iter()
                .position(|(child_edge, _)| *child_edge == edge)
            else {
                return Ok((None, work));
            };
            let child = node.children[index].1.clone();
            let locator = loaded
                .child_locators
                .get(index)
                .cloned()
                .ok_or_else(|| invalid("packed child locator is missing"))?;
            handle = TreeHandleV2 {
                reference: child,
                locator,
            };
        }
    }

    pub fn stream_authenticated_tree_v2(
        &self,
        descriptor: &AuthenticatedTreeDescriptorV2,
        limits: AuthenticatedTreeLimitsV1,
    ) -> Result<AuthenticatedTreeRowStreamV2> {
        self.stream_authenticated_tree_v2_with_io(descriptor, limits, None)
    }

    pub fn stream_authenticated_tree_v2_with_io(
        &self,
        descriptor: &AuthenticatedTreeDescriptorV2,
        limits: AuthenticatedTreeLimitsV1,
        io_ledger: Option<Arc<dyn AuthenticatedTreeIoLedgerV1>>,
    ) -> Result<AuthenticatedTreeRowStreamV2> {
        self.stream_authenticated_tree_v2_with_capture(
            descriptor,
            limits,
            io_ledger,
            PackCaptureV2::None,
            None,
        )
    }

    /// Streaming state is a slice of the caller's existing envelope. It
    /// covers this stream's descriptor/store, actual stack and one node decode.
    pub fn stream_authenticated_tree_v2_with_io_and_state(
        &self,
        descriptor: &AuthenticatedTreeDescriptorV2,
        limits: AuthenticatedTreeLimitsV1,
        io_ledger: Option<Arc<dyn AuthenticatedTreeIoLedgerV1>>,
        max_state_bytes: usize,
    ) -> Result<AuthenticatedTreeRowStreamV2> {
        self.stream_authenticated_tree_v2_with_capture(
            descriptor, limits, io_ledger, PackCaptureV2::None, Some(max_state_bytes),
        )
    }

    /// Share one bounded row walk between semantic inspection and full cold
    /// pack verification. The supplied spill remains bound to this store and
    /// closure; finish_pack_verification_with_work_callback seals the walk.
    pub fn stream_authenticated_tree_v2_with_pack_set_and_state(
        &self,
        descriptor: &AuthenticatedTreeDescriptorV2,
        limits: AuthenticatedTreeLimitsV1,
        io_ledger: Option<Arc<dyn AuthenticatedTreeIoLedgerV1>>,
        pack_set: Arc<dyn AuthenticatedTreePackSetV2>,
        closure_binding: Digest256,
        max_state_bytes: usize,
    ) -> Result<AuthenticatedTreeRowStreamV2> {
        pack_set.check_binding(
            self.physical_root_identity()?, self.store_id(), self.domain_digest(), closure_binding,
        )?;
        self.stream_authenticated_tree_v2_with_capture(
            descriptor, limits, io_ledger, PackCaptureV2::External(pack_set), Some(max_state_bytes),
        )
    }

    fn stream_authenticated_tree_v2_with_capture(
        &self,
        descriptor: &AuthenticatedTreeDescriptorV2,
        limits: AuthenticatedTreeLimitsV1,
        io_ledger: Option<Arc<dyn AuthenticatedTreeIoLedgerV1>>,
        pack_capture: PackCaptureV2,
        state_limit: Option<usize>,
    ) -> Result<AuthenticatedTreeRowStreamV2> {
        let limits = limits.validate()?;
        validate_descriptor_for_store(self, &descriptor.semantic, limits)?;
        validate_descriptor_v2_shape(descriptor)?;
        if descriptor.entries > limits.max_rows {
            return Err(budget("authenticated stream row limit exceeded"));
        }
        let base_state_bytes = std::mem::size_of::<AuthenticatedTreeRowStreamV2>()
            .checked_add(self.retained_heap_state_bytes()?)
            .and_then(|n| n.checked_add(tree_descriptor_state_bytes(descriptor).ok()?))
            .and_then(|n| n.checked_add(4096))
            .ok_or_else(|| budget("packed stream base state overflow"))?;
        if let Some(maximum) = state_limit {
            let workspace = limits.max_node_bytes.checked_mul(64)
                .and_then(|n| n.checked_add(65_536))
                .ok_or_else(|| budget("packed stream initial workspace overflow"))?;
            if maximum == 0 || maximum == usize::MAX || base_state_bytes.checked_add(workspace)
                .is_none_or(|n| n > maximum) {
                return Err(budget("packed stream initial state allowance exceeded"));
            }
        }
        let mut transcript = Digest256Hasher::new();
        transcript.update(b"tos-authenticated-tree-stream-v1\0");
        transcript.update(descriptor.commitment.as_bytes());
        Ok(AuthenticatedTreeRowStreamV2 {
            store: self.clone(),
            descriptor: descriptor.clone(),
            limits,
            io_ledger,
            _pin_lock: Arc::new(self.hold_generation_pin()?),
            stack: Vec::new(),
            state_limit,
            base_state_bytes,
            retained_node_bytes: 0,
            started: false,
            observed: 0,
            transcript,
            work: AuthenticatedTreeWorkV1::default(),
            pack_capture,
            done: false,
            failed: false,
        })
    }

    /// Compatibility full cold closure: streams every semantic node, then
    /// verifies each distinct reachable immutable pack digest once. Callers
    /// must explicitly precharge the local set against their finite node cap.
    pub fn verify_authenticated_tree_v2(
        &self,
        descriptor: &AuthenticatedTreeDescriptorV2,
        limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<AuthenticatedTreeCoverageV1> {
        self.verify_authenticated_tree_v2_with_io(descriptor, limits, None, deadline, cancelled)
    }

    pub fn verify_authenticated_tree_v2_with_io(
        &self,
        descriptor: &AuthenticatedTreeDescriptorV2,
        limits: AuthenticatedTreeLimitsV1,
        io_ledger: Option<Arc<dyn AuthenticatedTreeIoLedgerV1>>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<AuthenticatedTreeCoverageV1> {
        let mut stream = self.stream_authenticated_tree_v2_with_capture(
            descriptor,
            limits,
            io_ledger.clone(),
            PackCaptureV2::Local(HashSet::new()),
            None,
        )?;
        while stream.next_row(deadline, cancelled)?.is_some() {}
        let pack_digests = stream.pack_capture.take_local()?;
        for digest in pack_digests {
            check(deadline, cancelled)?;
            let remaining = remaining_bytes(stream.work, stream.limits)?;
            if remaining == 0 {
                return Err(budget("authenticated tree byte budget exceeded"));
            }
            let raw = self.read_authenticated_blob_with_io(
                digest,
                AUTHENTICATED_PACK_MAX_BYTES.min(remaining),
                io_ledger.as_deref(),
                deadline,
                cancelled,
            )?;
            let frame_count = scan_packed_chunk(&raw)?;
            stream
                .work
                .charge_pack_read(raw.len(), frame_count, stream.limits)?;
        }
        check(deadline, cancelled)?;
        stream.coverage().ok_or_else(|| {
            SegmentError::new(
                Code::InvalidReceipt,
                "packed authenticated tree coverage unavailable",
            )
        })
    }

    /// Full cold closure using an operation-local exact inventory spill.
    /// The spill only orders observed pack digests; every pending row is
    /// reopened and physically validated here by this SegmentStore.
    pub fn verify_authenticated_tree_v2_with_pack_set(
        &self,
        descriptor: &AuthenticatedTreeDescriptorV2,
        limits: AuthenticatedTreeLimitsV1,
        io_ledger: Option<Arc<dyn AuthenticatedTreeIoLedgerV1>>,
        pack_set: Arc<dyn AuthenticatedTreePackSetV2>,
        closure_binding: Digest256,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<AuthenticatedTreeCoverageV1> {
        check(deadline, cancelled)?;
        pack_set.check_binding(
            self.physical_root_identity()?,
            self.store_id(),
            self.domain_digest(),
            closure_binding,
        )?;
        let mut stream = self.stream_authenticated_tree_v2_with_capture(
            descriptor,
            limits,
            io_ledger.clone(),
            PackCaptureV2::External(pack_set.clone()),
            None,
        )?;
        while stream.next_row(deadline, cancelled)?.is_some() {}
        let mut verify = |digest: Digest256| {
            check(deadline, cancelled)?;
            let remaining = remaining_bytes(stream.work, stream.limits)?;
            if remaining == 0 {
                return Err(budget("authenticated tree byte budget exceeded"));
            }
            let raw = self.read_authenticated_blob_with_io(
                digest,
                AUTHENTICATED_PACK_MAX_BYTES.min(remaining),
                io_ledger.as_deref(),
                deadline,
                cancelled,
            )?;
            let frame_count = scan_packed_chunk(&raw)?;
            stream
                .work
                .charge_pack_read(raw.len(), frame_count, stream.limits)
        };
        pack_set.verify_pending(&mut verify)?;
        drop(verify);
        check(deadline, cancelled)?;
        stream.coverage().ok_or_else(|| {
            SegmentError::new(
                Code::InvalidReceipt,
                "packed authenticated tree coverage unavailable",
            )
        })
    }

    /// Full cold closure with the caller's existing cumulative work meter.
    /// The callback is charged before each traversed authenticated node and
    /// once before each distinct immutable pack is physically verified.
    pub fn verify_authenticated_tree_v2_with_pack_set_and_work_callback(
        &self,
        descriptor: &AuthenticatedTreeDescriptorV2,
        limits: AuthenticatedTreeLimitsV1,
        io_ledger: Option<Arc<dyn AuthenticatedTreeIoLedgerV1>>,
        pack_set: Arc<dyn AuthenticatedTreePackSetV2>,
        closure_binding: Digest256,
        deadline: Instant,
        cancelled: &AtomicBool,
        shared_work: &mut dyn FnMut() -> bool,
    ) -> Result<AuthenticatedTreeCoverageV1> {
        self.verify_authenticated_tree_v2_with_pack_set_controlled(
            descriptor, limits, io_ledger, pack_set, closure_binding,
            deadline, cancelled, shared_work, None,
        )
    }

    /// Same full closure with preallocation admission for the live traversal
    /// stack and decoder, under a caller-selected slice of its original state.
    pub fn verify_authenticated_tree_v2_with_pack_set_and_work_and_state(
        &self,
        descriptor: &AuthenticatedTreeDescriptorV2,
        limits: AuthenticatedTreeLimitsV1,
        io_ledger: Option<Arc<dyn AuthenticatedTreeIoLedgerV1>>,
        pack_set: Arc<dyn AuthenticatedTreePackSetV2>,
        closure_binding: Digest256,
        deadline: Instant,
        cancelled: &AtomicBool,
        shared_work: &mut dyn FnMut() -> bool,
        max_state_bytes: usize,
    ) -> Result<AuthenticatedTreeCoverageV1> {
        self.verify_authenticated_tree_v2_with_pack_set_controlled(
            descriptor, limits, io_ledger, pack_set, closure_binding,
            deadline, cancelled, shared_work, Some(max_state_bytes),
        )
    }

    fn verify_authenticated_tree_v2_with_pack_set_controlled(
        &self,
        descriptor: &AuthenticatedTreeDescriptorV2,
        limits: AuthenticatedTreeLimitsV1,
        io_ledger: Option<Arc<dyn AuthenticatedTreeIoLedgerV1>>,
        pack_set: Arc<dyn AuthenticatedTreePackSetV2>,
        closure_binding: Digest256,
        deadline: Instant,
        cancelled: &AtomicBool,
        shared_work: &mut dyn FnMut() -> bool,
        state_limit: Option<usize>,
    ) -> Result<AuthenticatedTreeCoverageV1> {
        check(deadline, cancelled)?;
        pack_set.check_binding(
            self.physical_root_identity()?,
            self.store_id(),
            self.domain_digest(),
            closure_binding,
        )?;
        let mut stream = self.stream_authenticated_tree_v2_with_capture(
            descriptor,
            limits,
            io_ledger.clone(),
            PackCaptureV2::External(pack_set.clone()),
            state_limit,
        )?;
        while stream
            .next_row_with_work_callback(deadline, cancelled, shared_work)?
            .is_some()
        {}
        stream.finish_pack_verification_with_work_callback(deadline, cancelled, shared_work)
    }
}

fn work_from_install(install: ImmutableBlobInstallV1) -> AuthenticatedTreeWorkV1 {
    AuthenticatedTreeWorkV1 {
        read_nodes: u64::from(install.read_bytes > 0),
        read_bytes: install.read_bytes,
        written_nodes: u64::from(install.written_bytes > 0),
        written_bytes: install.written_bytes,
        allocated_bytes: install.allocated_bytes,
    }
}

fn close_build_frame_v2(
    writer: &mut PackWriterV2<'_, '_>,
    kind: &[u8],
    frame: BuildFrameV2,
    limits: AuthenticatedTreeLimitsV1,
    retained_builder_state: usize,
    pending_payload_bytes: &mut u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Option<TreeHandleV2>> {
    check(deadline, cancelled)?;
    let frame_state = build_frame_state_bytes(&frame)?;
    writer.root_live_state_bytes = retained_builder_state
        .checked_add(frame_state)
        .ok_or_else(|| budget("packed builder close state overflow"))?;
    writer.check_cow_state(0)?;
    if frame.value.is_none() && frame.children.is_empty() {
        writer.root_live_state_bytes = retained_builder_state;
        return Ok(None);
    }
    if frame.value.is_none() && frame.children.len() == 1 {
        let child = frame.children.into_iter().next().map(|(_, child)| child);
        if let Some(child) = &child {
            writer.check_cow_state(tree_handle_state_bytes(child)?)?;
        }
        return Ok(child);
    }
    let frame_state = build_frame_state_bytes(&frame)?;
    writer.root_live_state_bytes = retained_builder_state
        .checked_add(frame_state)
        .ok_or_else(|| budget("packed builder close state overflow"))?;
    let child_count = frame.children.len();
    let child_array_bytes = child_count
        .checked_mul(
            std::mem::size_of::<(u8, AuthenticatedTreeNodeRefV1)>()
                .checked_add(std::mem::size_of::<TreeLocatorV2>())
                .ok_or_else(|| budget("packed builder close state overflow"))?,
        )
        .ok_or_else(|| budget("packed builder close state overflow"))?;
    let node_key_copies = limits
        .max_key_bytes
        .checked_mul(2)
        .ok_or_else(|| budget("packed builder close state overflow"))?;
    writer.check_cow_state(
        child_array_bytes
            .checked_add(node_key_copies)
            .ok_or_else(|| budget("packed builder close state overflow"))?,
    )?;
    let payload_bytes = frame
        .value
        .as_ref()
        .map(|entry| {
            u64::try_from(entry.key.len())
                .ok()
                .and_then(|key| {
                    u64::try_from(entry.value.len())
                        .ok()
                        .and_then(|value| key.checked_add(value))
                })
                .ok_or_else(|| budget("packed pending row size overflow"))
        })
        .transpose()?
        .unwrap_or(0);
    let mut children = Vec::new();
    let mut locators = Vec::new();
    children
        .try_reserve_exact(frame.children.len())
        .map_err(|_| budget("packed builder child allocation failed"))?;
    locators
        .try_reserve_exact(frame.children.len())
        .map_err(|_| budget("packed locator allocation failed"))?;
    let allocated_arrays = children
        .capacity()
        .checked_mul(std::mem::size_of::<(u8, AuthenticatedTreeNodeRefV1)>())
        .and_then(|bytes| {
            bytes.checked_add(
                locators
                    .capacity()
                    .checked_mul(std::mem::size_of::<TreeLocatorV2>())?,
            )
        })
        .ok_or_else(|| budget("packed builder close state overflow"))?;
    writer.check_cow_state(
        allocated_arrays
            .checked_add(node_key_copies)
            .ok_or_else(|| budget("packed builder close state overflow"))?,
    )?;
    for (edge, child) in frame.children {
        children.push((edge, child.reference));
        locators.push(child.locator);
    }
    let node = make_node(frame.value, children, limits)?;
    writer.root_live_state_bytes = retained_builder_state;
    let handle = writer.persist(kind, node, &locators, locators.capacity())?;
    *pending_payload_bytes = pending_payload_bytes
        .checked_sub(payload_bytes)
        .ok_or_else(|| invalid("packed pending payload accounting differs"))?;
    Ok(Some(handle))
}

fn attach_build_child_v2(
    writer: &mut PackWriterV2<'_, '_>,
    parent: &mut BuildFrameV2,
    edge: u8,
    child: TreeHandleV2,
    limits: AuthenticatedTreeLimitsV1,
    retained_builder_state: usize,
) -> Result<()> {
    if parent.children.len() >= limits.max_children {
        return Err(budget("authenticated node child limit exceeded"));
    }
    if parent
        .children
        .last()
        .is_some_and(|(previous, _)| *previous >= edge)
    {
        return Err(invalid("packed builder child order differs"));
    }
    let child_state = tree_handle_state_bytes(&child)?;
    let requested = parent
        .children
        .len()
        .checked_add(1)
        .ok_or_else(|| budget("packed builder child capacity overflow"))?;
    let requested_bytes = requested
        .checked_mul(std::mem::size_of::<(u8, TreeHandleV2)>())
        .ok_or_else(|| budget("packed builder child state overflow"))?;
    let current_bytes = parent
        .children
        .capacity()
        .checked_mul(std::mem::size_of::<(u8, TreeHandleV2)>())
        .ok_or_else(|| budget("packed builder child state overflow"))?;
    writer.root_live_state_bytes = retained_builder_state;
    writer.check_cow_state(
        child_state
            .checked_add(requested_bytes.saturating_sub(current_bytes))
            .ok_or_else(|| budget("packed builder child state overflow"))?,
    )?;
    parent
        .children
        .try_reserve_exact(1)
        .map_err(|_| budget("packed builder child allocation failed"))?;
    let allocated_bytes = parent
        .children
        .capacity()
        .checked_mul(std::mem::size_of::<(u8, TreeHandleV2)>())
        .ok_or_else(|| budget("packed builder child state overflow"))?;
    writer.check_cow_state(
        child_state
            .checked_add(allocated_bytes.saturating_sub(current_bytes))
            .ok_or_else(|| budget("packed builder child state overflow"))?,
    )?;
    parent.children.push((edge, child));
    Ok(())
}

fn entry_state_bytes(entry: &AuthenticatedTreeEntryV1) -> Result<usize> {
    std::mem::size_of::<AuthenticatedTreeEntryV1>()
        .checked_add(entry.key.capacity())
        .and_then(|bytes| bytes.checked_add(entry.value.capacity()))
        .ok_or_else(|| budget("packed builder entry state overflow"))
}

fn build_frame_state_bytes(frame: &BuildFrameV2) -> Result<usize> {
    let mut state = std::mem::size_of::<BuildFrameV2>()
        .checked_add(
            frame
                .children
                .capacity()
                .checked_mul(std::mem::size_of::<(u8, TreeHandleV2)>())
                .ok_or_else(|| budget("packed builder frame state overflow"))?,
        )
        .ok_or_else(|| budget("packed builder frame state overflow"))?;
    if let Some(value) = &frame.value {
        state = state
            .checked_add(entry_state_bytes(value)?)
            .ok_or_else(|| budget("packed builder frame state overflow"))?;
    }
    for (_, child) in &frame.children {
        state = state
            .checked_add(tree_handle_state_bytes(child)?)
            .ok_or_else(|| budget("packed builder frame state overflow"))?;
    }
    Ok(state)
}

fn build_frame_stack_state_bytes(frames: &Vec<BuildFrameV2>) -> Result<usize> {
    let mut state = frames
        .capacity()
        .checked_mul(std::mem::size_of::<BuildFrameV2>())
        .ok_or_else(|| budget("packed builder stack state overflow"))?;
    for frame in frames {
        state = state
            .checked_add(build_frame_state_bytes(frame)?)
            .ok_or_else(|| budget("packed builder stack state overflow"))?;
    }
    Ok(state)
}

fn build_cursor_state_bytes<I>(cursor: &BuildCursor<I>) -> Result<usize> {
    let mut state = std::mem::size_of::<BuildCursor<I>>();
    if let Some(previous) = &cursor.previous {
        state = state
            .checked_add(previous.capacity())
            .ok_or_else(|| budget("packed builder cursor state overflow"))?;
    }
    if let Some(pending) = &cursor.pending {
        state = state
            .checked_add(entry_state_bytes(pending)?)
            .ok_or_else(|| budget("packed builder cursor state overflow"))?;
    }
    Ok(state)
}

fn check_build_state<I>(
    writer: &mut PackWriterV2<'_, '_>,
    frames: &Vec<BuildFrameV2>,
    cursor: &BuildCursor<I>,
    _limits: AuthenticatedTreeLimitsV1,
    transient_bytes: usize,
) -> Result<()> {
    if writer.state_limit.is_none() {
        return Ok(());
    }
    writer.root_live_state_bytes = build_frame_stack_state_bytes(frames)?
        .checked_add(build_cursor_state_bytes(cursor)?)
        .ok_or_else(|| budget("packed builder retained state overflow"))?;
    writer.check_cow_state(transient_bytes)
}

fn tree_descriptor_state_bytes(descriptor: &AuthenticatedTreeDescriptorV2) -> Result<usize> {
    let mut state = std::mem::size_of::<AuthenticatedTreeDescriptorV2>()
        .checked_add(std::mem::size_of::<AuthenticatedTreeDescriptorV1>())
        .and_then(|n| n.checked_add(descriptor.semantic.kind.capacity()))
        .ok_or_else(|| budget("authenticated COW descriptor state overflow"))?;
    if let Some(root) = &descriptor.semantic.root {
        state = state
            .checked_add(tree_reference_state_bytes(root)?)
            .ok_or_else(|| budget("authenticated COW descriptor state overflow"))?;
    }
    if descriptor.physical_root.is_some() {
        state = state
            .checked_add(std::mem::size_of::<AuthenticatedTreeLocatorV2>())
            .ok_or_else(|| budget("authenticated COW descriptor state overflow"))?;
    }
    Ok(state)
}

fn tree_descriptor_handle_clone_state_bytes(
    descriptor: &AuthenticatedTreeDescriptorV2,
) -> Result<usize> {
    let Some(root) = &descriptor.semantic.root else {
        return Ok(0);
    };
    let seal_allocation_state = pack_seal_arc_allocation_state_bytes()?;
    std::mem::size_of::<TreeHandleV2>()
        .checked_add(tree_reference_state_bytes(root)?)
        .and_then(|n| n.checked_add(seal_allocation_state))
        .ok_or_else(|| budget("authenticated COW root handle state overflow"))
}

/// Upper bound for a separately allocated `Arc<PackSealStateV2>`, including
/// both atomic strong/weak counters and alignment padding. Tree handles count
/// the Arc pointer inline; this covers its heap allocation before each new Arc.
fn pack_seal_arc_allocation_state_bytes() -> Result<usize> {
    let (first_two_counters, _) = Layout::new::<AtomicUsize>()
        .extend(Layout::new::<AtomicUsize>())
        .map_err(|_| budget("authenticated pack seal layout overflow"))?;
    let (with_payload, _) = first_two_counters
        .extend(Layout::new::<PackSealStateV2>())
        .map_err(|_| budget("authenticated pack seal layout overflow"))?;
    Ok(with_payload.pad_to_align().size())
}

fn tree_reference_state_bytes(reference: &AuthenticatedTreeNodeRefV1) -> Result<usize> {
    std::mem::size_of::<AuthenticatedTreeNodeRefV1>()
        .checked_add(reference.min_key.capacity())
        .and_then(|n| n.checked_add(reference.max_key.capacity()))
        .ok_or_else(|| budget("authenticated COW node reference state overflow"))
}

fn tree_handle_state_bytes(handle: &TreeHandleV2) -> Result<usize> {
    let locator_state = match &handle.locator {
        TreeLocatorV2::Legacy(_) => std::mem::size_of::<TreeLocatorV2>(),
        TreeLocatorV2::Packed(_) => std::mem::size_of::<TreeLocatorV2>()
            .checked_add(pack_seal_arc_allocation_state_bytes()?)
            .ok_or_else(|| budget("authenticated COW locator state overflow"))?,
    };
    std::mem::size_of::<TreeHandleV2>()
        .checked_add(tree_reference_state_bytes(&handle.reference)?)
        .and_then(|n| n.checked_add(locator_state))
        .ok_or_else(|| budget("authenticated COW tree handle state overflow"))
}

fn tree_node_state_bytes(node: &TreeNode) -> Result<usize> {
    let mut state = std::mem::size_of::<TreeNode>()
        .checked_add(node.min_key.capacity())
        .and_then(|n| n.checked_add(node.max_key.capacity()))
        .and_then(|n| {
            n.checked_add(
                node.children
                    .capacity()
                    .checked_mul(std::mem::size_of::<(u8, AuthenticatedTreeNodeRefV1)>())?,
            )
        })
        .ok_or_else(|| budget("authenticated COW node state overflow"))?;
    if let Some(value) = &node.value {
        state = state
            .checked_add(std::mem::size_of::<AuthenticatedTreeEntryV1>())
            .and_then(|n| n.checked_add(value.key.capacity()))
            .and_then(|n| n.checked_add(value.value.capacity()))
            .ok_or_else(|| budget("authenticated COW node value state overflow"))?;
    }
    for (_, child) in &node.children {
        state = state
            .checked_add(tree_reference_state_bytes(child)?)
            .ok_or_else(|| budget("authenticated COW child reference state overflow"))?;
    }
    Ok(state)
}

// The slice preserves row borrowing; its owner supplies the actual retained
// allocation capacity. Temporary array callers supply their exact slot count.
fn tree_locator_slice_state_bytes(locators: &[TreeLocatorV2], capacity: usize) -> Result<usize> {
    if capacity < locators.len() {
        return Err(invalid(
            "authenticated COW locator capacity is below length",
        ));
    }
    let mut state = capacity
        .checked_mul(std::mem::size_of::<TreeLocatorV2>())
        .ok_or_else(|| budget("authenticated COW locator state overflow"))?;
    let seal_state_bytes = pack_seal_arc_allocation_state_bytes()?;
    for locator in locators {
        if matches!(locator, TreeLocatorV2::Packed(_)) {
            state = state
                .checked_add(seal_state_bytes)
                .ok_or_else(|| budget("authenticated COW locator state overflow"))?;
        }
    }
    Ok(state)
}

fn ancestor_stack_state_bytes(
    ancestors: &Vec<(TreeNode, Vec<TreeLocatorV2>, u8)>,
) -> Result<usize> {
    let mut state = ancestors
        .capacity()
        .checked_mul(std::mem::size_of::<(TreeNode, Vec<TreeLocatorV2>, u8)>())
        .ok_or_else(|| budget("authenticated COW ancestor state overflow"))?;
    for (node, locators, _) in ancestors {
        state = state
            .checked_add(tree_node_state_bytes(node)?)
            .and_then(|n| {
                n.checked_add(tree_locator_slice_state_bytes(locators, locators.capacity()).ok()?)
            })
            .ok_or_else(|| budget("authenticated COW ancestor state overflow"))?;
    }
    Ok(state)
}

fn delta_row_state_bytes(change: &AuthenticatedTreeDeltaV1) -> Result<usize> {
    let mut state = std::mem::size_of::<AuthenticatedTreeDeltaV1>()
        .checked_add(change.key.capacity())
        .ok_or_else(|| budget("authenticated COW delta state overflow"))?;
    if let Some(value) = &change.value {
        state = state
            .checked_add(value.capacity())
            .ok_or_else(|| budget("authenticated COW delta state overflow"))?;
    }
    Ok(state)
}

fn tree_node_decode_upper_bound(
    handle: &TreeHandleV2,
    limits: AuthenticatedTreeLimitsV1,
) -> Result<usize> {
    let frame = match &handle.locator {
        TreeLocatorV2::Packed(locator) => locator.frame_len as usize,
        TreeLocatorV2::Legacy(_) => limits.max_node_bytes,
    };
    let seal_allocation_state = pack_seal_arc_allocation_state_bytes()?;
    let per_child = std::mem::size_of::<PackedChildWireV2>()
        .checked_add(std::mem::size_of::<TreeLocatorV2>())
        .and_then(|n| n.checked_add(seal_allocation_state))
        .and_then(|n| n.checked_add(std::mem::size_of::<(u8, AuthenticatedTreeNodeRefV1)>()))
        .ok_or_else(|| budget("authenticated COW decode state overflow"))?;
    frame
        .checked_mul(5)
        .and_then(|n| n.checked_add(limits.max_children.checked_mul(per_child)?))
        .and_then(|n| n.checked_add(std::mem::size_of::<LoadedTreeNodeV2>()))
        .ok_or_else(|| budget("authenticated COW decode state overflow"))
}

fn tree_node_mutation_upper_bound(limits: AuthenticatedTreeLimitsV1) -> Result<usize> {
    let per_child = std::mem::size_of::<TreeLocatorV2>()
        .checked_add(pack_seal_arc_allocation_state_bytes()?)
        .and_then(|n| n.checked_add(std::mem::size_of::<(u8, AuthenticatedTreeNodeRefV1)>() * 2))
        .ok_or_else(|| budget("authenticated COW mutation state overflow"))?;
    limits
        .max_node_bytes
        .checked_mul(8)
        .and_then(|n| n.checked_add(limits.max_children.checked_mul(per_child)?))
        .and_then(|n| n.checked_add(limits.max_key_bytes.checked_mul(4)?))
        .and_then(|n| n.checked_add(limits.max_value_bytes.min(limits.max_node_bytes)))
        .and_then(|n| n.checked_add(64 * 1024))
        .ok_or_else(|| budget("authenticated COW mutation state overflow"))
}

fn update_one_v2(
    store: &SegmentStore,
    descriptor: &AuthenticatedTreeDescriptorV2,
    root: Option<TreeHandleV2>,
    change: AuthenticatedTreeDeltaV1,
    limits: AuthenticatedTreeLimitsV1,
    writer: &mut PackWriterV2<'_, '_>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Option<TreeHandleV2>> {
    if writer.state_limit.is_some() {
        let root_state = root
            .as_ref()
            .map(tree_handle_state_bytes)
            .transpose()?
            .unwrap_or(0);
        writer.check_cow_state(root_state)?;
    }
    let Some(mut handle) = root.clone() else {
        return match change.value {
            None => Ok(None),
            Some(value) => {
                let node = make_node(
                    Some(AuthenticatedTreeEntryV1 {
                        key: change.key,
                        value,
                    }),
                    Vec::new(),
                    limits,
                )?;
                writer.persist(&descriptor.kind, node, &[], 0).map(Some)
            }
        };
    };
    let mut ancestors: Vec<(TreeNode, Vec<TreeLocatorV2>, u8)> = Vec::new();
    let mut replacement: Option<TreeHandleV2>;
    loop {
        check(deadline, cancelled)?;
        if writer.state_limit.is_some() {
            let stack_bytes = ancestor_stack_state_bytes(&ancestors)?;
            let handle_bytes = tree_handle_state_bytes(&handle)?;
            let decode_bytes = tree_node_decode_upper_bound(&handle, limits)?;
            let mutation_bytes = tree_node_mutation_upper_bound(limits)?;
            writer.check_cow_state(
                stack_bytes
                    .checked_add(handle_bytes)
                    .and_then(|n| n.checked_add(decode_bytes))
                    .and_then(|n| n.checked_add(mutation_bytes))
                    .ok_or_else(|| budget("authenticated COW decode state overflow"))?,
            )?;
        }
        let active_pack = writer.active.as_ref();
        let work = &mut *writer.work;
        let loaded = load_node_v2(
            store,
            descriptor,
            &handle,
            limits,
            work,
            active_pack,
            writer.io_ledger.as_deref(),
            deadline,
            cancelled,
            &mut writer.shared_work,
        )?;
        let node = loaded.node;
        if writer.state_limit.is_some() {
            let stack_bytes = ancestor_stack_state_bytes(&ancestors)?;
            let node_bytes = tree_node_state_bytes(&node)?
                .checked_add(tree_locator_slice_state_bytes(
                    &loaded.child_locators,
                    loaded.child_locators.capacity(),
                )?)
                .ok_or_else(|| budget("authenticated COW loaded-node state overflow"))?;
            let operation_scratch = tree_node_mutation_upper_bound(limits)?;
            writer.check_cow_state(
                stack_bytes
                    .checked_add(tree_handle_state_bytes(&handle)?)
                    .and_then(|n| n.checked_add(node_bytes))
                    .and_then(|n| n.checked_add(operation_scratch))
                    .ok_or_else(|| budget("authenticated COW loaded-node state overflow"))?,
            )?;
        }
        let prefix_len = node_prefix_nibbles(&node);
        let shared = common_prefix_nibbles(&change.key, &node.min_key);
        if shared < prefix_len {
            let Some(value) = change.value.clone() else {
                return Ok(root);
            };
            let old_edge = nibble_at(&node.min_key, shared)
                .ok_or_else(|| invalid("packed split lacks old edge"))?;
            if change.key.len() * 2 == shared {
                let parent = make_node(
                    Some(AuthenticatedTreeEntryV1 {
                        key: change.key.clone(),
                        value,
                    }),
                    vec![(old_edge, handle.reference.clone())],
                    limits,
                )?;
                replacement =
                    Some(writer.persist(&descriptor.kind, parent, &[handle.locator.clone()], 1)?);
            } else {
                let new_edge = nibble_at(&change.key, shared)
                    .ok_or_else(|| invalid("packed split lacks new edge"))?;
                if new_edge == old_edge {
                    return Err(invalid("packed split edge did not diverge"));
                }
                let leaf = make_node(
                    Some(AuthenticatedTreeEntryV1 {
                        key: change.key.clone(),
                        value,
                    }),
                    Vec::new(),
                    limits,
                )?;
                let leaf_handle = writer.persist(&descriptor.kind, leaf, &[], 0)?;
                let mut children = vec![(old_edge, handle.clone()), (new_edge, leaf_handle)];
                children.sort_by_key(|(edge, _)| *edge);
                let refs = children
                    .iter()
                    .map(|(edge, child)| (*edge, child.reference.clone()))
                    .collect();
                let node = make_node(None, refs, limits)?;
                let locators = children
                    .into_iter()
                    .map(|(_, child)| child.locator)
                    .collect::<Vec<_>>();
                replacement =
                    Some(writer.persist(&descriptor.kind, node, &locators, locators.capacity())?);
            }
            break;
        }

        let key_nibbles = change.key.len() * 2;
        if key_nibbles == prefix_len {
            match (&node.value, &change.value) {
                (Some(old), Some(new)) if old.value.as_slice() == new.as_slice() => {
                    return Ok(root);
                }
                (None, None) => return Ok(root),
                _ => {}
            }
            let mut updated = node;
            updated.value = change.value.clone().map(|value| AuthenticatedTreeEntryV1 {
                key: change.key.clone(),
                value,
            });
            replacement = normalize_node_v2(
                &descriptor.kind,
                updated,
                loaded.child_locators,
                limits,
                writer,
                deadline,
                cancelled,
            )?;
            break;
        }
        let Some(edge) = nibble_at(&change.key, prefix_len) else {
            return Ok(root);
        };
        if let Some(index) = node
            .children
            .iter()
            .position(|(child_edge, _)| *child_edge == edge)
        {
            if writer.state_limit.is_some() {
                let next_capacity = if ancestors.len() < ancestors.capacity() {
                    ancestors.capacity()
                } else {
                    ancestors
                        .len()
                        .checked_add(1)
                        .ok_or_else(|| budget("authenticated COW ancestor depth overflow"))?
                };
                let growth = next_capacity
                    .saturating_sub(ancestors.capacity())
                    .checked_mul(std::mem::size_of::<(TreeNode, Vec<TreeLocatorV2>, u8)>())
                    .ok_or_else(|| budget("authenticated COW ancestor state overflow"))?;
                let stored = ancestor_stack_state_bytes(&ancestors)?
                    .checked_add(tree_node_state_bytes(&node)?)
                    .and_then(|n| {
                        n.checked_add(
                            tree_locator_slice_state_bytes(
                                &loaded.child_locators,
                                loaded.child_locators.capacity(),
                            )
                            .ok()?,
                        )
                    })
                    .and_then(|n| n.checked_add(growth))
                    .and_then(|n| {
                        n.checked_add(tree_reference_state_bytes(&node.children[index].1).ok()?)
                    })
                    .and_then(|n| n.checked_add(std::mem::size_of::<TreeLocatorV2>()))
                    .ok_or_else(|| budget("authenticated COW ancestor state overflow"))?;
                writer.check_cow_state(
                    stored
                        .checked_add(tree_handle_state_bytes(&handle)?)
                        .and_then(|n| n.checked_add(tree_node_mutation_upper_bound(limits).ok()?))
                        .ok_or_else(|| budget("authenticated COW ancestor state overflow"))?,
                )?;
                if ancestors.len() == ancestors.capacity() {
                    ancestors
                        .try_reserve_exact(1)
                        .map_err(|_| budget("authenticated COW ancestor allocation failed"))?;
                    writer.check_cow_state(
                        ancestor_stack_state_bytes(&ancestors)?
                            .checked_add(tree_node_state_bytes(&node)?)
                            .and_then(|n| {
                                n.checked_add(
                                    tree_locator_slice_state_bytes(
                                        &loaded.child_locators,
                                        loaded.child_locators.capacity(),
                                    )
                                    .ok()?,
                                )
                            })
                            .and_then(|n| {
                                n.checked_add(
                                    tree_reference_state_bytes(&node.children[index].1).ok()?,
                                )
                            })
                            .and_then(|n| n.checked_add(std::mem::size_of::<TreeLocatorV2>()))
                            .and_then(|n| n.checked_add(tree_handle_state_bytes(&handle).ok()?))
                            .and_then(|n| {
                                n.checked_add(tree_node_mutation_upper_bound(limits).ok()?)
                            })
                            .ok_or_else(|| budget("authenticated COW ancestor state overflow"))?,
                    )?;
                }
            }
            let child = node.children[index].1.clone();
            let locator = loaded
                .child_locators
                .get(index)
                .cloned()
                .ok_or_else(|| invalid("packed update child locator is missing"))?;
            ancestors.push((node, loaded.child_locators, edge));
            handle = TreeHandleV2 {
                reference: child,
                locator,
            };
            continue;
        }
        let Some(value) = change.value.clone() else {
            return Ok(root);
        };
        let leaf = make_node(
            Some(AuthenticatedTreeEntryV1 {
                key: change.key.clone(),
                value,
            }),
            Vec::new(),
            limits,
        )?;
        let leaf_handle = writer.persist(&descriptor.kind, leaf, &[], 0)?;
        let mut paired: Vec<(u8, TreeHandleV2)> = node
            .children
            .iter()
            .zip(loaded.child_locators.iter())
            .map(|((edge, reference), locator)| {
                (
                    *edge,
                    TreeHandleV2 {
                        reference: reference.clone(),
                        locator: locator.clone(),
                    },
                )
            })
            .collect();
        paired.push((edge, leaf_handle));
        paired.sort_by_key(|(child_edge, _)| *child_edge);
        let mut updated = node;
        updated.children = paired
            .iter()
            .map(|(child_edge, child)| (*child_edge, child.reference.clone()))
            .collect();
        let child_locators = paired.into_iter().map(|(_, child)| child.locator).collect();
        replacement = normalize_node_v2(
            &descriptor.kind,
            updated,
            child_locators,
            limits,
            writer,
            deadline,
            cancelled,
        )?;
        break;
    }
    while let Some((mut parent, mut child_locators, edge)) = ancestors.pop() {
        match replacement.take() {
            Some(child) => {
                let Some(position) = parent
                    .children
                    .iter()
                    .position(|(old_edge, _)| *old_edge == edge)
                else {
                    return Err(invalid("packed update parent edge disappeared"));
                };
                parent.children[position].1 = child.reference.clone();
                child_locators[position] = child.locator;
            }
            None => {
                let Some(position) = parent
                    .children
                    .iter()
                    .position(|(old_edge, _)| *old_edge == edge)
                else {
                    return Err(invalid("packed update parent edge disappeared"));
                };
                parent.children.remove(position);
                child_locators.remove(position);
            }
        }
        replacement = normalize_node_v2(
            &descriptor.kind,
            parent,
            child_locators,
            limits,
            writer,
            deadline,
            cancelled,
        )?;
    }
    Ok(replacement)
}

fn normalize_node_v2(
    kind: &[u8],
    node: TreeNode,
    child_locators: Vec<TreeLocatorV2>,
    limits: AuthenticatedTreeLimitsV1,
    writer: &mut PackWriterV2<'_, '_>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Option<TreeHandleV2>> {
    check(deadline, cancelled)?;
    if child_locators.len() != node.children.len() {
        return Err(invalid("packed normalized child locators differ"));
    }
    if node.value.is_none() && node.children.is_empty() {
        return Ok(None);
    }
    if node.value.is_none() && node.children.len() == 1 {
        let ((_, reference), locator) = node
            .children
            .into_iter()
            .zip(child_locators)
            .next()
            .ok_or_else(|| invalid("packed normalized child is missing"))?;
        return Ok(Some(TreeHandleV2 { reference, locator }));
    }
    let node = make_node(node.value, node.children, limits)?;
    writer
        .persist(kind, node, &child_locators, child_locators.capacity())
        .map(Some)
}

fn descriptor_root_handle(
    descriptor: &AuthenticatedTreeDescriptorV2,
) -> Result<Option<TreeHandleV2>> {
    let Some(reference) = descriptor.semantic.root.clone() else {
        if descriptor.physical_root.is_some() || descriptor.semantic.entries != 0 {
            return Err(invalid("empty packed descriptor shape differs"));
        }
        return Ok(None);
    };
    let locator = match &descriptor.physical_root {
        Some(locator) => {
            validate_locator(locator)?;
            let state = Arc::new(PackSealStateV2 {
                pack_id: locator.pack_id,
                digest: OnceLock::new(),
            });
            state
                .digest
                .set(locator.pack_digest)
                .map_err(|_| invalid("packed root digest state differs"))?;
            TreeLocatorV2::Packed(PackedHandleV2 {
                state,
                offset: locator.offset,
                frame_len: locator.frame_len,
                frame_sha256: locator.frame_sha256,
            })
        }
        None => TreeLocatorV2::Legacy(reference.digest),
    };
    Ok(Some(TreeHandleV2 { reference, locator }))
}

fn public_locator(locator: &TreeLocatorV2) -> Result<AuthenticatedTreeLocatorV2> {
    let TreeLocatorV2::Packed(handle) = locator else {
        return Err(invalid("legacy node has no packed locator"));
    };
    let pack_digest = handle
        .state
        .digest
        .get()
        .copied()
        .ok_or_else(|| invalid("packed node chunk is not sealed"))?;
    let locator = AuthenticatedTreeLocatorV2 {
        pack_digest,
        pack_id: handle.state.pack_id,
        offset: handle.offset,
        frame_len: handle.frame_len,
        frame_sha256: handle.frame_sha256,
    };
    validate_locator(&locator)?;
    Ok(locator)
}

fn validate_descriptor_v2_shape(descriptor: &AuthenticatedTreeDescriptorV2) -> Result<()> {
    match (&descriptor.semantic.root, &descriptor.physical_root) {
        (None, None) if descriptor.semantic.entries == 0 => Ok(()),
        (Some(root), Some(locator)) if descriptor.semantic.entries == root.entries => {
            validate_locator(locator)
        }
        (Some(root), None) if descriptor.semantic.entries == root.entries => Ok(()),
        _ => Err(invalid("packed descriptor root shape differs")),
    }
}

fn validate_locator(locator: &AuthenticatedTreeLocatorV2) -> Result<()> {
    if locator.frame_len == 0
        || locator.frame_len as usize > AUTHENTICATED_PACK_MAX_BYTES
        || locator
            .offset
            .checked_add(locator.frame_len as u64)
            .is_none()
    {
        return Err(invalid("packed node locator fields differ"));
    }
    Ok(())
}

fn validate_locator_for_limits(
    locator: &AuthenticatedTreeLocatorV2,
    limits: AuthenticatedTreeLimitsV1,
) -> Result<()> {
    validate_locator(locator)?;
    validate_frame_len_for_limits(locator.frame_len, limits)
}

fn validate_frame_len_for_limits(frame_len: u32, limits: AuthenticatedTreeLimitsV1) -> Result<()> {
    let max_frame = limits
        .max_node_bytes
        .checked_add(45)
        .and_then(|size| size.checked_add(limits.max_children.checked_mul(93)?))
        .ok_or_else(|| budget("packed node frame limit overflow"))?;
    if frame_len as usize > max_frame {
        return Err(budget("packed node frame exceeds canonical node envelope"));
    }
    Ok(())
}

fn locator_encoded_len() -> usize {
    32 + 16 + 8 + 4 + 32
}

fn encode_locator(raw: &mut Vec<u8>, locator: &AuthenticatedTreeLocatorV2) {
    raw.extend_from_slice(locator.pack_digest.as_bytes());
    raw.extend_from_slice(&locator.pack_id);
    raw.extend_from_slice(&locator.offset.to_le_bytes());
    raw.extend_from_slice(&locator.frame_len.to_le_bytes());
    raw.extend_from_slice(locator.frame_sha256.as_bytes());
}

fn decode_locator(reader: &mut Reader<'_>) -> Result<AuthenticatedTreeLocatorV2> {
    let locator = AuthenticatedTreeLocatorV2 {
        pack_digest: digest_from_raw(reader.take(32)?)?,
        pack_id: reader.array::<16>()?,
        offset: reader.u64()?,
        frame_len: reader.u32()?,
        frame_sha256: digest_from_raw(reader.take(32)?)?,
    };
    validate_locator(&locator)?;
    Ok(locator)
}

fn encode_packed_frame(
    pack_id: [u8; 16],
    offset: u64,
    canonical: &[u8],
    children: &[TreeLocatorV2],
) -> Result<Vec<u8>> {
    if children.len() > MAX_CHILD_NIBBLES || canonical.len() > u32::MAX as usize {
        return Err(budget("packed node frame fields exceed limit"));
    }
    let capacity = canonical
        .len()
        .checked_add(45)
        .and_then(|size| size.checked_add(children.len().checked_mul(93)?))
        .ok_or_else(|| budget("packed node frame size overflow"))?;
    let mut raw = Vec::new();
    raw.try_reserve_exact(capacity)
        .map_err(|_| budget("packed node frame allocation failed"))?;
    raw.extend_from_slice(PACKED_FRAME_MAGIC);
    raw.extend_from_slice(&PACKED_FRAME_VERSION.to_le_bytes());
    raw.extend_from_slice(&0u16.to_le_bytes());
    raw.extend_from_slice(&pack_id);
    raw.extend_from_slice(&offset.to_le_bytes());
    let frame_len_at = raw.len();
    raw.extend_from_slice(&0u32.to_le_bytes());
    raw.push(children.len() as u8);
    raw.extend_from_slice(&(canonical.len() as u32).to_le_bytes());
    for child in children {
        match child {
            TreeLocatorV2::Legacy(_) => raw.push(2),
            TreeLocatorV2::Packed(handle) if handle.state.pack_id == pack_id => {
                raw.push(0);
                raw.extend_from_slice(&handle.offset.to_le_bytes());
                raw.extend_from_slice(&handle.frame_len.to_le_bytes());
                raw.extend_from_slice(handle.frame_sha256.as_bytes());
            }
            TreeLocatorV2::Packed(handle) => {
                let digest = handle
                    .state
                    .digest
                    .get()
                    .copied()
                    .ok_or_else(|| invalid("external child pack is not sealed"))?;
                raw.push(1);
                raw.extend_from_slice(digest.as_bytes());
                raw.extend_from_slice(&handle.state.pack_id);
                raw.extend_from_slice(&handle.offset.to_le_bytes());
                raw.extend_from_slice(&handle.frame_len.to_le_bytes());
                raw.extend_from_slice(handle.frame_sha256.as_bytes());
            }
        }
    }
    raw.extend_from_slice(canonical);
    let frame_len = u32::try_from(raw.len()).map_err(|_| budget("packed frame is too long"))?;
    raw[frame_len_at..frame_len_at + 4].copy_from_slice(&frame_len.to_le_bytes());
    Ok(raw)
}

enum PackedChildWireV2 {
    Local {
        offset: u64,
        frame_len: u32,
        frame_sha256: Digest256,
    },
    External(AuthenticatedTreeLocatorV2),
    Legacy,
}

fn decode_packed_frame(
    raw: &[u8],
    locator: &PackedHandleV2,
    store_id: [u8; 16],
    domain_digest: Digest256,
    kind: &[u8],
    limits: AuthenticatedTreeLimitsV1,
) -> Result<(TreeNode, Vec<TreeLocatorV2>)> {
    if raw.len() != locator.frame_len as usize || Digest256::of_bytes(raw) != locator.frame_sha256 {
        return Err(SegmentError::new(
            Code::CorruptBytes,
            "packed node frame digest differs",
        ));
    }
    let mut reader = Reader::new(raw);
    if reader.take(8)? != PACKED_FRAME_MAGIC
        || reader.u16()? != PACKED_FRAME_VERSION
        || reader.u16()? != 0
        || reader.array::<16>()? != locator.state.pack_id
        || reader.u64()? != locator.offset
        || reader.u32()? as usize != raw.len()
    {
        return Err(invalid("packed node frame header differs"));
    }
    let child_count = reader.u8()? as usize;
    let canonical_len = reader.u32()? as usize;
    if child_count > limits.max_children || child_count > MAX_CHILD_NIBBLES {
        return Err(budget("packed node child locator count exceeds limit"));
    }
    let mut wire_locators = Vec::new();
    wire_locators
        .try_reserve_exact(child_count)
        .map_err(|_| budget("packed child locator allocation failed"))?;
    for _ in 0..child_count {
        wire_locators.push(match reader.u8()? {
            0 => PackedChildWireV2::Local {
                offset: reader.u64()?,
                frame_len: reader.u32()?,
                frame_sha256: digest_from_raw(reader.take(32)?)?,
            },
            1 => PackedChildWireV2::External(decode_locator(&mut reader)?),
            2 => PackedChildWireV2::Legacy,
            _ => return Err(invalid("packed child locator tag differs")),
        });
    }
    if canonical_len > limits.max_node_bytes {
        return Err(budget("canonical authenticated node exceeds limit"));
    }
    let canonical = reader.take(canonical_len)?;
    reader.finish()?;
    let node = decode_node(canonical, store_id, domain_digest, kind, limits)?;
    if node.children.len() != wire_locators.len() {
        return Err(invalid("packed child locator arity differs"));
    }
    let mut child_locators = Vec::new();
    child_locators
        .try_reserve_exact(node.children.len())
        .map_err(|_| budget("packed child locator allocation failed"))?;
    for ((_, reference), wire) in node.children.iter().zip(wire_locators) {
        let child = match wire {
            PackedChildWireV2::Local {
                offset,
                frame_len,
                frame_sha256,
            } => {
                let child = PackedHandleV2 {
                    state: locator.state.clone(),
                    offset,
                    frame_len,
                    frame_sha256,
                };
                validate_child_handle(&child, limits)?;
                TreeLocatorV2::Packed(child)
            }
            PackedChildWireV2::External(child_locator) => {
                if child_locator.pack_id == locator.state.pack_id {
                    return Err(invalid("same-pack child used external locator"));
                }
                let state = Arc::new(PackSealStateV2 {
                    pack_id: child_locator.pack_id,
                    digest: OnceLock::new(),
                });
                state
                    .digest
                    .set(child_locator.pack_digest)
                    .map_err(|_| invalid("external child digest state differs"))?;
                let child = PackedHandleV2 {
                    state,
                    offset: child_locator.offset,
                    frame_len: child_locator.frame_len,
                    frame_sha256: child_locator.frame_sha256,
                };
                validate_child_handle(&child, limits)?;
                TreeLocatorV2::Packed(child)
            }
            PackedChildWireV2::Legacy => TreeLocatorV2::Legacy(reference.digest),
        };
        child_locators.push(child);
    }
    Ok((node, child_locators))
}

fn validate_child_handle(handle: &PackedHandleV2, limits: AuthenticatedTreeLimitsV1) -> Result<()> {
    if handle.frame_len == 0
        || handle.frame_len as usize > AUTHENTICATED_PACK_MAX_BYTES
        || handle.offset.checked_add(handle.frame_len as u64).is_none()
    {
        return Err(invalid("packed child locator fields differ"));
    }
    validate_frame_len_for_limits(handle.frame_len, limits)?;
    Ok(())
}

fn load_node_v2(
    store: &SegmentStore,
    descriptor: &AuthenticatedTreeDescriptorV2,
    handle: &TreeHandleV2,
    limits: AuthenticatedTreeLimitsV1,
    work: &mut AuthenticatedTreeWorkV1,
    active_pack: Option<&ActivePackV2>,
    io_ledger: Option<&dyn AuthenticatedTreeIoLedgerV1>,
    deadline: Instant,
    cancelled: &AtomicBool,
    shared_work: &mut Option<&mut dyn FnMut() -> bool>,
) -> Result<LoadedTreeNodeV2> {
    check(deadline, cancelled)?;
    if shared_work.as_mut().is_some_and(|charge| !(**charge)()) {
        return Err(budget("authenticated tree shared work refused"));
    }
    match &handle.locator {
        TreeLocatorV2::Legacy(digest) => {
            if *digest != handle.reference.digest {
                return Err(invalid("legacy node locator differs from semantic address"));
            }
            let remaining = remaining_bytes(*work, limits)?;
            let max_bytes = limits.max_node_bytes.min(remaining);
            charge_tree_read(io_ledger, max_bytes as u64)?;
            let before = work.read_bytes;
            let node = load_node(
                store,
                &descriptor.semantic,
                &handle.reference,
                limits,
                work,
                deadline,
                cancelled,
            )?;
            record_tree_read(
                io_ledger,
                work.read_bytes
                    .checked_sub(before)
                    .ok_or_else(|| budget("legacy V2 read work regressed"))?,
            )?;
            let child_locators = node
                .children
                .iter()
                .map(|(_, child)| TreeLocatorV2::Legacy(child.digest))
                .collect();
            Ok(LoadedTreeNodeV2 {
                node,
                child_locators,
                physical_pack_digest: None,
            })
        }
        TreeLocatorV2::Packed(locator) => {
            validate_child_handle(locator, limits)?;
            if work.total_nodes() >= limits.max_nodes {
                return Err(budget("authenticated tree node budget exceeded"));
            }
            let remaining = remaining_bytes(*work, limits)?;
            if locator.frame_len as usize > remaining {
                return Err(budget("authenticated tree byte budget exceeded"));
            }
            let raw = if let Some(digest) = locator.state.digest.get().copied() {
                charge_tree_read(io_ledger, locator.frame_len as u64)?;
                store.read_authenticated_blob_range(
                    digest,
                    locator.offset,
                    locator.frame_len as usize,
                    AUTHENTICATED_PACK_MAX_BYTES,
                    deadline,
                    cancelled,
                )?
            } else {
                let active = active_pack.filter(|pack| {
                    pack.state.pack_id == locator.state.pack_id
                        && Arc::ptr_eq(&pack.state, &locator.state)
                });
                let active = active.ok_or_else(|| invalid("packed node chunk is not sealed"))?;
                let start = usize::try_from(locator.offset)
                    .map_err(|_| budget("active frame offset exceeds address space"))?;
                let end = start
                    .checked_add(locator.frame_len as usize)
                    .ok_or_else(|| budget("active frame range overflow"))?;
                let bytes = active
                    .raw
                    .get(start..end)
                    .ok_or_else(|| invalid("active frame lies outside current pack"))?;
                let mut copy = Vec::new();
                copy.try_reserve_exact(bytes.len())
                    .map_err(|_| budget("active frame allocation failed"))?;
                copy.extend_from_slice(bytes);
                copy
            };
            if locator.state.digest.get().is_some() {
                record_tree_read(io_ledger, raw.len() as u64)?;
            }
            work.charge_read(raw.len(), limits)?;
            let (node, child_locators) = decode_packed_frame(
                &raw,
                locator,
                store.store_id(),
                store.domain_digest(),
                &descriptor.kind,
                limits,
            )?;
            if node.entries != handle.reference.entries
                || node.min_key != handle.reference.min_key
                || node.max_key != handle.reference.max_key
            {
                return Err(SegmentError::new(
                    Code::CorruptBytes,
                    "packed authenticated child summary differs",
                ));
            }
            let canonical = encode_node(
                store.store_id(),
                store.domain_digest(),
                &descriptor.kind,
                &node,
                limits,
            )?;
            if Digest256::of_bytes(&canonical) != handle.reference.digest {
                return Err(SegmentError::new(
                    Code::CorruptBytes,
                    "packed canonical node digest differs",
                ));
            }
            check(deadline, cancelled)?;
            Ok(LoadedTreeNodeV2 {
                node,
                child_locators,
                // An unsealed pack is only reachable through the matching
                // writer-owned active buffer validated above. Its frame has
                // already passed both the frame and canonical node digests;
                // cold traversal records a pack digest only after sealing.
                physical_pack_digest: locator.state.digest.get().copied(),
            })
        }
    }
}

fn scan_packed_chunk(raw: &[u8]) -> Result<u64> {
    if raw.is_empty() || raw.len() > AUTHENTICATED_PACK_MAX_BYTES {
        return Err(budget("authenticated chunk length exceeds limit"));
    }
    let mut at = 0usize;
    let mut pack_id: Option<[u8; 16]> = None;
    let mut frames = 0u64;
    while at < raw.len() {
        let remaining = raw
            .get(at..)
            .ok_or_else(|| invalid("authenticated pack offset differs"))?;
        let mut reader = Reader::new(remaining);
        if reader.take(8)? != PACKED_FRAME_MAGIC
            || reader.u16()? != PACKED_FRAME_VERSION
            || reader.u16()? != 0
        {
            return Err(invalid("authenticated pack frame header differs"));
        }
        let id = reader.array::<16>()?;
        let offset = reader.u64()?;
        let frame_len = reader.u32()? as usize;
        if offset != at as u64 || frame_len < 45 || frame_len > remaining.len() {
            return Err(invalid("authenticated pack frame bounds differ"));
        }
        if pack_id.is_some_and(|previous| previous != id) {
            return Err(invalid("authenticated pack contains mixed pack IDs"));
        }
        pack_id = Some(id);
        at = at
            .checked_add(frame_len)
            .ok_or_else(|| budget("authenticated pack frame offset overflow"))?;
        frames = frames
            .checked_add(1)
            .ok_or_else(|| budget("authenticated pack frame count overflow"))?;
    }
    if frames == 0 {
        return Err(invalid("authenticated pack contains no frames"));
    }
    Ok(frames)
}

#[derive(Default)]
struct BuildFrame {
    value: Option<AuthenticatedTreeEntryV1>,
    children: Vec<(u8, AuthenticatedTreeNodeRefV1)>,
}

fn attach_build_child(
    parent: &mut BuildFrame,
    edge: u8,
    child: AuthenticatedTreeNodeRefV1,
    limits: AuthenticatedTreeLimitsV1,
) -> Result<()> {
    if parent.children.len() >= limits.max_children {
        return Err(budget("authenticated node child limit exceeded"));
    }
    if parent
        .children
        .last()
        .is_some_and(|(previous, _)| *previous >= edge)
    {
        return Err(invalid("authenticated builder child order differs"));
    }
    parent
        .children
        .try_reserve(1)
        .map_err(|_| budget("authenticated builder child allocation failed"))?;
    parent.children.push((edge, child));
    Ok(())
}

fn close_build_frame(
    store: &SegmentStore,
    kind: &[u8],
    frame: BuildFrame,
    limits: AuthenticatedTreeLimitsV1,
    work: &mut AuthenticatedTreeWorkV1,
    pending_payload_bytes: &mut u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Option<AuthenticatedTreeNodeRefV1>> {
    check(deadline, cancelled)?;
    if frame.value.is_none() && frame.children.is_empty() {
        return Ok(None);
    }
    if frame.value.is_none() && frame.children.len() == 1 {
        return Ok(frame.children.into_iter().next().map(|(_, child)| child));
    }
    let payload_bytes = frame
        .value
        .as_ref()
        .map(|entry| {
            u64::try_from(entry.key.len())
                .ok()
                .and_then(|key| {
                    u64::try_from(entry.value.len())
                        .ok()
                        .and_then(|value| key.checked_add(value))
                })
                .ok_or_else(|| budget("authenticated pending row size overflow"))
        })
        .transpose()?
        .unwrap_or(0);
    let node = make_node(frame.value, frame.children, limits)?;
    let reference = persist_node(store, kind, node, limits, work, deadline, cancelled)?;
    *pending_payload_bytes = pending_payload_bytes
        .checked_sub(payload_bytes)
        .ok_or_else(|| invalid("authenticated pending payload accounting differs"))?;
    Ok(Some(reference))
}

struct BuildCursor<I> {
    rows: I,
    pending: Option<AuthenticatedTreeEntryV1>,
    finished: bool,
    previous: Option<Vec<u8>>,
    consumed: u64,
}

impl<I> BuildCursor<I>
where
    I: Iterator<Item = Result<AuthenticatedTreeEntryV1>>,
{
    fn new(rows: I) -> Self {
        Self {
            rows,
            pending: None,
            finished: false,
            previous: None,
            consumed: 0,
        }
    }

    fn fill(&mut self, deadline: Instant, cancelled: &AtomicBool) -> Result<()> {
        if self.pending.is_some() || self.finished {
            return Ok(());
        }
        check(deadline, cancelled)?;
        match self.rows.next() {
            Some(Ok(row)) => self.pending = Some(row),
            Some(Err(error)) => return Err(error),
            None => self.finished = true,
        }
        check(deadline, cancelled)
    }

    fn take(
        &mut self,
        limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<AuthenticatedTreeEntryV1>> {
        self.fill(deadline, cancelled)?;
        let Some(row) = self.pending.take() else {
            return Ok(None);
        };
        validate_key_value(&row.key, Some(&row.value), limits)?;
        if self
            .previous
            .as_deref()
            .is_some_and(|key| row.key.as_slice() <= key)
        {
            return Err(SegmentError::new(
                Code::InvalidFormat,
                "authenticated builder keys are not strictly ordered",
            ));
        }
        self.consumed = self
            .consumed
            .checked_add(1)
            .ok_or_else(|| budget("tree row counter overflow"))?;
        if self.consumed > limits.max_rows {
            return Err(budget("authenticated build row limit exceeded"));
        }
        self.previous = Some(row.key.clone());
        Ok(Some(row))
    }
}

fn update_one(
    store: &SegmentStore,
    descriptor: &AuthenticatedTreeDescriptorV1,
    root: Option<AuthenticatedTreeNodeRefV1>,
    change: AuthenticatedTreeDeltaV1,
    limits: AuthenticatedTreeLimitsV1,
    work: &mut AuthenticatedTreeWorkV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Option<AuthenticatedTreeNodeRefV1>> {
    let Some(mut reference) = root.clone() else {
        return match change.value {
            None => Ok(None),
            Some(value) => {
                let node = make_node(
                    Some(AuthenticatedTreeEntryV1 {
                        key: change.key,
                        value,
                    }),
                    Vec::new(),
                    limits,
                )?;
                persist_node(
                    store,
                    &descriptor.kind,
                    node,
                    limits,
                    work,
                    deadline,
                    cancelled,
                )
                .map(Some)
            }
        };
    };
    let mut ancestors: Vec<(TreeNode, u8)> = Vec::new();
    let mut replacement: Option<AuthenticatedTreeNodeRefV1>;
    loop {
        check(deadline, cancelled)?;
        let node = load_node(
            store, descriptor, &reference, limits, work, deadline, cancelled,
        )?;
        let prefix_len = node_prefix_nibbles(&node);
        let shared = common_prefix_nibbles(&change.key, &node.min_key);
        if shared < prefix_len {
            let Some(value) = change.value.clone() else {
                return Ok(root);
            };
            let old_edge = nibble_at(&node.min_key, shared)
                .ok_or_else(|| invalid("authenticated split lacks old edge"))?;
            let parent = if change.key.len() * 2 == shared {
                make_node(
                    Some(AuthenticatedTreeEntryV1 {
                        key: change.key.clone(),
                        value,
                    }),
                    vec![(old_edge, reference.clone())],
                    limits,
                )?
            } else {
                let new_edge = nibble_at(&change.key, shared)
                    .ok_or_else(|| invalid("authenticated split lacks new edge"))?;
                if new_edge == old_edge {
                    return Err(invalid("authenticated split edge did not diverge"));
                }
                let leaf = make_node(
                    Some(AuthenticatedTreeEntryV1 {
                        key: change.key.clone(),
                        value,
                    }),
                    Vec::new(),
                    limits,
                )?;
                let new_ref = persist_node(
                    store,
                    &descriptor.kind,
                    leaf,
                    limits,
                    work,
                    deadline,
                    cancelled,
                )?;
                let mut children = vec![(old_edge, reference.clone()), (new_edge, new_ref)];
                children.sort_by_key(|(edge, _)| *edge);
                make_node(None, children, limits)?
            };
            replacement = Some(persist_node(
                store,
                &descriptor.kind,
                parent,
                limits,
                work,
                deadline,
                cancelled,
            )?);
            break;
        }

        let key_nibbles = change.key.len() * 2;
        if key_nibbles == prefix_len {
            match (&node.value, &change.value) {
                (Some(old), Some(new)) if old.value.as_slice() == new.as_slice() => {
                    return Ok(root);
                }
                (None, None) => return Ok(root),
                _ => {}
            }
            let mut updated = node;
            updated.value = change.value.clone().map(|value| AuthenticatedTreeEntryV1 {
                key: change.key.clone(),
                value,
            });
            replacement = normalize_node(
                store,
                &descriptor.kind,
                updated,
                limits,
                work,
                deadline,
                cancelled,
            )?;
            break;
        }
        let Some(edge) = nibble_at(&change.key, prefix_len) else {
            return Ok(root);
        };
        if let Some((_, child)) = node
            .children
            .iter()
            .find(|(child_edge, _)| *child_edge == edge)
        {
            let child = child.clone();
            ancestors.push((node, edge));
            reference = child;
            continue;
        }
        let Some(value) = change.value.clone() else {
            return Ok(root);
        };
        let leaf = make_node(
            Some(AuthenticatedTreeEntryV1 {
                key: change.key.clone(),
                value,
            }),
            Vec::new(),
            limits,
        )?;
        let leaf_ref = persist_node(
            store,
            &descriptor.kind,
            leaf,
            limits,
            work,
            deadline,
            cancelled,
        )?;
        let mut updated = node;
        updated.children.push((edge, leaf_ref));
        updated.children.sort_by_key(|(child_edge, _)| *child_edge);
        replacement = normalize_node(
            store,
            &descriptor.kind,
            updated,
            limits,
            work,
            deadline,
            cancelled,
        )?;
        break;
    }
    while let Some((mut parent, edge)) = ancestors.pop() {
        match replacement.take() {
            Some(child) => {
                let Some(position) = parent
                    .children
                    .iter()
                    .position(|(old_edge, _)| *old_edge == edge)
                else {
                    return Err(invalid("authenticated update parent edge disappeared"));
                };
                parent.children[position].1 = child;
            }
            None => parent.children.retain(|(old_edge, _)| *old_edge != edge),
        }
        replacement = normalize_node(
            store,
            &descriptor.kind,
            parent,
            limits,
            work,
            deadline,
            cancelled,
        )?;
    }
    Ok(replacement)
}

fn normalize_node(
    store: &SegmentStore,
    kind: &[u8],
    node: TreeNode,
    limits: AuthenticatedTreeLimitsV1,
    work: &mut AuthenticatedTreeWorkV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Option<AuthenticatedTreeNodeRefV1>> {
    if node.value.is_none() && node.children.is_empty() {
        return Ok(None);
    }
    if node.value.is_none() && node.children.len() == 1 {
        return Ok(node.children.into_iter().next().map(|(_, child)| child));
    }
    // Updates mutate the value/child set of a decoded parent. Its previous
    // count and bounds describe the old root, so rebuild the canonical summary
    // before serialization; decoded immutable nodes keep strict validation.
    let node = make_node(node.value, node.children, limits)?;
    persist_node(store, kind, node, limits, work, deadline, cancelled).map(Some)
}

#[derive(Clone, Debug)]
struct TreeNode {
    entries: u64,
    min_key: Vec<u8>,
    max_key: Vec<u8>,
    value: Option<AuthenticatedTreeEntryV1>,
    children: Vec<(u8, AuthenticatedTreeNodeRefV1)>,
}

fn make_node(
    value: Option<AuthenticatedTreeEntryV1>,
    children: Vec<(u8, AuthenticatedTreeNodeRefV1)>,
    limits: AuthenticatedTreeLimitsV1,
) -> Result<TreeNode> {
    if children.len() > limits.max_children || children.len() > MAX_CHILD_NIBBLES {
        return Err(budget("authenticated node child limit exceeded"));
    }
    if value.is_none() && children.len() < 2 {
        return Err(invalid("authenticated Patricia branch is not compressed"));
    }
    if value.is_none() && children.is_empty() {
        return Err(invalid("authenticated node has no rows"));
    }
    let mut entries = u64::from(value.is_some());
    for (index, (edge, child)) in children.iter().enumerate() {
        if *edge > 15
            || child.entries == 0
            || child.min_key.is_empty()
            || child.min_key.len() > limits.max_key_bytes
            || child.max_key.len() > limits.max_key_bytes
            || child.min_key > child.max_key
            || (index > 0 && children[index - 1].0 >= *edge)
        {
            return Err(invalid("authenticated child reference differs"));
        }
        entries = entries
            .checked_add(child.entries)
            .ok_or_else(|| budget("tree entry count overflow"))?;
    }
    let min_key = value
        .as_ref()
        .map(|entry| entry.key.clone())
        .or_else(|| children.first().map(|(_, child)| child.min_key.clone()))
        .ok_or_else(|| invalid("authenticated node minimum key missing"))?;
    let max_key = children
        .last()
        .map(|(_, child)| child.max_key.clone())
        .or_else(|| value.as_ref().map(|entry| entry.key.clone()))
        .ok_or_else(|| invalid("authenticated node maximum key missing"))?;
    let node = TreeNode {
        entries,
        min_key,
        max_key,
        value,
        children,
    };
    validate_node(&node, limits)?;
    Ok(node)
}

fn validate_node(node: &TreeNode, limits: AuthenticatedTreeLimitsV1) -> Result<()> {
    if node.entries == 0
        || node.min_key.is_empty()
        || node.max_key.is_empty()
        || node.min_key.len() > limits.max_key_bytes
        || node.max_key.len() > limits.max_key_bytes
        || node.min_key > node.max_key
        || node.children.len() > limits.max_children
        || node.children.len() > MAX_CHILD_NIBBLES
        || (node.value.is_none() && node.children.len() < 2)
    {
        return Err(invalid("authenticated node summary differs"));
    }
    if let Some(value) = &node.value {
        validate_key_value(&value.key, Some(&value.value), limits)?;
    }
    for (index, (edge, child)) in node.children.iter().enumerate() {
        if *edge > 15
            || child.entries == 0
            || child.min_key.is_empty()
            || child.min_key.len() > limits.max_key_bytes
            || child.max_key.len() > limits.max_key_bytes
            || child.min_key > child.max_key
            || (index > 0 && node.children[index - 1].0 >= *edge)
            || (index > 0 && node.children[index - 1].1.max_key >= child.min_key)
        {
            return Err(invalid("authenticated node child order differs"));
        }
    }
    let expected_count =
        node.children
            .iter()
            .try_fold(u64::from(node.value.is_some()), |sum, (_, child)| {
                sum.checked_add(child.entries)
                    .ok_or_else(|| budget("tree entry count overflow"))
            })?;
    let expected_min = node
        .value
        .as_ref()
        .map(|entry| entry.key.as_slice())
        .or_else(|| {
            node.children
                .first()
                .map(|(_, child)| child.min_key.as_slice())
        })
        .ok_or_else(|| invalid("authenticated node has no minimum"))?;
    let expected_max = node
        .children
        .last()
        .map(|(_, child)| child.max_key.as_slice())
        .or_else(|| node.value.as_ref().map(|entry| entry.key.as_slice()))
        .ok_or_else(|| invalid("authenticated node has no maximum"))?;
    if expected_count != node.entries
        || expected_min != node.min_key
        || expected_max != node.max_key
    {
        return Err(invalid("authenticated node count or bounds differ"));
    }
    let prefix_len = node_prefix_nibbles(node);
    if let Some(value) = &node.value {
        if value.key.len().checked_mul(2) != Some(prefix_len)
            || node.children.iter().any(|(_, child)| {
                !key_matches_prefix(&child.min_key, &value.key, prefix_len)
                    || !key_matches_prefix(&child.max_key, &value.key, prefix_len)
            })
        {
            return Err(invalid(
                "authenticated terminal row differs from branch prefix",
            ));
        }
    } else if node.children.len() < 2 {
        return Err(invalid("authenticated branch is not compressed"));
    }
    for (edge, child) in &node.children {
        if nibble_at(&child.min_key, prefix_len) != Some(*edge)
            || nibble_at(&child.max_key, prefix_len) != Some(*edge)
            || !key_matches_prefix(&child.min_key, &node.min_key, prefix_len)
            || !key_matches_prefix(&child.max_key, &node.min_key, prefix_len)
        {
            return Err(invalid("authenticated Patricia edge differs from bounds"));
        }
    }
    if node.value.is_none()
        && node.children.first().map(|(edge, _)| edge) == node.children.last().map(|(edge, _)| edge)
    {
        return Err(invalid(
            "authenticated Patricia node has one compressed edge",
        ));
    }
    Ok(())
}

fn node_prefix_nibbles(node: &TreeNode) -> usize {
    if let Some(value) = &node.value {
        value.key.len() * 2
    } else {
        common_prefix_nibbles(&node.min_key, &node.max_key)
    }
}

fn persist_node(
    store: &SegmentStore,
    kind: &[u8],
    node: TreeNode,
    limits: AuthenticatedTreeLimitsV1,
    work: &mut AuthenticatedTreeWorkV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<AuthenticatedTreeNodeRefV1> {
    check(deadline, cancelled)?;
    validate_node(&node, limits)?;
    let raw = encode_node(store.store_id(), store.domain_digest(), kind, &node, limits)?;
    if raw.len() > limits.max_node_bytes {
        return Err(budget("authenticated node exceeds byte limit"));
    }
    let digest = Digest256::of_bytes(&raw);
    let remaining = remaining_bytes(*work, limits)?;
    // A fresh install writes and reads back the complete node. Reserve the
    // worst case before touching storage; a reused object charges only its read.
    if work
        .total_nodes()
        .checked_add(2)
        .is_none_or(|n| n > limits.max_nodes)
        || raw.len().checked_mul(2).is_none_or(|n| n > remaining)
    {
        return Err(budget("authenticated tree node budget exceeded"));
    }
    let install = store.install_authenticated_blob(
        digest,
        &raw,
        limits.max_node_bytes.min(remaining),
        deadline,
        cancelled,
    )?;
    work.charge_install(install, limits)?;
    Ok(AuthenticatedTreeNodeRefV1 {
        digest,
        entries: node.entries,
        min_key: node.min_key,
        max_key: node.max_key,
    })
}

fn load_node(
    store: &SegmentStore,
    descriptor: &AuthenticatedTreeDescriptorV1,
    reference: &AuthenticatedTreeNodeRefV1,
    limits: AuthenticatedTreeLimitsV1,
    work: &mut AuthenticatedTreeWorkV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<TreeNode> {
    if work.total_nodes() >= limits.max_nodes {
        return Err(budget("authenticated tree node budget exceeded"));
    }
    let remaining = remaining_bytes(*work, limits)?;
    if remaining == 0 {
        return Err(budget("authenticated tree byte budget exceeded"));
    }
    let max_bytes = limits.max_node_bytes.min(remaining);
    let raw = store.read_authenticated_blob(reference.digest, max_bytes, deadline, cancelled)?;
    work.charge_read(raw.len(), limits)?;
    let node = decode_node(
        &raw,
        store.store_id(),
        store.domain_digest(),
        &descriptor.kind,
        limits,
    )?;
    if node.entries != reference.entries
        || node.min_key != reference.min_key
        || node.max_key != reference.max_key
    {
        return Err(SegmentError::new(
            Code::CorruptBytes,
            "authenticated child summary differs from node bytes",
        ));
    }
    Ok(node)
}

fn make_descriptor(
    store: &SegmentStore,
    kind: &[u8],
    root: Option<AuthenticatedTreeNodeRefV1>,
    entries: u64,
) -> Result<AuthenticatedTreeDescriptorV1> {
    if root.as_ref().map_or(0, |root| root.entries) != entries {
        return Err(invalid("authenticated root count differs"));
    }
    let mut descriptor = AuthenticatedTreeDescriptorV1 {
        store_id: store.store_id(),
        domain_digest: store.domain_digest(),
        kind: kind.to_vec(),
        root,
        entries,
        commitment: Digest256::of_bytes(b""),
    };
    descriptor.commitment = descriptor_root_commitment(
        descriptor.store_id,
        descriptor.domain_digest,
        &descriptor.kind,
        descriptor.root.as_ref(),
        descriptor.entries,
    );
    Ok(descriptor)
}

fn validate_descriptor_for_store(
    store: &SegmentStore,
    descriptor: &AuthenticatedTreeDescriptorV1,
    limits: AuthenticatedTreeLimitsV1,
) -> Result<()> {
    validate_kind(&descriptor.kind, limits)?;
    validate_descriptor_shape(descriptor, usize::MAX)?;
    if descriptor.store_id != store.store_id()
        || descriptor.domain_digest != store.domain_digest()
        || descriptor.commitment
            != descriptor_root_commitment(
                descriptor.store_id,
                descriptor.domain_digest,
                &descriptor.kind,
                descriptor.root.as_ref(),
                descriptor.entries,
            )
        || descriptor.root.as_ref().map_or(0, |root| root.entries) != descriptor.entries
        || descriptor.root.as_ref().is_some_and(|root| {
            root.min_key.len() > limits.max_key_bytes || root.max_key.len() > limits.max_key_bytes
        })
    {
        return Err(SegmentError::new(
            Code::InvalidReceipt,
            "authenticated tree descriptor or store binding differs",
        ));
    }
    Ok(())
}

fn validate_descriptor_shape(
    descriptor: &AuthenticatedTreeDescriptorV1,
    max_bytes: usize,
) -> Result<()> {
    if descriptor.kind.is_empty()
        || descriptor.kind.len() > u16::MAX as usize
        || descriptor.root.as_ref().map_or(0, |root| root.entries) != descriptor.entries
        || descriptor.root.as_ref().is_some_and(|root| {
            root.entries == 0
                || root.min_key.is_empty()
                || root.max_key.is_empty()
                || root.min_key > root.max_key
                || root.min_key.len() > max_bytes
                || root.max_key.len() > max_bytes
        })
    {
        return Err(invalid("authenticated descriptor fields differ"));
    }
    if descriptor.commitment
        != descriptor_root_commitment(
            descriptor.store_id,
            descriptor.domain_digest,
            &descriptor.kind,
            descriptor.root.as_ref(),
            descriptor.entries,
        )
    {
        return Err(SegmentError::new(
            Code::CorruptBytes,
            "authenticated descriptor commitment differs",
        ));
    }
    Ok(())
}

fn descriptor_encoded_len(descriptor: &AuthenticatedTreeDescriptorV1) -> Result<usize> {
    let mut len = 8usize + 2 + 2 + 16 + 32 + 2;
    len = len
        .checked_add(descriptor.kind.len())
        .ok_or_else(|| budget("descriptor size overflow"))?;
    len = len
        .checked_add(8 + 1 + 32)
        .ok_or_else(|| budget("descriptor size overflow"))?;
    if let Some(root) = &descriptor.root {
        len = len
            .checked_add(node_ref_encoded_len(root)?)
            .ok_or_else(|| budget("descriptor size overflow"))?;
    }
    Ok(len)
}

fn node_ref_encoded_len(reference: &AuthenticatedTreeNodeRefV1) -> Result<usize> {
    32usize
        .checked_add(8 + 4)
        .and_then(|len| len.checked_add(reference.min_key.len()))
        .and_then(|len| len.checked_add(4 + reference.max_key.len()))
        .ok_or_else(|| budget("authenticated node reference size overflow"))
}

fn descriptor_root_commitment(
    store_id: [u8; 16],
    domain_digest: Digest256,
    kind: &[u8],
    root: Option<&AuthenticatedTreeNodeRefV1>,
    entries: u64,
) -> Digest256 {
    let mut hasher = Digest256Hasher::new();
    hasher.update(b"tos-authenticated-tree-root-v1\0");
    hasher.update(&store_id);
    hasher.update(domain_digest.as_bytes());
    hasher.update(&(kind.len() as u16).to_le_bytes());
    hasher.update(kind);
    hasher.update(&entries.to_le_bytes());
    match root {
        None => hasher.update(&[0]),
        Some(root) => {
            hasher.update(&[1]);
            hash_node_ref(&mut hasher, root);
        }
    }
    hasher.finalize()
}

fn encode_node(
    store_id: [u8; 16],
    domain_digest: Digest256,
    kind: &[u8],
    node: &TreeNode,
    limits: AuthenticatedTreeLimitsV1,
) -> Result<Vec<u8>> {
    validate_kind(kind, limits)?;
    validate_node(node, limits)?;
    let encoded_len = node_encoded_len(kind, node)?;
    if encoded_len > limits.max_node_bytes {
        return Err(budget("authenticated node exceeds byte limit"));
    }
    let mut raw = Vec::new();
    raw.try_reserve_exact(encoded_len)
        .map_err(|_| budget("authenticated node allocation failed"))?;
    raw.extend_from_slice(NODE_MAGIC);
    raw.extend_from_slice(&1u16.to_le_bytes());
    raw.extend_from_slice(&0u16.to_le_bytes());
    raw.extend_from_slice(&store_id);
    raw.extend_from_slice(domain_digest.as_bytes());
    raw.extend_from_slice(&(kind.len() as u16).to_le_bytes());
    raw.extend_from_slice(kind);
    raw.extend_from_slice(&node.entries.to_le_bytes());
    put_key(&mut raw, &node.min_key)?;
    put_key(&mut raw, &node.max_key)?;
    match &node.value {
        None => raw.push(0),
        Some(entry) => {
            raw.push(1);
            put_key(&mut raw, &entry.key)?;
            put_value(&mut raw, &entry.value)?;
        }
    }
    raw.push(node.children.len() as u8);
    for (edge, child) in &node.children {
        raw.push(*edge);
        encode_node_ref(&mut raw, child)?;
    }
    if raw.len() != encoded_len {
        return Err(invalid("authenticated node encoded length differs"));
    }
    Ok(raw)
}

fn decode_node(
    raw: &[u8],
    expected_store: [u8; 16],
    expected_domain: Digest256,
    expected_kind: &[u8],
    limits: AuthenticatedTreeLimitsV1,
) -> Result<TreeNode> {
    if raw.len() > limits.max_node_bytes {
        return Err(budget("authenticated node exceeds byte limit"));
    }
    let mut reader = Reader::new(raw);
    if reader.take(8)? != NODE_MAGIC || reader.u16()? != 1 || reader.u16()? != 0 {
        return Err(invalid("authenticated node header differs"));
    }
    if reader.array::<16>()? != expected_store
        || digest_from_raw(reader.take(32)?)? != expected_domain
    {
        return Err(SegmentError::new(
            Code::InvalidReceipt,
            "authenticated node store or domain differs",
        ));
    }
    let kind_len = reader.u16()? as usize;
    if kind_len == 0 || kind_len > limits.max_kind_bytes || reader.take(kind_len)? != expected_kind
    {
        return Err(SegmentError::new(
            Code::InvalidReceipt,
            "authenticated node kind differs",
        ));
    }
    let entries = reader.u64()?;
    let min_key = reader.key(limits.max_key_bytes)?;
    let max_key = reader.key(limits.max_key_bytes)?;
    let value = match reader.u8()? {
        0 => None,
        1 => Some(AuthenticatedTreeEntryV1 {
            key: reader.key(limits.max_key_bytes)?,
            value: reader.value(limits.max_value_bytes)?,
        }),
        _ => return Err(invalid("authenticated node row tag differs")),
    };
    let child_count = reader.u8()? as usize;
    if child_count > limits.max_children || child_count > MAX_CHILD_NIBBLES {
        return Err(budget("authenticated node child limit exceeded"));
    }
    let mut children = Vec::new();
    children
        .try_reserve_exact(child_count)
        .map_err(|_| budget("authenticated node child allocation failed"))?;
    for _ in 0..child_count {
        let edge = reader.u8()?;
        children.push((edge, decode_node_ref(&mut reader, limits.max_key_bytes)?));
    }
    reader.finish()?;
    let node = TreeNode {
        entries,
        min_key,
        max_key,
        value,
        children,
    };
    validate_node(&node, limits)?;
    Ok(node)
}

fn node_encoded_len(kind: &[u8], node: &TreeNode) -> Result<usize> {
    let mut len = 8usize + 2 + 2 + 16 + 32 + 2;
    len = len
        .checked_add(kind.len())
        .ok_or_else(|| budget("node size overflow"))?;
    len = len
        .checked_add(8 + 4 + node.min_key.len() + 4 + node.max_key.len() + 1 + 1)
        .ok_or_else(|| budget("node size overflow"))?;
    if let Some(value) = &node.value {
        len = len
            .checked_add(4 + value.key.len() + 4 + value.value.len())
            .ok_or_else(|| budget("node size overflow"))?;
    }
    for (_, child) in &node.children {
        len = len
            .checked_add(1 + node_ref_encoded_len(child)?)
            .ok_or_else(|| budget("node size overflow"))?;
    }
    Ok(len)
}

fn encode_node_ref(raw: &mut Vec<u8>, reference: &AuthenticatedTreeNodeRefV1) -> Result<()> {
    raw.extend_from_slice(reference.digest.as_bytes());
    raw.extend_from_slice(&reference.entries.to_le_bytes());
    put_key(raw, &reference.min_key)?;
    put_key(raw, &reference.max_key)?;
    Ok(())
}

fn decode_node_ref(
    reader: &mut Reader<'_>,
    max_key_bytes: usize,
) -> Result<AuthenticatedTreeNodeRefV1> {
    let reference = AuthenticatedTreeNodeRefV1 {
        digest: digest_from_raw(reader.take(32)?)?,
        entries: reader.u64()?,
        min_key: reader.key(max_key_bytes)?,
        max_key: reader.key(max_key_bytes)?,
    };
    if reference.entries == 0 || reference.min_key > reference.max_key {
        return Err(invalid("authenticated node reference fields differ"));
    }
    Ok(reference)
}

fn hash_node_ref(hasher: &mut Digest256Hasher, reference: &AuthenticatedTreeNodeRefV1) {
    hasher.update(reference.digest.as_bytes());
    hasher.update(&reference.entries.to_le_bytes());
    hasher.update(&(reference.min_key.len() as u32).to_le_bytes());
    hasher.update(&reference.min_key);
    hasher.update(&(reference.max_key.len() as u32).to_le_bytes());
    hasher.update(&reference.max_key);
}

fn put_key(raw: &mut Vec<u8>, key: &[u8]) -> Result<()> {
    let len = u32::try_from(key.len()).map_err(|_| budget("authenticated key is too long"))?;
    raw.extend_from_slice(&len.to_le_bytes());
    raw.extend_from_slice(key);
    Ok(())
}

fn put_value(raw: &mut Vec<u8>, value: &[u8]) -> Result<()> {
    let len = u32::try_from(value.len()).map_err(|_| budget("authenticated value is too long"))?;
    raw.extend_from_slice(&len.to_le_bytes());
    raw.extend_from_slice(value);
    Ok(())
}

fn encode_authenticated_object(
    store_id: [u8; 16],
    domain_digest: Digest256,
    kind: &[u8],
    payload: &[u8],
    max_bytes: usize,
) -> Result<Vec<u8>> {
    let len = 8usize
        .checked_add(2 + 2 + 16 + 32 + 2 + kind.len() + 8)
        .and_then(|len| len.checked_add(payload.len()))
        .ok_or_else(|| budget("authenticated object size overflow"))?;
    if len > max_bytes || payload.len() > u64::MAX as usize {
        return Err(budget("authenticated object exceeds limit"));
    }
    let mut raw = Vec::new();
    raw.try_reserve_exact(len)
        .map_err(|_| budget("authenticated object allocation failed"))?;
    raw.extend_from_slice(OBJECT_MAGIC);
    raw.extend_from_slice(&1u16.to_le_bytes());
    raw.extend_from_slice(&0u16.to_le_bytes());
    raw.extend_from_slice(&store_id);
    raw.extend_from_slice(domain_digest.as_bytes());
    raw.extend_from_slice(&(kind.len() as u16).to_le_bytes());
    raw.extend_from_slice(kind);
    raw.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    raw.extend_from_slice(payload);
    Ok(raw)
}

fn decode_authenticated_object(
    raw: &[u8],
    expected_store: [u8; 16],
    expected_domain: Digest256,
    expected_kind: &[u8],
) -> Result<Vec<u8>> {
    let mut reader = Reader::new(raw);
    if reader.take(8)? != OBJECT_MAGIC || reader.u16()? != 1 || reader.u16()? != 0 {
        return Err(invalid("authenticated object header differs"));
    }
    if reader.array::<16>()? != expected_store
        || digest_from_raw(reader.take(32)?)? != expected_domain
    {
        return Err(SegmentError::new(
            Code::InvalidReceipt,
            "authenticated object store or domain differs",
        ));
    }
    let kind_len = reader.u16()? as usize;
    if kind_len == 0 || reader.take(kind_len)? != expected_kind {
        return Err(SegmentError::new(
            Code::InvalidReceipt,
            "authenticated object kind differs",
        ));
    }
    let payload_len = usize::try_from(reader.u64()?)
        .map_err(|_| budget("authenticated object payload exceeds address space"))?;
    let payload = reader.take(payload_len)?.to_vec();
    reader.finish()?;
    Ok(payload)
}

fn validate_kind(kind: &[u8], limits: AuthenticatedTreeLimitsV1) -> Result<()> {
    if kind.is_empty() || kind.len() > limits.max_kind_bytes || kind.len() > u16::MAX as usize {
        return Err(budget("authenticated tree kind exceeds limit"));
    }
    Ok(())
}

fn validate_key_value(
    key: &[u8],
    value: Option<&[u8]>,
    limits: AuthenticatedTreeLimitsV1,
) -> Result<()> {
    if key.is_empty()
        || key.len() > limits.max_key_bytes
        || key.len() > u32::MAX as usize
        || value.is_some_and(|value| {
            value.len() > limits.max_value_bytes || value.len() > u32::MAX as usize
        })
    {
        return Err(budget("authenticated key or value exceeds limit"));
    }
    Ok(())
}

fn key_starts_with_nibbles(key: &[u8], prefix: &[u8]) -> bool {
    prefix
        .iter()
        .enumerate()
        .all(|(index, nibble)| nibble_at(key, index) == Some(*nibble))
}

fn key_matches_prefix(key: &[u8], prefix_key: &[u8], nibbles: usize) -> bool {
    (0..nibbles).all(|index| nibble_at(key, index) == nibble_at(prefix_key, index))
}

fn nibble_at(key: &[u8], index: usize) -> Option<u8> {
    let byte = *key.get(index / 2)?;
    Some(if index.is_multiple_of(2) {
        byte >> 4
    } else {
        byte & 0x0f
    })
}

fn common_prefix_nibbles(left: &[u8], right: &[u8]) -> usize {
    let max = left.len().min(right.len()) * 2;
    (0..max)
        .take_while(|index| nibble_at(left, *index) == nibble_at(right, *index))
        .count()
}

fn remaining_bytes(
    work: AuthenticatedTreeWorkV1,
    limits: AuthenticatedTreeLimitsV1,
) -> Result<usize> {
    let remaining = limits
        .max_total_bytes
        .checked_sub(work.total_bytes())
        .ok_or_else(|| budget("authenticated tree byte budget exceeded"))?;
    usize::try_from(remaining)
        .map_err(|_| budget("authenticated tree byte budget exceeds address space"))
}

fn check_work(work: AuthenticatedTreeWorkV1, limits: AuthenticatedTreeLimitsV1) -> Result<()> {
    if work.total_nodes() > limits.max_nodes || work.total_bytes() > limits.max_total_bytes {
        eprintln!(
            "Authenticated tree work refused: nodes={}/{} bytes={}/{}",
            work.total_nodes(), limits.max_nodes, work.total_bytes(), limits.max_total_bytes,
        );
        return Err(budget("authenticated tree operation budget exceeded"));
    }
    Ok(())
}

struct Reader<'a> {
    raw: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn new(raw: &'a [u8]) -> Self {
        Self { raw, at: 0 }
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self
            .at
            .checked_add(len)
            .ok_or_else(|| invalid("authenticated wire offset overflow"))?;
        let bytes = self
            .raw
            .get(self.at..end)
            .ok_or_else(|| invalid("authenticated wire is truncated"))?;
        self.at = end;
        Ok(bytes)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        self.take(N)?
            .try_into()
            .map_err(|_| invalid("authenticated fixed field length differs"))
    }

    fn u8(&mut self) -> Result<u8> {
        Ok(self.array::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.array::<2>()?))
    }

    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.array::<4>()?))
    }

    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.array::<8>()?))
    }

    fn key(&mut self, max_bytes: usize) -> Result<Vec<u8>> {
        let len = self.u32()? as usize;
        if len == 0 || len > max_bytes {
            return Err(invalid("authenticated key length differs"));
        }
        Ok(self.take(len)?.to_vec())
    }

    fn value(&mut self, max_bytes: usize) -> Result<Vec<u8>> {
        let len = self.u32()? as usize;
        if len > max_bytes {
            return Err(budget("authenticated value exceeds limit"));
        }
        Ok(self.take(len)?.to_vec())
    }

    fn finish(&self) -> Result<()> {
        if self.at == self.raw.len() {
            Ok(())
        } else {
            Err(invalid("authenticated wire has trailing bytes"))
        }
    }
}

fn digest_from_raw(bytes: &[u8]) -> Result<Digest256> {
    if bytes.len() != 32 {
        return Err(invalid("authenticated digest length differs"));
    }
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut text = [0u8; 64];
    for (index, byte) in bytes.iter().copied().enumerate() {
        text[2 * index] = HEX[(byte >> 4) as usize];
        text[2 * index + 1] = HEX[(byte & 0x0f) as usize];
    }
    Digest256::from_hex(std::str::from_utf8(&text).expect("hex is ASCII"))
        .map_err(|_| invalid("authenticated digest encoding differs"))
}

fn invalid(detail: &'static str) -> SegmentError {
    SegmentError::new(Code::InvalidFormat, detail)
}

fn budget(detail: &'static str) -> SegmentError {
    SegmentError::new(Code::BudgetExceeded, detail)
}

/// Encode the existing exact V1 placement generation row for a generic tree
/// value. The row's logical key is repeated in this value for a self-contained
/// canonical wire form; CMD may use the same bytes as the tree key.
pub fn encode_placement_tree_row(
    row: &PlacementGenerationRowV1,
    max_bytes: usize,
) -> Result<Vec<u8>> {
    if row.key.is_empty()
        || row.key.len() > u32::MAX as usize
        || row.logical_length != row.placement.coordinate().size_bytes
        || row.logical_digest != row.placement.coordinate().sha256
    {
        return Err(invalid("placement tree row fields differ"));
    }
    let len = 8usize
        .checked_add(2 + 2 + 4 + row.key.len() + 32 + 8 + PlacementV1::ENCODED_BYTES)
        .ok_or_else(|| budget("placement tree row size overflow"))?;
    if len > max_bytes {
        return Err(budget("placement tree row exceeds limit"));
    }
    let mut raw = Vec::new();
    raw.try_reserve_exact(len)
        .map_err(|_| budget("placement tree row allocation failed"))?;
    raw.extend_from_slice(PLACEMENT_ROW_MAGIC);
    raw.extend_from_slice(&1u16.to_le_bytes());
    raw.extend_from_slice(&0u16.to_le_bytes());
    raw.extend_from_slice(&(row.key.len() as u32).to_le_bytes());
    raw.extend_from_slice(&row.key);
    raw.extend_from_slice(row.logical_digest.as_bytes());
    raw.extend_from_slice(&row.logical_length.to_le_bytes());
    raw.extend_from_slice(&row.placement.encode());
    Ok(raw)
}

pub fn decode_placement_tree_row(raw: &[u8], max_bytes: usize) -> Result<PlacementGenerationRowV1> {
    if raw.len() > max_bytes {
        return Err(budget("placement tree row exceeds limit"));
    }
    let mut reader = Reader::new(raw);
    if reader.take(8)? != PLACEMENT_ROW_MAGIC || reader.u16()? != 1 || reader.u16()? != 0 {
        return Err(invalid("placement tree row header differs"));
    }
    let key = reader.key(max_bytes)?;
    let logical_digest = digest_from_raw(reader.take(32)?)?;
    let logical_length = reader.u64()?;
    let placement = PlacementV1::decode(reader.take(PlacementV1::ENCODED_BYTES)?)?;
    reader.finish()?;
    if logical_length != placement.coordinate().size_bytes
        || logical_digest != placement.coordinate().sha256
    {
        return Err(invalid("placement tree row logical bytes differ"));
    }
    Ok(PlacementGenerationRowV1 {
        key,
        logical_digest,
        logical_length,
        placement,
    })
}

/// Return the first authenticated row inside an optional half-open key range
/// that is strictly after `after_exclusive`. The root's authenticated min/max
/// summaries let this seek skip every subtree that cannot contain a result;
/// callers must not implement `*_after` by restarting a full stream.
///
/// `max_state_bytes` is the caller's already-reserved transient allowance for
/// this seek, including its bounded stack, decoded nodes, one in-flight frame,
/// and the returned row. The function enforces that ceiling as nodes are
/// loaded. It does not reserve caller state or establish a source grant.
impl SegmentStore {
    pub fn lookup_authenticated_tree_v2_after_with_work_and_io(
        &self,
        descriptor: &AuthenticatedTreeDescriptorV2,
        lower_inclusive: Option<&[u8]>,
        upper_exclusive: Option<&[u8]>,
        after_exclusive: Option<&[u8]>,
        limits: AuthenticatedTreeLimitsV1,
        max_state_bytes: usize,
        io_ledger: Option<Arc<dyn AuthenticatedTreeIoLedgerV1>>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(Option<AuthenticatedTreeEntryV1>, AuthenticatedTreeWorkV1)> {
        self.lookup_authenticated_tree_v2_after_with_work_and_io_inner(
            descriptor,
            lower_inclusive,
            upper_exclusive,
            after_exclusive,
            limits,
            max_state_bytes,
            io_ledger,
            deadline,
            cancelled,
            None,
        )
    }

    /// Strict-after keyset lookup that charges the caller's existing
    /// cumulative work callback immediately before every authenticated node
    /// visit. The callback remains a call-scoped mutable reference and is not
    /// wrapped in the Send+Sync IO ledger.
    pub fn lookup_authenticated_tree_v2_after_with_work_and_io_and_callback(
        &self,
        descriptor: &AuthenticatedTreeDescriptorV2,
        lower_inclusive: Option<&[u8]>,
        upper_exclusive: Option<&[u8]>,
        after_exclusive: Option<&[u8]>,
        limits: AuthenticatedTreeLimitsV1,
        max_state_bytes: usize,
        io_ledger: Option<Arc<dyn AuthenticatedTreeIoLedgerV1>>,
        deadline: Instant,
        cancelled: &AtomicBool,
        callback: &mut dyn FnMut() -> bool,
    ) -> Result<(Option<AuthenticatedTreeEntryV1>, AuthenticatedTreeWorkV1)> {
        self.lookup_authenticated_tree_v2_after_with_work_and_io_inner(
            descriptor,
            lower_inclusive,
            upper_exclusive,
            after_exclusive,
            limits,
            max_state_bytes,
            io_ledger,
            deadline,
            cancelled,
            Some(callback),
        )
    }

    fn lookup_authenticated_tree_v2_after_with_work_and_io_inner(
        &self,
        descriptor: &AuthenticatedTreeDescriptorV2,
        lower_inclusive: Option<&[u8]>,
        upper_exclusive: Option<&[u8]>,
        after_exclusive: Option<&[u8]>,
        limits: AuthenticatedTreeLimitsV1,
        max_state_bytes: usize,
        io_ledger: Option<Arc<dyn AuthenticatedTreeIoLedgerV1>>,
        deadline: Instant,
        cancelled: &AtomicBool,
        mut shared_work: Option<&mut dyn FnMut() -> bool>,
    ) -> Result<(Option<AuthenticatedTreeEntryV1>, AuthenticatedTreeWorkV1)> {
        let limits = limits.validate()?;
        validate_descriptor_for_store(self, &descriptor.semantic, limits)?;
        validate_descriptor_v2_shape(descriptor)?;
        if max_state_bytes == 0 || max_state_bytes == usize::MAX {
            return Err(budget("authenticated range state allowance is invalid"));
        }
        for bound in [lower_inclusive, upper_exclusive, after_exclusive]
            .into_iter()
            .flatten()
        {
            if bound.len() > limits.max_key_bytes {
                return Err(budget("authenticated range key exceeds limit"));
            }
        }
        if lower_inclusive
            .zip(upper_exclusive)
            .is_some_and(|(lower, upper)| lower >= upper)
        {
            return Err(invalid("authenticated range bounds are reversed"));
        }
        if after_exclusive
            .zip(upper_exclusive)
            .is_some_and(|(after, upper)| after >= upper)
        {
            return Ok((None, AuthenticatedTreeWorkV1::default()));
        }
        check(deadline, cancelled)?;
        let _pin_lock = self.hold_generation_pin()?;
        let Some(root_reference) = descriptor.semantic.root.as_ref() else {
            return Ok((None, AuthenticatedTreeWorkV1::default()));
        };

        // A Patricia path cannot exceed two edges per key byte plus its root.
        // Reserve that finite stack before decoding any node, then enforce the
        // actual retained decoded-node and transient read overlap below.
        let max_depth = limits
            .max_key_bytes
            .checked_mul(2)
            .and_then(|depth| depth.checked_add(1))
            .ok_or_else(|| budget("authenticated range depth overflow"))?;
        let stack_floor = max_depth
            .checked_mul(std::mem::size_of::<RangeSearchFrameV2>())
            .ok_or_else(|| budget("authenticated range stack size overflow"))?;
        let transient_read = limits
            .max_node_bytes
            .checked_mul(4)
            .and_then(|n| n.checked_add(4096 + 65_536))
            .ok_or_else(|| budget("authenticated range node state overflow"))?;
        let root_handle_state = std::mem::size_of::<TreeHandleV2>()
            .checked_add(root_reference.min_key.len())
            .and_then(|n| n.checked_add(root_reference.max_key.len()))
            .and_then(|n| n.checked_add(256))
            .ok_or_else(|| budget("authenticated range root state overflow"))?;
        if stack_floor
            .checked_add(transient_read)
            .and_then(|n| n.checked_add(root_handle_state))
            .is_none_or(|required| required > max_state_bytes)
        {
            return Err(budget("authenticated range preflight state exceeded"));
        }
        let mut stack = Vec::new();
        stack
            .try_reserve_exact(max_depth)
            .map_err(|_| budget("authenticated range stack allocation failed"))?;
        let stack_storage = stack
            .capacity()
            .checked_mul(std::mem::size_of::<RangeSearchFrameV2>())
            .ok_or_else(|| budget("authenticated range stack capacity overflow"))?;
        if stack_storage > stack_floor.saturating_add(65_536)
            || stack_storage
                .checked_add(transient_read)
                .and_then(|n| n.checked_add(root_handle_state))
                .is_none_or(|required| required > max_state_bytes)
        {
            return Err(budget("authenticated range stack exceeds state allowance"));
        }

        let mut work = AuthenticatedTreeWorkV1::default();
        let root = descriptor_root_handle(descriptor)?
            .ok_or_else(|| invalid("authenticated range root disappeared"))?;
        let loaded = load_node_v2(
            self,
            descriptor,
            &root,
            limits,
            &mut work,
            None,
            io_ledger.as_deref(),
            deadline,
            cancelled,
            &mut shared_work,
        )?;
        drop(root);
        let root_state = loaded_node_retained_state(&loaded)?;
        if stack_storage
            .checked_add(root_state)
            .is_none_or(|state| state > max_state_bytes)
        {
            return Err(budget(
                "authenticated range decoded root exceeds state allowance",
            ));
        }
        stack.push(RangeSearchFrameV2 {
            loaded,
            next_child: 0,
            value_examined: false,
            retained_state_bytes: root_state,
        });
        let mut retained_nodes = root_state;

        while !stack.is_empty() {
            check(deadline, cancelled)?;
            let last = stack.len() - 1;
            if !stack[last].value_examined {
                stack[last].value_examined = true;
                if let Some(row) = stack[last].loaded.node.value.take() {
                    if authenticated_range_contains(
                        &row.key,
                        lower_inclusive,
                        upper_exclusive,
                        after_exclusive,
                    ) {
                        // The row moves out of the retained node. Its bytes
                        // were included in loaded_node_retained_state before
                        // this return, so caller ownership remains covered.
                        return Ok((Some(row), work));
                    }
                }
            }

            let child = {
                let frame = &mut stack[last];
                if frame.next_child >= frame.loaded.node.children.len() {
                    None
                } else {
                    let child_index = frame.next_child;
                    frame.next_child += 1;
                    let reference = &frame.loaded.node.children[child_index].1;
                    let locator = frame
                        .loaded
                        .child_locators
                        .get(child_index)
                        .ok_or_else(|| invalid("authenticated range child locator is missing"))?;
                    if !authenticated_range_overlaps(
                        reference,
                        lower_inclusive,
                        upper_exclusive,
                        after_exclusive,
                    ) {
                        continue;
                    }
                    let handle_state = std::mem::size_of::<TreeHandleV2>()
                        .checked_add(reference.min_key.len())
                        .and_then(|n| n.checked_add(reference.max_key.len()))
                        .and_then(|n| n.checked_add(std::mem::size_of::<TreeLocatorV2>()))
                        .and_then(|n| n.checked_add(256))
                        .ok_or_else(|| budget("authenticated range child state overflow"))?;
                    if stack_storage
                        .checked_add(retained_nodes)
                        .and_then(|n| n.checked_add(handle_state))
                        .and_then(|n| n.checked_add(transient_read))
                        .is_none_or(|state| state > max_state_bytes)
                    {
                        return Err(budget("authenticated range child preflight exceeded"));
                    }
                    Some(TreeHandleV2 {
                        reference: reference.clone(),
                        locator: locator.clone(),
                    })
                }
            };
            let Some(child) = child else {
                let removed = stack
                    .pop()
                    .ok_or_else(|| invalid("authenticated range stack disappeared"))?;
                retained_nodes = retained_nodes
                    .checked_sub(removed.retained_state_bytes)
                    .ok_or_else(|| budget("authenticated range retained state regressed"))?;
                continue;
            };
            let loaded = load_node_v2(
                self,
                descriptor,
                &child,
                limits,
                &mut work,
                None,
                io_ledger.as_deref(),
                deadline,
                cancelled,
                &mut shared_work,
            )?;
            drop(child);
            let retained_state_bytes = loaded_node_retained_state(&loaded)?;
            let next_retained = retained_nodes
                .checked_add(retained_state_bytes)
                .ok_or_else(|| budget("authenticated range retained state overflow"))?;
            if stack_storage
                .checked_add(next_retained)
                .is_none_or(|state| state > max_state_bytes)
            {
                return Err(budget(
                    "authenticated range retained nodes exceed allowance",
                ));
            }
            stack.push(RangeSearchFrameV2 {
                loaded,
                next_child: 0,
                value_examined: false,
                retained_state_bytes,
            });
            retained_nodes = next_retained;
        }
        Ok((None, work))
    }
}

struct RangeSearchFrameV2 {
    loaded: LoadedTreeNodeV2,
    next_child: usize,
    value_examined: bool,
    retained_state_bytes: usize,
}

fn authenticated_range_contains(
    key: &[u8],
    lower_inclusive: Option<&[u8]>,
    upper_exclusive: Option<&[u8]>,
    after_exclusive: Option<&[u8]>,
) -> bool {
    lower_inclusive.is_none_or(|lower| key >= lower)
        && upper_exclusive.is_none_or(|upper| key < upper)
        && after_exclusive.is_none_or(|after| key > after)
}

fn authenticated_range_overlaps(
    node: &AuthenticatedTreeNodeRefV1,
    lower_inclusive: Option<&[u8]>,
    upper_exclusive: Option<&[u8]>,
    after_exclusive: Option<&[u8]>,
) -> bool {
    lower_inclusive.is_none_or(|lower| node.max_key.as_slice() >= lower)
        && upper_exclusive.is_none_or(|upper| node.min_key.as_slice() < upper)
        && after_exclusive.is_none_or(|after| node.max_key.as_slice() > after)
}

fn loaded_node_retained_state(loaded: &LoadedTreeNodeV2) -> Result<usize> {
    let node = &loaded.node;
    let mut state = std::mem::size_of::<LoadedTreeNodeV2>()
        .checked_add(node.min_key.capacity())
        .and_then(|n| n.checked_add(node.max_key.capacity()))
        .and_then(|n| {
            n.checked_add(
                node.children
                    .capacity()
                    .checked_mul(std::mem::size_of::<(u8, AuthenticatedTreeNodeRefV1)>())?,
            )
        })
        .and_then(|n| {
            n.checked_add(
                loaded
                    .child_locators
                    .capacity()
                    .checked_mul(std::mem::size_of::<TreeLocatorV2>())?,
            )
        })
        .ok_or_else(|| budget("authenticated loaded-node state overflow"))?;
    if let Some(value) = &node.value {
        state = state
            .checked_add(value.key.capacity())
            .and_then(|n| n.checked_add(value.value.capacity()))
            .ok_or_else(|| budget("authenticated loaded-value state overflow"))?;
    }
    for (_, child) in &node.children {
        state = state
            .checked_add(child.min_key.capacity())
            .and_then(|n| n.checked_add(child.max_key.capacity()))
            .and_then(|n| n.checked_add(256))
            .ok_or_else(|| budget("authenticated child-reference state overflow"))?;
    }
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::SegmentLimits;
    use std::fs::{self, OpenOptions};
    use std::io::{Read, Seek, SeekFrom, Write};
    use std::os::unix::fs::DirBuilderExt;
    use std::path::PathBuf;
    use std::time::Duration;

    struct PrivateRoot(PathBuf);

    impl PrivateRoot {
        fn new() -> Self {
            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("test clock")
                .as_nanos();
            let path = std::env::temp_dir()
                .join(format!("tos-auth-tree-test-{}-{nonce}", std::process::id()));
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

    fn store_limits() -> SegmentLimits {
        SegmentLimits {
            max_segment_bytes: 1024 * 1024,
            max_frame_bytes: 256 * 1024,
            max_frames: 16,
            max_journal_bytes: 64 * 1024,
        }
    }

    fn tree_limits() -> AuthenticatedTreeLimitsV1 {
        AuthenticatedTreeLimitsV1 {
            max_key_bytes: 256,
            max_value_bytes: 1024,
            max_kind_bytes: 128,
            max_node_bytes: 16 * 1024,
            max_children: 16,
            max_nodes: 1024,
            max_total_bytes: 4 * 1024 * 1024,
            max_rows: 128,
        }
    }

    fn assert_cold_matches_delta(
        store: &SegmentStore,
        kind: &[u8],
        rows: Vec<AuthenticatedTreeEntryV1>,
        limits: AuthenticatedTreeLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) {
        let (cold, cold_work) = store
            .build_authenticated_tree_v1_with_work(
                kind,
                rows.clone().into_iter().map(Ok),
                limits,
                deadline,
                cancelled,
            )
            .expect("cold ordered build");
        let node_bound = 2 * rows.len() as u64 - 1;
        assert!(cold_work.written_nodes <= node_bound);
        assert!(cold_work.read_nodes <= node_bound);
        let empty = store
            .build_authenticated_tree_v1(kind, std::iter::empty(), limits, deadline, cancelled)
            .expect("empty build");
        let inserted = store
            .apply_authenticated_tree_delta_v1(
                &empty,
                rows.iter().map(|row| {
                    Ok(AuthenticatedTreeDeltaV1 {
                        key: row.key.clone(),
                        value: Some(row.value.clone()),
                    })
                }),
                limits,
                deadline,
                cancelled,
            )
            .expect("ordered COW insertion");
        assert_eq!(cold.root, inserted.root);
        assert_eq!(cold.commitment, inserted.commitment);
        assert_eq!(cold.entries, rows.len() as u64);
        for row in &rows {
            assert_eq!(
                store
                    .lookup_authenticated_tree_v1(&cold, &row.key, limits, deadline, cancelled,)
                    .expect("exact cold lookup"),
                Some(row.value.clone())
            );
        }
    }

    #[test]
    fn immutable_tree_delta_keeps_history_and_rejects_a_corrupt_lookup_path() {
        let root = PrivateRoot::new();
        let store = SegmentStore::initialize_empty(&root.0, b"private-tree-domain", store_limits())
            .expect("initialize store");
        let deadline = Instant::now() + Duration::from_secs(10);
        let cancelled = AtomicBool::new(false);
        let old = store
            .build_authenticated_tree_v1(
                b"test/history",
                [
                    AuthenticatedTreeEntryV1 {
                        key: b"a".to_vec(),
                        value: b"one".to_vec(),
                    },
                    AuthenticatedTreeEntryV1 {
                        key: b"aa".to_vec(),
                        value: b"two".to_vec(),
                    },
                    AuthenticatedTreeEntryV1 {
                        key: b"b".to_vec(),
                        value: b"three".to_vec(),
                    },
                ]
                .into_iter()
                .map(Ok),
                tree_limits(),
                deadline,
                &cancelled,
            )
            .expect("build old tree");
        let current = store
            .apply_authenticated_tree_delta_v1(
                &old,
                [
                    AuthenticatedTreeDeltaV1 {
                        key: b"a".to_vec(),
                        value: Some(b"ONE".to_vec()),
                    },
                    AuthenticatedTreeDeltaV1 {
                        key: b"aa".to_vec(),
                        value: None,
                    },
                    AuthenticatedTreeDeltaV1 {
                        key: b"c".to_vec(),
                        value: Some(b"four".to_vec()),
                    },
                ]
                .into_iter()
                .map(Ok),
                tree_limits(),
                deadline,
                &cancelled,
            )
            .expect("apply delta");

        assert_eq!(
            store
                .lookup_authenticated_tree_v1(&old, b"a", tree_limits(), deadline, &cancelled)
                .expect("read history"),
            Some(b"one".to_vec())
        );
        assert_eq!(
            store
                .lookup_authenticated_tree_v1(&current, b"a", tree_limits(), deadline, &cancelled,)
                .expect("read current"),
            Some(b"ONE".to_vec())
        );
        assert_eq!(
            store
                .lookup_authenticated_tree_v1(&current, b"aa", tree_limits(), deadline, &cancelled,)
                .expect("read deletion"),
            None
        );
        assert_eq!(
            store
                .lookup_authenticated_tree_v1(
                    &current,
                    b"missing",
                    tree_limits(),
                    deadline,
                    &cancelled,
                )
                .expect("read absence"),
            None
        );
        assert_eq!(
            store
                .verify_authenticated_tree_v1(&current, tree_limits(), deadline, &cancelled)
                .expect("cold verify")
                .entries,
            3
        );

        let root_ref = current.root.as_ref().expect("nonempty current root");
        let path = root.0.join("generations").join(root_ref.digest.to_hex());
        let mut object = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .expect("open root node for corruption probe");
        let mut first = [0u8; 1];
        object.read_exact(&mut first).expect("read root node");
        object.seek(SeekFrom::Start(0)).expect("rewind root node");
        object
            .write_all(&[first[0] ^ 0x01])
            .expect("corrupt root node");
        object.sync_all().expect("sync corrupted root node");
        assert!(
            store
                .lookup_authenticated_tree_v1(&current, b"a", tree_limits(), deadline, &cancelled,)
                .is_err()
        );
    }

    #[test]
    fn cold_builder_handles_retreating_lcp_and_long_prefixes() {
        let root = PrivateRoot::new();
        let store = SegmentStore::initialize_empty(&root.0, b"private-tree-domain", store_limits())
            .expect("initialize store");
        let deadline = Instant::now() + Duration::from_secs(30);
        let cancelled = AtomicBool::new(false);
        let limits = tree_limits();

        // The first two keys diverge later than the third: [aa, ab] share a
        // longer prefix than [aa, b]. The online builder must retreat its
        // open prefix stack when it sees `b`.
        assert_cold_matches_delta(
            &store,
            b"test/lcp-retreat",
            [b"aa".as_slice(), b"ab", b"b"]
                .into_iter()
                .enumerate()
                .map(|(index, key)| AuthenticatedTreeEntryV1 {
                    key: key.to_vec(),
                    value: format!("value-{index}").into_bytes(),
                })
                .collect(),
            limits,
            deadline,
            &cancelled,
        );

        // A terminal row may itself be a prefix of later rows while the
        // enclosing branch subsequently retreats to an earlier LCP.
        assert_cold_matches_delta(
            &store,
            b"test/lcp-retreat-terminal",
            [b"a".as_slice(), b"aa", b"ab", b"b"]
                .into_iter()
                .enumerate()
                .map(|(index, key)| AuthenticatedTreeEntryV1 {
                    key: key.to_vec(),
                    value: format!("value-{index}").into_bytes(),
                })
                .collect(),
            limits,
            deadline,
            &cancelled,
        );

        // A near-maximum common path stays iterative and compresses to the
        // same canonical root as COW insertion under the same finite limits.
        let mut long_limits = limits;
        long_limits.max_key_bytes = MAX_KEY_BYTES;
        long_limits.max_node_bytes = 128 * 1024;
        long_limits.max_total_bytes = 8 * 1024 * 1024;
        let mut first = vec![b'x'; MAX_KEY_BYTES];
        *first.last_mut().expect("nonempty key") = b'a';
        let mut second = first.clone();
        *second.last_mut().expect("nonempty key") = b'b';
        assert_cold_matches_delta(
            &store,
            b"test/long-prefix",
            vec![
                AuthenticatedTreeEntryV1 {
                    key: first,
                    value: b"left".to_vec(),
                },
                AuthenticatedTreeEntryV1 {
                    key: second,
                    value: b"right".to_vec(),
                },
            ],
            long_limits,
            deadline,
            &cancelled,
        );
    }

    #[test]
    fn packed_v2_preserves_semantics_history_and_rejects_bad_frames() {
        let root = PrivateRoot::new();
        let store = SegmentStore::initialize_empty(&root.0, b"private-tree-domain", store_limits())
            .expect("initialize store");
        let deadline = Instant::now() + Duration::from_secs(20);
        let cancelled = AtomicBool::new(false);
        let limits = tree_limits();
        let rows = [b"aa".as_slice(), b"ab", b"b", b"c"]
            .into_iter()
            .enumerate()
            .map(|(index, key)| AuthenticatedTreeEntryV1 {
                key: key.to_vec(),
                value: format!("value-{index}").into_bytes(),
            })
            .collect::<Vec<_>>();
        let legacy = store
            .build_authenticated_tree_v1(
                b"test/packed-v2",
                rows.clone().into_iter().map(Ok),
                limits,
                deadline,
                &cancelled,
            )
            .expect("legacy build");
        let (old, build_work) = store
            .build_authenticated_tree_v2_with_pack_cap(
                b"test/packed-v2",
                rows.clone().into_iter().map(Ok),
                limits,
                600,
                None,
                None,
                None,
                deadline,
                &cancelled,
            )
            .expect("packed build");
        assert_eq!(old.commitment, legacy.commitment);
        assert_eq!(old.entries, legacy.entries);
        assert!(old.physical_root.is_some());
        assert!(build_work.written_bytes > 0);
        let old_wire = legacy.encode(16 * 1024).expect("legacy wire");
        let legacy_wrapper = AuthenticatedTreeDescriptorV2::from_legacy(legacy.clone());
        assert_eq!(
            legacy_wrapper
                .encode(16 * 1024)
                .expect("legacy wrapper wire"),
            old_wire
        );
        let (legacy_noop, _) = store
            .apply_authenticated_tree_delta_v2_with_pack_cap(
                &legacy_wrapper,
                std::iter::empty::<Result<AuthenticatedTreeDeltaV1>>(),
                limits,
                600,
                None,
                deadline,
                &cancelled,
            )
            .expect("legacy wrapped no-op update");
        assert!(legacy_noop.physical_root.is_none());
        assert_eq!(
            legacy_noop.encode(16 * 1024).expect("legacy no-op wire"),
            old_wire
        );
        assert_eq!(
            AuthenticatedTreeDescriptorV2::decode(&old_wire, 16 * 1024)
                .expect("legacy-compatible decode")
                .encode(16 * 1024)
                .expect("legacy-compatible re-encode"),
            old_wire
        );
        let packed_wire = old.encode(16 * 1024).expect("packed wire");
        let decoded =
            AuthenticatedTreeDescriptorV2::decode(&packed_wire, 16 * 1024).expect("packed decode");
        assert_eq!(decoded, old);

        let mut stream = store
            .stream_authenticated_tree_v2(&old, limits)
            .expect("open packed stream");
        let mut observed = Vec::new();
        while let Some(row) = stream
            .next_row(deadline, &cancelled)
            .expect("read packed row")
        {
            observed.push(row);
        }
        assert_eq!(observed, rows);
        assert_eq!(stream.coverage().expect("EOF coverage").entries, 4);
        let coverage = store
            .verify_authenticated_tree_v2(&old, limits, deadline, &cancelled)
            .expect("cold packed verification");
        assert_eq!(coverage.entries, old.entries);

        let changes = [
            AuthenticatedTreeDeltaV1 {
                key: b"aa".to_vec(),
                value: Some(b"changed".to_vec()),
            },
            AuthenticatedTreeDeltaV1 {
                key: b"ab".to_vec(),
                value: Some(b"changed-ab".to_vec()),
            },
        ];
        let (current, update_work) = store
            .apply_authenticated_tree_delta_v2_with_pack_cap(
                &old,
                changes.clone().into_iter().map(Ok),
                limits,
                600,
                None,
                deadline,
                &cancelled,
            )
            .expect("packed COW update");
        assert!(update_work.read_nodes > 0);
        assert!(update_work.written_nodes > 0);
        let expected = store
            .apply_authenticated_tree_delta_v1(
                &legacy,
                changes.into_iter().map(Ok),
                limits,
                deadline,
                &cancelled,
            )
            .expect("legacy semantic reference update");
        assert_eq!(current.commitment, expected.commitment);
        assert_eq!(current.entries, expected.entries);
        assert_ne!(
            current.physical_root.as_ref().map(|root| root.pack_digest),
            old.physical_root.as_ref().map(|root| root.pack_digest)
        );
        assert_eq!(
            store
                .lookup_authenticated_tree_v2(&old, b"aa", limits, deadline, &cancelled)
                .expect("historical packed lookup"),
            Some(b"value-0".to_vec())
        );
        assert_eq!(
            store
                .lookup_authenticated_tree_v2(&current, b"aa", limits, deadline, &cancelled)
                .expect("current packed lookup"),
            Some(b"changed".to_vec())
        );
        assert_eq!(
            store
                .lookup_authenticated_tree_v2(&current, b"ab", limits, deadline, &cancelled)
                .expect("second same-call packed lookup"),
            Some(b"changed-ab".to_vec())
        );
        let current_root = descriptor_root_handle(&current)
            .expect("current root handle")
            .expect("current root exists");
        let loaded = load_node_v2(
            &store,
            &current,
            &current_root,
            limits,
            &mut AuthenticatedTreeWorkV1::default(),
            None,
            None,
            deadline,
            &cancelled,
            &mut None,
        )
        .expect("load current root locator sidecar");
        let old_root = descriptor_root_handle(&old)
            .expect("old root handle")
            .expect("old root exists");
        let old_loaded = load_node_v2(
            &store,
            &old,
            &old_root,
            limits,
            &mut AuthenticatedTreeWorkV1::default(),
            None,
            None,
            deadline,
            &cancelled,
            &mut None,
        )
        .expect("load old root locator sidecar");
        let current_pack = current
            .physical_root
            .as_ref()
            .expect("current packed root")
            .pack_digest;
        let unchanged_child_reused =
            loaded
                .node
                .children
                .iter()
                .enumerate()
                .any(|(current_index, (edge, reference))| {
                    let Some(old_index) =
                        old_loaded
                            .node
                            .children
                            .iter()
                            .position(|(old_edge, old_reference)| {
                                old_edge == edge && old_reference == reference
                            })
                    else {
                        return false;
                    };
                    let (Some(current_locator), Some(old_locator)) = (
                        loaded.child_locators.get(current_index),
                        old_loaded.child_locators.get(old_index),
                    ) else {
                        return false;
                    };
                    matches!(
                        (public_locator(current_locator), public_locator(old_locator)),
                        (Ok(current), Ok(old)) if current == old
                    )
                });
        assert!(
            unchanged_child_reused,
            "an untouched semantic child keeps its exact locator"
        );
        let changed_child_in_intermediate_pack =
            loaded
                .node
                .children
                .iter()
                .enumerate()
                .any(|(current_index, (edge, reference))| {
                    let Some(old_index) = old_loaded
                        .node
                        .children
                        .iter()
                        .position(|(old_edge, _)| old_edge == edge)
                    else {
                        return false;
                    };
                    if old_loaded.node.children[old_index].1 == *reference {
                        return false;
                    }
                    let (Some(current_locator), Some(old_locator)) = (
                        loaded.child_locators.get(current_index),
                        old_loaded.child_locators.get(old_index),
                    ) else {
                        return false;
                    };
                    matches!(
                        (public_locator(current_locator), public_locator(old_locator)),
                        (Ok(current), Ok(old))
                            if current.pack_digest != old.pack_digest
                                && current.pack_digest != current_pack
                    )
                });
        assert!(
            changed_child_in_intermediate_pack,
            "the test cap flushes a changed child separately from the final root"
        );

        let mut wrong_sha = current.clone();
        wrong_sha
            .physical_root
            .as_mut()
            .expect("current packed root")
            .frame_sha256 = Digest256::of_bytes(b"wrong frame");
        assert!(
            store
                .lookup_authenticated_tree_v2(&wrong_sha, b"aa", limits, deadline, &cancelled)
                .is_err()
        );
        let mut wrong_offset = current.clone();
        wrong_offset
            .physical_root
            .as_mut()
            .expect("current packed root")
            .offset += 1;
        assert!(
            store
                .lookup_authenticated_tree_v2(&wrong_offset, b"aa", limits, deadline, &cancelled)
                .is_err()
        );

        let restored = SegmentStore::open_existing(&root.0, store_limits()).expect("reopen store");
        assert_eq!(
            restored
                .lookup_authenticated_tree_v2(&current, b"c", limits, deadline, &cancelled,)
                .expect("restored untouched row"),
            Some(b"value-3".to_vec())
        );
        assert_eq!(
            restored
                .verify_authenticated_tree_v2(&current, limits, deadline, &cancelled)
                .expect("restored packed closure")
                .entries,
            current.entries
        );
    }
}
