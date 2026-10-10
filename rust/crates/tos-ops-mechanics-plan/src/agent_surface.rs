//! Maintained agent-surface currentness. Authored manifests keep authority.
use crate::route_cards::RouteSources;
use crate::{executor, route_cards};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io,
    path::Path,
    sync::atomic::{AtomicI32, Ordering},
    time::Duration,
};
use tos_foundation::{Digest256, Digest256Hasher};

pub const MANIFEST_PATH: &str = ".agents/agent-surface.manifest.json";
pub const CURRENTNESS_PATH: &str = ".agents/agent-surface.current.json";
pub const SKILLS_ROOT: &str = ".agents/skills";
pub const COMPANION_FAMILIES: &[&str] = &["references", "examples", "checks", "scripts", "assets"];
pub const FRONTMATTER_KEYS: &[&str] = &[
    "name",
    "description",
    "license",
    "compatibility",
    "aoa_scope",
    "aoa_status",
    "aoa_invocation_mode",
    "aoa_source_skill_path",
    "aoa_source_repo",
    "aoa_portable_profile",
];
const TOP_KEYS: &[&str] = &["name", "description", "license", "compatibility"];
pub const OPENAI_KEYS: &[&str] = &["implicit_activation_policy", "allow_implicit_invocation"];
fn invalid(s: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, s.into())
}
fn check(s: &RouteSources, cancel: &AtomicI32) -> io::Result<()> {
    if cancel.load(Ordering::Relaxed) != 0 {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "agent surface cancelled",
        ));
    }
    s.check()
}
fn strip(s: &str) -> &str {
    s.trim_matches(route_cards::python_space)
}
pub fn quoted_scalar_is_open(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(quote @ ('\'' | '"')) = chars.next() else {
        return false;
    };
    let mut chars = chars.peekable();
    let mut escaped = false;
    while let Some(c) = chars.next() {
        if quote == '\'' {
            if c == '\'' {
                if chars.peek() == Some(&'\'') {
                    chars.next();
                } else {
                    return false;
                }
            }
        } else if escaped {
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '"' {
            return false;
        }
    }
    true
}
pub fn frontmatter_scalars(
    block: &str,
    path: &str,
) -> io::Result<BTreeMap<String, Option<String>>> {
    let mut values: BTreeMap<_, _> = FRONTMATTER_KEYS
        .iter()
        .map(|k| (k.to_string(), None))
        .collect();
    let (mut active, mut seen) = (false, false);
    for (index, line) in route_cards::splitlines(block).into_iter().enumerate() {
        let n = index + 1;
        let text = strip(line);
        if text.is_empty() || text.starts_with('#') {
            continue;
        }
        let indent = line.len() - line.trim_start_matches(' ').len();
        let (key, value) = text.split_once(':').unwrap_or((text, ""));
        if indent == 0 {
            if key == "metadata" {
                if seen {
                    return Err(invalid(format!("{path}:{n}: duplicate metadata mapping")));
                }
                if !text.contains(':') || !strip(value).is_empty() {
                    return Err(invalid(format!("{path}:{n}: metadata must be a mapping")));
                }
                seen = true;
                active = true;
                continue;
            }
            active = false;
            if TOP_KEYS.contains(&key) {
                if values[key].is_some() {
                    return Err(invalid(format!(
                        "{path}:{n}: duplicate frontmatter field {key}"
                    )));
                }
                if strip(value).is_empty() {
                    return Err(invalid(format!(
                        "{path}:{n}: frontmatter field {key} needs a scalar"
                    )));
                }
                let scalar = strip(value);
                let block_scalar = scalar.starts_with(['>', '|'])
                    && scalar[1..]
                        .chars()
                        .all(|c| c.is_ascii_digit() || c == '+' || c == '-');
                if key == "description" && (block_scalar || quoted_scalar_is_open(scalar)) {
                    return Err(invalid(format!(
                        "{path}:{n}: multiline description scalars are not supported"
                    )));
                }
                values.insert(key.into(), Some(scalar.into()));
                continue;
            }
            if FRONTMATTER_KEYS.contains(&key) {
                return Err(invalid(format!(
                    "{path}:{n}: {key} must be an immediate child of metadata"
                )));
            }
        } else if FRONTMATTER_KEYS.contains(&key) && !TOP_KEYS.contains(&key) {
            if !active || indent != 2 {
                return Err(invalid(format!(
                    "{path}:{n}: {key} must be an immediate child of metadata"
                )));
            }
            if values[key].is_some() {
                return Err(invalid(format!(
                    "{path}:{n}: duplicate metadata field {key}"
                )));
            }
            if strip(value).is_empty() {
                return Err(invalid(format!(
                    "{path}:{n}: metadata field {key} needs a scalar"
                )));
            }
            values.insert(key.into(), Some(strip(value).into()));
        }
    }
    Ok(values)
}
pub fn policy_scalars(text: &str, path: &str) -> io::Result<BTreeMap<String, Option<String>>> {
    let mut values: BTreeMap<_, _> = OPENAI_KEYS.iter().map(|k| (k.to_string(), None)).collect();
    let (mut active, mut seen) = (false, false);
    for (index, line) in route_cards::splitlines(text).into_iter().enumerate() {
        let n = index + 1;
        let text = strip(line);
        if text.is_empty() || text.starts_with('#') {
            continue;
        }
        let indent = line.len() - line.trim_start_matches(' ').len();
        if text == "policy:" {
            if indent != 0 {
                return Err(invalid(format!(
                    "{path}:{n}: policy mapping must be top-level"
                )));
            }
            if seen {
                return Err(invalid(format!(
                    "{path}:{n}: duplicate top-level policy mapping"
                )));
            }
            seen = true;
            active = true;
            continue;
        }
        let (key, value) = text.split_once(':').unwrap_or((text, ""));
        if indent == 0 {
            active = false;
            if OPENAI_KEYS.contains(&key) {
                return Err(invalid(format!(
                    "{path}:{n}: {key} must be an immediate child of policy"
                )));
            }
            continue;
        }
        if !OPENAI_KEYS.contains(&key) {
            continue;
        }
        if !active || indent != 2 {
            return Err(invalid(format!(
                "{path}:{n}: {key} must be an immediate child of policy"
            )));
        }
        if values[key].is_some() {
            return Err(invalid(format!("{path}:{n}: duplicate policy field {key}")));
        }
        if strip(value).is_empty() {
            return Err(invalid(format!(
                "{path}:{n}: policy field {key} needs a scalar"
            )));
        }
        values.insert(key.into(), Some(strip(value).into()));
    }
    Ok(values)
}
pub fn boolean_scalar(value: Option<&str>, key: &str, path: &str) -> io::Result<Option<bool>> {
    match value {
        None => Ok(None),
        Some("true") => Ok(Some(true)),
        Some("false") => Ok(Some(false)),
        _ => Err(invalid(format!(
            "{path}: {key} must be the YAML boolean literal true or false"
        ))),
    }
}

pub fn profile_skill_ids(manifest: &Value) -> Vec<String> {
    manifest["profile_binding"]["sources"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|s| s["skills"].as_array().into_iter().flatten())
        .filter_map(|s| s.as_str().or_else(|| s.get("name").and_then(Value::as_str)))
        .map(str::to_owned)
        .collect()
}
pub fn legacy_projection_ids(manifest: &Value) -> Vec<String> {
    manifest["legacy_projection_migration"]["entries"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| s.get("legacy_name").and_then(Value::as_str))
        .map(str::to_owned)
        .collect()
}
fn tracked_files(
    root: &Path,
    sources: &mut RouteSources,
    package: &str,
    cancel: &AtomicI32,
) -> io::Result<Vec<String>> {
    check(sources, cancel)?;
    // The existing disposable Git executor bounds output and owns child cleanup.
    let answer = executor::capture_ci_git(
        root,
        vec![
            "git".into(),
            "ls-files".into(),
            "--cached".into(),
            "--".into(),
            package.into(),
        ],
        executor::Limits {
            command_wall: sources.remaining_time()?.min(Duration::from_secs(10)),
            lane_wall: sources.remaining_time()?.min(Duration::from_secs(10)),
            cleanup_grace: Duration::from_secs(1),
            output_bytes: 4 * 1024 * 1024,
        },
        cancel,
    );
    check(sources, cancel)?;
    let mut tracked = Vec::new();
    match answer {
        Ok((0, out, _)) => {
            let text =
                String::from_utf8(out).map_err(|_| invalid("Git package paths are not UTF-8"))?;
            for path in route_cards::splitlines(&text) {
                if !path.is_empty() && sources.is_file(path)? {
                    tracked.push(path.to_owned());
                }
            }
        }
        Ok(_) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    Ok(tracked)
}
pub fn package_files(
    root: &Path,
    sources: &mut RouteSources,
    package: &str,
    cancel: &AtomicI32,
) -> io::Result<Vec<String>> {
    let tracked = tracked_files(root, sources, package, cancel)?;
    if !tracked.is_empty() {
        return Ok(tracked);
    }
    fallback_package_files(sources, package)
}
fn fallback_package_files(sources: &mut RouteSources, package: &str) -> io::Result<Vec<String>> {
    let ignored = [
        ".deps",
        ".git",
        ".mypy_cache",
        ".pytest_cache",
        ".ruff_cache",
        "__pycache__",
        "htmlcov",
    ];
    let mut fallback = Vec::new();
    for path in sources.paths(package)? {
        let relative = Path::new(&path)
            .strip_prefix(package)
            .map_err(|_| invalid("package path escaped root"))?;
        if relative.iter().any(|p| ignored.iter().any(|i| p == *i))
            || relative
                .extension()
                .is_some_and(|x| x == "pyc" || x == "pyo")
        {
            continue;
        }
        if sources.is_file(&path)? {
            fallback.push(path);
        }
    }
    Ok(fallback)
}
pub fn parse_skill(
    root: &Path,
    sources: &mut RouteSources,
    relative: &str,
    cancel: &AtomicI32,
) -> io::Result<Value> {
    parse_skill_with_files(root, sources, relative, cancel, None, &mut 0)
}
fn parse_skill_with_files(
    root: &Path,
    sources: &mut RouteSources,
    relative: &str,
    cancel: &AtomicI32,
    selected: Option<&[String]>,
    payload_bytes: &mut usize,
) -> io::Result<Value> {
    check(sources, cancel)?;
    let text = sources
        .text(relative)?
        .ok_or_else(|| invalid(format!("missing {relative}")))?;
    let rest = text
        .strip_prefix("---\n")
        .ok_or_else(|| invalid(format!("{relative}: missing YAML frontmatter")))?;
    let end = rest
        .find("\n---\n")
        .ok_or_else(|| invalid(format!("{relative}: missing YAML frontmatter")))?;
    let values = frontmatter_scalars(&rest[..end], relative)?;
    if values["name"].as_deref().is_none_or(str::is_empty)
        || values["description"].as_deref().is_none_or(str::is_empty)
    {
        return Err(invalid(format!(
            "{relative}: name and description are required"
        )));
    }
    let package = Path::new(relative)
        .parent()
        .and_then(Path::to_str)
        .ok_or_else(|| invalid("skill needs a package parent"))?;
    let openai = format!("{package}/agents/openai.yaml");
    let policy = sources
        .text(&openai)?
        .ok_or_else(|| invalid(format!("missing {openai}")))?;
    let activation = policy_scalars(&policy, &openai)?;
    let implicit = boolean_scalar(
        activation["allow_implicit_invocation"].as_deref(),
        "allow_implicit_invocation",
        &openai,
    )?;
    let files = if let Some(all) = selected {
        let found: Vec<_> = all
            .iter()
            .filter(|p| Path::new(p).starts_with(package))
            .cloned()
            .collect();
        if found.is_empty() {
            fallback_package_files(sources, package)?
        } else {
            found
        }
    } else {
        package_files(root, sources, package, cancel)?
    };
    let mut counts: BTreeMap<&str, usize> = COMPANION_FAMILIES.iter().map(|k| (*k, 0)).collect();
    let mut hasher = Digest256Hasher::new();
    let mut bytes = 0usize;
    for path in &files {
        check(sources, cancel)?;
        let name = Path::new(path)
            .strip_prefix(package)
            .map_err(|_| invalid("package path escaped root"))?
            .to_str()
            .ok_or_else(|| invalid("non-UTF-8 package path"))?;
        if let Some(count) = counts.get_mut(name.split('/').next().unwrap_or("")) {
            *count += 1;
        }
        let raw = sources.bounded_bytes(path, 8 * 1024 * 1024, payload_bytes, 64 * 1024 * 1024)?;
        bytes = bytes
            .checked_add(raw.len())
            .ok_or_else(|| invalid("package byte overflow"))?;
        hasher.update(name.as_bytes());
        hasher.update(b"\0");
        hasher.update(&raw);
        hasher.update(b"\0");
    }
    let mut front = serde_json::Map::new();
    for key in FRONTMATTER_KEYS.iter().filter(|k| **k != "description") {
        front.insert((*key).into(), json!(values[*key]));
    }
    front.insert(
        "description_words".into(),
        json!(route_cards::whitespace_tokens(
            values["description"].as_deref().unwrap_or("")
        )),
    );
    Ok(
        json!({"id":values["name"],"entrypoint":relative,"package_bytes":bytes,"package_file_count":files.len(),"package_sha256":hasher.finalize().to_hex(),"companion_counts":counts,"frontmatter":front,"triggered_body_words":route_cards::whitespace_tokens(&rest[end+5..]),"activation":{"implicit_activation_policy":activation["implicit_activation_policy"],"allow_implicit_invocation":implicit},"on_demand_families_present":COMPANION_FAMILIES.iter().filter(|f| counts[**f] != 0).collect::<Vec<_>>()}),
    )
}
fn file_record(s: &mut RouteSources, path: &str, payload_bytes: &mut usize) -> io::Result<Value> {
    let bytes = s.bounded_bytes(path, 8 * 1024 * 1024, payload_bytes, 64 * 1024 * 1024)?;
    Ok(json!({"path":path,"bytes":bytes.len(),"sha256":Digest256::of_bytes(&bytes).to_hex()}))
}
pub fn build_currentness(root: &Path, cancel: &AtomicI32) -> io::Result<Value> {
    build_currentness_with_sources(root, &mut RouteSources::new(root)?, cancel)
}
pub fn build_currentness_with_sources(
    root: &Path,
    s: &mut RouteSources,
    cancel: &AtomicI32,
) -> io::Result<Value> {
    check(s, cancel)?;
    let raw = s.bytes(MANIFEST_PATH)?;
    let manifest: Value = serde_json::from_slice(&raw).map_err(io::Error::other)?;
    let mut packages = Vec::new();
    let mut payload_bytes = 0;
    if s.is_dir(SKILLS_ROOT)? {
        let tracked = tracked_files(root, s, SKILLS_ROOT, cancel)?;
        for path in s.paths(SKILLS_ROOT)? {
            let rel = Path::new(&path)
                .strip_prefix(SKILLS_ROOT)
                .map_err(|_| invalid("skill path escaped root"))?;
            if rel.components().count() == 2
                && rel.file_name().is_some_and(|x| x == "SKILL.md")
                && s.is_file(&path)?
            {
                packages.push(parse_skill_with_files(
                    root,
                    s,
                    &path,
                    cancel,
                    Some(&tracked),
                    &mut payload_bytes,
                )?);
            }
        }
    }
    let mut inventory = serde_json::Map::new();
    inventory.insert("skill_entrypoints".into(), json!(packages.len()));
    inventory.insert("agents_metadata".into(), json!(packages.len()));
    for family in COMPANION_FAMILIES {
        inventory.insert(
            (*family).into(),
            json!(
                packages
                    .iter()
                    .map(|p| p["companion_counts"][*family].as_u64().unwrap_or(0))
                    .sum::<u64>()
            ),
        );
    }
    let ids = profile_skill_ids(&manifest);
    inventory.insert("profile_skill_bindings".into(), json!(ids.len()));
    let mut binding = serde_json::Map::new();
    for key in [
        "schema_version",
        "profile",
        "runtime",
        "scope",
        "install_root",
        "install_mode",
        "source_manifest",
        "resolver",
    ] {
        binding.insert(key.into(), manifest["profile_binding"][key].clone());
    }
    binding.insert(
        "source_count".into(),
        json!(
            manifest["profile_binding"]["sources"]
                .as_array()
                .map_or(0, Vec::len)
        ),
    );
    binding.insert("skill_ids".into(), json!(ids));
    let legacy = legacy_projection_ids(&manifest);
    let mut depths = serde_json::Map::new();
    for probe in manifest["task_probes"].as_array().into_iter().flatten() {
        if let Some(id) = probe.get("id").and_then(Value::as_str) {
            depths.insert(
                id.into(),
                probe
                    .get("mandatory_reading_depth")
                    .ok_or_else(|| invalid("task probe missing mandatory_reading_depth"))?
                    .clone(),
            );
        }
    }
    let mut ports = Vec::new();
    if let Some(map) = manifest["owner_ports"].as_object() {
        let mut keys: Vec<_> = map.keys().collect();
        keys.sort();
        for id in keys {
            let port = &map[id];
            if !port.is_object() {
                continue;
            }
            let mut inputs = Vec::new();
            for value in port["currentness_inputs"].as_array().into_iter().flatten() {
                let path = value
                    .as_str()
                    .ok_or_else(|| invalid("owner port input path must be a string"))?;
                inputs.push(if s.is_file(path)? {
                    file_record(s, path, &mut payload_bytes)?
                } else {
                    json!({"path":path,"missing":true})
                });
            }
            ports.push(json!({"id":id,"manifest":port["manifest"],"inputs":inputs}));
        }
    }
    check(s, cancel)?;
    Ok(
        json!({"schema_version":"tos_agent_tool_owner_port_current_v1","owner_repo":manifest["owner_repo"],"source_manifest":MANIFEST_PATH,"manifest_sha256":Digest256::of_bytes(&raw).to_hex(),"builder":"rust/crates/tos-ops-mechanics-plan/src/agent_surface.rs","package_inventory":inventory,"packages":packages,"profile_binding":binding,"legacy_projection":{"entry_count":legacy.len(),"legacy_ids":legacy},"task_probe_depths":depths,"owner_ports":ports}),
    )
}
pub fn rendered_currentness(root: &Path, cancel: &AtomicI32) -> io::Result<String> {
    route_cards::render_currentness(&build_currentness(root, cancel)?)
}

/// Same maintained builder entrypoint, with bounded source custody.
pub fn run(root: &Path, check_only: bool, cancel: &AtomicI32) -> io::Result<i32> {
    let rendered = rendered_currentness(root, cancel)?;
    let output = root.join(CURRENTNESS_PATH);
    if check_only {
        match route_cards::read_output(&output)? {
            None => {
                eprintln!("[error] missing {CURRENTNESS_PATH}");
                return Ok(1);
            }
            Some(actual) if actual != rendered => {
                eprintln!("[error] stale {CURRENTNESS_PATH}; run the builder");
                return Ok(1);
            }
            _ => println!("[ok] currentness parity for {CURRENTNESS_PATH}"),
        }
    } else {
        route_cards::write_output(root, &output, &rendered)?;
        println!("[ok] wrote {CURRENTNESS_PATH}");
    }
    Ok(0)
}
