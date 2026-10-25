//! Joined private PWE source observations under THIS actual protected owner.
//! This is a concrete interpreter stage, NOT a NativeAccount/target/request,
//! historical grant authentication, current permission or application verdict.
use super::{CurrentPgRoleOwner, PgPrivatePweClaimSyntax, PgPrivatePweInitialForms};
use crate::durable_private_packages::{
    PrivateOwnerSelectedHistoryRead, SelectedPrivateInitialProducerResponse,
};
use crate::pwe_source_owner::PgPayloadViews;
use crate::source_cohort::revision::PgRecordRevisionIdentity;
use crate::source_command::{
    CommandContext, SourceCommandError as Error, SourceCommandResult as Result,
};
use crate::source_native_cli::private_assessment::{OperationBudget, scope_admission::ScopeAccess};
use crate::source_native_cli::private_owner::claim;
use crate::{DurableError, DurableResult};
use std::{collections::BTreeSet, mem::size_of, sync::atomic::AtomicBool, time::Instant};
use tos_foundation::{Digest256, Digest256Hasher, RelativePath};

/// No raw constructor, Clone, permission bit or eligibility/report projection.
/// All fields come from the same guarded actual producer/selected source below.
/// Reusing these observations cannot bypass a later live owner/current recheck.
pub(crate) struct PgPrivatePweSourceMaterial<'history> {
    syntax: PgPrivatePweClaimSyntax<'history>,
    initial_forms: PgPrivatePweInitialForms<'history>,
    selected: PgRecordRevisionIdentity,
    initial_stream: Digest256,
    current_stream: Digest256,
}

/// Lossless completion of THIS joined stage, before any legacy/public error
/// rendering. Body remains primary; a simultaneous actual owner-current exit
/// fault is separately retained by value. No Clone/flatten/legacy projection.
#[derive(Debug)]
pub(crate) enum PgPrivatePweSourceMaterialFailure {
    Body(DurableError),
    OwnerExit(DurableError),
    BodyAndOwnerExit {
        body: DurableError,
        owner_exit: DurableError,
    },
}
type MaterialResult<T> = std::result::Result<T, PgPrivatePweSourceMaterialFailure>;
fn complete_material<T>(
    body: DurableResult<T>,
    owner_exit: DurableResult<()>,
) -> MaterialResult<T> {
    match (body, owner_exit) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(body), Ok(())) => Err(PgPrivatePweSourceMaterialFailure::Body(body)),
        (Ok(_), Err(owner_exit)) => Err(PgPrivatePweSourceMaterialFailure::OwnerExit(owner_exit)),
        (Err(body), Err(owner_exit)) => {
            Err(PgPrivatePweSourceMaterialFailure::BodyAndOwnerExit { body, owner_exit })
        }
    }
}
fn admit_declaration_holders(budget: &mut OperationBudget) -> Result<()> {
    // Same actual incoming ScopeAccess and two Result<()> holders as the
    // accepted a090 owner wrapper; pay BEFORE constructing that accessor.
    budget.charge_bytes(size_of::<ScopeAccess<'_>>() + 2 * size_of::<Result<()>>())
}
impl PgPrivatePweSourceMaterial<'_> {
    pub(crate) fn syntax(&self) -> &PgPrivatePweClaimSyntax<'_> {
        &self.syntax
    }
    pub(crate) fn initial_forms(&self) -> &PgPrivatePweInitialForms<'_> {
        &self.initial_forms
    }
    pub(crate) fn input_identity(&self) -> &PgRecordRevisionIdentity {
        &self.selected
    }
    pub(crate) fn initial_stream_digest(&self) -> Digest256 {
        self.initial_stream
    }
    pub(crate) fn current_stream_digest(&self) -> Digest256 {
        self.current_stream
    }
}
fn overflow() -> Error {
    Error::Unsupported("private PWE source-material admission overflow")
}
fn initial_stream_digest(
    initial: &SelectedPrivateInitialProducerResponse<'_>,
    b: &mut OperationBudget,
    d: Instant,
    c: &AtomicBool,
) -> Result<Digest256> {
    b.charge_bytes(
        size_of::<ScopeAccess<'_>>()
            + size_of::<std::collections::btree_map::Iter<'_, String, Vec<u8>>>()
            + size_of::<Option<&[u8]>>()
            + size_of::<Result<Digest256>>(),
    )?;
    let raw = {
        let mut a = ScopeAccess::admitted_operation(b, d, c);
        a.inspect(
            size_of::<crate::durable_private_packages::PrivateOwnerPackageHistoryEntryV1>(),
            1,
        )?;
        let mut selected = None;
        for (name, raw) in &initial.initial_entry().members {
            a.inspect(size_of::<(String, Vec<u8>)>(), 1)?;
            if a.text_equal(name, claim::CLAIM_STREAM)? {
                selected = Some(raw.as_slice());
                break;
            }
        }
        selected.ok_or(Error::Conflict(
            "actual initial private PWE producer stream is absent",
        ))?
    };
    b.charge_work(raw.len().checked_add(1).ok_or_else(overflow)?)?;
    b.charge_bytes(
        size_of::<Digest256Hasher>() + size_of::<Digest256>() + size_of::<Result<Digest256>>(),
    )?;
    Ok(Digest256::of_bytes(raw))
}
fn bind_initial_stream(
    actual_initial: Digest256,
    reconstructed_initial: Option<Digest256>,
    current: Digest256,
) -> DurableResult<()> {
    // No-history case is only reached after the actual stream-history driver
    // has independently checked that its current records are all initial.
    if reconstructed_initial.unwrap_or(current) != actual_initial {
        return Err(DurableError::Conflict(
            "private PWE reconstructed stream does not start at actual initial producer",
        ));
    }
    Ok(())
}

impl<'store, 'budget, 'cancel, 'pure>
    CurrentPgRoleOwner<'_, '_, '_, 'store, 'budget, 'cancel, 'pure>
{
    /// Actually compose syntax/schema, current producer, retained revision
    /// archives, initial forms and original declaration under one protected
    /// owner and the SAME live RR reader. Not an arbitrary source provider.
    /// The full protected CLI/constructor must still establish its own upfront
    /// original-operation/native/driver quote before entering this stage.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn read_private_pwe_source_material<'history, 'payload>(
        &mut self,
        private: &mut PrivateOwnerSelectedHistoryRead<'history, '_, '_>,
        views: &PgPayloadViews<'payload, 'budget, 'cancel>,
        paths: &BTreeSet<RelativePath>,
        ctx: &CommandContext,
        source_path: &str,
        claim_id: &str,
        d: Instant,
        c: &'cancel AtomicBool,
    ) -> MaterialResult<PgPrivatePweSourceMaterial<'history>> {
        // Resource-only prepay through the actual retained host, before the
        // joined body/result/pair/failure holders and before semantic work.
        // This adds no owner grant or raw host accessor. The real current-owner
        // checks remain below and on the final ordinary Ok/Err exit.
        let prepaid = self.host.with_source_epoch(|_, payer, _same| {
            let holders = size_of::<PgPrivatePweSourceMaterial<'_>>()
                .checked_add(3 * size_of::<DurableResult<PgPrivatePweSourceMaterial<'_>>>())
                .and_then(|n| n.checked_add(size_of::<PgPrivatePweSourceMaterialFailure>()))
                .and_then(|n| {
                    n.checked_add(size_of::<MaterialResult<PgPrivatePweSourceMaterial<'_>>>())
                })
                .and_then(|n| n.checked_add(size_of::<DurableResult<()>>()))
                .and_then(|n| {
                    n.checked_add(size_of::<(
                        DurableResult<PgPrivatePweSourceMaterial<'_>>,
                        DurableResult<()>,
                    )>())
                })
                .and_then(|n| n.checked_add(size_of::<claim::PrivatePweArchivePlan<'_>>()))
                .and_then(|n| n.checked_add(size_of::<super::PgPrivatePweStreamHistory<'_, '_>>()))
                .and_then(|n| {
                    n.checked_add(3 * size_of::<Digest256>() + size_of::<Option<Digest256>>())
                })
                .ok_or(DurableError::Source(overflow()))?;
            // Direct original OperationBudget, not a first STO event that
            // pretends to pay the entry creating that event. These own
            // logical holders do not certify the remaining native startup.
            payer.inspect(holders, 1).map_err(DurableError::Source)
        });
        let body = match prepaid {
            Err(original) => Err(original),
            Ok(()) => (|| {
                self.check_current()?;
                let syntax = self.read_private_pwe_claim_syntax(
                    private,
                    views,
                    paths,
                    ctx,
                    source_path,
                    claim_id,
                    d,
                    c,
                )?;
                let plan = self.private_pwe_archive_requests(private, source_path, claim_id)?;
                self.with_source_epoch(|_, payer, _| {
                    payer
                        .inspect(0, 2 * size_of::<usize>())
                        .map_err(DurableError::Source)
                })?;
                if !std::ptr::eq(
                    syntax.producer().producer().entry(),
                    plan.producer().producer().entry(),
                ) {
                    return Err(DurableError::Conflict(
                        "private PWE syntax and revision plan use different actual RR producers",
                    ));
                }
                let history =
                    self.read_private_pwe_stream_history(private, &plan, source_path, claim_id)?;
                let reconstructed_initial = history.initial_archived_stream_digest();
                let current_stream = history.current_stream_digest();
                // The guard still owns actual archive/current source observations;
                // no raw archive buffer or private reader is replaced below.
                drop(history);
                let initial_forms =
                    self.private_pwe_initial_forms(private, source_path, claim_id)?;
                self.with_source_epoch(|_, payer, _| {
                    payer
                        .inspect(0, 2 * size_of::<usize>())
                        .map_err(DurableError::Source)
                })?;
                let creation = initial_forms.creation();
                if !std::ptr::eq(
                    syntax.producer().producer().entry(),
                    creation.source().current().producer().entry(),
                ) {
                    return Err(DurableError::Conflict(
                        "private PWE initial forms and syntax use different actual RR producers",
                    ));
                }
                let initial_stream = self.with_source_epoch(|_, payer, _| {
                    let (actual_d, actual_c) = payer.source_cutoff();
                    payer
                        .with_operation(|b| {
                            // Reuse the already authentic creation loan; do not repeat
                            // its whole read/schema path or clone its request/config.
                            admit_declaration_holders(b)?;
                            claim::check_private_pwe_initial_declaration(
                                creation.request().root(),
                                creation.normalized_configuration().root(),
                                &mut ScopeAccess::admitted_operation(b, actual_d, actual_c),
                            )?;
                            initial_stream_digest(
                                creation.source().initial(),
                                b,
                                actual_d,
                                actual_c,
                            )
                        })
                        .map_err(DurableError::Source)
                })?;
                self.with_source_epoch(|_, payer, _| {
                    payer
                        .inspect(
                            size_of::<DurableResult<()>>(),
                            2 * size_of::<Digest256>() + 1,
                        )
                        .map_err(DurableError::Source)
                })?;
                bind_initial_stream(initial_stream, reconstructed_initial, current_stream)?;
                self.with_source_epoch(|_, payer, _| {
                    payer
                        .inspect(0, size_of::<PgRecordRevisionIdentity>())
                        .map_err(DurableError::Source)
                })?;
                let selected = *self.input_identity();
                if syntax.input_identity() != &selected {
                    return Err(DurableError::Conflict(
                        "private PWE source material changed selected identity",
                    ));
                }
                Ok(PgPrivatePweSourceMaterial {
                    syntax,
                    initial_forms,
                    selected,
                    initial_stream,
                    current_stream,
                })
            })(),
        };
        let exit = self.check_current();
        complete_material(body, exit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_pwe_stream_join_requires_the_authentic_initial_head_even_for_valid_revision_roots() {
        // Digest relation DATA only. No fake owner, stream proof, native
        // positive, private history or source-material object is constructed.
        let initial = Digest256::of_bytes(b"actual first producer stream");
        let another = Digest256::of_bytes(b"another version-one stream");
        let current = Digest256::of_bytes(b"actual revised current stream");
        assert!(bind_initial_stream(initial, Some(initial), current).is_ok());
        assert!(matches!(
            bind_initial_stream(initial, Some(another), current),
            Err(DurableError::Conflict(
                "private PWE reconstructed stream does not start at actual initial producer"
            ))
        ));
    }
    #[test]
    fn private_pwe_stream_join_does_not_treat_no_archive_as_permission_to_change_initial_bytes() {
        let initial = Digest256::of_bytes(b"actual initial stream");
        assert!(bind_initial_stream(initial, None, initial).is_ok());
        assert!(
            bind_initial_stream(initial, None, Digest256::of_bytes(b"other current stream"))
                .is_err()
        );
    }

    fn owned(text: &str) -> (DurableError, *const u8) {
        let detail = text.to_owned();
        let pointer = detail.as_ptr();
        (
            DurableError::Source(Error::DeniedWithReason(detail)),
            pointer,
        )
    }
    fn assert_owned(error: DurableError, expected: &str, pointer: *const u8) {
        match error {
            DurableError::Source(Error::DeniedWithReason(detail)) => {
                assert_eq!(detail, expected);
                assert_eq!(detail.as_ptr(), pointer);
            }
            _ => panic!("original owned error variant was replaced"),
        }
    }
    #[test]
    fn private_pwe_material_completion_accepts_only_both_successes() {
        let value = String::from("owned successful DATA");
        let pointer = value.as_ptr();
        let returned = complete_material(Ok(value), Ok(())).unwrap();
        assert_eq!(returned.as_ptr(), pointer);
        assert_eq!(returned, "owned successful DATA");
    }
    #[test]
    fn private_pwe_material_completion_keeps_owned_body_error_on_clean_exit() {
        let (body, pointer) = owned("first body detail");
        match complete_material::<()>(Err(body), Ok(())) {
            Err(PgPrivatePweSourceMaterialFailure::Body(original)) => {
                assert_owned(original, "first body detail", pointer)
            }
            _ => panic!("body outcome was flattened"),
        }
    }
    #[test]
    fn private_pwe_material_completion_failed_exit_drops_success_and_keeps_owned_exit() {
        struct Data<'a>(&'a std::cell::Cell<bool>);
        impl Drop for Data<'_> {
            fn drop(&mut self) {
                self.0.set(true);
            }
        }
        let dropped = std::cell::Cell::new(false);
        let (exit, pointer) = owned("actual owner-current exit detail");
        match complete_material(Ok(Data(&dropped)), Err(exit)) {
            Err(PgPrivatePweSourceMaterialFailure::OwnerExit(original)) => {
                assert!(dropped.get());
                assert_owned(original, "actual owner-current exit detail", pointer);
            }
            _ => panic!("successful body escaped failed owner exit"),
        }
    }
    #[test]
    fn private_pwe_material_completion_retains_both_owned_faults_with_body_primary() {
        let (body, body_pointer) = owned("first original body detail");
        let (exit, exit_pointer) = owned("second actual owner-current exit detail");
        match complete_material::<()>(Err(body), Err(exit)) {
            Err(PgPrivatePweSourceMaterialFailure::BodyAndOwnerExit { body, owner_exit }) => {
                assert_owned(body, "first original body detail", body_pointer);
                assert_owned(
                    owner_exit,
                    "second actual owner-current exit detail",
                    exit_pointer,
                );
            }
            _ => panic!("one simultaneous owned fault was discarded"),
        }
    }
    #[test]
    fn private_pwe_material_declaration_holders_refuse_before_accessor_and_keep_prior_work() {
        let d = Instant::now() + std::time::Duration::from_secs(10);
        let mut budget = OperationBudget::new(
            &serde_json::json!({"budgets":{
            "worker_cpu_seconds":3,"worker_address_space_bytes":1_073_741_824}}),
            d,
        )
        .unwrap();
        let quote = size_of::<ScopeAccess<'_>>() + 2 * size_of::<Result<()>>();
        budget.lower_remaining(quote - 1, 64).unwrap();
        budget.charge_work(3).unwrap();
        assert!(matches!(
            admit_declaration_holders(&mut budget),
            Err(Error::Invalid("assessment cumulative byte budget"))
        ));
        assert_eq!(budget.test_work_remaining(), 61);
        assert!(budget.limits().is_err());
        // No accessor, source reader, schema worker or fake owner was created.
    }
}
