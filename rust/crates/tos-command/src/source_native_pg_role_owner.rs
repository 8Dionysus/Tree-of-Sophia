//! Live protected owner envelope for the PG C4 reader. This is neither a
//! formal-role result nor a grant to use a role/account. It supplies no source
//! bytes, private package permission, schema verdict or qualified assessment.
//! The native reader must still prove exact owner/history/profile/role, and
//! PWE must still check the actual report and application admission.
use super::PgRoleSchemaAccess;
use crate::pwe_source_owner::{PgPayloadHost, PgPayloadMember, PgPayloadViews};
use crate::source_assessment_journal::ProtectedAssessmentPgGrant;
use crate::source_cohort::revision::{
    PgRecordRevisionIdentity, SourceRevisionIdentityClosure, SourceRevisionReadSession,
};
use crate::source_command::{SourceCommandError as Error, SourceCommandResult};
use crate::source_native_cli::private_assessment::original_payer::OriginalAssessmentPayer;
use crate::source_native_cli::private_assessment::scope_admission::ScopeAccess;
use crate::source_text_owner::OwnerTextContext;
use crate::{DurableError, DurableResult};
use std::collections::BTreeSet;
use std::mem::{size_of, size_of_val};
use tos_compiler::pwe_native::Session;
use tos_foundation::{CutPreparedSchemaExecutionBinding, Digest256, FormatProfile};
use tos_foundation::{JsonValue, RelativePath};

#[path = "source_native_pg_claim_producer_policy.rs"]
mod claim_producer_policy;
pub(crate) use claim_producer_policy::PgClaimProducerReadPolicy;

#[path = "source_native_pg_private_pwe_input.rs"]
mod private_pwe_input;
pub(crate) use private_pwe_input::PgPrivatePweClaimSyntax;

/// Private-constructed, non-Clone lexical evidence of the current protected
/// account/context and actual PG host. The borrowed grant is the ACTUAL file-
/// selected Assessment grant, not a historical producer grant or JSON factory.
/// No getter exposes it as permission for an arbitrary source/dependency.
pub(crate) struct CurrentPgRoleOwner<'call, 'owner, 'view, 'store, 'budget, 'cancel, 'pure> {
    host: &'call mut PgPayloadHost<'view, 'store, 'budget, 'cancel, 'pure>,
    grant: &'owner ProtectedAssessmentPgGrant,
    owner: &'owner OwnerTextContext,
    selected: PgRecordRevisionIdentity,
}

fn overflow() -> Error {
    Error::Unsupported("PG role owner envelope overflow")
}

fn complete_presence_loan(
    result: SourceCommandResult<Option<bool>>,
    foreign: Option<DurableError>,
) -> DurableResult<bool> {
    match (result, foreign) {
        // A foreign first body error can outlive a later resource-refused
        // exit. The loan's sticky latch still prevents success; retain the
        // original foreign error instead of its later generic loan refusal.
        (_, Some(original)) => Err(original),
        (Err(original), None) => Err(DurableError::Source(original)),
        (Ok(Some(present)), None) => Ok(present),
        (Ok(None), None) => Err(DurableError::Invalid(
            "PG presence error relay inconsistent",
        )),
    }
}

// The real factory supplies these expected operands from OwnerTextContext.
// This predicate alone cannot construct a grant, owner or CurrentPgRoleOwner.
fn account_matches(
    access: &mut ScopeAccess<'_>,
    config: &JsonValue,
    uid: u32,
    context_path: &str,
) -> SourceCommandResult<()> {
    let selected_uid = access.field(config, "uid")?;
    if let JsonValue::Number(number) = selected_uid {
        access.inspect(number.lexeme.len(), 1)?;
    }
    if selected_uid.as_u64() != Some(u64::from(uid)) {
        return Err(Error::Denied("PG role current owner account differs"));
    }
    let configured_context = access.text(config, "source_context_ref")?;
    if !access.text_equal(configured_context, context_path)? {
        return Err(Error::Denied("PG role current owner context differs"));
    }
    Ok(())
}

/// Attempt both real protected rereads and the actual selected identity, even
/// after an earlier error. The host owns paid epoch/resource entry and exit.
/// Resource refusal can deny physical rereads; no unconditional I/O is claimed.
fn owner_checkpoint(
    host: &mut PgPayloadHost<'_, '_, '_, '_, '_>,
    grant: &ProtectedAssessmentPgGrant,
    owner: &OwnerTextContext,
) -> DurableResult<PgRecordRevisionIdentity> {
    host.with_source_epoch(|epoch, payer, same| {
        let carriers = 3 * size_of::<SourceCommandResult<()>>()
            + size_of::<SourceCommandResult<PgRecordRevisionIdentity>>()
            + size_of::<DurableResult<PgRecordRevisionIdentity>>();
        payer.reserve(carriers).map_err(DurableError::Source)?;
        let grant_check = payer.recheck_current_pg_grant_exact_source(same, grant);
        let owner_check = payer.recheck_owner_context_exact_source(same, owner);
        let (deadline, cancelled) = payer.source_cutoff();
        let binding = payer.with_operation(|budget| {
            let mut access = ScopeAccess::admitted_operation(budget, deadline, cancelled);
            // Price UTF-8 inspection before converting the actual retained
            // owner path. Strings from config are compared by admitted access.
            access.inspect(
                owner
                    .configuration_path()
                    .as_os_str()
                    .as_encoded_bytes()
                    .len(),
                1,
            )?;
            let context = owner
                .configuration_path()
                .to_str()
                .ok_or(Error::Invalid("PG role owner context path UTF-8"))?;
            account_matches(
                &mut access,
                grant.configuration(),
                owner.account_uid(),
                context,
            )
        });
        let identity = payer.with_operation(|budget| {
            epoch
                .selection()
                .identity_with_data_admission(budget, deadline, cancelled)
        });
        match (grant_check, owner_check, binding, identity) {
            (Err(original), _, _, _)
            | (Ok(()), Err(original), _, _)
            | (Ok(()), Ok(()), Err(original), _)
            | (Ok(()), Ok(()), Ok(()), Err(original)) => Err(DurableError::Source(original)),
            (Ok(()), Ok(()), Ok(()), Ok(identity)) => Ok(identity),
        }
    })
}

impl<'store, 'budget, 'cancel, 'pure>
    CurrentPgRoleOwner<'_, '_, '_, 'store, 'budget, 'cancel, 'pure>
{
    pub(crate) fn input_identity(&self) -> &PgRecordRevisionIdentity {
        &self.selected
    }
    pub(super) fn account_uid(&self) -> u64 {
        u64::from(self.owner.account_uid())
    }

    /// A cached mechanical role must be used under this current owner envelope
    /// and the same actual PG selection. This grants no `can_use` admission.
    pub(crate) fn check_current(&mut self) -> DurableResult<()> {
        let selected = owner_checkpoint(self.host, self.grant, self.owner)?;
        // identity_with_data_admission prepays the fixed complete comparison.
        if selected != self.selected {
            return Err(DurableError::Conflict(
                "PG role current owner epoch differs",
            ));
        }
        Ok(())
    }

    /// Native owner/index/history reads use this SAME live epoch, payer and
    /// Session. There is no whole host/worker accessor and no substitute loan.
    /// This envelope does not implement source-specific access/history law.
    pub(crate) fn with_source_epoch<T>(
        &mut self,
        run: impl FnOnce(
            &mut SourceRevisionReadSession<'store>,
            &OriginalAssessmentPayer<'budget, 'cancel>,
            &mut Session<'pure>,
        ) -> DurableResult<T>,
    ) -> DurableResult<T> {
        self.check_current()?;
        let result = self.host.with_source_epoch(run);
        let exited = self.check_current();
        match (result, exited) {
            (Err(original), _) | (Ok(_), Err(original)) => Err(original),
            (Ok(value), Ok(())) => Ok(value),
        }
    }

    /// Original-payer bridge for the physical presence probe. A positive
    /// answer is only navigation; the native reader still must read/validate
    /// the actual current member. Virtual publication carriers remain refused.
    pub(crate) fn has_current_metadata(&mut self, path: &RelativePath) -> DurableResult<bool> {
        self.with_source_epoch(|epoch, payer, same| {
            payer
                .charge_sto_source_measures(same, 0, 0, 0, 0, 0)
                .map_err(DurableError::Source)?;
            let carriers = size_of::<Option<DurableError>>()
                .checked_add(size_of::<SourceCommandResult<Option<bool>>>())
                .and_then(|n| n.checked_add(size_of::<DurableResult<bool>>()))
                .ok_or(DurableError::Source(overflow()))?;
            payer.reserve(carriers).map_err(DurableError::Source)?;
            let (deadline, cancelled) = payer.source_cutoff();
            let mut foreign = None;
            let result = payer.with_operation(|budget| {
                match epoch
                    .has_current_metadata_with_data_admission(path, budget, deadline, cancelled)
                {
                    Ok(present) => Ok(Some(present)),
                    // Resource errors use the original operation Err lane so
                    // its sticky latch cannot replace them with a generic M.
                    Err(DurableError::Source(original)) => Err(original),
                    Err(original) => {
                        // Owned foreign PG/source error, not a fabricated
                        // successful existence result or a rewritten reason.
                        foreign = Some(original);
                        Ok(None)
                    }
                }
            });
            complete_presence_loan(result, foreign)
        })
    }
}

// These operations remain inside the real owner envelope. They lend neither
// the worker nor the owner host, and a point index is navigation, not a role.
impl<'store, 'budget, 'cancel, 'pure>
    CurrentPgRoleOwner<'_, '_, '_, 'store, 'budget, 'cancel, 'pure>
{
    pub(super) fn selected_member<'payload>(
        &mut self,
        views: &PgPayloadViews<'payload, 'budget, 'cancel>,
        path: &RelativePath,
    ) -> DurableResult<PgPayloadMember<'payload>> {
        let body = (|| {
            self.check_current()?;
            views.member(self.host, path)
        })();
        let exit = self.check_current();
        match (body, exit) {
            (Err(original), _) | (Ok(_), Err(original)) => Err(original),
            (Ok(value), Ok(())) => Ok(value),
        }
    }

    pub(super) fn require_public_record(&mut self, id: &str, path: &str) -> DurableResult<()> {
        // Reborrow disjoint fields through the host, never expose the grant as
        // a JSON factory. Owner currentness is checked on both Result exits.
        self.check_current()?;
        let config = self.grant.configuration();
        let body = self.host.with_source_epoch(|_epoch, payer, _same| {
            let (deadline, cancelled) = payer.source_cutoff();
            payer
                .with_operation(|budget| {
                    let mut access = ScopeAccess::admitted_operation(budget, deadline, cancelled);
                    public_selection(&mut access, config, id, path)
                })
                .map_err(DurableError::Source)
        });
        let exit = self.check_current();
        match (body, exit) {
            (Err(original), _) | (Ok(_), Err(original)) => Err(original),
            (Ok(value), Ok(())) => Ok(value),
        }
    }

    pub(super) fn identity_closure(
        &mut self,
        id: &str,
    ) -> DurableResult<SourceRevisionIdentityClosure> {
        self.with_source_epoch(|epoch, payer, _same| {
            // Quote the actual input/key/Result holders before construction;
            // the lower DATA port separately quotes its query/row payloads.
            payer
                .inspect(
                    size_of::<BTreeSet<String>>()
                        + size_of::<String>()
                        + size_of::<SourceRevisionIdentityClosure>()
                        + id.len(),
                    id.len(),
                )
                .map_err(DurableError::Source)?;
            let tokens = BTreeSet::from([id.to_owned()]);
            let (deadline, cancelled) = payer.source_cutoff();
            let mut foreign = None;
            let result = payer.with_operation(|budget| {
                match epoch.lookup_source_identities_with_data_admission(
                    &tokens, budget, deadline, cancelled,
                ) {
                    Ok(value) => Ok(Some(value)),
                    Err(DurableError::Source(original)) => Err(original),
                    Err(original) => {
                        foreign = Some(original);
                        Ok(None)
                    }
                }
            });
            complete_owner_loan(result, foreign)
        })
    }

    pub(super) fn verify_identity_closure(
        &mut self,
        closure: &SourceRevisionIdentityClosure,
    ) -> DurableResult<()> {
        self.with_source_epoch(|epoch, payer, _same| {
            let (deadline, cancelled) = payer.source_cutoff();
            let mut foreign = None;
            let result = payer.with_operation(|budget| {
                match closure
                    .verify_held_session_with_data_admission(epoch, budget, deadline, cancelled)
                {
                    Ok(()) => Ok(Some(())),
                    Err(DurableError::Source(original)) => Err(original),
                    Err(original) => {
                        foreign = Some(original);
                        Ok(None)
                    }
                }
            });
            complete_owner_loan(result, foreign)
        })
    }
}

fn complete_owner_loan<T>(
    result: SourceCommandResult<Option<T>>,
    foreign: Option<DurableError>,
) -> DurableResult<T> {
    match (result, foreign) {
        (_, Some(original)) => Err(original),
        (Err(original), None) => Err(DurableError::Source(original)),
        (Ok(Some(value)), None) => Ok(value),
        (Ok(None), None) => Err(DurableError::Invalid(
            "PG role owner error relay inconsistent",
        )),
    }
}

// Exactly the maintained public Assessment source_records selector. Private
// records/Claims need their different source_access and semantic owner routes;
// this route never promotes their historical grants or payload permission.
fn public_selection(
    access: &mut ScopeAccess<'_>,
    config: &JsonValue,
    id: &str,
    path: &str,
) -> SourceCommandResult<()> {
    access.inspect(path.len().checked_mul(3).ok_or_else(overflow)?, 1)?;
    if !path.starts_with("ToS/source-witnesses/")
        || !path.ends_with(".json")
        || path.ends_with(".human-forms.json")
        || path.ends_with(crate::source_revisions::HISTORY)
        || path.split('/').any(|part| {
            part.starts_with('.')
                || matches!(
                    part,
                    "owner-local" | "catalog" | "payload" | "local-content" | "private"
                )
        })
    {
        return Err(Error::Denied(
            "PG role requires public selected metadata home",
        ));
    }
    let rows = access
        .field(config, "source_records")?
        .as_array()
        .ok_or(Error::Invalid("PG role current source selection array"))?;
    if rows.len() > 1024 {
        return Err(Error::Invalid("PG role current source selection count"));
    }
    let mut found = false;
    for row in rows {
        access.exact_keys(row, &["path", "record_id", "origin_id"])?;
        let selected_id = access.text(row, "record_id")?;
        if !access.text_equal(selected_id, id)? {
            continue;
        }
        if found {
            return Err(Error::Denied("PG role current source identity ambiguous"));
        }
        let selected_path = access.text(row, "path")?;
        let origin = access.text(row, "origin_id")?;
        access.inspect(origin.len(), 1)?;
        if !access.text_equal(selected_path, path)? || !crate::source_command::nonblank(origin) {
            return Err(Error::Denied("PG role current source selection differs"));
        }
        found = true;
    }
    if found {
        Ok(())
    } else {
        Err(Error::Denied(
            "PG role absent from current Assessment selection",
        ))
    }
}

// The exclusive host is PRIVATE to this owner. These are only the three
// closed operations: no worker/host/Session loan or quota mutation escapes.
// The enclosing with_current_pg_role_owner retains real grant/context exits;
// each actual PWE operation independently retains its paid PG/resource exits.
impl PgRoleSchemaAccess for CurrentPgRoleOwner<'_, '_, '_, '_, '_, '_, '_> {
    fn check_record_bytes(
        &mut self,
        source_path: &str,
        raw: &[u8],
        schema_path: &str,
    ) -> DurableResult<bool> {
        self.host.check_record_bytes(source_path, raw, schema_path)
    }
    fn selected_schema_contract_digest(
        &mut self,
        schema_path: &str,
    ) -> DurableResult<Option<Digest256>> {
        self.host.selected_schema_contract_digest(schema_path)
    }
    fn schema_validation_binding(
        &mut self,
    ) -> DurableResult<(FormatProfile, CutPreparedSchemaExecutionBinding)> {
        self.host.schema_validation_binding()
    }
}

/// Execute with the actual protected owner and original PG host. There is no
/// replacement Session/payer parameter. HRTB confines the owner token to this
/// invocation; T cannot contain its borrow. Later semantic role results remain
/// independently owner-issued and do not themselves grant application use.
pub(crate) fn with_current_pg_role_owner<'owner, 'view, 'store, 'budget, 'cancel, 'pure, T>(
    host: &mut PgPayloadHost<'view, 'store, 'budget, 'cancel, 'pure>,
    grant: &'owner ProtectedAssessmentPgGrant,
    owner: &'owner OwnerTextContext,
    run: impl for<'call> FnOnce(
        &mut CurrentPgRoleOwner<'call, 'owner, 'view, 'store, 'budget, 'cancel, 'pure>,
    ) -> DurableResult<T>,
) -> DurableResult<T> {
    host.with_current(|host| {
        // Pay token/result/callback carriers before constructing or issuing
        // any owner token. Whole activation/driver/native costs remain open.
        host.with_source_epoch(|_epoch, payer, _same| {
            let carriers = size_of::<CurrentPgRoleOwner<'_, '_, '_, '_, '_, '_, '_>>()
                .checked_add(size_of::<DurableResult<T>>())
                .and_then(|n| n.checked_add(size_of::<DurableResult<PgRecordRevisionIdentity>>()))
                .and_then(|n| n.checked_add(size_of_val(&run)))
                .ok_or(DurableError::Source(overflow()))?;
            payer.reserve(carriers).map_err(DurableError::Source)
        })?;
        let selected = match owner_checkpoint(host, grant, owner) {
            Ok(selected) => selected,
            Err(original) => {
                let _exit = owner_checkpoint(host, grant, owner);
                return Err(original);
            }
        };
        let mut current = CurrentPgRoleOwner {
            host,
            grant,
            owner,
            selected,
        };
        let body = run(&mut current);
        // Covers early checkpoint failure as well as callback failure. Both
        // physical owner rereads are attempted by checkpoint, with first error
        // preserved. The surrounding real host adds its paid epoch exit too.
        let exited = current.check_current();
        match (body, exited) {
            (Err(original), _) | (Ok(_), Err(original)) => Err(original),
            (Ok(value), Ok(_)) => Ok(value),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_command as cmd;
    use crate::source_native_cli::private_assessment::OperationBudget;
    use std::{
        sync::atomic::AtomicBool,
        time::{Duration, Instant},
    };

    #[test]
    fn current_role_account_predicate_rejects_cross_account_and_context() {
        let deadline = Instant::now() + Duration::from_secs(30);
        let cancel = AtomicBool::new(false);
        let invocation = serde_json::json!({"budgets":{
            "worker_cpu_seconds":1,"worker_address_space_bytes":1048576
        }});
        // These partial JSON operands exercise only a private predicate, not
        // a grant/role/owner factory or a positive native issuance fixture.
        let config = cmd::parse(br#"{"uid":17,"source_context_ref":"/actual/context"}"#).unwrap();
        let mut original = OperationBudget::new(&invocation, deadline).unwrap();
        let mut access = ScopeAccess::admitted_operation(&mut original, deadline, &cancel);
        assert!(account_matches(&mut access, &config, 17, "/actual/context").is_ok());
        assert!(matches!(
            account_matches(&mut access, &config, 18, "/actual/context"),
            Err(Error::Denied("PG role current owner account differs"))
        ));
        assert!(matches!(
            account_matches(&mut access, &config, 17, "/different/context"),
            Err(Error::Denied("PG role current owner context differs"))
        ));
    }

    #[test]
    fn presence_loan_keeps_foreign_first_error_after_later_resource_refusal() {
        let original = DurableError::Conflict("first actual source failure");
        let later = Error::Unsupported("original Assessment payer resource refusal persists");
        assert!(matches!(
            complete_presence_loan(Err(later), Some(original)),
            Err(DurableError::Conflict("first actual source failure"))
        ));
        assert!(matches!(
            complete_presence_loan(Err(Error::Denied("first resource failure")), None),
            Err(DurableError::Source(Error::Denied(
                "first resource failure"
            )))
        ));
        assert!(matches!(
            complete_presence_loan(Ok(None), None),
            Err(DurableError::Invalid(
                "PG presence error relay inconsistent"
            ))
        ));
        assert!(matches!(
            complete_presence_loan(Ok(Some(false)), None),
            Ok(false)
        ));
    }
    #[test]
    fn role_permission_uses_only_current_public_source_selection() {
        let deadline = Instant::now() + Duration::from_secs(30);
        let cancel = AtomicBool::new(false);
        let mut operation = OperationBudget::new(
            &serde_json::json!({"budgets":{
                "worker_cpu_seconds":1,"worker_address_space_bytes":1048576
            }}),
            deadline,
        )
        .unwrap();
        let mut access = ScopeAccess::admitted_operation(&mut operation, deadline, &cancel);
        let path = "ToS/source-witnesses/contexts/fixture/context.json";
        let row = serde_json::json!({"path":path,"record_id":"tos.context.fixture","origin_id":"owner:fixture"});
        // PRIVATE selector operands only. These cannot construct the real
        // protected grant, CurrentPgRoleOwner, PG views or semantic issuer.
        let selected = cmd::parse(
            &serde_json::to_vec(&serde_json::json!({"source_records":[row.clone()]})).unwrap(),
        )
        .unwrap();
        assert!(public_selection(&mut access, &selected, "tos.context.fixture", path).is_ok());
        assert!(matches!(
            public_selection(&mut access, &selected, "tos.context.other", path),
            Err(Error::Denied(
                "PG role absent from current Assessment selection"
            ))
        ));
        let historical = cmd::parse(
            &serde_json::to_vec(&serde_json::json!({
                "source_records":[],"allowed_ids":["tos.context.fixture"],"records":[row.clone()]
            }))
            .unwrap(),
        )
        .unwrap();
        assert!(matches!(
            public_selection(&mut access, &historical, "tos.context.fixture", path),
            Err(Error::Denied(
                "PG role absent from current Assessment selection"
            ))
        ));
        let duplicate = cmd::parse(
            &serde_json::to_vec(&serde_json::json!({"source_records":[row.clone(),row]})).unwrap(),
        )
        .unwrap();
        assert!(matches!(
            public_selection(&mut access, &duplicate, "tos.context.fixture", path),
            Err(Error::Denied("PG role current source identity ambiguous"))
        ));
        assert!(matches!(
            public_selection(
                &mut access,
                &selected,
                "tos.context.fixture",
                "ToS/source-witnesses/contexts/other/context.json"
            ),
            Err(Error::Denied("PG role current source selection differs"))
        ));
        assert!(matches!(
            public_selection(
                &mut access,
                &selected,
                "tos.context.fixture",
                "ToS/source-witnesses/owner-local/selected/context.json"
            ),
            Err(Error::Denied(
                "PG role requires public selected metadata home"
            ))
        ));
    }
}
