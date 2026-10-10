//! ToS-owned bundle preflight and rehearsal. Artifact policy, signatures,
//! registry lifecycle and the consumer verdict remain with the selected
//! abyss-machine CLI. ToS never imports an external owner's implementation.
use crate::executor::{Limits as ProcessLimits, capture_artifact_owner};
use crate::kag_release::{
    WholeBudget, directory, invalid, read_json, regular, relative, safe_absolute, strict_json,
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicI32, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tos_foundation::Digest256Hasher;

const MANIFEST: &str =
    "mechanics/release-support/parts/artifact-bundles/manifests/generated_readmodel.bundle.json";
const SUBJECT: &str = "ToS/derived-exports/root_entry_map.min.json";
const CLASS: &str = "tree_of_sophia_generated_readmodel_bundle";
const OWNER: &str = "Tree-of-Sophia";
const CONSUMER: &str = "Tree-of-Sophia:generated-readmodel";
const MARKER: &str = ".abyss-artifact-validator-output";
const ABI: &str = "artifact.abi.json";
const MAX_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const MAX_MEMBERS: usize = 100_000;

fn text(path: &Path) -> io::Result<&str> {
    path.to_str()
        .ok_or_else(|| invalid("artifact path must be UTF-8"))
}
fn field<'a>(v: &'a Value, key: &str) -> io::Result<&'a str> {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| invalid(format!("missing artifact field {key}")))
}
fn require(ok: bool, message: &str) -> io::Result<()> {
    if ok { Ok(()) } else { Err(invalid(message)) }
}
fn ok(v: &Value) -> bool {
    v.get("ok") == Some(&Value::Bool(true))
}
fn contains(v: &Value, name: &str) -> bool {
    v.as_array()
        .is_some_and(|a| a.iter().any(|v| v.as_str() == Some(name)))
}
fn has_error(v: &Value, expected: &str) -> bool {
    v["errors"].as_array().is_some_and(|a| {
        a.iter()
            .any(|v| v.as_str().is_some_and(|s| s.contains(expected)))
    })
}

struct Options {
    root: PathBuf,
    manifest: PathBuf,
    subject: PathBuf,
    bundle: PathBuf,
    registry: PathBuf,
    store: PathBuf,
    executable: String,
    owner_root: Option<PathBuf>,
    clean: bool,
}
fn selected(root: &Path, raw: &str) -> io::Result<PathBuf> {
    require(
        !raw.trim().is_empty() && raw != ".",
        "explicit artifact path must be nonempty and must not be current directory",
    )?;
    let p = Path::new(raw);
    let p = safe_absolute(&if p.is_absolute() {
        p.to_path_buf()
    } else {
        root.join(p)
    })?;
    require(
        p != root && p.parent().is_some(),
        "artifact output must not be repository or filesystem root",
    )?;
    Ok(p)
}
impl Options {
    fn parse(args: &[String]) -> io::Result<Self> {
        let mut values = BTreeMap::new();
        let mut clean = true;
        let mut i = 0;
        while i < args.len() {
            let key = &args[i];
            if key == "--no-clean" {
                clean = false;
                i += 1;
                continue;
            }
            if key == "--json" {
                i += 1;
                continue;
            }
            require(
                matches!(
                    key.as_str(),
                    "--repo-root"
                        | "--manifest"
                        | "--subject"
                        | "--bundle-dir"
                        | "--registry-dir"
                        | "--subject-store-root"
                        | "--abyss-machine"
                        | "--abyss-machine-root"
                ),
                "unknown artifact bundle option",
            )?;
            let value = args
                .get(i + 1)
                .ok_or_else(|| invalid("artifact bundle option requires a value"))?;
            require(
                !value.trim().is_empty()
                    && !value.contains('\0')
                    && values.insert(key.as_str(), value.as_str()).is_none(),
                "empty or duplicate artifact bundle option",
            )?;
            i += 2;
        }
        let root = safe_absolute(Path::new(
            values
                .get("--repo-root")
                .ok_or_else(|| invalid("--repo-root is required"))?,
        ))?;
        directory(&root)?;
        let pick = |key, default| selected(&root, values.get(key).copied().unwrap_or(default));
        let executable = values
            .get("--abyss-machine")
            .map(|s| s.to_string())
            .or_else(|| std::env::var("TOS_ABYSS_MACHINE_EXECUTABLE").ok())
            .unwrap_or_else(|| "abyss-machine".into());
        require(
            !executable.is_empty() && !executable.contains('\0'),
            "invalid artifact owner executable",
        )?;
        Ok(Self {
            manifest: pick("--manifest", MANIFEST)?,
            subject: pick("--subject", SUBJECT)?,
            bundle: pick(
                "--bundle-dir",
                "dist/abyss-artifact-bundle/tree-of-sophia-generated-readmodel",
            )?,
            registry: pick(
                "--registry-dir",
                "dist/abyss-artifact-registry/tree-of-sophia-generated-readmodel",
            )?,
            store: pick(
                "--subject-store-root",
                "dist/abyss-artifact-subjects/tree-of-sophia-generated-readmodel",
            )?,
            owner_root: values
                .get("--abyss-machine-root")
                .map(|s| safe_absolute(Path::new(s)))
                .transpose()?,
            root,
            executable,
            clean,
        })
    }
}

struct Context<'a> {
    options: &'a Options,
    end: Instant,
    cancel: &'a AtomicI32,
}
impl Context<'_> {
    fn check(&self) -> io::Result<()> {
        require(
            self.cancel.load(Ordering::Relaxed) == 0 && Instant::now() < self.end,
            "artifact validation cancelled or whole deadline exhausted",
        )
    }
    fn call(&self, store: &Path, args: &[&str]) -> io::Result<Value> {
        self.check()?;
        let mut argv = vec![self.options.executable.clone(), "artifacts".into()];
        argv.extend(args.iter().map(|s| s.to_string()));
        argv.push("--json".into());
        let remaining = self.end.saturating_duration_since(Instant::now());
        require(
            remaining > Duration::from_secs(2),
            "artifact owner deadline exhausted",
        )?;
        let (code, stdout, stderr) = capture_artifact_owner(
            &self.options.root,
            argv,
            store,
            ProcessLimits {
                command_wall: remaining.min(Duration::from_secs(120)),
                lane_wall: remaining,
                cleanup_grace: Duration::from_secs(1),
                output_bytes: 16 * 1024 * 1024,
            },
            self.cancel,
        )?;
        self.check()?;
        require(
            code == 0 || code == 1,
            "artifact owner process did not complete a consumer decision",
        )?;
        let v = strict_json(&stdout).map_err(|e| {
            invalid(format!(
                "artifact owner JSON: {e}; {}",
                String::from_utf8_lossy(&stderr[..stderr.len().min(2048)])
            ))
        })?;
        require(v.is_object(), "artifact owner result must be an object")?;
        // Expected denials use exit 1. A successful verdict cannot hide a failed process.
        require(
            code == 0 || !ok(&v),
            "artifact owner positive result with failed exit status",
        )?;
        Ok(v)
    }
    fn scope(&self, store: &Path) -> io::Result<()> {
        let v = self.call(store, &["paths"])?;
        require(
            v["artifact_subject_store"]["search_scope"]
                == json!({
                    "schema_version":"abyss_machine_artifact_subject_store_scope_v1",
                    "mode":"isolated", "roots":[text(store)?]
                }),
            "selected artifact owner does not prove the exact isolated subject-store scope",
        )
    }
    fn registry(&self, store: &Path, registry: &Path) -> io::Result<Value> {
        self.call(
            store,
            &[
                "bundle-registry",
                "--registry-dir",
                text(registry)?,
                "--artifact-class",
                CLASS,
            ],
        )
    }
    fn promote(&self, store: &Path, registry: &Path, evidence: &str) -> io::Result<Value> {
        let o = self.options;
        let promoted = self.call(
            store,
            &[
                "evidence-promote",
                text(&o.bundle)?,
                "--registry-dir",
                text(registry)?,
                "--lifecycle-state",
                "release-ready",
                "--consumer-ref",
                CONSUMER,
                "--evidence-ref",
                evidence,
                "--source-repo",
                OWNER,
                "--source-ref",
                &portable(&o.manifest, &o.root),
                "--producer",
                "Tree-of-Sophia generated readmodel builder",
                "--trust-root-mode",
                "host_managed",
            ],
        )?;
        let latest = self.registry(store, registry)?;
        let record = &latest["latest_by_artifact_class"][CLASS];
        let valid = ok(&promoted)
            && record.is_object()
            && record["record_id"] == promoted["promotion"]["record_id"]
            && record["lifecycle_state"] == "release-ready";
        require(
            valid,
            "artifact evidence promotion did not select the exact release-ready record",
        )?;
        Ok(json!({"ok":true,"promoted":promoted,"latest":latest}))
    }
    fn gate(
        &self,
        store: &Path,
        registry: &Path,
        record: Option<&Value>,
        digest: Option<&str>,
    ) -> io::Result<Value> {
        let mut args = vec![
            "trust-gate",
            "--registry-dir",
            text(registry)?,
            "--artifact-class",
            CLASS,
            "--consumer-intent",
            "agent",
            "--source-repo",
            OWNER,
            "--trust-root-mode",
            "host_managed",
        ];
        if let Some(record) = record {
            args.extend(["--record-id", field(record, "record_id")?]);
        }
        if let Some(digest) = digest {
            require(!digest.is_empty(), "missing aggregate subject digest")?;
            args.extend(["--subject-digest", digest]);
        }
        self.call(store, &args)
    }
    fn materialize(&self, store: &Path, registry: &Path) -> io::Result<Value> {
        self.call(
            store,
            &[
                "materialize-subjects",
                text(&self.options.bundle)?,
                "--store-root",
                text(store)?,
                "--registry-dir",
                text(registry)?,
                "--manifest",
                text(&self.options.manifest)?,
                "--consumer-intent",
                "agent",
                "--source-repo",
                OWNER,
                "--trust-root-mode",
                "host_managed",
            ],
        )
    }
}
fn portable(p: &Path, root: &Path) -> String {
    p.strip_prefix(root)
        .unwrap_or_else(|_| Path::new(p.file_name().unwrap_or_default()))
        .to_string_lossy()
        .into_owned()
}
fn latest_record(roundtrip: &Value) -> &Value {
    &roundtrip["latest"]["latest_by_artifact_class"][CLASS]
}
fn common_claims(v: &Value) -> bool {
    let c = &v["inspected_claims"];
    c["registry_latest"]["selected_record_is_latest"] == true
        && c["controls"]["required_controls_missing"] == json!([])
        && c["source"]["source_repo_matched"] == true
        && c["trust_root"]["trust_root_mode_matched"] == true
}
fn allowed(v: &Value) -> bool {
    ok(v)
        && matches!(v["verdict"].as_str(), Some("allow" | "warn"))
        && v["decision"]["model"] == "fail_closed_consumer_admission"
        && v["decision"]["allow"] == true
        && common_claims(v)
        && v["inspected_claims"]["artifact_subject_store"]["ok"] == true
}
fn denied_without_store(v: &Value) -> bool {
    v["ok"] == false
        && v["verdict"] == "deny"
        && v["decision"]["allow"] == false
        && contains(
            &v["blockers"],
            "required_artifact_subject_store_not_verified",
        )
        && common_claims(v)
        && v["inspected_claims"]["artifact_subject_store"]["required"] == true
        && v["inspected_claims"]["artifact_subject_store"]["ok"] == false
}
fn manifest_contract(manifest: &Value) -> io::Result<()> {
    require(
        manifest["artifact_class"] == CLASS
            && manifest["owner_repo"] == OWNER
            && manifest["public_safe"] == true,
        "unexpected generated readmodel artifact class/owner/public boundary",
    )?;
    let commands = manifest["consumer_command"]
        .as_array()
        .ok_or_else(|| invalid("missing consumer command contract"))?
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join("\n");
    for token in [
        "artifacts build-sidecars",
        "artifacts sign",
        "artifacts verify",
        "artifacts release-check",
        "artifacts evidence-promote",
        "artifacts materialize-subjects",
        "artifacts trust-gate",
        "artifacts registry-latest",
        "--source-repo Tree-of-Sophia",
        "--trust-root-mode host_managed",
    ] {
        require(
            commands.contains(token),
            &format!("manifest consumer_command must include {token}"),
        )?;
    }
    require(
        manifest["consumer_contract"]["subject_store_required"] == true
            && manifest["consumer_contract"]["admission_gate"] == "fail_closed_consumer_admission",
        "manifest consumer store/admission contract differs",
    )
}
fn files(root: &Path, cx: &Context<'_>) -> io::Result<Vec<PathBuf>> {
    directory(root)?;
    let mut pending = vec![root.to_path_buf()];
    let mut result = Vec::new();
    let mut visits = 0;
    while let Some(p) = pending.pop() {
        for e in fs::read_dir(p)? {
            cx.check()?;
            visits += 1;
            require(visits <= MAX_MEMBERS, "artifact tree member budget")?;
            let e = e?;
            let ty = e.file_type()?;
            require(!ty.is_symlink(), "artifact tree contains symlink")?;
            if ty.is_dir() {
                pending.push(e.path());
            } else {
                regular(&e.path())?;
                result.push(e.path());
            }
        }
    }
    result.sort();
    Ok(result)
}
fn is_partition(path: &Path) -> io::Result<bool> {
    if regular(path)?.len() > 256 * 1024 {
        return Ok(false);
    }
    match read_json(path) {
        Ok(v) => Ok(v["schema_version"] == "tos_partitioned_projection_v1"),
        Err(_) => Ok(false),
    }
}
fn subject_paths(o: &Options, manifest: &Value, cx: &Context<'_>) -> io::Result<BTreeSet<PathBuf>> {
    let root_ref = manifest
        .get("subject_repo_root")
        .and_then(Value::as_str)
        .unwrap_or(".");
    let root = fs::canonicalize(
        o.manifest
            .parent()
            .ok_or_else(|| invalid("manifest parent"))?
            .join(root_ref),
    )?;
    directory(&root)?;
    let specs = manifest["artifact_subjects"]
        .as_array()
        .ok_or_else(|| invalid("missing artifact_subjects"))?;
    require(
        !specs.is_empty() && specs.len() <= MAX_MEMBERS,
        "artifact subject count",
    )?;
    let mut exact = BTreeSet::new();
    let mut all = BTreeSet::new();
    for spec in specs {
        cx.check()?;
        if let Some(path) = spec.get("path") {
            let p = safe_absolute(&root.join(relative(path)?))?;
            regular(&p)?;
            exact.insert(p.clone());
            all.insert(p);
        } else if let Some(glob) = spec.get("glob") {
            let pattern = relative(glob)?;
            // Match the owner's filesystem glob grammar through its native
            // public path matcher; partition roots always require exact entries.
            let mut matched = 0;
            for p in files(&root, cx)? {
                let rel = text(
                    p.strip_prefix(&root)
                        .map_err(|_| invalid("glob escaped root"))?,
                )?;
                if crate::documentation_cross_corpus::glob_match(pattern, rel) {
                    require(
                        !is_partition(&p)?,
                        "partitioned projection closure requires exact artifact_subjects.path entries; glob matched a root",
                    )?;
                    all.insert(p);
                    matched += 1;
                }
            }
            require(matched > 0, "artifact subject glob has no matches")?;
        } else {
            return Err(invalid("artifact subject needs path or glob"));
        }
    }
    for path in &exact {
        if is_partition(path)? {
            let closure = tos_compiler::partitioned_projection_closure(
                path,
                tos_compiler::Limits::default(),
                cx.end,
                &|| cx.cancel.load(Ordering::Relaxed) != 0,
            )
            .map_err(io::Error::other)?;
            for part in closure {
                require(
                    exact.contains(&part),
                    "partitioned projection subject closure is incomplete; every manifest and part must be an exact subject",
                )?;
            }
        }
    }
    if let Some(path) = manifest["abi_subject"].get("path") {
        all.insert(safe_absolute(&root.join(relative(path)?))?);
    }
    all.insert(o.subject.clone());
    all.insert(o.manifest.clone());
    Ok(all)
}
fn scan(path: &Path, forbidden: &[String], cx: &Context<'_>) -> io::Result<(u64, String)> {
    cx.check()?;
    let before = regular(path)?;
    require(before.len() <= MAX_BYTES, "artifact subject bytes")?;
    let mut f = fs::File::open(path)?;
    let mut bytes = vec![0u8; 65536];
    let mut retained = Vec::<u8>::new();
    let mut total = 0u64;
    let mut hash = Digest256Hasher::new();
    let tail = forbidden
        .iter()
        .map(String::len)
        .max()
        .unwrap_or(1)
        .saturating_sub(1);
    loop {
        cx.check()?;
        let n = f.read(&mut bytes)?;
        if n == 0 {
            break;
        }
        total = total
            .checked_add(n as u64)
            .filter(|n| *n <= MAX_BYTES)
            .ok_or_else(|| invalid("artifact subject byte budget"))?;
        hash.update(&bytes[..n]);
        retained.extend_from_slice(&bytes[..n]);
        require(
            !forbidden
                .iter()
                .filter(|s| !s.is_empty())
                .any(|s| retained.windows(s.len()).any(|w| w == s.as_bytes())),
            "generated readmodel subjects contain private or machine-local markers",
        )?;
        if retained.len() > tail {
            retained.drain(..retained.len() - tail);
        }
    }
    let after = regular(path)?;
    require(
        total == before.len()
            && before.len() == after.len()
            && before.modified()? == after.modified()?,
        "artifact subject changed during read",
    )?;
    Ok((total, hash.finalize().to_hex()))
}
fn census(
    paths: &BTreeSet<PathBuf>,
    cx: &Context<'_>,
    public: bool,
) -> io::Result<BTreeMap<PathBuf, (u64, String)>> {
    let mut forbidden = vec![];
    if public {
        forbidden = vec![
            text(&cx.options.root)?.into(),
            "/srv/abyss-machine".into(),
            "/var/lib/abyss-machine".into(),
            "/etc/abyss-machine".into(),
            "PASSWORD=".into(),
            "TOKEN=".into(),
            "SECRET=".into(),
        ];
        if let Ok(home) = std::env::var("HOME") {
            if !home.is_empty() {
                forbidden.push(home);
            }
        }
    }
    let mut total = 0u64;
    let mut result = BTreeMap::new();
    for p in paths {
        let row = scan(p, &forbidden, cx)?;
        total = total
            .checked_add(row.0)
            .filter(|v| *v <= MAX_BYTES)
            .ok_or_else(|| invalid("artifact subject set byte budget"))?;
        result.insert(p.clone(), row);
    }
    Ok(result)
}
fn validate_outputs(o: &Options, subjects: &BTreeSet<PathBuf>, cx: &Context<'_>) -> io::Result<()> {
    let dirs = [&o.bundle, &o.registry, &o.store];
    for (index, p) in dirs.iter().enumerate() {
        require(
            !o.root.starts_with(p) && !subjects.iter().any(|s| s.starts_with(p)),
            "artifact output overlaps source",
        )?;
        for other in dirs.iter().skip(index + 1) {
            require(
                !p.starts_with(other) && !other.starts_with(p),
                "artifact outputs overlap",
            )?;
        }
        if p.exists() {
            let _ = files(p, cx)?;
            let safe = o.root.join(match index {
                0 => "dist/abyss-artifact-bundle",
                1 => "dist/abyss-artifact-registry",
                _ => "dist/abyss-artifact-subjects",
            });
            require(
                !o.clean
                    || (p.starts_with(&safe) && **p != safe)
                    || regular(&p.join(MARKER)).is_ok(),
                "refusing to clean output outside generated root without validator marker; use --no-clean",
            )?;
        }
    }
    Ok(())
}
fn prepare_outputs(o: &Options) -> io::Result<()> {
    for p in [&o.bundle, &o.registry, &o.store] {
        if o.clean && p.exists() {
            fs::remove_dir_all(p)?;
        }
        fs::create_dir_all(p)?;
        fs::write(
            p.join(MARKER),
            b"ToS generated artifact validation output\n",
        )?;
    }
    Ok(())
}
fn write_json(path: &Path, v: &Value) -> io::Result<()> {
    let mut f = fs::File::create(path)?;
    serde_json::to_writer_pretty(&mut f, v).map_err(io::Error::other)?;
    f.write_all(b"\n")
}
fn copy_bundle(from: &Path, to: &Path, cx: &Context<'_>) -> io::Result<()> {
    require(!to.exists(), "adversarial bundle output already exists")?;
    fs::create_dir_all(to)?;
    let mut total = 0u64;
    for p in files(from, cx)? {
        total = total
            .checked_add(regular(&p)?.len())
            .filter(|n| *n <= 64 * 1024 * 1024)
            .ok_or_else(|| invalid("bundle rehearsal copy budget"))?;
        let target = to.join(
            p.strip_prefix(from)
                .map_err(|_| invalid("bundle member escaped"))?,
        );
        fs::create_dir_all(target.parent().unwrap())?;
        fs::copy(p, target)?;
    }
    Ok(())
}
fn sanitize(v: &mut Value, o: &Options) {
    match v {
        Value::Array(rows) => {
            for row in rows {
                sanitize(row, o);
            }
        }
        Value::Object(rows) => {
            for row in rows.values_mut() {
                sanitize(row, o);
            }
        }
        Value::String(s) => {
            let replace = |root: &Path, label: &str| -> Option<String> {
                Path::new(s).strip_prefix(root).ok().map(|rel| {
                    if rel.as_os_str().is_empty() {
                        label.to_owned()
                    } else if label.is_empty() {
                        rel.to_string_lossy().into_owned()
                    } else {
                        format!("{label}/{}", rel.display())
                    }
                })
            };
            let mapped = replace(&o.root, "")
                .or_else(|| {
                    o.owner_root
                        .as_ref()
                        .and_then(|root| replace(root, "repo:abyss-machine"))
                })
                .or_else(|| {
                    std::env::var("ABYSS_MACHINE_TMP_ROOT")
                        .ok()
                        .and_then(|root| replace(Path::new(&root), "host-tmp:abyss-machine"))
                })
                .or_else(|| {
                    replace(
                        Path::new("/srv/abyss-machine/tmp"),
                        "host-tmp:abyss-machine",
                    )
                })
                .or_else(|| {
                    std::env::var("HOME")
                        .ok()
                        .filter(|home| !home.is_empty() && Path::new(s).starts_with(home))
                        .map(|_| "host-home-redacted".into())
                });
            if let Some(mapped) = mapped {
                *s = mapped;
            }
        }
        _ => (),
    }
}
fn sanitize_tree(root: &Path, cx: &Context<'_>) -> io::Result<()> {
    for p in files(root, cx)? {
        if p.extension().is_some_and(|e| e == "json") {
            let mut value = read_json(&p)?;
            sanitize(&mut value, cx.options);
            write_json(&p, &value)?;
        } else if p.extension().is_some_and(|e| e == "jsonl") {
            require(
                regular(&p)?.len() <= 16 * 1024 * 1024,
                "artifact JSONL bytes",
            )?;
            let raw = fs::read_to_string(&p)?;
            let mut out = String::new();
            for line in raw.lines() {
                cx.check()?;
                let mut v = strict_json(line.as_bytes())?;
                sanitize(&mut v, cx.options);
                out.push_str(&serde_json::to_string(&v).map_err(io::Error::other)?);
                out.push('\n');
            }
            fs::write(&p, out)?;
        }
    }
    let mut forbidden = vec![
        text(&cx.options.root)?.into(),
        "/srv/abyss-machine/tmp".into(),
    ];
    if let Some(root) = &cx.options.owner_root {
        forbidden.push(text(root)?.into());
    }
    if let Ok(home) = std::env::var("HOME") {
        if !home.is_empty() {
            forbidden.push(home);
        }
    }
    for p in files(root, cx)? {
        if p.extension().is_some_and(|e| e == "json" || e == "jsonl") {
            scan(&p, &forbidden, cx)?;
        }
    }
    Ok(())
}
struct Rehearsal(PathBuf);
impl Drop for Rehearsal {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn rehearsal(o: &Options) -> io::Result<Rehearsal> {
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    let p = o
        .bundle
        .parent()
        .ok_or_else(|| invalid("bundle parent"))?
        .join(format!(".artifact-rehearsal-{}-{n}", std::process::id()));
    fs::create_dir(&p)?;
    Ok(Rehearsal(p))
}
fn materialized_roundtrip(cx: &Context<'_>, store: &Path, registry: &Path) -> io::Result<Value> {
    cx.scope(store)?;
    let pre = cx.promote(store, registry, "materialized-subject-store-precondition")?;
    let materialized = cx.materialize(store, registry)?;
    require(ok(&materialized), "artifact subject materialization failed")?;
    let refreshed = cx.promote(store, registry, "materialized-subject-store-rehearsal")?;
    let gate = cx.gate(
        store,
        registry,
        None,
        Some(field(&materialized, "aggregate_digest")?),
    )?;
    require(
        allowed(&gate) && latest_record(&refreshed)["artifact_subject_store"]["ok"] == true,
        "isolated materialized subject store not admitted",
    )?;
    Ok(
        json!({"ok":true,"pre_registry":pre,"materialized":materialized,"refreshed_registry":refreshed,"trust_gate":gate}),
    )
}
fn adversarial(cx: &Context<'_>, temp: &Path) -> io::Result<Value> {
    let o = cx.options;
    let mut checks = serde_json::Map::new();
    for (name, wrong) in [("missing_abi", false), ("wrong_external_subject", true)] {
        let target = temp.join(name);
        copy_bundle(&o.bundle, &target, cx)?;
        if wrong {
            let mut v = read_json(&target.join(ABI))?;
            require(
                v["external_subject"].is_object(),
                "ABI has no external_subject",
            )?;
            v["external_subject"]["sha256"] = Value::String(format!("sha256:{}", "0".repeat(64)));
            write_json(&target.join(ABI), &v)?;
        } else {
            fs::remove_file(target.join(ABI))?;
        }
        let verification = cx.call(&o.store, &["verify", text(&target)?])?;
        let valid = verification["ok"] == false
            && if wrong {
                has_error(
                    &verification,
                    "subject digest does not match ABI external_subject sha256",
                )
            } else {
                contains(&verification["missing"], ABI)
            };
        require(
            valid,
            "corrupted ABI rehearsal was not refused for the expected cause",
        )?;
        checks.insert(name.into(), json!({"ok":true,"verification":verification}));
    }
    let private = temp.join("private-readmodel.json");
    require(
        regular(&o.subject)?.len() < MAX_BYTES,
        "private marker rehearsal size",
    )?;
    fs::copy(&o.subject, &private)?;
    fs::OpenOptions::new()
        .append(true)
        .open(&private)?
        .write_all(b"\nTOKEN=private-negative\n")?;
    let private_result = scan(&private, &["TOKEN=".into()], cx);
    require(
        private_result
            .as_ref()
            .is_err_and(|e| e.to_string().contains("private or machine-local markers")),
        "private marker was not rejected",
    )?;
    checks.insert("private_readmodel_marker".into(), json!({"ok":true,"error":"generated readmodel subjects contain private or machine-local markers"}));
    let target = temp.join("unverified-latest");
    copy_bundle(&o.bundle, &target, cx)?;
    fs::remove_file(target.join(ABI))?;
    let registry = temp.join("unverified-registry");
    let registered = cx.call(
        &o.store,
        &[
            "bundle-register",
            text(&target)?,
            "--registry-dir",
            text(&registry)?,
            "--lifecycle-state",
            "release-ready",
        ],
    )?;
    require(
        registered["ok"] == false && has_error(&registered, "successful bundle verification"),
        "unverified latest promotion was not rejected",
    )?;
    checks.insert(
        "unverified_latest_rejected".into(),
        json!({"ok":true,"registered":registered}),
    );
    let registry = temp.join("terminal-registry");
    let ready = cx.promote(&o.store, &registry, "terminal-state-rehearsal")?;
    let revoked = cx.call(
        &o.store,
        &[
            "bundle-register",
            text(&o.bundle)?,
            "--registry-dir",
            text(&registry)?,
            "--lifecycle-state",
            "revoked",
            "--revocation-reason",
            "Tree-of-Sophia generated readmodel terminal-state rehearsal",
            "--source-repo",
            OWNER,
            "--source-ref",
            &portable(&o.manifest, &o.root),
            "--producer",
            "Tree-of-Sophia generated readmodel builder",
            "--trust-root-mode",
            "host_managed",
        ],
    )?;
    require(
        ok(&revoked),
        "terminal registry rehearsal did not record revocation",
    )?;
    let record = if revoked["record"].is_object() {
        &revoked["record"]
    } else {
        &revoked["promotion"]
    };
    let gate = cx.gate(&o.store, &registry, Some(record), None)?;
    let after = cx.registry(&o.store, &registry)?;
    require(
        gate["verdict"] == "deny"
            && gate["decision"]["allow"] == false
            && gate["inspected_claims"]["lifecycle"]["terminal_state"] == true
            && after["latest_by_artifact_class"]
                .as_object()
                .is_some_and(|m| m.is_empty()),
        "revoked artifact still eligible for consumer/latest",
    )?;
    checks.insert("terminal_registry_state".into(), json!({"ok":true,"release_ready":ready,"revoked":revoked,"revoked_trust_gate":gate,"after_revoke":after}));
    checks.insert(
        "materialized_subject_store".into(),
        materialized_roundtrip(
            cx,
            &temp.join("isolated-store"),
            &temp.join("isolated-registry"),
        )?,
    );
    Ok(json!({"ok":true,"checks":checks}))
}
fn validate(o: &Options, cx: &Context<'_>) -> io::Result<Value> {
    let manifest = read_json(&o.manifest)?;
    manifest_contract(&manifest)?;
    let subjects = subject_paths(o, &manifest, cx)?;
    // The manifest describes local paths; public marker checks apply to content.
    let contents = subjects
        .iter()
        .filter(|p| **p != o.manifest)
        .cloned()
        .collect();
    census(&contents, cx, true)?;
    let before = census(&subjects, cx, false)?;
    validate_outputs(o, &subjects, cx)?;
    cx.scope(&o.store)?; // Fail before cleaning or producing any owner output.
    prepare_outputs(o)?;
    let temp = rehearsal(o)?;
    let build = cx.call(
        &o.store,
        &[
            "build-sidecars",
            "--bundle-dir",
            text(&o.bundle)?,
            "--manifest",
            text(&o.manifest)?,
        ],
    )?;
    require(ok(&build), "artifact sidecar build failed")?;
    let sign = cx.call(&o.store, &["sign", text(&o.bundle)?])?;
    require(ok(&sign), "artifact signature failed")?;
    let verify = cx.call(&o.store, &["verify", text(&o.bundle)?])?;
    require(ok(&verify), "artifact verification failed")?;
    let release = cx.call(&o.store, &["release-check", text(&o.bundle)?])?;
    require(ok(&release), "artifact release check failed")?;
    let identity = read_json(&o.bundle.join("artifact.identity.json"))?;
    require(
        verify["required_controls"] == json!(["abi_signature"])
            && verify["verified_controls"] == json!(["abi_signature"]),
        "unexpected artifact controls",
    )?;
    for name in ["sbom", "slsa_in_toto", "sigstore_cosign", "c2pa"] {
        let d = &identity["deferred_controls"][name];
        require(
            d["required"] == false && d["reason"].as_str().is_some_and(|s| !s.is_empty()),
            "missing explicit artifact control deferral",
        )?;
    }
    let mut roots = vec![text(&o.root)?.into()];
    if let Some(root) = &o.owner_root {
        roots.push(text(root)?.into());
    }
    for p in files(&o.bundle, cx)? {
        if p.extension().is_some_and(|e| e == "json" || e == "jsonl") {
            scan(&p, &roots, cx)?;
        }
    }
    let registry = cx.promote(
        &o.store,
        &o.registry,
        &format!("{}/artifact.verify.json", portable(&o.bundle, &o.root)),
    )?;
    let empty_store = temp.0.join("empty-store");
    let empty_registry = temp.0.join("precondition-registry");
    cx.scope(&empty_store)?;
    let pre_registry = cx.promote(
        &empty_store,
        &empty_registry,
        "materialized-subject-store-negative-precondition",
    )?;
    let pre_gate = cx.gate(
        &empty_store,
        &empty_registry,
        Some(latest_record(&pre_registry)),
        None,
    )?;
    require(
        denied_without_store(&pre_gate),
        "consumer did not deny the isolated missing subject store",
    )?;
    let materialized = cx.materialize(&o.store, &o.registry)?;
    require(ok(&materialized), "subject store materialization failed")?;
    let refreshed = cx.promote(&o.store, &o.registry, "materialized-subject-store")?;
    let gate = cx.gate(&o.store, &o.registry, Some(latest_record(&refreshed)), None)?;
    require(allowed(&gate), "materialized artifact not admitted")?;
    let digest_gate = cx.gate(
        &o.store,
        &o.registry,
        None,
        Some(field(&materialized, "aggregate_digest")?),
    )?;
    require(
        allowed(&digest_gate),
        "aggregate digest artifact not admitted",
    )?;
    let adversarial = adversarial(cx, &temp.0)?;
    sanitize_tree(&o.registry, cx)?;
    sanitize_tree(&o.store, cx)?;
    let latest = cx.registry(&o.store, &o.registry)?;
    require(
        allowed(&cx.gate(
            &o.store,
            &o.registry,
            None,
            Some(field(&materialized, "aggregate_digest")?),
        )?),
        "portable registry/store failed consumer admission after sanitization",
    )?;
    require(
        census(&subjects, cx, false)? == before,
        "source subjects changed during artifact owner validation",
    )?;
    let mut result = json!({"schema":"tos_abyss_machine_generated_readmodel_artifact_bundle_validation_v1","ok":true,
        "manifest_ref":portable(&o.manifest,&o.root),"subject_ref":portable(&o.subject,&o.root),"bundle_dir":portable(&o.bundle,&o.root),"registry_dir":portable(&o.registry,&o.root),"subject_store_root":portable(&o.store,&o.root),"artifact_class":CLASS,
        "required_controls":verify["required_controls"],"verified_controls":verify["verified_controls"],"deferred_controls":identity["deferred_controls"],"registry":latest,
        "pre_materialization_gate":{"ok":true,"trust_gate":pre_gate},"materialized_subject_store":materialized,"trust_gate":{"ok":true,"trust_gate":gate},"subject_store_gate":digest_gate,"adversarial_checks":adversarial,
        "steps":{"build_sidecars":build,"sign":sign,"verify":verify,"release_check":release,"initial_registry":registry},"source_subjects_unchanged":true});
    sanitize(&mut result, o);
    Ok(result)
}

pub fn run(args: &[String], started: Instant, cancel: &AtomicI32) -> io::Result<Value> {
    let budget = WholeBudget::begin()?;
    let options = Options::parse(args)?;
    let end = budget.deadline()?.min(started + Duration::from_secs(600));
    let cx = Context {
        options: &options,
        end,
        cancel,
    };
    cx.check()?;
    let runtime = read_json(
        &options
            .root
            .join("access/contracts/runtime-manifest.v1.json"),
    )?;
    let posture = &runtime["integration_posture"];
    if posture["state"] == "paused"
        && posture["external_activation"] == "disabled"
        && contains(&posture["scope"], "abyssos")
    {
        return Ok(
            json!({"schema_version":"tos_abyssos_admission_pause_v1","ok":true,"status":"paused","external_activation":"disabled","reason":"ToS integration freeze; AbyssOS admission is deferred until an explicit ToS operator command."}),
        );
    }
    validate(&options, &cx)
}
pub fn cli(args: &[String], started: Instant, cancel: &AtomicI32) -> i32 {
    if args.iter().any(|s| s == "--help" || s == "-h") {
        println!(
            "tos-ops-mechanics-plan --artifact-bundle --repo-root ROOT [--manifest FILE] [--subject FILE] [--bundle-dir DIR] [--registry-dir DIR] [--subject-store-root DIR] [--abyss-machine EXECUTABLE] [--abyss-machine-root ROOT] [--no-clean] [--json]"
        );
        return 0;
    }
    match run(args, started, cancel) {
        Ok(value) => {
            if args.iter().any(|s| s == "--json") {
                println!("{value}");
            } else if value["status"] == "paused" {
                println!(
                    "[paused] ToS AbyssOS admission is frozen; external artifact validation deferred"
                );
            } else {
                println!(
                    "[ok] generated readmodel artifact bundle verified with isolated subject store"
                );
            }
            0
        }
        Err(error) => {
            eprintln!("artifact bundle refused: {error}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tos_foundation::Digest256;
    fn fixture() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "tos-artifact-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&p).unwrap();
        p
    }
    fn options(root: &Path) -> Options {
        Options::parse(&["--repo-root".into(), text(root).unwrap().into()]).unwrap()
    }
    #[test]
    fn artifact_output_paths_reject_empty_current_traversal_and_source_overlap() {
        let root = fixture();
        let cancel = AtomicI32::new(0);
        for raw in ["", " ", ".", "..", "x/../y", "/"] {
            assert!(selected(&root, raw).is_err(), "{raw:?}");
        }
        assert!(selected(&root, text(&root).unwrap()).is_err());
        let o = options(&root);
        let cx = Context {
            options: &o,
            end: Instant::now() + Duration::from_secs(10),
            cancel: &cancel,
        };
        let subjects = BTreeSet::from([o.bundle.join("authored.json")]);
        assert!(validate_outputs(&o, &subjects, &cx).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn paused_artifact_route_never_invokes_owner_and_malformed_runtime_refuses() {
        let root = fixture();
        fs::create_dir_all(root.join("access/contracts")).unwrap();
        let runtime = root.join("access/contracts/runtime-manifest.v1.json");
        write_json(&runtime,&json!({"integration_posture":{"state":"paused","external_activation":"disabled","scope":["abyssos"]}})).unwrap();
        let args = vec![
            "--repo-root".into(),
            text(&root).unwrap().into(),
            "--abyss-machine".into(),
            "/missing/must-not-execute".into(),
        ];
        let cancel = AtomicI32::new(0);
        assert_eq!(
            run(&args, Instant::now(), &cancel).unwrap()["status"],
            "paused"
        );
        assert!(!root.join("dist").exists());
        fs::write(runtime, b"not json").unwrap();
        assert!(run(&args, Instant::now(), &cancel).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn public_marker_scan_crosses_chunk_boundary_and_cleanup_preserves_unmarked_output() {
        let root = fixture();
        let mut o = options(&root);
        o.bundle = root.join("external-output");
        fs::create_dir(&o.bundle).unwrap();
        fs::write(o.bundle.join("retain"), b"evidence").unwrap();
        let cancel = AtomicI32::new(0);
        let cx = Context {
            options: &o,
            end: Instant::now() + Duration::from_secs(10),
            cancel: &cancel,
        };
        assert!(validate_outputs(&o, &BTreeSet::new(), &cx).is_err());
        assert_eq!(fs::read(o.bundle.join("retain")).unwrap(), b"evidence");
        let path = root.join("subject.json");
        let mut raw = vec![b'x'; 65533];
        raw.extend_from_slice(b"TOKEN=private");
        fs::write(&path, raw).unwrap();
        assert!(
            scan(&path, &["TOKEN=".into()], &cx)
                .unwrap_err()
                .to_string()
                .contains("private or machine-local")
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn partitioned_bundle_requires_every_exact_member_and_rejects_tampering_and_glob_roots() {
        let root = fixture();
        let mut o = options(&root);
        o.manifest = root.join("bundle.json");
        o.subject = root.join("projection.min.json");
        let cancel = AtomicI32::new(0);
        let cx = Context {
            options: &o,
            end: Instant::now() + Duration::from_secs(10),
            cancel: &cancel,
        };
        // Fixed gzip bytes from the legacy v1 JSONL carrier, independent of the native reader.
        let raw = br#"{"key":"one","value":{"id":"one","label":"One"}}
"#;
        let wire: &[u8] = &[
            31, 139, 8, 0, 0, 0, 0, 0, 2, 255, 171, 86, 202, 78, 173, 84, 178, 82, 202, 207, 75,
            85, 210, 81, 42, 75, 204, 41, 77, 85, 178, 170, 86, 202, 76, 129, 139, 229, 36, 38,
            165, 230, 0, 121, 254, 64, 94, 109, 45, 23, 0, 238, 127, 12, 115, 49, 0, 0, 0,
        ];
        let sha = Digest256::of_bytes(wire).to_hex();
        let relative = format!("projection.min.parts/{}/{sha}.jsonl.gz", &sha[..2]);
        let part = root.join(&relative);
        fs::create_dir_all(part.parent().unwrap()).unwrap();
        fs::write(&part, wire).unwrap();
        let projection = json!({"schema_version":"tos_partitioned_projection_v1","logical_schema":"synthetic_projection_v1","header":{"schema_version":"synthetic_projection_v1"},"limits":{"root_bytes":262144,"index_bytes":131072,"part_bytes":8388608,"key_bytes":4096},"collections":{"rows":{"key_field":"id","order_fields":[],"root":{"kind":"data","prefix":"","path":relative,"sha256":sha,"size_bytes":wire.len(),"decoded_bytes":raw.len(),"decoded_sha256":Digest256::of_bytes(raw).to_hex(),"count":1}}}});
        write_json(&o.subject, &projection).unwrap();
        let mut manifest =
            json!({"subject_repo_root":".","artifact_subjects":[{"path":"projection.min.json"}]});
        write_json(&o.manifest, &manifest).unwrap();
        assert!(
            subject_paths(&o, &manifest, &cx)
                .unwrap_err()
                .to_string()
                .contains("closure is incomplete")
        );
        manifest["artifact_subjects"]
            .as_array_mut()
            .unwrap()
            .push(json!({"path":relative}));
        assert!(subject_paths(&o, &manifest, &cx).unwrap().contains(&part));
        fs::write(&part, b"tampered").unwrap();
        assert!(subject_paths(&o, &manifest, &cx).is_err());
        fs::write(&part, wire).unwrap();
        manifest["artifact_subjects"] = json!([{"glob":"projection.min.json"}]);
        assert!(
            subject_paths(&o, &manifest, &cx)
                .unwrap_err()
                .to_string()
                .contains("requires exact")
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn filesystem_globs_keep_recursive_classes_unicode_and_literal_brackets() {
        use crate::documentation_cross_corpus::glob_match as matches;
        for (glob, path, expect) in [
            ("**/[a-c]?.json", "nested/bя.json", true),
            ("**/[!x].json", "y.json", true),
            ("**/[!x].json", "x.json", false),
            ("[open", "[open", true),
            ("dir/*.json", "dir/sub/a.json", false),
            ("dir/**/*.json", "dir/a.json", true),
        ] {
            assert_eq!(matches(glob, path), expect, "{glob} {path}");
        }
    }
}
