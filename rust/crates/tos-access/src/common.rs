use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use tos_foundation::{
    JsonLimits, JsonMode, JsonString, JsonValue, emit_value_preserved_json, parse_json,
};
use tos_query::{
    AbortProbe, AbortReason, Budget, DisclosureLease, QueryError, QueryErrorCode, ReadModel,
    SourceDescendRequest, source_descend,
};

pub const OPERATION_ID: &str = "tos.source.descend";
pub const MCP_TOOL: &str = "tos_source_descend";
pub const HTTP_PREFIX: &str = "/api/source/navigation/";
pub const SEARCH_OPERATION_ID: &str = "tos.knowledge.search";
pub const SEARCH_MCP_TOOL: &str = "tos_knowledge_search";
pub const SEARCH_HTTP_PATH: &str = "/api/knowledge/search";
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
    pub http_method: String,
    pub http_path: String,
    pub cli_command: Option<String>,
}

static OPERATIONS: OnceLock<Result<Vec<RegisteredOperation>, AccessError>> = OnceLock::new();

pub fn registered_operations() -> Result<&'static [RegisteredOperation], AccessError> {
    OPERATIONS
        .get_or_init(|| {
            let limits = JsonLimits {
                max_bytes: 65_536,
                max_depth: 16,
                max_visits: 4096,
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
            if items.is_empty() {
                return Err(AccessError::new(
                    AccessErrorCode::Unavailable,
                    "native operation descriptor list invalid",
                ));
            }
            let mut registered = Vec::with_capacity(items.len());
            let mut seen_ops = std::collections::BTreeSet::new();
            let mut seen_tools = std::collections::BTreeSet::new();
            for item in items {
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
                let method = http
                    .and_then(|h| h.object_get("method"))
                    .and_then(JsonValue::as_str);
                let path = http
                    .and_then(|h| h.object_get("path_template"))
                    .and_then(JsonValue::as_str);
                let known = op.is_some_and(|id| {
                    id == OPERATION_ID
                        || id == SEARCH_OPERATION_ID
                        || crate::knowledge::KnowledgeOperation::from_id(id).is_some()
                });
                if !known
                    || !op.is_some_and(|id| seen_ops.insert(id))
                    || !tool.is_some_and(|id| !id.is_empty() && seen_tools.insert(id))
                    || !matches!(method, Some("GET" | "POST"))
                    || !path.is_some_and(|p| p.starts_with("/api/"))
                    || schema.and_then(JsonValue::as_object).is_none()
                    || description.is_none()
                {
                    return Err(AccessError::new(
                        AccessErrorCode::Unavailable,
                        "native operation binding invalid",
                    ));
                }
                registered.push(RegisteredOperation {
                    operation_id: op.unwrap().to_owned(),
                    mcp_tool: tool.unwrap().to_owned(),
                    mcp_description: description.unwrap().to_owned(),
                    input_schema: schema.unwrap().clone(),
                    http_method: method.unwrap().to_owned(),
                    http_path: path.unwrap().to_owned(),
                    cli_command: item
                        .object_get("cli")
                        .and_then(|c| c.object_get("command"))
                        .and_then(JsonValue::as_str)
                        .map(str::to_owned),
                });
            }
            Ok(registered)
        })
        .as_ref()
        .map(Vec::as_slice)
        .map_err(Clone::clone)
}

pub(crate) fn mcp_tool_list(executor: &dyn AccessExecutor) -> Result<Vec<u8>, AccessError> {
    let operations = registered_operations()?;
    let string = |text: &str| JsonValue::String(JsonString::from_utf8(text));
    let object = |fields: Vec<(&str, JsonValue)>| {
        JsonValue::Object(
            fields
                .into_iter()
                .map(|(k, v)| (JsonString::from_utf8(k), v))
                .collect(),
        )
    };
    let tools = operations
        .iter()
        .filter(|operation| match operation.operation_id.as_str() {
            OPERATION_ID => executor.source_descend_available(),
            SEARCH_OPERATION_ID => executor.knowledge_search_indexed_available(),
            id => crate::knowledge::KnowledgeOperation::from_id(id)
                .is_some_and(|op| executor.knowledge_available(op)),
        })
        .map(|operation| {
            object(vec![
                ("name", string(&operation.mcp_tool)),
                ("description", string(&operation.mcp_description)),
                ("inputSchema", operation.input_schema.clone()),
                (
                    "annotations",
                    object(vec![("readOnlyHint", JsonValue::Bool(true))]),
                ),
            ])
        })
        .collect();
    let list = object(vec![("tools", JsonValue::Array(tools))]);
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
    CursorExpired,
    PublicationPending,
    BudgetExceeded,
    Cancelled,
    DeadlineExceeded,
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
            AccessErrorCode::CursorExpired => 410,
            AccessErrorCode::StaleSelection | AccessErrorCode::PublicationPending => 409,
            AccessErrorCode::BudgetExceeded => 413,
            AccessErrorCode::Cancelled | AccessErrorCode::DeadlineExceeded => 408,
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
            AccessErrorCode::CursorExpired => "cursor_expired",
            AccessErrorCode::StaleSelection => "stale_selection",
            AccessErrorCode::PublicationPending => "publication_pending",
            AccessErrorCode::BudgetExceeded => "budget_exceeded",
            AccessErrorCode::Cancelled => "cancelled",
            AccessErrorCode::DeadlineExceeded => "deadline_exceeded",
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
            QueryErrorCode::Cancelled => AccessErrorCode::Cancelled,
            QueryErrorCode::DeadlineExceeded => AccessErrorCode::DeadlineExceeded,
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

/// The direct knowledge API retains its legacy default. This request names
/// indexed v2 explicitly and carries vocabulary values as selected data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexedSearchParams {
    pub query: String,
    pub sources: Vec<String>,
    pub kind_ids: Vec<String>,
    pub predicate_ids: Vec<String>,
    pub cursor: Option<String>,
    pub limit: usize,
}

impl IndexedSearchParams {
    pub fn new(
        query: String,
        sources: Vec<String>,
        kind_ids: Vec<String>,
        predicate_ids: Vec<String>,
        cursor: Option<String>,
        limit: usize,
    ) -> Result<Self, AccessError> {
        // Exact Unicode strip/lower and the three-code-point eligibility
        // check belong to QRY's pinned text profile, not host Rust casing.
        if query.chars().count() > 256
            || !(1..=100).contains(&limit)
            || cursor.as_ref().is_some_and(|value| value.len() > 65_536)
            || [&sources, &kind_ids, &predicate_ids].iter().any(|values| {
                values.len() > 256
                    || values
                        .iter()
                        .any(|value| value.is_empty() || value.chars().count() > 256)
            })
        {
            return Err(AccessError::new(
                AccessErrorCode::InvalidRequest,
                "invalid indexed knowledge search request",
            ));
        }
        Ok(Self {
            query,
            sources,
            kind_ids,
            predicate_ids,
            cursor,
            limit,
        })
    }

    pub fn from_json(value: &JsonValue) -> Result<Self, AccessError> {
        if value.as_object().is_none() {
            return Err(AccessError::new(
                AccessErrorCode::InvalidRequest,
                "search arguments must be an object",
            ));
        }
        let mode = value.object_get("mode").and_then(JsonValue::as_str);
        if mode != Some("indexed")
            || value
                .object_get("offset")
                .is_some_and(|v| v.as_u64() != Some(0))
        {
            return Err(AccessError::new(
                AccessErrorCode::InvalidRequest,
                "indexed mode required without offset",
            ));
        }
        let query = value
            .object_get("query")
            .map(|v| {
                v.as_str().ok_or_else(|| {
                    AccessError::new(AccessErrorCode::InvalidRequest, "query must be a string")
                })
            })
            .transpose()?
            .unwrap_or("")
            .to_owned();
        let list = |name: &str| -> Result<Vec<String>, AccessError> {
            let Some(raw) = value.object_get(name) else {
                return Ok(Vec::new());
            };
            let Some(array) = raw.as_array() else {
                return Err(AccessError::new(
                    AccessErrorCode::InvalidRequest,
                    "search filter must be an array",
                ));
            };
            array
                .iter()
                .map(|item| {
                    item.as_str().map(str::to_owned).ok_or_else(|| {
                        AccessError::new(
                            AccessErrorCode::InvalidRequest,
                            "search filter entry must be a string",
                        )
                    })
                })
                .collect()
        };
        let cursor = match value.object_get("cursor") {
            None | Some(JsonValue::Null) => None,
            Some(raw) => Some(
                raw.as_str()
                    .ok_or_else(|| {
                        AccessError::new(
                            AccessErrorCode::InvalidRequest,
                            "cursor must be a string or null",
                        )
                    })?
                    .to_owned(),
            ),
        };
        let limit = match value.object_get("limit") {
            None => 40,
            Some(raw) => usize::try_from(raw.as_u64().ok_or_else(|| {
                AccessError::new(AccessErrorCode::InvalidRequest, "limit must be an integer")
            })?)
            .map_err(|_| {
                AccessError::new(AccessErrorCode::InvalidRequest, "limit exceeds range")
            })?,
        };
        Self::new(
            query,
            list("sources")?,
            list("kind_ids")?,
            list("predicate_ids")?,
            cursor,
            limit,
        )
    }
}

#[derive(Clone, Copy)]
pub struct AccessProfile {
    pub max_request_bytes: usize,
    pub max_response_bytes: usize,
    pub max_line_bytes: usize,
    /// Caller-selected processing deadline. None does not claim a time cap.
    pub query_timeout: Option<Duration>,
}

impl AccessProfile {
    pub fn new(max_request_bytes: usize, max_response_bytes: usize, max_line_bytes: usize) -> Self {
        Self {
            max_request_bytes,
            max_response_bytes,
            max_line_bytes,
            query_timeout: None,
        }
    }
    pub fn with_query_timeout(mut self, timeout: Duration) -> Self {
        self.query_timeout = Some(timeout);
        self
    }
    pub(crate) fn deadline_probe(self) -> Arc<dyn AbortProbe> {
        let now = Instant::now();
        Arc::new(DeadlineProbe {
            deadline: self
                .query_timeout
                .map(|timeout| now.checked_add(timeout).unwrap_or(now)),
        })
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
    /// policy, pre-disclosure fence and QRY progress-abort probe are installed.
    fn source_descend_available(&self) -> bool;
    fn source_descend(
        &self,
        request: Params,
        abort_probe: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket, AccessError>;
    /// The indexed v2 transport is registered only after a complete selected
    /// knowledge publication and its QRY executor are installed.
    fn knowledge_search_indexed_available(&self) -> bool {
        false
    }
    fn knowledge_search_indexed(
        &self,
        _: IndexedSearchParams,
        _: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket, AccessError> {
        Err(AccessError::new(
            AccessErrorCode::Unavailable,
            "indexed knowledge search unavailable",
        ))
    }
    fn knowledge_available(&self, _: crate::knowledge::KnowledgeOperation) -> bool {
        false
    }
    fn knowledge(
        &self,
        _: crate::knowledge::KnowledgeRequest,
        _: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket, AccessError> {
        Err(AccessError::new(
            AccessErrorCode::Unavailable,
            "selected knowledge operation unavailable",
        ))
    }
}

struct DeadlineProbe {
    deadline: Option<Instant>,
}
impl AbortProbe for DeadlineProbe {
    fn reason(&self) -> Option<AbortReason> {
        self.deadline
            .filter(|deadline| Instant::now() >= *deadline)
            .map(|_| AbortReason::DeadlineExceeded)
    }
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

/// Retain processing cancellation through transport validation and the final
/// pre-send disclosure check, rather than dropping it when query returns.
pub(crate) fn checked_execute(
    probe: Arc<dyn AbortProbe>,
    execute: impl FnOnce(Arc<dyn AbortProbe>) -> Result<PreparedPacket, AccessError>,
) -> Result<PreparedPacket, AccessError> {
    crate::knowledge::check_abort(&probe)?;
    let mut packet = execute(Arc::clone(&probe))?;
    crate::knowledge::check_abort(&probe)?;
    packet.fence = Box::new(AbortFence {
        inner: packet.fence,
        probe,
    });
    Ok(packet)
}
struct AbortFence {
    inner: Box<dyn DisclosureFence>,
    probe: Arc<dyn AbortProbe>,
}
impl DisclosureFence for AbortFence {
    fn recheck(&mut self) -> Result<(), AccessError> {
        crate::knowledge::check_abort(&self.probe)?;
        self.inner.recheck()?;
        crate::knowledge::check_abort(&self.probe)
    }
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

/// Exact UTF-8 byte length of `json_string` without constructing the escaped
/// representation. A transport can reject oversized envelopes before the
/// duplicate text carrier is allocated.
pub(crate) fn json_string_len(value: &str) -> Option<usize> {
    value.chars().try_fold(2usize, |length, character| {
        let bytes = match character {
            '"' | '\\' | '\n' | '\r' | '\t' => 2,
            c if c < '\u{20}' => 6,
            c => c.len_utf8(),
        };
        length.checked_add(bytes)
    })
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
