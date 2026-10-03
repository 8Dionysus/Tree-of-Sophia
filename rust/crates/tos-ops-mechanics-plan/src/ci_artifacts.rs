//! Same-run CI artifact contracts. These receipts describe completed Cargo
//! products; they do not authenticate their own producer. Downloaded verifier
//! bytes must first be authenticated against the independent producer output.
use crate::executor::{self, Limits};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::{Duration, Instant};
use tos_foundation::{Digest256Hasher, JsonLimits, JsonMode, parse_json};

const EXECUTORS: [&str; 3] = [
    "tos-release-check",
    "tos-validation-lanes",
    "tos-software-ci",
];
const COMMANDS: [&str; 5] = [
    "tos-native-owner-command",
    "tos-schema-worker",
    "tos-validation-lanes",
    "tos-release-check",
    "tos-software-ci",
];
const TARGET: &str = "x86_64-unknown-linux-gnu";
const TOOLCHAIN: &str = "1.98.1";
const META: usize = 1024 * 1024;
const IMAGE: u64 = 1024 * 1024 * 1024;
const CARGO_BYTES: usize = 32 * 1024 * 1024;
fn bad(s: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, s)
}
struct Budget<'a> {
    deadline: Instant,
    cancel: &'a AtomicI32,
}
impl Budget<'_> {
    fn check(&self) -> io::Result<()> {
        if self.cancel.load(Ordering::Relaxed) != 0 || Instant::now() >= self.deadline {
            return Err(bad("CI artifact operation cancelled or expired"));
        }
        Ok(())
    }
    fn git(&self, root: &Path, reference: &str) -> io::Result<String> {
        self.check()?;
        let remaining = self.deadline.saturating_duration_since(Instant::now());
        let (code, out, _) = executor::capture_ci_git(
            root,
            vec![
                "git".into(),
                "rev-parse".into(),
                "--verify".into(),
                reference.into(),
            ],
            Limits {
                command_wall: remaining.min(Duration::from_secs(30)),
                lane_wall: remaining,
                cleanup_grace: Duration::from_secs(1),
                output_bytes: 8192,
            },
            self.cancel,
        )?;
        self.check()?;
        let value = std::str::from_utf8(&out)
            .map_err(|_| bad("non-UTF8 Git identity"))?
            .trim();
        if code != 0
            || ![40, 64].contains(&value.len())
            || !value.bytes().all(|c| c.is_ascii_hexdigit())
        {
            return Err(bad("invalid Git source identity"));
        }
        Ok(value.into())
    }
}
fn absolute(path: &Path) -> io::Result<()> {
    if !path.is_absolute()
        || path.as_os_str().len() > 4096
        || path.to_str().is_none_or(|s| s.contains(['\n', '\r', '\0']))
        || fs::canonicalize(path)? != path
    {
        return Err(bad("CI path must be canonical absolute without symlinks"));
    }
    Ok(())
}
fn open(path: &Path, cap: u64) -> io::Result<File> {
    absolute(path)?;
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path)?;
    let m = file.metadata()?;
    if !m.is_file() || m.len() > cap {
        return Err(bad("CI input must be a bounded regular file"));
    }
    Ok(file)
}
#[cfg(unix)]
fn stamp(m: &fs::Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
    use std::os::unix::fs::MetadataExt;
    (
        m.dev(),
        m.ino(),
        m.len(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    )
}
fn unchanged(file: &File, path: &Path, before: &fs::Metadata) -> io::Result<()> {
    if stamp(before) != stamp(&file.metadata()?)
        || stamp(before) != stamp(&fs::symlink_metadata(path)?)
    {
        return Err(bad("CI input changed during operation"));
    }
    Ok(())
}
fn bytes(path: &Path, cap: usize, b: &Budget<'_>) -> io::Result<Vec<u8>> {
    b.check()?;
    let mut file = open(path, cap as u64)?;
    let before = file.metadata()?;
    let mut raw = Vec::new();
    Read::by_ref(&mut file)
        .take(cap as u64 + 1)
        .read_to_end(&mut raw)?;
    if raw.len() > cap || raw.len() as u64 != before.len() {
        return Err(bad("CI input byte bound or identity mismatch"));
    }
    unchanged(&file, path, &before)?;
    b.check()?;
    Ok(raw)
}
fn strict(raw: &[u8], cap: usize) -> io::Result<Value> {
    parse_json(
        raw,
        JsonMode::PublishedStrict,
        JsonLimits {
            max_bytes: cap,
            max_depth: 64,
            max_visits: 300000,
            max_integer_digits: 4300,
        },
    )
    .map_err(|_| bad("invalid strict CI JSON"))?;
    serde_json::from_slice(raw).map_err(|_| bad("invalid CI JSON"))
}
fn hash(path: &Path, b: &Budget<'_>) -> io::Result<Value> {
    b.check()?;
    let mut file = open(path, IMAGE)?;
    let before = file.metadata()?;
    if before.len() < 64 {
        return Err(bad("CI image is too small"));
    }
    let mut digest = Digest256Hasher::new();
    let mut buffer = [0u8; 65536];
    let mut count = 0u64;
    loop {
        b.check()?;
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        count = count
            .checked_add(n as u64)
            .ok_or_else(|| bad("CI image bytes overflow"))?;
        if count > IMAGE {
            return Err(bad("CI image bound exceeded"));
        }
        digest.update(&buffer[..n]);
    }
    unchanged(&file, path, &before)?;
    if count != before.len() {
        return Err(bad("CI image length mismatch"));
    }
    b.check()?;
    Ok(json!({"sha256":digest.finalize().to_hex(),"size_bytes":count}))
}
fn sha(path: &Path, b: &Budget<'_>) -> io::Result<String> {
    let raw = bytes(path, META, b)?;
    let mut d = Digest256Hasher::new();
    d.update(&raw);
    Ok(d.finalize().to_hex())
}
fn identity(root: &Path, b: &Budget<'_>) -> io::Result<Value> {
    absolute(root)?;
    Ok(
        json!({"source_commit":b.git(root,"HEAD")?,"source_tree":b.git(root,"HEAD^{tree}")?,"lock_sha256":sha(&root.join("Cargo.lock"),b)?,"toolchain":TOOLCHAIN,"target":TARGET,"profile":"debug","features":[]}),
    )
}
fn keys(value: &Value, names: &[&str]) -> io::Result<()> {
    let obj = value.as_object().ok_or_else(|| bad("CI object required"))?;
    if obj.len() != names.len() || names.iter().any(|n| !obj.contains_key(*n)) {
        return Err(bad("CI object exact keys mismatch"));
    }
    Ok(())
}
fn json_write(path: &Path, value: &Value, b: &Budget<'_>) -> io::Result<()> {
    let mut raw = serde_json::to_vec(value).map_err(|_| bad("CI JSON encoding failed"))?;
    raw.push(b'\n');
    write_new(path, &raw, b)
}
fn write_new(path: &Path, raw: &[u8], b: &Budget<'_>) -> io::Result<()> {
    b.check()?;
    absolute(
        path.parent()
            .ok_or_else(|| bad("CI output parent missing"))?,
    )?;
    if raw.len() > META {
        return Err(bad("CI metadata output exceeded"));
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
        options.mode(0o600);
    }
    let mut f = options.open(path)?;
    f.write_all(raw)?;
    f.flush()?;
    b.check()
}
fn append(path: &Path, raw: &[u8], b: &Budget<'_>) -> io::Result<()> {
    b.check()?;
    absolute(path)?;
    let mut options = OpenOptions::new();
    options.append(true);
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let mut f = options.open(path)?;
    let m = f.metadata()?;
    if !m.is_file()
        || m.len()
            .checked_add(raw.len() as u64)
            .is_none_or(|n| n > META as u64)
    {
        return Err(bad("GitHub output bound exceeded"));
    }
    f.write_all(raw)?;
    f.flush()?;
    b.check()
}
/// Require successful, same-run Cargo messages for the requested executable set.
/// Messages are caller-held producer evidence, never a substitute for bootstrap trust.
fn cargo_products(
    root: &Path,
    messages: &Path,
    images: &Path,
    names: &[&str],
    version: &Path,
    expected_debug_info: u8,
    b: &Budget<'_>,
) -> io::Result<()> {
    absolute(images)?;
    if !images.ends_with(Path::new(TARGET).join("debug")) {
        return Err(bad("Cargo native target/profile directory mismatch"));
    }
    let v = bytes(version, 8192, b)?;
    if !std::str::from_utf8(&v)
        .map_err(|_| bad("invalid rustc version"))?
        .starts_with("rustc 1.98.1 ")
    {
        return Err(bad("Cargo toolchain mismatch"));
    }
    let f = open(messages, CARGO_BYTES as u64)?;
    let before = f.metadata()?;
    let mut reader = BufReader::new(f);
    let mut total = 0usize;
    let mut seen = BTreeSet::new();
    let mut success = 0;
    loop {
        b.check()?;
        let mut line = Vec::new();
        let n = reader
            .by_ref()
            .take(META as u64 + 1)
            .read_until(b'\n', &mut line)?;
        if n == 0 {
            break;
        }
        total = total
            .checked_add(n)
            .ok_or_else(|| bad("Cargo message overflow"))?;
        if n > META || total > CARGO_BYTES || line.last() != Some(&b'\n') {
            return Err(bad("Cargo message framing or byte bound exceeded"));
        }
        let value = strict(&line, META)?;
        if value["reason"] == "build-finished" {
            if value["success"] != true {
                return Err(bad("Cargo build failed"));
            }
            success += 1;
        }
        if value["reason"] != "compiler-artifact" {
            continue;
        }
        let Some(name) = value["target"]["name"].as_str() else {
            continue;
        };
        if !names.contains(&name) {
            continue;
        }
        let package = match name {
            "tos-access" => "tos-access",
            "tos-native-owner-command" => "tos-command",
            "tos-schema-worker" => "tos-validation",
            _ => "tos-ops-mechanics-plan",
        };
        if value["manifest_path"]
            != root
                .join(format!("rust/crates/{package}/Cargo.toml"))
                .to_string_lossy()
                .as_ref()
            || !value["package_id"].as_str().is_some_and(|s| {
                s.starts_with(&format!(
                    "path+file://{}#",
                    root.join(format!("rust/crates/{package}")).display()
                ))
            })
            || value["target"]["kind"] != json!(["bin"])
            || value["features"] != json!([])
            || value["profile"]["opt_level"] != "0"
            || value["profile"]["debug_assertions"] != true
            || value["profile"]["debuginfo"] != expected_debug_info
            || value["profile"]["test"] != false
            || value["executable"] != images.join(name).to_string_lossy().as_ref()
        {
            return Err(bad("Cargo executable context mismatch"));
        }
        if !seen.insert(name.to_owned()) {
            return Err(bad("duplicate Cargo executable"));
        }
    }
    unchanged(reader.get_ref(), messages, &before)?;
    b.check()?;
    if success != if names.len() == 3 { 1 } else { 4 } || seen.len() != names.len() {
        return Err(bad("Cargo stream lacks successful requested products"));
    }
    Ok(())
}
fn artifact_set(root: &Path, with_manifest: bool, b: &Budget<'_>) -> io::Result<()> {
    let mut names = EXECUTORS
        .iter()
        .map(|n| (*n).to_owned())
        .collect::<BTreeSet<_>>();
    if with_manifest {
        names.insert("manifest.json".into());
    }
    for entry in fs::read_dir(root)? {
        b.check()?;
        let entry = entry?;
        if !entry.file_type()?.is_file()
            || !entry.file_name().to_str().is_some_and(|n| names.remove(n))
        {
            return Err(bad("native CI artifact exact file set mismatch"));
        }
    }
    if !names.is_empty() {
        return Err(bad("native CI artifact missing file"));
    }
    Ok(())
}
fn manifest(root: &Path, artifacts: &Path, b: &Budget<'_>) -> io::Result<Value> {
    let mut value = identity(root, b)?;
    let mut binaries = serde_json::Map::new();
    for name in EXECUTORS {
        binaries.insert(name.into(), hash(&artifacts.join(name), b)?);
    }
    value
        .as_object_mut()
        .unwrap()
        .insert("binaries".into(), Value::Object(binaries));
    Ok(value)
}
fn validate_manifest(actual: &Value, expected: &Value) -> io::Result<()> {
    keys(
        actual,
        &[
            "source_commit",
            "source_tree",
            "lock_sha256",
            "toolchain",
            "target",
            "profile",
            "features",
            "binaries",
        ],
    )?;
    keys(&actual["binaries"], &EXECUTORS)?;
    for name in EXECUTORS {
        keys(&actual["binaries"][name], &["sha256", "size_bytes"])?;
    }
    if actual != expected {
        return Err(bad("native CI executor source or bytes mismatch"));
    }
    Ok(())
}
fn walk(
    depth: usize,
    dir: &Path,
    only_json: bool,
    out: &mut Vec<PathBuf>,
    b: &Budget<'_>,
) -> io::Result<()> {
    b.check()?;
    if depth > 64 {
        return Err(bad("software directory depth exceeded"));
    }
    absolute(dir)?;
    let mut entries = 0usize;
    for entry in fs::read_dir(dir)? {
        b.check()?;
        entries += 1;
        if entries > 1024 {
            return Err(bad("software directory bound exceeded"));
        }
        let p = entry?.path();
        let m = fs::symlink_metadata(&p)?;
        if m.file_type().is_symlink() {
            return Err(bad("software input symlink"));
        }
        if m.is_dir() {
            walk(depth + 1, &p, only_json, out, b)?;
        } else if m.is_file() && (!only_json || p.extension().is_some_and(|e| e == "json")) {
            if out.len() >= 1024 {
                return Err(bad("software member bound exceeded"));
            }
            out.push(p);
        } else if !m.is_file() {
            return Err(bad("software input not regular"));
        }
    }
    Ok(())
}
fn package_limits(root: &Path, images: &Path, b: &Budget<'_>) -> io::Result<String> {
    let mut files = vec![images.join("tos-access")];
    for n in COMMANDS {
        files.push(images.join(n));
    }
    walk(0, &root.join("access/web/dist"), false, &mut files, b)?;
    let mut access = Vec::new();
    for folder in ["access/contracts", "access/profiles"] {
        walk(0, &root.join(folder), true, &mut access, b)?;
    }
    if files
        .len()
        .checked_add(
            access
                .len()
                .checked_mul(2)
                .ok_or_else(|| bad("software member overflow"))?,
        )
        .and_then(|n| n.checked_add(5))
        .is_none_or(|n| n > 1024)
    {
        return Err(bad("software input envelope exceeds member cap"));
    }
    files.extend(access.iter().cloned());
    files.extend(access);
    for n in ["access/README.md", "Cargo.lock", "rust-toolchain.toml"] {
        files.push(root.join(n));
    }
    walk(0, &root.join("ToS/contracts"), true, &mut files, b)?;
    if files.len() + 2 > 1024 {
        return Err(bad("software input envelope exceeds member cap"));
    }
    let mut total = 1048576u64;
    for p in files {
        b.check()?;
        let file = open(&p, IMAGE)?;
        let m = file.metadata()?;
        total = total
            .checked_add(m.len())
            .ok_or_else(|| bad("software bytes overflow"))?;
        unchanged(&file, &p, &m)?;
    }
    let archive = total
        .checked_add(16777216)
        .ok_or_else(|| bad("archive bytes overflow"))?;
    b.check()?;
    Ok(format!("{total} {archive} 1024 4194304\n"))
}
/// These four bounded CLI operations retain the existing manifest and software
/// archive receipt shapes. Platform transport and initial verifier trust stay external.
pub fn run(mode: &str, args: &[String], cancel: &AtomicI32) -> io::Result<()> {
    let b = Budget {
        deadline: Instant::now() + Duration::from_secs(120),
        cancel,
    };
    let mut flags = BTreeMap::new();
    for pair in args.chunks(2) {
        if pair.len() != 2
            || !pair[0].starts_with("--")
            || flags
                .insert(pair[0].as_str(), PathBuf::from(&pair[1]))
                .is_some()
        {
            return Err(bad("invalid CI artifact arguments"));
        }
    }
    let allowed: &[&str] = match mode {
        "executor-manifest" => &[
            "--repo-root",
            "--artifact-root",
            "--image-root",
            "--cargo-messages",
            "--rustc-version",
            "--github-output",
        ],
        "executor-bind" => &[
            "--repo-root",
            "--artifact-root",
            "--github-env",
            "--github-path",
        ],
        "software-receipts" => &[
            "--repo-root",
            "--image-root",
            "--receipt-root",
            "--cargo-messages",
            "--rustc-version",
            "--github-env",
        ],
        "software-limits" => &["--repo-root", "--image-root", "--receipt-root"],
        _ => return Err(bad("unknown CI artifact operation")),
    };
    if flags.len() != allowed.len() || allowed.iter().any(|k| !flags.contains_key(k)) {
        return Err(bad("CI artifact exact arguments required"));
    }
    let get = |n: &str| flags.get(n).unwrap().as_path();
    let root = get("--repo-root");
    absolute(root)?;
    let initial_identity = identity(root, &b)?;
    match mode {
        "executor-manifest" => {
            let artifacts = get("--artifact-root");
            absolute(artifacts)?;
            artifact_set(artifacts, false, &b)?;
            cargo_products(
                root,
                get("--cargo-messages"),
                get("--image-root"),
                &EXECUTORS,
                get("--rustc-version"),
                1,
                &b,
            )?;
            let v = manifest(root, artifacts, &b)?;
            // Copies must match the completed Cargo products, not only one another.
            for n in EXECUTORS {
                if v["binaries"][n] != hash(&get("--image-root").join(n), &b)? {
                    return Err(bad("copied executor differs from Cargo product"));
                }
            }
            json_write(&artifacts.join("manifest.json"), &v, &b)?;
            let verifier = v["binaries"]["tos-software-ci"]["sha256"].as_str().unwrap();
            let manifest_sha = sha(&artifacts.join("manifest.json"), &b)?;
            append(
                get("--github-output"),
                format!("executor_sha256={verifier}\nexecutor_manifest_sha256={manifest_sha}\n")
                    .as_bytes(),
                &b,
            )?;
        }
        "executor-bind" => {
            let artifacts = get("--artifact-root");
            absolute(artifacts)?;
            artifact_set(artifacts, true, &b)?;
            let actual = strict(&bytes(&artifacts.join("manifest.json"), META, &b)?, META)?;
            validate_manifest(&actual, &manifest(root, artifacts, &b)?)?;
            let env = EXECUTORS
                .iter()
                .map(|n| {
                    format!(
                        "{}_EXECUTOR={}\n",
                        n.to_uppercase().replace('-', "_"),
                        artifacts.join(n).display()
                    )
                })
                .collect::<String>();
            append(get("--github-env"), env.as_bytes(), &b)?;
            append(
                get("--github-path"),
                format!("{}\n", artifacts.display()).as_bytes(),
                &b,
            )?;
        }
        "software-receipts" => {
            let images = get("--image-root");
            let receipt_root = get("--receipt-root");
            absolute(receipt_root)?;
            let mut names = vec!["tos-access"];
            names.extend(COMMANDS);
            cargo_products(
                root,
                get("--cargo-messages"),
                images,
                &names,
                get("--rustc-version"),
                1,
                &b,
            )?;
            let id = identity(root, &b)?;
            let mut products = serde_json::Map::new();
            for n in names {
                let mut proof = id.clone();
                let obj = proof.as_object_mut().unwrap();
                if n == "tos-access" {
                    obj.remove("features");
                }
                obj.insert(
                    "schema_version".into(),
                    json!(if n == "tos-access" {
                        "tos_native_access_build_v1"
                    } else {
                        "tos_native_software_command_build_v1"
                    }),
                );
                let image = hash(&images.join(n), &b)?;
                for k in ["sha256", "size_bytes"] {
                    obj.insert(k.into(), image[k].clone());
                }
                let receipt = receipt_root.join(if n == "tos-access" {
                    "tos-native-build.json".into()
                } else {
                    format!("{n}-build.json")
                });
                json_write(&receipt, &proof, &b)?;
                if n != "tos-access" {
                    products.insert(n.into(), json!({"binary":images.join(n),"receipt":receipt}));
                }
            }
            json_write(
                &receipt_root.join("tos-native-command-products.json"),
                &Value::Object(products),
                &b,
            )?;
            append(get("--github-env"),format!("TOS_NATIVE_OWNER_COMMAND_BIN={}\nTOS_PREPARED_EXECUTOR={}\nTOS_NATIVE_SOURCE_PREPARE_EXECUTABLE={}\nTOS_PREPARED_MAX_SECONDS=20\nTOS_NATIVE_SOURCE_PREPARE_SECONDS=45\nTOS_NATIVE_SOURCE_PREPARE_LIMITS={{\"max_input_bytes\":1048576,\"max_capture_bytes\":8388608,\"max_stage_bytes\":8388608,\"max_temp_bytes\":8388608}}\n",images.join("tos-native-owner-command").display(),images.join("tos-access").display(),images.join("tos-access").display()).as_bytes(),&b)?;
        }
        "software-limits" => {
            absolute(get("--image-root"))?;
            absolute(get("--receipt-root"))?;
            let products = strict(
                &bytes(
                    &get("--receipt-root").join("tos-native-command-products.json"),
                    META,
                    &b,
                )?,
                META,
            )?;
            keys(&products, &COMMANDS)?;
            for n in COMMANDS {
                keys(&products[n], &["binary", "receipt"])?;
                if products[n]["binary"] != get("--image-root").join(n).to_string_lossy().as_ref()
                    || products[n]["receipt"]
                        != get("--receipt-root")
                            .join(format!("{n}-build.json"))
                            .to_string_lossy()
                            .as_ref()
                {
                    return Err(bad("software product selection mismatch"));
                }
            }
            write_new(
                &get("--receipt-root").join("tos-native-package-limits.txt"),
                package_limits(root, get("--image-root"), &b)?.as_bytes(),
                &b,
            )?;
        }
        _ => unreachable!(),
    }
    if identity(root, &b)? != initial_identity {
        return Err(bad("CI source changed during operation"));
    }
    b.check()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn executor_manifest_requires_exact_identity_and_set() {
        let mut v = json!({"source_commit":"a","source_tree":"b","lock_sha256":"c","toolchain":TOOLCHAIN,"target":TARGET,"profile":"debug","features":[],"binaries":{}});
        for n in EXECUTORS {
            v["binaries"][n] = json!({"sha256":"d","size_bytes":64});
        }
        assert!(validate_manifest(&v, &v).is_ok());
        let mut changed = v.clone();
        changed["profile"] = json!("release");
        assert!(validate_manifest(&changed, &v).is_err());
        changed = v.clone();
        changed["binaries"]["unexpected"] = json!({});
        assert!(validate_manifest(&changed, &v).is_err());
        changed = v.clone();
        changed["binaries"][EXECUTORS[0]]["extra"] = json!(true);
        assert!(validate_manifest(&changed, &v).is_err());
        assert!(strict(br#"{"a":1,"a":2}"#, META).is_err());
    }
    fn fixture() -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "tos-ci-artifacts-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        root
    }
    #[test]
    fn held_images_and_exports_refuse_symlinks_overbounds_and_overwrites() {
        let root = fixture();
        let cancel = AtomicI32::new(0);
        let b = Budget {
            deadline: Instant::now() + Duration::from_secs(10),
            cancel: &cancel,
        };
        let image = root.join("image");
        fs::write(&image, [42u8; 64]).unwrap();
        assert_eq!(hash(&image, &b).unwrap()["size_bytes"], 64);
        #[cfg(unix)]
        {
            let link = root.join("link");
            std::os::unix::fs::symlink(&image, &link).unwrap();
            assert!(hash(&link, &b).is_err());
        }
        let large = root.join("large");
        File::create(&large).unwrap().set_len(IMAGE + 1).unwrap();
        assert!(hash(&large, &b).is_err());
        let output = root.join("env");
        fs::write(&output, b"held").unwrap();
        assert!(write_new(&output, b"replacement", &b).is_err());
        File::options()
            .write(true)
            .open(&output)
            .unwrap()
            .set_len(META as u64)
            .unwrap();
        assert!(append(&output, b"x", &b).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn producer_requires_successful_cargo_context_not_image_existence() {
        let root = fixture();
        let images = root.join(TARGET).join("debug");
        fs::create_dir_all(&images).unwrap();
        let version = root.join("rustc-version");
        fs::write(&version, b"rustc 1.98.1 (selected)\n").unwrap();
        let messages = root.join("cargo.jsonl");
        let mut stream = Vec::new();
        for n in EXECUTORS {
            let event = json!({"reason":"compiler-artifact","package_id":format!("path+file://{}/rust/crates/tos-ops-mechanics-plan#0.1.0",root.display()),"manifest_path":root.join("rust/crates/tos-ops-mechanics-plan/Cargo.toml"),"target":{"name":n,"kind":["bin"]},"features":[],"profile":{"opt_level":"0","debuginfo":1,"debug_assertions":true,"test":false},"executable":images.join(n)});
            serde_json::to_writer(&mut stream, &event).unwrap();
            stream.push(b'\n');
        }
        let cancel = AtomicI32::new(0);
        let b = Budget {
            deadline: Instant::now() + Duration::from_secs(10),
            cancel: &cancel,
        };
        fs::write(&messages, &stream).unwrap();
        assert!(cargo_products(&root, &messages, &images, &EXECUTORS, &version, 1, &b).is_err());
        stream.extend_from_slice(b"{\"reason\":\"build-finished\",\"success\":true}\n");
        fs::write(&messages, &stream).unwrap();
        assert!(cargo_products(&root, &messages, &images, &EXECUTORS, &version, 1, &b).is_ok());
        assert!(cargo_products(&root, &messages, &images, &EXECUTORS, &version, 2, &b).is_err());
        let wrong = root.join("wrong-images");
        fs::create_dir(&wrong).unwrap();
        assert!(cargo_products(&root, &messages, &wrong, &EXECUTORS, &version, 1, &b).is_err());
        // Software receipts require the explicitly selected reduced debug-info profile.
        let mut names = vec!["tos-access"];
        names.extend(COMMANDS);
        let mut software_stream = Vec::new();
        for n in &names {
            let package = match *n {
                "tos-access" => "tos-access",
                "tos-native-owner-command" => "tos-command",
                "tos-schema-worker" => "tos-validation",
                _ => "tos-ops-mechanics-plan",
            };
            let event = json!({"reason":"compiler-artifact","package_id":format!("path+file://{}/rust/crates/{package}#0.1.0",root.display()),"manifest_path":root.join(format!("rust/crates/{package}/Cargo.toml")),"target":{"name":n,"kind":["bin"]},"features":[],"profile":{"opt_level":"0","debuginfo":1,"debug_assertions":true,"test":false},"executable":images.join(n)});
            serde_json::to_writer(&mut software_stream, &event).unwrap();
            software_stream.push(b'\n');
        }
        for _ in 0..4 {
            software_stream
                .extend_from_slice(b"{\"reason\":\"build-finished\",\"success\":true}\n");
        }
        fs::write(&messages, &software_stream).unwrap();
        assert!(cargo_products(&root, &messages, &images, &names, &version, 1, &b).is_ok());
        assert!(cargo_products(&root, &messages, &images, &names, &version, 2, &b).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
