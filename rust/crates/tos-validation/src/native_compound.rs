//! Read-only reconstruction of maintained native bibliographic publications.
//! Historical transport evidence never grants a current writer or admission.
use crate::PredicateRead;
use crate::item_rules::{ItemLimits, ItemRefusal};
use crate::record_biblio_cut::{account, check, current, reserve};
use crate::source_cut::{CutSchemaExecutor, CutWorkerSchemaExecutor};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;
use tos_foundation::{
    CanonicalProfile, Digest256, JsonEmissionProfile, JsonLimits, JsonMode, JsonValue,
    RelativePath, canonical_bytes_v1, emit_json_profile, parse_json,
};
use tos_source_store::CorpusCutReader;

const HOME: &str = "ToS/source-witnesses";
const CONTROL: &str = "ToS/source-witnesses/.metadata-publication.json";
const TRANSACTIONS: &str = "ToS/source-witnesses/.metadata-transactions";
const HISTORY: &str = "source-revision-history.json";
const PROTOCOL: &str = "tos_selected_source_metadata_v1";
const MAX_GENERATION: u64 = 9_007_199_254_740_991;
const MAX_SIDE: usize = 8 * 1024 * 1024;
const MAX_FILE: usize = 2 * 1024 * 1024;
const MAX_MANIFEST: usize = 512 * 1024;
const MAX_HISTORY: usize = 128;
pub(crate) type Package = BTreeMap<String, Vec<u8>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeTransportState {
    Committed,
    RolledBack,
    Pending,
    Orphan,
}

#[derive(Debug)]
pub struct NativeCompoundObservation {
    pub claim_path: String,
    pub claim_id: String,
    pub transaction_id: String,
    pub manifest_sha256: String,
    pub transport: NativeTransportState,
}

fn bad(message: &str) -> ItemRefusal {
    ItemRefusal::Source(format!("native compound: {message}"))
}
fn text<'a>(v: &'a Value, key: &str) -> Result<&'a str, ItemRefusal> {
    v.get(key).and_then(Value::as_str).ok_or_else(|| bad(key))
}
fn integer(v: &Value, key: &str) -> Result<u64, ItemRefusal> {
    v.get(key).and_then(Value::as_u64).ok_or_else(|| bad(key))
}
fn array<'a>(v: &'a Value, key: &str) -> Result<&'a Vec<Value>, ItemRefusal> {
    v.get(key).and_then(Value::as_array).ok_or_else(|| bad(key))
}
fn keys(v: &Value, wanted: &[&str]) -> Result<(), ItemRefusal> {
    let object = v.as_object().ok_or_else(|| bad("object required"))?;
    if object.len() != wanted.len() || wanted.iter().any(|k| !object.contains_key(*k)) {
        return Err(bad("exact fields"));
    }
    Ok(())
}
fn hash(s: &str) -> Result<&str, ItemRefusal> {
    let raw = s
        .strip_prefix("sha256:")
        .ok_or_else(|| bad("prefixed sha256"))?;
    if raw.len() != 64
        || !raw
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err(bad("sha256 grammar"));
    }
    Ok(raw)
}
fn limits() -> JsonLimits {
    JsonLimits::new(MAX_SIDE, 64, 300_000, 4_300).expect("finite JSON limits")
}
fn ordered(raw: &[u8]) -> Result<JsonValue, ItemRefusal> {
    parse_json(raw, JsonMode::PublishedStrict, limits())
        .map(|v| v.into_root())
        .map_err(|e| ItemRefusal::Unsupported(format!("compound published JSON: {e:?}")))
}
fn decode(raw: &[u8]) -> Result<Value, ItemRefusal> {
    ordered(raw)?;
    serde_json::from_slice(raw)
        .map_err(|_| ItemRefusal::Unsupported("compound decoded representation".into()))
}
fn canonical(v: &Value) -> Result<Vec<u8>, ItemRefusal> {
    canonical_ordered(&ordered(
        &serde_json::to_vec(v).map_err(|_| bad("serialization"))?,
    )?)
}
fn canonical_ordered(v: &JsonValue) -> Result<Vec<u8>, ItemRefusal> {
    canonical_bytes_v1(v, CanonicalProfile::SourceCommandInputV1, limits())
        .map_err(|e| ItemRefusal::Unsupported(format!("compound canonical: {e:?}")))
}
fn digest(v: &Value) -> Result<String, ItemRefusal> {
    Ok(Digest256::of_bytes(&canonical(v)?).to_prefixed())
}
fn pretty(v: &JsonValue) -> Result<Vec<u8>, ItemRefusal> {
    emit_json_profile(v, JsonEmissionProfile::SourceFormSetPublishedV1, limits())
        .map(|v| v.bytes)
        .map_err(|e| ItemRefusal::Unsupported(format!("compound record bytes: {e:?}")))
}
fn reference(v: &Value, id: &str, version: &str) -> Result<Value, ItemRefusal> {
    let n = integer(v, version)?;
    if n == 0 {
        return Err(bad("positive record version"));
    }
    Ok(json!({"id":text(v,id)?,"version":n,"digest":digest(v)?}))
}
fn file_refs(files: &Package) -> Value {
    Value::Object(
        files
            .iter()
            .map(|(name, raw)| {
                (
                    name.clone(),
                    json!({"sha256":Digest256::of_bytes(raw).to_prefixed(),"bytes":raw.len()}),
                )
            })
            .collect(),
    )
}
fn revision(files: &Package) -> Result<String, ItemRefusal> {
    digest(&file_refs(files))
}
fn selected_names(path: &str) -> Result<[String; 3], ItemRefusal> {
    let (_, base) = path.rsplit_once('/').ok_or_else(|| bad("record path"))?;
    let stem = base
        .strip_suffix(".json")
        .ok_or_else(|| bad("record basename"))?;
    Ok([
        base.into(),
        format!("{stem}.human-forms.json"),
        HISTORY.into(),
    ])
}
fn parent(path: &str) -> Result<&str, ItemRefusal> {
    path.rsplit_once('/')
        .map(|v| v.0)
        .ok_or_else(|| bad("source parent"))
}
fn metadata_path(path: &str, directory: bool) -> Result<(), ItemRefusal> {
    RelativePath::parse(path).map_err(|_| bad("canonical metadata path"))?;
    let parts: Vec<_> = path.split('/').collect();
    if path.len() > 1024
        || !(3..=24).contains(&parts.len())
        || !path.starts_with(&format!("{HOME}/"))
        || parts.iter().any(|p| {
            p.starts_with('.')
                || matches!(
                    *p,
                    "payload" | "private" | "local-content" | "owner-local" | "catalog"
                )
        })
        || !directory && (parts.len() < 4 || !(path.ends_with(".json") || path.ends_with(".jsonl")))
    {
        return Err(bad("public metadata path scope"));
    }
    Ok(())
}
fn is_ancestor(a: &str, b: &str) -> bool {
    b.strip_prefix(a).is_some_and(|tail| tail.starts_with('/'))
}

#[derive(Clone)]
struct Transaction {
    manifest: Value,
    manifest_sha256: String,
    status: String,
    files: BTreeMap<String, (Option<Vec<u8>>, Option<Vec<u8>>)>,
}

/// One family invocation owns the directory index and deduplicated exact reads.
/// Neither cache nor historical state survives the selected operation.
pub(crate) struct WorkExpression<'a> {
    cut: &'a CorpusCutReader,
    limits: ItemLimits,
    cancelled: &'a AtomicBool,
    paths: BTreeSet<String>,
    raw: BTreeMap<String, Vec<u8>>,
    transactions: BTreeMap<String, Transaction>,
    histories: BTreeMap<(String, String), Value>,
    state: usize,
    bytes: u64,
    reads: Vec<PredicateRead>,
    publication: Option<Value>,
}
impl<'a> WorkExpression<'a> {
    pub(crate) fn new(
        cut: &'a CorpusCutReader,
        limits: ItemLimits,
        cancelled: &'a AtomicBool,
    ) -> Result<Self, ItemRefusal> {
        let mut this = Self {
            cut,
            limits,
            cancelled,
            paths: BTreeSet::new(),
            raw: BTreeMap::new(),
            transactions: BTreeMap::new(),
            histories: BTreeMap::new(),
            state: 0,
            bytes: 0,
            reads: Vec::new(),
            publication: None,
        };
        for member in cut.current().members() {
            check(limits.deadline, cancelled)?;
            reserve(
                &mut this.state,
                member.path.as_str().len() + 64,
                limits.max_state_bytes,
            )?;
            this.paths.insert(member.path.as_str().into());
        }
        if let Some(raw) = this.optional(CONTROL, 8192)? {
            let state = decode(&raw)?;
            state_valid(&state)?;
            this.publication = Some(state);
        }
        Ok(this)
    }
    fn optional(&mut self, path: &str, cap: usize) -> Result<Option<Vec<u8>>, ItemRefusal> {
        check(self.limits.deadline, self.cancelled)?;
        if let Some(raw) = self.raw.get(path) {
            if raw.len() > cap {
                return Err(ItemRefusal::Budget);
            }
            return Ok(Some(raw.clone()));
        }
        if !self.paths.contains(path) {
            return Ok(None);
        }
        let raw = current(self.cut, path, self.limits, self.cancelled, &mut self.bytes)?;
        if raw.len() > cap {
            return Err(ItemRefusal::Budget);
        }
        reserve(
            &mut self.state,
            raw.len()
                .checked_mul(3)
                .ok_or(ItemRefusal::Budget)?
                .checked_add(path.len() + 128)
                .ok_or(ItemRefusal::Budget)?,
            self.limits.max_state_bytes,
        )?;
        self.reads.push(PredicateRead::ExactPath {
            path: path.into(),
            digest: Digest256::of_bytes(&raw).to_prefixed(),
        });
        self.raw.insert(path.into(), raw.clone());
        Ok(Some(raw))
    }
    fn required(&mut self, path: &str, cap: usize) -> Result<Vec<u8>, ItemRefusal> {
        self.optional(path, cap)?
            .ok_or_else(|| bad(&format!("missing selected member {path}")))
    }
    fn record_read(&mut self, read: PredicateRead) -> Result<(), ItemRefusal> {
        reserve(
            &mut self.state,
            format!("{read:?}").len() + 64,
            self.limits.max_state_bytes,
        )?;
        self.reads.push(read);
        Ok(())
    }
    fn selected(&mut self, path: &str) -> Result<Package, ItemRefusal> {
        metadata_path(path, false)?;
        let home = parent(path)?;
        let mut files = Package::new();
        let mut total = 0;
        for name in selected_names(path)? {
            if let Some(raw) = self.optional(&format!("{home}/{name}"), MAX_FILE)? {
                total += raw.len();
                if total > MAX_SIDE {
                    return Err(ItemRefusal::Budget);
                }
                files.insert(name, raw);
            }
        }
        if !files.contains_key(path.rsplit('/').next().unwrap_or("")) {
            return Err(bad("selected record absent"));
        }
        Ok(files)
    }
    fn transaction(&mut self, id: &str) -> Result<Transaction, ItemRefusal> {
        hash(id)?;
        if let Some(tx) = self.transactions.get(id) {
            return Ok(tx.clone());
        }
        let directory = format!("{TRANSACTIONS}/{}", hash(id)?);
        let raw = self.required(&format!("{directory}/manifest.json"), MAX_MANIFEST)?;
        let manifest = decode(&raw)?;
        keys(
            &manifest,
            &[
                "schema_version",
                "transaction_id",
                "base_publication",
                "plan",
                "parents",
            ],
        )?;
        if text(&manifest, "schema_version")? != "tos_selected_metadata_transaction_v1"
            || text(&manifest, "transaction_id")? != id
        {
            return Err(bad("Work Expression transaction grammar"));
        }
        let base = &manifest["base_publication"];
        keys(base, &["token", "generation"])?;
        let generation = integer(base, "generation")?;
        if generation > MAX_GENERATION - 2 || base["token"].is_null() != (generation == 0) {
            return Err(bad("publication predecessor"));
        }
        if !base["token"].is_null() {
            hash(text(base, "token")?)?;
        }
        let plan = &manifest["plan"];
        keys(plan, &["authorization", "files", "new_directories"])?;
        if !plan["authorization"].is_object() || canonical(&plan["authorization"])?.len() > 65536 {
            return Err(bad("bounded authorization"));
        }
        let directories = array(plan, "new_directories")?;
        if directories.len() > 64 {
            return Err(ItemRefusal::Budget);
        }
        let dirs: Vec<_> = directories
            .iter()
            .map(|v| v.as_str().ok_or_else(|| bad("new directory")))
            .collect::<Result<_, _>>()?;
        let mut sorted_dirs = dirs.clone();
        sorted_dirs.sort_by_key(|v| (v.split('/').count(), *v));
        if dirs != sorted_dirs || dirs.iter().collect::<BTreeSet<_>>().len() != dirs.len() {
            return Err(bad("new directory order/uniqueness"));
        }
        for d in &dirs {
            metadata_path(d, true)?;
        }
        let rows = array(plan, "files")?;
        if !(1..=64).contains(&rows.len()) {
            return Err(ItemRefusal::Budget);
        }
        let mut files = BTreeMap::new();
        let mut blobs = BTreeMap::<String, Vec<u8>>::new();
        let mut sides = [0usize; 2];
        let mut total = 0usize;
        let mut changed = false;
        let mut last = "";
        for row in rows {
            check(self.limits.deadline, self.cancelled)?;
            keys(row, &["path", "before", "after"])?;
            let path = text(row, "path")?;
            metadata_path(path, false)?;
            if path <= last {
                return Err(bad("file path order/uniqueness"));
            }
            last = path;
            if row["before"].is_null() && row["after"].is_null() {
                return Err(bad("absent to absent file"));
            }
            changed |= row["before"] != row["after"];
            let mut bytes = [None, None];
            for (i, side) in ["before", "after"].iter().enumerate() {
                let binding = &row[*side];
                if binding.is_null() {
                    continue;
                }
                keys(binding, &["sha256", "bytes"])?;
                let sha = text(binding, "sha256")?;
                hash(sha)?;
                let size =
                    usize::try_from(integer(binding, "bytes")?).map_err(|_| ItemRefusal::Budget)?;
                sides[i] = sides[i].checked_add(size).ok_or(ItemRefusal::Budget)?;
                if size > MAX_SIDE || sides[i] > MAX_SIDE {
                    return Err(ItemRefusal::Budget);
                }
                if !blobs.contains_key(sha) {
                    total = total.checked_add(size).ok_or(ItemRefusal::Budget)?;
                    if total > 2 * MAX_SIDE {
                        return Err(ItemRefusal::Budget);
                    }
                    let raw = self.required(&format!("{directory}/{}.blob", hash(sha)?), size)?;
                    if raw.len() != size || Digest256::of_bytes(&raw).to_prefixed() != sha {
                        return Err(bad("transaction blob fixity"));
                    }
                    blobs.insert(sha.into(), raw);
                }
                bytes[i] = Some(blobs[sha].clone());
            }
            files.insert(path.into(), (bytes[0].take(), bytes[1].take()));
        }
        if !changed {
            return Err(bad("transaction has no change"));
        }
        for a in files.keys() {
            for b in files.keys().map(String::as_str).chain(dirs.iter().copied()) {
                if a.as_str() != b && is_ancestor(a, b) {
                    return Err(bad("target ancestor collision"));
                }
            }
        }
        for d in &dirs {
            if !files
                .iter()
                .any(|(p, (before, _))| is_ancestor(d, p) && before.is_none())
            {
                return Err(bad("new directory lacks new file"));
            }
        }
        let mut parents = BTreeSet::from([HOME.to_owned()]);
        for p in files.keys().map(String::as_str).chain(dirs.iter().copied()) {
            let mut p = parent(p)?;
            while p == HOME || p.starts_with(&format!("{HOME}/")) {
                parents.insert(p.into());
                if p == HOME {
                    break;
                }
                p = parent(p)?;
            }
        }
        let bindings = manifest["parents"]
            .as_object()
            .ok_or_else(|| bad("parent closure"))?;
        if bindings.keys().cloned().collect::<BTreeSet<_>>() != parents {
            return Err(bad("parent directory closure"));
        }
        for (p, binding) in bindings {
            if binding.is_null() {
                if !dirs.contains(&p.as_str()) {
                    return Err(bad("undeclared absent parent"));
                }
            } else {
                keys(binding, &["device", "inode", "mode", "uid"])?;
                for k in ["device", "inode", "mode", "uid"] {
                    integer(binding, k)?;
                }
                let mode = integer(binding, "mode")?;
                if mode & 0o170000 != 0o040000 || mode & 0o022 != 0 {
                    return Err(bad("historical directory posture"));
                }
            }
        }
        let sha = Digest256::of_bytes(&raw).to_prefixed();
        let completion = match self.optional(&format!("{directory}/completion.json"), 8192)? {
            Some(raw) => {
                let v = decode(&raw)?;
                keys(&v, &["schema_version", "publication"])?;
                if text(&v, "schema_version")? != "tos_selected_metadata_completion_v1" {
                    return Err(bad("completion schema"));
                }
                state_valid(&v["publication"])?;
                let s = &v["publication"];
                if text(s, "phase")? != "ready"
                    || text(s, "transaction_id")? != id
                    || text(s, "manifest_sha256")? != sha
                    || integer(s, "generation")? != generation + 2
                {
                    return Err(bad("terminal completion binding"));
                }
                Some(s.clone())
            }
            None => None,
        };
        let selected = self
            .publication
            .as_ref()
            .is_some_and(|s| s["transaction_id"] == id);
        let status = if selected {
            let s = self.publication.as_ref().unwrap();
            if text(s, "manifest_sha256")? != sha {
                return Err(bad("current terminal transaction drift"));
            }
            if text(s, "phase")? == "pending" {
                if integer(s, "generation")? != generation + 1 || completion.is_some() {
                    return Err(bad("exact pending transaction binding"));
                }
                "pending".into()
            } else {
                if integer(s, "generation")? != generation + 2
                    || completion.as_ref().is_some_and(|c| c != s)
                {
                    return Err(bad("terminal generation/completion drift"));
                }
                text(s, "outcome")?.into()
            }
        } else {
            completion
                .as_ref()
                .map(|s| text(s, "outcome").map(str::to_owned))
                .transpose()?
                .unwrap_or("orphan".into())
        };
        reserve(
            &mut self.state,
            total
                .checked_mul(3)
                .ok_or(ItemRefusal::Budget)?
                .checked_add(canonical(&manifest)?.len() * 3)
                .ok_or(ItemRefusal::Budget)?,
            self.limits.max_state_bytes,
        )?;
        let tx = Transaction {
            manifest,
            manifest_sha256: sha,
            status,
            files,
        };
        self.transactions.insert(id.into(), tx.clone());
        Ok(tx)
    }
    fn archive(&mut self, path: &str, id: &str, receipt: &Value) -> Result<Package, ItemRefusal> {
        let rev = text(receipt, "previous_revision")?;
        let home = format!(
            "{HOME}/.record-revisions/{}-{}",
            Digest256::of_bytes(id.as_bytes()).to_hex(),
            hash(rev)?
        );
        if text(receipt, "archive_path")? != home {
            return Err(bad("archive exact locator"));
        }
        let raw = self.required(&format!("{home}/manifest.json"), MAX_FILE)?;
        let manifest = decode(&raw)?;
        let v2 = text(&manifest, "schema_version")? == "tos_source_package_archive_v2";
        let mut wanted = vec![
            "schema_version",
            "source_path",
            "source",
            "revision",
            "files",
        ];
        if v2 {
            wanted.push("publication_protocol");
        }
        keys(&manifest, &wanted)?;
        if !matches!(
            text(&manifest, "schema_version")?,
            "tos_source_package_archive_v1" | "tos_source_package_archive_v2"
        ) || v2 && text(&manifest, "publication_protocol")? != PROTOCOL
            || text(&manifest, "source_path")? != path
            || manifest["source"] != receipt["previous_source"]
            || manifest["revision"] != receipt["previous_revision"]
        {
            return Err(bad("archive metadata binding"));
        }
        let bindings = manifest["files"]
            .as_object()
            .ok_or_else(|| bad("archive files"))?;
        if bindings.len() > 64 {
            return Err(ItemRefusal::Budget);
        }
        let mut files = Package::new();
        let mut expected = BTreeSet::from(["manifest.json".to_owned()]);
        let mut total = raw.len();
        for (name, b) in bindings {
            check(self.limits.deadline, self.cancelled)?;
            keys(b, &["blob", "sha256", "bytes"])?;
            if name.is_empty() || name.contains('/') || matches!(name.as_str(), "." | "..") {
                return Err(bad("flat archived filename"));
            }
            let sha = text(b, "sha256")?;
            let blob = format!("{}.blob", hash(sha)?);
            if text(b, "blob")? != blob {
                return Err(bad("archive blob name"));
            }
            let size = usize::try_from(integer(b, "bytes")?).map_err(|_| ItemRefusal::Budget)?;
            let raw = self.required(&format!("{home}/{blob}"), size.min(MAX_FILE))?;
            total = total.checked_add(raw.len()).ok_or(ItemRefusal::Budget)?;
            if total > MAX_SIDE + MAX_FILE {
                return Err(ItemRefusal::Budget);
            }
            if raw.len() != size || Digest256::of_bytes(&raw).to_prefixed() != sha {
                return Err(bad("archive exact blob bytes"));
            }
            files.insert(name.clone(), raw);
            expected.insert(blob);
        }
        let prefix = format!("{home}/");
        let actual: BTreeSet<_> = self
            .paths
            .range(prefix.clone()..)
            .take_while(|p| p.starts_with(&prefix))
            .map(|p| p[prefix.len()..].to_owned())
            .collect();
        if actual != expected {
            return Err(bad("archive extra/nested/unbound files"));
        }
        let names = selected_names(path)?;
        if v2 && (files.keys().any(|n| !names.contains(n)) || !files.contains_key(&names[0])) {
            return Err(bad("selected archive package scope"));
        }
        if revision(&files)? != rev {
            return Err(bad("archive package revision"));
        }
        let old = decode(
            files
                .get(&names[0])
                .ok_or_else(|| bad("archive source missing"))?,
        )?;
        if reference(&old, "record_id", "record_version")? != receipt["previous_source"] {
            return Err(bad("archive previous source"));
        }
        if let Some(request) = receipt.get("request") {
            let mut revised = old.clone();
            let object = revised
                .as_object_mut()
                .ok_or_else(|| bad("source object"))?;
            for (k, v) in request["fields"]
                .as_object()
                .ok_or_else(|| bad("revision fields"))?
            {
                object.insert(k.clone(), v.clone());
            }
            object.insert(
                "record_version".into(),
                json!(
                    integer(&old, "record_version")?
                        .checked_add(1)
                        .ok_or(ItemRefusal::Budget)?
                ),
            );
            if reference(&revised, "record_id", "record_version")? != receipt["source"] {
                return Err(bad("retained request successor"));
            }
        }
        Ok(files)
    }
    pub(crate) fn finish(self) -> (u64, Vec<PredicateRead>) {
        (self.bytes, self.reads)
    }
}
fn state_valid(v: &Value) -> Result<(), ItemRefusal> {
    keys(
        v,
        &[
            "schema_version",
            "generation",
            "transition_id",
            "phase",
            "transaction_id",
            "manifest_sha256",
            "outcome",
            "recovery_authorization",
            "token",
        ],
    )?;
    let generation = integer(v, "generation")?;
    let transition = text(v, "transition_id")?;
    let phase = text(v, "phase")?;
    if text(v, "schema_version")? != "tos_source_metadata_publication_v1"
        || !(1..=MAX_GENERATION).contains(&generation)
        || transition.len() != 32
        || !transition
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        || !matches!(phase, "pending" | "ready")
        || phase == "pending" && (!v["outcome"].is_null() || !v["recovery_authorization"].is_null())
        || phase == "ready" && !matches!(text(v, "outcome")?, "committed" | "rolled-back")
    {
        return Err(bad("publication state grammar"));
    }
    for k in ["transaction_id", "manifest_sha256", "token"] {
        hash(text(v, k)?)?;
    }
    if !v["recovery_authorization"].is_null()
        && (!v["recovery_authorization"].is_object()
            || canonical(&v["recovery_authorization"])?.len() > 4096)
    {
        return Err(bad("recovery evidence bound"));
    }
    let mut contents = v.clone();
    contents.as_object_mut().unwrap().remove("token");
    if text(v, "token")? != digest(&contents)? {
        return Err(bad("publication token digest"));
    }
    Ok(())
}

fn request_valid(request: &Value) -> Result<(), ItemRefusal> {
    keys(
        request,
        &[
            "schema_version",
            "operation",
            "record",
            "claim",
            "forms",
            "expression_forms",
            "claim_forms",
            "reason",
            "command_id",
            "fields",
            "expected_configuration",
            "expected_source",
            "expected_revision",
            "expected_dependencies",
            "expected_publication",
        ],
    )?;
    if text(request, "schema_version")? != "tos_local_work_expression_command_v1"
        || text(request, "operation")? != "work.expression.create"
        || canonical(request)?.len() > 1_048_576
    {
        return Err(bad("Work Expression request grammar"));
    }
    let reason = tos_foundation::python_strip_unicode16_v1(text(request, "reason")?, MAX_SIDE)
        .map_err(|_| ItemRefusal::Budget)?;
    if reason.is_empty()
        || reason.chars().count() > 4096
        || !(1..=256).contains(&text(request, "command_id")?.chars().count())
    {
        return Err(bad("request reason/command bounds"));
    }
    for k in [
        "expected_configuration",
        "expected_revision",
        "expected_dependencies",
    ] {
        hash(text(request, k)?)?;
    }
    if !request["expected_publication"].is_null() {
        hash(text(request, "expected_publication")?)?;
    }
    keys(&request["fields"], &["expression_claim_refs"])?;
    Ok(())
}
fn transaction_id(request: &Value) -> Result<String, ItemRefusal> {
    digest(
        &json!({"operation":"work.expression.create","command_id":request["command_id"],"owner_configuration":request["expected_configuration"],"request_digest":digest(request)?}),
    )
}
fn work_parent_receipt(receipt: &Value) -> Result<(), ItemRefusal> {
    let request = &receipt["request"];
    request_valid(request)?;
    let publication = &receipt["publication"];
    keys(
        publication,
        &["protocol", "transaction_id", "selected_files"],
    )?;
    let refs = array(&request["fields"], "expression_claim_refs")?;
    if text(publication, "protocol")? != PROTOCOL
        || text(publication, "transaction_id")? != transaction_id(request)?
        || publication["selected_files"]
            != json!([
                "source-revision-history.json",
                "work.human-forms.json",
                "work.json"
            ])
        || receipt["changed_fields"] != json!(["expression_claim_refs"])
        || request["claim"]["predicate"] != "has_expression"
        || request["claim"]["subject_ref"] != receipt["previous_source"]["id"]
        || request["record"]["work_ref"] != receipt["previous_source"]["id"]
        || request["claim"]["object"] != request["record"]["record_id"]
        || refs.last() != request["claim"].get("claim_id")
    {
        return Err(bad("explicit compound parent lineage"));
    }
    Ok(())
}

/// Existing `_history` law over exact read-only packages. Callers own custody.
/// Other compound parent handlers remain explicit unsupported profiles.
pub fn inspect_record_history(
    files: &BTreeMap<String, Vec<u8>>,
    record_raw: &[u8],
    deadline: std::time::Instant,
    cancelled: &AtomicBool,
) -> Result<Value, ItemRefusal> {
    check(deadline, cancelled)?;
    let record = decode(record_raw)?;
    let subject = reference(&record, "record_id", "record_version")?;
    let history = match files.get(HISTORY) {
        Some(raw) => decode(raw)?,
        None => {
            json!({"schema_version":"tos_source_revision_history_v1","record_id":subject["id"],"receipts":[]})
        }
    };
    keys(&history, &["schema_version", "record_id", "receipts"])?;
    if !matches!(
        text(&history, "schema_version")?,
        "tos_source_revision_history_v1" | "tos_source_revision_history_v2"
    ) || history["record_id"] != subject["id"]
    {
        return Err(bad("history subject/version"));
    }
    let receipts = array(&history, "receipts")?;
    if receipts.len() > MAX_HISTORY || files.contains_key(HISTORY) && receipts.is_empty() {
        return Err(bad("stored history capacity/empty chain"));
    }
    let mut commands = BTreeSet::new();
    let mut previous = None;
    for receipt in receipts {
        check(deadline, cancelled)?;
        let selected = receipt.get("publication").is_some();
        let mut fields = vec![
            "command_id",
            "request_digest",
            "principal_id",
            "authority_ref",
            "owner_configuration",
            "recorded_at",
            "reason",
            "previous_source",
            "source",
            "previous_revision",
            "archive_path",
            "dependencies",
            "changed_fields",
            "forms",
            "grants_admission",
            "request",
        ];
        if selected {
            fields.push("publication");
        }
        keys(receipt, &fields)?;
        if selected {
            if text(&history, "schema_version")? != "tos_source_revision_history_v2" {
                return Err(bad("selected history v2 required"));
            }
            let p = &receipt["publication"];
            keys(p, &["protocol", "transaction_id", "selected_files"])?;
            let names = array(p, "selected_files")?;
            if text(p, "protocol")? != PROTOCOL
                || !p["transaction_id"].is_string()
                || names.len() != 3
                || names
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<BTreeSet<_>>()
                    .len()
                    != 3
                || !names.iter().any(|n| n == HISTORY)
                || names.iter().any(|n| {
                    n.as_str()
                        .is_none_or(|s| s.contains('/') || s.is_empty() || matches!(s, "." | ".."))
                })
            {
                return Err(bad("selected history publication binding"));
            }
        }
        crate::retirement_rules::observed_instant_order(
            text(receipt, "recorded_at")?,
            text(receipt, "recorded_at")?,
        )
        .map_err(|_| bad("history aware instant"))?;
        let request = &receipt["request"];
        match text(request, "operation")? {
            "work.expression.create" => work_parent_receipt(receipt)?,
            "record.revise" => {}
            other => {
                return Err(ItemRefusal::Unsupported(format!(
                    "retained compound parent handler {other}"
                )));
            }
        }
        let fields = request["fields"]
            .as_object()
            .ok_or_else(|| bad("retained request fields"))?;
        if !commands.insert(text(receipt, "command_id")?.to_owned())
            || text(receipt, "request_digest")? != digest(request)?
            || receipt["command_id"] != request["command_id"]
            || receipt["previous_source"] != request["expected_source"]
            || receipt["previous_revision"] != request["expected_revision"]
            || receipt["owner_configuration"] != request["expected_configuration"]
            || receipt["dependencies"] != request["expected_dependencies"]
            || receipt["reason"] != request["reason"]
            || receipt["changed_fields"] != json!(fields.keys().collect::<Vec<_>>())
            || receipt["grants_admission"] != false
            || receipt["source"]["id"] != subject["id"]
            || receipt["previous_source"]["id"] != subject["id"]
            || integer(&receipt["source"], "version")?
                != integer(&receipt["previous_source"], "version")?
                    .checked_add(1)
                    .ok_or(ItemRefusal::Budget)?
            || previous.is_some_and(|p| receipt.get("previous_source") != Some(p))
        {
            return Err(bad("broken source revision chain"));
        }
        previous = receipt.get("source");
    }
    if previous.is_some_and(|p| p != &subject) {
        return Err(bad("current source differs from history head"));
    }
    check(deadline, cancelled)?;
    Ok(history)
}

impl WorkExpression<'_> {
    fn history(&mut self, path: &str, files: &Package) -> Result<Value, ItemRefusal> {
        check(self.limits.deadline, self.cancelled)?;
        // Bind memoized lineage to the whole selected package, not its subject
        // alone: source-copy forms and history bytes participate in revision.
        let key = (path.to_owned(), revision(files)?);
        if let Some(history) = self.histories.get(&key) {
            return Ok(history.clone());
        }
        let name = path
            .rsplit('/')
            .next()
            .ok_or_else(|| bad("record basename"))?;
        let raw = files
            .get(name)
            .ok_or_else(|| bad("history source absent"))?;
        let record = decode(raw)?;
        let id = text(&record, "record_id")?;
        let history = inspect_record_history(files, raw, self.limits.deadline, self.cancelled)?;
        for (index, receipt) in array(&history, "receipts")?.iter().enumerate() {
            check(self.limits.deadline, self.cancelled)?;
            let archived = self.archive(path, id, receipt)?;
            let predecessor = inspect_record_history(
                &archived,
                archived
                    .get(name)
                    .ok_or_else(|| bad("archive source absent"))?,
                self.limits.deadline,
                self.cancelled,
            )?;
            if array(&predecessor, "receipts")? != &array(&history, "receipts")?[..index] {
                return Err(bad("retained predecessor receipt prefix"));
            }
        }
        reserve(
            &mut self.state,
            canonical(&history)?
                .len()
                .checked_mul(3)
                .and_then(|n| n.checked_add(key.0.len() + key.1.len() + 128))
                .ok_or(ItemRefusal::Budget)?,
            self.limits.max_state_bytes,
        )?;
        self.histories.insert(key, history.clone());
        Ok(history)
    }
}

const SCOPE_KEYS: [&str; 9] = [
    "work_id",
    "work_source_path",
    "expression_id",
    "expression_source_path",
    "claim_id",
    "provenance_event_id",
    "allowed_work_form_ids",
    "allowed_expression_form_ids",
    "allowed_claim_form_ids",
];
fn typed_id(id: &str, kind: &str) -> bool {
    id.strip_prefix(&format!("tos.{kind}."))
        .is_some_and(segment)
}
fn segment(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'-'))
        && s.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric)
        && s.as_bytes().last().is_some_and(u8::is_ascii_alphanumeric)
        && !s
            .as_bytes()
            .windows(2)
            .any(|p| matches!(p[0], b'.' | b'-') && matches!(p[1], b'.' | b'-'))
}
fn scope_valid(scope: &Value, request: &Value, authority: &Value) -> Result<(), ItemRefusal> {
    keys(scope, &SCOPE_KEYS)?;
    for (k, kind) in [
        ("work_id", "work"),
        ("expression_id", "expression"),
        ("claim_id", "claim"),
        ("provenance_event_id", "event"),
    ] {
        if !typed_id(text(scope, k)?, kind) {
            return Err(bad("typed compound scope identities"));
        }
    }
    let work = text(scope, "work_source_path")?;
    let expression = text(scope, "expression_source_path")?;
    metadata_path(work, false)?;
    metadata_path(expression, false)?;
    if !work.starts_with("ToS/source-witnesses/works/")
        || work.split('/').count() < 5
        || !work.ends_with("/work.json")
        || !expression.ends_with("/expression.json")
    {
        return Err(bad("Work Expression home grammar"));
    }
    let child = parent(expression)?;
    let expected = format!("{}/expressions/", parent(work)?);
    if !child
        .strip_prefix(&expected)
        .is_some_and(|s| !s.contains('/') && segment(s))
    {
        return Err(bad("one exact child home"));
    }
    let mut seen = BTreeSet::new();
    for (field, allowed) in [
        ("forms", "allowed_work_form_ids"),
        ("expression_forms", "allowed_expression_form_ids"),
        ("claim_forms", "allowed_claim_form_ids"),
    ] {
        let ids = array(scope, allowed)?;
        if !(1..=32).contains(&ids.len()) {
            return Err(bad("form identity bounds"));
        }
        let mut grant = BTreeSet::new();
        for id in ids {
            let id = id.as_str().ok_or_else(|| bad("form identity"))?;
            let tail = id
                .strip_prefix("tos.form.")
                .ok_or_else(|| bad("form identity prefix"))?;
            if tail.is_empty()
                || !tail.as_bytes()[0].is_ascii_alphanumeric()
                || !tail.bytes().all(|c| {
                    c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'-' | b'_')
                })
                || !seen.insert(id)
                || !grant.insert(id)
            {
                return Err(bad("distinct subject form identities"));
            }
        }
        let selections = array(request, field)?;
        if !(1..=32).contains(&selections.len()) {
            return Err(bad("explicit form selections"));
        }
        let mut selected = BTreeSet::new();
        for item in selections {
            keys(item, &["form_id", "field_id"])?;
            if !grant.contains(text(item, "form_id")?) || !selected.insert(text(item, "form_id")?) {
                return Err(bad("form selection exceeds retained scope"));
            }
            text(item, "field_id")?;
        }
    }
    let claim = &request["claim"];
    let record = &request["record"];
    if record["record_id"] != scope["expression_id"]
        || record["work_ref"] != scope["work_id"]
        || claim["claim_id"] != scope["claim_id"]
        || claim["subject_ref"] != scope["work_id"]
        || claim["object"] != scope["expression_id"]
        || claim["provenance_event_ref"] != scope["provenance_event_id"]
        || claim["maker"]
            != json!({"maker_type":authority["maker_type"],"agent_ref":authority["principal_id"]})
    {
        return Err(bad("exact scope/request endpoints and maker"));
    }
    Ok(())
}
fn j(value: &Value) -> Result<JsonValue, ItemRefusal> {
    ordered(&serde_json::to_vec(value).map_err(|_| bad("JSON value encoding"))?)
}
fn object(fields: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        fields
            .into_iter()
            .map(|(k, v)| (tos_foundation::JsonString::from_utf8(k), v))
            .collect(),
    )
}
fn string(s: &str) -> JsonValue {
    JsonValue::String(tos_foundation::JsonString::from_utf8(s))
}
fn set(value: &mut JsonValue, key: &str, new: JsonValue) -> Result<(), ItemRefusal> {
    let JsonValue::Object(fields) = value else {
        return Err(bad("ordered object required"));
    };
    if let Some((_, old)) = fields.iter_mut().find(|(k, _)| k.as_str() == Some(key)) {
        *old = new;
    } else {
        fields.push((tos_foundation::JsonString::from_utf8(key), new));
    }
    Ok(())
}
fn ref_ordered(v: &Value, id: &str, version: &str) -> Result<JsonValue, ItemRefusal> {
    let r = reference(v, id, version)?;
    Ok(object(vec![
        ("id", j(&r["id"])?),
        ("version", j(&r["version"])?),
        ("digest", j(&r["digest"])?),
    ]))
}
fn refs_ordered(files: &[(String, Vec<u8>)]) -> JsonValue {
    JsonValue::Object(
        files
            .iter()
            .map(|(name, raw)| {
                (
                    tos_foundation::JsonString::from_utf8(name),
                    object(vec![
                        ("sha256", string(&Digest256::of_bytes(raw).to_prefixed())),
                        ("bytes", j(&json!(raw.len())).expect("bounded byte length")),
                    ]),
                )
            })
            .collect(),
    )
}
fn forms(
    source: &JsonValue,
    previous: Option<&JsonValue>,
    selections: &Value,
    principal: &str,
    claim: bool,
) -> Result<(JsonValue, JsonValue), ItemRefusal> {
    use crate::source_forms::source_copy_kernel as kernel;
    let fail = |e| ItemRefusal::Unsupported(format!("compound source-copy forms: {e:?}"));
    let selections = selections
        .as_array()
        .ok_or_else(|| bad("form selections"))?;
    if let Some(previous) = previous {
        let old = previous
            .object_get("forms")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| bad("previous forms"))?;
        for form in old {
            let id = form
                .object_get("form_id")
                .and_then(JsonValue::as_str)
                .ok_or_else(|| bad("previous form id"))?;
            if !selections.iter().any(|s| s["form_id"] == id)
                || form
                    .object_get("content")
                    .and_then(|v| v.object_get("kind"))
                    .and_then(JsonValue::as_str)
                    != Some("source-copy")
            {
                return Err(bad("parent explicitly rebinds all source-copy forms"));
            }
        }
    }
    let mut changes = Vec::new();
    for selection in selections {
        changes.push(
            kernel::prepare_form_change(
                source,
                previous,
                principal,
                text(selection, "form_id")?,
                text(selection, "field_id")?,
            )
            .map_err(fail)?,
        );
    }
    let subject = kernel::metadata_subject(source).map_err(fail)?;
    let result = kernel::apply_form_changes(previous, &subject, &changes).map_err(fail)?;
    let views = kernel::materialize_source_forms(source, &result).map_err(fail)?;
    if !views
        .iter()
        .all(|v| v.object_get("state").and_then(JsonValue::as_str) == Some("ready"))
        || !views.iter().any(|v| {
            v.object_get("role").and_then(JsonValue::as_str)
                == Some(if claim { "statement" } else { "name" })
        })
    {
        return Err(bad("ready source copies require name/statement"));
    }
    let refs = changes
        .iter()
        .map(|c| {
            kernel::form_reference(
                c.object_get("form")
                    .ok_or_else(|| bad("prepared form missing"))?,
            )
            .map_err(fail)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((result, JsonValue::Array(refs)))
}

struct Reconstructed {
    scope: Value,
    request: Value,
    parent_receipt: Value,
    child: Package,
    receipt: Value,
}
impl WorkExpression<'_> {
    fn reconstruct(
        &mut self,
        tx: &Transaction,
        schemas: &mut CutWorkerSchemaExecutor,
    ) -> Result<Reconstructed, ItemRefusal> {
        let plan = &tx.manifest["plan"];
        let transient = tx
            .files
            .values()
            .try_fold(0usize, |sum, (a, b)| {
                sum.checked_add(a.as_ref().map_or(0, Vec::len))?
                    .checked_add(b.as_ref().map_or(0, Vec::len))
            })
            .and_then(|n| n.checked_mul(12))
            .ok_or(ItemRefusal::Budget)?;
        if self
            .state
            .checked_add(transient)
            .is_none_or(|n| n > self.limits.max_state_bytes)
        {
            return Err(ItemRefusal::Budget);
        }
        let authority = &plan["authorization"];
        keys(
            authority,
            &[
                "schema_version",
                "scope",
                "principal_id",
                "maker_type",
                "authority_ref",
                "owner_configuration",
                "command_id",
                "request_digest",
                "dependency_bindings",
            ],
        )?;
        if text(authority, "schema_version")? != "tos_work_expression_authorization_v1" {
            return Err(bad("native Work Expression authorization profile"));
        }
        let scope = &authority["scope"];
        let work_path = text(scope, "work_source_path")?;
        let expression_path = text(scope, "expression_source_path")?;
        let home = parent(expression_path)?;
        let work_home = parent(work_path)?;
        let after = |name: &str| {
            tx.files
                .get(&format!("{home}/{name}"))
                .and_then(|v| v.1.as_ref())
                .cloned()
                .ok_or_else(|| bad("missing compound after buffer"))
        };
        let request_raw = after("source-create-request.json")?;
        let request = decode(&request_raw)?;
        request_valid(&request)?;
        scope_valid(scope, &request, authority)?;
        let environment_raw = after("source-create-environment.json")?;
        let environment = decode(&environment_raw)?;
        keys(
            &environment,
            &[
                "runtime",
                "runtime_version",
                "runtime_artifact_sha256",
                "backend",
                "hardware_target",
                "unicode_version",
                "argv_sha256",
            ],
        )?;
        for value in environment.as_object().unwrap().values() {
            if value.as_str().is_none_or(str::is_empty) {
                return Err(bad("retained environment fields"));
            }
        }
        for k in ["runtime_artifact_sha256", "argv_sha256"] {
            hash(&format!("sha256:{}", text(&environment, k)?))?;
        }
        let receipt_raw = after("work-expression-receipt.json")?;
        let actual_receipt = decode(&receipt_raw)?;
        let recorded_at = text(&actual_receipt, "recorded_at")?;
        crate::retirement_rules::observed_instant_order(recorded_at, recorded_at)
            .map_err(|_| bad("compound recorded aware instant"))?;
        let mut before = Package::new();
        for name in selected_names(work_path)? {
            if let Some(raw) = tx
                .files
                .get(&format!("{work_home}/{name}"))
                .and_then(|s| s.0.clone())
            {
                before.insert(name, raw);
            }
        }
        let old_raw = before
            .get("work.json")
            .ok_or_else(|| bad("retained parent input missing"))?;
        let old = decode(old_raw)?;
        if reference(&old, "record_id", "record_version")? != request["expected_source"]
            || revision(&before)? != text(&request, "expected_revision")?
            || authority["owner_configuration"] != request["expected_configuration"]
            || authority["command_id"] != request["command_id"]
            || text(authority, "request_digest")? != digest(&request)?
            || digest(&authority["dependency_bindings"])?
                != text(&request, "expected_dependencies")?
        {
            return Err(bad("retained authorization/request/before binding"));
        }
        let dirs = array(plan, "new_directories")?;
        if *dirs != vec![json!(home)] && *dirs != vec![json!(parent(home)?), json!(home)] {
            return Err(bad("exact new Expression directories"));
        }
        let history = self.history(work_path, &before)?;
        if array(&history, "receipts")?.len() >= MAX_HISTORY {
            return Err(bad("parent history capacity"));
        }
        let mut revised = old.clone();
        let map = revised
            .as_object_mut()
            .ok_or_else(|| bad("parent object"))?;
        for (k, v) in request["fields"].as_object().unwrap() {
            map.insert(k.clone(), v.clone());
        }
        map.insert(
            "record_version".into(),
            json!(
                integer(&old, "record_version")?
                    .checked_add(1)
                    .ok_or(ItemRefusal::Budget)?
            ),
        );
        let mut revised_ordered = ordered(old_raw)?;
        set(
            &mut revised_ordered,
            "expression_claim_refs",
            j(&request["fields"]["expression_claim_refs"])?,
        )?;
        set(
            &mut revised_ordered,
            "record_version",
            j(&revised["record_version"])?,
        )?;
        let expression = &request["record"];
        let claim = &request["claim"];
        let expression_ordered = ordered(&request_raw)?
            .object_get("record")
            .ok_or_else(|| bad("ordered expression"))?
            .clone();
        let claim_ordered = ordered(&request_raw)?
            .object_get("claim")
            .ok_or_else(|| bad("ordered Claim"))?
            .clone();
        let parent_raw = pretty(&revised_ordered)?;
        let expression_raw = pretty(&expression_ordered)?;
        let mut claim_raw = canonical_ordered(&claim_ordered)?;
        claim_raw.push(b'\n');
        let delta = crate::biblio_rules::inspect_bibliographic_delta(
            crate::biblio_rules::BiblioDeltaInput {
                parent_path: work_path,
                parent_before_raw: old_raw,
                parent_after_raw: &parent_raw,
                endpoint_path: expression_path,
                endpoint_raw: &expression_raw,
                claim_path: &format!("{home}/source-claims.jsonl"),
                claim_raw: &claim_raw,
            },
            self.limits,
            self.cancelled,
            schemas,
        )?;
        if !delta.issues.is_empty() {
            return Err(bad("Work Expression append/delta mechanics"));
        }
        for read in delta.reads {
            self.record_read(read)?;
        }
        if !text(expression, "language").is_ok_and(|v| !v.is_empty())
            || !text(expression, "expression_role").is_ok_and(|v| !v.is_empty())
            || ["variant_labels", "external_identifiers"].iter().any(|k| {
                !expression[*k]
                    .as_array()
                    .is_some_and(|a| a.iter().all(|v| v["status"] == "unverified"))
            })
        {
            return Err(bad("Expression language/role/unverified variants"));
        }
        let mut local_limits = self.limits;
        local_limits.max_state_bytes = self
            .limits
            .max_state_bytes
            .checked_sub(self.state)
            .and_then(|n| n.checked_sub(transient))
            .ok_or(ItemRefusal::Budget)?;
        local_limits.max_total_bytes = self
            .limits
            .max_total_bytes
            .checked_sub(self.bytes)
            .ok_or(ItemRefusal::Budget)?;
        let local = crate::record_rules::validate_source_claim_from_cut(
            self.cut,
            &claim_raw,
            schemas,
            local_limits,
            self.cancelled,
        )?;
        if !local.issues.is_empty() {
            return Err(bad("compound Claim local owner profile"));
        }
        for (path, sha) in local.dependency_digests {
            let relative = RelativePath::parse(&path).map_err(|_| bad("Claim contract path"))?;
            let size = self
                .cut
                .current()
                .member(&relative)
                .ok_or_else(|| bad("Claim dependency membership"))?
                .size_bytes;
            account(
                &mut self.bytes,
                usize::try_from(size).map_err(|_| ItemRefusal::Budget)?,
                self.limits.max_total_bytes,
            )?;
            self.record_read(PredicateRead::ExactPath {
                path,
                digest: sha.to_prefixed(),
            })?;
        }
        let form_name = "work.human-forms.json";
        let prior_forms = before.get(form_name).map(|v| ordered(v)).transpose()?;
        let principal = text(authority, "principal_id")?;
        let (parent_forms, parent_refs) = forms(
            &revised_ordered,
            prior_forms.as_ref(),
            &request["forms"],
            principal,
            false,
        )?;
        let (expression_forms, expression_refs) = forms(
            &expression_ordered,
            None,
            &request["expression_forms"],
            principal,
            false,
        )?;
        let (claim_forms, claim_refs) = forms(
            &claim_ordered,
            None,
            &request["claim_forms"],
            principal,
            true,
        )?;
        let id = transaction_id(&request)?;
        if id != tx.manifest["transaction_id"] {
            return Err(bad("compound transaction request identity"));
        }
        let archive_path = format!(
            "{HOME}/.record-revisions/{}-{}",
            Digest256::of_bytes(text(scope, "work_id")?.as_bytes()).to_hex(),
            hash(text(&request, "expected_revision")?)?
        );
        let parent_receipt_ordered = object(vec![
            ("command_id", j(&request["command_id"])?),
            ("request_digest", string(&digest(&request)?)),
            ("principal_id", j(&authority["principal_id"])?),
            ("authority_ref", j(&authority["authority_ref"])?),
            (
                "owner_configuration",
                j(&request["expected_configuration"])?,
            ),
            ("recorded_at", string(recorded_at)),
            ("reason", j(&request["reason"])?),
            (
                "previous_source",
                ref_ordered(&old, "record_id", "record_version")?,
            ),
            (
                "source",
                ref_ordered(&revised, "record_id", "record_version")?,
            ),
            ("previous_revision", j(&request["expected_revision"])?),
            ("archive_path", string(&archive_path)),
            ("dependencies", j(&request["expected_dependencies"])?),
            ("changed_fields", j(&json!(["expression_claim_refs"]))?),
            ("forms", parent_refs.clone()),
            ("grants_admission", JsonValue::Bool(false)),
            ("request", ordered(&request_raw)?),
            (
                "publication",
                object(vec![
                    ("protocol", string(PROTOCOL)),
                    ("transaction_id", string(&id)),
                    (
                        "selected_files",
                        j(&json!([HISTORY, "work.human-forms.json", "work.json"]))?,
                    ),
                ]),
            ),
        ]);
        let parent_receipt = decode(&canonical_ordered(&parent_receipt_ordered)?)?;
        work_parent_receipt(&parent_receipt)?;
        let mut history_ordered = match before.get(HISTORY) {
            Some(raw) => ordered(raw)?,
            None => object(vec![
                ("schema_version", string("tos_source_revision_history_v1")),
                ("record_id", j(&scope["work_id"])?),
                ("receipts", JsonValue::Array(vec![])),
            ]),
        };
        set(
            &mut history_ordered,
            "schema_version",
            string("tos_source_revision_history_v2"),
        )?;
        let mut receipts = history_ordered
            .object_get("receipts")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| bad("ordered receipt chain"))?
            .to_vec();
        receipts.push(parent_receipt_ordered.clone());
        set(&mut history_ordered, "receipts", JsonValue::Array(receipts))?;
        let parent_files: Vec<(String, Vec<u8>)> = vec![
            ("work.json".into(), parent_raw),
            (form_name.into(), pretty(&parent_forms)?),
            (HISTORY.into(), pretty(&history_ordered)?),
        ];
        let claim_form_name = format!(
            "source-claims.{}.human-forms.json",
            Digest256::of_bytes(text(scope, "claim_id")?.as_bytes()).to_hex()
        );
        let mut child_files: Vec<(String, Vec<u8>)> = vec![
            ("expression.json".into(), expression_raw),
            (
                "expression.human-forms.json".into(),
                pretty(&expression_forms)?,
            ),
            ("source-claims.jsonl".into(), claim_raw),
            (claim_form_name, pretty(&claim_forms)?),
        ];
        let outputs: Vec<_> = parent_files
            .iter()
            .map(|(n, r)| (format!("{work_home}/{n}"), r.clone()))
            .chain(
                child_files
                    .iter()
                    .map(|(n, r)| (format!("{home}/{n}"), r.clone())),
            )
            .collect();
        let event = compound_event(
            scope,
            &request,
            &before,
            &outputs,
            &environment,
            &authority["dependency_bindings"],
            recorded_at,
        )?;
        let mut event_raw = canonical(&event)?;
        event_raw.push(b'\n');
        if !schemas.check(
            &format!("{home}/source-create-provenance.jsonl"),
            &event_raw,
            "ToS/contracts/provenance-event-v2.schema.json",
            self.limits.deadline,
            self.cancelled,
        )? {
            return Err(bad("reconstructed provenance schema"));
        }
        for raw in [&parent_forms, &expression_forms, &claim_forms] {
            if !schemas.check(
                "compound-reconstructed-human-form-set",
                &canonical_ordered(raw)?,
                "ToS/contracts/human-form-set.schema.json",
                self.limits.deadline,
                self.cancelled,
            )? {
                return Err(bad("compound forms schema"));
            }
        }
        child_files.extend([
            ("source-create-request.json".into(), {
                let mut r = canonical(&request)?;
                r.push(b'\n');
                r
            }),
            ("source-create-environment.json".into(), {
                let mut r = canonical(&environment)?;
                r.push(b'\n');
                r
            }),
            ("source-create-provenance.jsonl".into(), event_raw),
        ]);
        let files: Vec<_> = parent_files
            .iter()
            .map(|(n, r)| (format!("{work_home}/{n}"), r.clone()))
            .chain(
                child_files
                    .iter()
                    .map(|(n, r)| (format!("{home}/{n}"), r.clone())),
            )
            .collect();
        let before_refs: Vec<_> = selected_names(work_path)?
            .iter()
            .filter_map(|n| before.get(n).map(|r| (n.clone(), r.clone())))
            .collect();
        let receipt_ordered = object(vec![
            ("schema_version", string("tos_work_expression_receipt_v1")),
            ("operation", string("work.expression.create")),
            ("transaction_id", string(&id)),
            ("command_id", j(&request["command_id"])?),
            ("request_digest", string(&digest(&request)?)),
            ("principal_id", j(&authority["principal_id"])?),
            ("authority_ref", j(&authority["authority_ref"])?),
            (
                "owner_configuration",
                j(&request["expected_configuration"])?,
            ),
            ("recorded_at", string(recorded_at)),
            ("scope", j(scope)?),
            ("dependencies", j(&request["expected_dependencies"])?),
            (
                "parent_before",
                ref_ordered(&old, "record_id", "record_version")?,
            ),
            (
                "parent_after",
                ref_ordered(&revised, "record_id", "record_version")?,
            ),
            ("parent_revision", j(&request["expected_revision"])?),
            ("parent_archive_ref", string(&archive_path)),
            (
                "parent_transition_sha256",
                string(
                    &Digest256::of_bytes(&canonical_ordered(&parent_receipt_ordered)?)
                        .to_prefixed(),
                ),
            ),
            ("parent_before_files", refs_ordered(&before_refs)),
            (
                "expression",
                ref_ordered(expression, "record_id", "record_version")?,
            ),
            ("claim", ref_ordered(claim, "claim_id", "claim_version")?),
            (
                "forms",
                object(vec![
                    ("work", parent_refs),
                    ("expression", expression_refs),
                    ("claim", claim_refs),
                ]),
            ),
            ("files", refs_ordered(&files)),
            ("grants_admission", JsonValue::Bool(false)),
        ]);
        let expected_raw = pretty(&receipt_ordered)?;
        let receipt = decode(&expected_raw)?;
        if actual_receipt != receipt || receipt_raw != expected_raw {
            return Err(bad("exact reconstructed compound receipt bytes"));
        }
        child_files.push(("work-expression-receipt.json".into(), expected_raw));
        if parent_files
            .iter()
            .chain(child_files.iter())
            .any(|(_, raw)| raw.len() > MAX_FILE)
        {
            return Err(ItemRefusal::Budget);
        }
        let expected: BTreeMap<_, _> = parent_files
            .into_iter()
            .map(|(n, r)| {
                (
                    format!("{work_home}/{n}"),
                    (before.get(&n).cloned(), Some(r)),
                )
            })
            .chain(
                child_files
                    .iter()
                    .map(|(n, r)| (format!("{home}/{n}"), (None, Some(r.clone())))),
            )
            .collect();
        if tx.files != expected {
            return Err(bad("exact whole retained before/after plan"));
        }
        if self.archive(work_path, text(scope, "work_id")?, &parent_receipt)? != before {
            return Err(bad("parent archive versus transaction inputs"));
        }
        Ok(Reconstructed {
            scope: scope.clone(),
            request,
            parent_receipt,
            child: child_files.into_iter().collect(),
            receipt,
        })
    }
}

fn compound_event(
    scope: &Value,
    request: &Value,
    before: &Package,
    outputs: &[(String, Vec<u8>)],
    environment: &Value,
    dependencies: &Value,
    recorded_at: &str,
) -> Result<Value, ItemRefusal> {
    const MODULE: &str =
        "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_expression_commands.py";
    let home = parent(text(scope, "expression_source_path")?)?;
    let request_ref = format!("{home}/source-create-request.json");
    let environment_ref = format!("{home}/source-create-environment.json");
    let mut request_raw = canonical(request)?;
    request_raw.push(b'\n');
    let mut environment_raw = canonical(environment)?;
    environment_raw.push(b'\n');
    let archive = format!(
        "{HOME}/.record-revisions/{}-{}",
        Digest256::of_bytes(text(scope, "work_id")?.as_bytes()).to_hex(),
        hash(text(request, "expected_revision")?)?
    );
    let prior: BTreeMap<_, _> = before
        .values()
        .map(|raw| {
            (
                format!("{archive}/{}.blob", Digest256::of_bytes(raw).to_hex()),
                raw,
            )
        })
        .collect();
    let output: BTreeMap<_, _> = outputs.iter().map(|(p, r)| (p, r)).collect();
    let entity = |reference: &str, raw: &[u8], role: &str| json!({"entity_ref":reference,"role":role,"sha256":Digest256::of_bytes(raw).to_hex(),"size_bytes":raw.len(),"media_type":if reference.ends_with(".jsonl"){"application/x-ndjson"}else{"application/json"},"availability":"owner_local","content_disclosure":"public_metadata_only","fixity_verified":false,"fixity_verified_at":null});
    let mut inputs = vec![entity(
        &request_ref,
        &request_raw,
        "caller-supplied-metadata-request",
    )];
    inputs.extend(
        prior
            .iter()
            .map(|(p, r)| entity(p, r, "retained-parent-metadata-input")),
    );
    let script = text(&dependencies["implementation"], MODULE)?;
    hash(&format!("sha256:{script}"))?;
    let mut env = environment.clone();
    env.as_object_mut().unwrap().remove("argv_sha256");
    env.as_object_mut().unwrap().insert(
        "environment_profile_binding".into(),
        json!({"ref":environment_ref,"sha256":Digest256::of_bytes(&environment_raw).to_hex()}),
    );
    let derivation =
        text(scope, "provenance_event_id")?.replacen("tos.event.", "tos.derivation.", 1);
    let output_bytes = output
        .values()
        .try_fold(0usize, |sum, raw| sum.checked_add(raw.len()))
        .ok_or(ItemRefusal::Budget)?;
    Ok(json!({
        "$schema":"https://tree-of-sophia.local/ToS/contracts/provenance-event-v2.schema.json","schema_version":"tos_provenance_event_v2","event_id":scope["provenance_event_id"],"event_version":1,"supersedes_event_ref":null,
        "record_binding":{"manifest_ref":format!("{home}/work-expression-receipt.json"),"digest_algorithm":"sha256","digest_scope":"exact_event_record_bytes"},
        "activity":{"event_type":"annotation","started_at":recorded_at,"ended_at":recorded_at,"status":"completed_with_warnings","terminal_reason":null,"exit_code":0,"warnings":["Captured prepared metadata buffers; the committed transaction is a separate verification.","Observed denotes the declared record link, not accepted bibliographic or textual truth."]},
        "entities":{"inputs":inputs,"outputs":output.iter().map(|(p,r)|entity(p,r,"prepared-compound-source-metadata")).collect::<Vec<_>>(),"byproducts":[entity(&environment_ref,&environment_raw,"runtime-description")]},
        "derivations":output.keys().enumerate().map(|(index,p)|json!({"derivation_id":format!("{derivation}.output-{index}"),"input_entity_ref":request_ref,"output_entity_ref":p,"relation":"was_derived_from","influence_asserted":true,"description":"Technical source metadata serialization; no historical influence or textual identity is asserted."})).collect::<Vec<_>>(),
        "responsibility":[{"agent_ref":"software:tos-source-expression-commands","agent_kind":"software","role":"executor","responsibility_posture":"performed","evidence_binding":{"ref":MODULE,"sha256":script},"human_evidence_status":"not_applicable"}],
        "method":{"procedure":{"name":"native-work-expression-metadata-serialization","version":"1","purpose":"Serialize one declared parent link and explicit source-copy forms without judging their content."},"command_capture":{"disclosure":"withheld_digest_only","argv":null,"argv_sha256":environment["argv_sha256"],"withholding_reason":"Process arguments may contain a private owner-configuration path."},"configuration_binding":{"ref":request_ref,"sha256":Digest256::of_bytes(&request_raw).to_hex()},"software_components":[{"name":"ToS native Work Expression adapter","version":"1","role":"serialization-runner","artifact_ref":MODULE,"artifact_sha256":script,"verification_status":"verified"}],"model_invocations":[],"environment":env},
        "manual_changes":{"status":"none_declared","change_receipts":[],"statement":"Caller authorship precedes this operation; no manual edits are performed inside serialization."},
        "measurements":[{"metric":"output_bytes","status":"measured","value":output_bytes,"unit":"bytes","method":"Sum of prepared source record, form and parent history buffers; excludes capture and receipt.","evidence_binding":null}],
        "evidence_authentication":{"capture_posture":"tool_captured","signature_status":"unsigned","signature_bindings":[],"verification_status":"unverified","producer_control_boundary":"The same unsigned local process serializes and records; hashes do not authenticate execution truth."},
        "rights_and_visibility":{"rights_record_bindings":[],"intended_uses":["local_research","public_metadata"],"content_visibility":"tracked_public_metadata","publication_authorized":false,"publication_authority_bindings":[]},
        "review_and_authority":{"mechanical_validation":"not_run","human_review_status":"not_performed","review_bindings":[],"accepted_uses":[],"promotion_authorized":false,"competence_evidence_bindings":[]},
        "reproducibility":{"classification":"partially_specified","known_gaps":["Upstream research, source reading and model invocations are outside this operation.","Runtime metadata is captured, not a complete archived execution environment."],"replay_scope":"Exact retained request, metadata and source-copy buffer construction; not bibliographic truth."},
        "authority_boundary":{"validator_role":"mechanics_and_closure_only_not_truth","claims_not_established":["execution_truth","content_truth","source_fidelity","translation_quality","semantic_correctness","rights_clearance","human_review","publication_authority","canon_authority"]}
    }))
}

impl WorkExpression<'_> {
    pub(crate) fn verify(
        &mut self,
        path: &str,
        claim: &Value,
        schemas: &mut CutWorkerSchemaExecutor,
    ) -> Result<NativeCompoundObservation, ItemRefusal> {
        check(self.limits.deadline, self.cancelled)?;
        metadata_path(path, false)?;
        if !path.ends_with("/source-claims.jsonl") {
            return Err(bad("native compound Claim carrier"));
        }
        let home = parent(path)?;
        let receipt_raw =
            self.required(&format!("{home}/work-expression-receipt.json"), MAX_FILE)?;
        let receipt = decode(&receipt_raw)?;
        if text(&receipt, "schema_version")? != "tos_work_expression_receipt_v1" {
            return Err(bad("native Claim compound receipt"));
        }
        let id = text(&receipt, "transaction_id")?;
        let tx = self.transaction(id)?;
        let transport = match tx.status.as_str() {
            "committed" => NativeTransportState::Committed,
            "rolled-back" => NativeTransportState::RolledBack,
            "pending" => NativeTransportState::Pending,
            "orphan" => NativeTransportState::Orphan,
            _ => return Err(bad("transaction outcome")),
        };
        let observation = NativeCompoundObservation {
            claim_path: path.into(),
            claim_id: text(claim, "claim_id")?.into(),
            transaction_id: id.into(),
            manifest_sha256: tx.manifest_sha256.clone(),
            transport,
        };
        if transport != NativeTransportState::Committed {
            return Ok(observation);
        }
        if self
            .publication
            .as_ref()
            .is_some_and(|p| p["phase"] != "ready")
        {
            return Err(bad("current source snapshot is pending owner recovery"));
        }
        let reconstructed = self.reconstruct(&tx, schemas)?;
        let scope = &reconstructed.scope;
        let work = text(scope, "work_source_path")?;
        let expression = text(scope, "expression_source_path")?;
        if path != format!("{}/source-claims.jsonl", parent(expression)?)
            || claim != &reconstructed.request["claim"]
            || receipt != reconstructed.receipt
            || receipt_raw != reconstructed.child["work-expression-receipt.json"]
            || self.required(path, MAX_FILE)? != reconstructed.child["source-claims.jsonl"]
        {
            return Err(bad("exact current compound Claim/receipt bytes"));
        }
        for name in [
            "source-create-request.json",
            "source-create-environment.json",
            "source-create-provenance.jsonl",
        ] {
            if self.required(&format!("{home}/{name}"), MAX_FILE)? != reconstructed.child[name] {
                return Err(bad("immutable compound capture changed"));
            }
        }
        let parent_files = self.selected(work)?;
        let parent_record = decode(&parent_files["work.json"])?;
        if parent_record["record_id"] != scope["work_id"] || parent_record["record_type"] != "work"
        {
            return Err(bad("current parent typed identity"));
        }
        let parent_history = self.history(work, &parent_files)?;
        if !array(&parent_history, "receipts")?.contains(&reconstructed.parent_receipt) {
            return Err(bad("compound transition missing in current parent lineage"));
        }
        let child_files = self.selected(expression)?;
        let child_record = decode(&child_files["expression.json"])?;
        if child_record["record_id"] != scope["expression_id"]
            || child_record["record_type"] != "expression"
            || child_record["work_ref"] != scope["work_id"]
        {
            return Err(bad("current Expression typed parent binding"));
        }
        let child_history = self.history(expression, &child_files)?;
        let mut initial = child_files["expression.json"] == reconstructed.child["expression.json"];
        for receipt in array(&child_history, "receipts")? {
            check(self.limits.deadline, self.cancelled)?;
            let archive = self.archive(expression, text(scope, "expression_id")?, receipt)?;
            if receipt["previous_source"] == reconstructed.receipt["expression"] {
                if archive["expression.json"] != reconstructed.child["expression.json"] {
                    return Err(bad("Expression initial archive bytes changed"));
                }
                initial = true;
            }
        }
        if !initial {
            return Err(bad("current Expression lacks committed initial lineage"));
        }
        check(self.limits.deadline, self.cancelled)?;
        Ok(observation)
    }
}
