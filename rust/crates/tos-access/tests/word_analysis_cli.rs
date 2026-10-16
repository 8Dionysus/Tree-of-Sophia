//! The installed-command shape must reach the real private reader, not an
//! unavailable-provider response or a subprocess running the Python oracle.
use std::{
    process::Command,
    time::{Duration, Instant},
};
#[allow(dead_code)]
#[path = "support/native_child.rs"]
mod native_child;
const PACKET_CAP: usize = 1_048_576;
use tos_query::reading_search::reading_fixture::ReadingFixture;

#[test]
fn native_word_task_and_candidate_refusal_use_selected_source() {
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut fixture = ReadingFixture::new_shared_root().with_word_analysis();
    let binary = native_child::selected_test_binary(env!("CARGO_BIN_EXE_tos-access"));
    let invoke = |extra: &[&str]| {
        native_child::bounded_output_before(
            Command::new(&binary)
                .arg("--root")
                .arg(&fixture.roots.source_root)
                .args([
                    "word-analysis",
                    "--query",
                    "судьбы",
                    "--language",
                    "ru",
                    "--rank",
                    "1",
                ])
                .args(extra),
            PACKET_CAP,
            deadline,
        )
    };
    let output = invoke(&[]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let task: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        task["schema_version"],
        "tos_zarathustra_word_analysis_task_v1"
    );
    assert_eq!(task["source"]["language"], "de");
    for (text, digest) in [
        ("surface", "surface_sha256"),
        ("exact_context", "context_sha256"),
    ] {
        assert_eq!(
            task["source"][digest],
            tos_foundation::Digest256::of_bytes(task["source"][text].as_str().unwrap().as_bytes())
                .to_hex()
        );
    }
    assert_eq!(task["authority"]["accepted"], false);
    assert_eq!(task["authority"]["canon_effect"], false);
    assert!(
        task["response_contract"]["validation_command"]
            .as_str()
            .unwrap()
            .contains("word-analysis --query")
    );
    // Existing callers pass the selected request as an absolute path. It is
    // still source data, never an alternate software/provider selection.
    let request = fixture.roots.source_root.join(
        "ToS/candidate-intake/zarathustra/concept-workbench-v1/requests/fate.concept-request.v2.json"
    );
    // Same existing child also exercises argparse's Unicode decimal spelling.
    let absolute = invoke(&["--request", request.to_str().unwrap(), "--rank", " +٠_١ "]);
    assert!(
        absolute.status.success(),
        "{}",
        String::from_utf8_lossy(&absolute.stderr)
    );
    let absolute_task: serde_json::Value = serde_json::from_slice(&absolute.stdout).unwrap();
    assert_eq!(absolute_task["analysis_task_id"], task["analysis_task_id"]);
    assert_eq!(absolute_task["source"], task["source"]);
    let candidate = fixture.root.join("invalid-candidate.json");
    std::fs::write(&candidate, b"{}").unwrap();
    let refused = invoke(&["--validate-candidate", candidate.to_str().unwrap()]);
    assert!(!refused.status.success());
    assert!(
        refused.stdout.is_empty(),
        "refusal must not disclose a partial source task"
    );
    assert_eq!(std::fs::read(candidate).unwrap(), b"{}");
    if let Some(path) = std::env::var_os("TOS_NATIVE_WORD_RETAIN_RECEIPT") {
        fixture.retain_word_for_consumers(std::path::Path::new(&path), &output.stdout, deadline);
    }
}

#[test]
fn native_concept_cards_preserve_coverage_zero_and_large_limit_identity() {
    let deadline = Instant::now() + Duration::from_secs(60);
    let fixture = ReadingFixture::new_shared_root();
    let binary = native_child::selected_test_binary(env!("CARGO_BIN_EXE_tos-access"));
    for limit in ["0", "1", "1000000000000000000000000000000"] {
        let output = native_child::bounded_output_before(
            Command::new(&binary)
                .arg("--root")
                .arg(&fixture.roots.source_root)
                .args([
                    "concept-search",
                    "--query",
                    "судьбы",
                    "--language",
                    "ru",
                    "--limit",
                    limit,
                ]),
            PACKET_CAP,
            deadline,
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let packet: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            packet["schema_version"],
            "tos_zarathustra_concept_search_result_v1"
        );
        let total = packet["coverage"]["total_source_results"].as_u64().unwrap();
        let returned = packet["results"].as_array().unwrap().len() as u64;
        assert_eq!(
            returned,
            total.min(limit.parse::<u64>().unwrap_or(u64::MAX))
        );
        assert_eq!(packet["coverage"]["returned_source_results"], returned);
        let identity = format!(
            "{}\nru\n{}\nsemantic=False\nlimit={limit}",
            packet["concept_search_route"]["route_id"].as_str().unwrap(),
            packet["query_analysis"]["normalized"].as_str().unwrap()
        );
        let digest = tos_foundation::Digest256::of_bytes(identity.as_bytes()).to_hex();
        assert_eq!(
            packet["search_result_id"],
            format!("tos.navigation.concept-search-result.sid-{}", &digest[..32])
        );
        for row in packet["results"].as_array().unwrap() {
            assert_eq!(row["accepted"], false);
            assert_eq!(row["canon_effect"], false);
        }
    }
}

#[test]
fn native_word_mcp_returns_the_real_source_task_in_maintained_envelope() {
    use std::io::Write;
    use std::process::Stdio;
    let deadline = Instant::now() + Duration::from_secs(60);
    let fixture = ReadingFixture::new_shared_root().with_word_analysis();
    let messages = [
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"word-native-consumer","version":"1"}}}),
        serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
        serde_json::json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"tos_zarathustra_prepare_word_analysis","arguments":{"query":"  судьбы  ","language":" RU ","rank":-12,"include_semantic_neighbors":"false"}}}),
    ];
    // Finite input file prevents stdin writes from blocking behind output.
    // It is outside the selected source root and is removed with the fixture.
    let input_path = fixture.root.join("word-mcp-request.jsonl");
    let mut input = std::fs::File::create(&input_path).unwrap();
    for message in messages {
        writeln!(input, "{message}").unwrap();
    }
    drop(input);
    assert!(std::fs::metadata(&input_path).unwrap().len() <= 65_536);
    let profile = tos_access::AccessProfile::new(65_536, PACKET_CAP, 65_536);
    let call_frame = tos_access::mcp::tool_result_frame_byte_bound(
        profile.max_response_bytes,
        profile.max_request_bytes,
    )
    .unwrap();
    // Three response frames (initialize, list, call), each constrained by
    // the executable's same outgoing frame profile before disclosure.
    let aggregate = call_frame.checked_mul(3).unwrap();
    let binary = native_child::selected_test_binary(env!("CARGO_BIN_EXE_tos-access"));
    let output = native_child::bounded_output_before(
        Command::new(&binary)
            .arg("--root")
            .arg(&fixture.roots.source_root)
            .arg("mcp")
            .stdin(Stdio::from(std::fs::File::open(input_path).unwrap())),
        aggregate,
        deadline,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let replies: Vec<serde_json::Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let tools = &replies.iter().find(|v| v["id"] == 2).unwrap()["result"]["tools"];
    assert!(
        tools
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["name"] == "tos_zarathustra_prepare_word_analysis")
    );
    let result = &replies.iter().find(|v| v["id"] == 3).unwrap()["result"];
    assert!(result.get("isError").is_none_or(|v| v == false), "{result}");
    let packet = &result["structuredContent"];
    assert_eq!(
        packet["schema"],
        "tos_zarathustra_word_analysis_capability_v1"
    );
    assert_eq!(packet["available"], true);
    assert_eq!(packet["authority"]["canon"], false);
    assert_eq!(
        packet["task"]["schema_version"],
        "tos_zarathustra_word_analysis_task_v1"
    );
    assert_eq!(packet["task"]["source"]["language"], "de");
    assert_eq!(packet["task"]["authority"]["semantic_fact_asserted"], false);
    let source = &packet["task"]["source"];
    assert_eq!(
        source["context_sha256"],
        tos_foundation::Digest256::of_bytes(source["exact_context"].as_str().unwrap().as_bytes())
            .to_hex()
    );
    let text: serde_json::Value =
        serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(text, *packet);
}

#[test]
fn native_http_word_uses_source_task_and_http_parameter_contract() {
    let deadline = Instant::now() + Duration::from_secs(60);
    let fixture = ReadingFixture::new_shared_root().with_word_analysis();
    let executor =
        tos_access::reading::ReadingLocalExecutor::open(fixture.roots.source_root.clone()).unwrap();
    let target = "/api/zarathustra/word-analysis?query=%D1%81%D1%83%D0%B4%D1%8C%D0%B1%D1%8B&language=&language=ru";
    let mut task = None;
    // HTTP int strings accept Unicode decimal, but not the MCP-only1.0 form
    // (the maintained HTTP fallback yields rank1 for that invalid spelling).
    for rank in ["%D9%A1", "1.0"] {
        let profile = tos_access::AccessProfile::new(65_536, PACKET_CAP, 65_536)
            .with_query_timeout(deadline.saturating_duration_since(Instant::now()));
        let response = tos_access::http::handle_get(
            &executor,
            "GET",
            &format!("{target}&rank=&rank={rank}"),
            profile,
        );
        assert_eq!(
            response.status,
            200,
            "{}",
            String::from_utf8_lossy(&response.body)
        );
        let packet: serde_json::Value = serde_json::from_slice(&response.body).unwrap();
        assert_eq!(packet["available"], true);
        assert_eq!(packet["task"]["source"]["language"], "de");
        assert_eq!(packet["task"]["authority"]["canon_effect"], false);
        if let Some(previous) = &task {
            assert_eq!(previous, &packet["task"]);
        }
        task = Some(packet["task"].clone());
    }
    let profile = tos_access::AccessProfile::new(65_536, PACKET_CAP, 65_536)
        .with_query_timeout(deadline.saturating_duration_since(Instant::now()));
    let refused = tos_access::http::handle_get(
        &executor,
        "GET",
        &format!("{target}&include_semantic_neighbors=t"),
        profile,
    );
    assert_eq!(refused.status, 400); // HTTP grammar is narrower than MCP bool.
    assert!(Instant::now() < deadline);
}
