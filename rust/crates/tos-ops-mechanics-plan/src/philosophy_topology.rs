//! Maintained philosophy topology mechanics; no textual/semantic admission.
use crate::route_cards::{RouteSources, python_space, splitlines};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::{AtomicI32, Ordering};
use tos_foundation::{JsonLimits, JsonMode, parse_json};
const MANIFEST: &str = "ToS/philosophy/philosophy.manifest.json";
const SCHEMA: &str = "ToS/contracts/philosophy-source-planting.schema.json";
type Issue = (String, String);
struct Context<'a> {
    source: RouteSources,
    cancel: &'a AtomicI32,
    issues: Vec<Issue>,
    issue_bytes: usize,
}
impl Context<'_> {
    fn check(&self) -> io::Result<()> {
        if self.cancel.load(Ordering::Relaxed) != 0 {
            return Err(io::Error::other("philosophy topology cancelled"));
        }
        self.source.check()
    }
    fn issue(&mut self, path: &str, message: impl Into<String>) -> io::Result<()> {
        self.check()?;
        let message = message.into();
        let bytes = self
            .issue_bytes
            .checked_add(path.len())
            .and_then(|v| v.checked_add(message.len()))
            .and_then(|v| v.checked_add(5))
            .ok_or_else(|| io::Error::other("philosophy issue accounting overflow"))?;
        if self.issues.len() >= 4096 || bytes > 1_048_576 {
            return Err(io::Error::other("philosophy issue output bound exceeded"));
        }
        self.issue_bytes = bytes;
        self.issues.push((path.into(), message));
        Ok(())
    }
    fn decode(&self, raw: &str) -> io::Result<Value> {
        self.check()?;
        parse_json(
            raw.as_bytes(),
            JsonMode::RequestLastWins,
            JsonLimits {
                max_bytes: 1_048_576,
                max_depth: 64,
                max_visits: 300_000,
                max_integer_digits: 4300,
            },
        )
        .map_err(|e| {
            io::Error::new(
                if matches!(
                    e.code,
                    FoundationErrorCode::BudgetExceeded
                        | FoundationErrorCode::NonfiniteFloat
                        | FoundationErrorCode::InvalidUnicodeScalar
                ) {
                    io::ErrorKind::Unsupported
                } else {
                    io::ErrorKind::InvalidData
                },
                format!("philosophy JSON profile: {e:?}"),
            )
        })?;
        serde_json::from_str(raw).map_err(io::Error::other)
    }
    fn json(&mut self, path: &str) -> io::Result<Option<Value>> {
        self.check()?;
        let Some(raw) = self.source.text(path)? else {
            self.issue(path, "missing JSON file")?;
            return Ok(None);
        };
        let value = match self.decode(&raw) {
            Ok(v) => v,
            Err(e) if e.kind() == io::ErrorKind::Unsupported => return Err(e),
            Err(e) => {
                self.issue(path, format!("invalid JSON: {e}"))?;
                return Ok(None);
            }
        };
        if !value.is_object() {
            self.issue(path, "JSON root must be an object")?;
            return Ok(None);
        }
        Ok(Some(value))
    }
    fn equal(
        &mut self,
        p: &str,
        v: &Value,
        k: &str,
        expected: &str,
        message: &str,
    ) -> io::Result<()> {
        if v.get(k).and_then(Value::as_str) != Some(expected) {
            self.issue(p, message)?;
        }
        Ok(())
    }
    fn paths(&mut self, root: &str) -> io::Result<Vec<String>> {
        self.check()?;
        self.source.paths(root)
    }
    fn count_json_lines(
        &mut self,
        path: &str,
        mut matches: impl FnMut(&Value) -> io::Result<bool>,
    ) -> io::Result<usize> {
        let raw = self
            .source
            .text(path)?
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "missing JSONL file"))?;
        let mut count = 0usize;
        for line in splitlines(&raw)
            .into_iter()
            .filter(|s| !s.trim_matches(python_space).is_empty())
        {
            self.check()?;
            let row = self.decode(line)?;
            if matches(&row)? {
                count += 1;
            }
        }
        Ok(count)
    }
}
// The maintained Python comparisons use numeric equality, including bools.
// Large integer lexemes never round through f64; floating lexemes follow
// Python json.loads' finite IEEE representation, as in the existing validators.
fn integer_key(v: &Value) -> io::Result<Option<num_bigint::BigInt>> {
    let lexeme = match v {
        Value::Bool(b) => return Ok(Some(num_bigint::BigInt::from(u8::from(*b)))),
        Value::Number(n) => n.to_string(),
        _ => return Ok(None),
    };
    if !lexeme.bytes().any(|c| matches!(c, b'.' | b'e' | b'E')) {
        return lexeme.parse().map(Some).map_err(io::Error::other);
    }
    let f: f64 = lexeme.parse().map_err(io::Error::other)?;
    if !f.is_finite() {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "nonfinite Python numeric observation",
        ));
    }
    if f.fract() != 0.0 {
        return Ok(None);
    }
    if f == 0.0 {
        return Ok(Some(num_bigint::BigInt::from(0)));
    }
    let bits = f.to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i32;
    let mantissa = (bits & ((1u64 << 52) - 1)) | if exponent == 0 { 0 } else { 1u64 << 52 };
    let power = if exponent == 0 {
        -1074
    } else {
        exponent - 1023 - 52
    };
    let n = num_bigint::BigInt::from(mantissa);
    let n = if power >= 0 {
        n << (power as usize)
    } else {
        n >> ((-power) as usize)
    };
    Ok(Some(if bits >> 63 == 0 { n } else { -n }))
}
fn py_equal(a: Option<&Value>, b: Option<&Value>) -> io::Result<bool> {
    let null = Value::Null;
    let a = a.unwrap_or(&null);
    let b = b.unwrap_or(&null);
    match (a, b) {
        (Value::Number(_) | Value::Bool(_), Value::Number(_) | Value::Bool(_)) => {
            match (integer_key(a)?, integer_key(b)?) {
                (Some(a), Some(b)) => Ok(a == b),
                (None, None) => Ok(a.as_f64() == b.as_f64()),
                _ => Ok(false),
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            if a.len() != b.len() {
                return Ok(false);
            }
            for (a, b) in a.iter().zip(b) {
                if !py_equal(Some(a), Some(b))? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        (Value::Object(a), Value::Object(b)) => {
            if a.len() != b.len() {
                return Ok(false);
            }
            for (k, a) in a {
                let Some(b) = b.get(k) else {
                    return Ok(false);
                };
                if !py_equal(Some(a), Some(b))? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        _ => Ok(a == b),
    }
}
fn string<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
    v.get(k).and_then(Value::as_str)
}
fn parent(path: &str) -> String {
    Path::new(path)
        .parent()
        .unwrap_or(Path::new(""))
        .to_string_lossy()
        .into_owned()
}
fn normalized(path: &str) -> Option<String> {
    if path.starts_with('/') || path.split('/').any(|p| p == "..") {
        return None;
    }
    let parts: Vec<_> = path
        .split('/')
        .filter(|p| !p.is_empty() && *p != ".")
        .collect();
    Some(if parts.is_empty() {
        ".".into()
    } else {
        parts.join("/")
    })
}
fn under(path: &str, root: &str) -> bool {
    path == root || path.strip_prefix(root).is_some_and(|s| s.starts_with('/'))
}
fn truth(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(o)) => !o.is_empty(),
        Some(Value::Number(n)) => n.as_f64() != Some(0.0),
    }
}
fn slug(value: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in value.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            if dash && !out.is_empty() {
                out.push('-');
            }
            out.push(c);
            dash = false;
        } else {
            dash = true;
        }
    }
    out
}
fn labels(v: &Value) -> BTreeSet<String> {
    let mut result = BTreeSet::new();
    if let Some(c) = v.get("capture_container").filter(|v| v.is_object()) {
        let title = if truth(c.get("original_title")) {
            c.get("original_title")
        } else {
            c.get("title")
        };
        if let Some(t) = title.and_then(Value::as_str) {
            result.insert(slug(t));
        }
    }
    if let Some(children) = v.get("branch_child_pages").and_then(Value::as_array) {
        for child in children {
            if let Some(s) = child.as_str() {
                result.insert(slug(s));
            } else if child.is_object() {
                for k in ["original_title", "title"] {
                    if let Some(s) = string(child, k) {
                        result.insert(slug(s));
                    }
                }
            }
        }
    }
    result.remove("");
    result
}
fn check_labels(c: &mut Context<'_>, path: &str, labels: &BTreeSet<String>) -> io::Result<()> {
    let parts: BTreeSet<_> = path.split('/').map(str::to_lowercase).collect();
    for label in labels {
        if parts.contains(label) {
            c.issue(
                path,
                format!("metadata-only source label used as path component: {label}"),
            )?;
        }
    }
    Ok(())
}
fn routes(c: &mut Context<'_>, v: &Value, key: &str, root: &str) -> io::Result<()> {
    let Some(entries) = v.get(key).filter(|v| !v.is_null()) else {
        return Ok(());
    };
    let Some(entries) = entries
        .as_array()
        .filter(|a| a.iter().all(|e| e.as_str().is_some_and(|s| !s.is_empty())))
    else {
        c.issue(
            MANIFEST,
            format!("{key} must be a list of non-empty strings"),
        )?;
        return Ok(());
    };
    for entry in entries {
        let entry = entry.as_str().unwrap();
        let Some(path) = normalized(entry) else {
            c.issue(
                entry,
                format!("{key} entries must be normalized repo-relative paths"),
            )?;
            continue;
        };
        if !under(&path, root) {
            c.issue(entry, format!("{key} entries must stay under {root}"))?;
        } else if !c.source.exists(&path)? {
            c.issue(entry, format!("{key} entry is missing"))?;
        }
    }
    Ok(())
}
fn atlas_membership(
    c: &mut Context<'_>,
    plant: &Value,
    location: &str,
    atlas_paths: &[String],
) -> io::Result<()> {
    let id = plant.get("atlas_row_id");
    if !py_equal(id, plant.get("dossier_id"))? {
        c.issue(location, "planting atlas_row_id and dossier_id must agree")?;
    }
    let mut matches = 0usize;
    for path in atlas_paths {
        match c.count_json_lines(path, |row| {
            Ok(row.is_object() && py_equal(row.get("row_id"), id)?)
        }) {
            Ok(count) => matches += count,
            Err(e) if e.kind() == io::ErrorKind::Unsupported => return Err(e),
            Err(e) => c.issue(
                location,
                format!("cannot read planting atlas membership: {e}"),
            )?,
        }
    }
    if matches != 1 {
        c.issue(
            location,
            "planting atlas row must resolve exactly once in current master tables",
        )?;
    }
    if let Some(branch) = string(plant, "branch_path") {
        match normalized(branch) {
            Some(p) if p.starts_with("ToS/philosophy/") => {
                if let Some(b) = c.json(&format!("{p}/branch.manifest.json"))? {
                    if !b
                        .get("atlas_rows")
                        .and_then(Value::as_array)
                        .is_some_and(|a| id.is_some_and(|v| a.contains(v)))
                    {
                        c.issue(
                            location,
                            "planting atlas row does not belong to the exact branch",
                        )?;
                    }
                }
            }
            _ => c.issue(
                location,
                "planting branch must resolve under ToS/philosophy",
            )?,
        }
    }
    Ok(())
}
fn witness(c: &mut Context<'_>, plant: &Value, p: &str) -> io::Result<()> {
    let Some(w) = plant.get("source_witness").filter(|v| v.is_object()) else {
        return Ok(());
    };
    for (key, root, name) in [
        ("artifact_id", "ToS/source-witnesses/artifacts/", "artifact"),
        (
            "composite_id",
            "ToS/source-witnesses/scholarly-composites/",
            "composite",
        ),
    ] {
        if w.get(key).is_some() {
            if let Some(record) = string(w, "record_ref").filter(|s| s.starts_with(root)) {
                if let Some(r) = c.json(record)? {
                    if r.get(key) != w.get(key) {
                        c.issue(
                            p,
                            format!("source-witness {key} differs from its exact record"),
                        )?;
                    }
                }
            } else {
                c.issue(
                    p,
                    format!(
                        "{name} source witness must route to the {} spine",
                        if name == "artifact" {
                            "artifact"
                        } else {
                            "scholarly-composite"
                        }
                    ),
                )?;
            }
            return Ok(());
        }
    }
    if w.get("work_id").is_none() {
        c.issue(p, "source witness has no admitted identity field")?;
        return Ok(());
    }
    if let Some(record) =
        string(w, "record_ref").filter(|s| s.starts_with("ToS/source-witnesses/works/"))
    {
        if let Some(r) = c.json(record)? {
            if string(&r, "record_type") != Some("work") || r.get("record_id") != w.get("work_id") {
                c.issue(
                    p,
                    "source-witness work_id differs from its exact Work record",
                )?;
            }
        }
    } else {
        c.issue(
            p,
            "work source witness must route to the bibliographic Work spine",
        )?;
    }
    let count = ["container_id", "container_ref", "membership_claim_ref"]
        .iter()
        .filter(|k| w.get(**k).is_some_and(|v| !v.is_null()))
        .count();
    if count > 0 && count != 3 {
        c.issue(
            p,
            "bibliographic Work container fields must be all present or all absent",
        )?;
    } else if count == 3 {
        if let Some(record) = string(w, "container_ref")
            .filter(|s| s.starts_with("ToS/source-witnesses/collections/"))
        {
            if let Some(r) = c.json(record)? {
                if string(&r, "record_type") != Some("collection")
                    || r.get("record_id") != w.get("container_id")
                {
                    c.issue(
                        p,
                        "source-witness container_id differs from its exact Collection record",
                    )?;
                }
                if !r
                    .get("membership_claim_refs")
                    .and_then(Value::as_array)
                    .is_some_and(|a| w.get("membership_claim_ref").is_some_and(|v| a.contains(v)))
                {
                    c.issue(
                        p,
                        "bibliographic Work membership claim is absent from its Collection record",
                    )?;
                }
            }
        } else {
            c.issue(
                p,
                "bibliographic Work container must route to the Collection spine",
            )?;
        }
        if let Some(record) = string(w, "container_ref") {
            match c.count_json_lines(
                &format!("{}/membership-claims.jsonl", parent(record)),
                |row| {
                    Ok(
                        py_equal(row.get("claim_id"), w.get("membership_claim_ref"))?
                            && py_equal(row.get("subject_ref"), w.get("container_id"))?
                            && string(row, "predicate") == Some("contains_work")
                            && py_equal(row.get("object"), w.get("work_id"))?,
                    )
                },
            ) {
                Err(e) if e.kind() == io::ErrorKind::Unsupported => return Err(e),
                Err(e) => c.issue(
                    p,
                    format!("cannot resolve bibliographic Work membership claims: {e}"),
                )?,
                Ok(exact) => {
                    if exact != 1 {
                        c.issue(p,"bibliographic Work planting lacks one exact Collection membership claim")?;
                    }
                }
            }
        }
    }
    Ok(())
}
pub fn run_validation(root: &Path, cancel: &AtomicI32) -> io::Result<Vec<Issue>> {
    let mut c = Context {
        source: RouteSources::new(root)?,
        cancel,
        issues: Vec::new(),
        issue_bytes: 48,
    };
    for path in [
        "ToS/philosophy/AGENTS.md",
        "ToS/philosophy/README.md",
        MANIFEST,
        "ToS/research-packets/AGENTS.md",
    ] {
        if !c.source.is_file(path)? {
            c.issue(path, "missing required philosophy topology file")?;
        }
    }
    let Some(manifest) = c.json(MANIFEST)? else {
        return Ok(c.issues);
    };
    for path in [
        "ToS/source-witnesses/notion",
        "ToS/source-witnesses/notion/philosophy",
    ] {
        if c.source.exists(path)? {
            c.issue(
                path,
                "AI/Notion research packets must not live under source-witnesses",
            )?;
        }
    }
    for (k, e) in [
        ("schema_version", "tos_philosophy_topology_v1"),
        ("branch_id", "philosophy"),
        ("path", "ToS/philosophy"),
    ] {
        c.equal(MANIFEST, &manifest, k, e, &format!("{k} must be {e}"))?;
    }
    if let Some(routes) = manifest.get("boundary_routes").filter(|v| v.is_object()) {
        if routes.get("source_page_witnesses").is_some() {
            c.issue(MANIFEST,"boundary_routes.source_page_witnesses must not route AI/Notion packets as source witnesses")?;
        }
        for (k, e) in [
            ("provisional_extraction", "ToS/candidate-intake"),
            ("research_packets", "ToS/research-packets"),
            ("source_witnesses", "ToS/source-witnesses"),
            ("canon_promotion", "ToS/canon"),
        ] {
            c.equal(
                MANIFEST,
                routes,
                k,
                e,
                &format!("boundary_routes.{k} must be {e}"),
            )?;
        }
    } else {
        c.issue(MANIFEST, "boundary_routes must be an object")?;
    }
    if let Some(policy) = manifest
        .get("path_component_policy")
        .filter(|v| v.is_object())
    {
        for k in ["repository_paths_describe", "metadata_only_inputs"] {
            if !policy
                .get(k)
                .and_then(Value::as_array)
                .is_some_and(|a| a.iter().all(Value::is_string))
            {
                c.issue(
                    MANIFEST,
                    format!("path_component_policy.{k} must be a string list"),
                )?;
            }
        }
    } else {
        c.issue(MANIFEST, "path_component_policy must be an object")?;
    }
    if manifest.get("source_witness_routes").is_some() {
        c.issue(
            MANIFEST,
            "source_witness_routes must not point to AI/Notion research packets",
        )?;
    }
    for k in ["mature_branch_shape", "promotion_pipeline"] {
        if !manifest.get(k).and_then(Value::as_array).is_some_and(|a| {
            !a.is_empty() && a.iter().all(|v| v.as_str().is_some_and(|s| !s.is_empty()))
        }) {
            c.issue(MANIFEST, format!("{k} must be a non-empty string list"))?;
        }
    }
    for (k, p) in [
        ("research_packet_contracts", "ToS/research-packets"),
        ("graph_view_routes", "ToS/philosophy/graph-workbench/views"),
        (
            "graph_view_contracts",
            "ToS/philosophy/graph-workbench/views",
        ),
        ("atlas_routes", "ToS/philosophy/atlas"),
    ] {
        routes(&mut c, &manifest, k, p)?;
    }
    let mut metadata_labels = BTreeSet::new();
    if let Some(entries) = manifest
        .get("research_packet_routes")
        .and_then(Value::as_array)
    {
        for entry in entries {
            let Some(entry) = entry.as_str().filter(|s| !s.is_empty()) else {
                c.issue(
                    MANIFEST,
                    "research_packet_routes entries must be non-empty strings",
                )?;
                continue;
            };
            let Some(path) = normalized(entry) else {
                c.issue(entry,"research packet routes must be normalized repo-relative paths under ToS/research-packets")?;
                continue;
            };
            if !under(&path, "ToS/research-packets") {
                c.issue(
                    entry,
                    "research packet routes must stay under ToS/research-packets",
                )?;
                continue;
            }
            if path.split('/').any(|p| p == "source-witnesses") {
                c.issue(
                    entry,
                    "research packet routes must not point into source-witnesses",
                )?;
                continue;
            }
            let agents = format!("{}/AGENTS.md", parent(&path));
            if !c.source.is_file(&agents)? {
                c.issue(&agents, "research packet route must have a local AGENTS.md")?;
            }
            let Some(packet) = c.json(&path)? else {
                continue;
            };
            metadata_labels.extend(labels(&packet));
            for (k, e, msg) in [
                (
                    "schema_version",
                    "tos_research_packet_v1",
                    "schema_version must be tos_research_packet_v1",
                ),
                (
                    "path",
                    parent(&path).as_str(),
                    "path must match the research packet directory",
                ),
                (
                    "domain_branch",
                    "ToS/philosophy",
                    "domain_branch must be ToS/philosophy",
                ),
            ] {
                c.equal(entry, &packet, k, e, msg)?;
            }
            if let Some(a) = packet.get("authority").filter(|v| v.is_object()) {
                for (k, e) in [
                    ("source_status", "not_source_witness"),
                    ("canon_status", "not_canon"),
                ] {
                    c.equal(entry, a, k, e, &format!("authority.{k} must be {e}"))?;
                }
            } else {
                c.issue(entry, "authority must be an object")?;
            }
            if !packet
                .get("capture_container")
                .filter(|v| v.is_object())
                .is_some_and(|v| truth(v.get("page_id")))
            {
                c.issue(entry, "capture_container.page_id is required")?;
            }
            if !packet
                .get("branch_child_pages")
                .and_then(Value::as_array)
                .is_some_and(|a| !a.is_empty())
            {
                c.issue(entry, "branch_child_pages must be a non-empty list")?;
            }
        }
    } else {
        c.issue(
            MANIFEST,
            "research_packet_routes must be a list when research packets are present",
        )?;
    }
    let mut declared = BTreeSet::new();
    if let Some(entries) = manifest
        .get("branch_manifests")
        .and_then(Value::as_array)
        .filter(|a| !a.is_empty())
    {
        let mut seen = BTreeSet::new();
        for entry in entries {
            let Some(entry) = entry.as_str().filter(|s| !s.is_empty()) else {
                c.issue(
                    MANIFEST,
                    "branch_manifests entries must be non-empty strings",
                )?;
                continue;
            };
            if !seen.insert(entry) {
                c.issue(MANIFEST, format!("duplicate branch manifest {entry}"))?;
            }
            declared.insert(parent(entry));
            check_labels(&mut c, entry, &metadata_labels)?;
            let Some(v) = c.json(entry)? else {
                continue;
            };
            c.equal(
                entry,
                &v,
                "path",
                &parent(entry),
                "branch manifest path must match its parent directory",
            )?;
            if !string(&v, "branch_id").is_some_and(|s| s.starts_with("philosophy.")) {
                c.issue(entry, "branch_id must start with philosophy.")?;
            }
            if !string(&v, "role").is_some_and(|s| !s.is_empty()) {
                c.issue(entry, "role must be a non-empty string")?;
            }
        }
    } else {
        c.issue(MANIFEST, "branch_manifests must be a non-empty list")?;
    }
    let paths = c.paths("ToS/philosophy/eras")?;
    let planting_paths: Vec<_> = paths
        .into_iter()
        .filter(|p| {
            let parts: Vec<_> = p.split('/').collect();
            parts.len() >= 7
                && parts[parts.len() - 4] == "sources"
                && parts[parts.len() - 3] == "plantings"
                && parts.last() == Some(&"source-planting.json")
        })
        .collect();
    let atlas_paths: Vec<_> = c
        .paths("ToS/philosophy/atlas/master-tables")?
        .into_iter()
        .filter(|p| {
            p.strip_prefix("ToS/philosophy/atlas/master-tables/")
                .is_some_and(|s| s.split('/').count() == 2 && s.ends_with("/rows.jsonl"))
        })
        .collect();
    let schema = if planting_paths.is_empty() {
        None
    } else {
        match c.source.text(SCHEMA)? {
            None => {
                c.issue(
                    SCHEMA,
                    "cannot load source-planting schema: missing JSON file",
                )?;
                None
            }
            Some(raw) => match c.decode(&raw) {
                Ok(schema) => Some(schema),
                Err(error) if error.kind() == io::ErrorKind::Unsupported => return Err(error),
                Err(error) => {
                    c.issue(
                        SCHEMA,
                        format!("cannot load source-planting schema: {error}"),
                    )?;
                    None
                }
            },
        }
    };
    let validator = match schema.as_ref() {
        Some(schema) => match jsonschema::options()
            .with_draft(jsonschema::Draft::Draft202012)
            .offline()
            .should_validate_formats(true)
            .with_format("date-time", |_| true)
            .with_format("uri", |_| true)
            .with_format("uri-reference", |_| true)
            .build(schema)
        {
            Ok(validator) => Some(validator),
            Err(error) => {
                c.issue(
                    SCHEMA,
                    format!("cannot load source-planting schema: {error}"),
                )?;
                None
            }
        },
        None => None,
    };
    let mut ids = BTreeSet::new();
    let mut branch_refs: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for path in planting_paths {
        let Some(plant) = c.json(&path)? else {
            continue;
        };
        if let Some(validator) = &validator {
            // Admit each retained diagnostic before growing the ordered list.
            // Schema array indexes compare numerically; object names lexically.
            let mut errors = Vec::new();
            let mut bytes = c.issue_bytes;
            for error in validator.iter_errors(&plant) {
                c.check()?;
                let parts: Vec<String> = error
                    .instance_path()
                    .segments()
                    .map(|s| s.to_string())
                    .collect();
                let suffix = parts.join(".");
                let location = if suffix.is_empty() {
                    path.clone()
                } else {
                    format!("{path}:{suffix}")
                };
                let message = format!("source-planting schema: {error}");
                bytes = bytes
                    .checked_add(location.len())
                    .and_then(|n| n.checked_add(message.len()))
                    .and_then(|n| n.checked_add(5))
                    .ok_or_else(|| {
                        io::Error::other("philosophy schema issue accounting overflow")
                    })?;
                if c.issues.len() + errors.len() >= 4096 || bytes > 1_048_576 {
                    return Err(io::Error::other(
                        "philosophy schema issue output bound exceeded",
                    ));
                }
                let order: Vec<_> = parts
                    .iter()
                    .map(|s| match s.parse::<usize>() {
                        Ok(i) => (0, i, String::new()),
                        Err(_) => (1, 0, s.clone()),
                    })
                    .collect();
                errors.push((order, location, message));
            }
            errors.sort_by(|a, b| a.0.cmp(&b.0));
            for (_, location, message) in errors {
                c.issue(&location, message)?;
            }
        }
        atlas_membership(&mut c, &plant, &path, &atlas_paths)?;
        if let Some(id) = string(&plant, "planting_id") {
            if !ids.insert(id.to_owned()) {
                c.issue(&path, format!("duplicate planting_id: {id}"))?;
            }
        }
        let expected = parent(&parent(&parent(&parent(&path))));
        if string(&plant, "branch_path") != Some(expected.as_str()) {
            c.issue(
                &path,
                "branch_path does not match the planting's philosophy branch",
            )?;
        }
        if let Some(branch) = string(&plant, "branch_path") {
            branch_refs
                .entry(branch.into())
                .or_default()
                .insert(path.clone());
        }
        if let Some(b) = plant.get("source_backlog_anchor").filter(|v| v.is_object()) {
            if let (Some(file), Some(line)) =
                (string(b, "path"), b.get("line").and_then(Value::as_i64))
            {
                let raw = c.source.text(file)?;
                let row = raw.as_ref().and_then(|s| {
                    let lines = splitlines(s);
                    let index = if line <= 0 {
                        lines.len() as i64 + line - 1
                    } else {
                        line - 1
                    };
                    usize::try_from(index)
                        .ok()
                        .and_then(|i| lines.get(i))
                        .copied()
                });
                if let Some(row) = row {
                    match c.decode(row) {
                        Ok(row) => {
                            for k in [
                                "atlas_row_id",
                                "dossier_id",
                                "branch_path",
                                "source_table_index",
                                "source_row_index",
                                "source_label",
                            ] {
                                let expected =
                                    if ["atlas_row_id", "dossier_id", "branch_path"].contains(&k) {
                                        plant.get(k)
                                    } else {
                                        b.get(k)
                                    };
                                if !py_equal(row.get(k), expected)? {
                                    c.issue(
                                        &path,
                                        format!(
                                            "source backlog {k} differs from exact line {line}"
                                        ),
                                    )?;
                                }
                            }
                        }
                        Err(e) => c.issue(
                            &path,
                            format!("cannot resolve exact source backlog row: {e}"),
                        )?,
                    }
                } else {
                    c.issue(
                        &path,
                        "cannot resolve exact source backlog row: missing file or line",
                    )?;
                }
            } else {
                c.issue(&path, "source backlog path and line are required")?;
            }
        }
        witness(&mut c, &plant, &path)?;
        for field in ["discovery_ref", "research_ref"] {
            if let Some(file) = string(&plant, field) {
                if !c.source.is_file(file)? {
                    c.issue(&path, format!("{field} does not resolve to a tracked file"))?;
                }
            } else {
                c.issue(&path, format!("{field} does not resolve to a tracked file"))?;
            }
        }
    }
    for branch in declared {
        branch_refs.entry(branch).or_default();
    }
    for (branch, actual) in branch_refs {
        for (label, suffix, key, count_key) in [
            (
                "branch",
                "branch.manifest.json",
                "source_planting_refs",
                "source_planting_count",
            ),
            (
                "sources",
                "sources/branch.manifest.json",
                "planting_refs",
                "planting_count",
            ),
        ] {
            let file = format!("{branch}/{suffix}");
            if actual.is_empty() && !c.source.is_file(&file)? {
                continue;
            }
            if let Some(v) = c.json(&file)? {
                if !actual.is_empty() || v.get(key).is_some() {
                    let refs = v
                        .get(key)
                        .and_then(Value::as_array)
                        .map(|a| {
                            a.iter()
                                .filter_map(Value::as_str)
                                .map(str::to_owned)
                                .collect::<BTreeSet<_>>()
                        })
                        .unwrap_or_default();
                    if refs != actual {
                        c.issue(
                            &format!("{branch}/{label}"),
                            format!("{key} differs from exact planting files"),
                        )?;
                    }
                }
                if (!actual.is_empty() || v.get(count_key).is_some())
                    && !py_equal(v.get(count_key), Some(&Value::from(actual.len() as u64)))?
                {
                    c.issue(
                        &format!("{branch}/{label}"),
                        format!("{count_key} differs from exact planting count"),
                    )?;
                }
            }
        }
    }
    for path in c.paths("ToS")? {
        check_labels(&mut c, &path, &metadata_labels)?;
    }
    c.check()?;
    Ok(c.issues)
}
pub fn run(root: &Path, cancel: &AtomicI32) -> io::Result<i32> {
    let issues = run_validation(root, cancel)?;
    if issues.is_empty() {
        writeln!(
            io::stdout().lock(),
            "[ok] validated ToS philosophy topology"
        )?;
        Ok(0)
    } else {
        let mut out = io::stderr().lock();
        writeln!(out, "Philosophy topology validation failed.")?;
        for (p, m) in issues {
            writeln!(out, "- {p}: {m}")?;
        }
        Ok(1)
    }
}
