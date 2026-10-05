//! Native software-only ZIP v1 assembly. No data selection or code execution.
//! Receipt integrity is not build admission; OPS supplies the actual products.
#[path = "software_installed.rs"]
pub mod installed;

use rawzip::{FileReader, ZipArchive, ZipArchiveEntryWayfinder};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
use tos_foundation::{
    CanonicalProfile, Digest256, Digest256Hasher, JsonLimits, JsonMode, JsonNumber, JsonNumberKind,
    JsonString, JsonValue, RelativePath, canonical_bytes_v1, parse_json,
};
use zip::{CompressionMethod, DateTime, ZipWriter, write::SimpleFileOptions};

const PROGRAM: &str = "access/src/tos_access/tos-access";
const COMMAND_SCHEMA: &str = "tos_native_software_command_build_v1";
const COMMANDS: [&str; 8] = [
    "tos-native-owner-command",
    "tos-schema-worker",
    "tos-validation-lanes",
    "tos-release-check",
    "tos-software-ci",
    "tos-ops-mechanics-plan",
    "tos-constructor-library",
    "tos-constructor-fragments",
];
fn command_member(name: &str) -> String {
    format!("native/bin/{name}")
}
fn executable_member(name: &str) -> bool {
    name == PROGRAM || COMMANDS.iter().any(|role| name == command_member(role))
}
const MANIFEST: &str = "software.manifest.json";
const MANIFEST_BYTES: usize = 1_048_576;
const STATIC: &str = "access/src/tos_access/web_dist/";
const TARGET: &str = "x86_64-unknown-linux-gnu";
const NATIVE_SCHEMA: &str = "tos_native_access_build_v1";
const SCHEMA: &str = "tos_software_bundle_manifest_v1";
const SOURCE_FILES: [&str; 1] = ["access/README.md"];
const TOS_SCHEMAS: [&str; 3] = [
    "semantic-entity-type-registry.schema.json",
    "semantic-relation-type-registry.schema.json",
    "epistemic-evidence-projection.schema.json",
];
const README: &str = "# Tree of Sophia software package\n\nNative Linux x86_64 software is the verified member\n`access/src/tos_access/tos-access`; it needs no Python runtime.\nRun that member with `--help`, or install this archive into a fresh user prefix\nwith `software install --archive ABSOLUTE_ARCHIVE --prefix ABSOLUTE_PREFIX`\nand the explicit total/archive/member/metadata budgets documented in access/README.md.\nThe installed entrypoint is PREFIX/bin/tos; invoke its absolute path explicitly.\nAn optional verified command cohort installs selected verified PREFIX/bin command links.\nOwner command execution still requires a protected explicit invocation and grants.\nSelect managed data separately with `--release-root ABSOLUTE_RELEASE`.\nNo selected data means truthful unavailable data capabilities.\n\nThis native archive contains no Python runtime or wheel backend.\nThe repository retains explicit LEGACY Python reference compatibility with\n`pip install ./access`, whose command is `tos-legacy`; it is not installed\nfrom this archive. Neither installation carries corpus data.\n";
type Result<T> = std::result::Result<T, String>;
trait Checked<T> {
    fn checked(self) -> Result<T>;
}
impl<T, E: std::fmt::Display> Checked<T> for std::result::Result<T, E> {
    fn checked(self) -> Result<T> {
        self.map_err(|e| e.to_string())
    }
}
#[derive(Clone, Copy)]
pub struct ArchiveLimits {
    pub max_total_bytes: u64,
    pub max_archive_bytes: u64,
    pub max_members: usize,
    /// Caller metadata budget: bounded buffers + retained names/typed records.
    /// This is structural accounting, not an allocator/RSS guarantee.
    pub max_metadata_bytes: usize,
}
impl ArchiveLimits {
    fn validate(self) -> Result<()> {
        if self.max_total_bytes == 0
            || self.max_archive_bytes == 0
            || self.max_members == 0
            // The locator probes a fixed 56-byte ZIP64 header. Each caller
            // buffer receives one quarter of the metadata operation budget.
            || self.max_metadata_bytes / 4 < 56
        {
            return Err("finite positive archive limits required".into());
        }
        Ok(())
    }
}
type Identity = (u64, u64, u64, i64, i64, i64, i64);
fn identity(file: &File) -> Result<Identity> {
    let m = file.metadata().checked()?;
    if !m.is_file() {
        return Err("software member is not a regular file".into());
    }
    Ok((
        m.dev(),
        m.ino(),
        m.len(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    ))
}
fn text(s: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(s))
}
fn number(n: u64) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: n.to_string(),
    })
}
fn object(fields: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        fields
            .into_iter()
            .map(|(k, v)| (JsonString::from_utf8(k), v))
            .collect(),
    )
}
fn field<'a>(v: &'a JsonValue, key: &str) -> Result<&'a JsonValue> {
    v.object_get(key)
        .ok_or_else(|| format!("missing software field {key}"))
}
fn string<'a>(v: &'a JsonValue, key: &str) -> Result<&'a str> {
    field(v, key)?
        .as_str()
        .ok_or_else(|| format!("invalid software string {key}"))
}
fn uint(v: &JsonValue, key: &str) -> Result<u64> {
    field(v, key)?
        .as_u64()
        .ok_or_else(|| format!("invalid software size {key}"))
}
fn json(raw: &[u8], cap: usize) -> Result<JsonValue> {
    parse_json(
        raw,
        JsonMode::PublishedStrict,
        JsonLimits {
            max_bytes: cap,
            max_depth: 32,
            max_visits: 100_000,
            max_integer_digits: 20,
        },
    )
    .checked()
    .map(|d| d.into_root())
}
fn encode(value: &JsonValue, cap: usize) -> Result<Vec<u8>> {
    canonical_bytes_v1(
        value,
        CanonicalProfile::SourceRecordDigestV1,
        JsonLimits {
            max_bytes: cap,
            max_depth: 32,
            max_visits: 100_000,
            max_integer_digits: 20,
        },
    )
    .checked()
}
fn sidecar(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(".manifest.json");
    PathBuf::from(s)
}
fn open_file(path: &Path, cap: u64) -> Result<File> {
    tos_fd_open::open_absolute_regular(path, cap).checked()
}
fn read_small(f: &mut File, cap: usize) -> Result<Vec<u8>> {
    let before = identity(f)?;
    if before.2 > cap as u64 {
        return Err("software document exceeds byte bound".into());
    }
    f.seek(SeekFrom::Start(0)).checked()?;
    let mut bytes = Vec::new();
    (&mut *f)
        .take(cap as u64 + 1)
        .read_to_end(&mut bytes)
        .checked()?;
    if bytes.len() > cap || before != identity(f)? {
        return Err("software document changed".into());
    }
    Ok(bytes)
}
fn hash(f: &mut File, expected: u64) -> Result<Digest256> {
    f.seek(SeekFrom::Start(0)).checked()?;
    let mut sha = Digest256Hasher::new();
    let mut count = 0u64;
    let mut buffer = [0u8; 65536];
    loop {
        let n = f.read(&mut buffer).checked()?;
        if n == 0 {
            break;
        }
        count = count
            .checked_add(n as u64)
            .filter(|n| *n <= expected)
            .ok_or("software stream exceeds declared size")?;
        sha.update(&buffer[..n]);
    }
    if count != expected {
        return Err("software stream size differs".into());
    }
    Ok(sha.finalize())
}
fn path_ok(name: &str) -> bool {
    !name.contains(['\\', ':', '\0'])
        && RelativePath::parse(name).is_ok()
        && Path::new(name).to_str() == Some(name)
}
fn allowed(name: &str) -> bool {
    if !path_ok(name) {
        return false;
    }
    if executable_member(name) {
        return true;
    }
    if [
        PROGRAM,
        MANIFEST,
        "Cargo.lock",
        "rust-toolchain.toml",
        "README.md",
        "access/README.md",
    ]
    .contains(&name)
    {
        return true;
    }
    if let Some(rest) = name.strip_prefix("access/src/tos_access/runtime_data/") {
        return ((rest.starts_with("access/contracts/") || rest.starts_with("access/profiles/"))
            && rest.ends_with(".json"))
            || TOS_SCHEMAS
                .iter()
                .any(|s| rest == format!("ToS/contracts/{s}"));
    }
    name.starts_with(STATIC)
        || ((name.starts_with("access/contracts/") || name.starts_with("access/profiles/"))
            && name.ends_with(".json"))
}
fn child(root: &File, name: &str) -> Result<File> {
    if !path_ok(name) {
        return Err("invalid software member path".into());
    }
    let mut dir = tos_fd_open::reopen_directory(root).checked()?;
    let mut pieces = name.split('/').peekable();
    while let Some(piece) = pieces.next() {
        if pieces.peek().is_none() {
            return tos_fd_open::open_regular_at(&dir, Path::new(piece)).checked();
        }
        dir = tos_fd_open::open_directory_at(&dir, Path::new(piece)).checked()?;
    }
    Err("empty software member path".into())
}
fn directory(root: &File, name: &str) -> Result<File> {
    if !name.is_empty() && !path_ok(name) {
        return Err("invalid software directory".into());
    }
    let mut dir = tos_fd_open::reopen_directory(root).checked()?;
    if name.is_empty() {
        return Ok(dir);
    }
    for p in name.split('/') {
        dir = tos_fd_open::open_directory_at(&dir, Path::new(p)).checked()?;
    }
    Ok(dir)
}
fn walk(
    root: &File,
    relative: &str,
    paths: &mut Vec<String>,
    limits: ArchiveLimits,
    path_bytes: &mut usize,
    exclude_package: bool,
    depth: usize,
    visited: &mut usize,
) -> Result<()> {
    if depth > 32 {
        return Err("software source nesting exceeds bound".into());
    }
    let dir = directory(root, relative)?;
    // Enumerate the retained directory, not a mutable ancestor pathname.
    let entries = fs::read_dir(format!("/proc/self/fd/{}", dir.as_raw_fd())).checked()?;
    for entry in entries {
        let entry = entry.checked()?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "non-UTF8 software filename")?;
        if exclude_package && ["runtime_data", "__pycache__"].contains(&name.as_str()) {
            continue;
        }
        *visited = visited
            .checked_add(1)
            .filter(|n| *n <= limits.max_members)
            .ok_or("software source traversal exceeds member bound")?;
        let p = if relative.is_empty() {
            name
        } else {
            format!("{relative}/{name}")
        };
        if !path_ok(&p) {
            return Err("invalid software source path".into());
        }
        let kind = entry.file_type().checked()?;
        if kind.is_symlink() || (!kind.is_dir() && !kind.is_file()) {
            return Err("non-regular software source member".into());
        }
        if paths.len() >= limits.max_members {
            return Err("software source member count exceeds cap".into());
        }
        if kind.is_dir() {
            walk(
                root,
                &p,
                paths,
                limits,
                path_bytes,
                exclude_package,
                depth + 1,
                visited,
            )?;
        } else {
            *path_bytes = path_bytes
                .checked_add(p.len() + std::mem::size_of::<String>())
                .filter(|n| *n <= limits.max_metadata_bytes / 4)
                .ok_or("software traversal path metadata exceeds budget")?;
            paths.push(p);
        }
    }
    Ok(())
}
fn git(root: &File, args: &[&str]) -> Result<String> {
    // The child changes directory through this parent's retained descriptor.
    // Git discovers the actual checkout from that directory, preserving its
    // normal subdirectory and linked-worktree semantics after a path rename.
    let cwd = format!("/proc/{}/fd/{}", std::process::id(), root.as_raw_fd());
    let mut process = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_COMMON_DIR")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .checked()?;
    let mut raw = Vec::new();
    let Some(stdout) = process.stdout.take() else {
        let _ = process.kill();
        let _ = process.wait();
        return Err("git stdout absent".into());
    };
    if let Err(error) = stdout.take(65_537).read_to_end(&mut raw) {
        let _ = process.kill();
        let _ = process.wait();
        return Err(error.to_string());
    }
    if raw.len() > 65_536 {
        let _ = process.kill();
        let _ = process.wait();
        return Err("software Git status exceeds bounded output".into());
    }
    let status = match process.wait() {
        Ok(status) => status,
        Err(error) => {
            let _ = process.kill();
            let _ = process.wait();
            return Err(error.to_string());
        }
    };
    if !status.success() {
        return Err("software Git command failed".into());
    }
    String::from_utf8(raw)
        .checked()
        .map(|s| s.trim().to_owned())
}
fn source(root: &File) -> Result<(String, String)> {
    let head = git(root, &["rev-parse", "HEAD"])?;
    let tree = git(root, &["rev-parse", "HEAD^{tree}"])?;
    let status = git(
        root,
        &[
            "status",
            "--porcelain=v1",
            "--untracked-files=all",
            "--",
            "Cargo.toml",
            "Cargo.lock",
            "rust-toolchain.toml",
            "rust",
            "access/packaging",
            "access/pyproject.toml",
            "access/README.md",
            ":(glob)access/src/tos_access/**/*.py",
            ":(exclude,glob)access/src/tos_access/**/runtime_data/**",
            ":(exclude,glob)access/src/tos_access/**/__pycache__/**",
            ":(glob)access/contracts/**/*.json",
            ":(glob)access/profiles/**/*.json",
            "access/web/src",
            "access/web/public",
            "access/web/index.html",
            "access/web/research.html",
            ":(glob)access/web/package*.json",
            "access/web/tsconfig.json",
            "access/web/vite.config.ts",
            "ToS/contracts/semantic-entity-type-registry.schema.json",
            "ToS/contracts/semantic-relation-type-registry.schema.json",
            "ToS/contracts/epistemic-evidence-projection.schema.json",
        ],
    )?;
    if !status.is_empty() {
        return Err("native software requires clean source closure".into());
    }
    Ok((head, tree))
}
fn proof(p: &JsonValue, source_ref: &str) -> Result<()> {
    proof_kind(p, source_ref, None)
}
fn proof_kind(p: &JsonValue, source_ref: &str, role: Option<&str>) -> Result<()> {
    let command = role.is_some();
    if role.is_some_and(|name| !COMMANDS.contains(&name)) {
        return Err("unsupported native command proof role".into());
    }
    let mut keys = vec![
        "schema_version",
        "sha256",
        "size_bytes",
        "target",
        "source_commit",
        "source_tree",
        "lock_sha256",
        "toolchain",
        "profile",
    ];
    if command {
        keys.push("features");
    }
    let fields = p.as_object().ok_or("native proof must be object")?;
    if fields.len() != keys.len()
        || fields
            .iter()
            .any(|(k, _)| !keys.contains(&k.as_str().unwrap_or("")))
    {
        return Err("native proof fields differ".into());
    }
    if string(p, "schema_version")?
        != if command {
            COMMAND_SCHEMA
        } else {
            NATIVE_SCHEMA
        }
        || string(p, "target")? != TARGET
        || string(p, "source_commit")? != source_ref
        || !["debug", "release"].contains(&string(p, "profile")?)
        || uint(p, "size_bytes")? < 64
    {
        return Err("native proof profile/source differs".into());
    }
    if let Some(role) = role {
        let expected: &[&str] = if role == "tos-ops-mechanics-plan" {
            &["compiler-backed-validators", "default"]
        } else {
            &[]
        };
        if !field(p, "features")?.as_array().is_some_and(|features| {
            features.len() == expected.len()
                && features
                    .iter()
                    .zip(expected)
                    .all(|(actual, expected)| actual.as_str() == Some(*expected))
        }) {
            return Err("native command role effective feature set differs".into());
        }
    }
    for key in ["sha256", "lock_sha256"] {
        Digest256::from_hex(string(p, key)?).checked()?;
    }
    for key in ["source_commit", "source_tree"] {
        let v = string(p, key)?;
        if ![40, 64].contains(&v.len())
            || !v
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err("invalid native Git identity".into());
        }
    }
    let version = string(p, "toolchain")?;
    if version.split('.').count() != 3
        || version
            .split('.')
            .any(|p| p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()))
    {
        return Err("invalid native toolchain version".into());
    }
    Ok(())
}
pub(crate) fn command_proof(role: &str, p: &JsonValue, access: &JsonValue) -> Result<()> {
    proof_kind(p, string(access, "source_commit")?, Some(role))?;
    for key in ["source_tree", "lock_sha256", "toolchain", "target"] {
        if field(p, key)? != field(access, key)? {
            return Err("native commands must match the exact access source cohort".into());
        }
    }
    Ok(())
}
fn command_closure<'a>(
    manifest: &'a JsonValue,
    access: &JsonValue,
) -> Result<Option<&'a JsonValue>> {
    let Some(commands) = manifest.object_get("native_commands") else {
        return Ok(None);
    };
    let roles = commands
        .as_object()
        .ok_or("native command closure must be object")?;
    if roles.is_empty()
        || roles.len() > COMMANDS.len()
        || roles
            .iter()
            .any(|(name, _)| !COMMANDS.contains(&name.as_str().unwrap_or("")))
    {
        return Err(
            "native command closure must contain a nonempty subset of supported roles".into(),
        );
    }
    for role in COMMANDS {
        if let Some(proof) = commands.object_get(role) {
            command_proof(role, proof, access)?;
        }
    }
    Ok(Some(commands))
}
fn header(f: &mut File) -> Result<()> {
    f.seek(SeekFrom::Start(0)).checked()?;
    let mut h = [0u8; 64];
    f.read_exact(&mut h).checked()?;
    if &h[..7] != b"\x7fELF\x02\x01\x01" || h[18..20] != [0x3e, 0] {
        return Err("native artifact is not x86_64 ELF64".into());
    }
    Ok(())
}
fn toolchain(raw: &[u8]) -> Result<String> {
    // rust-toolchain.toml is a document; Value::from_str parses one TOML value.
    let parsed = toml::from_str::<toml::Value>(std::str::from_utf8(raw).checked()?).checked()?;
    parsed
        .get("toolchain")
        .and_then(|v| v.get("channel"))
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
        .ok_or("software toolchain channel absent or invalid".into())
}

struct Input {
    name: String,
    file: Option<File>,
    generated: Option<&'static [u8]>,
    identity: Option<Identity>,
    size: u64,
    sha: Digest256,
}
fn add(
    inputs: &mut BTreeMap<String, Input>,
    name: String,
    mut f: File,
    limits: ArchiveLimits,
    total: &mut u64,
    metadata: &mut usize,
) -> Result<()> {
    if !allowed(&name) || inputs.len() >= limits.max_members || inputs.contains_key(&name) {
        return Err("invalid/duplicate/excess software member".into());
    }
    *metadata = metadata
        .checked_add(
            name.len()
                .checked_mul(2)
                .ok_or("input name size overflow")?,
        )
        .and_then(|n| n.checked_add(std::mem::size_of::<(String, Input)>()))
        .filter(|n| *n <= limits.max_metadata_bytes)
        .ok_or("software retained input metadata exceeds structural budget")?;
    let id = identity(&f)?;
    if name.starts_with(STATIC) && id.2 > crate::site::MAX_STATIC_BYTES as u64 {
        return Err("static member exceeds delivery cap".into());
    }
    let admitted = total
        .checked_add(id.2)
        .filter(|n| *n <= limits.max_total_bytes)
        .ok_or("software input total exceeds byte budget before hashing")?;
    let sha = hash(&mut f, id.2)?;
    if identity(&f)? != id {
        return Err("software input changed during hashing".into());
    }
    inputs.insert(
        name.clone(),
        Input {
            name,
            file: Some(f),
            generated: None,
            identity: Some(id),
            size: id.2,
            sha,
        },
    );
    *total = admitted;
    Ok(())
}
fn create(path: &Path) -> Result<File> {
    if !path.is_absolute() {
        return Err("software output must be absolute".into());
    }
    let parent = tos_fd_open::open_absolute_directory(path.parent().ok_or("output parent absent")?)
        .checked()?;
    let leaf = path
        .file_name()
        .and_then(|p| p.to_str())
        .ok_or("invalid output leaf")?;
    if !path_ok(leaf) {
        return Err("invalid output leaf".into());
    }
    OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .custom_flags(0x20000)
        .open(format!("/proc/self/fd/{}/{leaf}", parent.as_raw_fd()))
        .checked()
}

struct ArchiveOutput {
    file: File,
    position: u64,
    cap: u64,
}
impl Write for ArchiveOutput {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let end = self
            .position
            .checked_add(bytes.len() as u64)
            .filter(|n| *n <= self.cap)
            .ok_or_else(|| std::io::Error::other("archive output byte budget exceeded"))?;
        let n = self.file.write(bytes)?;
        self.position = end - (bytes.len() - n) as u64;
        Ok(n)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}
impl Seek for ArchiveOutput {
    fn seek(&mut self, from: SeekFrom) -> std::io::Result<u64> {
        let next = self.file.seek(from)?;
        if next > self.cap {
            return Err(std::io::Error::other("archive seek exceeds budget"));
        }
        self.position = next;
        Ok(next)
    }
}

pub fn build(
    root: &Path,
    web_dist: &Path,
    output: &Path,
    source_ref: &str,
    binary: &Path,
    receipt: &Path,
    limits: ArchiveLimits,
) -> Result<JsonValue> {
    build_with_commands(
        root, web_dist, output, source_ref, binary, receipt, None, limits,
    )
}
pub fn build_with_commands(
    root: &Path,
    web_dist: &Path,
    output: &Path,
    source_ref: &str,
    binary: &Path,
    receipt: &Path,
    command_products: Option<&Path>,
    limits: ArchiveLimits,
) -> Result<JsonValue> {
    limits.validate()?;
    for path in [output.to_path_buf(), sidecar(output)] {
        match fs::symlink_metadata(path) {
            Ok(_) => return Err("software output/sidecar already exists".into()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e.to_string()),
        }
    }
    let root_fd = tos_fd_open::open_absolute_directory(root).checked()?;
    let (head, tree) = source(&root_fd)?;
    if head != source_ref {
        return Err("source_ref differs from HEAD".into());
    }
    let mut proof_file = open_file(receipt, 65_536)?;
    let p = json(&read_small(&mut proof_file, 65_536)?, 65_536)?;
    proof(&p, &head)?;
    let mut image = open_file(binary, limits.max_total_bytes)?;
    let image_id = identity(&image)?;
    if image.metadata().checked()?.permissions().mode() & 0o111 == 0 {
        return Err("native input is not executable".into());
    }
    header(&mut image)?;
    if string(&p, "source_tree")? != tree || uint(&p, "size_bytes")? != image_id.2 {
        return Err("native image/tree receipt differs".into());
    }
    let lock = child(&root_fd, "Cargo.lock")?;
    let mut pin = child(&root_fd, "rust-toolchain.toml")?;
    let pin_bytes = read_small(&mut pin, 8192)?;
    if string(&p, "toolchain")? != toolchain(&pin_bytes)? {
        return Err("native toolchain receipt differs".into());
    }
    let mut inputs = BTreeMap::new();
    let mut total = 0u64;
    let mut input_metadata = 0usize;
    for name in SOURCE_FILES {
        add(
            &mut inputs,
            name.into(),
            child(&root_fd, name)?,
            limits,
            &mut total,
            &mut input_metadata,
        )?;
    }
    for (dir, extension, exclude) in [
        ("access/contracts", ".json", false),
        ("access/profiles", ".json", false),
    ] {
        let mut paths = Vec::new();
        walk(
            &root_fd, dir, &mut paths, limits, &mut 0, exclude, 0, &mut 0,
        )?;
        let mut admitted = 0;
        for path in paths.into_iter().filter(|p| p.ends_with(extension)) {
            let name = path.clone();
            add(
                &mut inputs,
                name,
                child(&root_fd, &path)?,
                limits,
                &mut total,
                &mut input_metadata,
            )?;
            admitted += 1;
        }
        if admitted == 0 {
            return Err("required software directory is empty".into());
        }
    }
    let web_root = tos_fd_open::open_absolute_directory(web_dist).checked()?;
    let mut web_paths = Vec::new();
    walk(
        &web_root,
        "",
        &mut web_paths,
        limits,
        &mut 0,
        false,
        0,
        &mut 0,
    )?;
    for path in web_paths {
        add(
            &mut inputs,
            format!("{STATIC}{path}"),
            child(&web_root, &path)?,
            limits,
            &mut total,
            &mut input_metadata,
        )?;
    }
    for schema in TOS_SCHEMAS {
        let path = format!("ToS/contracts/{schema}");
        add(
            &mut inputs,
            format!("access/src/tos_access/runtime_data/{path}"),
            child(&root_fd, &path)?,
            limits,
            &mut total,
            &mut input_metadata,
        )?;
    }
    let original = inputs
        .keys()
        .filter(|p| p.starts_with("access/contracts/") || p.starts_with("access/profiles/"))
        .cloned()
        .collect::<Vec<_>>();
    for path in original {
        let f = inputs[&path].file.as_ref().unwrap().try_clone().checked()?;
        add(
            &mut inputs,
            format!("access/src/tos_access/runtime_data/{path}"),
            f,
            limits,
            &mut total,
            &mut input_metadata,
        )?;
    }
    add(
        &mut inputs,
        PROGRAM.into(),
        image,
        limits,
        &mut total,
        &mut input_metadata,
    )?;
    add(
        &mut inputs,
        "Cargo.lock".into(),
        lock,
        limits,
        &mut total,
        &mut input_metadata,
    )?;
    add(
        &mut inputs,
        "rust-toolchain.toml".into(),
        pin,
        limits,
        &mut total,
        &mut input_metadata,
    )?;
    if inputs[PROGRAM].sha.to_hex() != string(&p, "sha256")?
        || inputs["Cargo.lock"].sha.to_hex() != string(&p, "lock_sha256")?
    {
        return Err("native executable/lock receipt differs from retained inputs".into());
    }
    let mut command_proofs = Vec::new();
    if let Some(products) = command_products {
        let mut descriptor = open_file(products, 65_536)?;
        let descriptors = json(&read_small(&mut descriptor, 65_536)?, 65_536)?;
        input_metadata = input_metadata
            .checked_add(65_536 * 3 + std::mem::size_of::<JsonValue>() * 40)
            .filter(|n| *n <= limits.max_metadata_bytes)
            .ok_or("native command selector metadata exceeds budget")?;
        let roles = descriptors
            .as_object()
            .ok_or("native command products must be object")?;
        if roles.is_empty()
            || roles.len() > COMMANDS.len()
            || roles
                .iter()
                .any(|(name, _)| !COMMANDS.contains(&name.as_str().unwrap_or("")))
        {
            return Err(
                "native command products must select a nonempty subset of supported roles".into(),
            );
        }
        for role in COMMANDS {
            let Some(selected) = descriptors.object_get(role) else {
                continue;
            };
            let fields = selected
                .as_object()
                .ok_or("native command product must be object")?;
            if fields.len() != 2
                || fields
                    .iter()
                    .any(|(key, _)| !["binary", "receipt"].contains(&key.as_str().unwrap_or("")))
            {
                return Err("native command product requires binary and receipt paths only".into());
            }
            if !Path::new(string(selected, "binary")?).is_absolute()
                || !Path::new(string(selected, "receipt")?).is_absolute()
            {
                return Err("native command product paths must be absolute".into());
            }
            let mut receipt = open_file(Path::new(string(selected, "receipt")?), 65_536)?;
            let proof = json(&read_small(&mut receipt, 65_536)?, 65_536)?;
            command_proof(role, &proof, &p)?;
            input_metadata = input_metadata
                .checked_add(
                    encode(&proof, 65_536)?
                        .len()
                        .checked_mul(3)
                        .ok_or("native proof metadata overflow")?
                        + std::mem::size_of::<JsonValue>() * 40,
                )
                .filter(|n| *n <= limits.max_metadata_bytes)
                .ok_or("native command proof metadata exceeds budget")?;
            let mut image = open_file(
                Path::new(string(selected, "binary")?),
                limits.max_total_bytes,
            )?;
            if image.metadata().checked()?.permissions().mode() & 0o111 == 0 {
                return Err("native command input is not executable".into());
            }
            header(&mut image)?;
            let member = command_member(role);
            add(
                &mut inputs,
                member.clone(),
                image,
                limits,
                &mut total,
                &mut input_metadata,
            )?;
            if inputs[&member].size != uint(&proof, "size_bytes")?
                || inputs[&member].sha.to_hex() != string(&proof, "sha256")?
            {
                return Err("native command executable differs from its build receipt".into());
            }
            command_proofs.push((role, proof));
        }
    }
    total = total
        .checked_add(README.len() as u64)
        .filter(|n| *n <= limits.max_total_bytes)
        .ok_or("software README exceeds remaining total budget")?;
    inputs.insert(
        "README.md".into(),
        Input {
            name: "README.md".into(),
            file: None,
            generated: Some(README.as_bytes()),
            identity: None,
            size: README.len() as u64,
            sha: Digest256::of_bytes(README.as_bytes()),
        },
    );
    for required in ["assets/tos-graph.js", "assets/tos-graph.css"] {
        if !inputs.contains_key(&format!("{STATIC}{required}")) {
            return Err("native software required web asset absent".into());
        }
    }
    if total > limits.max_total_bytes || inputs.len() + 1 > limits.max_members {
        return Err("software closure exceeds declared budget".into());
    }
    let writer_metadata = inputs
        .values()
        .try_fold(input_metadata, |n, m| {
            n.checked_add(
                m.name.len().checked_mul(3)?
                    + 64 * 2 // The two owned digest hex strings in embedded/sidecar rows.
                    + std::mem::size_of::<JsonValue>() * 14
                    + std::mem::size_of::<(JsonString, JsonValue)>() * 6,
            )
        })
        .filter(|n| *n <= limits.max_metadata_bytes)
        .ok_or("software manifest/writer metadata exceeds structural budget")?;
    let _ = writer_metadata;
    let members = inputs
        .values()
        .map(|m| {
            object(vec![
                ("path", text(&m.name)),
                ("sha256", text(&m.sha.to_hex())),
                ("size_bytes", number(m.size)),
            ])
        })
        .collect();
    let mut manifest_fields = vec![
        ("schema_version", text(SCHEMA)),
        ("software_ref", text(&head)),
        ("source_dirty", JsonValue::Bool(false)),
        ("data_included", JsonValue::Bool(false)),
        ("members", JsonValue::Array(members)),
        ("native_access", p),
    ];
    if !command_proofs.is_empty() {
        manifest_fields.push(("native_commands", object(command_proofs)));
    }
    let manifest = object(manifest_fields);
    let manifest_bytes = encode(&manifest, MANIFEST_BYTES)?;
    if total
        .checked_add(manifest_bytes.len() as u64)
        .is_none_or(|n| n > limits.max_total_bytes)
    {
        return Err("manifest-inclusive software budget exceeded".into());
    }
    // No whole staging copy: retained regular FDs stream straight into ZIP.
    let output_file = create(output)?;
    let mut writer = ZipWriter::new(ArchiveOutput {
        file: output_file,
        position: 0,
        cap: limits.max_archive_bytes,
    });
    let mut names = inputs.keys().cloned().collect::<Vec<_>>();
    names.push(MANIFEST.into());
    names.sort();
    for name in names {
        let size = if name == MANIFEST {
            manifest_bytes.len() as u64
        } else {
            inputs[&name].size
        };
        let mode = if executable_member(&name) {
            0o755
        } else {
            0o644
        };
        let opts = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .compression_level(Some(9))
            .last_modified_time(DateTime::from_date_and_time(2020, 1, 1, 0, 0, 0).checked()?)
            .unix_permissions(mode)
            .large_file(size >= 0x7fff_ffff);
        writer.start_file(&name, opts).checked()?;
        if name == MANIFEST {
            writer.write_all(&manifest_bytes).checked()?;
        } else {
            let m = inputs.get_mut(&name).unwrap();
            if let Some(raw) = m.generated {
                writer.write_all(raw).checked()?;
            } else {
                let f = m.file.as_mut().unwrap();
                if identity(f)? != m.identity.unwrap() {
                    return Err("software input changed before copy".into());
                }
                f.seek(SeekFrom::Start(0)).checked()?;
                let mut count = 0u64;
                let mut sha = Digest256Hasher::new();
                let mut buf = [0u8; 65536];
                loop {
                    let n = f.read(&mut buf).checked()?;
                    if n == 0 {
                        break;
                    }
                    count = count
                        .checked_add(n as u64)
                        .filter(|n| *n <= m.size)
                        .ok_or("software copy exceeds expected size")?;
                    sha.update(&buf[..n]);
                    writer.write_all(&buf[..n]).checked()?;
                }
                if count != m.size || sha.finalize() != m.sha || identity(f)? != m.identity.unwrap()
                {
                    return Err("software input changed during copy".into());
                }
            }
        }
    }
    let mut archive_file = writer.finish().checked()?.file;
    archive_file.sync_all().checked()?;
    let archive_size = identity(&archive_file)?.2;
    if archive_size > limits.max_archive_bytes {
        return Err("compressed software archive exceeds budget".into());
    }
    // Current Git and all retained source identities must still hold at finish.
    if source(&root_fd)? != (head.clone(), tree) {
        return Err("software source changed during assembly".into());
    }
    for m in inputs.values() {
        if let Some(f) = &m.file {
            if identity(f)? != m.identity.unwrap() {
                return Err("software retained input changed at finish".into());
            }
        }
    }
    let archive_sha = hash(&mut archive_file, archive_size)?;
    let mut external = manifest.clone();
    let JsonValue::Object(fields) = &mut external else {
        unreachable!()
    };
    fields.push((
        JsonString::from_utf8("archive_sha256"),
        text(&archive_sha.to_hex()),
    ));
    fields.push((
        JsonString::from_utf8("archive_size_bytes"),
        number(archive_size),
    ));
    let bytes = encode(&external, MANIFEST_BYTES + 1024)?;
    let mut external_file = create(&sidecar(output))?;
    external_file.write_all(&bytes).checked()?;
    external_file.sync_all().checked()?;
    Ok(external)
}

#[derive(Clone)]
struct Member {
    size: u64,
    sha: Digest256,
    mode: u32,
    method: rawzip::CompressionMethod,
    wayfinder: ZipArchiveEntryWayfinder,
}
pub struct VerifiedArchive {
    archive: ZipArchive<FileReader>,
    file: File,
    identity: Identity,
    sha: Digest256,
    manifest: JsonValue,
    members: BTreeMap<String, Member>,
    limits: ArchiveLimits,
}
// Every emitted byte is charged before passing it to the consumer. The final
// nonempty chunk and EOF both pass through rawzip's CRC/exact-size verifier.
fn consume_verified<D: Read>(
    mut reader: rawzip::ZipVerifier<D>,
    size: u64,
    out: &mut impl Write,
) -> Result<(Digest256, D)> {
    let mut count = 0u64;
    let mut digest = Digest256Hasher::new();
    let mut buf = [0u8; 65536];
    loop {
        let remaining = size - count;
        let allowance = remaining.saturating_add(1).min(buf.len() as u64) as usize;
        let n = reader.read(&mut buf[..allowance]).checked()?;
        if n == 0 {
            break;
        }
        count = count
            .checked_add(n as u64)
            .filter(|n| *n <= size)
            .ok_or("expanded software member exceeds exact size before output")?;
        digest.update(&buf[..n]);
        out.write_all(&buf[..n]).checked()?;
    }
    if count != size {
        return Err("expanded software member is truncated".into());
    }
    Ok((digest.finalize(), reader.into_inner()))
}
fn consume_member(
    archive: &ZipArchive<FileReader>,
    member: &Member,
    out: &mut impl Write,
) -> Result<Digest256> {
    let entry = archive.get_entry(member.wayfinder).checked()?;
    if member.method == rawzip::CompressionMethod::DEFLATE {
        let decoder = flate2::read::DeflateDecoder::new(entry.reader());
        let (digest, decoder) =
            consume_verified(entry.verifying_reader(decoder), member.size, out)?;
        if decoder.total_in() != member.wayfinder.compressed_size_hint() {
            return Err("DEFLATE stream does not consume exact compressed range".into());
        }
        Ok(digest)
    } else {
        consume_verified(entry.verifying_reader(entry.reader()), member.size, out).map(|p| p.0)
    }
}
struct Prefix {
    bytes: Vec<u8>,
    cap: usize,
}
impl Write for Prefix {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let n = bytes.len().min(self.cap - self.bytes.len());
        self.bytes.extend_from_slice(&bytes[..n]);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn zip_document(
    archive: &ZipArchive<FileReader>,
    member: &Member,
    cap: usize,
) -> Result<JsonValue> {
    if member.size > cap as u64 {
        return Err("software archive JSON exceeds cap".into());
    }
    let mut bytes = Vec::new();
    consume_member(archive, member, &mut bytes)?;
    json(&bytes, cap)
}
impl VerifiedArchive {
    pub fn open(path: &Path, limits: ArchiveLimits) -> Result<Self> {
        limits.validate()?;
        let mut file = open_file(path, limits.max_archive_bytes)?;
        let id = identity(&file)?;
        let mut external_file = open_file(&sidecar(path), (MANIFEST_BYTES + 1024) as u64)?;
        let external = json(
            &read_small(&mut external_file, MANIFEST_BYTES + 1024)?,
            MANIFEST_BYTES + 1024,
        )?;
        let sha = hash(&mut file, id.2)?;
        if uint(&external, "archive_size_bytes")? != id.2
            || string(&external, "archive_sha256")? != sha.to_hex()
        {
            return Err("archive sidecar identity differs".into());
        }
        // Locator and iterator borrow finite caller buffers; neither allocates
        // central records from an untrusted declared count. No masked reader.
        let buffer_bytes =
            (limits.max_metadata_bytes / 4).min(rawzip::MAX_CENTRAL_DIRECTORY_RECORD_SIZE);
        let mut central_buffer = vec![0u8; buffer_bytes];
        let mut local_buffer = vec![0u8; buffer_bytes];
        let archive = rawzip::ZipLocator::new()
            .max_search_space(limits.max_archive_bytes)
            .locate_in_file(file.try_clone().checked()?, &mut central_buffer)
            .map_err(|(_, e)| e.to_string())?;
        if archive.end_offset() != id.2
            || archive.entries_hint() == 0
            || archive.entries_hint() > limits.max_members as u64
            || id
                .2
                .checked_sub(archive.directory_offset())
                .is_none_or(|n| n > limits.max_metadata_bytes as u64)
        {
            return Err("software ZIP locator/count/metadata byte budget differs".into());
        }
        let mut metadata = buffer_bytes * 2;
        let mut members = BTreeMap::<String, Member>::new();
        let mut entries = archive.entries(&mut central_buffer);
        let mut expanded_total = 0u64;
        let mut ranges = BTreeMap::<u64, u64>::new();
        while let Some(header) = entries.next_entry().checked()? {
            if members.len() >= limits.max_members {
                return Err("ZIP member budget exceeded".into());
            }
            let raw_path = header.file_path();
            let name = std::str::from_utf8(raw_path.as_ref()).checked()?;
            let flags = header.flags();
            let method = header.compression_method();
            if !allowed(name)
                || header.is_dir()
                || members.contains_key(name)
                || flags.is_encrypted()
                || flags.has_strong_encryption()
                || flags.is_masked()
                || flags.bits() & (1 << 5) != 0 // Maintained ZIP patched-data refusal.
                || ![
                    rawzip::CompressionMethod::STORE,
                    rawzip::CompressionMethod::DEFLATE,
                ]
                .contains(&method)
            {
                return Err("unsafe/non-software/duplicate ZIP member".into());
            }
            let mode = header.external_attributes() >> 16;
            let expected_mode = if executable_member(&name) {
                0o755
            } else {
                0o644
            };
            if ![0, 0o100000].contains(&(mode & 0o170000)) || mode & 0o7777 != expected_mode {
                return Err("software member regular/executable mode differs".into());
            }
            let size = header.uncompressed_size_hint();
            expanded_total = expanded_total
                .checked_add(size)
                .filter(|n| *n <= limits.max_total_bytes)
                .ok_or("software expanded total exceeds budget before decompression")?;
            if (name == MANIFEST && size > MANIFEST_BYTES as u64)
                || (name.starts_with(STATIC) && size > crate::site::MAX_STATIC_BYTES as u64)
            {
                return Err("software member exceeds delivery/document cap".into());
            }
            // The explicit typed/name charge precedes owned metadata allocation.
            // BTree node/allocator overhead is still not an RSS guarantee.
            metadata = metadata
                .checked_add(name.len())
                .and_then(|n| {
                    n.checked_add(
                        std::mem::size_of::<(String, Member)>() + std::mem::size_of::<(u64, u64)>(),
                    )
                })
                .filter(|n| *n <= limits.max_metadata_bytes)
                .ok_or("retained ZIP metadata exceeds structural budget")?;
            let wayfinder = header.wayfinder();
            let entry = archive.get_entry(wayfinder).checked()?;
            let local = entry.local_header(&mut local_buffer).checked()?;
            if local.file_path().as_ref() != raw_path.as_ref()
                || local.flags() != flags
                || local.compression_method() != method
            {
                return Err("local/central ZIP path/flags/method mismatch".into());
            }
            if !flags.has_data_descriptor()
                && (local.crc32() != header.crc32()
                    || local.uncompressed_size_hint() != size
                    || local.compressed_size_hint() != header.compressed_size_hint())
            {
                return Err("local/central ZIP checksum/size mismatch".into());
            }
            if let Some(descriptor) = entry.reader().data_descriptor().checked()? {
                if descriptor.crc32() != header.crc32()
                    || descriptor.uncompressed_size() != size
                    || descriptor.compressed_size() != header.compressed_size_hint()
                {
                    return Err("ZIP descriptor/central checksum/size mismatch".into());
                }
            }
            let (data_start, data_end) = entry.compressed_data_range();
            let local_start = header.local_header_offset();
            if local_start >= data_start
                || data_start > data_end
                || data_end > archive.directory_offset()
                || ranges
                    .range(..=local_start)
                    .next_back()
                    .is_some_and(|(_, end)| *end > local_start)
                || ranges
                    .range(local_start..)
                    .next()
                    .is_some_and(|(start, _)| *start < data_end)
            {
                return Err("ZIP local/data range outside payload or overlapping".into());
            }
            ranges.insert(local_start, data_end);
            members.insert(
                name.to_owned(),
                Member {
                    size,
                    sha: Digest256::of_bytes(&[]),
                    mode: expected_mode,
                    method,
                    wayfinder,
                },
            );
        }
        if members.len() as u64 != archive.entries_hint() || identity(&file)? != id {
            return Err("ZIP actual central count or retained identity differs".into());
        }
        drop(entries);
        drop(ranges);
        drop(central_buffer);
        drop(local_buffer);
        let manifest = zip_document(
            &archive,
            members.get(MANIFEST).ok_or("software manifest absent")?,
            MANIFEST_BYTES,
        )?;
        if string(&manifest, "schema_version")? != SCHEMA
            || field(&manifest, "data_included")? != &JsonValue::Bool(false)
            || field(&manifest, "source_dirty")? != &JsonValue::Bool(false)
        {
            return Err("unsupported native software manifest".into());
        }
        let ref_id = string(&manifest, "software_ref")?;
        let native = field(&manifest, "native_access")?;
        proof(native, ref_id)?;
        let commands = command_closure(&manifest, native)?;
        let embedded = manifest.as_object().ok_or("manifest is not object")?;
        let outer = external.as_object().ok_or("sidecar is not object")?;
        if embedded.len() != if commands.is_some() { 7 } else { 6 }
            || outer.len() != embedded.len() + 2
            || embedded
                .iter()
                .any(|(k, v)| external.object_get(k.as_str().unwrap_or("")) != Some(v))
        {
            return Err("embedded/external software manifests differ".into());
        }
        let rows = field(&manifest, "members")?
            .as_array()
            .ok_or("software members absent")?;
        if rows.len() + 1 != members.len() {
            return Err("software manifest member closure differs".into());
        }
        let mut declared = BTreeMap::new();
        let mut total = 0u64;
        let mut last = "";
        for row in rows {
            let name = string(row, "path")?;
            if !allowed(name)
                || name == MANIFEST
                || name <= last
                || row.as_object().is_none_or(|f| f.len() != 3)
            {
                return Err("software member path/order/fields invalid".into());
            }
            last = name;
            let size = uint(row, "size_bytes")?;
            total = total
                .checked_add(size)
                .filter(|n| *n <= limits.max_total_bytes)
                .ok_or("software uncompressed closure exceeds cap")?;
            if name.starts_with(STATIC) && size > crate::site::MAX_STATIC_BYTES as u64 {
                return Err("static member exceeds software delivery cap".into());
            }
            let digest = Digest256::from_hex(string(row, "sha256")?).checked()?;
            metadata = metadata
                .checked_add(std::mem::size_of::<(&str, (u64, Digest256))>())
                .filter(|n| *n <= limits.max_metadata_bytes)
                .ok_or("declared ZIP metadata exceeds structural budget")?;
            declared.insert(name, (size, digest));
        }
        let mut elf_header = Prefix {
            bytes: Vec::new(),
            cap: 64,
        };
        let mut command_headers = BTreeMap::new();
        for role in COMMANDS {
            command_headers.insert(
                command_member(role),
                Prefix {
                    bytes: Vec::new(),
                    cap: 64,
                },
            );
        }
        let mut pin_bytes = Vec::new();
        for (name, member) in &mut members {
            let expected = if name == MANIFEST {
                total = total
                    .checked_add(member.size)
                    .filter(|n| *n <= limits.max_total_bytes)
                    .ok_or("manifest-inclusive software budget exceeded")?;
                None
            } else {
                let (size, sha) = declared.get(name.as_str()).ok_or("undeclared ZIP member")?;
                if *size != member.size {
                    return Err("declared ZIP member size differs".into());
                }
                Some(*sha)
            };
            member.sha = if name == PROGRAM {
                consume_member(&archive, member, &mut elf_header)?
            } else if let Some(header) = command_headers.get_mut(name) {
                consume_member(&archive, member, header)?
            } else if name == "rust-toolchain.toml" {
                if member.size > 8192 {
                    return Err("software toolchain pin oversized".into());
                }
                consume_member(&archive, member, &mut pin_bytes)?
            } else {
                consume_member(&archive, member, &mut std::io::sink())?
            };
            if expected.is_some_and(|s| s != member.sha) {
                return Err("software member SHA differs".into());
            }
        }
        for required in [PROGRAM, "Cargo.lock", "rust-toolchain.toml"] {
            if !declared.contains_key(required) {
                return Err("native proof member closure absent".into());
            }
        }
        for asset in ["assets/tos-graph.js", "assets/tos-graph.css"] {
            if !declared.contains_key(format!("{STATIC}{asset}").as_str()) {
                return Err("native software required asset absent".into());
            }
        }
        let image = declared.get(PROGRAM).unwrap();
        if image.0 != uint(native, "size_bytes")?
            || image.1.to_hex() != string(native, "sha256")?
            || declared["Cargo.lock"].1.to_hex() != string(native, "lock_sha256")?
        {
            return Err("native executable/lock proof differs".into());
        }
        if elf_header.bytes.len() != 64
            || &elf_header.bytes[..7] != b"\x7fELF\x02\x01\x01"
            || elf_header.bytes[18..20] != [0x3e, 0]
        {
            return Err("archive native image is not x86_64 ELF64".into());
        }
        for role in COMMANDS {
            let member = command_member(role);
            if let Some(proof) = commands.and_then(|commands| commands.object_get(role)) {
                let image = declared
                    .get(member.as_str())
                    .ok_or("native command role member absent")?;
                if image.0 != uint(proof, "size_bytes")?
                    || image.1.to_hex() != string(proof, "sha256")?
                {
                    return Err("native command role member proof differs".into());
                }
                let header = &command_headers[&member].bytes;
                if header.len() != 64
                    || &header[..7] != b"\x7fELF\x02\x01\x01"
                    || header[18..20] != [0x3e, 0]
                {
                    return Err("archive native command image is not x86_64 ELF64".into());
                }
            } else if declared.contains_key(member.as_str()) {
                return Err("native command member requires its selected role proof".into());
            }
        }
        if toolchain(&pin_bytes)? != string(native, "toolchain")? {
            return Err("archive native toolchain proof differs".into());
        }
        let mut verified = Self {
            archive,
            file,
            identity: id,
            sha,
            manifest,
            members,
            limits,
        };
        verified.recheck()?;
        Ok(verified)
    }
    fn recheck(&mut self) -> Result<()> {
        if identity(&self.file)? != self.identity
            || hash(&mut self.file, self.identity.2)? != self.sha
            || identity(&self.file)? != self.identity
        {
            return Err("retained software archive changed".into());
        }
        Ok(())
    }
    pub fn manifest(&self) -> &JsonValue {
        &self.manifest
    }
    pub fn extract(&mut self, destination: &Path) -> Result<JsonValue> {
        self.recheck()?;
        if !destination.is_absolute() {
            return Err("fresh extraction target must be absolute".into());
        }
        let parent = tos_fd_open::open_absolute_directory(
            destination.parent().ok_or("extraction parent absent")?,
        )
        .checked()?;
        let leaf = destination
            .file_name()
            .and_then(|p| p.to_str())
            .ok_or("extraction leaf invalid")?;
        if !path_ok(leaf) {
            return Err("extraction leaf invalid".into());
        }
        self.extract_at(&parent, leaf).map(|(report, _)| report)
    }
    fn extract_at(&mut self, parent: &File, leaf: &str) -> Result<(JsonValue, File)> {
        fs::create_dir(format!("/proc/self/fd/{}/{leaf}", parent.as_raw_fd())).checked()?;
        let root = tos_fd_open::open_directory_at(parent, Path::new(leaf)).checked()?;
        for (name, member) in &self.members {
            let mut directory = tos_fd_open::reopen_directory(&root).checked()?;
            let mut parts = name.split('/').peekable();
            while let Some(part) = parts.next() {
                let path = format!("/proc/self/fd/{}/{part}", directory.as_raw_fd());
                if parts.peek().is_some() {
                    match fs::create_dir(&path) {
                        Ok(()) => (),
                        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
                        Err(e) => return Err(e.to_string()),
                    }
                    directory =
                        tos_fd_open::open_directory_at(&directory, Path::new(part)).checked()?;
                } else {
                    let mut out = OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .custom_flags(0x20000)
                        .open(path)
                        .checked()?;
                    let sha = consume_member(&self.archive, member, &mut out)?;
                    if sha != member.sha {
                        return Err("retained member bytes changed during extraction".into());
                    }
                    out.set_permissions(fs::Permissions::from_mode(member.mode))
                        .checked()?;
                    out.sync_all().checked()?;
                }
            }
        }
        root.sync_all().checked()?;
        self.recheck()?;
        let selected = tos_fd_open::open_directory_at(parent, Path::new(leaf)).checked()?;
        let retained = root.metadata().checked()?;
        let current = selected.metadata().checked()?;
        if retained.dev() != current.dev() || retained.ino() != current.ino() {
            return Err("extraction directory changed during assembly".into());
        }
        Ok((
            object(vec![
                ("extracted", JsonValue::Bool(true)),
                (
                    "software_ref",
                    text(string(&self.manifest, "software_ref")?),
                ),
                ("data_included", JsonValue::Bool(false)),
                ("native_wheel_entry", JsonValue::Bool(false)),
                ("max_total_bytes", number(self.limits.max_total_bytes)),
            ]),
            root,
        ))
    }
    /// A fresh software-only user prefix. No PATH edits, data selection or cleanup.
    pub fn install(&mut self, prefix: &Path) -> Result<JsonValue> {
        self.recheck()?;
        if !prefix.is_absolute() {
            return Err("fresh installation prefix must be absolute".into());
        }
        let parent = tos_fd_open::open_absolute_directory(
            prefix.parent().ok_or("installation parent absent")?,
        )
        .checked()?;
        let leaf = prefix
            .file_name()
            .and_then(|p| p.to_str())
            .filter(|p| path_ok(p))
            .ok_or("installation prefix leaf invalid")?;
        fs::create_dir(format!("/proc/self/fd/{}/{leaf}", parent.as_raw_fd())).checked()?;
        let root = tos_fd_open::open_directory_at(&parent, Path::new(leaf)).checked()?;
        // Extraction consumes this same verified archive object. On any error,
        // the owned incomplete prefix remains inspectable; no install/startup success.
        let (extracted, software) = self.extract_at(&root, "software")?;
        fs::create_dir(format!("/proc/self/fd/{}/bin", root.as_raw_fd())).checked()?;
        let bin = tos_fd_open::open_directory_at(&root, Path::new("bin")).checked()?;
        let mut links = vec![("tos", PROGRAM.to_owned())];
        if let Some(commands) = self.manifest.object_get("native_commands") {
            for role in COMMANDS {
                if commands.object_get(role).is_some() {
                    links.push((role, command_member(role)));
                }
            }
        }
        for (role, member) in &links {
            std::os::unix::fs::symlink(
                format!("../software/{member}"),
                format!("/proc/self/fd/{}/{role}", bin.as_raw_fd()),
            )
            .checked()?;
        }
        bin.sync_all().checked()?;
        root.sync_all().checked()?;
        parent.sync_all().checked()?;
        self.recheck()?;
        let selected_bin = tos_fd_open::open_directory_at(&root, Path::new("bin")).checked()?;
        let retained_bin = bin.metadata().checked()?;
        let current_bin = selected_bin.metadata().checked()?;
        if retained_bin.dev() != current_bin.dev() || retained_bin.ino() != current_bin.ino() {
            return Err("installed entrypoint changed during assembly".into());
        }
        for (role, member) in &links {
            if fs::read_link(format!("/proc/self/fd/{}/{role}", selected_bin.as_raw_fd()))
                .checked()?
                != PathBuf::from(format!("../software/{member}"))
            {
                return Err("installed role entrypoint changed during assembly".into());
            }
        }
        let selected_software =
            tos_fd_open::open_directory_at(&root, Path::new("software")).checked()?;
        let retained_software = software.metadata().checked()?;
        let current_software = selected_software.metadata().checked()?;
        if retained_software.dev() != current_software.dev()
            || retained_software.ino() != current_software.ino()
        {
            return Err("installed software directory changed during assembly".into());
        }
        let selected = tos_fd_open::open_directory_at(&parent, Path::new(leaf)).checked()?;
        let retained = root.metadata().checked()?;
        let current = selected.metadata().checked()?;
        if retained.dev() != current.dev() || retained.ino() != current.ino() {
            return Err("installation prefix changed during assembly".into());
        }
        Ok(object(vec![
            ("installed", JsonValue::Bool(true)),
            ("software_ref", text(string(&extracted, "software_ref")?)),
            ("entrypoint", text("bin/tos")),
            ("data_included", JsonValue::Bool(false)),
            ("native_wheel_entry", JsonValue::Bool(false)),
        ]))
    }
}

/// Local software tooling runs before data selection and never starts a server.
pub fn run_if_requested(
    args: &[String],
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> Option<i32> {
    if args.first().is_none_or(|a| a != "software") {
        return None;
    }
    let result = (|| {
        let action = args
            .get(1)
            .ok_or("usage: software build|verify|extract|install OPTIONS")?;
        let mut options = BTreeMap::new();
        let mut at = 2;
        while at < args.len() {
            let name = args[at]
                .strip_prefix("--")
                .ok_or("expected software named option")?;
            let value = args.get(at + 1).ok_or("software option value missing")?;
            if options.insert(name.to_owned(), value.clone()).is_some() {
                return Err("duplicate software option".into());
            }
            at += 2;
        }
        let required = |name: &str| {
            options
                .get(name)
                .cloned()
                .ok_or_else(|| format!("required software option --{name}"))
        };
        let total = required("max-total-bytes")?.parse::<u64>().checked()?;
        let archive = required("max-archive-bytes")?.parse::<u64>().checked()?;
        let members = required("max-members")?.parse::<usize>().checked()?;
        let metadata = required("max-metadata-bytes")?.parse::<usize>().checked()?;
        let limits = ArchiveLimits {
            max_total_bytes: total,
            max_archive_bytes: archive,
            max_members: members,
            max_metadata_bytes: metadata,
        };
        limits.validate()?;
        let keys: &[&str] = match action.as_str() {
            "build" => &[
                "root",
                "web-dist",
                "output",
                "source-ref",
                "native-access-binary",
                "native-access-receipt",
                "native-command-products",
            ],
            "verify" => &["archive"],
            "extract" => &["archive", "destination"],
            "install" => &["archive", "prefix"],
            _ => return Err("unknown software action".into()),
        };
        if options.keys().any(|k| {
            !keys.contains(&k.as_str())
                && ![
                    "max-total-bytes",
                    "max-archive-bytes",
                    "max-members",
                    "max-metadata-bytes",
                ]
                .contains(&k.as_str())
        }) {
            return Err("unknown software option".into());
        }
        let value = match action.as_str() {
            "build" => build_with_commands(
                Path::new(&required("root")?),
                Path::new(&required("web-dist")?),
                Path::new(&required("output")?),
                &required("source-ref")?,
                Path::new(&required("native-access-binary")?),
                Path::new(&required("native-access-receipt")?),
                options
                    .get("native-command-products")
                    .map(|path| Path::new(path)),
                limits,
            )?,
            "verify" => VerifiedArchive::open(Path::new(&required("archive")?), limits)?
                .manifest()
                .clone(),
            "extract" => {
                let destination = required("destination")?;
                match fs::symlink_metadata(&destination) {
                    Ok(_) => return Err("fresh extraction destination already exists".into()),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                    Err(e) => return Err(e.to_string()),
                }
                VerifiedArchive::open(Path::new(&required("archive")?), limits)?
                    .extract(Path::new(&destination))?
            }
            "install" => {
                let prefix = required("prefix")?;
                match fs::symlink_metadata(&prefix) {
                    Ok(_) => return Err("fresh installation prefix already exists".into()),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                    Err(e) => return Err(e.to_string()),
                }
                VerifiedArchive::open(Path::new(&required("archive")?), limits)?
                    .install(Path::new(&prefix))?
            }
            _ => unreachable!(),
        };
        let bytes = encode(&value, MANIFEST_BYTES + 1024)?;
        stdout.write_all(&bytes).checked()?;
        stdout.write_all(b"\n").checked()?;
        Ok::<(), String>(())
    })();
    Some(match result {
        Ok(()) => 0,
        Err(error) => {
            let _ = writeln!(stderr, "software_archive_refused: {error}");
            1
        }
    })
}

#[cfg(test)]
mod role_feature_tests {
    use super::*;

    fn proof_with_features(features: &str) -> JsonValue {
        let raw = format!(
            r#"{{"schema_version":"tos_native_software_command_build_v1","sha256":"{digest}","size_bytes":64,"target":"x86_64-unknown-linux-gnu","source_commit":"{commit}","source_tree":"{commit}","lock_sha256":"{digest}","toolchain":"1.98.1","profile":"release","features":{features}}}"#,
            digest = "a".repeat(64),
            commit = "b".repeat(40),
        );
        json(raw.as_bytes(), 4096).unwrap()
    }

    #[test]
    fn constructor_roles_use_the_standard_empty_feature_proof() {
        let empty = proof_with_features("[]");
        let phi = proof_with_features(r#"["compiler-backed-validators","default"]"#);
        for role in ["tos-constructor-library", "tos-constructor-fragments"] {
            assert!(command_proof(role, &empty, &empty).is_ok());
            assert!(command_proof(role, &phi, &empty).is_err());
        }
    }

    #[test]
    fn native_role_features_are_exact_and_do_not_widen_other_commands() {
        let empty = proof_with_features("[]");
        let phi = proof_with_features(r#"["compiler-backed-validators","default"]"#);
        assert!(command_proof("tos-ops-mechanics-plan", &phi, &empty).is_ok());
        assert!(command_proof("tos-ops-mechanics-plan", &empty, &empty).is_err());
        for role in COMMANDS {
            if role != "tos-ops-mechanics-plan" {
                assert!(command_proof(role, &empty, &empty).is_ok());
                assert!(command_proof(role, &phi, &empty).is_err());
            }
        }
        for features in [
            r#"["compiler-backed-validators"]"#,
            r#"["default","compiler-backed-validators"]"#,
            r#"["compiler-backed-validators","default","wasm"]"#,
        ] {
            let wrong = proof_with_features(features);
            assert!(command_proof("tos-ops-mechanics-plan", &wrong, &empty).is_err());
        }
        assert!(command_proof("unknown-role", &empty, &empty).is_err());
    }
}
