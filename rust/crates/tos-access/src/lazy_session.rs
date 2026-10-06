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
    ordinary: bool,
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
        let tool = checked_field(arguments, "tool", state, deadline)?
            .and_then(tos_foundation::JsonValue::as_str);
        if !(matches!(tool, Some("tos_knowledge_search" | "tos_knowledge_search_indexed_v2"))
            || ordinary && matches!(tool, Some("tos_knowledge_catalog" | "tos_knowledge_node" | "tos_knowledge_relation" | "tos_philosophy_graph_status" | "tos_native_resource_read" | "tos_knowledge_lens_compile" | "tos_knowledge_focus" | "tos_knowledge_lens_open" | "tos_corpus_status" | "tos_corpus_summary" | "tos_corpus_graph_views" | "tos_philosophy_graph_layers" | "tos_philosophy_graph_snapshot" | "tos.snapshot" | "tos_corpus_search" | "tos_corpus_resources" | "tos_corpus_node" | "tos_corpus_relation_pack" | "tos_corpus_graph_view" | "tos_corpus_packet" | "tos_philosophy_graph_views" | "tos_philosophy_graph_view" | "tos.view.open" | "tos_access_health")))
            || checked_field(arguments, "arguments", state, deadline)?
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
    if id == "tos_source_navigation" && ordinary {
        let args = checked_field(root, "arguments", state, deadline)?
            .ok_or("Core navigation arguments absent")?;
        let fields = args.as_object().ok_or("Core navigation arguments object")?;
        if fields.len() != 1 {
            return Err("Core navigation strict boolean DTO");
        }
        let bibliographic_only = checked_field(args, "bibliographic_only", state, deadline)?
            .and_then(tos_foundation::JsonValue::as_bool)
            .ok_or("Core navigation strict boolean DTO")?;
        // The borrowed tree has already consumed the original JSON/work budget.
        // No serde copy or alternate source owner is needed for one boolean.
        return Ok((Some(Operation::Navigation(bibliographic_only)), None,
            original_cli_deadline(work)?.min(deadline), 0));
    }
    if matches!(id, "tos_knowledge_header" | "tos_evidence_projection") && ordinary {
        if !checked_field(root, "arguments", state, deadline)?
            .and_then(tos_foundation::JsonValue::as_object)
            .is_some_and(|fields| fields.is_empty()) {
            return Err("Core header strict empty arguments");
        }
        return Ok((Some(if id == "tos_knowledge_header" { Operation::KnowledgeHeader } else { Operation::Evidence }), None,
            original_cli_deadline(work)?.min(deadline), 0));
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
        Operation::Navigation(bibliographic_only) => (
            C::SourceNavigation { bibliographic_only: *bibliographic_only },
            R::Corpus, "carrier_corpus",
        ),
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
    result.map_err(|error| match error {
        tos_compiler::Error::Invalid(reason) | tos_compiler::Error::Budget(reason) => reason,
        error => {
            eprintln!("Core Whole same-state writer/reader: {error}");
            "Core Whole same-state writer/reader refused"
        }
    })?;
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
    cache_io: Option<&tos_source_store::PinnedSqliteIoBudget>,
    cache_space: Option<&tos_source_store::PinnedSqliteSpaceBudget>,
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
    if let Some(search) = request.search_read_model.as_ref() {
        search.validate_verify_chars()?;
        // Reference supplies a per-kind character ceiling in these same units.
        budget.nodes.max_verified_chars = budget.nodes.max_verified_chars
            .min(search.max_verify_chars);
        budget.relations.max_verified_chars = budget.relations.max_verified_chars
            .min(search.max_verify_chars);
    }
    let cursor = match checked_field(arguments, "cursor", ledger, cutoff)? {
        None | Some(tos_foundation::JsonValue::Null) => None,
        Some(value) => Some(value.as_str().ok_or("Core Whole cursor type")?),
    };

    macro_rules! deliver_bound_search {
        ($model:ident, $bound:ident, $native:ident, $view:ident, $revision:ident, $execute:path, $cursor_factory:expr) => {{
            let $revision = $bound.require_source_revision()
                .map_err(|_| tos_compiler::Error::Invalid("Core Whole query source revision absent"))?;
            if $revision != $native.source_revision || $view.source_revision() != Some($revision) {
                return Err(tos_compiler::Error::Invalid("Core Whole source associations differ"));
            }
            let overhead = whole_envelope_len(next, $revision).map_err(tos_compiler::Error::Invalid)?;
            let cap = session.limits.max_reply_bytes.checked_sub(overhead)
                .ok_or(tos_compiler::Error::Budget("Core Whole reply envelope cap"))?;
            budget.max_response_bytes = budget.max_response_bytes
                .min(cap).min(request.admission.whole_max_graph_bytes);
            if budget.max_response_bytes == 0 {
                return Err(tos_compiler::Error::Budget("Core Whole response cap"));
            }
            crate::reference_root_query::with_controlled_metadata_context($model,
                $bound, $view, cutoff, cancelled,
                |bytes| ledger.reserve(bytes).map_err(tos_compiler::Error::Invalid),
                |model, context| {
                context.with_operation(crate::reference_root_query::ReferenceMetadataOperation::IndexedSearch,
                    |authority| {
                    let current = || {
                        fence()?;
                        $view.verify_current().map_err(|_| "Core Whole transport source fence")?;
                        active(cutoff)
                    };
                    driver.respond(|sequence, _, reply| {
                        reply.narrow_deadline(cutoff)?;
                        current()?;
                        $execute(
                            model, $bound, authority, arguments, cursor, budget,
                            $cursor_factory,
                            |body| {
                                let map = |_| tos_query::search_v2::SearchV2Error {
                                    code: tos_query::search_v2::SearchV2ErrorCode::Unavailable,
                                    message: "Core Whole transport delivery refused",
                                };
                                current().map_err(map)?;
                                send_whole_result(ledger, reply, sequence, next, $revision, body).map_err(map)?;
                                current().map_err(map)
                            },
                        ).map_err(|_| "Core Whole scoped Search refused")?;
                        current()
                    }, current).map_err(tos_compiler::Error::Invalid)
                })
            })
        }};
    }

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
                            if let Some(selection) = request.search_read_model.as_ref()
                                .filter(|_| !request.query_store.configured)
                            {
                                if isolation.search_cache_source_root() != Some(root)
                                    || isolation.search_cache_path() != Some(selection.path.as_path())
                                {
                                    return Err(tos_compiler::Error::Invalid(
                                        "Core ordinary sidecar ticket source/path association",
                                    ));
                                }
                                isolation.search_cache_limits(&selection.path)
                                    .map_err(|_| tos_compiler::Error::Invalid(
                                        "Core ordinary sidecar ticket profile absent",
                                    ))?;
                                let io = cache_io.ok_or(tos_compiler::Error::Invalid(
                                    "Core ordinary sidecar I/O owner absent",
                                ))?;
                                let space = cache_space.ok_or(tos_compiler::Error::Invalid(
                                    "Core ordinary sidecar space owner absent",
                                ))?;
                                let graph_schema = if bound.source_basis().managed_source().is_some()
                                    || bound.source_basis().managed_source_v2().is_some()
                                {
                                    tos_compiler::managed_source::MANAGED_GRAPH_SCHEMA
                                } else {
                                    "tos_knowledge_graph_v1"
                                };
                                let current = || -> tos_compiler::Result<()> {
                                    fence().map_err(|_| tos_compiler::Error::Invalid(
                                        "Core ordinary sidecar currentness fence",
                                    ))?;
                                    view.verify_current()?;
                                    active(cutoff).map_err(|_| tos_compiler::Error::Budget(
                                        "Core ordinary sidecar cutoff",
                                    ))
                                };
                                let check_slot = |name: &std::ffi::OsStr,
                                                  file: Option<&std::fs::File>|
                                 -> tos_compiler::Result<()> {
                                    current()?;
                                    let parent = selection.path.parent().ok_or(
                                        tos_compiler::Error::Invalid(
                                            "Core ordinary sidecar parent absent",
                                        ),
                                    )?;
                                    let sources = [
                                        &request.source_paths.index_path,
                                        &request.source_paths.philosophy_graph_projection_path,
                                        &request.source_paths.bibliographic_graph_path,
                                        &request.source_paths.entity_type_registry_path,
                                        &request.source_paths.relation_type_registry_path,
                                        &request.source_paths.philosophy_post_planting_audit_path,
                                        &request.source_paths.evidence_projection_path,
                                    ];
                                    for source in sources {
                                        if source.parent() == Some(parent)
                                            && source.file_name() == Some(name)
                                        {
                                            return Err(tos_compiler::Error::Invalid(
                                                "Core ordinary sidecar leaf aliases a source path",
                                            ));
                                        }
                                    }
                                    if let Some(file) = file {
                                        use std::os::unix::fs::MetadataExt;
                                        let held = file.metadata().map_err(tos_compiler::Error::Io)?;
                                        for source in sources {
                                            ledger.charge_work(std::mem::size_of::<std::fs::Metadata>() as u64)
                                                .map_err(|_| tos_compiler::Error::Budget(
                                                    "Core ordinary sidecar source-alias census",
                                                ))?;
                                            match std::fs::metadata(source) {
                                                Ok(named) if (named.dev(), named.ino()) == (held.dev(), held.ino()) => {
                                                    return Err(tos_compiler::Error::Invalid(
                                                        "Core ordinary sidecar inode aliases a source",
                                                    ));
                                                }
                                                Ok(_) => {}
                                                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                                                Err(error) => return Err(tos_compiler::Error::Io(error)),
                                            }
                                        }
                                    }
                                    current()
                                };
                                super::ordinary_search_cache::with_ordinary_search_cache(
                                    model,
                                    isolation,
                                    super::ordinary_search_cache::OrdinarySearchCacheSelection {
                                        path: &selection.path,
                                        graph_schema,
                                        max_bytes: selection.max_bytes,
                                        max_postings: selection.max_postings,
                                    },
                                    io,
                                    space,
                                    cutoff,
                                    cancelled,
                                    &current,
                                    &check_slot,
                                    |sidecar| deliver_bound_search!(
                                        sidecar, bound, native, view, revision,
                                        tos_query::execute_scoped_controlled_sidecar_indexed_search_response,
                                        |initial, _receipt| crate::reference_cursor::NativeReferenceIndexedCursorCodec::new(
                                            initial,
                                            revision,
                                            tos_query::search_v2::SelectedQueryVocabulary::registered_source_ids(bound),
                                        )
                                    ),
                                )
                            } else {
                                deliver_bound_search!(
                                    model, bound, native, view, revision,
                                    tos_query::execute_scoped_controlled_indexed_search_response,
                                    |initial, receipt| Ok(crate::indexed_cursor::NativeIndexedCursorCodec::new(initial, receipt))
                                )
                            }
                        })
                })
        },
    )?;
    *generation = next;
    fence()
}

fn serve_whole_catalog(
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
            + std::mem::size_of::<tos_query::CatalogBudget>()
            + std::mem::size_of::<(&Path, &Request, &Ledger, Instant, Instant)>(),
    )?;
    let http = request
        .http
        .as_ref()
        .ok_or("Core Whole HTTP profile absent")?;
    let mut budget = http.selected.native()?.catalog;
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
                budget.max_packet_bytes = budget.max_packet_bytes
                    .min(cap).min(request.admission.whole_max_graph_bytes);
                if budget.max_packet_bytes == 0 {
                    return Err(tos_compiler::Error::Budget("Core Whole response cap"));
                }
                crate::reference_root_query::with_controlled_metadata_context(model,
                    bound, view, cutoff, cancelled,
                    |bytes| ledger.reserve(bytes).map_err(tos_compiler::Error::Invalid),
                    |model, context| {
                    context.with_operation(crate::reference_root_query::ReferenceMetadataOperation::Catalog,
                        |authority| {
                        let current = || {
                            fence()?;
                            view.verify_current().map_err(|_| "Core Whole transport source fence")?;
                            active(cutoff)
                        };
                        driver.respond(|sequence, _, reply| {
                            reply.narrow_deadline(cutoff)?;
                            current()?;
                            tos_query::execute_controlled_catalog_response(
                                model, bound, authority, budget,
                                |body| {
                                    let map = |_| tos_query::CatalogError {
                                        code: tos_query::CatalogErrorCode::PolicyBindingUnavailable,
                                        message: "Core Whole catalog delivery refused",
                                    };
                                    current().map_err(map)?;
                                    send_whole_result(ledger, reply, sequence, next, revision, body).map_err(map)?;
                                    current().map_err(map)
                                },
                            ).map_err(|_| "Core Whole scoped catalog refused")?;
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

fn state_inspect_key(ledger: &Ledger, cutoff: Instant,
    key: &tos_foundation::JsonString, kind: tos_query::search_v2::SearchKind) -> Result<()> {
    ledger.charge_work(key.units().len().checked_mul(2)
        .ok_or("Core inspect key work")? as u64)?;
    active(cutoff)?;
    let valid = if kind == tos_query::search_v2::SearchKind::Nodes {
        matches!(key.as_str(), Some("node_id" | "relation_limit"))
    } else { key.as_str() == Some("relation_id") };
    if !valid { return Err("Core inspect strict argument key"); }
    Ok(())
}

fn serve_whole_inspect(
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
    tool: &str,
    arguments: &tos_foundation::JsonValue,
) -> Result<()> {
    let kind = if tool == "tos_knowledge_node" { tos_query::search_v2::SearchKind::Nodes }
        else if tool == "tos_knowledge_relation" { tos_query::search_v2::SearchKind::Relations }
        else { return Err("Core inspect exact tool"); };
    let fields = arguments.as_object().ok_or("Core inspect arguments object")?;
    for (key, _) in fields {
        state_inspect_key(ledger, cutoff, key, kind)?;
    }
    let identifier = checked_field(arguments,
        if kind == tos_query::search_v2::SearchKind::Nodes { "node_id" } else { "relation_id" },
        ledger, cutoff)?.and_then(tos_foundation::JsonValue::as_str)
        .ok_or("Core inspect identifier string")?;
    let relation_limit = match checked_field(arguments, "relation_limit", ledger, cutoff)? {
        None => 200,
        Some(value) => usize::try_from(value.as_u64().filter(|n| *n <= 1000)
            .ok_or("Core inspect relation_limit integer")?)
            .map_err(|_| "Core inspect relation_limit size")?,
    };
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
            + std::mem::size_of::<tos_query::InspectBudget>()
            + std::mem::size_of::<(&Path, &Request, &Ledger, Instant, Instant)>(),
    )?;
    let http = request
        .http
        .as_ref()
        .ok_or("Core Whole HTTP profile absent")?;
    let mut budget = http.selected.native()?.inspect;
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
                    context.with_operation(if kind == tos_query::search_v2::SearchKind::Nodes {
                        crate::reference_root_query::ReferenceMetadataOperation::Node
                    } else { crate::reference_root_query::ReferenceMetadataOperation::Relation },
                        |authority| {
                        let current = || {
                            fence()?;
                            view.verify_current().map_err(|_| "Core Whole transport source fence")?;
                            active(cutoff)
                        };
                        driver.respond(|sequence, _, reply| {
                            reply.narrow_deadline(cutoff)?;
                            current()?;
                            tos_query::execute_controlled_inspect_response(
                                model, bound, authority, kind, identifier, relation_limit, budget,
                                |body| {
                                    let map = |_| tos_query::search_v2::SearchV2Error {
                                        code: tos_query::search_v2::SearchV2ErrorCode::Unavailable,
                                        message: "Core Whole catalog delivery refused",
                                    };
                                    current().map_err(map)?;
                                    send_whole_result(ledger, reply, sequence, next, revision, body).map_err(map)?;
                                    current().map_err(map)
                                },
                            ).map_err(|_| "Core Whole scoped catalog refused")?;
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

fn serve_whole_philosophy_status(
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
    tool: &str,
) -> Result<()> {
    let argument_geometry = arguments.retained_storage_bytes()
        .map_err(|_| "Core Corpus arguments state")?;
    let _argument_hold = ledger.reserve(argument_geometry.checked_mul(3)
        .ok_or("Core Corpus arguments forecast")?)?;
    ledger.charge_work(argument_geometry as u64)?;
    active(cutoff)?;
    let resource = tool == "tos_native_resource_read";
    let mut philosophy_request = tos_query::philosophy_read::PhilosophyReadRequest::Status;
    let (corpus_request, render) = if resource {
        let fields = arguments.as_object().filter(|f| f.len() == 2)
            .ok_or("Core resource strict arguments")?;
        for (key, _) in fields {
            ledger.charge_work(key.units().len().checked_mul(2)
                .ok_or("Core resource key work")? as u64)?;
            active(cutoff)?;
            if !matches!(key.as_str(), Some("uri" | "render")) { return Err("Core resource unknown argument"); }
        }
        let uri = checked_field(arguments, "uri", ledger, cutoff)?
            .and_then(tos_foundation::JsonValue::as_str).ok_or("Core resource URI string")?;
        ledger.charge_work(uri.len() as u64)?;
        let selected = match uri {
            "tos-philosophy://status" => None,
            "tos-philosophy://layers" => {
                philosophy_request = tos_query::philosophy_read::PhilosophyReadRequest::Layers; None
            }
            "tos-philosophy://snapshot" => {
                philosophy_request = tos_query::philosophy_read::PhilosophyReadRequest::Snapshot; None
            }
            "tos-corpus://status" => Some(tos_query::corpus_read::CorpusReadRequest::Status),
            "tos-corpus://summary" => Some(tos_query::corpus_read::CorpusReadRequest::Summary),
            "tos-corpus://graph-views" => Some(tos_query::corpus_read::CorpusReadRequest::GraphViews),
            _ if uri.starts_with("tos-corpus://graph-view/") => {
                let id = &uri["tos-corpus://graph-view/".len()..];
                if uri.len() > 4096 || id.is_empty() || id.contains('/') {
                    return Err("Core Corpus view resource identifier");
                }
                Some(tos_query::corpus_read::CorpusReadRequest::GraphView {
                    view_id: id.to_owned(), limit: 100,
                })
            }
            _ => return Err("Core resource controlled owner unavailable for URI"),
        };
        let render = match checked_field(arguments, "render", ledger, cutoff)? {
            Some(tos_foundation::JsonValue::Bool(render)) => *render,
            _ => return Err("Core resource render bool"),
        };
        (selected, render)
    } else if tool == "tos_philosophy_graph_status" {
        if !arguments.as_object().is_some_and(|f| f.is_empty()) {
            return Err("Core metadata strict empty arguments");
        }
        (None, false)
    } else {
        if matches!(tool, "tos_philosophy_graph_status" | "tos_philosophy_graph_layers" | "tos_philosophy_graph_snapshot" | "tos.snapshot") {
            if !arguments.as_object().is_some_and(|f| f.is_empty()) { return Err("Core metadata strict empty arguments"); }
            philosophy_request = match tool {
                "tos_philosophy_graph_layers" => tos_query::philosophy_read::PhilosophyReadRequest::Layers,
                "tos_philosophy_graph_snapshot" | "tos.snapshot" => tos_query::philosophy_read::PhilosophyReadRequest::Snapshot,
                _ => tos_query::philosophy_read::PhilosophyReadRequest::Status,
            };
            (None, false)
        } else {
            let operation = crate::knowledge::KnowledgeOperation::from_id(tool).ok_or("Core Corpus tool unavailable")?;
            let selected = crate::knowledge::KnowledgeRequest::from_arguments(operation, arguments).map_err(|_| "Core Corpus strict arguments")?;
            let crate::knowledge::KnowledgeRequest::Corpus(selected) = selected else { return Err("Core Corpus typed request differs"); };
            (Some(selected), false)
        }
    };
    let corpus_root = root.to_str().ok_or("Core Corpus root UTF-8")?;
    let corpus_index = request.source_paths.index_path.to_str().ok_or("Core Corpus index UTF-8")?;
    let _corpus_context_hold = ledger.reserve(corpus_root.len().checked_add(corpus_index.len())
        .and_then(|n| n.checked_add(std::mem::size_of::<tos_query::corpus_read::CorpusReadContext>()))
        .ok_or("Core Corpus context state")?)?;
    ledger.charge_work(corpus_root.len().checked_add(corpus_index.len())
        .ok_or("Core Corpus context work")? as u64)?;
    let corpus_context = tos_query::corpus_read::CorpusReadContext {
        tos_root: corpus_root.to_owned(), index_path: corpus_index.to_owned(),
    };
    let operation = match &corpus_request {
        Some(request) => crate::reference_root_query::ReferenceMetadataOperation::for_corpus(request),
        None => crate::reference_root_query::ReferenceMetadataOperation::for_philosophy(
            &philosophy_request),
    };
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
            + std::mem::size_of::<tos_query::InspectBudget>()
            + std::mem::size_of::<(&Path, &Request, &Ledger, Instant, Instant)>(),
    )?;
    let http = request
        .http
        .as_ref()
        .ok_or("Core Whole HTTP profile absent")?;
    let mut budget = http.selected.native()?.inspect;
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
                    context.with_operation(operation,
                        |authority| {
                        let current = || {
                            fence()?;
                            view.verify_current().map_err(|_| "Core Whole transport source fence")?;
                            active(cutoff)
                        };
                        driver.respond(|sequence, _, reply| {
                            reply.narrow_deadline(cutoff)?;
                            current()?;
                            let delivery = |body: &[u8]| {
                                    let map = |_| tos_query::search_v2::SearchV2Error {
                                        code: tos_query::search_v2::SearchV2ErrorCode::Unavailable,
                                        message: "Core Whole philosophy status delivery refused",
                                    };
                                    current().map_err(map)?;
                                    send_whole_result(ledger, reply, sequence, next, revision, body).map_err(map)?;
                                    current().map_err(map)
                                };
                            if let Some(corpus_request) = &corpus_request {
                                tos_query::execute_controlled_corpus_response(
                                    model, bound, authority, budget, corpus_request,
                                    &corpus_context, render, delivery)
                                    .map_err(|_| "Core Whole scoped Corpus metadata refused")?;
                            } else {
                                tos_query::execute_controlled_philosophy_metadata_response_render(
                                    model, bound, authority, budget, &philosophy_request, render, delivery)
                                    .map_err(|_| "Core Whole scoped Philosophy metadata refused")?;
                            }
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

fn serve_whole_philosophy_domain(
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
    tool: &str,
) -> Result<()> {
    let geometry = arguments.retained_storage_bytes().map_err(|_| "Core Philosophy arguments state")?;
    let _arguments = ledger.reserve(geometry.checked_mul(3).ok_or("Core Philosophy arguments forecast")?)?;
    ledger.charge_work(geometry as u64)?;
    active(cutoff)?;
    let (selected, render) = if tool == "tos_native_resource_read" {
        let fields = arguments.as_object().filter(|f| f.len() == 2)
            .ok_or("Core Philosophy resource strict arguments")?;
        if fields.iter().any(|(k,_)| !matches!(k.as_str(), Some("uri" | "render"))) {
            return Err("Core Philosophy resource unknown argument");
        }
        let uri = checked_field(arguments, "uri", ledger, cutoff)?
            .and_then(tos_foundation::JsonValue::as_str).ok_or("Core Philosophy resource URI")?;
        if uri.len() > 4096 { return Err("Core Philosophy URI byte bound"); }
        let selected = if uri == "tos-philosophy://views" {
            tos_query::philosophy_read::PhilosophyReadRequest::Views
        } else {
            let id = uri.strip_prefix("tos-philosophy://view/").ok_or("Core Philosophy resource unavailable")?;
            if id.is_empty() || id.contains('/') { return Err("Core Philosophy raw view identifier"); }
            tos_query::philosophy_read::PhilosophyReadRequest::View { view_id: id.to_owned(), limit: 1000 }
        };
        let render = match checked_field(arguments, "render", ledger, cutoff)? {
            Some(tos_foundation::JsonValue::Bool(v)) => *v,
            _ => return Err("Core Philosophy render bool"),
        };
        (selected, render)
    } else {
        let operation = match tool {
            "tos_philosophy_graph_views" => crate::knowledge::KnowledgeOperation::PhilosophyViews,
            "tos_philosophy_graph_view" | "tos.view.open" => crate::knowledge::KnowledgeOperation::PhilosophyView,
            _ => return Err("Core Philosophy domain tool unavailable"),
        };
        let crate::knowledge::KnowledgeRequest::Philosophy(selected) =
            crate::knowledge::KnowledgeRequest::from_arguments(operation, arguments)
                .map_err(|_| "Core Philosophy strict arguments")? else {
                return Err("Core Philosophy domain typed request differs");
            };
        (selected, false)
    };
    let operation = crate::reference_root_query::ReferenceMetadataOperation::for_philosophy(&selected);
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
            + std::mem::size_of::<tos_query::InspectBudget>()
            + std::mem::size_of::<(&Path, &Request, &Ledger, Instant, Instant)>(),
    )?;
    let http = request
        .http
        .as_ref()
        .ok_or("Core Whole HTTP profile absent")?;
    let mut budget = http.selected.native()?.inspect;
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
                    context.with_operation(operation,
                        |authority| {
                        let current = || {
                            fence()?;
                            view.verify_current().map_err(|_| "Core Whole transport source fence")?;
                            active(cutoff)
                        };
                        driver.respond(|sequence, _, reply| {
                            reply.narrow_deadline(cutoff)?;
                            current()?;
                            let delivery = |body: &[u8]| {
                                    let map = |_| tos_query::search_v2::SearchV2Error {
                                        code: tos_query::search_v2::SearchV2ErrorCode::Unavailable,
                                        message: "Core Whole philosophy status delivery refused",
                                    };
                                    current().map_err(map)?;
                                    send_whole_result(ledger, reply, sequence, next, revision, body).map_err(map)?;
                                    current().map_err(map)
                                };
                            tos_query::execute_controlled_philosophy_domain_response(
                                model, bound, authority, budget, &selected, render, delivery)
                                .map_err(|_| "Core Whole scoped Philosophy domain refused")?;
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

fn serve_whole_health(
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
    if !arguments.as_object().is_some_and(|fields| fields.is_empty()) {
        return Err("Core Health strict empty arguments");
    }
    let corpus_root = root.to_str().ok_or("Core Corpus root UTF-8")?;
    let corpus_index = request.source_paths.index_path.to_str().ok_or("Core Corpus index UTF-8")?;
    let _corpus_context_hold = ledger.reserve(corpus_root.len().checked_add(corpus_index.len())
        .and_then(|n| n.checked_add(std::mem::size_of::<tos_query::corpus_read::CorpusReadContext>()))
        .ok_or("Core Corpus context state")?)?;
    ledger.charge_work(corpus_root.len().checked_add(corpus_index.len())
        .ok_or("Core Corpus context work")? as u64)?;
    let corpus_context = tos_query::corpus_read::CorpusReadContext {
        tos_root: corpus_root.to_owned(), index_path: corpus_index.to_owned(),
    };
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
            + std::mem::size_of::<tos_query::InspectBudget>()
            + std::mem::size_of::<(&Path, &Request, &Ledger, Instant, Instant)>(),
    )?;
    let http = request
        .http
        .as_ref()
        .ok_or("Core Whole HTTP profile absent")?;
    let mut budget = http.selected.native()?.inspect;
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
                    let probe = context.abort_probe();
                    let current = || {
                        fence()?;
                        view.verify_current().map_err(|_| "Core Whole health source fence")?;
                        active(cutoff)
                    };
                    driver.respond(|sequence, _, reply| {
                        reply.narrow_deadline(cutoff)?; current()?;
                        crate::controlled_reference_health::execute_controlled_reference_health(
                            model, bound, context, &corpus_context, budget, probe.clone(),
                            |body| {
                                current().map_err(tos_compiler::Error::Invalid)?;
                                send_whole_result(ledger, reply, sequence, next, revision, body)
                                    .map_err(tos_compiler::Error::Invalid)?;
                                current().map_err(tos_compiler::Error::Invalid)
                            }).map_err(|_| "Core Whole health owner refused")?;
                        current()
                    }, current).map_err(tos_compiler::Error::Invalid)
                })
            })
        })
        },
    )?;
    *generation = next;
    fence()
}

fn serve_whole_lens(
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
    tool: &str,
) -> Result<()> {
    let geometry = arguments.retained_storage_bytes()
        .map_err(|_| "Core Lens arguments state")?;
    let _argument_hold = ledger.reserve(geometry.checked_mul(3)
        .ok_or("Core Lens arguments forecast")?)?;
    ledger.charge_work(geometry as u64)?;
    active(cutoff)?;
    let owned_focus;
    let (selected, operation) = match tool {
        "tos_knowledge_lens_compile" => {
            let fields = arguments.as_object().filter(|fields| fields.len() == 1)
                .ok_or("Core Lens strict spec arguments")?;
            if fields[0].0.as_str() != Some("spec") { return Err("Core Lens unknown argument"); }
            let spec = checked_field(arguments, "spec", ledger, cutoff)?
                .filter(|v| v.as_object().is_some()).ok_or("Core Lens spec object")?;
            (tos_query::ControlledLensRequest::Compile(spec),
                crate::reference_root_query::ReferenceMetadataOperation::Lens)
        }
        "tos_knowledge_focus" => {
            owned_focus = match crate::knowledge::KnowledgeRequest::from_arguments(
                crate::knowledge::KnowledgeOperation::Focus, arguments)
                .map_err(|_| "Core Focus arguments")? {
                crate::knowledge::KnowledgeRequest::Focus(focus) => focus,
                _ => return Err("Core Focus typed request differs"),
            };
            (tos_query::ControlledLensRequest::Focus(&owned_focus),
                crate::reference_root_query::ReferenceMetadataOperation::Focus)
        }
        "tos_knowledge_lens_open" => {
            let fields = arguments.as_object().filter(|fields| fields.len() == 1)
                .ok_or("Core Stored Lens strict arguments")?;
            if fields[0].0.as_str() != Some("lens_id") { return Err("Core Stored Lens unknown argument"); }
            let id = checked_field(arguments, "lens_id", ledger, cutoff)?
                .and_then(tos_foundation::JsonValue::as_str).ok_or("Core Stored Lens id")?;
            (tos_query::ControlledLensRequest::Stored(id),
                crate::reference_root_query::ReferenceMetadataOperation::StoredLens)
        }
        _ => return Err("Core Lens tool unavailable"),
    };
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
            + std::mem::size_of::<tos_query::knowledge_lens::LensBudget>()
            + std::mem::size_of::<(&Path, &Request, &Ledger, Instant, Instant)>(),
    )?;
    let http = request
        .http
        .as_ref()
        .ok_or("Core Whole HTTP profile absent")?;
    let mut budget = http.selected.native()?.lens;
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
                budget.inspect.max_response_bytes = budget.inspect.max_response_bytes
                    .min(cap).min(request.admission.whole_max_graph_bytes);
                if budget.inspect.max_response_bytes == 0 {
                    return Err(tos_compiler::Error::Budget("Core Whole response cap"));
                }
                crate::reference_root_query::with_controlled_metadata_context(model,
                    bound, view, cutoff, cancelled,
                    |bytes| ledger.reserve(bytes).map_err(tos_compiler::Error::Invalid),
                    |model, context| {
                    context.with_operation(operation,
                        |authority| {
                        let current = || {
                            fence()?;
                            view.verify_current().map_err(|_| "Core Whole transport source fence")?;
                            active(cutoff)
                        };
                        driver.respond(|sequence, _, reply| {
                            reply.narrow_deadline(cutoff)?;
                            current()?;
                            tos_query::execute_controlled_lens_request_response(
                                model, bound, authority, selected, budget,
                                |body| {
                                    let map = |_| tos_query::search_v2::SearchV2Error {
                                        code: tos_query::search_v2::SearchV2ErrorCode::Unavailable,
                                        message: "Core Whole Lens delivery refused",
                                    };
                                    current().map_err(map)?;
                                    send_whole_result(ledger, reply, sequence, next, revision, body).map_err(map)?;
                                    current().map_err(map)
                                },
                            ).map_err(|_| "Core Whole scoped catalog refused")?;
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

fn serve_whole_metadata(
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
    evidence_projection: bool,
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
            + std::mem::size_of::<tos_query::InspectBudget>()
            + std::mem::size_of::<(&Path, &Request, &Ledger, Instant, Instant)>(),
    )?;
    let http = request
        .http
        .as_ref()
        .ok_or("Core Whole HTTP profile absent")?;
    let mut budget = http.selected.native()?.inspect;
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
            if evidence_projection {
                let cap = session.limits.max_reply_bytes
                    .checked_sub(whole_envelope_len(next, &native.source_revision)
                        .map_err(tos_compiler::Error::Invalid)?)
                    .ok_or(tos_compiler::Error::Budget("Core Evidence reply envelope"))?
                    .min(request.admission.whole_max_graph_bytes);
                let limits = tos_compiler::native_snapshot_manifest::portable_native_snapshot_limits(
                    request.admission.max_build_seconds)?.capture;
                let _path = ledger.reserve(isolation.root().as_os_str().len()
                    .checked_add(256).ok_or(tos_compiler::Error::Budget("Core Evidence path workspace"))?)
                    .map_err(tos_compiler::Error::Invalid)?;
                let staging = fresh(isolation.root(), "tos-core-session-evidence")
                    .map_err(tos_compiler::Error::Invalid)?;
                let current = || {
                    fence()?;
                    active(cutoff)
                };
                return loan.with_owned_evidence_delivery(completed, &staging, limits, cap,
                    request.admission.json.limits().map_err(tos_compiler::Error::Invalid)?, |body| {
                    driver.respond(|sequence, _, reply| {
                        reply.narrow_deadline(cutoff)?;
                        current()?;
                        send_whole_result(ledger, reply, sequence, next, &native.source_revision, body)?;
                        current()
                    }, current).map_err(tos_compiler::Error::Invalid)
                });
            }
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
                    context.with_operation(crate::reference_root_query::ReferenceMetadataOperation::KnowledgeHeader,
                        |authority| {
                        let current = || {
                            fence()?;
                            view.verify_current().map_err(|_| "Core Whole transport source fence")?;
                            active(cutoff)
                        };
                        driver.respond(|sequence, _, reply| {
                            reply.narrow_deadline(cutoff)?;
                            current()?;
                            tos_query::execute_controlled_knowledge_header_response(
                                model, bound, authority, budget,
                                |body| {
                                    let map = |_| tos_query::search_v2::SearchV2Error {
                                        code: tos_query::search_v2::SearchV2ErrorCode::Unavailable,
                                        message: "Core Whole header delivery refused",
                                    };
                                    current().map_err(map)?;
                                    send_whole_result(ledger, reply, sequence, next, revision, body).map_err(map)?;
                                    current().map_err(map)
                                },
                            ).map_err(|_| "Core Whole scoped header refused")?;
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
    metadata_tool: &str,
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
            if metadata_tool == "tos_knowledge_search_indexed_v2" {
                store.indexed_search(arguments, request, ledger, heap, deadline, cancelled, cap)?
            } else {
                store.search(arguments, request, ledger, heap, deadline, cancelled, cap)?
            }
        } else {
            store.metadata(
                metadata_tool,
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
    request: &mut Request,
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
    // Snapshot verification opens its selected SQLite metadata connection.
    // Establish the one process heap before that first connection, under the
    // same original state ledger and actual kernel resource custody.
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
    .map_err(|error| match error {
        // These owner-authored static reasons include the original ledger's
        // refusal. Dynamic SQL, I/O and source text stay behind the boundary.
        tos_compiler::Error::Invalid(reason) | tos_compiler::Error::Budget(reason) => reason,
        _ => "Core lazy dedicated SQLite backend admission",
    })?;
    state.retained.set(
        state
            .retained
            .get()
            .checked_add(sqlite_heap.reserved_state_bytes())
            .ok_or("Core lazy SQLite reserved state overflow")?,
    );
    state.remaining(0)?;
    let mut startup_guard_refused = false;
    if request.snapshot_root.is_some() || request.expected_snapshot_guard.is_some() {
        let _guard_setup = state.reserve(ordinary_snapshot_guard::MAX_RETAINED_STATE_BYTES)?;
        let process = request.admission.process;
        let guard_open = request.open_snapshot_guard_with(
            root,
            deadline,
            |bytes| state.charge_work(bytes),
            |visits| state.charge_visits(visits),
            || {
                active(deadline)?;
                session
                    .control
                    .verify_current()
                    .map_err(|_| "Core snapshot guard issued control")?;
                process
                    .verify_current()
                    .map_err(|_| "Core snapshot guard current process")
            },
        );
        drop(_guard_setup);
        match guard_open {
            Ok(guard_bytes) => {
                state.retained.set(
                    state
                        .retained
                        .get()
                        .checked_add(guard_bytes)
                        .ok_or("Core snapshot guard retained state overflow")?,
                );
                state.remaining(0)?;
            }
            Err("DataAccessUnavailable") => startup_guard_refused = true,
            Err(error) => return Err(error),
        }
    }
    // One original session IO/space owner is reused by every ordinary sidecar
    // operation. The build file cap never becomes a read limit; a healthy cache
    // may be larger than the current build cap.
    let selected_store_at_startup = if ordinary && !startup_guard_refused {
        with_selected_store_choice(request, &state, deadline, |selected| Ok(selected))?
    } else {
        false
    };
    // A selected QueryStore owns indexed execution directly. Its sidecar
    // selector is retained only for max_verify_chars; no cache path/build ticket
    // or cache IO/space grant is consulted for that route.
    let cache_enabled = ordinary
        && request.search_read_model.is_some()
        && !selected_store_at_startup;
    let cache_owner_state_bytes = std::mem::size_of::<Option<native_ledger::Reservation<'_>>>()
        .checked_add(std::mem::size_of::<bool>())
        .and_then(|n| n.checked_add(std::mem::size_of::<u64>()))
        .and_then(|n| n.checked_add(std::mem::size_of::<(
            tos_source_store::PinnedSqliteIoBudget,
            tos_source_store::PinnedSqliteSpaceBudget,
        )>()))
        .and_then(|n| n.checked_add(std::mem::size_of::<(
            Option<tos_source_store::PinnedSqliteIoBudget>,
            Option<tos_source_store::PinnedSqliteSpaceBudget>,
        )>()))
        .and_then(|n| n.checked_add(
            tos_source_store::PinnedSqliteIoBudget::shared_state_upper_bound(),
        ))
        .and_then(|n| n.checked_add(
            tos_source_store::PinnedSqliteSpaceBudget::shared_state_upper_bound(),
        ))
        .ok_or("Core ordinary sidecar owner state overflow")?;
    let _cache_owner_state = if cache_enabled {
        Some(state.reserve(cache_owner_state_bytes)?)
    } else {
        None
    };
    let (cache_io, cache_space) = if cache_enabled {
        let selection = request.search_read_model.as_ref()
            .ok_or("Core ordinary sidecar selection absent")?;
        if isolation.search_cache_source_root() != Some(root)
            || isolation.search_cache_path() != Some(selection.path.as_path())
        {
            return Err("Core ordinary sidecar ticket source/path association");
        }
        let (_, issued_temp) = isolation.search_cache_limits(&selection.path)
            .map_err(|_| "Core ordinary sidecar ticket profile absent")?;
        let io = tos_source_store::PinnedSqliteIoBudget::new(
            request.admission.cold.max_work_bytes,
            request.admission.cold.max_work_bytes,
        ).map_err(|_| "Core ordinary sidecar original I/O budget")?;
        let space = tos_source_store::PinnedSqliteSpaceBudget::new(issued_temp)
            .map_err(|_| "Core ordinary sidecar issued space budget")?;
        (Some(io), Some(space))
    } else {
        (None, None)
    };
    let mut workspace = Workspace(&state);
    let fence_without_snapshot = || {
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
    let fence = || {
        fence_without_snapshot()?;
        request.check_snapshot_guard_with(
            deadline,
            |bytes| state.charge_work(bytes),
            |visits| state.charge_visits(visits),
            || active(deadline),
        )?;
        active(deadline)
    };
    let consume = |driver: &mut session_transport::Driver<'_, '_>| {
        if startup_guard_refused
            || (request.snapshot_root.is_some()
                && request
                    .check_snapshot_guard_with(
                        deadline,
                        |bytes| state.charge_work(bytes),
                        |visits| state.charge_visits(visits),
                        || active(deadline),
                    )
                    .is_err())
        {
            const REFUSAL: &[u8] = br#"{"schema_version":"tos_native_core_ordinary_session_ready_v1","ok":false,"error":"Selected data snapshot is unavailable","code":"DataAccessUnavailable"}"#;
            driver.startup(
                |reply| reply.send(session_transport::STARTUP, 0, &[REFUSAL]),
                &fence_without_snapshot,
            )?;
            if driver.receive(fence_without_snapshot)? {
                return Err("DataAccessUnavailable");
            }
            return Ok(());
        }
        let mut held: Option<Carrier> = None;
        let mut held_store: Option<lazy_store::Store> = None;
        let mut generation = 0u64;
        let mut store_indexed_ready = false;
        if ordinary && selected_store_at_startup {
            match lazy_store::open(request, &state, &sqlite_heap, deadline, deadline, cancelled) {
                Ok(store) => {
                    state.retained.set(
                        state.retained.get().checked_add(store.retained)
                            .ok_or("Core Store startup retained overflow")?,
                    );
                    state.remaining(0)?;
                    store_indexed_ready = store.supports_indexed_fts5();
                    held_store = Some(store);
                    generation = 1;
                }
                Err(_) => {
                    // Admission/cancellation still fails under its original
                    // cutoff; an unavailable selected Store never falls back.
                    active(deadline)?;
                    state.remaining(0)?;
                }
            }
        }
        // Shared connected capabilities retain the exact old lazy ABI. The
        // indexed profile is added only for an admitted selected backend.
        const LAZY_HEAD: &[u8] = br#"{"schema_version":"tos_native_core_lazy_session_ready_v1","ok":true,"profile":"tos_core_lazy_selected_v1""#;
        const ORDINARY_HEAD: &[u8] = br#"{"schema_version":"tos_native_core_ordinary_session_ready_v1","ok":true,"profile":"tos_core_ordinary_selected_v1""#;
        const COMMON: &[u8] = br#","reference_semantics":"cpython_pathlib_is_file_3_14","source_revision":null,"data_revision":null,"state_reused":false,"selection":{"schema_version":"tos_native_core_selected_profile_v1","generation":0,"profile":"selected_paths","source_revision":null,"data_revision":null,"exploration_revision":null,"state_reused":false},"capabilities":[{"operation":"tos_corpus_index_exists"},{"operation":"tos_philosophy_projection_exists"},{"operation":"tos_evidence_projection_exists"},{"operation":"tos_philosophy_audit_exists"},{"operation":"tos_corpus_index"},{"operation":"tos_bibliographic_graph"},{"operation":"tos_philosophy_projection"},{"operation":"tos_philosophy_audit_payload"},{"operation":"tos_corpus_header"},{"operation":"tos_native_call","tool":"tos_knowledge_search","profiles":["weak_query_store"]}"#;
        const WHOLE_INDEXED_CAP: &[u8] = br#",{"operation":"tos_native_call","tool":"tos_knowledge_search_indexed_v2","profiles":["whole_root"]}"#;
        const STORE_INDEXED_CAP: &[u8] = br#",{"operation":"tos_native_call","tool":"tos_knowledge_search_indexed_v2","profiles":["weak_query_store"]}"#;
        const LAZY_END: &[u8] = b"]}";
        const ORDINARY_END: &[u8] = br#",{"operation":"tos_native_call","tool":"tos_knowledge_lens_compile","profiles":["whole_root"]},{"operation":"tos_native_call","tool":"tos_knowledge_focus","profiles":["whole_root"]},{"operation":"tos_native_call","tool":"tos_knowledge_lens_open","profiles":["whole_root"]},{"operation":"tos_native_call","tool":"tos_philosophy_graph_status","profiles":["whole_root"]},{"operation":"tos_native_call","tool":"tos_corpus_status","profiles":["whole_root"]},{"operation":"tos_native_call","tool":"tos_corpus_summary","profiles":["whole_root"]},{"operation":"tos_native_call","tool":"tos_corpus_graph_views","profiles":["whole_root"]},{"operation":"tos_native_call","tool":"tos_knowledge_node","profiles":["whole_root"]},{"operation":"tos_native_call","tool":"tos_knowledge_relation","profiles":["whole_root"]},{"operation":"tos_native_call","tool":"tos_knowledge_catalog","profiles":["whole_root","weak_query_store"]},{"operation":"tos_source_navigation"},{"operation":"tos_knowledge_header","profiles":["whole_root","weak_query_store"]},{"operation":"tos_evidence_projection","profiles":["whole_root"]},{"operation":"tos_native_call","tool":"tos_knowledge_search","profiles":["whole_root"]},{"operation":"tos_native_call","tool":"tos_philosophy_graph_layers","profiles":["whole_root"]},{"operation":"tos_native_call","tool":"tos_philosophy_graph_snapshot","profiles":["whole_root"]},{"operation":"tos_native_call","tool":"tos.snapshot","profiles":["whole_root"]},{"operation":"tos_native_call","tool":"tos_corpus_search","profiles":["whole_root"]},{"operation":"tos_native_call","tool":"tos_corpus_resources","profiles":["whole_root"]},{"operation":"tos_native_call","tool":"tos_corpus_node","profiles":["whole_root"]},{"operation":"tos_native_call","tool":"tos_corpus_relation_pack","profiles":["whole_root"]},{"operation":"tos_native_call","tool":"tos_corpus_graph_view","profiles":["whole_root"]},{"operation":"tos_native_call","tool":"tos_corpus_packet","profiles":["whole_root"]},{"operation":"tos_native_call","tool":"tos_philosophy_graph_views","profiles":["whole_root"]},{"operation":"tos_native_call","tool":"tos_philosophy_graph_view","profiles":["whole_root"]},{"operation":"tos_native_call","tool":"tos.view.open","profiles":["whole_root"]},{"operation":"tos_native_call","tool":"tos_access_health","profiles":["whole_root"]},{"operation":"tos_native_call","tool":"tos_native_resource_read","profiles":["whole_root"],"resources":["tos-philosophy://status","tos-corpus://status","tos-corpus://summary","tos-corpus://graph-views","tos-philosophy://layers","tos-philosophy://snapshot","tos-philosophy://views"],"resource_templates":["tos-corpus://graph-view/{view_id}","tos-philosophy://view/{view_id}"]}]}"#;
        let whole_indexed_cap = if ordinary && selected_store_at_startup { &[] } else { WHOLE_INDEXED_CAP };
        let store_indexed_cap = if ordinary && store_indexed_ready { STORE_INDEXED_CAP } else { &[] };
        let ready = [if ordinary { ORDINARY_HEAD } else { LAZY_HEAD }, COMMON,
                     whole_indexed_cap, store_indexed_cap,
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
        if let Some(receipt) = ordinary.then(|| request.snapshot_guard_receipt()).flatten() {
            const GUARD_FIELD: &[u8] = br#","snapshot_guard":""#;
            const QUOTE: &[u8] = b"\"";
            let guarded_ready = [
                ORDINARY_HEAD,
                GUARD_FIELD,
                receipt.as_bytes(),
                QUOTE,
                COMMON,
                whole_indexed_cap,
                store_indexed_cap,
                ORDINARY_END,
            ];
            let ready_bytes = guarded_ready.iter().map(|segment| segment.len()).sum::<usize>();
            if ready_bytes
                > request
                    .http
                    .as_ref()
                    .ok_or("Core lazy profile absent")?
                    .max_startup_receipt_bytes
            {
                return Err("Core lazy startup receipt cap");
            }
            let _ready_state = state.reserve(ready_bytes)?;
            driver.startup(
                |reply| reply.send(session_transport::STARTUP, 0, &guarded_ready),
                &fence_without_snapshot,
            )?;
        } else {
            driver.startup(
                |reply| reply.send(session_transport::STARTUP, 0, &ready),
                &fence_without_snapshot,
            )?;
        }
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
                call(raw, sequence, request, &state, deadline, cancelled.as_ref(), ordinary)?
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
                if tool == "tos_access_health" {
                    with_selected_store_choice(request, &state, cutoff, |selected_store| {
                        if selected_store { return Err("Core selected QueryStore Health owner unavailable"); }
                        serve_whole_health(root, request, &isolation, session, driver,
                            &mut held, &mut held_store, &mut generation, &state,
                            &sqlite_heap, &resources, deadline, cutoff, cancelled, &fence, arguments)
                    })?;
                    last_whole = true;
                    continue;
                }
                if matches!(tool, "tos_knowledge_lens_compile" | "tos_knowledge_focus" | "tos_knowledge_lens_open") {
                    with_selected_store_choice(request, &state, cutoff, |selected_store| {
                        if selected_store { return Err("Core selected QueryStore Lens owner unavailable"); }
                        serve_whole_lens(root, request, &isolation, session, driver,
                            &mut held, &mut held_store, &mut generation, &state,
                            &sqlite_heap, &resources, deadline, cutoff, cancelled, &fence, arguments, tool)
                    })?;
                    last_whole = true;
                    continue;
                }
                let philosophy_domain = matches!(tool, "tos_philosophy_graph_views" | "tos_philosophy_graph_view" | "tos.view.open")
                    || tool == "tos_native_resource_read" && checked_field(arguments, "uri", &state, cutoff)?
                        .and_then(tos_foundation::JsonValue::as_str)
                        .is_some_and(|uri| uri == "tos-philosophy://views" || uri.starts_with("tos-philosophy://view/"));
                if philosophy_domain {
                    with_selected_store_choice(request, &state, cutoff, |selected_store| {
                        if selected_store { return Err("Core selected QueryStore Philosophy domain owner unavailable"); }
                        serve_whole_philosophy_domain(root, request, &isolation, session, driver,
                            &mut held, &mut held_store, &mut generation, &state,
                            &sqlite_heap, &resources, deadline, cutoff, cancelled, &fence, arguments, tool)
                    })?;
                    last_whole = true;
                    continue;
                }
                if matches!(tool, "tos_philosophy_graph_status" | "tos_native_resource_read" | "tos_corpus_status" | "tos_corpus_summary" | "tos_corpus_graph_views" | "tos_philosophy_graph_layers" | "tos_philosophy_graph_snapshot" | "tos.snapshot" | "tos_corpus_search" | "tos_corpus_resources" | "tos_corpus_node" | "tos_corpus_relation_pack" | "tos_corpus_graph_view" | "tos_corpus_packet") {
                    with_selected_store_choice(request, &state, cutoff, |selected_store| {
                        if selected_store { return Err("Core selected QueryStore philosophy owner unavailable"); }
                        serve_whole_philosophy_status(root, request, &isolation, session, driver,
                            &mut held, &mut held_store, &mut generation, &state,
                            &sqlite_heap, &resources, deadline, cutoff, cancelled, &fence, arguments, tool)
                    })?;
                    last_whole = true;
                    continue;
                }
                if matches!(tool, "tos_knowledge_node" | "tos_knowledge_relation") {
                    with_selected_store_choice(request, &state, cutoff, |selected_store| {
                        if selected_store { return Err("Core selected QueryStore inspect owner unavailable"); }
                        serve_whole_inspect(root, request, &isolation, session, driver,
                            &mut held, &mut held_store, &mut generation, &state,
                            &sqlite_heap, &resources, deadline, cutoff, cancelled, &fence,
                            tool, arguments)
                    })?;
                    last_whole = true;
                    continue;
                }
                if tool == "tos_knowledge_catalog" {
                    if !arguments.as_object().is_some_and(|fields| fields.is_empty()) {
                        return Err("Core catalog strict empty arguments");
                    }
                    let whole = with_selected_store_choice(request, &state, cutoff, |selected_store| {
                        if selected_store {
                            serve_store(request, session, driver, &mut held, &mut held_store,
                                &mut generation, &state, &sqlite_heap, deadline, cutoff,
                                cancelled, &fence, None, "tos_knowledge_catalog")?;
                            return Ok(false);
                        }
                        serve_whole_catalog(root, request, &isolation, session, driver,
                            &mut held, &mut held_store, &mut generation, &state,
                            &sqlite_heap, &resources, deadline, cutoff, cancelled, &fence)?;
                        Ok(true)
                    })?;
                    last_whole = whole;
                    continue;
                }
                if tool == "tos_knowledge_search_indexed_v2" {
                    let whole = with_selected_store_choice(request, &state, cutoff, |selected_store| {
                        if ordinary && selected_store != selected_store_at_startup {
                            return Err("Core indexed selected Store changed after READY");
                        }
                        if selected_store {
                            if !ordinary {
                                return Err("Core lazy selected QueryStore indexed owner unavailable");
                            }
                            if !store_indexed_ready {
                                return Err("Core selected QueryStore indexed FTS5 admission absent");
                            }
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
                                "tos_knowledge_search_indexed_v2",
                            )?;
                            return Ok(false);
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
                            cache_io.as_ref(),
                            cache_space.as_ref(),
                            deadline,
                            cutoff,
                            cancelled,
                            &fence,
                            arguments,
                        )?;
                        Ok(true)
                    })?;
                    last_whole = whole;
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
                            "tos_corpus_header",
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
            if matches!(operation, Operation::Evidence) {
                // Reference Evidence is an independently selected exact source
                // carrier; QueryStore selection must not divert this read.
                serve_whole_metadata(root, request, &isolation, session, driver,
                    &mut held, &mut held_store, &mut generation, &state,
                    &sqlite_heap, &resources, deadline, cutoff, cancelled, &fence, true)?;
                last_whole = true;
                continue;
            }
            if matches!(operation, Operation::KnowledgeHeader) {
                let whole = with_selected_store_choice(request, &state, cutoff, |selected_store| {
                    if selected_store {
                        serve_store(request, session, driver, &mut held, &mut held_store,
                            &mut generation, &state, &sqlite_heap, deadline, cutoff,
                            cancelled, &fence, None, "tos_knowledge_header")?;
                        return Ok(false);
                    }
                    serve_whole_metadata(root, request, &isolation, session, driver,
                        &mut held, &mut held_store, &mut generation, &state,
                        &sqlite_heap, &resources, deadline, cutoff, cancelled, &fence, false)?;
                    Ok(true)
                })?;
                last_whole = whole;
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
                        "tos_corpus_header",
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
