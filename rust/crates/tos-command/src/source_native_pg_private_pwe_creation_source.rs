//! Initial/current producer source for the still-separate creation interpreter.
//! Immutable creation bytes and broker metadata are NOT creator eligibility,
//! schemas/forms/provenance/semantic target/basis/report or current permission.
use super::{CurrentPgRoleOwner, PgClaimProducerReadPolicy, complete_owner_loan};
use crate::durable_private_packages::{
    PrivateOwnerSelectedHistoryRead, SelectedPrivateInitialProducerResponse,
};
use crate::source_command::{SourceCommandError as Error, SourceCommandResult as Result};
use crate::source_native_cli::private_assessment::scope_admission::ScopeAccess;
use crate::source_native_cli::private_owner::claim;
use crate::{DurableError, DurableResult};
use std::{collections::BTreeMap, mem::size_of};

#[path = "source_native_pg_private_pwe_creation_receipt.rs"]
mod receipt;
pub(crate) use receipt::PgPrivatePweCreationReceipt;
#[path = "source_native_pg_private_pwe_creation_forms.rs"]
mod forms;
pub(crate) use forms::PgPrivatePweInitialForms;

pub(crate) struct PgPrivatePweCreationSource<'history> {
    current: PgClaimProducerReadPolicy<'history>,
    initial: SelectedPrivateInitialProducerResponse<'history>,
}
impl PgPrivatePweCreationSource<'_> {
    pub(crate) fn current(&self) -> &PgClaimProducerReadPolicy<'_> {
        &self.current
    }
    pub(crate) fn initial(&self) -> &SelectedPrivateInitialProducerResponse<'_> {
        &self.initial
    }
}
fn overflow() -> Error {
    Error::Unsupported("private PWE creation source holders overflow")
}
fn member<'a>(
    members: &'a BTreeMap<String, Vec<u8>>,
    name: &str,
    access: &mut ScopeAccess<'_>,
) -> Result<&'a [u8]> {
    for (key, raw) in members {
        access.inspect(size_of::<(String, Vec<u8>)>(), 1)?;
        if access.text_equal(key, name)? {
            return Ok(raw);
        }
    }
    Err(Error::Conflict("private PWE creation source member absent"))
}
fn unchanged_initial(
    initial: &BTreeMap<String, Vec<u8>>,
    current: &BTreeMap<String, Vec<u8>>,
    access: &mut ScopeAccess<'_>,
) -> Result<()> {
    for name in [
        "source-create-owner-configuration.json",
        "source-create-request.json",
        "source-create-environment.json",
        "source-create-provenance.jsonl",
        "source-create-receipt.json",
    ] {
        let before = member(initial, name, access)?;
        let now = member(current, name, access)?;
        access.inspect(before.len().checked_add(now.len()).ok_or_else(overflow)?, 1)?;
        if before != now {
            return Err(Error::Conflict(
                "private PWE initial creation bytes changed",
            ));
        }
    }
    Ok(())
}

impl CurrentPgRoleOwner<'_, '_, '_, '_, '_, '_, '_> {
    /// Enforce the original request's retained declaration constraints on
    /// all three PWE families after reselecting THIS actual current owner/RR
    /// source. No token/Grant/permission is returned; normalized declaration
    /// consistency still is not historical grant authentication/eligibility.
    pub(crate) fn check_private_pwe_initial_declaration<'history>(
        &mut self,
        private: &PrivateOwnerSelectedHistoryRead<'history, '_, '_>,
        path: &str,
        claim: &str,
    ) -> DurableResult<()> {
        let body = (|| {
            self.check_current()?;
            let creation = self.private_pwe_creation_receipt(private, path, claim)?;
            self.with_source_epoch(|_,payer,_same| {
                let (d,c)=payer.source_cutoff();
                payer.with_operation(|budget| {
                    budget.charge_bytes(size_of::<ScopeAccess<'_>>()+2*size_of::<Result<()>>())?;
                    crate::source_native_cli::private_owner::claim::check_private_pwe_initial_declaration(
                        creation.request().root(),creation.normalized_configuration().root(),
                        &mut ScopeAccess::admitted_operation(budget,d,c),
                    )
                }).map_err(DurableError::Source)
            })
        })();
        let exit = self.check_current();
        match (body, exit) {
            (Err(original), _) | (Ok(_), Err(original)) => Err(original),
            (Ok(value), Ok(())) => Ok(value),
        }
    }

    /// Bind initial request/receipt/stream to actual broker metadata. It still
    /// does NOT issue full creation integrity, permission or native inputs.
    pub(crate) fn private_pwe_creation_receipt<'history>(
        &mut self,
        private: &PrivateOwnerSelectedHistoryRead<'history, '_, '_>,
        source_path: &str,
        claim_id: &str,
    ) -> DurableResult<PgPrivatePweCreationReceipt<'history>> {
        let body = (|| {
            self.check_current()?;
            self.with_source_epoch(|_, payer, same| {
                let bytes = size_of::<PgPrivatePweCreationSource<'_>>()
                    .checked_add(size_of::<PgPrivatePweCreationReceipt<'_>>())
                    .and_then(|n| {
                        n.checked_add(
                            2 * size_of::<DurableResult<PgPrivatePweCreationReceipt<'_>>>(),
                        )
                    })
                    .ok_or(DurableError::Source(overflow()))?;
                payer
                    .charge_sto_source_measures(same, 0, bytes, 0, 0, 1)
                    .map_err(DurableError::Source)
            })?;
            let source = self.private_pwe_creation_source(private, source_path, claim_id)?;
            self.with_source_epoch(|_, payer, _| {
                let (d, c) = payer.source_cutoff();
                payer
                    .with_operation(|budget| receipt::read(source, budget, d, c))
                    .map_err(DurableError::Source)
            })
        })();
        let exit = self.check_current();
        match (body, exit) {
            (Err(original), _) | (Ok(_), Err(original)) => Err(original),
            (Ok(value), Ok(())) => Ok(value),
        }
    }

    /// Actual live current selection/context and genuine FIRST broker row of
    /// this same selected RR history; never an arbitrary history/map factory.
    /// Every use must separately recheck current policy and pay original cost.
    pub(crate) fn private_pwe_creation_source<'history>(
        &mut self,
        private: &PrivateOwnerSelectedHistoryRead<'history, '_, '_>,
        source_path: &str,
        claim_id: &str,
    ) -> DurableResult<PgPrivatePweCreationSource<'history>> {
        let body = (|| {
            self.check_current()?;
            self.with_source_epoch(|_, payer, same| {
                let holders = size_of::<PgPrivatePweCreationSource<'_>>()
                    .checked_add(size_of::<Option<DurableError>>())
                    .and_then(|n| {
                        n.checked_add(size_of::<
                            Result<Option<SelectedPrivateInitialProducerResponse<'_>>>,
                        >())
                    })
                    .and_then(|n| {
                        n.checked_add(
                            2 * size_of::<DurableResult<PgPrivatePweCreationSource<'_>>>(),
                        )
                    })
                    .ok_or(DurableError::Source(overflow()))?;
                payer
                    .charge_sto_source_measures(same, 0, holders, 0, 0, 1)
                    .map_err(DurableError::Source)
            })?;
            let current = self.claim_producer_read_policy(private, source_path, claim_id, false)?;
            let grant = self.grant;
            let owner = self.owner;
            let initial = self.host.with_source_epoch(|_, payer, same| {
                let (d, c) = payer.source_cutoff();
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
                                let mut access = ScopeAccess::admitted_operation(budget, d, c);
                                let path = access.text(current.declared(), "source_path")?;
                                if !access.text_equal(path, selected.path())? {
                                    return Err(Error::Conflict(
                                        "private PWE creation current selection differs",
                                    ));
                                }
                            }
                            let initial = match private
                                .initial_producer_response_with_data_admission(
                                    current.producer().target_ref(),
                                    budget,
                                    d,
                                    c,
                                ) {
                                Ok(initial) => initial,
                                Err(DurableError::Source(original)) => return Err(original),
                                Err(original) => {
                                    foreign = Some(original);
                                    return Ok(None);
                                }
                            };
                            let mut access = ScopeAccess::admitted_operation(budget, d, c);
                            access.inspect(0, 2 * size_of::<usize>())?;
                            if !std::ptr::eq(initial.current().entry(), current.producer().entry())
                            {
                                return Err(Error::Conflict(
                                    "private PWE creation initial/current RR producer differs",
                                ));
                            }
                            unchanged_initial(
                                &initial.initial_entry().members,
                                &current.producer().entry().members,
                                &mut access,
                            )?;
                            Ok(Some(initial))
                        })
                    },
                );
                complete_owner_loan(result, foreign)
            })?;
            // raw_request_digest and owner_configuration_digest in the genuine
            // first broker row remain distinct from normalized CONFIG_FILE and
            // response/receipt compound commitment. No false hash equality.
            Ok(PgPrivatePweCreationSource { current, initial })
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
        sync::atomic::AtomicBool,
        time::{Duration, Instant},
    };
    fn setup() -> (Instant, AtomicBool, OperationBudget) {
        let d = Instant::now() + Duration::from_secs(30);
        (
            d,
            AtomicBool::new(false),
            OperationBudget::new(
                &serde_json::json!({"budgets":{
                    "worker_cpu_seconds":1,"worker_address_space_bytes":1048576
                }}),
                d,
            )
            .unwrap(),
        )
    }
    fn files() -> BTreeMap<String, Vec<u8>> {
        [
            "source-create-owner-configuration.json",
            "source-create-request.json",
            "source-create-environment.json",
            "source-create-provenance.jsonl",
            "source-create-receipt.json",
        ]
        .into_iter()
        .map(|name| (name.to_owned(), format!("exact:{name}").into_bytes()))
        .collect()
    }
    #[test]
    fn private_pwe_creation_source_rejects_changed_or_missing_immutable_original_member() {
        // DATA predicate only; no PgPrivatePweCreationSource or guard factory.
        let original = files();
        for name in original.keys() {
            let mut current = original.clone();
            current.get_mut(name).unwrap().push(b'!');
            let (d, c, mut b) = setup();
            assert!(
                unchanged_initial(
                    &original,
                    &current,
                    &mut ScopeAccess::admitted_operation(&mut b, d, &c)
                )
                .is_err()
            );
            let mut current = original.clone();
            current.remove(name);
            let (d, c, mut b) = setup();
            assert!(
                unchanged_initial(
                    &original,
                    &current,
                    &mut ScopeAccess::admitted_operation(&mut b, d, &c)
                )
                .is_err()
            );
        }
        let (d, c, mut b) = setup();
        assert!(
            unchanged_initial(
                &original,
                &original,
                &mut ScopeAccess::admitted_operation(&mut b, d, &c)
            )
            .is_ok()
        );
    }
}
