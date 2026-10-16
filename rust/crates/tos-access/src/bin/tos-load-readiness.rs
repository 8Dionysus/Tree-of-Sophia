//! Bounded protocol workload for an already installed Tree-of-Sophia Rust release.
//! The schedule supplies owner-selected requests; this tool adds no server API.
use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::ops::{Deref, DerefMut};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

type HmacSha256 = Hmac<Sha256>;
const SCHEDULE_SCHEMA: &str = "tos_protocol_load_schedule_v1";
const MCP_PROTOCOL: &str = "2025-11-25";
const MAX_SESSIONS: usize = 256;
const SUPPORTED_SESSION_STAGES: [usize; 6] = [2, 8, 16, 64, 128, 256];
const MAX_OPS: usize = 4096;
const MAX_NONCES: usize = 1024;
const MAX_OPERATIONS_PER_SECOND: f64 = 10_000.0;
const MAX_RUN_SECONDS: u64 = 3600;
const MAX_POSTLOAD_EXACT_SOURCE_BYTES: u64 = 1_048_576;
const MAX_POSTLOAD_SOURCE_READ_WORK_VALUE: u64 = 16_777_216;
const RECORD_REVISE_SEMANTIC_ORACLE_SCHEMA: &str =
    "tos_load_native_pg_record_revise_semantic_oracle_v1";
const RECORD_REVISE_RESULT_SCHEMA: &str = "tos_local_source_revision_result_v2";
const DEFAULT_OWNER_STAGE_CLONE_CAP_BYTES: u64 = 268_435_456;
const MAX_OWNER_STAGE_CLONE_CAP_BYTES: u64 = 1_073_741_824;
const NATIVE_STDERR_CAP: usize = 4096;
const USAGE: &str = concat!(
    "usage: tos-load-readiness --query-binary ABS [--model ABS --binding ABS] --schedule ABS --output DIR --unit UNIT ",
    "[--pg-stage-mode fresh|existing] [--source-text-search-selection ABS] ",
    "[--source-root ABS --source-inputs ABS] [--owner-binary ABS --owner-config ABS --invocation ABS --token-file ABS] ",
    "[--owner-stage-baseline ABS] [--owner-stage-clone-cap-bytes N] [--mcp-port PORT] [--owner-port PORT] [--sessions 2|8|16|64|128|256] ",
    "[--sdk-in-flight 1..4] [--operation-count N] [--channel-mix mcp=60,owner_http=20,sdk=20,projection=0] ",
    "[--operations-per-second N] [--load-window-seconds N] [--deadline-seconds N] [--request-cap-bytes N] ",
    "[--response-cap-bytes N] [--schedule-cap-bytes N] [--output-cap-bytes N]"
);
const NAVIGATION_OWNER_SCHEMA: &str = "tos_local_source_navigation_owner_v1";
const NAVIGATION_SELECTION_SCHEMA: &str = "tos_local_pg_source_navigation_selection_v1";
const NAVIGATION_PRODUCER_SCHEMA: &str = "tos_local_pg_source_navigation_producer_v1";
static STOP: AtomicBool = AtomicBool::new(false);

#[derive(Clone)]
struct Args {
    query_binary: PathBuf,
    owner_binary: Option<PathBuf>,
    source_root: Option<PathBuf>,
    source_inputs: Option<PathBuf>,
    model: Option<PathBuf>,
    binding: Option<PathBuf>,
    pg_stage_mode: PgStageMode,
    source_text_search_selection: Option<PathBuf>,
    owner_config: Option<PathBuf>,
    invocation: Option<PathBuf>,
    token_file: Option<PathBuf>,
    owner_stage_baseline: Option<PathBuf>,
    owner_stage_clone_cap_bytes: u64,
    schedule: PathBuf,
    output: PathBuf,
    unit: String,
    mcp_port: u16,
    owner_port: u16,
    sessions: usize,
    sdk_in_flight: usize,
    operation_count: Option<usize>,
    channel_mix: Option<BTreeMap<String, u8>>,
    operations_per_second: Option<f64>,
    load_window_seconds: Option<u64>,
    deadline_seconds: u64,
    request_cap: usize,
    response_cap: usize,
    schedule_cap: usize,
    output_cap: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PgStageMode {
    Fresh,
    Existing,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Plan {
    schema: String,
    #[serde(default)]
    setup: Vec<Operation>,
    sessions: Vec<SessionPlan>,
    /// Serial owner/source assertions after measured workers and projection drain.
    #[serde(default)]
    postload: Vec<Operation>,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionPlan {
    id: String,
    operations: Vec<Operation>,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Operation {
    channel: String,
    label: String,
    request: Value,
    expected_status: u16,
    #[serde(default)]
    expected_sha256: Option<String>,
    #[serde(default)]
    expected_body: Option<Value>,
    #[serde(default)]
    expected_stderr_prefix: Option<String>,
    /// Structural rights-state expectation for native PostgreSQL postload describe.
    #[serde(default)]
    expected_rights_allowed: Option<bool>,
    #[serde(default)]
    ignored_json_paths: Vec<String>,
    #[serde(default)]
    alternate_outcomes: Vec<ExpectedOutcome>,
    #[serde(default)]
    retry_same_command_id: bool,
    #[serde(default)]
    reconnect_before: bool,
    #[serde(default)]
    conflict_group: Option<String>,
    #[serde(default)]
    bind_expected_configuration_from_owner: bool,
    #[serde(default)]
    sdk_argv: Vec<String>,
    #[serde(default)]
    projection_argv: Vec<String>,
    #[serde(default)]
    interrupt_resume_after_ms: Option<u64>,
    #[serde(default)]
    bind_expected_from: Option<String>,
    /// The load-phase command ID whose successful durable result this one
    /// postload SDK request replays. A replay must match that request exactly.
    #[serde(default)]
    idempotency_replay_of: Option<String>,
    /// Runtime-only snapshot from the exact serial prepare-revise response
    /// used to bind this mutation's preconditions.
    #[serde(skip)]
    bound_revision_preview: Option<Value>,
    /// Runtime-only successful source mutation response used for exact replay
    /// comparison after normalizing the two replay flags.
    #[serde(skip)]
    bound_replay_response: Option<Value>,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ExpectedOutcome {
    status: u16,
    #[serde(default)]
    sha256: Option<String>,
    #[serde(default)]
    body: Option<Value>,
    #[serde(default)]
    ignored_json_paths: Vec<String>,
}

#[derive(Clone)]
struct WaveGate {
    active: Arc<Mutex<usize>>,
    limit: usize,
}

struct WavePermit {
    active: Arc<Mutex<usize>>,
}

struct SessionWorkerWave {
    handles: Vec<thread::JoinHandle<()>>,
    start_senders: Vec<mpsc::Sender<()>>,
    owned_process_groups: Arc<Mutex<Vec<i32>>>,
    started: bool,
    complete: bool,
}

impl SessionWorkerWave {
    fn new(owned_process_groups: Arc<Mutex<Vec<i32>>>) -> Self {
        Self {
            handles: Vec::new(),
            start_senders: Vec::new(),
            owned_process_groups,
            started: false,
            complete: false,
        }
    }

    fn push(&mut self, handle: thread::JoinHandle<()>, start: mpsc::Sender<()>) {
        self.handles.push(handle);
        self.start_senders.push(start);
    }

    fn start(&mut self) -> Result<(), String> {
        self.started = true;
        let mut failed = false;
        for sender in self.start_senders.drain(..) {
            failed |= sender.send(()).is_err();
        }
        if failed {
            Err("session worker start gate disconnected".into())
        } else {
            Ok(())
        }
    }

    fn join_all(&mut self) -> Result<(), String> {
        let mut panicked = false;
        for handle in self.handles.drain(..) {
            if handle.join().is_err() {
                panicked = true;
                if self.started {
                    signal_all(&self.owned_process_groups, libc::SIGKILL);
                }
            }
        }
        self.complete = true;
        if panicked {
            Err("session worker panicked; owned process groups were terminated".into())
        } else {
            Ok(())
        }
    }
}

impl Drop for SessionWorkerWave {
    fn drop(&mut self) {
        self.start_senders.clear();
        if !self.complete {
            if self.started {
                // The registry contains only process groups created by this
                // runner. A failed wave owns them all and must stop them before
                // joining workers that could otherwise remain blocked on I/O.
                signal_all(&self.owned_process_groups, libc::SIGKILL);
            }
            for handle in self.handles.drain(..) {
                let _ = handle.join();
            }
            self.complete = true;
        }
    }
}

impl WaveGate {
    fn new(limit: usize) -> Self {
        Self {
            active: Arc::new(Mutex::new(0)),
            limit,
        }
    }

    fn acquire(
        &self,
        deadline: Instant,
        dispatch_deadline: Option<Instant>,
    ) -> Result<Option<WavePermit>, String> {
        loop {
            if STOP.load(Ordering::Acquire) {
                return Err("bounded operation wave deadline/cancellation".into());
            }
            let now = Instant::now();
            if now >= deadline {
                return Err("bounded operation wave deadline/cancellation".into());
            }
            if dispatch_deadline.is_some_and(|dispatch| dispatch <= deadline && now >= dispatch) {
                return Ok(None);
            }
            let mut active = self
                .active
                .lock()
                .map_err(|_| "bounded operation wave counter poisoned")?;
            if *active < self.limit {
                *active += 1;
                return Ok(Some(WavePermit {
                    active: Arc::clone(&self.active),
                }));
            }
            drop(active);
            let wait = Duration::from_millis(2)
                .min(deadline.saturating_duration_since(now))
                .min(
                    dispatch_deadline
                        .map(|dispatch| dispatch.saturating_duration_since(now))
                        .unwrap_or(Duration::from_millis(2)),
                );
            thread::sleep(wait);
        }
    }
}

impl Drop for WavePermit {
    fn drop(&mut self) {
        if let Ok(mut active) = self.active.lock() {
            *active = active.saturating_sub(1);
        }
    }
}

#[derive(Clone)]
struct WorkloadGates {
    mcp_sessions: WaveGate,
    owner_http: WaveGate,
    source_reads: WaveGate,
    sdk_source_commands: WaveGate,
    operation_rate: OperationRateGate,
}

#[derive(Clone)]
struct OperationRateGate {
    interval: Option<Duration>,
    next_dispatch: Arc<Mutex<Option<Instant>>>,
}

impl OperationRateGate {
    fn new(operations_per_second: Option<f64>) -> Self {
        Self {
            interval: operations_per_second.map(|rate| {
                // Slower rates cannot dispatch a second operation within the
                // bounded one-hour run, so cap the wait without changing any
                // dispatch possible before the whole-run deadline.
                Duration::from_secs_f64(
                    (1.0 / rate)
                        .min(MAX_RUN_SECONDS as f64)
                        .max(Duration::from_nanos(1).as_secs_f64()),
                )
            }),
            next_dispatch: Arc::new(Mutex::new(None)),
        }
    }

    /// Returns false when the optional measured load window closes while this
    /// operation waits for its dispatch slot. The global task deadline remains
    /// an error because it also bounds cleanup and report finalization.
    fn acquire(
        &self,
        deadline: Instant,
        load_window_deadline: Option<Instant>,
    ) -> Result<bool, String> {
        loop {
            if STOP.load(Ordering::Acquire) {
                return Err("bounded workload cancelled".into());
            }
            let now = Instant::now();
            if now >= deadline {
                return Err("whole workload deadline".into());
            }
            if load_window_deadline.is_some_and(|window| window <= deadline && now >= window) {
                return Ok(false);
            }
            let Some(interval) = self.interval else {
                return Ok(true);
            };
            let mut next_dispatch = self
                .next_dispatch
                .lock()
                .map_err(|_| "operation-rate gate poisoned")?;
            let due = next_dispatch.unwrap_or(now);
            if now >= due {
                *next_dispatch = Some(now + interval);
                return Ok(true);
            }
            let remaining = due.saturating_duration_since(now);
            drop(next_dispatch);
            let until_deadline = deadline.saturating_duration_since(now);
            let until_window = load_window_deadline
                .map(|window| window.saturating_duration_since(now))
                .unwrap_or(until_deadline);
            thread::sleep(
                remaining
                    .min(until_deadline)
                    .min(until_window)
                    .min(Duration::from_millis(10)),
            );
        }
    }
}

#[derive(Clone)]
struct InputStamp {
    path: PathBuf,
    dev: u64,
    ino: u64,
    len: u64,
    mtime: i64,
    mtime_nsec: i64,
    ctime: i64,
    ctime_nsec: i64,
    mode: u32,
}

struct ManagedChild {
    child: Child,
    registry: Arc<Mutex<Vec<i32>>>,
    reaped: bool,
}
impl Deref for ManagedChild {
    type Target = Child;
    fn deref(&self) -> &Self::Target {
        &self.child
    }
}
impl DerefMut for ManagedChild {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.child
    }
}
impl ManagedChild {
    fn unregister(&mut self) {
        let pid = self.child.id() as i32;
        if let Ok(mut pids) = self.registry.lock() {
            pids.retain(|entry| *entry != pid);
        }
    }
    fn try_wait(&mut self) -> io::Result<Option<std::process::ExitStatus>> {
        let status = self.child.try_wait()?;
        if status.is_some() {
            self.reaped = true;
            self.unregister();
        }
        Ok(status)
    }
    fn wait(&mut self) -> io::Result<std::process::ExitStatus> {
        let status = self.child.wait()?;
        self.reaped = true;
        self.unregister();
        Ok(status)
    }
}
impl Drop for ManagedChild {
    fn drop(&mut self) {
        stop_child(self);
    }
}

#[derive(Default, Clone)]
struct McpSession {
    id: Option<String>,
}

#[derive(Clone)]
struct HttpReply {
    status: u16,
    headers: BTreeMap<String, String>,
    body: Vec<u8>,
}

#[derive(Clone)]
struct Attempt {
    status: u16,
    bytes: usize,
    sha256: String,
    body: Option<Vec<u8>>,
    stderr: Vec<u8>,
    elapsed_ms: f64,
    authenticated: Option<bool>,
    interrupted: bool,
}

#[derive(Clone)]
struct OpResult {
    value: Value,
    passed: bool,
    successful: bool,
    not_started: bool,
    conflict: bool,
    retry_count: usize,
    reconnect_count: usize,
    projection_interruptions: usize,
    projection_resumes: usize,
    projection_status_checks: usize,
    response_body: Option<Value>,
    elapsed_ms: f64,
}

#[derive(Clone, Default)]
struct ResourceSnapshot {
    cpu_usec: u64,
    memory_current: u64,
    memory_peak: u64,
    memory_limit: Option<u64>,
    memory_high_events: u64,
    memory_max_events: u64,
    memory_oom_events: u64,
    memory_oom_kill_events: u64,
    io_read_bytes: u64,
    io_write_bytes: u64,
}

fn operation_error(reason: &str) -> OpResult {
    OpResult {
        value: json!({"event":"operation","passed":false,"transport_error":true,"error":reason}),
        passed: false,
        successful: false,
        not_started: false,
        conflict: false,
        retry_count: 0,
        reconnect_count: 0,
        projection_interruptions: 0,
        projection_resumes: 0,
        projection_status_checks: 0,
        response_body: None,
        elapsed_ms: 0.0,
    }
}

fn operation_skipped(reason: &'static str) -> OpResult {
    OpResult {
        value: json!({"event":"operation","passed":false,"scheduled":true,"not_started":true,"transport_error":false,"skip_reason":reason}),
        passed: false,
        successful: false,
        not_started: true,
        conflict: false,
        retry_count: 0,
        reconnect_count: 0,
        projection_interruptions: 0,
        projection_resumes: 0,
        projection_status_checks: 0,
        response_body: None,
        elapsed_ms: 0.0,
    }
}

extern "C" fn signal_handler(_: i32) {
    STOP.store(true, Ordering::Release);
}

fn install_signals() {
    unsafe {
        libc::signal(libc::SIGTERM, signal_handler as libc::sighandler_t);
        libc::signal(libc::SIGINT, signal_handler as libc::sighandler_t);
    }
}

fn parse_args() -> Result<Option<Args>, String> {
    let mut values = BTreeMap::new();
    let mut args = std::env::args().skip(1);
    while let Some(key) = args.next() {
        if key == "--help" || key == "-h" {
            return Ok(None);
        }
        if !key.starts_with("--") || values.contains_key(&key) {
            return Err("invalid or duplicate option".into());
        }
        let value = args
            .next()
            .ok_or_else(|| "missing option value".to_owned())?;
        values.insert(key, value);
    }
    let take = |values: &mut BTreeMap<String, String>,
                key: &str,
                default: Option<&str>|
     -> Result<String, String> {
        values
            .remove(key)
            .or_else(|| default.map(str::to_owned))
            .ok_or_else(|| format!("missing {key}"))
    };
    let path = |values: &mut BTreeMap<String, String>, key: &str| -> Result<PathBuf, String> {
        let value = take(values, key, None)?;
        let p = PathBuf::from(value);
        if !p.is_absolute() {
            return Err(format!("{key} must be absolute"));
        }
        Ok(p)
    };
    let optional_path =
        |values: &mut BTreeMap<String, String>, key: &str| -> Result<Option<PathBuf>, String> {
            values
                .remove(key)
                .map(|value| {
                    let p = PathBuf::from(value);
                    if !p.is_absolute() {
                        return Err(format!("{key} must be absolute"));
                    }
                    Ok(p)
                })
                .transpose()
        };
    let number =
        |values: &mut BTreeMap<String, String>, key: &str, default: &str| -> Result<u64, String> {
            let value = take(values, key, Some(default))?;
            let parsed = value.parse::<u64>().map_err(|_| format!("invalid {key}"))?;
            if parsed == 0 {
                return Err(format!("{key} must be positive"));
            }
            Ok(parsed)
        };
    let query_binary = path(&mut values, "--query-binary")?;
    let owner_binary = optional_path(&mut values, "--owner-binary")?;
    let source_root = optional_path(&mut values, "--source-root")?;
    let source_inputs = optional_path(&mut values, "--source-inputs")?;
    if source_root.is_some() != source_inputs.is_some() {
        return Err("--source-root and --source-inputs must be supplied together".into());
    }
    let model = optional_path(&mut values, "--model")?;
    let binding = optional_path(&mut values, "--binding")?;
    if model.is_some() != binding.is_some() {
        return Err("--model and --binding must be supplied together".into());
    }
    if source_inputs.is_some() && model.is_none() {
        return Err("--source-root and --source-inputs require --model and --binding".into());
    }
    let pg_stage_mode = match take(&mut values, "--pg-stage-mode", Some("fresh"))?.as_str() {
        "fresh" => PgStageMode::Fresh,
        "existing" => PgStageMode::Existing,
        _ => return Err("--pg-stage-mode must be fresh or existing".into()),
    };
    let source_text_search_selection =
        optional_path(&mut values, "--source-text-search-selection")?;
    let owner_config = optional_path(&mut values, "--owner-config")?;
    let invocation = optional_path(&mut values, "--invocation")?;
    let token_file = optional_path(&mut values, "--token-file")?;
    let owner_stage_baseline = optional_path(&mut values, "--owner-stage-baseline")?;
    let clone_cap_was_explicit = values.contains_key("--owner-stage-clone-cap-bytes");
    let owner_stage_clone_cap_default = DEFAULT_OWNER_STAGE_CLONE_CAP_BYTES.to_string();
    let owner_stage_clone_cap_bytes = number(
        &mut values,
        "--owner-stage-clone-cap-bytes",
        &owner_stage_clone_cap_default,
    )?;
    if owner_stage_clone_cap_bytes > MAX_OWNER_STAGE_CLONE_CAP_BYTES {
        return Err("--owner-stage-clone-cap-bytes must be in 1..1073741824".into());
    }
    if clone_cap_was_explicit && owner_stage_baseline.is_none() {
        return Err("--owner-stage-clone-cap-bytes requires --owner-stage-baseline".into());
    }
    let owner_count = [
        owner_binary.is_some(),
        owner_config.is_some(),
        invocation.is_some(),
        token_file.is_some(),
    ]
    .into_iter()
    .filter(|v| *v)
    .count();
    if owner_count != 0 && owner_count != 4 {
        return Err("owner HTTP inputs must be supplied together".into());
    }
    let schedule = path(&mut values, "--schedule")?;
    let output = path(&mut values, "--output")?;
    let unit = take(&mut values, "--unit", None)?;
    if unit.is_empty() || unit.len() > 255 || !unit.is_ascii() || unit.contains('/') {
        return Err("invalid --unit component".into());
    }
    let mcp_port = number(&mut values, "--mcp-port", "5429")?;
    let owner_port = number(&mut values, "--owner-port", "44259")?;
    if mcp_port > 65535 || owner_port > 65535 || mcp_port == owner_port {
        return Err("ports must be distinct values in 1..65535".into());
    }
    let sessions = number(&mut values, "--sessions", "2")? as usize;
    if !SUPPORTED_SESSION_STAGES.contains(&sessions) {
        return Err("sessions must be one of 2, 8, 16, 64, 128, 256".into());
    }
    let sdk_in_flight = number(&mut values, "--sdk-in-flight", "4")? as usize;
    if !(1..=4).contains(&sdk_in_flight) {
        return Err("--sdk-in-flight must be one of 1, 2, 3 or 4".into());
    }
    let operation_count = values
        .remove("--operation-count")
        .map(|value| -> Result<usize, String> {
            let count = value
                .parse::<usize>()
                .map_err(|_| "invalid --operation-count".to_owned())?;
            if count == 0 || count > MAX_OPS {
                return Err("--operation-count must be in 1..4096".into());
            }
            Ok(count)
        })
        .transpose()?;
    let channel_mix = values
        .remove("--channel-mix")
        .map(|value| -> Result<BTreeMap<String, u8>, String> {
            let mut mix = BTreeMap::new();
            for item in value.split(',') {
                let (channel, weight) = item
                    .split_once('=')
                    .ok_or_else(|| "--channel-mix entries must be channel=percent".to_owned())?;
                if !matches!(channel, "mcp" | "owner_http" | "sdk" | "projection")
                    || mix.contains_key(channel)
                {
                    return Err("--channel-mix has an unknown or duplicate channel".into());
                }
                let weight = weight
                    .parse::<u8>()
                    .map_err(|_| "--channel-mix percent must be 0..100".to_owned())?;
                mix.insert(channel.to_owned(), weight);
            }
            if mix.is_empty() || mix.values().map(|weight| *weight as usize).sum::<usize>() != 100 {
                return Err("--channel-mix percentages must sum to 100".into());
            }
            Ok(mix)
        })
        .transpose()?;
    let deadline_seconds = number(&mut values, "--deadline-seconds", "120")?;
    if deadline_seconds > MAX_RUN_SECONDS {
        return Err("deadline exceeds 3600 seconds".into());
    }
    let operations_per_second = values
        .remove("--operations-per-second")
        .map(|value| -> Result<f64, String> {
            let rate = value
                .parse::<f64>()
                .map_err(|_| "invalid --operations-per-second".to_owned())?;
            if !rate.is_finite() || rate <= 0.0 || rate > MAX_OPERATIONS_PER_SECOND {
                return Err("--operations-per-second must be finite and in (0, 10000]".into());
            }
            Ok(rate)
        })
        .transpose()?;
    let load_window_seconds = values
        .remove("--load-window-seconds")
        .map(|value| -> Result<u64, String> {
            let seconds = value
                .parse::<u64>()
                .map_err(|_| "invalid --load-window-seconds".to_owned())?;
            if seconds == 0 || seconds > MAX_RUN_SECONDS {
                return Err("--load-window-seconds must be in 1..3600".into());
            }
            Ok(seconds)
        })
        .transpose()?;
    let request_cap = number(&mut values, "--request-cap-bytes", "1048576")? as usize;
    let response_cap = number(&mut values, "--response-cap-bytes", "4194304")? as usize;
    let schedule_cap = number(&mut values, "--schedule-cap-bytes", "16777216")? as usize;
    let output_cap = number(&mut values, "--output-cap-bytes", "8388608")? as usize;
    if request_cap > 1_048_576
        || response_cap > 4_194_304
        || schedule_cap > 16_777_216
        || output_cap > 67_108_864
    {
        return Err(
            "request/response/schedule/output caps exceed 1/4/16/64 MiB route limits".into(),
        );
    }
    if !values.is_empty() {
        return Err(format!("unknown option {}", values.keys().next().unwrap()));
    }
    Ok(Some(Args {
        query_binary,
        owner_binary,
        source_root,
        source_inputs,
        model,
        binding,
        pg_stage_mode,
        source_text_search_selection,
        owner_config,
        invocation,
        token_file,
        owner_stage_baseline,
        owner_stage_clone_cap_bytes,
        schedule,
        output,
        unit,
        mcp_port: mcp_port as u16,
        owner_port: owner_port as u16,
        sessions,
        sdk_in_flight,
        operation_count,
        channel_mix,
        operations_per_second,
        load_window_seconds,
        deadline_seconds,
        request_cap,
        response_cap,
        schedule_cap,
        output_cap,
    }))
}

fn sha(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

const MAX_STAGE_FILES: usize = 16_384;
const MAX_STAGE_FILE_BYTES: u64 = 33_554_432;
fn copy_stage_tree(
    source: &Path,
    destination: &Path,
    files: &mut usize,
    bytes: &mut u64,
    clone_cap_bytes: u64,
) -> Result<(), String> {
    let metadata =
        fs::symlink_metadata(source).map_err(|e| format!("stage source metadata: {e}"))?;
    let mode = metadata.mode() & 0o777;
    if metadata.uid() != unsafe { libc::geteuid() } || metadata.mode() & 0o7000 != 0 {
        return Err("owner stage contains an unowned or special-mode member".into());
    }
    if metadata.file_type().is_symlink() {
        return Err("owner stage refuses symlinks".into());
    }
    if metadata.is_dir() {
        fs::create_dir(destination).map_err(|e| format!("stage directory create: {e}"))?;
        for entry in fs::read_dir(source).map_err(|e| format!("stage directory read: {e}"))? {
            let entry = entry.map_err(|e| format!("stage directory entry: {e}"))?;
            let name = entry.file_name();
            if name.is_empty() || name == "." || name == ".." {
                return Err("owner stage contains an invalid entry name".into());
            }
            copy_stage_tree(
                &entry.path(),
                &destination.join(name),
                files,
                bytes,
                clone_cap_bytes,
            )?;
        }
        fs::set_permissions(destination, fs::Permissions::from_mode(mode))
            .map_err(|e| format!("stage directory permissions: {e}"))?;
        return Ok(());
    }
    if !metadata.is_file() || metadata.len() > MAX_STAGE_FILE_BYTES {
        return Err("owner stage contains a non-file or oversized file".into());
    }
    *files = files
        .checked_add(1)
        .ok_or("owner stage file count overflow")?;
    *bytes = bytes
        .checked_add(metadata.len())
        .filter(|total| *total <= clone_cap_bytes)
        .ok_or_else(|| {
            format!("owner stage exceeds selected clone byte cap ({clone_cap_bytes} bytes)")
        })?;
    if *files > MAX_STAGE_FILES {
        return Err("owner stage exceeds 16384-file bound".into());
    }
    let mut input = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(source)
        .map_err(|e| format!("stage source open: {e}"))?;
    let before = input
        .metadata()
        .map_err(|e| format!("stage source stat: {e}"))?;
    if before.dev() != metadata.dev()
        || before.ino() != metadata.ino()
        || before.len() != metadata.len()
    {
        return Err("owner stage member changed during selection".into());
    }
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .custom_flags(libc::O_NOFOLLOW)
        .open(destination)
        .map_err(|e| format!("stage destination create: {e}"))?;
    io::copy(&mut input.take(MAX_STAGE_FILE_BYTES + 1), &mut output)
        .map_err(|e| format!("stage file copy: {e}"))?;
    output
        .sync_all()
        .map_err(|e| format!("stage file sync: {e}"))?;
    fs::set_permissions(destination, fs::Permissions::from_mode(mode))
        .map_err(|e| format!("stage file permissions: {e}"))?;
    let after = fs::symlink_metadata(source).map_err(|e| format!("stage source recheck: {e}"))?;
    if after.dev() != metadata.dev()
        || after.ino() != metadata.ino()
        || after.len() != metadata.len()
        || after.mtime() != metadata.mtime()
        || after.mtime_nsec() != metadata.mtime_nsec()
        || after.ctime() != metadata.ctime()
        || after.ctime_nsec() != metadata.ctime_nsec()
    {
        return Err("owner stage member changed while copying".into());
    }
    Ok(())
}

fn rebase_path(path: &Path, from: &Path, to: &Path) -> Result<PathBuf, String> {
    let canonical = path
        .canonicalize()
        .map_err(|e| format!("selected stage input: {e}"))?;
    let from = from
        .canonicalize()
        .map_err(|e| format!("owner stage baseline: {e}"))?;
    let to = to
        .canonicalize()
        .map_err(|e| format!("owner stage destination: {e}"))?;
    match canonical.strip_prefix(&from) {
        Ok(relative) => Ok(to.join(relative)),
        Err(_) => Ok(canonical),
    }
}

fn rebase_json_paths(value: &mut Value, from: &Path, to: &Path) -> Result<(), String> {
    rebase_json_paths_at(value, None, from, to)
}

fn rebase_json_paths_at(
    value: &mut Value,
    key: Option<&str>,
    from: &Path,
    to: &Path,
) -> Result<(), String> {
    match value {
        Value::String(text) => {
            let path_field = matches!(
                key,
                Some(
                    "owner_config"
                        | "source_root"
                        | "private_source_root"
                        | "store_root"
                        | "namespace_path"
                        | "source_inputs_path"
                        | "corpus_store"
                        | "software_capture"
                        | "software_restored_root"
                        | "absolute_path"
                        | "path"
                )
            );
            let path = Path::new(text);
            if path_field && path.is_absolute() {
                if let Ok(canonical) = path.canonicalize()
                    && let Ok(relative) = canonical.strip_prefix(from)
                {
                    *text = to
                        .join(relative)
                        .to_str()
                        .ok_or("rebased owner stage path is not UTF-8")?
                        .to_owned();
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                rebase_json_paths_at(item, key, from, to)?;
            }
        }
        Value::Object(fields) => {
            for (child_key, item) in fields.iter_mut() {
                rebase_json_paths_at(item, Some(child_key), from, to)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn rebase_plan_paths(plan: &mut Plan, from: &Path, to: &Path) -> Result<(), String> {
    fn rebase_existing_argv_path(
        argument: &str,
        from: &Path,
        to: &Path,
        label: &str,
    ) -> Result<String, String> {
        let path = Path::new(argument);
        if !path.is_absolute() {
            return Err(format!("{label} must be absolute"));
        }
        let canonical = path
            .canonicalize()
            .map_err(|e| format!("{label} identity: {e}"))?;
        let relative = canonical
            .strip_prefix(from)
            .map_err(|_| format!("{label} must resolve inside the private stage baseline"))?;
        let staged = to
            .join(relative)
            .canonicalize()
            .map_err(|e| format!("rebased {label} identity: {e}"))?;
        if !staged.starts_with(to) {
            return Err(format!("rebased {label} escaped the private stage"));
        }
        staged
            .to_str()
            .map(str::to_owned)
            .ok_or_else(|| format!("rebased {label} path is not UTF-8"))
    }

    fn rebase_operation(operation: &mut Operation, from: &Path, to: &Path) -> Result<(), String> {
        for index in 0..operation.sdk_argv.len().saturating_sub(1) {
            if operation.sdk_argv[index] == "--invocation" {
                let argument = operation.sdk_argv[index + 1].clone();
                operation.sdk_argv[index + 1] =
                    rebase_existing_argv_path(&argument, from, to, "SDK invocation")?;
            } else if operation.sdk_argv[index] == "--selection"
                && native_pg_source_projection_read(operation)
            {
                let argument = operation.sdk_argv[index + 1].clone();
                operation.sdk_argv[index + 1] = rebase_existing_argv_path(
                    &argument,
                    from,
                    to,
                    "SDK source projection selection",
                )?;
            } else if operation.sdk_argv[index] == "--root"
                || operation.sdk_argv[index] == "--source-inputs"
                || operation.sdk_argv[index] == "--prepared-read-model"
                || operation.sdk_argv[index] == "--prepared-binding"
            {
                let argument = operation.sdk_argv[index + 1].clone();
                operation.sdk_argv[index + 1] =
                    rebase_existing_argv_path(&argument, from, to, "selected source reader input")?;
            }
        }
        for index in 0..operation.projection_argv.len().saturating_sub(1) {
            if operation.projection_argv[index] == "--selection" {
                let argument = operation.projection_argv[index + 1].clone();
                operation.projection_argv[index + 1] =
                    rebase_existing_argv_path(&argument, from, to, "projection selection")?;
            }
        }
        Ok(())
    }
    for operation in &mut plan.setup {
        rebase_operation(operation, from, to)?;
    }
    for operation in plan
        .sessions
        .iter_mut()
        .flat_map(|session| &mut session.operations)
    {
        rebase_operation(operation, from, to)?;
    }
    for operation in &mut plan.postload {
        rebase_operation(operation, from, to)?;
    }
    Ok(())
}

fn selected_sdk_owner_inputs(
    plan: &Plan,
    session_limit: usize,
) -> Result<Vec<(PathBuf, PathBuf)>, String> {
    let mut selected = BTreeMap::new();
    for operation in plan
        .setup
        .iter()
        .chain(
            plan.sessions
                .iter()
                .take(session_limit)
                .flat_map(|session| &session.operations),
        )
        .chain(plan.postload.iter())
        .filter(|operation| operation.channel == "sdk")
    {
        if native_query_source_read(operation) || native_pg_source_projection_read(operation) {
            continue;
        }
        let paths = operation
            .sdk_argv
            .windows(2)
            .filter(|pair| pair[0] == "--invocation")
            .map(|pair| PathBuf::from(&pair[1]))
            .collect::<Vec<_>>();
        if paths.len() != 1 || !Path::new(&operation.sdk_argv[0]).is_absolute() {
            return Err("SDK source command requires one explicit --invocation path".into());
        }
        let invocation = paths[0]
            .canonicalize()
            .map_err(|e| format!("SDK invocation identity: {e}"))?;
        let invocation_value = read_stage_json(&invocation)?;
        let owner_config = invocation_value
            .get("owner_config")
            .and_then(Value::as_str)
            .ok_or("SDK invocation owner_config path absent")?;
        let owner_config = PathBuf::from(owner_config)
            .canonicalize()
            .map_err(|e| format!("SDK owner configuration identity: {e}"))?;
        let previous = selected.insert(invocation, owner_config.clone());
        if previous.is_some_and(|path| path != owner_config) {
            return Err("SDK invocation selected conflicting owner configurations".into());
        }
    }
    Ok(selected.into_iter().collect())
}

fn selected_sdk_store_roots(plan: &Plan, session_limit: usize) -> Result<Vec<PathBuf>, String> {
    let mut selected = BTreeSet::new();
    for operation in plan
        .setup
        .iter()
        .chain(
            plan.sessions
                .iter()
                .take(session_limit)
                .flat_map(|session| &session.operations),
        )
        .chain(plan.postload.iter())
        .filter(|operation| operation.channel == "sdk")
    {
        if native_query_source_read(operation) {
            continue;
        }
        if native_pg_source_projection_read(operation) {
            let selection_path = projection_selection_path(operation)?;
            let selection = read_stage_json(&selection_path)?;
            let binding = projection_selection_binding(&selection, "exact-source")?;
            selected.insert(
                binding
                    .store_root
                    .canonicalize()
                    .map_err(|e| format!("SDK projection selected store root: {e}"))?,
            );
            continue;
        }
        let invocation_path = operation
            .sdk_argv
            .windows(2)
            .find(|pair| pair[0] == "--invocation")
            .map(|pair| PathBuf::from(&pair[1]))
            .ok_or("SDK source command requires one explicit --invocation path")?;
        let invocation = read_stage_json(&invocation_path)?;
        if let Some(store_root) = invocation
            .pointer("/source_selection/store_root")
            .and_then(Value::as_str)
        {
            selected.insert(
                PathBuf::from(store_root)
                    .canonicalize()
                    .map_err(|e| format!("SDK selected store root: {e}"))?,
            );
        }
    }
    Ok(selected.into_iter().collect())
}

#[derive(Clone)]
struct ProjectionSelectionBinding {
    store_root: PathBuf,
    domain: String,
    source_contract_sha256: String,
    navigation: bool,
}

fn projection_selection_path(operation: &Operation) -> Result<PathBuf, String> {
    let argv = projection_selection_argv(operation).ok_or(
        "source projection selection requires a projection or selected SDK exact-source route",
    )?;
    let paths = argv
        .windows(2)
        .filter(|pair| pair[0] == "--selection")
        .map(|pair| PathBuf::from(&pair[1]))
        .collect::<Vec<_>>();
    if paths.len() != 1
        || !paths[0].is_absolute()
        || argv
            .first()
            .is_none_or(|path| !Path::new(path).is_absolute())
    {
        return Err("projection route requires one explicit --selection path".into());
    }
    paths[0]
        .canonicalize()
        .map_err(|e| format!("projection selection identity: {e}"))
}

fn projection_selection_argv(operation: &Operation) -> Option<&[String]> {
    if operation.channel == "projection" {
        Some(&operation.projection_argv)
    } else if native_pg_source_projection_read(operation) {
        Some(&operation.sdk_argv)
    } else {
        None
    }
}

fn projection_selection_binding(
    value: &Value,
    request_operation: &str,
) -> Result<ProjectionSelectionBinding, String> {
    let request_navigation = matches!(
        request_operation,
        "bootstrap-navigation" | "advance-navigation"
    );
    let navigation = value["schema_version"] == NAVIGATION_OWNER_SCHEMA;
    if navigation != request_navigation {
        return Err("projection request and protected selection types differ".into());
    }
    let selected = if navigation {
        if value["schema_version"] != NAVIGATION_OWNER_SCHEMA
            || value["uid"].as_u64() != Some(unsafe { libc::geteuid() } as u64)
            || value["expires_at"].as_str().is_none_or(str::is_empty)
            || value["source_root"].as_str().is_none_or(str::is_empty)
        {
            return Err("navigation projection requires its protected owner configuration".into());
        }
        &value["source_navigation"]
    } else {
        value
    };
    let expected_schema = if navigation {
        NAVIGATION_SELECTION_SCHEMA
    } else {
        "tos_source_projection_selection_v1"
    };
    if selected["schema_version"] != expected_schema {
        return Err("projection selection schema does not match its request".into());
    }
    let store_root = selected["store_root"]
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or("projection selected store root absent")?;
    let domain = selected["domain"]
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or("projection selected domain absent")?;
    let source_contract_sha256 = selected["source_contract_sha256"]
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or("projection selected source contract absent")?;
    let store_root = PathBuf::from(store_root);
    if !store_root.is_absolute() {
        return Err("projection selected store root must be absolute".into());
    }
    if navigation {
        for (label, path) in [
            ("source root", value["source_root"].as_str()),
            ("navigation model", selected["model_path"].as_str()),
        ] {
            if path.is_none_or(|path| path.is_empty() || !Path::new(path).is_absolute()) {
                return Err(format!(
                    "navigation {label} must be an explicit absolute path"
                ));
            }
        }
        let producer = selected
            .get("producer")
            .and_then(Value::as_object)
            .ok_or("navigation projection producer selection absent")?;
        if producer.get("schema_version").and_then(Value::as_str)
            != Some(NAVIGATION_PRODUCER_SCHEMA)
            || producer
                .get("max_events")
                .and_then(Value::as_u64)
                .is_none_or(|max_events| !(1..=256).contains(&max_events))
            || producer
                .get("schema_worker_path")
                .and_then(Value::as_str)
                .is_none_or(|path| path.is_empty() || !Path::new(path).is_absolute())
            || producer
                .get("schema_worker_sha256")
                .and_then(Value::as_str)
                .is_none_or(|digest| !valid_sha(digest))
        {
            return Err(
                "navigation projection producer selection is incomplete or unbounded".into(),
            );
        }
    }
    Ok(ProjectionSelectionBinding {
        store_root,
        domain: domain.to_owned(),
        source_contract_sha256: source_contract_sha256.to_owned(),
        navigation,
    })
}

fn selected_projection_inputs(plan: &Plan, session_limit: usize) -> Result<Vec<PathBuf>, String> {
    let mut selected = BTreeSet::new();
    for operation in plan
        .setup
        .iter()
        .chain(
            plan.sessions
                .iter()
                .take(session_limit)
                .flat_map(|session| &session.operations),
        )
        .chain(plan.postload.iter())
        .filter(|operation| {
            operation.channel == "projection" || native_pg_source_projection_read(operation)
        })
    {
        let selection = projection_selection_path(operation)?;
        let value = read_stage_json(&selection)?;
        projection_selection_binding(
            &value,
            operation.request["operation"]
                .as_str()
                .ok_or("projection request operation absent")?,
        )?;
        selected.insert(selection);
    }
    Ok(selected.into_iter().collect())
}

fn required_stage_path(
    value: &mut Value,
    pointer: &str,
    from: &Path,
    to: &Path,
) -> Result<(), String> {
    let text = value
        .pointer(pointer)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .ok_or_else(|| format!("navigation owner path absent at {pointer}"))?;
    let path = Path::new(text);
    if !path.is_absolute() {
        return Err(format!(
            "navigation owner path must be absolute at {pointer}"
        ));
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|e| format!("navigation owner path metadata at {pointer}: {e}"))?;
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "navigation owner path cannot be a symlink at {pointer}"
        ));
    }
    let canonical = path
        .canonicalize()
        .map_err(|e| format!("navigation owner path identity at {pointer}: {e}"))?;
    let relative = canonical.strip_prefix(from).map_err(|_| {
        format!("navigation owner path must be inside the private stage at {pointer}")
    })?;
    let staged = to
        .join(relative)
        .canonicalize()
        .map_err(|e| format!("rebased navigation owner path at {pointer}: {e}"))?;
    if !staged.starts_with(to) {
        return Err(format!(
            "rebased navigation owner path escaped the stage at {pointer}"
        ));
    }
    let parent = pointer
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .ok_or("invalid navigation owner path pointer")?;
    let key = pointer
        .rsplit('/')
        .next()
        .ok_or("invalid navigation owner path pointer")?;
    let owner = value
        .pointer_mut(parent)
        .and_then(Value::as_object_mut)
        .ok_or_else(|| format!("navigation owner object absent at {parent}"))?;
    owner.insert(
        key.to_owned(),
        json!(
            staged
                .to_str()
                .ok_or("staged navigation path is not UTF-8")?
        ),
    );
    Ok(())
}

fn validate_navigation_stage_paths(value: &Value, baseline: &Path) -> Result<(), String> {
    for pointer in [
        "/source_root",
        "/source_navigation/store_root",
        "/source_navigation/model_path",
    ] {
        let text = value
            .pointer(pointer)
            .and_then(Value::as_str)
            .filter(|text| !text.is_empty())
            .ok_or_else(|| format!("navigation owner path absent at {pointer}"))?;
        let path = Path::new(text);
        if !path.is_absolute() {
            return Err(format!(
                "navigation owner path must be absolute at {pointer}"
            ));
        }
        let metadata = fs::symlink_metadata(path)
            .map_err(|e| format!("navigation owner path metadata at {pointer}: {e}"))?;
        if metadata.file_type().is_symlink()
            || !path
                .canonicalize()
                .map_err(|e| format!("navigation owner path identity at {pointer}: {e}"))?
                .starts_with(baseline)
        {
            return Err(format!(
                "navigation owner path must resolve inside the private stage at {pointer}"
            ));
        }
    }
    if let Some(worker_path) = value
        .pointer("/source_navigation/producer/schema_worker_path")
        .and_then(Value::as_str)
    {
        let worker = Path::new(worker_path);
        if !worker.is_absolute() {
            return Err("navigation schema worker path must be absolute".into());
        }
        let canonical = worker
            .canonicalize()
            .map_err(|e| format!("navigation schema worker identity: {e}"))?;
        if canonical.starts_with(baseline) {
            let metadata = fs::symlink_metadata(worker)
                .map_err(|e| format!("navigation schema worker metadata: {e}"))?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(
                    "staged navigation schema worker must be a regular non-symlink file".into(),
                );
            }
        }
    }
    Ok(())
}

fn rebase_navigation_owner_paths(value: &mut Value, from: &Path, to: &Path) -> Result<(), String> {
    // This protected selection also carries policy, proof, identity and
    // expectation evidence. Rebase only its known absolute paths and compare
    // the remaining JSON tree unchanged.
    let original = value.clone();
    for pointer in [
        "/source_root",
        "/source_navigation/store_root",
        "/source_navigation/model_path",
    ] {
        required_stage_path(value, pointer, from, to)?;
    }
    if value
        .pointer("/source_navigation/producer/schema_worker_path")
        .is_some()
    {
        let worker_path = value["source_navigation"]["producer"]["schema_worker_path"]
            .as_str()
            .ok_or("navigation schema worker path is invalid")?;
        let worker = Path::new(worker_path);
        if !worker.is_absolute() {
            return Err("navigation schema worker path must be absolute".into());
        }
        let canonical = worker
            .canonicalize()
            .map_err(|e| format!("navigation schema worker identity: {e}"))?;
        if let Ok(relative) = canonical.strip_prefix(from) {
            let metadata = fs::symlink_metadata(worker)
                .map_err(|e| format!("navigation schema worker metadata: {e}"))?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(
                    "staged navigation schema worker must be a regular non-symlink file".into(),
                );
            }
            let staged = to.join(relative);
            let staged_metadata = fs::symlink_metadata(&staged)
                .map_err(|e| format!("staged navigation schema worker metadata: {e}"))?;
            if !staged_metadata.is_file()
                || staged_metadata.file_type().is_symlink()
                || !staged.starts_with(to)
            {
                return Err("staged navigation schema worker is unavailable".into());
            }
            value["source_navigation"]["producer"]["schema_worker_path"] = json!(
                staged
                    .to_str()
                    .ok_or("staged schema worker path is not UTF-8")?
            );
        }
    }
    let mut original_non_paths = original;
    let mut rebased_non_paths = value.clone();
    for pointer in [
        "/source_root",
        "/source_navigation/store_root",
        "/source_navigation/model_path",
        "/source_navigation/producer/schema_worker_path",
    ] {
        if let Some(path) = original_non_paths.pointer_mut(pointer) {
            *path = Value::Null;
        }
        if let Some(path) = rebased_non_paths.pointer_mut(pointer) {
            *path = Value::Null;
        }
    }
    if original_non_paths != rebased_non_paths {
        return Err("navigation stage rebase changed protected non-path configuration".into());
    }
    Ok(())
}

fn read_stage_json(path: &Path) -> Result<Value, String> {
    let metadata = fs::symlink_metadata(path).map_err(|e| format!("owner stage JSON stat: {e}"))?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o077 != 0
        || metadata.len() > 1_048_576
    {
        return Err("owner stage configuration must be a private owned JSON file".into());
    }
    let raw = fs::read(path).map_err(|e| format!("owner stage JSON read: {e}"))?;
    serde_json::from_slice(&raw).map_err(|_| "owner stage configuration JSON invalid".into())
}

fn staged_owner_configuration_digest(
    invocation_path: &Path,
    baseline: &Path,
) -> Result<String, String> {
    let invocation = read_stage_json(invocation_path)?;
    let owner_path = PathBuf::from(
        invocation["owner_config"]
            .as_str()
            .ok_or("native invocation owner_config path absent")?,
    )
    .canonicalize()
    .map_err(|e| format!("native owner configuration path: {e}"))?;
    if !owner_path.starts_with(baseline) {
        return Err("native owner configuration must remain inside the private stage".into());
    }
    let _ = read_stage_json(&owner_path)?;
    let raw = fs::read(&owner_path).map_err(|e| format!("native owner configuration read: {e}"))?;
    let canonical = tos_foundation::canonical_raw_bytes_v1(
        &raw,
        tos_foundation::CanonicalProfile::SourceCommandInputV1,
        tos_foundation::JsonLimits::default(),
    )
    .map_err(|_| "native owner configuration canonicalization failed")?;
    Ok(format!("sha256:{}", sha(&canonical)))
}

fn prepare_owner_stage(
    args: &mut Args,
    plan: &mut Plan,
    needs_owner: bool,
    deadline: Instant,
) -> Result<Option<(PathBuf, PathBuf, usize, usize, u64)>, String> {
    let Some(baseline) = args.owner_stage_baseline.clone() else {
        return Ok(None);
    };
    let baseline = baseline
        .canonicalize()
        .map_err(|e| format!("owner stage baseline: {e}"))?;
    let baseline_metadata =
        fs::symlink_metadata(&baseline).map_err(|e| format!("owner stage baseline stat: {e}"))?;
    if baseline_metadata.file_type().is_symlink()
        || !baseline_metadata.is_dir()
        || baseline_metadata.uid() != unsafe { libc::geteuid() }
        || baseline_metadata.mode() & 0o077 != 0
        || args.output.starts_with(&baseline)
        || baseline.starts_with(&args.output)
    {
        return Err("owner stage baseline must be a separate private owned directory".into());
    }
    let sdk_owner_inputs = selected_sdk_owner_inputs(plan, args.sessions)?;
    let sdk_store_roots = selected_sdk_store_roots(plan, args.sessions)?;
    let projection_inputs = selected_projection_inputs(plan, args.sessions)?;
    let selected_operations = plan
        .setup
        .iter()
        .chain(
            plan.sessions
                .iter()
                .take(args.sessions)
                .flat_map(|session| &session.operations),
        )
        .chain(plan.postload.iter())
        .collect::<Vec<_>>();
    let mcp_requires_prepared_owner = selected_operations.iter().any(|operation| {
        operation.channel == "mcp" && !mcp_source_text_search_operation(operation)
    });
    let mcp_requires_source_reader = selected_operations.iter().any(|operation| {
        operation.channel == "mcp"
            && source_read_operation(operation)
            && !mcp_source_text_search_operation(operation)
    });
    if mcp_requires_prepared_owner && (args.model.is_none() || args.binding.is_none()) {
        return Err(
            "MCP operations beyond selected PG source-text search require --model and --binding"
                .into(),
        );
    }
    if mcp_requires_source_reader && (args.source_root.is_none() || args.source_inputs.is_none()) {
        return Err("MCP source-reader staging requires --source-root and --source-inputs".into());
    }
    let mut staged_inputs = Vec::new();
    if let Some(model) = args.model.as_deref() {
        staged_inputs.push(model);
    }
    if let Some(binding) = args.binding.as_deref() {
        staged_inputs.push(binding);
    }
    if let Some(source_root) = args.source_root.as_deref() {
        staged_inputs.push(source_root);
    }
    if let Some(source_inputs) = args.source_inputs.as_deref() {
        staged_inputs.push(source_inputs);
    }
    if let Some(selection) = args.source_text_search_selection.as_deref() {
        staged_inputs.push(selection);
    }
    if needs_owner {
        for (label, path) in [
            ("owner HTTP configuration", args.owner_config.as_mut()),
            ("owner HTTP invocation", args.invocation.as_mut()),
            ("owner HTTP token", args.token_file.as_mut()),
        ] {
            let path = path.ok_or_else(|| format!("{label} path absent"))?;
            let canonical = path
                .canonicalize()
                .map_err(|e| format!("{label} identity: {e}"))?;
            if !canonical.starts_with(&baseline) {
                return Err(format!(
                    "{label} must remain inside the private stage baseline"
                ));
            }
            *path = canonical;
            staged_inputs.push(path);
        }
    }
    for path in staged_inputs {
        if !path
            .canonicalize()
            .map_err(|e| format!("owner stage input: {e}"))?
            .starts_with(&baseline)
        {
            return Err("all supplied model, binding, reader and owner inputs must be inside the selected owner stage baseline".into());
        }
    }
    for (invocation, owner) in &sdk_owner_inputs {
        if !invocation.starts_with(&baseline) || !owner.starts_with(&baseline) {
            return Err("every SDK invocation and owner configuration must be inside the selected owner stage baseline".into());
        }
    }
    for store_root in &sdk_store_roots {
        let invalid = match args.pg_stage_mode {
            PgStageMode::Fresh => !store_root.starts_with(&baseline),
            PgStageMode::Existing => store_root.starts_with(&baseline),
        };
        if invalid {
            return Err(match args.pg_stage_mode {
                PgStageMode::Fresh => "fresh SDK PostgreSQL store must be inside the selected stage baseline",
                PgStageMode::Existing => "existing SDK PostgreSQL store must remain outside the cloned owner-input baseline",
            }
            .into());
        }
    }
    for selection in &projection_inputs {
        let value = read_stage_json(selection)?;
        let request_operation = if value["schema_version"] == NAVIGATION_OWNER_SCHEMA {
            "advance-navigation"
        } else {
            "status"
        };
        let binding = projection_selection_binding(&value, request_operation)?;
        let store_root = binding
            .store_root
            .canonicalize()
            .map_err(|e| format!("projection store root: {e}"))?;
        let store_mismatch = match args.pg_stage_mode {
            PgStageMode::Fresh => !store_root.starts_with(&baseline),
            PgStageMode::Existing => store_root.starts_with(&baseline),
        };
        if !selection.starts_with(&baseline) || store_mismatch {
            return Err(
                "projection selection must be staged and its PostgreSQL store must match the selected fresh/existing stage mode"
                    .into(),
            );
        }
        if binding.navigation {
            validate_navigation_stage_paths(&value, &baseline)?;
        }
    }
    let destination = args.output.join("owner-stage");
    if destination.exists() || destination.is_symlink() {
        return Err("owner stage destination must be fresh".into());
    }
    let mut files = 0usize;
    let mut bytes = 0u64;
    if let Err(error) = copy_stage_tree(
        &baseline,
        &destination,
        &mut files,
        &mut bytes,
        args.owner_stage_clone_cap_bytes,
    ) {
        let _ = fs::remove_dir_all(&destination);
        return Err(error);
    }
    let destination = destination
        .canonicalize()
        .map_err(|e| format!("owner stage destination: {e}"))?;
    rebase_plan_paths(plan, &baseline, &destination)?;
    args.model = args
        .model
        .as_deref()
        .map(|path| rebase_path(path, &baseline, &destination))
        .transpose()?;
    args.binding = args
        .binding
        .as_deref()
        .map(|path| rebase_path(path, &baseline, &destination))
        .transpose()?;
    args.source_root = args
        .source_root
        .as_deref()
        .map(|path| rebase_path(path, &baseline, &destination))
        .transpose()?;
    args.source_inputs = args
        .source_inputs
        .as_deref()
        .map(|path| rebase_path(path, &baseline, &destination))
        .transpose()?;
    args.source_text_search_selection = args
        .source_text_search_selection
        .as_deref()
        .map(|path| rebase_path(path, &baseline, &destination))
        .transpose()?;
    if needs_owner {
        args.owner_config = args
            .owner_config
            .as_deref()
            .map(|path| rebase_path(path, &baseline, &destination))
            .transpose()?;
        args.invocation = args
            .invocation
            .as_deref()
            .map(|path| rebase_path(path, &baseline, &destination))
            .transpose()?;
        args.token_file = args
            .token_file
            .as_deref()
            .map(|path| rebase_path(path, &baseline, &destination))
            .transpose()?;
    }
    if needs_owner {
        let owner_path = args
            .owner_config
            .as_ref()
            .ok_or("owner HTTP configuration absent after staging")?;
        let invocation_path = args
            .invocation
            .as_ref()
            .ok_or("owner HTTP invocation absent after staging")?;
        let owner_mode = fs::metadata(owner_path)
            .map_err(|e| format!("owner stage config metadata: {e}"))?
            .mode()
            & 0o777;
        let invocation_mode = fs::metadata(invocation_path)
            .map_err(|e| format!("owner stage invocation metadata: {e}"))?
            .mode()
            & 0o777;
        let mut owner = read_stage_json(owner_path)?;
        rebase_json_paths(&mut owner, &baseline, &destination)?;
        fs::write(
            owner_path,
            serde_json::to_vec(&owner).map_err(|_| "owner stage config encode failed")?,
        )
        .map_err(|e| format!("owner stage config write: {e}"))?;
        fs::set_permissions(owner_path, fs::Permissions::from_mode(owner_mode))
            .map_err(|e| format!("owner stage config mode: {e}"))?;
        let mut invocation = read_stage_json(invocation_path)?;
        rebase_json_paths(&mut invocation, &baseline, &destination)?;
        if invocation["owner_config"]
            != owner_path
                .to_str()
                .ok_or("owner config path UTF-8 required")?
        {
            return Err("staged invocation does not select the staged owner configuration".into());
        }
        fs::write(
            invocation_path,
            serde_json::to_vec(&invocation).map_err(|_| "owner stage invocation encode failed")?,
        )
        .map_err(|e| format!("owner stage invocation write: {e}"))?;
        fs::set_permissions(invocation_path, fs::Permissions::from_mode(invocation_mode))
            .map_err(|e| format!("owner stage invocation mode: {e}"))?;
    }
    if let Some(source_inputs_path) = args.source_inputs.as_ref() {
        let source_inputs_mode = fs::metadata(source_inputs_path)
            .map_err(|e| format!("owner stage source inputs metadata: {e}"))?
            .mode()
            & 0o777;
        let mut source_inputs = read_stage_json(&source_inputs_path)?;
        rebase_json_paths(&mut source_inputs, &baseline, &destination)?;
        fs::write(
            &source_inputs_path,
            serde_json::to_vec(&source_inputs)
                .map_err(|_| "owner stage source inputs encode failed")?,
        )
        .map_err(|e| format!("owner stage source inputs write: {e}"))?;
        fs::set_permissions(
            source_inputs_path,
            fs::Permissions::from_mode(source_inputs_mode),
        )
        .map_err(|e| format!("owner stage source inputs mode: {e}"))?;
        // The exact source-input snapshot is duplicated in the prepared database.
        // When its absolute namespace roots move with this stage clone, update only
        // that bound row and digest so the query opens the cloned roots.
        let source_inputs_raw = tos_foundation::canonical_raw_bytes_v1(
            &serde_json::to_vec(&source_inputs).map_err(|_| "source inputs encode failed")?,
            tos_foundation::CanonicalProfile::CorpusSnapshotV1,
            tos_foundation::JsonLimits::default(),
        )
        .map_err(|_| "source inputs canonicalization failed")?;
        let source_inputs_digest = sha(&source_inputs_raw);
        let source_inputs_text =
            std::str::from_utf8(&source_inputs_raw).map_err(|_| "source inputs UTF-8 required")?;
        let model = args
            .model
            .as_deref()
            .ok_or("prepared model absent for staged source inputs")?;
        let database = rusqlite::Connection::open(model)
            .map_err(|_| "owner stage prepared database open failed")?;
        let changed = database
            .execute(
                "UPDATE prepared_source_state SET inputs=?1,sha256=?2 WHERE singleton=1",
                rusqlite::params![source_inputs_text, source_inputs_digest],
            )
            .map_err(|_| "owner stage prepared source state update failed")?;
        if changed != 1 {
            return Err("owner stage prepared source state row unavailable".into());
        }
        drop(database);
        fs::write(&source_inputs_path, &source_inputs_raw)
            .map_err(|e| format!("owner stage canonical source inputs write: {e}"))?;
        fs::set_permissions(
            source_inputs_path,
            fs::Permissions::from_mode(source_inputs_mode),
        )
        .map_err(|e| format!("owner stage canonical source inputs mode: {e}"))?;
    }
    if let Some(source_text_selection_path) = args.source_text_search_selection.as_ref() {
        let selection_mode = fs::metadata(source_text_selection_path)
            .map_err(|e| format!("owner stage PG source-text selection metadata: {e}"))?
            .mode()
            & 0o777;
        let mut selection = read_stage_json(source_text_selection_path)?;
        rebase_json_paths(&mut selection, &baseline, &destination)?;
        fs::write(
            source_text_selection_path,
            serde_json::to_vec(&selection)
                .map_err(|_| "owner stage PG source-text selection encode failed")?,
        )
        .map_err(|e| format!("owner stage PG source-text selection write: {e}"))?;
        fs::set_permissions(
            source_text_selection_path,
            fs::Permissions::from_mode(selection_mode),
        )
        .map_err(|e| format!("owner stage PG source-text selection mode: {e}"))?;
    }
    for (source_invocation, source_owner) in sdk_owner_inputs {
        let invocation_path = rebase_path(&source_invocation, &baseline, &destination)?;
        let owner_path = rebase_path(&source_owner, &baseline, &destination)?;
        let invocation_mode = fs::metadata(&invocation_path)
            .map_err(|e| format!("SDK invocation metadata: {e}"))?
            .mode()
            & 0o777;
        let owner_mode = fs::metadata(&owner_path)
            .map_err(|e| format!("SDK owner configuration metadata: {e}"))?
            .mode()
            & 0o777;
        let mut owner = read_stage_json(&owner_path)?;
        rebase_json_paths(&mut owner, &baseline, &destination)?;
        fs::write(
            &owner_path,
            serde_json::to_vec(&owner).map_err(|_| "SDK owner configuration encode failed")?,
        )
        .map_err(|e| format!("SDK owner configuration write: {e}"))?;
        fs::set_permissions(&owner_path, fs::Permissions::from_mode(owner_mode))
            .map_err(|e| format!("SDK owner configuration mode: {e}"))?;
        let mut invocation = read_stage_json(&invocation_path)?;
        rebase_json_paths(&mut invocation, &baseline, &destination)?;
        if invocation["owner_config"]
            != owner_path.to_str().ok_or("SDK owner path UTF-8 required")?
        {
            return Err(
                "staged SDK invocation does not select its staged owner configuration".into(),
            );
        }
        fs::write(
            &invocation_path,
            serde_json::to_vec(&invocation).map_err(|_| "SDK invocation encode failed")?,
        )
        .map_err(|e| format!("SDK invocation write: {e}"))?;
        fs::set_permissions(
            &invocation_path,
            fs::Permissions::from_mode(invocation_mode),
        )
        .map_err(|e| format!("SDK invocation mode: {e}"))?;
    }
    for source_selection in projection_inputs {
        let selection_path = rebase_path(&source_selection, &baseline, &destination)?;
        let selection_mode = fs::metadata(&selection_path)
            .map_err(|e| format!("projection selection metadata: {e}"))?
            .mode()
            & 0o777;
        let mut selection = read_stage_json(&selection_path)?;
        if selection["schema_version"] == NAVIGATION_OWNER_SCHEMA {
            rebase_navigation_owner_paths(&mut selection, &baseline, &destination)?;
        } else {
            rebase_json_paths(&mut selection, &baseline, &destination)?;
        }
        fs::write(
            &selection_path,
            serde_json::to_vec(&selection).map_err(|_| "projection selection encode failed")?,
        )
        .map_err(|e| format!("projection selection write: {e}"))?;
        fs::set_permissions(&selection_path, fs::Permissions::from_mode(selection_mode))
            .map_err(|e| format!("projection selection mode: {e}"))?;
    }
    let mut configuration_bindings = 0usize;
    for operation in plan
        .setup
        .iter_mut()
        .chain(plan.postload.iter_mut())
        .filter(|operation| operation.bind_expected_configuration_from_owner)
    {
        if operation.channel != "sdk" || operation_name(operation) != Some("describe") {
            return Err(
                "native owner configuration binding is limited to a serial describe operation"
                    .into(),
            );
        }
        if operation.request["expected_configuration"] != Value::Null {
            return Err("native rights describe configuration must be bound from the staged owner, not supplied in the schedule".into());
        }
        let invocation_path = PathBuf::from(
            invocation_argument(operation).ok_or("native describe invocation absent")?,
        )
        .canonicalize()
        .map_err(|e| format!("native describe invocation path: {e}"))?;
        let invocation = read_stage_json(&invocation_path)?;
        if invocation["schema_version"] != "tos_local_pg_rights_revoke_invocation_v1" {
            return Err("automatic native configuration binding requires the ordinary PG rights describe route".into());
        }
        let digest = staged_owner_configuration_digest(&invocation_path, &destination)?;
        operation.request["expected_configuration"] = json!(digest);
        configuration_bindings += 1;
    }
    if plan
        .sessions
        .iter()
        .flat_map(|session| &session.operations)
        .any(|operation| operation.bind_expected_configuration_from_owner)
    {
        let owner_binary = args
            .owner_binary
            .as_ref()
            .ok_or("owner configuration binding requires selected native owner binary")?;
        let invocation = args
            .invocation
            .as_ref()
            .ok_or("owner configuration binding requires staged invocation")?;
        let preflight = Operation {
            channel: "sdk".into(),
            label: "owner-configuration-binding".into(),
            request: json!({
                "schema_version":"tos_local_source_command_v1",
                "operation":"describe"
            }),
            expected_status: 0,
            expected_sha256: None,
            expected_body: None,
            expected_stderr_prefix: None,
            expected_rights_allowed: None,
            ignored_json_paths: Vec::new(),
            alternate_outcomes: Vec::new(),
            retry_same_command_id: false,
            idempotency_replay_of: None,
            bound_revision_preview: None,
            bound_replay_response: None,
            reconnect_before: false,
            conflict_group: None,
            bind_expected_configuration_from_owner: false,
            bind_expected_from: None,
            sdk_argv: vec![
                owner_binary
                    .to_str()
                    .ok_or("selected owner binary path UTF-8 required")?
                    .to_owned(),
                "source-commands".into(),
                "--invocation".into(),
                invocation
                    .to_str()
                    .ok_or("staged invocation path UTF-8 required")?
                    .to_owned(),
            ],
            projection_argv: Vec::new(),
            interrupt_resume_after_ms: None,
        };
        let pids = Arc::new(Mutex::new(Vec::new()));
        let attempt = sdk_process(
            &preflight,
            &args.output,
            deadline,
            args.request_cap,
            args.response_cap,
            &pids,
            false,
        )?;
        if attempt.status != 0 {
            return Err("selected native owner configuration describe failed".into());
        }
        let response: Value = serde_json::from_slice(
            attempt
                .body
                .as_deref()
                .ok_or("native owner describe body absent")?,
        )
        .map_err(|_| "native owner describe response is not JSON")?;
        let digest = response
            .pointer("/result/owner_configuration")
            .and_then(Value::as_str)
            .filter(|value| value.strip_prefix("sha256:").is_some_and(valid_sha))
            .ok_or("native owner describe lacks a selected configuration digest")?
            .to_owned();
        for operation in plan
            .sessions
            .iter_mut()
            .flat_map(|session| &mut session.operations)
            .filter(|operation| operation.bind_expected_configuration_from_owner)
        {
            if operation.channel != "owner_http"
                || operation.request.get("expected_configuration").is_none()
            {
                return Err(
                    "owner configuration binding requires owner_http expected_configuration".into(),
                );
            }
            operation.request["expected_configuration"] = json!(digest);
            configuration_bindings += 1;
        }
    }
    args.owner_stage_baseline = Some(destination.clone());
    Ok(Some((
        baseline,
        destination,
        configuration_bindings,
        files,
        bytes,
    )))
}
fn executable_sha256(path: &Path, deadline: Instant) -> Result<String, String> {
    const MAX_EXECUTABLE_BYTES: u64 = 536_870_912;
    let before = fs::symlink_metadata(path).map_err(|e| format!("executable metadata: {e}"))?;
    if !before.is_file()
        || before.file_type().is_symlink()
        || before.len() > MAX_EXECUTABLE_BYTES
        || before.mode() & 0o111 == 0
    {
        return Err("selected executable must be a bounded regular executable file".into());
    }
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|e| format!("executable open: {e}"))?;
    let opened = file
        .metadata()
        .map_err(|e| format!("executable FD metadata: {e}"))?;
    let stamp = |m: &fs::Metadata| {
        (
            m.dev(),
            m.ino(),
            m.len(),
            m.mtime(),
            m.mtime_nsec(),
            m.ctime(),
            m.ctime_nsec(),
            m.mode(),
        )
    };
    if stamp(&before) != stamp(&opened) {
        return Err("executable path/FD identity changed before hashing".into());
    }
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        if STOP.load(Ordering::Acquire) || Instant::now() >= deadline {
            return Err("executable identity deadline/cancel".into());
        }
        let n = file
            .read(&mut buffer)
            .map_err(|e| format!("executable hash read: {e}"))?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    let after = file
        .metadata()
        .map_err(|e| format!("executable FD final metadata: {e}"))?;
    let path_after =
        fs::symlink_metadata(path).map_err(|e| format!("executable path final metadata: {e}"))?;
    if stamp(&opened) != stamp(&after) || stamp(&opened) != stamp(&path_after) {
        return Err("executable changed during identity hash".into());
    }
    Ok(hex(&hash.finalize()))
}
fn valid_sha(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn normalized_sha256(value: &str) -> Option<&str> {
    let value = value.strip_prefix("sha256:").unwrap_or(value);
    valid_sha(value).then_some(value)
}
fn hex(raw: &[u8]) -> String {
    raw.iter().map(|b| format!("{b:02x}")).collect()
}
fn unhex(value: &str) -> Result<Vec<u8>, String> {
    if value.len() % 2 != 0 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("invalid hex".into());
    }
    (0..value.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&value[i..i + 2], 16).map_err(|_| "invalid hex".into()))
        .collect()
}
fn json_bytes(value: &Value) -> Result<Vec<u8>, String> {
    serde_json::to_vec(value).map_err(|_| "JSON serialization failed".into())
}
fn file_stamp(path: &Path, deadline: Instant) -> Result<InputStamp, String> {
    if !path.is_absolute() {
        return Err("input path must be absolute".into());
    }
    let before = fs::symlink_metadata(path).map_err(|e| format!("input metadata: {e}"))?;
    if !before.is_file() || before.file_type().is_symlink() {
        return Err("input must be a regular non-symlink file".into());
    }
    if STOP.load(Ordering::Acquire) || Instant::now() >= deadline {
        return Err("input identity deadline/cancel".into());
    }
    let stamp = |m: &fs::Metadata| {
        (
            m.dev(),
            m.ino(),
            m.len(),
            m.mtime(),
            m.mtime_nsec(),
            m.ctime(),
            m.ctime_nsec(),
            m.mode(),
        )
    };
    let path_after = fs::symlink_metadata(path).map_err(|e| format!("input path metadata: {e}"))?;
    if stamp(&before) != stamp(&path_after) {
        return Err("input changed during identity check".into());
    }
    Ok(InputStamp {
        path: path.to_owned(),
        dev: before.dev(),
        ino: before.ino(),
        len: before.len(),
        mtime: before.mtime(),
        mtime_nsec: before.mtime_nsec(),
        ctime: before.ctime(),
        ctime_nsec: before.ctime_nsec(),
        mode: before.mode(),
    })
}
fn stamp_equal(a: &InputStamp, b: &InputStamp) -> bool {
    a.path == b.path
        && a.dev == b.dev
        && a.ino == b.ino
        && a.len == b.len
        && a.mtime == b.mtime
        && a.mtime_nsec == b.mtime_nsec
        && a.ctime == b.ctime
        && a.ctime_nsec == b.ctime_nsec
        && a.mode == b.mode
}
fn identity_json(stamp: &InputStamp, sha256: Option<&str>) -> Value {
    json!({"path":stamp.path,"device":stamp.dev,"inode":stamp.ino,"size_bytes":stamp.len,"mtime_seconds":stamp.mtime,"mtime_nanoseconds":stamp.mtime_nsec,"ctime_seconds":stamp.ctime,"ctime_nanoseconds":stamp.ctime_nsec,"mode":stamp.mode,"sha256":sha256})
}
fn token_bytes(path: &Path) -> Result<Vec<u8>, String> {
    use std::os::unix::fs::MetadataExt;
    let m = fs::symlink_metadata(path).map_err(|e| format!("token metadata: {e}"))?;
    if !m.is_file()
        || m.file_type().is_symlink()
        || m.uid() != unsafe { libc::geteuid() }
        || m.mode() & 0o077 != 0
        || !(m.len() == 64 || m.len() == 65)
    {
        return Err("token must be owner-only 64 lowercase hex bytes".into());
    }
    let mut f = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|e| format!("token open: {e}"))?;
    let mut raw = Vec::new();
    f.take(66)
        .read_to_end(&mut raw)
        .map_err(|e| format!("token read: {e}"))?;
    if raw.last() == Some(&b'\n') {
        raw.pop();
    }
    if raw.len() != 64
        || !raw
            .iter()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b))
    {
        return Err("invalid transport token".into());
    }
    unhex(std::str::from_utf8(&raw).unwrap())
}
fn cgroup_path(unit: &str) -> Result<PathBuf, String> {
    let content =
        fs::read_to_string("/proc/self/cgroup").map_err(|e| format!("cgroup identity: {e}"))?;
    let relative = content
        .lines()
        .find_map(|line| line.strip_prefix("0::"))
        .ok_or("cgroup v2 is required for aggregate metrics")?;
    if !relative.split('/').any(|part| part == unit) {
        return Err("current cgroup does not match --unit".into());
    }
    let root = Path::new("/sys/fs/cgroup").join(relative.trim_start_matches('/'));
    if !root.is_dir() {
        return Err("selected cgroup v2 directory unavailable".into());
    }
    Ok(root)
}
fn read_kv(path: &Path, key: &str) -> Result<u64, String> {
    let text =
        fs::read_to_string(path).map_err(|e| format!("resource metric {}: {e}", path.display()))?;
    text.lines()
        .find_map(|line| {
            let mut parts = line.split_whitespace();
            (parts.next() == Some(key))
                .then(|| parts.next()?.parse().ok())
                .flatten()
        })
        .ok_or_else(|| format!("resource metric {key} absent"))
}
fn resource_snapshot(root: &Path) -> Result<ResourceSnapshot, String> {
    let io_stat = fs::read_to_string(root.join("io.stat")).map_err(|e| format!("io.stat: {e}"))?;
    let mut io_read = 0u64;
    let mut io_write = 0u64;
    for line in io_stat.lines() {
        for field in line.split_whitespace().skip(1) {
            if let Some(v) = field.strip_prefix("rbytes=") {
                io_read = io_read
                    .checked_add(v.parse::<u64>().map_err(|_| "invalid io.stat rbytes")?)
                    .ok_or("io.stat overflow")?;
            }
            if let Some(v) = field.strip_prefix("wbytes=") {
                io_write = io_write
                    .checked_add(v.parse::<u64>().map_err(|_| "invalid io.stat wbytes")?)
                    .ok_or("io.stat overflow")?;
            }
        }
    }
    let memory_events = root.join("memory.events");
    Ok(ResourceSnapshot {
        cpu_usec: read_kv(&root.join("cpu.stat"), "usage_usec")?,
        memory_current: fs::read_to_string(root.join("memory.current"))
            .map_err(|e| format!("memory.current: {e}"))?
            .trim()
            .parse()
            .map_err(|_| "invalid memory.current")?,
        memory_peak: fs::read_to_string(root.join("memory.peak"))
            .map_err(|e| format!("memory.peak: {e}"))?
            .trim()
            .parse()
            .map_err(|_| "invalid memory.peak")?,
        memory_limit: match fs::read_to_string(root.join("memory.max"))
            .map_err(|e| format!("memory.max: {e}"))?
            .trim()
        {
            "max" => None,
            value => Some(value.parse().map_err(|_| "invalid memory.max")?),
        },
        memory_high_events: read_kv(&memory_events, "high")?,
        memory_max_events: read_kv(&memory_events, "max")?,
        memory_oom_events: read_kv(&memory_events, "oom")?,
        memory_oom_kill_events: read_kv(&memory_events, "oom_kill")?,
        io_read_bytes: io_read,
        io_write_bytes: io_write,
    })
}

fn write_lease(output: &Path, unit: &str, cap: usize) -> Result<String, String> {
    let canonical = fs::canonicalize(output).map_err(|e| format!("output directory: {e}"))?;
    let result = Command::new("abyss-machine")
        .args(["storage", "write-reservation", "list", "--json"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .map_err(|e| format!("storage reservation list: {e}"))?;
    if !result.status.success() || result.stdout.len() > 1_048_576 {
        return Err("canonical storage reservation list failed or exceeded 1 MiB".into());
    }
    let value: Value =
        serde_json::from_slice(&result.stdout).map_err(|_| "invalid storage reservation JSON")?;
    let records = value
        .get("records")
        .and_then(Value::as_array)
        .ok_or("storage reservation records absent")?;
    let matching = records
        .iter()
        .filter(|r| {
            r["active"] == true
                && r["hold_until_terminal"] == true
                && r["requested_bytes"].as_u64().unwrap_or(0) >= cap as u64
                && r["target"].as_str().is_some_and(|p| {
                    fs::canonicalize(p).ok().as_deref() == Some(canonical.as_path())
                })
                && r["execution_identity"].as_str().is_some_and(|v| {
                    v.starts_with(&format!("resource-launch:{unit}:")) && v.ends_with(":execution")
                })
        })
        .collect::<Vec<_>>();
    if value["ok"] != true
        || value["state_errors"]
            .as_array()
            .is_some_and(|v| !v.is_empty())
        || matching.len() != 1
    {
        return Err(
            "fresh unique terminal output reservation bound to this unit is required".into(),
        );
    }
    matching[0]["execution_identity"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| "output lease execution identity absent".into())
}
const VOLATILE_JSON_PATHS: &[&str] = &[
    "/result/revision",
    "/result/owner_configuration",
    "/result/rights/head_seq",
    "/result/rights/rights_version",
    "/result/selected_generation_sha256",
    "/result/selected_state_sha256",
    "/result/durable_commit/prepare_id_sha256",
    "/result/durable_commit/request_sha256",
    "/result/durable_commit/delta_sha256",
    "/result/durable_commit/member_root_sha256",
    "/result/durable_commit/commit_seq",
    "/result/receipt/owner_configuration",
    "/result/receipt/recorded_at",
    "/result/receipt/command_id",
    "/result/receipt/request_digest",
    "/result/receipt/request_sha256",
    "/result/receipt/commit_seq",
    "/result/receipt/rights_version",
    "/result/current_rights/head_seq",
    "/result/current_rights/rights_version",
    "/result/receipt/files/source-create-provenance.jsonl/sha256",
];

// These fields distinguish an original successful commit from its exact
// replay. They are ignored only for the replay response comparison, which
// separately requires both flags to be true.
const IDEMPOTENCY_REPLAY_JSON_PATHS: &[&str] =
    &["/result/replayed", "/result/durable_commit/replayed"];

fn outcomes(op: &Operation) -> Vec<ExpectedOutcome> {
    let mut values = vec![ExpectedOutcome {
        status: op.expected_status,
        sha256: op.expected_sha256.clone(),
        body: op.expected_body.clone(),
        ignored_json_paths: op.ignored_json_paths.clone(),
    }];
    values.extend(op.alternate_outcomes.clone());
    values.sort_by_key(|value| value.status);
    values
}

fn mcp_source_text_search_operation(op: &Operation) -> bool {
    op.channel == "mcp"
        && op.request["method"] == "tools/call"
        && op.request["params"]["name"] == "tos_source_text_search"
}

fn valid_mcp_source_text_search_request(op: &Operation) -> bool {
    let Some(arguments) = op.request.pointer("/params/arguments") else {
        return false;
    };
    let Some(fields) = arguments.as_object() else {
        return false;
    };
    let allowed = ["query", "limit", "document_types", "source_paths", "cursor"];
    if fields.keys().any(|key| !allowed.contains(&key.as_str()))
        || arguments["query"]
            .as_str()
            .is_none_or(|query| query.is_empty() || query.len() > 4096)
        || arguments["limit"]
            .as_u64()
            .is_none_or(|limit| !(1..=100).contains(&limit))
    {
        return false;
    }
    for key in ["document_types", "source_paths"] {
        if arguments.get(key).is_some_and(|value| {
            value.as_array().is_none_or(|items| {
                items.len() > 64
                    || items.iter().any(|item| {
                        item.as_str()
                            .is_none_or(|text| text.is_empty() || text.len() > 1024)
                    })
            })
        }) {
            return false;
        }
    }
    arguments.get("cursor").is_none_or(|value| {
        value
            .as_str()
            .is_some_and(|cursor| !cursor.is_empty() && cursor.len() <= 4096)
    })
}

fn mcp_source_text_search_response_succeeded(op: &Operation, attempt: &Attempt) -> bool {
    if attempt.status != 200 {
        return false;
    }
    let Some(raw) = attempt.body.as_deref() else {
        return false;
    };
    let Ok(response) = serde_json::from_slice::<Value>(raw) else {
        return false;
    };
    let Some(result) = response.get("result").and_then(Value::as_object) else {
        return false;
    };
    response["jsonrpc"] == "2.0"
        && response.get("id") == op.request.get("id")
        && response.get("error").is_none()
        && result.get("content").is_some_and(Value::is_array)
        && result
            .get("isError")
            .is_none_or(|value| value.as_bool() == Some(false))
}

fn interrupted_projection_apply(op: &Operation) -> bool {
    op.channel == "projection"
        && op.request["operation"] == "apply"
        && op.interrupt_resume_after_ms.is_some()
}

fn object_has_exact_keys(value: &Value, keys: &[&str]) -> bool {
    value.as_object().is_some_and(|object| {
        object.len() == keys.len() && keys.iter().all(|key| object.contains_key(*key))
    })
}

fn interrupted_projection_apply_response_matches(
    op: &Operation,
    attempt: &Attempt,
    cursor_observation: &Value,
) -> bool {
    if attempt.status != 0 || attempt.interrupted {
        return false;
    }
    let Some(max_events) = op.request["max_events"].as_u64() else {
        return false;
    };
    let Some(raw) = attempt.body.as_deref() else {
        return false;
    };
    let Ok(response) = serde_json::from_slice::<Value>(raw) else {
        return false;
    };
    if !object_has_exact_keys(&response, &["schema_version", "profile", "result"])
        || response["schema_version"] != "tos_source_projection_progress_v1"
        || response["profile"] != "tos_source_selected_text_v2"
        || response.get("error").is_some()
    {
        return false;
    }
    let Some(result) = response.get("result") else {
        return false;
    };
    if !object_has_exact_keys(
        result,
        &[
            "applied_events",
            "through_seq",
            "head_seq",
            "status",
            "work",
        ],
    ) {
        return false;
    }
    let Some(applied_events) = result["applied_events"].as_u64() else {
        return false;
    };
    let Some(through_seq) = result["through_seq"].as_u64() else {
        return false;
    };
    let Some(head_seq) = result["head_seq"].as_u64() else {
        return false;
    };
    let Some(current) = result["status"].get("Current") else {
        return false;
    };
    let Some(current_through_seq) = current["through_seq"].as_u64() else {
        return false;
    };
    let Some(before_interrupt) = cursor_observation.get("before_interrupt") else {
        return false;
    };
    let Some(before_through_seq) = before_interrupt["through_seq"].as_u64() else {
        return false;
    };
    let Some(before_head_seq) = before_interrupt["head_seq"].as_u64() else {
        return false;
    };
    let Some(after_interrupt) = cursor_observation.get("after_interrupt") else {
        return false;
    };
    let Some(partial_through_seq) = after_interrupt["through_seq"].as_u64() else {
        return false;
    };
    let Some(partial_head_seq) = after_interrupt["head_seq"].as_u64() else {
        return false;
    };
    let Some(final_cursor) = cursor_observation.get("after_resume") else {
        return false;
    };
    if applied_events == 0
        || applied_events > max_events
        || before_interrupt["status"] != "Pending"
        || before_head_seq.saturating_sub(before_through_seq) < 2
        || cursor_observation["partial_cursor_persisted"] != true
        || after_interrupt["status"] != "Pending"
        || partial_through_seq <= before_through_seq
        || partial_through_seq >= partial_head_seq
        || partial_head_seq < before_head_seq
        || through_seq != head_seq
        || current_through_seq != through_seq
        || through_seq.checked_sub(partial_through_seq) != Some(applied_events)
        || final_cursor["status"] != "Current"
        || final_cursor["through_seq"].as_u64() != Some(through_seq)
        || final_cursor["head_seq"].as_u64() != Some(head_seq)
        || !object_has_exact_keys(&result["status"], &["Current"])
        || !object_has_exact_keys(current, &["through_seq"])
    {
        return false;
    }
    let work_keys = [
        "examined_event_members",
        "exact_source_members_read",
        "exact_source_bytes_read",
        "custody_metadata_bytes_read",
        "segment_bytes_read",
        "committed_member_rows_examined",
        "examined_related_roots",
        "examined_dependency_paths",
        "extracted_documents",
        "published_source_paths",
        "published_documents",
    ];
    object_has_exact_keys(&result["work"], &work_keys)
        && work_keys
            .iter()
            .all(|key| result["work"][*key].as_u64().is_some())
}

fn exact_source_projection_request(request: &Value) -> bool {
    request.as_object().is_some_and(|fields| {
        fields.len() == 3
            && fields.contains_key("operation")
            && fields.contains_key("source_path")
            && fields.contains_key("revision")
            && request["operation"] == "exact-source"
            && request["source_path"]
                .as_str()
                .is_some_and(|path| tos_foundation::RelativePath::parse(path).is_ok())
            && request["revision"]
                .as_u64()
                .is_some_and(|revision| revision > 0)
    })
}

fn native_pg_source_projection_read(op: &Operation) -> bool {
    op.channel == "sdk"
        && op.sdk_argv.len() == 4
        && Path::new(&op.sdk_argv[0]).is_absolute()
        && op.sdk_argv[1] == "source-projection"
        && op.sdk_argv[2] == "--selection"
        && Path::new(&op.sdk_argv[3]).is_absolute()
        && op.projection_argv.is_empty()
        && exact_source_projection_request(&op.request)
}

fn selected_native_pg_source_projection_read(op: &Operation, args: &Args) -> bool {
    let Some(owner_binary) = args.owner_binary.as_deref().and_then(Path::to_str) else {
        return false;
    };
    native_pg_source_projection_read(op) && op.sdk_argv[0] == owner_binary
}

fn source_read_operation(op: &Operation) -> bool {
    match op.channel.as_str() {
        "mcp" => {
            op.request["method"] == "tools/call"
                && matches!(
                    op.request["params"]["name"].as_str(),
                    Some("tos_source_read_capabilities")
                        | Some("tos_source_handle_discover")
                        | Some("tos_source_read")
                        | Some("tos_source_text_search")
                )
        }
        "sdk" => {
            op.sdk_argv
                .windows(2)
                .any(|pair| pair[0] == "source" && matches!(pair[1].as_str(), "discover" | "read"))
                || native_pg_source_projection_read(op)
        }
        "projection" => matches!(
            op.request["operation"].as_str(),
            Some("search" | "search-text" | "exact-source")
        ),
        _ => false,
    }
}

fn native_query_source_read(op: &Operation) -> bool {
    op.channel == "sdk"
        && op
            .sdk_argv
            .windows(2)
            .any(|pair| pair[0] == "source" && pair[1] == "read")
        && !op.sdk_argv.iter().any(|arg| arg == "source-commands")
}

fn selected_query_source_read(op: &Operation, args: &Args) -> bool {
    let (Some(source_root), Some(source_inputs)) =
        (args.source_root.as_ref(), args.source_inputs.as_ref())
    else {
        return false;
    };
    let Some(query_binary) = args.query_binary.to_str() else {
        return false;
    };
    let Some(source_root) = source_root.to_str() else {
        return false;
    };
    let Some(source_inputs) = source_inputs.to_str() else {
        return false;
    };
    let Some(model) = args.model.as_deref().and_then(Path::to_str) else {
        return false;
    };
    let Some(binding) = args.binding.as_deref().and_then(Path::to_str) else {
        return false;
    };
    let expected = [
        query_binary,
        "--root",
        source_root,
        "--prepared-read-model",
        model,
        "--prepared-binding",
        binding,
        "--source-inputs",
        source_inputs,
        "source",
        "read",
        "-",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    op.channel == "sdk" && op.sdk_argv == expected
}

fn exact_source_read_request(request: &Value) -> bool {
    request.as_object().is_some_and(|fields| {
        fields.len() == 2
            && fields.contains_key("handle")
            && fields.contains_key("representation")
            && request["handle"]["schema_version"] == "tos_source_read_handle_v1"
            && matches!(
                request["representation"].as_str(),
                Some("record" | "native_public_unit" | "native_local_unit")
            )
    })
}

fn native_mutation_operation(op: &Operation) -> bool {
    op.channel == "sdk"
        && matches!(
            operation_name(op),
            Some("source.create" | "record.revise" | "rights.revoke")
        )
}

fn read_only_describe_operation(op: &Operation) -> bool {
    op.channel == "sdk"
        && op.request["schema_version"] == "tos_local_source_command_v1"
        && op.request["operation"] == "describe"
        && op.request.as_object().is_some_and(|fields| {
            (fields.len() == 2
                && fields.contains_key("schema_version")
                && fields.contains_key("operation"))
                || (fields.len() == 3
                    && fields.contains_key("schema_version")
                    && fields.contains_key("operation")
                    && fields.get("expected_configuration").is_some_and(|value| {
                        value.as_str().is_some_and(|digest| {
                            digest.strip_prefix("sha256:").is_some_and(valid_sha)
                        })
                    }))
        })
}

fn postload_rights_check(op: &Operation) -> bool {
    read_only_describe_operation(op)
        && op.expected_status == 0
        && op.expected_sha256.is_none()
        && op.expected_body.is_none()
        && op.expected_stderr_prefix.is_none()
        && op.ignored_json_paths.is_empty()
        && op.bind_expected_configuration_from_owner
        && op.expected_rights_allowed.is_some()
}

fn postload_rights_revoke_operation(op: &Operation) -> bool {
    let exact_request = op.request.as_object().is_some_and(|fields| {
        fields.len() == 6
            && fields.contains_key("schema_version")
            && fields.contains_key("operation")
            && fields.contains_key("command_id")
            && fields.contains_key("expected_configuration")
            && fields.contains_key("expected_rights_version")
            && fields.contains_key("reason")
    });
    rights_revoke_operation(op)
        && op.idempotency_replay_of.is_none()
        && exact_request
        && op.request["schema_version"] == "tos_local_source_command_v1"
        && op.request["command_id"]
            .as_str()
            .is_some_and(|id| !id.is_empty() && id.len() <= 128 && !id.contains('\0'))
        && op.request["reason"].as_str().is_some_and(|reason| {
            !reason.trim().is_empty() && reason.len() <= 512 && !reason.contains('\0')
        })
        && (op.request["expected_configuration"].is_null()
            || op.request["expected_configuration"]
                .as_str()
                .is_some_and(|digest| digest.strip_prefix("sha256:").is_some_and(valid_sha)))
        && (op.request["expected_rights_version"].is_null()
            || op.request["expected_rights_version"].as_u64().is_some())
        && op.expected_status == 0
        && op.expected_sha256.is_none()
        && op.expected_body.is_none()
        && op.ignored_json_paths.is_empty()
        && op.expected_stderr_prefix.is_none()
        && op.bind_expected_from.is_some()
}

fn postload_projection_rights_denial(op: &Operation) -> bool {
    op.channel == "projection"
        && matches!(
            op.request["operation"].as_str(),
            Some("search" | "search-text" | "exact-source")
        )
        && op.expected_status == 2
        && op.expected_sha256.is_none()
        && op.expected_body.is_none()
        && op.ignored_json_paths.is_empty()
        && op.expected_stderr_prefix.as_deref()
            == Some("source-projection refused: current rights revoked")
}

fn postload_read_only_operation(op: &Operation) -> bool {
    postload_exact_source_integrity(op)
        || postload_rights_check(op)
        || postload_projection_rights_denial(op)
        || (op.channel == "projection"
            && matches!(
                op.request["operation"].as_str(),
                Some("status" | "search" | "search-text" | "exact-source")
            ))
}

fn rights_allowed_value(body: &Value) -> Option<bool> {
    body.pointer("/rights/rights_allowed")
        .or_else(|| body.pointer("/result/rights/rights_allowed"))
        .and_then(Value::as_bool)
}

fn postload_rights_revoke_request_digest(request: &Value) -> Option<String> {
    let raw = json_bytes(request).ok()?;
    let canonical = tos_foundation::canonical_raw_bytes_v1(
        &raw,
        tos_foundation::CanonicalProfile::SourceCommandInputV1,
        tos_foundation::JsonLimits {
            max_bytes: 8_388_608,
            ..tos_foundation::JsonLimits::default()
        },
    )
    .ok()?;
    Some(format!("sha256:{}", sha(&canonical)))
}

fn postload_rights_revoke_response_matches(op: &Operation, attempt: &Attempt) -> bool {
    if attempt.status != 0 {
        return false;
    }
    let Some(raw) = attempt.body.as_deref() else {
        return false;
    };
    let Ok(response) = serde_json::from_slice::<Value>(raw) else {
        return false;
    };
    let Some(expected_version) = op.request["expected_rights_version"].as_u64() else {
        return false;
    };
    let Some(commit_seq) = response
        .pointer("/receipt/commit_seq")
        .and_then(Value::as_u64)
    else {
        return false;
    };
    let Some(receipt_version) = response
        .pointer("/receipt/rights_version")
        .and_then(Value::as_u64)
    else {
        return false;
    };
    let Some(expected_request_digest) = postload_rights_revoke_request_digest(&op.request) else {
        return false;
    };
    response["schema_version"] == "tos_local_pg_rights_revoke_result_v1"
        && response["operation"] == "rights.revoke"
        && response["owner_configuration"] == op.request["expected_configuration"]
        && response["receipt"]["command_id"] == op.request["command_id"]
        && response["receipt"]["replayed"] == false
        && response["receipt"]["request_sha256"].as_str() == Some(expected_request_digest.as_str())
        && commit_seq > 0
        && receipt_version == expected_version.saturating_add(1)
        && response
            .pointer("/current_rights/head_seq")
            .and_then(Value::as_u64)
            == Some(commit_seq)
        && response
            .pointer("/current_rights/rights_version")
            .and_then(Value::as_u64)
            == Some(receipt_version)
        && response.pointer("/current_rights/rights_allowed") == Some(&Value::Bool(false))
        && response["scope"] == "private CMD domain local current-use rights"
        && response["grants_admission"] == false
}

fn postload_rights_describe_response_matches(op: &Operation, attempt: &Attempt) -> bool {
    if attempt.status != 0 {
        return false;
    }
    let Some(raw) = attempt.body.as_deref() else {
        return false;
    };
    let Ok(response) = serde_json::from_slice::<Value>(raw) else {
        return false;
    };
    response["schema_version"] == "tos_local_pg_rights_revoke_result_v1"
        && response["operation"] == "describe"
        && response["owner_configuration"] == op.request["expected_configuration"]
        && response["owner_configuration"]
            .as_str()
            .is_some_and(|digest| digest.strip_prefix("sha256:").is_some_and(valid_sha))
        && response["principal_id"]
            .as_str()
            .is_some_and(|principal| !principal.is_empty() && principal.len() <= 256)
        && response
            .pointer("/rights/head_seq")
            .and_then(Value::as_u64)
            .is_some()
        && response
            .pointer("/rights/rights_version")
            .and_then(Value::as_u64)
            .is_some()
        && rights_allowed_value(&response) == op.expected_rights_allowed
        && response["scope"] == "private CMD domain local current-use rights"
        && response["grants_admission"] == false
}

fn postload_exact_source_integrity(op: &Operation) -> bool {
    match op.channel.as_str() {
        "mcp" => {
            op.request["method"] == "tools/call"
                && op.request["params"]["name"] == "tos_source_read"
        }
        "sdk" => {
            op.sdk_argv
                .windows(2)
                .any(|pair| pair[0] == "source" && pair[1] == "read")
                || native_pg_source_projection_read(op)
        }
        "projection" => op.request["operation"] == "exact-source",
        _ => false,
    }
}

fn postload_selected_source_integrity(op: &Operation) -> bool {
    op.channel == "sdk"
        && (op
            .sdk_argv
            .windows(2)
            .any(|pair| pair[0] == "source" && pair[1] == "read")
            || native_pg_source_projection_read(op))
}

fn postload_selected_vector_integrity(op: &Operation) -> bool {
    op.channel == "sdk"
        && op
            .sdk_argv
            .windows(2)
            .any(|pair| pair[0] == "source" && pair[1] == "read")
}

fn postload_sdk_pg_source_integrity(op: &Operation) -> bool {
    native_pg_source_projection_read(op) && op.expected_status == 0
}

fn postload_pg_source_integrity(op: &Operation) -> bool {
    op.channel == "projection"
        && op.request["operation"] == "exact-source"
        && op.expected_status == 0
}

fn exact_source_projection_operation(op: &Operation) -> bool {
    postload_sdk_pg_source_integrity(op) || postload_pg_source_integrity(op)
}

fn postload_check_kind(op: &Operation) -> &'static str {
    if op.idempotency_replay_of.is_some() {
        "idempotency_replay"
    } else if postload_rights_revoke_operation(op) {
        "rights_revocation"
    } else if postload_projection_rights_denial(op) {
        "rights_denial"
    } else if postload_rights_check(op) {
        "rights_state"
    } else if postload_sdk_pg_source_integrity(op) {
        "sdk_postgresql_source_integrity"
    } else if postload_pg_source_integrity(op) {
        "postgresql_source_integrity"
    } else if postload_exact_source_integrity(op) {
        "source_integrity"
    } else {
        "projection_state"
    }
}

fn digest_value(value: &Value) -> bool {
    value
        .as_str()
        .and_then(|text| text.strip_prefix("sha256:"))
        .is_some_and(valid_sha)
}

fn valid_utc_timestamp(value: &Value) -> bool {
    let Some(text) = value.as_str() else {
        return false;
    };
    let bytes = text.as_bytes();
    if bytes.len() < 20
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes[16] != b':'
        || !bytes[..4].iter().all(u8::is_ascii_digit)
        || !bytes[5..7].iter().all(u8::is_ascii_digit)
        || !bytes[8..10].iter().all(u8::is_ascii_digit)
        || !bytes[11..13].iter().all(u8::is_ascii_digit)
        || !bytes[14..16].iter().all(u8::is_ascii_digit)
        || !bytes[17..19].iter().all(u8::is_ascii_digit)
    {
        return false;
    }
    let month = text[5..7].parse::<u8>().ok();
    let day = text[8..10].parse::<u8>().ok();
    let hour = text[11..13].parse::<u8>().ok();
    let minute = text[14..16].parse::<u8>().ok();
    let second = text[17..19].parse::<u8>().ok();
    if !matches!(month, Some(1..=12))
        || !matches!(day, Some(1..=31))
        || !matches!(hour, Some(0..=23))
        || !matches!(minute, Some(0..=59))
        || !matches!(second, Some(0..=60))
    {
        return false;
    }
    if bytes.len() == 20 {
        return bytes[19] == b'Z';
    }
    bytes[19] == b'.'
        && bytes.last() == Some(&b'Z')
        && (22..=30).contains(&bytes.len())
        && bytes[20..bytes.len() - 1].iter().all(u8::is_ascii_digit)
}

fn ignored_value_valid(path: &str, value: &Value) -> bool {
    match path {
        "/result/revision"
        | "/result/owner_configuration"
        | "/result/receipt/request_sha256"
        | "/result/selected_generation_sha256"
        | "/result/selected_state_sha256"
        | "/result/durable_commit/prepare_id_sha256"
        | "/result/durable_commit/request_sha256"
        | "/result/durable_commit/delta_sha256"
        | "/result/durable_commit/member_root_sha256"
        | "/result/receipt/owner_configuration"
        | "/result/receipt/request_digest"
        | "/result/receipt/files/source-create-provenance.jsonl/sha256" => digest_value(value),
        "/result/receipt/command_id" => value
            .as_str()
            .is_some_and(|text| !text.is_empty() && text.len() <= 256 && text.is_ascii()),
        "/result/durable_commit/commit_seq" => value.as_u64().is_some_and(|number| number > 0),
        "/result/rights/head_seq"
        | "/result/rights/rights_version"
        | "/result/receipt/commit_seq"
        | "/result/receipt/rights_version"
        | "/result/current_rights/head_seq"
        | "/result/current_rights/rights_version" => value.as_u64().is_some(),
        "/result/receipt/recorded_at" => valid_utc_timestamp(value),
        _ => false,
    }
}

fn pointer_child(path: &str, segment: &str) -> String {
    let segment = segment.replace('~', "~0").replace('/', "~1");
    format!("{path}/{segment}")
}

fn body_matches(
    expected: &Value,
    actual: &Value,
    ignored_paths: &BTreeSet<String>,
    path: &str,
) -> bool {
    if ignored_paths.contains(path) {
        return expected.is_string() == actual.is_string()
            && expected.is_number() == actual.is_number()
            && expected.is_boolean() == actual.is_boolean()
            && expected.is_null() == actual.is_null()
            && ignored_value_valid(path, actual);
    }
    match (expected, actual) {
        (Value::Object(left), Value::Object(right)) => {
            left.len() == right.len()
                && left.keys().all(|key| {
                    right.get(key).is_some_and(|value| {
                        body_matches(&left[key], value, ignored_paths, &pointer_child(path, key))
                    })
                })
        }
        (Value::Array(left), Value::Array(right)) => {
            left.len() == right.len()
                && left.iter().zip(right).enumerate().all(|(index, (a, b))| {
                    body_matches(
                        a,
                        b,
                        ignored_paths,
                        &pointer_child(path, &index.to_string()),
                    )
                })
        }
        _ => expected == actual,
    }
}

const EXACT_SOURCE_PROJECTION_BODY_KEYS: &[&str] = &[
    "schema_version",
    "domain",
    "source_path",
    "revision",
    "bytes",
    "sha256",
    "source_read_work",
    "content_base64",
];
const EXACT_SOURCE_READ_WORK_KEYS: &[&str] = &[
    "custody_metadata_bytes_read",
    "segment_bytes_read",
    "committed_member_rows_examined",
];
const RECORD_REVISE_SEMANTIC_ORACLE_KEYS: &[&str] = &[
    "schema_version",
    "result_schema_version",
    "domain",
    "source_path",
    "command_id",
    "previous_revision",
];
const PG_REVISION_SOURCE_READ_WORK_KEYS: &[&str] = &[
    "scope",
    "exact_source_members_read",
    "exact_source_bytes_read",
    "historical_source_members_read",
    "historical_source_bytes_read",
    "custody_metadata_bytes_read",
    "segment_bytes_read",
    "committed_member_rows_examined",
    "metadata_member_lookups",
    "directory_index_rows_examined",
    "unique_index_lookups",
    "text_identity_lookups",
];

fn exact_source_read_work_matches(expected: &Value, actual: &Value) -> bool {
    object_has_exact_keys(expected, EXACT_SOURCE_READ_WORK_KEYS)
        && object_has_exact_keys(actual, EXACT_SOURCE_READ_WORK_KEYS)
        && EXACT_SOURCE_READ_WORK_KEYS.iter().all(|key| {
            expected[*key].as_u64() == Some(0)
                && actual[*key]
                    .as_u64()
                    .is_some_and(|value| value <= MAX_POSTLOAD_SOURCE_READ_WORK_VALUE)
        })
}

fn exact_source_projection_template_valid(op: &Operation, expected: &Value) -> bool {
    let Some(encoded) = expected.get("content_base64").and_then(Value::as_str) else {
        return false;
    };
    let Some(bytes) = expected.get("bytes").and_then(Value::as_u64) else {
        return false;
    };
    let encoded_len = bytes
        .saturating_add(2)
        .checked_div(3)
        .and_then(|groups| groups.checked_mul(4));
    object_has_exact_keys(expected, EXACT_SOURCE_PROJECTION_BODY_KEYS)
        && expected["schema_version"] == "tos_source_projection_exact_v1"
        && expected["domain"]
            .as_str()
            .is_some_and(|domain| !domain.is_empty() && domain.len() <= 1_024)
        && expected["source_path"] == op.request["source_path"]
        && expected["revision"] == op.request["revision"]
        && op.request["revision"]
            .as_u64()
            .is_some_and(|revision| revision > 0)
        && (1..=MAX_POSTLOAD_EXACT_SOURCE_BYTES).contains(&bytes)
        && expected["sha256"].as_str().is_some_and(valid_sha)
        && encoded.is_ascii()
        && encoded_len == Some(encoded.len() as u64)
        && exact_source_read_work_matches(
            &expected["source_read_work"],
            &expected["source_read_work"],
        )
}

fn exact_source_projection_response_matches(
    op: &Operation,
    expected: &Value,
    actual: &Value,
) -> bool {
    if !exact_source_projection_operation(op)
        || !exact_source_projection_template_valid(op, expected)
        || !object_has_exact_keys(actual, EXACT_SOURCE_PROJECTION_BODY_KEYS)
        || actual["schema_version"] != "tos_source_projection_exact_v1"
        || !exact_source_read_work_matches(
            &expected["source_read_work"],
            &actual["source_read_work"],
        )
    {
        return false;
    }
    [
        "schema_version",
        "domain",
        "source_path",
        "revision",
        "bytes",
        "sha256",
        "content_base64",
    ]
    .iter()
    .all(|key| expected[*key] == actual[*key])
}

fn record_revise_semantic_oracle_present(value: &Value) -> bool {
    value.get("schema_version").and_then(Value::as_str)
        == Some(RECORD_REVISE_SEMANTIC_ORACLE_SCHEMA)
}

fn record_revise_owner_context(op: &Operation) -> Option<(Value, Value, String, String)> {
    let invocation = read_stage_json(Path::new(invocation_argument(op)?)).ok()?;
    if invocation["schema_version"] != "tos_local_pg_record_revision_invocation_v1" {
        return None;
    }
    let domain = invocation
        .pointer("/source_selection/domain")
        .and_then(Value::as_str)?
        .to_owned();
    let owner_path = PathBuf::from(invocation["owner_config"].as_str()?);
    let owner = read_stage_json(&owner_path).ok()?;
    let source_path = owner["source_path"].as_str()?.to_owned();
    tos_foundation::RelativePath::parse(&source_path).ok()?;
    Some((invocation, owner, domain, source_path))
}

fn record_revise_semantic_oracle_valid(op: &Operation, expected: &Value) -> bool {
    if op.channel != "sdk"
        || operation_name(op) != Some("record.revise")
        || op.expected_status != 0
        || op.expected_sha256.is_some()
        || op.expected_stderr_prefix.is_some()
        || !op.ignored_json_paths.is_empty()
        || op.retry_same_command_id
        || op.reconnect_before
        || op.bind_expected_configuration_from_owner
        || op.bind_expected_from.is_none()
        || !record_revise_semantic_oracle_present(expected)
        || !object_has_exact_keys(expected, RECORD_REVISE_SEMANTIC_ORACLE_KEYS)
        || expected["result_schema_version"] != RECORD_REVISE_RESULT_SCHEMA
    {
        return false;
    }
    let Some(command_id) = op.request.get("command_id").and_then(Value::as_str) else {
        return false;
    };
    if command_id.is_empty() || command_id.len() > 128 {
        return false;
    }
    let Some((_, _, domain, source_path)) = record_revise_owner_context(op) else {
        return false;
    };
    expected["domain"].as_str() == Some(domain.as_str())
        && expected["source_path"].as_str() == Some(source_path.as_str())
        && expected["command_id"].as_str() == Some(command_id)
        && expected["previous_revision"]
            .as_str()
            .and_then(|value| value.strip_prefix("sha256:"))
            .is_some_and(valid_sha)
}

fn record_revise_preview_response_valid(
    op: &Operation,
    expected: &Value,
    response: &Value,
) -> bool {
    if !record_revise_semantic_oracle_valid(op, expected)
        || !object_has_exact_keys(response, &["schema_version", "result", "grants_admission"])
        || response["schema_version"] != "tos_local_pg_record_revision_result_v1"
        || response["grants_admission"] != false
    {
        return false;
    }
    let result = &response["result"];
    result["schema_version"] == RECORD_REVISE_RESULT_SCHEMA
        && result["source_path"] == expected["source_path"]
        && result["revision"] == expected["previous_revision"]
        && result["owner_configuration"]
            .as_str()
            .is_some_and(|digest| digest.strip_prefix("sha256:").is_some_and(valid_sha))
        && result["source"].as_object().is_some()
        && result["receipt"] == Value::Null
        && result["replayed"] == false
        && result["grants_admission"] == false
        && result["expected_dependencies"]
            .as_str()
            .is_some_and(|digest| digest.strip_prefix("sha256:").is_some_and(valid_sha))
        && result["prepared_source"].as_object().is_some()
        && result["prepared_forms"].as_array().is_some()
        && result["prepared_materializations"].as_array().is_some()
        && result
            .get("command_operations")
            .and_then(Value::as_array)
            .is_some_and(|operations| {
                operations
                    .iter()
                    .any(|operation| operation.as_str() == Some("prepare-revise"))
            })
}

fn pg_revision_source_read_work_valid(value: &Value) -> bool {
    object_has_exact_keys(value, PG_REVISION_SOURCE_READ_WORK_KEYS)
        && value["scope"]
            == "bounded PG/STO source selection; software capture and physical I/O excluded"
        && PG_REVISION_SOURCE_READ_WORK_KEYS
            .iter()
            .filter(|key| **key != "scope")
            .all(|key| {
                value[*key]
                    .as_u64()
                    .is_some_and(|count| count <= MAX_POSTLOAD_SOURCE_READ_WORK_VALUE)
            })
}

fn record_revise_result_keyset_matches(preview: &Value, actual: &Value) -> bool {
    const PREVIEW_ONLY: &[&str] = &[
        "prepared_source",
        "prepared_forms",
        "prepared_materializations",
        "expected_dependencies",
        "expected_publication",
    ];
    let Some(preview) = preview.as_object() else {
        return false;
    };
    let Some(actual) = actual.as_object() else {
        return false;
    };
    let mut expected_keys = preview
        .keys()
        .filter(|key| !PREVIEW_ONLY.contains(&key.as_str()))
        .cloned()
        .collect::<BTreeSet<_>>();
    expected_keys.insert("durable_commit".to_owned());
    expected_keys.len() == actual.len() && expected_keys.iter().all(|key| actual.contains_key(key))
}

fn record_revise_semantic_response_matches(
    op: &Operation,
    expected: &Value,
    actual: &Value,
) -> bool {
    if op.idempotency_replay_of.is_some() || !record_revise_semantic_oracle_valid(op, expected) {
        return false;
    }
    let Some(preview) = op.bound_revision_preview.as_ref() else {
        return false;
    };
    if !object_has_exact_keys(preview, &["schema_version", "result", "grants_admission"])
        || preview["schema_version"] != "tos_local_pg_record_revision_result_v1"
        || preview["grants_admission"] != false
    {
        return false;
    }
    let preview_result = &preview["result"];
    let result = &actual["result"];
    let Some((_, owner, domain, source_path)) = record_revise_owner_context(op) else {
        return false;
    };
    if !object_has_exact_keys(actual, &["schema_version", "result", "grants_admission"])
        || actual["schema_version"] != "tos_local_pg_record_revision_result_v1"
        || actual["grants_admission"] != false
        || preview_result["schema_version"] != RECORD_REVISE_RESULT_SCHEMA
        || result["schema_version"] != RECORD_REVISE_RESULT_SCHEMA
        || !record_revise_result_keyset_matches(preview_result, result)
        || expected["domain"].as_str() != Some(domain.as_str())
        || expected["source_path"].as_str() != Some(source_path.as_str())
        || result["source_path"].as_str() != Some(source_path.as_str())
        || preview_result["source_path"].as_str() != Some(source_path.as_str())
        || result["owner_configuration"] != preview_result["owner_configuration"]
        || !digest_value(&result["owner_configuration"])
        || result["source"] != preview_result["prepared_source"]
        || result["materializations"] != preview_result["prepared_materializations"]
        || result["replayed"] != false
        || result["grants_admission"] != false
        || result["revision"].as_str().is_none_or(|revision| {
            !revision.strip_prefix("sha256:").is_some_and(valid_sha)
                || revision == expected["previous_revision"].as_str().unwrap_or_default()
        })
        || preview_result["revision"] != expected["previous_revision"]
        || op.request["expected_revision"] != expected["previous_revision"]
        || op.request["expected_source"] != preview_result["source"]
        || op.request["expected_configuration"] != preview_result["owner_configuration"]
        || op.request["expected_dependencies"] != preview_result["expected_dependencies"]
        || op.request["expected_publication"] != preview_result["expected_publication"]
        || preview_result["receipt"] != Value::Null
        || preview_result["source"].as_object().is_none()
        || preview_result["prepared_source"].as_object().is_none()
        || preview_result["prepared_forms"].as_array().is_none()
        || preview_result["prepared_materializations"]
            .as_array()
            .is_none()
        || !digest_value(&preview_result["expected_dependencies"])
        || [
            preview_result.get("publication_snapshot"),
            result.get("publication_snapshot"),
        ]
        .into_iter()
        .flatten()
        .any(|snapshot| {
            !snapshot.is_null()
                && snapshot
                    .as_str()
                    .is_none_or(|value| value.is_empty() || value.len() > 1_024)
        })
        || !pg_revision_source_read_work_valid(&result["source_read_work"])
    {
        return false;
    }

    // All owner/profile/capability fields retain their preview shape and value.
    // Only the revision's documented commit outputs may change.
    const COMMIT_FIELDS: &[&str] = &[
        "source",
        "revision",
        "receipt",
        "replayed",
        "materializations",
        "publication_snapshot",
        "source_read_work",
        "durable_commit",
        "source_profile",
        "source_record_profile",
    ];
    if preview_result.as_object().is_none_or(|fields| {
        fields.iter().any(|(key, value)| {
            ![
                "prepared_source",
                "prepared_forms",
                "prepared_materializations",
                "expected_dependencies",
                "expected_publication",
            ]
            .contains(&key.as_str())
                && !COMMIT_FIELDS.contains(&key.as_str())
                && result.get(key) != Some(value)
        })
    }) {
        return false;
    }
    for key in ["source_profile", "source_record_profile"] {
        if preview_result
            .get(key)
            .is_some_and(|value| !value.is_object() || !result[key].is_object())
        {
            return false;
        }
    }
    let Some(commit_seq) = result["durable_commit"]["commit_seq"].as_u64() else {
        return false;
    };
    let durable_commit = &result["durable_commit"];
    if !object_has_exact_keys(
        durable_commit,
        &[
            "domain",
            "command_id",
            "prepare_id_sha256",
            "commit_seq",
            "request_sha256",
            "delta_sha256",
            "member_root_sha256",
            "replayed",
        ],
    ) || durable_commit["domain"].as_str() != Some(domain.as_str())
        || durable_commit["command_id"] != op.request["command_id"]
        || commit_seq == 0
        || commit_seq > i64::MAX as u64
        || durable_commit["replayed"] != false
        || ![
            "prepare_id_sha256",
            "request_sha256",
            "delta_sha256",
            "member_root_sha256",
        ]
        .iter()
        .all(|key| digest_value(&durable_commit[*key]))
    {
        return false;
    }

    let receipt = &result["receipt"];
    let record_id = owner["record_id"].as_str().unwrap_or_default();
    let previous_revision = expected["previous_revision"].as_str().unwrap_or_default();
    let expected_archive_path = previous_revision.strip_prefix("sha256:").map(|revision| {
        format!(
            "ToS/source-witnesses/.record-revisions/{}-{revision}",
            sha(record_id.as_bytes())
        )
    });
    let Some(mut changed_fields) = op.request["fields"]
        .as_object()
        .map(|fields| fields.keys().cloned().collect::<Vec<_>>())
    else {
        return false;
    };
    changed_fields.sort();
    let expected_changed_fields =
        Value::Array(changed_fields.into_iter().map(Value::String).collect());
    if !object_has_exact_keys(
        receipt,
        &[
            "command_id",
            "request_digest",
            "principal_id",
            "authority_ref",
            "owner_configuration",
            "recorded_at",
            "reason",
            "previous_source",
            "source",
            "previous_revision",
            "archive_path",
            "dependencies",
            "changed_fields",
            "forms",
            "grants_admission",
            "request",
            "publication",
        ],
    ) || receipt["command_id"] != op.request["command_id"]
        || !digest_value(&receipt["request_digest"])
        || receipt["principal_id"] != owner["principal_id"]
        || receipt["authority_ref"] != owner["authority_ref"]
        || receipt["owner_configuration"] != result["owner_configuration"]
        || !valid_utc_timestamp(&receipt["recorded_at"])
        || receipt["reason"] != op.request["reason"]
        || receipt["previous_source"] != preview_result["source"]
        || receipt["source"] != result["source"]
        || receipt["previous_revision"] != expected["previous_revision"]
        || Some(
            receipt["archive_path"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
        ) != expected_archive_path
        || receipt["dependencies"] != op.request["expected_dependencies"]
        || receipt["changed_fields"] != expected_changed_fields
        || receipt["forms"] != preview_result["prepared_forms"]
        || receipt["grants_admission"] != false
        || receipt["request"] != op.request
        || !object_has_exact_keys(
            &receipt["publication"],
            &["protocol", "transaction_id", "selected_files"],
        )
        || receipt["publication"]["protocol"] != result["publication_protocol"]
        || receipt["publication"]["transaction_id"]
            .as_str()
            .is_none_or(|value| !value.strip_prefix("sha256:").is_some_and(valid_sha))
        || receipt["publication"]["selected_files"] != result["selected_files"]
    {
        return false;
    }
    true
}

fn record_revise_semantic_replay_response_matches(
    op: &Operation,
    expected: &Value,
    actual: &Value,
) -> bool {
    if op.idempotency_replay_of.is_none()
        || !record_revise_semantic_oracle_valid(op, expected)
        || !idempotency_replay_flags_are_true(op, actual)
    {
        return false;
    }
    let Some(original) = op.bound_replay_response.as_ref() else {
        return false;
    };
    if !object_has_exact_keys(original, &["schema_version", "result", "grants_admission"])
        || original["schema_version"] != "tos_local_pg_record_revision_result_v1"
        || original["grants_admission"] != false
        || original.pointer("/result/replayed") != Some(&Value::Bool(false))
        || original.pointer("/result/durable_commit/replayed") != Some(&Value::Bool(false))
        || !pg_revision_source_read_work_valid(&original["result"]["source_read_work"])
        || !pg_revision_source_read_work_valid(&actual["result"]["source_read_work"])
    {
        return false;
    }
    // Source-read counters are per invocation; replay traverses the owner
    // route again. Keep their exact typed shape and cap, while matching every
    // semantic response field and changing only the two replay flags.
    let mut expected_replay = original.clone();
    expected_replay["result"]["replayed"] = Value::Bool(true);
    expected_replay["result"]["durable_commit"]["replayed"] = Value::Bool(true);
    expected_replay["result"]["source_read_work"] = actual["result"]["source_read_work"].clone();
    expected_replay == *actual
}

// Form-set bytes contain the command receipt, including its runtime timestamp.
// Check that revision against the exact published stage instead of the separate
// preflight's receipt; the rest of the response still matches the semantic oracle.
fn published_form_revision_matches(args: &Args, op: &Operation, actual: &Value) -> bool {
    let check = || -> Option<()> {
        if op.channel != "owner_http" || !op.bind_expected_configuration_from_owner {
            return None;
        }
        args.owner_stage_baseline.as_ref()?;
        let owner = read_stage_json(args.owner_config.as_deref()?).ok()?;
        let root = Path::new(owner["source_root"].as_str()?);
        if !root.starts_with(args.output.join("owner-stage")) {
            return None;
        }
        let relative = actual.pointer("/result/target_path")?.as_str()?;
        tos_foundation::RelativePath::parse(relative).ok()?;
        if !relative.ends_with(".human-forms.json") {
            return None;
        }
        let file = tos_fd_open::open_absolute_regular(&root.join(relative), 2_097_152).ok()?;
        let before = file.metadata().ok()?;
        let mut raw = Vec::new();
        (&file).take(2_097_153).read_to_end(&mut raw).ok()?;
        let after = file.metadata().ok()?;
        if raw.len() as u64 != before.len()
            || before.len() != after.len()
            || before.mtime() != after.mtime()
            || before.mtime_nsec() != after.mtime_nsec()
            || before.ctime() != after.ctime()
            || before.ctime_nsec() != after.ctime_nsec()
            || actual.pointer("/result/revision")?.as_str()? != format!("sha256:{}", sha(&raw))
        {
            return None;
        }
        let published: Value = serde_json::from_slice(&raw).ok()?;
        if published["growth_history"].as_array()?.last()? != actual.pointer("/result/receipt")? {
            return None;
        }
        Some(())
    };
    check().is_some()
}

fn outcome_matches(
    args: &Args,
    op: &Operation,
    expected: &ExpectedOutcome,
    attempt: &Attempt,
    projection_cursor_observation: &Value,
) -> bool {
    if expected.status != attempt.status
        || (mcp_source_text_search_operation(op)
            && !mcp_source_text_search_response_succeeded(op, attempt))
        || (interrupted_projection_apply(op)
            && !interrupted_projection_apply_response_matches(
                op,
                attempt,
                projection_cursor_observation,
            ))
        || expected
            .sha256
            .as_ref()
            .is_some_and(|digest| digest != &attempt.sha256)
        || op
            .expected_stderr_prefix
            .as_ref()
            .is_some_and(|prefix| !attempt.stderr.starts_with(prefix.as_bytes()))
    {
        return false;
    }
    if postload_rights_revoke_operation(op) && !postload_rights_revoke_response_matches(op, attempt)
    {
        return false;
    }
    if postload_rights_check(op) && !postload_rights_describe_response_matches(op, attempt) {
        return false;
    }
    let body_matches_expected = expected.body.as_ref().is_none_or(|body| {
        let Some(raw) = attempt.body.as_deref() else {
            return false;
        };
        let Ok(actual) = serde_json::from_slice::<Value>(raw) else {
            return false;
        };
        let mut ignored_paths = expected
            .ignored_json_paths
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if op.idempotency_replay_of.is_some() {
            ignored_paths.extend(
                IDEMPOTENCY_REPLAY_JSON_PATHS
                    .iter()
                    .map(|path| (*path).to_owned()),
            );
        }
        let matches_body = if record_revise_semantic_oracle_present(body) {
            if op.idempotency_replay_of.is_some() {
                record_revise_semantic_replay_response_matches(op, body, &actual)
            } else {
                record_revise_semantic_response_matches(op, body, &actual)
            }
        } else if exact_source_projection_operation(op) {
            exact_source_projection_response_matches(op, body, &actual)
        } else {
            body_matches(body, &actual, &ignored_paths, "")
        };
        matches_body
            && (op.idempotency_replay_of.is_none()
                || idempotency_replay_flags_are_true(op, &actual))
            && (!ignored_paths.contains("/result/revision")
                || published_form_revision_matches(args, op, &actual))
            && (!ignored_paths.contains("/result/receipt/command_id")
                || actual.pointer("/result/receipt/command_id") == op.request.get("command_id"))
    });
    body_matches_expected
        && (expected.sha256.is_some()
            || expected.body.is_some()
            || interrupted_projection_apply(op)
            || op.expected_stderr_prefix.is_some()
            || (postload_rights_revoke_operation(op)
                && postload_rights_revoke_response_matches(op, attempt))
            || (postload_rights_check(op)
                && postload_rights_describe_response_matches(op, attempt)))
}

fn valid_expected(value: &ExpectedOutcome, sdk: bool, response_cap: usize) -> bool {
    if value
        .body
        .as_ref()
        .is_some_and(record_revise_semantic_oracle_present)
    {
        // This descriptor is accepted only by the bound native-PG record.revise
        // route below; it is never a generic static body or status-only oracle.
        return false;
    }
    let status_valid = if sdk {
        matches!(value.status, 0 | 2)
    } else {
        (100..=599).contains(&value.status)
    };
    let body_size_valid = value
        .body
        .as_ref()
        .is_none_or(|body| json_bytes(body).is_ok_and(|raw| raw.len() <= response_cap));
    status_valid
        && (value.sha256.as_deref().is_some_and(valid_sha) || value.body.is_some())
        && body_size_valid
        && value.ignored_json_paths.len() <= VOLATILE_JSON_PATHS.len()
        && value
            .ignored_json_paths
            .iter()
            .all(|path| VOLATILE_JSON_PATHS.contains(&path.as_str()))
        && value
            .ignored_json_paths
            .iter()
            .collect::<BTreeSet<_>>()
            .len()
            == value.ignored_json_paths.len()
        && (value.ignored_json_paths.is_empty() || value.body.is_some())
}

fn sdk_conflict_outcome(value: &ExpectedOutcome) -> bool {
    value.status == 2
        && value.body.as_ref().is_some_and(|body| {
            body["schema_version"] == "tos_local_source_command_error_v1"
                && body["error"] == "JournalConflict"
        })
}

fn sdk_conflict_attempt(attempt: &Attempt) -> bool {
    attempt.status == 2
        && attempt.body.as_deref().is_some_and(|raw| {
            serde_json::from_slice::<Value>(raw).is_ok_and(|body| {
                body["schema_version"] == "tos_local_source_command_error_v1"
                    && body["error"] == "JournalConflict"
            })
        })
}

fn operation_name(op: &Operation) -> Option<&str> {
    op.request.get("operation").and_then(Value::as_str)
}

fn invocation_argument(op: &Operation) -> Option<&str> {
    op.sdk_argv
        .windows(2)
        .find(|pair| pair[0] == "--invocation")
        .map(|pair| pair[1].as_str())
}

fn setup_preview_operation(op: &Operation) -> bool {
    op.channel == "sdk"
        && matches!(
            operation_name(op),
            Some("prepare-create" | "prepare-revise")
        )
}

fn expected_binding_operation(op: &Operation) -> bool {
    op.channel == "sdk" && matches!(operation_name(op), Some("source.create" | "record.revise"))
}

fn rights_revoke_operation(op: &Operation) -> bool {
    op.channel == "sdk" && operation_name(op) == Some("rights.revoke")
}

fn replayable_source_mutation_operation(op: &Operation) -> bool {
    op.channel == "sdk" && matches!(operation_name(op), Some("source.create" | "record.revise"))
}

fn validate_revision_schema_provision_grant(op: &Operation, operation: &str) -> Result<(), String> {
    if op.channel != "sdk"
        || op.request
            != json!({"schema_version":"tos_local_source_command_v1","operation":operation})
        || op.bind_expected_from.is_some()
    {
        return Err(
            "revision schema provisioning requires its exact protected owner operation".into(),
        );
    }
    let invocation_path = PathBuf::from(
        invocation_argument(op).ok_or("revision schema provisioning invocation absent")?,
    );
    let invocation = read_stage_json(&invocation_path)?;
    if invocation["schema_version"] != "tos_local_pg_record_revision_invocation_v1" {
        return Err("revision schema provisioning requires the ordinary PG revision owner".into());
    }
    let owner_path = PathBuf::from(
        invocation["owner_config"]
            .as_str()
            .ok_or("revision schema provisioning owner configuration absent")?,
    );
    let owner = read_stage_json(&owner_path)?;
    let allowed = owner["allowed_operations"]
        .as_array()
        .ok_or("revision schema provisioning owner grants absent")?;
    if !allowed
        .iter()
        .any(|value| value.as_str() == Some(operation))
        || (operation == "work.revision-schema.provision" && owner["record_type"] != "work")
    {
        return Err(
            "revision schema provisioning is not present in the selected protected owner grant"
                .into(),
        );
    }
    Ok(())
}

fn rights_describe_binding_operation(op: &Operation) -> bool {
    op.channel == "sdk"
        && operation_name(op) == Some("describe")
        && op.bind_expected_configuration_from_owner
}

fn bind_expected_fields(op: &mut Operation, response: &Value) -> Result<(), String> {
    let operation = operation_name(op)
        .ok_or("bound command operation absent")?
        .to_owned();
    if operation == "rights.revoke" {
        let result = response.get("result").unwrap_or(response);
        if result["schema_version"] != "tos_local_pg_rights_revoke_result_v1"
            || result["operation"] != "describe"
        {
            return Err(
                "rights binding result is not the native PostgreSQL rights describe".into(),
            );
        }
        let owner_configuration = result
            .get("owner_configuration")
            .and_then(Value::as_str)
            .filter(|value| value.strip_prefix("sha256:").is_some_and(valid_sha))
            .ok_or("rights describe lacks its owner configuration digest")?;
        let rights_version = result
            .pointer("/rights/rights_version")
            .and_then(Value::as_u64)
            .ok_or("rights describe lacks its current rights version")?;
        if result.pointer("/rights/rights_allowed") != Some(&Value::Bool(true)) {
            return Err("rights describe does not show current-use rights enabled".into());
        }
        op.request["expected_configuration"] = json!(owner_configuration);
        op.request["expected_rights_version"] = json!(rights_version);
        return Ok(());
    }
    let result = response
        .get("result")
        .ok_or("bound preview response result absent")?;
    let owner_configuration = result
        .get("owner_configuration")
        .and_then(Value::as_str)
        .filter(|value| value.strip_prefix("sha256:").is_some_and(valid_sha))
        .ok_or("bound preview lacks its owner configuration digest")?;
    let dependencies = result
        .get("expected_dependencies")
        .and_then(Value::as_str)
        .filter(|value| value.strip_prefix("sha256:").is_some_and(valid_sha))
        .ok_or("bound preview lacks its dependency digest")?;
    let (expected_source, expected_revision) = match operation.as_str() {
        "source.create" => (
            result
                .get("expected_source")
                .cloned()
                .ok_or("create preview lacks expected_source")?,
            result
                .get("expected_revision")
                .cloned()
                .ok_or("create preview lacks expected_revision")?,
        ),
        "record.revise" => (
            result
                .get("source")
                .cloned()
                .ok_or("revision preview lacks its current source identity")?,
            result
                .get("revision")
                .cloned()
                .ok_or("revision preview lacks its current revision digest")?,
        ),
        _ => {
            return Err(
                "expected-field binding only supports source.create or record.revise".into(),
            );
        }
    };
    if operation == "record.revise"
        && expected_revision
            .as_str()
            .is_none_or(|value| !value.strip_prefix("sha256:").is_some_and(valid_sha))
    {
        return Err("revision preview current revision is not a digest".into());
    }
    op.request["expected_configuration"] = json!(owner_configuration);
    op.request["expected_source"] = expected_source;
    op.request["expected_revision"] = expected_revision;
    op.request["expected_dependencies"] = json!(dependencies);
    if op.request.get("expected_publication").is_some()
        || result.get("expected_publication").is_some()
    {
        op.request["expected_publication"] = result
            .get("expected_publication")
            .cloned()
            .ok_or("selected revision preview lacks expected_publication")?;
    }
    if let Some(expected) = op
        .expected_body
        .as_ref()
        .filter(|body| record_revise_semantic_oracle_present(body))
    {
        if operation != "record.revise"
            || !record_revise_preview_response_valid(op, expected, response)
            || op.request["expected_revision"] != expected["previous_revision"]
            || op.request["expected_source"] != result["source"]
            || op.request["expected_configuration"] != result["owner_configuration"]
            || op.request["expected_dependencies"] != result["expected_dependencies"]
            || op.request["expected_publication"] != result["expected_publication"]
        {
            return Err(
                "semantic record.revise oracle does not bind to its exact serial owner preview"
                    .into(),
            );
        }
        op.bound_revision_preview = Some(response.clone());
        return Ok(());
    }
    let bound_configuration = op.request["expected_configuration"].clone();
    if let Some(expected_body) = op.expected_body.as_mut() {
        if bound_configuration
            .as_str()
            .is_none_or(|value| !value.strip_prefix("sha256:").is_some_and(valid_sha))
        {
            return Err("bound mutation oracle requires the selected configuration digest".into());
        }
        let receipt_request = expected_body
            .pointer_mut("/result/receipt/request")
            .ok_or("bound mutation oracle lacks its recorded request")?;
        if !receipt_request.is_object() || receipt_request.get("expected_configuration").is_none() {
            return Err(
                "bound mutation oracle lacks its recorded configuration precondition".into(),
            );
        }
        receipt_request["expected_configuration"] = bound_configuration;
    }
    Ok(())
}

fn bind_operation_from_setup(
    op: &mut Operation,
    setup_results: &BTreeMap<String, Value>,
) -> Result<(), String> {
    let Some(label) = op.bind_expected_from.as_deref() else {
        return Ok(());
    };
    let receipt = setup_results
        .get(label)
        .ok_or_else(|| format!("setup result {label} is unavailable for expected-field binding"))?;
    let preview_request = receipt
        .get("request")
        .ok_or("setup preview request absent")?;
    let operation = operation_name(op).ok_or("bound command operation absent")?;
    let proposal_fields: &[&str] = match operation {
        "source.create" => &["record", "forms"],
        "record.revise" => &["fields", "forms", "reason"],
        "rights.revoke" => &[],
        _ => return Err(
            "expected-field binding only supports source.create, record.revise or rights.revoke"
                .into(),
        ),
    };
    if proposal_fields
        .iter()
        .any(|field| preview_request.get(*field) != op.request.get(*field))
    {
        return Err("commit proposal differs from the exact serial owner preview request".into());
    }
    let response = receipt
        .get("response")
        .ok_or("setup preview response absent")?;
    if op
        .expected_body
        .as_ref()
        .is_some_and(record_revise_semantic_oracle_present)
    {
        let setup_sdk_argv = receipt
            .get("sdk_argv")
            .and_then(Value::as_array)
            .ok_or("semantic revision preview argv absent")?;
        let setup_sdk_argv_matches = setup_sdk_argv.len() == op.sdk_argv.len()
            && setup_sdk_argv
                .iter()
                .zip(&op.sdk_argv)
                .all(|(setup_argument, command_argument)| {
                    setup_argument.as_str() == Some(command_argument.as_str())
                });
        if preview_request["schema_version"] != "tos_local_source_command_v1"
            || preview_request["operation"] != "prepare-revise"
            || !setup_sdk_argv_matches
            || invocation_argument(op).is_none()
        {
            return Err(
                "semantic record.revise must use the same native invocation as prepare-revise"
                    .into(),
            );
        }
    }
    bind_expected_fields(op, response)
}

fn bind_postload_rights_revoke(
    op: &mut Operation,
    postload_results: &BTreeMap<String, Value>,
) -> Result<(), String> {
    let label = op
        .bind_expected_from
        .as_deref()
        .ok_or("postload rights.revoke describe binding absent")?;
    let receipt = postload_results
        .get(label)
        .ok_or_else(|| format!("postload rights describe {label} is unavailable"))?;
    let request = receipt
        .get("request")
        .ok_or("postload rights describe request absent")?;
    let response = receipt
        .get("response")
        .ok_or("postload rights describe response absent")?;
    if request["schema_version"] != "tos_local_source_command_v1"
        || request["operation"] != "describe"
        || request["expected_configuration"] != response["owner_configuration"]
        || receipt["sdk_argv"] != json!(op.sdk_argv)
        || response["schema_version"] != "tos_local_pg_rights_revoke_result_v1"
        || response["operation"] != "describe"
        || response.pointer("/rights/rights_allowed") != Some(&Value::Bool(true))
    {
        return Err("postload rights.revoke is not bound to the matching successful rights-enabled describe".into());
    }
    bind_expected_fields(op, response)
}

fn validate_fresh_pg_stage(plan: &Plan, args: &Args) -> Result<bool, String> {
    let has_sdk_mutation = plan
        .setup
        .iter()
        .chain(
            plan.sessions
                .iter()
                .take(args.sessions)
                .flat_map(|session| &session.operations),
        )
        .chain(plan.postload.iter())
        .any(|op| {
            op.channel == "sdk"
                && matches!(
                    operation_name(op),
                    Some("source.create" | "record.revise" | "rights.revoke")
                )
        });
    if !has_sdk_mutation && plan.setup.is_empty() {
        return Ok(false);
    }
    let baseline = args
        .owner_stage_baseline
        .as_ref()
        .ok_or("native PG setup requires a private --owner-stage-baseline")?
        .canonicalize()
        .map_err(|e| format!("native PG stage baseline: {e}"))?;
    if std::env::var_os("TOS_SOURCE_PG_URL").is_none_or(|value| value.is_empty()) {
        return Err(
            "native PG setup requires TOS_SOURCE_PG_URL; its value is never reported".into(),
        );
    }
    let bootstrap = plan
        .setup
        .first()
        .ok_or("native PG setup requires source.bootstrap as its first serial setup operation")?;
    if operation_name(bootstrap) != Some("source.bootstrap")
        || bootstrap.channel != "sdk"
        || bootstrap.request
            != json!({"schema_version":"tos_local_source_command_v1","operation":"source.bootstrap"})
        || bootstrap.retry_same_command_id
        || bootstrap.conflict_group.is_some()
    {
        return Err(
            "fresh native PG stage must begin with the ordinary source.bootstrap entry".into(),
        );
    }
    let invocation_path =
        PathBuf::from(invocation_argument(bootstrap).ok_or("source.bootstrap invocation absent")?)
            .canonicalize()
            .map_err(|e| format!("source.bootstrap invocation path: {e}"))?;
    if !invocation_path.starts_with(&baseline) {
        return Err("source.bootstrap invocation must be inside the private stage baseline".into());
    }
    let invocation = read_stage_json(&invocation_path)?;
    let selection = invocation
        .get("source_selection")
        .ok_or("source.bootstrap selection absent")?;
    let invocation_schema = invocation["schema_version"].as_str();
    let streamed_bootstrap =
        invocation_schema == Some("tos_local_pg_source_bootstrap_streamed_invocation_v1");
    let legacy_bootstrap = invocation_schema == Some("tos_local_pg_source_bootstrap_invocation_v1");
    if !(streamed_bootstrap || legacy_bootstrap)
        || selection["initialization_mode"] != "initialize"
        || selection["initialize_schema"] != true
        || selection["job_fence"] != 1
        || selection["domain"].as_str().is_none_or(str::is_empty)
        || selection["job_id"].as_str().is_none_or(str::is_empty)
    {
        return Err("source.bootstrap must select a fresh initial CMD.2 domain at fence 1".into());
    }
    if streamed_bootstrap {
        // The maintained streamed-bootstrap invocation carries an explicit
        // set of bounded import/generation budgets. Keep the legacy invocation
        // profile above supported, while requiring the streamed profile to
        // retain its current top-level budget contract and enough capacity for
        // the selected source membership before the owner is launched.
        let budget_keys = [
            "max_revisions",
            "max_members",
            "max_source_bytes",
            "max_member_bytes",
            "max_dependency_bytes_per_member",
            "max_manifest_bytes",
            "max_manifest_entries",
            "max_manifest_row_bytes",
            "max_index_bytes",
            "max_sqlite_cache_bytes",
            "index",
            "workspace",
            "generation",
            "tree",
            "item",
        ];
        let budgets = invocation
            .get("streamed_budgets")
            .and_then(Value::as_object)
            .ok_or("streamed source.bootstrap budgets absent")?;
        if budgets.len() != budget_keys.len()
            || budget_keys.iter().any(|key| !budgets.contains_key(*key))
        {
            return Err("streamed source.bootstrap budget fields changed".into());
        }
        let member_count = selection["source_membership_count"]
            .as_u64()
            .filter(|count| *count > 0)
            .ok_or("streamed source.bootstrap membership count absent")?;
        let max_members = budgets["max_members"]
            .as_u64()
            .ok_or("streamed source.bootstrap member budget absent")?;
        let generation = budgets["generation"]
            .as_object()
            .ok_or("streamed source.bootstrap generation budgets absent")?;
        let generation_members = generation
            .get("max_members")
            .and_then(Value::as_u64)
            .ok_or("streamed source.bootstrap generation member budget absent")?;
        let generation_seconds = generation
            .get("max_elapsed_seconds")
            .and_then(Value::as_u64)
            .ok_or("streamed source.bootstrap generation deadline absent")?;
        if max_members < member_count
            || generation_members < member_count
            || generation_seconds == 0
            || generation_seconds > 86_400
        {
            return Err(
                "streamed source.bootstrap budgets do not cover the selected membership".into(),
            );
        }
    }
    let store_root = PathBuf::from(
        selection["store_root"]
            .as_str()
            .ok_or("source.bootstrap store_root absent")?,
    )
    .canonicalize()
    .map_err(|e| format!("source.bootstrap store root: {e}"))?;
    let store_metadata = fs::symlink_metadata(&store_root)
        .map_err(|e| format!("source.bootstrap store root stat: {e}"))?;
    if !store_root.starts_with(&baseline)
        || store_metadata.file_type().is_symlink()
        || !store_metadata.is_dir()
        || store_metadata.uid() != unsafe { libc::geteuid() }
        || store_metadata.mode() & 0o077 != 0
        || fs::read_dir(&store_root)
            .map_err(|e| format!("source.bootstrap store root scan: {e}"))?
            .next()
            .is_some()
    {
        return Err(
            "source.bootstrap requires a fresh private empty store under the stage baseline".into(),
        );
    }
    let owner_path = PathBuf::from(
        invocation["owner_config"]
            .as_str()
            .ok_or("source.bootstrap owner_config absent")?,
    )
    .canonicalize()
    .map_err(|e| format!("source.bootstrap owner config path: {e}"))?;
    if !owner_path.starts_with(&baseline) {
        return Err(
            "source.bootstrap owner configuration must be inside the private stage baseline".into(),
        );
    }
    let owner = read_stage_json(&owner_path)?;
    if owner["schema_version"] != "tos_local_pg_source_bootstrap_owner_v1"
        || owner["source_root"] != store_root.to_str().ok_or("store root UTF-8 required")?
        || owner["domain"] != selection["domain"]
        || owner["job_id"] != selection["job_id"]
        || owner["job_fence"] != 1
        || owner["initialization_mode"] != "initialize"
        || owner["initialize_schema"] != true
        || owner["allowed_operations"] != json!(["source.bootstrap"])
    {
        return Err(
            "source.bootstrap owner configuration does not bind the fresh stage selection".into(),
        );
    }
    let roots = selected_sdk_store_roots(plan, args.sessions)?;
    if roots.len() != 1 || roots[0] != store_root {
        return Err(
            "all SDK source commands in one load stage must share the bootstrap-selected store"
                .into(),
        );
    }
    let domain = selection["domain"].clone();
    let contract = selection["source_contract_sha256"].clone();
    let job_id = selection["job_id"].clone();
    let job_fence = selection["job_fence"].clone();
    let store_limits = selection["store_limits"].clone();
    for projection_selection_path in selected_projection_inputs(plan, args.sessions)? {
        let projection_selection = read_stage_json(&projection_selection_path)?;
        let request_operation = if projection_selection["schema_version"] == NAVIGATION_OWNER_SCHEMA
        {
            "advance-navigation"
        } else {
            "status"
        };
        let projection_binding =
            projection_selection_binding(&projection_selection, request_operation)?;
        let projection_store = projection_binding
            .store_root
            .canonicalize()
            .map_err(|e| format!("projection selected store root: {e}"))?;
        if !projection_selection_path.starts_with(&baseline)
            || projection_store != store_root
            || projection_binding.domain != domain.as_str().ok_or("bootstrap domain absent")?
            || projection_binding.source_contract_sha256
                != contract.as_str().ok_or("bootstrap contract absent")?
        {
            return Err(
                "projection selection changed the fresh bootstrap domain, contract or store".into(),
            );
        }
    }
    for operation in plan
        .setup
        .iter()
        .chain(
            plan.sessions
                .iter()
                .take(args.sessions)
                .flat_map(|session| &session.operations),
        )
        .chain(plan.postload.iter())
        .filter(|operation| {
            operation.channel == "sdk"
                && !native_query_source_read(operation)
                && !native_pg_source_projection_read(operation)
        })
    {
        let path =
            PathBuf::from(invocation_argument(operation).ok_or("SDK PG invocation path absent")?)
                .canonicalize()
                .map_err(|e| format!("SDK PG invocation path: {e}"))?;
        if !path.starts_with(&baseline) {
            return Err(
                "every SDK PG invocation must remain inside the private stage baseline".into(),
            );
        }
        let invocation = read_stage_json(&path)?;
        if let Some(selected) = invocation.get("source_selection") {
            let selected_store = PathBuf::from(
                selected["store_root"]
                    .as_str()
                    .ok_or("SDK PG invocation store_root absent")?,
            )
            .canonicalize()
            .map_err(|e| format!("SDK PG invocation store root: {e}"))?;
            if selected_store != store_root
                || selected["domain"] != domain
                || selected["source_contract_sha256"] != contract
                || selected["job_id"] != job_id
                || selected["job_fence"] != job_fence
                || selected["store_limits"] != store_limits
            {
                return Err("SDK PG invocation changed the bootstrap domain, job, contract or store selection".into());
            }
        } else if invocation["schema_version"] == "tos_local_pg_rights_revoke_invocation_v1" {
            let private_root = PathBuf::from(
                invocation["private_source_root"]
                    .as_str()
                    .ok_or("rights.revoke private_source_root absent")?,
            )
            .canonicalize()
            .map_err(|e| format!("rights.revoke private source root: {e}"))?;
            let owner_path = PathBuf::from(
                invocation["owner_config"]
                    .as_str()
                    .ok_or("rights.revoke owner_config absent")?,
            )
            .canonicalize()
            .map_err(|e| format!("rights.revoke owner configuration: {e}"))?;
            let owner = read_stage_json(&owner_path)?;
            let executable = PathBuf::from(
                invocation["native_executable"]
                    .as_str()
                    .ok_or("rights.revoke native_executable absent")?,
            )
            .canonicalize()
            .map_err(|e| format!("rights.revoke native executable: {e}"))?;
            let selected_binary = PathBuf::from(
                operation
                    .sdk_argv
                    .first()
                    .ok_or("rights.revoke SDK binary absent")?,
            )
            .canonicalize()
            .map_err(|e| format!("selected SDK executable: {e}"))?;
            if private_root != store_root
                || !owner_path.starts_with(&baseline)
                || owner["schema_version"] != "tos_local_pg_rights_revoke_owner_v1"
                || owner["source_root"] != store_root.to_str().ok_or("store root UTF-8 required")?
                || owner["domain"] != domain
                || owner["allowed_operations"] != json!(["rights.revoke"])
                || executable != selected_binary
            {
                return Err("rights.revoke invocation changed the fresh bootstrap domain, private store, owner or native executable".into());
            }
        } else {
            return Err("every SDK PG invocation must select the bootstrap store or use the ordinary rights.revoke owner".into());
        }
    }
    Ok(true)
}

fn validate_existing_pg_stage(plan: &Plan, args: &Args) -> Result<(), String> {
    if std::env::var_os("TOS_SOURCE_PG_URL").is_none_or(|value| value.is_empty()) {
        return Err(
            "existing native PG stage requires TOS_SOURCE_PG_URL; its value is never reported"
                .into(),
        );
    }
    let baseline = args
        .owner_stage_baseline
        .as_ref()
        .ok_or("existing native PG stage requires a private --owner-stage-baseline")?
        .canonicalize()
        .map_err(|e| format!("existing native PG stage baseline: {e}"))?;
    let baseline_metadata = fs::symlink_metadata(&baseline)
        .map_err(|e| format!("existing native PG baseline stat: {e}"))?;
    if baseline_metadata.file_type().is_symlink()
        || !baseline_metadata.is_dir()
        || baseline_metadata.uid() != unsafe { libc::geteuid() }
        || baseline_metadata.mode() & 0o077 != 0
        || args.output.starts_with(&baseline)
        || baseline.starts_with(&args.output)
    {
        return Err(
            "existing native PG baseline must be a separate private owned directory".into(),
        );
    }
    let selected_operations = plan
        .setup
        .iter()
        .chain(
            plan.sessions
                .iter()
                .take(args.sessions)
                .flat_map(|session| &session.operations),
        )
        .chain(plan.postload.iter())
        .collect::<Vec<_>>();
    if selected_operations.iter().any(|operation| {
        operation.channel == "sdk" && operation_name(operation) == Some("source.bootstrap")
    }) {
        return Err(
            "existing native PG attach mode refuses source.bootstrap in every schedule phase"
                .into(),
        );
    }
    let first = plan
        .setup
        .first()
        .ok_or("existing native PG attach mode requires a serial setup describe before load")?;
    if first.channel != "sdk"
        || operation_name(first) != Some("describe")
        || first.request
            != json!({"schema_version":"tos_local_source_command_v1","operation":"describe"})
        || first.expected_status != 0
        || (first.expected_sha256.is_none() && first.expected_body.is_none())
    {
        return Err("existing native PG attach mode must begin with an exact successful owner-native describe oracle".into());
    }
    let roots = selected_sdk_store_roots(plan, args.sessions)?;
    if roots.len() != 1 {
        return Err(
            "existing native PG attach mode requires exactly one selected SDK PostgreSQL store"
                .into(),
        );
    }
    let store_root = roots[0]
        .canonicalize()
        .map_err(|e| format!("existing PostgreSQL store root: {e}"))?;
    let store_metadata = fs::symlink_metadata(&store_root)
        .map_err(|e| format!("existing PostgreSQL store stat: {e}"))?;
    if store_root.starts_with(&baseline)
        || store_root.starts_with(&args.output)
        || store_metadata.file_type().is_symlink()
        || !store_metadata.is_dir()
        || store_metadata.uid() != unsafe { libc::geteuid() }
        || store_metadata.mode() & 0o077 != 0
    {
        return Err("existing PG store must be a private owned directory outside the cloned owner-input baseline".into());
    }
    let owner_inputs = selected_sdk_owner_inputs(plan, args.sessions)?;
    if owner_inputs.is_empty()
        || owner_inputs.iter().any(|(invocation, owner)| {
            !invocation.starts_with(&baseline) || !owner.starts_with(&baseline)
        })
    {
        return Err("existing PG attach needs SDK invocation and owner inputs inside the private stage baseline".into());
    }

    let first_invocation_path = PathBuf::from(
        invocation_argument(first).ok_or("initial existing-PG describe invocation absent")?,
    )
    .canonicalize()
    .map_err(|e| format!("initial existing-PG describe invocation: {e}"))?;
    let first_invocation = read_stage_json(&first_invocation_path)?;
    if !first_invocation_path.starts_with(&baseline) {
        return Err(
            "initial existing-PG describe invocation must be inside the stage baseline".into(),
        );
    }
    let first_selection = first_invocation
        .get("source_selection")
        .ok_or("initial existing-PG describe must carry its acquired source selection")?;
    let domain = first_selection["domain"]
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or("initial existing-PG describe domain absent")?;
    let contract = first_selection["source_contract_sha256"]
        .as_str()
        .and_then(normalized_sha256)
        .ok_or("initial existing-PG describe source contract digest absent")?
        .to_owned();
    let selected_store = PathBuf::from(
        first_selection["store_root"]
            .as_str()
            .ok_or("initial existing-PG describe store root absent")?,
    )
    .canonicalize()
    .map_err(|e| format!("initial existing-PG selected store root: {e}"))?;
    if selected_store != store_root
        || first_selection["job_id"].as_str().is_none_or(str::is_empty)
        || first_selection["job_fence"]
            .as_u64()
            .is_none_or(|fence| fence == 0)
        || first_selection["initialization_mode"] == "initialize"
        || first_selection["initialize_schema"] == true
    {
        return Err("initial existing-PG describe must select the already-owned store and its own non-initializing lease selector".into());
    }
    if selected_operations
        .iter()
        .any(|operation| operation.channel == "owner_http")
    {
        let http_invocation_path = args
            .invocation
            .as_ref()
            .ok_or("owner_http requires its selected native invocation")?
            .canonicalize()
            .map_err(|e| format!("owner_http invocation path: {e}"))?;
        let http_owner_path = args
            .owner_config
            .as_ref()
            .ok_or("owner_http requires its selected owner configuration")?
            .canonicalize()
            .map_err(|e| format!("owner_http owner configuration path: {e}"))?;
        if !http_invocation_path.starts_with(&baseline) || !http_owner_path.starts_with(&baseline) {
            return Err("owner_http invocation and owner configuration must remain inside the private owner-input baseline".into());
        }
        let http_invocation = read_stage_json(&http_invocation_path)?;
        let selected_http_owner = http_invocation
            .get("owner_config")
            .and_then(Value::as_str)
            .ok_or("owner_http native invocation owner_config absent")?;
        if PathBuf::from(selected_http_owner)
            .canonicalize()
            .map_err(|e| format!("owner_http invocation owner configuration: {e}"))?
            != http_owner_path
        {
            return Err(
                "owner_http native invocation must select the supplied owner configuration".into(),
            );
        }
        let http_selection = http_invocation
            .get("source_selection")
            .ok_or("owner_http invocation source selection absent")?;
        let http_store = PathBuf::from(
            http_selection["store_root"]
                .as_str()
                .ok_or("owner_http invocation store root absent")?,
        )
        .canonicalize()
        .map_err(|e| format!("owner_http selected store root: {e}"))?;
        if http_store != store_root
            || http_selection["domain"].as_str() != Some(domain)
            || http_selection["source_contract_sha256"]
                .as_str()
                .and_then(normalized_sha256)
                != Some(contract.as_str())
            || http_selection["job_id"].as_str().is_none_or(str::is_empty)
            || http_selection["job_fence"]
                .as_u64()
                .is_none_or(|fence| fence == 0)
            || http_selection["initialization_mode"] == "initialize"
            || http_selection["initialize_schema"] == true
        {
            return Err("owner_http invocation must select the attached store, shared domain and contract, with its own non-initializing lease selector".into());
        }
        let http_owner = read_stage_json(&http_owner_path)?;
        let owner_store = PathBuf::from(
            http_owner["source_root"]
                .as_str()
                .ok_or("owner_http owner configuration source_root absent")?,
        )
        .canonicalize()
        .map_err(|e| format!("owner_http owner source root: {e}"))?;
        let allowed_operations = http_owner["allowed_operations"]
            .as_array()
            .ok_or("owner_http allowed_operations absent")?;
        if owner_store != store_root
            || http_owner.get("domain").is_some_and(|value| {
                value
                    .as_str()
                    .is_none_or(|selected_domain| selected_domain != domain)
            })
            || http_owner
                .get("source_contract_sha256")
                .is_some_and(|value| {
                    value
                        .as_str()
                        .and_then(normalized_sha256)
                        .is_none_or(|selected_contract| selected_contract != contract)
                })
            || selected_operations
                .iter()
                .filter(|operation| operation.channel == "owner_http")
                .any(|operation| {
                    operation_name(operation).is_none_or(|name| {
                        !allowed_operations
                            .iter()
                            .any(|allowed| allowed.as_str() == Some(name))
                    })
                })
        {
            return Err("owner_http configuration must target the attached store and permit every scheduled operation".into());
        }
    }
    if let Some(selection_path) = args.source_text_search_selection.as_ref() {
        let selection_path = selection_path
            .canonicalize()
            .map_err(|e| format!("PG source-text selection path: {e}"))?;
        if !selection_path.starts_with(&baseline) {
            return Err(
                "PG source-text search selection must be inside the private owner-input baseline"
                    .into(),
            );
        }
        let selection = read_stage_json(&selection_path)?;
        let binding = projection_selection_binding(&selection, "status")?;
        let selected_text_store = binding
            .store_root
            .canonicalize()
            .map_err(|e| format!("PG source-text selection store root: {e}"))?;
        if binding.navigation
            || selected_text_store != store_root
            || binding.domain != domain
            || normalized_sha256(&binding.source_contract_sha256) != Some(contract.as_str())
            || !matches!(
                selection["profile"].as_str(),
                Some("tos_source_selected_text_v1" | "tos_source_selected_text_v2")
            )
        {
            return Err("PG source-text selection must use its declared source-search profile and share the attached domain, source contract and store".into());
        }
    }
    for operation in selected_operations.iter().filter(|operation| {
        operation.channel == "sdk"
            && !native_query_source_read(operation)
            && !native_pg_source_projection_read(operation)
    }) {
        let path =
            PathBuf::from(invocation_argument(operation).ok_or("SDK PG invocation path absent")?)
                .canonicalize()
                .map_err(|e| format!("SDK PG invocation path: {e}"))?;
        if !path.starts_with(&baseline) {
            return Err("every attached SDK PG invocation must remain inside the private owner-input baseline".into());
        }
        let invocation = read_stage_json(&path)?;
        if let Some(selected) = invocation.get("source_selection") {
            let selected_store = PathBuf::from(
                selected["store_root"]
                    .as_str()
                    .ok_or("SDK PG invocation store root absent")?,
            )
            .canonicalize()
            .map_err(|e| format!("SDK PG invocation store root: {e}"))?;
            if selected_store != store_root
                || selected["domain"].as_str() != Some(domain)
                || selected["source_contract_sha256"]
                    .as_str()
                    .and_then(normalized_sha256)
                    != Some(contract.as_str())
                || selected["job_id"].as_str().is_none_or(str::is_empty)
                || selected["job_fence"]
                    .as_u64()
                    .is_none_or(|fence| fence == 0)
                || selected["initialization_mode"] == "initialize"
                || selected["initialize_schema"] == true
            {
                return Err("attached SDK PG invocation changed the shared domain, contract or store, or lacks its own non-initializing lease selector".into());
            }
        } else if invocation["schema_version"] == "tos_local_pg_rights_revoke_invocation_v1" {
            let private_root = PathBuf::from(
                invocation["private_source_root"]
                    .as_str()
                    .ok_or("rights.revoke private source root absent")?,
            )
            .canonicalize()
            .map_err(|e| format!("rights.revoke private source root: {e}"))?;
            let owner_path = PathBuf::from(
                invocation["owner_config"]
                    .as_str()
                    .ok_or("rights.revoke owner configuration absent")?,
            )
            .canonicalize()
            .map_err(|e| format!("rights.revoke owner configuration: {e}"))?;
            let owner = read_stage_json(&owner_path)?;
            let executable = PathBuf::from(
                invocation["native_executable"]
                    .as_str()
                    .ok_or("rights.revoke native executable absent")?,
            )
            .canonicalize()
            .map_err(|e| format!("rights.revoke native executable: {e}"))?;
            let selected_binary = PathBuf::from(
                operation
                    .sdk_argv
                    .first()
                    .ok_or("rights.revoke SDK binary absent")?,
            )
            .canonicalize()
            .map_err(|e| format!("selected SDK executable: {e}"))?;
            if private_root != store_root
                || !owner_path.starts_with(&baseline)
                || owner["schema_version"] != "tos_local_pg_rights_revoke_owner_v1"
                || owner["source_root"] != store_root.to_str().ok_or("store root UTF-8 required")?
                || owner["domain"] != domain
                || owner["allowed_operations"] != json!(["rights.revoke"])
                || executable != selected_binary
            {
                return Err("rights.revoke changed the attached store, selected owner grant or SDK executable".into());
            }
        } else {
            return Err("attached SDK PG commands must carry their own source selection or the ordinary rights.revoke owner".into());
        }
    }
    for selection_path in selected_projection_inputs(plan, args.sessions)? {
        let selection = read_stage_json(&selection_path)?;
        let request_operation = if selection["schema_version"] == NAVIGATION_OWNER_SCHEMA {
            "advance-navigation"
        } else {
            "status"
        };
        let binding = projection_selection_binding(&selection, request_operation)?;
        let projected_store = binding
            .store_root
            .canonicalize()
            .map_err(|e| format!("attached projection store root: {e}"))?;
        if !selection_path.starts_with(&baseline)
            || projected_store != store_root
            || binding.domain != domain
            || normalized_sha256(&binding.source_contract_sha256) != Some(contract.as_str())
        {
            return Err("attached projection selection must share the selected domain, source contract and existing PG store".into());
        }
    }
    Ok(())
}

fn validate_pg_stage(plan: &Plan, args: &Args) -> Result<bool, String> {
    match args.pg_stage_mode {
        PgStageMode::Fresh => validate_fresh_pg_stage(plan, args),
        PgStageMode::Existing => {
            validate_existing_pg_stage(plan, args)?;
            Ok(true)
        }
    }
}

fn validate_projection_operation(
    operation: &Operation,
    args: &Args,
    allowed_operations: &[&str],
) -> Result<(), String> {
    let argv = projection_selection_argv(operation).ok_or(
        "projection operation requires its channel-specific native source-projection argv",
    )?;
    let operation_name = operation.request["operation"]
        .as_str()
        .ok_or("projection request operation absent")?;
    let selected_sdk_projection = operation.channel == "sdk";
    if argv.is_empty()
        || argv.len() > 32
        || argv[0].is_empty()
        || !Path::new(&argv[0]).is_absolute()
        || !argv.iter().any(|argument| argument == "source-projection")
        || !allowed_operations.contains(&operation_name)
        || argv
            .windows(2)
            .filter(|pair| pair[0] == "--selection")
            .count()
            != 1
        || !operation.request.is_object()
        || (selected_sdk_projection && !native_pg_source_projection_read(operation))
        || (selected_sdk_projection && !operation.projection_argv.is_empty())
        || (!selected_sdk_projection && !operation.sdk_argv.is_empty())
        || operation.retry_same_command_id
        || operation.reconnect_before
        || operation.conflict_group.is_some()
        || operation.bind_expected_configuration_from_owner
        || operation.bind_expected_from.is_some()
        || json_bytes(&operation.request)?.len() > args.request_cap
    {
        return Err("projection operation requires the installed source-projection CLI, one private selection and an allowed bounded request".into());
    }
    let selection_path = projection_selection_path(operation)?;
    let selection = read_stage_json(&selection_path)?;
    let binding = projection_selection_binding(&selection, operation_name)?;
    if let Some(max_events) = operation.request["max_events"].as_u64() {
        if max_events == 0 || max_events > 256 {
            return Err("projection max_events must be between 1 and 256".into());
        }
        if binding.navigation
            && max_events
                > selection["source_navigation"]["producer"]["max_events"]
                    .as_u64()
                    .ok_or("navigation producer event limit absent")?
        {
            return Err("navigation request exceeds the protected producer event limit".into());
        }
    }
    match operation_name {
        "bootstrap-navigation" => {
            if operation
                .request
                .as_object()
                .is_none_or(|fields| fields.len() != 1 || !fields.contains_key("operation"))
                || operation.interrupt_resume_after_ms.is_some()
            {
                return Err(
                    "navigation bootstrap is a one-time setup request with no extra fields".into(),
                );
            }
        }
        "advance-navigation" => {
            if operation.request.as_object().is_none_or(|fields| {
                fields.len() != 2
                    || !fields.contains_key("operation")
                    || !fields.contains_key("max_events")
            }) || operation.request["max_events"].as_u64() != Some(1)
                || operation.expected_status != 0
                || operation.expected_sha256.is_some()
                || operation.expected_body.is_some()
                || !operation.ignored_json_paths.is_empty()
                || !operation.alternate_outcomes.is_empty()
                || operation.interrupt_resume_after_ms.is_some()
            {
                return Err("navigation load advancement requires one bounded event and a successful exact response".into());
            }
        }
        "exact-source" => {
            if !exact_source_projection_request(&operation.request) {
                return Err(
                    "exact-source requires one selected relative path and positive revision".into(),
                );
            }
        }
        "apply" if operation.interrupt_resume_after_ms.is_some() => {
            if operation.request.as_object().is_none_or(|fields| {
                fields.len() != 2
                    || !fields.contains_key("operation")
                    || !fields.contains_key("max_events")
            }) || operation.expected_status != 0
                || operation.expected_sha256.is_some()
                || operation.expected_body.is_some()
                || !operation.ignored_json_paths.is_empty()
                || !operation.alternate_outcomes.is_empty()
            {
                return Err("interrupted projection apply requires a bounded progress-response oracle, not a static body or digest".into());
            }
        }
        _ => {}
    }
    if operation.interrupt_resume_after_ms.is_some()
        && (operation_name != "apply"
            || operation.request["max_events"]
                .as_u64()
                .is_none_or(|events| events < 2)
            || operation.interrupt_resume_after_ms == Some(0)
            || operation
                .interrupt_resume_after_ms
                .is_some_and(|millis| millis >= args.deadline_seconds.saturating_mul(1000)))
    {
        return Err("projection interruption requires bounded multi-event apply progress".into());
    }
    for argument in argv {
        if argument.len() > 4096 {
            return Err("projection argv element exceeds 4096 bytes".into());
        }
    }
    Ok(())
}

fn outcome_report(op: &Operation) -> Value {
    if op
        .expected_body
        .as_ref()
        .is_some_and(record_revise_semantic_oracle_present)
    {
        return json!([{"status":0,"oracle":"prepare-bound-native-pg-record-revise-with-exact-postload-replay"}]);
    }
    if exact_source_projection_operation(op) {
        return json!([{"status":0,"oracle":"exact-source-bytes-revision-and-bounded-read-work"}]);
    }
    if op.channel == "projection" && op.request["operation"] == "advance-navigation" {
        return json!([{"status":0,"oracle":"bounded-navigation-range"}]);
    }
    if interrupted_projection_apply(op) {
        return json!([{"status":0,"oracle":"bounded-interrupted-projection-progress-and-resumed-cursor"}]);
    }
    Value::Array(outcomes(op).iter().map(outcome_summary).collect())
}

fn outcome_summary(value: &ExpectedOutcome) -> Value {
    json!({
        "status": value.status,
        "sha256": value.sha256,
        "semantic_body_expected": value.body.is_some(),
        "ignored_json_paths": value.ignored_json_paths,
    })
}

fn validate_plan(plan: &Plan, args: &Args) -> Result<(usize, bool, bool), String> {
    if plan.schema != SCHEDULE_SCHEMA
        || plan.sessions.len() < args.sessions
        || plan.sessions.len() > MAX_SESSIONS
        || plan.setup.len() > 64
        || plan.postload.len() > 64
    {
        return Err("schedule schema/session count mismatch".into());
    }
    let native_pg_stage = validate_pg_stage(plan, args)?;
    let fresh_pg_stage = native_pg_stage && args.pg_stage_mode == PgStageMode::Fresh;
    let mut ids = BTreeSet::new();
    let mut total = plan.setup.len() + plan.postload.len();
    let mut owner_requests = 0usize;
    let mut command_ids = BTreeSet::new();
    let mut setup_labels = BTreeSet::new();
    let mut setup_previews: BTreeMap<String, (String, String)> = BTreeMap::new();
    let mut navigation_bootstrap_setup_count = 0usize;
    let mut needs_mcp = false;
    let mut needs_owner = false;
    let mut conflicts: BTreeMap<(usize, String), Vec<(usize, String, Vec<ExpectedOutcome>)>> =
        BTreeMap::new();

    // Setup is deliberately serial and owner-native. It is the only place
    // where a stage may bootstrap its fresh PostgreSQL namespace or derive
    // request bindings from an ordinary non-committing preview.
    for (setup_index, op) in plan.setup.iter().enumerate() {
        let request_bytes = json_bytes(&op.request)?;
        let name = operation_name(op).ok_or("setup operation name absent")?;
        if op.channel == "projection" {
            if name != "bootstrap-navigation"
                || setup_index + 1 != plan.setup.len()
                || navigation_bootstrap_setup_count != 0
                || op.label.is_empty()
                || op.label.len() > 64
                || !op.label.is_ascii()
                || !setup_labels.insert(op.label.clone())
                || op.expected_status != 0
                || !valid_expected(
                    &ExpectedOutcome {
                        status: op.expected_status,
                        sha256: op.expected_sha256.clone(),
                        body: op.expected_body.clone(),
                        ignored_json_paths: op.ignored_json_paths.clone(),
                    },
                    true,
                    args.response_cap,
                )
                || !op.alternate_outcomes.is_empty()
                || op.retry_same_command_id
                || op.reconnect_before
                || op.conflict_group.is_some()
                || op.bind_expected_configuration_from_owner
                || op.bind_expected_from.is_some()
                || op.idempotency_replay_of.is_some()
                || op.expected_rights_allowed.is_some()
            {
                return Err(
                    "navigation bootstrap is permitted once, as the final serial setup operation"
                        .into(),
                );
            }
            validate_projection_operation(op, args, &["bootstrap-navigation"])?;
            navigation_bootstrap_setup_count += 1;
            continue;
        }
        if name == "describe" && op.channel == "sdk" {
            let path =
                PathBuf::from(invocation_argument(op).ok_or("setup describe invocation absent")?);
            let invocation = read_stage_json(&path)?;
            if invocation["schema_version"] == "tos_local_pg_rights_revoke_invocation_v1"
                && !op.bind_expected_configuration_from_owner
            {
                return Err(
                    "ordinary rights describe setup must bind its exact staged owner configuration"
                        .into(),
                );
            }
        }
        if op.channel != "sdk"
            || op.label.is_empty()
            || op.label.len() > 64
            || !op.label.is_ascii()
            || !setup_labels.insert(op.label.clone())
            || op.expected_status != 0
            || !valid_expected(
                &ExpectedOutcome {
                    status: op.expected_status,
                    sha256: op.expected_sha256.clone(),
                    body: op.expected_body.clone(),
                    ignored_json_paths: op.ignored_json_paths.clone(),
                },
                true,
                args.response_cap,
            )
            || !op.alternate_outcomes.is_empty()
            || op.retry_same_command_id
            || op.reconnect_before
            || op.conflict_group.is_some()
            || (op.bind_expected_configuration_from_owner
                && (name != "describe"
                    || op
                        .request
                        .get("expected_configuration")
                        .is_none_or(|value| !value.is_null())))
            || op.idempotency_replay_of.is_some()
            || op.expected_rights_allowed.is_some()
            || !op.projection_argv.is_empty()
            || op.interrupt_resume_after_ms.is_some()
            || op.sdk_argv.len() > 32
            || op
                .sdk_argv
                .first()
                .is_none_or(|path| !Path::new(path).is_absolute() || path.is_empty())
            || !op.sdk_argv.iter().any(|arg| arg == "source-commands")
            || invocation_argument(op).is_none()
            || op
                .sdk_argv
                .windows(2)
                .filter(|pair| pair[0] == "--invocation")
                .count()
                != 1
            || !op.request.is_object()
            || request_bytes.len() > args.request_cap
            || !matches!(
                name,
                "source.bootstrap"
                    | "describe"
                    | "prepare-create"
                    | "prepare-revise"
                    | "source.create"
                    | "record.revise"
                    | "work.revision-schema.provision"
                    | "record.revision-schema.provision"
                    | "rights.revoke"
            )
        {
            return Err(
                "setup accepts only bounded serial native bootstrap, preview and commit commands"
                    .into(),
            );
        }
        if matches!(
            name,
            "work.revision-schema.provision" | "record.revision-schema.provision"
        ) {
            validate_revision_schema_provision_grant(op, name)?;
        }
        for arg in &op.sdk_argv {
            if arg.len() > 4096 {
                return Err("setup SDK argv element exceeds4096 bytes".into());
            }
        }
        let command_id = op
            .request
            .get("command_id")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty() && value.len() <= 128);
        if matches!(name, "source.create" | "record.revise" | "rights.revoke") {
            if command_id.is_none() || !op.bind_expected_from.is_some() {
                return Err(
                    "serial PG mutations require a command_id and exact owner preview binding"
                        .into(),
                );
            }
            if !command_ids.insert(command_id.unwrap().to_owned()) {
                return Err(
                    "each scheduled native source command needs a unique command_id".into(),
                );
            }
            if [
                "expected_configuration",
                "expected_source",
                "expected_revision",
                "expected_dependencies",
                "expected_publication",
            ]
            .iter()
            .any(|field| op.request.get(*field).is_some_and(|value| !value.is_null()))
                || (name == "rights.revoke"
                    && (!op
                        .request
                        .get("expected_rights_version")
                        .is_some_and(Value::is_null)
                        || !op
                            .request
                            .get("expected_configuration")
                            .is_some_and(Value::is_null)))
            {
                return Err(
                    "setup commit expected fields must be supplied only by the selected preview"
                        .into(),
                );
            }
        } else if command_id.is_some() {
            return Err(
                "native setup bootstrap and prepare operations must not carry commit command IDs"
                    .into(),
            );
        }
        if fresh_pg_stage && setup_index == 0 && name != "source.bootstrap" {
            return Err("serial native PG setup must start with source.bootstrap".into());
        }
        if native_pg_stage
            && args.pg_stage_mode == PgStageMode::Existing
            && setup_index == 0
            && name != "describe"
        {
            return Err(
                "existing native PG setup must start with the selected current-state describe"
                    .into(),
            );
        }
        if name == "source.bootstrap" && (!fresh_pg_stage || setup_index != 0) {
            return Err(
                "source.bootstrap is permitted only as the first serial setup operation in fresh PG mode".into(),
            );
        }
        if let Some(label) = op.bind_expected_from.as_deref() {
            let Some((preview_operation, preview_invocation)) = setup_previews.get(label) else {
                return Err(
                    "setup binding must reference an earlier labeled prepare preview".into(),
                );
            };
            let compatible = matches!(
                (name, preview_operation.as_str()),
                ("source.create", "prepare-create")
                    | ("record.revise", "prepare-revise")
                    | ("rights.revoke", "describe")
            );
            if !compatible
                || invocation_argument(op) != Some(preview_invocation.as_str())
                || !(expected_binding_operation(op) || rights_revoke_operation(op))
            {
                return Err(
                    "setup binding must use the matching owner preview and exact same invocation"
                        .into(),
                );
            }
        } else if expected_binding_operation(op) || rights_revoke_operation(op) {
            return Err("native PG mutations require bind_expected_from".into());
        }
        if setup_preview_operation(op) || rights_describe_binding_operation(op) {
            setup_previews.insert(
                op.label.clone(),
                (name.to_owned(), invocation_argument(op).unwrap().to_owned()),
            );
        }
    }
    if fresh_pg_stage && plan.setup.is_empty() {
        return Err(
            "native PG mutations require a serial bootstrap and fixture setup phase".into(),
        );
    }
    for (session_index, session) in plan.sessions.iter().take(args.sessions).enumerate() {
        if session.id.is_empty()
            || session.id.len() > 64
            || !session.id.is_ascii()
            || !ids.insert(session.id.clone())
        {
            return Err("session IDs must be unique bounded ASCII".into());
        }
        if session.operations.len() > 64 {
            return Err("per-session operation count exceeds64".into());
        }
        total = total
            .checked_add(session.operations.len())
            .ok_or("operation count overflow")?;
        let mut session_operation_labels = BTreeSet::new();
        for (step, op) in session.operations.iter().enumerate() {
            if op.label.is_empty()
                || op.label.len() > 64
                || !op.label.is_ascii()
                || !session_operation_labels.insert(op.label.clone())
                || op.idempotency_replay_of.is_some()
                || op.expected_rights_allowed.is_some()
            {
                return Err(
                    "operation labels must be unique bounded ASCII within each session".into(),
                );
            }
            let sdk = op.channel == "sdk";
            let native_process = matches!(op.channel.as_str(), "sdk" | "projection");
            let primary = ExpectedOutcome {
                status: op.expected_status,
                sha256: op.expected_sha256.clone(),
                body: op.expected_body.clone(),
                ignored_json_paths: op.ignored_json_paths.clone(),
            };
            let navigation_advance =
                op.channel == "projection" && op.request["operation"] == "advance-navigation";
            let interrupted_projection_apply = interrupted_projection_apply(op);
            let primary_valid = if navigation_advance {
                op.expected_status == 0
                    && op.expected_sha256.is_none()
                    && op.expected_body.is_none()
                    && op.ignored_json_paths.is_empty()
            } else if op
                .expected_body
                .as_ref()
                .is_some_and(record_revise_semantic_oracle_present)
            {
                native_pg_stage
                    && record_revise_semantic_oracle_valid(op, op.expected_body.as_ref().unwrap())
            } else if interrupted_projection_apply {
                op.expected_status == 0
                    && op.expected_sha256.is_none()
                    && op.expected_body.is_none()
                    && op.ignored_json_paths.is_empty()
                    && op.alternate_outcomes.is_empty()
            } else if exact_source_projection_operation(op) {
                op.expected_status == 0
                    && op.expected_sha256.is_none()
                    && op
                        .expected_body
                        .as_ref()
                        .is_some_and(|body| exact_source_projection_template_valid(op, body))
                    && op.ignored_json_paths.is_empty()
            } else {
                valid_expected(&primary, native_process, args.response_cap)
            };
            if op.label.is_empty()
                || op.label.len() > 64
                || !op.label.is_ascii()
                || !primary_valid
                || op.alternate_outcomes.len() > 1
                || op
                    .alternate_outcomes
                    .iter()
                    .any(|value| !valid_expected(value, native_process, args.response_cap))
            {
                return Err("operation label/status/digest invalid".into());
            }
            match op.channel.as_str() {
                "mcp" => {
                    needs_mcp = true;
                    if op.retry_same_command_id {
                        return Err("MCP retry is not supported by this schedule route".into());
                    }
                    if !op.sdk_argv.is_empty()
                        || !op.projection_argv.is_empty()
                        || op.conflict_group.is_some()
                        || op.interrupt_resume_after_ms.is_some()
                        || op.bind_expected_configuration_from_owner
                        || op.bind_expected_from.is_some()
                    {
                        return Err("invalid MCP operation options".into());
                    }
                    let b = json_bytes(&op.request)?;
                    if b.len() > 65536
                        || op.request["jsonrpc"] != "2.0"
                        || op.request.get("method").and_then(Value::as_str).is_none()
                        || op.request.get("id").is_none_or(Value::is_null)
                    {
                        return Err("MCP schedule entry must be a bounded JSON-RPC request".into());
                    }
                    if mcp_source_text_search_operation(op)
                        && (!valid_mcp_source_text_search_request(op) || op.expected_status != 200)
                    {
                        return Err("selected PG source-text MCP requests require bounded query arguments and an HTTP 200 JSON-RPC success oracle".into());
                    }
                }
                "owner_http" => {
                    needs_owner = true;
                    owner_requests += 1 + usize::from(op.retry_same_command_id);
                    let command_id = op
                        .request
                        .get("command_id")
                        .and_then(Value::as_str)
                        .filter(|v| !v.is_empty() && v.len() <= 128)
                        .ok_or("owner_http requires a bounded command_id")?;
                    if !command_ids.insert(command_id.to_owned()) {
                        return Err(
                            "each scheduled owner command needs a unique independent command_id"
                                .into(),
                        );
                    }
                    if op.request.as_object().is_none()
                        || op.reconnect_before
                        || !op.sdk_argv.is_empty()
                        || !op.projection_argv.is_empty()
                        || op.interrupt_resume_after_ms.is_some()
                        || (op.bind_expected_configuration_from_owner
                            && (args.owner_stage_baseline.is_none()
                                || op.request["expected_configuration"].as_str().is_none()))
                        || op.bind_expected_from.is_some()
                    {
                        return Err("owner_http requires an exact source-command object and no SDK/MCP-only options".into());
                    }
                    if json_bytes(&op.request)?.len() > args.request_cap {
                        return Err("source-command body exceeds request cap".into());
                    }
                }
                "sdk" => {
                    let invocation_args = op
                        .sdk_argv
                        .windows(2)
                        .filter(|pair| pair[0] == "--invocation")
                        .collect::<Vec<_>>();
                    let query_source_read = selected_query_source_read(op, args);
                    let pg_projection_source_read =
                        selected_native_pg_source_projection_read(op, args);
                    let owner_source_command =
                        op.sdk_argv.iter().any(|arg| arg == "source-commands");
                    let command_id = op
                        .request
                        .get("command_id")
                        .and_then(Value::as_str)
                        .filter(|value| !value.is_empty() && value.len() <= 128);
                    let read_only_describe = op.request["schema_version"]
                        == "tos_local_source_command_v1"
                        && op.request["operation"] == "describe"
                        && op.request.as_object().is_some_and(|fields| {
                            fields.len() == 2
                                && fields.contains_key("schema_version")
                                && fields.contains_key("operation")
                        });
                    let read_only_rights_describe = op.request["schema_version"]
                        == "tos_local_source_command_v1"
                        && op.request["operation"] == "describe"
                        && op.request.as_object().is_some_and(|fields| {
                            fields.len() == 3
                                && fields.contains_key("schema_version")
                                && fields.contains_key("operation")
                                && fields.get("expected_configuration").is_some_and(|value| {
                                    value.as_str().is_some_and(|digest| {
                                        digest.strip_prefix("sha256:").is_some_and(valid_sha)
                                    })
                                })
                        });
                    let bound_commit = op.bind_expected_from.is_some()
                        && expected_binding_operation(op)
                        && [
                            "expected_configuration",
                            "expected_source",
                            "expected_revision",
                            "expected_dependencies",
                            "expected_publication",
                        ]
                        .iter()
                        .all(|field| op.request.get(*field).is_none_or(Value::is_null));
                    let bound_rights_revoke = op.bind_expected_from.is_some()
                        && rights_revoke_operation(op)
                        && op
                            .request
                            .get("expected_configuration")
                            .is_some_and(Value::is_null)
                        && op
                            .request
                            .get("expected_rights_version")
                            .is_some_and(Value::is_null);
                    if op.sdk_argv.is_empty()
                        || op.sdk_argv.len() > 32
                        || op.sdk_argv[0].is_empty()
                        || !Path::new(&op.sdk_argv[0]).is_absolute()
                        || (!query_source_read
                            && !pg_projection_source_read
                            && !owner_source_command)
                        || (source_read_operation(op)
                            && !(query_source_read || pg_projection_source_read))
                        || (query_source_read
                            && (!exact_source_read_request(&op.request)
                                || !invocation_args.is_empty()
                                || command_id.is_some()
                                || op.retry_same_command_id
                                || op.conflict_group.is_some()
                                || op.bind_expected_from.is_some()
                                || op.bind_expected_configuration_from_owner))
                        || (pg_projection_source_read
                            && (op.expected_status != 0
                                || op.expected_stderr_prefix.is_some()
                                || !op.alternate_outcomes.is_empty()
                                || !invocation_args.is_empty()
                                || command_id.is_some()
                                || op.retry_same_command_id
                                || op.conflict_group.is_some()
                                || op.bind_expected_from.is_some()
                                || op.bind_expected_configuration_from_owner
                                || op.reconnect_before
                                || op.interrupt_resume_after_ms.is_some()))
                        || !op.projection_argv.is_empty()
                        || (!query_source_read
                            && !pg_projection_source_read
                            && invocation_args.len() != 1)
                        || (!query_source_read
                            && !pg_projection_source_read
                            && command_id.is_none()
                            && !read_only_describe
                            && !read_only_rights_describe)
                        || op.reconnect_before
                        || (op.retry_same_command_id && op.conflict_group.is_some())
                        || op.bind_expected_configuration_from_owner
                        || (op.bind_expected_from.is_some()
                            && !(bound_commit || bound_rights_revoke))
                        || (native_pg_stage
                            && (expected_binding_operation(op) || rights_revoke_operation(op))
                            && !(bound_commit || bound_rights_revoke))
                        || !op.request.is_object()
                        || json_bytes(&op.request)?.len() > args.request_cap
                    {
                        return Err("SDK entries require an exact selected source-reader/projection argv or one explicit native owner invocation".into());
                    }
                    if pg_projection_source_read {
                        if !native_pg_stage {
                            return Err("SDK source projection exact-source is permitted only in a selected native PostgreSQL stage".into());
                        }
                        validate_projection_operation(op, args, &["exact-source"])?;
                    }
                    if let Some(command_id) = command_id {
                        if !command_ids.insert(command_id.to_owned()) {
                            return Err(
                                "each scheduled source command needs a unique command_id".into()
                            );
                        }
                    }
                    if let Some(label) = op.bind_expected_from.as_deref() {
                        let Some((preview_operation, preview_invocation)) =
                            setup_previews.get(label)
                        else {
                            return Err("load binding must reference a serial setup preview".into());
                        };
                        let compatible = matches!(
                            (operation_name(op), preview_operation.as_str()),
                            (Some("source.create"), "prepare-create")
                                | (Some("record.revise"), "prepare-revise")
                                | (Some("rights.revoke"), "describe")
                        );
                        if !compatible
                            || invocation_argument(op) != Some(preview_invocation.as_str())
                        {
                            return Err("load binding must use the matching setup preview and same invocation".into());
                        }
                    }
                    for arg in &op.sdk_argv {
                        if arg.len() > 4096 {
                            return Err("SDK argv element exceeds4096 bytes".into());
                        }
                    }
                }
                "projection" => {
                    let projection_operation = op.request["operation"].as_str();
                    validate_projection_operation(
                        op,
                        args,
                        &[
                            "init",
                            "status",
                            "apply",
                            "search",
                            "search-text",
                            "exact-source",
                            "advance-navigation",
                        ],
                    )?;
                    if projection_operation == Some("advance-navigation") && op.expected_status != 0
                    {
                        return Err(
                            "navigation advancement expects the ordinary successful response"
                                .into(),
                        );
                    }
                }
                _ => return Err("channel must be mcp, owner_http, sdk or projection".into()),
            }
            if let Some(group) = &op.conflict_group {
                if group.is_empty()
                    || group.len() > 64
                    || !matches!(op.channel.as_str(), "owner_http" | "sdk")
                {
                    return Err(
                        "conflict_group requires owner_http or native SDK operations".into(),
                    );
                }
                conflicts.entry((step, group.clone())).or_default().push((
                    session_index,
                    op.channel.clone(),
                    outcomes(op),
                ));
            } else if !op.alternate_outcomes.is_empty() {
                return Err("alternate_outcomes require a conflict_group".into());
            }
        }
    }
    let mut load_mutations = BTreeMap::<String, &Operation>::new();
    for operation in plan
        .sessions
        .iter()
        .take(args.sessions)
        .flat_map(|session| &session.operations)
        .filter(|operation| replayable_source_mutation_operation(operation))
    {
        if let Some(command_id) = operation.request["command_id"].as_str() {
            load_mutations.insert(command_id.to_owned(), operation);
        }
    }
    let mut postload_labels = BTreeSet::new();
    let mut postload_integrity_checks = 0usize;
    let mut postload_selected_integrity_checks = 0usize;
    let mut postload_pg_integrity_checks = 0usize;
    let mut postload_rights_checks = 0usize;
    let mut postload_idempotency_checks = 0usize;
    let mut postload_integrity_positions = Vec::<usize>::new();
    let mut postload_selected_integrity_positions = Vec::<usize>::new();
    let mut postload_pg_integrity_positions = Vec::<usize>::new();
    let mut postload_idempotency_positions = Vec::<usize>::new();
    let mut postload_rights_check_positions = Vec::<(usize, bool)>::new();
    let mut postload_rights_revoke_positions = Vec::<usize>::new();
    let mut postload_rights_denial_positions = Vec::<usize>::new();
    let mut postload_pre_revoke_describes = BTreeMap::<String, Operation>::new();
    let preload_rights_revoke_scheduled = plan
        .setup
        .iter()
        .chain(
            plan.sessions
                .iter()
                .take(args.sessions)
                .flat_map(|session| &session.operations),
        )
        .any(|operation| rights_revoke_operation(operation));
    if native_pg_stage && preload_rights_revoke_scheduled {
        return Err("native PostgreSQL rights revocation is a postload operation after integrity and replay checks".into());
    }
    for (postload_index, op) in plan.postload.iter().enumerate() {
        let bound_postload_revoke = postload_rights_revoke_operation(op)
            && op
                .bind_expected_from
                .as_ref()
                .is_some_and(|label| postload_pre_revoke_describes.contains_key(label));
        let bound_load_replay = op.idempotency_replay_of.is_some()
            && op
                .bind_expected_from
                .as_ref()
                .is_none_or(|label| setup_previews.contains_key(label));
        let allowed_binding =
            op.bind_expected_from.is_none() || bound_postload_revoke || bound_load_replay;
        if op.label.is_empty()
            || op.label.len() > 64
            || !op.label.is_ascii()
            || !postload_labels.insert(op.label.clone())
            || !op.alternate_outcomes.is_empty()
            || op.retry_same_command_id
            || op.reconnect_before
            || op.conflict_group.is_some()
            || (op.bind_expected_configuration_from_owner && !postload_rights_check(op))
            || op.interrupt_resume_after_ms.is_some()
            || (op.idempotency_replay_of.is_none()
                && !postload_rights_revoke_operation(op)
                && op.bind_expected_from.is_some())
            || !allowed_binding
        {
            return Err("postload checks require unique labels and cannot carry load-only retry, reconnect, conflict or interruption options".into());
        }
        if postload_rights_revoke_operation(op) {
            let label = op.bind_expected_from.as_deref().ok_or(
                "postload rights.revoke must bind to its preceding current-rights describe",
            )?;
            let describe = postload_pre_revoke_describes
                .get(label)
                .ok_or("postload rights.revoke must bind to a prior rights-enabled describe")?;
            if invocation_argument(describe) != invocation_argument(op)
                || describe.sdk_argv != op.sdk_argv
                || op.request["schema_version"] != "tos_local_source_command_v1"
                || op.request["expected_configuration"] != Value::Null
                || op.request["expected_rights_version"] != Value::Null
            {
                return Err("postload rights.revoke must use the exact rights describe invocation and bind its live configuration/version".into());
            }
            postload_rights_revoke_positions.push(postload_index);
        }
        if postload_projection_rights_denial(op) {
            postload_rights_denial_positions.push(postload_index);
        }
        if let Some(target_command_id) = op.idempotency_replay_of.as_deref() {
            let target = load_mutations.get(target_command_id).copied().ok_or(
                "postload idempotency replay must name a selected load-phase SDK mutation",
            )?;
            if !replayable_source_mutation_operation(op)
                || op.request["command_id"] != target.request["command_id"]
                || op.request != target.request
                || op.sdk_argv != target.sdk_argv
                || op.bind_expected_from != target.bind_expected_from
                || op.expected_status != target.expected_status
                || op.expected_sha256 != target.expected_sha256
                || op.expected_body != target.expected_body
                || op.expected_stderr_prefix != target.expected_stderr_prefix
                || op.ignored_json_paths != target.ignored_json_paths
                || op.expected_sha256.is_some()
                || target.expected_sha256.is_some()
                || op.expected_body.is_none()
                || target.expected_body.is_none()
                || !target.expected_body.as_ref().is_some_and(|body| {
                    if record_revise_semantic_oracle_present(body) {
                        record_revise_semantic_oracle_valid(target, body)
                            && op.expected_body.as_ref().is_some_and(|replay_body| {
                                replay_body == body
                                    && record_revise_semantic_oracle_valid(op, replay_body)
                            })
                    } else {
                        IDEMPOTENCY_REPLAY_JSON_PATHS
                            .iter()
                            .all(|path| body.pointer(path) == Some(&Value::Bool(false)))
                    }
                })
                || target.expected_status != 0
                || target.conflict_group.is_some()
                || !target.alternate_outcomes.is_empty()
                || op.channel != "sdk"
                || op.sdk_argv.is_empty()
                || !op.projection_argv.is_empty()
            {
                return Err("postload idempotency replay must repeat one non-conflicting load SDK mutation and its exact oracle".into());
            }
            postload_idempotency_checks += 1;
            postload_idempotency_positions.push(postload_index);
            continue;
        }
        if !postload_rights_revoke_operation(op) && !postload_read_only_operation(op) {
            return Err("postload checks accept only source reads, native rights revocation, describe, and read-only projection operations".into());
        }
        let native_process = matches!(op.channel.as_str(), "sdk" | "projection");
        let primary = ExpectedOutcome {
            status: op.expected_status,
            sha256: op.expected_sha256.clone(),
            body: op.expected_body.clone(),
            ignored_json_paths: op.ignored_json_paths.clone(),
        };
        let exact_oracle_valid =
            if postload_rights_revoke_operation(op) || postload_rights_check(op) {
                op.expected_status == 0
                    && op.expected_sha256.is_none()
                    && op.expected_body.is_none()
                    && op.ignored_json_paths.is_empty()
            } else if postload_projection_rights_denial(op) {
                op.expected_status == 2
                    && op.expected_sha256.is_none()
                    && op.expected_body.is_none()
                    && op.ignored_json_paths.is_empty()
            } else if exact_source_projection_operation(op) {
                op.expected_status == 0
                    && op.expected_sha256.is_none()
                    && op
                        .expected_body
                        .as_ref()
                        .is_some_and(|body| exact_source_projection_template_valid(op, body))
                    && op.ignored_json_paths.is_empty()
            } else {
                valid_expected(&primary, native_process, args.response_cap)
            };
        if !exact_oracle_valid
            || !op.request.is_object()
            || json_bytes(&op.request)?.len() > args.request_cap
            || !op.sdk_argv.is_empty() && op.channel != "sdk"
            || !op.projection_argv.is_empty() && op.channel != "projection"
        {
            return Err("postload check requires one bounded exact response oracle and channel-specific argv".into());
        }
        match op.channel.as_str() {
            "mcp" => {
                needs_mcp = true;
                if (mcp_source_text_search_operation(op) && op.expected_status != 200)
                    || !(200..300).contains(&op.expected_status)
                    || op.request["jsonrpc"] != "2.0"
                    || op.request.get("method").and_then(Value::as_str).is_none()
                    || op.request.get("id").is_none_or(Value::is_null)
                    || !op.sdk_argv.is_empty()
                    || !op.projection_argv.is_empty()
                {
                    return Err("postload MCP check must be one bounded JSON-RPC request".into());
                }
                if mcp_source_text_search_operation(op) && !valid_mcp_source_text_search_request(op)
                {
                    return Err(
                        "postload PG source-text MCP request has invalid query arguments".into(),
                    );
                }
            }
            "sdk" => {
                let invocation_args = op
                    .sdk_argv
                    .windows(2)
                    .filter(|pair| pair[0] == "--invocation")
                    .collect::<Vec<_>>();
                let query_source_read = selected_query_source_read(op, args);
                let pg_projection_source_read = selected_native_pg_source_projection_read(op, args);
                let command_id = op
                    .request
                    .get("command_id")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty() && value.len() <= 128);
                if op.sdk_argv.is_empty()
                    || op.sdk_argv.len() > 32
                    || op.sdk_argv[0].is_empty()
                    || (op.expected_status != 0 && !postload_projection_rights_denial(op))
                    || !Path::new(&op.sdk_argv[0]).is_absolute()
                    || (!query_source_read
                        && !pg_projection_source_read
                        && !op.sdk_argv.iter().any(|arg| arg == "source-commands"))
                    || (source_read_operation(op)
                        && !(query_source_read || pg_projection_source_read))
                    || (query_source_read && !exact_source_read_request(&op.request))
                    || (pg_projection_source_read
                        && (op.expected_status != 0
                            || op.expected_stderr_prefix.is_some()
                            || !op.alternate_outcomes.is_empty()))
                    || !op.projection_argv.is_empty()
                    || ((!query_source_read && !pg_projection_source_read)
                        && invocation_args.len() != 1)
                    || ((query_source_read || pg_projection_source_read)
                        && !invocation_args.is_empty())
                    || (!query_source_read
                        && !pg_projection_source_read
                        && !read_only_describe_operation(op)
                        && (command_id.is_none()
                            || !(source_read_operation(op)
                                || postload_rights_revoke_operation(op))))
                    || (query_source_read && command_id.is_some())
                    || (read_only_describe_operation(op) && command_id.is_some())
                {
                    return Err(
                        "postload SDK checks require the selected exact-source reader or an explicit native owner invocation"
                            .into(),
                    );
                }
                if let Some(command_id) = command_id {
                    if !command_ids.insert(command_id.to_owned()) {
                        return Err("postload source-read command IDs must be unique".into());
                    }
                }
                if postload_rights_revoke_operation(op) {
                    let invocation_path = PathBuf::from(
                        invocation_argument(op).ok_or("postload rights invocation absent")?,
                    )
                    .canonicalize()
                    .map_err(|e| format!("postload rights invocation path: {e}"))?;
                    let invocation = read_stage_json(&invocation_path)?;
                    if invocation["schema_version"] != "tos_local_pg_rights_revoke_invocation_v1"
                        || op.request["expected_configuration"] != Value::Null
                        || op.request["expected_rights_version"] != Value::Null
                    {
                        return Err("postload rights.revoke requires the ordinary PostgreSQL rights invocation with dynamic describe binding".into());
                    }
                    if pg_projection_source_read {
                        if !native_pg_stage {
                            return Err("postload SDK source projection exact-source is permitted only in a selected native PostgreSQL stage".into());
                        }
                        validate_projection_operation(op, args, &["exact-source"])?;
                    }
                }
                if postload_rights_check(op) {
                    let invocation_path = PathBuf::from(
                        invocation_argument(op)
                            .ok_or("postload rights describe invocation absent")?,
                    )
                    .canonicalize()
                    .map_err(|e| format!("postload rights describe invocation path: {e}"))?;
                    let invocation = read_stage_json(&invocation_path)?;
                    if invocation["schema_version"] != "tos_local_pg_rights_revoke_invocation_v1"
                        || !op.bind_expected_configuration_from_owner
                    {
                        return Err("fresh PostgreSQL rights describes must bind the selected owner configuration".into());
                    }
                }
                for arg in &op.sdk_argv {
                    if arg.len() > 4096 {
                        return Err("postload SDK argv element exceeds4096 bytes".into());
                    }
                }
            }
            "projection" => {
                if op.expected_status != 0 && !postload_projection_rights_denial(op) {
                    return Err(
                        "postload projection checks require success or the declared rights refusal"
                            .into(),
                    );
                }
                validate_projection_operation(
                    op,
                    args,
                    &["status", "search", "search-text", "exact-source"],
                )?;
                if !op.sdk_argv.is_empty() {
                    return Err("postload projection checks cannot carry SDK argv".into());
                }
            }
            _ => return Err("postload channel must be mcp, sdk or read-only projection".into()),
        }
        if postload_exact_source_integrity(op) {
            postload_integrity_checks += 1;
            postload_integrity_positions.push(postload_index);
        }
        if postload_selected_source_integrity(op) {
            postload_selected_integrity_checks += 1;
            postload_selected_integrity_positions.push(postload_index);
        }
        if postload_pg_source_integrity(op) {
            postload_pg_integrity_checks += 1;
            postload_pg_integrity_positions.push(postload_index);
        }
        if postload_rights_check(op) {
            let allowed = op
                .expected_rights_allowed
                .ok_or("postload rights check must declare rights_allowed")?;
            if !allowed {
                let current = postload_pre_revoke_describes
                    .values()
                    .next()
                    .ok_or("revoked rights describe has no preceding rights-enabled describe")?;
                if invocation_argument(current) != invocation_argument(op)
                    || current.sdk_argv != op.sdk_argv
                {
                    return Err("pre- and post-revocation rights describes must use the same protected native invocation".into());
                }
            }
            postload_rights_check_positions.push((postload_index, allowed));
            if allowed {
                postload_pre_revoke_describes.insert(op.label.clone(), op.clone());
            }
            postload_rights_checks += 1;
        }
    }
    if native_pg_stage {
        let first_rights_describe = postload_rights_check_positions
            .first()
            .map(|(index, _)| *index)
            .unwrap_or(usize::MAX);
        let rights_order_valid = postload_rights_check_positions.len() == 2
            && postload_rights_check_positions[0].1
            && !postload_rights_check_positions[1].1
            && postload_rights_revoke_positions.len() == 1
            && postload_rights_denial_positions.len() == 1
            && postload_rights_check_positions[0].0 < postload_rights_revoke_positions[0]
            && postload_rights_revoke_positions[0] < postload_rights_check_positions[1].0
            && postload_rights_check_positions[1].0 < postload_rights_denial_positions[0];
        let integrity_before_rights = postload_integrity_positions
            .iter()
            .any(|index| *index < first_rights_describe)
            && postload_pg_integrity_positions
                .iter()
                .any(|index| *index < first_rights_describe)
            && postload_selected_integrity_checks > 0
            && postload_selected_integrity_positions
                .iter()
                .any(|index| *index < first_rights_describe)
            && postload_pg_integrity_checks > 0
            && postload_idempotency_positions.len() == 1
            && postload_idempotency_positions[0] < first_rights_describe;
        if postload_integrity_checks == 0
            || postload_selected_integrity_checks == 0
            || postload_pg_integrity_checks == 0
            || postload_idempotency_checks == 0
            || !rights_order_valid
            || !integrity_before_rights
        {
            return Err("native PostgreSQL load stages require measured-mutation replay and exact-source integrity before a postload rights describe -> revoke -> describe -> denied-projection sequence".into());
        }
    }
    let navigation_bootstrap_operations = plan
        .setup
        .iter()
        .filter(|operation| {
            operation.channel == "projection"
                && operation.request["operation"] == "bootstrap-navigation"
        })
        .collect::<Vec<_>>();
    let navigation_advance_operations = plan
        .sessions
        .iter()
        .take(args.sessions)
        .flat_map(|session| &session.operations)
        .filter(|operation| {
            operation.channel == "projection"
                && operation.request["operation"] == "advance-navigation"
        })
        .collect::<Vec<_>>();
    if navigation_bootstrap_operations.len() != navigation_bootstrap_setup_count
        || (navigation_advance_operations.is_empty() != navigation_bootstrap_operations.is_empty())
    {
        return Err(
            "scheduled navigation advancement requires exactly one one-time setup bootstrap".into(),
        );
    }
    if let Some(bootstrap) = navigation_bootstrap_operations.first() {
        let bootstrap_selection = projection_selection_path(bootstrap)?;
        for advance in &navigation_advance_operations {
            if projection_selection_path(advance)? != bootstrap_selection {
                return Err("navigation setup and measured advancement must use the same protected owner configuration".into());
            }
        }
    }
    if total == 0 || total > MAX_OPS || owner_requests + usize::from(needs_owner) > MAX_NONCES {
        return Err("bounded operation or source HTTP nonce budget exceeded".into());
    }
    let selected_operations = plan
        .setup
        .iter()
        .chain(
            plan.sessions
                .iter()
                .take(args.sessions)
                .flat_map(|session| &session.operations),
        )
        .chain(plan.postload.iter())
        .collect::<Vec<_>>();
    let needs_mcp_pg_source_text = selected_operations
        .iter()
        .any(|operation| mcp_source_text_search_operation(operation));
    let needs_mcp_prepared_owner = selected_operations.iter().any(|operation| {
        operation.channel == "mcp" && !mcp_source_text_search_operation(operation)
    });
    let needs_mcp_source_reader = selected_operations.iter().any(|operation| {
        operation.channel == "mcp"
            && source_read_operation(operation)
            && !mcp_source_text_search_operation(operation)
    });
    if needs_mcp_pg_source_text {
        if args.pg_stage_mode != PgStageMode::Existing
            || args.source_text_search_selection.is_none()
            || std::env::var_os("TOS_SOURCE_PG_URL").is_none_or(|value| value.is_empty())
        {
            return Err("tos_source_text_search requires --pg-stage-mode existing, an explicit --source-text-search-selection and TOS_SOURCE_PG_URL".into());
        }
    } else if args.source_text_search_selection.is_some() {
        return Err("--source-text-search-selection is unused by the selected schedule".into());
    }
    if let Some(selection_path) = args.source_text_search_selection.as_ref() {
        let metadata = fs::symlink_metadata(selection_path)
            .map_err(|e| format!("PG source-text selection metadata: {e}"))?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.uid() != unsafe { libc::geteuid() }
            || metadata.mode() & 0o077 != 0
        {
            return Err("PG source-text selection must be a private owned regular file".into());
        }
        if read_stage_json(selection_path)?.as_object().is_none() {
            return Err("PG source-text selection must be a JSON object".into());
        }
    }
    if needs_mcp_prepared_owner && (args.model.is_none() || args.binding.is_none()) {
        return Err("non-search MCP operations require --model and --binding".into());
    }
    if needs_mcp_source_reader && (args.source_root.is_none() || args.source_inputs.is_none()) {
        return Err("MCP source-reader schedule requires --source-root and --source-inputs".into());
    }
    if needs_owner
        && (args.owner_binary.is_none()
            || args.owner_config.is_none()
            || args.invocation.is_none()
            || args.token_file.is_none())
    {
        return Err("owner_http schedule requires --owner-binary, --owner-config, --invocation and --token-file".into());
    }
    for (_, group) in conflicts {
        let mut declared = group
            .first()
            .map(|entry| entry.2.clone())
            .ok_or("empty conflict group")?;
        declared.sort_by_key(|value| value.status);
        let channel = group
            .first()
            .map(|entry| entry.1.as_str())
            .ok_or("empty conflict group")?;
        let has_success = if channel == "sdk" {
            declared.iter().filter(|v| v.status == 0).count() == 1
        } else {
            declared
                .iter()
                .filter(|v| (200..300).contains(&v.status))
                .count()
                == 1
        };
        let has_conflict = if channel == "sdk" {
            declared.iter().filter(|v| sdk_conflict_outcome(v)).count() == 1
        } else {
            declared.iter().filter(|v| v.status == 409).count() == 1
        };
        let distinct_statuses = declared
            .iter()
            .map(|v| v.status)
            .collect::<BTreeSet<_>>()
            .len()
            == declared.len();
        if group.len() < 2
            || group.iter().map(|v| v.0).collect::<BTreeSet<_>>().len() < 2
            || group.iter().any(|v| v.1 != channel)
            || !has_success
            || !has_conflict
            || !distinct_statuses
            || group.iter().any(|entry| entry.2 != declared)
        {
            return Err("each conflict group must race distinct sessions with the same exact success/conflict outcomes on one channel".into());
        }
    }
    let interruption_slots = plan
        .sessions
        .iter()
        .take(args.sessions)
        .enumerate()
        .flat_map(|(session_index, session)| {
            session
                .operations
                .iter()
                .enumerate()
                .filter(|(_, operation)| operation.interrupt_resume_after_ms.is_some())
                .map(move |(step, _)| (session_index, step))
        })
        .collect::<Vec<_>>();
    if interruption_slots.len() > 1 {
        return Err("one schedule may contain only one durable projection interruption".into());
    }
    if let Some((interrupt_session, interrupt_step)) = interruption_slots.first().copied() {
        if plan
            .sessions
            .iter()
            .take(args.sessions)
            .enumerate()
            .any(|(session_index, session)| {
                session
                    .operations
                    .get(interrupt_step)
                    .is_some_and(|operation| {
                        operation.channel == "projection"
                            && (session_index != interrupt_session
                                || operation.interrupt_resume_after_ms.is_none())
                    })
            })
        {
            return Err(
                "projection interruption must run without another projection process in its wave"
                    .into(),
            );
        }
    }
    let report_floor = total
        .checked_mul(2048)
        .and_then(|v| v.checked_add(args.sessions * 512))
        .and_then(|v| v.checked_add(8192))
        .ok_or("report bound overflow")?;
    if report_floor > args.output_cap {
        return Err("output cap is below the schedule-derived JSONL bound".into());
    }
    Ok((total, needs_mcp, needs_owner))
}
fn remaining(deadline: Instant) -> Result<Duration, String> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|d| !d.is_zero())
        .ok_or_else(|| "whole workload deadline/cancellation".into())
}
fn spawn_group(
    command: &mut Command,
    pids: &Arc<Mutex<Vec<i32>>>,
    deadline: Instant,
) -> Result<ManagedChild, String> {
    let child = unsafe { command.process_group(0) }
        .spawn()
        .map_err(|e| format!("child start: {e}"))?;
    let mut managed = ManagedChild {
        child,
        registry: Arc::clone(pids),
        reaped: false,
    };
    let pid = managed.id() as i32;
    if let Ok(mut registry) = pids.lock() {
        registry.push(pid);
    } else {
        return Err("child registry poisoned".into());
    }
    if STOP.load(Ordering::Acquire) || Instant::now() >= deadline {
        return Err("whole workload deadline/cancellation before child registration".into());
    }
    Ok(managed)
}
fn kill_group(pid: i32, signal: i32) {
    let _ = signal_group(pid, signal);
}

fn signal_group(pid: i32, signal: i32) -> io::Result<()> {
    unsafe {
        if libc::kill(-pid, signal) == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
}

fn idempotency_replay_flags_are_true(op: &Operation, response: &Value) -> bool {
    matches!(operation_name(op), Some("source.create" | "record.revise"))
        && IDEMPOTENCY_REPLAY_JSON_PATHS
            .iter()
            .all(|path| response.pointer(path) == Some(&Value::Bool(true)))
}
fn owns_listener(pid: u32, port: u16) -> bool {
    let Ok(entries) = fs::read_dir(format!("/proc/{pid}/fd")) else {
        return false;
    };
    let mut sockets = BTreeSet::new();
    for entry in entries.flatten() {
        if let Ok(link) = fs::read_link(entry.path()) {
            let text = link.to_string_lossy();
            if let Some(inode) = text
                .strip_prefix("socket:[")
                .and_then(|s| s.strip_suffix(']'))
            {
                sockets.insert(inode.to_owned());
            }
        }
    }
    let Ok(tcp) = fs::read_to_string("/proc/net/tcp") else {
        return false;
    };
    let expected = format!("0100007F:{port:04X}");
    tcp.lines().skip(1).any(|row| {
        let fields = row.split_whitespace().collect::<Vec<_>>();
        fields.len() > 9
            && fields[1] == expected
            && fields[3] == "0A"
            && sockets.contains(fields[9])
    })
}
fn wait_listener(child: &mut ManagedChild, port: u16, deadline: Instant) -> Result<(), String> {
    while Instant::now() < deadline && !STOP.load(Ordering::Acquire) {
        if child
            .try_wait()
            .map_err(|e| format!("listener status: {e}"))?
            .is_some()
        {
            return Err("selected Rust listener exited before readiness".into());
        }
        if owns_listener(child.id(), port) {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(20));
    }
    Err("selected Rust listener readiness deadline".into())
}
fn http_connect(port: u16, deadline: Instant) -> Result<TcpStream, String> {
    let timeout = remaining(deadline)?.min(Duration::from_secs(30));
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let stream =
        TcpStream::connect_timeout(&addr, timeout).map_err(|e| format!("loopback connect: {e}"))?;
    stream
        .set_read_timeout(Some(remaining(deadline)?.min(Duration::from_secs(30))))
        .map_err(|e| format!("read timeout: {e}"))?;
    stream
        .set_write_timeout(Some(remaining(deadline)?.min(Duration::from_secs(30))))
        .map_err(|e| format!("write timeout: {e}"))?;
    Ok(stream)
}
fn read_line_bounded(stream: &mut TcpStream, cap: usize) -> Result<Vec<u8>, String> {
    let mut line = Vec::new();
    let mut byte = [0u8; 1];
    while line.len() < cap {
        let n = stream
            .read(&mut byte)
            .map_err(|e| format!("HTTP response read: {e}"))?;
        if n == 0 {
            return Err("HTTP response ended before header line".into());
        }
        line.push(byte[0]);
        if byte[0] == b'\n' {
            return Ok(line);
        }
    }
    Err("HTTP header line exceeds cap".into())
}
fn http_exchange(
    port: u16,
    method: &str,
    path: &str,
    body: &[u8],
    headers: &[(&str, String)],
    cap: usize,
    deadline: Instant,
    discard_body: bool,
) -> Result<HttpReply, String> {
    if body.len() > cap || !path.starts_with('/') || path.contains(['\r', '\n', ' ']) {
        return Err("HTTP request outside bounded local grammar".into());
    }
    let mut stream = http_connect(port, deadline)?;
    let mut head = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\nContent-Length: {}\r\n",
        body.len()
    );
    for (key, value) in headers {
        if value.contains(['\r', '\n']) {
            return Err("HTTP header value contains newline".into());
        }
        head.push_str(key);
        head.push_str(": ");
        head.push_str(value);
        head.push_str("\r\n");
    }
    head.push_str("\r\n");
    stream
        .write_all(head.as_bytes())
        .and_then(|_| stream.write_all(body))
        .map_err(|e| format!("HTTP request write: {e}"))?;
    let mut used = 0usize;
    let status_line = read_line_bounded(&mut stream, 8192)?;
    used += status_line.len();
    let status_text = std::str::from_utf8(&status_line)
        .map_err(|_| "invalid HTTP status encoding")?
        .trim_end();
    let mut status_parts = status_text.split_whitespace();
    if status_parts.next() != Some("HTTP/1.1") {
        return Err("invalid HTTP status line".into());
    }
    let status = status_parts
        .next()
        .ok_or("HTTP status absent")?
        .parse::<u16>()
        .map_err(|_| "invalid HTTP status")?;
    let mut response_headers = BTreeMap::new();
    loop {
        let line = read_line_bounded(&mut stream, 8192)?;
        used = used.checked_add(line.len()).ok_or("HTTP header overflow")?;
        if used > 65536 {
            return Err("HTTP response headers exceed65536 bytes".into());
        }
        if line == b"\r\n" || line == b"\n" {
            break;
        }
        let text = std::str::from_utf8(&line)
            .map_err(|_| "invalid HTTP header encoding")?
            .trim_end();
        let (name, value) = text.split_once(':').ok_or("invalid HTTP response header")?;
        let key = name.to_ascii_lowercase();
        if response_headers
            .insert(key, value.trim().to_owned())
            .is_some()
        {
            return Err("duplicate HTTP response header".into());
        }
        if response_headers.len() > 100 {
            return Err("HTTP response header count exceeds100".into());
        }
    }
    if response_headers.contains_key("transfer-encoding") {
        return Err("chunked/transfer-encoded response is unsupported".into());
    }
    let length = response_headers
        .get("content-length")
        .ok_or("HTTP content length absent")?
        .parse::<usize>()
        .map_err(|_| "invalid HTTP content length")?;
    if length > cap {
        return Err("HTTP response exceeds explicit cap".into());
    }
    if discard_body {
        stream.shutdown(std::net::Shutdown::Both).ok();
        return Ok(HttpReply {
            status,
            headers: response_headers,
            body: Vec::new(),
        });
    }
    let mut response = vec![0u8; length];
    stream
        .read_exact(&mut response)
        .map_err(|e| format!("HTTP response body: {e}"))?;
    Ok(HttpReply {
        status,
        headers: response_headers,
        body: response,
    })
}
fn signature_fields(fields: &Value) -> Result<Vec<u8>, String> {
    // Match source_command_http's existing Python ensure_ascii=True wire contract.
    let value = serde_json::to_string(fields).map_err(|_| "signature JSON serialization failed")?;
    let mut out = Vec::with_capacity(value.len());
    for ch in value.chars() {
        if ch < '\u{007f}' {
            out.push(ch as u8);
        } else {
            for unit in ch.encode_utf16(&mut [0; 2]).iter() {
                out.extend_from_slice(format!("\\u{unit:04x}").as_bytes());
            }
        }
    }
    Ok(out)
}
fn request_mac(
    secret: &[u8],
    method: &str,
    path: &str,
    timestamp: &str,
    nonce: &str,
    body: &[u8],
) -> Result<String, String> {
    let digest = sha(body);
    let fields = json!(["tos-request-v1", method, path, timestamp, nonce, digest]);
    let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC accepts 32-byte key");
    mac.update(&signature_fields(&fields)?);
    Ok(hex(&mac.finalize().into_bytes()))
}
fn response_authenticated(
    secret: &[u8],
    nonce: &str,
    status: u16,
    body: &[u8],
    proof: Option<&String>,
) -> bool {
    let Some(proof) = proof else {
        return false;
    };
    let Ok(bytes) = unhex(proof) else {
        return false;
    };
    let fields = json!(["tos-response-v1", nonce, status, sha(body)]);
    let Ok(encoded) = signature_fields(&fields) else {
        return false;
    };
    let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC accepts 32-byte key");
    mac.update(&encoded);
    mac.verify_slice(&bytes).is_ok()
}
fn fresh_nonce(used: &Mutex<BTreeSet<String>>) -> Result<String, String> {
    for _ in 0..4 {
        let mut raw = [0u8; 32];
        File::open("/dev/urandom")
            .and_then(|mut f| f.read_exact(&mut raw))
            .map_err(|e| format!("nonce source: {e}"))?;
        let nonce = hex(&raw);
        let mut seen = used.lock().map_err(|_| "nonce registry poisoned")?;
        if seen.insert(nonce.clone()) {
            return Ok(nonce);
        }
    }
    Err("nonce collision retry exhausted".into())
}
fn owner_http(
    port: u16,
    token: &[u8],
    nonces: &Mutex<BTreeSet<String>>,
    body: &[u8],
    deadline: Instant,
    request_cap: usize,
    response_cap: usize,
    drop_body: bool,
) -> Result<Attempt, String> {
    let started = Instant::now();
    if body.len() > request_cap || body.len() > 1_048_576 {
        return Err("source command request exceeds1MiB/selected cap".into());
    }
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "system clock before epoch")?
        .as_secs()
        .to_string();
    let nonce = fresh_nonce(nonces)?;
    let digest = sha(body);
    let proof = request_mac(token, "POST", "/commands", &timestamp, &nonce, body)?;
    let authorization = format!("ToS-HMAC-SHA256 {timestamp}:{nonce}:{digest}:{proof}");
    let headers = [
        ("Content-Type", "application/json".to_owned()),
        ("Authorization", authorization),
    ];
    let reply = http_exchange(
        port,
        "POST",
        "/commands",
        body,
        &headers,
        response_cap,
        deadline,
        drop_body,
    )?;
    if drop_body {
        return Ok(Attempt {
            status: reply.status,
            bytes: reply
                .headers
                .get("content-length")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
            sha256: String::new(),
            body: None,
            stderr: Vec::new(),
            elapsed_ms: started.elapsed().as_secs_f64() * 1000.0,
            authenticated: Some(false),
            interrupted: false,
        });
    }
    let authenticated = response_authenticated(
        token,
        &nonce,
        reply.status,
        &reply.body,
        reply.headers.get("x-tos-response-signature"),
    );
    Ok(Attempt {
        status: reply.status,
        bytes: reply.body.len(),
        sha256: sha(&reply.body),
        body: Some(reply.body),
        stderr: Vec::new(),
        elapsed_ms: started.elapsed().as_secs_f64() * 1000.0,
        authenticated: Some(authenticated),
        interrupted: false,
    })
}
fn mcp_http(
    port: u16,
    session: Option<&str>,
    request: &Value,
    deadline: Instant,
    cap: usize,
    method: &str,
) -> Result<HttpReply, String> {
    let body = json_bytes(request)?;
    if body.len() > 65536 {
        return Err("MCP request exceeds64KiB".into());
    }
    let mut headers = vec![
        ("Content-Type", "application/json".to_owned()),
        ("Accept", "application/json, text/event-stream".to_owned()),
        ("MCP-Protocol-Version", MCP_PROTOCOL.to_owned()),
    ];
    if let Some(id) = session {
        headers.push(("MCP-Session-Id", id.to_owned()));
    }
    let refs = headers
        .iter()
        .map(|(k, v)| (*k, v.clone()))
        .collect::<Vec<_>>();
    http_exchange(port, method, "/mcp", &body, &refs, cap, deadline, false)
}
fn initialize_mcp(
    port: u16,
    deadline: Instant,
    cap: usize,
) -> Result<(McpSession, u16, String, f64), String> {
    let request = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":MCP_PROTOCOL,"capabilities":{},"clientInfo":{"name":"tos-load-readiness","version":"1"}}});
    let started = Instant::now();
    let reply = mcp_http(port, None, &request, deadline, cap, "POST")?;
    let digest = sha(&reply.body);
    if reply.status != 200 {
        return Ok((
            McpSession::default(),
            reply.status,
            digest,
            started.elapsed().as_secs_f64() * 1000.0,
        ));
    }
    let session = reply
        .headers
        .get("mcp-session-id")
        .ok_or("MCP initialize omitted session ID")?
        .clone();
    let parsed: Value =
        serde_json::from_slice(&reply.body).map_err(|_| "MCP initialize response is not JSON")?;
    if parsed["id"] != 1 || parsed["result"]["protocolVersion"] != MCP_PROTOCOL {
        return Err("MCP initialize response differs from 2025-11-25 contract".into());
    }
    let initialized = json!({"jsonrpc":"2.0","method":"notifications/initialized"});
    let notification = mcp_http(port, Some(&session), &initialized, deadline, cap, "POST")?;
    if notification.status != 202 {
        return Err(format!(
            "MCP initialized notification status {}",
            notification.status
        ));
    }
    Ok((
        McpSession { id: Some(session) },
        reply.status,
        digest,
        started.elapsed().as_secs_f64() * 1000.0,
    ))
}
fn delete_mcp(port: u16, id: &str, deadline: Instant, cap: usize) -> Result<(), String> {
    let refs = [("MCP-Session-Id", id.to_owned())];
    let reply = http_exchange(port, "DELETE", "/mcp", &[], &refs, cap, deadline, false)?;
    if reply.status != 200 {
        return Err(format!("MCP session close status {}", reply.status));
    }
    Ok(())
}
fn native_command_process(
    argv: &[String],
    request: &Value,
    work_dir: &Path,
    deadline: Instant,
    request_cap: usize,
    response_cap: usize,
    pids: &Arc<Mutex<Vec<i32>>>,
    discard_response: bool,
    interrupt_after: Option<Duration>,
) -> Result<Attempt, String> {
    let started = Instant::now();
    let binary = PathBuf::from(&argv[0]);
    let mut input = json_bytes(request)?;
    input.push(b'\n');
    if input.len() > request_cap {
        return Err("SDK input exceeds request cap".into());
    }
    let mut command = Command::new(&binary);
    command
        .args(&argv[1..])
        .current_dir(work_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = spawn_group(&mut command, pids, deadline)?;
    let pid = child.id() as i32;
    let mut stdin = child.stdin.take().ok_or("SDK stdin unavailable")?;
    let mut stdout = child.stdout.take().ok_or("SDK stdout unavailable")?;
    let mut stderr = child.stderr.take().ok_or("SDK stderr unavailable")?;
    // Drain SDK output while stdin is being written; either pipe alone can fill
    // and deadlock an otherwise valid full-duplex client exchange.
    let (output_tx, output_rx) = mpsc::sync_channel(1);
    let reader = thread::Builder::new()
        .stack_size(128 * 1024)
        .spawn(move || {
            let mut output = Vec::new();
            let result = stdout
                .take(response_cap as u64 + 1)
                .read_to_end(&mut output)
                .map_err(|e| e.to_string());
            if output.len() > response_cap {
                kill_group(pid, libc::SIGKILL);
            }
            let _ = output_tx.send((output, result));
        })
        .map_err(|e| format!("SDK response drain start: {e}"))?;
    let (stderr_tx, stderr_rx) = mpsc::sync_channel(1);
    let stderr_reader = thread::Builder::new()
        .stack_size(128 * 1024)
        .spawn(move || {
            let mut diagnostic = Vec::new();
            let result = stderr
                .take(NATIVE_STDERR_CAP as u64 + 1)
                .read_to_end(&mut diagnostic)
                .map_err(|e| e.to_string());
            if diagnostic.len() > NATIVE_STDERR_CAP {
                kill_group(pid, libc::SIGKILL);
            }
            let _ = stderr_tx.send((diagnostic, result));
        })
        .map_err(|e| format!("SDK diagnostic drain start: {e}"))?;
    let write_result = stdin.write_all(&input).map_err(|e| e.to_string());
    drop(stdin);
    if let Err(error) = write_result {
        kill_group(pid, libc::SIGKILL);
        let _ = child.wait();
        let _ = reader.join();
        let _ = stderr_reader.join();
        return Err(format!(
            "SDK request pipe: {error}; process group terminated"
        ));
    }
    let interrupt_at = interrupt_after.map(|delay| Instant::now() + delay);
    let mut captured_output = None;
    let mut captured_stderr = None;
    let mut interrupted = false;
    let status = loop {
        if STOP.load(Ordering::Acquire) || Instant::now() >= deadline {
            kill_group(pid, libc::SIGKILL);
            let _ = child.wait();
            let _ = reader.join();
            let _ = stderr_reader.join();
            return Err("native process deadline; process group terminated".into());
        }
        if captured_output.is_none() {
            if let Ok(captured) = output_rx.try_recv() {
                captured_output = Some(captured);
            }
        }
        if captured_stderr.is_none() {
            if let Ok(captured) = stderr_rx.try_recv() {
                captured_stderr = Some(captured);
            }
        }
        if let Some(status) = child
            .try_wait()
            .map_err(|e| format!("native process status: {e}"))?
        {
            if interrupt_after.is_some() {
                kill_group(pid, libc::SIGKILL);
                let _ = reader.join();
                let _ = stderr_reader.join();
                return Err("projection apply completed before the requested interruption".into());
            }
            break status;
        }
        if interrupt_at.is_some_and(|at| Instant::now() >= at) {
            signal_group(pid, libc::SIGKILL)
                .map_err(|e| format!("projection process-group SIGKILL delivery: {e}"))?;
            let status = child
                .wait()
                .map_err(|e| format!("interrupted projection process wait: {e}"))?;
            if status.signal() != Some(libc::SIGKILL) {
                let _ = reader.join();
                let _ = stderr_reader.join();
                return Err("projection leader did not exit from the requested SIGKILL".into());
            }
            interrupted = true;
            break status;
        }
        thread::sleep(Duration::from_millis(10));
    };
    if captured_output.is_none() {
        captured_output = Some(
            output_rx
                .recv_timeout(remaining(deadline)?)
                .map_err(|e| format!("native process response drain: {e}"))?,
        );
    }
    if captured_stderr.is_none() {
        captured_stderr = Some(
            stderr_rx
                .recv_timeout(remaining(deadline)?)
                .map_err(|e| format!("native process diagnostic drain: {e}"))?,
        );
    }
    reader
        .join()
        .map_err(|_| "native response drain panicked")?;
    stderr_reader
        .join()
        .map_err(|_| "native diagnostic drain panicked")?;
    let (output, read_result) = captured_output.ok_or("native response drain absent")?;
    let (diagnostic, diagnostic_result) =
        captured_stderr.ok_or("native diagnostic drain absent")?;
    if STOP.load(Ordering::Acquire) || Instant::now() >= deadline {
        kill_group(pid, libc::SIGKILL);
        return Err("native process response exceeded the workload deadline".into());
    }
    read_result.map_err(|e| format!("native response read: {e}"))?;
    diagnostic_result.map_err(|e| format!("native diagnostic read: {e}"))?;
    if output.len() > response_cap {
        kill_group(pid, libc::SIGKILL);
        return Err("native response exceeds response cap; process group terminated".into());
    }
    if diagnostic.len() > NATIVE_STDERR_CAP {
        kill_group(pid, libc::SIGKILL);
        return Err("native diagnostics exceed 4096-byte cap; process group terminated".into());
    }
    Ok(Attempt {
        status: status.code().unwrap_or(255) as u16,
        bytes: output.len(),
        sha256: sha(&output),
        body: (!discard_response && !interrupted).then_some(output),
        stderr: diagnostic,
        elapsed_ms: started.elapsed().as_secs_f64() * 1000.0,
        authenticated: None,
        interrupted,
    })
}

fn sdk_process(
    op: &Operation,
    work_dir: &Path,
    deadline: Instant,
    request_cap: usize,
    response_cap: usize,
    pids: &Arc<Mutex<Vec<i32>>>,
    discard_response: bool,
) -> Result<Attempt, String> {
    // Drop the completed first response at the harness boundary, then issue
    // the same command in a new process so the owner exercises its replay path.
    native_command_process(
        &op.sdk_argv,
        &op.request,
        work_dir,
        deadline,
        request_cap,
        response_cap,
        pids,
        discard_response,
        None,
    )
}

fn projection_process(
    op: &Operation,
    work_dir: &Path,
    deadline: Instant,
    request_cap: usize,
    response_cap: usize,
    pids: &Arc<Mutex<Vec<i32>>>,
    interrupt_after: Option<Duration>,
) -> Result<Attempt, String> {
    native_command_process(
        &op.projection_argv,
        &op.request,
        work_dir,
        deadline,
        request_cap,
        response_cap,
        pids,
        false,
        interrupt_after,
    )
}

#[derive(Clone, Copy)]
enum ProjectionCursor {
    Current {
        through_seq: u64,
    },
    Pending {
        through_seq: u64,
        head_seq: u64,
    },
    Stale {
        through_seq: Option<u64>,
        head_seq: u64,
    },
}

impl ProjectionCursor {
    fn observation(self) -> Value {
        match self {
            Self::Current { through_seq } => json!({
                "status":"Current",
                "through_seq":through_seq,
                "head_seq":through_seq
            }),
            Self::Pending {
                through_seq,
                head_seq,
            } => json!({
                "status":"Pending",
                "through_seq":through_seq,
                "head_seq":head_seq
            }),
            Self::Stale {
                through_seq,
                head_seq,
            } => json!({
                "status":"Stale",
                "through_seq":through_seq,
                "head_seq":head_seq
            }),
        }
    }

    fn pending(self) -> Option<(u64, u64)> {
        match self {
            Self::Pending {
                through_seq,
                head_seq,
            } => Some((through_seq, head_seq)),
            _ => None,
        }
    }

    fn current(self) -> Option<u64> {
        match self {
            Self::Current { through_seq } => Some(through_seq),
            _ => None,
        }
    }
}

fn projection_status(
    op: &Operation,
    work_dir: &Path,
    deadline: Instant,
    request_cap: usize,
    response_cap: usize,
    pids: &Arc<Mutex<Vec<i32>>>,
) -> Result<ProjectionCursor, String> {
    let mut status_op = op.clone();
    status_op.request = json!({"operation":"status"});
    status_op.interrupt_resume_after_ms = None;
    let attempt = projection_process(
        &status_op,
        work_dir,
        deadline,
        request_cap,
        response_cap,
        pids,
        None,
    )?;
    if attempt.status != 0 || attempt.interrupted {
        return Err("projection cursor status command did not complete".into());
    }
    let body = attempt
        .body
        .as_deref()
        .ok_or("projection cursor status response absent")?;
    let response: Value = serde_json::from_slice(body)
        .map_err(|_| "projection cursor status response is not JSON")?;
    if response["schema_version"] != "tos_source_projection_status_v1" {
        return Err("projection cursor status response schema changed".into());
    }
    let status = response
        .get("result")
        .and_then(Value::as_object)
        .ok_or("projection cursor status result absent")?;
    if status.len() != 1 {
        return Err("projection cursor status variant changed".into());
    }
    let (name, value) = status
        .iter()
        .next()
        .ok_or("projection cursor status variant absent")?;
    match name.as_str() {
        "Current" => Ok(ProjectionCursor::Current {
            through_seq: value["through_seq"]
                .as_u64()
                .ok_or("projection current cursor absent")?,
        }),
        "Pending" => Ok(ProjectionCursor::Pending {
            through_seq: value["through_seq"]
                .as_u64()
                .ok_or("projection pending cursor absent")?,
            head_seq: value["head_seq"]
                .as_u64()
                .ok_or("projection pending head absent")?,
        }),
        "Stale" => Ok(ProjectionCursor::Stale {
            through_seq: value["through_seq"].as_u64(),
            head_seq: value["head_seq"]
                .as_u64()
                .ok_or("projection stale head absent")?,
        }),
        _ => Err("projection cursor status variant changed".into()),
    }
}

fn projection_queue_age(
    op: &Operation,
    work_dir: &Path,
    deadline: Instant,
    request_cap: usize,
    response_cap: usize,
    pids: &Arc<Mutex<Vec<i32>>>,
) -> Result<Value, String> {
    let mut queue_age_op = op.clone();
    queue_age_op.request = json!({"operation":"queue-age"});
    queue_age_op.interrupt_resume_after_ms = None;
    let attempt = projection_process(
        &queue_age_op,
        work_dir,
        deadline,
        request_cap,
        response_cap,
        pids,
        None,
    )?;
    if attempt.status != 0 || attempt.interrupted {
        return Err("projection queue-age command did not complete".into());
    }
    let body = attempt
        .body
        .as_deref()
        .ok_or("projection queue-age response absent")?;
    let response: Value =
        serde_json::from_slice(body).map_err(|_| "projection queue-age response is not JSON")?;
    if response["schema_version"] != "tos_source_projection_queue_age_v1"
        || response.get("profile").is_none()
    {
        return Err("projection queue-age response schema or profile changed".into());
    }
    let result = response
        .get("result")
        .and_then(Value::as_object)
        .ok_or("projection queue-age result absent")?;
    let status = result
        .get("status")
        .and_then(Value::as_str)
        .ok_or("projection queue-age status absent")?;
    if !matches!(status, "Current" | "Pending" | "Stale") {
        return Err("projection queue-age cursor status changed".into());
    }
    result
        .get("head_seq")
        .and_then(Value::as_u64)
        .ok_or("projection queue-age head sequence absent")?;
    result
        .get("sequence_lag")
        .and_then(Value::as_u64)
        .ok_or("projection queue-age sequence lag absent")?;
    let age_status = result
        .get("oldest_unprocessed_age_status")
        .and_then(Value::as_str)
        .ok_or("projection queue-age age status absent")?;
    if !matches!(
        age_status,
        "empty" | "known" | "unknown_historical" | "future_clock"
    ) {
        return Err("projection queue-age timestamp status changed".into());
    }
    let nullable_u64 = |key: &str| -> Result<Option<u64>, String> {
        match result.get(key) {
            Some(value) if value.is_null() => Ok(None),
            Some(value) => value
                .as_u64()
                .map(Some)
                .ok_or_else(|| format!("projection queue-age {key} is invalid")),
            None => Err(format!("projection queue-age {key} absent")),
        }
    };
    let through_seq = nullable_u64("through_seq")?;
    let oldest_seq = nullable_u64("oldest_unprocessed_seq")?;
    let enqueued_at = nullable_u64("oldest_unprocessed_enqueued_at_unix_micros")?;
    let age_millis = nullable_u64("oldest_unprocessed_age_millis")?;
    match age_status {
        "empty" if oldest_seq.is_none() && enqueued_at.is_none() && age_millis.is_none() => {}
        "known" if oldest_seq.is_some() && enqueued_at.is_some() && age_millis.is_some() => {}
        "unknown_historical"
            if oldest_seq.is_some() && enqueued_at.is_none() && age_millis.is_none() => {}
        "future_clock" if oldest_seq.is_some() && enqueued_at.is_some() && age_millis.is_none() => {
        }
        _ => return Err("projection queue-age fields disagree with timestamp status".into()),
    }
    Ok(json!({
        "profile":response["profile"],
        "status":status,
        "through_seq":through_seq,
        "head_seq":result["head_seq"],
        "sequence_lag":result["sequence_lag"],
        "oldest_unprocessed_seq":oldest_seq,
        "oldest_unprocessed_enqueued_at_unix_micros":enqueued_at,
        "oldest_unprocessed_age_millis":age_millis,
        "oldest_unprocessed_age_status":age_status
    }))
}

fn navigation_response_body(attempt: Option<&Attempt>) -> Result<Value, String> {
    let attempt = attempt.ok_or("navigation command response absent")?;
    if attempt.status != 0 || attempt.interrupted {
        return Err("navigation command did not complete successfully".into());
    }
    let body = attempt
        .body
        .as_deref()
        .ok_or("navigation command response body absent")?;
    serde_json::from_slice(body).map_err(|_| "navigation command response is not JSON".into())
}

fn navigation_bootstrap_observation(
    op: &Operation,
    attempt: Option<&Attempt>,
) -> Result<Value, String> {
    let response = navigation_response_body(attempt)?;
    let selection = read_stage_json(&projection_selection_path(op)?)?;
    let selected = &selection["source_navigation"];
    let expected_revision = selected["expectation"]["membership_root"]
        .as_str()
        .ok_or("navigation source expectation membership root absent")?;
    let expected_source_membership_count =
        selected["source_proof_v2"]["generation"]["current_members"]
            .as_u64()
            .ok_or("navigation retained source membership count absent")?;
    let source_head_sequence = response["source_head_sequence"]
        .as_u64()
        .ok_or("navigation bootstrap source head absent")?;
    let through_sequence = response["through_sequence"]
        .as_u64()
        .ok_or("navigation bootstrap cursor absent")?;
    let source_membership_count = response["source_membership_count"]
        .as_u64()
        .ok_or("navigation bootstrap source member count absent")?;
    let selected_owner_source_count = response["selected_owner_source_count"]
        .as_u64()
        .ok_or("navigation bootstrap owner source count absent")?;
    let selected_owner_source_bytes = response["selected_owner_source_bytes"]
        .as_u64()
        .ok_or("navigation bootstrap owner source bytes absent")?;
    let root = response["bootstrap_root"]
        .as_str()
        .ok_or("navigation bootstrap root absent")?;
    if response["schema_version"] != "tos_source_navigation_bootstrap_result_v1"
        || response["domain"] != selected["domain"]
        || response["consumer_id"] != selected["consumer_id"]
        || response["profile"] != selected["profile_id"]
        || response["source_revision"] != expected_revision
        || source_membership_count == 0
        || source_membership_count != expected_source_membership_count
        || source_head_sequence != through_sequence
        || !valid_sha(root)
        || response["replayed"] != false
    {
        return Err(
            "navigation bootstrap did not bind the selected source and new current cursor".into(),
        );
    }
    Ok(json!({
        "profile": response["profile"],
        "source_revision": response["source_revision"],
        "source_membership_count": source_membership_count,
        "source_head_sequence": source_head_sequence,
        "through_sequence": through_sequence,
        "bootstrap_root": root,
        "selected_owner_source_count": selected_owner_source_count,
        "selected_owner_source_bytes": selected_owner_source_bytes,
        "replayed": false
    }))
}

fn navigation_advance_observation(
    op: &Operation,
    attempt: Option<&Attempt>,
) -> Result<Value, String> {
    let response = navigation_response_body(attempt)?;
    let selection = read_stage_json(&projection_selection_path(op)?)?;
    let selected = &selection["source_navigation"];
    let profile = selected["profile_id"]
        .as_str()
        .ok_or("navigation selected profile absent")?;
    let cursor_before = response["cursor_before"]
        .as_u64()
        .ok_or("navigation cursor-before sequence absent")?;
    let cursor_sequence = response["cursor_sequence"]
        .as_u64()
        .ok_or("navigation cursor sequence absent")?;
    let source_head_sequence = response["source_head_sequence"]
        .as_u64()
        .ok_or("navigation source head sequence absent")?;
    let published = response["published"]
        .as_array()
        .ok_or("navigation published ranges absent")?;
    let max_events = op.request["max_events"]
        .as_u64()
        .filter(|events| *events > 0 && *events <= 256)
        .ok_or("navigation request event bound absent")?;
    let stop = response["stop"]
        .as_str()
        .ok_or("navigation stop tag absent")?;
    if response["schema_version"] != "tos_source_navigation_advance_result_v1"
        || response["profile"] != profile
        || cursor_sequence < cursor_before
        || cursor_sequence > source_head_sequence
    {
        return Err("navigation response identity or cursor order is invalid".into());
    }

    let sequence_lag_before = source_head_sequence - cursor_before;
    let sequence_lag_after = source_head_sequence - cursor_sequence;
    let mut range_root = None::<String>;
    let mut range_event_count = None::<u64>;
    let mut replayed = false;
    let mut idle = false;
    let mut more_pending = false;
    match stop {
        "up_to_date" if published.is_empty() => {
            if response.get("pending_reason").is_some()
                || cursor_before != cursor_sequence
                || cursor_sequence != source_head_sequence
                || response.get("range_root").is_some()
            {
                return Err("navigation idle result is not current or carries range data".into());
            }
            idle = true;
        }
        "up_to_date" | "more_pending" if published.len() == 1 => {
            let work = response
                .get("work")
                .and_then(Value::as_object)
                .ok_or("navigation work counters absent")?;
            let range = &published[0];
            let range_cursor_before = range["cursor_before"]
                .as_u64()
                .ok_or("navigation published range cursor-before absent")?;
            let range_through = range["through_sequence"]
                .as_u64()
                .ok_or("navigation published range endpoint absent")?;
            let event_count = work["range_event_count"]
                .as_u64()
                .ok_or("navigation published range event count absent")?;
            for key in [
                "events",
                "cursor_lag_events",
                "range_member_count",
                "range_member_bytes",
                "terminal_selected_source_count",
                "terminal_selected_source_bytes",
                "source_members",
                "source_bytes",
                "selected_source_count",
                "selected_source_bytes",
                "record_candidates",
                "record_rows",
                "claim_streams",
                "claim_rows",
                "max_events",
            ] {
                if work[key].as_u64().is_none() {
                    return Err(format!("navigation work counter {key} absent"));
                }
            }
            let root = range["coverage_root"]
                .as_str()
                .ok_or("navigation published range coverage root absent")?;
            let terminal_event_id = range["terminal_event_id"]
                .as_str()
                .filter(|id| !id.is_empty())
                .ok_or("navigation published terminal event ID absent")?;
            let terminal_root = range["terminal_cohort_root"]
                .as_str()
                .ok_or("navigation published terminal cohort root absent")?;
            for key in [
                "record_owner_set_root",
                "claim_owner_set_root",
                "claim_range_manifest_root",
            ] {
                if !range[key].as_str().is_some_and(valid_sha) {
                    return Err(format!("navigation published {key} absent or invalid"));
                }
            }
            for key in [
                "record_replaced_owner_count",
                "claim_replaced_owner_count",
                "record_removed_owner_count",
                "claim_removed_owner_count",
                "replacement_owner_count",
            ] {
                if range[key].as_u64().is_none() {
                    return Err(format!("navigation published {key} absent"));
                }
            }
            replayed = range["replayed"]
                .as_bool()
                .ok_or("navigation published replay marker absent")?;
            let is_more_pending = stop == "more_pending";
            if range_cursor_before != cursor_before
                || range_through != cursor_sequence
                || range_through <= range_cursor_before
                || range_through - range_cursor_before != event_count
                || work["events"].as_u64() != Some(event_count)
                || event_count > max_events
                || work["cursor_lag_events"].as_u64() != Some(sequence_lag_before)
                || work["terminal_selected_source_count"].as_u64()
                    != work["selected_source_count"].as_u64()
                || work["terminal_selected_source_bytes"].as_u64()
                    != work["selected_source_bytes"].as_u64()
                || work["max_events"]
                    .as_u64()
                    .is_none_or(|events| events == 0 || events > 256)
                || (is_more_pending
                    && (range_through >= source_head_sequence
                        || response["pending_reason"] != "event_budget_exceeded"))
                || (!is_more_pending
                    && (range_through != source_head_sequence
                        || !response["pending_reason"].is_null()))
                || response["range_root"].as_str() != Some(root)
                || !valid_sha(root)
                || !valid_sha(terminal_root)
                || terminal_event_id.is_empty()
            {
                return Err(
                    "navigation publication is not a continuous bounded range at the held head"
                        .into(),
                );
            }
            range_root = Some(root.to_owned());
            range_event_count = Some(event_count);
            more_pending = is_more_pending;
        }
        _ => {
            let reason = response
                .get("pending_reason")
                .and_then(Value::as_str)
                .unwrap_or("none");
            return Err(format!(
                "navigation result has unsupported stop/publication tag {stop} ({reason})"
            ));
        }
    }
    Ok(json!({
        "profile": profile,
        "cursor_before": cursor_before,
        "cursor_sequence": cursor_sequence,
        "source_head_sequence": source_head_sequence,
        "sequence_lag_before": sequence_lag_before,
        "sequence_lag_after": sequence_lag_after,
        "advanced": cursor_sequence > cursor_before,
        "idle_current": idle,
        "more_pending_prefix": more_pending,
        "replayed": replayed,
        "published_ranges": published.len(),
        "published": published,
        "range_event_count": range_event_count,
        "work": response.get("work"),
        "range_root": range_root,
        "stop": stop,
        "pending_reason": response.get("pending_reason").and_then(Value::as_str),
        "oldest_unprocessed_age_status": "not_exposed_by_navigation_owner"
    }))
}

fn navigation_quiescent_drain(
    template: &Operation,
    args: &Args,
    pids: &Arc<Mutex<Vec<i32>>>,
    cgroup: &Path,
    resource_baseline: &ResourceSnapshot,
    deadline: Instant,
) -> Value {
    let started = Instant::now();
    let mut op = template.clone();
    op.label = "navigation-quiescent-postload-drain".into();
    op.request = json!({"operation":"advance-navigation","max_events":1});
    let mut steps = 0usize;
    let mut more_pending_prefixes = 0usize;
    let mut replayed_ranges = 0usize;
    let mut range_event_count = 0u64;
    let mut source_member_count = 0u64;
    let mut source_bytes = 0u64;
    let mut selected_source_count = 0u64;
    let mut selected_source_bytes = 0u64;
    let mut record_row_count = 0u64;
    let mut claim_row_count = 0u64;
    let mut externally_advanced_events = 0u64;
    let mut quiescent_head = None::<u64>;
    let mut initial_cursor = None::<u64>;
    let mut final_cursor = None::<u64>;
    let mut max_lag_before = 0u64;
    let mut last_response_sha256 = None::<String>;
    let mut error = None::<String>;
    let mut complete = false;

    loop {
        if steps >= MAX_OPS {
            error = Some("navigation postload drain exceeded its finite step bound".into());
            break;
        }
        if STOP.load(Ordering::Acquire) || Instant::now() >= deadline {
            error =
                Some("navigation postload drain stopped at workload deadline/cancellation".into());
            break;
        }
        let current_resources = match resource_snapshot(cgroup) {
            Ok(resources) => resources,
            Err(reason) => {
                error = Some(reason);
                break;
            }
        };
        if resource_stop_observed_since(resource_baseline, &current_resources) {
            error = Some(
                "navigation postload drain stopped at the predeclared resource threshold".into(),
            );
            break;
        }
        let attempt = match projection_process(
            &op,
            &args.output,
            deadline,
            args.request_cap,
            args.response_cap,
            pids,
            None,
        ) {
            Ok(attempt) => attempt,
            Err(reason) => {
                error = Some(reason);
                break;
            }
        };
        let observation = match navigation_advance_observation(&op, Some(&attempt)) {
            Ok(observation) => observation,
            Err(reason) => {
                error = Some(reason);
                break;
            }
        };
        steps += 1;
        last_response_sha256 = Some(attempt.sha256);
        let cursor_before = observation["cursor_before"].as_u64().unwrap_or(0);
        let cursor_sequence = observation["cursor_sequence"].as_u64().unwrap_or(0);
        let source_head_sequence = observation["source_head_sequence"].as_u64().unwrap_or(0);
        let lag_before = observation["sequence_lag_before"].as_u64().unwrap_or(0);
        let lag_after = observation["sequence_lag_after"].as_u64().unwrap_or(0);
        max_lag_before = max_lag_before.max(lag_before);
        initial_cursor.get_or_insert(cursor_before);
        if observation["more_pending_prefix"] == true {
            more_pending_prefixes += 1;
        }
        if observation["replayed"] == true {
            replayed_ranges += 1;
        }
        range_event_count = range_event_count
            .saturating_add(observation["range_event_count"].as_u64().unwrap_or(0));
        let work = &observation["work"];
        source_member_count =
            source_member_count.saturating_add(work["source_members"].as_u64().unwrap_or(0));
        source_bytes = source_bytes.saturating_add(work["source_bytes"].as_u64().unwrap_or(0));
        selected_source_count = selected_source_count
            .saturating_add(work["selected_source_count"].as_u64().unwrap_or(0));
        selected_source_bytes = selected_source_bytes
            .saturating_add(work["selected_source_bytes"].as_u64().unwrap_or(0));
        record_row_count =
            record_row_count.saturating_add(work["record_rows"].as_u64().unwrap_or(0));
        claim_row_count = claim_row_count.saturating_add(work["claim_rows"].as_u64().unwrap_or(0));

        if let Some(expected_head) = quiescent_head {
            if source_head_sequence != expected_head {
                error = Some("source head changed during the quiescent postload drain".into());
                final_cursor = Some(cursor_sequence);
                break;
            }
        } else {
            quiescent_head = Some(source_head_sequence);
        }
        if let Some(previous_cursor) = final_cursor {
            if cursor_before < previous_cursor || cursor_sequence < previous_cursor {
                error = Some("navigation postload cursor moved backwards".into());
                final_cursor = Some(cursor_sequence);
                break;
            }
            externally_advanced_events = externally_advanced_events
                .saturating_add(cursor_before.saturating_sub(previous_cursor));
            if cursor_sequence == previous_cursor && lag_after > 0 {
                error = Some("navigation postload drain made no cursor progress".into());
                break;
            }
        }
        final_cursor = Some(cursor_sequence);
        if cursor_sequence == source_head_sequence && observation["stop"] == "up_to_date" {
            complete = true;
            break;
        }
        if observation["stop"] != "more_pending" || lag_after == 0 {
            error =
                Some("navigation postload drain did not return a bounded pending prefix".into());
            break;
        }
        if cursor_sequence <= cursor_before {
            error = Some("navigation postload pending prefix did not advance its cursor".into());
            break;
        }
    }

    json!({
        "phase":"quiescent-postload-drain",
        "passed":complete && error.is_none(),
        "complete":complete,
        "steps":steps,
        "step_limit":MAX_OPS,
        "max_events_per_step":1,
        "quiescent_source_head_sequence":quiescent_head,
        "initial_cursor_sequence":initial_cursor,
        "final_cursor_sequence":final_cursor,
        "initial_sequence_lag":quiescent_head.zip(initial_cursor).map(|(head,cursor)| head.saturating_sub(cursor)),
        "remaining_sequence_lag":quiescent_head.zip(final_cursor).map(|(head,cursor)| head.saturating_sub(cursor)),
        "max_sequence_lag_before":if steps > 0 { Some(max_lag_before) } else { None },
        "more_pending_prefix_steps":more_pending_prefixes,
        "replayed_range_steps":replayed_ranges,
        "published_range_events":range_event_count,
        "published_source_members":source_member_count,
        "published_source_bytes":source_bytes,
        "selected_source_count":selected_source_count,
        "selected_source_bytes":selected_source_bytes,
        "record_rows":record_row_count,
        "claim_rows":claim_row_count,
        "concurrently_advanced_cursor_events":externally_advanced_events,
        "last_response_sha256":last_response_sha256,
        "elapsed_seconds":started.elapsed().as_secs_f64(),
        "error":error
    })
}

fn run_operation(
    op: &Operation,
    session: &mut McpSession,
    args: &Args,
    secret: &[u8],
    nonces: &Mutex<BTreeSet<String>>,
    pids: &Arc<Mutex<Vec<i32>>>,
    gates: &WorkloadGates,
    measured_load_operation: bool,
    load_window_deadline: Option<Instant>,
    deadline: Instant,
) -> OpResult {
    let mut dispatch_started_at = None;
    let mut reconnect_count = 0usize;
    let mut reconnect_observation = None;
    let mut retry_count = 0usize;
    let mut projection_interruptions = 0usize;
    let mut projection_resumes = 0usize;
    let mut projection_status_checks = 0usize;
    let mut projection_cursor_observation = Value::Null;
    let mut projection_queue_age_observation = Value::Null;
    let mut projection_navigation_bootstrap_observation_value = Value::Null;
    let mut projection_navigation_advance_observation_value = Value::Null;
    let mut attempts = Vec::new();
    let mut error = None;
    let dispatch_deadline = if measured_load_operation {
        load_window_deadline
    } else {
        None
    };
    let _channel_permit = match op.channel.as_str() {
        "mcp" => gates.mcp_sessions.acquire(deadline, dispatch_deadline),
        "owner_http" => gates.owner_http.acquire(deadline, dispatch_deadline),
        "sdk" | "projection" => gates
            .sdk_source_commands
            .acquire(deadline, dispatch_deadline),
        _ => Err("unknown channel gate".into()),
    };
    let _channel_permit = match _channel_permit {
        Ok(Some(permit)) => Some(permit),
        Ok(None) => return operation_skipped("load-window-closed-before-dispatch"),
        Err(reason) => {
            error = Some(reason);
            None
        }
    };
    let _source_read_permit = if error.is_none() && source_read_operation(op) {
        match gates.source_reads.acquire(deadline, dispatch_deadline) {
            Ok(Some(permit)) => Some(permit),
            Ok(None) => return operation_skipped("load-window-closed-before-dispatch"),
            Err(reason) => {
                error = Some(reason);
                None
            }
        }
    } else {
        None
    };
    if error.is_none() && measured_load_operation {
        match gates.operation_rate.acquire(deadline, load_window_deadline) {
            Ok(true) => dispatch_started_at = Some(Instant::now()),
            Ok(false) => return operation_skipped("load-window-closed-before-dispatch"),
            Err(reason) => error = Some(reason),
        }
    } else if error.is_none() {
        dispatch_started_at = Some(Instant::now());
    }
    if error.is_none() {
        if op.channel == "mcp" {
            if session.id.is_none() {
                error = Some("MCP session unavailable at native session limit".to_owned());
            }
            if error.is_none() && op.reconnect_before {
                let old = session.id.take().unwrap();
                if let Err(e) = delete_mcp(args.mcp_port, &old, deadline, args.response_cap) {
                    error = Some(e);
                }
                if error.is_none() {
                    match initialize_mcp(args.mcp_port, deadline, args.response_cap) {
                        Ok((fresh, status, digest, elapsed)) if status == 200 => {
                            *session = fresh;
                            reconnect_count = 1;
                            reconnect_observation = Some(
                                json!({"status":status,"response_sha256":digest,"elapsed_ms":elapsed}),
                            );
                        }
                        Ok((_, status, _, _)) => {
                            error = Some(format!("MCP reconnect initialize status {status}"))
                        }
                        Err(e) => error = Some(e),
                    }
                }
            }
            if error.is_none() {
                let id = session.id.as_deref().unwrap();
                let attempt_started = Instant::now();
                match mcp_http(
                    args.mcp_port,
                    Some(id),
                    &op.request,
                    deadline,
                    args.response_cap,
                    "POST",
                ) {
                    Ok(reply) => attempts.push(Attempt {
                        status: reply.status,
                        bytes: reply.body.len(),
                        sha256: sha(&reply.body),
                        body: Some(reply.body),
                        stderr: Vec::new(),
                        elapsed_ms: attempt_started.elapsed().as_secs_f64() * 1000.0,
                        authenticated: None,
                        interrupted: false,
                    }),
                    Err(e) => error = Some(e),
                }
            }
        } else if op.channel == "owner_http" {
            let body = match json_bytes(&op.request) {
                Ok(v) => v,
                Err(e) => {
                    error = Some(e);
                    Vec::new()
                }
            };
            if error.is_none() {
                match owner_http(
                    args.owner_port,
                    secret,
                    nonces,
                    &body,
                    deadline,
                    args.request_cap,
                    args.response_cap,
                    op.retry_same_command_id,
                ) {
                    Ok(attempt) => {
                        if op.retry_same_command_id {
                            // The complete response was emitted by the owner and its headers observed; the harness deliberately closes before reading the body.
                            attempts.push(attempt);
                            retry_count = 1;
                            match owner_http(
                                args.owner_port,
                                secret,
                                nonces,
                                &body,
                                deadline,
                                args.request_cap,
                                args.response_cap,
                                false,
                            ) {
                                Ok(replay) => attempts.push(replay),
                                Err(e) => error = Some(e),
                            }
                        } else {
                            attempts.push(attempt);
                        }
                    }
                    Err(e) => error = Some(e),
                }
            }
        } else if op.channel == "sdk" {
            let first = sdk_process(
                op,
                &args.output,
                deadline,
                args.request_cap,
                args.response_cap,
                pids,
                op.retry_same_command_id,
            );
            match first {
                Ok(attempt) => {
                    attempts.push(attempt);
                    if op.retry_same_command_id {
                        retry_count = 1;
                        match sdk_process(
                            op,
                            &args.output,
                            deadline,
                            args.request_cap,
                            args.response_cap,
                            pids,
                            false,
                        ) {
                            Ok(replay) => attempts.push(replay),
                            Err(e) => error = Some(e),
                        }
                    }
                }
                Err(e) => error = Some(e),
            }
        } else if let Some(interrupt_after_ms) = op.interrupt_resume_after_ms {
            projection_status_checks += 1;
            match projection_status(
                op,
                &args.output,
                deadline,
                args.request_cap,
                args.response_cap,
                pids,
            ) {
                Err(e) => error = Some(e),
                Ok(before) => match before.pending() {
                    Some((before_through, before_head))
                        if before_head.saturating_sub(before_through) >= 2 =>
                    {
                        let mut progress_error = None;
                        projection_cursor_observation = json!({
                            "before_interrupt":before.observation(),
                            "after_interrupt":Value::Null,
                            "after_resume":Value::Null,
                            "partial_cursor_persisted":false
                        });
                        match projection_process(
                            op,
                            &args.output,
                            deadline,
                            args.request_cap,
                            args.response_cap,
                            pids,
                            Some(Duration::from_millis(interrupt_after_ms)),
                        ) {
                            Ok(interrupted) if interrupted.interrupted => {
                                attempts.push(interrupted);
                                projection_interruptions = 1;
                                projection_status_checks += 1;
                                let partial_cursor_persisted = match projection_status(
                                    op,
                                    &args.output,
                                    deadline,
                                    args.request_cap,
                                    args.response_cap,
                                    pids,
                                ) {
                                    Ok(after_interrupt) => {
                                        projection_cursor_observation["after_interrupt"] =
                                            after_interrupt.observation();
                                        after_interrupt.pending().is_some_and(
                                            |(through_seq, head_seq)| {
                                                through_seq > before_through
                                                    && through_seq < head_seq
                                                    && head_seq >= before_head
                                            },
                                        )
                                    }
                                    Err(e) => {
                                        progress_error = Some(e);
                                        false
                                    }
                                };
                                projection_cursor_observation["partial_cursor_persisted"] =
                                    json!(partial_cursor_persisted);
                                // Resume even when the interruption did not leave a provable
                                // partial cursor, so this disposable stage is not left pending.
                                match projection_process(
                                    op,
                                    &args.output,
                                    deadline,
                                    args.request_cap,
                                    args.response_cap,
                                    pids,
                                    None,
                                ) {
                                    Ok(resumed) if !resumed.interrupted => {
                                        attempts.push(resumed);
                                        projection_resumes = 1;
                                        projection_status_checks += 1;
                                        match projection_status(
                                            op,
                                            &args.output,
                                            deadline,
                                            args.request_cap,
                                            args.response_cap,
                                            pids,
                                        ) {
                                            Ok(after_resume) => {
                                                projection_cursor_observation["after_resume"] =
                                                    after_resume.observation();
                                                if after_resume.current().is_none() {
                                                    progress_error.get_or_insert_with(|| {
                                                        "projection resume did not reach a current durable cursor".into()
                                                    });
                                                }
                                            }
                                            Err(e) => progress_error = Some(e),
                                        }
                                    }
                                    Ok(_) => {
                                        progress_error = Some(
                                            "projection resume was unexpectedly interrupted".into(),
                                        )
                                    }
                                    Err(e) => progress_error = Some(e),
                                }
                                if !partial_cursor_persisted {
                                    progress_error.get_or_insert_with(|| {
                                        "interrupted projection did not persist a partial durable cursor".into()
                                    });
                                }
                                error = progress_error;
                            }
                            Ok(_) => error = Some("projection process was not interrupted".into()),
                            Err(e) => error = Some(e),
                        }
                    }
                    Some(_) => {
                        error = Some(
                            "projection interruption requires at least two pending source events"
                                .into(),
                        )
                    }
                    None => {
                        error =
                            Some("projection interruption requires a pending durable cursor".into())
                    }
                },
            }
        } else {
            match projection_process(
                op,
                &args.output,
                deadline,
                args.request_cap,
                args.response_cap,
                pids,
                None,
            ) {
                Ok(attempt) => attempts.push(attempt),
                Err(e) => error = Some(e),
            }
        }
    }
    if op.channel == "projection" && error.is_none() {
        match op.request["operation"].as_str() {
            Some("bootstrap-navigation") => {
                match navigation_bootstrap_observation(op, attempts.last()) {
                    Ok(observation) => {
                        projection_navigation_bootstrap_observation_value = observation
                    }
                    Err(reason) => error = Some(reason),
                }
            }
            Some("advance-navigation") => match navigation_advance_observation(op, attempts.last())
            {
                Ok(observation) => projection_navigation_advance_observation_value = observation,
                Err(reason) => error = Some(reason),
            },
            _ if postload_projection_rights_denial(op) => {}
            _ => {
                projection_status_checks += 1;
                match projection_queue_age(
                    op,
                    &args.output,
                    deadline,
                    args.request_cap,
                    args.response_cap,
                    pids,
                ) {
                    Ok(observation) => projection_queue_age_observation = observation,
                    Err(e) => error = Some(e),
                }
            }
        }
    }
    let final_attempt = attempts.last().cloned();
    let auth_ok = op.channel != "owner_http"
        || final_attempt
            .as_ref()
            .is_some_and(|a| a.authenticated == Some(true));
    let status = final_attempt.as_ref().map(|a| a.status);
    let navigation_advance =
        op.channel == "projection" && op.request["operation"] == "advance-navigation";
    let expected = outcomes(op);
    let matched_outcome = if navigation_advance {
        None
    } else {
        final_attempt.as_ref().and_then(|attempt| {
            expected.iter().find(|value| {
                outcome_matches(args, op, value, attempt, &projection_cursor_observation)
            })
        })
    };
    let response_oracle_matches = if navigation_advance {
        status == Some(0) && !projection_navigation_advance_observation_value.is_null()
    } else {
        matched_outcome.is_some()
    };
    let passed = error.is_none() && auth_ok && response_oracle_matches;
    let conflict = match final_attempt.as_ref() {
        Some(attempt) if op.channel == "sdk" => sdk_conflict_attempt(attempt),
        Some(attempt) => {
            attempt.status == 409
                && (op.channel != "owner_http" || attempt.authenticated == Some(true))
        }
        None => false,
    };
    let transport_error = error.is_some()
        || final_attempt.is_none()
        || (op.channel == "owner_http"
            && final_attempt
                .as_ref()
                .is_some_and(|a| a.authenticated != Some(true)));
    let successful = passed
        && if matches!(op.channel.as_str(), "sdk" | "projection") {
            status == Some(0)
        } else {
            status.is_some_and(|s| (200..300).contains(&s))
        };
    let response_body = final_attempt
        .as_ref()
        .and_then(|attempt| attempt.body.as_deref())
        .and_then(|body| serde_json::from_slice::<Value>(body).ok());
    let elapsed_ms = dispatch_started_at
        .map(|started| started.elapsed().as_secs_f64() * 1000.0)
        .unwrap_or(0.0);
    let attempt_values = attempts.iter().map(|a| json!({"status":a.status,"response_bytes":a.bytes,"response_sha256":a.sha256,"response_available_to_oracle":a.body.is_some(),"response_disposition":if a.interrupted {"interrupted"} else if a.body.is_none() {"dropped"} else {"delivered"},"diagnostic_bytes":a.stderr.len(),"diagnostic_sha256":sha(&a.stderr),"interrupted":a.interrupted,"elapsed_ms":a.elapsed_ms,"response_authenticated":a.authenticated})).collect::<Vec<_>>();
    let expected_report = outcome_report(op);
    let matched_report = if navigation_advance && passed {
        Some(json!({"status":0,"oracle":"bounded-navigation-range"}))
    } else if interrupted_projection_apply(op) && passed {
        Some(
            json!({"status":0,"oracle":"bounded-interrupted-projection-progress-and-resumed-cursor"}),
        )
    } else {
        matched_outcome.map(outcome_summary)
    };
    let value = json!({"event":"operation","label":op.label,"channel":op.channel,"status":status,"expected_outcomes":expected_report,"expected_stderr_prefix":op.expected_stderr_prefix,"matched_outcome":matched_report,"response_sha256":final_attempt.as_ref().map(|a|a.sha256.clone()),"passed":passed,"successful_command":successful,"transport_error":transport_error,"error":error,"conflict_group":op.conflict_group,"conflict":conflict,"attempts":attempt_values,"retry_count":retry_count,"reconnect_count":reconnect_count,"projection_interruptions":projection_interruptions,"projection_resumes":projection_resumes,"projection_status_checks":projection_status_checks,"projection_cursor":projection_cursor_observation,"projection_queue_age":projection_queue_age_observation,"projection_navigation_bootstrap":projection_navigation_bootstrap_observation_value,"projection_navigation_advance":projection_navigation_advance_observation_value,"reconnect":reconnect_observation,"elapsed_ms":elapsed_ms});
    OpResult {
        value,
        passed,
        successful,
        not_started: false,
        conflict,
        retry_count,
        reconnect_count,
        projection_interruptions,
        projection_resumes,
        projection_status_checks,
        response_body,
        elapsed_ms,
    }
}
fn stop_child(child: &mut ManagedChild) {
    let pid = child.id() as i32;
    child.unregister();
    if child.reaped {
        return;
    }
    kill_group(pid, libc::SIGTERM);
    // Do not reap the leader before signaling its process group again: an
    // unreaped pid pins the group id while pipe-holding descendants are killed.
    thread::sleep(Duration::from_millis(10));
    kill_group(pid, libc::SIGKILL);
    let stop_at = Instant::now() + Duration::from_secs(1);
    while Instant::now() < stop_at {
        if child.try_wait().ok().flatten().is_some() {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
}
fn signal_all(pids: &Arc<Mutex<Vec<i32>>>, signal: i32) {
    if let Ok(pids) = pids.lock() {
        for pid in pids.iter().copied() {
            kill_group(pid, signal);
        }
    }
}
fn resource_delta(start: &ResourceSnapshot, end: &ResourceSnapshot) -> Value {
    json!({"cpu_usage_usec":end.cpu_usec.saturating_sub(start.cpu_usec),"memory_current_start_bytes":start.memory_current,"memory_current_end_bytes":end.memory_current,"memory_peak_start_bytes":start.memory_peak,"memory_peak_bytes":end.memory_peak,"memory_peak_growth_bytes":end.memory_peak.saturating_sub(start.memory_peak),"memory_limit_bytes":end.memory_limit,"memory_high_events_delta":end.memory_high_events.saturating_sub(start.memory_high_events),"memory_max_events_delta":end.memory_max_events.saturating_sub(start.memory_max_events),"memory_oom_events_delta":end.memory_oom_events.saturating_sub(start.memory_oom_events),"memory_oom_kill_events_delta":end.memory_oom_kill_events.saturating_sub(start.memory_oom_kill_events),"io_read_bytes":end.io_read_bytes.saturating_sub(start.io_read_bytes),"io_write_bytes":end.io_write_bytes.saturating_sub(start.io_write_bytes)})
}

fn resource_stop_observed_since(start: &ResourceSnapshot, end: &ResourceSnapshot) -> bool {
    end.memory_limit
        .is_some_and(|limit| end.memory_peak as u128 * 10 >= limit as u128 * 9)
        || end.memory_high_events > start.memory_high_events
        || end.memory_max_events > start.memory_max_events
        || end.memory_oom_events > start.memory_oom_events
        || end.memory_oom_kill_events > start.memory_oom_kill_events
}

fn limit_plan_operations(
    plan: &mut Plan,
    session_limit: usize,
    operation_limit: Option<usize>,
    channel_mix: Option<&BTreeMap<String, u8>>,
) -> Result<(), String> {
    let sessions = plan.sessions.iter().take(session_limit).collect::<Vec<_>>();
    let available = sessions
        .iter()
        .map(|session| session.operations.len())
        .sum::<usize>();
    let operation_limit = operation_limit.unwrap_or(available);
    if operation_limit == 0 || operation_limit > available {
        return Err("--operation-count exceeds selected schedule operations".into());
    }
    let rounds = sessions
        .iter()
        .map(|session| session.operations.len())
        .max()
        .unwrap_or(0);
    let mut keep_per_session = vec![0usize; sessions.len()];
    if let Some(channel_mix) = channel_mix {
        let mut quotas = BTreeMap::<String, usize>::new();
        let mut fractional = Vec::<(String, usize)>::new();
        let mut assigned = 0usize;
        for (channel, weight) in channel_mix {
            let product = operation_limit * *weight as usize;
            let whole = product / 100;
            quotas.insert(channel.clone(), whole);
            fractional.push((channel.clone(), product % 100));
            assigned += whole;
        }
        fractional.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
        for (channel, _) in fractional.into_iter().take(operation_limit - assigned) {
            *quotas.entry(channel).or_default() += 1;
        }
        let mut selected = BTreeMap::<String, usize>::new();
        let mut blocked = vec![false; sessions.len()];
        for step in 0..rounds {
            for (index, session) in sessions.iter().enumerate() {
                if blocked[index] {
                    continue;
                }
                if let Some(operation) = session.operations.get(step) {
                    let selected_for_channel =
                        selected.entry(operation.channel.clone()).or_default();
                    if *selected_for_channel < quotas.get(&operation.channel).copied().unwrap_or(0)
                    {
                        *selected_for_channel += 1;
                        keep_per_session[index] = step + 1;
                    } else {
                        blocked[index] = true;
                    }
                }
            }
        }
        if quotas
            .iter()
            .any(|(channel, quota)| selected.get(channel).copied().unwrap_or(0) != *quota)
        {
            return Err("--channel-mix cannot be selected as per-session schedule prefixes".into());
        }
    } else {
        let mut remaining = operation_limit;
        for step in 0..rounds {
            for (index, session) in sessions.iter().enumerate() {
                if remaining == 0 {
                    break;
                }
                if session.operations.get(step).is_some() {
                    keep_per_session[index] = step + 1;
                    remaining -= 1;
                }
            }
        }
        if remaining != 0 {
            return Err("--operation-count could not be selected from workload schedule".into());
        }
    }
    let selected = keep_per_session.iter().sum::<usize>();
    if selected != operation_limit {
        return Err("selected schedule prefix count differs from --operation-count".into());
    }
    for (session, keep) in plan
        .sessions
        .iter_mut()
        .take(session_limit)
        .zip(keep_per_session)
    {
        session.operations.truncate(keep);
    }
    Ok(())
}

fn emit(file: &mut File, written: &mut usize, cap: usize, value: &Value) -> Result<(), String> {
    let mut raw = serde_json::to_vec(value).map_err(|_| "report JSON serialization failed")?;
    raw.push(b'\n');
    if written.checked_add(raw.len()).is_none_or(|n| n > cap) {
        return Err("aggregate report output cap exceeded".into());
    }
    file.write_all(&raw)
        .map_err(|e| format!("report write: {e}"))?;
    file.flush().map_err(|e| format!("report flush: {e}"))?;
    *written += raw.len();
    Ok(())
}
fn main_result() -> Result<i32, String> {
    let Some(mut args) = parse_args()? else {
        println!("{USAGE}");
        return Ok(0);
    };
    install_signals();
    if args.output.is_symlink() || !args.output.is_dir() {
        return Err("output must be a precreated owned directory".into());
    }
    let output_meta =
        fs::symlink_metadata(&args.output).map_err(|e| format!("output metadata: {e}"))?;
    if output_meta.uid() != unsafe { libc::geteuid() } || output_meta.mode() & 0o077 != 0 {
        return Err("output directory must be private and owned by the runner".into());
    }
    let deadline = Instant::now() + Duration::from_secs(args.deadline_seconds);
    let schedule_initial_stamp = file_stamp(&args.schedule, deadline)?;
    if schedule_initial_stamp.len > args.schedule_cap as u64 {
        return Err("schedule cap exceeded".into());
    }
    let mut schedule_file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&args.schedule)
        .map_err(|e| format!("schedule open: {e}"))?;
    let mut schedule_bytes = Vec::new();
    schedule_file
        .take(args.schedule_cap as u64 + 1)
        .read_to_end(&mut schedule_bytes)
        .map_err(|e| format!("schedule read: {e}"))?;
    if schedule_bytes.len() > args.schedule_cap
        || schedule_bytes.len() as u64 != schedule_initial_stamp.len
        || !stamp_equal(
            &schedule_initial_stamp,
            &file_stamp(&args.schedule, deadline)?,
        )
    {
        return Err("schedule changed between identity and read".into());
    }
    let schedule_sha256 = sha(&schedule_bytes);
    let mut plan: Plan =
        serde_json::from_slice(&schedule_bytes).map_err(|_| "invalid bounded workload schedule")?;
    let native_pg_stage = validate_pg_stage(&plan, &args)?;
    let fresh_pg_stage = native_pg_stage && args.pg_stage_mode == PgStageMode::Fresh;
    validate_plan(&plan, &args)?;
    limit_plan_operations(
        &mut plan,
        args.sessions,
        args.operation_count,
        args.channel_mix.as_ref(),
    )?;
    let (_, needs_mcp, needs_owner) = validate_plan(&plan, &args)?;
    let cgroup = cgroup_path(&args.unit)?;
    let lease_execution = write_lease(&args.output, &args.unit, args.output_cap)?;
    let owner_stage = prepare_owner_stage(&mut args, &mut plan, needs_owner, deadline)?;
    let mut paths = vec![args.query_binary.clone()];
    let prepared_model_index = args.model.as_ref().map(|path| {
        let index = paths.len();
        paths.push(path.clone());
        index
    });
    let prepared_binding_index = args.binding.as_ref().map(|path| {
        let index = paths.len();
        paths.push(path.clone());
        index
    });
    let schedule_index = paths.len();
    paths.push(args.schedule.clone());
    let mut owner_indexes = BTreeMap::new();
    for (name, path) in [
        ("owner_binary", &args.owner_binary),
        ("owner_config", &args.owner_config),
        ("invocation", &args.invocation),
        ("token_file", &args.token_file),
        ("source_inputs", &args.source_inputs),
        (
            "source_text_search_selection",
            &args.source_text_search_selection,
        ),
    ] {
        if let Some(path) = path {
            let index = paths.len();
            paths.push(path.clone());
            owner_indexes.insert(name, index);
        }
    }
    let mut input_stamps = Vec::new();
    for path in &paths {
        input_stamps.push(file_stamp(path, deadline)?);
    }
    if !stamp_equal(&input_stamps[schedule_index], &schedule_initial_stamp) {
        return Err("schedule path identity changed while preparing owner stage".into());
    }
    let (total_ops, needs_mcp, needs_owner) = validate_plan(&plan, &args)?;
    let logical_mcp_sessions = plan
        .sessions
        .iter()
        .take(args.sessions)
        .filter(|session| session.operations.iter().any(|op| op.channel == "mcp"))
        .count();
    let physical_mcp_session_limit = logical_mcp_sessions.min(32);
    let source_read_operations = plan
        .setup
        .iter()
        .chain(
            plan.sessions
                .iter()
                .take(args.sessions)
                .flat_map(|session| &session.operations),
        )
        .chain(plan.postload.iter())
        .filter(|operation| source_read_operation(operation))
        .count();
    let sdk_source_command_operations = plan
        .setup
        .iter()
        .chain(
            plan.sessions
                .iter()
                .take(args.sessions)
                .flat_map(|session| &session.operations),
        )
        .chain(plan.postload.iter())
        .filter(|operation| {
            operation.channel == "sdk"
                && operation
                    .sdk_argv
                    .iter()
                    .any(|arg| arg == "source-commands")
        })
        .count();
    let projection_operations = plan
        .sessions
        .iter()
        .take(args.sessions)
        .flat_map(|session| &session.operations)
        .filter(|operation| operation.channel == "projection")
        .count();
    let postload_projection_operations = plan
        .postload
        .iter()
        .filter(|operation| operation.channel == "projection")
        .count();
    let postload_source_read_operations = plan
        .postload
        .iter()
        .filter(|operation| postload_selected_source_integrity(operation))
        .count();
    let postload_selected_vector_read_operations = plan
        .postload
        .iter()
        .filter(|operation| postload_selected_vector_integrity(operation))
        .count();
    let postload_sdk_pg_source_read_operations = plan
        .postload
        .iter()
        .filter(|operation| postload_sdk_pg_source_integrity(operation))
        .count();
    let postload_pg_source_read_operations = plan
        .postload
        .iter()
        .filter(|operation| postload_pg_source_integrity(operation))
        .count();
    let postload_rights_checks = plan
        .postload
        .iter()
        .filter(|operation| postload_rights_check(operation))
        .count();
    let postload_rights_revocations = plan
        .postload
        .iter()
        .filter(|operation| postload_rights_revoke_operation(operation))
        .count();
    let postload_rights_denials = plan
        .postload
        .iter()
        .filter(|operation| postload_projection_rights_denial(operation))
        .count();
    let postload_idempotency_checks = plan
        .postload
        .iter()
        .filter(|operation| operation.idempotency_replay_of.is_some())
        .count();
    let projection_setup_operations = plan
        .setup
        .iter()
        .filter(|operation| operation.channel == "projection")
        .count();
    let projection_navigation_bootstrap_operations = plan
        .setup
        .iter()
        .filter(|operation| {
            operation.channel == "projection"
                && operation.request["operation"] == "bootstrap-navigation"
        })
        .count();
    let projection_navigation_operations = plan
        .setup
        .iter()
        .chain(
            plan.sessions
                .iter()
                .take(args.sessions)
                .flat_map(|session| &session.operations),
        )
        .filter(|operation| {
            operation.channel == "projection"
                && matches!(
                    operation.request["operation"].as_str(),
                    Some("bootstrap-navigation" | "advance-navigation")
                )
        })
        .count();
    let mut sdk_indexes = BTreeMap::new();
    for operation in plan
        .setup
        .iter()
        .chain(
            plan.sessions
                .iter()
                .take(args.sessions)
                .flat_map(|session| &session.operations),
        )
        .chain(plan.postload.iter())
    {
        if operation.channel == "sdk" {
            let path = PathBuf::from(&operation.sdk_argv[0]);
            if !sdk_indexes.contains_key(&path) {
                let index = paths.len();
                paths.push(path.clone());
                sdk_indexes.insert(path.clone(), index);
                input_stamps.push(file_stamp(&path, deadline)?);
            }
        }
    }
    let mut projection_indexes = BTreeMap::new();
    for operation in plan
        .setup
        .iter()
        .chain(
            plan.sessions
                .iter()
                .take(args.sessions)
                .flat_map(|session| &session.operations),
        )
        .chain(plan.postload.iter())
    {
        if operation.channel == "projection" {
            let path = PathBuf::from(&operation.projection_argv[0]);
            if !projection_indexes.contains_key(&path) {
                let index = paths.len();
                paths.push(path.clone());
                projection_indexes.insert(path.clone(), index);
                input_stamps.push(file_stamp(&path, deadline)?);
            }
        }
    }
    let sdk_owner_inputs = selected_sdk_owner_inputs(&plan, args.sessions)?;
    let mut sdk_owner_input_indexes = BTreeMap::new();
    for (invocation, owner_config) in &sdk_owner_inputs {
        for path in [invocation, owner_config] {
            if !paths.contains(path) {
                let index = paths.len();
                paths.push(path.clone());
                input_stamps.push(file_stamp(path, deadline)?);
                sdk_owner_input_indexes.insert(path.clone(), index);
            }
        }
    }
    let projection_inputs = selected_projection_inputs(&plan, args.sessions)?;
    let mut projection_input_indexes = BTreeMap::new();
    for path in &projection_inputs {
        if !paths.contains(path) {
            let index = paths.len();
            paths.push(path.clone());
            input_stamps.push(file_stamp(path, deadline)?);
            projection_input_indexes.insert(path.clone(), index);
        }
    }
    let mut executable_hashes = BTreeMap::new();
    executable_hashes.insert(0usize, executable_sha256(&args.query_binary, deadline)?);
    if let Some(index) = owner_indexes.get("owner_binary") {
        executable_hashes.insert(*index, executable_sha256(&paths[*index], deadline)?);
    }
    for index in sdk_indexes.values() {
        executable_hashes.insert(*index, executable_sha256(&paths[*index], deadline)?);
    }
    for index in projection_indexes.values() {
        executable_hashes.insert(*index, executable_sha256(&paths[*index], deadline)?);
    }
    let secret = if needs_owner {
        token_bytes(
            args.token_file
                .as_ref()
                .ok_or("owner HTTP token path absent")?,
        )?
    } else {
        Vec::new()
    };
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(args.output.join("measurements.jsonl"))
        .map_err(|e| format!("exclusive report open: {e}"))?;
    let mut written = 0usize;
    let sdk_identities = sdk_indexes
        .values()
        .map(|index| {
            identity_json(
                &input_stamps[*index],
                executable_hashes.get(index).map(String::as_str),
            )
        })
        .collect::<Vec<_>>();
    let projection_identities = projection_indexes
        .values()
        .map(|index| {
            identity_json(
                &input_stamps[*index],
                executable_hashes.get(index).map(String::as_str),
            )
        })
        .collect::<Vec<_>>();
    let owner_input_identities = owner_indexes
        .iter()
        .map(|(name, index)| {
            (
                name.to_string(),
                identity_json(
                    &input_stamps[*index],
                    executable_hashes.get(index).map(String::as_str),
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let sdk_owner_input_identities = sdk_owner_input_indexes
        .iter()
        .map(|(path, index)| {
            (
                path.display().to_string(),
                identity_json(&input_stamps[*index], None),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let projection_input_identities = projection_input_indexes
        .iter()
        .map(|(path, index)| {
            (
                path.display().to_string(),
                identity_json(&input_stamps[*index], None),
            )
        })
        .collect::<BTreeMap<_, _>>();
    emit(
        &mut output,
        &mut written,
        args.output_cap,
        &json!({
            "event": "start",
            "identities": {
                "query_binary": identity_json(&input_stamps[0], executable_hashes.get(&0).map(String::as_str)),
                "owner_inputs": owner_input_identities,
                "sdk_owner_inputs": sdk_owner_input_identities,
                "projection_selections": projection_input_identities,
                "prepared_model": prepared_model_index.map(|index| identity_json(&input_stamps[index], None)),
                "prepared_binding": prepared_binding_index.map(|index| identity_json(&input_stamps[index], None)),
                "external_sdk_binaries": sdk_identities,
                "external_projection_binaries": projection_identities,
                "schedule_sha256": schedule_sha256
            },
            "owner_stage_clone": owner_stage.as_ref().map(|(baseline, destination, configuration_bindings, copied_files, copied_bytes)| json!({
                "baseline": baseline,
                "destination": destination,
                "rebound_expected_configuration_requests": configuration_bindings,
                "projection_selection_count": projection_inputs.len(),
                "copied_files": copied_files,
                "copied_bytes": copied_bytes,
                "bounded_copy_bytes": args.owner_stage_clone_cap_bytes,
                "bounded_copy_files": MAX_STAGE_FILES
            })),
            "limits": {
                "logical_sessions": args.sessions,
                "serial_setup_operations": plan.setup.len(),
                "serial_postload_checks": plan.postload.len(),
                "logical_mcp_sessions": logical_mcp_sessions,
                "physical_mcp_sessions": physical_mcp_session_limit,
                "mcp_in_flight_limit": 32,
                "source_read_in_flight_limit": 32,
                "owner_http_in_flight_limit": 4,
                "sdk_source_command_in_flight_limit": args.sdk_in_flight,
                "scheduled_source_read_operations": source_read_operations,
                "scheduled_sdk_source_commands": sdk_source_command_operations,
                "scheduled_projection_operations": projection_operations,
                "postload_source_integrity_checks": postload_source_read_operations,
                "postload_selected_source_integrity_checks": postload_source_read_operations,
                "postload_selected_vector_integrity_checks": postload_selected_vector_read_operations,
                "postload_sdk_pg_source_integrity_checks": postload_sdk_pg_source_read_operations,
                "postload_native_pg_integrity_checks": postload_pg_source_read_operations,
                "postload_rights_state_checks": postload_rights_checks,
                "postload_rights_revocations": postload_rights_revocations,
                "postload_rights_denials": postload_rights_denials,
                "postload_idempotency_replays": postload_idempotency_checks,
                "postload_projection_checks": postload_projection_operations,
                "serial_projection_setup_operations": projection_setup_operations,
                "operation_count_limit": args.operation_count,
                "requested_channel_mix_percent": args.channel_mix.as_ref(),
                "operations_per_second_limit": args.operations_per_second,
                "load_window_seconds_limit": args.load_window_seconds,
                "fresh_pg_stage": fresh_pg_stage,
                "native_pg_stage": native_pg_stage,
                "pg_stage_mode": match args.pg_stage_mode {
                    PgStageMode::Fresh => "fresh",
                    PgStageMode::Existing => "existing",
                },
                "deadline_seconds": args.deadline_seconds,
                "request_cap_bytes": args.request_cap,
                "response_cap_bytes": args.response_cap,
                "schedule_cap_bytes": args.schedule_cap,
                "output_cap_bytes": args.output_cap
            },
            "lease_execution": lease_execution,
            "scope": "finite Rust protocol measurement; not capacity acceptance"
        }),
    )?;
    let resources_before = resource_snapshot(&cgroup)?;
    let pids = Arc::new(Mutex::new(Vec::new()));
    let watchdog_pids = Arc::clone(&pids);
    let watchdog_stop = Arc::new(AtomicBool::new(false));
    let watchdog_flag = Arc::clone(&watchdog_stop);
    let watchdog = thread::spawn(move || {
        while !watchdog_flag.load(Ordering::Acquire)
            && !STOP.load(Ordering::Acquire)
            && Instant::now() < deadline
        {
            thread::sleep(Duration::from_millis(20));
        }
        if !watchdog_flag.load(Ordering::Acquire) {
            signal_all(&watchdog_pids, libc::SIGKILL);
        }
    });
    let nonce_set = Arc::new(Mutex::new(BTreeSet::new()));
    let start_time = Instant::now();
    let mut mcp_server = None;
    let mut owner_server = None;
    if needs_mcp {
        let mut command = Command::new(&args.query_binary);
        let mut query_args = Vec::new();
        if let (Some(root), Some(inputs)) = (&args.source_root, &args.source_inputs) {
            query_args.extend([
                "--root".to_owned(),
                root.to_str()
                    .ok_or("source root path UTF-8 required")?
                    .to_owned(),
                "--source-inputs".to_owned(),
                inputs
                    .to_str()
                    .ok_or("source inputs path UTF-8 required")?
                    .to_owned(),
            ]);
        }
        if let (Some(model), Some(binding)) = (&args.model, &args.binding) {
            query_args.extend([
                "--prepared-read-model".to_owned(),
                model
                    .to_str()
                    .ok_or("model path UTF-8 required")?
                    .to_owned(),
                "--prepared-binding".to_owned(),
                binding
                    .to_str()
                    .ok_or("binding path UTF-8 required")?
                    .to_owned(),
            ]);
        }
        if let Some(selection) = &args.source_text_search_selection {
            query_args.extend([
                "--source-text-search-selection".to_owned(),
                selection
                    .to_str()
                    .ok_or("PG source-text selection path UTF-8 required")?
                    .to_owned(),
            ]);
        }
        query_args.extend([
            "mcp".to_owned(),
            "--transport".to_owned(),
            "streamable-http".to_owned(),
            "--host".to_owned(),
            "127.0.0.1".to_owned(),
            "--port".to_owned(),
            args.mcp_port.to_string(),
        ]);
        command
            .args(query_args)
            .current_dir(&args.output)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        mcp_server = Some(spawn_group(&mut command, &pids, deadline)?);
        wait_listener(mcp_server.as_mut().unwrap(), args.mcp_port, deadline)?;
    }
    if needs_owner {
        let origin = format!("http://127.0.0.1:{}", args.owner_port);
        let owner_binary = args
            .owner_binary
            .as_ref()
            .ok_or("owner HTTP executable absent")?;
        let owner_config = args
            .owner_config
            .as_ref()
            .ok_or("owner HTTP config absent")?;
        let invocation = args
            .invocation
            .as_ref()
            .ok_or("owner HTTP invocation absent")?;
        let token_file = args
            .token_file
            .as_ref()
            .ok_or("owner HTTP token path absent")?;
        let mut command = Command::new(owner_binary);
        command
            .args([
                "http",
                "--owner-config",
                owner_config.to_str().ok_or("owner config UTF-8 required")?,
                "--native-invocation",
                invocation.to_str().ok_or("invocation UTF-8 required")?,
                "--token-file",
                token_file.to_str().ok_or("token file UTF-8 required")?,
                "--browser-origin",
                &origin,
                "--port",
                &args.owner_port.to_string(),
            ])
            .current_dir(&args.output)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        owner_server = Some(spawn_group(&mut command, &pids, deadline)?);
        wait_listener(owner_server.as_mut().unwrap(), args.owner_port, deadline)?;
        // Authenticated catalog preflight; it discovers the actual descriptor but stores only its count and digest.
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "system clock before epoch")?
            .as_secs()
            .to_string();
        let nonce = fresh_nonce(&nonce_set)?;
        let digest = sha(&[]);
        let proof = request_mac(&secret, "GET", "/commands/catalog", &ts, &nonce, &[])?;
        let authorization = format!("ToS-HMAC-SHA256 {ts}:{nonce}:{digest}:{proof}");
        let reply = http_exchange(
            args.owner_port,
            "GET",
            "/commands/catalog",
            &[],
            &[("Authorization", authorization)],
            args.response_cap,
            deadline,
            false,
        )?;
        if reply.status != 200
            || !response_authenticated(
                &secret,
                &nonce,
                reply.status,
                &reply.body,
                reply.headers.get("x-tos-response-signature"),
            )
        {
            return Err("authenticated native source-command catalog preflight failed".into());
        }
        let catalog: Value = serde_json::from_slice(&reply.body)
            .map_err(|_| "native command catalog is not JSON")?;
        let handler_count = catalog["handler_count"]
            .as_u64()
            .ok_or("native handler count absent")?;
        emit(
            &mut output,
            &mut written,
            args.output_cap,
            &json!({"event":"owner_catalog","handler_count":handler_count,"catalog_sha256":sha(&reply.body),"authorization_status":catalog["authorization_status"]}),
        )?;
    }
    let mut logical_mcp_slots = vec![None; args.sessions];
    let mut mcp_slot_count = 0usize;
    for (logical_index, session) in plan.sessions.iter().take(args.sessions).enumerate() {
        if session.operations.iter().any(|op| op.channel == "mcp") {
            logical_mcp_slots[logical_index] = Some(mcp_slot_count % 32);
            mcp_slot_count += 1;
        }
    }
    let physical_mcp_sessions = mcp_slot_count.min(32);
    let mut mcp_sessions = Vec::with_capacity(physical_mcp_sessions);
    for slot in 0..physical_mcp_sessions {
        let started = Instant::now();
        let (state, status, digest) = {
            let (state, status, digest, _) =
                initialize_mcp(args.mcp_port, deadline, args.response_cap)?;
            (state, Some(status), Some(digest))
        };
        emit(
            &mut output,
            &mut written,
            args.output_cap,
            &json!({"event":"mcp_session_ready","slot":slot,"initialize_status":status,"initialize_sha256":digest,"initialize_elapsed_ms":started.elapsed().as_secs_f64()*1000.0,"available":status==Some(200)}),
        )?;
        mcp_sessions.push(Arc::new(Mutex::new(state)));
    }
    let gates = WorkloadGates {
        mcp_sessions: WaveGate::new(32),
        owner_http: WaveGate::new(4),
        source_reads: WaveGate::new(32),
        sdk_source_commands: WaveGate::new(args.sdk_in_flight),
        operation_rate: OperationRateGate::new(args.operations_per_second),
    };
    let start_metrics = Instant::now();
    let rounds = plan
        .sessions
        .iter()
        .take(args.sessions)
        .map(|s| s.operations.len())
        .max()
        .unwrap_or(0);
    let mut successes = 0usize;
    let mut errors = 0usize;
    let mut conflicts = 0usize;
    let mut conflict_groups = 0usize;
    let mut conflict_group_failures = 0usize;
    let mut retries = 0usize;
    let mut reconnects = 0usize;
    let mut projection_interruptions = 0usize;
    let mut projection_resumes = 0usize;
    let mut projection_status_checks = 0usize;
    let mut latencies = Vec::new();
    let mut all_latencies = Vec::new();
    let mut load_latencies = Vec::new();
    let load_operations_scheduled = plan
        .sessions
        .iter()
        .take(args.sessions)
        .map(|session| session.operations.len())
        .sum::<usize>();
    let mut load_operations_started = 0usize;
    let mut load_operations_skipped = 0usize;
    let mut load_operations_passed = 0usize;
    let mut load_operations_failed = 0usize;
    let mut scheduled_channel_mix = BTreeMap::<String, usize>::new();
    let mut started_channel_mix = BTreeMap::<String, usize>::new();
    let mut passed_channel_mix = BTreeMap::<String, usize>::new();
    let mut failed_channel_mix = BTreeMap::<String, usize>::new();
    let mut projection_queue_age_samples = 0usize;
    let mut projection_queue_age_status_counts = BTreeMap::<String, usize>::new();
    let mut projection_max_sequence_lag = 0u64;
    let mut projection_max_known_queue_age_millis = None::<u64>;
    let mut projection_navigation_bootstrap_observations = 0usize;
    let mut projection_navigation_advance_samples = 0usize;
    let mut projection_navigation_advanced = 0usize;
    let mut projection_navigation_idle_current = 0usize;
    let mut projection_navigation_more_pending_prefix = 0usize;
    let mut projection_navigation_replayed = 0usize;
    let mut projection_navigation_max_sequence_lag_before = None::<u64>;
    let mut projection_navigation_max_sequence_lag_after = None::<u64>;
    for operation in plan
        .sessions
        .iter()
        .take(args.sessions)
        .flat_map(|session| &session.operations)
    {
        *scheduled_channel_mix
            .entry(operation.channel.clone())
            .or_default() += 1;
    }
    let mut setup_results = BTreeMap::new();
    let mut setup_successes = 0usize;
    let mut setup_failures = 0usize;
    let mut successful_load_mutations = BTreeSet::new();
    let mut successful_load_mutation_responses = BTreeMap::<String, Value>::new();
    let setup_started_at = Instant::now();
    for setup_index in 0..plan.setup.len() {
        if STOP.load(Ordering::Acquire) || Instant::now() >= deadline {
            return Err("serial native setup deadline/cancellation".into());
        }
        let mut op = plan.setup[setup_index].clone();
        if let Err(reason) = bind_operation_from_setup(&mut op, &setup_results) {
            setup_failures += 1;
            errors += 1;
            emit(
                &mut output,
                &mut written,
                args.output_cap,
                &json!({"event":"setup","phase":"serial-owner-setup","label":op.label,"passed":false,"error":reason,"index":setup_index}),
            )?;
            break;
        }
        let mut result = run_operation(
            &op,
            &mut McpSession::default(),
            &args,
            &secret,
            &nonce_set,
            &pids,
            &gates,
            false,
            None,
            deadline,
        );
        if result.value["projection_navigation_bootstrap"].is_object() {
            projection_navigation_bootstrap_observations += 1;
        }
        if result.passed && result.response_body.is_none() {
            result.passed = false;
            result.successful = false;
            result.value["passed"] = json!(false);
            result.value["transport_error"] = json!(true);
            result.value["error"] = json!("native setup response is not JSON");
        }
        if result.passed {
            setup_successes += 1;
            if let Some(body) = result.response_body.clone() {
                setup_results.insert(
                    op.label.clone(),
                    json!({"request":op.request.clone(),"sdk_argv":op.sdk_argv.clone(),"response":body}),
                );
            }
        } else {
            setup_failures += 1;
            errors += 1;
        }
        retries += result.retry_count;
        conflicts += usize::from(result.conflict);
        all_latencies.push(result.elapsed_ms);
        if result.passed && result.successful {
            latencies.push(result.elapsed_ms);
        }
        let mut value = result.value;
        value["event"] = json!("setup");
        value["phase"] = json!("serial-owner-setup");
        value["index"] = json!(setup_index);
        emit(&mut output, &mut written, args.output_cap, &value)?;
        if !result.passed {
            break;
        }
    }
    let setup_elapsed_seconds = setup_started_at.elapsed().as_secs_f64();
    successes = setup_successes;
    let load_resources_before = resource_snapshot(&cgroup)?;
    let load_started_at = Instant::now();
    let load_window_deadline = args
        .load_window_seconds
        .map(|seconds| load_started_at + Duration::from_secs(seconds));
    let mut load_window_closed = false;
    for step in 0..rounds {
        if setup_failures != 0 {
            break;
        }
        if STOP.load(Ordering::Acquire) || Instant::now() >= deadline {
            return Err("whole workload deadline/cancellation".into());
        }
        if load_window_deadline.is_some_and(|window| window <= deadline && Instant::now() >= window)
        {
            load_window_closed = true;
            break;
        }
        let jobs = plan
            .sessions
            .iter()
            .take(args.sessions)
            .enumerate()
            .filter_map(|(index, s)| {
                s.operations
                    .get(step)
                    .cloned()
                    .map(|op| (index, s.id.clone(), op))
            })
            .map(|(index, session, mut op)| {
                bind_operation_from_setup(&mut op, &setup_results).map(|()| (index, session, op))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if jobs.is_empty() {
            continue;
        }
        let (tx, rx) = mpsc::channel();
        let mut workers = SessionWorkerWave::new(Arc::clone(&pids));
        for (index, session_name, op) in jobs.iter().cloned() {
            let (start_tx, start_rx) = mpsc::channel();
            let tx = tx.clone();
            let args = args.clone();
            let secret = secret.clone();
            let pids = Arc::clone(&pids);
            let nonce_set = Arc::clone(&nonce_set);
            let session_state =
                logical_mcp_slots[index].map(|slot| Arc::clone(&mcp_sessions[slot]));
            let gates = gates.clone();
            let load_window_deadline = load_window_deadline;
            let handle = thread::Builder::new()
                .stack_size(256 * 1024)
                .spawn(move || {
                    if start_rx.recv().is_err() {
                        return;
                    }
                    let result = if let Some(session_state) = session_state {
                        match session_state.lock() {
                            Ok(mut session) => run_operation(
                                &op,
                                &mut session,
                                &args,
                                &secret,
                                &nonce_set,
                                &pids,
                                &gates,
                                true,
                                load_window_deadline,
                                deadline,
                            ),
                            Err(_) => operation_error("MCP session pool poisoned"),
                        }
                    } else {
                        run_operation(
                            &op,
                            &mut McpSession::default(),
                            &args,
                            &secret,
                            &nonce_set,
                            &pids,
                            &gates,
                            true,
                            load_window_deadline,
                            deadline,
                        )
                    };
                    let _ = tx.send((index, session_name, op, result));
                })
                .map_err(|e| format!("bounded session worker start: {e}"))?;
            workers.push(handle, start_tx);
        }
        drop(tx);
        workers.start()?;
        let mut received = 0usize;
        let mut wave_results = Vec::with_capacity(jobs.len());
        while received < jobs.len() {
            let (index, session_name, op, result) = rx
                .recv_timeout(remaining(deadline)?)
                .map_err(|e| format!("session result deadline: {e}"))?;
            wave_results.push((index, session_name, op, result));
            received += 1;
        }
        workers.join_all()?;
        let mut group_members: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (position, (_, _, op, _)) in wave_results.iter().enumerate() {
            if let Some(group) = &op.conflict_group {
                group_members
                    .entry(group.clone())
                    .or_default()
                    .push(position);
            }
        }
        let mut group_passed = BTreeMap::new();
        for (group, members) in group_members {
            let group_successes = members
                .iter()
                .filter(|i| wave_results[**i].3.successful)
                .count();
            let group_conflicts = members
                .iter()
                .filter(|i| wave_results[**i].3.conflict && wave_results[**i].3.passed)
                .count();
            let group_ok = members.len() >= 2
                && group_successes == 1
                && group_conflicts + group_successes == members.len()
                && members.iter().all(|i| wave_results[*i].3.passed);
            conflict_groups += 1;
            if !group_ok {
                conflict_group_failures += 1;
            }
            group_passed.insert(group, group_ok);
        }
        for (index, session_name, op, mut result) in wave_results {
            let group_ok = op
                .conflict_group
                .as_ref()
                .map(|group| group_passed.get(group).copied().unwrap_or(false));
            let passed = result.passed && group_ok.unwrap_or(true);
            result.value["response_matches_oracle"] = json!(result.passed);
            result.value["conflict_group_passed"] = json!(group_ok);
            result.value["passed"] = json!(passed);
            if passed && result.successful && native_mutation_operation(&op) {
                if let Some(command_id) = op.request["command_id"].as_str() {
                    successful_load_mutations.insert(command_id.to_owned());
                    if let Some(response) = result.response_body.clone() {
                        successful_load_mutation_responses.insert(command_id.to_owned(), response);
                    }
                }
            }
            if result.not_started {
                load_operations_skipped += 1;
                load_window_closed = true;
            } else {
                load_operations_started += 1;
                *started_channel_mix.entry(op.channel.clone()).or_default() += 1;
                load_latencies.push(result.elapsed_ms);
                if passed {
                    successes += 1;
                    load_operations_passed += 1;
                    *passed_channel_mix.entry(op.channel.clone()).or_default() += 1;
                } else {
                    errors += 1;
                    load_operations_failed += 1;
                    *failed_channel_mix.entry(op.channel.clone()).or_default() += 1;
                }
                if op.channel == "projection" {
                    let navigation = &result.value["projection_navigation_advance"];
                    if let Some(observation) = navigation.as_object() {
                        projection_navigation_advance_samples += 1;
                        if observation["advanced"] == true {
                            projection_navigation_advanced += 1;
                        }
                        if observation["idle_current"] == true {
                            projection_navigation_idle_current += 1;
                        }
                        if observation["more_pending_prefix"] == true {
                            projection_navigation_more_pending_prefix += 1;
                        }
                        if observation["replayed"] == true {
                            projection_navigation_replayed += 1;
                        }
                        if let Some(lag) = observation
                            .get("sequence_lag_before")
                            .and_then(Value::as_u64)
                        {
                            projection_navigation_max_sequence_lag_before = Some(
                                projection_navigation_max_sequence_lag_before
                                    .unwrap_or(0)
                                    .max(lag),
                            );
                        }
                        if let Some(lag) = observation
                            .get("sequence_lag_after")
                            .and_then(Value::as_u64)
                        {
                            projection_navigation_max_sequence_lag_after = Some(
                                projection_navigation_max_sequence_lag_after
                                    .unwrap_or(0)
                                    .max(lag),
                            );
                        }
                    }
                    let queue_age = &result.value["projection_queue_age"];
                    if let Some(observation) = queue_age.as_object() {
                        projection_queue_age_samples += 1;
                        if let Some(status) = observation
                            .get("oldest_unprocessed_age_status")
                            .and_then(Value::as_str)
                        {
                            *projection_queue_age_status_counts
                                .entry(status.to_owned())
                                .or_default() += 1;
                        }
                        if let Some(lag) = observation.get("sequence_lag").and_then(Value::as_u64) {
                            projection_max_sequence_lag = projection_max_sequence_lag.max(lag);
                        }
                        if let Some(age) = observation
                            .get("oldest_unprocessed_age_millis")
                            .and_then(Value::as_u64)
                        {
                            projection_max_known_queue_age_millis =
                                Some(projection_max_known_queue_age_millis.unwrap_or(0).max(age));
                        }
                    }
                }
            }
            if result.conflict {
                conflicts += 1;
            }
            retries += result.retry_count;
            reconnects += result.reconnect_count;
            projection_interruptions += result.projection_interruptions;
            projection_resumes += result.projection_resumes;
            projection_status_checks += result.projection_status_checks;
            if !result.not_started {
                all_latencies.push(result.elapsed_ms);
                if passed && result.successful {
                    latencies.push(result.elapsed_ms);
                }
            }
            let is_transport_error = result.value["transport_error"] == true;
            let mut value = result.value;
            value["session"] = json!(session_name);
            value["elapsed_seconds"] = json!(result.elapsed_ms / 1000.0);
            value["independent_command_id"] =
                json!(op.request.get("command_id").and_then(Value::as_str));
            value["event"] = json!("request");
            value["index"] = json!(step * args.sessions + index);
            value["outcome"] = json!(if result.not_started {
                "not-started-load-window"
            } else if passed && result.conflict {
                "conflict-response"
            } else if passed {
                "exact-response"
            } else if group_ok == Some(false) {
                "conflict-group-invariant-failed"
            } else if result.conflict {
                "conflict-response-mismatch"
            } else if is_transport_error {
                "transport-error"
            } else {
                "response-mismatch"
            });
            emit(&mut output, &mut written, args.output_cap, &value)?;
        }
        if load_window_closed {
            break;
        }
    }
    let load_elapsed_seconds = load_started_at.elapsed().as_secs_f64();
    let load_resources_after = resource_snapshot(&cgroup)?;
    let load_resource_delta = resource_delta(&load_resources_before, &load_resources_after);
    let navigation_advance_template = plan
        .sessions
        .iter()
        .take(args.sessions)
        .flat_map(|session| &session.operations)
        .find(|operation| {
            operation.channel == "projection"
                && operation.request["operation"] == "advance-navigation"
        })
        .cloned();
    let postload_navigation_drain = if let Some(template) = navigation_advance_template {
        let drain = if setup_failures == 0 {
            navigation_quiescent_drain(
                &template,
                &args,
                &pids,
                &cgroup,
                &resources_before,
                deadline,
            )
        } else {
            json!({
                "phase":"quiescent-postload-drain",
                "passed":false,
                "complete":false,
                "steps":0,
                "skipped":"serial setup did not complete"
            })
        };
        if drain["passed"] != true && setup_failures == 0 {
            errors += 1;
        }
        emit(
            &mut output,
            &mut written,
            args.output_cap,
            &json!({"event":"projection_postload","navigation_drain":drain}),
        )?;
        drain
    } else {
        Value::Null
    };
    let mut postload_checks_attempted = 0usize;
    let mut postload_checks_passed = 0usize;
    let mut postload_checks_failed = 0usize;
    let mut postload_integrity_passed = 0usize;
    let mut postload_selected_integrity_passed = 0usize;
    let mut postload_selected_vector_integrity_passed = 0usize;
    let mut postload_sdk_pg_source_integrity_passed = 0usize;
    let mut postload_pg_integrity_passed = 0usize;
    let mut postload_rights_passed = 0usize;
    let mut postload_rights_revocation_passed = 0usize;
    let mut postload_rights_denial_passed = 0usize;
    let mut postload_idempotency_passed = 0usize;
    let mut postload_projection_state_passed = 0usize;
    let postload_started_at = Instant::now();
    let mut postload_results = BTreeMap::<String, Value>::new();
    let mut postload_mcp_session = McpSession::default();
    let mut postload_mcp_initialized = false;
    for (index, template) in plan.postload.iter().enumerate() {
        let kind = postload_check_kind(template);
        let mut op = template.clone();
        if let Some(target) = op.idempotency_replay_of.as_deref()
            && let Some(response) = successful_load_mutation_responses.get(target)
        {
            op.bound_replay_response = Some(response.clone());
        }
        let mut result = if setup_failures != 0 {
            operation_error("serial setup did not complete")
        } else if STOP.load(Ordering::Acquire) || Instant::now() >= deadline {
            operation_error("postload verification deadline/cancellation")
        } else if let Some(target) = op.idempotency_replay_of.as_deref()
            && !successful_load_mutations.contains(target)
        {
            operation_error(
                "idempotency replay target did not complete successfully during measured load",
            )
        } else if postload_rights_revoke_operation(&op)
            && native_pg_stage
            && (postload_checks_failed != 0
                || postload_selected_integrity_passed == 0
                || postload_pg_integrity_passed == 0
                || postload_idempotency_passed == 0
                || postload_rights_passed == 0)
        {
            postload_checks_attempted += 1;
            operation_skipped("pre-revoke-verification-failed")
        } else if let Err(reason) = if postload_rights_revoke_operation(&op) {
            bind_postload_rights_revoke(&mut op, &postload_results)
        } else {
            bind_operation_from_setup(&mut op, &setup_results)
        } {
            operation_error(&reason)
        } else if op.channel == "mcp" && !postload_mcp_initialized {
            postload_checks_attempted += 1;
            match initialize_mcp(args.mcp_port, deadline, args.response_cap) {
                Ok((session, status, digest, elapsed)) if status == 200 => {
                    postload_mcp_session = session;
                    postload_mcp_initialized = true;
                    emit(
                        &mut output,
                        &mut written,
                        args.output_cap,
                        &json!({"event":"postload_mcp_session_ready","initialize_status":status,"initialize_sha256":digest,"initialize_elapsed_ms":elapsed}),
                    )?;
                    run_operation(
                        &op,
                        &mut postload_mcp_session,
                        &args,
                        &secret,
                        &nonce_set,
                        &pids,
                        &gates,
                        false,
                        None,
                        deadline,
                    )
                }
                Ok((_, status, _, _)) => {
                    operation_error(&format!("postload MCP initialize status {status}"))
                }
                Err(reason) => operation_error(&reason),
            }
        } else {
            postload_checks_attempted += 1;
            run_operation(
                &op,
                &mut postload_mcp_session,
                &args,
                &secret,
                &nonce_set,
                &pids,
                &gates,
                false,
                None,
                deadline,
            )
        };
        let passed = result.passed && (result.successful || postload_projection_rights_denial(&op));
        if passed {
            if !postload_projection_rights_denial(&op) {
                successes += 1;
            }
            postload_checks_passed += 1;
            match kind {
                "source_integrity" => postload_integrity_passed += 1,
                "postgresql_source_integrity" => postload_integrity_passed += 1,
                "sdk_postgresql_source_integrity" => postload_integrity_passed += 1,
                "rights_state" => postload_rights_passed += 1,
                "rights_revocation" => postload_rights_revocation_passed += 1,
                "rights_denial" => postload_rights_denial_passed += 1,
                "idempotency_replay" => postload_idempotency_passed += 1,
                "projection_state" => postload_projection_state_passed += 1,
                _ => {}
            }
            if postload_pg_source_integrity(&op) {
                postload_pg_integrity_passed += 1;
            }
            if postload_selected_source_integrity(&op) {
                postload_selected_integrity_passed += 1;
            }
            if postload_selected_vector_integrity(&op) {
                postload_selected_vector_integrity_passed += 1;
            }
            if postload_sdk_pg_source_integrity(&op) {
                postload_sdk_pg_source_integrity_passed += 1;
            }
            if let Some(response) = result.response_body.clone() {
                postload_results.insert(
                    op.label.clone(),
                    json!({"request":op.request.clone(),"sdk_argv":op.sdk_argv.clone(),"response":response}),
                );
            }
        } else {
            postload_checks_failed += 1;
            errors += 1;
        }
        result.value["event"] = json!("postload_check");
        result.value["phase"] = json!("postload-verification");
        result.value["kind"] = json!(kind);
        result.value["index"] = json!(index);
        result.value["passed"] = json!(passed);
        result.value["idempotency_replay_of"] = json!(op.idempotency_replay_of.clone());
        result.value["expected_stderr_prefix"] = json!(op.expected_stderr_prefix.clone());
        result.value["rights_allowed_expected"] = op
            .expected_rights_allowed
            .map(Value::Bool)
            .unwrap_or(Value::Null);
        result.value["rights_allowed_actual"] = result
            .response_body
            .as_ref()
            .and_then(rights_allowed_value)
            .map(Value::Bool)
            .unwrap_or(Value::Null);
        result.value["postgresql_owner_integrity_check"] = json!(postload_pg_source_integrity(&op));
        emit(&mut output, &mut written, args.output_cap, &result.value)?;
    }
    let postload_elapsed_seconds = postload_started_at.elapsed().as_secs_f64();
    let load_operations_not_started =
        load_operations_scheduled.saturating_sub(load_operations_started);
    let mut not_started_channel_mix = scheduled_channel_mix.clone();
    for (channel, count) in &started_channel_mix {
        let remaining = not_started_channel_mix.entry(channel.clone()).or_default();
        *remaining = remaining.saturating_sub(*count);
    }
    not_started_channel_mix.retain(|_, count| *count != 0);
    for state in &mcp_sessions {
        if let Ok(state) = state.lock()
            && let Some(id) = state.id.as_deref()
        {
            let _ = delete_mcp(args.mcp_port, id, deadline, args.response_cap);
        }
    }
    if let Some(child) = mcp_server.as_mut() {
        stop_child(child);
    }
    if let Some(child) = owner_server.as_mut() {
        stop_child(child);
    }
    watchdog_stop.store(true, Ordering::Release);
    let _ = watchdog.join();
    let input_after = paths
        .iter()
        .map(|p| file_stamp(p, deadline))
        .collect::<Result<Vec<_>, _>>()?;
    if input_stamps
        .iter()
        .zip(&input_after)
        .any(|(a, b)| !stamp_equal(a, b))
    {
        return Err("selected input changed during workload".into());
    }
    let mut schedule_after = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&args.schedule)
        .map_err(|e| format!("schedule recheck open: {e}"))?;
    let mut schedule_after_bytes = Vec::new();
    schedule_after
        .take(args.schedule_cap as u64 + 1)
        .read_to_end(&mut schedule_after_bytes)
        .map_err(|e| format!("schedule recheck read: {e}"))?;
    if schedule_after_bytes.len() > args.schedule_cap
        || sha(&schedule_after_bytes) != schedule_sha256
    {
        return Err("schedule content changed during workload".into());
    }
    let resources_after = resource_snapshot(&cgroup)?;
    let resource_delta = resource_delta(&resources_before, &resources_after);
    let memory_headroom_stop_observed = resources_after
        .memory_limit
        .map(|limit| resources_after.memory_peak as u128 * 10 >= limit as u128 * 9);
    let resource_stop_observed = memory_headroom_stop_observed.unwrap_or(false)
        || [
            "memory_high_events_delta",
            "memory_max_events_delta",
            "memory_oom_events_delta",
            "memory_oom_kill_events_delta",
        ]
        .into_iter()
        .any(|key| resource_delta[key].as_u64().is_some_and(|delta| delta > 0));
    latencies.sort_by(f64::total_cmp);
    let quantile = |q: f64| -> Option<f64> {
        if latencies.is_empty() {
            None
        } else {
            Some(
                latencies[((q * latencies.len() as f64).ceil() as usize)
                    .saturating_sub(1)
                    .min(latencies.len() - 1)],
            )
        }
    };
    all_latencies.sort_by(f64::total_cmp);
    let all_quantile = |q: f64| -> Option<f64> {
        if all_latencies.is_empty() {
            None
        } else {
            Some(
                all_latencies[((q * all_latencies.len() as f64).ceil() as usize)
                    .saturating_sub(1)
                    .min(all_latencies.len() - 1)],
            )
        }
    };
    load_latencies.sort_by(f64::total_cmp);
    let load_quantile = |q: f64| -> Option<f64> {
        if load_latencies.is_empty() {
            None
        } else {
            Some(
                load_latencies[((q * load_latencies.len() as f64).ceil() as usize)
                    .saturating_sub(1)
                    .min(load_latencies.len() - 1)],
            )
        }
    };
    let storage_bytes = output
        .metadata()
        .map_err(|e| format!("report metadata: {e}"))?
        .len();
    let storage_allocated = output
        .metadata()
        .map_err(|e| format!("report metadata: {e}"))?
        .blocks()
        .saturating_mul(512);
    // Keep the report shape flat while splitting macro expansion into bounded chunks.
    let merge_finish_fields = |report: &mut Value, fields: Value| {
        let Value::Object(fields) = fields else {
            unreachable!("finish report chunks are objects");
        };
        report
            .as_object_mut()
            .expect("finish report is an object")
            .extend(fields);
    };
    let mut finish_report = json!({
        "event": "finish",
        "passed": errors == 0 && load_operations_not_started == 0 && !resource_stop_observed,
        "selected_file_metadata_unchanged": true,
        "schedule_unchanged": true,
        "scheduled": total_ops,
        "serial_setup_scheduled": plan.setup.len(),
        "postload_checks_scheduled": plan.postload.len(),
        "postload_checks_attempted": postload_checks_attempted,
        "postload_checks_passed": postload_checks_passed,
        "postload_checks_failed": postload_checks_failed,
        "postload_source_integrity_passed": postload_integrity_passed,
        "postload_selected_source_integrity_passed": postload_selected_integrity_passed,
        "postload_selected_vector_integrity_passed": postload_selected_vector_integrity_passed,
        "postload_sdk_pg_source_integrity_passed": postload_sdk_pg_source_integrity_passed,
        "postload_native_pg_integrity_passed": postload_pg_integrity_passed,
        "postload_rights_state_passed": postload_rights_passed,
        "postload_rights_revocations_passed": postload_rights_revocation_passed,
        "postload_rights_denials_passed": postload_rights_denial_passed,
    });
    merge_finish_fields(
        &mut finish_report,
        json!({
            "postload_idempotency_replays_passed": postload_idempotency_passed,
            "postload_projection_state_passed": postload_projection_state_passed,
            "postload_verification_elapsed_seconds": postload_elapsed_seconds,
            "serial_setup_attempted": setup_successes + setup_failures,
            "serial_setup_completed": setup_successes,
            "serial_setup_failures": setup_failures,
            "serial_setup_elapsed_seconds": setup_elapsed_seconds,
            "fresh_pg_stage": fresh_pg_stage,
            "native_pg_stage": native_pg_stage,
            "pg_stage_mode": match args.pg_stage_mode {
                PgStageMode::Fresh => "fresh",
                PgStageMode::Existing => "existing",
            },
            "postgresql_url_reported": false,
            "database_reset_or_drop_performed": false,
            "cleanup_policy": "runner stops only its child process groups and retains the private staged files and report; caller owns disposable PostgreSQL database cleanup",
            "logical_sessions": args.sessions,
            "load_operation_count_limit": args.operation_count,
            "load_operations_scheduled": load_operations_scheduled,
            "load_operations_started": load_operations_started,
        }),
    );
    merge_finish_fields(
        &mut finish_report,
        json!({
            "load_operations_passed": load_operations_passed,
            "load_operations_failed": load_operations_failed,
            "load_operations_not_started": load_operations_not_started,
            "load_operations_skipped_before_dispatch": load_operations_skipped,
            "requested_channel_mix_percent": args.channel_mix.as_ref(),
            "load_channel_mix": {
                "scheduled": scheduled_channel_mix,
                "started": started_channel_mix,
                "passed": passed_channel_mix,
                "failed": failed_channel_mix,
                "not_started": not_started_channel_mix
            },
            "operations_per_second_limit": args.operations_per_second,
            "load_window_seconds_limit": args.load_window_seconds,
            "load_window_closed": load_window_closed,
            "load_window_incomplete": load_operations_not_started != 0,
            "resource_stop_observed": resource_stop_observed,
            "memory_headroom_threshold_available": memory_headroom_stop_observed.is_some(),
            "memory_peak_at_or_above_90_percent_of_limit": memory_headroom_stop_observed,
            "load_window_elapsed_seconds": load_elapsed_seconds,
            "load_operations_per_second": if load_elapsed_seconds > 0.0 {
                Some(load_operations_started as f64 / load_elapsed_seconds)
            } else {
                None
            },
        }),
    );
    merge_finish_fields(
        &mut finish_report,
        json!({
            "load_successful_operations_per_second": if load_elapsed_seconds > 0.0 {
                Some(load_operations_passed as f64 / load_elapsed_seconds)
            } else {
                None
            },
            "load_operation_latency_ms": {
                "samples": load_latencies.len(),
                "p50": load_quantile(0.50),
                "p95": load_quantile(0.95),
                "p99": load_quantile(0.99),
                "max": load_latencies.last().copied()
            },
            "logical_mcp_sessions": logical_mcp_sessions,
            "physical_mcp_sessions": physical_mcp_session_limit,
            "channel_in_flight_limits": {
                "mcp": 32,
                "source_reads": 32,
                "owner_http": 4,
                "sdk_source_commands": args.sdk_in_flight,
                "source_projection_processes": args.sdk_in_flight
            },
            "scheduled_source_read_operations": source_read_operations,
            "scheduled_sdk_source_commands": sdk_source_command_operations,
            "scheduled_projection_operations": projection_operations,
            "serial_projection_setup_operations": projection_setup_operations,
            "started": successes + errors,
            "failures": errors,
            "conflicts": conflicts,
            "conflict_groups": conflict_groups,
            "conflict_group_failures": conflict_group_failures,
            "retries": retries,
        }),
    );
    merge_finish_fields(
        &mut finish_report,
        json!({
            "reconnects": reconnects,
            "projection_interruptions": projection_interruptions,
            "projection_resumes": projection_resumes,
            "projection_status_checks": projection_status_checks,
            "projection_queue_age": {
                "samples": projection_queue_age_samples,
                "oldest_unprocessed_age_status_counts": projection_queue_age_status_counts,
                "max_sequence_lag": if projection_queue_age_samples > 0 { Some(projection_max_sequence_lag) } else { None },
                "max_known_oldest_unprocessed_age_millis": projection_max_known_queue_age_millis
            },
            "projection_navigation": {
                "bootstrap_setup_operations": projection_navigation_bootstrap_operations,
                "bootstrap_current_observations": projection_navigation_bootstrap_observations,
                "advance_samples": projection_navigation_advance_samples,
                "advanced_samples": projection_navigation_advanced,
                "idle_current_samples": projection_navigation_idle_current,
                "more_pending_prefix_samples": projection_navigation_more_pending_prefix,
                "replayed_samples": projection_navigation_replayed,
                "max_sequence_lag_before": projection_navigation_max_sequence_lag_before,
                "max_sequence_lag_after": projection_navigation_max_sequence_lag_after,
                "quiescent_postload_drain": postload_navigation_drain,
                "oldest_unprocessed_age_status": if projection_navigation_operations > 0 { Some("not_exposed_by_navigation_owner") } else { None }
            },
            "all_operation_latency_ms": {
                "samples": all_latencies.len(),
                "p50": all_quantile(0.50),
                "p95": all_quantile(0.95),
                "p99": all_quantile(0.99),
                "max": all_latencies.last().copied()
            },
            "successful_latency_ms": {
                "samples": latencies.len(),
                "p50": quantile(0.50),
                "p95": quantile(0.95),
                "p99": quantile(0.99),
                "max": latencies.last().copied()
            },
            "elapsed_seconds": start_time.elapsed().as_secs_f64(),
            "operation_window_seconds": start_metrics.elapsed().as_secs_f64(),
            "resource_delta": resource_delta,
            "load_resource_delta": load_resource_delta,
            "report_storage_bytes_before_finish": storage_bytes,
            "report_storage_allocated_bytes_before_finish": storage_allocated,
            "scope": "finite Rust protocol measurement; not capacity acceptance"

        }),
    );
    emit(&mut output, &mut written, args.output_cap, &finish_report)?;
    output.sync_all().map_err(|e| format!("report sync: {e}"))?;
    Ok(
        if errors == 0 && load_operations_not_started == 0 && !resource_stop_observed {
            0
        } else {
            1
        },
    )
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_POSTLOAD_SOURCE_READ_WORK_VALUE, Operation, RECORD_REVISE_RESULT_SCHEMA,
        RECORD_REVISE_SEMANTIC_ORACLE_SCHEMA, copy_stage_tree, exact_source_projection_operation,
        exact_source_projection_response_matches, record_revise_semantic_oracle_valid,
        record_revise_semantic_replay_response_matches, record_revise_semantic_response_matches,
        sha,
    };
    use serde_json::{Value, json};
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_TEMP_ID: AtomicUsize = AtomicUsize::new(0);

    fn source_tree() -> (PathBuf, PathBuf) {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "tos-load-readiness-stage-budget-{}-{nonce}-{}",
            std::process::id(),
            NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        let source = root.join("source");
        fs::create_dir_all(source.join("nested")).expect("create source tree");
        fs::write(source.join("root.bin"), b"root").expect("write root fixture");
        fs::write(source.join("nested/inner.bin"), b"nest").expect("write nested fixture");
        (root, source)
    }

    fn revision_oracle_fixture() -> (PathBuf, Operation, Value, Value) {
        let (root, _) = source_tree();
        let owner_path = root.join("owner.json");
        let invocation_path = root.join("invocation.json");
        let source_path = "ToS/source-witnesses/item.json";
        let domain = "tos-test-record-revision";
        let command_id = "record-revise-1";
        let previous_revision = format!("sha256:{}", "a".repeat(64));
        let configuration = format!("sha256:{}", "b".repeat(64));
        let dependencies = format!("sha256:{}", "c".repeat(64));
        let owner = json!({
            "schema_version":"tos_local_pg_record_revision_owner_v1",
            "record_id":"record-test",
            "source_path":source_path,
            "principal_id":"principal-test",
            "authority_ref":"authority-test"
        });
        fs::write(&owner_path, serde_json::to_vec(&owner).unwrap()).unwrap();
        fs::set_permissions(&owner_path, fs::Permissions::from_mode(0o600)).unwrap();
        let invocation = json!({
            "schema_version":"tos_local_pg_record_revision_invocation_v1",
            "owner_config":owner_path,
            "source_selection":{"domain":domain}
        });
        fs::write(&invocation_path, serde_json::to_vec(&invocation).unwrap()).unwrap();
        fs::set_permissions(&invocation_path, fs::Permissions::from_mode(0o600)).unwrap();

        let old_source = json!({"record_id":"record-test","title":"before"});
        let new_source = json!({"record_id":"record-test","title":"after"});
        let selected_files = json!([
            "item.human-forms.json",
            "item.json",
            "revision-history.json"
        ]);
        let work = json!({
            "scope":"bounded PG/STO source selection; software capture and physical I/O excluded",
            "exact_source_members_read":1,
            "exact_source_bytes_read":512,
            "historical_source_members_read":1,
            "historical_source_bytes_read":512,
            "custody_metadata_bytes_read":128,
            "segment_bytes_read":512,
            "committed_member_rows_examined":2,
            "metadata_member_lookups":1,
            "directory_index_rows_examined":2,
            "unique_index_lookups":1,
            "text_identity_lookups":1
        });
        let preview_result = json!({
            "schema_version":RECORD_REVISE_RESULT_SCHEMA,
            "authentication":"local-unix-account",
            "owner_configuration":configuration,
            "source_path":source_path,
            "source":old_source,
            "revision":previous_revision,
            "command_operations":["describe","prepare-revise","record.revise"],
            "supported_operations":["record.revise"],
            "allowed_operations":["record.revise"],
            "allowed_fields":["title"],
            "allowed_form_ids":[],
            "receipt":null,
            "replayed":false,
            "grants_admission":false,
            "materializations":[],
            "publication_snapshot":null,
            "publication_protocol":"tos-selected-source-publication-v1",
            "selected_files":selected_files,
            "recovery":null,
            "record_type":"work",
            "source_profile":{"record_type":"work"},
            "source_read_work":work,
            "prepared_source":new_source,
            "prepared_forms":[],
            "prepared_materializations":[],
            "expected_dependencies":dependencies,
            "expected_publication":null
        });
        let preview = json!({
            "schema_version":"tos_local_pg_record_revision_result_v1",
            "result":preview_result,
            "grants_admission":false
        });
        let request = json!({
            "schema_version":"tos_local_source_command_v1",
            "operation":"record.revise",
            "command_id":command_id,
            "reason":"bounded oracle test",
            "fields":{"title":"after"},
            "forms":[],
            "expected_configuration":configuration,
            "expected_source":old_source,
            "expected_revision":previous_revision,
            "expected_dependencies":dependencies,
            "expected_publication":null
        });
        let expected = json!({
            "schema_version":RECORD_REVISE_SEMANTIC_ORACLE_SCHEMA,
            "result_schema_version":RECORD_REVISE_RESULT_SCHEMA,
            "domain":domain,
            "source_path":source_path,
            "command_id":command_id,
            "previous_revision":previous_revision
        });
        let op = Operation {
            channel: "sdk".into(),
            label: "revision".into(),
            request,
            expected_status: 0,
            expected_sha256: None,
            expected_body: Some(expected.clone()),
            expected_stderr_prefix: None,
            expected_rights_allowed: None,
            ignored_json_paths: Vec::new(),
            alternate_outcomes: Vec::new(),
            retry_same_command_id: false,
            reconnect_before: false,
            conflict_group: None,
            bind_expected_configuration_from_owner: false,
            sdk_argv: vec![
                "/bin/fixture".into(),
                "source-commands".into(),
                "--invocation".into(),
                invocation_path.to_string_lossy().into_owned(),
            ],
            projection_argv: Vec::new(),
            interrupt_resume_after_ms: None,
            bind_expected_from: Some("prepare-revise".into()),
            idempotency_replay_of: None,
            bound_revision_preview: Some(preview),
            bound_replay_response: None,
        };
        let mut result = preview_result;
        let result_object = result.as_object_mut().unwrap();
        for key in [
            "prepared_source",
            "prepared_forms",
            "prepared_materializations",
            "expected_dependencies",
            "expected_publication",
        ] {
            result_object.remove(key);
        }
        result["source"] = new_source.clone();
        result["revision"] = json!(format!("sha256:{}", "d".repeat(64)));
        result["materializations"] = json!([]);
        result["source_read_work"] = work;
        result["receipt"] = json!({
            "command_id":command_id,
            "request_digest":format!("sha256:{}", "e".repeat(64)),
            "principal_id":"principal-test",
            "authority_ref":"authority-test",
            "owner_configuration":configuration,
            "recorded_at":"2026-10-13T00:00:00Z",
            "reason":"bounded oracle test",
            "previous_source":old_source,
            "source":new_source,
            "previous_revision":previous_revision,
            "archive_path":format!(
                "ToS/source-witnesses/.record-revisions/{}-{}",
                sha(b"record-test"),
                previous_revision.trim_start_matches("sha256:")
            ),
            "dependencies":dependencies,
            "changed_fields":["title"],
            "forms":[],
            "grants_admission":false,
            "request":op.request,
            "publication":{
                "protocol":"tos-selected-source-publication-v1",
                "transaction_id":format!("sha256:{}", "f".repeat(64)),
                "selected_files":selected_files
            }
        });
        result["durable_commit"] = json!({
            "domain":domain,
            "command_id":command_id,
            "prepare_id_sha256":format!("sha256:{}", "1".repeat(64)),
            "commit_seq":9,
            "request_sha256":format!("sha256:{}", "2".repeat(64)),
            "delta_sha256":format!("sha256:{}", "3".repeat(64)),
            "member_root_sha256":format!("sha256:{}", "4".repeat(64)),
            "replayed":false
        });
        let actual = json!({
            "schema_version":"tos_local_pg_record_revision_result_v1",
            "result":result,
            "grants_admission":false
        });
        (root, op, expected, actual)
    }

    #[test]
    fn owner_stage_clone_refuses_tree_above_selected_cap() {
        let (root, source) = source_tree();
        let mut files = 0;
        let mut bytes = 0;
        let error = copy_stage_tree(&source, &root.join("clone"), &mut files, &mut bytes, 7)
            .expect_err("8-byte recursive tree must exceed a 7-byte cap");
        assert!(error.contains("selected clone byte cap (7 bytes)"));
        fs::remove_dir_all(root).expect("remove rejected fixture");
    }

    #[test]
    fn owner_stage_clone_copies_recursive_tree_at_selected_cap() {
        let (root, source) = source_tree();
        let destination = root.join("clone");
        let mut files = 0;
        let mut bytes = 0;
        copy_stage_tree(&source, &destination, &mut files, &mut bytes, 8)
            .expect("8-byte recursive tree fits an 8-byte cap");
        assert_eq!(files, 2);
        assert_eq!(bytes, 8);
        assert_eq!(fs::read(destination.join("root.bin")).unwrap(), b"root");
        assert_eq!(
            fs::read(destination.join("nested/inner.bin")).unwrap(),
            b"nest"
        );
        fs::remove_dir_all(root).expect("remove accepted fixture");
    }

    #[test]
    fn native_pg_record_revise_semantic_oracle_accepts_preview_bound_commit() {
        let (root, op, expected, actual) = revision_oracle_fixture();
        assert!(record_revise_semantic_oracle_valid(&op, &expected));
        assert!(record_revise_semantic_response_matches(
            &op, &expected, &actual
        ));
        fs::remove_dir_all(root).expect("remove oracle fixture");
    }

    fn exact_source_projection_fixture(channel: &str) -> (Operation, Value, Value) {
        let expected = json!({
            "schema_version":"tos_source_projection_exact_v1",
            "domain":"tos-test-source",
            "source_path":"ToS/source-witnesses/item.json",
            "revision":7,
            "bytes":3,
            "sha256":"a".repeat(64),
            "source_read_work":{
                "custody_metadata_bytes_read":0,
                "segment_bytes_read":0,
                "committed_member_rows_examined":0
            },
            "content_base64":"YWJj"
        });
        let mut actual = expected.clone();
        actual["source_read_work"]["custody_metadata_bytes_read"] = json!(2);
        actual["source_read_work"]["segment_bytes_read"] = json!(3);
        actual["source_read_work"]["committed_member_rows_examined"] = json!(1);
        let operation = Operation {
            channel: channel.into(),
            label: "exact-source".into(),
            request: json!({
                "operation":"exact-source",
                "source_path":"ToS/source-witnesses/item.json",
                "revision":7
            }),
            expected_status: 0,
            expected_sha256: None,
            expected_body: Some(expected.clone()),
            expected_stderr_prefix: None,
            expected_rights_allowed: None,
            ignored_json_paths: Vec::new(),
            alternate_outcomes: Vec::new(),
            retry_same_command_id: false,
            reconnect_before: false,
            conflict_group: None,
            bind_expected_configuration_from_owner: false,
            sdk_argv: if channel == "sdk" {
                vec![
                    "/bin/tos-native-owner-command".into(),
                    "source-projection".into(),
                    "--selection".into(),
                    "/tmp/source-selection.json".into(),
                ]
            } else {
                Vec::new()
            },
            projection_argv: if channel == "projection" {
                vec![
                    "/bin/tos-native-owner-command".into(),
                    "source-projection".into(),
                    "--selection".into(),
                    "/tmp/source-selection.json".into(),
                ]
            } else {
                Vec::new()
            },
            interrupt_resume_after_ms: None,
            bind_expected_from: None,
            idempotency_replay_of: None,
            bound_revision_preview: None,
            bound_replay_response: None,
        };
        (operation, expected, actual)
    }

    #[test]
    fn selected_source_projection_accepts_dynamic_bounded_work_on_both_routes() {
        for channel in ["sdk", "projection"] {
            let (operation, expected, actual) = exact_source_projection_fixture(channel);
            assert!(exact_source_projection_operation(&operation));
            assert!(exact_source_projection_response_matches(
                &operation, &expected, &actual
            ));

            let mut over_budget = actual.clone();
            over_budget["source_read_work"]["segment_bytes_read"] =
                json!(MAX_POSTLOAD_SOURCE_READ_WORK_VALUE + 1);
            assert!(!exact_source_projection_response_matches(
                &operation,
                &expected,
                &over_budget
            ));
        }
    }

    #[test]
    fn native_pg_record_revise_semantic_oracle_rejects_wrong_source_command_and_receipt() {
        let (root, op, expected, actual) = revision_oracle_fixture();
        let mut wrong_source = actual.clone();
        wrong_source["result"]["source_path"] = json!("ToS/source-witnesses/other.json");
        assert!(!record_revise_semantic_response_matches(
            &op,
            &expected,
            &wrong_source
        ));

        let mut wrong_command = actual.clone();
        wrong_command["result"]["receipt"]["command_id"] = json!("other-command");
        assert!(!record_revise_semantic_response_matches(
            &op,
            &expected,
            &wrong_command
        ));

        let mut malformed_receipt = actual;
        malformed_receipt["result"]["receipt"]
            .as_object_mut()
            .unwrap()
            .remove("authority_ref");
        assert!(!record_revise_semantic_response_matches(
            &op,
            &expected,
            &malformed_receipt
        ));
        fs::remove_dir_all(root).expect("remove oracle fixture");
    }

    #[test]
    fn native_pg_record_revise_semantic_replay_allows_only_replay_flag_changes() {
        let (root, mut op, expected, original) = revision_oracle_fixture();
        op.idempotency_replay_of = Some("record-revise-1".into());
        op.bound_replay_response = Some(original.clone());
        let mut replay = original.clone();
        replay["result"]["replayed"] = json!(true);
        replay["result"]["durable_commit"]["replayed"] = json!(true);
        replay["result"]["source_read_work"]["custody_metadata_bytes_read"] = json!(130);
        replay["result"]["source_read_work"]["segment_bytes_read"] = json!(520);
        replay["result"]["source_read_work"]["committed_member_rows_examined"] = json!(3);
        assert!(record_revise_semantic_replay_response_matches(
            &op, &expected, &replay
        ));
        let mut over_budget = replay.clone();
        over_budget["result"]["source_read_work"]["segment_bytes_read"] =
            json!(MAX_POSTLOAD_SOURCE_READ_WORK_VALUE + 1);
        assert!(!record_revise_semantic_replay_response_matches(
            &op,
            &expected,
            &over_budget
        ));
        let mut wrong_scope = replay.clone();
        wrong_scope["result"]["source_read_work"]["scope"] = json!("unbounded");
        assert!(!record_revise_semantic_replay_response_matches(
            &op,
            &expected,
            &wrong_scope
        ));
        replay["result"]["receipt"]["source_path"] = json!("unexpected");
        assert!(!record_revise_semantic_replay_response_matches(
            &op, &expected, &replay
        ));
        fs::remove_dir_all(root).expect("remove replay fixture");
    }
}

fn main() {
    match main_result() {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("tos-load-readiness refused: {error}");
            std::process::exit(2);
        }
    }
}
