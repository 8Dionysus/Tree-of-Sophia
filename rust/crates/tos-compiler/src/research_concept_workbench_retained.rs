//! Read-only retained native Concept default. No producer or SQL regeneration.
use super::*;
use std::fs::{File, Metadata};
use std::io::{Seek, SeekFrom};
use std::os::unix::fs::MetadataExt;

struct Held {
    reference: String,
    file: File,
    before: Metadata,
    digest: String,
}
fn fingerprint(m: &Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
    (
        m.dev(),
        m.ino(),
        m.len(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    )
}
fn hold(root: &ResearchExecution, reference: &str, expected: &str, private: bool) -> Result<Held> {
    root.tick(1)?;
    if expected.len() != 64
        || !expected
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(format!("retained invalid SHA256: {reference}"));
    }
    let mut file = root.source_file(reference, 256 * 1024 * 1024)?;
    let before = file.metadata().map_err(|e| e.to_string())?;
    if !before.is_file() || (private && before.mode() & 0o7777 != 0o600) {
        return Err(format!("retained file type/private mode: {reference}"));
    }
    let digest = root.hash_file(&mut file, 256 * 1024 * 1024)?;
    root.verify_file_unchanged(&file, &before)?;
    if digest != expected {
        return Err(format!("retained artifact drift: {reference}"));
    }
    Ok(Held {
        reference: reference.into(),
        file,
        before,
        digest,
    })
}

fn held_json(root: &ResearchExecution, item: &mut Held) -> Result<Value> {
    item.file
        .seek(SeekFrom::Start(0))
        .map_err(|e| e.to_string())?;
    let raw = root.read_file(&mut item.file, 64 * 1024 * 1024)?;
    root.verify_file_unchanged(&item.file, &item.before)?;
    if hash(&raw) != item.digest {
        return Err(format!("retained input drift: {}", item.reference));
    }
    parse_json(&raw, JsonMode::PublishedStrict, limits()?).map_err(|e| e.to_string())?;
    serde_json::from_slice(&raw).map_err(|e| e.to_string())
}

fn recipe_reference(root: &ResearchExecution, reference: &str, expected: &str) -> Result<String> {
    if file_hash(root, reference)? == expected {
        return Ok(reference.into());
    }
    if reference.starts_with("scripts/") && reference.ends_with(".py") {
        let stem = Path::new(reference)
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or("retained recipe name")?;
        let retained = format!("ToS/research-packets/retained-builder-inputs/{stem}/{expected}.py");
        if file_hash(root, &retained)? == expected {
            return Ok(retained);
        }
    }
    Err(format!("retained recipe input drift: {reference}"))
}
fn private_boundary(root: &ResearchExecution, reference: &str) -> Result<()> {
    use crate::owned_native_child::{CaptureLimits, capture_with_cancel};
    use std::os::fd::AsRawFd;
    root.tick(1)?;
    let cwd = format!("/proc/self/fd/{}", root.root_directory().as_raw_fd());
    let ignored = capture_with_cancel(
        std::process::Command::new("git")
            .args(["check-ignore", "-q", "--", reference])
            .current_dir(&cwd),
        None,
        CaptureLimits {
            max_stdin_bytes: 0,
            max_stdout_bytes: 4096,
            max_stderr_bytes: 16384,
        },
        root.deadline(),
        root.cancellation_flag(),
    )?;
    root.tick(1)?;
    let tracked = capture_with_cancel(
        std::process::Command::new("git")
            .args(["ls-files", "--error-unmatch", "--", reference])
            .current_dir(&cwd),
        None,
        CaptureLimits {
            max_stdin_bytes: 0,
            max_stdout_bytes: 4096,
            max_stderr_bytes: 16384,
        },
        root.deadline(),
        root.cancellation_flag(),
    )?;
    root.check()?;
    git_boundary(ignored.status.code(), tracked.status.code(), reference)
}
pub(super) fn check(
    root: &ResearchExecution,
    c: &Config,
    selected: &ConceptPlan,
    request: &Value,
) -> Result<Value> {
    root.tick(1)?;
    let manifest_ref = c.output("manifest");
    let mut manifest_file = root.source_file(manifest_ref, 64 * 1024 * 1024)?;
    let manifest_before = manifest_file.metadata().map_err(|e| e.to_string())?;
    let raw = root.read_file(&mut manifest_file, 64 * 1024 * 1024)?;
    root.verify_file_unchanged(&manifest_file, &manifest_before)?;
    parse_json(&raw, JsonMode::PublishedStrict, limits()?).map_err(|e| e.to_string())?;
    let manifest: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    if s(&manifest, "schema_version") != "tos_zarathustra_concept_workbench_manifest_v1"
        || s(&manifest, "route_id") != "zarathustra-concept-workbench-v1"
        || manifest
            .get("accepted_candidate_count")
            .and_then(Value::as_u64)
            != Some(0)
        || manifest.get("human_review_count").and_then(Value::as_u64) != Some(0)
        || manifest.get("graph_effect").and_then(Value::as_bool) != Some(false)
        || manifest.get("canon_effect").and_then(Value::as_bool) != Some(false)
    {
        return Err("retained Concept manifest identity/authority boundary".into());
    }
    if s(&manifest, "plan_ref") != selected.reference
        || s(&manifest, "plan_sha256") != selected.digest
        || s(&manifest, "identity_issuance_ref") != c.issuance
        || s(&manifest, "generator_ref") != GENERATOR
        || s(&manifest, "generator_sha256") != GENERATOR_SHA
    {
        return Err("retained Concept selected plan/recipe/issuance binding".into());
    }
    verify_plan_input(root, &json!({"ref":GENERATOR,"sha256":GENERATOR_SHA}))?;
    let requests = manifest
        .get("request_refs")
        .and_then(Value::as_array)
        .ok_or("retained request refs")?;
    if requests.len() != 1
        || s(&requests[0], "ref") != c.request_ref
        || s(&requests[0], "sha256") != file_hash(root, &c.request_ref)?
    {
        return Err("retained Concept request binding".into());
    }
    let mut held = vec![Held {
        reference: manifest_ref.into(),
        file: manifest_file,
        before: manifest_before,
        digest: hash(&raw),
    }];
    held.push(hold(root, &selected.reference, &selected.digest, false)?);
    held.push(hold(
        root,
        &c.request_ref,
        s(&requests[0], "sha256"),
        false,
    )?);
    if held_json(root, held.last_mut().ok_or("retained request descriptor")?)? != *request {
        return Err("retained request parsed bytes changed".into());
    }
    let recipe = recipe_reference(root, GENERATOR, GENERATOR_SHA)?;
    held.push(hold(root, &recipe, GENERATOR_SHA, false)?);
    for (key, reference, expected) in [
        (
            "request_schema",
            format!("{ROUTE}/concept-request.v2.schema.json"),
            None,
        ),
        (
            "relation_schema",
            format!("{ROUTE}/concept-relation-candidate.v1.schema.json"),
            None,
        ),
        (
            "english_task_schema",
            format!("{ROUTE}/english-on-demand-task.v1.schema.json"),
            None,
        ),
        (
            "english_candidate_schema",
            format!("{ROUTE}/english-translation-candidate.v1.schema.json"),
            None,
        ),
        (
            "concept_search_result_schema",
            format!("{ROUTE}/concept-search-result.v1.schema.json"),
            None,
        ),
        ("concept_search_query", QUERY.into(), Some(QUERY_SHA)),
        (
            "word_analysis_task_schema",
            format!("{ROUTE}/word-analysis-task.v1.schema.json"),
            None,
        ),
        ("word_analysis_prepare", WORD.into(), Some(WORD_SHA)),
    ] {
        root.tick(1)?;
        let expected_hash = s(&manifest, &format!("{key}_sha256"));
        if s(&manifest, &format!("{key}_ref")) != reference
            || expected.is_some_and(|wanted| wanted != expected_hash)
        {
            return Err(format!("retained bound source ref: {key}"));
        }
        let resolved = if expected.is_some() {
            recipe_reference(root, &reference, expected_hash)?
        } else {
            reference
        };
        held.push(hold(root, &resolved, expected_hash, false)?);
    }
    held.push(hold(
        root,
        &c.issuance,
        s(&manifest, "identity_issuance_sha256"),
        false,
    )?);
    let issuance = held_json(root, held.last_mut().ok_or("retained issuance descriptor")?)?;
    if s(&issuance, "schema_version") != "tos_zarathustra_concept_workbench_identity_issuance_v1" {
        return Err("retained issuance schema".into());
    }
    let mut bindings = BTreeMap::new();
    let mut unique_ids = BTreeSet::new();
    for record in issuance
        .get("records")
        .and_then(Value::as_array)
        .ok_or("retained issuance records")?
    {
        root.tick(1)?;
        let kind = s(record, "kind");
        let identity_binding = s(record, "binding");
        let id = s(record, "id");
        if kind.is_empty()
            || identity_binding.is_empty()
            || id.is_empty()
            || !unique_ids.insert(id)
            || bindings.insert((kind, identity_binding), id).is_some()
        {
            return Err("retained duplicate/empty issuance identity".into());
        }
    }
    let request_binding = binding(request);
    if bindings.get(&("workbench", "foundation-v1")).copied() != Some(s(&manifest, "workbench_id"))
        || !bindings.contains_key(&("request", request_binding.as_str()))
    {
        return Err("retained issued workbench/request identity binding".into());
    }
    let expected: BTreeMap<&str, &str> = c
        .outputs
        .iter()
        .filter(|(k, _)| k != "manifest")
        .map(|(k, p)| (k.as_str(), p.as_str()))
        .collect();
    let artifacts = manifest
        .get("artifacts")
        .and_then(Value::as_array)
        .ok_or("retained artifact list")?;
    if artifacts.len() != expected.len() {
        return Err("retained Concept incomplete artifact set".into());
    }
    let mut seen = BTreeSet::new();
    for artifact in artifacts {
        root.tick(1)?;
        let role = s(artifact, "role");
        let reference = s(artifact, "ref");
        if !seen.insert(role) || expected.get(role).copied() != Some(reference) {
            return Err("retained Concept artifact role/ref set".into());
        }
        held.push(hold(root, reference, s(artifact, "sha256"), false)?);
    }
    let private = manifest
        .get("private_artifacts")
        .and_then(Value::as_array)
        .ok_or("retained private artifact list")?;
    if private.len() != 2 {
        return Err("retained Concept incomplete private artifact set".into());
    }
    let mut private_seen = BTreeSet::new();
    for artifact in private {
        root.tick(1)?;
        let reference = s(artifact, "ref");
        if ![c.private_db.as_str(), c.private_request.as_str()].contains(&reference)
            || !private_seen.insert(reference)
            || s(artifact, "mode") != "0600"
            || artifact.get("tracked").and_then(Value::as_bool) != Some(false)
        {
            return Err("retained Concept private ref/mode/boundary".into());
        }
        private_boundary(root, reference)?;
        held.push(hold(root, reference, s(artifact, "sha256"), true)?);
    }
    // Recheck the retained descriptors and current selected pathnames. A
    // matching hash never authorizes a later carrier replacement silently.
    for item in &mut held {
        root.tick(1)?;
        root.verify_file_unchanged(&item.file, &item.before)?;
        let current = root.source_file(&item.reference, 256 * 1024 * 1024)?;
        if fingerprint(&current.metadata().map_err(|e| e.to_string())?) != fingerprint(&item.before)
            || root.hash_file(&mut item.file, 256 * 1024 * 1024)? != item.digest
        {
            return Err(format!("retained Concept final drift: {}", item.reference));
        }
        root.verify_file_unchanged(&item.file, &item.before)?;
    }
    root.check()?;
    Ok(
        json!({"producer":"native retained Concept workbench default","status":"retained_checked","manifest_ref":manifest_ref,"manifest_sha256":hash(&raw),"plan_ref":selected.reference,"plan_sha256":selected.digest,"request_ref":c.request_ref,"request_key":request["request_key"],"tracked_products":expected.len()+1,"private_products":private.len(),"complete_products":expected.len()+1+private.len(),"producer_replay":false,"sql_regeneration":false,"writes":false,"identity_issuance_created":false,"graph_effect":false,"canon_effect":false,"budget":root.budget_report()}),
    )
}
