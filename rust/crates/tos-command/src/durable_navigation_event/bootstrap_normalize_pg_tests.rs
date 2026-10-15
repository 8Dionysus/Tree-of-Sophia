use super::*;

#[test]
#[ignore = "requires explicit isolated TOS_CMD_POSTGRES_URL"]
fn navigation_initial_normalization_matches_shared_rules_across_committed_pages() {
    let url = std::env::var("TOS_CMD_POSTGRES_URL").expect("explicit private PG lab URL");
    let connect = || {
        let mut client = postgres::Client::connect(&url, postgres::NoTls).unwrap();
        client
            .batch_execute(
                "SET search_path=tos_initial_normalization_test; SET statement_timeout='20s'",
            )
            .unwrap();
        client
    };
    let mut client = connect();
    client.batch_execute(
        "CREATE SCHEMA tos_initial_normalization_test;
         CREATE TABLE cmd2_projection_navigation_initial(domain text,consumer_id text,
             selection_json text,header_raw text,position_json text,PRIMARY KEY(domain,consumer_id));
         CREATE TABLE cmd2_projection_navigation_initial_row(domain text,consumer_id text,
             collection text,row_key text,row_id text,ordinal bigint,row_json text,row_sha256 text,
             node_facts_json text,PRIMARY KEY(domain,consumer_id,collection,row_key));
         CREATE INDEX ON cmd2_projection_navigation_initial_row(domain,consumer_id,collection,row_key COLLATE \"C\");
         CREATE TABLE cmd2_projection_navigation_initial_contributor(domain text,consumer_id text,
             node_id text,owner_source_path text,owner_record_id text,
             PRIMARY KEY(domain,consumer_id,node_id,owner_source_path,owner_record_id));
         CREATE TABLE cmd2_projection_navigation_bootstrap_owner(domain text,consumer_id text,
             owner_source_path text,source_sha256 text,size_bytes bigint,
             PRIMARY KEY(domain,consumer_id,owner_source_path));
         CREATE TABLE cmd2_projection_navigation_bootstrap_row(domain text,consumer_id text,
             row_kind text,row_id text,owner_source_path text,owner_record_id text,
             active boolean DEFAULT true,visible boolean,from_id text,to_id text,row_json text,row_sha256 text,
             PRIMARY KEY(domain,consumer_id,row_kind,row_id,owner_source_path,owner_record_id));
         CREATE TABLE cmd2_projection_navigation_bootstrap_dependency(domain text,consumer_id text,
             owner_source_path text,owner_record_id text,dependency_path text,
             PRIMARY KEY(domain,consumer_id,owner_source_path,owner_record_id,dependency_path));"
    ).unwrap();
    let source = "ToS/current.json";
    let archive = "ToS/archive.json";
    let claim_file = "ToS/claims.jsonl";
    let mut input = Vec::new();
    for index in 0..300 {
        input.push(("nodes", json!({"node_id":format!("unrelated:{index:06}"), "source_ref":source, "payload":"x".repeat(5000)})));
    }
    input.extend([
        (
            "nodes",
            json!({"node_id":"work:current","source_ref":source}),
        ),
        (
            "nodes",
            json!({"node_id":"record-version:old","source_ref":archive,"properties":{
            "record_version_view":{"record_ref":{"id":"work:current","version":1},
                "provenance":{"source":{"source_ref":source}}}}}),
        ),
        (
            "nodes",
            json!({"node_id":"claim:a","source_ref":claim_file}),
        ),
        (
            "nodes",
            json!({"node_id":"claim:b","source_ref":claim_file}),
        ),
        (
            "nodes",
            json!({"node_id":"evidence:shared","source_ref":source}),
        ),
        (
            "nodes",
            json!({"node_id":"maker:a","node_kind":"maker","source_ref":source,
            "properties":{"identity_node_id":"maker:b"}}),
        ),
        (
            "nodes",
            json!({"node_id":"maker:b","node_kind":"maker","source_ref":source,
            "properties":{"identity_node_id":"identity:shared"}}),
        ),
        (
            "nodes",
            json!({"node_id":"identity:shared","source_ref":source}),
        ),
        (
            "edges",
            json!({"edge_id":"edge:a","from_id":"claim:a","to_id":"evidence:shared",
            "claim_ref":"a","source_claim_file_ref":claim_file,"source_ref":claim_file}),
        ),
        (
            "edges",
            json!({"edge_id":"edge:b","from_id":"claim:b","to_id":"evidence:shared",
            "claim_ref":"b","source_claim_file_ref":claim_file,"source_ref":claim_file}),
        ),
        (
            "edges",
            json!({"edge_id":"edge:maker","from_id":"claim:a","to_id":"maker:a",
            "claim_ref":"a","source_claim_file_ref":claim_file,"source_ref":claim_file}),
        ),
        (
            "edges",
            json!({"edge_id":"edge:alternative","from_id":"claim:a","to_id":"claim:b",
            "claim_ref":"a","source_claim_file_ref":claim_file,"source_ref":claim_file}),
        ),
        (
            "edges",
            json!({"edge_id":"edge:subject","edge_kind":"claim_subject_navigation",
            "predicate_id":"has_source_claim","from_id":"work:current","to_id":"claim:b",
            "claim_ref":"b","source_claim_file_ref":claim_file,"source_ref":claim_file}),
        ),
        (
            "edges",
            json!({"edge_id":"history:one","edge_kind":"exact_historical_record_reference",
            "from_id":"work:current","to_id":"record-version:old","source_refs":[archive,source]}),
        ),
        (
            "rights",
            json!({"rights_id":"rights:one","record_id":"work:current","source_ref":source}),
        ),
        (
            "rights",
            json!({"rights_id":"rights:one","record_id":"work:current","source_ref":source}),
        ),
    ]);
    let mut original = Vec::new();
    let mut ordinal = 0;
    let mut tx = client.transaction().unwrap();
    for (collection, value) in input {
        let (kind, field) = match collection {
            "nodes" => (DurableNavigationBootstrapRowKindV1::Node, "node_id"),
            "edges" => (DurableNavigationBootstrapRowKindV1::Edge, "edge_id"),
            _ => (DurableNavigationBootstrapRowKindV1::Right, "rights_id"),
        };
        let id = value[field].as_str().unwrap().to_owned();
        let raw = serde_json::to_vec(&value).unwrap();
        let (key, ord) = if collection == "rights" {
            let n = ordinal;
            ordinal += 1;
            (format!("{n:020}"), Some(n as i64))
        } else {
            (id.clone(), None)
        };
        tx.execute("INSERT INTO cmd2_projection_navigation_initial_row(domain,consumer_id,collection,row_key,row_id,ordinal,row_json,row_sha256)
            VALUES('lab','initial',$1,$2,$3,$4,$5,$6)", &[&collection, &key, &id, &ord,
                &std::str::from_utf8(&raw).unwrap(), &Digest256::of_bytes(&raw).to_hex()]).unwrap();
        original.push((kind, id, raw));
    }
    let expected =
        crate::source_navigation_bootstrap::normalize_original_fixture(original).unwrap();
    assert!(expected.len() > 300);
    let limits = DurableNavigationEventLimits {
        max_cohort_rows: 32,
        max_current_rows: 128,
        max_dependencies: 16,
        max_row_bytes: 64 * 1024,
        max_total_bytes: 1024 * 1024,
        max_adjacency_bytes: 1024 * 1024,
        ..Default::default()
    };
    let mut position = Position {
        phase: "node_facts".into(),
        ..Default::default()
    };
    tx.execute("INSERT INTO cmd2_projection_navigation_initial VALUES('lab','initial','selection','{}',$1)", &[&position_json(&position).unwrap()]).unwrap();
    tx.commit().unwrap();
    let cancelled = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut pages = 0;
    while position.phase != "normalized" {
        assert!(pages < 100);
        let prepared = position.phase.starts_with("normalize_").then(|| {
            prepare_page(
                &mut client,
                "lab",
                "initial",
                &position,
                limits,
                deadline,
                &cancelled,
            )
            .unwrap()
        });
        let mut tx = client.transaction().unwrap();
        assert_eq!(
            read_position(&mut tx, "lab", "initial", "selection").unwrap(),
            position
        );
        let next = if let Some(mut page) = prepared {
            assert!(page.rows.len() <= 32 && page.work <= limits.max_total_bytes);
            let owners = page
                .paths
                .iter()
                .map(|path| DurableNavigationBootstrapOwnerSourceV1 {
                    source_path: path.clone(),
                    sha256: Digest256::of_bytes(path.as_bytes()),
                    size_bytes: 1,
                })
                .collect::<Vec<_>>();
            persist_page(&mut tx, "lab", "initial", &mut page, &owners, limits).unwrap();
            page.next
        } else {
            metadata_page(
                &mut tx, "lab", "initial", &position, limits, deadline, &cancelled,
            )
            .unwrap()
            .0
        };
        commit_page(
            &mut tx,
            "lab",
            "initial",
            "selection",
            &position,
            &next,
            &[],
            limits,
        )
        .unwrap();
        if pages == 1 {
            tx.rollback().unwrap();
            drop(client);
            client = connect();
            let mut tx = client.transaction().unwrap();
            assert_eq!(
                read_position(&mut tx, "lab", "initial", "selection").unwrap(),
                position
            );
            tx.commit().unwrap();
        } else {
            tx.commit().unwrap();
            position = next;
        }
        // A new connection sees the committed page after a lost response.
        if pages == 8 {
            drop(client);
            client = connect();
            let mut tx = client.transaction().unwrap();
            assert_eq!(
                read_position(&mut tx, "lab", "initial", "selection").unwrap(),
                position
            );
            tx.commit().unwrap();
        }
        pages += 1;
    }
    assert!(pages > 10);
    assert_eq!(position.normalized_rows, expected.len() as u64);
    assert_eq!(
        position.normalized_bytes,
        expected.iter().map(|r| r.raw.len() as u64).sum::<u64>()
    );
    assert!(position.normalized_bytes > limits.max_total_bytes);
    assert_eq!(position.projected_rights, 1);
    let normalized = expected
        .iter()
        .cloned()
        .map(|row| prepare_navigation_bootstrap_row(row, limits).unwrap().0)
        .collect::<Vec<_>>();
    let mut expected_keys = BTreeSet::new();
    let mut expected_deps = BTreeSet::new();
    for row in expected {
        let (row, deps) = prepare_navigation_bootstrap_row(row, limits).unwrap();
        expected_keys.insert((
            row.kind.as_str().to_owned(),
            row.id.clone(),
            row.owner_source_path.clone(),
            row.owner_record_id.clone(),
            row.visible,
            row.from_id,
            row.to_id,
            String::from_utf8(row.raw).unwrap(),
            row.sha256.to_hex(),
        ));
        for dependency in deps {
            expected_deps.insert((
                row.owner_source_path.clone(),
                row.owner_record_id.clone(),
                dependency,
            ));
        }
    }
    let actual = client.query("SELECT row_kind,row_id,owner_source_path,owner_record_id,visible,from_id,to_id,row_json,row_sha256
        FROM cmd2_projection_navigation_bootstrap_row WHERE domain='lab' AND consumer_id='initial'", &[]).unwrap().into_iter()
        .map(|r| (r.get(0),r.get(1),r.get(2),r.get(3),r.get(4),r.get(5),r.get(6),r.get(7),r.get(8))).collect::<BTreeSet<_>>();
    assert_eq!(actual, expected_keys);
    let actual_deps = client.query("SELECT owner_source_path,owner_record_id,dependency_path
        FROM cmd2_projection_navigation_bootstrap_dependency WHERE domain='lab' AND consumer_id='initial'", &[]).unwrap()
        .into_iter().map(|r| (r.get(0),r.get(1),r.get(2))).collect::<BTreeSet<_>>();
    assert_eq!(actual_deps, expected_deps);
    // Exercise the PostgreSQL cursor/key order and exact raw hashing against
    // the established stream framing, including multiple owners for one node.
    let empty_hash = || NavigationBootstrapRootHasher {
        hash: Digest256Hasher::new(),
        last_source: None,
        last_row: None,
    };
    let mut expected_hash = empty_hash();
    let mut ordered = normalized.clone();
    ordered.sort_by(|a, b| {
        (a.kind, &a.id, &a.owner_source_path, &a.owner_record_id).cmp(&(
            b.kind,
            &b.id,
            &b.owner_source_path,
            &b.owner_record_id,
        ))
    });
    for row in &ordered {
        expected_hash.row(row).unwrap();
    }
    let mut actual_hash = empty_hash();
    let mut tx = client.transaction().unwrap();
    let hash_work = super::super::seal::hash_rows(
        &mut tx,
        "lab",
        "initial",
        &mut actual_hash,
        limits,
        deadline,
        &cancelled,
    )
    .unwrap();
    assert_eq!(actual_hash.finish(), expected_hash.finish());
    assert_eq!(hash_work, position.normalized_bytes);
    tx.commit().unwrap();
    // The seal derives its own facts and contributor/dependency sets. A
    // plausible stored position or updated row hash cannot bless corruption.
    let mut tx = client.transaction().unwrap();
    let closed =
        super::super::seal::verify_derived(&mut tx, "lab", "initial", limits, deadline, &cancelled)
            .unwrap();
    assert_eq!(
        (closed.rows, closed.bytes, closed.rights),
        (
            position.normalized_rows,
            position.normalized_bytes,
            position.projected_rights
        )
    );
    tx.commit().unwrap();
    for tamper in [
        "UPDATE cmd2_projection_navigation_initial_row SET node_facts_json='{\"visible\":false,\"current_owner\":null}' WHERE row_id='work:current'",
        "DELETE FROM cmd2_projection_navigation_initial_contributor WHERE node_id='evidence:shared' AND owner_record_id='a'",
        "INSERT INTO cmd2_projection_navigation_initial_contributor VALUES('lab','initial','evidence:shared','ToS/current.json','forged')",
        "UPDATE cmd2_projection_navigation_bootstrap_row SET active=false WHERE row_id='work:current'",
        "UPDATE cmd2_projection_navigation_bootstrap_row SET to_id='work:current' WHERE row_id='edge:a'",
        "UPDATE cmd2_projection_navigation_bootstrap_row SET row_json=row_json || ' ' WHERE row_id='work:current'",
        "DELETE FROM cmd2_projection_navigation_bootstrap_dependency WHERE dependency_path='ToS/archive.json'",
        "INSERT INTO cmd2_projection_navigation_bootstrap_owner VALUES('lab','initial','ToS/forged.json',repeat('0',64),1)",
    ] {
        let mut tx = client.transaction().unwrap();
        assert!(tx.execute(tamper, &[]).unwrap() > 0, "{tamper}");
        assert!(
            super::super::seal::verify_derived(
                &mut tx, "lab", "initial", limits, deadline, &cancelled
            )
            .is_err(),
            "{tamper}"
        );
        tx.rollback().unwrap();
    }
    client.batch_execute("CREATE TABLE cmd2_projection_navigation_adjacency_certificate(
        domain text,consumer_id text,node_id text,edge_count bigint,edge_digest text,certificate_root text,
        PRIMARY KEY(domain,consumer_id,node_id))").unwrap();
    let root = Digest256::of_bytes(b"independently tested adjacency root");
    let reference = bootstrap_adjacency_certificates(root, &normalized, limits).unwrap();
    let expected_adjacency_root = navigation_adjacency_root_initial(root, &reference);
    let mut adjacent = Position {
        phase: "adjacency".into(),
        ..Default::default()
    };
    let mut adjacency_pages = 0;
    while adjacent.phase != "publish" {
        let mut tx = client.transaction().unwrap();
        let (next, _) = super::super::seal::adjacency_page(
            &mut tx, "lab", "initial", &adjacent, root, limits, deadline, &cancelled,
        )
        .unwrap();
        if adjacency_pages == 1 {
            tx.rollback().unwrap();
        } else {
            tx.commit().unwrap();
            adjacent = next;
        }
        adjacency_pages += 1;
        assert!(adjacency_pages < 100);
    }
    assert!(adjacency_pages > 2);
    let mut tx = client.transaction().unwrap();
    assert_eq!(
        super::super::seal::verify_adjacency(
            &mut tx, "lab", "initial", root, limits, deadline, &cancelled
        )
        .unwrap()
        .0,
        expected_adjacency_root
    );
    tx.execute("UPDATE cmd2_projection_navigation_adjacency_certificate SET edge_count=edge_count+1 WHERE node_id='work:current'", &[]).unwrap();
    assert!(
        super::super::seal::verify_adjacency(
            &mut tx, "lab", "initial", root, limits, deadline, &cancelled
        )
        .is_err()
    );
    tx.rollback().unwrap();
    // A changed duplicate rights carrier must not become another owner copy.
    let mut position = Position {
        phase: "normalize_rights".into(),
        ..Default::default()
    };
    let mut page = prepare_page(
        &mut client,
        "lab",
        "initial",
        &position,
        limits,
        deadline,
        &cancelled,
    )
    .unwrap();
    page.rows[0].owner_source_path = "ToS/different.json".into();
    page.rows[0].raw.push(b' ');
    page.rows[0].sha256 = Digest256::of_bytes(&page.rows[0].raw);
    let mut tx = client.transaction().unwrap();
    assert!(persist_page(&mut tx, "lab", "initial", &mut page, &[], limits).is_err());
    tx.rollback().unwrap();
    position.phase = "unknown".into();
    client
        .execute(
            "UPDATE cmd2_projection_navigation_initial SET position_json=$1",
            &[&position_json(&position).unwrap()],
        )
        .unwrap();
    let mut tx = client.transaction().unwrap();
    assert!(read_position(&mut tx, "lab", "initial", "selection").is_err());
    tx.rollback().unwrap();
    let before = client
        .query_one(
            "SELECT count(*) FROM cmd2_projection_navigation_initial_row",
            &[],
        )
        .unwrap()
        .get::<_, i64>(0);
    let mut tx = client.transaction().unwrap();
    assert!(!super::super::seal::cleanup_page(&mut tx, "lab", "initial", limits).unwrap());
    tx.rollback().unwrap();
    assert_eq!(
        client
            .query_one(
                "SELECT count(*) FROM cmd2_projection_navigation_initial_row",
                &[]
            )
            .unwrap()
            .get::<_, i64>(0),
        before
    );
    let mut cleanup_pages = 0;
    loop {
        let mut tx = client.transaction().unwrap();
        let complete = super::super::seal::cleanup_page(&mut tx, "lab", "initial", limits).unwrap();
        tx.commit().unwrap();
        cleanup_pages += 1;
        if complete {
            break;
        }
        assert!(cleanup_pages < 100);
    }
    assert!(cleanup_pages > 2);
    assert_eq!(
        client
            .query_one(
                "SELECT count(*) FROM cmd2_projection_navigation_bootstrap_row",
                &[]
            )
            .unwrap()
            .get::<_, i64>(0),
        normalized.len() as i64
    );
    assert_eq!(
        client
            .query_one(
                "SELECT count(*) FROM cmd2_projection_navigation_initial_row",
                &[]
            )
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    assert_eq!(
        client
            .query_one(
                "SELECT count(*) FROM cmd2_projection_navigation_initial",
                &[]
            )
            .unwrap()
            .get::<_, i64>(0),
        1
    );
    client
        .batch_execute("DROP SCHEMA tos_initial_normalization_test CASCADE")
        .unwrap();
}
