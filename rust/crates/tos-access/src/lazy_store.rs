//! Genuine existing QueryStore owner under the same original lazy-session ledger.
//! CorpusHeader uses the existing selected Store metadata owner; generic query
//! kernels remain outside this connected metadata scope.
use super::*;
use serde_json::Value;
use std::io::Write;
use tos_foundation::JsonValue as FoundationValue;
use tos_query::search_v2::SearchV2ErrorCode;
use tos_query::source_diagnostic::{LegacyStore, OriginalStoreBudget, StoreUsage};
use tos_query::source_diagnostic::{QueryStoreIndexedContinuation, QueryStoreIndexedSearchRequest};

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

fn foundation_falsey(value: &FoundationValue) -> bool {
    match value {
        FoundationValue::Null => true,
        FoundationValue::Bool(value) => !value,
        FoundationValue::Number(number) => match number.kind {
            tos_foundation::JsonNumberKind::Int => {
                number.lexeme.parse::<i128>().map_or(false, |n| n == 0)
            }
            tos_foundation::JsonNumberKind::Float => {
                number.as_python_float().map_or(false, |n| n == 0.0)
            }
        },
        FoundationValue::String(value) => value.units().is_empty(),
        FoundationValue::Array(values) => values.is_empty(),
        FoundationValue::Object(values) => values.is_empty(),
    }
}

fn copied_filter_values(
    value: Option<&FoundationValue>,
    default_sources: bool,
    ledger: &Ledger,
    deadline: Instant,
) -> Result<Vec<String>> {
    let mut values = Vec::new();
    if let Some(value) = value.filter(|value| !foundation_falsey(value)) {
        match value {
            FoundationValue::Array(items) => {
                if items.len() > 100 {
                    return Err("Core indexed filter count");
                }
                values
                    .try_reserve_exact(items.len())
                    .map_err(|_| "Core indexed filter allocation")?;
                if values.capacity() != items.len() {
                    return Err("Core indexed filter capacity");
                }
                for item in items {
                    active(deadline)?;
                    ledger.charge_work(1)?;
                    let text = item.as_str().ok_or("Core indexed filter type")?;
                    if text.chars().count() > 256 {
                        return Err("Core indexed filter value bound");
                    }
                    values.push(text.to_owned());
                }
            }
            FoundationValue::String(text) => {
                let text = text.as_str().ok_or("Core indexed filter Unicode")?;
                let count = text.chars().count();
                if count > 100 {
                    return Err("Core indexed filter count");
                }
                values
                    .try_reserve_exact(count)
                    .map_err(|_| "Core indexed filter allocation")?;
                if values.capacity() != count {
                    return Err("Core indexed filter capacity");
                }
                for ch in text.chars() {
                    active(deadline)?;
                    ledger.charge_work(ch.len_utf8() as u64)?;
                    values.push(ch.to_string());
                }
            }
            FoundationValue::Object(fields) => {
                if fields.len() > 100 {
                    return Err("Core indexed filter count");
                }
                values
                    .try_reserve_exact(fields.len())
                    .map_err(|_| "Core indexed filter allocation")?;
                if values.capacity() != fields.len() {
                    return Err("Core indexed filter capacity");
                }
                for (key, _) in fields {
                    active(deadline)?;
                    ledger.charge_work(key.units().len() as u64)?;
                    values.push(
                        key.as_str()
                            .ok_or("Core indexed filter Unicode")?
                            .to_owned(),
                    );
                }
            }
            _ => return Err("Core indexed filter iterable type"),
        }
    }
    if values.is_empty() && default_sources {
        let defaults = tos_query::source_diagnostic::query_store_indexed_default_sources();
        values
            .try_reserve_exact(defaults.len())
            .map_err(|_| "Core indexed default source allocation")?;
        if values.capacity() != defaults.len() {
            return Err("Core indexed default source capacity");
        }
        for source in defaults {
            values.push((*source).to_owned());
        }
    }
    values.sort_unstable();
    values.dedup();
    if values.len() > 100 || values.iter().any(|value| value.chars().count() > 256) {
        return Err("Core indexed filter bound");
    }
    let bytes = values.iter().try_fold(0usize, |sum, value| {
        sum.checked_add(value.len())
            .ok_or("Core indexed filter bytes")
    })?;
    if bytes > 6 * 1024 {
        return Err("Core indexed filter bytes");
    }
    Ok(values)
}

fn cursor_error(error: tos_query::search_v2::SearchV2Error) -> &'static str {
    match error.code {
        SearchV2ErrorCode::InvalidRequest
        | SearchV2ErrorCode::QueryTooLong
        | SearchV2ErrorCode::QueryTooShort => "Core indexed cursor or query invalid",
        SearchV2ErrorCode::StaleSelection => "Core indexed cursor selected Store changed",
        SearchV2ErrorCode::StaleContinuation => "Core indexed cursor query or filters changed",
        SearchV2ErrorCode::CursorExpired => "Core indexed cursor expired",
        SearchV2ErrorCode::BudgetExceeded => "Core indexed cursor budget exceeded",
        _ => "Core indexed cursor unavailable",
    }
}

fn query_error(error: tos_query::source_diagnostic::DiagnosticError) -> &'static str {
    let message = error.0.as_str();
    if message.contains("cursor expired") {
        "Core indexed cursor expired"
    } else if message.contains("cursor snapshot changed") || message.contains("source revision") {
        "Core indexed cursor selected Store changed"
    } else if message.contains("cursor query") || message.contains("cursor filters") {
        "Core indexed cursor query or filters changed"
    } else if message.contains("budget") || message.contains("remaining") {
        "Core indexed QueryStore budget exceeded"
    } else if message.contains("query") || message.contains("filter") || message.contains("cursor")
    {
        "Core indexed request invalid"
    } else {
        "Core selected Store indexed search refused"
    }
}

#[derive(serde::Serialize)]
struct IndexedFiltersPacket<'a> {
    sources: &'a [String],
    kind_ids: &'a [String],
    predicate_ids: &'a [String],
}
#[derive(serde::Serialize)]
struct IndexedPagePacket<'a> {
    cursor: Option<&'a str>,
    next_cursor: Option<&'a str>,
    limit_per_kind: usize,
    ordering_scope: &'static str,
    has_more: bool,
}
#[derive(serde::Serialize)]
struct IndexedCountsPacket {
    matching_nodes: Option<usize>,
    matching_relations: Option<usize>,
    returned_nodes: usize,
    returned_relations: usize,
    scope: &'static str,
}
#[derive(serde::Serialize)]
struct IndexedKindWorkPacket {
    candidate_rows: u64,
    verified_chars: u64,
    sql_pages: u64,
}
#[derive(serde::Serialize)]
struct IndexedWorkPacket {
    nodes: IndexedKindWorkPacket,
    relations: IndexedKindWorkPacket,
}
#[derive(serde::Serialize)]
struct IndexedStorePacket<'a> {
    schema: &'static str,
    source_revision: Option<&'a str>,
    query: &'a str,
    filters: IndexedFiltersPacket<'a>,
    page: IndexedPagePacket<'a>,
    counts: IndexedCountsPacket,
    nodes: &'a [Value],
    relations: &'a [Value],
    authority_boundary: &'a Value,
    work: IndexedWorkPacket,
}

struct CountWriter<'a> {
    bytes: usize,
    cap: usize,
    ledger: &'a Ledger,
    deadline: Instant,
}
impl Write for CountWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        active(self.deadline).map_err(|_| std::io::Error::other("indexed output deadline"))?;
        let next = self
            .bytes
            .checked_add(bytes.len())
            .filter(|next| *next <= self.cap)
            .ok_or_else(|| std::io::Error::other("indexed output cap"))?;
        self.ledger
            .charge_work(bytes.len() as u64)
            .map_err(|_| std::io::Error::other("indexed output work cap"))?;
        self.bytes = next;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

struct OutputWriter<'a> {
    bytes: Vec<u8>,
    cap: usize,
    ledger: &'a Ledger,
    deadline: Instant,
}
impl Write for OutputWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        active(self.deadline).map_err(|_| std::io::Error::other("indexed output deadline"))?;
        let next = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .filter(|next| *next <= self.cap)
            .ok_or_else(|| std::io::Error::other("indexed output cap"))?;
        self.ledger
            .charge_work(bytes.len() as u64)
            .map_err(|_| std::io::Error::other("indexed output work cap"))?;
        if next > self.bytes.capacity() {
            return Err(std::io::Error::other("indexed output reservation exceeded"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
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

    pub(super) fn supports_indexed_fts5(&self) -> bool {
        self.owner.supports_indexed_fts5()
    }

    pub(super) fn indexed_search(
        &mut self,
        arguments: &FoundationValue,
        request: &Request,
        ledger: &Ledger,
        heap: &Arc<tos_compiler::DedicatedSessionSqliteHeap>,
        deadline: Instant,
        cancelled: &Arc<AtomicBool>,
        max_output: usize,
    ) -> Result<Vec<u8>> {
        if !self.supports_indexed_fts5() {
            return Err("Core selected QueryStore indexed FTS5 admission absent");
        }
        let limits = request
            .query_store_limits
            .as_ref()
            .ok_or("Core lazy Store limits absent")?;
        let max_verify_chars = match request.search_read_model.as_ref() {
            Some(selection) => {
                selection.validate_verify_chars()?;
                selection.max_verify_chars
            }
            None => 16_000_000,
        };
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
                + std::mem::size_of::<QueryStoreIndexedSearchRequest<'_>>()
                + std::mem::size_of::<QueryStoreIndexedContinuation<'_>>()
                + std::mem::size_of::<crate::reference_cursor::ReferenceCursorBinding>()
                + std::mem::size_of::<crate::reference_cursor::ReferenceCursorEnvelope>()
                + std::mem::size_of::<IndexedStorePacket<'_>>()
                + std::mem::size_of::<IndexedFiltersPacket<'_>>()
                + std::mem::size_of::<IndexedPagePacket<'_>>()
                + std::mem::size_of::<IndexedCountsPacket>()
                + std::mem::size_of::<IndexedWorkPacket>()
                + 2 * std::mem::size_of::<IndexedKindWorkPacket>()
                + std::mem::size_of::<CountWriter<'_>>()
                + std::mem::size_of::<OutputWriter<'_>>()
                + 3 * std::mem::size_of::<usize>()
                + std::mem::size_of::<Arc<dyn tos_query::AbortProbe>>()
                + std::mem::size_of_val(&remaining),
        )?;
        let _request_state = ledger.reserve(64 * 1024)?;
        if arguments.as_object().is_none() {
            return Err("Core indexed arguments object absent");
        }
        let query_value = super::checked_field(arguments, "query", ledger, deadline)?;
        let query = match query_value {
            None => "",
            Some(value) => value
                .as_str()
                .ok_or("Core indexed query must be a string")?,
        };
        let mut query_points = 0usize;
        for _ in query.chars() {
            active(deadline)?;
            ledger.charge_work(1)?;
            query_points += 1;
            if query_points > 256 {
                return Err("Core indexed query exceeds 256 characters");
            }
        }
        let normalize_reservation = query_points
            .checked_mul(6)
            .and_then(|bytes| bytes.checked_add(1024 + std::mem::size_of::<String>()))
            .ok_or("Core indexed query normalization state")?;
        let _normalize = ledger.reserve(normalize_reservation)?;
        let stripped = tos_foundation::python_strip_unicode16_v1(query, 256)
            .map_err(|_| "Core indexed query normalization refused")?;
        let normalized = tos_foundation::python_lower_unicode16_v1(stripped, 256, 256, 1024)
            .map_err(|_| "Core indexed query normalization refused")?;
        ledger.charge_work((stripped.len() + normalized.len()) as u64)?;

        let sources_value = super::checked_field(arguments, "sources", ledger, deadline)?;
        let kind_value = super::checked_field(arguments, "kind_ids", ledger, deadline)?;
        let predicate_value = super::checked_field(arguments, "predicate_ids", ledger, deadline)?;
        let sources = copied_filter_values(sources_value, true, ledger, deadline)?;
        let kind_ids = copied_filter_values(kind_value, false, ledger, deadline)?;
        let predicate_ids = copied_filter_values(predicate_value, false, ledger, deadline)?;
        let filter_count = sources
            .len()
            .checked_add(kind_ids.len())
            .and_then(|count| count.checked_add(predicate_ids.len()))
            .ok_or("Core indexed filter count")?;
        let filter_bytes = sources
            .iter()
            .chain(&kind_ids)
            .chain(&predicate_ids)
            .try_fold(0usize, |sum, value| {
                sum.checked_add(value.len())
                    .ok_or("Core indexed filter bytes")
            })?;
        if filter_count > 100 || filter_bytes > 6 * 1024 {
            return Err("Core indexed filter bound");
        }
        let limit_value = super::checked_field(arguments, "limit", ledger, deadline)?;
        let limit = tos_query::indexed_limit_from_foundation(limit_value);
        let cursor_value = super::checked_field(arguments, "cursor", ledger, deadline)?;
        let cursor = match cursor_value {
            None | Some(FoundationValue::Null) => None,
            Some(value) => Some(
                value
                    .as_str()
                    .ok_or("Core indexed cursor must be a string or null")?,
            ),
        };
        if cursor.is_some_and(|token| {
            token.is_empty()
                || token.len() > crate::reference_cursor::REFERENCE_INDEXED_CURSOR_MAX_BYTES
        }) {
            return Err("Core indexed cursor length invalid");
        }
        let cursor_source_revision = if cursor.is_some() {
            let revision = self
                .owner
                .indexed_graph_source_revision()
                .map_err(|_| "Core selected Store graph source revision absent")?;
            if revision.is_some_and(|value| {
                value.len() > crate::reference_cursor::REFERENCE_INDEXED_CURSOR_MAX_BYTES
            }) {
                return Err("Core indexed cursor source binding exceeds envelope");
            }
            revision.map(str::to_owned)
        } else {
            None
        };
        let mut binding = if cursor.is_some() {
            Some(
                crate::reference_cursor::ReferenceCursorBinding::new_for_query_store(
                    cursor_source_revision,
                    normalized.clone(),
                    sources.clone(),
                    kind_ids.clone(),
                    predicate_ids.clone(),
                )
                .map_err(cursor_error)?,
            )
        } else {
            None
        };
        let mut decoded = None;
        if let (Some(token), Some(binding)) = (cursor, binding.as_ref()) {
            let _cursor_state = ledger.reserve(
                crate::reference_cursor::REFERENCE_INDEXED_CURSOR_MAX_BYTES * 4
                    + 4 * crate::reference_cursor::REFERENCE_SEARCH_CHILD_MAX_BYTES
                    + 1024,
            )?;
            ledger.charge_work(token.len() as u64)?;
            decoded = Some(
                crate::reference_cursor::decode_reference_indexed_cursor(token, binding)
                    .map_err(cursor_error)?,
            );
        }
        let envelope = decoded.as_ref();
        let query_request = QueryStoreIndexedSearchRequest {
            query,
            sources: &sources,
            kind_ids: &kind_ids,
            predicate_ids: &predicate_ids,
            limit_per_kind: limit,
        };
        let continuation = QueryStoreIndexedContinuation {
            cursor_present: cursor.is_some(),
            nodes: envelope.and_then(|envelope| envelope.nodes.as_deref()),
            relations: envelope.and_then(|envelope| envelope.relations.as_deref()),
            nodes_exhausted: envelope.is_some_and(|envelope| envelope.nodes_exhausted),
            relations_exhausted: envelope.is_some_and(|envelope| envelope.relations_exhausted),
        };
        let original = budget(ledger, heap, limits, &remaining);
        let abort: Arc<dyn tos_query::AbortProbe> = Arc::new(StoreAbort {
            deadline,
            cancelled: Arc::clone(cancelled),
        });
        let mut usage = StoreUsage::default();
        let result = self.owner.indexed_query_store_page_with_owned_budget(
            &query_request,
            &continuation,
            max_verify_chars,
            &original,
            &mut usage,
            deadline,
            abort,
        );
        ledger.debit_store_usage(&usage)?;
        let mut page = result.map_err(query_error)?;
        let graph_revision = self
            .owner
            .indexed_graph_source_revision()
            .map_err(|_| "Core selected Store graph source revision absent")?;
        if page.store_revision != self.owner.revision
            || page.source_revision.as_deref() != graph_revision
            || page.normalized_query != normalized
            || page.filters.sources.as_slice() != query_request.sources
            || page.filters.kind_ids.as_slice() != query_request.kind_ids
            || page.filters.predicate_ids.as_slice() != query_request.predicate_ids
            || page.has_more != (page.nodes.has_more || page.relations.has_more)
        {
            return Err("Core indexed QueryStore page binding changed");
        }
        let now = self
            .owner
            .retained_state_upper_bound()
            .map_err(|_| "Core Store final retained census")?;
        if now != self.owner_retained {
            return Err("Core Store retained association changed");
        }
        let _page_state = ledger.reserve(page.retained_bytes)?;
        if page.has_more && binding.is_none() {
            let _cursor_state = ledger
                .reserve(crate::reference_cursor::REFERENCE_INDEXED_CURSOR_MAX_BYTES * 4 + 4096)?;
            binding = Some(
                crate::reference_cursor::ReferenceCursorBinding::new_for_query_store(
                    page.source_revision.clone(),
                    page.normalized_query.clone(),
                    page.filters.sources.clone(),
                    page.filters.kind_ids.clone(),
                    page.filters.predicate_ids.clone(),
                )
                .map_err(cursor_error)?,
            );
        }
        let next_cursor = if page.has_more {
            let binding = binding
                .as_ref()
                .ok_or("Core indexed cursor binding absent")?;
            let nodes = page.nodes.next_cursor.take();
            let relations = page.relations.next_cursor.take();
            let nodes_exhausted = nodes.is_none();
            let relations_exhausted = relations.is_none();
            let envelope = crate::reference_cursor::ReferenceCursorEnvelope::new(
                nodes,
                relations,
                nodes_exhausted,
                relations_exhausted,
            )
            .map_err(cursor_error)?;
            let token =
                crate::reference_cursor::encode_reference_indexed_cursor(binding, &envelope)
                    .map_err(cursor_error)?;
            ledger.charge_work(token.len() as u64)?;
            Some(token)
        } else {
            None
        };
        let empty_boundary = Value::Object(Default::default());
        let authority_boundary = self
            .owner
            .graph_header
            .get("authority_boundary")
            .unwrap_or(&empty_boundary);
        let first_page = cursor.is_none();
        let packet = IndexedStorePacket {
            schema: "tos_knowledge_search_indexed_v2",
            source_revision: page.source_revision.as_deref(),
            query,
            filters: IndexedFiltersPacket {
                sources: &page.filters.sources,
                kind_ids: &page.filters.kind_ids,
                predicate_ids: &page.filters.predicate_ids,
            },
            page: IndexedPagePacket {
                cursor,
                next_cursor: next_cursor.as_deref(),
                limit_per_kind: limit,
                ordering_scope: "global-rank",
                has_more: next_cursor.is_some(),
            },
            counts: IndexedCountsPacket {
                matching_nodes: (first_page && !page.nodes.has_more)
                    .then_some(page.nodes.rows.len()),
                matching_relations: (first_page && !page.relations.has_more)
                    .then_some(page.relations.rows.len()),
                returned_nodes: page.nodes.rows.len(),
                returned_relations: page.relations.rows.len(),
                scope: "exact-if-kind-exhausted-without-continuation",
            },
            nodes: &page.nodes.rows,
            relations: &page.relations.rows,
            authority_boundary,
            work: IndexedWorkPacket {
                nodes: IndexedKindWorkPacket {
                    candidate_rows: page.nodes.candidate_rows,
                    verified_chars: page.nodes.verified_chars,
                    sql_pages: page.nodes.sql_pages,
                },
                relations: IndexedKindWorkPacket {
                    candidate_rows: page.relations.candidate_rows,
                    verified_chars: page.relations.verified_chars,
                    sql_pages: page.relations.sql_pages,
                },
            },
        };
        let output_cap = max_output.min(limits.max_json_bytes);
        let mut counter = CountWriter {
            bytes: 0,
            cap: output_cap,
            ledger,
            deadline,
        };
        serde_json::to_writer(&mut counter, &packet)
            .map_err(|_| "Core indexed response output count refused")?;
        let output_bytes = counter.bytes;
        let _output_state = ledger.reserve(
            output_bytes
                .checked_add(
                    std::mem::size_of::<Vec<u8>>() + std::mem::size_of::<OutputWriter<'_>>(),
                )
                .ok_or("Core indexed output state")?,
        )?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(output_bytes)
            .map_err(|_| "Core indexed response allocation")?;
        if bytes.capacity() != output_bytes {
            return Err("Core indexed response capacity");
        }
        let mut writer = OutputWriter {
            bytes,
            cap: output_bytes,
            ledger,
            deadline,
        };
        serde_json::to_writer(&mut writer, &packet)
            .map_err(|_| "Core indexed response serialization refused")?;
        if writer.bytes.len() != output_bytes {
            return Err("Core indexed response geometry changed");
        }
        active(deadline)?;
        Ok(writer.bytes)
    }
}
