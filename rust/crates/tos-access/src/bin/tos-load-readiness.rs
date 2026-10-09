//! Bounded protocol workload for an already installed Tree-of-Sophia Rust release.
//! The schedule supplies owner-selected requests; this tool adds no server API.
use hmac::{Hmac, Mac};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::ops::{Deref, DerefMut};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::os::unix::process::CommandExt;
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
const MAX_OPS: usize = 4096;
const MAX_NONCES: usize = 1024;
static STOP: AtomicBool = AtomicBool::new(false);

#[derive(Clone)]
struct Args {
    query_binary: PathBuf,
    owner_binary: Option<PathBuf>,
    model: PathBuf,
    binding: PathBuf,
    owner_config: Option<PathBuf>,
    invocation: Option<PathBuf>,
    token_file: Option<PathBuf>,
    schedule: PathBuf,
    output: PathBuf,
    unit: String,
    mcp_port: u16,
    owner_port: u16,
    sessions: usize,
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
    expected_sha256: String,
    #[serde(default)]
    alternate_outcomes: Vec<ExpectedOutcome>,
    #[serde(default)]
    retry_same_command_id: bool,
    #[serde(default)]
    reconnect_before: bool,
    #[serde(default)]
    conflict_group: Option<String>,
    #[serde(default)]
    sdk_argv: Vec<String>,
}

#[derive(Clone, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
struct ExpectedOutcome {
    status: u16,
    sha256: String,
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
    elapsed_ms: f64,
    authenticated: Option<bool>,
}

#[derive(Clone)]
struct OpResult {
    value: Value,
    passed: bool,
    successful: bool,
    conflict: bool,
    retry_count: usize,
    reconnect_count: usize,
    elapsed_ms: f64,
}

#[derive(Clone, Default)]
struct ResourceSnapshot {
    cpu_usec: u64,
    memory_current: u64,
    memory_peak: u64,
    io_read_bytes: u64,
    io_write_bytes: u64,
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
            return Err("usage: tos-load-readiness --query-binary ABS --model ABS --binding ABS --schedule ABS --output DIR --unit UNIT [--owner-binary ABS --owner-config ABS --invocation ABS --token-file ABS] [--mcp-port PORT] [--owner-port PORT] [--sessions 2|8|16|64|128|256] [--deadline-seconds N] [--request-cap-bytes N] [--response-cap-bytes N] [--schedule-cap-bytes N] [--output-cap-bytes N]".into());
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
    let model = path(&mut values, "--model")?;
    let binding = path(&mut values, "--binding")?;
    let owner_config = optional_path(&mut values, "--owner-config")?;
    let invocation = optional_path(&mut values, "--invocation")?;
    let token_file = optional_path(&mut values, "--token-file")?;
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
    if ![2, 8, 16, 64, 128, 256].contains(&sessions) {
        return Err("sessions must be one of 2, 8, 16, 64, 128, 256".into());
    }
    let deadline_seconds = number(&mut values, "--deadline-seconds", "120")?;
    if deadline_seconds > 3600 {
        return Err("deadline exceeds 3600 seconds".into());
    }
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
        model,
        binding,
        owner_config,
        invocation,
        token_file,
        schedule,
        output,
        unit,
        mcp_port: mcp_port as u16,
        owner_port: owner_port as u16,
        sessions,
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
    Ok(format!("{:x}", hash.finalize()))
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
        io_read_bytes: io_read,
        io_write_bytes: io_write,
    })
}
fn cgroup_json(s: &ResourceSnapshot) -> Value {
    json!({"cpu_usage_usec":s.cpu_usec,"memory_current_bytes":s.memory_current,"memory_peak_bytes":s.memory_peak,"io_read_bytes":s.io_read_bytes,"io_write_bytes":s.io_write_bytes})
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
fn outcomes(op: &Operation) -> Vec<ExpectedOutcome> {
    let mut values = vec![ExpectedOutcome {
        status: op.expected_status,
        sha256: op.expected_sha256.clone(),
    }];
    values.extend(op.alternate_outcomes.clone());
    values.sort();
    values
}

fn validate_plan(plan: &Plan, args: &Args) -> Result<(usize, bool, bool), String> {
    if plan.schema != SCHEDULE_SCHEMA
        || plan.sessions.len() < args.sessions
        || plan.sessions.len() > MAX_SESSIONS
    {
        return Err("schedule schema/session count mismatch".into());
    }
    let mut ids = BTreeSet::new();
    let mut total = 0usize;
    let mut owner_requests = 0usize;
    let mut command_ids = BTreeSet::new();
    let mut needs_mcp = false;
    let mut needs_owner = false;
    let mut conflicts: BTreeMap<(usize, String), Vec<(usize, Vec<ExpectedOutcome>)>> =
        BTreeMap::new();
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
        for (step, op) in session.operations.iter().enumerate() {
            if op.label.is_empty()
                || op.label.len() > 64
                || !op.label.is_ascii()
                || !(100..=599).contains(&op.expected_status)
                || !valid_sha(&op.expected_sha256)
                || op.alternate_outcomes.len() > 1
                || op
                    .alternate_outcomes
                    .iter()
                    .any(|v| !(100..=599).contains(&v.status) || !valid_sha(&v.sha256))
            {
                return Err("operation label/status/digest invalid".into());
            }
            match op.channel.as_str() {
                "mcp" => {
                    needs_mcp = true;
                    if op.retry_same_command_id {
                        return Err("MCP retry is not supported by this schedule route".into());
                    }
                    if !op.sdk_argv.is_empty() || op.conflict_group.is_some() {
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
                    {
                        return Err("owner_http requires an exact source-command object and no SDK/MCP-only options".into());
                    }
                    if json_bytes(&op.request)?.len() > args.request_cap {
                        return Err("source-command body exceeds request cap".into());
                    }
                }
                "sdk" => {
                    if op.sdk_argv.is_empty()
                        || op.sdk_argv.len() > 32
                        || op.sdk_argv[0].is_empty()
                        || !Path::new(&op.sdk_argv[0]).is_absolute()
                        || op.reconnect_before
                        || op.retry_same_command_id
                        || op.conflict_group.is_some()
                        || op.expected_status > 255
                    {
                        return Err("SDK operation requires an explicit absolute argv, process exit status and no implicit retry".into());
                    }
                    for arg in &op.sdk_argv {
                        if arg.len() > 4096 {
                            return Err("SDK argv element exceeds4096 bytes".into());
                        }
                    }
                }
                _ => return Err("channel must be mcp, owner_http or sdk".into()),
            }
            if let Some(group) = &op.conflict_group {
                if group.is_empty() || group.len() > 64 || op.channel != "owner_http" {
                    return Err("conflict_group requires owner_http operations".into());
                }
                conflicts
                    .entry((step, group.clone()))
                    .or_default()
                    .push((session_index, outcomes(op)));
            } else if !op.alternate_outcomes.is_empty() {
                return Err("alternate_outcomes require a conflict_group".into());
            }
        }
    }
    if total == 0 || total > MAX_OPS || owner_requests + usize::from(needs_owner) > MAX_NONCES {
        return Err("bounded operation or source HTTP nonce budget exceeded".into());
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
            .map(|entry| entry.1.clone())
            .ok_or("empty conflict group")?;
        declared.sort();
        let has_success = declared
            .iter()
            .filter(|v| (200..300).contains(&v.status))
            .count()
            == 1;
        let has_conflict = declared.iter().filter(|v| v.status == 409).count() == 1;
        let distinct_statuses = declared
            .iter()
            .map(|v| v.status)
            .collect::<BTreeSet<_>>()
            .len()
            == declared.len();
        if group.len() < 2
            || group.iter().map(|v| v.0).collect::<BTreeSet<_>>().len() < 2
            || !has_success
            || !has_conflict
            || !distinct_statuses
            || group.iter().any(|entry| entry.1 != declared)
        {
            return Err("each conflict group must race distinct sessions with the same exact 2xx/409 outcome set".into());
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
    unsafe {
        libc::kill(-pid, signal);
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
            elapsed_ms: started.elapsed().as_secs_f64() * 1000.0,
            authenticated: Some(false),
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
        elapsed_ms: started.elapsed().as_secs_f64() * 1000.0,
        authenticated: Some(authenticated),
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
fn sdk_process(
    op: &Operation,
    work_dir: &Path,
    deadline: Instant,
    request_cap: usize,
    response_cap: usize,
    pids: &Arc<Mutex<Vec<i32>>>,
) -> Result<Attempt, String> {
    let started = Instant::now();
    let binary = PathBuf::from(&op.sdk_argv[0]);
    let mut input = json_bytes(&op.request)?;
    input.push(b'\n');
    if input.len() > request_cap {
        return Err("SDK input exceeds request cap".into());
    }
    let mut command = Command::new(&binary);
    command
        .args(&op.sdk_argv[1..])
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
    let (output, read_result) = output_rx
        .recv_timeout(remaining(deadline)?)
        .map_err(|e| format!("SDK response deadline: {e}"))?;
    reader.join().map_err(|_| "SDK response drain panicked")?;
    if STOP.load(Ordering::Acquire) || Instant::now() >= deadline {
        kill_group(pid, libc::SIGKILL);
        return Err("SDK operation deadline; process group terminated".into());
    }
    read_result.map_err(|e| format!("SDK response read: {e}"))?;
    if output.len() > response_cap {
        kill_group(pid, libc::SIGKILL);
        return Err("SDK response exceeds response cap; process group terminated".into());
    }
    if let Err(error) = write_result {
        kill_group(pid, libc::SIGKILL);
        return Err(format!(
            "SDK request pipe: {error}; process group terminated"
        ));
    }
    let status = loop {
        if STOP.load(Ordering::Acquire) || Instant::now() >= deadline {
            kill_group(pid, libc::SIGKILL);
            return Err("SDK exit deadline; process group terminated".into());
        }
        if let Some(status) = child
            .try_wait()
            .map_err(|e| format!("SDK client status: {e}"))?
        {
            break status;
        }
        thread::sleep(Duration::from_millis(10));
    };
    Ok(Attempt {
        status: status.code().unwrap_or(255) as u16,
        bytes: output.len(),
        sha256: sha(&output),
        elapsed_ms: started.elapsed().as_secs_f64() * 1000.0,
        authenticated: None,
    })
}
fn run_operation(
    op: &Operation,
    session: &mut McpSession,
    args: &Args,
    secret: &[u8],
    nonces: &Mutex<BTreeSet<String>>,
    pids: &Arc<Mutex<Vec<i32>>>,
    deadline: Instant,
) -> OpResult {
    let started = Instant::now();
    let mut reconnect_count = 0usize;
    let mut reconnect_observation = None;
    let mut retry_count = 0usize;
    let mut attempts = Vec::new();
    let mut error = None;
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
                    elapsed_ms: started.elapsed().as_secs_f64() * 1000.0,
                    authenticated: None,
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
    } else {
        match sdk_process(
            op,
            &args.output,
            deadline,
            args.request_cap,
            args.response_cap,
            pids,
        ) {
            Ok(attempt) => attempts.push(attempt),
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
            .find(|v| v.status == attempt.status && v.sha256 == attempt.sha256)
    });
    let passed = error.is_none() && auth_ok && matched_outcome.is_some();
    let status = final_attempt.as_ref().map(|a| a.status);
    let conflict = status == Some(409)
        && (op.channel != "owner_http"
            || final_attempt
                .as_ref()
                .is_some_and(|a| a.authenticated == Some(true)));
    let transport_error = error.is_some()
        || final_attempt.is_none()
        || (op.channel == "owner_http"
            && final_attempt
                .as_ref()
                .is_some_and(|a| a.authenticated != Some(true)));
    let successful = passed && status.is_some_and(|s| (200..300).contains(&s));
    let attempt_values = attempts.iter().map(|a| json!({"status":a.status,"response_bytes":a.bytes,"response_sha256":a.sha256,"elapsed_ms":a.elapsed_ms,"response_authenticated":a.authenticated})).collect::<Vec<_>>();
    let value = json!({"event":"operation","label":op.label,"channel":op.channel,"status":status,"expected_outcomes":expected,"matched_outcome":matched_outcome,"response_sha256":final_attempt.as_ref().map(|a|a.sha256.clone()),"passed":passed,"transport_error":transport_error,"error":error,"conflict_group":op.conflict_group,"conflict":conflict,"attempts":attempt_values,"retry_count":retry_count,"reconnect_count":reconnect_count,"reconnect":reconnect_observation,"elapsed_ms":started.elapsed().as_secs_f64()*1000.0});
    OpResult {
        value,
        passed,
        successful,
        conflict,
        retry_count,
        reconnect_count,
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
    json!({"cpu_usage_usec":end.cpu_usec.saturating_sub(start.cpu_usec),"memory_current_start_bytes":start.memory_current,"memory_current_end_bytes":end.memory_current,"memory_peak_bytes":end.memory_peak,"io_read_bytes":end.io_read_bytes.saturating_sub(start.io_read_bytes),"io_write_bytes":end.io_write_bytes.saturating_sub(start.io_write_bytes)})
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
    let args = parse_args()?;
    if args.output.is_symlink() || !args.output.is_dir() {
        return Err("output must be a precreated owned directory".into());
    }
    let output_meta =
        fs::symlink_metadata(&args.output).map_err(|e| format!("output metadata: {e}"))?;
    if output_meta.uid() != unsafe { libc::geteuid() } || output_meta.mode() & 0o077 != 0 {
        return Err("output directory must be private and owned by the runner".into());
    }
    let cgroup = cgroup_path(&args.unit)?;
    let lease_execution = write_lease(&args.output, &args.unit, args.output_cap)?;
    let deadline = Instant::now() + Duration::from_secs(args.deadline_seconds);
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
    let schedule_size = input_stamps[schedule_index].len;
    if schedule_size > args.schedule_cap as u64 {
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
    if schedule_bytes.len() > args.schedule_cap {
        return Err("schedule cap exceeded".into());
    }
    if schedule_bytes.len() as u64 != schedule_size {
        return Err("schedule changed between identity and read".into());
    }
    let schedule_sha256 = sha(&schedule_bytes);
    let plan: Plan =
        serde_json::from_slice(&schedule_bytes).map_err(|_| "invalid bounded workload schedule")?;
    let (total_ops, needs_mcp, needs_owner) = validate_plan(&plan, &args)?;
    let mut sdk_indexes = BTreeMap::new();
    for operation in plan
        .sessions
        .iter()
        .take(args.sessions)
        .flat_map(|session| &session.operations)
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
    let mut executable_hashes = BTreeMap::new();
    executable_hashes.insert(0usize, executable_sha256(&args.query_binary, deadline)?);
    if let Some(index) = owner_indexes.get("owner_binary") {
        executable_hashes.insert(*index, executable_sha256(&paths[*index], deadline)?);
    }
    for index in sdk_indexes.values() {
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
    emit(
        &mut output,
        &mut written,
        args.output_cap,
        &json!({"event":"start","identities":{"query_binary":identity_json(&input_stamps[0], executable_hashes.get(&0).map(String::as_str)),"owner_inputs":owner_input_identities,"prepared_model":identity_json(&input_stamps[1], None),"prepared_binding":identity_json(&input_stamps[2], None),"external_sdk_binaries":sdk_identities,"schedule_sha256":schedule_sha256},"limits":{"sessions":args.sessions,"deadline_seconds":args.deadline_seconds,"request_cap_bytes":args.request_cap,"response_cap_bytes":args.response_cap,"schedule_cap_bytes":args.schedule_cap,"output_cap_bytes":args.output_cap},"lease_execution":lease_execution,"scope":"finite Rust protocol measurement; not capacity acceptance"}),
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
        command
            .args([
                "--prepared-read-model",
                args.model.to_str().ok_or("model path UTF-8 required")?,
                "--prepared-binding",
                args.binding.to_str().ok_or("binding path UTF-8 required")?,
                "mcp",
                "--transport",
                "streamable-http",
                "--host",
                "127.0.0.1",
                "--port",
                &args.mcp_port.to_string(),
            ])
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
    let mut sessions = Vec::new();
    for session in plan.sessions.iter().take(args.sessions) {
        let started = Instant::now();
        let mcp_needed = session.operations.iter().any(|op| op.channel == "mcp");
        let (state, status, digest) = if mcp_needed {
            let (state, status, digest, _) =
                initialize_mcp(args.mcp_port, deadline, args.response_cap)?;
            (state, Some(status), Some(digest))
        } else {
            (McpSession::default(), None, None)
        };
        emit(
            &mut output,
            &mut written,
            args.output_cap,
            &json!({"event":"session_ready","session":session.id,"mcp_initialized":mcp_needed && status==Some(200),"initialize_status":status,"initialize_sha256":digest,"initialize_elapsed_ms":started.elapsed().as_secs_f64()*1000.0,"available":!mcp_needed || status==Some(200)}),
        )?;
        sessions.push(state);
    }
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
    let mut latencies = Vec::new();
    let mut all_latencies = Vec::new();
    for step in 0..rounds {
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
            .collect::<Vec<_>>();
        if jobs.is_empty() {
            continue;
        }
        let mut start_senders = Vec::new();
        let (tx, rx) = mpsc::channel();
        let mut handles = Vec::new();
        for (index, session_name, op) in jobs.iter().cloned() {
            let (start_tx, start_rx) = mpsc::channel();
            let tx = tx.clone();
            let args = args.clone();
            let secret = secret.clone();
            let pids = Arc::clone(&pids);
            let nonce_set = Arc::clone(&nonce_set);
            let session_state = sessions[index].clone();
            let handle = thread::Builder::new()
                .stack_size(256 * 1024)
                .spawn(move || {
                    if start_rx.recv().is_err() {
                        return;
                    }
                    let mut session = session_state;
                    let result = run_operation(
                        &op,
                        &mut session,
                        &args,
                        &secret,
                        &nonce_set,
                        &pids,
                        deadline,
                    );
                    let _ = tx.send((index, session_name, op, session, result));
                })
                .map_err(|e| format!("bounded session worker start: {e}"))?;
            handles.push(handle);
            start_senders.push(start_tx);
        }
        drop(tx);
        for start in start_senders {
            let _ = start.send(());
        }
        let mut received = 0usize;
        let mut wave_results = Vec::with_capacity(handles.len());
        while received < handles.len() {
            let (index, session_name, op, session, result) = rx
                .recv_timeout(remaining(deadline)?)
                .map_err(|e| format!("session result deadline: {e}"))?;
            sessions[index] = session;
            wave_results.push((index, session_name, op, result));
            received += 1;
        }
        for handle in handles {
            handle.join().map_err(|_| "session worker panicked")?;
        }
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
            if passed {
                successes += 1;
            } else {
                errors += 1;
            }
            if result.conflict {
                conflicts += 1;
            }
            retries += result.retry_count;
            reconnects += result.reconnect_count;
            all_latencies.push(result.elapsed_ms);
            if passed && result.successful {
                latencies.push(result.elapsed_ms);
            }
            let is_transport_error = result.value["transport_error"] == true;
            let mut value = result.value;
            value["session"] = json!(session_name);
            value["elapsed_seconds"] = json!(result.elapsed_ms / 1000.0);
            value["independent_command_id"] =
                json!(op.request.get("command_id").and_then(Value::as_str));
            value["event"] = json!("request");
            value["index"] = json!(step * args.sessions + index);
            value["outcome"] = json!(if passed && result.conflict {
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
    }
    for state in &sessions {
        if let Some(id) = state.id.as_deref() {
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
        &json!({"event":"finish","passed":errors==0,"selected_file_metadata_unchanged":true,"schedule_unchanged":true,"scheduled":total_ops,"started":successes+errors,"failures":errors,"conflicts":conflicts,"conflict_groups":conflict_groups,"conflict_group_failures":conflict_group_failures,"retries":retries,"reconnects":reconnects,"all_operation_latency_ms":{"samples":all_latencies.len(),"p50":all_quantile(0.50),"p95":all_quantile(0.95),"p99":all_quantile(0.99),"max":all_latencies.last().copied()},"successful_latency_ms":{"samples":latencies.len(),"p50":quantile(0.50),"p95":quantile(0.95),"p99":quantile(0.99),"max":latencies.last().copied()},"elapsed_seconds":start_time.elapsed().as_secs_f64(),"operation_window_seconds":start_metrics.elapsed().as_secs_f64(),"resource_delta":resource_delta(&resources_before,&resources_after),"report_storage_bytes_before_finish":storage_bytes,"report_storage_allocated_bytes_before_finish":storage_allocated,"scope":"finite Rust protocol measurement; not capacity acceptance"}),
    )?;
    output.sync_all().map_err(|e| format!("report sync: {e}"))?;
    Ok(if errors == 0 { 0 } else { 1 })
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
