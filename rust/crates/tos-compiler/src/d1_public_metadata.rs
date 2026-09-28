//! Small, emitted-byte reader metadata for the disposable D1 publication.
//! The serving clock is supplied by the migration after SQL import.

use crate::{
    Error, Result,
    d1_public_capture::{PublicCapture, compact, json},
    d1_public_rows::portable,
    d1_public_sql::{SqlSink, chunks, quote},
    knowledge_stage::{KnowledgeStage, WritePhase},
};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::Path;
use tos_foundation::{Digest256, Digest256Hasher};

const SCHEMA: &str = "tos_cloudflare_edge_read_model_v9";
const CONTENT: &str = "tos_cloudflare_edge_content_v5";
const SEARCH: &str = "tos_knowledge_search_read_model_v3";
const PRODUCER: &str = "tos-rust-public-d1-full-v1";

fn encoded(value: &Value, cap: usize) -> Result<String> {
    let raw = serde_json::to_string(value).map_err(|e| Error::Source(e.to_string()))?;
    if raw.len() > cap {
        return Err(Error::Budget("public D1 metadata bytes"));
    }
    Ok(raw)
}

fn histogram(db: &Connection, table: &str, fields: &str) -> Result<Vec<Value>> {
    let sql = format!("SELECT {fields},count(*) FROM {table} GROUP BY {fields} ORDER BY {fields}");
    let mut stmt = db.prepare(&sql)?;
    let mut rows = stmt.query([])?;
    let mut result = Vec::new();
    while let Some(row) = rows.next()? {
        if result.len() >= 16_384 {
            return Err(Error::Budget("public D1 lens count cells"));
        }
        let a: String = row.get(0)?;
        let b: String = row.get(1)?;
        let c: String = row.get(2)?;
        let n: i64 = row.get(3)?;
        if a.is_empty() || b.is_empty() || c.is_empty() || n <= 0 || n > 9_007_199_254_740_991 {
            return Err(Error::Invalid("public D1 lens count cell"));
        }
        result.push(json!([a, b, c, n]));
    }
    Ok(result)
}

pub(crate) struct PublicMetadata {
    pub(crate) revision: String,
    pub(crate) reader_top: Value,
    pub(crate) reader_top_raw: String,
    pub(crate) lens: Value,
    pub(crate) binding_prefix: String,
    pub(crate) binding_suffix: String,
}

pub(crate) fn prepare(
    stage: &mut KnowledgeStage<'_>,
    capture: &PublicCapture,
    root: &Path,
    header: &Value,
    catalog: &Value,
    source_revision: &str,
) -> Result<PublicMetadata> {
    if !stage.public_build()
        || header["source_revision"] != source_revision
        || catalog["source_revision"] != source_revision
        || catalog["schema"] != "tos_knowledge_catalog_v1"
    {
        return Err(Error::Invalid("public D1 metadata source binding"));
    }
    let (nodes, relations) = stage.with_connection(WritePhase::Finalize, |db| {
        Ok((
            histogram(db, "knowledge_nodes", "source_graph,kind_id,type_id")?,
            histogram(
                db,
                "knowledge_relations",
                "source_graph,predicate_id,relation_type_id",
            )?,
        ))
    })?;
    let lens = normalized(
        capture,
        root,
        &json!({
            "schema":"tos_published_lens_metadata_v1", "execution_version":"tos-lens-execution-v7",
            "source_revision":source_revision, "sort_key":"python-str-or-empty-lower-v1",
            "unicode_version":"16.0.0", "query_properties":header["query_properties"],
            "node_counts":nodes, "relation_counts":relations,
        }),
    )?;
    let lens_raw = encoded(&lens, 1_048_576)?;
    // These digests bind the exact path-portable values emitted to edge_meta,
    // not the pre-portable Stage values held in this private producer.
    let portable_catalog = normalized(capture, root, catalog)?;
    let portable_header = normalized(capture, root, header)?;
    let catalog_raw = encoded(&portable_catalog, 16 * 1024 * 1024)?;
    let header_raw = encoded(&portable_header, 16 * 1024 * 1024)?;
    let mut hash = Digest256Hasher::new();
    for text in [SCHEMA, CONTENT, SEARCH, PRODUCER, source_revision] {
        hash.update(text.as_bytes());
        hash.update(&[0]);
    }
    capture.update_revision_sources(&mut hash)?;
    for raw in [&header_raw, &catalog_raw, &lens_raw] {
        capture.charge_work(raw.len() as u64)?;
        hash.update(&(raw.len() as u64).to_be_bytes());
        hash.update(raw.as_bytes());
    }
    let revision = hash.finalize().to_hex();
    let reader_top = json!({
        "schema":"tos_published_knowledge_reader_v2", "read_model_schema":SCHEMA,
        "source_revision":source_revision, "data_revision":revision,
        "graph_schema":portable_header["schema"], "normalization_binding":portable_header["normalization_binding"],
        "catalog_sha256":Digest256::of_bytes(catalog_raw.as_bytes()).to_hex(),
        "row_integrity":"sha256-emitted-json-v1", "authority_boundary":portable_header["authority_boundary"],
        "lens_sha256":Digest256::of_bytes(lens_raw.as_bytes()).to_hex(),
    });
    // Python's compact binding uses insertion order. Construct that framing
    // explicitly, so SQLite can insert the actual serving epoch between two
    // static, hash-bound fragments without a caller-chosen epoch.
    let top_raw = format!(
        "{{\"schema\":\"tos_published_knowledge_reader_v2\",\"read_model_schema\":{schema},\"source_revision\":{source},\"data_revision\":{revision},\"graph_schema\":{graph},\"normalization_binding\":{normalization},\"catalog_sha256\":{catalog},\"row_integrity\":\"sha256-emitted-json-v1\",\"authority_boundary\":{boundary},\"lens_sha256\":{lens}}}",
        schema = encoded(&json!(SCHEMA), 128)?,
        source = encoded(&json!(source_revision), 128)?,
        revision = encoded(&json!(revision), 128)?,
        graph = encoded(&portable_header["schema"], 128)?,
        normalization = encoded(&portable_header["normalization_binding"], 4096)?,
        catalog = encoded(&reader_top["catalog_sha256"], 128)?,
        boundary = encoded(&portable_header["authority_boundary"], 4096)?,
        lens = encoded(&reader_top["lens_sha256"], 128)?,
    );
    if top_raw.len() > 131_072 {
        return Err(Error::Budget("public D1 reader top bytes"));
    }
    capture.charge_work(top_raw.len() as u64)?;
    let metadata_digest = Digest256::of_bytes(top_raw.as_bytes()).to_hex();
    let binding_prefix =
        "{\"schema\":\"tos_published_knowledge_snapshot_v1\",\"publication_epoch\":".to_owned();
    let binding_suffix = format!(
        ",\"metadata_sha256\":{metadata},\"read_model_schema\":{schema},\"source_revision\":{source},\"data_revision\":{revision},\"graph_schema\":{graph},\"normalization_binding\":{normalization}}}",
        metadata = encoded(&json!(metadata_digest), 128)?,
        schema = encoded(&json!(SCHEMA), 128)?,
        source = encoded(&json!(source_revision), 128)?,
        revision = encoded(&json!(revision), 128)?,
        graph = encoded(&portable_header["schema"], 128)?,
        normalization = encoded(&portable_header["normalization_binding"], 4096)?,
    );
    Ok(PublicMetadata {
        revision,
        reader_top,
        reader_top_raw: top_raw,
        lens,
        binding_prefix,
        binding_suffix,
    })
}

pub(crate) fn emit_edge_meta(
    sink: &mut SqlSink,
    capture: &PublicCapture,
    values: &BTreeMap<String, String>,
) -> Result<()> {
    for (key, raw) in values {
        if raw.len() > 16 * 1024 * 1024 {
            return Err(Error::Budget("public D1 metadata bytes"));
        }
        capture.charge_work(raw.len() as u64)?;
        for (part, chunk) in chunks(&raw).enumerate() {
            sink.insert(
                "edge_meta_next",
                &["key", "part", "json_chunk"],
                &[quote(key)?, part.to_string(), quote(chunk)?],
            )?;
        }
    }
    Ok(())
}

fn normalized(capture: &PublicCapture, root: &Path, value: &Value) -> Result<Value> {
    let raw = serde_json::to_vec(value).map_err(|e| Error::Source(e.to_string()))?;
    if raw.len() > 16 * 1024 * 1024 {
        return Err(Error::Budget("public D1 normalized metadata input"));
    }
    capture.charge_work(raw.len() as u64)?;
    let mut graph = json(&raw, 16 * 1024 * 1024)?;
    portable(
        &mut graph,
        root.to_str()
            .ok_or(Error::Invalid("public D1 root UTF-8"))?,
    );
    let output = compact(&graph, 16 * 1024 * 1024)?;
    capture.charge_work(output.len() as u64)?;
    serde_json::from_slice(&output).map_err(|e| Error::Source(e.to_string()))
}

fn positions(capture: &PublicCapture, role: &str, collection: &str, key: &str) -> Result<Value> {
    let mut map = serde_json::Map::new();
    capture.visit_rows(role, collection, |position, raw| {
        if position >= 63 {
            return Err(Error::Budget("public D1 view/layer positions"));
        }
        let value: Value = serde_json::from_slice(raw).map_err(|e| Error::Source(e.to_string()))?;
        if let Some(id) = value.get(key).and_then(Value::as_str) {
            if map.insert(id.to_owned(), json!(position)).is_some() {
                return Err(Error::Invalid("public D1 duplicate view/layer"));
            }
        }
        Ok(())
    })?;
    Ok(Value::Object(map))
}

pub(crate) fn values(
    capture: &PublicCapture,
    root: &Path,
    header: &Value,
    catalog: &Value,
    metadata: &PublicMetadata,
    navigation: &Value,
) -> Result<BTreeMap<String, String>> {
    let source_revision = header["source_revision"]
        .as_str()
        .ok_or(Error::Invalid("public D1 source revision"))?;
    if source_revision != metadata.reader_top["source_revision"] {
        return Err(Error::Invalid("public D1 metadata revision"));
    }
    let mut values = BTreeMap::new();
    let mut put = |key: &str, value: Value| -> Result<()> {
        let raw = encoded(&normalized(capture, root, &value)?, 16 * 1024 * 1024)?;
        if values.insert(key.to_owned(), raw).is_some() {
            return Err(Error::Invalid("public D1 duplicate metadata key"));
        }
        Ok(())
    };
    put("data_revision", json!({"sha256":metadata.revision}))?;
    put(
        "view_positions",
        positions(capture, "philosophy", "views", "view_id")?,
    )?;
    put(
        "layer_positions",
        positions(capture, "philosophy", "graph_layers", "layer_id")?,
    )?;
    put(
        "philosophy_top",
        capture.header_object("philosophy", "", 2 * 1024 * 1024)?,
    )?;
    put(
        "corpus_top",
        capture.header_object("corpus", "", 2 * 1024 * 1024)?,
    )?;
    for (key, path) in [
        (
            "evidence_projection",
            "ToS/derived-exports/epistemic_evidence_projection.min.json",
        ),
        (
            "philosophy_audit",
            "ToS/philosophy/graph-workbench/review-packets/table-i-post-planting-audit.json",
        ),
    ] {
        let value = match capture.read_input(path, 2 * 1024 * 1024)? {
            Some(raw) => serde_json::from_slice(&raw).map_err(|e| Error::Source(e.to_string()))?,
            None => json!({}),
        };
        put(key, value)?;
    }
    put(
        "word_analysis_capability",
        json!({
            "schema":"tos_zarathustra_word_analysis_capability_v1","available":false,
            "reason":"local source-bound word-analysis provider is excluded from the public bundle",
            "provider_ref":"scripts/prepare_zarathustra_word_analysis_v1.py",
            "publication_posture":"excluded_from_public_bundle","task":null,
            "authority":{"source_owner":"Tree-of-Sophia","access_plane_is_source":false,
                "is_semantic_truth":false,"writes_to_tree":false,"reviewed":false,"canon":false}
        }),
    )?;
    put("knowledge_top", header.clone())?;
    put(
        "knowledge_exploration_top",
        json!({"source_revision":source_revision,
        "authority_boundary":header["authority_boundary"]}),
    )?;
    put(
        "knowledge_search_top",
        json!({"schema":SEARCH,"source_revision":source_revision,
        "ngram_size":3,"matching_counts":"unknown-until-indexed-page-exhaustion"}),
    )?;
    put("knowledge_catalog", catalog.clone())?;
    put("knowledge_lens_top", metadata.lens.clone())?;
    put("source_navigation_top", navigation.clone())?;
    // Keep the reader top in its exact compact lexical order: its SHA is the
    // publication binding, while Value's map storage may reorder members.
    values.insert(
        "knowledge_reader_top".to_owned(),
        metadata.reader_top_raw.clone(),
    );
    let nav_raw = values
        .get("source_navigation_top")
        .ok_or(Error::Invalid("public D1 navigation header"))?;
    values.insert(
        "source_navigation_header_digest".to_owned(),
        encoded(
            &json!({"sha256":Digest256::of_bytes(nav_raw.as_bytes()).to_hex()}),
            1024,
        )?,
    );
    Ok(values)
}
