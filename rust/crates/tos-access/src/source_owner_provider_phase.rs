//! Linux descriptor transport for native-produced custom owner continuations.
//! The platform must authenticate each received descriptor against the held
//! child PID/image via SCM_CREDENTIALS before callbacks or another phase.
use std::io::{Read, Write};
#[cfg(target_os = "linux")]
use std::{
    fs::File,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
};

fn refuse() -> std::io::Error {
    std::io::Error::other("source owner phase refused")
}
#[cfg(target_os = "linux")]
fn now() -> std::io::Result<u64> {
    let mut t = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut t) } != 0
        || t.tv_sec < 0
        || t.tv_nsec < 0
    {
        return Err(refuse());
    }
    (t.tv_sec as u64)
        .checked_mul(1_000_000_000)
        .and_then(|n| n.checked_add(t.tv_nsec as u64))
        .ok_or_else(refuse)
}
#[cfg(target_os = "linux")]
fn stamp(fd: i32) -> std::io::Result<libc::stat> {
    let mut s = std::mem::MaybeUninit::<libc::stat>::uninit();
    if unsafe { libc::fstat(fd, s.as_mut_ptr()) } != 0 {
        return Err(refuse());
    }
    Ok(unsafe { s.assume_init() })
}
#[cfg(target_os = "linux")]
fn same(a: &libc::stat, b: &libc::stat) -> bool {
    a.st_dev == b.st_dev
        && a.st_ino == b.st_ino
        && a.st_mode == b.st_mode
        && a.st_nlink == b.st_nlink
        && a.st_uid == b.st_uid
        && a.st_gid == b.st_gid
        && a.st_size == b.st_size
        && a.st_mtime == b.st_mtime
        && a.st_mtime_nsec == b.st_mtime_nsec
        && a.st_ctime == b.st_ctime
        && a.st_ctime_nsec == b.st_ctime_nsec
}
#[cfg(target_os = "linux")]
const SEALS: i32 = libc::F_SEAL_WRITE | libc::F_SEAL_GROW | libc::F_SEAL_SHRINK | libc::F_SEAL_SEAL;
#[cfg(target_os = "linux")]
fn read_state(fd: OwnedFd) -> std::io::Result<Vec<u8>> {
    let before = stamp(fd.as_raw_fd())?;
    if before.st_mode & libc::S_IFMT != libc::S_IFREG
        || before.st_nlink != 0
        || before.st_uid != unsafe { libc::geteuid() }
        || before.st_size <= 0
        || before.st_size as u64 > tos_command::source_read_provider::STATE_BYTES as u64
        || unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GET_SEALS) } != SEALS
    {
        return Err(refuse());
    }
    let mut file = File::from(fd);
    if unsafe { libc::lseek(file.as_raw_fd(), 0, libc::SEEK_SET) } != 0 {
        return Err(refuse());
    }
    let mut raw = Vec::new();
    (&mut file)
        .take(before.st_size as u64 + 1)
        .read_to_end(&mut raw)?;
    if raw.len() != before.st_size as usize || !same(&before, &stamp(file.as_raw_fd())?) {
        return Err(refuse());
    }
    Ok(raw)
}
#[cfg(target_os = "linux")]
fn sealed(raw: &[u8]) -> std::io::Result<File> {
    let fd = unsafe {
        libc::memfd_create(
            c"tos-source-owner-state".as_ptr(),
            libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING,
        )
    };
    if fd < 0 {
        return Err(refuse());
    }
    let mut file = unsafe { File::from_raw_fd(fd) };
    file.write_all(raw)?;
    if unsafe { libc::fcntl(fd, libc::F_ADD_SEALS, SEALS) } != 0
        || unsafe { libc::lseek(fd, 0, libc::SEEK_SET) } != 0
    {
        return Err(refuse());
    }
    Ok(file)
}
#[cfg(target_os = "linux")]
fn send_state(channel: i32, file: &File, payload: &[u8]) -> std::io::Result<()> {
    // Alignment is guaranteed by the usize storage, unlike a byte Vec.
    let mut control = [0usize; 8];
    let mut iov = libc::iovec {
        iov_base: payload.as_ptr().cast_mut().cast(),
        iov_len: payload.len(),
    };
    let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;
    msg.msg_control = control.as_mut_ptr().cast();
    msg.msg_controllen = unsafe { libc::CMSG_SPACE(std::mem::size_of::<i32>() as u32) } as usize;
    let header = unsafe { libc::CMSG_FIRSTHDR(&msg) };
    if header.is_null() || msg.msg_controllen > std::mem::size_of_val(&control) {
        return Err(refuse());
    }
    unsafe {
        (*header).cmsg_level = libc::SOL_SOCKET;
        (*header).cmsg_type = libc::SCM_RIGHTS;
        (*header).cmsg_len = libc::CMSG_LEN(std::mem::size_of::<i32>() as u32) as usize;
        std::ptr::write_unaligned(libc::CMSG_DATA(header).cast::<i32>(), file.as_raw_fd());
    }
    if unsafe { libc::sendmsg(channel, &msg, libc::MSG_DONTWAIT | libc::MSG_NOSIGNAL) }
        != payload.len() as isize
    {
        return Err(refuse());
    }
    Ok(())
}
#[cfg(target_os = "linux")]
fn perform(args: &[String], stdin: &mut dyn Read, stdout: &mut dyn Write) -> std::io::Result<()> {
    let started = now()?;
    // No paths, state JSON argument, timeout renewal, or descriptor duplication.
    if !(args.len() == 4 || args.len() == 6)
        || args[2] != "--channel-fd"
        || (args.len() == 6 && !["--state-fd", "--binding-fd"].contains(&args[4].as_str()))
    {
        return Err(refuse());
    }
    let parse_fd = |s: &str| -> std::io::Result<i32> {
        if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
            return Err(refuse());
        }
        s.parse::<i32>().ok().filter(|n| *n > 2).ok_or_else(refuse)
    };
    let channel_no = parse_fd(&args[3])?;
    let state_no = if args.len() == 6 {
        Some(parse_fd(&args[5])?)
    } else {
        None
    };
    if state_no == Some(channel_no) {
        return Err(refuse());
    }
    if unsafe { libc::fcntl(channel_no, libc::F_GETFD) } < 0
        || state_no.is_some_and(|fd| unsafe { libc::fcntl(fd, libc::F_GETFD) } < 0)
    {
        return Err(refuse());
    }
    // Borrowed inherited integers become owned exactly once and close on refusal.
    let channel = unsafe { OwnedFd::from_raw_fd(channel_no) };
    let state = state_no.map(|n| unsafe { OwnedFd::from_raw_fd(n) });
    let mut kind: i32 = 0;
    let mut length = std::mem::size_of::<i32>() as libc::socklen_t;
    if unsafe {
        libc::getsockopt(
            channel.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_TYPE,
            (&mut kind as *mut i32).cast(),
            &mut length,
        )
    } != 0
        || kind != libc::SOCK_SEQPACKET
    {
        return Err(refuse());
    }
    let mut address: libc::sockaddr_storage = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of::<libc::sockaddr_storage>() as libc::socklen_t;
    if unsafe {
        libc::getsockname(
            channel.as_raw_fd(),
            (&mut address as *mut libc::sockaddr_storage).cast(),
            &mut len,
        )
    } != 0
        || (len as usize) < std::mem::size_of::<libc::sa_family_t>()
        || (len as usize) > std::mem::size_of::<libc::sockaddr_storage>()
        || address.ss_family as i32 != libc::AF_UNIX
    {
        return Err(refuse());
    }
    let is_continuation = args.get(4).map(String::as_str) == Some("--state-fd");
    let cap = if is_continuation {
        tos_command::source_read_provider::PHASE_INPUT_BYTES
    } else {
        65536 + 4096
    };
    let mut raw = Vec::new();
    stdin.take(cap as u64 + 1).read_to_end(&mut raw)?;
    if raw.len() > cap {
        return Err(refuse());
    }
    let result = if is_continuation {
        let state = state.ok_or_else(refuse)?;
        let prior = read_state(state)?;
        tos_command::source_read_provider::advance(&prior, &raw, started, now()?)
    } else {
        let initialized = state.map(read_state).transpose()?;
        tos_command::source_read_provider::begin_with_binding(
            &raw,
            initialized.as_deref(),
            started,
            5_000_000_000,
        )
    }
    .map_err(|_| refuse())?;
    result.check_clock(now()?).map_err(|_| refuse())?;
    if let Some(raw) = &result.continuation {
        let file = sealed(raw)?;
        result.check_clock(now()?).map_err(|_| refuse())?;
        send_state(channel.as_raw_fd(), &file, result.continuation_marker)?;
    }
    result.check_clock(now()?).map_err(|_| refuse())?;
    stdout.write_all(&result.output)?;
    stdout.write_all(b"\n")?;
    stdout.flush()?;
    result.check_clock(now()?).map_err(|_| refuse())?;
    Ok(())
}
pub fn run(
    args: &[String],
    stdin: &mut dyn Read,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    #[cfg(target_os = "linux")]
    let result = perform(args, stdin, stdout);
    #[cfg(not(target_os = "linux"))]
    let result: std::io::Result<()> = Err(refuse());
    match result {
        Ok(()) => 0,
        Err(_) => {
            let _ = writeln!(stderr, "source owner phase refused");
            2
        }
    }
}
