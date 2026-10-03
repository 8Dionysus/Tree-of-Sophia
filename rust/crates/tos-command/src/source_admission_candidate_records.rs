//! Candidate-fenced borrowed input to the SAME native Records validation.
//! This input mints neither a source revision nor a completed inventory token.
use crate::source_admission_spooled_candidate::{CandidateFence, SpoolCandidate};
use std::{io, sync::atomic::AtomicBool, time::Instant};
use tos_foundation::RelativePath;
use tos_source_store::SourcePresenceV1;
use tos_validation::{
    item_rules::ItemRefusal,
    record_biblio_cut::{
        SourceCutInput, SourceCutInputCoverage, SourceCutInputWithIdentity, SourceCutMemberMeta,
    },
};

pub(crate) struct CandidateRecordsInput<'a, 'host> {
    candidate: &'a SpoolCandidate<'host>,
    fence: CandidateFence,
    max_member_bytes: usize,
    max_owned_state_bytes: usize,
    callback_retained_state_bytes: usize,
}
fn refused() -> ItemRefusal {
    ItemRefusal::Source("candidate Records input refused".into())
}
impl<'a, 'host> CandidateRecordsInput<'a, 'host> {
    pub(crate) fn abandon(&self) {
        self.candidate.abandon();
    }
    pub(crate) fn verify_invocation(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), ItemRefusal> {
        self.check(deadline, cancelled)
    }
    pub(crate) fn require_callback_state(
        &self,
        required_state_bytes: usize,
        max_operation_state_bytes: usize,
    ) -> Result<(), ItemRefusal> {
        if self.callback_retained_state_bytes < required_state_bytes
            || self.max_owned_state_bytes > max_operation_state_bytes
        {
            self.candidate.abandon();
            return Err(ItemRefusal::Budget);
        }
        Ok(())
    }
    pub(crate) fn new(
        candidate: &'a SpoolCandidate<'host>,
        max_member_bytes: usize,
        max_owned_state_bytes: usize,
        callback_retained_state_bytes: usize,
    ) -> Result<Self, ItemRefusal> {
        if max_member_bytes == 0
            || max_member_bytes
                .checked_mul(4)
                .and_then(|n| n.checked_add(callback_retained_state_bytes))
                .and_then(|n| n.checked_add(16384))
                .is_none_or(|n| n > max_owned_state_bytes)
        {
            return Err(ItemRefusal::Budget);
        }
        let fence = candidate.fence().map_err(|_| refused())?;
        Ok(Self {
            candidate,
            fence,
            max_member_bytes,
            max_owned_state_bytes,
            callback_retained_state_bytes,
        })
    }
    fn check(&self, deadline: Instant, cancelled: &AtomicBool) -> Result<(), ItemRefusal> {
        if !self.candidate.matches_invocation(deadline, cancelled)
            || self.candidate.fence().map_err(|_| refused())? != self.fence
        {
            self.candidate.abandon();
            return Err(refused());
        }
        Ok(())
    }
    fn path(&self, path: &str) -> Result<RelativePath, ItemRefusal> {
        // Charge the caller's path, owned parser path and SQLite bind overlap
        // BEFORE constructing RelativePath. Pager I/O remains separately shared.
        if path
            .len()
            .checked_mul(16)
            .and_then(|n| n.checked_add(self.callback_retained_state_bytes))
            .and_then(|n| n.checked_add(8192))
            .is_none_or(|n| n > self.max_owned_state_bytes)
        {
            return Err(ItemRefusal::Budget);
        }
        RelativePath::parse(path).map_err(|_| refused())
    }
}
impl SourceCutInputWithIdentity<CandidateFence> for CandidateRecordsInput<'_, '_> {
    fn input_identity(&self) -> &CandidateFence {
        &self.fence
    }
    fn source_input(&self) -> &dyn SourceCutInput {
        self
    }
}
impl SourceCutInput for CandidateRecordsInput<'_, '_> {
    fn for_each_current_member_meta(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
        visit: &mut dyn FnMut(SourceCutMemberMeta<'_>) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal> {
        // This initial bridge deliberately verifies full raw bytes even for
        // metadata traversal. Its physical reads are charged, never inferred.
        self.for_each_current_member(deadline, cancelled, &mut |meta, _| visit(meta))?;
        Ok(())
    }
    fn with_current_member(
        &self,
        path: &str,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
        visit: &mut dyn FnMut(SourceCutMemberMeta<'_>, &[u8]) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal> {
        let result = (|| {
            self.check(deadline, cancelled)?;
            if max_bytes == 0 || max_bytes > self.max_member_bytes {
                return Err(ItemRefusal::Budget);
            }
            let path = self.path(path)?;
            let allowance = self
                .max_owned_state_bytes
                .checked_sub(self.callback_retained_state_bytes)
                .ok_or(ItemRefusal::Budget)?;
            let member = self
                .candidate
                .read_member_bound(&path, max_bytes, allowance)
                .map_err(|_| refused())?;
            visit(
                SourceCutMemberMeta {
                    path: member.metadata().path.as_str(),
                    size_bytes: member.metadata().size_bytes,
                },
                member.raw(),
            )?;
            drop(member);
            drop(path);
            self.check(deadline, cancelled)
        })();
        if result.is_err() {
            self.candidate.abandon();
        }
        result
    }
    fn for_each_current_member(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
        visit: &mut dyn FnMut(SourceCutMemberMeta<'_>, &[u8]) -> Result<(), ItemRefusal>,
    ) -> Result<SourceCutInputCoverage, ItemRefusal> {
        let result = (|| {
            self.check(deadline, cancelled)?;
            let mut callback_error = None;
            let membership = self.candidate.for_each_verified_member(
                self.max_member_bytes,
                self.max_owned_state_bytes,
                self.callback_retained_state_bytes,
                &mut |meta, raw| {
                    visit(
                        SourceCutMemberMeta {
                            path: meta.path.as_str(),
                            size_bytes: meta.size_bytes,
                        },
                        raw,
                    )
                    .map_err(|error| {
                        callback_error = Some(error);
                        io::Error::other("native Records callback refused")
                    })
                },
            );
            if let Some(error) = callback_error {
                return Err(error);
            }
            let membership = membership.map_err(|_| refused())?;
            self.check(deadline, cancelled)?;
            let (count, bytes) = self.candidate.membership_counts();
            Ok(SourceCutInputCoverage::after_verified_eof(
                membership, count, bytes,
            ))
        })();
        if result.is_err() {
            self.candidate.abandon();
        }
        result
    }
    fn path_presence(
        &self,
        path: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<SourcePresenceV1>, ItemRefusal> {
        let result = (|| {
            self.check(deadline, cancelled)?;
            let relative = self.path(path)?;
            let overlap = path
                .len()
                .checked_mul(16)
                .and_then(|n| n.checked_add(self.callback_retained_state_bytes))
                .and_then(|n| n.checked_add(8192))
                .ok_or(ItemRefusal::Budget)?;
            let allowance = self
                .max_owned_state_bytes
                .checked_sub(overlap)
                .ok_or(ItemRefusal::Budget)?;
            let presence = if self
                .candidate
                .member_bounded(&relative, allowance)
                .map_err(|_| refused())?
                .is_some()
            {
                Some(SourcePresenceV1::File)
            } else {
                let mut after = relative;
                loop {
                    let previous = after
                        .as_str()
                        .len()
                        .checked_mul(16)
                        .and_then(|n| n.checked_add(overlap))
                        .ok_or(ItemRefusal::Budget)?;
                    let row_allowance = self
                        .max_owned_state_bytes
                        .checked_sub(previous)
                        .ok_or(ItemRefusal::Budget)?;
                    let Some(next) = self
                        .candidate
                        .member_after_bounded(Some(&after), row_allowance)
                        .map_err(|_| refused())?
                    else {
                        break None;
                    };
                    if !next.path.as_str().starts_with(path) {
                        break None;
                    }
                    match next.path.as_str().as_bytes().get(path.len()) {
                        Some(b'/') => break Some(SourcePresenceV1::MaterializedDirectory),
                        Some(byte) if *byte < b'/' => after = next.path,
                        _ => break None,
                    }
                }
            };
            self.check(deadline, cancelled)?;
            Ok(presence)
        })();
        if result.is_err() {
            self.candidate.abandon();
        }
        result
    }
    fn verify_current_fence(
        &self,
        coverage: &SourceCutInputCoverage,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), ItemRefusal> {
        self.check(deadline, cancelled)?;
        let (count, bytes) = self.candidate.membership_counts();
        if coverage.membership() != self.fence.membership
            || coverage.member_count() != count
            || coverage.source_bytes_read() != bytes
        {
            self.candidate.abandon();
            return Err(refused());
        }
        self.check(deadline, cancelled)
    }
}
