//! A synthetic native validation sequence exercises ordered execution and durable
//! lifecycle risks through the real CLI, without executing repository tools.

#[cfg(target_os = "linux")]
mod growth_parallel {
    use serde_json::json;
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        path::PathBuf,
        process::{Command, Stdio},
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "tos-growth-parallel-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(root.join("docs/validation")).unwrap();
            fs::create_dir_all(root.join("cargo-target/debug/deps")).unwrap();
            let executable = |name: &str, raw: &[u8]| {
                let path = root.join(name);
                fs::write(&path, raw).unwrap();
                fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
            };
            executable("cargo-target/debug/tos-access", b"fixture exact consumer");
            executable("cargo", b"#!/bin/sh\n/bin/cat artifacts.jsonl\n");
            executable(
                "cargo-target/debug/deps/conformance-fixture",
                br#"#!/bin/sh
set -eu
name=${1%::}
[ "${3-}" = --test-threads=1 ] || exit 31
counter() {
  while ! mkdir counter.lock 2>/dev/null; do /bin/sleep 0.002; done
  active=$(/bin/cat active.count 2>/dev/null || echo 0)
  active=$((active + $1))
  printf '%s\n' "$active" > active.count
  maximum=$(/bin/cat max.count 2>/dev/null || echo 0)
  if [ "$active" -gt "$maximum" ]; then printf '%s\n' "$active" > max.count; fi
  rmdir counter.lock
  [ "$active" -le 2 ] || exit 32
  if [ "$SCENARIO" = serial ]; then [ "$active" -le 1 ] || exit 33; fi
}
echo $$ > "$name.pid"
mkdir "active-$name"
printf '%s\n' "$name" >> starts
counter 1
if [ "$SCENARIO" != success ] && [ "$SCENARIO" != serial ] && [ "$name" = beta ]; then
  /bin/sleep 20 & echo $! > ordinary.pid
  /usr/bin/setsid /bin/sh -c 'echo $$ > escaped.pid; exec /bin/sleep 20' &
  while [ ! -s escaped.pid ]; do /bin/sleep 0.002; done
fi
touch "$name.ready"
if [ "$SCENARIO" = success ]; then
  if [ "$name" != gamma ]; then
    while [ ! -f alpha.ready ] || [ ! -f beta.ready ]; do /bin/sleep 0.002; done
  fi
  /bin/sleep 0.03
elif [ "$SCENARIO" != serial ]; then
  if [ "$name" = alpha ]; then
    while [ ! -f beta.ready ]; do /bin/sleep 0.002; done
    case "$SCENARIO" in
      failure) exit 7;;
      count) echo 'test result: ok. 0 passed; 0 failed; 0 ignored;'; exit 0;;
      output) /usr/bin/head -c 65536 /dev/zero; printf '\n';;
      blocked) /usr/bin/yes x | /usr/bin/head -c 262144;;
    esac
  fi
  /bin/sleep 20
fi
rmdir "active-$name"
counter -1
touch "$name.done"
echo 'test result: ok. 1 passed; 0 failed; 0 ignored;'
"#,
            );
            let row = json!({"reason":"compiler-artifact","package_id":"path+file:///fixture#tos-conformance@0.0.0","target":{"name":"conformance","kind":["test"]},"executable":root.join("cargo-target/debug/deps/conformance-fixture")});
            fs::write(root.join("artifacts.jsonl"), row.to_string() + "\n").unwrap();
            let mut steps = vec![
                json!({"label":"prepare","command":[root.join("cargo"),"test","--workspace","--no-run","--exclude","tos-access","--message-format=json"]}),
            ];
            for name in ["alpha", "beta", "gamma"] {
                steps.push(json!({"label":name,"command":["@tos-native-growth-class","tos-conformance","test","conformance",format!("{name}::"),"1","0"]}));
            }
            steps.push(json!({"label":"joined","command":["/bin/sh","-c","test -f alpha.done && test -f beta.done && test -f gamma.done && touch joined"]}));
            fs::write(
                root.join("docs/validation/validation_lanes.json"),
                json!({"command_sequences":{"rust_workspace":steps}}).to_string(),
            )
            .unwrap();
            Self(root)
        }
        fn command(&self, scenario: &str, jobs: Option<&str>) -> Command {
            let executable = std::env::var_os("TOS_VALIDATION_LANES_TEST_EXECUTABLE")
                .unwrap_or_else(|| env!("CARGO_BIN_EXE_tos-validation-lanes").into());
            let mut command = Command::new(executable);
            command.arg("--repo-root").arg(&self.0).args([
                "--run",
                "rust_workspace",
                "--command-timeout-ms",
                if scenario == "success" || scenario == "serial" {
                    "5000"
                } else {
                    "1000"
                },
                "--lane-timeout-ms",
                "8000",
                "--cleanup-grace-ms",
                "500",
                "--max-output-bytes",
                if scenario == "output" {
                    "1024"
                } else {
                    "1048576"
                },
            ]);
            if let Some(jobs) = jobs {
                command.args(["--growth-jobs", jobs]);
            }
            command
                .env("CARGO_TARGET_DIR", self.0.join("cargo-target"))
                .env("SCENARIO", scenario)
                .env("PATH", "/usr/bin:/bin")
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            command
        }
        fn assert_reaped(&self) {
            for name in [
                "alpha.pid",
                "beta.pid",
                "gamma.pid",
                "ordinary.pid",
                "escaped.pid",
            ] {
                if let Ok(pid) = fs::read_to_string(self.0.join(name)) {
                    assert!(
                        !std::path::Path::new("/proc").join(pid.trim()).exists(),
                        "residual {name}: {pid}"
                    );
                }
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn two_growth_processes_overlap_keep_counts_and_join_before_later_commands() {
        for (scenario, jobs) in [("success", Some("2")), ("serial", None)] {
            let fixture = Fixture::new();
            let output = fixture.command(scenario, jobs).output().unwrap();
            assert!(
                output.status.success(),
                "{scenario}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(fixture.0.join("joined").is_file());
            assert_eq!(
                fs::read_to_string(fixture.0.join("max.count"))
                    .unwrap()
                    .trim(),
                if jobs.is_some() { "2" } else { "1" }
            );
            assert_eq!(
                fs::read_to_string(fixture.0.join("active.count"))
                    .unwrap()
                    .trim(),
                "0"
            );
            assert_eq!(
                fs::read_to_string(fixture.0.join("starts"))
                    .unwrap()
                    .lines()
                    .count(),
                3
            );
            let text = String::from_utf8_lossy(&output.stdout);
            assert_eq!(
                text.contains("native Growth batch: classes=3 jobs=2"),
                jobs.is_some()
            );
            for name in ["alpha", "beta", "gamma"] {
                assert!(text.contains(&format!("[ok] {name}\n")));
            }
            fixture.assert_reaped();
        }
        for jobs in ["0", "3", "invalid"] {
            let fixture = Fixture::new();
            let output = fixture.command("success", Some(jobs)).output().unwrap();
            assert_eq!(output.status.code(), Some(2));
            assert!(!fixture.0.join("starts").exists());
        }
    }

    #[test]
    fn growth_failure_count_timeout_cancel_and_blocked_output_reap_both_process_trees() {
        for scenario in ["failure", "count", "timeout", "cancel", "output", "blocked"] {
            let fixture = Fixture::new();
            let started = Instant::now();
            let mut child = fixture.command(scenario, Some("2")).spawn().unwrap();
            if scenario == "cancel" {
                while !fixture.0.join("beta.ready").exists()
                    && started.elapsed() < Duration::from_secs(3)
                {
                    std::thread::sleep(Duration::from_millis(2));
                }
                assert!(fixture.0.join("beta.ready").exists());
                assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGINT) }, 0);
            }
            if scenario == "blocked" {
                while child.try_wait().unwrap().is_none()
                    && started.elapsed() < Duration::from_secs(4)
                {
                    std::thread::sleep(Duration::from_millis(5));
                }
                assert!(
                    child.try_wait().unwrap().is_some(),
                    "blocked parallel output escaped command deadline"
                );
            }
            let output = child.wait_with_output().unwrap();
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "{scenario} exceeded bounded cleanup"
            );
            assert_eq!(
                output.status.code(),
                Some(match scenario {
                    "failure" => 7,
                    "cancel" => 130,
                    _ => 1,
                }),
                "{scenario}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(
                !fixture.0.join("gamma.ready").exists(),
                "{scenario}: queued class started after failure"
            );
            assert!(!fixture.0.join("gamma.pid").exists());
            assert!(
                !fs::read_to_string(fixture.0.join("starts"))
                    .unwrap_or_default()
                    .lines()
                    .any(|line| line == "gamma")
            );
            assert!(!fixture.0.join("joined").exists());
            assert!(
                fixture.0.join("escaped.pid").is_file(),
                "{scenario}: peer descendant was not exercised"
            );
            fixture.assert_reaped();
        }
    }
}

#[cfg(target_os = "linux")]
#[test]
fn ordered_runner_stops_and_owns_ordinary_and_escaped_children() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
    let root = std::env::temp_dir().join(format!(
        "tos-mechanics-execute-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(root.join("docs/validation")).unwrap();
    fs::write(root.join("docs/validation/validation_lanes.json"), r#"{"command_sequences":{"custody":[{"label":"assertions","command":["/bin/sh","adapter","assertions"]},{"label":"builder","command":["/bin/sh","adapter","builder","--check"]},{"label":"validator","command":["/bin/sh","adapter","validator"]}]}}"#).unwrap();
    let adapter = root.join("adapter");
    fs::write(
        &adapter,
        r#"#!/bin/sh
printf '%s\n' "$*" >> trace
case "$SCENARIO" in
  failure) exit 7;;
  output) head -c 65536 /dev/zero; sleep 10;;
  timeout|cancel|blocked)
    /bin/sleep 10 & echo $! > ordinary.pid
    /usr/bin/setsid /bin/sh -c 'echo $$ > escaped.pid; echo $$ >> escaped-all; exec /bin/sleep 10' &
    while [ ! -s escaped.pid ]; do sleep 0.001; done
    echo ready
    if [ "$SCENARIO" = blocked ]; then head -c 1048576 /dev/zero; fi
    wait;;
  daemon)
    /usr/bin/setsid /bin/sh -c 'echo $$ > escaped.pid; echo $$ >> escaped-all; exec /bin/sleep 10' &
    while [ ! -s escaped.pid ]; do sleep 0.001; done;;
esac
"#,
    )
    .unwrap();
    fs::set_permissions(&adapter, fs::Permissions::from_mode(0o700)).unwrap();
    for scenario in [
        "success",
        "failure",
        "timeout",
        "output",
        "cancel",
        "blocked",
        "daemon",
        "unavailable",
    ] {
        for path in ["trace", "ordinary.pid", "escaped.pid", "escaped-all"] {
            let _ = fs::remove_file(root.join(path));
        }
        let executable = std::env::var_os("TOS_VALIDATION_LANES_TEST_EXECUTABLE")
            .unwrap_or_else(|| env!("CARGO_BIN_EXE_tos-validation-lanes").into());
        let mut command = Command::new(executable);
        command
            .args([
                "--repo-root",
                root.to_str().unwrap(),
                "--run",
                "custody",
                "--command-timeout-ms",
                "200",
                "--lane-timeout-ms",
                "1500",
                "--cleanup-grace-ms",
                "500",
                "--max-output-bytes",
                if scenario == "output" {
                    "1024"
                } else {
                    "2097152"
                },
            ])
            .env("SCENARIO", scenario)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if scenario == "unavailable" {
            // Allow pidfd_open but deny the send syscall in this CLI child
            // only. Capability rejection must precede any adapter/tool fork.
            unsafe {
                command.pre_exec(|| {
                    let mut filter = [
                        libc::sock_filter {
                            code: (libc::BPF_LD | libc::BPF_W | libc::BPF_ABS) as u16,
                            jt: 0,
                            jf: 0,
                            k: 0,
                        },
                        libc::sock_filter {
                            code: (libc::BPF_JMP | libc::BPF_JEQ | libc::BPF_K) as u16,
                            jt: 0,
                            jf: 1,
                            k: libc::SYS_pidfd_send_signal as u32,
                        },
                        libc::sock_filter {
                            code: (libc::BPF_RET | libc::BPF_K) as u16,
                            jt: 0,
                            jf: 0,
                            k: libc::SECCOMP_RET_ERRNO | libc::EPERM as u32,
                        },
                        libc::sock_filter {
                            code: (libc::BPF_RET | libc::BPF_K) as u16,
                            jt: 0,
                            jf: 0,
                            k: libc::SECCOMP_RET_ALLOW,
                        },
                    ];
                    let program = libc::sock_fprog {
                        len: filter.len() as u16,
                        filter: filter.as_mut_ptr(),
                    };
                    if libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0
                        || libc::prctl(
                            libc::PR_SET_SECCOMP,
                            libc::SECCOMP_MODE_FILTER,
                            &program,
                            0,
                            0,
                        ) != 0
                    {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
        }
        let start = Instant::now();
        let mut child = command.spawn().unwrap();
        if scenario == "cancel" {
            while !root.join("escaped.pid").exists() && start.elapsed() < Duration::from_secs(1) {
                std::thread::sleep(Duration::from_millis(2));
            }
            assert!(root.join("escaped.pid").exists());
            unsafe {
                assert_eq!(libc::kill(child.id() as i32, libc::SIGINT), 0);
            }
        }
        if scenario == "blocked" {
            // Do not drain supervisor stdout until after its deadline. A full
            // sink must not keep the supervisor or its descendants running.
            while child.try_wait().unwrap().is_none() && start.elapsed() < Duration::from_secs(2) {
                std::thread::sleep(Duration::from_millis(5));
            }
            assert!(
                child.try_wait().unwrap().is_some(),
                "blocked output sink escaped deadline"
            );
        }
        let output = child.wait_with_output().unwrap();
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "{scenario} exceeded bounded return"
        );
        assert_eq!(
            output.status.code(),
            Some(match scenario {
                "success" | "daemon" => 0,
                "cancel" => 130,
                "failure" => 7,
                _ => 1,
            }),
            "{scenario}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        if scenario == "unavailable" {
            assert!(
                !root.join("trace").exists(),
                "unsupported custody host started a tool"
            );
            assert!(
                output.stdout.is_empty(),
                "capability probe followed command-start output"
            );
            continue;
        }
        let trace = fs::read_to_string(root.join("trace")).unwrap();
        assert_eq!(
            trace.lines().count(),
            if scenario == "success" || scenario == "daemon" {
                3
            } else {
                1
            },
            "{scenario}"
        );
        if scenario == "success" {
            assert_eq!(trace, "assertions\nbuilder --check\nvalidator\n");
            assert!(String::from_utf8_lossy(&output.stdout).ends_with("[ok] validator\n"));
        }
        for path in ["ordinary.pid", "escaped-all"] {
            if let Ok(value) = fs::read_to_string(root.join(path)) {
                for value in value.lines() {
                    let pid: i32 = value.trim().parse().unwrap();
                    assert!(
                        !std::path::Path::new(&format!("/proc/{pid}")).exists(),
                        "{scenario} left descendant {pid}"
                    );
                }
            }
        }
    }
    fs::remove_dir_all(root).unwrap();
}

// Native validation sequences preserve command order and first-failure stop.
#[cfg(target_os = "linux")]
#[test]
fn native_validation_sequences_preserve_order_and_first_failure() {
    use std::{
        fs,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };
    let root = std::env::temp_dir().join(format!(
        "tos-validation-lanes-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(root.join("docs/validation")).unwrap();
    fs::write(
        root.join("docs/validation/validation_lanes.json"),
        r#"{"command_sequences":{"sample":[{"label":"first","command":["/bin/sh","adapter.sh","first"]},{"label":"failing","command":["/bin/sh","adapter.sh","fail"]},{"label":"later","command":["/bin/sh","adapter.sh","later"]}],"rust_workspace":[{"label":"budgeted slow","command":["/bin/sh","adapter.sh","slow"],"command_timeout_ms":500},{"label":"never after timeout","command":["/bin/sh","adapter.sh","later"]}]}}"#,
    )
    .unwrap();
    let adapter = root.join("adapter.sh");
    fs::write(
        &adapter,
        "printf '%s\\n' \"$1\" >> trace\n[ \"$1\" = slow ] && /bin/sleep 5\n[ \"$1\" = fail ] && exit 17\nexit 0\n",
    )
    .unwrap();
    let executable = std::env::var_os("TOS_VALIDATION_LANES_TEST_EXECUTABLE")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_tos-validation-lanes").into());
    let output = Command::new(&executable)
        .arg("--repo-root")
        .arg(&root)
        .args(["--sequence", "sample", "--run", "sample"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(17));
    assert_eq!(
        fs::read_to_string(root.join("trace")).unwrap(),
        "first\nfail\n"
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("first: /bin/sh adapter.sh first\n"));
    assert!(stdout.contains("[ok] first\n"));
    assert!(stdout.contains("[run] failing: /bin/sh adapter.sh fail\n"));
    assert!(!stdout.contains("[run] later:"));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("[error] failing failed with exit code 17\n")
    );

    let legacy_option = Command::new(&executable)
        .arg("--repo-root")
        .arg(&root)
        .args(["--python", "/bin/sh", "--sequence", "sample"])
        .output()
        .unwrap();
    assert_eq!(legacy_option.status.code(), Some(2));
    assert!(
        String::from_utf8(legacy_option.stderr)
            .unwrap()
            .contains("unknown argument: --python")
    );
    assert_eq!(
        fs::read_to_string(root.join("trace")).unwrap(),
        "first\nfail\n"
    );

    fs::remove_file(root.join("trace")).unwrap();
    let output = Command::new(&executable)
        .arg("--repo-root")
        .arg(&root)
        .args(["--run", "rust_workspace"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("[run] budgeted slow: /bin/sh adapter.sh slow\n"));
    assert!(
        stdout
            .contains("[budget] budgeted slow: command_timeout_ms=500 lane_wall_cap_ms=3600000\n")
    );
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("execution wall deadline")
    );
    let trace = match fs::read_to_string(root.join("trace")) {
        Ok(trace) => trace,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => panic!("cannot read budgeted command trace: {error}"),
    };
    assert!(trace.is_empty() || trace == "slow\n");
    assert!(!trace.lines().any(|line| line == "later"));
    fs::remove_dir_all(root).unwrap();
}

// Release uses the same authored sequence and process custody, but preserves
// its own phase, inherited environment, command-display, and failure contract.
#[cfg(target_os = "linux")]
#[test]
fn release_phase_selection_preserves_environment_and_first_failure() {
    use serde_json::json;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::process::{Command, Output};
    use std::time::{SystemTime, UNIX_EPOCH};

    let root = std::env::temp_dir().join(format!(
        "tos-release-check-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(root.join("docs/validation")).unwrap();
    let manifest = root.join("docs/validation/validation_lanes.json");
    let adapter = root.join("adapter");
    fs::write(
        &adapter,
        "#!/bin/sh\nprintf '%s|%s\\n' \"$1\" \"${TOS_RELEASE_TEST_INHERITED-<unset>}\" >> trace\n[ \"$1\" = fail ] && [ \"$FAIL_SECOND\" = 1 ] && exit 17\nexit 0\n",
    )
    .unwrap();
    fs::set_permissions(&adapter, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(
        &manifest,
        serde_json::to_vec(&json!({"command_sequences":{"release_check":[
            {"label":"software contracts","command":[adapter.to_str().unwrap(), "quoted \"tail\\"]},
            {"label":"build software browser assets","command":[adapter.to_str().unwrap(), "fail"]},
            {"label":"run tests: access","command":[adapter.to_str().unwrap(), "first"]},
            {"label":"run tests: source","command":[adapter.to_str().unwrap(), "second"]}
        ]}}))
        .unwrap(),
    )
    .unwrap();
    let executable = std::env::var_os("TOS_RELEASE_CHECK_TEST_EXECUTABLE")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_tos-release-check").into());
    let invoke = |phase: &str, fail: bool, inherited: Option<&str>| -> Output {
        let mut command = Command::new(&executable);
        command.args([
            "--repo-root",
            root.to_str().unwrap(),
            "--phase",
            phase,
            "--command-timeout-ms",
            "2000",
            "--lane-timeout-ms",
            "5000",
        ]);
        command.env("FAIL_SECOND", if fail { "1" } else { "0" });
        if let Some(inherited) = inherited {
            command.env("TOS_RELEASE_TEST_INHERITED", inherited);
        } else {
            command.env_remove("TOS_RELEASE_TEST_INHERITED");
        }
        command.output().unwrap()
    };
    let checks = invoke("checks", false, None);
    assert_eq!(checks.status.code(), Some(0));
    assert_eq!(
        fs::read_to_string(root.join("trace")).unwrap(),
        "quoted \"tail\\|<unset>\nfail|<unset>\n"
    );
    let checks_out = String::from_utf8(checks.stdout).unwrap();
    let expected_display = format!(
        r#"[run] software contracts: {} "quoted \"tail\\""#,
        adapter.display()
    );
    assert!(checks_out.contains(&expected_display));
    assert!(!checks_out.contains("[run] run tests:"));
    assert!(!checks_out.contains("[ok]"));

    fs::remove_file(root.join("trace")).unwrap();
    let tests = invoke("tests", false, Some("0"));
    assert_eq!(tests.status.code(), Some(0));
    assert_eq!(
        fs::read_to_string(root.join("trace")).unwrap(),
        "first|0\nsecond|0\n"
    );
    assert_eq!(
        String::from_utf8(tests.stdout).unwrap(),
        format!(
            "[run] run tests: access: {} first\n[run] run tests: source: {} second\n",
            adapter.display(),
            adapter.display()
        )
    );

    fs::remove_file(root.join("trace")).unwrap();
    let all = invoke("all", true, None);
    assert_eq!(all.status.code(), Some(17));
    assert_eq!(
        fs::read_to_string(root.join("trace")).unwrap(),
        "quoted \"tail\\|<unset>\nfail|<unset>\n"
    );
    let all_out = String::from_utf8(all.stdout).unwrap();
    assert!(all_out.contains("[error] build software browser assets failed with exit code 17\n"));
    assert!(!all_out.contains("[run] run tests:"));
    assert!(all.stderr.is_empty());

    fs::remove_file(root.join("trace")).unwrap();
    fs::write(
        &manifest,
        serde_json::to_vec(&json!({"command_sequences":{"release_check":[
            {"label":"run tests: ","command":["python", "early"]}
        ]}}))
        .unwrap(),
    )
    .unwrap();
    let invalid = invoke("checks", false, None);
    assert_eq!(invalid.status.code(), Some(2));
    assert!(!root.join("trace").exists());
    assert!(
        String::from_utf8(invalid.stdout)
            .unwrap()
            .contains("[error] Python commands are unsupported in maintained validation lanes\n")
    );
    let legacy_option = Command::new(&executable)
        .args([
            "--repo-root",
            root.to_str().unwrap(),
            "--python",
            adapter.to_str().unwrap(),
            "--phase",
            "checks",
        ])
        .output()
        .unwrap();
    assert_eq!(legacy_option.status.code(), Some(2));
    assert!(
        String::from_utf8(legacy_option.stderr)
            .unwrap()
            .contains("unknown argument: --python")
    );
    assert!(!root.join("trace").exists());
    fs::remove_dir_all(root).unwrap();
}
