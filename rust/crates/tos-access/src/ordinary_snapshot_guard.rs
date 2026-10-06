//! Native guard for the maintained standalone `tos_access_data_snapshot_v1`
//! selection. This remains distinct from the ManagedRelease owner.
use crate::release_state::ReferenceReleaseGuard;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::Read,
    os::unix::fs::MetadataExt,
    path::{Component, Path, PathBuf},
    time::Instant,
};
use tos_foundation::{
    CanonicalProfile, Digest256, Digest256Hasher, JsonLimits, JsonMode, JsonValue, RelativePath,
    canonical_bytes_v1, parse_json,
};

const SCHEMA: &str = "tos_access_data_snapshot_v1";
const QUERY_STORE: &str = "data/ToS/derived-exports/runtime/knowledge.sqlite3";
const MANIFEST_CAP: usize = 1_048_576;
const MANIFEST_LIMITS: JsonLimits = JsonLimits {
    max_bytes: MANIFEST_CAP,
    max_depth: 64,
    max_visits: 300_000,
    max_integer_digits: 4_300,
};
const MAX_MEMBERS: usize = 16_384;
const MAX_DIRECTORIES: usize = 16_384;
const MAX_PATH_BYTES: usize = 4_096;
const MAX_RETAINED_STATE: usize = 16 * 1024 * 1024;
pub(crate) const MAX_RETAINED_STATE_BYTES: usize = MAX_RETAINED_STATE;

type Result<T> = std::result::Result<T, &'static str>;
type Identity = (u64, u64, u64, u32, i64, i64, i64, i64);
type DirectoryIdentity = (u64, u64);
type DirectoryChangeIdentity = (u64, u64, i64, i64, i64, i64);

fn active(deadline: Instant, check: &mut dyn FnMut() -> Result<()>) -> Result<()> {
    check()?;
    if Instant::now() >= deadline {
        return Err("Core snapshot original deadline expired");
    }
    Ok(())
}
fn identity(file: &File) -> Result<Identity> {
    let metadata = file
        .metadata()
        .map_err(|_| "Core snapshot identity unavailable")?;
    if !metadata.is_file() {
        return Err("Core snapshot member is not a regular file");
    }
    Ok((
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.mode(),
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec(),
    ))
}
fn directory_identity(file: &File) -> Result<DirectoryIdentity> {
    let metadata = file
        .metadata()
        .map_err(|_| "Core snapshot directory identity unavailable")?;
    if !metadata.is_dir() {
        return Err("Core snapshot directory changed type");
    }
    Ok((metadata.dev(), metadata.ino()))
}
fn directory_change_identity(file: &File) -> Result<DirectoryChangeIdentity> {
    let metadata = file
        .metadata()
        .map_err(|_| "Core snapshot directory identity unavailable")?;
    if !metadata.is_dir() {
        return Err("Core snapshot directory changed type");
    }
    Ok((
        metadata.dev(),
        metadata.ino(),
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec(),
    ))
}
fn child(root: &File, relative: &str) -> Result<File> {
    RelativePath::parse(relative).map_err(|_| "Core snapshot member path invalid")?;
    let mut parts = relative.split('/').peekable();
    let mut directory = tos_fd_open::reopen_directory(root)
        .map_err(|_| "Core snapshot data directory unavailable")?;
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            return tos_fd_open::open_regular_at(&directory, Path::new(part))
                .map_err(|_| "Core snapshot member unavailable");
        }
        directory = tos_fd_open::open_directory_at(&directory, Path::new(part))
            .map_err(|_| "Core snapshot member directory unavailable")?;
    }
    Err("Core snapshot member absent")
}
fn child_directory(root: &File, relative: &str) -> Result<File> {
    let mut directory = tos_fd_open::reopen_directory(root)
        .map_err(|_| "Core snapshot data directory unavailable")?;
    if relative.is_empty() {
        return Ok(directory);
    }
    for part in relative.split('/') {
        RelativePath::parse(part).map_err(|_| "Core snapshot directory path invalid")?;
        directory = tos_fd_open::open_directory_at(&directory, Path::new(part))
            .map_err(|_| "Core snapshot directory unavailable")?;
    }
    Ok(directory)
}
fn text<'a>(value: &'a JsonValue, name: &str) -> Result<&'a str> {
    value
        .object_get(name)
        .and_then(JsonValue::as_str)
        .filter(|value| !value.is_empty())
        .ok_or("Core snapshot manifest string absent")
}
fn digest(value: &JsonValue, name: &str) -> Result<Digest256> {
    Digest256::from_hex(text(value, name)?).map_err(|_| "Core snapshot manifest digest invalid")
}
fn keys(value: &JsonValue, expected: &[&str]) -> Result<()> {
    let fields = value
        .as_object()
        .ok_or("Core snapshot manifest object absent")?;
    if fields.len() != expected.len()
        || fields
            .iter()
            .any(|(key, _)| key.as_str().is_none_or(|key| !expected.contains(&key)))
    {
        return Err("Core snapshot manifest field set invalid");
    }
    Ok(())
}
fn normal_relative(path: &str) -> Result<()> {
    if path.is_empty() || path.len() > MAX_PATH_BYTES || RelativePath::parse(path).is_err() {
        return Err("Core snapshot relative path invalid");
    }
    Ok(())
}
fn digest_map(value: &JsonValue) -> Result<BTreeMap<String, String>> {
    let fields = value
        .as_object()
        .ok_or("Core snapshot input bindings absent")?;
    if fields.is_empty() || fields.len() > MAX_MEMBERS {
        return Err("Core snapshot input binding count invalid");
    }
    let mut result = BTreeMap::new();
    let mut previous = "";
    for (key, value) in fields {
        let path = key
            .as_str()
            .ok_or("Core snapshot input binding path invalid")?;
        normal_relative(path)?;
        if path <= previous {
            return Err("Core snapshot input bindings not sorted and unique");
        }
        previous = path;
        let value = value
            .as_str()
            .ok_or("Core snapshot input binding digest absent")?;
        Digest256::from_hex(value).map_err(|_| "Core snapshot input binding digest invalid")?;
        result.insert(path.to_owned(), value.to_owned());
    }
    Ok(result)
}
#[derive(Clone)]
struct DeclaredMember {
    size: u64,
    sha256: Digest256,
}
fn parse_manifest(
    raw: &[u8],
) -> Result<(
    String,
    BTreeMap<String, DeclaredMember>,
    usize,
    String,
    String,
    BTreeMap<String, String>,
)> {
    let parsed = parse_json(raw, JsonMode::PublishedStrict, MANIFEST_LIMITS)
        .map_err(|_| "Core snapshot manifest JSON invalid")?;
    let visits = parsed.visits();
    let mut root = parsed.into_root();
    if canonical_bytes_v1(&root, CanonicalProfile::CorpusSnapshotV1, MANIFEST_LIMITS)
        .map_err(|_| "Core snapshot manifest canonicalization failed")?
        != raw
    {
        return Err("Core snapshot manifest is not canonical");
    }
    keys(
        &root,
        &[
            "schema_version",
            "corpus_revision",
            "input_bindings",
            "compiler",
            "members",
            "data_revision",
        ],
    )?;
    if text(&root, "schema_version")? != SCHEMA {
        return Err("Core snapshot standalone schema unsupported");
    }
    digest(&root, "corpus_revision")?;
    let data_revision = digest(&root, "data_revision")?.to_hex();
    let inputs = digest_map(
        root.object_get("input_bindings")
            .ok_or("Core snapshot input bindings absent")?,
    )?;
    let compiler = root
        .object_get("compiler")
        .ok_or("Core snapshot compiler absent")?;
    keys(
        compiler,
        &[
            "schema",
            "compiler_version",
            "compiler_sha256",
            "compiler_paths",
            "input_bindings",
        ],
    )?;
    if text(compiler, "schema")? != "tos_query_store_v1"
        || text(compiler, "compiler_version")? != "tos_offline_knowledge_v2"
    {
        return Err("Core snapshot query-store ABI incompatible");
    }
    digest(compiler, "compiler_sha256")?;
    let compiler_inputs = digest_map(
        compiler
            .object_get("input_bindings")
            .ok_or("Core snapshot compiler input bindings absent")?,
    )?;
    if compiler_inputs
        .iter()
        .any(|(path, sha)| inputs.get(path) != Some(sha))
    {
        return Err("Core snapshot compiler binding is not bound to input");
    }
    let compiler_schema = text(compiler, "schema")?.to_owned();
    let compiler_version = text(compiler, "compiler_version")?.to_owned();
    let paths = compiler
        .object_get("compiler_paths")
        .and_then(JsonValue::as_array)
        .filter(|paths| !paths.is_empty() && paths.len() <= MAX_MEMBERS)
        .ok_or("Core snapshot compiler paths absent")?;
    let mut previous = "";
    for item in paths {
        let path = item.as_str().ok_or("Core snapshot compiler path invalid")?;
        normal_relative(path)?;
        if path <= previous {
            return Err("Core snapshot compiler paths not sorted and unique");
        }
        previous = path;
    }
    let members = root
        .object_get("members")
        .and_then(JsonValue::as_array)
        .filter(|members| !members.is_empty() && members.len() <= MAX_MEMBERS)
        .ok_or("Core snapshot members absent")?;
    let mut declared = BTreeMap::new();
    let mut previous = "";
    for item in members {
        keys(item, &["path", "size_bytes", "sha256"])?;
        let path = text(item, "path")?;
        normal_relative(path)?;
        if !path.starts_with("data/") || path == "data/manifest.json" || path <= previous {
            return Err("Core snapshot members not sorted or valid");
        }
        previous = path;
        let size = item
            .object_get("size_bytes")
            .and_then(JsonValue::as_u64)
            .ok_or("Core snapshot member size invalid")?;
        let sha256 = digest(item, "sha256")?;
        declared.insert(path.to_owned(), DeclaredMember { size, sha256 });
    }
    let mut expected = BTreeSet::new();
    for path in inputs.keys() {
        expected.insert(format!("data/{path}"));
    }
    if inputs.contains_key(QUERY_STORE.strip_prefix("data/").unwrap()) {
        return Err("Core snapshot QueryStore cannot be a source input");
    }
    expected.insert(QUERY_STORE.to_owned());
    if expected.len() != declared.len() || expected.iter().any(|path| !declared.contains_key(path))
    {
        return Err("Core snapshot members differ from input bindings");
    }
    if inputs.iter().any(|(path, binding)| {
        declared
            .get(&format!("data/{path}"))
            .is_none_or(|member| member.sha256.to_hex() != *binding)
    }) {
        return Err("Core snapshot input binding differs from member digest");
    }
    if let JsonValue::Object(fields) = &mut root {
        fields.retain(|(key, _)| key.as_str() != Some("data_revision"));
    }
    let canonical = canonical_bytes_v1(&root, CanonicalProfile::CorpusSnapshotV1, MANIFEST_LIMITS)
        .map_err(|_| "Core snapshot revision canonicalization failed")?;
    if Digest256::of_bytes(&canonical).to_hex() != data_revision {
        return Err("Core snapshot manifest revision mismatch");
    }
    Ok((
        data_revision,
        declared,
        visits,
        compiler_schema,
        compiler_version,
        compiler_inputs,
    ))
}

fn verify_query_store(
    path: &Path,
    expected_schema: &str,
    expected_version: &str,
    expected_bindings: &BTreeMap<String, String>,
    deadline: Instant,
    check: &mut dyn FnMut() -> Result<()>,
    charge_visits: &mut dyn FnMut(usize) -> Result<()>,
) -> Result<u64> {
    active(deadline, check)?;
    for suffix in ["-wal", "-journal"] {
        let sidecar = PathBuf::from(format!("{}{}", path.display(), suffix));
        match fs::symlink_metadata(sidecar) {
            Ok(_) => return Err("Core snapshot QueryStore has a mutable SQLite sidecar"),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err("Core snapshot QueryStore sidecar state unavailable"),
        }
    }
    let before =
        fs::symlink_metadata(path).map_err(|_| "Core snapshot QueryStore metadata unavailable")?;
    if before.file_type().is_symlink() || !before.is_file() {
        return Err("Core snapshot QueryStore is not a regular file");
    }
    let flags =
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let connection = rusqlite::Connection::open_with_flags(path, flags)
        .map_err(|_| "Core snapshot QueryStore is not a readable SQLite database")?;
    connection
        .pragma_update(None, "query_only", "ON")
        .map_err(|_| "Core snapshot QueryStore read-only profile failed")?;
    let mut metadata = BTreeMap::<String, serde_json::Value>::new();
    let mut total = 0usize;
    let mut rows_seen = 0usize;
    {
        let mut statement = connection
            .prepare("SELECT key,value FROM metadata")
            .map_err(|_| "Core snapshot QueryStore metadata table unavailable")?;
        let mut rows = statement
            .query([])
            .map_err(|_| "Core snapshot QueryStore metadata query failed")?;
        while let Some(row) = rows
            .next()
            .map_err(|_| "Core snapshot QueryStore metadata row failed")?
        {
            active(deadline, check)?;
            rows_seen = rows_seen
                .checked_add(1)
                .filter(|rows| *rows <= 65_536)
                .ok_or("Core snapshot QueryStore metadata row cap")?;
            charge_visits(1)?;
            let key: String = row
                .get(0)
                .map_err(|_| "Core snapshot QueryStore metadata key invalid")?;
            let value: String = row
                .get(1)
                .map_err(|_| "Core snapshot QueryStore metadata value invalid")?;
            total = total
                .checked_add(key.len())
                .and_then(|n| n.checked_add(value.len()))
                .filter(|n| *n <= 4 * 1024 * 1024)
                .ok_or("Core snapshot QueryStore metadata cap")?;
            let value = serde_json::from_str(&value)
                .map_err(|_| "Core snapshot QueryStore metadata JSON invalid")?;
            metadata.insert(key, value);
        }
    }
    let integrity: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|_| "Core snapshot QueryStore integrity check failed")?;
    if integrity != "ok"
        || metadata.get("schema") != Some(&serde_json::Value::String(expected_schema.to_owned()))
        || metadata.get("compiler_version")
            != Some(&serde_json::Value::String(expected_version.to_owned()))
        || metadata.get("complete") != Some(&serde_json::Value::Bool(true))
    {
        return Err("Core snapshot QueryStore metadata differs from manifest");
    }
    let actual_bindings = metadata
        .get("snapshot_bindings")
        .and_then(serde_json::Value::as_object)
        .ok_or("Core snapshot QueryStore input bindings absent")?
        .iter()
        .map(|(path, value)| {
            value
                .as_str()
                .map(|digest| (path.clone(), digest.to_owned()))
                .ok_or("Core snapshot QueryStore input binding invalid")
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    if &actual_bindings != expected_bindings {
        return Err("Core snapshot QueryStore input bindings differ from manifest");
    }
    drop(connection);
    let after = fs::symlink_metadata(path)
        .map_err(|_| "Core snapshot QueryStore changed during validation")?;
    if after.file_type().is_symlink()
        || !after.is_file()
        || (
            before.dev(),
            before.ino(),
            before.len(),
            before.mode(),
            before.mtime(),
            before.mtime_nsec(),
            before.ctime(),
            before.ctime_nsec(),
        ) != (
            after.dev(),
            after.ino(),
            after.len(),
            after.mode(),
            after.mtime(),
            after.mtime_nsec(),
            after.ctime(),
            after.ctime_nsec(),
        )
    {
        return Err("Core snapshot QueryStore changed during validation");
    }
    active(deadline, check)?;
    Ok(before.len())
}
fn hash_member(
    file: &mut File,
    deadline: Instant,
    check: &mut dyn FnMut() -> Result<()>,
) -> Result<Digest256> {
    let mut hash = Digest256Hasher::new();
    let mut block = [0u8; 64 * 1024];
    loop {
        active(deadline, check)?;
        let count = file
            .read(&mut block)
            .map_err(|_| "Core snapshot member read failed")?;
        if count == 0 {
            break;
        }
        hash.update(&block[..count]);
    }
    Ok(hash.finalize())
}

pub(crate) struct StandaloneSnapshotGuard {
    root_path: PathBuf,
    root: File,
    root_identity: DirectoryIdentity,
    data_path: PathBuf,
    data: File,
    data_identity: DirectoryIdentity,
    manifest_identity: Identity,
    manifest_sha256: Digest256,
    files: BTreeMap<String, Identity>,
    directories: BTreeMap<String, DirectoryIdentity>,
    release: Option<ReferenceReleaseGuard>,
    receipt: String,
}
impl StandaloneSnapshotGuard {
    pub(crate) fn open(
        root_path: &Path,
        logical_root: &Path,
        selected_paths: &[&Path],
        expected_receipt: Option<&str>,
        expected_reference_release_guard: Option<&str>,
        max_work_bytes: u64,
        deadline: Instant,
        mut charge_work: impl FnMut(u64) -> Result<()>,
        mut charge_visits: impl FnMut(usize) -> Result<()>,
        mut active_check: impl FnMut() -> Result<()>,
    ) -> Result<Self> {
        if !root_path.is_absolute()
            || root_path.as_os_str().len() > 8193
            || !logical_root.is_absolute()
            || logical_root.as_os_str().len() > 8193
            || root_path
                .components()
                .any(|part| !matches!(part, Component::RootDir | Component::Normal(_)))
            || logical_root
                .components()
                .any(|part| !matches!(part, Component::RootDir | Component::Normal(_)))
            || selected_paths.len() > 7
            || max_work_bytes == 0
        {
            return Err("Core snapshot selection bounds invalid");
        }
        let root = tos_fd_open::open_absolute_directory(root_path)
            .map_err(|_| "Core snapshot manifest root unavailable")?;
        let root_identity = directory_identity(&root)?;
        let data = tos_fd_open::open_directory_at(&root, Path::new("data"))
            .map_err(|_| "Core snapshot data root unavailable")?;
        let data_identity = directory_identity(&data)?;
        let data_path = root_path.join("data");
        if logical_root != data_path {
            return Err("Core snapshot logical source root differs from manifest data root");
        }
        for path in selected_paths {
            if !path.is_absolute()
                || path
                    .components()
                    .any(|part| matches!(part, Component::ParentDir))
            {
                return Err("Core snapshot selected source path invalid");
            }
        }
        let mut manifest = tos_fd_open::open_regular_at(&root, Path::new("manifest.json"))
            .map_err(|_| "Core snapshot manifest unavailable")?;
        let manifest_identity = identity(&manifest)?;
        let raw_len = usize::try_from(manifest_identity.2)
            .map_err(|_| "Core snapshot manifest size invalid")?;
        if raw_len == 0 || raw_len > MANIFEST_CAP {
            return Err("Core snapshot manifest byte budget exceeded");
        }
        let mut remaining_work = max_work_bytes
            .checked_sub(raw_len as u64)
            .ok_or("Core snapshot metadata work budget exceeded")?;
        charge_work(raw_len as u64)?;
        let mut raw = Vec::new();
        (&mut manifest)
            .take(MANIFEST_CAP as u64 + 1)
            .read_to_end(&mut raw)
            .map_err(|_| "Core snapshot manifest read failed")?;
        if raw.len() != raw_len || identity(&manifest)? != manifest_identity {
            return Err("Core snapshot manifest changed during read");
        }
        let manifest_sha256 = Digest256::of_bytes(&raw);
        let (data_revision, declared, visits, compiler_schema, compiler_version, compiler_inputs) =
            parse_manifest(&raw)?;
        let release = match std::env::var_os("TOS_RELEASE_ROOT") {
            Some(path) => Some(
                ReferenceReleaseGuard::open_for_standalone_snapshot(
                    Path::new(&path),
                    root_path,
                    &raw,
                )
                .map_err(|_| "Core snapshot Reference release binding unavailable")?,
            ),
            None => None,
        };
        if expected_reference_release_guard.is_some_and(|expected| {
            release.as_ref().map(ReferenceReleaseGuard::receipt) != Some(expected)
        }) {
            return Err("Core snapshot Reference release constructor selection changed");
        }
        charge_visits(visits)?;
        active(deadline, &mut active_check)?;
        if declared.len() > MAX_MEMBERS {
            return Err("Core snapshot member count budget exceeded");
        }
        let mut files = BTreeMap::new();
        let mut directories = BTreeMap::new();
        let mut actual = BTreeSet::new();
        let mut expected_directories = BTreeSet::new();
        for path in declared.keys() {
            let parts: Vec<_> = path
                .strip_prefix("data/")
                .ok_or("Core snapshot path prefix")?
                .split('/')
                .collect();
            for count in 1..parts.len() {
                expected_directories.insert(parts[..count].join("/"));
            }
        }
        let mut entries_seen = 0usize;
        fn walk(
            data_path: &Path,
            data: &File,
            relative: &str,
            declared: &BTreeMap<String, DeclaredMember>,
            actual: &mut BTreeSet<String>,
            directories: &mut BTreeMap<String, DirectoryIdentity>,
            entries_seen: &mut usize,
            charge_visits: &mut dyn FnMut(usize) -> Result<()>,
            deadline: Instant,
            active_check: &mut dyn FnMut() -> Result<()>,
        ) -> Result<()> {
            active(deadline, active_check)?;
            let directory = if relative.is_empty() {
                data_path.to_path_buf()
            } else {
                data_path.join(relative)
            };
            let dir = child_directory(data, relative)?;
            let dir_identity = directory_identity(&dir)?;
            let change_identity = directory_change_identity(&dir)?;
            let key = relative.to_owned();
            directories.insert(key, dir_identity);
            let entries =
                fs::read_dir(&directory).map_err(|_| "Core snapshot data tree unavailable")?;
            for entry in entries {
                active(deadline, active_check)?;
                let entry = entry.map_err(|_| "Core snapshot data tree entry unavailable")?;
                *entries_seen = entries_seen
                    .checked_add(1)
                    .ok_or("Core snapshot entry count overflow")?;
                if *entries_seen > MAX_MEMBERS + MAX_DIRECTORIES {
                    return Err("Core snapshot data tree entry budget exceeded");
                }
                charge_visits(1)?;
                let name = entry
                    .file_name()
                    .into_string()
                    .map_err(|_| "Core snapshot path is not UTF-8")?;
                let path = if relative.is_empty() {
                    name
                } else {
                    format!("{relative}/{name}")
                };
                normal_relative(&path)?;
                let metadata = fs::symlink_metadata(entry.path())
                    .map_err(|_| "Core snapshot data metadata unavailable")?;
                if metadata.file_type().is_symlink() {
                    return Err("Core snapshot data tree contains a symlink");
                }
                if metadata.is_dir() {
                    walk(
                        data_path,
                        data,
                        &path,
                        declared,
                        actual,
                        directories,
                        entries_seen,
                        charge_visits,
                        deadline,
                        active_check,
                    )?;
                } else if metadata.is_file() {
                    let member_path = format!("data/{path}");
                    let file = child(data, &path)?;
                    if identity(&file)?
                        != (
                            metadata.dev(),
                            metadata.ino(),
                            metadata.len(),
                            metadata.mode(),
                            metadata.mtime(),
                            metadata.mtime_nsec(),
                            metadata.ctime(),
                            metadata.ctime_nsec(),
                        )
                    {
                        return Err("Core snapshot data member changed during discovery");
                    }
                    if !declared.contains_key(&member_path) || !actual.insert(member_path) {
                        return Err("Core snapshot data tree has an undeclared member");
                    }
                } else {
                    return Err("Core snapshot data tree contains a special file");
                }
            }
            if directory_change_identity(&dir)? != change_identity {
                return Err("Core snapshot data directory changed during discovery");
            }
            Ok(())
        }
        walk(
            &data_path,
            &data,
            "",
            &declared,
            &mut actual,
            &mut directories,
            &mut entries_seen,
            &mut charge_visits,
            deadline,
            &mut active_check,
        )?;
        if actual.len() != declared.len()
            || declared.keys().any(|path| !actual.contains(path))
            || directories
                .keys()
                .filter(|path| !path.is_empty())
                .cloned()
                .collect::<BTreeSet<_>>()
                != expected_directories
        {
            return Err("Core snapshot files or directories differ from manifest");
        }
        let root_names = fs::read_dir(root_path)
            .map_err(|_| "Core snapshot root tree unavailable")?
            .map(|entry| {
                entry
                    .map_err(|_| "Core snapshot root entry unavailable")?
                    .file_name()
                    .into_string()
                    .map_err(|_| "Core snapshot root name invalid")
            })
            .collect::<Result<BTreeSet<_>>>()?;
        if root_names != BTreeSet::from(["data".to_owned(), "manifest.json".to_owned()]) {
            return Err("Core snapshot root must contain only data and manifest.json");
        }
        for (path, expected) in &declared {
            active(deadline, &mut active_check)?;
            let source = path
                .strip_prefix("data/")
                .ok_or("Core snapshot member prefix invalid")?;
            let mut file = child(&data, source)?;
            let before = identity(&file)?;
            if before.2 != expected.size {
                return Err("Core snapshot member size differs from manifest");
            }
            remaining_work = remaining_work
                .checked_sub(expected.size)
                .ok_or("Core snapshot member work budget exceeded")?;
            charge_work(expected.size)?;
            let actual_sha = hash_member(&mut file, deadline, &mut active_check)?;
            if actual_sha != expected.sha256 || identity(&file)? != before {
                return Err("Core snapshot member digest or identity differs");
            }
            files.insert(path.clone(), before);
        }
        let query_store_path = root_path.join(QUERY_STORE);
        let query_store_bytes = declared
            .get(QUERY_STORE)
            .ok_or("Core snapshot QueryStore manifest member absent")?
            .size;
        remaining_work = remaining_work
            .checked_sub(query_store_bytes)
            .ok_or("Core snapshot QueryStore verification work budget exceeded")?;
        charge_work(query_store_bytes)?;
        if verify_query_store(
            &query_store_path,
            &compiler_schema,
            &compiler_version,
            &compiler_inputs,
            deadline,
            &mut active_check,
            &mut charge_visits,
        )? != query_store_bytes
        {
            return Err("Core snapshot QueryStore size differs from manifest");
        }
        if expected_receipt.is_some_and(|receipt| {
            receipt.len() != 64
                || !receipt
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        }) {
            return Err("Core snapshot expected guard receipt invalid");
        }
        let receipt = Self::make_receipt(
            root_path,
            root_identity,
            data_identity,
            manifest_identity,
            manifest_sha256,
            &files,
            &directories,
            release.as_ref().map(ReferenceReleaseGuard::receipt),
        );
        if expected_receipt.is_some_and(|expected| expected != receipt) {
            return Err("Core snapshot retained guard identity changed");
        }
        let mut guard = Self {
            root_path: root_path.to_owned(),
            root,
            root_identity,
            data_path,
            data,
            data_identity,
            manifest_identity,
            manifest_sha256,
            files,
            directories,
            release,
            receipt,
        };
        guard.check(
            selected_paths,
            deadline,
            &mut charge_work,
            &mut charge_visits,
            &mut active_check,
        )?;
        // Ensure the revision was consumed into the receipt source and not left
        // as an unaccounted parser-only value.
        if data_revision.is_empty() {
            return Err("Core snapshot data revision absent");
        }
        Ok(guard)
    }
    fn make_receipt(
        root_path: &Path,
        root_identity: DirectoryIdentity,
        data_identity: DirectoryIdentity,
        manifest_identity: Identity,
        manifest_sha256: Digest256,
        files: &BTreeMap<String, Identity>,
        directories: &BTreeMap<String, DirectoryIdentity>,
        release_receipt: Option<&str>,
    ) -> String {
        let mut hash = Digest256Hasher::new();
        hash.update(b"tos-standalone-data-snapshot-guard-v1\0");
        hash.update(root_path.as_os_str().as_encoded_bytes());
        for value in [
            root_identity.0,
            root_identity.1,
            data_identity.0,
            data_identity.1,
        ] {
            hash.update(&value.to_be_bytes());
        }
        for value in [
            manifest_identity.0,
            manifest_identity.1,
            manifest_identity.2,
            manifest_identity.3 as u64,
            manifest_identity.4 as u64,
            manifest_identity.5 as u64,
            manifest_identity.6 as u64,
            manifest_identity.7 as u64,
        ] {
            hash.update(&value.to_be_bytes());
        }
        hash.update(manifest_sha256.as_bytes());
        if let Some(release_receipt) = release_receipt {
            hash.update(b"reference-release\0");
            hash.update(release_receipt.as_bytes());
        }
        for (path, identity) in files {
            hash.update(&(path.len() as u64).to_be_bytes());
            hash.update(path.as_bytes());
            for value in [
                identity.0,
                identity.1,
                identity.2,
                identity.3 as u64,
                identity.4 as u64,
                identity.5 as u64,
                identity.6 as u64,
                identity.7 as u64,
            ] {
                hash.update(&value.to_be_bytes());
            }
        }
        for (path, identity) in directories {
            hash.update(&(path.len() as u64).to_be_bytes());
            hash.update(path.as_bytes());
            for value in [identity.0, identity.1] {
                hash.update(&value.to_be_bytes());
            }
        }
        hash.finalize().to_hex()
    }
    pub(crate) fn receipt(&self) -> &str {
        &self.receipt
    }
    pub(crate) fn retained_state_upper_bound(&self) -> Result<usize> {
        let mut total = std::mem::size_of::<Self>()
            .checked_add(self.root_path.as_os_str().len())
            .and_then(|n| n.checked_add(self.data_path.as_os_str().len()))
            .and_then(|n| n.checked_add(self.receipt.capacity()))
            .and_then(|n| n.checked_add(self.files.len().checked_mul(160)?))
            .and_then(|n| n.checked_add(self.directories.len().checked_mul(160)?))
            .and_then(|n| {
                n.checked_add(
                    self.release
                        .as_ref()
                        .map_or(0, ReferenceReleaseGuard::retained_state_upper_bound),
                )
            })
            .ok_or("Core snapshot guard state overflow")?;
        for path in self.files.keys().chain(self.directories.keys()) {
            total = total
                .checked_add(path.capacity())
                .ok_or("Core snapshot guard path state overflow")?;
        }
        if total > MAX_RETAINED_STATE {
            return Err("Core snapshot guard retained-state cap");
        }
        Ok(total)
    }
    pub(crate) fn check(
        &self,
        selected_paths: &[&Path],
        deadline: Instant,
        charge_work: &mut dyn FnMut(u64) -> Result<()>,
        charge_visits: &mut dyn FnMut(usize) -> Result<()>,
        active_check: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        active(deadline, active_check)?;
        if directory_identity(&self.root)? != self.root_identity
            || directory_identity(&self.data)? != self.data_identity
        {
            return Err("Core snapshot selected root identity changed");
        }
        let current_root = tos_fd_open::open_absolute_directory(&self.root_path)
            .map_err(|_| "Core snapshot selected root changed")?;
        let current_data = tos_fd_open::open_directory_at(&current_root, Path::new("data"))
            .map_err(|_| "Core snapshot selected data root changed")?;
        if directory_identity(&current_root)? != self.root_identity
            || directory_identity(&current_data)? != self.data_identity
        {
            return Err("Core snapshot selected root identity changed");
        }
        let manifest = tos_fd_open::open_regular_at(&current_root, Path::new("manifest.json"))
            .map_err(|_| "Core snapshot selected manifest changed")?;
        if identity(&manifest)? != self.manifest_identity {
            return Err("Core snapshot selected manifest identity changed");
        }
        if let Some(release) = &self.release {
            release
                .check()
                .map_err(|_| "Core snapshot Reference release unavailable")?;
        }
        for selected in selected_paths {
            active(deadline, active_check)?;
            let Ok(relative_path) = selected.strip_prefix(&self.data_path) else {
                // The maintained DataGuard only checks reads below its selected
                // data root. External carrier and Store selections stay with
                // their selected SourceRoot/QueryStore owners.
                continue;
            };
            let relative = relative_path
                .to_str()
                .ok_or("Core snapshot selected source path encoding invalid")?;
            if relative.is_empty() {
                return Err("Core snapshot selected source path is the data root");
            }
            let metadata = match fs::symlink_metadata(selected) {
                Ok(metadata) => Some(metadata),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(_) => return Err("Core snapshot selected source metadata unavailable"),
            };
            if metadata
                .as_ref()
                .is_some_and(|metadata| metadata.file_type().is_symlink())
            {
                return Err("Core snapshot selected source is a symlink");
            }
            if let Some(metadata) = metadata {
                if !metadata.is_file() {
                    return Err("Core snapshot selected source is not a regular file");
                }
                let member = format!("data/{relative}");
                let expected = self
                    .files
                    .get(&member)
                    .ok_or("Core snapshot selected source is not a declared member")?;
                let current = child(&current_data, relative)?;
                charge_visits(1)?;
                charge_work(relative.len() as u64 + 64)?;
                if identity(&current)? != *expected {
                    return Err("Core snapshot selected member identity changed");
                }
            }
            let mut parent = Path::new(relative).parent();
            while let Some(directory) = parent {
                let directory = directory
                    .to_str()
                    .ok_or("Core snapshot selected directory encoding invalid")?;
                let expected = self
                    .directories
                    .get(directory)
                    .ok_or("Core snapshot selected directory is undeclared")?;
                let current = child_directory(&current_data, directory)?;
                charge_visits(1)?;
                charge_work(directory.len() as u64 + 64)?;
                if directory_identity(&current)? != *expected {
                    return Err("Core snapshot selected directory identity changed");
                }
                parent = Path::new(directory).parent();
            }
        }
        let receipt = Self::make_receipt(
            &self.root_path,
            self.root_identity,
            self.data_identity,
            self.manifest_identity,
            self.manifest_sha256,
            &self.files,
            &self.directories,
            self.release.as_ref().map(ReferenceReleaseGuard::receipt),
        );
        if receipt != self.receipt {
            return Err("Core snapshot retained guard receipt changed");
        }
        active(deadline, active_check)
    }
}
