//! Public v9 knowledge rows from the disposable normalized stage. The
//! selected block index stays private; D1 receives its maintained expanded
//! SQL transport with the same logical postings and rank documents.

use crate::{
    Error, Result,
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
use tos_foundation::{Digest256, JsonValue};

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
            let fragment = &search[start..end];
            sink.insert(
                "edge_meta_next",
                &["key", "part", "json_chunk"],
                &[
                    bounded_quote(capture, key)?,
                    part.to_string(),
                    bounded_quote(capture, fragment)?,
                ],
            )?;
            part += 1;
            start = *recent
                .front()
                .ok_or(Error::Invalid("public D1 search fragment"))?;
        }
    }
    if count % 8192 != 0 {
        sink.insert(
            "edge_meta_next",
            &["key", "part", "json_chunk"],
            &[
                bounded_quote(capture, key)?,
                part.to_string(),
                bounded_quote(capture, &search[start..])?,
            ],
        )?;
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
