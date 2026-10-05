//! One bounded local HTTP route for the first native query family.

use crate::{KnowledgeOperation, KnowledgeRequest, PreparedPacket};
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::{
    Arc,
    atomic::{AtomicU64, AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};
use tos_foundation::{JsonMode, JsonNumber, JsonNumberKind, JsonString, JsonValue, parse_json};
use tos_query::{AbortProbe, AbortReason};

use crate::common::{
    AccessError, AccessErrorCode, AccessExecutor, AccessProfile, DisclosureFence, HTTP_PREFIX,
    IndexedSearchParams, Params, SEARCH_HTTP_PATH, checked_execute, error_json, validate_packet,
};

const MAX_HEAD: usize = 8 * 1024;
const MAX_CONCURRENT: usize = 32;

// Retain the reservation in the closure itself: both unwinding a handler and
// dropping an unstarted closure after a spawn error release the connection.
struct ConnectionSlot(Arc<AtomicUsize>);
impl Drop for ConnectionSlot {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Poll the already-read request socket without blocking QRY's SQLite progress
/// callback on every VM instruction. The socket is restored to blocking mode
/// before the response write; this probe lives only through query execution.
struct HttpAbortProbe {
    client: TcpStream,
    deadline: Option<Instant>,
    checks: AtomicU64,
}

impl AbortProbe for HttpAbortProbe {
    fn reason(&self) -> Option<AbortReason> {
        if self
            .deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            return Some(AbortReason::DeadlineExceeded);
        }
        if self.checks.fetch_add(1, Ordering::Relaxed) % 1024 != 0 {
            return None;
        }
        match self.client.peek(&mut [0]) {
            // A peer may half-close its request stream while still reading the
            // response. EOF alone is not a cancelled HTTP request.
            Ok(0) => None,
            Ok(_) => None,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => None,
            Err(_) => Some(AbortReason::Cancelled),
        }
    }
}

pub type HttpResponse = ScopedHttpResponse<'static>;

/// A response cannot outlive the selected owner whose disclosure fence it holds.
pub struct ScopedHttpResponse<'hold> {
    pub status: u16,
    pub body: Vec<u8>,
    pub head_only: bool,
    fence: Option<Box<dyn DisclosureFence + 'hold>>,
    content_type: &'static str,
    csp_nonce: Option<String>,
}

impl ScopedHttpResponse<'_> {
    fn error(status: u16, message: &'static str) -> Self {
        let code = match status {
            404 => AccessErrorCode::UnknownExactId,
            413 => AccessErrorCode::BudgetExceeded,
            503 => AccessErrorCode::Unavailable,
            _ => AccessErrorCode::InvalidRequest,
        };
        let error = AccessError::new(code, message);
        Self {
            status,
            body: error_json(&error),
            head_only: false,
            content_type: "application/json; charset=utf-8",
            csp_nonce: None,
            fence: None,
        }
    }

    fn error_for_method(status: u16, message: &'static str, method: &str) -> Self {
        let mut response = Self::error(status, message);
        response.head_only = method == "HEAD";
        response
    }
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn percent_decode(raw: &str, plus_space: bool) -> Result<String, AccessError> {
    let mut bytes = Vec::with_capacity(raw.len());
    let source = raw.as_bytes();
    let mut at = 0;
    while at < source.len() {
        match source[at] {
            b'%' if at + 2 < source.len() => {
                let high = hex(source[at + 1]).ok_or_else(|| {
                    AccessError::new(AccessErrorCode::InvalidRequest, "invalid percent escape")
                })?;
                let low = hex(source[at + 2]).ok_or_else(|| {
                    AccessError::new(AccessErrorCode::InvalidRequest, "invalid percent escape")
                })?;
                bytes.push(high * 16 + low);
                at += 3;
            }
            b'%' => {
                return Err(AccessError::new(
                    AccessErrorCode::InvalidRequest,
                    "invalid percent escape",
                ));
            }
            b'+' if plus_space => {
                bytes.push(b' ');
                at += 1;
            }
            value => {
                bytes.push(value);
                at += 1;
            }
        }
    }
    String::from_utf8(bytes).map_err(|_| {
        AccessError::new(
            AccessErrorCode::InvalidRequest,
            "request target is not UTF-8",
        )
    })
}

fn bounded_legacy_int(raw: Option<&str>, default: i64, low: i64, high: i64) -> i64 {
    let parsed = raw
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(default);
    parsed.clamp(low, high)
}

fn query_value<'a>(query: &'a str, name: &str) -> Option<String> {
    query
        .split('&')
        .filter_map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            if percent_decode(key, true).ok()?.as_str() == name {
                percent_decode(value, true).ok()
            } else {
                None
            }
        })
        .next()
}

fn query_list(query: &str, name: &str) -> Vec<String> {
    query_value(query, name)
        .unwrap_or_default()
        .split(',')
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect()
}

fn indexed_cursor(query: &str) -> Result<Option<String>, AccessError> {
    let mut cursor = None;
    for pair in query.split('&') {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        if percent_decode(key, true)? == "cursor" {
            if cursor.is_some() {
                return Err(AccessError::new(
                    AccessErrorCode::InvalidRequest,
                    "indexed cursor appears more than once",
                ));
            }
            let value = percent_decode(value, true)?;
            if value.is_empty() {
                return Err(AccessError::new(
                    AccessErrorCode::InvalidRequest,
                    "indexed cursor is empty",
                ));
            }
            cursor = Some(value);
        }
    }
    Ok(cursor)
}

fn handle_search<'hold, E: crate::common::ScopedAccessExecutor<'hold> + ?Sized>(
    executor: &E,
    method: &str,
    query: &str,
    profile: AccessProfile,
    abort_probe: Arc<dyn AbortProbe>,
) -> ScopedHttpResponse<'hold> {
    let mode = query_value(query, "mode").unwrap_or_else(|| "legacy".into());
    if mode != "indexed" {
        use tos_foundation::JsonString;
        let string = |value: String| JsonValue::String(JsonString::from_utf8(&value));
        let list = |key| JsonValue::Array(query_list(query, key).into_iter().map(string).collect());
        let mut fields = vec![
            ("mode", string(mode.clone())),
            (
                "query",
                string(query_value(query, "query").unwrap_or_default()),
            ),
            ("sources", list("sources")),
            ("kind_ids", list("kind_ids")),
            ("predicate_ids", list("predicate_ids")),
            (
                "offset",
                JsonValue::Number(JsonNumber {
                    kind: JsonNumberKind::Int,
                    lexeme: bounded_legacy_int(
                        query_value(query, "offset").as_deref(),
                        0,
                        0,
                        100_000,
                    )
                    .to_string(),
                }),
            ),
            (
                "limit",
                JsonValue::Number(JsonNumber {
                    kind: JsonNumberKind::Int,
                    lexeme: bounded_legacy_int(query_value(query, "limit").as_deref(), 40, 1, 100)
                        .to_string(),
                }),
            ),
        ];
        if mode == "compressed" {
            // Reuse the strict cursor decoder: duplicates and malformed escapes
            // must refuse rather than silently start a new search.
            match indexed_cursor(query) {
                Ok(Some(cursor)) => fields.push(("cursor", string(cursor))),
                Ok(None) => {}
                Err(error) => return packet_response(Err(error), method, profile),
            }
            for key in ["offset", "limit"] {
                if let Some(raw) = query_value(query, key) {
                    let Ok(value) = raw.parse::<usize>() else {
                        return packet_response(
                            Err(AccessError::new(
                                AccessErrorCode::InvalidRequest,
                                "invalid compressed search integer",
                            )),
                            method,
                            profile,
                        );
                    };
                    fields.retain(|(name, _)| *name != key);
                    fields.push((
                        key,
                        JsonValue::Number(JsonNumber {
                            kind: JsonNumberKind::Int,
                            lexeme: value.to_string(),
                        }),
                    ));
                }
            }
        }
        let args = JsonValue::Object(
            fields
                .into_iter()
                .map(|(key, value)| (JsonString::from_utf8(key), value))
                .collect(),
        );
        let result = crate::search::SearchRequest::from_arguments(&args).and_then(|request| {
            checked_execute(abort_probe, |probe| request.execute(executor, probe))
        });
        return packet_response(result, method, profile);
    }
    if !executor.knowledge_search_indexed_available() {
        return ScopedHttpResponse::error_for_method(
            503,
            "indexed knowledge search unavailable",
            method,
        );
    }
    let offset = bounded_legacy_int(query_value(query, "offset").as_deref(), 0, 0, 100_000);
    if offset != 0 {
        return ScopedHttpResponse::error_for_method(
            400,
            "indexed search uses cursor, not offset",
            method,
        );
    }
    let limit = bounded_legacy_int(query_value(query, "limit").as_deref(), 40, 1, 100) as usize;
    let result = indexed_cursor(query)
        .and_then(|cursor| {
            IndexedSearchParams::new(
                query_value(query, "query").unwrap_or_default(),
                query_list(query, "sources"),
                query_list(query, "kind_ids"),
                query_list(query, "predicate_ids"),
                cursor,
                limit,
            )
        })
        .and_then(|params| {
            checked_execute(abort_probe, |probe| {
                executor.knowledge_search_indexed(params, probe)
            })
        })
        .and_then(|packet| {
            if packet.body.len() > profile.max_response_bytes {
                return Err(AccessError::new(
                    AccessErrorCode::BudgetExceeded,
                    "indexed search response budget exceeded",
                ));
            }
            validate_packet(&packet.body, profile.max_response_bytes)?;
            Ok(packet)
        });
    match result {
        Ok(packet) => ScopedHttpResponse {
            status: 200,
            body: packet.body,
            head_only: method == "HEAD",
            content_type: "application/json; charset=utf-8",
            csp_nonce: None,
            fence: Some(packet.fence),
        },
        Err(error) => ScopedHttpResponse {
            status: error.http_status(),
            body: error_json(&error),
            head_only: method == "HEAD",
            content_type: "application/json; charset=utf-8",
            csp_nonce: None,
            fence: None,
        },
    }
}

/// Parse and prepare using the existing HTTP route law inside a held owner.
pub fn handle_get_scoped<'hold, E: crate::common::ScopedAccessExecutor<'hold> + ?Sized>(
    executor: &E,
    method: &str,
    target: &str,
    profile: AccessProfile,
    probe: Arc<dyn AbortProbe>,
) -> ScopedHttpResponse<'hold> {
    handle_get_with_probe(executor, method, target, profile, probe, None)
}

/// Structured read route; no authored write or new scope is introduced.
pub fn handle_post_scoped<'hold, E: crate::common::ScopedAccessExecutor<'hold> + ?Sized>(
    executor: &E,
    target: &str,
    body: &[u8],
    profile: AccessProfile,
    probe: Arc<dyn AbortProbe>,
) -> ScopedHttpResponse<'hold> {
    handle_post_with_probe(executor, target, body, profile, probe)
}

pub fn handle_get(
    executor: &dyn AccessExecutor,
    method: &str,
    target: &str,
    profile: AccessProfile,
) -> HttpResponse {
    handle_get_with_probe(
        executor,
        method,
        target,
        profile,
        profile.deadline_probe(),
        None,
    )
}

/// Explicit installed software companion; data roots never select this handle.
pub fn handle_get_with_software(
    executor: &dyn AccessExecutor,
    method: &str,
    target: &str,
    profile: AccessProfile,
    site: &Arc<crate::site::SoftwareSite>,
) -> HttpResponse {
    handle_get_with_probe(
        executor,
        method,
        target,
        profile,
        profile.deadline_probe(),
        Some(site),
    )
}

fn handle_get_with_probe<'hold, E: crate::common::ScopedAccessExecutor<'hold> + ?Sized>(
    executor: &E,
    method: &str,
    target: &str,
    profile: AccessProfile,
    abort_probe: Arc<dyn AbortProbe>,
    site: Option<&Arc<crate::site::SoftwareSite>>,
) -> ScopedHttpResponse<'hold> {
    if method != "GET" && method != "HEAD" {
        return ScopedHttpResponse::error(405, "method not allowed");
    }
    if target.len() > profile.max_request_bytes {
        return ScopedHttpResponse::error_for_method(413, "request target too large", method);
    }
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if path == "/health" {
        let mut health_ok = false;
        let result = checked_execute(abort_probe, |probe| {
            let report = executor.access_health_report(probe)?;
            health_ok = report.ok;
            Ok(report.packet)
        });
        let mut response = packet_response(result, method, profile);
        if response.status == 200 && !health_ok {
            response.status = 503;
        }
        return response;
    }
    if let Some(operation) = crate::source_read::Operation::http(method, path) {
        return packet_response(
            checked_execute(abort_probe, |probe| {
                let request = crate::source_read::Request::from_bytes(operation, b"{}", profile)?;
                executor.source_read(request, probe)
            }),
            method,
            profile,
        );
    }
    if path == "/api/zarathustra/word-analysis" && executor.word_analysis_available() {
        use tos_foundation::JsonString;
        let result = checked_execute(abort_probe, |probe| {
            // Maintained parse_qs drops empty values before _single selects.
            let value = |name| {
                query
                    .split('&')
                    .find_map(|pair| query_value(pair, name).filter(|text| !text.is_empty()))
            };
            let string = |text: String| JsonValue::String(JsonString::from_utf8(&text));
            let raw = value("include_semantic_neighbors").unwrap_or_else(|| "false".into());
            let stripped =
                tos_foundation::python_strip_unicode16_v1(&raw, profile.max_request_bytes)
                    .map_err(|_| {
                        AccessError::new(
                            AccessErrorCode::InvalidRequest,
                            "word-analysis boolean exceeds request budget",
                        )
                    })?;
            let semantic = match stripped.to_ascii_lowercase().as_str() {
                "1" | "true" | "yes" | "on" => true,
                "0" | "false" | "no" | "off" => false,
                _ => {
                    return Err(AccessError::new(
                        AccessErrorCode::InvalidRequest,
                        "word-analysis boolean invalid",
                    ));
                }
            };
            let rank =
                crate::word_analysis::http_rank(&value("rank").unwrap_or_else(|| "1".into()));
            let args = JsonValue::Object(vec![
                (
                    JsonString::from_utf8("query"),
                    string(value("query").unwrap_or_default()),
                ),
                (
                    JsonString::from_utf8("language"),
                    string(value("language").unwrap_or_else(|| "ru".into())),
                ),
                (
                    JsonString::from_utf8("rank"),
                    JsonValue::Number(JsonNumber {
                        kind: JsonNumberKind::Int,
                        lexeme: rank.to_string(),
                    }),
                ),
                (
                    JsonString::from_utf8("include_semantic_neighbors"),
                    JsonValue::Bool(semantic),
                ),
            ]);
            crate::word_analysis::prepare_capability(executor, &args, profile, probe)
        });
        return packet_response(result, method, profile);
    }
    if path == crate::reading::HTTP_PATH {
        use tos_foundation::JsonString;
        let string = |s: &str| JsonValue::String(JsonString::from_utf8(s));
        let boolean =
            query_value(query, "include_semantic_neighbors").unwrap_or_else(|| "false".into());
        let boolean = tos_foundation::python_strip_unicode16_v1(&boolean, boolean.chars().count())
            .expect("typed UTF-8 input fits its exact scalar-count strip budget");
        let boolean = match boolean.to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => true,
            "0" | "false" | "no" | "off" => false,
            _ => {
                return packet_response(
                    Err(AccessError::new(
                        AccessErrorCode::InvalidRequest,
                        "reading boolean invalid",
                    )),
                    method,
                    profile,
                );
            }
        };
        let mut fields = vec![
            (
                "query",
                string(&query_value(query, "query").unwrap_or_default()),
            ),
            (
                "language",
                string(&query_value(query, "language").unwrap_or_else(|| "ru".into())),
            ),
            (
                "limit",
                JsonValue::Number(JsonNumber {
                    kind: JsonNumberKind::Int,
                    lexeme: bounded_legacy_int(query_value(query, "limit").as_deref(), 20, 0, 100)
                        .to_string(),
                }),
            ),
            ("include_semantic_neighbors", JsonValue::Bool(boolean)),
        ];
        if let Some(groups) = query_value(query, "group_by") {
            let groups = if groups == "none" {
                Vec::new()
            } else {
                groups
                    .split(',')
                    .filter(|g| !g.is_empty())
                    .map(string)
                    .collect()
            };
            fields.push(("group_by", JsonValue::Array(groups)));
        }
        let args = JsonValue::Object(
            fields
                .into_iter()
                .map(|(k, v)| (JsonString::from_utf8(k), v))
                .collect(),
        );
        return packet_response(
            crate::reading::from_arguments(&args).and_then(|request| {
                checked_execute(abort_probe, |probe| executor.reading_search(request, probe))
            }),
            method,
            profile,
        );
    }
    if path == "/" || path.starts_with("/static/") {
        let Some(site) = site else {
            return ScopedHttpResponse::error_for_method(
                503,
                "installed software site unavailable",
                method,
            );
        };
        let result = if path == "/" {
            site.shell(executor, profile, abort_probe)
                .map(|(packet, nonce)| (packet, "text/html; charset=utf-8", Some(nonce)))
        } else {
            percent_decode(&path[8..], false).and_then(|relative| {
                site.asset(&relative, abort_probe)
                    .map(|packet| (packet, crate::site::mime(&relative), None))
            })
        };
        return match result {
            Ok((packet, content_type, csp_nonce)) => ScopedHttpResponse {
                status: 200,
                body: packet.body,
                head_only: method == "HEAD",
                fence: Some(packet.fence),
                content_type,
                csp_nonce,
            },
            Err(error) => packet_response(Err(error), method, profile),
        };
    }
    if path == "/api/zarathustra/word-analysis" {
        let packet = crate::reading::public_word_analysis_capability(profile.max_response_bytes);
        return match packet {
            Ok(body) => ScopedHttpResponse {
                status: 200,
                body,
                head_only: method == "HEAD",
                fence: None,
                content_type: "application/json; charset=utf-8",
                csp_nonce: None,
            },
            Err(error) => packet_response(Err(error), method, profile),
        };
    }
    for (prefix, mode) in [
        ("/api/philosophy/query/epistemic/", "philosophy"),
        ("/api/corpus/query/epistemic/", "corpus"),
    ] {
        if let Some(encoded) = path.strip_prefix(prefix) {
            let request = percent_decode(encoded, false).map(|item_id| {
                KnowledgeRequest::EvidenceLens(tos_query::philosophy_read::EvidenceRequest {
                    mode: if mode == "philosophy" {
                        tos_query::philosophy_read::EvidenceMode::Philosophy
                    } else {
                        tos_query::philosophy_read::EvidenceMode::Corpus
                    },
                    item_id,
                    view_id: query_value(query, "view_id").filter(|v| !v.is_empty()),
                    limit: bounded_legacy_int(query_value(query, "limit").as_deref(), 80, 1, 200)
                        as usize,
                })
            });
            return knowledge_response(executor, request, method, profile, abort_probe);
        }
    }
    if path == "/api/source-gaps" {
        let request = tos_query::source_gap::SourceGapRequest {
            query: query_value(query, "query").unwrap_or_default(),
            limit: bounded_legacy_int(query_value(query, "limit").as_deref(), 20, 1, 100) as usize,
        };
        return packet_response(
            checked_execute(abort_probe, |probe| executor.source_gap(request, probe)),
            method,
            profile,
        );
    }
    if path == "/api/knowledge/explore/capabilities" {
        return packet_response(
            checked_execute(abort_probe, |_| {
                crate::exploration_contracts::execute_capabilities(
                    executor,
                    profile.max_response_bytes,
                )
            }),
            method,
            profile,
        );
    }
    if path == SEARCH_HTTP_PATH {
        return handle_search(executor, method, query, profile, abort_probe);
    }
    if let Ok(operations) = crate::common::registered_operations() {
        for operation in operations.iter().filter(|op| op.http_method == "GET") {
            let Some(op) = KnowledgeOperation::from_id(&operation.operation_id) else {
                continue;
            };
            let prefix = operation
                .http_path
                .split_once('{')
                .map(|(prefix, _)| prefix);
            let encoded = match prefix {
                Some(prefix) => path.strip_prefix(prefix),
                None if path == operation.http_path => Some(""),
                _ => None,
            };
            let Some(encoded) = encoded else {
                continue;
            };
            let request = match op {
                operation if operation.is_corpus() => {
                    corpus_http_request(operation, encoded, query)
                }
                operation if operation.is_philosophy() => {
                    philosophy_http_request(operation, encoded, query)
                }
                KnowledgeOperation::Catalog => Ok(KnowledgeRequest::Catalog),
                KnowledgeOperation::ExplorationContracts => {
                    Ok(KnowledgeRequest::ExplorationContracts)
                }
                KnowledgeOperation::SearchCapabilities => Ok(KnowledgeRequest::SearchCapabilities),
                KnowledgeOperation::Contracts => Ok(KnowledgeRequest::Contracts),
                KnowledgeOperation::Dossier => {
                    percent_decode(encoded, false).map(|object_id| KnowledgeRequest::Dossier {
                        object_id,
                        limit: bounded_legacy_int(
                            query_value(query, "limit").as_deref(),
                            300,
                            1,
                            300,
                        ) as usize,
                    })
                }
                KnowledgeOperation::StoredLens => percent_decode(encoded, false)
                    .map(|lens_id| KnowledgeRequest::StoredLens { lens_id }),
                KnowledgeOperation::Focus => focus_http_request(encoded, query),
                KnowledgeOperation::Node => {
                    percent_decode(encoded, false).map(|node_id| KnowledgeRequest::Node {
                        node_id,
                        relation_limit: bounded_legacy_int(
                            query_value(query, "relation_limit").as_deref(),
                            200,
                            0,
                            1000,
                        ) as usize,
                    })
                }
                KnowledgeOperation::Relation => percent_decode(encoded, false)
                    .map(|relation_id| KnowledgeRequest::Relation { relation_id }),
                _ => continue,
            };
            let response =
                knowledge_response(executor, request, method, profile, Arc::clone(&abort_probe));
            if op == KnowledgeOperation::PhilosophyScaleRows {
                return scale_export_response(response, encoded, method, profile, &abort_probe);
            }
            return response;
        }
    }
    let Some(encoded_id) = path.strip_prefix(HTTP_PREFIX) else {
        return ScopedHttpResponse::error_for_method(404, "not found", method);
    };
    if !executor.source_descend_available() {
        return ScopedHttpResponse::error_for_method(503, "source descent unavailable", method);
    }
    let outcome = percent_decode(encoded_id, false)
        .and_then(|node_id| {
            let max_depth =
                bounded_legacy_int(query_value(query, "max_depth").as_deref(), 8, 1, 8) as u8;
            let limit =
                bounded_legacy_int(query_value(query, "limit").as_deref(), 300, 1, 300) as usize;
            Params::new(node_id, max_depth, limit)
        })
        .and_then(|params| {
            checked_execute(abort_probe, |probe| executor.source_descend(params, probe))
        })
        .and_then(|packet| {
            if packet.body.len() > profile.max_response_bytes {
                return Err(AccessError::new(
                    AccessErrorCode::BudgetExceeded,
                    "source descent response budget exceeded",
                ));
            }
            validate_packet(&packet.body, profile.max_response_bytes)?;
            Ok(packet)
        });
    match outcome {
        Ok(packet) => ScopedHttpResponse {
            status: 200,
            body: packet.body,
            head_only: method == "HEAD",
            content_type: "application/json; charset=utf-8",
            csp_nonce: None,
            fence: Some(packet.fence),
        },
        Err(error) => ScopedHttpResponse {
            status: error.http_status(),
            body: error_json(&error),
            head_only: method == "HEAD",
            content_type: "application/json; charset=utf-8",
            csp_nonce: None,
            fence: None,
        },
    }
}

fn packet_response<'hold>(
    result: Result<PreparedPacket<'hold>, AccessError>,
    method: &str,
    profile: AccessProfile,
) -> ScopedHttpResponse<'hold> {
    let result = result.and_then(|packet| {
        if packet.body.len() > profile.max_response_bytes {
            return Err(AccessError::new(
                AccessErrorCode::BudgetExceeded,
                "query response budget exceeded",
            ));
        }
        validate_packet(&packet.body, profile.max_response_bytes)?;
        Ok(packet)
    });
    match result {
        Ok(packet) => ScopedHttpResponse {
            status: 200,
            body: packet.body,
            head_only: method == "HEAD",
            content_type: "application/json; charset=utf-8",
            csp_nonce: None,
            fence: Some(packet.fence),
        },
        Err(error) => ScopedHttpResponse {
            status: error.http_status(),
            body: error_json(&error),
            head_only: method == "HEAD",
            content_type: "application/json; charset=utf-8",
            csp_nonce: None,
            fence: None,
        },
    }
}
fn knowledge_response<'hold, E: crate::common::ScopedAccessExecutor<'hold> + ?Sized>(
    executor: &E,
    request: Result<KnowledgeRequest, AccessError>,
    method: &str,
    profile: AccessProfile,
    probe: Arc<dyn AbortProbe>,
) -> ScopedHttpResponse<'hold> {
    packet_response(
        request.and_then(|request| {
            if matches!(request, KnowledgeRequest::ExplorationContracts) {
                return checked_execute(probe, |_| {
                    crate::exploration_contracts::execute(executor, profile.max_response_bytes)
                });
            }
            if !executor.knowledge_available(request.operation()) {
                return Err(AccessError::new(
                    AccessErrorCode::Unavailable,
                    "selected knowledge operation unavailable",
                ));
            }
            checked_execute(probe, |probe| executor.knowledge(request, probe))
        }),
        method,
        profile,
    )
}
/// POST transports a bounded structured read, never an authored write.
pub fn handle_post(
    executor: &dyn AccessExecutor,
    target: &str,
    body: &[u8],
    profile: AccessProfile,
) -> HttpResponse {
    handle_post_with_probe(executor, target, body, profile, profile.deadline_probe())
}
fn handle_post_with_probe<'hold, E: crate::common::ScopedAccessExecutor<'hold> + ?Sized>(
    executor: &E,
    target: &str,
    body: &[u8],
    profile: AccessProfile,
    probe: Arc<dyn AbortProbe>,
) -> ScopedHttpResponse<'hold> {
    if target.len() > profile.max_request_bytes || body.len() > profile.max_request_bytes {
        return ScopedHttpResponse::error(413, "query request byte cap exceeded");
    }
    if let Some(operation) = crate::source_read::Operation::http("POST", target) {
        return packet_response(
            checked_execute(probe, |probe| {
                let parsed = parse_json(body, JsonMode::RequestLastWins, profile.json_limits())
                    .map_err(|_| {
                        AccessError::new(
                            AccessErrorCode::InvalidRequest,
                            "invalid exact-source request JSON",
                        )
                    })?;
                let request =
                    crate::source_read::Request::from_arguments(operation, parsed.root(), profile)?;
                executor.source_read(request, probe)
            }),
            "POST",
            profile,
        );
    }
    let operation = post_operation(target);
    let Some(operation) = operation else {
        return ScopedHttpResponse::error(404, "not found");
    };
    let request = parse_json(body, JsonMode::RequestLastWins, profile.json_limits())
        .map_err(|_| {
            AccessError::new(
                AccessErrorCode::InvalidRequest,
                "invalid bounded query JSON",
            )
        })
        .and_then(|document| KnowledgeRequest::from_body(operation, document.into_root()));
    knowledge_response(executor, request, "POST", profile, probe)
}
fn post_operation(target: &str) -> Option<KnowledgeOperation> {
    let path = target.split_once('?').map_or(target, |(path, _)| path);
    crate::common::registered_operations()
        .ok()?
        .iter()
        .find(|op| op.http_method == "POST" && op.http_path == path)
        .and_then(|op| KnowledgeOperation::from_id(&op.operation_id))
}
pub(crate) fn post_body(
    stream: &mut TcpStream,
    text: &str,
    profile: AccessProfile,
) -> Result<Vec<u8>, AccessError> {
    let control = HttpConnectionControl::compatibility(profile);
    post_body_controlled(stream, text, profile, &control)
}

fn post_body_controlled(
    stream: &mut TcpStream,
    text: &str,
    profile: AccessProfile,
    control: &HttpConnectionControl,
) -> Result<Vec<u8>, AccessError> {
    let mut length = None;
    let mut content_type = None;
    for line in text.split("\r\n").skip(1).filter(|line| !line.is_empty()) {
        let (name, value) = line.split_once(':').ok_or_else(|| {
            AccessError::new(AccessErrorCode::InvalidRequest, "invalid HTTP header")
        })?;
        match name.to_ascii_lowercase().as_str() {
            "transfer-encoding" => {
                return Err(AccessError::new(
                    AccessErrorCode::InvalidRequest,
                    "transfer encoding unsupported",
                ));
            }
            "content-length" => {
                if length.is_some() {
                    return Err(AccessError::new(
                        AccessErrorCode::InvalidRequest,
                        "duplicate Content-Length",
                    ));
                }
                let value = value.trim();
                if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
                    return Err(AccessError::new(
                        AccessErrorCode::InvalidRequest,
                        "invalid Content-Length",
                    ));
                }
                length = Some(value.parse::<usize>().map_err(|_| {
                    AccessError::new(
                        AccessErrorCode::BudgetExceeded,
                        "query request byte cap exceeded",
                    )
                })?);
            }
            "content-type" => {
                if content_type.is_some() {
                    return Err(AccessError::new(
                        AccessErrorCode::InvalidRequest,
                        "duplicate Content-Type",
                    ));
                }
                content_type = Some(value.trim());
            }
            _ => {}
        }
    }
    if !content_type.is_some_and(|v| {
        v.split(';')
            .next()
            .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"))
    }) {
        return Err(AccessError::new(
            AccessErrorCode::UnsupportedMediaType,
            "query requires application/json",
        ));
    }
    let length = length.filter(|n| *n > 0).ok_or_else(|| {
        AccessError::new(
            AccessErrorCode::InvalidRequest,
            "query requires positive Content-Length",
        )
    })?;
    if length > profile.max_request_bytes {
        return Err(AccessError::new(
            AccessErrorCode::BudgetExceeded,
            "query request byte cap exceeded",
        ));
    }
    let mut bytes = vec![0; length];
    let mut at = 0;
    while at < length {
        let n = control.read(stream, &mut bytes[at..]).map_err(|error| {
            AccessError::new(
                if matches!(error.kind(), std::io::ErrorKind::TimedOut) {
                    AccessErrorCode::DeadlineExceeded
                } else if error.kind() == std::io::ErrorKind::Interrupted {
                    AccessErrorCode::Cancelled
                } else {
                    AccessErrorCode::InvalidRequest
                },
                "query body read failed",
            )
        })?;
        if n == 0 {
            return Err(AccessError::new(
                AccessErrorCode::InvalidRequest,
                "incomplete query body",
            ));
        }
        at += n;
    }
    Ok(bytes)
}

pub(crate) fn read_head(stream: &mut TcpStream) -> std::io::Result<Vec<u8>> {
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    let deadline = Instant::now() + Duration::from_secs(5);
    while head.len() <= MAX_HEAD {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(std::io::ErrorKind::TimedOut.into());
        }
        stream.set_read_timeout(Some(remaining))?;
        if stream.read(&mut byte)? == 0 {
            break;
        }
        head.push(byte[0]);
        if head.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    Ok(head)
}

/// One absolute owner window shared by parsing, query, socket writes and flush.
/// This control limits transport; it supplies no source or disclosure authority.
#[derive(Clone)]
pub struct HttpConnectionControl {
    deadline: Instant,
    probe: Arc<dyn AbortProbe>,
}
impl HttpConnectionControl {
    pub fn new(deadline: Instant, probe: Arc<dyn AbortProbe>) -> Self {
        Self { deadline, probe }
    }
    fn compatibility(profile: AccessProfile) -> Self {
        let now = Instant::now();
        Self::new(
            now.checked_add(profile.query_timeout.unwrap_or(Duration::from_secs(5)))
                .unwrap_or(now),
            profile.deadline_probe(),
        )
    }
    pub fn check(&self) -> std::io::Result<()> {
        match self.probe.reason() {
            Some(AbortReason::Cancelled) => return Err(std::io::ErrorKind::Interrupted.into()),
            Some(AbortReason::DeadlineExceeded) => return Err(std::io::ErrorKind::TimedOut.into()),
            None => {}
        }
        if Instant::now() >= self.deadline {
            return Err(std::io::ErrorKind::TimedOut.into());
        }
        Ok(())
    }
    fn timeout(&self) -> std::io::Result<Duration> {
        self.check()?;
        Ok(self
            .deadline
            .saturating_duration_since(Instant::now())
            .min(Duration::from_millis(50)))
    }
    fn read(&self, stream: &mut TcpStream, bytes: &mut [u8]) -> std::io::Result<usize> {
        loop {
            stream.set_read_timeout(Some(self.timeout()?))?;
            match stream.read(bytes) {
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::TimedOut
                            | std::io::ErrorKind::WouldBlock
                            | std::io::ErrorKind::Interrupted
                    ) =>
                {
                    continue;
                }
                result => {
                    self.check()?;
                    return result;
                }
            }
        }
    }
}
impl AbortProbe for HttpConnectionControl {
    fn reason(&self) -> Option<AbortReason> {
        self.probe
            .reason()
            .or_else(|| (Instant::now() >= self.deadline).then_some(AbortReason::DeadlineExceeded))
    }
}
struct ControlledWriter<'a> {
    stream: &'a mut TcpStream,
    control: &'a HttpConnectionControl,
}
impl Write for ControlledWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        loop {
            self.stream
                .set_write_timeout(Some(self.control.timeout()?))?;
            match self.stream.write(bytes) {
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::TimedOut
                            | std::io::ErrorKind::WouldBlock
                            | std::io::ErrorKind::Interrupted
                    ) =>
                {
                    continue;
                }
                result => {
                    self.control.check()?;
                    return result;
                }
            }
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.control.check()?;
        self.stream.flush()?;
        self.control.check()
    }
}
fn read_head_controlled(
    stream: &mut TcpStream,
    control: &HttpConnectionControl,
) -> std::io::Result<Vec<u8>> {
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while head.len() <= MAX_HEAD {
        if control.read(stream, &mut byte)? == 0 {
            break;
        }
        head.push(byte[0]);
        if head.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    Ok(head)
}

/// Write the prepared response through the same final current/cancellation fence
/// used by socket delivery, retaining its disclosure hold through final flush.
pub fn write_response<'hold, W: Write>(
    stream: &mut W,
    mut response: ScopedHttpResponse<'hold>,
) -> std::io::Result<()> {
    if let Some(fence) = response.fence.as_mut() {
        if let Err(error) = fence.recheck() {
            response.status = error.http_status();
            response.body = error_json(&error);
            response.fence = None;
            response.content_type = "application/json; charset=utf-8";
            response.csp_nonce = None;
        }
    }
    let reason = match response.status {
        200 => "OK",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        409 => "Conflict",
        410 => "Gone",
        413 => "Content Too Large",
        415 => "Unsupported Media Type",
        503 => "Service Unavailable",
        _ => "Error",
    };
    let script_nonce = response
        .csp_nonce
        .as_ref()
        .map(|nonce| format!(" 'nonce-{nonce}'"))
        .unwrap_or_default();
    let cache = if response.content_type.starts_with("application/json") {
        "no-store"
    } else {
        "no-cache"
    };
    let header = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: {}\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\nContent-Security-Policy: default-src 'self'; base-uri 'none'; connect-src 'self'; font-src 'self'; form-action 'self'; frame-ancestors 'none'; img-src 'self' data:; object-src 'none'; script-src 'self'{} 'wasm-unsafe-eval'; style-src 'self'; worker-src 'self'\r\nPermissions-Policy: tools=(self), accelerometer=(), camera=(), geolocation=(), gyroscope=(), microphone=(), payment=(), usb=()\r\nCross-Origin-Opener-Policy: same-origin\r\nCross-Origin-Embedder-Policy: require-corp\r\nCross-Origin-Resource-Policy: same-origin\r\nOrigin-Agent-Cluster: ?1\r\nReferrer-Policy: no-referrer\r\nX-Frame-Options: DENY\r\n\r\n",
        response.status,
        reason,
        response.content_type,
        response.body.len(),
        cache,
        script_nonce
    );
    stream.write_all(header.as_bytes())?;
    if !response.head_only {
        stream.write_all(&response.body)?;
    }
    stream.flush()?;
    // Bytes already delivered cannot be rolled back. A failed final fence is
    // reported to the caller while the genuine owner lease is still retained.
    if let Some(fence) = response.fence.as_mut() {
        fence.recheck().map_err(std::io::Error::other)?;
    }
    Ok(())
}

/// Serve exactly one bounded HTTP/1.x request on an already accepted socket.
/// The listener owns admission and concurrency; this entry is useful for
/// actual socket-level conformance tests without a background server.
pub fn serve_connection(
    stream: TcpStream,
    executor: Arc<dyn AccessExecutor>,
    profile: AccessProfile,
) {
    serve_connection_with_software(stream, executor, profile, None)
}
/// Serve one request while borrowing a callback-local executor through the
/// complete response flush and its final disclosure-fence check. Call this
/// inside the owner callback that created the executor; the socket route does
/// not clone, store, or extend that selected source hold.
pub fn serve_connection_scoped<'hold, E: crate::common::ScopedAccessExecutor<'hold> + ?Sized>(
    stream: TcpStream,
    executor: &E,
    profile: AccessProfile,
) -> std::io::Result<()> {
    serve_connection_scoped_with_software(stream, executor, profile, None)
}

/// Same borrowed transport with the explicitly installed software companion.
/// The site handle is selected by its owner and remains borrowed through flush.
pub fn serve_connection_scoped_with_site<
    'hold,
    E: crate::common::ScopedAccessExecutor<'hold> + ?Sized,
>(
    stream: TcpStream,
    executor: &E,
    site: &Arc<crate::site::SoftwareSite>,
    profile: AccessProfile,
) -> std::io::Result<()> {
    serve_connection_scoped_with_software(stream, executor, profile, Some(site))
}

fn serve_connection_with_software(
    stream: TcpStream,
    executor: Arc<dyn AccessExecutor>,
    profile: AccessProfile,
    site: Option<Arc<crate::site::SoftwareSite>>,
) {
    let _ =
        serve_connection_scoped_with_software(stream, executor.as_ref(), profile, site.as_ref());
}

fn serve_connection_scoped_with_software<
    'hold,
    E: crate::common::ScopedAccessExecutor<'hold> + ?Sized,
>(
    mut stream: TcpStream,
    executor: &E,
    profile: AccessProfile,
    site: Option<&Arc<crate::site::SoftwareSite>>,
) -> std::io::Result<()> {
    let control = HttpConnectionControl::compatibility(profile);
    serve_connection_scoped_controlled(stream, executor, site, profile, &control)
}

pub fn serve_connection_scoped_controlled<
    'hold,
    E: crate::common::ScopedAccessExecutor<'hold> + ?Sized,
>(
    mut stream: TcpStream,
    executor: &E,
    site: Option<&Arc<crate::site::SoftwareSite>>,
    profile: AccessProfile,
    control: &HttpConnectionControl,
) -> std::io::Result<()> {
    control.check()?;
    let response = match read_head_controlled(&mut stream, control) {
        Ok(head) if head.len() <= MAX_HEAD && head.ends_with(b"\r\n\r\n") => {
            match std::str::from_utf8(&head) {
                Ok(text) => {
                    let first = text.split("\r\n").next().unwrap_or("");
                    match first.split_whitespace().collect::<Vec<_>>().as_slice() {
                        [method, target, "HTTP/1.1"] | [method, target, "HTTP/1.0"] => {
                            let body = if *method == "POST" {
                                if target.len() > profile.max_request_bytes {
                                    Err(AccessError::new(
                                        AccessErrorCode::BudgetExceeded,
                                        "request target too large",
                                    ))
                                } else if post_operation(target).is_none()
                                    && crate::source_read::Operation::http("POST", target).is_none()
                                {
                                    Err(AccessError::new(
                                        AccessErrorCode::UnknownExactId,
                                        "not found",
                                    ))
                                } else {
                                    post_body_controlled(&mut stream, text, profile, control)
                                }
                            } else {
                                Ok(Vec::new())
                            };
                            match body {
                                Err(error) => packet_response(Err(error), method, profile),
                                Ok(body) => match stream.try_clone().and_then(|client| {
                                    client.set_nonblocking(true)?;
                                    Ok(client)
                                }) {
                                    Ok(client) => {
                                        struct CombinedProbe {
                                            owner: HttpConnectionControl,
                                            socket: HttpAbortProbe,
                                        }
                                        impl AbortProbe for CombinedProbe {
                                            fn reason(&self) -> Option<AbortReason> {
                                                self.owner.reason().or_else(|| self.socket.reason())
                                            }
                                        }
                                        let probe = Arc::new(CombinedProbe {
                                            owner: control.clone(),
                                            socket: HttpAbortProbe {
                                                client,
                                                deadline: Some(control.deadline),
                                                checks: AtomicU64::new(0),
                                            },
                                        });
                                        if *method == "POST" {
                                            handle_post_with_probe(
                                                executor, target, &body, profile, probe,
                                            )
                                        } else {
                                            handle_get_with_probe(
                                                executor, method, target, profile, probe, site,
                                            )
                                        }
                                    }
                                    Err(_) => ScopedHttpResponse::error(
                                        503,
                                        "client cancellation probe unavailable",
                                    ),
                                },
                            }
                        }
                        _ => ScopedHttpResponse::error(400, "invalid HTTP request line"),
                    }
                }
                Err(_) => ScopedHttpResponse::error(400, "HTTP header is not UTF-8"),
            }
        }
        _ => ScopedHttpResponse::error(400, "HTTP header incomplete or oversized"),
    };
    stream.set_nonblocking(false)?;
    let rejected = response.status >= 400;
    write_response(
        &mut ControlledWriter {
            stream: &mut stream,
            control,
        },
        response,
    )?;
    if rejected {
        // Publish the complete refusal before draining bounded unread
        // request bytes. Dropping a socket with queued input sends RST on
        // Linux and can erase an otherwise complete client error response.
        let _ = stream.shutdown(Shutdown::Write);
        drain_rejected_request_controlled(&mut stream, profile.max_request_bytes, control);
    }
    control.check()
}

fn drain_rejected_request_controlled(
    stream: &mut TcpStream,
    max_bytes: usize,
    control: &HttpConnectionControl,
) {
    let deadline = control
        .deadline
        .min(Instant::now() + Duration::from_millis(200));
    let mut remaining = max_bytes;
    let mut buffer = [0u8; 1024];
    while remaining > 0 {
        let time = deadline.saturating_duration_since(Instant::now());
        if time.is_zero()
            || control.check().is_err()
            || stream
                .set_read_timeout(Some(time.min(Duration::from_millis(50))))
                .is_err()
        {
            break;
        }
        let size = remaining.min(buffer.len());
        match stream.read(&mut buffer[..size]) {
            Ok(0) => break,
            Ok(n) => remaining -= n,
            Err(_) => break,
        }
    }
}

/// Bounded, serial listener for an owner which must open and close each real
/// selected callback around socket delivery. `finished` signals normal owner
/// completion; cancellation and deadline remain failure conditions. No executor is cloned or stored;
/// the caller owns current authority, capture custody and per-query admission.
/// The installed software handle remains an independent, admitted code input.
#[cfg(not(target_arch = "wasm32"))]
pub fn serve_selected_connections(
    addr: &str,
    profile: AccessProfile,
    deadline: Instant,
    cancelled: &Arc<std::sync::atomic::AtomicBool>,
    finished: &std::sync::atomic::AtomicBool,
    mut consume: impl FnMut(
        TcpStream,
        Option<&Arc<crate::site::SoftwareSite>>,
        AccessProfile,
        &HttpConnectionControl,
    ) -> std::io::Result<()>,
) -> std::io::Result<()> {
    struct ListenerAbort {
        deadline: Instant,
        cancelled: Arc<std::sync::atomic::AtomicBool>,
    }
    impl AbortProbe for ListenerAbort {
        fn reason(&self) -> Option<AbortReason> {
            if self.cancelled.load(Ordering::Acquire) {
                Some(AbortReason::Cancelled)
            } else if Instant::now() >= self.deadline {
                Some(AbortReason::DeadlineExceeded)
            } else {
                None
            }
        }
    }
    fn check(probe: &dyn AbortProbe) -> std::io::Result<()> {
        match probe.reason() {
            Some(AbortReason::DeadlineExceeded) => Err(std::io::ErrorKind::TimedOut.into()),
            Some(AbortReason::Cancelled) => Err(std::io::ErrorKind::Interrupted.into()),
            None => Ok(()),
        }
    }
    let probe: Arc<dyn AbortProbe> = Arc::new(ListenerAbort {
        deadline,
        cancelled: Arc::clone(cancelled),
    });
    check(probe.as_ref())?;
    let now = Instant::now();
    let startup: Arc<dyn AbortProbe> = Arc::new(ListenerAbort {
        deadline: deadline.min(now.checked_add(Duration::from_secs(30)).unwrap_or(now)),
        cancelled: Arc::clone(cancelled),
    });
    let (listener, site) = installed_listener(addr, profile, Some(startup))?;
    listener.set_nonblocking(true)?;
    loop {
        check(probe.as_ref())?;
        if finished.load(Ordering::Acquire) {
            return Ok(());
        }
        match listener.accept() {
            Ok((stream, _)) => {
                check(probe.as_ref())?;
                let remaining = deadline.saturating_duration_since(Instant::now());
                let timeout = profile
                    .query_timeout
                    .map_or(remaining, |time| time.min(remaining));
                let control = HttpConnectionControl::new(
                    Instant::now()
                        .checked_add(timeout)
                        .unwrap_or(deadline)
                        .min(deadline),
                    Arc::clone(&probe),
                );
                consume(
                    stream,
                    site.as_ref(),
                    profile.with_query_timeout(timeout),
                    &control,
                )?;
                check(probe.as_ref())?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(
                    Duration::from_millis(10)
                        .min(deadline.saturating_duration_since(Instant::now())),
                );
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
}

/// Loopback-only serving. At most 32 active connections; a full slot pool
/// refuses immediately rather than accumulating unbounded work.
fn installed_listener(
    addr: &str,
    profile: AccessProfile,
    startup_probe: Option<Arc<dyn AbortProbe>>,
) -> std::io::Result<(TcpListener, Option<Arc<crate::site::SoftwareSite>>)> {
    let addresses = addr.to_socket_addrs()?.collect::<Vec<_>>();
    if addresses.is_empty() || addresses.iter().any(|address| !address.ip().is_loopback()) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "native access HTTP must bind loopback",
        ));
    }
    // Missing/unassembled software refuses only site routes; existing APIs
    // retain their independent explicit selected owner behavior.
    // Software admission uses one startup deadline; each request owns a fresh probe.
    let site = crate::site::SoftwareSite::installed_for_mcp(startup_probe.unwrap_or_else(|| {
        profile
            .with_query_timeout(std::time::Duration::from_secs(30))
            .deadline_probe()
    }))
    .map_err(|error| std::io::Error::other(format!("{}: {}", error.code_str(), error.message)))?;
    let listener = TcpListener::bind(addresses.as_slice())?;
    Ok((listener, site))
}

pub fn serve(
    addr: &str,
    executor: Arc<dyn AccessExecutor>,
    profile: AccessProfile,
) -> std::io::Result<()> {
    let (listener, site) = installed_listener(addr, profile, None)?;
    let active = Arc::new(AtomicUsize::new(0));
    for accepted in listener.incoming() {
        let mut stream = accepted?;
        if active.fetch_add(1, Ordering::AcqRel) >= MAX_CONCURRENT {
            active.fetch_sub(1, Ordering::AcqRel);
            // Admission runs on the listener thread. A slow overloaded peer
            // must not block acceptance of later connections. Best-effort 503;
            // if the socket cannot accept it immediately, close the connection.
            if stream.set_nonblocking(true).is_ok() {
                let _ = write_response(&mut stream, ScopedHttpResponse::error(503, "server busy"));
            }
            continue;
        }
        let slot = ConnectionSlot(Arc::clone(&active));
        let executor = Arc::clone(&executor);
        let site = site.clone();
        // A resource-limited spawn closes this accepted socket and releases its
        // slot; it must not panic the listener or retain a phantom connection.
        let _ = std::thread::Builder::new().spawn(move || {
            let _slot = slot;
            serve_connection_with_software(stream, executor, profile, site);
        });
    }
    Ok(())
}

/// Owner-only measurement mode. Ordinary serve keeps its existing listener,
/// admission, packet and cleanup contract. EOF is control, never a request.
#[cfg(not(target_arch = "wasm32"))]
pub fn serve_observed(
    addr: &str,
    executor: Arc<dyn AccessExecutor>,
    profile: AccessProfile,
    deadline: crate::http_observation::OwnerDeadline,
) -> std::io::Result<()> {
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (addr, executor, profile, deadline);
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "HTTP observation requires Linux",
        ))
    }
    #[cfg(target_os = "linux")]
    {
        use crate::http_observation::{
            Observation, WorkerObservation, poll_control, require_control_pipe,
        };
        use std::os::fd::AsRawFd;
        require_control_pipe()?;
        deadline.remaining()?;
        let startup_probe = deadline.startup_probe(
            profile
                .with_query_timeout(Duration::from_secs(30))
                .deadline_probe(),
        );
        let (listener, site) = installed_listener(addr, profile, Some(startup_probe))?;
        deadline.remaining()?;
        listener.set_nonblocking(true)?;
        let observation = Arc::new(Observation::default());
        let active = Arc::new(AtomicUsize::new(0));
        let mut workers: Vec<std::thread::JoinHandle<()>> = Vec::with_capacity(MAX_CONCURRENT);
        let mut worker_failed = false;
        fn reap(workers: &mut Vec<std::thread::JoinHandle<()>>, failed: &mut bool) {
            let mut at = 0;
            while at < workers.len() {
                if workers[at].is_finished() {
                    if workers.swap_remove(at).join().is_err() {
                        *failed = true;
                    }
                } else {
                    at += 1;
                }
            }
        }
        let result = (|| -> std::io::Result<()> {
            loop {
                deadline.remaining()?;
                reap(&mut workers, &mut worker_failed);
                let (eof, ready) = poll_control(listener.as_raw_fd(), deadline)?;
                if eof {
                    break;
                }
                if !ready {
                    continue;
                }
                // Return to control/deadline polling after a finite burst.
                for _ in 0..MAX_CONCURRENT {
                    deadline.remaining()?;
                    reap(&mut workers, &mut worker_failed);
                    let mut stream = match listener.accept() {
                        Ok((stream, _)) => stream,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                        Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                        Err(error) => return Err(error),
                    };
                    if workers.len() >= MAX_CONCURRENT {
                        observation.refused();
                        if stream.set_nonblocking(true).is_ok() {
                            let _ = write_response(
                                &mut stream,
                                ScopedHttpResponse::error(503, "server busy"),
                            );
                        }
                        continue;
                    }
                    // The handle census bounds at most 32 owned workers; the
                    // existing slot also stays inside the closure through drop.
                    active.fetch_add(1, Ordering::AcqRel);
                    let slot = ConnectionSlot(Arc::clone(&active));
                    let observed_slot = observation.connection();
                    let worker_observation = Arc::clone(&observation);
                    let worker_executor = Arc::clone(&executor);
                    let worker_site = site.clone();
                    match std::thread::Builder::new().spawn(move || {
                        let _slot = slot;
                        let _observed_slot = observed_slot;
                        let _scope = WorkerObservation::enter(worker_observation);
                        serve_connection_with_software(
                            stream,
                            worker_executor,
                            profile,
                            worker_site,
                        );
                    }) {
                        Ok(worker) => workers.push(worker),
                        Err(_) => observation.spawn_failed(),
                    }
                }
            }
            // Stop accepts on EOF, then drain only under the SAME work clock.
            while !workers.is_empty() {
                deadline.remaining()?;
                reap(&mut workers, &mut worker_failed);
                if !workers.is_empty() {
                    std::thread::sleep(Duration::from_nanos(deadline.remaining()?.min(1_000_000)));
                }
            }
            if worker_failed {
                return Err(std::io::Error::other("HTTP observation worker unwound"));
            }
            deadline.remaining()?;
            Ok(())
        })();
        // Stop accepts on every exit, including invalid control/error paths.
        // All owned workers still get only the remaining original work clock.
        drop(listener);
        while !workers.is_empty() && deadline.remaining().is_ok() {
            reap(&mut workers, &mut worker_failed);
            if !workers.is_empty() {
                if let Ok(remaining) = deadline.remaining() {
                    std::thread::sleep(Duration::from_nanos(remaining.min(1_000_000)));
                }
            }
        }
        let summary = observation.emit(
            result.is_ok() && !worker_failed && workers.is_empty(),
            deadline,
        );
        match result {
            Err(primary) => Err(primary),
            Ok(()) => match summary {
                Ok(true) => Ok(()),
                Ok(false) => Err(std::io::Error::other("HTTP observation incomplete")),
                Err(error) => Err(error),
            },
        }
    }
}

fn philosophy_http_request(
    operation: KnowledgeOperation,
    encoded: &str,
    query: &str,
) -> Result<KnowledgeRequest, AccessError> {
    use KnowledgeOperation as O;
    if operation == O::PhilosophyScaleRows {
        return Ok(KnowledgeRequest::Philosophy(
            tos_query::philosophy_read::PhilosophyReadRequest::ScaleExport {
                table: encoded.split('.').next().unwrap_or("").to_owned(),
                view_id: query_value(query, "view_id").filter(|v| !v.is_empty()),
                layers: query_list(query, "layers"),
            },
        ));
    }
    let text = |s: String| JsonValue::String(JsonString::from_utf8(&s));
    let count = |key: &str, default: i64, low: i64, high: i64| {
        JsonValue::Number(JsonNumber {
            kind: JsonNumberKind::Int,
            lexeme: bounded_legacy_int(query_value(query, key).as_deref(), default, low, high)
                .to_string(),
        })
    };
    let list =
        |key: &str| JsonValue::Array(query_list(query, key).into_iter().map(&text).collect());
    let optional = |key: &str| {
        query_value(query, key)
            .filter(|value| !value.is_empty())
            .map(&text)
            .unwrap_or(JsonValue::Null)
    };
    let fields = match operation {
        O::PhilosophyStatus => vec![],
        O::PhilosophySearch => vec![
            (
                "query",
                text(query_value(query, "query").unwrap_or_default()),
            ),
            ("limit", count("limit", 20, 1, 100)),
        ],
        O::PhilosophyScaleManifest => {
            vec![("view_id", optional("view_id")), ("layers", list("layers"))]
        }
        O::PhilosophyScaleRows => vec![
            (
                "table",
                text(encoded.split('.').next().unwrap_or("").to_owned()),
            ),
            ("view_id", optional("view_id")),
            ("layers", list("layers")),
        ],
        O::PhilosophyNode => vec![("node_id", text(percent_decode(encoded, false)?))],
        O::PhilosophyEdge => vec![("edge_id", text(percent_decode(encoded, false)?))],
        O::PhilosophyView => vec![
            (
                "view_id",
                text(percent_decode(
                    encoded.split('/').next().unwrap_or(""),
                    false,
                )?),
            ),
            ("limit", count("limit", 1000, 1, 1000)),
        ],
        O::PhilosophyNeighborhood => vec![
            ("node_id", text(percent_decode(encoded, false)?)),
            ("depth", count("depth", 1, 1, 3)),
            ("limit", count("limit", 80, 1, 300)),
            ("layers", list("layers")),
            ("predicates", list("predicates")),
        ],
        O::PhilosophyPath => vec![
            (
                "from_id",
                text(query_value(query, "from").unwrap_or_default()),
            ),
            ("to_id", text(query_value(query, "to").unwrap_or_default())),
            ("layers", list("layers")),
            ("predicates", list("predicates")),
            ("max_depth", count("max_depth", 6, 1, 8)),
            (
                "direction",
                text(query_value(query, "direction").unwrap_or_else(|| "outgoing".into())),
            ),
            ("view_id", optional("view_id")),
            ("excluded_edge_ids", list("exclude")),
            ("alternative_limit", count("alternatives", 1, 1, 5)),
        ],
        O::PhilosophyClusters => vec![
            ("view_id", optional("view_id")),
            ("cluster_kind", optional("kind")),
            ("limit", count("limit", 80, 1, 1000)),
        ],
        O::PhilosophyReview => vec![(
            "view_id",
            text(query_value(query, "view_id").unwrap_or_else(|| "chronology".into())),
        )],
        O::PhilosophyUnresolved => vec![("view_id", optional("view_id"))],
        _ => vec![],
    };
    KnowledgeRequest::from_arguments(
        operation,
        &JsonValue::Object(
            fields
                .into_iter()
                .map(|(key, value)| (JsonString::from_utf8(key), value))
                .collect(),
        ),
    )
}
fn focus_http_request(encoded: &str, query: &str) -> Result<KnowledgeRequest, AccessError> {
    let text = |s: &str| JsonValue::String(JsonString::from_utf8(s));
    let count = |n: i64| {
        JsonValue::Number(JsonNumber {
            kind: JsonNumberKind::Int,
            lexeme: n.to_string(),
        })
    };
    let fields = vec![
        ("node_id", text(&percent_decode(encoded, false)?)),
        (
            "sources",
            JsonValue::Array(
                query_list(query, "sources")
                    .iter()
                    .map(|s| text(s))
                    .collect(),
            ),
        ),
        (
            "predicate_ids",
            JsonValue::Array(
                query_list(query, "predicates")
                    .iter()
                    .map(|s| text(s))
                    .collect(),
            ),
        ),
        (
            "depth",
            count(bounded_legacy_int(
                query_value(query, "depth").as_deref(),
                1,
                0,
                5,
            )),
        ),
        (
            "node_limit",
            count(bounded_legacy_int(
                query_value(query, "node_limit").as_deref(),
                200,
                1,
                1000,
            )),
        ),
        (
            "relation_limit",
            count(bounded_legacy_int(
                query_value(query, "relation_limit").as_deref(),
                400,
                0,
                2000,
            )),
        ),
        (
            "direction",
            text(&query_value(query, "direction").unwrap_or_else(|| "either".into())),
        ),
        (
            "profile",
            text(&query_value(query, "profile").unwrap_or_else(|| "overview".into())),
        ),
    ];
    let args = JsonValue::Object(
        fields
            .into_iter()
            .map(|(key, value)| (JsonString::from_utf8(key), value))
            .collect(),
    );
    crate::knowledge::focus_from_arguments(&args).map(KnowledgeRequest::Focus)
}

fn corpus_http_request(
    operation: KnowledgeOperation,
    encoded: &str,
    query: &str,
) -> Result<KnowledgeRequest, AccessError> {
    use KnowledgeOperation as O;
    use tos_query::corpus_read::CorpusReadRequest as R;
    let request = match operation {
        O::CorpusStatus => R::Status,
        O::CorpusSummary => R::Summary,
        O::CorpusSearch => R::Search {
            query: query_value(query, "query").unwrap_or_default(),
            limit: bounded_legacy_int(query_value(query, "limit").as_deref(), 20, 1, 100) as usize,
            resource_kind: None,
        },
        O::CorpusNode => R::Node {
            node_id: percent_decode(encoded, false)?,
        },
        O::CorpusRelationPack => R::RelationPack {
            pack_id: percent_decode(encoded, false)?,
        },
        O::CorpusGraphView => R::GraphView {
            view_id: percent_decode(encoded, false)?,
            limit: bounded_legacy_int(query_value(query, "limit").as_deref(), 100, 1, 1000)
                as usize,
        },
        _ => {
            return Err(AccessError::new(
                AccessErrorCode::InvalidRequest,
                "corpus HTTP route unavailable",
            ));
        }
    };
    Ok(KnowledgeRequest::Corpus(request))
}

#[cfg(test)]
mod connection_lifecycle_tests {
    use super::*;

    #[test]
    fn failed_handler_and_unstarted_handler_release_capacity() {
        let active = Arc::new(AtomicUsize::new(1));
        let slot = ConnectionSlot(Arc::clone(&active));
        assert!(
            std::thread::spawn(move || {
                let _slot = slot;
                panic!("synthetic handler failure");
            })
            .join()
            .is_err()
        );
        assert_eq!(active.load(Ordering::Acquire), 0);

        // Builder drops this owned closure when it cannot create a thread.
        // Exercise that ownership path without depending on host exhaustion.
        active.store(1, Ordering::Release);
        let slot = ConnectionSlot(Arc::clone(&active));
        let unstarted = move || {
            let _slot = slot;
        };
        drop(unstarted);
        assert_eq!(active.load(Ordering::Acquire), 0);
    }
}

fn scale_export_response<'hold>(
    mut response: ScopedHttpResponse<'hold>,
    encoded: &str,
    method: &str,
    profile: AccessProfile,
    probe: &Arc<dyn AbortProbe>,
) -> ScopedHttpResponse<'hold> {
    if response.status != 200 {
        return response;
    }
    let result = (|| -> Result<Vec<u8>, AccessError> {
        let format = encoded
            .rsplit_once('.')
            .map(|(_, v)| v)
            .filter(|v| matches!(*v, "jsonl" | "csv"))
            .ok_or_else(|| {
                AccessError::new(
                    AccessErrorCode::UnknownExactId,
                    "unknown scale export format",
                )
            })?;
        let limits = tos_foundation::JsonLimits {
            max_bytes: profile.max_response_bytes,
            max_depth: 64,
            max_visits: 300_000,
            max_integer_digits: 4300,
        };
        let document = tos_foundation::parse_json(
            &response.body,
            tos_foundation::JsonMode::PublishedStrict,
            limits,
        )
        .map_err(|_| {
            AccessError::new(
                AccessErrorCode::CorruptSelectedCarrier,
                "invalid scale rows packet",
            )
        })?;
        let rows = document
            .root()
            .object_get("rows")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| {
                AccessError::new(AccessErrorCode::CorruptSelectedCarrier, "scale rows absent")
            })?;
        let mut output = Vec::new();
        let mut append = |raw: &[u8]| -> Result<(), AccessError> {
            if let Some(reason) = probe.reason() {
                return Err(AccessError::new(
                    match reason {
                        tos_query::AbortReason::Cancelled => AccessErrorCode::Cancelled,
                        tos_query::AbortReason::DeadlineExceeded => {
                            AccessErrorCode::DeadlineExceeded
                        }
                    },
                    "scale serialization interrupted",
                ));
            }
            if output
                .len()
                .checked_add(raw.len())
                .is_none_or(|n| n > profile.max_response_bytes)
            {
                return Err(AccessError::new(
                    AccessErrorCode::BudgetExceeded,
                    "scale export response budget",
                ));
            }
            output.extend_from_slice(raw);
            Ok(())
        };
        if format == "jsonl" {
            for row in rows {
                let raw = tos_foundation::emit_value_preserved_json(row, limits).map_err(|_| {
                    AccessError::new(AccessErrorCode::BudgetExceeded, "scale row encoding budget")
                })?;
                append(&raw)?;
                append(b"\n")?;
            }
        } else {
            let columns = rows
                .iter()
                .flat_map(|r| {
                    r.as_object()
                        .unwrap_or(&[])
                        .iter()
                        .filter_map(|(k, _)| k.as_str())
                })
                .collect::<std::collections::BTreeSet<_>>();
            let columns = columns.into_iter().collect::<Vec<_>>();
            let encode = |value: &str| {
                if value.contains([',', '"', '\r', '\n']) {
                    format!("\"{}\"", value.replace('"', "\"\""))
                } else {
                    value.to_owned()
                }
            };
            append(
                columns
                    .iter()
                    .map(|v| encode(v))
                    .collect::<Vec<_>>()
                    .join(",")
                    .as_bytes(),
            )?;
            append(b"\r\n")?;
            for row in rows {
                for (i, key) in columns.iter().enumerate() {
                    if i != 0 {
                        append(b",")?;
                    }
                    let value = row.object_get(key).unwrap_or(&JsonValue::Null);
                    let value = match value {
                        JsonValue::Null => String::new(),
                        JsonValue::Bool(true) => "True".into(),
                        JsonValue::Bool(false) => "False".into(),
                        JsonValue::String(_) => value.as_str().unwrap_or("").to_owned(),
                        _ => String::from_utf8(
                            tos_foundation::emit_value_preserved_json(value, limits).map_err(
                                |_| {
                                    AccessError::new(
                                        AccessErrorCode::BudgetExceeded,
                                        "scale CSV value budget",
                                    )
                                },
                            )?,
                        )
                        .map_err(|_| {
                            AccessError::new(
                                AccessErrorCode::CorruptSelectedCarrier,
                                "scale CSV UTF8",
                            )
                        })?,
                    };
                    append(encode(&value).as_bytes())?;
                }
                append(b"\r\n")?;
            }
        }
        Ok(output)
    })();
    match result {
        Ok(body) => {
            response.body = body;
            response.content_type = if encoded.ends_with(".csv") {
                "text/csv; charset=utf-8"
            } else {
                "application/x-ndjson; charset=utf-8"
            };
            response
        }
        Err(error) => packet_response(Err(error), method, profile),
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod scoped_socket_tests {
    use super::*;
    use std::cell::Cell;

    struct BorrowedExecutor<'a>(&'a Cell<usize>);
    struct BorrowedFence<'a>(&'a Cell<usize>);
    impl DisclosureFence for BorrowedFence<'_> {
        fn recheck(&mut self) -> Result<(), AccessError> {
            self.0.set(self.0.get() + 1);
            Ok(())
        }
    }
    impl<'a> crate::common::ScopedAccessExecutor<'a> for BorrowedExecutor<'a> {
        fn source_descend(
            &self,
            _: Params,
            _: Arc<dyn AbortProbe>,
        ) -> Result<PreparedPacket<'a>, AccessError> {
            Err(AccessError::new(AccessErrorCode::Unavailable, "unselected"))
        }
        fn knowledge_available(&self, operation: KnowledgeOperation) -> bool {
            operation == KnowledgeOperation::Catalog
        }
        fn knowledge(
            &self,
            _: KnowledgeRequest,
            _: Arc<dyn AbortProbe>,
        ) -> Result<PreparedPacket<'a>, AccessError> {
            Ok(PreparedPacket {
                body: b"{}".to_vec(),
                fence: Box::new(BorrowedFence(self.0)),
            })
        }
    }

    #[test]
    fn controlled_socket_uses_original_deadline_before_query() {
        struct NeverAbort;
        impl AbortProbe for NeverAbort {
            fn reason(&self) -> Option<AbortReason> {
                None
            }
        }
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let checks = Cell::new(0);
        let executor = BorrowedExecutor(&checks);
        std::thread::scope(|scope| {
            let client = scope.spawn(move || {
                let mut socket = TcpStream::connect(address).unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                socket.write_all(b"G").unwrap();
                let mut response = Vec::new();
                let _ = socket.read_to_end(&mut response);
            });
            let (socket, _) = listener.accept().unwrap();
            let started = Instant::now();
            let control = HttpConnectionControl::new(
                started + Duration::from_millis(30),
                Arc::new(NeverAbort),
            );
            let error = serve_connection_scoped_controlled(
                socket,
                &executor,
                None,
                AccessProfile::new(8192, 1048576, 8192).with_query_timeout(Duration::from_secs(2)),
                &control,
            )
            .unwrap_err();
            assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
            assert!(started.elapsed() < Duration::from_secs(1));
            client.join().unwrap();
        });
        assert_eq!(checks.get(), 0);
    }

    #[test]
    fn borrowed_socket_retains_owner_fence_through_flush() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let checks = Cell::new(0);
        let executor = BorrowedExecutor(&checks);
        std::thread::scope(|scope| {
            let client = scope.spawn(move || {
                let mut socket = TcpStream::connect(address).unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                socket
                    .write_all(b"GET /api/knowledge/catalog HTTP/1.1\r\nHost: localhost\r\n\r\n")
                    .unwrap();
                let mut response = String::new();
                socket.read_to_string(&mut response).unwrap();
                response
            });
            let (socket, _) = listener.accept().unwrap();
            serve_connection_scoped(socket, &executor, AccessProfile::new(8192, 1048576, 8192))
                .unwrap();
            let response = client.join().unwrap();
            assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
            assert!(response.ends_with("\r\n\r\n{}"));
        });
        assert_eq!(checks.get(), 2);
    }
}
