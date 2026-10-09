//! Agent surface authored contracts and bounded generated currentness checks.
use crate::agent_surface::{self, CURRENTNESS_PATH, MANIFEST_PATH, SKILLS_ROOT};
use crate::agent_surface_budget;
use crate::route_cards::{self, RouteSources};
use num_bigint::BigInt;
use regex::Regex;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::Path;
use std::sync::atomic::AtomicI32;
pub type Issue = (String, String);
const EXPECTED_PROFILE_SKILLS: &[&str] = &[
    "abyss-self-diagnostic-spine",
    "aoa-agents-skills",
    "aoa-checkpoint-closeout-bridge",
    "aoa-decision",
    "aoa-eval",
    "aoa-evals-skills",
    "aoa-kag",
    "aoa-knowledge-stewardship",
    "aoa-memo",
    "aoa-memo-writeback",
    "aoa-session-harvest",
    "aoa-session-memory-evidence-route",
    "aoa-session-memory-global-route",
    "aoa-session-progression-lift",
    "aoa-session-recovery",
    "aoa-stats",
    "aoa-summon",
    "os-abyss-artifact-trust-loop",
];
const EXPECTED_LEGACY_PROJECTIONS: &[&str] = &[
    "aoa-adr-write",
    "aoa-approval-gate-check",
    "aoa-automation-opportunity-scan",
    "aoa-bounded-context-map",
    "aoa-change-protocol",
    "aoa-checkpoint-closeout-bridge",
    "aoa-commit-growth-seam",
    "aoa-contract-test",
    "aoa-core-logic-boundary",
    "aoa-dry-run-first",
    "aoa-invariant-coverage-audit",
    "aoa-local-stack-bringup",
    "aoa-port-adapter-refactor",
    "aoa-property-invariants",
    "aoa-quest-harvest",
    "aoa-safe-infra-change",
    "aoa-sanitized-share",
    "aoa-session-donor-harvest",
    "aoa-session-progression-lift",
    "aoa-session-route-forks",
    "aoa-session-self-diagnose",
    "aoa-session-self-repair",
    "aoa-source-of-truth-check",
    "aoa-summon",
    "aoa-tdd-slice",
];
const EXPECTED_PORTS: &[&str] = &["eval_port", "kag_provider", "memo_port", "stats_port"];
const EXPECTED_PROBES: &[&str] = &[
    "approval_or_dry_run",
    "bounded_context_mapping",
    "eval_intake",
    "kag_currentness",
    "memo_candidate_routing",
    "owner_local_stats",
    "repo_local_change",
    "session_diagnosis_repair",
    "source_authority",
];
const PUBLIC_FORBIDDEN: &[&str] = &[
    "/home/",
    "/srv/",
    "/tmp/",
    "BEGIN PRIVATE KEY",
    "OPENAI_API_KEY",
    "AWS_SECRET_ACCESS_KEY",
    "file://",
];
fn issue(issues: &mut Vec<Issue>, location: &str, message: impl Into<String>) {
    issues.push((location.into(), message.into()));
}
fn value<'a>(v: &'a Value, key: &str) -> &'a Value {
    v.get(key).unwrap_or(&Value::Null)
}
fn text<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    value(v, key).as_str()
}
fn list(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn truth(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(x) => *x,
        Value::String(x) => !x.is_empty(),
        Value::Array(x) => !x.is_empty(),
        Value::Object(x) => !x.is_empty(),
        Value::Number(x) => x.to_string() != "0",
    }
}
fn names(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|s| s.to_string()).collect()
}
fn repr(v: &Value) -> String {
    match v {
        Value::Null => "None".into(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::String(s) => py_quote(s),
        _ => v.to_string(),
    }
}
fn py_quote(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::new();
    out.push(quote);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c)
            }
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}
fn py_sorted(values: &BTreeSet<String>) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|s| py_quote(s))
            .collect::<Vec<_>>()
            .join(", ")
    )
}
fn positive(v: &Value) -> bool {
    integer_value(v).is_some_and(|v| v > BigInt::from(0))
}
fn safe_relative(s: &str) -> bool {
    !s.is_empty() && !s.starts_with('/') && !s.split('/').any(|x| x == "..")
}
fn required_scalars(
    issues: &mut Vec<Issue>,
    v: &Value,
    location: &str,
    expected: &[(&str, Value)],
) {
    for (key, want) in expected {
        if value(v, key) != want {
            issue(issues, location, format!("{key} must be {}", repr(want)));
        }
    }
}
pub fn activation_policy_issues(
    invocation_mode: Option<&str>,
    implicit_policy: Option<&str>,
    allow_implicit: Option<bool>,
    skill_id: Option<&str>,
) -> Vec<String> {
    let expected = match invocation_mode {
        Some("explicit-only") => Some(("manual", false)),
        Some("explicit-preferred") if skill_id == Some("aoa-checkpoint-closeout-bridge") => {
            Some(("suggest", false))
        }
        Some("explicit-preferred") => Some(("invoke", true)),
        _ => None,
    };
    let Some((policy, allow)) = expected else {
        return vec![format!(
            "unknown aoa_invocation_mode {}",
            repr(&invocation_mode.map(|s| json!(s)).unwrap_or(Value::Null))
        )];
    };
    let mut issues = Vec::new();
    if implicit_policy != Some(policy) {
        issues.push(format!(
            "implicit_activation_policy={}, expected {}",
            repr(&implicit_policy.map(|s| json!(s)).unwrap_or(Value::Null)),
            py_quote(policy)
        ));
    }
    if allow_implicit != Some(allow) {
        issues.push(format!(
            "allow_implicit_invocation={}, expected {}",
            repr(&allow_implicit.map(|s| json!(s)).unwrap_or(Value::Null)),
            repr(&json!(allow))
        ));
    }
    issues
}
pub fn public_safety_issues(sources: &mut RouteSources) -> io::Result<Vec<Issue>> {
    let mut issues = Vec::new();
    for relative in [".agents/README.md", MANIFEST_PATH] {
        let Some(contents) = sources.text(relative)? else {
            issue(&mut issues, relative, "public authored surface is missing");
            continue;
        };
        for forbidden in PUBLIC_FORBIDDEN {
            if contents.contains(forbidden) {
                issue(
                    &mut issues,
                    relative,
                    format!(
                        "contains forbidden public-safety marker {}",
                        py_quote(forbidden)
                    ),
                );
            }
        }
    }
    Ok(issues)
}
pub fn profile_binding_issues(manifest: &Value) -> (Vec<Issue>, BTreeSet<String>) {
    let mut issues = Vec::new();
    let location = format!("{MANIFEST_PATH}#profile_binding");
    let binding = value(manifest, "profile_binding");
    if !binding.is_object() {
        return (
            vec![(location, "profile_binding must be an object".into())],
            BTreeSet::new(),
        );
    }
    required_scalars(
        &mut issues,
        binding,
        &location,
        &[
            ("schema_version", json!("tos_os_skill_profile_binding_v1")),
            ("profile", json!("os-user-default")),
            ("runtime", json!("codex")),
            ("scope", json!("user")),
            ("install_root", json!("$HOME/.codex/skills")),
            ("install_mode", json!("managed-copy")),
            (
                "source_manifest",
                json!("aoa-skills:config/os_skill_profiles.json"),
            ),
            (
                "source_ref",
                json!("aoa-skills@616ce49eed8a605782fb2f295060ae916e04c7a6"),
            ),
            (
                "resolver",
                json!("aoa-skills:scripts/bundles/install_os_skill_profile.py"),
            ),
        ],
    );
    let sources = value(binding, "sources");
    if !sources.is_array() || list(sources).is_empty() {
        issue(&mut issues, &location, "sources must be a non-empty list");
        return (issues, BTreeSet::new());
    }
    let mut actual = Vec::new();
    let mut selected = Vec::new();
    for (index, source) in list(sources).iter().enumerate() {
        let loc = format!("{location}.sources[{index}]");
        if !source.is_object() {
            issue(&mut issues, &loc, "source must be an object");
            continue;
        }
        let kind = text(source, "kind");
        let repo = text(source, "repo");
        let root = text(source, "root");
        let operation = value(source, "owner_operation");
        if !matches!(kind, Some("shared-home" | "owner-port" | "owner-link")) {
            issue(
                &mut issues,
                &loc,
                "kind must be shared-home, owner-port, or owner-link",
            );
        }
        if kind == Some("owner-link") && operation != &json!("install-user-skill") {
            issue(
                &mut issues,
                &loc,
                "owner-link owner_operation must be install-user-skill",
            );
        }
        if matches!(kind, Some("shared-home" | "owner-port")) && !operation.is_null() {
            issue(
                &mut issues,
                &loc,
                "shared-home and owner-port sources must not declare owner_operation",
            );
        }
        if repo.is_none_or(str::is_empty) {
            issue(&mut issues, &loc, "repo must be a non-empty string");
        }
        if root.is_none_or(|s| !safe_relative(s)) {
            issue(&mut issues, &loc, "root must be a safe relative owner root");
        }
        let skills = value(source, "skills");
        if !skills.is_array() || list(skills).is_empty() {
            issue(&mut issues, &loc, "skills must be a non-empty list");
            continue;
        }
        let mut source_names = BTreeSet::new();
        for (i, skill) in list(skills).iter().enumerate() {
            let skill_loc = format!("{loc}.skills[{i}]");
            let name;
            if kind == Some("owner-link") {
                if !skill.is_object() {
                    issue(
                        &mut issues,
                        &skill_loc,
                        "owner-link skill must be an object",
                    );
                    continue;
                }
                name = text(skill, "name");
                if name.is_none_or(str::is_empty) {
                    issue(
                        &mut issues,
                        &skill_loc,
                        "owner-link skill needs a non-empty name",
                    );
                    continue;
                }
                if text(skill, "path")
                    .is_none_or(|s| !s.starts_with("skills/") || s.split('/').any(|p| p == ".."))
                {
                    issue(
                        &mut issues,
                        &skill_loc,
                        "owner-link path must be a safe skills/ path",
                    );
                }
                if text(skill, "version").is_none_or(str::is_empty) {
                    issue(&mut issues, &skill_loc, "owner-link skill needs a version");
                }
            } else {
                name = skill.as_str();
                if name.is_none_or(str::is_empty) {
                    issue(
                        &mut issues,
                        &skill_loc,
                        "profile skill name must be a non-empty string",
                    );
                    continue;
                }
            }
            let name = name.unwrap().to_string();
            source_names.insert(name.clone());
            selected.push(name);
        }
        if let (Some(kind), Some(repo), Some(root)) = (kind, repo, root) {
            actual.push((
                kind.to_string(),
                repo.to_string(),
                root.to_string(),
                operation.clone(),
                source_names,
            ));
        }
    }
    let expected = vec![
        (
            "shared-home",
            "aoa-skills",
            "self",
            None,
            &[
                "aoa-decision",
                "aoa-eval",
                "aoa-knowledge-stewardship",
                "aoa-checkpoint-closeout-bridge",
                "aoa-memo-writeback",
                "aoa-session-harvest",
                "aoa-session-recovery",
            ][..],
        ),
        (
            "owner-port",
            "aoa-evals",
            "aoa-evals",
            None,
            &["aoa-evals-skills"][..],
        ),
        (
            "owner-port",
            "aoa-memo",
            "aoa-memo",
            None,
            &["aoa-memo"][..],
        ),
        (
            "owner-port",
            "aoa-stats",
            "aoa-stats",
            None,
            &["aoa-stats"][..],
        ),
        ("owner-port", "aoa-kag", "aoa-kag", None, &["aoa-kag"][..]),
        (
            "owner-port",
            "aoa-agents",
            "aoa-agents",
            None,
            &[
                "aoa-agents-skills",
                "aoa-session-progression-lift",
                "aoa-summon",
            ][..],
        ),
        (
            "owner-port",
            "abyss-machine",
            "abyss-machine",
            None,
            &["os-abyss-artifact-trust-loop"][..],
        ),
        (
            "owner-port",
            "abyss-stack",
            "abyss-stack",
            None,
            &["abyss-self-diagnostic-spine"][..],
        ),
        (
            "owner-link",
            ".aoa",
            ".aoa",
            Some("install-user-skill"),
            &[
                "aoa-session-memory-global-route",
                "aoa-session-memory-evidence-route",
            ][..],
        ),
    ]
    .into_iter()
    .map(|(kind, repo, root, operation, skills)| {
        (
            kind.into(),
            repo.into(),
            root.into(),
            operation.map(|s| json!(s)).unwrap_or(Value::Null),
            names(skills),
        )
    })
    .collect::<Vec<_>>();
    if actual != expected {
        issue(
            &mut issues,
            &location,
            "sources do not match the accepted os-user-default selection",
        );
    }
    let selected_set: BTreeSet<String> = selected.iter().cloned().collect();
    if selected.len() != selected_set.len() {
        issue(
            &mut issues,
            &location,
            "profile selection contains duplicate skills",
        );
    }
    if selected_set != names(EXPECTED_PROFILE_SKILLS) {
        issue(
            &mut issues,
            &location,
            format!(
                "profile skills must be {}",
                py_sorted(&names(EXPECTED_PROFILE_SKILLS))
            ),
        );
    }
    if text(binding, "duplicate_boundary").is_none_or(|s| !s.contains(".agents/skills")) {
        issue(
            &mut issues,
            &location,
            "duplicate_boundary must prohibit a repository-local projection",
        );
    }
    claim_limits(&mut issues, binding, &location);
    (issues, selected_set)
}
fn claim_limits(issues: &mut Vec<Issue>, value_: &Value, loc: &str) {
    let limits = value(value_, "claim_limits");
    if !limits.is_array()
        || list(limits).is_empty()
        || !list(limits)
            .iter()
            .all(|x| x.as_str().is_some_and(|s| !s.is_empty()))
    {
        issue(
            issues,
            loc,
            "claim_limits must be a non-empty list of strings",
        );
    }
}
pub fn legacy_projection_crosswalk_issues(manifest: &Value) -> Vec<Issue> {
    let mut issues = Vec::new();
    let location = format!("{MANIFEST_PATH}#legacy_projection_migration");
    let migration = value(manifest, "legacy_projection_migration");
    if !migration.is_object() {
        return vec![(
            location,
            "legacy_projection_migration must be an object".into(),
        )];
    }
    required_scalars(
        &mut issues,
        migration,
        &location,
        &[
            (
                "schema_version",
                json!("tos_legacy_skill_projection_crosswalk_v1"),
            ),
            (
                "source_catalog",
                json!("aoa-skills:capabilities/legacy-skill-migration.yaml"),
            ),
            (
                "source_ref",
                json!("aoa-skills@6eaeca11820adbbbe54f79a75c0ca5a54e0c4a15"),
            ),
            ("legacy_projection_root", json!(".agents/skills/")),
            ("post_migration_local_projection", json!("absent")),
            ("entry_count", json!(25)),
        ],
    );
    let entries = value(migration, "entries");
    if !entries.is_array() || list(entries).is_empty() {
        issue(&mut issues, &location, "entries must be a non-empty list");
        return issues;
    }
    let mut selected = Vec::new();
    for (index, entry) in list(entries).iter().enumerate() {
        let loc = format!("{location}.entries[{index}]");
        if !entry.is_object() {
            issue(&mut issues, &loc, "entry must be an object");
            continue;
        }
        for key in [
            "legacy_name",
            "legacy_path",
            "action",
            "target_id",
            "target_kind",
            "target_owner",
            "compatibility",
            "evidence_state",
            "reason",
        ] {
            if text(entry, key).is_none_or(str::is_empty) {
                issue(
                    &mut issues,
                    &loc,
                    format!("{key} must be a non-empty string"),
                );
            }
        }
        if let Some(name) = text(entry, "legacy_name") {
            selected.push(name.to_string());
        }
        if !matches!(
            text(entry, "action"),
            Some("retain-advertised" | "merge-mode" | "route-owner-object")
        ) {
            issue(
                &mut issues,
                &loc,
                "action is not a recognized migration action",
            );
        }
        if !matches!(
            text(entry, "target_kind"),
            Some("skill" | "mode" | "workflow" | "guard")
        ) {
            issue(
                &mut issues,
                &loc,
                "target_kind is not a recognized destination kind",
            );
        }
        if text(entry, "legacy_path").is_some_and(|s| !s.starts_with("skills/")) {
            issue(
                &mut issues,
                &loc,
                "legacy_path must remain an aoa-skills source path",
            );
        }
    }
    let set: BTreeSet<String> = selected.iter().cloned().collect();
    if set.len() != selected.len() {
        issue(
            &mut issues,
            &location,
            "crosswalk contains duplicate legacy names",
        );
    }
    if set != names(EXPECTED_LEGACY_PROJECTIONS) {
        issue(
            &mut issues,
            &location,
            format!(
                "crosswalk names must be {}",
                py_sorted(&names(EXPECTED_LEGACY_PROJECTIONS))
            ),
        );
    }
    claim_limits(&mut issues, migration, &location);
    issues
}
fn local_reference_issues(sources: &mut RouteSources, package: &str) -> io::Result<Vec<Issue>> {
    let mut issues = Vec::new();
    let pattern=Regex::new(r"\b(?:references|examples|checks|scripts|assets)/[A-Za-z0-9_.-]+(?:/[A-Za-z0-9_.-]+)*\.[A-Za-z0-9_-]+\b").unwrap();
    let assets = Regex::new(r"\./(assets/[A-Za-z0-9_.-]+)").unwrap();
    for file in ["SKILL.md", "agents/openai.yaml"] {
        let path = format!("{package}/{file}");
        let content = sources.text(&path)?.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("missing route file: {path}"),
            )
        })?;
        for m in pattern.find_iter(&content) {
            let before = &content[..m.start()];
            if before
                .chars()
                .rev()
                .take(20)
                .collect::<String>()
                .chars()
                .rev()
                .collect::<String>()
                .contains("repo:")
            {
                continue;
            }
            let route = m.as_str();
            if !sources.is_file(&format!("{package}/{route}"))? {
                issue(
                    &mut issues,
                    &path,
                    format!("broken local companion route {route}"),
                );
            }
        }
        for m in assets.captures_iter(&content) {
            let route = m.get(1).unwrap().as_str();
            if !sources.is_file(&format!("{package}/{route}"))? {
                issue(&mut issues, &path, format!("broken asset route {route}"));
            }
        }
    }
    Ok(issues)
}
pub fn validate_manifest(
    root: &Path,
    fetch_missing_budget_base: bool,
    interpreter: Option<&Path>,
    cancel: &AtomicI32,
) -> io::Result<Vec<Issue>> {
    let mut sources = RouteSources::new(root)?;
    validate_manifest_with_sources(
        root,
        &mut sources,
        fetch_missing_budget_base,
        interpreter,
        cancel,
    )
}
pub fn validate_manifest_with_sources(
    root: &Path,
    sources: &mut RouteSources,
    fetch_missing_budget_base: bool,
    interpreter: Option<&Path>,
    cancel: &AtomicI32,
) -> io::Result<Vec<Issue>> {
    let mut issues = Vec::new();
    let Some(raw) = sources.text(MANIFEST_PATH)? else {
        return Ok(vec![(
            MANIFEST_PATH.into(),
            "missing agent-surface manifest".into(),
        )]);
    };
    let manifest: Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => return Ok(vec![(MANIFEST_PATH.into(), format!("invalid JSON: {e}"))]),
    };
    if !manifest.is_object() {
        return Ok(vec![(
            MANIFEST_PATH.into(),
            "manifest root must be an object".into(),
        )]);
    }
    bounded_manifest(&manifest)?;
    required_scalars(
        &mut issues,
        &manifest,
        MANIFEST_PATH,
        &[
            (
                "schema_version",
                json!("tos_agent_tool_owner_port_documentation_v1"),
            ),
            ("owner_repo", json!("Tree-of-Sophia")),
            ("owner_surface", json!(".agents/AGENTS.md")),
            ("human_entrypoint", json!(".agents/README.md")),
            ("generated_currentness", json!(CURRENTNESS_PATH)),
            (
                "builder",
                json!("rust/crates/tos-ops-mechanics-plan/src/agent_surface.rs"),
            ),
            ("validator", json!("rust/crates/tos-ops-mechanics-plan/src/agent_surface_validation.rs")),
        ],
    );
    for key in ["owner_surface", "human_entrypoint", "builder", "validator"] {
        if let Some(route) = text(&manifest, key) {
            if !sources.is_file(route)? {
                issue(&mut issues, route, "declared route is missing");
            }
        }
    }
    let budget = value(&manifest, "context_budget");
    if !budget.is_object() {
        issue(
            &mut issues,
            MANIFEST_PATH,
            "context_budget must be an object",
        );
    }
    for key in [
        "discovery_description_max_words",
        "triggered_body_max_words",
        "mandatory_task_probe_depth_max",
    ] {
        if !positive(value(budget, key)) {
            issue(
                &mut issues,
                MANIFEST_PATH,
                format!("context_budget.{key} must be positive"),
            );
        }
    }
    let (profile_issues, profile_skill_set) = profile_binding_issues(&manifest);
    issues.extend(profile_issues);
    issues.extend(legacy_projection_crosswalk_issues(&manifest));
    let packages = value(&manifest, "skills");
    if !packages.is_array() {
        issue(&mut issues, MANIFEST_PATH, "skills must be a list");
    }
    let mut by_id = BTreeMap::new();
    for package in list(packages) {
        sources.check()?;
        if !package.is_object() {
            issue(
                &mut issues,
                MANIFEST_PATH,
                "each skill record must be an object",
            );
            continue;
        }
        let Some(skill_id) = text(package, "id").filter(|s| !s.is_empty()) else {
            issue(
                &mut issues,
                MANIFEST_PATH,
                "skill record needs a non-empty id",
            );
            continue;
        };
        if by_id.insert(skill_id.to_string(), package).is_some() {
            issue(
                &mut issues,
                MANIFEST_PATH,
                format!("duplicate skill record {skill_id}"),
            );
        }
        if text(package, "family").is_none_or(str::is_empty) {
            issue(
                &mut issues,
                MANIFEST_PATH,
                format!("{skill_id} needs a family"),
            );
        }
        let Some(entrypoint) = text(package, "entrypoint") else {
            issue(
                &mut issues,
                MANIFEST_PATH,
                format!("{skill_id} needs an entrypoint"),
            );
            continue;
        };
        if !sources.is_file(entrypoint)? {
            issue(
                &mut issues,
                entrypoint,
                format!("{skill_id} entrypoint is missing"),
            );
            continue;
        }
        let current = match agent_surface::parse_skill(root, sources, entrypoint, cancel) {
            Ok(v) => v,
            Err(e) => {
                issue(&mut issues, entrypoint, e.to_string());
                continue;
            }
        };
        if value(&current, "id") != &json!(skill_id) {
            issue(
                &mut issues,
                entrypoint,
                format!(
                    "frontmatter name {} does not match {}",
                    repr(value(&current, "id")),
                    py_quote(skill_id)
                ),
            );
        }
        let fm = value(&current, "frontmatter");
        if value(fm, "aoa_source_repo") != &json!("8Dionysus/aoa-skills") {
            issue(
                &mut issues,
                entrypoint,
                "aoa_source_repo must be 8Dionysus/aoa-skills",
            );
        }
        if text(fm, "aoa_source_skill_path").is_none_or(|s| !s.starts_with("skills/")) {
            issue(
                &mut issues,
                entrypoint,
                "aoa_source_skill_path must identify an aoa-skills source path",
            );
        }
        if integer(value(fm, "description_words"))
            > integer(value(budget, "discovery_description_max_words"))
        {
            issue(
                &mut issues,
                entrypoint,
                "discovery description exceeds context budget",
            );
        }
        if integer(value(&current, "triggered_body_words"))
            > integer(value(budget, "triggered_body_max_words"))
        {
            issue(
                &mut issues,
                entrypoint,
                "triggered SKILL.md body exceeds context budget",
            );
        }
        let package_path = entrypoint.rsplit_once('/').map(|(p, _)| p).unwrap_or(".");
        issues.extend(
            local_reference_issues(sources, package_path)?
                .into_iter()
                .map(|(loc, message)| (loc, format!("{skill_id}: {message}"))),
        );
        let activation = value(&current, "activation");
        issues.extend(
            activation_policy_issues(
                text(fm, "aoa_invocation_mode"),
                text(activation, "implicit_activation_policy"),
                value(activation, "allow_implicit_invocation").as_bool(),
                Some(skill_id),
            )
            .into_iter()
            .map(|message| (entrypoint.to_string(), format!("{skill_id}: {message}"))),
        );
    }
    let paths = if sources.is_dir(SKILLS_ROOT)? {
        sources.paths(SKILLS_ROOT)?
    } else {
        Vec::new()
    };
    let discovered: BTreeSet<String> = paths
        .iter()
        .filter_map(|path| {
            let suffix = path.strip_prefix(&format!("{SKILLS_ROOT}/"))?;
            let (name, file) = suffix.split_once('/')?;
            if file == "SKILL.md" {
                Some(name.to_string())
            } else {
                None
            }
        })
        .collect();
    let declared: BTreeSet<String> = by_id.keys().cloned().collect();
    if !discovered.is_empty() {
        issue(
            &mut issues,
            SKILLS_ROOT,
            format!(
                "discovered repository-local skills differ from expected empty set: {}",
                py_sorted(&discovered)
            ),
        );
    }
    if !declared.is_empty() {
        issue(
            &mut issues,
            MANIFEST_PATH,
            format!(
                "declared repository-local skills differ from expected empty set: {}",
                py_sorted(&declared)
            ),
        );
    }
    if discovered != declared {
        issue(
            &mut issues,
            MANIFEST_PATH,
            "manifest skill ids do not match local package discovery",
        );
    }
    if !paths.is_empty() {
        issue(
            &mut issues,
            SKILLS_ROOT,
            "stale repository-local projection remains; selected bundles belong to the OS user profile",
        );
    }
    let families = value(&manifest, "skill_families");
    if !families.is_object() {
        issue(
            &mut issues,
            MANIFEST_PATH,
            "skill_families must be an object",
        );
    }
    let mut family_members = BTreeSet::new();
    if let Some(families) = families.as_object() {
        for (family_id, family) in families {
            if !family.is_object() {
                issue(
                    &mut issues,
                    MANIFEST_PATH,
                    format!("{family_id} family must be an object"),
                );
                continue;
            }
            let members = value(family, "profile_skills");
            if !members.is_array() || list(members).is_empty() {
                issue(
                    &mut issues,
                    MANIFEST_PATH,
                    format!("{family_id}.profile_skills must be non-empty"),
                );
                continue;
            }
            for required in [
                "consumer",
                "load_moment",
                "canonical_owner",
                "freshness",
                "next_organ",
                "negative_controls",
            ] {
                if !truth(value(family, required)) {
                    issue(
                        &mut issues,
                        MANIFEST_PATH,
                        format!("{family_id}.{required} is required"),
                    );
                }
            }
            for member in list(members) {
                if let Some(member) = member.as_str() {
                    family_members.insert(member.to_string());
                    if !profile_skill_set.contains(member) {
                        issue(
                            &mut issues,
                            MANIFEST_PATH,
                            format!("{family_id} names unknown profile skill {member}"),
                        );
                    }
                } else {
                    issue(
                        &mut issues,
                        MANIFEST_PATH,
                        format!("{family_id} names unknown profile skill {}", repr(member)),
                    );
                }
            }
        }
    }
    if family_members != profile_skill_set {
        issue(
            &mut issues,
            MANIFEST_PATH,
            "profile skill families do not cover exactly the selected profile skills",
        );
    }
    let ports = value(&manifest, "owner_ports");
    if !ports.is_object() {
        issue(&mut issues, MANIFEST_PATH, "owner_ports must be an object");
    }
    let port_ids: BTreeSet<String> = ports
        .as_object()
        .map(|p| p.keys().cloned().collect())
        .unwrap_or_default();
    if port_ids != names(EXPECTED_PORTS) {
        issue(
            &mut issues,
            MANIFEST_PATH,
            format!("owner ports must be {}", py_sorted(&names(EXPECTED_PORTS))),
        );
    }
    if let Some(ports) = ports.as_object() {
        for (port_id, port) in ports {
            if !port.is_object() {
                issue(
                    &mut issues,
                    MANIFEST_PATH,
                    format!("{port_id} must be an object"),
                );
                continue;
            }
            for field in [
                "local_owner",
                "manifest",
                "consumer",
                "load_moment",
                "canonical_owner",
                "freshness",
                "next_organ",
                "currentness_inputs",
                "carrier_classes",
            ] {
                if !truth(value(port, field)) {
                    issue(
                        &mut issues,
                        MANIFEST_PATH,
                        format!("{port_id}.{field} is required"),
                    );
                }
            }
            for field in ["local_owner", "manifest"] {
                if let Some(route) = text(port, field) {
                    if !sources.is_file(route)? {
                        issue(&mut issues, route, format!("{port_id} route is missing"));
                    }
                }
            }
            let inputs = value(port, "currentness_inputs");
            for relative in list(inputs) {
                if let Some(relative) = relative.as_str() {
                    if !sources.is_file(relative)? {
                        issue(
                            &mut issues,
                            relative,
                            format!("{port_id} currentness input is missing"),
                        );
                    }
                } else {
                    issue(
                        &mut issues,
                        &repr(relative),
                        format!("{port_id} currentness input is missing"),
                    );
                }
            }
            for field in ["local_owner", "manifest"] {
                if let Some(route) = text(port, field) {
                    if !list(inputs).contains(&json!(route)) {
                        issue(
                            &mut issues,
                            MANIFEST_PATH,
                            format!("{port_id} {field} must be a currentness input"),
                        );
                    }
                }
            }
            if port_id == "kag_provider" {
                issues.extend(agent_surface_budget::generated_family_issues(
                    root,
                    sources,
                    port,
                    fetch_missing_budget_base,
                    interpreter,
                    cancel,
                )?);
            }
        }
    }
    let probes = value(&manifest, "task_probes");
    if !probes.is_array() {
        issue(&mut issues, MANIFEST_PATH, "task_probes must be a list");
    }
    let mut probe_ids = BTreeSet::new();
    let max_depth = integer(value(budget, "mandatory_task_probe_depth_max"));
    for probe in list(probes) {
        if !probe.is_object() {
            issue(
                &mut issues,
                MANIFEST_PATH,
                "each task probe must be an object",
            );
            continue;
        }
        let Some(id) = text(probe, "id").filter(|s| !s.is_empty()) else {
            issue(&mut issues, MANIFEST_PATH, "task probe needs an id");
            continue;
        };
        if !probe_ids.insert(id.to_string()) {
            issue(
                &mut issues,
                MANIFEST_PATH,
                format!("duplicate task probe {id}"),
            );
        }
        if !truth(value(probe, "first_route")) || !truth(value(probe, "required_chain")) {
            issue(
                &mut issues,
                MANIFEST_PATH,
                format!("{id} needs first_route and required_chain"),
            );
        }
        if !value(probe, "negative_controls").is_array()
            || list(value(probe, "negative_controls")).is_empty()
        {
            issue(
                &mut issues,
                MANIFEST_PATH,
                format!("{id} needs negative controls"),
            );
        }
        let depth = value(probe, "mandatory_reading_depth");
        if !positive(depth) || integer(depth) > max_depth {
            issue(
                &mut issues,
                MANIFEST_PATH,
                format!("{id} exceeds mandatory reading depth budget"),
            );
        }
        for skill in list(value(probe, "skill_ids")) {
            if skill
                .as_str()
                .is_none_or(|s| !profile_skill_set.contains(s))
            {
                issue(
                    &mut issues,
                    MANIFEST_PATH,
                    format!(
                        "{id} names unknown profile skill {}",
                        skill
                            .as_str()
                            .map(str::to_string)
                            .unwrap_or_else(|| repr(skill))
                    ),
                );
            }
        }
        let port = value(probe, "port_id");
        if !port.is_null() && port.as_str().is_none_or(|s| !port_ids.contains(s)) {
            issue(
                &mut issues,
                MANIFEST_PATH,
                format!(
                    "{id} names unknown port {}",
                    port.as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| repr(port))
                ),
            );
        }
    }
    if probe_ids != names(EXPECTED_PROBES) {
        issue(
            &mut issues,
            MANIFEST_PATH,
            format!("task probes must be {}", py_sorted(&names(EXPECTED_PROBES))),
        );
    }
    let inventory = value(&manifest, "package_inventory");
    if !inventory.is_object() {
        issue(
            &mut issues,
            MANIFEST_PATH,
            "package_inventory must be an object",
        );
    }
    let current_result = (|| -> io::Result<Value> {
        let has_currentness = sources.is_file(CURRENTNESS_PATH)?;
        if !has_currentness {
            issue(
                &mut issues,
                CURRENTNESS_PATH,
                "generated currentness is missing",
            );
        }
        let current = agent_surface::build_currentness_with_sources(root, sources, cancel)?;
        let rendered = route_cards::render_currentness(&current)?;
        if has_currentness && sources.bytes(CURRENTNESS_PATH)? != rendered.as_bytes() {
            issue(
                &mut issues,
                CURRENTNESS_PATH,
                "generated currentness is stale",
            );
        }
        Ok(current)
    })();
    match current_result {
        Ok(current) => {
            if inventory.is_object() && value(&current, "package_inventory") != inventory {
                issue(
                    &mut issues,
                    MANIFEST_PATH,
                    "package_inventory counts disagree with discovered packages",
                );
            }
            let current_ports: BTreeSet<String> = list(value(&current, "owner_ports"))
                .iter()
                .filter_map(|p| text(p, "id").map(str::to_string))
                .collect();
            if current_ports != port_ids {
                issue(
                    &mut issues,
                    CURRENTNESS_PATH,
                    "owner-port currentness ids disagree with manifest",
                );
            }
        }
        Err(e) => issue(
            &mut issues,
            CURRENTNESS_PATH,
            format!("cannot build currentness: {e}"),
        ),
    }
    issues.extend(public_safety_issues(sources)?);
    sources.check()?;
    if issues.len() > 4096 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "agent surface issue count exceeded",
        ));
    }
    Ok(issues)
}
fn integer_value(v: &Value) -> Option<BigInt> {
    match v {
        Value::Number(n) => n.to_string().parse::<BigInt>().ok(),
        Value::Bool(b) => Some(BigInt::from(if *b { 1 } else { 0 })),
        _ => None,
    }
}
fn integer(v: &Value) -> BigInt {
    integer_value(v).unwrap_or_else(|| BigInt::from(0))
}

fn bounded_manifest(manifest: &Value) -> io::Result<()> {
    let mut pending = vec![(manifest, 0usize)];
    let mut nodes = 0usize;
    let mut scalar_bytes = 0usize;
    while let Some((v, depth)) = pending.pop() {
        nodes += 1;
        if nodes > 8192 || depth > 64 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "agent surface manifest node/depth bound exceeded",
            ));
        }
        match v {
            Value::Array(values) => {
                if values.len() > 1024 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "agent surface manifest collection bound exceeded",
                    ));
                }
                pending.extend(values.iter().map(|v| (v, depth + 1)));
            }
            Value::Object(values) => {
                if values.len() > 1024 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "agent surface manifest collection bound exceeded",
                    ));
                }
                for (key, v) in values {
                    scalar_bytes += key.len();
                    pending.push((v, depth + 1));
                }
            }
            Value::String(s) => {
                if s.len() > 65536 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "agent surface manifest scalar bound exceeded",
                    ));
                }
                scalar_bytes += s.len();
            }
            _ => {}
        }
        if scalar_bytes > 4 * 1024 * 1024 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "agent surface manifest aggregate scalar bound exceeded",
            ));
        }
    }
    Ok(())
}
/// Independently callable parity check; a new invocation observes new source bytes.
pub fn currentness_parity_issues(root: &Path, cancel: &AtomicI32) -> io::Result<Vec<Issue>> {
    let mut sources = RouteSources::new(root)?;
    if !sources.is_file(CURRENTNESS_PATH)? {
        return Ok(vec![(
            CURRENTNESS_PATH.into(),
            "generated currentness is missing".into(),
        )]);
    }
    let current = agent_surface::build_currentness_with_sources(root, &mut sources, cancel)?;
    let rendered = route_cards::render_currentness(&current)?;
    if sources.bytes(CURRENTNESS_PATH)? != rendered.as_bytes() {
        return Ok(vec![(
            CURRENTNESS_PATH.into(),
            "generated currentness is stale".into(),
        )]);
    }
    Ok(Vec::new())
}
