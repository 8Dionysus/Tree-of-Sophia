//! MCP 2025-11-25 stdio framing and lifecycle for registered native tools.

use std::io::{self, BufRead, Write};

use tos_foundation::{JsonMode, JsonNumberKind, JsonValue, parse_json};

use crate::common::{
    AccessExecutor, AccessProfile, DisclosureFence, Params, json_string, mcp_tool_list,
    registered_operation, validate_packet,
};

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
            "tools/list" => {
                if executor.source_descend_available() {
                    match mcp_tool_list() {
                        Ok(list) => Some(rpc_result(&id, &list)),
                        Err(_) => Some(rpc_error(
                            &id,
                            -32603,
                            "Native operation registry unavailable",
                        )),
                    }
                } else {
                    Some(rpc_result(&id, br#"{"tools":[]}"#))
                }
            }
            "tools/call" => {
                let params = value.object_get("params");
                let name = params
                    .and_then(|p| p.object_get("name"))
                    .and_then(JsonValue::as_str);
                let registered = registered_operation();
                if name != registered.ok().map(|operation| operation.mcp_tool.as_str())
                    || !executor.source_descend_available()
                {
                    return Some(rpc_error(&id, -32602, "Unknown tool"));
                }
                let Some(arguments) = params.and_then(|p| p.object_get("arguments")) else {
                    return Some(rpc_error(&id, -32602, "Tool arguments required"));
                };
                if let Some(fields) = arguments.as_object() {
                    if fields.iter().any(|(name, _)| {
                        !matches!(name.as_str(), Some("node_id" | "max_depth" | "limit"))
                    }) {
                        return Some(rpc_error(&id, -32602, "Unknown tool argument"));
                    }
                }
                let result = Params::from_json(arguments)
                    .and_then(|request| executor.source_descend(request))
                    .and_then(|packet| {
                        if packet.body.len() > self.profile.max_response_bytes {
                            return Err(crate::common::AccessError::new(
                                crate::common::AccessErrorCode::BudgetExceeded,
                                "source descent response budget exceeded",
                            ));
                        }
                        validate_packet(&packet.body, self.profile.max_response_bytes)?;
                        Ok(packet)
                    });
                match result {
                    Ok(packet) => {
                        let text = match std::str::from_utf8(&packet.body) {
                            Ok(text) => json_string(text),
                            Err(_) => {
                                return Some(rpc_error(&id, -32603, "Query packet is not UTF-8"));
                            }
                        };
                        let mut body = b"{\"content\":[{\"type\":\"text\",\"text\":".to_vec();
                        body.extend(text);
                        body.extend_from_slice(b"}],\"structuredContent\":");
                        body.extend(packet.body);
                        body.push(b'}');
                        let frame = rpc_result(&id, &body);
                        if frame.len() > self.profile.max_response_bytes {
                            return Some(rpc_error(
                                &id,
                                -32603,
                                "MCP response frame exceeds byte budget",
                            ));
                        }
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
    let mut out = b"{\"jsonrpc\":\"2.0\",\"id\":".to_vec();
    out.extend_from_slice(id);
    out.extend_from_slice(b",\"result\":");
    out.extend_from_slice(result);
    out.push(b'}');
    out
}

fn rpc_error(id: &[u8], code: i32, message: &str) -> Vec<u8> {
    let mut out = b"{\"jsonrpc\":\"2.0\",\"id\":".to_vec();
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
