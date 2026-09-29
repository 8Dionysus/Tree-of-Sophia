//! Native controls for the maintained PublishedSearchTests cases: full rank
//! streams, empty work/long-value progress, deferred bodies and cursor integrity.
//! The tiny normalized carriers are disposable software fixtures, not a corpus.
#![cfg(not(target_arch = "wasm32"))]
use rusqlite::{Connection, OpenFlags};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
use tos_compiler::local_prepared::{
    PreparedReadLimits, PreparedRows, PublicationLimits, publish_prepared_rows,
};
use tos_foundation::{JsonLimits, JsonMode, JsonString, JsonValue, parse_json};
use tos_query::compressed_search::{
    CompressedSearchErrorCode, CompressedSearchRequest, PreparedSearchSession,
    PublishedSearchLimits,
};

fn json(raw: &str) -> JsonValue {
    parse_json(
        raw.as_bytes(),
        JsonMode::PublishedStrict,
        JsonLimits::default(),
    )
    .unwrap()
    .into_root()
}
fn text(raw: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(raw))
}
fn set(value: &mut JsonValue, name: &str, field: JsonValue) {
    if let JsonValue::Object(fields) = value {
        if let Some((_, value)) = fields.iter_mut().find(|(k, _)| k.as_str() == Some(name)) {
            *value = field;
        } else {
            fields.push((JsonString::from_utf8(name), field));
        }
    } else {
        panic!("fixture must be an object")
    }
}
struct Rows {
    nodes: Vec<JsonValue>,
    relations: Vec<JsonValue>,
}
impl PreparedRows for Rows {
    fn visit(
        &mut self,
        kind: &str,
        sink: &mut dyn FnMut(&JsonValue) -> tos_compiler::Result<()>,
    ) -> tos_compiler::Result<()> {
        for row in if kind == "node" {
            &self.nodes
        } else {
            &self.relations
        } {
            sink(row)?;
        }
        Ok(())
    }
}
fn rows() -> Rows {
    Rows {
        nodes: vec![
            json(
                r#"{"id":"a","entity_id":"ea","native_id":"a-native","source_graph":"philosophy","kind_id":"concept","type_id":"concept","display":{"title":{"en":"common"},"summary":{"ru":"Слово"}},"probe":{"false":false,"zero":0,"none":null,"key":"value"}}"#,
            ),
            json(
                r#"{"id":"A","entity_id":"eA","native_id":"A-native","source_graph":"philosophy","kind_id":"concept","type_id":"concept","display":{"title":{"en":"common prefix"}}}"#,
            ),
            json(
                r#"{"id":"b","entity_id":"eb","native_id":"b-native","source_graph":"philosophy","kind_id":"other","type_id":"concept","display":{"title":{"en":"Unicode İ Σ ß é"},"summary":{"en":"visible common"}}}"#,
            ),
            json(
                r#"{"id":"c","entity_id":"ec","native_id":"c-native","source_graph":"philosophy","kind_id":"concept","type_id":"concept","display":{"title":{"en":"else"}},"probe":{"common":false}}"#,
            ),
        ],
        relations: vec![json(
            r#"{"id":"r","native_id":"r-native","source_graph":"philosophy","from_id":"a","to_id":"b","predicate_id":"related_to","relation_type_id":"semantic","display":{"label":{"en":"common"}}}"#,
        )],
    }
}
struct Fixture {
    directory: PathBuf,
    path: PathBuf,
    binding: JsonValue,
    rows: Rows,
}
impl Fixture {
    fn publish(mut rows: Rows, max_row_bytes: usize) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("tos-qry-prepared-{}-{nonce}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        let path = directory.join("published.sqlite");
        let header = json(&format!(
            r#"{{"schema":"tos_knowledge_graph_v1","source_revision":"{}","normalization_binding":{{"schema":"tos_knowledge_graph_normalization_binding_v1","processor_digest":"{}","entity_registry_digest":"{}","relation_registry_digest":"{}","configuration_digest":"{}"}},"authority_boundary":{{"source_owner":"Tree-of-Sophia","is_source":false,"is_canon":false,"writes_to_tree":false}},"query_properties":[]}}"#,
            "a".repeat(64),
            "b".repeat(64),
            "b".repeat(64),
            "b".repeat(64),
            "b".repeat(64)
        ));
        let catalog = json(&format!(
            r#"{{"schema":"tos_knowledge_catalog_v1","source_revision":"{}","lenses":[]}}"#,
            "a".repeat(64)
        ));
        let binding = publish_prepared_rows(
            &path,
            &header,
            &catalog,
            &mut rows,
            PublicationLimits {
                max_row_bytes,
                ..Default::default()
            },
        )
        .unwrap();
        Self {
            directory,
            path,
            binding,
            rows,
        }
    }
    fn page(
        &self,
        request: CompressedSearchRequest,
        read_limits: PreparedReadLimits,
        search_limits: PublishedSearchLimits,
        now: u64,
    ) -> Result<JsonValue, tos_query::compressed_search::CompressedSearchError> {
        let db = Connection::open_with_flags(&self.path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        db.execute_batch("PRAGMA query_only=ON;BEGIN").unwrap();
        let mut session = PreparedSearchSession::new(&db, read_limits)?;
        let page = session.search(&self.binding, request, search_limits, now)?;
        // Actual caller's second snapshot uses the retained request meter.
        db.execute_batch("COMMIT;BEGIN").unwrap();
        session.recheck_binding(&self.binding)?;
        db.execute_batch("COMMIT").unwrap();
        Ok(page)
    }
    fn drain(
        &self,
        query: &str,
        limit: usize,
        read_limits: PreparedReadLimits,
        search_limits: PublishedSearchLimits,
    ) -> (Vec<JsonValue>, Vec<JsonValue>, Vec<JsonValue>) {
        let (mut cursor, mut nodes, mut relations, mut pages) =
            (None, Vec::new(), Vec::new(), Vec::new());
        for _ in 0..2000 {
            let page = self
                .page(
                    CompressedSearchRequest {
                        query: query.to_owned(),
                        limit,
                        cursor: cursor.clone(),
                        ..Default::default()
                    },
                    read_limits,
                    search_limits,
                    100,
                )
                .unwrap();
            nodes.extend(
                page.object_get("nodes")
                    .unwrap()
                    .as_array()
                    .unwrap()
                    .iter()
                    .cloned(),
            );
            relations.extend(
                page.object_get("relations")
                    .unwrap()
                    .as_array()
                    .unwrap()
                    .iter()
                    .cloned(),
            );
            if cursor.is_some() {
                assert!(
                    page.object_get("counts")
                        .unwrap()
                        .object_get("matching_nodes")
                        .unwrap()
                        .is_null()
                );
            }
            let next = page
                .object_get("page")
                .unwrap()
                .object_get("next_cursor")
                .unwrap()
                .as_str()
                .map(str::to_owned);
            let more = page
                .object_get("page")
                .unwrap()
                .object_get("has_more")
                .unwrap()
                .as_bool()
                .unwrap();
            if more {
                assert_ne!(cursor, next);
                assert!(next.as_ref().unwrap().len() <= 65_536);
            }
            pages.push(page);
            if !more {
                return (nodes, relations, pages);
            }
            cursor = next;
        }
        panic!("bounded fixture did not drain")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.directory).unwrap();
    }
}
fn read_limits(max_row_bytes: usize) -> PreparedReadLimits {
    PreparedReadLimits {
        max_row_bytes,
        max_response_bytes: 4 * 1024 * 1024,
        ..Default::default()
    }
}

#[test]
fn complete_four_rank_stream_and_short_unicode_filters() {
    let f = Fixture::publish(rows(), 1_048_576);
    let (nodes, relations, pages) =
        f.drain("common", 2, read_limits(1_048_576), Default::default());
    assert_eq!(nodes, f.rows.nodes);
    assert_eq!(relations, f.rows.relations);
    let ranks: Vec<u64> = pages
        .iter()
        .flat_map(|p| {
            p.object_get("ranks")
                .unwrap()
                .object_get("nodes")
                .unwrap()
                .as_array()
                .unwrap()
        })
        .map(|r| r.object_get("rank").unwrap().as_u64().unwrap())
        .collect();
    assert_eq!(ranks, [0, 1, 2, 3]);
    for query in [
        "",
        "a",
        "A-native",
        "false",
        "Слово",
        "İ",
        "ß",
        "e\u{301}",
        "no-match",
    ] {
        let (nodes, _, _) = f.drain(query, 2, read_limits(1_048_576), Default::default());
        if query == "" {
            assert_eq!(nodes, f.rows.nodes);
        } else if query == "a" {
            assert_eq!(nodes, f.rows.nodes);
        } else if query == "A-native" {
            assert_eq!(nodes, f.rows.nodes[..2]);
        } else if ["Слово", "false"].contains(&query) {
            assert_eq!(
                nodes,
                [f.rows.nodes[0].clone(), f.rows.nodes[3].clone()]
                    .into_iter()
                    .filter(
                        |n| query == "false" || n.object_get("id").unwrap().as_str() == Some("a")
                    )
                    .collect::<Vec<_>>()
            );
        } else if ["İ", "ß", "e\u{301}"].contains(&query) {
            assert_eq!(nodes, [f.rows.nodes[2].clone()]);
        } else if query == "no-match" {
            assert!(nodes.is_empty());
        }
    }
    let page = f
        .page(
            CompressedSearchRequest {
                query: "common".into(),
                kind_ids: vec!["other".into(), String::new()],
                ..Default::default()
            },
            read_limits(1_048_576),
            Default::default(),
            100,
        )
        .unwrap();
    assert_eq!(
        page.object_get("nodes").unwrap().as_array().unwrap(),
        [f.rows.nodes[2].clone()]
    );
    assert_eq!(
        page.object_get("relations").unwrap().as_array().unwrap(),
        f.rows.relations
    );
}

#[test]
fn empty_work_pages_resume_large_value_and_preserve_full_body() {
    let mut r = rows();
    set(
        &mut r.nodes[0],
        "wide",
        text(&("x".repeat(100_000) + "needle")),
    );
    set(
        &mut r.nodes[1],
        "wide",
        text(&("x".repeat(100_000) + "needXle")),
    );
    let f = Fixture::publish(r, 1_048_576);
    let (nodes, relations, pages) = f.drain(
        "needle",
        2,
        read_limits(1_048_576),
        PublishedSearchLimits {
            candidate_budget: 2,
            verification_bytes: 8192,
            ..Default::default()
        },
    );
    assert_eq!(nodes, [f.rows.nodes[0].clone()]);
    assert!(relations.is_empty());
    assert!(
        pages
            .iter()
            .filter(|p| p
                .object_get("nodes")
                .unwrap()
                .as_array()
                .unwrap()
                .is_empty()
                && p.object_get("page")
                    .unwrap()
                    .object_get("has_more")
                    .unwrap()
                    .as_bool()
                    == Some(true))
            .count()
            > 10
    );
}

#[test]
fn pending_suffix_drains_without_repeating_inner_search() {
    let mut r = rows();
    let base = r.nodes[0].clone();
    r.nodes = (0..5)
        .map(|i| {
            let mut n = base.clone();
            set(
                &mut n,
                "id",
                text(&((b'a' + i) as char).to_string().repeat(4096)),
            );
            set(&mut n, "entity_id", text(&format!("e{i}")));
            set(&mut n, "native_id", text(&format!("n{i}")));
            set(&mut n, "wide", text(&"x".repeat(85000)));
            n
        })
        .collect();
    r.relations.clear();
    let f = Fixture::publish(r, 131072);
    let (nodes, _, pages) = f.drain(
        "",
        5,
        read_limits(131072),
        PublishedSearchLimits {
            body_bytes: 262144,
            ..Default::default()
        },
    );
    assert_eq!(nodes, f.rows.nodes);
    assert_eq!(
        pages[0]
            .object_get("nodes")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(pages.len() >= 5);
    for page in pages.iter().take(5).skip(1) {
        assert_eq!(
            page.object_get("work")
                .unwrap()
                .object_get("nodes")
                .unwrap()
                .object_get("inner_pages")
                .unwrap()
                .as_u64(),
            Some(0)
        );
    }
}

#[test]
fn cursor_restart_query_expiry_and_same_binding_aba() {
    let f = Fixture::publish(rows(), 1_048_576);
    let first = f
        .page(
            CompressedSearchRequest {
                limit: 1,
                ..Default::default()
            },
            read_limits(1_048_576),
            Default::default(),
            100,
        )
        .unwrap();
    let cursor = first
        .object_get("page")
        .unwrap()
        .object_get("next_cursor")
        .unwrap()
        .as_str()
        .unwrap()
        .to_owned();
    let continued = f
        .page(
            CompressedSearchRequest {
                limit: 1,
                cursor: Some(cursor.clone()),
                ..Default::default()
            },
            read_limits(1_048_576),
            Default::default(),
            100,
        )
        .unwrap();
    assert_eq!(
        continued.object_get("nodes").unwrap().as_array().unwrap(),
        [f.rows.nodes[1].clone()]
    );
    assert_eq!(
        f.page(
            CompressedSearchRequest {
                query: "other".into(),
                cursor: Some(cursor.clone()),
                ..Default::default()
            },
            read_limits(1_048_576),
            Default::default(),
            100
        )
        .unwrap_err()
        .code,
        CompressedSearchErrorCode::CursorInvalid
    );
    assert_eq!(
        f.page(
            CompressedSearchRequest {
                cursor: Some(cursor.clone()),
                ..Default::default()
            },
            read_limits(1_048_576),
            Default::default(),
            1000
        )
        .unwrap_err()
        .code,
        CompressedSearchErrorCode::CursorExpired
    );
    Connection::open(&f.path)
        .unwrap()
        .execute("UPDATE search_header SET cursor_key=randomblob(32)", [])
        .unwrap();
    assert_eq!(
        f.page(
            CompressedSearchRequest {
                cursor: Some(cursor),
                ..Default::default()
            },
            read_limits(1_048_576),
            Default::default(),
            100
        )
        .unwrap_err()
        .code,
        CompressedSearchErrorCode::CursorInvalid
    );
}

#[test]
fn prepared_inspect_alias_endpoints_counts_and_selected_corruption_use_shared_meter() {
    use tos_query::search_v2::{SearchKind, SearchV2ErrorCode};
    let mut input = rows();
    set(&mut input.nodes[1], "entity_id", text("ea"));
    set(&mut input.nodes[0], "native_id", text("shared-native"));
    set(&mut input.nodes[1], "native_id", text("shared-native"));
    let f = Fixture::publish(input, 1_048_576);
    let db = Connection::open_with_flags(&f.path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    db.execute_batch("PRAGMA query_only=ON;BEGIN").unwrap();
    let limits = PreparedReadLimits {
        max_response_bytes: 4 * 1024 * 1024,
        ..Default::default()
    };
    let mut session = PreparedSearchSession::new(&db, limits).unwrap();
    let p = session
        .inspect(&f.binding, SearchKind::Nodes, "ea", 0)
        .unwrap();
    assert_eq!(
        p.object_get("shared_entity_id"),
        Some(&JsonValue::Bool(true))
    );
    assert_eq!(
        p.object_get("matches").unwrap().as_array().unwrap().len(),
        2
    );
    assert_eq!(
        p.object_get("related_relations")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        0
    );
    assert_eq!(
        p.object_get("counts")
            .unwrap()
            .object_get("related_relations"),
        Some(&json("1"))
    );
    let p = session
        .inspect(&f.binding, SearchKind::Nodes, "shared-native", 1)
        .unwrap();
    assert_eq!(
        p.object_get("ambiguous_native_id"),
        Some(&JsonValue::Bool(true))
    );
    assert_eq!(
        p.object_get("related_relations")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let p = session
        .inspect(&f.binding, SearchKind::Relations, "r-native", 1)
        .unwrap();
    let endpoints = p.object_get("endpoints").unwrap().as_array().unwrap();
    assert_eq!(
        endpoints
            .iter()
            .map(|v| v.object_get("id").unwrap().as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["a", "b"]
    );
    assert_eq!(
        endpoints[0]
            .object_get("probe")
            .unwrap()
            .object_get("false"),
        Some(&JsonValue::Bool(false))
    );
    assert_eq!(
        endpoints[0].object_get("probe").unwrap().object_get("zero"),
        Some(&json("0"))
    );
    assert_eq!(
        session
            .inspect(&f.binding, SearchKind::Nodes, "absent", 1)
            .unwrap_err()
            .code,
        SearchV2ErrorCode::UnknownIdentifier
    );
    db.execute_batch("COMMIT;BEGIN").unwrap();
    session.recheck_binding(&f.binding).unwrap();
    db.execute_batch("COMMIT").unwrap();
    drop(session);
    drop(db);

    // The exact selected row digest and indexed projection must both close.
    for sql in [
        "UPDATE edge_meta SET json_chunk='{\"sha256\":\"bad\"}' WHERE key='knowledge_node_digest:a'",
        "UPDATE knowledge_nodes SET kind_id='changed' WHERE id='a'",
        "DELETE FROM knowledge_nodes WHERE id='b'",
    ] {
        let f = Fixture::publish(rows(), 1_048_576);
        let db = Connection::open(&f.path).unwrap();
        db.execute_batch(sql).unwrap();
        db.execute_batch("BEGIN").unwrap();
        let mut session = PreparedSearchSession::new(&db, limits).unwrap();
        assert_eq!(
            session
                .inspect(&f.binding, SearchKind::Relations, "r", 1)
                .unwrap_err()
                .code,
            SearchV2ErrorCode::CorruptSelectedCarrier
        );
        db.execute_batch("ROLLBACK").unwrap();
    }
    let f = Fixture::publish(rows(), 1_048_576);
    let db = Connection::open(&f.path).unwrap();
    db.execute_batch("BEGIN").unwrap();
    let mut session = PreparedSearchSession::new(
        &db,
        PreparedReadLimits {
            max_bytes: 1,
            ..limits
        },
    )
    .unwrap();
    assert_eq!(
        session
            .inspect(&f.binding, SearchKind::Nodes, "a", 1)
            .unwrap_err()
            .code,
        SearchV2ErrorCode::BudgetExceeded
    );
    db.execute_batch("ROLLBACK").unwrap();
}
