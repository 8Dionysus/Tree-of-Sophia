//! Native metadata worker adapter over the CONCRETE protected PG role owner.
//! No arbitrary PgRoleSchemaAccess implementation, caller identity or schema
//! pins can enter its private constructor. This adapter is not a role issuer.
use super::{CurrentPgRoleOwner, PgRoleSchemaAccess};
use crate::source_cohort::revision::PgRecordRevisionIdentity;
use crate::source_command::{SourceCommandError, SourceCommandResult};
use crate::source_revisions::{self, RecordRead, RecordSchemaWorker};
use crate::{DurableError, DurableResult};
use std::{cell::RefCell, mem::{size_of, size_of_val}, sync::atomic::AtomicBool, time::Instant};
use tos_foundation::{CutPreparedSchemaExecutionBinding, Digest256, FormatProfile, JsonValue};
use tos_validation::{item_rules::ItemRefusal, source_cut::CutSchemaExecutor};

/// One private mutable loan, lent sequentially by native getter/check calls.
/// RefCell handles the legacy immutable trait getters without exposing a
/// second host alias. Owned failures remain until this adapter is consumed.
pub(crate) struct PgRoleKernelSchema<'state, 'loan, 'call, 'owner, 'view, 'store, 'budget, 'cancel, 'pure> {
    current: &'state RefCell<&'loan mut CurrentPgRoleOwner<'call, 'owner, 'view, 'store, 'budget, 'cancel, 'pure>>,
    selected: PgRecordRevisionIdentity,
    profile: FormatProfile,
    binding: CutPreparedSchemaExecutionBinding,
    deadline: Instant,
    cancelled: &'cancel AtomicBool,
    first: &'state RefCell<Option<DurableError>>,
}

/// Shared *sequential* control over the one genuine owner loan. The native
/// RecordRead and schema worker can coexist without copied source buffers or
/// a second mutable host alias. The factory confines both cells lexically.
#[derive(Clone, Copy)]
pub(super) struct PgRoleControl<'state, 'loan, 'call, 'owner, 'view, 'store, 'budget, 'cancel, 'pure> {
    current: &'state RefCell<&'loan mut CurrentPgRoleOwner<'call, 'owner, 'view, 'store, 'budget, 'cancel, 'pure>>,
    first: &'state RefCell<Option<DurableError>>,
}
impl<'loan, 'call, 'owner, 'view, 'store, 'budget, 'cancel, 'pure>
    PgRoleControl<'_, 'loan, 'call, 'owner, 'view, 'store, 'budget, 'cancel, 'pure> {
    pub(super) fn with_owner<T>(&self, run: impl FnOnce(
        &mut CurrentPgRoleOwner<'call, 'owner, 'view, 'store, 'budget, 'cancel, 'pure>,
    ) -> DurableResult<T>) -> SourceCommandResult<T> {
        if self.first.borrow().is_some() {
            return Err(SourceCommandError::Unsupported("PG native role original owner failure retained"));
        }
        let body = match self.current.try_borrow_mut() {
            Ok(mut owner) => run(&mut owner),
            Err(_) => Err(DurableError::Conflict("PG native role owner already borrowed")),
        };
        match body {
            Ok(value) => Ok(value),
            Err(original) => {
                let mut first = self.first.borrow_mut();
                if first.is_none() { *first = Some(original); }
                Err(SourceCommandError::Unsupported("PG native role original owner failure retained"))
            }
        }
    }
    pub(super) fn with_operation<T>(&self, run: impl FnOnce(
        &mut crate::source_native_cli::private_assessment::OperationBudget,
    ) -> SourceCommandResult<T>) -> SourceCommandResult<T> {
        self.with_owner(|owner| owner.with_source_epoch(|_epoch, payer, _same| {
            payer.with_operation(run).map_err(DurableError::Source)
        }))
    }
}
impl<'state, 'loan, 'call, 'owner, 'view, 'store, 'budget, 'cancel, 'pure>
    PgRoleKernelSchema<'state, 'loan, 'call, 'owner, 'view, 'store, 'budget, 'cancel, 'pure> {
    pub(super) fn control(&self) -> PgRoleControl<'state, 'loan, 'call, 'owner, 'view, 'store, 'budget, 'cancel, 'pure> {
        PgRoleControl { current: self.current, first: self.first }
    }
}

impl PgRoleKernelSchema<'_, '_, '_, '_, '_, '_, '_, '_, '_> {
    fn refused(&self, original: DurableError) {
        let mut first = self.first.borrow_mut();
        if first.is_none() { *first = Some(original); }
    }
    fn ready(&self) -> bool {
        if self.first.borrow().is_some() { return false; }
        if let Err(original) = crate::source_creation_store::active(self.deadline, self.cancelled) {
            self.refused(DurableError::Source(original));
            return false;
        }
        true
    }
    fn contract(&self, path: &str) -> Option<Digest256> {
        if !self.ready() { return None; }
        let result = match self.current.try_borrow_mut() {
            Ok(mut current) => current.selected_schema_contract_digest(path),
            Err(_) => Err(DurableError::Conflict("PG native schema owner already borrowed")),
        };
        match result {
            Ok(value) => value,
            Err(original) => { self.refused(original); None }
        }
    }
}

impl CutSchemaExecutor for PgRoleKernelSchema<'_, '_, '_, '_, '_, '_, '_, '_, '_> {
    fn schema_profile(&self) -> Option<FormatProfile> {
        self.ready().then_some(self.profile)
    }
    fn prepared_execution_binding(&self) -> Option<CutPreparedSchemaExecutionBinding> {
        self.ready().then_some(self.binding)
    }
    fn selected_contract_digest(&self, path: &str) -> Option<Digest256> {
        self.contract(path)
    }
    fn check(
        &mut self, source_path: &str, raw: &[u8], schema_path: &str,
        deadline: Instant, cancelled: &AtomicBool,
    ) -> Result<bool, ItemRefusal> {
        if !self.ready() { return Err(ItemRefusal::Budget); }
        if deadline != self.deadline || !std::ptr::eq(cancelled, self.cancelled) {
            self.refused(DurableError::Conflict("PG native schema cutoff differs from original owner"));
            return Err(ItemRefusal::Budget);
        }
        let result = match self.current.try_borrow_mut() {
            Ok(mut current) => current.check_record_bytes(source_path, raw, schema_path),
            Err(_) => Err(DurableError::Conflict("PG native schema owner already borrowed")),
        };
        match result {
            Ok(value) => Ok(value),
            Err(original) => {
                self.refused(original);
                // Allocation-free INTERNAL transport only. finish below
                // ALWAYS restores the actual owned error, even if the kernel
                // handles this sentinel and tries to return Ok. False is not
                // a refusal and is returned unchanged above.
                Err(ItemRefusal::Budget)
            }
        }
    }
    // Budget/quota setters, batches and finish retain their refusing defaults.
    // Only the real enclosing native owner finalizes its retained worker.
}

impl RecordSchemaWorker for PgRoleKernelSchema<'_, '_, '_, '_, '_, '_, '_, '_, '_> {
    type Identity = PgRecordRevisionIdentity;
    fn input_identity(&self) -> Self::Identity { self.selected }
    fn contract_digest(&self, path: &str) -> Option<Digest256> { self.contract(path) }
    fn resource_closure_bound_to_identity(&self) -> bool {
        // Concrete constructor below borrows ONLY CurrentPgRoleOwner's actual
        // private host. PWE bound ALL prepared resources to the same genuine
        // loan metadata at host entry; no generic provider can assert this.
        true
    }
    fn public_profile(
        &mut self,
        cut: Option<&tos_source_store::CorpusCutReader>,
        deadline: Instant,
        cancelled: &AtomicBool,
        ctx: &impl RecordRead<Identity = Self::Identity>,
        config: &JsonValue,
        record: &JsonValue,
    ) -> SourceCommandResult<(JsonValue, Vec<String>, Option<String>, Option<String>)> {
        if cut.is_some() || ctx.writer().is_some() {
            return Err(SourceCommandError::Denied("PG native role schema cannot use writer/corpus authority"));
        }
        source_revisions::public_profile_selected_readonly(self, deadline, cancelled, ctx, config, record)
    }
}

fn complete_kernel<T>(
    body: SourceCommandResult<T>, first: Option<DurableError>, exit: DurableResult<()>,
) -> DurableResult<T> {
    let body = match first {
        Some(original) => Err(original),
        None => body.map_err(DurableError::Source),
    };
    match (body, exit) {
        (Err(original), _) | (Ok(_), Err(original)) => Err(original),
        (Ok(value), Ok(())) => Ok(value),
    }
}

/// Only this concrete owner can construct the adapter. Bootstrap ends its
/// source/Session loan BEFORE snapshot/getter/check calls use the same private
/// host. No reentrant mutable host borrow inside an epoch callback is needed.
pub(crate) fn with_pg_role_kernel_schema<'loan, 'call, 'owner, 'view, 'store, 'budget, 'cancel, 'pure, T>(
    current: &'loan mut CurrentPgRoleOwner<'call, 'owner, 'view, 'store, 'budget, 'cancel, 'pure>,
    deadline: Instant,
    cancelled: &'cancel AtomicBool,
    run: impl for<'state> FnOnce(&mut PgRoleKernelSchema<'state, 'loan, 'call, 'owner, 'view, 'store, 'budget, 'cancel, 'pure>) -> SourceCommandResult<T>,
) -> DurableResult<T> {
    current.with_source_epoch(|_epoch, payer, _same| {
        let (actual_deadline, actual_cancelled) = payer.source_cutoff();
        if deadline != actual_deadline || !std::ptr::eq(cancelled, actual_cancelled) {
            return Err(DurableError::Conflict("PG native kernel cutoff differs from original owner"));
        }
        let bytes = size_of::<PgRoleKernelSchema<'_, '_, '_, '_, '_, '_, '_, '_, '_>>()
            .checked_add(size_of::<SourceCommandResult<T>>())
            .and_then(|n| n.checked_add(size_of::<DurableResult<T>>()))
            .and_then(|n| n.checked_add(size_of_val(&run)))
            .and_then(|n| n.checked_add(size_of::<RefCell<&mut CurrentPgRoleOwner<'_, '_, '_, '_, '_, '_, '_>>>()))
            .and_then(|n| n.checked_add(size_of::<RefCell<Option<DurableError>>>()))
            .ok_or(DurableError::Refused("PG native kernel carrier overflow"))?;
        payer.reserve(bytes).map_err(DurableError::Source)?;
        payer.inspect(0, size_of::<PgRecordRevisionIdentity>()).map_err(DurableError::Source)
    })?;
    let selected = *current.input_identity();
    let (profile, binding) = current.schema_validation_binding()?;
    if profile != FormatProfile::AssertedSourceCandidateV1 || profile != binding.schema_profile {
        return Err(DurableError::Refused("PG native role requires actual asserted candidate schema profile"));
    }
    let current = RefCell::new(current);
    let first = RefCell::new(None);
    let mut worker = PgRoleKernelSchema {
        current: &current, selected, profile, binding, deadline, cancelled,
        first: &first,
    };
    let body = run(&mut worker);
    let exit = match worker.current.try_borrow_mut() {
        Ok(mut current) => current.check_current(),
        Err(_) => Err(DurableError::Conflict("PG native kernel exit owner still borrowed")),
    };
    complete_kernel(body, first.into_inner(), exit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kernel_relay_rejects_swallowed_foreign_error_and_preserves_body_priority() {
        assert!(matches!(complete_kernel(Ok(17), Some(DurableError::Conflict("first worker fault")), Ok(())),
            Err(DurableError::Conflict("first worker fault"))));
        let body: SourceCommandResult<()> = Err(SourceCommandError::Denied("native body fault"));
        assert!(matches!(complete_kernel(body, None, Err(DurableError::Conflict("later exit fault"))),
            Err(DurableError::Source(SourceCommandError::Denied("native body fault")))));
        assert!(matches!(complete_kernel(Ok(false), None, Ok(())), Ok(false)));
        assert!(matches!(complete_kernel(Ok(true), None, Err(DurableError::Conflict("failed real exit"))),
            Err(DurableError::Conflict("failed real exit"))));
    }
}
