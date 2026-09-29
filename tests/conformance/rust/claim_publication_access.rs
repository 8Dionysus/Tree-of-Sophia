//! Read continuation of the actual Claim publication case. The caller supplies
//! its committed database and receipt binding; this module never builds rows,
//! copies a database, issues authority, or substitutes an independent fixture.
use std::{io::Cursor, path::Path};

use serde_json::{Value, json};
use tos_access::{cli, http::handle_get, mcp::run_io, prepared_local};

fn segment(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            out.push(char::from(byte));
        } else {
            use std::fmt::Write;
            write!(out, "%{byte:02X}").unwrap();
        }
    }
    out
}

fn read_three_wires(
    executor: &prepared_local::PreparedLocalExecutor,
    args: &[String],
    path: &str,
    tool: &str,
    arguments: Value,
) -> Value {
    let profile = prepared_local::profile();
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    assert_eq!(
        cli::run_cli(args, executor, profile, &mut stdout, &mut stderr),
        0,
        "committed Claim CLI: {}",
        String::from_utf8_lossy(&stderr)
    );
    let expected: Value = serde_json::from_slice(&stdout).unwrap();
    let http = handle_get(executor, "GET", path, profile);
    assert_eq!(http.status, 200, "{}", String::from_utf8_lossy(&http.body));
    assert_eq!(
        expected,
        serde_json::from_slice::<Value>(&http.body).unwrap()
    );

    let mut input = Vec::new();
    for message in [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize",
            "params":{"protocolVersion":"2025-11-25"}}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call",
            "params":{"name":tool,"arguments":arguments}}),
    ] {
        serde_json::to_writer(&mut input, &message).unwrap();
        input.push(b'\n');
    }
    let mut output = Vec::new();
    run_io(Cursor::new(input), &mut output, executor, profile).unwrap();
    let replies: Vec<Value> = output
        .split(|b| *b == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice(line).unwrap())
        .collect();
    assert_eq!(replies.len(), 2);
    assert_eq!(replies[1]["id"], 2);
    assert!(replies[1].get("error").is_none(), "{}", replies[1]);
    assert_ne!(replies[1]["result"]["isError"], true);
    assert_eq!(expected, replies[1]["result"]["structuredContent"]);
    expected
}

/// Invoke only after the native publication's guarded commit. Both identifiers
/// must come from its normalized addition, not from the predecessor fixture.
pub(super) fn verify_published_access(
    database: &Path,
    receipt_binding: &Path,
    expected_node_id: &str,
    expected_relation_id: &str,
) {
    // Two independent opens prove that no writer-owned in-memory state is
    // needed to consume the committed publication. This is adapter execution,
    // not an installed-binary or network-server assertion.
    for _ in 0..2 {
        let executor = prepared_local::PreparedLocalExecutor::open(
            database.to_owned(),
            receipt_binding.to_owned(),
            None,
        )
        .unwrap();
        for (kind, identifier, argument, tool) in [
            ("node", expected_node_id, "node_id", "tos_knowledge_node"),
            (
                "relation",
                expected_relation_id,
                "relation_id",
                "tos_knowledge_relation",
            ),
        ] {
            let packet = read_three_wires(
                &executor,
                &["knowledge".into(), kind.into(), identifier.into()],
                &format!("/api/knowledge/{kind}s/{}", segment(identifier)),
                tool,
                json!({argument: identifier}),
            );
            assert!(
                packet["matches"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|row| row["id"] == identifier),
                "newly published {kind} {identifier} absent: {packet}"
            );
        }
        read_three_wires(
            &executor,
            &["knowledge".into(), "catalog".into()],
            "/api/knowledge/catalog",
            "tos_knowledge_catalog",
            json!({}),
        );
    }
}
