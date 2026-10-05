//! Native reader of the existing managed-local ReleaseStore contract.
//! A shared `.release.lock` remains held until the prepared packet is dropped.
//! This holder authorizes only the selected admitted projection, not source bytes.
use crate::{AccessError, AccessErrorCode};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::Read,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::Arc,
};
use tos_foundation::{
    CanonicalProfile, Digest256, JsonLimits, JsonMode, JsonValue, RelativePath, canonical_bytes_v1,
    parse_json,
};

pub const NATIVE_DATA_SCHEMA: &str = "tos_access_native_data_snapshot_v1";
// Bootstrap metadata uses the shared foundation profile before owner cold caps
// can be decoded. It never supplies model, query or process limits.
const METADATA_LIMITS: JsonLimits = JsonLimits {
    max_bytes: 1_048_576,
    max_depth: 64,
    max_visits: 300_000,
    max_integer_digits: 4_300,
};
type Result<T> = std::result::Result<T, AccessError>;
fn unavailable(message: &'static str) -> AccessError {
    AccessError::new(AccessErrorCode::Unavailable, message)
}
fn text<'a>(value: &'a JsonValue, key: &str) -> Result<&'a str> {
    value
        .object_get(key)
        .and_then(JsonValue::as_str)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| unavailable("release string field absent"))
}
fn digest(value: &JsonValue, key: &str) -> Result<Digest256> {
    Digest256::from_hex(text(value, key)?).map_err(|_| unavailable("release digest invalid"))
}
fn keys(value: &JsonValue, expected: &[&str]) -> Result<()> {
    let fields = value
        .as_object()
        .ok_or_else(|| unavailable("release object absent"))?;
    if fields.len() != expected.len()
        || fields
            .iter()
            .any(|(key, _)| !key.as_str().is_some_and(|key| expected.contains(&key)))
    {
        return Err(unavailable("release object field set invalid"));
    }
    Ok(())
}
fn identity(file: &File) -> Result<(u64, u64)> {
    let m = file
        .metadata()
        .map_err(|_| unavailable("release file identity unavailable"))?;
    Ok((m.dev(), m.ino()))
}
fn child(root: &File, relative: &str) -> Result<File> {
    RelativePath::parse(relative).map_err(|_| unavailable("release member path invalid"))?;
    let mut parts = relative.split('/').peekable();
    let mut directory = tos_fd_open::reopen_directory(root)
        .map_err(|_| unavailable("release directory unavailable"))?;
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            return tos_fd_open::open_regular_at(&directory, Path::new(part))
                .map_err(|_| unavailable("release member unavailable"));
        }
        directory = tos_fd_open::open_directory_at(&directory, Path::new(part))
            .map_err(|_| unavailable("release member directory unavailable"))?;
    }
    Err(unavailable("release member absent"))
}
fn bytes(mut file: File, cap: usize) -> Result<Vec<u8>> {
    if file
        .metadata()
        .map_err(|_| unavailable("release metadata unavailable"))?
        .len()
        > cap as u64
    {
        return Err(unavailable("release metadata byte budget exceeded"));
    }
    let mut raw = Vec::new();
    file.by_ref()
        .take(cap as u64 + 1)
        .read_to_end(&mut raw)
        .map_err(|_| unavailable("release metadata read failed"))?;
    if raw.len() > cap {
        return Err(unavailable("release metadata byte budget exceeded"));
    }
    Ok(raw)
}
fn canonical_read(root: &File, path: &str) -> Result<(JsonValue, Vec<u8>)> {
    let raw = bytes(child(root, path)?, METADATA_LIMITS.max_bytes)?;
    let value = parse_json(&raw, JsonMode::PublishedStrict, METADATA_LIMITS)
        .map_err(|_| unavailable("release metadata JSON invalid"))?
        .into_root();
    if canonical_bytes_v1(&value, CanonicalProfile::CorpusSnapshotV1, METADATA_LIMITS)
        .map_err(|_| unavailable("release metadata canonicalization failed"))?
        != raw
    {
        return Err(unavailable("release metadata not canonical"));
    }
    Ok((value, raw))
}
fn absolute_directory(value: &str) -> Result<(PathBuf, File)> {
    let path = PathBuf::from(value);
    let file = tos_fd_open::open_absolute_directory(&path)
        .map_err(|_| unavailable("release selected directory invalid"))?;
    Ok((path, file))
}
fn safe_reference_absolute_path(value: &str, path: &Path) -> bool {
    value.starts_with('/')
        && !value.contains('\\')
        && !value.bytes().any(|byte| byte < 0x20 || byte == 0x7f)
        && (value == "/"
            || value.strip_prefix('/').is_some_and(|tail| {
                tail.split('/')
                    .all(|part| !part.is_empty() && part != "." && part != "..")
            }))
        && path.components().all(|part| {
            matches!(
                part,
                std::path::Component::RootDir | std::path::Component::Normal(_)
            )
        })
}
fn input_bindings(value: &JsonValue) -> Result<()> {
    let fields = value
        .as_object()
        .ok_or_else(|| unavailable("snapshot input bindings absent"))?;
    if fields.is_empty() {
        return Err(unavailable("snapshot input bindings empty"));
    }
    for (path, sha) in fields {
        RelativePath::parse(
            path.as_str()
                .ok_or_else(|| unavailable("snapshot input path invalid"))?,
        )
        .map_err(|_| unavailable("snapshot input path invalid"))?;
        Digest256::from_hex(
            sha.as_str()
                .ok_or_else(|| unavailable("snapshot input digest absent"))?,
        )
        .map_err(|_| unavailable("snapshot input digest invalid"))?;
    }
    Ok(())
}
/// Existing authored software declaration, not an ambient runtime tree glob.
pub const RUNTIME_DATA_DECLARATION_PATH: &str = "access/contracts/runtime-data.v1.json";
pub const RUNTIME_DATA_DECLARATION: &[u8] =
    include_bytes!("../../../../access/contracts/runtime-data.v1.json");

/// Root-relative source subjects the existing runtime-data owner exposes to a
/// query-capable consumer. A path remains a declaration, not a source grant.
fn declared_query_source_paths() -> Result<BTreeSet<String>> {
    let document = parse_json(
        RUNTIME_DATA_DECLARATION,
        JsonMode::PublishedStrict,
        METADATA_LIMITS,
    )
    .map_err(|_| unavailable("runtime-data declaration invalid"))?;
    let root = document.root();
    if text(root, "schema_version")? != "tos_access_runtime_data_allowlist_v1"
        || text(root, "publication_posture")? != "allowlist-only"
    {
        return Err(unavailable("runtime-data declaration profile invalid"));
    }
    let mut all_paths = BTreeSet::new();
    let mut query_paths = BTreeSet::new();
    for subject in root
        .object_get("subjects")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| unavailable("runtime-data subjects absent"))?
    {
        let path = text(subject, "source_path")?;
        RelativePath::parse(path).map_err(|_| unavailable("runtime-data source path invalid"))?;
        if !all_paths.insert(path.to_owned()) {
            return Err(unavailable("runtime-data source path duplicated"));
        }
        let roles = subject
            .object_get("consumer_roles")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| unavailable("runtime-data consumer roles absent"))?;
        if ["query-core", "http-reader", "native-mcp"]
            .iter()
            .any(|role| roles.iter().any(|value| value.as_str() == Some(*role)))
        {
            query_paths.insert(path.to_owned());
        }
    }
    if query_paths.is_empty() {
        return Err(unavailable("runtime-data query subjects absent"));
    }
    Ok(query_paths)
}

/// Producer layout for the existing allowlisted public query/http ledger subset.
/// Paths are derived from the owner declaration; identities/counts are not rules.
pub fn public_source_gap_paths() -> Result<Vec<String>> {
    use tos_query::source_gap::{SOURCE_GAP_LEDGER_PREFIX, SOURCE_GAP_RECORD_SUFFIX};
    let document = parse_json(
        RUNTIME_DATA_DECLARATION,
        JsonMode::PublishedStrict,
        METADATA_LIMITS,
    )
    .map_err(|_| unavailable("runtime-data declaration invalid"))?;
    let root = document.root();
    if text(root, "schema_version")? != "tos_access_runtime_data_allowlist_v1"
        || text(root, "publication_posture")? != "allowlist-only"
    {
        return Err(unavailable("runtime-data declaration profile invalid"));
    }
    let mut paths = std::collections::BTreeSet::new();
    let mut subjects = std::collections::BTreeSet::new();
    for subject in root
        .object_get("subjects")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| unavailable("runtime-data subjects absent"))?
    {
        let path = text(subject, "source_path")?;
        let Some(filename) = path.strip_prefix(SOURCE_GAP_LEDGER_PREFIX) else {
            continue;
        };
        let roles = subject
            .object_get("consumer_roles")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| unavailable("public ledger consumer roles absent"))?;
        if !["query-core", "http-reader"]
            .iter()
            .all(|role| roles.iter().any(|v| v.as_str() == Some(*role)))
        {
            continue;
        }
        if RelativePath::parse(path).is_err()
            || filename.contains('/')
            || !filename.ends_with(SOURCE_GAP_RECORD_SUFFIX)
            || !subjects.insert(text(subject, "subject_id")?)
            || !paths.insert(path.to_owned())
        {
            return Err(unavailable("public ledger declaration invalid"));
        }
    }
    if paths.is_empty() {
        return Err(unavailable("public ledger subset absent"));
    }
    Ok(paths.into_iter().collect())
}

#[derive(Clone)]
struct Member {
    size: u64,
    digest: Digest256,
}
pub struct ManagedRelease {
    root_path: PathBuf,
    root: File,
    data_path: PathBuf,
    data: File,
    root_identity: (u64, u64),
    data_identity: (u64, u64),
    lock_identity: (u64, u64),
    pair_id: String,
    pointer_raw: Vec<u8>,
    pair_raw: Vec<u8>,
    bindings_raw: Vec<u8>,
    manifest_raw: Vec<u8>,
    data_revision: String,
    corpus_revision: String,
    software_sha256: String,
    pub query_schema: String,
    pub compiler_version: String,
    selection_path: String,
    members: BTreeMap<String, Member>,
    source_bindings: BTreeMap<String, Digest256>,
    compiler_bindings: BTreeMap<String, Digest256>,
}
#[derive(Clone)]
pub struct ReleaseMemberGuard {
    path: String,
    identity: (u64, u64, u64, i64, i64, i64, i64),
}
fn member_identity(file: &File) -> Result<(u64, u64, u64, i64, i64, i64, i64)> {
    let m = file
        .metadata()
        .map_err(|_| unavailable("declared corpus member metadata unavailable"))?;
    Ok((
        m.dev(),
        m.ino(),
        m.size(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    ))
}
pub struct ReleaseLease {
    release: Arc<ManagedRelease>,
    _lock: File,
    member_guards: Vec<ReleaseMemberGuard>,
}

/// Native retained reader for the maintained Python `ReleaseStore` format.
/// This is intentionally separate from `ManagedRelease`, whose data manifest
/// has a different schema and whose selection has a different owner.
pub(crate) struct ReferenceReleaseGuard {
    root_path: PathBuf,
    root: File,
    root_identity: (u64, u64),
    lock: Option<File>,
    lock_identity: Option<(u64, u64)>,
    pair_id: String,
    pointer_raw: Vec<u8>,
    pair_raw: Vec<u8>,
    bindings_raw: Vec<u8>,
    revoked_digests: [String; 3],
    receipt: String,
}

impl ReferenceReleaseGuard {
    pub(crate) fn open_for_standalone_snapshot(
        release_root_path: &Path,
        snapshot_root_path: &Path,
        manifest_raw: &[u8],
    ) -> Result<Self> {
        Self::open_selected(release_root_path, Some((snapshot_root_path, manifest_raw)))
            .map(|(_, guard)| guard)
    }

    /// Resolve the maintained Reference ReleaseStore while retaining its shared
    /// lock and exact pointer/pair/binding bytes through the caller's operation.
    pub(crate) fn resolve_current_snapshot_root(
        release_root_path: &Path,
    ) -> Result<(PathBuf, Self)> {
        Self::open_selected(release_root_path, None)
    }

    fn open_selected(
        release_root_path: &Path,
        selected: Option<(&Path, &[u8])>,
    ) -> Result<(PathBuf, Self)> {
        let raw_root = release_root_path;
        if raw_root.as_os_str().len() > 8193
            || raw_root
                .components()
                .any(|part| matches!(part, std::path::Component::ParentDir))
        {
            return Err(unavailable("Reference release root path invalid"));
        }
        let root_path = if raw_root.is_absolute() {
            raw_root.to_owned()
        } else {
            std::env::current_dir()
                .map_err(|_| unavailable("Reference release current directory unavailable"))?
                .join(raw_root)
        };
        if root_path.as_os_str().len() > 8193
            || root_path.components().any(|part| {
                !matches!(
                    part,
                    std::path::Component::RootDir | std::path::Component::Normal(_)
                )
            })
        {
            return Err(unavailable("Reference release root path invalid"));
        }
        let root = tos_fd_open::open_absolute_directory(&root_path)
            .map_err(|_| unavailable("Reference release root unavailable"))?;
        let root_identity = identity(&root)?;
        let lock = match tos_fd_open::open_regular_at(&root, Path::new(".release.lock")) {
            Ok(lock) => {
                lock.try_lock_shared()
                    .map_err(|_| unavailable("Reference release shared lock unavailable"))?;
                Some(lock)
            }
            Err(error)
                if error
                    .source
                    .as_ref()
                    .is_some_and(|source| source.kind() == std::io::ErrorKind::NotFound) =>
            {
                None
            }
            Err(_) => return Err(unavailable("Reference release lock state unsafe")),
        };
        let lock_identity = lock.as_ref().map(identity).transpose()?;

        let (pointer, pointer_raw) = canonical_read(&root, "current.json")?;
        keys(&pointer, &["schema_version", "current", "previous"])?;
        if text(&pointer, "schema_version")? != "tos_access_release_pointer_v1" {
            return Err(unavailable("Reference release pointer schema invalid"));
        }
        let pair_id = digest(&pointer, "current")?.to_hex();
        match pointer.object_get("previous") {
            Some(JsonValue::Null) => {}
            Some(JsonValue::String(_)) if digest(&pointer, "previous")?.to_hex() != pair_id => {}
            _ => return Err(unavailable("Reference release previous pointer invalid")),
        }
        let (pair, pair_raw) = canonical_read(&root, &format!("pairs/{pair_id}.json"))?;
        keys(
            &pair,
            &[
                "schema_version",
                "software_sha256",
                "data_revision",
                "data_manifest_sha256",
                "corpus_revision",
                "query_schema",
                "compiler_version",
            ],
        )?;
        if text(&pair, "schema_version")? != "tos_access_release_pair_v1"
            || Digest256::of_bytes(&pair_raw).to_hex() != pair_id
        {
            return Err(unavailable("Reference release pair identity invalid"));
        }
        let pair_data_manifest = digest(&pair, "data_manifest_sha256")?;
        let (bindings, bindings_raw) = canonical_read(&root, &format!("bindings/{pair_id}.json"))?;
        keys(&bindings, &["data_root", "software_archive"])?;
        let bound_data_text = text(&bindings, "data_root")?;
        let archive_text = text(&bindings, "software_archive")?;
        let bound_data = PathBuf::from(bound_data_text);
        let archive = Path::new(archive_text);
        if !safe_reference_absolute_path(bound_data_text, &bound_data)
            || bound_data.as_os_str().len() > 8193
            || selected.is_some_and(|(path, _)| bound_data != path)
            || !safe_reference_absolute_path(archive_text, archive)
        {
            return Err(unavailable(
                "Reference release data binding differs from selected snapshot",
            ));
        }
        let resolved_manifest;
        let manifest_raw = if let Some((_, raw)) = selected {
            raw
        } else {
            let directory = tos_fd_open::open_absolute_directory(&bound_data)
                .map_err(|_| unavailable("Reference release snapshot root unavailable"))?;
            resolved_manifest = canonical_read(&directory, "manifest.json")?.1;
            &resolved_manifest
        };
        let snapshot = parse_json(manifest_raw, JsonMode::PublishedStrict, METADATA_LIMITS)
            .map_err(|_| unavailable("standalone snapshot manifest invalid for release"))?
            .into_root();
        if text(&snapshot, "schema_version")? != "tos_access_data_snapshot_v1" {
            return Err(unavailable(
                "standalone snapshot schema does not match Reference release",
            ));
        }
        let data_revision = text(&snapshot, "data_revision")?;
        let corpus_revision = text(&snapshot, "corpus_revision")?;
        let compiler = snapshot
            .object_get("compiler")
            .ok_or_else(|| unavailable("standalone snapshot compiler absent"))?;
        let query_schema = text(compiler, "schema")?;
        let compiler_version = text(compiler, "compiler_version")?;

        if pair_data_manifest != Digest256::of_bytes(manifest_raw)
            || text(&pair, "data_revision")? != data_revision
            || text(&pair, "corpus_revision")? != corpus_revision
            || text(&pair, "query_schema")? != query_schema
            || text(&pair, "compiler_version")? != compiler_version
        {
            return Err(unavailable(
                "Reference release pair differs from standalone snapshot",
            ));
        }
        let revoked_digests = [
            digest(&pair, "data_revision")?.to_hex(),
            digest(&pair, "corpus_revision")?.to_hex(),
            digest(&pair, "software_sha256")?.to_hex(),
        ];
        let receipt = Digest256::of_bytes(
            format!(
                "tos-reference-release-guard-v1\0{}\0{}\0{}",
                root_path.display(),
                pair_id,
                Digest256::of_bytes(manifest_raw).to_hex()
            )
            .as_bytes(),
        )
        .to_hex();
        let guard = Self {
            root_path,
            root,
            root_identity,
            lock,
            lock_identity,
            pair_id: pair_id.clone(),
            pointer_raw,
            pair_raw,
            bindings_raw,
            revoked_digests,
            receipt,
        };
        guard.check()?;
        Ok((bound_data, guard))
    }

    pub(crate) fn receipt(&self) -> &str {
        &self.receipt
    }

    pub(crate) fn retained_state_upper_bound(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.root_path.as_os_str().len())
            .saturating_add(self.pair_id.capacity())
            .saturating_add(self.pointer_raw.capacity())
            .saturating_add(self.pair_raw.capacity())
            .saturating_add(self.bindings_raw.capacity())
            .saturating_add(
                self.revoked_digests
                    .iter()
                    .map(String::capacity)
                    .sum::<usize>(),
            )
            .saturating_add(self.receipt.capacity())
    }

    pub(crate) fn check(&self) -> Result<()> {
        let root = tos_fd_open::open_absolute_directory(&self.root_path)
            .map_err(|_| unavailable("Reference release root changed"))?;
        if identity(&root)? != self.root_identity
            || self.lock.as_ref().map(identity).transpose()? != self.lock_identity
        {
            return Err(unavailable("Reference release holder identity changed"));
        }
        if canonical_read(&self.root, "current.json")?.1 != self.pointer_raw
            || canonical_read(&self.root, &format!("pairs/{}.json", self.pair_id))?.1
                != self.pair_raw
            || canonical_read(&self.root, &format!("bindings/{}.json", self.pair_id))?.1
                != self.bindings_raw
        {
            return Err(unavailable("Reference release selection changed"));
        }
        for (kind, value) in [
            ("data", self.revoked_digests[0].as_str()),
            ("corpus", self.revoked_digests[1].as_str()),
            ("software", self.revoked_digests[2].as_str()),
        ] {
            let directory = tos_fd_open::open_directory_at(&self.root, Path::new("revocations"))
                .and_then(|dir| tos_fd_open::open_directory_at(&dir, Path::new(kind)))
                .map_err(|_| unavailable("Reference release revocation state unavailable"))?;
            match tos_fd_open::open_regular_at(&directory, Path::new(&format!("{value}.json"))) {
                Err(error)
                    if error
                        .source
                        .as_ref()
                        .is_some_and(|source| source.kind() == std::io::ErrorKind::NotFound) => {}
                _ => {
                    return Err(unavailable(
                        "Reference release is revoked or revocation state unsafe",
                    ));
                }
            }
        }
        Ok(())
    }
}

/// Bounded metadata resolver for the maintained Reference ReleaseStore ABI.
/// It exposes selectors and a selection receipt, never projection/source bytes.
pub fn run_reference_root_if_requested(
    args: &[String],
    output: &mut dyn std::io::Write,
    errors: &mut dyn std::io::Write,
) -> Option<i32> {
    if args.first().map(String::as_str) != Some("reference-release-root") {
        return None;
    }
    let result = (|| -> std::result::Result<(), String> {
        if args.len() != 3 || args[1] != "--release-root" || args[2].len() > 8193 {
            return Err("Reference release resolver requires --release-root ABS".into());
        }
        let release_root = Path::new(&args[2]);
        if !safe_reference_absolute_path(&args[2], release_root) {
            return Err("Reference release resolver requires exact absolute root".into());
        }
        let (snapshot_root, guard) =
            ReferenceReleaseGuard::resolve_current_snapshot_root(release_root)
                .map_err(|error| error.to_string())?;
        guard.check().map_err(|error| error.to_string())?;
        let packet = serde_json::json!({
            "schema_version": "tos_reference_release_root_v1",
            "root": snapshot_root.join("data"),
            "snapshot_root": snapshot_root,
            "reference_release_guard": guard.receipt(),
        });
        serde_json::to_writer(&mut *output, &packet).map_err(|error| error.to_string())?;
        output.write_all(b"\n").map_err(|error| error.to_string())?;
        guard.check().map_err(|error| error.to_string())?;
        Ok(())
    })();
    Some(match result {
        Ok(()) => 0,
        Err(error) => {
            let _ = writeln!(errors, "Reference release resolver refused: {error}");
            125
        }
    })
}

impl ManagedRelease {
    /// Selection is explicit; no data or authority is discovered through cwd.
    pub fn open(root_path: &Path) -> Result<Arc<Self>> {
        Self::open_selected_source_root(root_path, None)
    }
    /// An explicit logical SourceRoot is accepted only when it is the `data/`
    /// child of the current pair's manifest directory. The shared release lock
    /// and normal pair/member checks remain the owner of that binding.
    pub fn open_selected_source_root(
        root_path: &Path,
        selected_source_root: Option<&Path>,
    ) -> Result<Arc<Self>> {
        if selected_source_root.is_some_and(|path| !path.is_absolute()) {
            return Err(unavailable("selected release source root must be absolute"));
        }
        let root = tos_fd_open::open_absolute_directory(root_path)
            .map_err(|_| unavailable("managed release root unavailable"))?;
        let root_identity = identity(&root)?;
        let lock = child(&root, ".release.lock")?;
        lock.try_lock_shared()
            .map_err(|_| unavailable("managed release holder busy or unavailable"))?;
        let lock_identity = identity(&lock)?;
        let (pointer, pointer_raw) = canonical_read(&root, "current.json")?;
        keys(&pointer, &["schema_version", "current", "previous"])?;
        if text(&pointer, "schema_version")? != "tos_access_release_pointer_v1" {
            return Err(unavailable("release pointer schema invalid"));
        }
        let pair_id = digest(&pointer, "current")?.to_hex();
        match pointer.object_get("previous") {
            Some(JsonValue::Null) => {}
            Some(JsonValue::String(_)) if digest(&pointer, "previous")?.to_hex() != pair_id => {}
            _ => return Err(unavailable("release previous pointer invalid")),
        }
        let (pair, pair_raw) = canonical_read(&root, &format!("pairs/{pair_id}.json"))?;
        keys(
            &pair,
            &[
                "schema_version",
                "software_sha256",
                "data_revision",
                "data_manifest_sha256",
                "corpus_revision",
                "query_schema",
                "compiler_version",
            ],
        )?;
        if text(&pair, "schema_version")? != "tos_access_release_pair_v1"
            || Digest256::of_bytes(&pair_raw).to_hex() != pair_id
        {
            return Err(unavailable("release pair identity invalid"));
        }
        let (bindings, bindings_raw) = canonical_read(&root, &format!("bindings/{pair_id}.json"))?;
        keys(&bindings, &["data_root", "software_archive"])?;
        // ReleaseStore binds the archive, but installation verification remains
        // the software release owner; the reader cannot manufacture that proof.
        let archive = Path::new(text(&bindings, "software_archive")?);
        tos_fd_open::open_absolute_regular(archive, u64::MAX)
            .map_err(|_| unavailable("selected software archive unavailable"))?;
        let bound_data_root = text(&bindings, "data_root")?;
        let expected_source_root = Path::new(bound_data_root).join("data");
        if selected_source_root.is_some_and(|path| path.to_str() != expected_source_root.to_str()) {
            return Err(unavailable(
                "explicit source root differs from current release pair",
            ));
        }
        let (data_path, data) = absolute_directory(bound_data_root)?;
        let data_identity = identity(&data)?;
        let (manifest, manifest_raw) = canonical_read(&data, "data/manifest.json")?;
        keys(
            &manifest,
            &[
                "schema_version",
                "corpus_revision",
                "input_bindings",
                "compiler",
                "members",
                "data_revision",
                "native_selection",
            ],
        )?;
        if text(&manifest, "schema_version")? != NATIVE_DATA_SCHEMA
            || Digest256::of_bytes(&manifest_raw) != digest(&pair, "data_manifest_sha256")?
            || text(&manifest, "corpus_revision")? != text(&pair, "corpus_revision")?
            || text(&manifest, "data_revision")? != text(&pair, "data_revision")?
        {
            return Err(unavailable("selected native snapshot binding invalid"));
        }
        let mut body = manifest.clone();
        if let JsonValue::Object(fields) = &mut body {
            fields.retain(|(key, _)| key.as_str() != Some("data_revision"));
        }
        if Digest256::of_bytes(
            &canonical_bytes_v1(&body, CanonicalProfile::CorpusSnapshotV1, METADATA_LIMITS)
                .map_err(|_| unavailable("native snapshot revision failed"))?,
        ) != digest(&manifest, "data_revision")?
        {
            return Err(unavailable("native snapshot revision mismatch"));
        }
        let compiler = manifest
            .object_get("compiler")
            .ok_or_else(|| unavailable("native snapshot compiler absent"))?;
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
        digest(compiler, "compiler_sha256")?;
        input_bindings(
            compiler
                .object_get("input_bindings")
                .ok_or_else(|| unavailable("compiler input bindings absent"))?,
        )?;
        input_bindings(
            manifest
                .object_get("input_bindings")
                .ok_or_else(|| unavailable("snapshot input bindings absent"))?,
        )?;
        let source_bindings = manifest
            .object_get("input_bindings")
            .unwrap()
            .as_object()
            .unwrap()
            .iter()
            .map(|(key, value)| {
                Ok((
                    key.as_str().unwrap().to_owned(),
                    Digest256::from_hex(value.as_str().unwrap())
                        .map_err(|_| unavailable("snapshot input digest invalid"))?,
                ))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        let compiler_bindings = compiler
            .object_get("input_bindings")
            .unwrap()
            .as_object()
            .unwrap()
            .iter()
            .map(|(key, value)| {
                Ok((
                    key.as_str().unwrap().to_owned(),
                    Digest256::from_hex(value.as_str().unwrap())
                        .map_err(|_| unavailable("compiler input binding invalid"))?,
                ))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        let mut prior = String::new();
        let compiler_paths = compiler
            .object_get("compiler_paths")
            .and_then(JsonValue::as_array)
            .filter(|paths| !paths.is_empty())
            .ok_or_else(|| unavailable("compiler paths absent"))?;
        for path in compiler_paths {
            let path = path
                .as_str()
                .ok_or_else(|| unavailable("compiler path invalid"))?;
            RelativePath::parse(path).map_err(|_| unavailable("compiler path invalid"))?;
            if path <= prior.as_str() {
                return Err(unavailable("compiler paths not sorted and unique"));
            }
            prior = path.to_owned();
        }
        if text(compiler, "schema")? != text(&pair, "query_schema")?
            || text(compiler, "compiler_version")? != text(&pair, "compiler_version")?
        {
            return Err(unavailable("release compiler pairing mismatch"));
        }
        let mut members = BTreeMap::new();
        let mut previous = String::new();
        for item in manifest
            .object_get("members")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| unavailable("native snapshot members absent"))?
        {
            keys(item, &["path", "size_bytes", "sha256"])?;
            let path = text(item, "path")?;
            RelativePath::parse(path).map_err(|_| unavailable("native snapshot member invalid"))?;
            if !path.starts_with("data/")
                || path <= previous.as_str()
                || path == "data/manifest.json"
            {
                return Err(unavailable("native snapshot members not sorted and unique"));
            }
            let size = item
                .object_get("size_bytes")
                .and_then(JsonValue::as_u64)
                .ok_or_else(|| unavailable("native snapshot member size invalid"))?;
            members.insert(
                path.to_owned(),
                Member {
                    size,
                    digest: digest(item, "sha256")?,
                },
            );
            previous = path.to_owned();
        }
        let selection_path = text(&manifest, "native_selection")?.to_owned();
        if !members.contains_key(&selection_path) {
            return Err(unavailable(
                "native selection is not a declared snapshot member",
            ));
        }
        let release = Arc::new(Self {
            root_path: root_path.to_owned(),
            root,
            data_path,
            data,
            root_identity,
            data_identity,
            lock_identity,
            pair_id,
            pointer_raw,
            pair_raw,
            bindings_raw,
            manifest_raw,
            data_revision: digest(&pair, "data_revision")?.to_hex(),
            corpus_revision: digest(&pair, "corpus_revision")?.to_hex(),
            software_sha256: digest(&pair, "software_sha256")?.to_hex(),
            query_schema: text(&pair, "query_schema")?.to_owned(),
            compiler_version: text(&pair, "compiler_version")?.to_owned(),
            selection_path,
            members,
            source_bindings,
            compiler_bindings,
        });
        release.check_locked()?;
        // The native cold owner verifies the selected model and exact companion
        // members. Whole data-snapshot publication remains the release builder.
        drop(lock);
        Ok(release)
    }
    fn check_holder_identity(&self) -> Result<()> {
        let root = tos_fd_open::open_absolute_directory(&self.root_path)
            .map_err(|_| unavailable("selected release root changed"))?;
        let data = tos_fd_open::open_absolute_directory(&self.data_path)
            .map_err(|_| unavailable("selected data root changed"))?;
        if identity(&root)? != self.root_identity
            || identity(&data)? != self.data_identity
            || identity(&child(&root, ".release.lock")?)? != self.lock_identity
        {
            return Err(unavailable("release holder identity changed"));
        }
        Ok(())
    }
    fn check_locked(&self) -> Result<()> {
        self.check_holder_identity()?;
        let (pointer, pointer_raw) = canonical_read(&self.root, "current.json")?;
        if text(&pointer, "schema_version")? != "tos_access_release_pointer_v1"
            || text(&pointer, "current")? != self.pair_id
            || pointer_raw != self.pointer_raw
        {
            return Err(unavailable("selected release is no longer current"));
        }
        if canonical_read(&self.root, &format!("pairs/{}.json", self.pair_id))?.1 != self.pair_raw
            || canonical_read(&self.root, &format!("bindings/{}.json", self.pair_id))?.1
                != self.bindings_raw
            || canonical_read(&self.data, "data/manifest.json")?.1 != self.manifest_raw
        {
            return Err(unavailable("selected release binding changed"));
        }
        for (kind, digest) in [
            ("data", &self.data_revision),
            ("corpus", &self.corpus_revision),
            ("software", &self.software_sha256),
        ] {
            let directory = tos_fd_open::open_directory_at(&self.root, Path::new("revocations"))
                .and_then(|dir| tos_fd_open::open_directory_at(&dir, Path::new(kind)))
                .map_err(|_| unavailable("release revocation holder unavailable"))?;
            match tos_fd_open::open_regular_at(&directory, Path::new(&format!("{digest}.json"))) {
                Err(error)
                    if error
                        .source
                        .as_ref()
                        .is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound) => {}
                _ => {
                    return Err(unavailable(
                        "selected release revoked or revocation state unsafe",
                    ));
                }
            }
        }
        Ok(())
    }
    pub fn acquire(self: &Arc<Self>) -> Result<ReleaseLease> {
        let lock = child(&self.root, ".release.lock")?;
        lock.try_lock_shared()
            .map_err(|_| unavailable("managed release holder busy or unavailable"))?;
        self.check_locked()?;
        Ok(ReleaseLease {
            release: Arc::clone(self),
            _lock: lock,
            member_guards: vec![],
        })
    }
    pub fn selection_bytes(&self) -> Result<Vec<u8>> {
        self.member_bytes(&self.selection_path, METADATA_LIMITS.max_bytes)
    }
    pub fn member_bytes(&self, path: &str, cap: usize) -> Result<Vec<u8>> {
        let expected = self
            .members
            .get(path)
            .ok_or_else(|| unavailable("selection path is not a declared snapshot member"))?;
        let raw = bytes(child(&self.data, path)?, cap)?;
        if raw.len() as u64 != expected.size || Digest256::of_bytes(&raw) != expected.digest {
            return Err(unavailable("selected member digest changed"));
        }
        Ok(raw)
    }
    pub fn member_binding(&self, path: &str) -> Result<(u64, Digest256)> {
        let member = self
            .members
            .get(path)
            .ok_or_else(|| unavailable("selection path is not a declared snapshot member"))?;
        Ok((member.size, member.digest))
    }
    pub fn member_path(&self, path: &str) -> Result<PathBuf> {
        if !self.members.contains_key(path) {
            return Err(unavailable(
                "selection path is not a declared snapshot member",
            ));
        }
        Ok(self.data_path.join(path))
    }
    pub fn pair_id(&self) -> &str {
        &self.pair_id
    }
    pub fn root_path(&self) -> &Path {
        &self.root_path
    }
}
impl ReleaseLease {
    /// Full raw closure is verified once at cold admission; subsequent addressed
    /// reads retain DataGuard identity checks under the existing release lock.
    pub fn admit_corpus_members(
        &mut self,
        receipt: &tos_compiler::CorpusOriginalReceipt,
        max_bytes: usize,
    ) -> Result<(
        tos_query::corpus_read::CorpusReadContext,
        Vec<ReleaseMemberGuard>,
    )> {
        self.check_hold()?;
        let release = &self.release;
        let declaration = parse_json(
            RUNTIME_DATA_DECLARATION,
            JsonMode::PublishedStrict,
            METADATA_LIMITS,
        )
        .map_err(|_| unavailable("runtime-data declaration invalid"))?;
        let source = declaration
            .root()
            .object_get("subjects")
            .and_then(JsonValue::as_array)
            .and_then(|subjects| {
                subjects.iter().find(|subject| {
                    subject.object_get("subject_id").and_then(JsonValue::as_str)
                        == Some("tos-corpus-index")
                })
            })
            .ok_or_else(|| unavailable("declared corpus index absent"))?;
        let path = text(source, "source_path")?;
        let roles = source
            .object_get("consumer_roles")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| unavailable("corpus consumer roles absent"))?;
        if path != receipt.origin.source_path
            || !["query-core", "http-reader", "native-mcp"]
                .iter()
                .all(|role| roles.iter().any(|v| v.as_str() == Some(*role)))
            || release.source_bindings.get(RUNTIME_DATA_DECLARATION_PATH)
                != Some(&Digest256::of_bytes(RUNTIME_DATA_DECLARATION))
        {
            return Err(unavailable("corpus original declaration binding differs"));
        }
        // Native produced output and captured public input have disjoint
        // provenance. Neither selection nor this check grants authored rights.
        let runtime_capture = receipt.origin.profile == "captured-runtime-projection-v1";
        let native = match receipt.origin.profile.as_str() {
            "captured-public-corpus-v1" if receipt.origin.native_producer.is_none() => None,
            // A held runtime projection has no invented Git or authored producer origin.
            // The compiler's typed receipt decoder verifies its complete member digest.
            "captured-runtime-projection-v1"
                if receipt.origin.native_producer.is_none()
                    && receipt.origin.source_git_commit.is_none()
                    && receipt.origin.source_git_tree.is_none()
                    && receipt.origin.capture_manifest_sha256.is_some() =>
            {
                None
            }
            "native-corpus-producer-v1" => {
                let proof = receipt
                    .origin
                    .native_producer
                    .as_ref()
                    .ok_or_else(|| unavailable("native corpus producer missing"))?;
                if proof.source_revision != release.corpus_revision
                    || proof.source_cut != receipt.source_cut
                    || proof.source_membership_sha256 != receipt.membership_root
                    || proof.descriptor_sha256 != receipt.descriptor_sha256
                    || proof.output_sha256 != receipt.origin.source_sha256
                    || proof.output_bytes != receipt.origin.source_size_bytes
                    || release
                        .compiler_bindings
                        .get("scripts/tos_corpus_index_common.py")
                        .map(|sha| sha.to_hex())
                        != Some(proof.owner_program_sha256.clone())
                    || release
                        .compiler_bindings
                        .get("ToS/contracts/tos-corpus-index.schema.json")
                        .map(|sha| sha.to_hex())
                        != Some(proof.owner_schema_sha256.clone())
                {
                    return Err(unavailable(
                        "native corpus source/software/output binding differs",
                    ));
                }
                Some(proof)
            }
            _ => {
                return Err(unavailable(
                    "corpus original provenance profile unsupported",
                ));
            }
        };
        let mut total = 0u64;
        let mut guards = vec![];
        let mut source_seen = false;
        for member in &receipt.origin.members {
            let selected = format!("data/{}", member.path);
            let (size, sha) = release.member_binding(&selected)?;
            if size != member.size_bytes
                || sha.to_hex() != member.sha256
                || match native {
                    None if runtime_capture => {
                        // The selected component root authenticates the complete captured
                        // member digest; DataGuard still binds every part below. Only the
                        // declared root needs a redundant source binding. Any supplied
                        // part binding must agree rather than silently overriding custody.
                        (member.path == receipt.origin.source_path
                            && release.source_bindings.get(&member.path) != Some(&sha))
                            || release
                                .source_bindings
                                .get(&member.path)
                                .is_some_and(|bound| bound != &sha)
                    }
                    None => release.source_bindings.get(&member.path) != Some(&sha),
                    Some(proof) => {
                        release.source_bindings.contains_key(&member.path)
                            || member.path != receipt.origin.source_path
                            || member.sha256 != proof.output_sha256
                            || member.size_bytes != proof.output_bytes
                    }
                }
            {
                return Err(unavailable("corpus source member binding differs"));
            }
            total = total
                .checked_add(size)
                .filter(|n| *n <= max_bytes as u64)
                .ok_or_else(|| unavailable("corpus original closure exceeds cold envelope"))?;
            let before = member_identity(&child(&release.data, &selected)?)?;
            release.member_bytes(&selected, max_bytes)?;
            if before != member_identity(&child(&release.data, &selected)?)? {
                return Err(unavailable("corpus member changed during admission"));
            }
            if member.path == path {
                source_seen = size == receipt.origin.source_size_bytes
                    && member.sha256 == receipt.origin.source_sha256;
            }
            guards.push(ReleaseMemberGuard {
                path: selected,
                identity: before,
            });
        }
        if !source_seen {
            return Err(unavailable("complete corpus index source member absent"));
        }
        let index = release.member_path(&format!("data/{path}"))?;
        let root = release.data_path.join("data");
        let context = tos_query::corpus_read::CorpusReadContext {
            tos_root: root
                .to_str()
                .ok_or_else(|| unavailable("corpus root path is not UTF-8"))?
                .to_owned(),
            index_path: index
                .to_str()
                .ok_or_else(|| unavailable("corpus index path is not UTF-8"))?
                .to_owned(),
        };
        self.member_guards = guards.clone();
        self.recheck()?;
        Ok((context, guards))
    }
    /// Admit the exact query-visible members from this already selected
    /// managed release. The seven-path cap matches the maintained Reference
    /// selector; results preserve input order. This selects no root and does
    /// not accept the older standalone Reference snapshot format.
    pub fn admit_query_source_members(
        &mut self,
        source_paths: &[&str],
        max_bytes: usize,
    ) -> Result<Vec<PathBuf>> {
        if source_paths.is_empty() || source_paths.len() > 7 || max_bytes == 0 {
            return Err(unavailable("query source member admission bounds invalid"));
        }
        self.check_hold()?;
        if self
            .release
            .source_bindings
            .get(RUNTIME_DATA_DECLARATION_PATH)
            != Some(&Digest256::of_bytes(RUNTIME_DATA_DECLARATION))
        {
            return Err(unavailable("runtime-data declaration binding differs"));
        }
        let declared = declared_query_source_paths()?;
        let max_bytes = u64::try_from(max_bytes)
            .map_err(|_| unavailable("query source byte budget invalid"))?;
        let mut seen = BTreeSet::new();
        let mut planned = Vec::with_capacity(source_paths.len());
        let mut total = 0u64;
        for source_path in source_paths {
            RelativePath::parse(source_path)
                .map_err(|_| unavailable("query source member path invalid"))?;
            if !declared.contains(*source_path) || !seen.insert((*source_path).to_owned()) {
                return Err(unavailable(
                    "query source member is undeclared or duplicated",
                ));
            }
            let member_path = format!("data/{source_path}");
            let (size, sha) = self.release.member_binding(&member_path)?;
            if self.release.source_bindings.get(*source_path) != Some(&sha) {
                return Err(unavailable("query source input binding differs"));
            }
            total = total
                .checked_add(size)
                .filter(|bytes| *bytes <= max_bytes)
                .ok_or_else(|| unavailable("query source member byte budget exceeded"))?;
            planned.push((member_path, size));
        }
        let mut paths = Vec::with_capacity(planned.len());
        let mut guards = Vec::with_capacity(planned.len());
        for (member_path, size) in planned {
            let cap = usize::try_from(size)
                .map_err(|_| unavailable("query source member size invalid"))?;
            let before = member_identity(&child(&self.release.data, &member_path)?)?;
            self.release.member_bytes(&member_path, cap)?;
            if member_identity(&child(&self.release.data, &member_path)?)? != before {
                return Err(unavailable("query source member changed during admission"));
            }
            paths.push(self.release.member_path(&member_path)?);
            guards.push(ReleaseMemberGuard {
                path: member_path,
                identity: before,
            });
        }
        self.member_guards.extend(guards);
        self.check_hold()?;
        Ok(paths)
    }
    pub fn retain_member_guards(&mut self, guards: &[ReleaseMemberGuard]) -> Result<()> {
        self.member_guards = guards.to_vec();
        self.check_hold()
    }
    pub(crate) fn public_evidence_projection(&mut self, cap: usize) -> Result<Vec<u8>> {
        self.check_hold()?;
        let source = "ToS/derived-exports/epistemic_evidence_projection.min.json";
        let path = format!("data/{source}");
        if self
            .release
            .source_bindings
            .get(RUNTIME_DATA_DECLARATION_PATH)
            != Some(&Digest256::of_bytes(RUNTIME_DATA_DECLARATION))
        {
            return Err(unavailable("Evidence Lens runtime declaration differs"));
        }
        let (size, sha) = self.release.member_binding(&path)?;
        if cap == 0 || size > cap as u64 {
            return Err(AccessError::new(
                AccessErrorCode::BudgetExceeded,
                "Evidence Lens carrier byte budget",
            ));
        }
        if self.release.source_bindings.get(source) != Some(&sha) {
            return Err(AccessError::new(
                AccessErrorCode::CorruptSelectedCarrier,
                "Evidence Lens input binding differs",
            ));
        }
        let before = member_identity(&child(&self.release.data, &path)?)?;
        let raw = self.release.member_bytes(&path, cap)?;
        if member_identity(&child(&self.release.data, &path)?)? != before {
            return Err(unavailable("Evidence Lens carrier changed during read"));
        }
        self.member_guards.push(ReleaseMemberGuard {
            path,
            identity: before,
        });
        self.check_hold()?;
        Ok(raw)
    }
    /// Optional audit comes only from the same owner-declared held snapshot.
    /// Missing declared membership is an absence observation in this snapshot,
    /// never a search of a source checkout or a fallback data root.
    pub(crate) fn public_philosophy_audit(
        &mut self,
        cap: usize,
    ) -> Result<(String, Option<Vec<u8>>)> {
        self.check_hold()?;
        let source =
            "ToS/philosophy/graph-workbench/review-packets/table-i-post-planting-audit.json";
        let path = format!("data/{source}");
        if self
            .release
            .source_bindings
            .get(RUNTIME_DATA_DECLARATION_PATH)
            != Some(&Digest256::of_bytes(RUNTIME_DATA_DECLARATION))
        {
            return Err(unavailable("audit runtime declaration differs"));
        }
        let navigation = self
            .release
            .data_path
            .join(&path)
            .to_str()
            .ok_or_else(|| unavailable("audit navigation path is not UTF-8"))?
            .to_owned();
        let Some(member) = self.release.members.get(&path) else {
            if self.release.source_bindings.contains_key(source) {
                return Err(unavailable("audit binding exists without declared member"));
            }
            self.check_hold()?;
            return Ok((navigation, None));
        };
        if cap == 0 || member.size > cap as u64 {
            return Err(AccessError::new(
                AccessErrorCode::BudgetExceeded,
                "audit carrier byte budget",
            ));
        }
        if self.release.source_bindings.get(source) != Some(&member.digest) {
            return Err(unavailable("audit input binding differs"));
        }
        let before = member_identity(&child(&self.release.data, &path)?)?;
        let raw = self.release.member_bytes(&path, cap)?;
        if member_identity(&child(&self.release.data, &path)?)? != before {
            return Err(unavailable("audit carrier changed during read"));
        }
        self.member_guards.push(ReleaseMemberGuard {
            path,
            identity: before,
        });
        self.check_hold()?;
        Ok((navigation, Some(raw)))
    }
    /// Verify the complete owner-declared subset under this same release hold.
    /// The native producer keeps original source bindings and the declaration
    /// digest in the existing manifest; data/<source_path> preserves provenance.
    pub fn public_source_gap_records(
        &self,
        max_input_bytes: usize,
    ) -> Result<Vec<(String, Vec<u8>)>> {
        use tos_query::source_gap::{
            SOURCE_GAP_LEDGER_PREFIX, SOURCE_GAP_MAX_RECORD_BYTES, SOURCE_GAP_MAX_RECORDS,
        };
        self.check_hold()?;
        let release = &self.release;
        if release.source_bindings.get(RUNTIME_DATA_DECLARATION_PATH)
            != Some(&Digest256::of_bytes(RUNTIME_DATA_DECLARATION))
        {
            return Err(unavailable("public ledger declaration binding unavailable"));
        }
        let paths = public_source_gap_paths()?;
        let member_paths = paths
            .iter()
            .map(|path| format!("data/{path}"))
            .collect::<Vec<_>>();
        let declared = release
            .members
            .keys()
            .filter(|path| path.starts_with(&format!("data/{SOURCE_GAP_LEDGER_PREFIX}")))
            .collect::<Vec<_>>();
        if declared != member_paths.iter().collect::<Vec<_>>() {
            return Err(unavailable("public ledger subset membership differs"));
        }
        let mut total = 0u64;
        for (source, member_path) in paths.iter().zip(&member_paths) {
            let member = release.members.get(member_path).unwrap();
            total = total
                .checked_add(member.size)
                .ok_or_else(|| unavailable("public ledger byte budget exceeded"))?;
            if total > max_input_bytes as u64 || member.size > SOURCE_GAP_MAX_RECORD_BYTES as u64 {
                return Err(unavailable("public ledger byte budget exceeded"));
            }
            if release.source_bindings.get(source) != Some(&member.digest) {
                return Err(unavailable("public ledger original source binding differs"));
            }
        }
        let mut records = Vec::new();
        for (index, (source, member_path)) in paths.into_iter().zip(member_paths).enumerate() {
            let raw = release.member_bytes(&member_path, SOURCE_GAP_MAX_RECORD_BYTES)?;
            // Verify every declared member, retaining only the maintained first100.
            if index < SOURCE_GAP_MAX_RECORDS {
                records.push((source, raw));
            }
        }
        self.check_hold()?;
        Ok(records)
    }
    /// The held shared lock serializes legitimate current/revocation writes.
    /// QRY row boundaries need the retained holder identity, not repeated JSON
    /// parsing of the entire manifest. Final packet recheck below verifies the
    /// exact control bytes and revocation state again before disclosure.
    pub fn check_hold(&self) -> Result<()> {
        self.release.check_holder_identity()?;
        for guard in &self.member_guards {
            if member_identity(&child(&self.release.data, &guard.path)?)? != guard.identity {
                return Err(unavailable("declared corpus member changed"));
            }
        }
        Ok(())
    }
    pub fn recheck(&mut self) -> Result<()> {
        self.check_hold()?;
        self.release.check_locked()
    }
}
