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
static STOP: AtomicBool = AtomicBool::new(false);

#[derive(Clone)]
struct Args {
    query_binary: PathBuf,
    owner_binary: Option<PathBuf>,
    source_root: Option<PathBuf>,
    source_inputs: Option<PathBuf>,
    model: PathBuf,
    binding: PathBuf,
    owner_config: Option<PathBuf>,
    invocation: Option<PathBuf>,
    token_file: Option<PathBuf>,
    owner_stage_baseline: Option<PathBuf>,
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

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Plan {
    schema: String,
    #[serde(default)]
    setup: Vec<Operation>,
    sessions: Vec<SessionPlan>,
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

    fn acquire(&self, deadline: Instant) -> Result<WavePermit, String> {
        loop {
            if STOP.load(Ordering::Acquire) || Instant::now() >= deadline {
                return Err("bounded operation wave deadline/cancellation".into());
            }
            let mut active = self
                .active
                .lock()
                .map_err(|_| "bounded operation wave counter poisoned")?;
            if *active < self.limit {
                *active += 1;
                return Ok(WavePermit {
                    active: Arc::clone(&self.active),
                });
            }
            drop(active);
            thread::sleep(Duration::from_millis(2));
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
            if load_window_deadline.is_some_and(|window| now >= window) {
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

fn parse_args() -> Result<Args, String> {
    let mut values = BTreeMap::new();
    let mut args = std::env::args().skip(1);
    while let Some(key) = args.next() {
        if key == "--help" || key == "-h" {
            return Err("usage: tos-load-readiness --query-binary ABS --model ABS --binding ABS --schedule ABS --output DIR --unit UNIT [--source-root ABS --source-inputs ABS] [--owner-binary ABS --owner-config ABS --invocation ABS --token-file ABS [--owner-stage-baseline ABS]] [--mcp-port PORT] [--owner-port PORT] [--sessions 2|8|16|64|128|256] [--sdk-in-flight 1..4] [--operation-count N] [--channel-mix mcp=60,owner_http=20,sdk=20,projection=0] [--operations-per-second N] [--load-window-seconds N] [--deadline-seconds N] [--request-cap-bytes N] [--response-cap-bytes N] [--schedule-cap-bytes N] [--output-cap-bytes N]".into());
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
    let model = path(&mut values, "--model")?;
    let binding = path(&mut values, "--binding")?;
    let owner_config = optional_path(&mut values, "--owner-config")?;
    let invocation = optional_path(&mut values, "--invocation")?;
    let token_file = optional_path(&mut values, "--token-file")?;
    let owner_stage_baseline = optional_path(&mut values, "--owner-stage-baseline")?;
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
        .map(|value| {
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
        .map(|value| {
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
        .map(|value| {
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
        .map(|value| {
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
    Ok(Args {
        query_binary,
        owner_binary,
        source_root,
        source_inputs,
        model,
        binding,
        owner_config,
        invocation,
        token_file,
        owner_stage_baseline,
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
    })
}

fn sha(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

const MAX_STAGE_FILES: usize = 16_384;
const MAX_STAGE_FILE_BYTES: u64 = 33_554_432;
const MAX_STAGE_TOTAL_BYTES: u64 = 268_435_456;

fn copy_stage_tree(
    source: &Path,
    destination: &Path,
    files: &mut usize,
    bytes: &mut u64,
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
            copy_stage_tree(&entry.path(), &destination.join(name), files, bytes)?;
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
        .filter(|total| *total <= MAX_STAGE_TOTAL_BYTES)
        .ok_or("owner stage exceeds 256 MiB / 16384-file bound")?;
    if *files > MAX_STAGE_FILES {
        return Err("owner stage exceeds 256 MiB / 16384-file bound".into());
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
        .filter(|operation| operation.channel == "sdk")
    {
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
        .filter(|operation| operation.channel == "sdk")
    {
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

fn selected_projection_inputs(plan: &Plan, session_limit: usize) -> Result<Vec<PathBuf>, String> {
    let mut selected = BTreeSet::new();
    for operation in plan
        .sessions
        .iter()
        .take(session_limit)
        .flat_map(|session| &session.operations)
        .filter(|operation| operation.channel == "projection")
    {
        let paths = operation
            .projection_argv
            .windows(2)
            .filter(|pair| pair[0] == "--selection")
            .map(|pair| PathBuf::from(&pair[1]))
            .collect::<Vec<_>>();
        if paths.len() != 1 || !Path::new(&operation.projection_argv[0]).is_absolute() {
            return Err("projection route requires one explicit --selection path".into());
        }
        let selection = paths[0]
            .canonicalize()
            .map_err(|e| format!("projection selection identity: {e}"))?;
        let value = read_stage_json(&selection)?;
        if value["schema_version"] != "tos_source_projection_selection_v1"
            || value["store_root"].as_str().is_none()
        {
            return Err("projection selection must bind a source store".into());
        }
        selected.insert(selection);
    }
    Ok(selected.into_iter().collect())
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
    needs_mcp: bool,
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
    if needs_mcp && (args.source_root.is_none() || args.source_inputs.is_none()) {
        return Err("MCP workload staging requires --source-root and --source-inputs".into());
    }
    let mut staged_inputs = vec![args.model.as_path(), args.binding.as_path()];
    if let Some(source_root) = args.source_root.as_deref() {
        staged_inputs.push(source_root);
    }
    if let Some(source_inputs) = args.source_inputs.as_deref() {
        staged_inputs.push(source_inputs);
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
            return Err("model, binding and all owner inputs must be inside the selected owner stage baseline".into());
        }
    }
    for (invocation, owner) in &sdk_owner_inputs {
        if !invocation.starts_with(&baseline) || !owner.starts_with(&baseline) {
            return Err("every SDK invocation and owner configuration must be inside the selected owner stage baseline".into());
        }
    }
    for store_root in &sdk_store_roots {
        if !store_root.starts_with(&baseline) {
            return Err("SDK PostgreSQL store must be inside the selected stage baseline".into());
        }
    }
    for selection in &projection_inputs {
        let value = read_stage_json(selection)?;
        let store_root = PathBuf::from(
            value["store_root"]
                .as_str()
                .ok_or("projection selected store root absent")?,
        )
        .canonicalize()
        .map_err(|e| format!("projection store root: {e}"))?;
        if !selection.starts_with(&baseline) || !store_root.starts_with(&baseline) {
            return Err(
                "projection selection and source store must be inside the selected stage baseline"
                    .into(),
            );
        }
    }
    let destination = args.output.join("owner-stage");
    if destination.exists() || destination.is_symlink() {
        return Err("owner stage destination must be fresh".into());
    }
    let mut files = 0usize;
    let mut bytes = 0u64;
    if let Err(error) = copy_stage_tree(&baseline, &destination, &mut files, &mut bytes) {
        let _ = fs::remove_dir_all(&destination);
        return Err(error);
    }
    let destination = destination
        .canonicalize()
        .map_err(|e| format!("owner stage destination: {e}"))?;
    rebase_plan_paths(plan, &baseline, &destination)?;
    args.model = rebase_path(&args.model, &baseline, &destination)?;
    args.binding = rebase_path(&args.binding, &baseline, &destination)?;
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
        let database = rusqlite::Connection::open(&args.model)
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
        rebase_json_paths(&mut selection, &baseline, &destination)?;
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
        .filter(|operation| operation.bind_expected_configuration_from_owner)
    {
        if operation.channel != "sdk" || operation_name(operation) != Some("describe") {
            return Err(
                "native owner configuration binding is limited to a serial describe operation"
                    .into(),
            );
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
            ignored_json_paths: Vec::new(),
            alternate_outcomes: Vec::new(),
            retry_same_command_id: false,
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
fn cgroup_json(s: &ResourceSnapshot) -> Value {
    json!({"cpu_usage_usec":s.cpu_usec,"memory_current_bytes":s.memory_current,"memory_peak_bytes":s.memory_peak,"memory_limit_bytes":s.memory_limit,"memory_high_events":s.memory_high_events,"memory_max_events":s.memory_max_events,"memory_oom_events":s.memory_oom_events,"memory_oom_kill_events":s.memory_oom_kill_events,"io_read_bytes":s.io_read_bytes,"io_write_bytes":s.io_write_bytes})
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

fn source_read_operation(op: &Operation) -> bool {
    match op.channel.as_str() {
        "mcp" => {
            op.request["method"] == "tools/call"
                && matches!(
                    op.request["params"]["name"].as_str(),
                    Some("tos_source_read_capabilities")
                        | Some("tos_source_handle_discover")
                        | Some("tos_source_read")
                )
        }
        "sdk" => op
            .sdk_argv
            .windows(2)
            .any(|pair| pair[0] == "source" && matches!(pair[1].as_str(), "discover" | "read")),
        "projection" => matches!(
            op.request["operation"].as_str(),
            Some("search" | "search-text" | "exact-source")
        ),
        _ => false,
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
) -> bool {
    if expected.status != attempt.status
        || expected
            .sha256
            .as_ref()
            .is_some_and(|digest| digest != &attempt.sha256)
    {
        return false;
    }
    let body_matches_expected = expected.body.as_ref().is_none_or(|body| {
        let Some(raw) = attempt.body.as_deref() else {
            return false;
        };
        let Ok(actual) = serde_json::from_slice::<Value>(raw) else {
            return false;
        };
        let ignored_paths = expected
            .ignored_json_paths
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        body_matches(body, &actual, &ignored_paths, "")
            && (!ignored_paths.contains("/result/revision")
                || published_form_revision_matches(args, op, &actual))
            && (!ignored_paths.contains("/result/receipt/command_id")
                || actual.pointer("/result/receipt/command_id") == op.request.get("command_id"))
    });
    body_matches_expected && (expected.sha256.is_some() || expected.body.is_some())
}

fn valid_expected(value: &ExpectedOutcome, sdk: bool, response_cap: usize) -> bool {
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

fn rights_describe_binding_operation(op: &Operation) -> bool {
    op.channel == "sdk"
        && operation_name(op) == Some("describe")
        && op.bind_expected_configuration_from_owner
}

fn bind_expected_fields(op: &mut Operation, response: &Value) -> Result<(), String> {
    let operation = operation_name(op).ok_or("bound command operation absent")?;
    let result = response
        .get("result")
        .ok_or("bound preview response result absent")?;
    if operation == "rights.revoke" {
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
    let (expected_source, expected_revision) = match operation {
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
    if invocation["schema_version"] != "tos_local_pg_source_bootstrap_invocation_v1"
        || selection["initialization_mode"] != "initialize"
        || selection["initialize_schema"] != true
        || selection["job_fence"] != 1
        || selection["domain"].as_str().is_none_or(str::is_empty)
        || selection["job_id"].as_str().is_none_or(str::is_empty)
    {
        return Err("source.bootstrap must select a fresh initial CMD.2 domain at fence 1".into());
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
        let projection_store = PathBuf::from(
            projection_selection["store_root"]
                .as_str()
                .ok_or("projection store root absent")?,
        )
        .canonicalize()
        .map_err(|e| format!("projection selected store root: {e}"))?;
        if !projection_selection_path.starts_with(&baseline)
            || projection_store != store_root
            || projection_selection["domain"] != domain
            || projection_selection["source_contract_sha256"] != contract
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
        .filter(|operation| operation.channel == "sdk")
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
            let selected_binary = PathBuf::from(&operation.sdk_argv[0])
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

fn outcome_report(op: &Operation) -> Value {
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
    {
        return Err("schedule schema/session count mismatch".into());
    }
    let fresh_pg_stage = validate_fresh_pg_stage(plan, args)?;
    let mut ids = BTreeSet::new();
    let mut total = plan.setup.len();
    let mut owner_requests = 0usize;
    let mut command_ids = BTreeSet::new();
    let mut setup_labels = BTreeSet::new();
    let mut setup_previews: BTreeMap<String, (String, String)> = BTreeMap::new();
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
                    | "rights.revoke"
            )
        {
            return Err(
                "setup accepts only bounded serial native bootstrap, preview and commit commands"
                    .into(),
            );
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
        if setup_index == 0 && name != "source.bootstrap" {
            return Err("serial native PG setup must start with source.bootstrap".into());
        }
        if name == "source.bootstrap" && setup_index != 0 {
            return Err(
                "source.bootstrap may appear only as the first serial setup operation".into(),
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
            if op.label.is_empty()
                || op.label.len() > 64
                || !op.label.is_ascii()
                || !valid_expected(&primary, native_process, args.response_cap)
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
                        || !op.sdk_argv.iter().any(|arg| arg == "source-commands")
                        || !op.projection_argv.is_empty()
                        || invocation_args.len() != 1
                        || (command_id.is_none()
                            && !read_only_describe
                            && !read_only_rights_describe)
                        || op.reconnect_before
                        || (op.retry_same_command_id && op.conflict_group.is_some())
                        || op.bind_expected_configuration_from_owner
                        || (op.bind_expected_from.is_some()
                            && !(bound_commit || bound_rights_revoke))
                        || (fresh_pg_stage
                            && (expected_binding_operation(op) || rights_revoke_operation(op))
                            && !(bound_commit || bound_rights_revoke))
                        || !op.request.is_object()
                        || json_bytes(&op.request)?.len() > args.request_cap
                    {
                        return Err("SDK source command requires explicit native argv and one invocation; retries and concurrent conflict groups are supported".into());
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
                    let selection_args = op
                        .projection_argv
                        .windows(2)
                        .filter(|pair| pair[0] == "--selection")
                        .collect::<Vec<_>>();
                    let projection_operation = op.request["operation"].as_str();
                    if op.projection_argv.is_empty()
                        || op.projection_argv.len() > 32
                        || op.projection_argv[0].is_empty()
                        || !Path::new(&op.projection_argv[0]).is_absolute()
                        || !op
                            .projection_argv
                            .iter()
                            .any(|arg| arg == "source-projection")
                        || selection_args.len() != 1
                        || !Path::new(selection_args[0][1]).is_absolute()
                        || !op.sdk_argv.is_empty()
                        || !op.request.is_object()
                        || !matches!(
                            projection_operation,
                            Some(
                                "init"
                                    | "status"
                                    | "apply"
                                    | "search"
                                    | "search-text"
                                    | "exact-source"
                            )
                        )
                        || json_bytes(&op.request)?.len() > args.request_cap
                        || op.retry_same_command_id
                        || op.reconnect_before
                        || op.conflict_group.is_some()
                        || op.bind_expected_configuration_from_owner
                        || op.bind_expected_from.is_some()
                        || (op.interrupt_resume_after_ms.is_some()
                            && (projection_operation != Some("apply")
                                || op.request["max_events"]
                                    .as_u64()
                                    .is_none_or(|events| events < 2)
                                || op.interrupt_resume_after_ms == Some(0)
                                || op.interrupt_resume_after_ms.is_some_and(|ms| {
                                    ms >= args.deadline_seconds.saturating_mul(1000)
                                })))
                    {
                        return Err("projection operation requires the existing source-projection CLI, one private selection and a supported request".into());
                    }
                    for arg in &op.projection_argv {
                        if arg.len() > 4096 {
                            return Err("projection argv element exceeds4096 bytes".into());
                        }
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
    if total == 0 || total > MAX_OPS || owner_requests + usize::from(needs_owner) > MAX_NONCES {
        return Err("bounded operation or source HTTP nonce budget exceeded".into());
    }
    if needs_mcp && (args.source_root.is_none() || args.source_inputs.is_none()) {
        return Err("MCP schedule requires --source-root and --source-inputs".into());
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
        .stderr(Stdio::null());
    let mut child = spawn_group(&mut command, pids, deadline)?;
    let pid = child.id() as i32;
    let mut stdin = child.stdin.take().ok_or("SDK stdin unavailable")?;
    let mut stdout = child.stdout.take().ok_or("SDK stdout unavailable")?;
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
    let write_result = stdin.write_all(&input).map_err(|e| e.to_string());
    drop(stdin);
    if let Err(error) = write_result {
        kill_group(pid, libc::SIGKILL);
        let _ = child.wait();
        let _ = reader.join();
        return Err(format!(
            "SDK request pipe: {error}; process group terminated"
        ));
    }
    let interrupt_at = interrupt_after.map(|delay| Instant::now() + delay);
    let mut captured_output = None;
    let mut interrupted = false;
    let status = loop {
        if STOP.load(Ordering::Acquire) || Instant::now() >= deadline {
            kill_group(pid, libc::SIGKILL);
            let _ = child.wait();
            let _ = reader.join();
            return Err("native process deadline; process group terminated".into());
        }
        if captured_output.is_none() {
            if let Ok(captured) = output_rx.try_recv() {
                captured_output = Some(captured);
            }
        }
        if let Some(status) = child
            .try_wait()
            .map_err(|e| format!("native process status: {e}"))?
        {
            if interrupt_after.is_some() {
                kill_group(pid, libc::SIGKILL);
                let _ = reader.join();
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
    reader
        .join()
        .map_err(|_| "native response drain panicked")?;
    let (output, read_result) = captured_output.ok_or("native response drain absent")?;
    if STOP.load(Ordering::Acquire) || Instant::now() >= deadline {
        kill_group(pid, libc::SIGKILL);
        return Err("native process response exceeded the workload deadline".into());
    }
    read_result.map_err(|e| format!("native response read: {e}"))?;
    if output.len() > response_cap {
        kill_group(pid, libc::SIGKILL);
        return Err("native response exceeds response cap; process group terminated".into());
    }
    Ok(Attempt {
        status: status.code().unwrap_or(255) as u16,
        bytes: output.len(),
        sha256: sha(&output),
        body: (!discard_response && !interrupted).then_some(output),
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

fn run_operation(
    op: &Operation,
    session: &mut McpSession,
    args: &Args,
    secret: &[u8],
    nonces: &Mutex<BTreeSet<String>>,
    pids: &Arc<Mutex<Vec<i32>>>,
    gates: &WorkloadGates,
    deadline: Instant,
) -> OpResult {
    let started = Instant::now();
    let mut reconnect_count = 0usize;
    let mut reconnect_observation = None;
    let mut retry_count = 0usize;
    let mut projection_interruptions = 0usize;
    let mut projection_resumes = 0usize;
    let mut projection_status_checks = 0usize;
    let mut projection_cursor_observation = Value::Null;
    let mut projection_queue_age_observation = Value::Null;
    let mut attempts = Vec::new();
    let mut error = None;
    let _channel_permit = match op.channel.as_str() {
        "mcp" => gates.mcp_sessions.acquire(deadline),
        "owner_http" => gates.owner_http.acquire(deadline),
        "sdk" | "projection" => gates.sdk_source_commands.acquire(deadline),
        _ => Err("unknown channel gate".into()),
    };
    let _source_read_permit = if source_read_operation(op) {
        Some(gates.source_reads.acquire(deadline))
    } else {
        None
    };
    let _channel_permit = match _channel_permit {
        Ok(permit) => Some(permit),
        Err(reason) => {
            error = Some(reason);
            None
        }
    };
    if _source_read_permit.as_ref().is_some_and(Result::is_err) {
        error = Some("source-read in-flight limit deadline/cancellation".into());
    }
    let _source_read_permit = _source_read_permit.and_then(Result::ok);
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
                        elapsed_ms: started.elapsed().as_secs_f64() * 1000.0,
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
    let final_attempt = attempts.last().cloned();
    let auth_ok = op.channel != "owner_http"
        || final_attempt
            .as_ref()
            .is_some_and(|a| a.authenticated == Some(true));
    let expected = outcomes(op);
    let matched_outcome = final_attempt.as_ref().and_then(|attempt| {
        expected
            .iter()
            .find(|value| outcome_matches(args, op, value, attempt))
    });
    let passed = error.is_none() && auth_ok && matched_outcome.is_some();
    let status = final_attempt.as_ref().map(|a| a.status);
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
    let attempt_values = attempts.iter().map(|a| json!({"status":a.status,"response_bytes":a.bytes,"response_sha256":a.sha256,"response_available_to_oracle":a.body.is_some(),"response_disposition":if a.interrupted {"interrupted"} else if a.body.is_none() {"dropped"} else {"delivered"},"interrupted":a.interrupted,"elapsed_ms":a.elapsed_ms,"response_authenticated":a.authenticated})).collect::<Vec<_>>();
    let expected_report = outcome_report(op);
    let matched_report = matched_outcome.map(outcome_summary);
    let value = json!({"event":"operation","label":op.label,"channel":op.channel,"status":status,"expected_outcomes":expected_report,"matched_outcome":matched_report,"response_sha256":final_attempt.as_ref().map(|a|a.sha256.clone()),"passed":passed,"transport_error":transport_error,"error":error,"conflict_group":op.conflict_group,"conflict":conflict,"attempts":attempt_values,"retry_count":retry_count,"reconnect_count":reconnect_count,"projection_interruptions":projection_interruptions,"projection_resumes":projection_resumes,"projection_status_checks":projection_status_checks,"projection_cursor":projection_cursor_observation,"projection_queue_age":projection_queue_age_observation,"reconnect":reconnect_observation,"elapsed_ms":started.elapsed().as_secs_f64()*1000.0});
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
        elapsed_ms: started.elapsed().as_secs_f64() * 1000.0,
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
    json!({"cpu_usage_usec":end.cpu_usec.saturating_sub(start.cpu_usec),"memory_current_start_bytes":start.memory_current,"memory_current_end_bytes":end.memory_current,"memory_peak_bytes":end.memory_peak,"memory_limit_bytes":end.memory_limit,"memory_high_events_delta":end.memory_high_events.saturating_sub(start.memory_high_events),"memory_max_events_delta":end.memory_max_events.saturating_sub(start.memory_max_events),"memory_oom_events_delta":end.memory_oom_events.saturating_sub(start.memory_oom_events),"memory_oom_kill_events_delta":end.memory_oom_kill_events.saturating_sub(start.memory_oom_kill_events),"io_read_bytes":end.io_read_bytes.saturating_sub(start.io_read_bytes),"io_write_bytes":end.io_write_bytes.saturating_sub(start.io_write_bytes)})
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
    install_signals();
    let mut args = parse_args()?;
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
    let fresh_pg_stage = validate_fresh_pg_stage(&plan, &args)?;
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
    let owner_stage = prepare_owner_stage(&mut args, &mut plan, needs_mcp, needs_owner, deadline)?;
    let mut paths = vec![
        args.query_binary.clone(),
        args.model.clone(),
        args.binding.clone(),
        args.schedule.clone(),
    ];
    let schedule_index = 3;
    let mut owner_indexes = BTreeMap::new();
    for (name, path) in [
        ("owner_binary", &args.owner_binary),
        ("owner_config", &args.owner_config),
        ("invocation", &args.invocation),
        ("token_file", &args.token_file),
        ("source_inputs", &args.source_inputs),
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
        .filter(|operation| operation.channel == "sdk")
        .count();
    let projection_operations = plan
        .sessions
        .iter()
        .take(args.sessions)
        .flat_map(|session| &session.operations)
        .filter(|operation| operation.channel == "projection")
        .count();
    let mut sdk_indexes = BTreeMap::new();
    for operation in plan.setup.iter().chain(
        plan.sessions
            .iter()
            .take(args.sessions)
            .flat_map(|session| &session.operations),
    ) {
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
        .sessions
        .iter()
        .take(args.sessions)
        .flat_map(|session| &session.operations)
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
                "prepared_model": identity_json(&input_stamps[1], None),
                "prepared_binding": identity_json(&input_stamps[2], None),
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
                "bounded_copy_bytes": MAX_STAGE_TOTAL_BYTES,
                "bounded_copy_files": MAX_STAGE_FILES
            })),
            "limits": {
                "logical_sessions": args.sessions,
                "serial_setup_operations": plan.setup.len(),
                "logical_mcp_sessions": logical_mcp_sessions,
                "physical_mcp_sessions": physical_mcp_session_limit,
                "mcp_in_flight_limit": 32,
                "source_read_in_flight_limit": 32,
                "owner_http_in_flight_limit": 4,
                "sdk_source_command_in_flight_limit": args.sdk_in_flight,
                "scheduled_source_read_operations": source_read_operations,
                "scheduled_sdk_source_commands": sdk_source_command_operations,
                "scheduled_projection_operations": projection_operations,
                "operation_count_limit": args.operation_count,
                "requested_channel_mix_percent": args.channel_mix.as_ref(),
                "operations_per_second_limit": args.operations_per_second,
                "load_window_seconds_limit": args.load_window_seconds,
                "fresh_pg_stage": fresh_pg_stage,
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
        query_args.extend([
            "--prepared-read-model".to_owned(),
            args.model
                .to_str()
                .ok_or("model path UTF-8 required")?
                .to_owned(),
            "--prepared-binding".to_owned(),
            args.binding
                .to_str()
                .ok_or("binding path UTF-8 required")?
                .to_owned(),
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
            deadline,
        );
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
                    json!({"request":op.request.clone(),"response":body}),
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
    let load_started_at = Instant::now();
    let load_window_deadline = args
        .load_window_seconds
        .map(|seconds| load_started_at + Duration::from_secs(seconds));
    let mut load_window_closed = false;
    for step in 0..rounds {
        if setup_failures != 0 {
            break;
        }
        if load_window_deadline.is_some_and(|window| Instant::now() >= window) {
            load_window_closed = true;
            break;
        }
        if STOP.load(Ordering::Acquire) || Instant::now() >= deadline {
            return Err("whole workload deadline/cancellation".into());
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
                    let result = match gates.operation_rate.acquire(deadline, load_window_deadline)
                    {
                        Ok(false) => operation_skipped("load-window-closed-before-dispatch"),
                        Err(reason) => operation_error(&reason),
                        Ok(true) => {
                            if let Some(session_state) = session_state {
                                match session_state.lock() {
                                    Ok(mut session) => run_operation(
                                        &op,
                                        &mut session,
                                        &args,
                                        &secret,
                                        &nonce_set,
                                        &pids,
                                        &gates,
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
                                    deadline,
                                )
                            }
                        }
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
    emit(
        &mut output,
        &mut written,
        args.output_cap,
        &json!({
            "event": "finish",
            "passed": errors == 0 && load_operations_not_started == 0 && !resource_stop_observed,
            "selected_file_metadata_unchanged": true,
            "schedule_unchanged": true,
            "scheduled": total_ops,
            "serial_setup_scheduled": plan.setup.len(),
            "serial_setup_attempted": setup_successes + setup_failures,
            "serial_setup_completed": setup_successes,
            "serial_setup_failures": setup_failures,
            "serial_setup_elapsed_seconds": setup_elapsed_seconds,
            "fresh_pg_stage": fresh_pg_stage,
            "postgresql_url_reported": false,
            "database_reset_or_drop_performed": false,
            "cleanup_policy": "runner stops only its child process groups and retains the private staged files and report; caller owns disposable PostgreSQL database cleanup",
            "logical_sessions": args.sessions,
            "load_operation_count_limit": args.operation_count,
            "load_operations_scheduled": load_operations_scheduled,
            "load_operations_started": load_operations_started,
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
            "started": successes + errors,
            "failures": errors,
            "conflicts": conflicts,
            "conflict_groups": conflict_groups,
            "conflict_group_failures": conflict_group_failures,
            "retries": retries,
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
            "report_storage_bytes_before_finish": storage_bytes,
            "report_storage_allocated_bytes_before_finish": storage_allocated,
            "scope": "finite Rust protocol measurement; not capacity acceptance"
        }),
    )?;
    output.sync_all().map_err(|e| format!("report sync: {e}"))?;
    Ok(
        if errors == 0 && load_operations_not_started == 0 && !resource_stop_observed {
            0
        } else {
            1
        },
    )
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
