//! Resource-only original Session adapter for protected physical rereads.
//!
//! The selected owner retains its FD, path and permissions. This adapter
//! opens/clones/seeks/reads no FD in a callback and issues no source authority.
//! Its logical raw-reader holders and requested allocation layouts are not a
//! complete quote for outer owner/path/metadata/driver/activation or allocator
//! RSS. Those remain explicit protected-caller funding obligations.

use super::OriginalAssessmentPayer;
use crate::source_assessment_journal::ProtectedAssessmentPgGrant;
use crate::source_command::{SourceCommandError, SourceCommandResult};
use crate::source_creation_store::{CreationReadAdmission, CreationReadStage, ReadLength};
use std::{
    fs::{File, Metadata},
    mem::size_of,
};
use tos_compiler::pwe_native::{Error, Session};

/// The SAME original payer and cumulative source Session, borrowed only for
/// this physical read. No operation-budget loan is held across foreign I/O.
pub(crate) struct OriginalProtectedReads<'loan, 'pure, 'budget, 'cancel> {
    payer: &'loan OriginalAssessmentPayer<'budget, 'cancel>,
    session: &'loan mut Session<'pure>,
}

impl<'loan, 'pure, 'budget, 'cancel> OriginalProtectedReads<'loan, 'pure, 'budget, 'cancel> {
    pub(crate) fn new(
        payer: &'loan OriginalAssessmentPayer<'budget, 'cancel>,
        session: &'loan mut Session<'pure>,
    ) -> SourceCommandResult<Self> {
        payer.charge_sto_source_measures(session, 0, 0, 0, 0, 0)?;
        // Before Self and the reader's first event. The enum is a single
        // nonrecursive raw-reader argument slot, reused across loop events;
        // the adapter retains neither the event nor the supplied descriptor.
        let fixed = size_of::<Self>()
            .checked_add(size_of::<SourceCommandResult<Self>>())
            .and_then(|n| n.checked_add(size_of::<CreationReadStage>()))
            .and_then(|n| n.checked_add(size_of::<ReadLength>()))
            .and_then(|n| n.checked_add(size_of::<Option<&mut dyn CreationReadAdmission>>()))
            .and_then(|n| n.checked_add(size_of::<SourceCommandResult<()>>()))
            // raw obtains its first metadata BEFORE Begin; those holders
            // therefore belong to this earlier bootstrap, not the callback.
            .and_then(|n| n.checked_add(size_of::<Vec<u8>>()))
            .and_then(|n| n.checked_add(2 * size_of::<Metadata>()))
            .and_then(|n| n.checked_add(size_of::<std::io::Result<usize>>()))
            .and_then(|n| n.checked_add(size_of::<SourceCommandResult<Vec<u8>>>()))
            .and_then(|n| n.checked_add(2 * size_of::<usize>()))
            // Additional profile flags at payer/owner/raw layers, exact
            // wrapper expected length, next request and sliced read argument. These
            // logical typed carriers precede raw's first metadata/plan event;
            // full foreign call-frame/RSS/allocator funding remains open.
            .and_then(|n| n.checked_add(3 * size_of::<bool>()))
            .and_then(|n| n.checked_add(2 * size_of::<usize>()))
            .and_then(|n| n.checked_add(size_of::<&mut [u8]>()))
            .and_then(|n| n.checked_add(size_of::<Option<SourceCommandResult<Vec<u8>>>>()))
            .and_then(|n| n.checked_add(size_of::<Option<SourceCommandResult<()>>>()))
            .and_then(|n| n.checked_add(2 * size_of::<bool>()))
            .and_then(|n| {
                n.checked_add(size_of::<
                    &mut dyn FnMut(&mut [u8], &mut dyn CreationReadAdmission),
                >())
            })
            .and_then(|n| {
                n.checked_add(size_of::<(
                    &mut File,
                    ReadLength,
                    std::time::Instant,
                    &std::sync::atomic::AtomicBool,
                    &Metadata,
                    bool,
                )>())
            })
            .ok_or_else(|| payer.error(Error::ResourceLimit))?;
        payer.charge_sto_source_measures(session, 0, fixed, 0, 0, 0)?;
        Ok(Self { payer, session })
    }

    /// Resource exit only; the actual protected owner still owes its current
    /// selection/permission/source exit. Preserve the first body error even
    /// when the same original resource cutoff also refuses.
    pub(crate) fn complete<T>(&mut self, result: SourceCommandResult<T>) -> SourceCommandResult<T> {
        let cutoff = self
            .payer
            .charge_sto_source_measures(self.session, 0, 0, 0, 0, 0);
        match (result, cutoff) {
            (Err(original), _) | (Ok(_), Err(original)) => Err(original),
            (Ok(value), Ok(())) => Ok(value),
        }
    }
}

impl OriginalAssessmentPayer<'_, '_> {
    /// No buffer or mutable Session escapes. The single private payer is
    /// constructed with exactly one lexical Session; every use checks it.
    /// Allocation remains paid after Err and is destroyed only with the payer.
    fn with_protected_workspace<T>(
        &self,
        session: &mut Session<'_>,
        run: impl FnOnce(&mut [u8], &mut Session<'_>) -> SourceCommandResult<T>,
    ) -> SourceCommandResult<T> {
        let body = (|| {
            self.charge_sto_source_measures(session, 0, 0, 0, 0, 0)?;
            // Local reference/control/Result/callback holders precede borrowing.
            // Requested allocator layouts, not full allocator/native/RSS funding.
            let holders = size_of::<std::cell::RefMut<'_, Option<Vec<u8>>>>()
                .checked_add(size_of::<SourceCommandResult<T>>())
                .and_then(|n| n.checked_add(std::mem::size_of_val(&run)))
                .and_then(|n| n.checked_add(size_of::<Option<Vec<u8>>>()))
                .and_then(|n| n.checked_add(size_of::<std::collections::TryReserveError>()))
                .and_then(|n| n.checked_add(size_of::<SourceCommandResult<()>>()))
                .ok_or_else(|| self.error(Error::ResourceLimit))?;
            self.charge_sto_source_measures(session, 0, holders, 0, 0, 1)?;
            let mut stored = self.protected_scratch.try_borrow_mut().map_err(|_| {
                SourceCommandError::Conflict("original protected workspace already borrowed")
            })?;
            if stored.is_none() {
                // ONE requested 65536-byte RawVec layout before allocation, plus
                // the full initialization write work before resize. Not a refund
                // or high-water correction to earlier event reservations.
                self.charge_sto_source_measures(session, 0, 65536, 0, 65536, 2)?;
                let mut buffer = Vec::new();
                buffer
                    .try_reserve_exact(65536)
                    .map_err(|_| self.error(Error::ResourceLimit))?;
                buffer.resize(65536, 0);
                *stored = Some(buffer);
            }
            let buffer = stored
                .as_mut()
                .ok_or(SourceCommandError::Invalid("protected workspace absent"))?;
            run(buffer.as_mut_slice(), session)
        })();
        // The closure drops the sequential borrow on every Result exit. The physical
        // allocation is retained, and the original paid counters never reset.
        let exit = self.charge_sto_source_measures(session, 0, 0, 0, 0, 0);
        match (body, exit) {
            (Err(original), _) | (Ok(_), Err(original)) => Err(original),
            (Ok(value), Ok(())) => Ok(value),
        }
    }
}

impl CreationReadAdmission for OriginalProtectedReads<'_, '_, '_, '_> {
    fn with_creation_read_workspace(
        &mut self,
        run: &mut dyn FnMut(&mut [u8], &mut dyn CreationReadAdmission),
    ) -> Option<SourceCommandResult<()>> {
        let payer = self.payer;
        Some(
            payer.with_protected_workspace(self.session, |buffer, same| {
                // A lexical reborrow of the same original Session, not a ledger.
                let mut reader = OriginalProtectedReads::new(payer, same)?;
                run(buffer, &mut reader);
                // The physical owner keeps its Result<Vec>; this lender still
                // attempts its real resource exit, independently of body success.
                reader.complete(Ok(()))
            }),
        )
    }

    fn before_creation_exact_read_init(&mut self) -> SourceCommandResult<()> {
        // Seven declared entry groups: exact-profile dispatch, metadata call,
        // metadata error branch, regular predicate, length read/conversion,
        // expected-length comparison and failure decision. This is a bounded
        // source-operation quote, not CPU/kernel/RSS or a grant.
        self.payer
            .charge_sto_source_measures(self.session, 0, 0, 0, 0, 7)
    }

    fn before_creation_exact_request_plan(&mut self) -> SourceCommandResult<()> {
        // Six declared source groups: checked cap+sentinel, output length,
        // checked remainder, nonzero filter, failure branch and slice clamp.
        // Resource work only, BEFORE that arithmetic, never nodes/input/state
        // as a proxy or a claim about machine instructions.
        self.payer
            .charge_sto_source_measures(self.session, 0, 0, 0, 0, 6)
    }

    fn before_creation_read(
        &mut self,
        _descriptor: &File,
        stage: CreationReadStage,
    ) -> SourceCommandResult<()> {
        // The raw owner, not this callback, selects/validates and reads the
        // actual supplied FD immediately after admission. No FD operation,
        // private JSON parse, worker or source dispatch occurs here.
        let (requested, owned, scratch, work) = match stage {
            CreationReadStage::Begin { scratch_bytes, .. } => {
                // The fixed raw slots were paid BEFORE the first metadata.
                // Pay this actual scratch before raw constructs its buffer.
                // Outer path traversal and grant ownership are not included.
                (0, 0, scratch_bytes, 0)
            }
            CreationReadStage::Read { requested_bytes } => {
                // Charge the ACTUAL entire request before syscall, including
                // EOF, I/O Err and each short-read retry. Never infer request
                // size from a cached file length or returned-byte count.
                // admit_source_prefix already spends these requested bytes
                // as input AND work. Do not debit them a second time as an
                // extra byte_work measurement.
                let requested = u64::try_from(requested_bytes)
                    .map_err(|_| self.payer.error(Error::ResourceLimit))?;
                (requested, 0, 0, 0)
            }
            CreationReadStage::Retain {
                length,
                capacity,
                additional,
            } => {
                // Same normative pinned Rust1.98.1 RawVec growth model as the
                // immutable original reader, not a newly invented multiplier.
                // Old backing remains paid; reserve the WHOLE new requested
                // backing before extend and pay old-copy plus incoming copy.
                let (new_owned, copied) =
                    super::immutable_read::growth(length, capacity, additional, size_of::<u8>())
                        .map_err(|error| self.payer.error(error))?;
                let copied = copied
                    .checked_add(additional)
                    .ok_or_else(|| self.payer.error(Error::ResourceLimit))?;
                let copied =
                    u64::try_from(copied).map_err(|_| self.payer.error(Error::ResourceLimit))?;
                (0, new_owned, 0, copied)
            }
        };
        // One source event (work), never nodes/refs/input masquerading as work.
        // This is an explicit event currency, not a native instruction bound.
        self.payer
            .charge_sto_source_measures(self.session, requested, owned, scratch, work, 1)
    }
}

impl crate::source_text_owner::ProtectedContextReadAdmission
    for OriginalProtectedReads<'_, '_, '_, '_>
{
    fn with_protected_context_workspace(
        &mut self,
        run: &mut dyn FnMut(
            &mut [u8],
            &mut dyn crate::source_text_owner::ProtectedContextReadAdmission,
        ),
    ) -> Option<SourceCommandResult<()>> {
        let payer = self.payer;
        Some(
            payer.with_protected_workspace(self.session, |buffer, same| {
                let mut reader = OriginalProtectedReads::new(payer, same)?;
                run(buffer, &mut reader);
                // The physical owner keeps its Result<Vec>; this lender still
                // attempts its real resource exit, independently of body success.
                reader.complete(Ok(()))
            }),
        )
    }

    fn before_protected_context_work(&mut self, bytes: usize) -> SourceCommandResult<()> {
        let work = u64::try_from(bytes).map_err(|_| self.payer.error(Error::ResourceLimit))?;
        self.payer
            .charge_sto_source_measures(self.session, 0, 0, 0, work, 1)
    }
}

impl crate::source_text_owner::PrivateScopeBindingAdmission
    for OriginalProtectedReads<'_, '_, '_, '_>
{
    fn retain_private_scope_binding(&mut self, bytes: usize) -> SourceCommandResult<()> {
        self.payer
            .charge_sto_source_measures(self.session, 0, bytes, 0, 0, 0)
    }
    fn inspect_private_scope_binding(
        &mut self,
        bytes: usize,
        events: usize,
    ) -> SourceCommandResult<()> {
        let bytes = u64::try_from(bytes).map_err(|_| self.payer.error(Error::ResourceLimit))?;
        let events = u64::try_from(events).map_err(|_| self.payer.error(Error::ResourceLimit))?;
        self.payer
            .charge_sto_source_measures(self.session, 0, 0, 0, bytes, events)
    }
}

impl OriginalAssessmentPayer<'_, '_> {
    /// Separate admitted port for the genuine legacy owner-private scope
    /// identity. d7's context recheck did not pay this additional hash recipe.
    pub(crate) fn private_owner_scope_binding_source(
        &self,
        session: &mut Session<'_>,
        owner: &crate::source_text_owner::OwnerTextContext,
    ) -> SourceCommandResult<tos_foundation::Digest256> {
        let (deadline, cancelled) = self.source_cutoff();
        let mut reader = OriginalProtectedReads::new(self, session)?;
        let result = owner.private_owner_scope_binding_with_exact_admission(
            deadline,
            cancelled,
            &mut reader,
        );
        reader.complete(result)
    }
}

impl OriginalAssessmentPayer<'_, '_> {
    /// Actual protected-grant reread on the SAME source Session. The selected
    /// grant performs its UID/mode/parents/inode/raw checks and uses this
    /// callback for its already-opened FD. This is not yet the whole protected
    /// Assessment operation or permission/semantic/PG role admission.
    pub(crate) fn recheck_current_pg_grant_source(
        &self,
        session: &mut Session<'_>,
        grant: &ProtectedAssessmentPgGrant,
    ) -> SourceCommandResult<()> {
        self.recheck_current_pg_grant_source_using(session, grant, false)
    }

    pub(crate) fn recheck_current_pg_grant_exact_source(
        &self,
        session: &mut Session<'_>,
        grant: &ProtectedAssessmentPgGrant,
    ) -> SourceCommandResult<()> {
        self.recheck_current_pg_grant_source_using(session, grant, true)
    }

    fn recheck_current_pg_grant_source_using(
        &self,
        session: &mut Session<'_>,
        grant: &ProtectedAssessmentPgGrant,
        exact_requests: bool,
    ) -> SourceCommandResult<()> {
        let (deadline, cancelled) = self.source_cutoff();
        let mut reader = OriginalProtectedReads::new(self, session)?;
        // The owner compares its newly read Vec with this immutable retained
        // raw value. Equal-length comparison can inspect BOTH full operands;
        // a length mismatch is cheaper. Pay that bound before the actual read.
        let comparison = grant
            .configuration_raw()
            .len()
            .checked_mul(2)
            .and_then(|n| u64::try_from(n).ok())
            .ok_or_else(|| self.error(Error::ResourceLimit))?;
        self.charge_sto_source_measures(reader.session, 0, 0, 0, comparison, 0)?;
        let result = if exact_requests {
            grant.verify_current_with_exact_read_admission(deadline, cancelled, &mut reader)
        } else {
            grant.verify_current_with_read_admission(deadline, cancelled, &mut reader)
        };
        reader.complete(result)
    }

    /// Actual owner-context FD/current-path/root rechecks. This retains all
    /// held-file byte/digest checks and pays real reread/compare/hash work on
    /// the SAME original Session, without a new pinned-I/O wallet. Outer
    /// context selection and complete native path/activation cost remain open.
    pub(crate) fn recheck_owner_context_source(
        &self,
        session: &mut Session<'_>,
        owner: &crate::source_text_owner::OwnerTextContext,
    ) -> SourceCommandResult<()> {
        self.recheck_owner_context_source_using(session, owner, false)
    }

    pub(crate) fn recheck_owner_context_exact_source(
        &self,
        session: &mut Session<'_>,
        owner: &crate::source_text_owner::OwnerTextContext,
    ) -> SourceCommandResult<()> {
        self.recheck_owner_context_source_using(session, owner, true)
    }

    fn recheck_owner_context_source_using(
        &self,
        session: &mut Session<'_>,
        owner: &crate::source_text_owner::OwnerTextContext,
        exact_requests: bool,
    ) -> SourceCommandResult<()> {
        let (deadline, cancelled) = self.source_cutoff();
        let mut reader = OriginalProtectedReads::new(self, session)?;
        let result = if exact_requests {
            owner.verify_context_base_with_exact_read_admission(deadline, cancelled, &mut reader)
        } else {
            owner.verify_context_base_with_read_admission(deadline, cancelled, &mut reader)
        };
        reader.complete(result)
    }
}

#[cfg(test)]
mod tests {
    use super::super::with_original_pwe_session;
    use super::*;
    use crate::source_creation_store::raw_with_length_limit_and_admission;
    use crate::source_native_cli::private_assessment::OperationBudget;
    use std::{
        path::PathBuf,
        sync::atomic::AtomicBool,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };
    use tos_compiler::pwe_native::Limits;

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "tos-original-protected-read-{}-{stamp}",
                std::process::id(),
            ));
            std::fs::create_dir(&path).unwrap();
            std::fs::write(path.join("input"), b"xy").unwrap();
            Self(path)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn limits(deadline: Instant) -> Limits {
        Limits {
            max_input_bytes: 262_144,
            max_nodes: 32,
            max_work: 262_144,
            max_refs: 8,
            max_depth: 8,
            max_items: 8,
            max_state_bytes: 1_048_576,
            deadline,
        }
    }
    fn budget(deadline: Instant) -> OperationBudget {
        OperationBudget::new(
            &serde_json::json!({"budgets":{
                "worker_cpu_seconds":1,"worker_address_space_bytes":1048576
            }}),
            deadline,
        )
        .unwrap()
    }
    #[test]
    fn protected_workspace_reuses_one_allocation_for_132_actual_fd_reads() {
        let fixture = Fixture::new();
        let deadline = Instant::now() + Duration::from_secs(30);
        let cancelled = AtomicBool::new(false);
        let mut original = budget(deadline);
        let profile = limits(deadline); // unchanged 1MiB state/262144 work
        with_original_pwe_session(&mut original, profile, &cancelled, |payer, same| {
            let mut address = None;
            let mut first_state = 0;
            for index in 0..132 {
                let mut file = File::open(fixture.0.join("input")).unwrap();
                let mut reader = OriginalProtectedReads::new(payer, same)?;
                let body = crate::source_creation_store::raw_exact_with_original_admission(
                    &mut file,
                    2,
                    deadline,
                    &cancelled,
                    &mut reader,
                );
                assert_eq!(reader.complete(body)?, b"xy");
                drop(reader);
                let stored = payer.protected_scratch.borrow();
                let buffer = stored.as_ref().expect("actual retained scratch allocation");
                assert_eq!(buffer.len(), 65536);
                let current = buffer.as_ptr();
                if let Some(expected) = address {
                    assert_eq!(current, expected);
                } else {
                    address = Some(current);
                    first_state = same.charges().4;
                }
                if index == 1 {
                    assert!(
                        same.charges().4 > first_state,
                        "new reader/output holders remain additive"
                    );
                    assert!(
                        same.charges().4 - first_state < 65536,
                        "no second scratch reservation"
                    );
                }
            }
            assert_eq!(
                same.charges().0,
                132 * 4,
                "every actual 3-byte slice and EOF request is still paid"
            );
            assert_eq!(same.charges().1, 0);
            assert_eq!(same.charges().3, 0);
            assert!(same.charges().4 < profile.max_state_bytes);
            assert!(same.has_source_payer(payer));
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn protected_workspace_releases_after_actual_io_error_before_next_read() {
        let fixture = Fixture::new();
        let deadline = Instant::now() + Duration::from_secs(30);
        let cancelled = AtomicBool::new(false);
        let mut original = budget(deadline);
        with_original_pwe_session(
            &mut original,
            limits(deadline),
            &cancelled,
            |payer, same| {
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .open(fixture.0.join("input"))
                    .unwrap();
                let mut reader = OriginalProtectedReads::new(payer, same)?;
                let body = crate::source_creation_store::raw_exact_with_original_admission(
                    &mut file,
                    2,
                    deadline,
                    &cancelled,
                    &mut reader,
                );
                assert!(matches!(
                    reader.complete(body),
                    Err(SourceCommandError::Invalid("creation descriptor read"))
                ));
                drop(reader);
                let address = payer.protected_scratch.borrow().as_ref().unwrap().as_ptr();
                let mut file = File::open(fixture.0.join("input")).unwrap();
                let mut reader = OriginalProtectedReads::new(payer, same)?;
                let body = crate::source_creation_store::raw_exact_with_original_admission(
                    &mut file,
                    2,
                    deadline,
                    &cancelled,
                    &mut reader,
                );
                assert_eq!(reader.complete(body)?, b"xy");
                drop(reader);
                assert_eq!(
                    payer.protected_scratch.borrow().as_ref().unwrap().as_ptr(),
                    address
                );
                assert_eq!(
                    same.charges().0,
                    3 + 4,
                    "failed and successful actual requests remain spent"
                );
                Ok(())
            },
        )
        .unwrap();
    }

    #[test]
    fn protected_workspace_refuses_nested_borrow_and_preserves_first_body_error() {
        let deadline = Instant::now() + Duration::from_secs(30);
        let cancelled = AtomicBool::new(false);
        let mut original = budget(deadline);
        let result: SourceCommandResult<()> = with_original_pwe_session(
            &mut original,
            limits(deadline),
            &cancelled,
            |payer, same| {
                payer.with_protected_workspace(same, |outer, same| {
                    outer[0] = 19;
                    let prefix = same.charges();
                    let nested: SourceCommandResult<()> = payer
                        .with_protected_workspace(same, |_buffer, _same| {
                            panic!("nested alias must never run")
                        });
                    assert!(matches!(
                        nested,
                        Err(SourceCommandError::Conflict(
                            "original protected workspace already borrowed"
                        ))
                    ));
                    assert_eq!(outer[0], 19);
                    assert!(
                        same.charges().4 >= prefix.4,
                        "denied nested borrow does not refund holders"
                    );
                    cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
                    Err(SourceCommandError::Invalid(
                        "first workspace callback failure",
                    ))
                })
            },
        );
        assert!(matches!(
            result,
            Err(SourceCommandError::Invalid(
                "first workspace callback failure"
            ))
        ));
    }

    #[test]
    fn protected_workspace_cannot_be_lent_to_another_original_session() {
        let deadline = Instant::now() + Duration::from_secs(30);
        let cancelled = AtomicBool::new(false);
        let mut original_a = budget(deadline);
        let mut original_b = budget(deadline);
        with_original_pwe_session(
            &mut original_a,
            limits(deadline),
            &cancelled,
            |payer_a, same_a| {
                payer_a.with_protected_workspace(same_a, |_buffer, _same| Ok(()))?;
                let address = payer_a
                    .protected_scratch
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .as_ptr();
                with_original_pwe_session(
                    &mut original_b,
                    limits(deadline),
                    &cancelled,
                    |payer_b, same_b| {
                        let prefix = same_b.charges();
                        let body: SourceCommandResult<()> =
                            payer_a.with_protected_workspace(same_b, |_buffer, _same| {
                                panic!("foreign Session must not borrow the paid allocation")
                            });
                        assert!(matches!(
                            body,
                            Err(SourceCommandError::Conflict(
                                "STO source Session uses a different original payer"
                            ))
                        ));
                        assert_eq!(same_b.charges(), prefix);
                        assert!(payer_b.protected_scratch.borrow().is_none());
                        Ok(())
                    },
                )?;
                assert_eq!(
                    payer_a
                        .protected_scratch
                        .borrow()
                        .as_ref()
                        .unwrap()
                        .as_ptr(),
                    address
                );
                Ok(())
            },
        )
        .unwrap();
    }

    #[test]
    fn protected_workspace_state_refusal_precedes_allocation_and_stays_sticky() {
        let deadline = Instant::now() + Duration::from_secs(30);
        let cancelled = AtomicBool::new(false);
        let mut original = budget(deadline);
        let mut profile = limits(deadline);
        profile.max_state_bytes = 65535;
        let result =
            with_original_pwe_session(&mut original, profile, &cancelled, |payer, same| {
                let body: SourceCommandResult<()> = payer
                    .with_protected_workspace(same, |_buffer, _same| {
                        panic!("unpaid allocation must not be issued")
                    });
                assert!(body.is_err());
                assert!(payer.protected_scratch.borrow().is_none());
                let prefix = same.charges();
                assert!(payer.with_operation(|_| Ok(())).is_err());
                assert_eq!(same.charges(), prefix);
                Ok(()) // swallowed resource refusal must remain fatal at finish
            });
        assert!(result.is_err());
    }

    #[test]
    fn actual_protected_read_prices_full_eof_on_original_session_without_nodes() {
        let fixture = Fixture::new();
        let deadline = Instant::now() + Duration::from_secs(30);
        let cancelled = AtomicBool::new(false);
        let mut original = budget(deadline);
        let original_work = original.work_remaining;
        let limits = limits(deadline);
        with_original_pwe_session(&mut original, limits, &cancelled, |payer, same| {
            let mut file = File::open(fixture.0.join("input")).unwrap();
            let mut reader = OriginalProtectedReads::new(payer, same)?;
            let result = raw_with_length_limit_and_admission(
                &mut file,
                ReadLength::Exact(2),
                deadline,
                &cancelled,
                Some(&mut reader),
            );
            let raw = reader.complete(result)?;
            assert_eq!(raw, b"xy");
            let spent = reader.session.charges();
            assert_eq!(
                spent.0,
                2 * 65_536,
                "actual payload request plus actual EOF request"
            );
            assert_eq!(spent.1, 0, "native work is not a node surrogate");
            assert!(
                spent.2 > spent.0,
                "copy/event work is additional to requested input work"
            );
            assert_eq!(spent.3, 0);
            assert!(spent.4 >= 65_536 + raw.capacity());
            assert!(reader.session.has_source_payer(payer));
            Ok(())
        })
        .unwrap();
        assert_eq!(
            original.work_remaining,
            original_work - limits.max_work,
            "source work envelope was prepaid once and not refunded/replaced"
        );
    }
    #[test]
    fn actual_protected_read_preserves_io_error_and_paid_request_prefix() {
        let fixture = Fixture::new();
        let deadline = Instant::now() + Duration::from_secs(30);
        let cancelled = AtomicBool::new(false);
        let mut original = budget(deadline);
        let result: SourceCommandResult<()> = with_original_pwe_session(
            &mut original,
            limits(deadline),
            &cancelled,
            |payer, same| {
                // A real write-only descriptor: metadata succeeds, read fails
                // after the full request's admission. This is not a mock I/O error.
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .open(fixture.0.join("input"))
                    .unwrap();
                let mut reader = OriginalProtectedReads::new(payer, same)?;
                let body = raw_with_length_limit_and_admission(
                    &mut file,
                    ReadLength::Exact(2),
                    deadline,
                    &cancelled,
                    Some(&mut reader),
                )
                .map(|_| ());
                assert!(matches!(
                    &body,
                    Err(SourceCommandError::Invalid("creation descriptor read"))
                ));
                assert_eq!(reader.session.charges().0, 65_536);
                reader.complete(body)
            },
        );
        assert!(matches!(
            result,
            Err(SourceCommandError::Invalid("creation descriptor read"))
        ));
    }
    #[test]
    fn actual_protected_read_denies_eof_overrun_and_keeps_sticky_prefix() {
        let fixture = Fixture::new();
        let deadline = Instant::now() + Duration::from_secs(30);
        let cancelled = AtomicBool::new(false);
        let mut original = budget(deadline);
        let mut limited = limits(deadline);
        limited.max_input_bytes = 65_536;
        let result =
            with_original_pwe_session(&mut original, limited, &cancelled, |payer, same| {
                let mut file = File::open(fixture.0.join("input")).unwrap();
                let mut reader = OriginalProtectedReads::new(payer, same)?;
                let body = raw_with_length_limit_and_admission(
                    &mut file,
                    ReadLength::Exact(2),
                    deadline,
                    &cancelled,
                    Some(&mut reader),
                );
                assert!(
                    body.is_err(),
                    "EOF request is not free or silently truncated"
                );
                assert_eq!(reader.session.charges().0, 65_536);
                let before = reader.session.charges();
                assert!(
                    reader.complete(Ok(())).is_err(),
                    "swallowing mapped refusal cannot renew reader"
                );
                assert_eq!(
                    reader.session.charges(),
                    before,
                    "no reset/refill on refusal"
                );
                // Deliberately swallow: the ORIGINAL payer's outer finish must
                // still deny success, with its same sticky refusal marker.
                drop(body);
                Ok(())
            });
        assert!(result.is_err());
    }
    #[test]
    fn exact_protected_profile_prices_small_actual_slices_and_nonfree_eof() {
        for payload in [b"".as_slice(), b"xy".as_slice()] {
            let fixture = Fixture::new();
            std::fs::write(fixture.0.join("input"), payload).unwrap();
            let deadline = Instant::now() + Duration::from_secs(30);
            let cancelled = AtomicBool::new(false);
            let mut original = budget(deadline);
            let work_before = original.work_remaining;
            let profile = limits(deadline);
            with_original_pwe_session(&mut original, profile, &cancelled, |payer, same| {
                let mut file = File::open(fixture.0.join("input")).unwrap();
                let mut reader = OriginalProtectedReads::new(payer, same)?;
                let body = crate::source_creation_store::raw_exact_with_original_admission(
                    &mut file,
                    payload.len(),
                    deadline,
                    &cancelled,
                    &mut reader,
                );
                let raw = reader.complete(body)?;
                assert_eq!(raw, payload);
                let requested = if payload.is_empty() { 1 } else { 3 + 1 };
                assert_eq!(
                    reader.session.charges().0,
                    requested,
                    "actual payload/sentinel slice and mandatory EOF, also for exact empty input"
                );
                assert_eq!(reader.session.charges().1, 0);
                assert_eq!(reader.session.charges().3, 0);
                assert!(reader.session.charges().2 > reader.session.charges().0);
                assert!(reader.session.charges().4 >= 65_536 + raw.capacity());
                assert!(reader.session.has_source_payer(payer));
                Ok(())
            })
            .unwrap();
            assert_eq!(original.work_remaining, work_before - profile.max_work);
        }
    }

    #[test]
    fn exact_protected_profile_denies_eof_before_read_and_keeps_sticky_prefix() {
        let fixture = Fixture::new();
        let deadline = Instant::now() + Duration::from_secs(30);
        let cancelled = AtomicBool::new(false);
        let mut original = budget(deadline);
        let mut profile = limits(deadline);
        profile.max_input_bytes = 3;
        let body = with_original_pwe_session(&mut original, profile, &cancelled, |payer, same| {
            let mut file = File::open(fixture.0.join("input")).unwrap();
            let mut reader = OriginalProtectedReads::new(payer, same)?;
            let raw = crate::source_creation_store::raw_exact_with_original_admission(
                &mut file,
                2,
                deadline,
                &cancelled,
                &mut reader,
            );
            assert!(raw.is_err());
            assert_eq!(reader.session.charges().0, 3);
            let prefix = reader.session.charges();
            assert!(reader.complete(Ok(())).is_err());
            assert_eq!(reader.session.charges(), prefix);
            // Deliberately swallow the mapped EOF refusal. Same payer finish
            // still denies success; no replacement ledger or refunded work.
            Ok(())
        });
        assert!(body.is_err());
    }

    #[test]
    fn exact_protected_profile_rejects_wrong_initial_length_without_a_request() {
        let fixture = Fixture::new();
        let deadline = Instant::now() + Duration::from_secs(30);
        let cancelled = AtomicBool::new(false);
        let mut original = budget(deadline);
        with_original_pwe_session(
            &mut original,
            limits(deadline),
            &cancelled,
            |payer, same| {
                let mut file = File::open(fixture.0.join("input")).unwrap();
                let mut reader = OriginalProtectedReads::new(payer, same)?;
                let prefix = reader.session.charges();
                let body = crate::source_creation_store::raw_exact_with_original_admission(
                    &mut file,
                    3,
                    deadline,
                    &cancelled,
                    &mut reader,
                );
                assert!(matches!(
                    body,
                    Err(SourceCommandError::Conflict(
                        "creation input length changed"
                    ))
                ));
                let after = reader.session.charges();
                assert_eq!(
                    after.0, prefix.0,
                    "wrong initial length issues no payload request"
                );
                assert_eq!(
                    after.2,
                    prefix.2 + 7,
                    "metadata entry was admitted before failure"
                );
                assert_eq!(
                    after.4, prefix.4,
                    "no Begin scratch/output allocation on mismatch"
                );
                reader.complete(Ok(()))
            },
        )
        .unwrap();
    }

    #[test]
    fn exact_protected_profile_keeps_real_io_error_and_actual_request_prefix() {
        let fixture = Fixture::new();
        let deadline = Instant::now() + Duration::from_secs(30);
        let cancelled = AtomicBool::new(false);
        let mut original = budget(deadline);
        let result: SourceCommandResult<()> = with_original_pwe_session(
            &mut original,
            limits(deadline),
            &cancelled,
            |payer, same| {
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .open(fixture.0.join("input"))
                    .unwrap();
                let mut reader = OriginalProtectedReads::new(payer, same)?;
                let body = crate::source_creation_store::raw_exact_with_original_admission(
                    &mut file,
                    2,
                    deadline,
                    &cancelled,
                    &mut reader,
                )
                .map(|_| ());
                assert!(matches!(
                    &body,
                    Err(SourceCommandError::Invalid("creation descriptor read"))
                ));
                assert_eq!(reader.session.charges().0, 3);
                reader.complete(body)
            },
        );
        assert!(matches!(
            result,
            Err(SourceCommandError::Invalid("creation descriptor read"))
        ));
    }
}
