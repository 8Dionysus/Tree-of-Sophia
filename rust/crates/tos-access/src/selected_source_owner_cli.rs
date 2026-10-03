//! Independent retained-vector exact-source transport. It selects the existing
//! native owner, not a Knowledge publication or a Python-prevalidated service.
use crate::{AccessError, AccessErrorCode, AccessProfile};
use std::io::{Read, Write};

fn invalid() -> AccessError {
    AccessError::new(
        AccessErrorCode::InvalidRequest,
        "selected source owner transport refused",
    )
}

#[cfg(target_os = "linux")]
fn vector(fd: i32) -> Result<Vec<u8>, AccessError> {
    use std::{
        fs::File,
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::fs::{FileExt, MetadataExt},
        },
    };
    if fd <= 2 || unsafe { libc::fcntl(fd, libc::F_GETFD) } < 0 {
        return Err(invalid());
    }
    // This CLI owns exactly this inherited descriptor. No duplicate, pathname
    // reopen or borrowed caller descriptor survives the child terminal.
    let file = unsafe { File::from_raw_fd(fd) };
    let before = file.metadata().map_err(|_| invalid())?;
    let seals = libc::F_SEAL_WRITE | libc::F_SEAL_GROW | libc::F_SEAL_SHRINK | libc::F_SEAL_SEAL;
    if !before.is_file()
        || before.nlink() != 0
        || before.uid() != unsafe { libc::geteuid() }
        || before.len() == 0
        || before.len() > 1_048_576
        || unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GET_SEALS) } != seals
    {
        return Err(invalid());
    }
    // Concurrent children inherit the same open-file description. Positional
    // reads preserve the two independent slots without changing its offset.
    let mut raw = vec![0; before.len() as usize + 1];
    let mut count = 0;
    while count < raw.len() {
        let read = file
            .read_at(&mut raw[count..], count as u64)
            .map_err(|_| invalid())?;
        if read == 0 {
            break;
        }
        count += read;
    }
    raw.truncate(count);
    let after = file.metadata().map_err(|_| invalid())?;
    let stamp = |m: &std::fs::Metadata| {
        (
            m.dev(),
            m.ino(),
            m.mode(),
            m.nlink(),
            m.uid(),
            m.gid(),
            m.len(),
            m.mtime(),
            m.mtime_nsec(),
            m.ctime(),
            m.ctime_nsec(),
        )
    };
    if raw.len() as u64 != before.len() || stamp(&before) != stamp(&after) {
        return Err(invalid());
    }
    Ok(raw)
}

pub(crate) fn run(
    args: &[String],
    profile: AccessProfile,
    stdin: &mut dyn Read,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (args, profile, stdin, stdout);
        let _ = writeln!(
            stderr,
            "unavailable: selected source descriptor transport requires Linux"
        );
        2
    }
    #[cfg(target_os = "linux")]
    {
        use crate::source_read::{Operation, Request};
        use std::{path::PathBuf, time::Duration};
        let result = (|| {
            // Admission/vector reads share one startup probe. The outer native
            // caller retains its original whole-call 50s envelope separately.
            let startup = profile
                .with_query_timeout(Duration::from_secs(30))
                .deadline_probe();
            let mut root = None;
            let mut revision = None;
            let mut fd = None;
            let mut local = None;
            let mut i = 2;
            while i + 1 < args.len() {
                let slot = match args[i].as_str() {
                    "--root" => &mut root,
                    "--revision" => &mut revision,
                    "--inputs-fd" => &mut fd,
                    "--local-text-selection" => &mut local,
                    _ => return Err(invalid()),
                };
                if slot.replace(args[i + 1].clone()).is_some() {
                    return Err(invalid());
                }
                i += 2;
            }
            if i + 1 != args.len() {
                return Err(invalid());
            }
            let operation = match args[i].as_str() {
                "capabilities" => Operation::Capabilities,
                "contracts" => Operation::Contract,
                "discover" => Operation::Discover,
                "read" => Operation::Read,
                _ => return Err(invalid()),
            };
            let absolute = |s: String| {
                let p = PathBuf::from(s);
                if !p.is_absolute()
                    || p.components()
                        .any(|c| matches!(c, std::path::Component::ParentDir))
                {
                    Err(invalid())
                } else {
                    Ok(p)
                }
            };
            let root = absolute(root.ok_or_else(invalid)?)?;
            let local = local.map(absolute).transpose()?;
            let revision = revision.ok_or_else(invalid)?;
            let fd: i32 = fd.ok_or_else(invalid)?.parse().map_err(|_| invalid())?;
            let raw = vector(fd)?;
            crate::knowledge::check_abort(&startup)?;
            let selected = crate::selected_source::Selection::open_raw_with_probe(
                &root,
                &raw,
                local.as_deref(),
                &revision,
                profile,
                startup,
            )?;
            let probe = profile.deadline_probe();
            let deadline = selected.deadline()?;
            let mut request = Vec::new();
            stdin
                .take(crate::source_read::MAX_REQUEST_BYTES as u64 + 1)
                .read_to_end(&mut request)
                .map_err(|_| invalid())?;
            let request = Request::from_bytes(operation, &request, profile)?;
            let packet = selected.prepare_owner(request, deadline, probe)?;
            Ok::<_, AccessError>(packet)
        })();
        match result {
            Ok(packet) => crate::cli::write_packet(packet, profile, stdout, stderr),
            Err(error) => {
                let _ = writeln!(stderr, "{}: {}", error.code_str(), error.message);
                2
            }
        }
    }
}
