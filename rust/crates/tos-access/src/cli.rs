//! Maintained one-shot options over owner-selected native query families.

use crate::{KnowledgeOperation, KnowledgeRequest};
use std::io::{Read, Write};
use tos_foundation::{JsonMode, JsonNumber, JsonNumberKind, JsonString, JsonValue, parse_json};

use crate::common::{checked_execute, validate_packet};
use crate::{AccessExecutor, AccessProfile, Params, PreparedPacket};

/// Consume the complete held packet within its owner scope, through final flush.
pub fn write_packet(
    mut packet: PreparedPacket<'_>,
    profile: AccessProfile,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    if packet.body.len() > profile.max_response_bytes {
        let _ = writeln!(stderr, "budget_exceeded: response byte budget exceeded");
        return 1;
    }
    if let Err(error) = validate_packet(&packet.body, profile.max_response_bytes) {
        let _ = writeln!(stderr, "{}: {}", error.code_str(), error.message);
        return 1;
    }
    if let Err(error) = packet.fence.recheck() {
        let _ = writeln!(stderr, "{}: {}", error.code_str(), error.message);
        return 1;
    }
    let result = stdout
        .write_all(&packet.body)
        .and_then(|_| stdout.write_all(b"\n"))
        .and_then(|_| stdout.flush());
    drop(packet);
    if let Err(error) = result {
        let _ = writeln!(stderr, "output failed: {error}");
        return 1;
    }
    0
}

fn run_search(
    args: &[String],
    executor: &dyn AccessExecutor,
    profile: AccessProfile,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    let string = |v: &str| JsonValue::String(JsonString::from_utf8(v));
    let number = |v: usize| {
        JsonValue::Number(JsonNumber {
            kind: JsonNumberKind::Int,
            lexeme: v.to_string(),
        })
    };
    let mut fields = vec![];
    let mut sources = Vec::new();
    let mut kinds = Vec::new();
    let mut predicates = Vec::new();
    let mut at = 2;
    if args.get(at).is_some_and(|arg| !arg.starts_with("--")) {
        fields.push((JsonString::from_utf8("query"), string(&args[at])));
        at += 1;
    }
    while at < args.len() {
        let option = args[at].as_str();
        if option == "--" || !option.starts_with("--") {
            if option == "--" {
                at += 1;
            }
            if fields.iter().any(|(key, _)| key.as_str() == Some("query"))
                || args.get(at).is_none()
                || (option == "--" && at + 1 != args.len())
            {
                let _ = writeln!(stderr, "search accepts one optional query");
                return 2;
            }
            fields.push((JsonString::from_utf8("query"), string(&args[at])));
            at += 1;
            continue;
        }
        if option == "--sources" {
            sources.clear();
            at += 1;
            while at < args.len() && !args[at].starts_with("--") {
                sources.push(string(&args[at]));
                at += 1;
            }
            continue;
        }
        let Some(value) = args.get(at + 1) else {
            let _ = writeln!(stderr, "missing value for {option}");
            return 2;
        };
        let field = match option {
            "--kind" => {
                kinds.push(string(value));
                None
            }
            "--predicate" => {
                predicates.push(string(value));
                None
            }
            "--mode" => Some(("mode", string(value))),
            "--cursor" => Some(("cursor", string(value))),
            "--offset" | "--limit" => {
                let Ok(value) = value.parse::<usize>() else {
                    let _ = writeln!(stderr, "invalid value for {option}");
                    return 2;
                };
                Some((
                    if option == "--offset" {
                        "offset"
                    } else {
                        "limit"
                    },
                    number(value),
                ))
            }
            _ => {
                let _ = writeln!(stderr, "unsupported search option: {option}");
                return 2;
            }
        };
        if let Some((key, value)) = field {
            fields.retain(|(name, _)| name.as_str() != Some(key));
            fields.push((JsonString::from_utf8(key), value));
        }
        at += 2;
    }
    for (key, values) in [
        ("sources", sources),
        ("kind_ids", kinds),
        ("predicate_ids", predicates),
    ] {
        fields.push((JsonString::from_utf8(key), JsonValue::Array(values)));
    }
    let request = match crate::search::SearchRequest::from_arguments(&JsonValue::Object(fields)) {
        Ok(value) => value,
        Err(error) => {
            let _ = writeln!(stderr, "{}: {}", error.code_str(), error.message);
            return 2;
        }
    };
    match checked_execute(profile.deadline_probe(), |probe| {
        request.execute(executor, probe)
    }) {
        Ok(packet) => write_packet(packet, profile, stdout, stderr),
        Err(error) => {
            let _ = writeln!(stderr, "{}: {}", error.code_str(), error.message);
            if error.code == crate::AccessErrorCode::Unavailable {
                3
            } else if error.code == crate::AccessErrorCode::InvalidRequest {
                2
            } else {
                1
            }
        }
    }
}

/// Exit code: 0 success, 2 request syntax, 3 selected capability unavailable,
/// 1 query/disclosure or output failure. Diagnostics stay on stderr.
pub fn run_cli(
    args: &[String],
    executor: &dyn AccessExecutor,
    profile: AccessProfile,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    run_cli_with_input(
        args,
        executor,
        profile,
        &mut std::io::stdin(),
        stdout,
        stderr,
    )
}

fn expanded_options(args: &[String]) -> Vec<String> {
    args.iter()
        .flat_map(|arg| {
            if arg.starts_with("--") {
                if let Some((option, value)) = arg.split_once('=') {
                    return vec![option.to_owned(), value.to_owned()];
                }
            }
            vec![arg.clone()]
        })
        .collect()
}

/// Maintained standalone listener options; binding remains loopback-only in
/// HTTP and no source owner is selected by a host or port argument.
pub fn parse_serve_address(args: &[String]) -> Result<String, crate::AccessError> {
    let invalid = || {
        crate::AccessError::new(
            crate::AccessErrorCode::InvalidRequest,
            "usage: tos-access serve [--host HOST] [--port PORT]",
        )
    };
    if args.len() == 1 && !args[0].starts_with("--") {
        return Ok(args[0].clone());
    }
    let args = expanded_options(args);
    let mut host = "127.0.0.1";
    let mut port = 8080u16;
    let mut at = 0;
    while at < args.len() {
        let value = args.get(at + 1).ok_or_else(invalid)?;
        match args[at].as_str() {
            "--host" if !value.is_empty() => host = value,
            "--port" => port = value.parse().map_err(|_| invalid())?,
            _ => return Err(invalid()),
        }
        at += 2;
    }
    Ok(if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    })
}

/// Structured queries use the same bounded parser for files and stdin.
pub fn run_cli_with_input(
    args: &[String],
    executor: &dyn AccessExecutor,
    profile: AccessProfile,
    stdin: &mut dyn Read,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    if args
        .iter()
        .try_fold(0usize, |n, arg| n.checked_add(arg.len()))
        .is_none_or(|n| n > profile.max_request_bytes)
    {
        let _ = writeln!(stderr, "budget_exceeded: argument byte budget exceeded");
        return 2;
    }
    let expanded = expanded_options(args);
    let args = expanded.as_slice();
    if args.first().is_some_and(|arg| arg == "reading-search") {
        return crate::reading::run_cli(args, executor, profile, stdout, stderr);
    }
    if let Some(code) = run_knowledge(args, executor, profile, stdin, stdout, stderr) {
        return code;
    }
    if args.len() >= 2 && args[0] == "knowledge" && args[1] == "search" {
        return run_search(args, executor, profile, stdout, stderr);
    }
    if args.len() < 3 || args[0] != "source" || args[1] != "descend" {
        let _ = writeln!(
            stderr,
            "usage: tos-access source descend NODE_ID [--max-depth 1..8] [--limit 1..300]"
        );
        return 2;
    }
    let mut max_depth = 8;
    let mut limit = 300;
    let mut at = 3;
    while at < args.len() {
        let Some(value) = args.get(at + 1) else {
            let _ = writeln!(stderr, "missing value for {}", args[at]);
            return 2;
        };
        let parsed = match value.parse::<usize>() {
            Ok(value) => value,
            Err(_) => {
                let _ = writeln!(stderr, "invalid value for {}", args[at]);
                return 2;
            }
        };
        match args[at].as_str() {
            "--max-depth" if (1..=8).contains(&parsed) => max_depth = parsed as u8,
            "--limit" if (1..=300).contains(&parsed) => limit = parsed,
            _ => {
                let _ = writeln!(stderr, "unsupported option or range: {}", args[at]);
                return 2;
            }
        }
        at += 2;
    }
    let params = match Params::new(args[2].clone(), max_depth, limit) {
        Ok(value) => value,
        Err(error) => {
            let _ = writeln!(stderr, "{}: {}", error.code_str(), error.message);
            return 2;
        }
    };
    if !executor.source_descend_available() {
        let _ = writeln!(
            stderr,
            "source descent unavailable: no owner-selected read model and current-policy fence"
        );
        return 3;
    }
    let packet = match checked_execute(profile.deadline_probe(), |probe| {
        executor.source_descend(params, probe)
    }) {
        Ok(packet) => packet,
        Err(error) => {
            let _ = writeln!(stderr, "{}: {}", error.code_str(), error.message);
            return 1;
        }
    };
    write_packet(packet, profile, stdout, stderr)
}

fn run_knowledge(
    args: &[String],
    executor: &dyn AccessExecutor,
    profile: AccessProfile,
    stdin: &mut dyn Read,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Option<i32> {
    let operations = match crate::common::registered_operations() {
        Ok(ops) => ops,
        Err(_) => return None,
    };
    let operation = operations.iter().find(|operation| {
        operation.cli_command.as_ref().is_some_and(|command| {
            let tokens = command.split_whitespace().collect::<Vec<_>>();
            crate::KnowledgeOperation::from_id(&operation.operation_id).is_some()
                && args.len() >= tokens.len()
                && tokens.iter().zip(args).all(|(a, b)| a == b)
        })
    })?;
    let op = KnowledgeOperation::from_id(&operation.operation_id)?;
    let result: Result<KnowledgeRequest, crate::AccessError> = (|| match op {
        KnowledgeOperation::Catalog if args.len() == 2 => Ok(KnowledgeRequest::Catalog),
        KnowledgeOperation::SearchCapabilities if args.len() == 2 => {
            Ok(KnowledgeRequest::SearchCapabilities)
        }
        KnowledgeOperation::Contracts if args.len() == 2 => Ok(KnowledgeRequest::Contracts),
        KnowledgeOperation::StoredLens
            if args.len() == 3 && !args[2].is_empty() && args[2].chars().count() <= 4096 =>
        {
            Ok(KnowledgeRequest::StoredLens {
                lens_id: args[2].clone(),
            })
        }
        KnowledgeOperation::Focus if args.len() >= 3 => focus_cli_request(args),
        KnowledgeOperation::Node if args.len() >= 3 => {
            let mut relation_limit = 200;
            let mut at = 3;
            while at < args.len() {
                if args[at] != "--relation-limit" {
                    return Err(crate::AccessError::new(
                        crate::AccessErrorCode::InvalidRequest,
                        "unsupported node option",
                    ));
                }
                relation_limit = args
                    .get(at + 1)
                    .and_then(|value| value.parse::<usize>().ok())
                    .filter(|n| *n <= 1000)
                    .ok_or_else(|| {
                        crate::AccessError::new(
                            crate::AccessErrorCode::InvalidRequest,
                            "relation_limit must be in 0..1000",
                        )
                    })?;
                at += 2;
            }
            if args[2].is_empty() || args[2].chars().count() > 4096 {
                return Err(crate::AccessError::new(
                    crate::AccessErrorCode::InvalidRequest,
                    "invalid node identifier",
                ));
            }
            Ok(KnowledgeRequest::Node {
                node_id: args[2].clone(),
                relation_limit,
            })
        }
        KnowledgeOperation::Relation
            if args.len() == 3 && !args[2].is_empty() && args[2].chars().count() <= 4096 =>
        {
            Ok(KnowledgeRequest::Relation {
                relation_id: args[2].clone(),
            })
        }
        KnowledgeOperation::Temporal | KnowledgeOperation::Lens if args.len() == 3 => {
            let max = profile.max_request_bytes.checked_add(1).ok_or_else(|| {
                crate::AccessError::new(
                    crate::AccessErrorCode::BudgetExceeded,
                    "request byte cap invalid",
                )
            })?;
            let mut raw = Vec::new();
            let read = if args[2] == "-" {
                stdin.take(max as u64).read_to_end(&mut raw)
            } else {
                std::fs::File::open(&args[2])
                    .and_then(|file| file.take(max as u64).read_to_end(&mut raw))
            };
            read.map_err(|_| {
                crate::AccessError::new(
                    crate::AccessErrorCode::InvalidRequest,
                    "cannot read structured query input",
                )
            })?;
            if raw.len() > profile.max_request_bytes {
                return Err(crate::AccessError::new(
                    crate::AccessErrorCode::BudgetExceeded,
                    "structured query input byte budget exceeded",
                ));
            }
            let document = parse_json(&raw, JsonMode::RequestLastWins, profile.json_limits())
                .map_err(|_| {
                    crate::AccessError::new(
                        crate::AccessErrorCode::InvalidRequest,
                        "invalid bounded query JSON",
                    )
                })?;
            KnowledgeRequest::from_body(op, document.into_root())
        }
        _ => Err(crate::AccessError::new(
            crate::AccessErrorCode::InvalidRequest,
            "invalid native knowledge command arguments",
        )),
    })();
    Some(match result {
        Err(error) => {
            let _ = writeln!(stderr, "{}: {}", error.code_str(), error.message);
            2
        }
        Ok(request) if !executor.knowledge_available(request.operation()) => {
            let _ = writeln!(stderr, "selected knowledge operation unavailable");
            3
        }
        Ok(request) => match checked_execute(profile.deadline_probe(), |probe| {
            executor.knowledge(request, probe)
        }) {
            Ok(packet) => write_packet(packet, profile, stdout, stderr),
            Err(error) => {
                let _ = writeln!(stderr, "{}: {}", error.code_str(), error.message);
                1
            }
        },
    })
}

fn focus_cli_request(args: &[String]) -> Result<KnowledgeRequest, crate::AccessError> {
    let invalid =
        |message| crate::AccessError::new(crate::AccessErrorCode::InvalidRequest, message);
    let text = |s: &str| JsonValue::String(JsonString::from_utf8(s));
    let mut fields = vec![(JsonString::from_utf8("node_id"), text(&args[2]))];
    let mut sources = Vec::new();
    let mut predicates = Vec::new();
    let mut at = 3;
    while at < args.len() {
        let option = args[at].as_str();
        if option == "--sources" {
            sources.clear();
            at += 1;
            while at < args.len() && !args[at].starts_with("--") {
                sources.push(text(&args[at]));
                at += 1;
            }
            continue;
        }
        let value = args
            .get(at + 1)
            .ok_or_else(|| invalid("focus option requires a value"))?;
        let key = match option {
            "--depth" => "depth",
            "--direction" => "direction",
            "--node-limit" => "node_limit",
            "--relation-limit" => "relation_limit",
            "--profile" => "profile",
            "--predicate" => {
                predicates.push(text(value));
                at += 2;
                continue;
            }
            _ => return Err(invalid("unsupported focus option")),
        };
        let value = if matches!(key, "depth" | "node_limit" | "relation_limit") {
            let n = value
                .parse::<u64>()
                .map_err(|_| invalid("focus count must be a nonnegative integer"))?;
            JsonValue::Number(JsonNumber {
                kind: JsonNumberKind::Int,
                lexeme: n.to_string(),
            })
        } else {
            text(value)
        };
        if let Some((_, old)) = fields
            .iter_mut()
            .find(|(name, _)| name.as_str() == Some(key))
        {
            *old = value
        } else {
            fields.push((JsonString::from_utf8(key), value));
        }
        at += 2;
    }
    fields.push((JsonString::from_utf8("sources"), JsonValue::Array(sources)));
    fields.push((
        JsonString::from_utf8("predicate_ids"),
        JsonValue::Array(predicates),
    ));
    crate::knowledge::focus_from_arguments(&JsonValue::Object(fields)).map(KnowledgeRequest::Focus)
}
