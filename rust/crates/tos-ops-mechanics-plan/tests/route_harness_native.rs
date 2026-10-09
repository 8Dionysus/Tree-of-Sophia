//! Native declared-task harness contracts and explicit source provenance.
#[cfg(target_os = "linux")]
#[test]
fn route_harness_declared_task_consumer_preserves_provenance_and_refusals() {
    use serde_json::{Value, json};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};
    use std::time::{SystemTime, UNIX_EPOCH};
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let fixture = Fixture(std::env::temp_dir().join(format!("tos-route-harness-{}-{}",
        std::process::id(), SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos())));
    let root = fixture.0.join("repo");
    let write = |rel: &str, text: &str| {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    };
    write(
        "AGENTS.md",
        "source review generated\r\nΟΣ ΟΣΑ İ\u{1c}bounded\r",
    );
    write("branch/AGENTS.md", "task-law validation source owner\n");
    write("owner/AGENTS.md", "handoff-law canon review source\n");
    write("branch/target.md", "source\u{a0}witness\n");
    write("scripts/check.py", "# declared path only\n");
    write(
        "docs/validation/validation_lanes.json",
        "{\"lanes\":{\"route_docs\":[]}}\n",
    );
    let task = |id: &str, owner: &str, marker: &str| {
        json!({"id":id,"prompt":"Route источник; don't accept a projection.",
        "target":"branch/target.md","owner_route":owner,"required_markers":[marker,"ος","οσα","i\u{0307}"],
        "boundary_markers":["source","review"],"on_demand_surfaces":["branch/target.md","branch/target.md"],
        "validation_paths":["scripts/check.py"],"validation_lanes":["route_docs"],
        "completion_evidence":["branch/target.md","scripts/check.py"],"handoff_routes":["external-owner"]})
    };
    let mut inventory = json!({"route_card_discovery":{"root_cards":["AGENTS.md"],"route_roots":["branch","owner"]},
        "context_budget":{"inherited_stack_max_tokens":1000,"task_overrides":{"handoff":999},
        "additional_context_max_tokens":1000,"owner_handoff_max_tokens":1000,"owner_hops_max":8,
        "missing_task_law_max":0,"boundary_deviation_max":0,"completion_coverage_min":1.0},
        "task_routes":[task("inherited","branch/AGENTS.md","task-law"),task("handoff","owner/AGENTS.md","handoff-law")]});
    let save = |inventory: &Value| {
        write(
            "docs/validation/agents_route_inventory.json",
            &serde_json::to_string_pretty(inventory).unwrap(),
        )
    };
    // Retain compatibility with historical float exponent spelling.
    inventory["context_budget"]["additional_context_max_tokens"] =
        serde_json::from_str("1e3").unwrap();
    save(&inventory);
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
    };
    git(&["init", "--quiet"]);
    git(&["config", "user.name", "Route Fixture"]);
    git(&["config", "user.email", "route@example.invalid"]);
    git(&["add", "."]);
    git(&["commit", "--quiet", "-m", "fixture"]);
    let executable = std::env::var_os("TOS_ROUTE_HARNESS_TEST_EXECUTABLE")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_tos-agents-route-harness").into());
    let run = |args: &[&str]| -> Output {
        Command::new(&executable).env_clear().env("PATH", "/usr/bin")
            .arg("--repo-root").arg(&root).args(args).current_dir(&root).output().unwrap()
    };
    let compare = |args: &[&str], expected: i32| -> Output {
        let native = run(args);
        assert_eq!(native.status.code(), Some(expected), "{}", String::from_utf8_lossy(&native.stderr));
        assert!(native.stderr.is_empty(), "{}", String::from_utf8_lossy(&native.stderr));
        native
    };
    let canonical = compare(&[], 0);
    assert_eq!(canonical.stdout, compare(&[], 0).stdout);
    let result: Value = serde_json::from_slice(&canonical.stdout).unwrap();
    assert_eq!(result["route_success_count"], 2);
    assert_ne!(result["source_ref"], "working-tree");
    assert_eq!(
        result["tasks"][1]["inheritance_stack"],
        json!(["AGENTS.md", "branch/AGENTS.md"])
    );
    assert_eq!(
        result["tasks"][1]["owner_handoff"]["in_inheritance_stack"],
        false
    );
    assert!(
        result["tasks"][1]["owner_handoff_context_tokens"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert_eq!(result["behavioral_model_runs"], 0);
    assert!(result["behavioral_claim"].is_null());
    compare(&["--check"], 0);
    let labelled: Value =
        serde_json::from_slice(&compare(&["--source-ref", "immutable-ref"], 0).stdout).unwrap();
    assert_eq!(labelled["source_ref"], "immutable-ref");
    let output = fixture.0.join("nested/result.json");
    let output_arg = output.to_str().unwrap();
    let native = run(&["--check", "--output", output_arg]);
    assert_eq!(
        native.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&native.stderr)
    );
    let native_bytes = fs::read(&output).unwrap();
    assert_eq!(native_bytes, canonical.stdout);
    let mut timing_native: Value =
        serde_json::from_slice(&run(&["--volatile-timing"]).stdout).unwrap();
    for task in timing_native["tasks"].as_array_mut().unwrap() {
        assert!(task["time_to_owner_ms"].as_f64().unwrap() >= 0.0);
        assert_eq!(task["route_resolution_measurement"], "Wall-clock duration of this harness lookup, measured for the current run.");
    }
    write("dirty.txt", "dirty\n");
    let dirty: Value =
        serde_json::from_slice(&compare(&["--source-ref", "immutable-ref"], 0).stdout).unwrap();
    assert_eq!(dirty["source_ref"], "working-tree");
    fs::remove_file(root.join("dirty.txt")).unwrap();
    inventory["task_routes"][0]["required_markers"]
        .as_array_mut()
        .unwrap()
        .push("absent-law".into());
    inventory["task_routes"][0]["boundary_markers"]
        .as_array_mut()
        .unwrap()
        .push("absent-boundary".into());
    inventory["task_routes"][0]["validation_lanes"]
        .as_array_mut()
        .unwrap()
        .push("unknown-lane".into());
    inventory["task_routes"][0]["completion_evidence"]
        .as_array_mut()
        .unwrap()
        .push("missing.md".into());
    inventory["context_budget"]["inherited_stack_max_tokens"] = 1.into();
    inventory["context_budget"]["owner_handoff_max_tokens"] = serde_json::from_str("1e-3").unwrap();
    save(&inventory);
    fs::remove_file(root.join("scripts/check.py")).unwrap();
    let failed: Value = serde_json::from_slice(&compare(&[], 0).stdout).unwrap();
    assert_eq!(failed["route_success_count"], 0);
    assert_eq!(failed["tasks"][0]["missing_task_specific_law"]["count"], 1);
    assert!(failed["tasks"][0]["budget"]["violations"].as_array().unwrap().iter().any(|v| v.as_str().unwrap().contains("inherited_context_tokens>1")));
    assert!(failed["tasks"][1]["budget"]["violations"].as_array().unwrap().iter().any(|v| v.as_str().unwrap().contains("owner_handoff_context_tokens>")));
    assert_eq!(
        failed["tasks"][0]["selected_validation"]["unknown_lanes"],
        json!(["unknown-lane"])
    );
    assert!(
        failed["tasks"][0]["budget"]["violations"]
            .as_array()
            .unwrap()
            .contains(&"completion_coverage".into())
    );
    compare(&["--check"], 1);
    // Same existing CLI output/check ordering: failed check still writes result.
    let native = run(&["--check", "--output", output_arg]);
    let native_bytes = fs::read(&output).unwrap();
    assert_eq!(native.status.code(), Some(1));
    assert_eq!(serde_json::from_slice::<Value>(&native_bytes).unwrap(), failed);
    // Bounded native safety envelope: missing target remains a task failure,
    // while path escape, symlink and FIFO inputs/outputs fail closed.
    inventory["task_routes"][0]["target"] = "missing-target.md".into();
    save(&inventory);
    compare(&["--check"], 1);
    inventory["task_routes"][0]["target"] = "../escape.md".into();
    save(&inventory);
    assert_eq!(run(&[]).status.code(), Some(1));
    inventory["task_routes"][0]["target"] = "branch/target.md".into();
    save(&inventory);
    fs::remove_file(root.join("branch/target.md")).unwrap();
    std::os::unix::fs::symlink(root.join("AGENTS.md"), root.join("branch/target.md")).unwrap();
    assert_eq!(run(&[]).status.code(), Some(1));
    fs::remove_file(root.join("branch/target.md")).unwrap();
    write("branch/target.md", "source\n");
    fs::remove_file(&output).unwrap();
    std::os::unix::fs::symlink(root.join("AGENTS.md"), &output).unwrap();
    let before = fs::read(root.join("AGENTS.md")).unwrap();
    assert_eq!(run(&["--output", output_arg]).status.code(), Some(1));
    assert_eq!(before, fs::read(root.join("AGENTS.md")).unwrap());
    fs::remove_file(&output).unwrap();
    use std::os::unix::ffi::OsStrExt;
    let fifo = std::ffi::CString::new(output.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
    assert_eq!(run(&["--output", output_arg]).status.code(), Some(1));
    fs::remove_file(&output).unwrap();
    // An output directory symlink is rejected before writing through it.
    let link = fixture.0.join("linked");
    std::os::unix::fs::symlink(root.join("branch"), &link).unwrap();
    assert_eq!(
        run(
            &["--output", link.join("escaped.json").to_str().unwrap()]
        )
        .status
        .code(),
        Some(1)
    );
    assert!(!Path::new(&root.join("branch/escaped.json")).exists());
    // Empty present cards still participate in the documented newline join.
    write("AGENTS.md", "");
    inventory["task_routes"][0]["required_markers"] = json!(["\ntask-law"]);
    save(&inventory);
    let empty_card: Value = serde_json::from_slice(&compare(&[], 0).stdout).unwrap();
    assert_eq!(
        empty_card["tasks"][0]["missing_task_specific_law"]["count"],
        0
    );
    inventory["task_routes"] = json!([]);
    save(&inventory);
    let empty: Value = serde_json::from_slice(&compare(&[], 0).stdout).unwrap();
    assert_eq!(empty["route_success_rate"], 0.0);
    compare(&["--check"], 0);
}
