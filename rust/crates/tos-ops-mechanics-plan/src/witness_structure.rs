//! Tracked text-free structural-map mechanics, with no payload access or semantic admission.
use crate::route_cards::{RouteSources, python_space, sha256_bytes, splitlines};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, Write},
    path::Path,
    sync::atomic::{AtomicI32, Ordering},
};
use tos_foundation::{JsonLimits, JsonMode, parse_json};

pub const MAP_PATH: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/alignments/structure/naumann-1893-dta-parts/structure-correspondence.json";
pub const ANCHOR_SET_PATH: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/alignments/structure/naumann-1893-dta-parts/structure-anchor-set.json";
pub const ANCHOR_RECORDS_PATH: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/alignments/structure/naumann-1893-dta-parts/structure-anchors.jsonl";
pub const SCHEMA_PATH: &str = "ToS/contracts/witness-structure-correspondence.schema.json";
pub const ANCHOR_SET_SCHEMA_PATH: &str = "ToS/contracts/witness-structure-anchor-set.schema.json";
pub const ANCHOR_SCHEMA_PATH: &str = "ToS/contracts/source-anchor.schema.json";
pub const PROVENANCE_SCHEMA_PATH: &str = "ToS/contracts/provenance-event.schema.json";
pub const PARALLEL_MAP_PATH: &str = "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/alignments/structure/naumann-1886-polilov-mysl-1996/structure-correspondence.json";
pub const PARALLEL_ANCHOR_RECORDS_PATH: &str = "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/alignments/structure/naumann-1886-polilov-mysl-1996/structure-anchors.jsonl";
pub const PARALLEL_SCHEMA_PATH: &str = "ToS/contracts/parallel-witness-structure-map.schema.json";
pub const NUMBERED_UNIT_MAP_PATH: &str = "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/expressions/de-naumann-1886/editions/leipzig-c-g-naumann-1886/items/internet-archive-google-harvard-scan-pdf/structure/numbered-unit-page-map.json";
pub const NUMBERED_UNIT_ANCHOR_RECORDS_PATH: &str = "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/expressions/de-naumann-1886/editions/leipzig-c-g-naumann-1886/items/internet-archive-google-harvard-scan-pdf/structure/numbered-unit-anchors.jsonl";
pub const NUMBERED_UNIT_SCHEMA_PATH: &str = "ToS/contracts/numbered-unit-page-map.schema.json";
pub const TARGET_NUMBERED_UNIT_MAP_PATH: &str = "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/expressions/ru-polilov-mysl-1996/structure/mysl-1996-volume-2-operator-pdf/numbered-unit-page-map.json";
pub const TARGET_NUMBERED_UNIT_ANCHOR_RECORDS_PATH: &str = "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/expressions/ru-polilov-mysl-1996/structure/mysl-1996-volume-2-operator-pdf/numbered-unit-anchors.jsonl";
pub const TARGET_NUMBERED_UNIT_SCHEMA_PATH: &str =
    "ToS/contracts/target-numbered-unit-page-map.schema.json";
pub const TARGET_NUMBERED_UNIT_EVENT_ID: &str = "tos.event.target-numbered-unit-page-map.friedrich-nietzsche.jenseits-von-gut-und-boese.ru-polilov-mysl-1996.2026-07-29";
pub const NUMBERED_UNIT_LABEL_MAP_PATH: &str = "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/alignments/structure/naumann-1886-polilov-mysl-1996/numbered-unit-label-correspondence.json";
pub const NUMBERED_UNIT_LABEL_PROVENANCE_PATH: &str = "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/alignments/structure/naumann-1886-polilov-mysl-1996/provenance.numbered-unit-label-correspondence.jsonl";
pub const NUMBERED_UNIT_LABEL_SCHEMA_PATH: &str =
    "ToS/contracts/parallel-numbered-unit-label-map.schema.json";
pub const NUMBERED_UNIT_LABEL_SOURCE_RIGHTS_PATH: &str = "ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/expressions/de-naumann-1886/editions/leipzig-c-g-naumann-1886/items/internet-archive-google-harvard-scan-pdf/rights.json";
pub const NUMBERED_UNIT_LABEL_TARGET_RIGHTS_PATH: &str = "ToS/source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/editions/moscow-mysl-1996-volume-2/items/operator-pdf/rights.json";
pub const NUMBERED_UNIT_LABEL_EVENT_ID: &str = "tos.event.parallel-numbered-unit-label-map.friedrich-nietzsche.jenseits-von-gut-und-boese.layered-rights-refresh.2026-08-02";
pub const NUMBERED_UNIT_EVENT_ID: &str = "tos.event.numbered-unit-page-map.friedrich-nietzsche.jenseits-von-gut-und-boese.de-naumann-1886.2026-07-29";
pub const PARALLEL_EVENT_ID: &str = "tos.event.structure-correspondence.friedrich-nietzsche.jenseits-von-gut-und-boese.naumann-1886-to-polilov-mysl-1996.source-237a-correction.2026-07-29";
pub const ANCHOR_EVENT_ID: &str = "tos.event.structure-anchors.friedrich-nietzsche.also-sprach-zarathustra.dta-parts-to-naumann-1893.2026-07-28";

pub type Issue = (String, String);
type Index<'a> = BTreeMap<&'a str, &'a Value>;
fn invalid(s: impl Into<String>) -> io::Error {
    io::Error::other(s.into())
}
fn arr(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn objects(v: &Value) -> impl Iterator<Item = &Value> {
    arr(v).iter().filter(|v| v.is_object())
}
fn text(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
fn index<'a>(v: &'a Value, key: &str) -> Index<'a> {
    objects(v)
        .filter_map(|v| v[key].as_str().map(|s| (s, v)))
        .collect()
}
fn get<'a>(m: &Index<'a>, v: &Value) -> &'a Value {
    m.get(text(v)).copied().unwrap_or(&Value::Null)
}
fn keys(m: &Index<'_>) -> BTreeSet<String> {
    m.keys().map(|s| (*s).into()).collect()
}
fn string_set(v: &Value) -> BTreeSet<String> {
    arr(v)
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}
fn strs(v: &[&str]) -> BTreeSet<String> {
    v.iter().map(|s| (*s).into()).collect()
}
fn values<'a>(v: impl Iterator<Item = &'a Value>) -> Value {
    Value::Array(v.cloned().collect())
}
fn tuple_set(v: &Value, fields: &[&str]) -> BTreeSet<Vec<String>> {
    objects(v)
        .map(|v| fields.iter().map(|f| v[*f].to_string()).collect())
        .collect()
}
fn tuple(v: &[Value]) -> Vec<String> {
    v.iter().map(Value::to_string).collect()
}
fn whole_page(page: &Value) -> Value {
    json!({"type":"page_region","page":page,"x":0,"y":0,"width":1,"height":1,"coordinate_space":"normalized_0_1"})
}
fn monotonic(v: &[i64], strict: bool) -> bool {
    v.windows(2)
        .all(|p| if strict { p[0] < p[1] } else { p[0] <= p[1] })
}
fn numbers(v: impl Iterator<Item = Value>) -> Vec<i64> {
    v.filter_map(|v| v.as_i64()).collect()
}
fn page_member(v: &Value) -> Option<i64> {
    let s = v
        .as_str()?
        .strip_prefix("EPUB/page_")?
        .strip_suffix(".html")?;
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

struct Context<'a> {
    source: RouteSources,
    cancel: &'a AtomicI32,
    issues: Vec<Issue>,
    issue_bytes: usize,
    schemas: BTreeMap<String, jsonschema::Validator>,
}
impl<'a> Context<'a> {
    fn new(root: &Path, cancel: &'a AtomicI32) -> io::Result<Self> {
        Ok(Self {
            source: RouteSources::new(root)?,
            cancel,
            issues: vec![],
            issue_bytes: 0,
            schemas: BTreeMap::new(),
        })
    }
    fn tick(&self) -> io::Result<()> {
        if self.cancel.load(Ordering::Relaxed) != 0 {
            return Err(invalid("witness-structure validation cancelled"));
        }
        self.source.check()
    }
    fn issue(&mut self, path: &str, message: impl Into<String>) -> io::Result<()> {
        self.tick()?;
        let message = message.into();
        self.issue_bytes = self
            .issue_bytes
            .checked_add(path.len() + message.len())
            .ok_or_else(|| invalid("witness issue accounting"))?;
        if self.issues.len() >= 4096 || self.issue_bytes > 1_048_576 {
            return Err(invalid("witness issue bound"));
        }
        self.issues.push((path.into(), message));
        Ok(())
    }
    fn require(&mut self, ok: bool, path: &str, message: impl Into<String>) -> io::Result<()> {
        self.tick()?;
        if !ok {
            self.issue(path, message)?;
        }
        Ok(())
    }
    fn decoded(&mut self, raw: &str, path: &str) -> io::Result<Option<Value>> {
        self.tick()?;
        let limits = JsonLimits::new(8 * 1024 * 1024, 96, 1_000_000, 4096)
            .map_err(|e| invalid(e.to_string()))?;
        if let Err(e) = parse_json(raw.as_bytes(), JsonMode::RequestLastWins, limits) {
            self.issue(path, format!("cannot read JSON: {e}"))?;
            return Ok(None);
        }
        let v: Value = serde_json::from_str(raw).map_err(io::Error::other)?;
        if !v.is_object() {
            self.issue(path, "JSON root must be an object")?;
            return Ok(None);
        }
        Ok(Some(v))
    }
    fn json(&mut self, path: &str) -> io::Result<Option<Value>> {
        self.tick()?;
        let Some(raw) = self.source.text(path)? else {
            self.issue(path, "file is missing")?;
            return Ok(None);
        };
        self.decoded(&raw, path)
    }
    fn jsonl(&mut self, path: &str) -> io::Result<Vec<Value>> {
        self.tick()?;
        let Some(raw) = self.source.text(path)? else {
            self.issue(path, "cannot read JSONL: file is missing")?;
            return Ok(vec![]);
        };
        let mut out = vec![];
        for (i, line) in splitlines(&raw).into_iter().enumerate() {
            if line.chars().all(python_space) {
                continue;
            }
            if let Some(v) = self.decoded(line, &format!("{path}:{}", i + 1))? {
                out.push(v);
            }
        }
        Ok(out)
    }
    fn digest(&mut self, path: &str) -> io::Result<String> {
        self.tick()?;
        Ok(sha256_bytes(&self.source.bytes(path)?))
    }
    fn schema(&mut self, v: &Value, schema: &str, path: &str) -> io::Result<()> {
        self.tick()?;
        if !self.schemas.contains_key(schema) {
            let s = self
                .json(schema)?
                .ok_or_else(|| invalid("witness schema absent"))?;
            let validator = jsonschema::options()
                .should_validate_formats(true)
                .offline()
                .build(&s)
                .map_err(|e| invalid(format!("witness schema {schema}: {e}")))?;
            self.schemas.insert(schema.into(), validator);
        }
        let mut errors = vec![];
        let mut size = 0usize;
        for e in self.schemas[schema].iter_errors(v) {
            self.tick()?;
            let msg = e.to_string();
            size += msg.len();
            if errors.len() >= 4096 || size > 1_048_576 {
                return Err(invalid("witness schema issue bound"));
            }
            errors.push((e.instance_path().to_string(), msg));
        }
        for (suffix, message) in errors {
            self.issue(&format!("{path}{suffix}"), message)?;
        }
        Ok(())
    }
    fn no_text(&mut self, v: &Value, path: &str, prefix: &str) -> io::Result<()> {
        self.tick()?;
        match v {
            Value::Object(m) => {
                for (k, v) in m {
                    let p = if prefix.is_empty() {
                        k.clone()
                    } else {
                        format!("{prefix}.{k}")
                    };
                    if [
                        "text",
                        "source_text",
                        "heading_text",
                        "quote",
                        "excerpt",
                        "transcription",
                    ]
                    .contains(&k.as_str())
                    {
                        self.issue(path, format!("source-text-bearing key is forbidden: {p}"))?;
                    }
                    self.no_text(v, path, &p)?;
                }
            }
            Value::Array(a) => {
                for (i, v) in a.iter().enumerate() {
                    self.no_text(v, path, &format!("{prefix}[{i}]"))?;
                }
            }
            _ => (),
        };
        Ok(())
    }
    fn load_map(&mut self, path: &str, schema: &str) -> io::Result<Option<Value>> {
        let v = self.json(path)?;
        if let Some(v) = &v {
            self.schema(v, schema, path)?;
            self.no_text(v, path, "")?;
        }
        Ok(v)
    }
    fn anchors(&mut self, path: &str) -> io::Result<Vec<Value>> {
        let rows = self.jsonl(path)?;
        let mut ids = BTreeSet::new();
        for (i, v) in rows.iter().enumerate() {
            let loc = format!("{path}:{}", i + 1);
            self.schema(v, ANCHOR_SCHEMA_PATH, &loc)?;
            self.no_text(v, &loc, "")?;
            self.require(
                v["anchor_id"]
                    .as_str()
                    .is_some_and(|id| ids.insert(id.to_owned())),
                &loc,
                "anchor_id is invalid or duplicated",
            )?;
        }
        Ok(rows)
    }
    fn fields(
        &mut self,
        v: &Value,
        expected: &Value,
        path: &str,
        message: impl Fn(&str) -> String,
    ) -> io::Result<()> {
        if let Some(m) = expected.as_object() {
            for (k, x) in m {
                self.require(&v[k] == x, path, message(k))?;
            }
        }
        Ok(())
    }
    fn events(&mut self, path: &str) -> io::Result<Vec<Value>> {
        let rows = self.jsonl(path)?;
        for (i, v) in rows.iter().enumerate() {
            self.schema(v, PROVENANCE_SCHEMA_PATH, &format!("{path}:{}", i + 1))?;
        }
        Ok(rows)
    }
    fn event<'v>(
        &mut self,
        events: &'v [Value],
        event_ref: &Value,
        required: Option<&str>,
        path: &str,
        message: &str,
    ) -> io::Result<Option<&'v Value>> {
        let found: Vec<_> = events
            .iter()
            .filter(|v| v["event_id"] == *event_ref)
            .collect();
        if found.len() != 1 || required.is_some_and(|s| event_ref != s) {
            self.issue(path, message)?;
            Ok(None)
        } else {
            Ok(Some(found[0]))
        }
    }
    fn bound_output(&mut self, path: &str, role: &str) -> io::Result<Vec<String>> {
        Ok(tuple(&[
            json!(path),
            json!(role),
            json!(self.digest(path)?),
        ]))
    }
    fn inventory(&mut self, w: &Value, parallel: bool, path: &str) -> io::Result<Option<Value>> {
        let binding = if parallel {
            &w["inventory"]["ref"]
        } else {
            &w["inventory_ref"]
        };
        let Some(reference) = binding.as_str() else {
            self.issue(path, "witness inventory reference is invalid")?;
            return Ok(None);
        };
        let Some(inv) = self.json(reference)? else {
            return Ok(None);
        };
        if parallel {
            let digest = self.digest(reference)?;
            self.require(
                w["inventory"]["sha256"] == digest,
                path,
                "witness inventory digest drifted",
            )?;
        }
        self.require(
            inv["item_id"] == w["item_ref"],
            path,
            "witness item_ref differs from inventory",
        )?;
        let files: Vec<_> = objects(&inv["files"])
            .filter(|f| f["file_id"] == w["file_ref"])
            .collect();
        if files.len() != 1 {
            self.issue(path, "witness file_ref does not resolve exactly once")?;
            return Ok(None);
        }
        let f = files[0];
        for (field, msg) in [
            ("file_sha256", "witness file digest differs from inventory"),
            ("profile", "witness profile differs from inventory"),
        ] {
            self.require(f[field] == w[field], path, msg)?;
        }
        if parallel && w["work_boundary"].is_object() {
            if let Some(p) = w["work_boundary"]["ref"].as_str() {
                if !self.source.is_file(p)? {
                    self.issue(path, "work-boundary artifact is missing")?;
                } else {
                    let digest = self.digest(p)?;
                    self.require(
                        w["work_boundary"]["sha256"] == digest,
                        path,
                        "work-boundary digest drifted",
                    )?;
                }
            } else {
                self.issue(path, "work-boundary ref is invalid")?;
            }
        }
        Ok(Some(f.clone()))
    }
}

mod labels;
mod numbered;
mod parallel;
mod zarathustra;

pub fn run_validation(root: &Path, cancel: &AtomicI32) -> io::Result<Vec<Issue>> {
    let mut c = Context::new(root, cancel)?;
    zarathustra::validate(&mut c)?;
    parallel::validate(&mut c)?;
    numbered::validate(&mut c, false)?;
    numbered::validate(&mut c, true)?;
    labels::validate(&mut c)?;
    c.tick()?;
    Ok(c.issues)
}
pub fn run(root: &Path, cancel: &AtomicI32) -> io::Result<i32> {
    let issues = run_validation(root, cancel)?;
    if !issues.is_empty() {
        let mut out = io::stderr().lock();
        writeln!(out, "Witness-structure correspondence validation failed.")?;
        for (path, msg) in issues {
            writeln!(out, "- {path}: {msg}")?;
        }
        return Ok(1);
    }
    println!(
        "[ok] validated text-free witness-structure correspondences\n[scope] Exact resource bindings and structural locator-candidate mechanics."
    );
    Ok(0)
}
