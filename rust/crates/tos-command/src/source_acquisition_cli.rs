//! One bounded acquisition compatibility wire; operation meaning stays with
//! the source-witnessing owner. No request can select executable code.
use crate::source_acquisition_batch as batch;
use serde_json::{Value, json};
use std::io::{BufRead, Read, Write};
use tos_foundation::{CanonicalProfile, JsonLimits, JsonMode, canonical_bytes_v1, parse_json};

const REQUEST_BYTES: usize = 16 * 1024 * 1024;
const RESPONSE_BYTES: usize = 64 * 1024 * 1024;

pub fn run() -> i32 {
    let result = (|| {
        let mut line = String::new();
        std::io::stdin()
            .lock()
            .take(REQUEST_BYTES as u64 + 1)
            .read_line(&mut line)
            .map_err(|e| e.to_string())?;
        if line.len() > REQUEST_BYTES || !line.ends_with('\n') {
            return Err("acquisition request exceeds wire budget".into());
        }
        // Wire bytes may contain Base64 and exceed the source-document budget.
        // Retain the existing strict JSON owner with this transport byte cap.
        let limits = JsonLimits {
            max_bytes: REQUEST_BYTES,
            ..JsonLimits::default()
        };
        let document = parse_json(line.as_bytes(), JsonMode::PublishedStrict, limits)
            .map_err(|e| format!("strict acquisition request: {e:?}"))?;
        let raw = canonical_bytes_v1(
            document.root(),
            CanonicalProfile::SourceCommandInputV1,
            limits,
        )
        .map_err(|e| format!("acquisition request encoding: {e:?}"))?;
        let request: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
        let value = match batch::text(&request, "family")? {
            "batch" => batch::invoke(&request),
            "handoff" => crate::source_acquisition_handoff::invoke(&request),
            "registry" => crate::source_registry_acquisition::invoke(&request),
            "custody" => crate::source_payload_custody::invoke(&request),
            "payload-import" => crate::source_payload_import::invoke(&request),
            "inventory" => crate::source_item_inventory::invoke(&request),
            _ => Err("unsupported acquisition family".into()),
        }?;
        Ok::<Value, String>(value)
    })();
    let code = if result.is_ok() { 0 } else { 2 };
    let response = match result {
        Ok(value) => json!({"kind":"result","value":value}),
        Err(message) => json!({"kind":"error","error":"AcquisitionBatchError","message":message}),
    };
    let mut out = std::io::stdout().lock();
    // This is a transport envelope, not a source canonicalization event.
    // Raw-manifest Base64 and verified context deliberately repeat source data.
    let encoded = serde_json::to_vec(&response)
        .map_err(|e| e.to_string())
        .and_then(|mut raw| {
            if raw.len() >= RESPONSE_BYTES {
                return Err("acquisition response exceeds wire budget".into());
            }
            raw.push(b'\n');
            out.write_all(&raw).map_err(|e| e.to_string())
        });
    match encoded {
        Ok(()) => code,
        Err(_) => 2,
    }
}
