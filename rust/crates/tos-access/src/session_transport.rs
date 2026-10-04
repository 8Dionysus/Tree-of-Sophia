//! Private same-owner Core session framing. No model, ticket, or FD grant crosses this wire.
//! The calling owner authenticates `control` against the issuer-held socket identity first,
//! and calls this loop INSIDE its selected model/evidence/context callback.
use std::{
    cell::RefCell,
    io,
    os::fd::{AsRawFd, BorrowedFd},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

const MAGIC: &[u8; 8] = b"TOSSES1\0";
const HEADER: usize = 40;
const PACKET: usize = 65536;
const PAYLOAD: usize = PACKET - HEADER;
pub(super) const REQUEST: u8 = 1;
pub(super) const STARTUP: u8 = 2;
pub(super) const REPLY: u8 = 3;
pub(super) const CLOSE: u8 = 4;
pub(super) const CLOSE_ACK: u8 = 5;
pub(super) const REFUSAL: u8 = 6;
type Result<T> = std::result::Result<T, &'static str>;

#[derive(Clone, Copy, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Limits {
    pub max_call_bytes: usize,
    pub max_reply_bytes: usize,
    pub max_chunks_per_frame: u64,
    pub max_calls: u64,
    pub max_total_request_bytes: u64,
    pub max_total_reply_bytes: u64,
}
impl Limits {
    pub fn validate(self) -> Result<Self> {
        if self.max_call_bytes == 0
            || self.max_reply_bytes == 0
            || self.max_chunks_per_frame == 0
            || self.max_calls == 0
            || self.max_total_request_bytes == 0
            || self.max_total_reply_bytes == 0
            || self.max_call_bytes as u64 > self.max_total_request_bytes
            || self.max_reply_bytes as u64 > self.max_total_reply_bytes
        {
            return Err("Core session finite transport limits");
        }
        Ok(self)
    }
}

/// Implemented by the ORIGINAL native owner ledger, never by a JSON admission flag.
/// Workspace includes exact allocated request capacity and both packet stack arrays.
/// Query parsing, packet body, metadata, model, checkpoint and output allocations remain
/// charged by that same owner; this framing reservation does not replace their charges.
pub(super) trait Workspace {
    fn reserve(&mut self, bytes: usize) -> Result<()>;
    fn release(&mut self, bytes: usize);
    fn charge_work(&mut self, bytes: u64) -> Result<()>;
}
fn active(deadline: Instant, cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::Acquire) {
        return Err("Core session cancelled");
    }
    if Instant::now() >= deadline {
        return Err("Core session original cutoff");
    }
    Ok(())
}
fn poll(fd: i32, events: i16, deadline: Instant, cancelled: &AtomicBool) -> Result<()> {
    loop {
        active(deadline, cancelled)?;
        let remaining = deadline.saturating_duration_since(Instant::now());
        let ms = remaining.min(Duration::from_millis(50)).as_millis().max(1) as i32;
        let mut p = libc::pollfd {
            fd,
            events,
            revents: 0,
        };
        let n = unsafe { libc::poll(&mut p, 1, ms) };
        active(deadline, cancelled)?;
        if n < 0 {
            if io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err("Core session socket poll");
        }
        if n == 0 {
            continue;
        }
        if p.revents & events != 0 {
            return Ok(());
        }
        return Err("Core session socket terminal before acknowledged close");
    }
}

#[derive(Clone, Copy)]
struct Header {
    kind: u8,
    sequence: u64,
    offset: u64,
    total: u64,
    length: u32,
}
impl Header {
    fn decode(raw: &[u8]) -> Result<Self> {
        if raw.len() < HEADER || &raw[..8] != MAGIC || raw[9..12] != [0, 0, 0] {
            return Err("Core session header magic/flags/reserved");
        }
        let field = |start| u64::from_le_bytes(raw[start..start + 8].try_into().unwrap());
        let h = Self {
            kind: raw[8],
            sequence: field(12),
            offset: field(20),
            total: field(28),
            length: u32::from_le_bytes(raw[36..40].try_into().unwrap()),
        };
        if !matches!(
            h.kind,
            REQUEST | STARTUP | REPLY | CLOSE | CLOSE_ACK | REFUSAL
        ) || h.length as usize != raw.len() - HEADER
            || h.length as usize > PAYLOAD
            || h.offset
                .checked_add(h.length as u64)
                .is_none_or(|end| end > h.total)
            || h.length == 0 && (h.total != 0 || h.offset != 0)
        {
            return Err("Core session header length/kind");
        }
        Ok(h)
    }
    fn encode(self, raw: &mut [u8]) {
        raw[..HEADER].fill(0);
        raw[..8].copy_from_slice(MAGIC);
        raw[8] = self.kind;
        raw[12..20].copy_from_slice(&self.sequence.to_le_bytes());
        raw[20..28].copy_from_slice(&self.offset.to_le_bytes());
        raw[28..36].copy_from_slice(&self.total.to_le_bytes());
        raw[36..40].copy_from_slice(&self.length.to_le_bytes());
    }
}
fn recv(
    fd: i32,
    raw: &mut [u8; PACKET],
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<usize> {
    loop {
        poll(fd, libc::POLLIN, deadline, cancelled)?;
        let mut iov = libc::iovec {
            iov_base: raw.as_mut_ptr().cast(),
            iov_len: raw.len(),
        };
        // Aligned ancillary storage. Close any actually received SCM_RIGHTS before refusal.
        let mut ancillary = [0_usize; 32];
        let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
        msg.msg_iov = &mut iov;
        msg.msg_iovlen = 1;
        msg.msg_control = ancillary.as_mut_ptr().cast();
        msg.msg_controllen = std::mem::size_of_val(&ancillary);
        let n = unsafe { libc::recvmsg(fd, &mut msg, libc::MSG_DONTWAIT | libc::MSG_CMSG_CLOEXEC) };
        if n < 0 {
            if matches!(
                io::Error::last_os_error().kind(),
                io::ErrorKind::Interrupted | io::ErrorKind::WouldBlock
            ) {
                continue;
            }
            return Err("Core session receive");
        }
        let had_control = msg.msg_controllen != 0;
        unsafe {
            let mut c = libc::CMSG_FIRSTHDR(&msg);
            while !c.is_null() {
                if (*c).cmsg_level == libc::SOL_SOCKET && (*c).cmsg_type == libc::SCM_RIGHTS {
                    let head = libc::CMSG_LEN(0) as usize;
                    if (*c).cmsg_len >= head {
                        let count = ((*c).cmsg_len - head) / std::mem::size_of::<i32>();
                        let fds = libc::CMSG_DATA(c).cast::<i32>();
                        for i in 0..count {
                            libc::close(*fds.add(i));
                        }
                    }
                }
                c = libc::CMSG_NXTHDR(&msg, c);
            }
        }
        if had_control || msg.msg_flags & (libc::MSG_TRUNC | libc::MSG_CTRUNC) != 0 {
            return Err("Core session ancillary or truncated record refused");
        }
        active(deadline, cancelled)?;
        if n == 0 {
            return Err("Core session EOF before explicit close");
        }
        return Ok(n as usize);
    }
}

pub(super) struct Reply<'a, 'w> {
    fd: i32,
    limits: Limits,
    deadline: Instant,
    cancelled: &'a AtomicBool,
    total_sent: &'a mut u64,
    workspace: &'a RefCell<&'w mut dyn Workspace>,
    expected_kind: u8,
    expected_sequence: u64,
    sent: bool,
    completed: bool,
}
impl Reply<'_, '_> {
    /// A call may shorten its admitted reply clock, never renew the owner's cutoff.
    pub fn narrow_deadline(&mut self, deadline: Instant) -> Result<()> {
        if self.sent || deadline > self.deadline {
            return Err("Core session call deadline extension");
        }
        self.deadline = deadline;
        active(self.deadline, self.cancelled)
    }

    /// Borrow packet/escaped segments from the held callback; no complete second reply Vec.
    /// Callback must recheck actual packet+source fences immediately before AND after this call.
    pub fn send(&mut self, kind: u8, sequence: u64, segments: &[&[u8]]) -> Result<()> {
        if self.sent
            || sequence != self.expected_sequence
            || (kind != self.expected_kind && !(self.expected_kind == REPLY && kind == REFUSAL))
        {
            return Err("Core session reply kind");
        }
        let total = segments
            .iter()
            .try_fold(0_usize, |n, s| n.checked_add(s.len()))
            .ok_or("Core session reply overflow")?;
        if total > self.limits.max_reply_bytes {
            return Err("Core session reply cap");
        }
        let next = self
            .total_sent
            .checked_add(total as u64)
            .filter(|n| *n <= self.limits.max_total_reply_bytes)
            .ok_or("Core session cumulative reply cap")?;
        let chunks = total.max(1).div_ceil(PAYLOAD) as u64;
        if chunks > self.limits.max_chunks_per_frame {
            return Err("Core session reply chunks cap");
        }
        // Poison the reply before any possible disclosure. A caught send failure cannot restart it.
        self.sent = true;
        let mut raw = [0_u8; PACKET];
        let mut offset = 0_usize;
        let mut index = 0_usize;
        let mut consumed = 0_usize;
        loop {
            active(self.deadline, self.cancelled)?;
            let length = (total - offset).min(PAYLOAD);
            self.workspace
                .borrow_mut()
                .charge_work((2 * length + HEADER) as u64)?;
            let h = Header {
                kind,
                sequence,
                offset: offset as u64,
                total: total as u64,
                length: length as u32,
            };
            h.encode(&mut raw);
            let mut filled = 0;
            while filled < length {
                if consumed == segments[index].len() {
                    index += 1;
                    consumed = 0;
                    continue;
                }
                let n = (segments[index].len() - consumed).min(length - filled);
                raw[HEADER + filled..HEADER + filled + n]
                    .copy_from_slice(&segments[index][consumed..consumed + n]);
                consumed += n;
                filled += n;
            }
            loop {
                poll(self.fd, libc::POLLOUT, self.deadline, self.cancelled)?;
                let n = unsafe {
                    libc::send(
                        self.fd,
                        raw.as_ptr().cast(),
                        HEADER + length,
                        libc::MSG_DONTWAIT | libc::MSG_NOSIGNAL,
                    )
                };
                if n < 0
                    && matches!(
                        io::Error::last_os_error().kind(),
                        io::ErrorKind::Interrupted | io::ErrorKind::WouldBlock
                    )
                {
                    continue;
                }
                if n != (HEADER + length) as isize {
                    return Err("Core session partial/failed record send");
                }
                break;
            }
            // Account every actually successful payload record, including a partial logical frame.
            *self.total_sent += length as u64;
            active(self.deadline, self.cancelled)?;
            offset += length;
            if offset == total {
                break;
            }
        }
        debug_assert_eq!(*self.total_sent, next);
        self.completed = true;
        Ok(())
    }
}

/// Startup closure sends actual capture association under seq0. Request closure parses the
/// exact session DTO with the existing bounded foundation parser, selects ONLY admitted
/// operations, and executes the same borrowed executor. No domain parser lives here.
/// All fences and query workspace checks remain INSIDE those owner closures.
pub(super) fn run(
    control: BorrowedFd<'_>,
    limits: Limits,
    deadline: Instant,
    cancelled: &AtomicBool,
    workspace: &mut dyn Workspace,
    mut startup: impl FnMut(&mut Reply<'_, '_>) -> Result<()>,
    mut call: impl FnMut(u64, &[u8], &mut Reply<'_, '_>) -> Result<()>,
    mut final_fence: impl FnMut() -> Result<()>,
) -> Result<()> {
    active(deadline, cancelled)?;
    let limits = limits.validate()?;
    let fixed_workspace = std::mem::size_of::<RefCell<&mut dyn Workspace>>()
        + 2 * PACKET
        + std::mem::size_of::<Vec<u8>>()
        + std::mem::size_of::<Reply>()
        + std::mem::size_of::<[usize; 32]>()
        + std::mem::size_of::<libc::msghdr>()
        + std::mem::size_of::<libc::iovec>()
        + std::mem::size_of::<libc::pollfd>()
        + std::mem::size_of::<Header>();
    // These callback values coexist with the wire buffers; charge their actual capture storage.
    let fixed_workspace = fixed_workspace
        .checked_add(std::mem::size_of_val(&startup))
        .and_then(|n| n.checked_add(std::mem::size_of_val(&call)))
        .and_then(|n| n.checked_add(std::mem::size_of_val(&final_fence)))
        .ok_or("Core session callback workspace overflow")?;
    let reserve = limits
        .max_call_bytes
        .checked_add(fixed_workspace)
        .ok_or("Core session workspace overflow")?;
    workspace.reserve(reserve)?;
    let workspace = RefCell::new(workspace);
    let result = (|| {
        active(deadline, cancelled)?;
        let mut request = Vec::new();
        request
            .try_reserve_exact(limits.max_call_bytes)
            .map_err(|_| "Core session input allocation")?;
        // Charge the actual allocator capacity; do not silently exceed the reservation.
        if request.capacity() > limits.max_call_bytes {
            return Err("Core session allocator exceeded reserved capacity");
        }
        let mut raw = [0_u8; PACKET];
        let mut total_sent = 0_u64;
        let mut total_received = 0_u64;
        let mut reply = Reply {
            fd: control.as_raw_fd(),
            limits,
            deadline,
            cancelled,
            total_sent: &mut total_sent,
            workspace: &workspace,
            expected_kind: STARTUP,
            expected_sequence: 0,
            sent: false,
            completed: false,
        };
        final_fence()?;
        startup(&mut reply)?;
        if !reply.completed {
            return Err("Core session startup absent");
        }
        final_fence()?;
        let mut sequence = 1_u64;
        loop {
            request.clear();
            let mut total = None;
            let mut chunks = 0_u64;
            loop {
                let n = recv(control.as_raw_fd(), &mut raw, deadline, cancelled)?;
                workspace.borrow_mut().charge_work(n as u64)?;
                let h = Header::decode(&raw[..n])?;
                if h.sequence != sequence || !matches!(h.kind, REQUEST | CLOSE) {
                    return Err("Core session request sequence/kind");
                }
                if h.kind == CLOSE {
                    if total.is_some() || h.total != 0 {
                        return Err("Core session close during frame");
                    }
                    final_fence()?;
                    // Close belongs to the original session cutoff, not the previous call's narrower cutoff.
                    reply.deadline = deadline;
                    reply.expected_kind = CLOSE_ACK;
                    reply.expected_sequence = sequence;
                    reply.sent = false;
                    reply.completed = false;
                    reply.send(CLOSE_ACK, sequence, &[])?;
                    final_fence()?;
                    // Peer/controller owns terminal cleanup. Ack alone never proves process terminal.
                    return Ok(());
                }
                if sequence > limits.max_calls
                    || h.total > limits.max_call_bytes as u64
                    || h.total == 0
                {
                    return Err("Core session call cap");
                }
                if total.is_none() {
                    if h.offset != 0 {
                        return Err("Core session initial offset");
                    }
                    total = Some(h.total);
                    total_received = total_received
                        .checked_add(h.total)
                        .filter(|n| *n <= limits.max_total_request_bytes)
                        .ok_or("Core session cumulative request cap")?;
                }
                if total != Some(h.total) || h.offset != request.len() as u64 {
                    return Err("Core session discontinuous frame");
                }
                chunks = chunks
                    .checked_add(1)
                    .filter(|n| *n <= limits.max_chunks_per_frame)
                    .ok_or("Core session request chunks cap")?;
                request.extend_from_slice(&raw[HEADER..n]);
                if request.len() as u64 == h.total {
                    break;
                }
            }
            final_fence()?;
            reply.deadline = deadline;
            reply.expected_kind = REPLY;
            reply.expected_sequence = sequence;
            reply.sent = false;
            reply.completed = false;
            call(sequence, &request, &mut reply)?;
            if !reply.completed {
                return Err("Core session reply absent");
            }
            final_fence()?;
            sequence = sequence
                .checked_add(1)
                .ok_or("Core session sequence overflow")?;
        }
    })();
    workspace.borrow_mut().release(reserve);
    result
}
