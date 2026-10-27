//! Bounded cross-job handoff for the native Software CI products.
//!
//! The producer validates the existing per-product receipts and the separate
//! test-fixture Cargo stream before sealing only the files needed by fixture
//! tests and package/browser validation. Consumers authenticate the manifest
//! with the producer job output before copying or making any staged binary
//! executable.
use crate::ci_artifacts::{self, Budget};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::AtomicI32;
use std::time::Duration;
use tos_foundation::Digest256Hasher;
use tos_foundation::native_software_roles::COMMANDS;

const SCHEMA: &str = "tos_software_ci_stage_manifest_v1";
const MAX_STAGE_MEMBERS: usize = 1024;
const MAX_STAGE_PAYLOAD_FILES: usize = MAX_STAGE_MEMBERS - 1;
const MAX_STAGE_DIRECTORIES: usize = 1024;
const MAX_STAGE_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MAX_GENERATED_BYTES: u64 = 64 * 1024 * 1024;
const STAGE_TIMEOUT: Duration = Duration::from_secs(180);
const FIXTURE: &str = "tos-e2e-fixture";
const FIXTURE_MESSAGES: &str = "fixture/cargo-messages.jsonl";

fn bad(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn parse_flags(args: &[String], allowed: &[&str]) -> io::Result<BTreeMap<String, PathBuf>> {
    let mut flags = BTreeMap::new();
    for pair in args.chunks(2) {
        if pair.len() != 2
            || !pair[0].starts_with("--")
            || pair[1].is_empty()
            || pair[1].contains(['\n', '\r', '\0'])
            || flags
                .insert(pair[0].clone(), PathBuf::from(&pair[1]))
                .is_some()
        {
            return Err(bad("invalid Software stage arguments"));
        }
    }
    if flags.len() != allowed.len() || allowed.iter().any(|key| !flags.contains_key(*key)) {
        return Err(bad("Software stage exact arguments required"));
    }
    Ok(flags)
}

fn path<'a>(flags: &'a BTreeMap<String, PathBuf>, key: &str) -> &'a Path {
    flags[key].as_path()
}

fn product_names() -> Vec<&'static str> {
    let mut names = vec!["tos-access"];
    names.extend(COMMANDS);
    names
}

fn receipt_names() -> Vec<String> {
    let mut names = vec!["tos-native-build.json".to_owned()];
    names.extend(COMMANDS.iter().map(|name| format!("{name}-build.json")));
    names.push("tos-native-command-products.json".to_owned());
    names
}

fn canonical_dir(path: &Path) -> io::Result<()> {
    ci_artifacts::absolute(path)?;
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(bad("Software stage directory must be a real directory"));
    }
    Ok(())
}

fn ensure_empty_dir(path: &Path) -> io::Result<()> {
    if path.exists() {
        canonical_dir(path)?;
        if fs::read_dir(path)?.next().is_some() {
            return Err(bad("Software stage output directory must be empty"));
        }
    } else {
        secure_create_dirs(path)?;
        canonical_dir(path)?;
    }
    Ok(())
}

/// Create a directory chain while refusing any existing symlink component.
fn secure_create_dirs(path: &Path) -> io::Result<()> {
    if !path.is_absolute()
        || path.as_os_str().len() > 4096
        || path
            .to_str()
            .is_none_or(|value| value.contains(['\n', '\r', '\0']))
    {
        return Err(bad("Software stage destination must be an absolute path"));
    }
    let mut current = PathBuf::new();
    for component in path.components() {
        match component {
            Component::RootDir => current.push(component.as_os_str()),
            Component::Normal(part) => {
                current.push(part);
                match fs::symlink_metadata(&current) {
                    Ok(metadata) if !metadata.file_type().is_symlink() && metadata.is_dir() => {
                        if fs::canonicalize(&current)? != current {
                            return Err(bad("Software stage destination has a symlink path"));
                        }
                    }
                    Ok(_) => return Err(bad("Software stage destination path is not a directory")),
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {
                        fs::create_dir(&current)?;
                    }
                    Err(error) => return Err(error),
                }
            }
            Component::CurDir | Component::ParentDir | Component::Prefix(_) => {
                return Err(bad("Software stage destination path is not normalized"));
            }
        }
    }
    Ok(())
}

#[cfg(unix)]
fn unchanged_stamp(metadata: &fs::Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
    use std::os::unix::fs::MetadataExt;
    (
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec(),
    )
}

#[cfg(not(unix))]
fn unchanged_stamp(metadata: &fs::Metadata) -> (u64,) {
    (metadata.len(),)
}

fn read_open(path: &Path) -> io::Result<File> {
    ci_artifacts::absolute(path)?;
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(bad("Software stage input must be a regular file"));
    }
    Ok(file)
}

fn write_open(path: &Path) -> io::Result<File> {
    ci_artifacts::absolute(
        path.parent()
            .ok_or_else(|| bad("Software stage output parent missing"))?,
    )?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
        options.mode(0o600);
    }
    options.open(path)
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> io::Result<()> {
    Ok(())
}

#[cfg(unix)]
fn is_executable(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_metadata: &fs::Metadata) -> bool {
    true
}

fn copy_file(source: &Path, target: &Path, cap: u64, b: &Budget<'_>) -> io::Result<(String, u64)> {
    b.check()?;
    let mut input = read_open(source)?;
    let before = input.metadata()?;
    if before.len() > cap {
        return Err(bad("Software stage file exceeds its byte cap"));
    }
    let mut output = write_open(target)?;
    let mut digest = Digest256Hasher::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 65536];
    loop {
        b.check()?;
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count as u64)
            .filter(|size| *size <= cap)
            .ok_or_else(|| bad("Software stage file byte cap exceeded"))?;
        output.write_all(&buffer[..count])?;
        digest.update(&buffer[..count]);
    }
    output.flush()?;
    let after = input.metadata()?;
    if unchanged_stamp(&before) != unchanged_stamp(&after)
        || unchanged_stamp(&before) != unchanged_stamp(&fs::symlink_metadata(source)?)
        || total != before.len()
    {
        return Err(bad("Software stage input changed while copying"));
    }
    // Uploaded binaries are intentionally non-executable; bind restores mode
    // only after the consumer has authenticated every staged byte.
    set_mode(target, 0o644)?;
    b.check()?;
    Ok((digest.finalize().to_hex(), total))
}

fn rel_string(path: &Path) -> io::Result<String> {
    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(bad(
            "Software stage path must be a normalized relative path",
        ));
    }
    if path.components().count() > 16 || path.as_os_str().len() > 4096 {
        return Err(bad("Software stage path exceeds its depth or byte cap"));
    }
    path.to_str()
        .map(|value| value.replace('\\', "/"))
        .ok_or_else(|| bad("Software stage path must be UTF-8"))
}

fn stage_path(root: &Path, relative: &str) -> io::Result<PathBuf> {
    let relative_path = Path::new(relative);
    let normalized = rel_string(relative_path)?;
    if normalized != relative {
        return Err(bad("Software stage manifest path is not normalized"));
    }
    Ok(root.join(relative_path))
}

fn file_cap(path: &str) -> io::Result<u64> {
    if path.starts_with("images/") || path == "fixture/tos-e2e-fixture" {
        Ok(ci_artifacts::IMAGE)
    } else if path.starts_with("receipts/") {
        Ok(ci_artifacts::META as u64)
    } else if path == FIXTURE_MESSAGES {
        Ok(ci_artifacts::CARGO_BYTES as u64)
    } else if path.starts_with("worker-generated/") {
        Ok(MAX_GENERATED_BYTES)
    } else if path.starts_with("web/dist/") {
        Ok(ci_artifacts::IMAGE)
    } else {
        Err(bad("Software stage contains an unowned path"))
    }
}

fn add_record(
    records: &mut BTreeMap<String, (String, u64)>,
    path: String,
    digest: String,
    size: u64,
) -> io::Result<()> {
    if records.insert(path, (digest, size)).is_some() {
        return Err(bad("duplicate Software stage file"));
    }
    if records.len() > MAX_STAGE_PAYLOAD_FILES {
        return Err(bad("Software stage member cap exceeded"));
    }
    Ok(())
}

fn sorted_children(directory: &Path, b: &Budget<'_>) -> io::Result<Vec<fs::DirEntry>> {
    let mut children = Vec::new();
    for child in fs::read_dir(directory)? {
        b.check()?;
        if children.len() >= MAX_STAGE_MEMBERS {
            return Err(bad("Software stage directory entry cap exceeded"));
        }
        children.push(child?);
    }
    children.sort_by_key(|entry| entry.file_name());
    Ok(children)
}

fn copy_tree(
    source_root: &Path,
    stage_root: &Path,
    stage_prefix: &str,
    max_tree_bytes: u64,
    records: &mut BTreeMap<String, (String, u64)>,
    b: &Budget<'_>,
) -> io::Result<u64> {
    canonical_dir(source_root)?;
    let mut pending = vec![(source_root.to_path_buf(), PathBuf::new(), 0usize)];
    let mut total = 0u64;
    let mut found = 0usize;
    let mut directory_count = 0usize;
    while let Some((directory, relative_dir, depth)) = pending.pop() {
        b.check()?;
        if depth > 16 {
            return Err(bad("Software stage tree depth exceeded"));
        }
        let children = sorted_children(&directory, b)?;
        for child in children {
            b.check()?;
            let kind = child.file_type()?;
            let name = child.file_name();
            if name
                .to_str()
                .is_none_or(|value| value.is_empty() || value.contains(['/', '\\']))
            {
                return Err(bad("Software stage tree has an invalid file name"));
            }
            let relative = relative_dir.join(&name);
            let source = child.path();
            if kind.is_symlink() {
                return Err(bad("Software stage refuses symlinks"));
            }
            if kind.is_dir() {
                directory_count += 1;
                if directory_count > MAX_STAGE_DIRECTORIES {
                    return Err(bad("Software stage directory cap exceeded"));
                }
                pending.push((source, relative, depth + 1));
                continue;
            }
            if !kind.is_file() {
                return Err(bad("Software stage accepts only regular files"));
            }
            found += 1;
            if found > MAX_STAGE_PAYLOAD_FILES {
                return Err(bad("Software stage member cap exceeded"));
            }
            let relative = rel_string(&relative)?;
            let staged = format!("{stage_prefix}/{relative}");
            let target = stage_path(stage_root, &staged)?;
            secure_create_dirs(target.parent().unwrap())?;
            let (digest, size) = copy_file(&source, &target, file_cap(&staged)?, b)?;
            total = total
                .checked_add(size)
                .filter(|bytes| *bytes <= max_tree_bytes)
                .ok_or_else(|| bad("Software stage tree byte cap exceeded"))?;
            add_record(records, staged, digest, size)?;
        }
    }
    if found == 0 {
        return Err(bad("Software stage input tree is empty"));
    }
    Ok(total)
}

fn check_assets(records: &BTreeMap<String, (String, u64)>) -> io::Result<()> {
    for required in [
        "web/dist/assets/tos-graph.js",
        "web/dist/assets/tos-graph.css",
        "worker-generated/tos_web_rules.js",
        "worker-generated/tos_web_rules_bg.wasm",
    ] {
        if !records.contains_key(required) {
            return Err(bad("Software stage required web asset is missing"));
        }
    }
    Ok(())
}

fn expected_manifest_paths() -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    for name in product_names() {
        paths.insert(format!("images/{name}"));
    }
    for name in receipt_names() {
        paths.insert(format!("receipts/{name}"));
    }
    paths.insert(format!("fixture/{FIXTURE}"));
    paths.insert(FIXTURE_MESSAGES.to_owned());
    paths
}

fn records_value(records: &BTreeMap<String, (String, u64)>) -> Value {
    Value::Array(
        records
            .iter()
            .map(|(path, (digest, size))| json!({"path":path,"sha256":digest,"size_bytes":size}))
            .collect(),
    )
}

fn record_map(value: &Value) -> io::Result<BTreeMap<String, (String, u64)>> {
    let entries = value
        .as_array()
        .ok_or_else(|| bad("Software stage files must be an array"))?;
    if entries.is_empty() || entries.len() > MAX_STAGE_PAYLOAD_FILES {
        return Err(bad("Software stage file count outside bound"));
    }
    let mut result = BTreeMap::new();
    let mut prior = None;
    for entry in entries {
        ci_artifacts::keys(entry, &["path", "sha256", "size_bytes"])?;
        let path = entry["path"]
            .as_str()
            .ok_or_else(|| bad("Software stage path missing"))?
            .to_owned();
        let digest = entry["sha256"]
            .as_str()
            .ok_or_else(|| bad("Software stage hash missing"))?
            .to_owned();
        let size = entry["size_bytes"]
            .as_u64()
            .ok_or_else(|| bad("Software stage size missing"))?;
        if digest.len() != 64
            || !digest
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            || prior
                .as_ref()
                .is_some_and(|previous: &String| previous >= &path)
            || result.insert(path.clone(), (digest, size)).is_some()
        {
            return Err(bad("Software stage files must be sorted and unique"));
        }
        let _ = stage_path(Path::new("/stage"), &path)?;
        if size > file_cap(&path)? {
            return Err(bad("Software stage file exceeds its declared class cap"));
        }
        prior = Some(path);
    }
    Ok(result)
}

fn fixture_contract(root: &Path, b: &Budget<'_>) -> io::Result<Value> {
    let features = ci_artifacts::software_fixture_features(root, b)?;
    Ok(json!({
        "name": FIXTURE,
        "package": "tos-access",
        "default_features": true,
        "features": features,
        "target": ci_artifacts::TARGET,
        "profile": "debug",
        "opt_level": "0",
        "debug_assertions": true,
        "debuginfo": 0,
        "test": false
    }))
}

fn worker_rules_contract() -> Value {
    json!({
        "package": "tos-web-rules",
        "features": ["wasm"],
        "target": "wasm32-unknown-unknown",
        "profile": "release",
        "toolchain": ci_artifacts::TOOLCHAIN,
        "wasm_bindgen": "0.2.128"
    })
}

fn current_platform() -> io::Result<Value> {
    let mut id = std::env::consts::OS.to_owned();
    let mut version_id = "unknown".to_owned();
    if std::env::consts::OS == "linux" {
        let source = File::open("/etc/os-release")?;
        let mut raw = Vec::new();
        source.take(8193).read_to_end(&mut raw)?;
        if raw.len() > 8192 {
            return Err(bad("Software stage OS identity exceeds byte cap"));
        }
        let text = std::str::from_utf8(&raw)
            .map_err(|_| bad("Software stage OS identity is not UTF-8"))?;
        for line in text.lines() {
            if let Some(value) = line.strip_prefix("ID=") {
                id = value
                    .trim_matches(|character| character == '\'' || character == '"')
                    .to_owned();
            } else if let Some(value) = line.strip_prefix("VERSION_ID=") {
                version_id = value
                    .trim_matches(|character| character == '\'' || character == '"')
                    .to_owned();
            }
        }
    }
    if id.is_empty() || version_id.is_empty() {
        return Err(bad("Software stage platform identity is incomplete"));
    }
    Ok(json!({
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "id": id,
        "version_id": version_id
    }))
}

fn make_manifest(
    platform: Value,
    identity: Value,
    fixture: Value,
    records: &BTreeMap<String, (String, u64)>,
    total_bytes: u64,
) -> Value {
    json!({
        "schema_version": SCHEMA,
        "platform": platform,
        "source_identity": identity,
        "products": product_names(),
        "fixture": fixture,
        "worker_rules": worker_rules_contract(),
        "files": records_value(records),
        "limits": {
            "max_members": MAX_STAGE_MEMBERS,
            "max_directories": MAX_STAGE_DIRECTORIES,
            "max_total_bytes": MAX_STAGE_BYTES,
            "max_single_image_bytes": ci_artifacts::IMAGE,
            "max_fixture_messages_bytes": ci_artifacts::CARGO_BYTES,
            "max_worker_generated_bytes": MAX_GENERATED_BYTES,
            "producer_payload_bytes": total_bytes
        }
    })
}

fn stage_manifest(flags: &BTreeMap<String, PathBuf>, b: &Budget<'_>) -> io::Result<()> {
    let root = path(flags, "--repo-root");
    let stage_root = path(flags, "--stage-root");
    let image_root = path(flags, "--image-root");
    let receipt_root = path(flags, "--receipt-root");
    let fixture_messages = path(flags, "--fixture-messages");
    let rustc_version = path(flags, "--rustc-version");
    let web_dist = path(flags, "--web-dist");
    let worker_generated = path(flags, "--worker-generated");
    let github_output = path(flags, "--github-output");
    for source in [
        root,
        image_root,
        receipt_root,
        fixture_messages,
        rustc_version,
        web_dist,
        worker_generated,
        github_output,
    ] {
        ci_artifacts::absolute(source)?;
    }
    ensure_empty_dir(stage_root)?;
    if web_dist != root.join("access/web/dist")
        || worker_generated != root.join("access/deploy/cloudflare-worker/generated")
    {
        return Err(bad("Software stage asset source path mismatch"));
    }
    let source_proof = ci_artifacts::validate_software_stage_inputs(
        root,
        image_root,
        receipt_root,
        fixture_messages,
        rustc_version,
        b,
    )?;
    let identity = source_proof["identity"].clone();
    let fixture_binary = source_proof["fixture_binary"].clone();
    let fixture_messages_hash = source_proof["fixture_messages"].clone();
    let fixture = fixture_contract(root, b)?;
    let platform = current_platform()?;
    for directory in ["images", "receipts", "fixture", "web", "worker-generated"] {
        secure_create_dirs(&stage_root.join(directory))?;
    }

    let mut records = BTreeMap::new();
    for name in product_names() {
        let source = image_root.join(name);
        let metadata = fs::symlink_metadata(&source)?;
        if !metadata.is_file() || !is_executable(&metadata) {
            return Err(bad("Software stage executable input is not executable"));
        }
        let relative = format!("images/{name}");
        let target = stage_path(stage_root, &relative)?;
        let (digest, size) = copy_file(&source, &target, ci_artifacts::IMAGE, b)?;
        add_record(&mut records, relative, digest, size)?;
    }
    for name in receipt_names() {
        let source = receipt_root.join(&name);
        let relative = format!("receipts/{name}");
        let target = stage_path(stage_root, &relative)?;
        let (digest, size) = copy_file(&source, &target, ci_artifacts::META as u64, b)?;
        add_record(&mut records, relative, digest, size)?;
    }
    if ci_artifacts::validate_software_stage_payload(
        root,
        &stage_root.join("images"),
        &stage_root.join("receipts"),
        b,
    )? != identity
    {
        return Err(bad(
            "Software stage copied products have a different identity",
        ));
    }
    let fixture_source = image_root.join(FIXTURE);
    let fixture_metadata = fs::symlink_metadata(&fixture_source)?;
    if !fixture_metadata.is_file() || !is_executable(&fixture_metadata) {
        return Err(bad("Software stage fixture input is not executable"));
    }
    let fixture_target = stage_path(stage_root, &format!("fixture/{FIXTURE}"))?;
    let (digest, size) = copy_file(&fixture_source, &fixture_target, ci_artifacts::IMAGE, b)?;
    if !hash_record_matches(&fixture_binary, &digest, size) {
        return Err(bad("Software stage fixture changed while staging"));
    }
    add_record(&mut records, format!("fixture/{FIXTURE}"), digest, size)?;
    let messages_target = stage_path(stage_root, FIXTURE_MESSAGES)?;
    let (digest, size) = copy_file(
        fixture_messages,
        &messages_target,
        ci_artifacts::CARGO_BYTES as u64,
        b,
    )?;
    if !hash_record_matches(&fixture_messages_hash, &digest, size) {
        return Err(bad("Software fixture Cargo stream changed while staging"));
    }
    add_record(&mut records, FIXTURE_MESSAGES.into(), digest, size)?;

    let web_bytes = copy_tree(
        web_dist,
        stage_root,
        "web/dist",
        MAX_STAGE_BYTES,
        &mut records,
        b,
    )?;
    let generated_bytes = copy_tree(
        worker_generated,
        stage_root,
        "worker-generated",
        MAX_GENERATED_BYTES,
        &mut records,
        b,
    )?;
    check_assets(&records)?;
    let required = expected_manifest_paths();
    if !required.iter().all(|name| records.contains_key(name)) {
        return Err(bad(
            "Software stage fixed product or receipt set is incomplete",
        ));
    }
    let total_bytes = records.values().try_fold(0u64, |sum, (_, size)| {
        sum.checked_add(*size)
            .filter(|total| *total <= MAX_STAGE_BYTES)
            .ok_or_else(|| bad("Software stage total byte cap exceeded"))
    })?;
    if total_bytes == 0 || web_bytes == 0 || generated_bytes == 0 {
        return Err(bad("Software stage required payload is empty"));
    }
    if identity != ci_artifacts::identity(root, b)? || platform != current_platform()? {
        return Err(bad("Software stage source changed while staging"));
    }
    let manifest = make_manifest(platform, identity, fixture, &records, total_bytes);
    let mut raw =
        serde_json::to_vec(&manifest).map_err(|_| bad("Software stage JSON encode failed"))?;
    raw.push(b'\n');
    if raw.len() > ci_artifacts::META {
        return Err(bad("Software stage manifest exceeds metadata cap"));
    }
    if total_bytes
        .checked_add(raw.len() as u64)
        .is_none_or(|bytes| bytes > MAX_STAGE_BYTES)
    {
        return Err(bad("Software stage total byte cap exceeded"));
    }
    let manifest_path = stage_root.join("manifest.json");
    ci_artifacts::write_new(&manifest_path, &raw, b)?;
    let digest = sha_bytes(&raw);
    ci_artifacts::append(
        github_output,
        format!("software_stage_manifest_sha256={digest}\n").as_bytes(),
        b,
    )
}

fn sha_bytes(raw: &[u8]) -> String {
    let mut digest = Digest256Hasher::new();
    digest.update(raw);
    digest.finalize().to_hex()
}

fn hash_record_matches(value: &Value, digest: &str, size: u64) -> bool {
    value["sha256"].as_str() == Some(digest) && value["size_bytes"].as_u64() == Some(size)
}

fn expected_directories(records: &BTreeMap<String, (String, u64)>) -> io::Result<BTreeSet<String>> {
    let mut expected = BTreeSet::from([
        "".to_owned(),
        "images".to_owned(),
        "receipts".to_owned(),
        "fixture".to_owned(),
        "web".to_owned(),
        "web/dist".to_owned(),
        "worker-generated".to_owned(),
    ]);
    for path in records.keys() {
        let mut parent = Path::new(path).parent();
        while let Some(directory) = parent {
            if directory.as_os_str().is_empty() {
                break;
            }
            expected.insert(rel_string(directory)?);
            parent = directory.parent();
        }
    }
    Ok(expected)
}

fn actual_inventory(
    stage_root: &Path,
    b: &Budget<'_>,
) -> io::Result<(BTreeMap<String, (String, u64)>, BTreeSet<String>)> {
    let mut files = BTreeMap::new();
    let mut directories = BTreeSet::from([String::new()]);
    let mut pending = vec![(stage_root.to_path_buf(), PathBuf::new(), 0usize)];
    let mut total = 0u64;
    let mut directory_count = 0usize;
    let mut member_count = 0usize;
    while let Some((directory, relative_dir, depth)) = pending.pop() {
        b.check()?;
        if depth > 16 {
            return Err(bad("Software stage tree depth exceeded"));
        }
        let children = sorted_children(&directory, b)?;
        for child in children {
            b.check()?;
            let kind = child.file_type()?;
            if kind.is_symlink() {
                return Err(bad("Software stage refuses symlinks"));
            }
            let relative = relative_dir.join(child.file_name());
            if kind.is_dir() {
                directory_count += 1;
                if directory_count > MAX_STAGE_DIRECTORIES {
                    return Err(bad("Software stage directory cap exceeded"));
                }
                directories.insert(rel_string(&relative)?);
                pending.push((child.path(), relative, depth + 1));
            } else if kind.is_file() {
                member_count += 1;
                if member_count > MAX_STAGE_MEMBERS {
                    return Err(bad("Software stage member cap exceeded"));
                }
                let relative = rel_string(&relative)?;
                if relative == "manifest.json" {
                    continue;
                }
                let cap = file_cap(&relative)?;
                let metadata = fs::symlink_metadata(child.path())?;
                if metadata.len() > cap {
                    return Err(bad("Software stage file exceeds its byte cap"));
                }
                let digest = ci_artifacts::hash_capped(&child.path(), cap, b)?;
                let sha = digest["sha256"]
                    .as_str()
                    .ok_or_else(|| bad("Software stage hash missing"))?
                    .to_owned();
                let size = digest["size_bytes"]
                    .as_u64()
                    .ok_or_else(|| bad("Software stage size missing"))?;
                total = total
                    .checked_add(size)
                    .filter(|bytes| *bytes <= MAX_STAGE_BYTES)
                    .ok_or_else(|| bad("Software stage total byte cap exceeded"))?;
                add_record(&mut files, relative, sha, size)?;
            } else {
                return Err(bad("Software stage accepts only regular files"));
            }
        }
    }
    if files.len() != 0 {
        Ok((files, directories))
    } else {
        Err(bad("Software stage payload is empty"))
    }
}

fn validate_fixture_manifest(value: &Value, expected: &Value) -> io::Result<()> {
    if value != expected {
        return Err(bad("Software stage fixture profile mismatch"));
    }
    Ok(())
}

fn validate_manifest_authority(
    value: &Value,
    expected_identity: &Value,
    expected_platform: &Value,
) -> io::Result<()> {
    if value["source_identity"] != *expected_identity {
        return Err(bad("Software stage source identity mismatch"));
    }
    if value["platform"] != *expected_platform {
        return Err(bad("Software stage producer/consumer platform mismatch"));
    }
    Ok(())
}

fn exact_inventory(
    actual: &BTreeMap<String, (String, u64)>,
    expected: &BTreeMap<String, (String, u64)>,
) -> io::Result<()> {
    if actual != expected {
        return Err(bad("Software stage exact file inventory mismatch"));
    }
    Ok(())
}

fn validate_manifest(
    value: &Value,
    root: &Path,
    b: &Budget<'_>,
) -> io::Result<BTreeMap<String, (String, u64)>> {
    ci_artifacts::keys(
        value,
        &[
            "schema_version",
            "platform",
            "source_identity",
            "products",
            "fixture",
            "worker_rules",
            "files",
            "limits",
        ],
    )?;
    if value["schema_version"] != SCHEMA {
        return Err(bad("unsupported Software stage manifest schema"));
    }
    validate_manifest_authority(
        value,
        &ci_artifacts::identity(root, b)?,
        &current_platform()?,
    )?;
    if value["products"] != json!(product_names()) {
        return Err(bad("Software stage product roster mismatch"));
    }
    validate_fixture_manifest(&value["fixture"], &fixture_contract(root, b)?)?;
    if value["worker_rules"] != worker_rules_contract() {
        return Err(bad("Software stage worker rules profile mismatch"));
    }
    ci_artifacts::keys(
        &value["limits"],
        &[
            "max_members",
            "max_directories",
            "max_total_bytes",
            "max_single_image_bytes",
            "max_fixture_messages_bytes",
            "max_worker_generated_bytes",
            "producer_payload_bytes",
        ],
    )?;
    if value["limits"]["max_members"] != json!(MAX_STAGE_MEMBERS)
        || value["limits"]["max_directories"] != json!(MAX_STAGE_DIRECTORIES)
        || value["limits"]["max_total_bytes"] != json!(MAX_STAGE_BYTES)
        || value["limits"]["max_single_image_bytes"] != json!(ci_artifacts::IMAGE)
        || value["limits"]["max_fixture_messages_bytes"] != json!(ci_artifacts::CARGO_BYTES)
        || value["limits"]["max_worker_generated_bytes"] != json!(MAX_GENERATED_BYTES)
    {
        return Err(bad("Software stage manifest limit contract mismatch"));
    }
    let records = record_map(&value["files"])?;
    if !expected_manifest_paths()
        .iter()
        .all(|required| records.contains_key(required))
        || records.keys().any(|path| {
            !(path.starts_with("web/dist/")
                || path.starts_with("worker-generated/")
                || expected_manifest_paths().contains(path))
        })
    {
        return Err(bad("Software stage manifest exact product set mismatch"));
    }
    let generated_total = records
        .iter()
        .filter(|(path, _)| path.starts_with("worker-generated/"))
        .try_fold(0u64, |sum, (_, (_, size))| {
            sum.checked_add(*size)
                .filter(|bytes| *bytes <= MAX_GENERATED_BYTES)
                .ok_or_else(|| bad("Software stage generated files exceed byte cap"))
        })?;
    if generated_total == 0 {
        return Err(bad("Software stage generated worker files are empty"));
    }
    let declared_total = records.values().try_fold(0u64, |sum, (_, size)| {
        sum.checked_add(*size)
            .filter(|bytes| *bytes <= MAX_STAGE_BYTES)
            .ok_or_else(|| bad("Software stage total byte cap exceeded"))
    })?;
    if value["limits"]["producer_payload_bytes"] != json!(declared_total) {
        return Err(bad("Software stage payload byte count mismatch"));
    }
    // The external inventory count excludes the manifest itself; account for
    // its bounded bytes in the one-megabyte stage metadata allowance below.
    Ok(records)
}

fn create_destination_dir(path: &Path, must_be_empty: bool) -> io::Result<()> {
    if path.exists() {
        canonical_dir(path)?;
        if must_be_empty && fs::read_dir(path)?.next().is_some() {
            return Err(bad("Software stage destination directory must be empty"));
        }
    } else {
        secure_create_dirs(path)?;
        canonical_dir(path)?;
    }
    Ok(())
}

fn preflight_bind(
    root: &Path,
    image_root: &Path,
    receipt_root: &Path,
    github_env: &Path,
    github_path: &Path,
) -> io::Result<()> {
    if !image_root.ends_with(Path::new(ci_artifacts::TARGET).join("debug")) {
        return Err(bad("Software stage consumer image target/profile mismatch"));
    }
    ci_artifacts::absolute(receipt_root)?;
    ci_artifacts::absolute(github_env)?;
    ci_artifacts::absolute(github_path)?;
    ci_artifacts::absolute(root)?;
    for path in [
        receipt_root.join("tos-native-build.json"),
        receipt_root.join("tos-native-command-products.json"),
        receipt_root.join("software-fixture-cargo-messages.jsonl"),
    ] {
        if fs::symlink_metadata(path).is_ok() {
            return Err(bad("Software stage receipt destination already exists"));
        }
    }
    for name in COMMANDS {
        if fs::symlink_metadata(receipt_root.join(format!("{name}-build.json"))).is_ok() {
            return Err(bad("Software stage receipt destination already exists"));
        }
    }
    create_destination_dir(image_root, true)?;
    create_destination_dir(&root.join("access/web/dist"), true)?;
    create_destination_dir(
        &root.join("access/deploy/cloudflare-worker/generated"),
        true,
    )?;
    Ok(())
}

fn copy_verified(
    source_root: &Path,
    relative: &str,
    destination: &Path,
    records: &BTreeMap<String, (String, u64)>,
    b: &Budget<'_>,
) -> io::Result<()> {
    let expected = records
        .get(relative)
        .ok_or_else(|| bad("Software stage copied an undeclared file"))?;
    let source = stage_path(source_root, relative)?;
    let (digest, size) = copy_file(&source, destination, file_cap(relative)?, b)?;
    if digest != expected.0 || size != expected.1 {
        return Err(bad("Software stage file changed after authentication"));
    }
    Ok(())
}

fn copy_verified_tree(
    stage_root: &Path,
    source_prefix: &str,
    destination_root: &Path,
    records: &BTreeMap<String, (String, u64)>,
    b: &Budget<'_>,
) -> io::Result<()> {
    for relative in records
        .keys()
        .filter(|path| path.starts_with(source_prefix))
    {
        let subpath = relative
            .strip_prefix(source_prefix)
            .ok_or_else(|| bad("Software stage asset path mismatch"))?;
        if subpath.is_empty() || subpath.starts_with('/') {
            return Err(bad("Software stage asset path mismatch"));
        }
        let target = stage_path(destination_root, subpath)?;
        secure_create_dirs(target.parent().unwrap())?;
        copy_verified(stage_root, relative, &target, records, b)?;
    }
    Ok(())
}

fn rebind_descriptor(
    stage_root: &Path,
    image_root: &Path,
    receipt_root: &Path,
    b: &Budget<'_>,
) -> io::Result<Vec<u8>> {
    let source = stage_path(stage_root, "receipts/tos-native-command-products.json")?;
    let raw = ci_artifacts::bytes(&source, ci_artifacts::META, b)?;
    let mut products = ci_artifacts::strict(&raw, ci_artifacts::META)?;
    ci_artifacts::keys(&products, &COMMANDS)?;
    for name in COMMANDS {
        ci_artifacts::keys(&products[name], &["binary", "receipt"])?;
        if !products[name]["binary"].is_string() || !products[name]["receipt"].is_string() {
            return Err(bad("Software stage product descriptor paths missing"));
        }
        products[name]["binary"] = json!(image_root.join(name).to_string_lossy().into_owned());
        products[name]["receipt"] = json!(
            receipt_root
                .join(format!("{name}-build.json"))
                .to_string_lossy()
                .into_owned()
        );
    }
    let mut rebound = serde_json::to_vec(&products)
        .map_err(|_| bad("Software stage product descriptor encoding failed"))?;
    rebound.push(b'\n');
    if rebound.len() > ci_artifacts::META {
        return Err(bad(
            "Software stage product descriptor exceeds metadata cap",
        ));
    }
    Ok(rebound)
}

fn restore_executable(path: &Path, expected: &(String, u64), b: &Budget<'_>) -> io::Result<()> {
    let digest = ci_artifacts::hash(path, b)?;
    if digest["sha256"] != expected.0 || digest["size_bytes"] != expected.1 {
        return Err(bad(
            "Software stage executable differs after materialization",
        ));
    }
    set_mode(path, 0o755)?;
    Ok(())
}

fn stage_bind(flags: &BTreeMap<String, PathBuf>, b: &Budget<'_>) -> io::Result<()> {
    let root = path(flags, "--repo-root");
    let stage_root = path(flags, "--stage-root");
    let digest_arg = path(flags, "--manifest-sha256")
        .to_str()
        .ok_or_else(|| bad("Software stage manifest digest is not UTF-8"))?;
    let image_root = path(flags, "--image-root");
    let receipt_root = path(flags, "--receipt-root");
    let github_env = path(flags, "--github-env");
    let github_path = path(flags, "--github-path");
    ci_artifacts::absolute(root)?;
    canonical_dir(stage_root)?;
    if digest_arg.len() != 64
        || !digest_arg
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(bad(
            "Software stage manifest digest must be lowercase SHA-256",
        ));
    }
    let manifest_path = stage_root.join("manifest.json");
    let manifest_raw = ci_artifacts::bytes(&manifest_path, ci_artifacts::META, b)?;
    if sha_bytes(&manifest_raw) != digest_arg {
        return Err(bad("Software stage manifest digest mismatch"));
    }
    let manifest = ci_artifacts::strict(&manifest_raw, ci_artifacts::META)?;
    let records = validate_manifest(&manifest, root, b)?;
    let payload_bytes = records.values().try_fold(0u64, |sum, (_, size)| {
        sum.checked_add(*size)
            .filter(|bytes| *bytes <= MAX_STAGE_BYTES)
            .ok_or_else(|| bad("Software stage total byte cap exceeded"))
    })?;
    if payload_bytes
        .checked_add(manifest_raw.len() as u64)
        .is_none_or(|bytes| bytes > MAX_STAGE_BYTES)
    {
        return Err(bad("Software stage total byte cap exceeded"));
    }
    let (actual, directories) = actual_inventory(stage_root, b)?;
    exact_inventory(&actual, &records)?;
    let actual_dirs = directories
        .into_iter()
        .filter(|directory| !directory.is_empty())
        .collect::<BTreeSet<_>>();
    let expected_dirs = expected_directories(&records)?
        .into_iter()
        .filter(|directory| !directory.is_empty())
        .collect::<BTreeSet<_>>();
    if actual_dirs != expected_dirs {
        return Err(bad("Software stage exact directory inventory mismatch"));
    }
    check_assets(&records)?;
    // No paths are created until the external digest, source identity, file
    // set, byte limits and hashes have all passed.
    preflight_bind(root, image_root, receipt_root, github_env, github_path)?;

    for name in product_names() {
        let relative = format!("images/{name}");
        copy_verified(stage_root, &relative, &image_root.join(name), &records, b)?;
    }
    let fixture_relative = format!("fixture/{FIXTURE}");
    copy_verified(
        stage_root,
        &fixture_relative,
        &image_root.join(FIXTURE),
        &records,
        b,
    )?;
    for name in receipt_names() {
        if name == "tos-native-command-products.json" {
            continue;
        }
        let relative = format!("receipts/{name}");
        copy_verified(stage_root, &relative, &receipt_root.join(name), &records, b)?;
    }
    copy_verified(
        stage_root,
        FIXTURE_MESSAGES,
        &receipt_root.join("software-fixture-cargo-messages.jsonl"),
        &records,
        b,
    )?;
    let descriptor = rebind_descriptor(stage_root, image_root, receipt_root, b)?;
    ci_artifacts::write_new(
        &receipt_root.join("tos-native-command-products.json"),
        &descriptor,
        b,
    )?;
    copy_verified_tree(
        stage_root,
        "web/dist/",
        &root.join("access/web/dist"),
        &records,
        b,
    )?;
    copy_verified_tree(
        stage_root,
        "worker-generated/",
        &root.join("access/deploy/cloudflare-worker/generated"),
        &records,
        b,
    )?;
    if manifest["source_identity"] != ci_artifacts::identity(root, b)?
        || manifest["platform"] != current_platform()?
    {
        return Err(bad(
            "Software stage source or platform changed while binding",
        ));
    }

    for name in product_names().into_iter().chain([FIXTURE]) {
        let relative = if name == FIXTURE {
            format!("fixture/{FIXTURE}")
        } else {
            format!("images/{name}")
        };
        restore_executable(
            &image_root.join(name),
            records
                .get(&relative)
                .ok_or_else(|| bad("Software stage executable receipt missing"))?,
            b,
        )?;
    }
    ci_artifacts::append_software_environment(github_env, image_root, b)?;
    let image = image_root.display();
    let env = format!(
        "TOS_SCHEMA_WORKER_PATH={image}/tos-schema-worker\nTOS_NATIVE_OWNER_COMMAND_PATH={image}/tos-native-owner-command\nTOS_E2E_FIXTURE_BIN={image}/{FIXTURE}\n"
    );
    ci_artifacts::append(github_env, env.as_bytes(), b)?;
    ci_artifacts::append(
        github_path,
        format!("{}\n", image_root.display()).as_bytes(),
        b,
    )?;
    Ok(())
}

pub(crate) fn run(mode: &str, args: &[String], cancel: &AtomicI32) -> io::Result<()> {
    let b = Budget::new(cancel, STAGE_TIMEOUT);
    let allowed = match mode {
        "software-stage-manifest" => &[
            "--repo-root",
            "--stage-root",
            "--image-root",
            "--receipt-root",
            "--fixture-messages",
            "--rustc-version",
            "--web-dist",
            "--worker-generated",
            "--github-output",
        ][..],
        "software-stage-bind" => &[
            "--repo-root",
            "--stage-root",
            "--manifest-sha256",
            "--image-root",
            "--receipt-root",
            "--github-env",
            "--github-path",
        ][..],
        _ => return Err(bad("unknown Software stage operation")),
    };
    let flags = parse_flags(args, allowed)?;
    match mode {
        "software-stage-manifest" => stage_manifest(&flags, &b),
        "software-stage-bind" => stage_bind(&flags, &b),
        _ => Err(bad("unknown Software stage operation")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_profile() -> Value {
        json!({
            "name": FIXTURE,
            "package": "tos-access",
            "default_features": true,
            "features": ["e2e-fixture", "source-owner", "source-tools"],
            "target": ci_artifacts::TARGET,
            "profile": "debug",
            "opt_level": "0",
            "debug_assertions": true,
            "debuginfo": 0,
            "test": false
        })
    }

    #[test]
    fn fixture_manifest_profile_is_exact() {
        let expected = fixture_profile();
        assert!(validate_fixture_manifest(&expected, &expected).is_ok());
        let mut changed = expected.clone();
        changed["profile"] = json!("release");
        assert!(validate_fixture_manifest(&changed, &expected).is_err());
    }

    #[test]
    fn manifest_authority_rejects_source_and_platform_drift() {
        let identity = json!({"source_commit":"a","source_tree":"b","lock_sha256":"c"});
        let platform = json!({"os":"linux","arch":"x86_64","id":"ubuntu","version_id":"26.04"});
        let value = json!({"source_identity":identity,"platform":platform});
        assert!(validate_manifest_authority(&value, &identity, &platform).is_ok());
        assert!(
            validate_manifest_authority(
                &value,
                &json!({"source_commit":"d","source_tree":"b","lock_sha256":"c"}),
                &platform
            )
            .is_err()
        );
        assert!(
            validate_manifest_authority(
                &value,
                &identity,
                &json!({"os":"linux","arch":"aarch64","id":"ubuntu","version_id":"26.04"})
            )
            .is_err()
        );
    }

    #[test]
    fn stage_inventory_rejects_tampered_extra_and_missing_files() {
        let digest = "a".repeat(64);
        let declared = ci_artifacts::strict(
            format!(
                "[{{\"path\":\"images/tos-access\",\"sha256\":\"{digest}\",\"size_bytes\":64}}]"
            )
            .as_bytes(),
            ci_artifacts::META,
        )
        .unwrap();
        let expected = record_map(&declared).unwrap();
        assert!(exact_inventory(&expected, &expected).is_ok());

        let tampered = BTreeMap::from([("images/tos-access".to_owned(), ("b".repeat(64), 64))]);
        assert!(exact_inventory(&tampered, &expected).is_err());

        let extra = BTreeMap::from([
            ("images/tos-access".to_owned(), (digest.clone(), 64)),
            ("images/unexpected".to_owned(), (digest.clone(), 64)),
        ]);
        assert!(exact_inventory(&extra, &expected).is_err());

        let missing = BTreeMap::new();
        assert!(exact_inventory(&missing, &expected).is_err());
    }
}
