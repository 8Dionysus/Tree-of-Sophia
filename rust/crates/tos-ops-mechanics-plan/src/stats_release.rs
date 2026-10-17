//! Standalone ToS stats exports: local validation, immutable publication and status.
use crate::release_files::{
    self as shared, WholeBudget, budget_check, canonical, digest, directory, hex, invalid, keys,
    read_json, regular, safe_absolute, tree,
};
use serde_json::{Value, json};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::Path,
};
use tos_foundation::Digest256;

pub const SOURCE_SCHEMA: &str = "tos_stats_port_export_v1";
pub const SCHEMA: &str = "tos_stats_export_release_v1";
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
        files.push("export.json".into());
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

/// Validate the ToS-owned export and its local packet bindings. Consumer
/// admission is deliberately absent: a consumer validates at its own boundary.
pub fn validate_port(port: &Path) -> io::Result<Value> {
    let _budget = WholeBudget::begin()?;
    let port = safe_absolute(port)?;
    if port.file_name().and_then(|v| v.to_str()) != Some("port.manifest.json") {
        return Err(invalid("select the ToS stats/port.manifest.json"));
    }
    let root = port
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| invalid("stats source root absent"))?;
    if root.join(PORT) != port {
        return Err(invalid("stats port location differs"));
    }
    let document = read_json(&port)?;
    if document["schema_version"] != "aoa_stats_local_port_v1"
        || document["owner_repo"] != "Tree-of-Sophia"
        || document["evidence_posture"]["raw_content_allowed"] != false
    {
        return Err(invalid("unsupported ToS stats port"));
    }
    nonempty(&document["contract_version"])?;
    let measurements = document["measurements"]
        .as_array()
        .filter(|v| !v.is_empty())
        .ok_or_else(|| invalid("stats measurements absent"))?;
    let exports = document["exports"]
        .as_array()
        .filter(|v| !v.is_empty())
        .ok_or_else(|| invalid("stats exports absent"))?;
    let mut identities = std::collections::BTreeSet::new();
    for contract in measurements {
        nonempty(&contract["contract_version"])?;
        if contract["schema_version"] != "aoa_stats_measurement_contract_v1"
            || !identities.insert(nonempty(&contract["measurement_id"])?.to_owned())
            || contract["owner_repo"] != "Tree-of-Sophia"
        {
            return Err(invalid(
                "stats measurement identity differs or is duplicated",
            ));
        }
    }
    let mut packets = std::collections::BTreeSet::new();
    let mut exported = std::collections::BTreeSet::new();
    for export in exports {
        let id = nonempty(&export["measurement_id"])?;
        if !identities.contains(id) || !exported.insert(id.to_owned()) {
            return Err(invalid("stats export measurement differs or is duplicated"));
        }
        let (index, contract) = measurements
            .iter()
            .enumerate()
            .find(|(_, v)| v["measurement_id"] == id)
            .unwrap();
        for reference in export["packet_refs"]
            .as_array()
            .filter(|v| !v.is_empty())
            .ok_or_else(|| invalid("stats packet refs absent"))?
        {
            let reference = shared::relative(reference)?;
            if !reference.starts_with("stats/packets/") || !packets.insert(reference.to_owned()) {
                return Err(invalid("stats packet path differs or is duplicated"));
            }
            let packet = read_json(&root.join(reference))?;
            if packet["schema_version"] != "aoa_stats_measurement_packet_v1"
                || packet["measurement_id"] != id
                || packet["writer_repo"] != "Tree-of-Sophia"
                || packet["contract_ref"]
                    != format!("stats/port.manifest.json#/measurements/{index}")
                || packet["contract_version"] != contract["contract_version"]
                || packet["posture"]["raw_content_included"] != false
            {
                return Err(invalid("stats packet binding differs"));
            }
            observation(&packet)?;
            if packet["value"]["status"] == "observed" && contract["statistic"] == "ratio" {
                let value = &packet["value"];
                let numerator = value["numerator"]
                    .as_u64()
                    .ok_or_else(|| invalid("stats numerator"))?;
                let denominator = value["denominator"]
                    .as_u64()
                    .filter(|v| *v > 0)
                    .ok_or_else(|| invalid("stats denominator"))?;
                let number = value["number"]
                    .as_f64()
                    .filter(|v| v.is_finite())
                    .ok_or_else(|| invalid("stats ratio number"))?;
                if numerator > denominator
                    || (number - numerator as f64 / denominator as f64).abs() > 1e-12
                {
                    return Err(invalid("stats ratio is inconsistent"));
                }
            }
        }
    }
    if exported != identities {
        return Err(invalid("stats measurement lacks an export"));
    }
    Ok(json!({"valid":true,"owner_repo":"Tree-of-Sophia",
        "measurements":measurements.len(),"packets":packets.len(),"grants_semantic_acceptance":false}))
}

pub fn verify_export(root: &Path) -> io::Result<Value> {
    let _budget = WholeBudget::begin()?;
    let root = safe_absolute(root)?;
    directory(&root)?;
    let manifest = read_json(&root.join("export.json"))?;
    keys(
        &manifest,
        &[
            "schema_version",
            "source_kind",
            "source_revision",
            "evidence_posture",
            "observation",
            "files",
            "export_revision",
        ],
    )?;
    if manifest["schema_version"] != SCHEMA || manifest["source_kind"] != SOURCE_KIND {
        return Err(invalid("unsupported stats export"));
    }
    hex(&manifest["source_revision"])?;
    let integration = hex(&manifest["export_revision"])?;
    if root.file_name().and_then(|n| n.to_str()) != Some(integration) {
        return Err(invalid("stats export directory identity"));
    }
    let mut body = manifest.clone();
    body.as_object_mut().unwrap().remove("export_revision");
    if hash_value(&body)? != integration {
        return Err(invalid("stats export manifest identity"));
    }
    let observed = records(&root.join("Tree-of-Sophia"))?;
    if manifest["files"] != observed || manifest["source_revision"] != revision(&observed)? {
        return Err(invalid("stats export source binding differs"));
    }
    verify_members(&root, true)?;
    validate_port(&root.join("Tree-of-Sophia").join(PORT))?;
    if manifest["evidence_posture"]
        != posture(&read_json(&root.join("Tree-of-Sophia").join(PORT))?)?
        || manifest["observation"]
            != observation(&read_json(&root.join("Tree-of-Sophia").join(PACKET))?)?
    {
        return Err(invalid(
            "stats export posture/observation differs from source",
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

pub fn build_release(source: &Path, release: &Path) -> io::Result<Value> {
    let _budget = WholeBudget::begin()?;
    let source = safe_absolute(source)?;
    directory(&source)?;
    let before = records(&source)?;
    let source_revision = revision(&before)?;
    validate_port(&source.join(PORT))?;
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
        validate_port(&tree_root.join(PORT))?;
        if records(&source)? != before || records(&tree_root)? != before {
            return Err(invalid("stats source changed during export"));
        }
        verify_members(&stage, false)?;
        let mut manifest = json!({"schema_version":SCHEMA,"source_kind":SOURCE_KIND,
            "source_revision":source_revision,"evidence_posture":selected_posture,
            "observation":selected_observation,"files":before});
        let identity = hash_value(&manifest)?;
        manifest["export_revision"] = Value::String(identity.clone());
        OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(stage.join("export.json"))?
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
        if verify_export(&destination)? != manifest {
            return Err(invalid("existing stats export differs"));
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
            &digest(&destination.join("export.json"))?,
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
    result["export_revision"] = Value::Null;
    result["observation"] = Value::Null;
    let success = &result["state"]["last_success"];
    if success.is_null() {
        return Ok(result);
    }
    let integration = hex(&success["artifact_revision"])?;
    let path = release.join("releases").join(integration);
    let manifest = verify_export(&path)?;
    if manifest["source_revision"] != success["source_revision"]
        || digest(&path.join("export.json"))? != hex(&success["artifact_manifest_sha256"])?
    {
        return Err(invalid("last successful stats export binding differs"));
    }
    result["export_revision"] = manifest["export_revision"].clone();
    result["observation"] = manifest["observation"].clone();
    Ok(result)
}
