//! Source-owned documentation atlas: one tracked inventory and one byte pass.
use crate::{
    executor::{self, Limits},
    route_cards::{self, RouteSources},
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicI32, Ordering},
    time::Duration,
};
use tos_foundation::{Digest256, Digest256Hasher};
pub const MAP_PATH: &str = "docs/validation/documentation_family_map.json";
pub const CURRENTNESS_PATH: &str = "docs/validation/documentation-family.current.json";
pub const TRACKED_SOURCE: &str = "git ls-files -z";
fn invalid(m: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, m.into())
}
fn check(s: &RouteSources, c: &AtomicI32) -> io::Result<()> {
    if c.load(Ordering::Relaxed) != 0 {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "documentation atlas cancelled",
        ));
    }
    s.remaining_time().map(|_| ())
}
fn strings(v: &Value) -> impl Iterator<Item = &str> {
    v.as_array().into_iter().flatten().filter_map(Value::as_str)
}
fn suffix(path: &str) -> String {
    Path::new(path)
        .extension()
        .and_then(|v| v.to_str())
        .map(|v| format!(".{v}"))
        .unwrap_or_default()
}
pub fn tracked_source_declaration() -> &'static str {
    TRACKED_SOURCE
}
pub fn tracked_paths(root: &Path, s: &RouteSources, cancel: &AtomicI32) -> io::Result<Vec<String>> {
    check(s, cancel)?;
    let remaining = s.remaining_time()?.min(Duration::from_secs(10));
    let (code, out, _) = executor::capture_ci_git(
        root,
        vec!["git".into(), "ls-files".into(), "-z".into()],
        Limits {
            command_wall: remaining,
            lane_wall: remaining,
            cleanup_grace: Duration::from_secs(1),
            output_bytes: 16 * 1024 * 1024,
        },
        cancel,
    )?;
    if code != 0 {
        return Err(invalid(
            "documentation tracked source requires readable Git index",
        ));
    }
    let mut paths = Vec::new();
    for raw in out.split(|b| *b == 0).filter(|b| !b.is_empty()) {
        let p = std::str::from_utf8(raw)
            .map_err(|_| invalid("documentation tracked path is not UTF-8"))?;
        if p.len() > 4096
            || !Path::new(p)
                .components()
                .all(|c| matches!(c, Component::Normal(_)))
        {
            return Err(invalid("invalid documentation tracked path"));
        }
        paths.push(p.to_owned());
        if paths.len() > 65536 {
            return Err(invalid("documentation tracked path bound exceeded"));
        }
    }
    check(s, cancel)?;
    Ok(paths)
}
fn projected(path: &str, rules: &Value) -> bool {
    !strings(&rules["exclude_paths"]).any(|p| p == path)
        && !strings(&rules["exclude_prefixes"]).any(|p| path.starts_with(p))
}
pub fn included_surface(path: &str, rules: &Value) -> bool {
    projected(path, rules) && strings(&rules["include_extensions"]).any(|e| e == suffix(path))
}
pub fn family_for(path: &str, families: &[Value]) -> Option<String> {
    for f in families {
        let m = &f["match"];
        if m["kind"] == "root_files" && Path::new(path).components().count() == 1 {
            return f["id"].as_str().map(str::to_owned);
        }
        if m["prefix"].as_str().is_some_and(|p| path.starts_with(p))
            && !strings(&m["exclude_prefixes"]).any(|p| path.starts_with(p))
        {
            return f["id"].as_str().map(str::to_owned);
        }
    }
    None
}
#[derive(Debug)]
enum GlobPart {
    Star,
    Any,
    Char(char),
    Class {
        negated: bool,
        ranges: Vec<(char, char)>,
    },
}
/// The map's case-sensitive shell glob; separators are ordinary characters,
/// matching fnmatchcase rather than filesystem recursive-glob traversal.
struct HumanExclusion(Vec<GlobPart>);
impl HumanExclusion {
    fn new(pattern: &str) -> io::Result<Self> {
        if pattern.len() > 4096 {
            return Err(invalid("documentation exclusion glob exceeds byte bound"));
        }
        let p: Vec<_> = pattern.chars().collect();
        let mut i = 0;
        let mut out = Vec::new();
        while i < p.len() {
            let c = p[i];
            i += 1;
            match c {
                '*' => {
                    if !matches!(out.last(), Some(GlobPart::Star)) {
                        out.push(GlobPart::Star);
                    }
                }
                '?' => out.push(GlobPart::Any),
                '[' => {
                    let start = i;
                    let mut j = i;
                    if j < p.len() && p[j] == '!' {
                        j += 1;
                    }
                    if j < p.len() && p[j] == ']' {
                        j += 1;
                    }
                    while j < p.len() && p[j] != ']' {
                        j += 1;
                    }
                    if j == p.len() {
                        out.push(GlobPart::Char('['));
                        continue;
                    }
                    let negated = p.get(start) == Some(&'!');
                    let mut k = start + usize::from(negated);
                    let mut ranges = Vec::new();
                    while k < j {
                        if k + 2 < j && p[k + 1] == '-' {
                            if p[k] <= p[k + 2] {
                                ranges.push((p[k], p[k + 2]));
                            }
                            k += 3;
                        } else {
                            ranges.push((p[k], p[k]));
                            k += 1;
                        }
                    }
                    out.push(GlobPart::Class { negated, ranges });
                    i = j + 1;
                }
                c => out.push(GlobPart::Char(c)),
            }
        }
        Ok(Self(out))
    }
    fn matches(&self, text: &str) -> bool {
        let text: Vec<_> = text.chars().collect();
        let (mut i, mut j) = (0, 0);
        let mut star = None;
        let mut retry = 0;
        while i < text.len() {
            let yes = match self.0.get(j) {
                Some(GlobPart::Any) => true,
                Some(GlobPart::Char(c)) => *c == text[i],
                Some(GlobPart::Class { negated, ranges }) => {
                    ranges.iter().any(|(a, b)| *a <= text[i] && text[i] <= *b) != *negated
                }
                _ => false,
            };
            if yes {
                i += 1;
                j += 1;
            } else if matches!(self.0.get(j), Some(GlobPart::Star)) {
                star = Some(j);
                retry = i;
                j += 1;
            } else if let Some(s) = star {
                retry += 1;
                i = retry;
                j = s + 1;
            } else {
                return false;
            }
        }
        while matches!(self.0.get(j), Some(GlobPart::Star)) {
            j += 1;
        }
        j == self.0.len()
    }
}
#[derive(Default)]
struct Count {
    count: u64,
    bytes: u64,
    lines: u64,
    kinds: BTreeMap<String, u64>,
}
pub fn build_currentness(root: &Path, cancel: &AtomicI32) -> io::Result<Value> {
    build_currentness_with_sources(root, &mut RouteSources::new(root)?, cancel)
}
pub fn build_currentness_with_sources(
    root: &Path,
    s: &mut RouteSources,
    cancel: &AtomicI32,
) -> io::Result<Value> {
    let map: Value = serde_json::from_slice(&s.bytes(MAP_PATH)?).map_err(io::Error::other)?;
    let paths = tracked_paths(root, s, cancel)?;
    build_currentness_with_tracked(root, s, &map, &paths, cancel)
}
pub fn build_currentness_with_tracked(
    _root: &Path,
    s: &mut RouteSources,
    map: &Value,
    tracked: &[String],
    cancel: &AtomicI32,
) -> io::Result<Value> {
    check(s, cancel)?;
    let families = map["families"]
        .as_array()
        .ok_or_else(|| invalid("documentation families must be an array"))?;
    let rules = &map["surface_rules"];
    let atlas = &map["atlas_method"];
    if atlas["tracked_source"] != TRACKED_SOURCE {
        return Err(invalid(format!(
            "atlas_method.tracked_source must match the builder operation: {TRACKED_SOURCE}"
        )));
    }
    let human: BTreeSet<_> = strings(&atlas["human_extensions"]).collect();
    let structured: BTreeSet<_> = strings(&atlas["structured_carrier_extensions"]).collect();
    let executable: BTreeSet<_> = strings(&atlas["executable_carrier_extensions"]).collect();
    let exclusion = HumanExclusion::new(
        atlas["human_exclusion"]
            .as_str()
            .ok_or_else(|| invalid("missing human_exclusion"))?,
    )?;
    let mut records = Vec::new();
    let mut unhandled = Vec::new();
    let mut kinds = BTreeMap::<String, u64>::new();
    let mut summaries = BTreeMap::<String, Count>::new();
    let mut humans = BTreeMap::<String, Count>::new();
    let (mut projection_count, mut human_excluded, mut used) = (0usize, 0u64, 0usize);
    for path in tracked {
        check(s, cancel)?;
        let extension = suffix(path);
        let is_human = human.contains(extension.as_str());
        let excluded_human = is_human && exclusion.matches(path);
        human_excluded += u64::from(excluded_human);
        let include = included_surface(path, rules);
        let in_human = is_human && !excluded_human;
        projection_count += usize::from(projected(path, rules));
        if !include && !in_human {
            continue;
        }
        let raw = s.bounded_bytes(path, 64 * 1024 * 1024, &mut used, 512 * 1024 * 1024)?;
        let family = family_for(path, families);
        let fid = family.as_deref().unwrap_or("unhandled");
        if in_human {
            let c = humans.entry(fid.into()).or_default();
            c.count += 1;
            c.bytes += raw.len() as u64;
            c.lines += raw.iter().filter(|b| **b == b'\n').count() as u64;
        }
        if include {
            if family.is_none() {
                unhandled.push(path.clone());
            }
            let kind = if is_human {
                "human"
            } else if structured.contains(extension.as_str()) {
                "structured"
            } else if executable.contains(extension.as_str()) {
                "executable"
            } else {
                "other"
            };
            *kinds.entry(kind.into()).or_default() += 1;
            let c = summaries.entry(fid.into()).or_default();
            c.count += 1;
            c.bytes += raw.len() as u64;
            *c.kinds.entry(kind.into()).or_default() += 1;
            records.push(json!({"path":path,"family_id":fid,"surface_kind":kind,"bytes":raw.len(),"sha256":Digest256::of_bytes(&raw).to_hex(),"currentness":"tracked_source_snapshot"}));
        }
    }
    records.sort_by(|a, b| a["path"].as_str().cmp(&b["path"].as_str()));
    unhandled.sort();
    let mut digest = Digest256Hasher::new();
    for r in &records {
        digest.update(r["path"].as_str().unwrap().as_bytes());
        digest.update(b"\0");
        digest.update(r["sha256"].as_str().unwrap().as_bytes());
        digest.update(b"\0");
    }
    let mut family_summaries = Vec::new();
    for f in families {
        let id = f["id"]
            .as_str()
            .ok_or_else(|| invalid("family id must be text"))?;
        let empty = Count::default();
        let c = summaries.get(id).unwrap_or(&empty);
        family_summaries.push(json!({"family_id":id,"tracked_surface_count":c.count,"tracked_surface_bytes":c.bytes,"surface_kind_counts":c.kinds}));
    }
    let mut human_counts = BTreeMap::new();
    let (mut hc, mut hb, mut hl) = (0u64, 0u64, 0u64);
    for (id, c) in humans {
        hc += c.count;
        hb += c.bytes;
        hl += c.lines;
        human_counts.insert(id, json!({"count":c.count,"bytes":c.bytes,"lines":c.lines}));
    }
    let excluded_rules = strings(&rules["exclude_paths"])
        .collect::<BTreeSet<_>>()
        .len()
        + strings(&rules["exclude_prefixes"])
            .collect::<BTreeSet<_>>()
            .len();
    check(s, cancel)?;
    Ok(
        json!({"schema_version":"tos_documentation_family_currentness_v1","source_map":MAP_PATH,"source_map_sha256":Digest256::of_bytes(&s.bytes(MAP_PATH)?).to_hex(),"generated_by":"scripts/build_documentation_family_currentness.py","tracked_source":TRACKED_SOURCE,"tracked_surface_digest":digest.finalize().to_hex(),"atlas_method":atlas,
 "coverage":{"tracked_path_count":projection_count,"tracked_surface_count":records.len(),"excluded_tracked_path_count":projection_count.checked_sub(records.len()).ok_or_else(||invalid("documentation coverage count underflow"))?,"excluded_generated_carrier_rules":excluded_rules,"unhandled_family_count":unhandled.len(),"unhandled_paths":unhandled,"surface_kind_counts":kinds,"human_scope":{"count":hc,"bytes":hb,"lines":hl,"family_counts":human_counts,"excluded_skill_launch_metadata":human_excluded}},
 "family_summaries":family_summaries,"context_summary":{"metric":"whitespace_tokens_v1","posture":"summary_first_records_on_demand","tracked_surface_count":records.len(),"family_count":families.len(),"unhandled_family_count":unhandled.len(),"record_loading":"machine readers may select records by family_id or path; do not load the full carrier into an always-on prompt"},"tracked_surfaces":records}),
    )
}
pub fn render_currentness(value: &Value) -> io::Result<String> {
    route_cards::render_currentness(value)
}
pub fn run(
    root: &Path,
    output: Option<&Path>,
    check_only: bool,
    cancel: &AtomicI32,
) -> io::Result<i32> {
    let target = output
        .map(|p| {
            if p.is_absolute() {
                p.to_owned()
            } else {
                root.join(p)
            }
        })
        .unwrap_or_else(|| root.join(CURRENTNESS_PATH));
    // Explicit output paths retain the CLI's relative-parent spelling. Resolve
    // dot components before the existing no-symlink output writer opens them.
    let mut resolved = PathBuf::new();
    for component in target.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                resolved.pop();
            }
            component => resolved.push(component.as_os_str()),
        }
    }
    let shown = target.strip_prefix(root).unwrap_or(&target).display();
    let rendered = render_currentness(&build_currentness(root, cancel)?)?;
    if check_only {
        if route_cards::read_output(&resolved)?
            .is_none_or(|v| v.replace("\r\n", "\n").replace('\r', "\n") != rendered)
        {
            println!("documentation family currentness is stale or missing: {shown}");
            return Ok(1);
        }
        println!("documentation family currentness is current: {shown}");
    } else {
        route_cards::write_output(root, &resolved, &rendered)?;
        println!("wrote {shown}");
    }
    Ok(0)
}
