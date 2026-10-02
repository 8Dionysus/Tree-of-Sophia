//! Native consumer for one frozen acquisition handoff.
//!
//! Handoffs are transport evidence only. This module verifies their exact
//! manifest, selected records, payload custody, fixity, rights/provenance
//! bindings, and accepted base before materializing a private candidate batch.
//! It never admits or publishes a corpus revision.

use crate::source_acquisition_batch as batch;
use crate::source_admission::{AdmissionBatch, AdmissionLimits};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde_json::{Map, Value, json};
use sha1::{Digest as Sha1Digest, Sha1};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{Read, Write},
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};
use tos_foundation::{Digest256, Digest256Hasher, JsonLimits, RelativePath, SourceRevision};
use tos_source_store::{
    CaptureRestoreLimits, CorpusReader, ReadLimits, SoftwareCaptureReader,
    SoftwareCaptureSelectionV1, verify_capture,
};

type Result<T> = std::result::Result<T, String>;

const HANDOFF_SCHEMA: &str = "tos_acquisition_handoff_v1";
const FIXITY_SCHEMA: &str = "tos_acquisition_independent_fixity_v1";
const VALIDATION_CONTEXT_SCHEMA: &str = "tos_corpus_validation_context_v1";
const MAX_JSON_BYTES: u64 = 64 * 1024 * 1024;
const MAX_CAPTURE_ARCHIVE_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MAX_CAPTURE_SOURCE_BYTES: u64 = 16 * 1024 * 1024 * 1024;
const MAX_PAYLOAD_BYTES: u64 = 300 * 1024 * 1024;

fn required_text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing text field: {key}"))
}

fn required_u64(value: &Value, key: &str) -> Result<u64> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("missing unsigned integer field: {key}"))
}

fn value_at<'a>(value: &'a Value, key: &str) -> Result<&'a Value> {
    value
        .get(key)
        .ok_or_else(|| format!("missing field: {key}"))
}

fn is_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn checked_digest(value: &str, label: &str) -> Result<Digest256> {
    if !is_digest(value) {
        return Err(format!("{label} must be lowercase SHA-256"));
    }
    Digest256::from_hex(value).map_err(|_| format!("{label} must be lowercase SHA-256"))
}

fn expand_home(value: &str) -> Result<PathBuf> {
    if value == "~" || value.starts_with("~/") {
        let home = std::env::var_os("HOME").ok_or("HOME is unavailable for selected path")?;
        let mut path = PathBuf::from(home);
        if value.len() > 2 {
            path.push(&value[2..]);
        }
        Ok(path)
    } else {
        Ok(PathBuf::from(value))
    }
}

fn checked_root(value: &str, label: &str) -> Result<PathBuf> {
    let path = expand_home(value)?;
    if !path.is_absolute() {
        return Err(format!("{label} must be an absolute directory"));
    }
    tos_fd_open::open_absolute_directory(&path)
        .map_err(|_| format!("{label} is not a readable non-symlink directory"))?;
    fs::canonicalize(&path).map_err(|_| format!("{label} cannot be normalized"))
}

fn strict_json(path: &Path, label: &str, cap: u64) -> Result<(Value, Vec<u8>)> {
    let raw = batch::read_bytes(path, None, false, false, cap)
        .map_err(|error| format!("cannot read {label}: {error}"))?;
    let value = batch::parse(&raw).map_err(|error| format!("{label}: {error}"))?;
    Ok((value, raw))
}

fn file_sha(path: &Path, cap: u64) -> Result<(u64, String)> {
    let body = batch::read_bytes(path, None, false, false, cap)
        .map_err(|error| format!("cannot read file {}: {error}", path.display()))?;
    Ok((body.len() as u64, batch::sha(&body)))
}

fn path_leaf(path: &Path) -> Result<&str> {
    path.file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "selected path leaf is not UTF-8".to_owned())
}

fn verify_validation_context(value: &Value, validator_sha256: &str) -> Result<Value> {
    checked_digest(validator_sha256, "validator_sha256")?;
    let object = value
        .as_object()
        .ok_or("explicit validation context is required")?;
    let expected = BTreeSet::from([
        "schema_version",
        "validator_sha256",
        "grammar_root_ref",
        "historical_evidence",
    ]);
    if object.keys().map(String::as_str).collect::<BTreeSet<_>>() != expected
        || value.get("schema_version").and_then(Value::as_str) != Some(VALIDATION_CONTEXT_SCHEMA)
    {
        return Err("validation context has an unexpected schema".into());
    }
    if required_text(value, "validator_sha256")? != validator_sha256 {
        return Err("validation context validator identity differs from selected batch".into());
    }
    let grammar = checked_root(
        required_text(value, "grammar_root_ref")?,
        "validation context grammar root",
    )?;
    let rows = value_at(value, "historical_evidence")?
        .as_array()
        .ok_or("historical validation evidence must be a list")?;

    let metadata = ReadLimits {
        max_manifest_bytes: 64 * 1024 * 1024,
        max_manifest_entries: 250_000,
        max_selected_object_bytes: MAX_CAPTURE_SOURCE_BYTES,
        json: JsonLimits::new(64 * 1024 * 1024, 64, 2_000_000, 4096)
            .map_err(|error| format!("validation JSON limits: {error}"))?,
    };
    let capture_limits = CaptureRestoreLimits {
        metadata,
        max_archive_bytes: MAX_CAPTURE_ARCHIVE_BYTES,
        max_decoded_bytes: MAX_CAPTURE_SOURCE_BYTES,
        max_source_bytes: MAX_CAPTURE_SOURCE_BYTES,
    };
    let deadline = Instant::now() + Duration::from_secs(3600);
    let cancelled = AtomicBool::new(false);
    let mut captures = BTreeSet::new();
    let mut restored_roots = BTreeSet::new();
    let mut binding_rows = Vec::with_capacity(rows.len());

    for row in rows {
        let row_object = row
            .as_object()
            .ok_or("historical validation evidence needs capture and restored root")?;
        if row_object.len() != 2
            || !row_object.contains_key("capture_ref")
            || !row_object.contains_key("restored_root_ref")
        {
            return Err("historical validation evidence needs capture and restored root".into());
        }
        let capture = checked_root(
            required_text(row, "capture_ref")?,
            "historical capture root",
        )?;
        let restored = checked_root(
            required_text(row, "restored_root_ref")?,
            "historical restore root",
        )?;
        if !captures.insert(capture.clone()) || !restored_roots.insert(restored.clone()) {
            return Err("historical validation evidence overlaps another pack".into());
        }

        let capture_path = capture.join("capture.json");
        let members_path = capture.join("members.jsonl");
        let receipt_path = restored.join("restore-receipt.json");
        let (manifest, manifest_raw) = strict_json(
            &capture_path,
            "historical capture manifest",
            metadata.max_manifest_bytes as u64,
        )?;
        let commit = required_text(&manifest, "source_git_commit")?.to_owned();
        let tree = required_text(&manifest, "source_git_tree")?.to_owned();
        let selection = SoftwareCaptureSelectionV1 {
            source_git_commit: commit.clone(),
            source_git_tree: tree,
            capture_manifest_sha256: Digest256::of_bytes(&manifest_raw),
        };
        verify_capture(&capture, &selection, capture_limits, deadline, &cancelled)
            .map_err(|error| format!("historical capture verification failed: {error}"))?;
        let members_sha = required_text(&manifest, "members_sha256")?;
        let (_, members_actual_sha) = file_sha(&members_path, metadata.max_manifest_bytes as u64)?;
        if members_actual_sha != members_sha {
            return Err("historical capture members changed".into());
        }
        let archive_sha = required_text(&manifest, "archive_sha256")?;
        let _reader = SoftwareCaptureReader::open(
            &capture,
            &restored,
            selection.clone(),
            metadata,
            deadline,
            &cancelled,
        )
        .map_err(|error| {
            format!("historical restore receipt does not bind its capture: {error}")
        })?;
        let (receipt, _receipt_raw) = strict_json(
            &receipt_path,
            "historical restore receipt",
            metadata.max_manifest_bytes as u64,
        )?;
        if receipt.get("schema_version").and_then(Value::as_str)
            != Some("tos_corpus_restore_receipt_v1")
            || required_text(&receipt, "source_git_commit")? != commit
            || required_u64(&receipt, "member_count")? != required_u64(&manifest, "member_count")?
            || required_u64(&receipt, "source_bytes")? != required_u64(&manifest, "source_bytes")?
            || required_text(&receipt, "manifest_sha256")?
                != selection.capture_manifest_sha256.to_hex()
        {
            return Err("historical restore receipt does not bind its capture".into());
        }
        let (_, capture_sha) = file_sha(&capture_path, metadata.max_manifest_bytes as u64)?;
        let (_, receipt_sha) = file_sha(&receipt_path, metadata.max_manifest_bytes as u64)?;
        binding_rows.push(json!({
            "capture_ref": capture,
            "capture_manifest_sha256": capture_sha,
            "members_sha256": members_actual_sha,
            "archive_sha256": archive_sha,
            "restored_root_ref": restored,
            "restore_receipt_ref": receipt_path,
            "restore_receipt_sha256": receipt_sha,
        }));
    }

    let grammar_ref = grammar.to_string_lossy().into_owned();
    let mut admission_flags = Map::new();
    admission_flags.insert("grammar_root".into(), Value::String(grammar_ref.clone()));
    if !binding_rows.is_empty() {
        admission_flags.insert(
            "historical_capture".into(),
            Value::Array(
                binding_rows
                    .iter()
                    .map(|row| row["capture_ref"].clone())
                    .collect(),
            ),
        );
        admission_flags.insert(
            "historical_root".into(),
            Value::Array(
                binding_rows
                    .iter()
                    .map(|row| row["restored_root_ref"].clone())
                    .collect(),
            ),
        );
    }
    Ok(json!({
        "schema_version": VALIDATION_CONTEXT_SCHEMA,
        "validator_sha256": validator_sha256,
        "grammar_root_ref": grammar_ref,
        "historical_evidence": binding_rows,
        "admission_flags": admission_flags,
    }))
}

fn request_validation_context(request: &Value) -> Result<Value> {
    if let Some(path) = request
        .get("validation_context_path")
        .and_then(Value::as_str)
    {
        let path = expand_home(path)?;
        let path = if path.is_absolute() {
            path
        } else {
            std::env::current_dir()
                .map_err(|error| format!("cannot resolve validation context path: {error}"))?
                .join(path)
        };
        return strict_json(&path, "validation context", MAX_JSON_BYTES).map(|(value, _)| value);
    }
    value_at(request, "validation_context").cloned()
}

fn valid_run_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() < 16
        || !bytes[..8].iter().all(u8::is_ascii_digit)
        || bytes[8] != b'T'
        || !bytes[9..15].iter().all(u8::is_ascii_digit)
        || bytes[15] != b'Z'
    {
        return false;
    }
    if bytes.len() == 16 {
        return true;
    }
    bytes[16] == b'-' && bytes.len() > 17 && bytes[17..].iter().all(u8::is_ascii_digit)
}

fn expected_records(context: &batch::BatchContext) -> Result<BTreeMap<String, (Value, Value)>> {
    let mut result = BTreeMap::new();
    for (selection, record) in batch::records(context)? {
        let reference = required_text(&record, "ref")?.to_owned();
        result.insert(reference, (selection, record));
    }
    Ok(result)
}

fn payload_key(payload: &Value) -> Result<(String, String, String)> {
    Ok((
        required_text(payload, "item_ref")?.to_owned(),
        required_text(payload, "file_ref")?.to_owned(),
        batch::destination_ref(payload)?,
    ))
}

fn expected_payloads(
    context: &batch::BatchContext,
) -> Result<BTreeMap<(String, String, String), Value>> {
    let mut result = BTreeMap::new();
    for payload in batch::payloads(context)? {
        let key = payload_key(&payload)?;
        if result.insert(key, payload).is_some() {
            return Err("manifest repeats an Item/File/destination binding".into());
        }
    }
    Ok(result)
}

fn load_handoff(root: &Path, handoff_ref: &str) -> Result<(PathBuf, Value, String)> {
    let path = batch::path_under(root, handoff_ref)?;
    let (handoff, raw) = strict_json(&path, "handoff receipt", MAX_JSON_BYTES)?;
    if handoff.get("schema_version").and_then(Value::as_str) != Some(HANDOFF_SCHEMA) {
        return Err("handoff has an unexpected schema".into());
    }
    Ok((path, handoff, batch::sha(&raw)))
}

fn load_selected_context(
    root: &Path,
    handoff: &Value,
    expected_manifest_sha256: &str,
    repo_root: &Path,
) -> Result<batch::BatchContext> {
    checked_digest(expected_manifest_sha256, "caller-selected manifest digest")?;
    let selection = value_at(handoff, "input_selection")?;
    if required_text(selection, "ref")? != "manifest.json" {
        return Err("handoff input selection must name manifest.json".into());
    }
    if required_text(selection, "sha256")? != expected_manifest_sha256 {
        return Err("handoff manifest digest differs from caller-selected digest".into());
    }
    let path = batch::path_under(root, "manifest.json")?;
    batch::load_manifest(&path, repo_root, Some(expected_manifest_sha256))
}

fn verify_item_payload_bindings(
    context: &batch::BatchContext,
    acquisition_root: &Path,
) -> Result<()> {
    batch::verify_prepared_output(context, acquisition_root, false)
}

fn sha_and_metadata(
    path: &Path,
    expected_mode: Option<u32>,
    cap: u64,
    label: &str,
) -> Result<(u64, String)> {
    let body = batch::read_bytes(path, expected_mode, false, false, cap)
        .map_err(|error| format!("{label}: {error}"))?;
    Ok((body.len() as u64, batch::sha(&body)))
}

fn verify_accepted_selected_source(
    accepted_root: &Path,
    snapshot: &tos_source_store::Snapshot,
    reference: &str,
    selected_sha: &str,
) -> Result<()> {
    let relative =
        RelativePath::parse(reference).map_err(|_| "unsafe accepted source reference")?;
    let accepted_path = batch::path_under(accepted_root, reference)?;
    match snapshot.member(&relative) {
        None => match fs::symlink_metadata(&accepted_path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("cannot inspect accepted source path: {error}")),
            Ok(_) => Err(format!(
                "accepted source view contains a path absent from selected base manifest: {reference}"
            )),
        },
        Some(member) => {
            if member.sha256.to_hex() != selected_sha {
                return Err(format!(
                    "handoff would replace accepted source bytes: {reference}"
                ));
            }
            if member.mode != 0o644 {
                return Err(format!(
                    "accepted source mode is unsupported for candidate update: {reference}"
                ));
            }
            let (size, digest) = sha_and_metadata(
                &accepted_path,
                Some(0o644),
                MAX_JSON_BYTES,
                "accepted base source",
            )?;
            if size != member.size_bytes || digest != member.sha256.to_hex() {
                return Err(format!(
                    "accepted source view differs from base manifest: {reference}"
                ));
            }
            Ok(())
        }
    }
}

fn verify_source_records(
    root: &Path,
    handoff: &Value,
    context: &batch::BatchContext,
    accepted: Option<(&Path, &tos_source_store::Snapshot)>,
) -> Result<Vec<Value>> {
    let expected = expected_records(context)?;
    let rows = value_at(handoff, "source_records")?
        .as_array()
        .ok_or("handoff source record closure differs from manifest")?;
    if rows.len() != expected.len() {
        return Err("handoff source record closure differs from manifest".into());
    }
    let mut seen = BTreeSet::new();
    let mut selected = Vec::with_capacity(rows.len());
    for row in rows {
        let reference = required_text(row, "ref")?;
        if !seen.insert(reference.to_owned()) {
            return Err("handoff source record closure differs from manifest".into());
        }
        let (selection, record) = expected
            .get(reference)
            .ok_or_else(|| format!("handoff contains an unselected source record: {reference}"))?;
        if required_text(row, "item_ref")? != required_text(selection, "item_ref")?
            || required_text(row, "kind")? != required_text(record, "kind")?
        {
            return Err(format!("handoff source identity differs: {reference}"));
        }
        if required_text(row, "handoff_ref")? != format!("source/{reference}")
            || required_text(row, "sha256")? != required_text(record, "sha256")?
        {
            return Err(format!(
                "handoff source digest binding differs: {reference}"
            ));
        }
        let rights = value_at(selection, "rights")?;
        if required_text(row, "rights_ref")? != required_text(rights, "ref")?
            || required_text(row, "rights_sha256")? != required_text(rights, "sha256")?
        {
            return Err(format!("handoff rights binding differs: {reference}"));
        }
        let source = batch::path_under(root, required_text(row, "handoff_ref")?)?;
        let (size, digest) = sha_and_metadata(
            &source,
            Some(0o644),
            MAX_JSON_BYTES,
            "handoff selected source",
        )?;
        if size != required_u64(row, "byte_size")? {
            return Err(format!("handoff source size differs: {reference}"));
        }
        if digest != required_text(record, "sha256")? {
            return Err(format!("handoff source bytes differ: {reference}"));
        }
        if let Some((accepted_root, snapshot)) = accepted {
            verify_accepted_selected_source(accepted_root, snapshot, reference, &digest)?;
        }
        selected.push(row.clone());
    }
    if seen.len() != expected.len() {
        return Err("handoff source record closure differs from manifest".into());
    }
    Ok(selected)
}

fn verify_delta(
    root: &Path,
    handoff: &Value,
    context: &batch::BatchContext,
    expected_base: &str,
) -> Result<()> {
    let provenance = value_at(handoff, "provenance_delta")?;
    let delta_ref = batch::provenance_delta_ref(context);
    if required_text(provenance, "ref")? != delta_ref {
        return Err("handoff provenance delta reference is not deterministic".into());
    }
    let path = batch::path_under(root, &delta_ref)?;
    let (_, digest) = file_sha(&path, MAX_JSON_BYTES)?;
    if required_text(provenance, "sha256")? != digest {
        return Err("handoff provenance delta digest differs".into());
    }
    let selection_delta = value_at(&context.manifest, "provenance_delta")?;
    if required_text(provenance, "event_ref")? != required_text(selection_delta, "event_ref")? {
        return Err("handoff provenance event differs".into());
    }
    if required_text(provenance, "base_revision")? != expected_base
        || required_text(provenance, "base_revision")?
            != required_text(&context.manifest, "base_revision")?
    {
        return Err("handoff provenance delta base differs from selected accepted base".into());
    }
    Ok(())
}

fn parse_fixity_rows(path: &Path) -> Result<Vec<Value>> {
    let raw = batch::read_bytes(path, None, false, false, MAX_JSON_BYTES)
        .map_err(|error| format!("cannot read fixity JSONL: {error}"))?;
    let mut rows = Vec::new();
    if raw.is_empty() {
        return Ok(rows);
    }
    for line in raw.split_inclusive(|byte| *byte == b'\n') {
        if !line.ends_with(b"\n") {
            return Err("fixity JSONL is malformed: line is not newline terminated".into());
        }
        let value = batch::parse(&line[..line.len() - 1])
            .map_err(|error| format!("fixity JSONL is malformed: {error}"))?;
        if batch::canonical(&value)? != line {
            return Err("fixity JSONL is malformed: row is not canonical JSON".into());
        }
        rows.push(value);
    }
    Ok(rows)
}

fn verify_payload_destination(path: &Path, payload: &Value) -> Result<Value> {
    // Historical handoffs were readable without imposing the producer's
    // current-user owner check. Keep the immutable legacy custody rule:
    // read-only bytes, one link, exact size and both content digests.
    let digest = batch::digest_file(path, Some(0o444), false, true, MAX_PAYLOAD_BYTES)?;
    if digest["byte_size"] != payload["byte_size"]
        || digest["sha256"] != payload["sha256"]
        || payload
            .get("git_blob_sha1")
            .and_then(Value::as_str)
            .is_some_and(|expected| digest["git_blob_sha1"].as_str() != Some(expected))
    {
        return Err(format!(
            "payload custody differs: {}",
            required_text(payload, "file_ref")?
        ));
    }
    Ok(digest)
}

fn verify_fixity_and_custody(
    root: &Path,
    handoff: &Value,
    context: &batch::BatchContext,
    run_id: &str,
) -> Result<Vec<Value>> {
    let fixity = value_at(handoff, "independent_fixity")?;
    let fixity_ref = required_text(fixity, "ref")?;
    let summary_ref = required_text(fixity, "summary_ref")?;
    let expected_fixity_ref = format!("receipts/fixity-{run_id}.jsonl");
    let expected_summary_ref = format!("receipts/fixity-{run_id}.json");
    if fixity_ref != expected_fixity_ref || summary_ref != expected_summary_ref {
        return Err("handoff fixity references are not bound to its run".into());
    }
    let fixity_path = batch::path_under(root, fixity_ref)?;
    let summary_path = batch::path_under(root, summary_ref)?;
    let (fixity_size, fixity_sha) = file_sha(&fixity_path, MAX_JSON_BYTES)?;
    let (summary_size, summary_sha) = file_sha(&summary_path, MAX_JSON_BYTES)?;
    let declared_jsonl = fixity
        .get("jsonl_sha256")
        .or_else(|| fixity.get("sha256"))
        .and_then(Value::as_str)
        .ok_or("handoff fixity JSONL digest is missing")?;
    if declared_jsonl != fixity_sha
        || required_text(fixity, "sha256")? != fixity_sha
        || required_text(fixity, "summary_sha256")? != summary_sha
    {
        return Err("handoff fixity digest differs".into());
    }
    let (summary, _) = strict_json(&summary_path, "fixity summary", MAX_JSON_BYTES)?;
    let expected = expected_payloads(context)?;
    if summary.get("schema_version").and_then(Value::as_str) != Some(FIXITY_SCHEMA)
        || required_text(&summary, "batch_id")? != required_text(&context.manifest, "batch_id")?
        || required_text(&summary, "run_id")? != run_id
        || required_text(&summary, "manifest_sha256")? != context.manifest_sha256
        || required_text(&summary, "fixity_jsonl_ref")? != fixity_ref
        || required_text(&summary, "fixity_jsonl_sha256")? != fixity_sha
        || summary.get("independent_pass") != Some(&Value::Bool(true))
        || required_u64(&summary, "rows")? != expected.len() as u64
        || required_u64(&summary, "verified")? != expected.len() as u64
        || required_u64(&summary, "invalid")? != 0
    {
        return Err("fixity summary does not bind complete handoff".into());
    }
    let fixity_rows = parse_fixity_rows(&fixity_path)?;
    if fixity_rows.len() != expected.len() {
        return Err("fixity rows do not close over selected payloads".into());
    }
    let mut seen = BTreeSet::new();
    for row in &fixity_rows {
        let key = payload_key(row).map_err(|_| "fixity contains an unselected payload")?;
        if !seen.insert(key.clone()) {
            return Err("fixity rows do not close over selected payloads".into());
        }
        let payload = expected
            .get(&key)
            .ok_or("fixity contains an unselected payload")?;
        if required_text(row, "status")? != "verified" {
            return Err("fixity contains an unverified payload row".into());
        }
        let destination_ref = batch::destination_ref(payload)?;
        for (key, expected_value) in [
            ("item_ref", required_text(payload, "item_ref")?),
            ("destination_ref", destination_ref.as_str()),
            ("relative_path", required_text(payload, "relative_path")?),
            (
                "provider_revision",
                required_text(payload, "provider_revision")?,
            ),
            (
                "provider_source_id",
                required_text(payload, "provider_source_id")?,
            ),
            ("expected_sha256", required_text(payload, "sha256")?),
        ] {
            if required_text(row, key)? != expected_value {
                return Err(format!(
                    "fixity row binding differs: {}",
                    required_text(payload, "file_ref")?
                ));
            }
        }
        let expected_size = required_u64(payload, "byte_size")?;
        if required_u64(row, "expected_byte_size")? != expected_size
            || required_u64(row, "byte_size")? != expected_size
            || required_text(row, "sha256")? != required_text(payload, "sha256")?
        {
            return Err(format!(
                "fixity row binding differs: {}",
                required_text(payload, "file_ref")?
            ));
        }
        let source = batch::payload_path(&root.join("payload"), payload)?;
        let digest = verify_payload_destination(&source, payload)?;
        if required_text(row, "git_blob_sha1")? != required_text(&digest, "git_blob_sha1")? {
            return Err(format!(
                "fixity Git blob digest differs: {}",
                required_text(payload, "file_ref")?
            ));
        }
    }
    if seen != expected.keys().cloned().collect() {
        return Err("fixity rows do not close over selected payloads".into());
    }

    let custody_rows = value_at(handoff, "payload_custody")?
        .as_array()
        .ok_or("handoff payload custody closure differs from manifest")?;
    if custody_rows.len() != expected.len() {
        return Err("handoff payload custody closure differs from manifest".into());
    }
    let mut custody_seen = BTreeSet::new();
    for row in custody_rows {
        let key =
            payload_key(row).map_err(|_| "handoff payload custody contains an unselected file")?;
        if !custody_seen.insert(key.clone()) {
            return Err("handoff payload custody closure differs from manifest".into());
        }
        let payload = expected
            .get(&key)
            .ok_or("handoff payload custody contains an unselected file")?;
        if !matches!(
            required_text(row, "status")?,
            "acquired" | "already_present"
        ) {
            return Err("handoff payload row is not acquired custody".into());
        }
        let destination_ref = batch::destination_ref(payload)?;
        let bindings = [
            ("run_id", run_id),
            ("batch_id", required_text(&context.manifest, "batch_id")?),
            ("manifest_sha256", context.manifest_sha256.as_str()),
            ("item_ref", required_text(payload, "item_ref")?),
            ("file_ref", required_text(payload, "file_ref")?),
            ("destination_ref", destination_ref.as_str()),
            ("provider_url", required_text(payload, "provider_url")?),
            (
                "provider_revision",
                required_text(payload, "provider_revision")?,
            ),
            (
                "provider_source_id",
                required_text(payload, "provider_source_id")?,
            ),
            ("expected_sha256", required_text(payload, "sha256")?),
        ];
        for (field, expected_value) in bindings {
            if required_text(row, field)? != expected_value {
                return Err(format!(
                    "handoff payload digest binding differs: {}",
                    required_text(payload, "file_ref")?
                ));
            }
        }
        let expected_size = required_u64(payload, "byte_size")?;
        if required_u64(row, "expected_byte_size")? != expected_size {
            return Err(format!(
                "handoff payload digest binding differs: {}",
                required_text(payload, "file_ref")?
            ));
        }
    }
    if custody_seen != expected.keys().cloned().collect() {
        return Err("handoff payload custody closure differs from manifest".into());
    }
    let _ = (fixity_size, summary_size);
    Ok(expected.into_values().collect())
}

fn verify_handoff(
    acquisition_root: &Path,
    handoff_path: &Path,
    handoff: &Value,
    context: &batch::BatchContext,
    expected_base_revision: &str,
    accepted: Option<(&Path, &tos_source_store::Snapshot)>,
) -> Result<(Vec<Value>, Vec<Value>)> {
    if handoff.get("batch_id") != context.manifest.get("batch_id")
        || handoff.get("batch_revision") != context.manifest.get("batch_revision")
    {
        return Err("handoff batch differs from its manifest".into());
    }
    if required_text(handoff, "base_revision")? != expected_base_revision {
        return Err("handoff base revision differs from selected accepted base".into());
    }
    if required_text(handoff, "base_revision")?
        != required_text(&context.manifest, "base_revision")?
    {
        return Err("handoff base revision differs from batch manifest".into());
    }
    if required_text(handoff, "acquisition_status")? != "acquired-not-admitted" {
        return Err("handoff is not a complete acquired-not-admitted transfer".into());
    }
    if required_text(handoff, "admission_status")? != "not-admitted"
        || required_text(handoff, "publication_status")? != "not-published"
    {
        return Err("handoff crosses the acquisition authority boundary".into());
    }
    if required_u64(handoff, "topology_preimages")? != 0 {
        return Err("handoff contains a topology preimage claim".into());
    }
    let run_id = required_text(handoff, "run_id")?;
    if !valid_run_id(run_id) || path_leaf(handoff_path)? != format!("handoff-{run_id}.json") {
        return Err("handoff path is not bound to its run identity".into());
    }
    let input_selection = value_at(handoff, "input_selection")?;
    if required_text(input_selection, "ref")? != "manifest.json"
        || required_text(input_selection, "sha256")? != context.manifest_sha256
    {
        return Err("handoff manifest digest differs".into());
    }
    let selected_source_rows = verify_source_records(acquisition_root, handoff, context, accepted)?;
    verify_delta(acquisition_root, handoff, context, expected_base_revision)?;
    let payloads = verify_fixity_and_custody(acquisition_root, handoff, context, run_id)?;
    Ok((selected_source_rows, payloads))
}

fn read_limits() -> Result<ReadLimits> {
    Ok(ReadLimits {
        max_manifest_bytes: 128 * 1024 * 1024,
        max_manifest_entries: 1_000_000,
        max_selected_object_bytes: 2 * 1024 * 1024 * 1024,
        json: JsonLimits::new(128 * 1024 * 1024, 64, 4_000_000, 4096)
            .map_err(|error| format!("accepted snapshot JSON limits: {error}"))?,
    })
}

fn accepted_snapshot(
    store_root: &Path,
    base_revision: &str,
) -> Result<(CorpusReader, tos_source_store::Snapshot)> {
    let digest = checked_digest(base_revision, "base_revision")?;
    let reader = CorpusReader::open_existing(store_root, read_limits()?)
        .map_err(|error| format!("accepted corpus store is unavailable: {error}"))?;
    if reader
        .select_current()
        .map_err(|error| format!("accepted corpus pointer is invalid: {error}"))?
        != Some(SourceRevision(digest))
    {
        return Err("accepted corpus pointer is not the selected base revision".into());
    }
    let snapshot = reader.load_exact(SourceRevision(digest)).map_err(|error| {
        format!("accepted corpus manifest is not the selected base revision: {error}")
    })?;
    Ok((reader, snapshot))
}

fn assert_path_absent(path: &Path, label: &str) -> Result<()> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("cannot inspect {label}: {error}")),
        Ok(_) => Err(format!("{label} must be new: {}", path.display())),
    }
}

fn create_stage(output: &Path) -> Result<Stage> {
    if !output.is_absolute() {
        return Err("adapter output must be a new absolute path".into());
    }
    assert_path_absent(output, "adapter output")?;
    let parent = output.parent().ok_or("adapter output parent is missing")?;
    let parent_file = tos_fd_open::open_absolute_directory(parent).map_err(|_| {
        format!(
            "adapter output parent must be a real directory: {}",
            parent.display()
        )
    })?;
    let leaf = path_leaf(output)?;
    for attempt in 0..8 {
        let mut random = [0u8; 24];
        File::open("/dev/urandom")
            .and_then(|mut file| file.read_exact(&mut random))
            .map_err(|error| format!("cannot create adapter staging directory: {error}"))?;
        let name = format!(
            ".{leaf}.adapter-{}-{attempt}",
            Digest256::of_bytes(&random).to_hex()
        );
        match rustix::fs::mkdirat(
            &parent_file,
            name.as_str(),
            rustix::fs::Mode::from_raw_mode(0o700),
        ) {
            Ok(()) => {
                let directory = tos_fd_open::open_directory_at(&parent_file, Path::new(&name))
                    .map_err(|error| format!("cannot open adapter staging directory: {error}"))?;
                let metadata = directory.metadata().map_err(|error| error.to_string())?;
                return Ok(Stage {
                    path: parent.join(&name),
                    parent: parent.to_path_buf(),
                    name,
                    directory,
                    dev: metadata.dev(),
                    ino: metadata.ino(),
                    published: false,
                });
            }
            Err(error) if error == rustix::io::Errno::EXIST => continue,
            Err(error) => {
                return Err(format!(
                    "cannot create adapter staging directory under {}: {error}",
                    parent.display()
                ));
            }
        }
    }
    Err("cannot reserve a unique adapter staging directory".into())
}

struct Stage {
    path: PathBuf,
    parent: PathBuf,
    name: String,
    directory: File,
    dev: u64,
    ino: u64,
    published: bool,
}

impl Stage {
    fn publish(&mut self, output: &Path) -> Result<()> {
        if output.parent() != Some(self.parent.as_path()) {
            return Err("adapter output parent changed".into());
        }
        let parent = tos_fd_open::open_absolute_directory(&self.parent)
            .map_err(|error| format!("adapter output parent changed: {error}"))?;
        let current = tos_fd_open::open_directory_at(&parent, Path::new(&self.name))
            .map_err(|error| format!("adapter staging path changed: {error}"))?;
        let now = current.metadata().map_err(|error| error.to_string())?;
        let held = self
            .directory
            .metadata()
            .map_err(|error| error.to_string())?;
        if now.dev() != self.dev
            || now.ino() != self.ino
            || held.dev() != self.dev
            || held.ino() != self.ino
        {
            return Err("adapter staging path changed".into());
        }
        let output_leaf = output.file_name().ok_or("adapter output leaf is missing")?;
        rustix::fs::renameat_with(
            &parent,
            self.name.as_str(),
            &parent,
            output_leaf,
            rustix::fs::RenameFlags::NOREPLACE,
        )
        .map_err(|error| format!("cannot publish adapter output without replacement: {error}"))?;
        parent.sync_all().map_err(|error| error.to_string())?;
        self.published = true;
        Ok(())
    }
}

impl Drop for Stage {
    fn drop(&mut self) {
        if self.published {
            return;
        }
        if let Ok(parent) = tos_fd_open::open_absolute_directory(&self.parent) {
            if let Ok(current) = tos_fd_open::open_directory_at(&parent, Path::new(&self.name)) {
                if let Ok(metadata) = current.metadata() {
                    if metadata.dev() == self.dev && metadata.ino() == self.ino {
                        let _ = fs::remove_dir_all(&self.path);
                    }
                }
            }
        }
    }
}

fn file_stamp(metadata: &std::fs::Metadata) -> (u64, u64, u64, u32, u64, i64, i64, i64, i64) {
    (
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.mode(),
        metadata.nlink(),
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec(),
    )
}

fn copy_no_clobber(
    source: &Path,
    destination: &Path,
    expected_size: u64,
    expected_sha256: &str,
    source_mode: u32,
    destination_mode: u32,
    source_owner: bool,
    source_single_link: bool,
) -> Result<()> {
    if !is_digest(expected_sha256) {
        return Err("copy requires lowercase SHA-256".into());
    }
    let mut input = tos_fd_open::open_absolute_regular(source, expected_size)
        .map_err(|error| format!("handoff source is not a regular file: {error}"))?;
    let before = input.metadata().map_err(|error| error.to_string())?;
    if before.len() != expected_size
        || before.mode() & 0o7777 != source_mode
        || (source_owner && before.uid() != rustix::process::geteuid().as_raw())
        || (source_single_link && before.nlink() != 1)
    {
        return Err(format!(
            "handoff source custody differs: {}",
            source.display()
        ));
    }
    if let Some(parent_path) = destination.parent() {
        fs::create_dir_all(parent_path).map_err(|error| {
            format!(
                "cannot create candidate directory {}: {error}",
                parent_path.display()
            )
        })?;
    }
    let parent_path = destination
        .parent()
        .ok_or("candidate destination has no parent")?;
    let parent = tos_fd_open::open_absolute_directory(parent_path)
        .map_err(|error| format!("candidate directory is not a real directory: {error}"))?;
    let leaf = destination
        .file_name()
        .ok_or("candidate destination has no leaf")?;
    let mut random = [0u8; 16];
    File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut random))
        .map_err(|error| format!("cannot reserve candidate file: {error}"))?;
    let temporary_name = format!(
        ".adapter-{}-{}",
        std::process::id(),
        Digest256::of_bytes(&random).to_hex()
    );
    let fd = rustix::fs::openat(
        &parent,
        temporary_name.as_str(),
        rustix::fs::OFlags::WRONLY
            | rustix::fs::OFlags::CREATE
            | rustix::fs::OFlags::EXCL
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::from_raw_mode(0o600),
    )
    .map_err(|error| format!("cannot create candidate temporary file: {error}"))?;
    let mut output = File::from(fd);
    let header = format!("blob {expected_size}\0");
    let mut sha1 = Sha1::new();
    sha1.update(header.as_bytes());
    let mut sha256 = Digest256Hasher::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    let operation = (|| -> Result<()> {
        loop {
            let count = input.read(&mut buffer).map_err(|error| error.to_string())?;
            if count == 0 {
                break;
            }
            total = total
                .checked_add(count as u64)
                .filter(|size| *size <= expected_size)
                .ok_or("handoff source grew during copy")?;
            output
                .write_all(&buffer[..count])
                .map_err(|error| error.to_string())?;
            sha256.update(&buffer[..count]);
            sha1.update(&buffer[..count]);
        }
        if total != expected_size || sha256.finalize().to_hex() != expected_sha256 {
            return Err(format!(
                "handoff source fixity differs: {}",
                source.display()
            ));
        }
        let after = input.metadata().map_err(|error| error.to_string())?;
        let named = tos_fd_open::open_absolute_regular(source, expected_size)
            .map_err(|error| format!("handoff source path changed: {error}"))?
            .metadata()
            .map_err(|error| error.to_string())?;
        if file_stamp(&before) != file_stamp(&after) || file_stamp(&after) != file_stamp(&named) {
            return Err(format!(
                "handoff source changed during copy: {}",
                source.display()
            ));
        }
        rustix::fs::fchmod(&output, rustix::fs::Mode::from_raw_mode(destination_mode))
            .map_err(|error| error.to_string())?;
        output.sync_all().map_err(|error| error.to_string())?;
        match rustix::fs::linkat(
            &parent,
            temporary_name.as_str(),
            &parent,
            leaf,
            rustix::fs::AtFlags::empty(),
        ) {
            Ok(()) => (),
            Err(error) if error == rustix::io::Errno::EXIST => {
                return Err(format!(
                    "candidate destination conflicts: {}",
                    destination.display()
                ));
            }
            Err(error) => return Err(format!("cannot publish candidate member: {error}")),
        }
        rustix::fs::unlinkat(
            &parent,
            temporary_name.as_str(),
            rustix::fs::AtFlags::empty(),
        )
        .map_err(|error| error.to_string())?;
        parent.sync_all().map_err(|error| error.to_string())?;
        let digest = batch::digest_file(
            destination,
            Some(destination_mode),
            true,
            true,
            expected_size,
        )?;
        if required_u64(&digest, "byte_size")? != expected_size
            || required_text(&digest, "sha256")? != expected_sha256
        {
            return Err(format!(
                "candidate destination fixity differs: {}",
                destination.display()
            ));
        }
        let git_blob = format!("{:x}", sha1.finalize());
        if required_text(&digest, "git_blob_sha1")? != git_blob {
            return Err(format!(
                "candidate destination Git blob digest differs: {}",
                destination.display()
            ));
        }
        Ok(())
    })();
    let _ = rustix::fs::unlinkat(
        &parent,
        temporary_name.as_str(),
        rustix::fs::AtFlags::empty(),
    );
    operation
}

fn create_candidate_roots(stage: &Path) -> Result<()> {
    for relative in [
        "source",
        "payload",
        "receipts",
        "receipts/acquisition-evidence",
    ] {
        let path = stage.join(relative);
        fs::create_dir(&path).map_err(|error| {
            format!(
                "cannot create candidate directory {}: {error}",
                path.display()
            )
        })?;
        let directory = tos_fd_open::open_absolute_directory(&path)
            .map_err(|error| format!("candidate directory is not safe: {error}"))?;
        rustix::fs::fchmod(&directory, rustix::fs::Mode::from_raw_mode(0o755))
            .map_err(|error| format!("cannot set candidate directory mode: {error}"))?;
    }
    Ok(())
}

fn safe_batch_slug(batch_id: &str) -> Result<&str> {
    let slug = batch_id
        .strip_prefix("tos.acquisition-batch.")
        .ok_or("batch id cannot form a safe candidate manifest name")?;
    let bytes = slug.as_bytes();
    if bytes.is_empty()
        || !bytes[0].is_ascii_lowercase() && !bytes[0].is_ascii_digit()
        || !bytes.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'.' || *byte == b'-'
        })
    {
        return Err("batch id cannot form a safe candidate manifest name".into());
    }
    Ok(slug)
}

fn read_batch_candidate(path: &Path, source_root: &Path) -> Result<()> {
    let json_limits = JsonLimits::new(16 * 1024 * 1024, 64, 200_000, 4096)
        .map_err(|error| format!("candidate batch JSON limits: {error}"))?;
    let limits = AdmissionLimits {
        max_batch_bytes: 16 * 1024 * 1024,
        max_members: 100_000,
        max_member_bytes: MAX_PAYLOAD_BYTES,
        max_source_bytes: 1024 * 1024 * 1024 * 1024,
        json: json_limits,
    };
    let cancelled = AtomicBool::new(false);
    AdmissionBatch::read(
        path,
        source_root,
        limits,
        Instant::now() + Duration::from_secs(60),
        &cancelled,
    )
    .map(|_| ())
    .map_err(|error| format!("candidate tos_corpus_batch_v1 rejected by corpus consumer: {error}"))
}

fn adapt_handoff(request: &Value) -> Result<Value> {
    let base_revision = required_text(request, "base_revision")?;
    checked_digest(base_revision, "base_revision")?;
    let validator_sha256 = required_text(request, "validator_sha256")?;
    checked_digest(validator_sha256, "validator_sha256")?;
    let validation = request_validation_context(request)?;
    let validation_binding = verify_validation_context(&validation, validator_sha256)?;

    let acquisition_root = checked_root(
        required_text(request, "acquisition_root")?,
        "acquisition root",
    )?;
    let accepted_store = checked_root(
        required_text(request, "accepted_store_root")?,
        "accepted store root",
    )?;
    let accepted_source = checked_root(
        required_text(request, "accepted_source_root")?,
        "accepted source root",
    )?;
    let output = expand_home(required_text(request, "output_root")?)?;
    if !output.is_absolute() {
        return Err("adapter output must be a new absolute path".into());
    }
    if output
        .parent()
        .is_none_or(|parent| tos_fd_open::open_absolute_directory(parent).is_err())
    {
        return Err(format!(
            "adapter output parent must be a regular directory: {}",
            output.parent().unwrap_or(Path::new(".")).display()
        ));
    }

    let (accepted_reader, accepted_snapshot) = accepted_snapshot(&accepted_store, base_revision)?;
    let accepted_manifest_ref = format!("revisions/{base_revision}/snapshot.json");
    let accepted_manifest_path = batch::path_under(&accepted_store, &accepted_manifest_ref)?;
    let (accepted_manifest_size, accepted_manifest_sha) =
        file_sha(&accepted_manifest_path, 128 * 1024 * 1024)?;
    let _ = accepted_manifest_size;
    let accepted_validator_sha256 = accepted_snapshot.validator_sha256().to_hex();
    let validator_transition = if accepted_validator_sha256 == validator_sha256 {
        "aligned"
    } else {
        "explicit-grammar-update-required"
    };
    let admission_preflight = if validator_transition == "aligned" {
        "not-run-transport-only"
    } else {
        validator_transition
    };

    let (handoff_path, handoff, handoff_sha256) =
        load_handoff(&acquisition_root, required_text(request, "handoff_ref")?)?;
    let repo_root = checked_root(required_text(request, "repo_root")?, "repository root")?;
    let context = load_selected_context(
        &acquisition_root,
        &handoff,
        required_text(request, "expected_manifest_sha256")?,
        &repo_root,
    )?;
    verify_item_payload_bindings(&context, &acquisition_root)?;
    let manifest_path = batch::path_under(&acquisition_root, "manifest.json")?;
    let (selected_source_rows, payloads) = verify_handoff(
        &acquisition_root,
        &handoff_path,
        &handoff,
        &context,
        base_revision,
        Some((&accepted_source, &accepted_snapshot)),
    )?;
    let _ = (&accepted_reader, &manifest_path);

    let mut stage = create_stage(&output)?;
    create_candidate_roots(&stage.path)?;
    let source_root = stage.path.join("source");
    let payload_root = stage.path.join("payload");
    let receipts_root = stage.path.join("receipts");
    let evidence_root = receipts_root.join("acquisition-evidence");
    let mut updates = Vec::with_capacity(selected_source_rows.len());
    let mut sorted_source_rows = selected_source_rows.clone();
    sorted_source_rows.sort_by_key(|row| row["ref"].as_str().unwrap_or_default().to_owned());
    for row in &sorted_source_rows {
        let reference = required_text(row, "ref")?;
        let source = batch::path_under(&acquisition_root, required_text(row, "handoff_ref")?)?;
        let destination = batch::path_under(&source_root, reference)?;
        copy_no_clobber(
            &source,
            &destination,
            required_u64(row, "byte_size")?,
            required_text(row, "sha256")?,
            0o644,
            0o644,
            false,
            false,
        )?;
        updates.push(json!({
            "path": reference,
            "sha256": required_text(row, "sha256")?,
            "size_bytes": required_u64(row, "byte_size")?,
            "mode": 0o644,
        }));
    }
    for payload in &payloads {
        let source = batch::payload_path(&acquisition_root.join("payload"), payload)?;
        let destination = batch::payload_path(&payload_root, payload)?;
        copy_no_clobber(
            &source,
            &destination,
            required_u64(payload, "byte_size")?,
            required_text(payload, "sha256")?,
            0o444,
            0o444,
            false,
            true,
        )?;
        let _ = batch::verify_destination(&destination, payload)?;
    }

    let run_id = required_text(&handoff, "run_id")?;
    let delta_ref = batch::provenance_delta_ref(&context);
    let fixity = value_at(&handoff, "independent_fixity")?;
    let evidence_sources = [
        (
            "manifest.json",
            manifest_path,
            context.manifest_sha256.clone(),
        ),
        ("handoff.json", handoff_path.clone(), handoff_sha256.clone()),
        (
            "provenance-delta.json",
            batch::path_under(&acquisition_root, &delta_ref)?,
            required_text(value_at(&handoff, "provenance_delta")?, "sha256")?.to_owned(),
        ),
        (
            "fixity.jsonl",
            batch::path_under(&acquisition_root, required_text(fixity, "ref")?)?,
            required_text(fixity, "jsonl_sha256")?.to_owned(),
        ),
        (
            "fixity-summary.json",
            batch::path_under(&acquisition_root, required_text(fixity, "summary_ref")?)?,
            required_text(fixity, "summary_sha256")?.to_owned(),
        ),
    ];
    let mut evidence_refs = Map::new();
    for (name, source, digest) in evidence_sources {
        let destination = evidence_root.join(name);
        let info = tos_fd_open::open_absolute_regular(&source, MAX_JSON_BYTES)
            .map_err(|error| format!("handoff evidence {name} is not a regular file: {error}"))?
            .metadata()
            .map_err(|error| error.to_string())?;
        copy_no_clobber(
            &source,
            &destination,
            info.len(),
            &digest,
            info.mode() & 0o7777,
            0o644,
            false,
            false,
        )?;
        let source_ref = source
            .strip_prefix(&acquisition_root)
            .map_err(|_| "handoff evidence source escaped acquisition root")?
            .to_str()
            .ok_or("handoff evidence source ref is not UTF-8")?;
        evidence_refs.insert(
            name.to_owned(),
            json!({
                "ref": destination.strip_prefix(&stage.path).map_err(|_| "evidence path escaped staging")?.to_str().ok_or("evidence ref is not UTF-8")?,
                "source_ref": source_ref,
                "sha256": digest,
            }),
        );
    }

    let batch_value = json!({
        "schema_version": "tos_corpus_batch_v1",
        "base_revision": base_revision,
        "validator_sha256": validator_sha256,
        "updates": updates,
        "retirements": [],
    });
    let slug = safe_batch_slug(required_text(&context.manifest, "batch_id")?)?;
    let batch_ref = format!("manifests/tos-corpus-batch-{slug}.json");
    let batch_path = stage.path.join(&batch_ref);
    let batch_raw = batch::canonical(&batch_value)?;
    batch::publish(&batch_path, &batch_raw, 0o644)?;
    read_batch_candidate(&batch_path, &source_root)?;

    let validation_context_bytes = batch::canonical(&validation_binding)?;
    let validation_context_sha256 = batch::sha(&validation_context_bytes);
    let validation_context_ref = "receipts/validation-context.json";
    batch::publish(
        &stage.path.join(validation_context_ref),
        &validation_context_bytes,
        0o644,
    )?;
    let candidate_batch_sha256 = file_sha(&batch_path, MAX_JSON_BYTES)?.1;
    let handoff_relative = handoff_path
        .strip_prefix(&acquisition_root)
        .map_err(|_| "handoff path escaped acquisition root")?
        .to_str()
        .ok_or("handoff path is not UTF-8")?;
    let adapter_receipt = json!({
        "schema_version": "tos_acquisition_handoff_adapter_receipt_v1",
        "handoff_ref": evidence_refs["handoff.json"]["ref"],
        "handoff_source_ref": handoff_relative,
        "handoff_sha256": handoff_sha256,
        "manifest_ref": evidence_refs["manifest.json"]["ref"],
        "manifest_source_ref": "manifest.json",
        "manifest_sha256": context.manifest_sha256,
        "caller_expected_manifest_sha256": required_text(request, "expected_manifest_sha256")?,
        "provenance_delta_ref": evidence_refs["provenance-delta.json"]["ref"],
        "provenance_delta_source_ref": required_text(value_at(&handoff, "provenance_delta")?, "ref")?,
        "provenance_delta_sha256": required_text(value_at(&handoff, "provenance_delta")?, "sha256")?,
        "fixity_ref": evidence_refs["fixity.jsonl"]["ref"],
        "fixity_source_ref": required_text(fixity, "ref")?,
        "fixity_jsonl_sha256": required_text(fixity, "jsonl_sha256")?,
        "fixity_summary_ref": evidence_refs["fixity-summary.json"]["ref"],
        "fixity_summary_source_ref": required_text(fixity, "summary_ref")?,
        "fixity_summary_sha256": required_text(fixity, "summary_sha256")?,
        "base_revision": base_revision,
        "accepted_manifest_ref": accepted_manifest_ref,
        "accepted_manifest_sha256": accepted_manifest_sha,
        "accepted_validator_sha256": accepted_validator_sha256,
        "validator_sha256": validator_sha256,
        "validator_transition": validator_transition,
        "validation_context_ref": validation_context_ref,
        "validation_context_sha256": validation_context_sha256,
        "evidence": evidence_refs,
        "candidate_batch_ref": batch_ref,
        "candidate_batch_sha256": candidate_batch_sha256,
        "input_root": "source",
        "payload_source_root": "payload",
        "admission_status": "not-admitted",
        "admission_preflight": admission_preflight,
        "validation_context_posture": "transport-bound; downstream-source-validator-required",
        "publication_status": "not-published",
        "topology_preimages": 0,
        "authority_boundary": "validated private batch input only; corpus admission remains with corpus_admit and its selected store",
    });
    batch::publish(
        &stage.path.join("receipts/acquisition-handoff-adapter.json"),
        &batch::canonical(&adapter_receipt)?,
        0o644,
    )?;
    stage.publish(&output)?;
    Ok(json!({
        "status": "candidate-not-admitted",
        "output_root": output,
        "candidate_batch_ref": batch_ref,
        "candidate_batch_sha256": candidate_batch_sha256,
        "admission_status": "not-admitted",
        "admission_preflight": admission_preflight,
    }))
}

fn verify_for_intake(request: &Value) -> Result<Value> {
    let root = checked_root(
        required_text(request, "acquisition_root")?,
        "acquisition root",
    )?;
    let repo_root = checked_root(required_text(request, "repo_root")?, "repository root")?;
    let expected_base = required_text(request, "expected_base_revision")?;
    checked_digest(expected_base, "expected_base_revision")?;
    let expected_manifest = required_text(request, "expected_manifest_sha256")?;
    checked_digest(expected_manifest, "caller-selected manifest digest")?;
    let (handoff_path, handoff, _) = load_handoff(&root, required_text(request, "handoff_ref")?)?;
    let context = load_selected_context(&root, &handoff, expected_manifest, &repo_root)?;
    verify_item_payload_bindings(&context, &root)?;
    let (selected_source_rows, payloads) = verify_handoff(
        &root,
        &handoff_path,
        &handoff,
        &context,
        expected_base,
        None,
    )?;
    Ok(json!({
        "root": root,
        "handoff_path": handoff_path,
        "handoff": handoff,
        "context": {
            "repo_root": context.repo_root,
            "manifest_path": context.manifest_path,
            "manifest_ref": context.manifest_ref,
            "manifest_sha256": context.manifest_sha256,
            "raw_manifest_base64": BASE64.encode(context.raw_manifest),
            "manifest": context.manifest,
        },
        "selected_source_rows": selected_source_rows,
        "payloads": payloads,
    }))
}

pub fn invoke(request: &Value) -> Result<Value> {
    match required_text(request, "operation")? {
        "validation-context" => verify_validation_context(
            value_at(request, "validation_context")?,
            required_text(request, "validator_sha256")?,
        ),
        "verify" => verify_for_intake(request),
        "adapt" => adapt_handoff(request),
        _ => Err("unsupported acquisition handoff operation".into()),
    }
}
