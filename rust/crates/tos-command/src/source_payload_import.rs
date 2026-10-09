//! Rights-gated transfer of frozen source-payload plans.
//!
//! This owner consumes the authored server-import contracts, verifies the
//! exact selected payload bytes, delegates R2 bytes to `corpus_r2_cli`, and
//! records immutable receipts. Publication state remains a separate local
//! registry. No operation discovers payloads or admits source meaning.

use crate::corpus_r2_cli::{
    R2TransportKindV1, R2TransportV1, raw_read_verified_v1, raw_transfer_v1,
};
use crate::source_acquisition_batch as io_owner;
use rustix::fs::{AtFlags, FlockOperation, Mode, OFlags};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tos_foundation::{Digest256, Digest256Hasher};
use tos_source_store::ChunkedFileTransportV1;

type Result<T> = std::result::Result<T, String>;
const PLAN_SCHEMA: &str = "ToS/contracts/server-import-contract.schema.json";
const RECEIPT_SCHEMA: &str = "ToS/contracts/server-import-receipt.schema.json";
const REVIEW_SCHEMA: &str = "ToS/contracts/source-payload-rights-review.schema.json";
const MAX_JSON: u64 = 16 * 1024 * 1024;
const MAX_PAYLOAD: u64 = 300 * 1024 * 1024;
pub const HELP: &str = "usage: tos-native-owner-command source-payload < REQUEST_JSON\n\nOperations: verify-local, import, read-local, read-remote, registry-enable, registry-disable, batch.\nImport requires confirm_transfer=true and uses the shared Wrangler/REST R2 byte boundary. Batch preflights every frozen plan before creating one transport.\n";
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone)]
struct PlanContext {
    repo: PathBuf,
    path: PathBuf,
    plan_ref: String,
    plan_sha256: String,
    plan: Value,
}

#[derive(Clone)]
struct VerifiedFile {
    item_ref: String,
    file_ref: String,
    relative_path: String,
    local_path: PathBuf,
    byte_size: u64,
    sha256: String,
    media_type: String,
    manifest_ref: String,
    manifest_sha256: String,
    rights_ref: String,
    rights_sha256: String,
}

fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing text field: {key}"))
}

fn optional_text<'a>(value: &'a Value, key: &str) -> Result<Option<&'a str>> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value)),
        _ => Err(format!("{key} must be text or null")),
    }
}

fn unsigned(value: &Value, key: &str) -> Result<u64> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("{key} must be a nonnegative integer"))
}

fn absolute_path(value: &Value, key: &str) -> Result<PathBuf> {
    let raw = text(value, key)?;
    let path = PathBuf::from(raw);
    if !path.is_absolute() || path.to_str() != Some(raw) {
        return Err(format!("{key} must be an absolute UTF-8 path"));
    }
    Ok(path)
}

fn strict_object(value: &Value, required: &[&str], optional: &[&str]) -> Result<()> {
    let object = value.as_object().ok_or("request object required")?;
    for key in required {
        if !object.contains_key(*key) {
            return Err(format!("missing request field: {key}"));
        }
    }
    for key in object.keys() {
        if !required.contains(&key.as_str()) && !optional.contains(&key.as_str()) {
            return Err(format!("unknown request field: {key}"));
        }
    }
    Ok(())
}

fn read_json(path: &Path) -> Result<Value> {
    let raw = io_owner::read_bytes(path, None, false, false, MAX_JSON)?;
    io_owner::parse(&raw)
}

fn digest_hex(bytes: &[u8]) -> String {
    Digest256::of_bytes(bytes).to_hex()
}

fn validate_schema(repo: &Path, ref_path: &str, value: &Value) -> Result<()> {
    let path = io_owner::path_under(repo, ref_path)?;
    let schema = read_json(&path)?;
    let validator = jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .should_validate_formats(true)
        .offline()
        .build(&schema)
        .map_err(|e| {
            format!(
                "cannot prepare {} schema: {e}",
                Path::new(ref_path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
            )
        })?;
    validator.validate(value).map_err(|e| {
        format!(
            "{} schema rejected input: {e}",
            Path::new(ref_path)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
        )
    })
}

fn repo_file(repo: &Path, reference: &str) -> Result<PathBuf> {
    if !reference.starts_with("ToS/") {
        return Err(format!("source reference is outside ToS/: {reference}"));
    }
    io_owner::path_under(repo, reference)
}

fn load_plan(repo: &Path, requested: &Path) -> Result<PlanContext> {
    if !repo.is_absolute() {
        return Err("repo_root must be absolute".into());
    }
    tos_fd_open::open_absolute_directory(repo)
        .map_err(|_| "repo_root must be a no-follow directory")?;
    let path = if requested.is_absolute() {
        requested.to_owned()
    } else {
        repo.join(requested)
    };
    let raw = io_owner::read_bytes(&path, None, false, false, MAX_JSON)?;
    let plan = io_owner::parse(&raw)?;
    validate_schema(repo, PLAN_SCHEMA, &plan)?;
    let plan_sha256 = digest_hex(&raw);
    let plan_ref = match path.strip_prefix(repo) {
        Ok(relative) => {
            let value = relative.to_str().ok_or("plan reference must be UTF-8")?;
            let safe = value.to_owned();
            io_owner::safe_ref(&safe)?;
            safe
        }
        Err(_) => text(&plan, "server_import_id")?.to_owned(),
    };
    let context = PlanContext {
        repo: repo.to_owned(),
        path,
        plan_ref,
        plan_sha256,
        plan,
    };
    let review_status = text(&context.plan["rights_policy"], "review_status")?;
    if review_status == "agent-reviewed" {
        if unsigned(&context.plan, "contract_version")? < 3 {
            return Err("agent-reviewed requires contract_version >= 3".into());
        }
        let review = context.plan["rights_policy"]
            .get("rights_review")
            .ok_or("agent-reviewed plan lacks rights_review")?;
        let review_path = repo_file(repo, text(review, "ref")?)?;
        let review_bytes = io_owner::read_bytes(&review_path, None, false, false, MAX_JSON)?;
        if digest_hex(&review_bytes) != text(review, "sha256")? {
            return Err("rights review SHA-256 differs from frozen plan".into());
        }
        let review_doc = io_owner::parse(&review_bytes)?;
        validate_schema(repo, REVIEW_SCHEMA, &review_doc)?;
    }
    Ok(context)
}

fn checked_plan_inputs(context: &PlanContext) -> Result<(PathBuf, Value, PathBuf, Value)> {
    let manifest_ref = text(&context.plan["manifest"], "ref")?;
    let manifest_path = repo_file(&context.repo, manifest_ref)?;
    let manifest_bytes = io_owner::read_bytes(&manifest_path, None, false, false, MAX_JSON)?;
    if digest_hex(&manifest_bytes) != text(&context.plan["manifest"], "sha256")? {
        return Err("manifest SHA-256 differs from frozen plan".into());
    }
    if context.plan["manifest"]["verified"] != true {
        return Err("frozen plan marks manifest unverified".into());
    }
    let manifest = io_owner::parse(&manifest_bytes)?;
    if manifest.get("item_id") != context.plan.get("item_ref") {
        return Err("plan Item does not match manifest Item".into());
    }
    let plan_files = context.plan["payload_files"]
        .as_array()
        .ok_or("plan payload_files must be an array")?;
    let manifest_files = manifest["payload_files"]
        .as_array()
        .ok_or("manifest payload_files must be an array")?;
    let mut plan_by_id = BTreeMap::new();
    let mut path_set = BTreeSet::new();
    for entry in plan_files {
        let id = text(entry, "file_ref")?.to_owned();
        let relative = text(entry, "relative_path")?.to_owned();
        if plan_by_id.insert(id, entry).is_some() || !path_set.insert(relative) {
            return Err("plan payload inventory has duplicate File IDs or paths".into());
        }
    }
    let mut manifest_by_id = BTreeMap::new();
    path_set.clear();
    for entry in manifest_files {
        let id = text(entry, "file_id")?.to_owned();
        let relative = text(entry, "relative_path")?.to_owned();
        if manifest_by_id.insert(id, entry).is_some() || !path_set.insert(relative) {
            return Err("manifest payload inventory has duplicate File IDs or paths".into());
        }
    }
    if plan_by_id.keys().collect::<Vec<_>>() != manifest_by_id.keys().collect::<Vec<_>>() {
        return Err("plan and manifest do not contain the same File inventory".into());
    }
    for (id, planned) in &plan_by_id {
        let witnessed = manifest_by_id
            .get(id)
            .ok_or("manifest File inventory mismatch")?;
        for field in ["relative_path", "byte_size", "sha256"] {
            if planned.get(field) != witnessed.get(field) {
                return Err(format!(
                    "plan/manifest inventory mismatch for {id}: {field}"
                ));
            }
        }
    }
    let rights_ref = text(&context.plan["rights_policy"], "rights_record_ref")?;
    let rights_path = repo_file(&context.repo, rights_ref)?;
    let rights_bytes = io_owner::read_bytes(&rights_path, None, false, false, MAX_JSON)?;
    if digest_hex(&rights_bytes) != text(&context.plan["rights_policy"], "rights_record_sha256")? {
        return Err("rights-record SHA-256 differs from frozen plan".into());
    }
    let rights = io_owner::parse(&rights_bytes)?;
    if let Some(ref_from_manifest) = manifest.get("rights_ref").and_then(Value::as_str)
        && ref_from_manifest != rights_ref
    {
        return Err("plan rights record differs from manifest rights_ref".into());
    }
    Ok((manifest_path, manifest, rights_path, rights))
}

fn rights_values(rights: &Value, field: &str) -> Vec<String> {
    let mut containers = vec![rights];
    if let Some(layers) = rights.get("layer_assessments").and_then(Value::as_array) {
        containers.extend(layers);
    }
    containers
        .into_iter()
        .filter_map(|v| v.get(field).and_then(Value::as_str))
        .map(normalize)
        .collect()
}

fn normalize(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace(['-', ' '], "_")
}

fn source_posture_resolution(
    context: &PlanContext,
    rights: &Value,
) -> Result<(BTreeSet<String>, BTreeSet<String>, BTreeSet<String>)> {
    if context.plan["rights_policy"]["review_status"] != "agent-reviewed" {
        return Ok((BTreeSet::new(), BTreeSet::new(), BTreeSet::new()));
    }
    let review_info = &context.plan["rights_policy"]["rights_review"];
    let review_path = repo_file(&context.repo, text(review_info, "ref")?)?;
    let review = read_json(&review_path)?;
    let Some(resolution) = review.get("source_posture_resolution") else {
        return Ok((BTreeSet::new(), BTreeSet::new(), BTreeSet::new()));
    };
    let valid_assessments = [
        "licensed",
        "open_licensed",
        "public_domain_reviewed",
        "public_domain",
        "permission_granted",
    ];
    if rights_values(rights, "assessment_status")
        .iter()
        .any(|v| !valid_assessments.contains(&v.as_str()))
    {
        return Err(
            "source posture resolution requires positive assessments for every layer".into(),
        );
    }
    if context.plan["allowed_derivatives"]
        .as_object()
        .is_none_or(|entries| entries.values().any(|v| v["state"] != "prohibited"))
    {
        return Err("source posture resolution only permits exact-byte private retention".into());
    }
    let server = strings_set(
        resolution.get("server_processing_postures"),
        "server_processing_postures",
    )?;
    let redistribution = strings_set(
        resolution.get("redistribution_postures"),
        "redistribution_postures",
    )?;
    let expected_server = rights_values(rights, "server_processing_posture")
        .into_iter()
        .filter(|v| ["not_authorized", "authorized_with_conditions"].contains(&v.as_str()))
        .collect::<BTreeSet<_>>();
    let expected_redistribution = rights_values(rights, "redistribution_posture")
        .into_iter()
        .filter(|v| {
            [
                "not_authorized",
                "metadata_only",
                "authorized_with_conditions",
            ]
            .contains(&v.as_str())
        })
        .collect::<BTreeSet<_>>();
    let mut source_conditions = BTreeSet::new();
    let mut containers = vec![rights];
    if let Some(layers) = rights.get("layer_assessments").and_then(Value::as_array) {
        containers.extend(layers);
    }
    for container in containers {
        if let Some(items) = container.get("restrictions").and_then(Value::as_array) {
            for item in items {
                if let Some(s) = item.as_str() {
                    source_conditions.insert(s.to_owned());
                }
            }
        }
    }
    let retained = resolution
        .get("retained_conditions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let retained_set = retained
        .iter()
        .filter_map(|v| {
            v.get("source_condition")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .collect::<BTreeSet<_>>();
    if retained_set != source_conditions || retained_set.len() != retained.len() {
        return Err("source condition review must retain every exact condition once".into());
    }
    if server != expected_server
        || redistribution != expected_redistribution
        || server.is_empty() && redistribution.is_empty() && retained_set.is_empty()
    {
        return Err(
            "source posture resolution does not exactly match historical admission defaults".into(),
        );
    }
    Ok((server, redistribution, retained_set))
}

fn strings_set(value: Option<&Value>, label: &str) -> Result<BTreeSet<String>> {
    value
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{label} must be an array"))?
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("{label} entry must be text"))
        })
        .collect()
}

fn validate_agent_review(
    context: &PlanContext,
    manifest_path: &Path,
    rights_path: &Path,
) -> Result<Option<(String, String)>> {
    if context.plan["rights_policy"]["review_status"] != "agent-reviewed" {
        return Ok(None);
    }
    if unsigned(&context.plan, "contract_version")? < 3 {
        return Err("agent-reviewed requires contract_version >= 3".into());
    }
    let info = &context.plan["rights_policy"]["rights_review"];
    let reference = text(info, "ref")?;
    let path = repo_file(&context.repo, reference)?;
    let raw = io_owner::read_bytes(&path, None, false, false, MAX_JSON)?;
    let expected = text(info, "sha256")?;
    if digest_hex(&raw) != expected {
        return Err("agent rights review evidence digest mismatch".into());
    }
    let review = io_owner::parse(&raw)?;
    validate_schema(&context.repo, REVIEW_SCHEMA, &review)?;
    let scope = &review["scope"];
    let planned = context.plan["payload_files"]
        .as_array()
        .ok_or("plan File array required")?
        .iter()
        .map(|v| text(v, "file_ref").map(str::to_owned))
        .collect::<Result<BTreeSet<_>>>()?;
    let reviewed = strings_set(scope.get("file_refs"), "review file_refs")?;
    if scope["item_ref"] != context.plan["item_ref"] || reviewed != planned {
        return Err(
            "agent rights review scope does not cover exactly planned Item and Files".into(),
        );
    }
    if scope["manifest_ref"] != context.plan["manifest"]["ref"]
        || scope["manifest_sha256"] != context.plan["manifest"]["sha256"]
    {
        return Err("agent review manifest scope differs from plan".into());
    }
    if scope["rights_record_ref"] != context.plan["rights_policy"]["rights_record_ref"]
        || scope["rights_record_sha256"] != context.plan["rights_policy"]["rights_record_sha256"]
    {
        return Err("agent review rights-record scope differs from plan".into());
    }
    if review["actor"]["kind"] != "model" {
        return Err("agent review actor kind is not model".into());
    }
    let decision = &review["decision"];
    if decision["access_class"] != "controlled-research"
        || decision["raw_cloud_retention"] != "controlled-research"
        || decision["server_processing"] != "authorized"
        || decision["publication"] != "not-published"
    {
        return Err("agent rights review is not limited to controlled research".into());
    }
    if context.plan["access_class"] != "controlled-research"
        || context.plan["publication_status"] != "not-published"
    {
        return Err("agent-reviewed plans cannot authorize public payload publication".into());
    }
    if decision["expires_at"] != context.plan["rights_policy"]["expires_at"] {
        return Err("agent rights review expiry differs from plan".into());
    }
    let revocation_check_ref = text(decision, "revocation_check_ref")?;
    let revocation_check_path = repo_file(&context.repo, revocation_check_ref)?;
    // The v1 review contract gives this reference no digest or typed state.
    // Open the exact bounded no-follow file for custody, but do not treat its
    // prose as revocation approval; current plan statuses below are the typed
    // takedown gate.
    let revocation_check_bytes =
        io_owner::read_bytes(&revocation_check_path, None, false, false, MAX_JSON)?;
    if revocation_check_bytes.is_empty() {
        return Err("agent rights review revocation-check reference is empty".into());
    }
    let evidence = review["license_evidence"]
        .as_array()
        .ok_or("agent review license evidence array required")?;
    if !evidence.iter().any(|v| v["kind"] == "primary-license") {
        return Err("agent review lacks primary license evidence".into());
    }
    let license_refs = strings_set(
        context.plan["rights_policy"].get("permission_or_license_refs"),
        "permission_or_license_refs",
    )?;
    for item in evidence {
        let evidence_ref = text(item, "evidence_ref")?;
        let evidence_path = repo_file(&context.repo, evidence_ref)?;
        let evidence_raw = io_owner::read_bytes(&evidence_path, None, false, false, MAX_JSON)?;
        if digest_hex(&evidence_raw) != text(item, "evidence_sha256")? {
            return Err("tracked license evidence digest mismatch".into());
        }
        if !license_refs.contains(text(item, "source_ref")?) && !license_refs.contains(evidence_ref)
        {
            return Err("license evidence is not bound to plan permission_or_license_refs".into());
        }
    }
    if decision["conditions"].as_array().is_none_or(Vec::is_empty) {
        return Err("agent review must preserve processing conditions".into());
    }
    if digest_file(manifest_path)? != text(scope, "manifest_sha256")?
        || digest_file(rights_path)? != text(scope, "rights_record_sha256")?
    {
        return Err("agent review scope no longer matches source bytes".into());
    }
    Ok(Some((reference.to_owned(), expected.to_owned())))
}

fn validate_rights(context: &PlanContext, rights: &Value) -> Result<()> {
    if let Some(scope) = rights.get("scope_refs").and_then(Value::as_array) {
        if !scope.is_empty() {
            let mut required = BTreeSet::from([text(&context.plan, "item_ref")?.to_owned()]);
            for file in context.plan["payload_files"]
                .as_array()
                .ok_or("plan File array required")?
            {
                required.insert(text(file, "file_ref")?.to_owned());
            }
            let have = scope
                .iter()
                .filter_map(Value::as_str)
                .collect::<BTreeSet<_>>();
            if required.iter().any(|value| !have.contains(value.as_str())) {
                return Err("rights record scope does not cover planned Item and Files".into());
            }
        }
    }
    let blocked = [
        "denied",
        "rejected",
        "restricted",
        "rights_unknown",
        "unknown",
        "not_authorized",
        "unauthorized",
        "prohibited",
        "forbidden",
    ];
    let known_assessments = [
        "licensed",
        "open_licensed",
        "public_domain_reviewed",
        "public_domain",
        "permission_granted",
        "research_only",
    ];
    let assessments = rights_values(rights, "assessment_status");
    if assessments.is_empty()
        || assessments
            .iter()
            .any(|v| !known_assessments.contains(&v.as_str()) && !blocked.contains(&v.as_str()))
    {
        return Err("rights record has an unsupported assessment status".into());
    }
    if assessments.iter().any(|v| blocked.contains(&v.as_str())) {
        return Err("rights record contains a denied or restricted assessment".into());
    }
    let plan_assessment = normalize(text(&context.plan["rights_policy"], "assessment_status")?);
    let positive = [
        "licensed",
        "open_licensed",
        "public_domain_reviewed",
        "public_domain",
        "permission_granted",
    ];
    if [
        "open_licensed",
        "public_domain_reviewed",
        "permission_granted",
    ]
    .contains(&plan_assessment.as_str())
        && !assessments.iter().any(|v| positive.contains(&v.as_str()))
    {
        return Err("rights record does not support positive plan assessment".into());
    }
    let (reviewed_server, reviewed_redist, retained) = source_posture_resolution(context, rights)?;
    let mut conditions = BTreeSet::new();
    let mut containers = vec![rights];
    if let Some(layers) = rights.get("layer_assessments").and_then(Value::as_array) {
        containers.extend(layers);
    }
    for c in containers {
        if let Some(items) = c.get("restrictions").and_then(Value::as_array) {
            for i in items {
                if let Some(s) = i.as_str() {
                    conditions.insert(s.to_owned());
                }
            }
        }
    }
    if conditions.difference(&retained).next().is_some() {
        return Err("rights record restrictions require an exact source-bound review".into());
    }
    let server_known = ["authorized", "allowed", "open", "local_only"];
    for value in rights_values(rights, "server_processing_posture") {
        if !server_known.contains(&value.as_str())
            && !blocked.contains(&value.as_str())
            && !reviewed_server.contains(&value)
        {
            return Err("rights record has an unsupported server-processing posture".into());
        }
        if blocked.contains(&value.as_str()) && !reviewed_server.contains(&value) {
            return Err("rights record does not authorize server processing".into());
        }
    }
    let redist_known = ["authorized", "allowed", "open", "public", "local_only"];
    let redistribution = rights_values(rights, "redistribution_posture");
    for value in &redistribution {
        if !redist_known.contains(&value.as_str())
            && !blocked.contains(&value.as_str())
            && !reviewed_redist.contains(value)
        {
            return Err("rights record has an unsupported redistribution posture".into());
        }
        if blocked.contains(&value.as_str()) && !reviewed_redist.contains(value) {
            return Err("rights record does not authorize redistribution".into());
        }
        if context.plan["access_class"] == "public-payload"
            && !["authorized", "allowed", "open", "public"].contains(&value.as_str())
        {
            return Err("rights record does not authorize public redistribution".into());
        }
    }
    let visibility = rights_values(rights, "visibility");
    let visibility_known = [
        "local_only",
        "private",
        "controlled",
        "public",
        "metadata_only",
    ];
    if visibility
        .iter()
        .any(|v| !visibility_known.contains(&v.as_str()))
    {
        return Err("rights record has an unsupported visibility posture".into());
    }
    if context.plan["access_class"] == "public-payload"
        && visibility
            .iter()
            .any(|v| ["local_only", "private", "restricted"].contains(&v.as_str()))
    {
        return Err("local-only rights cannot authorize public payload".into());
    }
    if context.plan["access_class"] == "controlled-research"
        && visibility.contains(&"local_only".to_owned())
        && context.plan["rights_policy"]["review_status"] != "agent-reviewed"
    {
        return Err("legacy local-only rights require explicit v3 controlled review".into());
    }
    Ok(())
}

fn digest_file(path: &Path) -> Result<String> {
    Ok(digest_hex(&io_owner::read_bytes(
        path, None, false, false, MAX_JSON,
    )?))
}

fn check_expiry(value: Option<&Value>) -> Result<()> {
    let Some(raw) = value.and_then(Value::as_str) else {
        return Ok(());
    };
    let expires = parse_rfc3339(raw).ok_or("rights expires_at is not a valid RFC3339 timestamp")?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "system clock predates Unix epoch")?
        .as_secs() as i64;
    if expires <= now {
        return Err("rights policy has expired".into());
    }
    Ok(())
}

fn parse_rfc3339(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() < 20
        || b.get(4) != Some(&b'-')
        || b.get(7) != Some(&b'-')
        || !matches!(b.get(10), Some(b'T' | b't'))
        || b.get(13) != Some(&b':')
        || b.get(16) != Some(&b':')
    {
        return None;
    }
    let n = |a: usize, z: usize| std::str::from_utf8(b.get(a..z)?).ok()?.parse::<i64>().ok();
    let (year, month, day, hour, minute, second) = (
        n(0, 4)?,
        n(5, 7)?,
        n(8, 10)?,
        n(11, 13)?,
        n(14, 16)?,
        n(17, 19)?,
    );
    if !(1..=12).contains(&month)
        || !(0..=23).contains(&hour)
        || !(0..=59).contains(&minute)
        || !(0..=59).contains(&second)
    {
        return None;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    if day < 1 || day > days[(month - 1) as usize] {
        return None;
    }
    let mut i = 19;
    if b.get(i) == Some(&b'.') {
        i += 1;
        let start = i;
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if i == start {
            return None;
        }
    }
    let offset = match b.get(i) {
        Some(b'Z' | b'z') if i + 1 == b.len() => 0,
        Some(sign @ (b'+' | b'-')) if i + 6 == b.len() && b.get(i + 3) == Some(&b':') => {
            let oh = n(i + 1, i + 3)?;
            let om = n(i + 4, i + 6)?;
            if oh > 23 || om > 59 {
                return None;
            }
            let v = oh * 3600 + om * 60;
            if *sign == b'+' { v } else { -v }
        }
        _ => return None,
    };
    let y = year - i64::from(month <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = month + if month > 2 { -3 } else { 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days_since_epoch = era * 146097 + doe - 719468;
    Some(days_since_epoch * 86400 + hour * 3600 + minute * 60 + second - offset)
}

fn enforce_transfer(context: &PlanContext) -> Result<()> {
    let plan = &context.plan;
    let access = text(plan, "access_class")?;
    if !["controlled-research", "public-payload"].contains(&access) {
        return Err(format!("payload transfer denied for access_class={access}"));
    }
    if plan["payload_transfer_authorized"] != true {
        return Err("payload_transfer_authorized is false".into());
    }
    let approval = &plan["operator_transfer_approval"];
    if approval["approved"] != true
        || approval["approved_by_real_human"] != true
        || !text(approval, "approval_ref").is_ok_and(|value| !value.is_empty())
        || !text(approval, "approved_at").is_ok_and(|value| !value.is_empty())
    {
        return Err("real-human operator transfer approval is missing".into());
    }
    validate_current_server_import_state(context, "transfer")?;
    if access == "public-payload"
        && ["withdrawn", "metadata-only"].contains(&text(plan, "publication_status")?)
    {
        return Err("public payload plan is not publication-capable".into());
    }
    let (_, manifest, rights_path, rights) = checked_plan_inputs(context)?;
    let manifest_path = repo_file(&context.repo, text(&plan["manifest"], "ref")?)?;
    let review_status = text(&plan["rights_policy"], "review_status")?;
    if review_status == "agent-reviewed" {
        validate_agent_review(context, &manifest_path, &rights_path)?;
    } else if !["human-reviewed", "legal-reviewed"].contains(&review_status) {
        return Err("source review is not human/legal-reviewed or agent-reviewed".into());
    }
    validate_rights(context, &rights)?;
    if context.plan["rights_policy"]["recheck_before_transfer"] != true {
        return Err("rights policy disabled mandatory pre-transfer recheck".into());
    }
    check_expiry(context.plan["rights_policy"].get("expires_at"))?;
    if context.plan["rights_policy"]["permission_or_license_refs"]
        .as_array()
        .is_none_or(Vec::is_empty)
    {
        return Err("rights policy has no permission or license evidence".into());
    }
    let assessment = normalize(text(&context.plan["rights_policy"], "assessment_status")?);
    if access == "public-payload"
        && ![
            "public_domain_reviewed",
            "open_licensed",
            "permission_granted",
        ]
        .contains(&assessment.as_str())
    {
        return Err(
            "public-payload requires public-domain/open-license/permission assessment".into(),
        );
    }
    if access == "controlled-research"
        && ["rights_unknown", "restricted", "rejected"].contains(&assessment.as_str())
    {
        return Err("controlled-research cannot transfer restricted or unknown rights".into());
    }
    let _ = manifest;
    Ok(())
}

fn validate_current_server_import_state(context: &PlanContext, action: &str) -> Result<()> {
    let status = text(&context.plan, "server_import_status")?;
    if !["approved-not-uploaded", "imported"].contains(&status) {
        return Err(format!(
            "current server-import/takedown status {status} blocks {action}"
        ));
    }
    Ok(())
}

fn validate_current_takedown_state(context: &PlanContext, action: &str) -> Result<()> {
    validate_current_server_import_state(context, action)?;
    let publication_status = text(&context.plan, "publication_status")?;
    if ["withdrawn", "metadata-only"].contains(&publication_status) {
        return Err(format!(
            "current publication status {publication_status} blocks {action}"
        ));
    }
    Ok(())
}

fn payload_path(
    context: &PlanContext,
    source_root: &Path,
    layout: &str,
    manifest_path: &Path,
    relative: &str,
) -> Result<PathBuf> {
    if !source_root.is_absolute() {
        return Err("payload source root must be absolute".into());
    }
    let parts = relative
        .strip_prefix("payload/")
        .ok_or("payload path must begin with payload/")?;
    if parts.is_empty()
        || parts.contains('/')
        || parts == "."
        || parts == ".."
        || relative.contains(['\\', '\0'])
    {
        return Err("payload path must contain exactly one safe filename".into());
    }
    let base = match layout {
        "repo" => manifest_path
            .strip_prefix(&context.repo)
            .map_err(|_| "repo layout requires an in-repository manifest")?
            .parent()
            .ok_or("manifest parent missing")?
            .to_owned(),
        "source-witness" => manifest_path
            .strip_prefix(context.repo.join("ToS/source-witnesses"))
            .map_err(|_| "source-witness layout requires a ToS/source-witnesses manifest")?
            .parent()
            .ok_or("manifest parent missing")?
            .to_owned(),
        "item" => PathBuf::new(),
        _ => return Err("unknown payload-source layout".into()),
    };
    let mut suffix = base;
    suffix.push("payload");
    suffix.push(parts);
    let rel = suffix.to_str().ok_or("payload path must be UTF-8")?;
    io_owner::path_under(source_root, rel)
}

fn verify_local(
    context: &PlanContext,
    source_root: &Path,
    layout: &str,
    file_ref: Option<&str>,
) -> Result<Vec<VerifiedFile>> {
    let (manifest_path, manifest, _, _) = checked_plan_inputs(context)?;
    let plan_files = context.plan["payload_files"]
        .as_array()
        .ok_or("plan payload_files array required")?;
    let mut result = Vec::new();
    for entry in plan_files {
        let id = text(entry, "file_ref")?;
        if file_ref.is_some_and(|selected| selected != id) {
            continue;
        }
        if entry["verified"] != true {
            return Err(format!("frozen plan marks File unverified: {id}"));
        }
        let relative = text(entry, "relative_path")?;
        let manifest_entry = manifest["payload_files"]
            .as_array()
            .ok_or("manifest File array required")?
            .iter()
            .find(|v| v["file_id"] == id)
            .ok_or("manifest File missing")?;
        let path = payload_path(context, source_root, layout, &manifest_path, relative)?;
        let size = unsigned(entry, "byte_size")?;
        let digest = text(entry, "sha256")?;
        verify_local_file(&path, size, digest, &id)?;
        result.push(VerifiedFile {
            item_ref: text(&context.plan, "item_ref")?.to_owned(),
            file_ref: id.to_owned(),
            relative_path: relative.to_owned(),
            local_path: path,
            byte_size: size,
            sha256: digest.to_owned(),
            media_type: manifest_entry
                .get("media_type")
                .and_then(Value::as_str)
                .unwrap_or("application/octet-stream")
                .to_owned(),
            manifest_ref: text(&context.plan["manifest"], "ref")?.to_owned(),
            manifest_sha256: text(&context.plan["manifest"], "sha256")?.to_owned(),
            rights_ref: text(&context.plan["rights_policy"], "rights_record_ref")?.to_owned(),
            rights_sha256: text(&context.plan["rights_policy"], "rights_record_sha256")?.to_owned(),
        });
    }
    if file_ref.is_some() && result.is_empty() {
        return Err("file_ref is not present in frozen plan".into());
    }
    Ok(result)
}

/// Verify an explicitly selected payload with bounded memory use.
fn verify_local_file(
    path: &Path,
    expected_size: u64,
    expected_sha256: &str,
    file_ref: &str,
) -> Result<()> {
    let mut input = tos_fd_open::open_absolute_regular(path, MAX_PAYLOAD)
        .map_err(|_| format!("payload is not a no-follow regular file: {file_ref}"))?;
    let before = input
        .metadata()
        .map_err(|_| "cannot inspect payload file")?;
    if before.len() != expected_size {
        return Err(format!("payload byte-size mismatch for {file_ref}"));
    }
    let mut hasher = Digest256Hasher::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 1024 * 1024];
    loop {
        let count = input
            .read(&mut buffer)
            .map_err(|_| "cannot read payload file")?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count as u64)
            .ok_or("payload size overflow")?;
        if total > expected_size {
            return Err(format!("payload grew while verifying {file_ref}"));
        }
        hasher.update(&buffer[..count]);
    }
    let after = input.metadata().map_err(|_| "cannot restat payload file")?;
    let current = fs::symlink_metadata(path).map_err(|_| "cannot restat payload path")?;
    let stamp = |m: &std::fs::Metadata| {
        (
            m.dev(),
            m.ino(),
            m.len(),
            m.mtime(),
            m.mtime_nsec(),
            m.ctime(),
            m.ctime_nsec(),
        )
    };
    if total != expected_size || stamp(&before) != stamp(&after) || stamp(&after) != stamp(&current)
    {
        return Err(format!("payload changed while verifying {file_ref}"));
    }
    if hasher.finalize().to_hex() != expected_sha256 {
        return Err(format!("payload SHA-256 mismatch for {file_ref}"));
    }
    Ok(())
}

fn instant() -> Result<String> {
    crate::source_serialization::instant().map_err(|e| format!("native clock: {e:?}"))
}

fn publication_id(item: &str, file: &str, rights: &str) -> String {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(item.as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(file.as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(rights.as_bytes());
    format!("pub-{}", &digest_hex(&bytes)[..32])
}

fn receipt_filename(context: &PlanContext, file: &VerifiedFile) -> String {
    let id = text(&context.plan, "server_import_id")
        .unwrap_or("import")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>();
    format!(
        "{}.{}.{}.receipt.json",
        id,
        &file.sha256[..16],
        &context.plan_sha256[..16]
    )
}

fn receipt(
    context: &PlanContext,
    file: &VerifiedFile,
    bucket_alias: &str,
    transfer: &crate::corpus_r2_cli::RawTransferReceiptV1,
) -> Result<Value> {
    let safe_id = text(&context.plan, "server_import_id")?
        .to_ascii_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>();
    let receipt_id = format!(
        "tos.receipt.server-import.{safe_id}.{}.",
        &file.sha256[..16]
    ) + &context.plan_sha256[..16];
    let mut identity = json!({
        "item_ref":file.item_ref,"file_ref":file.file_ref,"item_revision_sha256":file.manifest_sha256,
        "file_revision_sha256":file.sha256,"rights_record_ref":file.rights_ref,"rights_revision_sha256":file.rights_sha256
    });
    if text(&context.plan["rights_policy"], "review_status")? == "agent-reviewed" {
        let info = &context.plan["rights_policy"]["rights_review"];
        identity["rights_review_ref"] = json!(text(info, "ref")?);
        identity["rights_review_sha256"] = json!(text(info, "sha256")?);
        identity["rights_review_actor_kind"] = json!("model");
    }
    let value = json!({
        "$schema":"https://tree-of-sophia.local/ToS/contracts/server-import-receipt.schema.json",
        "schema_version":"tos_server_import_receipt_v1","receipt_id":receipt_id,"server_import_id":context.plan["server_import_id"],
        "plan":{"ref":context.plan_ref,"sha256":context.plan_sha256},"identity":identity,
        "source":{"relative_path":file.relative_path,"byte_size":file.byte_size,"sha256":file.sha256,"media_type":file.media_type},
        "storage":{"provider":"cloudflare-r2","bucket_alias":bucket_alias,"object_key":format!("blobs/sha256/{}/{}",&file.sha256[..2],file.sha256),"storage_class":"Standard","remote_status":transfer.remote_status},
        "verification":{"local_verified":true,"upload_attempted":transfer.upload_attempted,"readback_verified":transfer.readback_verified,"readback_sha256":transfer.sha256.to_hex(),"readback_byte_size":transfer.byte_size,"verified_at":instant()?},
        "publication":{"publication_id":publication_id(&file.item_ref,&file.file_ref,&file.rights_sha256),"status":"disabled","access_class":context.plan["access_class"],"rights_revision_sha256":file.rights_sha256}
    });
    validate_schema(&context.repo, RECEIPT_SCHEMA, &value)?;
    validate_receipt(&value)?;
    Ok(value)
}

fn validate_receipt(value: &Value) -> Result<()> {
    let source_sha = text(&value["source"], "sha256")?;
    if source_sha.len() != 64
        || !source_sha
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("receipt source SHA-256 is malformed".into());
    }
    if value["storage"]["object_key"] != format!("blobs/sha256/{}/{}", &source_sha[..2], source_sha)
    {
        return Err("receipt object key is not content-addressed".into());
    }
    if value["verification"]["readback_sha256"] != value["source"]["sha256"]
        || value["identity"]["file_revision_sha256"] != value["source"]["sha256"]
    {
        return Err("receipt source/readback digest mismatch".into());
    }
    if value["publication"]["rights_revision_sha256"] != value["identity"]["rights_revision_sha256"]
        || value["identity"]["item_revision_sha256"]
            .as_str()
            .is_none_or(str::is_empty)
    {
        return Err("receipt revision binding mismatch".into());
    }
    let review_fields = [
        "rights_review_ref",
        "rights_review_sha256",
        "rights_review_actor_kind",
    ];
    let present = review_fields
        .iter()
        .filter(|key| value["identity"].get(**key).is_some())
        .count();
    if present != 0 && present != review_fields.len() {
        return Err("receipt has incomplete rights-review identity".into());
    }
    Ok(())
}

fn load_receipt(path: &Path, repo: &Path) -> Result<Value> {
    let value = read_json(path)?;
    validate_schema(repo, RECEIPT_SCHEMA, &value)?;
    validate_receipt(&value)?;
    Ok(value)
}

fn validate_receipt_against_plan(context: &PlanContext, receipt: &Value) -> Result<()> {
    if receipt["plan"]["sha256"] != context.plan_sha256 {
        return Err("current plan digest differs from imported receipt".into());
    }
    let (manifest_path, manifest, rights_path, rights) = checked_plan_inputs(context)?;
    if manifest["item_id"] != receipt["identity"]["item_ref"] {
        return Err("current Item differs from receipt".into());
    }
    if context.plan["access_class"] != receipt["publication"]["access_class"] {
        return Err("current access class differs from receipt".into());
    }
    if context.plan["manifest"]["sha256"] != receipt["identity"]["item_revision_sha256"] {
        return Err("current manifest revision differs from receipt".into());
    }
    if context.plan["rights_policy"]["rights_record_sha256"]
        != receipt["identity"]["rights_revision_sha256"]
    {
        return Err("current rights revision differs from receipt".into());
    }
    let allowed = context.plan["payload_files"]
        .as_array()
        .ok_or("plan File array required")?
        .iter()
        .any(|v| v["file_ref"] == receipt["identity"]["file_ref"]);
    if !allowed {
        return Err("receipt File is outside the current plan".into());
    }
    validate_current_takedown_state(context, "source reads")?;
    let status = text(&context.plan["rights_policy"], "review_status")?;
    if status == "agent-reviewed" {
        let info = &context.plan["rights_policy"]["rights_review"];
        if receipt["identity"]["rights_review_ref"] != info["ref"]
            || receipt["identity"]["rights_review_sha256"] != info["sha256"]
            || receipt["identity"]["rights_review_actor_kind"] != "model"
        {
            return Err("current agent rights-review revision differs from receipt".into());
        }
        validate_agent_review(context, &manifest_path, &rights_path)?;
    } else if !["human-reviewed", "legal-reviewed"].contains(&status)
        || [
            "rights_review_ref",
            "rights_review_sha256",
            "rights_review_actor_kind",
        ]
        .iter()
        .any(|k| receipt["identity"].get(*k).is_some())
    {
        return Err("current legacy rights revision is not reviewed".into());
    }
    validate_rights(context, &rights)?;
    check_expiry(context.plan["rights_policy"].get("expires_at"))?;
    Ok(())
}

fn registry_default() -> Value {
    json!({"schema_version":"tos_source_publication_registry_v1","entries":[]})
}

fn registry_lock_path(path: &Path) -> Result<PathBuf> {
    let parent = path.parent().ok_or("registry parent missing")?;
    let leaf = path
        .file_name()
        .and_then(|v| v.to_str())
        .ok_or("registry filename must be UTF-8")?;
    Ok(parent.join(format!(".{leaf}.lock")))
}

fn registry_lock(path: &Path, operation: FlockOperation) -> Result<File> {
    lock_path_with(&registry_lock_path(path)?, operation)
}

fn load_registry_unlocked(path: &Path) -> Result<Value> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(registry_default());
        }
        Err(_) => return Err("cannot inspect publication registry".into()),
        Ok(_) => {}
    }
    let value = read_json(path)?;
    if value["schema_version"] != "tos_source_publication_registry_v1" {
        return Err("unsupported publication registry schema".into());
    }
    let entries = value["entries"]
        .as_array()
        .ok_or("publication registry entries must be an array")?;
    let mut publication_ids = BTreeSet::new();
    let mut receipt_ids = BTreeSet::new();
    for entry in entries {
        if !["enabled", "disabled", "revoked"].contains(&text(entry, "status")?) {
            return Err("publication registry status invalid".into());
        }
        for field in [
            "publication_id",
            "item_ref",
            "file_ref",
            "rights_revision_sha256",
            "receipt_id",
        ] {
            if text(entry, field)?.is_empty() {
                return Err(format!("publication registry {field} is empty"));
            }
        }
        if !publication_ids.insert(text(entry, "publication_id")?.to_owned())
            || !receipt_ids.insert(text(entry, "receipt_id")?.to_owned())
        {
            return Err("publication registry contains duplicate identities".into());
        }
    }
    Ok(value)
}

fn load_registry(path: &Path) -> Result<Value> {
    let (registry, _guard) = load_registry_with_guard(path)?;
    Ok(registry)
}

fn load_registry_with_guard(path: &Path) -> Result<(Value, File)> {
    let guard = registry_lock(path, FlockOperation::LockShared)?;
    let registry = load_registry_unlocked(path)?;
    Ok((registry, guard))
}

fn find_registry_entry<'a>(registry: &'a mut Value, selector: &str) -> Result<&'a mut Value> {
    registry["entries"]
        .as_array_mut()
        .ok_or("publication registry entries must be an array")?
        .iter_mut()
        .find(|e| e["publication_id"] == selector || e["receipt_id"] == selector)
        .ok_or_else(|| format!("publication is not registered: {selector}"))
}

fn enable_publication(receipt: &Value, registry_path: &Path, mode: &str) -> Result<Value> {
    if !["public-payload", "controlled-research"].contains(&mode) {
        return Err("publication mode is invalid".into());
    }
    if receipt["publication"]["status"] != "disabled"
        || receipt["identity"]["rights_revision_sha256"]
            != receipt["publication"]["rights_revision_sha256"]
        || receipt["identity"]["file_revision_sha256"] != receipt["source"]["sha256"]
    {
        return Err("only an exactly revision-bound disabled receipt can be enabled".into());
    }
    if receipt["publication"]["access_class"] != mode {
        return Err("publication mode differs from plan access class".into());
    }
    let _lock = registry_lock(registry_path, FlockOperation::LockExclusive)?;
    let mut registry = load_registry_unlocked(registry_path)?;
    let publication = text(&receipt["publication"], "publication_id")?.to_owned();
    let mut index = None;
    for (i, e) in registry["entries"]
        .as_array()
        .ok_or("registry entries missing")?
        .iter()
        .enumerate()
    {
        if e["publication_id"] == publication {
            index = Some(i);
            break;
        }
    }
    let now = instant()?;
    let entry = if let Some(i) = index {
        let entry = &mut registry["entries"][i];
        for (field, want) in [
            ("receipt_id", receipt["receipt_id"].clone()),
            ("item_ref", receipt["identity"]["item_ref"].clone()),
            ("file_ref", receipt["identity"]["file_ref"].clone()),
            (
                "file_revision_sha256",
                receipt["identity"]["file_revision_sha256"].clone(),
            ),
            (
                "rights_revision_sha256",
                receipt["identity"]["rights_revision_sha256"].clone(),
            ),
            ("mode", json!(mode)),
        ] {
            if entry[field] != want {
                return Err(
                    "publication registry conflicts with immutable receipt identity".into(),
                );
            }
        }
        if entry["status"] != "enabled" {
            if entry["status"] == "revoked" {
                return Err("revoked publication cannot be re-enabled".into());
            }
            entry["status"] = json!("enabled");
            entry["enabled_at"] = json!(now);
        }
        entry.clone()
    } else {
        let entry = json!({"publication_id":publication,"receipt_id":receipt["receipt_id"],"item_ref":receipt["identity"]["item_ref"],"file_ref":receipt["identity"]["file_ref"],"file_revision_sha256":receipt["identity"]["file_revision_sha256"],"rights_revision_sha256":receipt["identity"]["rights_revision_sha256"],"mode":mode,"status":"enabled","enabled_at":now});
        registry["entries"]
            .as_array_mut()
            .unwrap()
            .push(entry.clone());
        entry
    };
    write_atomic(registry_path, &registry)?;
    Ok(entry)
}

fn disable_publication(
    registry_path: &Path,
    selector: &str,
    reason: &str,
    revoked: bool,
) -> Result<Value> {
    if reason.trim().is_empty() {
        return Err("disable or revoke requires a reason".into());
    }
    let _lock = registry_lock(registry_path, FlockOperation::LockExclusive)?;
    let mut registry = load_registry_unlocked(registry_path)?;
    let entry = find_registry_entry(&mut registry, selector)?;
    if entry["status"] == "revoked" && !revoked {
        return Err("revoked publication cannot be downgraded to disabled".into());
    }
    entry["status"] = json!(if revoked { "revoked" } else { "disabled" });
    entry["disabled_at"] = json!(instant()?);
    entry["reason"] = json!(reason);
    let result = entry.clone();
    write_atomic(registry_path, &registry)?;
    Ok(result)
}

fn require_enabled_in_registry(registry: &Value, selector: &str, receipt: &Value) -> Result<()> {
    let entry = registry["entries"]
        .as_array()
        .ok_or("registry entries missing")?
        .iter()
        .find(|entry| entry["publication_id"] == selector || entry["receipt_id"] == selector)
        .ok_or_else(|| format!("publication is not registered: {selector}"))?;
    if entry["status"] != "enabled" {
        return Err(format!(
            "publication is {}",
            entry["status"].as_str().unwrap_or("invalid")
        ));
    }
    for (field, want) in [
        ("receipt_id", receipt["receipt_id"].clone()),
        ("item_ref", receipt["identity"]["item_ref"].clone()),
        ("file_ref", receipt["identity"]["file_ref"].clone()),
        (
            "rights_revision_sha256",
            receipt["identity"]["rights_revision_sha256"].clone(),
        ),
    ] {
        if entry[field] != want {
            return Err("publication registry receipt or identity mismatch".into());
        }
    }
    Ok(())
}

fn publish_local(source: &Path, output: &Path, size: u64, sha: &str) -> Result<()> {
    if fs::symlink_metadata(output).is_ok() {
        return Err("read output already exists; refusing overwrite".into());
    }
    let parent = output.parent().ok_or("read output parent missing")?;
    fs::create_dir_all(parent).map_err(|_| "cannot create read output parent")?;
    let directory = tos_fd_open::open_absolute_directory(parent)
        .map_err(|_| "read output parent must be no-follow")?;
    let leaf = output
        .file_name()
        .and_then(|v| v.to_str())
        .ok_or("read output filename must be UTF-8")?;
    let tmp = format!(
        ".{leaf}.{}.{}.tmp",
        std::process::id(),
        TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    let descriptor = rustix::fs::openat(
        &directory,
        tmp.as_str(),
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o600),
    )
    .map_err(|_| "cannot create read output temporary")?;
    let result = (|| {
        let mut dst = File::from(descriptor);
        let mut src = tos_fd_open::open_absolute_regular(source, MAX_PAYLOAD)
            .map_err(|_| "read source must be a no-follow regular file")?;
        let mut hasher = Digest256Hasher::new();
        let mut total = 0u64;
        let mut buf = [0u8; 1024 * 1024];
        loop {
            let n = src.read(&mut buf).map_err(|_| "cannot read source bytes")?;
            if n == 0 {
                break;
            }
            total = total.checked_add(n as u64).ok_or("output size overflow")?;
            if total > size {
                return Err("source grew during read".into());
            }
            hasher.update(&buf[..n]);
            dst.write_all(&buf[..n])
                .map_err(|_| "cannot write read output")?;
        }
        dst.sync_all().map_err(|_| "cannot sync read output")?;
        if total != size || hasher.finalize().to_hex() != sha {
            return Err("source changed or failed readback verification".into());
        }
        rustix::fs::linkat(&directory, tmp.as_str(), &directory, leaf, AtFlags::empty())
            .map_err(|_| "read output appeared; refusing overwrite")?;
        directory
            .sync_all()
            .map_err(|_| "cannot sync read output directory")?;
        Ok(())
    })();
    let _ = rustix::fs::unlinkat(&directory, tmp.as_str(), AtFlags::empty());
    result
}

fn build_transport(request: &Value) -> Result<R2TransportV1> {
    let spec = request
        .get("transport")
        .ok_or("transport configuration required")?;
    strict_object(
        spec,
        &["kind", "bucket", "wrangler"],
        &["account_id", "cwd", "timeout_seconds", "max_upload_bytes"],
    )?;
    let kind = match text(spec, "kind")? {
        "wrangler" => R2TransportKindV1::Wrangler,
        "rest" => R2TransportKindV1::Rest,
        _ => return Err("transport.kind must be wrangler or rest".into()),
    };
    let wrangler = absolute_path(spec, "wrangler")?;
    let cwd = optional_text(spec, "cwd")?.map(PathBuf::from);
    if cwd.as_ref().is_some_and(|p| !p.is_absolute()) {
        return Err("transport.cwd must be absolute".into());
    }
    let timeout = spec
        .get("timeout_seconds")
        .and_then(Value::as_u64)
        .unwrap_or(120);
    let mut transport = R2TransportV1::new(
        kind,
        text(spec, "bucket")?,
        optional_text(spec, "account_id")?,
        &wrangler,
        cwd.as_deref(),
        timeout,
    )?;
    if let Some(limit) = spec.get("max_upload_bytes") {
        transport = transport.with_max_upload_bytes(
            limit
                .as_u64()
                .ok_or("max_upload_bytes must be a nonnegative integer")?,
        )?;
    }
    Ok(transport)
}

fn request_context(request: &Value) -> Result<(PlanContext, PathBuf, String, Option<String>)> {
    let repo = absolute_path(request, "repo_root")?;
    let plan = PathBuf::from(text(request, "plan")?);
    let context = load_plan(&repo, &plan)?;
    let source_root = absolute_path(request, "payload_source_root")?;
    let layout = optional_text(request, "payload_source_layout")?
        .unwrap_or("source-witness")
        .to_owned();
    let file_ref = optional_text(request, "file_ref")?.map(str::to_owned);
    Ok((context, source_root, layout, file_ref))
}

fn operation_import(request: &Value) -> Result<Value> {
    strict_object(
        request,
        &[
            "schema_version",
            "family",
            "operation",
            "repo_root",
            "plan",
            "payload_source_root",
            "receipt_dir",
            "scratch_root",
            "transport",
            "confirm_transfer",
        ],
        &["payload_source_layout", "file_ref", "bucket_alias"],
    )?;
    if request["confirm_transfer"] != true {
        return Err("import requires confirm_transfer=true; verify-local is read-only".into());
    }
    let (context, source_root, layout, file_ref) = request_context(request)?;
    let receipt_dir = absolute_path(request, "receipt_dir")?;
    let scratch = absolute_path(request, "scratch_root")?;
    let bucket_alias = optional_text(request, "bucket_alias")?.unwrap_or("private-source-payloads");
    let mut transport = build_transport(request)?;
    let outcomes = import_context(
        &context,
        &source_root,
        &layout,
        &mut transport,
        &receipt_dir,
        bucket_alias,
        &scratch,
        file_ref.as_deref(),
    )?;
    Ok(json!({"status":"imported","receipts":outcomes}))
}

fn operation_read_local(request: &Value) -> Result<Value> {
    strict_object(
        request,
        &[
            "schema_version",
            "family",
            "operation",
            "repo_root",
            "plan",
            "receipt",
            "registry",
            "payload_source_root",
            "output",
        ],
        &["payload_source_layout"],
    )?;
    let repo = absolute_path(request, "repo_root")?;
    let context = load_plan(&repo, Path::new(text(request, "plan")?))?;
    let receipt = load_receipt(&absolute_path(request, "receipt")?, &repo)?;
    let registry = absolute_path(request, "registry")?;
    // Serialize disable/revoke against the complete local verified publication.
    let (registry_snapshot, _registry_guard) = load_registry_with_guard(&registry)?;
    require_enabled_in_registry(
        &registry_snapshot,
        text(&receipt["publication"], "publication_id")?,
        &receipt,
    )?;
    validate_receipt_against_plan(&context, &receipt)?;
    let root = absolute_path(request, "payload_source_root")?;
    let layout = optional_text(request, "payload_source_layout")?.unwrap_or("source-witness");
    let file_ref = text(&receipt["identity"], "file_ref")?;
    let files = verify_local(&context, &root, layout, Some(file_ref))?;
    let file = files
        .iter()
        .find(|f| f.sha256 == receipt["source"]["sha256"].as_str().unwrap_or_default())
        .ok_or("enabled publication does not resolve to receipt bytes")?;
    let output = absolute_path(request, "output")?;
    publish_local(&file.local_path, &output, file.byte_size, &file.sha256)?;
    Ok(json!({"status":"read-local-verified","receipt_id":receipt["receipt_id"]}))
}

fn operation_read_remote(request: &Value) -> Result<Value> {
    operation_read_remote_with_transport(request, build_transport)
}

fn operation_read_remote_with_transport<T, F>(
    request: &Value,
    transport_factory: F,
) -> Result<Value>
where
    T: ChunkedFileTransportV1,
    F: FnOnce(&Value) -> Result<T>,
{
    strict_object(
        request,
        &[
            "schema_version",
            "family",
            "operation",
            "repo_root",
            "plan",
            "receipt",
            "registry",
            "scratch_root",
            "transport",
            "output",
        ],
        &["publication"],
    )?;
    let repo = absolute_path(request, "repo_root")?;
    let receipt = load_receipt(&absolute_path(request, "receipt")?, &repo)?;
    let plan = Path::new(text(request, "plan")?);
    let context = load_plan(&repo, plan)?;
    validate_receipt_against_plan(&context, &receipt)?;
    let registry = absolute_path(request, "registry")?;
    let selector = optional_text(request, "publication")?
        .unwrap_or(text(&receipt["publication"], "publication_id")?);
    // Hold admission stable through remote readback and local output publication.
    // This does not delete the provider object or cancel an already-started fetch.
    let (registry_snapshot, _registry_guard) = load_registry_with_guard(&registry)?;
    require_enabled_in_registry(&registry_snapshot, selector, &receipt)?;
    let mut transport = transport_factory(request)?;
    raw_read_verified_v1(
        &mut transport,
        text(&receipt["storage"], "object_key")?,
        &absolute_path(request, "output")?,
        unsigned(&receipt["source"], "byte_size")?,
        Digest256::from_hex(text(&receipt["source"], "sha256")?)
            .map_err(|_| "receipt SHA-256 invalid")?,
        &absolute_path(request, "scratch_root")?,
    )?;
    Ok(json!({"status":"read-remote-verified","receipt_id":receipt["receipt_id"]}))
}

fn batch_digest(value: &str) -> Result<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("expected_plans_sha256 must be lowercase SHA-256".into());
    }
    Ok(())
}

fn append_journal(file: &mut File, value: &Value) -> Result<()> {
    serde_json::to_writer(&mut *file, value).map_err(|e| e.to_string())?;
    file.write_all(b"\n").map_err(|e| e.to_string())?;
    file.sync_all().map_err(|_| "cannot sync batch journal")?;
    Ok(())
}

fn operation_batch(request: &Value) -> Result<Value> {
    operation_batch_with_transport(request, build_transport)
}

fn batch_failure_type(message: &str) -> &'static str {
    if message.starts_with("frozen plans")
        || message.starts_with("plan changed during batch")
        || message.starts_with("batch ")
    {
        "BatchError"
    } else if message.contains("Wrangler") || message.contains("Cloudflare REST") {
        "TransportError"
    } else if message.contains("R2")
        || message.contains("remote")
        || message.contains("readback")
        || message.contains("object is unavailable")
    {
        "RemoteIntegrityError"
    } else if message.contains("rights")
        || message.contains("review")
        || message.contains("transfer")
    {
        "RightsGateError"
    } else if message.contains("payload") || message.contains("local") {
        "LocalIntegrityError"
    } else if message.contains("receipt") || message.contains("publication") {
        "ReceiptConflict"
    } else {
        "NativeImportError"
    }
}

fn operation_batch_with_transport<T, F>(request: &Value, transport_factory: F) -> Result<Value>
where
    T: ChunkedFileTransportV1,
    F: FnOnce(&Value) -> Result<T>,
{
    strict_object(
        request,
        &[
            "schema_version",
            "family",
            "operation",
            "repo_root",
            "plans_file",
            "expected_plans_sha256",
            "payload_source_root",
            "scratch_root",
            "receipt_dir",
            "output",
            "transport",
            "transfer",
        ],
        &["payload_source_layout", "bucket_alias"],
    )?;
    let transfer = request["transfer"]
        .as_bool()
        .ok_or("transfer must be boolean")?;
    let repo = absolute_path(request, "repo_root")?;
    let plans_path = absolute_path(request, "plans_file")?;
    let plans_sha = text(request, "expected_plans_sha256")?;
    batch_digest(plans_sha)?;
    let raw = io_owner::read_bytes(&plans_path, None, false, false, MAX_JSON)?;
    if digest_hex(&raw) != plans_sha {
        return Err("frozen plans file SHA-256 differs".into());
    }
    let index = io_owner::parse(&raw)?;
    if index
        .as_object()
        .is_none_or(|o| o.len() != 1 || !o.contains_key("plans"))
    {
        return Err("frozen plans file must contain exactly plans".into());
    }
    let entries = index["plans"]
        .as_array()
        .filter(|a| !a.is_empty())
        .ok_or("frozen plans file needs a nonempty plans array")?;
    let mut seen = BTreeSet::new();
    let root = absolute_path(request, "payload_source_root")?;
    let layout = optional_text(request, "payload_source_layout")?.unwrap_or("source-witness");
    let mut prepared = Vec::new();
    let mut total_files = 0u64;
    let mut total_bytes = 0u64;
    for entry in entries {
        strict_object(entry, &["ref", "sha256"], &[])?;
        let reference = text(entry, "ref")?;
        let expected = text(entry, "sha256")?;
        batch_digest(expected)?;
        if !seen.insert(reference.to_owned()) {
            return Err("frozen plans file contains duplicate plan ref".into());
        }
        let path = io_owner::path_under(&repo, reference)?;
        if digest_file(&path)? != expected {
            return Err(format!("frozen plan digest differs: {reference}"));
        }
        let context = load_plan(&repo, &path)?;
        if context.plan_ref != reference || context.plan_sha256 != expected {
            return Err(format!(
                "loaded plan differs from frozen index: {reference}"
            ));
        }
        enforce_transfer(&context)?;
        let files = verify_local(&context, &root, layout, None)?;
        total_files = total_files
            .checked_add(files.len() as u64)
            .ok_or("batch file count overflow")?;
        for file in &files {
            total_bytes = total_bytes
                .checked_add(file.byte_size)
                .ok_or("batch byte count overflow")?;
        }
        prepared.push((context, files.len() as u64));
    }
    let output_dir = absolute_path(request, "output")?;
    fs::create_dir_all(&output_dir).map_err(|_| "cannot create batch output directory")?;
    tos_fd_open::open_absolute_directory(&output_dir)
        .map_err(|_| "batch output must be no-follow directory")?;
    let run = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "clock before Unix epoch")?
        .as_nanos();
    let prefix = format!(
        "batch-{run}-{}",
        TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    let summary_path = output_dir.join(format!("{prefix}.json"));
    let journal_path = output_dir.join(format!("{prefix}.jsonl"));
    let mut journal = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&journal_path)
        .map_err(|_| "cannot create batch journal")?;
    let mut summary = json!({"schema":"tos_source_payload_batch_summary_v1","started_at":instant()?,"plans_file_sha256":plans_sha,"plans":prepared.len(),"files":total_files,"bytes":total_bytes,"transfer_requested":transfer,"phase":if transfer{"transfer"}else{"verify-only"},"complete":!transfer,"completed_plans":0,"completed_files":0,"local_verified_files":total_files,"uploaded_files":0,"reused_receipts":0,"journal":journal_path.file_name().unwrap().to_string_lossy()});
    write_atomic(&summary_path, &summary)?;
    let mut transport_factory = Some(transport_factory);
    let transfer_result = (|| -> Result<()> {
        if !transfer {
            return Ok(());
        }
        let factory = transport_factory
            .take()
            .ok_or("batch transport factory was already consumed")?;
        let mut transport = factory(request)?;
        let receipts = absolute_path(request, "receipt_dir")?;
        let scratch = absolute_path(request, "scratch_root")?;
        let alias = optional_text(request, "bucket_alias")?
            .unwrap_or(text(&request["transport"], "bucket")?);
        for (context, expected_files) in &prepared {
            if digest_file(&context.path)? != context.plan_sha256 {
                return Err(format!("plan changed during batch: {}", context.plan_ref));
            }
            let mut completed_this_plan = 0u64;
            import_context_with_progress(
                context,
                &root,
                layout,
                &mut transport,
                &receipts,
                alias,
                &scratch,
                None,
                |outcome| {
                    append_journal(
                        &mut journal,
                        &json!({
                            "plan_ref":context.plan_ref,
                            "plan_sha256":context.plan_sha256,
                            "completed_at":instant()?,
                            "files":1,
                            "uploaded_files":if outcome["upload_attempted"] == true { 1u64 } else { 0 },
                            "reused_receipts":if outcome["receipt_reused"] == true { 1u64 } else { 0 },
                            "outcome":outcome
                        }),
                    )?;
                    summary["completed_files"] =
                        json!(summary["completed_files"].as_u64().unwrap_or(0) + 1);
                    summary["uploaded_files"] = json!(
                        summary["uploaded_files"].as_u64().unwrap_or(0)
                            + if outcome["upload_attempted"] == true {
                                1u64
                            } else {
                                0
                            }
                    );
                    summary["reused_receipts"] = json!(
                        summary["reused_receipts"].as_u64().unwrap_or(0)
                            + if outcome["receipt_reused"] == true {
                                1u64
                            } else {
                                0
                            }
                    );
                    completed_this_plan += 1;
                    write_atomic(&summary_path, &summary)
                },
            )?;
            if completed_this_plan != *expected_files {
                return Err("batch file count changed after preflight".into());
            }
            summary["completed_plans"] =
                json!(summary["completed_plans"].as_u64().unwrap_or(0) + 1);
            write_atomic(&summary_path, &summary)?;
        }
        summary["complete"] = json!(true);
        Ok(())
    })();
    if let Err(error) = transfer_result {
        summary["complete"] = json!(false);
        summary["phase"] = json!("failed");
        summary["failure_type"] = json!(batch_failure_type(&error));
        summary["failure_message"] = json!(error);
        summary["finished_at"] = json!(instant()?);
        write_atomic(&summary_path, &summary)?;
        return Err(format!(
            "batch failed; journal {} and summary {} retained",
            journal_path.file_name().unwrap().to_string_lossy(),
            summary_path.file_name().unwrap().to_string_lossy()
        ));
    }
    summary["finished_at"] = json!(instant()?);
    write_atomic(&summary_path, &summary)?;
    Ok(
        json!({"status":if transfer{"transferred"}else{"verified-only"},"summary":summary_path.file_name().unwrap().to_string_lossy(),"journal":journal_path.file_name().unwrap().to_string_lossy(),"plans":summary["plans"],"files":summary["files"],"complete":summary["complete"]}),
    )
}

/// Invoke one strict source-payload owner request.
pub fn invoke(request: &Value) -> Result<Value> {
    if text(request, "schema_version")? != "tos_source_payload_import_request_v1" {
        return Err("unsupported source-payload request schema".into());
    }
    if let Some(family) = request.get("family").and_then(Value::as_str) {
        if family != "payload-import" {
            return Err("unsupported source-payload request family".into());
        }
    }
    match text(request, "operation")? {
        "verify-local" => {
            strict_object(
                request,
                &[
                    "schema_version",
                    "family",
                    "operation",
                    "repo_root",
                    "plan",
                    "payload_source_root",
                ],
                &["payload_source_layout", "file_ref"],
            )?;
            let (context, root, layout, file_ref) = request_context(request)?;
            let files = verify_local(&context, &root, &layout, file_ref.as_deref())?;
            Ok(
                json!({"status":"verified-local","server_import_id":context.plan["server_import_id"],"plan_sha256":context.plan_sha256,"files":files.iter().map(|f|json!({"item_ref":f.item_ref,"file_ref":f.file_ref,"relative_path":f.relative_path,"byte_size":f.byte_size,"sha256":f.sha256})).collect::<Vec<_>>()}),
            )
        }
        "import" => operation_import(request),
        "read-local" => operation_read_local(request),
        "read-remote" => operation_read_remote(request),
        "registry-enable" => {
            strict_object(
                request,
                &[
                    "schema_version",
                    "family",
                    "operation",
                    "repo_root",
                    "receipt",
                    "registry",
                    "mode",
                ],
                &[],
            )?;
            let repo = absolute_path(request, "repo_root")?;
            let value = load_receipt(&absolute_path(request, "receipt")?, &repo)?;
            enable_publication(
                &value,
                &absolute_path(request, "registry")?,
                text(request, "mode")?,
            )
        }
        "registry-disable" => {
            strict_object(
                request,
                &[
                    "schema_version",
                    "family",
                    "operation",
                    "registry",
                    "publication",
                    "reason",
                ],
                &["revoke"],
            )?;
            let revoke = request
                .get("revoke")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            disable_publication(
                &absolute_path(request, "registry")?,
                text(request, "publication")?,
                text(request, "reason")?,
                revoke,
            )
        }
        "batch" => operation_batch(request),
        _ => Err("unsupported source-payload operation".into()),
    }
}

/// Standalone user CLI: one strict JSON request on stdin, one JSON result.
pub fn run() -> i32 {
    let result = (|| {
        let mut raw = Vec::new();
        std::io::stdin()
            .lock()
            .take(MAX_JSON + 1)
            .read_to_end(&mut raw)
            .map_err(|_| "cannot read request")?;
        if raw.len() as u64 > MAX_JSON {
            return Err("source-payload request exceeds byte budget".into());
        }
        let request = io_owner::parse(&raw)?;
        let mut request = request;
        if request.get("family").is_none() {
            request["family"] = json!("payload-import");
        }
        invoke(&request)
    })();
    let (code, response) = match result {
        Ok(v) => (0, json!({"kind":"result","value":v})),
        Err(e) => (
            2,
            json!({"kind":"error","error":"SourcePayloadImportError","message":e}),
        ),
    };
    match serde_json::to_writer(&mut std::io::stdout().lock(), &response) {
        Ok(()) => {
            let _ = std::io::stdout().lock().write_all(b"\n");
            code
        }
        Err(_) => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rfc3339_expiry_parser_honors_offsets_and_leap_dates() {
        assert_eq!(
            parse_rfc3339("2024-02-29T00:00:00Z"),
            Some(parse_rfc3339("2024-02-29T01:00:00+01:00").unwrap())
        );
        assert!(parse_rfc3339("2023-02-29T00:00:00Z").is_none());
        assert!(parse_rfc3339("2024-01-01T00:00:00").is_none());
    }

    #[test]
    fn publication_identifier_is_stable_and_domain_separated() {
        let a = publication_id("tos.item.a", "tos.file.b", &"c".repeat(64));
        assert_eq!(
            a,
            publication_id("tos.item.a", "tos.file.b", &"c".repeat(64))
        );
        assert_ne!(
            a,
            publication_id("tos.item.a", "tos.file.c", &"c".repeat(64))
        );
    }
}

fn receipt_matches(value: &Value, context: &PlanContext, file: &VerifiedFile) -> Result<bool> {
    validate_schema(&context.repo, RECEIPT_SCHEMA, value)?;
    validate_receipt(value)?;
    let review_ok = if text(&context.plan["rights_policy"], "review_status")? == "agent-reviewed" {
        let info = &context.plan["rights_policy"]["rights_review"];
        value["identity"]["rights_review_ref"] == info["ref"]
            && value["identity"]["rights_review_sha256"] == info["sha256"]
            && value["identity"]["rights_review_actor_kind"] == "model"
    } else {
        [
            "rights_review_ref",
            "rights_review_sha256",
            "rights_review_actor_kind",
        ]
        .iter()
        .all(|key| value["identity"].get(*key).is_none())
    };
    Ok(review_ok
        && value["server_import_id"] == context.plan["server_import_id"]
        && value["plan"]["sha256"] == context.plan_sha256
        && value["identity"]["item_ref"] == file.item_ref
        && value["identity"]["file_ref"] == file.file_ref
        && value["identity"]["item_revision_sha256"] == file.manifest_sha256
        && value["identity"]["file_revision_sha256"] == file.sha256
        && value["identity"]["rights_revision_sha256"] == file.rights_sha256
        && value["storage"]["object_key"]
            == format!("blobs/sha256/{}/{}", &file.sha256[..2], file.sha256)
        && value["verification"]["readback_verified"] == true)
}

fn json_pretty(value: &Value) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn lock_path_with(path: &Path, operation: FlockOperation) -> Result<File> {
    let parent = path.parent().ok_or("lock parent missing")?;
    fs::create_dir_all(parent).map_err(|_| "cannot create local coordination directory")?;
    let directory = tos_fd_open::open_absolute_directory(parent)
        .map_err(|_| "coordination directory must be no-follow")?;
    let leaf = path
        .file_name()
        .and_then(|v| v.to_str())
        .ok_or("lock filename must be UTF-8")?;
    let descriptor = rustix::fs::openat(
        &directory,
        leaf,
        OFlags::RDWR | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o600),
    )
    .map_err(|_| "cannot open coordination lock")?;
    let file = File::from(descriptor);
    rustix::fs::flock(&file, operation).map_err(|_| "cannot acquire coordination lock")?;
    Ok(file)
}

fn lock_path(path: &Path) -> Result<File> {
    lock_path_with(path, FlockOperation::LockExclusive)
}

fn write_atomic(path: &Path, value: &Value) -> Result<()> {
    let parent = path.parent().ok_or("output parent missing")?;
    fs::create_dir_all(parent).map_err(|_| "cannot create output parent")?;
    let directory = tos_fd_open::open_absolute_directory(parent)
        .map_err(|_| "output parent must be no-follow")?;
    let leaf = path
        .file_name()
        .and_then(|v| v.to_str())
        .ok_or("output filename must be UTF-8")?;
    let bytes = json_pretty(value)?;
    if fs::symlink_metadata(path).is_ok() {
        io_owner::read_bytes(path, None, false, false, MAX_JSON)?;
    }
    let tmp = format!(
        ".{leaf}.{}.{}.tmp",
        std::process::id(),
        TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    let descriptor = rustix::fs::openat(
        &directory,
        tmp.as_str(),
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o600),
    )
    .map_err(|_| "cannot create atomic output")?;
    let result = (|| {
        let mut file = File::from(descriptor);
        file.write_all(&bytes)
            .map_err(|_| "cannot write atomic output")?;
        file.sync_all().map_err(|_| "cannot sync atomic output")?;
        rustix::fs::renameat(&directory, tmp.as_str(), &directory, leaf)
            .map_err(|_| "cannot publish atomic output")?;
        directory
            .sync_all()
            .map_err(|_| "cannot sync output directory")?;
        Ok(())
    })();
    if result.is_err() {
        let _ = rustix::fs::unlinkat(&directory, tmp.as_str(), AtFlags::empty());
    }
    result
}

fn create_snapshot(
    source: &Path,
    scratch: &Path,
    expected_size: u64,
    expected_sha: &str,
) -> Result<PathBuf> {
    if expected_size > MAX_PAYLOAD {
        return Err("payload exceeds 300 MiB single-object limit".into());
    }
    fs::create_dir_all(scratch).map_err(|_| "cannot create managed scratch root")?;
    let _scratch = tos_fd_open::open_absolute_directory(scratch)
        .map_err(|_| "scratch root must be no-follow")?;
    let token = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let path = scratch.join(format!(
        ".tos-source-snapshot-{}-{token}.bin",
        std::process::id()
    ));
    let mut input = tos_fd_open::open_absolute_regular(source, MAX_PAYLOAD)
        .map_err(|_| "payload source must be a no-follow regular file")?;
    let before = input
        .metadata()
        .map_err(|_| "cannot inspect payload source")?;
    if before.len() != expected_size {
        return Err("payload byte-size differs from frozen plan".into());
    }
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&path)
        .map_err(|_| "cannot create payload snapshot")?;
    let mut hasher = Digest256Hasher::new();
    let mut buffer = [0u8; 1024 * 1024];
    let mut total = 0u64;
    let result: Result<()> = (|| {
        loop {
            let n = input
                .read(&mut buffer)
                .map_err(|_| "cannot read payload source")?;
            if n == 0 {
                break;
            }
            total = total.checked_add(n as u64).ok_or("payload size overflow")?;
            if total > expected_size {
                return Err("payload changed during snapshot".into());
            }
            hasher.update(&buffer[..n]);
            output
                .write_all(&buffer[..n])
                .map_err(|_| "cannot write payload snapshot")?;
        }
        output
            .sync_all()
            .map_err(|_| "cannot sync payload snapshot")?;
        let after = input
            .metadata()
            .map_err(|_| "cannot restat payload source")?;
        let current =
            fs::symlink_metadata(source).map_err(|_| "cannot restat payload source path")?;
        let stamp = |m: &std::fs::Metadata| {
            (
                m.dev(),
                m.ino(),
                m.len(),
                m.mtime(),
                m.mtime_nsec(),
                m.ctime(),
                m.ctime_nsec(),
            )
        };
        if total != expected_size
            || digest_hex_from_hasher(hasher) != expected_sha
            || stamp(&before) != stamp(&after)
            || stamp(&after) != stamp(&current)
        {
            return Err("payload changed or failed SHA-256 during snapshot".into());
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&path);
    }
    result?;
    Ok(path)
}

/// Removes the unique payload snapshot on every exit from one file's import.
struct RemoveOnDrop(PathBuf);

impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn digest_hex_from_hasher(hasher: Digest256Hasher) -> String {
    hasher.finalize().to_hex()
}

fn import_context<T: ChunkedFileTransportV1>(
    context: &PlanContext,
    source_root: &Path,
    layout: &str,
    transport: &mut T,
    receipt_dir: &Path,
    bucket_alias: &str,
    scratch: &Path,
    file_ref: Option<&str>,
) -> Result<Vec<Value>> {
    import_context_with_progress(
        context,
        source_root,
        layout,
        transport,
        receipt_dir,
        bucket_alias,
        scratch,
        file_ref,
        |_| Ok(()),
    )
}

fn import_context_with_progress<T, P>(
    context: &PlanContext,
    source_root: &Path,
    layout: &str,
    transport: &mut T,
    receipt_dir: &Path,
    bucket_alias: &str,
    scratch: &Path,
    file_ref: Option<&str>,
    mut on_file: P,
) -> Result<Vec<Value>>
where
    T: ChunkedFileTransportV1,
    P: FnMut(&Value) -> Result<()>,
{
    enforce_transfer(context)?;
    let files = verify_local(context, source_root, layout, file_ref)?;
    fs::create_dir_all(receipt_dir).map_err(|_| "cannot create receipt directory")?;
    let mut outputs = Vec::new();
    for file in files {
        let lock = lock_path(&receipt_dir.join(".source-payload-import.lock"))?;
        let snapshot = RemoveOnDrop(create_snapshot(
            &file.local_path,
            scratch,
            file.byte_size,
            &file.sha256,
        )?);
        let sha = Digest256::from_hex(&file.sha256).map_err(|_| "invalid planned SHA-256")?;
        let transfer = raw_transfer_v1(
            transport,
            &format!("blobs/sha256/{}/{}", &file.sha256[..2], file.sha256),
            &snapshot.0,
            file.byte_size,
            sha,
            &file.media_type,
            scratch,
        )?;
        let value = receipt(context, &file, bucket_alias, &transfer)?;
        let path = receipt_dir.join(receipt_filename(context, &file));
        let reused = if fs::symlink_metadata(&path).is_ok() {
            let existing = read_json(&path)?;
            if !receipt_matches(&existing, context, &file)? {
                return Err(format!(
                    "immutable receipt binding conflict: {}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                ));
            }
            true
        } else {
            let bytes = json_pretty(&value)?;
            match io_owner::publish(&path, &bytes, 0o600) {
                Ok("copied") => false,
                Ok("already_present") => {
                    let existing = read_json(&path)?;
                    if !receipt_matches(&existing, context, &file)? {
                        return Err("immutable receipt race changed binding".into());
                    }
                    true
                }
                _ => return Err("immutable receipt publication failed".into()),
            }
        };
        drop(lock);
        let outcome = json!({"file_ref":file.file_ref,"receipt":path.file_name().unwrap_or_default().to_string_lossy(),"remote_status":transfer.remote_status,"upload_attempted":transfer.upload_attempted,"receipt_reused":reused});
        on_file(&outcome)?;
        outputs.push(outcome);
    }
    Ok(outputs)
}

#[cfg(test)]
mod transfer_tests {
    use super::*;
    use std::collections::HashMap;
    use std::io;
    use tempfile::TempDir;

    struct MemoryTransport {
        objects: HashMap<String, Vec<u8>>,
        puts: usize,
        mutate_after_put: Option<(PathBuf, Vec<u8>)>,
        mutate_on_fetch: Option<(PathBuf, Vec<u8>)>,
    }
    impl ChunkedFileTransportV1 for MemoryTransport {
        fn fetch(&mut self, key: &str, destination: &Path, _max_bytes: u64) -> io::Result<bool> {
            if let Some((path, bytes)) = self.mutate_on_fetch.take() {
                fs::write(path, bytes)?;
            }
            if let Some(bytes) = self.objects.get(key) {
                let mut out = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(destination)?;
                out.write_all(bytes)?;
                Ok(true)
            } else {
                Ok(false)
            }
        }
        fn put(
            &mut self,
            key: &str,
            source: &Path,
            _size: u64,
            _media: &str,
            _storage: &str,
        ) -> io::Result<()> {
            if self.objects.contains_key(key) {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "overwrite refused",
                ));
            }
            self.objects.insert(key.to_owned(), fs::read(source)?);
            self.puts += 1;
            if let Some((path, bytes)) = self.mutate_after_put.take() {
                fs::write(path, bytes)?;
            }
            Ok(())
        }
    }

    struct GatedReadTransport {
        body: Vec<u8>,
        fetch_started: std::sync::mpsc::SyncSender<()>,
        resume_fetch: std::sync::mpsc::Receiver<()>,
    }

    impl ChunkedFileTransportV1 for GatedReadTransport {
        fn fetch(&mut self, _key: &str, destination: &Path, max_bytes: u64) -> io::Result<bool> {
            if self.body.len() as u64 > max_bytes {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "fixture exceeds requested fetch bound",
                ));
            }
            self.fetch_started
                .send(())
                .map_err(|_| io::Error::other("test coordinator stopped"))?;
            self.resume_fetch
                .recv()
                .map_err(|_| io::Error::other("test coordinator stopped"))?;
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(destination)?;
            output.write_all(&self.body)?;
            Ok(true)
        }

        fn put(
            &mut self,
            _key: &str,
            _source: &Path,
            _size: u64,
            _media: &str,
            _storage: &str,
        ) -> io::Result<()> {
            Err(io::Error::other("read-only test transport"))
        }
    }

    #[derive(Default)]
    struct FailAfterOneTransport {
        objects: HashMap<String, Vec<u8>>,
        successful_puts: usize,
    }
    impl ChunkedFileTransportV1 for FailAfterOneTransport {
        fn fetch(&mut self, key: &str, destination: &Path, _max_bytes: u64) -> io::Result<bool> {
            if let Some(bytes) = self.objects.get(key) {
                let mut output = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(destination)?;
                output.write_all(bytes)?;
                Ok(true)
            } else {
                Ok(false)
            }
        }

        fn put(
            &mut self,
            key: &str,
            source: &Path,
            _size: u64,
            _media: &str,
            _storage: &str,
        ) -> io::Result<()> {
            if self.successful_puts == 1 {
                return Err(io::Error::other("injected second-file transfer failure"));
            }
            self.objects.insert(key.to_owned(), fs::read(source)?);
            self.successful_puts += 1;
            Ok(())
        }
    }

    struct Fixture {
        temp: TempDir,
        repo: PathBuf,
        payload_root: PathBuf,
        context: PlanContext,
        file_sha: String,
        body: Vec<u8>,
    }
    impl Fixture {
        fn new() -> Self {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path();
            let repo = root.join("repo");
            fs::create_dir_all(&repo).unwrap();
            let source = Path::new(env!("CARGO_MANIFEST_DIR"))
                .ancestors()
                .nth(3)
                .unwrap();
            for ref_path in [PLAN_SCHEMA, RECEIPT_SCHEMA, REVIEW_SCHEMA] {
                let dest = repo.join(ref_path);
                fs::create_dir_all(dest.parent().unwrap()).unwrap();
                fs::copy(source.join(ref_path), dest).unwrap();
            }
            let item = "tos.item.fixture.khuddakapatha";
            let item_root = "ToS/source-witnesses/works/fixture/items/khuddakapatha";
            let manifest_ref = format!("{item_root}/item.manifest.json");
            let rights_ref = format!("{item_root}/rights.json");
            let review_ref =
                "ToS/source-witnesses/server-import/reviews/fixture-rights-review.json";
            let evidence_ref = "ToS/source-witnesses/server-import/reviews/fixture-license.txt";
            let revoke_ref = "ToS/source-witnesses/server-import/reviews/fixture-revocation.md";
            for reference in [
                &manifest_ref,
                &rights_ref,
                review_ref,
                evidence_ref,
                revoke_ref,
            ] {
                let path = repo.join(reference);
                fs::create_dir_all(path.parent().unwrap()).unwrap();
            }
            fs::write(repo.join(evidence_ref), b"CC0 exact fixture license\n").unwrap();
            fs::write(repo.join(revoke_ref), b"Review before every transfer\n").unwrap();
            let payload_root = root.join("payload-root");
            let payload =
                payload_root.join("works/fixture/items/khuddakapatha/payload/witness.txt");
            fs::create_dir_all(payload.parent().unwrap()).unwrap();
            let body = b"exact source witness\n".to_vec();
            fs::write(&payload, &body).unwrap();
            let file_sha = digest_hex(&body);
            let file_ref = format!("tos.file.sha256.{file_sha}");
            let manifest = json!({"item_id":item,"rights_ref":rights_ref,"payload_files":[{"file_id":file_ref,"relative_path":"payload/witness.txt","byte_size":body.len(),"sha256":file_sha,"media_type":"text/plain"}]});
            let rights = json!({"item_id":item,"scope_refs":[item,file_ref],"assessment_status":"licensed","permissions":["retain exact bytes"],"restrictions":[],"visibility":"controlled","redistribution_posture":"authorized","server_processing_posture":"authorized","source":"https://example.test/license"});
            let write = |path: &Path, value: &Value| {
                fs::write(path, json_pretty(value).unwrap()).unwrap();
            };
            let manifest_path = repo.join(&manifest_ref);
            let rights_path = repo.join(&rights_ref);
            write(&manifest_path, &manifest);
            write(&rights_path, &rights);
            let manifest_sha = digest_file(&manifest_path).unwrap();
            let rights_sha = digest_file(&rights_path).unwrap();
            let evidence_sha = digest_file(&repo.join(evidence_ref)).unwrap();
            let review = json!({"$schema":"https://tree-of-sophia.local/ToS/contracts/source-payload-rights-review.schema.json","schema_version":"tos_source_payload_rights_review_v1","review_id":"tos.review.fixture.khuddakapatha.20260913","actor":{"kind":"model","actor_ref":"model:test","model":"test-model"},"reviewed_at":"2026-09-13T00:00:00Z","scope":{"item_ref":item,"file_refs":[file_ref],"manifest_ref":manifest_ref,"manifest_sha256":manifest_sha,"rights_record_ref":rights_ref,"rights_record_sha256":rights_sha},"license_evidence":[{"source_ref":"https://example.test/license","evidence_ref":evidence_ref,"evidence_sha256":evidence_sha,"kind":"primary-license","scope":"exact fixture file"}],"decision":{"access_class":"controlled-research","raw_cloud_retention":"controlled-research","server_processing":"authorized","publication":"not-published","conditions":["No derivatives; raw bytes stay private."],"expires_at":null,"revocation_check_ref":revoke_ref}});
            let review_path = repo.join(review_ref);
            write(&review_path, &review);
            let review_sha = digest_file(&review_path).unwrap();
            let derivatives = [
                "ocr",
                "transcription",
                "page_images",
                "snippets",
                "lexical_index",
                "embeddings",
                "alignments",
                "translations",
                "annotations",
                "search_projection",
                "graph_projection",
            ]
            .into_iter()
            .map(|key| {
                (
                    key.to_owned(),
                    json!({"state":"prohibited","conditions":["exact source only"]}),
                )
            })
            .collect::<serde_json::Map<_, _>>();
            let plan = json!({"$schema":"https://tree-of-sophia.local/ToS/contracts/server-import-contract.schema.json","schema_version":"tos_server_import_contract_v1","server_import_id":"tos.server-import.fixture.khuddakapatha.20260913","item_ref":item,"manifest":{"ref":manifest_ref,"sha256":manifest_sha,"verified":true},"payload_files":[{"file_ref":file_ref,"relative_path":"payload/witness.txt","byte_size":body.len(),"sha256":file_sha,"verified":true}],"rights_policy":{"rights_record_ref":rights_ref,"rights_record_sha256":rights_sha,"assessment_status":"open-licensed","review_status":"agent-reviewed","rights_review":{"ref":review_ref,"sha256":review_sha},"jurisdictions_reviewed":["MX"],"permission_or_license_refs":["https://example.test/license",evidence_ref],"expires_at":null,"recheck_before_transfer":true},"access_class":"controlled-research","allowed_derivatives":Value::Object(derivatives),"payload_transfer_authorized":true,"operator_transfer_approval":{"approved":true,"approved_by_real_human":true,"approved_at":"2026-09-13T00:00:00Z","approval_ref":"ToS/source-witnesses/server-import/reviews/operator-approval.md"},"server_import_status":"approved-not-uploaded","publication_status":"not-published","server_receipt_refs":[],"takedown":{"public_contact_url":"https://example.test/contact","procedure_ref":"ToS/source-witnesses/server-import/SERVER_IMPORT_PROTOCOL.md","disable_supported":true,"delete_supported":true,"last_reviewed_at":"2026-09-13T00:00:00Z"},"provenance_event_refs":["tos.event.server-import.fixture.20260913"],"server_copy_is_authority":false,"automatic_checkout_payload_discovery_allowed":false,"contract_version":3});
            let plan_path = repo.join("ToS/source-witnesses/server-import/plans/fixture.json");
            fs::create_dir_all(plan_path.parent().unwrap()).unwrap();
            write(&plan_path, &plan);
            let context = load_plan(&repo, &plan_path).unwrap();
            Self {
                temp,
                repo,
                payload_root,
                context,
                file_sha,
                body,
            }
        }

        fn bind_rights(&mut self, rights: Value, resolution: Option<Value>) {
            let rights_path = self.repo.join(
                self.context.plan["rights_policy"]["rights_record_ref"]
                    .as_str()
                    .unwrap(),
            );
            fs::write(&rights_path, json_pretty(&rights).unwrap()).unwrap();
            let rights_sha = digest_file(&rights_path).unwrap();

            let review_ref = self.context.plan["rights_policy"]["rights_review"]["ref"]
                .as_str()
                .unwrap();
            let review_path = self.repo.join(review_ref);
            let mut review = read_json(&review_path).unwrap();
            review["scope"]["rights_record_sha256"] = json!(rights_sha);
            if let Some(resolution) = resolution {
                review["source_posture_resolution"] = resolution;
            } else if let Some(object) = review.as_object_mut() {
                object.remove("source_posture_resolution");
            }
            fs::write(&review_path, json_pretty(&review).unwrap()).unwrap();
            let review_sha = digest_file(&review_path).unwrap();

            let plan_path = self.context.path.clone();
            let mut plan = self.context.plan.clone();
            plan["rights_policy"]["rights_record_sha256"] = json!(rights_sha);
            plan["rights_policy"]["rights_review"]["sha256"] = json!(review_sha);
            fs::write(&plan_path, json_pretty(&plan).unwrap()).unwrap();
            self.context = load_plan(&self.repo, &plan_path).unwrap();
        }

        fn plan_index(&self, paths: &[PathBuf]) -> (PathBuf, String) {
            let entries = paths
                .iter()
                .map(|path| {
                    let relative = path.strip_prefix(&self.repo).unwrap().to_str().unwrap();
                    json!({"ref":relative,"sha256":digest_file(path).unwrap()})
                })
                .collect::<Vec<_>>();
            let path = self.temp.path().join("plans-index.json");
            fs::write(&path, json_pretty(&json!({"plans":entries})).unwrap()).unwrap();
            let sha = digest_file(&path).unwrap();
            (path, sha)
        }

        fn add_second_file(&mut self) -> String {
            let body = b"second exact source witness\n";
            let file_sha = digest_hex(body);
            let file_ref = format!("tos.file.sha256.{file_sha}");
            let relative_path = "payload/z-second.txt";
            let payload_path = self
                .payload_root
                .join("works/fixture/items/khuddakapatha")
                .join(relative_path);
            fs::create_dir_all(payload_path.parent().unwrap()).unwrap();
            fs::write(&payload_path, body).unwrap();

            let manifest_path = self
                .repo
                .join(self.context.plan["manifest"]["ref"].as_str().unwrap());
            let mut manifest = read_json(&manifest_path).unwrap();
            manifest["payload_files"]
                .as_array_mut()
                .unwrap()
                .push(json!({
                    "file_id":file_ref,
                    "relative_path":relative_path,
                    "byte_size":body.len(),
                    "sha256":file_sha,
                    "media_type":"text/plain"
                }));
            fs::write(&manifest_path, json_pretty(&manifest).unwrap()).unwrap();
            let manifest_sha = digest_file(&manifest_path).unwrap();

            let rights_path = self.repo.join(
                self.context.plan["rights_policy"]["rights_record_ref"]
                    .as_str()
                    .unwrap(),
            );
            let mut rights = read_json(&rights_path).unwrap();
            rights["scope_refs"]
                .as_array_mut()
                .unwrap()
                .push(json!(file_ref));
            fs::write(&rights_path, json_pretty(&rights).unwrap()).unwrap();
            let rights_sha = digest_file(&rights_path).unwrap();

            let review_path = self.repo.join(
                self.context.plan["rights_policy"]["rights_review"]["ref"]
                    .as_str()
                    .unwrap(),
            );
            let mut review = read_json(&review_path).unwrap();
            review["scope"]["file_refs"]
                .as_array_mut()
                .unwrap()
                .push(json!(file_ref));
            review["scope"]["manifest_sha256"] = json!(manifest_sha);
            review["scope"]["rights_record_sha256"] = json!(rights_sha);
            fs::write(&review_path, json_pretty(&review).unwrap()).unwrap();
            let review_sha = digest_file(&review_path).unwrap();

            let mut plan = self.context.plan.clone();
            plan["manifest"]["sha256"] = json!(manifest_sha);
            plan["payload_files"].as_array_mut().unwrap().push(json!({
                "file_ref":file_ref,
                "relative_path":relative_path,
                "byte_size":body.len(),
                "sha256":file_sha,
                "verified":true
            }));
            plan["rights_policy"]["rights_record_sha256"] = json!(rights_sha);
            plan["rights_policy"]["rights_review"]["sha256"] = json!(review_sha);
            fs::write(&self.context.path, json_pretty(&plan).unwrap()).unwrap();
            self.context = load_plan(&self.repo, &self.context.path).unwrap();
            file_ref
        }

        fn batch_request(&self, plans_file: &Path, plans_sha: &str, transfer: bool) -> Value {
            let scratch = self.temp.path().join("scratch");
            let receipts = self.temp.path().join("receipts");
            let output = self.temp.path().join("batch-output");
            json!({
                "schema_version":"tos_source_payload_import_request_v1",
                "family":"payload-import",
                "operation":"batch",
                "repo_root":self.repo.to_str().unwrap(),
                "plans_file":plans_file.to_str().unwrap(),
                "expected_plans_sha256":plans_sha,
                "payload_source_root":self.payload_root.to_str().unwrap(),
                "payload_source_layout":"source-witness",
                "scratch_root":scratch.to_str().unwrap(),
                "receipt_dir":receipts.to_str().unwrap(),
                "output":output.to_str().unwrap(),
                "transport":{"kind":"wrangler","bucket":"fixture-bucket","wrangler":"/usr/bin/wrangler"},
                "transfer":transfer,
            })
        }
    }

    fn posture_resolution(
        server: &[&str],
        redistribution: &[&str],
        retained: &[(&str, &str)],
    ) -> Value {
        let retained_conditions = retained
            .iter()
            .map(|(source_condition, how_satisfied)| {
                json!({"source_condition":source_condition,"how_satisfied":how_satisfied})
            })
            .collect::<Vec<_>>();
        let mut resolution = json!({
            "basis":"primary-license-private-retention",
            "server_processing_postures":server,
            "redistribution_postures":redistribution,
            "rationale":"Primary license evidence supports exact-byte retention under these prior admission defaults.",
        });
        if !retained_conditions.is_empty() {
            resolution["retained_conditions"] = json!(retained_conditions);
        }
        resolution
    }

    #[test]
    fn import_readback_and_receipt_reuse_preserve_the_frozen_binding() {
        let f = Fixture::new();
        let receipts = f.temp.path().join("receipts");
        let scratch = f.temp.path().join("scratch");
        let mut transport = MemoryTransport {
            objects: HashMap::new(),
            puts: 0,
            mutate_after_put: None,
            mutate_on_fetch: None,
        };
        let first = import_context(
            &f.context,
            &f.payload_root,
            "source-witness",
            &mut transport,
            &receipts,
            "fixture-bucket",
            &scratch,
            None,
        )
        .unwrap();
        assert_eq!(transport.puts, 1);
        assert_eq!(first[0]["remote_status"], "uploaded");
        assert_eq!(first[0]["receipt_reused"], false);
        let receipt_path = f
            .temp
            .path()
            .join("receipts")
            .join(first[0]["receipt"].as_str().unwrap());
        let receipt = load_receipt(&receipt_path, &f.repo).unwrap();
        assert_eq!(receipt["source"]["sha256"], f.file_sha);
        assert_eq!(receipt["verification"]["readback_verified"], true);
        assert!(
            !fs::read_to_string(&receipt_path)
                .unwrap()
                .contains(f.temp.path().to_str().unwrap())
        );
        let second = import_context(
            &f.context,
            &f.payload_root,
            "source-witness",
            &mut transport,
            &receipts,
            "fixture-bucket",
            &scratch,
            None,
        )
        .unwrap();
        assert_eq!(transport.puts, 1);
        assert_eq!(second[0]["remote_status"], "already-matched");
        assert_eq!(second[0]["receipt_reused"], true);
        let registry = f.temp.path().join("publication-registry.json");
        enable_publication(&receipt, &registry, "controlled-research").unwrap();
        let output = f.temp.path().join("read.txt");
        let read_local_request = json!({
            "schema_version":"tos_source_payload_import_request_v1",
            "family":"payload-import",
            "operation":"read-local",
            "repo_root":f.repo.to_str().unwrap(),
            "plan":f.context.path.to_str().unwrap(),
            "receipt":receipt_path.to_str().unwrap(),
            "registry":registry.to_str().unwrap(),
            "payload_source_root":f.payload_root.to_str().unwrap(),
            "output":output.to_str().unwrap(),
        });
        assert_eq!(
            operation_read_local(&read_local_request).unwrap()["status"],
            "read-local-verified"
        );
        assert_eq!(fs::read(&output).unwrap(), f.body);
        let occupied = f.temp.path().join("occupied.txt");
        fs::write(&occupied, b"keep existing output\n").unwrap();
        let mut no_clobber_request = read_local_request.clone();
        no_clobber_request["output"] = json!(occupied.to_str().unwrap());
        assert!(operation_read_local(&no_clobber_request).is_err());
        assert_eq!(fs::read(&occupied).unwrap(), b"keep existing output\n");
        disable_publication(
            &registry,
            text(&receipt["publication"], "publication_id").unwrap(),
            "fixture temporary disable",
            false,
        )
        .unwrap();
        let mut disabled_read = read_local_request.clone();
        disabled_read["output"] = json!(f.temp.path().join("disabled.txt").to_str().unwrap());
        assert!(operation_read_local(&disabled_read).is_err());
        enable_publication(&receipt, &registry, "controlled-research").unwrap();
        disable_publication(
            &registry,
            text(&receipt["publication"], "publication_id").unwrap(),
            "fixture revocation",
            true,
        )
        .unwrap();
        assert!(enable_publication(&receipt, &registry, "controlled-research").is_err());
        assert_eq!(
            load_registry(&registry).unwrap()["entries"][0]["status"],
            "revoked"
        );
        let mut revoked_read = read_local_request;
        revoked_read["output"] = json!(f.temp.path().join("revoked.txt").to_str().unwrap());
        assert!(operation_read_local(&revoked_read).is_err());
    }

    #[test]
    fn import_uses_only_the_frozen_inventory_and_transfers_its_verified_snapshot() {
        let f = Fixture::new();
        let payload = f
            .payload_root
            .join("works/fixture/items/khuddakapatha/payload/witness.txt");
        let unlisted = payload.with_file_name("unlisted.txt");
        fs::write(&unlisted, b"not named by the frozen plan\n").unwrap();
        let key = format!("blobs/sha256/{}/{}", &f.file_sha[..2], f.file_sha);
        let mut transport = MemoryTransport {
            objects: HashMap::new(),
            puts: 0,
            mutate_after_put: None,
            mutate_on_fetch: Some((payload.clone(), b"changed after snapshot\n".to_vec())),
        };
        let outcomes = import_context(
            &f.context,
            &f.payload_root,
            "source-witness",
            &mut transport,
            &f.temp.path().join("receipts"),
            "fixture-bucket",
            &f.temp.path().join("scratch"),
            None,
        )
        .unwrap();
        assert_eq!(outcomes.len(), 1);
        assert_eq!(transport.puts, 1);
        assert_eq!(transport.objects.len(), 1);
        assert_eq!(transport.objects.get(&key).unwrap(), &f.body);
        assert_eq!(fs::read(&payload).unwrap(), b"changed after snapshot\n");
        assert_eq!(
            fs::read(&unlisted).unwrap(),
            b"not named by the frozen plan\n"
        );
    }

    #[test]
    fn local_tamper_rights_drift_and_wrong_remote_bytes_fail_before_put() {
        let f = Fixture::new();
        let receipts = f.temp.path().join("receipts");
        let scratch = f.temp.path().join("scratch");
        let mut transport = MemoryTransport {
            objects: HashMap::new(),
            puts: 0,
            mutate_after_put: None,
            mutate_on_fetch: None,
        };
        let payload = f
            .payload_root
            .join("works/fixture/items/khuddakapatha/payload/witness.txt");
        fs::write(&payload, b"wrong local bytes\n").unwrap();
        assert!(
            import_context(
                &f.context,
                &f.payload_root,
                "source-witness",
                &mut transport,
                &receipts,
                "fixture-bucket",
                &scratch,
                None
            )
            .is_err()
        );
        assert_eq!(transport.puts, 0);
        fs::write(&payload, &f.body).unwrap();
        let key = format!("blobs/sha256/{}/{}", &f.file_sha[..2], f.file_sha);
        transport
            .objects
            .insert(key, b"wrong remote bytes".to_vec());
        assert!(
            import_context(
                &f.context,
                &f.payload_root,
                "source-witness",
                &mut transport,
                &receipts,
                "fixture-bucket",
                &scratch,
                None
            )
            .is_err()
        );
        assert_eq!(transport.puts, 0);
        fs::write(
            f.repo
                .join("ToS/source-witnesses/works/fixture/items/khuddakapatha/rights.json"),
            b"changed rights\n",
        )
        .unwrap();
        transport.objects.clear();
        assert!(
            import_context(
                &f.context,
                &f.payload_root,
                "source-witness",
                &mut transport,
                &receipts,
                "fixture-bucket",
                &scratch,
                None
            )
            .is_err()
        );
        assert_eq!(transport.puts, 0);
    }

    #[test]
    fn agent_review_reads_its_exact_revocation_reference_and_terminal_plan_states_block() {
        let f = Fixture::new();
        let revocation_ref = f.context.plan["rights_policy"]["rights_review"]["ref"]
            .as_str()
            .and_then(|review_ref| {
                read_json(&f.repo.join(review_ref)).ok().and_then(|review| {
                    review["decision"]["revocation_check_ref"]
                        .as_str()
                        .map(str::to_owned)
                })
            })
            .unwrap();
        let revocation_path = f.repo.join(&revocation_ref);
        let external = f.temp.path().join("external-revocation-note.md");
        fs::write(&external, b"outside the repository\n").unwrap();
        fs::remove_file(&revocation_path).unwrap();
        std::os::unix::fs::symlink(&external, &revocation_path).unwrap();
        assert!(enforce_transfer(&f.context).is_err());

        let mut context = f.context.clone();
        context.plan["server_import_status"] = json!("approved-not-uploaded");
        for status in ["withdrawn", "takedown-pending", "deleted"] {
            context.plan["server_import_status"] = json!(status);
            assert!(validate_current_server_import_state(&context, "test transfer").is_err());
        }
        context.plan["server_import_status"] = json!("approved-not-uploaded");
        context.plan["publication_status"] = json!("withdrawn");
        assert!(validate_current_takedown_state(&context, "test read").is_err());
    }

    #[test]
    fn import_requires_a_loadable_plan_before_transport_configuration() {
        let f = Fixture::new();
        let request = json!({
            "schema_version":"tos_source_payload_import_request_v1",
            "family":"payload-import",
            "operation":"import",
            "repo_root":f.repo.to_str().unwrap(),
            "plan":f.temp.path().join("missing-plan.json").to_str().unwrap(),
            "payload_source_root":f.payload_root.to_str().unwrap(),
            "receipt_dir":f.temp.path().join("receipts").to_str().unwrap(),
            "scratch_root":f.temp.path().join("scratch").to_str().unwrap(),
            "transport":{"kind":"invalid","bucket":"fixture-bucket","wrangler":"/usr/bin/wrangler"},
            "confirm_transfer":true
        });
        let error = operation_import(&request).unwrap_err();
        assert!(error.contains("missing-plan.json"));
    }

    #[test]
    fn plan_cannot_narrow_the_manifest_file_inventory() {
        let mut f = Fixture::new();
        let manifest_path = f
            .repo
            .join(f.context.plan["manifest"]["ref"].as_str().unwrap());
        let mut manifest = read_json(&manifest_path).unwrap();
        let extra = b"unlisted witness bytes\n";
        manifest["payload_files"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "file_id":format!("tos.file.sha256.{}", digest_hex(extra)),
                "relative_path":"payload/unlisted.txt",
                "byte_size":extra.len(),
                "sha256":digest_hex(extra),
                "media_type":"text/plain"
            }));
        fs::write(&manifest_path, json_pretty(&manifest).unwrap()).unwrap();
        let mut plan = f.context.plan.clone();
        plan["manifest"]["sha256"] = json!(digest_file(&manifest_path).unwrap());
        fs::write(&f.context.path, json_pretty(&plan).unwrap()).unwrap();
        f.context = load_plan(&f.repo, &f.context.path).unwrap();
        assert!(verify_local(&f.context, &f.payload_root, "source-witness", None).is_err());
    }

    #[test]
    fn symlinked_payload_roots_and_changed_review_evidence_fail_before_put() {
        let f = Fixture::new();
        let alias = f.temp.path().join("payload-root-alias");
        std::os::unix::fs::symlink(&f.payload_root, &alias).unwrap();
        assert!(verify_local(&f.context, &alias, "source-witness", None).is_err());

        let review_path = f.repo.join(
            f.context.plan["rights_policy"]["rights_review"]["ref"]
                .as_str()
                .unwrap(),
        );
        fs::write(&review_path, b"altered rights review\n").unwrap();
        let mut transport = MemoryTransport {
            objects: HashMap::new(),
            puts: 0,
            mutate_after_put: None,
            mutate_on_fetch: None,
        };
        assert!(
            import_context(
                &f.context,
                &f.payload_root,
                "source-witness",
                &mut transport,
                &f.temp.path().join("receipts"),
                "fixture-bucket",
                &f.temp.path().join("scratch"),
                None,
            )
            .is_err()
        );
        assert_eq!(transport.puts, 0);
    }

    #[test]
    fn legacy_human_review_remains_eligible_for_exact_controlled_transfer() {
        let mut f = Fixture::new();
        let mut plan = f.context.plan.clone();
        plan["rights_policy"]["review_status"] = json!("human-reviewed");
        plan["rights_policy"]
            .as_object_mut()
            .unwrap()
            .remove("rights_review");
        plan["contract_version"] = json!(2);
        fs::write(&f.context.path, json_pretty(&plan).unwrap()).unwrap();
        f.context = load_plan(&f.repo, &f.context.path).unwrap();
        assert!(enforce_transfer(&f.context).is_ok());
    }

    #[test]
    fn rights_review_is_exact_and_does_not_waive_denials_uncertainty_or_conditions() {
        let mut f = Fixture::new();
        let mut rights = read_json(
            &f.repo
                .join("ToS/source-witnesses/works/fixture/items/khuddakapatha/rights.json"),
        )
        .unwrap();
        rights["server_processing_posture"] = json!("not_authorized");
        rights["redistribution_posture"] = json!("metadata_only");
        f.bind_rights(rights.clone(), None);
        assert!(enforce_transfer(&f.context).is_err());
        f.bind_rights(
            rights.clone(),
            Some(posture_resolution(
                &["not_authorized"],
                &["metadata_only"],
                &[],
            )),
        );
        assert!(enforce_transfer(&f.context).is_ok());

        let mut denied = rights.clone();
        denied["server_processing_posture"] = json!("denied");
        f.bind_rights(
            denied,
            Some(posture_resolution(&[], &["metadata_only"], &[])),
        );
        assert!(enforce_transfer(&f.context).is_err());

        let mut uncertain = rights.clone();
        uncertain["assessment_status"] = json!("research_only");
        f.bind_rights(
            uncertain,
            Some(posture_resolution(
                &["not_authorized"],
                &["metadata_only"],
                &[],
            )),
        );
        assert!(enforce_transfer(&f.context).is_err());

        let mut conditional = rights;
        conditional["server_processing_posture"] = json!("authorized_with_conditions");
        conditional["redistribution_posture"] = json!("authorized_with_conditions");
        conditional["restrictions"] = json!(["Retain attribution.", "Retain license notice."]);
        f.bind_rights(
            conditional.clone(),
            Some(posture_resolution(
                &["authorized_with_conditions"],
                &["authorized_with_conditions"],
                &[("Retain attribution.", "Original bytes remain unchanged.")],
            )),
        );
        assert!(enforce_transfer(&f.context).is_err());
        f.bind_rights(
            conditional,
            Some(posture_resolution(
                &["authorized_with_conditions"],
                &["authorized_with_conditions"],
                &[
                    ("Retain attribution.", "Original bytes remain unchanged."),
                    (
                        "Retain license notice.",
                        "The source record retains the notice.",
                    ),
                ],
            )),
        );
        assert!(enforce_transfer(&f.context).is_ok());
    }

    #[test]
    fn private_retention_resolution_cannot_authorize_public_delivery_or_derivatives() {
        let mut f = Fixture::new();
        let mut rights = read_json(
            &f.repo
                .join("ToS/source-witnesses/works/fixture/items/khuddakapatha/rights.json"),
        )
        .unwrap();
        rights["server_processing_posture"] = json!("not_authorized");
        rights["redistribution_posture"] = json!("not_authorized");
        let resolution = posture_resolution(&["not_authorized"], &["not_authorized"], &[]);
        f.bind_rights(rights, Some(resolution));
        assert!(enforce_transfer(&f.context).is_ok());
        f.context.plan["access_class"] = json!("public-payload");
        assert!(enforce_transfer(&f.context).is_err());
        f.context.plan["access_class"] = json!("metadata-only");
        assert!(enforce_transfer(&f.context).is_err());
        f.context.plan["access_class"] = json!("controlled-research");
        f.context.plan["allowed_derivatives"]["embeddings"]["state"] = json!("allowed");
        assert!(enforce_transfer(&f.context).is_err());
    }

    #[test]
    fn batch_preflight_avoids_transport_and_partial_transfer_keeps_journal() {
        let f = Fixture::new();
        let first_plan = f.context.path.clone();
        let mut second = f.context.plan.clone();
        second["server_import_id"] = json!("tos.server-import.fixture.second.20260913");
        let second_path = first_plan.with_file_name("fixture-second.json");
        fs::write(&second_path, json_pretty(&second).unwrap()).unwrap();
        let (plans_file, plans_sha) = f.plan_index(&[first_plan.clone(), second_path.clone()]);
        let request = f.batch_request(&plans_file, &plans_sha, true);

        let mut verify_only = f.batch_request(&plans_file, &plans_sha, false);
        verify_only["output"] = json!(f.temp.path().join("batch-verify-output").to_str().unwrap());
        let constructed = std::cell::Cell::new(false);
        let response = operation_batch_with_transport(&verify_only, |_| {
            constructed.set(true);
            Ok(MemoryTransport {
                objects: HashMap::new(),
                puts: 0,
                mutate_after_put: None,
                mutate_on_fetch: None,
            })
        })
        .unwrap();
        assert!(!constructed.get());
        assert_eq!(response["status"], "verified-only");
        assert_eq!(response["plans"], 2);
        assert_eq!(response["files"], 2);
        assert_eq!(response["complete"], true);

        let wrong_digest = f.batch_request(&plans_file, &"0".repeat(64), true);
        let constructed = std::cell::Cell::new(false);
        assert!(
            operation_batch_with_transport(&wrong_digest, |_| {
                constructed.set(true);
                Ok(MemoryTransport {
                    objects: HashMap::new(),
                    puts: 0,
                    mutate_after_put: None,
                    mutate_on_fetch: None,
                })
            })
            .is_err()
        );
        assert!(!constructed.get());
        assert!(!f.temp.path().join("batch-output").exists());

        let payload = f
            .payload_root
            .join("works/fixture/items/khuddakapatha/payload/witness.txt");
        fs::write(&payload, b"changed before batch preflight\n").unwrap();
        let constructed = std::cell::Cell::new(false);
        assert!(
            operation_batch_with_transport(&request, |_| {
                constructed.set(true);
                Ok(MemoryTransport {
                    objects: HashMap::new(),
                    puts: 0,
                    mutate_after_put: None,
                    mutate_on_fetch: None,
                })
            })
            .is_err()
        );
        assert!(!constructed.get());
        assert!(!f.temp.path().join("batch-output").exists());
        fs::write(&payload, &f.body).unwrap();

        let mut transport = Some(MemoryTransport {
            objects: HashMap::new(),
            puts: 0,
            mutate_after_put: Some((second_path.clone(), b"changed after first plan\n".to_vec())),
            mutate_on_fetch: None,
        });
        let result = operation_batch_with_transport(&request, |_| Ok(transport.take().unwrap()));
        assert!(result.is_err());

        let output = f.temp.path().join("batch-output");
        let summaries = fs::read_dir(&output)
            .unwrap()
            .filter_map(|entry| {
                let path = entry.ok()?.path();
                path.extension()
                    .is_some_and(|ext| ext == "json")
                    .then_some(path)
            })
            .collect::<Vec<_>>();
        let journals = fs::read_dir(&output)
            .unwrap()
            .filter_map(|entry| {
                let path = entry.ok()?.path();
                path.extension()
                    .is_some_and(|ext| ext == "jsonl")
                    .then_some(path)
            })
            .collect::<Vec<_>>();
        assert_eq!(summaries.len(), 1);
        assert_eq!(journals.len(), 1);
        let summary = read_json(&summaries[0]).unwrap();
        assert_eq!(summary["complete"], false);
        assert_eq!(summary["failure_type"], "BatchError");
        assert_eq!(summary["completed_plans"], 1);
        let journal = fs::read_to_string(&journals[0]).unwrap();
        let entries = journal.lines().collect::<Vec<_>>();
        assert_eq!(entries.len(), 1);
        assert_eq!(
            io_owner::parse(entries[0].as_bytes()).unwrap()["plan_ref"],
            first_plan.strip_prefix(&f.repo).unwrap().to_str().unwrap()
        );
    }

    #[test]
    fn batch_journals_each_file_before_a_later_file_in_the_same_plan_fails() {
        let mut f = Fixture::new();
        let second_file_ref = f.add_second_file();
        let plan = f.context.path.clone();
        let (plans_file, plans_sha) = f.plan_index(&[plan.clone()]);
        let mut request = f.batch_request(&plans_file, &plans_sha, true);
        request["output"] = json!(
            f.temp
                .path()
                .join("batch-same-plan-output")
                .to_str()
                .unwrap()
        );

        assert!(
            operation_batch_with_transport(&request, |_| { Ok(FailAfterOneTransport::default()) })
                .is_err()
        );

        let output = f.temp.path().join("batch-same-plan-output");
        let summaries = fs::read_dir(&output)
            .unwrap()
            .filter_map(|entry| {
                let path = entry.ok()?.path();
                path.extension()
                    .is_some_and(|ext| ext == "json")
                    .then_some(path)
            })
            .collect::<Vec<_>>();
        let journals = fs::read_dir(&output)
            .unwrap()
            .filter_map(|entry| {
                let path = entry.ok()?.path();
                path.extension()
                    .is_some_and(|ext| ext == "jsonl")
                    .then_some(path)
            })
            .collect::<Vec<_>>();
        assert_eq!(summaries.len(), 1);
        assert_eq!(journals.len(), 1);
        let summary = read_json(&summaries[0]).unwrap();
        assert_eq!(summary["complete"], false);
        assert_eq!(summary["completed_plans"], 0);
        assert_eq!(summary["completed_files"], 1);
        assert_eq!(summary["uploaded_files"], 1);
        assert_eq!(summary["files"], 2);
        let journal = fs::read_to_string(&journals[0]).unwrap();
        let entries = journal.lines().collect::<Vec<_>>();
        assert_eq!(entries.len(), 1);
        let entry = io_owner::parse(entries[0].as_bytes()).unwrap();
        assert_eq!(
            entry["plan_ref"],
            plan.strip_prefix(&f.repo).unwrap().to_str().unwrap()
        );
        assert_eq!(entry["files"], 1);
        assert_eq!(entry["uploaded_files"], 1);
        assert_eq!(entry["outcome"]["receipt"].is_string(), true);
        assert_eq!(entry["outcome"]["file_ref"].is_string(), true);
        assert_eq!(
            entry["outcome"]["file_ref"],
            format!("tos.file.sha256.{}", f.file_sha)
        );
        assert_ne!(entry["outcome"]["file_ref"], second_file_ref);
    }

    #[test]
    fn registry_protected_remote_read_checks_current_plan_before_transport() {
        let f = Fixture::new();
        let receipts = f.temp.path().join("receipts");
        let scratch = f.temp.path().join("scratch");
        let registry = f.temp.path().join("publication-registry.json");
        let mut transport = MemoryTransport {
            objects: HashMap::new(),
            puts: 0,
            mutate_after_put: None,
            mutate_on_fetch: None,
        };
        let outcomes = import_context(
            &f.context,
            &f.payload_root,
            "source-witness",
            &mut transport,
            &receipts,
            "fixture-bucket",
            &scratch,
            None,
        )
        .unwrap();
        let receipt_path = receipts.join(outcomes[0]["receipt"].as_str().unwrap());
        let receipt = load_receipt(&receipt_path, &f.repo).unwrap();
        enable_publication(&receipt, &registry, "controlled-research").unwrap();

        let output = f.temp.path().join("remote-read.txt");
        let request = json!({
            "schema_version":"tos_source_payload_import_request_v1",
            "family":"payload-import",
            "operation":"read-remote",
            "repo_root":f.repo.to_str().unwrap(),
            "plan":f.context.path.to_str().unwrap(),
            "receipt":receipt_path.to_str().unwrap(),
            "registry":registry.to_str().unwrap(),
            "publication":receipt["publication"]["publication_id"],
            "output":output.to_str().unwrap(),
            "scratch_root":scratch.to_str().unwrap(),
            "transport":{"kind":"wrangler","bucket":"fixture-bucket","wrangler":"/usr/bin/wrangler"},
        });
        let response = operation_read_remote_with_transport(&request, |_| Ok(transport)).unwrap();
        assert_eq!(response["status"], "read-remote-verified");
        assert_eq!(fs::read(&output).unwrap(), f.body);

        let mut unprotected_request = request.clone();
        unprotected_request.as_object_mut().unwrap().remove("plan");
        unprotected_request
            .as_object_mut()
            .unwrap()
            .remove("registry");
        let constructed = std::cell::Cell::new(false);
        assert!(
            operation_read_remote_with_transport(&unprotected_request, |_| {
                constructed.set(true);
                Ok(MemoryTransport {
                    objects: HashMap::new(),
                    puts: 0,
                    mutate_after_put: None,
                    mutate_on_fetch: None,
                })
            })
            .is_err()
        );
        assert!(!constructed.get());

        disable_publication(
            &registry,
            text(&receipt["publication"], "publication_id").unwrap(),
            "fixture temporary disable",
            false,
        )
        .unwrap();
        let disabled_output = f.temp.path().join("disabled-remote-read.txt");
        let mut disabled_request = request.clone();
        disabled_request["output"] = json!(disabled_output.to_str().unwrap());
        let constructed = std::cell::Cell::new(false);
        assert!(
            operation_read_remote_with_transport(&disabled_request, |_| {
                constructed.set(true);
                Ok(MemoryTransport {
                    objects: HashMap::new(),
                    puts: 0,
                    mutate_after_put: None,
                    mutate_on_fetch: None,
                })
            })
            .is_err()
        );
        assert!(!constructed.get());
        assert!(!disabled_output.exists());
        enable_publication(&receipt, &registry, "controlled-research").unwrap();

        disable_publication(
            &registry,
            text(&receipt["publication"], "publication_id").unwrap(),
            "fixture terminal revocation",
            true,
        )
        .unwrap();
        let revoked_output = f.temp.path().join("revoked-remote-read.txt");
        let mut revoked_request = request.clone();
        revoked_request["output"] = json!(revoked_output.to_str().unwrap());
        let constructed = std::cell::Cell::new(false);
        assert!(
            operation_read_remote_with_transport(&revoked_request, |_| {
                constructed.set(true);
                Ok(MemoryTransport {
                    objects: HashMap::new(),
                    puts: 0,
                    mutate_after_put: None,
                    mutate_on_fetch: None,
                })
            })
            .is_err()
        );
        assert!(!constructed.get());
        assert!(!revoked_output.exists());

        fs::write(
            f.repo
                .join("ToS/source-witnesses/works/fixture/items/khuddakapatha/rights.json"),
            b"rights changed after publication\n",
        )
        .unwrap();
        let stale_output = f.temp.path().join("stale-read.txt");
        let mut stale_request = request;
        stale_request["output"] = json!(stale_output.to_str().unwrap());
        let constructed = std::cell::Cell::new(false);
        assert!(
            operation_read_remote_with_transport(&stale_request, |_| {
                constructed.set(true);
                Ok(MemoryTransport {
                    objects: HashMap::new(),
                    puts: 0,
                    mutate_after_put: None,
                    mutate_on_fetch: None,
                })
            })
            .is_err()
        );
        assert!(!constructed.get());
        assert!(!stale_output.exists());
    }

    #[test]
    fn concurrent_disable_waits_for_registry_gated_remote_output_publication() {
        let f = Fixture::new();
        let receipts = f.temp.path().join("receipts");
        let scratch = f.temp.path().join("scratch");
        let registry = f.temp.path().join("publication-registry.json");
        let mut transport = MemoryTransport {
            objects: HashMap::new(),
            puts: 0,
            mutate_after_put: None,
            mutate_on_fetch: None,
        };
        let outcomes = import_context(
            &f.context,
            &f.payload_root,
            "source-witness",
            &mut transport,
            &receipts,
            "fixture-bucket",
            &scratch,
            None,
        )
        .unwrap();
        let receipt_path = receipts.join(outcomes[0]["receipt"].as_str().unwrap());
        let receipt = load_receipt(&receipt_path, &f.repo).unwrap();
        let selector = text(&receipt["publication"], "publication_id")
            .unwrap()
            .to_owned();
        enable_publication(&receipt, &registry, "controlled-research").unwrap();

        let output = f.temp.path().join("concurrent-remote-read.txt");
        let request = json!({
            "schema_version":"tos_source_payload_import_request_v1",
            "family":"payload-import",
            "operation":"read-remote",
            "repo_root":f.repo.to_str().unwrap(),
            "plan":f.context.path.to_str().unwrap(),
            "receipt":receipt_path.to_str().unwrap(),
            "registry":registry.to_str().unwrap(),
            "publication":selector.clone(),
            "output":output.to_str().unwrap(),
            "scratch_root":scratch.to_str().unwrap(),
            "transport":{"kind":"wrangler","bucket":"fixture-bucket","wrangler":"/usr/bin/wrangler"},
        });
        let (fetch_started_tx, fetch_started_rx) = std::sync::mpsc::sync_channel(0);
        let (resume_fetch_tx, resume_fetch_rx) = std::sync::mpsc::sync_channel(0);
        let read_request = request.clone();
        let read_body = f.body.clone();
        let reader = std::thread::spawn(move || {
            operation_read_remote_with_transport(&read_request, |_| {
                Ok(GatedReadTransport {
                    body: read_body,
                    fetch_started: fetch_started_tx,
                    resume_fetch: resume_fetch_rx,
                })
            })
        });
        fetch_started_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("reader reached the synthetic fetch after passing the registry gate");

        assert!(
            registry_lock(&registry, FlockOperation::NonBlockingLockExclusive).is_err(),
            "remote read must retain the shared registry guard while fetch is pending"
        );

        let (disable_started_tx, disable_started_rx) = std::sync::mpsc::sync_channel(0);
        let (disable_done_tx, disable_done_rx) = std::sync::mpsc::channel();
        let disable_registry = registry.clone();
        let disable_selector = selector.to_owned();
        let disabler = std::thread::spawn(move || {
            disable_started_tx.send(()).unwrap();
            let result = disable_publication(
                &disable_registry,
                &disable_selector,
                "concurrent fixture disable",
                false,
            );
            let _ = disable_done_tx.send(result.clone());
            result
        });
        disable_started_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("disable worker started");
        assert!(
            disable_done_rx.try_recv().is_err(),
            "disable must not complete before verified output publication"
        );
        resume_fetch_tx.send(()).unwrap();

        let response = reader.join().unwrap().unwrap();
        assert_eq!(response["status"], "read-remote-verified");
        assert_eq!(fs::read(&output).unwrap(), f.body);
        disabler.join().unwrap().unwrap();
        assert_eq!(
            load_registry(&registry).unwrap()["entries"][0]["status"],
            "disabled"
        );
        assert!(disable_done_rx.try_recv().unwrap().is_ok());
    }
}
