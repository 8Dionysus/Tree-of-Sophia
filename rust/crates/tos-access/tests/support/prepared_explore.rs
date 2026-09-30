// Included in the existing prepared_inspect_lens module: reuse its prepared
// producer/codec, and the maintained raw native fixture rather than invent rows.
#[test]
#[ignore = "requires finite admitted native prepare/exploration integration"]
fn prepared_explore_native_rows_pages_replay_and_current_fence() {
    use serde_json::{Value, json as value};
    use std::collections::{BTreeMap, BTreeSet};
    use std::time::Instant;
    use tos_access::{KnowledgeRequest, http::handle_post};
    let fixture = tos_compiler::knowledge_full_fixture::build_native_fixture_bounded(
        tos_compiler::knowledge_stage::StageLimits {
            sqlite: tos_compiler::Limits {
                max_rows: 1000,
                max_row_bytes: 1_048_576,
                max_output_bytes: 32 * 1024 * 1024,
                max_work_bytes: 128 * 1024 * 1024,
                sqlite_cache_kib: 8192,
                max_sql_vm_steps: 100_000_000,
            },
            max_temp_bytes: 16 * 1024 * 1024,
            max_seek_rows: 2,
            max_seek_bytes: 1_048_576,
        },
        Instant::now() + Duration::from_secs(30),
    );
    assert!(fixture.graph_input_bytes.len() <= 1_048_576);
    let graph = json(&fixture.graph_input_bytes);
    drop(fixture); // release producer DB before allocating prepared main/journal
    let nodes = graph
        .object_get("nodes")
        .unwrap()
        .as_array()
        .unwrap()
        .to_vec();
    let relations = graph
        .object_get("relations")
        .unwrap()
        .as_array()
        .unwrap()
        .to_vec();
    assert!(nodes.len() <= 200 && relations.len() <= 200);
    let mut degree = BTreeMap::<&str, usize>::new();
    for edge in &relations {
        for field in ["from_id", "to_id"] {
            *degree
                .entry(edge.object_get(field).unwrap().as_str().unwrap())
                .or_default() += 1;
        }
    }
    let origin = nodes
        .iter()
        .max_by_key(|n| {
            degree
                .get(n.object_get("id").unwrap().as_str().unwrap())
                .copied()
                .unwrap_or(0)
        })
        .unwrap();
    let origin_id = origin.object_get("id").unwrap().as_str().unwrap();
    assert!(
        degree.get(origin_id).copied().unwrap_or(0) > 1,
        "fixture must exercise actual pagination"
    );
    let revision = graph
        .object_get("source_revision")
        .unwrap()
        .as_str()
        .unwrap();
    let request = value!({"schema_version":"tos_exploration_request_v2","source_revision":revision,
        "origin":{"kind":"node","id":origin_id,"content_revision":origin.object_get("content_revision").unwrap().as_str().unwrap()},
        "profile":"all","max_depth":1,"page_nodes":1,"page_relations":1});
    let request_raw = serde_json::to_vec(&request).unwrap();
    let mut header = graph.clone();
    let JsonValue::Object(fields) = &mut header else {
        panic!("header")
    };
    fields.retain(|(k, _)| !matches!(k.as_str(), Some("nodes" | "relations")));
    // The selected-model fixture omits public delivery's owner label. Adapt
    // that boundary as the real public header does; do not alter native rows,
    // normalization identity or the three false authority flags.
    let boundary = fields
        .iter_mut()
        .find(|(key, _)| key.as_str() == Some("authority_boundary"))
        .unwrap();
    let JsonValue::Object(boundary) = &mut boundary.1 else {
        panic!("authority boundary")
    };
    if let Some((_, owner)) = boundary
        .iter()
        .find(|(key, _)| key.as_str() == Some("source_owner"))
    {
        assert_eq!(owner.as_str(), Some("Tree-of-Sophia"));
    } else {
        boundary.push((
            tos_foundation::JsonString::from_utf8("source_owner"),
            json(b"\"Tree-of-Sophia\""),
        ));
    }
    let catalog = json(
        format!(
            r#"{{"schema":"tos_knowledge_catalog_v1","source_revision":"{revision}","lenses":[]}}"#
        )
        .as_bytes(),
    );
    let dir = std::env::temp_dir().join(format!(
        "tos-prepared-explore-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&dir).unwrap();
    let path = dir.join("prepared.sqlite");
    let binding_path = dir.join("binding.json");
    let publication = PublicationLimits {
        max_bytes: 4 * 1024 * 1024,
        max_mutations: 100_000,
        max_row_bytes: 1_048_576,
        max_metadata_bytes: 1_048_576,
        max_changes: 16,
        max_change_bytes: 65_536,
    };
    let binding = publish_prepared_rows_until(
        &path,
        &header,
        &catalog,
        &mut Rows {
            nodes: nodes.clone(),
            relations: relations.clone(),
        },
        publication,
        Instant::now() + Duration::from_secs(10),
    )
    .unwrap();
    fs::write(&binding_path, encode(&binding)).unwrap();
    let executor = tos_access::prepared_local::PreparedLocalExecutor::open(
        path.clone(),
        binding_path.clone(),
        None,
    )
    .unwrap();
    let profile = tos_access::prepared_local::profile();
    let capabilities = tos_access::http::handle_get(
        &executor,
        "GET",
        "/api/knowledge/explore/capabilities",
        profile,
    );
    assert_eq!(capabilities.status, 200);
    let capabilities: Value = serde_json::from_slice(&capabilities.body).unwrap();
    assert_eq!(capabilities["available"], true);
    assert_eq!(
        capabilities["execution_version"],
        tos_query::knowledge_exploration::PUBLISHED_EXPLORATION_EXECUTION_VERSION
    );
    assert_eq!(capabilities["restart_survival"], false);
    assert_eq!(capabilities["limits"]["work_per_page"], 512);
    let post = |body: &[u8]| {
        let response = handle_post(&executor, "/api/knowledge/explore", body, profile);
        assert_eq!(
            response.status,
            200,
            "{}",
            String::from_utf8_lossy(&response.body)
        );
        let mut wire = Vec::new();
        tos_access::http::write_response(&mut wire, response).unwrap();
        assert!(
            wire.starts_with(b"HTTP/1.1 200 "),
            "{}",
            String::from_utf8_lossy(&wire)
        );
        let split = wire
            .windows(4)
            .position(|part| part == b"\r\n\r\n")
            .unwrap()
            + 4;
        serde_json::from_slice::<Value>(&wire[split..]).unwrap()
    };
    let mut packet = post(&request_raw);
    assert_eq!(packet["status"], "paused");
    let first_cursor = packet["page"]["next_cursor"].as_str().unwrap().to_owned();
    let mut seen_nodes = BTreeSet::new();
    let mut seen_relations = BTreeSet::new();
    let mut continued = false;
    let mut completed = false;
    for _ in 0..32 {
        assert_eq!(packet["source_revision"], revision);
        for (field, source, seen) in [
            ("nodes", &nodes, &mut seen_nodes),
            ("relations", &relations, &mut seen_relations),
        ] {
            for row in packet[field].as_array().unwrap() {
                let id = row["id"].as_str().unwrap();
                let original = source
                    .iter()
                    .find(|n| n.object_get("id").and_then(JsonValue::as_str) == Some(id))
                    .expect("only actual published carriers");
                assert_eq!(
                    row["content_revision"].as_str(),
                    original
                        .object_get("content_revision")
                        .and_then(JsonValue::as_str)
                );
                seen.insert(id.to_owned());
            }
        }
        let Some(cursor) = packet["page"]["next_cursor"].as_str() else {
            completed = true;
            break;
        };
        let body = serde_json::to_vec(&value!({"cursor":cursor})).unwrap();
        packet = post(&body);
        if !continued {
            assert_eq!(
                post(&body),
                packet,
                "same cursor replays exact admitted page"
            );
            continued = true;
        }
    }
    assert!(
        completed && continued,
        "bounded fixture must complete, not merely produce a first page"
    );
    assert!(seen_nodes.contains(origin_id) && !seen_relations.is_empty());
    let mut held = executor
        .knowledge(
            KnowledgeRequest::Explore(json(&request_raw)),
            profile.deadline_probe(),
        )
        .unwrap();
    // A real metadata-only successor invalidates old selections and held pages.
    let mut next_header = header.clone();
    let JsonValue::Object(fields) = &mut next_header else {
        unreachable!()
    };
    for (key, value) in fields {
        if key.as_str() == Some("source_revision") {
            *value = json(format!("\"{}\"", "d".repeat(64)).as_bytes());
        }
    }
    let next_catalog = json(
        format!(
            r#"{{"schema":"tos_knowledge_catalog_v1","source_revision":"{}","lenses":[]}}"#,
            "d".repeat(64)
        )
        .as_bytes(),
    );
    apply_prepared_delta_until(
        &path,
        &binding,
        &next_header,
        &next_catalog,
        std::iter::empty::<tos_compiler::Result<PreparedChange>>(),
        publication,
        Instant::now() + Duration::from_secs(10),
    )
    .unwrap();
    assert!(
        held.fence.recheck().is_err(),
        "no old page/cursor admission after currentness changes"
    );
    let stale = handle_post(
        &executor,
        "/api/knowledge/explore",
        &serde_json::to_vec(&value!({"cursor":first_cursor})).unwrap(),
        profile,
    );
    assert_ne!(stale.status, 200);
    drop(held);
    drop(executor);
    fs::remove_dir_all(dir).unwrap();
}
