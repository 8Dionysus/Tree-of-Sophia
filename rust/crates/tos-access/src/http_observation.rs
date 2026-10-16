//! Optional owner-controlled HTTP measurements. These are kernel/connection
//! counters, not actor identity, authority, packet disclosure or flush receipts.
use std::cell::RefCell;
use std::fmt::Write as _;
use std::io;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

const MAX_WINDOW_NS: u64 = 3_600_000_000_000;
const MARKER: &str = "TOS_HTTP_OBSERVATION ";

#[derive(Clone, Copy)]
pub struct OwnerDeadline(pub(crate) u64);
impl OwnerDeadline {
    pub(crate) fn startup_probe(
        self,
        inner: Arc<dyn tos_query::AbortProbe>,
    ) -> Arc<dyn tos_query::AbortProbe> {
        Arc::new(OwnerStartupProbe {
            deadline: self,
            inner,
        })
    }
    pub(crate) fn remaining(self) -> io::Result<u64> {
        self.0
            .checked_sub(monotonic_ns()?)
            .filter(|n| *n > 0)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::TimedOut,
                    "HTTP observation original deadline",
                )
            })
    }
}

struct OwnerStartupProbe {
    deadline: OwnerDeadline,
    inner: Arc<dyn tos_query::AbortProbe>,
}
impl tos_query::AbortProbe for OwnerStartupProbe {
    fn reason(&self) -> Option<tos_query::AbortReason> {
        if self.deadline.remaining().is_err() {
            Some(tos_query::AbortReason::DeadlineExceeded)
        } else {
            self.inner.reason()
        }
    }
}

/// Preserve ordinary address parsing; only this explicit owner option opts in.
pub fn parse_serve_options(
    args: &[String],
) -> Result<(String, Option<OwnerDeadline>), crate::AccessError> {
    let invalid =
        |message| crate::AccessError::new(crate::AccessErrorCode::InvalidRequest, message);
    let mut ordinary = Vec::new();
    let mut deadline = None;
    let mut at = 0;
    while at < args.len() {
        let raw = if args[at] == "--observe-stdin-eof-deadline-ns" {
            at += 1;
            Some(
                args.get(at)
                    .ok_or_else(|| invalid("observation deadline value required"))?
                    .as_str(),
            )
        } else {
            args[at].strip_prefix("--observe-stdin-eof-deadline-ns=")
        };
        if let Some(raw) = raw {
            if deadline.is_some() || raw.is_empty() || !raw.bytes().all(|c| c.is_ascii_digit()) {
                return Err(invalid(
                    "one unsigned decimal observation deadline required",
                ));
            }
            let value = OwnerDeadline(
                raw.parse()
                    .map_err(|_| invalid("observation deadline exceeds u64"))?,
            );
            let remaining = value.remaining().map_err(|_| {
                invalid("observation requires a future Linux CLOCK_MONOTONIC deadline")
            })?;
            if remaining > MAX_WINDOW_NS {
                return Err(invalid(
                    "HTTP observation deadline window exceeds 3600 seconds",
                ));
            }
            deadline = Some(value);
        } else {
            ordinary.push(args[at].clone());
        }
        at += 1;
    }
    Ok((crate::cli::parse_serve_address(&ordinary)?, deadline))
}

#[cfg(target_os = "linux")]
fn monotonic_ns() -> io::Result<u64> {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // CLOCK_MONOTONIC is the same absolute domain used by Python monotonic_ns.
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut time) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let seconds =
        u64::try_from(time.tv_sec).map_err(|_| io::Error::other("negative monotonic clock"))?;
    let nanos = u64::try_from(time.tv_nsec)
        .map_err(|_| io::Error::other("negative monotonic nanoseconds"))?;
    if nanos >= 1_000_000_000 {
        return Err(io::Error::other("invalid monotonic nanoseconds"));
    }
    seconds
        .checked_mul(1_000_000_000)
        .and_then(|n| n.checked_add(nanos))
        .ok_or_else(|| io::Error::other("monotonic clock overflow"))
}
#[cfg(not(target_os = "linux"))]
fn monotonic_ns() -> io::Result<u64> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "HTTP observation requires Linux CLOCK_MONOTONIC",
    ))
}

// The fixed summary refuses before formatting can exceed its allocation.
struct SummaryLine {
    bytes: [u8; 1024],
    len: usize,
}
impl std::fmt::Write for SummaryLine {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        let end = self
            .len
            .checked_add(value.len())
            .filter(|end| *end <= self.bytes.len())
            .ok_or(std::fmt::Error)?;
        self.bytes[self.len..end].copy_from_slice(value.as_bytes());
        self.len = end;
        Ok(())
    }
}

#[derive(Default)]
pub(crate) struct Observation {
    accepted: AtomicU64,
    refused_capacity: AtomicU64,
    spawn_failed: AtomicU64,
    connection_live: AtomicU64,
    connection_peak: AtomicU64,
    entered: AtomicU64,
    completed_ok: AtomicU64,
    completed_error: AtomicU64,
    operation_live: AtomicU64,
    operation_peak: AtomicU64,
    overflowed: AtomicBool,
}
impl Observation {
    fn increment(&self, counter: &AtomicU64) -> u64 {
        match counter.fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_add(1)) {
            Ok(previous) => previous + 1,
            Err(_) => {
                self.overflowed.store(true, Ordering::Release);
                u64::MAX
            }
        }
    }
    fn decrement(&self, counter: &AtomicU64) {
        if counter
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_sub(1))
            .is_err()
        {
            self.overflowed.store(true, Ordering::Release);
        }
    }
    pub(crate) fn connection(self: &Arc<Self>) -> ConnectionObservation {
        self.increment(&self.accepted);
        let live = self.increment(&self.connection_live);
        self.connection_peak.fetch_max(live, Ordering::AcqRel);
        ConnectionObservation(Arc::clone(self))
    }
    pub(crate) fn refused(&self) {
        self.increment(&self.refused_capacity);
    }
    pub(crate) fn spawn_failed(&self) {
        self.increment(&self.spawn_failed);
    }
    pub(crate) fn emit(&self, orderly: bool, deadline: OwnerDeadline) -> io::Result<bool> {
        let read = |n: &AtomicU64| n.load(Ordering::Acquire);
        let live = read(&self.connection_live);
        let operations_live = read(&self.operation_live);
        let overflow = self.overflowed.load(Ordering::Acquire);
        let complete = orderly
            && live == 0
            && operations_live == 0
            && !overflow
            && deadline.remaining().is_ok();
        let mut line = SummaryLine {
            bytes: [0; 1024],
            len: 0,
        };
        write!(&mut line,
            "{MARKER}{{\"schema\":\"tos_http_observation_v1\",\"complete\":{complete},\"connections\":{{\"accepted\":{},\"refused_capacity\":{},\"spawn_failed\":{},\"live\":{live},\"peak\":{}}},\"operations\":{{\"entered\":{},\"completed_ok\":{},\"completed_error\":{},\"live\":{operations_live},\"peak\":{}}},\"queue\":{{\"supported\":false,\"depth\":0}},\"overflowed\":{overflow}}}\n",
            read(&self.accepted),
            read(&self.refused_capacity),
            read(&self.spawn_failed),
            read(&self.connection_peak),
            read(&self.entered),
            read(&self.completed_ok),
            read(&self.completed_error),
            read(&self.operation_peak)
        ).map_err(|_| io::Error::other("HTTP observation summary exceeds 1024 bytes"))?;
        write_summary(&line.bytes[..line.len])?;
        if complete {
            deadline.remaining()?;
        }
        Ok(complete)
    }
}
pub(crate) struct ConnectionObservation(Arc<Observation>);
impl Drop for ConnectionObservation {
    fn drop(&mut self) {
        self.0.decrement(&self.0.connection_live);
    }
}

thread_local! { static CURRENT: RefCell<Option<Arc<Observation>>> = const { RefCell::new(None) }; }
/// Each observed worker carries its own server collector; other servers and
/// MCP/CLI threads never inherit these counters.
pub(crate) struct WorkerObservation(Option<Arc<Observation>>);
impl WorkerObservation {
    pub(crate) fn enter(observation: Arc<Observation>) -> Self {
        Self(CURRENT.with(|slot| slot.replace(Some(observation))))
    }
}
impl Drop for WorkerObservation {
    fn drop(&mut self) {
        CURRENT.with(|slot| slot.replace(self.0.take()));
    }
}
pub(crate) struct KernelOperation {
    observation: Arc<Observation>,
    ok: bool,
}
impl KernelOperation {
    pub(crate) fn enter() -> Option<Self> {
        CURRENT.with(|slot| {
            slot.borrow().as_ref().map(|observation| {
                observation.increment(&observation.entered);
                let live = observation.increment(&observation.operation_live);
                observation.operation_peak.fetch_max(live, Ordering::AcqRel);
                Self {
                    observation: Arc::clone(observation),
                    ok: false,
                }
            })
        })
    }
    pub(crate) fn success(&mut self) {
        self.ok = true;
    }
}
impl Drop for KernelOperation {
    fn drop(&mut self) {
        self.observation.increment(if self.ok {
            &self.observation.completed_ok
        } else {
            &self.observation.completed_error
        });
        self.observation.decrement(&self.observation.operation_live);
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn require_control_pipe() -> io::Result<()> {
    let mut stamp = std::mem::MaybeUninit::<libc::stat>::uninit();
    if unsafe { libc::fstat(libc::STDIN_FILENO, stamp.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let stamp = unsafe { stamp.assume_init() };
    if stamp.st_mode & libc::S_IFMT != libc::S_IFIFO {
        return Err(io::Error::other(
            "HTTP observation requires owned stdin pipe",
        ));
    }
    Ok(())
}
/// Polling never reads commands: any byte is invalid, and only EOF stops accept.
#[cfg(target_os = "linux")]
pub(crate) fn poll_control(listener: i32, deadline: OwnerDeadline) -> io::Result<(bool, bool)> {
    let remaining = deadline.remaining()?;
    let millis = (remaining / 1_000_000).clamp(0, 10) as i32;
    let mut fds = [
        libc::pollfd {
            fd: libc::STDIN_FILENO,
            events: libc::POLLIN,
            revents: 0,
        },
        libc::pollfd {
            fd: listener,
            events: libc::POLLIN,
            revents: 0,
        },
    ];
    let count = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, millis) };
    if count < 0 {
        let error = io::Error::last_os_error();
        if error.kind() == io::ErrorKind::Interrupted {
            return Ok((false, false));
        }
        return Err(error);
    }
    if fds
        .iter()
        .any(|fd| fd.revents & (libc::POLLERR | libc::POLLNVAL) != 0)
    {
        return Err(io::Error::other(
            "HTTP observation control/listener poll failed",
        ));
    }
    if fds[0].revents & (libc::POLLIN | libc::POLLHUP) != 0 {
        let mut byte = 0u8;
        let count = unsafe { libc::read(libc::STDIN_FILENO, (&mut byte as *mut u8).cast(), 1) };
        if count == 0 {
            return Ok((true, false));
        }
        if count > 0 {
            return Err(io::Error::other(
                "HTTP observation control bytes are invalid",
            ));
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
    deadline.remaining()?;
    Ok((false, fds[1].revents & libc::POLLIN != 0))
}
#[cfg(target_os = "linux")]
fn write_summary(bytes: &[u8]) -> io::Result<()> {
    // One <=PIPE_BUF nonblocking write: full stderr cannot renew the deadline.
    let old = unsafe { libc::fcntl(libc::STDERR_FILENO, libc::F_GETFL) };
    if old < 0 {
        return Err(io::Error::last_os_error());
    }
    if unsafe { libc::fcntl(libc::STDERR_FILENO, libc::F_SETFL, old | libc::O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error());
    }
    let written = unsafe { libc::write(libc::STDERR_FILENO, bytes.as_ptr().cast(), bytes.len()) };
    let result = if written == bytes.len() as isize {
        Ok(())
    } else if written < 0 {
        Err(io::Error::last_os_error())
    } else {
        Err(io::Error::new(
            io::ErrorKind::WriteZero,
            "partial HTTP observation summary",
        ))
    };
    let restored = unsafe { libc::fcntl(libc::STDERR_FILENO, libc::F_SETFL, old) };
    if restored < 0 {
        return Err(io::Error::last_os_error());
    }
    result
}
#[cfg(not(target_os = "linux"))]
fn write_summary(_: &[u8]) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "HTTP observation requires Linux",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observed_kernel_scope_releases_after_unwind_without_leaking() {
        assert!(KernelOperation::enter().is_none());
        let observation = Arc::new(Observation::default());
        let result = std::panic::catch_unwind({
            let observation = Arc::clone(&observation);
            move || {
                let _worker = WorkerObservation::enter(observation);
                let _operation = KernelOperation::enter().expect("observed worker");
                panic!("kernel unwound");
            }
        });
        assert!(result.is_err());
        assert!(KernelOperation::enter().is_none());
        assert_eq!(observation.entered.load(Ordering::Acquire), 1);
        assert_eq!(observation.completed_error.load(Ordering::Acquire), 1);
        assert_eq!(observation.completed_ok.load(Ordering::Acquire), 0);
        assert_eq!(observation.operation_live.load(Ordering::Acquire), 0);
    }

    #[test]
    fn summary_refuses_before_crossing_fixed_capacity() {
        let mut line = SummaryLine {
            bytes: [0; 1024],
            len: 0,
        };
        line.write_str(&"x".repeat(1024)).expect("exact capacity");
        assert!(line.write_str("y").is_err());
        assert_eq!(line.len, 1024);
        assert_eq!(line.bytes[1023], b'x');
    }
}
