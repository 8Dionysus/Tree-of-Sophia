//! Maintained native installation and real-host WEB verification.
//! Build/install selection belongs to the caller; verification never admits data.
use crate::executor::{self, Limits};
use serde_json::{Value, json};
use std::{
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicI32, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tos_foundation::Digest256Hasher;

fn bad(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}
fn path(value: &Path) -> io::Result<String> {
    value
        .to_str()
        .map(str::to_owned)
        .ok_or_else(|| bad("verification path must be UTF-8"))
}
fn exact(value: &Path) -> io::Result<()> {
    if !value.is_absolute() || fs::canonicalize(value)? != value {
        return Err(bad(
            "verification path must be canonical absolute without symlinks",
        ));
    }
    Ok(())
}
struct Temporary(PathBuf);
impl Temporary {
    fn new() -> io::Result<Self> {
        use std::os::unix::fs::DirBuilderExt;
        let parent = std::env::temp_dir().canonicalize()?;
        for attempt in 0..16 {
            let tick = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| bad("clock before epoch"))?
                .as_nanos();
            let value = parent.join(format!(
                "tos-native-verify-{}-{tick}-{attempt}",
                std::process::id()
            ));
            match fs::DirBuilder::new().mode(0o700).create(&value) {
                Ok(()) => return Ok(Self(value)),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
        Err(bad("cannot allocate isolated verification directory"))
    }
}
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
struct Run<'a> {
    deadline: Instant,
    command_wall: Duration,
    max_output: usize,
    cancel: &'a AtomicI32,
}
impl Run<'_> {
    fn check(&self) -> io::Result<()> {
        if self.cancel.load(Ordering::Relaxed) != 0 || Instant::now() >= self.deadline {
            return Err(bad("native verification cancelled or expired"));
        }
        Ok(())
    }
    fn capture(
        &self,
        root: &Path,
        argv: Vec<String>,
        env: &[(&str, String)],
    ) -> io::Result<(i32, Vec<u8>, Vec<u8>)> {
        self.check()?;
        let argv = if env.is_empty() {
            argv
        } else {
            let mut command = vec!["/usr/bin/env".into()];
            for (key, value) in env {
                command.push(format!("{key}={value}"));
            }
            command.extend(argv);
            command
        };
        let remaining = self.deadline.saturating_duration_since(Instant::now());
        let result = executor::capture_ci_git(
            root,
            argv,
            Limits {
                command_wall: remaining.min(self.command_wall),
                lane_wall: remaining,
                cleanup_grace: Duration::from_secs(1),
                output_bytes: self.max_output,
            },
            self.cancel,
        )?;
        self.check()?;
        Ok(result)
    }
    fn run(&self, root: &Path, argv: Vec<String>, env: &[(&str, String)]) -> io::Result<Vec<u8>> {
        let (code, out, err) = self.capture(root, argv, env)?;
        if code != 0 {
            io::stdout().write_all(&out)?;
            io::stderr().write_all(&err)?;
            return Err(bad(format!(
                "native verification child failed with exit code {code}"
            )));
        }
        Ok(out)
    }
    fn bytes(&self, p: &Path, cap: u64) -> io::Result<Vec<u8>> {
        use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
        self.check()?;
        exact(p)?;
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(p)?;
        let a = file.metadata()?;
        if !a.is_file() || a.len() > cap {
            return Err(bad("verification input byte limit or file type"));
        }
        let mut bytes = Vec::new();
        Read::by_ref(&mut file)
            .take(cap + 1)
            .read_to_end(&mut bytes)?;
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
        if bytes.len() as u64 != a.len()
            || stamp(&a) != stamp(&file.metadata()?)
            || stamp(&a) != stamp(&fs::symlink_metadata(p)?)
        {
            return Err(bad("verification input changed"));
        }
        self.check()?;
        Ok(bytes)
    }
    fn digest(&self, p: &Path) -> io::Result<String> {
        use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
        self.check()?;
        exact(p)?;
        let mut f = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(p)?;
        let a = f.metadata()?;
        if !a.is_file() || a.len() > 1024 * 1024 * 1024 {
            return Err(bad("verification image is not a bounded regular file"));
        }
        let mut hash = Digest256Hasher::new();
        let mut buffer = [0u8; 65536];
        let mut count = 0u64;
        loop {
            self.check()?;
            let n = f.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            count = count
                .checked_add(n as u64)
                .ok_or_else(|| bad("image byte overflow"))?;
            if count > a.len() {
                return Err(bad("image grew during hash"));
            }
            hash.update(&buffer[..n]);
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
        if count != a.len()
            || stamp(&a) != stamp(&f.metadata()?)
            || stamp(&a) != stamp(&fs::symlink_metadata(p)?)
        {
            return Err(bad("verification image changed"));
        }
        Ok(hash.finalize().to_hex())
    }
}
fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|v| (*v).into()).collect()
}
fn copy_reader_fixture(source: &Path, target: &Path, r: &Run<'_>) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::create_dir(target)?;
    fs::set_permissions(target, fs::Permissions::from_mode(0o700))?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let destination = target.join(entry.file_name());
        if kind.is_dir() {
            copy_reader_fixture(&entry.path(), &destination, r)?;
        } else if kind.is_file() {
            fs::write(&destination, r.bytes(&entry.path(), 1024 * 1024)?)?;
            let mode = if target.file_name().is_some_and(|name| name == "objects")
                || entry.file_name() == "rootset-v2.json"
            {
                0o444
            } else {
                0o600
            };
            fs::set_permissions(&destination, fs::Permissions::from_mode(mode))?;
        } else {
            return Err(bad("reader fixture contains a non-regular entry"));
        }
    }
    Ok(())
}
fn installed(prefix: &Path, name: &str) -> io::Result<PathBuf> {
    let resolved = prefix.join("bin").join(name).canonicalize()?;
    if !resolved.starts_with(prefix) {
        return Err(bad("installed binary resolves outside selected prefix"));
    }
    Ok(resolved)
}
fn verify_reader(root: &Path, prefix: &Path, work: &Path, r: &Run<'_>) -> io::Result<Value> {
    let executable = installed(prefix, "tos-reader")?;
    let binary = path(&executable)?;
    let before = r.digest(&executable)?;
    let capabilities: Value = serde_json::from_slice(&r.run(
        work,
        vec![binary.clone(), "--capabilities".into()],
        &[("PATH", "".into())],
    )?)
    .map_err(|e| bad(e.to_string()))?;
    let required = json!({"schema_version":"tos_reader_capabilities_v1","store_format":"tos_native_admission_v2","supported_store_formats":["tos_native_admission_v2"],"default_format":"v2","platform":"linux","minimum_kernel":"5.6","required_open_api":"openat2","path_traversal":"beneath_no_symlinks","unsafe_fallback":false});
    for (key, value) in required.as_object().unwrap() {
        if capabilities.get(key) != Some(value) {
            return Err(bad(format!(
                "installed reader platform capability differs: {key}"
            )));
        }
    }
    let (code, out, _) = r.capture(
        work,
        vec![binary.clone(), "--format".into(), "v1".into()],
        &[("PATH", "".into())],
    )?;
    if code == 0 || !out.is_empty() {
        return Err(bad("installed reader accepted V1 or returned bytes"));
    }
    let fixture = root.join("tests/conformance/rust/corpus-v2/store");
    let store = work.join("reader-v2-store");
    copy_reader_fixture(&fixture, &store, r)?;
    let stage = work.join("reader-v2-stage");
    fs::create_dir(&stage)?;
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&stage, fs::Permissions::from_mode(0o700))?;
    for name in [
        "staging",
        "segments-v2/staging",
        "segments-v2/segments",
        "segments-v2/pins",
        "segments-v2/attempts",
        "segments-v2/leaves",
    ] {
        fs::create_dir_all(store.join(name))?;
        fs::set_permissions(store.join(name), fs::Permissions::from_mode(0o700))?;
    }
    let cases = [
        (
            "fb256b25e0c6295ffe7f2d26c984d128075acbf4ac614fbed75887eebe83fc0b",
            "retained bytes",
        ),
        (
            "f285308fdf95feabdf64174575b89ee8ac7c31cecb407fd814a7e3cd502d775a",
            "current bytes",
        ),
    ];
    let mut verified = 0;
    for (revision, expected) in cases {
        for (selector, selected) in [
            ("--source-id", "tos.item.example"),
            ("--member-path", "texts/exact.txt"),
        ] {
            let argv = vec![
                binary.clone(),
                "--store".into(),
                path(&store)?,
                "--revision".into(),
                revision.into(),
                selector.into(),
                selected.into(),
                "--stage-dir".into(),
                path(&stage)?,
                "--max-manifest-bytes".into(),
                "65536".into(),
                "--max-manifest-entries".into(),
                "16".into(),
                "--max-selected-object-bytes".into(),
                "4096".into(),
                "--json-max-depth".into(),
                "32".into(),
                "--json-max-visits".into(),
                "100000".into(),
                "--json-max-integer-digits".into(),
                "64".into(),
                "--max-read-bytes".into(),
                "33554432".into(),
                "--max-write-bytes".into(),
                "1048576".into(),
                "--max-segment-bytes".into(),
                "1048576".into(),
                "--max-frame-bytes".into(),
                "65536".into(),
                "--max-frames".into(),
                "8".into(),
                "--max-journal-bytes".into(),
                "65536".into(),
                "--tree-max-key-bytes".into(),
                "1024".into(),
                "--tree-max-value-bytes".into(),
                "65536".into(),
                "--tree-max-kind-bytes".into(),
                "64".into(),
                "--tree-max-node-bytes".into(),
                "131072".into(),
                "--tree-max-children".into(),
                "16".into(),
                "--tree-max-nodes".into(),
                "128".into(),
                "--tree-max-total-bytes".into(),
                "16777216".into(),
                "--tree-max-rows".into(),
                "16".into(),
                "--max-state-bytes".into(),
                "134217728".into(),
                "--deadline-seconds".into(),
                "30".into(),
            ];
            if r.run(work, argv, &[("PATH", "".into())])? != expected.as_bytes() {
                return Err(bad(format!(
                    "installed reader bytes differ for {revision} {selector}"
                )));
            }
            verified += 1;
        }
    }
    if verified == 0 || r.digest(&executable)? != before {
        return Err(bad("reader verification empty or installed image changed"));
    }
    Ok(
        json!({"status":"pass","verified_exact_cases":verified,"reader_sha256":before,"outside_checkout":true,"path_empty":true}),
    )
}
fn verify_commands(root: &Path, prefix: &Path, work: &Path, r: &Run<'_>) -> io::Result<Value> {
    let names = [
        "tos-validation-lanes",
        "tos-release-check",
        "tos-software-ci",
    ];
    let mut products = serde_json::Map::new();
    for name in names {
        products.insert(name.into(), r.digest(&installed(prefix, name)?)?.into());
    }
    fs::create_dir_all(work.join("docs/validation"))?;
    let steps = json!([
        {"label":"software contracts","command":["/bin/sh","-c","printf '%s' native > checks-executed"]},
        {"label":"run tests","command":["/bin/sh","-c","printf '%s' native > tests-executed"]}
    ]);
    fs::write(
        work.join("docs/validation/validation_lanes.json"),
        serde_json::to_vec(&json!({"command_sequences":{"release_check":steps}}))
            .map_err(|e| bad(e.to_string()))?,
    )?;
    let selected = r.run(
        work,
        vec![
            path(&installed(prefix, "tos-validation-lanes")?)?,
            "--repo-root".into(),
            path(work)?,
            "--sequence".into(),
            "release_check".into(),
        ],
        &[("PATH", "".into())],
    )?;
    let expected = steps
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            format!(
                "{}: {}\n",
                s["label"].as_str().unwrap(),
                s["command"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap())
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        })
        .collect::<String>();
    if selected != expected.as_bytes() {
        return Err(bad("installed lane selection differs from authored order"));
    }
    for phase in ["checks", "tests"] {
        r.run(
            work,
            vec![
                path(&installed(prefix, "tos-release-check")?)?,
                "--repo-root".into(),
                path(work)?,
                "--phase".into(),
                phase.into(),
            ],
            &[("PATH", "".into())],
        )?;
        if r.bytes(&work.join(format!("{phase}-executed")), 64)? != b"native"
            || (phase == "checks" && work.join("tests-executed").exists())
        {
            return Err(bad("installed release phase execution differs"));
        }
    }
    let git = |args: &[&str]| -> io::Result<String> {
        let mut argv = strings(&[
            "git",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
        ]);
        argv.extend(strings(args));
        String::from_utf8(r.run(work, argv, &[])?)
            .map(|s| s.trim().to_owned())
            .map_err(|e| bad(e.to_string()))
    };
    git(&["init", "--quiet"])?;
    git(&["config", "user.name", "Installed command fixture"])?;
    git(&["config", "user.email", "fixture@example.invalid"])?;
    git(&["add", "."])?;
    git(&["commit", "--quiet", "-m", "installed command baseline"])?;
    let base = git(&["rev-parse", "HEAD"])?;
    let source = work.join("rust/crates/tos-access/src/package.rs");
    fs::create_dir_all(source.parent().unwrap())?;
    fs::write(source, "// native package trigger\n")?;
    git(&["add", "."])?;
    git(&["commit", "--quiet", "-m", "native package change"])?;
    let ci = path(&installed(prefix, "tos-software-ci")?)?;
    let output = r.run(
        work,
        vec![
            ci.clone(),
            "plan".into(),
            "--repo-root".into(),
            path(work)?,
            "--base".into(),
            base,
        ],
        &[("GITHUB_OUTPUT", path(&work.join("selector-output"))?)],
    )?;
    let selection: Value = serde_json::from_slice(&output).map_err(|e| bad(e.to_string()))?;
    if selection["software_mode"] != "browser"
        || selection["rust"] != true
        || selection["worker"] != false
    {
        return Err(bad("installed CI selector omitted native package consumer"));
    }
    let mut needs = json!({"plan":{"result":"success","outputs":{"software_mode":"browser","worker":"false","rust":"true"}},"software_prepare":{"result":"success"},"software":{"result":"success"},"software_package":{"result":"success"},"worker":{"result":"skipped"},"rust":{"result":"success"},"postgres":{"result":"success"}});
    r.run(
        work,
        vec![ci.clone(), "gate".into()],
        &[("CI_NEEDS", needs.to_string()), ("PATH", "".into())],
    )?;
    needs["plan"]["result"] = "failure".into();
    let (code, _, _) = r.capture(
        work,
        vec![ci, "gate".into()],
        &[("CI_NEEDS", needs.to_string()), ("PATH", "".into())],
    )?;
    if code == 0 {
        return Err(bad("installed CI gate accepted failed preparation"));
    }
    for name in names {
        if products[name] != r.digest(&installed(prefix, name)?)? {
            return Err(bad("installed command changed during verification"));
        }
    }
    let _ = root;
    Ok(
        json!({"status":"pass","products":products,"outside_checkout":true,"native_phase_execution":true,"native_git_selection":true,"failed_preparation_rejected":true}),
    )
}
pub fn run(mode: &str, args: &[String], cancel: &AtomicI32) -> io::Result<()> {
    let mut root = None;
    let mut prefix = None;
    let mut entries_only = false;
    let mut command_ms = 900_000u64;
    let mut lane_ms = 3_600_000u64;
    let mut max_output = 16 * 1024 * 1024usize;
    let mut args = args.iter();
    while let Some(key) = args.next() {
        match key.as_str() {
            "--command-entries-only" => entries_only = true,
            "--repo-root" | "--installed-prefix" => {
                let value = PathBuf::from(
                    args.next()
                        .ok_or_else(|| bad("missing verification path"))?,
                );
                match key.as_str() {
                    "--repo-root" => root = Some(value),
                    _ => prefix = Some(value),
                }
            }
            "--command-timeout-ms" | "--lane-timeout-ms" | "--max-output-bytes" => {
                let value: u64 = args
                    .next()
                    .ok_or_else(|| bad("missing verification limit"))?
                    .parse()
                    .map_err(|_| bad("invalid verification limit"))?;
                if value == 0 {
                    return Err(bad("verification limits must be positive"));
                }
                match key.as_str() {
                    "--command-timeout-ms" if value <= 3_600_000 => command_ms = value,
                    "--lane-timeout-ms" if value <= 7_200_000 => lane_ms = value,
                    "--max-output-bytes" if value <= 64 * 1024 * 1024 => {
                        max_output = value as usize
                    }
                    _ => return Err(bad("verification limit exceeds bound")),
                }
            }
            _ => return Err(bad(format!("unknown verification argument: {key}"))),
        }
    }
    let root = root.ok_or_else(|| bad("--repo-root required"))?;
    exact(&root)?;
    if !["verify-reader-install", "verify-mechanics-install"].contains(&mode) {
        return Err(bad("unknown native verification mode"));
    }
    if entries_only && mode != "verify-mechanics-install" {
        return Err(bad("verification option does not apply to mode"));
    }
    let temporary = Temporary::new()?;
    if temporary.0.starts_with(&root) {
        return Err(bad(
            "verification temporary directory must be outside checkout",
        ));
    }
    let work = temporary.0.join("fixture");
    fs::create_dir(&work)?;
    let r = Run {
        deadline: Instant::now() + Duration::from_millis(lane_ms),
        command_wall: Duration::from_millis(command_ms),
        max_output,
        cancel,
    };
    let result = {
        let prefix = if let Some(prefix) = prefix {
            exact(&prefix)?;
            prefix
        } else {
            let install = temporary.0.join("install");
            let mut argv = strings(&["cargo", "install", "--locked", "--offline", "--path"]);
            argv.push(
                if mode == "verify-reader-install" {
                    "rust/crates/tos-reader"
                } else {
                    "rust/crates/tos-ops-mechanics-plan"
                }
                .into(),
            );
            argv.extend(["--root".into(), path(&install)?]);
            if mode == "verify-mechanics-install" {
                argv.push("--debug".into());
                if entries_only {
                    argv.push("--no-default-features".into());
                    for name in [
                        "tos-validation-lanes",
                        "tos-release-check",
                        "tos-software-ci",
                    ] {
                        argv.extend(["--bin".into(), name.into()]);
                    }
                }
            }
            r.run(&root, argv, &[])?;
            install
        };
        if prefix.starts_with(&root) {
            return Err(bad("native installation must be outside checkout"));
        }
        if mode == "verify-reader-install" {
            verify_reader(&root, &prefix, &work, &r)?
        } else {
            if !entries_only {
                r.run(
                    &root,
                    strings(&[
                        "cargo",
                        "test",
                        "--locked",
                        "--offline",
                        "-p",
                        "tos-ops-mechanics-plan",
                        "--test",
                        "executor_native",
                    ]),
                    &[
                        (
                            "TOS_MECHANICS_TEST_EXECUTABLE",
                            path(&installed(&prefix, "tos-ops-mechanics-plan")?)?,
                        ),
                        (
                            "TOS_VALIDATION_LANES_TEST_EXECUTABLE",
                            path(&installed(&prefix, "tos-validation-lanes")?)?,
                        ),
                        (
                            "TOS_RELEASE_CHECK_TEST_EXECUTABLE",
                            path(&installed(&prefix, "tos-release-check")?)?,
                        ),
                    ],
                )?;
            }
            verify_commands(&root, &prefix, &work, &r)?
        }
    };
    r.check()?;
    serde_json::to_writer(io::stdout().lock(), &result).map_err(|e| bad(e.to_string()))?;
    println!();
    Ok(())
}
