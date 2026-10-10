//! Regression coverage for original-row verification under a V4 carrier.
//!
//! These tests run in a child process because the dedicated SQLite heap is
//! process-global. The optional producer-row probe is ignored by default and
//! reads only the exact raw bytes named by `TOS_ORIGINAL_VERIFY_REAL_ROW_FILE`.
use super::*;
use crate::knowledge_payload_read::RuntimeKnowledgeOwnedBudget;
use rusqlite::{Connection, params};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64},
};
use std::time::{Duration, Instant};

const CHILD: &str = "TOS_ORIGINAL_STAGE_BUDGET_TEST_CHILD";
const NODE_COUNT: usize = 4096;
const NODE_PAYLOAD_BYTES: usize = 900;
const WORK_LIMIT: u64 = 2 * 1024 * 1024 * 1024;
const SQLITE_HEAP_BYTES: usize = 32 * 1024 * 1024;
const RETAINED_BYTES: usize = 128 * 1024 * 1024;
const LAYOUT: KnowledgePayloadLayout = KnowledgePayloadLayout::CarrierOnceV4;

#[derive(Debug, PartialEq, Eq)]
struct DatabaseSnapshot {
    rows: Vec<(String, i64, String, i64, Vec<u8>)>,
    carriers: Vec<(Vec<u8>, i64, Vec<u8>)>,
    dictionaries: Vec<(Vec<u8>, Vec<u8>)>,
}

fn enter_isolated_test(test_name: &str, include_ignored: bool) -> bool {
    if std::env::var_os(CHILD).is_some() {
        return true;
    }
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", test_name, "--nocapture"])
        .env(CHILD, "1");
    if include_ignored {
        command.arg("--ignored");
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    eprint!("{}", String::from_utf8_lossy(&output.stderr));
    false
}

fn install_schema(db: &Connection) {
    db.execute_batch(crate::knowledge_byte_dictionary::ddl(LAYOUT))
        .unwrap();
    db.execute_batch(crate::knowledge_stage::SOURCE_CARRIER_DDL)
        .unwrap();
    db.execute_batch(ROW_DDL_CARRIER).unwrap();
}

fn store_dictionary(db: &Connection, state: &CreationState<'_>, bytes: &[u8]) -> Vec<u8> {
    let digest = Digest256::of_bytes(bytes);
    let mut stored = Vec::new();
    crate::knowledge_byte_codec::with_encoded(state, bytes, bytes.len(), |frame| {
        db.execute(
            "INSERT INTO knowledge_byte_dictionaries(dictionary_sha256,dictionary) VALUES (?1,?2)",
            params![digest.as_bytes().as_slice(), frame],
        )?;
        stored.extend_from_slice(frame);
        Ok(())
    })
    .unwrap();
    stored
}

fn encode_packet(state: &CreationState<'_>, dictionary: &[u8], raw: &[u8]) -> Result<Vec<u8>> {
    let verified =
        crate::knowledge_byte_dictionary::VerifiedDictionary::from_bytes(dictionary, |bytes| {
            state.charge_work(bytes)
        })?;
    let mut stored = Vec::new();
    LAYOUT.with_encoded_dictionary(state, Some(verified), raw, raw.len().max(1), |frame| {
        stored.extend_from_slice(frame);
        Ok(())
    })?;
    Ok(stored)
}

fn insert_packet(
    db: &Connection,
    state: &CreationState<'_>,
    dictionary: &[u8],
    collection: &str,
    ordinal: i64,
    id: &str,
    raw: &[u8],
    require_dictionary_frame: bool,
) -> Result<Vec<u8>> {
    let digest = Digest256::of_bytes(raw);
    let stored = encode_packet(state, dictionary, raw)?;
    if require_dictionary_frame && !crate::knowledge_byte_codec::is_dictionary_frame(&stored) {
        return Err(Error::Invalid(
            "test packet did not use V4 dictionary frame",
        ));
    }
    db.execute(
        "INSERT INTO knowledge_source_carriers(packet_sha256,packet_len,packet) VALUES (?1,?2,?3)",
        params![
            digest.as_bytes().as_slice(),
            raw.len() as i64,
            stored.as_slice()
        ],
    )?;
    db.execute(
        "INSERT INTO philosophy_original_rows(collection,ordinal,id,packet_len,packet_sha256) VALUES (?1,?2,?3,?4,?5)",
        params![collection, ordinal, id, raw.len() as i64, digest.as_bytes().as_slice()],
    )?;
    Ok(stored)
}

fn snapshot(db: &Connection) -> DatabaseSnapshot {
    let mut statement = db
        .prepare(
            "SELECT collection,ordinal,id,packet_len,packet_sha256 FROM philosophy_original_rows ORDER BY collection,ordinal",
        )
        .unwrap();
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    let mut statement = db
        .prepare(
            "SELECT packet_sha256,packet_len,packet FROM knowledge_source_carriers ORDER BY packet_sha256,packet_len",
        )
        .unwrap();
    let carriers = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    let mut statement = db
        .prepare(
            "SELECT dictionary_sha256,dictionary FROM knowledge_byte_dictionaries ORDER BY dictionary_sha256",
        )
        .unwrap();
    let dictionaries = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    DatabaseSnapshot {
        rows,
        carriers,
        dictionaries,
    }
}

fn limits() -> NavigationOriginalLimits {
    NavigationOriginalLimits {
        max_rows: crate::knowledge_original_rows::MAX_ROWS,
        max_row_bytes: crate::knowledge_original_rows::MAX_ROW_BYTES,
        max_total_bytes: crate::knowledge_original_rows::MAX_TOTAL_BYTES,
    }
}

fn test_node(id: &str) -> Vec<u8> {
    format!(
        "{{\"node_id\":\"{id}\",\"payload\":\"{}\"}}",
        "x".repeat(NODE_PAYLOAD_BYTES)
    )
    .into_bytes()
}

fn make_receipt(
    header_raw: &[u8],
    node_root: &str,
    node_input_root: &str,
    total_bytes: u64,
) -> PhilosophyOriginalReceipt {
    let mut receipt = PhilosophyOriginalReceipt {
        profile: PHILOSOPHY_ORIGINAL_PROFILE.into(),
        descriptor_sha256: "a".repeat(64),
        source_cut: "cut:owned-v4-budget-test".into(),
        membership_root: "b".repeat(64),
        source_graph: "fixture.graph".into(),
        nodes: NODE_COUNT as u64,
        edges: 0,
        node_input_root_sha256: node_input_root.into(),
        edge_input_root_sha256: Digest256Hasher::new().finalize().to_hex(),
        header_sha256: Digest256::of_bytes(header_raw).to_hex(),
        nodes_root_sha256: node_root.into(),
        edges_root_sha256: ordered_hash("edges").finalize().to_hex(),
        component_root_sha256: String::new(),
        total_bytes,
    };
    receipt.component_root_sha256 = root(&receipt).unwrap();
    receipt
}

#[test]
fn owned_v4_original_verification_uses_the_same_finite_budget_and_preserves_rows() {
    const TEST: &str = "knowledge_philosophy_original::stage_budget_tests::owned_v4_original_verification_uses_the_same_finite_budget_and_preserves_rows";
    if !enter_isolated_test(TEST, false) {
        return;
    }

    let deadline = Instant::now() + Duration::from_secs(300);
    let cancelled = Arc::new(AtomicBool::new(false));
    let remaining = |bytes: usize| {
        RETAINED_BYTES
            .checked_sub(bytes)
            .ok_or(Error::Budget("original stage budget test retained bytes"))
    };
    let heap = crate::sqlite_budget::DedicatedSessionSqliteHeap::establish(
        SQLITE_HEAP_BYTES,
        &remaining,
        deadline,
        &cancelled,
    )
    .unwrap();
    let work = Arc::new(AtomicU64::new(0));
    let vm = Arc::new(AtomicU64::new(0));
    let budget = RuntimeKnowledgeOwnedBudget {
        remaining_after_retained: &remaining,
        original_work: &work,
        original_work_limit: WORK_LIMIT,
        original_sql_vm: &vm,
        original_sql_vm_limit: 10_000_000,
        original_sqlite_heap: &heap,
        remaining_json_visits: 1_000_000,
        owner_deadline: deadline,
        operation_deadline: deadline,
        cancelled: &cancelled,
    };
    let state = CreationState::from_runtime_owned_budget(&budget).unwrap();
    let db = Connection::open_in_memory().unwrap();
    install_schema(&db);
    let dictionary = vec![b'x'; crate::knowledge_byte_codec::DICTIONARY_WINDOW_BYTES];
    let dictionary_stored = store_dictionary(&db, &state, &dictionary);

    let header =
        format!("{{\"schema_version\":\"v1\",\"counts\":{{\"nodes\":{NODE_COUNT},\"edges\":0}}}}")
            .into_bytes();
    insert_packet(&db, &state, &dictionary, "header", 0, "", &header, false).unwrap();
    let mut node_hash = ordered_hash("nodes");
    let mut input_hash = Digest256Hasher::new();
    let mut total_bytes = header.len() as u64;
    let mut first_node_raw = Vec::new();
    let mut first_node_frame = Vec::new();
    for ordinal in 0..NODE_COUNT {
        let id = format!("node-{ordinal:05}");
        let raw = test_node(&id);
        let digest = Digest256::of_bytes(&raw);
        ordered_item(&mut node_hash, ordinal as u64, &raw);
        text(&mut input_hash, &id);
        input_hash.update(digest.as_bytes());
        if ordinal == 0 {
            first_node_raw = raw.clone();
        }
        let frame = insert_packet(
            &db,
            &state,
            &dictionary,
            "nodes",
            ordinal as i64,
            &id,
            &raw,
            true,
        )
        .unwrap();
        if ordinal == 0 {
            first_node_frame = frame;
        }
        total_bytes += raw.len() as u64;
    }
    let receipt = make_receipt(
        &header,
        &node_hash.finalize().to_hex(),
        &input_hash.finalize().to_hex(),
        total_bytes,
    );
    let before = snapshot(&db);

    // The ordinary offline ceiling is too small for the work actually needed
    // to decode and authenticate one 32 KiB dictionary for each source row.
    let mut legacy_unowned_work = 0;
    assert!(matches!(
        verify_rows_with_state(
            &db,
            &receipt,
            limits(),
            &mut legacy_unowned_work,
            crate::knowledge_original_rows::MAX_COLD_WORK,
            None,
            LAYOUT,
        ),
        Err(Error::Budget(_))
    ));
    assert!(legacy_unowned_work <= crate::knowledge_original_rows::MAX_COLD_WORK);
    assert_eq!(snapshot(&db), before);

    let mut unowned_work = 0;
    verify_rows_with_state(
        &db,
        &receipt,
        limits(),
        &mut unowned_work,
        WORK_LIMIT,
        None,
        LAYOUT,
    )
    .unwrap();
    assert!(unowned_work > crate::knowledge_original_rows::MAX_COLD_WORK);
    eprintln!(
        "V4 original rows: unowned_decode_work={} legacy_cap={} owned_work_cap={}",
        unowned_work,
        crate::knowledge_original_rows::MAX_COLD_WORK,
        WORK_LIMIT - state.observed_work_bytes()
    );
    assert_eq!(snapshot(&db), before);

    // The selected owner admits the same verification against its original,
    // finite counter; the proof is the counter movement, not a copied cap.
    let work_before_owned = state.observed_work_bytes();
    let mut owned_work = 0;
    let owned_cap = state.remaining_work_bytes().unwrap();
    verify_rows_with_state(
        &db,
        &receipt,
        limits(),
        &mut owned_work,
        owned_cap,
        Some(&state),
        LAYOUT,
    )
    .unwrap();
    let work_after_owned = state.observed_work_bytes();
    let owned_delta = work_after_owned - work_before_owned;
    assert!(owned_delta > 0);
    assert!(work_after_owned < WORK_LIMIT);
    eprintln!(
        "V4 owned verification: original_work_before={} original_work_after={} owned_work_delta={}",
        work_before_owned, work_after_owned, owned_delta
    );
    assert_eq!(snapshot(&db), before);

    // A valid V2 frame with different bytes under the old digest must fail the
    // V4 dictionary identity check, then the exact stored bytes are restored.
    let other_dictionary = vec![b'y'; dictionary.len()];
    let corrupt_dictionary = {
        let mut stored = Vec::new();
        crate::knowledge_byte_codec::with_encoded(
            &state,
            &other_dictionary,
            other_dictionary.len(),
            |frame| {
                stored.extend_from_slice(frame);
                Ok(())
            },
        )
        .unwrap();
        stored
    };
    let dictionary_digest = Digest256::of_bytes(&dictionary);
    db.execute(
        "UPDATE knowledge_byte_dictionaries SET dictionary=?1 WHERE dictionary_sha256=?2",
        params![
            corrupt_dictionary.as_slice(),
            dictionary_digest.as_bytes().as_slice()
        ],
    )
    .unwrap();
    let mut corrupt_work = 0;
    let err = verify_rows_with_state(
        &db,
        &receipt,
        limits(),
        &mut corrupt_work,
        state.remaining_work_bytes().unwrap(),
        Some(&state),
        LAYOUT,
    )
    .unwrap_err();
    assert!(matches!(
        err,
        Error::Invalid("V4 dictionary digest differs")
    ));
    db.execute(
        "UPDATE knowledge_byte_dictionaries SET dictionary=?1 WHERE dictionary_sha256=?2",
        params![
            dictionary_stored.as_slice(),
            dictionary_digest.as_bytes().as_slice()
        ],
    )
    .unwrap();
    assert_eq!(snapshot(&db), before);

    // A valid V4 frame that decodes to changed raw bytes must fail the row
    // digest check even though the carrier key and declared size are intact.
    let mut changed_node = first_node_raw.clone();
    let payload_byte = changed_node.len() - 3;
    assert_eq!(changed_node[payload_byte], b'x');
    changed_node[payload_byte] = b'y';
    let changed_frame = encode_packet(&state, &dictionary, &changed_node).unwrap();
    assert!(crate::knowledge_byte_codec::is_dictionary_frame(
        &changed_frame
    ));
    let first_digest = Digest256::of_bytes(&first_node_raw);
    db.execute(
        "UPDATE knowledge_source_carriers SET packet=?1 WHERE packet_sha256=?2 AND packet_len=?3",
        params![
            changed_frame.as_slice(),
            first_digest.as_bytes().as_slice(),
            first_node_raw.len() as i64
        ],
    )
    .unwrap();
    let mut corrupt_work = 0;
    let err = verify_rows_with_state(
        &db,
        &receipt,
        limits(),
        &mut corrupt_work,
        state.remaining_work_bytes().unwrap(),
        Some(&state),
        LAYOUT,
    )
    .unwrap_err();
    assert!(matches!(
        err,
        Error::Invalid("philosophy original row digest differs")
    ));
    db.execute(
        "UPDATE knowledge_source_carriers SET packet=?1 WHERE packet_sha256=?2 AND packet_len=?3",
        params![
            first_node_frame.as_slice(),
            first_digest.as_bytes().as_slice(),
            first_node_raw.len() as i64
        ],
    )
    .unwrap();
    assert_eq!(snapshot(&db), before);

    // Keep the same original atomic, but give a second operation only half the
    // work consumed by the successful verification. The owner itself must
    // stop and leave its SQL-backed source rows untouched.
    let current_work = state.observed_work_bytes();
    let exhausted_limit = current_work + (owned_delta / 2).max(1);
    let exhausted_budget = RuntimeKnowledgeOwnedBudget {
        remaining_after_retained: &remaining,
        original_work: &work,
        original_work_limit: exhausted_limit,
        original_sql_vm: &vm,
        original_sql_vm_limit: 10_000_000,
        original_sqlite_heap: &heap,
        remaining_json_visits: 1_000_000,
        owner_deadline: deadline,
        operation_deadline: deadline,
        cancelled: &cancelled,
    };
    let exhausted_state = CreationState::from_runtime_owned_budget(&exhausted_budget).unwrap();
    let mut exhausted_local_work = 0;
    let err = verify_rows_with_state(
        &db,
        &receipt,
        limits(),
        &mut exhausted_local_work,
        exhausted_state.remaining_work_bytes().unwrap(),
        Some(&exhausted_state),
        LAYOUT,
    )
    .unwrap_err();
    assert!(matches!(err, Error::Budget(_)));
    assert!(exhausted_state.observed_work_bytes() > current_work);
    assert!(exhausted_state.observed_work_bytes() <= exhausted_limit);
    assert_eq!(snapshot(&db), before);
    assert!(Instant::now() < deadline);
}

fn exact_real_row(path: &std::path::Path) -> Vec<u8> {
    let metadata = std::fs::symlink_metadata(path).unwrap();
    assert!(metadata.file_type().is_file() && !metadata.file_type().is_symlink());
    assert!(metadata.len() > 0);
    assert!(metadata.len() <= crate::knowledge_original_rows::MAX_ROW_BYTES as u64);
    let raw = std::fs::read(path).unwrap();
    assert_eq!(raw.len() as u64, metadata.len());
    raw
}

#[test]
#[ignore = "set TOS_ORIGINAL_VERIFY_REAL_ROW_FILE to an exact producer row and run explicitly"]
fn real_producer_row_uses_dictionary_aware_page_work_cap() {
    const TEST: &str = "knowledge_philosophy_original::stage_budget_tests::real_producer_row_uses_dictionary_aware_page_work_cap";
    if !enter_isolated_test(TEST, true) {
        return;
    }
    let path = std::env::var_os("TOS_ORIGINAL_VERIFY_REAL_ROW_FILE")
        .map(std::path::PathBuf::from)
        .expect("TOS_ORIGINAL_VERIFY_REAL_ROW_FILE must name the exact raw row");
    let raw = exact_real_row(&path);
    let deadline = Instant::now() + Duration::from_secs(60);
    let cancelled = Arc::new(AtomicBool::new(false));
    let remaining = |bytes: usize| {
        RETAINED_BYTES
            .checked_sub(bytes)
            .ok_or(Error::Budget("real-row page probe retained bytes"))
    };
    let heap = crate::sqlite_budget::DedicatedSessionSqliteHeap::establish(
        SQLITE_HEAP_BYTES,
        &remaining,
        deadline,
        &cancelled,
    )
    .unwrap();
    let work = Arc::new(AtomicU64::new(0));
    let vm = Arc::new(AtomicU64::new(0));
    let budget = RuntimeKnowledgeOwnedBudget {
        remaining_after_retained: &remaining,
        original_work: &work,
        original_work_limit: WORK_LIMIT,
        original_sql_vm: &vm,
        original_sql_vm_limit: 10_000_000,
        original_sqlite_heap: &heap,
        remaining_json_visits: 1_000_000,
        owner_deadline: deadline,
        operation_deadline: deadline,
        cancelled: &cancelled,
    };
    let state = CreationState::from_runtime_owned_budget(&budget).unwrap();
    let db = Connection::open_in_memory().unwrap();
    install_schema(&db);
    let mut dictionary = Vec::with_capacity(crate::knowledge_byte_codec::DICTIONARY_WINDOW_BYTES);
    while dictionary.len() < crate::knowledge_byte_codec::DICTIONARY_WINDOW_BYTES {
        let take = raw
            .len()
            .min(crate::knowledge_byte_codec::DICTIONARY_WINDOW_BYTES - dictionary.len());
        dictionary.extend_from_slice(&raw[..take]);
    }
    store_dictionary(&db, &state, &dictionary);
    let stored = insert_packet(
        &db,
        &state,
        &dictionary,
        "nodes",
        0,
        "real-producer-row",
        &raw,
        true,
    )
    .unwrap();
    let db_snapshot = snapshot(&db);
    assert!(crate::knowledge_byte_codec::is_dictionary_frame(&stored));

    // The production page helper must admit this exact row using the new
    // dictionary-aware envelope while keeping its logical page at raw.len().
    let page = page_with_layout(
        &db,
        PhilosophyOriginalCollection::Nodes,
        None,
        1,
        raw.len(),
        raw.len() as u64,
        LAYOUT,
    )
    .unwrap();
    assert_eq!(page.rows.len(), 1);
    assert_eq!(page.rows[0].raw, raw);
    assert_eq!(snapshot(&db), db_snapshot);

    let legacy_cap = (raw.len() as u64).saturating_mul(16);
    let mut measured_work = 0;
    let measured = page_with_layout_and_work(
        &db,
        PhilosophyOriginalCollection::Nodes,
        None,
        1,
        raw.len(),
        raw.len() as u64,
        LAYOUT,
        &mut measured_work,
        WORK_LIMIT,
    )
    .unwrap();
    assert_eq!(measured.rows[0].raw, raw);
    assert!(measured_work > legacy_cap);
    let mut legacy_work = 0;
    assert!(matches!(
        page_with_layout_and_work(
            &db,
            PhilosophyOriginalCollection::Nodes,
            None,
            1,
            raw.len(),
            raw.len() as u64,
            LAYOUT,
            &mut legacy_work,
            legacy_cap,
        ),
        Err(Error::Budget(_))
    ));
    let mut tiny_work = 0;
    assert!(matches!(
        page_with_layout_and_work(
            &db,
            PhilosophyOriginalCollection::Nodes,
            None,
            1,
            raw.len(),
            raw.len() as u64,
            LAYOUT,
            &mut tiny_work,
            1,
        ),
        Err(Error::Budget(_))
    ));
    assert_eq!(snapshot(&db), db_snapshot);
    eprintln!(
        "real producer row: path={} logical_bytes={} measured_decode_work={} legacy_page_work_cap={}",
        path.display(),
        raw.len(),
        measured_work,
        legacy_cap
    );
    assert!(Instant::now() < deadline);
}
