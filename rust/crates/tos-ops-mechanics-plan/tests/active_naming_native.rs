//! Actual default validator vs its maintained Python on one disposable tree.
#[cfg(target_os = "linux")]
#[test]
fn active_naming_default_consumer_matches_maintained_python_and_retains_moved_history() {
    use std::fs;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};
    let root = std::env::temp_dir().join(format!(
        "tos-active-naming-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let write = |rel: &str, text: &str| {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    };
    write(
        "scripts/validate_active_naming.py",
        include_str!("../../../../scripts/validate_active_naming.py"),
    );
    write("legacy/wave/old.md", "seed-pack\n");
    write("kag/indexes/segments/source/00.jsonl", "wave-pack\n");
    write("access/web/package-lock.json", "seed-pack\n");
    write(
        "docs/allowed.md",
        "first-wave seed_claim_ref may_seed_gold seed.node_ids one-seeder\n",
    );
    write(
        "docs/quotes.md",
        "ToS Deep Research_ A48 — Океания _ khipu _ rongorongo as frontier seed.docx\nBentham включён как заданный master-seed и как пороговая фигура: его ранние тексты до 1820 года учитываются только как генеалогический вход, тогда как ядро документа остаётся в пределах 1820–1900.\n",
    );
    write("docs/bad.md", "ordinary seed.\r\nſEED-pack\r\n");
    write("ToS/first-wave/item.json", "{}\n");
    write("mechanics/experience/v0.7/README.md", "clean\n");
    let topology = serde_json::json!({"schema_version":"tos_mechanics_topology_v2","owner_repo":"Tree-of-Sophia","root":"mechanics/",
        "legacy_policy":"package-local-only-when-active-route-has-moved-path-or-raw-receipt-accounting",
        "packages":[{"slug":"experience","class":"head-fed/local","status":"active","active_parts":["write-guards"],"legacy_required":true}],
        "moved_path_accounting":{"experience":{"write-guards":["mechanics/experience/v0.7-wave-old"]}},
        "moved_path_targets":{"mechanics/experience/v0.7-wave-old":"mechanics/experience/parts/write-guards"}});
    write(
        "mechanics/topology.json",
        &serde_json::to_string(&topology).unwrap(),
    );
    let executable = std::env::var_os("TOS_ACTIVE_NAMING_TEST_EXECUTABLE")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_tos-ops-mechanics-plan").into());
    let compare = |expected: i32| {
        let native = Command::new(&executable)
            .arg("--repo-root")
            .arg(&root)
            .arg("--active-naming-validate")
            .output()
            .unwrap();
        let python = Command::new("/usr/bin/python3")
            .arg("-B")
            .arg("-c")
            .arg("import pathlib, runpy, sys; path=sys.argv[1]; sys.argv=sys.argv[1:]; sys.path.insert(0,str(pathlib.Path(path).parent)); raise SystemExit(runpy.run_path(path,run_name='tos_maintained_python_oracle')['main']())")
            .arg(root.join("scripts/validate_active_naming.py"))
            .output()
            .unwrap();
        assert_eq!(
            native.status.code(),
            Some(expected),
            "{}",
            String::from_utf8_lossy(&native.stderr)
        );
        assert_eq!(
            python.status.code(),
            Some(expected),
            "{}",
            String::from_utf8_lossy(&python.stderr)
        );
        assert_eq!(native.stdout, python.stdout);
        assert_eq!(native.stderr, python.stderr);
        String::from_utf8(native.stderr).unwrap()
    };
    let findings = compare(1);
    assert!(findings.contains("ToS/first-wave: retired active name in path: wave"));
    assert!(
        findings.contains("docs/bad.md: retired active path/id reference in content: ſEED-pack")
    );
    assert!(
        findings
            .contains("mechanics/experience/v0.7: retired experience pass marker in path: v0.7")
    );
    assert!(
        !findings.contains("legacy/")
            && !findings.contains("kag/")
            && !findings.contains("mechanics/topology.json")
    );
    fs::remove_dir_all(root.join("ToS/first-wave")).unwrap();
    fs::remove_dir_all(root.join("mechanics/experience/v0.7")).unwrap();
    fs::remove_file(root.join("docs/bad.md")).unwrap();
    compare(0);
    // Historical moved keys remain excluded while a real current target still
    // follows the active law. This is source representation, not naming/canon
    // authority over the historical paths being described.
    let mut changed = topology.clone();
    changed["moved_path_targets"]["mechanics/experience/v0.7-wave-old"] =
        "mechanics/experience/parts/wave-pack".into();
    write(
        "mechanics/topology.json",
        &serde_json::to_string(&changed).unwrap(),
    );
    let findings = compare(1);
    assert!(findings.contains("mechanics/topology.json: retired active path/id reference in content: mechanics/experience/parts/wave-pack"));
    assert!(!root.join("legacy/wave/old.md").is_symlink());
    fs::remove_dir_all(root).unwrap();
}
