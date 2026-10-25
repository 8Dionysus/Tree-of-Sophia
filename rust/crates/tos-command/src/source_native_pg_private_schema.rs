//! Sequential schema bridge for private source kernels, not a Claim/role issuer.
//! Only the concrete owner plus genuine retained views construct its reader.
//! The callback receives no PgPayloadHost or replaceable native worker. All
//! quota setters, batches and finish keep the refusing trait defaults.
use super::super::{CurrentPgRoleOwner, with_pg_role_kernel_schema};
use super::{SelectedRoleRead, overflow};
use crate::pwe_source_owner::{PgPayloadMember, PgPayloadViews};
use crate::source_cohort::revision::PgRecordRevisionIdentity;
use crate::source_command::{
    CommandContext, SourceCommandError as Error, SourceCommandResult as Result,
};
use crate::source_native_cli::private_owner::profile::{
    PrivateSelectedSchemaExecutor, PrivateSelectedSourceIdentity,
};
use crate::source_revisions::{self, RecordRead, RecordSchemaWorker};
use crate::{DurableError, DurableResult};
use std::{
    collections::BTreeSet,
    mem::{size_of, size_of_val},
    sync::atomic::AtomicBool,
    time::Instant,
};
use tos_foundation::{
    CutPreparedSchemaExecutionBinding, Digest256, FormatProfile, JsonValue, RelativePath,
};
use tos_validation::{item_rules::ItemRefusal, source_cut::CutSchemaExecutor};

// Generic implementation detail only: no public constructor and no generic
// issuer. Production construction below always uses the actual closed owner
// kernel and its concrete retained SelectedRoleRead. No copied files table.
struct PrivateSchema<'borrow, W, R> {
    worker: &'borrow mut W,
    reader: &'borrow R,
}
impl<W, R> CutSchemaExecutor for PrivateSchema<'_, W, R>
where
    W: RecordSchemaWorker<Identity = PgRecordRevisionIdentity>,
    R: RecordRead<Identity = PgRecordRevisionIdentity>,
{
    fn schema_profile(&self) -> Option<FormatProfile> {
        self.worker.schema_profile()
    }
    fn prepared_execution_binding(&self) -> Option<CutPreparedSchemaExecutionBinding> {
        self.worker.prepared_execution_binding()
    }
    fn selected_contract_digest(&self, path: &str) -> Option<Digest256> {
        self.worker.selected_contract_digest(path)
    }
    fn check(
        &mut self,
        path: &str,
        raw: &[u8],
        schema: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> std::result::Result<bool, ItemRefusal> {
        self.worker.check(path, raw, schema, deadline, cancelled)
    }
}
impl<W, R> PrivateSelectedSchemaExecutor for PrivateSchema<'_, W, R>
where
    W: RecordSchemaWorker<Identity = PgRecordRevisionIdentity>,
    R: RecordRead<Identity = PgRecordRevisionIdentity>,
{
    fn private_selection_identity(&self) -> PrivateSelectedSourceIdentity {
        PrivateSelectedSourceIdentity::PgRecord(RecordSchemaWorker::input_identity(self.worker))
    }
    fn validate_profile_registry(
        &mut self,
        ctx: &CommandContext,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<JsonValue> {
        // This is an actual selected registry check, never the bootstrap
        // CommandContext presented as a PG revision or a true-only schema.
        self.reader.check()?;
        if ctx.base_revision != self.reader.input_identity().bootstrap_revision
            || self.reader.input_identity() != RecordSchemaWorker::input_identity(self.worker)
        {
            return Err(Error::Conflict("private PG schema source identity differs"));
        }
        source_revisions::validate_source_profile_registry_for_worker(
            self.worker,
            deadline,
            cancelled,
            self.reader,
        )
    }
}

fn run_private<W, R, T>(
    worker: &mut W,
    reader: &R,
    run: impl FnOnce(&mut dyn PrivateSelectedSchemaExecutor) -> Result<T>,
) -> Result<T>
where
    W: RecordSchemaWorker<Identity = PgRecordRevisionIdentity>,
    R: RecordRead<Identity = PgRecordRevisionIdentity>,
{
    let mut private = PrivateSchema { worker, reader };
    run(&mut private)
}

fn context_identity(
    ctx: &CommandContext,
    bootstrap: tos_foundation::SourceRevision,
    uid: u64,
) -> Result<()> {
    if ctx.base_revision != bootstrap {
        return Err(Error::Conflict("private PG context bootstrap differs"));
    }
    if ctx.effective_uid != uid {
        return Err(Error::Denied(
            "private PG context account differs from actual owner",
        ));
    }
    Ok(())
}

fn admitted_context_is_tos(
    file: &crate::source_command::SourceFile,
    budget: &mut crate::source_native_cli::private_assessment::OperationBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<bool> {
    // Every candidate header and the at-most-four-byte prefix comparison,
    // including rejected non-ToS candidates, BEFORE reading/branching on it.
    // No prior context validation or retained-path quote pays this new walk.
    let bytes = size_of::<crate::source_command::SourceFile>()
        .checked_add(b"ToS/".len())
        .ok_or_else(overflow)?;
    crate::source_native_cli::private_assessment::scope_admission::ScopeAccess::admitted_operation(
        budget, deadline, cancelled,
    )
    .inspect(bytes, 1)?;
    Ok(file.path.as_str().starts_with("ToS/"))
}

/// Callable schema-only bridge for existing private Profile/Claim parsers.
/// It provides no writer Grant, source-access policy, private package state,
/// NativeAccount, PG basis or generic PWE-dispatch exception. Those remain the
/// caller's separate closed semantic gates. Missing retained inputs refuse.
///
/// Parser/canonical/native activation/driver fit is NOT established by the
/// fixed control/member quotes here; the existing kernels retain their costs.
#[allow(clippy::too_many_arguments)]
pub(crate) fn with_pg_private_schema_kernel<
    'payload,
    'owner,
    'view,
    'store,
    'budget,
    'cancel,
    'pure,
    T,
>(
    current: &mut CurrentPgRoleOwner<'_, 'owner, 'view, 'store, 'budget, 'cancel, 'pure>,
    views: &PgPayloadViews<'payload, 'budget, 'cancel>,
    paths: &BTreeSet<RelativePath>,
    ctx: &CommandContext,
    deadline: Instant,
    cancelled: &'cancel AtomicBool,
    run: impl FnOnce(&mut dyn PrivateSelectedSchemaExecutor) -> Result<T>,
) -> DurableResult<T> {
    let body = (|| {
        current.check_current()?;
        if paths.is_empty() || paths.len() > 64 {
            return Err(DurableError::Refused(
                "private PG kernel selected closure count",
            ));
        }
        // Actual typed fixed/member/result/callback carriers, before the Vec
        // or adapter is constructed. No raw source buffer is cloned here.
        let member_backing = paths
            .len()
            .checked_mul(size_of::<PgPayloadMember<'_>>())
            .ok_or(DurableError::Source(overflow()))?;
        let fixed = size_of::<SelectedRoleRead<'_, '_, '_, '_, '_, '_, '_, '_, '_, '_>>()
            .checked_add(2 * size_of::<usize>()) // two sized references in PrivateSchema
            .and_then(|n| n.checked_add(size_of::<Result<T>>()))
            .and_then(|n| n.checked_add(size_of::<DurableResult<T>>()))
            .and_then(|n| n.checked_add(size_of_val(&run)))
            .and_then(|n| n.checked_add(member_backing))
            .ok_or(DurableError::Source(overflow()))?;
        current.with_source_epoch(|_, payer, same| {
            payer
                .charge_sto_source_measures(same, 0, fixed, 0, 0, 1)
                .map_err(DurableError::Source)
        })?;
        let mut members = Vec::new();
        members
            .try_reserve_exact(paths.len())
            .map_err(|_| DurableError::Refused("private PG borrowed member backing"))?;
        for path in paths {
            members.push(current.selected_member(views, path)?);
        }
        let selected = *current.input_identity();
        let uid = current.account_uid();
        with_pg_role_kernel_schema(current, deadline, cancelled, |worker| {
            let reader = SelectedRoleRead {
                members,
                control: worker.control(),
                selected,
                uid,
                deadline,
                cancelled,
            };
            reader.check()?;
            reader.control.with_operation(|budget| {
                budget.charge_work(size_of::<u64>() + size_of::<tos_foundation::SourceRevision>())
            })?;
            context_identity(ctx, selected.bootstrap_revision, uid)?;
            // Legacy context bytes are only a compatibility operand. Every
            // authored member must match the REAL retained PG input before
            // passing that context to old private kernels. Never substitute
            // the ctx bootstrap revision for the opaque selected identity.
            for file in &ctx.files {
                let is_tos = reader.control.with_operation(|budget| {
                    admitted_context_is_tos(file, budget, deadline, cancelled)
                })?;
                if !is_tos {
                    continue;
                }
                reader.control.with_operation(|budget| {
                    budget.charge_work(file.raw.len().checked_add(1).ok_or_else(overflow)?)
                })?;
                if reader.file(&file.path)? != Some(file.raw.as_slice()) {
                    return Err(Error::Conflict(
                        "private PG context bytes differ from retained source",
                    ));
                }
            }
            run_private(worker, &reader, run)
        })
    })();
    let exit = current.check_current();
    match (body, exit) {
        (Err(original), _) | (Ok(_), Err(original)) => Err(original),
        (Ok(value), Ok(())) => Ok(value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tos_foundation::SourceRevision;

    #[test]
    fn private_pg_context_walk_pays_non_tos_headers_before_prefix_and_keeps_failed_prefix() {
        use crate::source_command::SourceFile;
        use crate::source_native_cli::private_assessment::OperationBudget;
        let deadline = Instant::now() + std::time::Duration::from_secs(30);
        let cancelled = AtomicBool::new(false);
        let mut budget = OperationBudget::new(
            &serde_json::json!({"budgets":{
                "worker_cpu_seconds":1,"worker_address_space_bytes":1048576
            }}),
            deadline,
        )
        .unwrap();
        let fee = size_of::<SourceFile>() + b"ToS/".len() + 1;
        budget.lower_remaining(1_048_576, 3 * fee - 1).unwrap();
        let skipped = SourceFile {
            path: RelativePath::parse("other/context.json").unwrap(),
            raw: Vec::new(),
        };
        let selected = SourceFile {
            path: RelativePath::parse("ToS/context.json").unwrap(),
            raw: Vec::new(),
        };
        assert!(!admitted_context_is_tos(&skipped, &mut budget, deadline, &cancelled).unwrap());
        assert_eq!(budget.test_work_remaining(), 2 * fee - 1);
        assert!(!admitted_context_is_tos(&skipped, &mut budget, deadline, &cancelled).unwrap());
        assert_eq!(budget.test_work_remaining(), fee - 1);
        // Even a ToS match cannot be returned when its BEFORE-prefix debit
        // fails. No actual owner/reader/worker is fabricated for this test.
        assert!(matches!(
            admitted_context_is_tos(&selected, &mut budget, deadline, &cancelled),
            Err(Error::Invalid("assessment cumulative work budget"))
        ));
        assert_eq!(budget.test_work_remaining(), fee - 1);
        assert!(budget.limits().is_err());
    }

    #[test]
    fn private_pg_kernel_context_rejects_foreign_bootstrap_and_account() {
        let bootstrap = SourceRevision(Digest256::of_bytes(b"fixture actual bootstrap"));
        let other = SourceRevision(Digest256::of_bytes(b"fixture other bootstrap"));
        // Ordinary compatibility data for the private predicate, never a real
        // CurrentPgRoleOwner or issuer. Factory separately checks live owner
        // plus every authored byte against its actual retained PG reader.
        let ctx = CommandContext {
            base_revision: bootstrap,
            configuration_raw: Vec::new(),
            request_raw: Vec::new(),
            recorded_at: String::new(),
            effective_uid: 17,
            files: Vec::new(),
        };
        assert!(context_identity(&ctx, bootstrap, 17).is_ok());
        assert!(matches!(
            context_identity(&ctx, other, 17),
            Err(Error::Conflict("private PG context bootstrap differs"))
        ));
        assert!(matches!(
            context_identity(&ctx, bootstrap, 18),
            Err(Error::Denied(
                "private PG context account differs from actual owner"
            ))
        ));
    }
}
