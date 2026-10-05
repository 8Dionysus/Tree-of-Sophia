//! Private native Core transport. Every limit and selected source is supplied by the caller.
#[path = "lazy_session.rs"]
mod lazy_session;
#[path = "probe_session.rs"]
mod probe_session;
#[path = "session_owner.rs"]
mod session_owner;
#[path = "session_startup.rs"]
mod session_startup;
#[path = "session_transport.rs"]
mod session_transport;
use serde::Deserialize;
use serde_json::Value;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicPtr, Ordering},
};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use tos_foundation::{JsonLimits, JsonMode, parse_json};

const INPUT_CAP: usize = 16 * 1024 * 1024;
const OUTPUT_CAP: usize = 64 * 1024 * 1024;
type Result<T> = std::result::Result<T, &'static str>;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonAdmission {
    max_bytes: usize,
    max_depth: usize,
    max_visits: usize,
    max_integer_digits: usize,
}
impl JsonAdmission {
    fn limits(&self) -> Result<JsonLimits> {
        if self.max_bytes == 0
            || self.max_depth == 0
            || self.max_visits == 0
            || self.max_integer_digits == 0
        {
            return Err("invalid Core JSON admission");
        }
        Ok(JsonLimits {
            max_bytes: self.max_bytes,
            max_depth: self.max_depth,
            max_visits: self.max_visits,
            max_integer_digits: self.max_integer_digits,
        })
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Admission {
    max_build_seconds: u64,
    tmpfs_quota_bytes: u64,
    inode_limit: u64,
    working_ram_bytes: u64,
    whole_max_rows: u64,
    whole_max_row_bytes: usize,
    whole_max_graph_bytes: usize,
    whole_max_catalog_bytes: usize,
    whole_max_catalog_inputs_bytes: usize,
    whole_max_state_bytes: usize,
    json: JsonAdmission,
    operation_seconds: f64,
    work_deadline_ns: u64,
    stage_ticket_fd: i32,
    cold: tos_compiler::ColdOpenLimits,
    process: tos_compiler::NativeProcessLimits,
}
impl Admission {
    fn deadline(&self) -> Result<Instant> {
        if !self.operation_seconds.is_finite()
            || self.operation_seconds <= 5.0
            || self.max_build_seconds == 0
            || self.tmpfs_quota_bytes == 0
            || self.inode_limit == 0
            || self.working_ram_bytes == 0
            || self.whole_max_rows == 0
            || self.whole_max_row_bytes == 0
            || self.whole_max_graph_bytes == 0
            || self.whole_max_catalog_bytes == 0
            || self.whole_max_catalog_inputs_bytes == 0
            || self.whole_max_state_bytes == 0
            || self.stage_ticket_fd < 3
        {
            return Err("invalid Core resource admission");
        }
        if unsafe { libc::fcntl(self.stage_ticket_fd, libc::F_GETFD) } < 0 {
            return Err("Core inherited stage ticket absent");
        }
        if let Some(ambient) = std::env::var_os("ABYSS_STAGE_TICKET_FD") {
            if ambient.to_str().and_then(|v| v.parse::<i32>().ok()) != Some(self.stage_ticket_fd) {
                return Err("Core stage ticket binding");
            }
        }
        // Sample Instant before the kernel clock so conversion cannot extend
        // the caller's cutoff by time spent obtaining or decoding that clock.
        let instant_before_clock = Instant::now();
        let mut now = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut now) } != 0
            || now.tv_sec < 0
            || now.tv_nsec < 0
        {
            return Err("Core monotonic clock unavailable");
        }
        let now_ns = u64::try_from(now.tv_sec)
            .ok()
            .and_then(|n| n.checked_mul(1_000_000_000))
            .and_then(|n| n.checked_add(now.tv_nsec as u64))
            .ok_or("Core monotonic clock overflow")?;
        let remaining = self
            .work_deadline_ns
            .checked_sub(now_ns)
            .filter(|n| *n > 0)
            .ok_or("Core original deadline expired")?;
        let span = Duration::try_from_secs_f64(self.operation_seconds)
            .map_err(|_| "Core operation span")?;
        instant_before_clock
            .checked_add(Duration::from_nanos(remaining).min(span))
            .ok_or("Core deadline overflow")
    }
    fn whole(&self) -> Result<tos_compiler::native_snapshot::NativeWholeSnapshotLimits> {
        Ok(tos_compiler::native_snapshot::NativeWholeSnapshotLimits {
            max_rows: self.whole_max_rows,
            max_row_bytes: self.whole_max_row_bytes,
            max_graph_bytes: self.whole_max_graph_bytes,
            max_catalog_bytes: self.whole_max_catalog_bytes,
            max_catalog_inputs_bytes: self.whole_max_catalog_inputs_bytes,
            max_state_bytes: self.whole_max_state_bytes,
            json: self.json.limits()?,
        })
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Sources {
    index_path: PathBuf,
    philosophy_graph_projection_path: PathBuf,
    bibliographic_graph_path: PathBuf,
    entity_type_registry_path: PathBuf,
    relation_type_registry_path: PathBuf,
    philosophy_post_planting_audit_path: PathBuf,
    evidence_projection_path: PathBuf,
}
impl Sources {
    fn validate(&self) -> Result<()> {
        for p in [
            &self.index_path,
            &self.philosophy_graph_projection_path,
            &self.bibliographic_graph_path,
            &self.entity_type_registry_path,
            &self.relation_type_registry_path,
            &self.philosophy_post_planting_audit_path,
            &self.evidence_projection_path,
        ] {
            if !p.is_absolute() || p.as_os_str().len() > 8193 {
                return Err("Core selected source path");
            }
        }
        Ok(())
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    // Owner-produced accounting; never a caller quota or transport field.
    #[serde(skip)]
    caller_retained_state_bytes: usize,
    admission: Admission,
    source_paths: Sources,
    arguments: Value,
    query_store: QueryStoreSelection,
    query_store_limits: Option<QueryStoreLimits>,
    http: Option<crate::core_http_admission::HttpAdmission>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QueryStoreSelection {
    path: PathBuf,
    configured: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QueryStoreLimits {
    max_database_bytes: u64,
    max_input_bytes: u64,
    max_json_bytes: usize,
    max_rows: u64,
    max_work_steps: u64,
    max_sql_vm_steps: u64,
    sqlite_cache_kib: u32,
}
impl QueryStoreLimits {
    fn native(&self) -> Result<tos_query::source_diagnostic::Limits> {
        if self.max_database_bytes == 0
            || self.max_input_bytes == 0
            || self.max_json_bytes == 0
            || self.max_json_bytes > i64::MAX as usize
            || self.max_rows == 0
            || self.max_work_steps == 0
            || self.max_sql_vm_steps == 0
            || self.sqlite_cache_kib == 0
        {
            return Err("Core selected QueryStore explicit limits refused");
        }
        Ok(tos_query::source_diagnostic::Limits {
            max_input_bytes: self.max_input_bytes,
            max_json_bytes: self.max_json_bytes,
            max_rows: self.max_rows,
            max_work_steps: self.max_work_steps,
            max_sql_vm_steps: self.max_sql_vm_steps,
            sqlite_cache_kib: self.sqlite_cache_kib,
        })
    }
}
struct StoreAbort {
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
}
impl tos_query::AbortProbe for StoreAbort {
    fn reason(&self) -> Option<tos_query::AbortReason> {
        if self.cancelled.load(Ordering::Relaxed) {
            Some(tos_query::AbortReason::Cancelled)
        } else if Instant::now() >= self.deadline {
            Some(tos_query::AbortReason::DeadlineExceeded)
        } else {
            None
        }
    }
}
#[path = "core_legacy_query_executor.rs"]
mod legacy_query_executor;

fn selected_store_inputs(request: &Request) -> [(String, PathBuf); 5] {
    let s = &request.source_paths;
    [
        (
            "ToS/derived-exports/tos_corpus_index.min.json".into(),
            s.index_path.clone(),
        ),
        (
            "ToS/derived-exports/philosophy_graph_projection.min.json".into(),
            s.philosophy_graph_projection_path.clone(),
        ),
        (
            "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json".into(),
            s.bibliographic_graph_path.clone(),
        ),
        (
            "ToS/doctrine/semantic-interchange/entity-types.v1.json".into(),
            s.entity_type_registry_path.clone(),
        ),
        (
            "ToS/doctrine/semantic-interchange/relation-types.v1.json".into(),
            s.relation_type_registry_path.clone(),
        ),
    ]
}

fn selected_store_result(
    request: &Request,
    operation: &Operation,
    reply_fd: Option<i32>,
    deadline: Instant,
    cancelled: &Arc<AtomicBool>,
) -> Result<()> {
    let selected = request
        .query_store_limits
        .as_ref()
        .ok_or("Core selected QueryStore requires explicit limits")?;
    let limits = selected.native()?;
    bind_ticket(&request.admission)?;
    let _isolation =
        tos_compiler::private_tmpfs_stage::PrivateTmpfsStageIsolation::select_from_environment(
            request.admission.tmpfs_quota_bytes,
            request.admission.inode_limit,
            request.admission.working_ram_bytes,
        )
        .map_err(|_| "Core QueryStore private stage refused")?;
    request
        .admission
        .process
        .verify_current()
        .map_err(|_| "Core QueryStore live process envelope refused")?;
    let resources = crate::native_cold_resources::LinuxCgroupColdOpenResourceHold::acquire(
        request.admission.working_ram_bytes,
        deadline,
        cancelled.clone(),
    )
    .map_err(|_| "Core QueryStore actual kernel resources refused")?;
    let inputs = selected_store_inputs(request);
    let abort: Arc<dyn tos_query::AbortProbe> = Arc::new(StoreAbort {
        deadline,
        cancelled: cancelled.clone(),
    });
    let mut store = tos_query::source_diagnostic::LegacyStore::open_bounded(
        &request.query_store.path,
        &inputs,
        limits,
        selected.max_database_bytes,
        deadline,
        abort,
    )
    .map_err(|_| "Core selected QueryStore authentication refused")?;
    if let Operation::Serve(listen, max_connections) = operation {
        return legacy_query_executor::serve_legacy_store(
            store,
            &resources,
            request,
            listen,
            *max_connections,
            deadline,
            cancelled,
        );
    }
    let mut out = BoundedOutput::new(OUTPUT_CAP, deadline);
    out.literal(br#"{"schema_version":"tos_native_core_snapshot_result_v1","ok":true,"result":"#)?;
    match operation {
        Operation::CorpusHeader => out.value(
            &store
                .corpus_header_with_graph_views_bounded(request.admission.whole_max_state_bytes)
                .map_err(|_| "Core QueryStore corpus header refused")?,
        )?,
        Operation::KnowledgeHeader => out.value(&store.graph_header)?,
        Operation::QueryCall(tool, arguments) => {
            out.value(
                &store
                    .query_call(tool, arguments, request.admission.whole_max_state_bytes)
                    .map_err(|_| "Core selected QueryStore generic query refused")?,
            )?;
        }
        Operation::Addressed(_) => {
            return Err(
                "addressed update parent has no captured source inputs; run a complete graph build",
            );
        }
        Operation::Graph | Operation::Snapshot => {
            if matches!(operation, Operation::Snapshot) {
                out.literal(br#"{"graph":"#)?;
            }
            let mut graph = BoundedOutput::new(request.admission.whole_max_graph_bytes, deadline);
            graph.literal(b"{")?;
            for (key, value) in store
                .graph_header
                .as_object()
                .ok_or("Core QueryStore graph header object")?
            {
                if matches!(key.as_str(), "nodes" | "relations") {
                    continue;
                }
                graph.value(key)?;
                graph.literal(b":")?;
                graph.value(value)?;
                graph.literal(b",")?;
            }
            graph.literal(br#""nodes":["#)?;
            for relations in [false, true] {
                if relations {
                    graph.literal(br#"],"relations":["#)?;
                }
                let mut first = true;
                store
                    .visit_knowledge_table(relations, |value| {
                        if !first {
                            graph.literal(b",").map_err(|message| {
                                tos_query::source_diagnostic::DiagnosticError(message.into())
                            })?;
                        }
                        graph.value(value).map_err(|message| {
                            tos_query::source_diagnostic::DiagnosticError(message.into())
                        })?;
                        first = false;
                        Ok(())
                    })
                    .map_err(|_| "Core QueryStore ordered graph rows refused")?;
            }
            graph.literal(b"]}")?;
            out.literal(&graph.bytes)?;
            if matches!(operation, Operation::Snapshot) {
                let mut catalog =
                    BoundedOutput::new(request.admission.whole_max_catalog_bytes, deadline);
                catalog.value(&store.catalog)?;
                out.literal(br#", "catalog":"#)?;
                out.literal(&catalog.bytes)?;
                out.literal(b"}")?;
            }
            out.literal(br#", "state_reused":false,"state_profile":"tos_query_store_v1""#)?;
        }
        _ => return Err("Core QueryStore operation unsupported"),
    }
    out.literal(b"}\n")?;
    let bytes = out.bytes;
    store
        .verify_currentness()
        .map_err(|_| "Core QueryStore final currentness refused")?;
    resources
        .check_current(deadline, cancelled.as_ref())
        .map_err(|_| "Core QueryStore final resources refused")?;
    request
        .admission
        .process
        .verify_current()
        .map_err(|_| "Core QueryStore final process refused")?;
    if matches!(operation, Operation::Graph | Operation::Snapshot) {
        send_state_marker(reply_fd.ok_or("Core QueryStore reply descriptor absent")?, None,
            br#"{"role":"tos-native-query-store-snapshot-v1","schema_version":"tos_query_store_v1"}"#, deadline)?;
    }
    disclose_bytes(&bytes, deadline)?;
    store
        .verify_currentness()
        .map_err(|_| "Core QueryStore post-disclosure currentness refused")?;
    resources
        .check_current(deadline, cancelled.as_ref())
        .map_err(|_| "Core QueryStore post-disclosure resources refused")?;
    request
        .admission
        .process
        .verify_current()
        .map_err(|_| "Core QueryStore post-disclosure process refused")?;
    Ok(())
}

struct Selection {
    root: PathBuf,
    operation: String,
    state_fd: Option<i32>,
    reply_fd: Option<i32>,
    session_control_fd: Option<i32>,
    work_deadline_ns: u64,
}
fn selection(args: &[String]) -> Result<Option<Selection>> {
    let claimed = args.first().is_some_and(|a| a == "core-snapshot")
        || args.first().is_some_and(|a| a == "--root")
            && args.get(2).is_some_and(|a| a == "core-snapshot");
    if !claimed {
        return Ok(None);
    }
    let mut root = None;
    let mut operation = None;
    let mut state_fd = None;
    let mut reply_fd = None;
    let mut session_control_fd = None;
    let mut work_deadline_ns = None;
    let mut command = false;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "core-snapshot" {
            if command {
                return Err("duplicate Core command");
            }
            command = true;
            i += 1;
            continue;
        }
        let value = args.get(i + 1).ok_or("Core selector value absent")?;
        match args[i].as_str() {
            "--work-deadline-ns" if work_deadline_ns.is_none() => {
                work_deadline_ns = Some(
                    value
                        .parse::<u64>()
                        .map_err(|_| "Core original deadline selector")?,
                );
            }
            "--root" if root.is_none() => root = Some(PathBuf::from(value)),
            "--operation" if operation.is_none() => operation = Some(value.clone()),
            "--snapshot-state-fd" if state_fd.is_none() => {
                state_fd = Some(value.parse::<i32>().map_err(|_| "Core state FD")?)
            }
            "--session-control-fd" if session_control_fd.is_none() => {
                session_control_fd = Some(
                    value
                        .parse::<i32>()
                        .map_err(|_| "Core session FD selector")?,
                );
            }
            "--state-reply-fd" if reply_fd.is_none() => {
                reply_fd = Some(value.parse::<i32>().map_err(|_| "Core reply FD")?)
            }
            _ => return Err("unknown or duplicate Core selector"),
        }
        i += 2;
    }
    let root = root
        .filter(|p| p.is_absolute())
        .ok_or("Core root must be absolute")?;
    if state_fd.is_some_and(|n| n < 3)
        || reply_fd.is_some_and(|n| n < 3)
        || state_fd.is_some() && state_fd == reply_fd
    {
        return Err("Core descriptor selectors");
    }
    if matches!(
        operation.as_deref(),
        Some("tos_native_session" | "tos_native_probe_session" | "tos_native_lazy_session")
    ) != session_control_fd.is_some()
        || session_control_fd.is_some_and(|fd| fd < 3)
        || session_control_fd.is_some() && (state_fd.is_some() || reply_fd.is_some())
    {
        return Err("Core exact session selector association");
    }
    Ok(Some(Selection {
        root,
        operation: operation.ok_or("Core operation absent")?,
        state_fd,
        reply_fd,
        session_control_fd,
        work_deadline_ns: work_deadline_ns
            .filter(|n| *n > 0)
            .ok_or("Core original deadline selector required")?,
    }))
}
fn read_input_with_visits(
    input: &mut dyn Read,
    deadline: Instant,
    cap: usize,
) -> Result<(Vec<u8>, usize)> {
    active(deadline)?;
    let previous = unsafe { libc::fcntl(0, libc::F_GETFL) };
    if previous < 0 || unsafe { libc::fcntl(0, libc::F_SETFL, previous | libc::O_NONBLOCK) } < 0 {
        return Err("Core stdin descriptor mode refused");
    }
    let mut raw = Vec::new();
    let read = (|| -> Result<()> {
        let mut buffer = [0_u8; 65536];
        loop {
            active(deadline)?;
            let remaining = (cap + 1).saturating_sub(raw.len());
            if remaining == 0 {
                return Err("Core request byte cap");
            }
            match input.read(&mut buffer[..remaining.min(65536)]) {
                Ok(0) => return Ok(()),
                Ok(count) => raw.extend_from_slice(&buffer[..count]),
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::Interrupted | std::io::ErrorKind::WouldBlock
                    ) =>
                {
                    std::thread::sleep(Duration::from_millis(1))
                }
                Err(_) => return Err("Core request read"),
            }
        }
    })();
    if unsafe { libc::fcntl(0, libc::F_SETFL, previous) } < 0 {
        return Err("Core stdin descriptor restoration refused");
    }
    read?;
    active(deadline)?;
    if raw.len() > cap {
        return Err("Core request byte cap");
    }
    // Parse the small admission envelope under fixed transport limits before trusting its limits.
    let guard = JsonLimits {
        max_bytes: cap,
        max_depth: 128,
        max_visits: cap,
        max_integer_digits: 4096,
    };
    active(deadline)?;
    let parsed = parse_json(&raw, JsonMode::PublishedStrict, guard)
        .map_err(|_| "Core strict request JSON")?;
    let visits = parsed.visits();
    drop(parsed);
    active(deadline)?;
    Ok((raw, visits))
}
fn read_input(input: &mut dyn Read, deadline: Instant, cap: usize) -> Result<Vec<u8>> {
    read_input_with_visits(input, deadline, cap).map(|(raw, _)| raw)
}
fn read_request(input: &mut dyn Read, deadline: Instant) -> Result<Request> {
    let raw = read_input(input, deadline, INPUT_CAP)?;
    let request: Request = serde_json::from_slice(&raw).map_err(|_| "Core request shape")?;
    active(deadline)?;
    request.source_paths.validate()?;
    if !request.query_store.path.is_absolute() || request.query_store.path.as_os_str().len() > 8193
    {
        return Err("Core query store path must be absolute and bounded");
    }
    if !request.arguments.is_object() {
        return Err("Core arguments object required");
    }
    Ok(request)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NavigationArguments {
    bibliographic_only: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OnceArguments {
    include_catalog_inputs: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AddressedArguments {
    source_graph: String,
    source_id: String,
    source_record: Value,
    source_revision: String,
    expected_parent_revision: Option<String>,
    return_report: bool,
}
enum Operation {
    Serve(String, u64),
    QueryCall(String, Value),
    Graph,
    Snapshot,
    Once(bool),
    Addressed(AddressedArguments),
    CorpusIndex,
    Navigation(bool),
    Bibliographic,
    PhilosophyProjection,
    PhilosophyAuditPayload,
    Evidence,
    CorpusHeader,
    KnowledgeHeader,
    Exists(ExistsKind),
}
enum ExistsKind {
    Index,
    Philosophy,
    PhilosophyAudit,
    Evidence,
}
fn operation(id: &str, arguments: Value) -> Result<Operation> {
    let empty = || {
        if arguments.as_object().is_some_and(|o| o.is_empty()) {
            Ok(())
        } else {
            Err("Core operation takes no arguments")
        }
    };
    Ok(match id {
        "tos_native_call" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct CallArguments {
                tool: String,
                arguments: Value,
            }
            let args: CallArguments =
                serde_json::from_value(arguments).map_err(|_| "Core native call arguments")?;
            Operation::QueryCall(args.tool, args.arguments)
        }
        "tos_native_serve" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct ServeArguments {
                listen: String,
                max_connections: u64,
            }
            let args: ServeArguments =
                serde_json::from_value(arguments).map_err(|_| "Core HTTP listen arguments")?;
            if args.max_connections == 0 {
                return Err("Core HTTP connection allowance must be positive");
            }
            Operation::Serve(args.listen, args.max_connections)
        }
        "tos_knowledge_graph" => {
            empty()?;
            Operation::Graph
        }
        "tos_knowledge_snapshot" => {
            empty()?;
            Operation::Snapshot
        }
        "tos_knowledge_snapshot_once" => Operation::Once(
            serde_json::from_value::<OnceArguments>(arguments)
                .map_err(|_| "Core once arguments")?
                .include_catalog_inputs,
        ),
        "tos_knowledge_graph_addressed" => {
            let update: AddressedArguments =
                serde_json::from_value(arguments).map_err(|_| "Core addressed arguments")?;
            if !update.source_record.is_object() {
                return Err("Core addressed record object required");
            }
            Operation::Addressed(update)
        }
        "tos_source_navigation" => Operation::Navigation(
            serde_json::from_value::<NavigationArguments>(arguments)
                .map_err(|_| "Core navigation arguments")?
                .bibliographic_only,
        ),
        "tos_corpus_index" => {
            empty()?;
            Operation::CorpusIndex
        }
        "tos_bibliographic_graph" => {
            empty()?;
            Operation::Bibliographic
        }
        "tos_philosophy_projection" => {
            empty()?;
            Operation::PhilosophyProjection
        }
        "tos_philosophy_audit_payload" => {
            empty()?;
            Operation::PhilosophyAuditPayload
        }
        "tos_philosophy_audit_exists" => {
            empty()?;
            Operation::Exists(ExistsKind::PhilosophyAudit)
        }
        "tos_evidence_projection" => {
            empty()?;
            Operation::Evidence
        }
        "tos_corpus_header" => {
            empty()?;
            Operation::CorpusHeader
        }
        "tos_knowledge_header" => {
            empty()?;
            Operation::KnowledgeHeader
        }
        "tos_corpus_index_exists" => {
            empty()?;
            Operation::Exists(ExistsKind::Index)
        }
        "tos_philosophy_projection_exists" => {
            empty()?;
            Operation::Exists(ExistsKind::Philosophy)
        }
        "tos_evidence_projection_exists" => {
            empty()?;
            Operation::Exists(ExistsKind::Evidence)
        }
        _ => return Err("unknown Core operation"),
    })
}
static OPERATION_CANCELLED: AtomicPtr<AtomicBool> = AtomicPtr::new(std::ptr::null_mut());
extern "C" fn cancel_operation(_signal: libc::c_int) {
    let token = OPERATION_CANCELLED.load(Ordering::Acquire);
    if !token.is_null() {
        // Linux target atomic pointer and bool operations are lock-free. The
        // CLI guard holds the original Arc until both handlers are restored.
        unsafe {
            (*token).store(true, Ordering::Release);
        }
    }
}
struct SignalGuard {
    token: Arc<AtomicBool>,
    old_term: libc::sigaction,
    old_int: libc::sigaction,
}
impl SignalGuard {
    fn install() -> Result<Self> {
        let token = Arc::new(AtomicBool::new(false));
        let mut blocked: libc::sigset_t = unsafe { std::mem::zeroed() };
        let mut old_mask: libc::sigset_t = unsafe { std::mem::zeroed() };
        unsafe {
            libc::sigemptyset(&mut blocked);
            libc::sigaddset(&mut blocked, libc::SIGTERM);
            libc::sigaddset(&mut blocked, libc::SIGINT);
        }
        if unsafe { libc::pthread_sigmask(libc::SIG_BLOCK, &blocked, &mut old_mask) } != 0 {
            return Err("Core signal mask");
        }
        if unsafe { libc::sigismember(&old_mask, libc::SIGTERM) } != 0
            || unsafe { libc::sigismember(&old_mask, libc::SIGINT) } != 0
        {
            unsafe {
                libc::pthread_sigmask(libc::SIG_SETMASK, &old_mask, std::ptr::null_mut());
            }
            return Err("Core cancellation signals were already blocked");
        }
        if OPERATION_CANCELLED
            .compare_exchange(
                std::ptr::null_mut(),
                Arc::as_ptr(&token).cast_mut(),
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_err()
        {
            unsafe {
                libc::pthread_sigmask(libc::SIG_SETMASK, &old_mask, std::ptr::null_mut());
            }
            return Err("Core operation signal owner already selected");
        }
        let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
        action.sa_sigaction = cancel_operation as usize;
        unsafe {
            libc::sigemptyset(&mut action.sa_mask);
        }
        let mut old_term = unsafe { std::mem::zeroed() };
        let mut old_int = unsafe { std::mem::zeroed() };
        if unsafe { libc::sigaction(libc::SIGTERM, &action, &mut old_term) } != 0 {
            OPERATION_CANCELLED.store(std::ptr::null_mut(), Ordering::Release);
            unsafe {
                libc::pthread_sigmask(libc::SIG_SETMASK, &old_mask, std::ptr::null_mut());
            }
            return Err("Core TERM handler");
        }
        if unsafe { libc::sigaction(libc::SIGINT, &action, &mut old_int) } != 0 {
            unsafe {
                libc::sigaction(libc::SIGTERM, &old_term, std::ptr::null_mut());
            }
            OPERATION_CANCELLED.store(std::ptr::null_mut(), Ordering::Release);
            unsafe {
                libc::pthread_sigmask(libc::SIG_SETMASK, &old_mask, std::ptr::null_mut());
            }
            return Err("Core INT handler");
        }
        if unsafe { libc::pthread_sigmask(libc::SIG_SETMASK, &old_mask, std::ptr::null_mut()) } != 0
        {
            // Refuse without leaving the borrowed atomic pointer installed.
            unsafe {
                libc::sigaction(libc::SIGTERM, &old_term, std::ptr::null_mut());
                libc::sigaction(libc::SIGINT, &old_int, std::ptr::null_mut());
            }
            OPERATION_CANCELLED.store(std::ptr::null_mut(), Ordering::Release);
            return Err("Core signal mask restoration");
        }
        Ok(Self {
            token,
            old_term,
            old_int,
        })
    }
}
impl Drop for SignalGuard {
    fn drop(&mut self) {
        let mut blocked: libc::sigset_t = unsafe { std::mem::zeroed() };
        let mut old_mask: libc::sigset_t = unsafe { std::mem::zeroed() };
        unsafe {
            libc::sigemptyset(&mut blocked);
            libc::sigaddset(&mut blocked, libc::SIGTERM);
            libc::sigaddset(&mut blocked, libc::SIGINT);
            if libc::pthread_sigmask(libc::SIG_BLOCK, &blocked, &mut old_mask) != 0 {
                libc::_exit(3);
            }
            if libc::sigaction(libc::SIGTERM, &self.old_term, std::ptr::null_mut()) != 0
                || libc::sigaction(libc::SIGINT, &self.old_int, std::ptr::null_mut()) != 0
            {
                libc::_exit(3);
            }
            OPERATION_CANCELLED.store(std::ptr::null_mut(), Ordering::Release);
            if libc::pthread_sigmask(libc::SIG_SETMASK, &old_mask, std::ptr::null_mut()) != 0 {
                libc::_exit(3);
            }
        }
    }
}
fn active(deadline: Instant) -> Result<()> {
    let token = OPERATION_CANCELLED.load(Ordering::Acquire);
    if !token.is_null() && unsafe { (*token).load(Ordering::Acquire) } {
        return Err("Core original operation cancelled");
    }
    if Instant::now() >= deadline {
        Err("Core original deadline expired")
    } else {
        Ok(())
    }
}

// `Path.is_file()` semantics follow a selected symlink; O_PATH holds the exact
// resolved inode without reading payload or manufacturing source authority.
fn selected_is_file(path: &Path, deadline: Instant) -> Result<bool> {
    with_selected_is_file(path, deadline, |exists| Ok(exists))
}

// One fixed pathname buffer, five retained stat observations plus one syscall
// return temporary, descriptor owner, two comparison tuples and borrowed locals.
// No CString/Vec or std metadata pathname conversion allocates outside this bound.
const SELECTED_PROBE_METADATA_WORKSPACE: usize = 8194
    + 6 * std::mem::size_of::<libc::stat>()
    + std::mem::size_of::<std::fs::File>()
    + 2 * std::mem::size_of::<(u64, u64, u32, i64, i64, i64, i64, i64)>()
    + std::mem::size_of::<(&Path, &[u8], &std::fs::File, Instant)>()
    + 4 * std::mem::size_of::<usize>();
/// Selected metadata property under explicit CPython3.14 semantics. A bounded
/// stack pathname goes directly to stat/fstat/open, avoiding hidden heap path
/// conversions. O_PATH and exact inode metadata remain held through disclosure.
fn with_selected_is_file<T>(
    path: &Path,
    deadline: Instant,
    disclose: impl FnOnce(bool) -> Result<T>,
) -> Result<T> {
    with_selected_metadata_property(path, deadline, true, disclose)
}

// Existing selected-property owner also supports Path.exists() for default
// Store selection; both variants retain the actual resolved O_PATH through use.
fn with_selected_metadata_property<T>(
    path: &Path,
    deadline: Instant,
    regular_only: bool,
    disclose: impl FnOnce(bool) -> Result<T>,
) -> Result<T> {
    use std::os::{
        fd::{AsRawFd, FromRawFd},
        unix::ffi::OsStrExt,
    };
    active(deadline)?;
    let bytes = path.as_os_str().as_bytes();
    if bytes.len() > 8193 {
        return Err("Core selected path bound");
    }
    // Python3.14 is_file returns false for an embedded-NUL path too. Never
    // pass a truncated name to a syscall and accidentally inspect another file.
    if bytes.contains(&0) {
        let result = disclose(false)?;
        active(deadline)?;
        return Ok(result);
    }
    let mut name = [0_u8; 8194];
    name[..bytes.len()].copy_from_slice(bytes);
    let named = || -> Option<libc::stat> {
        let mut metadata: libc::stat = unsafe { std::mem::zeroed() };
        if unsafe { libc::stat(name.as_ptr().cast(), &mut metadata) } == 0 {
            Some(metadata)
        } else {
            None
        }
    };
    let matches_property =
        |m: &libc::stat| !regular_only || m.st_mode & libc::S_IFMT == libc::S_IFREG;
    let before = match named() {
        Some(m) if matches_property(&m) => m,
        _ => {
            active(deadline)?;
            let result = disclose(false)?;
            if named().is_some_and(|m| matches_property(&m)) {
                return Err("Core selected false existence changed during disclosure");
            }
            active(deadline)?;
            return Ok(result);
        }
    };
    let fd = unsafe { libc::open(name.as_ptr().cast(), libc::O_PATH | libc::O_CLOEXEC) };
    if fd < 0 {
        return Err("Core selected existence hold");
    }
    let file = unsafe { std::fs::File::from_raw_fd(fd) };
    let held_stat = || -> Result<libc::stat> {
        let mut metadata: libc::stat = unsafe { std::mem::zeroed() };
        if unsafe { libc::fstat(file.as_raw_fd(), &mut metadata) } == 0 {
            Ok(metadata)
        } else {
            Err("Core selected held metadata")
        }
    };
    let identity = |m: &libc::stat| {
        (
            m.st_dev,
            m.st_ino,
            m.st_mode,
            m.st_size,
            m.st_mtime,
            m.st_mtime_nsec,
            m.st_ctime,
            m.st_ctime_nsec,
        )
    };
    let held = held_stat()?;
    let after = named().ok_or("Core selected existence changed")?;
    if !matches_property(&held)
        || identity(&before) != identity(&held)
        || identity(&held) != identity(&after)
    {
        return Err("Core selected existence changed");
    }
    active(deadline)?;
    let result = disclose(true)?;
    active(deadline)?;
    let current_held = held_stat()?;
    let current_named = named().ok_or("Core selected target changed during disclosure")?;
    if identity(&held) != identity(&current_held)
        || identity(&current_held) != identity(&current_named)
    {
        return Err("Core selected existence changed during disclosure");
    }
    active(deadline)?;
    Ok(result)
}

fn send_state(
    reply_fd: i32,
    state: Option<&tos_compiler::native_snapshot::ProducerIssuedCoreSnapshotState>,
    deadline: Instant,
) -> Result<()> {
    send_state_marker(reply_fd, state,
        br#"{"role":"tos-native-core-snapshot-state-v1","schema_version":"tos_native_core_snapshot_state_v1"}"#, deadline)
}
fn send_state_marker(
    reply_fd: i32,
    state: Option<&tos_compiler::native_snapshot::ProducerIssuedCoreSnapshotState>,
    marker: &[u8],
    deadline: Instant,
) -> Result<()> {
    use std::os::fd::AsRawFd;
    active(deadline)?;
    let mut kind: libc::c_int = 0;
    let mut size = std::mem::size_of_val(&kind) as libc::socklen_t;
    if unsafe {
        libc::getsockopt(
            reply_fd,
            libc::SOL_SOCKET,
            libc::SO_TYPE,
            (&mut kind as *mut libc::c_int).cast(),
            &mut size,
        )
    } != 0
        || kind != libc::SOCK_SEQPACKET
    {
        return Err("Core state reply socket type");
    }
    let mut domain: libc::c_int = 0;
    size = std::mem::size_of_val(&domain) as libc::socklen_t;
    if unsafe {
        libc::getsockopt(
            reply_fd,
            libc::SOL_SOCKET,
            libc::SO_DOMAIN,
            (&mut domain as *mut libc::c_int).cast(),
            &mut size,
        )
    } != 0
        || domain != libc::AF_UNIX
    {
        return Err("Core state reply socket domain");
    }
    let mut peer: libc::ucred = unsafe { std::mem::zeroed() };
    size = std::mem::size_of_val(&peer) as libc::socklen_t;
    if unsafe {
        libc::getsockopt(
            reply_fd,
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut peer as *mut libc::ucred).cast(),
            &mut size,
        )
    } != 0
        || peer.uid != unsafe { libc::geteuid() }
        || peer.pid != unsafe { libc::getppid() }
    {
        return Err("Core state reply peer");
    }
    let mut iov = libc::iovec {
        iov_base: marker.as_ptr().cast_mut().cast(),
        iov_len: marker.len(),
    };
    // usize alignment meets cmsghdr alignment; CMSG_SPACE includes its padding.
    let control_len = unsafe { libc::CMSG_SPACE(std::mem::size_of::<i32>() as u32) } as usize;
    let mut control = [0usize; 8];
    if control_len > std::mem::size_of_val(&control) {
        return Err("Core state control size");
    }
    let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;
    if let Some(state) = state {
        msg.msg_control = control.as_mut_ptr().cast();
        msg.msg_controllen = control_len;
        unsafe {
            let header = libc::CMSG_FIRSTHDR(&msg);
            if header.is_null() {
                return Err("Core state control header");
            }
            (*header).cmsg_level = libc::SOL_SOCKET;
            (*header).cmsg_type = libc::SCM_RIGHTS;
            (*header).cmsg_len = libc::CMSG_LEN(std::mem::size_of::<i32>() as u32) as usize;
            std::ptr::write_unaligned(
                libc::CMSG_DATA(header).cast::<i32>(),
                state.as_fd().as_raw_fd(),
            );
        }
    }
    // A producer-authenticated reused result sends the same neutral marker
    // with no SCM_RIGHTS. SO_PASSCRED at the receiver still authenticates this
    // held child; the SDK verifies the terminal and expected count before
    // retaining its existing private descriptor.
    loop {
        active(deadline)?;
        let sent =
            unsafe { libc::sendmsg(reply_fd, &msg, libc::MSG_NOSIGNAL | libc::MSG_DONTWAIT) };
        if sent == marker.len() as isize {
            return active(deadline);
        }
        if sent >= 0 {
            return Err("Core state marker truncated");
        }
        let error = std::io::Error::last_os_error();
        if error.kind() == std::io::ErrorKind::Interrupted {
            continue;
        }
        if error.kind() != std::io::ErrorKind::WouldBlock {
            return Err("Core state descriptor send");
        }
        let mut poll = libc::pollfd {
            fd: reply_fd,
            events: libc::POLLOUT,
            revents: 0,
        };
        let milliseconds = deadline
            .saturating_duration_since(Instant::now())
            .as_millis()
            .min(50) as i32;
        if milliseconds == 0 {
            return Err("Core state send deadline");
        }
        if unsafe { libc::poll(&mut poll, 1, milliseconds) } < 0
            && std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted
        {
            return Err("Core state reply poll");
        }
    }
}

struct BoundedOutput {
    bytes: Vec<u8>,
    cap: usize,
    deadline: Instant,
}
impl BoundedOutput {
    fn reserved(cap: usize, deadline: Instant, bytes: usize) -> Result<Self> {
        active(deadline)?;
        if bytes > cap {
            return Err("Core output original state reservation");
        }
        let mut out = Self::new(cap, deadline);
        out.bytes
            .try_reserve_exact(bytes)
            .map_err(|_| "Core output state allocation")?;
        if out.bytes.capacity() > cap {
            return Err("Core output actual state capacity");
        }
        active(deadline)?;
        Ok(out)
    }

    fn new(cap: usize, deadline: Instant) -> Self {
        Self {
            bytes: Vec::new(),
            cap: cap.min(OUTPUT_CAP),
            deadline,
        }
    }
    fn literal(&mut self, raw: &[u8]) -> Result<()> {
        self.write_all(raw)
            .map_err(|_| "Core output bound or deadline")
    }
    fn value(&mut self, value: &impl serde::Serialize) -> Result<()> {
        serde_json::to_writer(self, value).map_err(|_| "Core output serialization")
    }
    fn owner_value(
        &mut self,
        value: &tos_foundation::JsonValue,
        mut limits: JsonLimits,
    ) -> Result<()> {
        let remaining = self
            .cap
            .checked_sub(self.bytes.len())
            .ok_or("Core output remaining")?;
        limits.max_bytes = limits.max_bytes.min(remaining);
        active(self.deadline)?;
        let encoded = tos_foundation::emit_value_preserved_json(value, limits)
            .map_err(|_| "Core owner value output bound")?;
        self.literal(&encoded)
    }
}
impl Write for BoundedOutput {
    fn write(&mut self, raw: &[u8]) -> std::io::Result<usize> {
        active(self.deadline).map_err(std::io::Error::other)?;
        if self
            .bytes
            .len()
            .checked_add(raw.len())
            .is_none_or(|n| n > self.cap)
        {
            return Err(std::io::Error::other("Core output bound or deadline"));
        }
        self.bytes
            .try_reserve_exact(raw.len())
            .map_err(std::io::Error::other)?;
        self.bytes.extend_from_slice(raw);
        Ok(raw.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn encode_catalog_inputs(
    out: &mut BoundedOutput,
    inputs: &tos_compiler::prepared_catalog_semantics::CatalogInputs,
    json: JsonLimits,
) -> Result<()> {
    out.literal(br#"{"header":"#)?;
    out.owner_value(&inputs.header, json)?;
    out.literal(br#", "entity_type_registry":"#)?;
    out.owner_value(&inputs.entity_registry, json)?;
    out.literal(br#", "relation_type_registry":"#)?;
    out.owner_value(&inputs.relation_registry, json)?;
    out.literal(br#", "lenses":["#)?;
    for (index, lens) in inputs.lenses.iter().enumerate() {
        if index != 0 {
            out.literal(b",")?;
        }
        out.owner_value(lens, json)?;
    }
    out.literal(br#"],"source_order_profile":"#)?;
    out.value(&inputs.source_order_profile)?;
    out.literal(b"}")
}
fn fresh(root: &Path, stem: &str) -> Result<PathBuf> {
    let path = root.join(format!("{stem}-{}.sqlite3", std::process::id()));
    match std::fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(path),
        _ => Err("Core private stage path is not fresh"),
    }
}
fn source_paths(s: &Sources) -> tos_compiler::PublicCaptureInputPaths {
    tos_compiler::PublicCaptureInputPaths {
        index_path: s.index_path.clone(),
        philosophy_graph_projection_path: s.philosophy_graph_projection_path.clone(),
        bibliographic_graph_path: s.bibliographic_graph_path.clone(),
        entity_type_registry_path: s.entity_type_registry_path.clone(),
        relation_type_registry_path: s.relation_type_registry_path.clone(),
        philosophy_post_planting_audit_path: s.philosophy_post_planting_audit_path.clone(),
        evidence_projection_path: s.evidence_projection_path.clone(),
    }
}
fn bind_ticket(admission: &Admission) -> Result<()> {
    if unsafe { libc::fcntl(admission.stage_ticket_fd, libc::F_GETFD) } < 0 {
        return Err("Core inherited stage ticket absent");
    }
    // Entry is called directly from the single-threaded CLI before any producer
    // is selected. The sealed ticket owner validates the actual FD after this binding.
    unsafe {
        std::env::set_var(
            "ABYSS_STAGE_TICKET_FD",
            admission.stage_ticket_fd.to_string(),
        );
    }
    Ok(())
}
fn whole_result_bytes(
    snapshot: &tos_compiler::native_snapshot::NativeKnowledgeSnapshot,
    operation: &Operation,
    admission: &Admission,
    deadline: Instant,
    state_reused: bool,
) -> Result<Vec<u8>> {
    let mut out = BoundedOutput::new(OUTPUT_CAP, deadline);
    out.literal(br#"{"schema_version":"tos_native_core_snapshot_result_v1","ok":true,"result":"#)?;
    match operation {
        Operation::Graph => out.value(&snapshot.graph)?,
        Operation::Snapshot => {
            out.literal(br#"{"graph":"#)?;
            out.value(&snapshot.graph)?;
            out.literal(br#", "catalog":"#)?;
            out.value(&snapshot.catalog)?;
            out.literal(b"}")?;
        }
        Operation::Once(include_inputs) => {
            out.literal(br#"{"graph":"#)?;
            out.value(&snapshot.graph)?;
            out.literal(br#", "catalog":"#)?;
            out.value(&snapshot.catalog)?;
            out.literal(br#", "source_state":"#)?;
            out.value(&snapshot.source_state)?;
            if *include_inputs {
                let inputs = snapshot
                    .catalog_inputs
                    .as_ref()
                    .ok_or("Core requested CatalogInputs absent")?;
                out.literal(br#", "catalog_inputs":"#)?;
                encode_catalog_inputs(&mut out, inputs, admission.json.limits()?)?;
            }
            out.literal(b"}")?;
        }
        _ => return Err("Core whole output operation mismatch"),
    }
    if matches!(operation, Operation::Graph | Operation::Snapshot) {
        out.literal(br#", "state_reused":"#)?;
        out.value(&state_reused)?;
    }
    out.literal(b"}\n")?;
    active(deadline)?;
    Ok(out.bytes)
}

fn reused_result_bytes(
    reused: &tos_compiler::native_snapshot::ReusedNativeKnowledgeSnapshot,
    operation: &Operation,
    deadline: Instant,
) -> Result<Vec<u8>> {
    let mut out = BoundedOutput::new(OUTPUT_CAP, deadline);
    out.literal(br#"{"schema_version":"tos_native_core_snapshot_result_v1","ok":true,"result":"#)?;
    match operation {
        Operation::Graph => out.value(&reused.graph)?,
        Operation::Snapshot => {
            out.literal(br#"{"graph":"#)?;
            out.value(&reused.graph)?;
            out.literal(br#", "catalog":"#)?;
            out.value(&reused.catalog)?;
            out.literal(b"}")?;
        }
        _ => return Err("Core reused output operation mismatch"),
    }
    // Only the producer's authenticated currentness gate can supply this type.
    // Its caller retains the old state descriptor; no replacement is emitted.
    out.literal(br#", "state_reused":true}"#)?;
    out.literal(b"\n")?;
    active(deadline)?;
    Ok(out.bytes)
}

struct HeldWholeSnapshot {
    capture: tos_compiler::PublicCapture,
    completed: Option<tos_compiler::native_snapshot::CompletedNativeSnapshot>,
    snapshot: WholeSnapshotOutcome,
    _isolation: tos_compiler::private_tmpfs_stage::PrivateTmpfsStageIsolation,
}
enum WholeSnapshotOutcome {
    Built(tos_compiler::native_snapshot::NativeKnowledgeSnapshot),
    Reused(tos_compiler::native_snapshot::ReusedNativeKnowledgeSnapshot),
}
fn build_whole_selected(
    root: &Path,
    request: &Request,
    deadline: Instant,
    include_inputs: bool,
    retain_state: bool,
    state_fd: Option<i32>,
    cancelled: &Arc<AtomicBool>,
) -> Result<HeldWholeSnapshot> {
    active(deadline)?;
    let limits = tos_compiler::native_snapshot_manifest::portable_native_snapshot_limits(
        request.admission.max_build_seconds,
    )
    .map_err(|_| "Core native build profile refused")?;
    bind_ticket(&request.admission)?;
    let isolation =
        tos_compiler::private_tmpfs_stage::PrivateTmpfsStageIsolation::select_from_environment(
            request.admission.tmpfs_quota_bytes,
            request.admission.inode_limit,
            request.admission.working_ram_bytes,
        )
        .map_err(|_| "Core private stage ticket refused")?;
    active(deadline)?;
    let capture_path = fresh(isolation.root(), "tos-core-capture")?;
    let capture = tos_compiler::PublicCapture::create_runtime_selected(
        root,
        &source_paths(&request.source_paths),
        &capture_path,
        limits.capture,
        deadline,
        cancelled.clone(),
    )
    .map_err(|_| "Core selected source capture refused")?;
    if let Some(fd) = state_fd {
        if fd < 3 {
            return Err("Core state descriptor refused");
        }
        // The inherited descriptor stays borrowed for the entire CLI. The
        // producer validates its seals, issuer receipt and complete same-cut
        // currentness; an unmatched descriptor falls through to a real build.
        let reused = tos_compiler::native_snapshot::reuse_native_knowledge_snapshot_if_current(
            unsafe { std::os::fd::BorrowedFd::borrow_raw(fd) },
            &capture,
            request.admission.whole_max_state_bytes,
            request.admission.json.limits()?,
            include_inputs,
            deadline,
            cancelled.as_ref(),
        )
        .map_err(|_| "Core previous state authentication refused")?;
        if let Some(reused) = reused {
            active(deadline)?;
            return Ok(HeldWholeSnapshot {
                capture,
                completed: None,
                snapshot: WholeSnapshotOutcome::Reused(reused),
                _isolation: isolation,
            });
        }
    }
    let candidate = fresh(isolation.root(), "tos-core-snapshot")?;
    let (completed, snapshot) =
        tos_compiler::native_snapshot::build_native_knowledge_snapshot_from_capture(
            &capture,
            &candidate,
            tos_compiler::native_snapshot_manifest::RUNTIME_DATA_DECLARATION,
            &isolation,
            limits,
            request.admission.whole()?,
            include_inputs,
            retain_state,
            deadline,
            cancelled.as_ref(),
        )
        .map_err(|error| {
            compiler_terminal_diagnostic("Core whole snapshot", &error);
            "Core native whole snapshot refused"
        })?;
    active(deadline)?;
    // The retained capture, completed receipt, actual source closure and state
    // remain alive together until the final caller disclosure checks.
    Ok(HeldWholeSnapshot {
        capture,
        completed: Some(completed),
        snapshot: WholeSnapshotOutcome::Built(snapshot),
        _isolation: isolation,
    })
}
fn existence_result(kind: ExistsKind, request: &Request, deadline: Instant) -> Result<Vec<u8>> {
    let path = match kind {
        ExistsKind::Index => &request.source_paths.index_path,
        ExistsKind::Philosophy => &request.source_paths.philosophy_graph_projection_path,
        ExistsKind::Evidence => &request.source_paths.evidence_projection_path,
        ExistsKind::PhilosophyAudit => &request.source_paths.philosophy_post_planting_audit_path,
    };
    let exists = selected_is_file(path, deadline)?;
    let mut out = BoundedOutput::new(OUTPUT_CAP, deadline);
    out.literal(br#"{"schema_version":"tos_native_core_snapshot_result_v1","ok":true,"result":"#)?;
    out.value(&exists)?;
    out.literal(b"}\n")?;
    active(deadline)?;
    Ok(out.bytes)
}

// This dedicated CLI owns no parent descriptor: O_NONBLOCK is temporary on
// the inherited open-file description and its exact previous flags are restored
// before returning. Errors are terminal; callers must not subsequently print
// an unbounded diagnostic on the restored blocking stream.
fn disclose_bytes(mut bytes: &[u8], deadline: Instant) -> Result<()> {
    active(deadline)?;
    let previous = unsafe { libc::fcntl(1, libc::F_GETFL) };
    if previous < 0 || unsafe { libc::fcntl(1, libc::F_SETFL, previous | libc::O_NONBLOCK) } < 0 {
        return Err("Core output descriptor mode refused");
    }
    let delivery = (|| {
        while !bytes.is_empty() {
            active(deadline)?;
            let count = unsafe { libc::write(1, bytes.as_ptr().cast(), bytes.len()) };
            if count > 0 {
                bytes = &bytes[count as usize..];
            } else if count == 0 {
                return Err("Core output made no progress");
            } else {
                let error = std::io::Error::last_os_error();
                if !matches!(
                    error.kind(),
                    std::io::ErrorKind::Interrupted | std::io::ErrorKind::WouldBlock
                ) {
                    return Err("Core output delivery refused");
                }
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        active(deadline)
    })();
    if unsafe { libc::fcntl(1, libc::F_SETFL, previous) } < 0 {
        return Err("Core output descriptor restoration refused");
    }
    delivery
}

struct HeldCarrierResult {
    capture: tos_compiler::PublicCapture,
    _isolation: tos_compiler::private_tmpfs_stage::PrivateTmpfsStageIsolation,
    bytes: Vec<u8>,
}

struct HeldEvidenceResult {
    evidence: tos_compiler::epistemic_evidence::CompletedEvidenceProjection,
    _isolation: tos_compiler::private_tmpfs_stage::PrivateTmpfsStageIsolation,
    bytes: Vec<u8>,
}

fn read_selected_evidence(
    root: &Path,
    request: &Request,
    deadline: Instant,
    cancelled: &Arc<AtomicBool>,
) -> Result<HeldEvidenceResult> {
    active(deadline)?;
    let limits = tos_compiler::native_snapshot_manifest::portable_native_snapshot_limits(
        request.admission.max_build_seconds,
    )
    .map_err(|_| "Core native evidence profile refused")?;
    bind_ticket(&request.admission)?;
    let isolation =
        tos_compiler::private_tmpfs_stage::PrivateTmpfsStageIsolation::select_from_environment(
            request.admission.tmpfs_quota_bytes,
            request.admission.inode_limit,
            request.admission.working_ram_bytes,
        )
        .map_err(|_| "Core private stage ticket refused")?;
    let staging = fresh(isolation.root(), "tos-core-evidence")?;
    let evidence = tos_compiler::epistemic_evidence::check_isolated_selected(
        root,
        &source_paths(&request.source_paths),
        &staging,
        limits.capture,
        deadline,
        cancelled.clone(),
    )
    .map_err(|error| {
        compiler_terminal_diagnostic("Core selected evidence owner check", &error);
        "Core selected evidence owner check refused"
    })?;
    let budget = tos_compiler::native_snapshot_carriers::CapturedCarrierReadBudget {
        max_rows: request.admission.whole_max_rows,
        max_input_bytes: limits.capture.max_input_bytes,
        max_output_bytes: request.admission.whole_max_graph_bytes.min(OUTPUT_CAP),
        json: request.admission.json.limits()?,
    };
    let raw = evidence
        .with_current(|view| {
            tos_compiler::native_snapshot_carriers::read_complete_evidence_projection(
                view,
                budget,
                deadline,
                cancelled.as_ref(),
            )
        })
        .map_err(|_| "Core complete selected evidence read refused")?;
    let mut out = BoundedOutput::new(OUTPUT_CAP, deadline);
    out.literal(br#"{"schema_version":"tos_native_core_snapshot_result_v1","ok":true,"result":"#)?;
    out.literal(&raw)?;
    out.literal(b"}\n")?;
    drop(raw);
    active(deadline)?;
    Ok(HeldEvidenceResult {
        evidence,
        _isolation: isolation,
        bytes: out.bytes,
    })
}
fn read_selected_carrier(
    root: &Path,
    request: &Request,
    carrier: tos_compiler::native_snapshot_carriers::CapturedCarrierRequest,
    role: tos_compiler::RuntimeCaptureRole,
    deadline: Instant,
    cancelled: &Arc<AtomicBool>,
) -> Result<HeldCarrierResult> {
    active(deadline)?;
    let limits = tos_compiler::native_snapshot_manifest::portable_native_snapshot_limits(
        request.admission.max_build_seconds,
    )
    .map_err(|_| "Core native carrier profile refused")?;
    bind_ticket(&request.admission)?;
    let isolation =
        tos_compiler::private_tmpfs_stage::PrivateTmpfsStageIsolation::select_from_environment(
            request.admission.tmpfs_quota_bytes,
            request.admission.inode_limit,
            request.admission.working_ram_bytes,
        )
        .map_err(|_| "Core private stage ticket refused")?;
    let path = fresh(isolation.root(), "tos-core-carrier")?;
    let capture = tos_compiler::PublicCapture::create_runtime_carrier_selected(
        root,
        &source_paths(&request.source_paths),
        role,
        &path,
        limits.capture,
        deadline,
        cancelled.clone(),
    )
    .map_err(|_| "Core selected carrier capture refused")?;
    let budget = tos_compiler::native_snapshot_carriers::CapturedCarrierReadBudget {
        max_rows: request.admission.whole_max_rows,
        max_input_bytes: limits.capture.max_input_bytes,
        max_output_bytes: request.admission.whole_max_graph_bytes.min(OUTPUT_CAP),
        json: request.admission.json.limits()?,
    };
    let raw = capture
        .with_captured_carriers(|view| {
            tos_compiler::native_snapshot_carriers::read_complete_captured_carrier(
                view, carrier, budget, deadline, cancelled,
            )
        })
        .map_err(|_| "Core complete selected carrier read refused")?;
    let mut out = BoundedOutput::new(OUTPUT_CAP, deadline);
    out.literal(br#"{"schema_version":"tos_native_core_snapshot_result_v1","ok":true,"result":"#)?;
    out.literal(&raw)?;
    out.literal(b"}\n")?;
    drop(raw);
    active(deadline)?;
    Ok(HeldCarrierResult {
        capture,
        _isolation: isolation,
        bytes: out.bytes,
    })
}

fn emit_value(value: &Value, deadline: Instant) -> Result<Vec<u8>> {
    let mut out = BoundedOutput::new(OUTPUT_CAP, deadline);
    out.literal(br#"{"schema_version":"tos_native_core_snapshot_result_v1","ok":true,"result":"#)?;
    out.value(value)?;
    out.literal(b"}\n")?;
    Ok(out.bytes)
}

fn run(
    selection: Selection,
    mut request: Request,
    deadline: Instant,
    cancelled: &Arc<AtomicBool>,
) -> Result<()> {
    use tos_compiler::RuntimeCaptureRole as R;
    use tos_compiler::native_snapshot_carriers::CapturedCarrierRequest as C;
    if request.admission.work_deadline_ns != selection.work_deadline_ns {
        return Err("Core request deadline differs from the original CLI cutoff");
    }
    let deadline = deadline.min(request.admission.deadline()?);
    active(deadline)?;
    request.caller_retained_state_bytes = request
        .caller_retained_state_bytes
        .checked_add(std::mem::size_of::<Selection>())
        .and_then(|n| n.checked_add(selection.root.capacity()))
        .and_then(|n| n.checked_add(selection.operation.capacity()))
        .ok_or("Core selected CLI state overflow")?;
    let operation = operation(&selection.operation, std::mem::take(&mut request.arguments))?;
    // The former QueryStore has independent five-input binding and selection
    // semantics. Retain its own authenticated SQL dispatcher when explicitly
    // selected or discovered; source capture remains a separate owner route.
    let uses_query_store = matches!(
        operation,
        Operation::Graph
            | Operation::Snapshot
            | Operation::Addressed(_)
            | Operation::KnowledgeHeader
            | Operation::CorpusHeader
            | Operation::Serve(_, _)
            | Operation::QueryCall(_, _)
    );
    let selected_store =
        uses_query_store && (request.query_store.configured || request.query_store.path.exists());
    // Lower exact carrier reads and one-shot bootstrap do not consult the
    // Reference QueryStore; preserve their independent selectors.
    if request.http.is_some()
        && !matches!(
            operation,
            Operation::Serve(_, _) | Operation::QueryCall(_, _)
        )
    {
        return Err("Core HTTP allowances supplied to a non-HTTP operation");
    }
    let retained = matches!(
        operation,
        Operation::Graph | Operation::Snapshot | Operation::Addressed(_)
    );
    if retained != selection.reply_fd.is_some() {
        return Err("Core retained result requires exactly one state reply socket");
    }
    if matches!(operation, Operation::Once(_)) && selection.state_fd.is_some() {
        return Err("Core once cannot borrow retained state");
    }
    if !retained && selection.state_fd.is_some() {
        return Err("Core lower carrier cannot borrow retained state");
    }
    if selected_store {
        match operation {
            Operation::CorpusHeader
            | Operation::KnowledgeHeader
            | Operation::Graph
            | Operation::Snapshot
            | Operation::Addressed(_)
            | Operation::QueryCall(_, _)
            | Operation::Serve(_, _) => {
                return selected_store_result(
                    &request,
                    &operation,
                    selection.reply_fd,
                    deadline,
                    cancelled,
                );
            }
            _ => unreachable!(),
        }
    }
    if let Operation::Exists(kind) = operation {
        return disclose_bytes(&existence_result(kind, &request, deadline)?, deadline);
    }
    if let Operation::QueryCall(tool, arguments) = &operation {
        if tool == "tos_native_resource_read" {
            let profile = request
                .http
                .as_ref()
                .ok_or("Core resource HTTP admission unavailable")?
                .profile()?;
            let fields = arguments
                .as_object()
                .filter(|fields| fields.len() == 2)
                .ok_or("Core resource argument unavailable")?;
            let uri = fields
                .get("uri")
                .and_then(Value::as_str)
                .ok_or("Core resource URI unavailable")?;
            let render = fields
                .get("render")
                .and_then(Value::as_bool)
                .ok_or("Core resource render flag unavailable")?;
            let resource = crate::mcp_resources::request(uri, profile)
                .map_err(|_| "Core native resource refused")?;
            // The maintained native parser owns the typed request now. Drop
            // both original transport DOMs before capture/model custody starts.
            drop(operation);
            return serve_selected_root(
                &selection.root,
                &request,
                "",
                0,
                deadline,
                cancelled,
                Some(SelectedRootCall::Resource(resource, render)),
                None,
            );
        }
        return serve_selected_root(
            &selection.root,
            &request,
            "",
            0,
            deadline,
            &cancelled,
            Some(SelectedRootCall::Tool(tool, arguments)),
            None,
        );
    }
    if let Operation::Serve(listen, max_connections) = &operation {
        return serve_selected_root(
            &selection.root,
            &request,
            listen,
            *max_connections,
            deadline,
            &cancelled,
            None,
            None,
        );
    }
    let carrier = match &operation {
        Operation::CorpusIndex => Some((C::CorpusIndex, R::Corpus)),
        // Reference's direct-source branch returns the complete index;
        // its QueryStore-only header branch is fenced above until native parity.
        Operation::CorpusHeader => Some((C::CorpusIndex, R::Corpus)),
        Operation::Navigation(bibliographic_only) => Some((
            C::SourceNavigation {
                bibliographic_only: *bibliographic_only,
            },
            R::Corpus,
        )),
        Operation::Bibliographic => Some((C::BibliographicGraph, R::Bibliographic)),
        Operation::PhilosophyProjection => Some((C::PhilosophyProjection, R::Philosophy)),
        Operation::PhilosophyAuditPayload => Some((C::PhilosophyAuditPayload, R::PhilosophyAudit)),
        _ => None,
    };
    if let Some((carrier, role)) = carrier {
        let held = read_selected_carrier(
            &selection.root,
            &request,
            carrier,
            role,
            deadline,
            &cancelled,
        )?;
        return held
            .capture
            .with_captured_carriers(|view| {
                view.verify_current()?;
                disclose_bytes(&held.bytes, deadline).map_err(tos_compiler::Error::Invalid)
            })
            .map_err(|_| "Core carrier disclosure currentness refused");
    }
    if matches!(operation, Operation::Evidence) {
        let held = read_selected_evidence(&selection.root, &request, deadline, cancelled)?;
        return held
            .evidence
            .with_current(|_| {
                disclose_bytes(&held.bytes, deadline).map_err(tos_compiler::Error::Invalid)
            })
            .map_err(|_| "Core evidence disclosure currentness refused");
    }
    if let Operation::Addressed(update) = &operation {
        let state_fd = selection
            .state_fd
            .ok_or("Core addressed requires previous producer state")?;
        let limits = tos_compiler::native_snapshot_manifest::portable_native_snapshot_limits(
            request.admission.max_build_seconds,
        )
        .map_err(|_| "Core native addressed profile refused")?;
        bind_ticket(&request.admission)?;
        let isolation =
            tos_compiler::private_tmpfs_stage::PrivateTmpfsStageIsolation::select_from_environment(
                request.admission.tmpfs_quota_bytes,
                request.admission.inode_limit,
                request.admission.working_ram_bytes,
            )
            .map_err(|_| "Core addressed stage refused")?;
        let capture = tos_compiler::PublicCapture::create_runtime_selected(
            &selection.root,
            &source_paths(&request.source_paths),
            &fresh(isolation.root(), "tos-core-addressed-capture")?,
            limits.capture,
            deadline,
            cancelled.clone(),
        )
        .map_err(|_| "Core addressed source capture refused")?;
        let (completed, snapshot, report) =
            tos_compiler::native_snapshot::build_native_addressed_knowledge_snapshot_from_capture(
                unsafe { std::os::fd::BorrowedFd::borrow_raw(state_fd) },
                &capture,
                &fresh(isolation.root(), "tos-core-addressed")?,
                tos_compiler::native_snapshot_manifest::RUNTIME_DATA_DECLARATION,
                &isolation,
                limits,
                request.admission.whole()?,
                false,
                tos_compiler::native_snapshot::NativeAddressedUpdate {
                    source_graph: &update.source_graph,
                    source_id: &update.source_id,
                    source_record: &update.source_record,
                    source_revision: &update.source_revision,
                    expected_parent_revision: update.expected_parent_revision.as_deref(),
                    return_report: update.return_report,
                },
                deadline,
                cancelled.as_ref(),
            )
            .map_err(|_| "Core native addressed transition refused")?;
        let mut out = BoundedOutput::new(OUTPUT_CAP, deadline);
        out.literal(
            br#"{"schema_version":"tos_native_core_snapshot_result_v1","ok":true,"result":"#,
        )?;
        if update.return_report {
            out.literal(br#"{"graph":"#)?;
            out.value(&snapshot.graph)?;
            out.literal(br#", "report":"#)?;
            out.value(&report.ok_or("Core addressed report absent")?.value)?;
            out.literal(b"}")?;
        } else {
            out.value(&snapshot.graph)?;
        }
        out.literal(br#", "state_reused":false}"#)?;
        out.literal(b"\n")?;
        let bytes = out.bytes;
        return completed
            .with_capture_carriers(&capture, |_| {
                send_state(
                    selection
                        .reply_fd
                        .ok_or(tos_compiler::Error::Invalid("Core state reply absent"))?,
                    snapshot.state.as_ref(),
                    deadline,
                )
                .map_err(tos_compiler::Error::Invalid)?;
                disclose_bytes(&bytes, deadline).map_err(tos_compiler::Error::Invalid)
            })
            .map_err(|_| "Core addressed disclosure currentness refused");
    }
    let include_inputs = matches!(operation, Operation::Once(true));
    let held = build_whole_selected(
        &selection.root,
        &request,
        deadline,
        include_inputs,
        retained,
        selection.state_fd,
        &cancelled,
    )?;
    let (bytes, state) = match &held.snapshot {
        WholeSnapshotOutcome::Built(snapshot) => {
            let bytes = if matches!(operation, Operation::KnowledgeHeader) {
                let graph = snapshot
                    .graph
                    .as_object()
                    .ok_or("Core graph header object absent")?;
                let header = graph
                    .iter()
                    .filter(|(key, _)| !matches!(key.as_str(), "nodes" | "relations"))
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect();
                emit_value(&Value::Object(header), deadline)?
            } else {
                whole_result_bytes(snapshot, &operation, &request.admission, deadline, false)?
            };
            (bytes, snapshot.state.as_ref())
        }
        WholeSnapshotOutcome::Reused(reused) => {
            (reused_result_bytes(reused, &operation, deadline)?, None)
        }
    };
    // The exact completed capture stays held through both descriptor disclosure
    // and final stdout write. The receiver commits only after child EOF/join.
    let deliver = || -> tos_compiler::Result<()> {
        if let Some(fd) = selection.reply_fd {
            send_state(fd, state, deadline).map_err(tos_compiler::Error::Invalid)?;
        }
        disclose_bytes(&bytes, deadline).map_err(tos_compiler::Error::Invalid)
    };
    if let Some(completed) = held.completed.as_ref() {
        completed.with_capture_carriers(&held.capture, |_| deliver())
    } else {
        held.capture.with_captured_carriers(|_| deliver())
    }
    .map_err(|_| "Core whole disclosure currentness refused")
}

/// Installed private CLI entry. Failures are terminal and never invoke the
/// Reference graph builder, discover another source root, or print on a
/// restored potentially blocking output stream after a partial disclosure.
// The maintained native main owns this Vec through the entire call. Accept
// its actual owner, not a slice which hides the live allocation capacity.
pub fn run_if_requested(args: &Vec<String>, input: &mut dyn Read) -> Option<i32> {
    match selection(args) {
        Ok(None) => None,
        Err(message) => {
            terminal_diagnostic(message);
            Some(2)
        }
        Ok(Some(selection)) => Some(
            match (|| {
                let deadline = original_cli_deadline(selection.work_deadline_ns)?;
                let signal = SignalGuard::install()?;
                let mut argv_state = std::mem::size_of::<Vec<String>>()
                    .checked_add(std::mem::size_of::<SignalGuard>())
                    .ok_or("Core CLI signal owner state overflow")?
                    .checked_add(
                        args.capacity()
                            .checked_mul(std::mem::size_of::<String>())
                            .ok_or("Core CLI argument capacity overflow")?,
                    )
                    .ok_or("Core CLI argument state overflow")?;
                for argument in args {
                    argv_state = argv_state
                        .checked_add(argument.capacity())
                        .ok_or("Core CLI argument state overflow")?;
                }
                if matches!(
                    selection.operation.as_str(),
                    "tos_native_session" | "tos_native_probe_session" | "tos_native_lazy_session"
                ) {
                    let probe = selection.operation == "tos_native_probe_session";
                    let lazy = selection.operation == "tos_native_lazy_session";
                    let (raw, startup_visits) = read_input_with_visits(input, deadline, 65536)?;
                    let startup_bytes = raw.len();
                    let startup: session_startup::Startup =
                        serde_json::from_slice(&raw).map_err(|_| "Core session startup DTO")?;
                    drop(raw);
                    let argv_state = argv_state
                        .checked_add(std::mem::size_of::<Selection>())
                        .and_then(|n| n.checked_add(selection.root.capacity()))
                        .and_then(|n| n.checked_add(selection.operation.capacity()))
                        .ok_or("Core session selected owner state overflow")?;
                    let (mut request, limits, whole) = if lazy {
                        startup.into_lazy_owner_request(
                            selection.work_deadline_ns,
                            argv_state,
                            startup_bytes,
                        )?
                    } else if probe {
                        startup.into_probe_owner_request(
                            selection.work_deadline_ns,
                            argv_state,
                            startup_bytes,
                        )?
                    } else {
                        startup.into_owner_request(
                            selection.work_deadline_ns,
                            argv_state,
                            startup_bytes,
                        )?
                    };
                    let deadline = deadline.min(request.admission.deadline()?);
                    let control = crate::private_stage_run::verify_issued_consumer_control(
                        selection
                            .session_control_fd
                            .ok_or("Core session control selector absent")?,
                        whole,
                        selection.work_deadline_ns,
                    )
                    .map_err(|_| "Core actual issued session control refused")?;
                    let session = session_owner::Session {
                        control,
                        limits,
                        startup_visits,
                    };
                    request.caller_retained_state_bytes = request
                        .caller_retained_state_bytes
                        .checked_add(session.retained_state_upper_bound()?)
                        .ok_or("Core session original caller/control state overflow")?;
                    if lazy {
                        return lazy_session::run(
                            &selection.root,
                            &session,
                            &request,
                            deadline,
                            &signal.token,
                            startup_bytes,
                        );
                    }
                    if probe {
                        return probe_session::run(
                            &session,
                            &request,
                            deadline,
                            &signal.token,
                            startup_bytes,
                        );
                    }
                    return serve_selected_root(
                        &selection.root,
                        &request,
                        "",
                        0,
                        deadline,
                        &signal.token,
                        None,
                        Some(&session),
                    );
                }
                let mut request = read_request(input, deadline)?;
                request.caller_retained_state_bytes = argv_state;
                run(selection, request, deadline, &signal.token)
            })() {
                Ok(()) => 0,
                Err(message) => {
                    terminal_diagnostic(message);
                    1
                }
            },
        ),
    }
}

// One best-effort bounded diagnostic; never waits on a caller's stderr pipe.
// Compiler categories and bounded static owner context only. Never format payload,
// SQL errors, paths, or Source(String); stderr delivery is best-effort/nonblocking.
fn compiler_terminal_diagnostic(prefix: &'static str, error: &tos_compiler::Error) {
    use std::fmt::Write;
    struct Diagnostic {
        bytes: [u8; 512],
        len: usize,
    }
    impl std::fmt::Write for Diagnostic {
        fn write_str(&mut self, text: &str) -> std::fmt::Result {
            if text.len() > self.bytes.len() - self.len {
                return Err(std::fmt::Error);
            }
            self.bytes[self.len..self.len + text.len()].copy_from_slice(text.as_bytes());
            self.len += text.len();
            Ok(())
        }
    }
    let mut out = Diagnostic {
        bytes: [0; 512],
        len: 0,
    };
    let _ = write!(out, "{prefix} ");
    let context = match error {
        tos_compiler::Error::Invalid(context) => {
            let _ = out.write_str("invalid: ");
            Some(*context)
        }
        tos_compiler::Error::Budget(context) => {
            let _ = out.write_str("budget: ");
            Some(*context)
        }
        tos_compiler::Error::PreparedUnsupported(context) => {
            let _ = out.write_str("prepared unsupported: ");
            Some(*context)
        }
        tos_compiler::Error::ManagedSourceUnsupported(context) => {
            let _ = out.write_str("managed source unsupported: ");
            Some(*context)
        }
        tos_compiler::Error::Io(error) => {
            let _ = write!(out, "I/O kind: {:?}", error.kind());
            None
        }
        tos_compiler::Error::Sql(_) => {
            let _ = out.write_str("SQLite refused");
            None
        }
        tos_compiler::Error::SqlitePhase { phase, .. } => {
            let _ = write!(out, "SQLite phase: {phase:?}");
            None
        }
        tos_compiler::Error::SqliteVmBudget { phase, .. } => {
            let _ = write!(out, "SQLite VM budget phase: {phase:?}");
            None
        }
        tos_compiler::Error::Source(_) => {
            let _ = out.write_str("source carrier refused");
            None
        }
    };
    if let Some(context) = context {
        let mut end = context.len().min(256);
        while !context.is_char_boundary(end) {
            end -= 1;
        }
        let _ = out.write_str(&context[..end]);
    }
    let _ = out.write_str("\n");
    terminal_diagnostic_bytes(&out.bytes[..out.len]);
}

fn terminal_diagnostic(message: &'static str) {
    terminal_diagnostic_bytes(&message.as_bytes()[..message.len().min(256)]);
}

fn terminal_diagnostic_bytes(raw: &[u8]) {
    let previous = unsafe { libc::fcntl(2, libc::F_GETFL) };
    if previous < 0 || unsafe { libc::fcntl(2, libc::F_SETFL, previous | libc::O_NONBLOCK) } < 0 {
        return;
    }
    unsafe {
        libc::write(2, raw.as_ptr().cast(), raw.len().min(512));
        libc::fcntl(2, libc::F_SETFL, previous);
    }
}

fn original_cli_deadline(cutoff_ns: u64) -> Result<Instant> {
    let instant_before_clock = Instant::now();
    let mut now = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut now) } != 0
        || now.tv_sec < 0
        || now.tv_nsec < 0
    {
        return Err("Core original monotonic clock unavailable");
    }
    let now_ns = u64::try_from(now.tv_sec)
        .ok()
        .and_then(|n| n.checked_mul(1_000_000_000))
        .and_then(|n| n.checked_add(now.tv_nsec as u64))
        .ok_or("Core original clock overflow")?;
    let remaining = cutoff_ns
        .checked_sub(now_ns)
        .filter(|n| *n > 0)
        .ok_or("Core original deadline expired before stdin")?;
    instant_before_clock
        .checked_add(Duration::from_nanos(remaining))
        .ok_or("Core original deadline overflow")
}

enum SelectedRootCall<'a> {
    Tool(&'a str, &'a Value),
    Resource(crate::KnowledgeRequest, bool),
}

impl Request {
    fn retained_resource_state_upper_bound(&self) -> Result<usize> {
        use tos_foundation::{OwnedState, checked_state_add};
        if !self.arguments.is_null() {
            return Err("Core resource transport arguments still retained");
        }
        let mut bytes = std::mem::size_of::<Self>()
            .checked_add(self.caller_retained_state_bytes)
            .ok_or("Core resource caller state overflow")?;
        for amount in [
            self.source_paths.index_path.capacity(),
            self.source_paths
                .philosophy_graph_projection_path
                .capacity(),
            self.source_paths.bibliographic_graph_path.capacity(),
            self.source_paths.entity_type_registry_path.capacity(),
            self.source_paths.relation_type_registry_path.capacity(),
            self.source_paths
                .philosophy_post_planting_audit_path
                .capacity(),
            self.source_paths.evidence_projection_path.capacity(),
            self.query_store
                .path
                .owned_heap_bytes()
                .map_err(|_| "Core request state overflow")?,
        ] {
            bytes = checked_state_add(bytes, amount).map_err(|_| "Core request state overflow")?;
        }
        Ok(bytes)
    }
}

impl SelectedRootCall<'_> {
    fn is_graph_views(&self) -> bool {
        matches!(
            self,
            Self::Resource(
                crate::KnowledgeRequest::Corpus(
                    tos_query::corpus_read::CorpusReadRequest::GraphViews
                ),
                _
            )
        ) || matches!(self, Self::Tool("tos_corpus_graph_views", arguments)
                if arguments.as_object().is_some_and(|fields| fields.is_empty()))
    }
    fn into_graph_views_request(self) -> Self {
        if matches!(&self, Self::Tool("tos_corpus_graph_views", arguments)
            if arguments.as_object().is_some_and(|fields| fields.is_empty()))
        {
            Self::Resource(
                crate::KnowledgeRequest::Corpus(
                    tos_query::corpus_read::CorpusReadRequest::GraphViews,
                ),
                false,
            )
        } else {
            self
        }
    }
}

/// Resource-only formatting over the already genuine packet value; no model or URI owner is recreated.
fn render_resource_packet(
    body: &[u8],
    body_capacity: usize,
    cap: usize,
    deadline: Instant,
    cancelled: &Arc<AtomicBool>,
    view: &tos_compiler::native_snapshot::CompletedCaptureCarriers<'_>,
    remaining_after_retained: &impl Fn(usize) -> tos_compiler::Result<usize>,
) -> tos_compiler::Result<Vec<u8>> {
    let mut check = || -> tos_foundation::Result<()> {
        if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
            return Err(tos_foundation::FoundationError::new(
                tos_foundation::FoundationErrorCode::BudgetExceeded,
                "Resource render original cutoff/cancellation",
            ));
        }
        Ok(())
    };
    let mut admit = |bytes: usize, visits: usize| -> tos_foundation::Result<()> {
        let work = bytes
            .checked_mul(2)
            .and_then(|n| visits.checked_mul(2).and_then(|v| n.checked_add(v)))
            .and_then(|n| u64::try_from(n).ok())
            .ok_or_else(|| {
                tos_foundation::FoundationError::new(
                    tos_foundation::FoundationErrorCode::BudgetExceeded,
                    "Resource render work overflow",
                )
            })?;
        view.charge_work(work).map_err(|_| {
            tos_foundation::FoundationError::new(
                tos_foundation::FoundationErrorCode::BudgetExceeded,
                "Resource render original work",
            )
        })
    };
    let fixed = std::mem::size_of::<tos_foundation::JsonDocument>()
        + std::mem::size_of::<Vec<u8>>()
        + std::mem::size_of::<JsonLimits>()
        + std::mem::size_of_val(&check)
        + std::mem::size_of_val(&admit);
    let held = body_capacity
        .checked_add(fixed)
        .ok_or(tos_compiler::Error::Budget(
            "Resource render fixed state overflow",
        ))?;
    let available = remaining_after_retained(held)?;
    let mut limits = crate::common::packet_json_limits(cap);
    check().map_err(|_| tos_compiler::Error::Budget("Resource render original parse cutoff"))?;
    view.charge_work(
        u64::try_from(body.len())
            .map_err(|_| tos_compiler::Error::Budget("Resource render parse overflow"))?,
    )?;
    let document = tos_foundation::parse_json_with_state_budget_and_check(
        body,
        JsonMode::PublishedStrict,
        limits,
        available,
        &mut check,
    )
    .map_err(|_| tos_compiler::Error::Budget("Resource render original parser state/JSON"))?;
    let tree = document
        .root()
        .retained_storage_bytes()
        .map_err(|_| tos_compiler::Error::Budget("Resource render retained tree"))?;
    let available = remaining_after_retained(held.checked_add(tree).ok_or(
        tos_compiler::Error::Budget("Resource render tree state overflow"),
    )?)?;
    limits.max_visits =
        limits
            .max_visits
            .checked_sub(document.visits())
            .ok_or(tos_compiler::Error::Budget(
                "Resource render original parser/writer visits",
            ))?;
    let (bytes, _) = tos_foundation::emit_python_pretty_sorted_json_with_state_budget(
        document.root(),
        limits,
        available,
        &mut check,
        &mut admit,
    )
    .map_err(|_| tos_compiler::Error::Budget("Resource render original output/state/work"))?;
    check().map_err(|_| tos_compiler::Error::Budget("Resource render original final cutoff"))?;
    drop(document);
    Ok(bytes)
}

/// Deliver one query while its authentic executor, view and disclosure lease stay borrowed.
/// The caller supplies its additive original-state census and pre-admitted query reservation;
/// this helper never creates an independent allowance or retires the packet before send.
fn deliver_selected_root_call<'hold, E: crate::ScopedAccessExecutor<'hold> + ?Sized>(
    executor: &E,
    call: SelectedRootCall<'_>,
    json_limits: JsonLimits,
    profile: crate::AccessProfile,
    deadline: Instant,
    cancelled: &Arc<AtomicBool>,
    view: &tos_compiler::native_snapshot::CompletedCaptureCarriers<'_>,
    remaining_after_retained: impl Fn(usize) -> tos_compiler::Result<usize>,
    reserve_resource_state: impl FnOnce(usize) -> tos_compiler::Result<()>,
    send: impl FnOnce(&[u8]) -> tos_compiler::Result<()>,
) -> tos_compiler::Result<()> {
    let stateful_graph_views = call.is_graph_views();
    let (tool, resource_render, resource_request, argument_storage) = match call
        .into_graph_views_request()
    {
        SelectedRootCall::Resource(resource, render) => {
            ("tos_native_resource_read", render, Some(resource), None)
        }
        SelectedRootCall::Tool(tool, arguments) => {
            let mut raw = BoundedOutput::new(INPUT_CAP, deadline);
            raw.value(arguments).map_err(tos_compiler::Error::Invalid)?;
            let arguments_document = parse_json(&raw.bytes, JsonMode::PublishedStrict, json_limits)
                .map_err(|_| tos_compiler::Error::Invalid("Core query argument JSON refused"))?;
            (tool, false, None, Some((raw, arguments_document)))
        }
    };
    let empty = tos_foundation::JsonValue::Null;
    let arguments = argument_storage
        .as_ref()
        .map_or(&empty, |(_, doc)| doc.root());
    let registered = if resource_request.is_none() {
        Some(
            crate::common::registered_operations()
                .map_err(|_| tos_compiler::Error::Invalid("Core native registry unavailable"))?,
        )
    } else {
        None
    };
    let operation = registered
        .as_ref()
        .and_then(|items| items.iter().find(|op| op.mcp_tool == tool));
    if resource_request.is_none() {
        let operation =
            operation.ok_or(tos_compiler::Error::Invalid("Core native tool unavailable"))?;
        let allowed = operation
            .input_schema
            .object_get("properties")
            .and_then(tos_foundation::JsonValue::as_object);
        if arguments.as_object().is_none_or(|fields| {
            fields.iter().any(|(name, _)| {
                !allowed.is_some_and(|properties| properties.iter().any(|(key, _)| key == name))
            })
        }) {
            return Err(tos_compiler::Error::Invalid(
                "Core native tool argument unavailable",
            ));
        }
    }
    if stateful_graph_views {
        let remaining = remaining_after_retained(0)?;
        reserve_resource_state(remaining)?;
    }
    let probe: Arc<dyn tos_query::AbortProbe> = Arc::new(CoreQueryProbe {
        deadline,
        cancelled: cancelled.clone(),
    });
    let mut packet = crate::common::checked_execute(probe, |probe| {
        if let Some(request) = resource_request {
            return executor.knowledge(request, probe);
        }
        if tool == crate::common::SEARCH_MCP_TOOL {
            return crate::search::SearchRequest::from_arguments(arguments)
                .and_then(|request| request.execute(executor, probe));
        }
        if tool == crate::common::MCP_TOOL {
            return crate::Params::from_json(arguments)
                .and_then(|request| executor.source_descend(request, probe));
        }
        let operation = operation.ok_or_else(|| {
            crate::AccessError::new(
                crate::AccessErrorCode::Unavailable,
                "selected Root native tool unavailable",
            )
        })?;
        let op = crate::KnowledgeOperation::from_id(&operation.operation_id).ok_or_else(|| {
            crate::AccessError::new(
                crate::AccessErrorCode::Unavailable,
                "selected Root native tool unavailable",
            )
        })?;
        if op == crate::KnowledgeOperation::AccessHealth {
            return executor.access_health(probe);
        }
        if op == crate::KnowledgeOperation::PreparedStatus {
            return executor.prepared_status(probe);
        }
        crate::KnowledgeRequest::from_arguments(op, arguments).and_then(|request| {
            if matches!(request, crate::KnowledgeRequest::ExplorationContracts) {
                crate::exploration_contracts::execute(executor, profile.max_response_bytes)
            } else {
                executor.knowledge(request, probe)
            }
        })
    })
    .map_err(|_| tos_compiler::Error::Invalid("Core selected native query refused"))?;
    if packet.body.len() > profile.max_response_bytes {
        return Err(tos_compiler::Error::Budget("Core query response bytes"));
    }
    let rendered = if resource_render {
        Some(render_resource_packet(
            &packet.body,
            packet.body.capacity(),
            profile.max_response_bytes,
            deadline,
            cancelled,
            view,
            &remaining_after_retained,
        )?)
    } else {
        None
    };
    let render_capacity = rendered.as_ref().map_or(0, Vec::capacity);
    let render_fixed = if resource_render {
        std::mem::size_of::<Option<Vec<u8>>>()
    } else {
        0
    };
    let simultaneous_packet = packet
        .body
        .capacity()
        .checked_add(render_capacity)
        .and_then(|n| n.checked_add(render_fixed))
        .ok_or(tos_compiler::Error::Budget(
            "Resource render held body/output state overflow",
        ))?;
    let resource_remaining_state = if tool == "tos_native_resource_read" {
        Some(remaining_after_retained(simultaneous_packet)?)
    } else {
        None
    };
    if resource_render {
        // The bounded renderer already validates the same packet using the existing strict owner parser.
    } else if let Some(remaining) = resource_remaining_state {
        view.charge_work(
            u64::try_from(packet.body.len())
                .map_err(|_| tos_compiler::Error::Budget("Core packet validation work overflow"))?,
        )?;
        crate::common::validate_packet_with_state_budget(
            &packet.body,
            profile.max_response_bytes,
            remaining,
        )
        .map_err(|_| tos_compiler::Error::Budget("Core resource packet validation state/JSON"))?;
    } else {
        crate::common::validate_packet(&packet.body, profile.max_response_bytes)
            .map_err(|_| tos_compiler::Error::Invalid("Core query packet invalid"))?;
    }
    packet
        .fence
        .recheck()
        .map_err(|_| tos_compiler::Error::Invalid("Core query disclosure fence"))?;
    view.verify_current()?;
    let prefix = br#"{"schema_version":"tos_native_core_snapshot_result_v1","ok":true,"result":"#;
    let resource_text = if resource_render {
        Some(
            std::str::from_utf8(rendered.as_deref().ok_or(tos_compiler::Error::Invalid(
                "Resource render output absent",
            ))?)
            .map_err(|_| tos_compiler::Error::Invalid("Core resource text encoding"))?,
        )
    } else {
        None
    };
    let mut envelope_reservation = None;
    let cap = if tool == "tos_native_resource_read" {
        // Original packet and actual pretty-output capacity coexist with this escaped envelope;
        // the decoded tree has already been dropped and all allocations use the original remainder.
        drop(argument_storage);
        drop(registered);
        let work = u64::try_from(resource_text.map_or(packet.body.len(), str::len))
            .ok()
            .and_then(|n| n.checked_mul(2))
            .ok_or(tos_compiler::Error::Budget(
                "Core resource encoding work overflow",
            ))?;
        view.charge_work(work)?;
        let payload_bytes = if let Some(text) = resource_text {
            crate::common::json_string_len(text).ok_or(tos_compiler::Error::Budget(
                "Core resource encoding size overflow",
            ))?
        } else {
            packet.body.len()
        };
        let envelope_bytes = prefix
            .len()
            .checked_add(payload_bytes)
            .and_then(|n| n.checked_add(2))
            .ok_or(tos_compiler::Error::Budget(
                "Core resource envelope size overflow",
            ))?;
        let remaining_state = resource_remaining_state.ok_or(tos_compiler::Error::Invalid(
            "Core resource original state missing",
        ))?;
        let cap = profile
            .max_mcp_frame_bytes
            .min(remaining_state)
            .min(OUTPUT_CAP);
        if envelope_bytes > cap {
            return Err(tos_compiler::Error::Budget(
                "Core resource envelope state/frame budget",
            ));
        }
        envelope_reservation = Some(envelope_bytes);
        cap
    } else {
        OUTPUT_CAP
    };
    let mut out = if let Some(bytes) = envelope_reservation {
        BoundedOutput::reserved(cap, deadline, bytes).map_err(tos_compiler::Error::Invalid)?
    } else {
        BoundedOutput::new(cap, deadline)
    };
    out.literal(prefix).map_err(tos_compiler::Error::Invalid)?;
    if let Some(text) = resource_text {
        out.value(&text).map_err(tos_compiler::Error::Invalid)?;
    } else {
        out.literal(&packet.body)
            .map_err(tos_compiler::Error::Invalid)?;
    }
    out.literal(b"}\n").map_err(tos_compiler::Error::Invalid)?;
    packet
        .fence
        .recheck()
        .map_err(|_| tos_compiler::Error::Invalid("Core query encoded disclosure fence"))?;
    view.verify_current()?;
    send(&out.bytes)?;
    packet
        .fence
        .recheck()
        .map_err(|_| tos_compiler::Error::Invalid("Core query final disclosure fence"))?;
    view.verify_current()
}

fn serve_selected_root(
    root: &Path,
    request: &Request,
    listen: &str,
    max_connections: u64,
    deadline: Instant,
    cancelled: &Arc<AtomicBool>,
    query_call: Option<SelectedRootCall<'_>>,
    session: Option<&session_owner::Session>,
) -> Result<()> {
    let http = request
        .http
        .as_ref()
        .ok_or("Core HTTP requires explicit query and transport allowances")?;
    let profile = http.profile()?;
    let selected = http.selected.native()?;
    let legacy = http.legacy.native()?;
    let indexed = http.indexed.native()?;
    let contracts = http.contracts.native()?;
    let mut checkpoints = http.checkpoints()?;
    let checkpoint_state = checkpoints.clone();
    let limits = tos_compiler::native_snapshot_manifest::portable_native_snapshot_limits(
        request.admission.max_build_seconds,
    )
    .map_err(|_| "Core HTTP source limits refused")?;
    bind_ticket(&request.admission)?;
    let isolation =
        tos_compiler::private_tmpfs_stage::PrivateTmpfsStageIsolation::select_from_environment(
            request.admission.tmpfs_quota_bytes,
            request.admission.inode_limit,
            request.admission.working_ram_bytes,
        )
        .map_err(|_| "Core HTTP private stage refused")?;
    let capture = tos_compiler::PublicCapture::create_runtime_selected(
        root,
        &source_paths(&request.source_paths),
        &fresh(isolation.root(), "tos-core-http-capture")?,
        limits.capture,
        deadline,
        cancelled.clone(),
    )
    .map_err(|_| "Core HTTP exact source capture refused")?;
    let completed = tos_compiler::native_snapshot::build_native_snapshot_from_capture(
        &capture,
        &fresh(isolation.root(), "tos-core-http-model")?,
        tos_compiler::native_snapshot_manifest::RUNTIME_DATA_DECLARATION,
        &isolation,
        limits,
    )
    .map_err(|error| {
        compiler_terminal_diagnostic("Core HTTP native producer", &error);
        "Core HTTP full native producer refused"
    })?
    .into_reference_query_delivery();
    let evidence = tos_compiler::native_snapshot::check_completed_evidence_projection(
        &capture,
        &fresh(isolation.root(), "tos-core-http-evidence")?,
        limits.capture,
        deadline,
    )
    .map_err(|error| {
        compiler_terminal_diagnostic("Core HTTP evidence owner", &error);
        "Core HTTP evidence owner refused"
    })?;
    // This is the original request allowance, not a new per-owner grant.
    // Before conversion the builder records and the prospective contiguous
    // membership slots coexist; keys/digests move without payload clones.
    let resource_call =
        session.is_some() || matches!(query_call, Some(SelectedRootCall::Resource(_, _)));
    let evidence = if resource_call {
        let mut held = request.retained_resource_state_upper_bound()?;
        for amount in [
            capture
                .retained_state_upper_bound()
                .map_err(|_| "Core capture retained state refused")?,
            completed
                .retained_query_state_upper_bound()
                .map_err(|_| "Core completed retained state refused")?,
            evidence
                .query_transition_state_upper_bound()
                .map_err(|_| "Core Evidence transition state refused")?,
        ] {
            held = held
                .checked_add(amount)
                .ok_or("Core transition retained state overflow")?;
        }
        let remaining = request
            .admission
            .whole_max_state_bytes
            .checked_sub(held)
            .ok_or("Core Evidence original retained state allowance")?;
        let workspace = evidence
            .query_delivery_workspace_bytes()
            .map_err(|_| "Core Evidence workspace refused")?;
        if workspace > remaining {
            return Err("Core Evidence original workspace allowance");
        }
        evidence
            .into_reference_query_delivery(workspace)
            .map_err(|_| "Core Evidence query transition refused")?
    } else {
        evidence
    };
    let resources = crate::native_cold_resources::LinuxCgroupColdOpenResourceHold::acquire(
        request.admission.working_ram_bytes,
        deadline,
        cancelled.clone(),
    )
    .map_err(|_| "Core HTTP actual kernel cold resources unavailable")?;
    let corpus = tos_query::corpus_read::CorpusReadContext {
        tos_root: root.to_str().ok_or("Core HTTP root requires UTF-8")?.into(),
        index_path: request
            .source_paths
            .index_path
            .to_str()
            .ok_or("Core HTTP index path requires UTF-8")?
            .into(),
    };
    crate::reference_root_query::with_selected_metadata_context(
        &capture,
        &completed,
        &isolation,
        request.admission.cold,
        request.admission.process,
        request.admission.working_ram_bytes,
        &resources,
        deadline,
        cancelled,
        |model, bound, context, view| {
            completed.with_evidence_projection(&capture, &evidence, |evidence_view| {
                // The actual selected index was captured through the same held
                // source paths. The context describes that selected member,
                // never a spool path or a normalized-row reconstruction.
                view.verify_current()?;
                let stateful_graph_views = session.is_some() || query_call.as_ref()
                    .is_some_and(SelectedRootCall::is_graph_views);
                if stateful_graph_views {
                    context.reserve_resource_operation_scopes()
                        .map_err(|_| tos_compiler::Error::Budget("Core original operation scope reservation"))?;
                }
                let executor = crate::reference_query_executor::ReferenceQueryExecutor::new(
                    model,
                    bound,
                    context,
                    &mut checkpoints,
                    selected,
                    legacy,
                    indexed,
                    view,
                    contracts,
                    Some(evidence_view),
                    Some(&corpus),
                )
                .map_err(|_| tos_compiler::Error::Invalid("Core HTTP selected executor refused"))?;
                {
                    let remaining_after_retained = |packet_capacity: usize| -> tos_compiler::Result<usize> {
                    // Add every distinct still-live owner before reserving
                    // the escaped result. Aliased capture/model/vocabulary
                    // references do not introduce another allocation charge.
                    let mut held = request.retained_resource_state_upper_bound()
                        .map_err(tos_compiler::Error::Invalid)?;
                    for amount in [capture.retained_state_upper_bound()?,
                        completed.retained_query_state_upper_bound()?,
                        evidence.retained_query_state_upper_bound()?,
                        executor.retained_state_upper_bound()
                            .map_err(|_| tos_compiler::Error::Budget("Core executor retained state"))?,
                        std::mem::size_of_val(&corpus), corpus.tos_root.capacity(),
                        corpus.index_path.capacity(),
                        std::mem::size_of::<crate::exploration_checkpoints::ProcessExplorationCheckpoints>(),
                        checkpoint_state.retained_empty_state_upper_bound()
                            .map_err(|_| tos_compiler::Error::Invalid("Core resource checkpoint state changed"))?,
                        resources.retained_state_upper_bound()?,
                        crate::common::scoped_packet_wrapper_state_bytes(),
                        crate::reference_root_query::reference_lease_state_bytes(),
                        crate::knowledge::combined_probe_state_bytes(),
                        std::mem::size_of::<CoreQueryProbe>(),
                        std::mem::size_of::<AtomicBool>(),
                        tos_compiler::private_tmpfs_stage::PRIVATE_TMPFS_SELECT_COST.retained_bytes,
                        tos_compiler::private_tmpfs_stage::PRIVATE_TMPFS_VERIFY_COST.workspace_bytes,
                        std::mem::size_of::<BoundedOutput>(),
                        std::mem::size_of::<tos_compiler::native_snapshot::CompletedCaptureCarriers<'_>>(),
                        std::mem::size_of::<tos_compiler::native_snapshot::CompletedEvidenceProjectionView<'_>>(),
                        packet_capacity, std::mem::size_of::<crate::KnowledgeRequest>()] {
                        held = held.checked_add(amount)
                            .ok_or(tos_compiler::Error::Budget("Core whole retained state overflow"))?;
                    }
                    let remaining_state = request.admission.whole_max_state_bytes
                        .checked_sub(held)
                        .ok_or(tos_compiler::Error::Budget("Core resource whole retained state"))?;
                        Ok(remaining_state)
                    };
                    if let Some(session) = session {
                        return session_owner::run_held(session, &executor, request, profile, deadline,
                            cancelled, view, bound.require_source_revision().map_err(|_| {
                                tos_compiler::Error::Invalid("Core session held source revision absent")
                            })?, remaining_after_retained,
                            |remaining| { executor.reserve_resource_query_state(remaining); Ok(()) });
                    }
                    if let Some(call) = query_call {
                    return deliver_selected_root_call(
                        &executor, call,
                        request.admission.json.limits().map_err(tos_compiler::Error::Invalid)?,
                        profile, deadline, cancelled, view,
                        remaining_after_retained,
                        |remaining| { executor.reserve_resource_query_state(remaining); Ok(()) },
                        |bytes| disclose_bytes(bytes, deadline).map_err(tos_compiler::Error::Invalid),
                    );
                    }
                }
                // This is association evidence from the admitted held model,
                // not a Worker publication marker or a new owner grant.
                evidence_view.charge_work(
                    u64::try_from(evidence_view.raw().len()).map_err(|_| {
                        tos_compiler::Error::Budget("Core HTTP startup evidence byte overflow")
                    })?,
                )?;
                let inputs = view.retained_inputs()?;
                let binding = &completed.stage().binding;
                let receipt = serde_json::json!({
                    "schema": "tos_native_core_http_startup_v1",
                    "source_revision": bound.require_source_revision().map_err(|_| {
                        tos_compiler::Error::Invalid("Core HTTP startup source revision absent")
                    })?,
                    "owner_profile": binding.owner_profile,
                    "source_cut": binding.source_cut,
                    "through_commit_seq": binding.through_commit_seq,
                    "model_abi": bound.selection().model_abi,
                    "graph_root_sha256": bound.selection().graph_root_sha256.to_hex(),
                    "catalog_packet_sha256": bound.selection().catalog_packet_sha256.to_hex(),
                    "declaration_sha256": completed.declaration_sha256().to_hex(),
                    "evidence_source_revision": evidence_view.source_revision(),
                    "evidence_sha256": tos_foundation::Digest256::of_bytes(evidence_view.raw()).to_hex(),
                    "captured_inputs": inputs.into_iter().map(|(path, sha256, size_bytes)| {
                        serde_json::json!({"path":path,"sha256":sha256,"size_bytes":size_bytes})
                    }).collect::<Vec<_>>()
                });
                let mut startup = BoundedOutput::new(http.max_startup_receipt_bytes, deadline);
                startup.value(&receipt).map_err(tos_compiler::Error::Invalid)?;
                startup.literal(b"\n").map_err(tos_compiler::Error::Invalid)?;
                view.verify_current()?;
                disclose_bytes(&startup.bytes, deadline).map_err(tos_compiler::Error::Invalid)?;
                view.verify_current()?;
                let finished = AtomicBool::new(false);
                let mut accepted = 0_u64;
                crate::http::serve_selected_connections(
                    listen,
                    profile,
                    deadline,
                    cancelled,
                    &finished,
                    |stream, site, profile, control| {
                        let result = crate::http::serve_connection_scoped_controlled(
                            stream, &executor, site, profile, control,
                        );
                        accepted = accepted.checked_add(1).ok_or_else(|| {
                            std::io::Error::other("Core HTTP connection count overflow")
                        })?;
                        if accepted >= max_connections {
                            finished.store(true, Ordering::Release);
                        }
                        result
                    },
                )
                .map_err(tos_compiler::Error::Io)
            })
        },
    )
    .map_err(|error| {
        compiler_terminal_diagnostic("Core HTTP held callback or delivery", &error);
        "Core HTTP held callback or delivery refused"
    })
}

struct CoreQueryProbe {
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
}
impl tos_query::AbortProbe for CoreQueryProbe {
    fn reason(&self) -> Option<tos_query::AbortReason> {
        if self.cancelled.load(Ordering::Acquire) {
            Some(tos_query::AbortReason::Cancelled)
        } else if Instant::now() >= self.deadline {
            Some(tos_query::AbortReason::DeadlineExceeded)
        } else {
            None
        }
    }
}
