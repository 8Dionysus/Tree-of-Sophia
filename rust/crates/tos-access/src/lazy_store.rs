//! Genuine existing QueryStore owner under the same original lazy-session ledger.
//! CorpusHeader uses the existing selected Store metadata owner; generic query
//! kernels remain outside this connected metadata scope.
use super::*;
use tos_query::source_diagnostic::{LegacyStore, OriginalStoreBudget, StoreUsage};

pub(super) struct Store {
    pub owner: LegacyStore,
    /// Exact owner census already included in Ledger, excluded only as an alias
    /// from metadata's callback which itself supplies this same census.
    pub owner_retained: usize,
    pub retained: usize,
    pub exploration_revision_json: Vec<u8>,
}
fn budget<'a>(
    ledger: &'a Ledger,
    heap: &Arc<tos_compiler::DedicatedSessionSqliteHeap>,
    limits: &QueryStoreLimits,
    remaining: &'a dyn Fn(usize) -> tos_query::source_diagnostic::Result<usize>,
) -> OriginalStoreBudget<'a> {
    OriginalStoreBudget {
        original_sqlite_heap: Arc::clone(heap),
        remaining_after_retained: remaining,
        byte_work: Arc::clone(&ledger.work),
        max_byte_work: ledger.work_limit,
        sql_vm_steps: Arc::clone(&ledger.sql_vm),
        max_sql_vm_steps: ledger.sql_vm_limit,
        store_sql_vm_steps: Arc::clone(&ledger.store_sql_vm),
        store_steps: Arc::clone(&ledger.store_steps),
        max_store_steps: limits.max_work_steps,
        json_visits: Arc::clone(&ledger.visits),
        max_json_visits: ledger.visit_limit,
        max_rows_remaining: ledger.rows.get(),
        max_input_bytes_remaining: ledger.input_bytes.get(),
    }
}
// The callback can fail precisely when additional state is exhausted. Keep
// its known static diagnostic and one owner conversion simultaneously admitted.
const CALLBACK_DIAGNOSTIC_STATE: usize =
    2 * (std::mem::size_of::<String>() + "Core lazy original simultaneous state".len());
fn error(text: &'static str) -> tos_query::source_diagnostic::DiagnosticError {
    tos_query::source_diagnostic::DiagnosticError(text.into())
}
fn encode_revision(text: &str, ledger: &Ledger, deadline: Instant) -> Result<Vec<u8>> {
    // Geometry only; existing Core serde string writer owns JSON escaping.
    // Debit original byte/visit work BEFORE count/emit, including failure.
    ledger.charge_work(text.len() as u64)?;
    ledger.charge_visits(2)?;
    let mut quoted = 2usize;
    for (i, byte) in text.bytes().enumerate() {
        if i % 128 == 0 {
            active(deadline)?;
        }
        let width = match byte {
            b'"' | b'\\' => 2,
            b'\n' | b'\r' | b'\t' | 8 | 12 => 2,
            0..=31 => 6,
            _ => 1,
        };
        quoted = quoted
            .checked_add(width)
            .ok_or("Core Store revision geometry")?;
    }
    ledger.charge_work(quoted as u64)?;
    let _state = ledger.reserve(
        quoted
            .checked_add(std::mem::size_of::<BoundedOutput>())
            .and_then(|n| n.checked_add(std::mem::size_of::<&str>()))
            .ok_or("Core Store revision output state")?,
    )?;
    let mut output = BoundedOutput::reserved(quoted, deadline, quoted)?;
    output.value(&text)?;
    if output.bytes.len() != quoted {
        return Err("Core Store revision writer geometry mismatch");
    }
    active(deadline)?;
    Ok(output.bytes)
}
pub(super) fn open(
    request: &Request,
    ledger: &Ledger,
    heap: &Arc<tos_compiler::DedicatedSessionSqliteHeap>,
    owner_deadline: Instant,
    deadline: Instant,
    cancelled: &Arc<AtomicBool>,
) -> Result<Store> {
    let limits = request
        .query_store_limits
        .as_ref()
        .ok_or("Core lazy Store requires original explicit limits")?;
    let native = limits.native()?;
    // Original Core owner supplies exactly five canonical names/selected paths.
    // Admit clones before that helper runs; no captured normalized-row surrogate.
    let paths = [
        &request.source_paths.index_path,
        &request.source_paths.philosophy_graph_projection_path,
        &request.source_paths.bibliographic_graph_path,
        &request.source_paths.entity_type_registry_path,
        &request.source_paths.relation_type_registry_path,
    ];
    let names = [
        "ToS/derived-exports/tos_corpus_index.min.json",
        "ToS/derived-exports/philosophy_graph_projection.min.json",
        "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json",
        "ToS/doctrine/semantic-interchange/entity-types.v1.json",
        "ToS/doctrine/semantic-interchange/relation-types.v1.json",
    ];
    let bytes = paths.iter().zip(names).try_fold(
        std::mem::size_of::<[(String, PathBuf); 5]>(),
        |n, (path, name)| {
            n.checked_add(path.capacity())
                .and_then(|n| n.checked_add(name.len()))
                .ok_or("Core lazy Store selected clone census")
        },
    )?;
    let _inputs = ledger.reserve(bytes)?;
    let inputs = selected_store_inputs(request);
    let remaining = |n| ledger.remaining(n).map_err(error);
    let _locals = ledger.reserve(
        CALLBACK_DIAGNOSTIC_STATE
            + std::mem::size_of::<OriginalStoreBudget<'_>>()
            + std::mem::size_of::<StoreUsage>()
            + 2 * std::mem::size_of::<StoreAbort>()
            + 4 * std::mem::size_of::<usize>()
            + 2 * std::mem::size_of::<Arc<dyn tos_query::AbortProbe>>()
            + std::mem::size_of_val(&remaining)
            + std::mem::size_of::<Store>(),
    )?;
    // Immutable owner lifetime remains the original admitted session cutoff.
    // A distinct operation probe narrows this construction only; QRY restores
    // original progress before handing the retained Store back to this Driver.
    let abort: Arc<dyn tos_query::AbortProbe> = Arc::new(StoreAbort {
        deadline: owner_deadline,
        cancelled: Arc::clone(cancelled),
    });
    let operation: Arc<dyn tos_query::AbortProbe> = Arc::new(StoreAbort {
        deadline,
        cancelled: Arc::clone(cancelled),
    });
    let original = budget(ledger, heap, limits, &remaining);
    let mut usage = StoreUsage::default();
    let result = LegacyStore::open_bounded_with_owned_budget(
        &request.query_store.path,
        &inputs,
        native,
        limits.max_database_bytes,
        owner_deadline,
        abort,
        deadline,
        operation,
        &original,
        &mut usage,
    );
    ledger.debit_store_usage(&usage)?; // QRY already debits original JSON/work/SQL.
    let owner = result.map_err(|_| "Core lazy genuine selected Store opening refused")?;
    let owner_retained = owner
        .retained_state_upper_bound()
        .map_err(|_| "Core lazy Store actual retained state")?;
    // The authentic owner's String stays borrowed; the second buffer is only
    // a bounded transport encoding, never a caller-created revision authority.
    let _owner = ledger.reserve(owner_retained)?;
    let exploration_revision_json = encode_revision(&owner.revision, ledger, deadline)?;
    let retained = owner_retained
        .checked_add(
            std::mem::size_of::<Store>()
                .checked_sub(std::mem::size_of::<LegacyStore>())
                .ok_or("Core lazy Store wrapper inline alias")?,
        )
        .and_then(|n| n.checked_add(exploration_revision_json.capacity()))
        .ok_or("Core lazy Store wrapper census")?;
    ledger.remaining(
        retained
            .checked_sub(owner_retained)
            .ok_or("Core lazy Store opening alias")?,
    )?;
    Ok(Store {
        owner,
        owner_retained,
        retained,
        exploration_revision_json,
    })
}
impl Store {
    pub(super) fn fence(
        &self,
        request: &Request,
        ledger: &Ledger,
        heap: &Arc<tos_compiler::DedicatedSessionSqliteHeap>,
    ) -> Result<()> {
        let limits = request
            .query_store_limits
            .as_ref()
            .ok_or("Core Store fence limits")?;
        let already = self.owner_retained;
        let remaining = |total: usize| {
            ledger
                .remaining(
                    total
                        .checked_sub(already)
                        .ok_or_else(|| error("Core Store alias census"))?,
                )
                .map_err(error)
        };
        let _locals = ledger.reserve(
            CALLBACK_DIAGNOSTIC_STATE
                + std::mem::size_of::<OriginalStoreBudget<'_>>()
                + std::mem::size_of_val(&remaining),
        )?;
        let original = budget(ledger, heap, limits, &remaining);
        self.owner
            .verify_currentness_with_owned_budget(&original)
            .map_err(|_| "Core lazy selected Store currentness fence")
    }
    pub(super) fn metadata(
        &mut self,
        tool: &str,
        request: &Request,
        ledger: &Ledger,
        heap: &Arc<tos_compiler::DedicatedSessionSqliteHeap>,
        deadline: Instant,
        cancelled: &Arc<AtomicBool>,
        max_output: usize,
    ) -> Result<Vec<u8>> {
        let limits = request
            .query_store_limits
            .as_ref()
            .ok_or("Core lazy Store limits absent")?;
        let already = self.owner_retained;
        let remaining = |total: usize| {
            let extra = total
                .checked_sub(already)
                .ok_or_else(|| error("Core Store alias census"))?;
            ledger.remaining(extra).map_err(error)
        };
        let _locals = ledger.reserve(
            CALLBACK_DIAGNOSTIC_STATE
                + std::mem::size_of::<OriginalStoreBudget<'_>>()
                + std::mem::size_of::<StoreUsage>()
                + std::mem::size_of::<StoreAbort>()
                + 2 * std::mem::size_of::<usize>()
                + std::mem::size_of::<Arc<dyn tos_query::AbortProbe>>()
                + std::mem::size_of_val(&remaining),
        )?;
        let original = budget(ledger, heap, limits, &remaining);
        let abort: Arc<dyn tos_query::AbortProbe> = Arc::new(StoreAbort {
            deadline,
            cancelled: Arc::clone(cancelled),
        });
        let mut usage = StoreUsage::default();
        let result = self.owner.metadata_packet_with_owned_budget(
            tool,
            &original,
            &mut usage,
            deadline,
            abort,
            max_output.min(limits.max_json_bytes),
        );
        ledger.debit_store_usage(&usage)?; // Both outcomes, before result/disclosure.
        let bytes = result.map_err(|_| "Core lazy selected Store metadata refused")?;
        let now = self
            .owner
            .retained_state_upper_bound()
            .map_err(|_| "Core Store final retained census")?;
        if now != self.owner_retained {
            return Err("Core Store retained association changed");
        }
        Ok(bytes) // Caller keeps this body AND authentic Store through send/final fences.
    }
    pub(super) fn search(
        &mut self,
        arguments: &tos_foundation::JsonValue,
        request: &Request,
        ledger: &Ledger,
        heap: &Arc<tos_compiler::DedicatedSessionSqliteHeap>,
        deadline: Instant,
        cancelled: &Arc<AtomicBool>,
        max_output: usize,
    ) -> Result<Vec<u8>> {
        let limits = request
            .query_store_limits
            .as_ref()
            .ok_or("Core lazy Store limits absent")?;
        let already = self.owner_retained;
        let remaining = |total: usize| {
            let extra = total
                .checked_sub(already)
                .ok_or_else(|| error("Core Store alias census"))?;
            ledger.remaining(extra).map_err(error)
        };
        let _locals = ledger.reserve(
            CALLBACK_DIAGNOSTIC_STATE
                + std::mem::size_of::<OriginalStoreBudget<'_>>()
                + std::mem::size_of::<StoreUsage>()
                + std::mem::size_of::<StoreAbort>()
                + 2 * std::mem::size_of::<usize>()
                + std::mem::size_of::<Arc<dyn tos_query::AbortProbe>>()
                + std::mem::size_of_val(&remaining),
        )?;
        let original = budget(ledger, heap, limits, &remaining);
        let abort: Arc<dyn tos_query::AbortProbe> = Arc::new(StoreAbort {
            deadline,
            cancelled: Arc::clone(cancelled),
        });
        let mut usage = StoreUsage::default();
        let result = self.owner.search_packet_from_foundation_with_owned_budget(
            arguments,
            &original,
            &mut usage,
            deadline,
            abort,
            max_output.min(limits.max_json_bytes),
        );
        ledger.debit_store_usage(&usage)?; // Both outcomes, before result/disclosure.
        let bytes = result.map_err(|_| "Core lazy selected Store Search refused")?;
        let now = self
            .owner
            .retained_state_upper_bound()
            .map_err(|_| "Core Store final retained census")?;
        if now != self.owner_retained {
            return Err("Core Store retained association changed");
        }
        Ok(bytes) // Caller keeps this body AND authentic Store through send/final fences.
    }
}
