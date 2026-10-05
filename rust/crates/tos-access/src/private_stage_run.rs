//! Generic Linux private tmpfs stage controller; no model or resource admission.
//! Caller owns an exclusively delegated empty consumer cgroup and capped output.
//! Source port only: actual unprivileged namespace/cgroup placement is not proved.
use std::{
    ffi::{CString, OsStr},
    fs::{self, DirBuilder, File, OpenOptions},
    io::{Read, Write},
    os::{
        fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd},
        unix::{
            ffi::OsStrExt,
            fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
            process::CommandExt,
        },
    },
    path::{Path, PathBuf},
    process::{Child, Command},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::Duration,
};
const PATH_BYTES: usize = 4096;
const PATH_PARTS: usize = 128;
const ARGC: usize = 256;
const ARGV_BYTES: usize = 128 * 1024;
const FALLBACKS: [&str; 3] = ["/var/tmp", "/usr/tmp", "/tmp"];
const SCHEMA: &str = "abyss_machine_private_tmpfs_stage_v1";
static CANCELLED: AtomicBool = AtomicBool::new(false);
extern "C" fn cancel(_: i32) {
    CANCELLED.store(true, Ordering::Relaxed);
}
fn error() -> String {
    std::io::Error::last_os_error().to_string()
}
fn clock_ns() -> Result<u64, String> {
    let mut t = unsafe { std::mem::zeroed::<libc::timespec>() };
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut t) } != 0 {
        return Err(error());
    }
    u64::try_from(t.tv_sec)
        .ok()
        .and_then(|n| n.checked_mul(1_000_000_000))
        .and_then(|n| n.checked_add(t.tv_nsec as u64))
        .ok_or_else(|| "monotonic clock overflow".into())
}
#[derive(Clone, Copy)]
struct Cutoff {
    whole: u64,
    work: u64,
}
impl Cutoff {
    fn select(original: u64, shutdown_ms: u64) -> Result<Self, String> {
        let reserve = shutdown_ms
            .checked_mul(1_000_000)
            .filter(|n| *n > 0)
            .ok_or("positive finite cleanup reserve required")?;
        let work = original
            .checked_sub(reserve)
            .ok_or("cleanup reserve exceeds original cutoff")?;
        if work <= clock_ns()? {
            return Err("original work cutoff already expired".into());
        }
        Ok(Self {
            whole: original,
            work,
        })
    }
    fn check(&self) -> Result<(), String> {
        if CANCELLED.load(Ordering::Relaxed) || clock_ns()? >= self.work {
            return Err("stage cancelled or original work cutoff expired".into());
        }
        Ok(())
    }
    fn cleanup_check(&self) -> Result<(), String> {
        if clock_ns()? >= self.whole {
            return Err(
                "original cleanup cutoff expired; outer containment cleanup required".into(),
            );
        }
        Ok(())
    }
}
fn cstring(value: &OsStr) -> Result<CString, String> {
    CString::new(value.as_bytes()).map_err(|e| e.to_string())
}
fn path_shape(path: &Path) -> Result<(), String> {
    let bytes = path.as_os_str().as_bytes();
    if bytes.is_empty()
        || bytes.len() > PATH_BYTES
        || !path.is_absolute()
        || bytes
            .iter()
            .any(|b| b.is_ascii_whitespace() || *b == b'\\' || *b == 0)
    {
        return Err("bounded canonical absolute path required".into());
    }
    let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
    let parts: Vec<&str> = text.split('/').skip(1).collect();
    if parts.len() > PATH_PARTS
        || parts
            .iter()
            .any(|p| p.is_empty() || *p == "." || *p == "..")
    {
        return Err("bounded canonical path components required".into());
    }
    Ok(())
}
fn directory(path: &Path) -> Result<File, String> {
    path_shape(path)?;
    let mut held = unsafe {
        libc::open(
            c"/".as_ptr(),
            libc::O_PATH | libc::O_DIRECTORY | libc::O_CLOEXEC,
        )
    };
    if held < 0 {
        return Err(error());
    }
    let mut file = unsafe { File::from_raw_fd(held) };
    for part in path.as_os_str().as_bytes().split(|b| *b == b'/').skip(1) {
        let name = CString::new(part).map_err(|e| e.to_string())?;
        held = unsafe {
            libc::openat(
                file.as_raw_fd(),
                name.as_ptr(),
                libc::O_PATH | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if held < 0 {
            return Err(error());
        }
        file = unsafe { File::from_raw_fd(held) }
    }
    Ok(file)
}
fn member(root: &File, name: &str, write: bool) -> Result<File, String> {
    let name = CString::new(name).map_err(|e| e.to_string())?;
    let flags = (if write {
        libc::O_WRONLY
    } else {
        libc::O_RDONLY
    }) | libc::O_NOFOLLOW
        | libc::O_CLOEXEC
        | libc::O_NONBLOCK;
    let fd = unsafe { libc::openat(root.as_raw_fd(), name.as_ptr(), flags) };
    if fd < 0 {
        return Err(error());
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}
fn read_file(mut file: File, cap: usize, rows: usize, line: usize) -> Result<String, String> {
    let mut bytes = vec![0u8; cap.checked_add(1).ok_or("kernel read bound overflow")?];
    let mut used = 0;
    loop {
        let n = file.read(&mut bytes[used..]).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        used += n;
        if used > cap {
            return Err("kernel evidence byte bound exceeded".into());
        }
    }
    bytes.truncate(used);
    let text = String::from_utf8(bytes).map_err(|e| e.to_string())?;
    for (i, row) in text.lines().enumerate() {
        if i >= rows || row.len() > line {
            return Err("kernel evidence row/line bound exceeded".into());
        }
    }
    Ok(text)
}
fn kernel(path: &Path, cap: usize, rows: usize, line: usize) -> Result<String, String> {
    read_file(
        OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)
            .map_err(|e| e.to_string())?,
        cap,
        rows,
        line,
    )
}
fn scalar(root: &File, name: &str) -> Result<u64, String> {
    let raw = read_file(member(root, name, false)?, 64, 1, 64)?;
    raw.trim()
        .parse()
        .map_err(|_| format!("finite actual cgroup scalar required: {name}"))
}
fn contents(root: &File, name: &str, cap: usize) -> Result<String, String> {
    read_file(member(root, name, false)?, cap, 256, 4096)
}
fn identical(a: &File, b: &File) -> Result<(), String> {
    let a = a.metadata().map_err(|e| e.to_string())?;
    let b = b.metadata().map_err(|e| e.to_string())?;
    if a.dev() != b.dev() || a.ino() != b.ino() {
        return Err("held named directory identity changed".into());
    }
    Ok(())
}
fn membership() -> Result<PathBuf, String> {
    let text = kernel(Path::new("/proc/self/cgroup"), 65536, 256, 4096)?;
    let matches: Vec<&str> = text.lines().filter_map(|l| l.strip_prefix("0::")).collect();
    if matches.len() != 1 {
        return Err("unique actual cgroup v2 membership required".into());
    }
    let path = Path::new("/sys/fs/cgroup").join(matches[0].trim_start_matches('/'));
    path_shape(&path)?;
    Ok(path)
}

// Persistent IO is outside private tmpfs quota and separately admitted/priced
// by the caller. Selecting a directory cannot acquire a resource grant.
fn verify_persistent(path: &Path, held: &File, stage: &Path) -> Result<std::fs::Metadata, String> {
    path_shape(path)?;
    for forbidden in std::iter::once(stage).chain(FALLBACKS.iter().map(Path::new)) {
        if path.starts_with(forbidden) || forbidden.starts_with(path) {
            return Err("persistent store overlaps private stage or fallback".into());
        }
    }
    let named = directory(path)?;
    identical(held, &named)?;
    let info = held.metadata().map_err(|e| e.to_string())?;
    if info.mode() & 0o077 != 0 || info.uid() != unsafe { libc::getuid() } {
        return Err("persistent store must be private and owned by actual caller UID".into());
    }
    Ok(info)
}

fn consumer_limits(held: &File, ram: u64) -> Result<(), String> {
    if scalar(held, "memory.max")? != ram || scalar(held, "memory.swap.max")? != 0 {
        return Err("consumer finite hard RAM/swap differs from caller".into());
    }
    Ok(())
}
fn topology(
    setup: &Path,
    consumer: &Path,
    held: &File,
    quota: u64,
    ram: u64,
    end: Cutoff,
) -> Result<(), String> {
    end.check()?;
    let common = setup.parent().ok_or("setup parent absent")?;
    if common == Path::new("/sys/fs/cgroup")
        || consumer == setup
        || consumer.parent() != Some(common)
    {
        return Err(
            "consumer must be distinct sibling of actual setup under dedicated parent".into(),
        );
    }
    let named = directory(consumer)?;
    identical(held, &named)?;
    consumer_limits(held, ram)?;
    let common_fd = directory(common)?;
    let setup_fd = directory(setup)?;
    let aggregate = quota.checked_add(ram).ok_or("additive RAM overflow")?;
    if scalar(&common_fd, "memory.max")? != aggregate
        || scalar(&common_fd, "memory.swap.max")? != 0
        || scalar(&setup_fd, "memory.max")? != quota
        || scalar(&setup_fd, "memory.swap.max")? != 0
    {
        return Err("actual additive setup/common RAM envelope differs".into());
    }
    if !contents(&common_fd, "cgroup.procs", 4096)?
        .trim()
        .is_empty()
        || !contents(&common_fd, "cgroup.subtree_control", 4096)?
            .split_whitespace()
            .any(|v| v == "memory")
    {
        return Err("aggregate parent must be empty and delegate memory controller".into());
    }
    let mut at = common.to_path_buf();
    for _ in 0..PATH_PARTS {
        end.check()?;
        if at == Path::new("/sys/fs/cgroup") {
            break;
        }
        let fd = directory(&at)?;
        let raw = contents(&fd, "memory.max", 64)?;
        if raw.trim() != "max" && raw.trim().parse::<u64>().map_err(|e| e.to_string())? < aggregate
        {
            return Err("ancestor actual hard RAM below additive caller envelope".into());
        }
        if !at.pop() {
            return Err("cgroup ancestry escaped".into());
        }
    }
    end.check()
}

// Supported pinned one-context search profile; generic SDK callers do not
// acquire a multiple-child allowance. G includes four roles plus one Python
// fork duplicate: 4*64MiB+128MiB=384MiB; Python128MiB+G=original512MiB.
const SDK_PHASE_N: u64 = 134_217_728;
const SDK_PHASE_G: u64 = 402_653_184;
const SDK_PHASE_ROLE: u64 = 67_108_864;
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SdkPhaseAs {
    setup_as_bytes: u64,
    guardian_state_bytes: u64,
}
impl SdkPhaseAs {
    fn verify(self) -> Result<(), String> {
        if self.setup_as_bytes != SDK_PHASE_N
            || self.guardian_state_bytes != SDK_PHASE_G
            || self.setup_as_bytes.checked_add(self.guardian_state_bytes) != Some(SDK_SETUP_BYTES)
            || SDK_PHASE_ROLE
                .checked_mul(4)
                .and_then(|n| n.checked_add(self.setup_as_bytes))
                != Some(self.guardian_state_bytes)
        {
            return Err("unsupported SDK phase AS partition".into());
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct IssuedSdkPhaseAs {
    selected: SdkPhaseAs,
    parent_setup_soft_as_bytes: u64,
    role_soft_as_bytes: u64,
}
fn phase_decimal(raw: &str) -> Result<u64, String> {
    if raw.is_empty() || raw.len() > 20 || !raw.bytes().all(|b| b.is_ascii_digit()) {
        return Err("bounded SDK decimal required".into());
    }
    let n = raw.parse::<u64>().map_err(|e| e.to_string())?;
    if n == 0 || n.to_string() != raw {
        return Err("canonical positive SDK decimal required".into());
    }
    Ok(n)
}
fn phase_pair(n: Option<&str>, g: Option<&str>) -> Result<Option<SdkPhaseAs>, String> {
    match (n, g) {
        (None, None) => Ok(None),
        (Some(n), Some(g)) => {
            let p = SdkPhaseAs {
                setup_as_bytes: phase_decimal(n)?,
                guardian_state_bytes: phase_decimal(g)?,
            };
            p.verify()?;
            Ok(Some(p))
        }
        _ => Err("both SDK phase AS selectors required".into()),
    }
}
fn actual_as() -> Result<libc::rlimit, String> {
    let mut v = unsafe { std::mem::zeroed::<libc::rlimit>() };
    if unsafe { libc::getrlimit(libc::RLIMIT_AS, &mut v) } != 0 {
        return Err(error());
    }
    Ok(v)
}
fn phase_environment(p: SdkPhaseAs, end: Cutoff) -> Result<(), String> {
    for (key, n) in [
        ("TOS_SDK_SETUP_AS_BYTES", p.setup_as_bytes),
        ("TOS_SDK_GUARDIAN_STATE_BYTES", p.guardian_state_bytes),
        ("TOS_SDK_ORIGINAL_WORK_DEADLINE_NS", end.work),
        ("TOS_SDK_ORIGINAL_WHOLE_DEADLINE_NS", end.whole),
    ] {
        let raw = std::env::var(key).map_err(|_| format!("SDK selector absent: {key}"))?;
        if phase_decimal(&raw)? != n {
            return Err("SDK original selector/cutoff mismatch".into());
        }
    }
    end.check()
}
// Genuine procfs VM ranges: bounded fixed stack, no heap/ELF/RSS inference.
fn phase_mapped_as(end: Cutoff) -> Result<u64, String> {
    end.check()?;
    let fd = unsafe {
        libc::open(
            c"/proc/self/maps".as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(error());
    }
    let mut held = unsafe { File::from_raw_fd(fd) };
    let mut fs = unsafe { std::mem::zeroed::<libc::statfs>() };
    if unsafe { libc::fstatfs(fd, &mut fs) } != 0 || fs.f_type as u64 != 0x9fa0 {
        return Err("actual procfs maps required".into());
    }
    let mut raw = [0u8; 65_537];
    let mut used = 0;
    loop {
        end.check()?;
        let n = held.read(&mut raw[used..]).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        used += n;
        if used > 65_536 {
            return Err("SDK maps64KiB bound".into());
        }
    }
    let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    if page <= 0 {
        return Err("actual page size required".into());
    }
    let text = std::str::from_utf8(&raw[..used]).map_err(|e| e.to_string())?;
    if text.is_empty() || !text.ends_with('\n') {
        return Err("complete mapping lines required".into());
    }
    let mut total = 0u64;
    let mut previous = 0;
    let mut rows = 0;
    for line in text.lines() {
        end.check()?;
        rows += 1;
        if rows > 4096 || line.len() > 8192 {
            return Err("SDK mapping rows/line bound".into());
        }
        let range = line
            .split_ascii_whitespace()
            .next()
            .ok_or("mapping range absent")?;
        let (a, b) = range.split_once('-').ok_or("mapping range shape")?;
        if [a, b]
            .iter()
            .any(|v| v.is_empty() || v.len() > 16 || !v.bytes().all(|c| c.is_ascii_hexdigit()))
        {
            return Err("mapping address shape".into());
        }
        let start = u64::from_str_radix(a, 16).map_err(|e| e.to_string())?;
        let stop = u64::from_str_radix(b, 16).map_err(|e| e.to_string())?;
        if start < previous || start >= stop || start % page as u64 != 0 || stop % page as u64 != 0
        {
            return Err("ordered page-aligned mapping ranges required".into());
        }
        total = total
            .checked_add(stop - start)
            .ok_or("mapping sum overflow")?;
        previous = stop;
    }
    end.check()?;
    Ok(total)
}
fn guardian_as(ceiling: u64, end: Cutoff) -> Result<(), String> {
    end.check()?;
    if ceiling == 0 || ceiling > SDK_PHASE_ROLE || phase_mapped_as(end)? > ceiling {
        return Err("SDK actual guardian mappings exceed role ceiling".into());
    }
    let old = actual_as()?;
    if old.rlim_max != SDK_CONSUMER_BYTES as libc::rlim_t || old.rlim_cur < ceiling as libc::rlim_t
    {
        return Err("SDK guardian hard drift or setup soft increase refused".into());
    }
    let next = libc::rlimit {
        rlim_cur: ceiling as libc::rlim_t,
        rlim_max: old.rlim_max,
    };
    if unsafe { libc::setrlimit(libc::RLIMIT_AS, &next) } != 0 {
        return Err(error());
    }
    let actual = actual_as()?;
    if actual.rlim_cur != next.rlim_cur
        || actual.rlim_max != next.rlim_max
        || phase_mapped_as(end)? > ceiling
    {
        return Err("actual guardian AS drift".into());
    }
    end.check()
}
fn issued_phase_as(
    selected: Option<SdkPhaseAs>,
    end: Cutoff,
) -> Result<Option<IssuedSdkPhaseAs>, String> {
    selected
        .map(|p| {
            p.verify()?;
            phase_environment(p, end)?;
            let actual = actual_as()?;
            if actual.rlim_max != SDK_CONSUMER_BYTES as libc::rlim_t
                || actual.rlim_cur == 0
                || actual.rlim_cur > p.setup_as_bytes as libc::rlim_t
            {
                return Err("SDK positive actualS<=issuedN/original hard2.5GiB required".into());
            }
            let s = actual.rlim_cur as u64;
            Ok(IssuedSdkPhaseAs {
                selected: p,
                parent_setup_soft_as_bytes: s,
                role_soft_as_bytes: s.min(SDK_PHASE_ROLE),
            })
        })
        .transpose()
}
fn verify_phase_auth(auth: &ControlAuth) -> Result<(), String> {
    if let Some(p) = auth.sdk_phase_as {
        p.selected.verify()?;
        if p.parent_setup_soft_as_bytes == 0
            || p.parent_setup_soft_as_bytes > p.selected.setup_as_bytes
            || p.role_soft_as_bytes != p.parent_setup_soft_as_bytes.min(SDK_PHASE_ROLE)
        {
            return Err("issued SDK N/G/S/role mismatch".into());
        }
    }
    Ok(())
}
fn phase_restore_consumer(
    o: &Options,
    auth: &ControlAuth,
    fd: i32,
    end: Cutoff,
) -> Result<(), String> {
    if let Some(selected) = o.sdk_phase_as {
        match_control(fd, auth, end)?;
        let p = auth.sdk_phase_as.ok_or("issued phase auth absent")?;
        if p.selected != selected {
            return Err("inner phase selector/auth mismatch".into());
        }
        phase_environment(selected, end)?;
        let old = actual_as()?;
        if old.rlim_cur != p.role_soft_as_bytes as libc::rlim_t
            || old.rlim_max != SDK_CONSUMER_BYTES as libc::rlim_t
        {
            return Err("inherited phase soft/hard differs before consumer restore".into());
        }
        // Only after actual self placement+consumer_limits+connected issued auth.
        let next = libc::rlimit {
            rlim_cur: old.rlim_max,
            rlim_max: old.rlim_max,
        };
        if unsafe { libc::setrlimit(libc::RLIMIT_AS, &next) } != 0 {
            return Err(error());
        }
        let actual = actual_as()?;
        if actual.rlim_cur != next.rlim_cur || actual.rlim_max != next.rlim_max {
            return Err("consumer-only soft restore drift".into());
        }
        match_control(fd, auth, end)?;
    }
    Ok(())
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ControlAuth {
    schema: String,
    socket_dev: u64,
    socket_ino: u64,
    socket_cookie: u64,
    parent_pid: i32,
    parent_uid: u32,
    parent_gid: u32,
    original_whole_deadline_ns: u64,
    work_deadline_ns: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sdk_phase_as: Option<IssuedSdkPhaseAs>,
}
fn socket_option<T: Copy>(fd: i32, name: i32) -> Result<T, String> {
    let mut value = unsafe { std::mem::zeroed::<T>() };
    let mut len = std::mem::size_of::<T>() as libc::socklen_t;
    if unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_SOCKET,
            name,
            &mut value as *mut T as *mut libc::c_void,
            &mut len,
        )
    } != 0
        || len as usize != std::mem::size_of::<T>()
    {
        return Err("control socket option unavailable".into());
    }
    Ok(value)
}
fn socket_identity(fd: i32) -> Result<(u64, u64, u64), String> {
    if fd < 3 {
        return Err("control FD must be an explicit inherited descriptor >=3".into());
    }
    let mut st = unsafe { std::mem::zeroed::<libc::stat>() };
    if unsafe { libc::fstat(fd, &mut st) } != 0
        || st.st_mode & libc::S_IFMT != libc::S_IFSOCK
        || socket_option::<i32>(fd, libc::SO_DOMAIN)? != libc::AF_UNIX
        || socket_option::<i32>(fd, libc::SO_TYPE)? != libc::SOCK_SEQPACKET
    {
        return Err("connected seqpacket control socket required".into());
    }
    for peer in [false, true] {
        let mut addr = unsafe { std::mem::zeroed::<libc::sockaddr_storage>() };
        let mut len = std::mem::size_of::<libc::sockaddr_storage>() as libc::socklen_t;
        let result = unsafe {
            if peer {
                libc::getpeername(fd, &mut addr as *mut _ as *mut libc::sockaddr, &mut len)
            } else {
                libc::getsockname(fd, &mut addr as *mut _ as *mut libc::sockaddr, &mut len)
            }
        };
        if result != 0 || addr.ss_family as i32 != libc::AF_UNIX {
            return Err("connected AF_UNIX control channel required".into());
        }
    }
    let cookie = socket_option::<u64>(fd, libc::SO_COOKIE)?;
    if cookie == 0 {
        return Err("control socket kernel cookie absent".into());
    }
    Ok((st.st_dev, st.st_ino, cookie))
}
fn issue_control(
    fd: i32,
    end: Cutoff,
    selected: Option<SdkPhaseAs>,
) -> Result<ControlAuth, String> {
    end.check()?;
    let (socket_dev, socket_ino, socket_cookie) = socket_identity(fd)?;
    let peer = socket_option::<libc::ucred>(fd, libc::SO_PEERCRED)?;
    if peer.pid != unsafe { libc::getppid() }
        || peer.pid <= 0
        || peer.uid != unsafe { libc::geteuid() }
        || peer.gid != unsafe { libc::getegid() }
    {
        return Err(
            "control channel must connect directly to original same-identity SDK parent".into(),
        );
    }
    Ok(ControlAuth {
        schema: "tos_native_consumer_control_v1".into(),
        socket_dev,
        socket_ino,
        socket_cookie,
        parent_pid: peer.pid,
        parent_uid: peer.uid,
        parent_gid: peer.gid,
        original_whole_deadline_ns: end.whole,
        work_deadline_ns: end.work,
        sdk_phase_as: issued_phase_as(selected, end)?,
    })
}
fn match_control(fd: i32, auth: &ControlAuth, end: Cutoff) -> Result<(), String> {
    end.check()?;
    verify_phase_auth(auth)?;
    if auth.schema != "tos_native_consumer_control_v1"
        || auth.parent_pid <= 0
        || auth.original_whole_deadline_ns != end.whole
        || auth.work_deadline_ns != end.work
        || socket_identity(fd)? != (auth.socket_dev, auth.socket_ino, auth.socket_cookie)
    {
        return Err("issued control socket identity or original cutoff changed".into());
    }
    // Host peer PID/UID can translate in the child namespace. Stable kernel
    // socket identity is rechecked; the authenticated original credentials are
    // retained as provenance, never compared to namespace-translated integers.
    Ok(())
}
/// Held control transport; genuine stage/model admission is separately required.
pub struct IssuedConsumerControl {
    held: File,
    auth: ControlAuth,
}
impl IssuedConsumerControl {
    /// Logical retained transport state only; kernel socket charge remains
    /// inside the real caller cgroup and the FD census is separately admitted.
    pub fn retained_state_upper_bound(&self) -> Result<usize, String> {
        self.verify_current()?;
        std::mem::size_of::<Self>()
            .checked_add(self.auth.schema.capacity())
            .ok_or_else(|| "held control state byte census overflow".into())
    }
    pub fn as_fd(&self) -> BorrowedFd<'_> {
        self.held.as_fd()
    }
    pub fn verify_current(&self) -> Result<(), String> {
        match_control(
            self.held.as_raw_fd(),
            &self.auth,
            Cutoff {
                whole: self.auth.original_whole_deadline_ns,
                work: self.auth.work_deadline_ns,
            },
        )
    }
}
/// Validate issuer metadata and hold only the authenticated transport socket.
/// No JSON constructor creates a grant; callers must bind genuine stage first.
pub fn verify_issued_consumer_control(
    fd: i32,
    original: u64,
    work: u64,
) -> Result<IssuedConsumerControl, String> {
    let selected =
        std::env::var("ABYSS_CONSUMER_CONTROL_FD").map_err(|_| "issued control FD absent")?;
    if selected.parse::<i32>().map_err(|e| e.to_string())? != fd {
        return Err("native control CLI/env FD mismatch".into());
    }
    let raw = std::env::var("ABYSS_CONSUMER_CONTROL_AUTH")
        .map_err(|_| "issued control metadata absent")?;
    if raw.len() > 4096 {
        return Err("control metadata exceeds finite cap".into());
    }
    let auth: ControlAuth = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    if auth.original_whole_deadline_ns != original
        || auth.work_deadline_ns != work
        || work >= original
    {
        return Err("control original cutoffs mismatch".into());
    }
    let end = Cutoff {
        whole: original,
        work,
    };
    match_control(fd, &auth, end)?;
    for (path, id) in [
        ("/proc/self/uid_map", auth.parent_uid),
        ("/proc/self/gid_map", auth.parent_gid),
    ] {
        let raw = kernel(Path::new(path), 4096, 16, 1024)?;
        if raw.split_whitespace().collect::<Vec<_>>() != ["0", &id.to_string(), "1"] {
            return Err("control original identity/user namespace mapping mismatch".into());
        }
    }
    let duplicate = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 3) };
    if duplicate < 0 {
        return Err(error());
    }
    let result = IssuedConsumerControl {
        held: unsafe { File::from_raw_fd(duplicate) },
        auth,
    };
    result.verify_current()?;
    Ok(result)
}

#[derive(Clone)]
struct Options {
    unshare: PathBuf,
    consumer: PathBuf,
    scratch: PathBuf,
    quota: u64,
    inodes: u64,
    ram: u64,
    original: u64,
    shutdown_ms: u64,
    persistent: Option<PathBuf>,
    control_fd: Option<i32>,
    sdk_phase_as: Option<SdkPhaseAs>,
    direct_custody: Option<(libc::pid_t, libc::sigset_t)>,
    command: Vec<String>,
}
fn limits(o: &Options) -> Result<(), String> {
    if let Some(p) = o.sdk_phase_as {
        p.verify()?;
        if o.quota != SDK_SETUP_BYTES || o.ram != SDK_CONSUMER_BYTES {
            return Err("SDK phase exact setup/consumer profile required".into());
        }
    }
    if !matches!(std::env::consts::ARCH, "x86_64" | "aarch64") {
        return Err("supported Linux syscall source architectures are x86_64/aarch64".into());
    }
    let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    if page <= 0
        || o.quota < page as u64
        || o.quota % (page as u64) != 0
        || o.inodes < 16
        || o.ram == 0
    {
        return Err(
            "page aligned positive quota, inodes>=16 and positive working RAM required".into(),
        );
    }
    o.quota.checked_add(o.ram).ok_or("additive RAM overflow")?;
    if o.command.is_empty()
        || o.command.len() > ARGC
        || o.command.iter().any(|v| v.len() > PATH_BYTES)
        || o.command
            .iter()
            .try_fold(0usize, |n, v| n.checked_add(v.len() + 1))
            .filter(|n| *n <= ARGV_BYTES)
            .is_none()
    {
        return Err("command argv transport exceeds finite source limits".into());
    }
    path_shape(Path::new(&o.command[0]))?;
    if !Path::new(&o.command[0]).is_file() {
        return Err("explicit consumer executable absent".into());
    }
    Ok(())
}
struct SignalGuard {
    previous: [libc::sigaction; 2],
    installed: usize,
}
impl SignalGuard {
    fn install() -> Result<Self, String> {
        CANCELLED.store(false, Ordering::Relaxed);
        let mut guard = Self {
            previous: unsafe { std::mem::zeroed() },
            installed: 0,
        };
        let mut action = unsafe { std::mem::zeroed::<libc::sigaction>() };
        action.sa_sigaction = cancel as *const () as usize;
        unsafe { libc::sigemptyset(&mut action.sa_mask) };
        for (i, sig) in [libc::SIGINT, libc::SIGTERM].iter().enumerate() {
            if unsafe { libc::sigaction(*sig, &action, &mut guard.previous[i]) } != 0 {
                return Err(error());
            }
            guard.installed += 1
        }
        Ok(guard)
    }
}
impl Drop for SignalGuard {
    fn drop(&mut self) {
        for (i, sig) in [libc::SIGINT, libc::SIGTERM]
            .iter()
            .enumerate()
            .take(self.installed)
        {
            unsafe { libc::sigaction(*sig, &self.previous[i], std::ptr::null_mut()) };
        }
    }
}
struct Mask {
    old: libc::sigset_t,
    active: bool,
}
impl Mask {
    fn block() -> Result<Self, String> {
        let mut set = unsafe { std::mem::zeroed() };
        let mut old = unsafe { std::mem::zeroed() };
        unsafe {
            libc::sigemptyset(&mut set);
            libc::sigaddset(&mut set, libc::SIGINT);
            libc::sigaddset(&mut set, libc::SIGTERM)
        };
        let r = unsafe { libc::pthread_sigmask(libc::SIG_BLOCK, &set, &mut old) };
        if r != 0 {
            return Err(std::io::Error::from_raw_os_error(r).to_string());
        }
        Ok(Self { old, active: true })
    }
    fn restore(&mut self) -> Result<(), String> {
        if self.active {
            let r = unsafe {
                libc::pthread_sigmask(libc::SIG_SETMASK, &self.old, std::ptr::null_mut())
            };
            if r != 0 {
                return Err(std::io::Error::from_raw_os_error(r).to_string());
            }
            self.active = false
        }
        Ok(())
    }
}
impl Drop for Mask {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}
struct Leader {
    child: Child,
    owned: bool,
    reaped: bool,
}
impl Leader {
    fn spawn(
        mut command: Command,
        held_fd: i32,
        persistent_fd: Option<i32>,
        control_fd: Option<i32>,
    ) -> Result<(Self, Option<String>), String> {
        let mut mask = Mask::block()?;
        let old = mask.old;
        unsafe {
            command.pre_exec(move || {
                if libc::setsid() < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                libc::signal(libc::SIGINT, libc::SIG_DFL);
                libc::signal(libc::SIGTERM, libc::SIG_DFL);
                if libc::syscall(libc::SYS_close_range, 3u32, u32::MAX, 4u32) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                for fd in [Some(held_fd), persistent_fd, control_fd]
                    .into_iter()
                    .flatten()
                    .filter(|fd| *fd >= 3)
                {
                    let flags = libc::fcntl(fd, libc::F_GETFD);
                    if flags < 0 || libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                }
                let r = libc::pthread_sigmask(libc::SIG_SETMASK, &old, std::ptr::null_mut());
                if r != 0 {
                    return Err(std::io::Error::from_raw_os_error(r));
                }
                Ok(())
            });
        }
        let result = command.spawn();
        let leader = match result {
            Ok(child) => Self {
                child,
                owned: true,
                reaped: false,
            },
            Err(e) => {
                let restored = mask.restore();
                return Err(format!("namespace spawn: {e}; mask restore: {restored:?}"));
            }
        };
        let restoration_error = mask.restore().err();
        Ok((leader, restoration_error))
    }
    fn pid(&self) -> i32 {
        self.child.id() as i32
    }
    fn exited(&mut self) -> Result<Option<i32>, String> {
        if !self.owned || self.reaped {
            return Err(
                "unshare leader ownership lost; external containment cleanup required".into(),
            );
        }
        let mut info = unsafe { std::mem::zeroed::<libc::siginfo_t>() };
        if unsafe {
            libc::waitid(
                libc::P_PID,
                self.pid() as libc::id_t,
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        } != 0
        {
            self.owned = false;
            return Err(error());
        }
        let pid = unsafe { info.si_pid() };
        if pid == 0 {
            return Ok(None);
        }
        if pid != self.pid() {
            self.owned = false;
            return Err("unexpected unshare child identity".into());
        }
        Ok(Some(if info.si_code == libc::CLD_EXITED {
            unsafe { info.si_status() }
        } else {
            128 + unsafe { info.si_status() }
        }))
    }
    fn signal(&mut self) -> Result<(), String> {
        self.exited()?;
        if unsafe { libc::kill(-self.pid(), libc::SIGKILL) } < 0
            && std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
        {
            return Err(error());
        }
        Ok(())
    }
    fn live_group(&self, end: Cutoff) -> Result<bool, String> {
        for (count, entry) in fs::read_dir("/proc")
            .map_err(|e| e.to_string())?
            .enumerate()
        {
            end.cleanup_check()?;
            if count >= 65536 {
                return Err("owned setup process scan exceeds 65536 rows".into());
            }
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.file_name().to_string_lossy().parse::<u32>().is_err() {
                continue;
            }
            let path = entry.path().join("stat");
            let text = match kernel(&path, 4096, 1, 4096) {
                Ok(text) => text,
                Err(e) => {
                    if !path.exists() {
                        continue;
                    }
                    return Err(e);
                }
            };
            let (_, tail) = text.rsplit_once(')').ok_or("process stat shape")?;
            let fields: Vec<&str> = tail.split_whitespace().collect();
            if fields.len() < 4 {
                return Err("process stat shape".into());
            }
            if fields[2].parse::<i32>().map_err(|e| e.to_string())? == self.pid()
                && fields[0] != "Z"
                && fields[0] != "X"
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
fn consumer_populated(held: &File) -> Result<bool, String> {
    let text = contents(held, "cgroup.events", 4096)?;
    match text
        .lines()
        .find_map(|line| line.strip_prefix("populated "))
    {
        Some("0") => Ok(false),
        Some("1") => Ok(true),
        _ => Err("actual consumer population evidence absent".into()),
    }
}
fn cleanup(leader: &mut Leader, held: &File, end: Cutoff) -> Result<(), String> {
    // Both scopes are signalled before waiting: setup descendants cannot survive
    // while a consumer drain consumes the remaining cutoff.
    let killed = member(held, "cgroup.kill", true)
        .and_then(|mut f| f.write_all(b"1\n").map_err(|e| e.to_string()));
    let group = leader.signal();
    if killed.is_err() || group.is_err() {
        return Err(format!(
            "consumer kill: {killed:?}; owned setup kill: {group:?}; external containment cleanup required"
        ));
    }
    loop {
        end.cleanup_check()?;
        if !consumer_populated(held)? && leader.exited()?.is_some() && !leader.live_group(end)? {
            leader.child.wait().map_err(|e| e.to_string())?;
            leader.reaped = true;
            leader.owned = false;
            return Ok(());
        }
        thread::sleep(Duration::from_millis(5));
    }
}
// The direct held-ELF SDK branch inherits its caller's temporarily blocked
// acquisition mask. Restore exactly the original mask and bind death to the
// explicit actual caller before installing stage handlers or spawning a child.
fn apply_direct_custody(parent: libc::pid_t, mask: &libc::sigset_t) -> Result<(), String> {
    if parent <= 0 || unsafe { libc::getppid() } != parent {
        return Err("direct stage original parent differs".into());
    }
    if unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) } != 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    if unsafe { libc::getppid() } != parent {
        return Err("direct stage original parent ended during death binding".into());
    }
    let mut death_signal: libc::c_int = 0;
    if unsafe {
        libc::prctl(
            libc::PR_GET_PDEATHSIG,
            &mut death_signal as *mut libc::c_int,
            0,
            0,
            0,
        )
    } != 0
        || death_signal != libc::SIGKILL
    {
        return Err("direct stage parent-death binding differs".into());
    }
    let result = unsafe { libc::pthread_sigmask(libc::SIG_SETMASK, mask, std::ptr::null_mut()) };
    if result != 0 {
        return Err(std::io::Error::from_raw_os_error(result).to_string());
    }
    let mut actual: libc::sigset_t = unsafe { std::mem::zeroed() };
    let result = unsafe { libc::pthread_sigmask(libc::SIG_SETMASK, std::ptr::null(), &mut actual) };
    if result != 0 {
        return Err(std::io::Error::from_raw_os_error(result).to_string());
    }
    for signal in 1..=64 {
        if unsafe { libc::sigismember(mask, signal) }
            != unsafe { libc::sigismember(&actual, signal) }
        {
            return Err("direct stage original signal mask differs".into());
        }
    }
    if unsafe { libc::getppid() } != parent {
        return Err("direct stage original parent ended during mask restoration".into());
    }
    Ok(())
}

fn outer(o: &Options) -> Result<i32, String> {
    if let Some((parent, mask)) = o.direct_custody.as_ref() {
        apply_direct_custody(*parent, mask)?;
    }
    limits(o)?;
    let end = Cutoff::select(o.original, o.shutdown_ms)?;
    if o.sdk_phase_as.is_some() && o.control_fd.is_none() {
        return Err("SDK phase direct parent control required".into());
    }
    let control = o
        .control_fd
        .map(|fd| issue_control(fd, end, o.sdk_phase_as))
        .transpose()?;
    if let Some(auth) = control.as_ref() {
        if let Some(p) = auth.sdk_phase_as {
            guardian_as(p.role_soft_as_bytes, end)?;
        }
    }
    let _signals = SignalGuard::install()?;
    if unsafe { libc::getuid() } == 0 {
        return Err("ordinary unprivileged caller required".into());
    }
    path_shape(&o.unshare)?;
    if !o.unshare.is_file() {
        return Err("explicit generic unshare executable absent".into());
    }
    let scratch = directory(&o.scratch)?;
    if scratch.metadata().map_err(|e| e.to_string())?.mode() & 0o077 != 0 {
        return Err("caller selected scratch parent must be private0700".into());
    }
    let held = directory(&o.consumer)?;
    let persistent = o
        .persistent
        .as_ref()
        .map(|path| directory(path))
        .transpose()?;
    let setup = membership()?;
    topology(&setup, &o.consumer, &held, o.quota, o.ram, end)?;
    if !contents(&held, "cgroup.procs", 4096)?.trim().is_empty() || consumer_populated(&held)? {
        return Err("delegated consumer must initially be empty".into());
    }
    drop(member(&held, "cgroup.kill", true)?); // fail before launch if cleanup not delegated
    let parent_mnt = fs::metadata("/proc/self/ns/mnt")
        .map_err(|e| e.to_string())?
        .ino();
    let parent_net = fs::metadata("/proc/self/ns/net")
        .map_err(|e| e.to_string())?
        .ino();
    let root = o.scratch.join(format!(
        "tos-private-stage-{}-{}",
        std::process::id(),
        clock_ns()?
    ));
    path_shape(&root)?;
    if FALLBACKS
        .iter()
        .any(|p| root.starts_with(p) || Path::new(p).starts_with(&root))
    {
        return Err("private stage backing root overlaps fallback bind path".into());
    }
    if let Some(held) = persistent.as_ref() {
        verify_persistent(
            o.persistent.as_ref().ok_or("persistent selection absent")?,
            held,
            &root,
        )?;
    }
    DirBuilder::new()
        .mode(0o700)
        .create(&root)
        .map_err(|e| e.to_string())?;
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut command = Command::new(&o.unshare);
    command
        .args([
            "--user",
            "--map-root-user",
            "--mount",
            "--net",
            "--pid",
            "--mount-proc",
            "--fork",
            "--kill-child=SIGKILL",
        ])
        .arg(executable)
        .arg("private-stage-run")
        .arg("--namespace-inner");
    command.args([
        "--consumer-cgroup",
        o.consumer.to_str().ok_or("consumer path UTF8")?,
        "--scratch-parent",
        o.scratch.to_str().ok_or("scratch path UTF8")?,
        "--unshare-exe",
        o.unshare.to_str().ok_or("unshare path UTF8")?,
    ]);
    for (key, value) in [
        ("--quota-bytes", o.quota),
        ("--inodes", o.inodes),
        ("--working-ram-bytes", o.ram),
        ("--work-deadline-ns", o.original),
        ("--maximum-shutdown-ms", o.shutdown_ms),
        ("--parent-mount-namespace", parent_mnt),
        ("--parent-net-namespace", parent_net),
        ("--host-uid", unsafe { libc::getuid() } as u64),
        ("--host-gid", unsafe { libc::getgid() } as u64),
        ("--consumer-fd", held.as_raw_fd() as u64),
    ] {
        command.arg(key).arg(value.to_string());
    }
    if let Some(held) = persistent.as_ref() {
        command
            .arg("--persistent-store")
            .arg(o.persistent.as_ref().ok_or("persistent selection absent")?)
            .arg("--persistent-fd")
            .arg(held.as_raw_fd().to_string());
    }
    if let (Some(fd), Some(auth)) = (o.control_fd, control.as_ref()) {
        match_control(fd, auth, end)?;
        command
            .arg("--consumer-control-fd")
            .arg(fd.to_string())
            .arg("--control-auth")
            .arg(serde_json::to_string(auth).map_err(|e| e.to_string())?);
    }
    if let Some(p) = o.sdk_phase_as {
        command
            .arg("--sdk-setup-as-bytes")
            .arg(p.setup_as_bytes.to_string())
            .arg("--sdk-guardian-state-bytes")
            .arg(p.guardian_state_bytes.to_string());
    }
    command
        .arg("--root")
        .arg(&root)
        .arg("--setup-cgroup")
        .arg(&setup)
        .arg("--")
        .args(&o.command);
    let spawned = Leader::spawn(
        command,
        held.as_raw_fd(),
        persistent.as_ref().map(AsRawFd::as_raw_fd),
        o.control_fd,
    );
    let (mut leader, restoration_error) = match spawned {
        Ok(leader) => leader,
        Err(e) => {
            let _ = fs::remove_dir(&root);
            return Err(e);
        }
    };
    let result = (|| {
        if let Some(e) = restoration_error {
            return Err(format!("parent signal mask restoration failed: {e}"));
        }
        loop {
            end.check()?;
            if let Some(code) = leader.exited()? {
                return Ok(code);
            }
            thread::sleep(Duration::from_millis(5));
        }
    })();
    let closed = cleanup(&mut leader, &held, end);
    if closed.is_err() {
        return Err(format!(
            "consumer result: {result:?}; cleanup: {closed:?}; private empty backing path retained {}",
            root.display()
        ));
    }
    fs::remove_dir(&root).map_err(|e| format!("backing directory cleanup failed: {e}"))?;
    end.cleanup_check()?;
    result
}
fn mount(
    source: Option<&OsStr>,
    target: &Path,
    kind: Option<&OsStr>,
    flags: libc::c_ulong,
    data: Option<&OsStr>,
) -> Result<(), String> {
    let source = source.map(cstring).transpose()?;
    let target = cstring(target.as_os_str())?;
    let kind = kind.map(cstring).transpose()?;
    let data = data.map(cstring).transpose()?;
    if unsafe {
        libc::mount(
            source.as_ref().map_or(std::ptr::null(), |v| v.as_ptr()),
            target.as_ptr(),
            kind.as_ref().map_or(std::ptr::null(), |v| v.as_ptr()),
            flags,
            data.as_ref()
                .map_or(std::ptr::null(), |v| v.as_ptr().cast()),
        )
    } != 0
    {
        return Err(error());
    }
    Ok(())
}
fn loopback() -> Result<(), String> {
    let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM | libc::SOCK_CLOEXEC, 0) };
    if fd < 0 {
        return Err(error());
    }
    let socket = unsafe { File::from_raw_fd(fd) };
    let mut request = [0u8; 40];
    request[..2].copy_from_slice(b"lo");
    if unsafe { libc::ioctl(socket.as_raw_fd(), 0x8913u64, request.as_mut_ptr()) } < 0 {
        return Err(error());
    }
    let flags = i16::from_ne_bytes([request[16], request[17]]) | 1;
    request[16..18].copy_from_slice(&flags.to_ne_bytes());
    if unsafe { libc::ioctl(socket.as_raw_fd(), 0x8914u64, request.as_mut_ptr()) } < 0 {
        return Err(error());
    }
    if unsafe { libc::ioctl(socket.as_raw_fd(), 0x8913u64, request.as_mut_ptr()) } < 0
        || i16::from_ne_bytes([request[16], request[17]]) & 1 == 0
    {
        return Err("isolated loopback did not become UP".into());
    }
    Ok(())
}
#[repr(C)]
struct Ruleset {
    handled: u64,
}
#[repr(C, packed)]
struct PathRule {
    allowed: u64,
    parent: i32,
}
fn confine(root: &Path, fallbacks: &[String], persistent: Option<&File>) -> Result<(), String> {
    if !matches!(std::env::consts::ARCH, "x86_64" | "aarch64") {
        return Err("Landlock syscall architecture unsupported".into());
    }
    let abi = unsafe { libc::syscall(444, 0, 0, 1) };
    if abi < 3 {
        return Err("Landlock ABI>=3 required".into());
    }
    let handled = (1u64 << 1) | (((1u64 << 15) - 1) & !((1u64 << 4) - 1));
    let attr = Ruleset { handled };
    let raw = unsafe { libc::syscall(444, &attr, std::mem::size_of::<Ruleset>(), 0) };
    if raw < 0 {
        return Err(error());
    }
    let rules = unsafe { File::from_raw_fd(raw as i32) };
    for path in std::iter::once(root).chain(fallbacks.iter().map(Path::new)) {
        let resolved = fs::canonicalize(path).map_err(|e| e.to_string())?;
        let parent = directory(&resolved)?;
        let rule = PathRule {
            allowed: handled,
            parent: parent.as_raw_fd(),
        };
        if unsafe { libc::syscall(445, rules.as_raw_fd(), 1, &rule, 0) } < 0 {
            return Err(error());
        }
    }
    if let Some(held) = persistent {
        let rule = PathRule {
            allowed: handled,
            parent: held.as_raw_fd(),
        };
        if unsafe { libc::syscall(445, rules.as_raw_fd(), 1, &rule, 0) } < 0 {
            return Err(error());
        }
    }
    if unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0
        || unsafe { libc::syscall(446, rules.as_raw_fd(), 0) } < 0
    {
        return Err(error());
    }
    Ok(())
}
fn drop_caps() -> Result<(), String> {
    let raw = kernel(Path::new("/proc/sys/kernel/cap_last_cap"), 16, 1, 16)?;
    let last = raw.trim().parse::<u32>().map_err(|e| e.to_string())?;
    if last > 63 {
        return Err("capability transport source bound63 exceeded".into());
    }
    for cap in 0..=last {
        if unsafe { libc::prctl(libc::PR_CAPBSET_DROP, cap, 0, 0, 0) } != 0 {
            return Err(error());
        }
    }
    if unsafe {
        libc::prctl(
            libc::PR_CAP_AMBIENT,
            libc::PR_CAP_AMBIENT_CLEAR_ALL,
            0,
            0,
            0,
        )
    } != 0
    {
        return Err(error());
    }
    #[repr(C)]
    struct Header {
        version: u32,
        pid: i32,
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Data {
        effective: u32,
        permitted: u32,
        inheritable: u32,
    }
    let header = Header {
        version: 0x20080522,
        pid: 0,
    };
    let data = [Data {
        effective: 0,
        permitted: 0,
        inheritable: 0,
    }; 2];
    if unsafe { libc::syscall(libc::SYS_capset, &header, &data) } < 0 {
        return Err(error());
    }
    Ok(())
}
struct Inner {
    root: PathBuf,
    setup: PathBuf,
    consumer_fd: i32,
    persistent_fd: Option<i32>,
    parent_mnt: u64,
    parent_net: u64,
    host_uid: u64,
    host_gid: u64,
    control_auth: Option<ControlAuth>,
}
fn inner(o: &Options, i: &Inner) -> Result<i32, String> {
    limits(o)?;
    let end = Cutoff::select(o.original, o.shutdown_ms)?;
    end.check()?;
    match (o.control_fd, i.control_auth.as_ref()) {
        (None, None) => (),
        (Some(fd), Some(auth)) if fd != i.consumer_fd && Some(fd) != i.persistent_fd => {
            match_control(fd, auth, end)?
        }
        _ => return Err("control descriptor/auth handoff mismatch".into()),
    }
    if i.control_auth
        .as_ref()
        .and_then(|a| a.sdk_phase_as.map(|p| p.selected))
        != o.sdk_phase_as
    {
        return Err("inner SDK phase selectors/auth mismatch".into());
    }
    if let Some(auth) = i.control_auth.as_ref() {
        if let Some(p) = auth.sdk_phase_as {
            phase_environment(p.selected, end)?;
            let actual = actual_as()?;
            if actual.rlim_cur != p.role_soft_as_bytes as libc::rlim_t
                || actual.rlim_max != SDK_CONSUMER_BYTES as libc::rlim_t
            {
                return Err("inner inherited phase AS drift".into());
            }
            guardian_as(p.role_soft_as_bytes, end)?;
        }
    }
    path_shape(&i.root)?;
    if i.root.parent() != Some(o.scratch.as_path()) || i.consumer_fd < 3 {
        return Err("inner selected backing root or held consumer FD invalid".into());
    }
    let mnt = fs::metadata("/proc/self/ns/mnt")
        .map_err(|e| e.to_string())?
        .ino();
    let net = fs::metadata("/proc/self/ns/net")
        .map_err(|e| e.to_string())?
        .ino();
    if mnt == i.parent_mnt || net == i.parent_net || i.host_uid == 0 {
        return Err("fresh ordinary user/mount/network namespaces required".into());
    }
    for (path, id) in [
        ("/proc/self/uid_map", i.host_uid),
        ("/proc/self/gid_map", i.host_gid),
    ] {
        let raw = kernel(Path::new(path), 4096, 16, 1024)?;
        if raw.split_whitespace().collect::<Vec<_>>() != ["0", &id.to_string(), "1"] {
            return Err("exact ordinary single UID/GID mapping required".into());
        }
    }
    let persistent = match (&o.persistent, i.persistent_fd) {
        (None, None) => None,
        (Some(path), Some(fd)) if fd >= 3 => {
            let duplicate = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 3) };
            if duplicate < 0 {
                return Err(error());
            }
            let held = unsafe { File::from_raw_fd(duplicate) };
            verify_persistent(path, &held, &i.root)?;
            Some(held)
        }
        _ => {
            return Err(
                "selected persistent store and actual held descriptor must correspond".into(),
            );
        }
    };
    let duplicate = unsafe { libc::fcntl(i.consumer_fd, libc::F_DUPFD_CLOEXEC, 3) };
    if duplicate < 0 {
        return Err(error());
    }
    let held = unsafe { File::from_raw_fd(duplicate) };
    let actual = membership()?;
    if actual != i.setup {
        return Err("actual preplacement setup membership differs".into());
    }
    topology(&actual, &o.consumer, &held, o.quota, o.ram, end)?;
    if consumer_populated(&held)? {
        return Err("consumer became populated before placement".into());
    }
    end.check()?;
    mount(
        None,
        Path::new("/"),
        None,
        libc::MS_REC | libc::MS_PRIVATE,
        None,
    )?;
    let data = format!("size={},nr_inodes={},mode=700", o.quota, o.inodes);
    mount(
        Some(OsStr::new(SCHEMA)),
        &i.root,
        Some(OsStr::new("tmpfs")),
        libc::MS_NOSUID | libc::MS_NODEV | libc::MS_NOEXEC,
        Some(OsStr::new(&data)),
    )?;
    for name in ["tmp", "capture", "stage", "output"] {
        end.check()?;
        DirBuilder::new()
            .mode(0o700)
            .create(i.root.join(name))
            .map_err(|e| e.to_string())?;
    }
    let temporary = i.root.join("tmp");
    let temp = fs::metadata(&temporary).map_err(|e| e.to_string())?;
    let mut fallbacks: Vec<String> = Vec::new();
    for name in FALLBACKS {
        end.check()?;
        let path = Path::new(name);
        let info = match fs::symlink_metadata(path) {
            Ok(info) => info,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.to_string()),
        };
        if info.file_type().is_symlink() {
            let named = fs::canonicalize(path).map_err(|e| e.to_string())?;
            if !fallbacks.iter().any(|p| Path::new(p) == named) {
                return Err("unsupported SQLite fallback alias".into());
            }
        } else {
            if !info.is_dir() {
                return Err("SQLite fallback is not a directory".into());
            }
            mount(Some(temporary.as_os_str()), path, None, libc::MS_BIND, None)?;
        }
        let info = fs::metadata(path).map_err(|e| e.to_string())?;
        if info.dev() != temp.dev() || info.ino() != temp.ino() {
            return Err("fallback does not share aggregate private quota".into());
        }
        fallbacks.push(name.into());
    }
    let root = directory(&i.root)?;
    let meta = root.metadata().map_err(|e| e.to_string())?;
    let mut stats = unsafe { std::mem::zeroed::<libc::statvfs>() };
    if unsafe { libc::fstatvfs(root.as_raw_fd(), &mut stats) } != 0
        || stats.f_blocks.checked_mul(stats.f_frsize) != Some(o.quota)
        || stats.f_files != o.inodes
    {
        return Err("actual kernel tmpfs byte/inode ceiling differs".into());
    }
    let mut mount_id = unsafe { std::mem::zeroed::<libc::statx>() };
    if unsafe {
        libc::statx(
            root.as_raw_fd(),
            c"".as_ptr(),
            libc::AT_EMPTY_PATH,
            libc::STATX_MNT_ID,
            &mut mount_id,
        )
    } != 0
        || mount_id.stx_mask & libc::STATX_MNT_ID == 0
    {
        return Err("actual private mount ID unavailable".into());
    }
    let mut ticket = serde_json::json!({"schema":SCHEMA,"quota_bytes":o.quota,"inode_limit":o.inodes,"working_ram_bytes":o.ram,"root":i.root,"root_device":meta.dev(),"root_inode":meta.ino(),"mount_id":mount_id.stx_mnt_id,"mount_namespace_inode":mnt,"parent_mount_namespace_inode":i.parent_mnt,"fallbacks":fallbacks,"lifetime":"consumer-process-mount-namespace","capabilities":"dropped-before-exec","write_confinement":"landlock-v3","consumer_requires_dumpable_zero":true});
    if let Some(held) = persistent.as_ref() {
        let path = o.persistent.as_ref().ok_or("persistent selection absent")?;
        let info = verify_persistent(path, held, &i.root)?;
        if info.dev() == meta.dev() {
            return Err("persistent store must be outside private tmpfs device".into());
        }
        ticket["schema"] = serde_json::json!("abyss_machine_private_tmpfs_stage_v2");
        ticket["persistent_store"] = serde_json::json!({"root":path,"root_device":info.dev(),"root_inode":info.ino(),"quota_scope":"outside-private-tmpfs"});
    }
    let bytes = serde_json::to_vec(&ticket).map_err(|e| e.to_string())?;
    if bytes.len() > 8192 {
        return Err("private stage ticket bound8192 exceeded".into());
    }
    let fd = unsafe {
        libc::memfd_create(
            if persistent.is_some() {
                c"abyss_machine_private_tmpfs_stage_v2".as_ptr()
            } else {
                c"abyss_machine_private_tmpfs_stage_v1".as_ptr()
            },
            libc::MFD_ALLOW_SEALING | libc::MFD_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(error());
    }
    let mut custody = unsafe { File::from_raw_fd(fd) };
    custody.write_all(&bytes).map_err(|e| e.to_string())?;
    if unsafe {
        libc::fcntl(
            fd,
            libc::F_ADD_SEALS,
            libc::F_SEAL_SEAL | libc::F_SEAL_SHRINK | libc::F_SEAL_GROW | libc::F_SEAL_WRITE,
        )
    } < 0
    {
        return Err(error());
    }
    end.check()?;
    loopback()?;
    topology(&i.setup, &o.consumer, &held, o.quota, o.ram, end)?;
    let mut placement = member(&held, "cgroup.procs", true)?;
    placement.write_all(b"0\n").map_err(|e| e.to_string())?;
    drop(placement);
    if membership()? != o.consumer {
        return Err("actual consumer membership differs after self placement".into());
    }
    consumer_limits(&held, o.ram)?;
    if o.sdk_phase_as.is_some() {
        phase_restore_consumer(
            o,
            i.control_auth.as_ref().ok_or("phase auth absent")?,
            o.control_fd.ok_or("phase FD absent")?,
            end,
        )?;
    }
    drop(held);
    unsafe { libc::close(i.consumer_fd) };
    std::env::set_current_dir(&i.root).map_err(|e| e.to_string())?;
    confine(&i.root, &fallbacks, persistent.as_ref())?;
    if let Some(held) = persistent.as_ref() {
        verify_persistent(
            o.persistent.as_ref().ok_or("persistent selection absent")?,
            held,
            &i.root,
        )?;
    }
    drop(persistent);
    if let Some(fd) = i.persistent_fd {
        unsafe { libc::close(fd) };
    }
    if unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0) } != 0 {
        return Err(error());
    }
    drop_caps()?;
    end.check()?;
    // All other inherited descriptors become CLOEXEC; only genuine sealed ticket
    // survives the explicit consumer exec. No writable cgroup FD reaches it.
    if unsafe { libc::syscall(libc::SYS_close_range, 3u32, u32::MAX, 4u32) } < 0 {
        return Err(error());
    }
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) } < 0 {
        return Err(error());
    }
    if let (Some(control_fd), Some(auth)) = (o.control_fd, i.control_auth.as_ref()) {
        match_control(control_fd, auth, end)?;
        let flags = unsafe { libc::fcntl(control_fd, libc::F_GETFD) };
        if flags < 0
            || unsafe { libc::fcntl(control_fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) } < 0
        {
            return Err(error());
        }
    }
    let mut command = Command::new(&o.command[0]);
    command
        .env_remove("ABYSS_CONSUMER_CONTROL_FD")
        .env_remove("ABYSS_CONSUMER_CONTROL_AUTH");
    if let (Some(control_fd), Some(auth)) = (o.control_fd, i.control_auth.as_ref()) {
        command
            .env("ABYSS_CONSUMER_CONTROL_FD", control_fd.to_string())
            .env(
                "ABYSS_CONSUMER_CONTROL_AUTH",
                serde_json::to_string(auth).map_err(|e| e.to_string())?,
            );
    }
    command
        .args(&o.command[1..])
        .env("ABYSS_STAGE_ROOT", &i.root)
        .env("ABYSS_STAGE_TICKET_FD", fd.to_string())
        .env("TMPDIR", &temporary)
        .env("SQLITE_TMPDIR", &temporary);
    Err(command.exec().to_string())
}
fn options(args: &[String]) -> Result<(Options, Option<Inner>), String> {
    if args.len() > ARGC + 40
        || args.iter().any(|s| s.len() > PATH_BYTES)
        || args
            .iter()
            .try_fold(0usize, |n, s| n.checked_add(s.len() + 1))
            .filter(|n| *n <= ARGV_BYTES)
            .is_none()
    {
        return Err("stage CLI transport exceeds finite source limits".into());
    }
    let mut values = std::collections::BTreeMap::new();
    let mut inner_flag = false;
    let mut index = 1;
    while index < args.len() && args[index] != "--" {
        if args[index] == "--namespace-inner" {
            if inner_flag {
                return Err("duplicate inner option".into());
            }
            inner_flag = true;
            index += 1;
            continue;
        }
        let value = args.get(index + 1).ok_or("option requires value")?;
        if values
            .insert(args[index].as_str(), value.as_str())
            .is_some()
        {
            return Err("duplicate option".into());
        }
        index += 2;
    }
    if args.get(index).map(String::as_str) != Some("--") {
        return Err("explicit consumer argv separator required".into());
    }
    let command = args[index + 1..].to_vec();
    let control_fd = values
        .remove("--consumer-control-fd")
        .map(|v| v.parse::<i32>().map_err(|e| e.to_string()))
        .transpose()?;
    let control_auth = values
        .remove("--control-auth")
        .map(|v| serde_json::from_str::<ControlAuth>(v).map_err(|e| e.to_string()))
        .transpose()?;
    if !inner_flag && control_auth.is_some() {
        return Err("control metadata is internal namespace handoff only".into());
    }
    let sdk_phase_as = phase_pair(
        values.remove("--sdk-setup-as-bytes"),
        values.remove("--sdk-guardian-state-bytes"),
    )?;
    let parent = values.remove("--expected-parent-pid");
    let original_mask = values.remove("--restore-signal-mask");
    let direct_custody = match (parent, original_mask) {
        (None, None) => None,
        (Some(parent), Some(csv)) if !inner_flag => {
            let parent = parent
                .parse::<libc::pid_t>()
                .map_err(|_| "direct stage parent PID")?;
            if parent <= 0 || csv.len() > 64 * 3 {
                return Err("direct stage parent/mask outside original finite bound".into());
            }
            let mut mask: libc::sigset_t = unsafe { std::mem::zeroed() };
            if unsafe { libc::sigemptyset(&mut mask) } != 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            if !csv.is_empty() {
                let mut count = 0;
                for field in csv.split(',') {
                    count += 1;
                    let signal = field
                        .parse::<libc::c_int>()
                        .map_err(|_| "direct stage original signal")?;
                    if count > 64
                        || !(1..=64).contains(&signal)
                        || signal == libc::SIGKILL
                        || signal == libc::SIGSTOP
                        || unsafe { libc::sigismember(&mask, signal) } != 0
                    {
                        return Err("direct stage original signal mask malformed".into());
                    }
                    if unsafe { libc::sigaddset(&mut mask, signal) } != 0 {
                        return Err(std::io::Error::last_os_error().to_string());
                    }
                }
            }
            Some((parent, mask))
        }
        _ => return Err("direct stage parent and mask require an outer paired selection".into()),
    };
    let persistent = values.remove("--persistent-store").map(PathBuf::from);
    let persistent_fd = values
        .remove("--persistent-fd")
        .map(|value| value.parse::<i32>().map_err(|e| e.to_string()))
        .transpose()?;
    if !inner_flag && persistent_fd.is_some() {
        return Err("persistent descriptor is internal namespace handoff only".into());
    }
    let mut get = |name: &str| values.remove(name).ok_or_else(|| format!("missing {name}"));
    macro_rules! n {
        ($key:literal) => {
            get($key)?
                .parse::<u64>()
                .map_err(|e| format!("{}: {e}", $key))?
        };
    }
    let o = Options {
        unshare: PathBuf::from(get("--unshare-exe")?),
        consumer: PathBuf::from(get("--consumer-cgroup")?),
        scratch: PathBuf::from(get("--scratch-parent")?),
        quota: n!("--quota-bytes"),
        inodes: n!("--inodes"),
        ram: n!("--working-ram-bytes"),
        original: n!("--work-deadline-ns"),
        shutdown_ms: n!("--maximum-shutdown-ms"),
        persistent,
        control_fd,
        sdk_phase_as,
        direct_custody,
        command,
    };
    let i = if inner_flag {
        Some(Inner {
            root: PathBuf::from(get("--root")?),
            setup: PathBuf::from(get("--setup-cgroup")?),
            consumer_fd: i32::try_from(n!("--consumer-fd")).map_err(|e| e.to_string())?,
            persistent_fd,
            parent_mnt: n!("--parent-mount-namespace"),
            parent_net: n!("--parent-net-namespace"),
            host_uid: n!("--host-uid"),
            host_gid: n!("--host-gid"),
            control_auth,
        })
    } else {
        None
    };
    if !values.is_empty() {
        return Err("unknown stage controller option".into());
    }
    Ok((o, i))
}

/// Native-only exec boundary: Node/Worker V8 stays under physical cgroup limits.
/// Original Core argv, inherited environment/stdin/stage FD and PID are retained.
fn native_process_exec(args: &[String]) -> Result<i32, String> {
    if args.len() < 7
        || args.len() > ARGC + 6
        || args.iter().any(|v| v.len() > PATH_BYTES)
        || args
            .iter()
            .try_fold(0usize, |n, v| n.checked_add(v.len() + 1))
            .filter(|n| *n <= ARGV_BYTES)
            .is_none()
        || args[1] != "--address-space-bytes"
        || args[3] != "--file-size-bytes"
        || args[5] != "--"
    {
        return Err("usage: native-process-exec --address-space-bytes N --file-size-bytes N -- ORIGINAL_CORE_ARGS".into());
    }
    let address = args[2]
        .parse::<u64>()
        .ok()
        .filter(|n| *n > 0)
        .ok_or("positive native address space limit required")?;
    let file_size = args[4]
        .parse::<u64>()
        .ok()
        .filter(|n| *n > 0)
        .ok_or("positive native file size limit required")?;
    for (resource, wanted) in [(libc::RLIMIT_AS, address), (libc::RLIMIT_FSIZE, file_size)] {
        let wanted = libc::rlim_t::try_from(wanted).map_err(|e| e.to_string())?;
        if wanted == libc::RLIM_INFINITY {
            return Err("requested native process bound must be finite".into());
        }
        let mut before = unsafe { std::mem::zeroed::<libc::rlimit>() };
        if unsafe { libc::getrlimit(resource, &mut before) } != 0 {
            return Err(error());
        }
        if before.rlim_cur != libc::RLIM_INFINITY && wanted > before.rlim_cur {
            return Err("native limit would raise finite parent soft boundary".into());
        }
        if before.rlim_max != libc::RLIM_INFINITY && wanted > before.rlim_max {
            return Err("native limit would raise finite parent hard boundary".into());
        }
        let selected = libc::rlimit {
            rlim_cur: wanted,
            rlim_max: wanted,
        };
        if unsafe { libc::setrlimit(resource, &selected) } != 0 {
            return Err(error());
        }
        let mut actual = unsafe { std::mem::zeroed::<libc::rlimit>() };
        if unsafe { libc::getrlimit(resource, &mut actual) } != 0
            || actual.rlim_cur != wanted
            || actual.rlim_max != wanted
        {
            return Err("native actual process hard bounds differ".into());
        }
    }
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    Err(Command::new(executable).args(&args[6..]).exec().to_string())
}

// Entered before SDK/Python threads under one caller-selected delegated scope.
// Configuration describes actual paths; private-stage-run remains the issuer.
const SDK_SETUP_BYTES: u64 = 536_870_912;
const SDK_CONSUMER_BYTES: u64 = 2_684_354_560;
const SDK_SCOPE_BYTES: u64 = 3_221_225_472;
fn sdk_write(root: &File, name: &str, value: &str) -> Result<(), String> {
    member(root, name, true)?
        .write_all(value.as_bytes())
        .map_err(|e| e.to_string())
}
fn sdk_singleton(root: &File) -> Result<(), String> {
    let expected = std::process::id().to_string();
    let actual = contents(root, "cgroup.procs", 4096)?;
    if actual.split_whitespace().collect::<Vec<_>>() != vec![expected.as_str()] {
        return Err("scope/setup must contain this bootstrap alone".into());
    }
    Ok(())
}
fn sdk_no_children(path: &Path, end: Cutoff) -> Result<(), String> {
    for (n, entry) in fs::read_dir(path).map_err(|e| e.to_string())?.enumerate() {
        end.cleanup_check()?;
        if n >= 512 {
            return Err("scope cgroup entry bound exceeded".into());
        }
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            return Err("unexpected descendant cgroup; external scope containment required".into());
        }
    }
    Ok(())
}
// Bootstrap may inherit infinity under the existing ordinary entry. Establish
// ONLY the same original2.5GiB allowance as old sdk_child_limits, never raise
// a finite inherited boundary; the subsequent guardian clamp stays lower-only.
fn sdk_bootstrap_original_as(end: Cutoff) -> Result<(), String> {
    end.check()?;
    let before = actual_as()?;
    let wanted = SDK_CONSUMER_BYTES as libc::rlim_t;
    if (before.rlim_cur != libc::RLIM_INFINITY && wanted > before.rlim_cur)
        || (before.rlim_max != libc::RLIM_INFINITY && wanted > before.rlim_max)
    {
        return Err("SDK bootstrap original AS allowance would raise inherited boundary".into());
    }
    let selected = libc::rlimit {
        rlim_cur: wanted,
        rlim_max: wanted,
    };
    if unsafe { libc::setrlimit(libc::RLIMIT_AS, &selected) } != 0 {
        return Err(error());
    }
    let actual = actual_as()?;
    if actual.rlim_cur != wanted || actual.rlim_max != wanted {
        return Err("SDK bootstrap original AS allowance drift".into());
    }
    end.check()
}
fn sdk_child_limits(phase: Option<SdkPhaseAs>) -> std::io::Result<()> {
    if let Some(p) = phase {
        let mut old = unsafe { std::mem::zeroed::<libc::rlimit>() };
        if unsafe { libc::getrlimit(libc::RLIMIT_AS, &mut old) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        if old.rlim_cur != SDK_PHASE_ROLE as libc::rlim_t
            || old.rlim_max != SDK_CONSUMER_BYTES as libc::rlim_t
        {
            return Err(std::io::Error::from_raw_os_error(libc::EPERM));
        }
        // Sole fresh Python allowance selected by this genuine bootstrap, before imports.
        let next = libc::rlimit {
            rlim_cur: p.setup_as_bytes as libc::rlim_t,
            rlim_max: old.rlim_max,
        };
        if unsafe { libc::setrlimit(libc::RLIMIT_AS, &next) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
    }

    for (resource, upper) in [
        (
            libc::RLIMIT_AS,
            phase.map_or(SDK_CONSUMER_BYTES, |p| p.setup_as_bytes),
        ),
        (libc::RLIMIT_FSIZE, 536_870_912),
    ] {
        let mut old = unsafe { std::mem::zeroed::<libc::rlimit>() };
        if unsafe { libc::getrlimit(resource, &mut old) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        let wanted = upper as libc::rlim_t;
        if (old.rlim_cur != libc::RLIM_INFINITY && wanted > old.rlim_cur)
            || (old.rlim_max != libc::RLIM_INFINITY && wanted > old.rlim_max)
        {
            return Err(std::io::Error::from_raw_os_error(libc::EPERM));
        }
        let hard = if resource == libc::RLIMIT_AS && phase.is_some() {
            old.rlim_max
        } else {
            wanted
        };
        let selected = libc::rlimit {
            rlim_cur: wanted,
            rlim_max: hard,
        };
        if unsafe { libc::setrlimit(resource, &selected) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        let mut actual = unsafe { std::mem::zeroed::<libc::rlimit>() };
        if unsafe { libc::getrlimit(resource, &mut actual) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        if actual.rlim_cur != wanted || actual.rlim_max != hard {
            return Err(std::io::Error::from_raw_os_error(libc::EPERM));
        }
    }
    Ok(())
}
fn sdk_python_run(args: &[String]) -> Result<i32, String> {
    if args.len() > ARGC + 16
        || args.iter().any(|v| v.len() > PATH_BYTES)
        || args
            .iter()
            .try_fold(0usize, |n, v| n.checked_add(v.len() + 1))
            .filter(|n| *n <= ARGV_BYTES)
            .is_none()
    {
        return Err("SDK CLI transport exceeds finite source limits".into());
    }
    let split = args
        .iter()
        .position(|a| a == "--")
        .ok_or("SDK command separator absent")?;
    if split < 1 || (split - 1) % 2 != 0 {
        return Err("bounded SDK option pairs required".into());
    }
    let mut selected = std::collections::BTreeMap::new();
    for pair in args[1..split].chunks_exact(2) {
        if !matches!(
            pair[0].as_str(),
            "--scope-name"
                | "--scratch-parent"
                | "--unshare-exe"
                | "--work-deadline-ns"
                | "--maximum-shutdown-ms"
                | "--persistent-store"
                | "--sdk-setup-as-bytes"
                | "--sdk-guardian-state-bytes"
        ) || selected
            .insert(pair[0].as_str(), pair[1].as_str())
            .is_some()
        {
            return Err("unknown or repeated SDK option".into());
        }
    }
    let sdk_phase_as = phase_pair(
        selected.get("--sdk-setup-as-bytes").copied(),
        selected.get("--sdk-guardian-state-bytes").copied(),
    )?;
    let get = |key: &str| {
        selected
            .get(key)
            .copied()
            .ok_or_else(|| format!("missing SDK option {key}"))
    };
    let original = get("--work-deadline-ns")?
        .parse::<u64>()
        .map_err(|e| e.to_string())?;
    let shutdown = get("--maximum-shutdown-ms")?
        .parse::<u64>()
        .map_err(|e| e.to_string())?;
    if shutdown != 5000
        || original
            .checked_sub(clock_ns()?)
            .is_none_or(|n| n > 50_000_000_000)
    {
        return Err("SDK original whole <=50s and explicit 5000ms cleanup profile required".into());
    }
    let end = Cutoff::select(original, shutdown)?;
    let name = get("--scope-name")?;
    let uuid = name
        .strip_prefix("tos-sdk-session-")
        .and_then(|s| s.strip_suffix(".scope"))
        .ok_or("unique SDK scope name required")?;
    if uuid.len() != 36
        || !uuid.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
            }
        })
    {
        return Err("canonical lowercase UUID scope required".into());
    }
    let common = membership()?;
    if common.file_name().and_then(OsStr::to_str) != Some(name) {
        return Err("actual scope name differs".into());
    }
    let common_fd = directory(&common)?;
    let meta = common_fd.metadata().map_err(|e| e.to_string())?;
    let mut filesystem = unsafe { std::mem::zeroed::<libc::statfs>() };
    if unsafe { libc::fstatfs(common_fd.as_raw_fd(), &mut filesystem) } != 0 {
        return Err(error());
    }
    if filesystem.f_type != libc::CGROUP2_SUPER_MAGIC {
        return Err("actual cgroup v2 filesystem required".into());
    }
    let uid = unsafe { libc::geteuid() };
    if uid == 0
        || meta.uid() != uid
        || scalar(&common_fd, "memory.max")? != SDK_SCOPE_BYTES
        || scalar(&common_fd, "memory.swap.max")? != 0
        || contents(&common_fd, "cgroup.type", 64)?.trim() != "domain"
        || !contents(&common_fd, "cgroup.subtree_control", 4096)?
            .trim()
            .is_empty()
        || !contents(&common_fd, "cgroup.controllers", 4096)?
            .split_whitespace()
            .any(|x| x == "memory")
    {
        return Err(
            "actual owned delegated 3GiB/swap0 empty-controller domain scope required".into(),
        );
    }
    sdk_singleton(&common_fd)?;
    sdk_no_children(&common, end)?;
    let mut tasks = fs::read_dir("/proc/self/task").map_err(|e| e.to_string())?;
    if tasks
        .next()
        .transpose()
        .map_err(|e| e.to_string())?
        .is_none()
        || tasks.next().is_some()
    {
        return Err("SDK bootstrap must precede all threads".into());
    }
    let scratch = PathBuf::from(get("--scratch-parent")?);
    let scratch_fd = directory(&scratch)?;
    let scratch_meta = scratch_fd.metadata().map_err(|e| e.to_string())?;
    if scratch_meta.uid() != uid || scratch_meta.mode() & 0o077 != 0 {
        return Err("caller-owned private scratch directory required".into());
    }
    let unshare = PathBuf::from(get("--unshare-exe")?);
    path_shape(&unshare)?;
    if fs::canonicalize(&unshare).map_err(|e| e.to_string())? != unshare || !unshare.is_file() {
        return Err("actual canonical unshare executable required".into());
    }
    let persistent = selected.get("--persistent-store").map(PathBuf::from);
    if let Some(path) = &persistent {
        let held = directory(path)?;
        let m = held.metadata().map_err(|e| e.to_string())?;
        if m.uid() != uid
            || m.mode() & 0o077 != 0
            || path.starts_with(&scratch)
            || scratch.starts_with(path)
        {
            return Err(
                "private persistent store must be separately admitted and disjoint from scratch"
                    .into(),
            );
        }
        for fallback in FALLBACKS {
            let fallback = Path::new(fallback);
            if path.starts_with(fallback) || fallback.starts_with(path) {
                return Err("persistent store overlaps fallback".into());
            }
        }
    }
    let setup = common.join("setup");
    let consumer = common.join("consumer");
    let o = Options {
        unshare,
        consumer: consumer.clone(),
        scratch,
        quota: SDK_SETUP_BYTES,
        inodes: 65536,
        ram: SDK_CONSUMER_BYTES,
        original,
        shutdown_ms: shutdown,
        persistent,
        control_fd: None,
        sdk_phase_as,
        direct_custody: None,
        command: args[split + 1..].to_vec(),
    };
    limits(&o)?;
    let _signals = SignalGuard::install()?;
    end.check()?;
    let mut setup_fd = None;
    let mut consumer_fd = None;
    let mut moved = false;
    let mut memory_added = false;
    let result = (|| -> Result<i32, String> {
        end.check()?;
        identical(&common_fd, &directory(&common)?)?;
        sdk_singleton(&common_fd)?;
        fs::create_dir(&setup).map_err(|e| e.to_string())?;
        setup_fd = Some(directory(&setup)?);
        fs::create_dir(&consumer).map_err(|e| e.to_string())?;
        consumer_fd = Some(directory(&consumer)?);
        let setup_held = setup_fd.as_ref().ok_or("owned setup absent")?;
        let consumer_held = consumer_fd.as_ref().ok_or("owned consumer absent")?;
        end.check()?;
        sdk_write(setup_held, "cgroup.procs", "0\n")?;
        moved = true;
        if membership()? != setup {
            return Err("SDK bootstrap setup placement differs".into());
        }
        if !contents(&common_fd, "cgroup.procs", 4096)?
            .trim()
            .is_empty()
        {
            return Err("scope parent not empty after placement".into());
        }
        end.check()?;
        sdk_write(&common_fd, "cgroup.subtree_control", "+memory\n")?;
        memory_added = true;
        for (fd, bytes) in [
            (setup_held, SDK_SETUP_BYTES),
            (consumer_held, SDK_CONSUMER_BYTES),
        ] {
            end.check()?;
            sdk_write(fd, "memory.max", &format!("{bytes}\n"))?;
            sdk_write(fd, "memory.swap.max", "0\n")?;
        }
        topology(&setup, &consumer, consumer_held, o.quota, o.ram, end)?;
        if !contents(consumer_held, "cgroup.procs", 4096)?
            .trim()
            .is_empty()
            || consumer_populated(consumer_held)?
        {
            return Err("SDK consumer not empty".into());
        }
        drop(member(consumer_held, "cgroup.kill", true)?);
        let mut config = serde_json::json!({"schema":"tos_sdk_stage_config_v1","setup_cgroup":setup,"consumer_cgroup":consumer,"scratch_parent":o.scratch,"unshare_exe":o.unshare,"original_whole_deadline_ns":original.to_string(),"original_work_deadline_ns":end.work.to_string(),"maximum_shutdown_ms":shutdown,"quota_bytes":o.quota,"inode_limit":o.inodes,"working_ram_bytes":o.ram,"aggregate_ram_bytes":SDK_SCOPE_BYTES,"swap_max_bytes":0});
        if let Some(path) = &o.persistent {
            config["persistent_store"] = serde_json::json!(path);
        }
        if let Some(p) = sdk_phase_as {
            config["setup_as_bytes"] = serde_json::json!(p.setup_as_bytes);
            config["guardian_state_bytes"] = serde_json::json!(p.guardian_state_bytes);
        }
        let encoded = serde_json::to_string(&config).map_err(|e| e.to_string())?;
        if encoded.len() > 65536 {
            return Err("SDK stage configuration exceeds finite envelope".into());
        }
        if sdk_phase_as.is_some() {
            sdk_bootstrap_original_as(end)?;
            guardian_as(SDK_PHASE_ROLE, end)?;
        }
        let mut command = Command::new(&o.command[0]);
        command
            .args(&o.command[1..])
            .env("TOS_SDK_STAGE_CONFIG", encoded)
            .env_remove("ABYSS_STAGE_TICKET_FD")
            .env_remove("ABYSS_STAGE_ROOT")
            .env_remove("ABYSS_CONSUMER_CONTROL_FD")
            .env_remove("ABYSS_CONSUMER_CONTROL_AUTH");
        if let Some(p) = sdk_phase_as {
            command
                .env("TOS_SDK_SETUP_AS_BYTES", p.setup_as_bytes.to_string())
                .env(
                    "TOS_SDK_GUARDIAN_STATE_BYTES",
                    p.guardian_state_bytes.to_string(),
                )
                .env("TOS_SDK_ORIGINAL_WORK_DEADLINE_NS", end.work.to_string())
                .env("TOS_SDK_ORIGINAL_WHOLE_DEADLINE_NS", end.whole.to_string());
        }
        unsafe {
            command.pre_exec(move || sdk_child_limits(sdk_phase_as));
        }
        end.check()?;
        // No cgroup handle or stage ticket is inherited by the SDK entry.
        let (mut leader, restoration) = Leader::spawn(command, -1, None, None)?;
        let outcome = (|| -> Result<i32, String> {
            if let Some(e) = restoration {
                return Err(e);
            }
            loop {
                end.check()?;
                if let Some(code) = leader.exited()? {
                    return Ok(code);
                }
                thread::sleep(Duration::from_millis(5));
            }
        })();
        let closed = cleanup(&mut leader, consumer_held, end);
        match (outcome, closed) {
            (Ok(code), Ok(())) => Ok(code),
            (a, b) => Err(format!(
                "SDK result {a:?}; cleanup {b:?}; external scope custody may be required"
            )),
        }
    })();
    let restored = (|| -> Result<(), String> {
        end.cleanup_check()?;
        identical(&common_fd, &directory(&common)?)?;
        if let Some(fd) = consumer_fd.as_ref() {
            identical(fd, &directory(&consumer)?)?;
            if consumer_populated(fd)? {
                return Err("consumer still populated; external scope containment required".into());
            }
            sdk_no_children(&consumer, end)?;
            fs::remove_dir(&consumer).map_err(|e| e.to_string())?;
        }
        if moved {
            let fd = setup_fd.as_ref().ok_or("setup identity absent")?;
            identical(fd, &directory(&setup)?)?;
            sdk_singleton(fd)?;
            sdk_no_children(&setup, end)?;
        }
        if memory_added {
            sdk_write(&common_fd, "cgroup.subtree_control", "-memory\n")?;
        }
        if moved {
            sdk_write(&common_fd, "cgroup.procs", "0\n")?;
            if membership()? != common {
                return Err("scope restore placement differs".into());
            }
        }
        if let Some(fd) = setup_fd.as_ref() {
            identical(fd, &directory(&setup)?)?;
            sdk_no_children(&setup, end)?;
            fs::remove_dir(&setup).map_err(|e| e.to_string())?;
        }
        sdk_singleton(&common_fd)?;
        sdk_no_children(&common, end)?;
        if !contents(&common_fd, "cgroup.subtree_control", 4096)?
            .trim()
            .is_empty()
        {
            return Err("scope controller restoration differs".into());
        }
        end.cleanup_check()
    })();
    if let Err(e) = restored {
        return Err(format!(
            "SDK result {result:?}; owned scope restoration {e}; external containment required"
        ));
    }
    if CANCELLED.load(Ordering::Relaxed) {
        return Err("SDK session cancellation observed".into());
    }
    result
}

/// CLI owns process-global signal handling and a genuine OS stage lifetime.
pub fn run_if_requested(args: &[String]) -> Option<i32> {
    if args.first().map(String::as_str) == Some("sdk-python-run") {
        return Some(match sdk_python_run(args) {
            Ok(code) => code,
            Err(e) => {
                eprintln!("SDK scope bootstrap refused: {e}");
                125
            }
        });
    }
    if args.first().map(String::as_str) == Some("native-process-exec") {
        return Some(match native_process_exec(args) {
            Ok(code) => code,
            Err(e) => {
                eprintln!("native process boundary refused: {e}");
                125
            }
        });
    }
    if args.first().map(String::as_str) != Some("private-stage-run") {
        return None;
    }
    Some(
        match options(args).and_then(|(o, i)| match i {
            Some(i) => inner(&o, &i),
            None => outer(&o),
        }) {
            Ok(code) => code,
            Err(e) => {
                eprintln!("private stage startup/cleanup refused: {e}");
                125
            }
        },
    )
}
