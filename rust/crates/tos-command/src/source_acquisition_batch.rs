//! Selection-owned acquisition custody. No corpus admission or publication.
use crate::source_command as cmd;
use base64::Engine;
use rustix::fs::{AtFlags, FlockOperation, Mode, OFlags};
use serde_json::{Value, json};
use sha1::{Digest, Sha1};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs::{self, File, Metadata};
use std::io::{Read, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};
use tos_foundation::{Digest256, Digest256Hasher};

pub type Result<T> = std::result::Result<T, String>;
pub struct BatchContext {
    pub repo_root: PathBuf,
    pub manifest_path: PathBuf,
    pub manifest_ref: String,
    pub manifest_sha256: String,
    pub raw_manifest: Vec<u8>,
    pub manifest: Value,
}
pub fn text<'a>(v: &'a Value, k: &str) -> Result<&'a str> {
    v.get(k)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing text field: {k}"))
}
pub fn optional_text<'a>(v: &'a Value, k: &str) -> Result<Option<&'a str>> {
    match v.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value)),
        _ => Err(format!("{k} must be text or null")),
    }
}
pub fn array<'a>(v: &'a Value, k: &str) -> Result<&'a Vec<Value>> {
    v.get(k)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("missing array: {k}"))
}
pub fn optional_u64(v: &Value, k: &str) -> Result<Option<u64>> {
    match v.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_u64()
            .map(Some)
            .ok_or_else(|| format!("{k} must be a nonnegative integer or null")),
    }
}
pub fn sha(body: &[u8]) -> String {
    Digest256::of_bytes(body)
        .to_prefixed()
        .trim_start_matches("sha256:")
        .to_owned()
}
pub fn canonical(v: &Value) -> Result<Vec<u8>> {
    let raw = serde_json::to_vec(v).map_err(|e| e.to_string())?;
    let v = cmd::parse(&raw).map_err(|e| format!("invalid JSON: {e:?}"))?;
    let mut bytes = cmd::canonical(&v).map_err(|e| format!("canonical JSON: {e:?}"))?;
    bytes.push(b'\n');
    Ok(bytes)
}
pub fn parse(raw: &[u8]) -> Result<Value> {
    let v = cmd::parse(raw).map_err(|e| format!("invalid or duplicate-key JSON: {e:?}"))?;
    serde_json::from_slice(&cmd::canonical(&v).map_err(|e| format!("JSON: {e:?}"))?)
        .map_err(|e| e.to_string())
}
pub fn safe_ref(s: &str) -> Result<()> {
    if s.is_empty()
        || s.starts_with('/')
        || s.contains(['\\', '\0'])
        || s.split('/').any(|c| c.is_empty() || c == "." || c == "..")
    {
        return Err(format!("unsafe relative reference: {s}"));
    }
    Ok(())
}
pub fn path_under(root: &Path, s: &str) -> Result<PathBuf> {
    safe_ref(s)?;
    let mut p = root.to_owned();
    tos_fd_open::open_absolute_directory(root).map_err(|e| e.to_string())?;
    for part in s.split('/') {
        p.push(part);
        match fs::symlink_metadata(&p) {
            Ok(m) if m.file_type().is_symlink() => {
                return Err(format!("symlink in selected path: {}", p.display()));
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(p)
}
fn identity(m: &Metadata) -> (u64, u64, u64, u32, u32, u64, i64, i64, i64, i64) {
    (
        m.dev(),
        m.ino(),
        m.len(),
        m.mode(),
        m.uid(),
        m.nlink(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    )
}
pub fn read_bytes(
    p: &Path,
    mode: Option<u32>,
    owner: bool,
    single: bool,
    cap: u64,
) -> Result<Vec<u8>> {
    let mut f = tos_fd_open::open_absolute_regular(p, cap)
        .map_err(|e| format!("regular no-follow file {}: {e}", p.display()))?;
    let before = f.metadata().map_err(|e| e.to_string())?;
    if mode.is_some_and(|m| before.mode() & 0o7777 != m)
        || owner && before.uid() != rustix::process::geteuid().as_raw()
        || single && before.nlink() != 1
    {
        return Err(format!(
            "file mode, owner or hard-link conflict: {}",
            p.display()
        ));
    }
    let mut b = Vec::new();
    Read::by_ref(&mut f)
        .take(cap.checked_add(1).ok_or("read budget overflow")?)
        .read_to_end(&mut b)
        .map_err(|e| e.to_string())?;
    let after = f.metadata().map_err(|e| e.to_string())?;
    let current = tos_fd_open::open_absolute_regular(p, cap)
        .map_err(|e| e.to_string())?
        .metadata()
        .map_err(|e| e.to_string())?;
    if b.len() as u64 > cap
        || identity(&before) != identity(&after)
        || identity(&after) != identity(&current)
    {
        return Err(format!("file changed during readback: {}", p.display()));
    }
    Ok(b)
}
pub fn read_json(p: &Path) -> Result<Value> {
    parse(&read_bytes(p, None, false, false, 16 * 1024 * 1024)?)
}
pub fn private_root(p: &Path) -> Result<File> {
    let f = tos_fd_open::open_absolute_directory(p).map_err(|e| e.to_string())?;
    let m = f.metadata().map_err(|e| e.to_string())?;
    if m.mode() & 0o7777 != 0o700 || m.uid() != rustix::process::geteuid().as_raw() {
        return Err(format!(
            "custody root must remain owner-only (0700): {}",
            p.display()
        ));
    }
    Ok(f)
}
fn same_dir(p: &Path, f: &File) -> Result<()> {
    let now = tos_fd_open::open_absolute_directory(p)
        .map_err(|e| e.to_string())?
        .metadata()
        .map_err(|e| e.to_string())?;
    let held = f.metadata().map_err(|e| e.to_string())?;
    if now.dev() != held.dev() || now.ino() != held.ino() {
        return Err("custody directory pathname changed".into());
    }
    Ok(())
}
fn mkdir(p: &Path, mode: u32) -> Result<File> {
    let parent = p.parent().ok_or("directory parent missing")?;
    let d = tos_fd_open::open_absolute_directory(parent).map_err(|e| e.to_string())?;
    rustix::fs::mkdirat(
        &d,
        p.file_name().ok_or("directory leaf missing")?,
        Mode::from_raw_mode(mode),
    )
    .map_err(|e| e.to_string())?;
    let f = tos_fd_open::open_absolute_directory(p).map_err(|e| e.to_string())?;
    same_dir(parent, &d)?;
    if f.metadata().map_err(|e| e.to_string())?.mode() & 0o7777 != mode {
        return Err("directory creation mode changed".into());
    }
    Ok(f)
}
fn parents(p: &Path) -> Result<File> {
    let parent = p.parent().ok_or("parent missing")?;
    let mut current = PathBuf::from("/");
    let mut d = tos_fd_open::open_absolute_directory(&current).map_err(|e| e.to_string())?;
    for part in parent
        .strip_prefix("/")
        .map_err(|_| "absolute destination required")?
        .components()
    {
        let leaf = Path::new(part.as_os_str());
        match tos_fd_open::open_directory_at(&d, leaf) {
            Ok(next) => d = next,
            Err(_) => {
                rustix::fs::mkdirat(&d, leaf, Mode::from_raw_mode(0o755))
                    .map_err(|e| e.to_string())?;
                d = tos_fd_open::open_directory_at(&d, leaf).map_err(|e| e.to_string())?;
            }
        }
        current.push(leaf);
    }
    same_dir(parent, &d)?;
    Ok(d)
}
pub fn publish(p: &Path, body: &[u8], mode: u32) -> Result<&'static str> {
    let d = parents(p)?;
    let leaf = p.file_name().ok_or("missing destination leaf")?;
    if fs::symlink_metadata(p).is_ok() {
        if read_bytes(
            p,
            Some(mode),
            mode == 0o444,
            mode == 0o444,
            300 * 1024 * 1024,
        )? != body
        {
            return Err(format!("immutable output conflict: {}", p.display()));
        }
        return Ok("already_present");
    }
    let token = crate::source_serialization::instant()
        .map_err(|e| format!("clock: {e:?}"))?
        .replace([':', '.'], "");
    let tmp = format!(
        ".{}.{}.{}.tmp",
        leaf.to_string_lossy(),
        std::process::id(),
        token
    );
    let fd = rustix::fs::openat(
        &d,
        tmp.as_str(),
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o600),
    )
    .map_err(|e| e.to_string())?;
    let result = (|| {
        let mut f = File::from(fd);
        f.write_all(body).map_err(|e| e.to_string())?;
        rustix::fs::fchmod(&f, Mode::from_raw_mode(mode)).map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
        same_dir(p.parent().unwrap(), &d)?;
        let copied = match rustix::fs::linkat(&d, tmp.as_str(), &d, leaf, AtFlags::empty()) {
            Ok(()) => true,
            Err(e) if e == rustix::io::Errno::EXIST => {
                if read_bytes(p, Some(mode), mode == 0o444, false, 300 * 1024 * 1024)? != body {
                    return Err("immutable destination race".into());
                }
                false
            }
            Err(e) => return Err(e.to_string()),
        };
        rustix::fs::unlinkat(&d, tmp.as_str(), AtFlags::empty()).map_err(|e| e.to_string())?;
        d.sync_all().map_err(|e| e.to_string())?;
        if read_bytes(
            p,
            Some(mode),
            mode == 0o444,
            mode == 0o444,
            300 * 1024 * 1024,
        )? != body
        {
            return Err("publication readback differs".into());
        }
        Ok(if copied { "copied" } else { "already_present" })
    })();
    let _ = rustix::fs::unlinkat(&d, tmp.as_str(), AtFlags::empty());
    result
}
pub fn load_manifest(path: &Path, repo: &Path, expected: Option<&str>) -> Result<BatchContext> {
    tos_fd_open::open_absolute_directory(repo).map_err(|e| e.to_string())?;
    let path = if path.is_absolute() {
        path.to_owned()
    } else {
        repo.join(path)
    };
    let raw = read_bytes(&path, None, false, false, 16 * 1024 * 1024)?;
    let digest = sha(&raw);
    if expected.is_some_and(|v| v != digest) {
        return Err("acquisition batch manifest SHA-256 differs".into());
    }
    let manifest = parse(&raw)?;
    crate::source_acquisition_contract::validate_manifest(repo, &manifest)?;
    let manifest_ref = path
        .strip_prefix(repo)
        .ok()
        .and_then(Path::to_str)
        .unwrap_or(text(&manifest, "batch_id")?)
        .to_owned();
    Ok(BatchContext {
        repo_root: repo.to_owned(),
        manifest_path: path,
        manifest_ref,
        manifest_sha256: digest,
        raw_manifest: raw,
        manifest,
    })
}
pub fn records(c: &BatchContext) -> Result<Vec<(Value, Value)>> {
    let mut rows = BTreeMap::new();
    for s in array(&c.manifest, "selection")? {
        for r in array(s, "records")? {
            let name = text(r, "ref")?;
            if let Some((_, prior)) = rows.get(name) {
                if text(prior, "sha256")? != text(r, "sha256")? {
                    return Err("selected record has divergent digests".into());
                }
            }
            rows.entry(name.to_owned())
                .or_insert((s.clone(), r.clone()));
        }
    }
    Ok(rows.into_values().collect())
}
pub fn destination_ref(p: &Value) -> Result<String> {
    Ok(format!(
        "{}/{}",
        text(p, "item_root_ref")?,
        text(p, "relative_path")?
    ))
}
pub fn payloads(c: &BatchContext) -> Result<Vec<Value>> {
    let mut rows = Vec::new();
    for s in array(&c.manifest, "selection")? {
        rows.extend(array(s, "payload_files")?.iter().cloned());
    }
    rows.sort_by_key(|p| {
        (
            p["item_ref"].as_str().unwrap_or("").to_owned(),
            p["file_ref"].as_str().unwrap_or("").to_owned(),
            destination_ref(p).unwrap_or_default(),
        )
    });
    Ok(rows)
}
pub fn payload_path(root: &Path, p: &Value) -> Result<PathBuf> {
    let item = text(p, "item_root_ref")?
        .strip_prefix("ToS/source-witnesses/")
        .ok_or("payload Item outside source witnesses")?;
    let relative = text(p, "relative_path")?;
    if !relative.starts_with("payload/") {
        return Err("payload path must begin with payload/".into());
    }
    path_under(root, &format!("{item}/{relative}"))
}
pub fn git_blob_sha1(body: &[u8]) -> String {
    let mut hash = Sha1::new();
    hash.update(format!("blob {}\0", body.len()).as_bytes());
    hash.update(body);
    format!("{:x}", hash.finalize())
}

struct RootedRegular {
    root_path: PathBuf,
    directories: Vec<File>,
    directory_names: Vec<OsString>,
    leaf: OsString,
    file: File,
}

impl RootedRegular {
    fn open(root: &Path, path: &Path, cap: u64) -> Result<Self> {
        let relative = path
            .strip_prefix(root)
            .map_err(|_| format!("selected file is outside custody root: {}", path.display()))?;
        let parts = relative
            .components()
            .map(|component| match component {
                Component::Normal(part) => Ok(part.to_owned()),
                _ => Err(format!("invalid custody-relative file: {}", path.display())),
            })
            .collect::<Result<Vec<_>>>()?;
        let leaf = parts
            .last()
            .cloned()
            .ok_or_else(|| format!("selected file is the custody root: {}", path.display()))?;
        let directory_names = parts[..parts.len() - 1].to_vec();
        let root_fd = tos_fd_open::open_absolute_directory(root).map_err(|error| {
            format!("cannot open custody root without following symlinks: {root:?}: {error}")
        })?;
        let mut directories = vec![root_fd];
        for name in &directory_names {
            let parent = directories.last().expect("root directory retained");
            let child =
                tos_fd_open::open_directory_at(parent, Path::new(name)).map_err(|error| {
                    format!(
                        "symlink or invalid ancestor in custody path: {}: {error}",
                        path.display()
                    )
                })?;
            directories.push(child);
        }
        let parent = directories.last().expect("root directory retained");
        let file = tos_fd_open::open_regular_at(parent, Path::new(&leaf)).map_err(|error| {
            format!(
                "cannot open payload under custody root: {}: {error}",
                path.display()
            )
        })?;
        let size = file.metadata().map_err(|error| error.to_string())?.len();
        if size > cap {
            return Err(format!(
                "payload exceeds bounded file limit: {}",
                path.display()
            ));
        }
        Ok(Self {
            root_path: root.to_owned(),
            directories,
            directory_names,
            leaf,
            file,
        })
    }

    fn recheck_route(&self, expected_file: &Metadata, path: &Path) -> Result<()> {
        let visible_root = tos_fd_open::open_absolute_directory(&self.root_path)
            .map_err(|error| format!("custody root changed while reading: {error}"))?;
        let held_root = self.directories[0]
            .metadata()
            .map_err(|error| error.to_string())?;
        let named_root = visible_root.metadata().map_err(|error| error.to_string())?;
        if held_root.dev() != named_root.dev() || held_root.ino() != named_root.ino() {
            return Err(format!(
                "custody root changed while reading: {}",
                self.root_path.display()
            ));
        }

        for (index, name) in self.directory_names.iter().enumerate() {
            let named = tos_fd_open::open_directory_at(&self.directories[index], Path::new(name))
                .map_err(|error| {
                format!(
                    "custody path changed while reading: {}: {error}",
                    path.display()
                )
            })?;
            let named_meta = named.metadata().map_err(|error| error.to_string())?;
            let held_meta = self.directories[index + 1]
                .metadata()
                .map_err(|error| error.to_string())?;
            if named_meta.dev() != held_meta.dev() || named_meta.ino() != held_meta.ino() {
                return Err(format!(
                    "custody path changed while reading: {}",
                    path.display()
                ));
            }
        }

        let named_file = tos_fd_open::open_regular_at(
            self.directories.last().expect("root directory retained"),
            Path::new(&self.leaf),
        )
        .map_err(|error| {
            format!(
                "payload path changed while reading: {}: {error}",
                path.display()
            )
        })?;
        let named_meta = named_file.metadata().map_err(|error| error.to_string())?;
        if identity(expected_file) != identity(&named_meta) {
            return Err(format!(
                "payload path changed while reading: {}",
                path.display()
            ));
        }
        Ok(())
    }
}

fn digest_open_file(
    file: &mut File,
    path: &Path,
    before: &Metadata,
    mode: Option<u32>,
    expected_owner_uid: Option<u32>,
    single: bool,
    cap: u64,
    capture_bytes: bool,
) -> Result<(Value, Option<Vec<u8>>)> {
    if before.len() > cap {
        return Err(format!(
            "payload exceeds bounded file limit: {}",
            path.display()
        ));
    }
    if mode.is_some_and(|wanted| before.mode() & 0o7777 != wanted)
        || expected_owner_uid.is_some_and(|wanted| before.uid() != wanted)
        || single && before.nlink() != 1
    {
        return Err(format!(
            "file mode, owner or hard-link conflict: {}",
            path.display()
        ));
    }
    let mut sha256 = Digest256Hasher::new();
    let mut git = Sha1::new();
    git.update(format!("blob {}\0", before.len()).as_bytes());
    let mut total = 0u64;
    let mut block = [0u8; 65536];
    let mut bytes = capture_bytes.then(Vec::new);
    loop {
        let count = file.read(&mut block).map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count as u64)
            .ok_or("file byte count overflow")?;
        if total > cap {
            return Err(format!(
                "payload exceeds bounded file limit: {}",
                path.display()
            ));
        }
        sha256.update(&block[..count]);
        git.update(&block[..count]);
        if let Some(bytes) = &mut bytes {
            bytes.extend_from_slice(&block[..count]);
        }
    }
    let after = file.metadata().map_err(|error| error.to_string())?;
    if identity(before) != identity(&after) || total != before.len() {
        return Err(format!("file changed during readback: {}", path.display()));
    }
    Ok((
        json!({
            "byte_size": total,
            "sha256": sha256.finalize().to_hex(),
            "git_blob_sha1": format!("{:x}", git.finalize()),
        }),
        bytes,
    ))
}

fn digest_rooted_file(
    root: &Path,
    path: &Path,
    mode: Option<u32>,
    expected_owner_uid: Option<u32>,
    single: bool,
    cap: u64,
    capture_bytes: bool,
) -> Result<(Value, Option<Vec<u8>>)> {
    let mut selected = RootedRegular::open(root, path, cap)?;
    let before = selected
        .file
        .metadata()
        .map_err(|error| error.to_string())?;
    let result = digest_open_file(
        &mut selected.file,
        path,
        &before,
        mode,
        expected_owner_uid,
        single,
        cap,
        capture_bytes,
    )?;
    let after = selected
        .file
        .metadata()
        .map_err(|error| error.to_string())?;
    selected.recheck_route(&after, path)?;
    Ok(result)
}

#[cfg(test)]
mod rooted_file_tests {
    use super::*;
    use std::os::unix::fs::symlink;

    #[test]
    fn held_rooted_file_rejects_intermediate_replacement_after_open() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "tos-acquisition-rooted-{}-{nonce}",
            std::process::id()
        ));
        let selected_directory = root.join("nested/selected");
        fs::create_dir_all(&selected_directory).unwrap();
        let path = selected_directory.join("payload.bin");
        fs::write(&path, b"held descriptor fixture").unwrap();
        let opened = RootedRegular::open(&root, &path, 1024).unwrap();
        let hashed_inode = opened.file.metadata().unwrap();

        let moved = root.join("moved-selected");
        fs::rename(&selected_directory, &moved).unwrap();
        symlink(&moved, &selected_directory).unwrap();
        assert!(opened.recheck_route(&hashed_inode, &path).is_err());

        drop(opened);
        let _ = fs::remove_dir_all(&root);
    }
}

/// Read and hash one file through a held custody-root descriptor.
///
/// Every parent directory remains open during the read. Before returning,
/// the named root and every selected component are reopened from their held
/// parents and checked against those descriptors and the hashed inode.
pub fn read_bytes_under(
    root: &Path,
    path: &Path,
    mode: Option<u32>,
    expected_owner_uid: Option<u32>,
    single: bool,
    cap: u64,
) -> Result<(Vec<u8>, Value)> {
    let (digest, bytes) =
        digest_rooted_file(root, path, mode, expected_owner_uid, single, cap, true)?;
    Ok((bytes.ok_or("rooted read omitted bytes")?, digest))
}

/// Hash one file through a held custody-root descriptor without buffering it.
pub fn digest_file_under(
    root: &Path,
    path: &Path,
    mode: Option<u32>,
    expected_owner_uid: Option<u32>,
    single: bool,
    cap: u64,
) -> Result<Value> {
    digest_rooted_file(root, path, mode, expected_owner_uid, single, cap, false)
        .map(|(digest, _)| digest)
}

pub fn digest_file(
    path: &Path,
    mode: Option<u32>,
    owner: bool,
    single: bool,
    cap: u64,
) -> Result<Value> {
    let mut file = tos_fd_open::open_absolute_regular(path, cap).map_err(|e| e.to_string())?;
    let before = file.metadata().map_err(|e| e.to_string())?;
    let expected_owner_uid = owner.then(|| rustix::process::geteuid().as_raw());
    let (digest, _) = digest_open_file(
        &mut file,
        path,
        &before,
        mode,
        expected_owner_uid,
        single,
        cap,
        false,
    )?;
    let after = file.metadata().map_err(|e| e.to_string())?;
    let current = tos_fd_open::open_absolute_regular(path, cap)
        .map_err(|e| e.to_string())?
        .metadata()
        .map_err(|e| e.to_string())?;
    if identity(&after) != identity(&current) {
        return Err(format!("file changed during readback: {}", path.display()));
    }
    Ok(digest)
}
pub fn verify_destination(path: &Path, p: &Value) -> Result<Value> {
    let digest = digest_file(path, Some(0o444), true, true, 300 * 1024 * 1024)?;
    if digest["byte_size"] != p["byte_size"]
        || digest["sha256"] != p["sha256"]
        || p.get("git_blob_sha1")
            .and_then(Value::as_str)
            .is_some_and(|v| Some(v) != digest["git_blob_sha1"].as_str())
    {
        return Err("destination fixity differs".into());
    }
    Ok(digest)
}
pub fn provenance_delta_ref(c: &BatchContext) -> String {
    format!(
        "source/ToS/source-witnesses/discovery/acquisition-batches/{}/provenance-delta.json",
        c.manifest["batch_id"]
            .as_str()
            .unwrap_or("")
            .trim_start_matches("tos.acquisition-batch.")
    )
}
fn delta(c: &BatchContext) -> Result<Value> {
    let v = &c.manifest;
    let d = &v["provenance_delta"];
    Ok(
        json!({"$schema":"https://tree-of-sophia.local/ToS/contracts/acquisition-provenance-delta.schema.json","schema_version":"tos_acquisition_provenance_delta_v1","event_ref":d["event_ref"],"event_version":d["event_version"],"change_kind":d["change_kind"],"batch_id":v["batch_id"],"batch_revision":v["batch_revision"],"base_revision":v["base_revision"],"selection_manifest_ref":"manifest.json","selection_manifest_sha256":c.manifest_sha256,"record_refs":d["record_refs"],"payload_file_refs":d["payload_file_refs"],"item_refs":array(v,"selection")?.iter().map(|s|s["item_ref"].clone()).collect::<Vec<_>>(),"supersedes_event_ref":d.get("supersedes_event_ref").unwrap_or(&Value::Null),"materialization":"apply-selected-records-and-custody-to-the-bound-base","admission_status":"not-admitted"}),
    )
}
fn source_files(root: &Path) -> Result<BTreeSet<String>> {
    fn walk(root: &Path, p: &Path, out: &mut BTreeSet<String>) -> Result<()> {
        tos_fd_open::open_absolute_directory(p).map_err(|e| e.to_string())?;
        for e in fs::read_dir(p).map_err(|e| e.to_string())? {
            let p = e.map_err(|e| e.to_string())?.path();
            let m = fs::symlink_metadata(&p).map_err(|e| e.to_string())?;
            if m.is_dir() {
                walk(root, &p, out)?;
            } else if m.is_file() {
                out.insert(
                    p.strip_prefix(root)
                        .map_err(|e| e.to_string())?
                        .to_str()
                        .ok_or("path UTF8")?
                        .to_owned(),
                );
            } else {
                return Err("custody contains symlink or special file".into());
            }
        }
        Ok(())
    }
    let mut out = BTreeSet::new();
    walk(root, root, &mut out)?;
    Ok(out)
}
pub fn verify_prepared_output(c: &BatchContext, out: &Path, private: bool) -> Result<()> {
    if private {
        for p in [
            out.to_owned(),
            out.join("source"),
            out.join("payload"),
            out.join("receipts"),
        ] {
            private_root(&p)?;
        }
    }
    if read_bytes(
        &out.join("manifest.json"),
        None,
        false,
        false,
        16 * 1024 * 1024,
    )? != c.raw_manifest
    {
        return Err("prepared output manifest differs".into());
    }
    let receipt = read_json(&out.join("receipts/preparation.json"))?;
    for key in ["batch_id", "batch_revision", "base_revision"] {
        if receipt[key] != c.manifest[key] {
            return Err(format!("preparation receipt {key} differs"));
        }
    }
    if receipt["schema_version"] != "tos_acquisition_preparation_receipt_v1"
        || receipt["manifest_ref"] != "manifest.json"
        || receipt["manifest_sha256"] != c.manifest_sha256
        || receipt["topology_preimages"] != 0
    {
        return Err("preparation receipt does not bind frozen selection".into());
    }
    let refs = records(c)?;
    let mut expected = refs
        .iter()
        .map(|(_, r)| text(r, "ref").map(str::to_owned))
        .collect::<Result<BTreeSet<_>>>()?;
    expected.insert(
        provenance_delta_ref(c)
            .trim_start_matches("source/")
            .to_owned(),
    );
    if source_files(&out.join("source"))? != expected {
        return Err("prepared source closure differs".into());
    }
    let rows = array(&receipt, "records")?;
    if rows.len() != refs.len() || receipt["record_count"].as_u64() != Some(refs.len() as u64) {
        return Err("preparation receipt record count differs".into());
    }
    let mut row_map = BTreeMap::new();
    for r in rows {
        if row_map.insert(text(r, "ref")?.to_owned(), r).is_some() {
            return Err("preparation receipt repeats record".into());
        }
    }
    let mut total = 0;
    for (_, r) in refs {
        let name = text(&r, "ref")?;
        let b = read_bytes(
            &path_under(&out.join("source"), name)?,
            Some(0o644),
            false,
            false,
            16 * 1024 * 1024,
        )?;
        if sha(&b) != text(&r, "sha256")? {
            return Err("prepared selected record digest differs".into());
        }
        let row = row_map.get(name).ok_or("preparation record missing")?;
        for k in ["ref", "kind", "sha256"] {
            if row[k] != r[k] {
                return Err(format!("preparation record {k} differs"));
            }
        }
        if row["handoff_ref"] != format!("source/{name}")
            || row["byte_size"].as_u64() != Some(b.len() as u64)
        {
            return Err("preparation record binding differs".into());
        }
        total += b.len();
    }
    if receipt["selected_record_bytes"].as_u64() != Some(total as u64) {
        return Err("preparation byte total differs".into());
    }
    let delta_ref = provenance_delta_ref(c);
    let body = read_bytes(
        &path_under(out, &delta_ref)?,
        None,
        false,
        false,
        16 * 1024 * 1024,
    )?;
    if receipt["provenance_delta_ref"] != delta_ref
        || receipt["provenance_delta_sha256"] != sha(&body)
        || body != canonical(&delta(c)?)?
    {
        return Err("prepared provenance delta differs".into());
    }
    crate::source_acquisition_contract::validate_schema(
        &c.repo_root,
        "ToS/contracts/acquisition-provenance-delta.schema.json",
        &parse(&body)?,
    )?;
    crate::source_acquisition_contract::verify_item_bindings(
        &c.repo_root,
        &out.join("source"),
        &c.manifest,
    )?;
    Ok(())
}
fn prepare(c: &BatchContext, metadata: &Path, out: &Path) -> Result<Value> {
    if fs::symlink_metadata(out).is_ok() {
        return Err("preparation output must be new".into());
    }
    let selected = records(c)?;
    for (_, r) in &selected {
        read_bytes(
            &path_under(metadata, text(r, "ref")?)?,
            Some(0o644),
            false,
            false,
            16 * 1024 * 1024,
        )?;
    }
    let held = mkdir(out, 0o700)?;
    for name in ["source", "payload", "receipts"] {
        mkdir(&out.join(name), 0o700)?;
    }
    same_dir(out, &held)?;
    publish(&out.join("manifest.json"), &c.raw_manifest, 0o644)?;
    let mut rows = Vec::new();
    for (_, r) in selected {
        let name = text(&r, "ref")?;
        let body = read_bytes(
            &path_under(metadata, name)?,
            Some(0o644),
            false,
            false,
            16 * 1024 * 1024,
        )?;
        if sha(&body) != text(&r, "sha256")? {
            return Err("selected record digest differs".into());
        }
        let status = publish(&path_under(&out.join("source"), name)?, &body, 0o644)?;
        rows.push(json!({"ref":name,"handoff_ref":format!("source/{name}"),"kind":r["kind"],"sha256":r["sha256"],"byte_size":body.len(),"status":status}));
    }
    let delta_ref = provenance_delta_ref(c);
    let delta_value = delta(c)?;
    crate::source_acquisition_contract::validate_schema(
        &c.repo_root,
        "ToS/contracts/acquisition-provenance-delta.schema.json",
        &delta_value,
    )?;
    let body = canonical(&delta_value)?;
    publish(&path_under(out, &delta_ref)?, &body, 0o644)?;
    let total: u64 = rows
        .iter()
        .map(|r| r["byte_size"].as_u64().unwrap_or(0))
        .sum();
    let receipt = json!({"schema_version":"tos_acquisition_preparation_receipt_v1","batch_id":c.manifest["batch_id"],"batch_revision":c.manifest["batch_revision"],"manifest_ref":"manifest.json","manifest_sha256":c.manifest_sha256,"base_revision":c.manifest["base_revision"],"record_count":rows.len(),"selected_record_bytes":total,"records":rows,"provenance_delta_ref":delta_ref,"provenance_delta_sha256":sha(&body),"topology_preimages":0,"storage_model":"batch_delta_without_per_target_topology_preimages","payload_acquisition_performed":false,"admission_status":"not-admitted","authority_boundary":"selection and metadata custody only; no semantic, rights, canon, corpus admission, R2, publication, or deployment acceptance"});
    publish(
        &out.join("receipts/preparation.json"),
        &canonical(&receipt)?,
        0o644,
    )?;
    verify_prepared_output(c, out, true)?;
    Ok(
        json!({"status":"prepared-not-acquired","batch_id":c.manifest["batch_id"],"manifest_sha256":c.manifest_sha256,"output_root":out,"record_count":rows.len(),"selected_record_bytes":total,"provenance_delta":delta_ref}),
    )
}
fn lock(out: &Path) -> Result<File> {
    let parent = out.parent().ok_or("output parent missing")?;
    let d = tos_fd_open::open_absolute_directory(parent).map_err(|e| e.to_string())?;
    let name = format!(
        ".{}.acquisition.lock",
        out.file_name().ok_or("output name")?.to_string_lossy()
    );
    let fd = rustix::fs::openat(
        &d,
        name.as_str(),
        OFlags::RDWR
            | OFlags::CREATE
            | OFlags::APPEND
            | OFlags::NOFOLLOW
            | OFlags::NONBLOCK
            | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o600),
    )
    .map_err(|e| e.to_string())?;
    let f = File::from(fd);
    let meta = f.metadata().map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.nlink() != 1 || meta.uid() != rustix::process::geteuid().as_raw() {
        return Err("batch lock type/owner/hard-link conflict".into());
    }
    rustix::fs::flock(&f, FlockOperation::LockExclusive).map_err(|e| e.to_string())?;
    let current = tos_fd_open::open_absolute_regular(&parent.join(name), 1024)
        .map_err(|e| e.to_string())?
        .metadata()
        .map_err(|e| e.to_string())?;
    if identity(&current) != identity(&meta) {
        return Err("batch lock pathname changed after acquiring lock".into());
    }
    same_dir(parent, &d)?;
    Ok(f)
}
fn recover(c: &BatchContext, out: &Path) -> Result<()> {
    for name in ["source", "payload", "receipts"] {
        private_root(&out.join(name))?;
    }
    private_root(out)?;
    if read_bytes(
        &out.join("manifest.json"),
        None,
        false,
        false,
        16 * 1024 * 1024,
    )? != c.raw_manifest
    {
        return Err("partial preparation manifest differs".into());
    }
    let allowed: BTreeSet<_> = ["manifest.json", "source", "payload", "receipts"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    for e in fs::read_dir(out).map_err(|e| e.to_string())? {
        let e = e.map_err(|e| e.to_string())?;
        if !allowed.contains(&e.file_name().to_string_lossy().to_string()) {
            return Err("partial preparation contains foreign evidence".into());
        }
    }
    let mut expected = records(c)?
        .iter()
        .map(|(_, r)| text(r, "ref").map(str::to_owned))
        .collect::<Result<BTreeSet<_>>>()?;
    expected.insert(provenance_delta_ref(c).trim_start_matches("source/").into());
    if !source_files(&out.join("source"))?.is_subset(&expected)
        || fs::read_dir(out.join("payload"))
            .map_err(|e| e.to_string())?
            .next()
            .is_some()
        || fs::read_dir(out.join("receipts"))
            .map_err(|e| e.to_string())?
            .next()
            .is_some()
    {
        return Err("partial preparation has payload, receipt or foreign source evidence".into());
    }
    let mut allowed_files: BTreeSet<String> = expected
        .into_iter()
        .map(|r| format!("source/{r}"))
        .collect();
    allowed_files.insert("manifest.json".into());
    let mut allowed_dirs: BTreeSet<String> = ["source", "payload", "receipts"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    for file in &allowed_files {
        let mut p = Path::new(file).parent();
        while let Some(dir) = p {
            if !dir.as_os_str().is_empty() {
                allowed_dirs.insert(dir.to_str().ok_or("recovery path UTF8")?.to_owned());
            }
            p = dir.parent();
        }
    }
    fn remove_tree(
        root: &Path,
        p: &Path,
        files: &BTreeSet<String>,
        dirs: &BTreeSet<String>,
    ) -> Result<()> {
        let d = tos_fd_open::open_absolute_directory(p).map_err(|e| e.to_string())?;
        for entry in fs::read_dir(p).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            let reference = path
                .strip_prefix(root)
                .map_err(|e| e.to_string())?
                .to_str()
                .ok_or("recovery path UTF8")?
                .to_owned();
            let info = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
            same_dir(p, &d)?;
            if info.is_dir() && dirs.contains(&reference) {
                remove_tree(root, &path, files, dirs)?;
                same_dir(p, &d)?;
                rustix::fs::unlinkat(&d, entry.file_name(), AtFlags::REMOVEDIR)
                    .map_err(|e| e.to_string())?;
            } else if info.is_file() && files.contains(&reference) {
                rustix::fs::unlinkat(&d, entry.file_name(), AtFlags::empty())
                    .map_err(|e| e.to_string())?;
            } else {
                return Err(
                    "foreign evidence appeared during interrupted preparation recovery".into(),
                );
            }
        }
        same_dir(p, &d)?;
        Ok(())
    }
    remove_tree(out, out, &allowed_files, &allowed_dirs)?;
    fs::remove_dir(out).map_err(|e| e.to_string())?;
    Ok(())
}
pub fn jsonl(path: &Path, local: bool) -> Result<Vec<Value>> {
    match fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.to_string()),
        Ok(_) => {}
    }
    let raw = read_bytes(path, None, local, local, 64 * 1024 * 1024)?;
    let raw = std::str::from_utf8(&raw).map_err(|_| "JSONL is not valid UTF-8")?;
    raw.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let v = parse(l.as_bytes())?;
            if !v.is_object() {
                return Err("JSONL rows must be objects".into());
            }
            Ok(v)
        })
        .collect()
}
fn append(path: &Path, row: &Value) -> Result<()> {
    let parent = path.parent().ok_or("journal parent")?;
    let d = tos_fd_open::open_absolute_directory(parent).map_err(|e| e.to_string())?;
    let fd = rustix::fs::openat(
        &d,
        path.file_name().ok_or("journal name")?,
        OFlags::WRONLY
            | OFlags::CREATE
            | OFlags::APPEND
            | OFlags::NOFOLLOW
            | OFlags::NONBLOCK
            | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o644),
    )
    .map_err(|e| e.to_string())?;
    let mut f = File::from(fd);
    let m = f.metadata().map_err(|e| e.to_string())?;
    if !m.is_file() || m.nlink() != 1 || m.uid() != rustix::process::geteuid().as_raw() {
        return Err("acquisition journal type/owner/hard-link conflict".into());
    }
    rustix::fs::flock(&f, FlockOperation::LockExclusive).map_err(|e| e.to_string())?;
    same_dir(parent, &d)?;
    let current = tos_fd_open::open_absolute_regular(path, 64 * 1024 * 1024)
        .map_err(|e| e.to_string())?
        .metadata()
        .map_err(|e| e.to_string())?;
    if current.dev() != m.dev() || current.ino() != m.ino() {
        return Err("journal pathname changed".into());
    }
    f.write_all(&canonical(row)?).map_err(|e| e.to_string())?;
    f.sync_all().map_err(|e| e.to_string())?;
    Ok(())
}
fn now() -> Result<String> {
    crate::source_serialization::instant().map_err(|e| format!("clock: {e:?}"))
}
fn key(p: &Value) -> Result<(String, String, String)> {
    Ok((
        text(p, "item_ref")?.into(),
        text(p, "file_ref")?.into(),
        if p.get("destination_ref").is_some() {
            text(p, "destination_ref")?.into()
        } else {
            destination_ref(p)?
        },
    ))
}
pub fn fetch_request(payload: &Value) -> Result<Vec<u8>> {
    use base64::Engine;
    use std::io::BufRead;
    let mut output = std::io::stdout().lock();
    output
        .write_all(&canonical(&json!({"kind":"fetch","payload":payload}))?)
        .map_err(|e| e.to_string())?;
    output.flush().map_err(|e| e.to_string())?;
    let mut line = String::new();
    std::io::stdin()
        .lock()
        .take(401 * 1024 * 1024)
        .read_line(&mut line)
        .map_err(|e| e.to_string())?;
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct FetchReply {
        body_base64: Option<String>,
        error: Option<String>,
    }
    let value: FetchReply = serde_json::from_str(&line).map_err(|e| e.to_string())?;
    match (value.body_base64, value.error) {
        (None, Some(error)) if error.len() <= 4096 => Err(error),
        (Some(body), None) => {
            let decoded = base64::engine::general_purpose::STANDARD
                .decode(body)
                .map_err(|e| e.to_string())?;
            if decoded.len() > 300 * 1024 * 1024 {
                return Err("fixture response exceeds bounded payload limit".into());
            }
            Ok(decoded)
        }
        _ => Err("invalid native fixture fetch reply".into()),
    }
}
pub fn fetch_url(payload: &Value) -> Result<Vec<u8>> {
    let size = payload["byte_size"].as_u64().ok_or("payload byte size")?;
    if size > 300 * 1024 * 1024 {
        return Err("payload exceeds bounded transfer limit".into());
    }
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let cap = size.checked_add(1).ok_or("payload size overflow")? as usize;
    crate::source_text_owner_ocr::bounded_process(
        "/usr/bin/curl",
        &[
            "-q",
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--max-redirs",
            "10",
            "--proto",
            "=http,https",
            "--proto-redir",
            "=http,https",
            "--max-time",
            "45",
            "--user-agent",
            "Tree-of-Sophia-bounded-acquisition/1",
            "--url",
            text(payload, "provider_url")?,
        ],
        None,
        cap,
        std::time::Instant::now() + std::time::Duration::from_secs(46),
        &cancel,
    )
    .map_err(|e| format!("provider fetch failed: {e:?}"))
}
fn fixity(c: &BatchContext, out: &Path, run: &str) -> Result<(Vec<Value>, String, String)> {
    let mut rows = Vec::new();
    for p in payloads(c)? {
        let mut row = json!({"item_ref":p["item_ref"],"file_ref":p["file_ref"],"destination_ref":destination_ref(&p)?,"relative_path":p["relative_path"],"provider_revision":p["provider_revision"],"provider_source_id":p["provider_source_id"],"expected_byte_size":p["byte_size"],"expected_sha256":p["sha256"]});
        match payload_path(&out.join("payload"), &p).and_then(|path| verify_destination(&path, &p))
        {
            Ok(v) => {
                for (k, v) in v.as_object().ok_or("digest object")? {
                    row[k] = v.clone();
                }
                row["status"] = json!("verified");
            }
            Err(e) => {
                row["status"] = json!("missing-or-invalid");
                row["error"] = json!(e);
            }
        }
        rows.push(row);
    }
    let body = rows
        .iter()
        .map(canonical)
        .collect::<Result<Vec<_>>>()?
        .concat();
    let reference = format!("receipts/fixity-{run}.jsonl");
    publish(&out.join(&reference), &body, 0o644)?;
    let good = rows.iter().filter(|r| r["status"] == "verified").count();
    let summary = json!({"schema_version":"tos_acquisition_independent_fixity_v1","batch_id":c.manifest["batch_id"],"manifest_sha256":c.manifest_sha256,"run_id":run,"fixity_jsonl_ref":reference,"fixity_jsonl_sha256":sha(&body),"rows":rows.len(),"verified":good,"invalid":rows.len()-good,"independent_pass":true,"authority_boundary":"independent byte/readback fixity only; no admission or semantic acceptance"});
    let summary_ref = format!("receipts/fixity-{run}.json");
    publish(&out.join(&summary_ref), &canonical(&summary)?, 0o644)?;
    Ok((rows, reference, summary_ref))
}
fn acquire(
    c: &BatchContext,
    metadata: &Path,
    out: &Path,
    attempts: u64,
    fetch: &mut impl FnMut(&Value) -> Result<Vec<u8>>,
) -> Result<Value> {
    if attempts == 0 || attempts > 100 {
        return Err("max_attempts must be positive and bounded".into());
    }
    if fs::symlink_metadata(out).is_ok() && !out.join("receipts/preparation.json").is_file() {
        recover(c, out)?;
    }
    if fs::symlink_metadata(out).is_err() {
        prepare(c, metadata, out)?;
    }
    verify_prepared_output(c, out, true)?;
    let journal = out.join("receipts/acquisition.jsonl");
    let mut previous = BTreeMap::new();
    for row in jsonl(&journal, true)? {
        let attempt = row["attempt"]
            .as_u64()
            .ok_or("acquisition journal attempt must be a nonnegative integer")?;
        if let Ok(k) = key(&row) {
            let n = previous.entry(k).or_insert(0);
            *n = std::cmp::max(*n, attempt);
        }
    }
    let stamp = now()?.replace(['-', ':', '.'], "");
    let mut run = stamp.clone();
    let mut suffix = 0;
    while out.join(format!("receipts/handoff-{run}.json")).exists() {
        suffix += 1;
        run = format!("{stamp}-{suffix}");
    }
    let mut custody = Vec::new();
    for p in payloads(c)? {
        let k = key(&p)?;
        let prev = *previous.get(&k).unwrap_or(&0);
        let base = json!({"run_id":run,"batch_id":c.manifest["batch_id"],"manifest_sha256":c.manifest_sha256,"item_ref":p["item_ref"],"file_ref":p["file_ref"],"destination_ref":destination_ref(&p)?,"provider_url":p["provider_url"],"provider_revision":p["provider_revision"],"provider_source_id":p["provider_source_id"],"expected_byte_size":p["byte_size"],"expected_sha256":p["sha256"]});
        let path = payload_path(&out.join("payload"), &p);
        let retained = path
            .as_ref()
            .ok()
            .filter(|p| fs::symlink_metadata(p).is_ok());
        let result = if let Some(path) = retained {
            Some(verify_destination(path, &p).map(|_| "already_present".to_owned()))
        } else if let Err(e) = &path {
            Some(Err(e.clone()))
        } else {
            None
        };
        if let Some(result) = result {
            let mut row = base.clone();
            row["attempt"] = json!(prev);
            row["completed_at"] = json!(now()?);
            match result {
                Ok(s) => row["status"] = json!(s),
                Err(e) => {
                    row["status"] = json!("conflict");
                    row["error"] = json!(e);
                }
            }
            append(&journal, &row)?;
            custody.push(row);
            continue;
        }
        let path = path?;
        for attempt in 1..=attempts {
            let mut failure_type = "SourceFetchError";
            let result = fetch(&p).and_then(|body| {
                failure_type = "SourceIntegrityError";
                if body.len() as u64 != p["byte_size"].as_u64().ok_or("payload byte_size")?
                    || sha(&body) != text(&p, "sha256")?
                {
                    return Err("provider bytes differ from frozen selection".into());
                }
                let git = git_blob_sha1(&body);
                if p.get("git_blob_sha1")
                    .and_then(Value::as_str)
                    .is_some_and(|v| v != git)
                {
                    return Err("provider Git blob differs".into());
                }
                let status = publish(&path, &body, 0o444)?;
                verify_destination(&path, &p)?;
                Ok(if status == "copied" {
                    "acquired"
                } else {
                    "already_present"
                })
            });
            let mut row = base.clone();
            row["attempt"] = json!(prev.checked_add(attempt).ok_or("attempt overflow")?);
            row["completed_at"] = json!(now()?);
            match &result {
                Ok(s) => {
                    row["status"] = json!(s);
                    row["readback_verified"] = json!(true);
                }
                Err(e) => {
                    row["status"] = json!("failed");
                    row["error"] = json!(e);
                    row["failure_type"] = json!(failure_type);
                }
            }
            append(&journal, &row)?;
            if result.is_ok() || attempt == attempts {
                custody.push(row);
                break;
            }
        }
    }
    verify_prepared_output(c, out, true)?;
    let (rows, fixity_ref, summary_ref) = fixity(c, out, &run)?;
    verify_prepared_output(c, out, true)?;
    let verified = rows.iter().filter(|r| r["status"] == "verified").count();
    let status = if verified == rows.len() {
        "acquired-not-admitted"
    } else if verified > 0 {
        "partially-acquired-not-admitted"
    } else {
        "prepared-not-acquired"
    };
    let mut source_rows = Vec::new();
    for (s, r) in records(c)? {
        let reference = text(&r, "ref")?;
        let body = read_bytes(
            &path_under(&out.join("source"), reference)?,
            Some(0o644),
            false,
            false,
            16 * 1024 * 1024,
        )?;
        source_rows.push(json!({"item_ref":s["item_ref"],"ref":reference,"handoff_ref":format!("source/{reference}"),"kind":r["kind"],"sha256":r["sha256"],"byte_size":body.len(),"rights_ref":s["rights"]["ref"],"rights_sha256":s["rights"]["sha256"]}));
    }
    let fsha = sha(&read_bytes(
        &out.join(&fixity_ref),
        None,
        false,
        false,
        64 * 1024 * 1024,
    )?);
    let handoff = json!({"schema_version":"tos_acquisition_handoff_v1","batch_id":c.manifest["batch_id"],"batch_revision":c.manifest["batch_revision"],"run_id":run,"input_selection":{"ref":"manifest.json","sha256":c.manifest_sha256},"base_revision":c.manifest["base_revision"],"source_records":source_rows,"payload_custody":custody,"independent_fixity":{"ref":fixity_ref,"summary_ref":summary_ref,"sha256":fsha,"jsonl_sha256":fsha,"summary_sha256":sha(&read_bytes(&out.join(&summary_ref),None,false,false,16*1024*1024)?)},"provenance_delta":{"ref":provenance_delta_ref(c),"sha256":sha(&read_bytes(&out.join(provenance_delta_ref(c)),None,false,false,16*1024*1024)?),"event_ref":c.manifest["provenance_delta"]["event_ref"],"base_revision":c.manifest["base_revision"]},"acquisition_status":status,"admission_status":"not-admitted","publication_status":"not-published","rights_posture":"preserved per selected Item rights record","topology_preimages":0,"source_failure_isolation":true,"restartable":true,"authority_boundary":"exact reviewed record and local payload custody handoff; no corpus admission, R2 transfer, publication, canon, semantic, or deployment acceptance"});
    let handoff_ref = format!("receipts/handoff-{run}.json");
    publish(&out.join(&handoff_ref), &canonical(&handoff)?, 0o644)?;
    Ok(
        json!({"status":status,"batch_id":c.manifest["batch_id"],"manifest_sha256":c.manifest_sha256,"handoff_ref":handoff_ref,"fixity_ref":fixity_ref,"payload_count":rows.len(),"verified_payload_count":verified,"failed_payload_count":rows.len()-verified,"admission_status":"not-admitted"}),
    )
}
pub fn invoke(request: &Value) -> Result<Value> {
    let repo = Path::new(text(request, "repo_root")?);
    let operation = text(request, "operation")?;
    if operation == "measure_storage" {
        let out = Path::new(text(request, "output_root")?);
        let files = source_files(out)?;
        let mut metadata = 0;
        let mut metadata_bytes = 0;
        let mut payload = 0;
        let mut payload_bytes = 0;
        let mut preimage = 0;
        let mut preimage_bytes = 0;
        for name in files {
            let size = fs::symlink_metadata(out.join(&name))
                .map_err(|e| e.to_string())?
                .len();
            if name.starts_with("source/") {
                metadata += 1;
                metadata_bytes += size;
            }
            if name.starts_with("payload/") {
                payload += 1;
                payload_bytes += size;
            }
            if name.split('/').any(|p| p.starts_with("topology-before")) {
                preimage += 1;
                preimage_bytes += size;
            }
        }
        return Ok(
            json!({"schema_version":"tos_acquisition_storage_measurement_v1","metadata_file_count":metadata,"metadata_bytes":metadata_bytes,"payload_file_count":payload,"payload_bytes":payload_bytes,"topology_preimage_count":preimage,"topology_preimage_bytes":preimage_bytes,"storage_model":"selected-records-and-batch-delta"}),
        );
    }
    let out = request
        .get("output_root")
        .and_then(Value::as_str)
        .map(Path::new);
    let path = if operation == "verify_local" {
        out.ok_or("output root required")?.join("manifest.json")
    } else {
        PathBuf::from(text(request, "manifest_path")?)
    };
    let expected = match request.get("expected_manifest_sha256") {
        None | Some(Value::Null) => None,
        Some(value) => Some(
            value
                .as_str()
                .ok_or("expected manifest SHA-256 must be text")?,
        ),
    };
    if matches!(operation, "prepare" | "acquire") && expected.is_none() {
        return Err("requires expected frozen manifest SHA-256".into());
    }
    let c = load_manifest(&path, repo, expected)?;
    match operation {
        "load_manifest" => Ok(
            json!({"repo_root":c.repo_root,"manifest_path":c.manifest_path,"manifest_ref":c.manifest_ref,"manifest_sha256":c.manifest_sha256,"raw_manifest_base64":base64::engine::general_purpose::STANDARD.encode(&c.raw_manifest),"manifest":c.manifest}),
        ),
        "verify_prepared_output" => {
            verify_prepared_output(
                &c,
                out.ok_or("output root")?,
                request
                    .get("require_private_roots")
                    .and_then(Value::as_bool)
                    .unwrap_or(true),
            )?;
            Ok(Value::Null)
        }
        "verify_local" => {
            let out = out.ok_or("output root")?;
            verify_prepared_output(&c, out, true)?;
            let mut rows = Vec::new();
            for p in payloads(&c)? {
                let mut row = json!({"item_ref":p["item_ref"],"file_ref":p["file_ref"],"destination_ref":destination_ref(&p)?});
                match payload_path(&out.join("payload"), &p)
                    .and_then(|path| verify_destination(&path, &p))
                {
                    Ok(v) => {
                        row["status"] = json!("verified");
                        row["byte_size"] = v["byte_size"].clone();
                        row["sha256"] = v["sha256"].clone();
                    }
                    Err(e) => {
                        row["status"] = json!("missing-or-invalid");
                        row["error"] = json!(e);
                    }
                }
                rows.push(row);
            }
            Ok(
                json!({"status":if rows.iter().all(|r|r["status"]=="verified"){"verified"}else{"incomplete"},"batch_id":c.manifest["batch_id"],"manifest_sha256":c.manifest_sha256,"rows":rows,"topology_preimages":0,"admission_status":"not-admitted"}),
            )
        }
        "prepare" | "acquire" => {
            let out = out.ok_or("output root")?;
            if !out.is_absolute() {
                return Err("output root must be absolute".into());
            }
            let _guard = lock(out)?;
            let metadata = Path::new(text(request, "metadata_root")?);
            if operation == "prepare" {
                prepare(&c, metadata, out)
            } else {
                let attempts = match request.get("max_attempts") {
                    None => 2,
                    Some(value) => value
                        .as_u64()
                        .ok_or("max_attempts must be an unsigned integer")?,
                };
                let callback = match request.get("fetch_callback") {
                    None => false,
                    Some(value) => value.as_bool().ok_or("fetch_callback must be boolean")?,
                };
                acquire(&c, metadata, out, attempts, &mut |p| {
                    if callback {
                        fetch_request(p)
                    } else {
                        fetch_url(p)
                    }
                })
            }
        }
        _ => Err("unsupported acquisition batch operation".into()),
    }
}
