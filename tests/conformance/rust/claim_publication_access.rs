//! Read continuation of the actual Claim publication case. The caller supplies
//! its committed database and receipt binding; this module never builds rows,
//! copies a database, issues authority, or substitutes an independent fixture.
use std::{
    io::Cursor,
    path::Path,
    time::{Duration, Instant},
};

#[path = "../../../rust/crates/tos-access/tests/support/native_child.rs"]
mod native_child;

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

fn remaining(deadline: Instant) -> Duration {
    let left = deadline
        .checked_duration_since(Instant::now())
        .expect("whole Claim access deadline elapsed");
    assert!(!left.is_zero(), "whole Claim access deadline elapsed");
    left.min(Duration::from_secs(5))
}
fn profile_until(deadline: Instant) -> tos_access::AccessProfile {
    prepared_local::profile().with_query_timeout(remaining(deadline))
}

fn read_three_wires(
    executor: &prepared_local::PreparedLocalExecutor,
    args: &[String],
    path: &str,
    tool: &str,
    arguments: Value,
    deadline: Instant,
) -> Value {
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    assert_eq!(
        cli::run_cli(
            args,
            executor,
            profile_until(deadline),
            &mut stdout,
            &mut stderr
        ),
        0,
        "committed Claim CLI: {}",
        String::from_utf8_lossy(&stderr)
    );
    let expected: Value = serde_json::from_slice(&stdout).unwrap();
    let http = handle_get(executor, "GET", path, profile_until(deadline));
    assert_eq!(http.status, 200, "{}", String::from_utf8_lossy(&http.body));
    let mut wire = Vec::new();
    tos_access::http::write_response(&mut wire, http).unwrap();
    assert!(
        wire.starts_with(b"HTTP/1.1 200 "),
        "{}",
        String::from_utf8_lossy(&wire)
    );
    let body = wire
        .windows(4)
        .position(|bytes| bytes == b"\r\n\r\n")
        .unwrap()
        + 4;
    assert_eq!(
        expected,
        serde_json::from_slice::<Value>(&wire[body..]).unwrap()
    );
    remaining(deadline);

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
    run_io(
        Cursor::new(input),
        &mut output,
        executor,
        profile_until(deadline),
    )
    .unwrap();
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
    remaining(deadline);
    expected
}

/// Invoke only after the native publication's guarded commit. Both identifiers
/// must come from its normalized addition, not from the predecessor fixture.
/// The same caller deadline covers setup, hashing and every transport/child.
pub(super) fn verify_published_access_until(
    database: &Path,
    receipt_binding: &Path,
    expected_node_id: &str,
    expected_relation_id: &str,
    deadline: Instant,
) {
    // OPS selects and protects the coherent access product (which may be an
    // installed prefix). No copy or separate data fixture is made here.
    let binary = std::path::PathBuf::from(
        std::env::var_os("TOS_NATIVE_PREPARED_CONSUMER_BIN")
            .expect("exact protected access executable required"),
    );
    assert!(binary.is_absolute());
    let expected_sha = std::env::var("TOS_NATIVE_PREPARED_CONSUMER_SHA256").unwrap();
    assert_eq!(
        native_child::bounded_sha_before(&binary, 512 * 1024 * 1024, deadline).to_hex(),
        expected_sha
    );
    let actual = |args: &[String], expected: &Value| {
        let child_deadline = Instant::now() + remaining(deadline);
        let output = native_child::bounded_output_before(
            std::process::Command::new(&binary)
                .arg("--prepared-read-model")
                .arg(database)
                .arg("--prepared-binding")
                .arg(receipt_binding)
                .args(args),
            prepared_local::PREPARED_RESPONSE_BYTES,
            child_deadline.min(deadline),
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            *expected,
            serde_json::from_slice::<Value>(&output.stdout).unwrap()
        );
    };
    {
        remaining(deadline);
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
            let args = ["knowledge".into(), kind.into(), identifier.into()];
            let packet = read_three_wires(
                &executor,
                &args,
                &format!("/api/knowledge/{kind}s/{}", segment(identifier)),
                tool,
                json!({argument: identifier}),
                deadline,
            );
            assert!(
                packet["matches"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|row| row["id"] == identifier),
                "newly published {kind} {identifier} absent: {packet}"
            );
            actual(&args, &packet);
        }
        let args = ["knowledge".into(), "catalog".into()];
        let catalog = read_three_wires(
            &executor,
            &args,
            "/api/knowledge/catalog",
            "tos_knowledge_catalog",
            json!({}),
            deadline,
        );
        drop(executor);
        actual(&args, &catalog);
        remaining(deadline);
    }
}
