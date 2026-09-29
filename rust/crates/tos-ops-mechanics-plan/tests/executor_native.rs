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

// Existing controlled consumer now enters through installed Python wrappers.
// Exact adapters guard exec replacement/argv/environment; the original tiny
// validation sequence retains command-authority order and first-failure stop.
#[cfg(target_os = "linux")]
#[test]
fn installed_entrypoints_preserve_argv_environment_and_validation_first_failure() {
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
    fs::create_dir_all(root.join("scripts")).unwrap();
    for (name, source) in [
        (
            "validation_lanes.py",
            include_str!("../../../../scripts/validation_lanes.py"),
        ),
        (
            "release_check.py",
            include_str!("../../../../scripts/release_check.py"),
        ),
        (
            "software_ci.py",
            include_str!("../../../../scripts/software_ci.py"),
        ),
        (
            "validate_mechanics_topology.py",
            include_str!("../../../../scripts/validate_mechanics_topology.py"),
        ),
        (
            "validate_active_naming.py",
            include_str!("../../../../scripts/validate_active_naming.py"),
        ),
    ] {
        fs::write(root.join("scripts").join(name), source).unwrap();
    }
    fs::create_dir(root.join("bin")).unwrap();
    let selected = root.join("bin/selected native");
    fs::write(&selected, "#!/usr/bin/python3\nimport json,os,sys\nprint(json.dumps({'argv':sys.argv,'pid':os.getpid(),'sentinel':os.environ['WRAPPER_SENTINEL'],'pytest':os.environ['PYTEST_DISABLE_PLUGIN_AUTOLOAD'],'needs':os.environ['CI_NEEDS'],'github':os.environ['GITHUB_OUTPUT']},ensure_ascii=False))\nraise SystemExit(17)\n").unwrap();
    fs::set_permissions(&selected, fs::Permissions::from_mode(0o700)).unwrap();
    let inspect =
        |script: &str, key: &str, args: &[&str], expected: Vec<String>, path_lookup: bool| {
            let mut command = Command::new("/usr/bin/python3");
            command
                .arg("-B")
                .arg(root.join("scripts").join(script))
                .args(args)
                .env(
                    key,
                    if path_lookup {
                        ""
                    } else {
                        selected.to_str().unwrap()
                    },
                )
                .env("PATH", root.join("bin"))
                .env("WRAPPER_SENTINEL", "source literal $value, пробел")
                .env("PYTEST_DISABLE_PLUGIN_AUTOLOAD", "0")
                .env("CI_NEEDS", "{\"fixture\":true}")
                .env("GITHUB_OUTPUT", root.join("caller-output"))
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped());
            let child = command.spawn().unwrap();
            let pid = child.id();
            let output = child.wait_with_output().unwrap();
            assert_eq!(
                output.status.code(),
                Some(17),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.stderr.is_empty());
            let trace: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(
                trace["pid"], pid,
                "wrapper must exec rather than spawn a second owner"
            );
            assert_eq!(trace["argv"], serde_json::json!(expected));
            assert_eq!(trace["sentinel"], "source literal $value, пробел");
            assert_eq!(trace["pytest"], "0");
            assert_eq!(trace["needs"], "{\"fixture\":true}");
            assert_eq!(
                trace["github"],
                root.join("caller-output").to_str().unwrap()
            );
            assert!(!root.join("caller-output").exists());
        };
    let base = |tail: &[&str]| {
        let mut argv = vec![
            selected.to_str().unwrap().to_owned(),
            "--repo-root".into(),
            root.to_str().unwrap().into(),
        ];
        argv.extend(tail.iter().map(|v| (*v).to_owned()));
        argv
    };
    inspect(
        "validation_lanes.py",
        "TOS_VALIDATION_LANES_EXECUTOR",
        &["--che", "--sequence=quoted sequence", "--ru", "sample"],
        base(&[
            "--python",
            "/usr/bin/python3",
            "--check",
            "--sequence",
            "quoted sequence",
            "--run",
            "sample",
        ]),
        false,
    );
    inspect(
        "release_check.py",
        "TOS_RELEASE_CHECK_EXECUTOR",
        &["--ph=checks"],
        base(&["--python", "/usr/bin/python3", "--phase", "checks"]),
        false,
    );
    inspect(
        "software_ci.py",
        "TOS_SOFTWARE_CI_EXECUTOR",
        &["plan", "--ba=quoted ref $value", "--fu"],
        vec![
            selected.to_str().unwrap().into(),
            "plan".into(),
            "--repo-root".into(),
            root.to_str().unwrap().into(),
            "--base".into(),
            "quoted ref $value".into(),
            "--full".into(),
        ],
        false,
    );
    inspect(
        "validate_mechanics_topology.py",
        "TOS_OPS_MECHANICS_EXECUTOR",
        &["legacy ignored argument"],
        base(&["--mechanics-topology-validate"]),
        false,
    );
    inspect(
        "validate_active_naming.py",
        "TOS_OPS_MECHANICS_EXECUTOR",
        &[],
        base(&["--active-naming-validate"]),
        false,
    );
    let installed = root.join("bin/tos-software-ci");
    fs::copy(&selected, &installed).unwrap();
    inspect(
        "software_ci.py",
        "TOS_SOFTWARE_CI_EXECUTOR",
        &["gate"],
        vec![installed.to_str().unwrap().into(), "gate".into()],
        true,
    );
    let unavailable = Command::new("/usr/bin/python3")
        .arg("-B")
        .arg(root.join("scripts/software_ci.py"))
        .arg("gate")
        .env("TOS_SOFTWARE_CI_EXECUTOR", root.join("missing-native"))
        .env("PATH", root.join("bin"))
        .output()
        .unwrap();
    assert_eq!(unavailable.status.code(), Some(1));
    assert!(unavailable.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&unavailable.stderr).contains("cannot execute native software CI")
    );
    let invalid = Command::new("/usr/bin/python3")
        .arg("-B")
        .arg(root.join("scripts/validation_lanes.py"))
        .arg("--unknown")
        .env("TOS_VALIDATION_LANES_EXECUTOR", &selected)
        .output()
        .unwrap();
    assert_eq!(invalid.status.code(), Some(2));
    assert!(invalid.stdout.is_empty());
    let help = Command::new("/usr/bin/python3")
        .arg("-B")
        .arg(root.join("scripts/validate_active_naming.py"))
        .arg("--help")
        .env("TOS_OPS_MECHANICS_EXECUTOR", root.join("missing-native"))
        .output()
        .unwrap();
    assert_eq!(help.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&help.stdout).contains("--feedback-cache"));
    // Explicit cache branch remains Python-owned, independently of whether
    // native is installed. No real cache is written in this routing fixture.
    let compatibility = Command::new("/usr/bin/python3").arg("-B").arg("-c")
        .arg("import pathlib,runpy,sys; root=pathlib.Path(sys.argv[1]); sys.path.insert(0,str(root/'scripts')); modules={n:runpy.run_path(str(root/'scripts'/n),run_name='tos_import_api') for n in ['validation_lanes.py','release_check.py','software_ci.py','validate_mechanics_topology.py','validate_active_naming.py']}; assert modules['software_ci.py']['select'](['README.md'])['software_mode']=='none'; assert callable(modules['validation_lanes.py']['command_sequence']); assert callable(modules['release_check.py']['select_steps']); assert callable(modules['validate_mechanics_topology.py']['run_validation']); naming=modules['validate_active_naming.py']; seen=[]; naming['native_main'].__globals__['main']=lambda args:seen.append(args) or 23; assert naming['native_main'](['--feedback-cache=external.sqlite'])==23; assert seen==[['--feedback-cache=external.sqlite']]")
        .arg(&root).env("TOS_OPS_MECHANICS_EXECUTOR",root.join("missing-native")).output().unwrap();
    assert!(
        compatibility.status.success(),
        "{}",
        String::from_utf8_lossy(&compatibility.stderr)
    );
    assert!(!root.join("external.sqlite").exists());
    fs::write(
        root.join("docs/validation/validation_lanes.json"),
        r#"{"command_sequences":{"sample":[{"label":"first","command":["python","-B","adapter.py","first"]},{"label":"failing","command":["python","-B","adapter.py","fail"]},{"label":"later","command":["python","-B","adapter.py","later"]}]}}"#,
    )
    .unwrap();
    let adapter = root.join("adapter.py");
    fs::write(
        &adapter,
        "import pathlib,sys\nwith pathlib.Path('trace').open('a') as trace: trace.write(sys.argv[1]+'\\n')\nraise SystemExit(17 if sys.argv[1]=='fail' else 0)\n",
    )
    .unwrap();
    let executable = std::env::var_os("TOS_VALIDATION_LANES_TEST_EXECUTABLE")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_tos-validation-lanes").into());
    let output = Command::new("/usr/bin/python3")
        .arg("-B")
        .arg(root.join("scripts/validation_lanes.py"))
        .env("TOS_VALIDATION_LANES_EXECUTOR", executable)
        .args(["--sequence", "sample", "--run", "sample"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(17));
    assert_eq!(
        fs::read_to_string(root.join("trace")).unwrap(),
        "first\nfail\n"
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("first: /usr/bin/python3 -B adapter.py first\n"));
    assert!(stdout.contains("[ok] first\n"));
    assert!(stdout.contains("[run] failing: /usr/bin/python3 -B adapter.py fail\n"));
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
