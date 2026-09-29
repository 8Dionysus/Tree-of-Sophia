//! One disposable Git history exercises the actual selector and Python oracle.
#[cfg(target_os = "linux")]
#[test]
fn software_ci_actual_history_and_required_gate_match_maintained_python() {
    use std::fs;
    use std::process::{Command, Output};
    use std::time::{SystemTime, UNIX_EPOCH};
    let root = std::env::temp_dir().join(format!(
        "tos-software-ci-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(root.join("access/src/tos_access")).unwrap();
    fs::create_dir(root.join("scripts")).unwrap();
    fs::write(
        root.join("scripts/software_ci.py"),
        include_str!("../../../../scripts/software_ci.py"),
    )
    .unwrap();
    let git = |args: &[&str]| {
        let output = Command::new("git")
            .args(args)
            .current_dir(&root)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    };
    git(&["init"]);
    git(&["config", "user.name", "Software CI fixture"]);
    git(&["config", "user.email", "fixture@example.invalid"]);
    fs::write(root.join("README.md"), "[old missing](missing-old.md)\n").unwrap();
    fs::write(root.join("access/src/tos_access/reader.py"), "# source\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-m", "baseline"]);
    let base = git(&["rev-parse", "HEAD"]);
    fs::remove_file(root.join("access/src/tos_access/reader.py")).unwrap();
    fs::write(
        root.join("access/заметка😀.md"),
        "[reference]\n[reference]: ../target.md\n",
    )
    .unwrap();
    fs::write(root.join("target.md"), "exists\n").unwrap();
    fs::write(root.join("README.md"),"[old missing](missing-old.md)\n```md\n[literal](fake.md)\n```\n[new](target.md#part)\n[external](https://example.invalid/no-fetch)\n[root](/hosted-site)\n").unwrap();
    git(&["add", "-A"]);
    git(&["commit", "-m", "reader removal plus Unicode documentation"]);
    let executable = std::env::var_os("TOS_SOFTWARE_CI_TEST_EXECUTABLE")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_tos-software-ci").into());
    let invoke = |native: bool, mode: &str, needs: Option<&str>, full: bool| -> Output {
        let mut command = if native {
            Command::new(&executable)
        } else {
            Command::new("/usr/bin/python3")
        };
        if !native {
            // Invoke the maintained Python main explicitly. Its installed
            // __main__ entry is native after cutover, never the Python oracle.
            command.arg("-B").arg("-c")
                .arg("import pathlib, runpy, sys; path=sys.argv[1]; sys.argv=sys.argv[1:]; sys.path.insert(0,str(pathlib.Path(path).parent)); raise SystemExit(runpy.run_path(path,run_name='tos_maintained_python_oracle')['main']())")
                .arg(root.join("scripts/software_ci.py"));
        }
        command.arg(mode);
        if mode == "plan" {
            if native {
                command.arg("--repo-root").arg(&root);
            }
            command.arg("--base").arg(&base);
            if full {
                command.arg("--full");
            }
            command.env(
                "GITHUB_OUTPUT",
                root.join(if native {
                    "native-output"
                } else {
                    "python-output"
                }),
            );
        } else {
            command.env("CI_NEEDS", needs.unwrap());
            command.env_remove("GITHUB_OUTPUT");
        }
        command.current_dir(&root).output().unwrap()
    };
    for full in [false, true] {
        let native = invoke(true, "plan", None, full);
        let python = invoke(false, "plan", None, full);
        assert_eq!(
            native.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&native.stderr)
        );
        assert_eq!(
            python.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&python.stderr)
        );
        assert_eq!(native.stdout, python.stdout);
        assert!(native.stderr.is_empty());
        assert!(python.stderr.is_empty());
        assert_eq!(
            fs::read(root.join("native-output")).unwrap(),
            fs::read(root.join("python-output")).unwrap()
        );
        let selection: serde_json::Value = serde_json::from_slice(&native.stdout).unwrap();
        assert_eq!(
            selection["software_mode"],
            if full { "full" } else { "reader" }
        );
    }
    // Current worktree Markdown is checked, even when Git path selection stays
    // pinned to the same committed old/new history. Unchanged old gaps are not
    // treated as newly introduced links.
    fs::write(root.join("README.md"), "<<<<<<< branch\n[new](absent.md)\n").unwrap();
    let native = invoke(true, "plan", None, false);
    let python = invoke(false, "plan", None, false);
    assert_eq!(native.status.code(), Some(1));
    assert_eq!(python.status.code(), Some(1));
    for output in [native, python] {
        let diagnostic = String::from_utf8(output.stderr).unwrap();
        assert!(diagnostic.contains("README.md: unresolved merge marker"));
        assert!(diagnostic.contains("README.md: missing repository link target absent.md"));
    }
    let good = serde_json::json!({"plan":{"result":"success","outputs":{"software_mode":"reader","worker":"true","rust":"false"}},
        "software":{"result":"success"},"worker":{"result":"success"},"rust":{"result":"skipped"}});
    let text = serde_json::to_string(&good).unwrap();
    let native = invoke(true, "gate", Some(&text), false);
    let python = invoke(false, "gate", Some(&text), false);
    assert_eq!(native.status.code(), Some(0));
    assert_eq!(native.stdout, python.stdout);
    for job in ["plan", "software", "worker", "rust"] {
        for result in [Some("failure"), Some("cancelled"), None] {
            let mut bad = good.clone();
            if let Some(result) = result {
                bad[job]["result"] = result.into();
            } else {
                bad.as_object_mut().unwrap().remove(job);
            }
            let text = serde_json::to_string(&bad).unwrap();
            assert_eq!(
                invoke(true, "gate", Some(&text), false).status.code(),
                Some(1)
            );
            assert_eq!(
                invoke(false, "gate", Some(&text), false).status.code(),
                Some(1)
            );
        }
    }
    fs::remove_dir_all(root).unwrap();
}
