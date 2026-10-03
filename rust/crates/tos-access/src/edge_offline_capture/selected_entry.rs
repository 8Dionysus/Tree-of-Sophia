//! Supervised-host CLI adapter over the borrowed native capture operation.
//! Selected transactions are owned here; library callers keep their stronger
//! borrowed custody contract. No successful result is emitted before cleanup.

use super::native_selected_capture::{CaptureLimits, SelectedView, capture_selected_native};
use crate::software_archive::installed::{
    InstalledAccessBudget, InstalledAccessLimits, SelectedRole, verify_running_role,
};
use std::{
    collections::BTreeMap,
    ffi::CString,
    fs::File,
    io::{Read, Write},
    os::fd::AsRawFd,
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
use tos_foundation::{
    JsonLimits, JsonMode, JsonValue, emit_value_preserved_json, parse_json_with_state_budget,
};

type Result<T> = std::result::Result<T, String>;
const REQUEST_BYTES: usize = 10 * 1024 * 1024;
const REQUEST_STATE: usize = 64 * 1024 * 1024;
const FACT_BYTES: usize = 8192;

fn limits(bytes: usize) -> JsonLimits {
    JsonLimits {
        max_bytes: bytes,
        max_depth: 128,
        max_visits: 1_000_000,
        max_integer_digits: 4300,
    }
}
fn field<'a>(value: &'a JsonValue, name: &str) -> Result<&'a JsonValue> {
    value
        .object_get(name)
        .ok_or_else(|| format!("selected entry missing {name}"))
}
fn text<'a>(value: &'a JsonValue, name: &str) -> Result<&'a str> {
    field(value, name)?
        .as_str()
        .ok_or_else(|| format!("selected entry string {name}"))
}
fn uint(value: &JsonValue, name: &str) -> Result<u64> {
    field(value, name)?
        .as_u64()
        .filter(|v| *v > 0)
        .ok_or_else(|| format!("selected entry positive integer {name}"))
}
fn size(value: &JsonValue, name: &str) -> Result<usize> {
    usize::try_from(uint(value, name)?).map_err(|_| format!("selected entry width {name}"))
}
fn exact(value: &JsonValue, names: &[&str]) -> Result<()> {
    let entries = value.as_object().ok_or("selected entry object required")?;
    if entries.len() != names.len() || names.iter().any(|name| value.object_get(name).is_none()) {
        return Err("selected entry exact fields required".into());
    }
    Ok(())
}
fn active(deadline: Instant) -> Result<()> {
    if Instant::now() >= deadline {
        Err("selected entry original deadline elapsed".into())
    } else {
        Ok(())
    }
}
fn directory(value: &str, deadline: Instant) -> Result<PathBuf> {
    active(deadline)?;
    let path = PathBuf::from(value);
    if !path.is_absolute()
        || path.components().any(|part| {
            matches!(
                part,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
    {
        return Err("selected entry absolute directory required".into());
    }
    tos_fd_open::open_absolute_directory(&path).map_err(|e| e.to_string())?;
    active(deadline)?;
    Ok(path)
}
fn fd_count(cap: usize, deadline: Instant) -> Result<usize> {
    let mut count = 0usize;
    for entry in std::fs::read_dir("/proc/self/fd").map_err(|e| e.to_string())? {
        active(deadline)?;
        entry.map_err(|e| e.to_string())?;
        count = count.checked_add(1).ok_or("selected entry FD overflow")?;
        if count > cap {
            return Err("selected entry whole FD inventory bound".into());
        }
    }
    Ok(count)
}
fn read(path: &Path, cap: usize, deadline: Instant) -> Result<Vec<u8>> {
    active(deadline)?;
    let file = File::open(path).map_err(|e| e.to_string())?;
    let mut raw = Vec::new();
    file.take(cap as u64 + 1)
        .read_to_end(&mut raw)
        .map_err(|e| e.to_string())?;
    active(deadline)?;
    if raw.len() > cap {
        return Err("selected entry metadata byte bound".into());
    }
    Ok(raw)
}
fn kernel_cgroup(pid: u32, deadline: Instant) -> Result<String> {
    let raw = read(
        Path::new(&format!("/proc/{pid}/cgroup")),
        FACT_BYTES,
        deadline,
    )?;
    let text = std::str::from_utf8(&raw).map_err(|_| "selected entry cgroup encoding")?;
    let mut lines = text.lines();
    let group = lines
        .next()
        .and_then(|line| line.strip_prefix("0::"))
        .ok_or("selected entry unified cgroup required")?;
    if lines.next().is_some() || !group.starts_with('/') || group == "/" {
        return Err("selected entry exclusive service cgroup required".into());
    }
    Ok(group.to_owned())
}
fn process(pid: u32, deadline: Instant) -> Result<(u32, u64)> {
    let raw = read(
        Path::new(&format!("/proc/{pid}/stat")),
        FACT_BYTES,
        deadline,
    )?;
    let raw = std::str::from_utf8(&raw).map_err(|_| "selected entry process encoding")?;
    let fields = raw
        .rsplit_once(") ")
        .ok_or("selected entry process stat")?
        .1
        .split_whitespace()
        .collect::<Vec<_>>();
    let parent = fields
        .get(1)
        .ok_or("selected entry process parent")?
        .parse()
        .map_err(|_| "selected entry process parent integer")?;
    let start = fields
        .get(19)
        .ok_or("selected entry process start")?
        .parse()
        .map_err(|_| "selected entry process start integer")?;
    Ok((parent, start))
}
// systemctl formats usec-valued properties as finite timespans. Accept exact
// integer/fixed-decimal components; infinity and unrecognised units refuse.
fn timespan(raw: &str) -> Result<u64> {
    if raw == "0" {
        return Ok(0);
    }
    let mut total = 0u64;
    for part in raw.split_whitespace() {
        let at = part
            .find(|c: char| !c.is_ascii_digit() && c != '.')
            .ok_or("selected entry service duration unit")?;
        let (number, unit) = part.split_at(at);
        let factor = match unit {
            "us" => 1,
            "ms" => 1000,
            "s" => 1_000_000,
            "min" => 60_000_000,
            "h" => 3_600_000_000,
            "d" => 86_400_000_000,
            _ => return Err("selected entry service duration unit".into()),
        };
        let (whole, fraction) = number.split_once('.').unwrap_or((number, ""));
        if fraction.len() > 6 || !fraction.bytes().all(|b| b.is_ascii_digit()) {
            return Err("selected entry service duration precision".into());
        }
        let whole: u64 = whole
            .parse()
            .map_err(|_| "selected entry service duration integer")?;
        let fraction: u64 = if fraction.is_empty() {
            0
        } else {
            fraction
                .parse()
                .map_err(|_| "selected entry service duration fraction")?
        };
        let denominator = 10u64.pow(number.split_once('.').map_or(0, |(_, f)| f.len()) as u32);
        let scaled = fraction
            .checked_mul(factor)
            .ok_or("selected entry duration overflow")?;
        if scaled % denominator != 0 {
            return Err("selected entry submicrosecond service duration".into());
        }
        total = total
            .checked_add(
                whole
                    .checked_mul(factor)
                    .and_then(|v| v.checked_add(scaled / denominator))
                    .ok_or("selected entry duration overflow")?,
            )
            .ok_or("selected entry duration overflow")?;
    }
    Ok(total)
}

fn service_properties(
    unit: &str,
    deadline: Instant,
    cleanup: Instant,
) -> Result<BTreeMap<String, String>> {
    if !unit.ends_with(".service")
        || unit.len() > 255
        || !unit
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-@".contains(&b))
    {
        return Err("selected entry explicit supervised service name required".into());
    }
    active(deadline)?;
    let mut command = Command::new("/usr/bin/systemctl");
    let parent = unsafe { libc::getpid() };
    // Trusted host metadata tool receives no selected data or software FDs.
    // Death of this adapter requests its death even before facts are admitted.
    unsafe {
        command.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) != 0
                || libc::getppid() != parent
            {
                return Err(std::io::Error::other("supervisor probe death guard"));
            }
            Ok(())
        });
    }
    let mut child = command
        .args([
            "--user",
            "--no-pager",
            "show",
            "-p",
            "ControlGroup",
            "-p",
            "InvocationID",
            "-p",
            "MainPID",
            "-p",
            "KillMode",
            "-p",
            "SendSIGKILL",
            "-p",
            "Restart",
            "-p",
            "RuntimeMaxUSec",
            "-p",
            "RuntimeRandomizedExtraUSec",
            "-p",
            "TimeoutStopUSec",
            "-p",
            "ActiveEnterTimestampMonotonic",
            "--",
            unit,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let mut pipe = child.stdout.take().ok_or("selected entry service pipe")?;
    let fd = pipe.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    let setup =
        flags >= 0 && unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } >= 0;
    let outcome = (|| -> Result<Vec<u8>> {
        if !setup {
            return Err("selected entry service pipe flags".into());
        }
        let mut raw = Vec::new();
        let mut eof = false;
        loop {
            active(deadline)?;
            let mut buffer = [0; 1024];
            match pipe.read(&mut buffer) {
                Ok(0) => eof = true,
                Ok(n) => {
                    if raw.len() + n > FACT_BYTES {
                        return Err("selected entry service metadata bound".into());
                    }
                    raw.extend_from_slice(&buffer[..n]);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e.to_string()),
            }
            if eof && let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                if !status.success() {
                    return Err("selected entry supervisor service unavailable".into());
                }
                return Ok(raw);
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    })();
    if outcome.is_err() {
        let _ = child.kill();
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                _ if Instant::now() < cleanup => std::thread::sleep(Duration::from_millis(2)),
                _ => {
                    let _ = writeln!(
                        std::io::stderr(),
                        "selected entry supervisor probe cleanup unresolved; no capture/adoption; pid={}",
                        child.id()
                    );
                    unsafe { libc::_exit(125) }
                }
            }
        }
    }
    let raw = outcome?;
    let raw = std::str::from_utf8(&raw).map_err(|_| "selected entry service metadata encoding")?;
    let mut result = BTreeMap::new();
    for line in raw.lines() {
        let (key, value) = line
            .split_once('=')
            .ok_or("selected entry service metadata shape")?;
        if result.insert(key.to_owned(), value.to_owned()).is_some() {
            return Err("selected entry duplicate service metadata".into());
        }
    }
    Ok(result)
}

#[derive(Eq, PartialEq)]
struct Supervisor {
    properties: BTreeMap<String, String>,
    main_start: u64,
}
fn supervisor(
    unit: &str,
    work: Instant,
    cleanup: Instant,
    supervisor_total_ms: u64,
    cleanup_ms: u64,
) -> Result<Supervisor> {
    let properties = service_properties(unit, work, cleanup)?;
    let get = |name: &str| {
        properties
            .get(name)
            .map(String::as_str)
            .ok_or_else(|| format!("selected entry supervisor missing {name}"))
    };
    if get("KillMode")? != "control-group"
        || get("SendSIGKILL")? != "yes"
        || get("Restart")? != "no"
        || get("RuntimeRandomizedExtraUSec")? != "0"
    {
        return Err("selected entry supervised-host cleanup capability unavailable".into());
    }
    let invocation = get("InvocationID")?;
    if invocation.len() != 32 || !invocation.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("selected entry active supervisor invocation required".into());
    }
    let runtime = timespan(get("RuntimeMaxUSec")?)?;
    let stop = timespan(get("TimeoutStopUSec")?)?;
    let started: u64 = get("ActiveEnterTimestampMonotonic")?
        .parse()
        .map_err(|_| "selected entry service start integer")?;
    let mut now = unsafe { std::mem::zeroed::<libc::timespec>() };
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut now) } != 0 {
        return Err("selected entry monotonic clock".into());
    }
    let now = u64::try_from(now.tv_sec)
        .ok()
        .and_then(|s| s.checked_mul(1_000_000))
        .and_then(|s| s.checked_add(now.tv_nsec as u64 / 1000))
        .ok_or("selected entry clock overflow")?;
    if runtime == 0
        || runtime > supervisor_total_ms * 1000
        || stop == 0
        || stop > cleanup_ms * 1000
        || started == 0
        || started.checked_add(runtime).is_none_or(|end| end <= now)
    {
        return Err("selected entry finite live supervisor lifetime required".into());
    }
    let group = get("ControlGroup")?;
    let own = std::process::id();
    if kernel_cgroup(own, work)? != group {
        return Err("selected entry supervisor/kernel cgroup mismatch".into());
    }
    let main: u32 = get("MainPID")?
        .parse()
        .map_err(|_| "selected entry supervisor main pid")?;
    if main <= 1 || kernel_cgroup(main, work)? != group {
        return Err("selected entry supervisor main cgroup mismatch".into());
    }
    let (_, main_start) = process(main, work)?;
    let mut current = own;
    let mut found = false;
    for _ in 0..64 {
        if current == main {
            found = true;
            break;
        }
        current = process(current, work)?.0;
        if current <= 1 {
            break;
        }
    }
    if !found {
        return Err("selected entry process is outside supervisor ancestry".into());
    }
    Ok(Supervisor {
        properties,
        main_start,
    })
}

pub(super) fn run(path: &str, stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    let original = Instant::now();
    let outcome = (|| -> Result<()> {
        let preliminary = original + Duration::from_millis(1_200_000);
        let path = super::input_path(path)?;
        let file = tos_fd_open::open_absolute_regular(&path, REQUEST_BYTES as u64)
            .map_err(|e| e.to_string())?;
        let mut raw = Vec::new();
        file.take(REQUEST_BYTES as u64 + 1)
            .read_to_end(&mut raw)
            .map_err(|e| e.to_string())?;
        active(preliminary)?;
        let envelope = parse_json_with_state_budget(
            &raw,
            JsonMode::PublishedStrict,
            limits(REQUEST_BYTES),
            REQUEST_STATE,
        )
        .map_err(|_| "selected entry request JSON")?
        .into_root();
        exact(
            &envelope,
            &[
                "schema",
                "capture",
                "selected_prefix",
                "scratch",
                "transport_limits",
                "installed_limits",
                "work_timeout_ms",
                "cleanup_timeout_ms",
                "supervisor_unit",
                "supervisor_total_ms",
            ],
        )?;
        if text(&envelope, "schema")? != "tos_edge_selected_capture_request_v1" {
            return Err("selected entry request schema".into());
        }
        let work_ms = uint(&envelope, "work_timeout_ms")?;
        let cleanup_ms = uint(&envelope, "cleanup_timeout_ms")?;
        let supervisor_total_ms = uint(&envelope, "supervisor_total_ms")?;
        if work_ms > 1_200_000 || cleanup_ms > 10_000 || supervisor_total_ms > 1_210_000 {
            return Err("selected entry clock bounds".into());
        }
        let work = original
            .checked_add(Duration::from_millis(work_ms))
            .ok_or("selected entry work clock overflow")?;
        let cleanup = work
            .checked_add(Duration::from_millis(cleanup_ms))
            .ok_or("selected entry cleanup clock overflow")?;
        active(work)?;
        let unit = text(&envelope, "supervisor_unit")?;
        let capture = field(&envelope, "capture")?;
        if text(capture, "schema")? != "tos_edge_offline_capture_request_v1" {
            return Err("selected entry original v1 capture required".into());
        }
        let transport = field(&envelope, "transport_limits")?;
        exact(
            transport,
            &[
                "frame_bytes",
                "schema_bytes",
                "request_state_bytes",
                "result_state_bytes",
                "stream_bytes",
                "metadata_bytes",
                "state_bytes",
                "io_bytes",
                "held_fds",
                "census_entries",
            ],
        )?;
        let mut capture_limits = CaptureLimits {
            frame_bytes: uint(transport, "frame_bytes")?,
            schema_bytes: uint(transport, "schema_bytes")?,
            request_state_bytes: size(transport, "request_state_bytes")?,
            result_state_bytes: size(transport, "result_state_bytes")?,
            stream_bytes: size(transport, "stream_bytes")?,
            metadata_bytes: size(transport, "metadata_bytes")?,
            state_bytes: size(transport, "state_bytes")?,
            io_bytes: uint(transport, "io_bytes")?,
            held_fds: size(transport, "held_fds")?,
            census_entries: size(transport, "census_entries")?,
        };
        let installed = field(&envelope, "installed_limits")?;
        exact(
            installed,
            &[
                "max_image_bytes",
                "max_metadata_bytes",
                "max_state_bytes",
                "max_io_bytes",
                "max_held_fds",
            ],
        )?;
        let mut budget = InstalledAccessBudget::new(InstalledAccessLimits {
            max_image_bytes: uint(installed, "max_image_bytes")?,
            max_metadata_bytes: size(installed, "max_metadata_bytes")?,
            max_state_bytes: size(installed, "max_state_bytes")?,
            max_io_bytes: uint(installed, "max_io_bytes")?,
            max_held_fds: size(installed, "max_held_fds")?,
        })?;
        // Reserve this adapter's coexisting envelope/parser/ordered-request,
        // owner-limit conversion and bounded platform metadata state before
        // handing the remainder of the same cap to the native ledger. SQLite
        // and allocator RSS still require the host's physical memory bound.
        let wrapper_state = REQUEST_STATE
            .checked_add(2 * REQUEST_BYTES + 4 * 1_048_576 + 131_072)
            .and_then(|n| {
                capture_limits
                    .held_fds
                    .checked_mul(128)
                    .and_then(|fds| n.checked_add(fds))
            })
            .ok_or("selected entry wrapper state reservation overflow")?;
        capture_limits.state_bytes = capture_limits
            .state_bytes
            .checked_sub(wrapper_state)
            .filter(|remaining| *remaining > 0)
            .ok_or("selected entry cumulative wrapper/native state budget")?;
        // Envelope, limit conversion and two full bounded kernel/service
        // metadata/ancestry passes. Installed payload rehashes retain their
        // own installed budget and are not silently absorbed into this cap.
        let wrapper_io = (REQUEST_BYTES + 1_048_576 + 2 * FACT_BYTES * (64 + 5)) as u64;
        capture_limits.io_bytes = capture_limits
            .io_bytes
            .checked_sub(wrapper_io)
            .filter(|remaining| *remaining > 0)
            .ok_or("selected entry cumulative wrapper/native IO budget")?;
        if fd_count(capture_limits.held_fds, work)?
            .checked_add(6)
            .is_none_or(|n| n > capture_limits.held_fds)
        {
            return Err("selected entry service probe FD preflight".into());
        }
        let pinned_supervisor = supervisor(unit, work, cleanup, supervisor_total_ms, cleanup_ms)?;
        let prefix = directory(text(&envelope, "selected_prefix")?, work)?;
        let scratch = directory(text(&envelope, "scratch")?, work)?;
        let mut image = verify_running_role(
            &prefix,
            SelectedRole::Access,
            cleanup,
            &mut || active(work),
            &mut budget,
        )?;
        // Reuse the capture owner's declared VM/value profile; do not create
        // an independently evolving schema or SQLite limits registry here.
        let owner_limits_raw =
            emit_value_preserved_json(field(capture, "limits")?, limits(1_048_576))
                .map_err(|_| "selected entry owner limits encoding")?;
        let owner_limits_value = serde_json::from_slice(&owner_limits_raw)
            .map_err(|_| "selected entry owner limits JSON")?;
        let owner_limits = super::limits(&owner_limits_value, text(capture, "operation")?)?;
        let vm_steps = owner_limits
            .pair
            .max_work_bytes
            .min(100_000_000)
            .max(100_000);
        let value_bytes = owner_limits
            .prepared
            .max_row_bytes
            .max(owner_limits.prepared.max_metadata_bytes);
        let selected_count = [
            "d1_database",
            "before_prepared_database",
            "after_prepared_database",
        ]
        .into_iter()
        .filter(|name| {
            capture
                .object_get(name)
                .is_some_and(|value| !matches!(value, JsonValue::Null))
        })
        .count();
        // Main SQLite + identity guard + possible WAL/SHM, all frames,
        // native internal descriptors, safe-open/running-image temporaries,
        // and the service probe's spawn/pipe/null-stdio descriptors.
        let reserved_fds = fd_count(capture_limits.held_fds, work)?
            .checked_add(selected_count * 5 + 16 + 3 + 6)
            .ok_or("selected entry whole FD count overflow")?;
        if reserved_fds > capture_limits.held_fds {
            return Err("selected entry whole wrapper FD budget".into());
        }
        let mut connections = Vec::<(String, super::HeldSqlite)>::new();
        for name in [
            "d1_database",
            "before_prepared_database",
            "after_prepared_database",
        ] {
            if let Some(value) = capture.object_get(name)
                && !matches!(value, JsonValue::Null)
            {
                let path =
                    super::input_path(value.as_str().ok_or("selected entry database path")?)?;
                active(work)?;
                let db =
                    super::open_read_only_with_deadline(&path, vm_steps, value_bytes, Some(work))?;
                fd_count(capture_limits.held_fds, work)?;
                db.connection
                    .busy_timeout(Duration::ZERO)
                    .map_err(|e| e.to_string())?;
                db.connection
                    .execute_batch("PRAGMA query_only=ON; BEGIN;")
                    .map_err(|e| e.to_string())?;
                db.connection
                    .query_row("SELECT count(*) FROM main.sqlite_schema", [], |row| {
                        row.get::<_, i64>(0)
                    })
                    .map_err(|e| e.to_string())?;
                db.identity.verify_selected_file_identity()?;
                active(work)?;
                connections.push((name.to_owned(), db));
            }
        }
        let views = connections
            .iter()
            .map(|(input_field, connection)| SelectedView {
                input_field,
                connection: &connection.connection,
            })
            .collect::<Vec<_>>();
        let ordered = emit_value_preserved_json(capture, limits(REQUEST_BYTES))
            .map_err(|_| "selected entry original capture encoding")?;
        let output_cap = capture_limits.stream_bytes;
        let environment = [
            CString::new("LANG=C.UTF-8").unwrap(),
            CString::new("LC_ALL=C.UTF-8").unwrap(),
            CString::new("TZ=UTC").unwrap(),
        ];
        match capture_selected_native(
            &ordered,
            &views,
            &mut image,
            &mut budget,
            &scratch,
            capture_limits,
            work,
            cleanup,
            &|| active(work),
            &environment,
        ) {
            Ok(result) => {
                if supervisor(unit, work, cleanup, supervisor_total_ms, cleanup_ms)?
                    != pinned_supervisor
                {
                    return Err("selected entry supervisor association changed".into());
                }
                for (_, selected) in &connections {
                    selected.identity.verify_selected_file_identity()?;
                }
                let raw = emit_value_preserved_json(&result, limits(output_cap))
                    .map_err(|_| "selected entry result encoding")?;
                active(work)?;
                stdout
                    .write_all(&raw)
                    .and_then(|_| stdout.write_all(b"\n"))
                    .map_err(|e| e.to_string())?;
                Ok(())
            }
            Err(failure) if !failure.child_released => {
                let _ = writeln!(
                    stderr,
                    "selected entry unresolved cleanup: no success/adoption; supervisor_unit={unit}; child_pid={:?}; retained_directory={:?}; metadata_complete={}; reason={}",
                    failure.unreaped_child_pid(),
                    failure.retained_directory,
                    failure.metadata_complete,
                    failure.reason
                );
                let _ = stderr.flush();
                // CLI-only terminal failure law: the kernel closes our owned
                // read transactions on process death. Immutable frames remain.
                // PDEATHSIG requests child death; the selected finite service
                // owns residual cleanup. This is not child-release evidence.
                unsafe { libc::_exit(125) }
            }
            Err(failure) => {
                let _ = writeln!(
                    stderr,
                    "selected entry failed capture: child_released=true; retained_directory={:?}; metadata_complete={}",
                    failure.retained_directory, failure.metadata_complete
                );
                Err(failure.reason)
            }
        }
    })();
    match outcome {
        Ok(()) => 0,
        Err(reason) => {
            let _ = writeln!(stderr, "selected edge capture: {reason}");
            2
        }
    }
}
