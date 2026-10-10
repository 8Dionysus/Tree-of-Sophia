//! Local immutable KAG publication. Authored source and the selected KAG owner's
//! semantic validators retain their authority; this module binds their bytes.
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::{Duration, Instant};
use tos_foundation::{CanonicalProfile, Digest256Hasher, JsonLimits, canonical_raw_bytes_v1};

pub const SCHEMA: &str = "tos_kag_integration_v1";
pub const PRIMARY: &str =
    "ToS/canon/source/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/node.json";
pub const PROGRAM_PATHS: [&str; 5] = [
    "scripts/build_repo_local_kag_release.py",
    "scripts/query_repo_local_kag.py",
    "scripts/validate_repo_local_kag_family.py",
    "scripts/validators/local_kag_subtree.py",
    "scripts/validators/repo_local_kag_index.py",
];
// Published v1 integrations made before the owner exposed its probe CLI bind
// these four programs. Reading those immutable releases needs no interpreter.
const HISTORICAL_PROGRAM_PATHS: [&str; 4] = [
    "scripts/build_repo_local_kag_release.py",
    "scripts/query_repo_local_kag.py",
    "scripts/validators/local_kag_subtree.py",
    "scripts/validators/repo_local_kag_index.py",
];
// The source export is separately limited to 8 MiB. These limits cover the
// selected owner's generated carriers and every verification/copy pass.
pub const MAX_MANIFEST_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_MEMBERS: usize = 100_000;
pub const MAX_ARTIFACT_BYTES: u64 = 1024 * 1024 * 1024;
static CANCELLED: AtomicI32 = AtomicI32::new(0);
thread_local! { static DEADLINE: std::cell::Cell<Option<Instant>> = const { std::cell::Cell::new(None) }; }
pub fn cancel(signal: i32) {
    CANCELLED.store(signal, Ordering::Relaxed);
}
pub(crate) fn budget_check() -> io::Result<()> {
    if CANCELLED.load(Ordering::Relaxed) != 0
        || DEADLINE.with(|d| d.get().is_some_and(|end| Instant::now() >= end))
    {
        return Err(invalid("KAG operation cancelled or expired"));
    }
    Ok(())
}
pub(crate) struct WholeBudget {
    owns_deadline: bool,
}
impl WholeBudget {
    /// The original current operation's deadline, never a renewed allowance.
    pub(crate) fn deadline(&self) -> io::Result<Instant> {
        budget_check()?;
        DEADLINE
            .with(|d| d.get())
            .ok_or_else(|| invalid("KAG operation has no active deadline"))
    }
    pub(crate) fn begin() -> io::Result<Self> {
        budget_check()?;
        let owns_deadline = DEADLINE.with(|d| {
            if d.get().is_some() {
                false
            } else {
                d.set(Some(Instant::now() + Duration::from_secs(600)));
                true
            }
        });
        Ok(Self { owns_deadline })
    }
}
impl Drop for WholeBudget {
    fn drop(&mut self) {
        if self.owns_deadline {
            DEADLINE.with(|d| d.set(None));
        }
    }
}

pub(crate) fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}
pub fn canonical(value: &Value) -> io::Result<Vec<u8>> {
    let raw = serde_json::to_vec(value).map_err(|e| invalid(e.to_string()))?;
    canonical_raw_bytes_v1(
        &raw,
        CanonicalProfile::CorpusSnapshotV1,
        JsonLimits {
            max_bytes: MAX_MANIFEST_BYTES,
            ..JsonLimits::default()
        },
    )
    .map_err(|e| invalid(e.to_string()))
}
pub(crate) fn strict_json(raw: &[u8]) -> io::Result<Value> {
    // Canonicalization first rejects decoded duplicate fields and nonfinite JSON.
    canonical_raw_bytes_v1(
        raw,
        CanonicalProfile::CorpusSnapshotV1,
        JsonLimits {
            max_bytes: MAX_MANIFEST_BYTES,
            ..JsonLimits::default()
        },
    )
    .map_err(|e| invalid(e.to_string()))?;
    serde_json::from_slice(raw).map_err(|e| invalid(e.to_string()))
}
pub(crate) fn keys(value: &Value, expected: &[&str]) -> io::Result<()> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("expected JSON object"))?;
    if object.len() != expected.len() || expected.iter().any(|key| !object.contains_key(*key)) {
        return Err(invalid("unexpected JSON field set"));
    }
    Ok(())
}
pub(crate) fn hex(value: &Value) -> io::Result<&str> {
    let value = value
        .as_str()
        .ok_or_else(|| invalid("digest must be a string"))?;
    if value.len() != 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(invalid("digest must be lowercase SHA-256"));
    }
    Ok(value)
}
pub(crate) fn safe_absolute(path: &Path) -> io::Result<PathBuf> {
    budget_check()?;
    if path
        .components()
        .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
    {
        return Err(invalid("path contains traversal segments"));
    }
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut current = PathBuf::new();
    for component in path.components() {
        budget_check()?;
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(invalid("path contains symlink"));
            }
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => break,
            Err(e) => return Err(e),
        }
    }
    Ok(path)
}
pub(crate) fn relative(value: &Value) -> io::Result<&str> {
    let value = value
        .as_str()
        .ok_or_else(|| invalid("member path must be string"))?;
    if value.is_empty()
        || value.contains('\\')
        || value.chars().any(|c| (c as u32) < 32)
        || value.starts_with('/')
        || value
            .split('/')
            .any(|c| c.is_empty() || matches!(c, "." | ".." | ".git"))
    {
        return Err(invalid("member path is not normalized"));
    }
    Ok(value)
}
pub(crate) fn regular(path: &Path) -> io::Result<fs::Metadata> {
    safe_absolute(path)?;
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err(invalid("expected regular file"));
    }
    Ok(meta)
}
pub(crate) fn directory(path: &Path) -> io::Result<()> {
    safe_absolute(path)?;
    if !fs::symlink_metadata(path)?.is_dir() {
        return Err(invalid("expected regular directory"));
    }
    Ok(())
}
pub(crate) fn digest(path: &Path) -> io::Result<String> {
    budget_check()?;
    let meta = regular(path)?;
    if meta.len() > MAX_ARTIFACT_BYTES {
        return Err(invalid("artifact byte limit exceeded"));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let mut file = options.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(invalid("artifact is not a regular file"));
    }
    let mut hash = Digest256Hasher::new();
    let mut bytes = [0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        budget_check()?;
        let count = file.read(&mut bytes)?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count as u64)
            .ok_or_else(|| invalid("byte count overflow"))?;
        if total > MAX_ARTIFACT_BYTES {
            return Err(invalid("artifact byte limit exceeded"));
        }
        hash.update(&bytes[..count]);
    }
    if total != meta.len() || file.metadata()?.len() != meta.len() {
        return Err(invalid("file changed while hashing"));
    }
    Ok(hash.finalize().to_hex())
}
pub(crate) fn read_json(path: &Path) -> io::Result<Value> {
    if regular(path)?.len() > MAX_MANIFEST_BYTES as u64 {
        return Err(invalid("manifest byte limit exceeded"));
    }
    let mut raw = Vec::new();
    File::open(path)?
        .take(MAX_MANIFEST_BYTES as u64 + 1)
        .read_to_end(&mut raw)?;
    if raw.len() > MAX_MANIFEST_BYTES {
        return Err(invalid("manifest byte limit exceeded"));
    }
    strict_json(&raw)
}
pub(crate) fn tree(root: &Path) -> io::Result<(Vec<(String, PathBuf)>, BTreeSet<String>)> {
    directory(root)?;
    let mut files = Vec::new();
    let mut directories = BTreeSet::new();
    let mut pending = vec![root.to_path_buf()];
    let mut visited = 0usize;
    while let Some(current) = pending.pop() {
        budget_check()?;
        for entry in fs::read_dir(&current)? {
            budget_check()?;
            let entry = entry?;
            visited += 1;
            if visited > MAX_MEMBERS {
                return Err(invalid("artifact member limit exceeded"));
            }
            let path = entry.path();
            let name = path
                .strip_prefix(root)
                .map_err(|_| invalid("member leaves root"))?
                .to_str()
                .ok_or_else(|| invalid("artifact name is not UTF-8"))?
                .to_owned();
            relative(&Value::String(name.clone()))?;
            let kind = entry.file_type()?;
            if kind.is_dir() {
                directories.insert(name);
                pending.push(path);
            } else if kind.is_file() {
                regular(&path)?;
                files.push((name, path));
            } else {
                return Err(invalid("artifact tree contains symlink or special file"));
            }
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    Ok((files, directories))
}
fn parent_directories(paths: &BTreeSet<String>) -> BTreeSet<String> {
    let mut result = BTreeSet::new();
    for path in paths {
        let mut parent = Path::new(path).parent();
        while let Some(p) = parent {
            if !p.as_os_str().is_empty() {
                result.insert(p.to_str().unwrap().to_owned());
            }
            parent = p.parent();
        }
    }
    result
}
fn manifest_files(root: &Path) -> io::Result<Value> {
    let (files, _) = tree(root)?;
    let mut total = 0u64;
    let mut result = Vec::new();
    for (path, file) in files {
        let size = regular(&file)?.len();
        total = total
            .checked_add(size)
            .ok_or_else(|| invalid("artifact size overflow"))?;
        if total > MAX_ARTIFACT_BYTES {
            return Err(invalid("artifact closure byte limit exceeded"));
        }
        result.push(json!({"path":path,"size_bytes":size,"sha256":digest(&file)?}));
    }
    Ok(Value::Array(result))
}
fn verify_members(root: &Path, entries: &Value) -> io::Result<()> {
    let entries = entries
        .as_array()
        .ok_or_else(|| invalid("integration files must be list"))?;
    let mut expected = BTreeSet::new();
    let mut previous = "";
    let mut total = 0u64;
    for entry in entries {
        keys(entry, &["path", "sha256", "size_bytes"])?;
        let path = relative(&entry["path"])?;
        if path <= previous || path == "integration.json" {
            return Err(invalid("integration members must be sorted unique"));
        }
        previous = path;
        let size = entry["size_bytes"]
            .as_u64()
            .ok_or_else(|| invalid("invalid member size"))?;
        total = total
            .checked_add(size)
            .ok_or_else(|| invalid("member size overflow"))?;
        if total > MAX_ARTIFACT_BYTES {
            return Err(invalid("integration closure byte limit exceeded"));
        }
        let file = root.join(path);
        if regular(&file)?.len() != size || digest(&file)? != hex(&entry["sha256"])? {
            return Err(invalid(format!("integration member changed: {path}")));
        }
        expected.insert(path.to_owned());
    }
    expected.insert("integration.json".into());
    let (files, dirs) = tree(root)?;
    if files.into_iter().map(|(p, _)| p).collect::<BTreeSet<_>>() != expected {
        return Err(invalid("integration file membership differs"));
    }
    let mut expected_dirs = parent_directories(&expected);
    for dir in [
        "export",
        "export/Tree-of-Sophia",
        "provider",
        "provider/Tree-of-Sophia",
        "artifacts",
    ] {
        expected_dirs.insert(dir.into());
    }
    if dirs != expected_dirs {
        return Err(invalid("integration directory membership differs"));
    }
    Ok(())
}
fn identity_safe(value: &Value) -> io::Result<()> {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let lower = key.to_lowercase();
                if lower.contains("attempt")
                    || lower.contains("timestamp")
                    || matches!(
                        lower.as_str(),
                        "time" | "created_at" | "updated_at" | "started_at" | "completed_at"
                    )
                {
                    return Err(invalid("portable identity contains runtime field"));
                }
                identity_safe(child)?;
            }
        }
        Value::Array(array) => {
            for child in array {
                identity_safe(child)?;
            }
        }
        Value::String(text) => {
            let raw = text.as_bytes();
            if text.starts_with('/')
                || text.starts_with("\\\\")
                || (raw.len() >= 3
                    && raw[0].is_ascii_alphabetic()
                    && raw[1] == b':'
                    && matches!(raw[2], b'/' | b'\\'))
            {
                return Err(invalid("portable identity contains host absolute path"));
            }
        }
        _ => {}
    }
    Ok(())
}
fn program_manifest(root: &Path) -> io::Result<Value> {
    directory(root)?;
    let mut total = 0u64;
    for path in PROGRAM_PATHS {
        total = total
            .checked_add(regular(&root.join(path))?.len())
            .ok_or_else(|| invalid("KAG program size overflow"))?;
        if total > 32 * 1024 * 1024 {
            return Err(invalid("selected KAG program closure exceeds 32 MiB"));
        }
    }
    PROGRAM_PATHS
        .into_iter()
        .map(|path| Ok(json!({"path":path,"sha256":digest(&root.join(path))?})))
        .collect::<io::Result<Vec<_>>>()
        .map(Value::Array)
}
fn verify_source_copy(export: &Path, provider: &Path, manifest: &Value) -> io::Result<()> {
    tree(provider)?;
    let entries = manifest["files"]
        .as_array()
        .ok_or_else(|| invalid("missing source file list"))?;
    for entry in entries {
        keys(entry, &["path", "sha256", "size_bytes"])?;
        let path = relative(&entry["path"])?;
        let size = entry["size_bytes"]
            .as_u64()
            .ok_or_else(|| invalid("invalid source size"))?;
        for file in [
            export.join("Tree-of-Sophia").join(path),
            provider.join(path),
        ] {
            if regular(&file)?.len() != size || digest(&file)? != hex(&entry["sha256"])? {
                return Err(invalid("private source copy changed"));
            }
        }
    }
    Ok(())
}
pub fn verify_integration(root: &Path, expected_revision: &str) -> io::Result<Value> {
    let _budget = WholeBudget::begin()?;
    directory(root)?;
    let integration = read_json(&root.join("integration.json"))?;
    let mut observed_raw = Vec::new();
    File::open(root.join("integration.json"))?
        .take(MAX_MANIFEST_BYTES as u64 + 1)
        .read_to_end(&mut observed_raw)?;
    if observed_raw.len() > MAX_MANIFEST_BYTES || observed_raw != canonical(&integration)? {
        return Err(invalid("integration manifest not canonical"));
    }
    keys(
        &integration,
        &[
            "schema_version",
            "corpus_revision",
            "export_revision",
            "distribution_identity",
            "primary_source",
            "files",
            "programs",
            "integration_revision",
        ],
    )?;
    if integration["schema_version"] != SCHEMA
        || integration["corpus_revision"] != expected_revision
    {
        return Err(invalid("integration revision/schema differs"));
    }
    hex(&integration["corpus_revision"])?;
    hex(&integration["export_revision"])?;
    let revision = hex(&integration["integration_revision"])?;
    let mut body = integration.clone();
    body.as_object_mut().unwrap().remove("integration_revision");
    if crate::route_cards::sha256_bytes(&canonical(&body)?) != revision {
        return Err(invalid("integration identity mismatch"));
    }
    if !integration["distribution_identity"].is_object() {
        return Err(invalid("distribution identity must be object"));
    }
    keys(
        &integration["primary_source"],
        &["identity", "owner_return_route"],
    )?;
    keys(
        &integration["primary_source"]["identity"],
        &["path", "content_hash"],
    )?;
    if integration["primary_source"]["identity"]["path"] != PRIMARY
        || !integration["primary_source"]["owner_return_route"].is_object()
    {
        return Err(invalid("invalid primary source"));
    }
    hex(&integration["primary_source"]["identity"]["content_hash"])?;
    identity_safe(&integration["distribution_identity"])?;
    identity_safe(&integration["primary_source"])?;
    verify_members(root, &integration["files"])?;
    let programs = integration["programs"]
        .as_array()
        .ok_or_else(|| invalid("program bindings must be list"))?;
    let paths: &[&str] = match programs.len() {
        5 => &PROGRAM_PATHS,
        4 => &HISTORICAL_PROGRAM_PATHS,
        _ => return Err(invalid("program membership differs")),
    };
    for (entry, path) in programs.iter().zip(paths) {
        keys(entry, &["path", "sha256"])?;
        hex(&entry["sha256"])?;
        if entry["path"] != *path {
            return Err(invalid("program path differs"));
        }
    }
    let export = crate::kag_corpus_export::verify_export(&root.join("export"))?;
    if export["corpus_revision"] != expected_revision
        || export["export_revision"] != integration["export_revision"]
        || export["primary_source"]["sha256"]
            != integration["primary_source"]["identity"]["content_hash"]
    {
        return Err(invalid("export binding differs"));
    }
    verify_source_copy(
        &root.join("export"),
        &root.join("provider/Tree-of-Sophia"),
        &export,
    )?;
    Ok(integration)
}

fn sync_tree(root: &Path) -> io::Result<()> {
    let (files, dirs) = tree(root)?;
    for (_, file) in files {
        budget_check()?;
        File::open(file)?.sync_all()?;
    }
    let mut dirs: Vec<_> = dirs.into_iter().collect();
    dirs.sort_by_key(|p| std::cmp::Reverse(p.split('/').count()));
    for dir in dirs {
        budget_check()?;
        File::open(root.join(dir))?.sync_all()?;
    }
    File::open(root)?.sync_all()
}

#[cfg(target_os = "linux")]
pub(crate) fn rename_new(source: &Path, destination: &Path) -> io::Result<()> {
    use std::os::unix::ffi::OsStrExt;
    let source = std::ffi::CString::new(source.as_os_str().as_bytes())
        .map_err(|_| invalid("unsafe rename source"))?;
    let destination = std::ffi::CString::new(destination.as_os_str().as_bytes())
        .map_err(|_| invalid("unsafe rename destination"))?;
    let result = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            source.as_ptr(),
            libc::AT_FDCWD,
            destination.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
#[cfg(not(target_os = "linux"))]
pub(crate) fn rename_new(_: &Path, _: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "immutable publication requires Linux no-replace rename",
    ))
}

fn copy_sources(export: &Path, provider: &Path, manifest: &Value) -> io::Result<()> {
    if fs::symlink_metadata(provider).is_ok() {
        return Err(invalid("provider output must be new"));
    }
    fs::create_dir_all(provider)?;
    directory(provider)?;
    for entry in manifest["files"]
        .as_array()
        .ok_or_else(|| invalid("source file list missing"))?
    {
        let name = relative(&entry["path"])?;
        let source = export.join("Tree-of-Sophia").join(name);
        let target = provider.join(name);
        let expected = entry["size_bytes"]
            .as_u64()
            .ok_or_else(|| invalid("invalid source size"))?;
        if regular(&source)?.len() != expected || digest(&source)? != hex(&entry["sha256"])? {
            return Err(invalid("source changed before copy"));
        }
        fs::create_dir_all(target.parent().unwrap())?;
        safe_absolute(&target)?;
        let mut input = File::open(&source)?;
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)?;
        let copied = io::copy(&mut input.take(expected + 1), &mut output)?;
        if copied != expected {
            return Err(invalid("source changed during copy"));
        }
    }
    verify_source_copy(export, provider, manifest)
}

fn bounded_error(error: impl std::fmt::Display) -> String {
    error
        .to_string()
        .replace('\0', "\\x00")
        .replace('\u{7f}', "\\x7f")
        .chars()
        .take(4096)
        .collect()
}
pub(crate) fn run_owner(
    root: &Path,
    argv: Vec<String>,
    deadline: Instant,
    cancel: &AtomicI32,
) -> io::Result<Vec<u8>> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() || cancel.load(Ordering::Relaxed) != 0 {
        return Err(invalid("selected owner operation cancelled or expired"));
    }
    let (status, out, err) = crate::executor::capture_kag_owner(
        root,
        argv,
        crate::executor::Limits {
            command_wall: remaining.min(Duration::from_secs(300)),
            lane_wall: remaining,
            cleanup_grace: Duration::from_secs(1),
            output_bytes: MAX_MANIFEST_BYTES,
        },
        cancel,
    )?;
    if status != 0 {
        return Err(invalid(format!(
            "selected external owner failed ({status}): {}",
            bounded_error(String::from_utf8_lossy(&err))
        )));
    }
    Ok(out)
}

/// Produce one controlled local release; every expensive stage shares one wall
/// deadline and selected-owner stdout/stderr have the existing custody bound.
pub fn build_release(
    repo: &Path,
    store: &Path,
    revision: &str,
    kag: &Path,
    release: &Path,
    python: &Path,
    cancel: &AtomicI32,
) -> io::Result<Value> {
    let _budget = WholeBudget::begin()?;
    hex(&Value::String(revision.into()))?;
    let repo = safe_absolute(repo)?;
    directory(&repo)?;
    let store = safe_absolute(store)?;
    directory(&store)?;
    let kag = safe_absolute(kag)?;
    directory(&kag)?;
    let programs = program_manifest(&kag)?;
    let python = safe_absolute(python)?;
    regular(&python)?;
    let release = safe_absolute(release)?;
    fs::create_dir_all(release.join("releases"))?;
    directory(&release)?;
    directory(&release.join("releases"))?;
    let status = crate::kag_downstream_status::Status::new(&release.join("status"), "kag")?;
    let attempt = status.begin(revision)?;
    let temporary = release.join(format!(".kag-release-{attempt}"));
    let deadline = DEADLINE
        .with(|d| d.get())
        .ok_or_else(|| invalid("missing whole KAG deadline"))?;
    let result = (|| {
        budget_check()?;
        fs::create_dir(&temporary)?;
        let stage = temporary.join("stage");
        fs::create_dir(&stage)?;
        let export_root = stage.join("export");
        let export = crate::kag_corpus_export::build_export(&repo, &store, revision, &export_root)?;
        let provider = stage.join("provider/Tree-of-Sophia");
        copy_sources(&export_root, &provider, &export)?;
        let controls = crate::provider_controls::materialize(
            &provider,
            &repo.join("kag/provider-template.json"),
        )?;
        let controls = serde_json::to_vec(&controls).map_err(|e| invalid(e.to_string()))?;
        let artifacts = stage.join("artifacts");
        fs::create_dir(&artifacts)?;
        run_owner(
            &kag,
            vec![
                python.to_string_lossy().into_owned(),
                kag.join(PROGRAM_PATHS[0]).to_string_lossy().into_owned(),
                "--repo-root".into(),
                provider.to_string_lossy().into_owned(),
                "--artifact-root".into(),
                artifacts.to_string_lossy().into_owned(),
            ],
            deadline,
            cancel,
        )?;
        directory(&artifacts)?;
        let raw = run_owner(
            &kag,
            vec![
                python.to_string_lossy().into_owned(),
                kag.join("scripts/validate_repo_local_kag_family.py")
                    .to_string_lossy()
                    .into_owned(),
                "--repo-root".into(),
                provider.to_string_lossy().into_owned(),
                "--artifact-root".into(),
                artifacts.to_string_lossy().into_owned(),
                "--no-shadow-git".into(),
                "--probe-source".into(),
                PRIMARY.into(),
            ],
            deadline,
            cancel,
        )?;
        let probe = strict_json(&raw)?;
        keys(&probe, &["primary_source", "distribution_identity"])?;
        crate::provider_controls::verify_request(&provider, &controls)?;
        keys(
            &probe["primary_source"],
            &["identity", "owner_return_route"],
        )?;
        keys(
            &probe["primary_source"]["identity"],
            &["path", "content_hash"],
        )?;
        if probe["primary_source"]["identity"]["path"] != PRIMARY
            || probe["primary_source"]["identity"]["content_hash"]
                != export["primary_source"]["sha256"]
            || !probe["primary_source"]["owner_return_route"].is_object()
            || !probe["distribution_identity"].is_object()
        {
            return Err(invalid("KAG probe primary source mismatch"));
        }
        identity_safe(&probe)?;
        if program_manifest(&kag)? != programs {
            return Err(invalid("selected KAG programs changed during publication"));
        }
        let repeated = crate::kag_corpus_export::verify_export(&export_root)?;
        if repeated != export {
            return Err(invalid("source export changed during publication"));
        }
        verify_source_copy(&export_root, &provider, &repeated)?;
        if program_manifest(&kag)? != programs {
            return Err(invalid("KAG programs changed before manifest"));
        }
        let body = json!({"schema_version":SCHEMA,"corpus_revision":revision,"export_revision":export["export_revision"],
            "distribution_identity":probe["distribution_identity"],"primary_source":probe["primary_source"],
            "files":manifest_files(&stage)?,"programs":programs});
        let integration_revision = crate::route_cards::sha256_bytes(&canonical(&body)?);
        let mut integration = body;
        integration["integration_revision"] = Value::String(integration_revision.clone());
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(stage.join("integration.json"))?;
        output.write_all(&canonical(&integration)?)?;
        output.sync_all()?;
        sync_tree(&stage)?;
        File::open(&temporary)?.sync_all()?;
        if Instant::now() >= deadline || cancel.load(Ordering::Relaxed) != 0 {
            return Err(invalid(
                "selected owner operation cancelled or expired before publication",
            ));
        }
        let destination = release.join("releases").join(&integration_revision);
        match rename_new(&stage, &destination) {
            Ok(()) => {
                File::open(destination.parent().unwrap())?.sync_all()?;
                File::open(&release)?.sync_all()?;
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
        let observed = verify_integration(&destination, revision)?;
        budget_check()?;
        if observed != integration {
            return Err(invalid("published KAG identity differs"));
        }
        // Complete owned staging cleanup before publishing a success status.
        // A refused cleanup retains diagnostics and records a failed attempt,
        // even if the immutable artifact already reached its release path.
        let (temporary_files, temporary_dirs) = tree(&temporary)?;
        for (_, file) in temporary_files {
            budget_check()?;
            regular(&file)?;
            fs::remove_file(file)?;
        }
        let mut temporary_dirs: Vec<_> = temporary_dirs.into_iter().collect();
        temporary_dirs.sort_by_key(|path| std::cmp::Reverse(path.split('/').count()));
        for path in temporary_dirs {
            budget_check()?;
            let path = temporary.join(path);
            directory(&path)?;
            fs::remove_dir(path)?;
        }
        budget_check()?;
        fs::remove_dir(&temporary)?;
        budget_check()?;
        match status.succeed(
            &attempt,
            &integration_revision,
            &digest(&destination.join("integration.json"))?,
        ) {
            Ok(()) => {}
            Err(e) if e.to_string().contains("stale") => {}
            Err(e) => return Err(e),
        }
        Ok(observed)
    })();
    if let Err(error) = &result {
        let _ = status.fail(&attempt, &bounded_error(error));
    }
    result
}

pub fn status_release(release: &Path, expected_revision: &str) -> io::Result<Value> {
    let _budget = WholeBudget::begin()?;
    let release = safe_absolute(release)?;
    let status = crate::kag_downstream_status::Status::new(&release.join("status"), "kag")?
        .status(expected_revision)?;
    budget_check()?;
    let mut result = status;
    result["source_kind"] = Value::String("corpus_revision".into());
    result["integration_revision"] = Value::Null;
    result["primary_source"] = Value::Null;
    let success = result["state"]["last_success"].clone();
    if success.is_null() {
        return Ok(result);
    }
    let revision = hex(&success["artifact_revision"])?;
    let destination = release.join("releases").join(revision);
    let observed = verify_integration(&destination, hex(&success["source_revision"])?)?;
    if observed["integration_revision"] != revision
        || digest(&destination.join("integration.json"))?
            != hex(&success["artifact_manifest_sha256"])?
    {
        return Err(invalid("selected KAG integration differs from status"));
    }
    result["integration_revision"] = observed["integration_revision"].clone();
    result["primary_source"] = observed["primary_source"].clone();
    Ok(result)
}
