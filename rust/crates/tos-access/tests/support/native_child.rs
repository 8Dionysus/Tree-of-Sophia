use std::{
    fs,
    io::{BufReader, Read},
    process::{Child, Command, Output, Stdio},
    time::Duration,
};
use tos_foundation::{Digest256, Digest256Hasher};
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
fn bounded_child_output_until(
    mut child: OwnedChild,
    stdout_max: usize,
    timeout: Duration,
) -> Output {
    use std::sync::mpsc;
    use std::time::Instant;
    const STDERR_MAX: usize = 16 * 1024;
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
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let result = pipe.take((max + 1) as u64).read_to_end(&mut bytes);
            let _ = tx.send((kind, result, bytes));
        });
    }
    drop(tx);
    let deadline = Instant::now() + timeout;
    let mut stdout = None;
    let mut stderr = None;
    loop {
        while let Ok((kind, result, bytes)) = rx.try_recv() {
            result.unwrap();
            let max = if kind == 0 { stdout_max } else { STDERR_MAX };
            if bytes.len() > max {
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
        if let Some(status) = child.try_wait().unwrap() {
            if stdout.is_some() && stderr.is_some() {
                return Output {
                    status,
                    stdout: stdout.take().unwrap(),
                    stderr: stderr.take().unwrap(),
                };
            }
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("native child exceeded {timeout:?} deadline");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
pub(super) fn bounded_child_output(child: OwnedChild, stdout_max: usize) -> Output {
    bounded_child_output_until(child, stdout_max, Duration::from_secs(60))
}
pub(super) fn bounded_output(command: &mut Command, stdout_max: usize) -> Output {
    bounded_output_until(command, stdout_max, Duration::from_secs(60))
}
pub(super) fn bounded_output_until(
    command: &mut Command,
    stdout_max: usize,
    timeout: Duration,
) -> Output {
    let child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    bounded_child_output_until(OwnedChild(child), stdout_max, timeout)
}
pub(super) fn bounded_sha(path: &std::path::Path, max: u64) -> Digest256 {
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
    hasher.finalize()
}
