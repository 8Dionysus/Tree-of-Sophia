//! Selected abyss-stack OCR verification for one owner-local TextLayer.
//! The stronger owner verifies its own signed execution; this reader only
//! checks the independently selected ToS grant and copies exact evidence.
//! No path here invokes the owner's execute or render operations.

use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_creation_store::active;
use crate::source_text_owner::{normalized_absolute, read_absolute};
use base64::Engine;
use base64::alphabet::STANDARD as BASE64_ALPHABET;
use base64::engine::general_purpose::{GeneralPurpose, GeneralPurposeConfig};
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};
use tos_foundation::{Digest256, JsonValue};

const ORIGINAL_ADAPTER: &str =
    "mechanics/inference-pilots/parts/tos-foundation-lab/bounded_tesseract_ocr.py";
const PAGE_ADAPTER: &str =
    "mechanics/inference-pilots/parts/tos-foundation-lab/retained_page_tesseract_ocr.py";
const EVIDENCE: [(&str, &str, &str); 3] = [
    ("owner-ocr-receipt.json", "receipt.json", "receipt_sha256"),
    (
        "owner-ocr-signature.sigstore.json",
        "signature.sigstore.json",
        "signature_sha256",
    ),
    ("owner-ocr-signer.pub", "signer.pub", "public_key_sha256"),
];
const MAX_STDOUT: usize = 512 * 1024;
const MAX_STDERR: usize = 64 * 1024;

fn evidence_cap(name: &str) -> usize {
    match name {
        "owner-ocr-receipt.json" => 128 * 1024,
        "owner-ocr-signature.sigstore.json" => 64 * 1024,
        "owner-ocr-signer.pub" => 4096,
        _ => unreachable!("only the three fixed OCR evidence names are iterated"),
    }
}

pub(crate) struct OwnerOcrInitial {
    pub(crate) content: Vec<u8>,
    pub(crate) evidence: BTreeMap<String, Vec<u8>>,
}

fn invalid() -> SourceCommandError {
    SourceCommandError::Invalid("native owner OCR evidence")
}

fn serde(value: &JsonValue) -> SourceCommandResult<Value> {
    serde_json::from_slice(&cmd::canonical(value)?).map_err(|_| invalid())
}

fn sha(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

pub(crate) fn bounded_process(
    program: &str,
    args: &[&str],
    cwd: Option<&Path>,
    stdout_limit: usize,
    end: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<u8>> {
    bounded_process_with_stdin(program, args, cwd, None, stdout_limit, end, cancelled)
}

/// Run the existing bounded verifier with optional exact stdin bytes. The
/// caller already owns and has validated those bytes; a concurrent writer
/// keeps large inputs from blocking output drains or the shared deadline.
pub(crate) fn bounded_process_with_stdin(
    program: &str,
    args: &[&str],
    cwd: Option<&Path>,
    stdin: Option<&[u8]>,
    stdout_limit: usize,
    end: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<u8>> {
    active(end, cancelled)?;
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_clear()
        .env("PATH", "/usr/bin")
        .env("LC_ALL", "C.UTF-8")
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .process_group(0);
    if let Some(root) = cwd {
        command.current_dir(root);
    }
    let mut child = command
        .spawn()
        .map_err(|_| SourceCommandError::Denied("native owner verifier unavailable"))?;
    let group = rustix::process::Pid::from_child(&child);
    let out = child.stdout.take().ok_or(invalid())?;
    let err = child.stderr.take().ok_or(invalid())?;
    let overflow = Arc::new(AtomicBool::new(false));
    let (sender, receiver) = mpsc::channel();
    for (index, mut pipe, limit) in [
        (0usize, Box::new(out) as Box<dyn Read + Send>, stdout_limit),
        (1usize, Box::new(err) as Box<dyn Read + Send>, MAX_STDERR),
    ] {
        let sender = sender.clone();
        let overflow = Arc::clone(&overflow);
        thread::spawn(move || {
            let mut bytes = Vec::new();
            let mut block = [0u8; 8192];
            loop {
                match pipe.read(&mut block) {
                    Ok(0) => break,
                    Ok(count) => {
                        if bytes.len().checked_add(count).is_none_or(|n| n > limit) {
                            overflow.store(true, Ordering::Relaxed);
                        } else if !overflow.load(Ordering::Relaxed) {
                            bytes.extend_from_slice(&block[..count]);
                        }
                    }
                    Err(_) => {
                        overflow.store(true, Ordering::Relaxed);
                        break;
                    }
                }
            }
            let _ = sender.send((index, bytes));
        });
    }
    let stdin_failed = Arc::new(AtomicBool::new(false));
    if let Some(input) = stdin {
        let mut pipe = child.stdin.take().ok_or(invalid())?;
        let input = input.to_vec();
        let sender = sender.clone();
        let stdin_failed = Arc::clone(&stdin_failed);
        thread::spawn(move || {
            if pipe.write_all(&input).is_err() {
                stdin_failed.store(true, Ordering::Relaxed);
            }
            drop(pipe);
            let _ = sender.send((2usize, Vec::new()));
        });
    }
    drop(sender);
    let mut status = None;
    let mut streams = [
        None,
        None,
        if stdin.is_none() {
            Some(Vec::new())
        } else {
            None
        },
    ];
    let mut failed = None;
    while status.is_none() || streams.iter().any(Option::is_none) {
        if let Err(error) = active(end, cancelled) {
            failed = Some(error);
            break;
        }
        if overflow.load(Ordering::Relaxed) {
            failed = Some(SourceCommandError::Unsupported(
                "native owner verifier output budget",
            ));
            break;
        }
        if stdin_failed.load(Ordering::Relaxed) {
            failed = Some(SourceCommandError::Denied(
                "native owner verifier stdin was incomplete",
            ));
            break;
        }
        if status.is_none() {
            match child.try_wait() {
                Ok(observed) => status = observed,
                Err(_) => {
                    failed = Some(invalid());
                    break;
                }
            }
        }
        while let Ok((index, bytes)) = receiver.try_recv() {
            streams[index] = Some(bytes);
        }
        if status.is_none() || streams.iter().any(Option::is_none) {
            thread::sleep(Duration::from_millis(5));
        }
    }
    if failed.is_some() {
        let _ = rustix::process::kill_process_group(group, rustix::process::Signal::KILL);
        let _ = child.wait();
        return Err(failed.ok_or(invalid())?);
    }
    if !status.is_some_and(|status| status.success()) || overflow.load(Ordering::Relaxed) {
        return Err(SourceCommandError::Denied(
            "native owner OCR verification refused",
        ));
    }
    active(end, cancelled)?;
    streams[0].take().ok_or(invalid())
}

fn verified_adapter(
    material: &JsonValue,
    page: bool,
    uid: u32,
    end: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PathBuf> {
    let root = normalized_absolute(cmd::text(material, "owner_source_root")?)?;
    if root.canonicalize().map_err(|_| invalid())? != root {
        return Err(SourceCommandError::Denied(
            "native owner OCR source root changed",
        ));
    }
    let adapter = root.join(if page { PAGE_ADAPTER } else { ORIGINAL_ADAPTER });
    let raw = read_absolute(&adapter, uid, false, 128 * 1024, end, cancelled)?;
    if Digest256::of_bytes(&raw).to_hex() != cmd::text(material, "adapter_sha256")? {
        return Err(SourceCommandError::Conflict(
            "native owner OCR adapter changed",
        ));
    }
    let revision = bounded_process(
        "/usr/bin/git",
        &["rev-parse", "HEAD"],
        Some(&root),
        256,
        end,
        cancelled,
    )?;
    let selected = cmd::text(material, "owner_source_ref")?;
    if selected.strip_prefix("commit:") != Some(String::from_utf8_lossy(&revision).trim()) {
        return Err(SourceCommandError::Conflict(
            "native owner OCR source revision changed",
        ));
    }
    let status = bounded_process(
        "/usr/bin/git",
        &["status", "--porcelain", "--untracked-files=normal"],
        Some(&root),
        65_536,
        end,
        cancelled,
    )?;
    if status.iter().any(|byte| !byte.is_ascii_whitespace()) {
        return Err(SourceCommandError::Conflict(
            "native owner OCR source is dirty",
        ));
    }
    Ok(adapter)
}

fn evidence_from_root(
    material: &JsonValue,
    uid: u32,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<BTreeMap<String, Vec<u8>>> {
    let root = normalized_absolute(cmd::text(material, "receipt_root")?)?;
    let mut copied = BTreeMap::new();
    for (name, original, key) in EVIDENCE {
        let raw = read_absolute(
            &root.join(original),
            uid,
            true,
            evidence_cap(name),
            deadline,
            cancelled,
        )?;
        if Digest256::of_bytes(&raw).to_hex() != cmd::text(material, key)? {
            return Err(SourceCommandError::Conflict(
                "native owner OCR evidence changed",
            ));
        }
        copied.insert(name.to_owned(), raw);
    }
    Ok(copied)
}

fn checked_result(
    response: &[u8],
    material: &JsonValue,
    scope: &JsonValue,
    language: &str,
    page: bool,
    copied: &BTreeMap<String, Vec<u8>>,
    disclose: bool,
) -> SourceCommandResult<Option<Vec<u8>>> {
    if response.len() > MAX_STDOUT {
        return Err(SourceCommandError::Unsupported(
            "native owner OCR result budget",
        ));
    }
    let result: Value = serde_json::from_slice(response).map_err(|_| invalid())?;
    let receipt = result.get("receipt").ok_or(invalid())?;
    let receipt_raw = copied.get("owner-ocr-receipt.json").ok_or(invalid())?;
    let selected_receipt: Value = serde_json::from_slice(receipt_raw).map_err(|_| invalid())?;
    let selected_scope = serde(scope)?;
    let language = match language {
        "de" => "deu",
        "ru" => "rus",
        other => other,
    };
    if result["ok"] != true
        || receipt != &selected_receipt
        || result["receipt_sha256"] != cmd::text(material, "receipt_sha256")?
        || result["signature_sha256"] != cmd::text(material, "signature_sha256")?
        || result["public_key_sha256"] != cmd::text(material, "public_key_sha256")?
        || receipt["owner"]["source_ref"] != cmd::text(material, "owner_source_ref")?
        || receipt["owner"]["adapter_sha256"] != cmd::text(material, "adapter_sha256")?
        || receipt["source_scope"] != selected_scope
        || receipt["language"] != language
        || receipt["output_sha256"] != cmd::text(material, "content_sha256")?
        || receipt["output_bytes"] != cmd::integer(material, "byte_size")?
    {
        return Err(SourceCommandError::Conflict(
            "native owner OCR receipt differs from grant",
        ));
    }
    if page {
        if receipt["schema_version"] != "tos_retained_pdf_page_ocr_execution_v1"
            || receipt["input_representation"]
                != serde(cmd::field(material, "input_representation")?)?
            || receipt["input_verification"]["render_execution"] != "not_performed"
            || receipt["input_verification"]["historical_receipt_signature"] != "absent"
        {
            return Err(SourceCommandError::Conflict(
                "native owner page OCR evidence differs",
            ));
        }
    }
    if !disclose {
        if result.get("content_base64").is_some() {
            return Err(SourceCommandError::Denied(
                "native owner metadata verification disclosed text",
            ));
        }
        return Ok(None);
    }
    let encoded = result["content_base64"].as_str().ok_or(invalid())?;
    if encoded.len() > 174_764 {
        return Err(SourceCommandError::Unsupported(
            "native owner OCR encoded content budget",
        ));
    }
    let python_base64 = GeneralPurpose::new(
        &BASE64_ALPHABET,
        GeneralPurposeConfig::new().with_decode_allow_trailing_bits(true),
    );
    let content = python_base64.decode(encoded).map_err(|_| invalid())?;
    if content.len() as u64 != cmd::integer(material, "byte_size")?
        || Digest256::of_bytes(&content).to_hex() != cmd::text(material, "content_sha256")?
        || std::str::from_utf8(&content).is_err()
    {
        return Err(SourceCommandError::Conflict(
            "native owner OCR content differs",
        ));
    }
    Ok(Some(content))
}

pub(crate) fn verify_initial(
    material: &JsonValue,
    scope: &JsonValue,
    language: &str,
    page: bool,
    uid: u32,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<OwnerOcrInitial> {
    // The owning command gives every owner check in this invocation the same
    // absolute 30-second ceiling. A later copied-record check cannot renew it.
    let end = deadline;
    let adapter = verified_adapter(material, page, uid, end, cancelled)?;
    let adapter_path = adapter.to_str().ok_or(invalid())?;
    let receipt_root = cmd::text(material, "receipt_root")?;
    let receipt_sha = cmd::text(material, "receipt_sha256")?;
    let public_sha = cmd::text(material, "public_key_sha256")?;
    let source_ref = cmd::text(material, "owner_source_ref")?;
    let response = bounded_process(
        "/usr/bin/python3",
        &[
            adapter_path,
            "--emit-content",
            "verify",
            "--receipt-root",
            receipt_root,
            "--receipt-sha256",
            receipt_sha,
            "--public-key-sha256",
            public_sha,
            "--owner-source-ref",
            source_ref,
        ],
        None,
        MAX_STDOUT,
        end,
        cancelled,
    )?;
    let evidence = evidence_from_root(material, uid, end, cancelled)?;
    let content = checked_result(&response, material, scope, language, page, &evidence, true)?
        .ok_or(invalid())?;
    verified_adapter(material, page, uid, end, cancelled)?;
    Ok(OwnerOcrInitial { content, evidence })
}

pub(crate) fn verify_record(
    material: &JsonValue,
    scope: &JsonValue,
    language: &str,
    page: bool,
    evidence_dir: &Path,
    copied: &BTreeMap<String, Vec<u8>>,
    uid: u32,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let end = deadline;
    let adapter = verified_adapter(material, page, uid, end, cancelled)?;
    let adapter_path = adapter.to_str().ok_or(invalid())?;
    let mut paths = Vec::new();
    for (name, _, key) in EVIDENCE {
        let path = evidence_dir.join(name);
        let raw = read_absolute(&path, uid, true, evidence_cap(name), end, cancelled)?;
        if copied.get(name) != Some(&raw)
            || Digest256::of_bytes(&raw).to_hex() != cmd::text(material, key)?
        {
            return Err(SourceCommandError::Conflict(
                "native owner OCR copied evidence changed",
            ));
        }
        paths.push(path);
    }
    let args = [
        adapter_path,
        "verify-record",
        "--receipt",
        paths[0].to_str().ok_or(invalid())?,
        "--signature",
        paths[1].to_str().ok_or(invalid())?,
        "--public-key",
        paths[2].to_str().ok_or(invalid())?,
        "--receipt-sha256",
        cmd::text(material, "receipt_sha256")?,
        "--public-key-sha256",
        cmd::text(material, "public_key_sha256")?,
        "--owner-source-ref",
        cmd::text(material, "owner_source_ref")?,
    ];
    let response = bounded_process("/usr/bin/python3", &args, None, MAX_STDOUT, end, cancelled)?;
    checked_result(&response, material, scope, language, page, copied, false)?;
    verified_adapter(material, page, uid, end, cancelled)?;
    Ok(())
}

pub(crate) fn validate_material(material: &JsonValue, page: bool) -> SourceCommandResult<()> {
    let mut fields = vec![
        "authority_ref",
        "expires_at",
        "access_allowed",
        "receipt_root",
        "receipt_sha256",
        "signature_sha256",
        "public_key_sha256",
        "owner_source_root",
        "owner_source_ref",
        "adapter_sha256",
        "content_sha256",
        "byte_size",
    ];
    if page {
        fields.push("input_representation");
    }
    cmd::exact_keys(material, &fields)?;
    if cmd::field(material, "access_allowed")? != &JsonValue::Bool(true)
        || cmd::text(material, "authority_ref")?.trim().is_empty()
        || !(1..=131_072).contains(&cmd::integer(material, "byte_size")?)
        || !cmd::text(material, "owner_source_ref")?
            .strip_prefix("commit:")
            .is_some_and(|sha| {
                sha.len() == 40
                    && sha
                        .bytes()
                        .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            })
        || [
            "receipt_sha256",
            "signature_sha256",
            "public_key_sha256",
            "adapter_sha256",
            "content_sha256",
        ]
        .iter()
        .any(|name| cmd::text(material, name).map_or(true, |value| !sha(value)))
    {
        return Err(SourceCommandError::Denied("native owner OCR grant invalid"));
    }
    normalized_absolute(cmd::text(material, "owner_source_root")?)?;
    normalized_absolute(cmd::text(material, "receipt_root")?)?;
    if page {
        validate_page_binding(cmd::field(material, "input_representation")?)?;
    }
    Ok(())
}

pub(crate) fn validate_page_binding(binding: &JsonValue) -> SourceCommandResult<()> {
    cmd::exact_keys(
        binding,
        &[
            "schema_version",
            "source_file_ref",
            "source_file_sha256",
            "page_number",
            "page_index_origin",
            "render_id",
            "sample_id",
            "render_manifest_sha256",
            "render_receipt_sha256",
            "sample_plan_sha256",
            "input_file_ref",
            "input_sha256",
            "input_bytes",
            "media_type",
            "width_pixels",
            "height_pixels",
            "renderer",
            "renderer_version",
            "resolution_dpi",
            "render_execution",
            "historical_receipt_signature",
        ],
    )?;
    let source = cmd::text(binding, "source_file_sha256")?;
    let input = cmd::text(binding, "input_sha256")?;
    if cmd::text(binding, "schema_version")? != "tos_retained_pdf_page_input_binding_v1"
        || !sha(source)
        || !sha(input)
        || cmd::text(binding, "source_file_ref")? != format!("tos.file.sha256.{source}")
        || cmd::text(binding, "input_file_ref")? != format!("tos.file.sha256.{input}")
        || !(1..=10_000).contains(&cmd::integer(binding, "page_number")?)
        || cmd::integer(binding, "page_index_origin")? != 1
        || cmd::text(binding, "media_type")? != "image/png"
        || cmd::text(binding, "renderer")? != "poppler-pdftoppm"
        || !["26.01.0", "26.05.0"].contains(&cmd::text(binding, "renderer_version")?)
        || cmd::integer(binding, "resolution_dpi")? != 300
        || cmd::text(binding, "render_execution")? != "retained-not-observed-this-run"
        || cmd::text(binding, "historical_receipt_signature")? != "absent"
        || !(1..=10 * 1024 * 1024).contains(&cmd::integer(binding, "input_bytes")?)
        || cmd::integer(binding, "width_pixels")? == 0
        || cmd::integer(binding, "height_pixels")? == 0
        || cmd::integer(binding, "width_pixels")?
            .checked_mul(cmd::integer(binding, "height_pixels")?)
            .is_none_or(|pixels| pixels > 12_000_000)
        || [
            "render_manifest_sha256",
            "render_receipt_sha256",
            "sample_plan_sha256",
        ]
        .iter()
        .any(|name| cmd::text(binding, name).map_or(true, |value| !sha(value)))
    {
        return Err(SourceCommandError::Invalid("native owner page OCR binding"));
    }
    Ok(())
}

pub(crate) fn validate_page_anchor(
    anchor: &JsonValue,
    binding: &JsonValue,
    scope: &JsonValue,
) -> SourceCommandResult<()> {
    validate_page_binding(binding)?;
    if cmd::field(binding, "source_file_ref")? != cmd::field(scope, "file_ref")?
        || cmd::field(binding, "source_file_sha256")? != cmd::field(scope, "file_sha256")?
    {
        return Err(SourceCommandError::Conflict(
            "native owner page OCR original File differs",
        ));
    }
    let target = cmd::field(anchor, "target")?;
    let payload = cmd::field(anchor, "selector_payload")?;
    let expression = cmd::field(payload, "expression")?;
    let envelope = cmd::field(expression, "selector")?;
    let selector = cmd::field(envelope, "selector")?;
    let state = cmd::field(envelope, "state")?;
    let page = cmd::object(vec![(
        "page_number",
        cmd::field(binding, "page_number")?.clone(),
    )]);
    if cmd::field(target, "file_id")? != cmd::field(scope, "file_ref")?
        || cmd::field(target, "file_sha256")? != cmd::field(scope, "file_sha256")?
        || cmd::field(target, "item_id")? != cmd::field(scope, "item_ref")?
        || cmd::text(target, "media_type")? != "application/pdf"
        || cmd::text(payload, "kind")? != "selector_expression"
        || cmd::text(expression, "mode")? != "single"
        || cmd::text(state, "state_type")? != "digest_state"
        || cmd::field(state, "representation_sha256")? != cmd::field(scope, "file_sha256")?
        || cmd::text(state, "media_type")? != "application/pdf"
        || cmd::text(selector, "type")? != "page_region"
        || cmd::field(selector, "page_identity")? != &page
        || cmd::text(selector, "coordinate_space")? != "normalized_0_1"
        || cmd::integer(selector, "x")? != 0
        || cmd::integer(selector, "y")? != 0
        || cmd::integer(selector, "width")? != 1
        || cmd::integer(selector, "height")? != 1
    {
        return Err(SourceCommandError::Conflict(
            "native owner page OCR anchor differs",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn source_value(value: &Value) -> JsonValue {
        cmd::parse(&serde_json::to_vec(value).expect("encode synthetic OCR value"))
            .expect("parse synthetic OCR value")
    }

    fn page_binding() -> Value {
        let source_sha = "1".repeat(64);
        let input_sha = "5".repeat(64);
        json!({
            "schema_version": "tos_retained_pdf_page_input_binding_v1",
            "source_file_ref": format!("tos.file.sha256.{source_sha}"),
            "source_file_sha256": source_sha,
            "page_number": 44,
            "page_index_origin": 1,
            "render_id": "synthetic-render",
            "sample_id": "synthetic-page-44",
            "render_manifest_sha256": "2".repeat(64),
            "render_receipt_sha256": "3".repeat(64),
            "sample_plan_sha256": "4".repeat(64),
            "input_file_ref": format!("tos.file.sha256.{input_sha}"),
            "input_sha256": input_sha,
            "input_bytes": 100,
            "media_type": "image/png",
            "width_pixels": 10,
            "height_pixels": 20,
            "renderer": "poppler-pdftoppm",
            "renderer_version": "26.01.0",
            "resolution_dpi": 300,
            "render_execution": "retained-not-observed-this-run",
            "historical_receipt_signature": "absent"
        })
    }

    fn fixture(
        page: bool,
    ) -> (
        JsonValue,
        JsonValue,
        BTreeMap<String, Vec<u8>>,
        Value,
        Vec<u8>,
    ) {
        let content = b"Exact OCR: Gr\xc3\xbc\n".to_vec();
        let scope_value = json!({
            "work_ref": "tos.work.synthetic-ocr",
            "expression_ref": "tos.expression.synthetic-ocr",
            "edition_ref": "tos.edition.synthetic-ocr",
            "item_ref": "tos.item.synthetic-ocr",
            "file_ref": format!("tos.file.sha256.{}", "1".repeat(64)),
            "file_sha256": "1".repeat(64)
        });
        let scope = source_value(&scope_value);
        let content_sha = Digest256::of_bytes(&content).to_hex();
        let owner_source_ref = format!("commit:{}", "a".repeat(40));
        let adapter_sha = "b".repeat(64);
        let signature_sha = "c".repeat(64);
        let public_key_sha = "d".repeat(64);
        let input = page_binding();
        let receipt = if page {
            json!({
                "schema_version": "tos_retained_pdf_page_ocr_execution_v1",
                "owner": {"source_ref": owner_source_ref, "adapter_sha256": adapter_sha},
                "source_scope": scope_value,
                "language": "deu",
                "output_sha256": content_sha,
                "output_bytes": content.len(),
                "input_representation": input,
                "input_verification": {
                    "render_execution": "not_performed",
                    "historical_receipt_signature": "absent"
                }
            })
        } else {
            json!({
                "schema_version": "tos_operator_synthetic_png_ocr_execution_v1",
                "owner": {"source_ref": owner_source_ref, "adapter_sha256": adapter_sha},
                "source_scope": scope_value,
                "language": "deu",
                "output_sha256": content_sha,
                "output_bytes": content.len()
            })
        };
        let receipt_raw = serde_json::to_vec(&receipt).expect("encode synthetic receipt");
        let receipt_sha = Digest256::of_bytes(&receipt_raw).to_hex();
        let mut material_value = json!({
            "receipt_sha256": receipt_sha,
            "signature_sha256": signature_sha,
            "public_key_sha256": public_key_sha,
            "owner_source_ref": owner_source_ref,
            "adapter_sha256": adapter_sha,
            "content_sha256": content_sha,
            "byte_size": content.len()
        });
        if page {
            material_value["input_representation"] = input.clone();
        }
        let material = source_value(&material_value);
        let mut response = json!({
            "ok": true,
            "receipt": receipt,
            "receipt_sha256": receipt_sha,
            "signature_sha256": signature_sha,
            "public_key_sha256": public_key_sha,
            "content_base64": base64::engine::general_purpose::STANDARD.encode(&content)
        });
        let copied = BTreeMap::from([("owner-ocr-receipt.json".to_owned(), receipt_raw)]);
        (material, scope, copied, response, content)
    }

    #[test]
    fn synthetic_result_keeps_exact_utf8_content_and_receipt_binding() {
        let (material, scope, copied, mut response, content) = fixture(false);
        let raw = serde_json::to_vec(&response).expect("encode synthetic verifier result");
        assert_eq!(
            checked_result(&raw, &material, &scope, "de", false, &copied, true).unwrap(),
            Some(content)
        );
        response["receipt"]["source_scope"]["item_ref"] = json!("tos.item.other");
        let raw = serde_json::to_vec(&response).expect("encode unbound synthetic result");
        assert!(checked_result(&raw, &material, &scope, "de", false, &copied, true).is_err());
    }

    #[test]
    fn metadata_replay_accepts_receipt_only_and_refuses_disclosed_text() {
        let (material, scope, copied, mut response, _) = fixture(false);
        response.as_object_mut().unwrap().remove("content_base64");
        let raw = serde_json::to_vec(&response).expect("encode metadata result");
        assert_eq!(
            checked_result(&raw, &material, &scope, "de", false, &copied, false).unwrap(),
            None
        );
        response["content_base64"] = json!("ZXhhY3Q=");
        let raw = serde_json::to_vec(&response).expect("encode disclosing metadata result");
        assert!(checked_result(&raw, &material, &scope, "de", false, &copied, false).is_err());
    }

    #[test]
    fn retained_page_result_requires_current_page_and_capture_posture() {
        let (material, scope, copied, mut response, content) = fixture(true);
        let binding = cmd::field(&material, "input_representation").unwrap();
        validate_page_binding(binding).unwrap();
        let raw = serde_json::to_vec(&response).expect("encode retained page result");
        assert_eq!(
            checked_result(&raw, &material, &scope, "de", true, &copied, true).unwrap(),
            Some(content)
        );
        response["receipt"]["input_verification"]["render_execution"] = json!("performed");
        let raw = serde_json::to_vec(&response).expect("encode false render posture");
        assert!(checked_result(&raw, &material, &scope, "de", true, &copied, true).is_err());
    }

    #[test]
    fn retained_page_binding_rejects_renderer_and_source_identity_drift() {
        let mut binding = page_binding();
        validate_page_binding(&source_value(&binding)).unwrap();
        binding["renderer"] = json!("unselected-renderer");
        assert!(validate_page_binding(&source_value(&binding)).is_err());
        binding["renderer"] = json!("poppler-pdftoppm");
        binding["input_file_ref"] = json!(format!("tos.file.sha256.{}", "6".repeat(64)));
        assert!(validate_page_binding(&source_value(&binding)).is_err());
    }
}
