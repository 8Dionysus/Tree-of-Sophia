//! MCP 2025-11-25 stdio framing and lifecycle for registered native tools.

use std::{
    io::{self, BufRead, Write},
    sync::Arc,
    time::Duration,
};
use tos_query::AbortProbe;

use tos_foundation::{JsonMode, JsonNumberKind, JsonValue, parse_json};

use crate::common::{
    AccessExecutor, AccessProfile, DisclosureFence, MCP_TOOL, Params, SEARCH_MCP_TOOL,
    SourceDescendDelivery, checked_execute, checked_execute_source_descend, json_string,
    json_string_len, mcp_tool_list, registered_operations, validate_packet,
};

const RPC_PREFIX: &[u8] = b"{\"jsonrpc\":\"2.0\",\"id\":";
const RESULT_PREFIX: &[u8] = b",\"result\":";
const TEXT_PREFIX: &[u8] = b"{\"content\":[{\"type\":\"text\",\"text\":";
const STRUCTURED_PREFIX: &[u8] = b"}],\"structuredContent\":";

fn tool_result_frame_len(
    packet_bytes: usize,
    escaped_text_bytes: usize,
    id_bytes: usize,
) -> Option<usize> {
    [
        RPC_PREFIX.len(),
        id_bytes,
        RESULT_PREFIX.len(),
        TEXT_PREFIX.len(),
        escaped_text_bytes,
        STRUCTURED_PREFIX.len(),
        packet_bytes,
        2, // result and JSON-RPC closing braces
        1, // newline
    ]
    .into_iter()
    .try_fold(0usize, usize::checked_add)
}

/// Conservative complete tool-result frame allowance from declared packet and
/// request caps. JSON quoting can expand each UTF-8 byte to at most six bytes;
/// both the text copy and a canonicalized string request ID include quotes.
/// The raw structured copy, fixed envelope and newline are included. Returns
/// None on arithmetic overflow; this does not select a production allowance.
pub fn tool_result_frame_byte_bound(
    max_packet_bytes: usize,
    max_request_bytes: usize,
) -> Option<usize> {
    let escaped_text = max_packet_bytes.checked_mul(6)?.checked_add(2)?;
    let id = max_request_bytes.checked_mul(6)?.checked_add(2)?;
    tool_result_frame_len(max_packet_bytes, escaped_text, id)
}

// Software metadata replies carry the same absolute request probe through
// final output too; data replies replace this with their stronger held fence.
struct RpcProbeFence(Arc<dyn AbortProbe>);
impl DisclosureFence for RpcProbeFence {
    fn recheck(&mut self) -> Result<(), crate::AccessError> {
        crate::knowledge::check_abort(&self.0)
    }
}

pub(crate) struct McpSession {
    handshake_accepted: bool,
    initialized: bool,
    pending_initialize: bool,
    profile: AccessProfile,
    pending_fence: Option<Box<dyn DisclosureFence>>,
    pending_id: Option<Vec<u8>>,
    pending_tool_result: bool,
    software: Option<Arc<crate::site::SoftwareSite>>,
    #[cfg(not(target_arch = "wasm32"))]
    source_text_search: Option<Arc<crate::source_text_search::SelectedPgSourceTextSearch>>,
}

impl McpSession {
    pub(crate) fn with_software(
        profile: AccessProfile,
        software: Option<Arc<crate::site::SoftwareSite>>,
    ) -> Self {
        Self {
            handshake_accepted: false,
            initialized: false,
            pending_initialize: false,
            profile,
            pending_fence: None,
            pending_id: None,
            pending_tool_result: false,
            software,
            #[cfg(not(target_arch = "wasm32"))]
            source_text_search: None,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn with_selected_source_text_search(
        profile: AccessProfile,
        software: Option<Arc<crate::site::SoftwareSite>>,
        source_text_search: Option<Arc<crate::source_text_search::SelectedPgSourceTextSearch>>,
    ) -> Self {
        let mut session = Self::with_software(profile, software);
        session.source_text_search = source_text_search;
        session
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn source_text_search_selected(&self) -> bool {
        self.source_text_search.is_some()
    }

    #[cfg(target_arch = "wasm32")]
    fn source_text_search_selected(&self) -> bool {
        false
    }


    pub(crate) fn initializing(&self) -> bool {
        self.pending_initialize
    }

    pub(crate) fn handle_line_with_probe(
        &mut self,
        executor: &dyn AccessExecutor,
        line: &[u8],
        probe: Arc<dyn AbortProbe>,
    ) -> Option<Vec<u8>> {
        self.pending_initialize = false;
        self.pending_fence = None;
        self.pending_id = None;
        self.pending_tool_result = false;
        if line.len() > self.profile.max_line_bytes {
            return Some(rpc_error(b"null", -32700, "MCP frame exceeds byte budget"));
        }
        let document = match parse_json(line, JsonMode::RequestLastWins, self.profile.json_limits())
        {
            Ok(value) => value,
            Err(_) => return Some(rpc_error(b"null", -32700, "Invalid JSON")),
        };
        let value = document.root();
        let Some(_) = value.as_object() else {
            return Some(rpc_error(b"null", -32600, "Invalid Request"));
        };
        let id = match value.object_get("id") {
            None => None,
            Some(JsonValue::String(s)) => s.as_str().map(|s| json_string(s)),
            Some(JsonValue::Number(n)) if n.kind == JsonNumberKind::Int => {
                Some(n.lexeme.as_bytes().to_vec())
            }
            _ => return Some(rpc_error(b"null", -32600, "Invalid Request")),
        };
        self.pending_id = id.clone();
        if value.object_get("jsonrpc").and_then(JsonValue::as_str) != Some("2.0") {
            return Some(rpc_error(
                id.as_deref().unwrap_or(b"null"),
                -32600,
                "Invalid Request",
            ));
        }
        let Some(method) = value.object_get("method").and_then(JsonValue::as_str) else {
            return Some(rpc_error(
                id.as_deref().unwrap_or(b"null"),
                -32600,
                "Invalid Request",
            ));
        };
        if id.is_none() {
            if method == "notifications/initialized" && self.handshake_accepted {
                self.initialized = true;
            }
            // Notifications never produce a JSON-RPC response. An in-flight
            // synchronous call cannot be interrupted on this stdio profile.
            return None;
        }
        let id = id.unwrap();
        if method == "initialize" {
            if self.handshake_accepted {
                return Some(rpc_error(&id, -32600, "MCP session already initialized"));
            }
            let requested = value
                .object_get("params")
                .and_then(|p| p.object_get("protocolVersion"))
                .and_then(JsonValue::as_str);
            if requested != Some("2025-11-25") {
                return Some(rpc_error(&id, -32602, "Unsupported MCP protocol version"));
            }
            self.pending_initialize = true;
            return Some(rpc_result(&id, br#"{"protocolVersion":"2025-11-25","capabilities":{"tools":{"listChanged":false},"resources":{"subscribe":false,"listChanged":false},"prompts":{"listChanged":false}},"serverInfo":{"name":"tree-of-sophia","version":"0.0.0"}}"#));
        }
        if !self.initialized {
            return Some(rpc_error(&id, -32002, "MCP session is not initialized"));
        }
        self.pending_fence = Some(Box::new(RpcProbeFence(Arc::clone(&probe))));
        match method {
            "ping" => Some(rpc_result(&id, b"{}")),
            "resources/list" => Some(rpc_result(&id, &crate::mcp_resources::list(false))),
            "resources/templates/list" => Some(rpc_result(&id, &crate::mcp_resources::list(true))),
            "prompts/list" => Some(rpc_result(&id, &crate::mcp_prompts::list())),
            "prompts/get" => match crate::knowledge::check_abort(&probe)
                .and_then(|_| {
                    crate::mcp_prompts::get(
                        value.object_get("params"),
                        self.profile.max_response_bytes,
                    )
                })
                .and_then(|body| {
                    crate::knowledge::check_abort(&probe)?;
                    Ok(body)
                }) {
                Ok(body) => Some(rpc_result(&id, &body)),
                Err(error) => Some(rpc_error(&id, -32602, error.message)),
            },
            "resources/read" => {
                let uri = match crate::mcp_resources::uri(value.object_get("params")) {
                    Ok(uri) => uri,
                    Err(error) => return Some(rpc_error(&id, -32602, error.message)),
                };
                let result = checked_execute(probe, |probe| {
                    let request = crate::mcp_resources::request(uri, self.profile)?;
                    executor.knowledge(request, probe)
                })
                .and_then(|packet| {
                    if packet.body.len() > self.profile.max_response_bytes {
                        return Err(crate::AccessError::new(
                            crate::AccessErrorCode::BudgetExceeded,
                            "resource packet exceeds byte budget",
                        ));
                    }
                    validate_packet(&packet.body, self.profile.max_response_bytes)?;
                    Ok(packet)
                });
                match result {
                    Ok(packet) => {
                        let text = match std::str::from_utf8(&packet.body) {
                            Ok(text) => text,
                            Err(_) => {
                                return Some(rpc_error(
                                    &id,
                                    -32603,
                                    "Resource packet is not UTF-8",
                                ));
                            }
                        };
                        match crate::mcp_resources::contents(uri, text, id.len(), self.profile) {
                            Ok(body) => {
                                self.pending_fence = Some(packet.fence);
                                self.pending_id = Some(id.clone());
                                Some(rpc_result(&id, &body))
                            }
                            Err(error) => Some(rpc_error(&id, -32603, error.message)),
                        }
                    }
                    Err(error) => Some(rpc_error(&id, -32000, error.message)),
                }
            }
            "tools/list" => match mcp_tool_list(
                executor,
                self.software.is_some(),
                self.source_text_search_selected(),
            ) {
                Ok(list) => Some(rpc_result(&id, &list)),
                Err(_) => Some(rpc_error(
                    &id,
                    -32603,
                    "Native operation registry unavailable",
                )),
            },
            "tools/call" => {
                self.pending_tool_result = true;
                let params = value.object_get("params");
                let name = params
                    .and_then(|p| p.object_get("name"))
                    .and_then(JsonValue::as_str);
                let known = registered_operations().ok().is_some_and(|operations| {
                    operations
                        .iter()
                        .any(|operation| Some(operation.mcp_tool.as_str()) == name)
                });
                let available = match name {
                    Some(MCP_TOOL) => executor.source_descend_available(),
                    Some("tos_source_gap_search") => executor.source_gap_available(),
                    Some(crate::reading::PUBLIC_WORD_CAPABILITY_TOOL)
                    | Some(crate::reading::PUBLIC_READING_CAPABILITY_TOOL) => true,
                    Some(crate::knowledge::ACCESS_HEALTH_OPERATION) => {
                        executor.access_health_available()
                    }
                    Some(crate::knowledge::PREPARED_STATUS_OPERATION) => {
                        executor.prepared_status_available()
                    }
                    Some(crate::word_analysis::MCP_TOOL) => {
                        self.software.is_some() || executor.word_analysis_available()
                    }
                    Some(crate::reading::MCP_TOOL) => executor.reading_search_available(),
                    Some(SEARCH_MCP_TOOL) => {
                        executor.knowledge_search_indexed_available()
                            || executor.knowledge_search_legacy_available()
                            || executor.knowledge_search_compressed_available()
                    }
                    Some(crate::SOURCE_TEXT_SEARCH_MCP_TOOL) => self.source_text_search_selected(),
                    Some(name)
                        if registered_operations().ok().is_some_and(|ops| {
                            ops.iter().any(|op| {
                                op.mcp_tool == name
                                    && crate::source_read::Operation::from_id(&op.operation_id)
                                        .is_some()
                            })
                        }) =>
                    {
                        executor.source_read_available()
                            || registered_operations().ok().is_some_and(|ops| {
                                ops.iter().any(|op| {
                                    op.mcp_tool == name
                                        && crate::source_read::Operation::from_id(&op.operation_id)
                                            .is_some_and(|o| o.software_only())
                                })
                            })
                    }
                    Some(name) => registered_operations()
                        .ok()
                        .and_then(|ops| ops.iter().find(|op| op.mcp_tool == name))
                        .and_then(|op| crate::KnowledgeOperation::from_id(&op.operation_id))
                        .is_some_and(|op| {
                            op == crate::KnowledgeOperation::ExplorationContracts
                                || executor.knowledge_available(op)
                        }),
                    None => false,
                };
                if known && !available && name == Some(crate::SOURCE_TEXT_SEARCH_MCP_TOOL) {
                    return Some(tool_error(
                        &id,
                        "durable PostgreSQL source-text consumer unavailable",
                    ));
                }
                // A declared source operation without its selected owner has
                // a genuine unavailable diagnostic, not an unknown tool name.
                // Keep it absent from discovery; this grants no source access.
                if known
                    && !available
                    && registered_operations().ok().is_some_and(|ops| {
                        ops.iter().any(|op| {
                            Some(op.mcp_tool.as_str()) == name
                                && crate::source_read::Operation::from_id(&op.operation_id)
                                    .is_some_and(|operation| !operation.software_only())
                        })
                    })
                {
                    return Some(tool_error(
                        &id,
                        "exact source reader unavailable: no selected owner",
                    ));
                }
                if !known || !available {
                    return Some(rpc_error(&id, -32602, "Unknown tool"));
                }
                let Some(arguments) = params.and_then(|p| p.object_get("arguments")) else {
                    return Some(rpc_error(&id, -32602, "Tool arguments required"));
                };
                let operation = registered_operations()
                    .ok()
                    .and_then(|ops| ops.iter().find(|op| Some(op.mcp_tool.as_str()) == name));
                let allowed = operation
                    .and_then(|op| op.input_schema.object_get("properties"))
                    .and_then(JsonValue::as_object);
                if arguments.as_object().is_none_or(|fields| {
                    fields.iter().any(|(name, _)| {
                        !allowed
                            .is_some_and(|properties| properties.iter().any(|(key, _)| key == name))
                    })
                }) {
                    return Some(rpc_error(&id, -32602, "Unknown tool argument"));
                }
                let result = checked_execute(probe, |probe| match name {
                    Some(crate::word_analysis::MCP_TOOL) => {
                        if executor.word_analysis_available() {
                            crate::word_analysis::prepare_capability(
                                executor,
                                arguments,
                                self.profile,
                                probe,
                            )
                        } else {
                            self.software
                                .as_ref()
                                .expect("available software")
                                .word_analysis_negative(arguments, probe, self.profile)
                        }
                    }
                    Some(crate::reading::PUBLIC_WORD_CAPABILITY_TOOL) => {
                        crate::reading::public_word_analysis_capability(
                            self.profile.max_response_bytes,
                        )
                        .map(|body| crate::PreparedPacket {
                            body,
                            fence: Box::new(RpcProbeFence(probe)),
                        })
                    }
                    Some(crate::reading::PUBLIC_READING_CAPABILITY_TOOL) => {
                        crate::reading::public_reading_capability(self.profile.max_response_bytes)
                            .map(|body| crate::PreparedPacket {
                                body,
                                fence: Box::new(RpcProbeFence(probe)),
                            })
                    }
                    Some("tos_source_gap_search") => {
                        tos_query::source_gap::SourceGapRequest::from_json(arguments)
                            .map_err(|_| {
                                crate::AccessError::new(
                                    crate::AccessErrorCode::InvalidRequest,
                                    "invalid source-gap request",
                                )
                            })
                            .and_then(|request| executor.source_gap(request, probe))
                    }
                    Some(MCP_TOOL) => Params::from_json(arguments)
                        .and_then(|request| executor.source_descend(request, probe)),
                    Some(crate::reading::MCP_TOOL) => crate::reading::from_arguments(arguments)
                        .and_then(|request| executor.reading_search(request, probe)),

                    Some(SEARCH_MCP_TOOL) => {
                        crate::search::SearchRequest::from_arguments(arguments)
                            .and_then(|request| request.execute(executor, probe))
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    Some(crate::SOURCE_TEXT_SEARCH_MCP_TOOL) => {
                        let selection = self.source_text_search.as_ref().ok_or_else(|| {
                            crate::AccessError::new(
                                crate::AccessErrorCode::Unavailable,
                                "no durable PostgreSQL source-text consumer selected",
                            )
                        })?;
                        tos_query::source_text_search::SourceTextSearchRequest::from_value(
                            arguments,
                        )
                        .map_err(|_| {
                            crate::AccessError::new(
                                crate::AccessErrorCode::InvalidRequest,
                                "invalid source-text search request",
                            )
                        })
                        .and_then(|request| selection.execute_request(request, self.profile, probe))
                    }
                    #[cfg(target_arch = "wasm32")]
                    Some(crate::SOURCE_TEXT_SEARCH_MCP_TOOL) => Err(crate::AccessError::new(
                        crate::AccessErrorCode::Unavailable,
                        "durable PostgreSQL source-text search is native-only",
                    )),
                    Some(_)
                        if crate::source_read::Operation::from_id(
                            &operation.unwrap().operation_id,
                        )
                        .is_some() =>
                    {
                        let op = crate::source_read::Operation::from_id(
                            &operation.unwrap().operation_id,
                        )
                        .unwrap();
                        crate::source_read::Request::from_arguments(op, arguments, self.profile)
                            .and_then(|request| executor.source_read(request, probe))
                    }
                    Some(_) => crate::KnowledgeOperation::from_id(&operation.unwrap().operation_id)
                        .ok_or_else(|| {
                            crate::AccessError::new(
                                crate::AccessErrorCode::Unavailable,
                                "native operation unavailable",
                            )
                        })
                        .and_then(|op| {
                            if op == crate::KnowledgeOperation::AccessHealth {
                                return executor.access_health(Arc::clone(&probe));
                            }
                            if op == crate::KnowledgeOperation::PreparedStatus {
                                return executor.prepared_status(Arc::clone(&probe));
                            }
                            crate::KnowledgeRequest::from_arguments(op, arguments).and_then(
                                |request| {
                                    if matches!(
                                        request,
                                        crate::KnowledgeRequest::ExplorationContracts
                                    ) {
                                        crate::exploration_contracts::execute(
                                            executor,
                                            self.profile.max_response_bytes,
                                        )
                                    } else {
                                        executor.knowledge(request, probe)
                                    }
                                },
                            )
                        }),
                    None => unreachable!(),
                })
                .and_then(|packet| {
                    if packet.body.len() > self.profile.max_response_bytes {
                        return Err(crate::common::AccessError::new(
                            crate::common::AccessErrorCode::BudgetExceeded,
                            "query response budget exceeded",
                        ));
                    }
                    validate_packet(&packet.body, self.profile.max_response_bytes)?;
                    Ok(packet)
                });
                match result {
                    Ok(packet) => {
                        let text = match std::str::from_utf8(&packet.body) {
                            Ok(text) => text,
                            Err(_) => {
                                return Some(rpc_error(&id, -32603, "Query packet is not UTF-8"));
                            }
                        };
                        let frame_len = json_string_len(text).and_then(|escaped| {
                            tool_result_frame_len(packet.body.len(), escaped, id.len())
                        });
                        if !frame_len
                            .is_some_and(|length| length <= self.profile.max_mcp_frame_bytes)
                        {
                            return Some(rpc_error(
                                &id,
                                -32603,
                                "MCP response frame exceeds byte budget",
                            ));
                        }
                        let mut body = Vec::with_capacity(frame_len.unwrap());
                        body.extend_from_slice(TEXT_PREFIX);
                        body.extend(json_string(text));
                        body.extend_from_slice(STRUCTURED_PREFIX);
                        body.extend(packet.body);
                        body.push(b'}');
                        let frame = rpc_result(&id, &body);
                        self.pending_fence = Some(packet.fence);
                        self.pending_id = Some(id.clone());
                        Some(frame)
                    }
                    Err(error) => Some(tool_error(&id, error.message)),
                }
            }
            _ => Some(rpc_error(&id, -32601, "Method not found")),
        }
    }

    /// Source navigation has a callback-local owner fence. When selected, frame
    /// construction and the complete transport write happen before that owner
    /// callback can release its PostgreSQL/policy transaction.
    pub(crate) fn handle_line_with_probe_and_delivery(
        &mut self,
        executor: &dyn AccessExecutor,
        line: &[u8],
        probe: Arc<dyn AbortProbe>,
        suffix: &[u8],
        write: &mut dyn FnMut(&[u8]) -> io::Result<()>,
    ) -> io::Result<(Option<Vec<u8>>, bool)> {
        let Some((id, request)) = source_descend_call(line, self.profile) else {
            return Ok((self.handle_line_with_probe(executor, line, probe), false));
        };
        if !self.initialized || !executor.source_descend_available() {
            return Ok((self.handle_line_with_probe(executor, line, probe), false));
        }
        self.pending_initialize = false;
        self.pending_fence = None;
        self.pending_id = None;
        self.pending_tool_result = false;
        let request = match request {
            Ok(request) => request,
            Err(error) => {
                source_descend_reply(&id, Err(error), suffix, self.profile, write)
                    .map_err(|_| io::Error::other("MCP source navigation refusal write failed"))?;
                return Ok((None, true));
            }
        };
        let mut emitted = false;
        let mut io_failure = None;
        let mut checked_write = |frame: &[u8]| match write(frame) {
            Ok(()) => Ok(()),
            Err(error) => {
                io_failure = Some(error);
                Err(io::Error::other(
                    "MCP source navigation transport write failed",
                ))
            }
        };
        let mut delivery: Box<SourceDescendDelivery<'_>> = Box::new(|result| {
            emitted = true;
            source_descend_reply(&id, result, suffix, self.profile, &mut checked_write)
        });
        let outcome = checked_execute_source_descend(
            probe,
            |probe, delivery| executor.source_descend_with_delivery(request, probe, delivery),
            delivery.as_mut(),
        );
        drop(delivery);
        drop(checked_write);
        if let Some(error) = io_failure {
            return Err(error);
        }
        let outcome_error = outcome.err();
        if !emitted {
            let error = outcome_error.unwrap_or_else(|| {
                crate::AccessError::new(
                    crate::AccessErrorCode::Unavailable,
                    "source descent owner returned without delivery",
                )
            });
            source_descend_reply(&id, Err(error), suffix, self.profile, write)
                .map_err(|_| io::Error::other("MCP source navigation refusal write failed"))?;
        } else if let Some(error) = outcome_error {
            return Err(io::Error::other(format!(
                "MCP source navigation delivery failed: {}",
                error.message
            )));
        }
        Ok((None, true))
    }

    /// Retain the same disclosure fence through transport output and final flush.
    /// Initialize is committed only after the complete response was delivered.
    pub(crate) fn write_reply(
        &mut self,
        mut frame: Vec<u8>,
        suffix: &[u8],
        write: impl FnOnce(&[u8]) -> io::Result<()>,
    ) -> io::Result<()> {
        let fence = self.pending_fence.take();
        let mut fence = fence;
        if let Some(current) = fence.as_mut() {
            if let Err(error) = current.recheck() {
                frame = if self.pending_tool_result {
                    tool_error(self.pending_id.as_deref().unwrap_or(b"null"), error.message)
                } else {
                    rpc_error(
                        self.pending_id.as_deref().unwrap_or(b"null"),
                        -32000,
                        error.message,
                    )
                };
                fence = None;
            }
        }
        if !frame
            .len()
            .checked_add(suffix.len())
            .is_some_and(|bytes| bytes <= self.profile.max_mcp_frame_bytes)
        {
            frame = rpc_error(
                self.pending_id.as_deref().unwrap_or(b"null"),
                -32603,
                "MCP response frame exceeds byte budget",
            );
            fence = None;
            self.pending_initialize = false;
            if !frame
                .len()
                .checked_add(suffix.len())
                .is_some_and(|bytes| bytes <= self.profile.max_mcp_frame_bytes)
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "MCP refusal frame exceeds byte budget",
                ));
            }
        }
        frame.extend_from_slice(suffix);
        write(&frame)?;
        if self.pending_initialize {
            self.handshake_accepted = true;
        }
        drop(fence);
        Ok(())
    }
}

fn rpc_result(id: &[u8], result: &[u8]) -> Vec<u8> {
    let mut out = RPC_PREFIX.to_vec();
    out.extend_from_slice(id);
    out.extend_from_slice(RESULT_PREFIX);
    out.extend_from_slice(result);
    out.push(b'}');
    out
}

fn rpc_error(id: &[u8], code: i32, message: &str) -> Vec<u8> {
    let mut out = RPC_PREFIX.to_vec();
    out.extend_from_slice(id);
    out.extend_from_slice(b",\"error\":{\"code\":");
    out.extend_from_slice(code.to_string().as_bytes());
    out.extend_from_slice(b",\"message\":");
    out.extend(json_string(message));
    out.extend_from_slice(b"}}");
    out
}

fn tool_error(id: &[u8], message: &str) -> Vec<u8> {
    let mut body = b"{\"content\":[{\"type\":\"text\",\"text\":".to_vec();
    body.extend(json_string(message));
    body.extend_from_slice(b"}],\"isError\":true}");
    rpc_result(id, &body)
}

fn tool_result(id: &[u8], text: &str, packet: &[u8]) -> Vec<u8> {
    let mut body = Vec::with_capacity(packet.len().saturating_add(128));
    body.extend_from_slice(TEXT_PREFIX);
    body.extend(json_string(text));
    body.extend_from_slice(STRUCTURED_PREFIX);
    body.extend_from_slice(packet);
    body.push(b'}');
    rpc_result(id, &body)
}

fn source_descend_call(
    line: &[u8],
    profile: AccessProfile,
) -> Option<(Vec<u8>, Result<Params, crate::AccessError>)> {
    if line.len() > profile.max_line_bytes {
        return None;
    }
    let document = parse_json(line, JsonMode::RequestLastWins, profile.json_limits()).ok()?;
    let value = document.root();
    if value.object_get("jsonrpc").and_then(JsonValue::as_str) != Some("2.0")
        || value.object_get("method").and_then(JsonValue::as_str) != Some("tools/call")
    {
        return None;
    }
    let id = match value.object_get("id")? {
        JsonValue::String(value) => json_string(value.as_str()?),
        JsonValue::Number(value) if value.kind == JsonNumberKind::Int => {
            value.lexeme.as_bytes().to_vec()
        }
        _ => return None,
    };
    let params = value.object_get("params")?;
    if params.object_get("name").and_then(JsonValue::as_str) != Some(MCP_TOOL) {
        return None;
    }
    let arguments = params.object_get("arguments")?;
    Some((id, Params::from_json(arguments)))
}

fn source_descend_reply<'packet>(
    id: &[u8],
    result: Result<crate::PreparedPacket<'packet>, crate::AccessError>,
    suffix: &[u8],
    profile: AccessProfile,
    write: &mut dyn FnMut(&[u8]) -> io::Result<()>,
) -> Result<(), crate::AccessError> {
    let (mut packet, mut packet_error) = match result {
        Ok(packet) => (Some(packet), None),
        Err(error) => (None, Some(error)),
    };
    if let Some(packet) = packet.as_mut() {
        packet_error = if packet.body.len() > profile.max_response_bytes {
            Some(crate::AccessError::new(
                crate::AccessErrorCode::BudgetExceeded,
                "source descent response budget exceeded",
            ))
        } else if let Err(error) = validate_packet(&packet.body, profile.max_response_bytes) {
            Some(error)
        } else if std::str::from_utf8(&packet.body).is_err() {
            Some(crate::AccessError::new(
                crate::AccessErrorCode::CorruptSelectedCarrier,
                "source descent packet is not UTF-8",
            ))
        } else {
            packet.fence.recheck().err()
        };
    }
    let mut frame = match (packet.as_ref(), packet_error.as_ref()) {
        (_, Some(error)) => tool_error(id, &error.message),
        (Some(packet), None) => match std::str::from_utf8(&packet.body) {
            Ok(text) => tool_result(id, text, &packet.body),
            Err(_) => tool_error(id, "source descent packet is not UTF-8"),
        },
        (None, None) => tool_error(id, "source descent refused"),
    };
    if !frame
        .len()
        .checked_add(suffix.len())
        .is_some_and(|bytes| bytes <= profile.max_mcp_frame_bytes)
    {
        frame = rpc_error(id, -32603, "MCP response frame exceeds byte budget");
        if !frame
            .len()
            .checked_add(suffix.len())
            .is_some_and(|bytes| bytes <= profile.max_mcp_frame_bytes)
        {
            return Err(crate::AccessError::new(
                crate::AccessErrorCode::BudgetExceeded,
                "MCP refusal frame exceeds byte budget",
            ));
        }
    }
    frame.extend_from_slice(suffix);
    let write_result = write(&frame);
    // An I/O error can arrive after some or all bytes were emitted. Keep the
    // disclosure fence through that ambiguous outcome as well, and prefer a
    // failed final fence when both checks fail.
    let final_fence = packet
        .as_mut()
        .and_then(|packet| packet.fence.recheck().err());
    if let Err(_error) = write_result {
        return Err(final_fence.unwrap_or_else(|| {
            crate::AccessError::new(
                crate::AccessErrorCode::Cancelled,
                "MCP source descent response write failed",
            )
        }));
    }
    if let Some(error) = final_fence {
        return Err(error);
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn run_stdio_with_software(
    executor: &dyn AccessExecutor,
    profile: AccessProfile,
    software: Option<Arc<crate::site::SoftwareSite>>,
) -> io::Result<()> {
    run_stdio_with_source_text_search(executor, profile, software, None)
}
#[cfg(target_arch = "wasm32")]
pub fn run_stdio_with_software(
    executor: &dyn AccessExecutor,
    profile: AccessProfile,
    software: Option<Arc<crate::site::SoftwareSite>>,
) -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    run_io_with_software(stdin.lock(), stdout.lock(), executor, profile, software)
}
#[cfg(not(target_arch = "wasm32"))]
pub fn run_stdio_with_source_text_search(
    executor: &dyn AccessExecutor,
    profile: AccessProfile,
    software: Option<Arc<crate::site::SoftwareSite>>,
    source_text_search: Option<Arc<crate::source_text_search::SelectedPgSourceTextSearch>>,
) -> io::Result<()> {
    #[cfg(unix)]
    {
        let stdin = io::stdin();
        let stdout = io::stdout();
        let mut input = stdin.lock();
        let mut output = stdout.lock();
        let mut session =
            McpSession::with_selected_source_text_search(profile, software, source_text_search);
        run_io_loop(
            &mut input,
            &mut output,
            &mut session,
            executor,
            profile,
            Some(libc::STDOUT_FILENO),
        )
    }
    #[cfg(not(unix))]
    {
        let stdin = io::stdin();
        let stdout = io::stdout();
        run_io_with_source_text_search(
            stdin.lock(),
            stdout.lock(),
            executor,
            profile,
            software,
            source_text_search,
        )
    }
}
#[cfg(target_arch = "wasm32")]
pub fn run_stdio_with_source_text_search(
    executor: &dyn AccessExecutor,
    profile: AccessProfile,
    software: Option<Arc<crate::site::SoftwareSite>>,
    _source_text_search: Option<Arc<crate::source_text_search::SelectedPgSourceTextSearch>>,
) -> io::Result<()> {
    run_stdio_with_software(executor, profile, software)
}

/// Same protocol path over supplied streams for real byte-level client tests.
pub fn run_io<R: BufRead, W: Write>(
    input: R,
    output: W,
    executor: &dyn AccessExecutor,
    profile: AccessProfile,
) -> io::Result<()> {
    run_io_with_software(input, output, executor, profile, None)
}
pub fn run_io_with_software<R: BufRead, W: Write>(
    input: R,
    output: W,
    executor: &dyn AccessExecutor,
    profile: AccessProfile,
    software: Option<Arc<crate::site::SoftwareSite>>,
) -> io::Result<()> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        run_io_with_source_text_search(input, output, executor, profile, software, None)
    }
    #[cfg(target_arch = "wasm32")]
    {
        run_io_without_source_text_search(input, output, executor, profile, software)
    }
}
#[cfg(target_arch = "wasm32")]
fn run_io_without_source_text_search<R: BufRead, W: Write>(
    mut input: R,
    mut output: W,
    executor: &dyn AccessExecutor,
    profile: AccessProfile,
    software: Option<Arc<crate::site::SoftwareSite>>,
) -> io::Result<()> {
    let mut session = McpSession::with_software(profile, software);
    run_io_loop(
        &mut input,
        &mut output,
        &mut session,
        executor,
        profile,
        None,
    )
}
#[cfg(not(target_arch = "wasm32"))]
pub fn run_io_with_source_text_search<R: BufRead, W: Write>(
    mut input: R,
    mut output: W,
    executor: &dyn AccessExecutor,
    profile: AccessProfile,
    software: Option<Arc<crate::site::SoftwareSite>>,
    source_text_search: Option<Arc<crate::source_text_search::SelectedPgSourceTextSearch>>,
) -> io::Result<()> {
    let mut session =
        McpSession::with_selected_source_text_search(profile, software, source_text_search);
    run_io_loop(
        &mut input,
        &mut output,
        &mut session,
        executor,
        profile,
        None,
    )
}
#[cfg(target_arch = "wasm32")]
pub fn run_io_with_source_text_search<R: BufRead, W: Write>(
    input: R,
    output: W,
    executor: &dyn AccessExecutor,
    profile: AccessProfile,
    software: Option<Arc<crate::site::SoftwareSite>>,
    _source_text_search: Option<Arc<crate::source_text_search::SelectedPgSourceTextSearch>>,
) -> io::Result<()> {
    run_io_with_software(input, output, executor, profile, software)
}

fn run_io_loop<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    session: &mut McpSession,
    executor: &dyn AccessExecutor,
    profile: AccessProfile,
    source_fd: Option<i32>,
) -> io::Result<()> {
    loop {
        let mut line = Vec::new();
        let mut overflow = false;
        loop {
            let mut byte = [0u8; 1];
            if input.read(&mut byte)? == 0 {
                return Ok(());
            }
            if byte[0] == b'\n' {
                break;
            }
            if line.len() < profile.max_line_bytes {
                line.push(byte[0]);
            } else {
                overflow = true;
            }
        }
        session.pending_id = None;
        session.pending_initialize = false;
        let response = if overflow {
            Some(rpc_error(b"null", -32700, "MCP frame exceeds byte budget"))
        } else {
            let operation_profile =
                if source_fd.is_some() && source_descend_call(&line, profile).is_some() {
                    profile.with_query_timeout(Duration::from_secs(30))
                } else {
                    profile
                };
            let probe = operation_profile.deadline_probe();
            let mut write = |frame: &[u8]| {
                if let Some(fd) = source_fd {
                    crate::common::write_fd_with_probe(fd, frame, probe.as_ref())
                } else {
                    output.write_all(frame)?;
                    output.flush()
                }
            };
            let (response, _) = session.handle_line_with_probe_and_delivery(
                executor,
                &line,
                Arc::clone(&probe),
                b"\n",
                &mut write,
            )?;
            drop(write);
            response
        };
        if let Some(frame) = response {
            session.write_reply(frame, b"\n", |frame| {
                output.write_all(frame)?;
                output.flush()
            })?;
        }
    }
}
