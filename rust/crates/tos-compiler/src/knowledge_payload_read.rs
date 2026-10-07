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
    /// Canonicalize a selected query value under the same original work,
    /// retained-state and JSON-visit owners as selected payload parsing.
    pub(crate) fn canonicalize_foundation_owned_with_limits(
        &self,
        value: &tos_foundation::JsonValue,
        profile: tos_foundation::CanonicalProfile,
        mut limits: JsonLimits,
    ) -> Result<Vec<u8>> {
        use tos_foundation::canonical_bytes_v1_with_state_budget_and_visits;
        self.check()?;
        limits.max_visits = limits.max_visits.min(self.remaining_json_visits()?);
        if limits.max_visits == 0 {
            return Err(Error::Budget("owned query canonical visits"));
        }
        let available = self.remaining_after_retained(0)?;
        let (bytes, visits) =
            canonical_bytes_v1_with_state_budget_and_visits(value, profile, limits, available)
                .map_err(|_| Error::Budget("owned query canonical state"))?;
        self.debit_json_visits(visits)?;
        self.charge_work(bytes.len())?;
        self.check()?;
        Ok(bytes)
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
/// Run one bounded typed-query SQL primitive against the same original VM
/// counter. The query ceiling is an absolute temporary endpoint derived from
/// the already-used counter; installing/restoring callbacks never resets or
/// refunds that counter. Only compiler-owned typed primitives call this
/// helper, so a caller never receives the connection handle.
pub(crate) fn with_query_vm_window<T>(
    context: &RuntimeKnowledgeReadContext<'_, '_>,
    db: &Connection,
    max_vm_steps: u64,
    operation: impl FnOnce() -> Result<T>,
) -> Result<(T, u64)> {
    use crate::sqlite_budget::SharedVmWindow;
    use std::sync::atomic::Ordering;

    if max_vm_steps == 0 {
        return Err(Error::Budget("controlled query VM window"));
    }
    let state = context.state;
    let fixed = std::mem::size_of::<(
        &RuntimeKnowledgeReadContext<'_, '_>,
        &Connection,
        u64,
        SharedVmWindow,
        Result<SharedVmWindow>,
        Arc<AtomicU64>,
        Arc<AtomicBool>,
        Result<T>,
        Result<()>,
    )>()
    .checked_add(std::mem::size_of_val(&operation))
    .and_then(|n| n.checked_add(2 * SharedVmWindow::callback_state_upper_bound()))
    .ok_or(Error::Budget("controlled query VM controller state"))?;
    let _hold = state.hold(fixed)?;
    context.check()?;
    let counter = state.sql_vm_counter();
    let original_cap = state.sql_vm_limit();
    let current = counter.load(Ordering::Acquire);
    // The installed callback reserves one instruction slot before the query
    // begins. That slot is charged to the caller's per-query cap.
    let query_cap = current
        .checked_add(max_vm_steps)
        .filter(|cap| *cap <= original_cap)
        .ok_or(Error::Budget("controlled query VM admission"))?;
    SharedVmWindow::reserve(Arc::clone(&counter), query_cap)?.install(
        db,
        state.operation_deadline(),
        state.cancellation_handle(),
    );
    let result = operation();
    let after = counter.load(Ordering::Acquire);
    let used = after.saturating_sub(current).saturating_sub(1); // exclude this query window's prepaid first slot
    let active = context.check();
    // Restore the original owner endpoint even when SQL refuses. If the owner
    // has exhausted its aggregate allowance, this replacement also fails
    // closed and the enclosing operation must terminate.
    let restore = SharedVmWindow::reserve(counter, original_cap).map(|window| {
        window.install(db, state.operation_deadline(), state.cancellation_handle());
    });
    match (active, result, restore) {
        (Err(error), _, _) => Err(error),
        (_, Err(error), _) => Err(error),
        (_, Ok(_), Err(error)) => Err(error),
        (Ok(()), Ok(value), Ok(())) => Ok((value, used)),
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

/// Borrow the authentic whole builder state for one explicitly clipped cold
/// read. This does not create, reset or return a second CreationState; JSON,
/// work, VM, SQLite heap, cancellation and retained-byte charges all remain on
/// the loan's original owner.
pub(crate) fn with_snapshot_owned_knowledge_read_context<'owner, 'budget>(
    loan: &crate::native_snapshot::NativeSnapshotOwnedReadLoan<'owner, 'budget>,
    operation_deadline: Instant,
    consume: impl FnOnce(&RuntimeKnowledgeReadContext<'owner, 'budget>) -> Result<()>,
) -> Result<()> {
    let state = loan.owned_state();
    if operation_deadline > loan.operation_deadline() || operation_deadline <= Instant::now() {
        return Err(Error::Budget("snapshot cold-read operation deadline"));
    }
    let fixed = std::mem::size_of::<(
        &crate::native_snapshot::NativeSnapshotOwnedReadLoan<'_, '_>,
        &CreationState<'_>,
        Instant,
        RuntimeKnowledgeReadContext<'_, '_>,
        CreationStateHold<'_, '_>,
        Result<()>,
        Result<()>,
    )>()
    .checked_add(std::mem::size_of_val(&consume))
    .ok_or(Error::Budget("snapshot cold-read context frame"))?;
    let _frame = state.hold(fixed)?;
    state.active()?;
    let context = RuntimeKnowledgeReadContext {
        state,
        owner_deadline: operation_deadline,
    };
    let outcome = consume(&context);
    let active = state.active();
    match (active, outcome) {
        (Err(error), _) => Err(error),
        (_, Err(error)) => Err(error),
        (Ok(()), Ok(())) => Ok(()),
    }
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
    let carrier = layout.uses_carriers();
    if max_row_bytes == 0 || row.logical_len == 0 || row.logical_len > max_row_bytes
        || row.physical.is_empty() || row.physical.len() > layout.physical_bound(max_row_bytes)? {
        return Err(Error::Budget("selected logical payload bytes"));
    }
    let result = match row.payload_codec {
        0 => {
            if row.source_packet_sha256.is_some() || row.source_packet.is_some() {
                return Err(Error::Invalid("selected inline source carrier"));
            }
            layout.with_decoded(state, row.physical, Some(row.logical_len), max_row_bytes, |raw| {
                if charged_digest(state, raw)? != row.logical_sha256 {
                    return Err(Error::Invalid("selected inline logical payload differs"));
                }
                consume(raw, context)
            })
        }
        1 if carrier => {
            let source = row.source_packet.ok_or(Error::Invalid("selected source carrier absent"))?;
            let digest = row.source_packet_sha256.ok_or(Error::Invalid("selected carrier digest absent"))?;
            layout.with_decoded(state, source, None, max_row_bytes, |source| {
                if charged_digest(state, source)? != digest {
                    return Err(Error::Invalid("selected source carrier differs"));
                }
                layout.with_decoded(state, row.physical, None, max_row_bytes, |stored| {
                    crate::knowledge_payload_codec::with_hydrated_payload(
                        state, stored, source, stored_limits, source_limits,
                        max_row_bytes, row.logical_len, row.logical_sha256,
                        |logical| consume(logical, context),
                    )
                })
            })
        }
        _ => Err(Error::Invalid("selected payload codec incompatible with ABI")),
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
        let layout = crate::knowledge_stage::KnowledgePayloadLayout::from_model_abi(abi);
        if !layout.uses_carriers() && abi != crate::knowledge_selected::KNOWLEDGE_MODEL_ABI {
            return Err(Error::Invalid("selected payload ABI unsupported"));
        }
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
