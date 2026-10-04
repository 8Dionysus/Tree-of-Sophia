//! Same-owner logical payload delivery for an explicitly verified selected ABI.
//! Query owns authentic SQL-row binding and keeps borrowed physical/carrier
//! bytes admitted until this synchronous delivery and final selection fences end.
use crate::d1_public_capture::{CreationState, CreationStateHold};
use crate::sqlite_budget::SharedVmWindow;
use crate::{DedicatedSessionSqliteHeap, Error, Result, VerifiedKnowledgeModel};
use rusqlite::Connection;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64},
    },
    time::Instant,
};
use tos_foundation::{Digest256, Digest256Hasher, JsonLimits};

/// Compiler-internal loan of authenticated owner counters/control.
/// No public constructor or ingress grants original-session provenance.
/// The actual controlled owner must supply its retained admission/identity.
/// The remainder callback includes every retained caller/model/SQL/input owner.
/// Serial caller debits usage.json_visits on both success and failure and poisons
/// the session on failure. Work/VM use the original atomics directly.
pub(crate) struct RuntimeKnowledgeOwnedBudget<'a> {
    pub(crate) remaining_after_retained: &'a dyn Fn(usize) -> Result<usize>,
    pub(crate) original_work: &'a Arc<AtomicU64>,
    pub(crate) original_work_limit: u64,
    pub(crate) original_sql_vm: &'a Arc<AtomicU64>,
    pub(crate) original_sql_vm_limit: u64,
    pub(crate) original_sqlite_heap: &'a Arc<DedicatedSessionSqliteHeap>,
    pub(crate) remaining_json_visits: usize,
    pub(crate) owner_deadline: Instant,
    pub(crate) operation_deadline: Instant,
    pub(crate) cancelled: &'a Arc<AtomicBool>,
}
#[derive(Default, Clone, Copy)]
pub(crate) struct RuntimeKnowledgeReadUsage {
    pub(crate) json_visits: usize,
}

/// These are borrowed fields of the actual selected SQL row, not a source or
/// model authority. CarrierOnce row layout is accepted only under independently
/// verified model ABI; Query must retain SQL row/statement/connection custody.
pub(crate) struct SelectedPayloadRow<'a> {
    pub(crate) payload_codec: u8,
    pub(crate) physical: &'a [u8],
    pub(crate) logical_len: usize,
    pub(crate) logical_sha256: Digest256,
    pub(crate) source_packet_sha256: Option<Digest256>,
    pub(crate) source_packet: Option<&'a [u8]>,
}

/// A callback sees the same live codec context. Its prospective allocations
/// include all codec owners still held; it may not use the bare outer remainder
/// to omit them. Returned owners require persistent admission by the caller.
pub(crate) struct RuntimeKnowledgeReadContext<'state, 'budget> {
    state: &'state CreationState<'budget>,
    owner_deadline: Instant,
}
impl<'state, 'budget> RuntimeKnowledgeReadContext<'state, 'budget> {
    pub(crate) fn owned_state(&self) -> &'state CreationState<'budget> {
        self.state
    }
    /// Reserve this once in the persistent connection/model owner before any
    /// hook installation. Replacement overlap is separately held below.
    pub(crate) fn sql_callback_retained_state_bytes() -> usize {
        SharedVmWindow::callback_state_upper_bound()
    }
    /// Caller already retains one callback slot for this authentic connection.
    /// The explicit operation ceiling may only narrow the original absolute VM
    /// ceiling; the phase owner derives it from its admitted remaining budget.
    /// No connection is opened here and no counter is initialized or reset.
    pub(crate) fn install_operation_sql_controller<'connection>(
        &self,
        db: &'connection Connection,
        absolute_operation_vm_limit: u64,
    ) -> Result<RuntimeKnowledgeSqlHook<'connection, 'state, 'budget>> {
        if absolute_operation_vm_limit == 0
            || absolute_operation_vm_limit > self.state.sql_vm_limit()
        {
            return Err(Error::Budget("runtime SQL operation ceiling"));
        }
        let fixed = std::mem::size_of::<(
            &Self,
            &Connection,
            u64,
            SharedVmWindow,
            Result<SharedVmWindow>,
            Arc<AtomicU64>,
            Arc<AtomicBool>,
            Result<RuntimeKnowledgeSqlHook<'connection, 'state, 'budget>>,
        )>()
        .checked_add(SharedVmWindow::callback_state_upper_bound())
        .ok_or(Error::Budget("runtime SQL controller state"))?;
        let hold = self.state.hold(fixed)?;
        self.state.active()?;
        self.state.heap().verify_current()?;
        let window =
            SharedVmWindow::reserve(self.state.sql_vm_counter(), absolute_operation_vm_limit)?;
        window.install(
            db,
            self.state.operation_deadline(),
            self.state.cancellation_handle(),
        );
        self.state.active()?;
        Ok(RuntimeKnowledgeSqlHook {
            db,
            state: self.state,
            owner_deadline: self.owner_deadline,
            _replacement_hold: hold,
        })
    }
    pub(crate) fn check(&self) -> Result<()> {
        self.state.active()
    }
    pub(crate) fn charge_work(&self, bytes: usize) -> Result<()> {
        self.state.charge_work(bytes)
    }
    pub(crate) fn remaining_after_retained(&self, additional: usize) -> Result<usize> {
        self.state.remaining(additional)
    }
    /// Same aggregate visit ledger as codec parsing; no additional allowance.
    pub(crate) fn remaining_json_visits(&self) -> Result<usize> {
        self.state.remaining_json_visits()
    }
    /// Debit admission even on failure; callers must not refund failed work.
    pub(crate) fn debit_json_visits(&self, used: usize) -> Result<()> {
        self.state.debit_json_visits(used)
    }
    /// Parse within the existing state/check/visit owner and release its tree
    /// after the callback. Returned owners require caller persistent admission.
    pub(crate) fn with_foundation_owned_with_limits<T>(
        &self,
        raw: &[u8],
        limits: JsonLimits,
        operation: impl FnOnce(&tos_foundation::JsonValue) -> Result<T>,
    ) -> Result<T> {
        let fixed = std::mem::size_of::<(&Self, &[u8], JsonLimits)>()
            .checked_add(std::mem::size_of_val(&operation))
            .and_then(|n| n.checked_add(std::mem::size_of::<Result<T>>()))
            .ok_or(Error::Budget("selected logical parser controller state"))?;
        let _fixed = self.state.hold(fixed)?;
        self.state
            .with_foundation_owned_with_limits(raw, limits, operation)
    }
    pub(crate) fn with_reserved_state<T>(
        &self,
        bytes: usize,
        operation: impl FnOnce() -> Result<T>,
    ) -> Result<T> {
        let fixed = std::mem::size_of::<(&Self, usize)>()
            .checked_add(std::mem::size_of_val(&operation))
            .and_then(|n| n.checked_add(std::mem::size_of::<Result<T>>()))
            .ok_or(Error::Budget("selected reserved-state controller"))?;
        let total = bytes
            .checked_add(fixed)
            .ok_or(Error::Budget("selected reserved-state overflow"))?;
        let hold = self.state.hold(total)?;
        let result = operation();
        drop(hold);
        result
    }
}
/// Scoped replacement overlap for one already-admitted connection callback.
/// Dropping on failure retains charged VM windows; the caller must terminate.
/// The persistent callback slot stays in the outer model/connection census.
pub(crate) struct RuntimeKnowledgeSqlHook<'connection, 'state, 'budget> {
    db: &'connection Connection,
    state: &'state CreationState<'budget>,
    owner_deadline: Instant,
    _replacement_hold: CreationStateHold<'state, 'budget>,
}
impl RuntimeKnowledgeSqlHook<'_, '_, '_> {
    /// Only after all narrowed cold/model/source guards have succeeded may the
    /// retained connection resume its immutable original session lifetime.
    /// Same counter, same original ceiling; unused windows stay charged.
    pub(crate) fn promote_to_owner_after_fences(self) -> Result<()> {
        let fixed = std::mem::size_of::<(
            &Connection,
            SharedVmWindow,
            Result<SharedVmWindow>,
            Arc<AtomicU64>,
            Arc<AtomicBool>,
            Result<()>,
        )>()
        .checked_add(SharedVmWindow::callback_state_upper_bound())
        .ok_or(Error::Budget("runtime SQL promotion state"))?;
        let _fixed = self.state.hold(fixed)?;
        self.state.active()?;
        self.state.heap().verify_current()?;
        SharedVmWindow::reserve(self.state.sql_vm_counter(), self.state.sql_vm_limit())?.install(
            self.db,
            self.owner_deadline,
            self.state.cancellation_handle(),
        );
        self.state.active()
    }
}

/// A synchronous admission context for genuine cold preverification. This
/// factory supplies no model/source authority: the caller retains authentic
/// input/SQL/custody owners and validates the expected ABI before disclosure.
/// Usage is charged on both outcomes; any failure terminates the owner session.
pub(crate) fn with_runtime_knowledge_read_context<'budget, T>(
    budget: &'budget RuntimeKnowledgeOwnedBudget<'budget>,
    usage: &mut RuntimeKnowledgeReadUsage,
    consume: impl FnOnce(&RuntimeKnowledgeReadContext<'_, '_>) -> Result<T>,
) -> Result<T> {
    if usage.json_visits != 0 {
        return Err(Error::Invalid("runtime knowledge usage must begin empty"));
    }
    let state = CreationState::from_runtime_owned_budget(budget)?;
    let outcome = (|| {
        let fixed = std::mem::size_of::<(
            &RuntimeKnowledgeOwnedBudget<'_>,
            &mut RuntimeKnowledgeReadUsage,
            RuntimeKnowledgeReadContext<'_, '_>,
            Result<T>,
            Result<T>,
        )>()
        .checked_add(std::mem::size_of_val(&consume))
        .ok_or(Error::Budget("runtime knowledge controller state"))?;
        let _fixed = state.hold(fixed)?;
        let context = RuntimeKnowledgeReadContext {
            state: &state,
            owner_deadline: budget.owner_deadline,
        };
        let result = consume(&context);
        state.active()?;
        result
    })();
    usage.json_visits = state.json_visits();
    outcome
}

fn charged_digest(state: &CreationState<'_>, bytes: &[u8]) -> Result<Digest256> {
    let mut hash = Digest256Hasher::new();
    state.active()?;
    for part in bytes.chunks(4096) {
        state.charge_work(part.len())?;
        hash.update(part);
    }
    state.active()?;
    Ok(hash.finalize())
}

/// Compiler-only cold/read delivery after the caller has authenticated the
/// explicit expected ABI and retained the real SQL row. This supplies no model
/// authority; cold callers still perform complete schema/root/source fences.
pub(crate) fn with_logical_payload_for_verified_layout<T>(
    context: &RuntimeKnowledgeReadContext<'_, '_>,
    layout: crate::knowledge_stage::KnowledgePayloadLayout,
    row: &SelectedPayloadRow<'_>,
    stored_limits: JsonLimits,
    source_limits: JsonLimits,
    max_row_bytes: usize,
    consume: impl FnOnce(&[u8], &RuntimeKnowledgeReadContext<'_, '_>) -> Result<T>,
) -> Result<T> {
    let state = context.state;
    let fixed = std::mem::size_of::<(
        &RuntimeKnowledgeReadContext<'_, '_>,
        crate::knowledge_stage::KnowledgePayloadLayout,
        &SelectedPayloadRow<'_>,
        JsonLimits,
        JsonLimits,
        usize,
        &CreationState<'_>,
        Digest256Hasher,
        Digest256,
        std::slice::Chunks<'_, u8>,
        Option<&[u8]>,
        Option<Digest256>,
        &[u8],
        Digest256,
        bool,
        Result<T>,
        Result<T>,
    )>()
    .checked_add(std::mem::size_of_val(&consume))
    .ok_or(Error::Budget("logical payload controller state"))?;
    let _fixed = state.hold(fixed)?;
    state.active()?;
    let carrier = layout == crate::knowledge_stage::KnowledgePayloadLayout::CarrierOnceV1;
    if max_row_bytes == 0
        || row.logical_len == 0
        || row.logical_len > max_row_bytes
        || row.physical.is_empty()
        || row.physical.len() > max_row_bytes
    {
        return Err(Error::Budget("selected logical payload bytes"));
    }
    let result = match row.payload_codec {
        0 => {
            if row.source_packet_sha256.is_some()
                || row.source_packet.is_some()
                || row.physical.len() != row.logical_len
                || charged_digest(state, row.physical)? != row.logical_sha256
            {
                return Err(Error::Invalid("selected inline logical payload differs"));
            }
            consume(row.physical, context)
        }
        1 if carrier => {
            let source = row
                .source_packet
                .ok_or(Error::Invalid("selected source carrier absent"))?;
            let digest = row
                .source_packet_sha256
                .ok_or(Error::Invalid("selected carrier digest absent"))?;
            if source.is_empty()
                || source.len() > max_row_bytes
                || charged_digest(state, source)? != digest
            {
                return Err(Error::Invalid("selected source carrier differs"));
            }
            crate::knowledge_payload_codec::with_hydrated_payload(
                state,
                row.physical,
                source,
                stored_limits,
                source_limits,
                max_row_bytes,
                row.logical_len,
                row.logical_sha256,
                |logical| consume(logical, context),
            )
        }
        _ => Err(Error::Invalid(
            "selected payload codec incompatible with ABI",
        )),
    };
    state.active()?;
    result
}

/// Authenticate exact logical bytes before any parser/output callback. This
/// wrapper does not query a model path, reset SQL hooks or select a layout from
/// table presence. Cold/Query callers supply rows from their held actual DB.
/// Caller must debit reported visits even on Err and terminate, without retry.
pub(crate) fn with_selected_logical_payload_owned<'budget, T>(
    model: &VerifiedKnowledgeModel<'_>,
    row: &SelectedPayloadRow<'_>,
    budget: &'budget RuntimeKnowledgeOwnedBudget<'budget>,
    usage: &mut RuntimeKnowledgeReadUsage,
    stored_limits: JsonLimits,
    source_limits: JsonLimits,
    max_row_bytes: usize,
    consume: impl FnOnce(&[u8], &RuntimeKnowledgeReadContext<'_, '_>) -> Result<T>,
) -> Result<T> {
    with_runtime_knowledge_read_context(budget, usage, |context| {
        let state = context.state;
        let fixed = std::mem::size_of::<(
            &VerifiedKnowledgeModel<'_>,
            &SelectedPayloadRow<'_>,
            &RuntimeKnowledgeOwnedBudget<'_>,
            &mut RuntimeKnowledgeReadUsage,
            JsonLimits,
            JsonLimits,
            usize,
        )>()
        .checked_add(std::mem::size_of::<RuntimeKnowledgeReadContext<'_, '_>>())
        .and_then(|n| n.checked_add(std::mem::size_of_val(&consume)))
        .and_then(|n| n.checked_add(std::mem::size_of::<Digest256Hasher>()))
        .and_then(|n| n.checked_add(std::mem::size_of::<Result<T>>()))
        .and_then(|n| n.checked_add(std::mem::size_of::<Result<T>>()))
        .ok_or(Error::Budget("selected payload controller state"))?;
        let _fixed = state.hold(fixed)?;
        state.active()?;
        model.check_pin()?;
        let abi = &model.selection().model_abi;
        let layout = if abi == crate::knowledge_stage::KNOWLEDGE_CARRIER_ONCE_MODEL_ABI {
            crate::knowledge_stage::KnowledgePayloadLayout::CarrierOnceV1
        } else if abi == crate::knowledge_selected::KNOWLEDGE_MODEL_ABI {
            crate::knowledge_stage::KnowledgePayloadLayout::InlineV1
        } else {
            return Err(Error::Invalid("selected payload ABI unsupported"));
        };
        let result = with_logical_payload_for_verified_layout(
            context,
            layout,
            row,
            stored_limits,
            source_limits,
            max_row_bytes,
            consume,
        );
        // Scope buffers/context remain retained through both disclosure fences.
        state.active()?;
        model.check_pin()?;
        state.active()?;
        result
    })
}
