//! Lexical immutable payload custody over an actual PG source read epoch.
//!
//! This is a new, explicit selected-buffer lane, not a memo of filesystem
//! currentness. It never replaces the existing FS capture verifier. The first
//! read executes the actual committed-current SQL/STO owner, retains its normal
//! observation, and moves its verified returned buffer into this private loan.
//! Later uses borrow that SAME immutable buffer and fence the SAME live epoch.
//! No caller-supplied bytes, verified flag, digest-only factory or opaque stamp
//! can create a loan. Its mutable epoch borrow cannot survive the owner callback.
//!
//! This lane certifies neither current physical backing-store health after the
//! initial read nor complete native funding. It does not refresh from a file or
//! assert that a mutable file has become immutable. Rights/head/audit/job changes
//! still fail the actual epoch fence. Selected-read policy and semantic/retained
//! closure remain separate, required owner obligations.

use super::{PgGrowthCurrentInput, admitted_freshness_data};
use crate::source_cohort::ManagedCurrentMember;
use crate::source_cohort::revision::{PgRecordRevisionIdentity, SourceRevisionReadSession};
use crate::source_command::SourceCommandError;
use crate::source_native_cli::private_assessment::original_payer::OriginalAssessmentPayer;
use crate::{DurableError, DurableResult};
use std::{collections::BTreeSet, mem::size_of};
use tos_compiler::pwe_native::Session;
use tos_foundation::RelativePath;

/// Only `with_current_payload_loan` constructs this type, after the actual
/// selected read. All fields are private, there is no Clone or mutable payload
/// accessor, and each byte borrow is confined to a scoped callback.
/// The real `&mut SourceRevisionReadSession` keeps the issuing epoch alive;
/// there is no address-only same-host test and no ABA allocation assumption.
/// The original mutable Session is also borrowed for the entire loan; callers
/// cannot supply a second Session to a later byte access.
pub(crate) struct PgCurrentPayloadLoan<'host, 'store, 'budget, 'cancel, 'pure> {
    epoch: &'host mut SourceRevisionReadSession<'store>,
    payer: &'host OriginalAssessmentPayer<'budget, 'cancel>,
    selected: PgRecordRevisionIdentity,
    members: Vec<ManagedCurrentMember>,
    session: &'host mut Session<'pure>,
}

/// A borrowed binding, issued only inside a fenced loan callback. The real
/// current-member result supplies custody revision, generation, commit, raw
/// digest/length, dependency claims and placement; none is caller-authored.
/// This carries transport evidence, never semantic membership/current use.
pub(crate) struct PgCurrentPayloadRef<'payload> {
    selected: &'payload PgRecordRevisionIdentity,
    member: &'payload ManagedCurrentMember,
}

impl PgCurrentPayloadRef<'_> {
    pub(crate) fn selected(&self) -> &PgRecordRevisionIdentity {
        self.selected
    }

    pub(crate) fn member(&self) -> &ManagedCurrentMember {
        self.member
    }
}

impl<'store> PgGrowthCurrentInput<'_, 'store> {
    /// Enter an actual selected PG epoch and read the ENTIRE explicit path set.
    /// The selected set is bounded by this input profile, not by corpus size.
    /// No prefix walk, top-N subset or silent truncation supplies completeness.
    /// The existing callback pays every STO request on the original Session,
    /// including EOF/Err prefixes. Freshness-query DATA uses the same payer.
    /// This does not fund the existing committed-read SQL, metadata clones,
    /// foreign driver, initial owner bootstrap or the caller's semantic work.
    /// Those debts are explicit; this method is not a fully funded public API.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn with_current_payload_loan<'budget, 'cancel, 'pure, T>(
        &self,
        payer: &OriginalAssessmentPayer<'budget, 'cancel>,
        session: &mut Session<'pure>,
        paths: &BTreeSet<RelativePath>,
        max_bytes: u64,
        run: impl FnOnce(
            &mut PgCurrentPayloadLoan<'_, 'store, 'budget, 'cancel, 'pure>,
        ) -> DurableResult<T>,
    ) -> DurableResult<T> {
        // Also rejects a swallowed original resource refusal, even at zero.
        payer
            .charge_sto_source_measures(session, 0, 0, 0, 0, 0)
            .map_err(DurableError::Source)?;
        if max_bytes == 0 || paths.is_empty() || paths.len() > self.max_prefix_members {
            return Err(DurableError::Refused(
                "PG payload loan selected set or read cap",
            ));
        }
        let max_bytes = max_bytes.min(self.max_member_bytes as u64);
        let (deadline, cancelled) = payer.source_cutoff();
        let carriers = size_of::<PgCurrentPayloadLoan<'_, '_, '_, '_, '_>>()
            .checked_add(size_of::<DurableResult<ManagedCurrentMember>>())
            .and_then(|n| n.checked_add(size_of::<DurableResult<T>>()))
            .ok_or(DurableError::Source(SourceCommandError::Unsupported(
                "PG payload loan logical carriers overflow",
            )))?;
        let comparisons = paths.iter().try_fold(0usize, |sum, path| {
            path.as_str()
                .len()
                .checked_mul(4)
                .and_then(|work| sum.checked_add(work))
                .ok_or(DurableError::Source(SourceCommandError::Unsupported(
                    "PG payload loan path comparison overflow",
                )))
        })?;
        let backing = paths
            .len()
            .checked_mul(size_of::<ManagedCurrentMember>())
            .ok_or(DurableError::Source(SourceCommandError::Unsupported(
                "PG payload loan selected carrier backing overflow",
            )))?;
        // Logical incoming/result holders and both path comparisons, before
        // construction. This is not a complete activation/driver/RSS quote.
        payer
            .inspect(carriers, comparisons)
            .map_err(DurableError::Source)?;
        let selected = self.identity;
        self.with_selected_read_epoch_with_query_data_admission(
            payer,
            deadline,
            cancelled,
            |epoch| {
                // Pay the whole requested Vec backing before allocation. Old
                // observation/raw clones still belong to the explicit lower
                // metadata/native funding debt; this is not a fake full quote.
                session
                    .retain_source_state(backing)
                    .map_err(|error| DurableError::Source(payer.error(error)))?;
                let mut members = Vec::new();
                members.try_reserve_exact(paths.len()).map_err(|_| {
                    DurableError::Source(
                        payer.error(tos_compiler::pwe_native::Error::ResourceLimit),
                    )
                })?;
                for path in paths {
                    let member = payer.read_current_member_sto_source(
                        session, epoch, &selected, path, max_bytes,
                    )?;
                    if member.path != *path
                        || member.metadata.path != *path
                        || member.current_generation != selected.head_seq
                        || member.metadata.size_bytes != member.raw.len() as u64
                    {
                        return Err(DurableError::Conflict(
                            "PG payload loan differs from its actual current observation",
                        ));
                    }
                    members.push(member);
                }
                // The returned buffer came only from the verified physical
                // current read above; no reconstructed JSON or caller Vec.
                let mut loan = PgCurrentPayloadLoan {
                    epoch,
                    payer,
                    selected,
                    members,
                    session,
                };
                let result = run(&mut loan);
                let cutoff = payer
                    .charge_sto_source_measures(loan.session, 0, 0, 0, 0, 0)
                    .map_err(DurableError::Source);
                complete_use(result, cutoff)
            },
        )
    }
}

impl<'store, 'budget, 'cancel, 'pure> PgCurrentPayloadLoan<'_, 'store, 'budget, 'cancel, 'pure> {
    /// Split the complete retained set from its mutable epoch and Session.
    /// All five borrows share this callback's lifetime; T cannot carry any of
    /// them out. The callback receives no whole mutable loan, so immutable
    /// registry/schema/context views may coexist with independently borrowed
    /// epoch checks and sequential uses of the SAME original Session.
    ///
    /// This does not copy, reread, parse or rehash payloads, and does not grant
    /// current read permission or semantic authority. Each consumer still pays
    /// its actual lookup/parse/worker work and performs its required inner
    /// freshness checks. Physical FS verification is unchanged.
    pub(crate) fn with_selected_payloads<T>(
        &mut self,
        run: impl for<'view> FnOnce(
            &'view PgRecordRevisionIdentity,
            &'view [ManagedCurrentMember],
            &'view mut SourceRevisionReadSession<'store>,
            &'view OriginalAssessmentPayer<'budget, 'cancel>,
            &'view mut Session<'pure>,
        ) -> DurableResult<T>,
    ) -> DurableResult<T> {
        self.payer
            .charge_sto_source_measures(self.session, 0, 0, 0, 0, 0)
            .map_err(DurableError::Source)?;
        // Actual callback storage, split arguments, result and both exit
        // Result carriers are paid BEFORE the view is constructed. This is a
        // logical holder quote, not a driver/activation/native-fit claim.
        let carriers = size_of::<(
            &PgRecordRevisionIdentity,
            &[ManagedCurrentMember],
            &mut SourceRevisionReadSession<'store>,
            &OriginalAssessmentPayer<'budget, 'cancel>,
            &mut Session<'pure>,
        )>()
        .checked_add(std::mem::size_of_val(&run))
        .and_then(|n| n.checked_add(size_of::<PgRecordRevisionIdentity>()))
        .and_then(|n| n.checked_add(size_of::<DurableResult<PgRecordRevisionIdentity>>()))
        .and_then(|n| {
            n.checked_add(size_of::<(
                std::time::Instant,
                &std::sync::atomic::AtomicBool,
            )>())
        })
        .and_then(|n| n.checked_add(size_of::<DurableResult<T>>()))
        .and_then(|n| n.checked_add(2 * size_of::<DurableResult<()>>()))
        .ok_or(DurableError::Source(SourceCommandError::Unsupported(
            "PG selected payload split carriers overflow",
        )))?;
        self.payer.reserve(carriers).map_err(DurableError::Source)?;
        let (deadline, cancelled) = self.payer.source_cutoff();
        let identity = self
            .payer
            .with_operation(|budget| {
                self.epoch
                    .selection()
                    .identity_with_data_admission(budget, deadline, cancelled)
            })
            .map_err(DurableError::Source)?;
        if identity != self.selected {
            return Err(DurableError::Conflict("PG payload issuing epoch changed"));
        }
        admitted_freshness_data(self.epoch, self.payer, deadline, cancelled)?;
        // Disjoint private fields only: no second epoch/Session, mutable
        // payload reference, pointer stamp or manufactured readset.
        let result = run(
            &self.selected,
            &self.members,
            &mut *self.epoch,
            self.payer,
            &mut *self.session,
        );
        let cutoff = self
            .payer
            .charge_sto_source_measures(self.session, 0, 0, 0, 0, 0)
            .map_err(DurableError::Source);
        // Attempt the real paid source exit on every callback Result. As on
        // the enclosing owner path, resource refusal may deny physical SQL.
        let fenced = admitted_freshness_data(self.epoch, self.payer, deadline, cancelled);
        complete_use(result, complete_use(cutoff, fenced))
    }

    /// Borrow exact bytes without copying/rehashing that already verified
    /// private buffer. Every invocation checks the real original Session and
    /// the actual source epoch before AND after its callback, including Err.
    /// The callback owns prospective payment for inspecting/parsing the bytes;
    /// merely borrowing a slice does not debit their size a second time.
    pub(crate) fn with_bytes<T>(
        &mut self,
        path: &RelativePath,
        run: impl FnOnce(PgCurrentPayloadRef<'_>, &mut Session<'pure>) -> DurableResult<T>,
    ) -> DurableResult<T> {
        self.payer
            .charge_sto_source_measures(self.session, 0, 0, 0, 0, 0)
            .map_err(DurableError::Source)?;
        let (deadline, cancelled) = self.payer.source_cutoff();
        let carriers = size_of::<DurableResult<T>>()
            .checked_add(size_of::<PgCurrentPayloadRef<'_>>())
            .ok_or(DurableError::Source(SourceCommandError::Unsupported(
                "PG payload borrowed result carriers overflow",
            )))?;
        self.payer.reserve(carriers).map_err(DurableError::Source)?;
        // An admitted identity check on the real borrowed selection, not a
        // caller-authored identity or a fabricated finalized readset.
        let identity = self
            .payer
            .with_operation(|budget| {
                self.epoch
                    .selection()
                    .identity_with_data_admission(budget, deadline, cancelled)
            })
            .map_err(DurableError::Source)?;
        if identity != self.selected {
            return Err(DurableError::Conflict("PG payload issuing epoch changed"));
        }
        admitted_freshness_data(self.epoch, self.payer, deadline, cancelled)?;
        let result = (|| {
            // Each comparison can inspect both common prefixes plus its first
            // boundary. Bounding by every SELECTED member is conservative even
            // for the binary search; it never depends on the whole corpus size.
            let lookup_work = path
                .as_str()
                .len()
                .checked_add(1)
                .and_then(|n| n.checked_mul(2))
                .and_then(|n| n.checked_mul(self.members.len()))
                .ok_or(DurableError::Source(SourceCommandError::Unsupported(
                    "PG payload selected lookup work overflow",
                )))?;
            self.payer
                .inspect(0, lookup_work)
                .map_err(DurableError::Source)?;
            let index = self
                .members
                .binary_search_by(|member| member.path.cmp(path));
            match index {
                Err(_) => Err(DurableError::Refused(
                    "PG payload path outside selected loan",
                )),
                Ok(index) => run(
                    PgCurrentPayloadRef {
                        selected: &self.selected,
                        member: &self.members[index],
                    },
                    self.session,
                ),
            }
        })();
        let cutoff = self
            .payer
            .charge_sto_source_measures(self.session, 0, 0, 0, 0, 0)
            .map_err(DurableError::Source);
        // Attempt the source fence even on callback Err. A resource refusal
        // can prevent physical SQL, as on the existing paid epoch route.
        let fenced = admitted_freshness_data(self.epoch, self.payer, deadline, cancelled);
        complete_use(result, complete_use(cutoff, fenced))
    }
}

fn complete_use<T>(result: DurableResult<T>, fenced: DurableResult<()>) -> DurableResult<T> {
    match (result, fenced) {
        (Err(original), _) | (Ok(_), Err(original)) => Err(original),
        (Ok(value), Ok(())) => Ok(value),
    }
}
