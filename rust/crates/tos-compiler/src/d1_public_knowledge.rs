//! Public v9 knowledge rows from the disposable normalized stage. The
//! selected block index stays private; D1 receives its maintained expanded
//! SQL transport with the same logical postings and rank documents.

use crate::{
    Error, Result,
    d1::{D1Cell, D1RowTransition, D1Table},
    d1_public_capture::{MAX_ROW_BYTES, PublicCapture, compact, json},
    d1_public_lens::{LensCounts, emit_lens_auxiliary},
    d1_public_rows::{encoded, lower_search, portable, preflight_large_fields},
    d1_public_sql::{MAX_ROW_VALUE_BYTES, SqlSink, chunks, quote, quote_len},
    knowledge_posting_codec::{MAX_POSTING_DELTA_BYTES, decode_posting_block},
    knowledge_search::{SearchBuildLimits, SourceRow, document},
    knowledge_stage::{KnowledgeStage, WritePhase},
};
use rusqlite::{Statement, params};
use std::{collections::VecDeque, path::Path};
use tos_foundation::{Digest256, JsonValue, python_lower_unicode16_v1};

#[derive(Default)]
pub(crate) struct KnowledgeSqlCounts {
    pub nodes: u64,
    pub relations: u64,
    pub postings: u64,
    pub distinct_grams: u64,
}

/// Public path normalization precedes the public catalog and search index.
/// Their digests/postings must describe the bytes that D1 actually emits.
/// This changes only the disposable public stage, never a selected model.
pub(crate) fn portabilize_public_stage(
    stage: &mut KnowledgeStage<'_>,
    capture: &PublicCapture,
    root: &Path,
) -> Result<()> {
    if !stage.public_build() {
        return Err(Error::Invalid(
            "public D1 path adaptation on selected stage",
        ));
    }
    let root = root
        .to_str()
        .ok_or(Error::Invalid("public D1 root UTF-8"))?;
    for table in ["knowledge_nodes", "knowledge_relations"] {
        let mut after = -1i64;
        loop {
            let page: Vec<(i64, String, Vec<u8>)> = stage.with_connection(
                WritePhase::Normalized,
                |db| {
                    let mut statement = db.prepare(&format!(
                        "SELECT source_order,id,payload FROM {table} WHERE source_order>?1 ORDER BY source_order LIMIT 8"
                    ))?;
                    let page = statement
                        .query_map([after], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
                        .collect::<std::result::Result<_, _>>()
                        .map_err(Error::from)?;
                    Ok(page)
                },
            )?;
            if page.is_empty() {
                break;
            }
            let mut changed = Vec::new();
            let mut page_bytes = 0u64;
            for (position, id, raw) in page {
                if position <= after || raw.len() > MAX_ROW_BYTES {
                    return Err(Error::Invalid("public D1 normalized row order/bytes"));
                }
                after = position;
                capture.charge_work(raw.len() as u64)?;
                let mut value = json(&raw, MAX_ROW_BYTES)?;
                portable(&mut value, root);
                let bytes = compact(&value, MAX_ROW_BYTES)?;
                capture.charge_work(bytes.len() as u64)?;
                if bytes != raw {
                    page_bytes = page_bytes
                        .checked_add(bytes.len() as u64)
                        .filter(|total| *total <= 64 * 1024 * 1024)
                        .ok_or(Error::Budget("public D1 normalized page bytes"))?;
                    changed.push((id, bytes));
                }
            }
            if !changed.is_empty() {
                stage.with_write_page(WritePhase::Normalized, 8, page_bytes, |stage| {
                    stage.charge_materialized(changed.len() as u64, page_bytes)?;
                    stage.with_connection(WritePhase::Normalized, |db| {
                        let mut statement = db.prepare(&format!(
                            "UPDATE {table} SET payload=?1,payload_len=?2,payload_sha256=?3 WHERE id=?4"
                        ))?;
                        for (id, bytes) in &changed {
                            let digest = Digest256::of_bytes(bytes);
                            let count = statement.execute(params![
                                bytes,
                                bytes.len() as i64,
                                digest.as_bytes().as_slice(),
                                id
                            ])?;
                            if count != 1 {
                                return Err(Error::Invalid("public D1 normalized row changed"));
                            }
                        }
                        Ok(())
                    })
                })?;
            }
        }
    }
    Ok(())
}

fn text<'a>(value: &'a JsonValue, key: &str) -> &'a str {
    value
        .object_get(key)
        .and_then(JsonValue::as_str)
        .unwrap_or("")
}
fn display<'a>(value: &'a JsonValue, field: &str) -> &'a str {
    value
        .object_get("display")
        .and_then(|display| display.object_get(field))
        .and_then(|field| field.object_get("default"))
        .and_then(JsonValue::as_str)
        .unwrap_or("")
}

/// Reuse the maintained Rust public row/search kernels for one exact
/// normalized prepared row. Position is the selected D1 posting address;
/// predecessor comparison and complete tie-group allocation belong to the
/// caller-held D1 snapshot capture.
pub fn project_private_knowledge_row(
    kind: &str,
    position: i64,
    raw: &str,
    repo_root: &str,
    limits: SearchBuildLimits,
) -> Result<Vec<D1RowTransition>> {
    limits.validate()?;
    if !matches!(kind, "node" | "relation") || position < 0 || raw.len() > limits.max_payload_bytes
    {
        return Err(Error::Invalid("private D1 normalized row profile/bytes"));
    }
    if raw.len() as u64 > limits.max_work_bytes {
        return Err(Error::Budget("private D1 search projection work"));
    }
    let mut item = crate::d1_public_capture::json(raw.as_bytes(), MAX_ROW_BYTES)?;
    portable(&mut item, repo_root);
    if String::from_utf8(compact(&item, MAX_ROW_BYTES)?)
        .map_err(|_| Error::Invalid("private D1 normalized row UTF-8"))?
        != raw
    {
        return Err(Error::Invalid(
            "private D1 normalized row requires portable-path migration",
        ));
    }
    let field = |name: &str| {
        item.object_get(name)
            .and_then(JsonValue::as_str)
            .unwrap_or("")
    };
    let id = field("id");
    let source_graph = field("source_graph");
    let term_id = field(if kind == "node" {
        "kind_id"
    } else {
        "predicate_id"
    });
    if id.is_empty() || source_graph.is_empty() || term_id.is_empty() {
        return Err(Error::Invalid("private D1 normalized row identity"));
    }
    let native_id = item
        .object_get("native_id")
        .and_then(JsonValue::as_str)
        .map(str::to_owned);
    let row = SourceRow {
        position,
        id: id.to_owned(),
        source_graph: source_graph.to_owned(),
        native_id,
        term_id: term_id.to_owned(),
        payload_len: raw.len() as i64,
        payload_sha256: Digest256::of_bytes(raw.as_bytes()).as_bytes().to_vec(),
        payload: Some(raw.as_bytes().to_vec()),
    };
    let plural = if kind == "node" { "nodes" } else { "relations" };
    let doc = document(&row, plural, raw.as_bytes(), limits)?;
    let lower = |value: &str, cap: usize| {
        python_lower_unicode16_v1(value, cap, cap, cap)
            .map_err(|error| Error::Source(error.to_string()))
    };
    let primary_name = if kind == "node" { "title" } else { "label" };
    let secondary_name = if kind == "node" {
        "summary"
    } else {
        "explanation"
    };
    let primary = lower(display(&item, primary_name), limits.max_rank_field_bytes)?;
    let secondary = display(&item, secondary_name).to_owned();
    let mut result = Vec::new();
    let text = |value: &str| D1Cell::Text(value.to_owned());
    let integer = |value: i64| D1Cell::Integer(value);
    if kind == "node" {
        result.push(D1RowTransition {
            table: D1Table::KnowledgeNodes,
            before: None,
            after: Some(vec![
                text(id),
                text(field("entity_id")),
                text(field("native_id")),
                text(source_graph),
                text(field("kind_id")),
                text(field("type_id")),
                text(&primary),
                text(&secondary),
                text(&doc.text),
                text(raw),
            ]),
        });
    } else {
        result.push(D1RowTransition {
            table: D1Table::KnowledgeRelations,
            before: None,
            after: Some(vec![
                text(id),
                text(field("native_id")),
                text(source_graph),
                text(field("from_id")),
                text(field("to_id")),
                text(field("predicate_id")),
                text(field("relation_type_id")),
                text(&primary),
                text(&secondary),
                text(&doc.text),
                text(raw),
            ]),
        });
    }
    // Match the maintained producer's quoted-value threshold and overflow
    // companions; the D1 base row cannot retain both oversized fields.
    let base_row = result[0]
        .after
        .as_mut()
        .ok_or(Error::Invalid("private D1 base row"))?;
    let field_count = base_row.len();
    let base_bytes = base_row[..field_count - 2]
        .iter()
        .try_fold(1024usize, |used, cell| {
            let D1Cell::Text(value) = cell else {
                return Err(Error::Invalid("private D1 base field"));
            };
            used.checked_add(quote_len(value)?)
                .ok_or(Error::Budget("private D1 selection fields"))
        })?;
    let inline = base_bytes
        .checked_add(quote_len(&doc.text)?)
        .and_then(|used| used.checked_add(quote_len(raw).ok()?))
        .is_some_and(|used| used <= MAX_ROW_VALUE_BYTES);
    if !inline {
        if base_bytes
            .checked_add(4)
            .is_none_or(|used| used > MAX_ROW_VALUE_BYTES)
        {
            return Err(Error::Budget("private D1 selection fields"));
        }
        base_row[field_count - 2] = text("");
        base_row[field_count - 1] = text("");
        let payload_key = format!("knowledge_{kind}_payload:{id}");
        for (part, chunk) in chunks(raw).enumerate() {
            result.push(D1RowTransition {
                table: D1Table::EdgeMeta,
                before: None,
                after: Some(vec![text(&payload_key), integer(part as i64), text(chunk)]),
            });
        }
        let search_key = format!("knowledge_{kind}_search:{id}");
        visit_search_fragments(&doc.text, |part, fragment| {
            result.push(D1RowTransition {
                table: D1Table::EdgeMeta,
                before: None,
                after: Some(vec![
                    text(&search_key),
                    integer(part as i64),
                    text(fragment),
                ]),
            });
            Ok(())
        })?;
    }
    result.push(D1RowTransition {
        table: D1Table::KnowledgeSearchDocuments,
        before: None,
        after: Some(vec![
            text(if kind == "node" { "nodes" } else { "relations" }),
            integer(position),
            text(id),
            text(source_graph),
            text(if kind == "node" { field("kind_id") } else { "" }),
            text(if kind == "node" {
                ""
            } else {
                field("predicate_id")
            }),
            text(&doc.id_lower),
            text(&doc.native_id_lower),
            text(&doc.identity_values),
            text(&doc.visible_values),
            integer(doc.text.chars().count() as i64),
            text(&doc.digest.to_hex()),
        ]),
    });
    let mut work = (raw.len() as u64)
        .checked_add(doc.text.len() as u64)
        .filter(|bytes| *bytes <= limits.max_work_bytes)
        .ok_or(Error::Budget("private D1 search projection work"))?;
    let mut grams = std::collections::BTreeSet::new();
    let mut window = VecDeque::with_capacity(3);
    for scalar in doc.text.chars() {
        if window.len() == 3 {
            window.pop_front();
        }
        window.push_back(scalar);
        if window.len() < 3 {
            continue;
        }
        let gram = window.iter().collect::<String>();
        // Charge every attempted window before retaining another unique gram.
        work = work
            .checked_add(gram.len() as u64 + 8)
            .filter(|bytes| *bytes <= limits.max_work_bytes)
            .ok_or(Error::Budget("private D1 search projection work"))?;
        if !grams.contains(&gram) {
            if grams.len() as u64 >= limits.max_postings {
                return Err(Error::Budget("private D1 search projection postings"));
            }
            grams.insert(gram);
        }
    }
    for gram in grams {
        work = work
            .checked_add(gram.len() as u64 + 16)
            .filter(|bytes| *bytes <= limits.max_work_bytes)
            .ok_or(Error::Budget("private D1 search projection work"))?;
        result.push(D1RowTransition {
            table: D1Table::KnowledgeSearchGrams,
            before: None,
            after: Some(vec![
                text(if kind == "node" { "nodes" } else { "relations" }),
                integer(3),
                text(&gram),
                integer(position),
            ]),
        });
    }
    result.push(D1RowTransition {
        table: D1Table::KnowledgeLensOrder,
        before: None,
        after: Some(vec![
            text(kind),
            text(id),
            text(&lower(id, limits.max_rank_field_bytes)?),
            text(if kind == "relation" {
                field("from_id")
            } else {
                ""
            }),
            text(if kind == "relation" {
                field("to_id")
            } else {
                ""
            }),
        ]),
    });
    let digest_key = format!("knowledge_{kind}_digest:{id}");
    let digest_raw = format!(
        "{{\"sha256\":\"{}\"}}",
        Digest256::of_bytes(raw.as_bytes()).to_hex()
    );
    result.push(D1RowTransition {
        table: D1Table::EdgeMeta,
        before: None,
        after: Some(vec![text(&digest_key), integer(0), text(&digest_raw)]),
    });
    let retained_bytes = result
        .iter()
        .filter_map(|row| row.after.as_ref())
        .flat_map(|row| row.iter())
        .try_fold(0u64, |used, value| {
            let bytes = match value {
                D1Cell::Text(text) => text.len() as u64,
                D1Cell::Integer(_) => 8,
                D1Cell::Null => 0,
            };
            used.checked_add(bytes)
                .ok_or(Error::Budget("private D1 projected bytes"))
        })?;
    if retained_bytes > limits.max_work_bytes {
        return Err(Error::Budget("private D1 projected bytes"));
    }
    Ok(result)
}
fn bounded_quote(capture: &PublicCapture, value: &str) -> Result<String> {
    let len = quote_len(value)?;
    if len > MAX_ROW_VALUE_BYTES {
        return Err(Error::Budget("public D1 knowledge field bytes"));
    }
    capture.charge_work(len as u64)?;
    quote(value)
}
fn meta_chunk(capture: &PublicCapture, sink: &mut SqlSink, key: &str, value: &str) -> Result<()> {
    for (part, chunk) in chunks(value).enumerate() {
        sink.insert(
            "edge_meta_next",
            &["key", "part", "json_chunk"],
            &[
                bounded_quote(capture, key)?,
                part.to_string(),
                bounded_quote(capture, chunk)?,
            ],
        )?;
    }
    Ok(())
}

fn search_fragments(
    capture: &PublicCapture,
    sink: &mut SqlSink,
    key: &str,
    search: &str,
) -> Result<()> {
    visit_search_fragments(search, |part, fragment| {
        sink.insert(
            "edge_meta_next",
            &["key", "part", "json_chunk"],
            &[
                bounded_quote(capture, key)?,
                part.to_string(),
                bounded_quote(capture, fragment)?,
            ],
        )
    })
}

// Shared by full public SQL emission and private transition projection.
fn visit_search_fragments(
    search: &str,
    mut emit: impl FnMut(usize, &str) -> Result<()>,
) -> Result<()> {
    let mut recent = VecDeque::with_capacity(1023);
    let mut start = 0usize;
    let mut count = 0usize;
    let mut part = 0usize;
    for (byte, scalar) in search.char_indices() {
        recent.push_back(byte);
        if recent.len() > 1023 {
            recent.pop_front();
        }
        count += 1;
        if count % 8192 == 0 {
            let end = byte + scalar.len_utf8();
            emit(part, &search[start..end])?;
            part += 1;
            start = *recent
                .front()
                .ok_or(Error::Invalid("public D1 search fragment"))?;
        }
    }
    if count % 8192 != 0 {
        emit(part, &search[start..])?;
    }
    Ok(())
}

fn emit_normalized_row(
    capture: &PublicCapture,
    sink: &mut SqlSink,
    documents: &mut Statement<'_>,
    lens_counts: &mut LensCounts,
    max_lens_bytes: u64,
    max_lens_memberships: u64,
    kind: &str,
    row: SourceRow,
    root: &str,
    limits: SearchBuildLimits,
) -> Result<()> {
    let raw = row
        .payload
        .as_deref()
        .ok_or(Error::Budget("public D1 knowledge payload"))?;
    if row.payload_len < 0
        || row.payload_len as usize != raw.len()
        || row.payload_sha256.as_slice() != Digest256::of_bytes(raw).as_bytes()
    {
        return Err(Error::Invalid("public D1 knowledge payload digest"));
    }
    let mut item = crate::d1_public_capture::json(raw, MAX_ROW_BYTES)?;
    portable(&mut item, root);
    if text(&item, "id") != row.id || text(&item, "source_graph") != row.source_graph {
        return Err(Error::Invalid("public D1 knowledge row identity"));
    }
    let item_json = encoded(capture, &item)?;
    if item_json.as_bytes() != raw {
        return Err(Error::Invalid(
            "public D1 search source was not path adapted",
        ));
    }
    let portable_row = SourceRow {
        payload_len: item_json.len() as i64,
        payload_sha256: Digest256::of_bytes(item_json.as_bytes())
            .as_bytes()
            .to_vec(),
        payload: None,
        ..row
    };
    let is_node = kind == "node";
    let doc = document(
        &portable_row,
        if is_node { "nodes" } else { "relations" },
        item_json.as_bytes(),
        limits,
    )?;
    let stored: (String, i64, Vec<u8>, String, String, String, String) = documents.query_row(
        params![
            if kind == "node" { "nodes" } else { "relations" },
            portable_row.position
        ],
        |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
            ))
        },
    )?;
    if stored.0 != portable_row.id
        || stored.1 != doc.chars as i64
        || stored.2.as_slice() != doc.digest.as_bytes()
        || stored.3 != doc.id_lower
        || stored.4 != doc.native_id_lower
        || stored.5 != doc.identity_values
        || stored.6 != doc.visible_values
    {
        return Err(Error::Invalid("public D1 search document/source mismatch"));
    }
    capture.charge_work(doc.serialization_bytes as u64 + doc.text.len() as u64)?;
    let id = &portable_row.id;
    let (table, columns, secondary, secondary_name, primary_name) = if is_node {
        (
            "knowledge_nodes_next",
            &[
                "id",
                "entity_id",
                "native_id",
                "source_graph",
                "kind_id",
                "type_id",
                "title_text",
                "summary_text",
                "search_text",
                "json",
            ][..],
            display(&item, "summary"),
            "summary_text",
            "title",
        )
    } else {
        (
            "knowledge_relations_next",
            &[
                "id",
                "native_id",
                "source_graph",
                "from_id",
                "to_id",
                "predicate_id",
                "relation_type_id",
                "label_text",
                "explanation_text",
                "search_text",
                "json",
            ][..],
            display(&item, "explanation"),
            "explanation_text",
            "label",
        )
    };
    let primary = lower_search(capture, display(&item, primary_name))?;
    let mut values = if is_node {
        vec![
            bounded_quote(capture, id)?,
            bounded_quote(capture, text(&item, "entity_id"))?,
            bounded_quote(capture, text(&item, "native_id"))?,
            bounded_quote(capture, &portable_row.source_graph)?,
            bounded_quote(capture, text(&item, "kind_id"))?,
            bounded_quote(capture, text(&item, "type_id"))?,
            bounded_quote(capture, &primary)?,
            bounded_quote(capture, secondary)?,
        ]
    } else {
        vec![
            bounded_quote(capture, id)?,
            bounded_quote(capture, text(&item, "native_id"))?,
            bounded_quote(capture, &portable_row.source_graph)?,
            bounded_quote(capture, text(&item, "from_id"))?,
            bounded_quote(capture, text(&item, "to_id"))?,
            bounded_quote(capture, text(&item, "predicate_id"))?,
            bounded_quote(capture, text(&item, "relation_type_id"))?,
            bounded_quote(capture, &primary)?,
            bounded_quote(capture, secondary)?,
        ]
    };
    let base = values.iter().try_fold(1024usize, |sum, value| {
        sum.checked_add(value.len())
            .ok_or(Error::Budget("public D1 knowledge row bytes"))
    })?;
    let inline = base
        .checked_add(quote_len(&doc.text)?)
        .and_then(|n| n.checked_add(quote_len(&item_json).ok()?))
        .is_some_and(|bytes| bytes <= MAX_ROW_VALUE_BYTES);
    if inline {
        preflight_large_fields(capture, &[&doc.text, &item_json])?;
        values.push(bounded_quote(capture, &doc.text)?);
        values.push(bounded_quote(capture, &item_json)?);
    } else {
        if item_json.len() > MAX_ROW_BYTES {
            return Err(Error::Budget("public D1 knowledge overflow source"));
        }
        if base
            .checked_add(4)
            .is_none_or(|bytes| bytes > MAX_ROW_VALUE_BYTES)
        {
            return Err(Error::Budget("public D1 knowledge selection fields"));
        }
        values.push("''".to_owned());
        values.push("''".to_owned());
        meta_chunk(
            capture,
            sink,
            &format!("knowledge_{kind}_payload:{id}"),
            &item_json,
        )?;
        search_fragments(
            capture,
            sink,
            &format!("knowledge_{kind}_search:{id}"),
            &doc.text,
        )?;
    }
    let selector = format!("id={}", bounded_quote(capture, id)?);
    sink.insert_chunked(
        table,
        columns,
        &values,
        &selector,
        &[
            (secondary_name, secondary),
            ("search_text", if inline { &doc.text } else { "" }),
            ("json", if inline { &item_json } else { "" }),
        ],
    )?;
    let digest = format!(
        "{{\"sha256\":\"{}\"}}",
        Digest256::of_bytes(item_json.as_bytes()).to_hex()
    );
    meta_chunk(
        capture,
        sink,
        &format!("knowledge_{kind}_digest:{id}"),
        &digest,
    )?;
    emit_lens_auxiliary(
        capture,
        sink,
        kind,
        id,
        &item_json,
        &item,
        max_lens_bytes,
        max_lens_memberships,
        lens_counts,
    )?;
    sink.insert(
        "knowledge_lens_order_next",
        &["kind", "id", "sort_key", "from_id", "to_id"],
        &[
            bounded_quote(capture, kind)?,
            bounded_quote(capture, id)?,
            bounded_quote(capture, &lower_search(capture, id)?)?,
            bounded_quote(capture, if is_node { "" } else { text(&item, "from_id") })?,
            bounded_quote(capture, if is_node { "" } else { text(&item, "to_id") })?,
        ],
    )?;
    Ok(())
}

pub(crate) fn emit_knowledge(
    stage: &mut KnowledgeStage<'_>,
    capture: &PublicCapture,
    sink: &mut SqlSink,
    root: &Path,
    limits: SearchBuildLimits,
    counts: &mut KnowledgeSqlCounts,
    lens_counts: &mut LensCounts,
    max_lens_bytes: u64,
    max_lens_memberships: u64,
) -> Result<()> {
    let root = root
        .to_str()
        .ok_or(Error::Invalid("public D1 root UTF-8"))?;
    for (kind, table) in [
        ("node", "knowledge_nodes"),
        ("relation", "knowledge_relations"),
    ] {
        stage.with_connection(WritePhase::Search, |db| {
            let mut documents = db.prepare("SELECT id,document_chars,document_digest,id_lower,native_id_lower,identity_values,visible_values FROM search_documents WHERE kind=?1 AND position=?2")?;
            let sql = if kind == "node" {
                "SELECT source_order,id,source_graph,native_id,kind_id,payload_len,payload_sha256,CASE WHEN payload_len<=?1 AND length(payload)=payload_len THEN payload ELSE NULL END FROM knowledge_nodes ORDER BY source_order"
            } else {
                "SELECT source_order,id,source_graph,native_id,predicate_id,payload_len,payload_sha256,CASE WHEN payload_len<=?1 AND length(payload)=payload_len THEN payload ELSE NULL END FROM knowledge_relations ORDER BY source_order"
            };
            let mut statement = db.prepare(sql)?;
            let mut rows = statement.query([limits.max_payload_bytes as i64])?;
            let mut expected = 0i64;
            while let Some(row) = rows.next()? {
                let carrier = SourceRow {position:row.get(0)?,id:row.get(1)?,source_graph:row.get(2)?,
                    native_id:row.get(3)?,term_id:row.get(4)?,payload_len:row.get(5)?,
                    payload_sha256:row.get(6)?,payload:row.get(7)?};
                if carrier.position != expected { return Err(Error::Invalid("public D1 knowledge order")); }
                let charge = carrier.payload.as_ref().map_or(0,Vec::len) as u64;
                capture.charge_work(charge)?;
                emit_normalized_row(capture,sink,&mut documents,lens_counts,max_lens_bytes,
                    max_lens_memberships,kind,carrier,root,limits)?;
                expected = expected.checked_add(1).ok_or(Error::Budget("public D1 knowledge rows"))?;
            }
            let target = if table == "knowledge_nodes" {&mut counts.nodes} else {&mut counts.relations};
            *target = expected as u64;
            Ok(())
        })?;
    }
    emit_search(stage, capture, sink, counts)
}

fn emit_search(
    stage: &mut KnowledgeStage<'_>,
    capture: &PublicCapture,
    sink: &mut SqlSink,
    counts: &mut KnowledgeSqlCounts,
) -> Result<()> {
    stage.with_connection(WritePhase::Search, |db| {
        let mut documents = db.prepare("SELECT kind,position,id,source_graph,kind_id,predicate_id,id_lower,native_id_lower,identity_values,visible_values,document_chars,document_digest FROM search_documents ORDER BY kind,position")?;
        let mut rows = documents.query([])?;
        while let Some(row) = rows.next()? {
            let kind:String=row.get(0)?; let position:i64=row.get(1)?;
            let fields:Vec<String> = (2..10).map(|column|row.get(column)).collect::<std::result::Result<_,_>>()?;
            let chars:i64=row.get(10)?; let digest:Vec<u8>=row.get(11)?;
            if chars<0 || digest.len()!=32 {return Err(Error::Invalid("public D1 search document"));}
            capture.charge_work(fields.iter().map(String::len).sum::<usize>() as u64)?;
            let mut values=vec![bounded_quote(capture,&kind)?,position.to_string()];
            for field in &fields { values.push(bounded_quote(capture,field)?); }
            values.push(chars.to_string());
            values.push(bounded_quote(capture,&Digest256::from_bytes(digest.try_into().map_err(|_|Error::Invalid("public D1 search digest"))?).to_hex())?);
            sink.insert_chunked("knowledge_search_documents_next",&["kind","position","id","source_graph","kind_id","predicate_id","id_lower","native_id_lower","identity_values","visible_values","document_chars","document_digest"],&values,
                &format!("kind={} AND position={position}",quote(&kind)?),&[])?;
        }
        drop(rows); drop(documents);
        let mut posting = db.prepare("SELECT kind,n,gram,first_position,last_position,postings,CASE WHEN length(deltas)<=?1 THEN deltas ELSE NULL END FROM search_posting_blocks ORDER BY kind,n,gram,last_position")?;
        let mut rows = posting.query([MAX_POSTING_DELTA_BYTES as i64])?;
        let mut previous:Option<(String,Vec<u8>,u64)>=None;
        while let Some(row)=rows.next()? {
            let kind:String=row.get(0)?; let n:i64=row.get(1)?; let gram:Vec<u8>=row.get(2)?;
            let first:i64=row.get(3)?; let last:i64=row.get(4)?; let count:i64=row.get(5)?;
            let deltas:Option<Vec<u8>>=row.get(6)?;
            if n!=3 || first<0 || last<0 || count<1 || count>256 {return Err(Error::Invalid("public D1 posting block"));}
            let deltas=deltas.ok_or(Error::Budget("public D1 posting block bytes"))?;
            capture.charge_work((gram.len()+deltas.len()) as u64)?;
            let positions=decode_posting_block(first as u64,last as u64,count as u16,&deltas)?;
            capture.charge_work((positions.len()*8) as u64)?;
            let gram_text=std::str::from_utf8(&gram).map_err(|_|Error::Invalid("public D1 gram UTF-8"))?;
            if gram_text.chars().count()!=3 {return Err(Error::Invalid("public D1 gram width"));}
            if let Some((old_kind,old_gram,old_last))=&previous {
                if old_kind==&kind && old_gram==&gram && *old_last>=positions[0] {return Err(Error::Invalid("public D1 posting order"));}
            }
            for position in positions {
                sink.insert("knowledge_search_grams_next", &["kind","n","gram","position"],
                    &[bounded_quote(capture,&kind)?,"3".to_owned(),bounded_quote(capture,gram_text)?,position.to_string()])?;
                counts.postings=counts.postings.checked_add(1).ok_or(Error::Budget("public D1 postings"))?;
            }
            previous=Some((kind,gram,last as u64));
        }
        drop(rows); drop(posting);
        let mut stats=db.prepare("SELECT kind,n,gram,postings FROM search_gram_stats ORDER BY kind,n,gram")?;
        let mut rows=stats.query([])?;
        let mut sum=0u64;
        while let Some(row)=rows.next()? {
            let kind:String=row.get(0)?; let n:i64=row.get(1)?; let gram:Vec<u8>=row.get(2)?; let postings:i64=row.get(3)?;
            if n!=3 || postings<1 {return Err(Error::Invalid("public D1 gram stats"));}
            let gram=std::str::from_utf8(&gram).map_err(|_|Error::Invalid("public D1 gram UTF-8"))?;
            sink.insert("knowledge_search_gram_stats_next", &["kind","n","gram","postings"],
                &[bounded_quote(capture,&kind)?,"3".to_owned(),bounded_quote(capture,gram)?,postings.to_string()])?;
            counts.distinct_grams=counts.distinct_grams.checked_add(1).ok_or(Error::Budget("public D1 distinct grams"))?;
            sum=sum.checked_add(postings as u64).ok_or(Error::Budget("public D1 gram total"))?;
        }
        if sum!=counts.postings {return Err(Error::Invalid("public D1 posting/stat coverage"));}
        Ok(())
    })
}

#[cfg(test)]
mod private_projection_tests {
    use super::*;
    fn limits() -> SearchBuildLimits {
        SearchBuildLimits {
            max_payload_bytes: 1_000_000,
            max_document_chars: 1_000_000,
            max_document_bytes: 4_000_000,
            max_rank_field_bytes: 1_000_000,
            max_postings: 100_000,
            max_work_bytes: 8_000_000,
            gram_batch_rows: 128,
        }
    }
    #[test]
    fn private_overflow_uses_maintained_payload_chunks_and_search_overlap() {
        let raw = serde_json::json!({"id":"overflow", "source_graph":"fixture", "kind_id":"concept",
            "display":{"title":{"default":"Overflow"}}, "attributes":{"padding":"'".repeat(1_010_000)}}).to_string();
        let mut caps = limits();
        caps.max_payload_bytes = 3_000_000;
        caps.max_document_chars = 4_000_000;
        caps.max_document_bytes = 16_000_000;
        caps.max_work_bytes = 128_000_000;
        let projected = project_private_knowledge_row("node", 0, &raw, "/fixture", caps).unwrap();
        let base = projected
            .iter()
            .find(|row| row.table == D1Table::KnowledgeNodes)
            .unwrap()
            .after
            .as_ref()
            .unwrap();
        assert_eq!(
            &base[8..],
            &[D1Cell::Text(String::new()), D1Cell::Text(String::new())]
        );
        let payload = projected
            .iter()
            .filter(|row| row.table == D1Table::EdgeMeta)
            .filter_map(|row| row.after.as_ref())
            .filter(|row| row[0] == D1Cell::Text("knowledge_node_payload:overflow".into()))
            .map(|row| match &row[2] {
                D1Cell::Text(value) => value.as_str(),
                _ => panic!("payload type"),
            })
            .collect::<String>();
        assert_eq!(payload, raw);
        let search_parts = projected
            .iter()
            .filter(|row| row.table == D1Table::EdgeMeta)
            .filter_map(|row| row.after.as_ref())
            .filter(|row| row[0] == D1Cell::Text("knowledge_node_search:overflow".into()))
            .map(|row| match &row[2] {
                D1Cell::Text(value) => value.as_str(),
                _ => panic!("search type"),
            })
            .collect::<Vec<_>>();
        assert!(!search_parts.is_empty());
        for pair in search_parts.windows(2) {
            let trailing = pair[0]
                .chars()
                .rev()
                .take(1023)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<String>();
            assert!(pair[1].starts_with(&trailing));
        }
    }
    #[test]
    fn private_postings_and_work_caps_are_enforced_by_the_projector() {
        let raw =
            serde_json::json!({"id":"node-abcdef","source_graph":"fixture", "kind_id":"concept",
            "display":{"title":{"default":"Finite indexed source"}}, "attributes":{}})
            .to_string();
        let rows = project_private_knowledge_row("node", 0, &raw, "/fixture", limits()).unwrap();
        assert!(
            rows.iter()
                .any(|row| row.table == D1Table::KnowledgeSearchGrams)
        );
        let mut posting_limit = limits();
        posting_limit.max_postings = 1;
        assert!(matches!(
            project_private_knowledge_row("node", 0, &raw, "/fixture", posting_limit),
            Err(Error::Budget("private D1 search projection postings"))
        ));
        let mut work_limit = limits();
        work_limit.max_work_bytes = 1;
        assert!(matches!(
            project_private_knowledge_row("node", 0, &raw, "/fixture", work_limit),
            Err(Error::Budget("private D1 search projection work"))
        ));
    }
}
