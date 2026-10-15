use super::*;

#[test]
#[ignore = "requires explicit isolated TOS_CMD_POSTGRES_URL"]
fn navigation_batch_uses_only_affected_neighborhood_and_keeps_shared_owners() {
    let url = std::env::var("TOS_CMD_POSTGRES_URL").expect("explicit private PG lab URL");
    let mut client = postgres::Client::connect(&url, postgres::NoTls).unwrap();
    let mut tx = client.transaction().unwrap();
    tx.batch_execute(
        "SET LOCAL statement_timeout='20s';
         CREATE TEMP TABLE cmd2_projection_navigation_bootstrap_row (
           domain text,consumer_id text,row_kind text,row_id text,owner_source_path text,
           owner_record_id text,active boolean,visible boolean,from_id text,to_id text,
           row_json text,row_sha256 text);
         CREATE INDEX ON cmd2_projection_navigation_bootstrap_row(domain,consumer_id,owner_source_path,owner_record_id);
         CREATE INDEX ON cmd2_projection_navigation_bootstrap_row(domain,consumer_id,row_id COLLATE \"C\") WHERE row_kind='node' AND active;
         CREATE INDEX ON cmd2_projection_navigation_bootstrap_row(domain,consumer_id,row_id COLLATE \"C\") WHERE row_kind='edge' AND active;
         CREATE INDEX ON cmd2_projection_navigation_bootstrap_row(domain,consumer_id,from_id COLLATE \"C\",row_id COLLATE \"C\") WHERE row_kind='edge' AND active;
         CREATE INDEX ON cmd2_projection_navigation_bootstrap_row(domain,consumer_id,to_id COLLATE \"C\",row_id COLLATE \"C\") WHERE row_kind='edge' AND active;
         CREATE TEMP TABLE cmd2_projection_navigation_node (
           domain text,consumer_id text,source_path text,record_id text,node_id text,
           visible boolean,row_json text,row_sha256 text);
         CREATE TEMP TABLE cmd2_projection_navigation_edge (
           domain text,consumer_id text,source_path text,record_id text,edge_id text,
           from_id text,to_id text,visible boolean,row_json text,row_sha256 text);
         CREATE TEMP TABLE cmd2_projection_navigation_diagnostic (
           domain text,consumer_id text,source_path text,record_id text,row_json text);
         CREATE TEMP TABLE cmd2_projection_navigation_dependency (
           domain text,consumer_id text,source_path text,record_id text,dependency_path text,
           dependency_role text,sha256 text,absent_reason text);",
    ).unwrap();
    let node = |id: &str| {
        let raw = serde_json::to_vec(&json!({"node_id":id,"label":id})).unwrap();
        PersistedNavigationNode {
            node_id: id.into(),
            visible: true,
            sha256: Digest256::of_bytes(&raw),
            row_json: raw,
        }
    };
    let edge = |id: &str, from: &str, to: &str| {
        let raw = serde_json::to_vec(&json!({"edge_id":id,"from_id":from,"to_id":to})).unwrap();
        PersistedNavigationEdge {
            edge_id: id.into(),
            from_id: from.into(),
            to_id: to.into(),
            visible: true,
            sha256: Digest256::of_bytes(&raw),
            row_json: raw,
        }
    };
    let insert = |tx: &mut postgres::Transaction<'_>,
                  kind: &str,
                  id: &str,
                  path: &str,
                  from: Option<&str>,
                  to: Option<&str>,
                  raw: &[u8]| {
        tx.execute(
            "INSERT INTO cmd2_projection_navigation_bootstrap_row VALUES
            ('lab','navigation',$1,$2,$3,$3,true,true,$4,$5,$6,$7)",
            &[
                &kind,
                &id,
                &path,
                &from,
                &to,
                &std::str::from_utf8(raw).unwrap(),
                &Digest256::of_bytes(raw).to_hex(),
            ],
        )
        .unwrap();
    };
    let a = node("a");
    let b = node("b");
    let c = node("c");
    let ab = edge("ab", "a", "b");
    let ca = edge("ca", "c", "a");
    insert(&mut tx, "node", "a", "changed", None, None, &a.row_json);
    insert(&mut tx, "node", "a", "shared", None, None, &a.row_json);
    insert(&mut tx, "node", "b", "external", None, None, &b.row_json);
    insert(&mut tx, "node", "c", "external", None, None, &c.row_json);
    insert(
        &mut tx,
        "edge",
        "ab",
        "changed",
        Some("a"),
        Some("b"),
        &ab.row_json,
    );
    insert(
        &mut tx,
        "edge",
        "ca",
        "external",
        Some("c"),
        Some("a"),
        &ca.row_json,
    );
    tx.batch_execute(
        "INSERT INTO cmd2_projection_navigation_bootstrap_row
         SELECT 'lab','navigation','node','unrelated:'||n,'unrelated','unrelated',true,true,
                NULL,NULL,'{}',repeat('0',64) FROM generate_series(1,100000) n;
         ANALYZE cmd2_projection_navigation_bootstrap_row;",
    )
    .unwrap();
    let seq_rows = |tx: &mut postgres::Transaction<'_>| -> i64 {
        tx.query_one(
            "SELECT COALESCE(sum(seq_tup_read),0)::bigint FROM pg_stat_xact_user_tables
                      WHERE schemaname LIKE 'pg_temp_%'",
            &[],
        )
        .unwrap()
        .get(0)
    };
    let before = seq_rows(&mut tx);
    let owners = BTreeSet::from([("changed".into(), "changed".into())]);
    let replacement = PersistedRows {
        dependencies: vec![],
        nodes: vec![a.clone()],
        edges: vec![ab],
        diagnostics: vec![],
        cohort_bytes: 256,
    };
    let limits = DurableNavigationEventLimits {
        max_current_rows: 32,
        max_adjacency_rows: 32,
        max_dependencies: 16,
        max_current_cohorts: 8,
        max_total_bytes: 16384,
        max_adjacency_bytes: 8192,
        max_row_bytes: 4096,
        ..Default::default()
    };
    verify_navigation_batch_final_budget(
        &mut tx,
        "lab",
        "navigation",
        &owners,
        std::slice::from_ref(&replacement),
        64,
        limits,
    )
    .unwrap();
    let (nodes, edges, affected) = load_navigation_batch_neighborhood(
        &mut tx,
        "lab",
        "navigation",
        &owners,
        &[&replacement],
        limits,
    )
    .unwrap();
    assert_eq!(nodes.len(), 4);
    assert_eq!(edges.len(), 2);
    assert_eq!(affected, BTreeSet::from(["a".into(), "c".into()]));
    let bootstrap = Digest256::of_bytes(b"bootstrap");
    let prior = Digest256::of_bytes(b"prior");
    let (updates, _) = prepare_navigation_adjacency_update_batch(
        &mut tx,
        "lab",
        "navigation",
        &owners,
        bootstrap,
        prior,
        2,
        &[&replacement],
        limits,
    )
    .unwrap();
    assert_eq!(updates.keys().cloned().collect::<BTreeSet<_>>(), affected);
    assert_eq!(updates["a"].as_ref().unwrap().edge_count, 1);
    assert_eq!(updates["c"].as_ref().unwrap().edge_count, 1);
    assert!(
        seq_rows(&mut tx) - before < 1000,
        "ordinary mutation must not scan the 100000 unrelated carriers"
    );

    let mut conflicting = replacement;
    conflicting.nodes[0].row_json = br#"{"node_id":"a","label":"changed"}"#.to_vec();
    conflicting.nodes[0].sha256 = Digest256::of_bytes(&conflicting.nodes[0].row_json);
    assert!(
        prepare_navigation_adjacency_update_batch(
            &mut tx,
            "lab",
            "navigation",
            &owners,
            bootstrap,
            prior,
            2,
            &[&conflicting],
            limits
        )
        .is_err(),
        "surviving shared contribution must agree exactly"
    );
    let removed_owners = BTreeSet::from([
        ("changed".into(), "changed".into()),
        ("shared".into(), "shared".into()),
    ]);
    let empty = PersistedRows {
        dependencies: vec![],
        nodes: vec![],
        edges: vec![],
        diagnostics: vec![],
        cohort_bytes: 0,
    };
    assert!(
        prepare_navigation_adjacency_update_batch(
            &mut tx,
            "lab",
            "navigation",
            &removed_owners,
            bootstrap,
            prior,
            2,
            &[&empty],
            limits
        )
        .is_err(),
        "removal cannot strand an unchanged incoming edge"
    );
    tx.execute(
        "UPDATE cmd2_projection_navigation_bootstrap_row SET row_json=repeat('x',6000)
                 WHERE row_id='b'",
        &[],
    )
    .unwrap();
    assert!(
        matches!(
            load_navigation_batch_neighborhood(
                &mut tx,
                "lab",
                "navigation",
                &owners,
                &[&conflicting],
                limits
            ),
            Err(DurableError::Refused(_))
        ),
        "SQL must gate oversized selected carriers before decoding them"
    );
    tx.rollback().unwrap();
}
