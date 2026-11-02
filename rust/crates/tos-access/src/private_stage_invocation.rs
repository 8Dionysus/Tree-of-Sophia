//! Host-resident custody relay for one protected foundation invocation.
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_command::source_protected_foundation_invocation::ProtectedFoundationInvocation;

pub(crate) struct Relay {
    owner: ProtectedFoundationInvocation,
    socket: UnixStream,
    pending: Vec<u8>,
    sent: usize,
    terminal_pending: bool,
    complete: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    use std::time::Duration;

    #[test]
    fn exact_host_custody_roundtrip() {
        let root =
            std::env::temp_dir().join(format!("tos-invocation-relay-{}", std::process::id()));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .unwrap();
        let path = root.join("invocation.json");
        std::fs::write(&path, b"{}\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let cancel = AtomicBool::new(false);
        let deadline = Instant::now() + Duration::from_secs(5);
        let (mut relay, mut child, _) = Relay::select(&path, deadline, &cancel).unwrap();
        let worker = std::thread::spawn(move || {
            child.write_all(&[1]).unwrap();
            let mut header = [0; 4];
            child.read_exact(&mut header).unwrap();
            assert_eq!(u32::from_be_bytes(header), 3);
            let mut bytes = [0; 3];
            child.read_exact(&mut bytes).unwrap();
            assert_eq!(&bytes, b"{}\n");
            child.write_all(&[2]).unwrap();
            child.read_exact(&mut header).unwrap();
            assert_eq!(u32::from_be_bytes(header), 0);
        });
        while !worker.is_finished() {
            assert!(Instant::now() < deadline);
            relay.service(deadline, &cancel).unwrap();
            std::thread::sleep(Duration::from_millis(1));
        }
        worker.join().unwrap();
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_dir(&root).unwrap();
    }
}

impl Relay {
    pub(crate) fn select(
        path: &Path,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(Self, UnixStream, serde_json::Value), String> {
        let owner = ProtectedFoundationInvocation::select(path, deadline, cancelled)
            .map_err(|_| "protected invocation host selection refused")?;
        let (socket, child) = UnixStream::pair().map_err(|e| e.to_string())?;
        socket.set_nonblocking(true).map_err(|e| e.to_string())?;
        let mut meta = unsafe { std::mem::zeroed::<libc::stat>() };
        if unsafe { libc::fstat(child.as_raw_fd(), &mut meta) } != 0 {
            return Err("protected invocation socket metadata unavailable".into());
        }
        let mut cookie = 0u64;
        let mut len = std::mem::size_of::<u64>() as libc::socklen_t;
        if unsafe {
            libc::getsockopt(
                child.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_COOKIE,
                (&mut cookie as *mut u64).cast(),
                &mut len,
            )
        } != 0
            || cookie == 0
            || len as usize != std::mem::size_of::<u64>()
        {
            return Err("protected invocation socket identity unavailable".into());
        }
        let info = serde_json::json!({"fd":child.as_raw_fd(),"socket_dev":meta.st_dev,"socket_ino":meta.st_ino,"socket_cookie":cookie,"path":owner.path(),"sha256":owner.digest().to_hex(),"protocol":"tos_foundation_invocation_custody_v1","operations":["initial","eof"]});
        Ok((
            Self {
                owner,
                socket,
                pending: Vec::new(),
                sent: 0,
                terminal_pending: false,
                complete: false,
            },
            child,
            info,
        ))
    }

    pub(crate) fn completed(&self) -> bool {
        self.complete
    }

    pub(crate) fn service(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), String> {
        if self.pending.is_empty() {
            let mut op = [0u8; 1];
            match self.socket.read(&mut op) {
                Ok(0) => return Ok(()),
                Ok(1) => {
                    let payload = match op[0] {
                        1 => self
                            .owner
                            .initial_read(deadline, cancelled)
                            .map_err(|_| "protected invocation initial check refused")?,
                        2 => {
                            self.owner
                                .verify_before_source_eof(deadline, cancelled)
                                .map_err(|_| "protected invocation terminal check refused")?;
                            self.terminal_pending = true;
                            Vec::new()
                        }
                        _ => return Err("protected invocation operation refused".into()),
                    };
                    self.pending = (payload.len() as u32).to_be_bytes().to_vec();
                    self.pending.extend_from_slice(&payload);
                    self.sent = 0;
                }
                Ok(_) => return Err("protected invocation operation malformed".into()),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => return Ok(()),
                Err(_) => return Err("protected invocation channel read failed".into()),
            }
        }
        while self.sent < self.pending.len() {
            match self.socket.write(&self.pending[self.sent..]) {
                Ok(0) => return Err("protected invocation channel closed".into()),
                Ok(n) => self.sent += n,
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                    ) =>
                {
                    return Ok(());
                }
                Err(_) => return Err("protected invocation channel write failed".into()),
            }
        }
        self.pending.clear();
        self.sent = 0;
        if self.terminal_pending {
            self.complete = true;
            self.terminal_pending = false;
        }
        Ok(())
    }
}
