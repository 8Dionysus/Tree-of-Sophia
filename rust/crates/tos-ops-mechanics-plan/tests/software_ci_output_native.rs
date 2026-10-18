//! Real CLI output custody refuses a FIFO before a blocking writer open.
#[cfg(target_os = "linux")]
#[test]
fn software_ci_output_fifo_refuses_without_a_reader_or_hang() {
    use std::{
        ffi::CString,
        fs,
        os::unix::ffi::OsStrExt,
        process::{Command, Stdio},
        thread,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };
    let root = std::env::temp_dir().join(format!(
        "tos-ci-output-fifo-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let git = |args: &[&str]| {
        let result = Command::new("git")
            .args(args)
            .current_dir(&root)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    };
    git(&["init", "--initial-branch=main"]);
    git(&["config", "user.name", "CI output fixture"]);
    git(&["config", "user.email", "fixture@example.invalid"]);
    git(&["config", "commit.gpgsign", "false"]);
    git(&["config", "core.hooksPath", "/dev/null"]);
    fs::write(root.join("README.md"), "synthetic\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "--quiet", "-m", "synthetic"]);
    let output = root.join("github-output.fifo");
    let cpath = CString::new(output.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(cpath.as_ptr(), 0o600) }, 0);
    let executable = std::env::var_os("TOS_SOFTWARE_CI_TEST_EXECUTABLE")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_tos-software-ci").into());
    let mut child = Command::new(executable)
        .args(["plan", "--repo-root"])
        .arg(&root)
        .args(["--base", "HEAD", "--full"])
        .env("GITHUB_OUTPUT", &output)
        .current_dir(&root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut timed_out = false;
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if Instant::now() >= deadline {
            timed_out = true;
            child.kill().unwrap(); // Only the fixture's own CLI child.
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    let result = child.wait_with_output().unwrap();
    fs::remove_dir_all(&root).unwrap();
    assert!(
        !timed_out,
        "GITHUB_OUTPUT FIFO blocked; fixture child killed and reaped"
    );
    assert_eq!(result.status.code(), Some(1));
    assert!(!result.stderr.is_empty());
    let selection: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(selection["software_mode"], "full");
}
