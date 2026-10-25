//! Live protected owner envelope for the PG C4 reader. This is neither a
//! formal-role result nor a grant to use a role/account. It supplies no source
//! bytes, private package permission, schema verdict or qualified assessment.
//! The native reader must still prove exact owner/history/profile/role, and
//! PWE must still check the actual report and application admission.
use crate::pwe_source_owner::PgPayloadHost;
use crate::source_assessment_journal::ProtectedAssessmentPgGrant;
use crate::source_cohort::revision::{PgRecordRevisionIdentity, SourceRevisionReadSession};
use crate::source_command::{SourceCommandError as Error, SourceCommandResult};
use crate::source_native_cli::private_assessment::scope_admission::ScopeAccess;
use crate::source_native_cli::private_assessment::original_payer::OriginalAssessmentPayer;
use crate::source_text_owner::OwnerTextContext;
use crate::{DurableError, DurableResult};
use std::mem::{size_of, size_of_val};
use tos_foundation::JsonValue;
use tos_compiler::pwe_native::Session;

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
        let grant_check = payer.recheck_current_pg_grant_source(same, grant);
        let owner_check = payer.recheck_owner_context_source(same, owner);
        let (deadline, cancelled) = payer.source_cutoff();
        let binding = payer.with_operation(|budget| {
            let mut access = ScopeAccess::admitted_operation(budget, deadline, cancelled);
            // Price UTF-8 inspection before converting the actual retained
            // owner path. Strings from config are compared by admitted access.
            access.inspect(
                owner.configuration_path().as_os_str().as_encoded_bytes().len(), 1,
            )?;
            let context = owner.configuration_path().to_str()
                .ok_or(Error::Invalid("PG role owner context path UTF-8"))?;
            account_matches(&mut access, grant.configuration(), owner.account_uid(), context)
        });
        let identity = payer.with_operation(|budget| {
            epoch.selection().identity_with_data_admission(budget, deadline, cancelled)
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

impl<'store, 'budget, 'cancel, 'pure> CurrentPgRoleOwner<'_, '_, '_, 'store, 'budget, 'cancel, 'pure> {
    pub(crate) fn input_identity(&self) -> &PgRecordRevisionIdentity {
        &self.selected
    }

    /// A cached mechanical role must be used under this current owner envelope
    /// and the same actual PG selection. This grants no `can_use` admission.
    pub(crate) fn check_current(
        &mut self,
    ) -> DurableResult<()> {
        let selected = owner_checkpoint(self.host, self.grant, self.owner)?;
        // identity_with_data_admission prepays the fixed complete comparison.
        if selected != self.selected {
            return Err(DurableError::Conflict("PG role current owner epoch differs"));
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
        let mut current = CurrentPgRoleOwner { host, grant, owner, selected };
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
    use std::{sync::atomic::AtomicBool, time::{Duration, Instant}};

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
        assert!(matches!(account_matches(&mut access, &config, 18, "/actual/context"),
            Err(Error::Denied("PG role current owner account differs"))));
        assert!(matches!(account_matches(&mut access, &config, 17, "/different/context"),
            Err(Error::Denied("PG role current owner context differs"))));
    }
}
