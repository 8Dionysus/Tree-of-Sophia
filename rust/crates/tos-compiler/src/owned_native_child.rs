//! Finite native tool invocation shared by exact Poppler/research consumers.
//! One owner polls nonblocking stdin and both output pipes; no detached reader
//! or writer thread can survive an error. Captured bytes confer no authority.
use std::{
    io::{Read, Write},
    os::{fd::AsRawFd, unix::process::CommandExt},
    process::{Child, Command, ExitStatus, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
pub type Result<T> = std::result::Result<T, String>;
#[derive(Clone, Copy, Debug)]
pub struct CaptureLimits {
    pub max_stdin_bytes: usize,
    pub max_stdout_bytes: usize,
    pub max_stderr_bytes: usize,
}
#[derive(Debug)]
pub struct CapturedChild {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}
struct Owner {
    child: Child,
    reaped: bool,
    cleanup_attempted: bool,
}
impl Owner {
    fn signal_group(&mut self) -> Result<()> {
        // The leader remains unreaped (waitid WNOWAIT on success), so this
        // freshly allocated process-group PID cannot have been reused.
        if unsafe { libc::kill(-(self.child.id() as libc::pid_t), libc::SIGKILL) } < 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::ESRCH) {
                return Err(format!("native child group kill: {error}"));
            }
        }
        Ok(())
    }
    fn finalize(&mut self) -> Result<ExitStatus> {
        self.cleanup_attempted = true;
        let signal = self.signal_group();
        let _ = self.child.kill();
        // Cleanup owns a finite one-second reserve even after execution expiry.
        // A kernel-uninterruptible task is reported unreaped, never hidden by
        // blocking wait or an unbounded/repeated destructor reaper.
        let cleanup_deadline = Instant::now() + Duration::from_secs(1);
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => {
                    self.reaped = true;
                    signal?;
                    return Ok(status);
                }
                Ok(None) => {}
                Err(error) => {
                    return Err(format!(
                        "native child reap refusal pid={}: {error}",
                        self.child.id()
                    ));
                }
            }
            if Instant::now() >= cleanup_deadline {
                return Err(format!(
                    "native child cleanup deadline; unreaped pid={} remains under invoking process/outer owner custody",
                    self.child.id()
                ));
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    fn exited_unreaped(&self) -> Result<bool> {
        let mut info = unsafe { std::mem::zeroed::<libc::siginfo_t>() };
        if unsafe {
            libc::waitid(
                libc::P_PID,
                self.child.id(),
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        } < 0
        {
            return Err(format!(
                "native child waitid: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(unsafe { info.si_pid() } != 0)
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        if !self.reaped && !self.cleanup_attempted {
            // Panic-only last resort: signal and one nonblocking reap attempt.
            // Normal returns always carry finalize's explicit cleanup outcome.
            let _ = self.signal_group();
            let _ = self.child.kill();
            let _ = self.child.try_wait();
        }
    }
}
fn nonblocking(fd: libc::c_int) -> Result<()> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    Ok(())
}
fn check(deadline: Instant, cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::Relaxed) {
        return Err("native child cancelled".into());
    }
    if Instant::now() >= deadline {
        return Err("native child whole deadline".into());
    }
    Ok(())
}
fn drain<R: Read>(
    stream: &mut Option<R>,
    out: &mut Vec<u8>,
    cap: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<bool> {
    let mut progress = false;
    let mut buffer = [0u8; 8192];
    for _ in 0..128 {
        check(deadline, cancelled)?;
        let Some(pipe) = stream.as_mut() else {
            break;
        };
        match pipe.read(&mut buffer) {
            Ok(0) => {
                *stream = None;
                progress = true;
                break;
            }
            Ok(count) => {
                if count > cap.saturating_sub(out.len()) {
                    return Err("native child output byte cap".into());
                }
                out.extend_from_slice(&buffer[..count]);
                progress = true;
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(format!("native child pipe read: {e}")),
        }
    }
    Ok(progress)
}
/// The deadline belongs to the caller's already-started whole operation; it is
/// never restarted for a tool. EOF and exact tool-status interpretation remain
/// explicit in the consumer, including Git's meaningful nonzero status.
pub fn capture(
    command: &mut Command,
    input: Option<&[u8]>,
    limits: CaptureLimits,
    deadline: Instant,
) -> Result<CapturedChild> {
    static NOT_CANCELLED: AtomicBool = AtomicBool::new(false);
    capture_with_cancel(command, input, limits, deadline, &NOT_CANCELLED)
}
pub fn capture_with_cancel(
    command: &mut Command,
    input: Option<&[u8]>,
    limits: CaptureLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<CapturedChild> {
    check(deadline, cancelled)?;
    let input = input.unwrap_or(&[]);
    if input.len() > limits.max_stdin_bytes {
        return Err("native child stdin byte cap".into());
    }
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    let child = command
        .spawn()
        .map_err(|e| format!("native child spawn: {e}"))?;
    let mut owner = Owner {
        child,
        reaped: false,
        cleanup_attempted: false,
    };
    let result = (|| {
        let mut stdin = owner.child.stdin.take();
        let mut stdout = owner.child.stdout.take();
        let mut stderr = owner.child.stderr.take();
        nonblocking(
            stdin
                .as_ref()
                .ok_or("native child stdin absent")?
                .as_raw_fd(),
        )?;
        nonblocking(
            stdout
                .as_ref()
                .ok_or("native child stdout absent")?
                .as_raw_fd(),
        )?;
        nonblocking(
            stderr
                .as_ref()
                .ok_or("native child stderr absent")?
                .as_raw_fd(),
        )?;
        let mut written = 0;
        let mut out = Vec::new();
        let mut err = Vec::new();
        loop {
            check(deadline, cancelled)?;
            let mut progress = false;
            if written == input.len() {
                stdin = None;
            } else if let Some(pipe) = stdin.as_mut() {
                let end = input.len().min(written.saturating_add(8192));
                match pipe.write(&input[written..end]) {
                    Ok(0) => return Err("native child stdin closed before complete input".into()),
                    Ok(count) => {
                        written += count;
                        progress = true;
                    }
                    Err(e)
                        if e.kind() == std::io::ErrorKind::WouldBlock
                            || e.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(e) => return Err(format!("native child stdin write: {e}")),
                }
            }
            progress |= drain(
                &mut stdout,
                &mut out,
                limits.max_stdout_bytes,
                deadline,
                cancelled,
            )?;
            progress |= drain(
                &mut stderr,
                &mut err,
                limits.max_stderr_bytes,
                deadline,
                cancelled,
            )?;
            if stdin.is_none() && stdout.is_none() && stderr.is_none() && owner.exited_unreaped()? {
                // Finalize the owned group before reaping its exited leader.
                let status = owner.finalize()?;
                check(deadline, cancelled)?;
                return Ok(CapturedChild {
                    status,
                    stdout: out,
                    stderr: err,
                });
            }
            if !progress {
                let remaining = deadline.saturating_duration_since(Instant::now());
                std::thread::sleep(remaining.min(Duration::from_millis(2)));
            }
        }
    })();
    match result {
        Ok(output) => Ok(output),
        Err(error) => {
            let cleanup = if owner.cleanup_attempted {
                Ok(())
            } else {
                owner.finalize().map(|_| ())
            };
            match cleanup {
                Ok(()) => Err(error),
                Err(cleanup) => Err(format!("{error}; {cleanup}")),
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stdout_and_stderr_caps_fail_before_accumulation() {
        let limits = CaptureLimits {
            max_stdin_bytes: 0,
            max_stdout_bytes: 3,
            max_stderr_bytes: 3,
        };
        let mut c = Command::new("/bin/sh");
        c.args(["-c", "printf abcd"]);
        assert!(
            capture(
                &mut c,
                None,
                limits,
                Instant::now() + Duration::from_secs(2)
            )
            .is_err()
        );
        let mut c = Command::new("/bin/sh");
        c.args(["-c", "printf abcd >&2"]);
        assert!(
            capture(
                &mut c,
                None,
                limits,
                Instant::now() + Duration::from_secs(2)
            )
            .is_err()
        );
    }
    #[test]
    fn stdin_and_two_output_streams_are_owned() {
        let limits = CaptureLimits {
            max_stdin_bytes: 5,
            max_stdout_bytes: 5,
            max_stderr_bytes: 4,
        };
        let mut c = Command::new("/bin/sh");
        c.args(["-c", "cat; printf warn >&2"]);
        let output = capture(
            &mut c,
            Some(b"exact"),
            limits,
            Instant::now() + Duration::from_secs(2),
        )
        .unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, b"exact");
        assert_eq!(output.stderr, b"warn");
    }
    #[test]
    fn inherited_expired_deadline_and_cancel_refuse_before_spawn() {
        let limits = CaptureLimits {
            max_stdin_bytes: 0,
            max_stdout_bytes: 0,
            max_stderr_bytes: 0,
        };
        let mut c = Command::new("/no-such-executable");
        assert!(
            capture(&mut c, None, limits, Instant::now())
                .unwrap_err()
                .contains("deadline")
        );
        let cancelled = AtomicBool::new(true);
        assert!(
            capture_with_cancel(
                &mut c,
                None,
                limits,
                Instant::now() + Duration::from_secs(1),
                &cancelled
            )
            .unwrap_err()
            .contains("cancelled")
        );
    }
    #[test]
    fn live_timeout_returns_after_owned_child_is_killed_and_reaped() {
        let limits = CaptureLimits {
            max_stdin_bytes: 0,
            max_stdout_bytes: 0,
            max_stderr_bytes: 0,
        };
        let mut c = Command::new("/bin/sh");
        c.args(["-c", "sleep 30"]);
        let start = Instant::now();
        assert!(
            capture(&mut c, None, limits, start + Duration::from_millis(25))
                .unwrap_err()
                .contains("deadline")
        );
        assert!(start.elapsed() < Duration::from_secs(2));
    }
}
