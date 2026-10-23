//! Portable signed OCR import for one ToS-owned TextLayer. ToS verifies the
//! independently pinned receipt, signer, source binding and exact output bytes.
//! Producer implementation and runtime admission remain with the producer.

use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_creation_store::active;
use crate::source_text_owner::{normalized_absolute, read_absolute};
use base64::Engine;
use p256::ecdsa::{Signature, VerifyingKey, signature::Verifier};
use p256::pkcs8::DecodePublicKey;
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};
use tos_foundation::{CanonicalProfile, Digest256, JsonLimits, JsonValue, canonical_raw_bytes_v1};

const EVIDENCE: [(&str, &str, &str); 3] = [
    ("owner-ocr-receipt.json", "receipt.json", "receipt_sha256"),
    (
        "owner-ocr-signature.sigstore.json",
        "signature.sigstore.json",
        "signature_sha256",
    ),
    ("owner-ocr-signer.pub", "signer.pub", "public_key_sha256"),
];
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
    bounded_process_with_stdin_guarded(
        program,
        args,
        cwd,
        stdin,
        stdout_limit,
        end,
        cancelled,
        &mut || Ok(()),
    )
}

/// The same bounded process protocol with the caller's current owner fence.
/// A failed fence terminates and reaps the owned child before returning it.
pub(crate) fn bounded_process_with_stdin_guarded(
    program: &str,
    args: &[&str],
    cwd: Option<&Path>,
    stdin: Option<&[u8]>,
    stdout_limit: usize,
    end: Instant,
    cancelled: &AtomicBool,
    check: &mut dyn FnMut() -> SourceCommandResult<()>,
) -> SourceCommandResult<Vec<u8>> {
    active(end, cancelled)?;
    check()?;
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
        if let Err(error) = active(end, cancelled).and_then(|()| check()) {
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
    check()?;
    streams[0].take().ok_or(invalid())
}

fn strict_evidence_json(raw: &[u8], max_bytes: usize) -> SourceCommandResult<Value> {
    canonical_raw_bytes_v1(
        raw,
        CanonicalProfile::CorpusSnapshotV1,
        JsonLimits {
            max_bytes,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| invalid())?;
    serde_json::from_slice(raw).map_err(|_| invalid())
}

fn verified_receipt(
    copied: &BTreeMap<String, Vec<u8>>,
    end: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Value> {
    active(end, cancelled)?;
    let raw = copied.get("owner-ocr-receipt.json").ok_or(invalid())?;
    let public = copied.get("owner-ocr-signer.pub").ok_or(invalid())?;
    let bundle = strict_evidence_json(
        copied
            .get("owner-ocr-signature.sigstore.json")
            .ok_or(invalid())?,
        64 * 1024,
    )?;
    if !matches!(
        bundle["mediaType"].as_str(),
        Some("application/vnd.dev.sigstore.bundle.v0.3+json")
            | Some("application/vnd.dev.sigstore.bundle.v0.2+json")
            | Some("application/vnd.dev.sigstore.bundle.v0.1+json")
    ) || bundle["messageSignature"]["messageDigest"]["algorithm"] != "SHA2_256"
    {
        return Err(SourceCommandError::Unsupported(
            "OCR requires a key-pinned P-256/SHA-256 Sigstore message signature",
        ));
    }
    let encoded_digest = bundle["messageSignature"]["messageDigest"]["digest"]
        .as_str()
        .ok_or(invalid())?;
    let expected_digest = base64::engine::general_purpose::STANDARD
        .decode(encoded_digest)
        .map_err(|_| invalid())?;
    if expected_digest.as_slice() != Digest256::of_bytes(raw).as_bytes() {
        return Err(SourceCommandError::Conflict(
            "OCR signature bundle message digest differs",
        ));
    }
    let encoded_signature = bundle["messageSignature"]["signature"]
        .as_str()
        .ok_or(invalid())?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded_signature)
        .map_err(|_| invalid())?;
    let signature = Signature::from_der(&bytes).map_err(|_| invalid())?;
    let public = std::str::from_utf8(public).map_err(|_| invalid())?;
    let key = VerifyingKey::from_public_key_pem(public).map_err(|_| invalid())?;
    key.verify(raw, &signature)
        .map_err(|_| SourceCommandError::Denied("OCR receipt signature does not verify"))?;
    active(end, cancelled)?;
    let receipt = strict_evidence_json(raw, 128 * 1024)?;
    Ok(receipt)
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

fn bound_receipt(
    receipt: &Value,
    material: &JsonValue,
    scope: &JsonValue,
    language: &str,
    page: bool,
) -> SourceCommandResult<()> {
    let selected_scope = serde(scope)?;
    let language = match language {
        "de" => "deu",
        "ru" => "rus",
        other => other,
    };
    if receipt["schema_version"]
        != if page {
            "tos_retained_pdf_page_ocr_execution_v1"
        } else {
            "tos_operator_synthetic_png_ocr_execution_v1"
        }
        || receipt["status"] != "completed"
        || receipt["returncode"] != 0
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
    // Preserve the source/input and finite execution assertions of these two
    // versioned receipt profiles, without importing a producer's host policy.
    let limits = &receipt["limits"];
    let output_limit = limits["max_output_bytes"]
        .as_u64()
        .filter(|v| (1..=131_072).contains(v));
    let pixel_limit = limits["max_pixels"]
        .as_u64()
        .filter(|v| (1..=12_000_000).contains(v));
    let seconds = limits["max_seconds"]
        .as_f64()
        .filter(|v| v.is_finite() && *v > 0.0 && *v <= 30.0);
    let elapsed = receipt["elapsed_seconds"]
        .as_f64()
        .filter(|v| v.is_finite() && *v >= 0.0);
    let source_cap = if page {
        128 * 1024 * 1024
    } else {
        10 * 1024 * 1024
    };
    if receipt["source_sha256"] != selected_scope["file_sha256"]
        || !receipt["source_bytes"]
            .as_u64()
            .is_some_and(|v| (1..=source_cap).contains(&v))
        || !output_limit.is_some_and(|max| {
            receipt["output_bytes"]
                .as_u64()
                .is_some_and(|n| n > 0 && n <= max)
        })
        || !seconds.zip(elapsed).is_some_and(|(max, n)| n <= max)
        || pixel_limit.is_none()
        || limits.as_object().is_none_or(|v| v.len() != 3)
    {
        return Err(SourceCommandError::Conflict(
            "OCR source or execution bounds differ",
        ));
    }
    let pixels = if page {
        let binding = cmd::field(material, "input_representation")?;
        validate_page_binding(binding)?;
        cmd::integer(binding, "width_pixels")?.checked_mul(cmd::integer(binding, "height_pixels")?)
    } else {
        receipt["image_dimensions"]
            .as_array()
            .filter(|v| v.len() == 2)
            .and_then(|v| v[0].as_u64().zip(v[1].as_u64()))
            .and_then(|(w, h)| {
                if w > 0 && h > 0 {
                    w.checked_mul(h)
                } else {
                    None
                }
            })
    };
    if !pixels
        .zip(pixel_limit)
        .is_some_and(|(n, max)| n > 0 && n <= max)
    {
        return Err(SourceCommandError::Conflict(
            "OCR input image bounds differ",
        ));
    }
    if page {
        if receipt["schema_version"] != "tos_retained_pdf_page_ocr_execution_v1"
            || receipt["input_representation"]
                != serde(cmd::field(material, "input_representation")?)?
            || receipt["input_verification"]["schema_version"]
                != "tos_retained_pdf_page_verification_capture_v1"
            || receipt["input_verification"]["event_type"] != "verification_capture"
            || receipt["input_verification"]["status"] != "verified"
            || receipt["input_verification"]["source_scope"] != selected_scope
            || receipt["input_verification"]["source_bytes"] != receipt["source_bytes"]
            || receipt["input_verification"]["input_representation"]
                != receipt["input_representation"]
            || receipt["input_verification"]["render_execution"] != "not_performed"
            || receipt["input_verification"]["historical_receipt_signature"] != "absent"
        {
            return Err(SourceCommandError::Conflict(
                "native owner page OCR evidence differs",
            ));
        }
    }
    Ok(())
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
    let root = normalized_absolute(cmd::text(material, "receipt_root")?)?;
    let evidence = evidence_from_root(material, uid, end, cancelled)?;
    let receipt = verified_receipt(&evidence, end, cancelled)?;
    bound_receipt(&receipt, material, scope, language, page)?;
    let content = read_absolute(
        &root.join("content.txt"),
        uid,
        true,
        131_072,
        end,
        cancelled,
    )?;
    if content.len() as u64 != cmd::integer(material, "byte_size")?
        || Digest256::of_bytes(&content).to_hex() != cmd::text(material, "content_sha256")?
        || std::str::from_utf8(&content).is_err()
    {
        return Err(SourceCommandError::Conflict(
            "native owner OCR content differs",
        ));
    }
    if evidence_from_root(material, uid, end, cancelled)? != evidence {
        return Err(SourceCommandError::Conflict(
            "OCR evidence changed during signature verification",
        ));
    }
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
    }
    let receipt = verified_receipt(copied, end, cancelled)?;
    bound_receipt(&receipt, material, scope, language, page)?;
    for (name, _, key) in EVIDENCE {
        let raw = read_absolute(
            &evidence_dir.join(name),
            uid,
            true,
            evidence_cap(name),
            end,
            cancelled,
        )?;
        if copied.get(name) != Some(&raw)
            || Digest256::of_bytes(&raw).to_hex() != cmd::text(material, key)?
        {
            return Err(SourceCommandError::Conflict(
                "OCR evidence changed during signature verification",
            ));
        }
    }
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
        "owner_source_ref",
        "adapter_sha256",
        "content_sha256",
        "byte_size",
    ];
    if page {
        fields.push("input_representation");
    }
    // Immutable earlier grants may retain a historical source-root label. It
    // is never resolved, inspected or used to choose executable code.
    if material.object_get("owner_source_root").is_some() {
        fields.push("owner_source_root");
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
        cmd::parse(&serde_json::to_vec(value).unwrap()).unwrap()
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

    fn fixture(page: bool) -> (JsonValue, JsonValue, Value, Vec<u8>) {
        let content = b"Exact OCR: Gr\xc3\xbc\n".to_vec();
        let mut receipt: Value = serde_json::from_slice(include_bytes!(
            "../tests/fixtures/portable-ocr/receipt.json"
        ))
        .unwrap();
        let scope = source_value(&receipt["source_scope"]);
        let mut material = json!({"receipt_sha256":"1".repeat(64),"signature_sha256":"2".repeat(64),
            "public_key_sha256":"3".repeat(64),"owner_source_ref":receipt["owner"]["source_ref"],
            "adapter_sha256":receipt["owner"]["adapter_sha256"],
            "content_sha256":Digest256::of_bytes(&content).to_hex(),"byte_size":content.len()});
        if page {
            let binding = page_binding();
            receipt["schema_version"] = json!("tos_retained_pdf_page_ocr_execution_v1");
            receipt["input_representation"] = binding.clone();
            receipt["input_verification"] = json!({"schema_version":"tos_retained_pdf_page_verification_capture_v1","event_type":"verification_capture","status":"verified","source_scope":receipt["source_scope"],"source_bytes":receipt["source_bytes"],"input_representation":binding,"render_execution":"not_performed","historical_receipt_signature":"absent"});
            material["input_representation"] = binding;
        }
        (source_value(&material), scope, receipt, content)
    }

    #[test]
    fn signed_receipt_binding_refuses_source_language_and_output_drift() {
        let (material, scope, receipt, _) = fixture(false);
        bound_receipt(&receipt, &material, &scope, "de", false).unwrap();
        for field in [
            "source_scope",
            "language",
            "output_sha256",
            "source_sha256",
            "source_bytes",
            "limits",
        ] {
            let mut changed = receipt.clone();
            changed[field] = json!("different");
            assert!(bound_receipt(&changed, &material, &scope, "de", false).is_err());
        }
    }

    #[test]
    fn retained_page_receipt_requires_current_input_and_capture_posture() {
        let (material, scope, mut receipt, _) = fixture(true);
        validate_page_binding(cmd::field(&material, "input_representation").unwrap()).unwrap();
        bound_receipt(&receipt, &material, &scope, "de", true).unwrap();
        receipt["input_verification"]["render_execution"] = json!("performed");
        assert!(bound_receipt(&receipt, &material, &scope, "de", true).is_err());
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

    #[test]
    fn ambiguous_signed_json_is_refused() {
        assert!(strict_evidence_json(br#"{"x":1,"x":2}"#, 1024).is_err());
        assert!(strict_evidence_json(br#"{"x":1,"\u0078":2}"#, 1024).is_err());
        assert!(strict_evidence_json(br#"{"x":1}"#, 1024).is_ok());
    }

    #[test]
    fn portable_signed_import_and_metadata_replay_need_no_producer_checkout() {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        let selected = std::env::temp_dir().join(format!(
            "tos-portable-ocr-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&selected)
            .unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(selected.clone());
        let evidence = [
            (
                "receipt.json",
                include_bytes!("../tests/fixtures/portable-ocr/receipt.json").as_slice(),
            ),
            (
                "signature.sigstore.json",
                include_bytes!("../tests/fixtures/portable-ocr/signature.sigstore.json").as_slice(),
            ),
            (
                "signer.pub",
                include_bytes!("../tests/fixtures/portable-ocr/signer.pub").as_slice(),
            ),
        ];
        for (name, raw) in evidence {
            let path = selected.join(name);
            std::fs::write(&path, raw).unwrap();
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        let (material, scope, _, content) = fixture(false);
        let mut material = serde(&material).unwrap();
        material["authority_ref"] = json!("synthetic-independent-grant");
        material["access_allowed"] = json!(true);
        material["expires_at"] = json!("2099-01-01T00:00:00Z");
        material["receipt_root"] = json!(selected);
        // A historical root label is accepted as inert provenance; it is absent.
        material["owner_source_root"] = json!("/not/available/producer-checkout");
        for (index, key) in ["receipt_sha256", "signature_sha256", "public_key_sha256"]
            .iter()
            .enumerate()
        {
            material[*key] = json!(Digest256::of_bytes(evidence[index].1).to_hex());
        }
        std::fs::write(selected.join("content.txt"), &content).unwrap();
        std::fs::set_permissions(
            selected.join("content.txt"),
            std::fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        let material = source_value(&material);
        validate_material(&material, false).unwrap();
        let deadline = Instant::now() + Duration::from_secs(60);
        let cancelled = AtomicBool::new(false);
        let uid = unsafe { libc::geteuid() };
        let initial =
            verify_initial(&material, &scope, "de", false, uid, deadline, &cancelled).unwrap();
        assert_eq!(initial.content, content);
        let recorded = selected.join("record");
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&recorded)
            .unwrap();
        for (name, raw) in &initial.evidence {
            let path = recorded.join(name);
            std::fs::write(&path, raw).unwrap();
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        std::fs::remove_file(selected.join("content.txt")).unwrap();
        verify_record(
            &material,
            &scope,
            "de",
            false,
            &recorded,
            &initial.evidence,
            uid,
            deadline,
            &cancelled,
        )
        .unwrap();
        std::fs::write(selected.join("content.txt"), &content).unwrap();
        std::fs::set_permissions(
            selected.join("content.txt"),
            std::fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        // A valid DER encoding with one changed signature bit, with all material
        // hashes updated, must fail cryptographic verification itself.
        let mut forged: Value = serde_json::from_slice(evidence[1].1).unwrap();
        let mut signature = base64::engine::general_purpose::STANDARD
            .decode(forged["messageSignature"]["signature"].as_str().unwrap())
            .unwrap();
        *signature.last_mut().unwrap() ^= 1;
        Signature::from_der(&signature).unwrap();
        forged["messageSignature"]["signature"] =
            json!(base64::engine::general_purpose::STANDARD.encode(&signature));
        let raw = serde_json::to_vec(&forged).unwrap();
        std::fs::write(selected.join("signature.sigstore.json"), &raw).unwrap();
        let mut tampered = serde(&material).unwrap();
        tampered["signature_sha256"] = json!(Digest256::of_bytes(&raw).to_hex());
        assert!(matches!(
            verify_initial(
                &source_value(&tampered),
                &scope,
                "de",
                false,
                uid,
                deadline,
                &cancelled
            ),
            Err(SourceCommandError::Denied(
                "OCR receipt signature does not verify"
            ))
        ));
        std::fs::write(selected.join("signature.sigstore.json"), evidence[1].1).unwrap();
        // The receipt's independently selected bytes must also match the bundle.
        let mut changed: Value = serde_json::from_slice(evidence[0].1).unwrap();
        changed["language"] = json!("rus");
        let raw = serde_json::to_vec(&changed).unwrap();
        std::fs::write(selected.join("receipt.json"), &raw).unwrap();
        let mut tampered = serde(&material).unwrap();
        tampered["receipt_sha256"] = json!(Digest256::of_bytes(&raw).to_hex());
        assert!(
            verify_initial(
                &source_value(&tampered),
                &scope,
                "de",
                false,
                uid,
                deadline,
                &cancelled
            )
            .is_err()
        );
    }
}
