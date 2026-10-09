//! Exact bounded source-return carrier from an explicitly selected V1 corpus.
//! Fixity is mechanical evidence; admission, rights and KAG publication remain
//! with their owners. No input is read from the current checkout's ToS tree.
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::CString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use tos_foundation::{
    CanonicalProfile, Digest256, Digest256Hasher, JsonLimits, JsonMode, RelativePath,
    SourceRevision, canonical_bytes_v1, parse_json,
};
use tos_source_store::{CorpusReader, ReadLimits, Selector};

pub const PRIMARY: &str =
    "ToS/canon/source/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/node.json";
pub const SOURCE_PATHS: [&str; 6] = [
    PRIMARY,
    "ToS/derived-exports/README.md",
    "ToS/public-compatibility/concept_node.example.json",
    "ToS/public-compatibility/source_node.example.json",
    "ToS/zarathustra/prologue-1/TRILINGUAL_ENTRY.md",
    "ToS/zarathustra/public-entry/TINY_ENTRY_ROUTE.md",
];
pub const CAPSULE: &str = "ToS/derived-exports/kag_export.min.json";
const SCHEMA: &str = "tos_kag_source_export_v1";
const MAX: u64 = 8 * 1024 * 1024;
static STAGES: AtomicU64 = AtomicU64::new(0);
fn bad(e: impl ToString) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, e.to_string())
}
fn digest(bytes: &[u8]) -> String {
    Digest256::of_bytes(bytes).to_hex()
}
fn hex(v: &Value) -> io::Result<&str> {
    let s = v.as_str().ok_or_else(|| bad("digest must be a string"))?;
    if s.len() != 64
        || !s
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(bad("digest must be lowercase SHA256"));
    }
    Ok(s)
}
fn keys(v: &Value, expected: &[&str]) -> io::Result<()> {
    let actual = v.as_object().ok_or_else(|| bad("expected object"))?;
    if actual.len() != expected.len() || expected.iter().any(|k| !actual.contains_key(*k)) {
        return Err(bad("unexpected object fields"));
    }
    Ok(())
}
fn canonical(v: &Value) -> io::Result<Vec<u8>> {
    let raw = serde_json::to_vec(v).map_err(bad)?;
    let limits = JsonLimits {
        max_bytes: MAX as usize,
        ..JsonLimits::default()
    };
    let parsed = parse_json(&raw, JsonMode::RequestLastWins, limits).map_err(bad)?;
    canonical_bytes_v1(parsed.root(), CanonicalProfile::CorpusSnapshotV1, limits).map_err(bad)
}
fn directory(root: &Path) -> io::Result<()> {
    if !root.is_absolute() || fs::canonicalize(root)? != root || !root.is_dir() {
        return Err(bad(
            "select an explicit regular absolute directory without symlinks",
        ));
    }
    Ok(())
}
fn read(root: &Path, relative: &str, cap: u64) -> io::Result<Vec<u8>> {
    crate::kag_release::budget_check()?;
    let path = root.join(relative);
    if fs::canonicalize(&path)? != path || !fs::symlink_metadata(&path)?.is_file() {
        return Err(bad(
            "source must be a regular file without symlink ancestors",
        ));
    }
    let f = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&path)?;
    if f.metadata()?.len() > cap {
        return Err(bad("source byte budget exceeded"));
    }
    let mut raw = Vec::new();
    f.take(cap + 1).read_to_end(&mut raw)?;
    if raw.len() as u64 > cap {
        return Err(bad("source grew beyond byte budget"));
    }
    Ok(raw)
}
use std::os::unix::fs::OpenOptionsExt;
fn json_file(root: &Path, relative: &str) -> io::Result<Value> {
    let raw = read(root, relative, MAX)?;
    parse_json(
        &raw,
        JsonMode::PublishedStrict,
        JsonLimits {
            max_bytes: MAX as usize,
            ..JsonLimits::default()
        },
    )
    .map_err(bad)?;
    serde_json::from_slice(&raw).map_err(bad)
}
fn inventory(
    root: &Path,
    relative: &Path,
    files: &mut BTreeSet<String>,
    dirs: &mut Vec<PathBuf>,
) -> io::Result<()> {
    crate::kag_release::budget_check()?;
    if relative.components().count() > 32 || files.len() > 4096 || dirs.len() > 4096 {
        return Err(bad("export inventory exceeds finite traversal bound"));
    }
    let path = root.join(relative);
    let metadata = fs::symlink_metadata(&path)?;
    if metadata.is_dir() {
        dirs.push(path.clone());
        for child in fs::read_dir(&path)? {
            let child = child?;
            inventory(root, &relative.join(child.file_name()), files, dirs)?;
        }
    } else if metadata.is_file() {
        files.insert(
            relative
                .to_str()
                .ok_or_else(|| bad("non UTF8 export path"))?
                .to_owned(),
        );
    } else {
        return Err(bad("export contains symlink or nonregular entry"));
    }
    Ok(())
}
/// Source mechanics shares the original operation clock, including cancellation.
pub(crate) fn tick(
    s: &crate::route_cards::RouteSources,
    cancel: &std::sync::atomic::AtomicI32,
) -> io::Result<()> {
    if cancel.load(Ordering::Relaxed) != 0 {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "selected export cancelled",
        ));
    }
    crate::kag_release::budget_check()?;
    s.check()
}
fn stamp(m: &fs::Metadata) -> (u64, u64, u64, u32, i64, i64, i64, i64) {
    use std::os::unix::fs::MetadataExt;
    (
        m.dev(),
        m.ino(),
        m.len(),
        m.mode(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    )
}
/// Constructible only by the current verifier; owns observed descriptor custody.
pub struct VerifiedExport {
    sources: std::cell::RefCell<crate::route_cards::RouteSources>,
    held: BTreeMap<String, (File, fs::Metadata)>,
    manifest: Value,
    read_bytes: usize,
}
impl VerifiedExport {
    fn new(sources: crate::route_cards::RouteSources) -> Self {
        Self {
            sources: std::cell::RefCell::new(sources),
            held: BTreeMap::new(),
            manifest: Value::Null,
            read_bytes: 0,
        }
    }
    pub(crate) fn manifest(&self) -> &Value {
        &self.manifest
    }
    pub fn summary(&self) -> Value {
        json!({"export_revision":self.manifest["export_revision"],"corpus_revision":self.manifest["corpus_revision"],"primary_source":self.manifest["primary_source"]})
    }
    fn inventory(&self, cancel: &std::sync::atomic::AtomicI32) -> io::Result<BTreeSet<String>> {
        let mut sources = self.sources.borrow_mut();
        tick(&sources, cancel)?;
        let directories = std::cell::Cell::new(0usize);
        let over_bound = std::cell::Cell::new(false);
        let paths = sources.selected_physical_paths(".", &|path, directory| {
            if directory {
                directories.set(directories.get() + 1);
            }
            if Path::new(path).components().count() > 32 || directories.get() > 4096 {
                over_bound.set(true);
                return false;
            }
            true
        })?;
        if over_bound.get() {
            return Err(bad("export inventory exceeds finite traversal bound"));
        }
        let mut files = BTreeSet::new();
        for path in &paths {
            tick(&sources, cancel)?;
            let metadata = sources
                .metadata(path)?
                .ok_or_else(|| bad("selected export entry disappeared"))?;
            if metadata.is_dir() {
                continue;
            }
            if !metadata.is_file() {
                return Err(bad("export contains symlink or nonregular entry"));
            }
            files.insert(path.strip_prefix("./").unwrap_or(path).to_owned());
            if files.len() > 4096 {
                return Err(bad("export inventory exceeds finite traversal bound"));
            }
        }
        Ok(files)
    }

    fn read(
        &mut self,
        path: &str,
        cap: u64,
        cancel: &std::sync::atomic::AtomicI32,
    ) -> io::Result<Vec<u8>> {
        let mut sources = self.sources.borrow_mut();
        tick(&sources, cancel)?;
        let (raw, meta, file) = sources.bounded_held_bytes(
            path,
            usize::try_from(cap).map_err(bad)?,
            &mut self.read_bytes,
            4 * MAX as usize,
        )?;
        if let Some((held, previous)) = self.held.get(path) {
            if stamp(previous) != stamp(&meta) || stamp(previous) != stamp(&held.metadata()?) {
                return Err(bad("export member changed during verification"));
            }
        }
        self.held.insert(path.to_owned(), (file, meta));
        tick(&sources, cancel)?;
        Ok(raw)
    }
    fn json_file(
        &mut self,
        path: &str,
        cancel: &std::sync::atomic::AtomicI32,
    ) -> io::Result<Value> {
        let raw = self.read(path, MAX, cancel)?;
        parse_json(
            &raw,
            JsonMode::PublishedStrict,
            JsonLimits {
                max_bytes: MAX as usize,
                ..JsonLimits::default()
            },
        )
        .map_err(bad)?;
        serde_json::from_slice(&raw).map_err(bad)
    }
    fn check_local(&self, cancel: &std::sync::atomic::AtomicI32) -> io::Result<()> {
        if self.inventory(cancel)? != self.held.keys().cloned().collect() {
            return Err(bad("selected export inventory changed"));
        }
        let mut sources = self.sources.borrow_mut();
        tick(&sources, cancel)?;
        sources.verify_root()?;
        for (path, (file, meta)) in &self.held {
            tick(&sources, cancel)?;
            let current = sources
                .metadata(path)?
                .ok_or_else(|| bad("selected export member disappeared"))?;
            if stamp(meta) != stamp(&file.metadata()?) || stamp(meta) != stamp(&current) {
                return Err(bad("selected export member binding changed"));
            }
        }
        sources.verify_root()
    }
    pub fn check(
        &self,
        primary: &crate::route_cards::RouteSources,
        cancel: &std::sync::atomic::AtomicI32,
    ) -> io::Result<()> {
        tick(primary, cancel)?;
        self.check_local(cancel)
    }
    pub fn bind_repo(
        &self,
        sources: &mut crate::route_cards::RouteSources,
        cancel: &std::sync::atomic::AtomicI32,
    ) -> io::Result<()> {
        self.check(sources, cancel)?;
        let mut bytes = 0;
        for path in SOURCE_PATHS {
            tick(sources, cancel)?;
            let binding = self.manifest["files"]
                .as_array()
                .and_then(|a| a.iter().find(|b| b["path"] == path))
                .ok_or_else(|| bad("missing source binding"))?;
            let size = binding["size_bytes"]
                .as_u64()
                .ok_or_else(|| bad("invalid source size"))?;
            let raw = sources.bounded_bytes(
                path,
                usize::try_from(size).map_err(bad)?,
                &mut bytes,
                MAX as usize,
            )?;
            if raw.len() as u64 != size || digest(&raw) != binding["sha256"] {
                return Err(bad(
                    "selected KAG export differs from current owned source closure",
                ));
            }
        }
        self.check(sources, cancel)
    }
}
/// Verify exact membership, aggregate size, every byte binding and source-return
/// semantics. The receiver deliberately does not require the current producer.
pub fn verify_export(root: &Path) -> io::Result<Value> {
    let budget = crate::kag_release::WholeBudget::begin()?;
    let root = crate::kag_release::safe_absolute(root)?;
    let sources = crate::route_cards::RouteSources::new_until(&root, budget.deadline()?)?;
    let verified = verify_receiver(
        VerifiedExport::new(sources),
        &std::sync::atomic::AtomicI32::new(0),
    )?;
    Ok(verified.manifest.clone())
}
/// Exact downstream handle on the caller's original shared clock and lookup ledger.
pub fn verify_with_sources(
    root: &Path,
    primary: &crate::route_cards::RouteSources,
    cancel: &std::sync::atomic::AtomicI32,
) -> io::Result<VerifiedExport> {
    let root = crate::kag_release::safe_absolute(root)?;
    let sources =
        crate::route_cards::RouteSources::new_until_related(&root, primary.deadline(), primary)?;
    verify_receiver(VerifiedExport::new(sources), cancel)
}
fn verify_receiver(
    mut receiver: VerifiedExport,
    cancel: &std::sync::atomic::AtomicI32,
) -> io::Result<VerifiedExport> {
    let manifest = receiver.json_file("export.json", cancel)?;
    keys(
        &manifest,
        &[
            "schema_version",
            "corpus_revision",
            "producer_sha256",
            "primary_source",
            "files",
            "export_revision",
        ],
    )?;
    if manifest["schema_version"] != SCHEMA {
        return Err(bad("unsupported KAG source export"));
    }
    for k in ["corpus_revision", "producer_sha256", "export_revision"] {
        hex(&manifest[k])?;
    }
    let mut body = manifest.clone();
    body.as_object_mut().unwrap().remove("export_revision");
    if digest(&canonical(&body)?) != manifest["export_revision"] {
        return Err(bad("export identity mismatch"));
    }
    let mut expected = SOURCE_PATHS.to_vec();
    expected.push(CAPSULE);
    expected.sort_unstable();
    let bindings = manifest["files"]
        .as_array()
        .ok_or_else(|| bad("files must be an exact list"))?;
    if bindings.len() != expected.len() {
        return Err(bad("source membership differs"));
    }
    let mut by_path = BTreeMap::new();
    let mut total = 0u64;
    for (binding, relative) in bindings.iter().zip(&expected) {
        keys(binding, &["path", "sha256", "size_bytes"])?;
        if binding["path"].as_str() != Some(*relative) {
            return Err(bad("source membership differs"));
        }
        let sha = hex(&binding["sha256"])?;
        let size = binding["size_bytes"]
            .as_u64()
            .ok_or_else(|| bad("invalid byte size"))?;
        total = total
            .checked_add(size)
            .ok_or_else(|| bad("byte count overflow"))?;
        if total > MAX {
            return Err(bad("KAG export exceeds 8MiB"));
        }
        let raw = receiver.read(&format!("Tree-of-Sophia/{relative}"), size, cancel)?;
        if raw.len() as u64 != size || digest(&raw) != sha {
            return Err(bad("exported source fixity mismatch"));
        }
        by_path.insert(*relative, binding);
    }
    let files = receiver.inventory(cancel)?;
    let declared: BTreeSet<_> = std::iter::once("export.json".to_owned())
        .chain(expected.iter().map(|p| format!("Tree-of-Sophia/{p}")))
        .collect();
    if files != declared {
        return Err(bad("undeclared export files"));
    }
    let node = receiver.json_file(&format!("Tree-of-Sophia/{PRIMARY}"), cancel)?;
    let capsule = receiver.json_file(&format!("Tree-of-Sophia/{CAPSULE}"), cancel)?;
    if !node.is_object() || !capsule.is_object() {
        return Err(bad("node and capsule must be objects"));
    }
    let id = node["node_id"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| bad("missing source identity"))?;
    let primary = json!({"record_id": id, "path": PRIMARY, "sha256":by_path[PRIMARY]["sha256"], "corpus_revision":manifest["corpus_revision"]});
    if manifest["primary_source"] != primary || capsule["object_id"] != id {
        return Err(bad("capsule does not return exact canonical source"));
    }
    let mirror = receiver.json_file(
        "Tree-of-Sophia/ToS/public-compatibility/source_node.example.json",
        cancel,
    )?;
    let entry = json!({"repo":"Tree-of-Sophia", "path":"ToS/public-compatibility/source_node.example.json", "match_key":"node_id", "match_value":id});
    let layers = mirror["interpretation_layers"]
        .as_array()
        .filter(|a| !a.is_empty())
        .ok_or_else(|| bad("missing interpretation layers"))?;
    if layers.iter().any(|v| v.as_str().is_none_or(str::is_empty))
        || mirror["node_id"] != id
        || capsule["section_handles"] != mirror["interpretation_layers"]
        || capsule["owner_repo"] != "Tree-of-Sophia"
        || capsule["kind"] != "source_node"
        || capsule["entry_surface"] != entry
    {
        return Err(bad("invalid source-return capsule"));
    }
    let relations: Vec<_> = [
        ("bounded_hop", SOURCE_PATHS[2]),
        ("capsule_surface", SOURCE_PATHS[4]),
        ("tiny_entry_route", SOURCE_PATHS[5]),
    ]
    .iter()
    .map(
        |(kind, path)| json!({"relation_type":kind, "target_ref":format!("Tree-of-Sophia/{path}")}),
    )
    .collect();
    if capsule["direct_relations"] != json!(relations) {
        return Err(bad("relation leaves exported source closure"));
    }
    for key in [
        "primary_question",
        "summary_50",
        "summary_200",
        "provenance_note",
        "non_identity_boundary",
    ] {
        if capsule[key].as_str().is_none_or(|s| s.trim().is_empty()) {
            return Err(bad("missing source-return explanation"));
        }
    }
    receiver.manifest = manifest;
    receiver.check_local(cancel)?;
    Ok(receiver)
}
fn producer(repo: &Path) -> io::Result<String> {
    crate::kag_release::budget_check()?;
    directory(repo)?;
    let mut files = BTreeSet::new();
    for subtree in [
        "rust/crates/tos-ops-mechanics-plan/src",
        "rust/crates/tos-source-store/src",
        "rust/crates/tos-foundation/src",
    ] {
        let mut dirs = Vec::new();
        inventory(repo, Path::new(subtree), &mut files, &mut dirs)?;
    }
    for path in [
        "Cargo.lock",
        "Cargo.toml",
        "rust-toolchain.toml",
        "rust/crates/tos-ops-mechanics-plan/Cargo.toml",
        "rust/crates/tos-source-store/Cargo.toml",
        "rust/crates/tos-foundation/Cargo.toml",
    ] {
        files.insert(path.to_owned());
    }
    let mut bindings = serde_json::Map::new();
    let mut total = 0u64;
    for path in files {
        let raw = read(repo, &path, MAX - total)?;
        total += raw.len() as u64;
        bindings.insert(path, json!(digest(&raw)));
    }
    // Bind the running native program as well as its source composition. A
    // checkout with different source bytes cannot stand in for the executable.
    let executable = std::env::current_exe()?;
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(executable)?;
    let cap = 1024 * 1024 * 1024u64;
    if !file.metadata()?.is_file() || file.metadata()?.len() > cap {
        return Err(bad("native producer exceeds executable bound"));
    }
    let mut hasher = Digest256Hasher::new();
    let mut count = 0u64;
    let mut block = [0u8; 32768];
    loop {
        crate::kag_release::budget_check()?;
        let n = file.read(&mut block)?;
        if n == 0 {
            break;
        }
        count += n as u64;
        if count > cap {
            return Err(bad("native producer grew beyond executable bound"));
        }
        hasher.update(&block[..n]);
    }
    bindings.insert(
        "native_executable_sha256".to_owned(),
        json!(hasher.finalize().to_hex()),
    );
    Ok(digest(&canonical(&Value::Object(bindings))?))
}
struct Stage(PathBuf);
impl Drop for Stage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    fs::create_dir_all(path.parent().unwrap())?;
    let mut f = OpenOptions::new().write(true).create_new(true).open(path)?;
    f.write_all(bytes)?;
    f.sync_all()
}
fn publish(stage: &Path, output: &Path) -> io::Result<()> {
    let source = CString::new(stage.as_os_str().as_bytes()).map_err(bad)?;
    let target = CString::new(output.as_os_str().as_bytes()).map_err(bad)?;
    // Linux's exclusive rename keeps even an empty concurrent output intact.
    if unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            source.as_ptr(),
            libc::AT_FDCWD,
            target.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    File::open(output.parent().unwrap())?.sync_all()
}
/// Construct the whole export from six exact immutable corpus members. A
/// manifest budget is finite and independent from the 8MiB exported-byte cap.
pub fn build_export(repo: &Path, store: &Path, revision: &str, output: &Path) -> io::Result<Value> {
    let _budget = crate::kag_release::WholeBudget::begin()?;
    crate::kag_release::budget_check()?;
    let store_path = crate::kag_release::safe_absolute(store)?;
    let store = store_path.as_path();
    let output_path = crate::kag_release::safe_absolute(output)?;
    let output = output_path.as_path();
    directory(store)?;
    directory(repo)?;
    let revision = SourceRevision(Digest256::from_hex(revision).map_err(bad)?);
    if !output.is_absolute() || output.file_name().is_none() || fs::symlink_metadata(output).is_ok()
    {
        return Err(bad("output must be a new absolute regular path"));
    }
    let parent = output.parent().unwrap();
    fs::create_dir_all(parent)?;
    directory(parent)?;
    let before = producer(repo)?;
    let json_limits = JsonLimits {
        max_bytes: 64 * 1024 * 1024,
        max_visits: 4_000_000,
        ..JsonLimits::default()
    };
    let reader = CorpusReader::open_existing(
        store,
        ReadLimits {
            max_manifest_bytes: json_limits.max_bytes,
            max_manifest_entries: 1_000_000,
            max_selected_object_bytes: MAX,
            json: json_limits,
        },
    )
    .map_err(bad)?;
    let snapshot = reader.load_exact(revision).map_err(bad)?;
    let mut descriptors = Vec::new();
    let mut total = 0u64;
    for relative in SOURCE_PATHS {
        let path = RelativePath::parse(relative).map_err(bad)?;
        let metadata = snapshot
            .member(&path)
            .ok_or_else(|| bad("selected corpus lacks bounded KAG source closure"))?;
        total = total
            .checked_add(metadata.size_bytes)
            .ok_or_else(|| bad("source byte count overflow"))?;
        if total > MAX {
            return Err(bad("source inputs exceed 8MiB"));
        }
        descriptors.push(
            reader
                .resolve(&snapshot, Selector::Path(&path))
                .map_err(bad)?,
        );
    }
    let mut selected = None;
    for _ in 0..128 {
        let path = parent.join(format!(
            ".kag-export-{}-{}",
            std::process::id(),
            STAGES.fetch_add(1, Ordering::Relaxed)
        ));
        match fs::create_dir(&path) {
            Ok(()) => {
                selected = Some(Stage(path));
                break;
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
    }
    let stage = selected.ok_or_else(|| bad("cannot reserve export stage"))?;
    let source = stage.0.join("Tree-of-Sophia");
    for descriptor in descriptors {
        let mut raw = Vec::new();
        reader
            .read_selected(&snapshot, &descriptor, MAX, &mut raw)
            .map_err(bad)?;
        write_new(&source.join(descriptor.path.as_str()), &raw)?;
    }
    let payload = crate::derived_kag::build_payload(&source)?;
    let capsule = canonical_bytes_v1(
        &payload,
        CanonicalProfile::CorpusSnapshotV1,
        JsonLimits {
            max_bytes: MAX as usize,
            ..JsonLimits::default()
        },
    )
    .map_err(bad)?;
    if total
        .checked_add(capsule.len() as u64)
        .is_none_or(|n| n > MAX)
    {
        return Err(bad("source plus capsule exceeds 8MiB"));
    }
    write_new(&source.join(CAPSULE), &capsule)?;
    let mut paths = SOURCE_PATHS.to_vec();
    paths.push(CAPSULE);
    paths.sort_unstable();
    let mut bindings = Vec::new();
    for relative in paths {
        let raw = read(&source, relative, MAX)?;
        bindings.push(json!({"path":relative, "sha256":digest(&raw), "size_bytes":raw.len()}));
    }
    let node = json_file(&source, PRIMARY)?;
    let primary_digest = bindings.iter().find(|v| v["path"] == PRIMARY).unwrap()["sha256"].clone();
    let mut manifest = json!({"schema_version":SCHEMA,"corpus_revision":revision.0.to_hex(),"producer_sha256":before,"files":bindings,"primary_source":{"record_id":node["node_id"],"path":PRIMARY,"sha256":primary_digest,"corpus_revision":revision.0.to_hex()}});
    let identity = digest(&canonical(&manifest)?);
    manifest["export_revision"] = json!(identity);
    write_new(&stage.0.join("export.json"), &canonical(&manifest)?)?;
    verify_export(&stage.0)?;
    if producer(repo)? != before {
        return Err(bad("producer changed during construction"));
    }
    let mut files = BTreeSet::new();
    let mut dirs = Vec::new();
    inventory(&stage.0, Path::new(""), &mut files, &mut dirs)?;
    dirs.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
    for directory in dirs {
        File::open(directory)?.sync_all()?;
    }
    publish(&stage.0, output)?;
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Stage, PathBuf, String) {
        let base = std::env::temp_dir().join(format!(
            "tos-kag-corpus-test-{}-{}",
            std::process::id(),
            STAGES.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&base).unwrap();
        let store = base.join("store");
        fs::create_dir_all(store.join("revisions")).unwrap();
        fs::create_dir_all(store.join("objects")).unwrap();
        let mut files = Vec::new();
        for path in SOURCE_PATHS {
            let raw = match path {
                PRIMARY => canonical(&json!({"node_id":"corpus-only-node"})).unwrap(),
                "ToS/public-compatibility/source_node.example.json" => canonical(&json!({"node_id":"corpus-only-node","interpretation_layers":["corpus-only-layer"],"relations":[{"relation_type":"bounded_hop","target_ref":"corpus-only-concept"}]})).unwrap(),
                "ToS/public-compatibility/concept_node.example.json" => canonical(&json!({"node_id":"corpus-only-concept"})).unwrap(),
                _ => b"# exact corpus bytes\n".to_vec(),
            };
            let sha = digest(&raw);
            let object = store.join("objects").join(&sha);
            if object.exists() {
                assert_eq!(fs::read(&object).unwrap(), raw);
            } else {
                write_new(&object, &raw).unwrap();
            }
            files.push(json!({"path":path,"sha256":sha,"size_bytes":raw.len(),"mode":420}));
        }
        files.sort_by(|a, b| a["path"].as_str().cmp(&b["path"].as_str()));
        let mut snapshot = json!({"schema_version":"tos_corpus_snapshot_v1","base_revision":null,"validator_sha256":"1".repeat(64),"files":files,"identities":{"corpus-only-node":PRIMARY},"dependencies":{},"retirements":[]});
        let revision = digest(&canonical(&snapshot).unwrap());
        snapshot["revision"] = json!(revision);
        write_new(
            &store
                .join("revisions")
                .join(&revision)
                .join("snapshot.json"),
            &canonical(&snapshot).unwrap(),
        )
        .unwrap();
        (Stage(base), store, revision)
    }
    fn repo() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .unwrap()
            .to_owned()
    }
    #[test]
    fn whole_source_export_uses_selected_corpus_and_refuses_tampering() {
        let (base, store, revision) = fixture();
        let output = base.0.join("export");
        let manifest = build_export(&repo(), &store, &revision, &output).unwrap();
        assert_eq!(manifest["primary_source"]["record_id"], "corpus-only-node");
        assert_eq!(verify_export(&output).unwrap(), manifest);
        assert!(build_export(&repo(), &store, &revision, &output).is_err());
        fs::write(output.join("undeclared.txt"), b"extra").unwrap();
        assert!(verify_export(&output).is_err());
        fs::remove_file(output.join("undeclared.txt")).unwrap();
        fs::write(output.join("Tree-of-Sophia").join(PRIMARY), b"altered").unwrap();
        assert!(verify_export(&output).is_err());
    }
    #[test]
    fn selected_handle_preserves_original_clock_and_refuses_named_substitution() {
        let (base, store, revision) = fixture();
        let output = base.0.join("export");
        build_export(&repo(), &store, &revision, &output).unwrap();
        let cancel = std::sync::atomic::AtomicI32::new(0);
        let mut primary = crate::route_cards::RouteSources::new(&repo()).unwrap();
        let verified = verify_with_sources(&output, &primary, &cancel).unwrap();
        assert_eq!(verified.sources.borrow().deadline(), primary.deadline());
        verified.check(&primary, &cancel).unwrap();
        // The fixture corpus differs from this repository; a manifest is no admission.
        assert!(verified.bind_repo(&mut primary, &cancel).is_err());
        let path = output.join("Tree-of-Sophia").join(PRIMARY);
        let identical = fs::read(&path).unwrap();
        let replacement = path.with_extension("replacement");
        fs::write(&replacement, &identical).unwrap();
        fs::rename(replacement, &path).unwrap();
        assert!(verified.check(&primary, &cancel).is_err());
        cancel.store(1, Ordering::Relaxed);
        assert!(verify_with_sources(&output, &primary, &cancel).is_err());
    }
    #[test]
    fn corrupt_selected_object_never_publishes_output() {
        let (base, store, revision) = fixture();
        let output = base.0.join("export");
        let snapshot: Value = serde_json::from_slice(
            &fs::read(
                store
                    .join("revisions")
                    .join(&revision)
                    .join("snapshot.json"),
            )
            .unwrap(),
        )
        .unwrap();
        let sha = snapshot["files"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["path"] == PRIMARY)
            .unwrap()["sha256"]
            .as_str()
            .unwrap();
        fs::write(store.join("objects").join(sha), b"corrupt").unwrap();
        assert!(build_export(&repo(), &store, &revision, &output).is_err());
        assert!(!output.exists());
    }

    #[test]
    fn export_rehash_cannot_change_source_return_and_links_are_refused() {
        for case in ["node", "entry", "sections", "relation", "symlink"] {
            let (base, store, revision) = fixture();
            let output = base.0.join("export");
            let mut manifest = build_export(&repo(), &store, &revision, &output).unwrap();
            let relative = if case == "node" { PRIMARY } else { CAPSULE };
            let path = output.join("Tree-of-Sophia").join(relative);
            if case == "symlink" {
                let outside = base.0.join("outside.json");
                fs::rename(&path, &outside).unwrap();
                std::os::unix::fs::symlink(&outside, &path).unwrap();
            } else {
                let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                match case {
                    "node" => value["node_id"] = json!("incompatible-node"),
                    "entry" => value["entry_surface"]["match_value"] = json!("incompatible-node"),
                    "sections" => value["section_handles"] = json!(["other-layer"]),
                    "relation" => {
                        value["direct_relations"][0]["target_ref"] = json!(
                            "Tree-of-Sophia/ToS/public-compatibility/source_node.example.json"
                        )
                    }
                    _ => unreachable!(),
                }
                let raw = canonical(&value).unwrap();
                fs::write(&path, &raw).unwrap();
                let entry = manifest["files"]
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .find(|v| v["path"] == relative)
                    .unwrap();
                entry["sha256"] = json!(digest(&raw));
                entry["size_bytes"] = json!(raw.len());
                manifest.as_object_mut().unwrap().remove("export_revision");
                manifest["export_revision"] = json!(digest(&canonical(&manifest).unwrap()));
                fs::write(output.join("export.json"), canonical(&manifest).unwrap()).unwrap();
            }
            assert!(verify_export(&output).is_err(), "accepted {case}");
        }
    }

    // The selected foreign owner is a controlled subprocess in these tests.
    // Its CLI protocol is exercised without importing or executing ToS Python.
    fn selected_owner(base: &Path, source_hash: &str) -> (PathBuf, PathBuf) {
        use std::os::unix::fs::PermissionsExt;
        let kag = base.join("selected-kag");
        for path in crate::kag_release::PROGRAM_PATHS {
            write_new(&kag.join(path), b"controlled foreign owner fixture\n").unwrap();
        }
        write_new(&kag.join("mode"), b"ok\n").unwrap();
        write_new(
            &kag.join("probe.json"),
            &canonical(&json!({
                "primary_source": {"identity": {"path": PRIMARY, "content_hash": source_hash},
                    "owner_return_route": {"repo": "Tree-of-Sophia", "surface": PRIMARY}},
                "distribution_identity": {"corpus": "controlled-fixture"}
            }))
            .unwrap(),
        )
        .unwrap();
        let interpreter = base.join("foreign-owner-runner");
        write_new(&interpreter, br##"#!/bin/sh
set -eu
program=$1
shift
test "$1" = --repo-root
provider=$2
test "$3" = --artifact-root
artifacts=$4
shift 4
mode=$(cat mode)
case "$program" in
  */scripts/build_repo_local_kag_release.py)
    test "$#" = 0
    case "$mode" in
      fail) echo 'controlled producer failure' >&2; exit 7;;
      mutate) printf changed >> "$provider/ToS/canon/source/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/node.json";;
      control-mutate) printf changed >> "$provider/kag/README.md";;
      program-mutate) printf changed >> scripts/query_repo_local_kag.py;;
    esac
    mkdir -p "$provider/kag/indexes"
    printf '{"fixture":true}' > "$provider/kag/indexes/hot.json"
    printf '{"fixture":true}' > "$artifacts/config.json"
    ;;
  */scripts/validate_repo_local_kag_family.py)
    test "$#" = 3
    test "$1" = --no-shadow-git
    test "$2" = --probe-source
    test "$3" = ToS/canon/source/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/node.json
    test "$mode" != control-fail || { echo 'provider-home source reference refused' >&2; exit 9; }
    cat probe.json
    ;;
  *) exit 11;;
esac
"##).unwrap();
        fs::set_permissions(&interpreter, fs::Permissions::from_mode(0o700)).unwrap();
        (kag, interpreter)
    }

    fn publish(
        store: &Path,
        revision: &str,
        kag: &Path,
        release: &Path,
        runner: &Path,
    ) -> io::Result<Value> {
        crate::kag_release::build_release(
            &repo(),
            store,
            revision,
            kag,
            release,
            runner,
            &std::sync::atomic::AtomicI32::new(0),
        )
    }

    fn source_hash(store: &Path, revision: &str) -> String {
        let snapshot: Value = serde_json::from_slice(
            &fs::read(store.join("revisions").join(revision).join("snapshot.json")).unwrap(),
        )
        .unwrap();
        snapshot["files"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["path"] == PRIMARY)
            .unwrap()["sha256"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    #[test]
    fn publication_is_deterministic_preserves_prior_success_and_binds_owner_programs() {
        let (base, store, revision) = fixture();
        let (kag, runner) = selected_owner(&base.0, &source_hash(&store, &revision));
        let release = base.0.join("release");
        let first = publish(&store, &revision, &kag, &release, &runner).unwrap();
        assert_eq!(first["programs"].as_array().unwrap().len(), 5);
        assert_eq!(
            publish(&store, &revision, &kag, &release, &runner).unwrap(),
            first
        );
        let other = base.0.join("other-release");
        assert_eq!(
            publish(&store, &revision, &kag, &other, &runner).unwrap(),
            first
        );
        let status = crate::kag_release::status_release(&release, &revision).unwrap();
        assert_eq!(status["freshness"], "current");
        assert_eq!(
            status["integration_revision"],
            first["integration_revision"]
        );
        assert_eq!(status["primary_source"], first["primary_source"]);
        let old = release
            .join("releases")
            .join(first["integration_revision"].as_str().unwrap());
        fs::write(kag.join("mode"), b"fail\n").unwrap();
        assert!(publish(&store, &revision, &kag, &release, &runner).is_err());
        assert_eq!(
            crate::kag_release::verify_integration(&old, &revision).unwrap(),
            first
        );
        let status = crate::kag_downstream_status::Status::new(&release.join("status"), "kag")
            .unwrap()
            .status(&revision)
            .unwrap();
        assert_eq!(status["freshness"], "current");
        assert_eq!(status["state"]["latest"]["state"], "failed");
        fs::write(kag.join("mode"), b"ok\n").unwrap();
        fs::write(
            kag.join("scripts/query_repo_local_kag.py"),
            b"changed foreign owner\n",
        )
        .unwrap();
        let second = publish(&store, &revision, &kag, &release, &runner).unwrap();
        assert_ne!(
            second["integration_revision"],
            first["integration_revision"]
        );
        assert_eq!(
            crate::kag_release::verify_integration(&old, &revision).unwrap(),
            first
        );
    }

    #[test]
    fn owner_failure_mutation_and_invalid_probe_never_publish() {
        for mode in [
            "fail",
            "mutate",
            "control-mutate",
            "program-mutate",
            "control-fail",
            "mismatch",
            "duplicate",
        ] {
            let (base, store, revision) = fixture();
            let (kag, runner) = selected_owner(&base.0, &source_hash(&store, &revision));
            fs::write(kag.join("mode"), mode).unwrap();
            if mode == "mismatch" {
                let mut probe: Value =
                    serde_json::from_slice(&fs::read(kag.join("probe.json")).unwrap()).unwrap();
                probe["primary_source"]["identity"]["content_hash"] = json!("f".repeat(64));
                fs::write(kag.join("probe.json"), canonical(&probe).unwrap()).unwrap();
            } else if mode == "duplicate" {
                fs::write(
                    kag.join("probe.json"),
                    b"{\"primary_source\":{},\"primary_source\":{}}",
                )
                .unwrap();
            }
            let release = base.0.join("release");
            assert!(
                publish(&store, &revision, &kag, &release, &runner).is_err(),
                "accepted {mode}"
            );
            assert_eq!(fs::read_dir(release.join("releases")).unwrap().count(), 0);
            let status = crate::kag_downstream_status::Status::new(&release.join("status"), "kag")
                .unwrap()
                .status(&revision)
                .unwrap();
            assert_eq!(status["freshness"], "missing");
            assert_eq!(status["state"]["latest"]["state"], "failed");
        }
    }

    fn rewrite_integration(path: &Path, value: &mut Value) {
        value
            .as_object_mut()
            .unwrap()
            .remove("integration_revision");
        value["integration_revision"] = json!(digest(&canonical(value).unwrap()));
        fs::write(path.join("integration.json"), canonical(value).unwrap()).unwrap();
    }

    #[test]
    fn complete_membership_tamper_and_historical_program_binding_are_checked() {
        for case in [
            "extra",
            "member",
            "manifest",
            "primary",
            "historical",
            "unknown-program",
        ] {
            let (base, store, revision) = fixture();
            let (kag, runner) = selected_owner(&base.0, &source_hash(&store, &revision));
            let release = base.0.join("release");
            let mut integration = publish(&store, &revision, &kag, &release, &runner).unwrap();
            let path = release
                .join("releases")
                .join(integration["integration_revision"].as_str().unwrap());
            match case {
                "extra" => fs::write(path.join("unexpected.bin"), b"extra").unwrap(),
                "member" => fs::write(path.join("artifacts/config.json"), b"changed").unwrap(),
                "manifest" => fs::write(path.join("integration.json"), b"invalid JSON").unwrap(),
                "primary" => {
                    integration["primary_source"]["identity"]["content_hash"] =
                        json!("f".repeat(64));
                    rewrite_integration(&path, &mut integration);
                }
                "historical" => {
                    integration["programs"].as_array_mut().unwrap().remove(2);
                    rewrite_integration(&path, &mut integration);
                }
                "unknown-program" => {
                    integration["programs"][2]["path"] = json!("scripts/unrecognized.py");
                    rewrite_integration(&path, &mut integration);
                }
                _ => unreachable!(),
            }
            let observed = crate::kag_release::verify_integration(&path, &revision);
            if case == "historical" {
                assert_eq!(observed.unwrap(), integration);
            } else {
                assert!(observed.is_err(), "accepted {case}");
                assert!(crate::kag_release::status_release(&release, &revision).is_err());
                if case == "extra" {
                    assert!(publish(&store, &revision, &kag, &release, &runner).is_err());
                    assert!(path.join("unexpected.bin").exists());
                }
            }
        }
        let (base, _, revision) = fixture();
        let missing = base.0.join("missing");
        let status = crate::kag_release::status_release(&missing, &revision).unwrap();
        assert_eq!(status["freshness"], "missing");
        assert!(status["integration_revision"].is_null());
        assert!(!missing.exists());
    }
}
