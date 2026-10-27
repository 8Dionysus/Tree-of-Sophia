//! Current-lane Cargo artifact selection for the real conformance consumers.
//! No filesystem artifact search, persisted registry, or inherited hash authority.
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::{
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
    sync::atomic::{AtomicI32, Ordering},
    time::{Duration, Instant},
};
use tos_foundation::Digest256Hasher;

fn invalid(message: &str) -> io::Error {
    io::Error::other(message)
}
fn check(deadline: Instant, cancel: &AtomicI32) -> io::Result<()> {
    if cancel.load(Ordering::Relaxed) != 0 || Instant::now() >= deadline {
        return Err(invalid(
            "conformance product selection cancelled or expired",
        ));
    }
    Ok(())
}

/// Start a separately bounded phase without extending the enclosing lane.
pub(crate) fn selection_deadline(
    now: Instant,
    lane_deadline: Instant,
    phase_wall: Duration,
) -> Instant {
    lane_deadline.min(now + phase_wall)
}

fn cargo_test(argv: &[String]) -> bool {
    argv.first()
        .is_some_and(|p| Path::new(p).file_name().is_some_and(|p| p == "cargo"))
        && argv.get(1).is_some_and(|p| p == "test")
        && argv.iter().any(|p| p == "--workspace")
}

pub(crate) fn preparation(argv: &[String]) -> bool {
    cargo_test(argv)
        && argv.iter().any(|p| p == "--no-run")
        && argv.iter().any(|p| p == "--message-format=json")
}
pub(crate) fn execution(argv: &[String]) -> bool {
    cargo_test(argv) && !argv.iter().any(|p| p == "--no-run")
}
/// Exact validation-lane command that builds the native consumer used by
/// conformance fixtures. It is also the sole source of PreparedAccess state.
pub(crate) fn access_preparation(argv: &[String]) -> bool {
    argv.first().is_some_and(|program| {
        Path::new(program)
            .file_name()
            .is_some_and(|name| name == "cargo")
    }) && argv.iter().skip(1).map(String::as_str).eq([
        "build",
        "--locked",
        "-p",
        "tos-access",
        "--bin",
        "tos-access",
    ])
}
/// Workspace tests that explicitly remove the conformance package consume the
/// current lane's Access image without requiring the conformance test image.
/// Cargo options must occur before the libtest separator.
pub(crate) fn prepared_access_execution(argv: &[String]) -> bool {
    if !cargo_test(argv) {
        return false;
    }
    let end = argv
        .iter()
        .position(|part| part == "--")
        .unwrap_or(argv.len());
    let cargo_args = &argv[2..end];
    cargo_args.iter().any(|part| part == "--workspace")
        && cargo_args
            .windows(2)
            .any(|pair| pair[0].as_str() == "--exclude" && pair[1].as_str() == "tos-conformance")
        && !cargo_args.iter().any(|part| part == "--no-run")
}
fn target() -> io::Result<PathBuf> {
    let path = PathBuf::from(
        std::env::var_os("CARGO_TARGET_DIR")
            .ok_or_else(|| invalid("CARGO_TARGET_DIR required for conformance products"))?,
    );
    if !path.is_absolute() || path.canonicalize()? != path {
        return Err(invalid(
            "conformance target must be an exact absolute directory",
        ));
    }
    Ok(path)
}
fn image(path: &Path, target: &Path, deadline: Instant, cancel: &AtomicI32) -> io::Result<String> {
    check(deadline, cancel)?;
    if !path.is_absolute() || !path.starts_with(target) || path.canonicalize()? != path {
        return Err(invalid("Cargo executable outside exact target"));
    }
    let mut file = File::options()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let before = file.metadata()?;
    if !before.is_file() || before.mode() & 0o111 == 0 {
        return Err(invalid("Cargo product is not an executable regular file"));
    }
    let stamp = |m: &fs::Metadata| {
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
    let mut digest = Digest256Hasher::new();
    let mut buffer = [0u8; 65536];
    let mut remaining = before.len();
    while remaining != 0 {
        check(deadline, cancel)?;
        let cap = remaining.min(buffer.len() as u64) as usize;
        let count = file.read(&mut buffer[..cap])?;
        if count == 0 {
            return Err(invalid("Cargo product truncated during hash"));
        }
        digest.update(&buffer[..count]);
        remaining -= count as u64;
    }
    check(deadline, cancel)?;
    if stamp(&before) != stamp(&file.metadata()?)
        || stamp(&before) != stamp(&fs::symlink_metadata(path)?)
    {
        return Err(invalid("Cargo product changed during hash"));
    }
    Ok(digest.finalize().to_hex())
}

#[derive(Clone)]
pub(crate) struct PreparedAccess {
    target: PathBuf,
    executable: PathBuf,
    executable_sha: String,
}
impl PreparedAccess {
    pub(crate) fn select(deadline: Instant, cancel: &AtomicI32) -> io::Result<Self> {
        Self::select_for_target(target()?, deadline, cancel)
    }

    fn select_for_target(
        target_dir: PathBuf,
        deadline: Instant,
        cancel: &AtomicI32,
    ) -> io::Result<Self> {
        if target()? != target_dir {
            return Err(invalid(
                "conformance target changed during Access selection",
            ));
        }
        let executable = target_dir.join("debug/tos-access");
        let executable_sha = image(&executable, &target_dir, deadline, cancel)?;
        if target()? != target_dir {
            return Err(invalid(
                "conformance target changed during Access selection",
            ));
        }
        Ok(Self {
            target: target_dir,
            executable,
            executable_sha,
        })
    }

    pub(crate) fn environment(
        &self,
        deadline: Instant,
        cancel: &AtomicI32,
        publication_case_sha: &str,
    ) -> io::Result<Vec<(String, String)>> {
        if target()? != self.target
            || image(&self.executable, &self.target, deadline, cancel)? != self.executable_sha
        {
            return Err(invalid("prepared Access consumer changed after selection"));
        }
        Ok(vec![
            (
                "TOS_NATIVE_PREPARED_CONSUMER_BIN".into(),
                self.executable
                    .to_str()
                    .ok_or_else(|| invalid("non UTF-8 executable"))?
                    .into(),
            ),
            (
                "TOS_NATIVE_PREPARED_CONSUMER_SHA256".into(),
                self.executable_sha.clone(),
            ),
            (
                "TOS_NATIVE_CLAIM_PUBLICATION_CASE_SHA256".into(),
                publication_case_sha.into(),
            ),
        ])
    }
}

#[derive(Clone)]
pub(crate) struct Products {
    target: PathBuf,
    executable: PathBuf,
    executable_sha: String,
    access: PreparedAccess,
    command_tests: Option<(String, PathBuf, String)>,
}
impl Products {
    pub(crate) fn select(stdout: &[u8], deadline: Instant, cancel: &AtomicI32) -> io::Result<Self> {
        let target = target()?;
        let mut executable = None;
        let mut command_tests = None;
        for line in stdout.split(|b| *b == b'\n').filter(|l| !l.is_empty()) {
            check(deadline, cancel)?;
            let value: serde_json::Value = serde_json::from_slice(line)
                .map_err(|_| invalid("invalid Cargo JSON artifact stream"))?;
            if value["reason"] != "compiler-artifact" {
                continue;
            }
            let package = value["package_id"].as_str().unwrap_or("");
            let package_matches = package.starts_with("tos-conformance ")
                || package
                    .rsplit('#')
                    .next()
                    .is_some_and(|p| p.starts_with("tos-conformance@"));
            let command_package = package.starts_with("tos-command ")
                || package
                    .rsplit('#')
                    .next()
                    .is_some_and(|p| p.starts_with("tos-command@"))
                || package
                    .split('#')
                    .next()
                    .is_some_and(|p| p.ends_with("/tos-command"));
            if command_package
                && value["profile"]["test"] == true
                && value["target"]["kind"]
                    .as_array()
                    .is_some_and(|k| k.iter().any(|k| k == "lib"))
            {
                if let Some(path) = value["executable"].as_str() {
                    let name = value["target"]["name"]
                        .as_str()
                        .ok_or_else(|| invalid("command library target has no name"))?;
                    if command_tests
                        .replace((name.to_owned(), PathBuf::from(path)))
                        .is_some()
                    {
                        return Err(invalid(
                            "ambiguous current-lane command library test executable",
                        ));
                    }
                }
            }
            if package_matches
                && value["target"]["name"] == "conformance"
                && value["target"]["kind"]
                    .as_array()
                    .is_some_and(|v| v.iter().any(|x| x == "test"))
            {
                let path = value["executable"]
                    .as_str()
                    .ok_or_else(|| invalid("conformance Cargo artifact has no executable"))?;
                if executable.replace(PathBuf::from(path)).is_some() {
                    return Err(invalid("ambiguous conformance Cargo executable"));
                }
            }
        }
        let executable = executable
            .ok_or_else(|| invalid("current Cargo stream lacks conformance executable"))?;
        let executable_sha = image(&executable, &target, deadline, cancel)?;
        let access = PreparedAccess::select_for_target(target.clone(), deadline, cancel)?;
        let command_tests = command_tests
            .map(|(name, path)| {
                let digest = image(&path, &target, deadline, cancel)?;
                Ok::<_, io::Error>((name, path, digest))
            })
            .transpose()?;
        Ok(Self {
            target,
            executable,
            executable_sha,
            access,
            command_tests,
        })
    }
    pub(crate) fn growth_command(
        &self,
        command: &[String],
        deadline: Instant,
        cancel: &AtomicI32,
    ) -> io::Result<Vec<String>> {
        if command.len() < 7 || command[0] != crate::growth_native_plan::NATIVE_CLASS {
            return Err(invalid(
                "invalid source-owned native Growth class invocation",
            ));
        }
        let (executable, expected_digest) = match (
            command[1].as_str(),
            command[2].as_str(),
            command[3].as_str(),
        ) {
            ("tos-conformance", "test", "conformance") => (&self.executable, &self.executable_sha),
            ("tos-command", "lib", name) => {
                let (selected_name, executable, digest) =
                    self.command_tests.as_ref().ok_or_else(|| {
                        invalid("current Cargo stream lacks command library test image")
                    })?;
                if name != selected_name {
                    return Err(invalid(
                        "native class differs from current Cargo library target",
                    ));
                }
                (executable, digest)
            }
            _ => {
                return Err(invalid(
                    "native class has no current-lane prepared test product",
                ));
            }
        };
        // The existing environment selector rechecks Conformance and Access
        // immediately before spawn; only the additional library image needs
        // its own hash here. Avoid reading Conformance twice per class.
        if target()? != self.target
            || (executable != &self.executable
                && image(executable, &self.target, deadline, cancel)? != *expected_digest)
        {
            return Err(invalid(
                "native Growth test product changed after preparation",
            ));
        }
        let executable = executable
            .to_str()
            .ok_or_else(|| invalid("non-UTF8 native class executable"))?;
        let mut argv = vec![
            executable.into(),
            command[4].clone(),
            "--nocapture".into(),
            "--test-threads=1".into(),
        ];
        argv.extend_from_slice(&command[7..]);
        Ok(argv)
    }

    pub(crate) fn environment(
        &self,
        deadline: Instant,
        cancel: &AtomicI32,
    ) -> io::Result<Vec<(String, String)>> {
        if target()? != self.target
            || self.access.target != self.target
            || image(&self.executable, &self.target, deadline, cancel)? != self.executable_sha
        {
            return Err(invalid("conformance products changed after preparation"));
        }
        self.access
            .environment(deadline, cancel, &self.executable_sha)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn post_build_selection_gets_its_own_budget_but_cannot_renew_the_lane() {
        let compile_started = Instant::now();
        let phase_wall = crate::executor::Limits::default().command_wall;
        let compile_deadline = compile_started + phase_wall;
        let remaining_compile_budget = Duration::from_secs(8);
        let compile_finished = compile_deadline - remaining_compile_budget;
        let selection_started =
            compile_finished + remaining_compile_budget + Duration::from_millis(1);
        let lane_deadline = compile_started + Duration::from_secs(10_800);

        let selection = selection_deadline(selection_started, lane_deadline, phase_wall);
        assert!(selection_started > compile_deadline);
        assert_eq!(selection, selection_started + phase_wall);
        assert!(selection > compile_deadline);

        let expired_lane = Instant::now() - Duration::from_millis(1);
        let bounded = selection_deadline(Instant::now(), expired_lane, phase_wall);
        assert_eq!(bounded, expired_lane);
        assert!(check(bounded, &AtomicI32::new(0)).is_err());

        let cancelled = AtomicI32::new(15);
        let fresh_lane = Instant::now() + phase_wall;
        let bounded = selection_deadline(Instant::now(), fresh_lane, phase_wall);
        assert!(check(bounded, &cancelled).is_err());
    }
}
