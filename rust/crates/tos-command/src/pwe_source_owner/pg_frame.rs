//! PG payload/frame consumer of the actual Common split loan.
//!
//! This is a transport/validation host, not an Assessment ticket issuer. Only
//! the maintained protected PG owner may select its read policy and semantic
//! admission route. No buffer, schema verdict or epoch becomes such a grant.
//! The filesystem HeldFrames/full-byte verifier remains an independent lane.
use crate::source_cohort::ManagedCurrentMember;
use crate::source_cohort::revision::{PgRecordRevisionIdentity, SourceRevisionReadSession};
use crate::source_command::SourceCommandError;
use crate::source_native_cli::PgCurrentPayloadLoan;
use crate::source_native_cli::private_assessment::original_payer::OriginalAssessmentPayer;
use crate::{DurableError, DurableResult};
use std::{
    cmp::Ordering as CmpOrdering,
    mem::{size_of, size_of_val},
};
use tos_compiler::pwe_native::{Error, Session};
use tos_foundation::{CutPreparedSchemaExecutionBinding, Digest256, RelativePath};
use tos_validation::source_cut::{CandidateCutWorkerSchemaExecutor, CutSchemaExecutor};
use tos_validation::source_foundation_schema::SourceFoundationSelectedSchemaResource;

/// Immutable transport views, issued only by with_pg_payload_host. They may
/// coexist across mutable frame guards; neither fields nor a constructor are
/// exposed. A view/borrow cannot escape the Common callback's lifetime.
pub(crate) struct PgPayloadViews<'view, 'budget, 'cancel> {
    selected: &'view PgRecordRevisionIdentity,
    members: &'view [ManagedCurrentMember],
    payer: &'view OriginalAssessmentPayer<'budget, 'cancel>,
}

/// A borrowed actual member. It is not a NativeRoleRecord, Claim admission,
/// catalogue membership witness, completeness receipt or historical identity.
pub(crate) struct PgPayloadMember<'view> {
    selected: &'view PgRecordRevisionIdentity,
    member: &'view ManagedCurrentMember,
}
impl<'view> PgPayloadMember<'view> {
    pub(crate) fn raw(&self) -> &'view [u8] {
        &self.member.raw
    }
    pub(crate) fn source_member(&self) -> &'view ManagedCurrentMember {
        self.member
    }
}

/// Actual independently borrowed epoch and typed worker. Session is always a
/// temporary operation parameter, never a second stored mutable loan. No FS
/// revision or root digest substitutes for the complete typed PG selection.
struct PgPayloadFrame<'view, 'store, 'budget, 'cancel> {
    selected: &'view PgRecordRevisionIdentity,
    epoch: &'view mut SourceRevisionReadSession<'store>,
    payer: &'view OriginalAssessmentPayer<'budget, 'cancel>,
    worker: &'view mut CandidateCutWorkerSchemaExecutor<PgRecordRevisionIdentity>,
    binding: CutPreparedSchemaExecutionBinding,
    schema_set: Digest256,
    contracts: Digest256,
    limits: Digest256,
}

/// Linear host retaining the original mutable Session loan. Public methods do
/// not accept a replacement Session. No saved pointer/stamp is needed to keep
/// the same allocation/ledger alive, and no second mutable loan is retained.
pub(crate) struct PgPayloadHost<'view, 'store, 'budget, 'cancel, 'pure> {
    frame: PgPayloadFrame<'view, 'store, 'budget, 'cancel>,
    session: &'view mut Session<'pure>,
}

/// The ONLY frame/view factory: direct child of the private-constructed actual
/// PG loan. Raw caller vectors, manifests, grants and alleged readsets are not
/// accepted. The real protected owner remains responsible for current read
/// permissions and private/public/history/role/application predicates.
pub(crate) fn with_pg_payload_host<'store, 'budget, 'cancel, 'pure, T>(
    loan: &mut PgCurrentPayloadLoan<'_, 'store, 'budget, 'cancel, 'pure>,
    worker: &mut CandidateCutWorkerSchemaExecutor<PgRecordRevisionIdentity>,
    run: impl for<'view> FnOnce(
        PgPayloadViews<'view, 'budget, 'cancel>,
        &mut PgPayloadHost<'view, 'store, 'budget, 'cancel, 'pure>,
    ) -> DurableResult<T>,
) -> DurableResult<T> {
    loan.with_selected_payloads(|selected, members, epoch, payer, session| {
        check_session(session, payer)?;
        let carriers = size_of::<PgPayloadFrame<'_, '_, '_, '_>>()
            .checked_add(size_of::<PgPayloadHost<'_, '_, '_, '_, '_>>())
            .and_then(|n| n.checked_add(size_of::<PgPayloadViews<'_, '_, '_>>()))
            .and_then(|n| n.checked_add(size_of::<DurableResult<T>>()))
            .and_then(|n| n.checked_add(size_of::<DurableResult<()>>()))
            .and_then(|n| n.checked_add(size_of_val(&run)))
            .ok_or(overflow())?;
        payer.reserve(carriers).map_err(DurableError::Source)?;
        inspect(
            payer,
            0,
            size_of::<CutPreparedSchemaExecutionBinding>() + 3 * size_of::<Digest256>(),
        )?;
        let binding = worker.prepared_execution_binding();
        let schema_set = worker.schema_set_digest();
        let contracts = worker.contract_selection_digest();
        let limits = worker.limits_sha256();
        let frame = PgPayloadFrame {
            selected,
            epoch,
            payer,
            worker,
            binding,
            schema_set,
            contracts,
            limits,
        };
        let mut host = PgPayloadHost { frame, session };
        let result = (|| {
            // Build checkpoint, then actual worker-resource closure. Expected
            // raw payload hashes are NOT recomputed by any freshness guard.
            host.verify()?;
            host.frame.bind_worker_resources(members, host.session)?;
            // Host entry after the immutable schema/selection binding work.
            host.verify()?;
            let views = PgPayloadViews {
                selected,
                members,
                payer,
            };
            run(views, &mut host)
        })();
        // Host exit on every callback/binding Result, including Err. The
        // enclosing Common splitter additionally owns its own paid exits.
        let fenced = host.verify();
        complete(result, fenced)
    })
}

impl<'view, 'budget, 'cancel> PgPayloadViews<'view, 'budget, 'cancel> {
    /// Lookup in the exact retained set. Currentness is checked by the ACTUAL
    /// frame before exposure, not by a cached bit. The returned borrow is of
    /// the immutable set, so a second lookup/guard does not borrow the first.
    pub(crate) fn member(
        &self,
        host: &mut PgPayloadHost<'_, '_, 'budget, 'cancel, '_>,
        path: &RelativePath,
    ) -> DurableResult<PgPayloadMember<'view>> {
        check_session(host.session, self.payer)?;
        inspect(self.payer, 0, size_of::<PgRecordRevisionIdentity>() + 1)?;
        if self.selected != host.frame.selected || !std::ptr::eq(self.payer, host.frame.payer) {
            return Err(DurableError::Conflict(
                "PWE PG views/frame selection or payer differs",
            ));
        }
        host.verify()?;
        let member = lookup(self.members, path.as_str(), host.session, self.payer)?;
        self.payer
            .reserve(size_of::<PgPayloadMember<'_>>())
            .map_err(DurableError::Source)?;
        Ok(PgPayloadMember {
            selected: self.selected,
            member,
        })
    }
}

impl<'view, 'store, 'budget, 'cancel, 'pure> PgPayloadHost<'view, 'store, 'budget, 'cancel, 'pure> {
    pub(crate) fn verify(&mut self) -> DurableResult<()> {
        self.frame.verify(self.session)
    }

    /// Target/basis stages operate on this same host. Inner pure/source calls
    /// borrow its Session sequentially; they cannot supply another ledger.
    pub(crate) fn with_current<T>(
        &mut self,
        run: impl FnOnce(&mut Self) -> DurableResult<T>,
    ) -> DurableResult<T> {
        let carriers = size_of::<DurableResult<T>>()
            .checked_add(size_of::<DurableResult<()>>())
            .and_then(|n| n.checked_add(size_of::<&mut Self>()))
            .and_then(|n| n.checked_add(size_of_val(&run)))
            .ok_or(overflow())?;
        self.frame
            .payer
            .reserve(carriers)
            .map_err(DurableError::Source)?;
        self.verify()?;
        let result = run(self);
        let fenced = self.verify();
        complete(result, fenced)
    }

    /// Pure preparation/qualification uses the exact retained Session. Map
    /// original errors before the real paid exit, not in a later fixed M.
    pub(crate) fn with_session<T>(
        &mut self,
        semantic: DurableError,
        run: impl FnOnce(&mut Session<'pure>) -> tos_compiler::pwe_native::Result<T>,
    ) -> DurableResult<T> {
        self.frame.with_current(self.session, |frame, same| {
            frame.session_result(run(same), semantic)
        })
    }

    /// Source/dependency operations borrow the real epoch/payer/Session only.
    /// The executor remains exclusively held by the private frame. Schema
    /// checks run sequentially through check_schema, never through a replaceable
    /// mutable worker (or a quota-resetting executor trait) in this callback.
    pub(crate) fn with_source_epoch<T>(
        &mut self,
        run: impl FnOnce(
            &mut SourceRevisionReadSession<'store>,
            &OriginalAssessmentPayer<'budget, 'cancel>,
            &mut Session<'pure>,
        ) -> DurableResult<T>,
    ) -> DurableResult<T> {
        self.frame.with_source_epoch(self.session, run)
    }

    pub(crate) fn check_schema(
        &mut self,
        member: &PgPayloadMember<'_>,
        schema: &PgPayloadMember<'_>,
    ) -> DurableResult<()> {
        self.frame.check_schema(self.session, member, schema)
    }
}

impl<'view, 'store, 'budget, 'cancel> PgPayloadFrame<'view, 'store, 'budget, 'cancel> {
    /// Map the result immediately at the real Session boundary, BEFORE an
    /// exit fence can observe the sticky refusal. Genuine semantic negatives
    /// retain their operation-specific refusal; resource/cancellation/actual
    /// upstream causes always come from the original payer, never a fixed M.
    fn session_result<T>(
        &self,
        result: tos_compiler::pwe_native::Result<T>,
        semantic: DurableError,
    ) -> DurableResult<T> {
        map_session_result(result, self.payer, semantic)
    }

    /// Freshness/resource checkpoint, never a physical-backing-store recheck.
    /// Worker and epoch identities are inspected after prospective payment.
    /// Actual epoch fencing is attempted even when another check returns Err;
    /// sticky resource refusal may prevent physical SQL, as on the owner route.
    fn verify(&mut self, session: &mut Session<'_>) -> DurableResult<()> {
        let carriers = size_of::<PgRecordRevisionIdentity>()
            .checked_add(size_of::<
                crate::source_command::SourceCommandResult<PgRecordRevisionIdentity>,
            >())
            .and_then(|n| n.checked_add(2 * size_of::<DurableResult<()>>()))
            .and_then(|n| {
                n.checked_add(size_of::<(
                    std::time::Instant,
                    &std::sync::atomic::AtomicBool,
                )>())
            })
            .ok_or(overflow())?;
        self.payer.reserve(carriers).map_err(DurableError::Source)?;
        let checked = (|| {
            check_session(session, self.payer)?;
            let work = size_of::<PgRecordRevisionIdentity>()
                .checked_mul(2)
                .and_then(|n| n.checked_add(2 * size_of::<CutPreparedSchemaExecutionBinding>()))
                .and_then(|n| n.checked_add(6 * size_of::<Digest256>()))
                .ok_or(overflow())?;
            inspect(self.payer, 0, work)?;
            let (deadline, cancelled) = self.payer.source_cutoff();
            let actual = self
                .payer
                .with_operation(|budget| {
                    self.epoch
                        .selection()
                        .identity_with_data_admission(budget, deadline, cancelled)
                })
                .map_err(DurableError::Source)?;
            if actual != *self.selected
                || self.worker.input_identity() != self.selected
                || self.worker.prepared_execution_binding() != self.binding
                || self.worker.schema_set_digest() != self.schema_set
                || self.worker.contract_selection_digest() != self.contracts
                || self.worker.limits_sha256() != self.limits
            {
                return Err(DurableError::Conflict(
                    "PWE PG epoch/worker binding differs",
                ));
            }
            Ok(())
        })();
        let fenced = epoch_fresh(self.epoch, self.payer);
        complete(checked, fenced)
    }

    /// Existing target preparation and basis capture can run through this
    /// same-session scope. It supplies real before/after guards, not semantic
    /// acceptance, a ticket, a successful default or a new callback ledger.
    fn with_current<'pure, T>(
        &mut self,
        session: &mut Session<'pure>,
        run: impl FnOnce(&mut Self, &mut Session<'pure>) -> DurableResult<T>,
    ) -> DurableResult<T> {
        check_session(session, self.payer)?;
        let carriers = size_of::<DurableResult<T>>()
            .checked_add(size_of::<DurableResult<()>>())
            .and_then(|n| n.checked_add(size_of::<(&mut Self, &mut Session<'pure>)>()))
            .and_then(|n| n.checked_add(size_of_val(&run)))
            .ok_or(overflow())?;
        self.payer.reserve(carriers).map_err(DurableError::Source)?;
        self.verify(session)?;
        let result = run(self, session);
        let fenced = self.verify(session);
        complete(result, fenced)
    }

    /// Native dependency/permission owners can borrow the ACTUAL source lane
    /// without reentering PgGrowthCurrentInput's already-borrowed RefCell.
    /// They may create its existing bounded input view over this same epoch;
    /// this supplies neither a new source owner nor an admission result. No
    /// executor handle escapes: matching configuration pins alone cannot prove
    /// continuity of cumulative execution/diagnostics/quota state.
    fn with_source_epoch<'pure, T>(
        &mut self,
        session: &mut Session<'pure>,
        run: impl FnOnce(
            &mut SourceRevisionReadSession<'store>,
            &OriginalAssessmentPayer<'budget, 'cancel>,
            &mut Session<'pure>,
        ) -> DurableResult<T>,
    ) -> DurableResult<T> {
        let holders = size_of::<(
            &mut SourceRevisionReadSession<'store>,
            &OriginalAssessmentPayer<'budget, 'cancel>,
            &mut Session<'pure>,
        )>()
        .checked_add(size_of_val(&run))
        .ok_or(overflow())?;
        self.payer.reserve(holders).map_err(DurableError::Source)?;
        self.with_current(session, |frame, same| {
            run(&mut *frame.epoch, frame.payer, same)
        })
    }

    /// Validate an actual borrowed member against an actual selected schema.
    /// The complete worker resource set was bound to actual loan metadata at
    /// construction. This schema verdict grants NO role/current-use authority.
    /// Full worker/driver/activation funding remains the maintained owner's
    /// quota/quote obligation; this pays the native inspection/copy envelope.
    fn check_schema(
        &mut self,
        session: &mut Session<'_>,
        member: &PgPayloadMember<'_>,
        schema: &PgPayloadMember<'_>,
    ) -> DurableResult<()> {
        self.with_current(session, |frame, _same| {
            inspect(
                frame.payer,
                0,
                2 * size_of::<PgRecordRevisionIdentity>() + 4,
            )?;
            if member.selected != frame.selected || schema.selected != frame.selected {
                return Err(DurableError::Conflict(
                    "PWE PG schema/member selection differs",
                ));
            }
            let path = member.member.path.as_str();
            let root = schema.member.path.as_str();
            let names = path.len().checked_add(root.len()).ok_or(overflow())?;
            let bytes = member
                .member
                .raw
                .len()
                .checked_add(names)
                .ok_or(overflow())?;
            inspect(frame.payer, bytes, 1)?;
            if !root.starts_with("ToS/contracts/") || !root.ends_with(".schema.json") {
                return Err(DurableError::Refused(
                    "PWE PG schema path is not a selected contract",
                ));
            }
            // Error path strings are also real holders: prepay their logical
            // retained bytes/copies before worker execution can produce Err.
            frame.payer.reserve(names).map_err(DurableError::Source)?;
            inspect(frame.payer, names, 1)?;
            let (deadline, cancelled) = frame.payer.source_cutoff();
            match frame
                .worker
                .check(path, &member.member.raw, root, deadline, cancelled)
            {
                Ok(true) => Ok(()),
                Ok(false) => Err(DurableError::Refused(
                    "PWE PG selected schema rejected payload",
                )),
                Err(reason) => Err(DurableError::Source(SourceCommandError::SchemaExecution {
                    path: path.to_owned(),
                    root: root.to_owned(),
                    reason,
                })),
            }
        })
    }

    fn bind_worker_resources(
        &mut self,
        members: &[ManagedCurrentMember],
        session: &mut Session<'_>,
    ) -> DurableResult<()> {
        // Only selected resource metadata is revisited. No full Claim cut,
        // registry raw traversal, corpus inventory, JSON parse or rehash.
        inspect(self.payer, 0, 1)?;
        if self.worker.source_resource_count() == 0 {
            return Err(DurableError::Refused(
                "PWE PG worker selected schema closure is empty",
            ));
        }
        // source_resources is a capture-free Map over a slice Iter. Four
        // words bound that iterator; the yielded metadata/Option holder is
        // quoted by its actual public type before construction/next().
        let carriers = size_of::<[usize; 4]>()
            .checked_add(size_of::<Option<SourceFoundationSelectedSchemaResource<'_>>>())
            .ok_or(overflow())?;
        self.payer.reserve(carriers).map_err(DurableError::Source)?;
        let mut resources = self.worker.source_resources();
        loop {
            inspect(self.payer, 0, 1)?;
            let Some(resource) = resources.next() else {
                break;
            };
            let member = lookup(members, resource.path, session, self.payer)?;
            inspect(
                self.payer,
                0,
                size_of::<u64>() + size_of::<tos_foundation::Digest256>(),
            )?;
            if resource.size_bytes != member.metadata.size_bytes
                || resource.sha256 != member.metadata.sha256
            {
                return Err(DurableError::Conflict(
                    "PWE PG worker resource differs from actual payload",
                ));
            }
        }
        Ok(())
    }
}

fn check_session(
    session: &mut Session<'_>,
    payer: &OriginalAssessmentPayer<'_, '_>,
) -> DurableResult<()> {
    // The same real pair is checked even when measures are zero. This checks
    // original sticky refusal as well as SOURCE mode/cutoff; no debit reset.
    payer
        .charge_sto_source_measures(session, 0, 0, 0, 0, 0)
        .map_err(DurableError::Source)
}

fn map_session_result<T>(
    result: tos_compiler::pwe_native::Result<T>,
    payer: &OriginalAssessmentPayer<'_, '_>,
    semantic: DurableError,
) -> DurableResult<T> {
    result.map_err(|error| match error {
        Error::Upstream(_) | Error::Cancelled | Error::Deadline | Error::ResourceLimit => {
            DurableError::Source(payer.error(error))
        }
        _ => semantic,
    })
}

fn inspect(
    payer: &OriginalAssessmentPayer<'_, '_>,
    bytes: usize,
    visits: usize,
) -> DurableResult<()> {
    let work = bytes.checked_add(visits).ok_or(overflow())?;
    payer.inspect(bytes, work).map_err(DurableError::Source)
}

fn lookup<'members>(
    members: &'members [ManagedCurrentMember],
    path: &str,
    session: &mut Session<'_>,
    payer: &OriginalAssessmentPayer<'_, '_>,
) -> DurableResult<&'members ManagedCurrentMember> {
    check_session(session, payer)?;
    inspect(payer, 0, 3)?;
    let mut low = 0;
    let mut high = members.len();
    loop {
        // Common constructs this private set in BTreeSet<RelativePath> order
        // and checks returned path equality before push. RelativePath derives
        // Ord over its lexical String. No untrusted slice enters this helper.
        // Pay bounds/index work BEFORE accessing the next candidate, then pay
        // both path operands BEFORE comparison. No full selected-set scan.
        inspect(payer, 0, 8)?;
        if low == high {
            break;
        }
        let middle = low + (high - low) / 2;
        let member = &members[middle];
        let bytes = path
            .len()
            .checked_add(member.path.as_str().len())
            .ok_or(overflow())?;
        inspect(payer, bytes, 1)?;
        match member.path.as_str().cmp(path) {
            CmpOrdering::Equal => return Ok(member),
            CmpOrdering::Less => low = middle + 1,
            CmpOrdering::Greater => high = middle,
        }
    }
    Err(DurableError::Refused(
        "PWE PG required payload absent from actual selected loan",
    ))
}

fn epoch_fresh(
    epoch: &mut SourceRevisionReadSession<'_>,
    payer: &OriginalAssessmentPayer<'_, '_>,
) -> DurableResult<()> {
    let carriers = size_of::<crate::source_command::SourceCommandResult<DurableResult<()>>>()
        .checked_add(size_of::<DurableResult<()>>())
        .ok_or(overflow())?;
    payer.reserve(carriers).map_err(DurableError::Source)?;
    let (deadline, cancelled) = payer.source_cutoff();
    // A nested Result preserves the actual Durable error without a cloned
    // error cell. Source errors must cross the outer layer as Err so the
    // original-operation sticky check cannot replace that original E.
    payer
        .with_operation(|budget| {
            match epoch.verify_fresh_with_query_data_admission(budget, deadline, cancelled) {
                Err(DurableError::Source(original)) => Err(original),
                result => Ok(result),
            }
        })
        .map_err(DurableError::Source)?
}

fn complete<T>(result: DurableResult<T>, fenced: DurableResult<()>) -> DurableResult<T> {
    match (result, fenced) {
        (Err(original), _) | (Ok(_), Err(original)) => Err(original),
        (Ok(value), Ok(())) => Ok(value),
    }
}
fn overflow() -> DurableError {
    DurableError::Source(SourceCommandError::Unsupported(
        "PWE PG logical holder/work overflow",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_native_cli::private_assessment::{
        OperationBudget, original_payer::with_original_pwe_session_durable,
    };
    use std::{
        sync::atomic::{AtomicBool, Ordering},
        time::{Duration, Instant},
    };
    use tos_compiler::pwe_native::Limits;

    #[test]
    fn pg_payload_helpers_reject_foreign_session_and_missing_member_without_authority_fixture() {
        let cancelled = AtomicBool::new(false);
        let deadline = Instant::now() + Duration::from_secs(30);
        let config = serde_json::json!({"budgets": {"worker_cpu_seconds": 1, "worker_address_space_bytes": 1048576}});
        let mut original = OperationBudget::new(&config, deadline).unwrap();
        let mut other = OperationBudget::new(&config, deadline).unwrap();
        let limits = Limits {
            max_input_bytes: 256,
            max_state_bytes: 256,
            max_work: 128,
            max_nodes: 32,
            max_refs: 16,
            max_depth: 8,
            max_items: 16,
            deadline,
        };
        let result =
            with_original_pwe_session_durable(&mut original, limits, &cancelled, |payer, same| {
                with_original_pwe_session_durable(&mut other, limits, &cancelled, |foreign, _| {
                    assert!(matches!(
                        check_session(same, foreign),
                        Err(DurableError::Source(SourceCommandError::Conflict(
                            "STO source Session uses a different original payer"
                        )))
                    ));
                    Ok(())
                })?;
                let before = same.charges();
                let missing = lookup(&[], "ToS/required.json", same, payer);
                assert!(matches!(
                    missing,
                    Err(DurableError::Refused(
                        "PWE PG required payload absent from actual selected loan"
                    ))
                ));
                assert_eq!(
                    same.charges(),
                    before,
                    "native lookup fees cannot become SOURCE input/work/state"
                );
                Ok(())
            });
        assert!(result.is_ok());
    }

    #[test]
    fn pg_payload_helpers_preserve_original_error_sticky_prefix_and_cancelled_exit() {
        let cancelled = AtomicBool::new(false);
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut original = OperationBudget::new(
            &serde_json::json!({"budgets": {
                "worker_cpu_seconds": 1, "worker_address_space_bytes": 1048576
            }}),
            deadline,
        )
        .unwrap();
        let limits = Limits {
            max_input_bytes: 128,
            max_state_bytes: 128,
            max_work: 64,
            max_nodes: 16,
            max_refs: 8,
            max_depth: 8,
            max_items: 8,
            deadline,
        };
        let result: DurableResult<()> =
            with_original_pwe_session_durable(&mut original, limits, &cancelled, |payer, same| {
                same.admit_source_work(3)
                    .map_err(|error| DurableError::Source(payer.error(error)))?;
                let before = same.charges();
                let error = inspect(payer, 0, usize::MAX);
                assert!(matches!(
                    error,
                    Err(DurableError::Source(SourceCommandError::Invalid(
                        "assessment cumulative work budget"
                    )))
                ));
                assert_eq!(same.charges(), before);
                assert!(
                    check_session(same, payer).is_err(),
                    "sticky original refusal survives a swallowed error"
                );
                cancelled.store(true, Ordering::Relaxed);
                complete(
                    Err(DurableError::Conflict("original PG consumer body")),
                    check_session(same, payer),
                )
            });
        assert!(matches!(
            result,
            Err(DurableError::Conflict("original PG consumer body"))
        ));
    }

    #[test]
    fn pg_session_result_keeps_original_resource_error_and_real_semantic_refusal() {
        let cancelled = AtomicBool::new(false);
        let deadline = Instant::now() + Duration::from_secs(30);
        let config = serde_json::json!({"budgets": {"worker_cpu_seconds": 1, "worker_address_space_bytes": 1048576}});
        for resource in [false, true] {
            let mut original = OperationBudget::new(&config, deadline).unwrap();
            let limits = Limits {
                max_input_bytes: 128,
                max_state_bytes: 128,
                max_work: 64,
                max_nodes: 16,
                max_refs: 8,
                max_depth: 8,
                max_items: 8,
                deadline,
            };
            let result: DurableResult<()> = with_original_pwe_session_durable(
                &mut original,
                limits,
                &cancelled,
                |payer, same| {
                    let native = if resource {
                        same.admit_source_work(65)
                    } else {
                        Err(Error::NeedsBinding("actual semantic binding missing"))
                    };
                    map_session_result(
                        native,
                        payer,
                        DurableError::Refused("real semantic refusal"),
                    )
                },
            );
            if resource {
                assert!(matches!(
                    result,
                    Err(DurableError::Source(SourceCommandError::Unsupported(
                        "PWE Assessment Session exhausted"
                    )))
                ));
            } else {
                assert!(matches!(
                    result,
                    Err(DurableError::Refused("real semantic refusal"))
                ));
            }
        }
    }
}
