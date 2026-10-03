use std::{
    fs,
    io::{BufReader, Read},
    process::{Child, Command, Output, Stdio},
    time::{Duration, Instant},
};
use tos_foundation::{Digest256, Digest256Hasher};
// The admitted controller owns image digest, held-FD and pre/post custody.
// An explicit override avoids mutable Cargo target origins in installed runs.
pub(super) fn selected_test_binary(embedded: &str) -> std::path::PathBuf {
    use std::os::unix::{ffi::OsStrExt, fs::PermissionsExt};
    let Some(value) = std::env::var_os("TOS_NATIVE_ACCESS_TEST_BINARY") else {
        return embedded.into();
    };
    let bytes = value.as_bytes();
    assert!(
        !bytes.is_empty() && bytes.len() <= 4096 && !bytes.contains(&0),
        "invalid explicit native test binary path"
    );
    let path = std::path::PathBuf::from(value);
    assert!(
        path.is_absolute()
            && path.components().all(|component| matches!(
                component,
                std::path::Component::RootDir | std::path::Component::Normal(_)
            )),
        "explicit native test binary must be an absolute normalized path"
    );
    let file = tos_fd_open::open_absolute_regular(&path, 512 * 1024 * 1024)
        .expect("explicit native test binary must be a bounded nofollow regular file");
    assert!(
        file.metadata().unwrap().permissions().mode() & 0o111 != 0,
        "explicit native test binary is not executable"
    );
    path
}
// Reap every owned child even if a later packet/custody assertion unwinds.
// These programs do not spawn a service/process tree of their own.
pub(super) struct OwnedChild(pub(super) Child);
impl std::ops::Deref for OwnedChild {
    type Target = Child;
    fn deref(&self) -> &Child {
        &self.0
    }
}
impl std::ops::DerefMut for OwnedChild {
    fn deref_mut(&mut self) -> &mut Child {
        &mut self.0
    }
}
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
// Opt-in failure evidence only. The existing generic reader path stays unchanged.
type DiagnosticPipes = [std::sync::Arc<std::sync::Mutex<Vec<u8>>>; 2];
struct DiagnosticPrefix(Option<Vec<u8>>);
impl std::fmt::Display for DiagnosticPrefix {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use std::fmt::Write;
        match &self.0 {
            None => f.write_str("unavailable"),
            Some(bytes) => {
                write!(
                    f,
                    "captured-prefix-bytes={} total-stream-length=unknown prefix=",
                    bytes.len()
                )?;
                // Stream escaping directly into the panic payload avoids a second
                // UTF8-lossy string allocation; every retained byte stays visible.
                for byte in bytes.escape_ascii() {
                    f.write_char(char::from(byte))?;
                }
                Ok(())
            }
        }
    }
}
#[track_caller]
fn diagnostic_refusal(
    child: &mut OwnedChild,
    phase: &str,
    trigger: &str,
    detail: &str,
    captures: &DiagnosticPipes,
) -> ! {
    let observed_before_kill = child.try_wait();
    let kill_result = child.kill();
    let post_kill_reap = child.wait();
    // Never add an EOF wait or a new deadline window. A busy/unavailable capture
    // is explicitly unknown; a zero prefix does not assert an empty stream.
    let stdout = DiagnosticPrefix(captures[0].try_lock().ok().map(|b| b.clone()));
    let stderr = DiagnosticPrefix(captures[1].try_lock().ok().map(|b| b.clone()));
    panic!(
        "native child refusal phase={phase} trigger={trigger} detail={detail} observed_before_kill={observed_before_kill:?} original_signal=unknown_unless_observed_before_kill kill_result={kill_result:?} post_kill_reap={post_kill_reap:?} stdout=[{stdout}] stderr=[{stderr}]"
    );
}
#[track_caller]
fn bounded_child_output_until(
    mut child: OwnedChild,
    stdout_max: usize,
    deadline: Instant,
    diagnostic: Option<&str>,
) -> Output {
    use std::sync::mpsc;
    const STDERR_MAX: usize = 16 * 1024;
    let captures: Option<DiagnosticPipes> = diagnostic.map(|_| {
        std::array::from_fn(|kind| {
            std::sync::Arc::new(std::sync::Mutex::new(Vec::with_capacity(if kind == 0 {
                stdout_max + 1
            } else {
                STDERR_MAX + 1
            })))
        })
    });
    let (tx, rx) = mpsc::channel();
    for (kind, pipe, max) in [
        (
            0,
            Box::new(child.stdout.take().unwrap()) as Box<dyn Read + Send>,
            stdout_max,
        ),
        (
            1,
            Box::new(child.stderr.take().unwrap()) as Box<dyn Read + Send>,
            STDERR_MAX,
        ),
    ] {
        let tx = tx.clone();
        let capture = captures.as_ref().map(|pipes| pipes[kind].clone());
        std::thread::spawn(move || {
            let mut bytes = if capture.is_some() {
                Vec::with_capacity(max + 1)
            } else {
                Vec::new()
            };
            let result = if let Some(capture) = capture {
                let mut limited = pipe.take((max + 1) as u64);
                let mut chunk = [0u8; 1024];
                (|| -> std::io::Result<usize> {
                    loop {
                        let count = match limited.read(&mut chunk) {
                            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {
                                continue;
                            }
                            Err(error) => return Err(error),
                            Ok(count) => count,
                        };
                        if count == 0 {
                            return Ok(bytes.len());
                        }
                        bytes.extend_from_slice(&chunk[..count]);
                        if let Ok(mut prefix) = capture.lock() {
                            prefix.extend_from_slice(&chunk[..count]);
                        }
                    }
                })()
            } else {
                pipe.take((max + 1) as u64).read_to_end(&mut bytes)
            };
            let _ = tx.send((kind, result, bytes));
        });
    }
    drop(tx);
    let mut stdout = None;
    let mut stderr = None;
    loop {
        if Instant::now() >= deadline {
            if let (Some(phase), Some(captures)) = (diagnostic, &captures) {
                diagnostic_refusal(
                    &mut child,
                    phase,
                    "absolute-deadline",
                    "deadline reached",
                    captures,
                );
            }
            let _ = child.kill();
            let _ = child.wait();
            panic!("native child exceeded its absolute deadline");
        }
        while let Ok((kind, result, bytes)) = rx.try_recv() {
            if let (Err(error), Some(phase), Some(captures)) = (&result, diagnostic, &captures) {
                diagnostic_refusal(
                    &mut child,
                    phase,
                    "pipe-read-error",
                    &error.to_string(),
                    captures,
                );
            }
            result.unwrap();
            let max = if kind == 0 { stdout_max } else { STDERR_MAX };
            if bytes.len() > max {
                if let (Some(phase), Some(captures)) = (diagnostic, &captures) {
                    let detail = if kind == 0 {
                        "stdout exceeded configured cap"
                    } else {
                        "stderr exceeded 16384-byte cap"
                    };
                    diagnostic_refusal(&mut child, phase, "output-cap", detail, captures);
                }
                let _ = child.kill();
                let _ = child.wait();
                panic!("native child exceeded {max}-byte output cap");
            }
            if kind == 0 {
                stdout = Some(bytes);
            } else {
                stderr = Some(bytes);
            }
        }
        let observed_status = child.try_wait();
        if let (Err(error), Some(phase), Some(captures)) = (&observed_status, diagnostic, &captures)
        {
            diagnostic_refusal(
                &mut child,
                phase,
                "child-status-error",
                &error.to_string(),
                captures,
            );
        }
        if let Some(status) = observed_status.unwrap() {
            if stdout.is_some() && stderr.is_some() {
                return Output {
                    status,
                    stdout: stdout.take().unwrap(),
                    stderr: stderr.take().unwrap(),
                };
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
#[track_caller]
pub(super) fn bounded_child_output(child: OwnedChild, stdout_max: usize) -> Output {
    bounded_child_output_until(
        child,
        stdout_max,
        Instant::now() + Duration::from_secs(60),
        None,
    )
}
#[track_caller]
pub(super) fn bounded_output(command: &mut Command, stdout_max: usize) -> Output {
    bounded_output_until(command, stdout_max, Duration::from_secs(60))
}
#[track_caller]
pub(super) fn bounded_output_until(
    command: &mut Command,
    stdout_max: usize,
    timeout: Duration,
) -> Output {
    bounded_output_before(command, stdout_max, Instant::now() + timeout)
}
#[track_caller]
pub(super) fn bounded_output_before(
    command: &mut Command,
    stdout_max: usize,
    deadline: Instant,
) -> Output {
    assert!(
        Instant::now() < deadline,
        "native child deadline already elapsed"
    );
    let child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    bounded_child_output_until(OwnedChild(child), stdout_max, deadline, None)
}
/// Only the selected factory opts into bounded partial failure evidence.
#[track_caller]
pub(super) fn bounded_output_before_diagnostic(
    command: &mut Command,
    stdout_max: usize,
    deadline: Instant,
    phase: &str,
) -> Output {
    assert!(
        Instant::now() < deadline,
        "native child refusal phase={phase} trigger=deadline-before-spawn original_signal=unknown child=not-spawned output=unavailable"
    );
    let child = command.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()
        .unwrap_or_else(|error| panic!("native child refusal phase={phase} trigger=spawn-error error={error} original_signal=unknown child=not-spawned output=unavailable"));
    bounded_child_output_until(OwnedChild(child), stdout_max, deadline, Some(phase))
}
pub(super) fn bounded_sha(path: &std::path::Path, max: u64) -> Digest256 {
    hash_before(path, max, None)
}
pub(super) fn bounded_sha_before(path: &std::path::Path, max: u64, deadline: Instant) -> Digest256 {
    hash_before(path, max, Some(deadline))
}
fn hash_before(path: &std::path::Path, max: u64, deadline: Option<Instant>) -> Digest256 {
    let check = || {
        assert!(
            deadline.is_none_or(|end| Instant::now() < end),
            "native hash deadline elapsed"
        )
    };
    check();
    use std::os::unix::fs::MetadataExt;
    let file = tos_fd_open::open_absolute_regular(path, max).unwrap();
    let before = file.metadata().unwrap();
    let identity = |m: &fs::Metadata| {
        (
            m.dev(),
            m.ino(),
            m.len(),
            m.mtime(),
            m.mtime_nsec(),
            m.ctime(),
            m.ctime_nsec(),
        )
    };
    let mut reader = BufReader::new(file);
    let mut consumed = 0u64;
    let mut hasher = Digest256Hasher::new();
    let mut bytes = [0u8; 64 * 1024];
    loop {
        check();
        let read = reader.read(&mut bytes).unwrap();
        if read == 0 {
            break;
        }
        consumed = consumed.checked_add(read as u64).unwrap();
        assert!(consumed <= max, "hash input grew beyond byte cap");
        hasher.update(&bytes[..read]);
    }
    assert_eq!(consumed, before.len(), "hash input size changed");
    assert_eq!(
        identity(&before),
        identity(&reader.get_ref().metadata().unwrap()),
        "hash input changed while reading"
    );
    assert_eq!(
        identity(&before),
        identity(&fs::symlink_metadata(path).unwrap()),
        "hash pathname changed while reading"
    );
    check();
    hasher.finalize()
}
