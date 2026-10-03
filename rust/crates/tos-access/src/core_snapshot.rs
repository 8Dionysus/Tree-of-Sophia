//! Private native Core transport. Every limit and selected source is supplied by the caller.
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
    admission: Admission,
    source_paths: Sources,
    arguments: Value,
    query_store: QueryStoreSelection,
    http: Option<crate::core_http_admission::HttpAdmission>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QueryStoreSelection {
    path: PathBuf,
    configured: bool,
}

struct Selection {
    root: PathBuf,
    operation: String,
    state_fd: Option<i32>,
    reply_fd: Option<i32>,
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
            "--root" if root.is_none() => root = Some(PathBuf::from(value)),
            "--operation" if operation.is_none() => operation = Some(value.clone()),
            "--snapshot-state-fd" if state_fd.is_none() => {
                state_fd = Some(value.parse::<i32>().map_err(|_| "Core state FD")?)
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
    Ok(Some(Selection {
        root,
        operation: operation.ok_or("Core operation absent")?,
        state_fd,
        reply_fd,
    }))
}
fn read_request(input: &mut dyn Read) -> Result<Request> {
    let mut raw = Vec::new();
    input
        .take((INPUT_CAP + 1) as u64)
        .read_to_end(&mut raw)
        .map_err(|_| "Core request read")?;
    if raw.len() > INPUT_CAP {
        return Err("Core request byte cap");
    }
    // Parse the small admission envelope under fixed transport limits before trusting its limits.
    let guard = JsonLimits {
        max_bytes: INPUT_CAP,
        max_depth: 128,
        max_visits: INPUT_CAP,
        max_integer_digits: 4096,
    };
    let parsed = parse_json(&raw, JsonMode::PublishedStrict, guard)
        .map_err(|_| "Core strict request JSON")?;
    drop(parsed);
    let request: Request = serde_json::from_slice(&raw).map_err(|_| "Core request shape")?;
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
    use std::os::{
        fd::FromRawFd,
        unix::{ffi::OsStrExt, fs::MetadataExt},
    };
    active(deadline)?;
    let before = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e)
            if matches!(
                e.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied
            ) =>
        {
            active(deadline)?;
            return Ok(false);
        }
        Err(_) => return Err("Core selected existence lookup"),
    };
    if !before.is_file() {
        active(deadline)?;
        return Ok(false);
    }
    let name = std::ffi::CString::new(path.as_os_str().as_bytes())
        .map_err(|_| "Core selected path NUL")?;
    let fd = unsafe { libc::open(name.as_ptr(), libc::O_PATH | libc::O_CLOEXEC) };
    if fd < 0 {
        return Err("Core selected existence hold");
    }
    let file = unsafe { std::fs::File::from_raw_fd(fd) };
    let held = file
        .metadata()
        .map_err(|_| "Core selected existence metadata")?;
    let after = std::fs::metadata(path).map_err(|_| "Core selected existence changed")?;
    let identity = |m: &std::fs::Metadata| {
        (
            m.dev(),
            m.ino(),
            m.mode(),
            m.size(),
            m.mtime(),
            m.mtime_nsec(),
            m.ctime(),
            m.ctime_nsec(),
        )
    };
    if !held.is_file()
        || identity(&before) != identity(&held)
        || identity(&held) != identity(&after)
    {
        return Err("Core selected existence changed");
    }
    active(deadline)?;
    Ok(true)
}

fn send_state(
    reply_fd: i32,
    state: Option<&tos_compiler::native_snapshot::ProducerIssuedCoreSnapshotState>,
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
    let marker = br#"{"role":"tos-native-core-snapshot-state-v1","schema_version":"tos_native_core_snapshot_state_v1"}"#;
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
        .map_err(|_| "Core native whole snapshot refused")?;
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
    .map_err(|_| "Core selected evidence owner check refused")?;
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

fn run(selection: Selection, request: Request) -> Result<()> {
    use tos_compiler::RuntimeCaptureRole as R;
    use tos_compiler::native_snapshot_carriers::CapturedCarrierRequest as C;
    let deadline = request.admission.deadline()?;
    let signal = SignalGuard::install()?;
    let cancelled = signal.token.clone();
    let operation = operation(&selection.operation, request.arguments.clone())?;
    // The former QueryStore has independent five-input binding and selection
    // semantics. Until that owner is migrated, do not silently replace an
    // explicitly selected or discovered existing store with a source rebuild.
    if request.query_store.configured || request.query_store.path.exists() {
        return Err("Core selected legacy QueryStore requires its native owner");
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
    if let Operation::Exists(kind) = operation {
        return disclose_bytes(&existence_result(kind, &request, deadline)?, deadline);
    }
    if let Operation::Serve(listen, max_connections) = &operation {
        return serve_selected_root(
            &selection.root,
            &request,
            listen,
            *max_connections,
            deadline,
            &cancelled,
        );
    }
    if request.http.is_some() {
        return Err("Core HTTP allowances supplied to a non-HTTP operation");
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
            if *bibliographic_only {
                R::Bibliographic
            } else {
                R::Corpus
            },
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
        let held = read_selected_evidence(&selection.root, &request, deadline, &cancelled)?;
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
pub fn run_if_requested(args: &[String], input: &mut dyn Read) -> Option<i32> {
    match selection(args) {
        Ok(None) => None,
        Err(_) => Some(2),
        Ok(Some(selection)) => Some(
            match read_request(input).and_then(|request| run(selection, request)) {
                Ok(()) => 0,
                Err(_) => 1,
            },
        ),
    }
}

fn serve_selected_root(
    root: &Path,
    request: &Request,
    listen: &str,
    max_connections: u64,
    deadline: Instant,
    cancelled: &Arc<AtomicBool>,
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
    .map_err(|_| "Core HTTP full native producer refused")?;
    let evidence = tos_compiler::native_snapshot::check_completed_evidence_projection(
        &capture,
        &fresh(isolation.root(), "tos-core-http-evidence")?,
        limits.capture,
        deadline,
    )
    .map_err(|_| "Core HTTP evidence owner refused")?;
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
    .map_err(|_| "Core HTTP held callback or delivery refused")
}
