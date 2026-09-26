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
        native::run(root, plan, limits, cancel)
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

#[cfg(target_os = "linux")]
mod native {
    use super::*;
    use std::fs::{self, File};
    use std::io::Read;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::process::CommandExt;
    use std::process::{Child, Command, Stdio};
    use std::thread;
    use std::time::Instant;

    const MAX_CHILDREN: usize = 4096;

    fn error(message: impl Into<String>) -> io::Error {
        io::Error::other(message.into())
    }

    fn children() -> io::Result<Vec<i32>> {
        // Dedicated supervisor has one thread. Bounded proc read protects the
        // cleanup path against an unexpectedly prolific trusted tool.
        let mut bytes = String::new();
        File::open(format!("/proc/self/task/{}/children", std::process::id()))?
            .take(65537)
            .read_to_string(&mut bytes)?;
        if bytes.len() > 65536 {
            return Err(error("descendant custody enumeration exceeded 64 KiB"));
        }
        let pids: Result<Vec<i32>, _> = bytes.split_whitespace().map(str::parse).collect();
        let pids = pids.map_err(|_| error("invalid descendant PID"))?;
        if pids.len() > MAX_CHILDREN {
            return Err(error("descendant custody exceeded 4096 direct children"));
        }
        Ok(pids)
    }

    fn pidfd(pid: i32) -> io::Result<File> {
        let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) } as i32;
        if fd < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(unsafe { File::from_raw_fd(fd) })
        }
    }

    fn kill(fd: &File) -> io::Result<()> {
        let rc = unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                fd.as_raw_fd(),
                libc::SIGKILL,
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
        child: Child,
        identity: File,
        reaped: bool,
        cleaned: bool,
        grace: Duration,
    }

    impl Custody {
        fn cleanup(&mut self) -> io::Result<()> {
            if self.cleaned {
                return Ok(());
            }
            self.cleaned = true;
            self.cleanup_once()
        }

        fn cleanup_once(&mut self) -> io::Result<()> {
            let deadline = Instant::now() + self.grace;
            // Do not signal a numeric PGID after its leader has been reaped.
            // pidfd is the root identity; adopted children cannot reuse their
            // PID until this sole parent reaps them.
            if !self.reaped {
                unsafe {
                    libc::kill(-(self.child.id() as i32), libc::SIGKILL);
                }
                kill(&self.identity)?;
            }
            loop {
                if !self.reaped && self.child.try_wait()?.is_some() {
                    self.reaped = true;
                }
                for pid in children()? {
                    if !self.reaped && pid == self.child.id() as i32 {
                        continue;
                    }
                    let identity = pidfd(pid)?;
                    kill(&identity)?;
                    let mut status = 0;
                    let rc = unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) };
                    if rc < 0 {
                        return Err(io::Error::last_os_error());
                    }
                }
                let remaining = children()?;
                if self.reaped && remaining.is_empty() {
                    return Ok(());
                }
                if Instant::now() >= deadline {
                    return Err(error(format!(
                        "cleanup deadline; residual child PIDs {remaining:?}"
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
        if fs::read_dir("/proc/self/task")?.count() != 1 || !children()?.is_empty() {
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
        drop(pidfd(std::process::id() as i32)?);
        let _stdout_mode = Nonblocking::new(1)?;
        let _stderr_mode = Nonblocking::new(2)?;
        let lane_deadline = Instant::now() + limits.lane_wall;
        for command in &plan.commands {
            let deadline = lane_deadline.min(Instant::now() + limits.command_wall);
            write(
                1,
                format!("[mechanics-local] {}\n", command.argv.join(" ")).as_bytes(),
                deadline,
                cancel,
            )?;
            let mut process = Command::new(&command.argv[0]);
            process
                .args(&command.argv[1..])
                .current_dir(root)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            let parent = std::process::id() as i32;
            unsafe {
                process.pre_exec(move || {
                    if libc::setpgid(0, 0) != 0
                        || libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) != 0
                        || libc::getppid() != parent
                    {
                        return Err(io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
            let child = process.spawn()?;
            let identity = match pidfd(child.id() as i32) {
                Ok(fd) => fd,
                Err(err) => {
                    // Root has not been reaped, so this numeric identity is
                    // still reserved. Fail closed and leave bounded PID evidence.
                    unsafe {
                        libc::kill(-(child.id() as i32), libc::SIGKILL);
                        libc::kill(child.id() as i32, libc::SIGKILL);
                    }
                    return Err(error(format!(
                        "pidfd custody unavailable for PID {}: {err}",
                        child.id()
                    )));
                }
            };
            let mut custody = Custody {
                child,
                identity,
                reaped: false,
                cleaned: false,
                grace: limits.cleanup_grace,
            };
            let stdout = custody.child.stdout.take().unwrap();
            let stderr = custody.child.stderr.take().unwrap();
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
                        status = custody.child.try_wait()?;
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
                write(
                    2,
                    format!(
                        "[error] mechanics-local command failed: {} ({})\n",
                        command.argv.join(" "),
                        status.unwrap()
                    )
                    .as_bytes(),
                    lane_deadline,
                    cancel,
                )?;
                return Ok(1);
            }
        }
        write(1, format!("[ok] completed mechanics-local unittest, builder, and validator coverage across {} test files\n", plan.test_file_count).as_bytes(), lane_deadline, cancel)?;
        Ok(0)
    }
}
