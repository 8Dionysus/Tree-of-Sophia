//! Bounded host launcher for the source-navigation model compiler child.
//!
//! This module owns only process admission and its finite stdio protocol. The
//! private-stage issuer remains responsible for namespace isolation and stage
//! cleanup; owner selection and final publication remain in the caller.

use super::source_navigation_model_compile::{
    CHILD_MAX_TOTAL_MS, SOURCE_NAVIGATION_PHASE_STAGES, SourceNavigationModelCompileRequestV2,
    SourceNavigationModelCompileResultV1,
};
use crate::source_command::{SourceCommandError, SourceCommandResult};
use std::{
    ffi::CString,
    fs::{self, File},
    io::{self, Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd, RawFd},
        unix::{fs::MetadataExt, process::CommandExt},
    },
    path::{Component, Path, PathBuf},
    process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
use tos_foundation::{Digest256, Digest256Hasher};

const REQUEST_SCHEMA: &str = "tos_source_navigation_model_compile_request_v2";
const RESULT_SCHEMA: &str = "tos_source_navigation_model_compile_result_v1";
const CANDIDATE_LEAF: &str = "source-navigation-model.sqlite";
const REQUEST_MAX_BYTES: usize = 8 * 1024 * 1024;
const RESPONSE_MAX_BYTES: usize = 8 * 1024 * 1024;
const STDERR_RETAIN_BYTES: usize = 16 * 1024;
const EXECUTABLE_MAX_BYTES: u64 = 512 * 1024 * 1024;
const WORKER_MAX_BYTES: u64 = 256 * 1024 * 1024;
const CHILD_MAX_SHUTDOWN_MS: u64 = 60_000;
const POST_EXIT_DRAIN_MS: u64 = 250;
const PIPE_READ_SLICE_BYTES: usize = 64 * 1024;
const MASKED_FALLBACK_ROOTS: [&str; 3] = ["/var/tmp", "/usr/tmp", "/tmp"];
const CHILD_REFUSAL_PREFIX: &str = "source-navigation model compile refused: ";
const CHILD_REFUSAL_MAX_BYTES: usize = 4 * 1024;
const PHASE_TRACE_TAG: &str = "tos_source_navigation_phase_v1";
const PHASE_TRACE_PREFIX: &str = "tos_source_navigation_phase_v1 ";
const PHASE_TRACE_MAX_LINE_BYTES: usize = 160;
const PHASE_TRACE_MAX_LINES: usize = 16;

/// Exact private-stage issuer and delegated resource envelope selected by the
/// owner. `persistent_staging_root` is the stable parent of the fresh
/// per-invocation `request.artifact_staging_root` directory.
#[derive(Clone, Debug)]
pub(crate) struct SourceNavigationModelLauncherV1 {
    pub tos_access_path: PathBuf,
    pub tos_access_sha256: String,
    pub unshare_path: PathBuf,
    pub unshare_sha256: String,
    pub setup_cgroup: PathBuf,
    pub consumer_cgroup: PathBuf,
    pub scratch_parent: PathBuf,
    pub persistent_staging_root: PathBuf,
    pub shutdown_grace_ms: u64,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SourceNavigationModelProcessLimits {
    pub max_total_ms: u64,
    pub tmpfs_bytes: u64,
    pub tmpfs_inodes: u64,
    pub working_ram_bytes: u64,
    pub max_model_bytes: u64,
}

struct HeldCgroups {
    common: File,
    setup: File,
    consumer: File,
    setup_procs: File,
}

struct ChildCleanupGuard<'a> {
    child: Child,
    cgroups: &'a HeldCgroups,
    complete: bool,
}

impl Drop for ChildCleanupGuard<'_> {
    fn drop(&mut self) {
        if self.complete {
            return;
        }
        cleanup_cgroups(self.cgroups);
        signal_group(self.child.id(), libc::SIGKILL);
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct PipeState {
    stdin: Option<ChildStdin>,
    stdout: Option<ChildStdout>,
    stderr: Option<ChildStderr>,
    input: Vec<u8>,
    input_offset: usize,
    output: Vec<u8>,
    stderr_output: Vec<u8>,
    output_oversized: bool,
    stderr_oversized: bool,
    input_failed: bool,
}

/// Run the typed compile request through the existing `tos-access
/// private-stage-run` issuer. The exact request bytes sent on stdin are hashed
/// and the child's response must echo that digest and fixed candidate leaf.
pub(crate) fn launch_source_navigation_model_compile(
    launcher: &SourceNavigationModelLauncherV1,
    limits: SourceNavigationModelProcessLimits,
    remaining_budget_ms: u64,
    request: &mut SourceNavigationModelCompileRequestV2,
    cancelled: &AtomicBool,
) -> SourceCommandResult<SourceNavigationModelCompileResultV1> {
    // Establish the outer deadline before path checks or executable hashing.
    let started = monotonic_ns()?;
    let total_ms = remaining_budget_ms
        .min(limits.max_total_ms)
        .min(CHILD_MAX_TOTAL_MS);
    if total_ms == 0
        || launcher.shutdown_grace_ms == 0
        || launcher.shutdown_grace_ms > CHILD_MAX_SHUTDOWN_MS
        || total_ms <= launcher.shutdown_grace_ms
    {
        return Err(SourceCommandError::Invalid(
            "source-navigation child deadline envelope",
        ));
    }
    let total_ns = total_ms
        .checked_mul(1_000_000)
        .ok_or(SourceCommandError::Invalid(
            "source-navigation deadline overflow",
        ))?;
    let whole_deadline_ns = started
        .checked_add(total_ns)
        .ok_or(SourceCommandError::Invalid(
            "source-navigation deadline overflow",
        ))?;
    let work_deadline_ns = whole_deadline_ns
        .checked_sub(launcher.shutdown_grace_ms * 1_000_000)
        .ok_or(SourceCommandError::Invalid(
            "source-navigation deadline overflow",
        ))?;
    let child_work_ms = total_ms - launcher.shutdown_grace_ms;
    check_cancelled(cancelled)?;
    verify_limit_binding(request, limits)?;
    let input_root = verify_inputs(request)?;
    verify_paths_and_staging(launcher, request, &input_root)?;

    let (access_image, access_digest) = pinned_executable(
        &launcher.tos_access_path,
        &launcher.tos_access_sha256,
        EXECUTABLE_MAX_BYTES,
        "source-navigation tos-access image",
    )?;
    let (unshare_image, unshare_digest) = pinned_executable(
        &launcher.unshare_path,
        &launcher.unshare_sha256,
        EXECUTABLE_MAX_BYTES,
        "source-navigation unshare image",
    )?;
    verify_canonical_system_executable(&launcher.unshare_path, &unshare_image)?;
    let native_path = std::env::current_exe()
        .map_err(|_| SourceCommandError::Invalid("source-navigation native image path"))?;
    let (native_image, native_digest) = pinned_current_native(&native_path)?;
    request.native_executable_sha256 = format!("sha256:{}", native_digest.to_hex());

    let held = validate_cgroups(launcher, limits)?;
    check_cancelled(cancelled)?;
    if monotonic_ns()? >= work_deadline_ns {
        return Err(SourceCommandError::Conflict(
            "source-navigation child deadline elapsed before launch",
        ));
    }

    let current_ns = monotonic_ns()?;
    let remaining_work_ns = work_deadline_ns
        .checked_sub(current_ns)
        .filter(|remaining| *remaining >= 1_000_000)
        .ok_or(SourceCommandError::Conflict(
            "source-navigation child work budget exhausted before launch",
        ))?;
    request.operation_budget_ms = (remaining_work_ns / 1_000_000).min(child_work_ms);

    let request_bytes = serde_json::to_vec(request)
        .map_err(|_| SourceCommandError::Invalid("source-navigation request encoding"))?;
    if request.schema_version != REQUEST_SCHEMA || request_bytes.len() > REQUEST_MAX_BYTES {
        return Err(SourceCommandError::Invalid(
            "source-navigation request schema or byte cap",
        ));
    }
    let request_digest = Digest256::of_bytes(&request_bytes).to_hex();
    let access_fd = access_image.as_raw_fd();
    let unshare_fd = unshare_image.as_raw_fd();
    let setup_procs_fd = held.setup_procs.as_raw_fd();
    let expected_parent = unsafe { libc::getpid() };
    let access_exec = fd_path(access_fd);
    let native_hex = native_digest.to_hex();
    let access_hex = access_digest.to_hex();

    let mut command = Command::new(&access_exec);
    command
        .arg("private-stage-run")
        .args([
            "--unshare-exe",
            launcher
                .unshare_path
                .to_str()
                .ok_or(SourceCommandError::Invalid(
                    "source-navigation unshare path encoding",
                ))?,
            "--consumer-cgroup",
            launcher
                .consumer_cgroup
                .to_str()
                .ok_or(SourceCommandError::Invalid(
                    "source-navigation consumer cgroup path",
                ))?,
            "--scratch-parent",
            launcher
                .scratch_parent
                .to_str()
                .ok_or(SourceCommandError::Invalid(
                    "source-navigation scratch path",
                ))?,
            "--quota-bytes",
            &limits.tmpfs_bytes.to_string(),
            "--inodes",
            &limits.tmpfs_inodes.to_string(),
            "--working-ram-bytes",
            &limits.working_ram_bytes.to_string(),
            "--work-deadline-ns",
            &whole_deadline_ns.to_string(),
            "--maximum-shutdown-ms",
            &launcher.shutdown_grace_ms.to_string(),
            "--persistent-store",
            request
                .artifact_staging_root
                .to_str()
                .ok_or(SourceCommandError::Invalid(
                    "source-navigation staging path",
                ))?,
            "--protected-ro",
            request
                .read_only_input_root
                .to_str()
                .ok_or(SourceCommandError::Invalid(
                    "source-navigation protected input path",
                ))?,
            "--expected-controller-sha256",
            &format!("sha256:{access_hex}"),
            "--expected-consumer-sha256",
            &format!("sha256:{native_hex}"),
            "--expected-unshare-sha256",
            &format!("sha256:{}", unshare_digest.to_hex()),
            "--unshare-image-fd",
            &unshare_fd.to_string(),
            "--",
        ])
        .arg(&native_path)
        .arg("source-navigation-model-compile")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LANG", "C.UTF-8")
        .env("LC_ALL", "C.UTF-8")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // The pinned executable descriptors and cgroup destination are the only
    // intentionally inherited descriptors. Move only this forked child into
    // the issuer/setup cgroup before it can execute any userspace code.
    unsafe {
        command.pre_exec(move || {
            if libc::getppid() != expected_parent {
                return Err(io::Error::from_raw_os_error(libc::ESRCH));
            }
            if libc::setpgid(0, 0) != 0 {
                return Err(io::Error::last_os_error());
            }
            for fd in [access_fd, unshare_fd] {
                let flags = libc::fcntl(fd, libc::F_GETFD);
                if flags < 0 || libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) < 0 {
                    return Err(io::Error::last_os_error());
                }
            }
            let moved = libc::write(setup_procs_fd, b"0\n".as_ptr().cast(), 2);
            if moved != 2 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let child = command
        .spawn()
        .map_err(|_| SourceCommandError::Denied("source-navigation private stage unavailable"))?;
    // Parent copies of these descriptors are no longer needed once the child
    // has inherited them. Keeping them open would complicate EOF and custody.
    drop((access_image, unshare_image, native_image));

    let mut child_guard = ChildCleanupGuard {
        child,
        cgroups: &held,
        complete: false,
    };
    let child = &mut child_guard.child;

    let mut pipes = PipeState {
        stdin: child.stdin.take(),
        stdout: child.stdout.take(),
        stderr: child.stderr.take(),
        input: request_bytes,
        input_offset: 0,
        output: Vec::new(),
        stderr_output: Vec::with_capacity(STDERR_RETAIN_BYTES),
        output_oversized: false,
        stderr_oversized: false,
        input_failed: false,
    };
    set_nonblocking(&pipes)?;
    let mut child_status: Option<ExitStatus> = None;
    let mut term_sent_at: Option<Instant> = None;
    let mut kill_sent = false;
    let mut failure: Option<&'static str> = None;
    let mut status_seen_at: Option<Instant> = None;

    loop {
        if child_status.is_none() {
            child_status = child
                .try_wait()
                .map_err(|_| SourceCommandError::Invalid("source-navigation child wait"))?;
            if child_status.is_some() {
                status_seen_at = Some(Instant::now());
                pipes.stdin.take();
            }
        }
        if failure.is_none() {
            if cancelled.load(Ordering::Acquire) {
                failure = Some("source-navigation child cancelled");
            } else if monotonic_ns()? >= work_deadline_ns {
                failure = Some("source-navigation child work deadline exceeded");
            } else if pipes.output_oversized {
                failure = Some("source-navigation child response exceeds byte cap");
            } else if pipes.input_failed {
                failure = Some("source-navigation child request delivery failed");
            }
        }
        if failure.is_some() && term_sent_at.is_none() && child_status.is_none() {
            signal_group(child.id(), libc::SIGTERM);
            term_sent_at = Some(Instant::now());
        }
        let groups_empty = cgroups_empty(&held)?;
        if child_status.is_some() && !groups_empty {
            if failure.is_none() {
                failure =
                    Some("source-navigation private stage left processes in delegated cgroups");
            }
            if term_sent_at.is_none() {
                let _ = cleanup_cgroups(&held);
                signal_group(child.id(), libc::SIGTERM);
                term_sent_at = Some(Instant::now());
            }
        }
        if term_sent_at.is_some()
            && !kill_sent
            && (monotonic_ns()? >= whole_deadline_ns
                || term_sent_at
                    .map(|start| {
                        start.elapsed() >= Duration::from_millis(launcher.shutdown_grace_ms)
                    })
                    .unwrap_or(false))
        {
            let _ = cleanup_cgroups(&held);
            signal_group(child.id(), libc::SIGKILL);
            kill_sent = true;
        }
        if monotonic_ns()? >= whole_deadline_ns && child_status.is_none() {
            failure.get_or_insert("source-navigation child exceeded shutdown deadline");
            let _ = cleanup_cgroups(&held);
            signal_group(child.id(), libc::SIGKILL);
            kill_sent = true;
        }
        if monotonic_ns()? >= whole_deadline_ns && !groups_empty {
            failure.get_or_insert("source-navigation cgroup cleanup exceeded total deadline");
            let _ = cleanup_cgroups(&held);
            signal_group(child.id(), libc::SIGKILL);
            pipes.stdin.take();
            pipes.stdout.take();
            pipes.stderr.take();
            break;
        }
        if child_status.is_some()
            && pipes.stdout.is_some()
            && status_seen_at
                .map(|at| at.elapsed() >= Duration::from_millis(POST_EXIT_DRAIN_MS))
                .unwrap_or(false)
        {
            if failure.is_none() {
                failure = Some("source-navigation child left inherited output pipes open");
            }
            let _ = cleanup_cgroups(&held);
            signal_group(child.id(), libc::SIGKILL);
            pipes.stdout.take();
            pipes.stderr.take();
        }
        if child_status.is_some()
            && pipes.stderr.is_some()
            && status_seen_at
                .map(|at| at.elapsed() >= Duration::from_millis(POST_EXIT_DRAIN_MS))
                .unwrap_or(false)
        {
            if failure.is_none() {
                failure = Some("source-navigation child left inherited output pipes open");
            }
            let _ = cleanup_cgroups(&held);
            signal_group(child.id(), libc::SIGKILL);
            pipes.stdout.take();
            pipes.stderr.take();
        }

        if child_status.is_some()
            && pipes.stdin.is_none()
            && pipes.stdout.is_none()
            && pipes.stderr.is_none()
            && groups_empty
        {
            break;
        }
        pump_pipes(&mut pipes)?;
        if child_status.is_none() || pipes.stdout.is_some() || pipes.stderr.is_some() {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    let status = match child_status {
        Some(status) => status,
        None => child
            .wait()
            .map_err(|_| SourceCommandError::Invalid("source-navigation child reap"))?,
    };
    same_named_directory(&launcher.setup_cgroup, &held.setup)?;
    same_named_directory(&launcher.consumer_cgroup, &held.consumer)?;
    if failure.is_some()
        || pipes.output_oversized
        || pipes.input_failed
        || !cgroups_empty(&held)?
        || pipes.input_offset != pipes.input.len()
        || cancelled.load(Ordering::Acquire)
        || monotonic_ns()? >= whole_deadline_ns
    {
        forward_validated_phase_trace(&pipes.stderr_output, pipes.stderr_oversized);
        return Err(SourceCommandError::Conflict(
            failure.unwrap_or("source-navigation compile child refused"),
        ));
    }
    if !status.success() {
        forward_validated_phase_trace(&pipes.stderr_output, pipes.stderr_oversized);
        if let Some(reason) = sanitized_child_refusal(&pipes.stderr_output) {
            return Err(SourceCommandError::DeniedWithReason(reason));
        }
        let stderr = String::from_utf8_lossy(&pipes.stderr_output);
        let cause = crate::source_admission_spooled_index::bounded_source_cause(
            "receiver-source",
            "private-stage-run",
            &format!("oversized={};{stderr}", pipes.stderr_oversized),
        );
        return Err(SourceCommandError::DeniedWithReason(format!(
            "source-navigation private-stage-run refused: {cause}"
        )));
    }
    child_guard.complete = true;
    let response: SourceNavigationModelCompileResultV1 = serde_json::from_slice(&pipes.output)
        .map_err(|_| SourceCommandError::Invalid("source-navigation child response schema"))?;
    validate_result(&response, &request_digest, request, limits)?;
    verify_candidate_readback(request, &response, limits)?;
    check_cancelled(cancelled)?;
    if monotonic_ns()? >= whole_deadline_ns {
        return Err(SourceCommandError::Conflict(
            "source-navigation child exceeded its total deadline",
        ));
    }
    Ok(response)
}

fn check_cancelled(cancelled: &AtomicBool) -> SourceCommandResult<()> {
    if cancelled.load(Ordering::Acquire) {
        Err(SourceCommandError::Conflict(
            "source-navigation model preparation cancelled",
        ))
    } else {
        Ok(())
    }
}

fn verify_limit_binding(
    request: &SourceNavigationModelCompileRequestV2,
    limits: SourceNavigationModelProcessLimits,
) -> SourceCommandResult<()> {
    let selected = request.model_limits;
    if limits.max_total_ms == 0
        || limits.tmpfs_bytes == 0
        || limits.tmpfs_inodes == 0
        || limits.working_ram_bytes == 0
        || limits.max_model_bytes == 0
        || selected.max_total_ms != limits.max_total_ms
        || selected.tmpfs_bytes != limits.tmpfs_bytes
        || selected.tmpfs_inodes != limits.tmpfs_inodes
        || selected.working_ram_bytes != limits.working_ram_bytes
        || selected.max_model_bytes != limits.max_model_bytes
    {
        return Err(SourceCommandError::Conflict(
            "source-navigation child limits differ from selected owner",
        ));
    }
    Ok(())
}

fn verify_inputs(request: &SourceNavigationModelCompileRequestV2) -> SourceCommandResult<File> {
    let root = normalized_absolute(&request.read_only_input_root)?;
    let cut = normalized_absolute(&request.source_cut_root)?;
    if cut == root || !cut.starts_with(&root) {
        return Err(SourceCommandError::Invalid(
            "source-navigation selected cut is outside protected input root",
        ));
    }
    let root_fd = tos_fd_open::open_absolute_directory(&root)
        .map_err(|_| SourceCommandError::Conflict("source-navigation protected input root"))?;
    verify_owned_private_directory(&root_fd, "source-navigation protected input root")?;
    let cut_fd = tos_fd_open::open_absolute_directory(&cut)
        .map_err(|_| SourceCommandError::Conflict("source-navigation selected cut root"))?;
    same_named_directory(&cut, &cut_fd)?;

    let worker_path = normalized_absolute(&request.schema_worker.absolute_path)?;
    let processor_path = normalized_absolute(&request.normalization_processor.absolute_path)?;
    let canonical_worker = fs::canonicalize(&worker_path)
        .map_err(|_| SourceCommandError::Conflict("source-navigation selected schema worker"))?;
    let worker_under_masked_fallback = MASKED_FALLBACK_ROOTS
        .iter()
        .map(Path::new)
        .any(|fallback| canonical_worker.starts_with(fallback));
    if canonical_worker != worker_path
        || (!worker_path.starts_with(&root) && worker_under_masked_fallback)
        || !processor_path.starts_with(&root)
    {
        return Err(SourceCommandError::Invalid(
            "source-navigation selected worker or processor path",
        ));
    }
    let (worker_image, _) = pinned_executable(
        &worker_path,
        &request.schema_worker.sha256,
        WORKER_MAX_BYTES,
        "source-navigation selected schema worker",
    )?;
    drop(worker_image);
    verify_file_digest(
        &processor_path,
        &request.normalization_processor.sha256,
        WORKER_MAX_BYTES,
        Some(request.normalization_processor.size_bytes),
        "source-navigation selected normalization processor",
    )?;
    Ok(root_fd)
}

fn verify_paths_and_staging(
    launcher: &SourceNavigationModelLauncherV1,
    request: &SourceNavigationModelCompileRequestV2,
    input_root: &File,
) -> SourceCommandResult<()> {
    let input_path = normalized_absolute(&request.read_only_input_root)?;
    let stable = normalized_absolute(&launcher.persistent_staging_root)?;
    let fresh = normalized_absolute(&request.artifact_staging_root)?;
    let scratch = normalized_absolute(&launcher.scratch_parent)?;
    let setup = normalized_absolute(&launcher.setup_cgroup)?;
    let consumer = normalized_absolute(&launcher.consumer_cgroup)?;
    let tos_access = normalized_absolute(&launcher.tos_access_path)?;
    let unshare = normalized_absolute(&launcher.unshare_path)?;
    if fresh.parent() != Some(stable.as_path())
        || !paths_disjoint(&input_path, &stable)
        || !paths_disjoint(&input_path, &scratch)
        || !paths_disjoint(&stable, &scratch)
        || !paths_disjoint(&input_path, &setup)
        || !paths_disjoint(&input_path, &consumer)
        || !paths_disjoint(&stable, &setup)
        || !paths_disjoint(&stable, &consumer)
        || !paths_disjoint(&scratch, &setup)
        || !paths_disjoint(&scratch, &consumer)
        || !paths_disjoint(&tos_access, &input_path)
        || !paths_disjoint(&unshare, &input_path)
    {
        return Err(SourceCommandError::Invalid(
            "source-navigation launcher paths overlap",
        ));
    }
    same_named_directory(&input_path, input_root)?;
    let stable_fd = tos_fd_open::open_absolute_directory(&stable)
        .map_err(|_| SourceCommandError::Conflict("source-navigation persistent stage parent"))?;
    verify_owned_private_directory(&stable_fd, "source-navigation persistent stage parent")?;
    let fresh_fd = tos_fd_open::open_absolute_directory(&fresh)
        .map_err(|_| SourceCommandError::Conflict("source-navigation fresh artifact stage"))?;
    verify_owned_private_directory(&fresh_fd, "source-navigation fresh artifact stage")?;
    let leaf = fresh.file_name().ok_or(SourceCommandError::Invalid(
        "source-navigation fresh stage leaf",
    ))?;
    let relative = Path::new(leaf);
    let anchored = tos_fd_open::open_directory_at(&stable_fd, relative)
        .map_err(|_| SourceCommandError::Conflict("source-navigation fresh stage identity"))?;
    if !same_identity(&fresh_fd, &anchored)? || !directory_is_empty(&fresh)? {
        return Err(SourceCommandError::Conflict(
            "source-navigation fresh stage must be empty and anchored",
        ));
    }
    let scratch_fd = tos_fd_open::open_absolute_directory(&scratch)
        .map_err(|_| SourceCommandError::Conflict("source-navigation scratch parent"))?;
    verify_owned_private_directory(&scratch_fd, "source-navigation scratch parent")?;
    Ok(())
}

fn validate_cgroups(
    launcher: &SourceNavigationModelLauncherV1,
    limits: SourceNavigationModelProcessLimits,
) -> SourceCommandResult<HeldCgroups> {
    let setup_path = normalized_absolute(&launcher.setup_cgroup)?;
    let consumer_path = normalized_absolute(&launcher.consumer_cgroup)?;
    let common_path = setup_path.parent().ok_or(SourceCommandError::Invalid(
        "source-navigation cgroup parent",
    ))?;
    if common_path == Path::new("/sys/fs/cgroup")
        || consumer_path == setup_path
        || consumer_path.parent() != Some(common_path)
    {
        return Err(SourceCommandError::Invalid(
            "source-navigation cgroups must be dedicated siblings",
        ));
    }
    let common = tos_fd_open::open_absolute_directory(common_path).map_err(|_| {
        SourceCommandError::Denied("source-navigation delegated cgroup unavailable")
    })?;
    let setup = tos_fd_open::open_absolute_directory(&setup_path)
        .map_err(|_| SourceCommandError::Denied("source-navigation setup cgroup unavailable"))?;
    let consumer = tos_fd_open::open_absolute_directory(&consumer_path)
        .map_err(|_| SourceCommandError::Denied("source-navigation consumer cgroup unavailable"))?;
    let common_meta = common
        .metadata()
        .map_err(|_| SourceCommandError::Invalid("source-navigation cgroup metadata"))?;
    let uid = unsafe { libc::geteuid() };
    let mut filesystem = unsafe { std::mem::zeroed::<libc::statfs>() };
    if unsafe { libc::fstatfs(common.as_raw_fd(), &mut filesystem) } != 0
        || filesystem.f_type != libc::CGROUP2_SUPER_MAGIC
        || uid == 0
        || common_meta.uid() != uid
    {
        return Err(SourceCommandError::Denied(
            "source-navigation requires an owned delegated cgroup v2 scope",
        ));
    }
    let aggregate = limits
        .tmpfs_bytes
        .checked_add(limits.working_ram_bytes)
        .ok_or(SourceCommandError::Invalid(
            "source-navigation cgroup quota overflow",
        ))?;
    if cgroup_scalar(&common, "memory.max")? != aggregate
        || cgroup_scalar(&common, "memory.swap.max")? != 0
        || cgroup_scalar(&setup, "memory.max")? != limits.tmpfs_bytes
        || cgroup_scalar(&setup, "memory.swap.max")? != 0
        || cgroup_scalar(&consumer, "memory.max")? != limits.working_ram_bytes
        || cgroup_scalar(&consumer, "memory.swap.max")? != 0
        || cgroup_text(&common, "cgroup.type", 64)?.trim() != "domain"
        || cgroup_text(&setup, "cgroup.type", 64)?.trim() != "domain"
        || cgroup_text(&consumer, "cgroup.type", 64)?.trim() != "domain"
        || !cgroup_text(&common, "cgroup.controllers", 4096)?
            .split_whitespace()
            .any(|entry| entry == "memory")
        || !cgroup_text(&common, "cgroup.subtree_control", 4096)?
            .split_whitespace()
            .any(|entry| entry == "memory")
        || !cgroups_empty_inner(&common, &setup, &consumer)?
    {
        return Err(SourceCommandError::Conflict(
            "source-navigation delegated cgroup topology differs",
        ));
    }
    let membership = fs::read_to_string("/proc/self/cgroup")
        .map_err(|_| SourceCommandError::Invalid("source-navigation parent cgroup membership"))?;
    let actual = membership
        .lines()
        .filter_map(|line| line.strip_prefix("0::"))
        .collect::<Vec<_>>();
    if actual.len() != 1 {
        return Err(SourceCommandError::Invalid(
            "source-navigation parent cgroup membership is ambiguous",
        ));
    }
    let parent_membership = Path::new("/sys/fs/cgroup").join(actual[0].trim_start_matches('/'));
    if parent_membership.starts_with(common_path) {
        return Err(SourceCommandError::Conflict(
            "source-navigation launcher parent must remain outside delegated scope",
        ));
    }
    let setup_procs = cgroup_open(&setup, "cgroup.procs", libc::O_WRONLY)?;
    // Probe exact fallback cleanup authority before any child is admitted.
    drop(cgroup_open(&setup, "cgroup.kill", libc::O_WRONLY)?);
    drop(cgroup_open(&consumer, "cgroup.kill", libc::O_WRONLY)?);
    Ok(HeldCgroups {
        common,
        setup,
        consumer,
        setup_procs,
    })
}

fn pinned_executable(
    path: &Path,
    expected_hex: &str,
    max_bytes: u64,
    detail: &'static str,
) -> SourceCommandResult<(File, Digest256)> {
    let normalized = normalized_absolute(path)?;
    let file = tos_fd_open::open_absolute_regular(&normalized, max_bytes)
        .map_err(|_| SourceCommandError::Denied(detail))?;
    let metadata = file
        .metadata()
        .map_err(|_| SourceCommandError::Invalid(detail))?;
    if metadata.mode() & 0o111 == 0 {
        return Err(SourceCommandError::Invalid(detail));
    }
    let expected =
        Digest256::from_hex(expected_hex).map_err(|_| SourceCommandError::Invalid(detail))?;
    let actual = hash_file(
        file.try_clone()
            .map_err(|_| SourceCommandError::Invalid(detail))?,
        max_bytes,
    )?;
    if actual != expected {
        return Err(SourceCommandError::Conflict(detail));
    }
    Ok((file, actual))
}

fn pinned_current_native(path: &Path) -> SourceCommandResult<(File, Digest256)> {
    let normalized = normalized_absolute(path)?;
    let file = File::open("/proc/self/exe").map_err(|_| {
        SourceCommandError::Conflict("source-navigation native image is not pinnable")
    })?;
    let metadata = file
        .metadata()
        .map_err(|_| SourceCommandError::Invalid("source-navigation native image metadata"))?;
    let named =
        tos_fd_open::open_absolute_regular(&normalized, EXECUTABLE_MAX_BYTES).map_err(|_| {
            SourceCommandError::Conflict("source-navigation native image is not pinnable")
        })?;
    if metadata.mode() & 0o111 == 0 || !same_identity(&file, &named)? {
        return Err(SourceCommandError::Invalid(
            "source-navigation native image identity differs",
        ));
    }
    let digest = hash_file(
        file.try_clone()
            .map_err(|_| SourceCommandError::Invalid("source-navigation native image"))?,
        EXECUTABLE_MAX_BYTES,
    )?;
    Ok((file, digest))
}

fn verify_canonical_system_executable(path: &Path, image: &File) -> SourceCommandResult<()> {
    let canonical = fs::canonicalize(path)
        .map_err(|_| SourceCommandError::Invalid("source-navigation unshare canonical path"))?;
    let metadata = image
        .metadata()
        .map_err(|_| SourceCommandError::Invalid("source-navigation unshare metadata"))?;
    if canonical != path
        || metadata.mode() & 0o022 != 0
        || metadata.uid() == unsafe { libc::geteuid() }
    {
        return Err(SourceCommandError::Denied(
            "source-navigation unshare executable must be immutable system code",
        ));
    }
    let mut parent = path.parent();
    while let Some(path) = parent {
        if path == Path::new("/") {
            break;
        }
        let directory = tos_fd_open::open_absolute_directory(path)
            .map_err(|_| SourceCommandError::Invalid("source-navigation unshare parent path"))?;
        let metadata = directory.metadata().map_err(|_| {
            SourceCommandError::Invalid("source-navigation unshare parent metadata")
        })?;
        if metadata.mode() & 0o022 != 0 || metadata.uid() == unsafe { libc::geteuid() } {
            return Err(SourceCommandError::Denied(
                "source-navigation unshare path has a writable parent",
            ));
        }
        parent = path.parent();
    }
    Ok(())
}

fn verify_file_digest(
    path: &Path,
    expected_hex: &str,
    max_bytes: u64,
    exact_size: Option<u64>,
    detail: &'static str,
) -> SourceCommandResult<()> {
    let normalized = normalized_absolute(path)?;
    let file = tos_fd_open::open_absolute_regular(&normalized, max_bytes)
        .map_err(|_| SourceCommandError::Conflict(detail))?;
    let metadata = file
        .metadata()
        .map_err(|_| SourceCommandError::Invalid(detail))?;
    if exact_size.is_some_and(|expected| metadata.len() != expected) {
        return Err(SourceCommandError::Conflict(detail));
    }
    let expected =
        Digest256::from_hex(expected_hex).map_err(|_| SourceCommandError::Invalid(detail))?;
    if hash_file(file, max_bytes)? != expected {
        return Err(SourceCommandError::Conflict(detail));
    }
    Ok(())
}

fn hash_file(mut file: File, max_bytes: u64) -> SourceCommandResult<Digest256> {
    let mut hash = Digest256Hasher::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|_| SourceCommandError::Invalid("source-navigation selected image read"))?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(read as u64)
            .ok_or(SourceCommandError::Invalid(
                "source-navigation image size overflow",
            ))?;
        if total > max_bytes {
            return Err(SourceCommandError::Invalid(
                "source-navigation selected image byte cap",
            ));
        }
        hash.update(&buffer[..read]);
    }
    Ok(hash.finalize())
}

fn validate_result(
    result: &SourceNavigationModelCompileResultV1,
    request_digest: &str,
    request: &SourceNavigationModelCompileRequestV2,
    limits: SourceNavigationModelProcessLimits,
) -> SourceCommandResult<()> {
    let stage = &result.stage;
    let expectation = &result.expectation;
    // The final model binds the managed source proof and verified collections,
    // whereas the request binds the immutable export used to build that model.
    // Derive the same receipt as the producer instead of comparing those two
    // intentionally distinct identities.
    let snapshot = tos_compiler::native_snapshot_manifest::portable_native_snapshot_limits(
        request
            .model_limits
            .max_total_ms
            .min(CHILD_MAX_TOTAL_MS)
            .div_ceil(1_000),
    )
    .map_err(|_| SourceCommandError::Invalid("source-navigation model receipt limits"))?;
    let expected_binding = tos_compiler::managed_agent_producer::managed_model_receipt_binding(
        &request.source_binding,
        &request.source_proof_v2,
        &stage.verified_inputs,
        snapshot.full.seal.max_header_bytes,
    )
    .map_err(|_| SourceCommandError::Conflict("source-navigation managed receipt binding"))?;
    if result.schema_version != RESULT_SCHEMA
        || result.raw_request_sha256 != request_digest
        || result.candidate_leaf != CANDIDATE_LEAF
        || stage.source_cut != expected_binding.source_cut
        || stage.membership_root != expected_binding.membership_root
        || stage.input_collections != stage.verified_inputs.len()
        || !same_binding(&stage.binding, &expected_binding)
        || expectation.source_cut != stage.source_cut
        || expectation.membership_root != stage.membership_root
        || expectation.through_commit_seq != request.source_binding.through_commit_seq
        || expectation.route_map_version != request.source_binding.route_map_version
        || expectation.index_generation != request.source_binding.index_generation
        || expectation.model_sha256 != stage.sqlite_sha256
        || expectation.model_size_bytes != stage.sqlite_size_bytes
        || stage.sqlite_size_bytes == 0
        || stage.sqlite_size_bytes > limits.max_model_bytes
        || !expectation.complete
        || result.navigation_original.source_cut != stage.source_cut
        || result.navigation_original.membership_root != stage.membership_root
        || result.artifact_fs_verity.algorithm != "sha256"
    {
        return Err(SourceCommandError::Conflict(
            "source-navigation compile child receipt binding differs",
        ));
    }
    Digest256::from_hex(&expectation.model_sha256)
        .map_err(|_| SourceCommandError::Invalid("source-navigation model digest"))?;
    Digest256::from_hex(&result.artifact_fs_verity.digest)
        .map_err(|_| SourceCommandError::Invalid("source-navigation fs-verity measurement"))?;
    Ok(())
}

fn verify_candidate_readback(
    request: &SourceNavigationModelCompileRequestV2,
    result: &SourceNavigationModelCompileResultV1,
    limits: SourceNavigationModelProcessLimits,
) -> SourceCommandResult<()> {
    let path = request.artifact_staging_root.join(CANDIDATE_LEAF);
    let pinned = tos_fd_open::open_absolute_regular(&path, limits.max_model_bytes)
        .map_err(|_| SourceCommandError::Conflict("source-navigation child candidate absent"))?;
    let metadata = pinned
        .metadata()
        .map_err(|_| SourceCommandError::Invalid("source-navigation child candidate metadata"))?;
    if metadata.len() != result.stage.sqlite_size_bytes {
        return Err(SourceCommandError::Conflict(
            "source-navigation child candidate size differs",
        ));
    }
    let actual = hash_file(
        pinned
            .try_clone()
            .map_err(|_| SourceCommandError::Invalid("source-navigation child candidate"))?,
        limits.max_model_bytes,
    )?;
    let expected = Digest256::from_hex(&result.expectation.model_sha256)
        .map_err(|_| SourceCommandError::Invalid("source-navigation child candidate digest"))?;
    if actual != expected || actual.to_hex() != result.stage.sqlite_sha256 {
        return Err(SourceCommandError::Conflict(
            "source-navigation child candidate readback digest differs",
        ));
    }
    let measured = tos_compiler::measure_existing_native_knowledge_artifact_custody(
        &path,
        &result.expectation,
    )
    .map_err(|_| SourceCommandError::Conflict("source-navigation child candidate lacks custody"))?;
    if measured != result.artifact_fs_verity || !same_named_regular(&path, &pinned)? {
        return Err(SourceCommandError::Conflict(
            "source-navigation child candidate custody changed",
        ));
    }
    Ok(())
}

fn same_binding(
    actual: &tos_compiler::SourceBinding,
    expected: &tos_compiler::SourceBinding,
) -> bool {
    actual.owner_profile == expected.owner_profile
        && actual.source_cut == expected.source_cut
        && actual.through_commit_seq == expected.through_commit_seq
        && actual.membership_root == expected.membership_root
        && actual.index_generation == expected.index_generation
        && actual.route_map_version == expected.route_map_version
        && actual.reader_abi == expected.reader_abi
        && actual.projection_root_sha256 == expected.projection_root_sha256
        && actual.complete == expected.complete
}

fn normalized_absolute(path: &Path) -> SourceCommandResult<PathBuf> {
    if !path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::RootDir | Component::Normal(_)))
    {
        return Err(SourceCommandError::Invalid(
            "source-navigation expected normalized absolute path",
        ));
    }
    Ok(path.to_path_buf())
}

fn verify_owned_private_directory(file: &File, detail: &'static str) -> SourceCommandResult<()> {
    let metadata = file
        .metadata()
        .map_err(|_| SourceCommandError::Invalid(detail))?;
    if !metadata.is_dir()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o077 != 0
    {
        return Err(SourceCommandError::Denied(detail));
    }
    Ok(())
}

fn same_named_directory(path: &Path, pinned: &File) -> SourceCommandResult<()> {
    let named = tos_fd_open::open_absolute_directory(path).map_err(|_| {
        SourceCommandError::Conflict("source-navigation directory identity changed")
    })?;
    if !same_identity(&named, pinned)? {
        return Err(SourceCommandError::Conflict(
            "source-navigation directory identity changed",
        ));
    }
    Ok(())
}

fn same_named_regular(path: &Path, pinned: &File) -> SourceCommandResult<bool> {
    let named = tos_fd_open::open_absolute_regular(path, u64::MAX)
        .map_err(|_| SourceCommandError::Conflict("source-navigation candidate path changed"))?;
    let left = pinned
        .metadata()
        .map_err(|_| SourceCommandError::Invalid("source-navigation candidate metadata"))?;
    let right = named
        .metadata()
        .map_err(|_| SourceCommandError::Invalid("source-navigation candidate metadata"))?;
    Ok(left.dev() == right.dev() && left.ino() == right.ino())
}

fn same_identity(left: &File, right: &File) -> SourceCommandResult<bool> {
    let left = left
        .metadata()
        .map_err(|_| SourceCommandError::Invalid("source-navigation directory metadata"))?;
    let right = right
        .metadata()
        .map_err(|_| SourceCommandError::Invalid("source-navigation directory metadata"))?;
    Ok(left.dev() == right.dev() && left.ino() == right.ino())
}

fn directory_is_empty(path: &Path) -> SourceCommandResult<bool> {
    let entries = fs::read_dir(path)
        .map_err(|_| SourceCommandError::Invalid("source-navigation fresh stage read"))?;
    let mut count = 0usize;
    for entry in entries {
        entry.map_err(|_| SourceCommandError::Invalid("source-navigation fresh stage read"))?;
        count += 1;
        if count > 1 {
            return Ok(false);
        }
    }
    Ok(count == 0)
}

fn paths_disjoint(left: &Path, right: &Path) -> bool {
    !left.starts_with(right) && !right.starts_with(left)
}

fn fd_path(fd: RawFd) -> PathBuf {
    PathBuf::from(format!("/proc/self/fd/{fd}"))
}

fn cgroup_open(directory: &File, name: &str, access: libc::c_int) -> SourceCommandResult<File> {
    let name = CString::new(name)
        .map_err(|_| SourceCommandError::Invalid("source-navigation cgroup file name"))?;
    let fd = unsafe {
        libc::openat(
            directory.as_raw_fd(),
            name.as_ptr(),
            access | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        )
    };
    if fd < 0 {
        return Err(SourceCommandError::Denied(
            "source-navigation delegated cgroup file unavailable",
        ));
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}

fn cgroup_text(directory: &File, name: &str, cap: usize) -> SourceCommandResult<String> {
    let mut file = cgroup_open(directory, name, libc::O_RDONLY)?;
    let mut bytes = Vec::with_capacity(cap.min(4096));
    file.take(cap as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| SourceCommandError::Invalid("source-navigation cgroup read"))?;
    if bytes.len() > cap {
        return Err(SourceCommandError::Invalid(
            "source-navigation cgroup evidence byte cap",
        ));
    }
    String::from_utf8(bytes)
        .map_err(|_| SourceCommandError::Invalid("source-navigation cgroup evidence encoding"))
}

fn cgroup_scalar(directory: &File, name: &str) -> SourceCommandResult<u64> {
    cgroup_text(directory, name, 64)?
        .trim()
        .parse::<u64>()
        .map_err(|_| SourceCommandError::Invalid("source-navigation cgroup scalar"))
}

fn cgroups_empty(held: &HeldCgroups) -> SourceCommandResult<bool> {
    cgroups_empty_inner(&held.common, &held.setup, &held.consumer)
}

fn cgroups_empty_inner(common: &File, setup: &File, consumer: &File) -> SourceCommandResult<bool> {
    Ok(cgroup_text(common, "cgroup.procs", 4096)?.trim().is_empty()
        && cgroup_text(setup, "cgroup.procs", 4096)?.trim().is_empty()
        && cgroup_text(consumer, "cgroup.procs", 4096)?
            .trim()
            .is_empty()
        && cgroup_population(common)? == Some(false)
        && cgroup_population(setup)? == Some(false)
        && cgroup_population(consumer)? == Some(false))
}

fn cgroup_population(directory: &File) -> SourceCommandResult<Option<bool>> {
    let text = cgroup_text(directory, "cgroup.events", 4096)?;
    Ok(text
        .lines()
        .find_map(|line| match line.strip_prefix("populated ") {
            Some("0") => Some(false),
            Some("1") => Some(true),
            _ => None,
        }))
}

fn cleanup_cgroups(held: &HeldCgroups) -> bool {
    let mut complete = true;
    for directory in [&held.setup, &held.consumer] {
        let result = cgroup_open(directory, "cgroup.kill", libc::O_WRONLY).and_then(|mut kill| {
            kill.write_all(b"1\n")
                .map_err(|_| SourceCommandError::Invalid("source-navigation cgroup kill"))
        });
        complete &= result.is_ok();
    }
    complete
}

fn signal_group(pid: u32, signal: libc::c_int) {
    let Ok(pid) = i32::try_from(pid) else {
        return;
    };
    unsafe {
        libc::kill(-pid, signal);
    }
}

fn monotonic_ns() -> SourceCommandResult<u64> {
    let mut now = unsafe { std::mem::zeroed::<libc::timespec>() };
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut now) } != 0
        || now.tv_sec < 0
        || now.tv_nsec < 0
    {
        return Err(SourceCommandError::Invalid(
            "source-navigation monotonic clock unavailable",
        ));
    }
    (now.tv_sec as u64)
        .checked_mul(1_000_000_000)
        .and_then(|seconds| seconds.checked_add(now.tv_nsec as u64))
        .ok_or(SourceCommandError::Invalid(
            "source-navigation monotonic clock overflow",
        ))
}

fn set_nonblocking(pipes: &PipeState) -> SourceCommandResult<()> {
    for fd in [
        pipes.stdin.as_ref().map(AsRawFd::as_raw_fd),
        pipes.stdout.as_ref().map(AsRawFd::as_raw_fd),
        pipes.stderr.as_ref().map(AsRawFd::as_raw_fd),
    ]
    .into_iter()
    .flatten()
    {
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err(SourceCommandError::Invalid(
                "source-navigation child pipe nonblocking setup",
            ));
        }
    }
    Ok(())
}

fn pump_pipes(pipes: &mut PipeState) -> SourceCommandResult<()> {
    let mut poll_fds = [
        libc::pollfd {
            fd: pipes.stdin.as_ref().map_or(-1, AsRawFd::as_raw_fd),
            events: if pipes.stdin.is_some() && pipes.input_offset < pipes.input.len() {
                libc::POLLOUT
            } else {
                0
            },
            revents: 0,
        },
        libc::pollfd {
            fd: pipes.stdout.as_ref().map_or(-1, AsRawFd::as_raw_fd),
            events: libc::POLLIN,
            revents: 0,
        },
        libc::pollfd {
            fd: pipes.stderr.as_ref().map_or(-1, AsRawFd::as_raw_fd),
            events: libc::POLLIN,
            revents: 0,
        },
    ];
    let polled = unsafe { libc::poll(poll_fds.as_mut_ptr(), poll_fds.len() as _, 0) };
    if polled < 0 {
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(SourceCommandError::Invalid(
                "source-navigation child pipe poll failed",
            ));
        }
    }
    if pipes.stdin.is_some() && pipes.input_offset == pipes.input.len() {
        pipes.stdin.take();
    } else if poll_fds[0].revents & (libc::POLLOUT | libc::POLLERR | libc::POLLHUP) != 0 {
        let Some(stdin) = pipes.stdin.as_ref() else {
            return Ok(());
        };
        let left = &pipes.input[pipes.input_offset..];
        let written = unsafe { libc::write(stdin.as_raw_fd(), left.as_ptr().cast(), left.len()) };
        if written > 0 {
            pipes.input_offset += written as usize;
            if pipes.input_offset == pipes.input.len() {
                pipes.stdin.take();
            }
        } else if written == 0 {
            pipes.input_failed = true;
        } else {
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::WouldBlock
                && error.kind() != io::ErrorKind::Interrupted
            {
                pipes.input_failed = true;
            }
        }
    }
    read_pipe(
        &mut pipes.stdout,
        &mut pipes.output,
        RESPONSE_MAX_BYTES,
        &mut pipes.output_oversized,
        PIPE_READ_SLICE_BYTES,
    )?;
    read_pipe(
        &mut pipes.stderr,
        &mut pipes.stderr_output,
        STDERR_RETAIN_BYTES,
        &mut pipes.stderr_oversized,
        PIPE_READ_SLICE_BYTES,
    )?;
    // The actual text is untrusted and is never returned to the owner.
    Ok(())
}

fn forward_validated_phase_trace(stderr: &[u8], oversized: bool) {
    let markers = validated_phase_trace(stderr, oversized).unwrap_or_default();
    let search_refusal = validated_search_work_refusal(stderr, oversized);
    let stderr = io::stderr();
    let mut output = stderr.lock();
    for marker in markers {
        let _ = writeln!(output, "{marker}");
    }
    if let Some(marker) = search_refusal {
        let _ = writeln!(output, "{marker}");
    }
    let _ = output.flush();
}

// Forward only the compiler's numeric work refusal. Never relay a child path,
// source text, arbitrary stderr, or an ambiguous duplicate diagnostic.
fn validated_search_work_refusal(stderr: &[u8], oversized: bool) -> Option<String> {
    if oversized || stderr.len() > STDERR_RETAIN_BYTES {
        return None;
    }
    let mut matched = None;
    for line in std::str::from_utf8(stderr).ok()?.lines() {
        let Some(value) = line.strip_prefix("Native search work refused at ") else {
            continue;
        };
        if matched.is_some() || line.len() > 256 {
            return None;
        }
        let value = value.strip_prefix("rust/crates/tos-compiler/src/knowledge_search.rs:")?;
        let (line_number, fields) = value.split_once(": ")?;
        let number = |value: &str| -> Option<u64> {
            if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
                return None;
            }
            value.parse().ok()
        };
        let line_number = number(line_number)?;
        let mut fields = fields.split(' ');
        let charged = number(fields.next()?.strip_prefix("charged_bytes=")?)?;
        let next = number(fields.next()?.strip_prefix("next_bytes=")?)?;
        let limit = number(fields.next()?.strip_prefix("work_limit=")?)?;
        if fields.next().is_some()
            || line_number == 0
            || line_number > 100_000
            || next == 0
            || limit == 0
            || charged > limit
            || charged.checked_add(next)? <= limit
        {
            return None;
        }
        matched = Some(format!(
            "tos_source_navigation_search_work_v1 compiler_line={line_number} charged_bytes={charged} next_bytes={next} work_limit={limit}"
        ));
    }
    matched
}

fn validated_phase_trace(stderr: &[u8], oversized: bool) -> Option<Vec<String>> {
    if oversized || stderr.len() > STDERR_RETAIN_BYTES {
        return None;
    }
    let stderr = std::str::from_utf8(stderr).ok()?;
    let mut markers = Vec::new();
    let mut active_stage: Option<usize> = None;
    let mut last_stage: Option<usize> = None;
    let mut last_elapsed_ms: Option<u64> = None;

    for line in stderr.lines() {
        if !line.starts_with(PHASE_TRACE_TAG) {
            continue;
        }
        if line.len() > PHASE_TRACE_MAX_LINE_BYTES || markers.len() == PHASE_TRACE_MAX_LINES {
            return None;
        }
        let fields: Vec<&str> = line.strip_prefix(PHASE_TRACE_PREFIX)?.split(' ').collect();
        if fields.len() != 3 {
            return None;
        }
        let stage = fields[0].strip_prefix("stage=")?;
        let stage_index = SOURCE_NAVIGATION_PHASE_STAGES
            .iter()
            .position(|candidate| *candidate == stage)?;
        let edge = fields[1].strip_prefix("edge=")?;
        let elapsed_text = fields[2].strip_prefix("elapsed_ms=")?;
        if elapsed_text.is_empty() || !elapsed_text.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        let elapsed_ms = elapsed_text.parse::<u64>().ok()?;
        if elapsed_ms > CHILD_MAX_TOTAL_MS || last_elapsed_ms.is_some_and(|last| elapsed_ms < last)
        {
            return None;
        }

        match edge {
            "begin" => {
                if active_stage.is_some() || last_stage.is_some_and(|last| stage_index <= last) {
                    return None;
                }
                active_stage = Some(stage_index);
                last_stage = Some(stage_index);
            }
            "end" => {
                if active_stage != Some(stage_index) {
                    return None;
                }
                active_stage = None;
            }
            _ => return None,
        }
        last_elapsed_ms = Some(elapsed_ms);
        markers.push(format!(
            "{PHASE_TRACE_PREFIX}stage={stage} edge={edge} elapsed_ms={elapsed_ms}"
        ));
    }
    if markers.is_empty() {
        None
    } else {
        Some(markers)
    }
}

fn sanitized_child_refusal(stderr: &[u8]) -> Option<String> {
    let stderr = std::str::from_utf8(stderr).ok()?;
    let mut matched = None;
    for line in stderr.lines() {
        if !line.starts_with(CHILD_REFUSAL_PREFIX) {
            continue;
        }
        if line.len() > CHILD_REFUSAL_MAX_BYTES
            || line[CHILD_REFUSAL_PREFIX.len()..]
                .bytes()
                .any(|byte| byte.is_ascii_control())
            || matched.is_some()
        {
            return None;
        }
        matched = Some(line.to_owned());
    }
    matched
}

fn read_pipe<T: AsRawFd>(
    pipe: &mut Option<T>,
    output: &mut Vec<u8>,
    cap: usize,
    oversized: &mut bool,
    read_budget: usize,
) -> SourceCommandResult<()> {
    let Some(current) = pipe.as_ref() else {
        return Ok(());
    };
    if output.capacity() == 0 && cap != 0 {
        output.reserve(cap.min(64 * 1024));
    }
    let mut bytes = [0u8; 16 * 1024];
    let mut drained = 0usize;
    loop {
        if drained >= read_budget {
            break;
        }
        let capacity = bytes.len().min(read_budget - drained);
        let read = unsafe { libc::read(current.as_raw_fd(), bytes.as_mut_ptr().cast(), capacity) };
        if read > 0 {
            let read = read as usize;
            drained = drained.saturating_add(read);
            if output.len().saturating_add(read) > cap {
                *oversized = true;
                let retained = cap.saturating_sub(output.len());
                output.extend_from_slice(&bytes[..retained]);
            } else {
                output.extend_from_slice(&bytes[..read]);
            }
            continue;
        }
        if read == 0 {
            pipe.take();
            break;
        }
        let error = io::Error::last_os_error();
        if error.kind() == io::ErrorKind::WouldBlock || error.kind() == io::ErrorKind::Interrupted {
            break;
        }
        return Err(SourceCommandError::Invalid(
            "source-navigation child pipe read failed",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod phase_trace_tests {
    use super::*;

    #[test]
    fn accepts_only_ordered_allowlisted_markers_and_reconstructs_them() {
        let stderr = concat!(
            "private child text is ignored\n",
            "tos_source_navigation_phase_v1 stage=input_cut_verification edge=begin elapsed_ms=1\n",
            "tos_source_navigation_phase_v1 stage=input_cut_verification edge=end elapsed_ms=4\n",
            "tos_source_navigation_phase_v1 stage=catalog_planning edge=begin elapsed_ms=4\n",
        );
        let markers = validated_phase_trace(stderr.as_bytes(), false).expect("valid trace");
        assert_eq!(markers.len(), 3);
        assert_eq!(
            markers[2],
            "tos_source_navigation_phase_v1 stage=catalog_planning edge=begin elapsed_ms=4"
        );
        let search = "Native search work refused at rust/crates/tos-compiler/src/knowledge_search.rs:1062: charged_bytes=63 next_bytes=2 work_limit=64\n";
        assert_eq!(
            validated_search_work_refusal(search.as_bytes(), false).as_deref(),
            Some(
                "tos_source_navigation_search_work_v1 compiler_line=1062 charged_bytes=63 next_bytes=2 work_limit=64"
            )
        );
    }

    #[test]
    fn rejects_unknown_malformed_or_out_of_order_markers() {
        let invalid = [
            "tos_source_navigation_phase_v1 stage=unlisted edge=begin elapsed_ms=1\n",
            "tos_source_navigation_phase_v1 stage=input_cut_verification edge=begin elapsed_ms=1 extra=x\n",
            "tos_source_navigation_phase_v1 stage=input_cut_verification edge=end elapsed_ms=1\n",
            concat!(
                "tos_source_navigation_phase_v1 stage=input_cut_verification edge=begin elapsed_ms=2\n",
                "tos_source_navigation_phase_v1 stage=input_cut_verification edge=end elapsed_ms=1\n",
            ),
        ];
        for stderr in invalid {
            assert!(validated_phase_trace(stderr.as_bytes(), false).is_none());
        }
        let valid = "Native search work refused at rust/crates/tos-compiler/src/knowledge_search.rs:1062: charged_bytes=63 next_bytes=2 work_limit=64\n";
        for invalid in [
            valid.replace(
                "rust/crates/tos-compiler/src/knowledge_search.rs",
                "/private/source.txt",
            ),
            valid.replace("next_bytes=2", "next_bytes=0"),
            valid.replace("charged_bytes=63", "charged_bytes=65"),
            valid.replace("charged_bytes=63", "charged_bytes=1"),
            valid.replace("work_limit=64", "work_limit=secret"),
            format!("{valid}{valid}"),
        ] {
            assert!(validated_search_work_refusal(invalid.as_bytes(), false).is_none());
        }
        assert!(validated_search_work_refusal(valid.as_bytes(), true).is_none());
    }

    #[test]
    fn rejects_oversized_or_truncated_trace_input_and_lines() {
        let valid = concat!(
            "tos_source_navigation_phase_v1 stage=input_cut_verification ",
            "edge=begin elapsed_ms=1\n",
        );
        assert!(validated_phase_trace(valid.as_bytes(), true).is_none());
        let long_line = format!(
            "{}stage=input_cut_verification edge=begin elapsed_ms=1 {}\n",
            PHASE_TRACE_PREFIX,
            "x".repeat(PHASE_TRACE_MAX_LINE_BYTES)
        );
        assert!(validated_phase_trace(long_line.as_bytes(), false).is_none());
    }
}
