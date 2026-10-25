//! Closed archive DATA read on the actual selected owner RR transaction.
//! This is not package_state, immutable creation evidence, a Claim target,
//! NativeAccount, basis, report, ticket or permission to use another Claim.
use super::{CurrentPgRoleOwner, complete_owner_loan};
use crate::durable_private_packages::{
    PrivateOwnerArchiveSnapshotV1, PrivateOwnerPackageFamilyV1, PrivateOwnerScopeV1,
    PrivateOwnerSelectedHistoryRead,
};
use crate::source_command::{SourceCommandError as Error, SourceCommandResult as Result};
use crate::source_native_cli::private_assessment::scope_admission::ScopeAccess;
use crate::source_native_cli::private_owner::claim::{self, PrivatePweArchivePlan};
use crate::source_text_private_store::PrivatePackage;
use crate::{DurableError, DurableResult};
use std::mem::size_of;
use tos_foundation::Digest256;

#[path = "source_native_pg_private_pwe_archive_manifest.rs"]
mod manifest;
pub(crate) use manifest::{PgPrivatePweArchivedFiles, PrivatePweArchivedFile};

#[path = "source_native_pg_private_pwe_stream_chain.rs"]
mod stream_chain;
pub(crate) use stream_chain::PgPrivatePweStreamHistory;

/// The plan remains borrowed while these genuine archived bytes are held.
/// Neither a raw reference nor a public snapshot can construct this value.
/// Its source/package digest fields are database observations, not proof that
/// the receipt/manifest/history/creation/form semantics are valid.
pub(crate) struct PgPrivatePweArchiveData<'plan, 'history> {
    plan: &'plan PrivatePweArchivePlan<'history>,
    request_index: usize,
    snapshot: PrivateOwnerArchiveSnapshotV1,
}
impl PgPrivatePweArchiveData<'_, '_> {
    pub(crate) fn members(&self) -> &PrivatePackage {
        &self.snapshot.members
    }
    pub(crate) fn request_index(&self) -> usize {
        self.request_index
    }
    pub(crate) fn reference(&self) -> &str {
        self.plan.requests()[self.request_index].reference()
    }
    pub(crate) fn observed_source_revision(&self) -> Digest256 {
        self.snapshot.source_revision
    }
    pub(crate) fn observed_package_digest(&self) -> Digest256 {
        self.snapshot.package_digest
    }
}

fn overflow() -> Error {
    Error::Unsupported("private PWE archive read holders overflow")
}

// Only a conjunction inside the real constructor, never an authority factory.
// Distinct immutable maps containing equal revisions/bytes are NOT the same
// RR producer. This prevents applying plan A to a second history guard B.
fn same_entry<T>(planned: &T, current: &T) -> bool {
    std::ptr::eq(planned, current)
}

fn check_archive_header(
    access: &mut ScopeAccess<'_>,
    expected: &PrivateOwnerScopeV1,
    target: &str,
    reference: &str,
    generation: u64,
    archive: &PrivateOwnerArchiveSnapshotV1,
) -> Result<()> {
    let scope_matches = access.text_equal(&archive.scope.domain, &expected.domain)?
        && access.text_equal(&archive.scope.owner_scope, &expected.owner_scope)?;
    access.inspect(
        size_of::<Digest256>() * 2
            + size_of::<u64>() * 2
            + size_of::<PrivateOwnerPackageFamilyV1>(),
        1,
    )?;
    if !scope_matches || archive.scope.context_binding != expected.context_binding {
        return Err(Error::Conflict("private PWE archive owner scope differs"));
    }
    if !access.text_equal(&archive.target_ref, target)?
        || !access.text_equal(&archive.reference, reference)?
    {
        return Err(Error::Conflict(
            "private PWE archive selected request differs",
        ));
    }
    if archive.family != PrivateOwnerPackageFamilyV1::Claim
        || archive.private_audit_generation != generation
    {
        return Err(Error::Conflict(
            "private PWE archive family or RR generation differs",
        ));
    }
    Ok(())
}

impl<'call, 'owner, 'view, 'store, 'budget, 'cancel, 'pure>
    CurrentPgRoleOwner<'call, 'owner, 'view, 'store, 'budget, 'cancel, 'pure>
{
    /// Inspect a genuine archive with a freshly checked current scope and
    /// producer, not under permission/payment retained inside a DATA value.
    /// This checks ONE complete archived package; creation/chain/form proofs
    /// still require the subsequent closed interpreter and actual schemas.
    pub(crate) fn check_private_pwe_archive<'data, 'plan, 'history>(
        &mut self,
        private: &PrivateOwnerSelectedHistoryRead<'history, '_, '_>,
        data: &'data PgPrivatePweArchiveData<'plan, 'history>,
        source_path: &str,
        claim_id: &str,
    ) -> DurableResult<PgPrivatePweArchivedFiles<'data, 'plan, 'history>> {
        let body = (|| {
            self.check_current()?;
            self.with_source_epoch(|_, payer, same| {
                let holders = size_of::<PgPrivatePweArchivedFiles<'_, '_, '_>>()
                    .checked_add(
                        2 * size_of::<DurableResult<PgPrivatePweArchivedFiles<'_, '_, '_>>>(),
                    )
                    .ok_or(DurableError::Source(overflow()))?;
                payer
                    .charge_sto_source_measures(same, 0, holders, 0, 0, 1)
                    .map_err(DurableError::Source)
            })?;
            let current = self.claim_producer_read_policy(private, source_path, claim_id, false)?;
            self.with_source_epoch(|_, payer, _| {
                payer
                    .inspect(0, 2 * size_of::<usize>())
                    .map_err(DurableError::Source)
            })?;
            if !same_entry(
                data.plan.producer().producer().entry(),
                current.producer().entry(),
            ) {
                return Err(DurableError::Conflict(
                    "private PWE archive plan belongs to another RR producer",
                ));
            }
            let grant = self.grant;
            let owner = self.owner;
            self.host.with_source_epoch(|_, payer, same| {
                let (deadline, cancelled) = payer.source_cutoff();
                claim::with_current_pg_claim_read_scope_exact_source(
                    payer,
                    same,
                    grant,
                    owner,
                    source_path,
                    claim_id,
                    false,
                    |selected, _same| {
                        payer.with_operation(|budget| {
                            {
                                let mut access =
                                    ScopeAccess::admitted_operation(budget, deadline, cancelled);
                                let path = access.text(current.declared(), "source_path")?;
                                if !access.text_equal(path, selected.path())? {
                                    return Err(Error::Conflict(
                                        "private PWE archive current selection differs",
                                    ));
                                }
                                check_archive_header(
                                    &mut access,
                                    current.producer().scope(),
                                    current.producer().target_ref(),
                                    data.reference(),
                                    current.producer().private_audit_generation(),
                                    &data.snapshot,
                                )?;
                            }
                            manifest::check_bound_archive(
                                data,
                                current.declared(),
                                budget,
                                deadline,
                                cancelled,
                            )
                        })
                    },
                )
                .map_err(DurableError::Source)
            })
        })();
        let exit = self.check_current();
        match (body, exit) {
            (Err(original), _) | (Ok(_), Err(original)) => Err(original),
            (Ok(value), Ok(())) => Ok(value),
        }
    }

    /// Each invocation reselects the current protected Claim, reissues its
    /// actual producer input and charges the original operation anew. No
    /// transferred plan, matching coordinates, historical writer declaration
    /// or prior payment can replace those live checks.
    ///
    /// Query DATA is admitted by the existing complete archive envelope on
    /// this SAME RR reader before HEADER/MEMBERS queries. The current method
    /// adds its own actual typed holders and comparisons before use. This is
    /// not a complete driver/hash/native/allocator/Drop/RSS enclosure.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn read_private_pwe_archive<'plan, 'history>(
        &mut self,
        private: &mut PrivateOwnerSelectedHistoryRead<'history, '_, '_>,
        plan: &'plan PrivatePweArchivePlan<'history>,
        request_index: usize,
        source_path: &str,
        claim_id: &str,
    ) -> DurableResult<PgPrivatePweArchiveData<'plan, 'history>> {
        let body = (|| {
            self.check_current()?;
            self.with_source_epoch(|_, payer, same| {
                // Includes the Result/Option foreign-error relay before its
                // construction, not a later charge used as retroactive credit.
                let holders = size_of::<PgPrivatePweArchiveData<'_, '_>>()
                    .checked_add(size_of::<PrivateOwnerArchiveSnapshotV1>())
                    .and_then(|n| n.checked_add(2 * size_of::<Option<DurableError>>()))
                    .and_then(|n| {
                        n.checked_add(size_of::<Result<Option<PrivateOwnerArchiveSnapshotV1>>>())
                    })
                    .and_then(|n| {
                        n.checked_add(
                            2 * size_of::<DurableResult<PgPrivatePweArchiveData<'_, '_>>>(),
                        )
                    })
                    .and_then(|n| n.checked_add(size_of::<usize>()))
                    .ok_or(DurableError::Source(overflow()))?;
                payer
                    .charge_sto_source_measures(same, 0, holders, 0, 0, 1)
                    .map_err(DurableError::Source)
            })?;
            let request = plan
                .requests()
                .get(request_index)
                .ok_or(DurableError::Refused(
                    "private PWE archive request outside complete plan",
                ))?;
            // Reuses no previous permission or operation credit. This real
            // method checks actual scope/context/current package head/receipt,
            // producer family and current Assessment source_access predicates.
            let current = self.claim_producer_read_policy(private, source_path, claim_id, false)?;
            self.with_source_epoch(|_, payer, _| {
                payer
                    .inspect(0, 2 * size_of::<usize>())
                    .map_err(DurableError::Source)
            })?;
            if !same_entry(
                plan.producer().producer().entry(),
                current.producer().entry(),
            ) {
                return Err(DurableError::Conflict(
                    "private PWE archive plan belongs to another RR producer",
                ));
            }
            let producer = current.producer();
            let target = producer.target_ref();
            let grant = self.grant;
            let owner = self.owner;
            let snapshot = self.host.with_source_epoch(|_, payer, same| {
                let (deadline, cancelled) = payer.source_cutoff();
                let mut foreign = None;
                let result = claim::with_current_pg_claim_read_scope_exact_source(
                    payer,
                    same,
                    grant,
                    owner,
                    source_path,
                    claim_id,
                    false,
                    |selected, _same| {
                        payer.with_operation(|budget| {
                            {
                                let mut access =
                                    ScopeAccess::admitted_operation(budget, deadline, cancelled);
                                let declared_path =
                                    access.text(current.declared(), "source_path")?;
                                if !access.text_equal(declared_path, selected.path())? {
                                    return Err(Error::Conflict(
                                        "private PWE archive current selection differs",
                                    ));
                                }
                            }
                            // This short original-operation loan ends before any
                            // later native schema call; no reentrant payer hooks.
                            let archive = match private
                                .read_claim_archive_with_query_data_admission(
                                    target,
                                    request.reference(),
                                    budget,
                                    deadline,
                                    cancelled,
                                ) {
                                Ok(Some(archive)) => archive,
                                Ok(None) => {
                                    return Err(Error::Conflict(
                                        "private PWE required archive absent",
                                    ));
                                }
                                Err(DurableError::Source(original)) => return Err(original),
                                Err(original) => {
                                    foreign = Some(original);
                                    return Ok(None);
                                }
                            };
                            let mut access =
                                ScopeAccess::admitted_operation(budget, deadline, cancelled);
                            check_archive_header(
                                &mut access,
                                producer.scope(),
                                target,
                                request.reference(),
                                producer.private_audit_generation(),
                                &archive,
                            )?;
                            Ok(Some(archive))
                        })
                    },
                );
                // A detailed PG/driver fault remains owned and primary even
                // if a later paid selector/epoch exit independently refuses.
                complete_owner_loan(result, foreign)
            })?;
            Ok(PgPrivatePweArchiveData {
                plan,
                request_index,
                snapshot,
            })
        })();
        let exit = self.check_current();
        match (body, exit) {
            (Err(original), _) | (Ok(_), Err(original)) => Err(original),
            (Ok(value), Ok(())) => Ok(value),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_native_cli::private_assessment::OperationBudget;
    use std::{
        collections::BTreeMap,
        sync::atomic::{AtomicBool, Ordering},
        time::{Duration, Instant},
    };

    // Header-predicate fixtures ONLY: none of these raw structs can construct
    // PgPrivatePweArchiveData or an actual history guard/owner/permission.
    fn scope() -> PrivateOwnerScopeV1 {
        PrivateOwnerScopeV1::for_context("fixture-domain", Digest256::of_bytes(b"context")).unwrap()
    }
    fn snapshot() -> PrivateOwnerArchiveSnapshotV1 {
        PrivateOwnerArchiveSnapshotV1 {
            scope: scope(),
            target_ref: "private/claims/selected".into(),
            reference: "private/.record-revisions/exact".into(),
            family: PrivateOwnerPackageFamilyV1::Claim,
            source_revision: Digest256::of_bytes(b"source"),
            package_digest: Digest256::of_bytes(b"package"),
            members: BTreeMap::new(),
            private_audit_generation: 7,
        }
    }
    fn setup() -> (Instant, AtomicBool, OperationBudget) {
        let deadline = Instant::now() + Duration::from_secs(30);
        let budget = OperationBudget::new(
            &serde_json::json!({"budgets":{
                "worker_cpu_seconds":1,"worker_address_space_bytes":1048576
            }}),
            deadline,
        )
        .unwrap();
        (deadline, AtomicBool::new(false), budget)
    }
    fn check(archive: &PrivateOwnerArchiveSnapshotV1) -> Result<()> {
        let (d, c, mut b) = setup();
        check_archive_header(
            &mut ScopeAccess::admitted_operation(&mut b, d, &c),
            &scope(),
            "private/claims/selected",
            "private/.record-revisions/exact",
            7,
            archive,
        )
    }
    #[test]
    fn private_pwe_archive_header_rejects_owner_target_and_reference_substitution() {
        assert!(check(&snapshot()).is_ok());
        let mut a = snapshot();
        a.scope.context_binding = Digest256::of_bytes(b"other");
        assert!(matches!(
            check(&a),
            Err(Error::Conflict("private PWE archive owner scope differs"))
        ));
        let mut a = snapshot();
        a.scope.domain.push('x');
        assert!(check(&a).is_err());
        let mut a = snapshot();
        a.scope.owner_scope.push('x');
        assert!(check(&a).is_err());
        let mut a = snapshot();
        a.target_ref.push('x');
        assert!(check(&a).is_err());
        let mut a = snapshot();
        a.reference.push('x');
        assert!(check(&a).is_err());
    }
    #[test]
    fn private_pwe_archive_header_rejects_wrong_family_and_rr_generation() {
        let mut a = snapshot();
        a.family = PrivateOwnerPackageFamilyV1::Assessment;
        assert!(matches!(
            check(&a),
            Err(Error::Conflict(
                "private PWE archive family or RR generation differs"
            ))
        ));
        let mut a = snapshot();
        a.private_audit_generation += 1;
        assert!(check(&a).is_err());
    }
    #[test]
    fn private_pwe_archive_plan_reuse_requires_identical_entry_not_equal_copy() {
        let original = String::from("equal immutable producer bytes");
        let copied = original.clone();
        assert!(same_entry(&original, &original));
        assert!(!same_entry(&original, &copied));
    }
    #[test]
    fn private_pwe_archive_header_cancel_keeps_original_failed_prefix() {
        let (d, c, mut b) = setup();
        let before = b.test_work_remaining();
        c.store(true, Ordering::Relaxed);
        assert!(
            check_archive_header(
                &mut ScopeAccess::admitted_operation(&mut b, d, &c),
                &scope(),
                "private/claims/selected",
                "private/.record-revisions/exact",
                7,
                &snapshot()
            )
            .is_err()
        );
        assert_eq!(b.test_work_remaining(), before);
    }
}
