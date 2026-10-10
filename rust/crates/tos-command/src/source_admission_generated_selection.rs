//! Live generated-byte selection over the actual held CandidateFence.
//! Completed byte coverage does not issue semantic acceptance.
use super::source_admission_spooled_candidate::CandidateFence;
use super::source_capacity_workload::{
    WeightedScaleGeneratedAllV1, WeightedScaleGeneratedEofV1, WeightedScaleGeneratedTraversalV1,
};
use std::{io, sync::atomic::AtomicBool, time::Instant};
use tos_foundation::Digest256;
use tos_validation::{
    item_rules::ItemRefusal,
    record_biblio_cut::{GeneratedSourceSelection, SourceCutInputCoverage},
};

pub(crate) struct GeneratedCandidateSelectionV1 {
    provider: WeightedScaleGeneratedAllV1,
    fence: CandidateFence,
    retained_state_bytes: usize,
}
impl GeneratedCandidateSelectionV1 {
    pub(crate) fn new(
        provider: WeightedScaleGeneratedAllV1,
        fence: CandidateFence,
    ) -> io::Result<Self> {
        // Provider's issued state already includes its own header; only the
        // wrapper's additional inline custody is added. Arc ownership is charged
        // by the existing Candidate/Native owner at construction.
        let retained_state_bytes = provider
            .retained_state_bytes()
            .checked_add(
                std::mem::size_of::<Self>() - std::mem::size_of::<WeightedScaleGeneratedAllV1>(),
            )
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "generated wrapper state overflow",
                )
            })?;
        provider.verify_candidate_fence(fence)?;
        Ok(Self {
            provider,
            fence,
            retained_state_bytes,
        })
    }
    pub(crate) fn begin_traversal(&self) -> WeightedScaleGeneratedTraversalV1 {
        self.provider.begin_traversal()
    }
    pub(crate) fn observe_member(
        &self,
        traversal: &mut WeightedScaleGeneratedTraversalV1,
        path: &str,
        raw: &[u8],
        actual_fence: CandidateFence,
        caller_live_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> io::Result<()> {
        self.provider
            .observe_member(
                traversal,
                path,
                raw,
                actual_fence,
                caller_live_state_bytes
                    .checked_add(self.retained_state_bytes - self.provider.retained_state_bytes())
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            "generated wrapper caller state overflow",
                        )
                    })?,
                deadline,
                cancelled,
            )
            .map(|_| ())
    }
    pub(crate) fn finish_traversal(
        &self,
        traversal: WeightedScaleGeneratedTraversalV1,
        actual_fence: CandidateFence,
        coverage: &SourceCutInputCoverage,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> io::Result<WeightedScaleGeneratedEofV1> {
        if actual_fence != self.fence
            || coverage.membership() != self.fence.membership
            || coverage.member_count() != self.fence.membership.count
            || coverage.source_bytes_read() != self.fence.source_bytes
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "generated physical EOF coverage differs",
            ));
        }
        self.provider
            .finish_traversal(traversal, actual_fence, deadline, cancelled)
    }
}
fn refusal(error: io::Error) -> ItemRefusal {
    if error.kind() == io::ErrorKind::TimedOut {
        ItemRefusal::Deadline
    } else {
        ItemRefusal::Source(super::source_command::public_io_reason(&error))
    }
}
impl GeneratedSourceSelection for GeneratedCandidateSelectionV1 {
    fn binding_digest(&self) -> Digest256 {
        self.provider.selection_sha256()
    }
    fn declared_member_count(&self) -> u64 {
        self.provider.declared_generated_count()
    }
    fn retained_state_bytes(&self) -> usize {
        self.retained_state_bytes
    }
    fn selects_member(&self, path: &str) -> Result<bool, ItemRefusal> {
        self.provider.selects_semantic_member(path).map_err(refusal)
    }
    fn selects_required_member(&self, path: &str) -> Result<bool, ItemRefusal> {
        self.provider.selects_required_member(path).map_err(refusal)
    }
    fn selects_catalog_member(&self, path: &str) -> Result<bool, ItemRefusal> {
        self.provider.selects_record(path).map_err(refusal)
    }
    fn selects_claim_row(&self, path: &str, physical_line: u64) -> Result<bool, ItemRefusal> {
        self.provider
            .selects_claim_row(path, physical_line)
            .map_err(refusal)
    }
    fn selects_row(&self, path: &str, physical_line: u64) -> Result<bool, ItemRefusal> {
        self.provider
            .selects_semantic_row(path, physical_line)
            .map_err(refusal)
    }
    fn verify_member(
        &self,
        path: &str,
        raw: &[u8],
        caller_live_state_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), ItemRefusal> {
        let caller_live = caller_live_state_bytes
            .checked_add(self.retained_state_bytes - self.provider.retained_state_bytes())
            .ok_or(ItemRefusal::Budget)?;
        self.provider
            .verify_selected_member(path, raw, self.fence, caller_live, deadline, cancelled)
            .map(|_| ())
            .map_err(refusal)
    }
}
