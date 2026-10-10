//! Linux execution in a dedicated, single-threaded process with no other children.
//! Subreaper custody is process-wide; this is a CLI boundary, not an in-process
//! task runner. It does not sandbox trusted mechanics tools or migrate Python.

use crate::Plan;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub command_wall: Duration,
    pub lane_wall: Duration,
    pub cleanup_grace: Duration,
    /// Combined child stdout and stderr, per command.
    pub output_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            command_wall: Duration::from_secs(300),
            lane_wall: Duration::from_secs(3600),
            cleanup_grace: Duration::from_secs(1),
            output_bytes: 16 * 1024 * 1024,
        }
    }
}

/// Runs in plan order, fails on the first nonzero child, and prints the legacy
/// runner's progress/success lines. Cancellation holds a signal number (2 or
/// 15); the caller must only set it from a signal handler or bounded callback.
/// Returns 0 for success, 1 for execution/limit failure, 128+signal for cancel.
/// Requires an otherwise disposable Linux process; refuses existing threads or
/// children. SIGKILL of this supervisor is outside the in-process guarantee.
pub fn run(root: &Path, plan: &Plan, limits: Limits, cancel: &AtomicI32) -> io::Result<i32> {
    #[cfg(target_os = "linux")]
    {
        native::run(root, plan, limits, cancel, native::Style::Mechanics)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (root, plan, limits, cancel);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "execution requires Linux subreaper/pidfd custody",
        ))
    }
}

/// Execute selected manifest commands through the same dedicated Linux
/// pidfd/subreaper boundary as mechanics-local. The labels and first failing
/// child status belong to the validation-lane command contract.
pub fn run_validation_sequence(
    root: &Path,
    python: &str,
    steps: &[crate::validation_lanes::BudgetedCommandStep],
    limits: Limits,
    cancel: &AtomicI32,
) -> io::Result<i32> {
    // Only validation selections carry source-owned per-step budgets. Other
    // executor clients and serialized mechanics plans retain their old shape.
    let expanded = crate::growth_native_plan::expand_steps(root, steps)?;
    let mut commands = Vec::with_capacity(expanded.len());
    let mut timeouts = Vec::with_capacity(expanded.len());
    for (command, timeout_ms) in &expanded {
        if timeout_ms.is_some_and(|value| value == 0 || value > 3_600_000) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid per-step validation timeout",
            ));
        }
        commands.push(command.clone());
        timeouts.push(timeout_ms.map(Duration::from_millis));
    }
    let plan = selected_plan(
        &commands,
        "tos_validation_lanes_selected_v1",
        "validation_lane_step",
    );
    #[cfg(target_os = "linux")]
    {
        native::run(
            root,
            &plan,
            limits,
            cancel,
            native::Style::Validation(python, &timeouts),
        )
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (root, plan, limits, cancel, timeouts);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "execution requires Linux subreaper/pidfd custody",
        ))
    }
}

/// Run an already-selected release phase through the same dedicated process
/// custody. An empty `checks` phase is a successful no-op, as in Python.
pub fn run_release_sequence(
    root: &Path,
    steps: &[(String, Vec<String>)],
    limits: Limits,
    cancel: &AtomicI32,
) -> io::Result<i32> {
    if steps.is_empty() {
        return Ok(0);
    }
    let plan = selected_plan(steps, "tos_release_check_selected_v1", "release_check_step");
    #[cfg(target_os = "linux")]
    {
        native::run(root, &plan, limits, cancel, native::Style::Release)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (root, plan, limits, cancel);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "execution requires Linux subreaper/pidfd custody",
        ))
    }
}

/// Capture one trusted command for the software CI Git reader. Uses the same
/// process custody and combined-output ceiling, without progress on stdout.
/// Kept crate-private: it is not a second general command runner.
pub(crate) fn capture_ci_git(
    root: &Path,
    argv: Vec<String>,
    limits: Limits,
    cancel: &AtomicI32,
) -> io::Result<(i32, Vec<u8>, Vec<u8>)> {
    #[cfg(target_os = "linux")]
    {
        let plan = selected_plan(&[(String::new(), argv)], "tos_ci_git_capture_v1", "ci_git");
        let mut streams = [Vec::new(), Vec::new()];
        let code = native::run_captured(root, &plan, limits, cancel, &mut streams)?;
        let [stdout, stderr] = streams;
        Ok((code, stdout, stderr))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (root, argv, limits, cancel);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "execution requires Linux subreaper/pidfd custody",
        ))
    }
}

/// The selected KAG owner's Python producer/probe are host adapters. Reuse the
/// existing bounded subreaper custody; the publisher owns result validation.
pub(crate) fn capture_kag_owner(
    root: &Path,
    argv: Vec<String>,
    limits: Limits,
    cancel: &AtomicI32,
) -> io::Result<(i32, Vec<u8>, Vec<u8>)> {
    capture_ci_git(root, argv, limits, cancel)
}

/// The external artifact owner's declared CLI, with one explicit store scope.
/// The environment belongs only to the child; nested rehearsals cannot alter
/// this process or acquire ambient host stores.
pub(crate) fn capture_artifact_owner(
    root: &Path,
    argv: Vec<String>,
    store: &Path,
    limits: Limits,
    cancel: &AtomicI32,
) -> io::Result<(i32, Vec<u8>, Vec<u8>)> {
    #[cfg(target_os = "linux")]
    {
        let store = store
            .to_str()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "artifact store path"))?;
        let plan = selected_plan(
            &[(String::new(), argv)],
            "tos_artifact_owner_capture_v1",
            "artifact_owner",
        );
        let mut streams = [Vec::new(), Vec::new()];
        let overrides = [(
            "ABYSS_MACHINE_ARTIFACT_SUBJECT_STORE_ISOLATED_ROOT".into(),
            store.into(),
        )];
        let code =
            native::run_captured_with_env(root, &plan, limits, cancel, &mut streams, &overrides)?;
        let [stdout, stderr] = streams;
        Ok((code, stdout, stderr))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (root, argv, store, limits, cancel);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "artifact owner requires Linux process custody",
        ))
    }
}

/// The philosophy CLI's existing Linux custody, with its original deadline
/// captured before option/worker setup. Other executor clients keep their law.
pub(crate) fn run_philosophy_product(
    argv: Vec<String>,
    limits: Limits,
    cancel: &AtomicI32,
    deadline: std::time::Instant,
) -> io::Result<i32> {
    #[cfg(target_os = "linux")]
    {
        let plan = selected_plan(
            &[(String::new(), argv)],
            "tos_philosophy_product_worker_v1",
            "philosophy_product",
        );
        native::run_product_until(Path::new("/"), &plan, limits, cancel, deadline)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (argv, limits, cancel, deadline);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "philosophy CLI requires Linux subreaper/pidfd custody",
        ))
    }
}

/// Exact interpreter facts observation for the KAG receipt admission check.
/// Isolated standard-library metadata only; no repository imports or fallback.
pub(crate) fn capture_agent_surface_runtime(
    root: &Path,
    interpreter: &Path,
    distributions: &[String],
    remaining: Duration,
    cancel: &AtomicI32,
) -> io::Result<(i32, Vec<u8>, Vec<u8>)> {
    const PROBE: &str = r#"import sys,json,importlib.metadata
versions={}
for name in json.loads(sys.argv[1]):
 try: versions[name]={'state':'installed','version':importlib.metadata.version(name)}
 except importlib.metadata.PackageNotFoundError: versions[name]={'state':'missing'}
 except (TypeError,ValueError) as exc: versions[name]={'state':'error','error':str(exc)[:1024]}
print(json.dumps({'implementation':sys.implementation.name,'version':list(sys.version_info[:3]),'dependencies':versions},separators=(',',':')))
"#;
    if !interpreter.is_absolute() || !interpreter.is_file() || interpreter.as_os_str().len() > 4096
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "explicit Python interpreter must be an absolute executable file",
        ));
    }
    if distributions.len() > 64
        || distributions
            .iter()
            .any(|n| n.is_empty() || n.len() > 256 || n.chars().any(|c| c < ' ' || c == '\u{7f}'))
        || remaining.is_zero()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid bounded runtime facts request",
        ));
    }
    let argv = vec![
        interpreter
            .to_str()
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "non-UTF-8 Python interpreter")
            })?
            .to_owned(),
        "-I".into(),
        "-c".into(),
        PROBE.into(),
        serde_json::to_string(distributions).map_err(io::Error::other)?,
    ];
    #[cfg(target_os = "linux")]
    {
        let plan = selected_plan(
            &[(String::new(), argv)],
            "tos_agent_surface_runtime_facts_v1",
            "agent_surface_runtime",
        );
        let mut streams = [Vec::new(), Vec::new()];
        let code = native::run_captured(
            root,
            &plan,
            Limits {
                command_wall: remaining.min(Duration::from_secs(10)),
                lane_wall: remaining.min(Duration::from_secs(10)),
                cleanup_grace: Duration::from_secs(1),
                output_bytes: 16384,
            },
            cancel,
            &mut streams,
        )?;
        let [stdout, stderr] = streams;
        Ok((code, stdout, stderr))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (root, argv, cancel);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "runtime facts require Linux execution custody",
        ))
    }
}

fn selected_plan(
    steps: &[(String, Vec<String>)],
    schema_version: &'static str,
    kind: &'static str,
) -> Plan {
    Plan {
        schema_version,
        test_file_count: 0,
        commands: steps
            .iter()
            .map(|(label, argv)| crate::Command {
                kind,
                home: label.clone(),
                argv: argv.clone(),
            })
            .collect(),
    }
}

#[cfg(target_os = "linux")]
mod native {
    use super::*;
    use std::ffi::CString;
    use std::fs::{self, File};
    use std::io::Read;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::process::ExitStatusExt;
    use std::process::ExitStatus;
    use std::thread;
    use std::time::Instant;

    #[derive(Clone, Copy)]
    pub(super) enum Style<'a> {
        Mechanics,
        Validation(&'a str, &'a [Option<Duration>]),
        Release,
        Capture,
    }

    // Python subprocess.list2cmdline, used only for release progress text.
    // The argv passed to execvp remains the original selected manifest argv.
    fn list2cmdline(argv: &[String]) -> String {
        let mut result = String::new();
        for (index, argument) in argv.iter().enumerate() {
            if index != 0 {
                result.push(' ');
            }
            let quote = argument.is_empty() || argument.contains(' ') || argument.contains('\t');
            if quote {
                result.push('"');
            }
            let mut backslashes = 0usize;
            for character in argument.chars() {
                match character {
                    '\\' => backslashes += 1,
                    '"' => {
                        for _ in 0..backslashes * 2 + 1 {
                            result.push('\\');
                        }
                        backslashes = 0;
                        result.push('"');
                    }
                    _ => {
                        for _ in 0..backslashes {
                            result.push('\\');
                        }
                        backslashes = 0;
                        result.push(character);
                    }
                }
            }
            for _ in 0..backslashes {
                result.push('\\');
            }
            if quote {
                for _ in 0..backslashes {
                    result.push('\\');
                }
                result.push('"');
            }
        }
        result
    }

    fn error(message: impl Into<String>) -> io::Error {
        io::Error::other(message.into())
    }

    // Parse and visit each PID as it arrives; count and proc text size never
    // prevent an earlier descendant from being killed. Fixed memory and the
    // caller's one wall deadline bound every scan, including many children.
    fn visit_pid_list(
        mut reader: impl Read,
        deadline: Instant,
        mut visit: impl FnMut(i32),
    ) -> io::Result<usize> {
        let mut buffer = [0u8; 8192];
        let mut value = 0u32;
        let mut digits = 0usize;
        let mut count = 0usize;
        loop {
            if Instant::now() >= deadline {
                return Err(error("descendant enumeration deadline"));
            }
            let len = match reader.read(&mut buffer) {
                Ok(len) => len,
                Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
                Err(err) => return Err(err),
            };
            for byte in &buffer[..len] {
                if Instant::now() >= deadline {
                    return Err(error("descendant enumeration deadline"));
                }
                if byte.is_ascii_digit() {
                    digits += 1;
                    value = value
                        .checked_mul(10)
                        .and_then(|v| v.checked_add((byte - b'0') as u32))
                        .filter(|v| digits <= 10 && *v <= i32::MAX as u32)
                        .ok_or_else(|| error("invalid descendant PID"))?;
                } else if byte.is_ascii_whitespace() {
                    if digits != 0 {
                        if value == 0 {
                            return Err(error("invalid descendant PID"));
                        }
                        visit(value as i32);
                        count = count.saturating_add(1);
                        value = 0;
                        digits = 0;
                    }
                } else {
                    return Err(error("invalid descendant PID"));
                }
            }
            if len == 0 {
                if digits != 0 {
                    if value == 0 {
                        return Err(error("invalid descendant PID"));
                    }
                    visit(value as i32);
                    count = count.saturating_add(1);
                }
                return Ok(count);
            }
        }
    }

    fn visit_children(deadline: Instant, visit: impl FnMut(i32)) -> io::Result<usize> {
        visit_pid_list(
            File::open(format!("/proc/self/task/{}/children", std::process::id()))?,
            deadline,
            visit,
        )
    }

    fn pidfd(pid: i32) -> io::Result<File> {
        let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) } as i32;
        if fd < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(unsafe { File::from_raw_fd(fd) })
        }
    }

    fn signal(fd: &File, signal: i32) -> io::Result<()> {
        let rc = unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                fd.as_raw_fd(),
                signal,
                std::ptr::null::<libc::siginfo_t>(),
                0,
            )
        };
        if rc == 0 {
            return Ok(());
        }
        let err = io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::ESRCH) {
            Ok(())
        } else {
            Err(err)
        }
    }

    struct Custody {
        pid: i32,
        identity: File,
        reaped: bool,
        cleaned: bool,
        grace: Duration,
    }

    impl Custody {
        fn poll_exit(&mut self) -> io::Result<Option<ExitStatus>> {
            if self.reaped {
                return Ok(None);
            }
            let mut raw = 0;
            let rc = unsafe { libc::waitpid(self.pid, &mut raw, libc::WNOHANG) };
            if rc == self.pid {
                self.reaped = true;
                Ok(Some(ExitStatus::from_raw(raw)))
            } else if rc == 0 {
                Ok(None)
            } else if io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
                Ok(None)
            } else {
                Err(io::Error::last_os_error())
            }
        }

        fn cleanup(&mut self) -> io::Result<()> {
            if self.cleaned {
                return Ok(());
            }
            self.cleaned = true;
            self.cleanup_once()
        }

        fn cleanup_once(&mut self) -> io::Result<()> {
            let deadline = Instant::now() + self.grace;
            let mut first_error = None;
            let mut residual_sample = Vec::with_capacity(32);
            // This process is the sole reaper. Non-reaped root/direct-child
            // PIDs stay reserved; pidfds hold identities across every signal.
            if !self.reaped {
                unsafe {
                    libc::kill(-self.pid, libc::SIGKILL);
                }
                if let Err(err) = signal(&self.identity, libc::SIGKILL) {
                    first_error = Some(err);
                }
            }
            loop {
                if !self.reaped {
                    if let Err(err) = self.poll_exit() {
                        if first_error.is_none() {
                            first_error = Some(err);
                        }
                    }
                }
                residual_sample.clear();
                let root_pid = self.pid;
                let root_reaped = self.reaped;
                let scanned = visit_children(deadline, |pid| {
                    if !root_reaped && pid == root_pid {
                        return;
                    }
                    let result = pidfd(pid).and_then(|identity| signal(&identity, libc::SIGKILL));
                    if let Err(err) = result {
                        // One unavailable PID cannot prevent cleanup of all
                        // subsequent descendants in this or later sweeps.
                        if first_error.is_none() {
                            first_error = Some(err);
                        }
                    }
                    let mut status = 0;
                    let rc = unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) };
                    if rc <= 0 {
                        if residual_sample.len() < 32 {
                            residual_sample.push(pid);
                        }
                        if rc < 0 && first_error.is_none() {
                            first_error = Some(io::Error::last_os_error());
                        }
                    }
                });
                match scanned {
                    Ok(0) if self.reaped => {
                        return match first_error {
                            None => Ok(()),
                            Some(err) => Err(err),
                        };
                    }
                    Err(err) if first_error.is_none() => first_error = Some(err),
                    _ => {}
                }
                if Instant::now() >= deadline {
                    return Err(error(format!(
                        "cleanup deadline; root PID {} reaped={}; residual PID sample (max32) {residual_sample:?}; first error: {}",
                        self.pid,
                        self.reaped,
                        first_error.map_or_else(|| "none".into(), |e| e.to_string()),
                    )));
                }
                thread::sleep(Duration::from_millis(1));
            }
        }
    }

    impl Drop for Custody {
        fn drop(&mut self) {
            let _ = self.cleanup();
        }
    }

    fn pipe() -> io::Result<(File, File)> {
        let mut fds = [-1; 2];
        if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(unsafe { (File::from_raw_fd(fds[0]), File::from_raw_fd(fds[1])) })
    }

    fn spawn(
        root: &Path,
        argv: &[String],
        grace: Duration,
        overrides: &[(String, String)],
        inherited_stage_ticket_fd: Option<i32>,
    ) -> io::Result<(Custody, File, File)> {
        let root = CString::new(root.as_os_str().as_bytes()).map_err(|_| error("NUL root"))?;
        let args: Result<Vec<CString>, _> =
            argv.iter().map(|a| CString::new(a.as_bytes())).collect();
        let args = args.map_err(|_| error("NUL argv"))?;
        let mut pointers: Vec<*const libc::c_char> = args.iter().map(|a| a.as_ptr()).collect();
        pointers.push(std::ptr::null());
        let environment: Vec<CString> = std::env::vars_os()
            .filter(|(key, _)| !overrides.iter().any(|(name, _)| key == name.as_str()))
            .map(|(key, value)| {
                let mut entry = key.as_bytes().to_vec();
                entry.push(b'=');
                entry.extend_from_slice(value.as_bytes());
                CString::new(entry).map_err(|_| error("NUL environment"))
            })
            .chain(overrides.iter().map(|(key, value)| {
                CString::new(format!("{key}={value}")).map_err(|_| error("NUL environment"))
            }))
            .collect::<io::Result<_>>()?;
        let mut envp: Vec<*const libc::c_char> = environment.iter().map(|s| s.as_ptr()).collect();
        envp.push(std::ptr::null());
        let (stdout, out_child) = pipe()?;
        let (stderr, err_child) = pipe()?;
        let parent = std::process::id() as i32;
        let pid = unsafe { libc::fork() };
        if pid < 0 {
            return Err(io::Error::last_os_error());
        }
        if pid == 0 {
            // Only preallocated memory and async-signal-safe syscalls. No
            // synchronous exec-error-pipe handshake can delay the supervisor.
            unsafe {
                if libc::setpgid(0, 0) != 0
                    || libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) != 0
                    || libc::getppid() != parent
                    || libc::chdir(root.as_ptr()) != 0
                    || libc::dup2(out_child.as_raw_fd(), 1) < 0
                    || libc::dup2(err_child.as_raw_fd(), 2) < 0
                    || libc::close_range(3, u32::MAX, 4) != 0
                {
                    libc::_exit(126);
                }
                if let Some(fd) = inherited_stage_ticket_fd {
                    if libc::fcntl(fd, libc::F_SETFD, 0) < 0 {
                        libc::_exit(126);
                    }
                }
                libc::execvpe(pointers[0], pointers.as_ptr(), envp.as_ptr());
                libc::_exit(127);
            }
        }
        drop(out_child);
        drop(err_child);
        let identity = match pidfd(pid) {
            Ok(fd) => fd,
            Err(err) => {
                // A non-reaped PID remains reserved. Kill fail-closed; report
                // the exact residual identity instead of claiming cleanup.
                unsafe {
                    libc::kill(-pid, libc::SIGKILL);
                    libc::kill(pid, libc::SIGKILL);
                }
                return Err(error(format!(
                    "pidfd custody unavailable; residual PID {pid}: {err}"
                )));
            }
        };
        Ok((
            Custody {
                pid,
                identity,
                reaped: false,
                cleaned: false,
                grace,
            },
            stdout,
            stderr,
        ))
    }

    struct Nonblocking {
        fd: i32,
        previous: i32,
        restored: bool,
    }
    impl Nonblocking {
        fn new(fd: i32) -> io::Result<Self> {
            let previous = unsafe { libc::fcntl(fd, libc::F_GETFL) };
            if previous < 0
                || unsafe { libc::fcntl(fd, libc::F_SETFL, previous | libc::O_NONBLOCK) } < 0
            {
                return Err(io::Error::last_os_error());
            }
            Ok(Self {
                fd,
                previous,
                restored: false,
            })
        }
        fn restore(&mut self) -> io::Result<()> {
            if !self.restored {
                if unsafe { libc::fcntl(self.fd, libc::F_SETFL, self.previous) } < 0 {
                    return Err(io::Error::last_os_error());
                }
                self.restored = true;
            }
            Ok(())
        }
    }
    impl Drop for Nonblocking {
        fn drop(&mut self) {
            let _ = self.restore();
        }
    }

    fn cancelled(cancel: &AtomicI32) -> io::Result<()> {
        if cancel.load(Ordering::Relaxed) != 0 {
            Err(error("execution cancelled"))
        } else {
            Ok(())
        }
    }

    fn is_native_foundation_command(argv: &[String]) -> bool {
        argv.first()
            .and_then(|value| Path::new(value).file_name())
            .and_then(|value| value.to_str())
            == Some("tos-native-owner-command")
            && argv.get(1).map(String::as_str) == Some("foundation")
            && argv
                .windows(2)
                .any(|pair| pair[0] == "--invocation" && !pair[1].is_empty())
    }

    fn foundation_stage_ticket_fd(argv: &[String]) -> io::Result<Option<i32>> {
        if !is_native_foundation_command(argv) {
            return Ok(None);
        }
        let raw = std::env::var("ABYSS_STAGE_TICKET_FD")
            .map_err(|_| error("native foundation lane requires issuer ABYSS_STAGE_TICKET_FD"))?;
        let fd = raw
            .parse::<i32>()
            .ok()
            .filter(|fd| *fd > 2)
            .ok_or_else(|| error("invalid issuer ABYSS_STAGE_TICKET_FD"))?;
        if unsafe { libc::fcntl(fd, libc::F_GETFD) } < 0 {
            return Err(error("issuer ABYSS_STAGE_TICKET_FD is not open"));
        }
        Ok(Some(fd))
    }

    fn write(fd: i32, mut bytes: &[u8], deadline: Instant, cancel: &AtomicI32) -> io::Result<()> {
        while !bytes.is_empty() {
            cancelled(cancel)?;
            if Instant::now() >= deadline {
                return Err(error("execution wall deadline (output sink)"));
            }
            let count = unsafe { libc::write(fd, bytes.as_ptr().cast(), bytes.len()) };
            if count > 0 {
                bytes = &bytes[count as usize..];
                continue;
            }
            let err = io::Error::last_os_error();
            if count == 0 {
                return Err(error("output sink made no progress"));
            }
            if !matches!(
                err.kind(),
                io::ErrorKind::Interrupted | io::ErrorKind::WouldBlock
            ) {
                return Err(err);
            }
            thread::sleep(Duration::from_millis(2));
        }
        Ok(())
    }

    pub(super) fn run(
        root: &Path,
        plan: &Plan,
        limits: Limits,
        cancel: &AtomicI32,
        style: Style<'_>,
    ) -> io::Result<i32> {
        run_inner(root, plan, limits, cancel, style, None, None, &[])
    }

    pub(super) fn run_captured(
        root: &Path,
        plan: &Plan,
        limits: Limits,
        cancel: &AtomicI32,
        streams: &mut [Vec<u8>; 2],
    ) -> io::Result<i32> {
        run_inner(
            root,
            plan,
            limits,
            cancel,
            Style::Capture,
            Some(streams),
            None,
            &[],
        )
    }

    pub(super) fn run_captured_with_env(
        root: &Path,
        plan: &Plan,
        limits: Limits,
        cancel: &AtomicI32,
        streams: &mut [Vec<u8>; 2],
        overrides: &[(String, String)],
    ) -> io::Result<i32> {
        run_inner(
            root,
            plan,
            limits,
            cancel,
            Style::Capture,
            Some(streams),
            None,
            overrides,
        )
    }

    pub(super) fn run_product_until(
        root: &Path,
        plan: &Plan,
        limits: Limits,
        cancel: &AtomicI32,
        deadline: Instant,
    ) -> io::Result<i32> {
        run_inner(
            root,
            plan,
            limits,
            cancel,
            Style::Capture,
            None,
            Some(deadline),
            &[],
        )
    }

    fn run_inner(
        root: &Path,
        plan: &Plan,
        limits: Limits,
        cancel: &AtomicI32,
        style: Style<'_>,
        mut streams: Option<&mut [Vec<u8>; 2]>,
        original_deadline: Option<Instant>,
        child_overrides: &[(String, String)],
    ) -> io::Result<i32> {
        cancelled(cancel)?;
        if original_deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            return Err(error("execution wall deadline before setup"));
        }
        if limits.command_wall.is_zero()
            || limits.command_wall > Duration::from_secs(3600)
            || limits.lane_wall.is_zero()
            || limits.lane_wall > Duration::from_secs(86400)
            || limits.cleanup_grace.is_zero()
            || limits.cleanup_grace > Duration::from_secs(2)
            || limits.output_bytes == 0
            || limits.output_bytes > 64 * 1024 * 1024
        {
            return Err(error("invalid execution limits"));
        }
        if fs::read_dir("/proc/self/task")?.count() != 1
            || visit_children(
                original_deadline.map_or(Instant::now() + limits.cleanup_grace, |deadline| {
                    deadline.min(Instant::now() + limits.cleanup_grace)
                }),
                |_| {},
            )? != 0
        {
            return Err(error(
                "executor requires a dedicated single-threaded process without existing children",
            ));
        }
        if !root.is_absolute() || fs::canonicalize(root)? != root {
            return Err(error("execution root must be absolute without symlinks"));
        }
        if plan.commands.is_empty()
            || plan.commands.len() > 4096
            || plan
                .commands
                .iter()
                .any(|c| c.argv.is_empty() || c.argv.iter().any(|a| a.contains('\0')))
        {
            return Err(error("invalid command plan"));
        }
        if unsafe { libc::prctl(libc::PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0) } != 0 {
            return Err(io::Error::last_os_error());
        }
        // Probe pidfd before any tool runs. No weaker custody fallback.
        let self_identity = pidfd(std::process::id() as i32)?;
        signal(&self_identity, 0)?;
        drop(self_identity);
        let mut stdout_mode = Nonblocking::new(1)?;
        let mut stderr_mode = match Nonblocking::new(2) {
            Ok(mode) => mode,
            Err(error) => {
                if original_deadline.is_some() {
                    stdout_mode.restore()?;
                }
                return Err(error);
            }
        };
        let result = (|| -> io::Result<i32> {
            let lane_deadline = original_deadline
                .map_or(Instant::now() + limits.lane_wall, |deadline| {
                    deadline.min(Instant::now() + limits.lane_wall)
                });
            cancelled(cancel)?;
            if Instant::now() >= lane_deadline {
                return Err(error("execution wall deadline during setup"));
            }
            let mut products: Option<crate::conformance_products::Products> = None;
            if let Style::Validation(_, timeouts) = style {
                if timeouts.len() != plan.commands.len() {
                    return Err(error("validation timeout/command count differs"));
                }
            }
            for (index, command) in plan.commands.iter().enumerate() {
                let explicit_timeout = match style {
                    Style::Validation(_, timeouts) => timeouts[index],
                    _ => None,
                };
                let command_wall = explicit_timeout.unwrap_or(limits.command_wall);
                let deadline = lane_deadline.min(Instant::now() + command_wall);
                let progress = match style {
                    Style::Capture => String::new(),
                    Style::Mechanics => format!("[mechanics-local] {}\n", command.argv.join(" ")),
                    Style::Validation(_, _) => {
                        format!("[run] {}: {}\n", command.home, command.argv.join(" "))
                    }
                    Style::Release => {
                        format!("[run] {}: {}\n", command.home, list2cmdline(&command.argv))
                    }
                };
                write(1, progress.as_bytes(), deadline, cancel)?;
                if let Some(timeout) = explicit_timeout {
                    write(
                        1,
                        format!(
                            "[budget] {}: command_timeout_ms={} lane_wall_cap_ms={}\n",
                            command.home,
                            timeout.as_millis(),
                            limits.lane_wall.as_millis()
                        )
                        .as_bytes(),
                        deadline,
                        cancel,
                    )?;
                }
                let preparing = matches!(style, Style::Validation(_, _))
                    && crate::conformance_products::preparation(&command.argv);
                let growth_class = matches!(style, Style::Validation(_, _))
                    && command
                        .argv
                        .first()
                        .is_some_and(|arg| arg == crate::growth_native_plan::NATIVE_CLASS);
                let argv = if growth_class {
                    products
                        .as_ref()
                        .ok_or_else(|| {
                            error("native Growth requires current-lane Cargo product preparation")
                        })?
                        .growth_command(&command.argv, deadline, cancel)?
                } else {
                    command.argv.clone()
                };
                let overrides = if matches!(style, Style::Validation(_, _))
                    && (crate::conformance_products::execution(&command.argv) || growth_class)
                {
                    products.as_ref().ok_or_else(|| error("workspace conformance requires successful current-lane Cargo artifact preparation"))?.environment(deadline, cancel)?
                } else {
                    Vec::new()
                };
                let mut overrides = overrides;
                overrides.extend_from_slice(child_overrides);
                if let Style::Validation(python, _) = style {
                    if crate::conformance_products::execution(&command.argv) || growth_class {
                        if python.contains('\0') {
                            return Err(error("invalid explicit maintained Python interpreter"));
                        }
                        if !python.is_empty() {
                            overrides.push(("TOS_MAINTAINED_PYTHON".into(), python.into()));
                        }
                    }
                }
                let inherited_stage_ticket_fd = match style {
                    Style::Validation(_, _) => foundation_stage_ticket_fd(&argv)?,
                    _ => None,
                };
                let mut cargo_stdout = Vec::new();
                let (mut custody, stdout, stderr) = spawn(
                    root,
                    &argv,
                    limits.cleanup_grace,
                    &overrides,
                    inherited_stage_ticket_fd,
                )?;
                let _out_mode = Nonblocking::new(stdout.as_raw_fd())?;
                let _err_mode = Nonblocking::new(stderr.as_raw_fd())?;
                let mut eof = [false, false];
                let mut status = None;
                let mut output_bytes = 0usize;
                let execution = (|| -> io::Result<()> {
                    loop {
                        cancelled(cancel)?;
                        if Instant::now() >= deadline {
                            return Err(error("execution wall deadline"));
                        }
                        if status.is_none() {
                            status = custody.poll_exit()?;
                            if status.is_some() {
                                custody.reaped = true;
                                // A successful daemonizing tool may still have
                                // descendants without open output. Own them too.
                                custody.cleanup()?;
                            }
                        }
                        for (index, (source, sink)) in
                            [(stdout.as_raw_fd(), 1), (stderr.as_raw_fd(), 2)]
                                .into_iter()
                                .enumerate()
                        {
                            if eof[index] {
                                continue;
                            }
                            let mut buffer = [0u8; 8192];
                            let count = unsafe {
                                libc::read(source, buffer.as_mut_ptr().cast(), buffer.len())
                            };
                            if count == 0 {
                                eof[index] = true;
                            } else if count > 0 {
                                output_bytes += count as usize;
                                if output_bytes > limits.output_bytes {
                                    return Err(error("combined child output byte limit exceeded"));
                                }
                                if (preparing || growth_class) && index == 0 {
                                    cargo_stdout.extend_from_slice(&buffer[..count as usize]);
                                }
                                if let Some(streams) = streams.as_deref_mut() {
                                    streams[index].extend_from_slice(&buffer[..count as usize]);
                                } else {
                                    write(sink, &buffer[..count as usize], deadline, cancel)?;
                                }
                            } else {
                                let err = io::Error::last_os_error();
                                if !matches!(
                                    err.kind(),
                                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                                ) {
                                    return Err(err);
                                }
                            }
                        }
                        if status.is_some() && eof.iter().all(|v| *v) {
                            return Ok(());
                        }
                        thread::sleep(Duration::from_millis(2));
                    }
                })();
                // Cleanup outcome overrides a normal return: residual custody is
                // never reported as successful lane completion.
                custody.cleanup()?;
                execution?;
                if status.unwrap().success() && growth_class {
                    crate::growth_native_plan::verify_result(&command.argv, &cargo_stdout)?;
                }
                if status.unwrap().success() && preparing {
                    products = Some(crate::conformance_products::Products::select(
                        &cargo_stdout,
                        deadline,
                        cancel,
                    )?);
                }
                if !status.unwrap().success() {
                    let failure = match style {
                        Style::Capture => String::new(),
                        Style::Mechanics => format!(
                            "[error] mechanics-local command failed: {} ({})\n",
                            command.argv.join(" "),
                            status.unwrap()
                        ),
                        Style::Validation(_, _) => format!(
                            "[error] {} failed with exit code {}\n",
                            command.home,
                            status
                                .unwrap()
                                .code()
                                .unwrap_or(-status.unwrap().signal().unwrap_or(0))
                        ),
                        Style::Release => format!(
                            "[error] {} failed with exit code {}\n",
                            command.home,
                            status
                                .unwrap()
                                .code()
                                .unwrap_or(-status.unwrap().signal().unwrap_or(0))
                        ),
                    };
                    write(
                        if matches!(style, Style::Release) {
                            1
                        } else {
                            2
                        },
                        failure.as_bytes(),
                        lane_deadline,
                        cancel,
                    )?;
                    return Ok(match style {
                        Style::Mechanics => 1,
                        Style::Validation(_, _) | Style::Release | Style::Capture => {
                            status.unwrap().code().unwrap_or_else(|| {
                                // sys.exit(-signal) from the Python compatibility entry
                                // is observed by its parent as 256-signal on Unix.
                                256 - status.unwrap().signal().unwrap_or(0)
                            })
                        }
                    });
                }
                if matches!(style, Style::Validation(_, _)) {
                    write(
                        1,
                        format!("[ok] {}\n", command.home).as_bytes(),
                        deadline,
                        cancel,
                    )?;
                }
            }
            if matches!(style, Style::Mechanics) {
                let summary = if plan
                    .commands
                    .iter()
                    .all(|command| command.kind == "native_assertions")
                {
                    format!(
                        "[ok] completed native mechanics assertions across {} supported homes; Growth Cycle reference cohort not run\n",
                        plan.commands.len()
                    )
                } else if plan
                    .commands
                    .iter()
                    .any(|command| command.kind == "native_assertions")
                {
                    let native = plan
                        .commands
                        .iter()
                        .filter(|command| command.kind == "native_assertions")
                        .count();
                    let reference = plan
                        .commands
                        .iter()
                        .filter(|command| command.kind == "unittest")
                        .count();
                    format!(
                        "[ok] completed mechanics-local native assertions across {native} homes, reference unittest across {reference} homes, builders and validators; {} reference test files discovered\n",
                        plan.test_file_count
                    )
                } else {
                    format!(
                        "[ok] completed mechanics-local unittest, builder, and validator coverage across {} test files\n",
                        plan.test_file_count
                    )
                };
                write(1, summary.as_bytes(), lane_deadline, cancel)?;
            }
            Ok(0)
        })();
        if original_deadline.is_some() {
            // Restore both descriptions even if the first restoration fails;
            // a product refusal must not leave shared stdout/stderr modes set.
            // Reverse acquisition order is required when `2>&1` aliases
            // stdout and stderr to the same open-file description.
            let stderr_result = stderr_mode.restore();
            let stdout_result = stdout_mode.restore();
            stdout_result?;
            stderr_result?;
        }
        result
    }
    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn private_stage_ticket_is_forwarded_only_to_selected_foundation_command() {
            assert!(is_native_foundation_command(&[
                "/opt/tos/bin/tos-native-owner-command".into(),
                "foundation".into(),
                "--repo-root".into(),
                "/source/Tree-of-Sophia".into(),
                "--invocation".into(),
                "/run/owner/foundation.json".into(),
            ]));
            assert!(!is_native_foundation_command(&[
                "tos-native-owner-command".into(),
                "source-commands".into(),
                "--invocation".into(),
                "/run/owner/commands.json".into(),
            ]));
            assert!(!is_native_foundation_command(&[
                "tos-native-owner-command".into(),
                "foundation".into(),
                "--help".into(),
            ]));
        }

        #[test]
        fn aliased_output_descriptions_restore_original_flags() {
            let (_reader, writer) = pipe().unwrap();
            let fd = unsafe { libc::dup(writer.as_raw_fd()) };
            assert!(fd >= 0);
            let alias = unsafe { File::from_raw_fd(fd) };
            let original = unsafe { libc::fcntl(writer.as_raw_fd(), libc::F_GETFL) };
            let mut stdout_mode = Nonblocking::new(writer.as_raw_fd()).unwrap();
            let mut stderr_mode = Nonblocking::new(alias.as_raw_fd()).unwrap();
            stderr_mode.restore().unwrap();
            stdout_mode.restore().unwrap();
            assert_eq!(
                unsafe { libc::fcntl(alias.as_raw_fd(), libc::F_GETFL) },
                original
            );
        }

        #[test]
        fn philosophy_original_deadline_refuses_before_worker_setup() {
            let cancel = AtomicI32::new(0);
            let error = crate::executor::run_philosophy_product(
                vec!["must-not-be-selected-or-spawned".into()],
                Limits::default(),
                &cancel,
                Instant::now() - Duration::from_secs(1),
            )
            .unwrap_err();
            assert!(error.to_string().contains("deadline before setup"));
        }

        #[test]
        fn descendant_stream_visits_beyond_old_count_and_byte_caps() {
            // Synthetic kernel-list bytes, not a live fork storm. Values cross
            // both old4096-entry and64KiB refusal boundaries; buffer boundaries
            // may split a PID. Every earlier PID remains immediately available
            // to cleanup while later entries are still being read.
            let input = (10_000_000..10_010_000)
                .map(|pid| format!("{pid} "))
                .collect::<String>();
            let mut visited = Vec::new();
            let count = visit_pid_list(
                input.as_bytes(),
                Instant::now() + Duration::from_secs(1),
                |pid| visited.push(pid),
            )
            .unwrap();
            assert_eq!(count, 10_000);
            assert_eq!(visited, (10_000_000..10_010_000).collect::<Vec<_>>());
        }
    }
}
