//! Linux local Worker + exact-request Node supervisor. No capture/model/issuer.
//! Caller owns the genuine stage FD and original monotonic cutoff. Every child
//! leader stays unreaped until its process group has been terminated.
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    os::{
        fd::AsRawFd,
        unix::{
            fs::{OpenOptionsExt, PermissionsExt},
            process::CommandExt,
        },
    },
    path::{Path, PathBuf},
    process::{Child, ChildStderr, ChildStdout, Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};
use tos_foundation::{Digest256, JsonLimits, JsonMode, JsonValue, parse_json_with_state_budget};
static CANCELLED: AtomicBool = AtomicBool::new(false);
static ACTIVE: AtomicBool = AtomicBool::new(false);
extern "C" fn cancelled(_: libc::c_int) {
    CANCELLED.store(true, Ordering::Relaxed);
}
fn err() -> String {
    std::io::Error::last_os_error().to_string()
}
fn monotonic_ns() -> Result<u64, String> {
    let mut t = unsafe { std::mem::zeroed::<libc::timespec>() };
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut t) } != 0 {
        return Err(err());
    }
    u64::try_from(t.tv_sec)
        .ok()
        .and_then(|s| s.checked_mul(1_000_000_000))
        .and_then(|s| s.checked_add(t.tv_nsec as u64))
        .ok_or_else(|| "monotonic clock overflow".into())
}
struct Signals {
    installed: usize,
    previous: [libc::sigaction; 2],
}
impl Signals {
    fn install() -> Result<Self, String> {
        if ACTIVE.swap(true, Ordering::SeqCst) {
            return Err("only one local verifier supervisor per process".into());
        }
        CANCELLED.store(false, Ordering::Relaxed);
        let mut this = Self {
            installed: 0,
            previous: unsafe { std::mem::zeroed() },
        };
        let mut action = unsafe { std::mem::zeroed::<libc::sigaction>() };
        action.sa_sigaction = cancelled as *const () as usize;
        unsafe { libc::sigemptyset(&mut action.sa_mask) };
        for (i, sig) in [libc::SIGTERM, libc::SIGINT].iter().enumerate() {
            if unsafe { libc::sigaction(*sig, &action, &mut this.previous[i]) } != 0 {
                return Err(err());
            }
            this.installed += 1;
        }
        Ok(this)
    }
}
impl Drop for Signals {
    fn drop(&mut self) {
        for (i, sig) in [libc::SIGTERM, libc::SIGINT]
            .iter()
            .enumerate()
            .take(self.installed)
        {
            unsafe { libc::sigaction(*sig, &self.previous[i], std::ptr::null_mut()) };
        }
        ACTIVE.store(false, Ordering::SeqCst);
    }
}
struct SpawnMask {
    previous: libc::sigset_t,
    active: bool,
}
impl SpawnMask {
    fn block() -> Result<Self, String> {
        let mut blocked = unsafe { std::mem::zeroed() };
        let mut previous = unsafe { std::mem::zeroed() };
        unsafe {
            libc::sigemptyset(&mut blocked);
            libc::sigaddset(&mut blocked, libc::SIGTERM);
            libc::sigaddset(&mut blocked, libc::SIGINT)
        };
        let result = unsafe { libc::pthread_sigmask(libc::SIG_BLOCK, &blocked, &mut previous) };
        if result != 0 {
            return Err(std::io::Error::from_raw_os_error(result).to_string());
        }
        Ok(Self {
            previous,
            active: true,
        })
    }
    fn restore(&mut self) -> Result<(), String> {
        if self.active {
            let result = unsafe {
                libc::pthread_sigmask(libc::SIG_SETMASK, &self.previous, std::ptr::null_mut())
            };
            if result != 0 {
                return Err(std::io::Error::from_raw_os_error(result).to_string());
            }
            self.active = false;
        }
        Ok(())
    }
}
impl Drop for SpawnMask {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}
struct Group {
    child: Child,
    out: ChildStdout,
    stderr: ChildStderr,
    log: File,
    owned: bool,
    reaped: bool,
    cleanup_started: bool,
    cleanup_deadline: Instant,
    maximum_proc_entries: usize,
}
impl Group {
    fn spawn(
        mut command: Command,
        log_path: &Path,
        fd: Option<i32>,
        cleanup_deadline: Instant,
        maximum_proc_entries: usize,
    ) -> Result<Self, String> {
        let log = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(log_path)
            .map_err(|e| e.to_string())?;
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut mask = SpawnMask::block()?;
        let previous = mask.previous;
        unsafe {
            command.pre_exec(move || {
                if libc::setsid() == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                // Restore default cancellation in each leader while parent keeps handlers.
                libc::signal(libc::SIGTERM, libc::SIG_DFL);
                libc::signal(libc::SIGINT, libc::SIG_DFL);
                // CLOEXEC every inherited non-stdio descriptor, then preserve only the
                // genuine caller stage descriptor for Node. No Worker stage leak.
                if libc::syscall(libc::SYS_close_range, 3u32, u32::MAX, 4u32) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if let Some(fd) = fd {
                    let flags = libc::fcntl(fd, libc::F_GETFD);
                    if flags < 0 || libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                }
                let result =
                    libc::pthread_sigmask(libc::SIG_SETMASK, &previous, std::ptr::null_mut());
                if result != 0 {
                    return Err(std::io::Error::from_raw_os_error(result));
                }
                Ok(())
            });
        }
        let spawned = command.spawn();
        let mut child = match spawned {
            Ok(child) => child,
            Err(e) => {
                let restoration = mask.restore();
                return Err(format!(
                    "spawn failed: {e}; signal restoration: {restoration:?}"
                ));
            }
        };
        let out = child.stdout.take().ok_or("spawn stdout absent")?;
        let stderr = child.stderr.take().ok_or("spawn stderr absent")?;
        let mut group = Self {
            child,
            out,
            stderr,
            log,
            owned: true,
            reaped: false,
            cleanup_started: false,
            cleanup_deadline,
            maximum_proc_entries,
        };
        for fd in [group.out.as_raw_fd(), group.stderr.as_raw_fd()] {
            let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
            if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0
            {
                return Err(err());
            }
        }
        if let Err(e) = mask.restore() {
            let cleanup = group.stop();
            return Err(format!(
                "signal restoration failed: {e}; cleanup: {cleanup:?}"
            ));
        }
        Ok(group)
    }
    fn id(&self) -> i32 {
        self.child.id() as i32
    }
    fn exited(&mut self) -> Result<Option<i32>, String> {
        if !self.owned || self.reaped {
            return Err(
                "leader ownership unavailable; external containment cleanup required".into(),
            );
        }
        let mut info = unsafe { std::mem::zeroed::<libc::siginfo_t>() };
        if unsafe {
            libc::waitid(
                libc::P_PID,
                self.id() as libc::id_t,
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        } != 0
        {
            self.owned = false;
            return Err(err());
        }
        let pid = unsafe { info.si_pid() };
        if pid == 0 {
            return Ok(None);
        }
        if pid != self.id() {
            self.owned = false;
            return Err("unexpected direct child identity".into());
        }
        Ok(Some(if info.si_code == libc::CLD_EXITED {
            unsafe { info.si_status() }
        } else {
            128 + unsafe { info.si_status() }
        }))
    }
    fn signal(&mut self, sig: i32) -> Result<(), String> {
        self.exited()?;
        if unsafe { libc::kill(-self.id(), sig) } == 0 {
            return Ok(());
        }
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ESRCH) {
            Ok(())
        } else {
            Err(error.to_string())
        }
    }
    fn pump(&mut self, total: &mut usize, maximum: usize) -> Result<(), String> {
        // Four bounded reads per pipe per sweep; producers cannot starve cutoff.
        for stream in [
            &mut self.out as &mut dyn Read,
            &mut self.stderr as &mut dyn Read,
        ] {
            for _ in 0..4 {
                let mut buffer = [0u8; 4096];
                match stream.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(n) => {
                        *total = total.checked_add(n).ok_or("log byte overflow")?;
                        if *total > maximum {
                            return Err("aggregate Worker+Node logs exceeded admission".into());
                        }
                        self.log
                            .write_all(&buffer[..n])
                            .map_err(|e| e.to_string())?
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(e.to_string()),
                }
            }
        }
        Ok(())
    }
    fn live_group_members(&self) -> Result<bool, String> {
        // The zombie leader keeps PGID reserved. kill(0) cannot distinguish that
        // zombie from surviving descendants, so check bounded Linux proc status.
        for (count, entry) in fs::read_dir("/proc")
            .map_err(|e| e.to_string())?
            .enumerate()
        {
            if count >= self.maximum_proc_entries || Instant::now() >= self.cleanup_deadline {
                return Err("process-group confirmation bound exhausted; external containment cleanup required".into());
            }
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.file_name().to_string_lossy().parse::<u32>().is_err() {
                continue;
            }
            let path = entry.path().join("stat");
            let bytes = match bounded_file(&path, 4096) {
                Ok(bytes) => bytes,
                Err(e) => {
                    if !path.exists() {
                        continue;
                    }
                    return Err(e);
                }
            };
            let text = std::str::from_utf8(&bytes).map_err(|e| e.to_string())?;
            let (_, tail) = text.rsplit_once(')').ok_or("malformed process stat")?;
            let columns: Vec<&str> = tail.split_whitespace().collect();
            if columns.len() < 4 {
                return Err("short process stat".into());
            }
            if columns[2].parse::<i32>().map_err(|e| e.to_string())? == self.id()
                && columns[0] != "Z"
                && columns[0] != "X"
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
    fn stop(&mut self) -> Result<(), String> {
        if self.reaped {
            return Ok(());
        }
        if self.cleanup_started {
            return Err("cleanup already attempted; external containment cleanup required".into());
        }
        self.cleanup_started = true;
        // Whole cutoff is finite; no additional private ten-second extension.
        // KILL every group on every terminal path, including successful Node exit,
        // before reaping the leader. Node normally already finalized its own Core.
        self.signal(libc::SIGKILL)?;
        loop {
            if self.exited()?.is_some() && !self.live_group_members()? {
                self.child.wait().map_err(|e| e.to_string())?;
                self.reaped = true;
                self.owned = false;
                return Ok(());
            }
            if Instant::now() >= self.cleanup_deadline {
                return Err(
                    "cleanup deadline exhausted; external containment cleanup required".into(),
                );
            }
            thread::sleep(Duration::from_millis(5));
        }
    }
}
impl Drop for Group {
    fn drop(&mut self) {
        if !self.reaped && !self.cleanup_started {
            let _ = self.stop();
        }
    }
}
fn bounded_file(path: &Path, maximum: usize) -> Result<Vec<u8>, String> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|e| e.to_string())?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("selected input is not regular file".into());
    }
    let mut bytes = Vec::new();
    file.take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > maximum {
        return Err("selected input exceeds byte admission".into());
    }
    Ok(bytes)
}
fn json(raw: &[u8], maximum: usize) -> Result<tos_foundation::JsonDocument, String> {
    parse_json_with_state_budget(
        raw,
        JsonMode::PublishedStrict,
        JsonLimits {
            max_bytes: maximum,
            max_depth: 32,
            max_visits: 8192,
            max_integer_digits: 20,
        },
        maximum.checked_mul(64).ok_or("parser state overflow")?,
    )
    .map_err(|e| e.to_string())
}
fn text<'a>(v: &'a JsonValue, key: &str) -> Result<&'a str, String> {
    v.object_get(key)
        .and_then(JsonValue::as_str)
        .ok_or_else(|| format!("missing string {key}"))
}
fn number(v: &JsonValue, key: &str) -> Result<u64, String> {
    v.object_get(key)
        .and_then(JsonValue::as_u64)
        .filter(|n| *n > 0)
        .ok_or_else(|| format!("missing positive integer {key}"))
}
fn canonical(path: &Path) -> Result<(), String> {
    if !path.is_absolute() || fs::canonicalize(path).map_err(|e| e.to_string())? != path {
        return Err("selected paths must be canonical absolute".into());
    }
    Ok(())
}
/// All resource limits selected by the native caller; original cutoff never rewritten.
#[derive(Debug)]
pub struct SupervisionOptions {
    pub worker_root: PathBuf,
    pub node_exe: PathBuf,
    pub node_script: PathBuf,
    pub request_path: PathBuf,
    pub worker_port: u16,
    pub scratch_root: PathBuf,
    pub work_deadline_ns: u64,
    pub maximum_seconds: u64,
    pub maximum_request_bytes: usize,
    pub maximum_marker_bytes: usize,
    pub maximum_health_bytes: usize,
    pub maximum_log_bytes: usize,
    pub maximum_readiness_ms: u64,
    pub maximum_shutdown_ms: u64,
    pub maximum_proc_entries: usize,
    pub maximum_fd_entries: usize,
}
fn markers(o: &SupervisionOptions, revision: &str, digest: &str) -> Result<(), String> {
    let a = bounded_file(
        &o.worker_root.join("runtime/manifest.json"),
        o.maximum_marker_bytes,
    )?;
    let b = bounded_file(
        &o.worker_root.join("dist/__edge/build-manifest.json"),
        o.maximum_marker_bytes,
    )?;
    if a != b || Digest256::of_bytes(&a).to_hex() != digest {
        return Err("completion pair differs from selected raw request".into());
    }
    let doc = json(&a, o.maximum_marker_bytes)?;
    if text(doc.root(), "data_revision")? != revision
        || text(doc.root(), "schema")? != "tos_cloudflare_edge_build_v1"
        || text(doc.root(), "read_model_schema")? != "tos_cloudflare_edge_read_model_v9"
    {
        return Err("completion identity differs".into());
    }
    Ok(())
}
fn health(
    o: &SupervisionOptions,
    until: Instant,
    revision: &str,
    wire: &mut Vec<u8>,
) -> Result<(), String> {
    wire.clear();
    let remaining = until
        .checked_duration_since(Instant::now())
        .ok_or("readiness cutoff exhausted")?
        .min(Duration::from_millis(100));
    let address: SocketAddr = format!("127.0.0.1:{}", o.worker_port)
        .parse()
        .map_err(|e: std::net::AddrParseError| e.to_string())?;
    let mut stream = TcpStream::connect_timeout(&address, remaining).map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(remaining))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(remaining))
        .map_err(|e| e.to_string())?;
    write!(
        stream,
        "GET /health HTTP/1.0\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n",
        o.worker_port
    )
    .map_err(|e| e.to_string())?;
    let limit = o
        .maximum_health_bytes
        .checked_add(8192)
        .ok_or("health bound overflow")?;
    let mut buffer = [0u8; 4096];
    loop {
        if Instant::now() >= until {
            return Err("readiness cutoff exhausted".into());
        }
        stream
            .set_read_timeout(Some(
                until
                    .checked_duration_since(Instant::now())
                    .ok_or("readiness cutoff exhausted")?
                    .min(Duration::from_millis(100)),
            ))
            .map_err(|e| e.to_string())?;
        let n = stream.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        if wire.len() + n > limit {
            return Err("health wire exceeds admission".into());
        }
        wire.extend_from_slice(&buffer[..n]);
    }
    let split = wire
        .windows(4)
        .position(|p| p == b"\r\n\r\n")
        .ok_or("health header absent")?;
    if split > 8192 || !(wire.starts_with(b"HTTP/1.0 200 ") || wire.starts_with(b"HTTP/1.1 200 ")) {
        return Err("Worker health status/header refused".into());
    }
    let header = std::str::from_utf8(&wire[..split]).map_err(|e| e.to_string())?;
    let transfer = header
        .lines()
        .filter_map(|line| line.split_once(':'))
        .find(|(key, _)| key.eq_ignore_ascii_case("transfer-encoding"));
    let body = &wire[split + 4..];
    let decoded;
    let body = if let Some((_, value)) = transfer {
        if !value.trim().eq_ignore_ascii_case("chunked") {
            return Err("unsupported health transfer coding".into());
        }
        decoded = unchunk(body, o.maximum_health_bytes)?;
        decoded.as_slice()
    } else {
        body
    };
    let doc = json(body, o.maximum_health_bytes)?;
    if text(doc.root(), "data_revision")? != revision
        || text(doc.root(), "runtime")? != "cloudflare-worker"
        || doc.root().object_get("ok").and_then(JsonValue::as_bool) != Some(true)
    {
        return Err("Worker bound health differs".into());
    }
    Ok(())
}

fn unchunk(mut wire: &[u8], maximum: usize) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    loop {
        let line = wire
            .windows(2)
            .position(|p| p == b"\r\n")
            .ok_or("chunk header absent")?;
        if line > 32 {
            return Err("chunk size header exceeds bound".into());
        }
        let n = usize::from_str_radix(
            std::str::from_utf8(&wire[..line]).map_err(|e| e.to_string())?,
            16,
        )
        .map_err(|e| e.to_string())?;
        wire = &wire[line + 2..];
        if n == 0 {
            if wire != b"\r\n" {
                return Err("health trailers refused".into());
            }
            return Ok(bytes);
        }
        if n > maximum - bytes.len() || wire.len() < n + 2 || &wire[n..n + 2] != b"\r\n" {
            return Err("health chunk exceeds bound or malformed".into());
        }
        bytes.extend_from_slice(&wire[..n]);
        wire = &wire[n + 2..];
    }
}
fn worker_listener_owned(
    group: &Group,
    o: &SupervisionOptions,
    until: Instant,
) -> Result<bool, String> {
    let raw = bounded_file(
        Path::new("/proc/net/tcp"),
        o.maximum_proc_entries
            .checked_mul(256)
            .ok_or("socket table bound overflow")?,
    )?;
    let target = format!("0100007F:{:04X}", o.worker_port);
    let mut inode = None;
    for line in std::str::from_utf8(&raw)
        .map_err(|e| e.to_string())?
        .lines()
        .skip(1)
    {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 10 {
            return Err("malformed Linux socket table".into());
        }
        if fields[1] == target && fields[3] == "0A" {
            if inode.replace(fields[9].to_owned()).is_some() {
                return Err("ambiguous Worker listener".into());
            }
        }
    }
    let Some(inode) = inode else { return Ok(false) };
    let socket = format!("socket:[{inode}]");
    let mut fd_count = 0usize;
    for (count, entry) in fs::read_dir("/proc")
        .map_err(|e| e.to_string())?
        .enumerate()
    {
        if count >= o.maximum_proc_entries || Instant::now() >= until {
            return Err("Worker listener ownership inspection exceeds admission".into());
        }
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_name().to_string_lossy().parse::<u32>().is_err() {
            continue;
        }
        let path = entry.path().join("stat");
        let raw = match bounded_file(&path, 4096) {
            Ok(raw) => raw,
            Err(e) => {
                if !path.exists() {
                    continue;
                }
                return Err(e);
            }
        };
        let text = std::str::from_utf8(&raw).map_err(|e| e.to_string())?;
        let (_, tail) = text.rsplit_once(')').ok_or("malformed process stat")?;
        let fields: Vec<&str> = tail.split_whitespace().collect();
        if fields.len() < 4 {
            return Err("short process stat".into());
        }
        if fields[2].parse::<i32>().map_err(|e| e.to_string())? != group.id() {
            continue;
        }
        let fds = match fs::read_dir(entry.path().join("fd")) {
            Ok(fds) => fds,
            Err(e) => {
                if !entry.path().exists() {
                    continue;
                }
                return Err(e.to_string());
            }
        };
        for fd in fds {
            fd_count += 1;
            if fd_count > o.maximum_fd_entries || Instant::now() >= until {
                return Err("Worker owned FD inspection exceeds admission".into());
            }
            let fd = fd.map_err(|e| e.to_string())?;
            match fs::read_link(fd.path()) {
                Ok(target) if target.as_os_str() == std::ffi::OsStr::new(&socket) => {
                    return Ok(true);
                }
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.to_string()),
            }
        }
    }
    Err("selected Worker listener is outside owned process group".into())
}

/// Runs the actual maintained local adapter and Node client. No host CLI required.
/// Process-global SIGTERM/SIGINT scope: caller must not use concurrent supervisors.
pub fn supervise(o: &SupervisionOptions) -> Result<(), String> {
    let started = Instant::now();
    let now = monotonic_ns()?;
    let ns = o
        .work_deadline_ns
        .checked_sub(now)
        .ok_or("original cutoff expired")?;
    if ns == 0
        || ns
            > o.maximum_seconds
                .checked_mul(1_000_000_000)
                .ok_or("seconds overflow")?
    {
        return Err("original cutoff exceeds explicit finite seconds admission".into());
    }
    let whole = started
        .checked_add(Duration::from_nanos(ns))
        .ok_or("cutoff overflow")?;
    let work = whole
        .checked_sub(Duration::from_millis(o.maximum_shutdown_ms))
        .filter(|v| *v > started)
        .ok_or("no work window after cleanup reserve")?;
    if o.worker_port == 0
        || [
            o.maximum_request_bytes,
            o.maximum_marker_bytes,
            o.maximum_health_bytes,
            o.maximum_log_bytes,
            o.maximum_proc_entries,
            o.maximum_fd_entries,
        ]
        .contains(&0)
        || o.maximum_readiness_ms == 0
        || o.maximum_shutdown_ms == 0
    {
        return Err("all limits must be explicit positive finite values".into());
    }
    for path in [
        &o.worker_root,
        &o.node_exe,
        &o.node_script,
        &o.request_path,
        &o.scratch_root,
    ] {
        canonical(path)?;
    }
    if !o.worker_root.is_dir()
        || !o.scratch_root.is_dir()
        || fs::metadata(&o.scratch_root)
            .map_err(|e| e.to_string())?
            .permissions()
            .mode()
            & 0o077
            != 0
    {
        return Err("existing canonical private scratch directory required".into());
    }
    let raw = bounded_file(&o.request_path, o.maximum_request_bytes)?;
    let doc = json(&raw, o.maximum_request_bytes)?;
    let request = doc.root();
    if text(request, "schema")? != "tos_native_worker_full_verification_v1"
        || text(request, "worker_root")? != o.worker_root.to_str().ok_or("non-UTF8 Worker root")?
        || text(request, "worker_base")? != format!("http://127.0.0.1:{}", o.worker_port)
    {
        return Err("Worker wrapper selection differs from raw request".into());
    }
    let revision = text(request, "data_revision")?;
    Digest256::from_hex(revision).map_err(|e| e.to_string())?;
    let marker = text(request, "completion_marker_sha256")?;
    Digest256::from_hex(marker).map_err(|e| e.to_string())?;
    let startup = request
        .object_get("native_startup")
        .ok_or("native startup absent")?;
    if text(startup, "work_deadline_ns")? != o.work_deadline_ns.to_string() {
        return Err("original native monotonic cutoff differs".into());
    }
    let fd = i32::try_from(number(startup, "stage_ticket_fd")?).map_err(|e| e.to_string())?;
    if fd < 3 || unsafe { libc::fcntl(fd, libc::F_GETFD) } < 0 {
        return Err("genuine inherited caller stage FD absent".into());
    }
    let _signals = Signals::install()?;
    markers(o, revision, marker)?;
    let address = format!("127.0.0.1:{}", o.worker_port);
    let probe =
        TcpListener::bind(&address).map_err(|e| format!("Worker port already occupied: {e}"))?;
    drop(probe);
    let wrangler = o.worker_root.join("node_modules/.bin/wrangler");
    if !wrangler.is_file() {
        return Err("installed local Wrangler required; no fetch/install fallback".into());
    }
    let mut command = Command::new(wrangler);
    command
        .args(["dev", "--local", "--ip", "127.0.0.1", "--port"])
        .arg(o.worker_port.to_string())
        .current_dir(&o.worker_root);
    command
        .env("npm_config_offline", "true")
        .env("npm_config_audit", "false")
        .env("npm_config_fund", "false")
        .env("WRANGLER_SEND_METRICS", "false")
        .env("NO_UPDATE_NOTIFIER", "1")
        .env("XDG_CACHE_HOME", o.scratch_root.join("xdg-cache"))
        .env("XDG_CONFIG_HOME", o.scratch_root.join("xdg-config"))
        .env("npm_config_cache", o.scratch_root.join("npm-cache"))
        .env("TMPDIR", &o.scratch_root);
    for key in [
        "TOS_QUERY_STORE_PATH",
        "TOS_RELEASE_ROOT",
        "TOS_VERIFY_SOURCE_ROOT",
        "TOS_VERIFY_PROFILE",
        "TOS_VERIFY_EXPECT_DELTA_FROM",
    ] {
        command.env_remove(key);
    }
    let mut worker = Group::spawn(
        command,
        &o.scratch_root.join("worker.log"),
        None,
        whole,
        o.maximum_proc_entries,
    )?;
    let mut node: Option<Group> = None;
    let mut total = 0usize;
    let mut health_diagnostics = 0usize;
    let mut preliminary_health_diagnostic = false;
    let mut health_signatures = [None; 8];
    let result = (|| {
        let ready = work.min(
            started
                .checked_add(Duration::from_millis(o.maximum_readiness_ms))
                .ok_or("readiness overflow")?,
        );
        loop {
            worker.pump(&mut total, o.maximum_log_bytes)?;
            if CANCELLED.load(Ordering::Relaxed) || Instant::now() >= ready {
                return Err("Worker readiness cancelled or cutoff exhausted".into());
            }
            if worker.exited()?.is_some() {
                return Err("Worker leader exited before readiness".into());
            }
            let mut health_wire = Vec::new();
            let listener_owned = worker_listener_owned(&worker, o, ready)?;
            let healthy = if listener_owned {
                match health(o, ready, revision, &mut health_wire) {
                    Ok(()) => true,
                    Err(reason) => {
                        // Preserve bounded transport evidence in the existing Worker log.
                        // The complete response is still refused by the unchanged health
                        // predicate; a prefix here is explicitly diagnostic only.
                        let header_end = health_wire.windows(4).position(|p| p == b"\r\n\r\n");
                        let status_200 = header_end.is_some()
                            && (health_wire.starts_with(b"HTTP/1.0 200 ")
                                || health_wire.starts_with(b"HTTP/1.1 200 "));
                        let body_start = header_end.map_or(0, |p| p + 4);
                        let body_prefix =
                            &health_wire[body_start..health_wire.len().min(body_start + 4096)];
                        let signature = (
                            Digest256::of_bytes(reason.as_bytes()),
                            Digest256::of_bytes(body_prefix),
                        );
                        if health_diagnostics < 8
                            && (status_200 || !preliminary_health_diagnostic)
                            && !health_signatures[..health_diagnostics].contains(&Some(signature))
                        {
                            let prefix = &health_wire[..health_wire.len().min(4096)];
                            let reason = &reason.as_bytes()[..reason.len().min(256)];
                            let parts: [&[u8]; 7] = [
                                b"\n[readiness health refusal; expected data_revision=",
                                revision.as_bytes(),
                                b"; reason=",
                                reason,
                                b"; raw wire prefix follows, at most 4096 bytes]\n",
                                prefix,
                                b"\n[end readiness wire prefix]\n",
                            ];
                            let bytes = parts
                                .iter()
                                .try_fold(0usize, |n, p| n.checked_add(p.len()))
                                .ok_or("health diagnostic bound overflow")?;
                            total = total.checked_add(bytes).ok_or("log aggregate overflow")?;
                            if total > o.maximum_log_bytes {
                                return Err("aggregate Worker+Node logs exceeded admission".into());
                            }
                            for part in parts {
                                worker.log.write_all(part).map_err(|e| e.to_string())?;
                            }
                            preliminary_health_diagnostic |= !status_200;
                            health_signatures[health_diagnostics] = Some(signature);
                            health_diagnostics += 1;
                        }
                        false
                    }
                }
            } else {
                false
            };
            if healthy {
                markers(o, revision, marker)?;
                if !worker_listener_owned(&worker, o, ready)? {
                    return Err("Worker listener ownership changed across readiness".into());
                }
                if worker.exited()?.is_some() {
                    return Err("Worker ownership exited across readiness".into());
                }
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        if bounded_file(&o.request_path, o.maximum_request_bytes)? != raw {
            return Err("raw verifier request changed before Node dispatch".into());
        }
        let mut command = Command::new(&o.node_exe);
        command
            .arg(&o.node_script)
            .args(["--max-seconds", &o.maximum_seconds.to_string(), "--request"])
            .arg(&o.request_path)
            .args([
                "--work-deadline-ns",
                &o.work_deadline_ns.to_string(),
                "--worker-base-url",
                &format!("http://127.0.0.1:{}", o.worker_port),
            ]);
        for key in ["TOS_QUERY_STORE_PATH", "TOS_RELEASE_ROOT"] {
            command.env_remove(key);
        }
        node = Some(Group::spawn(
            command,
            &o.scratch_root.join("node.log"),
            Some(fd),
            whole,
            o.maximum_proc_entries,
        )?);
        loop {
            worker.pump(&mut total, o.maximum_log_bytes)?;
            let client = node.as_mut().ok_or("Node not owned")?;
            client.pump(&mut total, o.maximum_log_bytes)?;
            if CANCELLED.load(Ordering::Relaxed) || Instant::now() >= work {
                return Err(
                    "verifier cancelled or work cutoff reached; cleanup reserve entered".into(),
                );
            }
            if worker.exited()?.is_some() {
                return Err("Worker leader exited during verifier".into());
            }
            if let Some(code) = client.exited()? {
                if code != 0 {
                    return Err(format!(
                        "Node verifier exit {code}; private evidence retained"
                    ));
                }
                markers(o, revision, marker)?;
                if bounded_file(&o.request_path, o.maximum_request_bytes)? != raw {
                    return Err("raw verifier request changed during Node call".into());
                }
                if Instant::now() >= work {
                    return Err("post-verifier work cutoff exhausted".into());
                }
                return Ok(());
            }
            thread::sleep(Duration::from_millis(5));
        }
    })();
    // Attempt both cleanup paths even when one refuses. Never signal stale IDs.
    let node_cleanup = match node.as_mut() {
        Some(group) => group.stop(),
        None => Ok(()),
    };
    let worker_cleanup = worker.stop();
    if node_cleanup.is_err() || worker_cleanup.is_err() {
        return Err(format!(
            "verification: {result:?}; Node cleanup: {node_cleanup:?}; Worker cleanup: {worker_cleanup:?}; private evidence retained, external containment cleanup required"
        ));
    }
    if Instant::now() >= whole {
        return Err(format!(
            "whole cutoff exhausted after cleanup; verification: {result:?}; private evidence retained"
        ));
    }
    result
}
pub fn run_if_requested(args: &[String]) -> Option<i32> {
    if args.first().map(String::as_str) != Some("verify-edge-local") {
        return None;
    }
    Some(match parse_options(args).and_then(|o| supervise(&o)) {
        Ok(()) => {
            println!(
                "{{\"schema\":\"tos_edge_local_supervision_v1\",\"verified\":true,\"cleanup\":\"owned_groups_completed\"}}"
            );
            0
        }
        Err(e) => {
            eprintln!("{e}");
            2
        }
    })
}
fn parse_options(args: &[String]) -> Result<SupervisionOptions, String> {
    let mut values = std::collections::BTreeMap::new();
    let mut i = 1;
    while i < args.len() {
        let key = args[i].as_str();
        let value = args.get(i + 1).ok_or("option needs value")?;
        if values.insert(key, value.as_str()).is_some() {
            return Err(format!("duplicate option {key}"));
        }
        i += 2;
    }
    let mut get = |key: &str| values.remove(key).ok_or_else(|| format!("missing {key}"));
    let worker_root = PathBuf::from(get("--worker-root")?);
    let node_exe = PathBuf::from(get("--node-exe")?);
    let node_script = PathBuf::from(get("--node-script")?);
    let request_path = PathBuf::from(get("--request")?);
    let scratch_root = PathBuf::from(get("--scratch-root")?);
    macro_rules! num {
        ($name:literal,$ty:ty) => {
            get($name)?
                .parse::<$ty>()
                .map_err(|e| format!("{}: {e}", $name))?
        };
    }
    let o = SupervisionOptions {
        worker_root,
        node_exe,
        node_script,
        request_path,
        scratch_root,
        worker_port: num!("--worker-port", u16),
        work_deadline_ns: num!("--work-deadline-ns", u64),
        maximum_seconds: num!("--max-seconds", u64),
        maximum_request_bytes: num!("--maximum-request-bytes", usize),
        maximum_marker_bytes: num!("--maximum-marker-bytes", usize),
        maximum_health_bytes: num!("--maximum-health-bytes", usize),
        maximum_log_bytes: num!("--maximum-log-bytes", usize),
        maximum_readiness_ms: num!("--maximum-readiness-ms", u64),
        maximum_shutdown_ms: num!("--maximum-shutdown-ms", u64),
        maximum_proc_entries: num!("--maximum-proc-entries", usize),
        maximum_fd_entries: num!("--maximum-fd-entries", usize),
    };
    if !values.is_empty() {
        return Err("unknown supervisor option".into());
    }
    Ok(o)
}
