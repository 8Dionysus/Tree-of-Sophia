//! Explicit-root registry import, history and inspection over the shared rules.
use crate::kag_release::{self, invalid};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicI32, Ordering},
    time::{Duration, Instant},
};
use tos_compiler::source_registry::{self as rules, DocumentInput, Result as RuleResult};

pub struct Context<'a> {
    cancel: &'a AtomicI32,
    deadline: Instant,
    work: u64,
}
impl<'a> Context<'a> {
    pub fn new(cancel: &'a AtomicI32) -> Self {
        Self {
            cancel,
            deadline: Instant::now() + Duration::from_secs(600),
            work: 8 * 1024 * 1024 * 1024,
        }
    }
    pub fn check(&mut self, n: u64) -> RuleResult<()> {
        if self.cancel.load(Ordering::Relaxed) != 0 || Instant::now() >= self.deadline {
            return Err("registry operation cancelled or timed out".into());
        }
        self.work = self
            .work
            .checked_sub(n)
            .ok_or("registry work budget exhausted")?;
        Ok(())
    }
    pub fn read(&mut self, path: &Path) -> io::Result<Vec<u8>> {
        self.check(1).map_err(invalid)?;
        let before = kag_release::regular(path)?;
        if before.len() > rules::MAX_FILE as u64 {
            return Err(invalid("registry file size limit"));
        }
        use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
        let mut f = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)?;
        let meta = f.metadata()?;
        let stamp = |m: &fs::Metadata| {
            (
                m.dev(),
                m.ino(),
                m.len(),
                m.mtime(),
                m.mtime_nsec(),
                m.ctime(),
                m.ctime_nsec(),
            )
        };
        if !meta.is_file() || stamp(&before) != stamp(&meta) {
            return Err(invalid("registry input changed while opening"));
        }
        let mut out = Vec::new();
        (&mut f)
            .take(rules::MAX_FILE as u64 + 1)
            .read_to_end(&mut out)?;
        if out.len() > rules::MAX_FILE
            || stamp(&meta) != stamp(&f.metadata()?)
            || stamp(&meta) != stamp(&fs::symlink_metadata(path)?)
        {
            return Err(invalid("registry input changed while reading"));
        }
        self.check(out.len() as u64).map_err(invalid)?;
        Ok(out)
    }
    pub fn json(&mut self, path: &Path) -> io::Result<Value> {
        rules::decoded(
            &self.read(path)?,
            path.extension().is_some_and(|e| e == "gz"),
        )
        .map_err(invalid)
    }
}
pub fn member(root: &Path, name: &Value) -> io::Result<PathBuf> {
    let name = kag_release::relative(name)?;
    kag_release::safe_absolute(&root.join(name))
}
fn directory(root: &Path) -> io::Result<PathBuf> {
    let root = kag_release::safe_absolute(root)?;
    kag_release::directory(&root)?;
    Ok(root)
}
fn processor(context: &mut Context<'_>) -> io::Result<Value> {
    // Bind the complete native implementation, including its linked rules.
    // The file path is deliberately omitted from portable research metadata.
    let executable = context.read(&std::env::current_exe()?)?;
    Ok(json!({"native/tos-source-registry": rules::digest(&executable)}))
}

fn snapshot_path(packet: &Path, pointer: &Value) -> io::Result<PathBuf> {
    let id = kag_release::hex(&pointer["snapshot_id"])?;
    Ok(packet.join("snapshots").join(id))
}
pub fn current(context: &mut Context<'_>, root: &Path) -> io::Result<(PathBuf, Value, Vec<Value>)> {
    let packet = directory(root)?;
    let pointer = context.json(&packet.join("current.json"))?;
    let snapshot = snapshot_path(&packet, &pointer)?;
    let summary = context.json(&snapshot.join("snapshot.json"))?;
    if summary["snapshot_id"] != pointer["snapshot_id"] {
        return Err(invalid("snapshot pointer identity mismatch"));
    }
    let documents = documents(context, &snapshot, &summary)?;
    Ok((snapshot, summary, documents))
}
fn documents(
    context: &mut Context<'_>,
    snapshot: &Path,
    summary: &Value,
) -> io::Result<Vec<Value>> {
    rules::array(&summary["documents"])
        .map_err(invalid)?
        .iter()
        .map(|name| context.json(&member(snapshot, name)?))
        .collect()
}
pub(crate) fn outputs_equal(
    context: &mut Context<'_>,
    path: &Path,
    body: &[u8],
) -> io::Result<bool> {
    let raw = context.read(path)?;
    if raw == body {
        return Ok(true);
    }
    if path.extension().is_none_or(|e| e != "gz") {
        return Ok(false);
    }
    // Gzip headers and compression choices do not own snapshot identity.
    Ok(
        rules::decompressed(&raw).map_err(invalid)?
            == rules::decompressed(body).map_err(invalid)?,
    )
}
fn immutable(context: &mut Context<'_>, path: &Path, body: &[u8]) -> io::Result<()> {
    if path.exists() {
        if !outputs_equal(context, path, body)? {
            return Err(invalid(format!(
                "immutable output collision: {}",
                path.display()
            )));
        }
        return Ok(());
    }
    kag_release::safe_absolute(path)?;
    fs::create_dir_all(path.parent().ok_or_else(|| invalid("missing parent"))?)?;
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(body)?;
    file.sync_all()?;
    context.check(body.len() as u64).map_err(invalid)
}
pub fn normalize(
    source_root: &Path,
    packet: &Path,
    input_root: Option<&Path>,
    check: bool,
    cancel: &AtomicI32,
) -> io::Result<Value> {
    let source = directory(source_root)?;
    let packet = directory(packet)?;
    let incoming = input_root.map(directory).transpose()?;
    let mut context = Context::new(cancel);
    let _lock = if check {
        None
    } else {
        let path = packet.join(".registry-import.lock");
        kag_release::safe_absolute(&path)?;
        let f = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        f.try_lock()
            .map_err(|e| invalid(format!("registry import already active: {e}")))?;
        Some(f)
    };
    let manifest = context.json(&packet.join("input.manifest.json"))?;
    let current = if packet.join("current.json").exists() {
        Some(current(&mut context, &packet)?)
    } else {
        None
    };
    let producer = if check {
        current
            .as_ref()
            .ok_or_else(|| invalid("current snapshot absent"))?
            .1["processor_sha256"]
            .clone()
    } else {
        processor(&mut context)?
    };
    let mut profiles = serde_json::Map::new();
    let mut originals = BTreeMap::new();
    let mut inputs = Vec::new();
    for spec in rules::array(&manifest["documents"]).map_err(invalid)? {
        let adapter = rules::text(&spec["adapter"]).map_err(invalid)?;
        let profile_body = context.read(&member(&source, &spec["adapter"])?)?;
        let profile = rules::decoded(&profile_body, false).map_err(invalid)?;
        profiles.insert(
            adapter.into(),
            json!({"sha256":rules::digest(&profile_body),"profile":profile}),
        );
        let mut files = serde_json::Map::new();
        let mut data = BTreeMap::new();
        for kind in ["xlsx", "docx"] {
            let f = &spec[kind];
            let path = if let Some(root) = &incoming {
                member(root, &f["source_path"])?
            } else {
                member(&packet, &f["original_path"])?
            };
            let body = context.read(&path)?;
            let sha = rules::digest(&body);
            if f["sha256"].as_str().is_some_and(|s| s != sha) {
                return Err(invalid(format!(
                    "input changed; review and update manifest: {}",
                    path.display()
                )));
            }
            let name = format!("originals/{sha}.{kind}");
            files.insert(kind.into(),json!({"source_path":f["source_path"],"sha256":sha,"size_bytes":body.len(),"original_path":name}));
            originals.insert(name, body.clone());
            data.insert(kind, body);
        }
        inputs.push(DocumentInput {
            spec: spec.clone(),
            files: Value::Object(files),
            xlsx: data.remove("xlsx").unwrap(),
            docx: data.remove("docx").unwrap(),
            profile,
        });
    }
    let result = rules::build(
        manifest,
        Value::Object(profiles),
        producer,
        inputs,
        &mut |n| context.check(n),
    )
    .map_err(invalid)?;
    let snapshot = snapshot_path(&packet, &result.summary)?;
    if check {
        if current
            .as_ref()
            .is_none_or(|(_, s, _)| s["snapshot_id"] != result.summary["snapshot_id"])
        {
            return Err(invalid("current snapshot identity drift"));
        }
        for (name, body) in &result.outputs {
            if !outputs_equal(&mut context, &snapshot.join(name), body)? {
                return Err(invalid(format!("generated snapshot drift: {name}")));
            }
        }
        for (name, body) in &originals {
            if context.read(&packet.join(name))? != *body {
                return Err(invalid(format!("original byte drift: {name}")));
            }
        }
    } else {
        for (name, body) in &originals {
            immutable(&mut context, &packet.join(name), body)?
        }
        for (name, body) in &result.outputs {
            immutable(&mut context, &snapshot.join(name), body)?
        }
        if !snapshot.join("delta.json").exists() {
            let previous = current
                .as_ref()
                .filter(|(_, s, _)| s["snapshot_id"] != result.summary["snapshot_id"]);
            let docs = rules::array(&result.summary["documents"])
                .map_err(invalid)?
                .iter()
                .map(|name| rules::decoded(&result.outputs[rules::text(name)?], true))
                .collect::<RuleResult<Vec<_>>>()
                .map_err(invalid)?;
            let mut delta = rules::delta(previous.map_or(&[], |(_, _, d)| d.as_slice()), &docs)
                .map_err(invalid)?;
            delta["previous_snapshot_id"] =
                previous.map_or(Value::Null, |(_, s, _)| s["snapshot_id"].clone());
            immutable(
                &mut context,
                &snapshot.join("delta.json"),
                &rules::encoded(&delta).map_err(invalid)?,
            )?;
        }
        let pointer=rules::encoded(&json!({"schema_version":"tos_source_registry_pointer_v1","snapshot_id":result.summary["snapshot_id"]})).map_err(invalid)?;
        let temp = packet.join(format!(".current-{}.tmp", std::process::id()));
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        f.write_all(&pointer)?;
        f.sync_all()?;
        fs::rename(&temp, packet.join("current.json"))?;
        File::open(&packet)?.sync_all()?;
    }
    Ok(result.summary)
}
pub fn inspect(
    packet: &Path,
    corpus: &str,
    document: &str,
    record: Option<&str>,
    cancel: &AtomicI32,
) -> io::Result<Value> {
    let (_, _, docs) = current(&mut Context::new(cancel), packet)?;
    let mut result = Vec::new();
    for d in docs {
        if d["corpus_id"] != corpus || d["document_id"] != document {
            continue;
        }
        if let Some(id) = record {
            result.extend(
                rules::array(&d["records"])
                    .map_err(invalid)?
                    .iter()
                    .filter(|r| r["record_id"] == id || r["source_record_id"] == id)
                    .cloned(),
            )
        } else {
            result.push(json!({"files":d["files"],"report_parts":d["report_parts"]}))
        }
    }
    if result.is_empty() {
        return Err(invalid("no matching record/document in current snapshot"));
    }
    Ok(json!(result))
}
pub fn validate(source: &Path, packet: &Path, cancel: &AtomicI32) -> io::Result<Value> {
    let summary = normalize(source, packet, None, true, cancel)?;
    let mut context = Context::new(cancel);
    let (_, _, docs) = current(&mut context, packet)?;
    let schema = context.json(&member(
        source,
        &json!("ToS/contracts/source-registry-normalization.schema.json"),
    )?)?;
    let validator = jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .build(&schema)
        .map_err(|e| invalid(e.to_string()))?;
    let mut ids = BTreeSet::new();
    let mut counts = BTreeMap::<String, u64>::new();
    for d in docs {
        context.check(1).map_err(invalid)?;
        validator.validate(&d).map_err(|e| invalid(e.to_string()))?;
        for r in rules::array(&d["records"]).map_err(invalid)? {
            let names = |key: &str| -> io::Result<Vec<String>> {
                rules::array(&r[key])
                    .map_err(invalid)?
                    .iter()
                    .map(|f| {
                        rules::text(&f["source_field"])
                            .map(str::to_owned)
                            .map_err(invalid)
                    })
                    .collect()
            };
            let mut raw = names("raw_fields")?;
            let mut reported = names("reported_fields")?;
            raw.sort();
            reported.sort();
            if raw != reported || raw.windows(2).any(|p| p[0] == p[1]) {
                return Err(invalid("field accounting error"));
            }
            if !ids.insert(rules::text(&r["record_id"]).map_err(invalid)?.to_owned()) {
                return Err(invalid("duplicate global record ID"));
            }
            *counts
                .entry(rules::text(&r["kind"]).map_err(invalid)?.into())
                .or_default() += 1;
            if r["source"]["original_sha256"] != d["files"]["xlsx"]["sha256"] {
                return Err(invalid("record original fixity mismatch"));
            }
        }
        for s in rules::array(&d["sheets"]).map_err(invalid)? {
            if s["record_count"].as_u64()
                != Some(
                    rules::array(&d["records"])
                        .map_err(invalid)?
                        .iter()
                        .filter(|r| r["source"]["sheet"] == s["name"])
                        .count() as u64,
                )
            {
                return Err(invalid("sheet record coverage mismatch"));
            }
        }
    }
    for (kind, n) in counts {
        if summary["counts"][kind].as_u64() != Some(n) {
            return Err(invalid("snapshot record total mismatch"));
        }
    }
    Ok(summary)
}
