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
    steps: &[(String, Vec<String>)],
    limits: Limits,
    cancel: &AtomicI32,
) -> io::Result<i32> {
    let plan = selected_plan(
        steps,
        "tos_validation_lanes_selected_v1",
        "validation_lane_step",
    );
    #[cfg(target_os = "linux")]
    {
        native::run(root, &plan, limits, cancel, native::Style::Validation)
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
    pub(super) enum Style {
        Mechanics,
        Validation,
        Release,
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

    fn spawn(root: &Path, argv: &[String], grace: Duration) -> io::Result<(Custody, File, File)> {
        let root = CString::new(root.as_os_str().as_bytes()).map_err(|_| error("NUL root"))?;
        let args: Result<Vec<CString>, _> =
            argv.iter().map(|a| CString::new(a.as_bytes())).collect();
        let args = args.map_err(|_| error("NUL argv"))?;
        let mut pointers: Vec<*const libc::c_char> = args.iter().map(|a| a.as_ptr()).collect();
        pointers.push(std::ptr::null());
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
                libc::execvp(pointers[0], pointers.as_ptr());
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
    }
    impl Nonblocking {
        fn new(fd: i32) -> io::Result<Self> {
            let previous = unsafe { libc::fcntl(fd, libc::F_GETFL) };
            if previous < 0
                || unsafe { libc::fcntl(fd, libc::F_SETFL, previous | libc::O_NONBLOCK) } < 0
            {
                return Err(io::Error::last_os_error());
            }
            Ok(Self { fd, previous })
        }
    }
    impl Drop for Nonblocking {
        fn drop(&mut self) {
            unsafe {
                libc::fcntl(self.fd, libc::F_SETFL, self.previous);
            }
        }
    }

    fn cancelled(cancel: &AtomicI32) -> io::Result<()> {
        if cancel.load(Ordering::Relaxed) != 0 {
            Err(error("execution cancelled"))
        } else {
            Ok(())
        }
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
        style: Style,
    ) -> io::Result<i32> {
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
            || visit_children(Instant::now() + limits.cleanup_grace, |_| {})? != 0
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
        let _stdout_mode = Nonblocking::new(1)?;
        let _stderr_mode = Nonblocking::new(2)?;
        let lane_deadline = Instant::now() + limits.lane_wall;
        for command in &plan.commands {
            let deadline = lane_deadline.min(Instant::now() + limits.command_wall);
            let progress = match style {
                Style::Mechanics => format!("[mechanics-local] {}\n", command.argv.join(" ")),
                Style::Validation => {
                    format!("[run] {}: {}\n", command.home, command.argv.join(" "))
                }
                Style::Release => {
                    format!("[run] {}: {}\n", command.home, list2cmdline(&command.argv))
                }
            };
            write(1, progress.as_bytes(), deadline, cancel)?;
            let (mut custody, stdout, stderr) = spawn(root, &command.argv, limits.cleanup_grace)?;
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
                        let count =
                            unsafe { libc::read(source, buffer.as_mut_ptr().cast(), buffer.len()) };
                        if count == 0 {
                            eof[index] = true;
                        } else if count > 0 {
                            output_bytes += count as usize;
                            if output_bytes > limits.output_bytes {
                                return Err(error("combined child output byte limit exceeded"));
                            }
                            write(sink, &buffer[..count as usize], deadline, cancel)?;
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
            if !status.unwrap().success() {
                let failure = match style {
                    Style::Mechanics => format!(
                        "[error] mechanics-local command failed: {} ({})\n",
                        command.argv.join(" "),
                        status.unwrap()
                    ),
                    Style::Validation => format!(
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
                    Style::Validation | Style::Release => {
                        status.unwrap().code().unwrap_or_else(|| {
                            // sys.exit(-signal) from the Python compatibility entry
                            // is observed by its parent as 256-signal on Unix.
                            256 - status.unwrap().signal().unwrap_or(0)
                        })
                    }
                });
            }
            if matches!(style, Style::Validation) {
                write(
                    1,
                    format!("[ok] {}\n", command.home).as_bytes(),
                    deadline,
                    cancel,
                )?;
            }
        }
        if matches!(style, Style::Mechanics) {
            write(1, format!("[ok] completed mechanics-local unittest, builder, and validator coverage across {} test files\n", plan.test_file_count).as_bytes(), lane_deadline, cancel)?;
        }
        Ok(0)
    }
    #[cfg(test)]
    mod tests {
        use super::*;

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
