//! Reviewed discovery queue, receipt replay and owner-evidenced readiness.
//! Mechanical closure preserves source identities, rights scope and history;
//! the resulting navigation does not perform a new owner review.
#[path = "open_work_queue/closure.rs"]
mod closure;
#[path = "open_work_queue/constraints.rs"]
mod constraints;
#[path = "open_work_queue/history.rs"]
mod history;
#[path = "open_work_queue/measurement.rs"]
mod measurement;
#[path = "open_work_queue/readiness.rs"]
mod readiness;
use crate::{
    kag_release::{self, invalid},
    source_registry::Context,
};
use closure::*;
use constraints::*;
use history::*;
pub use measurement::{MeasureOptions, measure};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::{Path, PathBuf},
    sync::atomic::AtomicI32,
};
use tos_compiler::source_registry as codec;
pub const LEDGER: &str = "ToS/source-witnesses/discovery/candidates/reviewed-candidates.jsonl";
pub const RECEIPTS: &str = "ToS/source-witnesses/discovery/candidates/receipts";
pub const TIMINGS: &str = "ToS/source-witnesses/discovery/timings";
pub const QUEUE: &str = "ToS/source-witnesses/discovery/candidates/queue.current.json";
const RUNS: &str = "ToS/source-witnesses/discovery/runs";
const SOURCE: &str = "ToS/source-witnesses";
const MASTER: [&str; 3] = [
    "ToS/philosophy/atlas/master-tables/table-i/rows.jsonl",
    "ToS/philosophy/atlas/master-tables/table-ii/rows.jsonl",
    "ToS/philosophy/atlas/master-tables/table-iii/rows.jsonl",
];
const DOSSIERS: &str = "ToS/philosophy/atlas/dossiers/index.jsonl";
const BACKLOG: &str = "ToS/philosophy/atlas/dossiers/source-anchor-backlog.jsonl";
const WORKS: &str = "ToS/source-witnesses/catalog/works.jsonl";
const READY: &str = "ready-for-discovery";
const PRODUCER: &str = "tos-open-work-queue";
const LEGACY_PRODUCER: &str = "scripts/build_open_work_candidate_queue.py";
type Result<T> = io::Result<T>;
type Located = (Value, String);
type Records = BTreeMap<String, Located>;
type Set = BTreeSet<String>;
fn require(condition: bool, message: impl Into<String>) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(invalid(message))
    }
}
fn obj(v: &Value) -> Result<&serde_json::Map<String, Value>> {
    v.as_object().ok_or_else(|| invalid("expected object"))
}
fn arr(v: &Value) -> Result<&[Value]> {
    v.as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| invalid("expected array"))
}
fn list(v: &Value) -> &[Value] {
    v.as_array().map_or(&[], Vec::as_slice)
}
fn txt(v: &Value) -> Result<&str> {
    v.as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| invalid("expected non-empty string"))
}
fn text(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
fn req<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    txt(&v[key]).map_err(|_| invalid(format!("{key} must be a non-empty string")))
}
fn strings(v: &Value) -> Result<Set> {
    arr(v)?.iter().map(|v| txt(v).map(str::to_owned)).collect()
}
fn references(v: &Value) -> Set {
    list(v)
        .iter()
        .filter_map(|v| v.as_str().filter(|s| !s.is_empty()).map(str::to_owned))
        .collect()
}
fn io_refs(v: &Value) -> Set {
    list(v)
        .iter()
        .filter_map(|v| v["ref"].as_str().map(str::to_owned))
        .collect()
}
fn intersects(a: &Set, b: &Set) -> bool {
    !a.is_disjoint(b)
}
fn union<'a>(sets: impl IntoIterator<Item = &'a Set>) -> Set {
    sets.into_iter().flat_map(|s| s.iter().cloned()).collect()
}
fn valid_sha(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
fn sha(v: &Value) -> Result<&str> {
    let s = txt(v)?;
    require(valid_sha(s), "expected lowercase SHA-256")?;
    Ok(s)
}
fn integer(v: &Value) -> Result<i64> {
    v.as_i64().ok_or_else(|| invalid("expected integer"))
}
fn positive_version(v: &Value) -> Result<i64> {
    let n = integer(&v["record_version"])?;
    require(n >= 1, "record_version must be positive")?;
    Ok(n)
}
fn canonical(v: &Value) -> Result<Vec<u8>> {
    let mut bytes = codec::encoded(v).map_err(invalid)?;
    require(bytes.pop() == Some(b'\n'), "canonical JSON terminator")?;
    Ok(bytes)
}
fn digest(v: &Value) -> Result<String> {
    Ok(codec::digest(&canonical(v)?))
}
fn queue_digest(v: &Value) -> Result<String> {
    let mut v = v.clone();
    obj(&v)?;
    v.as_object_mut().unwrap().remove("queue_sha256");
    digest(&v)
}
fn stamp(v: &Value) -> Result<i64> {
    tos_validation::observed_utc_or_naive_timestamp_micros(txt(v)?)
        .map_err(|e| invalid(format!("invalid queue timestamp: {e:?}")))
}
fn optional_stamp(v: &Value) -> Result<Option<i64>> {
    if v.is_null() || v == "" {
        Ok(None)
    } else {
        stamp(v).map(Some)
    }
}
fn physical_lines(raw: &[u8]) -> impl Iterator<Item = &[u8]> {
    let mut at = 0;
    std::iter::from_fn(move || {
        if at >= raw.len() {
            return None;
        }
        let start = at;
        while at < raw.len() && !matches!(raw[at], b'\r' | b'\n') {
            at += 1;
        }
        if at < raw.len() {
            let c = raw[at];
            at += 1;
            if c == b'\r' && raw.get(at) == Some(&b'\n') {
                at += 1;
            }
        }
        Some(&raw[start..at])
    })
}
fn scheme(s: &str) -> Option<&str> {
    let (head, _) = s.split_once(':')?;
    if head.starts_with(|c: char| c.is_ascii_alphabetic())
        && head
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"+-.".contains(&c))
    {
        Some(head)
    } else {
        None
    }
}
fn http_url(s: &str, credential_free: bool) -> bool {
    let Some((scheme, rest)) = s.split_once("://") else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    (scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https"))
        && !authority.is_empty()
        && (!credential_free || !authority.contains('@'))
}
fn queueable(v: &Value) -> bool {
    matches!(text(v), "work" | "work-like-corpus")
}
fn terminal(v: &Value) -> bool {
    matches!(
        text(v),
        "held_source_witness"
            | "metadata_only"
            | "publication_candidate"
            | "deferred"
            | "blocked"
            | "exhausted_for_now"
            | "duplicate"
            | "rejected"
            | "superseded"
    )
}
fn positive_rights(v: &Value) -> bool {
    matches!(
        text(v),
        "public_domain_reviewed" | "licensed" | "permission_granted"
    )
}
fn candidate_line(location: &str) -> Result<usize> {
    location
        .rsplit_once(':')
        .ok_or_else(|| invalid("candidate source line absent"))?
        .1
        .parse()
        .map_err(|_| invalid("candidate source line invalid"))
}
fn candidate_key(v: &Value, location: &str) -> Result<(i64, i64, usize, String)> {
    Ok((
        integer(&v["selection"]["chronology_sort_year"])?,
        integer(&v["selection"]["atlas_row_order"])?,
        candidate_line(location)?,
        req(v, "candidate_id")?.into(),
    ))
}
fn receipt_key(v: &Value) -> Result<(i64, i64, String)> {
    Ok((
        stamp(&v["issued_at"])?,
        positive_version(v)?,
        req(v, "receipt_id")?.into(),
    ))
}
struct Repo<'a> {
    root: PathBuf,
    context: Context<'a>,
}
impl<'a> Repo<'a> {
    fn new(root: &Path, cancel: &'a AtomicI32) -> Result<Self> {
        let root = kag_release::safe_absolute(root)?;
        kag_release::directory(&root)?;
        Ok(Self {
            root,
            context: Context::new(cancel),
        })
    }
    fn path(&self, rel: &str) -> Result<PathBuf> {
        kag_release::relative(&json!(rel))?;
        kag_release::safe_absolute(&self.root.join(rel))
    }
    fn exists(&self, rel: &str) -> Result<bool> {
        Ok(self.path(rel)?.exists())
    }
    fn is_file(&self, rel: &str) -> Result<bool> {
        let p = self.path(rel)?;
        match fs::symlink_metadata(p) {
            Ok(m) => Ok(m.is_file()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e),
        }
    }
    fn read(&mut self, rel: &str) -> Result<Vec<u8>> {
        self.context.read(&self.path(rel)?)
    }
    fn hash(&mut self, rel: &str) -> Result<String> {
        Ok(codec::digest(&self.read(rel)?))
    }
    fn json(&mut self, rel: &str) -> Result<Value> {
        let v = self.context.json(&self.path(rel)?)?;
        obj(&v)?;
        Ok(v)
    }
    fn lines(&mut self, rel: &str) -> Result<Vec<Located>> {
        let raw = self.read(rel)?;
        let mut out = Vec::new();
        for (n, line) in physical_lines(&raw).enumerate() {
            let line = std::str::from_utf8(line).map_err(io::Error::other)?;
            if line.trim().is_empty() {
                continue;
            }
            let v = codec::decoded(line.as_bytes(), false).map_err(invalid)?;
            obj(&v)?;
            out.push((v, format!("{rel}:{}", n + 1)));
        }
        Ok(out)
    }
    fn files(&mut self, root: &str, name: &str, recursive: bool) -> Result<Vec<String>> {
        let p = self.path(root)?;
        if !p.exists() {
            return Ok(vec![]);
        }
        kag_release::directory(&p)?;
        let mut todo = vec![p];
        let mut out = vec![];
        let mut n = 0;
        while let Some(p) = todo.pop() {
            for e in fs::read_dir(p)? {
                self.context.check(1).map_err(invalid)?;
                n += 1;
                require(n <= 100_000, "queue file enumeration budget")?;
                let e = e?;
                let typ = e.file_type()?;
                require(!typ.is_symlink(), "symlink in queue inputs")?;
                if typ.is_dir() && recursive {
                    todo.push(e.path())
                } else if typ.is_file()
                    && (e.file_name() == name
                        || name == "*.json" && e.path().extension().is_some_and(|s| s == "json"))
                {
                    out.push(
                        e.path()
                            .strip_prefix(&self.root)
                            .map_err(io::Error::other)?
                            .to_str()
                            .ok_or_else(|| invalid("non-UTF8 queue path"))?
                            .into(),
                    );
                }
            }
        }
        out.sort();
        Ok(out)
    }
    fn unique_record(
        &mut self,
        root: &str,
        filename: &str,
        key: &str,
        id: &str,
    ) -> Result<Located> {
        let mut found = None;
        for path in self.files(root, filename, true)? {
            let v = self.json(&path)?;
            if v[key] == id {
                require(
                    found.is_none(),
                    format!("duplicate canonical {filename} {id}"),
                )?;
                found = Some((v, path));
            }
        }
        found.ok_or_else(|| invalid(format!("missing canonical {filename} {id}")))
    }
    fn catalog(&mut self, filename: &str, id: &str) -> Result<Value> {
        let mut found = None;
        for (v, _) in self.lines(&format!("{SOURCE}/catalog/{filename}"))? {
            if v["record_id"] == id {
                require(found.is_none(), "duplicate canonical catalog record")?;
                found = Some(v);
            }
        }
        found.ok_or_else(|| invalid(format!("missing canonical catalog record {id}")))
    }
    fn schema(&mut self, name: &str, v: &Value) -> Result<()> {
        let schema = self.json(&format!("ToS/contracts/{name}.schema.json"))?;
        let validator = jsonschema::options()
            .should_validate_formats(true)
            .build(&schema)
            .map_err(|e| invalid(e.to_string()))?;
        validator
            .validate(v)
            .map_err(|e| invalid(format!("{name}: {e}")))
    }
}
fn source_refs(repo: &mut Repo<'_>, v: &Value) -> Result<()> {
    let refs = arr(&v["source_refs"])?;
    require(!refs.is_empty(), "source_refs must be non-empty")?;
    let selection = &v["selection"];
    let expected = req(selection, "atlas_row_id")?;
    let mut atlas = false;
    for reference in refs {
        let source = req(reference, "source_path")?;
        let selector = obj(&reference["selector"])?;
        require(!selector.is_empty(), "source selector must be non-empty")?;
        let records = if source.ends_with(".json") {
            vec![(repo.json(source)?, source.into())]
        } else {
            repo.lines(source)?
        };
        let matched: Vec<_> = records
            .iter()
            .filter(|(v, _)| {
                selector
                    .iter()
                    .all(|(k, value)| v.get(k).unwrap_or(&Value::Null) == value)
            })
            .collect();
        require(!matched.is_empty(), "source selector does not resolve")?;
        for key in ["row_id", "dossier_id"] {
            if let Some(chosen) = selector.get(key) {
                atlas = true;
                require(
                    txt(chosen)? == expected,
                    "source selector does not bind candidate atlas row",
                )?;
                if key == "row_id" {
                    let orders: BTreeSet<_> = matched
                        .iter()
                        .filter_map(|(v, _)| v["row_order"].as_i64())
                        .collect();
                    require(
                        orders.is_empty()
                            || orders == BTreeSet::from([integer(&selection["atlas_row_order"])?]),
                        "source row order differs",
                    )?;
                }
            }
        }
    }
    require(atlas, "source_refs lack a resolved atlas selector")
}
fn load_candidates(repo: &mut Repo<'_>) -> Result<Vec<Located>> {
    let rows = repo.lines(LEDGER)?;
    let mut seen = Set::new();
    for (v, location) in &rows {
        let id = req(v, "candidate_id")?;
        require(seen.insert(id.into()), "duplicate candidate_id")?;
        require(
            v["review"]["review_status"] == "reviewed",
            "candidate is not reviewed",
        )?;
        candidate_key(v, location)?;
        source_refs(repo, v)?;
    }
    Ok(rows)
}
fn discoveries(repo: &mut Repo<'_>) -> Result<Records> {
    let mut out = Records::new();
    for path in repo.files(RUNS, "*.json", false)? {
        let v = repo.json(&path)?;
        let id = req(&v, "discovery_id")?.into();
        require(
            out.insert(id, (v, path)).is_none(),
            "duplicate discovery_id",
        )?;
    }
    Ok(out)
}
fn provenance(repo: &mut Repo<'_>) -> Result<Records> {
    let mut out = Records::new();
    for path in repo.files(SOURCE, "provenance.jsonl", true)? {
        for (v, loc) in repo.lines(&path)? {
            let id = req(&v, "event_id")?.into();
            require(
                out.insert(id, (v, loc)).is_none(),
                "duplicate provenance event",
            )?;
        }
    }
    Ok(out)
}
pub fn build(
    root: &Path,
    plan: Option<&str>,
    readiness: bool,
    cancel: &AtomicI32,
) -> Result<Value> {
    let mut repo = Repo::new(root, cancel)?;
    let mut payload = build_inner(&mut repo)?;
    if readiness {
        payload = readiness::project(&mut repo, payload, plan)?;
    } else {
        require(plan.is_none(), "readiness plan requires readiness mode")?;
    }
    Ok(payload)
}
fn build_inner(repo: &mut Repo<'_>) -> Result<Value> {
    let candidates = load_candidates(repo)?;
    let discoveries = discoveries(repo)?;
    let events = provenance(repo)?;
    let receipts = load_receipts(repo, &candidates, &discoveries, &events)?;
    history::validate_history(repo, &candidates, &receipts, &discoveries, &events)?;
    history::queue_payload(
        repo,
        &candidates,
        &receipts,
        &discoveries,
        &events,
        None,
        &Set::new(),
        &BTreeMap::new(),
        PRODUCER,
    )
}
pub fn render(payload: &Value) -> Result<String> {
    String::from_utf8(
        crate::prepared_dossier_render::readiness_report_bytes(payload).map_err(invalid)?,
    )
    .map_err(io::Error::other)
}
pub fn validate(root: &Path, cancel: &AtomicI32) -> Result<()> {
    let mut repo = Repo::new(root, cancel)?;
    for (v, _) in repo.lines(LEDGER)? {
        repo.schema("open-work-candidate", &v)?;
    }
    for (directory, schema) in [
        (RECEIPTS, "open-work-candidate-receipt"),
        (TIMINGS, "open-work-channel-timing-receipt"),
    ] {
        for p in repo.files(directory, "*.json", false)? {
            let v = repo.json(&p)?;
            repo.schema(schema, &v)?;
        }
    }
    let actual = repo.json(QUEUE)?;
    repo.schema("open-work-candidate-queue", &actual)?;
    require(
        actual["queue_sha256"] == queue_digest(&actual)?,
        "queue_sha256 differs",
    )?;
    let expected = build_inner(&mut repo)?;
    require(
        repo.read(QUEUE)? == render(&expected)?.as_bytes(),
        "generated queue is stale",
    )
}
pub fn check(root: &Path, expected: &Value, cancel: &AtomicI32) -> Result<()> {
    let mut repo = Repo::new(root, cancel)?;
    require(
        repo.read(QUEUE)? == render(expected)?.as_bytes(),
        "generated queue is stale",
    )
}
pub fn write(root: &Path, expected: &Value) -> Result<()> {
    crate::route_cards::write_output(root, Path::new(QUEUE), &render(expected)?)
}

#[cfg(test)]
#[path = "open_work_queue/tests.rs"]
mod tests;
