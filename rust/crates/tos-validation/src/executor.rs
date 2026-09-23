//! One-shot, fail-closed native boundary for the JSON Schema probe.
//!
//! This module does not create a validation trace or admission attestation.
//! The caller must pin the dedicated worker's exact ELF digest. Linux copies
//! that ELF to a sealed executable memfd before launching it, so a path change
//! after verification cannot change the worker image.

use std::path::PathBuf;
use std::time::Duration;

use tos_foundation::Digest256;

use crate::{FormatProfile, SchemaResource};

const REQUEST_MAGIC: &[u8; 8] = b"TOSV2RQ1";
const RESPONSE_MAGIC: &[u8; 8] = b"TOSV2RS1";
const MAX_URI_BYTES: usize = 4096;
const MAX_FRAME_BYTES: usize = 36 * 1024 * 1024;
const RESPONSE_BYTES: usize = 8 + 32 + 32 + 32 + 2;

#[derive(Debug, Clone)]
pub struct ExactWorkerIdentity {
    pub absolute_path: PathBuf,
    pub sha256: Digest256,
}

#[derive(Debug, Clone, Copy)]
pub struct ExecutorBudget {
    /// Deadline for image verification, fork, transfer and worker execution.
    pub execution_wall: Duration,
    /// Additional, caller-visible allowance for SIGKILL and WNOHANG reap.
    pub cleanup_grace: Duration,
    pub cpu_seconds: u64,
    pub address_space_bytes: u64,
}

impl ExecutorBudget {
    pub fn laboratory() -> Self {
        Self {
            execution_wall: Duration::from_secs(5),
            cleanup_grace: Duration::from_millis(200),
            cpu_seconds: 3,
            address_space_bytes: 1024 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionIdentity {
    pub worker_sha256: Digest256,
    pub request_sha256: Digest256,
    pub schema_set_sha256: Digest256,
    pub instance_sha256: Digest256,
    pub profile: FormatProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutorFailure {
    UnsupportedHost,
    WorkerIdentity,
    InputBudget,
    ResourceLimitUnknown,
    Spawn,
    Timeout,
    CpuLimit,
    CrashSignal(i32),
    CrashExit(i32),
    ReapPending(i32),
    Protocol,
    Backend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutorOutcome {
    SchemaValid(ExecutionIdentity),
    SchemaInvalid(ExecutionIdentity),
    InputRejected(ExecutionIdentity),
    Indeterminate {
        reason: ExecutorFailure,
        identity: Option<ExecutionIdentity>,
    },
}

fn unknown(reason: ExecutorFailure, identity: Option<ExecutionIdentity>) -> ExecutorOutcome {
    ExecutorOutcome::Indeterminate { reason, identity }
}

pub struct BoundedSchemaExecutor;

impl BoundedSchemaExecutor {
    pub fn evaluate(
        worker: &ExactWorkerIdentity,
        resources: &[SchemaResource],
        profile: FormatProfile,
        root_uri: &str,
        raw_instance: &[u8],
        budget: ExecutorBudget,
    ) -> ExecutorOutcome {
        #[cfg(all(target_os = "linux", target_pointer_width = "64"))]
        {
            native::evaluate(worker, resources, profile, root_uri, raw_instance, budget)
        }
        #[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
        {
            let _ = (worker, resources, profile, root_uri, raw_instance, budget);
            unknown(ExecutorFailure::UnsupportedHost, None)
        }
    }
}

/// Called only by the dedicated executable. Its process limits are imposed by
/// the parent before `exec`; a direct call to this function has no such limit.
#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
pub fn worker_once() -> std::io::Result<()> {
    native::worker_once()
}

#[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
pub fn worker_once() -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "bounded schema worker requires Linux process limits",
    ))
}

#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
mod native {
    use super::*;
    use std::collections::BTreeMap;
    use std::fs::{File, OpenOptions};
    use std::io::{self, Read, Write};
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
    use std::thread;
    use std::time::Instant;
    use tos_foundation::Digest256Hasher;

    // Linux UAPI MFD_EXEC. Requiring this flag fails closed on older kernels
    // or hosts that refuse executable anonymous files.
    const MFD_EXEC_FLAG: u32 = 0x0010;
    const MAX_WORKER_BYTES: u64 = 128 * 1024 * 1024;

    #[cfg(test)]
    thread_local! {
        static TEST_CHILD_STDOUT_INODE: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    }

    fn profile_byte(profile: FormatProfile) -> u8 {
        match profile {
            FormatProfile::LegacyPythonObserved20260923 => 1,
            FormatProfile::AssertedSourceCandidateV1 => 2,
        }
    }

    fn parse_profile(value: u8) -> Option<FormatProfile> {
        match value {
            1 => Some(FormatProfile::LegacyPythonObserved20260923),
            2 => Some(FormatProfile::AssertedSourceCandidateV1),
            _ => None,
        }
    }

    fn put_bytes(output: &mut Vec<u8>, value: &[u8]) -> Result<(), ExecutorFailure> {
        let size = u32::try_from(value.len()).map_err(|_| ExecutorFailure::InputBudget)?;
        output.extend_from_slice(&size.to_be_bytes());
        output.extend_from_slice(value);
        if output.len() > MAX_FRAME_BYTES {
            return Err(ExecutorFailure::InputBudget);
        }
        Ok(())
    }

    fn schema_set_digest(resources: &[SchemaResource]) -> Result<Digest256, ExecutorFailure> {
        let mut members = BTreeMap::new();
        for resource in resources {
            if members
                .insert(resource.uri.as_str(), Digest256::of_bytes(&resource.raw))
                .is_some()
            {
                return Err(ExecutorFailure::Backend);
            }
        }
        let mut digest = Digest256Hasher::new();
        digest.update(b"tos-schema-set-v1\0");
        for (uri, raw_digest) in members {
            digest.update(&(uri.len() as u64).to_be_bytes());
            digest.update(uri.as_bytes());
            digest.update(raw_digest.as_bytes());
        }
        Ok(digest.finalize())
    }

    fn make_request(
        resources: &[SchemaResource],
        profile: FormatProfile,
        root_uri: &str,
        raw_instance: &[u8],
    ) -> Result<(Vec<u8>, Digest256, Digest256, Digest256), ExecutorFailure> {
        if resources.len() > crate::SchemaBackendProbe::MAX_RESOURCES
            || raw_instance.len() > crate::SchemaBackendProbe::MAX_INSTANCE_BYTES
            || root_uri.len() > MAX_URI_BYTES
        {
            return Err(ExecutorFailure::InputBudget);
        }
        let mut total = 0usize;
        for resource in resources {
            total = total
                .checked_add(resource.raw.len())
                .ok_or(ExecutorFailure::InputBudget)?;
            if resource.uri.len() > MAX_URI_BYTES
                || resource.raw.len() > crate::SchemaBackendProbe::MAX_RESOURCE_BYTES
                || total > crate::SchemaBackendProbe::MAX_TOTAL_BYTES
            {
                return Err(ExecutorFailure::InputBudget);
            }
        }
        let schema_digest = schema_set_digest(resources)?;
        let instance_digest = Digest256::of_bytes(raw_instance);
        let mut nonce = [0u8; 16];
        File::open("/dev/urandom")
            .and_then(|mut random| random.read_exact(&mut nonce))
            .map_err(|_| ExecutorFailure::Spawn)?;
        let mut frame = Vec::with_capacity(total.min(MAX_FRAME_BYTES));
        frame.extend_from_slice(REQUEST_MAGIC);
        frame.extend_from_slice(&nonce);
        frame.push(profile_byte(profile));
        frame.extend_from_slice(&(resources.len() as u32).to_be_bytes());
        for resource in resources {
            put_bytes(&mut frame, resource.uri.as_bytes())?;
            put_bytes(&mut frame, &resource.raw)?;
        }
        put_bytes(&mut frame, root_uri.as_bytes())?;
        put_bytes(&mut frame, raw_instance)?;
        let request_digest = Digest256::of_bytes(&frame);
        Ok((frame, request_digest, schema_digest, instance_digest))
    }

    /// Snapshot the verified worker into an executable, sealed in-memory file.
    /// Hashing the *copy* removes the in-place mutation race of path+hash+exec.
    fn sealed_worker(worker: &ExactWorkerIdentity) -> Result<File, ExecutorFailure> {
        if !worker.absolute_path.is_absolute() {
            return Err(ExecutorFailure::WorkerIdentity);
        }
        let mut source = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&worker.absolute_path)
            .map_err(|_| ExecutorFailure::WorkerIdentity)?;
        let metadata = source
            .metadata()
            .map_err(|_| ExecutorFailure::WorkerIdentity)?;
        if !metadata.file_type().is_file()
            || metadata.permissions().mode() & 0o111 == 0
            || metadata.len() == 0
            || metadata.len() > MAX_WORKER_BYTES
        {
            return Err(ExecutorFailure::WorkerIdentity);
        }
        let fd = unsafe {
            libc::memfd_create(
                c"tos-schema-worker".as_ptr(),
                libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING | MFD_EXEC_FLAG,
            )
        };
        if fd < 0 {
            return Err(ExecutorFailure::WorkerIdentity);
        }
        let mut sealed = unsafe { File::from_raw_fd(fd) };
        let mut digest = Digest256Hasher::new();
        let mut copied = 0u64;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let count = source
                .read(&mut buffer)
                .map_err(|_| ExecutorFailure::WorkerIdentity)?;
            if count == 0 {
                break;
            }
            copied = copied
                .checked_add(count as u64)
                .ok_or(ExecutorFailure::WorkerIdentity)?;
            if copied > MAX_WORKER_BYTES {
                return Err(ExecutorFailure::WorkerIdentity);
            }
            digest.update(&buffer[..count]);
            sealed
                .write_all(&buffer[..count])
                .map_err(|_| ExecutorFailure::WorkerIdentity)?;
        }
        if copied != metadata.len() || digest.finalize() != worker.sha256 {
            return Err(ExecutorFailure::WorkerIdentity);
        }
        let seals =
            libc::F_SEAL_WRITE | libc::F_SEAL_GROW | libc::F_SEAL_SHRINK | libc::F_SEAL_SEAL;
        if unsafe { libc::fcntl(sealed.as_raw_fd(), libc::F_ADD_SEALS, seals) } != 0 {
            return Err(ExecutorFailure::WorkerIdentity);
        }
        let after = source
            .metadata()
            .map_err(|_| ExecutorFailure::WorkerIdentity)?;
        if metadata.dev() != after.dev() || metadata.ino() != after.ino() {
            return Err(ExecutorFailure::WorkerIdentity);
        }
        Ok(sealed)
    }

    // After fork, use only libc calls until exec. No Rust allocator, lock, or
    // destructor may run in a possibly multithreaded parent process's child.
    unsafe fn child_exec(
        worker_fd: i32,
        input_fd: i32,
        output_fd: i32,
        null_fd: i32,
        budget: ExecutorBudget,
        parent_pid: libc::pid_t,
        argv: *const *mut libc::c_char,
    ) -> ! {
        let as_limit = libc::rlimit {
            rlim_cur: budget.address_space_bytes,
            rlim_max: budget.address_space_bytes,
        };
        let cpu_limit = libc::rlimit {
            rlim_cur: budget.cpu_seconds.saturating_sub(1).max(1),
            rlim_max: budget.cpu_seconds,
        };
        if unsafe { libc::setpgid(0, 0) } != 0
            || unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) } != 0
            || unsafe { libc::getppid() } != parent_pid
            || unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0
            || unsafe { libc::setrlimit(libc::RLIMIT_AS, &as_limit) } != 0
            || unsafe { libc::setrlimit(libc::RLIMIT_CPU, &cpu_limit) } != 0
            || unsafe { libc::chdir(c"/".as_ptr()) } != 0
            || unsafe { libc::dup2(input_fd, 0) } < 0
            || unsafe { libc::dup2(output_fd, 1) } < 0
            || unsafe { libc::dup2(null_fd, 2) } < 0
        {
            unsafe { libc::_exit(126) };
        }
        // The executable fd remains usable for execveat; all ambient fds
        // (including socket peers and the memfd) close on successful exec.
        const CLOSE_RANGE_CLOEXEC_FLAG: libc::c_int = 4;
        if unsafe { libc::close_range(3, u32::MAX, CLOSE_RANGE_CLOEXEC_FLAG) } != 0 {
            unsafe { libc::_exit(126) };
        }
        let env: [*mut libc::c_char; 1] = [std::ptr::null_mut()];
        unsafe {
            libc::execveat(
                worker_fd,
                c"".as_ptr(),
                argv,
                env.as_ptr(),
                libc::AT_EMPTY_PATH,
            );
            libc::_exit(126)
        }
    }

    fn socket_pair() -> io::Result<(File, File)> {
        let mut fds = [-1, -1];
        if unsafe {
            libc::socketpair(
                libc::AF_UNIX,
                libc::SOCK_STREAM | libc::SOCK_CLOEXEC,
                0,
                fds.as_mut_ptr(),
            )
        } != 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(unsafe { (File::from_raw_fd(fds[0]), File::from_raw_fd(fds[1])) })
    }

    fn poll_exit(pid: i32, status: &mut Option<i32>) -> Result<(), ExecutorFailure> {
        if status.is_some() {
            return Ok(());
        }
        let mut raw = 0;
        let observed = unsafe { libc::waitpid(pid, &mut raw, libc::WNOHANG) };
        if observed == pid {
            *status = Some(raw);
            Ok(())
        } else if observed == 0 {
            Ok(())
        } else {
            Err(ExecutorFailure::ResourceLimitUnknown)
        }
    }

    fn kill_and_reap(
        pid: i32,
        status: &mut Option<i32>,
        cleanup_grace: Duration,
    ) -> Result<(), ExecutorFailure> {
        // waitpid already released this PID/PGID for reuse. Never signal it
        // after that point, even if a descendant kept a protocol socket open.
        if status.is_some() {
            return Ok(());
        }
        unsafe {
            libc::kill(-pid, libc::SIGKILL);
            libc::kill(pid, libc::SIGKILL);
        }
        let reap_deadline = Instant::now() + cleanup_grace;
        while status.is_none() && Instant::now() < reap_deadline {
            poll_exit(pid, status)?;
            if status.is_none() {
                thread::sleep(Duration::from_millis(1));
            }
        }
        if status.is_none() {
            // A kernel-uninterruptible child cannot be reaped on a deadline.
            // Report the exact residual PID to the caller/host supervisor;
            // no unbounded blocking or detached request thread is hidden.
            return Err(ExecutorFailure::ReapPending(pid));
        }
        Ok(())
    }

    fn status_failure(status: i32) -> Option<ExecutorFailure> {
        let signal = status & 0x7f;
        if signal != 0 {
            return Some(if signal == libc::SIGXCPU {
                ExecutorFailure::CpuLimit
            } else {
                ExecutorFailure::CrashSignal(signal)
            });
        }
        let exit_code = (status >> 8) & 0xff;
        (exit_code != 0).then_some(ExecutorFailure::CrashExit(exit_code))
    }

    pub(super) fn evaluate(
        worker: &ExactWorkerIdentity,
        resources: &[SchemaResource],
        profile: FormatProfile,
        root_uri: &str,
        raw_instance: &[u8],
        budget: ExecutorBudget,
    ) -> ExecutorOutcome {
        if budget.execution_wall.is_zero()
            || budget.cleanup_grace > Duration::from_secs(1)
            || budget.cpu_seconds == 0
            || budget.cpu_seconds > 60
            || budget.address_space_bytes < 64 * 1024 * 1024
            || budget.address_space_bytes > 8 * 1024 * 1024 * 1024
        {
            return unknown(ExecutorFailure::ResourceLimitUnknown, None);
        }
        let (request, request_sha256, schema_set_sha256, instance_sha256) =
            match make_request(resources, profile, root_uri, raw_instance) {
                Ok(value) => value,
                Err(reason) => return unknown(reason, None),
            };
        let identity = ExecutionIdentity {
            worker_sha256: worker.sha256,
            request_sha256,
            schema_set_sha256,
            instance_sha256,
            profile,
        };
        let start = Instant::now();
        let image = match sealed_worker(worker) {
            Ok(image) => image,
            Err(reason) => return unknown(reason, Some(identity)),
        };
        if start.elapsed() >= budget.execution_wall {
            return unknown(ExecutorFailure::Timeout, Some(identity));
        }
        let argv = [
            c"tos-schema-worker".as_ptr() as *mut libc::c_char,
            std::ptr::null_mut(),
        ];
        run_image(image, request, identity, budget, start, &argv)
    }

    fn run_image(
        image: File,
        request: Vec<u8>,
        identity: ExecutionIdentity,
        budget: ExecutorBudget,
        start: Instant,
        argv: &[*mut libc::c_char],
    ) -> ExecutorOutcome {
        let (input_parent, input_child) = match socket_pair() {
            Ok(pair) => pair,
            Err(_) => return unknown(ExecutorFailure::Spawn, Some(identity)),
        };
        let (output_parent, output_child) = match socket_pair() {
            Ok(pair) => pair,
            Err(_) => return unknown(ExecutorFailure::Spawn, Some(identity)),
        };
        #[cfg(test)]
        TEST_CHILD_STDOUT_INODE.with(|cell| cell.set(output_child.metadata().unwrap().ino()));
        let null = match OpenOptions::new().write(true).open("/dev/null") {
            Ok(file) => file,
            Err(_) => return unknown(ExecutorFailure::Spawn, Some(identity)),
        };
        let parent_pid = unsafe { libc::getpid() };
        let pid = unsafe { libc::fork() };
        if pid < 0 {
            return unknown(ExecutorFailure::Spawn, Some(identity));
        }
        if pid == 0 {
            unsafe {
                child_exec(
                    image.as_raw_fd(),
                    input_child.as_raw_fd(),
                    output_child.as_raw_fd(),
                    null.as_raw_fd(),
                    budget,
                    parent_pid,
                    argv.as_ptr(),
                )
            }
        }
        drop(input_child);
        drop(output_child);
        drop(null);
        drop(image);
        let mut input = Some(input_parent);
        let output = output_parent;
        let mut written = 0usize;
        let mut response = Vec::with_capacity(RESPONSE_BYTES + 1);
        let mut output_eof = false;
        let mut status = None;
        let mut failure = None;
        while !output_eof || written != request.len() || status.is_none() {
            if start.elapsed() >= budget.execution_wall {
                failure = Some(ExecutorFailure::Timeout);
                break;
            }
            if let Err(reason) = poll_exit(pid, &mut status) {
                failure = Some(reason);
                break;
            }
            if written == request.len() {
                if let Some(fd) = input.take() {
                    unsafe { libc::shutdown(fd.as_raw_fd(), libc::SHUT_WR) };
                }
            }
            let mut fds = [
                libc::pollfd {
                    fd: input.as_ref().map_or(-1, AsRawFd::as_raw_fd),
                    events: libc::POLLOUT,
                    revents: 0,
                },
                libc::pollfd {
                    fd: if output_eof { -1 } else { output.as_raw_fd() },
                    events: libc::POLLIN,
                    revents: 0,
                },
            ];
            if unsafe { libc::poll(fds.as_mut_ptr(), 2, 2) } < 0 {
                if io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
                    failure = Some(ExecutorFailure::Protocol);
                    break;
                }
                continue;
            }
            if fds[0].revents & libc::POLLOUT != 0 {
                let count = unsafe {
                    libc::send(
                        fds[0].fd,
                        request[written..].as_ptr().cast(),
                        request.len() - written,
                        libc::MSG_DONTWAIT | libc::MSG_NOSIGNAL,
                    )
                };
                if count > 0 {
                    written += count as usize;
                } else if count == 0
                    || (count < 0 && io::Error::last_os_error().kind() != io::ErrorKind::WouldBlock)
                {
                    failure = Some(ExecutorFailure::Protocol);
                    break;
                }
            }
            if fds[1].revents & (libc::POLLIN | libc::POLLHUP) != 0 {
                let mut buffer = [0u8; 256];
                let count = unsafe {
                    libc::recv(
                        fds[1].fd,
                        buffer.as_mut_ptr().cast(),
                        buffer.len(),
                        libc::MSG_DONTWAIT,
                    )
                };
                if count == 0 {
                    output_eof = true;
                } else if count > 0 {
                    response.extend_from_slice(&buffer[..count as usize]);
                    if response.len() > RESPONSE_BYTES {
                        failure = Some(ExecutorFailure::Protocol);
                        break;
                    }
                } else if io::Error::last_os_error().kind() != io::ErrorKind::WouldBlock {
                    failure = Some(ExecutorFailure::Protocol);
                    break;
                }
            }
            if fds
                .iter()
                .any(|fd| fd.revents & (libc::POLLERR | libc::POLLNVAL) != 0)
            {
                failure = Some(ExecutorFailure::Protocol);
                break;
            }
        }
        if let Some(reason) = failure {
            let observed_failure = status.and_then(status_failure);
            let result = kill_and_reap(pid, &mut status, budget.cleanup_grace);
            return unknown(
                result
                    .err()
                    .unwrap_or_else(|| observed_failure.unwrap_or(reason)),
                Some(identity),
            );
        }
        let Some(status) = status else {
            return unknown(ExecutorFailure::ResourceLimitUnknown, Some(identity));
        };
        if let Some(reason) = status_failure(status) {
            return unknown(reason, Some(identity));
        }
        interpret_response(&response, identity)
    }

    fn interpret_response(response: &[u8], identity: ExecutionIdentity) -> ExecutorOutcome {
        if response.len() != RESPONSE_BYTES
            || &response[..8] != RESPONSE_MAGIC
            || &response[8..40] != identity.request_sha256.as_bytes()
            || &response[40..72] != identity.schema_set_sha256.as_bytes()
            || &response[72..104] != identity.instance_sha256.as_bytes()
        {
            return unknown(ExecutorFailure::Protocol, Some(identity));
        }
        match (response[104], response[105]) {
            (0, 0) => ExecutorOutcome::SchemaValid(identity),
            (1, 0) => ExecutorOutcome::SchemaInvalid(identity),
            (2, 1) => ExecutorOutcome::InputRejected(identity),
            (3, 1) => unknown(ExecutorFailure::InputBudget, Some(identity)),
            (3, 2) => unknown(ExecutorFailure::Backend, Some(identity)),
            _ => unknown(ExecutorFailure::Protocol, Some(identity)),
        }
    }

    struct Cursor<'a> {
        bytes: &'a [u8],
        offset: usize,
    }

    impl<'a> Cursor<'a> {
        fn take(&mut self, count: usize) -> io::Result<&'a [u8]> {
            let end = self
                .offset
                .checked_add(count)
                .filter(|end| *end <= self.bytes.len())
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "short request"))?;
            let part = &self.bytes[self.offset..end];
            self.offset = end;
            Ok(part)
        }

        fn bytes(&mut self, limit: usize) -> io::Result<&'a [u8]> {
            let len = u32::from_be_bytes(self.take(4)?.try_into().unwrap()) as usize;
            if len > limit {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "oversize field"));
            }
            self.take(len)
        }
    }

    pub(super) fn worker_once() -> io::Result<()> {
        let mut frame = Vec::new();
        io::stdin()
            .take((MAX_FRAME_BYTES + 1) as u64)
            .read_to_end(&mut frame)?;
        if frame.len() > MAX_FRAME_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "oversize request",
            ));
        }
        let request_sha256 = Digest256::of_bytes(&frame);
        let mut cursor = Cursor {
            bytes: &frame,
            offset: 0,
        };
        if cursor.take(8)? != REQUEST_MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "protocol version",
            ));
        }
        let _nonce = cursor.take(16)?;
        let profile = parse_profile(cursor.take(1)?[0])
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "format profile"))?;
        let count = u32::from_be_bytes(cursor.take(4)?.try_into().unwrap()) as usize;
        if count > crate::SchemaBackendProbe::MAX_RESOURCES {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "resource count"));
        }
        let mut total = 0usize;
        let mut resources = Vec::with_capacity(count);
        for _ in 0..count {
            let uri = std::str::from_utf8(cursor.bytes(MAX_URI_BYTES)?)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "resource uri"))?;
            let raw = cursor.bytes(crate::SchemaBackendProbe::MAX_RESOURCE_BYTES)?;
            total = total
                .checked_add(raw.len())
                .filter(|total| *total <= crate::SchemaBackendProbe::MAX_TOTAL_BYTES)
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "resource bytes"))?;
            resources.push(SchemaResource {
                uri: uri.to_owned(),
                raw: raw.to_vec(),
            });
        }
        let root_uri = std::str::from_utf8(cursor.bytes(MAX_URI_BYTES)?)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "root uri"))?;
        let raw_instance = cursor.bytes(crate::SchemaBackendProbe::MAX_INSTANCE_BYTES)?;
        if cursor.offset != frame.len() {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "trailing bytes"));
        }
        let schema_set_sha256 = schema_set_digest(&resources)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "duplicate resource"))?;
        let instance_sha256 = Digest256::of_bytes(raw_instance);
        let result = match crate::SchemaBackendProbe::new(resources, profile) {
            Ok(probe) => {
                if probe.schema_set_digest() != schema_set_sha256 {
                    (3, 2)
                } else {
                    match probe.is_valid_raw(root_uri, raw_instance) {
                        Ok(true) => (0, 0),
                        Ok(false) => (1, 0),
                        Err(crate::SchemaProbeError::InvalidPublishedJson(_))
                        | Err(crate::SchemaProbeError::InvalidJson) => (2, 1),
                        Err(crate::SchemaProbeError::BudgetExceeded) => (3, 1),
                        Err(_) => (3, 2),
                    }
                }
            }
            Err(crate::SchemaProbeError::BudgetExceeded) => (3, 1),
            Err(_) => (3, 2),
        };
        let mut response = Vec::with_capacity(RESPONSE_BYTES);
        response.extend_from_slice(RESPONSE_MAGIC);
        response.extend_from_slice(request_sha256.as_bytes());
        response.extend_from_slice(schema_set_sha256.as_bytes());
        response.extend_from_slice(instance_sha256.as_bytes());
        response.push(result.0);
        response.push(result.1);
        io::stdout().write_all(&response)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn fixture_image(path: &str) -> File {
            let path = std::fs::canonicalize(path).unwrap();
            let bytes = std::fs::read(&path).unwrap();
            sealed_worker(&ExactWorkerIdentity {
                absolute_path: path,
                sha256: Digest256::of_bytes(&bytes),
            })
            .unwrap()
        }

        fn fixture_identity() -> ExecutionIdentity {
            ExecutionIdentity {
                worker_sha256: Digest256::of_bytes(b"fixture-worker"),
                request_sha256: Digest256::of_bytes(b"fixture-request"),
                schema_set_sha256: Digest256::of_bytes(b"fixture-schemas"),
                instance_sha256: Digest256::of_bytes(b"fixture-instance"),
                profile: FormatProfile::AssertedSourceCandidateV1,
            }
        }

        #[test]
        fn response_requires_exact_request_and_complete_clean_frame() {
            let identity = ExecutionIdentity {
                worker_sha256: Digest256::of_bytes(b"worker"),
                request_sha256: Digest256::of_bytes(b"request"),
                schema_set_sha256: Digest256::of_bytes(b"schemas"),
                instance_sha256: Digest256::of_bytes(b"instance"),
                profile: FormatProfile::AssertedSourceCandidateV1,
            };
            let mut response = Vec::new();
            response.extend_from_slice(RESPONSE_MAGIC);
            response.extend_from_slice(identity.request_sha256.as_bytes());
            response.extend_from_slice(identity.schema_set_sha256.as_bytes());
            response.extend_from_slice(identity.instance_sha256.as_bytes());
            response.extend_from_slice(&[0, 0]);
            assert_eq!(
                interpret_response(&response, identity),
                ExecutorOutcome::SchemaValid(identity)
            );
            for index in [0, 8, 40, 72, 105] {
                let mut forged = response.clone();
                forged[index] ^= 1;
                assert!(matches!(
                    interpret_response(&forged, identity),
                    ExecutorOutcome::Indeterminate {
                        reason: ExecutorFailure::Protocol,
                        ..
                    }
                ));
            }
            response.push(0);
            assert!(matches!(
                interpret_response(&response, identity),
                ExecutorOutcome::Indeterminate {
                    reason: ExecutorFailure::Protocol,
                    ..
                }
            ));
        }

        #[test]
        fn oversized_and_duplicate_inputs_never_launch_worker() {
            let one = SchemaResource {
                uri: "https://example.invalid/schema".to_owned(),
                raw: b"{}".to_vec(),
            };
            assert!(matches!(
                make_request(
                    &[one.clone()],
                    FormatProfile::AssertedSourceCandidateV1,
                    "root",
                    &vec![0; crate::SchemaBackendProbe::MAX_INSTANCE_BYTES + 1]
                ),
                Err(ExecutorFailure::InputBudget)
            ));
            assert!(matches!(
                make_request(
                    &[one.clone(), one],
                    FormatProfile::AssertedSourceCandidateV1,
                    "root",
                    b"null"
                ),
                Err(ExecutorFailure::Backend)
            ));
        }

        #[test]
        fn worker_that_never_reads_stdin_is_killed_without_waiting_for_a_writer() {
            let image = fixture_image("/usr/bin/sleep");
            let arg = c"2";
            let argv = [
                c"sleep".as_ptr() as *mut libc::c_char,
                arg.as_ptr() as *mut libc::c_char,
                std::ptr::null_mut(),
            ];
            let budget = ExecutorBudget {
                execution_wall: Duration::from_millis(200),
                cleanup_grace: Duration::from_millis(200),
                cpu_seconds: 2,
                address_space_bytes: 1024 * 1024 * 1024,
            };
            let start = Instant::now();
            let result = run_image(
                image,
                vec![b'x'; 2 * 1024 * 1024],
                fixture_identity(),
                budget,
                start,
                &argv,
            );
            assert!(matches!(
                result,
                ExecutorOutcome::Indeterminate {
                    reason: ExecutorFailure::Timeout,
                    ..
                }
            ));
            assert!(start.elapsed() < Duration::from_millis(600));
        }

        #[test]
        fn escaped_descendant_retaining_stdout_cannot_hold_parent_after_deadline() {
            use std::os::unix::ffi::OsStrExt;
            use std::time::{SystemTime, UNIX_EPOCH};

            let image = fixture_image("/bin/sh");
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let dir = std::env::temp_dir()
                .join(format!("tos-val2-escaped-{}-{unique}", std::process::id()));
            std::fs::create_dir(&dir).unwrap();
            let pid_file = dir.join("child.pid");
            let pid_arg = std::ffi::CString::new(pid_file.as_os_str().as_bytes()).unwrap();
            // The inner shell records its exact PID, then becomes sleep. It
            // has a new session and retains the worker's stdout socket.
            let script = c"/usr/bin/setsid /bin/sh -c 'echo $$ > \"$1\"; exec /usr/bin/sleep 1.2' child \"$1\" & echo ready; wait";
            let argv = [
                c"sh".as_ptr() as *mut libc::c_char,
                c"-c".as_ptr() as *mut libc::c_char,
                script.as_ptr() as *mut libc::c_char,
                c"fixture".as_ptr() as *mut libc::c_char,
                pid_arg.as_ptr() as *mut libc::c_char,
                std::ptr::null_mut(),
            ];
            let budget = ExecutorBudget {
                execution_wall: Duration::from_millis(600),
                cleanup_grace: Duration::from_millis(200),
                cpu_seconds: 2,
                address_space_bytes: 1024 * 1024 * 1024,
            };
            let start = Instant::now();
            let result = run_image(
                image,
                b"request".to_vec(),
                fixture_identity(),
                budget,
                start,
                &argv,
            );
            let child_pid: i32 = std::fs::read_to_string(&pid_file)
                .unwrap()
                .trim()
                .parse()
                .unwrap();
            let pidfd_raw = unsafe { libc::syscall(libc::SYS_pidfd_open, child_pid, 0) as i32 };
            assert!(pidfd_raw >= 0);
            let pidfd = unsafe { File::from_raw_fd(pidfd_raw) };
            let expected_inode = TEST_CHILD_STDOUT_INODE.with(std::cell::Cell::get);
            let fd1 = std::fs::read_link(format!("/proc/{child_pid}/fd/1")).unwrap();
            assert_eq!(fd1.to_string_lossy(), format!("socket:[{expected_inode}]"));
            assert!(matches!(
                result,
                ExecutorOutcome::Indeterminate {
                    reason: ExecutorFailure::Timeout,
                    ..
                }
            ));
            assert!(start.elapsed() < Duration::from_millis(950));
            let mut exit_poll = libc::pollfd {
                fd: pidfd.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            assert_eq!(unsafe { libc::poll(&mut exit_poll, 1, 1500) }, 1);
            assert_ne!(exit_poll.revents & libc::POLLIN, 0);
            std::fs::remove_file(pid_file).unwrap();
            std::fs::remove_dir(dir).unwrap();
        }
    }
}
