//! Consumer side of the stage-issued one-invocation custody channel.
use crate::source_command::{SourceCommandError, SourceCommandResult};
use crate::source_creation_store::active;
use std::io::{Read, Write};
use std::os::fd::FromRawFd;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tos_foundation::Digest256;

pub(crate) struct Client {
    socket: UnixStream,
    digest: String,
}
impl Client {
    pub(crate) fn select(path: &Path) -> SourceCommandResult<Option<Self>> {
        if std::env::var_os("TOS_FOUNDATION_INVOCATION_CUSTODY").is_none() {
            return Ok(None);
        }
        if std::env::var_os("TOS_FOUNDATION_INVOCATION_CUSTODY").as_deref()
            != Some(std::ffi::OsStr::new("1"))
        {
            return Err(SourceCommandError::Denied(
                "foundation invocation custody marker",
            ));
        }
        let stage = tos_compiler::private_tmpfs_stage::PrivateTmpfsStageIsolation::select_issued_from_environment()
            .map_err(|_| SourceCommandError::Denied("foundation invocation stage custody"))?;
        let (fd, digest) = stage
            .invocation_custody(path)
            .map_err(|_| SourceCommandError::Denied("foundation invocation ticket binding"))?
            .ok_or(SourceCommandError::Denied(
                "foundation invocation ticket absent",
            ))?;
        let duplicate = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 3) };
        if duplicate < 0 {
            return Err(SourceCommandError::Denied(
                "foundation invocation channel absent",
            ));
        }
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
        if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC) } < 0 {
            unsafe { libc::close(duplicate) };
            return Err(SourceCommandError::Denied(
                "foundation invocation channel protection",
            ));
        }
        let socket = unsafe { UnixStream::from_raw_fd(duplicate) };
        socket
            .set_nonblocking(true)
            .map_err(|_| SourceCommandError::Denied("foundation invocation channel mode"))?;
        Ok(Some(Self { socket, digest }))
    }
    pub(crate) fn retained_state_bytes(&self) -> usize {
        self.digest.capacity()
    }
    fn transfer(
        &mut self,
        op: u8,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<u8>> {
        loop {
            active(deadline, cancelled)?;
            match self.socket.write(&[op]) {
                Ok(1) => break,
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                    ) =>
                {
                    std::thread::sleep(Duration::from_millis(1))
                }
                _ => {
                    return Err(SourceCommandError::Denied(
                        "foundation invocation channel write",
                    ));
                }
            }
        }
        let mut header = [0u8; 4];
        self.read_exact(&mut header, deadline, cancelled)?;
        let length = u32::from_be_bytes(header) as usize;
        if length > 1_048_576 || (op == 1 && length == 0) || (op == 2 && length != 0) {
            return Err(SourceCommandError::Denied(
                "foundation invocation channel length",
            ));
        }
        let mut raw = vec![0; length];
        self.read_exact(&mut raw, deadline, cancelled)?;
        if op == 1 && Digest256::of_bytes(&raw).to_hex() != self.digest {
            return Err(SourceCommandError::Conflict(
                "foundation invocation custody digest",
            ));
        }
        Ok(raw)
    }
    fn read_exact(
        &mut self,
        target: &mut [u8],
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        let mut done = 0;
        while done < target.len() {
            active(deadline, cancelled)?;
            match self.socket.read(&mut target[done..]) {
                Ok(0) => {
                    return Err(SourceCommandError::Denied(
                        "foundation invocation custodian closed",
                    ));
                }
                Ok(n) => done += n,
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                    ) =>
                {
                    std::thread::sleep(Duration::from_millis(1))
                }
                Err(_) => {
                    return Err(SourceCommandError::Denied(
                        "foundation invocation channel read",
                    ));
                }
            }
        }
        Ok(())
    }
    pub(crate) fn initial(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<u8>> {
        self.transfer(1, deadline, cancelled)
    }
    pub(crate) fn terminal(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        self.transfer(2, deadline, cancelled).map(drop)
    }
}
