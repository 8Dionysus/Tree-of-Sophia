//! Earned logical MemberPath coverage, separate from the historical flat V1 digest.
//! This source is a future-cohort seam, not wired into the frozen V1 validator.
//! Load ONLY as a child of source_admission_spooled_candidate: the private
//! request/profile check below uses the candidate owner's original fields.
//! Neither coverage nor interval evidence grants native admission or publication.
use crate::source_admission_spooled_candidate::{CandidateFence, SpoolCandidate};
use std::{
    io::{self, Cursor},
    mem::size_of,
};
use tos_foundation::{Digest256, RelativePath};
use tos_source_store::{
    SEGMENT_MEMBER_VALUE_BYTES_V1, SegmentIndexIntervalClosureV2, SegmentIndexKeySpaceV1,
    SegmentIndexReaderV2, SegmentIndexRootV2, SegmentMemberMetadataV1, decode_segment_member_value,
};

fn invalid(message: &'static str) -> io::Error {
    io::Error::other(message)
}
fn physical(error: impl std::fmt::Display) -> io::Error {
    io::Error::other(error.to_string())
}

/// Owner-private request/profile binding, available because this module is a
/// child of the candidate owner. There is no caller-supplied request assertion.
fn check_candidate_request(
    candidate: &SpoolCandidate<'_>,
    reader: Option<&SegmentIndexReaderV2>,
    charged_row_state_bytes: usize,
    total_state_cap: usize,
) -> io::Result<()> {
    candidate.tick()?;
    // Equal envelope also satisfies candidate bounded visitors' original cap.
    if total_state_cap > candidate.limits.candidate.max_state_bytes
        || charged_row_state_bytes != candidate.limits.max_row_state_bytes
        || reader.is_some_and(|r| {
            !r.shares_request(
                &candidate.ledger,
                &candidate.space_budget,
                candidate.deadline,
                &candidate.cancelled,
            )
        })
    {
        return Err(invalid(
            "logical membership original request or row envelope differs",
        ));
    }
    Ok(())
}

/// Finite simultaneous source/physical state. These caps are additive terms
/// inside the existing original operation, never independent resource grants.
#[derive(Clone, Copy)]
pub(crate) struct LogicalMembershipLimitsV2 {
    pub max_member_bytes: usize,
    pub max_candidate_owned_state_bytes: usize,
    pub max_physical_walk_workspace_bytes: usize,
    pub max_total_state_bytes: usize,
    pub retained_state_bytes: usize,
    pub max_path_bytes: usize,
    pub max_rows: u64,
    pub max_source_bytes: u64,
    pub max_key_bytes: u64,
}
impl LogicalMembershipLimitsV2 {
    fn check(self, physical_workspace: usize) -> io::Result<usize> {
        if self.max_member_bytes == 0
            || self.max_member_bytes == usize::MAX
            || self.max_path_bytes == 0
            || self.max_path_bytes > 8192
            || self.max_rows == u64::MAX
            || self.max_source_bytes == u64::MAX
            || self.max_key_bytes == u64::MAX
            || self.max_total_state_bytes == usize::MAX
            || self.max_candidate_owned_state_bytes == usize::MAX
            || self.max_physical_walk_workspace_bytes == usize::MAX
            || physical_workspace > self.max_physical_walk_workspace_bytes
        {
            return Err(invalid("logical membership finite profile differs"));
        }
        let own = size_of::<LogicalMembershipRootCoverageV2>()
            .checked_add(size_of::<LogicalMembershipIntervalV2>())
            .and_then(|n| n.checked_add(SEGMENT_MEMBER_VALUE_BYTES_V1 * 2))
            .and_then(|n| n.checked_add(self.max_path_bytes.checked_mul(32)?))
            .and_then(|n| n.checked_add(4096))
            .ok_or_else(|| invalid("logical membership state overflow"))?;
        // Charge overlapping candidate, physical reader/walker and callback state.
        if self
            .retained_state_bytes
            .checked_add(own)
            .and_then(|n| n.checked_add(self.max_candidate_owned_state_bytes))
            .and_then(|n| n.checked_add(physical_workspace))
            .is_none_or(|n| n > self.max_total_state_bytes)
        {
            return Err(invalid(
                "logical membership simultaneous state exceeds profile",
            ));
        }
        Ok(own + physical_workspace)
    }
}

/// Private constructor: genuine candidate byte EOF plus full authenticated
/// physical EOF and exact row/mode equality. Holds the original reader request.
/// Logical root SHA is tree-shaped V2 identity, never SourceMembershipV1.
pub(crate) struct LogicalMembershipRootCoverageV2<'a> {
    reader: &'a SegmentIndexReaderV2,
    fence: CandidateFence,
    root: SegmentIndexRootV2,
    source_bytes: u64,
    key_bytes: u64,
    full_page_count: u64,
    original_state_cap: usize,
}
impl<'a> LogicalMembershipRootCoverageV2<'a> {
    pub(crate) fn root(&self) -> SegmentIndexRootV2 {
        self.root
    }
    pub(crate) fn fence(&self) -> CandidateFence {
        self.fence
    }
    pub(crate) fn source_bytes(&self) -> u64 {
        self.source_bytes
    }
    pub(crate) fn key_bytes(&self) -> u64 {
        self.key_bytes
    }
    pub(crate) fn full_page_count(&self) -> u64 {
        self.full_page_count
    }
}

/// Initial/restore bridge only. Every actual source byte is read and hashed by
/// SpoolCandidate; every physical page/leaf reaches EOF. No affected-key shortcut.
pub(crate) fn cover_full_member_root_v2<'a>(
    candidate: &SpoolCandidate<'_>,
    reader: &'a SegmentIndexReaderV2,
    limits: LogicalMembershipLimitsV2,
) -> io::Result<LogicalMembershipRootCoverageV2<'a>> {
    check_candidate_request(
        candidate,
        Some(reader),
        limits.max_candidate_owned_state_bytes,
        limits.max_total_state_bytes,
    )?;
    if reader.keyspace() != SegmentIndexKeySpaceV1::MemberPath {
        return Err(invalid("logical membership root role differs"));
    }
    let workspace = reader.walk_workspace_upper_bound().map_err(physical)?;
    let callback_state = limits
        .check(workspace)?
        .checked_add(limits.retained_state_bytes)
        .ok_or_else(|| invalid("logical membership callback retained state overflow"))?;
    let root = reader.root_descriptor();
    let fence = candidate.fence()?;
    let (mut rows, mut source_bytes, mut key_bytes) = (0u64, 0u64, 0u64);
    let membership = candidate.for_each_verified_member(
        limits.max_member_bytes,
        limits.max_candidate_owned_state_bytes,
        callback_state,
        &mut |metadata, _actual_source_bytes| {
            let key = metadata.path.as_str().as_bytes();
            if key.len() > limits.max_path_bytes {
                return Err(invalid("logical member path exceeds cap"));
            }
            let mut wire = [0u8; SEGMENT_MEMBER_VALUE_BYTES_V1];
            let mut sink = Cursor::new(wire.as_mut_slice());
            if reader.read_value(key, &mut sink).map_err(physical)? != Some(52)
                || sink.position() != 52
            {
                return Err(invalid("logical member exact row is absent"));
            }
            let decoded = decode_segment_member_value(&wire).map_err(physical)?;
            if decoded.size_bytes != metadata.size_bytes
                || decoded.raw_sha256 != metadata.sha256
                || decoded.source_mode != metadata.mode
            {
                return Err(invalid(
                    "logical member differs from authentic candidate metadata",
                ));
            }
            rows = rows
                .checked_add(1)
                .ok_or_else(|| invalid("logical member count overflow"))?;
            source_bytes = source_bytes
                .checked_add(metadata.size_bytes)
                .ok_or_else(|| invalid("logical source bytes overflow"))?;
            key_bytes = key_bytes
                .checked_add(key.len() as u64)
                .ok_or_else(|| invalid("logical key bytes overflow"))?;
            if rows > limits.max_rows
                || source_bytes > limits.max_source_bytes
                || key_bytes > limits.max_key_bytes
            {
                return Err(invalid("logical membership actual totals exceed cap"));
            }
            Ok(())
        },
    )?;
    // A physical full walk independently rejects extras, malformed roles,
    // summaries and missing branches. The candidate visitor already checked
    // every value's path, raw SHA, source length and actual source mode.
    let closure = reader
        .walk_pages_and_leaves(
            limits.max_physical_walk_workspace_bytes,
            &mut |_| Ok(()),
            &mut |leaf| {
                if leaf.key.len() > limits.max_path_bytes {
                    return Err(tos_source_store::StoreError::new(
                        tos_source_store::StoreErrorCode::CorruptSelectedObject,
                        "member path exceeds finite cap",
                    ));
                }
                let text = std::str::from_utf8(leaf.key).map_err(|_| {
                    tos_source_store::StoreError::new(
                        tos_source_store::StoreErrorCode::CorruptSelectedObject,
                        "member path is not UTF8",
                    )
                })?;
                let path = RelativePath::parse(text).map_err(|_| {
                    tos_source_store::StoreError::new(
                        tos_source_store::StoreErrorCode::CorruptSelectedObject,
                        "member path is not canonical",
                    )
                })?;
                if path.as_str().as_bytes() != leaf.key {
                    return Err(tos_source_store::StoreError::new(
                        tos_source_store::StoreErrorCode::CorruptSelectedObject,
                        "member path normalization differs",
                    ));
                }
                Ok(())
            },
        )
        .map_err(physical)?;
    let values = rows
        .checked_mul(52)
        .ok_or_else(|| invalid("logical metadata bytes overflow"))?;
    if candidate.fence()? != fence
        || membership != fence.membership
        || source_bytes != fence.source_bytes
        || closure.root != root
        || closure.logical_summary.row_count != rows
        || closure.logical_summary.key_bytes != key_bytes
        || closure.logical_summary.value_bytes != values
    {
        return Err(invalid("logical membership full EOF binding differs"));
    }
    Ok(LogicalMembershipRootCoverageV2 {
        reader,
        fence,
        root,
        source_bytes,
        key_bytes,
        full_page_count: closure.page_count,
        original_state_cap: candidate.limits.candidate.max_state_bytes,
    })
}

/// Authenticated membership evidence for a literal half-open byte interval.
/// This does not claim dependency/semantic affected closure or a new root.
pub(crate) struct LogicalMembershipIntervalV2 {
    root: SegmentIndexRootV2,
    closure: SegmentIndexIntervalClosureV2,
    source_bytes: u64,
    row_transcript: Digest256,
}
impl LogicalMembershipIntervalV2 {
    pub(crate) fn root(&self) -> SegmentIndexRootV2 {
        self.root
    }
    pub(crate) fn closure(&self) -> &SegmentIndexIntervalClosureV2 {
        &self.closure
    }
    pub(crate) fn source_bytes(&self) -> u64 {
        self.source_bytes
    }
    pub(crate) fn row_transcript(&self) -> Digest256 {
        self.row_transcript
    }
}

/// Only the earned full basis can invoke this interval method. Callers cannot
/// pass constructed physical receipts. No guessed prefix or exact-key successor.
/// The maintained native dependency kernel must separately earn semantic closure.
pub(crate) fn cover_member_interval_v2(
    basis: &LogicalMembershipRootCoverageV2<'_>,
    lower: &[u8],
    upper: &[u8],
    limits: LogicalMembershipLimitsV2,
) -> io::Result<LogicalMembershipIntervalV2> {
    // Borrowed caller bounds are still simultaneous retained state; validate
    // before hashing so oversized input cannot spend unbounded pre-fence CPU.
    if lower.is_empty()
        || upper.is_empty()
        || lower.len() > 8193
        || upper.len() > 8193
        || lower > upper
        || limits.max_total_state_bytes > basis.original_state_cap
    {
        return Err(invalid("logical membership interval literal bounds differ"));
    }
    let bound_bytes = lower
        .len()
        .checked_add(upper.len())
        .ok_or_else(|| invalid("interval bound state overflow"))?;
    let mut limits = limits;
    limits.retained_state_bytes = limits
        .retained_state_bytes
        .checked_add(bound_bytes)
        .ok_or_else(|| invalid("interval retained state overflow"))?;
    let reader = basis.reader;
    if reader.root_descriptor() != basis.root {
        return Err(invalid("logical membership original root changed"));
    }
    let workspace = reader
        .interval_workspace_upper_bound(52)
        .map_err(physical)?;
    limits.check(workspace)?;
    let (mut rows, mut source_bytes, mut key_bytes) = (0u64, 0u64, 0u64);
    let mut hash = tos_foundation::Digest256Hasher::new();
    hash.update(b"TOS_FND_MEMBER_INTERVAL_V2\0");
    hash.update(&(lower.len() as u64).to_le_bytes());
    hash.update(lower);
    hash.update(&(upper.len() as u64).to_le_bytes());
    hash.update(upper);
    let closure = reader
        .walk_interval_values(
            lower,
            upper,
            limits.max_physical_walk_workspace_bytes,
            52,
            &mut |leaf, raw| {
                if leaf.key.len() > limits.max_path_bytes {
                    return Err(interval_refusal());
                }
                let text = std::str::from_utf8(leaf.key).map_err(|_| interval_refusal())?;
                let path = RelativePath::parse(text).map_err(|_| interval_refusal())?;
                if path.as_str().as_bytes() != leaf.key {
                    return Err(interval_refusal());
                }
                let metadata: SegmentMemberMetadataV1 = decode_segment_member_value(raw)?;
                rows = rows.checked_add(1).ok_or_else(interval_refusal)?;
                source_bytes = source_bytes
                    .checked_add(metadata.size_bytes)
                    .ok_or_else(interval_refusal)?;
                key_bytes = key_bytes
                    .checked_add(leaf.key.len() as u64)
                    .ok_or_else(interval_refusal)?;
                if rows > limits.max_rows
                    || source_bytes > limits.max_source_bytes
                    || key_bytes > limits.max_key_bytes
                {
                    return Err(interval_refusal());
                }
                hash.update(&(leaf.key.len() as u64).to_le_bytes());
                hash.update(leaf.key);
                hash.update(raw);
                Ok(())
            },
        )
        .map_err(physical)?;
    if closure.root != basis.root
        || closure.rows != rows
        || closure.logical_key_bytes != key_bytes
        || closure.physical_value_bytes
            != rows
                .checked_mul(52)
                .ok_or_else(|| invalid("interval metadata bytes overflow"))?
    {
        return Err(invalid("logical membership interval EOF differs"));
    }
    hash.update(b"EOF\0");
    hash.update(&rows.to_le_bytes());
    hash.update(&source_bytes.to_le_bytes());
    Ok(LogicalMembershipIntervalV2 {
        root: basis.root,
        closure,
        source_bytes,
        row_transcript: hash.finalize(),
    })
}
fn interval_refusal() -> tos_source_store::StoreError {
    tos_source_store::StoreError::new(
        tos_source_store::StoreErrorCode::CorruptSelectedObject,
        "logical membership interval row or bound differs",
    )
}

/// EOF of the actual candidate maintained reverse-dependency affected queue.
/// This is source-local scheduling evidence. It does not establish that the
/// native parser, history, identity or edge checks completed for these members.
pub(crate) struct AffectedClosureBasisV2 {
    fence: CandidateFence,
    rows: u64,
    key_bytes: u64,
    present_source_bytes: u64,
    transcript: Digest256,
}
impl AffectedClosureBasisV2 {
    pub(crate) fn fence(&self) -> CandidateFence {
        self.fence
    }
    pub(crate) fn rows(&self) -> u64 {
        self.rows
    }
    pub(crate) fn key_bytes(&self) -> u64 {
        self.key_bytes
    }
    pub(crate) fn present_source_bytes(&self) -> u64 {
        self.present_source_bytes
    }
    pub(crate) fn transcript(&self) -> Digest256 {
        self.transcript
    }
}
/// Typed source membership meaning for each affected path; absent means absent
/// from this candidate, not a claimed historical retirement authorization.
pub(crate) enum AffectedMemberV2<'a> {
    Present {
        path: &'a RelativePath,
        metadata: SegmentMemberMetadataV1,
    },
    Absent {
        path: &'a RelativePath,
    },
}
/// Effects must stay private until this function returns. Caller native parser
/// state is included in retained_state_bytes before either cursor allocates.
pub(crate) fn cover_candidate_affected_basis_v2(
    candidate: &SpoolCandidate<'_>,
    limits: LogicalMembershipLimitsV2,
    visit: &mut impl FnMut(AffectedMemberV2<'_>) -> io::Result<()>,
) -> io::Result<AffectedClosureBasisV2> {
    check_candidate_request(
        candidate,
        None,
        limits.max_candidate_owned_state_bytes,
        limits.max_total_state_bytes,
    )?;
    limits.check(0)?;
    let fence = candidate.fence()?;
    let mut hash = tos_foundation::Digest256Hasher::new();
    hash.update(b"TOS_FND_CANDIDATE_AFFECTED_BASIS_V2\0");
    hash.update(fence.batch_sha256.as_bytes());
    let (mut rows, mut key_bytes, mut present_source_bytes) = (0u64, 0u64, 0u64);
    let mut previous: Option<RelativePath> = None;
    loop {
        let Some(path) = candidate.affected_after(previous.as_ref())? else {
            break;
        };
        if path.as_str().len() > limits.max_path_bytes
            || previous
                .as_ref()
                .is_some_and(|p| p.as_str() >= path.as_str())
        {
            return Err(invalid("candidate affected queue order or cap differs"));
        }
        rows = rows
            .checked_add(1)
            .ok_or_else(|| invalid("affected rows overflow"))?;
        key_bytes = key_bytes
            .checked_add(path.as_str().len() as u64)
            .ok_or_else(|| invalid("affected keys overflow"))?;
        if rows > limits.max_rows || key_bytes > limits.max_key_bytes {
            return Err(invalid("affected queue exceeds finite cap"));
        }
        hash.update(&(path.as_str().len() as u64).to_le_bytes());
        hash.update(path.as_str().as_bytes());
        match candidate.member_bounded(&path, limits.max_candidate_owned_state_bytes)? {
            Some(meta) => {
                present_source_bytes = present_source_bytes
                    .checked_add(meta.size_bytes)
                    .ok_or_else(|| invalid("affected source bytes overflow"))?;
                if present_source_bytes > limits.max_source_bytes {
                    return Err(invalid("affected source bytes exceed cap"));
                }
                hash.update(&[1]);
                hash.update(&meta.size_bytes.to_le_bytes());
                hash.update(meta.sha256.as_bytes());
                hash.update(&meta.mode.to_le_bytes());
                visit(AffectedMemberV2::Present {
                    path: &path,
                    metadata: SegmentMemberMetadataV1 {
                        size_bytes: meta.size_bytes,
                        raw_sha256: meta.sha256,
                        source_mode: meta.mode,
                    },
                })?;
            }
            None => {
                hash.update(&[0]);
                visit(AffectedMemberV2::Absent { path: &path })?;
            }
        }
        previous = Some(path);
    }
    if candidate.fence()? != fence {
        return Err(invalid("affected queue original fence changed"));
    }
    hash.update(b"EOF\0");
    for count in [rows, key_bytes, present_source_bytes] {
        hash.update(&count.to_le_bytes());
    }
    Ok(AffectedClosureBasisV2 {
        fence,
        rows,
        key_bytes,
        present_source_bytes,
        transcript: hash.finalize(),
    })
}
