//! Candidate-side source index and exact retirement-only transfer.
//!
//! This is the maintained `corpus_source_validation.source_index` /
//! `corpus_source_retirement` kernel. Inputs come from root's held candidate
//! custody and the complete freshly rendered owner report. Nothing here selects
//! a store revision, writes bytes, accepts source meaning, or grants rights.
use serde_json::{Value, json};
use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    io,
    ops::Bound,
};
use tos_foundation::{
    Digest256, FoundationErrorCode, JsonLimits, JsonMode, JsonValue, RelativePath, SourceRevision,
    parse_json, parse_json_with_state_budget,
};
use tos_source_store::{
    SourceCutFormat, SourceCutMemberTuple, SourceCutMemberWitness, SourceCutReadsetV1,
    SourceCutSelection, SourcePresenceV1,
};
use tos_validation::source_cut::{CutExecutionBinding, CutPreparedSchemaExecutionBinding};

/// Metadata copied only from the already selected V2 current root. The rootset
/// digest and revision let the delta validator reject a proof borrowed from a
/// historical row or a stale selection without walking the global History
/// tree.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SelectedNativeAdmissionRootV1 {
    revision: SourceRevision,
    rootset_sha256: Digest256,
    validator_sha256: Digest256,
    membership_v1: Option<tos_source_store::SourceMembershipV1>,
    member_count: u64,
    source_bytes: u64,
    identity_count: u64,
    dependency_source_count: u64,
    dependency_count: u64,
    retirement_count: u64,
    completion_proof: crate::source_admission_spooled_index::NativeAdmissionCompletionProofV1,
}

impl SelectedNativeAdmissionRootV1 {
    pub(crate) fn from_selected_current_roots(
        roots: &super::source_admission_segment_v2::SourceRevisionRootsV2,
        rootset_sha256: Digest256,
    ) -> io::Result<Self> {
        let completion_proof = roots
            .completion_proof
            .ok_or_else(|| invalid("selected V2 root has no native completion proof"))?;
        if completion_proof.validator_sha256() != roots.validator_sha256
            || completion_proof.source_bytes() != roots.source_bytes
            || completion_proof.identity_count() != roots.identity_count
            || completion_proof.dependency_source_count() != roots.dependency_source_count
            || completion_proof.dependency_count() != roots.dependency_count
            || completion_proof.membership_v1() != roots.membership_v1
            || roots
                .membership_v1
                .is_some_and(|membership| membership.count != roots.member_count)
        {
            return Err(invalid(
                "selected V2 completion proof differs from its root",
            ));
        }
        Ok(Self {
            revision: roots.revision,
            rootset_sha256,
            validator_sha256: roots.validator_sha256,
            membership_v1: roots.membership_v1,
            member_count: roots.member_count,
            source_bytes: roots.source_bytes,
            identity_count: roots.identity_count,
            dependency_source_count: roots.dependency_source_count,
            dependency_count: roots.dependency_count,
            retirement_count: roots.retirement_count,
            completion_proof,
        })
    }

    pub(crate) fn revision(self) -> SourceRevision {
        self.revision
    }
    pub(crate) fn rootset_sha256(self) -> Digest256 {
        self.rootset_sha256
    }
    pub(crate) fn validator_sha256(self) -> Digest256 {
        self.validator_sha256
    }
    pub(crate) fn membership_v1(self) -> Option<tos_source_store::SourceMembershipV1> {
        self.membership_v1
    }
    pub(crate) fn member_count(self) -> u64 {
        self.member_count
    }
    pub(crate) fn source_bytes(self) -> u64 {
        self.source_bytes
    }
    pub(crate) fn identity_count(self) -> u64 {
        self.identity_count
    }
    pub(crate) fn dependency_source_count(self) -> u64 {
        self.dependency_source_count
    }
    pub(crate) fn dependency_count(self) -> u64 {
        self.dependency_count
    }
    pub(crate) fn retirement_count(self) -> u64 {
        self.retirement_count
    }
    pub(crate) fn completion_proof(
        self,
    ) -> crate::source_admission_spooled_index::NativeAdmissionCompletionProofV1 {
        self.completion_proof
    }
}

/// The selected V2 read adapter implements these operations against its one
/// held current-root session. Each lookup must append its exact positive or
/// negative witness to the retained `SourceCutReadsetV1`; `tick` and
/// `check_state` must charge the same invocation clock/work/state/cancel
/// handles used by SourceEntry preparation.
pub(crate) trait SourceEntryDeltaIndexReader {
    fn tick(&mut self, work_units: usize) -> io::Result<()>;
    fn check_state(&mut self, additional_bytes: usize) -> io::Result<()>;
    /// Recheck this exact accumulated readset against the session's latest
    /// selected root, using the same invocation ledger and cancellation state.
    fn verify_readset_current(&mut self) -> io::Result<SourceCutSelection>;
    fn current_member(&mut self, path: &RelativePath) -> io::Result<Option<SourceCutMemberTuple>>;
    fn current_identity_path(&mut self, id: &str) -> io::Result<Option<RelativePath>>;
    fn current_dependencies(&mut self, source: &RelativePath) -> io::Result<Vec<RelativePath>>;
    fn current_object_refcount(&mut self, digest: Digest256) -> io::Result<Option<u64>>;
    fn readset(&self) -> &SourceCutReadsetV1;
    fn json_limits(&self) -> JsonLimits;
    /// Remaining parse-state capacity under the same selected invocation
    /// profile before accounting the delta result currently held by this
    /// validator. The validator subtracts that local reservation explicitly.
    fn json_state_bytes(&self) -> usize;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceEntryDependencyDeltaV1 {
    source: RelativePath,
    before: Vec<RelativePath>,
    after: Vec<RelativePath>,
}

impl SourceEntryDependencyDeltaV1 {
    pub(crate) fn source(&self) -> &RelativePath {
        &self.source
    }
    pub(crate) fn before(&self) -> &[RelativePath] {
        &self.before
    }
    pub(crate) fn after(&self) -> &[RelativePath] {
        &self.after
    }
}

/// Exact before/after tuple for one existing member row. SourceEntry replaces
/// bytes only, so the source mode is preserved from the authenticated base.
/// The matching `SourceChange` retains the full after bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceEntryMemberDeltaV1 {
    path: RelativePath,
    before: SourceCutMemberTuple,
    after: SourceCutMemberTuple,
}

impl SourceEntryMemberDeltaV1 {
    pub(crate) fn path(&self) -> &RelativePath {
        &self.path
    }
    pub(crate) fn before(&self) -> SourceCutMemberTuple {
        self.before
    }
    pub(crate) fn after(&self) -> SourceCutMemberTuple {
        self.after
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceEntryObjectRefcountDeltaV1 {
    digest: Digest256,
    before_count: Option<u64>,
    after_count: Option<u64>,
}

impl SourceEntryObjectRefcountDeltaV1 {
    pub(crate) fn digest(self) -> Digest256 {
        self.digest
    }
    pub(crate) fn before_count(self) -> Option<u64> {
        self.before_count
    }
    pub(crate) fn after_count(self) -> Option<u64> {
        self.after_count
    }
}

/// A verified owner-local delta. This proves the exact three prepared member
/// replacements and the resulting outgoing dependency rows over the selected
/// authenticated base. It is not a synthetic full `IndexView`; the caller
/// must still publish these bounded COW updates through the V2 root builder.
#[derive(Debug)]
pub(crate) struct ValidatedSourceEntryDeltaV1<'a> {
    selected_base: SelectedNativeAdmissionRootV1,
    observed_base_revision: SourceRevision,
    proposal: &'a crate::source_command::PreparedCommand,
    source_path: RelativePath,
    record_id: &'a str,
    updates: [&'a crate::source_command::SourceChange; 3],
    member_updates: [SourceEntryMemberDeltaV1; 3],
    dependency_updates: [SourceEntryDependencyDeltaV1; 3],
    object_refcount_updates: Vec<SourceEntryObjectRefcountDeltaV1>,
    member_count_after: u64,
    source_bytes_after: u64,
    identity_count_after: u64,
    dependency_source_count_after: u64,
    dependency_count_after: u64,
    retirement_count_after: u64,
    prepared_schema: tos_validation::source_cut::CutPreparedSchemaExecutionBinding,
}

impl<'proposal> ValidatedSourceEntryDeltaV1<'proposal> {
    pub(crate) fn selected_base(&self) -> SelectedNativeAdmissionRootV1 {
        self.selected_base
    }
    pub(crate) fn observed_base_revision(&self) -> SourceRevision {
        self.observed_base_revision
    }
    pub(crate) fn proposal(&self) -> &crate::source_command::PreparedCommand {
        self.proposal
    }
    pub(crate) fn source_path(&self) -> &RelativePath {
        &self.source_path
    }
    pub(crate) fn record_id(&self) -> &str {
        self.record_id
    }
    pub(crate) fn updates(&self) -> &[&crate::source_command::SourceChange; 3] {
        &self.updates
    }
    pub(crate) fn member_updates(&self) -> &[SourceEntryMemberDeltaV1; 3] {
        &self.member_updates
    }
    pub(crate) fn dependency_updates(&self) -> &[SourceEntryDependencyDeltaV1; 3] {
        &self.dependency_updates
    }
    pub(crate) fn object_refcount_updates(&self) -> &[SourceEntryObjectRefcountDeltaV1] {
        &self.object_refcount_updates
    }
    pub(crate) fn member_count_after(&self) -> u64 {
        self.member_count_after
    }
    pub(crate) fn source_bytes_after(&self) -> u64 {
        self.source_bytes_after
    }
    pub(crate) fn identity_count_after(&self) -> u64 {
        self.identity_count_after
    }
    pub(crate) fn dependency_source_count_after(&self) -> u64 {
        self.dependency_source_count_after
    }
    pub(crate) fn dependency_count_after(&self) -> u64 {
        self.dependency_count_after
    }
    pub(crate) fn retirement_count_after(&self) -> u64 {
        self.retirement_count_after
    }
    pub(crate) fn prepared_schema(
        &self,
    ) -> tos_validation::source_cut::CutPreparedSchemaExecutionBinding {
        self.prepared_schema
    }

    /// Recheck the captured exact readset against the latest current root and
    /// merge the bounded delta onto that root. This produces the only result
    /// from which the incremental route may issue a successor completion
    /// proof; callers cannot carry stale absolute counts across unrelated
    /// accepted writers.
    pub(crate) fn validate_current_successor<'delta>(
        &'delta self,
        reader: &mut dyn SourceEntryDeltaIndexReader,
        current_base: SelectedNativeAdmissionRootV1,
    ) -> io::Result<ValidatedSourceEntrySuccessorV1<'delta, 'proposal>> {
        let current = reader.verify_readset_current()?;
        let readset = reader.readset();
        if current.format != SourceCutFormat::NativeAdmissionV2
            || current.current_revision != current_base.revision
            || current.rootset_sha256 != Some(current_base.rootset_sha256)
            || readset.base_revision != self.observed_base_revision
            || current_base.validator_sha256 != self.selected_base.validator_sha256
            || current_base.completion_proof.prepared_schema() != self.prepared_schema
            || current_base.member_count == 0
        {
            return Err(invalid(
                "source-entry delta cannot be rebased onto the latest current root",
            ));
        }

        for update in &self.member_updates {
            let witness = readset_member(readset, &update.path)?;
            if witness.expected_presence != Some(SourcePresenceV1::File)
                || witness.expected_member != Some(update.before)
                || witness.expected_indexed_ids.is_none()
            {
                return Err(invalid(
                    "source-entry delta member changed before latest-root rebase",
                ));
            }
        }
        for update in &self.dependency_updates {
            let mut witnesses = readset
                .dependencies
                .iter()
                .filter(|witness| witness.path == update.source);
            if witnesses
                .next()
                .is_none_or(|witness| witness.expected_targets != update.before)
                || witnesses.next().is_some()
            {
                return Err(invalid(
                    "source-entry dependencies changed before latest-root rebase",
                ));
            }
        }
        for update in &self.object_refcount_updates {
            validate_delta_object_refcount_lookup(readset, update.digest, update.before_count)?;
        }

        let mut source_bytes_after = current_base.source_bytes;
        let mut dependency_count_after = current_base.dependency_count;
        let mut dependency_source_count_after = current_base.dependency_source_count;
        for update in &self.member_updates {
            source_bytes_after = source_bytes_after
                .checked_sub(update.before.size_bytes)
                .and_then(|bytes| bytes.checked_add(update.after.size_bytes))
                .ok_or_else(|| invalid("rebased source byte count overflow"))?;
        }
        for update in &self.dependency_updates {
            let before_count = u64::try_from(update.before.len())
                .map_err(|_| invalid("rebased prior dependency count overflow"))?;
            let after_count = u64::try_from(update.after.len())
                .map_err(|_| invalid("rebased successor dependency count overflow"))?;
            dependency_count_after = dependency_count_after
                .checked_sub(before_count)
                .and_then(|count| count.checked_add(after_count))
                .ok_or_else(|| invalid("rebased dependency count overflow"))?;
            match (before_count != 0, after_count != 0) {
                (false, true) => {
                    dependency_source_count_after = dependency_source_count_after
                        .checked_add(1)
                        .ok_or_else(|| invalid("rebased dependency source count overflow"))?;
                }
                (true, false) => {
                    dependency_source_count_after = dependency_source_count_after
                        .checked_sub(1)
                        .ok_or_else(|| invalid("rebased dependency source count underflow"))?;
                }
                _ => {}
            }
        }
        if dependency_source_count_after > dependency_count_after {
            return Err(invalid("rebased dependency counts are inconsistent"));
        }
        Ok(ValidatedSourceEntrySuccessorV1 {
            delta: self,
            current_base,
            current_selection: current,
            member_count_after: current_base.member_count,
            source_bytes_after,
            identity_count_after: current_base.identity_count,
            dependency_source_count_after,
            dependency_count_after,
            retirement_count_after: current_base.retirement_count,
        })
    }
}

#[derive(Debug)]
pub(crate) struct ValidatedSourceEntrySuccessorV1<'delta, 'proposal> {
    delta: &'delta ValidatedSourceEntryDeltaV1<'proposal>,
    current_base: SelectedNativeAdmissionRootV1,
    current_selection: SourceCutSelection,
    member_count_after: u64,
    source_bytes_after: u64,
    identity_count_after: u64,
    dependency_source_count_after: u64,
    dependency_count_after: u64,
    retirement_count_after: u64,
}

impl<'delta, 'proposal> ValidatedSourceEntrySuccessorV1<'delta, 'proposal> {
    pub(crate) fn delta(&self) -> &ValidatedSourceEntryDeltaV1<'proposal> {
        self.delta
    }
    pub(crate) fn current_base(&self) -> SelectedNativeAdmissionRootV1 {
        self.current_base
    }
    pub(crate) fn current_selection(&self) -> SourceCutSelection {
        self.current_selection
    }
    pub(crate) fn member_count_after(&self) -> u64 {
        self.member_count_after
    }
    pub(crate) fn source_bytes_after(&self) -> u64 {
        self.source_bytes_after
    }
    pub(crate) fn identity_count_after(&self) -> u64 {
        self.identity_count_after
    }
    pub(crate) fn dependency_source_count_after(&self) -> u64 {
        self.dependency_source_count_after
    }
    pub(crate) fn dependency_count_after(&self) -> u64 {
        self.dependency_count_after
    }
    pub(crate) fn retirement_count_after(&self) -> u64 {
        self.retirement_count_after
    }

    pub(crate) fn completion_proof(
        &self,
    ) -> crate::source_admission_spooled_index::NativeAdmissionCompletionProofV1 {
        crate::source_admission_spooled_index::NativeAdmissionCompletionProofV1::from_validated_source_entry_successor(self)
    }
}

pub const RETIREMENT_SCHEMA: &str = "ToS/contracts/provenance-event.schema.json";
pub const MAX_EVENT_BYTES: usize = 1_048_576;
const STRUCTURED_JSON_BYTES: u64 = 16 * 1024 * 1024;

/// Logical dependency row codec for the SAME source-index kernel. A valid row
/// is not a completed native root, publication fence, or accepted delta.
/// The mechanical tree key space distinguishes forward and reverse ordering.
#[derive(Clone, Copy)]
pub(crate) enum NativeDependencyDirectionV1 {
    Forward,
    Reverse,
}
#[derive(Clone, Copy)]
pub(crate) struct NativeDependencyRowLimitsV1 {
    pub max_path_bytes: usize,
    pub max_key_bytes: usize,
    pub max_value_bytes: usize,
    pub max_state_bytes: usize,
    pub retained_context_bytes: usize,
}
pub(crate) struct EncodedNativeDependencyRowV1 {
    pub key: Vec<u8>,
    pub value: Vec<u8>,
}
pub(crate) struct BorrowedNativeDependencyRowV1<'a> {
    pub source: &'a str,
    pub target: &'a str,
}
const NATIVE_DEPENDENCY_TAG_V1: &[u8; 8] = b"TOSDEP1\0";

fn dependency_row_state(
    source_bytes: usize,
    target_bytes: usize,
    limits: NativeDependencyRowLimitsV1,
) -> io::Result<(usize, usize)> {
    if [
        limits.max_path_bytes,
        limits.max_key_bytes,
        limits.max_value_bytes,
        limits.max_state_bytes,
    ]
    .iter()
    .any(|n| *n == 0 || *n == usize::MAX)
        || source_bytes == 0
        || target_bytes == 0
        || source_bytes > limits.max_path_bytes
        || target_bytes > limits.max_path_bytes
        || u32::try_from(source_bytes).is_err()
        || u32::try_from(target_bytes).is_err()
    {
        return Err(invalid("native dependency finite row profile"));
    }
    let paths = source_bytes
        .checked_add(target_bytes)
        .ok_or_else(|| invalid("native dependency path size overflow"))?;
    let key = paths
        .checked_add(1)
        .ok_or_else(|| invalid("native dependency key size overflow"))?;
    let value = paths
        .checked_add(16)
        .ok_or_else(|| invalid("native dependency value size overflow"))?;
    // Include path validation clones, both output buffers and fixed framing
    // BEFORE validating paths or reserving any variable allocation.
    let peak = paths
        .checked_mul(2)
        .and_then(|n| n.checked_add(key))
        .and_then(|n| n.checked_add(value))
        .and_then(|n| n.checked_add(limits.retained_context_bytes))
        .and_then(|n| n.checked_add(1024))
        .ok_or_else(|| invalid("native dependency simultaneous state overflow"))?;
    if key > limits.max_key_bytes || value > limits.max_value_bytes || peak > limits.max_state_bytes
    {
        return Err(invalid("native dependency row exceeds selected profile"));
    }
    Ok((key, value))
}

/// Call only from rows emitted by the maintained `insert_dependency` kernel.
/// Keys are S||00||T and T||00||S; normalized ToS paths contain no NUL.
pub(crate) fn encode_native_dependency_row_v1(
    direction: NativeDependencyDirectionV1,
    source: &str,
    target: &str,
    limits: NativeDependencyRowLimitsV1,
    check: &mut dyn FnMut() -> io::Result<()>,
) -> io::Result<EncodedNativeDependencyRowV1> {
    check()?;
    let (key_bytes, value_bytes) = dependency_row_state(source.len(), target.len(), limits)?;
    RelativePath::parse(source).map_err(|_| invalid("native dependency source path"))?;
    RelativePath::parse(target).map_err(|_| invalid("native dependency target path"))?;
    if source == target || source.contains('\0') || target.contains('\0') {
        return Err(invalid("native dependency pair differs from global kernel"));
    }
    let mut key = Vec::new();
    let mut value = Vec::new();
    key.try_reserve_exact(key_bytes)
        .map_err(|_| invalid("native dependency key allocation"))?;
    value
        .try_reserve_exact(value_bytes)
        .map_err(|_| invalid("native dependency value allocation"))?;
    if key.capacity() > key_bytes || value.capacity() > value_bytes {
        return Err(invalid("native dependency allocator exceeds reservation"));
    }
    let (first, second) = match direction {
        NativeDependencyDirectionV1::Forward => (source, target),
        NativeDependencyDirectionV1::Reverse => (target, source),
    };
    for chunk in first.as_bytes().chunks(256) {
        check()?;
        key.extend_from_slice(chunk);
    }
    key.push(0);
    for chunk in second.as_bytes().chunks(256) {
        check()?;
        key.extend_from_slice(chunk);
    }
    value.extend_from_slice(NATIVE_DEPENDENCY_TAG_V1);
    value.extend_from_slice(&(source.len() as u32).to_be_bytes());
    value.extend_from_slice(&(target.len() as u32).to_be_bytes());
    for path in [source, target] {
        for chunk in path.as_bytes().chunks(256) {
            check()?;
            value.extend_from_slice(chunk);
        }
    }
    check()?;
    Ok(EncodedNativeDependencyRowV1 { key, value })
}

/// Export each actual global-kernel edge once in native key order. The view
/// exists only after the full validator completed; this function grants no new
/// authority. The caller reserves its retained root-writer/cache state in limits.
pub(crate) fn for_each_native_dependency_row_v1(
    view: &crate::source_admission_spooled_index::IndexView<'_>,
    direction: NativeDependencyDirectionV1,
    limits: NativeDependencyRowLimitsV1,
    check: &mut dyn FnMut() -> io::Result<()>,
    visit: &mut dyn FnMut(&[u8], &[u8]) -> io::Result<()>,
) -> io::Result<u64> {
    check()?;
    view.verify_candidate()?;
    let mut after: Option<(RelativePath, RelativePath)> = None;
    let mut rows = 0u64;
    loop {
        check()?;
        let pair = view.dependency_pair_after(
            direction,
            after.as_ref().map(|(source, target)| (source, target)),
        )?;
        // SQLite charged the old cursor and new tuple overlap before allocating
        // its result. Release the old cursor before encoding the new pair.
        drop(after.take());
        let Some((source, target)) = pair else {
            break;
        };
        let tuple_state = source
            .as_str()
            .len()
            .checked_add(target.as_str().len())
            .and_then(|bytes| bytes.checked_mul(16))
            .and_then(|bytes| bytes.checked_add(2048))
            .ok_or_else(|| invalid("native dependency export tuple state overflow"))?;
        let mut row_limits = limits;
        row_limits.retained_context_bytes = limits
            .retained_context_bytes
            .checked_add(tuple_state)
            .ok_or_else(|| invalid("native dependency export retained state overflow"))?;
        let row = encode_native_dependency_row_v1(
            direction,
            source.as_str(),
            target.as_str(),
            row_limits,
            check,
        )?;
        rows = rows
            .checked_add(1)
            .filter(|rows| *rows <= view.dependency_count())
            .ok_or_else(|| invalid("native dependency export exceeds verified edge count"))?;
        visit(&row.key, &row.value)?;
        drop(row);
        after = Some((source, target));
    }
    if rows != view.dependency_count() {
        return Err(invalid("native dependency export omitted verified edges"));
    }
    view.verify_candidate()?;
    check()?;
    Ok(rows)
}

/// Borrowed physical-tree value validation: exact framing, normalized paths,
/// direction/key relation and true raw EOF precede a retirement dependency use.
pub(crate) fn decode_native_dependency_row_v1<'a>(
    direction: NativeDependencyDirectionV1,
    key: &[u8],
    raw: &'a [u8],
    limits: NativeDependencyRowLimitsV1,
    check: &mut dyn FnMut() -> io::Result<()>,
) -> io::Result<BorrowedNativeDependencyRowV1<'a>> {
    check()?;
    if raw.len() < 16 || raw.get(..8) != Some(NATIVE_DEPENDENCY_TAG_V1.as_slice()) {
        return Err(invalid("native dependency value version/framing"));
    }
    let source_len = u32::from_be_bytes(raw[8..12].try_into().unwrap()) as usize;
    let target_len = u32::from_be_bytes(raw[12..16].try_into().unwrap()) as usize;
    let (key_bytes, value_bytes) = dependency_row_state(source_len, target_len, limits)?;
    if raw.len() != value_bytes || key.len() != key_bytes {
        return Err(invalid("native dependency value or key EOF differs"));
    }
    let source = std::str::from_utf8(&raw[16..16 + source_len])
        .map_err(|_| invalid("native dependency source encoding"))?;
    let target = std::str::from_utf8(&raw[16 + source_len..])
        .map_err(|_| invalid("native dependency target encoding"))?;
    RelativePath::parse(source).map_err(|_| invalid("native dependency source path"))?;
    RelativePath::parse(target).map_err(|_| invalid("native dependency target path"))?;
    if source == target || source.contains('\0') || target.contains('\0') {
        return Err(invalid("native dependency pair differs from global kernel"));
    }
    let (first, second) = match direction {
        NativeDependencyDirectionV1::Forward => (source, target),
        NativeDependencyDirectionV1::Reverse => (target, source),
    };
    if key.get(..first.len()) != Some(first.as_bytes())
        || key.get(first.len()) != Some(&0)
        || key.get(first.len() + 1..) != Some(second.as_bytes())
    {
        return Err(invalid(
            "native dependency typed key/value relation differs",
        ));
    }
    check()?;
    Ok(BorrowedNativeDependencyRowV1 { source, target })
}

/// All closures use ONE caller-owned cumulative byte/state/deadline budget.
/// `read` verifies length/digest against membership before returning bytes;
/// `verify_member` streams the same verification without retaining a large
/// review document. Neither may resolve a mutable checkout pathname.
pub struct CandidateInput<'a> {
    pub members: &'a BTreeMap<String, Value>,
    pub read: &'a mut dyn FnMut(&str, usize) -> io::Result<Vec<u8>>,
    pub verify_member: &'a mut dyn FnMut(&str) -> io::Result<()>,
    pub check: &'a mut dyn FnMut() -> io::Result<()>,
    pub json: JsonLimits,
    /// Per-document Foundation parser workspace, reserved independently from
    /// raw/decoded bytes and the growing retained source index.
    pub json_state_bytes: usize,
}

/// Bounded path and member lookups shared by resident and disk-backed index
/// storage. The streamed implementation uses only the sealed candidate's
/// typed metadata and held-object reads; no caller-supplied iterator is used.
pub(crate) trait CandidateIndexInput {
    fn tick(&mut self) -> io::Result<()>;
    fn check_row_state(&self, _bytes: usize) -> io::Result<()> {
        Ok(())
    }
    fn row_state_limit(&self) -> usize {
        usize::MAX
    }
    fn member(&mut self, path: &str) -> io::Result<bool>;
    fn member_after(&mut self, after: Option<&str>) -> io::Result<Option<String>>;
    fn member_size(&mut self, path: &str) -> io::Result<u64>;
    fn member_digest(&mut self, path: &str) -> io::Result<Digest256>;
    fn read_raw(&mut self, path: &str, cap: usize) -> io::Result<Vec<u8>>;
    fn verify_member(&mut self, path: &str) -> io::Result<()>;
    fn json_limits(&self) -> JsonLimits;
    fn json_state_bytes(&self) -> usize;

    fn bytes(&mut self, path: &str, cap: usize) -> io::Result<Vec<u8>> {
        self.tick()?;
        if !self.member(path)? {
            return Err(invalid("source companion is missing"));
        }
        let size = self.member_size(path)?;
        if size > cap as u64 {
            return Err(invalid("source companion exceeds its bounded size"));
        }
        let digest = self.member_digest(path)?;
        let raw = self.read_raw(path, cap)?;
        self.tick()?;
        if raw.len() as u64 != size || Digest256::of_bytes(&raw) != digest {
            return Err(invalid(
                "candidate source bytes differ from exact membership",
            ));
        }
        Ok(raw)
    }
}
/// Constructed only by the caller receiving its complete sealed FND report.
/// Catalog rows are from that same fresh render, not an existing catalog file.
pub struct FreshRows {
    pub records: Vec<Value>,
    pub claims: Vec<Value>,
    pub native_semantic: BTreeMap<String, Vec<String>>,
}

/// A replayable, one-row-at-a-time source for the fresh native catalog facts.
///
/// The resident validator adapts its existing vectors without changing their
/// behavior. A bounded provider can instead read each exact callback row from
/// an already-reserved spool, validate its EOF/count, and keep at most one
/// decoded row live while the shared source-index law consumes it.
pub(crate) trait FreshIndexRows {
    fn record_count(&self) -> io::Result<u64>;
    fn claim_count(&self) -> io::Result<u64>;
    fn native_semantic_count(&self) -> io::Result<u64>;
    fn native_semantic_row_count(&self) -> io::Result<u64>;
    fn for_each_record(
        &self,
        visit: &mut dyn FnMut(&str, &str) -> io::Result<()>,
    ) -> io::Result<()>;
    fn for_each_claim(&self, visit: &mut dyn FnMut(&str, &str) -> io::Result<()>)
    -> io::Result<()>;
    fn for_each_native_semantic(
        &self,
        visit: &mut dyn FnMut(&str, &str, bool) -> io::Result<()>,
    ) -> io::Result<()>;
}

/// Optional bounded producer interface for a catalog renderer. The store is
/// reserved by the current candidate invocation; callers give it only the two
/// exact identity bindings extracted by the maintained renderer callback.
pub(crate) trait FreshIndexRowsWriter {
    fn push_record(&mut self, id: &str, source_ref: &str) -> io::Result<()>;
    fn push_claim(&mut self, id: &str, source_ref: &str) -> io::Result<()>;
    fn push_native_semantic(&mut self, id: &str, path: &str) -> io::Result<()>;
}

impl FreshIndexRows for FreshRows {
    fn record_count(&self) -> io::Result<u64> {
        Ok(self.records.len() as u64)
    }

    fn claim_count(&self) -> io::Result<u64> {
        Ok(self.claims.len() as u64)
    }

    fn native_semantic_count(&self) -> io::Result<u64> {
        Ok(self.native_semantic.len() as u64)
    }

    fn native_semantic_row_count(&self) -> io::Result<u64> {
        self.native_semantic
            .values()
            .try_fold(0u64, |count, paths| {
                let rows = u64::try_from(paths.len())
                    .map_err(|_| invalid("fresh semantic source row count overflow"))?;
                count
                    .checked_add(rows)
                    .ok_or_else(|| invalid("fresh semantic source row count overflow"))
            })
    }

    fn for_each_record(
        &self,
        visit: &mut dyn FnMut(&str, &str) -> io::Result<()>,
    ) -> io::Result<()> {
        for row in &self.records {
            visit(string(row, "record_id")?, string(row, "source_record_ref")?)?;
        }
        Ok(())
    }

    fn for_each_claim(
        &self,
        visit: &mut dyn FnMut(&str, &str) -> io::Result<()>,
    ) -> io::Result<()> {
        for row in &self.claims {
            visit(
                string(row, "claim_id")?,
                string(row, "source_claim_file_ref")?,
            )?;
        }
        Ok(())
    }

    fn for_each_native_semantic(
        &self,
        visit: &mut dyn FnMut(&str, &str, bool) -> io::Result<()>,
    ) -> io::Result<()> {
        for (id, refs) in &self.native_semantic {
            for (position, path) in refs.iter().enumerate() {
                visit(id, path, position == 0)?;
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Index {
    pub identities: BTreeMap<String, String>,
    pub dependencies: BTreeMap<String, Vec<String>>,
}

/// One prior identity path, represented by the original resident string or an
/// already bounded streamed relative path. Keeping the latter avoids cloning
/// a second owned string after the base lookup's preallocation check.
pub(crate) enum BaseIdentityPath {
    Resident(String),
    Streamed(RelativePath),
}

impl BaseIdentityPath {
    fn as_str(&self) -> &str {
        match self {
            Self::Resident(path) => path,
            Self::Streamed(path) => path.as_str(),
        }
    }
}

#[derive(Clone, Copy)]
pub struct IndexLimits {
    pub max_edges: usize,
    pub max_state_bytes: usize,
}
/// Program-selected native schema execution. Both raw inputs are candidate
/// bound. None means valid; Some is the actual first schema diagnostic.
pub type SchemaCheck<'a> = dyn FnMut(&str, &[u8], &[u8]) -> io::Result<Option<String>> + 'a;

fn invalid(message: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_string())
}
fn decode_budget() -> io::Error {
    io::Error::new(
        io::ErrorKind::FileTooLarge,
        "JSON decoding byte budget exceeded",
    )
}
fn field<'a>(value: &'a Value, key: &str) -> io::Result<&'a Value> {
    value
        .as_object()
        .and_then(|v| v.get(key))
        .ok_or_else(|| invalid(format!("source index input lacks {key}")))
}
fn text(value: &Value) -> io::Result<&str> {
    value
        .as_str()
        .ok_or_else(|| invalid("source index string field is invalid"))
}
fn string<'a>(value: &'a Value, key: &str) -> io::Result<&'a str> {
    text(field(value, key)?)
}
fn array(value: &Value) -> io::Result<&[Value]> {
    value
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| invalid("source index array field is invalid"))
}
fn member_size(value: &Value) -> io::Result<u64> {
    field(value, "size_bytes")?
        .as_u64()
        .ok_or_else(|| invalid("candidate member size is invalid"))
}

fn owned_text_state_upper_bound(bytes: usize) -> io::Result<usize> {
    // Match the existing conservative row-state multiplier and include
    // allocator/string headers. Callers use this for short-lived copies while
    // the backend row and cursor are still live.
    bytes
        .checked_mul(16)
        .and_then(|n| n.checked_add(2048))
        .ok_or_else(|| invalid("native source owned-text state overflow"))
}

fn lookup_allowance(
    input: &dyn CandidateIndexInput,
    state: &IndexAccounting,
    held_bytes: usize,
) -> io::Result<usize> {
    input.check_row_state(held_bytes)?;
    let candidate_remaining = input
        .row_state_limit()
        .checked_sub(held_bytes)
        .ok_or_else(|| invalid("native source candidate row state exceeded"))?;
    Ok(state
        .remaining_temporary(held_bytes)?
        .min(candidate_remaining))
}
impl CandidateInput<'_> {
    fn tick(&mut self) -> io::Result<()> {
        (self.check)()
    }
    fn bytes(&mut self, path: &str, cap: usize) -> io::Result<Vec<u8>> {
        self.tick()?;
        let members = self.members;
        let member = members
            .get(path)
            .ok_or_else(|| invalid("source companion is missing"))?;
        let size = member_size(member)?;
        if size > cap as u64 {
            return Err(invalid("source companion exceeds its bounded size"));
        }
        let raw = (self.read)(path, cap)?;
        self.tick()?;
        if raw.len() as u64 != size
            || Digest256::of_bytes(&raw).to_hex() != string(member, "sha256")?
        {
            return Err(invalid(
                "candidate source bytes differ from exact membership",
            ));
        }
        Ok(raw)
    }
}

impl CandidateIndexInput for CandidateInput<'_> {
    fn tick(&mut self) -> io::Result<()> {
        CandidateInput::tick(self)
    }

    fn member(&mut self, path: &str) -> io::Result<bool> {
        Ok(self.members.contains_key(path))
    }

    fn member_after(&mut self, after: Option<&str>) -> io::Result<Option<String>> {
        Ok(match after {
            Some(path) => self
                .members
                .range::<str, _>((Bound::Excluded(path), Bound::Unbounded))
                .next()
                .map(|(path, _)| path.clone()),
            None => self.members.keys().next().cloned(),
        })
    }

    fn member_size(&mut self, path: &str) -> io::Result<u64> {
        member_size(
            self.members
                .get(path)
                .ok_or_else(|| invalid("source companion is missing"))?,
        )
    }

    fn member_digest(&mut self, path: &str) -> io::Result<Digest256> {
        let raw = string(
            self.members
                .get(path)
                .ok_or_else(|| invalid("source companion is missing"))?,
            "sha256",
        )?;
        let digest = Digest256::from_hex(raw).map_err(invalid)?;
        if digest.to_hex() != raw {
            return Err(invalid("candidate member digest is not canonical"));
        }
        Ok(digest)
    }

    fn read_raw(&mut self, path: &str, cap: usize) -> io::Result<Vec<u8>> {
        (self.read)(path, cap)
    }

    fn verify_member(&mut self, path: &str) -> io::Result<()> {
        (self.verify_member)(path)
    }

    fn json_limits(&self) -> JsonLimits {
        self.json
    }

    fn json_state_bytes(&self) -> usize {
        self.json_state_bytes
    }
}

pub(crate) struct IndexAccounting {
    limits: IndexLimits,
    bytes: usize,
    edges: usize,
}
impl IndexAccounting {
    fn new(limits: IndexLimits) -> io::Result<Self> {
        if limits.max_edges == 0
            || limits.max_edges == usize::MAX
            || limits.max_state_bytes == 0
            || limits.max_state_bytes == usize::MAX
        {
            return Err(invalid("invalid source index limits"));
        }
        Ok(Self {
            limits,
            bytes: 0,
            edges: 0,
        })
    }
    fn reserve(&mut self, bytes: usize, edge: bool) -> io::Result<()> {
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .filter(|n| *n <= self.limits.max_state_bytes)
            .ok_or_else(|| invalid("source index state budget exceeded"))?;
        if edge {
            self.edges = self
                .edges
                .checked_add(1)
                .filter(|n| *n <= self.limits.max_edges)
                .ok_or_else(|| invalid("source index edge budget exceeded"))?;
        }
        Ok(())
    }
    fn check_temporary(&self, bytes: usize) -> io::Result<()> {
        self.bytes
            .checked_add(bytes)
            .filter(|n| *n <= self.limits.max_state_bytes)
            .ok_or_else(|| invalid("source index temporary state budget exceeded"))?;
        Ok(())
    }
    fn remaining_temporary(&self, bytes: usize) -> io::Result<usize> {
        self.limits
            .max_state_bytes
            .checked_sub(self.bytes)
            .and_then(|remaining| remaining.checked_sub(bytes))
            .ok_or_else(|| invalid("source index temporary state budget exceeded"))
    }
    fn reserve_edge_count(&mut self) -> io::Result<()> {
        self.edges = self
            .edges
            .checked_add(1)
            .filter(|n| *n <= self.limits.max_edges)
            .ok_or_else(|| invalid("source index edge budget exceeded"))?;
        Ok(())
    }
}

/// Storage boundary for the one maintained global identity/link kernel.
/// The resident adapter retains the historic `Index` maps. The spooled FND
/// adapter stores the same sorted relations in its private quota-bound SQLite
/// database and returns a cursor-only view after EOF.
pub(crate) trait AdmissionIndexBackend {
    fn retains_index_rows(&self) -> bool;
    fn source_record_count(&self) -> io::Result<u64>;
    fn source_claim_count(&self) -> io::Result<u64>;
    fn source_native_semantic_count(&self) -> io::Result<u64>;
    fn source_native_semantic_row_count(&self) -> io::Result<u64>;
    fn for_each_source_record(
        &mut self,
        visit: &mut dyn FnMut(&mut dyn AdmissionIndexBackend, &str, &str) -> io::Result<()>,
    ) -> io::Result<()>;
    fn for_each_source_claim(
        &mut self,
        visit: &mut dyn FnMut(&mut dyn AdmissionIndexBackend, &str, &str) -> io::Result<()>,
    ) -> io::Result<()>;
    fn for_each_source_native_semantic(
        &mut self,
        visit: &mut dyn for<'row> FnMut(NativeSemanticStep<'row>) -> io::Result<()>,
    ) -> io::Result<()>;
    fn identity_path(
        &mut self,
        id: &str,
        max_owned_state_bytes: usize,
    ) -> io::Result<Option<String>>;
    fn insert_identity(&mut self, id: &str, path: &str) -> io::Result<()>;
    fn dependency_source_exists(&mut self, source: &str) -> io::Result<bool>;
    fn dependency_exists(&mut self, source: &str, target: &str) -> io::Result<bool>;
    fn insert_dependency(&mut self, source: &str, target: &str) -> io::Result<()>;
}

pub(crate) enum NativeSemanticStep<'row> {
    Preflight {
        cursor_id_bytes: usize,
        id_bytes: usize,
        path_bytes: usize,
    },
    Row {
        backend: &'row mut dyn AdmissionIndexBackend,
        cursor_id_bytes: usize,
        id: &'row str,
        path: &'row str,
        group_first: bool,
    },
}

pub(crate) struct ResidentIndexBackend<'rows> {
    pub(crate) index: Index,
    dependencies: BTreeMap<String, BTreeSet<String>>,
    fresh: &'rows dyn FreshIndexRows,
}

impl<'rows> ResidentIndexBackend<'rows> {
    fn new(fresh: &'rows dyn FreshIndexRows) -> Self {
        Self {
            index: Index::default(),
            dependencies: BTreeMap::new(),
            fresh,
        }
    }

    fn finish(mut self) -> Index {
        self.index.dependencies = self
            .dependencies
            .into_iter()
            .map(|(source, targets)| (source, targets.into_iter().collect()))
            .collect();
        self.index
    }
}

impl AdmissionIndexBackend for ResidentIndexBackend<'_> {
    fn retains_index_rows(&self) -> bool {
        true
    }

    fn source_record_count(&self) -> io::Result<u64> {
        self.fresh.record_count()
    }

    fn source_claim_count(&self) -> io::Result<u64> {
        self.fresh.claim_count()
    }

    fn source_native_semantic_count(&self) -> io::Result<u64> {
        self.fresh.native_semantic_count()
    }

    fn source_native_semantic_row_count(&self) -> io::Result<u64> {
        self.fresh.native_semantic_row_count()
    }

    fn for_each_source_record(
        &mut self,
        visit: &mut dyn FnMut(&mut dyn AdmissionIndexBackend, &str, &str) -> io::Result<()>,
    ) -> io::Result<()> {
        let fresh = self.fresh;
        fresh.for_each_record(&mut |id, source_ref| visit(self, id, source_ref))
    }

    fn for_each_source_claim(
        &mut self,
        visit: &mut dyn FnMut(&mut dyn AdmissionIndexBackend, &str, &str) -> io::Result<()>,
    ) -> io::Result<()> {
        let fresh = self.fresh;
        fresh.for_each_claim(&mut |id, source_ref| visit(self, id, source_ref))
    }

    fn for_each_source_native_semantic(
        &mut self,
        visit: &mut dyn for<'row> FnMut(NativeSemanticStep<'row>) -> io::Result<()>,
    ) -> io::Result<()> {
        let fresh = self.fresh;
        fresh.for_each_native_semantic(&mut |id, path, group_first| {
            visit(NativeSemanticStep::Preflight {
                cursor_id_bytes: 0,
                id_bytes: id.len(),
                path_bytes: path.len(),
            })?;
            visit(NativeSemanticStep::Row {
                backend: self,
                cursor_id_bytes: 0,
                id,
                path,
                group_first,
            })
        })
    }

    fn identity_path(
        &mut self,
        id: &str,
        max_owned_state_bytes: usize,
    ) -> io::Result<Option<String>> {
        let Some(path) = self.index.identities.get(id) else {
            return Ok(None);
        };
        let bytes = path
            .len()
            .checked_mul(16)
            .and_then(|n| n.checked_add(2048))
            .ok_or_else(|| invalid("source identity lookup state overflow"))?;
        if bytes > max_owned_state_bytes {
            return Err(invalid("source identity lookup exceeds state budget"));
        }
        Ok(Some(path.clone()))
    }

    fn insert_identity(&mut self, id: &str, path: &str) -> io::Result<()> {
        self.index.identities.insert(id.to_owned(), path.to_owned());
        Ok(())
    }

    fn dependency_source_exists(&mut self, source: &str) -> io::Result<bool> {
        Ok(self.dependencies.contains_key(source))
    }

    fn dependency_exists(&mut self, source: &str, target: &str) -> io::Result<bool> {
        Ok(self
            .dependencies
            .get(source)
            .is_some_and(|targets| targets.contains(target)))
    }

    fn insert_dependency(&mut self, source: &str, target: &str) -> io::Result<()> {
        self.dependencies
            .entry(source.to_owned())
            .or_default()
            .insert(target.to_owned());
        Ok(())
    }
}

fn bind(
    input: &mut dyn CandidateIndexInput,
    backend: &mut dyn AdmissionIndexBackend,
    state: &mut IndexAccounting,
    id: &str,
    path: &str,
) -> io::Result<()> {
    input.tick()?;
    let held_bytes = owned_text_state_upper_bound(
        id.len()
            .checked_add(path.len())
            .ok_or_else(|| invalid("source identity row state overflow"))?,
    )?;
    input.check_row_state(held_bytes)?;
    if !input.member(path)? {
        return Err(invalid(
            "source identity is outside the admitted member set",
        ));
    }
    let max_owned_state_bytes = lookup_allowance(input, state, held_bytes)?;
    if let Some(old) = backend.identity_path(id, max_owned_state_bytes)? {
        if old != path {
            return Err(invalid("duplicate source identity"));
        }
        return Ok(());
    }
    if backend.retains_index_rows() {
        state.reserve(
            id.len()
                .checked_add(path.len())
                .and_then(|n| n.checked_add(96))
                .ok_or_else(|| invalid("source identity state overflow"))?,
            false,
        )?;
    }
    backend.insert_identity(id, path)
}
fn edge(
    input: &mut dyn CandidateIndexInput,
    backend: &mut dyn AdmissionIndexBackend,
    state: &mut IndexAccounting,
    source: &str,
    target: &str,
) -> io::Result<()> {
    if source == target {
        return Ok(());
    }
    input.check_row_state(
        source
            .len()
            .checked_add(target.len())
            .and_then(|n| n.checked_add(1024))
            .ok_or_else(|| invalid("source dependency row state overflow"))?,
    )?;
    if backend.dependency_exists(source, target)? {
        return Ok(());
    }
    let source_exists = backend.dependency_source_exists(source)?;
    if backend.retains_index_rows() {
        if !source_exists {
            state.reserve(
                source
                    .len()
                    .checked_add(96)
                    .ok_or_else(|| invalid("source dependency state overflow"))?,
                false,
            )?;
        }
        state.reserve(
            target
                .len()
                .checked_add(64)
                .ok_or_else(|| invalid("source dependency state overflow"))?,
            true,
        )?;
    } else {
        state.reserve_edge_count()?;
    }
    backend.insert_dependency(source, target)
}
/// Python json.loads(bytes) observes BOM / zero-pattern UTF-16 and UTF-32.
/// Raw digest custody precedes decoding. This does not normalize authored text.
fn json_encoding(raw: &[u8], cap: usize) -> io::Result<Vec<u8>> {
    if raw.len() > cap {
        return Err(decode_budget());
    }
    let (skip, width, little) = if raw.starts_with(&[0, 0, 0xfe, 0xff]) {
        (4, 4, false)
    } else if raw.starts_with(&[0xff, 0xfe, 0, 0]) {
        (4, 4, true)
    } else if raw.starts_with(&[0xfe, 0xff]) {
        (2, 2, false)
    } else if raw.starts_with(&[0xff, 0xfe]) {
        (2, 2, true)
    } else if raw.starts_with(&[0xef, 0xbb, 0xbf]) {
        return utf8_surrogatepass(&raw[3..], cap);
    } else if raw.len() >= 4 && raw[0] == 0 && raw[1] == 0 && raw[2] == 0 {
        (0, 4, false)
    } else if raw.len() >= 4 && raw[1] == 0 && raw[2] == 0 && raw[3] == 0 {
        (0, 4, true)
    } else if raw.len() >= 2 && raw[0] == 0 {
        (0, 2, false)
    } else if raw.len() >= 2 && raw[1] == 0 {
        (0, 2, true)
    } else {
        return utf8_surrogatepass(raw, cap);
    };
    let bytes = &raw[skip..];
    if bytes.len() % width != 0 {
        return Err(invalid("invalid JSON byte encoding"));
    }
    let mut result = String::new();
    if width == 2 {
        let units = bytes.chunks_exact(2).map(|v| {
            if little {
                u16::from_le_bytes([v[0], v[1]])
            } else {
                u16::from_be_bytes([v[0], v[1]])
            }
        });
        // Python's surrogatepass retains lone literal surrogates. Represent them
        // as JSON escapes so the existing FND legacy parser retains WTF-16.
        for value in char::decode_utf16(units) {
            let extra = match &value {
                Ok(c) => c.len_utf8(),
                Err(_) => 6,
            };
            if extra > cap.saturating_sub(result.len()) {
                return Err(decode_budget());
            }
            match value {
                Ok(c) => result.push(c),
                Err(e) => result.push_str(&format!("\\u{:04x}", e.unpaired_surrogate())),
            }
        }
    } else {
        for v in bytes.chunks_exact(4) {
            let n = if little {
                u32::from_le_bytes([v[0], v[1], v[2], v[3]])
            } else {
                u32::from_be_bytes([v[0], v[1], v[2], v[3]])
            };
            let extra = char::from_u32(n).map_or(6, char::len_utf8);
            if extra > cap.saturating_sub(result.len()) {
                return Err(decode_budget());
            }
            if let Some(c) = char::from_u32(n) {
                result.push(c)
            } else if (0xd800..=0xdfff).contains(&n) {
                result.push_str(&format!("\\u{n:04x}"))
            } else {
                return Err(invalid("invalid JSON byte encoding"));
            }
        }
    }
    Ok(result.into_bytes())
}
fn utf8_surrogatepass(mut raw: &[u8], cap: usize) -> io::Result<Vec<u8>> {
    let mut result = Vec::new();
    loop {
        match std::str::from_utf8(raw) {
            Ok(_) => {
                if raw.len() > cap.saturating_sub(result.len()) {
                    return Err(decode_budget());
                }
                result.extend_from_slice(raw);
                break;
            }
            Err(error) => {
                let good = error.valid_up_to();
                if good.saturating_add(6) > cap.saturating_sub(result.len()) {
                    return Err(decode_budget());
                }
                result.extend_from_slice(&raw[..good]);
                raw = &raw[good..];
                if raw.len() < 3
                    || raw[0] != 0xed
                    || !(0xa0..=0xbf).contains(&raw[1])
                    || !(0x80..=0xbf).contains(&raw[2])
                {
                    return Err(invalid("invalid JSON UTF-8 encoding"));
                }
                let unit = ((raw[0] as u16 & 15) << 12)
                    | ((raw[1] as u16 & 63) << 6)
                    | (raw[2] as u16 & 63);
                result.extend_from_slice(format!("\\u{unit:04x}").as_bytes());
                raw = &raw[3..];
            }
        }
    }
    Ok(result)
}
fn document(
    raw: &[u8],
    mode: JsonMode,
    limits: JsonLimits,
    state_bytes: usize,
) -> io::Result<Option<JsonValue>> {
    let decoded = match json_encoding(raw, limits.max_bytes) {
        Ok(v) => v,
        Err(error) if error.kind() == io::ErrorKind::FileTooLarge => return Err(error),
        Err(_) => return Ok(None),
    };
    // Decode expansion is charged to the selected parser's byte limit.
    match parse_json_with_state_budget(&decoded, mode, limits, state_bytes) {
        Ok(v) => Ok(Some(v.into_root())),
        Err(e) if e.code == FoundationErrorCode::BudgetExceeded => Err(invalid(e)),
        Err(_) => Ok(None),
    }
}
fn strict_object(raw: &[u8], limits: JsonLimits) -> io::Result<Value> {
    let decoded = json_encoding(raw, limits.max_bytes)?;
    let value = parse_json(&decoded, JsonMode::PublishedStrict, limits)
        .map_err(|e| match e.code {
            FoundationErrorCode::DuplicateMember => {
                invalid("retirement JSON contains duplicate fields")
            }
            FoundationErrorCode::BudgetExceeded => invalid(e),
            _ => invalid("retirement input is not finite JSON"),
        })?
        .into_root();
    fn convert(v: &JsonValue) -> io::Result<Value> {
        Ok(match v {
            JsonValue::Null => Value::Null,
            JsonValue::Bool(v) => Value::Bool(*v),
            JsonValue::String(v) => Value::String(
                v.as_str()
                    .ok_or_else(|| invalid("retirement input requires scalar strings"))?
                    .to_owned(),
            ),
            JsonValue::Number(n) => {
                if n.lexeme.contains(['.', 'e', 'E'])
                    && !n.lexeme.parse::<f64>().is_ok_and(f64::is_finite)
                {
                    return Err(invalid("retirement input is not finite JSON"));
                }
                Value::Number(n.lexeme.parse().map_err(invalid)?)
            }
            JsonValue::Array(v) => Value::Array(v.iter().map(convert).collect::<io::Result<_>>()?),
            JsonValue::Object(v) => {
                let mut out = serde_json::Map::new();
                for (k, v) in v {
                    out.insert(
                        k.as_str()
                            .ok_or_else(|| invalid("retirement input requires scalar keys"))?
                            .to_owned(),
                        convert(v)?,
                    );
                }
                Value::Object(out)
            }
        })
    }
    let result = convert(&value)?;
    if !result.is_object() {
        return Err(invalid("retirement input must be a JSON object"));
    }
    Ok(result)
}
fn event_limits(input: &dyn CandidateIndexInput) -> JsonLimits {
    let json = input.json_limits();
    JsonLimits {
        max_bytes: MAX_EVENT_BYTES.min(json.max_bytes),
        ..json
    }
}
fn reference_path(value: &str) -> Cow<'_, str> {
    let value = value.split('#').next().unwrap_or(value);
    if let Some((path, tail)) = value.rsplit_once(':') {
        // Regex $ also permits a final LF; all other trailing whitespace stays.
        let tail = tail.strip_suffix('\n').unwrap_or(tail);
        if !tail.is_empty()
            && tail
                .chars()
                .all(tos_foundation::python_decimal_unicode16_v1)
        {
            return if value.ends_with('\n') {
                Cow::Owned(format!("{path}\n"))
            } else {
                Cow::Borrowed(path)
            };
        }
    }
    Cow::Borrowed(value)
}
fn references(
    input: &mut dyn CandidateIndexInput,
    backend: &mut dyn AdmissionIndexBackend,
    state: &mut IndexAccounting,
    source: &str,
    value: &JsonValue,
) -> io::Result<()> {
    input.tick()?;
    match value {
        JsonValue::String(s) => {
            if let Some(s) = s.as_str() {
                let held_bytes = owned_text_state_upper_bound(
                    source
                        .len()
                        .checked_add(s.len())
                        .ok_or_else(|| invalid("source reference state overflow"))?,
                )?;
                let max_owned_state_bytes = lookup_allowance(input, state, held_bytes)?;
                if let Some(target) = backend.identity_path(s, max_owned_state_bytes)? {
                    edge(input, backend, state, source, &target)?;
                }
                if s.starts_with("ToS/") {
                    let p = reference_path(s);
                    if input.member(p.as_ref())? {
                        edge(input, backend, state, source, p.as_ref())?;
                    }
                }
            } else {
                // An unpaired surrogate in a fragment must not erase a valid
                // scalar path before '#'. It cannot match a scalar identity.
                let prefix = s
                    .units()
                    .split(|u| *u == b'#' as u16)
                    .next()
                    .unwrap_or(s.units());
                if let Ok(prefix) = String::from_utf16(prefix) {
                    if prefix.starts_with("ToS/") {
                        let path = reference_path(&prefix);
                        if input.member(path.as_ref())? {
                            edge(input, backend, state, source, path.as_ref())?;
                        }
                    }
                }
            }
        }
        JsonValue::Array(v) => {
            for item in v {
                references(input, backend, state, source, item)?;
            }
        }
        JsonValue::Object(v) => {
            for (_, item) in v {
                references(input, backend, state, source, item)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn add_delta_target<R: SourceEntryDeltaIndexReader + ?Sized>(
    reader: &mut R,
    targets: &mut Vec<RelativePath>,
    target: RelativePath,
    retained_state_bytes: usize,
) -> io::Result<()> {
    reader.tick(targets.len().saturating_add(1))?;
    if targets.contains(&target) {
        return Ok(());
    }
    let next_capacity = targets.capacity().max(targets.len().saturating_add(1));
    let list_state = next_capacity
        .checked_mul(std::mem::size_of::<RelativePath>())
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Vec<RelativePath>>()))
        .and_then(|bytes| {
            targets.iter().try_fold(bytes, |total, path| {
                total.checked_add(path.as_str().len().saturating_mul(4))
            })
        })
        .and_then(|bytes| bytes.checked_add(target.as_str().len().saturating_mul(4)))
        .and_then(|bytes| bytes.checked_add(128))
        .ok_or_else(|| invalid("source-entry dependency delta state overflow"))?;
    reader.check_state(
        retained_state_bytes
            .checked_add(list_state)
            .ok_or_else(|| invalid("source-entry dependency delta state overflow"))?,
    )?;
    targets
        .try_reserve_exact(1)
        .map_err(|_| invalid("source-entry dependency delta allocation"))?;
    reader.check_state(
        retained_state_bytes
            .checked_add(relative_path_vec_state(targets)?)
            .and_then(|bytes| {
                bytes.checked_add(
                    std::mem::size_of::<RelativePath>()
                        .checked_add(target.as_str().len().saturating_mul(4))?,
                )
            })
            .ok_or_else(|| invalid("source-entry dependency delta state overflow"))?,
    )?;
    targets.push(target);
    reader.tick(1)
}

fn validate_delta_identity_lookup(
    readset: &SourceCutReadsetV1,
    id: &str,
    expected_path: Option<&RelativePath>,
) -> io::Result<()> {
    let mut witnesses = readset.identities.iter().filter(|witness| witness.id == id);
    if witnesses
        .next()
        .is_none_or(|witness| witness.expected_path.as_ref() != expected_path)
        || witnesses.next().is_some()
    {
        return Err(invalid(
            "source-entry delta identity lookup lacks one exact readset witness",
        ));
    }
    Ok(())
}

fn validate_delta_identity_target_member(
    readset: &SourceCutReadsetV1,
    id: &str,
    path: &RelativePath,
) -> io::Result<()> {
    let witness = readset_member(readset, path)?;
    let ids = witness
        .expected_indexed_ids
        .as_deref()
        .ok_or_else(|| invalid("source-entry identity target lacks its inverse-set witness"))?;
    if witness.expected_presence != Some(SourcePresenceV1::File)
        || witness.expected_member.is_none()
        || ids.windows(2).any(|pair| pair[0] >= pair[1])
        || ids
            .iter()
            .filter(|candidate| candidate.as_str() == id)
            .count()
            != 1
    {
        return Err(invalid(
            "source-entry identity target does not have an exact inverse-set member witness",
        ));
    }
    Ok(())
}

fn validate_delta_member_lookup(
    readset: &SourceCutReadsetV1,
    path: &RelativePath,
    selected: Option<SourceCutMemberTuple>,
) -> io::Result<()> {
    let witness = readset_member(readset, path)?;
    let exact_file = selected.is_some_and(|member| {
        witness.expected_presence == Some(SourcePresenceV1::File)
            && witness.expected_member == Some(member)
            && witness
                .expected_indexed_ids
                .as_ref()
                .is_some_and(|ids| ids.windows(2).all(|pair| pair[0] < pair[1]))
    });
    let exact_non_file = selected.is_none()
        && witness.expected_member.is_none()
        && witness.expected_indexed_ids.is_none()
        && witness.expected_presence != Some(SourcePresenceV1::File);
    if !exact_file && !exact_non_file {
        return Err(invalid(
            "source-entry delta member lookup lacks one exact positive or negative witness",
        ));
    }
    Ok(())
}

/// Apply the exact global-kernel reference interpretation to one proposed
/// JSON member, using the selected current root for identity and membership
/// lookups. The adapter records every lookup in the same SourceCut readset.
fn collect_source_entry_references<R: SourceEntryDeltaIndexReader + ?Sized>(
    reader: &mut R,
    value: &JsonValue,
    source: &RelativePath,
    changed_paths: &[RelativePath; 3],
    targets: &mut Vec<RelativePath>,
    retained_state_bytes: usize,
) -> io::Result<()> {
    reader.tick(1)?;
    match value {
        JsonValue::String(text) => {
            if let Some(text) = text.as_str() {
                if !text.is_empty() {
                    let identity_target = reader.current_identity_path(text)?;
                    validate_delta_identity_lookup(
                        reader.readset(),
                        text,
                        identity_target.as_ref(),
                    )?;
                    if let Some(target) = identity_target {
                        validate_delta_identity_target_member(reader.readset(), text, &target)?;
                        if target != *source {
                            add_delta_target(reader, targets, target, retained_state_bytes)?;
                        }
                    }
                }
                if text.starts_with("ToS/") {
                    let path_text = reference_path(text);
                    if let Ok(path) = RelativePath::parse(path_text.as_ref()) {
                        let present = if changed_paths.contains(&path) {
                            true
                        } else {
                            let member = reader.current_member(&path)?;
                            validate_delta_member_lookup(reader.readset(), &path, member)?;
                            member.is_some()
                        };
                        if present && path != *source {
                            add_delta_target(reader, targets, path, retained_state_bytes)?;
                        }
                    }
                }
            } else {
                // Match the full-source kernel's scalar-prefix rule for a
                // surrogate-bearing JSON string without treating it as an ID.
                let prefix = text
                    .units()
                    .split(|unit| *unit == b'#' as u16)
                    .next()
                    .unwrap_or(text.units());
                if let Ok(prefix) = String::from_utf16(prefix)
                    && prefix.starts_with("ToS/")
                {
                    let path_text = reference_path(&prefix);
                    if let Ok(path) = RelativePath::parse(path_text.as_ref()) {
                        let present = if changed_paths.contains(&path) {
                            true
                        } else {
                            let member = reader.current_member(&path)?;
                            validate_delta_member_lookup(reader.readset(), &path, member)?;
                            member.is_some()
                        };
                        if present && path != *source {
                            add_delta_target(reader, targets, path, retained_state_bytes)?;
                        }
                    }
                }
            }
        }
        JsonValue::Array(values) => {
            for value in values {
                collect_source_entry_references(
                    reader,
                    value,
                    source,
                    changed_paths,
                    targets,
                    retained_state_bytes,
                )?;
            }
        }
        JsonValue::Object(values) => {
            for (_, value) in values {
                collect_source_entry_references(
                    reader,
                    value,
                    source,
                    changed_paths,
                    targets,
                    retained_state_bytes,
                )?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn readset_member<'a>(
    readset: &'a SourceCutReadsetV1,
    path: &RelativePath,
) -> io::Result<&'a SourceCutMemberWitness> {
    let mut matches = readset
        .members
        .iter()
        .filter(|witness| witness.path == *path);
    let witness = matches
        .next()
        .ok_or_else(|| invalid("source-entry delta member predicate was not retained"))?;
    if matches.next().is_some() {
        return Err(invalid(
            "source-entry delta has duplicate member predicates",
        ));
    }
    Ok(witness)
}

fn validate_delta_object_refcount_lookup(
    readset: &SourceCutReadsetV1,
    digest: Digest256,
    expected_count: Option<u64>,
) -> io::Result<()> {
    if expected_count == Some(0) {
        return Err(invalid(
            "source object refcount zero must be represented by absence",
        ));
    }
    let mut witnesses = readset
        .object_refcounts
        .iter()
        .filter(|witness| witness.digest == digest);
    if witnesses
        .next()
        .is_none_or(|witness| witness.expected_count != expected_count)
        || witnesses.next().is_some()
    {
        return Err(invalid(
            "source-entry object refcount lacks one exact readset witness",
        ));
    }
    Ok(())
}

fn object_refcount_delta_state(
    updates: &Vec<SourceEntryObjectRefcountDeltaV1>,
) -> io::Result<usize> {
    std::mem::size_of::<Vec<SourceEntryObjectRefcountDeltaV1>>()
        .checked_add(
            updates
                .capacity()
                .checked_mul(std::mem::size_of::<SourceEntryObjectRefcountDeltaV1>())
                .ok_or_else(|| invalid("source-entry object-refcount state overflow"))?,
        )
        .ok_or_else(|| invalid("source-entry object-refcount state overflow"))
}

fn relative_path_vec_state(paths: &Vec<RelativePath>) -> io::Result<usize> {
    paths.iter().try_fold(
        std::mem::size_of::<Vec<RelativePath>>()
            .checked_add(
                paths
                    .capacity()
                    .checked_mul(std::mem::size_of::<RelativePath>())
                    .ok_or_else(|| invalid("source-entry path vector state overflow"))?,
            )
            .ok_or_else(|| invalid("source-entry path vector state overflow"))?,
        |total, path| {
            total
                .checked_add(
                    std::mem::size_of::<RelativePath>()
                        .checked_add(path.as_str().len().saturating_mul(4))
                        .ok_or_else(|| invalid("source-entry path text state overflow"))?,
                )
                .ok_or_else(|| invalid("source-entry path vector state overflow"))
        },
    )
}

fn source_entry_delta_state(
    expected_paths: &[RelativePath; 3],
    dependency_updates: &[SourceEntryDependencyDeltaV1],
    member_updates: &[SourceEntryMemberDeltaV1],
    current_before: Option<&Vec<RelativePath>>,
) -> io::Result<usize> {
    let mut bytes = std::mem::size_of::<ValidatedSourceEntryDeltaV1<'_>>()
        .checked_add(
            expected_paths
                .iter()
                .try_fold(0usize, |total, path| {
                    total.checked_add(
                        std::mem::size_of::<RelativePath>()
                            .checked_add(path.as_str().len().saturating_mul(4))?,
                    )
                })
                .ok_or_else(|| invalid("source-entry selected path state overflow"))?,
        )
        .and_then(|total| {
            total.checked_add(
                std::mem::size_of::<Vec<SourceEntryDependencyDeltaV1>>().checked_add(
                    3usize.checked_mul(std::mem::size_of::<SourceEntryDependencyDeltaV1>())?,
                )?,
            )
        })
        .and_then(|total| {
            total.checked_add(
                std::mem::size_of::<Vec<SourceEntryMemberDeltaV1>>().checked_add(
                    3usize.checked_mul(std::mem::size_of::<SourceEntryMemberDeltaV1>())?,
                )?,
            )
        })
        .ok_or_else(|| invalid("source-entry retained delta state overflow"))?;
    for update in dependency_updates {
        bytes = bytes
            .checked_add(update.source.as_str().len().saturating_mul(4))
            .ok_or_else(|| invalid("source-entry dependency result state overflow"))?;
        bytes = bytes
            .checked_add(relative_path_vec_state(&update.before)?)
            .ok_or_else(|| invalid("source-entry dependency result state overflow"))?;
        bytes = bytes
            .checked_add(relative_path_vec_state(&update.after)?)
            .ok_or_else(|| invalid("source-entry dependency result state overflow"))?;
    }
    for update in member_updates {
        bytes = bytes
            .checked_add(update.path.as_str().len().saturating_mul(4))
            .ok_or_else(|| invalid("source-entry member result state overflow"))?;
    }
    bytes
        .checked_add(
            current_before
                .map(relative_path_vec_state)
                .transpose()?
                .unwrap_or(0),
        )
        .and_then(|total| {
            total.checked_add(if current_before.is_some() {
                std::mem::size_of::<Vec<RelativePath>>()
            } else {
                0
            })
        })
        .and_then(|total| total.checked_add(4096))
        .ok_or_else(|| invalid("source-entry retained delta state overflow"))
}

fn validate_source_entry_identity_closure(
    readset: &SourceCutReadsetV1,
    record_path: &RelativePath,
    form_path: &RelativePath,
    history_path: &RelativePath,
    record_id: &str,
) -> io::Result<()> {
    let record_ids = [record_id.to_owned()];
    let no_ids: &[String] = &[];
    for (path, expected_ids) in [
        (record_path, record_ids.as_slice()),
        (form_path, no_ids),
        (history_path, no_ids),
    ] {
        let witness = readset_member(readset, path)?;
        if witness.expected_presence != Some(SourcePresenceV1::File)
            || witness.expected_member.is_none()
            || witness.expected_indexed_ids.as_deref() != Some(expected_ids)
        {
            return Err(invalid(
                "source-entry delta changes or omits an indexed identity path",
            ));
        }
        for id in expected_ids {
            let mut matches = readset
                .identities
                .iter()
                .filter(|item| item.id.as_str() == id.as_str());
            if matches
                .next()
                .is_none_or(|item| item.expected_path.as_ref() != Some(path))
                || matches.next().is_some()
            {
                return Err(invalid("source-entry delta identity inverse is not exact"));
            }
        }
    }
    Ok(())
}

/// Verify and compute the bounded SourceEntry successor index delta. The
/// source handler has already prepared and schema-checked these exact bytes;
/// this function reuses the global kernel's recursive reference semantics and
/// adds every positive/negative root lookup to the retained readset before the
/// caller performs its final currentness check.
#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_source_entry_delta<'a>(
    current: SourceCutSelection,
    selected: SelectedNativeAdmissionRootV1,
    reader: &mut dyn SourceEntryDeltaIndexReader,
    proposal: &'a crate::source_command::PreparedCommand,
    configuration_raw: &[u8],
    source_path: &'a str,
    record_id: &'a str,
    schema: &CutExecutionBinding,
    prepared_schema: CutPreparedSchemaExecutionBinding,
) -> io::Result<ValidatedSourceEntryDeltaV1<'a>> {
    if current.format != SourceCutFormat::NativeAdmissionV2
        || current.current_revision != selected.revision
        || current.rootset_sha256 != Some(selected.rootset_sha256)
        || selected.validator_sha256 != selected.completion_proof.validator_sha256()
        || selected.source_bytes != selected.completion_proof.source_bytes()
        || selected.identity_count != selected.completion_proof.identity_count()
        || selected.dependency_source_count != selected.completion_proof.dependency_source_count()
        || selected.dependency_count != selected.completion_proof.dependency_count()
        || selected.dependency_source_count > selected.dependency_count
        || selected.membership_v1 != selected.completion_proof.membership_v1()
    {
        return Err(invalid(
            "source-entry delta selected root lacks a matching completed baseline",
        ));
    }
    if reader.readset().base_revision != selected.revision
        || proposal.base_revision != reader.readset().base_revision
        || schema.source_revision != reader.readset().base_revision
        || Digest256::of_bytes(configuration_raw) != proposal.configuration_raw_sha256
        || proposal.handler_id != "native-witness-link-selected-revision"
        || !matches!(
            proposal.operation.as_str(),
            "record.revise" | "record.recover"
        )
        || proposal.replayed
        || record_id.is_empty()
        || schema.schema_profile != selected.completion_proof.prepared_schema().schema_profile
        || schema.schema_set_sha256
            != selected
                .completion_proof
                .prepared_schema()
                .schema_set_sha256
        || schema.worker_sha256 != selected.completion_proof.prepared_schema().worker_sha256
        || prepared_schema != selected.completion_proof.prepared_schema()
    {
        return Err(invalid(
            "source-entry delta proposal or prepared schema differs from its baseline",
        ));
    }

    let name_state = std::mem::size_of::<[String; 3]>()
        .checked_add(
            source_path
                .len()
                .checked_mul(12)
                .ok_or_else(|| invalid("source-entry selected name state overflow"))?,
        )
        .and_then(|bytes| bytes.checked_add(512))
        .ok_or_else(|| invalid("source-entry selected name state overflow"))?;
    reader.check_state(name_state)?;
    let mut expected_names = crate::source_revisions::names(source_path)
        .map_err(|_| invalid("source-entry delta protected path is invalid"))?;
    expected_names.sort();
    if proposal.changes.len() != expected_names.len() {
        return Err(invalid(
            "source-entry delta does not contain the exact maintained file set",
        ));
    }
    let (parent, _) = source_path
        .rsplit_once('/')
        .ok_or_else(|| invalid("source-entry delta source path has no parent"))?;
    if !source_path.starts_with("ToS/source-witnesses/")
        || source_path.split('/').count() < 5
        || source_path.split('/').any(|part| {
            matches!(
                part,
                "payload" | "local-content" | "catalog" | "owner-local" | "private" | "retirements"
            ) || part.starts_with('.')
        })
        || !source_path.ends_with(".json")
        || source_path.ends_with(".human-forms.json")
    {
        return Err(invalid(
            "source-entry delta protected path is outside its owner",
        ));
    }
    let mut expected_paths = Vec::new();
    let selected_path_state = std::mem::size_of::<Vec<RelativePath>>()
        .checked_add(
            3usize
                .checked_mul(std::mem::size_of::<RelativePath>())
                .ok_or_else(|| invalid("source-entry selected path state overflow"))?,
        )
        .and_then(|bytes| {
            expected_names.iter().try_fold(bytes, |total, name| {
                name.len()
                    .checked_add(parent.len())
                    .and_then(|len| len.checked_mul(4))
                    .and_then(|path_bytes| total.checked_add(path_bytes))
            })
        })
        .and_then(|bytes| bytes.checked_add(512))
        .ok_or_else(|| invalid("source-entry selected path state overflow"))?;
    reader.check_state(selected_path_state)?;
    expected_paths
        .try_reserve_exact(3)
        .map_err(|_| invalid("source-entry delta path allocation"))?;
    for name in &expected_names {
        expected_paths.push(
            RelativePath::parse(&format!("{parent}/{name}"))
                .map_err(|_| invalid("source-entry delta exact path is invalid"))?,
        );
    }
    expected_paths.sort();
    if expected_paths.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(invalid("source-entry delta exact paths are not unique"));
    }
    let expected_paths: [RelativePath; 3] = expected_paths
        .try_into()
        .map_err(|_| invalid("source-entry delta exact path count differs"))?;
    let mut updates: [Option<&crate::source_command::SourceChange>; 3] = [None, None, None];
    for change in &proposal.changes {
        let index = expected_paths
            .iter()
            .position(|path| path == &change.path)
            .ok_or_else(|| invalid("source-entry delta changes an unselected path"))?;
        if updates[index].replace(change).is_some() || change.after.is_none() {
            return Err(invalid(
                "source-entry delta has a duplicate path or deletes a selected file",
            ));
        }
    }
    let updates = [
        updates[0].ok_or_else(|| invalid("source-entry delta omitted a selected file"))?,
        updates[1].ok_or_else(|| invalid("source-entry delta omitted a selected file"))?,
        updates[2].ok_or_else(|| invalid("source-entry delta omitted a selected file"))?,
    ];

    let source_relative = RelativePath::parse(source_path)
        .map_err(|_| invalid("source-entry delta source path is invalid"))?;
    let form_relative = expected_paths
        .iter()
        .find(|path| path.as_str().ends_with(".human-forms.json"))
        .cloned()
        .ok_or_else(|| invalid("source-entry delta human-form path absent"))?;
    let history_relative = expected_paths
        .iter()
        .find(|path| path.as_str().ends_with("/source-revision-history.json"))
        .cloned()
        .ok_or_else(|| invalid("source-entry delta history path absent"))?;
    validate_source_entry_identity_closure(
        reader.readset(),
        &source_relative,
        &form_relative,
        &history_relative,
        record_id,
    )?;

    for dependency in &proposal.reads {
        let witness = readset_member(reader.readset(), &dependency.path)?;
        if witness.expected_presence != Some(SourcePresenceV1::File)
            || witness
                .expected_member
                .is_none_or(|member| member.sha256 != dependency.raw_sha256)
        {
            return Err(invalid(
                "source-entry delta schema or owner dependency is outside its exact readset",
            ));
        }
    }

    let mut source_bytes_after = selected.source_bytes;
    let mut dependency_count_after = selected.dependency_count;
    let mut dependency_source_count_after = selected.dependency_source_count;
    let mut dependency_updates = Vec::new();
    let mut member_updates = Vec::new();
    reader.check_state(
        std::mem::size_of::<Vec<SourceEntryDependencyDeltaV1>>()
            .checked_add(
                3usize
                    .checked_mul(std::mem::size_of::<SourceEntryDependencyDeltaV1>())
                    .ok_or_else(|| invalid("source-entry dependency result state overflow"))?,
            )
            .and_then(|bytes| {
                bytes.checked_add(
                    std::mem::size_of::<Vec<SourceEntryMemberDeltaV1>>()
                        + 3 * std::mem::size_of::<SourceEntryMemberDeltaV1>(),
                )
            })
            .ok_or_else(|| invalid("source-entry delta allocation state overflow"))?,
    )?;
    dependency_updates
        .try_reserve_exact(3)
        .map_err(|_| invalid("source-entry dependency result allocation"))?;
    member_updates
        .try_reserve_exact(3)
        .map_err(|_| invalid("source-entry member result allocation"))?;
    for (path, change) in expected_paths.iter().zip(updates.iter()) {
        let old = readset_member(reader.readset(), path)?
            .expected_member
            .ok_or_else(|| invalid("source-entry delta selected prior file is absent"))?;
        if change.before != Some(old.sha256) {
            return Err(invalid(
                "source-entry delta before digest differs from selected member",
            ));
        }
        let after = change
            .after
            .as_deref()
            .ok_or_else(|| invalid("source-entry delta selected file is deleted"))?;
        let after_bytes = u64::try_from(after.len())
            .map_err(|_| invalid("source-entry delta member length overflow"))?;
        let after_tuple = SourceCutMemberTuple {
            sha256: Digest256::of_bytes(after),
            size_bytes: after_bytes,
            mode: old.mode,
        };
        source_bytes_after = source_bytes_after
            .checked_sub(old.size_bytes)
            .and_then(|bytes| bytes.checked_add(after_bytes))
            .ok_or_else(|| invalid("source-entry delta source byte count overflow"))?;

        let before_targets = reader.current_dependencies(path)?;
        if before_targets.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(invalid(
                "source-entry delta prior dependencies are not strictly ordered",
            ));
        }
        let retained_delta_state = source_entry_delta_state(
            &expected_paths,
            &dependency_updates,
            &member_updates,
            Some(&before_targets),
        )?;
        let limits = reader.json_limits();
        if after.len() > limits.max_bytes {
            return Err(decode_budget());
        }
        let parse_state = after
            .len()
            .checked_mul(3)
            .and_then(|bytes| {
                bytes.checked_add(
                    std::mem::size_of::<JsonValue>()
                        .checked_add(std::mem::size_of::<Vec<RelativePath>>())?,
                )
            })
            .ok_or_else(|| invalid("source-entry JSON validation state overflow"))?;
        let parse_live_state = retained_delta_state
            .checked_add(parse_state)
            .ok_or_else(|| invalid("source-entry JSON validation state overflow"))?;
        reader.check_state(parse_live_state)?;
        reader.tick(after.len())?;
        let json_state_bytes = reader
            .json_state_bytes()
            .checked_sub(retained_delta_state)
            .map(|remaining| remaining.min(parse_state))
            .ok_or_else(|| invalid("source-entry JSON workspace is exhausted"))?;
        let document = document(
            after,
            JsonMode::LegacyPythonObserved,
            limits,
            json_state_bytes,
        )?
        .ok_or_else(|| invalid("source-entry proposed member is not valid JSON"))?;
        reader.tick(after.len())?;
        let mut after_targets = Vec::new();
        if path == &source_relative {
            let identity_field = match document
                .object_get("schema_version")
                .and_then(JsonValue::as_str)
            {
                Some("tos_artifact_source_witness_v1" | "tos_artifact_source_witness_v2") => {
                    "artifact_id"
                }
                Some("tos_scholarly_composite_witness_v1") => "composite_id",
                Some("tos_source_link_v1") => "record_id",
                _ => return Err(invalid("source-entry delta record schema is not native")),
            };
            if document
                .object_get(identity_field)
                .and_then(JsonValue::as_str)
                != Some(record_id)
            {
                return Err(invalid(
                    "source-entry delta changed the selected record identity",
                ));
            }
        }
        collect_source_entry_references(
            reader,
            &document,
            path,
            &expected_paths,
            &mut after_targets,
            parse_live_state,
        )?;
        drop(document);
        reader.tick(after_targets.len().saturating_mul(usize::BITS as usize + 1))?;
        after_targets.sort();
        after_targets.dedup();
        reader.check_state(
            retained_delta_state
                .checked_add(relative_path_vec_state(&after_targets)?)
                .ok_or_else(|| invalid("source-entry dependency result state overflow"))?,
        )?;
        let before_edges = u64::try_from(before_targets.len())
            .map_err(|_| invalid("source-entry prior dependency count overflow"))?;
        let after_edges = u64::try_from(after_targets.len())
            .map_err(|_| invalid("source-entry new dependency count overflow"))?;
        dependency_count_after = dependency_count_after
            .checked_sub(before_edges)
            .and_then(|count| count.checked_add(after_edges))
            .ok_or_else(|| invalid("source-entry dependency count overflow"))?;
        match (before_edges != 0, after_edges != 0) {
            (false, true) => {
                dependency_source_count_after = dependency_source_count_after
                    .checked_add(1)
                    .ok_or_else(|| invalid("source-entry dependency source count overflow"))?;
            }
            (true, false) => {
                dependency_source_count_after = dependency_source_count_after
                    .checked_sub(1)
                    .ok_or_else(|| invalid("source-entry dependency source count underflow"))?;
            }
            _ => {}
        }
        dependency_updates.push(SourceEntryDependencyDeltaV1 {
            source: path.clone(),
            before: before_targets,
            after: after_targets,
        });
        member_updates.push(SourceEntryMemberDeltaV1 {
            path: path.clone(),
            before: old,
            after: after_tuple,
        });
    }
    if dependency_source_count_after > dependency_count_after {
        return Err(invalid("source-entry dependency counts are inconsistent"));
    }
    let dependency_updates: [SourceEntryDependencyDeltaV1; 3] = dependency_updates
        .try_into()
        .map_err(|_| invalid("source-entry dependency update cardinality differs"))?;
    let member_updates: [SourceEntryMemberDeltaV1; 3] = member_updates
        .try_into()
        .map_err(|_| invalid("source-entry member update cardinality differs"))?;

    let object_refcount_capacity = member_updates
        .len()
        .checked_mul(2)
        .ok_or_else(|| invalid("source-entry object-refcount delta capacity overflow"))?;
    let object_refcount_result_state = std::mem::size_of::<Vec<Digest256>>()
        .checked_add(
            object_refcount_capacity
                .checked_mul(std::mem::size_of::<Digest256>())
                .ok_or_else(|| invalid("source-entry object-refcount state overflow"))?,
        )
        .and_then(|bytes| {
            bytes.checked_add(
                std::mem::size_of::<Vec<SourceEntryObjectRefcountDeltaV1>>().checked_add(
                    object_refcount_capacity
                        .checked_mul(std::mem::size_of::<SourceEntryObjectRefcountDeltaV1>())?,
                )?,
            )
        })
        .ok_or_else(|| invalid("source-entry object-refcount state overflow"))?;
    reader.check_state(object_refcount_result_state)?;
    let mut object_digests = Vec::new();
    object_digests
        .try_reserve_exact(object_refcount_capacity)
        .map_err(|_| invalid("source-entry object digest allocation"))?;
    for update in &member_updates {
        object_digests.push(update.before.sha256);
        object_digests.push(update.after.sha256);
    }
    object_digests.sort_unstable();
    object_digests.dedup();
    let mut object_refcount_updates = Vec::new();
    object_refcount_updates
        .try_reserve_exact(object_digests.len())
        .map_err(|_| invalid("source-entry object-refcount result allocation"))?;
    for digest in object_digests {
        reader.tick(1)?;
        let before_count = reader.current_object_refcount(digest)?;
        validate_delta_object_refcount_lookup(reader.readset(), digest, before_count)?;
        let removed = u64::try_from(
            member_updates
                .iter()
                .filter(|update| update.before.sha256 == digest)
                .count(),
        )
        .map_err(|_| invalid("source-entry removed object reference count overflow"))?;
        let added = u64::try_from(
            member_updates
                .iter()
                .filter(|update| update.after.sha256 == digest)
                .count(),
        )
        .map_err(|_| invalid("source-entry added object reference count overflow"))?;
        let after_total = before_count
            .unwrap_or(0)
            .checked_sub(removed)
            .and_then(|count| count.checked_add(added))
            .ok_or_else(|| invalid("source-entry object reference count is inconsistent"))?;
        object_refcount_updates.push(SourceEntryObjectRefcountDeltaV1 {
            digest,
            before_count,
            after_count: (after_total != 0).then_some(after_total),
        });
    }
    let readset = reader.readset();
    for update in &dependency_updates {
        let mut witnesses = readset
            .dependencies
            .iter()
            .filter(|witness| witness.path == update.source);
        if witnesses
            .next()
            .is_none_or(|witness| witness.expected_targets != update.before)
            || witnesses.next().is_some()
        {
            return Err(invalid(
                "source-entry delta outgoing dependency witness differs",
            ));
        }
    }
    reader.check_state(
        source_entry_delta_state(&expected_paths, &dependency_updates, &member_updates, None)?
            .checked_add(object_refcount_delta_state(&object_refcount_updates)?)
            .ok_or_else(|| invalid("source-entry combined delta state overflow"))?,
    )?;
    if selected.member_count == 0 {
        return Err(invalid("source-entry selected baseline has no members"));
    }
    Ok(ValidatedSourceEntryDeltaV1 {
        selected_base: selected,
        observed_base_revision: readset.base_revision,
        proposal,
        source_path: source_relative,
        record_id,
        updates,
        member_updates,
        dependency_updates,
        object_refcount_updates,
        member_count_after: selected.member_count,
        source_bytes_after,
        identity_count_after: selected.identity_count,
        dependency_source_count_after,
        dependency_count_after,
        retirement_count_after: selected.retirement_count,
        prepared_schema,
    })
}

/// Existing standalone kernel callers retain the original return contract.
pub fn build_index(
    input: &mut CandidateInput<'_>,
    fresh: &FreshRows,
    base: Option<&Index>,
    limits: IndexLimits,
    schemas: &mut SchemaCheck<'_>,
) -> io::Result<Index> {
    build_index_accounted(input, fresh, base, limits, schemas).map(|(index, _)| index)
}
/// Same kernel and pre-allocation ledger; return its retained logical upper
/// bound so the complete admission operation does not lose index state at the
/// phase boundary. This is not an allocator/RSS measurement.
pub fn build_index_accounted(
    input: &mut CandidateInput<'_>,
    fresh: &FreshRows,
    base: Option<&Index>,
    limits: IndexLimits,
    schemas: &mut SchemaCheck<'_>,
) -> io::Result<(Index, usize)> {
    build_index_from_rows_accounted(input, fresh, base, limits, schemas)
}

/// Same resident return contract for a privately retained, replayable row
/// source. This is the narrow bridge used by the spooled admission callback;
/// source-law decisions still run only in `build_index_into` below.
pub(crate) fn build_index_from_rows_accounted(
    input: &mut CandidateInput<'_>,
    fresh: &dyn FreshIndexRows,
    base: Option<&Index>,
    limits: IndexLimits,
    schemas: &mut SchemaCheck<'_>,
) -> io::Result<(Index, usize)> {
    let mut backend = ResidentIndexBackend::new(fresh);
    let mut base_identity = |id: &str, max_state_bytes: usize| {
        let Some(path) = base.and_then(|index| index.identities.get(id)) else {
            return Ok(None);
        };
        let owned_state = path
            .len()
            .checked_mul(16)
            .and_then(|bytes| bytes.checked_add(2048))
            .ok_or_else(|| invalid("base source identity clone state overflow"))?;
        if owned_state > max_state_bytes {
            return Err(invalid("base source identity clone exceeds state budget"));
        }
        Ok(Some(BaseIdentityPath::Resident(path.clone())))
    };
    let retained_state =
        build_index_into(input, &mut base_identity, limits, &mut backend, schemas)?;
    Ok((backend.finish(), retained_state))
}

/// The resident API and the streamed FND index use the same maintained parser,
/// schema, duplicate-ID, structured-reference and dependency predicates.
/// Backends own only row storage; they cannot attest a candidate or mint a view.
pub(crate) fn build_index_into(
    input: &mut dyn CandidateIndexInput,
    base_identity: &mut dyn FnMut(&str, usize) -> io::Result<Option<BaseIdentityPath>>,
    limits: IndexLimits,
    backend: &mut dyn AdmissionIndexBackend,
    schemas: &mut SchemaCheck<'_>,
) -> io::Result<usize> {
    input.tick()?;
    let retained_rows = backend.retains_index_rows();
    let mut state = IndexAccounting::new(limits)?;
    if retained_rows {
        state.reserve(std::mem::size_of::<Index>(), false)?;
    }
    let expected_records = backend.source_record_count()?;
    let mut record_count = 0u64;
    backend.for_each_source_record(&mut |backend, id, source_ref| {
        record_count = record_count
            .checked_add(1)
            .ok_or_else(|| invalid("fresh record row count overflow"))?;
        bind(input, backend, &mut state, id, source_ref)
    })?;
    if record_count != expected_records || backend.source_record_count()? != expected_records {
        return Err(invalid(
            "fresh record source did not reach authenticated EOF",
        ));
    }
    let expected_claims = backend.source_claim_count()?;
    let mut claim_count = 0u64;
    backend.for_each_source_claim(&mut |backend, id, source_ref| {
        claim_count = claim_count
            .checked_add(1)
            .ok_or_else(|| invalid("fresh claim row count overflow"))?;
        bind(input, backend, &mut state, id, source_ref)
    })?;
    if claim_count != expected_claims || backend.source_claim_count()? != expected_claims {
        return Err(invalid(
            "fresh claim source did not reach authenticated EOF",
        ));
    }
    struct SemanticGroup {
        id: String,
        minimum_path: String,
        previous_path: Option<BaseIdentityPath>,
        previous_seen: bool,
    }
    fn semantic_group_state_upper_bound(
        id: &str,
        minimum_path: &str,
        previous_path: Option<&BaseIdentityPath>,
    ) -> io::Result<usize> {
        let text_bytes = id
            .len()
            .checked_add(minimum_path.len())
            .and_then(|n| n.checked_add(previous_path.map_or(0, |path| path.as_str().len())))
            .ok_or_else(|| invalid("native semantic group text state overflow"))?;
        std::mem::size_of::<SemanticGroup>()
            .checked_add(owned_text_state_upper_bound(text_bytes)?)
            .ok_or_else(|| invalid("native semantic group state overflow"))
    }
    fn semantic_row_state_upper_bound(id: &str, path: &str) -> io::Result<usize> {
        owned_text_state_upper_bound(
            id.len()
                .checked_add(path.len())
                .ok_or_else(|| invalid("native semantic provider row state overflow"))?,
        )
    }
    fn check_semantic_peak(
        input: &dyn CandidateIndexInput,
        state: &IndexAccounting,
        group_bytes: usize,
        provider_row_bytes: usize,
        cursor_bytes: usize,
    ) -> io::Result<()> {
        let peak = group_bytes
            .checked_add(provider_row_bytes)
            .and_then(|n| n.checked_add(cursor_bytes))
            .ok_or_else(|| invalid("native semantic peak state overflow"))?;
        state.check_temporary(peak)?;
        input.check_row_state(peak)
    }
    fn finish_semantic_group(
        input: &mut dyn CandidateIndexInput,
        backend: &mut dyn AdmissionIndexBackend,
        state: &mut IndexAccounting,
        group: SemanticGroup,
        provider_row_bytes: usize,
        cursor_bytes: usize,
    ) -> io::Result<()> {
        let anchor = if group.previous_seen {
            group
                .previous_path
                .as_ref()
                .map(BaseIdentityPath::as_str)
                .ok_or_else(|| invalid("native semantic previous-path state is inconsistent"))?
        } else {
            group.minimum_path.as_str()
        };
        let group_bytes = semantic_group_state_upper_bound(
            &group.id,
            &group.minimum_path,
            group.previous_path.as_ref(),
        )?;
        let binding_bytes = owned_text_state_upper_bound(
            group
                .id
                .len()
                .checked_add(anchor.len())
                .ok_or_else(|| invalid("native semantic binding state overflow"))?,
        )?;
        check_semantic_peak(
            input,
            state,
            group_bytes,
            binding_bytes
                .checked_add(provider_row_bytes)
                .ok_or_else(|| invalid("native semantic binding peak overflow"))?,
            cursor_bytes,
        )?;
        bind(input, backend, state, &group.id, anchor)
    }
    let mut current_group: Option<SemanticGroup> = None;
    let mut semantic_count = 0u64;
    let expected_semantic_rows = backend.source_native_semantic_row_count()?;
    let mut semantic_rows = 0u64;
    backend.for_each_source_native_semantic(&mut |step| match step {
        NativeSemanticStep::Preflight {
            cursor_id_bytes,
            id_bytes,
            path_bytes,
        } => {
            input.tick()?;
            let provider_row_bytes = owned_text_state_upper_bound(
                id_bytes
                    .checked_add(path_bytes)
                    .ok_or_else(|| invalid("native semantic provider row state overflow"))?,
            )?;
            let cursor_bytes = if cursor_id_bytes == 0 {
                0
            } else {
                owned_text_state_upper_bound(cursor_id_bytes)?
            };
            let current_group_bytes = current_group
                .as_ref()
                .map(|group| {
                    semantic_group_state_upper_bound(
                        &group.id,
                        &group.minimum_path,
                        group.previous_path.as_ref(),
                    )
                })
                .transpose()?
                .unwrap_or(0);
            check_semantic_peak(
                input,
                &state,
                current_group_bytes,
                provider_row_bytes,
                cursor_bytes,
            )
        }
        NativeSemanticStep::Row {
            backend,
            cursor_id_bytes,
            id,
            path,
            group_first,
        } => {
            input.tick()?;
            let provider_row_bytes = semantic_row_state_upper_bound(id, path)?;
            let cursor_bytes = if cursor_id_bytes == 0 {
                0
            } else {
                owned_text_state_upper_bound(cursor_id_bytes)?
            };
            let current_group_bytes = current_group
                .as_ref()
                .map(|group| {
                    semantic_group_state_upper_bound(
                        &group.id,
                        &group.minimum_path,
                        group.previous_path.as_ref(),
                    )
                })
                .transpose()?
                .unwrap_or(0);
            check_semantic_peak(
                input,
                &state,
                current_group_bytes,
                provider_row_bytes,
                cursor_bytes,
            )?;
            if !input.member(path)? {
                return Err(invalid(
                    "source identity is outside the admitted member set",
                ));
            }
            let starts_group = current_group
                .as_ref()
                .is_none_or(|group| group.id.as_str() != id);
            if group_first != starts_group {
                return Err(invalid(
                    "fresh semantic group marker differs from ordered source rows",
                ));
            }
            if starts_group {
                if current_group
                    .as_ref()
                    .is_some_and(|group| id <= group.id.as_str())
                {
                    return Err(invalid(
                        "fresh semantic identities are not strictly ordered",
                    ));
                }
                if let Some(group) = current_group.take() {
                    finish_semantic_group(
                        input,
                        backend,
                        &mut state,
                        group,
                        provider_row_bytes,
                        cursor_bytes,
                    )?;
                }
                semantic_count = semantic_count
                    .checked_add(1)
                    .ok_or_else(|| invalid("fresh semantic row count overflow"))?;
                // Hold room for old cursor, the provider row, and the fixed
                // new-group state before the base lookup can allocate a path.
                let prelookup_group_bytes = semantic_group_state_upper_bound(id, path, None)?;
                check_semantic_peak(
                    input,
                    &state,
                    prelookup_group_bytes,
                    provider_row_bytes,
                    cursor_bytes,
                )?;
                let base_path_state_allowance = lookup_allowance(
                    input,
                    &state,
                    prelookup_group_bytes
                        .checked_add(provider_row_bytes)
                        .and_then(|n| n.checked_add(cursor_bytes))
                        .ok_or_else(|| invalid("native semantic base lookup state overflow"))?,
                )?;
                let previous_path = base_identity(id, base_path_state_allowance)?;
                let next_group_bytes =
                    semantic_group_state_upper_bound(id, path, previous_path.as_ref())?;
                check_semantic_peak(
                    input,
                    &state,
                    next_group_bytes,
                    provider_row_bytes,
                    cursor_bytes,
                )?;
                current_group = Some(SemanticGroup {
                    id: id.to_owned(),
                    minimum_path: path.to_owned(),
                    previous_seen: previous_path
                        .as_ref()
                        .is_some_and(|previous| previous.as_str() == path),
                    previous_path,
                });
            } else if let Some(group) = current_group.as_mut() {
                if group
                    .previous_path
                    .as_ref()
                    .is_some_and(|previous| previous.as_str() == path)
                {
                    group.previous_seen = true;
                }
                if path < group.minimum_path.as_str() {
                    let old_group_bytes = semantic_group_state_upper_bound(
                        &group.id,
                        &group.minimum_path,
                        group.previous_path.as_ref(),
                    )?;
                    let new_minimum_bytes = owned_text_state_upper_bound(path.len())?;
                    let replacement_peak = old_group_bytes
                        .checked_add(new_minimum_bytes)
                        .ok_or_else(|| invalid("native semantic replacement state overflow"))?;
                    check_semantic_peak(
                        input,
                        &state,
                        replacement_peak,
                        provider_row_bytes,
                        cursor_bytes,
                    )?;
                    // Build the replacement while old group, cursor and row
                    // buffers remain live under the peak check above.
                    let replacement = path.to_owned();
                    group.minimum_path = replacement;
                }
            }
            semantic_rows = semantic_rows
                .checked_add(1)
                .ok_or_else(|| invalid("fresh semantic source row count overflow"))?;
            Ok(())
        }
    })?;
    if let Some(group) = current_group.take() {
        finish_semantic_group(input, backend, &mut state, group, 0, 0)?;
    }
    let expected_semantics = backend.source_native_semantic_count()?;
    if semantic_count != expected_semantics
        || semantic_rows != expected_semantic_rows
        || backend.source_native_semantic_row_count()? != expected_semantic_rows
    {
        return Err(invalid(
            "fresh semantic source did not reach authenticated EOF",
        ));
    }
    let mut schema = None;
    let mut after: Option<String> = None;
    while let Some(path) = input.member_after(after.as_deref())? {
        after = Some(path.clone());
        if path.starts_with("ToS/source-witnesses/retirements/") && path.ends_with(".json") {
            let raw = input.bytes(&path, MAX_EVENT_BYTES)?;
            let row = strict_object(&raw, event_limits(input))?;
            if schema.is_none() {
                let raw = input.bytes(RETIREMENT_SCHEMA, MAX_EVENT_BYTES)?;
                strict_object(&raw, event_limits(input))?;
                schema = Some(raw);
            }
            let schema = schema
                .as_deref()
                .ok_or_else(|| invalid("retirement schema missing"))?;
            if schemas(&path, &raw, schema)?.is_some()
                || string(&row, "schema_version")? != "tos_provenance_event_v1"
                || string(&row, "event_type")? != "migration"
                || string(field(&row, "method")?, "name")? != "corpus-source-retirement"
                || !string(&row, "event_id")?.starts_with("tos.event.")
            {
                return Err(invalid(
                    "source retirement record has an invalid operation or ID",
                ));
            }
            bind(input, backend, &mut state, string(&row, "event_id")?, &path)?;
        }
    }
    let json = input.json_limits();
    let json_state_bytes = input.json_state_bytes();
    let mut after: Option<String> = None;
    while let Some(path) = input.member_after(after.as_deref())? {
        after = Some(path.clone());
        input.tick()?;
        if path.starts_with("ToS/contracts/")
            || path.starts_with("ToS/doctrine/semantic-interchange/")
        {
            continue;
        }
        if path.ends_with(".json") && input.member_size(&path)? <= STRUCTURED_JSON_BYTES {
            let raw = input.bytes(&path, json.max_bytes)?;
            if let Some(row) =
                document(&raw, JsonMode::LegacyPythonObserved, json, json_state_bytes)?
            {
                references(input, backend, &mut state, &path, &row)?;
            }
        } else if path.ends_with(".jsonl") {
            let raw = input.bytes(&path, json.max_bytes)?;
            for line in raw.split(|b| *b == b'\n') {
                input.tick()?;
                if line.iter().all(u8::is_ascii_whitespace) {
                    continue;
                }
                if let Some(row) =
                    document(line, JsonMode::LegacyPythonObserved, json, json_state_bytes)?
                {
                    references(input, backend, &mut state, &path, &row)?;
                }
            }
        }
    }
    struct SemanticReplayGroup {
        id: String,
        anchor: String,
    }
    fn semantic_replay_group_state_upper_bound(id: &str, anchor: &str) -> io::Result<usize> {
        std::mem::size_of::<SemanticReplayGroup>()
            .checked_add(owned_text_state_upper_bound(
                id.len()
                    .checked_add(anchor.len())
                    .ok_or_else(|| invalid("native semantic replay group state overflow"))?,
            )?)
            .ok_or_else(|| invalid("native semantic replay group state overflow"))
    }
    let mut replay_group: Option<SemanticReplayGroup> = None;
    let expected_replay_rows = backend.source_native_semantic_row_count()?;
    let mut replay_rows = 0u64;
    let mut replay_groups = 0u64;
    backend.for_each_source_native_semantic(&mut |step| match step {
        NativeSemanticStep::Preflight {
            cursor_id_bytes,
            id_bytes,
            path_bytes,
        } => {
            input.tick()?;
            let row_bytes = owned_text_state_upper_bound(
                id_bytes
                    .checked_add(path_bytes)
                    .ok_or_else(|| invalid("native semantic replay row state overflow"))?,
            )?;
            let cursor_bytes = if cursor_id_bytes == 0 {
                0
            } else {
                owned_text_state_upper_bound(cursor_id_bytes)?
            };
            let group_bytes = replay_group
                .as_ref()
                .map(|group| semantic_replay_group_state_upper_bound(&group.id, &group.anchor))
                .transpose()?
                .unwrap_or(0);
            check_semantic_peak(input, &state, group_bytes, row_bytes, cursor_bytes)
        }
        NativeSemanticStep::Row {
            backend,
            cursor_id_bytes,
            id,
            path,
            group_first,
        } => {
            input.tick()?;
            let row_bytes = semantic_row_state_upper_bound(id, path)?;
            let cursor_bytes = if cursor_id_bytes == 0 {
                0
            } else {
                owned_text_state_upper_bound(cursor_id_bytes)?
            };
            let old_group_bytes = replay_group
                .as_ref()
                .map(|group| semantic_replay_group_state_upper_bound(&group.id, &group.anchor))
                .transpose()?
                .unwrap_or(0);
            check_semantic_peak(input, &state, old_group_bytes, row_bytes, cursor_bytes)?;
            let starts_group = replay_group
                .as_ref()
                .is_none_or(|group| group.id.as_str() != id);
            if starts_group != group_first {
                return Err(invalid(
                    "fresh semantic replay marker differs from ordered source rows",
                ));
            }
            if starts_group {
                if replay_group
                    .as_ref()
                    .is_some_and(|group| id <= group.id.as_str())
                {
                    return Err(invalid(
                        "fresh semantic replay identities are not strictly ordered",
                    ));
                }
                let id_copy_bytes = owned_text_state_upper_bound(id.len())?;
                let lookup_held_bytes = old_group_bytes
                    .checked_add(row_bytes)
                    .and_then(|n| n.checked_add(cursor_bytes))
                    .and_then(|n| n.checked_add(id_copy_bytes))
                    .ok_or_else(|| invalid("native semantic replay lookup state overflow"))?;
                let max_anchor_state = lookup_allowance(input, &state, lookup_held_bytes)?;
                let anchor = backend
                    .identity_path(id, max_anchor_state)?
                    .ok_or_else(|| invalid("native semantic anchor missing"))?;
                let next_group_bytes = semantic_replay_group_state_upper_bound(id, &anchor)?;
                let replacement_peak = old_group_bytes
                    .checked_add(next_group_bytes)
                    .and_then(|n| n.checked_add(row_bytes))
                    .and_then(|n| n.checked_add(cursor_bytes))
                    .ok_or_else(|| invalid("native semantic replay replacement overflow"))?;
                state.check_temporary(replacement_peak)?;
                input.check_row_state(replacement_peak)?;
                replay_group = Some(SemanticReplayGroup {
                    id: id.to_owned(),
                    anchor,
                });
                replay_groups = replay_groups
                    .checked_add(1)
                    .ok_or_else(|| invalid("native semantic replay group count overflow"))?;
            }
            let group = replay_group
                .as_ref()
                .ok_or_else(|| invalid("native semantic replay group is missing"))?;
            edge(input, backend, &mut state, &group.anchor, path)?;
            replay_rows = replay_rows
                .checked_add(1)
                .ok_or_else(|| invalid("native semantic replay row count overflow"))?;
            Ok(())
        }
    })?;
    let expected_replay_groups = backend.source_native_semantic_count()?;
    if replay_groups != expected_replay_groups
        || replay_rows != expected_replay_rows
        || backend.source_native_semantic_row_count()? != expected_replay_rows
    {
        return Err(invalid(
            "fresh semantic edge replay did not reach authenticated EOF",
        ));
    }
    input.tick()?;
    Ok(state.bytes)
}

pub fn validate_retirements(
    input: &mut CandidateInput<'_>,
    retirements: &[Value],
    base: Option<&Value>,
    schemas: &mut SchemaCheck<'_>,
) -> io::Result<BTreeMap<String, String>> {
    input.tick()?;
    if retirements.is_empty() {
        return Ok(BTreeMap::new());
    }
    let base = base.ok_or_else(|| invalid("source retirement requires an accepted base"))?;
    let schema = input.bytes(RETIREMENT_SCHEMA, MAX_EVENT_BYTES)?;
    strict_object(&schema, event_limits(input))?;
    let mut groups: BTreeMap<&str, Vec<&Value>> = BTreeMap::new();
    for row in retirements {
        input.tick()?;
        groups
            .entry(string(row, "event_ref")?)
            .or_default()
            .push(row);
    }
    let mut identities = BTreeMap::new();
    for (event_ref, rows) in groups {
        input.tick()?;
        if !event_ref.starts_with("ToS/source-witnesses/retirements/")
            || !event_ref.ends_with(".json")
        {
            return Err(invalid(
                "retirement event must use the source retirement owner path",
            ));
        }
        let raw = input.bytes(event_ref, MAX_EVENT_BYTES)?;
        let event = strict_object(&raw, event_limits(input))?;
        if let Some(issue) = schemas(event_ref, &raw, &schema)? {
            return Err(invalid(format!(
                "{event_ref}: retirement event violates provenance schema: {issue}"
            )));
        }
        let method = field(&event, "method")?;
        let id = string(&event, "event_id")?;
        if string(&event, "schema_version")? != "tos_provenance_event_v1"
            || string(&event, "event_type")? != "migration"
            || !id.starts_with("tos.event.")
            || !matches!(
                string(&event, "status")?,
                "completed" | "completed_with_warnings"
            )
            || string(method, "name")? != "corpus-source-retirement"
            || string(method, "version")? != "1"
        {
            return Err(invalid(
                "retirement event does not declare the source retirement operation",
            ));
        }
        if tos_validation::retirement_rules::observed_datetime_order(
            string(&event, "started_at")?,
            string(&event, "ended_at")?,
        )
        .map_err(|e| invalid(format!("retirement date-time invalid: {e:?}")))?
            == std::cmp::Ordering::Greater
        {
            return Err(invalid("retirement event ends before it starts"));
        }
        let config = field(method, "configuration")?;
        let keys = config
            .as_object()
            .ok_or_else(|| invalid("retirement configuration must be an object"))?;
        if keys.len() != 5
            || [
                "base_revision",
                "retirements",
                "reason",
                "review_ref",
                "review_sha256",
            ]
            .iter()
            .any(|k| !keys.contains_key(*k))
        {
            return Err(invalid(
                "retirement configuration must bind base, exact targets and owner review",
            ));
        }
        let mut targets = rows
            .iter()
            .map(|r| Ok((string(r, "path")?, string(r, "sha256")?)))
            .collect::<io::Result<Vec<_>>>()?;
        targets.sort_by(|a, b| a.0.cmp(b.0));
        let targets = targets
            .into_iter()
            .map(|(p, s)| json!({"path":p,"sha256":s}))
            .collect::<Vec<_>>();
        if field(config, "base_revision")? != field(base, "revision")?
            || field(config, "retirements")? != &json!(targets)
        {
            return Err(invalid(
                "retirement targets or accepted base differ from the source batch",
            ));
        }
        if tos_foundation::python_strip_unicode16_v1(
            string(config, "reason")?,
            input.json.max_bytes,
        )
        .map_err(invalid)?
        .is_empty()
        {
            return Err(invalid("retirement needs a source-visible reason"));
        }
        let review = string(config, "review_ref")?;
        RelativePath::parse(review).map_err(invalid)?;
        let digest = string(config, "review_sha256")?;
        if Digest256::from_hex(digest).is_err()
            || digest
                .bytes()
                .any(|b| !(b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
        {
            return Err(invalid("retirement review digest is invalid"));
        }
        if !review.starts_with("ToS/review-ledger/") || review == event_ref {
            return Err(invalid(
                "retirement review must return to the source-owned review ledger",
            ));
        }
        let metadata = input
            .members
            .get(review)
            .ok_or_else(|| invalid("retirement review is missing"))?;
        if string(metadata, "sha256")? != digest || member_size(metadata)? == 0 {
            return Err(invalid(
                "retirement review is empty or has a different digest",
            ));
        }
        input.tick()?;
        (input.verify_member)(review)?;
        input.tick()?;
        let mut expected_inputs=targets.iter().map(|r|Ok(json!({"ref":string(r,"path")?,"role":"retired_source","sha256":string(r,"sha256")?}))).collect::<io::Result<Vec<_>>>()?;
        expected_inputs.push(json!({"ref":review,"role":"source_owner_review","sha256":digest}));
        if field(&event, "inputs")? != &json!(expected_inputs) {
            return Err(invalid(
                "retirement provenance inputs do not bind the exact sources and review",
            ));
        }
        if field(&event, "outputs")? != &json!([{"ref":event_ref,"role":"corpus_retirement_event"}])
        {
            return Err(invalid(
                "retirement provenance output must name this retained event",
            ));
        }
        if field(&event, "receipt_refs")? != &json!([review]) {
            return Err(invalid(
                "retirement receipt must return to its exact owner review",
            ));
        }
        if identities
            .insert(id.to_owned(), event_ref.to_owned())
            .is_some()
        {
            return Err(invalid("duplicate source retirement event ID"));
        }
    }
    input.tick()?;
    Ok(identities)
}

/// None routes a wider edit to full source validation. A surviving incoming
/// dependency or a reused accepted ID refuses instead of taking that fallback.
pub fn membership_transition(
    input: &mut CandidateInput<'_>,
    retirements: &[Value],
    base: Option<&Value>,
    event_ids: &BTreeMap<String, String>,
) -> io::Result<Option<Index>> {
    input.tick()?;
    let Some(base) = base.filter(|_| !retirements.is_empty()) else {
        return Ok(None);
    };
    let retired = retirements
        .iter()
        .map(|r| Ok(string(r, "path")?.to_owned()))
        .collect::<io::Result<BTreeSet<_>>>()?;
    if retired
        .iter()
        .any(|p| !p.starts_with("ToS/source-witnesses/"))
    {
        return Ok(None);
    }
    let events = event_ids.values().cloned().collect::<BTreeSet<_>>();
    let mut reviews = BTreeMap::new();
    for path in &events {
        let raw = input.bytes(path, MAX_EVENT_BYTES)?;
        let event = strict_object(&raw, event_limits(input))?;
        reviews.insert(
            path.clone(),
            string(
                field(field(&event, "method")?, "configuration")?,
                "review_ref",
            )?
            .to_owned(),
        );
    }
    let mut previous = BTreeMap::new();
    for entry in array(field(base, "files")?)? {
        input.tick()?;
        if previous.insert(string(entry, "path")?, entry).is_some() {
            return Err(invalid("duplicate accepted base member"));
        }
    }
    if events.iter().any(|p| previous.contains_key(p.as_str())) {
        return Ok(None);
    }
    let added = input
        .members
        .keys()
        .filter(|p| !previous.contains_key(p.as_str()))
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut expected = events.clone();
    expected.extend(
        reviews
            .values()
            .filter(|p| !previous.contains_key(p.as_str()))
            .cloned(),
    );
    if added != expected
        || previous
            .keys()
            .filter(|p| !input.members.contains_key(**p))
            .map(|p| (*p).to_owned())
            .collect::<BTreeSet<_>>()
            != retired
    {
        return Ok(None);
    }
    for (path, entry) in &previous {
        input.tick()?;
        if !retired.contains(*path) && input.members.get(*path) != Some(*entry) {
            return Ok(None);
        }
    }
    let mut index = Index::default();
    let old_deps = field(base, "dependencies")?
        .as_object()
        .ok_or_else(|| invalid("accepted dependencies must be an object"))?;
    for (source, targets) in old_deps {
        input.tick()?;
        if retired.contains(source) {
            continue;
        }
        let targets = array(targets)?
            .iter()
            .map(|v| Ok(text(v)?.to_owned()))
            .collect::<io::Result<Vec<_>>>()?;
        if targets.iter().any(|p| retired.contains(p)) {
            return Err(invalid(format!(
                "retirement leaves an incoming source dependency unresolved: {source}"
            )));
        }
        index.dependencies.insert(source.clone(), targets);
    }
    let old_ids = field(base, "identities")?
        .as_object()
        .ok_or_else(|| invalid("accepted identities must be an object"))?;
    for (id, path) in old_ids {
        input.tick()?;
        let path = text(path)?;
        if !retired.contains(path) {
            index.identities.insert(id.clone(), path.to_owned());
        }
    }
    for (id, path) in event_ids {
        input.tick()?;
        if old_ids.contains_key(id) {
            return Err(invalid(
                "retirement event reuses an accepted source identity",
            ));
        }
        index.identities.insert(id.clone(), path.clone());
    }
    for (event, review) in reviews {
        index.dependencies.insert(event, vec![review]);
    }
    input.tick()?;
    Ok(Some(index))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn members(raw: &BTreeMap<String, Vec<u8>>) -> BTreeMap<String, Value> {
        raw.iter().map(|(p,b)|(p.clone(),json!({"path":p,"sha256":Digest256::of_bytes(b).to_hex(),"size_bytes":b.len(),"mode":420}))).collect()
    }
    fn with_input<T>(
        raw: &BTreeMap<String, Vec<u8>>,
        f: impl FnOnce(&mut CandidateInput<'_>) -> io::Result<T>,
    ) -> io::Result<T> {
        let members = members(raw);
        let mut read = |p: &str, cap: usize| {
            let b = raw.get(p).ok_or_else(|| invalid("missing fixture"))?;
            if b.len() > cap {
                return Err(invalid("fixture cap"));
            }
            Ok(b.clone())
        };
        let mut verify = |p: &str| {
            if raw.contains_key(p) {
                Ok(())
            } else {
                Err(invalid("missing fixture review"))
            }
        };
        let mut check = || Ok(());
        f(&mut CandidateInput {
            members: &members,
            read: &mut read,
            verify_member: &mut verify,
            check: &mut check,
            json: JsonLimits::default(),
            json_state_bytes: 1024 * 1024,
        })
    }
    #[test]
    fn structured_values_native_anchor_and_duplicate_conflict() {
        let a = "ToS/source-witnesses/a.json";
        let b = "ToS/source-witnesses/b.json";
        let c = "ToS/source-witnesses/c.json";
        let raw=BTreeMap::from([
            (a.to_owned(),r#"{"tos.b":"unused","nested":["tos.b","ToS/source-witnesses/c.json:١٢#anchor","ToS/source-witnesses/b.json#\ud800"],"x":"tos.b","x":"unused"}"#.as_bytes().to_vec()),
            (b.to_owned(),b"{}".to_vec()),(c.to_owned(),b"{}".to_vec()),
            ("ToS/source-witnesses/note.md".to_owned(),b"tos.b".to_vec()),
            ("ToS/contracts/test.json".to_owned(),br#"{"value":"tos.b"}"#.to_vec()),
        ]);
        let fresh = FreshRows {
            records: vec![json!({"record_id":"tos.b","source_record_ref":b})],
            claims: vec![],
            native_semantic: BTreeMap::from([(
                "tos.native".to_owned(),
                vec![a.to_owned(), c.to_owned()],
            )]),
        };
        let base = Index {
            identities: BTreeMap::from([("tos.native".to_owned(), c.to_owned())]),
            dependencies: BTreeMap::new(),
        };
        let limits = IndexLimits {
            max_edges: 32,
            max_state_bytes: 16384,
        };
        let result = with_input(&raw, |input| {
            build_index(input, &fresh, Some(&base), limits, &mut |_, _, _| {
                panic!("no retirement schema read")
            })
        })
        .unwrap();
        assert_eq!(result.identities["tos.native"], c);
        assert_eq!(result.dependencies[a], vec![b.to_owned(), c.to_owned()]);
        assert_eq!(result.dependencies[c], vec![a.to_owned()]);
        assert!(
            !result
                .dependencies
                .contains_key("ToS/source-witnesses/note.md")
        );
        assert!(!result.dependencies.contains_key("ToS/contracts/test.json"));
        // A resource refusal must abort admission, never masquerade as an
        // optional malformed document and silently omit its dependencies.
        let exhausted = with_input(&raw, |input| {
            input.json_state_bytes = 1;
            build_index(input, &fresh, Some(&base), limits, &mut |_, _, _| {
                panic!("no retirement schema read")
            })
        })
        .unwrap_err();
        assert!(exhausted.to_string().contains("JSON parser state budget"));
        let conflict = FreshRows {
            records: vec![
                json!({"record_id":"tos.b","source_record_ref":a}),
                json!({"record_id":"tos.b","source_record_ref":b}),
            ],
            claims: vec![],
            native_semantic: BTreeMap::new(),
        };
        assert!(
            with_input(&raw, |input| build_index(
                input,
                &conflict,
                None,
                limits,
                &mut |_, _, _| Ok(None)
            ))
            .unwrap_err()
            .to_string()
            .contains("duplicate source identity")
        );
        assert!(
            with_input(&raw, |input| build_index(
                input,
                &fresh,
                None,
                IndexLimits {
                    max_edges: 1,
                    ..limits
                },
                &mut |_, _, _| Ok(None)
            ))
            .is_err()
        );
    }
    #[test]
    fn python_json_and_regex_boundaries() {
        assert_eq!(reference_path("ToS/a:12#x"), "ToS/a");
        assert_eq!(reference_path("ToS/a:١٢"), "ToS/a");
        assert_eq!(reference_path("ToS/a:²"), "ToS/a:²");
        assert_eq!(reference_path("ToS/a:12\n"), "ToS/a\n");
        assert_eq!(reference_path("ToS/a:12\r"), "ToS/a:12\r");
        let source = r#"{"x":"wrong","x":"tos.correct","other":NaN}"#;
        for encoding in [
            source.as_bytes().to_vec(),
            source.encode_utf16().flat_map(u16::to_le_bytes).collect(),
            source
                .chars()
                .flat_map(|c| (c as u32).to_be_bytes())
                .collect(),
        ] {
            let row = document(
                &encoding,
                JsonMode::LegacyPythonObserved,
                JsonLimits::default(),
                1024 * 1024,
            )
            .unwrap()
            .unwrap();
            assert_eq!(row.object_get("x").unwrap().as_str(), Some("tos.correct"));
        }
        assert!(
            strict_object(br#"{"x":1,"x":2}"#, JsonLimits::default())
                .unwrap_err()
                .to_string()
                .contains("duplicate fields")
        );
        assert!(strict_object(br#"{"x":1e999}"#, JsonLimits::default()).is_err());
        assert!(
            document(
                b"not json",
                JsonMode::LegacyPythonObserved,
                JsonLimits::default(),
                1024 * 1024,
            )
            .unwrap()
            .is_none()
        );
        let row = document(
            b"{\"x\":\"\xed\xa0\x80\",\"y\":\"tos.correct\"}",
            JsonMode::LegacyPythonObserved,
            JsonLimits::default(),
            1024 * 1024,
        )
        .unwrap()
        .unwrap();
        assert_eq!(row.object_get("y").unwrap().as_str(), Some("tos.correct"));
    }
    #[test]
    fn retirement_exact_bindings_and_narrow_transfer() {
        let old = "ToS/source-witnesses/old.md";
        let event = "ToS/source-witnesses/retirements/old.json";
        let review = "ToS/review-ledger/old.md";
        let old_sha = Digest256::of_bytes(b"old").to_hex();
        let review_sha = Digest256::of_bytes(b"review").to_hex();
        let config = json!({"base_revision":"base","retirements":[{"path":old,"sha256":old_sha}],"reason":"retained source owner review","review_ref":review,"review_sha256":review_sha});
        let event_value = json!({"schema_version":"tos_provenance_event_v1","event_type":"migration","event_id":"tos.event.retirement","status":"completed","started_at":"2026-09-30T00:00:00Z","ended_at":"2026-09-30T00:00:01Z","method":{"name":"corpus-source-retirement","version":"1","configuration":config},"inputs":[{"ref":old,"role":"retired_source","sha256":old_sha},{"ref":review,"role":"source_owner_review","sha256":review_sha}],"outputs":[{"ref":event,"role":"corpus_retirement_event"}],"receipt_refs":[review]});
        let raw = BTreeMap::from([
            (RETIREMENT_SCHEMA.to_owned(), b"{}".to_vec()),
            (review.to_owned(), b"review".to_vec()),
            (event.to_owned(), serde_json::to_vec(&event_value).unwrap()),
        ]);
        let surviving = members(&raw)[RETIREMENT_SCHEMA].clone();
        let mut base = json!({"revision":"base","files":[surviving,{"path":old,"sha256":old_sha,"size_bytes":3,"mode":420}],"identities":{"tos.old":old},"dependencies":{}});
        let retired = vec![json!({"path":old,"sha256":old_sha,"event_ref":event})];
        let ids = with_input(&raw, |input| {
            validate_retirements(input, &retired, Some(&base), &mut |_, _, schema| {
                assert_eq!(schema, b"{}");
                Ok(None)
            })
        })
        .unwrap();
        let index = with_input(&raw, |input| {
            membership_transition(input, &retired, Some(&base), &ids)
        })
        .unwrap()
        .unwrap();
        assert_eq!(
            index.identities,
            BTreeMap::from([("tos.event.retirement".to_owned(), event.to_owned())])
        );
        assert_eq!(index.dependencies[event], vec![review.to_owned()]);
        base["dependencies"] = json!({(RETIREMENT_SCHEMA):[old]});
        assert!(
            with_input(&raw, |input| membership_transition(
                input,
                &retired,
                Some(&base),
                &ids
            ))
            .unwrap_err()
            .to_string()
            .contains("incoming source dependency unresolved")
        );
        base["dependencies"] = json!({});
        base["files"][0]["mode"] = json!(384);
        assert!(
            with_input(&raw, |input| membership_transition(
                input,
                &retired,
                Some(&base),
                &ids
            ))
            .unwrap()
            .is_none()
        );
        assert!(
            with_input(&BTreeMap::new(), |input| validate_retirements(
                input,
                &[],
                None,
                &mut |_, _, _| panic!("empty retirement fastpath")
            ))
            .unwrap()
            .is_empty()
        );
        let mut broken = raw.clone();
        let mut changed = event_value;
        changed["outputs"] = json!([]);
        broken.insert(event.to_owned(), serde_json::to_vec(&changed).unwrap());
        assert!(
            with_input(&broken, |input| validate_retirements(
                input,
                &retired,
                Some(&base),
                &mut |_, _, _| Ok(None)
            ))
            .unwrap_err()
            .to_string()
            .contains("output must name")
        );
    }
}
