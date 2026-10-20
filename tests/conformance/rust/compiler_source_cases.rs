//! Actual cold repository plan/render through the existing selected cut and
//! captured-software fixture. Scope is the finite declared fixture Git tree.
use super::*;
use serde_json::json;
use std::collections::BTreeMap;
use std::io::Write;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tos_compiler::knowledge_repository_source::{
    RepositorySourceLimits, plan_repository_source_inputs, render_repository_source_plan,
};
use tos_compiler::knowledge_stage::{
    ExactInputReceipt, KnowledgeStage, StageIsolation, StageLimits, StageOwner, WritePhase,
};
use tos_compiler::{
    Limits, QueryVocabulary, RepositoryRootInput, SourceBinding, TopologyLimits,
    prepare_repository_topology,
};
use tos_validation::executor::{ExactWorkerIdentity, ExecutorBudget};
use tos_validation::source_cut::{CutSchemaExecutor, CutWorkerLimits, CutWorkerSchemaExecutor};

include!("native_corpus_query.rs");
#[path = "native_managed_corpus_consumer.rs"]
mod native_managed_corpus_consumer;

// The frozen Python packets predate the source-history storage migration.
// Description and manifest fixity follow the independently parsed authored input;
// every other historical branch field and all projection assertions stay exact.
fn reconcile_source_home_inputs(expected: &mut Value, files: &BTreeMap<String, Vec<u8>>) {
    let home_raw = &files["ToS/source_home.manifest.json"];
    let home: Value = serde_json::from_slice(home_raw).unwrap();
    let home_sha256 = Digest256::of_bytes(home_raw).to_hex();
    let branch = home["branches"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == "source_witnesses")
        .unwrap();
    fn visit(value: &mut Value, branch: &Value, home_sha256: &str) -> usize {
        if value.get("manifest_kind").and_then(Value::as_str) == Some("source_home_manifest") {
            assert_eq!(value["path"], "ToS/source_home.manifest.json");
            assert_eq!(
                value["sha256"],
                "89d243df361586d2435748ec5d32b5929ddfa2a0742d6776f1e661319635126c"
            );
            value["sha256"] = json!(home_sha256);
        }
        if value.get("id").and_then(Value::as_str) == Some("source_witnesses") {
            for field in ["id", "path", "owner_surface"] {
                assert_eq!(value[field], branch[field], "unchanged branch {field}");
            }
            assert_eq!(
                value["role"],
                "Tracked corpus catalog, bibliographic work/expression/edition/item topology, parallel physical-artifact and scholarly-composite witnesses, narrowly gitignored immutable payloads, fixity, provenance, rights, and primary source-facing witness material."
            );
            value["role"] = branch["role"].clone();
            return 1;
        }
        match value {
            Value::Array(rows) => rows
                .iter_mut()
                .map(|row| visit(row, branch, home_sha256))
                .sum(),
            Value::Object(fields) => fields
                .values_mut()
                .map(|row| visit(row, branch, home_sha256))
                .sum(),
            _ => 0,
        }
    }
    assert!(
        visit(expected, branch, &home_sha256) > 0,
        "historical source-witness branch present"
    );
}

struct FixtureOwner;
impl StageOwner for FixtureOwner {
    fn verify_receipt(&self, _: &ExactInputReceipt) -> tos_compiler::Result<()> {
        Ok(())
    }
    fn recheck_sealed_cut(&self, _: &ExactInputReceipt) -> tos_compiler::Result<()> {
        Ok(())
    }
}
struct FixtureIsolation;
impl StageIsolation for FixtureIsolation {
    fn verify(&self, _: &Path, _: StageLimits, _: WritePhase) -> tos_compiler::Result<()> {
        Ok(())
    }
}
struct NativeSelectedIsolation<'a> {
    started: Instant,
    deadline: Instant,
    cancelled: &'a AtomicBool,
    enabled: AtomicBool,
    catalog_seen: AtomicBool,
    search_seen: AtomicBool,
}
impl StageIsolation for NativeSelectedIsolation<'_> {
    fn verify(&self, _: &Path, _: StageLimits, phase: WritePhase) -> tos_compiler::Result<()> {
        if self.cancelled.load(Ordering::Relaxed) {
            return Err(tos_compiler::Error::Budget("native corpus cancelled"));
        }
        if Instant::now() >= self.deadline {
            return Err(tos_compiler::Error::Budget("native corpus whole deadline"));
        }
        if self.enabled.load(Ordering::Relaxed) {
            let seen = match phase {
                WritePhase::Catalog => Some(&self.catalog_seen),
                WritePhase::Search => Some(&self.search_seen),
                _ => None,
            };
            if let Some(seen) = seen {
                if !seen.swap(true, Ordering::Relaxed) {
                    let _ = writeln!(
                        std::io::stderr().lock(),
                        "native corpus selected write phase={phase:?} elapsed_seconds={}",
                        self.started.elapsed().as_secs()
                    );
                }
            }
        }
        Ok(())
    }
}
fn git(root: &Path, args: &[&str]) -> Vec<u8> {
    let mut command = Command::new("git");
    command.arg("-C").arg(root).args(args);
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("GIT_") {
            command.env_remove(key);
        }
    }
    let out = command
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "bounded fixture Git failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    out.stdout
}
fn schema_sources(repository: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut authored = BTreeMap::new();
    for entry in fs::read_dir(repository.join("ToS/contracts")).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().into_string().unwrap();
        if name.ends_with(".schema.json") {
            authored.insert(
                format!("ToS/contracts/{name}"),
                fs::read(entry.path()).unwrap(),
            );
        }
    }
    authored
}
// CurrentDeclared is test-owned source evidence, never a production plan/output.
// One logical construction counter is shared by intake, rows and pure variants.
struct CurrentReference {
    commit: String,
    left: usize,
    visits_left: usize,
    deadline: Instant,
}
impl CurrentReference {
    fn visit(&mut self, count: usize) -> Result<(), &'static str> {
        self.visits_left = self
            .visits_left
            .checked_sub(count)
            .ok_or("reference visit limit")?;
        self.charge(0)
    }
    fn charge(&mut self, bytes: usize) -> Result<(), &'static str> {
        if Instant::now() >= self.deadline {
            return Err("reference deadline");
        }
        self.left = self.left.checked_sub(bytes).ok_or("reference work limit")?;
        Ok(())
    }
    fn parsed(&mut self, raw: &[u8]) -> Result<Value, &'static str> {
        self.visit(raw.len())?;
        // Logical source-value/header/temporary allowance, not allocator/RSS.
        self.charge(
            raw.len()
                .checked_mul(128)
                .and_then(|n| n.checked_add(4096))
                .ok_or("reference parse overflow")?,
        )?;
        serde_json::from_slice(raw).map_err(|_| "reference JSON")
    }
    fn git_read(
        &mut self,
        repository: &Path,
        args: &[&str],
        limit: usize,
    ) -> Result<Vec<u8>, &'static str> {
        use std::io::{Read, Seek, SeekFrom};
        use std::process::Stdio;
        // Same logical output allowance plus explicit spool/read/poll holders.
        // A pipe read/wait could otherwise outlive the case's cutoff check.
        self.charge(limit.checked_add(1).ok_or("reference read overflow")?)?;
        self.charge(
            limit
                .checked_add(4096 + 512)
                .ok_or("reference spool overflow")?,
        )?;
        let mut spool = tempfile::tempfile().map_err(|_| "reference Git spool")?;
        let stdout = spool
            .try_clone()
            .map_err(|_| "reference Git spool handle")?;
        let mut command = Command::new("git");
        command
            .arg("--literal-pathspecs")
            .arg("-C")
            .arg(repository)
            .args(args);
        for (key, _) in std::env::vars_os() {
            if key.to_string_lossy().starts_with("GIT_") {
                command.env_remove(key);
            }
        }
        self.charge(0)?;
        let mut child = command
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_NO_REPLACE_OBJECTS", "1")
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| "reference Git spawn")?;
        let status = loop {
            // Poll the actual child and its actual output, not an external stat
            // of an unrelated pathname. Never block on a live stdout pipe.
            let refusal = if self.visit(1).is_err() {
                Some("reference cutoff/work limit")
            } else if Instant::now() >= self.deadline {
                Some("reference deadline")
            } else {
                match spool.metadata() {
                    Ok(meta) if meta.len() <= limit as u64 => None,
                    Ok(_) => Some("reference Git byte limit"),
                    Err(_) => Some("reference Git spool metadata"),
                }
            };
            if let Some(reason) = refusal {
                let _ = child.kill();
                let _ = child.wait(); // reap only after terminating the child
                return Err(reason);
            }
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => {
                    let left = self.deadline.saturating_duration_since(Instant::now());
                    std::thread::sleep(left.min(Duration::from_millis(5)));
                }
                Err(_) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("reference Git wait");
                }
            }
        };
        self.charge(0)?;
        if !status.success() {
            return Err("reference Git status");
        }
        if spool
            .metadata()
            .map_err(|_| "reference Git spool metadata")?
            .len()
            > limit as u64
        {
            return Err("reference Git byte limit");
        }
        spool
            .seek(SeekFrom::Start(0))
            .map_err(|_| "reference Git spool seek")?;
        let mut output = Vec::with_capacity(limit + 1);
        let mut buffer = [0u8; 4096];
        loop {
            self.charge(0)?;
            let room = (limit + 1 - output.len()).min(buffer.len());
            let count = spool
                .read(&mut buffer[..room])
                .map_err(|_| "reference Git read")?;
            if count == 0 {
                break;
            }
            output.extend_from_slice(&buffer[..count]);
            if output.len() > limit {
                return Err("reference Git byte limit");
            }
        }
        self.charge(0)?;
        Ok(output)
    }
    fn pinned_blob(&mut self, repository: &Path, path: &str) -> Result<Vec<u8>, &'static str> {
        RelativePath::parse(path).map_err(|_| "reference path")?;
        let pin = self.commit.clone();
        let entry = self.git_read(
            repository,
            &["ls-tree", "-z", "--full-tree", &pin, "--", path],
            path.len().checked_add(128).ok_or("reference path bytes")?,
        )?;
        let text = std::str::from_utf8(&entry).map_err(|_| "reference file metadata UTF8")?;
        let expected_suffix = format!("\t{path}\0");
        if !text.starts_with("100644 blob ")
            || !text.ends_with(&expected_suffix)
            || text.bytes().filter(|b| *b == 0).count() != 1
        {
            return Err("reference regular pinned file");
        }
        let spec = format!("{}:{path}", self.commit);
        let size = self.git_read(repository, &["cat-file", "-s", &spec], 64)?;
        let size: usize = std::str::from_utf8(&size)
            .map_err(|_| "reference size UTF8")?
            .trim()
            .parse()
            .map_err(|_| "reference size")?;
        if size > 2 * 1024 * 1024 {
            return Err("reference member limit");
        }
        let raw = self.git_read(repository, &["cat-file", "blob", &spec], size)?;
        if raw.len() != size {
            return Err("reference blob size");
        }
        Ok(raw)
    }
}
fn reference_direct_schemas(metadata: &[u8]) -> Result<Vec<&str>, &'static str> {
    if !metadata.ends_with(&[0]) {
        return Err("reference tree EOF");
    }
    let text = std::str::from_utf8(metadata).map_err(|_| "reference tree UTF8")?;
    let mut selected = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for (index, entry) in text.split_terminator('\0').enumerate() {
        if index >= 512 {
            return Err("reference tree count");
        }
        let (fields, path) = entry.split_once('\t').ok_or("reference tree row")?;
        let fields = fields.split_whitespace().collect::<Vec<_>>();
        if fields.len() != 3 {
            return Err("reference tree fields");
        }
        RelativePath::parse(path).map_err(|_| "reference tree path")?;
        let Some(name) = path.strip_prefix("ToS/contracts/") else {
            return Err("reference tree root");
        };
        if name.contains('/') || !name.ends_with(".schema.json") {
            continue;
        }
        if fields[0] != "100644"
            || fields[1] != "blob"
            || fields[2].len() != 40
            || !fields[2].bytes().all(|c| c.is_ascii_hexdigit())
        {
            return Err("reference schema blob");
        }
        if !seen.insert(path) {
            return Err("reference duplicate schema");
        }
        selected.push(path);
    }
    if selected.is_empty() {
        return Err("reference empty schemas");
    }
    selected.sort_unstable();
    Ok(selected)
}
fn reference_insert(
    files: &mut BTreeMap<String, Vec<u8>>,
    path: &str,
    raw: Vec<u8>,
) -> Result<(), &'static str> {
    RelativePath::parse(path).map_err(|_| "reference input path")?;
    if files.contains_key(path) {
        return Err("reference duplicate input");
    }
    if files.len() >= 512 || raw.len() > 2 * 1024 * 1024 {
        return Err("reference input count/member");
    }
    let total = files
        .values()
        .try_fold(raw.len(), |n, v| n.checked_add(v.len()))
        .ok_or("reference input overflow")?;
    if total > 16 * 1024 * 1024 {
        return Err("reference input bytes");
    }
    files.insert(path.to_owned(), raw);
    Ok(())
}
fn sources(
    deadline: Instant,
) -> (
    BTreeMap<String, Vec<u8>>,
    BTreeMap<String, Vec<u8>>,
    CurrentReference,
) {
    let repository = super::validation_cut_cases::repository();
    let mut reference = CurrentReference {
        commit: String::new(),
        left: 32 * 1024 * 1024,
        visits_left: 1_000_000,
        deadline,
    };
    reference.charge(16 * 1024).unwrap(); // one live Git/read/parse control workspace
    let commit = reference
        .git_read(&repository, &["rev-parse", "HEAD^{commit}"], 64)
        .unwrap();
    reference.commit = std::str::from_utf8(&commit).unwrap().trim().to_owned();
    assert_eq!(reference.commit.len(), 40);
    assert!(reference.commit.bytes().all(|c| c.is_ascii_hexdigit()));
    let pin = reference.commit.clone();
    let metadata = reference
        .git_read(
            &repository,
            &["ls-tree", "-rz", "--full-tree", &pin, "--", "ToS/contracts"],
            2 * 1024 * 1024,
        )
        .unwrap();
    // Reserve bounded path/index state before parsing the tree metadata.
    reference
        .charge(metadata.len().checked_mul(4).unwrap() + 512 * 128)
        .unwrap();
    reference
        .visit(metadata.len().checked_add(512 * 512).unwrap())
        .unwrap();
    let mut authored = BTreeMap::new();
    for path in reference_direct_schemas(&metadata).unwrap() {
        let raw = reference.pinned_blob(&repository, path).unwrap();
        reference_insert(&mut authored, path, raw).unwrap();
    }
    let home = reference
        .pinned_blob(&repository, "ToS/source_home.manifest.json")
        .unwrap();
    let home_value = reference.parsed(&home).unwrap();
    reference_insert(&mut authored, "ToS/source_home.manifest.json", home).unwrap();
    for branch in home_value["branches"].as_array().unwrap() {
        let surface = branch["owner_surface"].as_str().unwrap();
        let raw = reference.pinned_blob(&repository, surface).unwrap();
        reference_insert(&mut authored, surface, raw).unwrap();
    }
    // This is the existing capture copy; reference rows borrow these raw maps.
    reference
        .charge(authored.iter().map(|(p, v)| p.len() + v.len() + 256).sum())
        .unwrap();
    let mut captured = authored.clone();
    for path in [
        "ToS/public-compatibility/source_node.example.json",
        "ToS/philosophy/philosophy.manifest.json",
        "rust/crates/tos-compiler/src/lib.rs",
    ] {
        let raw = reference.pinned_blob(&repository, path).unwrap();
        reference_insert(&mut captured, path, raw).unwrap();
    }
    (authored, captured, reference)
}
// These finite roles are a test contract, not calls to the production classifier.
fn reference_resource_role(path: &str) -> Result<(&str, &str, &str), &'static str> {
    let tail = path
        .strip_prefix("ToS/")
        .ok_or("reference non-ToS resource")?;
    let directory = tail.split_once('/').map(|(v, _)| v);
    let (owner, layer) = match directory {
        None => ("ToS", "source_home"),
        Some("candidate-intake") => ("ToS/candidate-intake", "candidate_intake"),
        Some("canon") => ("ToS/canon", "canon"),
        Some("contracts") => ("ToS/contracts", "contract"),
        Some("doctrine") => ("ToS/doctrine", "doctrine"),
        Some("derived-exports") => ("ToS/derived-exports", "derived_export"),
        Some("philosophy") => ("ToS/philosophy", "domain_topology"),
        Some("public-compatibility") => ("ToS/public-compatibility", "public_compatibility"),
        Some("research-packets") => ("ToS/research-packets", "research_packet"),
        Some("review-ledger") => ("ToS/review-ledger", "review_evidence"),
        Some("source-witnesses") => ("ToS/source-witnesses", "source_witness"),
        Some("zarathustra") => ("ToS/zarathustra", "golden_route_orientation"),
        _ => return Err("reference undeclared resource role"),
    };
    let name = path.rsplit('/').next().unwrap();
    let kind = match name {
        "AGENTS.md" => "route_card",
        "source_home.manifest.json" => "source_home_manifest",
        "philosophy.manifest.json" => "philosophy_manifest",
        "branch.manifest.json" => "branch_manifest",
        "node.json" => "node_payload",
        "edges.csv" => "relation_pack",
        _ if directory == Some("source-witnesses") => "source_witness",
        _ if directory == Some("research-packets") => "research_packet",
        _ if directory == Some("review-ledger") => "review_note",
        _ if directory == Some("derived-exports") => "derived_export",
        _ if directory == Some("contracts") && name.ends_with(".schema.json") => "contract_schema",
        _ if name.ends_with(".json") => "json",
        _ if name.ends_with(".md") => "markdown",
        _ if name.ends_with(".csv") => "tabular",
        _ => return Err("reference unsupported resource kind"),
    };
    Ok((kind, owner, layer))
}
fn reference_metadata_matches(
    files: &BTreeMap<&str, &[u8]>,
    observed: &[(&str, usize, Digest256)],
) -> bool {
    files.len() == observed.len()
        && observed.iter().enumerate().all(|(i, (p, n, h))| {
            !observed[..i].iter().any(|(prior, _, _)| prior == p)
                && files
                    .get(*p)
                    .is_some_and(|raw| raw.len() == *n && Digest256::of_bytes(raw) == *h)
        })
}
fn reference_repository_rows(
    reference: &mut CurrentReference,
    files: &BTreeMap<String, Vec<u8>>,
) -> Result<BTreeMap<&'static str, Vec<Value>>, &'static str> {
    if files.len() > 512 {
        return Err("reference row input count");
    }
    let total = files
        .values()
        .try_fold(0usize, |n, v| n.checked_add(v.len()))
        .ok_or("reference row input overflow")?;
    if total > 16 * 1024 * 1024 {
        return Err("reference row input bytes");
    }
    reference.visit(
        files
            .keys()
            .try_fold(
                files.len().checked_mul(128).ok_or("reference row visits")?,
                |n, p| n.checked_add(p.len()),
            )
            .ok_or("reference row visits")?,
    )?;
    let mut result = BTreeMap::from([
        ("branches", Vec::new()),
        ("manifests", Vec::new()),
        ("resources", Vec::new()),
        ("source_order", Vec::new()),
    ]);
    let home = reference.parsed(
        files
            .get("ToS/source_home.manifest.json")
            .ok_or("reference home absent")?,
    )?;
    let mut ids = std::collections::BTreeSet::new();
    for (ordinal, b) in home["branches"]
        .as_array()
        .ok_or("reference branches")?
        .iter()
        .enumerate()
    {
        let id = b["id"].as_str().ok_or("reference branch ID")?;
        let path = b["path"].as_str().ok_or("reference branch path")?;
        let surface = b["owner_surface"]
            .as_str()
            .ok_or("reference branch owner")?;
        if !ids.insert(id) || !files.contains_key(surface) {
            return Err("reference duplicate/missing branch");
        }
        let text_bytes = [
            id.len(),
            path.len(),
            surface.len(),
            b["role"].as_str().ok_or("reference branch role")?.len(),
        ]
        .into_iter()
        .try_fold(0usize, usize::checked_add)
        .ok_or("reference branch bytes")?;
        reference.charge(
            text_bytes
                .checked_mul(8)
                .and_then(|n| n.checked_add(8192))
                .ok_or("reference branch bytes")?,
        )?;
        let layer_path = format!("{}/AGENTS.md", path.trim_end_matches('/'));
        let (_, _, layer) = reference_resource_role(&layer_path)?;
        result.get_mut("branches").unwrap().push(json!({"id":id,"path":path,"owner_surface":surface,"authority_layer":layer,"role":b["role"]}));
        result
            .get_mut("source_order")
            .unwrap()
            .push(json!({"collection":"branches","id":id,"ordinal":ordinal}));
    }
    for (ordinal, (path, raw)) in files
        .iter()
        .filter(|(p, _)| p.starts_with("ToS/"))
        .enumerate()
    {
        if raw.len() > 2 * 1024 * 1024 {
            return Err("reference row member bytes");
        }
        reference.charge(
            8192usize
                .checked_add(path.len().checked_mul(8).ok_or("reference row overflow")?)
                .ok_or("reference row overflow")?,
        )?;
        let (kind, owner, layer) = reference_resource_role(path)?;
        result.get_mut("resources").unwrap().push(json!({"path":path,"resource_kind":kind,"owner_branch":owner,"authority_layer":layer,"sha256":Digest256::of_bytes(raw).to_hex(),"size_bytes":raw.len()}));
        result
            .get_mut("source_order")
            .unwrap()
            .push(json!({"collection":"resources","id":path,"ordinal":ordinal}));
        if path.ends_with(".manifest.json") {
            let value = reference.parsed(raw)?;
            let version = match value.get("schema_version") {
                None | Some(Value::Null) => "",
                Some(Value::String(s)) => s,
                _ => return Err("reference manifest version profile"),
            };
            let declared = if path == "ToS/source_home.manifest.json" {
                value.get("home")
            } else {
                value.get("path")
            }
            .and_then(Value::as_str);
            let n = result["manifests"].len();
            result.get_mut("manifests").unwrap().push(json!({"path":path,"manifest_kind":kind,"owner_branch":owner,"authority_layer":layer,"schema_version":version,"branch_id":value.get("branch_id").and_then(Value::as_str),"declared_path":declared,"sha256":Digest256::of_bytes(raw).to_hex()}));
            result
                .get_mut("source_order")
                .unwrap()
                .push(json!({"collection":"manifests","id":path,"ordinal":n}));
        }
    }
    // Explicit logical emission bound is prepaid in each row's allowance.
    for rows in result.values() {
        if serde_json::to_vec(rows)
            .map_err(|_| "reference row JSON")?
            .len()
            > 16 * 1024 * 1024
        {
            return Err("reference row output bytes");
        }
    }
    Ok(result)
}
fn reference_input_variants(reference: &mut CurrentReference) {
    let h = "0000000000000000000000000000000000000000";
    for (name, body) in [
        (
            "different-a.schema.json",
            b"{\"type\":\"string\"}".as_slice(),
        ),
        (
            "renamed-b.schema.json",
            b"{\"type\":\"integer\"}".as_slice(),
        ),
    ] {
        reference.charge(32 * 1024).unwrap();
        reference.visit(4096).unwrap();
        let path = format!("ToS/contracts/{name}");
        let meta = format!("100644 blob {h}\t{path}\0");
        assert_eq!(
            reference_direct_schemas(meta.as_bytes()).unwrap(),
            vec![path.as_str()]
        );
        let nested = format!("{meta}100644 blob {h}\tToS/contracts/nested/ignored.schema.json\0");
        assert_eq!(
            reference_direct_schemas(nested.as_bytes()).unwrap(),
            vec![path.as_str()]
        );
        assert!(reference_direct_schemas(format!("{meta}{meta}").as_bytes()).is_err());
        assert!(reference_direct_schemas(meta.replace("100644", "120000").as_bytes()).is_err());
        let mut files = BTreeMap::new();
        reference_insert(&mut files, &path, body.to_vec()).unwrap();
        assert!(reference_insert(&mut files, &path, body.to_vec()).is_err());
        let borrowed = files
            .iter()
            .map(|(p, raw)| (p.as_str(), raw.as_slice()))
            .collect::<BTreeMap<_, _>>();
        assert!(reference_metadata_matches(
            &borrowed,
            &[(&path, body.len(), Digest256::of_bytes(body))]
        ));
        assert!(!reference_metadata_matches(
            &borrowed,
            &[(&path, body.len() + 1, Digest256::of_bytes(body))]
        ));
        assert!(!reference_metadata_matches(
            &borrowed,
            &[(&path, body.len(), Digest256::of_bytes(b"changed"))]
        ));
        assert!(!reference_metadata_matches(&borrowed, &[]));
        assert!(!reference_metadata_matches(
            &borrowed,
            &[(
                "ToS/contracts/undeclared.schema.json",
                body.len(),
                Digest256::of_bytes(body)
            )]
        ));
        assert_eq!(
            reference_resource_role(&path).unwrap(),
            ("contract_schema", "ToS/contracts", "contract")
        );
        assert!(reference_resource_role("ToS/undeclared/x.json").is_err());
        assert!(reference_direct_schemas(meta.trim_end_matches('\0').as_bytes()).is_err());
        drop(borrowed);
        reference_insert(&mut files, "ToS/source_home.manifest.json",
            br#"{"home":"ToS/","branches":[{"id":"contracts","path":"ToS/contracts","owner_surface":"ToS/contracts/AGENTS.md","role":"Different declared input"}]}"#.to_vec()).unwrap();
        reference_insert(
            &mut files,
            "ToS/contracts/AGENTS.md",
            b"Independent owner surface".to_vec(),
        )
        .unwrap();
        let rows = reference_repository_rows(reference, &files).unwrap();
        let row = rows["resources"]
            .iter()
            .find(|row| row["path"] == path)
            .unwrap();
        assert_eq!(
            row,
            &json!({"path":path,"resource_kind":"contract_schema","owner_branch":"ToS/contracts","authority_layer":"contract","sha256":Digest256::of_bytes(body).to_hex(),"size_bytes":body.len()})
        );
        assert_eq!(rows["branches"][0]["role"], "Different declared input");
        assert_eq!(rows["resources"].len(), files.len());
    }
}

#[test]
fn actual_selected_capture_repository_plan_render_matches_maintained_python() {
    let deadline = Instant::now() + Duration::from_secs(60);
    let (authored, captured, mut reference) = sources(deadline);
    reference_input_variants(&mut reference);
    let mut reference_rows = reference_repository_rows(&mut reference, &captured).unwrap();
    let fixture = tempfile::tempdir().unwrap();
    let git_root = fixture.path().join("selected-git");
    fs::create_dir(&git_root).unwrap();
    git(&git_root, &["init", "-q"]);
    for (name, raw) in &captured {
        let path = git_root.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, raw).unwrap();
    }
    git(
        &git_root,
        &["add", "--", "ToS", "rust/crates/tos-compiler/src/lib.rs"],
    );
    git(
        &git_root,
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-qm",
            "Selected maintained fixture bytes",
        ],
    );
    let commit = String::from_utf8(git(&git_root, &["rev-parse", "HEAD^{commit}"]))
        .unwrap()
        .trim()
        .to_owned();
    let capture = super::source_cut_cases::captured_software_fixture(&git_root, &commit, &["ToS"]);
    let cancelled = AtomicBool::new(false);
    let read_limits = ReadLimits {
        max_manifest_bytes: 2 * 1024 * 1024,
        max_manifest_entries: 512,
        max_selected_object_bytes: 2 * 1024 * 1024,
        json: JsonLimits::default(),
    };
    let inventory = tos_source_store::SoftwareCaptureReader::open(
        &capture.capture,
        &capture.restored,
        capture.selection.clone(),
        read_limits,
        deadline,
        &cancelled,
    )
    .unwrap();
    reference.charge(512 * 256).unwrap();
    reference
        .visit(captured.len().checked_mul(captured.len()).unwrap())
        .unwrap();
    let declared = captured
        .iter()
        .filter(|(p, _)| p.starts_with("ToS/"))
        .map(|(p, raw)| (p.as_str(), raw.as_slice()))
        .collect::<BTreeMap<_, _>>();
    let observed = inventory
        .members()
        .map(|m| {
            (
                m.path.as_str(),
                usize::try_from(m.size_bytes).unwrap(),
                m.sha256,
            )
        })
        .collect::<Vec<_>>();
    assert!(reference_metadata_matches(&declared, &observed));
    drop(declared);
    drop(observed);
    let store = fixture.path().join("source-store");
    let revision = super::validation_cut_cases::write_cut_store(&authored, &store);
    let reader = CorpusReader::open_existing(&store, read_limits).unwrap();
    let cut = reader
        .open_source_cut(
            revision,
            tos_source_store::CutReadLimits {
                max_revisions: 1,
                max_members: 512,
                max_total_bytes: 16 * 1024 * 1024,
                max_member_bytes: 2 * 1024 * 1024,
            },
            deadline,
            &cancelled,
        )
        .unwrap();
    let membership = cut.stream(revision).unwrap().expectation();
    let worker_image =
        super::command_form_cases::schema_image(ExecutorBudget::laboratory(), deadline, &cancelled);
    let mut schemas = CutWorkerSchemaExecutor::from_cut_with_image(
        &cut,
        tos_validation::FormatProfile::LegacyPythonObserved20260923,
        &worker_image,
        ExecutorBudget::laboratory(),
        CutWorkerLimits {
            max_receipts: 128,
            max_receipt_bytes: 131_072,
        },
        deadline,
        &cancelled,
    )
    .unwrap();
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    // Preserve the complete maintained descriptor: its dossier identity binds
    // another registered source even for this repository-only producer case.
    let descriptor_bytes = fs::read(
        repository.join("rust/crates/tos-compiler/tests/fixtures/query-vocabulary.v1.json"),
    )
    .unwrap();
    let vocabulary = QueryVocabulary::parse(
        &descriptor_bytes,
        &[
            "philosophy-node-edge-v1",
            "canon-node-relation-v1",
            "candidate-relation-v1",
            "source-navigation-node-edge-v1",
            "reified-bibliographic-claims-v1",
            "declared-identity-and-source-ref-joins-v1",
            "repository-topology-v1",
            "indexed-node-edge-v1",
        ],
    )
    .unwrap();
    let root_raw=serde_json::to_vec(&json!({"node_id":"fixture-source-root","node_type":"repository-root","label":"Selected fixture repository","source_ref":"ToS/source_home.manifest.json"})).unwrap();
    let root_sha = Digest256::of_bytes(&root_raw).to_hex();
    let limits = RepositorySourceLimits {
        max_inventory_members: 512,
        max_source_bytes: 2 * 1024 * 1024,
        max_row_bytes: 2 * 1024 * 1024,
        max_plan_bytes: 16 * 1024 * 1024,
        max_work_bytes: 32 * 1024 * 1024,
    };
    let job = "selected-fixture-repository-source";
    let plan = plan_repository_source_inputs(
        &cut,
        revision,
        membership,
        &inventory,
        &capture.selection,
        job,
        &vocabulary,
        RepositoryRootInput {
            source_cut: job,
            material: &root_raw,
            material_sha256: &root_sha,
            identity_id: "root:fixture-source-root",
        },
        &mut schemas,
        limits,
        deadline,
        &cancelled,
    )
    .unwrap();
    let mut wrong_membership = membership;
    wrong_membership.count += 1;
    assert!(
        plan_repository_source_inputs(
            &cut,
            revision,
            wrong_membership,
            &inventory,
            &capture.selection,
            job,
            &vocabulary,
            plan.root_input(),
            &mut schemas,
            limits,
            deadline,
            &cancelled
        )
        .is_err()
    );
    schemas.finish(deadline, &cancelled).unwrap();
    let receipt = ExactInputReceipt {
        binding: SourceBinding {
            owner_profile: "private-selected-fixture".into(),
            source_cut: job.into(),
            through_commit_seq: 0,
            membership_root: membership.digest.to_hex(),
            index_generation: revision.0.to_hex(),
            route_map_version: "fixture-v1".into(),
            reader_abi: "fixture-v1".into(),
            projection_root_sha256: Digest256::of_bytes(b"independent-private-target").to_hex(),
            complete: true,
        },
        collections: plan.receipt().collections.clone(),
    };
    let stage_limits = StageLimits {
        sqlite: Limits {
            max_rows: 4096,
            max_row_bytes: 2 * 1024 * 1024,
            max_output_bytes: 64 * 1024 * 1024,
            max_work_bytes: 128 * 1024 * 1024,
            sqlite_cache_kib: 512,
            max_sql_vm_steps: 10_000_000,
        },
        max_temp_bytes: 64 * 1024 * 1024,
        max_seek_rows: 128,
        max_seek_bytes: 16 * 1024 * 1024,
    };
    let owner = FixtureOwner;
    let isolation = FixtureIsolation;
    let mut stage = KnowledgeStage::create(
        &fixture.path().join("repository-candidate.sqlite"),
        stage_limits,
        receipt,
        &owner,
        &isolation,
    )
    .unwrap();
    render_repository_source_plan(&mut stage, &plan, &vocabulary, deadline, &cancelled).unwrap();
    // HistoricalExact evidence retains its original input scope; CurrentDeclared
    // rows were independently frozen before capture/planning, not patched from it.
    let mut historical = crate::frozen_legacy_python_oracle("repository-topology");
    assert_eq!(historical["resources"].as_array().unwrap().len(), 202);
    reconcile_source_home_inputs(&mut historical, &captured);
    let source = &vocabulary
        .sources
        .iter()
        .find(|source| source.adapter_profile == "repository-topology-v1")
        .unwrap()
        .source_graph_id;
    let mut orders = reference_rows.remove("source_order").unwrap();
    for collection in ["branches", "manifests", "resources"] {
        let mut actual = Vec::new();
        let mut after = None;
        loop {
            let page = stage
                .scan_input(source, collection, after.as_deref(), 128)
                .unwrap();
            for row in page.rows {
                actual.push(serde_json::from_slice::<Value>(&row.payload).unwrap());
            }
            after = page.next_id;
            if after.is_none() {
                break;
            }
        }
        let mut wanted = reference_rows.remove(collection).unwrap();
        // Raw roots sort by identity, independently from owner array ordinal.
        wanted.sort_by_key(|v| {
            v.get("id")
                .or_else(|| v.get("path"))
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned()
        });
        assert_eq!(
            actual, wanted,
            "whole selected maintained {collection} rows"
        );
    }
    let mut actual_orders = Vec::new();
    let mut after = None;
    loop {
        let page = stage
            .scan_input(source, "source_order", after.as_deref(), 128)
            .unwrap();
        for row in page.rows {
            actual_orders.push(serde_json::from_slice::<Value>(&row.payload).unwrap());
        }
        after = page.next_id;
        if after.is_none() {
            break;
        }
    }
    orders.sort_by_key(|v| {
        format!(
            "{}:{}",
            v["collection"].as_str().unwrap(),
            v["id"].as_str().unwrap()
        )
    });
    assert_eq!(actual_orders, orders, "exact dense original owner ordering");
    let prepared = prepare_repository_topology(
        &mut stage,
        &vocabulary,
        plan.root_input(),
        TopologyLimits {
            max_rows: 4096,
            max_page_rows: 32,
            max_row_bytes: 2 * 1024 * 1024,
            max_work_bytes: 64 * 1024 * 1024,
        },
    )
    .unwrap();
    let rows = plan
        .receipt()
        .collections
        .iter()
        .filter(|r| r.collection != "source_order")
        .map(|r| r.expected_count)
        .sum::<u64>();
    assert_eq!(prepared.nodes, rows + 1);
    assert!(prepared.relations > 0);
}

#[derive(Default)]
struct CatalogOutput {
    files: BTreeMap<String, Vec<u8>>,
    current: Option<String>,
    manifest: Value,
}
impl tos_compiler::source_witness_catalog::SourceCatalogSink for CatalogOutput {
    fn begin_file(&mut self, source: &str) -> tos_compiler::Result<()> {
        assert!(self.current.is_none());
        assert!(self.files.insert(source.into(), Vec::new()).is_none());
        self.current = Some(source.into());
        Ok(())
    }
    fn file_bytes(&mut self, raw: &[u8]) -> tos_compiler::Result<()> {
        let output = self.files.get_mut(self.current.as_ref().unwrap()).unwrap();
        assert!(output.len() + raw.len() <= 2 * 1024 * 1024);
        output.extend_from_slice(raw);
        Ok(())
    }
    fn end_file(&mut self, source: &str, sha256: &str) -> tos_compiler::Result<()> {
        assert_eq!(self.current.take().as_deref(), Some(source));
        assert_eq!(Digest256::of_bytes(&self.files[source]).to_hex(), sha256);
        Ok(())
    }
    fn addressed_row(&mut self, _: &str, _: &[u8]) -> tos_compiler::Result<()> {
        Ok(())
    }
    fn manifest(&mut self, value: &Value) -> tos_compiler::Result<()> {
        self.manifest = value.clone();
        Ok(())
    }
}
#[derive(Default)]
struct BibliographicOutput(BTreeMap<String, Vec<Value>>);
impl tos_compiler::source_bibliographic::BibliographicSink for BibliographicOutput {
    fn row(&mut self, collection: &str, _: &str, raw: &[u8]) -> tos_compiler::Result<()> {
        assert!(raw.len() <= 1024 * 1024);
        let rows = self.0.entry(collection.into()).or_default();
        assert!(rows.len() < 1024);
        rows.push(serde_json::from_slice(raw).unwrap());
        Ok(())
    }
}

fn fixture_catalog_inputs(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn add_file(
        root: &Path,
        path: &Path,
        files: &mut BTreeMap<String, Vec<u8>>,
        total: &mut usize,
    ) {
        let metadata = fs::symlink_metadata(path).unwrap();
        assert!(metadata.is_file() && !metadata.file_type().is_symlink());
        assert!(
            metadata.len() <= 2 * 1024 * 1024,
            "catalog fixture member cap: {}",
            path.display()
        );
        let relative = path
            .strip_prefix(root)
            .unwrap()
            .to_str()
            .unwrap()
            .replace('\\', "/");
        let raw = fs::read(path).unwrap();
        *total = total.checked_add(raw.len()).unwrap();
        assert!(files.len() < 512 && *total <= 16 * 1024 * 1024);
        assert!(
            files.insert(relative, raw).is_none(),
            "duplicate catalog fixture path"
        );
    }

    fn visit(
        root: &Path,
        directory: &Path,
        schemas_only: bool,
        files: &mut BTreeMap<String, Vec<u8>>,
        total: &mut usize,
    ) {
        let mut entries = fs::read_dir(directory)
            .unwrap()
            .map(|entry| entry.unwrap())
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            let relative = path.strip_prefix(root).unwrap();
            if relative == Path::new("ToS/source-witnesses/catalog")
                || relative == Path::new("ToS/source-witnesses/payload")
                || relative.starts_with("ToS/source-witnesses/catalog")
                || relative.starts_with("ToS/source-witnesses/payload")
            {
                continue;
            }
            let metadata = fs::symlink_metadata(&path).unwrap();
            assert!(
                !metadata.file_type().is_symlink(),
                "catalog fixture symlink: {}",
                path.display()
            );
            if metadata.is_dir() {
                visit(root, &path, schemas_only, files, total);
            } else if metadata.is_file() {
                if !schemas_only
                    || path
                        .file_name()
                        .is_some_and(|name| name.to_string_lossy().ends_with(".schema.json"))
                {
                    add_file(root, &path, files, total);
                }
            } else {
                panic!("unsupported catalog fixture member: {}", path.display());
            }
        }
    }

    let owner_local = root.join("ToS/source-witnesses/owner-local");
    assert!(
        matches!(
            fs::symlink_metadata(&owner_local),
            Err(ref error) if error.kind() == std::io::ErrorKind::NotFound
        ),
        "public catalog fixture contains reserved owner-local data"
    );
    let mut files = BTreeMap::new();
    let mut total = 0usize;
    visit(
        root,
        &root.join("ToS/contracts"),
        true,
        &mut files,
        &mut total,
    );
    for path in [
        "ToS/doctrine/semantic-interchange/entity-types.v1.json",
        "ToS/doctrine/semantic-interchange/relation-types.v1.json",
        // Synthetic identity endpoints cite this exact isolated evidence file.
        "ToS/research-packets/foundation-laboratory-2026-07/JENSEITS_1886_LETTER_705_SOURCE_READING_V1.md",
    ] {
        let selected = root.join(path);
        if selected.is_file() {
            add_file(root, &selected, &mut files, &mut total);
        }
    }
    // The authored source tree is the compiler input. Generated catalog
    // projections, reserved owner-local data and physical payload bytes are
    // outside this producer's read contract.
    visit(
        root,
        &root.join("ToS/source-witnesses"),
        false,
        &mut files,
        &mut total,
    );
    assert!(files.contains_key("ToS/contracts/corpus-record.schema.json"));
    assert!(files.contains_key("ToS/doctrine/semantic-interchange/entity-types.v1.json"));
    files
}

fn catalog_manifest_bytes(manifest: &Value) -> Vec<u8> {
    let mut ordered = serde_json::Map::new();
    for key in [
        "schema_version",
        "owner_repo",
        "source_root",
        "generated_by",
        "record_schema_ref",
        "claim_schema_ref",
        "extension_schema_refs",
        "record_files",
        "claim_file",
        "counts",
        "catalog_sha256",
        "selected_metadata_publication",
        "authority_boundary",
    ] {
        if let Some(value) = manifest.get(key) {
            ordered.insert(key.to_owned(), value.clone());
        }
    }
    assert_eq!(ordered.len(), manifest.as_object().unwrap().len());
    let mut raw = serde_json::to_vec_pretty(&Value::Object(ordered)).unwrap();
    raw.push(b'\n');
    raw
}

pub(crate) struct NativeCatalogFixture {
    pub(crate) files: BTreeMap<String, Vec<u8>>,
    pub(crate) manifest: Value,
    pub(crate) graph: BTreeMap<String, Vec<Value>>,
    pub(crate) collection_count: usize,
    pub(crate) record_count: u64,
    pub(crate) node_count: u64,
    pub(crate) edge_count: u64,
}

pub(crate) fn native_catalog_fixture(
    repository: &Path,
    files: &BTreeMap<String, Vec<u8>>,
    deadline: Instant,
) -> NativeCatalogFixture {
    native_catalog_fixture_with_graph(repository, files, deadline, true)
}

fn native_catalog_fixture_with_graph(
    repository: &Path,
    files: &BTreeMap<String, Vec<u8>>,
    deadline: Instant,
    include_bibliographic_graph: bool,
) -> NativeCatalogFixture {
    use tos_compiler::source_bibliographic::{BibliographicLimits, render_bibliographic_graph};
    use tos_compiler::source_witness_catalog::{
        SourceCatalogLimits, SourceCatalogValidator, render_source_witness_catalog,
    };
    use tos_compiler::{
        SourceCatalogInputLimits, plan_source_catalog_inputs, prepare_source_catalog_plan,
        render_source_bibliographic_plan,
    };

    assert!(repository.is_absolute());
    assert!(
        repository
            .join("rust/crates/tos-compiler/src/source_witness_catalog.rs")
            .is_file()
    );
    assert!(files.len() <= 512);
    assert!(files.values().map(Vec::len).sum::<usize>() <= 16 * 1024 * 1024);
    let fixture = tempfile::tempdir().unwrap();
    let store = fixture.path().join("source-store");
    let revision = super::validation_cut_cases::write_cut_store(files, &store);
    let cancelled = AtomicBool::new(false);
    let read_limits = ReadLimits {
        max_manifest_bytes: 2 * 1024 * 1024,
        max_manifest_entries: 512,
        max_selected_object_bytes: 2 * 1024 * 1024,
        json: JsonLimits::default(),
    };
    let reader = CorpusReader::open_existing(&store, read_limits).unwrap();
    let cut = reader
        .open_source_cut(
            revision,
            tos_source_store::CutReadLimits {
                max_revisions: 1,
                max_members: 512,
                max_total_bytes: 16 * 1024 * 1024,
                max_member_bytes: 2 * 1024 * 1024,
            },
            deadline,
            &cancelled,
        )
        .unwrap();
    let membership = cut.stream(revision).unwrap().expectation();
    let binding = SourceBinding {
        owner_profile: "private-selected-fixture".into(),
        source_cut: "selected-fixture-bibliographic-source".into(),
        through_commit_seq: 0,
        membership_root: membership.digest.to_hex(),
        index_generation: revision.0.to_hex(),
        route_map_version: "fixture-v1".into(),
        reader_abi: "fixture-v1".into(),
        projection_root_sha256: Digest256::of_bytes(b"independent-catalog-plan").to_hex(),
        complete: true,
    };
    let limits = BibliographicLimits {
        catalog: SourceCatalogLimits {
            max_files: 512,
            max_rows: 4096,
            max_file_bytes: 2 * 1024 * 1024,
            max_row_bytes: 1024 * 1024,
            max_contract_bytes: 16 * 1024 * 1024,
            max_output_row_bytes: 1024 * 1024,
        },
        max_claim_cohort_rows: 16,
        max_claim_cohort_bytes: 16 * 1024 * 1024,
        max_output_rows: 4096,
        max_output_bytes: 16 * 1024 * 1024,
        deadline,
    };
    let plan = plan_source_catalog_inputs(
        &cut,
        revision,
        membership,
        &binding,
        SourceCatalogInputLimits {
            max_manifest_members: 512,
            max_selected_members: 512,
            max_plan_bytes: 16 * 1024 * 1024,
            max_work_bytes: 64 * 1024 * 1024,
        },
        limits,
        &cancelled,
    )
    .unwrap();
    let collection_count = plan.input_receipt().collections.len();
    let mut receipt = plan.input_receipt();
    receipt.binding.projection_root_sha256 =
        Digest256::of_bytes(b"independent-catalog-target").to_hex();
    let owner = FixtureOwner;
    let isolation = FixtureIsolation;
    let mut stage = KnowledgeStage::create(
        &fixture.path().join("bibliographic-candidate.sqlite"),
        StageLimits {
            sqlite: Limits {
                max_rows: 8192,
                max_row_bytes: 2 * 1024 * 1024,
                max_output_bytes: 64 * 1024 * 1024,
                max_work_bytes: 128 * 1024 * 1024,
                sqlite_cache_kib: 512,
                max_sql_vm_steps: 20_000_000,
            },
            max_temp_bytes: 64 * 1024 * 1024,
            max_seek_rows: 128,
            max_seek_bytes: 16 * 1024 * 1024,
        },
        receipt,
        &owner,
        &isolation,
    )
    .unwrap();
    let worker_image =
        super::command_form_cases::schema_image(ExecutorBudget::laboratory(), deadline, &cancelled);
    // One explicit finite envelope for this complete catalog/schema operation.
    // Limits derive from the declared output/receipt budget and common deadline.
    let mut operation = tos_validation::executor::BatchStreamBudget::laboratory();
    operation.max_chunks = limits.max_output_rows;
    operation.max_total_units = limits.max_output_rows;
    operation.total_execution_wall = deadline.saturating_duration_since(Instant::now());
    operation.operation_cpu_seconds = operation.total_execution_wall.as_secs().saturating_add(1);
    operation.operation_address_space_bytes = ExecutorBudget::laboratory().address_space_bytes;
    let validator = SourceCatalogValidator::from_cut_with_image(
        &cut,
        &worker_image,
        ExecutorBudget::laboratory(),
        tos_validation::source_cut::CutWorkerLimits {
            max_receipts: usize::try_from(limits.max_output_rows).unwrap(),
            max_receipt_bytes: usize::try_from(limits.max_output_bytes).unwrap(),
        },
        operation,
        deadline,
        &cancelled,
    )
    .unwrap();
    let (catalog_receipt, graph, node_count, edge_count) = if include_bibliographic_graph {
        let candidate = render_source_bibliographic_plan(
            &plan,
            &cut,
            revision,
            membership,
            &mut stage,
            &validator,
            &mut tos_command::source_forms_compiler::NativeBibliographicForms,
            limits,
            512,
            16 * 1024 * 1024,
        )
        .unwrap();
        validator.finish().unwrap();
        let mut graph = BibliographicOutput::default();
        render_bibliographic_graph(
            &mut stage,
            &candidate.catalog,
            &candidate.bibliographic,
            limits,
            &mut graph,
        )
        .unwrap();
        (
            candidate.catalog,
            graph.0,
            candidate.bibliographic.node_count,
            candidate.bibliographic.edge_count,
        )
    } else {
        // Claim-command fixtures need the native catalog writer. Bibliographic
        // graph closure is independently exercised by the dedicated case.
        let receipt = prepare_source_catalog_plan(
            &plan, &cut, revision, membership, &mut stage, &validator, limits,
        )
        .unwrap();
        validator.finish().unwrap();
        (receipt, BTreeMap::new(), 0, 0)
    };
    let mut catalog = CatalogOutput::default();
    render_source_witness_catalog(&mut stage, &catalog_receipt, limits.catalog, &mut catalog)
        .unwrap();
    NativeCatalogFixture {
        files: catalog.files,
        manifest: catalog.manifest,
        graph,
        collection_count,
        record_count: catalog_receipt.record_count,
        node_count,
        edge_count,
    }
}

pub(crate) fn publish_native_catalog_fixture(repository: &Path, root: &Path, deadline: Instant) {
    publish_native_catalog_fixture_inner(repository, root, deadline, None);
}

/// A selected command fixture keeps its exact offline schema closure rather
/// than importing unrelated contracts into the metadata revision dispatcher.
pub(crate) fn publish_native_catalog_fixture_with_selected_schemas(
    repository: &Path,
    root: &Path,
    deadline: Instant,
    schemas: &BTreeMap<String, Vec<u8>>,
) {
    publish_native_catalog_fixture_inner(repository, root, deadline, Some(schemas));
}

fn publish_native_catalog_fixture_inner(
    repository: &Path,
    root: &Path,
    deadline: Instant,
    selected_schemas: Option<&BTreeMap<String, Vec<u8>>>,
) {
    // The migrated profile registry references the current contract closure.
    // Frozen command fixtures retain their original schemas; add only missing
    // owner schemas before freezing the next selected source revision.
    let schemas = selected_schemas
        .cloned()
        .unwrap_or_else(|| schema_sources(repository));
    for (relative, raw) in schemas {
        let target = root.join(relative);
        if !target.exists() {
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(target, raw).unwrap();
        }
    }
    let inputs = fixture_catalog_inputs(root);
    if let Some(selected) = selected_schemas {
        assert!(!selected.is_empty() && selected.len() <= 128);
        let actual = inputs
            .iter()
            .filter(|(path, _)| {
                path.starts_with("ToS/contracts/") && path.ends_with(".schema.json")
            })
            .map(|(path, raw)| (path.clone(), raw.clone()))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            &actual, selected,
            "catalog fixture exact selected schema closure"
        );
    }
    let publication_path = root.join("ToS/source-witnesses/.metadata-publication.json");
    let publication_raw = match fs::read(&publication_path) {
        Ok(raw) => Some(raw),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => panic!("fixture publication: {error}"),
    };
    let epoch =
        tos_source_store::MetadataPublicationEpoch::select(publication_raw.as_ref().map(|raw| {
            parse_json(raw, JsonMode::PublishedStrict, JsonLimits::default())
                .unwrap()
                .into_root()
        }))
        .unwrap();
    let mut catalog = native_catalog_fixture_with_graph(repository, &inputs, deadline, false);
    if let Some(token) = epoch.token() {
        let file_digests = catalog
            .files
            .iter()
            .map(|(path, raw)| {
                (
                    path.clone(),
                    Value::String(Digest256::of_bytes(raw).to_hex()),
                )
            })
            .collect::<serde_json::Map<_, _>>();
        catalog.manifest["selected_metadata_publication"] = serde_json::json!({
            "protocol":"tos_selected_source_metadata_v1", "token":token, "files":file_digests,
        });
    }
    assert_eq!(fs::read(&publication_path).ok(), publication_raw);
    assert_eq!(
        catalog.manifest["schema_version"],
        "tos_source_witness_catalog_v3"
    );
    assert_eq!(
        catalog.manifest["generated_by"],
        "tos-native-owner-command source-catalog build"
    );
    assert!(
        catalog
            .files
            .contains_key("ToS/source-witnesses/catalog/claims.jsonl")
    );
    for (relative, raw) in catalog.files {
        let target = root.join(&relative);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, raw).unwrap();
    }
    let target = root.join("ToS/source-witnesses/catalog/catalog.manifest.json");
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(target, catalog_manifest_bytes(&catalog.manifest)).unwrap();
}

#[test]
fn actual_selected_catalog_and_native_forms_match_maintained_python() {
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let mut files = schema_sources(&repository);
    // The declared source carrier contains schemas and two genuine maintained
    // fixture records; repository branch inputs are outside this family scope.
    for name in ["entity-types.v1.json", "relation-types.v1.json"] {
        let path = format!("ToS/doctrine/semantic-interchange/{name}");
        files.insert(path.clone(), fs::read(repository.join(path)).unwrap());
    }
    let fixture_root = repository.join("access/tests/fixtures/knowledge-contract");
    let record = "ToS/source-witnesses/semantic-descriptions/crosscutting-concept-freedom/crosscutting-concept.json";
    let forms = "ToS/source-witnesses/semantic-descriptions/crosscutting-concept-freedom/crosscutting-concept.human-forms.json";
    for path in [record, forms] {
        files.insert(path.into(), fs::read(fixture_root.join(path)).unwrap());
    }
    assert!(files.len() <= 512);
    assert!(files.values().map(Vec::len).sum::<usize>() <= 16 * 1024 * 1024);
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut native = native_catalog_fixture(&repository, &files, deadline);
    let mut expected = crate::frozen_legacy_python_oracle("catalog-bibliographic");
    assert_eq!(
        expected["manifest"]["generated_by"], "scripts/build_source_witness_catalog.py",
        "the captured legacy producer identity remains explicit"
    );
    expected["manifest"]["generated_by"] =
        serde_json::Value::String("tos-native-owner-command source-catalog build".into());
    let actual_files: BTreeMap<String, String> = native
        .files
        .into_iter()
        .map(|(path, raw)| (path, String::from_utf8(raw).unwrap()))
        .collect();
    assert_eq!(
        serde_json::to_value(actual_files).unwrap(),
        expected["files"]
    );
    assert_eq!(native.manifest, expected["manifest"]);
    for collection in ["nodes", "edges", "claim_traces"] {
        let rows = native.graph.remove(collection).unwrap_or_default();
        assert_eq!(
            serde_json::to_value(rows).unwrap(),
            expected["graph"][collection]
        );
    }
    assert_eq!(native.collection_count, 5);
    assert_eq!(native.record_count, 1);
    assert_eq!(native.node_count, 1);
    assert_eq!(native.edge_count, 0);
}

/// One finite maintained authored recipe exercises real canon/candidate rows
/// and Claim/endpoint retained history. Capture and source-cut identities stay
/// distinct; neither the owner stubs nor this oracle issue source admission.
#[test]
fn actual_native_corpus_composition_with_retained_claim_matches_maintained_python() {
    native_corpus_composition_case(false);
}

/// Required installed-native producer and consumer stage of the same authored
/// recipe. OPS admits this Linux kernel/product boundary independently from
/// pure selected reads.
#[test]
#[ignore = "requires admitted Linux private-stage ticket, fs-verity, and native owner/consumer binaries"]
fn actual_native_corpus_managed_installed_consumer() {
    if !native_managed_corpus_consumer::resume_selected_phase() {
        native_corpus_composition_case(true);
    }
}

fn corpus_full_limits(source_count: usize) -> tos_compiler::FullKnowledgeLimits {
    tos_compiler::FullKnowledgeLimits {
        scope: tos_compiler::ScopeLimits {
            max_sources: source_count,
            max_rows: 8192,
            max_index_work_bytes: 8 * 1024 * 1024,
        },
        catalog: tos_compiler::catalog::CatalogLimits::default(),
        catalog_index: tos_compiler::CatalogIndexLimits::default(),
        search: tos_compiler::SearchBuildLimits {
            max_payload_bytes: 1024 * 1024,
            max_document_chars: 1024 * 1024,
            max_document_bytes: 4 * 1024 * 1024,
            max_rank_field_bytes: 1024 * 1024,
            max_postings: 10_000_000,
            max_work_bytes: 2 * 1024 * 1024 * 1024,
            gram_batch_rows: 64,
        },
        seal: tos_compiler::SealLimits {
            max_header_bytes: 1024 * 1024,
        },
        max_registry_bytes: 4 * 1024 * 1024,
    }
}

/// Full native producer at an ALREADY selected S. No new cut, subset,
/// caller header, managed fallback or synthetic Original is constructed here.
/// Fixture stage authority is explicitly synthetic; producer/source fixity is real.
pub(crate) fn native_selected_model_at_existing_cut(
    cut: &tos_source_store::CorpusCutReader,
    revision: tos_foundation::SourceRevision,
    software: &tos_source_store::SoftwareCaptureReader,
    selected_path: std::path::PathBuf,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> tos_compiler::knowledge_full_fixture::FullKnowledgeFixture {
    use tos_compiler::knowledge_canon_source::*;
    use tos_compiler::knowledge_stage::{InputCollectionReceipt, InputRow};
    use tos_compiler::source_bibliographic::{BibliographicLimits, BibliographicSourceCut};
    use tos_compiler::source_corpus::*;
    use tos_compiler::source_navigation_source::project_source_navigation_from_cut;
    use tos_compiler::source_witness_catalog::{SourceCatalogLimits, SourceCatalogValidator};
    use tos_compiler::{
        SourceCatalogInputLimits, plan_source_catalog_inputs, render_source_bibliographic_plan,
    };
    fn raw_receipt(
        source: &str,
        collection: &str,
        role: &str,
        profile: &str,
        rows: &BTreeMap<String, Vec<u8>>,
    ) -> InputCollectionReceipt {
        let mut hash = tos_foundation::Digest256Hasher::new();
        for (id, raw) in rows {
            hash.update(&(id.len() as u64).to_be_bytes());
            hash.update(id.as_bytes());
            hash.update(Digest256::of_bytes(raw).as_bytes());
        }
        InputCollectionReceipt {
            source_graph: source.into(),
            collection: collection.into(),
            input_role: role.into(),
            adapter_profile: profile.into(),
            expected_count: rows.len() as u64,
            expected_root_sha256: hash.finalize().to_hex(),
        }
    }
    let started = Instant::now();
    assert!(started < deadline);
    let fixture = tempfile::tempdir().unwrap();
    let mut files = BTreeMap::new();
    let mut stream = cut.stream(revision).unwrap();
    let expected = stream.expectation();
    while let Some(member) = stream.next_member(deadline, cancelled).unwrap() {
        assert_eq!(member.revision, revision);
        assert!(files.len() < 512 && member.raw.len() <= 2 * 1024 * 1024);
        assert!(
            files
                .insert(member.path.as_str().to_owned(), member.raw)
                .is_none()
        );
    }
    assert_eq!(stream.coverage(), Some(expected));
    assert!(files.values().map(Vec::len).sum::<usize>() <= 16 * 1024 * 1024);
    // Authoritative repository planner checks inventory too; preflight the
    // complete S membership against actual captured metadata without relabeling.
    for (path, raw) in &files {
        let metadata = software
            .member(&tos_foundation::RelativePath::parse(path).unwrap())
            .expect("full S member absent from actual software inventory");
        assert_eq!(metadata.sha256, Digest256::of_bytes(raw));
        assert_eq!(metadata.size_bytes, raw.len() as u64);
        // capture_git maps regular Git tree mode 100644 to permission bits
        // 0644; SoftwareCaptureReader validates that declared representation.
        assert_eq!(metadata.mode, 0o644);
    }
    // Retain actual verified membership in the existing command log, without
    // raw contents. Admit a conservative JSON-escape bound before allocation.
    let manifest_limit = 128 * 1024;
    let manifest_bound = files
        .keys()
        .try_fold(1024usize, |used, path| {
            path.len()
                .checked_mul(6)
                .and_then(|n| n.checked_add(256))
                .and_then(|n| used.checked_add(n))
                .filter(|n| *n <= manifest_limit)
        })
        .expect("actual S manifest evidence budget");
    let manifest_rows = files.iter().map(|(path, raw)| {
        let metadata = software.member(&tos_foundation::RelativePath::parse(path).unwrap()).unwrap();
        json!({"path":path,"size_bytes":raw.len(),"sha256":Digest256::of_bytes(raw).to_prefixed(),
            "mode":metadata.mode})
    }).collect::<Vec<_>>();
    let manifest = json!({"event":"tos_actual_frozen_source_manifest_v1",
        "source_revision":revision.0.to_prefixed(),"mode_representation":"permission_bits",
        "membership":{"count":expected.count,"sha256":expected.digest.to_prefixed()},
        "members":manifest_rows});
    let manifest_raw = serde_json::to_vec(&manifest).unwrap();
    assert!(manifest_raw.len().checked_add(1).unwrap() <= manifest_bound);
    writeln!(
        std::io::stderr().lock(),
        "{}",
        std::str::from_utf8(&manifest_raw).unwrap()
    )
    .unwrap();
    // Fixed direct reads are admitted against actual S BEFORE any producer.
    // Registry-derived profile routes retain their existing owner verifier.
    for required in [
        "ToS/source_home.manifest.json",
        "ToS/doctrine/semantic-interchange/entity-types.v1.json",
        "ToS/doctrine/semantic-interchange/relation-types.v1.json",
        "ToS/doctrine/semantic-interchange/query-vocabulary.v1.json",
        "ToS/contracts/semantic-entity-type-registry.schema.json",
        "ToS/contracts/semantic-relation-type-registry.schema.json",
        "ToS/contracts/corpus-record.schema.json",
        "ToS/contracts/claim-packet.schema.json",
        "ToS/contracts/source-witness-bibliographic-graph.schema.json",
        "ToS/contracts/tos-corpus-index.schema.json",
        "ToS/contracts/tos-source-home.schema.json",
        "ToS/contracts/tos-node-contract.schema.json",
        "ToS/contracts/human-form.schema.json",
        "ToS/contracts/human-form-set.schema.json",
        "ToS/contracts/human-form-template.schema.json",
        "ToS/contracts/source-claim-record.schema.json",
        "ToS/contracts/historical-claim.schema.json",
        "ToS/contracts/source-temporal-claim.schema.json",
        "ToS/contracts/claim-display-fields.schema.json",
        "ToS/contracts/provenance-event-v2.schema.json",
    ] {
        assert!(
            files.contains_key(required),
            "fixed producer input absent from actual S: {required}"
        );
    }
    for required in [
        "rust/crates/tos-compiler/src/source_corpus.rs",
        "access/contracts/runtime-data.v1.json",
    ] {
        assert!(
            software
                .member(&tos_foundation::RelativePath::parse(required).unwrap())
                .is_some(),
            "fixed producer software input absent: {required}"
        );
    }
    // Descriptor bytes belong to the SAME authenticated S, not an ambient
    // test file; the actual normalizer/header pin this exact descriptor.
    let descriptor = files["ToS/doctrine/semantic-interchange/query-vocabulary.v1.json"].clone();
    let vocabulary =
        QueryVocabulary::parse(&descriptor, tos_compiler::NATIVE_KNOWLEDGE_ADAPTER_PROFILES)
            .unwrap();
    let full_limits = corpus_full_limits(vocabulary.sources.len());
    let runtime_path =
        tos_foundation::RelativePath::parse("access/contracts/runtime-data.v1.json").unwrap();
    let runtime_components = software
        .select_components(std::slice::from_ref(&runtime_path))
        .unwrap();
    let runtime_raw = software
        .read_selected_component(
            &runtime_components,
            &runtime_path,
            1_048_576,
            deadline,
            cancelled,
        )
        .unwrap();
    let runtime: Value = serde_json::from_slice(&runtime_raw).unwrap();
    let outputs = runtime["subjects"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["subject_id"] == "tos-corpus-index")
        .collect::<Vec<_>>();
    assert_eq!(outputs.len(), 1);
    let output_path = outputs[0]["source_path"].as_str().unwrap().to_owned();
    let membership = cut.stream(revision).unwrap().expectation();
    let binding = SourceBinding {
        owner_profile: "private-selected-fixture".into(),
        source_cut: revision.0.to_hex(),
        through_commit_seq: 0,
        membership_root: membership.digest.to_hex(),
        index_generation: revision.0.to_hex(),
        route_map_version: "fixture-v1".into(),
        reader_abi: "fixture-v1".into(),
        projection_root_sha256: Digest256::of_bytes(b"independent-native-corpus-plan").to_hex(),
        complete: true,
    };
    let limits = BibliographicLimits {
        catalog: SourceCatalogLimits {
            max_files: 512,
            max_rows: 4096,
            max_file_bytes: 2 * 1024 * 1024,
            max_row_bytes: 1024 * 1024,
            max_contract_bytes: 16 * 1024 * 1024,
            max_output_row_bytes: 1024 * 1024,
        },
        max_claim_cohort_rows: 16,
        max_claim_cohort_bytes: 16 * 1024 * 1024,
        max_output_rows: 4096,
        max_output_bytes: 16 * 1024 * 1024,
        deadline,
    };
    let catalog_plan = plan_source_catalog_inputs(
        &cut,
        revision,
        membership,
        &binding,
        SourceCatalogInputLimits {
            max_manifest_members: 512,
            max_selected_members: 512,
            max_plan_bytes: 16 * 1024 * 1024,
            max_work_bytes: 64 * 1024 * 1024,
        },
        limits,
        &cancelled,
    )
    .unwrap();
    let owner = FixtureOwner;
    let isolation = FixtureIsolation;
    let stage_limits = StageLimits {
        sqlite: Limits {
            max_rows: 8192,
            max_row_bytes: 2 * 1024 * 1024,
            max_output_bytes: 64 * 1024 * 1024,
            max_work_bytes: 128 * 1024 * 1024,
            sqlite_cache_kib: 512,
            max_sql_vm_steps: 20_000_000,
        },
        max_temp_bytes: 64 * 1024 * 1024,
        max_seek_rows: 16,
        max_seek_bytes: 32 * 1024 * 1024,
    };
    let mut catalog_stage = KnowledgeStage::create(
        &fixture.path().join("catalog.sqlite"),
        stage_limits,
        catalog_plan.input_receipt(),
        &owner,
        &isolation,
    )
    .unwrap();
    let mut operation = tos_validation::executor::BatchStreamBudget::laboratory();
    operation.max_chunks = limits.max_output_rows;
    operation.max_total_units = limits.max_output_rows;
    let worker_path = super::validation_cut_cases::selected_worker_path();
    let worker = ExactWorkerIdentity {
        sha256: Digest256::of_bytes(&fs::read(&worker_path).unwrap()),
        absolute_path: worker_path,
    };
    // Exact schema closure and image preparation belong to this whole case.
    // The scalar laboratory's five-second default may expire during that
    // preparation; only time left on the original whole clock is available.
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .filter(|duration| !duration.is_zero())
        .expect("native corpus whole schema preparation deadline");
    let schema_budget = ExecutorBudget {
        execution_wall: remaining,
        ..ExecutorBudget::laboratory()
    };
    operation.total_execution_wall = remaining;
    operation.operation_cpu_seconds = remaining.as_secs().saturating_add(1);
    operation.operation_address_space_bytes = schema_budget.address_space_bytes;
    let validator = SourceCatalogValidator::from_cut(
        &cut,
        &worker,
        schema_budget,
        CutWorkerLimits {
            max_receipts: 4096,
            max_receipt_bytes: 16 * 1024 * 1024,
        },
        operation,
        deadline,
        &cancelled,
    )
    .unwrap();
    let mut forms = tos_command::source_forms_compiler::NativeBibliographicForms;
    let candidate = render_source_bibliographic_plan(
        &catalog_plan,
        &cut,
        revision,
        membership,
        &mut catalog_stage,
        &validator,
        &mut forms,
        limits,
        512,
        16 * 1024 * 1024,
    )
    .unwrap();
    assert!(candidate.catalog.record_count > 1);
    assert!(candidate.bibliographic.edge_count > 0);
    let canon_files = files
        .iter()
        .filter(|(p, _)| {
            (p.starts_with("ToS/canon/")
                && (p.ends_with("/node.json") || p.ends_with("/node.human-forms.json")))
                || ((p.starts_with("ToS/canon/") || p.starts_with("ToS/candidate-intake/"))
                    && p.ends_with("/edges.csv"))
        })
        .map(|(p, b)| (p.clone(), b.clone()))
        .collect::<BTreeMap<_, _>>();
    assert!(
        canon_files.is_empty(),
        "this bounded recipe declares explicit zero canon input"
    );
    let contracts = [
        "ToS/contracts/tos-node-contract.schema.json",
        "ToS/contracts/human-form-set.schema.json",
    ]
    .into_iter()
    .map(|p| (p.into(), files[p].clone()))
    .collect::<BTreeMap<_, _>>();
    let canon_inputs = vec![
        raw_receipt(
            CANON_SOURCE_CUSTODY,
            "source-files",
            CANON_SOURCE_FILES_ROLE,
            CANON_SOURCE_FILES_PROFILE,
            &canon_files,
        ),
        raw_receipt(
            CANON_SOURCE_CUSTODY,
            "contracts",
            CANON_SOURCE_CONTRACTS_ROLE,
            CANON_SOURCE_CONTRACTS_PROFILE,
            &contracts,
        ),
    ];
    let mut canon_planner = KnowledgeStage::create(
        &fixture.path().join("canon.sqlite"),
        stage_limits,
        ExactInputReceipt {
            binding: binding.clone(),
            collections: canon_inputs,
        },
        &owner,
        &isolation,
    )
    .unwrap();
    for (collection, rows) in [("source-files", &canon_files), ("contracts", &contracts)] {
        let mut entries = rows.iter();
        loop {
            let borrowed = entries
                .by_ref()
                .take(stage_limits.max_seek_rows)
                .map(|(id, raw)| InputRow {
                    source_graph: CANON_SOURCE_CUSTODY,
                    collection,
                    id,
                    payload: raw,
                })
                .collect::<Vec<_>>();
            if borrowed.is_empty() {
                break;
            }
            canon_planner.ingest_input_batch(&borrowed).unwrap();
        }
    }
    let canon_limits = CanonSourceLimits {
        max_manifest_members: 512,
        max_selected_members: 512,
        max_nodes: 128,
        max_packs: 16,
        max_edges: 1024,
        max_source_bytes: 2 * 1024 * 1024,
        max_raw_row_bytes: 2 * 1024 * 1024,
        max_csv_fields: 128,
        max_csv_record_bytes: 2 * 1024 * 1024,
        max_forms: 256,
        max_forms_output_bytes: 262144,
        max_page_rows: 8,
        max_page_bytes: 16 * 1024 * 1024,
        max_work_bytes: 64 * 1024 * 1024,
    };
    let root_raw = serde_json::to_vec(
        &json!({"node_id":"fixture-source-root","node_type":"repository-root",
        "label":"Selected fixture repository","source_ref":"ToS/source_home.manifest.json"}),
    )
    .unwrap();
    let root_sha = Digest256::of_bytes(&root_raw).to_hex();
    let (repository_plan, canon_plan) = plan_native_corpus_source_families(
        &mut canon_planner,
        &cut,
        &software,
        &binding,
        &vocabulary,
        RepositoryRootInput {
            source_cut: &binding.source_cut,
            material: &root_raw,
            material_sha256: &root_sha,
            identity_id: "root:fixture-source-root",
        },
        &validator,
        RepositorySourceLimits {
            max_inventory_members: 512,
            max_source_bytes: 2 * 1024 * 1024,
            max_row_bytes: 2 * 1024 * 1024,
            max_plan_bytes: 16 * 1024 * 1024,
            max_work_bytes: 32 * 1024 * 1024,
        },
        canon_limits,
        deadline,
        &cancelled,
        tos_command::source_forms_compiler::materialize_compiler_forms,
    )
    .unwrap();
    assert_eq!(canon_plan.receipt().nodes, 0);
    assert_eq!(canon_plan.receipt().packs, 0);
    assert_eq!(canon_plan.receipt().edges, 0);
    let entities: Value =
        serde_json::from_slice(&files["ToS/doctrine/semantic-interchange/entity-types.v1.json"])
            .unwrap();
    let source = BibliographicSourceCut {
        cut: &cut,
        expected_revision: revision,
        expected_membership: membership,
        stage_source_cut: &binding.source_cut,
        max_read_files: 512,
        max_read_bytes: 16 * 1024 * 1024,
    };
    let navigation = project_source_navigation_from_cut(
        &mut catalog_stage,
        &candidate.catalog,
        &source,
        &validator,
        &entities,
        &mut forms,
        limits,
    )
    .unwrap();
    assert!(!navigation.value()["edges"].as_array().unwrap().is_empty());
    let mut collections = repository_plan.receipt().collections.clone();
    collections.extend(
        canon_plan
            .receipt()
            .collections
            .iter()
            .map(|c| InputCollectionReceipt {
                source_graph: c.source_graph.clone(),
                collection: c.collection.clone(),
                input_role: c.input_role.clone(),
                adapter_profile: c.adapter_profile.clone(),
                expected_count: c.count,
                expected_root_sha256: c.root_sha256.clone(),
            }),
    );
    collections.extend(catalog_plan.input_receipt().collections);
    let mut target = KnowledgeStage::create(
        &fixture.path().join("corpus.sqlite"),
        stage_limits,
        ExactInputReceipt {
            binding: binding.clone(),
            collections,
        },
        &owner,
        &isolation,
    )
    .unwrap();
    render_repository_source_plan(
        &mut target,
        &repository_plan,
        &vocabulary,
        deadline,
        &cancelled,
    )
    .unwrap();
    render_canon_source_plan(
        &mut canon_planner,
        &canon_plan,
        &cut,
        revision,
        membership,
        &mut target,
        canon_limits,
        deadline,
        &cancelled,
    )
    .unwrap();
    for input in &catalog_plan.input_receipt().collections {
        let mut after = None;
        loop {
            let page = catalog_stage
                .scan_input(
                    &input.source_graph,
                    &input.collection,
                    after.as_deref(),
                    stage_limits.max_seek_rows,
                )
                .unwrap();
            if !page.rows.is_empty() {
                let borrowed = page
                    .rows
                    .iter()
                    .map(|row| InputRow {
                        source_graph: &input.source_graph,
                        collection: &input.collection,
                        id: &row.id,
                        payload: &row.payload,
                    })
                    .collect::<Vec<_>>();
                target.ingest_input_batch(&borrowed).unwrap();
            }
            after = page.next_id;
            if after.is_none() {
                break;
            }
        }
    }
    let originals = tos_compiler::NavigationOriginalLimits {
        max_rows: 4096,
        max_row_bytes: limits.catalog.max_output_row_bytes,
        max_total_bytes: 16 * 1024 * 1024,
    };
    let projection = project_native_corpus_from_sources(
        &mut target,
        &vocabulary,
        &cut,
        &software,
        &repository_plan,
        &canon_plan,
        &navigation,
        &validator,
        NativeCorpusLimits {
            originals,
            max_work_bytes: 128 * 1024 * 1024,
            schema_work: operation,
        },
        deadline,
        &cancelled,
    )
    .unwrap();
    // The projection already finished the one shared schema operation. Close
    // its sealed worker image before the independent oracle and selected DB.
    drop(validator);
    let mut graph = BibliographicOutput::default();
    tos_compiler::source_bibliographic::render_bibliographic_graph(
        &mut catalog_stage,
        &candidate.catalog,
        &candidate.bibliographic,
        limits,
        &mut graph,
    )
    .unwrap();
    // These private planners have completed their producer/proof work. The
    // independent selected stage consumes owned receipts/rows, not their DBs.
    // KnowledgeStage::Drop owns candidate/sidecar/lease deletion for these
    // unfinished private stages. Release them before the final allocation;
    // verify removal without attempting a second filesystem deletion.
    drop(catalog_stage);
    drop(canon_planner);
    assert!(
        matches!(fs::symlink_metadata(fixture.path().join("catalog.sqlite")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound)
    );
    assert!(
        matches!(fs::symlink_metadata(fixture.path().join("canon.sqlite")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound)
    );
    let mut selected_rows = BTreeMap::<(String, String), BTreeMap<String, Vec<u8>>>::new();
    let mut selected_collections = repository_plan.receipt().collections.clone();
    selected_collections.extend(canon_plan.receipt().collections.iter().map(|c| {
        InputCollectionReceipt {
            source_graph: c.source_graph.clone(),
            collection: c.collection.clone(),
            input_role: c.input_role.clone(),
            adapter_profile: c.adapter_profile.clone(),
            expected_count: c.count,
            expected_root_sha256: c.root_sha256.clone(),
        }
    }));
    for source in &vocabulary.sources {
        if [
            "repository-topology-v1",
            "canon-node-relation-v1",
            "candidate-relation-v1",
        ]
        .contains(&source.adapter_profile.as_str())
        {
            continue;
        }
        if source.adapter_profile == "declared-identity-and-source-ref-joins-v1" {
            // This derived family declares scope, not source node/edge rows.
            // Its actual relations consume the complete same-cut base seal.
            selected_collections.push(raw_receipt(
                &source.source_graph_id,
                "join_scope",
                &source.input_role,
                &source.adapter_profile,
                &BTreeMap::new(),
            ));
            continue;
        }
        let names: &[&str] = if source.adapter_profile == "reified-bibliographic-claims-v1" {
            &["nodes", "edges", "claim_traces"]
        } else {
            &["nodes", "edges"]
        };
        for &collection in names {
            let mut rows = BTreeMap::new();
            let values = match source.adapter_profile.as_str() {
                "source-navigation-node-edge-v1" => navigation.value()[collection]
                    .as_array()
                    .unwrap()
                    .as_slice(),
                "reified-bibliographic-claims-v1" => {
                    graph.0.get(collection).map_or(&[][..], Vec::as_slice)
                }
                // The selected authored recipe has no inputs for these raw
                // families. Zero receipts are explicit; no philosophy original
                // component or whole-source philosophy parity is claimed.
                "philosophy-node-edge-v1" | "indexed-node-edge-v1" => &[],
                _ => panic!("unsupported selected source family"),
            };
            let key = match collection {
                "nodes" => "node_id",
                "edges" => "edge_id",
                "claim_traces" => "claim_ref",
                _ => unreachable!(),
            };
            for value in values {
                let id = value[key].as_str().unwrap().to_owned();
                let raw = serde_json::to_vec(value).unwrap();
                assert!(raw.len() <= limits.catalog.max_output_row_bytes);
                assert!(rows.insert(id, raw).is_none());
            }
            selected_collections.push(raw_receipt(
                &source.source_graph_id,
                collection,
                &source.input_role,
                &source.adapter_profile,
                &rows,
            ));
            assert!(
                selected_rows
                    .insert((source.source_graph_id.clone(), collection.into()), rows)
                    .is_none()
            );
        }
    }
    let mut selected_binding = binding.clone();
    selected_binding.projection_root_sha256 =
        Digest256::of_bytes(&serde_json::to_vec(&selected_collections).unwrap()).to_hex();
    assert!(!selected_path.exists());
    fs::create_dir_all(selected_path.parent().unwrap()).unwrap();
    // Keep the actual SQLite allocation compatible with the existing final
    // VACUUM's two-database temp reserve. This narrows output, not host quota.
    let search_work_bytes = 2 * 1024 * 1024 * 1024;
    let mut selected_stage_limits = stage_limits;
    // This independent fixture stage has a finite 2 GiB work ceiling.
    // selected_rows excludes repository and canon rows streamed from plans,
    // so its raw length is not the complete normalized search workload.
    // The ceiling covers document
    // materialization, trigram probes/rehash/sort, posting writes and merges,
    // and final root verification under separate row/posting/temp limits. This
    // is a fixture work ceiling, not a measured total or a production limit.
    selected_stage_limits.sqlite.max_work_bytes = search_work_bytes;
    selected_stage_limits.sqlite.max_output_bytes = stage_limits
        .sqlite
        .max_output_bytes
        .min(stage_limits.max_temp_bytes / 2);
    let selected_isolation = NativeSelectedIsolation {
        started,
        deadline,
        cancelled: &cancelled,
        enabled: AtomicBool::new(false),
        catalog_seen: AtomicBool::new(false),
        search_seen: AtomicBool::new(false),
    };
    let mut selected_stage = KnowledgeStage::create(
        &selected_path,
        selected_stage_limits,
        ExactInputReceipt {
            binding: selected_binding.clone(),
            collections: selected_collections,
        },
        &owner,
        &selected_isolation,
    )
    .unwrap();
    render_repository_source_plan(
        &mut selected_stage,
        &repository_plan,
        &vocabulary,
        deadline,
        &cancelled,
    )
    .unwrap();
    // The source planner is consumed by its first successful render above.
    // Continue from that actual frozen output, never reopen an erased plan or
    // recreate private tables in the independent selected stage.
    let mut transfer_work = 0u64;
    for collection in &canon_plan.receipt().collections {
        let mut after = None;
        let mut count = 0u64;
        let mut root = tos_foundation::Digest256Hasher::new();
        loop {
            assert!(Instant::now() < deadline);
            assert!(!cancelled.load(std::sync::atomic::Ordering::Relaxed));
            let page = target
                .scan_input(
                    &collection.source_graph,
                    &collection.collection,
                    after.as_deref(),
                    16,
                )
                .unwrap();
            let mut borrowed = Vec::with_capacity(page.rows.len());
            for row in &page.rows {
                transfer_work = transfer_work
                    .checked_add(u64::try_from(row.id.len() + row.payload.len()).unwrap())
                    .filter(|n| *n <= stage_limits.sqlite.max_work_bytes)
                    .expect("native selected transfer work budget");
                count += 1;
                assert!(count <= collection.count);
                root.update(&(row.id.len() as u64).to_be_bytes());
                root.update(row.id.as_bytes());
                root.update(Digest256::of_bytes(&row.payload).as_bytes());
                borrowed.push(InputRow {
                    source_graph: &collection.source_graph,
                    collection: &collection.collection,
                    id: &row.id,
                    payload: &row.payload,
                });
            }
            if !borrowed.is_empty() {
                selected_stage.ingest_input_batch(&borrowed).unwrap();
            }
            after = page.next_id;
            if after.is_none() {
                break;
            }
        }
        assert_eq!(count, collection.count);
        assert_eq!(root.finalize().to_hex(), collection.root_sha256);
    }
    // Canon EOF/count/root was checked during transfer above. The native
    // projection already owns its independent Original producer evidence.
    drop(target);
    assert!(
        matches!(fs::symlink_metadata(fixture.path().join("corpus.sqlite")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound)
    );
    for ((source, collection), rows) in &selected_rows {
        let mut entries = rows.iter();
        loop {
            let borrowed = entries
                .by_ref()
                .take(stage_limits.max_seek_rows)
                .map(|(id, raw)| InputRow {
                    source_graph: source,
                    collection,
                    id,
                    payload: raw,
                })
                .collect::<Vec<_>>();
            if borrowed.is_empty() {
                break;
            }
            selected_stage.ingest_input_batch(&borrowed).unwrap();
        }
    }
    // Bind this original plan to the final selected transport, not the prior
    // corpus planner's projection root. The producer validates every other
    // source-selection field and preserves its independent output proof.
    let original = tos_compiler::prepare_native_corpus_original(
        &projection,
        &tos_foundation::RelativePath::parse(&output_path).unwrap(),
        &selected_binding,
        &vocabulary,
        tos_compiler::CorpusOriginalSourceLimits {
            originals,
            max_members: 1,
            max_work_bytes: 64 * 1024 * 1024,
        },
        deadline,
        &cancelled,
    )
    .unwrap();
    let mut navigation_header = navigation.value().clone();
    for field in ["nodes", "edges", "rights"] {
        navigation_header.as_object_mut().unwrap().remove(field);
    }
    let navigation_header_raw = serde_json::to_vec(&navigation_header).unwrap();
    let navigation_claim = tos_compiler::NavigationHeaderClaim {
        expected_sha256: Digest256::of_bytes(&navigation_header_raw).to_hex(),
        raw_json: navigation_header_raw,
    };
    let rights = navigation.value()["rights"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| serde_json::to_vec(r).unwrap())
        .collect::<Vec<_>>();
    let rights_refs = rights.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let rights_root = tos_compiler::navigation_original_rights_root(&rights_refs);
    let additional = tos_compiler::NativeFamilyInputs {
        prepared_canon_projection: false,
        prepared_philosophy_projection: false,
        repository_root: Some(repository_plan.root_input()),
        navigation_original: Some(tos_compiler::NavigationOriginalInput {
            rights: &rights_refs,
            expected_rights_root_sha256: &rights_root,
            limits: originals,
        }),
        philosophy_original: None,
        corpus_original: Some(&original),
        topology: tos_compiler::TopologyLimits {
            max_rows: stage_limits.sqlite.max_rows,
            max_page_rows: 16,
            max_row_bytes: stage_limits.sqlite.max_row_bytes,
            max_work_bytes: stage_limits.sqlite.max_work_bytes,
        },
        canon_prepare: tos_compiler::knowledge_canon_prepare::CanonPrepareLimits {
            max_nodes: stage_limits.sqlite.max_rows,
            max_packs: stage_limits.sqlite.max_rows,
            max_edges: stage_limits.sqlite.max_rows,
            max_node_relations: stage_limits.sqlite.max_rows,
            max_page_rows: 16,
            max_row_bytes: limits.catalog.max_output_row_bytes,
            max_work_bytes: stage_limits.sqlite.max_work_bytes,
        },
        canon: tos_compiler::knowledge_canon_materialize::CanonMaterializeLimits {
            max_raw_bytes: limits.catalog.max_output_row_bytes,
            max_output_bytes: limits.catalog.max_output_row_bytes,
            max_registry_bytes: 4 * 1024 * 1024,
            max_page_rows: 16,
            max_page_bytes: 16 * 1024 * 1024,
            max_rows: stage_limits.sqlite.max_rows,
            max_work_bytes: stage_limits.sqlite.max_work_bytes,
        },
        indexed: tos_compiler::IndexedLimits {
            max_row_bytes: limits.catalog.max_output_row_bytes,
            max_page_rows: 16,
        },
        processing_run: None,
    };
    selected_isolation.enabled.store(true, Ordering::Relaxed);
    let selected = tos_compiler::knowledge_full_fixture::finish_native_source_fixture(
        selected_stage,
        selected_path,
        &files["ToS/doctrine/semantic-interchange/entity-types.v1.json"],
        &files["ToS/doctrine/semantic-interchange/relation-types.v1.json"],
        vocabulary.clone(),
        descriptor,
        &navigation_claim,
        additional,
        full_limits,
    );
    let cold = selected.open().unwrap();
    let receipt = cold.corpus_original_receipt().unwrap();
    assert_eq!(
        receipt.origin.native_producer.as_ref(),
        Some(projection.receipt())
    );
    assert!(receipt.origin.source_git_commit.is_none());
    assert_eq!(receipt.origin.members.len(), 1);
    assert_eq!(
        selected
            .corpus_original
            .as_ref()
            .unwrap()
            .component_root_sha256,
        selected
            .expectation
            .corpus_original_root_sha256
            .as_ref()
            .unwrap()
            .as_str()
    );
    assert_eq!(selected.expectation.source_cut, binding.source_cut);
    assert_eq!(
        selected.expectation.membership_root,
        membership.digest.to_hex()
    );
    assert_eq!(
        selected.expectation.through_commit_seq,
        binding.through_commit_seq
    );
    drop(cold);
    assert_eq!(projection.receipt().source_revision, revision.0.to_hex());
    assert!(!selected.graph_input_bytes.is_empty());
    selected
}

fn native_corpus_composition_case(installed: bool) {
    use tos_compiler::knowledge_canon_source::*;
    use tos_compiler::knowledge_stage::{InputCollectionReceipt, InputRow};
    use tos_compiler::source_bibliographic::{BibliographicLimits, BibliographicSourceCut};
    use tos_compiler::source_corpus::*;
    use tos_compiler::source_navigation_source::project_source_navigation_from_cut;
    use tos_compiler::source_witness_catalog::{SourceCatalogLimits, SourceCatalogValidator};
    use tos_compiler::{
        SourceCatalogInputLimits, plan_source_catalog_inputs, render_source_bibliographic_plan,
    };
    fn phase(started: Instant, deadline: Instant, name: &str) {
        // Write to the actual stderr handle: libtest's captured eprintln!
        // output is lost when the outer whole-case deadline kills the process.
        let _ = writeln!(
            std::io::stderr().lock(),
            "native corpus phase={name} elapsed_seconds={}",
            started.elapsed().as_secs()
        );
        assert!(
            Instant::now() < deadline,
            "native corpus deadline after {name}"
        );
    }
    fn add_tree(repository: &Path, relative: &str, files: &mut BTreeMap<String, Vec<u8>>) {
        let mut entries = fs::read_dir(repository.join(relative))
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect::<Vec<_>>();
        entries.sort();
        for path in entries {
            let name = path.strip_prefix(repository).unwrap().to_str().unwrap();
            assert!(!path.is_symlink());
            if path.is_dir() {
                add_tree(repository, name, files);
            } else {
                files.insert(name.into(), fs::read(path).unwrap());
            }
        }
    }
    fn raw_receipt(
        source: &str,
        collection: &str,
        role: &str,
        profile: &str,
        rows: &BTreeMap<String, Vec<u8>>,
    ) -> InputCollectionReceipt {
        let mut hash = tos_foundation::Digest256Hasher::new();
        for (id, raw) in rows {
            hash.update(&(id.len() as u64).to_be_bytes());
            hash.update(id.as_bytes());
            hash.update(Digest256::of_bytes(raw).as_bytes());
        }
        InputCollectionReceipt {
            source_graph: source.into(),
            collection: collection.into(),
            input_role: role.into(),
            adapter_profile: profile.into(),
            expected_count: rows.len() as u64,
            expected_root_sha256: hash.finalize().to_hex(),
        }
    }
    let started = Instant::now();
    // The installed case also materializes and seals the selected model,
    // then exercises cold native transports and revocation. Capture through
    // materialization alone takes about three minutes under the admitted
    // four-GiB host profile; leave a finite budget for indexing and readers.
    let deadline = started + Duration::from_secs(if installed { 600 } else { 240 });
    let cancelled = AtomicBool::new(false);
    phase(started, deadline, "source-preparation");
    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let descriptor = fs::read(
        repository.join("rust/crates/tos-compiler/tests/fixtures/query-vocabulary.v1.json"),
    )
    .unwrap();
    let vocabulary =
        QueryVocabulary::parse(&descriptor, tos_compiler::NATIVE_KNOWLEDGE_ADAPTER_PROFILES)
            .unwrap();
    let full_limits = corpus_full_limits(vocabulary.sources.len());
    if installed {
        native_managed_corpus_consumer::preflight(
            tos_compiler::knowledge_full_fixture::native_fixture_cold_limits(
                full_limits,
                vocabulary.sources.len(),
            ),
        );
        phase(started, deadline, "preflight-passed");
    }
    let (mut files, _, mut reference) = sources(deadline);
    let vocabulary_path = "ToS/doctrine/semantic-interchange/query-vocabulary.v1.json";
    files.insert(
        vocabulary_path.into(),
        fs::read(repository.join(vocabulary_path)).unwrap(),
    );
    for name in ["entity-types.v1.json", "relation-types.v1.json"] {
        let path = format!("ToS/doctrine/semantic-interchange/{name}");
        files.insert(path.clone(), fs::read(repository.join(path)).unwrap());
    }
    // Complete current canon-node membership for this selected recipe. No row
    // is synthesized to fit a producer schema, and no post-derivation truncation.
    let mut canon_tree = BTreeMap::new();
    add_tree(&repository, "ToS/canon", &mut canon_tree);
    for (path, raw) in canon_tree {
        if path.ends_with("/node.json") || path.ends_with("/node.human-forms.json") {
            files.insert(path, raw);
        }
    }
    for path in [
        "ToS/canon/relations/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/edges.csv",
        "ToS/candidate-intake/thus-spoke-zarathustra/prologue-1/mode-b/edges.csv",
        "ToS/research-packets/foundation-laboratory-2026-07/JENSEITS_1886_LETTER_705_SOURCE_READING_V1.md",
    ] {
        files.insert(path.into(), fs::read(repository.join(path)).unwrap());
    }
    for path in [
        "ToS/source-witnesses/relations/nietzsche-letter-705-addressee",
        "ToS/source-witnesses/agents/constantin-georg-naumann",
        "ToS/source-witnesses/documents/friedrich-nietzsche/naumann-letter-705",
    ] {
        add_tree(&repository, path, &mut files);
    }
    let historical = super::retained_history_capture::retained_history(&repository);
    let historical_root = historical.path().join("source");
    for path in [
        "ToS/source-witnesses/.record-revisions/2c4c3a4f5cb2cbf1713ebdaa0b27dfcb0729cf980f33591a6e1e2ea6296b8d25-f63f2f0562a6a662be9c5340ddde5686a6de35a8991ad2ad7b53e3a8fd134eba",
        "ToS/source-witnesses/.record-revisions/709df7fb307a1331d25fa253f7159a3ee27b3862898ddf8db74c6cbfc6965438-75afe571bb0254a738c20c0d5d09fac8b11ce0f299068b534ef652ef09575422",
        "ToS/source-witnesses/.record-revisions/3b6ca195bb9bb9fb57cc1e0d9bece8b18011ef12c3d99d614aac5fa3760ad712-8d38fda8bf756906f8ed3543a8cc069082d39b04b188db5050d76a2ad663a497",
    ] {
        add_tree(&historical_root, path, &mut files);
    }
    assert!(files.len() <= 512);
    assert!(files.values().map(Vec::len).sum::<usize>() <= 16 * 1024 * 1024);
    let mut reference_rows = reference_repository_rows(&mut reference, &files).unwrap();
    phase(started, deadline, "selected-inputs-ready");
    let fixture = tempfile::TempDir::new().unwrap();
    let git_root = fixture.path().join("selected-git");
    fs::create_dir(&git_root).unwrap();
    git(&git_root, &["init", "-q"]);
    // This larger caller copy is distinct from sources()'s discarded capture.
    // Charge the same counter again; no credit/refund for that earlier copy.
    let capture_copy = files
        .iter()
        .try_fold(0usize, |sum, (path, raw)| {
            sum.checked_add(path.len())?
                .checked_add(raw.len())?
                .checked_add(256)
        })
        .expect("reference caller capture copy overflow");
    reference.charge(capture_copy).unwrap();
    let mut captured = files.clone();
    // The native capture binds one exact maintained compiler implementation
    // beside the authored ToS cut; neither one becomes the other's authority.
    let compiler = "rust/crates/tos-compiler/src/lib.rs";
    captured.insert(
        compiler.into(),
        fs::read(repository.join(compiler)).unwrap(),
    );
    let corpus_owner = "rust/crates/tos-compiler/src/source_corpus.rs";
    captured.insert(
        corpus_owner.into(),
        fs::read(repository.join(corpus_owner)).unwrap(),
    );
    let worker_source = "rust/crates/tos-validation/src/bin/tos-schema-worker.rs";
    captured.insert(
        worker_source.into(),
        fs::read(repository.join(worker_source)).unwrap(),
    );
    let declaration_raw =
        fs::read(repository.join("access/contracts/runtime-data.v1.json")).unwrap();
    let declaration: Value = serde_json::from_slice(&declaration_raw).unwrap();
    let outputs = declaration["subjects"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|subject| subject["subject_id"] == "tos-corpus-index")
        .collect::<Vec<_>>();
    assert_eq!(outputs.len(), 1);
    let output_path = outputs[0]["source_path"].as_str().unwrap().to_owned();
    captured.insert(
        "access/contracts/runtime-data.v1.json".into(),
        declaration_raw,
    );
    assert!(captured.len() <= 512);
    assert!(captured.values().map(Vec::len).sum::<usize>() <= 16 * 1024 * 1024);
    phase(started, deadline, "capture-inputs-ready");
    for (path, raw) in &captured {
        let target = git_root.join(path);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, raw).unwrap();
    }
    git(
        &git_root,
        &[
            "add",
            "--",
            "ToS",
            "rust/crates/tos-compiler/src/lib.rs",
            "rust/crates/tos-compiler/src/source_corpus.rs",
            "rust/crates/tos-validation/src/bin/tos-schema-worker.rs",
            "access/contracts/runtime-data.v1.json",
        ],
    );
    git(
        &git_root,
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-qm",
            "Finite authored corpus recipe",
        ],
    );
    let commit = String::from_utf8(git(&git_root, &["rev-parse", "HEAD^{commit}"]))
        .unwrap()
        .trim()
        .to_owned();
    phase(started, deadline, "fixture-commit-ready");
    let capture = super::source_cut_cases::captured_software_fixture(
        &git_root,
        &commit,
        &[
            "ToS",
            "rust/crates/tos-compiler/src/lib.rs",
            "rust/crates/tos-compiler/src/source_corpus.rs",
            "rust/crates/tos-validation/src/bin/tos-schema-worker.rs",
            "access/contracts",
        ],
    );
    phase(started, deadline, "software-capture-ready");
    let read_limits = ReadLimits {
        max_manifest_bytes: 2 * 1024 * 1024,
        max_manifest_entries: 512,
        max_selected_object_bytes: 2 * 1024 * 1024,
        json: JsonLimits::default(),
    };
    let software = tos_source_store::SoftwareCaptureReader::open(
        &capture.capture,
        &capture.restored,
        capture.selection.clone(),
        read_limits,
        deadline,
        &cancelled,
    )
    .unwrap();
    reference.charge(512 * 256).unwrap();
    reference
        .visit(captured.len().checked_mul(captured.len()).unwrap())
        .unwrap();
    let declared = captured
        .iter()
        .map(|(p, raw)| (p.as_str(), raw.as_slice()))
        .collect::<BTreeMap<_, _>>();
    let observed = software
        .members()
        .map(|m| {
            (
                m.path.as_str(),
                usize::try_from(m.size_bytes).unwrap(),
                m.sha256,
            )
        })
        .collect::<Vec<_>>();
    assert!(reference_metadata_matches(&declared, &observed));
    drop(declared);
    drop(observed);
    let store = fixture.path().join("source-store");
    let revision = super::validation_cut_cases::write_cut_store(&files, &store);
    // The retained consumer recipe stays small and keeps its historical
    // oracle. Prepare the complete, separately identified writer input before
    // the expensive catalog/index stages so missing inputs fail here.
    let producer = installed.then(|| {
        let base = native_managed_corpus_consumer::prepare_native_corpus_build(
            tos_compiler::knowledge_full_fixture::native_fixture_cold_limits(
                full_limits,
                vocabulary.sources.len(),
            ),
            &repository,
            &store,
            revision,
            &capture,
            &super::validation_cut_cases::selected_worker_path(),
        );
        native_managed_corpus_consumer::prepare_producer_source(
            &base,
            &repository,
            &fixture.path().join("producer-source-store"),
        )
    });
    let reader = CorpusReader::open_existing(&store, read_limits).unwrap();
    let cut = reader
        .open_source_cut(
            revision,
            tos_source_store::CutReadLimits {
                max_revisions: 1,
                max_members: 512,
                max_total_bytes: 16 * 1024 * 1024,
                max_member_bytes: 2 * 1024 * 1024,
            },
            deadline,
            &cancelled,
        )
        .unwrap();
    phase(started, deadline, "source-cut-ready");
    let membership = cut.stream(revision).unwrap().expectation();
    let binding = SourceBinding {
        owner_profile: "private-selected-fixture".into(),
        source_cut: "finite-native-corpus-source".into(),
        through_commit_seq: 0,
        membership_root: membership.digest.to_hex(),
        index_generation: revision.0.to_hex(),
        route_map_version: "fixture-v1".into(),
        reader_abi: "fixture-v1".into(),
        projection_root_sha256: Digest256::of_bytes(b"independent-native-corpus-plan").to_hex(),
        complete: true,
    };
    let limits = BibliographicLimits {
        catalog: SourceCatalogLimits {
            max_files: 512,
            max_rows: 4096,
            max_file_bytes: 2 * 1024 * 1024,
            max_row_bytes: 1024 * 1024,
            max_contract_bytes: 16 * 1024 * 1024,
            max_output_row_bytes: 1024 * 1024,
        },
        max_claim_cohort_rows: 16,
        max_claim_cohort_bytes: 16 * 1024 * 1024,
        max_output_rows: 4096,
        max_output_bytes: 16 * 1024 * 1024,
        deadline,
    };
    let catalog_plan = plan_source_catalog_inputs(
        &cut,
        revision,
        membership,
        &binding,
        SourceCatalogInputLimits {
            max_manifest_members: 512,
            max_selected_members: 512,
            max_plan_bytes: 16 * 1024 * 1024,
            max_work_bytes: 64 * 1024 * 1024,
        },
        limits,
        &cancelled,
    )
    .unwrap();
    let owner = FixtureOwner;
    let isolation = FixtureIsolation;
    let stage_limits = StageLimits {
        sqlite: Limits {
            max_rows: 8192,
            max_row_bytes: 2 * 1024 * 1024,
            max_output_bytes: 64 * 1024 * 1024,
            max_work_bytes: 128 * 1024 * 1024,
            sqlite_cache_kib: 512,
            max_sql_vm_steps: 20_000_000,
        },
        max_temp_bytes: 64 * 1024 * 1024,
        max_seek_rows: 16,
        max_seek_bytes: 32 * 1024 * 1024,
    };
    let mut catalog_stage = KnowledgeStage::create(
        &fixture.path().join("catalog.sqlite"),
        stage_limits,
        catalog_plan.input_receipt(),
        &owner,
        &isolation,
    )
    .unwrap();
    let mut operation = tos_validation::executor::BatchStreamBudget::laboratory();
    operation.max_chunks = limits.max_output_rows;
    operation.max_total_units = limits.max_output_rows;
    let worker_path = super::validation_cut_cases::selected_worker_path();
    let worker = ExactWorkerIdentity {
        sha256: Digest256::of_bytes(&fs::read(&worker_path).unwrap()),
        absolute_path: worker_path,
    };
    // Exact schema closure and image preparation belong to this whole case.
    // The scalar laboratory's five-second default may expire during that
    // preparation; only time left on the original whole clock is available.
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .filter(|duration| !duration.is_zero())
        .expect("native corpus whole schema preparation deadline");
    let schema_budget = ExecutorBudget {
        execution_wall: remaining,
        ..ExecutorBudget::laboratory()
    };
    operation.total_execution_wall = remaining;
    operation.operation_cpu_seconds = remaining.as_secs().saturating_add(1);
    operation.operation_address_space_bytes = schema_budget.address_space_bytes;
    let validator = SourceCatalogValidator::from_cut(
        &cut,
        &worker,
        schema_budget,
        CutWorkerLimits {
            max_receipts: 4096,
            max_receipt_bytes: 16 * 1024 * 1024,
        },
        operation,
        deadline,
        &cancelled,
    )
    .unwrap();
    let mut forms = tos_command::source_forms_compiler::NativeBibliographicForms;
    phase(started, deadline, "bibliographic-render-start");
    let candidate = render_source_bibliographic_plan(
        &catalog_plan,
        &cut,
        revision,
        membership,
        &mut catalog_stage,
        &validator,
        &mut forms,
        limits,
        512,
        16 * 1024 * 1024,
    )
    .unwrap();
    phase(started, deadline, "bibliographic-candidate-ready");
    assert!(candidate.catalog.record_count > 1);
    assert!(candidate.bibliographic.edge_count > 0);
    let canon_files = files
        .iter()
        .filter(|(p, _)| {
            (p.starts_with("ToS/canon/")
                && (p.ends_with("/node.json") || p.ends_with("/node.human-forms.json")))
                || ((p.starts_with("ToS/canon/") || p.starts_with("ToS/candidate-intake/"))
                    && p.ends_with("/edges.csv"))
        })
        .map(|(p, b)| (p.clone(), b.clone()))
        .collect::<BTreeMap<_, _>>();
    let contracts = [
        "ToS/contracts/tos-node-contract.schema.json",
        "ToS/contracts/human-form-set.schema.json",
    ]
    .into_iter()
    .map(|p| (p.into(), files[p].clone()))
    .collect::<BTreeMap<_, _>>();
    let canon_inputs = vec![
        raw_receipt(
            CANON_SOURCE_CUSTODY,
            "source-files",
            CANON_SOURCE_FILES_ROLE,
            CANON_SOURCE_FILES_PROFILE,
            &canon_files,
        ),
        raw_receipt(
            CANON_SOURCE_CUSTODY,
            "contracts",
            CANON_SOURCE_CONTRACTS_ROLE,
            CANON_SOURCE_CONTRACTS_PROFILE,
            &contracts,
        ),
    ];
    let mut canon_planner = KnowledgeStage::create(
        &fixture.path().join("canon.sqlite"),
        stage_limits,
        ExactInputReceipt {
            binding: binding.clone(),
            collections: canon_inputs,
        },
        &owner,
        &isolation,
    )
    .unwrap();
    for (collection, rows) in [("source-files", &canon_files), ("contracts", &contracts)] {
        let mut entries = rows.iter();
        loop {
            let borrowed = entries
                .by_ref()
                .take(stage_limits.max_seek_rows)
                .map(|(id, raw)| InputRow {
                    source_graph: CANON_SOURCE_CUSTODY,
                    collection,
                    id,
                    payload: raw,
                })
                .collect::<Vec<_>>();
            if borrowed.is_empty() {
                break;
            }
            canon_planner.ingest_input_batch(&borrowed).unwrap();
        }
    }
    let canon_limits = CanonSourceLimits {
        max_manifest_members: 512,
        max_selected_members: 512,
        max_nodes: 128,
        max_packs: 16,
        max_edges: 1024,
        max_source_bytes: 2 * 1024 * 1024,
        max_raw_row_bytes: 2 * 1024 * 1024,
        max_csv_fields: 128,
        max_csv_record_bytes: 2 * 1024 * 1024,
        max_forms: 256,
        max_forms_output_bytes: 262144,
        max_page_rows: 8,
        max_page_bytes: 16 * 1024 * 1024,
        max_work_bytes: 64 * 1024 * 1024,
    };
    let root_raw = serde_json::to_vec(
        &json!({"node_id":"fixture-source-root","node_type":"repository-root",
        "label":"Selected fixture repository","source_ref":"ToS/source_home.manifest.json"}),
    )
    .unwrap();
    let root_sha = Digest256::of_bytes(&root_raw).to_hex();
    let (repository_plan, canon_plan) = plan_native_corpus_source_families(
        &mut canon_planner,
        &cut,
        &software,
        &binding,
        &vocabulary,
        RepositoryRootInput {
            source_cut: &binding.source_cut,
            material: &root_raw,
            material_sha256: &root_sha,
            identity_id: "root:fixture-source-root",
        },
        &validator,
        RepositorySourceLimits {
            max_inventory_members: 512,
            max_source_bytes: 2 * 1024 * 1024,
            max_row_bytes: 2 * 1024 * 1024,
            max_plan_bytes: 16 * 1024 * 1024,
            max_work_bytes: 32 * 1024 * 1024,
        },
        canon_limits,
        deadline,
        &cancelled,
        tos_command::source_forms_compiler::materialize_compiler_forms,
    )
    .unwrap();
    assert!(canon_plan.receipt().nodes > 0);
    assert!(canon_plan.receipt().packs >= 2);
    assert!(canon_plan.receipt().edges > 0);
    let entities: Value =
        serde_json::from_slice(&files["ToS/doctrine/semantic-interchange/entity-types.v1.json"])
            .unwrap();
    let source = BibliographicSourceCut {
        cut: &cut,
        expected_revision: revision,
        expected_membership: membership,
        stage_source_cut: &binding.source_cut,
        max_read_files: 512,
        max_read_bytes: 16 * 1024 * 1024,
    };
    let navigation = project_source_navigation_from_cut(
        &mut catalog_stage,
        &candidate.catalog,
        &source,
        &validator,
        &entities,
        &mut forms,
        limits,
    )
    .unwrap();
    assert!(!navigation.value()["edges"].as_array().unwrap().is_empty());
    let mut collections = repository_plan.receipt().collections.clone();
    collections.extend(
        canon_plan
            .receipt()
            .collections
            .iter()
            .map(|c| InputCollectionReceipt {
                source_graph: c.source_graph.clone(),
                collection: c.collection.clone(),
                input_role: c.input_role.clone(),
                adapter_profile: c.adapter_profile.clone(),
                expected_count: c.count,
                expected_root_sha256: c.root_sha256.clone(),
            }),
    );
    collections.extend(catalog_plan.input_receipt().collections);
    let mut target = KnowledgeStage::create(
        &fixture.path().join("corpus.sqlite"),
        stage_limits,
        ExactInputReceipt {
            binding: binding.clone(),
            collections,
        },
        &owner,
        &isolation,
    )
    .unwrap();
    render_repository_source_plan(
        &mut target,
        &repository_plan,
        &vocabulary,
        deadline,
        &cancelled,
    )
    .unwrap();
    render_canon_source_plan(
        &mut canon_planner,
        &canon_plan,
        &cut,
        revision,
        membership,
        &mut target,
        canon_limits,
        deadline,
        &cancelled,
    )
    .unwrap();
    for input in &catalog_plan.input_receipt().collections {
        let mut after = None;
        loop {
            let page = catalog_stage
                .scan_input(
                    &input.source_graph,
                    &input.collection,
                    after.as_deref(),
                    stage_limits.max_seek_rows,
                )
                .unwrap();
            if !page.rows.is_empty() {
                let borrowed = page
                    .rows
                    .iter()
                    .map(|row| InputRow {
                        source_graph: &input.source_graph,
                        collection: &input.collection,
                        id: &row.id,
                        payload: &row.payload,
                    })
                    .collect::<Vec<_>>();
                target.ingest_input_batch(&borrowed).unwrap();
            }
            after = page.next_id;
            if after.is_none() {
                break;
            }
        }
    }
    let originals = tos_compiler::NavigationOriginalLimits {
        max_rows: 4096,
        max_row_bytes: limits.catalog.max_output_row_bytes,
        max_total_bytes: 16 * 1024 * 1024,
    };
    phase(started, deadline, "native-projection-start");
    let projection = project_native_corpus_from_sources(
        &mut target,
        &vocabulary,
        &cut,
        &software,
        &repository_plan,
        &canon_plan,
        &navigation,
        &validator,
        NativeCorpusLimits {
            originals,
            max_work_bytes: 128 * 1024 * 1024,
            schema_work: operation,
        },
        deadline,
        &cancelled,
    )
    .unwrap();
    // The projection already finished the one shared schema operation. Close
    // its sealed worker image before the independent oracle and selected DB.
    drop(validator);
    phase(started, deadline, "native-projection-ready");
    // The maintained whole oracle receives exactly the selected authored paths.
    // Generated catalogue companions are private oracle outputs, not new inputs.
    let oracle_root = fixture.path().join("oracle");
    for (path, raw) in &files {
        let to = oracle_root.join(path);
        fs::create_dir_all(to.parent().unwrap()).unwrap();
        fs::write(to, raw).unwrap();
    }
    let historical_raw = crate::frozen_legacy_python_oracle_raw("corpus-composition");
    let mut expected: Value = serde_json::from_slice(&historical_raw).unwrap();
    reconcile_source_home_inputs(&mut expected, &files);
    assert_eq!(
        expected["validation_refs"],
        serde_json::json!([
            "scripts/build_tos_corpus_index.py",
            "scripts/validate_tos_corpus_index.py",
            "tests/test_tos_corpus_index.py",
        ])
    );
    expected["validation_refs"] = serde_json::json!([
        "rust/crates/tos-compiler/src/source_corpus.rs",
        "tests/conformance/rust/compiler_source_cases.rs",
    ]);
    assert_eq!(expected["resources"].as_array().unwrap().len(), 337);
    for collection in ["branches", "manifests", "resources"] {
        expected[collection] = serde_json::json!(reference_rows.remove(collection).unwrap());
    }
    expected["counts"]["resources"] =
        serde_json::json!(expected["resources"].as_array().unwrap().len());
    assert_retained_corpus_navigation_extension(
        &projection.value()["source_navigation"],
        &expected["source_navigation"],
        &files,
    );
    expected["source_navigation"] = projection.value()["source_navigation"].clone();
    expected["counts"]["source_navigation_nodes"] = json!(12);
    expected["counts"]["source_navigation_edges"] = json!(10);
    let expected_limits = JsonLimits::new(16 * 1024 * 1024, 96, 1_000_000, 4096).unwrap();
    let expected_document = parse_json(
        &serde_json::to_vec(&expected).unwrap(),
        JsonMode::PublishedStrict,
        expected_limits,
    )
    .unwrap();
    let mut expected_raw = canonical_bytes_v1(
        expected_document.root(),
        CanonicalProfile::SourceRecordDigestV1,
        expected_limits,
    )
    .unwrap();
    expected_raw.push(b'\n');
    let mut query_oracle: Value = crate::frozen_legacy_python_oracle("corpus-query-cases");
    reconcile_source_home_inputs(&mut query_oracle, &files);
    // CurrentDeclared counts derive from the independently frozen input recipe;
    // the historical query packets retain their own 337-resource source scope.
    // Preserve historical query fields, updating the nine resource and independently checked navigation count groups.
    for pointer in [
        "/cases/status/counts/resources",
        "/cases/summary/counts/resources",
        "/cases/summary/status/counts/resources",
        "/cases/topology/counts/resources",
        "/cases/route/counts/resources",
        "/cases/promotion/counts/resources",
        "/cases/packet/counts/resources",
        "/cases/packet/view/counts/resources",
        "/cases/packet-empty/counts/resources",
    ] {
        let count = query_oracle.pointer_mut(pointer).unwrap();
        assert_eq!(*count, serde_json::json!(337));
        *count = expected["counts"]["resources"].clone();
        let parent = pointer.strip_suffix("/resources").unwrap();
        for (field, old, new) in [
            ("source_navigation_nodes", 6, 12),
            ("source_navigation_edges", 4, 10),
        ] {
            let value = query_oracle
                .pointer_mut(&format!("{parent}/{field}"))
                .unwrap();
            assert_eq!(*value, json!(old));
            *value = json!(new);
        }
    }
    for pointer in [
        "/cases/search/schema",
        "/cases/search-filtered/schema",
        "/cases/packet/schema",
        "/cases/packet-empty/schema",
    ] {
        let schema = query_oracle.pointer_mut(pointer).unwrap();
        let historical =
            if pointer.ends_with("/packet/schema") || pointer.ends_with("/packet-empty/schema") {
                "tos_corpus_mcp_packet_v1"
            } else {
                "tos_corpus_mcp_search_v1"
            };
        assert_eq!(schema.as_str(), Some(historical));
        *schema = match pointer {
            "/cases/search/schema" | "/cases/search-filtered/schema" => {
                serde_json::json!("tos_corpus_mcp_search_v2")
            }
            _ => serde_json::json!("tos_corpus_mcp_packet_v2"),
        };
    }
    assert_eq!(projection.value(), &expected);
    assert_eq!(projection.output_bytes(), expected_raw.as_slice());
    phase(started, deadline, "maintained-oracle-matched");
    // A fresh independently declared selected stage receives actual family
    // outputs. Catalogue custody and private planner tables stay out of its DDL.
    let mut graph = BibliographicOutput::default();
    tos_compiler::source_bibliographic::render_bibliographic_graph(
        &mut catalog_stage,
        &candidate.catalog,
        &candidate.bibliographic,
        limits,
        &mut graph,
    )
    .unwrap();
    let mut selected_rows = BTreeMap::<(String, String), BTreeMap<String, Vec<u8>>>::new();
    let mut selected_collections = repository_plan.receipt().collections.clone();
    selected_collections.extend(canon_plan.receipt().collections.iter().map(|c| {
        InputCollectionReceipt {
            source_graph: c.source_graph.clone(),
            collection: c.collection.clone(),
            input_role: c.input_role.clone(),
            adapter_profile: c.adapter_profile.clone(),
            expected_count: c.count,
            expected_root_sha256: c.root_sha256.clone(),
        }
    }));
    for source in &vocabulary.sources {
        if [
            "repository-topology-v1",
            "canon-node-relation-v1",
            "candidate-relation-v1",
        ]
        .contains(&source.adapter_profile.as_str())
        {
            continue;
        }
        if source.adapter_profile == "declared-identity-and-source-ref-joins-v1" {
            // This derived family declares scope, not source node/edge rows.
            // Its actual relations consume the complete same-cut base seal.
            selected_collections.push(raw_receipt(
                &source.source_graph_id,
                "join_scope",
                &source.input_role,
                &source.adapter_profile,
                &BTreeMap::new(),
            ));
            continue;
        }
        let names: &[&str] = if source.adapter_profile == "reified-bibliographic-claims-v1" {
            &["nodes", "edges", "claim_traces"]
        } else {
            &["nodes", "edges"]
        };
        for &collection in names {
            let mut rows = BTreeMap::new();
            let values = match source.adapter_profile.as_str() {
                "source-navigation-node-edge-v1" => navigation.value()[collection]
                    .as_array()
                    .unwrap()
                    .as_slice(),
                "reified-bibliographic-claims-v1" => {
                    graph.0.get(collection).map_or(&[][..], Vec::as_slice)
                }
                // The selected authored recipe has no inputs for these raw
                // families. Zero receipts are explicit; no philosophy original
                // component or whole-source philosophy parity is claimed.
                "philosophy-node-edge-v1" | "indexed-node-edge-v1" => &[],
                _ => panic!("unsupported selected source family"),
            };
            let key = match collection {
                "nodes" => "node_id",
                "edges" => "edge_id",
                "claim_traces" => "claim_ref",
                _ => unreachable!(),
            };
            for value in values {
                let id = value[key].as_str().unwrap().to_owned();
                let raw = serde_json::to_vec(value).unwrap();
                assert!(raw.len() <= limits.catalog.max_output_row_bytes);
                assert!(rows.insert(id, raw).is_none());
            }
            selected_collections.push(raw_receipt(
                &source.source_graph_id,
                collection,
                &source.input_role,
                &source.adapter_profile,
                &rows,
            ));
            assert!(
                selected_rows
                    .insert((source.source_graph_id.clone(), collection.into()), rows)
                    .is_none()
            );
        }
    }
    let mut selected_binding = binding.clone();
    selected_binding.projection_root_sha256 =
        Digest256::of_bytes(&serde_json::to_vec(&selected_collections).unwrap()).to_hex();
    let selected_dir = fixture.path().join("native-selected");
    fs::create_dir(&selected_dir).unwrap();
    let selected_path = selected_dir.join("knowledge.sqlite3");
    // Keep the actual SQLite allocation compatible with the existing final
    // VACUUM's two-database temp reserve. This narrows output, not host quota.
    let search_work_bytes = 2 * 1024 * 1024 * 1024;
    let mut selected_stage_limits = stage_limits;
    // This independent fixture stage has a finite 2 GiB work ceiling.
    // selected_rows excludes repository and canon rows streamed from plans,
    // so its raw length is not the complete normalized search workload.
    // The ceiling covers document
    // materialization, trigram probes/rehash/sort, posting writes and merges,
    // and final root verification under separate row/posting/temp limits. This
    // is a fixture work ceiling, not a measured total or a production limit.
    selected_stage_limits.sqlite.max_work_bytes = search_work_bytes;
    selected_stage_limits.sqlite.max_output_bytes = stage_limits
        .sqlite
        .max_output_bytes
        .min(stage_limits.max_temp_bytes / 2);
    let selected_isolation = NativeSelectedIsolation {
        started,
        deadline,
        cancelled: &cancelled,
        enabled: AtomicBool::new(false),
        catalog_seen: AtomicBool::new(false),
        search_seen: AtomicBool::new(false),
    };
    let mut selected_stage = KnowledgeStage::create(
        &selected_path,
        selected_stage_limits,
        ExactInputReceipt {
            binding: selected_binding.clone(),
            collections: selected_collections,
        },
        &owner,
        &selected_isolation,
    )
    .unwrap();
    render_repository_source_plan(
        &mut selected_stage,
        &repository_plan,
        &vocabulary,
        deadline,
        &cancelled,
    )
    .unwrap();
    // The source planner is consumed by its first successful render above.
    // Continue from that actual frozen output, never reopen an erased plan or
    // recreate private tables in the independent selected stage.
    let mut transfer_work = 0u64;
    for collection in &canon_plan.receipt().collections {
        let mut after = None;
        let mut count = 0u64;
        let mut root = tos_foundation::Digest256Hasher::new();
        loop {
            assert!(Instant::now() < deadline);
            assert!(!cancelled.load(std::sync::atomic::Ordering::Relaxed));
            let page = target
                .scan_input(
                    &collection.source_graph,
                    &collection.collection,
                    after.as_deref(),
                    16,
                )
                .unwrap();
            let mut borrowed = Vec::with_capacity(page.rows.len());
            for row in &page.rows {
                transfer_work = transfer_work
                    .checked_add(u64::try_from(row.id.len() + row.payload.len()).unwrap())
                    .filter(|n| *n <= stage_limits.sqlite.max_work_bytes)
                    .expect("native selected transfer work budget");
                count += 1;
                assert!(count <= collection.count);
                root.update(&(row.id.len() as u64).to_be_bytes());
                root.update(row.id.as_bytes());
                root.update(Digest256::of_bytes(&row.payload).as_bytes());
                borrowed.push(InputRow {
                    source_graph: &collection.source_graph,
                    collection: &collection.collection,
                    id: &row.id,
                    payload: &row.payload,
                });
            }
            if !borrowed.is_empty() {
                selected_stage.ingest_input_batch(&borrowed).unwrap();
            }
            after = page.next_id;
            if after.is_none() {
                break;
            }
        }
        assert_eq!(count, collection.count);
        assert_eq!(root.finalize().to_hex(), collection.root_sha256);
    }
    for ((source, collection), rows) in &selected_rows {
        let mut entries = rows.iter();
        loop {
            let borrowed = entries
                .by_ref()
                .take(stage_limits.max_seek_rows)
                .map(|(id, raw)| InputRow {
                    source_graph: source,
                    collection,
                    id,
                    payload: raw,
                })
                .collect::<Vec<_>>();
            if borrowed.is_empty() {
                break;
            }
            selected_stage.ingest_input_batch(&borrowed).unwrap();
        }
    }
    // Bind this original plan to the final selected transport, not the prior
    // corpus planner's projection root. The producer validates every other
    // source-selection field and preserves its independent output proof.
    let original = tos_compiler::prepare_native_corpus_original(
        &projection,
        &tos_foundation::RelativePath::parse(&output_path).unwrap(),
        &selected_binding,
        &vocabulary,
        tos_compiler::CorpusOriginalSourceLimits {
            originals,
            max_members: 1,
            max_work_bytes: 64 * 1024 * 1024,
        },
        deadline,
        &cancelled,
    )
    .unwrap();
    let mut navigation_header = navigation.value().clone();
    for field in ["nodes", "edges", "rights"] {
        navigation_header.as_object_mut().unwrap().remove(field);
    }
    let navigation_header_raw = serde_json::to_vec(&navigation_header).unwrap();
    let navigation_claim = tos_compiler::NavigationHeaderClaim {
        expected_sha256: Digest256::of_bytes(&navigation_header_raw).to_hex(),
        raw_json: navigation_header_raw,
    };
    let rights = navigation.value()["rights"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| serde_json::to_vec(r).unwrap())
        .collect::<Vec<_>>();
    let rights_refs = rights.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let rights_root = tos_compiler::navigation_original_rights_root(&rights_refs);
    let additional = tos_compiler::NativeFamilyInputs {
        prepared_canon_projection: false,
        prepared_philosophy_projection: false,
        repository_root: Some(repository_plan.root_input()),
        navigation_original: Some(tos_compiler::NavigationOriginalInput {
            rights: &rights_refs,
            expected_rights_root_sha256: &rights_root,
            limits: originals,
        }),
        philosophy_original: None,
        corpus_original: Some(&original),
        topology: tos_compiler::TopologyLimits {
            max_rows: stage_limits.sqlite.max_rows,
            max_page_rows: 16,
            max_row_bytes: stage_limits.sqlite.max_row_bytes,
            max_work_bytes: stage_limits.sqlite.max_work_bytes,
        },
        canon_prepare: tos_compiler::knowledge_canon_prepare::CanonPrepareLimits {
            max_nodes: stage_limits.sqlite.max_rows,
            max_packs: stage_limits.sqlite.max_rows,
            max_edges: stage_limits.sqlite.max_rows,
            max_node_relations: stage_limits.sqlite.max_rows,
            max_page_rows: 16,
            max_row_bytes: limits.catalog.max_output_row_bytes,
            max_work_bytes: stage_limits.sqlite.max_work_bytes,
        },
        canon: tos_compiler::knowledge_canon_materialize::CanonMaterializeLimits {
            max_raw_bytes: limits.catalog.max_output_row_bytes,
            max_output_bytes: limits.catalog.max_output_row_bytes,
            max_registry_bytes: 4 * 1024 * 1024,
            max_page_rows: 16,
            max_page_bytes: 16 * 1024 * 1024,
            max_rows: stage_limits.sqlite.max_rows,
            max_work_bytes: stage_limits.sqlite.max_work_bytes,
        },
        indexed: tos_compiler::IndexedLimits {
            max_row_bytes: limits.catalog.max_output_row_bytes,
            max_page_rows: 16,
        },
        processing_run: None,
    };
    phase(started, deadline, "selected-model-start");
    selected_isolation.enabled.store(true, Ordering::Relaxed);
    let selected = tos_compiler::knowledge_full_fixture::finish_native_source_fixture(
        selected_stage,
        selected_path,
        &files["ToS/doctrine/semantic-interchange/entity-types.v1.json"],
        &files["ToS/doctrine/semantic-interchange/relation-types.v1.json"],
        vocabulary.clone(),
        descriptor,
        &navigation_claim,
        additional,
        full_limits,
    );
    phase(started, deadline, "selected-model-sealed");
    let cold = selected.open().unwrap();
    let receipt = cold.corpus_original_receipt().unwrap();
    assert_eq!(
        receipt.origin.native_producer.as_ref(),
        Some(projection.receipt())
    );
    assert!(receipt.origin.source_git_commit.is_none());
    assert_eq!(receipt.origin.members.len(), 1);
    assert_eq!(
        selected
            .corpus_original
            .as_ref()
            .unwrap()
            .component_root_sha256,
        selected
            .expectation
            .corpus_original_root_sha256
            .as_ref()
            .unwrap()
            .as_str()
    );
    assert_eq!(selected.expectation.source_cut, binding.source_cut);
    assert_eq!(
        selected.expectation.membership_root,
        membership.digest.to_hex()
    );
    assert_eq!(
        selected.expectation.through_commit_seq,
        binding.through_commit_seq
    );
    drop(cold);
    let inspect = tos_query::InspectBudget {
        max_open_vm_steps: selected.cold_limits().max_vm_steps,
        max_read_vm_steps: stage_limits.sqlite.max_sql_vm_steps,
        max_matches: usize::try_from(stage_limits.sqlite.max_rows).unwrap(),
        max_rows: stage_limits.sqlite.max_rows,
        max_field_bytes: 8192,
        max_payload_bytes: originals.max_row_bytes,
        max_decoded_bytes: stage_limits.sqlite.max_work_bytes,
        max_response_bytes: 8 * 1024 * 1024,
        json: tos_foundation::JsonLimits::default(),
    };
    phase(started, deadline, "cold-query-start");
    let packets = assert_native_corpus_query_packets(
        &selected,
        &projection,
        &expected,
        &query_oracle,
        &oracle_root,
        &output_path,
        tos_query::corpus_read::CorpusReadBudget {
            inspect,
            // Existing API caller law: logical JSON/string work has its own
            // bound, distinct from row count and SQLite VM accounting.
            max_work_steps: inspect.max_read_vm_steps,
        },
    );
    phase(started, deadline, "cold-query-matched");
    if installed {
        let case_root = fixture.path().join("continuation");
        fs::create_dir(&case_root).unwrap();
        let managed = native_managed_corpus_consumer::prepare_managed_native_corpus(
            &selected,
            &projection,
            &repository,
            &output_path,
            &files,
            &packets,
            &case_root,
        );
        let (case, root, retain) = native_managed_corpus_consumer::finish_preparation(
            managed,
            producer.unwrap(),
            &case_root,
        );
        if retain {
            // Complete immutable inputs survive a later transport/producer
            // failure. Incomplete preparation still has normal TempDir cleanup.
            fixture.keep();
            capture.temporary.keep();
        }
        native_managed_corpus_consumer::run_phase(
            &case,
            &native_managed_corpus_consumer::selected_phase(),
            &root,
        );
    }
}
