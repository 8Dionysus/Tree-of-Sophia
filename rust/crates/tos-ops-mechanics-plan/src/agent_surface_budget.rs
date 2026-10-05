//! Native admission checks for the concrete generated KAG budget carrier.
//! These checks preserve source/producer boundaries and do not grant KAG acceptance.
use crate::{
    executor::{self, Limits},
    route_cards::{RouteSources, sha256_bytes},
};
use num_bigint::BigInt;
use serde_json::{Value, json};
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    io,
    path::{Component, Path},
    sync::atomic::AtomicI32,
    time::{Duration, Instant},
};
use tos_foundation::{CanonicalProfile, JsonLimits, JsonMode, canonical_bytes_v1, parse_json};
pub type Issue = (String, String);
const MAX_BUDGET_SOURCE_BYTES: usize = 1024 * 1024 * 1024;
const MAX_BUDGET_GIT_BYTES: usize = 64 * 1024 * 1024;
const MAX_BUDGET_GIT_COMMAND_BYTES: usize = 16 * 1024 * 1024;
const V1: &str = "aoa-repo-local-kag-budget-receipt-v1";
const V2: &str = "aoa-repo-local-kag-budget-receipt-v2";
const SEG: &str = "aoa-repo-local-kag-segmented-family-v1";
const TIER: &str = "aoa-repo-local-kag-distribution-manifest-v1";
const MANIFEST: &str = "kag/indexes/index_family.manifest.json";
const PM: &str = "config/repo-local-kag-budget-producer.json";
const PS: &str = "schemas/repo-local-kag-budget-producer-manifest.schema.json";
const ACTION: &str = ".github/actions/repo-local-kag-index/action.yml";
const CANDIDATE: &str = "aoa-kag:budget-receipt-candidate-identity-v2";
const ALGORITHM: &str = "sha256:canonical-json-file-inventory-v2";
const RUNTIME_PREFIX: &str = "aoa-kag:budget-producer-runtime-contract:";
const COMMON: &[&str] = &[
    "schema_version",
    "repo",
    "scope",
    "base_ref",
    "head_family_digest",
    "changed_generated_bytes",
    "changed_generated_files",
    "default_limit_bytes",
    "allowed_bytes",
    "tracked_bytes",
    "tracked_bytes_max",
    "allowed_tracked_bytes",
    "reason",
    "approved_by",
    "decision_ref",
];
const CANDIDATE_FIELDS: &[&str] = &[
    "contract_version",
    "algorithm",
    "seal",
    "file_count",
    "excluded_path",
    "base_ref",
    "family_digest",
    "source_snapshot",
    "source_epoch",
];
const PRODUCER_FIELDS: &[&str] = &[
    "contract_version",
    "owner",
    "revision_binding",
    "source_digest",
    "procedure_manifest",
    "files",
    "action",
    "execution_inputs",
    "identity_digest",
];
const PROCEDURE_FIELDS: &[&str] = &[
    "manifest_path",
    "manifest_digest",
    "schema_path",
    "closure_mode",
    "dynamic_import_policy",
    "python_entrypoints",
    "python_import_closure",
    "schema_inputs",
    "action_path",
    "action_inputs",
    "environment",
    "dependencies",
];
const INPUT_FIELDS: &[&str] = &["state", "kind", "value_digest", "bytes"];
fn invalid(s: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, s.into())
}
const DIAGNOSTIC_LIMIT: &str = "budget diagnostic limit exceeded";
fn push(out: &mut Vec<Issue>, label: &str, message: impl Into<String>) {
    if out.last().is_some_and(|(_, m)| m == DIAGNOSTIC_LIMIT) {
        return;
    }
    let message = message.into();
    let bytes = out.iter().map(|(l, m)| l.len() + m.len()).sum::<usize>();
    if out.len() >= 4095
        || bytes
            .checked_add(label.len())
            .and_then(|n| n.checked_add(message.len()))
            .is_none_or(|n| n > 16 * 1024 * 1024)
    {
        out.push((label.to_owned(), DIAGNOSTIC_LIMIT.to_owned()));
        return;
    }
    out.push((label.to_owned(), message));
}
/// The concrete carrier input profile uses the existing bounded JSON parser;
/// decoded duplicate names retain Python's request-style last-value behavior.
fn budget_json_preflight(raw: &[u8]) -> io::Result<()> {
    if let Err(e) = parse_json(
        raw,
        JsonMode::RequestLastWins,
        JsonLimits {
            max_bytes: 8 * 1024 * 1024,
            max_depth: 64,
            max_visits: 65536,
            max_integer_digits: 4300,
        },
    ) {
        if e.code == tos_foundation::FoundationErrorCode::BudgetExceeded {
            return Err(invalid(format!("budget JSON input profile exceeded: {e}")));
        }
        // The established carrier diagnostic below owns malformed JSON.
    }
    Ok(())
}
fn array(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn string(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
fn integer(v: &Value) -> Option<BigInt> {
    if !v.is_number() {
        return None;
    }
    v.to_string().parse().ok()
}
fn nonnegative(v: &Value) -> bool {
    integer(v).is_some_and(|n| n >= BigInt::from(0))
}
fn hex(s: &str, lo: usize, hi: usize) -> bool {
    (lo..=hi).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn digest_ok(v: &Value, prefixed: bool) -> bool {
    v.as_str().is_some_and(|s| {
        if prefixed {
            s.strip_prefix("sha256:").is_some_and(|s| hex(s, 64, 64))
        } else {
            hex(s, 64, 64)
        }
    })
}
fn digest(out: &mut Vec<Issue>, label: &str, field: &str, v: &Value, prefixed: bool) {
    if !digest_ok(v, prefixed) {
        push(
            out,
            label,
            format!(
                "budget receipt field {field} must be {}",
                if prefixed {
                    "sha256:<64 lowercase hex>"
                } else {
                    "64 lowercase hex"
                }
            ),
        );
    }
}
fn shape(out: &mut Vec<Issue>, label: &str, v: &Value, fields: &[&str], field: &str) {
    let Some(obj) = v.as_object() else {
        push(
            out,
            label,
            format!("budget receipt field {field} must be an object"),
        );
        return;
    };
    let expected: BTreeSet<_> = fields.iter().copied().collect();
    for name in &expected {
        if !obj.contains_key(*name) {
            push(
                out,
                label,
                format!("budget receipt field {field} missing {name}"),
            );
        }
    }
    for name in obj.keys().collect::<BTreeSet<_>>() {
        if !expected.contains(name.as_str()) {
            push(
                out,
                label,
                format!("budget receipt field {field} has unexpected {name}"),
            );
        }
    }
}
fn relative(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 4096
        && !s.chars().any(|c| c < ' ' || c == '\u{7f}')
        && Path::new(s)
            .components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
}
fn canon(v: &Value) -> io::Result<String> {
    let bytes = serde_json::to_vec(v).map_err(io::Error::other)?;
    let doc = parse_json(
        &bytes,
        JsonMode::RequestLastWins,
        JsonLimits {
            max_bytes: 16 * 1024 * 1024,
            ..JsonLimits::default()
        },
    )
    .map_err(io::Error::other)?;
    let bytes = canonical_bytes_v1(
        doc.root(),
        CanonicalProfile::SourceRecordDigestV1,
        JsonLimits {
            max_bytes: 16 * 1024 * 1024,
            ..JsonLimits::default()
        },
    )
    .map_err(io::Error::other)?;
    Ok(sha256_bytes(&bytes))
}
struct GitState<'a> {
    deadline: Instant,
    bytes: Cell<usize>,
    source_bytes: Cell<usize>,
    cancel: &'a AtomicI32,
}
impl GitState<'_> {
    fn remaining(&self) -> io::Result<Duration> {
        self.deadline
            .checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
            .ok_or_else(|| invalid("route operation deadline exceeded"))
    }
    fn charge(&self, bytes: usize) -> io::Result<()> {
        let total = self
            .bytes
            .get()
            .checked_add(bytes)
            .filter(|n| *n <= MAX_BUDGET_GIT_BYTES)
            .ok_or_else(|| invalid("budget Git aggregate output bound exceeded"))?;
        self.bytes.set(total);
        self.remaining()?;
        Ok(())
    }
}
fn git(root: &Path, args: &[&str], state: &GitState<'_>) -> io::Result<(i32, Vec<u8>)> {
    let mut argv = vec!["git".into()];
    argv.extend(args.iter().map(|s| s.to_string()));
    let remaining = state.remaining()?;
    let cap = MAX_BUDGET_GIT_BYTES
        .checked_sub(state.bytes.get())
        .filter(|n| *n > 0)
        .ok_or_else(|| invalid("budget Git aggregate output bound exceeded"))?
        .min(MAX_BUDGET_GIT_COMMAND_BYTES);
    let (code, out, err) = executor::capture_ci_git(
        root,
        argv,
        Limits {
            command_wall: remaining.min(Duration::from_secs(60)),
            lane_wall: remaining,
            output_bytes: cap,
            ..Limits::default()
        },
        state.cancel,
    )?;
    state.charge(
        out.len()
            .checked_add(err.len())
            .ok_or_else(|| invalid("budget Git output accounting overflow"))?,
    )?;
    Ok((code, out))
}
fn git_bytes(
    root: &Path,
    revision: &str,
    path: &str,
    cancel: &GitState<'_>,
) -> io::Result<Option<Vec<u8>>> {
    let spec = format!("{revision}:{path}");
    let (code, bytes) = git(root, &["show", &spec], cancel)?;
    Ok((code == 0).then_some(bytes))
}
fn nul_paths(root: &Path, args: &[&str], cancel: &GitState<'_>) -> io::Result<BTreeSet<String>> {
    let mut args = args.to_vec();
    args.push("-z");
    let (code, out) = git(root, &args, cancel)?;
    if code != 0 {
        return Err(invalid("source epoch requires a readable Git worktree"));
    }
    let mut result = BTreeSet::new();
    for raw in out.split(|b| *b == 0).filter(|s| !s.is_empty()) {
        let s = std::str::from_utf8(raw)
            .map_err(|_| invalid("source epoch encountered a non-UTF-8 path"))?;
        if !relative(s) {
            return Err(invalid("source epoch encountered an unsafe path"));
        }
        result.insert(s.into());
        if result.len() > 65536 {
            return Err(invalid("budget Git path count exceeded"));
        }
    }
    Ok(result)
}
fn control(s: &str) -> bool {
    ["kag/indexes", "kag/receipts/index_family_budget"]
        .iter()
        .any(|p| s == *p || s.starts_with(&format!("{p}/")))
}
fn candidate_seal(
    root: &Path,
    s: &mut RouteSources,
    d: &str,
    cancel: &GitState<'_>,
) -> io::Result<(String, usize)> {
    let (code, raw) = git(
        root,
        &[
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
        ],
        cancel,
    )?;
    if code != 0 {
        return Err(invalid(
            "candidate identity requires a readable Git worktree",
        ));
    }
    let excluded = format!("kag/receipts/index_family_budget/{d}.json");
    let mut paths = BTreeSet::new();
    for p in raw.split(|b| *b == 0).filter(|s| !s.is_empty()) {
        let p = std::str::from_utf8(p)
            .map_err(|_| invalid("candidate identity encountered a non-UTF-8 Git path"))?;
        if !relative(p) {
            return Err(invalid("candidate identity encountered an unsafe path"));
        }
        if p != excluded {
            paths.insert(p.to_owned());
        }
        if paths.len() > 65536 {
            return Err(invalid("candidate inventory count exceeded"));
        }
    }
    let mut records = Vec::new();
    for p in paths {
        if !s.is_file(&p)? {
            if !s.exists(&p)? {
                continue;
            }
            return Err(invalid(format!(
                "candidate identity cannot inventory non-file path {p}"
            )));
        }
        let mut used = cancel.source_bytes.get();
        let (b, held_mode) =
            s.bounded_bytes_with_mode(&p, 64 * 1024 * 1024, &mut used, MAX_BUDGET_SOURCE_BYTES)?;
        cancel.source_bytes.set(used);
        let mode = if held_mode & 0o100 != 0 {
            "0755"
        } else {
            "0644"
        };
        records.push(json!({"path":p,"state":"present","kind":"file","mode":mode,"bytes":b.len(),"content_digest":sha256_bytes(&b)}));
    }
    let count = records.len();
    Ok((
        canon(
            &json!({"contract_version":CANDIDATE,"algorithm":ALGORITHM,"excluded_path":excluded,"files":records}),
        )?,
        count,
    ))
}
fn source_epoch(root: &Path, s: &mut RouteSources, cancel: &GitState<'_>) -> io::Result<String> {
    let (code, _) = git(root, &["rev-parse", "HEAD"], cancel)?;
    if code != 0 {
        return Err(invalid("source epoch requires a readable Git worktree"));
    }
    let mut dirty = nul_paths(root, &["diff", "--name-only", "--cached"], cancel)?;
    dirty.extend(nul_paths(root, &["diff", "--name-only"], cancel)?);
    dirty.extend(nul_paths(
        root,
        &["ls-files", "--others", "--exclude-standard"],
        cancel,
    )?);
    dirty.retain(|p| !control(p));
    if !dirty.is_empty() {
        return Err(invalid(format!(
            "source epoch is not clean; source drift is present at: {}",
            dirty.into_iter().collect::<Vec<_>>().join(", ")
        )));
    }
    let (code, out) = git(root, &["ls-files", "-s", "--cached", "-z"], cancel)?;
    if code != 0 {
        return Err(invalid("source epoch cannot inspect the Git worktree"));
    }
    let mut records = BTreeMap::new();
    for raw in out.split(|b| *b == 0).filter(|s| !s.is_empty()) {
        let text = std::str::from_utf8(raw)
            .map_err(|_| invalid("source epoch encountered a non-UTF-8 path"))?;
        let (meta, p) = text
            .split_once('\t')
            .ok_or_else(|| invalid("source epoch found a malformed Git index entry"))?;
        let fields: Vec<_> = meta.split(' ').collect();
        if fields.len() != 3 {
            return Err(invalid("source epoch found a malformed Git index entry"));
        }
        if fields[2] != "0" || !relative(p) {
            return Err(invalid("source epoch found an unstable Git index entry"));
        }
        if control(p) {
            continue;
        }
        if !s.exists(p)? {
            return Err(invalid(format!("source epoch source path is missing: {p}")));
        }
        let record = if fields[0] == "160000" {
            json!({"path":p,"mode":fields[0],"blob_id":fields[1],"kind":"gitlink"})
        } else {
            if !s.is_file(p)? {
                return Err(invalid(format!(
                    "source epoch found a non-file source path: {p}"
                )));
            }
            if fields[0] == "120000" {
                return Err(invalid(format!("source epoch mode changed for {p}")));
            }
            let mut used = cancel.source_bytes.get();
            let b = s.bounded_bytes(p, 64 * 1024 * 1024, &mut used, MAX_BUDGET_SOURCE_BYTES)?;
            cancel.source_bytes.set(used);
            json!({"path":p,"mode":fields[0],"kind":"file","bytes":b.len(),"content_digest":sha256_bytes(&b)})
        };
        records.insert(p.to_owned(), record);
        if records.len() > 65536 {
            return Err(invalid("source epoch count exceeded"));
        }
    }
    Ok(format!(
        "sha256:{}",
        canon(
            &json!({"contract_version":"aoa-kag:budget-receipt-source-epoch-v1","files":records.into_values().collect::<Vec<_>>()})
        )?
    ))
}
pub fn budget_exceedance_relation(
    changed: &Value,
    limit: &Value,
    tracked: &Value,
    max: &Value,
) -> io::Result<Option<&'static str>> {
    let mut values = vec![
        ("changed_generated_bytes", changed),
        ("default_limit_bytes", limit),
        ("tracked_bytes", tracked),
        ("tracked_bytes_max", max),
    ];
    values.sort_by_key(|v| v.0);
    let bad: Vec<_> = values
        .iter()
        .filter(|v| integer(v.1).is_none())
        .map(|v| v.0)
        .collect();
    if !bad.is_empty() {
        return Err(invalid(format!(
            "budget relation fields must be integers: {}",
            bad.join(", ")
        )));
    }
    let bad: Vec<_> = values
        .iter()
        .filter(|v| !nonnegative(v.1))
        .map(|v| v.0)
        .collect();
    if !bad.is_empty() {
        return Err(invalid(format!(
            "budget relation fields must not be negative: {}",
            bad.join(", ")
        )));
    }
    Ok(
        match (
            integer(changed) > integer(limit),
            integer(tracked) > integer(max),
        ) {
            (false, false) => None,
            (true, false) => Some("generated_delta"),
            (false, true) => Some("tracked_size"),
            (true, true) => Some("generated_delta_and_tracked_size"),
        },
    )
}
pub fn canonical_budget_scope<'a>(relation: Option<&'a str>, base_has_v3: bool) -> Option<&'a str> {
    relation.map(|r| if base_has_v3 { r } else { "v2_to_v3_migration" })
}
fn base_has_v3(
    root: &Path,
    base: &str,
    fetch: bool,
    cancel: &GitState<'_>,
) -> io::Result<Option<bool>> {
    if !hex(base, 40, 64) {
        return Ok(None);
    }
    let spec = format!("{base}^{{commit}}");
    let (mut code, mut out) = git(root, &["rev-parse", "--verify", &spec], cancel)?;
    if code != 0 && fetch {
        let (fetched, _) = git(
            root,
            &[
                "fetch",
                "--no-tags",
                "--no-recurse-submodules",
                "origin",
                base,
            ],
            cancel,
        )?;
        if fetched == 0 {
            (code, out) = git(root, &["rev-parse", "--verify", &spec], cancel)?;
        }
    }
    if code != 0 {
        return Ok(None);
    }
    let resolved = std::str::from_utf8(&out).map_err(io::Error::other)?.trim();
    let spec = format!("{resolved}:{MANIFEST}");
    let (code, _) = git(root, &["cat-file", "-e", &spec], cancel)?;
    Ok(Some(code == 0))
}
fn safe_generated(v: &Value, field: &str) -> io::Result<String> {
    let Some(p) = v.as_str().filter(|s| !s.is_empty()) else {
        return Err(invalid(format!("{field} must be a non-empty string")));
    };
    if !relative(p) || !p.starts_with("kag/indexes/") {
        return Err(invalid(format!(
            "{field} must be a safe repository-relative KAG path"
        )));
    }
    Ok(p.into())
}
fn descriptor_paths(m: &Value, base: bool) -> io::Result<BTreeSet<String>> {
    let prefix = if base { "base" } else { "current" };
    let field = if m["schema_version"] == SEG {
        "segments"
    } else {
        "shards"
    };
    let items = m[field]
        .as_array()
        .ok_or_else(|| invalid(format!("{prefix} family manifest {field} must be an array")))?;
    let mut paths = BTreeSet::from([MANIFEST.into()]);
    for item in items {
        if !item.is_object() || !item["path"].is_string() {
            return Err(invalid(format!(
                "{prefix} family manifest contains a malformed {} descriptor",
                if field == "segments" {
                    "segment"
                } else {
                    "shard"
                }
            )));
        }
        paths.insert(safe_generated(
            &item["path"],
            &format!("{prefix} family manifest shard path"),
        )?);
    }
    Ok(paths)
}
fn changed_measurements(
    root: &Path,
    s: &mut RouteSources,
    m: &Value,
    base: &str,
    cancel: &GitState<'_>,
) -> io::Result<(usize, usize)> {
    let spec = format!("{base}^{{commit}}");
    let (code, out) = git(root, &["rev-parse", "--verify", &spec], cancel)?;
    if code != 0 {
        return Err(invalid("budget receipt base_ref cannot be resolved"));
    }
    if std::str::from_utf8(&out).map_err(io::Error::other)?.trim() != base {
        return Err(invalid(
            "budget receipt base_ref must be the resolved commit identity",
        ));
    }
    let mut paths = descriptor_paths(m, false)?;
    match git_bytes(root, base, MANIFEST, cancel)? {
        None => {
            for name in [
                "source_surface_index.json",
                "repo_artifact_index.json",
                "repo_anchor_index.json",
                "repo_entity_index.json",
                "repo_event_index.json",
                "repo_assertion_index.json",
                "repo_relation_index.json",
            ] {
                let p = format!("kag/indexes/{name}");
                if git_bytes(root, base, &p, cancel)?.is_some() {
                    paths.insert(p);
                }
            }
        }
        Some(raw) => {
            let b: Value = serde_json::from_slice(&raw)
                .map_err(|_| invalid("base family manifest is invalid"))?;
            if !b.is_object() {
                return Err(invalid("base family manifest must be an object"));
            }
            if b["schema_version"] == TIER {
                let corpus = git_bytes(root, base, "kag/indexes/corpus.manifest.json", cancel)?;
                let hot = git_bytes(root, base, "kag/indexes/hot_profile.json", cancel)?;
                let (corpus, hot) = corpus.zip(hot).ok_or_else(|| {
                    invalid("base tiered family control manifests are incomplete")
                })?;
                let c: Value = serde_json::from_slice(&corpus)
                    .map_err(|_| invalid("base tiered family control manifest is invalid"))?;
                let h: Value = serde_json::from_slice(&hot)
                    .map_err(|_| invalid("base tiered family control manifest is invalid"))?;
                let state = string(&b["placement"]["state"]);
                if !c["objects"].is_array()
                    || !h["selection"]["include_record_kinds"].is_array()
                    || !matches!(state, "shadow" | "externalized")
                {
                    return Err(invalid("base tiered family placement is malformed"));
                }
                paths.extend(
                    [
                        MANIFEST,
                        "kag/indexes/corpus.manifest.json",
                        "kag/indexes/hot_profile.json",
                        "kag/indexes/artifact_locators.json",
                    ]
                    .map(String::from),
                );
                for d in array(&c["objects"]) {
                    if !d.is_object() {
                        return Err(invalid("base tiered object descriptor is malformed"));
                    }
                    let (Some(kind), Some(range)) = (d["kind"].as_str(), d["range"].as_str())
                    else {
                        return Err(invalid("base tiered object path is malformed"));
                    };
                    if state == "shadow"
                        || array(&h["selection"]["include_record_kinds"]).contains(&d["kind"])
                    {
                        paths.insert(safe_generated(
                            &json!(format!("kag/indexes/shards/{kind}/{range}.jsonl")),
                            "base tiered object shard path",
                        )?);
                    }
                }
            } else {
                paths.extend(descriptor_paths(&b, true)?);
            }
        }
    }
    let (mut bytes, mut files) = (0usize, 0usize);
    for p in paths {
        let old = git_bytes(root, base, &p, cancel)?;
        let new = if s.is_file(&p)? {
            let mut used = cancel.source_bytes.get();
            let b = s.bounded_bytes(&p, 64 * 1024 * 1024, &mut used, MAX_BUDGET_SOURCE_BYTES)?;
            cancel.source_bytes.set(used);
            Some(b)
        } else {
            None
        };
        if old != new {
            files += 1;
            bytes = bytes
                .checked_add(
                    old.as_ref()
                        .map_or(0, Vec::len)
                        .max(new.as_ref().map_or(0, Vec::len)),
                )
                .ok_or_else(|| invalid("generated delta byte overflow"))?;
        }
    }
    Ok((bytes, files))
}
fn producer_file(out: &mut Vec<Issue>, label: &str, v: &Value, field: &str) {
    shape(
        out,
        label,
        v,
        &["path", "state", "content_digest", "bytes", "git_blob"],
        field,
    );
    if !v.is_object() {
        return;
    }
    if v["state"] != "present" {
        push(
            out,
            label,
            format!("budget receipt field {field}.state must be 'present'"),
        );
    }
    let p = string(&v["path"]);
    if p.is_empty()
        || Path::new(p).is_absolute()
        || Path::new(p)
            .components()
            .any(|c| matches!(c, Component::ParentDir))
    {
        push(
            out,
            label,
            format!("budget receipt field {field}.path must be a relative path"),
        );
    } else if p.chars().any(|c| c < ' ' || c == '\u{7f}') {
        push(
            out,
            label,
            format!("budget receipt field {field}.path must not contain control characters"),
        );
    }
    digest(
        out,
        label,
        &format!("{field}.content_digest"),
        &v["content_digest"],
        false,
    );
    if !v["git_blob"]
        .as_str()
        .is_some_and(|s| s.strip_prefix("sha1:").is_some_and(|s| hex(s, 40, 40)))
    {
        push(
            out,
            label,
            format!("budget receipt field {field}.git_blob must be sha1:<40 lowercase hex>"),
        );
    }
    if !nonnegative(&v["bytes"]) {
        push(
            out,
            label,
            format!("budget receipt field {field}.bytes must be a non-negative integer"),
        );
    }
}
fn declared_paths(pm: &Value, python: bool) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    for field in if python {
        &[
            "python_entrypoints",
            "python_import_closure",
            "schema_inputs",
        ][..]
    } else {
        &["schema_inputs"][..]
    } {
        for item in array(&pm[*field]) {
            if let Some(p) = item.as_str().filter(|p| python || !p.is_empty()) {
                paths.insert(p.into());
            }
        }
    }
    for field in ["manifest_path", "schema_path", "action_path"] {
        if let Some(p) = pm[field].as_str().filter(|p| python || !p.is_empty()) {
            paths.insert(p.into());
        }
    }
    paths
}
fn file_inventory(out: &mut Vec<Issue>, label: &str, files: &Value, pm: &Value) {
    if !files.is_array() || !pm.is_object() {
        return;
    }
    let expected = declared_paths(pm, true);
    let paths: Vec<_> = array(files)
        .iter()
        .filter_map(|f| f["path"].as_str())
        .collect();
    let actual: BTreeSet<_> = paths.iter().map(|s| s.to_string()).collect();
    let missing = expected.difference(&actual).cloned().collect::<Vec<_>>();
    let extra = actual.difference(&expected).cloned().collect::<Vec<_>>();
    let duplicate = actual.len() != paths.len();
    if !missing.is_empty() || !extra.is_empty() || duplicate {
        let mut details = Vec::new();
        if !missing.is_empty() {
            details.push(format!("missing={}", missing.join(",")));
        }
        if !extra.is_empty() {
            details.push(format!("extra={}", extra.join(",")));
        }
        if duplicate {
            details.push("duplicate=present".into());
        }
        push(
            out,
            label,
            format!(
                "budget receipt producer file inventory does not cover the declared procedure exactly ({})",
                details.join("; ")
            ),
        );
    }
}
fn pinned_owner(
    out: &mut Vec<Issue>,
    label: &str,
    files: &Value,
    pm: &Value,
    cancel: &GitState<'_>,
) -> io::Result<()> {
    let Some(root) = std::env::var_os("AOA_KAG_ROOT").filter(|v| !v.is_empty()) else {
        return Ok(());
    };
    let root = Path::new(&root);
    let revision = std::env::var("AOA_KAG_ACTION_REVISION")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("AOA_KAG_REVISION").ok())
        .unwrap_or_default();
    if !hex(&revision, 40, 64) {
        push(
            out,
            label,
            "budget receipt producer files require a pinned aoa-kag revision",
        );
        return Ok(());
    }
    if !root.is_dir() {
        push(
            out,
            label,
            "budget receipt producer files cannot access the pinned aoa-kag checkout",
        );
        if pm.is_object() {
            push(
                out,
                label,
                "budget receipt procedure manifest cannot access the pinned aoa-kag checkout",
            );
        }
        return Ok(());
    }
    let (code, _) = git(root, &["rev-parse", "--git-dir"], cancel)?;
    if code != 0 {
        push(
            out,
            label,
            "budget receipt producer files cannot read the pinned aoa-kag checkout",
        );
        return Ok(());
    }
    for f in array(files) {
        let Some(p) = f["path"].as_str().filter(|s| !s.is_empty()) else {
            continue;
        };
        if !relative(p) {
            continue;
        }
        let spec = format!("{revision}:{p}");
        let (code, blob) = git(root, &["rev-parse", "--verify", &spec], cancel)?;
        let bytes = git_bytes(root, &revision, p, cancel)?;
        let Some(bytes) = bytes.filter(|_| code == 0) else {
            push(
                out,
                label,
                format!("budget receipt producer file {p} is missing from pinned aoa-kag revision"),
            );
            continue;
        };
        if f["content_digest"] != sha256_bytes(&bytes) {
            push(
                out,
                label,
                format!(
                    "budget receipt producer file {p} content does not match pinned aoa-kag source"
                ),
            );
        }
        if f["bytes"] != json!(bytes.len()) {
            push(
                out,
                label,
                format!(
                    "budget receipt producer file {p} bytes do not match pinned aoa-kag source"
                ),
            );
        }
        if f["git_blob"]
            != format!(
                "sha1:{}",
                std::str::from_utf8(&blob).map_err(io::Error::other)?.trim()
            )
        {
            push(
                out,
                label,
                format!(
                    "budget receipt producer file {p} git_blob does not match pinned aoa-kag source"
                ),
            );
        }
    }
    if pm.is_object() {
        match git_bytes(root, &revision, PM, cancel)? {
            None => push(
                out,
                label,
                "budget receipt procedure manifest is missing from the pinned aoa-kag revision",
            ),
            Some(raw) => match serde_json::from_slice::<Value>(&raw) {
                Err(_) => push(
                    out,
                    label,
                    "budget receipt procedure manifest cannot be decoded from the pinned aoa-kag revision",
                ),
                Ok(payload) => {
                    if !payload.is_object() {
                        push(
                            out,
                            label,
                            "budget receipt procedure manifest payload must be an object",
                        );
                        return Ok(());
                    }
                    if pm["manifest_digest"] != sha256_bytes(&raw) {
                        push(
                            out,
                            label,
                            "budget receipt producer procedure manifest digest does not match pinned owner payload",
                        );
                    }
                    if pm.get("dynamic_imports").unwrap_or(&json!([]))
                        != payload.get("dynamic_imports").unwrap_or(&json!([]))
                    {
                        push(
                            out,
                            label,
                            "budget receipt producer procedure manifest dynamic_imports does not match pinned owner payload",
                        );
                    }
                    for field in [
                        "closure_mode",
                        "dynamic_import_policy",
                        "python_entrypoints",
                        "python_import_closure",
                        "schema_inputs",
                        "action_inputs",
                        "environment",
                        "dependencies",
                    ] {
                        if pm[field] != payload[field] {
                            push(
                                out,
                                label,
                                format!(
                                    "budget receipt producer procedure manifest {field} does not match pinned owner payload"
                                ),
                            );
                        }
                    }
                }
            },
        }
    }
    Ok(())
}
fn procedure(out: &mut Vec<Issue>, label: &str, pm: &Value, files: &Value, action: &Value) {
    if !pm.is_object() {
        push(
            out,
            label,
            "budget receipt producer identity procedure_manifest must be an object",
        );
        return;
    }
    let mut fields = PROCEDURE_FIELDS.to_vec();
    if pm.get("dynamic_imports").is_some() {
        fields.push("dynamic_imports");
    }
    shape(
        out,
        label,
        pm,
        &fields,
        "producer_identity.procedure_manifest",
    );
    for field in [
        "manifest_path",
        "schema_path",
        "closure_mode",
        "dynamic_import_policy",
        "action_path",
    ] {
        if !pm[field].as_str().is_some_and(|s| !s.is_empty()) {
            push(
                out,
                label,
                format!(
                    "budget receipt producer procedure manifest field {field} must be a non-empty string"
                ),
            );
        }
    }
    digest(
        out,
        label,
        "producer_identity.procedure_manifest.manifest_digest",
        &pm["manifest_digest"],
        false,
    );
    if files.is_array() {
        let records: Vec<_> = array(files)
            .iter()
            .filter(|f| f.is_object() && f["path"] == pm["manifest_path"])
            .collect();
        if records.len() != 1 {
            push(
                out,
                label,
                "budget receipt producer procedure manifest path must identify exactly one producer file",
            );
        } else if records[0]["content_digest"] != pm["manifest_digest"] {
            push(
                out,
                label,
                "budget receipt producer procedure manifest digest does not match its file record",
            );
        }
        if action.is_object() && action["path"] != pm["action_path"] {
            push(
                out,
                label,
                "budget receipt producer action path does not match procedure manifest",
            );
        }
    }
    for field in [
        "python_entrypoints",
        "python_import_closure",
        "schema_inputs",
        "action_inputs",
        "environment",
        "dependencies",
    ] {
        if !pm[field].as_array().is_some_and(|a| !a.is_empty()) {
            push(
                out,
                label,
                format!(
                    "budget receipt producer procedure manifest field {field} must be a non-empty array"
                ),
            );
        }
    }
    let empty = json!([]);
    let edges = pm.get("dynamic_imports").unwrap_or(&empty);
    if !edges.is_array() {
        push(
            out,
            label,
            "budget receipt procedure dynamic_imports must be an array",
        );
    } else {
        for edge in array(edges) {
            let Some(obj) = edge.as_object().filter(|o| {
                o.len() == 3
                    && ["kind", "source", "target"]
                        .iter()
                        .all(|f| o.contains_key(*f))
            }) else {
                push(
                    out,
                    label,
                    "budget receipt procedure dynamic import must declare kind, source and target only",
                );
                continue;
            };
            if !matches!(
                string(&obj["kind"]),
                "module_from_spec" | "spec_from_file_location"
            ) {
                push(
                    out,
                    label,
                    "budget receipt procedure dynamic import kind is unsupported",
                );
            }
            for field in ["source", "target"] {
                if !obj[field].as_str().is_some_and(relative)
                    || !pm["python_import_closure"].is_array()
                    || !array(&pm["python_import_closure"]).contains(&obj[field])
                {
                    push(
                        out,
                        label,
                        format!(
                            "budget receipt procedure dynamic import {field} must be a safe path in the declared import closure"
                        ),
                    );
                }
            }
        }
    }
    for (field, expected) in [
        ("manifest_path", PM),
        ("schema_path", PS),
        ("action_path", ACTION),
    ] {
        if pm[field] != expected {
            push(
                out,
                label,
                format!("budget receipt producer procedure manifest {field} must be {expected}"),
            );
        }
    }
}
fn canonical_output(m: &Value) -> Option<&str> {
    let files = m["compatibility"]["files"].as_array()?;
    let mut paths = Vec::new();
    for f in files {
        if f.is_object() && f["kind"] == "source" {
            let p = f["path"].as_str()?;
            if !relative(p) {
                return None;
            }
            paths.push(p);
        }
    }
    (paths.len() == 1).then(|| paths[0])
}
fn input(
    out: &mut Vec<Issue>,
    label: &str,
    v: &Value,
    name: &str,
    kind: &str,
    expected: Option<&str>,
    bound: &str,
) {
    let field = format!("producer_identity.execution_inputs.action_inputs.{name}");
    shape(out, label, v, INPUT_FIELDS, &field);
    if !v.is_object() {
        return;
    }
    if v["state"] != "set" {
        push(
            out,
            label,
            format!("budget receipt producer action input {name} state must be 'set'"),
        );
    }
    if v["kind"] != kind {
        push(
            out,
            label,
            format!("budget receipt producer action input {name} kind must be '{kind}'"),
        );
    }
    digest(
        out,
        label,
        &format!("{field}.value_digest"),
        &v["value_digest"],
        false,
    );
    if let Some(expected) = expected {
        if v["value_digest"] != sha256_bytes(expected.as_bytes()) {
            push(
                out,
                label,
                format!(
                    "budget receipt producer action input {name} value_digest does not match {bound}"
                ),
            );
        }
        if v["bytes"] != json!(expected.len()) {
            push(
                out,
                label,
                format!("budget receipt producer action input {name} bytes does not match {bound}"),
            );
        }
    }
}
fn environment(out: &mut Vec<Issue>, label: &str, value: &Value, pm: &Value) {
    if !value.as_array().is_some_and(|a| !a.is_empty()) {
        push(
            out,
            label,
            "budget receipt producer runtime environment must be a non-empty array",
        );
        return;
    }
    let expected: BTreeMap<_, _> = array(&pm["environment"])
        .iter()
        .filter_map(|v| Some((v["name"].as_str()?, v["role"].as_str()?)))
        .collect();
    let mut names = Vec::new();
    for (index, v) in array(value).iter().enumerate() {
        let field = format!("producer_identity.execution_inputs.environment[{index}]");
        shape(
            out,
            label,
            v,
            &["name", "role", "state", "kind", "value_digest", "bytes"],
            &field,
        );
        if !v.is_object() {
            continue;
        }
        let name = string(&v["name"]);
        let role = string(&v["role"]);
        if name.is_empty() {
            push(
                out,
                label,
                format!("budget receipt field {field}.name must be a non-empty string"),
            );
        } else {
            names.push(name);
            if let Some(expected) = expected.get(name) {
                if role != *expected {
                    push(
                        out,
                        label,
                        format!(
                            "budget receipt runtime environment {name} role does not match the producer procedure"
                        ),
                    );
                }
            } else {
                push(
                    out,
                    label,
                    format!(
                        "budget receipt runtime environment {name} is not declared by the producer procedure"
                    ),
                );
            }
        }
        if role.is_empty() {
            push(
                out,
                label,
                format!("budget receipt field {field}.role must be a non-empty string"),
            );
        }
        let state = string(&v["state"]);
        if !matches!(state, "set" | "unset") {
            push(
                out,
                label,
                format!("budget receipt field {field}.state must be 'set' or 'unset'"),
            );
        }
        if v["kind"] != "environment" {
            push(
                out,
                label,
                format!("budget receipt field {field}.kind must be 'environment'"),
            );
        }
        digest(
            out,
            label,
            &format!("{field}.value_digest"),
            &v["value_digest"],
            false,
        );
        if state == "unset" {
            if v["value_digest"] != sha256_bytes(b"<unset>") {
                push(
                    out,
                    label,
                    format!(
                        "budget receipt unset runtime environment {name} value_digest does not match the unset sentinel"
                    ),
                );
            }
            if v["bytes"] != 0 {
                push(
                    out,
                    label,
                    format!("budget receipt unset runtime environment {name} bytes must be zero"),
                );
            }
        } else if state == "set" {
            if let Some(actual) = std::env::var_os(name) {
                #[cfg(unix)]
                let bytes = {
                    use std::os::unix::ffi::OsStrExt;
                    actual.as_os_str().as_bytes()
                };
                #[cfg(not(unix))]
                let bytes = actual.to_str().unwrap_or("").as_bytes();
                if v["value_digest"] != sha256_bytes(bytes) {
                    push(
                        out,
                        label,
                        format!(
                            "budget receipt set runtime environment {name} value_digest does not match the current action environment"
                        ),
                    );
                }
                if v["bytes"] != json!(bytes.len()) {
                    push(
                        out,
                        label,
                        format!(
                            "budget receipt set runtime environment {name} bytes do not match the current action environment"
                        ),
                    );
                }
            } else {
                push(
                    out,
                    label,
                    format!(
                        "budget receipt set runtime environment {name} cannot be verified against the current action environment"
                    ),
                );
            }
        }
        if !nonnegative(&v["bytes"]) {
            push(
                out,
                label,
                format!("budget receipt field {field}.bytes must be a non-negative integer"),
            );
        }
    }
    names.sort();
    if names != expected.keys().copied().collect::<Vec<_>>()
        || names.iter().copied().collect::<BTreeSet<_>>().len() != names.len()
    {
        push(
            out,
            label,
            "budget receipt runtime environment does not match the producer procedure",
        );
    }
}
/// Live Python host observations are not portable producer-contract evidence.
/// The maintained Python checker performs these observations unconditionally;
/// native admission therefore fails closed until the stronger owner provides a
/// native verification route. Never execute a Python sidecar from this check.
fn live_python_runtime_issues(out: &mut Vec<Issue>, label: &str) {
    push(
        out,
        label,
        "budget receipt live Python runtime verification is unsupported by the native validator",
    );
}
fn constraint_supported(s: &str) -> bool {
    let re = regex::Regex::new(r"^(===|==|!=|>=|<=|>|<|~=)?\s*\d+(?:\.\d+)*$").unwrap();
    !s.trim().is_empty()
        && s.split(',')
            .all(|t| re.is_match(t.trim()) && !t.trim().starts_with("==="))
}
fn runtime(out: &mut Vec<Issue>, label: &str, e: &Value, pm: &Value) {
    let v = &e["interpreter"];
    let field = "producer_identity.execution_inputs.interpreter";
    shape(
        out,
        label,
        v,
        &[
            "implementation",
            "version",
            "invoked_path_digest",
            "resolved_path_digest",
            "artifact_digest",
        ],
        field,
    );
    if v.is_object() {
        for name in ["implementation", "version"] {
            if !v[name].as_str().is_some_and(|s| !s.is_empty()) {
                push(
                    out,
                    label,
                    format!("budget receipt field {field}.{name} must be a non-empty string"),
                );
            }
        }
        for name in [
            "invoked_path_digest",
            "resolved_path_digest",
            "artifact_digest",
        ] {
            digest(out, label, &format!("{field}.{name}"), &v[name], false);
        }
        let python = array(&pm["dependencies"])
            .iter()
            .find(|d| d["name"] == "python")
            .and_then(|d| d["version"].as_str());
        if let Some(version) = python {
            if v["version"] != version {
                push(
                    out,
                    label,
                    "budget receipt interpreter version does not match the declared python dependency",
                );
            }
            if !constraint_supported(version) {
                push(
                    out,
                    label,
                    "budget receipt declared python dependency constraint is unsupported",
                );
            }
        }
        if v["implementation"].is_string() && v["version"].is_string() {
            let path = sha256_bytes(b"<approved-python-interpreter>");
            for name in ["invoked_path_digest", "resolved_path_digest"] {
                if v[name] != path {
                    push(
                        out,
                        label,
                        format!(
                            "budget receipt interpreter {name} does not match the approved Python interpreter"
                        ),
                    );
                }
            }
            let expected = sha256_bytes(
                format!(
                    "{RUNTIME_PREFIX}python:{}:{}",
                    string(&v["implementation"]),
                    string(&v["version"])
                )
                .as_bytes(),
            );
            if v["artifact_digest"] != expected {
                push(
                    out,
                    label,
                    "budget receipt interpreter artifact_digest does not match the captured Python runtime contract",
                );
            }
        }
    }
    let deps = &e["dependencies"];
    if !deps.as_array().is_some_and(|a| !a.is_empty()) {
        push(
            out,
            label,
            "budget receipt producer runtime dependencies must be a non-empty array",
        );
        return;
    }
    let expected: BTreeMap<_, _> = array(&pm["dependencies"])
        .iter()
        .filter_map(|d| {
            d["version"].as_str()?;
            Some((d["name"].as_str()?, d))
        })
        .collect();
    let mut names = Vec::new();
    for (index, d) in array(deps).iter().enumerate() {
        let field = format!("producer_identity.execution_inputs.dependencies[{index}]");
        shape(
            out,
            label,
            d,
            &[
                "name",
                "declared_version",
                "required",
                "state",
                "resolved_version",
                "path_digest",
                "artifact_digest",
                "artifact_bytes",
                "artifact_files",
            ],
            &field,
        );
        if !d.is_object() {
            continue;
        }
        let name = string(&d["name"]);
        let declaration = expected.get(name);
        if name.is_empty() {
            push(
                out,
                label,
                format!("budget receipt field {field}.name must be a non-empty string"),
            );
        } else {
            names.push(name);
            if let Some(decl) = declaration {
                if d["declared_version"] != decl["version"] {
                    push(
                        out,
                        label,
                        format!(
                            "budget receipt runtime dependency {name} version does not match the producer procedure"
                        ),
                    );
                }
                if &d["required"] != decl.get("required").unwrap_or(&Value::Bool(true)) {
                    push(
                        out,
                        label,
                        format!(
                            "budget receipt runtime dependency {name} required flag does not match the producer procedure"
                        ),
                    );
                }
            } else {
                push(
                    out,
                    label,
                    format!(
                        "budget receipt runtime dependency {name} is not declared by the producer procedure"
                    ),
                );
            }
        }
        if !d["declared_version"]
            .as_str()
            .is_some_and(|s| !s.is_empty())
        {
            push(
                out,
                label,
                format!("budget receipt field {field}.declared_version must be a non-empty string"),
            );
        }
        if !d["required"].is_boolean() {
            push(
                out,
                label,
                format!("budget receipt field {field}.required must be a boolean"),
            );
        }
        let state = string(&d["state"]);
        if !matches!(state, "available" | "unavailable" | "declared") {
            push(
                out,
                label,
                format!("budget receipt field {field}.state is unsupported"),
            );
        }
        if d["required"] == true && state != "available" {
            push(
                out,
                label,
                format!("budget receipt required runtime dependency {name} must be available"),
            );
        }
        let resolved = &d["resolved_version"];
        if !resolved.is_null() && !resolved.is_string() {
            push(
                out,
                label,
                format!("budget receipt field {field}.resolved_version must be a string or null"),
            );
        }
        if state == "available" && !resolved.as_str().is_some_and(|s| !s.is_empty()) {
            push(
                out,
                label,
                format!(
                    "budget receipt available runtime dependency {name} must keep resolved_version"
                ),
            );
        }
        if matches!(state, "declared" | "unavailable") && !resolved.is_null() {
            push(
                out,
                label,
                format!(
                    "budget receipt {state} runtime dependency {name} must keep resolved_version null"
                ),
            );
        }
        if name == "python"
            && declaration.is_some()
            && state == "available"
            && resolved.as_str().is_some_and(|s| !s.is_empty())
            && v["version"].as_str().is_some_and(|s| !s.is_empty())
            && *resolved != v["version"]
        {
            push(
                out,
                label,
                "budget receipt runtime dependency python resolved_version does not match the captured interpreter",
            );
        }
        for f in ["path_digest", "artifact_digest"] {
            if !d[f].is_null() {
                digest(out, label, &format!("{field}.{f}"), &d[f], false);
            } else if state == "available" {
                push(
                    out,
                    label,
                    format!("budget receipt available runtime dependency {name} must keep {f}"),
                );
            }
        }
        if d["name"].is_string()
            && d["declared_version"].is_string()
            && matches!(state, "available" | "declared")
        {
            let expected_path =
                sha256_bytes(format!("<approved-dependency-root>/{name}").as_bytes());
            if d["path_digest"] != expected_path {
                push(
                    out,
                    label,
                    format!(
                        "budget receipt runtime dependency {name} path_digest does not match the approved dependency contract"
                    ),
                );
            }
            let expected = sha256_bytes(
                format!(
                    "{RUNTIME_PREFIX}dependency:{name}:{}",
                    string(&d["declared_version"])
                )
                .as_bytes(),
            );
            if d["artifact_digest"] != expected {
                push(
                    out,
                    label,
                    format!(
                        "budget receipt runtime dependency {name} artifact_digest does not match the approved dependency contract"
                    ),
                );
            }
            if d["artifact_bytes"] != json!(expected.len()) {
                push(
                    out,
                    label,
                    format!(
                        "budget receipt runtime dependency {name} artifact_bytes does not match the approved dependency contract"
                    ),
                );
            }
            if d["artifact_files"] != 1 {
                push(
                    out,
                    label,
                    format!(
                        "budget receipt runtime dependency {name} artifact_files does not match the approved dependency contract"
                    ),
                );
            }
        }
        for f in ["artifact_bytes", "artifact_files"] {
            if !nonnegative(&d[f]) {
                push(
                    out,
                    label,
                    format!("budget receipt field {field}.{f} must be a non-negative integer"),
                );
            }
        }
    }
    names.sort();
    if names != expected.keys().copied().collect::<Vec<_>>()
        || names.iter().copied().collect::<BTreeSet<_>>().len() != names.len()
    {
        push(
            out,
            label,
            "budget receipt runtime dependencies do not match the producer procedure",
        );
    }
}
fn non_python(out: &mut Vec<Issue>, label: &str, value: &Value, pm: &Value, files: &Value) {
    if !value.as_array().is_some_and(|a| !a.is_empty()) {
        push(
            out,
            label,
            "budget receipt producer non-Python inputs must be a non-empty array",
        );
        return;
    }
    let expected = declared_paths(pm, false);
    let mut actual = Vec::new();
    for (index, v) in array(value).iter().enumerate() {
        let field = format!("producer_identity.execution_inputs.non_python_inputs[{index}]");
        shape(out, label, v, &["path", "content_digest", "bytes"], &field);
        if !v.is_object() {
            continue;
        }
        let p = string(&v["path"]);
        if !relative(p) {
            push(
                out,
                label,
                format!("budget receipt field {field}.path must be a relative path"),
            );
            continue;
        }
        actual.push(p.to_owned());
        if !expected.contains(p) {
            push(
                out,
                label,
                format!(
                    "budget receipt non-Python input path {p} is not declared by the producer procedure"
                ),
            );
        }
        digest(
            out,
            label,
            &format!("{field}.content_digest"),
            &v["content_digest"],
            false,
        );
        if !nonnegative(&v["bytes"]) {
            push(
                out,
                label,
                format!("budget receipt field {field}.bytes must be a non-negative integer"),
            );
        }
        let records: Vec<_> = array(files).iter().filter(|f| f["path"] == p).collect();
        if records.len() != 1 {
            push(
                out,
                label,
                format!(
                    "budget receipt non-Python input path {p} must identify exactly one producer file"
                ),
            );
        } else {
            if v["content_digest"] != records[0]["content_digest"] {
                push(
                    out,
                    label,
                    format!(
                        "budget receipt non-Python input {p} digest does not match producer file"
                    ),
                );
            }
            if v["bytes"] != records[0]["bytes"] {
                push(
                    out,
                    label,
                    format!("budget receipt non-Python input {p} bytes do not match producer file"),
                );
            }
        }
    }
    actual.sort();
    if actual != expected.into_iter().collect::<Vec<_>>()
        || actual.iter().collect::<BTreeSet<_>>().len() != actual.len()
    {
        push(
            out,
            label,
            "budget receipt non-Python inputs do not match the producer procedure and file inventory",
        );
    }
}
fn identity(
    root: &Path,
    s: &mut RouteSources,
    out: &mut Vec<Issue>,
    label: &str,
    m: &Value,
    r: &Value,
    d: &str,
    cancel: &GitState<'_>,
    interpreter: Option<&Path>,
) -> io::Result<()> {
    let c = &r["candidate_identity"];
    shape(out, label, c, CANDIDATE_FIELDS, "candidate_identity");
    if c.is_object() {
        for (field, expected, msg) in [
            (
                "contract_version",
                json!(CANDIDATE),
                "budget receipt candidate identity contract is not v2",
            ),
            (
                "algorithm",
                json!(ALGORITHM),
                "budget receipt candidate identity algorithm is not v2",
            ),
            (
                "base_ref",
                r["base_ref"].clone(),
                "budget receipt candidate identity base_ref does not match receipt",
            ),
            (
                "family_digest",
                json!(d),
                "budget receipt candidate identity family digest does not match receipt",
            ),
            (
                "source_snapshot",
                r["head_source_snapshot"].clone(),
                "budget receipt candidate identity source snapshot does not match receipt",
            ),
            (
                "excluded_path",
                json!(format!("kag/receipts/index_family_budget/{d}.json")),
                "budget receipt candidate identity excluded path is not canonical",
            ),
        ] {
            if c[field] != expected {
                push(out, label, msg);
            }
        }
        if !nonnegative(&c["file_count"]) {
            push(
                out,
                label,
                "budget receipt candidate identity file_count must be non-negative",
            );
        }
        for field in ["seal", "family_digest"] {
            digest(
                out,
                label,
                &format!("candidate_identity.{field}"),
                &c[field],
                false,
            );
        }
        for field in ["source_snapshot", "source_epoch"] {
            digest(
                out,
                label,
                &format!("candidate_identity.{field}"),
                &c[field],
                true,
            );
        }
        if !c["base_ref"].as_str().is_some_and(|s| hex(s, 40, 64)) {
            push(
                out,
                label,
                "budget receipt candidate identity base_ref must be lowercase Git commit ref",
            );
        }
        match candidate_seal(root, s, d, cancel) {
            Err(e) => push(
                out,
                label,
                format!("budget receipt candidate seal cannot be recomputed: {e}"),
            ),
            Ok((seal, count)) => {
                if c["seal"] != seal {
                    push(
                        out,
                        label,
                        "budget receipt candidate identity seal does not match the current candidate",
                    );
                }
                if c["file_count"] != json!(count) {
                    push(
                        out,
                        label,
                        "budget receipt candidate identity file_count does not match the current candidate",
                    );
                }
            }
        }
        if digest_ok(&c["source_epoch"], true) {
            match source_epoch(root, s, cancel) {
                Err(e) => push(
                    out,
                    label,
                    format!("budget receipt candidate source epoch cannot be recomputed: {e}"),
                ),
                Ok(epoch) => {
                    if c["source_epoch"] != epoch {
                        push(
                            out,
                            label,
                            "budget receipt candidate identity source epoch does not match the current source",
                        );
                    }
                }
            }
        }
    }
    if !m["family_identity"].is_object() {
        push(
            out,
            label,
            "family manifest family_identity must be an object",
        );
    } else if !m["family_identity"]["source_snapshot"].is_string() {
        push(
            out,
            label,
            "family manifest source snapshot must be a string",
        );
    } else if r["head_source_snapshot"] != m["family_identity"]["source_snapshot"] {
        push(
            out,
            label,
            "budget receipt source snapshot does not match the family manifest",
        );
    }
    let p = &r["producer_identity"];
    shape(out, label, p, PRODUCER_FIELDS, "producer_identity");
    if !p.is_object() {
        return Ok(());
    }
    if p["owner"] != "aoa-kag" {
        push(
            out,
            label,
            "budget receipt producer identity owner must be aoa-kag",
        );
    }
    let profile = match string(&p["contract_version"]) {
        "aoa-kag:budget-receipt-producer-identity-v3" => Some((
            "content-addressed-procedure-import-closure-runtime-inputs-and-descriptor-io-v1",
            "aoa-kag:budget-receipt-producer-runtime-inputs-v1",
        )),
        "aoa-kag:budget-receipt-producer-identity-v4" => Some((
            "content-addressed-procedure-import-closure-portable-runtime-contract-and-descriptor-io-v1",
            "aoa-kag:budget-receipt-producer-runtime-inputs-v2",
        )),
        _ => None,
    };
    if let Some((binding, _)) = profile {
        if p["revision_binding"] != binding {
            push(
                out,
                label,
                "budget receipt producer identity revision binding does not match its contract version",
            );
        }
    } else {
        push(
            out,
            label,
            "budget receipt producer identity contract is unsupported",
        );
    }
    for field in ["source_digest", "identity_digest"] {
        digest(
            out,
            label,
            &format!("producer_identity.{field}"),
            &p[field],
            false,
        );
    }
    let files = &p["files"];
    let action = &p["action"];
    producer_file(out, label, action, "producer_identity.action");
    if !files.as_array().is_some_and(|a| !a.is_empty()) {
        push(
            out,
            label,
            "budget receipt producer identity files must be a non-empty array",
        );
    } else {
        for (i, f) in array(files).iter().enumerate() {
            producer_file(out, label, f, &format!("producer_identity.files[{i}]"));
        }
        if p["source_digest"] != canon(files)? {
            push(
                out,
                label,
                "budget receipt producer source digest does not match its files",
            );
        }
        if action.is_object() {
            let records: Vec<_> = array(files)
                .iter()
                .filter(|f| f.is_object() && f["path"] == action["path"])
                .collect();
            if records.len() != 1 {
                push(
                    out,
                    label,
                    "budget receipt producer action path must identify exactly one producer file",
                );
            } else if records[0] != action {
                push(
                    out,
                    label,
                    "budget receipt producer action does not match its file record",
                );
            }
        }
    }
    let fields = [
        "contract_version",
        "owner",
        "revision_binding",
        "source_digest",
        "procedure_manifest",
        "action",
        "execution_inputs",
    ];
    if fields.iter().all(|f| p.get(*f).is_some()) {
        let material: serde_json::Map<String, Value> = fields
            .iter()
            .map(|f| (f.to_string(), p[*f].clone()))
            .collect();
        if p["identity_digest"] != canon(&Value::Object(material))? {
            push(
                out,
                label,
                "budget receipt producer identity digest does not match its identity material",
            );
        }
    }
    let pm = &p["procedure_manifest"];
    procedure(out, label, pm, files, action);
    file_inventory(out, label, files, pm);
    pinned_owner(out, label, files, pm, cancel)?;
    let e = &p["execution_inputs"];
    shape(
        out,
        label,
        e,
        &[
            "schema_version",
            "action_inputs",
            "environment",
            "dependencies",
            "interpreter",
            "non_python_inputs",
            "dynamic_imports",
            "command_targets",
            "manifest_digest",
        ],
        "producer_identity.execution_inputs",
    );
    if !e.is_object() {
        return Ok(());
    }
    if e["schema_version"]
        != profile
            .map(|(_, runtime)| json!(runtime))
            .unwrap_or(Value::Null)
    {
        push(
            out,
            label,
            "budget receipt producer runtime-input schema does not match its contract version",
        );
    }
    digest(
        out,
        label,
        "producer_identity.execution_inputs.manifest_digest",
        &e["manifest_digest"],
        false,
    );
    if e["dynamic_imports"] != json!([]) {
        push(
            out,
            label,
            "budget receipt producer runtime-input dynamic_imports must be empty",
        );
    }
    let legacy = array(&pm["action_inputs"]).contains(&json!("jobs"));
    let a = &e["action_inputs"];
    let mut af = vec!["repo-root", "output", "history-ref", "event-history-ref"];
    if legacy {
        af.push("jobs");
    }
    shape(
        out,
        label,
        a,
        &af,
        "producer_identity.execution_inputs.action_inputs",
    );
    if a.is_object() {
        input(
            out,
            label,
            &a["repo-root"],
            "repo-root",
            "path",
            Some("<owner-root>"),
            "canonical owner root",
        );
    }
    let output = canonical_output(m);
    if output.is_none() {
        push(
            out,
            label,
            "budget receipt canonical family output path is missing",
        );
    } else if a.is_object() {
        input(
            out,
            label,
            &a["output"],
            "output",
            "relative-path",
            output,
            "canonical family output",
        );
    }
    if a.is_object() {
        if let Some(base) = r["base_ref"].as_str() {
            for name in ["history-ref", "event-history-ref"] {
                input(
                    out,
                    label,
                    &a[name],
                    name,
                    "git-ref",
                    Some(base),
                    "receipt base_ref",
                );
            }
        }
    }
    let t = &e["command_targets"];
    let mut tf = vec![
        "repo_root",
        "base_ref",
        "history_ref",
        "event_history_ref",
        "output",
        "family_mode",
        "artifact_root",
        "externalized",
    ];
    if legacy {
        tf.push("jobs");
    }
    shape(
        out,
        label,
        t,
        &tf,
        "producer_identity.execution_inputs.command_targets",
    );
    if legacy && a.is_object() {
        let jobs = t["jobs"].as_str().filter(|s| matches!(*s, "1" | "2" | "3"));
        input(
            out,
            label,
            &a["jobs"],
            "jobs",
            "bounded-integer",
            jobs,
            "command target jobs",
        );
        if jobs.is_none() {
            push(
                out,
                label,
                "budget receipt producer command target jobs must be a bounded integer in [1, 3]",
            );
        }
    }
    if t.is_object() {
        shape(
            out,
            label,
            &t["repo_root"],
            &["path_digest", "resolved_path_digest"],
            "producer_identity.execution_inputs.command_targets.repo_root",
        );
        if t["repo_root"].is_object() {
            let expected = sha256_bytes(b"<owner-root>");
            for field in ["path_digest", "resolved_path_digest"] {
                digest(
                    out,
                    label,
                    &format!(
                        "producer_identity.execution_inputs.command_targets.repo_root.{field}"
                    ),
                    &t["repo_root"][field],
                    false,
                );
                if t["repo_root"][field] != expected {
                    push(
                        out,
                        label,
                        format!(
                            "budget receipt producer command target repo_root {field} does not match canonical owner root"
                        ),
                    );
                }
                if a["repo-root"].is_object()
                    && t["repo_root"][field] != a["repo-root"]["value_digest"]
                {
                    push(
                        out,
                        label,
                        format!(
                            "budget receipt producer command target repo_root {field} does not match action input repo-root"
                        ),
                    );
                }
            }
        }
        let mode = if m["schema_version"] == SEG {
            "segmented"
        } else {
            "portable"
        };
        if t["family_mode"] != mode {
            push(
                out,
                label,
                format!("budget receipt producer command target family_mode must be '{mode}'"),
            );
        }
        if !t["artifact_root"].is_null() {
            push(
                out,
                label,
                format!(
                    "budget receipt producer command target artifact_root must be null for {mode} family"
                ),
            );
        }
        if t["externalized"] != false {
            push(
                out,
                label,
                format!(
                    "budget receipt producer command target externalized must be false for {mode} family"
                ),
            );
        }
        if legacy
            && !t["jobs"]
                .as_str()
                .is_some_and(|s| matches!(s, "1" | "2" | "3"))
        {
            push(
                out,
                label,
                "budget receipt producer command target jobs must be a bounded integer in [1, 3]",
            );
        }
        if r["base_ref"].is_string() {
            for field in ["base_ref", "history_ref", "event_history_ref"] {
                if t[field] != r["base_ref"] {
                    push(
                        out,
                        label,
                        format!(
                            "budget receipt producer command target {field} does not match receipt base_ref"
                        ),
                    );
                }
            }
        }
        if let Some(output) = output {
            if t["output"] != output {
                push(
                    out,
                    label,
                    "budget receipt producer command target output does not match canonical family output",
                );
            }
        }
    }
    if pm.is_object()
        && e["manifest_digest"].is_string()
        && pm["manifest_digest"] != e["manifest_digest"]
    {
        push(
            out,
            label,
            "budget receipt producer procedure manifest digest does not match execution inputs",
        );
    }
    environment(out, label, &e["environment"], pm);
    runtime(out, label, e, pm);
    verify_live_runtime(root, s, out, label, e, pm, interpreter, cancel)?;
    non_python(out, label, &e["non_python_inputs"], pm, files);
    Ok(())
}
pub fn budget_receipt_contract_issues(
    root: &Path,
    s: &mut RouteSources,
    m: &Value,
    r: &Value,
    d: &str,
    label: &str,
    base_override: Option<bool>,
    fetch: bool,
    require_v2: bool,
    cancel: &AtomicI32,
    interpreter: Option<&Path>,
) -> io::Result<Vec<Issue>> {
    let state = GitState {
        deadline: Instant::now() + s.remaining_time()?,
        bytes: Cell::new(0),
        source_bytes: Cell::new(0),
        cancel,
    };
    let cancel = &state;
    let mut out = Vec::new();
    if !r.is_object() {
        push(&mut out, label, "budget receipt must be a JSON object");
        return Ok(out);
    }
    let v2 = r["schema_version"] == V2;
    if require_v2 && !v2 {
        push(
            &mut out,
            label,
            "current generated KAG budget receipt must use the identity-bound v2 schema",
        );
    }
    let mut fields = COMMON.to_vec();
    if v2 {
        fields.extend([
            "head_source_snapshot",
            "candidate_identity",
            "producer_identity",
        ]);
    }
    let expected: BTreeSet<_> = fields.iter().copied().collect();
    for f in &expected {
        if r.get(*f).is_none() {
            push(
                &mut out,
                label,
                format!("budget receipt missing required field {f}"),
            );
        }
    }
    let mut strings = vec![
        "schema_version",
        "repo",
        "scope",
        "base_ref",
        "head_family_digest",
        "reason",
        "approved_by",
        "decision_ref",
    ];
    if v2 {
        strings.push("head_source_snapshot");
    }
    for f in strings {
        if r.get(f).is_some() && !r[f].is_string() {
            push(
                &mut out,
                label,
                format!("budget receipt field {f} must be a string"),
            );
        }
    }
    for f in [
        "changed_generated_bytes",
        "changed_generated_files",
        "default_limit_bytes",
        "allowed_bytes",
        "tracked_bytes",
        "tracked_bytes_max",
        "allowed_tracked_bytes",
    ] {
        if let Some(v) = r.get(f) {
            if integer(v).is_none() {
                push(
                    &mut out,
                    label,
                    format!("budget receipt field {f} must be an integer"),
                );
            } else if !nonnegative(v) {
                push(
                    &mut out,
                    label,
                    format!("budget receipt field {f} must not be negative"),
                );
            }
        }
    }
    if r["schema_version"].is_string() && r["schema_version"] != V1 && !v2 {
        push(
            &mut out,
            label,
            "budget receipt schema_version must be one of ['aoa-repo-local-kag-budget-receipt-v1', 'aoa-repo-local-kag-budget-receipt-v2']",
        );
    }
    if v2 {
        for f in &expected {
            if r.get(*f).is_none() {
                push(
                    &mut out,
                    label,
                    format!("budget receipt missing required field {f}"),
                );
            }
        }
        for f in r.as_object().unwrap().keys().collect::<BTreeSet<_>>() {
            if !expected.contains(f.as_str()) {
                push(
                    &mut out,
                    label,
                    format!("budget receipt has unexpected field {f}"),
                );
            }
        }
        digest(
            &mut out,
            label,
            "head_source_snapshot",
            &r["head_source_snapshot"],
            true,
        );
        identity(root, s, &mut out, label, m, r, d, cancel, interpreter)?;
    }
    if m["repo"]["name"].is_string() && r["repo"] != m["repo"]["name"] {
        push(
            &mut out,
            label,
            "budget receipt repo must match the family repository",
        );
    }
    if m["repo"].is_object() {
        if !m["repo"]["git_ref"]
            .as_str()
            .is_some_and(|s| !s.trim().is_empty())
        {
            push(
                &mut out,
                label,
                "family repository must keep a non-empty history ref",
            );
        }
    } else {
        push(
            &mut out,
            label,
            "family manifest repo must be an object with a history ref",
        );
    }
    let scope = string(&r["scope"]);
    let recognized = matches!(
        scope,
        "generated_delta"
            | "tracked_size"
            | "generated_delta_and_tracked_size"
            | "v2_to_v3_migration"
    );
    if r["scope"].is_string() && !recognized {
        push(
            &mut out,
            label,
            "budget receipt scope is not recognized by the aoa-kag v1 contract",
        );
    }
    let base = string(&r["base_ref"]);
    if r["base_ref"].is_string() && !hex(base, 40, 64) {
        push(
            &mut out,
            label,
            "budget receipt base_ref must be a lowercase Git commit ref",
        );
    }
    if r["head_family_digest"] != d {
        push(
            &mut out,
            label,
            "budget receipt is not bound to the current family digest",
        );
    }
    for (field, value) in [
        (
            "default_limit_bytes",
            &m["budgets"]["changed_generated_bytes_max"],
        ),
        ("tracked_bytes_max", &m["budgets"]["tracked_bytes_max"]),
        ("tracked_bytes", &m["summary"]["tracked_bytes"]),
    ] {
        if !value.is_null() && r[field] != *value {
            push(
                &mut out,
                label,
                format!("budget receipt field {field} does not match the family contract"),
            );
        }
    }
    let decision = if m["schema_version"] == SEG {
        "AOA-KAG-D-0051-bounded-segmented-kag-family.md"
    } else if m["schema_version"] == TIER {
        "AOA-KAG-D-0039-tiered-content-addressed-kag-distribution.md"
    } else {
        "AOA-KAG-D-0017-portable-content-addressed-repository-family.md"
    };
    if r["decision_ref"] != format!("aoa-kag:docs/decisions/{decision}") {
        push(
            &mut out,
            label,
            "budget receipt field decision_ref does not match the family contract",
        );
    }
    if v2 && hex(base, 40, 64) {
        match changed_measurements(root, s, m, base, cancel) {
            Err(e) => push(
                &mut out,
                label,
                format!("budget receipt generated delta cannot be recomputed: {e}"),
            ),
            Ok((bytes, files)) => {
                for (field, value) in [
                    ("changed_generated_bytes", bytes),
                    ("changed_generated_files", files),
                    ("allowed_bytes", bytes),
                ] {
                    if r[field] != json!(value) {
                        push(
                            &mut out,
                            label,
                            format!(
                                "budget receipt field {field} does not match current generated delta"
                            ),
                        );
                    }
                }
            }
        }
    }
    let inputs = [
        &r["changed_generated_bytes"],
        &m["budgets"]["changed_generated_bytes_max"],
        &m["summary"]["tracked_bytes"],
        &m["budgets"]["tracked_bytes_max"],
    ];
    let mut relation = None;
    if inputs.iter().all(|v| integer(v).is_some()) {
        match budget_exceedance_relation(inputs[0], inputs[1], inputs[2], inputs[3]) {
            Ok(r) => relation = r,
            Err(e) => push(
                &mut out,
                label,
                format!("budget receipt exceedance relation is malformed: {e}"),
            ),
        }
    }
    if recognized {
        let state = match base_override {
            Some(b) => Some(b),
            None => base_has_v3(root, base, fetch, cancel)?,
        };
        match state {
            None => push(
                &mut out,
                label,
                if scope == "v2_to_v3_migration" {
                    "cannot verify v2_to_v3_migration base family"
                } else {
                    "cannot verify budget receipt base family"
                },
            ),
            Some(state) => {
                if relation.is_none() {
                    push(
                        &mut out,
                        label,
                        "budget receipt scope cannot authorize a family with no exceeded budget dimension",
                    );
                } else if scope == "v2_to_v3_migration" {
                    if state {
                        push(
                            &mut out,
                            label,
                            "v2_to_v3_migration requires a base without a v3 family manifest",
                        );
                    }
                } else if !state {
                    push(
                        &mut out,
                        label,
                        "budget receipt scope must be 'v2_to_v3_migration' for a pre-v3 base family",
                    );
                } else if Some(scope) != canonical_budget_scope(relation, true) {
                    push(
                        &mut out,
                        label,
                        format!(
                            "budget receipt scope does not match the current exceedance: expected '{}'",
                            relation.unwrap()
                        ),
                    );
                }
            }
        }
    }
    for (allowed, measured, msg) in [
        (
            "allowed_bytes",
            "changed_generated_bytes",
            "budget receipt allowed_bytes must cover changed_generated_bytes",
        ),
        (
            "allowed_tracked_bytes",
            "tracked_bytes",
            "budget receipt allowed_tracked_bytes must cover tracked_bytes",
        ),
    ] {
        if let (Some(a), Some(b)) = (integer(&r[allowed]), integer(&r[measured])) {
            if a < b {
                push(&mut out, label, msg);
            }
        }
    }
    for f in ["reason", "approved_by"] {
        if r[f].as_str().is_some_and(|s| s.trim().is_empty()) {
            push(
                &mut out,
                label,
                format!("budget receipt {f} must not be empty"),
            );
        }
    }
    if out.last().is_some_and(|(_, m)| m == DIAGNOSTIC_LIMIT) {
        return Err(invalid(DIAGNOSTIC_LIMIT));
    }
    s.check()?;
    Ok(out)
}
fn external_relative(value: &Value) -> Option<&str> {
    value.as_str().filter(|s| {
        !s.is_empty()
            && !s.contains('\0')
            && !Path::new(s).is_absolute()
            && !Path::new(s)
                .components()
                .any(|c| matches!(c, Component::ParentDir | Component::Prefix(_)))
    })
}
fn external_generated_family_issues(
    s: &mut RouteSources,
    port: &Value,
    f: &Value,
) -> io::Result<Vec<Issue>> {
    let mut out = Vec::new();
    for field in ["manifest", "segments", "receipt_root"] {
        if external_relative(&f[field]).is_none() {
            push(
                &mut out,
                "kag_provider",
                format!("generated_family.{field} must be a safe external provider-relative route"),
            );
        }
    }
    for field in ["builder", "validator"] {
        if !f[field].as_str().is_some_and(|v| v.starts_with("aoa-kag:")) {
            push(
                &mut out,
                "kag_provider",
                format!("generated_family.{field} must keep the aoa-kag owner handle"),
            );
        }
    }
    let mut routes = BTreeMap::new();
    for field in [
        "provider_template",
        "owner_route",
        "publication_route",
        "validation_route",
    ] {
        let Some(path) = external_relative(&f[field]) else {
            push(
                &mut out,
                "kag_provider",
                format!("generated_family.{field} must be an authored repository-relative route"),
            );
            continue;
        };
        routes.insert(field, path);
        if !s.is_file(path)? {
            push(
                &mut out,
                path,
                format!("external KAG {field} route is missing"),
            );
        }
    }
    if routes.get("provider_template").copied() != port["manifest"].as_str() {
        push(
            &mut out,
            "kag_provider",
            "external KAG provider_template must match the port manifest route",
        );
    }
    if routes.get("owner_route").copied() != port["local_owner"].as_str() {
        push(
            &mut out,
            "kag_provider",
            "external KAG owner_route must match the local owner",
        );
    }
    if let Some(path) = routes.get("provider_template").copied() {
        if s.is_file(path)? {
            // The authored publisher's existing template operand ceiling.
            let mut read = 0;
            let template = match s.bounded_bytes(path, 65536, &mut read, 65536) {
                Ok(raw) => {
                    let raw = raw.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&raw);
                    let value = if budget_json_preflight(raw).is_ok() {
                        serde_json::from_slice::<Value>(raw).unwrap_or(Value::Null)
                    } else {
                        Value::Null
                    };
                    s.check()?;
                    value
                }
                Err(e) => {
                    s.check()?;
                    if e.kind() == io::ErrorKind::Interrupted {
                        return Err(e);
                    }
                    Value::Null
                }
            };
            let declared = &template["files"]["kag/manifest.json"];
            let classes = declared["record_classes"].as_array();
            let valid_classes = classes.is_some_and(|classes| {
                classes.iter().all(Value::is_string)
                    && classes
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<BTreeSet<_>>()
                        == BTreeSet::from(["node", "edge", "index", "projection", "receipt"])
            });
            let route = format!(
                "Tree-of-Sophia:{}",
                routes.get("publication_route").copied().unwrap_or("")
            );
            let publication_bound = declared["validation_routes"].as_array().is_some_and(|a| {
                a.iter()
                    .any(|entry| entry.is_object() && entry["route"].as_str() == Some(&route))
            });
            if template["schema_version"].as_str() != Some("tos_kag_provider_template_v1")
                || !declared.is_object()
                || declared["schema_version"].as_str() != Some("aoa-local-kag-manifest-v1")
                || declared["repo"].as_str() != Some("Tree-of-Sophia")
                || declared["owner_surface"].as_str() != routes.get("owner_route").copied()
                || !valid_classes
                || !publication_bound
            {
                push(
                    &mut out,
                    path,
                    "external KAG provider template does not bind its schema, owner, record classes and publication route",
                );
            }
        }
    }
    s.check()?;
    Ok(out)
}

pub fn generated_family_issues(
    root: &Path,
    s: &mut RouteSources,
    port: &Value,
    fetch: bool,
    interpreter: Option<&Path>,
    cancel: &AtomicI32,
) -> io::Result<Vec<Issue>> {
    let mut out = Vec::new();
    let f = &port["generated_family"];
    if !f.is_object() {
        return Ok(vec![(
            "kag_provider".into(),
            "generated_family carrier declaration is required".into(),
        )]);
    }
    match f["scope"].as_str() {
        Some("external_integration_release") => return external_generated_family_issues(s, port, f),
        None if f["scope"].is_null() => {},
        Some("selected_local_family") => {},
        _ => return Ok(vec![("kag_provider".into(), "generated_family.scope must select external_integration_release or selected_local_family".into())]),
    }
    let manifest = f["manifest"].as_str();
    let receipt_root = f["receipt_root"].as_str();
    let carrier = f["segments"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(|s| ("segments", s))
        .or_else(|| f["shards"].as_str().map(|s| ("shards", s)));
    for (label, p) in [
        ("manifest", manifest),
        (carrier.map_or("shards", |c| c.0), carrier.map(|c| c.1)),
        ("receipt_root", receipt_root),
    ] {
        let Some(p) = p.filter(|s| !s.is_empty()) else {
            push(
                &mut out,
                "kag_provider",
                format!("generated_family.{label} is required"),
            );
            continue;
        };
        if label == "manifest" {
            if !s.is_file(p)? {
                push(&mut out, p, "generated KAG family manifest is missing");
            }
        } else if !s.is_dir(p)? {
            push(
                &mut out,
                p,
                format!("generated KAG family {label} route is missing"),
            );
        }
    }
    for field in ["builder", "validator"] {
        if !f[field].as_str().is_some_and(|s| s.starts_with("aoa-kag:")) {
            push(
                &mut out,
                "kag_provider",
                format!("generated_family.{field} must keep the aoa-kag owner handle"),
            );
        }
    }
    let (Some(manifest), Some(receipt_root)) = (manifest, receipt_root) else {
        return Ok(out);
    };
    if !s.is_file(manifest)? {
        return Ok(out);
    }
    let raw = s.bytes(manifest)?;
    budget_json_preflight(&raw)?;
    let m: Value = match serde_json::from_slice(&raw) {
        Ok(m) => m,
        Err(_) => {
            push(
                &mut out,
                manifest,
                "generated KAG family manifest lacks family_identity.content_digest",
            );
            return Ok(out);
        }
    };
    let Some(digest_value) = m
        .get("family_identity")
        .and_then(|v| v.get("content_digest"))
    else {
        push(
            &mut out,
            manifest,
            "generated KAG family manifest lacks family_identity.content_digest",
        );
        return Ok(out);
    };
    let Some(d) = digest_value.as_str() else {
        push(
            &mut out,
            manifest,
            "generated KAG family digest must be 64 lowercase hex characters",
        );
        return Ok(out);
    };
    if !hex(d, 64, 64) {
        push(
            &mut out,
            manifest,
            "generated KAG family digest must be 64 lowercase hex characters",
        );
        return Ok(out);
    }
    let rp = format!("{receipt_root}/{d}.json");
    if !s.is_file(&rp)? {
        push(
            &mut out,
            &rp,
            "matching generated KAG budget receipt is missing",
        );
        return Ok(out);
    }
    let raw = s.bytes(&rp)?;
    budget_json_preflight(&raw)?;
    let r: Value = match serde_json::from_slice(&raw) {
        Ok(r) => r,
        Err(_) => {
            push(
                &mut out,
                &rp,
                "matching generated KAG budget receipt is invalid",
            );
            return Ok(out);
        }
    };
    out.extend(budget_receipt_contract_issues(
        root,
        s,
        &m,
        &r,
        d,
        &rp,
        None,
        fetch,
        true,
        cancel,
        interpreter,
    )?);
    Ok(out)
}
fn satisfies(constraint: &str, actual: &[BigInt]) -> Option<bool> {
    let re = regex::Regex::new(r"^(===|==|!=|>=|<=|>|<|~=)?\s*(\d+(?:\.\d+)*)$").ok()?;
    if constraint.trim().is_empty() || actual.is_empty() {
        return None;
    }
    for term in constraint.split(',') {
        let c = re.captures(term.trim())?;
        let op = c.get(1).map_or("==", |m| m.as_str());
        if op == "===" {
            return None;
        }
        let expected: Vec<BigInt> = c
            .get(2)?
            .as_str()
            .split('.')
            .map(|p| p.parse().ok())
            .collect::<Option<_>>()?;
        let width = actual.len().max(expected.len()).max(3);
        let mut left = actual.to_vec();
        left.resize(width, BigInt::from(0));
        let mut right = expected.clone();
        right.resize(width, BigInt::from(0));
        let valid = match op {
            ">=" => left >= right,
            ">" => left > right,
            "<=" => left <= right,
            "<" => left < right,
            "==" => left == right,
            "!=" => left != right,
            "~=" => {
                let index = expected.len().saturating_sub(2);
                let mut upper = expected[..index].to_vec();
                upper.push(&expected[index] + BigInt::from(1));
                upper.resize(width, BigInt::from(0));
                left >= right && left < upper
            }
            _ => return None,
        };
        if !valid {
            return Some(false);
        }
    }
    Some(true)
}
fn verify_live_runtime(
    root: &Path,
    sources: &RouteSources,
    out: &mut Vec<Issue>,
    label: &str,
    e: &Value,
    pm: &Value,
    interpreter: Option<&Path>,
    cancel: &GitState<'_>,
) -> io::Result<()> {
    let Some(interpreter) = interpreter else {
        live_python_runtime_issues(out, label);
        return Ok(());
    };
    let declared: BTreeSet<_> = array(&pm["dependencies"])
        .iter()
        .filter_map(|d| {
            d["version"].as_str()?;
            d["name"].as_str()
        })
        .collect();
    let distributions: Vec<String> = array(&e["dependencies"])
        .iter()
        .filter_map(|d| {
            let name = d["name"].as_str()?;
            (name != "python"
                && declared.contains(name)
                && d["state"] == "available"
                && d["resolved_version"]
                    .as_str()
                    .is_some_and(|s| !s.is_empty()))
            .then(|| name.to_owned())
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let (code, bytes, stderr) = executor::capture_agent_surface_runtime(
        root,
        interpreter,
        &distributions,
        sources.remaining_time()?,
        cancel.cancel,
    )?;
    cancel.charge(
        bytes
            .len()
            .checked_add(stderr.len())
            .ok_or_else(|| invalid("runtime facts output accounting overflow"))?,
    )?;
    if code != 0 {
        push(
            out,
            label,
            format!("budget receipt live Python runtime facts probe failed with exit {code}"),
        );
        return Ok(());
    }
    let facts: Value = serde_json::from_slice(&bytes)
        .map_err(|_| invalid("Python runtime facts probe returned malformed JSON"))?;
    if facts.as_object().map_or(true, |m| m.len() != 3)
        || !facts["implementation"]
            .as_str()
            .is_some_and(|s| !s.is_empty() && s.len() <= 128)
        || !facts["version"]
            .as_array()
            .is_some_and(|a| a.len() == 3 && a.iter().all(nonnegative))
        || facts["dependencies"].as_object().map_or(true, |m| {
            m.len() != distributions.len() || !distributions.iter().all(|n| m.contains_key(n))
        })
    {
        return Err(invalid(
            "Python runtime facts probe returned invalid typed facts",
        ));
    }
    let i = &e["interpreter"];
    if i.is_object() {
        if i["implementation"].is_string() && i["implementation"] != facts["implementation"] {
            push(
                out,
                label,
                "budget receipt interpreter implementation does not match the current Python runtime",
            );
        }
        let version = array(&facts["version"])
            .iter()
            .map(|v| integer(v).unwrap())
            .collect::<Vec<_>>();
        let decl = array(&pm["dependencies"])
            .iter()
            .find(|v| v["name"] == "python")
            .and_then(|v| v["version"].as_str());
        if let Some(decl) = decl {
            if satisfies(decl, &version) == Some(false) {
                push(
                    out,
                    label,
                    "budget receipt interpreter runtime does not satisfy the declared python dependency",
                );
            }
        }
        if i["implementation"].is_string() && i["version"].is_string() {
            let expected = sha256_bytes(
                format!(
                    "{RUNTIME_PREFIX}python:{}:{}",
                    string(&facts["implementation"]),
                    string(&i["version"])
                )
                .as_bytes(),
            );
            if i["artifact_digest"]!=expected&&!out.iter().any(|(l,m)|l==label&&m=="budget receipt interpreter artifact_digest does not match the captured Python runtime contract"){push(out,label,"budget receipt interpreter artifact_digest does not match the captured Python runtime contract");}
        }
    }
    let declared: BTreeSet<_> = array(&pm["dependencies"])
        .iter()
        .filter_map(|d| {
            d["version"].as_str()?;
            d["name"].as_str()
        })
        .collect();
    for d in array(&e["dependencies"]) {
        let Some(name) = d["name"].as_str().filter(|s| *s != "python") else {
            continue;
        };
        if !declared.contains(name)
            || d["state"] != "available"
            || !d["resolved_version"]
                .as_str()
                .is_some_and(|s| !s.is_empty())
        {
            continue;
        }
        let Some(fact) = facts["dependencies"].get(name) else {
            push(
                out,
                label,
                format!(
                    "budget receipt available runtime dependency {name} cannot be resolved by the bounded native runtime facts contract"
                ),
            );
            continue;
        };
        match string(&fact["state"]) {
            "installed" => {
                if !fact["version"]
                    .as_str()
                    .is_some_and(|s| !s.is_empty() && s.len() <= 4096)
                {
                    return Err(invalid(
                        "Python runtime facts probe returned invalid package version",
                    ));
                }
                if d["resolved_version"] != fact["version"] {
                    push(
                        out,
                        label,
                        format!(
                            "budget receipt runtime dependency {name} resolved_version does not match the current runtime"
                        ),
                    );
                }
            }
            "missing" => push(
                out,
                label,
                format!(
                    "budget receipt available runtime dependency {name} is not installed in the current runtime"
                ),
            ),
            "error" => push(
                out,
                label,
                format!(
                    "budget receipt available runtime dependency {name} cannot be resolved in the current runtime: {}",
                    string(&fact["error"])
                ),
            ),
            _ => {
                return Err(invalid(
                    "Python runtime facts probe returned invalid package state",
                ));
            }
        }
    }
    Ok(())
}
