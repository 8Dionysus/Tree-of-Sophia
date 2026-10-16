use rusqlite::Connection;
use std::{path::Path, time::Instant};
use tempfile::tempdir;
use tos_compiler::local_prepared_bulk::{BulkBootstrapLimits, initialize_bulk_with};
use tos_compiler::local_prepared_search::{
    PreparedSearchDocument, SearchChange, apply_delta_with, initialize_with,
};
use tos_foundation::{JsonLimits, JsonMode, JsonValue, parse_json};

fn value(raw: &[u8]) -> JsonValue {
    parse_json(raw, JsonMode::PublishedStrict, JsonLimits::default())
        .unwrap()
        .into_root()
}

fn document(id: u64, kind: &str, raw: &[u8], source_order: u64) -> PreparedSearchDocument {
    PreparedSearchDocument::from_item(id, kind, &value(raw), source_order).unwrap()
}

fn binding(revision: &str) -> JsonValue {
    value(format!(r#"{{"source_revision":"{revision}"}}"#).as_bytes())
}

fn tables(db: &Connection) -> i64 {
    db.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name LIKE 'search_%'",
        [],
        |row| row.get(0),
    )
    .unwrap()
}

fn insert_fixture(
    sink: &mut dyn FnMut(PreparedSearchDocument) -> tos_compiler::Result<()>,
) -> tos_compiler::Result<()> {
    sink(document(
        1,
        "node",
        r#"{"id":"a","native_id":"α","title":"First Évidence","source_graph":"fixture"}"#
            .as_bytes(),
        0,
    ))?;
    sink(document(
        2,
        "node",
        r#"{"id":"b","native_id":"β","title":"Second evidence","source_graph":"fixture"}"#
            .as_bytes(),
        1,
    ))?;
    sink(document(
        3,
        "relation",
        br#"{"id":"e","native_id":"edge-e","label":"Supports evidence","predicate_id":"supports","source_graph":"fixture"}"#,
        2,
    ))
}

#[test]
fn bulk_bootstrap_writes_search_v3_and_leaves_commit_to_owner() {
    let dir = tempdir().unwrap();
    let main = dir.path().join("prepared.sqlite");
    let scratch = dir.path().join("bulk-scratch.sqlite");
    let db = Connection::open(&main).unwrap();
    db.execute_batch("BEGIN IMMEDIATE").unwrap();

    let report = initialize_bulk_with(
        &db,
        &binding("fixture-revision-a"),
        &scratch,
        BulkBootstrapLimits::new(1_048_576, 4096),
        4096,
        1_048_576,
        insert_fixture,
        Some(Instant::now() + std::time::Duration::from_secs(10)),
    )
    .unwrap();

    assert!(
        !db.is_autocommit(),
        "bulk writer must leave owner transaction open"
    );
    assert!(
        !scratch.exists(),
        "successful publication removes private scratch"
    );
    assert_eq!(report.documents, 3);
    assert_eq!(report.high_water, 3);
    assert!(report.reverse_memberships > 0);
    assert_eq!(
        db.query_row("SELECT COUNT(*) FROM search_documents", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        3
    );
    assert_eq!(
        db.query_row("SELECT COUNT(*) FROM search_document_terms", [], |row| row
            .get::<_, i64>(
            0
        ))
        .unwrap(),
        3
    );
    assert_eq!(
        db.query_row("SELECT COUNT(*) FROM search_header", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    let header: String = db
        .query_row(
            "SELECT header FROM search_header WHERE singleton=1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(header.contains(tos_compiler::local_prepared_search::SCHEMA));
    assert!(
        db.query_row("SELECT COUNT(*) FROM search_blocks", [], |row| row
            .get::<_, i64>(0))
            .unwrap()
            > 0
    );

    db.execute_batch("ROLLBACK").unwrap();
    assert_eq!(
        tables(&db),
        0,
        "owner rollback removes every uncommitted table"
    );
}

#[test]
fn bulk_bootstrap_splits_a_513_document_posting_stream_at_the_native_block_boundary() {
    let dir = tempdir().unwrap();
    let main = dir.path().join("prepared.sqlite");
    let scratch = dir.path().join("bulk-scratch.sqlite");
    let db = Connection::open(&main).unwrap();
    db.execute_batch("BEGIN IMMEDIATE").unwrap();
    let report = initialize_bulk_with(
        &db,
        &binding("fixture-revision-513"),
        &scratch,
        BulkBootstrapLimits::new(16_777_216, 1_000_000),
        1_000_000,
        16_777_216,
        |sink| {
            for id in 1..=513 {
                let raw = format!(
                    r#"{{"id":"document-{id}","title":"sharedtoken","source_graph":"fixture"}}"#
                );
                sink(document(id, "node", raw.as_bytes(), id - 1))?;
            }
            Ok(())
        },
        Some(Instant::now() + std::time::Duration::from_secs(30)),
    )
    .unwrap();
    assert_eq!(report.documents, 513);
    assert_eq!(report.high_water, 513);
    assert!(!scratch.exists());
    let common_term: i64 = db
        .query_row(
            "SELECT term_id FROM search_terms WHERE posting_count=513 LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let blocks: (i64, i64, i64) = db
        .query_row(
            "SELECT COUNT(*),MAX(posting_count),SUM(posting_count) FROM search_blocks WHERE term_id=?1",
            [common_term],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(
        blocks,
        (
            3,
            tos_compiler::local_prepared_search::BLOCK_SIZE as i64,
            513
        )
    );
    assert_eq!(
        db.query_row("SELECT COUNT(*) FROM search_documents", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        513,
    );
    db.execute_batch("ROLLBACK").unwrap();
    assert_eq!(
        tables(&db),
        0,
        "the owner still controls the complete bulk transaction"
    );
}

#[test]
fn bulk_producer_failure_cleans_scratch_without_committing_or_closing_owner() {
    let dir = tempdir().unwrap();
    let main = dir.path().join("prepared.sqlite");
    let scratch = dir.path().join("bulk-scratch.sqlite");
    let db = Connection::open(&main).unwrap();
    db.execute_batch("BEGIN IMMEDIATE").unwrap();

    let failure = initialize_bulk_with(
        &db,
        &binding("fixture-revision-b"),
        &scratch,
        BulkBootstrapLimits::new(1_048_576, 4096),
        4096,
        1_048_576,
        |sink| {
            sink(document(
                1,
                "node",
                br#"{"id":"a","title":"staged before failure"}"#,
                0,
            ))?;
            Err(tos_compiler::Error::Invalid(
                "fixture producer interruption",
            ))
        },
        Some(Instant::now() + std::time::Duration::from_secs(10)),
    )
    .unwrap_err();

    assert!(matches!(failure, tos_compiler::Error::Invalid(_)));
    assert!(!db.is_autocommit(), "bulk writer leaves rollback to owner");
    assert!(
        !scratch.exists(),
        "producer failure removes only private scratch"
    );
    assert!(
        tables(&db) > 0,
        "main DDL remains inside caller transaction"
    );
    db.execute_batch("ROLLBACK").unwrap();
    assert_eq!(
        tables(&db),
        0,
        "owner rollback leaves no partial search schema"
    );
}

#[test]
fn invalid_bulk_limits_refuse_before_ddl_or_scratch_creation() {
    let dir = tempdir().unwrap();
    let db = Connection::open(dir.path().join("prepared.sqlite")).unwrap();
    let scratch = dir.path().join("must-not-exist.sqlite");
    db.execute_batch("BEGIN IMMEDIATE").unwrap();
    let mut invalid = BulkBootstrapLimits::new(1_048_576, 4096);
    invalid.max_tail_bytes = 0;

    assert!(
        initialize_bulk_with(
            &db,
            &binding("fixture-revision-c"),
            Path::new(&scratch),
            invalid,
            4096,
            1_048_576,
            |_| panic!("invalid limits must be rejected before producer work"),
            None,
        )
        .is_err()
    );
    assert!(!db.is_autocommit());
    assert!(!scratch.exists());
    assert_eq!(tables(&db), 0);
    db.execute_batch("ROLLBACK").unwrap();
}

#[test]
fn owner_delta_error_rolls_back_forward_and_reverse_search_mutations_together() {
    let dir = tempdir().unwrap();
    let main = dir.path().join("prepared.sqlite");
    let db = Connection::open(&main).unwrap();
    let old = binding("fixture-revision-before");
    let next = binding("fixture-revision-after");
    db.execute_batch("BEGIN IMMEDIATE").unwrap();
    initialize_with(&db, &old, 4096, 1_048_576, insert_fixture).unwrap();
    db.execute_batch("COMMIT").unwrap();

    let old_header: String = db
        .query_row(
            "SELECT header FROM search_header WHERE singleton=1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let old_reverse: (i64, Vec<u8>, Vec<u8>) = db
        .query_row(
            "SELECT term_count,payload,digest FROM search_document_terms WHERE doc_id=1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    db.execute_batch("BEGIN IMMEDIATE").unwrap();
    let error = apply_delta_with(&db, &old, &next, 4096, |sink| {
        sink(SearchChange::Update(document(
            1,
            "node",
            r#"{"id":"a","native_id":"α","title":"Changed after publication","source_graph":"fixture"}"#.as_bytes(),
            0,
        )))?;
        Err(tos_compiler::Error::Invalid("fixture owner abort"))
    })
    .unwrap_err();

    assert!(matches!(error, tos_compiler::Error::Invalid(_)));
    assert!(!db.is_autocommit(), "the owner transaction remains open");
    let changed_reverse: (i64, Vec<u8>, Vec<u8>) = db
        .query_row(
            "SELECT term_count,payload,digest FROM search_document_terms WHERE doc_id=1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_ne!(
        changed_reverse, old_reverse,
        "forward mutation reaches its reverse frame before owner abort"
    );
    db.execute_batch("ROLLBACK").unwrap();

    let reopened = Connection::open(&main).unwrap();
    let restored_header: String = reopened
        .query_row(
            "SELECT header FROM search_header WHERE singleton=1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let restored_reverse: (i64, Vec<u8>, Vec<u8>) = reopened
        .query_row(
            "SELECT term_count,payload,digest FROM search_document_terms WHERE doc_id=1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(restored_header, old_header);
    assert_eq!(restored_reverse, old_reverse);
    assert!(restored_header.contains("fixture-revision-before"));
}
