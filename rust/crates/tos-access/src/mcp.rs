//! MCP 2025-11-25 stdio framing and lifecycle for registered native tools.

use std::io::{self, BufRead, Write};

use tos_foundation::{JsonMode, JsonNumberKind, JsonValue, parse_json};

use crate::common::{
    AccessExecutor, AccessProfile, DisclosureFence, IndexedSearchParams, MCP_TOOL, Params,
    SEARCH_MCP_TOOL, checked_execute, json_string, json_string_len, mcp_tool_list,
    registered_operations, validate_packet,
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

struct McpSession {
    handshake_accepted: bool,
    initialized: bool,
    profile: AccessProfile,
    pending_fence: Option<Box<dyn DisclosureFence>>,
    pending_id: Option<Vec<u8>>,
}

impl McpSession {
    fn new(profile: AccessProfile) -> Self {
        Self {
            handshake_accepted: false,
            initialized: false,
            profile,
            pending_fence: None,
            pending_id: None,
        }
    }

    fn handle_line(&mut self, executor: &dyn AccessExecutor, line: &[u8]) -> Option<Vec<u8>> {
        self.pending_fence = None;
        self.pending_id = None;
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
            self.handshake_accepted = true;
            return Some(rpc_result(&id, br#"{"protocolVersion":"2025-11-25","capabilities":{"tools":{"listChanged":false}},"serverInfo":{"name":"tree-of-sophia","version":"0.0.0"}}"#));
        }
        if !self.initialized {
            return Some(rpc_error(&id, -32002, "MCP session is not initialized"));
        }
        match method {
            "ping" => Some(rpc_result(&id, b"{}")),
            "tools/list" => match mcp_tool_list(executor) {
                Ok(list) => Some(rpc_result(&id, &list)),
                Err(_) => Some(rpc_error(
                    &id,
                    -32603,
                    "Native operation registry unavailable",
                )),
            },
            "tools/call" => {
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
                    Some(SEARCH_MCP_TOOL) => executor.knowledge_search_indexed_available(),
                    Some(name) => registered_operations()
                        .ok()
                        .and_then(|ops| ops.iter().find(|op| op.mcp_tool == name))
                        .and_then(|op| crate::KnowledgeOperation::from_id(&op.operation_id))
                        .is_some_and(|op| executor.knowledge_available(op)),
                    None => false,
                };
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
                let result = checked_execute(self.profile.deadline_probe(), |probe| match name {
                    Some(MCP_TOOL) => Params::from_json(arguments)
                        .and_then(|request| executor.source_descend(request, probe)),
                    Some(SEARCH_MCP_TOOL) => IndexedSearchParams::from_json(arguments)
                        .and_then(|request| executor.knowledge_search_indexed(request, probe)),
                    Some(_) => crate::KnowledgeOperation::from_id(&operation.unwrap().operation_id)
                        .ok_or_else(|| {
                            crate::AccessError::new(
                                crate::AccessErrorCode::Unavailable,
                                "native operation unavailable",
                            )
                        })
                        .and_then(|op| crate::KnowledgeRequest::from_arguments(op, arguments))
                        .and_then(|request| executor.knowledge(request, probe)),
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

/// Each stdio line is one UTF-8 JSON-RPC message. Process diagnostics belong
/// on stderr; stdout carries protocol frames only.
pub fn run_stdio(executor: &dyn AccessExecutor, profile: AccessProfile) -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    run_io(stdin.lock(), stdout.lock(), executor, profile)
}

/// Same protocol path over supplied streams for real byte-level client tests.
pub fn run_io<R: BufRead, W: Write>(
    mut input: R,
    mut output: W,
    executor: &dyn AccessExecutor,
    profile: AccessProfile,
) -> io::Result<()> {
    let mut session = McpSession::new(profile);
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
        let response = if overflow {
            Some(rpc_error(b"null", -32700, "MCP frame exceeds byte budget"))
        } else {
            session.handle_line(executor, &line)
        };
        if let Some(mut frame) = response {
            let fence = session.pending_fence.take();
            let mut fence = fence;
            if let Some(current) = fence.as_mut() {
                if let Err(error) = current.recheck() {
                    frame = tool_error(
                        session.pending_id.as_deref().unwrap_or(b"null"),
                        error.message,
                    );
                    fence = None;
                }
            }
            frame.push(b'\n');
            output.write_all(&frame)?;
            output.flush()?;
            drop(fence);
        }
    }
}
