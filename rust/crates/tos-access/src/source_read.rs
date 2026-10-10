//! Exact-source transport requests. Source selection and disclosure remain owner-owned.
use crate::{AccessError, AccessErrorCode, AccessExecutor, AccessProfile};
use std::io::{Read, Write};
use tos_foundation::{JsonMode, JsonValue, emit_value_preserved_json, parse_json};

pub const MAX_REQUEST_BYTES: usize = 65_536;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Operation {
    Capabilities,
    Contract,
    Discover,
    Read,
}
impl Operation {
    pub fn software_only(self) -> bool {
        matches!(self, Self::Capabilities | Self::Contract)
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "tos.source.read.capabilities" => Some(Self::Capabilities),
            "tos.source.read.contract" => Some(Self::Contract),
            "tos.source.handle.discover" => Some(Self::Discover),
            "tos.source.read" => Some(Self::Read),
            _ => None,
        }
    }
    pub fn http(method: &str, path: &str) -> Option<Self> {
        match (method, path.split_once('?').map_or(path, |(p, _)| p)) {
            ("GET" | "HEAD", "/api/source/capabilities") => Some(Self::Capabilities),
            ("GET" | "HEAD", "/api/source/contracts") => Some(Self::Contract),
            ("POST", "/api/source/handles") => Some(Self::Discover),
            ("POST", "/api/source/read") => Some(Self::Read),
            _ => None,
        }
    }
}
#[derive(Debug)]
pub struct Request {
    pub operation: Operation,
    pub body: Vec<u8>,
}
impl Request {
    pub fn from_arguments(
        operation: Operation,
        value: &JsonValue,
        profile: AccessProfile,
    ) -> Result<Self, AccessError> {
        if value.as_object().is_none() {
            return Err(invalid());
        }
        let mut limits = profile.json_limits();
        limits.max_bytes = limits.max_bytes.min(MAX_REQUEST_BYTES);
        let body = emit_value_preserved_json(value, limits).map_err(|_| invalid())?;
        Ok(Self { operation, body })
    }
    pub fn from_bytes(
        operation: Operation,
        raw: &[u8],
        profile: AccessProfile,
    ) -> Result<Self, AccessError> {
        if raw.len() > MAX_REQUEST_BYTES.min(profile.max_request_bytes) {
            return Err(AccessError::new(
                AccessErrorCode::BudgetExceeded,
                "source request byte budget exceeded",
            ));
        }
        // Maintained file/stdin source requests reject duplicate members.
        let parsed = parse_json(raw, JsonMode::PublishedStrict, profile.json_limits())
            .map_err(|_| invalid())?;
        Self::from_arguments(operation, parsed.root(), profile)
    }
}
fn invalid() -> AccessError {
    AccessError::new(
        AccessErrorCode::InvalidRequest,
        "invalid exact-source request",
    )
}

pub fn run_cli(
    args: &[String],
    executor: &dyn AccessExecutor,
    profile: AccessProfile,
    stdin: &mut dyn Read,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Option<i32> {
    if args.first().map(String::as_str) != Some("source") {
        return None;
    }
    #[cfg(not(target_arch = "wasm32"))]
    if args.get(1).map(String::as_str) == Some("selected-owner") {
        return Some(crate::selected_source_owner_cli::run(
            args, profile, stdin, stdout, stderr,
        ));
    }
    #[cfg(not(target_arch = "wasm32"))]
    if args.get(1).map(String::as_str) == Some("owner-provider-phase") {
        return Some(crate::source_owner_provider_phase::run(
            args, stdin, stdout, stderr,
        ));
    }
    let operation = match args.get(1).map(String::as_str) {
        Some("capabilities") => Operation::Capabilities,
        Some("contracts") => Operation::Contract,
        Some("discover") => Operation::Discover,
        Some("read") => Operation::Read,
        _ => return None,
    };
    let request = (|| {
        let raw = if matches!(operation, Operation::Capabilities | Operation::Contract) {
            if args.len() != 2 {
                return Err(invalid());
            }
            b"{}".to_vec()
        } else {
            if args.len() != 3 {
                return Err(invalid());
            }
            let mut raw = Vec::new();
            let cap = MAX_REQUEST_BYTES.min(profile.max_request_bytes) as u64 + 1;
            if args[2] == "-" {
                stdin
                    .take(cap)
                    .read_to_end(&mut raw)
                    .map_err(|_| invalid())?;
            } else {
                std::fs::File::open(&args[2])
                    .map_err(|_| invalid())?
                    .take(cap)
                    .read_to_end(&mut raw)
                    .map_err(|_| invalid())?;
            }
            raw
        };
        Request::from_bytes(operation, &raw, profile)
    })();
    Some(match request {
        Err(error) => {
            let _ = writeln!(stderr, "{}: {}", error.code_str(), error.message);
            2
        }
        Ok(request) => match crate::checked_execute(profile.deadline_probe(), |probe| {
            executor.source_read(request, probe)
        }) {
            Ok(packet) => crate::cli::write_packet(packet, profile, stdout, stderr),
            Err(error) => {
                let _ = writeln!(stderr, "{}: {}", error.code_str(), error.message);
                1
            }
        },
    })
}

/// Software discovery never opens a source or mints a disclosure grant.
pub(crate) fn software_packet(
    operation: Operation,
    probe: std::sync::Arc<dyn tos_query::AbortProbe>,
) -> Result<crate::PreparedPacket<'static>, AccessError> {
    use tos_command::source_read_owner::{SourceReadOperation, software_packet};
    crate::knowledge::check_abort(&probe)?;
    let op = match operation {
        Operation::Capabilities => SourceReadOperation::Capabilities,
        Operation::Contract => SourceReadOperation::Contract,
        _ => {
            return Err(AccessError::new(
                AccessErrorCode::Unavailable,
                "exact source reader unavailable: no selected owner",
            ));
        }
    };
    let descriptor = software_packet(op).map_err(|_| {
        AccessError::new(
            AccessErrorCode::Unavailable,
            "source software contract unavailable",
        )
    })?;
    let body = portable_packet(operation, descriptor)?;
    crate::knowledge::check_abort(&probe)?;
    Ok(crate::PreparedPacket {
        body,
        fence: Box::new(SoftwareFence(probe)),
    })
}
/// The owner keeps a compact internal descriptor; portable adapters publish the
/// maintained software contract envelope. Data selection cannot replace it.
pub(crate) fn portable_packet(operation: Operation, body: Vec<u8>) -> Result<Vec<u8>, AccessError> {
    if operation != Operation::Contract {
        return Ok(body);
    }
    use tos_foundation::{CanonicalProfile, JsonLimits, JsonString, canonical_bytes_v1};
    const SOURCE_REF: &str = "access/contracts/source-read.v1.schema.json";
    const CONTRACT: &[u8] =
        include_bytes!("../../../../access/contracts/source-read.v1.schema.json");
    let limits = JsonLimits {
        max_bytes: 65_536,
        ..JsonLimits::default()
    };
    let packaged = |raw: &[u8]| {
        parse_json(raw, JsonMode::PublishedStrict, limits)
            .map(|parsed| parsed.into_root())
            .map_err(|_| {
                AccessError::new(
                    AccessErrorCode::Unavailable,
                    "packaged source software contract invalid",
                )
            })
    };
    let text = |value: &str| JsonValue::String(JsonString::from_utf8(value));
    let object = |fields: Vec<(&str, JsonValue)>| {
        JsonValue::Object(
            fields
                .into_iter()
                .map(|(key, value)| (JsonString::from_utf8(key), value))
                .collect(),
        )
    };
    let packet = object(vec![
        ("schema", text("tos_source_read_contract_bundle_v1")),
        ("contract", packaged(CONTRACT)?),
        ("descriptor", packaged(&body)?),
        ("source_ref", text(SOURCE_REF)),
        (
            "authority_boundary",
            object(vec![
                ("is_source", JsonValue::Bool(false)),
                ("writes_to_source", JsonValue::Bool(false)),
                ("grants_current_use", JsonValue::Bool(false)),
                ("source_owner", text("Tree-of-Sophia/source-witnesses")),
                (
                    "note",
                    text(
                        "Exact metadata selection grants no text access. An explicitly selected native owner separately checks recorded public rights for native_public_unit; no arbitrary source access, new rights or current-use grant follows.",
                    ),
                ),
            ]),
        ),
    ]);
    canonical_bytes_v1(&packet, CanonicalProfile::SourceRecordDigestV1, limits).map_err(|_| {
        AccessError::new(
            AccessErrorCode::BudgetExceeded,
            "source software contract packet byte budget exceeded",
        )
    })
}
struct SoftwareFence(std::sync::Arc<dyn tos_query::AbortProbe>);
impl crate::DisclosureFence for SoftwareFence {
    fn recheck(&mut self) -> Result<(), AccessError> {
        crate::knowledge::check_abort(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source_profile() -> AccessProfile {
        AccessProfile::new(4096, 4096, 4096)
    }

    #[test]
    fn source_requests_reject_duplicate_members_before_owner_dispatch() {
        let error = Request::from_bytes(
            Operation::Discover,
            br#"{"selector":{"layer":"metadata_record"},"selector":{"layer":"claim_record"}}"#,
            source_profile(),
        )
        .err()
        .expect("duplicate JSON members must be refused");
        assert_eq!(error.code, AccessErrorCode::InvalidRequest);
    }

    #[test]
    fn source_requests_enforce_the_owner_request_byte_cap() {
        let body = vec![b' '; MAX_REQUEST_BYTES + 1];
        let error = Request::from_bytes(Operation::Read, &body, source_profile())
            .err()
            .expect("oversized request must be refused before parsing");
        assert_eq!(error.code, AccessErrorCode::BudgetExceeded);
    }
}
