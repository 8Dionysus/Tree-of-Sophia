//! Exact immutable-baseline contracts through the maintained native CLI.
//! Git remains a platform operation; no previous ToS engine is invoked.
#![cfg(target_os = "linux")]
use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};
const BASELINE_ENV: &str = "TOS_SEMANTIC_REGISTRY_BASELINE_COMMIT";
const INTRO_ENV: &str = "TOS_SEMANTIC_REGISTRY_ALLOW_INITIAL_INTRODUCTION";
const READER: &str = "scripts/source_record_profiles.py";
const REFS: [&str; 4] = [
    "ToS/doctrine/semantic-interchange/entity-types.v1.json",
    "ToS/doctrine/semantic-interchange/relation-types.v1.json",
    "ToS/contracts/semantic-entity-type-registry.schema.json",
    "ToS/contracts/semantic-relation-type-registry.schema.json",
];
const BYTES: [&[u8]; 4] = [
    include_bytes!("../../../../ToS/doctrine/semantic-interchange/entity-types.v1.json"),
    include_bytes!("../../../../ToS/doctrine/semantic-interchange/relation-types.v1.json"),
    include_bytes!("../../../../ToS/contracts/semantic-entity-type-registry.schema.json"),
    include_bytes!("../../../../ToS/contracts/semantic-relation-type-registry.schema.json"),
];
struct Fixture {
    root: PathBuf,
    initial: String,
    baseline: String,
    registries: Vec<Value>,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "tos-registry-native-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let mut f = Self {
            root,
            initial: String::new(),
            baseline: String::new(),
            registries: BYTES[..2]
                .iter()
                .map(|b| serde_json::from_slice(b).unwrap())
                .collect(),
        };
        for (p, b) in REFS.iter().zip(BYTES) {
            f.write(p, b);
        }
        f.git(&["init", "--quiet"]);
        f.git(&[
            "commit",
            "--quiet",
            "--allow-empty",
            "-m",
            "Synthetic pre-registry history",
        ]);
        f.initial = f.git(&["rev-parse", "HEAD"]);
        f.git(&["add", "ToS"]);
        f.git(&[
            "commit",
            "--quiet",
            "-m",
            "Synthetic immutable registry baseline",
        ]);
        f.baseline = f.git(&["rev-parse", "HEAD"]);
        f
    }
    fn write(&self, p: &str, b: &[u8]) {
        let p = self.root.join(p);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, b).unwrap();
    }
    fn git(&self, args: &[&str]) -> String {
        let o = Command::new("/usr/bin/git")
            .env_clear()
            .env("PATH", "/usr/bin")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .arg("-C")
            .arg(&self.root)
            .args([
                "-c",
                "core.hooksPath=/dev/null",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "user.name=ToS synthetic fixture",
                "-c",
                "user.email=fixture@example.invalid",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        String::from_utf8(o.stdout).unwrap().trim().into()
    }
    fn save(&self, registries: &[Value]) {
        for (p, v) in REFS.iter().zip(registries) {
            self.write(p, &serde_json::to_vec(v).unwrap());
        }
    }
    fn native(&self, baseline: Option<&str>, intro: bool, env: &[(&str, &str)]) -> Output {
        let exe = std::env::var_os("TOS_SEMANTIC_REGISTRY_TEST_EXECUTABLE")
            .or_else(|| std::env::var_os("TOS_MECHANICS_TEST_EXECUTABLE"))
            .unwrap_or_else(|| env!("CARGO_BIN_EXE_tos-ops-mechanics-plan").into());
        let mut c = Command::new(exe);
        c.env_clear()
            .env("PATH", "/usr/bin")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .args(["--semantic-registry-transition", "--repo-root"])
            .arg(&self.root)
            .arg("--json");
        if let Some(b) = baseline {
            c.args(["--baseline-commit", b]);
        }
        if intro {
            c.arg("--allow-initial-introduction");
        }
        for (k, v) in env {
            c.env(k, v);
        }
        let o = c.output().unwrap();
        assert!(o.stdout.len() + o.stderr.len() <= 1024 * 1024);
        o
    }
    fn report(&self, baseline: &str, intro: bool) -> Value {
        report(self.native(Some(baseline), intro, &[]))
    }
    fn extension(&self, index: usize) -> Vec<Value> {
        let mut r = self.registries.clone();
        let p = profile(&mut r, index);
        let mut route = p["schemas"][0].clone();
        route["schema_version"] = json!(format!(
            "{}_synthetic_extension",
            route["schema_version"].as_str().unwrap()
        ));
        p["schemas"].as_array_mut().unwrap().push(route);
        bump(p, "profile_version", 1);
        bump(&mut r[index], "registry_version", 1);
        r
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn bump(v: &mut Value, k: &str, delta: i64) {
    v[k] = json!(v[k].as_i64().unwrap() + delta);
}
fn profile(r: &mut [Value], index: usize) -> &mut Value {
    let (entries, key) = if index == 0 {
        ("types", "source_record_profile")
    } else {
        ("relations", "source_claim_profile")
    };
    let entry = r[index][entries]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|e| {
            e.get(key).is_some() && (index == 0 || e[key]["reader"] == "semantic-relation-v1")
        })
        .unwrap();
    &mut entry[key]
}
fn report(o: Output) -> Value {
    assert!(
        !o.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(
        o.status.code(),
        Some(if v["valid"] == true { 0 } else { 1 })
    );
    v
}
fn refuses(o: Output, needle: &str) {
    assert_eq!(o.status.code(), Some(1));
    assert!(o.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&o.stderr).contains(needle),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
}
fn violation(v: &Value, needle: &str) {
    assert_eq!(v["valid"], false);
    assert!(
        v["violations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v.as_str().unwrap().contains(needle)),
        "{v}"
    );
}
#[test]
fn baselines_require_exact_available_commit_objects() {
    let f = Fixture::new();
    for b in [
        None,
        Some(""),
        Some("main"),
        Some("HEAD^"),
        Some(&f.baseline[..12]),
        Some("0000000000000000000000000000000000000000"),
        Some("ffffffffffffffffffffffffffffffffffffffff"),
    ] {
        let o = f.native(b, false, &[]);
        assert_eq!(o.status.code(), Some(1));
        assert!(o.stdout.is_empty());
    }
    let blob = f.git(&["rev-parse", &format!("{}:{}", f.baseline, REFS[0])]);
    refuses(f.native(Some(&blob), false, &[]), "not a commit object");
    let o = f.native(None, false, &[]);
    let text = String::from_utf8_lossy(&o.stderr);
    assert!(text.contains(BASELINE_ENV) && text.contains("--baseline-commit FULL_COMMIT_OID"));
}
#[test]
fn unchanged_and_entity_claim_extensions_preserve_the_report() {
    let f = Fixture::new();
    let v = f.report(&f.baseline, false);
    assert_eq!(v["valid"], true);
    assert_eq!(v["baseline_sha256"], v["current_sha256"]);
    assert_eq!(v["semantic_acceptance"], false);
    for i in 0..2 {
        f.save(&f.extension(i));
        let v = f.report(&f.baseline, false);
        assert_eq!(v["valid"], true, "{v}");
        assert_eq!(v["baseline_commit"], f.baseline);
        assert_ne!(v["baseline_sha256"], v["current_sha256"]);
    }
}
#[test]
fn profile_and_registry_versions_advance_separately() {
    let f = Fixture::new();
    for i in 0..2 {
        for missing in ["profile", "registry"] {
            let mut r = f.extension(i);
            if missing == "profile" {
                bump(profile(&mut r, i), "profile_version", -1);
            } else {
                bump(&mut r[i], "registry_version", -1);
            }
            f.save(&r);
            violation(
                &f.report(&f.baseline, false),
                &format!("must increase {missing}_version"),
            );
        }
    }
}
#[test]
fn version_bumps_never_authorize_historical_schema_removal_or_repurpose() {
    let f = Fixture::new();
    for i in 0..2 {
        for remove in [true, false] {
            let mut r = f.extension(i);
            let routes = profile(&mut r, i)["schemas"].as_array_mut().unwrap();
            if remove {
                routes.remove(0);
            } else {
                routes[0]["schema_ref"] = json!("ToS/contracts/synthetic-repurposed.schema.json");
            }
            f.save(&r);
            violation(
                &f.report(&f.baseline, false),
                "removed or repurposed a historical schema route",
            );
        }
    }
}
#[test]
fn identity_and_reader_repurpose_require_successor_identity() {
    let f = Fixture::new();
    for (i, k, value) in [
        (0, "id_prefix", "tos.synthetic-repurposed."),
        (0, "record_type", "synthetic-repurposed"),
        (1, "reader", "historical-temporal-v1"),
    ] {
        let mut r = f.extension(i);
        profile(&mut r, i)[k] = json!(value);
        f.save(&r);
        violation(&f.report(&f.baseline, false), "explicit successor identity");
    }
}
#[test]
fn newer_head_and_git_replace_never_substitute_the_baseline() {
    let f = Fixture::new();
    let mut r = f.extension(0);
    profile(&mut r, 0)["schemas"]
        .as_array_mut()
        .unwrap()
        .remove(0);
    f.save(&r);
    f.git(&["add", "ToS"]);
    f.git(&["commit", "--quiet", "-m", "Incompatible current registry"]);
    let current = f.git(&["rev-parse", "HEAD"]);
    f.git(&["replace", &f.baseline, &current]);
    let v = f.report(&f.baseline, false);
    violation(&v, "historical schema route");
    assert_eq!(v["baseline_commit"], f.baseline);
}
#[test]
fn partial_baseline_cannot_be_treated_as_initial_introduction() {
    let f = Fixture::new();
    f.git(&["rm", "--quiet", REFS[1]]);
    f.git(&["commit", "--quiet", "-m", "Incomplete baseline"]);
    let missing = f.git(&["rev-parse", "HEAD"]);
    f.save(&f.registries);
    refuses(f.native(Some(&missing), true, &[]), "partial baseline");
}
#[test]
fn introduction_requires_explicit_authority_and_reports_no_prior_comparison() {
    let f = Fixture::new();
    refuses(
        f.native(Some(&f.initial), false, &[]),
        "initial introduction requires explicit",
    );
    let v = f.report(&f.initial, true);
    assert_eq!(v["valid"], true);
    assert_eq!(v["transition_kind"], "initial-introduction");
    assert_eq!(v["compared_previous_registry"], false);
    assert_eq!(v["initial_introduction_explicitly_allowed"], true);
    assert_eq!(v["semantic_acceptance"], false);
    assert_eq!(v["baseline_sha256"], json!({}));
    let actual: std::collections::BTreeSet<_> = v["baseline_absent_refs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    let expected: std::collections::BTreeSet<_> = REFS.into_iter().chain([READER]).collect();
    assert_eq!(actual, expected);
    let mut r = f.extension(0);
    bump(profile(&mut r, 0), "profile_version", -1);
    f.save(&r);
    assert_eq!(f.report(&f.baseline, true)["valid"], false);
    assert_eq!(
        report(f.native(None, false, &[(BASELINE_ENV, &f.initial), (INTRO_ENV, "1")]))["transition_kind"],
        "initial-introduction"
    );
}
#[test]
fn shallow_ancestry_refuses_introduction() {
    let f = Fixture::new();
    f.write(".git/shallow", format!("{}\n", f.initial).as_bytes());
    refuses(f.native(Some(&f.initial), true, &[]), "shallow history");
}
#[test]
fn grafts_cannot_hide_deleted_registry_history() {
    let f = Fixture::new();
    f.git(&["rm", "--quiet", "-r", "ToS"]);
    f.git(&["commit", "--quiet", "-m", "Erased registry baseline"]);
    let deleted = f.git(&["rev-parse", "HEAD"]);
    f.git(&["restore", &format!("--source={}", f.baseline), "--", "ToS"]);
    f.write(".git/info/grafts", format!("{deleted}\n").as_bytes());
    refuses(
        f.native(Some(&deleted), true, &[]),
        "Git grafts are not allowed",
    );
    assert_eq!(f.report(&f.baseline, false)["valid"], true);
    fs::remove_file(f.root.join(".git/info/grafts")).unwrap();
    f.write("synthetic-grafts", format!("{deleted}\n").as_bytes());
    refuses(
        f.native(
            Some(&deleted),
            true,
            &[(
                "GIT_GRAFT_FILE",
                f.root.join("synthetic-grafts").to_str().unwrap(),
            )],
        ),
        "Git grafts are not allowed",
    );
}
#[test]
fn deleted_registries_and_historical_declared_readers_are_not_first_introduction() {
    let f = Fixture::new();
    f.git(&["rm", "--quiet", "-r", "ToS"]);
    f.git(&["commit", "--quiet", "-m", "Deleted prior registries"]);
    let deleted = f.git(&["rev-parse", "HEAD"]);
    refuses(
        f.native(Some(&deleted), true, &[]),
        "history already contains",
    );
    f.git(&["switch", "--quiet", "--detach", &f.initial]);
    f.write(
        READER,
        b"# Historical reader identity sentinel; never executed.\n",
    );
    f.git(&["add", READER]);
    f.git(&["commit", "--quiet", "-m", "Pre-registry declared reader"]);
    let reader = f.git(&["rev-parse", "HEAD"]);
    refuses(
        f.native(Some(&reader), true, &[]),
        "history already contains",
    );
}
#[test]
fn schema_and_duplicate_json_refuse_before_comparison() {
    let f = Fixture::new();
    let mut r = f.registries.clone();
    profile(&mut r, 0)["profile_version"] = json!(false);
    f.save(&r);
    refuses(f.native(Some(&f.baseline), false, &[]), "profile_version");
    f.write(REFS[0], br#"{"types": [], "types": []}"#);
    let o = f.native(Some(&f.baseline), false, &[]);
    assert_eq!(o.status.code(), Some(1));
    let text = String::from_utf8_lossy(&o.stderr);
    assert!(
        text.contains("duplicate JSON key") || text.contains("duplicate decoded JSON member"),
        "{text}"
    );
}
#[test]
fn explicit_environment_baseline_drives_the_registered_native_gate() {
    let f = Fixture::new();
    assert_eq!(
        report(f.native(None, false, &[(BASELINE_ENV, &f.baseline)]))["baseline_commit"],
        f.baseline
    );
    let v: Value = serde_json::from_slice(include_bytes!(
        "../../../../docs/validation/validation_lanes.json"
    ))
    .unwrap();
    let gate = &v["command_sequences"]["semantic_registry_transition"];
    assert_eq!(gate.as_array().unwrap().len(), 1);
    let lane = &v["lanes"]["semantic_registry_transition"];
    assert_eq!(lane["layer"], "source-contract");
    assert_eq!(lane["mode"], "blocking");
    assert!(
        lane["required_environment"]
            .as_array()
            .unwrap()
            .contains(&json!(BASELINE_ENV))
    );
    assert_eq!(
        gate[0]["command"],
        json!([
            "tos-ops-mechanics-plan",
            "--repo-root",
            ".",
            "--semantic-registry-transition"
        ])
    );
    assert!(
        !v["command_sequences"]["release_check"]
            .as_array()
            .unwrap()
            .contains(&gate[0])
    );
    assert!(
        !v["lanes"]["release"]["covers_lanes"]
            .as_array()
            .unwrap()
            .contains(&json!("semantic_registry_transition"))
    );
}
