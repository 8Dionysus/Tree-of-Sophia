//! Authored definition tree with independent semantic and navigation edges.
//! Selection is source-reviewed metadata; this module never replaces registry consumers or files.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use tos_foundation::Digest256;

const MAX_MEMBER: u64 = 2 * 1024 * 1024;
const MAX_TOTAL: usize = 16 * 1024 * 1024;
const MAX_ENTRIES: usize = 8192;
const CANDIDATE: &str = "candidate-not-selected";
const SELECTED: &str = "selected-authored";
const FILES: [&str; 2] = ["entity-types.v1.json", "relation-types.v1.json"];
const GROUPS: [(&str, &str, usize); 3] = [
    ("types", "type_id", 0),
    ("property_definitions", "property_id", 0),
    ("relations", "relation_type_id", 1),
];
pub type Result<T> = std::result::Result<T, TreeError>;
#[derive(Debug)]
pub struct TreeError {
    pub code: &'static str,
    pub detail: String,
}
impl std::fmt::Display for TreeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.detail)
    }
}
impl std::error::Error for TreeError {}
fn fail<T>(code: &'static str, detail: impl Into<String>) -> Result<T> {
    Err(TreeError {
        code,
        detail: detail.into(),
    })
}
fn io(e: std::io::Error) -> TreeError {
    TreeError {
        code: "Io",
        detail: e.to_string(),
    }
}
fn invalid(e: serde_json::Error) -> TreeError {
    TreeError {
        code: "InvalidJson",
        detail: e.to_string(),
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub id: String,
    pub collection: String,
    pub path: String,
    pub navigation_parent: String,
    #[serde(default)]
    pub previous_paths: Vec<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Baseline {
    pub path: String,
    pub sha256: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema_version: String,
    pub posture: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection_review_ref: Option<String>,
    pub baseline: Vec<Baseline>,
    pub envelopes: Vec<String>,
    pub navigation_roots: Vec<String>,
    pub navigation_groups: BTreeMap<String, String>,
    pub entries: Vec<Entry>,
}
fn safe(root: &Path, rel: &str) -> Result<PathBuf> {
    let p = Path::new(rel);
    if rel.is_empty() || p.components().any(|x| !matches!(x, Component::Normal(_))) {
        return fail("UnsafePath", rel);
    }
    let mut out = root.to_path_buf();
    for c in p.components() {
        out.push(c);
        if fs::symlink_metadata(&out).is_ok_and(|m| m.file_type().is_symlink()) {
            return fail("UnsafePath", rel);
        }
    }
    Ok(out)
}
fn read(root: &Path, path: &str, total: &mut usize) -> Result<Vec<u8>> {
    let p = safe(root, path)?;
    let f = File::open(p).map_err(io)?;
    let mut bytes = Vec::new();
    f.take(MAX_MEMBER + 1).read_to_end(&mut bytes).map_err(io)?;
    *total += bytes.len();
    if bytes.len() as u64 > MAX_MEMBER || *total > MAX_TOTAL {
        return fail("ResourceLimit", path);
    }
    Ok(bytes)
}
fn value(root: &Path, path: &str, total: &mut usize) -> Result<Value> {
    serde_json::from_slice(&read(root, path, total)?).map_err(invalid)
}
fn pretty(v: &impl Serialize) -> Result<Vec<u8>> {
    let mut b = serde_json::to_vec_pretty(v).map_err(invalid)?;
    b.push(b'\n');
    Ok(b)
}
fn put(root: &Path, rel: &str, bytes: &[u8]) -> Result<()> {
    let p = safe(root, rel)?;
    fs::create_dir_all(p.parent().unwrap()).map_err(io)?;
    let mut f = File::options()
        .write(true)
        .create_new(true)
        .open(p)
        .map_err(io)?;
    f.write_all(bytes).map_err(io)?;
    f.sync_all().map_err(io)
}
fn strings(v: &Value, key: &str) -> Result<Vec<String>> {
    v.get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| TreeError {
            code: "InvalidDefinition",
            detail: key.into(),
        })?
        .iter()
        .map(|s| {
            s.as_str().map(str::to_owned).ok_or_else(|| TreeError {
                code: "InvalidDefinition",
                detail: key.into(),
            })
        })
        .collect()
}
fn id(v: &Value, key: &str) -> Result<String> {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| TreeError {
            code: "InvalidDefinition",
            detail: key.into(),
        })
}
fn acyclic(
    graph: &BTreeMap<String, Vec<String>>,
    cycle: &'static str,
    missing: &'static str,
) -> Result<()> {
    // Iterative traversal bounds stack use even for deliberately deep input.
    let mut done = BTreeSet::new();
    for start in graph.keys() {
        let mut active = BTreeSet::new();
        let mut work = vec![(start.clone(), false)];
        while let Some((node, exit)) = work.pop() {
            if exit {
                active.remove(&node);
                done.insert(node);
                continue;
            }
            if done.contains(&node) {
                continue;
            }
            if !active.insert(node.clone()) {
                return fail(cycle, node);
            }
            let parents = graph.get(&node).ok_or_else(|| TreeError {
                code: missing,
                detail: node.clone(),
            })?;
            work.push((node, true));
            for parent in parents.iter().rev() {
                work.push((parent.clone(), false));
            }
        }
    }
    Ok(())
}
pub struct Assembly {
    pub manifest: Manifest,
    pub registries: [Value; 2],
    pub baseline_bytes: [Vec<u8>; 2],
}
pub fn assemble(root: &Path) -> Result<Assembly> {
    let mut total = 0;
    let manifest: Manifest =
        serde_json::from_slice(&read(root, "manifest.json", &mut total)?).map_err(invalid)?;
    if manifest.schema_version != "tos_definition_tree_v1"
        || !matches!(manifest.posture.as_str(), CANDIDATE | SELECTED)
        || manifest.envelopes.len() != 2
        || manifest.baseline.len() != 2
        || manifest.entries.len() > MAX_ENTRIES
    {
        return fail("InvalidManifest", "version, posture or inventory bounds");
    }
    match (&*manifest.posture, &manifest.selection_review_ref) {
        (SELECTED, Some(reference))
            if reference.starts_with("ToS/review-ledger/")
                && !reference.ends_with('/')
                && Path::new(reference)
                    .components()
                    .all(|c| matches!(c, Component::Normal(_))) => {}
        (CANDIDATE, None) => {}
        _ => {
            return fail(
                "InvalidManifest",
                "selected authored tree requires a repository-relative selection review; candidate cannot claim selection",
            );
        }
    }
    let mut baseline_bytes: [Vec<u8>; 2] = Default::default();
    let mut baselines = vec![];
    let mut registries = vec![];
    for i in 0..2 {
        baseline_bytes[i] = read(root, &manifest.baseline[i].path, &mut total)?;
        if Digest256::of_bytes(&baseline_bytes[i]).to_hex() != manifest.baseline[i].sha256 {
            return fail("BaselineDigestMismatch", &manifest.baseline[i].path);
        }
        baselines.push(serde_json::from_slice::<Value>(&baseline_bytes[i]).map_err(invalid)?);
        registries.push(value(root, &manifest.envelopes[i], &mut total)?);
    }
    for (collection, _, index) in GROUPS {
        if registries[index].get(collection) != Some(&json!([])) {
            return fail("InvalidManifest", "envelope collection must be empty");
        }
    }
    let mut records: BTreeMap<String, (String, Value)> = BTreeMap::new();
    let mut paths = BTreeSet::new();
    let mut nav: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for root in &manifest.navigation_roots {
        if nav.insert(root.clone(), vec![]).is_some() {
            return fail("DuplicateCanonicalDefinition", root);
        }
    }
    if nav.is_empty() {
        return fail("UnplacedDefinition", "no navigation root");
    }
    for (group, parent) in &manifest.navigation_groups {
        if nav.insert(group.clone(), vec![parent.clone()]).is_some() {
            return fail("DuplicateCanonicalDefinition", group);
        }
    }
    for e in &manifest.entries {
        let (_, key, index) = GROUPS
            .iter()
            .find(|(c, _, _)| *c == e.collection)
            .ok_or_else(|| TreeError {
                code: "InvalidManifest",
                detail: e.collection.clone(),
            })?;
        if records.contains_key(&e.id) || !paths.insert(e.path.clone()) || nav.contains_key(&e.id) {
            return fail("DuplicateCanonicalDefinition", &e.id);
        }
        if !e.path.starts_with("canonical/") {
            return fail("InvalidManifest", "home outside canonical/");
        }
        let record = value(root, &e.path, &mut total)?;
        if id(&record, key)? != e.id {
            return fail("InvalidManifest", "entry ID differs from definition");
        }
        records.insert(e.id.clone(), (e.collection.clone(), record.clone()));
        nav.insert(e.id.clone(), vec![e.navigation_parent.clone()]);
        registries[*index][&e.collection]
            .as_array_mut()
            .unwrap()
            .push(record);
    }
    acyclic(&nav, "NavigationCycle", "UnplacedDefinition")?;
    // Every canonical JSON file must be named once; stale copies cannot become shadow homes.
    let mut queue = vec![safe(root, "canonical")?];
    let mut seen = 0;
    while let Some(dir) = queue.pop() {
        for item in fs::read_dir(dir).map_err(io)? {
            let item = item.map_err(io)?;
            let kind = item.file_type().map_err(io)?;
            seen += 1;
            if seen > MAX_ENTRIES * 8 {
                return fail("ResourceLimit", "canonical inventory");
            }
            if kind.is_symlink() {
                return fail("UnsafePath", "canonical symlink");
            }
            if kind.is_dir() {
                queue.push(item.path());
            } else {
                let rel = item
                    .path()
                    .strip_prefix(root)
                    .map_err(|_| TreeError {
                        code: "UnsafePath",
                        detail: "root".into(),
                    })?
                    .to_string_lossy()
                    .to_string();
                if !paths.contains(&rel) {
                    return fail("UnlistedDefinition", rel);
                }
            }
        }
    }
    for (collection, parent_key) in [
        ("types", "parent_type_ids"),
        ("relations", "parent_relation_type_ids"),
    ] {
        let mut graph = BTreeMap::new();
        for (id, (c, record)) in &records {
            if c == collection {
                graph.insert(id.clone(), strings(record, parent_key)?);
            }
        }
        acyclic(&graph, "SemanticCycle", "UnresolvedParent")?;
    }
    // Baseline identity is durable. Exact historical edge retention is the initial
    // migration gate; selected source evolves under its normal owner review route.
    check_historical_records(&registries, &baselines, manifest.posture == CANDIDATE)?;
    for (_, (collection, record)) in &records {
        let refs: Vec<(&str, &str)> = match collection.as_str() {
            "types" => vec![("supersedes_type_id", "types")],
            "relations" => vec![
                ("domain_type_ids", "types"),
                ("range_type_ids", "types"),
                ("inverse_relation_type_id", "relations"),
                ("supersedes_relation_type_id", "relations"),
            ],
            _ => vec![("applies_to", "types")],
        };
        for (field, target) in refs {
            let values = match record.get(field) {
                Some(Value::Array(a)) => a.clone(),
                Some(Value::String(s)) => vec![json!(s)],
                None | Some(Value::Null) => vec![],
                _ => return fail("InvalidDefinition", field),
            };
            for v in values {
                if !v
                    .as_str()
                    .and_then(|s| records.get(s))
                    .is_some_and(|(c, _)| c == target)
                {
                    return fail("DanglingReference", format!("{field}: {v}"));
                }
            }
        }
    }
    Ok(Assembly {
        manifest,
        registries: registries.try_into().unwrap(),
        baseline_bytes,
    })
}
fn check_historical_records(
    registries: &[Value],
    baselines: &[Value],
    preserve_edges: bool,
) -> Result<()> {
    for (collection, key, index) in GROUPS {
        let baseline = baselines[index][collection]
            .as_array()
            .ok_or_else(|| TreeError {
                code: "InvalidManifest",
                detail: collection.into(),
            })?;
        let current = registries[index][collection]
            .as_array()
            .ok_or_else(|| TreeError {
                code: "InvalidManifest",
                detail: collection.into(),
            })?;
        let current_by_id: BTreeMap<_, _> = current
            .iter()
            .map(|v| Ok((id(v, key)?, v)))
            .collect::<Result<_>>()?;
        for old in baseline {
            let old_id = id(old, key)?;
            let record = current_by_id.get(&old_id).ok_or_else(|| TreeError {
                code: "MissingBaselineDefinition",
                detail: old_id.clone(),
            })?;
            if preserve_edges {
                let parent_key = match collection {
                    "types" => Some("parent_type_ids"),
                    "relations" => Some("parent_relation_type_ids"),
                    _ => None,
                };
                if let Some(p) = parent_key {
                    if strings(old, p)?.into_iter().collect::<BTreeSet<_>>()
                        != strings(record, p)?.into_iter().collect()
                    {
                        return fail("SemanticEdgeLoss", old_id);
                    }
                }
            }
        }
    }
    Ok(())
}
impl Assembly {
    pub fn evidence(&self) -> Result<Value> {
        let mut result = vec![];
        for i in 0..2 {
            let bytes = pretty(&self.registries[i])?;
            let baseline: Value =
                serde_json::from_slice(&self.baseline_bytes[i]).map_err(invalid)?;
            result.push(json!({"file": FILES[i], "parsed_value_equal": baseline == self.registries[i], "byte_equal": self.baseline_bytes[i] == bytes, "baseline_sha256": Digest256::of_bytes(&self.baseline_bytes[i]).to_hex(), "assembled_sha256": Digest256::of_bytes(&bytes).to_hex()}));
        }
        Ok(
            json!({"posture":self.manifest.posture, "selection_review_ref":self.manifest.selection_review_ref, "home_count":self.manifest.entries.len(), "registries":result}),
        )
    }
    /// Initial/forensic migration guard, deliberately separate from source evolution.
    pub fn check_migration(&self) -> Result<()> {
        let baseline = self
            .baseline_bytes
            .iter()
            .map(|bytes| serde_json::from_slice::<Value>(bytes).map_err(invalid))
            .collect::<Result<Vec<_>>>()?;
        check_historical_records(&self.registries, &baseline, true)?;
        if baseline.as_slice() != self.registries.as_slice() {
            return fail(
                "MigrationValueDivergence",
                "initial migration requires full parsed-value equality; use current-source checks for reviewed evolution",
            );
        }
        Ok(())
    }
    /// Compare both exact compatibility filenames in a plain directory. No output
    /// manifest/evidence is read; unrelated files are ignored, subsets are refused.
    pub fn check_compatibility(&self, output: &Path) -> Result<()> {
        self.check_generated(output)
    }
    pub fn check_generated(&self, output: &Path) -> Result<()> {
        let mut total = 0;
        for (i, name) in FILES.iter().enumerate() {
            if read(output, name, &mut total)? != pretty(&self.registries[i])? {
                return fail("GeneratedOutputDivergence", *name);
            }
        }
        Ok(())
    }
    pub fn write_candidate(&self, output: &Path, interrupt_after_first: bool) -> Result<()> {
        // A fresh directory and one final rename commit the complete pair together.
        // No overwrite, no current/publication pointer, and no cleanup of somebody else's files.
        if fs::symlink_metadata(output).is_ok() {
            return fail("CandidateExists", output.display().to_string());
        }
        let parent = output.parent().ok_or_else(|| TreeError {
            code: "UnsafePath",
            detail: "candidate parent".into(),
        })?;
        fs::create_dir_all(parent).map_err(io)?;
        let stem = output
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| TreeError {
                code: "UnsafePath",
                detail: "candidate name".into(),
            })?;
        let stage = parent.join(format!(".{stem}.stage-{}", std::process::id()));
        fs::create_dir(&stage).map_err(io)?;
        let result = (|| {
            put(&stage, FILES[0], &pretty(&self.registries[0])?)?;
            if interrupt_after_first {
                return fail(
                    "NotPublished",
                    "interrupted before complete candidate validation",
                );
            }
            put(&stage, FILES[1], &pretty(&self.registries[1])?)?;
            self.check_generated(&stage)?;
            put(&stage, "evidence.json", &pretty(&self.evidence()?)?)?;
            File::open(&stage).map_err(io)?.sync_all().map_err(io)?;
            if fs::symlink_metadata(output).is_ok() {
                return fail("CandidateExists", output.display().to_string());
            }
            rename_new(&stage, output)?;
            File::open(parent).map_err(io)?.sync_all().map_err(io)
        })();
        if result.is_err() && stage.exists() {
            let _ = fs::remove_dir_all(stage);
        }
        result
    }
}
#[cfg(target_os = "linux")]
fn rename_new(from: &Path, to: &Path) -> Result<()> {
    use std::os::unix::ffi::OsStrExt;
    let from = std::ffi::CString::new(from.as_os_str().as_bytes()).map_err(|_| TreeError {
        code: "UnsafePath",
        detail: "NUL path".into(),
    })?;
    let to = std::ffi::CString::new(to.as_os_str().as_bytes()).map_err(|_| TreeError {
        code: "UnsafePath",
        detail: "NUL path".into(),
    })?;
    // RENAME_NOREPLACE closes the existence-check race without replacing even an empty directory.
    let result = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            from.as_ptr(),
            libc::AT_FDCWD,
            to.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io(std::io::Error::last_os_error()))
    }
}
#[cfg(not(target_os = "linux"))]
fn rename_new(_from: &Path, _to: &Path) -> Result<()> {
    fail(
        "UnsupportedAtomicPublish",
        "candidate commit requires atomic no-replace rename",
    )
}
pub fn split(entity: &Path, relation: &Path, output: &Path) -> Result<()> {
    if fs::symlink_metadata(output).is_ok() {
        return fail("CandidateExists", output.display().to_string());
    }
    let mut raw = vec![];
    let mut docs = vec![];
    let mut total = 0;
    for p in [entity, relation] {
        let parent = p.parent().ok_or_else(|| TreeError {
            code: "UnsafePath",
            detail: "input parent".into(),
        })?;
        let name = p
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| TreeError {
                code: "UnsafePath",
                detail: "input name".into(),
            })?;
        let b = read(parent, name, &mut total)?;
        docs.push(serde_json::from_slice::<Value>(&b).map_err(invalid)?);
        raw.push(b);
    }
    fs::create_dir_all(output).map_err(io)?;
    let mut manifest = Manifest {
        schema_version: "tos_definition_tree_v1".into(),
        posture: CANDIDATE.into(),
        selection_review_ref: None,
        baseline: vec![],
        envelopes: vec![],
        navigation_roots: vec![
            "navigation.entities".into(),
            "navigation.properties".into(),
            "navigation.relations".into(),
        ],
        navigation_groups: BTreeMap::new(),
        entries: vec![],
    };
    for i in 0..2 {
        let p = format!("baseline/{}", FILES[i]);
        put(output, &p, &raw[i])?;
        manifest.baseline.push(Baseline {
            path: p,
            sha256: Digest256::of_bytes(&raw[i]).to_hex(),
        });
    }
    for (collection, key, index) in GROUPS {
        let entries = docs[index][collection]
            .as_array()
            .ok_or_else(|| TreeError {
                code: "InvalidDefinition",
                detail: collection.into(),
            })?
            .clone();
        let branch = match collection {
            "types" => "entities",
            "relations" => "relations",
            _ => "properties",
        };
        let parent_key = match collection {
            "types" => Some("parent_type_ids"),
            "relations" => Some("parent_relation_type_ids"),
            _ => None,
        };
        let mut primary = BTreeMap::new();
        for record in &entries {
            let parents = match parent_key {
                Some(k) => strings(record, k)?,
                None => vec![],
            };
            primary.insert(
                id(record, key)?,
                parents.first().cloned().into_iter().collect::<Vec<_>>(),
            );
        }
        acyclic(&primary, "SemanticCycle", "UnresolvedParent")?;
        for record in entries {
            let entry_id = id(&record, key)?;
            if entry_id
                .bytes()
                .any(|b| !b.is_ascii_lowercase() && !b.is_ascii_digit() && !b".-_".contains(&b))
            {
                return fail("InvalidDefinition", entry_id);
            }
            let parent = match collection {
                "types" => strings(&record, "parent_type_ids")?.first().cloned(),
                "relations" => strings(&record, "parent_relation_type_ids")?
                    .first()
                    .cloned(),
                _ => None,
            }
            .unwrap_or_else(|| format!("navigation.{branch}"));
            let mut ancestors = vec![entry_id.clone()];
            let mut current = entry_id.clone();
            while let Some(parent) = primary.get(&current).and_then(|p| p.first()) {
                ancestors.push(parent.clone());
                current = parent.clone();
            }
            ancestors.reverse();
            let path = format!("canonical/{branch}/{}.json", ancestors.join("/"));
            put(output, &path, &pretty(&record)?)?;
            manifest.entries.push(Entry {
                id: entry_id,
                collection: collection.into(),
                path,
                navigation_parent: parent,
                previous_paths: vec![],
            });
        }
        docs[index][collection] = json!([]);
    }
    for (i, doc) in docs.iter().enumerate() {
        let p = format!("shared/{}", FILES[i]);
        put(output, &p, &pretty(doc)?)?;
        manifest.envelopes.push(p);
    }
    put(output, "manifest.json", &pretty(&manifest)?)?;
    let assembled = assemble(output)?;
    if assembled.evidence()?["registries"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["parsed_value_equal"] != true)
    {
        return fail("SplitMismatch", "full parsed-value equality failed");
    }
    Ok(())
}
