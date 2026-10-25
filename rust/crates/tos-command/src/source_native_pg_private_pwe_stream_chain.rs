//! Complete stream-chain subproof on the actual protected owner/RR archives.
//! This deliberately issues no PackageState/creation/form/Claim target or
//! native admission; those remaining semantic gates cannot use a bool here.
use super::{
    CurrentPgRoleOwner, PgPrivatePweArchivedFiles, PrivateOwnerSelectedHistoryRead, overflow,
    same_entry,
};
use crate::source_command::{SourceCommandError as Error, SourceCommandResult as Result};
use crate::source_native_cli::private_assessment::{OperationBudget, scope_admission::ScopeAccess};
use crate::source_native_cli::private_owner::claim::{self, PrivatePweArchivePlan};
use crate::source_native_roles::admitted_document;
use crate::{DurableError, DurableResult};
use std::mem::size_of;
use tos_foundation::{Digest256, JsonDocument, JsonValue};

/// The complete receipt sequence and exact CURRENT head were compared. No
/// raw constructor, Clone, target/use/permission projection. In particular
/// forms/growth_history/creation provenance/schema/endpoint checks are absent
/// from this weaker type and remain required before a real native target.
pub(crate) struct PgPrivatePweStreamHistory<'plan, 'history> {
    _plan: &'plan PrivatePweArchivePlan<'history>,
    initial_stream_digest: Option<Digest256>,
    current_stream_digest: Digest256,
    receipts: usize,
}
impl PgPrivatePweStreamHistory<'_, '_> {
    pub(crate) fn receipt_count(&self) -> usize {
        self.receipts
    }
    pub(crate) fn initial_archived_stream_digest(&self) -> Option<Digest256> {
        self.initial_stream_digest
    }
    pub(crate) fn current_stream_digest(&self) -> Digest256 {
        self.current_stream_digest
    }
}
fn archived<'a>(
    files: &'a PgPrivatePweArchivedFiles<'_, '_, '_>,
    name: &str,
    access: &mut ScopeAccess<'_>,
) -> Result<&'a [u8]> {
    for file in files.files() {
        access.inspect(size_of::<super::PrivatePweArchivedFile<'_>>(), 1)?;
        if access.text_equal(file.name(), name)? {
            return Ok(file.raw());
        }
    }
    Err(Error::Conflict(
        "private Claim package evidence is incomplete",
    ))
}
fn current<'a>(
    plan: &'a PrivatePweArchivePlan<'_>,
    name: &str,
    access: &mut ScopeAccess<'_>,
) -> Result<&'a [u8]> {
    for (key, raw) in &plan.producer().producer().entry().members {
        access.inspect(size_of::<(String, Vec<u8>)>(), 1)?;
        if access.text_equal(key, name)? {
            return Ok(raw);
        }
    }
    Err(Error::Conflict(
        "private Claim package evidence is incomplete",
    ))
}
fn immutable_creation(
    files: &PgPrivatePweArchivedFiles<'_, '_, '_>,
    plan: &PrivatePweArchivePlan<'_>,
    budget: &mut OperationBudget,
    deadline: std::time::Instant,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<()> {
    let mut access = ScopeAccess::admitted_operation(budget, deadline, cancelled);
    for name in [
        "source-create-owner-configuration.json",
        "source-create-request.json",
        "source-create-environment.json",
        "source-create-provenance.jsonl",
        "source-create-receipt.json",
    ] {
        let before = archived(files, name, &mut access)?;
        let now = current(plan, name, &mut access)?;
        access.inspect(before.len().checked_add(now.len()).ok_or_else(overflow)?, 1)?;
        if before != now {
            return Err(Error::Conflict(
                "private Claim immutable creation evidence changed",
            ));
        }
    }
    Ok(())
}

impl<'call, 'owner, 'view, 'store, 'budget, 'cancel, 'pure>
    CurrentPgRoleOwner<'call, 'owner, 'view, 'store, 'budget, 'cancel, 'pure>
{
    /// Iterate EVERY history request in source order; read/check one actual RR
    /// archive at a time with independent original payment and live scope.
    /// No vector of copied packages, second reader or generic archive provider.
    pub(crate) fn read_private_pwe_stream_history<'plan, 'history>(
        &mut self,
        private: &mut PrivateOwnerSelectedHistoryRead<'history, '_, '_>,
        plan: &'plan PrivatePweArchivePlan<'history>,
        source_path: &str,
        claim_id: &str,
    ) -> DurableResult<PgPrivatePweStreamHistory<'plan, 'history>> {
        let body = (|| {
            self.check_current()?;
            self.with_source_epoch(|_, payer, same| {
                let holders = size_of::<PgPrivatePweStreamHistory<'_, '_>>()
                    .checked_add(2 * size_of::<DurableResult<PgPrivatePweStreamHistory<'_, '_>>>())
                    .and_then(|n| n.checked_add(size_of::<JsonDocument>() + size_of::<Vec<&str>>()))
                    .and_then(|n| {
                        n.checked_add(plan.requests().len().checked_mul(size_of::<&str>())?)
                    })
                    .ok_or(DurableError::Source(overflow()))?;
                payer
                    .charge_sto_source_measures(same, 0, holders, 0, 0, 1)
                    .map_err(DurableError::Source)
            })?;
            let actual = self.claim_producer_read_policy(private, source_path, claim_id, false)?;
            self.with_source_epoch(|_, payer, _| {
                payer
                    .inspect(0, 2 * size_of::<usize>())
                    .map_err(DurableError::Source)
            })?;
            if !same_entry(
                plan.producer().producer().entry(),
                actual.producer().entry(),
            ) {
                return Err(DurableError::Conflict(
                    "private PWE stream plan belongs to another RR producer",
                ));
            }
            let (current_digest, current_initial, creation) =
                self.with_source_epoch(|_, payer, _| {
                    let (d, c) = payer.source_cutoff();
                    payer
                        .with_operation(|budget| {
                            let (stream, receipt) = {
                                let mut access = ScopeAccess::admitted_operation(budget, d, c);
                                (
                                    current(plan, claim::CLAIM_STREAM, &mut access)?,
                                    current(plan, claim::RECEIPT_FILE, &mut access)?,
                                )
                            };
                            let (digest, initial) = claim::inspect_private_pwe_stream(
                                stream,
                                actual.declared(),
                                budget,
                                d,
                                c,
                            )?;
                            let receipt = admitted_document(receipt, budget, d, c)?;
                            Ok((digest, initial, receipt))
                        })
                        .map_err(DurableError::Source)
                })?;
            let mut commands: Vec<&str> = Vec::new();
            commands
                .try_reserve_exact(plan.requests().len())
                .map_err(|_| DurableError::Source(overflow()))?;
            let mut expected = None;
            let mut initial = None;
            for index in 0..plan.requests().len() {
                self.with_source_epoch(|_, payer, same| {
                    let bytes = size_of::<super::PgPrivatePweArchiveData<'_, '_>>()
                        + size_of::<PgPrivatePweArchivedFiles<'_, '_, '_>>()
                        + size_of::<claim::PrivatePweStreamRevision>()
                        + 2 * size_of::<DurableResult<claim::PrivatePweStreamRevision>>();
                    payer
                        .charge_sto_source_measures(same, 0, bytes, 0, 0, 1)
                        .map_err(DurableError::Source)
                })?;
                // Each real read/check repeats the actual current protected
                // selection and same-entry/RR/scope/header guards. The driver
                // never accepts a raw files table or user-authored query ref.
                let data =
                    self.read_private_pwe_archive(private, plan, index, source_path, claim_id)?;
                let files =
                    self.check_private_pwe_archive(private, &data, source_path, claim_id)?;
                let command = self.with_source_epoch(|_, payer, _| {
                    let (d, c) = payer.source_cutoff();
                    payer
                        .with_operation(|budget| {
                            let mut access = ScopeAccess::admitted_operation(budget, d, c);
                            access.inspect(size_of::<claim::PrivatePweArchiveRequest>(), 1)?;
                            let request = &plan.requests()[index];
                            let history = plan
                                .history_document()
                                .ok_or(Error::Conflict("private PWE stream history absent"))?;
                            let receipt = access
                                .field(history.root(), "receipts")?
                                .as_array()
                                .and_then(|a| a.get(request.receipt_index()))
                                .ok_or(Error::Conflict("private PWE stream receipt absent"))?;
                            let command = access.text(receipt, "command_id")?;
                            let create_command = access.text(creation.root(), "command_id")?;
                            if access.text_equal(command, create_command)? {
                                return Err(Error::Conflict(
                                    "private Claim command identity is repeated",
                                ));
                            }
                            for previous in &commands {
                                if access.text_equal(command, previous)? {
                                    return Err(Error::Conflict(
                                        "private Claim command identity is repeated",
                                    ));
                                }
                            }
                            Ok((command, receipt))
                        })
                        .map_err(DurableError::Source)
                })?;
                let link = self.with_source_epoch(|_, payer, _| {
                    let (d, c) = payer.source_cutoff();
                    payer
                        .with_operation(|budget| {
                            immutable_creation(&files, plan, budget, d, c)?;
                            let raw = archived(
                                &files,
                                claim::CLAIM_STREAM,
                                &mut ScopeAccess::admitted_operation(budget, d, c),
                            )?;
                            claim::reconstruct_private_pwe_stream_revision(
                                raw,
                                actual.declared(),
                                command.1,
                                budget,
                                d,
                                c,
                            )
                        })
                        .map_err(DurableError::Source)
                })?;
                self.with_source_epoch(|_, payer, _| {
                    payer
                        .inspect(0, 2 * size_of::<Digest256>() + size_of::<bool>())
                        .map_err(DurableError::Source)
                })?;
                if let Some(expected) = expected {
                    if link.predecessor != expected {
                        return Err(DurableError::Source(Error::Conflict(
                            "private Claim archived stream chain is broken",
                        )));
                    }
                } else {
                    if !link.all_predecessors_initial {
                        return Err(DurableError::Source(Error::Conflict(
                            "private Claim history omits its initial stream",
                        )));
                    }
                    initial = Some(link.predecessor);
                }
                expected = Some(link.successor);
                commands.push(command.0);
                // All raw owned archive buffers drop here, after sequential
                // inspection, without refunding any prior successful debit.
            }
            self.with_source_epoch(|_, payer, _| {
                payer
                    .inspect(0, 2 * size_of::<Digest256>() + size_of::<bool>())
                    .map_err(DurableError::Source)
            })?;
            if let Some(expected) = expected {
                if expected != current_digest {
                    return Err(DurableError::Source(Error::Conflict(
                        "private Claim current stream is not its retained history head",
                    )));
                }
            } else if !current_initial {
                return Err(DurableError::Source(Error::Conflict(
                    "private Claim stream has revisions without retained history",
                )));
            }
            Ok(PgPrivatePweStreamHistory {
                _plan: plan,
                initial_stream_digest: initial,
                current_stream_digest: current_digest,
                receipts: plan.requests().len(),
            })
        })();
        let exit = self.check_current();
        match (body, exit) {
            (Err(original), _) | (Ok(_), Err(original)) => Err(original),
            (Ok(value), Ok(())) => Ok(value),
        }
    }
}
