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
/// Verify exact membership, aggregate size, every byte binding and source-return
/// semantics. The receiver deliberately does not require the current producer.
pub fn verify_export(root: &Path) -> io::Result<Value> {
    let _budget = crate::kag_release::WholeBudget::begin()?;
    crate::kag_release::budget_check()?;
    let root_path = crate::kag_release::safe_absolute(root)?;
    let root = root_path.as_path();
    directory(root)?;
    let manifest = json_file(root, "export.json")?;
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
        let raw = read(root, &format!("Tree-of-Sophia/{relative}"), size)?;
        if raw.len() as u64 != size || digest(&raw) != sha {
            return Err(bad("exported source fixity mismatch"));
        }
        by_path.insert(*relative, binding);
    }
    let mut files = BTreeSet::new();
    let mut dirs = Vec::new();
    inventory(root, Path::new(""), &mut files, &mut dirs)?;
    let declared: BTreeSet<_> = std::iter::once("export.json".to_owned())
        .chain(expected.iter().map(|p| format!("Tree-of-Sophia/{p}")))
        .collect();
    if files != declared {
        return Err(bad("undeclared export files"));
    }
    let source = root.join("Tree-of-Sophia");
    let node = json_file(&source, PRIMARY)?;
    let capsule = json_file(&source, CAPSULE)?;
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
    let mirror = json_file(&source, "ToS/public-compatibility/source_node.example.json")?;
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
    Ok(manifest)
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
            write_new(&store.join("objects").join(&sha), &raw).unwrap();
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
}
