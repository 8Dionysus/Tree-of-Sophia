//! Native reader of the existing managed-local ReleaseStore contract.
//! A shared `.release.lock` remains held until the prepared packet is dropped.
//! This holder authorizes only the selected admitted projection, not source bytes.
use crate::{AccessError, AccessErrorCode};
use std::{
    collections::BTreeMap,
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
}
pub struct ReleaseLease {
    release: Arc<ManagedRelease>,
    _lock: File,
}
impl ManagedRelease {
    /// Selection is explicit; no data or authority is discovered through cwd.
    pub fn open(root_path: &Path) -> Result<Arc<Self>> {
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
        let (data_path, data) = absolute_directory(text(&bindings, "data_root")?)?;
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
    /// The held shared lock serializes legitimate current/revocation writes.
    /// QRY row boundaries need the retained holder identity, not repeated JSON
    /// parsing of the entire manifest. Final packet recheck below verifies the
    /// exact control bytes and revocation state again before disclosure.
    pub fn check_hold(&self) -> Result<()> {
        self.release.check_holder_identity()
    }
    pub fn recheck(&mut self) -> Result<()> {
        self.release.check_locked()
    }
}
