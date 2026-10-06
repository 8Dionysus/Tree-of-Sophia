//! Candidate-fenced borrowed input to the SAME native Records validation.
//! This input mints neither a source revision nor a completed inventory token.
use crate::source_admission_spooled_candidate::{CandidateFence, SpoolCandidate};
use std::{
    cell::Cell,
    io,
    sync::{Arc, atomic::AtomicBool},
    time::Instant,
};
use tos_foundation::RelativePath;
use tos_source_store::{PinnedSqliteIoBudget, SourcePresenceV1};
use tos_validation::{
    item_rules::ItemRefusal,
    record_biblio_cut::{
        GeneratedSourceSelection, SourceCutInput, SourceCutInputCoverage,
        SourceCutInputWithIdentity, SourceCutMemberMeta, SourceCutPrefixCoverage,
    },
};

pub(crate) struct CandidateRecordsInput<'a, 'host> {
    candidate: &'a SpoolCandidate<'host>,
    fence: CandidateFence,
    max_member_bytes: usize,
    max_owned_state_bytes: Cell<usize>,
    callback_retained_state_bytes: Cell<usize>,
    record_selection: Option<Arc<tos_validation::source_record_selection::SourceRecordSelection>>,
    generated_selection:
        Option<Arc<super::source_admission_generated_selection::GeneratedCandidateSelectionV1>>,
}
#[track_caller]
fn input_refusal(error: io::Error) -> ItemRefusal {
    let site = std::panic::Location::caller().line();
    ItemRefusal::Source(crate::source_admission_spooled_index::bounded_source_cause(
        "candidate-input",
        &site.to_string(),
        &error.to_string(),
    ))
}
#[track_caller]
fn refused() -> ItemRefusal {
    let site = std::panic::Location::caller().line();
    ItemRefusal::Source(crate::source_admission_spooled_index::bounded_source_cause(
        "candidate-input",
        &site.to_string(),
        "candidate Records input refused",
    ))
}
impl<'a, 'host> CandidateRecordsInput<'a, 'host> {
    pub(crate) fn with_record_selection(
        mut self,
        selection: Option<Arc<tos_validation::source_record_selection::SourceRecordSelection>>,
    ) -> Self {
        self.record_selection = selection;
        self
    }
    fn generated_caller_state(&self) -> Result<usize, ItemRefusal> {
        // Native debits the provider's owned state before deriving this
        // remaining callback envelope. The provider adds its own state once
        // against the original pre-debit input envelope.
        Ok(self.callback_retained_state_bytes.get())
    }
    pub(crate) fn with_generated_selection(
        mut self,
        selection: Option<
            Arc<super::source_admission_generated_selection::GeneratedCandidateSelectionV1>,
        >,
    ) -> Self {
        self.generated_selection = selection;
        self
    }
    pub(crate) fn shares_io_budget(&self, budget: &PinnedSqliteIoBudget) -> bool {
        self.candidate.shares_io_budget(budget)
    }
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
    /// Bootstrap custody consumes the same ledger after this input is lent.
    /// Narrow the inclusive callback/raw envelope by that actual debit before
    /// the evaluator runs, preserving the exact local row reserve. Never grow
    /// either grant or reinterpret a whole operation as a SQL row.
    pub(crate) fn restrict_operation_state(
        &self,
        selected_callback_state_bytes: usize,
        max_operation_state_bytes: usize,
    ) -> Result<usize, ItemRefusal> {
        let owned = self.max_owned_state_bytes.get();
        let callback = self.callback_retained_state_bytes.get();
        if selected_callback_state_bytes != callback {
            self.candidate.abandon();
            return Err(refused());
        }
        let narrowed = owned.min(max_operation_state_bytes);
        let narrowed_callback = callback
            .checked_sub(owned - narrowed)
            .filter(|bytes| *bytes != 0);
        let Some(narrowed_callback) = narrowed_callback else {
            self.candidate.abandon();
            return Err(tos_validation::item_budget_origin!());
        };
        self.max_owned_state_bytes.set(narrowed);
        self.callback_retained_state_bytes.set(narrowed_callback);
        Ok(narrowed_callback)
    }
    pub(crate) fn require_callback_state(
        &self,
        required_state_bytes: usize,
        max_operation_state_bytes: usize,
    ) -> Result<(), ItemRefusal> {
        if self.callback_retained_state_bytes.get() < required_state_bytes {
            self.candidate.abandon();
            return Err(ItemRefusal::BudgetCheck {
                check: "candidate callback retained state",
                used: Some(required_state_bytes as u64),
                limit: Some(self.callback_retained_state_bytes.get() as u64),
            });
        }
        if self.max_owned_state_bytes.get() > max_operation_state_bytes {
            self.candidate.abandon();
            return Err(ItemRefusal::BudgetCheck {
                check: "candidate input operation state",
                used: Some(self.max_owned_state_bytes.get() as u64),
                limit: Some(max_operation_state_bytes as u64),
            });
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
            return Err(tos_validation::item_budget_origin!());
        }
        let fence = candidate.fence().map_err(|error| input_refusal(error))?;
        Ok(Self {
            candidate,
            fence,
            max_member_bytes,
            max_owned_state_bytes: Cell::new(max_owned_state_bytes),
            callback_retained_state_bytes: Cell::new(callback_retained_state_bytes),
            record_selection: None,
            generated_selection: None,
        })
    }
    fn check(&self, deadline: Instant, cancelled: &AtomicBool) -> Result<(), ItemRefusal> {
        if !self.candidate.matches_invocation(deadline, cancelled)
            || self
                .candidate
                .fence()
                .map_err(|error| input_refusal(error))?
                != self.fence
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
            .and_then(|n| n.checked_add(self.callback_retained_state_bytes.get()))
            .and_then(|n| n.checked_add(8192))
            .is_none_or(|n| n > self.max_owned_state_bytes.get())
        {
            return Err(tos_validation::item_budget_origin!());
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
    fn record_selection(
        &self,
    ) -> Option<Arc<tos_validation::source_record_selection::SourceRecordSelection>> {
        self.record_selection.clone()
    }
    fn generated_selection(&self) -> Option<Arc<dyn GeneratedSourceSelection>> {
        self.generated_selection
            .as_ref()
            .map(|selection| Arc::clone(selection) as Arc<dyn GeneratedSourceSelection>)
    }
    fn for_each_current_member_meta(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
        visit: &mut dyn FnMut(SourceCutMemberMeta<'_>) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal> {
        let result = (|| {
            self.check(deadline, cancelled)?;
            let allowance = self
                .max_owned_state_bytes
                .get()
                .checked_sub(self.callback_retained_state_bytes.get())
                .ok_or(tos_validation::item_budget_origin!())?;
            let mut callback_error = None;
            let walked = self
                .candidate
                .for_each_verified_member_metadata(allowance, &mut |meta| {
                    visit(SourceCutMemberMeta {
                        path: meta.path.as_str(),
                        size_bytes: meta.size_bytes,
                    })
                    .map_err(|error| {
                        callback_error = Some(error);
                        io::Error::other("native Records metadata callback refused")
                    })
                });
            if let Some(error) = callback_error {
                return Err(error);
            }
            walked.map_err(|error| {
                ItemRefusal::Source(crate::source_command::public_io_reason(&error))
            })?;
            self.check(deadline, cancelled)
        })();
        if result.is_err() {
            self.candidate.abandon();
        }
        result
    }

    fn for_each_current_member_meta_under(
        &self,
        directory: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
        visit: &mut dyn FnMut(SourceCutMemberMeta<'_>) -> Result<(), ItemRefusal>,
    ) -> Result<SourceCutPrefixCoverage, ItemRefusal> {
        let result = (|| {
            self.check(deadline, cancelled)?;
            let directory_path = self.path(directory)?;
            let allowance = self
                .max_owned_state_bytes
                .get()
                .checked_sub(self.callback_retained_state_bytes.get())
                .ok_or(tos_validation::item_budget_origin!())?;
            let mut previous: Option<RelativePath> = None;
            let mut count = 0u64;
            loop {
                self.check(deadline, cancelled)?;
                let row = self
                    .candidate
                    .member_under_after_bounded(&directory_path, previous.as_ref(), allowance)
                    .map_err(|error| input_refusal(error))?;
                let Some(row) = row else {
                    break;
                };
                if previous
                    .as_ref()
                    .is_some_and(|p| row.path.as_str() <= p.as_str())
                {
                    return Err(refused());
                }
                visit(SourceCutMemberMeta {
                    path: row.path.as_str(),
                    size_bytes: row.size_bytes,
                })?;
                count = count
                    .checked_add(1)
                    .ok_or(tos_validation::item_budget_origin!())?;
                previous = Some(row.path);
            }
            drop(previous);
            self.check(deadline, cancelled)?;
            SourceCutPrefixCoverage::after_verified_prefix_eof(directory, count)
        })();
        if result.is_err() {
            self.candidate.abandon();
        }
        result
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
            // The caller supplies a ceiling, while this adapter retains the
            // verified maximum of the candidate's actual members. Intersect
            // both bounds before reading; a wider caller ceiling must not
            // reject a smaller candidate or expand its allocation allowance.
            let max_bytes = max_bytes.min(self.max_member_bytes);
            if max_bytes == 0 {
                return Err(tos_validation::item_budget_origin!());
            }
            // The caller's policy ceiling may exceed the candidate's observed
            // largest member. Read under their intersection: the source fence
            // and raw-state reserve were established for the observed ceiling.
            let max_bytes = max_bytes.min(self.max_member_bytes);
            let path = self.path(path)?;
            let allowance = self
                .max_owned_state_bytes
                .get()
                .checked_sub(self.callback_retained_state_bytes.get())
                .ok_or(tos_validation::item_budget_origin!())?;
            let member = self
                .candidate
                .read_member_bound(&path, max_bytes, allowance)
                .map_err(|error| input_refusal(error))?;
            if let Some(selection) = &self.generated_selection {
                if selection.selects_member(member.metadata().path.as_str())? {
                    selection.verify_member(
                        member.metadata().path.as_str(),
                        member.raw(),
                        self.generated_caller_state()?,
                        deadline,
                        cancelled,
                    )?;
                }
            }
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
            let mut generated_traversal = self
                .generated_selection
                .as_ref()
                .map(|selection| selection.begin_traversal());
            let caller_live = self.generated_caller_state()?;
            let membership = self.candidate.for_each_verified_member(
                self.max_member_bytes,
                self.max_owned_state_bytes.get(),
                self.callback_retained_state_bytes.get(),
                &mut |meta, raw| {
                    if let (Some(selection), Some(traversal)) =
                        (&self.generated_selection, &mut generated_traversal)
                    {
                        if let Err(error) = selection.observe_member(
                            traversal,
                            meta.path.as_str(),
                            raw,
                            self.fence,
                            caller_live,
                            deadline,
                            cancelled,
                        ) {
                            callback_error = Some(ItemRefusal::Source(
                                crate::source_command::public_io_reason(&error),
                            ));
                            return Err(io::Error::other(
                                "generated physical member proof refused",
                            ));
                        }
                    }
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
            let membership = membership.map_err(|error| input_refusal(error))?;
            self.check(deadline, cancelled)?;
            let (count, bytes) = self.candidate.membership_counts();
            let coverage = SourceCutInputCoverage::after_verified_eof(membership, count, bytes);
            if let (Some(selection), Some(traversal)) =
                (&self.generated_selection, generated_traversal)
            {
                selection
                    .finish_traversal(traversal, self.fence, &coverage, deadline, cancelled)
                    .map_err(|error| {
                        ItemRefusal::Source(crate::source_command::public_io_reason(&error))
                    })?;
            }
            Ok(coverage)
        })();
        if result.is_err() {
            self.candidate.abandon();
        }
        result
    }
    fn current_member_size(
        &self,
        path: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<u64>, ItemRefusal> {
        let result = (|| {
            self.check(deadline, cancelled)?;
            let relative = self.path(path)?;
            let overlap = path
                .len()
                .checked_mul(16)
                .and_then(|n| n.checked_add(self.callback_retained_state_bytes.get()))
                .and_then(|n| n.checked_add(8192))
                .ok_or(tos_validation::item_budget_origin!())?;
            let allowance = self
                .max_owned_state_bytes
                .get()
                .checked_sub(overlap)
                .ok_or(tos_validation::item_budget_origin!())?;
            let size = self
                .candidate
                .member_bounded(&relative, allowance)
                .map_err(|error| input_refusal(error))?
                .map(|member| member.size_bytes);
            self.check(deadline, cancelled)?;
            Ok(size)
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
                .and_then(|n| n.checked_add(self.callback_retained_state_bytes.get()))
                .and_then(|n| n.checked_add(8192))
                .ok_or(tos_validation::item_budget_origin!())?;
            let allowance = self
                .max_owned_state_bytes
                .get()
                .checked_sub(overlap)
                .ok_or(tos_validation::item_budget_origin!())?;
            let presence = if self
                .candidate
                .member_bounded(&relative, allowance)
                .map_err(|error| input_refusal(error))?
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
                        .ok_or(tos_validation::item_budget_origin!())?;
                    let row_allowance = self
                        .max_owned_state_bytes
                        .get()
                        .checked_sub(previous)
                        .ok_or(tos_validation::item_budget_origin!())?;
                    let Some(next) = self
                        .candidate
                        .member_after_bounded(Some(&after), row_allowance)
                        .map_err(|error| input_refusal(error))?
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
