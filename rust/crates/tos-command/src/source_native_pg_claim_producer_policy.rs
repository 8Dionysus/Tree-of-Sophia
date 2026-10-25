//! Current private Claim producer read-policy INPUT, borrowed from the actual
//! owner RR history guard. It is NOT Claim lineage/metadata/role/application
//! authority and never grants the historical producer's writer permissions.
use super::{CurrentPgRoleOwner, complete_owner_loan};
use crate::durable_private_packages::{
    PrivateOwnerPackageFamilyV1, PrivateOwnerScopeV1, PrivateOwnerSelectedHistoryRead,
    SelectedPrivateProducerResponse,
};
use crate::source_command::{SourceCommandError as Error, SourceCommandResult};
use crate::source_native_cli::private_assessment::scope_admission::ScopeAccess;
use crate::{DurableError, DurableResult};
use std::mem::size_of;
use tos_foundation::{Digest256, Digest256Hasher, JsonDocument, JsonValue};

/// Private construction follows actual owner/epoch/current read-selection and
/// selected producer verification. No constructor accepts JSON/hash/bool/raw
/// history structs. The borrowed producer keeps its genuine guard lifetime.
/// Current permission must still be rechecked at every later native use.
pub(crate) struct PgClaimProducerReadPolicy<'history> {
    producer: SelectedPrivateProducerResponse<'history>,
    document: JsonDocument,
}
impl PgClaimProducerReadPolicy<'_> {
    pub(crate) fn declared(&self) -> &JsonValue {
        self.document.root()
    }
    pub(crate) fn producer(&self) -> &SelectedPrivateProducerResponse<'_> {
        &self.producer
    }
}

fn check_private_scope(
    access: &mut ScopeAccess<'_>,
    actual: &PrivateOwnerScopeV1,
    expected: &PrivateOwnerScopeV1,
) -> SourceCommandResult<()> {
    if !access.text_equal(&actual.domain, &expected.domain)?
        || !access.text_equal(&actual.owner_scope, &expected.owner_scope)?
    {
        return Err(Error::Denied(
            "PG Claim producer belongs to another private owner",
        ));
    }
    access.inspect(2 * size_of::<Digest256>(), 1)?;
    if actual.context_binding != expected.context_binding {
        return Err(Error::Denied("PG Claim producer context binding differs"));
    }
    Ok(())
}

impl CurrentPgRoleOwner<'_, '_, '_, '_, '_, '_, '_> {
    /// Select the actual package producer's declaration for a currently selected
    /// Claim ID. Declared membership is not proof that this Claim exists in the
    /// package; the unchanged lineage/content kernels still must establish it.
    /// The live RR guard supplies historical read-policy INPUT, not verified
    /// Claim content, a PG target/ticket, current permission or native basis.
    pub(crate) fn claim_producer_read_policy<'history>(
        &mut self,
        private: &PrivateOwnerSelectedHistoryRead<'history, '_, '_>,
        source_path: &str,
        claim: &str,
        content_requested: bool,
    ) -> DurableResult<PgClaimProducerReadPolicy<'history>> {
        let body = (|| {
            self.check_current()?;
            let grant = self.grant;
            let owner = self.owner;
            self.host.with_source_epoch(|epoch, payer, same| {
                // Before constructing the owned foreign-error relay and this
                // outer bridge's actual Result/Option carriers. The selector
                // separately admits its own callback/result/control holders.
                let relay = size_of::<Option<DurableError>>()
                    .checked_add(size_of::<SourceCommandResult<Option<PgClaimProducerReadPolicy<'_>>>>())
                    .and_then(|n| n.checked_add(size_of::<DurableResult<PgClaimProducerReadPolicy<'_>>>()))
                    .ok_or_else(|| DurableError::Source(payer.error(tos_compiler::pwe_native::Error::ResourceLimit)))?;
                payer.charge_sto_source_measures(same, 0, relay, 0, 0, 1)
                    .map_err(DurableError::Source)?;
                let mut foreign = None;
                let result = crate::source_native_cli::private_owner::claim::with_current_pg_claim_read_scope_exact_source(
                    payer, same, grant, owner, source_path, claim, content_requested,
                    |selected, same| {
                        let body = (|| -> DurableResult<PgClaimProducerReadPolicy<'history>> {
                            let binding = payer.private_owner_scope_binding_source(same, owner)
                                .map_err(DurableError::Source)?;
                            let domain = epoch.selection().cohort.domain();
                            // Exact primitives of PrivateOwnerScopeV1::for_context:
                            // three length words+domain literal+actual UTF8 domain+
                            // context digest, then lowercase64hex and domain copy.
                            let hash_bytes = (3 * size_of::<u64>() + b"tos-private-owner-scope-v1".len()
                                + size_of::<Digest256>()).checked_add(domain.len())
                                .ok_or_else(|| DurableError::Source(payer.error(tos_compiler::pwe_native::Error::ResourceLimit)))?;
                            let holders = size_of::<PrivateOwnerScopeV1>()
                                + size_of::<Digest256Hasher>()
                                + size_of::<DurableResult<PrivateOwnerScopeV1>>()
                                + size_of::<PgClaimProducerReadPolicy<'_>>()
                                + size_of::<SourceCommandResult<Option<PgClaimProducerReadPolicy<'_>>>>()
                                + size_of::<Option<DurableError>>();
                            let backing = domain.len().checked_add(64).and_then(|n| n.checked_add(holders))
                                .ok_or_else(|| DurableError::Source(payer.error(tos_compiler::pwe_native::Error::ResourceLimit)))?;
                            let work = hash_bytes.checked_add(domain.len()).and_then(|n| n.checked_add(64))
                                .and_then(|n| u64::try_from(n).ok())
                                .ok_or_else(|| DurableError::Source(payer.error(tos_compiler::pwe_native::Error::ResourceLimit)))?;
                            payer.charge_sto_source_measures(same, 0, backing, 0, work, 1)
                                .map_err(DurableError::Source)?;
                            let expected_scope = PrivateOwnerScopeV1::for_context(domain, binding)?;
                            let (deadline, cancelled) = payer.source_cutoff();
                            payer.with_operation(|budget| {
                                let mut access = ScopeAccess::admitted_operation(budget, deadline, cancelled);
                                check_private_scope(&mut access, private.scope(), &expected_scope)
                            }).map_err(DurableError::Source)?;
                            // The live selector already proved named package/path
                            // grammar. Borrow the parent, with no marker/path copy.
                            payer.inspect(0, selected.path().len()).map_err(DurableError::Source)?;
                            let target = selected.path().rsplit_once('/').ok_or(
                                DurableError::Source(Error::Invalid("PG selected Claim package parent")))?.0;
                            let mut query_foreign = None;
                            let issued = payer.with_operation(|budget| {
                                match private.current_producer_response_with_data_admission(target, budget, deadline, cancelled) {
                                    Ok(producer) => Ok(Some(producer)),
                                    Err(DurableError::Source(original)) => Err(original),
                                    Err(original) => { query_foreign = Some(original); Ok(None) }
                                }
                            });
                            let producer = complete_owner_loan(issued, query_foreign)?;
                            payer.inspect(0, size_of::<PrivateOwnerPackageFamilyV1>()).map_err(DurableError::Source)?;
                            if producer.entry().family != PrivateOwnerPackageFamilyV1::Claim {
                                return Err(DurableError::Refused("selected producer is not a native Claim family"));
                            }
                            let document = payer.with_operation(|budget| {
                                let document = crate::source_native_roles::admitted_document(
                                    &producer.entry().response_bytes, budget, deadline, cancelled)?;
                                let mut access = ScopeAccess::admitted_operation(budget, deadline, cancelled);
                                crate::source_native_cli::private_owner::claim::validate_pg_claim_producer_policy(
                                    document.root(), &selected, &mut access)?;
                                Ok(document)
                            }).map_err(DurableError::Source)?;
                            Ok(PgClaimProducerReadPolicy { producer, document })
                        })();
                        match body {
                            Ok(policy) => Ok(Some(policy)),
                            Err(DurableError::Source(original)) => Err(original),
                            Err(original) => { foreign = Some(original); Ok(None) }
                        }
                    },
                );
                complete_owner_loan(result, foreign)
            })
        })();
        let exited = self.check_current();
        match (body, exited) {
            (Err(original), _) | (Ok(_), Err(original)) => Err(original),
            (Ok(policy), Ok(())) => Ok(policy),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_native_cli::private_assessment::OperationBudget;
    use std::{
        sync::atomic::AtomicBool,
        time::{Duration, Instant},
    };

    #[test]
    fn scope_binding_predicate_rejects_owner_substitution_with_same_pg_domain() {
        // Scope comparison operands only. These raw scope structs do NOT
        // construct a live RR reader, producer policy or CurrentPgRoleOwner.
        let deadline = Instant::now() + Duration::from_secs(30);
        let cancel = AtomicBool::new(false);
        let mut original = OperationBudget::new(
            &serde_json::json!({"budgets":{
                "worker_cpu_seconds":1,"worker_address_space_bytes":1048576
            }}),
            deadline,
        )
        .unwrap();
        let mut access = ScopeAccess::admitted_operation(&mut original, deadline, &cancel);
        let first = PrivateOwnerScopeV1::for_context(
            "same actual PG domain",
            Digest256::of_bytes(b"actual owner A scope context"),
        )
        .unwrap();
        let second = PrivateOwnerScopeV1::for_context(
            "same actual PG domain",
            Digest256::of_bytes(b"actual owner B scope context"),
        )
        .unwrap();
        assert!(check_private_scope(&mut access, &first, &first).is_ok());
        assert!(matches!(
            check_private_scope(&mut access, &first, &second),
            Err(Error::Denied(
                "PG Claim producer belongs to another private owner"
            ))
        ));
        let mut changed = first.clone();
        changed.context_binding = second.context_binding;
        assert!(matches!(
            check_private_scope(&mut access, &changed, &first),
            Err(Error::Denied("PG Claim producer context binding differs"))
        ));
    }
}
