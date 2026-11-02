//! Versioned deployment resource configuration shared by native and WASM consumers.
use crate::{
    FoundationError, FoundationErrorCode, JsonLimits, JsonMode, JsonNumberKind, JsonString,
    JsonValue, Result, parse_json,
};

pub const DEPLOYMENT_PROFILE_MAX_BYTES: usize = 65_536;
pub const DEFAULT_DEPLOYMENT_PROFILE: &[u8] =
    include_bytes!("../../../../access/profiles/deployment-profile.default.v1.json");

fn invalid(detail: impl Into<String>) -> FoundationError {
    FoundationError::new(FoundationErrorCode::InvalidJson, detail)
}
type Object<'a> = &'a [(JsonString, JsonValue)];
fn object_unchecked(value: &JsonValue) -> Result<Object<'_>> {
    value.as_object().ok_or_else(|| invalid("expected object"))
}
fn object<'a>(value: &'a JsonValue, names: &[&str]) -> Result<Object<'a>> {
    let entries = object_unchecked(value)?;
    for (key, _) in entries {
        if !key.as_str().is_some_and(|key| names.contains(&key)) {
            return Err(invalid(format!(
                "unknown profile field: {:?}",
                key.as_str()
            )));
        }
    }
    Ok(entries)
}
fn member<'a>(entries: Object<'a>, name: &str) -> Option<&'a JsonValue> {
    entries
        .iter()
        .find(|(key, _)| key.as_str() == Some(name))
        .map(|(_, value)| value)
}
fn number(value: &JsonValue, name: &str, minimum: u64) -> Result<u64> {
    match value {
        JsonValue::Number(n) if n.kind == JsonNumberKind::Int => n
            .lexeme
            .parse::<u64>()
            .ok()
            .filter(|v| *v >= minimum)
            .ok_or_else(|| invalid(format!("invalid range for {name}"))),
        _ => Err(invalid(format!("expected integer for {name}"))),
    }
}
fn optional_number(value: &JsonValue, name: &str, minimum: u64) -> Result<Option<u64>> {
    if matches!(value, JsonValue::Null) {
        Ok(None)
    } else {
        number(value, name, minimum).map(Some)
    }
}
fn version(value: &JsonValue) -> Result<String> {
    match value {
        JsonValue::String(s) if s.as_str() == Some("tos_deployment_profile_v1") => {
            Ok("tos_deployment_profile_v1".into())
        }
        _ => Err(invalid("unsupported deployment profile version")),
    }
}

/// Parse a bounded profile and fill omitted fields from the embedded default file.
pub fn parse_deployment_profile(raw: &[u8]) -> Result<DeploymentProfile> {
    let limits = JsonLimits::new(DEPLOYMENT_PROFILE_MAX_BYTES, 16, 4096, 20)?;
    let input = parse_json(raw, JsonMode::PublishedStrict, limits)?;
    let default = parse_json(
        DEFAULT_DEPLOYMENT_PROFILE,
        JsonMode::PublishedStrict,
        limits,
    )?;
    DeploymentProfile::read(input.root(), default.root())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransportHttpProfile {
    pub max_header_bytes: u64,
    pub max_connections: u64,
}
impl TransportHttpProfile {
    fn read(value: &JsonValue, default: &JsonValue) -> Result<Self> {
        let object = object(value, &["max_header_bytes", "max_connections"])?;
        let defaults = object_unchecked(default)?;
        Ok(Self {
            max_header_bytes: number(
                member(object, "max_header_bytes")
                    .unwrap_or(member(defaults, "max_header_bytes").unwrap()),
                "max_header_bytes",
                1,
            )?,
            max_connections: number(
                member(object, "max_connections")
                    .unwrap_or(member(defaults, "max_connections").unwrap()),
                "max_connections",
                1,
            )?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransportMcpProfile {
    pub max_sessions: u64,
    pub max_connections: u64,
    pub session_idle_seconds: u64,
}
impl TransportMcpProfile {
    fn read(value: &JsonValue, default: &JsonValue) -> Result<Self> {
        let object = object(
            value,
            &["max_sessions", "max_connections", "session_idle_seconds"],
        )?;
        let defaults = object_unchecked(default)?;
        Ok(Self {
            max_sessions: number(
                member(object, "max_sessions").unwrap_or(member(defaults, "max_sessions").unwrap()),
                "max_sessions",
                1,
            )?,
            max_connections: number(
                member(object, "max_connections")
                    .unwrap_or(member(defaults, "max_connections").unwrap()),
                "max_connections",
                1,
            )?,
            session_idle_seconds: number(
                member(object, "session_idle_seconds")
                    .unwrap_or(member(defaults, "session_idle_seconds").unwrap()),
                "session_idle_seconds",
                1,
            )?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransportAccessProfile {
    pub max_request_bytes: u64,
    pub max_response_bytes: u64,
    pub max_line_bytes: u64,
}
impl TransportAccessProfile {
    fn read(value: &JsonValue, default: &JsonValue) -> Result<Self> {
        let object = object(
            value,
            &["max_request_bytes", "max_response_bytes", "max_line_bytes"],
        )?;
        let defaults = object_unchecked(default)?;
        Ok(Self {
            max_request_bytes: number(
                member(object, "max_request_bytes")
                    .unwrap_or(member(defaults, "max_request_bytes").unwrap()),
                "max_request_bytes",
                1,
            )?,
            max_response_bytes: number(
                member(object, "max_response_bytes")
                    .unwrap_or(member(defaults, "max_response_bytes").unwrap()),
                "max_response_bytes",
                1,
            )?,
            max_line_bytes: number(
                member(object, "max_line_bytes")
                    .unwrap_or(member(defaults, "max_line_bytes").unwrap()),
                "max_line_bytes",
                1,
            )?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransportProfile {
    pub http: TransportHttpProfile,
    pub mcp: TransportMcpProfile,
    pub access: TransportAccessProfile,
}
impl TransportProfile {
    fn read(value: &JsonValue, default: &JsonValue) -> Result<Self> {
        let object = object(value, &["http", "mcp", "access"])?;
        let defaults = object_unchecked(default)?;
        Ok(Self {
            http: TransportHttpProfile::read(
                member(object, "http").unwrap_or(member(defaults, "http").unwrap()),
                member(defaults, "http").unwrap(),
            )?,
            mcp: TransportMcpProfile::read(
                member(object, "mcp").unwrap_or(member(defaults, "mcp").unwrap()),
                member(defaults, "mcp").unwrap(),
            )?,
            access: TransportAccessProfile::read(
                member(object, "access").unwrap_or(member(defaults, "access").unwrap()),
                member(defaults, "access").unwrap(),
            )?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceReadProfile {
    pub max_request_bytes: u64,
}
impl SourceReadProfile {
    fn read(value: &JsonValue, default: &JsonValue) -> Result<Self> {
        let object = object(value, &["max_request_bytes"])?;
        let defaults = object_unchecked(default)?;
        Ok(Self {
            max_request_bytes: number(
                member(object, "max_request_bytes")
                    .unwrap_or(member(defaults, "max_request_bytes").unwrap()),
                "max_request_bytes",
                1,
            )?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryIndexedProfile {
    pub default_page_size: u64,
    pub max_page_size: u64,
    pub max_cursor_bytes: u64,
}
impl QueryIndexedProfile {
    fn read(value: &JsonValue, default: &JsonValue) -> Result<Self> {
        let object = object(
            value,
            &["default_page_size", "max_page_size", "max_cursor_bytes"],
        )?;
        let defaults = object_unchecked(default)?;
        Ok(Self {
            default_page_size: number(
                member(object, "default_page_size")
                    .unwrap_or(member(defaults, "default_page_size").unwrap()),
                "default_page_size",
                1,
            )?,
            max_page_size: number(
                member(object, "max_page_size")
                    .unwrap_or(member(defaults, "max_page_size").unwrap()),
                "max_page_size",
                1,
            )?,
            max_cursor_bytes: number(
                member(object, "max_cursor_bytes")
                    .unwrap_or(member(defaults, "max_cursor_bytes").unwrap()),
                "max_cursor_bytes",
                1,
            )?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuerySourceTextProfile {
    pub max_cursor_bytes: u64,
    pub max_response_bytes: u64,
}
impl QuerySourceTextProfile {
    fn read(value: &JsonValue, default: &JsonValue) -> Result<Self> {
        let object = object(value, &["max_cursor_bytes", "max_response_bytes"])?;
        let defaults = object_unchecked(default)?;
        Ok(Self {
            max_cursor_bytes: number(
                member(object, "max_cursor_bytes")
                    .unwrap_or(member(defaults, "max_cursor_bytes").unwrap()),
                "max_cursor_bytes",
                1,
            )?,
            max_response_bytes: number(
                member(object, "max_response_bytes")
                    .unwrap_or(member(defaults, "max_response_bytes").unwrap()),
                "max_response_bytes",
                1,
            )?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuerySourceGapWorkerProfile {
    pub max_source_bytes: u64,
    pub max_response_bytes: u64,
    pub max_work_steps: u64,
}
impl QuerySourceGapWorkerProfile {
    fn read(value: &JsonValue, default: &JsonValue) -> Result<Self> {
        let object = object(
            value,
            &["max_source_bytes", "max_response_bytes", "max_work_steps"],
        )?;
        let defaults = object_unchecked(default)?;
        Ok(Self {
            max_source_bytes: number(
                member(object, "max_source_bytes")
                    .unwrap_or(member(defaults, "max_source_bytes").unwrap()),
                "max_source_bytes",
                1,
            )?,
            max_response_bytes: number(
                member(object, "max_response_bytes")
                    .unwrap_or(member(defaults, "max_response_bytes").unwrap()),
                "max_response_bytes",
                1,
            )?,
            max_work_steps: number(
                member(object, "max_work_steps")
                    .unwrap_or(member(defaults, "max_work_steps").unwrap()),
                "max_work_steps",
                1,
            )?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryProfile {
    pub indexed: QueryIndexedProfile,
    pub source_text: QuerySourceTextProfile,
    pub source_gap_worker: QuerySourceGapWorkerProfile,
}
impl QueryProfile {
    fn read(value: &JsonValue, default: &JsonValue) -> Result<Self> {
        let object = object(value, &["indexed", "source_text", "source_gap_worker"])?;
        let defaults = object_unchecked(default)?;
        Ok(Self {
            indexed: QueryIndexedProfile::read(
                member(object, "indexed").unwrap_or(member(defaults, "indexed").unwrap()),
                member(defaults, "indexed").unwrap(),
            )?,
            source_text: QuerySourceTextProfile::read(
                member(object, "source_text").unwrap_or(member(defaults, "source_text").unwrap()),
                member(defaults, "source_text").unwrap(),
            )?,
            source_gap_worker: QuerySourceGapWorkerProfile::read(
                member(object, "source_gap_worker")
                    .unwrap_or(member(defaults, "source_gap_worker").unwrap()),
                member(defaults, "source_gap_worker").unwrap(),
            )?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmissionAcquisitionProfile {
    pub max_payload_bytes: u64,
    pub max_record_bytes: u64,
}
impl AdmissionAcquisitionProfile {
    fn read(value: &JsonValue, default: &JsonValue) -> Result<Self> {
        let object = object(value, &["max_payload_bytes", "max_record_bytes"])?;
        let defaults = object_unchecked(default)?;
        Ok(Self {
            max_payload_bytes: number(
                member(object, "max_payload_bytes")
                    .unwrap_or(member(defaults, "max_payload_bytes").unwrap()),
                "max_payload_bytes",
                1,
            )?,
            max_record_bytes: number(
                member(object, "max_record_bytes")
                    .unwrap_or(member(defaults, "max_record_bytes").unwrap()),
                "max_record_bytes",
                1,
            )?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmissionTextLayerProfile {
    pub max_record_bytes: u64,
    pub max_package_bytes: u64,
}
impl AdmissionTextLayerProfile {
    fn read(value: &JsonValue, default: &JsonValue) -> Result<Self> {
        let object = object(value, &["max_record_bytes", "max_package_bytes"])?;
        let defaults = object_unchecked(default)?;
        Ok(Self {
            max_record_bytes: number(
                member(object, "max_record_bytes")
                    .unwrap_or(member(defaults, "max_record_bytes").unwrap()),
                "max_record_bytes",
                1,
            )?,
            max_package_bytes: number(
                member(object, "max_package_bytes")
                    .unwrap_or(member(defaults, "max_package_bytes").unwrap()),
                "max_package_bytes",
                1,
            )?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmissionProfile {
    pub acquisition: AdmissionAcquisitionProfile,
    pub text_layer: AdmissionTextLayerProfile,
}
impl AdmissionProfile {
    fn read(value: &JsonValue, default: &JsonValue) -> Result<Self> {
        let object = object(value, &["acquisition", "text_layer"])?;
        let defaults = object_unchecked(default)?;
        Ok(Self {
            acquisition: AdmissionAcquisitionProfile::read(
                member(object, "acquisition").unwrap_or(member(defaults, "acquisition").unwrap()),
                member(defaults, "acquisition").unwrap(),
            )?,
            text_layer: AdmissionTextLayerProfile::read(
                member(object, "text_layer").unwrap_or(member(defaults, "text_layer").unwrap()),
                member(defaults, "text_layer").unwrap(),
            )?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveInstallProfile {
    pub max_total_bytes: Option<u64>,
    pub max_archive_bytes: Option<u64>,
    pub max_members: Option<u64>,
    pub max_metadata_bytes: Option<u64>,
}
impl ArchiveInstallProfile {
    fn read(value: &JsonValue, default: &JsonValue) -> Result<Self> {
        let object = object(
            value,
            &[
                "max_total_bytes",
                "max_archive_bytes",
                "max_members",
                "max_metadata_bytes",
            ],
        )?;
        let defaults = object_unchecked(default)?;
        Ok(Self {
            max_total_bytes: optional_number(
                member(object, "max_total_bytes")
                    .unwrap_or(member(defaults, "max_total_bytes").unwrap()),
                "max_total_bytes",
                1,
            )?,
            max_archive_bytes: optional_number(
                member(object, "max_archive_bytes")
                    .unwrap_or(member(defaults, "max_archive_bytes").unwrap()),
                "max_archive_bytes",
                1,
            )?,
            max_members: optional_number(
                member(object, "max_members").unwrap_or(member(defaults, "max_members").unwrap()),
                "max_members",
                1,
            )?,
            max_metadata_bytes: optional_number(
                member(object, "max_metadata_bytes")
                    .unwrap_or(member(defaults, "max_metadata_bytes").unwrap()),
                "max_metadata_bytes",
                224,
            )?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportsPublicD1Profile {
    pub max_work_bytes: u64,
    pub max_model_bytes: u64,
    pub max_postings: u64,
    pub max_build_seconds: Option<u64>,
    pub max_state_bytes: Option<u64>,
    pub max_json_visits: Option<u64>,
}
impl ExportsPublicD1Profile {
    fn read(value: &JsonValue, default: &JsonValue) -> Result<Self> {
        let object = object(
            value,
            &[
                "max_work_bytes",
                "max_model_bytes",
                "max_postings",
                "max_build_seconds",
                "max_state_bytes",
                "max_json_visits",
            ],
        )?;
        let defaults = object_unchecked(default)?;
        Ok(Self {
            max_work_bytes: number(
                member(object, "max_work_bytes")
                    .unwrap_or(member(defaults, "max_work_bytes").unwrap()),
                "max_work_bytes",
                1,
            )?,
            max_model_bytes: number(
                member(object, "max_model_bytes")
                    .unwrap_or(member(defaults, "max_model_bytes").unwrap()),
                "max_model_bytes",
                1,
            )?,
            max_postings: number(
                member(object, "max_postings").unwrap_or(member(defaults, "max_postings").unwrap()),
                "max_postings",
                1,
            )?,
            max_build_seconds: optional_number(
                member(object, "max_build_seconds")
                    .unwrap_or(member(defaults, "max_build_seconds").unwrap()),
                "max_build_seconds",
                1,
            )?,
            max_state_bytes: optional_number(
                member(object, "max_state_bytes")
                    .unwrap_or(member(defaults, "max_state_bytes").unwrap()),
                "max_state_bytes",
                131072,
            )?,
            max_json_visits: optional_number(
                member(object, "max_json_visits")
                    .unwrap_or(member(defaults, "max_json_visits").unwrap()),
                "max_json_visits",
                1,
            )?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportsProfile {
    pub public_d1: ExportsPublicD1Profile,
}
impl ExportsProfile {
    fn read(value: &JsonValue, default: &JsonValue) -> Result<Self> {
        let object = object(value, &["public_d1"])?;
        let defaults = object_unchecked(default)?;
        Ok(Self {
            public_d1: ExportsPublicD1Profile::read(
                member(object, "public_d1").unwrap_or(member(defaults, "public_d1").unwrap()),
                member(defaults, "public_d1").unwrap(),
            )?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeploymentProfile {
    pub schema_version: String,
    pub transport: TransportProfile,
    pub source_read: SourceReadProfile,
    pub query: QueryProfile,
    pub admission: AdmissionProfile,
    pub archive_install: ArchiveInstallProfile,
    pub exports: ExportsProfile,
}
impl DeploymentProfile {
    fn read(value: &JsonValue, default: &JsonValue) -> Result<Self> {
        let object = object(
            value,
            &[
                "transport",
                "source_read",
                "query",
                "admission",
                "archive_install",
                "exports",
                "schema_version",
            ],
        )?;
        let defaults = object_unchecked(default)?;
        Ok(Self {
            schema_version: version(
                member(object, "schema_version")
                    .ok_or_else(|| invalid("schema_version is required"))?,
            )?,
            transport: TransportProfile::read(
                member(object, "transport").unwrap_or(member(defaults, "transport").unwrap()),
                member(defaults, "transport").unwrap(),
            )?,
            source_read: SourceReadProfile::read(
                member(object, "source_read").unwrap_or(member(defaults, "source_read").unwrap()),
                member(defaults, "source_read").unwrap(),
            )?,
            query: QueryProfile::read(
                member(object, "query").unwrap_or(member(defaults, "query").unwrap()),
                member(defaults, "query").unwrap(),
            )?,
            admission: AdmissionProfile::read(
                member(object, "admission").unwrap_or(member(defaults, "admission").unwrap()),
                member(defaults, "admission").unwrap(),
            )?,
            archive_install: ArchiveInstallProfile::read(
                member(object, "archive_install")
                    .unwrap_or(member(defaults, "archive_install").unwrap()),
                member(defaults, "archive_install").unwrap(),
            )?,
            exports: ExportsProfile::read(
                member(object, "exports").unwrap_or(member(defaults, "exports").unwrap()),
                member(defaults, "exports").unwrap(),
            )?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deployment_profile_defaults_and_partial_overrides() {
        let default = parse_deployment_profile(DEFAULT_DEPLOYMENT_PROFILE).unwrap();
        let partial =
            parse_deployment_profile(br#"{"schema_version":"tos_deployment_profile_v1"}"#).unwrap();
        assert_eq!(default, partial);
        assert_eq!(default.transport.http.max_header_bytes, 8192);
        assert_eq!(default.query.indexed.default_page_size, 40);
        assert_eq!(default.archive_install.max_total_bytes, Some(4_294_967_296));
        assert_eq!(
            default.archive_install.max_archive_bytes,
            Some(1_073_741_824)
        );
        assert_eq!(default.archive_install.max_members, Some(4_096));
        assert_eq!(default.archive_install.max_metadata_bytes, Some(4_194_304));
        let changed = parse_deployment_profile(br#"{"schema_version":"tos_deployment_profile_v1","archive_install":{"max_members":123}}"#).unwrap();
        assert_eq!(changed.archive_install.max_members, Some(123));
        assert_eq!(changed.transport, default.transport);
    }

    #[test]
    fn deployment_profile_rejects_invalid_inputs() {
        for raw in [
            r#"{}"#,
            r#"{"schema_version":"other"}"#,
            r#"{"schema_version":"tos_deployment_profile_v1","extra":1}"#,
            r#"{"schema_version":"tos_deployment_profile_v1","transport":{"http":{"extra":1}}}"#,
            r#"{"schema_version":"tos_deployment_profile_v1","transport":{"http":{"max_connections":0}}}"#,
            r#"{"schema_version":"tos_deployment_profile_v1","archive_install":{"max_metadata_bytes":223}}"#,
            r#"{"schema_version":"tos_deployment_profile_v1","archive_install":{"max_members":18446744073709551616}}"#,
            r#"{"schema_version":"tos_deployment_profile_v1","archive_install":{"max_members":1.5}}"#,
            r#"{"schema_version":"tos_deployment_profile_v1","schema_version":"tos_deployment_profile_v1"}"#,
            r#"{"#,
        ] {
            assert!(
                parse_deployment_profile(raw.as_bytes()).is_err(),
                "accepted {raw}"
            );
        }
        assert!(parse_deployment_profile(&vec![b' '; DEPLOYMENT_PROFILE_MAX_BYTES + 1]).is_err());
    }
}
