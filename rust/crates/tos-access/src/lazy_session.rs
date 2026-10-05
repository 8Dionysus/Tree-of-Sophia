//! One serial Stage/control session; selected carriers precede WholeRoot.
//! Construction remains source preparation until the owned D1/Store/Root joins are complete.
use super::*;
#[path = "lazy_store.rs"]
mod lazy_store;
#[path = "native_ledger.rs"]
mod native_ledger;
use native_ledger::{Ledger, Workspace};
use tos_compiler::private_tmpfs_stage::{PRIVATE_TMPFS_SELECT_COST, PRIVATE_TMPFS_VERIFY_COST};

fn field<'a>(
    v: &'a tos_foundation::JsonValue,
    name: &str,
) -> Option<&'a tos_foundation::JsonValue> {
    v.as_object()?
        .iter()
        .find(|(k, _)| k.as_str() == Some(name))
        .map(|(_, v)| v)
}
fn checked_field<'a>(
    value: &'a tos_foundation::JsonValue,
    name: &str,
    ledger: &Ledger,
    deadline: Instant,
) -> Result<Option<&'a tos_foundation::JsonValue>> {
    let Some(object) = value.as_object() else {
        return Ok(None);
    };
    for (key, item) in object {
        let bytes = key
            .units()
            .len()
            .checked_mul(2)
            .and_then(|n| n.checked_add(name.len()))
            .ok_or("Core Store Search borrowed lookup work")?;
        ledger.charge_work(bytes as u64)?;
        active(deadline)?;
        if key.as_str() == Some(name) {
            return Ok(Some(item));
        }
    }
    Ok(None)
}

fn checked_document_storage(
    value: &tos_foundation::JsonValue,
    depth: usize,
    maximum_depth: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<usize> {
    active(deadline)?;
    if cancelled.load(Ordering::Acquire) {
        return Err("Core Search census cancelled");
    }
    if depth > maximum_depth {
        return Err("Core Search census original depth");
    }
    use tos_foundation::JsonValue as J;
    let add =
        |left: usize, right: usize| left.checked_add(right).ok_or("Core Search census overflow");
    match value {
        J::Null | J::Bool(_) | J::Number(_) | J::String(_) => value
            .retained_storage_bytes()
            .map_err(|_| "Core Search leaf census"),
        J::Array(values) => {
            let mut bytes = values
                .capacity()
                .checked_mul(std::mem::size_of::<J>())
                .ok_or("Core Search array census")?;
            for child in values {
                bytes = add(
                    bytes,
                    checked_document_storage(child, depth + 1, maximum_depth, deadline, cancelled)?,
                )?;
            }
            Ok(bytes)
        }
        J::Object(entries) => {
            let mut bytes = entries
                .capacity()
                .checked_mul(std::mem::size_of::<(tos_foundation::JsonString, J)>())
                .ok_or("Core Search object census")?;
            for (key, child) in entries {
                active(deadline)?;
                if cancelled.load(Ordering::Acquire) {
                    return Err("Core Search key census cancelled");
                }
                bytes = add(
                    bytes,
                    key.retained_storage_bytes()
                        .map_err(|_| "Core Search key census")?,
                )?;
                bytes = add(
                    bytes,
                    checked_document_storage(child, depth + 1, maximum_depth, deadline, cancelled)?,
                )?;
            }
            Ok(bytes)
        }
    }
}

fn call(
    raw: &[u8],
    sequence: u64,
    request: &Request,
    state: &Ledger,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(
    Option<Operation>,
    Option<tos_foundation::JsonDocument>,
    Instant,
    usize,
)> {
    if cancelled.load(Ordering::Acquire) {
        return Err("Core lazy original cancellation");
    }
    active(deadline)?;
    state.charge_work(raw.len() as u64)?; // Before traversal, including invalid/refused calls.
    let mut limits = request.admission.json.limits()?;
    limits.max_depth = limits.max_depth.min(96);
    limits.max_visits = limits.max_visits.min(state.remaining_visits()?);
    let mut check = || {
        if cancelled.load(Ordering::Acquire) {
            return Err(tos_foundation::FoundationError::new(
                tos_foundation::FoundationErrorCode::BudgetExceeded,
                "Core lazy original cancellation",
            ));
        }
        active(deadline).map_err(|_| {
            tos_foundation::FoundationError::new(
                tos_foundation::FoundationErrorCode::BudgetExceeded,
                "Core probe original parse cutoff",
            )
        })
    };
    let fixed = std::mem::size_of::<JsonLimits>()
        + std::mem::size_of::<tos_foundation::JsonDocument>()
        + std::mem::size_of::<Value>()
        + std::mem::size_of::<ExistsKind>()
        + std::mem::size_of::<Operation>()
        + std::mem::size_of::<(
            u64,
            u64,
            &[u8],
            &Request,
            &Ledger,
            &AtomicBool,
            &tos_foundation::JsonValue,
            &str,
            Instant,
        )>()
        + std::mem::size_of::<Instant>()
        + std::mem::size_of_val(&check)
        + std::mem::size_of::<
            std::slice::Iter<'_, (tos_foundation::JsonString, tos_foundation::JsonValue)>,
        >()
        + std::mem::size_of::<(&tos_foundation::JsonValue, &str, &Ledger, Instant, usize)>()
        + 4 * std::mem::size_of::<usize>();
    let _local = state.reserve(fixed)?;
    // Both branch iterators and recursive local/result slots are priced
    // before parsing, so live document + recursive census cannot borrow
    // overlapping state. No allocator or actual compiler-stack claim.
    let census_frame = std::mem::size_of::<(
        &tos_foundation::JsonValue,
        usize,
        usize,
        Instant,
        &AtomicBool,
    )>() + 3 * std::mem::size_of::<usize>()
        + std::mem::size_of::<Result<usize>>()
        + std::mem::size_of::<std::slice::Iter<'_, tos_foundation::JsonValue>>()
        + std::mem::size_of::<
            std::slice::Iter<'_, (tos_foundation::JsonString, tos_foundation::JsonValue)>,
        >()
        + std::mem::size_of::<(&tos_foundation::JsonString, &tos_foundation::JsonValue)>();
    let _census = state.reserve(
        census_frame
            .checked_mul(
                limits
                    .max_depth
                    .checked_add(1)
                    .ok_or("Core Search census depth slots")?,
            )
            .ok_or("Core Search census frame slots")?,
    )?;
    // Parse failure is terminal and keeps the admitted original visit ceiling.
    state.charge_visits(limits.max_visits)?;
    let doc = tos_foundation::parse_json_with_state_budget_and_check(
        raw,
        JsonMode::PublishedStrict,
        limits,
        state.remaining(0)?,
        &mut check,
    )
    .map_err(|_| "Core probe original JSON/state admission")?;
    state.settle_unused_visits(
        limits
            .max_visits
            .checked_sub(doc.visits())
            .ok_or("Core lazy parser visit report")?,
    )?;
    let root = doc.root();
    let fields = root.as_object().ok_or("Core probe call object")?;
    if fields.len() != 5
        || fields.iter().any(|(k, _)| {
            !matches!(
                k.as_str(),
                Some(
                    "schema_version" | "sequence" | "operation" | "arguments" | "work_deadline_ns"
                )
            )
        })
        || field(root, "schema_version").and_then(tos_foundation::JsonValue::as_str)
            != Some("tos_native_core_session_call_v1")
        || field(root, "sequence").and_then(tos_foundation::JsonValue::as_u64) != Some(sequence)
    {
        return Err("Core probe strict call DTO");
    }
    let work = field(root, "work_deadline_ns")
        .and_then(tos_foundation::JsonValue::as_u64)
        .filter(|n| *n > 0 && *n <= request.admission.work_deadline_ns)
        .ok_or("Core probe call cutoff extension")?;
    let id = field(root, "operation")
        .and_then(tos_foundation::JsonValue::as_str)
        .ok_or("Core probe operation")?;
    if id == "tos_native_call" {
        let arguments = field(root, "arguments").ok_or("Core Store Search arguments absent")?;
        let fields = arguments
            .as_object()
            .ok_or("Core Store Search arguments object")?;
        if fields.len() != 2 {
            return Err("Core Store Search strict generic DTO");
        }
        for (key, _) in fields {
            state.charge_work(
                key.units()
                    .len()
                    .checked_mul(2)
                    .ok_or("Core Store Search key work")? as u64,
            )?;
            active(deadline)?;
            if !matches!(key.as_str(), Some("tool" | "arguments")) {
                return Err("Core Store Search strict generic key");
            }
        }
        if !matches!(
            checked_field(arguments, "tool", state, deadline)?
                .and_then(tos_foundation::JsonValue::as_str),
            Some("tos_knowledge_search" | "tos_knowledge_search_indexed_v2")
        ) || checked_field(arguments, "arguments", state, deadline)?
            .and_then(tos_foundation::JsonValue::as_object)
            .is_none()
        {
            return Err("Core Store Search exact tool DTO");
        }
        // Existing retained tree census visits no string contents or new heap;
        // admit its actual bounded traversal before moving the document owner.
        state.charge_work(
            (doc.visits() as u64)
                .checked_mul(std::mem::size_of::<(
                    tos_foundation::JsonString,
                    tos_foundation::JsonValue,
                )>() as u64)
                .ok_or("Core Store Search document census work")?,
        )?;
        active(deadline)?;
        let retained =
            checked_document_storage(doc.root(), 0, limits.max_depth, deadline, cancelled)?;
        state.remaining(retained)?;
        active(deadline)?;
        return Ok((
            None,
            Some(doc),
            original_cli_deadline(work)?.min(deadline),
            retained,
        ));
    }
    if !field(root, "arguments")
        .and_then(tos_foundation::JsonValue::as_object)
        .is_some_and(|fields| fields.is_empty())
    {
        return Err("Core probe strict empty arguments");
    }
    if !matches!(
        id,
        "tos_corpus_index_exists"
            | "tos_philosophy_projection_exists"
            | "tos_evidence_projection_exists"
            | "tos_philosophy_audit_exists"
            | "tos_corpus_index"
            | "tos_bibliographic_graph"
            | "tos_philosophy_projection"
            | "tos_philosophy_audit_payload"
            | "tos_corpus_header"
    ) {
        return Err("Core probe operation outside admitted profile");
    }
    // Existing Core operation owner is the sole operation-to-selected-path grammar.
    let selected = operation(id, Value::Object(Default::default()))?;
    Ok((
        Some(selected),
        None,
        original_cli_deadline(work)?.min(deadline),
        0,
    ))
}

struct Carrier {
    capture: tos_compiler::PublicCapture,
    role: tos_compiler::RuntimeCaptureRole,
    retained: usize,
}
fn carrier(
    operation: &Operation,
) -> Option<(
    tos_compiler::native_snapshot_carriers::CapturedCarrierRequest,
    tos_compiler::RuntimeCaptureRole,
    &'static str,
)> {
    use tos_compiler::RuntimeCaptureRole as R;
    use tos_compiler::native_snapshot_carriers::CapturedCarrierRequest as C;
    Some(match operation {
        Operation::CorpusIndex => (C::CorpusIndex, R::Corpus, "carrier_corpus"),
        Operation::CorpusHeader => (C::CorpusHeader, R::Corpus, "carrier_corpus"),
        Operation::Bibliographic => (
            C::BibliographicGraph,
            R::Bibliographic,
            "carrier_bibliographic",
        ),
        Operation::PhilosophyProjection => {
            (C::PhilosophyProjection, R::Philosophy, "carrier_philosophy")
        }
        Operation::PhilosophyAuditPayload => (
            C::PhilosophyAuditPayload,
            R::PhilosophyAudit,
            "carrier_philosophy_audit",
        ),
        _ => return None,
    })
}
// Default Store presence uses the same bounded selected metadata owner, before
// any capture. A present directory also selects Store and is then refused by its
// authentic opener, rather than silently falling back to Corpus.
fn with_selected_store_choice<T>(
    request: &Request,
    ledger: &Ledger,
    deadline: Instant,
    use_selection: impl FnOnce(bool) -> Result<T>,
) -> Result<T> {
    if request.query_store.configured {
        return use_selection(true);
    }
    let path = &request.query_store.path;
    let _metadata = ledger.reserve(
        SELECTED_PROBE_METADATA_WORKSPACE
            .checked_add(std::mem::size_of_val(&use_selection))
            .ok_or("Core lazy selected Store callback state")?,
    )?;
    ledger.charge_work(
        (path.as_os_str().len() as u64)
            .checked_mul(4)
            .ok_or("Core lazy selected Store path work")?,
    )?;
    with_selected_metadata_property(path, deadline, false, use_selection)
}
fn selected_paths_size(s: &Sources) -> Result<usize> {
    let paths = [
        &s.index_path,
        &s.philosophy_graph_projection_path,
        &s.bibliographic_graph_path,
        &s.entity_type_registry_path,
        &s.relation_type_registry_path,
        &s.philosophy_post_planting_audit_path,
        &s.evidence_projection_path,
    ];
    paths.into_iter().try_fold(
        std::mem::size_of::<tos_compiler::PublicCaptureInputPaths>(),
        |n, p| {
            n.checked_add(p.capacity())
                .ok_or("Core lazy path clone census overflow")
        },
    )
}
fn create_selected_capture(
    root: &Path,
    request: &Request,
    isolation: &tos_compiler::private_tmpfs_stage::PrivateTmpfsStageIsolation,
    profile: tos_compiler::RuntimeCaptureProfile,
    generation: u64,
    ledger: &Ledger,
    owner_deadline: Instant,
    deadline: Instant,
    cancelled: &Arc<AtomicBool>,
    sqlite_heap: &Arc<tos_compiler::DedicatedSessionSqliteHeap>,
) -> Result<tos_compiler::PublicCapture> {
    active(deadline)?;
    let mut limits = tos_compiler::native_snapshot_manifest::portable_native_snapshot_limits(
        request.admission.max_build_seconds,
    )
    .map_err(|_| "Core lazy existing carrier limits")?
    .capture;
    limits.max_work_bytes = ledger.work_limit;
    limits.max_sql_vm_steps = ledger.sql_vm_limit;
    // Forecast both temporary source selector and bounded generated path before cloning/formatting.
    let builder = selected_paths_size(&request.source_paths)?
        .checked_add(
            isolation
                .root()
                .as_os_str()
                .len()
                .checked_add(96)
                .ok_or("Core lazy path bound")?,
        )
        .and_then(|n| {
            n.checked_add(96 + std::mem::size_of::<PathBuf>() + std::mem::size_of::<String>())
        })
        .ok_or("Core lazy carrier construction locals")?;
    let _builder = ledger.reserve(builder)?;
    let stem = format!("tos-core-lazy-carrier-{generation}");
    let path = fresh(isolation.root(), &stem)?;
    let paths = source_paths(&request.source_paths);
    let remaining = |additional| {
        ledger
            .remaining(additional)
            .map_err(tos_compiler::Error::Invalid)
    };
    let _constructor_locals = ledger.reserve(
        std::mem::size_of::<tos_compiler::RuntimeCaptureCreationUsage>()
            + std::mem::size_of::<tos_compiler::RuntimeCaptureOwnedBudget<'_>>()
            + std::mem::size_of::<tos_compiler::RuntimeCaptureProfile>()
            + std::mem::size_of::<Result<tos_compiler::PublicCapture>>()
            + std::mem::size_of_val(&remaining)
            + 4 * std::mem::size_of::<usize>(),
    )?;
    let mut usage = tos_compiler::RuntimeCaptureCreationUsage::default();
    let budget = tos_compiler::RuntimeCaptureOwnedBudget {
        remaining_after_retained: &remaining,
        original_work: Arc::clone(&ledger.work),
        original_work_limit: ledger.work_limit,
        creation_work_allowance: ledger.remaining_work()?,
        creation_deadline: deadline,
        original_sql_vm: Arc::clone(&ledger.sql_vm),
        original_sql_vm_limit: ledger.sql_vm_limit,
        original_sqlite_heap: Arc::clone(sqlite_heap),
        max_creation_json_visits: ledger.remaining_visits()?,
    };
    let result = tos_compiler::PublicCapture::create_runtime_selected_with_owned_budget(
        root,
        &paths,
        profile,
        &path,
        limits,
        owner_deadline,
        Arc::clone(cancelled),
        budget,
        &mut usage,
    );
    // Settle actual original usage on both outcomes before any possible disclosure.
    ledger.charge_visits(usage.json_visits)?;
    let capture = result.map_err(|_| "Core lazy owned carrier creation refused")?;
    let retained = capture
        .retained_state_upper_bound()
        .map_err(|_| "Core lazy selected capture census")?;
    ledger.remaining(retained)?;
    Ok(capture)
}

fn create_carrier(
    root: &Path,
    request: &Request,
    isolation: &tos_compiler::private_tmpfs_stage::PrivateTmpfsStageIsolation,
    role: tos_compiler::RuntimeCaptureRole,
    generation: u64,
    ledger: &Ledger,
    owner_deadline: Instant,
    deadline: Instant,
    cancelled: &Arc<AtomicBool>,
    sqlite_heap: &Arc<tos_compiler::DedicatedSessionSqliteHeap>,
) -> Result<Carrier> {
    let capture = create_selected_capture(
        root,
        request,
        isolation,
        tos_compiler::RuntimeCaptureProfile::Carrier(role),
        generation,
        ledger,
        owner_deadline,
        deadline,
        cancelled,
        sqlite_heap,
    )?;
    let retained = capture
        .retained_state_upper_bound()
        .map_err(|_| "Core lazy carrier census")?;
    Ok(Carrier {
        capture,
        role,
        retained,
    })
}

/// Promote once inside the existing lazy owner. The callback owns the full
/// remaining synchronous Driver lifetime; no model/state loan escapes it.
fn with_whole_selected(
    root: &Path,
    request: &Request,
    isolation: &tos_compiler::private_tmpfs_stage::PrivateTmpfsStageIsolation,
    generation: u64,
    ledger: &Ledger,
    owner_deadline: Instant,
    deadline: Instant,
    cancelled: &Arc<AtomicBool>,
    sqlite_heap: &Arc<tos_compiler::DedicatedSessionSqliteHeap>,
    consume: impl for<'scope, 'budget> FnOnce(
        &'scope tos_compiler::native_snapshot::CompletedNativeSnapshot,
        &'scope tos_compiler::native_snapshot::NativeKnowledgeSnapshot,
        tos_compiler::native_snapshot::NativeSnapshotOwnedReadLoan<'scope, 'budget>,
    ) -> tos_compiler::Result<()>,
) -> Result<()> {
    active(deadline)?;
    let capture = create_selected_capture(
        root,
        request,
        isolation,
        tos_compiler::RuntimeCaptureProfile::Whole,
        generation,
        ledger,
        owner_deadline,
        deadline,
        cancelled,
        sqlite_heap,
    )?;
    let retained = capture
        .retained_state_upper_bound()
        .map_err(|_| "Core Whole capture retained state")?;
    let _capture = ledger.reserve(retained)?;
    let candidate_workspace = isolation
        .root()
        .as_os_str()
        .len()
        .checked_add(128)
        .and_then(|n| n.checked_add(std::mem::size_of::<PathBuf>() + std::mem::size_of::<String>()))
        .ok_or("Core Whole candidate path state")?;
    let _candidate_workspace = ledger.reserve(candidate_workspace)?;
    let stem = format!("tos-core-whole-{generation}");
    let candidate = fresh(isolation.root(), &stem)?;
    let remaining = |bytes| {
        ledger
            .remaining(bytes)
            .map_err(tos_compiler::Error::Invalid)
    };
    let mut usage = tos_compiler::native_snapshot::NativeSnapshotCreationUsage::default();
    let limits = tos_compiler::native_snapshot_manifest::portable_native_snapshot_limits(
        request.admission.max_build_seconds,
    )
    .map_err(|_| "Core Whole native limits")?;
    let budget = tos_compiler::native_snapshot::NativeSnapshotOwnedBudget {
        remaining_after_retained: &remaining,
        original_sqlite_heap: sqlite_heap,
        max_creation_json_visits: ledger.remaining_visits()?,
        creation_deadline: owner_deadline,
    };
    let run = || {
        tos_compiler::native_snapshot::with_native_knowledge_snapshot_from_capture_with_owned_budget_and_layout_for_session(
        &capture, &candidate,
        tos_compiler::native_snapshot_manifest::RUNTIME_DATA_DECLARATION,
        isolation, limits, request.admission.whole().map_err(tos_compiler::Error::Invalid)?,
        false, false, owner_deadline, cancelled.as_ref(), deadline, budget, &mut usage,
        tos_compiler::knowledge_stage::KnowledgePayloadLayout::CarrierOnceV1,
        consume,
    )
    };
    let local_bytes = std::mem::size_of_val(&run)
        .checked_add(std::mem::size_of_val(&remaining))
        .and_then(|n| {
            n.checked_add(
                std::mem::size_of::<tos_compiler::NativeSnapshotCreationUsage>()
                    + std::mem::size_of::<tos_compiler::NativeSnapshotOwnedBudget<'_>>()
                    + std::mem::size_of::<tos_compiler::native_snapshot::NativeSnapshotLimits>()
                    + std::mem::size_of::<tos_compiler::Result<()>>(),
            )
        })
        .ok_or("Core Whole original caller frame")?;
    let _local = ledger.reserve(local_bytes)?;
    let result = run();
    // Settle shared aggregate JSON even when builder or reader refuses.
    ledger.charge_visits(usage.json_visits)?;
    result.map_err(|_| "Core Whole same-state writer/reader refused")?;
    active(owner_deadline)
}

struct Decimal {
    bytes: [u8; 20],
    start: usize,
}
impl Decimal {
    fn new(mut n: u64) -> Self {
        let mut result = Self {
            bytes: [0; 20],
            start: 20,
        };
        loop {
            result.start -= 1;
            result.bytes[result.start] = b'0' + (n % 10) as u8;
            n /= 10;
            if n == 0 {
                break;
            }
        }
        result
    }
    fn bytes(&self) -> &[u8] {
        &self.bytes[self.start..]
    }
}
const RESULT_PREFIX:&[u8]=br#"{"schema_version":"tos_native_core_snapshot_result_v1","ok":true,"selection":{"schema_version":"tos_native_core_selected_profile_v1","generation":"#;
const PROFILE_PREFIX: &[u8] = br#", "profile":""#;
const RESULT_SUFFIX:&[u8]=br#"","source_revision":null,"data_revision":null,"exploration_revision":null,"state_reused":false},"result":"#;
fn partial_envelope_len(generation: u64, profile: &str) -> Result<usize> {
    [
        RESULT_PREFIX.len(),
        Decimal::new(generation).bytes().len(),
        PROFILE_PREFIX.len(),
        profile.len(),
        RESULT_SUFFIX.len(),
        1,
    ]
    .into_iter()
    .try_fold(0usize, |n, x| {
        n.checked_add(x).ok_or("Core lazy envelope size overflow")
    })
}
fn send_partial_result(
    ledger: &Ledger,
    reply: &mut session_transport::Reply<'_, '_>,
    sequence: u64,
    generation: u64,
    profile: &'static str,
    body: &[u8],
) -> Result<()> {
    let _local = ledger.reserve(
        std::mem::size_of::<Decimal>()
            + std::mem::size_of::<[&[u8]; 8]>()
            + std::mem::size_of::<(&Ledger, &str, &[u8], u64, u64)>(),
    )?;
    let generation = Decimal::new(generation);
    reply.send(
        session_transport::REPLY,
        sequence,
        &[
            RESULT_PREFIX,
            generation.bytes(),
            PROFILE_PREFIX,
            profile.as_bytes(),
            RESULT_SUFFIX,
            body,
            b"}",
        ],
    )
}

const STORE_PROFILE: &[u8] = br#", "profile":"weak_query_store","source_revision":null,"data_revision":null,"exploration_revision":"#;
const STORE_SUFFIX: &[u8] = br#", "state_reused":false},"result":"#;
fn store_envelope_len(generation: u64, store: &lazy_store::Store) -> Result<usize> {
    [
        RESULT_PREFIX.len(),
        Decimal::new(generation).bytes().len(),
        STORE_PROFILE.len(),
        store.exploration_revision_json.len(),
        STORE_SUFFIX.len(),
        1,
    ]
    .into_iter()
    .try_fold(0usize, |n, x| {
        n.checked_add(x).ok_or("Core Store envelope geometry")
    })
}
fn send_store_result(
    ledger: &Ledger,
    reply: &mut session_transport::Reply<'_, '_>,
    sequence: u64,
    generation: u64,
    store: &lazy_store::Store,
    body: &[u8],
) -> Result<()> {
    let _locals =
        ledger.reserve(std::mem::size_of::<Decimal>() + std::mem::size_of::<[&[u8]; 7]>())?;
    let generation = Decimal::new(generation);
    reply.send(
        session_transport::REPLY,
        sequence,
        &[
            RESULT_PREFIX,
            generation.bytes(),
            STORE_PROFILE,
            &store.exploration_revision_json,
            STORE_SUFFIX,
            body,
            b"}",
        ],
    )
}

fn serve_carrier(
    root: &Path,
    request: &Request,
    isolation: &tos_compiler::private_tmpfs_stage::PrivateTmpfsStageIsolation,
    session: &session_owner::Session,
    driver: &mut session_transport::Driver<'_, '_>,
    held: &mut Option<Carrier>,
    generation: &mut u64,
    state: &Ledger,
    operation: &Operation,
    owner_deadline: Instant,
    cutoff: Instant,
    cancelled: &Arc<AtomicBool>,
    fence: &dyn Fn() -> Result<()>,
    sqlite_heap: &Arc<tos_compiler::DedicatedSessionSqliteHeap>,
) -> Result<()> {
    let (which, role, profile) =
        carrier(operation).ok_or("Core lazy operation outside joined profile")?;
    let _locals = state.reserve(
        std::mem::size_of::<(u64, Instant, &Request, &Ledger)>()
            + std::mem::size_of::<tos_compiler::native_snapshot_carriers::CapturedCarrierRequest>()
            + std::mem::size_of::<tos_compiler::RuntimeCaptureRole>()
            + std::mem::size_of::<Option<Carrier>>()
            + std::mem::size_of::<tos_compiler::native_snapshot::CompletedCaptureCarriers<'_>>()
            + std::mem::size_of::<&str>(),
    )?;
    if !held.as_ref().is_some_and(|c| c.role == role) {
        if let Some(old) = held.take() {
            state.retained.set(
                state
                    .retained
                    .get()
                    .checked_sub(old.retained)
                    .ok_or("Core lazy retained owner removal")?,
            );
            drop(old); // No original work/VM/JSON or Stage quota refund.
        }
        let next = generation
            .checked_add(1)
            .ok_or("Core lazy generation overflow")?;
        let created = create_carrier(
            root,
            request,
            isolation,
            role,
            next,
            state,
            owner_deadline,
            cutoff,
            cancelled,
            sqlite_heap,
        )?;
        state.retained.set(
            state
                .retained
                .get()
                .checked_add(created.retained)
                .ok_or("Core lazy held carrier state")?,
        );
        state.remaining(0)?;
        *held = Some(created);
        *generation = next; // Only after authentic owner construction succeeds.
    }
    let c = held.as_ref().ok_or("Core lazy carrier absent")?;
    c.capture.with_captured_carriers(|view| {
        let call=|sequence, _: &[u8], reply: &mut session_transport::Reply<'_, '_>| {
            reply.narrow_deadline(cutoff)?;
            let mut json=request.admission.json.limits()?;
            json.max_visits=json.max_visits.min(state.remaining_visits()?);
            let budget=tos_compiler::native_snapshot_carriers::CapturedCarrierReadBudget {
                max_rows:state.rows.get(),max_input_bytes:state.input_bytes.get(),
                max_output_bytes:request.admission.whole_max_graph_bytes.min(
                    session.limits.max_reply_bytes.checked_sub(partial_envelope_len(*generation,profile)?)
                    .ok_or("Core lazy original reply envelope cap")?),json,
            };
            let mut usage=tos_compiler::native_snapshot_carriers::CapturedCarrierUsage::default();
            let remaining=|bytes|state.remaining(bytes).map_err(tos_compiler::Error::Invalid);
            let _reader_locals=state.reserve(
                std::mem::size_of::<tos_compiler::native_snapshot_carriers::CapturedCarrierReadBudget>()
                +std::mem::size_of::<tos_compiler::native_snapshot_carriers::CapturedCarrierUsage>()
                +std::mem::size_of::<tos_compiler::native_snapshot_carriers::CapturedCarrierDelivery>()
                +std::mem::size_of_val(&remaining)
                +std::mem::size_of::<(u64,Instant,&Request,&Ledger)>())?;
            let result=tos_compiler::native_snapshot_carriers::read_complete_captured_carrier_with_state(
                view,which,budget,cutoff,cancelled.as_ref(),&remaining,&mut usage);
            state.debit_carrier_usage(usage)?; // BOTH outcomes before unwrap/disclosure.
            let delivery=result.map_err(|_|"Core lazy complete carrier refused")?;
            let _body=state.reserve(delivery.bytes.capacity())?;
            view.verify_current().map_err(|_|"Core lazy carrier pre-disclosure fence")?;
            fence()?;
            send_partial_result(state,reply,sequence,*generation,profile,&delivery.bytes)?;
            view.verify_current().map_err(|_|"Core lazy carrier post-disclosure fence")?;
            fence()?;
            active(cutoff)
        };
        let _callback=state.reserve(std::mem::size_of_val(&call))
            .map_err(tos_compiler::Error::Invalid)?;
        driver.respond(call,fence).map_err(tos_compiler::Error::Invalid)
    }).map_err(|_|"Core lazy held carrier call refused")
}

const WHOLE_PROFILE: &[u8] = br#", "profile":"whole_root","source_revision":""#;
const WHOLE_SUFFIX: &[u8] =
    br#"","data_revision":null,"exploration_revision":null,"state_reused":false},"result":"#;
fn whole_envelope_len(generation: u64, revision: &str) -> Result<usize> {
    [
        RESULT_PREFIX.len(),
        Decimal::new(generation).bytes().len(),
        WHOLE_PROFILE.len(),
        revision.len(),
        WHOLE_SUFFIX.len(),
        1,
    ]
    .into_iter()
    .try_fold(0usize, |sum, n| {
        sum.checked_add(n)
            .ok_or("Core Whole result envelope geometry")
    })
}
fn send_whole_result(
    ledger: &Ledger,
    reply: &mut session_transport::Reply<'_, '_>,
    sequence: u64,
    generation: u64,
    revision: &str,
    body: &[u8],
) -> Result<()> {
    ledger.charge_work(revision.len() as u64)?;
    if revision.len() != 64
        || !revision
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("Core Whole source revision is not canonical SHA256");
    }
    let _frame = ledger.reserve(
        std::mem::size_of::<Decimal>()
            + std::mem::size_of::<[&[u8]; 7]>()
            + std::mem::size_of::<(&Ledger, &str, &[u8], u64, u64)>(),
    )?;
    let generation = Decimal::new(generation);
    reply.send(
        session_transport::REPLY,
        sequence,
        &[
            RESULT_PREFIX,
            generation.bytes(),
            WHOLE_PROFILE,
            revision.as_bytes(),
            WHOLE_SUFFIX,
            body,
            b"}",
        ],
    )
}

/// One genuine Whole SourceRoot Search call. All root/model/query/authority
/// owners remain inside synchronous scopes; the cold DB closes before return.
fn serve_whole_indexed_search(
    root: &Path,
    request: &Request,
    isolation: &tos_compiler::private_tmpfs_stage::PrivateTmpfsStageIsolation,
    session: &session_owner::Session,
    driver: &mut session_transport::Driver<'_, '_>,
    carrier: &mut Option<Carrier>,
    store: &mut Option<lazy_store::Store>,
    generation: &mut u64,
    ledger: &Ledger,
    heap: &Arc<tos_compiler::DedicatedSessionSqliteHeap>,
    resources: &dyn tos_compiler::native_snapshot::NativeColdOpenResourceHold,
    owner_deadline: Instant,
    cutoff: Instant,
    cancelled: &Arc<AtomicBool>,
    fence: &dyn Fn() -> Result<()>,
    arguments: &tos_foundation::JsonValue,
) -> Result<()> {
    fence()?;
    // Drop only previously held selected owners; no work/VM/JSON clock refund.
    if let Some(old) = carrier.take() {
        ledger.retained.set(
            ledger
                .retained
                .get()
                .checked_sub(old.retained)
                .ok_or("Core Whole prior carrier retained removal")?,
        );
        drop(old);
    }
    if let Some(old) = store.take() {
        ledger.retained.set(
            ledger
                .retained
                .get()
                .checked_sub(old.retained)
                .ok_or("Core Whole prior Store retained removal")?,
        );
        drop(old);
    }
    let next = generation
        .checked_add(1)
        .ok_or("Core Whole generation overflow")?;
    let _locals = ledger.reserve(
        std::mem::size_of::<u64>()
            + std::mem::size_of::<tos_query::IndexedPageBudget>()
            + std::mem::size_of::<(&Path, &Request, &Ledger, Instant, Instant)>(),
    )?;
    let http = request
        .http
        .as_ref()
        .ok_or("Core Whole HTTP profile absent")?;
    let mut budget = http.indexed.native()?;
    let cursor = match checked_field(arguments, "cursor", ledger, cutoff)? {
        None | Some(tos_foundation::JsonValue::Null) => None,
        Some(value) => Some(value.as_str().ok_or("Core Whole cursor type")?),
    };
    with_whole_selected(
        root,
        request,
        isolation,
        next,
        ledger,
        owner_deadline,
        cutoff,
        cancelled,
        heap,
        |completed, native, loan| {
            completed.with_controlled_selected_knowledge_model(&loan, isolation,
            request.admission.cold, request.admission.process,
            request.admission.working_ram_bytes, resources, cutoff,
            |model, vocabulary, descriptor, view| {
            tos_query::with_controlled_knowledge_binding(model, vocabulary, descriptor,
                |model, bound| {
                let revision = bound.require_source_revision()
                    .map_err(|_| tos_compiler::Error::Invalid("Core Whole query source revision absent"))?;
                if revision != native.source_revision || view.source_revision() != Some(revision) {
                    return Err(tos_compiler::Error::Invalid("Core Whole source associations differ"));
                }
                let overhead = whole_envelope_len(next, revision).map_err(tos_compiler::Error::Invalid)?;
                let cap = session.limits.max_reply_bytes.checked_sub(overhead)
                    .ok_or(tos_compiler::Error::Budget("Core Whole reply envelope cap"))?;
                budget.max_response_bytes = budget.max_response_bytes
                    .min(cap).min(request.admission.whole_max_graph_bytes);
                if budget.max_response_bytes == 0 {
                    return Err(tos_compiler::Error::Budget("Core Whole response cap"));
                }
                crate::reference_root_query::with_controlled_metadata_context(model,
                    bound, view, cutoff, cancelled,
                    |bytes| ledger.reserve(bytes).map_err(tos_compiler::Error::Invalid),
                    |model, context| {
                    context.with_operation(crate::reference_root_query::ReferenceMetadataOperation::IndexedSearch,
                        |authority| {
                        let current = || {
                            fence()?;
                            view.verify_current().map_err(|_| "Core Whole transport source fence")?;
                            active(cutoff)
                        };
                        driver.respond(|sequence, _, reply| {
                            reply.narrow_deadline(cutoff)?;
                            current()?;
                            tos_query::execute_scoped_controlled_indexed_search_response(
                                model, bound, authority, arguments, cursor, budget,
                                |initial, receipt| Ok(crate::indexed_cursor::NativeIndexedCursorCodec::new(initial, receipt)),
                                |body| {
                                    let map = |_| tos_query::search_v2::SearchV2Error {
                                        code: tos_query::search_v2::SearchV2ErrorCode::Unavailable,
                                        message: "Core Whole transport delivery refused",
                                    };
                                    current().map_err(map)?;
                                    send_whole_result(ledger, reply, sequence, next, revision, body).map_err(map)?;
                                    current().map_err(map)
                                },
                            ).map_err(|_| "Core Whole scoped Search refused")?;
                            current()
                        }, current).map_err(tos_compiler::Error::Invalid)
                    })
                })
            })
        })
        },
    )?;
    *generation = next;
    fence()
}

fn parse_whole_legacy_search_request<'ledger>(
    arguments: &tos_foundation::JsonValue,
    budget: &tos_query::knowledge_legacy_search::LegacySearchBudget,
    ledger: &'ledger Ledger,
    cutoff: Instant,
    cancelled: &AtomicBool,
) -> Result<(
    tos_query::knowledge_legacy_search::LegacySearchRequest,
    native_ledger::Reservation<'ledger>,
)> {
    let _frame = ledger.reserve(
        std::mem::size_of::<(
            &tos_foundation::JsonValue,
            &tos_query::knowledge_legacy_search::LegacySearchBudget,
            &Ledger,
            Instant,
            &AtomicBool,
            [Option<&tos_foundation::JsonValue>; 3],
            Option<&[tos_foundation::JsonValue]>,
            Result<(
                tos_query::knowledge_legacy_search::LegacySearchRequest,
                native_ledger::Reservation<'ledger>,
            )>,
            crate::search::SearchRequest,
            u64,
            u64,
            usize,
        )>()
        .checked_add(std::mem::size_of::<std::slice::Iter<'_, tos_foundation::JsonValue>>())
        .and_then(|n| n.checked_add(6 * std::mem::size_of::<usize>()))
        .ok_or("Core SourceRoot legacy Search parser frame")?,
    )?;
    let mode = checked_field(arguments, "mode", ledger, cutoff)?;
    if mode.is_some_and(|value| value.as_str() != Some("legacy")) {
        return Err("Core SourceRoot legacy Search mode refused");
    }
    let query = checked_field(arguments, "query", ledger, cutoff)?;
    let query_bytes = match query {
        None => 0,
        Some(value) => value
            .as_str()
            .ok_or("Core SourceRoot legacy Search query type")?
            .len(),
    };
    if query_bytes > budget.inspect.max_field_bytes {
        return Err("Core SourceRoot legacy Search query cap");
    }
    let census_frame = std::mem::size_of::<(
        &tos_foundation::JsonValue,
        usize,
        usize,
        Instant,
        &AtomicBool,
    )>() + 3 * std::mem::size_of::<usize>()
        + std::mem::size_of::<Result<usize>>()
        + std::mem::size_of::<std::slice::Iter<'_, tos_foundation::JsonValue>>()
        + std::mem::size_of::<
            std::slice::Iter<'_, (tos_foundation::JsonString, tos_foundation::JsonValue)>,
        >()
        + std::mem::size_of::<(&tos_foundation::JsonString, &tos_foundation::JsonValue)>();
    let tree_bytes = {
        let _census = ledger.reserve(
            census_frame
                .checked_mul(97)
                .ok_or("Core SourceRoot legacy Search census frame")?,
        )?;
        checked_document_storage(arguments, 0, 96, cutoff, cancelled)?
    };
    let mut input_bytes = u64::try_from(query_bytes)
        .map_err(|_| "Core SourceRoot legacy Search input size")?;
    let mut input_values = 0u64;
    let mut owned_text_bytes = query_bytes;
    for field in ["sources", "kind_ids", "predicate_ids"] {
        let Some(value) = checked_field(arguments, field, ledger, cutoff)? else {
            continue;
        };
        if value.is_null() {
            continue;
        }
        let values = value
            .as_array()
            .ok_or("Core SourceRoot legacy Search filter type")?;
        for value in values {
            active(cutoff)?;
            let text = value
                .as_str()
                .ok_or("Core SourceRoot legacy Search filter value")?;
            let length = u64::try_from(text.len())
                .map_err(|_| "Core SourceRoot legacy Search filter size")?;
            input_values = input_values
                .checked_add(1)
                .ok_or("Core SourceRoot legacy Search filter count")?;
            input_bytes = input_bytes
                .checked_add(length)
                .ok_or("Core SourceRoot legacy Search input size")?;
            owned_text_bytes = owned_text_bytes
                .checked_add(text.len())
                .ok_or("Core SourceRoot legacy Search retained text")?;
            if input_values > budget.inspect.max_rows
                || input_bytes > budget.inspect.max_decoded_bytes
                || text.len() > budget.inspect.max_field_bytes
            {
                return Err("Core SourceRoot legacy Search input cap");
            }
        }
    }
    let owned_slots = usize::try_from(input_values)
        .map_err(|_| "Core SourceRoot legacy Search slot count")?
        .checked_mul(std::mem::size_of::<String>())
        .ok_or("Core SourceRoot legacy Search slot bytes")?;
    let retained = tree_bytes
        .checked_add(std::mem::size_of::<crate::search::SearchRequest>())
        .and_then(|n| n.checked_add(owned_text_bytes))
        .and_then(|n| n.checked_add(owned_slots))
        .and_then(|n| n.checked_add(512))
        .ok_or("Core SourceRoot legacy Search retained forecast")?;
    ledger.charge_work(
        u64::try_from(tree_bytes)
            .ok()
            .and_then(|n| n.checked_add(u64::try_from(owned_text_bytes).ok()?))
            .ok_or("Core SourceRoot legacy Search copy work")?,
    )?;
    active(cutoff)?;
    let hold = ledger.reserve(retained)?;
    let parsed = crate::search::SearchRequest::from_arguments(arguments)
        .map_err(|_| "Core SourceRoot legacy Search arguments refused")?;
    let request = match parsed {
        crate::search::SearchRequest::Legacy(request) => request,
        _ => return Err("Core SourceRoot only admits legacy Search arguments"),
    };
    Ok((request, hold))
}

/// Execute the maintained offset/limit legacy contract against the Whole
/// SourceRoot owner. The body and observed-carrier lease stay inside Reply.
fn serve_whole_legacy_search(
    root: &Path,
    request: &Request,
    isolation: &tos_compiler::private_tmpfs_stage::PrivateTmpfsStageIsolation,
    session: &session_owner::Session,
    driver: &mut session_transport::Driver<'_, '_>,
    carrier: &mut Option<Carrier>,
    store: &mut Option<lazy_store::Store>,
    generation: &mut u64,
    ledger: &Ledger,
    heap: &Arc<tos_compiler::DedicatedSessionSqliteHeap>,
    resources: &dyn tos_compiler::native_snapshot::NativeColdOpenResourceHold,
    owner_deadline: Instant,
    cutoff: Instant,
    cancelled: &Arc<AtomicBool>,
    fence: &dyn Fn() -> Result<()>,
    arguments: &tos_foundation::JsonValue,
) -> Result<()> {
    fence()?;
    // The prior owner is released without refunding any original counter.
    if let Some(old) = carrier.take() {
        ledger.retained.set(
            ledger
                .retained
                .get()
                .checked_sub(old.retained)
                .ok_or("Core Whole prior carrier retained removal")?,
        );
        drop(old);
    }
    if let Some(old) = store.take() {
        ledger.retained.set(
            ledger
                .retained
                .get()
                .checked_sub(old.retained)
                .ok_or("Core Whole prior Store retained removal")?,
        );
        drop(old);
    }
    let next = generation
        .checked_add(1)
        .ok_or("Core Whole generation overflow")?;
    let _locals = ledger.reserve(
        std::mem::size_of::<tos_query::knowledge_legacy_search::LegacySearchBudget>()
            + std::mem::size_of::<(&Path, &Request, &Ledger, Instant, Instant)>(),
    )?;
    let http = request
        .http
        .as_ref()
        .ok_or("Core Whole HTTP profile absent")?;
    let initial_budget = http.legacy.native()?;
    with_whole_selected(
        root,
        request,
        isolation,
        next,
        ledger,
        owner_deadline,
        cutoff,
        cancelled,
        heap,
        |completed, native, loan| {
            completed.with_controlled_selected_knowledge_model(
                &loan,
                isolation,
                request.admission.cold,
                request.admission.process,
                request.admission.working_ram_bytes,
                resources,
                cutoff,
                |model, vocabulary, descriptor, view| {
                    tos_query::with_controlled_knowledge_binding(
                        model,
                        vocabulary,
                        descriptor,
                        |model, bound| {
                            let revision = bound.require_source_revision().map_err(|_| {
                                tos_compiler::Error::Invalid(
                                    "Core Whole legacy source revision absent",
                                )
                            })?;
                            if revision != native.source_revision
                                || view.source_revision() != Some(revision)
                            {
                                return Err(tos_compiler::Error::Invalid(
                                    "Core Whole legacy source associations differ",
                                ));
                            }
                            let mut budget = initial_budget;
                            let overhead = whole_envelope_len(next, revision)
                                .map_err(tos_compiler::Error::Invalid)?;
                            let cap = session
                                .limits
                                .max_reply_bytes
                                .checked_sub(overhead)
                                .ok_or(tos_compiler::Error::Budget(
                                    "Core Whole legacy reply envelope cap",
                                ))?;
                            budget.inspect.max_response_bytes = budget
                                .inspect
                                .max_response_bytes
                                .min(cap)
                                .min(request.admission.whole_max_graph_bytes);
                            if budget.inspect.max_response_bytes == 0 {
                                return Err(tos_compiler::Error::Budget(
                                    "Core Whole legacy response cap",
                                ));
                            }
                            let (legacy_request, _legacy_request_state) =
                                parse_whole_legacy_search_request(
                                    arguments,
                                    &budget,
                                    ledger,
                                    cutoff,
                                    cancelled.as_ref(),
                                )
                                .map_err(tos_compiler::Error::Invalid)?;
                            crate::reference_root_query::with_controlled_metadata_context(
                                model,
                                bound,
                                view,
                                cutoff,
                                cancelled,
                                |bytes| {
                                    ledger
                                        .reserve(bytes)
                                        .map_err(tos_compiler::Error::Invalid)
                                },
                                |model, context| {
                                    context.with_operation(
                                        crate::reference_root_query::ReferenceMetadataOperation::LegacySearch,
                                        |authority| {
                                            let current = || {
                                                fence()?;
                                                view.verify_current().map_err(|_| {
                                                    "Core Whole legacy source fence"
                                                })?;
                                                active(cutoff)
                                            };
                                            driver
                                                .respond(
                                                    |sequence, _, reply| {
                                                        reply.narrow_deadline(cutoff)?;
                                                        current()?;
                                                        let packet =
                                                            tos_query::knowledge_legacy_search::execute_controlled_legacy_search(
                                                                model,
                                                                bound,
                                                                authority,
                                                                &legacy_request,
                                                                budget,
                                                            )
                                                            .map_err(|_| {
                                                                "Core Whole legacy Search refused"
                                                            })?;
                                                        let (body, mut disclosure) =
                                                            packet.into_parts();
                                                        let _body = ledger.reserve(
                                                            body.capacity()
                                                                .checked_add(std::mem::size_of::<Vec<u8>>())
                                                                .ok_or("Core Whole legacy body state")?,
                                                        )?;
                                                        if body.len()
                                                            > budget.inspect.max_response_bytes
                                                        {
                                                            return Err(
                                                                "Core Whole legacy response cap",
                                                            );
                                                        }
                                                        current()?;
                                                        send_whole_result(
                                                            ledger,
                                                            reply,
                                                            sequence,
                                                            next,
                                                            revision,
                                                            &body,
                                                        )?;
                                                        disclosure
                                                            .recheck()
                                                            .map_err(|_| "Core Whole legacy disclosure fence")?;
                                                        current()
                                                    },
                                                    current,
                                                )
                                                .map_err(tos_compiler::Error::Invalid)
                                        },
                                    )
                                },
                            )
                        },
                    )
                },
            )
        },
    )?;
    *generation = next;
    fence()
}

fn serve_store(
    request: &Request,
    session: &session_owner::Session,
    driver: &mut session_transport::Driver<'_, '_>,
    carrier: &mut Option<Carrier>,
    held: &mut Option<lazy_store::Store>,
    generation: &mut u64,
    ledger: &Ledger,
    heap: &Arc<tos_compiler::DedicatedSessionSqliteHeap>,
    owner_deadline: Instant,
    deadline: Instant,
    cancelled: &Arc<AtomicBool>,
    fence: &dyn Fn() -> Result<()>,
    search: Option<&tos_foundation::JsonValue>,
) -> Result<()> {
    if let Some(old) = carrier.take() {
        ledger.retained.set(
            ledger
                .retained
                .get()
                .checked_sub(old.retained)
                .ok_or("Core lazy carrier to Store retained removal")?,
        );
        drop(old); // No work/VM/JSON/Stage quota refund; one backend pool persists.
    }
    if held.is_none() {
        let next = generation
            .checked_add(1)
            .ok_or("Core Store generation overflow")?;
        let created = lazy_store::open(request, ledger, heap, owner_deadline, deadline, cancelled)?;
        ledger.retained.set(
            ledger
                .retained
                .get()
                .checked_add(created.retained)
                .ok_or("Core Store retained insertion")?,
        );
        ledger.remaining(0)?;
        *held = Some(created);
        *generation = next;
    }
    let store = held.as_mut().ok_or("Core selected Store owner absent")?;
    let call = |sequence, _: &[u8], reply: &mut session_transport::Reply<'_, '_>| {
        reply.narrow_deadline(deadline)?;
        let cap = request.admission.whole_max_graph_bytes.min(
            session
                .limits
                .max_reply_bytes
                .checked_sub(store_envelope_len(*generation, store)?)
                .ok_or("Core Store original result envelope cap")?,
        );
        let body = if let Some(arguments) = search {
            store.search(arguments, request, ledger, heap, deadline, cancelled, cap)?
        } else {
            store.metadata(
                "tos_corpus_header",
                request,
                ledger,
                heap,
                deadline,
                cancelled,
                cap,
            )?
        };
        let _body = ledger.reserve(
            body.capacity()
                .checked_add(std::mem::size_of::<Vec<u8>>())
                .ok_or("Core Store final body census")?,
        )?;
        store.fence(request, ledger, heap)?;
        fence()?;
        send_store_result(ledger, reply, sequence, *generation, store, &body)?;
        store.fence(request, ledger, heap)?;
        fence()?;
        active(deadline)
    };
    let _locals = ledger.reserve(
        std::mem::size_of_val(&call)
            + std::mem::size_of::<(u64, Instant, usize, &Request, &Ledger)>(),
    )?;
    driver.respond(call, fence)
}

pub(super) fn run(
    root: &Path,
    session: &session_owner::Session,
    request: &Request,
    deadline: Instant,
    cancelled: &Arc<AtomicBool>,
    startup_bytes: usize,
    ordinary: bool,
) -> Result<()> {
    if request.admission.cold.max_work_bytes == 0 {
        return Err("Core probe original work allowance absent");
    }
    let capture_limits = tos_compiler::native_snapshot_manifest::portable_native_snapshot_limits(
        request.admission.max_build_seconds,
    )
    .map_err(|_| "Core lazy original capture limits")?
    .capture;
    let baseline = request
        .retained_resource_state_upper_bound()?
        .checked_add(
            std::mem::size_of::<Ledger>()
                + std::mem::size_of::<Workspace>()
                + std::mem::size_of::<Option<Carrier>>()
                + std::mem::size_of::<Option<lazy_store::Store>>()
                + std::mem::size_of::<u64>()
                + 4 * (2 * std::mem::size_of::<usize>()
                    + std::mem::size_of::<std::sync::atomic::AtomicU64>())
                + 2 * std::mem::size_of::<usize>()
                + std::mem::size_of::<std::sync::atomic::AtomicUsize>(),
        )
        .ok_or("Core lazy original fixed state")?;
    if baseline > request.admission.whole_max_state_bytes
        || session.startup_visits > request.admission.json.max_visits
    {
        return Err("Core lazy original baseline admission");
    }
    let state = Ledger {
        retained: std::cell::Cell::new(baseline),
        state_limit: request.admission.whole_max_state_bytes,
        reserved: std::cell::Cell::new(0),
        work: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        // Whole lifetime uses the existing portable capture owner ceiling.
        // Cold limits narrow its own operation using the same original ledger.
        work_limit: capture_limits.max_work_bytes,
        sql_vm: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        sql_vm_limit: capture_limits.max_sql_vm_steps,
        store_steps: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        store_sql_vm: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        visits: Arc::new(std::sync::atomic::AtomicUsize::new(session.startup_visits)),
        visit_limit: request.admission.json.max_visits,
        rows: std::cell::Cell::new(request.admission.whole_max_rows),
        input_bytes: std::cell::Cell::new(capture_limits.max_input_bytes),
    };
    state.charge_work(startup_bytes as u64)?;
    // QRY owned refusals have a bounded formatter/string workspace, admitted
    // once before any Store owner is allowed to allocate, including failure.
    let _store_diagnostics =
        state.reserve(tos_query::source_diagnostic::owned_store_diagnostic_workspace_bytes())?;
    // SourceStore process VFS Box is Rust heap, distinct from the SQLite
    // backend pool and each held connection. Admit it once for this process.
    let _source_store_process = state
        .reserve(tos_source_store::PinnedSqliteConnection::process_rust_state_upper_bound())?;
    // Existing Stage owner declares its parsing/verification forecasts. Debit BEFORE
    // execution, even when selection refuses; this never opens or invents a model.
    let setup = state.reserve(PRIVATE_TMPFS_SELECT_COST.workspace_bytes)?;
    state.charge_work(PRIVATE_TMPFS_SELECT_COST.read_bytes)?;
    let isolation =
        tos_compiler::private_tmpfs_stage::PrivateTmpfsStageIsolation::select_from_environment(
            request.admission.tmpfs_quota_bytes,
            request.admission.inode_limit,
            request.admission.working_ram_bytes,
        )
        .map_err(|_| "Core probe actual private Stage refused")?;
    drop(setup);
    // Owner's existing post-selection upper bound covers Ticket heap and held FDs.
    state.retained.set(
        state
            .retained
            .get()
            .checked_add(
                isolation
                    .retained_state_upper_bound()
                    .map_err(|_| "Core probe Stage retained state")?,
            )
            .ok_or("Core probe Stage retained state overflow")?,
    );
    state.remaining(0)?;
    request
        .admission
        .process
        .verify_current()
        .map_err(|_| "Core probe actual process envelope")?;
    // Same actual Linux RAM/swap custody used by the existing Root owner. Its
    // bounded helper uses two <=8194-byte path arrays/membership buffers and <=64-byte
    // scalar observations. This local conservative forecast remains ORIGINAL state/work.
    const KERNEL_WORKSPACE: usize = 64 * 1024;
    const KERNEL_READ_WORK: u64 = 2 * 8194 + 16 * 64;
    let kernel_setup = state.reserve(KERNEL_WORKSPACE)?;
    state.charge_work(KERNEL_READ_WORK)?;
    let resources = crate::native_cold_resources::LinuxCgroupColdOpenResourceHold::acquire(
        request.admission.working_ram_bytes,
        deadline,
        cancelled.clone(),
    )
    .map_err(|_| "Core probe genuine kernel RAM custody")?;
    drop(kernel_setup);
    state.retained.set(
        state
            .retained
            .get()
            .checked_add(
                resources
                    .retained_state_upper_bound()
                    .map_err(|_| "Core probe kernel custody retained state")?,
            )
            .ok_or("Core probe kernel state overflow")?,
    );
    state.remaining(0)?;
    // Dedicated native process only. One finite HALF-original-state backend
    // suballocation, never widened or renewed by a selected-profile transition.
    let heap_bytes = tos_compiler::dedicated_session_heap_bytes(state.state_limit)
        .map_err(|_| "Core lazy original SQLite pool profile")?;
    let heap_remaining = |bytes| state.remaining(bytes).map_err(tos_compiler::Error::Invalid);
    let _heap_locals = state.reserve(
        std::mem::size_of_val(&heap_remaining)
            + std::mem::size_of::<Arc<tos_compiler::DedicatedSessionSqliteHeap>>()
            + std::mem::size_of::<usize>(),
    )?;
    let sqlite_heap = tos_compiler::DedicatedSessionSqliteHeap::establish(
        heap_bytes,
        &heap_remaining,
        deadline,
        cancelled.as_ref(),
    )
    .map_err(|_| "Core lazy dedicated SQLite backend admission")?;
    state.retained.set(
        state
            .retained
            .get()
            .checked_add(sqlite_heap.reserved_state_bytes())
            .ok_or("Core lazy SQLite reserved state overflow")?,
    );
    state.remaining(0)?;
    let mut workspace = Workspace(&state);
    let fence = || {
        active(deadline)?;
        session
            .control
            .verify_current()
            .map_err(|_| "Core probe issued control fence")?;
        request
            .admission
            .process
            .verify_current()
            .map_err(|_| "Core probe current process fence")?;
        let held = state.reserve(PRIVATE_TMPFS_VERIFY_COST.workspace_bytes)?;
        state.charge_work(PRIVATE_TMPFS_VERIFY_COST.read_bytes)?;
        isolation
            .quota_usage()
            .map_err(|_| "Core probe live private Stage fence")?;
        drop(held);
        let kernel = state.reserve(KERNEL_WORKSPACE)?;
        state.charge_work(KERNEL_READ_WORK)?;
        resources
            .check_current(deadline, cancelled.as_ref())
            .map_err(|_| "Core probe current hard RAM/swap fence")?;
        drop(kernel);
        active(deadline)
    };
    let consume = |driver: &mut session_transport::Driver<'_, '_>| {
        // Shared connected capabilities retain the exact old lazy ABI. The
        // ordinary add-on belongs to the joined genuine legacy SourceRoot route.
        const LAZY_HEAD: &[u8] = br#"{"schema_version":"tos_native_core_lazy_session_ready_v1","ok":true,"profile":"tos_core_lazy_selected_v1""#;
        const ORDINARY_HEAD: &[u8] = br#"{"schema_version":"tos_native_core_ordinary_session_ready_v1","ok":true,"profile":"tos_core_ordinary_selected_v1""#;
        const COMMON: &[u8] = br#","reference_semantics":"cpython_pathlib_is_file_3_14","source_revision":null,"data_revision":null,"state_reused":false,"selection":{"schema_version":"tos_native_core_selected_profile_v1","generation":0,"profile":"selected_paths","source_revision":null,"data_revision":null,"exploration_revision":null,"state_reused":false},"capabilities":[{"operation":"tos_corpus_index_exists"},{"operation":"tos_philosophy_projection_exists"},{"operation":"tos_evidence_projection_exists"},{"operation":"tos_philosophy_audit_exists"},{"operation":"tos_corpus_index"},{"operation":"tos_bibliographic_graph"},{"operation":"tos_philosophy_projection"},{"operation":"tos_philosophy_audit_payload"},{"operation":"tos_corpus_header"},{"operation":"tos_native_call","tool":"tos_knowledge_search","profiles":["weak_query_store"]},{"operation":"tos_native_call","tool":"tos_knowledge_search_indexed_v2","profiles":["whole_root"]}"#;
        const LAZY_END: &[u8] = b"]}";
        const ORDINARY_END: &[u8] = br#",{"operation":"tos_native_call","tool":"tos_knowledge_search","profiles":["whole_root"]}]}"#;
        let ready = [if ordinary { ORDINARY_HEAD } else { LAZY_HEAD }, COMMON,
                     if ordinary { ORDINARY_END } else { LAZY_END }];
        // Exactly nine connected operations; CorpusHeader selects its authentic
        // configured/default Store owner before any Corpus carrier construction.
        if ready.iter().map(|segment| segment.len()).sum::<usize>()
            > request
                .http
                .as_ref()
                .ok_or("Core lazy profile absent")?
                .max_startup_receipt_bytes
        {
            return Err("Core lazy startup receipt cap");
        }
        driver.startup(
            |reply| reply.send(session_transport::STARTUP, 0, &ready),
            &fence,
        )?;
        let mut held: Option<Carrier> = None;
        let mut held_store: Option<lazy_store::Store> = None;
        let mut generation = 0u64;
        let _whole_flag = state.reserve(
            std::mem::size_of::<bool>() + std::mem::size_of::<native_ledger::Reservation<'_>>(),
        )?;
        let mut last_whole = false;
        loop {
            let more = {
                let owner_fence = || {
                    fence()?;
                    if let Some(c) = held.as_ref() {
                        c.capture
                            .with_captured_carriers(|view| view.verify_current())
                            .map_err(|_| "Core lazy retained carrier receive/close fence")?;
                    }
                    if let Some(store) = held_store.as_ref() {
                        store.fence(request, &state, &sqlite_heap)?;
                    }
                    Ok(())
                };
                driver.receive(owner_fence)?
            };
            if !more {
                break;
            }

            let _call_slots = state.reserve(std::mem::size_of::<(
                Option<Operation>,
                Option<tos_foundation::JsonDocument>,
                Instant,
                usize,
            )>())?;
            let (operation, search_document, cutoff, document_bytes) = {
                let (sequence, raw) = driver.pending_request()?;
                call(raw, sequence, request, &state, deadline, cancelled.as_ref())?
            };
            let cutoff = cutoff.min(deadline);
            if let Some(document) = search_document.as_ref() {
                let _document = state.reserve(document_bytes)?;
                let outer = checked_field(document.root(), "arguments", &state, cutoff)?
                    .ok_or("Core Store Search envelope arguments")?;
                let arguments = checked_field(outer, "arguments", &state, cutoff)?
                    .ok_or("Core Store Search tool arguments")?;
                let tool = checked_field(outer, "tool", &state, cutoff)?
                    .and_then(tos_foundation::JsonValue::as_str)
                    .ok_or("Core selected Search tool absent")?;
                if tool == "tos_knowledge_search_indexed_v2" {
                    with_selected_store_choice(request, &state, cutoff, |selected_store| {
                        if selected_store {
                            return Err("Core selected QueryStore indexed owner unavailable");
                        }
                        serve_whole_indexed_search(
                            root,
                            request,
                            &isolation,
                            session,
                            driver,
                            &mut held,
                            &mut held_store,
                            &mut generation,
                            &state,
                            &sqlite_heap,
                            &resources,
                            deadline,
                            cutoff,
                            cancelled,
                            &fence,
                            arguments,
                        )
                    })?;
                    last_whole = true;
                    continue;
                }
                let whole_source_root = with_selected_store_choice(request, &state, cutoff, |selected_store| {
                    if selected_store {
                        serve_store(
                            request,
                            session,
                            driver,
                            &mut held,
                            &mut held_store,
                            &mut generation,
                            &state,
                            &sqlite_heap,
                            deadline,
                            cutoff,
                            cancelled,
                            &fence,
                            Some(arguments),
                        )?;
                        return Ok(false);
                    }
                    if !ordinary {
                        return Err("Core SourceRoot legacy Search outside ordinary profile");
                    }
                    serve_whole_legacy_search(
                        root,
                        request,
                        &isolation,
                        session,
                        driver,
                        &mut held,
                        &mut held_store,
                        &mut generation,
                        &state,
                        &sqlite_heap,
                        &resources,
                        deadline,
                        cutoff,
                        cancelled,
                        &fence,
                        arguments,
                    )?;
                    Ok(true)
                })?;
                last_whole = whole_source_root;
                continue;
            }
            let operation = operation.ok_or("Core lazy selected operation absent")?;
            if let Operation::Exists(kind) = operation {
                if last_whole && held.is_none() && held_store.is_none() {
                    generation = generation
                        .checked_add(1)
                        .ok_or("Core Whole to selected-paths generation overflow")?;
                    last_whole = false;
                }
                let path = match kind {
                    ExistsKind::Index => &request.source_paths.index_path,
                    ExistsKind::Philosophy => {
                        &request.source_paths.philosophy_graph_projection_path
                    }
                    ExistsKind::Evidence => &request.source_paths.evidence_projection_path,
                    ExistsKind::PhilosophyAudit => {
                        &request.source_paths.philosophy_post_planting_audit_path
                    }
                };
                driver.respond(
                    |sequence, _, reply| {
                        reply.narrow_deadline(cutoff)?;
                        state.charge_work(
                            (path.as_os_str().len() as u64)
                                .checked_mul(4)
                                .ok_or("Core lazy selected-path work")?,
                        )?;
                        let disclose = |exists| {
                            fence()?;
                            if let Some(c) = held.as_ref() {
                                c.capture
                                    .with_captured_carriers(|view| view.verify_current())
                                    .map_err(|_| "Core lazy retained carrier changed")?;
                            }
                            let profile = held
                                .as_ref()
                                .map(|c| match c.role {
                                    tos_compiler::RuntimeCaptureRole::Corpus => "carrier_corpus",
                                    tos_compiler::RuntimeCaptureRole::Bibliographic => {
                                        "carrier_bibliographic"
                                    }
                                    tos_compiler::RuntimeCaptureRole::Philosophy => {
                                        "carrier_philosophy"
                                    }
                                    tos_compiler::RuntimeCaptureRole::PhilosophyAudit => {
                                        "carrier_philosophy_audit"
                                    }
                                })
                                .unwrap_or("selected_paths");
                            if let Some(store) = held_store.as_ref() {
                                store.fence(request, &state, &sqlite_heap)?;
                                send_store_result(
                                    &state,
                                    reply,
                                    sequence,
                                    generation,
                                    store,
                                    if exists { b"true" } else { b"false" },
                                )?;
                                store.fence(request, &state, &sqlite_heap)?;
                            } else {
                                send_partial_result(
                                    &state,
                                    reply,
                                    sequence,
                                    generation,
                                    profile,
                                    if exists { b"true" } else { b"false" },
                                )?;
                            }
                            fence()?;
                            if let Some(c) = held.as_ref() {
                                c.capture
                                    .with_captured_carriers(|view| view.verify_current())
                                    .map_err(
                                        |_| "Core lazy retained carrier changed after disclosure",
                                    )?;
                            }
                            active(cutoff)
                        };
                        let bytes = SELECTED_PROBE_METADATA_WORKSPACE
                            .checked_add(std::mem::size_of_val(&disclose))
                            .and_then(|n| n.checked_add(std::mem::size_of::<Result<()>>()))
                            .ok_or("Core lazy probe local state")?;
                        let _probe = state.reserve(bytes)?;
                        with_selected_is_file(path, cutoff, disclose)
                    },
                    &fence,
                )?;
                continue;
            }
            let execute_carrier = |driver: &mut session_transport::Driver<'_, '_>,
                                   held: &mut Option<Carrier>,
                                   held_store: &mut Option<lazy_store::Store>,
                                   generation: &mut u64| {
                if let Some(old) = held_store.take() {
                    state.retained.set(
                        state
                            .retained
                            .get()
                            .checked_sub(old.retained)
                            .ok_or("Core Store to carrier retained removal")?,
                    );
                    drop(old); // Does not renew any original counter/clock/backend allowance.
                }
                serve_carrier(
                    root,
                    request,
                    &isolation,
                    session,
                    driver,
                    held,
                    generation,
                    &state,
                    &operation,
                    deadline,
                    cutoff,
                    cancelled,
                    &fence,
                    &sqlite_heap,
                )
            };
            let _dispatch = state.reserve(std::mem::size_of_val(&execute_carrier))?;
            if matches!(operation, Operation::CorpusHeader) {
                with_selected_store_choice(request, &state, cutoff, |selected_store| {
                    if selected_store {
                        serve_store(
                            request,
                            session,
                            driver,
                            &mut held,
                            &mut held_store,
                            &mut generation,
                            &state,
                            &sqlite_heap,
                            deadline,
                            cutoff,
                            cancelled,
                            &fence,
                            None,
                        )
                    } else {
                        execute_carrier(driver, &mut held, &mut held_store, &mut generation)
                    }
                })?;
            } else {
                execute_carrier(driver, &mut held, &mut held_store, &mut generation)?;
            }
            last_whole = false;
        }
        // Native close ACK must succeed while all selected owners remain held.
        drop(held);
        drop(held_store);
        Ok(())
    };
    let _callbacks =
        state.reserve(std::mem::size_of_val(&fence) + std::mem::size_of_val(&consume))?;
    session_transport::with_driver(
        session.control.as_fd(),
        session.limits,
        deadline,
        cancelled.as_ref(),
        &mut workspace,
        consume,
    )
}
