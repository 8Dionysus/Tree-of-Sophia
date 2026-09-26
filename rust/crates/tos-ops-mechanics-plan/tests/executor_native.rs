//! One synthetic package fixture exercises ordered execution and durable
//! lifecycle risks through the real CLI, without executing repository tools.
#[cfg(target_os = "linux")]
#[test]
fn ordered_runner_stops_and_owns_ordinary_and_escaped_children() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
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
        "success", "failure", "timeout", "output", "cancel", "blocked", "daemon",
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
