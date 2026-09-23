use std::sync::OnceLock;

use tos_foundation::{
    JsonLimits, JsonMode, JsonString, JsonValue, emit_value_preserved_json, parse_json,
};
use tos_query::{
    Budget, DisclosureLease, QueryError, QueryErrorCode, ReadModel, SourceDescendRequest,
    source_descend,
};

pub const OPERATION_ID: &str = "tos.source.descend";
pub const MCP_TOOL: &str = "tos_source_descend";
pub const HTTP_PREFIX: &str = "/api/source/navigation/";
pub const REQUEST_PROFILE: &str = "tos_request_last_wins_json_v1";
pub const DESCRIPTOR: &str = include_str!("../operations.v1.json");

pub fn descriptor() -> &'static str {
    DESCRIPTOR
}

#[derive(Clone)]
pub struct RegisteredOperation {
    pub operation_id: String,
    pub mcp_tool: String,
    pub mcp_description: String,
    pub input_schema: JsonValue,
}

static OPERATION: OnceLock<Result<RegisteredOperation, AccessError>> = OnceLock::new();

pub fn registered_operation() -> Result<&'static RegisteredOperation, AccessError> {
    OPERATION
        .get_or_init(|| {
            let limits = JsonLimits {
                max_bytes: 16_384,
                max_depth: 16,
                max_visits: 1024,
                max_integer_digits: 16,
            };
            let document = parse_json(DESCRIPTOR.as_bytes(), JsonMode::PublishedStrict, limits)
                .map_err(|_| {
                    AccessError::new(
                        AccessErrorCode::Unavailable,
                        "native operation descriptor invalid",
                    )
                })?;
            let root = document.root();
            if root
                .object_get("schema_version")
                .and_then(JsonValue::as_str)
                != Some("tos_native_access_operations_v1")
                || root
                    .object_get("request_json_profile")
                    .and_then(JsonValue::as_str)
                    != Some(REQUEST_PROFILE)
            {
                return Err(AccessError::new(
                    AccessErrorCode::Unavailable,
                    "native operation descriptor version invalid",
                ));
            }
            let items = root
                .object_get("operations")
                .and_then(JsonValue::as_array)
                .ok_or_else(|| {
                    AccessError::new(
                        AccessErrorCode::Unavailable,
                        "native operation descriptor list absent",
                    )
                })?;
            if items.len() != 1 {
                return Err(AccessError::new(
                    AccessErrorCode::Unavailable,
                    "native operation descriptor list invalid",
                ));
            }
            let item = &items[0];
            let op = item.object_get("operation_id").and_then(JsonValue::as_str);
            let mcp = item.object_get("mcp");
            let tool = mcp
                .and_then(|m| m.object_get("tool"))
                .and_then(JsonValue::as_str);
            let description = mcp
                .and_then(|m| m.object_get("description"))
                .and_then(JsonValue::as_str);
            let schema = mcp.and_then(|m| m.object_get("input_schema"));
            let http = item.object_get("http");
            if op != Some(OPERATION_ID)
                || tool != Some(MCP_TOOL)
                || http
                    .and_then(|h| h.object_get("method"))
                    .and_then(JsonValue::as_str)
                    != Some("GET")
                || http
                    .and_then(|h| h.object_get("path_template"))
                    .and_then(JsonValue::as_str)
                    != Some("/api/source/navigation/{node_id}")
                || schema.and_then(JsonValue::as_object).is_none()
                || description.is_none()
            {
                return Err(AccessError::new(
                    AccessErrorCode::Unavailable,
                    "native operation binding invalid",
                ));
            }
            Ok(RegisteredOperation {
                operation_id: op.unwrap().to_owned(),
                mcp_tool: tool.unwrap().to_owned(),
                mcp_description: description.unwrap().to_owned(),
                input_schema: schema.unwrap().clone(),
            })
        })
        .as_ref()
        .map_err(Clone::clone)
}

pub(crate) fn mcp_tool_list() -> Result<Vec<u8>, AccessError> {
    let operation = registered_operation()?;
    let string = |text: &str| JsonValue::String(JsonString::from_utf8(text));
    let object = |fields: Vec<(&str, JsonValue)>| {
        JsonValue::Object(
            fields
                .into_iter()
                .map(|(k, v)| (JsonString::from_utf8(k), v))
                .collect(),
        )
    };
    let tool = object(vec![
        ("name", string(&operation.mcp_tool)),
        ("description", string(&operation.mcp_description)),
        ("inputSchema", operation.input_schema.clone()),
        (
            "annotations",
            object(vec![("readOnlyHint", JsonValue::Bool(true))]),
        ),
    ]);
    let list = object(vec![("tools", JsonValue::Array(vec![tool]))]);
    emit_value_preserved_json(
        &list,
        JsonLimits {
            max_bytes: 32_768,
            max_depth: 32,
            max_visits: 2048,
            max_integer_digits: 16,
        },
    )
    .map_err(|_| {
        AccessError::new(
            AccessErrorCode::Unavailable,
            "native tool descriptor cannot be emitted",
        )
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessErrorCode {
    InvalidRequest,
    UnknownExactId,
    StaleSelection,
    PublicationPending,
    BudgetExceeded,
    PolicyDenied,
    IndexIncomplete,
    CorruptSelectedCarrier,
    Unavailable,
}

#[derive(Clone, Debug)]
pub struct AccessError {
    pub code: AccessErrorCode,
    pub message: &'static str,
}

impl AccessError {
    pub const fn new(code: AccessErrorCode, message: &'static str) -> Self {
        Self { code, message }
    }
    pub fn http_status(&self) -> u16 {
        match self.code {
            AccessErrorCode::InvalidRequest => 400,
            AccessErrorCode::UnknownExactId => 404,
            AccessErrorCode::StaleSelection | AccessErrorCode::PublicationPending => 409,
            AccessErrorCode::BudgetExceeded => 413,
            AccessErrorCode::PolicyDenied => 403,
            AccessErrorCode::IndexIncomplete
            | AccessErrorCode::CorruptSelectedCarrier
            | AccessErrorCode::Unavailable => 503,
        }
    }
    pub fn code_str(&self) -> &'static str {
        match self.code {
            AccessErrorCode::InvalidRequest => "invalid_request",
            AccessErrorCode::UnknownExactId => "unknown_exact_id",
            AccessErrorCode::StaleSelection => "stale_selection",
            AccessErrorCode::PublicationPending => "publication_pending",
            AccessErrorCode::BudgetExceeded => "budget_exceeded",
            AccessErrorCode::PolicyDenied => "policy_denied",
            AccessErrorCode::IndexIncomplete => "index_incomplete",
            AccessErrorCode::CorruptSelectedCarrier => "corrupt_selected_carrier",
            AccessErrorCode::Unavailable => "unavailable",
        }
    }
}

impl From<QueryError> for AccessError {
    fn from(value: QueryError) -> Self {
        let code = match value.code {
            QueryErrorCode::InvalidRequest => AccessErrorCode::InvalidRequest,
            QueryErrorCode::UnknownExactId => AccessErrorCode::UnknownExactId,
            QueryErrorCode::StaleSelection => AccessErrorCode::StaleSelection,
            QueryErrorCode::PublicationPending => AccessErrorCode::PublicationPending,
            QueryErrorCode::BudgetExceeded => AccessErrorCode::BudgetExceeded,
            QueryErrorCode::PolicyDenied => AccessErrorCode::PolicyDenied,
            QueryErrorCode::IndexIncomplete => AccessErrorCode::IndexIncomplete,
            QueryErrorCode::CorruptSelectedCarrier => AccessErrorCode::CorruptSelectedCarrier,
            QueryErrorCode::Unavailable => AccessErrorCode::Unavailable,
        };
        Self {
            code,
            message: value.message,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Params {
    pub node_id: String,
    pub max_depth: u8,
    pub limit: usize,
}

impl Params {
    pub fn new(node_id: String, max_depth: u8, limit: usize) -> Result<Self, AccessError> {
        if node_id.is_empty() || !(1..=8).contains(&max_depth) || !(1..=300).contains(&limit) {
            return Err(AccessError::new(
                AccessErrorCode::InvalidRequest,
                "invalid source descent request",
            ));
        }
        Ok(Self {
            node_id,
            max_depth,
            limit,
        })
    }
    pub fn from_json(value: &JsonValue) -> Result<Self, AccessError> {
        let node_id = value
            .object_get("node_id")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| {
                AccessError::new(AccessErrorCode::InvalidRequest, "node_id must be a string")
            })?;
        let depth = match value.object_get("max_depth") {
            Some(v) => v.as_u64().ok_or_else(|| {
                AccessError::new(
                    AccessErrorCode::InvalidRequest,
                    "max_depth must be an integer",
                )
            })?,
            None => 8,
        };
        let limit = match value.object_get("limit") {
            Some(v) => v.as_u64().ok_or_else(|| {
                AccessError::new(AccessErrorCode::InvalidRequest, "limit must be an integer")
            })?,
            None => 300,
        };
        let depth = u8::try_from(depth).map_err(|_| {
            AccessError::new(AccessErrorCode::InvalidRequest, "max_depth exceeds range")
        })?;
        let limit = usize::try_from(limit).map_err(|_| {
            AccessError::new(AccessErrorCode::InvalidRequest, "limit exceeds range")
        })?;
        Self::new(node_id.to_owned(), depth, limit)
    }
}

#[derive(Clone, Copy)]
pub struct AccessProfile {
    pub max_request_bytes: usize,
    pub max_response_bytes: usize,
    pub max_line_bytes: usize,
}

impl AccessProfile {
    pub fn new(max_request_bytes: usize, max_response_bytes: usize, max_line_bytes: usize) -> Self {
        Self {
            max_request_bytes,
            max_response_bytes,
            max_line_bytes,
        }
    }
    pub fn json_limits(self) -> JsonLimits {
        JsonLimits {
            max_bytes: self.max_request_bytes,
            max_depth: 64,
            max_visits: 30_000,
            max_integer_digits: 64,
        }
    }
}

pub trait AccessExecutor: Send + Sync {
    /// Only true when a sealed source cut, verified selected model, live source
    /// policy and pre-disclosure fence are all actually installed.
    fn source_descend_available(&self) -> bool;
    fn source_descend(&self, request: Params) -> Result<PreparedPacket, AccessError>;
}

/// Owner-selected policy/pin fence tied to the exact staged packet. The
/// transport keeps it alive through the last output write.
pub trait DisclosureFence: Send {
    fn recheck(&mut self) -> Result<(), AccessError>;
}

pub struct PreparedPacket {
    pub body: Vec<u8>,
    pub fence: Box<dyn DisclosureFence>,
}

/// One-shot binding of the common query rule to an owner-selected model.
/// Disclosure fencing belongs to the source-owner adapter.
pub struct QuerySession<M: ReadModel> {
    pub model: M,
    pub budget: Budget,
}

impl<M: ReadModel> QuerySession<M> {
    pub fn execute(&mut self, params: Params) -> Result<PreparedPacket, AccessError> {
        let request = SourceDescendRequest {
            node_id: params.node_id,
            max_depth: params.max_depth,
            limit: params.limit,
            at_least_commit_seq: None,
        };
        let result = source_descend(&mut self.model, &request, self.budget)?;
        let (body, lease) = result.into_parts();
        Ok(PreparedPacket {
            body,
            fence: Box::new(QueryDisclosureFence(lease)),
        })
    }
}

struct QueryDisclosureFence(Box<dyn DisclosureLease>);
impl DisclosureFence for QueryDisclosureFence {
    fn recheck(&mut self) -> Result<(), AccessError> {
        self.0.recheck().map_err(Into::into)
    }
}

pub(crate) fn json_string(value: &str) -> Vec<u8> {
    let mut output = Vec::with_capacity(value.len() + 2);
    output.push(b'"');
    for character in value.chars() {
        match character {
            '"' => output.extend_from_slice(br#"\""#),
            '\\' => output.extend_from_slice(br"\\"),
            '\n' => output.extend_from_slice(br"\n"),
            '\r' => output.extend_from_slice(br"\r"),
            '\t' => output.extend_from_slice(br"\t"),
            c if c < '\u{20}' => {
                output.extend_from_slice(format!("\\u{:04x}", c as u32).as_bytes())
            }
            c => {
                let mut encoded = [0u8; 4];
                output.extend_from_slice(c.encode_utf8(&mut encoded).as_bytes());
            }
        }
    }
    output.push(b'"');
    output
}

pub(crate) fn error_json(error: &AccessError) -> Vec<u8> {
    let mut out = b"{\"error\":".to_vec();
    out.extend(json_string(error.message));
    out.extend_from_slice(b",\"code\":");
    out.extend(json_string(error.code_str()));
    out.push(b'}');
    out
}

pub(crate) fn validate_packet(raw: &[u8], max_bytes: usize) -> Result<(), AccessError> {
    let limits = JsonLimits {
        max_bytes,
        max_depth: 64,
        max_visits: 300_000,
        max_integer_digits: 4_300,
    };
    let document = parse_json(raw, JsonMode::PublishedStrict, limits).map_err(|_| {
        AccessError::new(
            AccessErrorCode::CorruptSelectedCarrier,
            "query packet is not valid bounded JSON",
        )
    })?;
    if document.root().as_object().is_none() {
        return Err(AccessError::new(
            AccessErrorCode::CorruptSelectedCarrier,
            "query packet must be an object",
        ));
    }
    Ok(())
}
