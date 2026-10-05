//! Canonical bounded-row serialization of the genuinely validated private index.
//! This serializer does not construct an index or mint source admission.
use super::source_admission::invalid;
use super::source_admission_candidate::canonical;
use super::source_admission_spooled_candidate::SpoolCandidate;
use super::source_admission_spooled_index::IndexView;
use serde_json::{Value, json};
use std::{
    fs::File,
    io::{self, Write},
    os::unix::fs::MetadataExt,
};
use tos_foundation::{Digest256, Digest256Hasher, JsonLimits, JsonString, JsonValue};
use tos_source_store::PinnedSqliteSpaceReservation;

/// A held output file with its original shared physical-space reservation.
pub(crate) struct ManifestSink<'a> {
    pub file: &'a mut File,
    pub reservation: &'a PinnedSqliteSpaceReservation,
}

/// A supplied finite output/row profile, independent of total member count.
#[derive(Clone, Copy)]
pub(crate) struct ManifestStreamLimits {
    pub max_manifest_bytes: u64,
    pub row_json: JsonLimits,
}

// Forecast the maintained parser's bounded owned strings, key/value slots,
// container growth and recursive stack before any manifest row is allocated.
// This is logical workspace, not an allocator/RSS measurement. Encoded buffers
// and the four simultaneous row representations are reserved by the callers.
fn row_json_state_upper_bound(limits: JsonLimits) -> io::Result<usize> {
    let slot = std::mem::size_of::<(JsonValue, JsonValue)>()
        .checked_add(5 * std::mem::size_of::<usize>())
        .and_then(|n| n.checked_mul(2))
        .ok_or_else(|| invalid("manifest JSON slot state overflow"))?;
    let stack_slot = std::mem::size_of::<JsonValue>()
        + std::mem::size_of::<JsonString>()
        + std::mem::size_of::<std::collections::HashMap<Vec<u16>, usize>>()
        + 2 * std::mem::size_of::<Vec<JsonValue>>();
    limits
        .max_bytes
        .checked_mul(16)
        .and_then(|n| {
            limits
                .max_visits
                .min(limits.max_bytes)
                .checked_mul(slot)
                .and_then(|slots| n.checked_add(slots))
        })
        .and_then(|n| {
            limits
                .max_depth
                .checked_add(1)
                .and_then(|depth| depth.checked_mul(stack_slot))
                .and_then(|stack| n.checked_add(stack))
        })
        .ok_or_else(|| invalid("manifest JSON retained state overflow"))
}

struct Output<'a, 'host> {
    candidate: &'a SpoolCandidate<'host>,
    sink: Option<ManifestSink<'a>>,
    limit: u64,
    bytes: u64,
    hash: Digest256Hasher,
}
impl Output<'_, '_> {
    fn reserve_row(&self, limits: JsonLimits, locator_bytes: usize) -> io::Result<()> {
        self.candidate.check_state(
            row_json_state_upper_bound(limits)?
                .checked_mul(4)
                .and_then(|n| n.checked_add(limits.max_bytes.checked_mul(8)?))
                .and_then(|n| n.checked_add(locator_bytes.checked_mul(16)?))
                .and_then(|n| n.checked_add(4096))
                .ok_or_else(|| invalid("manifest row/locator overlap overflow"))?,
        )
    }
    fn emit(&mut self, raw: &[u8]) -> io::Result<()> {
        self.candidate.tick()?;
        let next = self
            .bytes
            .checked_add(raw.len() as u64)
            .filter(|n| *n <= self.limit)
            .ok_or_else(|| invalid("streamed manifest output bound"))?;
        if let Some(sink) = self.sink.as_mut() {
            let mut remaining = raw;
            while !remaining.is_empty() {
                self.candidate.tick()?;
                self.candidate.debit_write(remaining.len() as u64)?;
                match sink.file.write(remaining) {
                    Ok(0) => return Err(io::Error::new(io::ErrorKind::WriteZero, "manifest sink")),
                    Ok(n) => {
                        self.candidate.record_write_returned(n as u64)?;
                        let allocated = sink
                            .file
                            .metadata()?
                            .blocks()
                            .checked_mul(512)
                            .ok_or_else(|| invalid("manifest allocated bytes overflow"))?;
                        sink.reservation
                            .update_actual_allocated(allocated)
                            .map_err(invalid)?;
                        remaining = &remaining[n..];
                    }
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(e),
                }
                self.candidate.tick()?;
            }
        }
        self.hash.update(raw);
        self.bytes = next;
        self.candidate.tick()
    }
    fn value(&mut self, value: &Value, limits: JsonLimits) -> io::Result<()> {
        self.candidate.tick()?;
        let raw = canonical(value, limits)?;
        self.candidate.tick()?;
        let body = raw
            .strip_suffix(b"\n")
            .ok_or_else(|| invalid("CorpusSnapshotV1 row LF absent"))?;
        self.emit(body)
    }
}

/// The body pass has no `revision` field. Only a native validated IndexView may
/// supply identity/dependency rows. The physical pass writes a held caller-owned
/// sink; its quota reservation and FD custody must outlive the entire publish.
/// On any error the caller abandons that output and its candidate invocation.
pub(crate) fn serialize(
    candidate: &SpoolCandidate<'_>,
    index: &IndexView<'_>,
    revision: Option<Digest256>,
    sink: Option<ManifestSink<'_>>,
    limits: ManifestStreamLimits,
) -> io::Result<(u64, Digest256)> {
    let result = serialize_inner(candidate, index, revision, sink, limits);
    if result.is_err() {
        candidate.abandon();
    }
    result
}
fn serialize_inner(
    candidate: &SpoolCandidate<'_>,
    index: &IndexView<'_>,
    revision: Option<Digest256>,
    sink: Option<ManifestSink<'_>>,
    limits: ManifestStreamLimits,
) -> io::Result<(u64, Digest256)> {
    candidate.tick()?;
    if limits.max_manifest_bytes == 0 || limits.max_manifest_bytes == u64::MAX {
        return Err(invalid("streamed manifest requires finite byte profile"));
    }
    let row_json = JsonLimits::new(
        limits.row_json.max_bytes,
        limits.row_json.max_depth,
        limits.row_json.max_visits,
        limits.row_json.max_integer_digits,
    )
    .map_err(invalid)?;
    // Canonical row encoding retains the input DOM, typed parse/canonical DOM,
    // original and final row buffers. Reserve their finite overlap BEFORE rows.
    candidate.check_state(
        row_json_state_upper_bound(row_json)?
            .checked_mul(4)
            .and_then(|n| n.checked_add(row_json.max_bytes.checked_mul(8)?))
            .ok_or_else(|| invalid("manifest row state overflow"))?,
    )?;
    index.verify_candidate()?;
    let fence = index.fence();
    if candidate.fence()? != fence {
        return Err(invalid("manifest candidate/index fence differs"));
    }
    let mut out = Output {
        candidate,
        sink,
        limit: limits.max_manifest_bytes,
        bytes: 0,
        hash: Digest256Hasher::new(),
    };
    out.reserve_row(row_json, 0)?;
    // Exact lexicographic root-key order of CorpusSnapshotV1 canonical bytes.
    out.emit(b"{\"base_revision\":")?;
    out.value(
        &fence
            .base_revision
            .map(|r| Value::String(r.0.to_hex()))
            .unwrap_or(Value::Null),
        row_json,
    )?;
    out.emit(b",\"dependencies\":{")?;
    let mut source_after = None;
    let mut sources = 0u64;
    let mut edges = 0u64;
    while let Some(source) = index.dependency_source_after(source_after.as_ref())? {
        out.reserve_row(
            row_json,
            source
                .as_str()
                .len()
                .checked_add(source_after.as_ref().map_or(0, |p| p.as_str().len()))
                .ok_or_else(|| invalid("manifest locator state overflow"))?,
        )?;
        if source_after.as_ref().is_some_and(|p| p >= &source) {
            return Err(invalid("manifest dependency source order"));
        }
        if sources != 0 {
            out.emit(b",")?;
        }
        out.value(&Value::String(source.as_str().into()), row_json)?;
        out.emit(b":[")?;
        let mut after = None;
        let mut count = 0u64;
        while let Some(target) = index.dependency_after(&source, after.as_ref())? {
            out.reserve_row(
                row_json,
                source
                    .as_str()
                    .len()
                    .checked_add(source_after.as_ref().map_or(0, |p| p.as_str().len()))
                    .and_then(|n| n.checked_add(target.as_str().len()))
                    .and_then(|n| n.checked_add(after.as_ref().map_or(0, |p| p.as_str().len())))
                    .ok_or_else(|| invalid("manifest locator state overflow"))?,
            )?;
            if after.as_ref().is_some_and(|p| p >= &target) {
                return Err(invalid("manifest dependency target order"));
            }
            if count != 0 {
                out.emit(b",")?;
            }
            out.value(&Value::String(target.as_str().into()), row_json)?;
            after = Some(target);
            count = count
                .checked_add(1)
                .ok_or_else(|| invalid("manifest edge count"))?;
        }
        out.emit(b"]")?;
        edges = edges
            .checked_add(count)
            .ok_or_else(|| invalid("manifest edge count"))?;
        sources = sources
            .checked_add(1)
            .ok_or_else(|| invalid("manifest source count"))?;
        source_after = Some(source);
    }
    if (sources, edges) != (index.dependency_source_count(), index.dependency_count()) {
        return Err(invalid("manifest dependency EOF count differs"));
    }
    drop(source_after);
    out.emit(b"},\"files\":[")?;
    let mut after = None;
    let mut members = 0u64;
    let mut source_bytes = 0u64;
    while let Some(row) = candidate.member_after(after.as_ref())? {
        out.reserve_row(
            row_json,
            row.path
                .as_str()
                .len()
                .checked_add(after.as_ref().map_or(0, |p| p.as_str().len()))
                .ok_or_else(|| invalid("manifest locator state overflow"))?,
        )?;
        if after.as_ref().is_some_and(|p| p >= &row.path) {
            return Err(invalid("manifest member order"));
        }
        if members != 0 {
            out.emit(b",")?;
        }
        out.value(
            &json!({"path":row.path.as_str(),"sha256":row.sha256.to_hex(),
            "size_bytes":row.size_bytes,"mode":row.mode}),
            row_json,
        )?;
        source_bytes = source_bytes
            .checked_add(row.size_bytes)
            .ok_or_else(|| invalid("manifest source bytes"))?;
        members = members
            .checked_add(1)
            .ok_or_else(|| invalid("manifest member count"))?;
        after = Some(row.path);
    }
    if (members, source_bytes) != candidate.membership_counts() {
        return Err(invalid("manifest member EOF count differs"));
    }
    drop(after);
    out.emit(b"],\"identities\":{")?;
    let mut after = None;
    let mut identities = 0u64;
    while let Some((id, path)) = index.identities_after(after.as_deref())? {
        out.reserve_row(
            row_json,
            id.len()
                .checked_add(path.as_str().len())
                .and_then(|n| n.checked_add(after.as_ref().map_or(0, String::len)))
                .ok_or_else(|| invalid("manifest locator state overflow"))?,
        )?;
        if after.as_ref().is_some_and(|p| p >= &id) {
            return Err(invalid("manifest identity order"));
        }
        if identities != 0 {
            out.emit(b",")?;
        }
        out.value(&Value::String(id.clone()), row_json)?;
        out.emit(b":")?;
        out.value(&Value::String(path.as_str().into()), row_json)?;
        after = Some(id);
        identities = identities
            .checked_add(1)
            .ok_or_else(|| invalid("manifest identity count"))?;
    }
    if identities != index.identity_count() {
        return Err(invalid("manifest identity EOF count differs"));
    }
    drop(after);
    out.emit(b"},\"retirements\":[")?;
    for ordinal in 0..fence.retirement_count {
        let row = candidate
            .retirement_at(ordinal)?
            .ok_or_else(|| invalid("manifest retirement absent"))?;
        out.reserve_row(
            row_json,
            row.path
                .as_str()
                .len()
                .checked_add(row.event_ref.as_str().len())
                .ok_or_else(|| invalid("manifest locator state overflow"))?,
        )?;
        if ordinal != 0 {
            out.emit(b",")?;
        }
        out.value(
            &json!({"path":row.path.as_str(),"sha256":row.sha256.to_hex(),
            "event_ref":row.event_ref.as_str(),"event_sha256":row.event_sha256.to_hex(),
            "event_size_bytes":row.event_size_bytes}),
            row_json,
        )?;
    }
    out.emit(b"]")?;
    if let Some(revision) = revision {
        out.emit(b",\"revision\":")?;
        out.value(&Value::String(revision.to_hex()), row_json)?;
    }
    out.emit(b",\"schema_version\":\"tos_corpus_snapshot_v1\",\"validator_sha256\":")?;
    out.value(&Value::String(fence.validator_sha256.to_hex()), row_json)?;
    out.emit(b"}\n")?;
    index.verify_candidate()?;
    if candidate.fence()? != fence {
        return Err(invalid("manifest candidate changed during serialization"));
    }
    candidate.tick()?;
    Ok((out.bytes, out.hash.finalize()))
}
