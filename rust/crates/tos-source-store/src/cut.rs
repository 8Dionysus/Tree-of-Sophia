//! Complete traversal of the existing immutable source-corpus carrier.
//!
//! Manifest membership and index claims are authenticated by the exact revision
//! digest, not by a new source registry. This reader grants no source admission,
//! current rights, semantic assessment, or authority to publish retained bytes.

use std::collections::BTreeSet;
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use tos_foundation::{Digest256, Digest256Hasher, RelativePath, SourceRevision};

use crate::{CorpusReader, Result, CorpusDescriptor, Snapshot, StoreError, StoreErrorCode};

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceMembershipV1 {
    pub count: u64,
    pub digest: Digest256,
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
        if limits.max_revisions == 0 || limits.max_revisions == usize::MAX
            || limits.max_members == 0 || limits.max_members == u64::MAX
            || limits.max_total_bytes == 0 || limits.max_total_bytes == u64::MAX
            || limits.max_member_bytes == 0 || limits.max_member_bytes == u64::MAX
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
                if !is_source_member(&member.path.as_str().to_owned()) {
                    return Err(StoreError::new(StoreErrorCode::InvalidMemberIndex,
                        "member is outside the source admission carrier"));
                }
                members = members.checked_add(1).ok_or_else(|| refusal("source member count overflow"))?;
                bytes = bytes.checked_add(member.size_bytes).ok_or_else(|| refusal("source byte count overflow"))?;
                if members > limits.max_members || bytes > limits.max_total_bytes
                    || member.size_bytes > limits.max_member_bytes
                {
                    return Err(refusal("source cut exceeds declared budget"));
                }
            }
            next = snapshot.base_revision();
            snapshots.push(snapshot);
        }
        check_time(deadline, cancelled)?;
        Ok(CorpusCutReader { reader: self.clone(), snapshots, limits })
    }
}

impl CorpusCutReader {
    pub fn current(&self) -> &Snapshot { &self.snapshots[0] }
    /// Current is index zero. Retained revisions keep original paths and
    /// identities; they never contribute current ID ownership by inference.
    pub fn revisions(&self) -> impl Iterator<Item = &Snapshot> { self.snapshots.iter() }
    /// Random exact companion lookup under the same anchored cut. The rule
    /// executor owns its aggregate lookup budget; every individual read remains
    /// capped and private until complete digest verification.
    pub fn read_member(&self, revision: SourceRevision, path: &RelativePath,
        max_bytes: u64, deadline: Instant, cancelled: &AtomicBool) -> Result<SourceMemberV1> {
        check_time(deadline, cancelled)?;
        let snapshot = self.snapshots.iter().find(|s| s.revision() == revision)
            .ok_or_else(|| StoreError::new(StoreErrorCode::MissingRevision, "revision is outside opened source cut"))?;
        let metadata = snapshot.member(path)
            .ok_or_else(|| StoreError::new(StoreErrorCode::MissingMember, "member is outside exact source revision"))?;
        let descriptor = CorpusDescriptor { revision, path: path.clone(), sha256: metadata.sha256,
            size_bytes: metadata.size_bytes, mode: metadata.mode };
        let mut stage = TimedStage { raw: Vec::new(), deadline, cancelled };
        self.reader.read_selected(snapshot, &descriptor, max_bytes.min(self.limits.max_member_bytes), &mut stage)?;
        check_time(deadline, cancelled)?;
        Ok(SourceMemberV1 { path: path.clone(), raw: stage.raw, revision,
            stable_ids: snapshot.ids_for_path(path).map(str::to_owned).collect() })
    }
    pub fn presence(&self, revision: SourceRevision, path: &RelativePath) -> Option<SourcePresenceV1> {
        let snapshot = self.snapshots.iter().find(|s| s.revision() == revision)?;
        if snapshot.member(path).is_some() { return Some(SourcePresenceV1::File); }
        let prefix = format!("{}/", path.as_str());
        snapshot.members().any(|m| m.path.as_str().starts_with(&prefix))
            .then_some(SourcePresenceV1::MaterializedDirectory)
    }
    /// Each revision has its own strictly path-ordered stream and EOF root.
    /// Retained revisions must be validated under their frozen profile, rather
    /// than merged into the current rule runner or renamed with path suffixes.
    pub fn stream(&self, revision: SourceRevision) -> Result<SourceMemberStreamV1<'_>> {
        let snapshot = self.snapshots.iter().find(|s| s.revision() == revision)
            .ok_or_else(|| StoreError::new(StoreErrorCode::MissingRevision, "revision is outside opened source cut"))?;
        let mut expected = Digest256Hasher::new();
        expected.update(b"tos-val-full-membership-v1\0");
        for member in snapshot.members() {
            feed_member(&mut expected, &member.path.as_str().to_owned(), member.size_bytes, member.sha256);
        }
        Ok(SourceMemberStreamV1 {
            cut: self, snapshot, last: None, count: 0, bytes: 0,
            actual: { let mut h = Digest256Hasher::new(); h.update(b"tos-val-full-membership-v1\0"); h },
            expected: SourceMembershipV1 { count: snapshot.member_count() as u64, digest: expected.finalize() },
            complete: false, failed: false,
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
    pub fn expectation(&self) -> SourceMembershipV1 { self.expected }
    pub fn coverage(&self) -> Option<SourceMembershipV1> { self.complete.then_some(self.expected) }
    pub fn next_member(&mut self, deadline: Instant, cancelled: &AtomicBool) -> Result<Option<SourceMemberV1>> {
        if self.failed { return Err(refusal("source stream already refused")); }
        let result = self.next_inner(deadline, cancelled);
        if result.is_err() { self.failed = true; self.complete = false; }
        result
    }
    fn next_inner(&mut self, deadline: Instant, cancelled: &AtomicBool) -> Result<Option<SourceMemberV1>> {
        check_time(deadline, cancelled)?;
        if self.complete { return Ok(None); }
        let Some(metadata) = self.snapshot.member_after(self.last.as_ref()) else {
            if self.count != self.expected.count || self.actual.clone().finalize() != self.expected.digest {
                return Err(StoreError::new(StoreErrorCode::DescriptorMismatch, "source stream membership differs"));
            }
            self.complete = true;
            return Ok(None);
        };
        let next_bytes = self.bytes.checked_add(metadata.size_bytes).ok_or_else(|| refusal("source stream byte count overflow"))?;
        if self.count >= self.cut.limits.max_members || next_bytes > self.cut.limits.max_total_bytes {
            return Err(refusal("source stream exceeds declared budget"));
        }
        let member = self.cut.read_member(self.snapshot.revision(), &metadata.path,
            self.cut.limits.max_member_bytes, deadline, cancelled)?;
        feed_member(&mut self.actual, metadata.path.as_str(), member.raw.len() as u64, Digest256::of_bytes(&member.raw));
        self.last = Some(metadata.path.clone());
        self.count += 1;
        self.bytes = next_bytes;
        Ok(Some(member))
    }
}

struct TimedStage<'a> { raw: Vec<u8>, deadline: Instant, cancelled: &'a AtomicBool }
impl Write for TimedStage<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.cancelled.load(Ordering::Relaxed) || Instant::now() >= self.deadline {
            return Err(io::Error::new(io::ErrorKind::TimedOut, "source stream cancelled or expired"));
        }
        self.raw.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}
fn check_time(deadline: Instant, cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline { Err(refusal("source read cancelled or expired")) } else { Ok(()) }
}
fn refusal(detail: &'static str) -> StoreError { StoreError::new(StoreErrorCode::BudgetExceeded, detail) }
fn feed_member(h: &mut Digest256Hasher, path: &str, length: u64, digest: Digest256) {
    h.update(&(path.len() as u64).to_be_bytes()); h.update(path.as_bytes());
    h.update(&length.to_be_bytes()); h.update(digest.as_bytes());
}
fn is_source_member(path: &str) -> bool {
    path.starts_with("ToS/")
        && !path.split('/').any(|p| matches!(p, ".git" | "payload" | "owner-local"))
        && (!(path.starts_with("ToS/derived-exports/") || path.starts_with("ToS/source-witnesses/catalog/")) || path.ends_with(".md"))
}
