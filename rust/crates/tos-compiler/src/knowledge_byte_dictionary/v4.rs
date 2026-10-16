//! V4 dictionaries rotate after a bounded group and are V2-framed on disk.
//! Exact dictionary hashes refer to decoded bytes, never source authority.
use super::*;
use crate::knowledge_byte_codec as codec;

pub(crate) const DDL: &str = "CREATE TABLE knowledge_byte_dictionaries(dictionary_sha256 BLOB PRIMARY KEY NOT NULL CHECK(length(dictionary_sha256)=32),dictionary BLOB NOT NULL CHECK(length(dictionary) BETWEEN 18 AND 32785));";
pub(crate) const PREPARATION_SCHEMA: crate::knowledge_stage::PreparationSchema = crate::knowledge_stage::preparation_schema!(
    table "knowledge_byte_dictionary_pending(dictionary_kind TEXT NOT NULL,source_graph TEXT NOT NULL,samples INTEGER NOT NULL CHECK(samples BETWEEN 1 AND 256),dictionary BLOB NOT NULL CHECK(length(dictionary)<=32768),dictionary_sha256 BLOB CHECK(dictionary_sha256 IS NULL OR length(dictionary_sha256)=32),PRIMARY KEY(dictionary_kind,source_graph)) WITHOUT ROWID"
);
pub(crate) const MAX_WRITE_BYTES: usize =
    2 * codec::DICTIONARY_WINDOW_BYTES + codec::HEADER + MAX_GRAPH_BYTES + 256;
const ROTATE_ROWS: i64 = 256;

fn read_digest<'s, 'b>(
    db: &Connection,
    state: &'s CreationState<'b>,
    digest: Digest256,
) -> Result<OwnedDictionary<'s, 'b>> {
    with_statement(db, state, c"SELECT CASE WHEN typeof(dictionary)='blob' AND length(dictionary) BETWEEN 18 AND 32785 THEN dictionary END FROM knowledge_byte_dictionaries WHERE dictionary_sha256=?1", |statement| {
        statement.bind_blob(1, digest.as_bytes()).map_err(sql_error)?;
        if !statement.step().map_err(sql_error)? { return Err(Error::Invalid("V4 dictionary absent")); }
        let ValueRef::Blob(stored) = statement.value_ref(0).map_err(sql_error)? else { return Err(Error::Invalid("V4 dictionary type/length")); };
        let mut owned = allocate_with_capacity(state, codec::DICTIONARY_WINDOW_BYTES)?;
        codec::with_decoded(state, stored, None, codec::DICTIONARY_WINDOW_BYTES, |raw| {
            state.charge_work(raw.len())?;
            if Digest256::of_bytes(raw) != digest { return Err(Error::Invalid("V4 dictionary digest differs")); }
            state.charge_work(raw.len())?;
            owned.bytes.extend_from_slice(raw);
            owned.digest = Some(digest);
            Ok(())
        })?;
        if !statement.step().map_err(sql_error)? { Ok(owned) } else { Err(Error::Invalid("V4 dictionary duplicate")) }
    })
}

pub(crate) fn read<'s, 'b>(
    db: &Connection,
    state: &'s CreationState<'b>,
    stored: &[u8],
    max_bytes: usize,
) -> Result<Option<OwnedDictionary<'s, 'b>>> {
    if !codec::is_dictionary_frame(stored) {
        return Ok(None);
    }
    let (_, digest) = codec::dictionary_frame_metadata(stored, None, max_bytes)?;
    read_digest(db, state, digest).map(Some)
}

pub(crate) fn prepare<'s, 'b>(
    db: &Connection,
    state: &'s CreationState<'b>,
    kind: &str,
    graph: &str,
    raw: &[u8],
) -> Result<(Option<OwnedDictionary<'s, 'b>>, u64, u64)> {
    if !matches!(kind, "source" | "node" | "relation")
        || graph.is_empty()
        || graph.len() > MAX_GRAPH_BYTES
        || raw.is_empty()
    {
        return Err(Error::Invalid("V4 dictionary producer family"));
    }
    let mut owned = allocate_with_capacity(state, codec::DICTIONARY_WINDOW_BYTES)?;
    let (samples, selected) = with_statement(db, state, c"SELECT samples,CASE WHEN typeof(dictionary)='blob' AND length(dictionary)<=32768 THEN dictionary END,dictionary_sha256 FROM knowledge_byte_dictionary_pending WHERE dictionary_kind=?1 AND source_graph=?2", |statement| {
        statement.bind_text(1, kind).map_err(sql_error)?;
        statement.bind_text(2, graph).map_err(sql_error)?;
        if !statement.step().map_err(sql_error)? { return Ok((0, None)); }
        let samples = statement.integer(0).map_err(sql_error)?;
        if !(1..=ROTATE_ROWS).contains(&samples) { return Err(Error::Invalid("V4 dictionary sample count")); }
        let ValueRef::Blob(pending) = statement.value_ref(1).map_err(sql_error)? else { return Err(Error::Invalid("V4 dictionary pending type/length")); };
        let digest = match statement.value_ref(2).map_err(sql_error)? {
            ValueRef::Null => None,
            ValueRef::Blob(value) if value.len() == 32 => Some(Digest256::from_bytes(value.try_into().map_err(|_| Error::Invalid("V4 dictionary digest"))?)),
            _ => return Err(Error::Invalid("V4 dictionary digest")),
        };
        if digest.is_some() && !pending.is_empty() || digest.is_none() && pending.is_empty() {
            return Err(Error::Invalid("V4 dictionary pending shape"));
        }
        if samples < ROTATE_ROWS {
            state.charge_work(pending.len())?;
            owned.bytes.extend_from_slice(pending);
        }
        if statement.step().map_err(sql_error)? { return Err(Error::Invalid("V4 dictionary pending duplicate")); }
        Ok(if samples == ROTATE_ROWS { (0, None) } else { (samples, digest) })
    })?;
    if let Some(digest) = selected {
        drop(owned);
        let selected = read_digest(db, state, digest)?;
        with_statement(db, state, c"UPDATE knowledge_byte_dictionary_pending SET samples=?3 WHERE dictionary_kind=?1 AND source_graph=?2", |statement| {
            statement.bind_text(1, kind).map_err(sql_error)?;
            statement.bind_text(2, graph).map_err(sql_error)?;
            statement.bind_i64(3, samples + 1).map_err(sql_error)?;
            if statement.step().map_err(sql_error)? { return Err(Error::Invalid("V4 dictionary count update")); }
            Ok(())
        })?;
        return Ok((Some(selected), 1, (kind.len() + graph.len() + 128) as u64));
    }
    let take = raw
        .len()
        .min(codec::DICTIONARY_WINDOW_BYTES - owned.bytes.len());
    state.charge_work(take)?;
    owned.bytes.extend_from_slice(&raw[..take]);
    let sealed = owned.bytes.len() == codec::DICTIONARY_WINDOW_BYTES || samples == 31;
    let mut published_bytes = 0;
    if sealed {
        state.charge_work(owned.bytes.len())?;
        let digest = Digest256::of_bytes(&owned.bytes);
        owned.digest = Some(digest);
        codec::with_encoded(
            state,
            &owned.bytes,
            codec::DICTIONARY_WINDOW_BYTES,
            |stored| {
                with_statement(db, state, c"INSERT INTO knowledge_byte_dictionaries(dictionary_sha256,dictionary) VALUES (?1,?2) ON CONFLICT(dictionary_sha256) DO NOTHING", |statement| {
                statement.bind_blob(1, digest.as_bytes()).map_err(sql_error)?;
                statement.bind_blob(2, stored).map_err(sql_error)?;
                if statement.step().map_err(sql_error)? { return Err(Error::Invalid("V4 dictionary insertion row")); }
                Ok(())
            })?;
                published_bytes = stored.len();
                Ok(())
            },
        )?;
        let selected = read_digest(db, state, digest)?;
        state.charge_work(owned.bytes.len())?;
        if selected.bytes != owned.bytes {
            return Err(Error::Invalid("V4 dictionary digest collision"));
        }
    }
    with_statement(db, state, c"INSERT INTO knowledge_byte_dictionary_pending(dictionary_kind,source_graph,samples,dictionary,dictionary_sha256) VALUES (?1,?2,?3,?4,?5) ON CONFLICT(dictionary_kind,source_graph) DO UPDATE SET samples=excluded.samples,dictionary=excluded.dictionary,dictionary_sha256=excluded.dictionary_sha256", |statement| {
        statement.bind_text(1, kind).map_err(sql_error)?;
        statement.bind_text(2, graph).map_err(sql_error)?;
        statement.bind_i64(3, samples + 1).map_err(sql_error)?;
        statement.bind_blob(4, if sealed { &[] } else { &owned.bytes }).map_err(sql_error)?;
        if let Some(digest) = owned.digest { statement.bind_blob(5, digest.as_bytes()).map_err(sql_error)?; }
        // Unbound parameters are SQL NULL on this newly prepared statement.
        if statement.step().map_err(sql_error)? { return Err(Error::Invalid("V4 dictionary selection row")); }
        Ok(())
    })?;
    let bytes = published_bytes
        + if sealed { 0 } else { owned.bytes.len() }
        + kind.len()
        + graph.len()
        + 256;
    Ok((
        if sealed { Some(owned) } else { None },
        1 + u64::from(sealed),
        bytes as u64,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64},
    };
    use std::time::{Duration, Instant};

    #[test]
    fn rotating_packed_dictionaries_keep_old_exact_frames_readable() {
        const CHILD: &str = "TOS_ROTATING_DICTIONARY_TEST_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let result = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "knowledge_byte_dictionary::v4::tests::rotating_packed_dictionaries_keep_old_exact_frames_readable", "--nocapture"])
                .env(CHILD, "1").output().unwrap();
            assert!(
                result.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            );
            return;
        }
        use crate::knowledge_payload_read::RuntimeKnowledgeOwnedBudget;
        let deadline = Instant::now() + Duration::from_secs(60);
        let cancelled = Arc::new(AtomicBool::new(false));
        let remaining = |n: usize| {
            (16 * 1024 * 1024usize)
                .checked_sub(n)
                .ok_or(Error::Budget("dictionary test state"))
        };
        let heap = crate::sqlite_budget::DedicatedSessionSqliteHeap::establish(
            4 * 1024 * 1024,
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
            original_work_limit: 1024 * 1024 * 1024,
            original_sql_vm: &vm,
            original_sql_vm_limit: 10_000_000,
            original_sqlite_heap: &heap,
            remaining_json_visits: 1_000_000,
            owner_deadline: deadline,
            operation_deadline: deadline,
            cancelled: &cancelled,
        };
        let state = CreationState::from_runtime_owned_budget(&budget).unwrap();
        let rank_identity = serde_json::to_string(&vec![
            "exact e\u{301} \"quoting\"",
            "строка".repeat(2048).as_str(),
        ])
        .unwrap();
        let rank_visible =
            serde_json::to_string(&vec!["source\r\nline", "東京".repeat(1024).as_str()]).unwrap();
        let abi = tos_foundation::KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V2_CARRIER_ONCE_V4;
        crate::knowledge_search_rank::with_encoded_pair(
            true,
            Some(&state),
            &rank_identity,
            &rank_visible,
            32768,
            |a, b| {
                let (identity, visible) =
                    crate::knowledge_search_rank::decode_pair_owned(abi, a, b, 32768, &state)?;
                assert_eq!(
                    (identity.as_str(), visible.as_str()),
                    (rank_identity.as_str(), rank_visible.as_str())
                );
                assert!(
                    crate::search_rank_field_size(
                        tos_foundation::KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V1_CARRIER_ONCE_V3,
                        a,
                        32768
                    )
                    .is_err()
                );
                assert!(
                    crate::search_rank_field_size(
                        abi,
                        rusqlite::types::ValueRef::Text(rank_identity.as_bytes()),
                        32768
                    )
                    .is_err()
                );
                let mut work = 0;
                assert!(
                    crate::decode_search_rank_field(abi, a, 32768, 0, &mut work, 1_048_576)
                        .is_err()
                );
                assert!(
                    crate::decode_search_rank_field(abi, a, 32768, 1_048_576, &mut work, 0)
                        .is_err()
                );
                assert!(crate::search_rank_field_size(abi, a, 8).is_err());
                let mut corrupt = a.as_blob().unwrap().to_vec();
                corrupt.push(0);
                assert!(
                    crate::decode_search_rank_field(
                        abi,
                        rusqlite::types::ValueRef::Blob(&corrupt),
                        32768,
                        1_048_576,
                        &mut work,
                        1_048_576
                    )
                    .is_err()
                );
                Ok(())
            },
        )
        .unwrap();
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(DDL).unwrap();
        db.execute_batch(PREPARATION_SCHEMA.temporary).unwrap();
        let mut retained = Vec::new();
        for i in 0..520 {
            let raw = format!(
                "{{\"id\":{i},\"group\":{},\"exact\":\"{}\"}}\n",
                i / 256,
                "original e\u{301} \r\n spelling ".repeat(320)
            )
            .into_bytes();
            let (dictionary, rows, bytes) = prepare(&db, &state, "node", "fixture", &raw).unwrap();
            assert!(rows <= MAX_WRITE_ROWS as u64 && bytes <= MAX_WRITE_BYTES as u64);
            if let Some(dictionary) = dictionary {
                let dictionary = dictionary.verified().unwrap();
                codec::with_dictionary_encoded(&state, &raw, dictionary, 32768, |stored| {
                    let selected = read(&db, &state, stored, 32768)?.unwrap();
                    codec::with_dictionary_decoded(
                        &state,
                        stored,
                        selected.verified()?,
                        Some(raw.len()),
                        32768,
                        |actual| {
                            assert_eq!(actual, raw);
                            Ok(())
                        },
                    )?;
                    if [16, 272, 519].contains(&i) {
                        retained.push((stored.to_vec(), raw.clone()));
                    }
                    Ok(())
                })
                .unwrap();
            }
        }
        let (count, physical): (u64, u64) = db
            .query_row(
                "SELECT count(*),sum(length(dictionary)) FROM knowledge_byte_dictionaries",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(count, 3);
        assert!(physical < count * codec::DICTIONARY_WINDOW_BYTES as u64 / 2);
        assert_eq!(retained.len(), 3);
        let known = codec::dictionary_frame_metadata(&retained[0].0, None, 32768)
            .unwrap()
            .1;
        for (stored, expected) in retained {
            let selected = read(&db, &state, &stored, 32768).unwrap().unwrap();
            codec::with_dictionary_decoded(
                &state,
                &stored,
                selected.verified().unwrap(),
                Some(expected.len()),
                32768,
                |actual| {
                    assert_eq!(actual, expected);
                    Ok(())
                },
            )
            .unwrap();
            assert!(
                super::super::read(&db, &state, &stored, 32768).is_err(),
                "legacy raw-dictionary context must reject V4 storage"
            );
        }
        db.execute(
            "UPDATE knowledge_byte_dictionaries SET dictionary=?1",
            [&[0u8; 18][..]],
        )
        .unwrap();
        assert!(read_digest(&db, &state, known).is_err());
        assert!(Instant::now() < deadline);
    }
}
