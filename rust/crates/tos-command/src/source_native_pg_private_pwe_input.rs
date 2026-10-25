//! Actual protected private Claim syntax lane. This is deliberately weaker
//! than an integrity-checked target: package_state/creation/archive/form,
//! endpoint/maker/event, full basis and native admission remain unissued.
//! No code here enters the generic evaluator or its held PWE exception.
use super::{CurrentPgRoleOwner, PgClaimProducerReadPolicy};
use crate::durable_private_packages::PrivateOwnerSelectedHistoryRead;
use crate::pwe_source_owner::PgPayloadViews;
use crate::source_cohort::revision::PgRecordRevisionIdentity;
use crate::source_command::{
    CommandContext, SourceCommandError as Error, SourceCommandResult as Result,
};
use crate::source_native_cli::private_assessment::scope_admission::ScopeAccess;
use crate::source_native_cli::private_owner::claim::{
    self, ParsedPrivatePweClaim, PrivatePweClaimFamily,
};
use crate::source_native_roles::{admitted_document, with_pg_private_schema_kernel};
use crate::{DurableError, DurableResult};
use std::{collections::BTreeSet, mem::size_of, sync::atomic::AtomicBool, time::Instant};
use tos_foundation::{JsonDocument, RelativePath};
use tos_validation::source_cut::CutSchemaExecutor;

const BINDINGS: &str = "ToS/doctrine/semantic-interchange/pwe-bindings.v1.json";
const BINDINGS_SCHEMA: &str = "ToS/contracts/pwe-bindings.schema.json";
const RELATIONS: &str = "ToS/doctrine/semantic-interchange/relation-types.v1.json";
const RELATIONS_SCHEMA: &str = "ToS/contracts/semantic-relation-type-registry.schema.json";

/// Genuine immutable RR source plus strict parsing and actual schema checks.
/// No raw constructor, Clone, grant, target, basis, NativeAccount, report or
/// can_use projection. Reading fields does not satisfy omitted integrity law.
pub(crate) struct PgPrivatePweClaimSyntax<'history> {
    producer: PgClaimProducerReadPolicy<'history>,
    parsed: ParsedPrivatePweClaim,
    selected: PgRecordRevisionIdentity,
}
impl PgPrivatePweClaimSyntax<'_> {
    pub(crate) fn document(&self) -> &JsonDocument {
        self.parsed.document()
    }
    pub(crate) fn family(&self) -> PrivatePweClaimFamily {
        self.parsed.family()
    }
    pub(crate) fn input_identity(&self) -> &PgRecordRevisionIdentity {
        &self.selected
    }
    pub(crate) fn producer(&self) -> &PgClaimProducerReadPolicy<'_> {
        &self.producer
    }
}
fn overflow() -> Error {
    Error::Unsupported("private PWE syntax holder overflow")
}

fn check_source_schema<W: CutSchemaExecutor + ?Sized>(
    worker: &mut W,
    path: &str,
    raw: &[u8],
    schema: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<()> {
    if worker.selected_contract_digest(schema).is_none() {
        return Err(Error::Conflict(
            "private PWE schema outside retained worker closure",
        ));
    }
    match worker.check(path, raw, schema, deadline, cancelled) {
        Ok(true) => Ok(()),
        Ok(false) => Err(Error::Invalid("private PWE source schema rejected bytes")),
        Err(reason) => Err(Error::SchemaExecution {
            path: path.to_owned(),
            root: schema.to_owned(),
            reason,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, time::Duration};
    use tos_foundation::Digest256;
    use tos_validation::item_rules::ItemRefusal;

    // Negative-only source-schema helper fixture. It constructs no host,
    // owner, worker binding, producer, current selection or positive issuer.
    struct Rejected {
        contract: bool,
        calls: Cell<usize>,
        detail: Option<String>,
    }
    impl CutSchemaExecutor for Rejected {
        fn selected_contract_digest(&self, path: &str) -> Option<Digest256> {
            (self.contract && path == BINDINGS_SCHEMA)
                .then(|| Digest256::of_bytes(b"fixture schema operand"))
        }
        fn check(
            &mut self,
            _: &str,
            _: &[u8],
            _: &str,
            _: Instant,
            _: &AtomicBool,
        ) -> std::result::Result<bool, ItemRefusal> {
            self.calls.set(self.calls.get() + 1);
            match self.detail.take() {
                Some(detail) => Err(ItemRefusal::Source(detail)),
                None => Ok(false),
            }
        }
    }

    #[test]
    fn private_pwe_schema_helper_retains_owned_native_source_detail() {
        let deadline = Instant::now() + Duration::from_secs(30);
        let cancelled = AtomicBool::new(false);
        let detail = String::from("original schema transport detail, not a compatibility category");
        let ptr = detail.as_ptr();
        let mut worker = Rejected {
            contract: true,
            calls: Cell::new(0),
            detail: Some(detail),
        };
        match check_source_schema(
            &mut worker,
            BINDINGS,
            b"{}",
            BINDINGS_SCHEMA,
            deadline,
            &cancelled,
        ) {
            Err(Error::SchemaExecution {
                path,
                root,
                reason: ItemRefusal::Source(actual),
            }) => {
                assert_eq!(actual.as_ptr(), ptr);
                assert_eq!(path, BINDINGS);
                assert_eq!(root, BINDINGS_SCHEMA);
            }
            other => panic!("expected the original owned schema detail: {other:?}"),
        }
        assert_eq!(worker.calls.get(), 1);
    }

    #[test]
    fn private_pwe_schema_helper_never_substitutes_true_for_missing_or_negative_schema() {
        let deadline = Instant::now() + Duration::from_secs(30);
        let cancelled = AtomicBool::new(false);
        let mut worker = Rejected {
            contract: false,
            calls: Cell::new(0),
            detail: None,
        };
        assert!(matches!(
            check_source_schema(
                &mut worker,
                BINDINGS,
                b"{}",
                BINDINGS_SCHEMA,
                deadline,
                &cancelled
            ),
            Err(Error::Conflict(
                "private PWE schema outside retained worker closure"
            ))
        ));
        assert_eq!(worker.calls.get(), 0);
        worker.contract = true;
        assert!(matches!(
            check_source_schema(
                &mut worker,
                BINDINGS,
                b"{}",
                BINDINGS_SCHEMA,
                deadline,
                &cancelled
            ),
            Err(Error::Invalid("private PWE source schema rejected bytes"))
        ));
        assert_eq!(worker.calls.get(), 1);
    }
}

impl<'store, 'budget, 'cancel, 'pure>
    CurrentPgRoleOwner<'_, '_, '_, 'store, 'budget, 'cancel, 'pure>
{
    /// Read/parse/check the selected CURRENT private Claim using its actual
    /// producer and current protected read selection. All three PWE forms
    /// share this syntax lane. It never declares full Claim lineage or source
    /// target validity merely because a schema accepted a local_only document.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn read_private_pwe_claim_syntax<'history, 'payload>(
        &mut self,
        private: &PrivateOwnerSelectedHistoryRead<'history, '_, '_>,
        views: &PgPayloadViews<'payload, 'budget, 'cancel>,
        paths: &BTreeSet<RelativePath>,
        ctx: &CommandContext,
        source_path: &str,
        claim_id: &str,
        deadline: Instant,
        cancelled: &'cancel AtomicBool,
    ) -> DurableResult<PgPrivatePweClaimSyntax<'history>> {
        let body = (|| {
            self.check_current()?;
            self.with_source_epoch(|_, payer, same| {
                let (actual_deadline, actual_cancelled) = payer.source_cutoff();
                if deadline != actual_deadline || !std::ptr::eq(cancelled, actual_cancelled) {
                    return Err(DurableError::Conflict(
                        "private PWE source cutoff differs from original owner",
                    ));
                }
                let bytes = size_of::<PgPrivatePweClaimSyntax<'_>>()
                    .checked_add(2 * size_of::<JsonDocument>())
                    .and_then(|n| n.checked_add(2 * size_of::<RelativePath>()))
                    .and_then(|n| {
                        n.checked_add(3 * size_of::<DurableResult<PgPrivatePweClaimSyntax<'_>>>())
                    })
                    .and_then(|n| n.checked_add(BINDINGS.len() + RELATIONS.len()))
                    .ok_or(DurableError::Source(overflow()))?;
                payer
                    .charge_sto_source_measures(
                        same,
                        0,
                        bytes,
                        0,
                        (BINDINGS.len() + RELATIONS.len()) as u64,
                        1,
                    )
                    .map_err(DurableError::Source)
            })?;
            let producer =
                self.claim_producer_read_policy(private, source_path, claim_id, false)?;
            let bindings_path = RelativePath::parse(BINDINGS).map_err(|_| {
                DurableError::Source(Error::Invalid("private PWE binding source path"))
            })?;
            let relations_path = RelativePath::parse(RELATIONS).map_err(|_| {
                DurableError::Source(Error::Invalid("private PWE relation source path"))
            })?;
            let bindings_member = self.selected_member(views, &bindings_path)?;
            let relations_member = self.selected_member(views, &relations_path)?;
            let grant = self.grant;
            let owner = self.owner;
            // The real callback token never leaves this protected selector.
            // Parser operation loans END before any native worker call below.
            let ((parsed, claim_stream), bindings, relations) =
                self.host.with_source_epoch(|_, payer, same| {
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
                                let parsed = claim::parse_selected_claim_stream(
                                    &producer, &selected, budget, deadline, cancelled,
                                )?;
                                let bindings = admitted_document(
                                    bindings_member.raw(),
                                    budget,
                                    deadline,
                                    cancelled,
                                )?;
                                let relations = admitted_document(
                                    relations_member.raw(),
                                    budget,
                                    deadline,
                                    cancelled,
                                )?;
                                Ok((parsed, bindings, relations))
                            })
                        },
                    )
                    .map_err(DurableError::Source)
                })?;
            let dependencies = self.host.with_source_epoch(|_, payer, same| {
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
                            let mut access =
                                ScopeAccess::admitted_operation(budget, deadline, cancelled);
                            parsed.check_registered_source(
                                bindings.root(),
                                relations.root(),
                                &selected,
                                &mut access,
                            )
                        })
                    },
                )
                .map_err(DurableError::Source)
            })?;
            let claim_raw = claim_stream
                .get(parsed.raw_range())
                .ok_or(DurableError::Corrupt(
                    "private PWE parsed source range differs",
                ))?;
            self.with_source_epoch(|_, payer, same| {
                // Each possible SchemaExecution path/root backing BEFORE the
                // corresponding native call; original ItemRefusal remains owned.
                let backing = BINDINGS
                    .len()
                    .checked_add(BINDINGS_SCHEMA.len())
                    .and_then(|n| n.checked_add(RELATIONS.len() + RELATIONS_SCHEMA.len()))
                    .and_then(|n| n.checked_add(source_path.len() + parsed.family().schema().len()))
                    .and_then(|n| n.checked_add(3 * (size_of::<Error>() + size_of::<Result<()>>())))
                    .ok_or(DurableError::Source(overflow()))?;
                payer
                    .charge_sto_source_measures(same, 0, backing, 0, 0, 3)
                    .map_err(DurableError::Source)
            })?;
            with_pg_private_schema_kernel(
                self,
                views,
                paths,
                ctx,
                deadline,
                cancelled,
                |worker| {
                    check_source_schema(
                        worker,
                        BINDINGS,
                        bindings_member.raw(),
                        BINDINGS_SCHEMA,
                        deadline,
                        cancelled,
                    )?;
                    check_source_schema(
                        worker,
                        RELATIONS,
                        relations_member.raw(),
                        RELATIONS_SCHEMA,
                        deadline,
                        cancelled,
                    )?;
                    // Iteration/UTF8 inspection prepaid above on original payer;
                    // each contract accessor adds the real closed resource fence.
                    for dependency in dependencies {
                        let path = dependency
                            .as_str()
                            .ok_or(Error::Invalid("private PWE dependency path"))?;
                        if worker.selected_contract_digest(path).is_none() {
                            return Err(Error::Unsupported(
                                "PWE required schema dependency not selected",
                            ));
                        }
                    }
                    check_source_schema(
                        worker,
                        source_path,
                        claim_raw,
                        parsed.family().schema(),
                        deadline,
                        cancelled,
                    )
                },
            )?;
            let selected = *self.input_identity();
            // These are not successful substitutes for the still-unimplemented
            // package/creation/archive/form/endpoint/PG basis/dispatch gates.
            Ok(PgPrivatePweClaimSyntax {
                producer,
                parsed,
                selected,
            })
        })();
        let exit = self.check_current();
        match (body, exit) {
            (Err(original), _) | (Ok(_), Err(original)) => Err(original),
            (Ok(value), Ok(())) => Ok(value),
        }
    }
}
