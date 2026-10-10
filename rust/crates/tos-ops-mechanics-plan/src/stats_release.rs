//! Local stats-port transport. The selected aoa-stats owner validates meaning;
//! ToS owns exact staging, immutable publication and downstream status only.
use crate::kag_release::{
    self as shared, WholeBudget, budget_check, canonical, digest, directory, hex, invalid, keys,
    read_json, regular, safe_absolute, tree,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::AtomicI32,
    time::Instant,
};
use tos_foundation::Digest256;

pub const SOURCE_SCHEMA: &str = "tos_stats_port_export_v1";
pub const SCHEMA: &str = "tos_stats_integration_v1";
pub const SOURCE_KIND: &str = "stats_port_export";
pub const SOURCE_PATHS: [&str; 5] = [
    "stats/AGENTS.md",
    "stats/README.md",
    "stats/VALIDATION.md",
    "stats/packets/table-i-prepared-dossier-route-ratio.reference.json",
    "stats/port.manifest.json",
];
const PORT: &str = "stats/port.manifest.json";
const PACKET: &str = "stats/packets/table-i-prepared-dossier-route-ratio.reference.json";
const MAX_SOURCE: u64 = 1024 * 1024;
fn hash_value(value: &Value) -> io::Result<String> {
    Ok(Digest256::of_bytes(&canonical(value)?).to_hex())
}
fn nonempty(value: &Value) -> io::Result<&str> {
    value
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| invalid("stats observation text absent"))
}
fn records(root: &Path) -> io::Result<Value> {
    directory(root)?;
    let mut total = 0u64;
    let mut records = Vec::new();
    for name in SOURCE_PATHS {
        budget_check()?;
        let path = root.join(name);
        let bytes = regular(&path)?.len();
        total = total
            .checked_add(bytes)
            .filter(|v| *v <= MAX_SOURCE)
            .ok_or_else(|| invalid("stats source exceeds 1 MiB"))?;
        records.push(json!({"path":name,"sha256":digest(&path)?,"size_bytes":bytes}));
    }
    Ok(Value::Array(records))
}
fn revision(records: &Value) -> io::Result<String> {
    hash_value(&json!({"schema_version":SOURCE_SCHEMA,"files":records}))
}
fn observation(packet: &Value) -> io::Result<Value> {
    for value in [
        &packet["observation_id"],
        &packet["observed_at"],
        &packet["provenance"]["source_revision"],
    ] {
        nonempty(value)?;
    }
    let live = packet
        .get("posture")
        .and_then(|p| p.get("live_state"))
        .ok_or_else(|| invalid("stats packet lacks explicit live_state"))?;
    Ok(
        json!({"observation_id":packet["observation_id"],"observed_at":packet["observed_at"],
        "source_revision":packet["provenance"]["source_revision"],"live_state":live}),
    )
}
fn posture(port: &Value) -> io::Result<Value> {
    port.get("evidence_posture")
        .filter(|v| v.is_object())
        .cloned()
        .ok_or_else(|| invalid("stats port lacks evidence_posture object"))
}
fn expected_files(manifest: bool) -> Vec<String> {
    let mut files: Vec<_> = SOURCE_PATHS
        .iter()
        .map(|p| format!("Tree-of-Sophia/{p}"))
        .collect();
    if manifest {
        files.push("integration.json".into());
    }
    files.sort();
    files
}
fn verify_members(root: &Path, manifest: bool) -> io::Result<()> {
    let (files, _) = tree(root)?;
    if files.into_iter().map(|(name, _)| name).collect::<Vec<_>>() != expected_files(manifest) {
        return Err(invalid("stats stage has undeclared or missing files"));
    }
    Ok(())
}
fn read_bounded(path: &Path, max: u64) -> io::Result<Vec<u8>> {
    if regular(path)?.len() > max {
        return Err(invalid("stats dependency byte bound"));
    }
    let mut raw = Vec::new();
    File::open(path)?.take(max + 1).read_to_end(&mut raw)?;
    if raw.len() as u64 > max {
        return Err(invalid("stats dependency grew past byte bound"));
    }
    Ok(raw)
}

// Decode the bounded repr emitted by CPython's own `-m site` CLI. No ToS
// Python program executes to discover the explicitly selected owner's runtime.
fn python_path_repr(raw: &str) -> io::Result<String> {
    let quote = raw
        .chars()
        .next()
        .filter(|c| matches!(c, '\'' | '"'))
        .ok_or_else(|| invalid("selected owner site path quoting"))?;
    if !raw.ends_with(quote) || raw.len() < 2 {
        return Err(invalid("selected owner site path end"));
    }
    let mut chars = raw[1..raw.len() - 1].chars();
    let mut result = String::new();
    while let Some(c) = chars.next() {
        if c != '\\' {
            result.push(c);
            continue;
        }
        let escaped = chars
            .next()
            .ok_or_else(|| invalid("selected owner path escape"))?;
        match escaped {
            '\\' | '\'' | '"' => result.push(escaped),
            'n' => result.push('\n'),
            'r' => result.push('\r'),
            't' => result.push('\t'),
            'a' => result.push('\u{7}'),
            'b' => result.push('\u{8}'),
            'f' => result.push('\u{c}'),
            'v' => result.push('\u{b}'),
            'x' | 'u' | 'U' => {
                let count = match escaped {
                    'x' => 2,
                    'u' => 4,
                    _ => 8,
                };
                let mut value = 0u32;
                for _ in 0..count {
                    value = value
                        .checked_mul(16)
                        .and_then(|v| {
                            chars
                                .next()
                                .and_then(|c| c.to_digit(16))
                                .and_then(|n| v.checked_add(n))
                        })
                        .ok_or_else(|| invalid("selected owner path codepoint"))?;
                }
                result.push(
                    char::from_u32(value).ok_or_else(|| invalid("selected owner path Unicode"))?,
                );
            }
            _ => return Err(invalid("unsupported selected owner path escape")),
        }
    }
    Ok(result)
}
fn runtime_identity(
    python: &Path,
    cwd: &Path,
    deadline: Instant,
    cancel: &AtomicI32,
) -> io::Result<Value> {
    regular(python)?;
    let run = |args: &[&str]| {
        shared::run_owner(
            cwd,
            std::iter::once(python.to_string_lossy().into_owned())
                .chain(args.iter().map(|s| (*s).to_owned()))
                .collect(),
            deadline,
            cancel,
        )
    };
    let version = run(&["--version"])?;
    let version = std::str::from_utf8(&version)
        .map_err(|_| invalid("selected Python version UTF8"))?
        .trim()
        .strip_prefix("Python ")
        .ok_or_else(|| invalid("selected Python version"))?;
    let parts = version
        .split('.')
        .take(3)
        .map(|part| {
            let digits: String = part.chars().take_while(char::is_ascii_digit).collect();
            digits
                .parse::<u32>()
                .map_err(|_| invalid("selected Python version component"))
        })
        .collect::<io::Result<Vec<_>>>()?;
    if parts.len() != 3 {
        return Err(invalid("selected Python version components"));
    }
    let site = run(&["-I", "-B", "-m", "site"])?;
    if site.len() > 65536 {
        return Err(invalid("selected Python site byte cap"));
    }
    let site = std::str::from_utf8(&site).map_err(|_| invalid("selected Python site UTF8"))?;
    let mut paths = Vec::new();
    let mut in_paths = false;
    for line in site.lines() {
        if line == "sys.path = [" {
            in_paths = true;
            continue;
        }
        if in_paths && line == "]" {
            in_paths = false;
            break;
        }
        if in_paths {
            if paths.len() >= 128 {
                return Err(invalid("selected Python site path cap"));
            }
            paths.push(PathBuf::from(python_path_repr(
                line.trim()
                    .strip_suffix(',')
                    .ok_or_else(|| invalid("selected Python site row"))?,
            )?));
        }
    }
    if in_paths || paths.is_empty() {
        return Err(invalid("selected Python site paths absent"));
    }
    let mut packages = BTreeMap::new();
    for package in ["jsonschema", "referencing"] {
        'search: for path in &paths {
            if !path.is_dir() {
                continue;
            }
            directory(path)?;
            let mut entries = 0;
            for entry in fs::read_dir(path)? {
                budget_check()?;
                entries += 1;
                if entries > 16384 {
                    return Err(invalid("selected Python distribution scan cap"));
                }
                let entry = entry?;
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if !name.starts_with(&format!("{package}-")) || !name.ends_with(".dist-info") {
                    continue;
                }
                let raw = read_bounded(&entry.path().join("METADATA"), 1024 * 1024)?;
                let metadata = std::str::from_utf8(&raw)
                    .map_err(|_| invalid("selected package metadata UTF8"))?;
                let field = |key: &str| {
                    metadata
                        .lines()
                        .take_while(|line| !line.is_empty())
                        .find_map(|line| line.strip_prefix(key))
                };
                if field("Name: ") != Some(package) {
                    return Err(invalid("selected package name differs"));
                }
                let version = field("Version: ")
                    .filter(|v| !v.is_empty())
                    .ok_or_else(|| invalid("selected package version absent"))?;
                packages.insert(package, version.to_owned());
                break 'search;
            }
        }
        if !packages.contains_key(package) {
            return Err(invalid(format!(
                "selected runtime lacks {package} metadata"
            )));
        }
    }
    Ok(json!({"python":parts,"packages":packages}))
}
fn validator_identity(
    owner: &Path,
    python: &Path,
    deadline: Instant,
    cancel: &AtomicI32,
) -> io::Result<String> {
    let mut files = BTreeMap::new();
    let mut total = 0u64;
    let mut add = |path: &Path| -> io::Result<()> {
        budget_check()?;
        total = total
            .checked_add(regular(path)?.len())
            .filter(|v| *v <= 8 * 1024 * 1024)
            .ok_or_else(|| invalid("stats owner identity exceeds 8 MiB"))?;
        if files.len() >= 1024 {
            return Err(invalid("stats owner identity member cap"));
        }
        files.insert(
            path.strip_prefix(owner)
                .map_err(|_| invalid("stats owner identity path"))?
                .to_str()
                .ok_or_else(|| invalid("stats owner path UTF8"))?
                .to_owned(),
            digest(path)?,
        );
        Ok(())
    };
    add(&owner.join("scripts/validate_stats_protocol.py"))?;
    for (relative, suffix) in [("src/aoa_stats_builder", ".py"), ("stats", ".schema.json")] {
        let root = owner.join(relative);
        if root.exists() {
            for (name, path) in tree(&root)?.0 {
                if name.ends_with(suffix) {
                    add(&path)?;
                }
            }
        }
    }
    let inventory = owner.join("stats/federation/owner-inventory.json");
    if inventory.exists() {
        add(&inventory)?;
    }
    let mut identity = runtime_identity(python, owner, deadline, cancel)?;
    identity["files"] = serde_json::to_value(files).map_err(|e| invalid(e.to_string()))?;
    hash_value(&identity)
}

pub fn validate_port(
    owner: &Path,
    port: &Path,
    python: &Path,
    cancel: &AtomicI32,
) -> io::Result<(i32, Vec<u8>, Vec<u8>)> {
    let budget = WholeBudget::begin()?;
    let owner = safe_absolute(owner)?;
    directory(&owner)?;
    let port = safe_absolute(port)?;
    regular(&port)?;
    let python = safe_absolute(python)?;
    regular(&python)?;
    let validator = owner.join("scripts/validate_stats_protocol.py");
    regular(&validator)?;
    let cwd = port
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| invalid("stats port directory"))?;
    crate::executor::capture_kag_owner(
        cwd,
        vec![
            python.to_string_lossy().into_owned(),
            "-I".into(),
            "-B".into(),
            validator.to_string_lossy().into_owned(),
            "--port".into(),
            port.to_string_lossy().into_owned(),
        ],
        crate::executor::Limits {
            command_wall: budget.deadline()?.saturating_duration_since(Instant::now()),
            lane_wall: budget.deadline()?.saturating_duration_since(Instant::now()),
            cleanup_grace: std::time::Duration::from_secs(1),
            output_bytes: shared::MAX_MANIFEST_BYTES,
        },
        cancel,
    )
}

pub fn verify_integration(root: &Path) -> io::Result<Value> {
    let _budget = WholeBudget::begin()?;
    let root = safe_absolute(root)?;
    directory(&root)?;
    let manifest = read_json(&root.join("integration.json"))?;
    keys(
        &manifest,
        &[
            "schema_version",
            "source_kind",
            "source_revision",
            "evidence_posture",
            "observation",
            "files",
            "validator_sha256",
            "integration_revision",
        ],
    )?;
    if manifest["schema_version"] != SCHEMA || manifest["source_kind"] != SOURCE_KIND {
        return Err(invalid("unsupported stats integration"));
    }
    hex(&manifest["source_revision"])?;
    hex(&manifest["validator_sha256"])?;
    let integration = hex(&manifest["integration_revision"])?;
    if root.file_name().and_then(|n| n.to_str()) != Some(integration) {
        return Err(invalid("stats integration directory identity"));
    }
    let mut body = manifest.clone();
    body.as_object_mut().unwrap().remove("integration_revision");
    if hash_value(&body)? != integration {
        return Err(invalid("stats integration manifest identity"));
    }
    let observed = records(&root.join("Tree-of-Sophia"))?;
    if manifest["files"] != observed || manifest["source_revision"] != revision(&observed)? {
        return Err(invalid("stats integration source binding differs"));
    }
    verify_members(&root, true)?;
    if manifest["evidence_posture"]
        != posture(&read_json(&root.join("Tree-of-Sophia").join(PORT))?)?
        || manifest["observation"]
            != observation(&read_json(&root.join("Tree-of-Sophia").join(PACKET))?)?
    {
        return Err(invalid(
            "stats integration posture/observation differs from source",
        ));
    }
    Ok(manifest)
}
fn sync_tree(root: &Path) -> io::Result<()> {
    let (files, directories) = tree(root)?;
    for (_, path) in files {
        File::open(path)?.sync_all()?;
    }
    for path in directories.iter().rev() {
        File::open(root.join(path))?.sync_all()?;
    }
    File::open(root)?.sync_all()
}

pub fn build_release(
    source: &Path,
    owner: &Path,
    release: &Path,
    python: &Path,
    cancel: &AtomicI32,
) -> io::Result<Value> {
    let budget = WholeBudget::begin()?;
    let source = safe_absolute(source)?;
    directory(&source)?;
    let owner = safe_absolute(owner)?;
    directory(&owner)?;
    let python = safe_absolute(python)?;
    regular(&python)?;
    let before = records(&source)?;
    let source_revision = revision(&before)?;
    let validator = validator_identity(&owner, &python, budget.deadline()?, cancel)?;
    let release = safe_absolute(release)?;
    let status = crate::kag_downstream_status::Status::new(&release, "stats")?;
    let attempt = status.begin(&source_revision)?;
    let temporary = release.join(format!(".stats-stage-{attempt}"));
    let result: io::Result<Value> = (|| {
        fs::create_dir(&temporary)?;
        let stage = temporary.join("release");
        fs::create_dir(&stage)?;
        let tree_root = stage.join("Tree-of-Sophia");
        fs::create_dir(&tree_root)?;
        for name in SOURCE_PATHS {
            budget_check()?;
            let dest = tree_root.join(name);
            fs::create_dir_all(dest.parent().unwrap())?;
            let raw = read_bounded(&source.join(name), MAX_SOURCE)?;
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(dest)?
                .write_all(&raw)?;
        }
        if records(&tree_root)? != before {
            return Err(invalid("stats staged source differs"));
        }
        let selected_posture = posture(&read_json(&tree_root.join(PORT))?)?;
        let selected_observation = observation(&read_json(&tree_root.join(PACKET))?)?;
        verify_members(&stage, false)?;
        let (code, out, err) = validate_port(&owner, &tree_root.join(PORT), &python, cancel)?;
        if code != 0 {
            let detail = if err.is_empty() { &out } else { &err };
            return Err(invalid(format!(
                "selected stats owner failed ({code}): {}",
                String::from_utf8_lossy(detail)
                    .chars()
                    .take(4096)
                    .collect::<String>()
            )));
        }
        if records(&source)? != before || records(&tree_root)? != before {
            return Err(invalid(
                "stats source or stage changed during owner validation",
            ));
        }
        if validator_identity(&owner, &python, budget.deadline()?, cancel)? != validator {
            return Err(invalid(
                "selected stats validator changed during owner validation",
            ));
        }
        verify_members(&stage, false)?;
        let mut manifest = json!({"schema_version":SCHEMA,"source_kind":SOURCE_KIND,
            "source_revision":source_revision,"evidence_posture":selected_posture,
            "observation":selected_observation,"files":before,"validator_sha256":validator});
        let identity = hash_value(&manifest)?;
        manifest["integration_revision"] = Value::String(identity.clone());
        OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(stage.join("integration.json"))?
            .write_all(&canonical(&manifest)?)?;
        sync_tree(&stage)?;
        let releases = release.join("releases");
        if !releases.exists() {
            fs::create_dir(&releases)?;
        }
        directory(&releases)?;
        let destination = releases.join(&identity);
        if !destination.exists() {
            match shared::rename_new(&stage, &destination) {
                Ok(()) => File::open(&releases)?.sync_all()?,
                Err(_) if destination.exists() => {}
                Err(error) => return Err(error),
            }
        }
        if verify_integration(&destination)? != manifest {
            return Err(invalid("existing stats integration differs"));
        }
        if stage.exists() {
            verify_members(&stage, true)?;
            let (files, directories) = tree(&stage)?;
            for (_, path) in files {
                fs::remove_file(path)?;
            }
            for path in directories.iter().rev() {
                fs::remove_dir(stage.join(path))?;
            }
            fs::remove_dir(&stage)?;
        }
        fs::remove_dir(&temporary)?;
        status.succeed(
            &attempt,
            &identity,
            &digest(&destination.join("integration.json"))?,
        )?;
        Ok(manifest)
    })();
    if let Err(error) = &result {
        let detail: String = error
            .to_string()
            .replace('\0', "\\x00")
            .replace('\u{7f}', "\\x7f")
            .chars()
            .take(4096)
            .collect();
        if let Err(recording) = status.fail(&attempt, &detail) {
            return Err(invalid(format!(
                "stats release failed: {detail}; status failure recording failed: {recording}"
            )));
        }
    }
    result
}
pub fn status_release(release: &Path, expected: &str) -> io::Result<Value> {
    let _budget = WholeBudget::begin()?;
    hex(&Value::String(expected.to_owned()))?;
    let release = safe_absolute(release)?;
    let mut result =
        crate::kag_downstream_status::Status::new(&release, "stats")?.status(expected)?;
    result["source_kind"] = Value::String(SOURCE_KIND.into());
    result["integration_revision"] = Value::Null;
    result["observation"] = Value::Null;
    let success = &result["state"]["last_success"];
    if success.is_null() {
        return Ok(result);
    }
    let integration = hex(&success["artifact_revision"])?;
    let path = release.join("releases").join(integration);
    let manifest = verify_integration(&path)?;
    if manifest["source_revision"] != success["source_revision"]
        || digest(&path.join("integration.json"))? != hex(&success["artifact_manifest_sha256"])?
    {
        return Err(invalid("last successful stats integration binding differs"));
    }
    result["integration_revision"] = manifest["integration_revision"].clone();
    result["observation"] = manifest["observation"].clone();
    Ok(result)
}
