//! Native cross-corpus documentation guards.
//! Local script references reuse the admitted tracked atlas and inventory roots.
//! Physical nested-card checking separately prices the existing wider traversal
//! ceiling, streaming only card names while refusing untracked cards. Ordinary
//! route discovery/card counts, custody, byte/path/depth and clock laws remain.
//! Source-owned cross-corpus documentation guards and owner-validator coordination.
use crate::{
    documentation_family as family,
    mechanics_topology::{self, MarkdownRules},
    route_cards::{self, RouteSources},
};
use num_bigint::BigInt;
use regex::Regex;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    path::{Path, PathBuf},
    sync::atomic::AtomicI32,
};
use tos_foundation::{
    JsonLimits, JsonMode, emit_python_compact_json, parse_json, python_lower_unicode16_v1,
};
pub type Issue = (String, String);
const EXPECTED_FAMILY_IDS: &[&str] = &[
    "access",
    "agents-other",
    "agents-skills",
    "docs",
    "evals",
    "github",
    "kag",
    "manifests",
    "mechanics",
    "memo",
    "quests",
    "root",
    "rust",
    "scripts",
    "stats",
    "tests",
    "tos",
];
const ROLE_KEYS: &[&str] = &[
    "authored",
    "executable",
    "generated",
    "receipt",
    "runtime",
    "tool",
];
const MARKDOWN_SUFFIXES: &[&str] = &[".md", ".txt", ".yaml", ".yml"];
const EXTERNAL_OWNER_MARKERS: &[&str] = &[
    "aoa-kag",
    "aoa-evals",
    "aoa-memo",
    "aoa_kag_root",
    "aoa_evals_root",
    "aoa_memo_root",
    "external owner",
    "stronger owner",
];
const CONTEXT_MEASURES: &[&str] = &[
    "agents_route_max_inherited",
    "generated_summary",
    "max",
    "sum",
];
const EXPECTED_SURFACE_EXTENSIONS: &[&str] =
    &[".json", ".md", ".py", ".sh", ".txt", ".yaml", ".yml"];
const EXPECTED_CONTEXT_PROBE_IDS: &[&str] = &[
    "inherited_agents_stacks",
    "machine_projection_summary",
    "mechanics_entry",
    "public_entry",
    "root_entry",
    "skill_discovery",
];
fn expected_context_probes() -> Value {
    json!({"root_entry": {"surfaces": ["README.md"], "measure": "sum", "max_tokens": 700}, "inherited_agents_stacks": {"surfaces": [".agents/agents-route.current.json"], "measure": "agents_route_max_inherited", "max_tokens": 2800}, "mechanics_entry": {"surfaces": ["mechanics/README.md"], "measure": "sum", "max_tokens": 1400}, "skill_discovery": {"surfaces": [".agents/README.md"], "measure": "max", "max_tokens": 1800}, "public_entry": {"surfaces": ["ToS/zarathustra/public-entry/TINY_ENTRY_ROUTE.md"], "measure": "sum", "max_tokens": 1200}, "machine_projection_summary": {"surfaces": ["docs/validation/documentation-family.current.json"], "measure": "generated_summary", "max_tokens": 1600}})
}
const EXPECTED_PUBLIC_AUTHORED_SURFACES: &[&str] = &[
    ".agents/README.md",
    "AGENTS.md",
    "README.md",
    "ToS/README.md",
    "ToS/zarathustra/public-entry/TINY_ENTRY_ROUTE.md",
    "docs/validation/README.md",
];
const EXPECTED_PUBLIC_FORBIDDEN_MARKERS: &[&str] = &[
    "/home/",
    "/runtime-state/",
    "/srv/",
    "/tmp/",
    "AWS_SECRET_ACCESS_KEY",
    "BEGIN PRIVATE KEY",
    "OPENAI_API_KEY",
    "file://",
    "holder_pid",
    "provider_credentials",
    "provider_secret",
    "session_id",
    "terminal_pid",
];
fn expected_family_matches() -> Value {
    json!({"root": {"kind": "root_files"}, "agents-skills": {"prefix": ".agents/skills/"}, "agents-other": {"prefix": ".agents/", "exclude_prefixes": [".agents/skills/"]}, "github": {"prefix": ".github/"}, "tos": {"prefix": "ToS/"}, "access": {"prefix": "access/"}, "docs": {"prefix": "docs/"}, "evals": {"prefix": "evals/"}, "stats": {"prefix": "stats/"}, "kag": {"prefix": "kag/"}, "mechanics": {"prefix": "mechanics/"}, "scripts": {"prefix": "scripts/"}, "tests": {"prefix": "tests/"}, "manifests": {"prefix": "manifests/"}, "memo": {"prefix": "memo/"}, "quests": {"prefix": "quests/"}, "rust": {"prefix": "rust/"}})
}
const EXPECTED_GENERATED_CARRIER_PATHS: &[&str] =
    &["docs/validation/documentation-family.current.json"];
const EXPECTED_GENERATED_CARRIER_PREFIXES: &[&str] =
    &["kag/indexes/", "kag/receipts/index_family_budget/"];
fn canonical_source_map_routes() -> Value {
    json!({"owner_surface": "docs/validation/README.md", "schema_ref": "docs/validation/documentation-family-map.schema.json", "currentness_schema_ref": "docs/validation/documentation-family-currentness.schema.json", "generated_currentness": "docs/validation/documentation-family.current.json", "builder": "rust/crates/tos-ops-mechanics-plan/src/documentation_family.rs", "validator": "rust/crates/tos-ops-mechanics-plan/src/documentation_cross_corpus.rs"})
}
static NULL: Value = Value::Null;
fn value<'a>(v: &'a Value, k: &str) -> &'a Value {
    v.get(k).unwrap_or(&NULL)
}
fn text<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
    value(v, k).as_str()
}
fn array(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn names(v: &Value) -> BTreeSet<String> {
    array(v)
        .iter()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect()
}
fn set(v: &[&str]) -> BTreeSet<String> {
    v.iter().map(|v| v.to_string()).collect()
}
fn invalid(v: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, v.into())
}
fn issue(out: &mut Vec<Issue>, location: &str, message: impl Into<String>) -> io::Result<()> {
    let message = message.into();
    let bytes = out
        .iter()
        .try_fold(0usize, |n, (l, m)| {
            n.checked_add(l.len()).and_then(|n| n.checked_add(m.len()))
        })
        .and_then(|n| n.checked_add(location.len()))
        .and_then(|n| n.checked_add(message.len()));
    if out.len() >= 4096 || message.len() > 8192 || bytes.is_none_or(|n| n > 16 * 1024 * 1024) {
        return Err(invalid(
            "cross-corpus documentation diagnostic bound exceeded",
        ));
    }
    out.push((location.into(), message));
    Ok(())
}
fn quote(v: &str) -> String {
    format!(
        "'{}'",
        v.replace('\\', "\\\\")
            .replace('\'', "\\'")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
    )
}
fn repr(v: &Value) -> String {
    match v {
        Value::Null => "None".into(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::String(v) => quote(v),
        _ => v.to_string(),
    }
}
fn sorted(v: &BTreeSet<String>) -> String {
    format!(
        "[{}]",
        v.iter().map(|v| quote(v)).collect::<Vec<_>>().join(", ")
    )
}
fn safe(v: &str) -> bool {
    !v.starts_with('/') && !v.split('/').any(|v| v == "..") && !v.contains("//")
}
fn integer(v: &Value) -> Option<BigInt> {
    match v {
        Value::Number(n) => n.to_string().parse().ok(),
        Value::Bool(b) => Some(BigInt::from(if *b { 1 } else { 0 })),
        _ => None,
    }
}
fn positive(v: &Value) -> bool {
    integer(v).is_some_and(|n| n > BigInt::from(0))
}

fn string_list(v: &Value, nonempty: bool) -> bool {
    v.is_array()
        && (!nonempty || !array(v).is_empty())
        && array(v)
            .iter()
            .all(|v| v.as_str().is_some_and(|s| !s.is_empty()))
}
fn dotted_list(v: &Value) -> bool {
    string_list(v, true)
        && array(v)
            .iter()
            .all(|v| v.as_str().unwrap().starts_with('.'))
        && names(v).len() == array(v).len()
}
pub fn validate_surface_rules(rules: &Value, issues: &mut Vec<Issue>) -> io::Result<()> {
    let loc = format!("{}#surface_rules", family::MAP_PATH);
    if !rules.is_object() {
        issue(issues, &loc, "surface_rules must be an object")?;
        return Ok(());
    }
    let extensions = value(rules, "include_extensions");
    if !dotted_list(extensions) || names(extensions) != set(EXPECTED_SURFACE_EXTENSIONS) {
        issue(
            issues,
            &loc,
            format!(
                "include_extensions must be the explicit set {}",
                sorted(&set(EXPECTED_SURFACE_EXTENSIONS))
            ),
        )?;
    }
    for field in [
        "exclude_paths",
        "generated_carrier_paths",
        "exclude_prefixes",
        "generated_carrier_prefixes",
    ] {
        let values = value(rules, field);
        if !string_list(values, field.starts_with("generated_carrier_")) {
            issue(
                issues,
                &loc,
                format!("{field} must be a list of non-empty strings"),
            )?;
            continue;
        }
        for v in array(values) {
            let item = v.as_str().unwrap();
            if !safe(item) {
                issue(
                    issues,
                    &loc,
                    format!("{field} contains an unsafe path: {item}"),
                )?;
            }
            if field.ends_with("prefixes") && !item.ends_with('/') {
                issue(
                    issues,
                    &loc,
                    format!("exclude prefix must end with '/': {item}"),
                )?;
            }
        }
    }
    for (exclusions, carriers) in [
        ("exclude_paths", "generated_carrier_paths"),
        ("exclude_prefixes", "generated_carrier_prefixes"),
    ] {
        if value(rules, exclusions).is_array() && value(rules, carriers).is_array() {
            let extra = names(value(rules, exclusions))
                .difference(&names(value(rules, carriers)))
                .cloned()
                .collect::<Vec<_>>();
            if !extra.is_empty() {
                issue(
                    issues,
                    &loc,
                    format!(
                        "{exclusions} must stay within {carriers}: {}",
                        extra.join(", ")
                    ),
                )?;
            }
        }
    }
    for (field, expected, verb) in [
        (
            "generated_carrier_paths",
            set(EXPECTED_GENERATED_CARRIER_PATHS),
            "name",
        ),
        (
            "generated_carrier_prefixes",
            set(EXPECTED_GENERATED_CARRIER_PREFIXES),
            "name",
        ),
        (
            "exclude_paths",
            set(EXPECTED_GENERATED_CARRIER_PATHS),
            "retain",
        ),
        (
            "exclude_prefixes",
            set(EXPECTED_GENERATED_CARRIER_PREFIXES),
            "retain",
        ),
    ] {
        let v = value(rules, field);
        if v.is_array() && array(v).iter().all(Value::is_string) && names(v) != expected {
            issue(
                issues,
                &loc,
                format!("{field} must {verb} the canonical generated carriers"),
            )?;
        }
    }
    Ok(())
}
pub fn validate_context_probe_configuration(
    probes: &Value,
    issues: &mut Vec<Issue>,
) -> io::Result<()> {
    let loc = format!("{}#context_probes", family::MAP_PATH);
    if !probes.is_array() || array(probes).is_empty() {
        issue(issues, &loc, "context_probes must be a non-empty list")?;
        return Ok(());
    }
    let mut ids = Vec::new();
    for (index, probe) in array(probes).iter().enumerate() {
        if !probe.is_object() {
            issue(issues, &loc, format!("probe {index} must be an object"))?;
            continue;
        }
        let id = text(probe, "id");
        if id.is_none_or(str::is_empty) {
            issue(issues, &loc, format!("probe {index} needs a non-empty id"))?;
        } else {
            ids.push(id.unwrap().to_string());
        }
        let probe_loc = format!(
            "{loc}#{}",
            id.filter(|s| !s.is_empty())
                .map(str::to_owned)
                .unwrap_or_else(|| index.to_string())
        );
        if !string_list(value(probe, "surfaces"), true) {
            issue(
                issues,
                &probe_loc,
                "surfaces must be a non-empty list of strings",
            )?;
        }
        if text(probe, "measure").is_none_or(|v| !CONTEXT_MEASURES.contains(&v)) {
            issue(
                issues,
                &probe_loc,
                format!(
                    "unsupported context measure: {}",
                    repr(value(probe, "measure"))
                ),
            )?;
        }
    }
    let id_set: BTreeSet<String> = ids.iter().cloned().collect();
    if id_set.len() != ids.len() {
        issue(issues, &loc, "context probe ids must be unique")?;
    }
    let missing = set(EXPECTED_CONTEXT_PROBE_IDS)
        .difference(&id_set)
        .cloned()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        issue(
            issues,
            &loc,
            format!("missing required context probes: {}", missing.join(", ")),
        )?;
    }
    let expected = expected_context_probes();
    for (id, contract) in expected.as_object().unwrap() {
        if let Some(probe) = array(probes)
            .iter()
            .rev()
            .find(|p| text(p, "id") == Some(id))
        {
            let probe_loc = format!("{loc}#{id}");
            if value(probe, "surfaces") != value(contract, "surfaces") {
                issue(
                    issues,
                    &probe_loc,
                    "surfaces do not match the canonical context contract",
                )?;
            }
            if value(probe, "measure") != value(contract, "measure") {
                issue(
                    issues,
                    &probe_loc,
                    format!(
                        "measure does not match the canonical context contract: {}",
                        text(contract, "measure").unwrap()
                    ),
                )?;
            }
            if value(probe, "max_tokens") != value(contract, "max_tokens") {
                issue(
                    issues,
                    &probe_loc,
                    format!(
                        "max_tokens does not match the canonical context contract: {}",
                        value(contract, "max_tokens")
                    ),
                )?;
            }
        }
    }
    Ok(())
}
pub fn validate_family_match_rules(families: &Value, issues: &mut Vec<Issue>) -> io::Result<()> {
    let loc = format!("{}#families", family::MAP_PATH);
    if !families.is_array() {
        return Ok(());
    }
    let mut roots = 0;
    let mut rules = Vec::new();
    let expected = expected_family_matches();
    for f in array(families) {
        if !f.is_object() {
            continue;
        }
        let id = text(f, "id").unwrap_or("<unknown>");
        let m = value(f, "match");
        let here = format!("{loc}#{id}");
        if !m.is_object() {
            issue(issues, &here, "match must be an object")?;
            continue;
        }
        if expected.get(id).is_none() {
            issue(issues, &here, "family id has no canonical matcher")?;
        } else if expected.get(id) != Some(m) {
            issue(
                issues,
                &here,
                "family match does not match its canonical matcher",
            )?;
        }
        if text(m, "kind") == Some("root_files") {
            roots += 1;
            continue;
        }
        let Some(prefix) =
            text(m, "prefix").filter(|s| !s.is_empty() && s.ends_with('/') && safe(s))
        else {
            issue(
                issues,
                &here,
                "prefix must be a non-empty relative directory boundary",
            )?;
            continue;
        };
        let exclusions = value(m, "exclude_prefixes");
        if !exclusions.is_null()
            && (!exclusions.is_array()
                || !array(exclusions).iter().all(|v| {
                    v.as_str().is_some_and(|s| {
                        s.ends_with('/') && !s.starts_with('/') && !s.split('/').any(|p| p == "..")
                    })
                }))
        {
            issue(
                issues,
                &here,
                "exclude_prefixes must contain safe directory boundaries",
            )?;
        }
        rules.push((prefix, id, m));
    }
    if roots != 1 {
        issue(
            issues,
            &loc,
            "families must declare exactly one root_files match",
        )?;
    }
    for (i, (left, left_id, left_match)) in rules.iter().enumerate() {
        for (right, right_id, right_match) in &rules[i + 1..] {
            if left == right {
                issue(
                    issues,
                    &loc,
                    format!("families have duplicate match prefix: {left_id}, {right_id}"),
                )?;
                continue;
            }
            let broad = if left.starts_with(right) {
                Some((*right, *right_id, *right_match, *left, *left_id))
            } else if right.starts_with(left) {
                Some((*left, *left_id, *left_match, *right, *right_id))
            } else {
                None
            };
            if let Some((_, broad_id, broad_match, narrow, narrow_id)) = broad {
                if !array(value(broad_match, "exclude_prefixes"))
                    .iter()
                    .any(|v| v.as_str().is_some_and(|s| narrow.starts_with(s)))
                {
                    issue(
                        issues,
                        &loc,
                        format!(
                            "overlapping family prefixes require an explicit exclusion: {broad_id} and {narrow_id}"
                        ),
                    )?;
                }
            }
        }
    }
    Ok(())
}
pub fn validate_authority_declarations(
    declarations: &Value,
    issues: &mut Vec<Issue>,
    location: &str,
) -> io::Result<()> {
    let mut seen = BTreeMap::new();
    for (index, d) in array(declarations).iter().enumerate() {
        if !d.is_object() {
            issue(
                issues,
                location,
                format!("declaration {index} must be an object"),
            )?;
            continue;
        }
        let (Some(id), Some(owner), Some(strength)) = (
            text(d, "id").filter(|s| !s.is_empty()),
            text(d, "owner").filter(|s| !s.is_empty()),
            text(d, "strength").filter(|s| !s.is_empty()),
        ) else {
            issue(
                issues,
                location,
                format!("declaration {index} needs id, owner, and strength"),
            )?;
            continue;
        };
        let f = value(d, "family_id");
        let o = value(d, "family_owner");
        if f.is_null() != o.is_null()
            || !f.is_null()
                && (f.as_str().is_none_or(str::is_empty) || o.as_str().is_none_or(str::is_empty))
        {
            issue(
                issues,
                location,
                format!("declaration {id} needs a complete family_id/family_owner binding"),
            )?;
        }
        if let Some(previous) = seen.get(id) {
            issue(
                issues,
                location,
                format!(
                    "{} authority declaration: {id}",
                    if previous == &(owner, strength) {
                        "duplicate"
                    } else {
                        "conflicting"
                    }
                ),
            )?;
        } else {
            seen.insert(id, (owner, strength));
        }
    }
    Ok(())
}
pub fn validate_family_authority_claims(
    families: &Value,
    declarations: &Value,
    issues: &mut Vec<Issue>,
) -> io::Result<()> {
    let declarations_by_id: BTreeMap<_, _> = array(declarations)
        .iter()
        .filter_map(|d| text(d, "id").map(|id| (id, d)))
        .collect();
    let families_by_id: BTreeMap<_, _> = array(families)
        .iter()
        .filter_map(|d| text(d, "id").map(|id| (id, d)))
        .collect();
    let mut referenced = BTreeMap::<String, BTreeSet<String>>::new();
    for f in array(families) {
        if !f.is_object() {
            continue;
        }
        let id = text(f, "id").unwrap_or("<unknown>");
        let loc = format!("{}#{id}", family::MAP_PATH);
        let keys = value(f, "authority_keys");
        if !string_list(keys, true) {
            issue(
                issues,
                &loc,
                "authority_keys must be a non-empty list of strings",
            )?;
            continue;
        }
        for key in array(keys) {
            let key = key.as_str().unwrap();
            referenced.entry(id.into()).or_default().insert(key.into());
            let Some(d) = declarations_by_id.get(key) else {
                issue(
                    issues,
                    &loc,
                    format!("authority key has no declaration: {key}"),
                )?;
                continue;
            };
            if value(d, "family_id") != value(f, "id") {
                issue(
                    issues,
                    &loc,
                    format!(
                        "authority key {key} is bound to family {}, not {}",
                        repr(value(d, "family_id")),
                        repr(value(f, "id"))
                    ),
                )?;
            }
            if value(d, "family_owner") != value(f, "owner") {
                issue(
                    issues,
                    &loc,
                    format!("family owner conflicts with declaration {key}"),
                )?;
            }
        }
    }
    for d in array(declarations) {
        if !d.is_object() || value(d, "family_id").is_null() {
            continue;
        }
        let id = text(d, "family_id").unwrap_or("");
        if !families_by_id.contains_key(id) {
            issue(
                issues,
                "authority_declarations",
                format!("family-bound declaration references unknown family: {id}"),
            )?;
            continue;
        }
        let declaration_id = text(d, "id");
        if declaration_id.is_none_or(|key| !referenced.get(id).is_some_and(|set| set.contains(key)))
        {
            issue(
                issues,
                &format!("{}#{id}", family::MAP_PATH),
                format!(
                    "family-bound declaration is not referenced by authority_keys: {}",
                    declaration_id
                        .map(str::to_string)
                        .unwrap_or_else(|| repr(value(d, "id")))
                ),
            )?;
        }
    }
    Ok(())
}
fn read_json(s: &mut RouteSources, path: &str) -> io::Result<Value> {
    let raw = s.bytes(path)?;
    parse_json(
        &raw,
        JsonMode::RequestLastWins,
        JsonLimits {
            max_bytes: 8 * 1024 * 1024,
            max_depth: 64,
            max_visits: 65536,
            max_integer_digits: 4300,
        },
    )
    .map_err(io::Error::other)?;
    serde_json::from_slice(&raw).map_err(io::Error::other)
}
fn glob_match(pattern: &str, path: &str) -> bool {
    let mut re = String::from("^");
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '*' => {
                if chars.get(i + 1) == Some(&'*') {
                    i += 1;
                    if chars.get(i + 1) == Some(&'/') {
                        i += 1;
                        re.push_str("(?:.*/)?");
                    } else {
                        re.push_str(".*");
                    }
                } else {
                    re.push_str("[^/]*");
                }
            }
            '?' => re.push_str("[^/]"),
            c => re.push_str(&regex::escape(&c.to_string())),
        }
        i += 1;
    }
    re.push('$');
    Regex::new(&re).is_ok_and(|r| r.is_match(path))
}
fn glob_exists(s: &mut RouteSources, pattern: &str) -> io::Result<bool> {
    if !safe(pattern) {
        return Err(invalid("unsafe documentation currentness glob"));
    }
    let prefix = pattern
        .split('/')
        .take_while(|part| !part.contains(['*', '?', '[']))
        .collect::<Vec<_>>()
        .join("/");
    let root = if prefix.is_empty() { "." } else { &prefix };
    if !s.is_dir(root)? {
        return Ok(false);
    }
    Ok(s.paths(root)?
        .into_iter()
        .any(|path| glob_match(pattern, &path)))
}
pub fn validate_source_map(
    root: &Path,
    s: &mut RouteSources,
    p: &Value,
    issues: &mut Vec<Issue>,
) -> io::Result<()> {
    let required = [
        "schema_version",
        "owner_repo",
        "owner_surface",
        "schema_ref",
        "currentness_schema_ref",
        "generated_currentness",
        "builder",
        "validator",
        "atlas_method",
        "surface_rules",
        "families",
        "authority_declarations",
        "public_authored_surfaces",
        "public_forbidden_markers",
        "context_probes",
        "guard_families",
        "projection_decision",
    ];
    let missing = required
        .iter()
        .filter(|k| p.get(**k).is_none())
        .copied()
        .collect::<BTreeSet<_>>();
    if !missing.is_empty() {
        issue(
            issues,
            family::MAP_PATH,
            format!(
                "missing required fields: {}",
                missing.into_iter().collect::<Vec<_>>().join(", ")
            ),
        )?;
        return Ok(());
    }
    if text(p, "schema_version") != Some("tos_documentation_family_map_v1") {
        issue(issues, family::MAP_PATH, "unsupported schema_version")?;
    }
    if text(p, "owner_repo") != Some("Tree-of-Sophia") {
        issue(
            issues,
            family::MAP_PATH,
            "owner_repo must be Tree-of-Sophia",
        )?;
    }
    for (k, expected) in canonical_source_map_routes().as_object().unwrap() {
        if value(p, k) != expected {
            issue(
                issues,
                family::MAP_PATH,
                format!(
                    "{k} must remain the canonical route: {}",
                    expected.as_str().unwrap()
                ),
            )?;
        }
    }
    for key in [
        "owner_surface",
        "schema_ref",
        "currentness_schema_ref",
        "generated_currentness",
        "builder",
        "validator",
    ] {
        match text(p, key).filter(|s| !s.is_empty()) {
            None => issue(
                issues,
                family::MAP_PATH,
                format!("{key} must be a non-empty path"),
            )?,
            Some(route) if !route.starts_with("external:") => {
                if !s.exists(route)? {
                    issue(
                        issues,
                        family::MAP_PATH,
                        format!("{key} points to missing path: {route}"),
                    )?;
                }
            }
            _ => {}
        }
    }
    let atlas = value(p, "atlas_method");
    let atlas_loc = format!("{}#atlas_method", family::MAP_PATH);
    if text(atlas, "tracked_source") != Some(family::tracked_source_declaration()) {
        issue(
            issues,
            &atlas_loc,
            format!(
                "tracked_source must match the builder operation: {}",
                family::tracked_source_declaration()
            ),
        )?;
    }
    let groups = [
        "human_extensions",
        "structured_carrier_extensions",
        "executable_carrier_extensions",
    ];
    let mut extension_sets = BTreeMap::new();
    for name in groups {
        let values = value(atlas, name);
        if !dotted_list(values) {
            issue(
                issues,
                &atlas_loc,
                format!("{name} must be a non-empty unique list of dotted strings"),
            )?;
        } else {
            extension_sets.insert(name, names(values));
        }
    }
    if extension_sets.len() == groups.len() {
        for (left, values) in &extension_sets {
            for (right, others) in &extension_sets {
                if left < right && !values.is_disjoint(others) {
                    issue(
                        issues,
                        &atlas_loc,
                        format!("carrier extension groups overlap: {left}, {right}"),
                    )?;
                }
            }
        }
        let included = value(value(p, "surface_rules"), "include_extensions");
        if included.is_array() {
            let all = extension_sets
                .values()
                .flat_map(|v| v.iter().cloned())
                .collect::<BTreeSet<_>>();
            if all != names(included) {
                issue(
                    issues,
                    &atlas_loc,
                    "atlas extension groups must exactly cover surface_rules.include_extensions",
                )?;
            }
        }
    }
    if text(atlas, "human_exclusion")
        .is_none_or(|v| v.is_empty() || v.starts_with('/') || v.split('/').any(|v| v == ".."))
    {
        issue(
            issues,
            &atlas_loc,
            "human_exclusion must be a non-empty safe relative glob",
        )?;
    }
    let lane_path = "docs/validation/validation_lanes.json";
    let lanes = match read_json(s, lane_path) {
        Ok(lanes) => lanes,
        Err(e) => {
            issue(
                issues,
                &root.join(lane_path).to_string_lossy(),
                format!("cannot load validation lane authority: {e}"),
            )?;
            Value::Null
        }
    };
    let declared = value(&lanes, "lanes");
    if !declared.is_object() {
        issue(
            issues,
            &root.join(lane_path).to_string_lossy(),
            "validation lane authority must declare a lanes object",
        )?;
    }
    let families = value(p, "families");
    if !families.is_array() {
        issue(issues, family::MAP_PATH, "families must be a list")?;
        return Ok(());
    }
    let mut ids = Vec::new();
    for f in array(families) {
        if !f.is_object() {
            issue(issues, family::MAP_PATH, "each family must be an object")?;
            continue;
        }
        let Some(id) = text(f, "id").filter(|s| !s.is_empty()) else {
            issue(
                issues,
                family::MAP_PATH,
                "family id must be a non-empty string",
            )?;
            continue;
        };
        ids.push(id.to_string());
        let loc = format!("{}#{id}", family::MAP_PATH);
        for field in [
            "label",
            "match",
            "consumer",
            "load_moment",
            "owner",
            "roles",
            "currentness_inputs",
            "next_organ",
            "context",
            "public_safety",
            "validation_lane",
            "authority_keys",
        ] {
            if f.get(field).is_none() {
                issue(issues, &loc, format!("missing family field: {field}"))?;
            }
        }
        match text(f, "validation_lane").filter(|s| !s.is_empty()) {
            None => issue(issues, &loc, "validation_lane must be a non-empty string")?,
            Some(lane) if declared.is_object() && declared.get(lane).is_none() => issue(
                issues,
                &loc,
                format!("validation_lane is not in command authority: {lane}"),
            )?,
            _ => {}
        }
        let roles = value(f, "roles");
        if !roles.is_object()
            || roles
                .as_object()
                .map(|v| v.keys().cloned().collect::<BTreeSet<_>>())
                != Some(set(ROLE_KEYS))
        {
            issue(
                issues,
                &loc,
                "roles must name authored/generated/executable/runtime/tool/receipt exactly",
            )?;
        }
        let context = value(f, "context");
        if !context.is_object()
            || !matches!(
                text(context, "posture"),
                Some(
                    "always_on"
                        | "always_on_entry_then_on_demand"
                        | "triggered_or_on_demand"
                        | "deep_on_demand"
                        | "on_demand"
                        | "tool_on_demand"
                        | "internal_port"
                )
            )
        {
            issue(issues, &loc, "context posture is invalid")?;
        } else if !positive(value(context, "max_tokens")) {
            issue(issues, &loc, "context max_tokens must be positive")?;
        }
        let safety = value(f, "public_safety");
        if !safety.is_object()
            || text(safety, "posture").is_none()
            || !value(safety, "forbidden").is_array()
        {
            issue(
                issues,
                &loc,
                "public_safety must declare posture and forbidden markers",
            )?;
        }
        for v in array(value(f, "currentness_inputs")) {
            let Some(v) = v.as_str().filter(|s| !s.is_empty()) else {
                issue(issues, &loc, "currentness_inputs must contain strings")?;
                continue;
            };
            if v.starts_with("external:") {
                continue;
            }
            if v.contains(['*', '?']) {
                if !glob_exists(s, v)? {
                    issue(issues, &loc, format!("currentness glob has no match: {v}"))?;
                }
            } else if !s.exists(v)? {
                issue(issues, &loc, format!("currentness input is missing: {v}"))?;
            }
        }
    }
    let id_set = ids.iter().cloned().collect::<BTreeSet<_>>();
    if ids.len() != id_set.len() {
        issue(issues, family::MAP_PATH, "family ids must be unique")?;
    }
    if id_set != set(EXPECTED_FAMILY_IDS) {
        issue(
            issues,
            family::MAP_PATH,
            format!(
                "family ids must cover {}",
                sorted(&set(EXPECTED_FAMILY_IDS))
            ),
        )?;
    }
    if !value(p, "authority_declarations").is_array()
        || array(value(p, "authority_declarations")).is_empty()
    {
        issue(
            issues,
            family::MAP_PATH,
            "authority_declarations must be a non-empty list",
        )?;
    }
    for (field, required) in [
        (
            "public_authored_surfaces",
            EXPECTED_PUBLIC_AUTHORED_SURFACES,
        ),
        (
            "public_forbidden_markers",
            EXPECTED_PUBLIC_FORBIDDEN_MARKERS,
        ),
    ] {
        let values = value(p, field);
        if !string_list(values, true) {
            issue(
                issues,
                family::MAP_PATH,
                format!("{field} must be a non-empty list of strings"),
            )?;
        }
        if values.is_array() && array(values).iter().all(Value::is_string) {
            let missing = set(required)
                .difference(&names(values))
                .cloned()
                .collect::<Vec<_>>();
            if !missing.is_empty() {
                issue(
                    issues,
                    family::MAP_PATH,
                    format!(
                        "{field} is missing required entries: {}",
                        missing.join(", ")
                    ),
                )?;
            }
        }
    }
    let guards = value(p, "guard_families");
    let guard_ids = array(guards)
        .iter()
        .filter_map(|g| text(g, "id").map(str::to_owned))
        .collect::<BTreeSet<_>>();
    if !guards.is_array()
        || guard_ids
            != set(&[
                "broken_markdown_route",
                "stale_executable_route",
                "authority_conflict",
                "generated_projection_mismatch",
                "missing_family_coverage",
                "public_safety",
                "context_budget",
            ])
    {
        issue(
            issues,
            family::MAP_PATH,
            "guard_families must cover all seven cross-corpus guards",
        )?;
    }
    if text(value(p, "projection_decision"), "llms_txt") != Some("not_added") {
        issue(
            issues,
            family::MAP_PATH,
            "projection_decision must preserve the evidence-backed llms.txt no-add decision",
        )?;
    }
    for key in [
        "schema_ref",
        "builder",
        "validator",
        "generated_currentness",
    ] {
        if let Some(route) = text(p, key) {
            if !route.starts_with("external:") && !s.exists(route)? {
                issue(
                    issues,
                    family::MAP_PATH,
                    format!("source map route is missing: {route}"),
                )?;
            }
        }
    }
    Ok(())
}
fn read_text(s: &mut RouteSources, path: &str, used: &mut usize) -> io::Result<String> {
    let raw = s.bounded_bytes(path, 64 * 1024 * 1024, used, 512 * 1024 * 1024)?;
    let text = String::from_utf8(raw).map_err(io::Error::other)?;
    Ok(text.replace("\r\n", "\n").replace('\r', "\n"))
}
pub fn route_visible_markdown(rules: &MarkdownRules, text: &str) -> io::Result<String> {
    let rendered = rules.rendered(text)?;
    let indented = Regex::new(r"(?m)^(?: {4}|\t).*$")
        .unwrap()
        .replace_all(&rendered, "")
        .into_owned();
    let b = indented.as_bytes();
    let mut out = String::new();
    let mut copied = 0;
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'`' || i > 0 && b[i - 1] == b'`' {
            i += 1;
            continue;
        }
        let count = b[i..].iter().take_while(|c| **c == b'`').count();
        let line_end = b[i..]
            .iter()
            .position(|c| *c == b'\n')
            .map(|n| i + n)
            .unwrap_or(b.len());
        let mut closing = None;
        for width in (1..=count).rev() {
            let mut j = i + width;
            while j < line_end {
                if b[j] == b'`' {
                    let run = b[j..line_end].iter().take_while(|c| **c == b'`').count();
                    if run >= width {
                        closing = Some(j + run);
                        break;
                    }
                    j += run;
                } else {
                    j += 1;
                }
            }
            if closing.is_some() {
                break;
            }
        }
        if let Some(end) = closing {
            out.push_str(&indented[copied..i]);
            copied = end;
            i = end;
        } else {
            i += count;
        }
    }
    out.push_str(&indented[copied..]);
    Ok(out)
}
fn declared_payload(
    root: &Path,
    s: &mut RouteSources,
    rules: &MarkdownRules,
    source: &str,
    reference: &str,
    tracked: &BTreeSet<String>,
) -> io::Result<bool> {
    let (target, fragment) = rules.reference_parts(reference);
    if !fragment.is_empty() || target.is_empty() {
        return Ok(false);
    }
    for candidate in [
        root.join(source).parent().unwrap().join(target),
        root.join(target),
    ] {
        let normalized = normalize_path(&candidate);
        let Ok(relative) = normalized.strip_prefix(root) else {
            continue;
        };
        let Some(relative) = relative.to_str() else {
            continue;
        };
        let parts = relative.split('/').collect::<Vec<_>>();
        if parts.len() != 13
            || parts[..3] != ["ToS", "source-witnesses", "works"]
            || parts[5] != "expressions"
            || parts[7] != "editions"
            || parts[9] != "items"
            || parts[11] != "payload"
        {
            continue;
        }
        let manifest = format!("{}/item.manifest.json", parts[..11].join("/"));
        if !tracked.contains(&manifest) {
            continue;
        }
        let Ok(m) = read_json(s, &manifest) else {
            continue;
        };
        if text(&m, "storage_posture") != Some("local_gitignored_payload") {
            continue;
        }
        let expected = format!("payload/{}", parts[12]);
        if array(value(&m, "payload_files"))
            .iter()
            .any(|v| text(v, "relative_path") == Some(&expected))
        {
            return Ok(true);
        }
    }
    Ok(false)
}
fn normalize_path(path: &Path) -> PathBuf {
    let mut p = PathBuf::new();
    for c in path.components() {
        match c {
            std::path::Component::ParentDir => {
                p.pop();
            }
            std::path::Component::CurDir => {}
            c => p.push(c.as_os_str()),
        }
    }
    p
}
pub fn validate_markdown_routes(
    root: &Path,
    s: &mut RouteSources,
    tracked: &[String],
    issues: &mut Vec<Issue>,
) -> io::Result<()> {
    let rules = MarkdownRules::new()?;
    let tracked_set = tracked.iter().cloned().collect();
    let mut used = 0;
    let mut anchors = BTreeMap::<String, BTreeSet<String>>::new();
    let mut anchor_bytes = 0usize;
    let mut ref_count = 0usize;
    let mut ref_bytes = 0usize;
    for path in tracked {
        if !path.ends_with(".md") || !s.is_file(path)? {
            continue;
        }
        let text = read_text(s, path, &mut used)?;
        let rendered = route_visible_markdown(&rules, &text)?;
        let (mut refs, definitions) = rules.references(
            &rendered,
            100000usize.saturating_sub(ref_count),
            64 * 1024 * 1024usize - ref_bytes,
        )?;
        for (label, explicit) in rules.reference_uses(&rendered)? {
            let name = if explicit.is_empty() {
                &label
            } else {
                &explicit
            };
            let key = rules.reference_label(name)?;
            if let Some(target) = definitions.get(&key) {
                refs.push(target.clone());
            } else {
                issue(
                    issues,
                    path,
                    format!("unresolved reference-style documentation route: {name}"),
                )?;
            }
        }
        ref_count = ref_count
            .checked_add(refs.len())
            .ok_or_else(|| invalid("documentation reference count overflow"))?;
        ref_bytes = refs.iter().try_fold(ref_bytes, |sum, r| {
            sum.checked_add(r.len())
                .ok_or_else(|| invalid("documentation reference bytes overflow"))
        })?;
        if ref_count > 100000 || ref_bytes > 64 * 1024 * 1024 {
            return Err(invalid("documentation reference bound exceeded"));
        }
        for reference in refs {
            if reference.starts_with("http:")
                || reference.starts_with("https:")
                || reference.starts_with("mailto:")
            {
                continue;
            }
            let Some(resolved) = rules.resolve(root, &root.join(path), &reference)? else {
                if !declared_payload(root, s, &rules, path, &reference, &tracked_set)? {
                    issue(
                        issues,
                        path,
                        format!("broken local documentation route: {reference}"),
                    )?;
                }
                continue;
            };
            let (_, fragment) = rules.reference_parts(&reference);
            if !fragment.is_empty() {
                let relative = resolved
                    .strip_prefix(root)
                    .map_err(io::Error::other)?
                    .to_str()
                    .ok_or_else(|| invalid("non-UTF-8 fragment target"))?
                    .to_string();
                if !anchors.contains_key(&relative) {
                    let text = read_text(s, &relative, &mut used)?;
                    let (ids, bytes) = rules.anchors(&text, anchor_bytes)?;
                    anchor_bytes += bytes;
                    anchors.insert(relative.clone(), ids);
                }
                if !anchors[&relative].contains(&rules.normalized_fragment(fragment)?) {
                    issue(
                        issues,
                        path,
                        format!("broken local documentation fragment: {reference}"),
                    )?;
                }
            }
        }
        s.check()?;
    }
    Ok(())
}
fn command_regex() -> Regex {
    Regex::new(
        r"((?:\.\./)*(?:[A-Za-z0-9_.-]+/)*(?:scripts|mechanics)/[A-Za-z0-9_./-]+\.(?:py|sh))",
    )
    .unwrap()
}
fn command_references<'a>(re: &Regex, line: &'a str) -> Vec<(usize, usize, &'a str)> {
    re.find_iter(line)
        .filter(|m| {
            m.start() == 0
                || !line.as_bytes()[m.start() - 1].is_ascii_alphanumeric()
                    && !b"_./-".contains(&line.as_bytes()[m.start() - 1])
        })
        .map(|m| (m.start(), m.end(), m.as_str()))
        .collect()
}
fn command_carriers(line: &str) -> Vec<(usize, &str)> {
    let b = line.as_bytes();
    let mut result = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i < b.len() {
        let mut end = i;
        let split = if b";|,".contains(&b[i]) {
            if b[i] == b'|' && b.get(i + 1) == Some(&b'|') {
                end += 1;
            }
            true
        } else if b[i] == b'&' && b.get(i + 1) == Some(&b'&') {
            end += 1;
            true
        } else if i > 0 && b".!?".contains(&b[i - 1]) {
            let c = line[i..].chars().next().unwrap();
            if route_cards::python_space(c) {
                end = i + c.len_utf8() - 1;
                while end + 1 < b.len() {
                    let c = line[end + 1..].chars().next().unwrap();
                    if !route_cards::python_space(c) {
                        break;
                    }
                    end += c.len_utf8();
                }
                true
            } else {
                false
            }
        } else {
            false
        };
        if split {
            result.push((start, &line[start..i]));
            start = end + 1;
            i = start;
        } else {
            let c = line[i..].chars().next().unwrap();
            i += c.len_utf8();
        }
    }
    result.push((start, &line[start..]));
    result
}
fn external_reference_allowed(re: &Regex, line: &str, start: usize) -> io::Result<bool> {
    for (offset, carrier) in command_carriers(line) {
        if offset <= start && start < offset + carrier.len() {
            if command_references(re, carrier).len() != 1 {
                return Ok(false);
            }
            let output_bound = carrier
                .len()
                .checked_mul(3)
                .ok_or_else(|| io::Error::other("command carrier lowercase bound overflow"))?;
            let lowered =
                python_lower_unicode16_v1(carrier, carrier.len(), output_bound, output_bound)
                    .map_err(io::Error::other)?;
            return Ok(EXTERNAL_OWNER_MARKERS.iter().any(|m| lowered.contains(m)));
        }
    }
    Ok(false)
}
pub fn validate_executable_routes(
    root: &Path,
    s: &mut RouteSources,
    tracked: &[String],
    issues: &mut Vec<Issue>,
) -> io::Result<()> {
    let rules = MarkdownRules::new()?;
    let re = command_regex();
    let mut used = 0;
    for path in tracked {
        if !MARKDOWN_SUFFIXES.iter().any(|ext| path.ends_with(ext)) || !s.is_file(path)? {
            continue;
        }
        if Path::new(path)
            .file_name()
            .is_some_and(|s| s == "CHANGELOG.md")
            || [
                "ToS/research-packets/",
                "ToS/review-ledger/",
                "kag/receipts/",
            ]
            .iter()
            .any(|prefix| path.starts_with(prefix))
        {
            continue;
        }
        let contents = read_text(s, path, &mut used)?;
        // D0062 preserves decision command citations as historical operations.
        // This does not exempt their authored Markdown links or active guides.
        if let Some(record) = path.strip_prefix("docs/decisions/TOS-D-") {
            if record.len() > 5
                && record.as_bytes()[..4].iter().all(u8::is_ascii_digit)
                && record.as_bytes()[4] == b'-'
                && record.ends_with(".md")
                && !record.contains('/')
                && contents.contains(&format!("Decision ID: TOS-D-{}", &record[..4]))
            {
                continue;
            }
        }
        for line in route_cards::splitlines(&contents) {
            for (start, _, reference) in command_references(&re, line) {
                let resolved = rules.resolve(root, &root.join(path), reference)?;
                let valid = if let Some(resolved) = &resolved {
                    s.is_file(
                        resolved
                            .strip_prefix(root)
                            .map_err(io::Error::other)?
                            .to_str()
                            .ok_or_else(|| invalid("non-UTF-8 executable target"))?,
                    )?
                } else {
                    false
                };
                if !valid {
                    if !external_reference_allowed(&re, line, start)? {
                        issue(
                            issues,
                            path,
                            format!("stale executable reference: {reference}"),
                        )?;
                    }
                    continue;
                }
            }
        }
    }
    let inventory=read_json(s,route_cards::INVENTORY).unwrap_or_else(|_|json!({"route_card_discovery":{"root_cards":["AGENTS.md"],"route_roots":[".github","ToS","docs","mechanics","scripts","tests","evals","memo","kag","stats","manifests",".agents"]}}));
    // The inventory mandates tracked AGENTS cards. Reuse the documentation
    // atlas membership already admitted above; physical untracked-card refusal
    // remains with the nested-card owner validation below.
    let roots = array(&inventory["route_card_discovery"]["route_roots"]);
    let mut cards = Vec::new();
    for path in tracked {
        let eligible = path == "AGENTS.md"
            || (Path::new(path)
                .file_name()
                .is_some_and(|n| n == "AGENTS.md")
                && roots.iter().filter_map(|r| r.as_str()).any(|root| {
                    path.strip_prefix(root)
                        .is_some_and(|tail| tail.starts_with('/'))
                }));
        if eligible {
            if !s.is_file(path)? {
                return Err(invalid(format!("tracked route card is missing: {path}")));
            }
            cards.push(path.clone());
        }
    }
    issues.extend(route_cards::validate_local_script_references(s, &cards)?);
    Ok(())
}
fn summary_tokens(summary: &Value) -> io::Result<usize> {
    let raw = serde_json::to_vec(summary).map_err(io::Error::other)?;
    let doc = parse_json(
        &raw,
        JsonMode::RequestLastWins,
        JsonLimits {
            max_bytes: 8 * 1024 * 1024,
            ..JsonLimits::default()
        },
    )
    .map_err(io::Error::other)?;
    let compact = emit_python_compact_json(
        doc.root(),
        JsonLimits {
            max_bytes: 8 * 1024 * 1024,
            ..JsonLimits::default()
        },
    )
    .map_err(io::Error::other)?;
    let mut spaced = Vec::with_capacity(compact.len());
    let mut quoted = false;
    let mut escaped = false;
    for b in compact {
        spaced.push(b);
        if quoted {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                quoted = false;
            }
        } else if b == b'"' {
            quoted = true;
        } else if b == b',' || b == b':' {
            spaced.push(b' ');
        }
    }
    if spaced.len() > 16 * 1024 * 1024 {
        return Err(invalid("context summary byte bound exceeded"));
    }
    Ok(route_cards::whitespace_tokens(
        std::str::from_utf8(&spaced).map_err(io::Error::other)?,
    ))
}
pub fn validate_context_probes(
    s: &mut RouteSources,
    p: &Value,
    issues: &mut Vec<Issue>,
) -> io::Result<BTreeMap<String, BigInt>> {
    let mut measured = BTreeMap::new();
    let mut used = 0;
    for probe in array(value(p, "context_probes")) {
        if !probe.is_object() {
            issue(issues, "context_probes", "probe must be an object")?;
            continue;
        }
        let id = text(probe, "id").unwrap_or("<unknown>");
        let loc = format!("context_probes#{id}");
        let surfaces = value(probe, "surfaces");
        if !string_list(surfaces, true) {
            issue(issues, &loc, "surfaces must be a non-empty list of strings")?;
            continue;
        }
        let measure = text(probe, "measure");
        if measure.is_none_or(|m| !CONTEXT_MEASURES.contains(&m)) {
            issue(
                issues,
                &loc,
                format!(
                    "unsupported context measure: {}",
                    repr(value(probe, "measure"))
                ),
            )?;
            continue;
        }
        let result = (|| -> io::Result<Option<BigInt>> {
            match measure.unwrap() {
                "agents_route_max_inherited" => {
                    let mut values = Vec::new();
                    let mut missing = Vec::new();
                    for surface in array(surfaces) {
                        let surface = surface.as_str().unwrap();
                        let payload = read_json(s, surface)?;
                        let mut found = false;
                        for route in array(value(&payload, "task_routes")) {
                            if let Some(n) = integer(value(route, "inherited_context_tokens")) {
                                values.push(n);
                                found = true;
                            }
                        }
                        if !found {
                            missing.push(surface);
                        }
                    }
                    if !missing.is_empty() {
                        issue(
                            issues,
                            &loc,
                            format!(
                                "configured route surface produced no inherited measurements: {}",
                                missing.join(", ")
                            ),
                        )?;
                        return Ok(None);
                    }
                    Ok(Some(
                        values.into_iter().max().unwrap_or_else(|| BigInt::from(0)),
                    ))
                }
                "generated_summary" => {
                    if array(surfaces).len() != 1 {
                        issue(
                            issues,
                            &loc,
                            "generated_summary requires exactly one configured surface",
                        )?;
                        return Ok(None);
                    }
                    let payload = read_json(s, array(surfaces)[0].as_str().unwrap())?;
                    let summary = value(&payload, "context_summary");
                    if !summary.is_object() {
                        issue(
                            issues,
                            &loc,
                            "configured generated_summary surface lacks context_summary",
                        )?;
                        return Ok(None);
                    }
                    Ok(Some(BigInt::from(summary_tokens(summary)?)))
                }
                m => {
                    let mut values = Vec::new();
                    for surface in array(surfaces) {
                        values.push(route_cards::whitespace_tokens(&read_text(
                            s,
                            surface.as_str().unwrap(),
                            &mut used,
                        )?));
                    }
                    Ok(Some(BigInt::from(if m == "sum" {
                        values.iter().try_fold(0usize, |sum, n| {
                            sum.checked_add(*n)
                                .ok_or_else(|| invalid("context token count overflow"))
                        })?
                    } else {
                        values.into_iter().max().unwrap_or(0)
                    })))
                }
            }
        })();
        match result {
            Ok(Some(n)) => {
                measured.insert(id.to_owned(), n.clone());
                let maximum = value(probe, "max_tokens");
                if !positive(maximum) {
                    issue(issues, &loc, "max_tokens must be positive")?;
                } else if n > integer(maximum).unwrap_or_else(|| BigInt::from(1)) {
                    issue(
                        issues,
                        &loc,
                        format!("context budget exceeded: {n}>{maximum}"),
                    )?;
                }
            }
            Ok(None) => {}
            Err(e) => issue(issues, &loc, format!("cannot measure probe: {e}"))?,
        }
    }
    Ok(measured)
}
pub fn validate_public_safety(
    s: &mut RouteSources,
    p: &Value,
    issues: &mut Vec<Issue>,
) -> io::Result<()> {
    let mut used = 0;
    for relative in array(value(p, "public_authored_surfaces")) {
        let Some(relative) = relative.as_str() else {
            return Err(invalid("public surface must be a string"));
        };
        if !s.is_file(relative)? {
            issue(issues, relative, "public authored surface is missing")?;
            continue;
        }
        let contents = read_text(s, relative, &mut used)?;
        for marker in array(value(p, "public_forbidden_markers")) {
            let Some(marker) = marker.as_str() else {
                return Err(invalid("public forbidden marker must be a string"));
            };
            if contents.contains(marker) {
                issue(
                    issues,
                    relative,
                    format!("contains forbidden public-safety marker {}", quote(marker)),
                )?;
            }
        }
    }
    Ok(())
}
fn family_coverage(current: &Value, p: &Value, issues: &mut Vec<Issue>) -> io::Result<()> {
    let coverage = value(current, "coverage");
    if value(coverage, "unhandled_family_count") != &json!(0) {
        let paths = array(value(coverage, "unhandled_paths"))
            .iter()
            .take(5)
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        issue(
            issues,
            family::MAP_PATH,
            format!(
                "unhandled family count is {}: {}",
                value(coverage, "unhandled_family_count"),
                sorted(&paths)
            ),
        )?;
    }
    let expected = array(value(p, "families"))
        .iter()
        .filter_map(|f| text(f, "id").map(str::to_owned))
        .collect::<BTreeSet<_>>();
    let actual = array(value(current, "family_summaries"))
        .iter()
        .filter_map(|f| text(f, "family_id").map(str::to_owned))
        .collect::<BTreeSet<_>>();
    if expected != actual {
        issue(
            issues,
            family::MAP_PATH,
            format!(
                "generated family summaries differ from authored families: {}",
                sorted(&expected.symmetric_difference(&actual).cloned().collect())
            ),
        )?;
    }
    Ok(())
}
fn validate_existing_owner_contracts(
    root: &Path,
    s: &mut RouteSources,
    interpreter: Option<&Path>,
    export: Option<&crate::kag_corpus_export::VerifiedExport>,
    cancel: &AtomicI32,
    issues: &mut Vec<Issue>,
) -> io::Result<()> {
    for (prefix, results) in [
        (
            "AGENTS-route validator",
            route_cards::run_validation_with_card_discovery_limit(
                root,
                s,
                cancel,
                route_cards::MAX_SELECTED_PATH_DISCOVERY_ENTRIES,
            )?,
        ),
        (
            "mechanics topology validator",
            mechanics_topology::validate_with_deadline(root, s.deadline(), cancel)?,
        ),
        (
            "decision validator",
            crate::decision_records::run_validation(root, s, cancel)?,
        ),
        (
            "public-entry validator",
            crate::tiny_entry::validate(root, s, cancel)?,
        ),
    ] {
        for (location, message) in results {
            issue(issues, &location, format!("{prefix}: {message}"))?;
        }
    }
    let agent = crate::agent_surface_validation::validate_manifest_with_sources(
        root,
        s,
        false,
        interpreter,
        cancel,
    )?;
    if !agent.is_empty() {
        let mut lines = vec!["Agent surface validation failed.".to_string()];
        lines.extend(agent.into_iter().map(|(l, m)| format!("- {l}: {m}")));
        let detail = lines
            .iter()
            .skip(lines.len().saturating_sub(4))
            .cloned()
            .collect::<Vec<_>>()
            .join(" ");
        issue(
            issues,
            "rust/crates/tos-ops-mechanics-plan/src/agent_surface_validation.rs",
            format!("agent-surface validator: {detail}"),
        )?;
    }
    // D0062: corpus artifacts are verified at an explicitly selected export.
    // An unselected software docs check cannot require an ambient corpus map.
    let root_results = if export.is_some() {
        crate::root_entry_map::validate_verified_export(root, s, export, cancel)?
    } else {
        Vec::new()
    };
    if !root_results.is_empty() {
        // The legacy owner exits on its first defect and emits that message alone.
        let detail_lines = root_results[0].1.trim().lines().collect::<Vec<_>>();
        let detail = detail_lines
            .iter()
            .skip(detail_lines.len().saturating_sub(4))
            .copied()
            .collect::<Vec<_>>()
            .join(" ");
        issue(
            issues,
            "rust/crates/tos-ops-mechanics-plan/src/root_entry_map.rs",
            format!("owner validator: {detail}"),
        )?;
    }
    // Immutable KAG export admission remains at its explicit --export owner route.
    // The former no-argument invocation could only produce argparse exit 2.
    s.check()?;
    Ok(())
}
pub fn run_validation(
    root: &Path,
    interpreter: Option<&Path>,
    cancel: &AtomicI32,
) -> io::Result<Vec<Issue>> {
    run_validation_with_export(root, interpreter, None, cancel)
}
/// Verify an explicitly selected immutable export before documentation walks.
pub fn run_validation_with_export(
    root: &Path,
    interpreter: Option<&Path>,
    export: Option<&Path>,
    cancel: &AtomicI32,
) -> io::Result<Vec<Issue>> {
    let mut s = family::new_sources(root)?;
    let verified = export
        .map(|path| crate::kag_corpus_export::verify_with_sources(path, &s, cancel))
        .transpose()?;
    run_validation_with_verified_export(root, &mut s, true, interpreter, verified.as_ref(), cancel)
}
pub fn run_validation_with_sources(
    root: &Path,
    s: &mut RouteSources,
    reuse_existing: bool,
    interpreter: Option<&Path>,
    cancel: &AtomicI32,
) -> io::Result<Vec<Issue>> {
    run_validation_with_verified_export(root, s, reuse_existing, interpreter, None, cancel)
}
fn run_validation_with_verified_export(
    root: &Path,
    s: &mut RouteSources,
    reuse_existing: bool,
    interpreter: Option<&Path>,
    export: Option<&crate::kag_corpus_export::VerifiedExport>,
    cancel: &AtomicI32,
) -> io::Result<Vec<Issue>> {
    let mut issues = Vec::new();
    let p = match read_json(s, family::MAP_PATH) {
        Ok(p) if p.is_object() => p,
        _ => {
            return Ok(vec![(
                family::MAP_PATH.into(),
                "source family map is missing or invalid JSON".into(),
            )]);
        }
    };
    validate_source_map(root, s, &p, &mut issues)?;
    validate_surface_rules(value(&p, "surface_rules"), &mut issues)?;
    validate_context_probe_configuration(value(&p, "context_probes"), &mut issues)?;
    validate_family_match_rules(value(&p, "families"), &mut issues)?;
    validate_authority_declarations(
        value(&p, "authority_declarations"),
        &mut issues,
        "authority_declarations",
    )?;
    validate_family_authority_claims(
        value(&p, "families"),
        value(&p, "authority_declarations"),
        &mut issues,
    )?;
    let tracked = family::tracked_paths(root, s, cancel)?;
    validate_markdown_routes(root, s, &tracked, &mut issues)?;
    validate_executable_routes(root, s, &tracked, &mut issues)?;
    let current = family::build_currentness_with_tracked(root, s, &p, &tracked, cancel);
    match &current {
        Ok(current) => family_coverage(current, &p, &mut issues)?,
        Err(e) => issue(
            &mut issues,
            family::MAP_PATH,
            format!("cannot build family coverage: {e}"),
        )?,
    }
    let projection = (|| -> io::Result<bool> {
        let current = current.as_ref().map_err(|e| invalid(e.to_string()))?;
        let expected = family::render_currentness(current)?;
        let actual = s.text(family::CURRENTNESS_PATH)?.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "missing generated projection: {}",
                    root.join(family::CURRENTNESS_PATH).display()
                ),
            )
        })?;
        Ok(actual == expected)
    })();
    match projection {
        Ok(false) => issue(
            &mut issues,
            family::CURRENTNESS_PATH,
            "generated documentation family projection is stale",
        )?,
        Ok(true) => {}
        Err(e) => issue(
            &mut issues,
            family::CURRENTNESS_PATH,
            format!("cannot load generated projection: {e}"),
        )?,
    }
    validate_public_safety(s, &p, &mut issues)?;
    validate_context_probes(s, &p, &mut issues)?;
    if reuse_existing {
        validate_existing_owner_contracts(root, s, interpreter, export, cancel, &mut issues)?;
    }
    s.check()?;
    Ok(issues)
}
