//! One synthetic package fixture exercises ordered execution and durable
//! lifecycle risks through the real CLI, without executing repository tools.
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
    for path in [
        "mechanics/fixture/tests/test_unit.py",
        "mechanics/fixture/scripts/build_unit.py",
        "mechanics/fixture/scripts/validate_unit.py",
    ] {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"").unwrap();
    }
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
        let executable = std::env::var_os("TOS_MECHANICS_TEST_EXECUTABLE")
            .unwrap_or_else(|| env!("CARGO_BIN_EXE_tos-ops-mechanics-plan").into());
        let mut command = Command::new(executable);
        command
            .args([
                "--repo-root",
                root.to_str().unwrap(),
                "--python",
                adapter.to_str().unwrap(),
                "--execute",
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
            assert_eq!(
                trace,
                "-m unittest discover -s mechanics/fixture/tests -p test*.py\nmechanics/fixture/scripts/build_unit.py --check\nmechanics/fixture/scripts/validate_unit.py\n"
            );
            assert!(String::from_utf8_lossy(&output.stdout).ends_with("[ok] completed mechanics-local unittest, builder, and validator coverage across 1 test files\n"));
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

// The manifest is the command authority. This one controlled child sequence
// guards exact Python substitution, authored order, and first-failure stop.
#[cfg(target_os = "linux")]
#[test]
fn validation_lane_selection_runs_in_order_and_stops_at_first_failure() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

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
        r#"{"command_sequences":{"sample":[{"label":"first","command":["python","first"]},{"label":"failing","command":["python","fail"]},{"label":"later","command":["python","later"]}]}}"#,
    )
    .unwrap();
    let adapter = root.join("adapter");
    fs::write(
        &adapter,
        "#!/bin/sh\nprintf '%s\\n' \"$1\" >> trace\n[ \"$1\" = fail ] && exit 17\nexit 0\n",
    )
    .unwrap();
    fs::set_permissions(&adapter, fs::Permissions::from_mode(0o700)).unwrap();
    let executable = std::env::var_os("TOS_VALIDATION_LANES_TEST_EXECUTABLE")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_tos-validation-lanes").into());
    let output = Command::new(executable)
        .args([
            "--repo-root",
            root.to_str().unwrap(),
            "--python",
            adapter.to_str().unwrap(),
            "--sequence",
            "sample",
            "--run",
            "sample",
            "--command-timeout-ms",
            "2000",
            "--lane-timeout-ms",
            "5000",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(17));
    assert_eq!(
        fs::read_to_string(root.join("trace")).unwrap(),
        "first\nfail\n"
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains(&format!("first: {} first\n", adapter.display())));
    assert!(stdout.contains("[ok] first\n"));
    assert!(stdout.contains(&format!("[run] failing: {} fail\n", adapter.display())));
    assert!(!stdout.contains("[run] later:"));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("[error] failing failed with exit code 17\n")
    );
    fs::remove_dir_all(root).unwrap();
}

// Release uses the same authored sequence and process custody, but preserves
// its own phase, environment, Windows command-display, and failure contract.
#[cfg(target_os = "linux")]
#[test]
fn release_phase_selection_preserves_environment_and_first_failure() {
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
    fs::write(
        &manifest,
        r#"{"command_sequences":{"release_check":[{"label":"software contracts","command":["python","quoted \"tail\\"]},{"label":"build software browser assets","command":["python","fail"]},{"label":"run tests","command":["python","tests"]}]}}"#,
    )
    .unwrap();
    let adapter = root.join("adapter");
    fs::write(
        &adapter,
        "#!/bin/sh\nprintf '%s|%s\\n' \"$1\" \"${PYTEST_DISABLE_PLUGIN_AUTOLOAD-<unset>}\" >> trace\n[ \"$1\" = fail ] && [ \"$FAIL_SECOND\" = 1 ] && exit 17\nexit 0\n",
    )
    .unwrap();
    fs::set_permissions(&adapter, fs::Permissions::from_mode(0o700)).unwrap();
    let executable = std::env::var_os("TOS_RELEASE_CHECK_TEST_EXECUTABLE")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_tos-release-check").into());
    let invoke = |phase: &str, fail: bool, inherited: Option<&str>| -> Output {
        let mut command = Command::new(&executable);
        command.args([
            "--repo-root",
            root.to_str().unwrap(),
            "--python",
            adapter.to_str().unwrap(),
            "--phase",
            phase,
            "--command-timeout-ms",
            "2000",
            "--lane-timeout-ms",
            "5000",
        ]);
        command.env("FAIL_SECOND", if fail { "1" } else { "0" });
        if let Some(inherited) = inherited {
            command.env("PYTEST_DISABLE_PLUGIN_AUTOLOAD", inherited);
        } else {
            command.env_remove("PYTEST_DISABLE_PLUGIN_AUTOLOAD");
        }
        command.output().unwrap()
    };
    let checks = invoke("checks", false, None);
    assert_eq!(checks.status.code(), Some(0));
    assert_eq!(
        fs::read_to_string(root.join("trace")).unwrap(),
        "quoted \"tail\\|1\nfail|1\n"
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
    assert_eq!(fs::read_to_string(root.join("trace")).unwrap(), "tests|0\n");
    assert_eq!(
        String::from_utf8(tests.stdout).unwrap(),
        format!("[run] run tests: {} tests\n", adapter.display())
    );

    fs::remove_file(root.join("trace")).unwrap();
    let all = invoke("all", true, None);
    assert_eq!(all.status.code(), Some(17));
    assert_eq!(
        fs::read_to_string(root.join("trace")).unwrap(),
        "quoted \"tail\\|1\nfail|1\n"
    );
    let all_out = String::from_utf8(all.stdout).unwrap();
    assert!(all_out.contains("[error] build software browser assets failed with exit code 17\n"));
    assert!(!all_out.contains("[run] run tests:"));
    assert!(all.stderr.is_empty());

    fs::remove_file(root.join("trace")).unwrap();
    fs::write(
        &manifest,
        r#"{"command_sequences":{"release_check":[{"label":"run tests","command":["python","early"]},{"label":"run tests","command":["python","late"]}]}}"#,
    )
    .unwrap();
    let invalid = invoke("checks", false, None);
    assert_eq!(invalid.status.code(), Some(2));
    assert!(!root.join("trace").exists());
    assert!(
        String::from_utf8(invalid.stdout)
            .unwrap()
            .contains("[error] selected sequence must contain exactly one final run tests step\n")
    );
    fs::remove_dir_all(root).unwrap();
}
