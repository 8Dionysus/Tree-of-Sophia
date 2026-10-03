//! Future returning caller for the existing private native v2 operation.
//! Software association is verified separately; this transport grants neither
//! data/source admission nor execution authority. Caller owns admitted storage,
//! selected transactions and exclusive connection use until this call returns.
//! Caller also owns child reaping exclusively (no competing waitpid/SIGCHLD
//! reaper). Cancellation checks must be thread-safe, repeatable token checks.
//! Failed scopes are retained; successful scopes contain only transport files
//! and are removed after the anchored child group is proved released.

use super::typed_snapshot;
use crate::software_archive::installed::{
    InstalledAccessBudget, SelectedRole, VerifiedInstalledAccess,
};
use rusqlite::Connection;
use std::{
    ffi::CString,
    fs::{self, File},
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::fs::MetadataExt,
    },
    path::{Path, PathBuf},
    ptr,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};
use tos_foundation::{
    Digest256, Digest256Hasher, JsonLimits, JsonMode, JsonNumber, JsonNumberKind, JsonString,
    JsonValue, emit_value_preserved_json, parse_json_with_state_budget,
};

const REQUEST_BYTES: usize = 10 * 1024 * 1024;
const EVIDENCE_BYTES: usize = 8192;
const BUFFER_BYTES: usize = 65536;
static SCOPES: AtomicU64 = AtomicU64::new(1);
type Result<T> = std::result::Result<T, String>;

pub(crate) struct SelectedView<'a> {
    pub(crate) input_field: &'a str,
    pub(crate) connection: &'a Connection,
}
/// Structural/IO bounds, not an allocator/RSS promise. Frame/schema charges
/// use the encoder's shared ledger; request and output JSON have separate state
/// caps. IO also covers census/currentness and retained evidence writes.
pub(crate) struct CaptureLimits {
    pub(crate) frame_bytes: u64,
    pub(crate) schema_bytes: u64,
    pub(crate) request_state_bytes: usize,
    pub(crate) result_state_bytes: usize,
    pub(crate) stream_bytes: usize,
    pub(crate) metadata_bytes: usize,
    pub(crate) state_bytes: usize,
    pub(crate) io_bytes: u64,
    pub(crate) held_fds: usize,
    pub(crate) census_entries: usize,
}
pub(crate) struct CaptureFailure<'a> {
    pub(crate) reason: String,
    pub(crate) retained_directory: Option<PathBuf>,
    /// False means the caller must retain selected connections/frames and hand
    /// child custody to its outer supervisor; this function cannot prove release.
    pub(crate) child_released: bool,
    pub(crate) metadata_complete: bool,
    // Borrowed selections remain pinned by the returned failure until its owner
    // resolves custody. This is especially necessary when cleanup is unproven.
    pub(crate) selected_views: &'a [SelectedView<'a>],
    pub(crate) selected_image: &'a VerifiedInstalledAccess,
    custody: Option<CaptureCustody>,
}
struct CaptureCustody {
    child: Option<Child>,
    scope: Option<Scope>,
    frames: Vec<HeldFile>,
}
impl CaptureFailure<'_> {
    pub(crate) fn unreaped_child_pid(&self) -> Option<libc::pid_t> {
        self.custody
            .as_ref()
            .and_then(|c| c.child.as_ref())
            .filter(|c| !c.released)
            .map(|c| c.pid)
    }
}
struct LimitedWriter<'a> {
    raw: &'a mut Vec<u8>,
    cap: usize,
}
impl Write for LimitedWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.cap.saturating_sub(self.raw.len()) {
            return Err(std::io::Error::other("bounded JSON evidence"));
        }
        self.raw.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
struct Ledger {
    state: usize,
    io: u64,
    metadata: usize,
}
impl Ledger {
    fn state(&mut self, n: usize, limits: &CaptureLimits) -> Result<()> {
        self.state = self
            .state
            .checked_add(n)
            .filter(|v| *v <= limits.state_bytes)
            .ok_or("native selected state budget")?;
        Ok(())
    }
    fn io(&mut self, n: u64, limits: &CaptureLimits) -> Result<()> {
        self.io = self
            .io
            .checked_add(n)
            .filter(|v| *v <= limits.io_bytes)
            .ok_or("native selected IO budget")?;
        Ok(())
    }
    fn metadata(&mut self, n: usize, limits: &CaptureLimits) -> Result<()> {
        self.metadata = self
            .metadata
            .checked_add(n)
            .filter(|v| *v <= limits.metadata_bytes)
            .ok_or("native selected evidence metadata budget")?;
        Ok(())
    }
}
fn active(deadline: Instant, cancel: &(dyn Fn() -> Result<()> + Sync)) -> Result<()> {
    if Instant::now() >= deadline {
        return Err("native selected original deadline elapsed".into());
    }
    cancel()
}
fn json_limits(bytes: usize) -> JsonLimits {
    JsonLimits {
        max_bytes: bytes,
        max_depth: 128,
        max_visits: 1_000_000,
        max_integer_digits: 4300,
    }
}
fn text(value: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(value))
}
fn number(value: u64) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: value.to_string(),
    })
}
fn member<'a>(value: &'a JsonValue, key: &str) -> Result<&'a JsonValue> {
    value
        .object_get(key)
        .ok_or_else(|| format!("native selected missing {key}"))
}
fn patch(value: &mut JsonValue, key: &str, replacement: JsonValue) -> Result<()> {
    let JsonValue::Object(fields) = value else {
        return Err("native selected request object required".into());
    };
    if let Some((_, value)) = fields
        .iter_mut()
        .find(|(name, _)| name.as_str() == Some(key))
    {
        *value = replacement;
    } else {
        fields.push((JsonString::from_utf8(key), replacement));
    }
    Ok(())
}
fn identity(file: &File) -> Result<(u64, u64, u64, u32, i64, i64, i64, i64)> {
    let m = file
        .metadata()
        .map_err(|_| "native selected descriptor metadata")?;
    Ok((
        m.dev(),
        m.ino(),
        m.len(),
        m.mode(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    ))
}
struct HeldFile {
    file: File,
    path: PathBuf,
    stamp: (u64, u64, u64, u32, i64, i64, i64, i64),
}
impl HeldFile {
    fn current(&self) -> Result<()> {
        let named = tos_fd_open::open_absolute_regular(&self.path, self.stamp.2)
            .map_err(|_| "native selected named file unavailable")?;
        if identity(&named)? != self.stamp || identity(&self.file)? != self.stamp {
            return Err("native selected file custody changed".into());
        }
        Ok(())
    }
}
fn create_at(directory: &File, leaf: &str) -> Result<File> {
    let leaf = CString::new(leaf).map_err(|_| "native selected filename NUL")?;
    // Fixed single-component leaves; exclusive creation in an owned held scope.
    let fd = unsafe {
        libc::openat(
            directory.as_raw_fd(),
            leaf.as_ptr(),
            libc::O_RDWR | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600,
        )
    };
    if fd < 0 {
        return Err("native selected exclusive file create".into());
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}
fn write_bounded(
    file: &mut File,
    raw: &[u8],
    deadline: Instant,
    cancel: &(dyn Fn() -> Result<()> + Sync),
    ledger: &mut Ledger,
    limits: &CaptureLimits,
) -> Result<()> {
    for chunk in raw.chunks(BUFFER_BYTES) {
        active(deadline, cancel)?;
        ledger.io(chunk.len() as u64, limits)?;
        file.write_all(chunk)
            .map_err(|_| "native selected evidence write")?;
    }
    active(deadline, cancel)?;
    file.sync_all()
        .map_err(|_| "native selected evidence sync")?;
    active(deadline, cancel)
}
struct Scope {
    root: File,
    directory: File,
    path: PathBuf,
    root_path: PathBuf,
    root_id: (u64, u64),
    directory_id: (u64, u64),
    leaf: String,
}
impl Scope {
    fn new(scratch: &Path, created: &mut Option<PathBuf>) -> Result<Self> {
        if !scratch.is_absolute() || scratch.as_os_str().len() > 4096 {
            return Err("native selected absolute scratch required".into());
        }
        let root = tos_fd_open::open_absolute_directory(scratch)
            .map_err(|_| "native selected scratch open")?;
        let m = root
            .metadata()
            .map_err(|_| "native selected scratch metadata")?;
        let root_id = (m.dev(), m.ino());
        let id = SCOPES
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .map_err(|_| "native selected scope identities exhausted")?;
        let leaf = format!("native-edge-{}-{id}", unsafe { libc::getpid() });
        let c = CString::new(leaf.as_str()).map_err(|_| "native selected scope name")?;
        if unsafe { libc::mkdirat(root.as_raw_fd(), c.as_ptr(), 0o700) } != 0 {
            return Err("native selected exclusive scope create".into());
        }
        *created = Some(scratch.join(&leaf));
        let directory = tos_fd_open::open_directory_at(&root, Path::new(&leaf))
            .map_err(|_| "native selected scope open")?;
        let m = directory
            .metadata()
            .map_err(|_| "native selected scope metadata")?;
        let directory_id = (m.dev(), m.ino());
        Ok(Self {
            root,
            directory,
            path: scratch.join(&leaf),
            root_path: scratch.to_owned(),
            root_id,
            directory_id,
            leaf,
        })
    }
    fn current(&self) -> Result<()> {
        let root = tos_fd_open::open_absolute_directory(&self.root_path)
            .map_err(|_| "native selected named scratch unavailable")?;
        let named = tos_fd_open::open_directory_at(&root, Path::new(&self.leaf))
            .map_err(|_| "native selected named scope unavailable")?;
        for (file, want) in [
            (&root, self.root_id),
            (&self.root, self.root_id),
            (&named, self.directory_id),
            (&self.directory, self.directory_id),
        ] {
            let m = file
                .metadata()
                .map_err(|_| "native selected scope metadata")?;
            if (m.dev(), m.ino()) != want {
                return Err("native selected scope custody changed".into());
            }
        }
        Ok(())
    }
    fn remove(&self, leaves: &[String], deadline: Instant) -> Result<()> {
        self.current()?;
        for leaf in leaves {
            if Instant::now() >= deadline {
                return Err("native selected cleanup deadline elapsed".into());
            }
            let c = CString::new(leaf.as_str()).map_err(|_| "native selected cleanup leaf")?;
            if unsafe { libc::unlinkat(self.directory.as_raw_fd(), c.as_ptr(), 0) } != 0 {
                return Err("native selected transport unlink failed".into());
            }
        }
        let c = CString::new(self.leaf.as_str()).map_err(|_| "native selected cleanup scope")?;
        if unsafe { libc::unlinkat(self.root.as_raw_fd(), c.as_ptr(), libc::AT_REMOVEDIR) } != 0 {
            return Err("native selected scope remove failed".into());
        }
        Ok(())
    }
}
fn pipe() -> Result<(File, File)> {
    let mut fds = [-1; 2];
    if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) } != 0 {
        return Err("native selected pipe create".into());
    }
    Ok((unsafe { File::from_raw_fd(fds[0]) }, unsafe {
        File::from_raw_fd(fds[1])
    }))
}
fn nonblocking(file: &File) -> Result<()> {
    let fd = file.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err("native selected nonblocking pipe".into());
    }
    Ok(())
}
struct Child {
    pid: libc::pid_t,
    released: bool,
    startup_error: bool,
}
impl Child {
    fn terminal(&self, deadline: Instant) -> Result<Option<i32>> {
        let mut info = unsafe { std::mem::zeroed::<libc::siginfo_t>() };
        loop {
            if Instant::now() >= deadline {
                return Err("native selected ownership observation deadline elapsed".into());
            }
            if unsafe {
                libc::waitid(
                    libc::P_PID,
                    self.pid as u32,
                    &mut info,
                    libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
                )
            } == 0
            {
                break;
            }
            if std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
                return Err("native selected owned child anchor unavailable".into());
            }
        }
        let pid = unsafe { info.si_pid() };
        if pid == 0 {
            return Ok(None);
        }
        if pid != self.pid {
            return Err("native selected owned child identity differs".into());
        }
        Ok(Some(if info.si_code == libc::CLD_EXITED {
            unsafe { info.si_status() }
        } else {
            -unsafe { info.si_status() }
        }))
    }
    fn signal(&self, signal: i32, deadline: Instant) -> Result<()> {
        self.terminal(deadline)?;
        // Also signal the owned leader directly: cancellation may arrive before
        // the fresh child has established its session/process group.
        if unsafe { libc::kill(self.pid, signal) } != 0
            && std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
        {
            return Err("native selected owned leader signal failed".into());
        }
        if unsafe { libc::kill(-self.pid, signal) } != 0
            && std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
        {
            return Err("native selected owned group signal failed".into());
        }
        Ok(())
    }
    fn release(
        &mut self,
        deadline: Instant,
        ledger: &mut Ledger,
        limits: &CaptureLimits,
    ) -> Result<()> {
        let mut failed = false;
        for (signal, allowance) in [
            (libc::SIGTERM, Duration::from_secs(1)),
            (libc::SIGKILL, Duration::from_secs(4)),
        ] {
            if Instant::now() >= deadline {
                return Err("native selected child cleanup deadline elapsed".into());
            }
            if self.signal(signal, deadline).is_err() {
                failed = true;
                continue;
            }
            let phase = (Instant::now() + allowance).min(deadline);
            while Instant::now() < phase {
                match live_group(self.pid, deadline, ledger, limits) {
                    Ok(false) => break,
                    Ok(true) => std::thread::sleep(Duration::from_millis(10)),
                    Err(_) => {
                        failed = true;
                        break;
                    }
                }
            }
        }
        if live_group(self.pid, deadline, ledger, limits)? {
            return Err("native selected owned group remains live".into());
        }
        while Instant::now() < deadline {
            if self.terminal(deadline)?.is_some() {
                let mut status = 0;
                let pid = unsafe { libc::waitpid(self.pid, &mut status, libc::WNOHANG) };
                if pid == self.pid {
                    self.released = true;
                    return if failed {
                        Err("native selected group cleanup observation failed".into())
                    } else {
                        Ok(())
                    };
                }
                if pid < 0 {
                    return Err("native selected child reap ownership lost".into());
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Err("native selected owned child remains unreaped".into())
    }
}
fn live_group(
    pgid: libc::pid_t,
    deadline: Instant,
    ledger: &mut Ledger,
    limits: &CaptureLimits,
) -> Result<bool> {
    if Instant::now() >= deadline {
        return Err("native selected census deadline elapsed".into());
    }
    let entries = fs::read_dir("/proc").map_err(|_| "native selected group census open")?;
    let mut seen = 0usize;
    let mut raw = [0u8; 4097];
    let mut live = false;
    for entry in entries {
        if Instant::now() >= deadline {
            return Err("native selected census deadline elapsed".into());
        }
        seen = seen
            .checked_add(1usize)
            .ok_or("native selected census count overflow")?;
        if seen > limits.census_entries {
            return Err("native selected census entry budget".into());
        }
        let entry = entry.map_err(|_| "native selected census entry")?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !name.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let path = entry.path().join("stat");
        let mut file = match File::open(&path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return Err("native selected census source unavailable".into()),
        };
        ledger.io(raw.len() as u64, limits)?;
        let n = file
            .read(&mut raw)
            .map_err(|_| "native selected census read")?;
        if n > 4096 {
            return Err("native selected census stat budget".into());
        }
        // task comm is arbitrary bytes; only the ASCII numeric/state tail
        // after the last ')' is relevant to bounded process-group census.
        let end = raw[..n]
            .iter()
            .rposition(|byte| *byte == b')')
            .ok_or("native selected census stat shape")?;
        let tail = std::str::from_utf8(&raw[end + 1..n])
            .map_err(|_| "native selected census tail encoding")?;
        let mut fields = tail.split_whitespace();
        let state = fields.next().ok_or("native selected census state")?;
        let _parent = fields.next().ok_or("native selected census parent")?;
        let group = fields
            .next()
            .ok_or("native selected census group")?
            .parse::<i32>()
            .map_err(|_| "native selected census group integer")?;
        if group == pgid && !matches!(state, "Z" | "X") {
            live = true;
        }
    }
    Ok(live)
}
fn spawn(
    image: &File,
    argv: &[CString],
    environment: &[CString],
    out: &File,
    err: &File,
    null: &File,
    ledger: &mut Ledger,
    limits: &CaptureLimits,
) -> Result<Child> {
    ledger.state(
        (argv.len() + environment.len() + 2)
            .checked_mul(std::mem::size_of::<*const libc::c_char>())
            .ok_or("native selected argv state overflow")?,
        limits,
    )?;
    let mut args: Vec<_> = argv.iter().map(|v| v.as_ptr()).collect();
    args.push(ptr::null());
    let mut env: Vec<_> = environment.iter().map(|v| v.as_ptr()).collect();
    env.push(ptr::null());
    let parent = unsafe { libc::getpid() };
    let image_fd = image.as_raw_fd();
    let out = out.as_raw_fd();
    let err = err.as_raw_fd();
    let null = null.as_raw_fd();
    // No allocation, locks, Rust runtime or user callback in the child. All
    // buffers/FDs are retained in the parent until exec or anchored cleanup.
    let mut blocked = unsafe { std::mem::zeroed::<libc::sigset_t>() };
    let mut previous = unsafe { std::mem::zeroed::<libc::sigset_t>() };
    unsafe {
        libc::sigemptyset(&mut blocked);
        libc::sigaddset(&mut blocked, libc::SIGINT);
        libc::sigaddset(&mut blocked, libc::SIGTERM);
    }
    if unsafe { libc::pthread_sigmask(libc::SIG_BLOCK, &blocked, &mut previous) } != 0 {
        return Err("native selected caller signal mask block failed".into());
    }
    let pid = unsafe { libc::fork() };
    if pid < 0 {
        unsafe {
            libc::pthread_sigmask(libc::SIG_SETMASK, &previous, ptr::null_mut());
        }
        return Err("native selected fork failed".into());
    }
    if pid == 0 {
        unsafe {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) != 0
                || libc::getppid() != parent
                || libc::setsid() < 0
            {
                libc::_exit(125);
            }
            // Reset termination dispositions in the child only before unmask.
            let mut disposition = std::mem::zeroed::<libc::sigaction>();
            disposition.sa_sigaction = libc::SIG_DFL;
            libc::sigemptyset(&mut disposition.sa_mask);
            if libc::sigaction(libc::SIGINT, &disposition, ptr::null_mut()) != 0
                || libc::sigaction(libc::SIGTERM, &disposition, ptr::null_mut()) != 0
            {
                libc::_exit(125);
            }
            let mut mask = std::mem::zeroed::<libc::sigset_t>();
            libc::sigemptyset(&mut mask);
            if libc::sigprocmask(libc::SIG_SETMASK, &mask, ptr::null_mut()) != 0 {
                libc::_exit(125);
            }
            if libc::dup2(null, 0) < 0 || libc::dup2(out, 1) < 0 || libc::dup2(err, 2) < 0 {
                libc::_exit(125);
            }
            // Only stdio and the pinned ELF may survive to exec. The selected
            // image is not reopened through a pathname, and other caller FDs
            // grant no capabilities to this child.
            if image_fd < 3 {
                libc::_exit(125);
            }
            if image_fd > 3
                && libc::syscall(libc::SYS_close_range, 3u32, (image_fd - 1) as u32, 0u32) != 0
            {
                libc::_exit(125);
            }
            if libc::syscall(libc::SYS_close_range, (image_fd + 1) as u32, u32::MAX, 0u32) != 0 {
                libc::_exit(125);
            }
            libc::execveat(
                image_fd,
                c"".as_ptr(),
                args.as_ptr(),
                env.as_ptr(),
                libc::AT_EMPTY_PATH,
            );
            libc::_exit(126);
        }
    }
    // Keep the spawned child owned even if restoring the caller mask fails;
    // observation will report the error and still run anchored cleanup.
    let startup_error =
        unsafe { libc::pthread_sigmask(libc::SIG_SETMASK, &previous, ptr::null_mut()) } != 0;
    Ok(Child {
        pid,
        released: false,
        startup_error,
    })
}
struct Streams {
    stdout: Vec<u8>,
    counts: [usize; 2],
    hashes: [Digest256Hasher; 2],
    eof: [bool; 2],
    files: Option<[File; 2]>,
}
fn observe(
    child: &Child,
    pipes: &mut [File; 2],
    streams: &mut Streams,
    deadline: Instant,
    cancel: &(dyn Fn() -> Result<()> + Sync),
    ledger: &mut Ledger,
    limits: &CaptureLimits,
) -> Result<()> {
    if child.startup_error {
        return Err("native selected caller signal mask restore failed".into());
    }
    let mut raw = [0u8; BUFFER_BYTES];
    while !streams.eof.iter().all(|e| *e) {
        active(deadline, cancel)?;
        let mut polls = [
            libc::pollfd {
                fd: if streams.eof[0] {
                    -1
                } else {
                    pipes[0].as_raw_fd()
                },
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: if streams.eof[1] {
                    -1
                } else {
                    pipes[1].as_raw_fd()
                },
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        let status = unsafe { libc::poll(polls.as_mut_ptr(), 2, 20) };
        if status < 0 {
            if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err("native selected stream poll".into());
        }
        for index in 0..2 {
            if streams.eof[index] || polls[index].revents == 0 {
                continue;
            }
            let remaining = (limits.stream_bytes + 1)
                .checked_sub(streams.counts[index])
                .ok_or("native selected stream budget")?;
            let n = remaining.min(raw.len());
            ledger.io(n as u64, limits)?;
            let n = match pipes[index].read(&mut raw[..n]) {
                Ok(n) => n,
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                    ) =>
                {
                    continue;
                }
                Err(_) => return Err("native selected stream read"),
            };
            if n == 0 {
                streams.eof[index] = true;
                continue;
            }
            streams.counts[index] += n;
            streams.hashes[index].update(&raw[..n]);
            write_bounded(
                &mut streams
                    .files
                    .as_mut()
                    .ok_or("native selected stream guards absent")?[index],
                &raw[..n],
                deadline,
                cancel,
                ledger,
                limits,
            )?;
            if index == 0 {
                streams.stdout.extend_from_slice(&raw[..n]);
            }
            if streams.counts[index] > limits.stream_bytes {
                return Err("native selected stream overflow".into());
            }
        }
    }
    loop {
        active(deadline, cancel)?;
        if let Some(status) = child.terminal(deadline)? {
            if status != 0 {
                return Err(format!("native selected child status {status}"));
            }
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Caller supplies selected software and explicit environment, never a prefix
/// fallback. Work/cleanup deadlines are original absolute instants; cleanup
/// continues after cancellation and cannot renew either clock. When custody is
/// unproven, failure returns the retained namespace for the outer supervisor.
pub(crate) fn capture_selected_native<'a>(
    raw_ordered_request: &[u8],
    views: &'a [SelectedView<'a>],
    image: &'a mut VerifiedInstalledAccess,
    installed_budget: &mut InstalledAccessBudget,
    scratch: &Path,
    limits: CaptureLimits,
    work_deadline: Instant,
    cleanup_deadline: Instant,
    cancel: &(dyn Fn() -> Result<()> + Sync),
    environment: &[CString],
) -> std::result::Result<JsonValue, CaptureFailure<'a>> {
    let mut ledger = Ledger {
        state: 0,
        io: 0,
        metadata: 0,
    };
    let mut scope = None;
    let mut created_directory = None;
    let mut request_facts = JsonValue::Null;
    let mut stream_facts = JsonValue::Null;
    let mut leaves = Vec::new();
    let mut child = None;
    let mut child_released = true;
    let mut phase = "admission";
    let mut held = Vec::new();
    let mut outcome = (|| -> Result<JsonValue> {
        active(work_deadline, cancel)?;
        if image.role() != SelectedRole::Access {
            return Err("native selected capture requires the installed Access role".into());
        }
        if cleanup_deadline <= work_deadline
            || limits.frame_bytes == 0
            || limits.schema_bytes == 0
            || limits.stream_bytes == 0
            || limits.stream_bytes == usize::MAX
            || limits.metadata_bytes == 0
            || limits.request_state_bytes == 0
            || limits.result_state_bytes == 0
            || limits.census_entries == 0
            || raw_ordered_request.len() > REQUEST_BYTES
            || views.is_empty()
            || views.len() > 3
        {
            return Err("native selected finite explicit limits required".into());
        }
        let mut disposition = unsafe { std::mem::zeroed::<libc::sigaction>() };
        if unsafe { libc::sigaction(libc::SIGCHLD, ptr::null(), &mut disposition) } != 0
            || disposition.sa_sigaction != libc::SIG_DFL
            || disposition.sa_flags & libc::SA_NOCLDWAIT != 0
        {
            return Err(
                "native selected requires exclusively owned default SIGCHLD custody".into(),
            );
        }
        for entry in environment {
            if !matches!(
                entry.to_bytes(),
                b"LANG=C" | b"LANG=C.UTF-8" | b"LC_ALL=C" | b"LC_ALL=C.UTF-8" | b"TZ=UTC"
            ) {
                return Err(
                    "native selected environment permits only explicit C locale/UTC".into(),
                );
            }
        }
        if environment.len() > 3 {
            return Err("native selected environment count bound".into());
        }
        let fd_peak = installed_budget
            .held_fds()
            .checked_add(views.len() + 16)
            .ok_or("native selected FD count overflow")?;
        if fd_peak > limits.held_fds {
            return Err("native selected FD budget".into());
        }
        ledger.state(
            limits
                .request_state_bytes
                .checked_add(limits.result_state_bytes)
                .and_then(|v| v.checked_add(REQUEST_BYTES))
                .and_then(|v| v.checked_add(limits.stream_bytes + 1))
                .and_then(|v| v.checked_add(2 * BUFFER_BYTES + 32768))
                .ok_or("native selected state reservation overflow")?,
            &limits,
        )?;
        let mut request = parse_json_with_state_budget(
            raw_ordered_request,
            JsonMode::PublishedStrict,
            json_limits(REQUEST_BYTES),
            limits.request_state_bytes,
        )
        .map_err(|_| "native selected request JSON")?
        .into_root();
        if member(&request, "schema")?.as_str() != Some("tos_edge_offline_capture_request_v1") {
            return Err("native selected requires original v1 borrowed request".into());
        }
        if request.as_object().is_none() {
            return Err("native selected request object required".into());
        }
        let operation = member(&request, "operation")?
            .as_str()
            .ok_or("native selected operation string")?
            .to_owned();
        if !matches!(
            operation.as_str(),
            "prepared-delta"
                | "prepared-catchup"
                | "source-navigation-bootstrap"
                | "source-navigation-delta"
                | "source-navigation-integrity"
        ) {
            return Err("native selected unsupported operation".into());
        }
        let order = [
            "d1_database",
            "before_prepared_database",
            "after_prepared_database",
        ];
        let mut previous = None;
        for view in views {
            let index = order
                .iter()
                .position(|name| *name == view.input_field)
                .ok_or("native selected view field")?;
            if previous.is_some_and(|n| n >= index) || view.connection.is_autocommit() {
                return Err("native selected ordered held views required".into());
            }
            previous = Some(index);
        }
        for field in order {
            let selected = request.object_get(field).is_some_and(|v| !v.is_null());
            if selected != views.iter().any(|v| v.input_field == field) {
                return Err("native selected request/views differ".into());
            }
        }
        image.verify_current(
            cleanup_deadline,
            &mut || active(work_deadline, cancel),
            installed_budget,
        )?;
        scope = Some(Scope::new(scratch, &mut created_directory)?);
        let owned = scope.as_ref().ok_or("native selected scope absent")?;
        phase = "frames";
        ledger.io(limits.frame_bytes, &limits)?;
        let mut frame_budget =
            typed_snapshot::EncodeBudget::new(limits.frame_bytes, limits.schema_bytes)?;
        let mut inventories = Vec::new();
        for (index, view) in views.iter().enumerate() {
            active(work_deadline, cancel)?;
            let leaf = format!("snapshot-{index}.lsnap");
            let path = owned.path.join(&leaf);
            let mut file = create_at(&owned.directory, &leaf)?;
            leaves.push(leaf);
            let role = if view.input_field == "d1_database" {
                typed_snapshot::Role::D1
            } else {
                typed_snapshot::Role::Prepared
            };
            let interrupt = view.connection.get_interrupt_handle();
            let (finish, stopped) = std::sync::mpsc::channel();
            let inventory = std::thread::scope(|threads| {
                threads.spawn(move || {
                    loop {
                        let remaining = work_deadline.saturating_duration_since(Instant::now());
                        if remaining.is_zero() || cancel().is_err() {
                            interrupt.interrupt();
                            break;
                        }
                        match stopped.recv_timeout(remaining.min(Duration::from_millis(20))) {
                            Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                        }
                    }
                });
                let result = typed_snapshot::encode_borrowed(
                    view.connection,
                    role,
                    view.input_field,
                    &mut file,
                    &mut frame_budget,
                    work_deadline,
                    cancel,
                );
                let _ = finish.send(());
                result
            })?;
            // Producer inventory is bounded to a finite frame; reserve its JSON
            // representation before converting from serde to ordered Foundation.
            ledger.state(40960, &limits)?;
            let mut raw = Vec::with_capacity(8192);
            serde_json::to_writer(
                LimitedWriter {
                    raw: &mut raw,
                    cap: 8192,
                },
                &inventory,
            )
            .map_err(|_| "native selected inventory serialization bound")?;
            if raw.len() > 8192 {
                return Err("native selected inventory state bound".into());
            }
            inventories.push(
                parse_json_with_state_budget(
                    &raw,
                    JsonMode::PublishedStrict,
                    json_limits(8192),
                    32768,
                )
                .map_err(|_| "native selected inventory JSON")?
                .into_root(),
            );
            patch(
                &mut request,
                view.input_field,
                text(path.to_str().ok_or("native selected frame path UTF-8")?),
            )?;
            let stamp = identity(&file)?;
            held.push(HeldFile { file, path, stamp });
        }
        let inventory = JsonValue::Object(vec![
            (
                JsonString::from_utf8("schema"),
                text("tos_edge_typed_snapshot_inventory_v1"),
            ),
            (
                JsonString::from_utf8("snapshots"),
                JsonValue::Array(inventories),
            ),
        ]);
        patch(
            &mut request,
            "schema",
            text("tos_edge_offline_capture_request_v2"),
        )?;
        patch(
            &mut request,
            "manifest_json",
            text(
                owned
                    .path
                    .join("manifest.json")
                    .to_str()
                    .ok_or("native selected manifest path UTF-8")?,
            ),
        )?;
        patch(
            &mut request,
            "snapshot_frame_max_bytes",
            number(limits.frame_bytes),
        )?;
        patch(
            &mut request,
            "snapshot_schema_max_allocation_bytes",
            number(limits.schema_bytes),
        )?;
        phase = "request";
        let raw = emit_value_preserved_json(&request, json_limits(REQUEST_BYTES))
            .map_err(|_| "native selected request emission")?;
        let mut request_file = create_at(&owned.directory, "request.json")?;
        leaves.push("request.json".into());
        write_bounded(
            &mut request_file,
            &raw,
            work_deadline,
            cancel,
            &mut ledger,
            &limits,
        )?;
        request_facts = JsonValue::Object(vec![
            (JsonString::from_utf8("file"), text("request.json")),
            (JsonString::from_utf8("bytes"), number(raw.len() as u64)),
            (
                JsonString::from_utf8("sha256"),
                text(&Digest256::of_bytes(&raw).to_hex()),
            ),
        ]);
        let request_path = owned.path.join("request.json");
        let stamp = identity(&request_file)?;
        held.push(HeldFile {
            file: request_file,
            path: request_path.clone(),
            stamp,
        });
        drop(raw);
        drop(request);
        let mut streams = Streams {
            stdout: Vec::new(),
            counts: [0; 2],
            hashes: [Digest256Hasher::new(), Digest256Hasher::new()],
            eof: [false; 2],
            files: Some([
                create_at(&owned.directory, "stdout.bin")?,
                create_at(&owned.directory, "stderr.bin")?,
            ]),
        };
        leaves.extend(["stdout.bin".into(), "stderr.bin".into()]);
        streams
            .stdout
            .try_reserve_exact(limits.stream_bytes + 1)
            .map_err(|_| "native selected stream allocation")?;
        let (out_read, out_write) = pipe()?;
        let (err_read, err_write) = pipe()?;
        nonblocking(&out_read)?;
        nonblocking(&err_read)?;
        let null = File::open("/dev/null").map_err(|_| "native selected null stdin")?;
        let argv = [
            CString::new("tos-access").unwrap(),
            CString::new("edge-offline-capture").unwrap(),
            CString::new("--expected-parent-pid").unwrap(),
            CString::new(unsafe { libc::getpid() }.to_string())
                .map_err(|_| "native selected parent argument")?,
            CString::new("--request").unwrap(),
            CString::new(
                request_path
                    .to_str()
                    .ok_or("native selected request path UTF-8")?,
            )
            .map_err(|_| "native selected request argument")?,
        ];
        phase = "child";
        owned.current()?;
        image.verify_current(
            cleanup_deadline,
            &mut || active(work_deadline, cancel),
            installed_budget,
        )?;
        child = Some(spawn(
            image.image(),
            &argv,
            environment,
            &out_write,
            &err_write,
            &null,
            &mut ledger,
            &limits,
        )?);
        child_released = false;
        drop(out_write);
        drop(err_write);
        let observation = observe(
            child.as_ref().ok_or("native selected child absent")?,
            &mut [out_read, err_read],
            &mut streams,
            work_deadline,
            cancel,
            &mut ledger,
            &limits,
        );
        stream_facts = JsonValue::Array(
            (0..2)
                .map(|index| {
                    JsonValue::Object(vec![
                        (
                            JsonString::from_utf8("file"),
                            text(if index == 0 {
                                "stdout.bin"
                            } else {
                                "stderr.bin"
                            }),
                        ),
                        (
                            JsonString::from_utf8("read_bytes"),
                            number(streams.counts[index] as u64),
                        ),
                        (
                            JsonString::from_utf8("read_sha256"),
                            text(&streams.hashes[index].clone().finalize().to_hex()),
                        ),
                        (
                            JsonString::from_utf8("eof"),
                            JsonValue::Bool(streams.eof[index]),
                        ),
                    ])
                })
                .collect(),
        );
        for (index, file) in streams
            .files
            .take()
            .ok_or("native selected stream guards absent")?
            .into_iter()
            .enumerate()
        {
            let path = owned.path.join(if index == 0 {
                "stdout.bin"
            } else {
                "stderr.bin"
            });
            let stamp = identity(&file)?;
            if stamp.2 != streams.counts[index] as u64 {
                return Err("native selected stream evidence length differs".into());
            }
            let selected = HeldFile { file, path, stamp };
            held.push(selected);
        }
        phase = "child-cleanup";
        let release = child
            .as_mut()
            .ok_or("native selected child absent")?
            .release(cleanup_deadline, &mut ledger, &limits);
        child_released = child.as_ref().is_some_and(|c| c.released);
        release?;
        observation?;
        phase = "currentness";
        active(work_deadline, cancel)?;
        owned.current()?;
        for frame in &held {
            frame.current()?;
        }
        for view in views {
            if view.connection.is_autocommit() {
                return Err("native selected caller released snapshot".into());
            }
        }
        image.verify_current(
            cleanup_deadline,
            &mut || active(work_deadline, cancel),
            installed_budget,
        )?;
        phase = "result";
        let result = parse_json_with_state_budget(
            &streams.stdout,
            JsonMode::PublishedStrict,
            json_limits(limits.stream_bytes),
            limits.result_state_bytes,
        )
        .map_err(|_| "native selected result JSON")?
        .into_root();
        if result.as_object().is_none_or(|fields| fields.len() != 3)
            || member(&result, "schema")?.as_str() != Some("tos_edge_offline_capture_result_v2")
            || member(&result, "operation")?.as_str() != Some(operation.as_str())
            || member(member(&result, "receipt")?, "snapshot_transport")? != &inventory
        {
            return Err("native selected receipt snapshot binding differs".into());
        }
        active(work_deadline, cancel)?;
        Ok(result)
    })();
    // Finally-style anchored cleanup also covers unexpected failures between
    // spawn and the explicit observation cleanup phase. Neither deadline nor
    // IO/census budget is renewed for this attempt.
    if let Some(owned_child) = child.as_mut() {
        if !owned_child.released {
            if let Err(reason) = owned_child.release(cleanup_deadline, &mut ledger, &limits) {
                if outcome.is_ok() {
                    outcome = Err(reason);
                }
            }
        }
        child_released = owned_child.released;
    }
    if let Some(owned) = scope.as_ref() {
        if outcome.is_ok() && child_released {
            // Native manifest is its own bounded output; it remains a failure
            // artifact until transport success. It is removed only if present.
            if tos_fd_open::open_regular_at(&owned.directory, Path::new("manifest.json")).is_ok() {
                leaves.push("manifest.json".into());
            }
            if let Err(reason) = owned.remove(&leaves, cleanup_deadline) {
                return Err(CaptureFailure {
                    reason,
                    retained_directory: Some(owned.path.clone()),
                    child_released,
                    metadata_complete: false,
                    selected_views: views,
                    selected_image: image,
                    custody: Some(CaptureCustody {
                        child: child.take(),
                        scope: scope.take(),
                        frames: held,
                    }),
                });
            }
        } else {
            let reason = outcome
                .as_ref()
                .err()
                .cloned()
                .unwrap_or_else(|| "native selected custody unresolved".into());
            let value = JsonValue::Object(vec![
                (
                    JsonString::from_utf8("schema"),
                    text("tos_edge_private_capture_failure_v1"),
                ),
                (JsonString::from_utf8("phase"), text(phase)),
                (JsonString::from_utf8("request"), request_facts),
                (JsonString::from_utf8("streams"), stream_facts),
                (
                    JsonString::from_utf8("child_pid"),
                    child
                        .as_ref()
                        .map_or(JsonValue::Null, |c| number(c.pid as u64)),
                ),
                (
                    JsonString::from_utf8("child_released"),
                    JsonValue::Bool(child_released),
                ),
            ]);
            let metadata_complete = (|| -> Result<()> {
                ledger.state(EVIDENCE_BYTES * 4, &limits)?;
                let raw = emit_value_preserved_json(&value, json_limits(EVIDENCE_BYTES))
                    .map_err(|_| "native selected failure metadata JSON")?;
                ledger.metadata(raw.len(), &limits)?;
                let mut file = create_at(&owned.directory, "capture-failure.json")?;
                write_bounded(
                    &mut file,
                    &raw,
                    cleanup_deadline,
                    &|| Ok(()),
                    &mut ledger,
                    &limits,
                )
            })()
            .is_ok();
            return Err(CaptureFailure {
                reason,
                retained_directory: Some(owned.path.clone()),
                child_released,
                metadata_complete,
                selected_views: views,
                selected_image: image,
                custody: Some(CaptureCustody {
                    child: child.take(),
                    scope: scope.take(),
                    frames: held,
                }),
            });
        }
    }
    outcome.map_err(|reason| CaptureFailure {
        reason,
        retained_directory: created_directory,
        child_released,
        metadata_complete: false,
        selected_views: views,
        selected_image: image,
        custody: Some(CaptureCustody {
            child: child.take(),
            scope: scope.take(),
            frames: held,
        }),
    })
}
