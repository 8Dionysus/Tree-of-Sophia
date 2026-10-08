//! Whole v1 Reading producer. Candidate semantics and frozen rendering recipe
//! remain subordinate to source, policy and review owners.
use crate::research_eternal_return::{bytes, digest, lines, load};
use crate::research_execution::ResearchExecution;
use rusqlite::{
    Connection, params_from_iter,
    types::{Value as SqlValue, ValueRef},
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    os::unix::fs::PermissionsExt,
    path::Path,
};
type Result<T> = std::result::Result<T, String>;
const METHOD: &str = "zarathustra-reading-workbench-v1";
const WORK: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra";
const ROUTE: &str = "ToS/candidate-intake/zarathustra/reading-workbench-v1";
const IMPLEMENTATIONS: [(&str, &str); 4] = [
    (
        "scripts/build_zarathustra_reading_workbench_v1.py",
        "56aa0e4313680e15de01793849fb050dfb43fb8ff2be39069487e0714d038975",
    ),
    (
        "scripts/zarathustra_discourse.py",
        "d2a66e5851adff49b31ffc857db4da4a0c0f702d3ed16e8c64b5552c2b061dee",
    ),
    (
        "scripts/zarathustra_voice_policy.py",
        "0261e798f3697e4836f07783b433978eb3131396dd47f34e7132b32ac2763388",
    ),
    (
        "scripts/zarathustra_recurring_formulas.py",
        "cc80e480268e7916662f4a214c9f69fa3400f45eec2ac9a2a0f2cd7e21fa0e9d",
    ),
];
fn s<'a>(v: &'a Value, k: &str) -> Result<&'a str> {
    v[k].as_str()
        .ok_or_else(|| format!("reading string required: {k}"))
}
fn n(v: &Value, k: &str) -> Result<u64> {
    v[k].as_u64()
        .ok_or_else(|| format!("reading unsigned integer required: {k}"))
}
fn arr<'a>(v: &'a Value, k: &str) -> Result<&'a Vec<Value>> {
    v[k].as_array()
        .ok_or_else(|| format!("reading array required: {k}"))
}
fn compact(v: &Value) -> Result<String> {
    let mut v = v.clone();
    v.sort_all_objects();
    serde_json::to_string(&v).map_err(|e| e.to_string())
}
fn sha(s: &str) -> String {
    digest(s.as_bytes())
}
fn private_ref(name: &str) -> String {
    format!("{WORK}/gold-sets/foundation-pilot-v1/local-content/{name}")
}
fn database_ref() -> String {
    private_ref("reading-workbench-v1/reading-workbench.v1.sqlite3")
}
fn slice(text: &str, start: u64, end: u64) -> Result<String> {
    let c: Vec<_> = text.chars().collect();
    let a = usize::try_from(start).map_err(|e| e.to_string())?;
    let b = usize::try_from(end).map_err(|e| e.to_string())?;
    c.get(a..b)
        .map(|v| v.iter().collect())
        .ok_or("reading span outside context".into())
}
fn quick_check(root: &ResearchExecution, db: &Connection) -> Result<()> {
    root.check()?;
    let result: String = db
        .query_row("PRAGMA quick_check", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    root.check()?;
    if result != "ok" {
        return Err("private input quick_check failed".into());
    }
    Ok(())
}
fn query(root: &ResearchExecution, db: &Connection, sql: &str) -> Result<Vec<Value>> {
    root.check()?;
    let mut statement = db.prepare(sql).map_err(|e| e.to_string())?;
    let names: Vec<_> = statement
        .column_names()
        .iter()
        .map(|s| s.to_string())
        .collect();
    let mut rows = statement.query([]).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().map_err(|e| e.to_string())? {
        root.tick(names.len() as u64)?;
        let mut v = serde_json::Map::new();
        for (i, k) in names.iter().enumerate() {
            let x = match row.get_ref(i).map_err(|e| e.to_string())? {
                ValueRef::Null => Value::Null,
                ValueRef::Integer(n) => json!(n),
                ValueRef::Text(b) => json!(std::str::from_utf8(b).map_err(|e| e.to_string())?),
                _ => return Err("unexpected reading SQLite scalar".into()),
            };
            v.insert(k.clone(), x);
        }
        out.push(Value::Object(v));
    }
    root.check()?;
    Ok(out)
}
struct Input {
    file: File,
    metadata: std::fs::Metadata,
    hash: String,
    reference: String,
    manifest_file: File,
    manifest_metadata: std::fs::Metadata,
    manifest_hash: String,
}
fn load_input(
    root: &ResearchExecution,
    reference: &str,
    manifest_ref: &str,
    role: &str,
) -> Result<(Input, Value)> {
    let mut manifest_file = root.source_file(manifest_ref, 8 * 1024 * 1024)?;
    let manifest_metadata = manifest_file.metadata().map_err(|e| e.to_string())?;
    let raw = root.read_file(&mut manifest_file, 8 * 1024 * 1024)?;
    let manifest_hash = digest(&raw);
    let manifest: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    root.verify_file_unchanged(&manifest_file, &manifest_metadata)?;
    let expected = arr(&manifest, "private_artifacts")?
        .iter()
        .find(|x| x["ref"] == reference)
        .ok_or("private input absent from manifest")?;
    let mut file = root.source_file(reference, 512 * 1024 * 1024)?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    if metadata.permissions().mode() & 0o7777 != 0o600 {
        return Err("expected mode-0600 regular private input".into());
    }
    let hash = root.hash_file(&mut file, 512 * 1024 * 1024)?;
    if expected["sha256"] != hash {
        return Err(format!("predecessor fixity drift: {role}"));
    }
    root.verify_file_unchanged(&file, &metadata)?;
    let evidence = json!({"ref":reference,"sha256":hash,"role":role,"manifest_ref":manifest_ref,"manifest_sha256":digest(&raw)});
    Ok((
        Input {
            file,
            metadata,
            hash,
            reference: reference.into(),
            manifest_file,
            manifest_metadata,
            manifest_hash,
        },
        evidence,
    ))
}
fn validate_policies(
    root: &ResearchExecution,
    contexts: &[Value],
    policies: &Value,
) -> Result<u64> {
    let mut grouped: BTreeMap<(String, String), Vec<&Value>> = BTreeMap::new();
    for c in contexts {
        root.tick(1)?;
        grouped
            .entry((s(c, "language")?.into(), s(c, "reading_ref")?.into()))
            .or_default()
            .push(c)
    }
    let mut checked = 0;
    for chapter in arr(policies, "chapters")? {
        let reading = s(chapter, "reading_ref")?;
        for (lang, witness) in chapter["witnesses"]
            .as_object()
            .ok_or("policy witnesses required")?
        {
            let rows = grouped
                .get(&(lang.clone(), reading.into()))
                .ok_or("policy chapter missing")?;
            let text = rows
                .iter()
                .map(|c| s(c, "exact_text"))
                .collect::<Result<Vec<_>>>()?
                .concat();
            root.tick(text.chars().count() as u64)?;
            if rows.len() as u64 != n(witness, "context_count")?
                || sha(&text) != s(witness, "chapter_exact_sha256")?
            {
                return Err("chapter voice policy source drift".into());
            }
        }
        for rule in arr(chapter, "overrides")? {
            root.tick(1)?;
            if !matches!(s(rule, "status")?, "proposed" | "ambiguous" | "deferred") {
                return Err("voice rule unexpectedly accepted".into());
            }
            let rows = grouped
                .get(&(s(rule, "language")?.into(), reading.into()))
                .ok_or("policy rule chapter missing")?;
            if rule.get("start_context_ref").is_some() {
                let a = rows
                    .iter()
                    .position(|c| c["context_unit_ref"] == rule["start_context_ref"])
                    .ok_or("policy start absent")?;
                let b = rows
                    .iter()
                    .position(|c| c["context_unit_ref"] == rule["end_context_ref"])
                    .ok_or("policy end absent")?;
                let selected = rows.get(a..=b).ok_or("voice policy range extent drift")?;
                if selected.len() as u64 != n(rule, "context_count")? || selected.first().is_none()
                {
                    return Err("voice policy range extent drift".into());
                }
                if selected[0]["exact_sha256"] != rule["start_context_exact_sha256"]
                    || selected[selected.len() - 1]["exact_sha256"]
                        != rule["end_context_exact_sha256"]
                {
                    return Err("voice policy endpoint fixity drift".into());
                }
                let text = selected
                    .iter()
                    .map(|c| s(c, "exact_text"))
                    .collect::<Result<Vec<_>>>()?
                    .concat();
                root.tick(text.chars().count() as u64)?;
                if sha(&text) != s(rule, "full_range_sha256")? {
                    return Err("voice policy range fixity drift".into());
                }
            }
            checked += 1;
        }
    }
    Ok(checked)
}
fn crosswalk(
    root: &ResearchExecution,
    contexts: &[Value],
    surfaces: &[Value],
    occurrences: &[Value],
) -> Result<(Vec<Value>, Vec<Value>)> {
    let by: BTreeMap<_, _> = contexts
        .iter()
        .map(|c| Ok((s(c, "context_unit_ref")?.to_owned(), c)))
        .collect::<Result<_>>()?;
    let spans: BTreeMap<_, _> = surfaces
        .iter()
        .map(|r| {
            Ok((
                (
                    s(r, "context_unit_ref")?.to_owned(),
                    n(r, "start_offset")?,
                    n(r, "end_offset")?,
                ),
                r["surface_unit_id"].clone(),
            ))
        })
        .collect::<Result<_>>()?;
    // Python insertion order of first encountered context is semantically visible.
    let mut groups: Vec<(String, Vec<&Value>)> = Vec::new();
    let mut gap = Vec::new();
    for r in occurrences {
        root.tick(1)?;
        if r["language"] != "de"
            || r["in_work_scope"].as_i64() == Some(0)
            || r["in_work_scope"] == false
        {
            continue;
        }
        let Some(reference) = r["context_unit_ref"].as_str() else {
            gap.push(json!({"kind":"legacy_occurrence_context_unmapped","occurrence_ref":r["existing_occurrence_ref"],"status":"deferred"}));
            continue;
        };
        if !by.contains_key(reference) {
            gap.push(json!({"kind":"legacy_occurrence_context_unmapped","occurrence_ref":r["existing_occurrence_ref"],"status":"deferred"}));
            continue;
        }
        if let Some((_, rows)) = groups.iter_mut().find(|(k, _)| k == reference) {
            rows.push(r)
        } else {
            groups.push((reference.into(), vec![r]))
        }
    }
    let mut mappings = Vec::new();
    for (reference, mut rows) in groups {
        rows.sort_by_key(|r| r["token_ordinal"].as_u64().unwrap_or(0));
        let text = s(by[&reference], "exact_text")?;
        let mut cursor = 0u64;
        for r in rows {
            let form = s(r, "exact_form")?;
            let suffix = slice(text, cursor, text.chars().count() as u64)?;
            root.tick(suffix.chars().count() as u64)?;
            let found = suffix
                .find(form)
                .map(|byte| cursor + suffix[..byte].chars().count() as u64);
            if found.is_none() || sha(form) != s(r, "exact_form_sha256")? {
                gap.push(json!({"kind":"legacy_occurrence_exact_crosswalk_failed","context_unit_ref":reference,"occurrence_ref":r["existing_occurrence_ref"],"status":"deferred"}));
                continue;
            }
            let start = found.unwrap();
            let end = start + form.chars().count() as u64;
            mappings.push(json!({"existing_occurrence_ref":r["existing_occurrence_ref"],"context_unit_ref":reference,"surface_unit_ref":spans.get(&(reference.clone(),start,end)),"start_offset":start,"end_offset":end,"exact_text":form,"exact_sha256":sha(form),"status":"proposed"}));
            cursor = end;
        }
    }
    Ok((mappings, gap))
}
fn sql_value(v: &Value) -> Result<SqlValue> {
    Ok(match v {
        Value::Null => SqlValue::Null,
        Value::Bool(b) => SqlValue::Integer(i64::from(*b)),
        Value::Number(n) => SqlValue::Integer(n.as_i64().ok_or("SQLite integer out of range")?),
        Value::String(s) => SqlValue::Text(s.clone()),
        _ => return Err("reading SQL scalar required".into()),
    })
}
fn insert(
    root: &ResearchExecution,
    db: &Connection,
    table: &str,
    rows: &[Vec<Value>],
) -> Result<()> {
    if rows.is_empty() {
        return Ok(());
    }
    let arity = rows[0].len();
    let mut st = db
        .prepare(&format!(
            "INSERT INTO {table} VALUES({})",
            vec!["?"; arity].join(",")
        ))
        .map_err(|e| e.to_string())?;
    for row in rows {
        root.tick(row.len() as u64)?;
        let values = row.iter().map(sql_value).collect::<Result<Vec<_>>>()?;
        st.execute(params_from_iter(values))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
fn fields(rows: &[Value], keys: &[&str]) -> Result<Vec<Vec<Value>>> {
    rows.iter()
        .map(|r| {
            keys.iter()
                .map(|k| {
                    r.get(*k)
                        .cloned()
                        .ok_or_else(|| format!("missing reading SQL field {k}"))
                })
                .collect()
        })
        .collect()
}
struct Data {
    contexts: Vec<Value>,
    sentences: Vec<Value>,
    clauses: Vec<Value>,
    alignments: Vec<Value>,
    segments: Vec<Value>,
    events: Vec<Value>,
    mappings: Vec<Value>,
    families: Vec<Value>,
    memberships: Vec<Value>,
    relations: Vec<Value>,
}
fn database(
    root: &ResearchExecution,
    data: &Data,
    metadata: &BTreeMap<String, String>,
) -> Result<Vec<u8>> {
    const MIB: u64 = 1024 * 1024;
    let mut scope = root.sqlite_scope(tos_source_store::PinnedSqliteAuxLimits {
        main_logical_bytes: 256 * MIB,
        main_allocated_bytes: 256 * MIB,
        temp_db_logical_bytes: 256 * MIB,
        temp_db_allocated_bytes: 256 * MIB,
        main_journal_logical_bytes: 0,
        main_journal_allocated_bytes: 0,
        temp_journal_logical_bytes: 4 * MIB,
        temp_journal_allocated_bytes: 4 * MIB,
        other_aux_aggregate_logical_bytes: 8 * MIB,
        other_aux_aggregate_allocated_bytes: 8 * MIB,
        max_live_aux: 8,
    })?;
    let recipe = (|| -> Result<()> {
        let mut db = scope
            .scope_mut()
            .open_connection()
            .map_err(|e| e.to_string())?;
        let deadline = root.deadline();
        db.progress_handler(1000, Some(move || std::time::Instant::now() >= deadline));
        // Nonpersistent pager cache target; VACUUM may use another 96 MiB.
        // The outer RAM envelope remains binding and this is not a fit claim.
        db.execute_batch("PRAGMA main.cache_size=-98304")
            .map_err(|e| e.to_string())?;
        let cache: i64 = db
            .query_row("PRAGMA main.cache_size", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        if cache != -98304 {
            return Err("Reading pager cache readback mismatch".into());
        }
        db.execute_batch(include_str!("research_reading_workbench.sql"))
            .map_err(|e| e.to_string())?;
        let tx = db.transaction().map_err(|e| e.to_string())?;
        insert(
            root,
            &tx,
            "metadata",
            &metadata
                .iter()
                .map(|(k, v)| vec![json!(k), json!(v)])
                .collect::<Vec<_>>(),
        )?;
        insert(
            root,
            &tx,
            "contexts",
            &fields(
                &data.contexts,
                &[
                    "context_unit_ref",
                    "language",
                    "part",
                    "reading_ref",
                    "unit_kind",
                    "witness_order",
                    "exact_text",
                    "exact_sha256",
                    "anchor_refs_json",
                ],
            )?,
        )?;
        insert(
            root,
            &tx,
            "source_sentences",
            &fields(
                &data.sentences,
                &[
                    "sentence_id",
                    "context_unit_ref",
                    "start_offset",
                    "end_offset",
                    "exact_text",
                    "exact_sha256",
                ],
            )?,
        )?;
        insert(
            root,
            &tx,
            "source_clauses",
            &fields(
                &data.clauses,
                &[
                    "clause_id",
                    "sentence_id",
                    "context_unit_ref",
                    "start_offset",
                    "end_offset",
                    "exact_text",
                    "exact_sha256",
                    "boundary_status",
                ],
            )?,
        )?;
        insert(
            root,
            &tx,
            "translation_alignments",
            &fields(
                &data.alignments,
                &[
                    "alignment_id",
                    "claim_id",
                    "granularity",
                    "part",
                    "parent_paragraph_alignment_ref",
                    "parent_sentence_alignment_ref",
                    "candidate_role",
                    "correspondence_shape",
                    "ordered_source_unit_refs_json",
                    "ordered_target_unit_refs_json",
                    "exact_source_text",
                    "exact_target_text",
                    "score_millionths",
                    "score_components_json",
                    "status",
                    "reason_codes_json",
                    "competing_alignment_refs_json",
                    "semantic_equivalence_asserted",
                    "human_acceptance",
                ],
            )?,
        )?;
        let segment_rows = data
            .segments
            .iter()
            .map(|r| {
                let mut out = fields(
                    &[r.clone()],
                    &[
                        "segment_id",
                        "context_unit_ref",
                        "sentence_unit_ref",
                        "start_offset",
                        "end_offset",
                        "exact_text",
                        "exact_sha256",
                        "speaker_role",
                        "speaker_status",
                    ],
                )?;
                let out = &mut out[0];
                out.push(json!(compact(&r["speaker_candidates"])?));
                out.push(json!(compact(&r["evidence_refs"])?));
                for k in [
                    "kind",
                    "quote_depth",
                    "speech_turn_id",
                    "utterer_role",
                    "attribution_basis",
                    "performed_role",
                    "modality",
                ] {
                    out.push(r[k].clone())
                }
                Ok(out.clone())
            })
            .collect::<Result<Vec<_>>>()?;
        insert(root, &tx, "discourse_segments", &segment_rows)?;
        insert(
            root,
            &tx,
            "quote_events",
            &data
                .events
                .iter()
                .map(|r| {
                    Ok(vec![
                        r["event_id"].clone(),
                        r["context_unit_ref"].clone(),
                        r["offset"].clone(),
                        json!(compact(r)?),
                    ])
                })
                .collect::<Result<Vec<_>>>()?,
        )?;
        insert(
            root,
            &tx,
            "occurrence_spans",
            &fields(
                &data.mappings,
                &[
                    "existing_occurrence_ref",
                    "context_unit_ref",
                    "surface_unit_ref",
                    "start_offset",
                    "end_offset",
                    "exact_text",
                    "exact_sha256",
                    "status",
                ],
            )?,
        )?;
        insert(
            root,
            &tx,
            "formulas",
            &data
                .families
                .iter()
                .map(|r| {
                    Ok(vec![
                        r["formula_id"].clone(),
                        json!(
                            arr(r, "normalized_tokens")?
                                .iter()
                                .map(|v| v.as_str().ok_or("formula token string required".into()))
                                .collect::<Result<Vec<_>>>()?
                                .join(" ")
                        ),
                        r["token_count"].clone(),
                        r["occurrence_count"].clone(),
                        r["reading_count"].clone(),
                        r["status"].clone(),
                    ])
                })
                .collect::<Result<Vec<_>>>()?,
        )?;
        let mut memberships = Vec::new();
        for m in &data.memberships {
            for s in arr(m, "source_spans")? {
                memberships.push(vec![
                    m["formula_id"].clone(),
                    m["formula_occurrence_id"].clone(),
                    s["context_unit_ref"].clone(),
                    s["start_offset"].clone(),
                    s["end_offset"].clone(),
                    s["exact_text"].clone(),
                    s["exact_sha256"].clone(),
                    m.get("status").cloned().unwrap_or(json!("proposed")),
                ]);
            }
        }
        insert(root, &tx, "formula_occurrences", &memberships)?;
        insert(
            root,
            &tx,
            "formula_relations",
            &data
                .relations
                .iter()
                .map(|r| Ok(vec![r["relation_id"].clone(), json!(compact(r)?)]))
                .collect::<Result<Vec<_>>>()?,
        )?;
        tx.commit().map_err(|e| e.to_string())?;
        root.check()?;
        db.execute_batch("VACUUM").map_err(|e| e.to_string())?;
        let integrity: String = db
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        if integrity != "ok" {
            return Err("output database failed integrity_check".into());
        }
        db.close().map_err(|(_, e)| e.to_string())?;
        root.check()?;
        Ok(())
    })();
    scope.complete(recipe, 256 * MIB)
}
fn counts<'a>(rows: impl Iterator<Item = &'a Value>, field: &str) -> Result<Value> {
    let mut result = BTreeMap::<String, u64>::new();
    for row in rows {
        *result.entry(s(row, field)?.into()).or_default() += 1;
    }
    Ok(json!(result))
}
fn materialize(
    root: &ResearchExecution,
    output: &ResearchExecution,
) -> Result<(BTreeMap<String, Vec<u8>>, Value, Vec<u8>)> {
    let specs = [
        (
            "source_spine",
            private_ref("linguistic-source-spine-v1/linguistic-spine.v1.sqlite3"),
            format!(
                "{WORK}/technical-markup/zarathustra-linguistic-source-spine-v1/manifest.v1.json"
            ),
        ),
        (
            "analysis_predecessor",
            private_ref("linguistic-analysis-spine-v1/linguistic-analysis.v1.sqlite3"),
            format!(
                "{WORK}/technical-markup/zarathustra-linguistic-analysis-spine-v1/manifest.v1.json"
            ),
        ),
        (
            "concept_workbench",
            private_ref("concept-workbench-v1/workbench-index.v1.sqlite3"),
            "ToS/candidate-intake/zarathustra/concept-workbench-v1/manifest.v1.json".into(),
        ),
    ];
    let mut inputs = Vec::new();
    let mut evidence = Vec::new();
    for (role, reference, manifest) in specs {
        let (input, e) = load_input(root, &reference, &manifest, role)?;
        inputs.push(input);
        evidence.push(e)
    }
    // These are complete table exports. A search index matching only a prefix
    // of ORDER BY causes random table-page rereads before the final sort. Keep
    // the exact order, but scan each table once into the owned memory sorter.
    let source = root.open_sqlite_readonly_for_ordered_scan(&inputs[0].file)?;
    quick_check(root, &source)?;
    let contexts = query(
        root,
        &source,
        "SELECT * FROM contexts NOT INDEXED ORDER BY language,witness_order",
    )?;
    let sentences = query(
        root,
        &source,
        "SELECT * FROM sentences NOT INDEXED ORDER BY language,witness_ordinal",
    )?;
    let surfaces = query(
        root,
        &source,
        "SELECT * FROM surface_units NOT INDEXED ORDER BY language,witness_ordinal",
    )?;
    drop(source);
    let analysis = root.open_sqlite_readonly_for_ordered_scan(&inputs[1].file)?;
    quick_check(root, &analysis)?;
    let clauses = query(
        root,
        &analysis,
        "SELECT clause_unit_id AS clause_id,sentence_unit_ref AS sentence_id,context_unit_ref,start_offset,end_offset,exact_text,exact_sha256,boundary_status FROM clauses NOT INDEXED ORDER BY language,part,context_unit_ref,sentence_clause_ordinal",
    )?;
    let alignments = query(
        root,
        &analysis,
        "SELECT * FROM translation_alignments NOT INDEXED ORDER BY alignment_id",
    )?;
    drop(analysis);
    let concept = root.open_sqlite_readonly_for_ordered_scan(&inputs[2].file)?;
    quick_check(root, &concept)?;
    let occurrences = query(
        root,
        &concept,
        "SELECT * FROM exact_occurrences NOT INDEXED ORDER BY language,part,token_ordinal",
    )?;
    drop(concept);
    let policy_ref = format!("{ROUTE}/chapter-voice-policies.v1.json");
    let policy_raw = root.read(&policy_ref)?;
    let policy_sha = digest(&policy_raw);
    let policies: Value = serde_json::from_slice(&policy_raw).map_err(|e| e.to_string())?;
    let chapter_refs: BTreeSet<_> = contexts
        .iter()
        .filter_map(|c| c["reading_ref"].as_str().filter(|s| s.contains(".r")))
        .collect();
    let policy_refs = arr(&policies, "chapters")?
        .iter()
        .map(|c| s(c, "reading_ref"))
        .collect::<Result<BTreeSet<_>>>()?;
    if chapter_refs != policy_refs || chapter_refs.len() != 81 {
        return Err("voice policy must cover exactly all 81 chapter refs".into());
    }
    let policy_count = validate_policies(root, &contexts, &policies)?;
    let (segments, events, mut gaps) =
        crate::research_reading_discourse::build_discourse(root, &contexts, &sentences, &policies)?;
    let conservation =
        crate::research_reading_discourse::validate_partition(root, &contexts, &segments)?;
    let (mappings, mapping_gaps) = crosswalk(root, &contexts, &surfaces, &occurrences)?;
    let mapping_gap_count = mapping_gaps.len();
    gaps.extend(mapping_gaps);
    let (families, memberships, relations, formula_receipt) =
        crate::research_reading_formulas::build_formulas(root, &contexts, &surfaces)?;
    let context_map: BTreeMap<_, _> = contexts
        .iter()
        .map(|c| Ok((s(c, "context_unit_ref")?.to_owned(), c)))
        .collect::<Result<_>>()?;
    let mut anchors: BTreeMap<String, &Value> = BTreeMap::new();
    for r in &sentences {
        anchors.insert(s(r, "sentence_id")?.into(), r);
    }
    for r in &clauses {
        anchors.insert(s(r, "clause_id")?.into(), r);
    }
    let mut anchor_count = 0u64;
    for a in &alignments {
        for field in [
            "ordered_source_unit_refs_json",
            "ordered_target_unit_refs_json",
        ] {
            let refs: Vec<String> =
                serde_json::from_str(s(a, field)?).map_err(|e| e.to_string())?;
            for r in refs {
                root.tick(1)?;
                let unit = anchors
                    .get(&r)
                    .ok_or("predecessor alignment anchor absent")?;
                let context = context_map
                    .get(s(unit, "context_unit_ref")?)
                    .ok_or("alignment context absent")?;
                let exact = slice(
                    s(context, "exact_text")?,
                    n(unit, "start_offset")?,
                    n(unit, "end_offset")?,
                )?;
                if exact != s(unit, "exact_text")? || sha(&exact) != s(unit, "exact_sha256")? {
                    return Err("predecessor alignment source anchor mismatch".into());
                }
                anchor_count += 1;
            }
        }
    }
    let mut metadata: BTreeMap<String, String> = evidence
        .iter()
        .map(|e| Ok((format!("{}_sha256", s(e, "role")?), s(e, "sha256")?.into())))
        .collect::<Result<_>>()?;
    for (k, v) in [
        ("method_version", METHOD),
        ("builder_sha256", IMPLEMENTATIONS[0].1),
        (
            "source_root_posture",
            "explicit_private_input_root_not_public_fallback",
        ),
        ("accepted", "false"),
        ("human_review", "false"),
        ("semantic_equivalence_asserted", "false"),
    ] {
        metadata.insert(k.into(), v.into());
    }
    let data = Data {
        contexts,
        sentences,
        clauses,
        alignments,
        segments,
        events,
        mappings,
        families,
        memberships,
        relations,
    };
    let database = database(output, &data, &metadata)?;
    let mut readings = BTreeSet::new();
    for c in &data.contexts {
        readings.insert((
            s(c, "language")?.to_owned(),
            s(c, "reading_ref")?.to_owned(),
        ));
    }
    let mut census = Vec::new();
    for (lang, reading) in readings {
        root.check()?;
        let cs: Vec<_> = data
            .contexts
            .iter()
            .filter(|c| c["language"] == lang && c["reading_ref"] == reading)
            .collect();
        let ss: Vec<_> = data
            .segments
            .iter()
            .filter(|s| s["language"] == lang && s["reading_ref"] == reading)
            .collect();
        census.push(json!({"language":lang,"reading_ref":reading,"context_count":cs.len(),"context_refs_sha256":sha(&compact(&json!(cs.iter().map(|c|c["context_unit_ref"].clone()).collect::<Vec<_>>()))?),"segment_count":ss.len(),"speaker_roles":counts(ss.iter().copied(),"speaker_role")?,"speaker_status_counts":counts(ss.iter().copied(),"speaker_status")?}));
    }
    let mut chapter_counts = BTreeMap::new();
    let mut quote_counts = BTreeMap::new();
    let mut max_depth = BTreeMap::new();
    for lang in ["de", "ru"] {
        chapter_counts.insert(
            lang,
            data.contexts
                .iter()
                .filter(|c| c["language"] == lang)
                .filter_map(|c| c["reading_ref"].as_str().filter(|s| s.contains(".r")))
                .collect::<BTreeSet<_>>()
                .len(),
        );
        quote_counts.insert(
            lang,
            counts(
                data.events.iter().filter(|e| e["language"] == lang),
                "action",
            )?,
        );
        max_depth.insert(
            lang,
            data.events
                .iter()
                .filter(|e| e["language"] == lang)
                .map(|e| n(e, "depth_after"))
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .max()
                .ok_or("reading language has no quote events")?,
        );
    }
    let receipt = json!({"schema_version":"tos_zarathustra_reading_coverage_v1","method_version":METHOD,"chapters_per_language":chapter_counts,"conservation":conservation,"sentence_count":data.sentences.len(),"clause_count":data.clauses.len(),"source_bound_voice_policies_checked":policy_count,"alignment_anchors_checked_with_real_locator":anchor_count,"quotation_actions":quote_counts,"max_quote_depth":max_depth,"legacy_german_occurrence_crosswalk":{"mapped":data.mappings.len(),"gaps":mapping_gap_count,"method":"ordered_exact_stream_candidate_not_relabelled_XML_offsets","russian_legacy_bridge":"deferred_page_block_coordinates"},"formulas":formula_receipt,"gap_counts":counts(gaps.iter(),"kind")?,"accepted":false,"human_review":false,"canon_effect":false,"limitations":["voice policies and explicit reporting cues are candidates, not a coreference model","quoted frame utterer does not resolve the embedded voice","legacy morphology and first-verb dependency heuristics are not improved or certified here","DE/RU sentence and clause alignments remain predecessor proposals; verse exclusions retained","etymology and English require a separate source-bound on-demand agent analysis"]});
    let formula_census: Vec<_> = data
        .families
        .iter()
        .map(|f| {
            let mut f = f.clone();
            if let Some(o) = f.as_object_mut() {
                for k in [
                    "normalized_tokens",
                    "normalized_text",
                    "exact_text",
                    "display_text",
                ] {
                    o.remove(k);
                }
            }
            f
        })
        .collect();
    let mut encoded = BTreeMap::from([
        ("coverage-receipt.v1.json".into(), bytes(&receipt, true)?),
        ("reading-census.v1.jsonl".into(), lines(&census)?),
        (
            "quote-boundary-ledger.v1.jsonl".into(),
            lines(&data.events)?,
        ),
        ("gap-ledger.v1.jsonl".into(), lines(&gaps)?),
        ("formula-census.v1.jsonl".into(), lines(&formula_census)?),
    ]);
    evidence
        .push(json!({"ref":policy_ref,"sha256":policy_sha,"role":"source_visible_voice_policy"}));
    let implementation: Vec<_> = IMPLEMENTATIONS
        .iter()
        .map(|(r, h)| json!({"ref":r,"sha256":h}))
        .collect();
    let artifacts: Vec<_> = [
        "coverage-receipt.v1.json",
        "reading-census.v1.jsonl",
        "quote-boundary-ledger.v1.jsonl",
        "gap-ledger.v1.jsonl",
        "formula-census.v1.jsonl",
    ]
    .iter()
    .map(|name| json!({"ref":format!("{ROUTE}/{name}"),"sha256":digest(&encoded[*name])}))
    .collect();
    let manifest = json!({"schema_version":"tos_zarathustra_reading_manifest_v1","method_version":METHOD,"private_database":{"ref":database_ref(),"sha256":digest(&database),"mode":"0600"},"inputs":evidence,"implementation":implementation,"artifacts":artifacts,"predecessor_retained":true,"tracked_source_strings":false,"accepted":false,"publication_posture":"excluded_from_public_bundle","human_review":false,"canon_effect":false});
    encoded.insert("manifest.v1.json".into(), bytes(&manifest, true)?);
    for input in &mut inputs {
        root.verify_file_unchanged(&input.file, &input.metadata)?;
        root.verify_file_unchanged(&input.manifest_file, &input.manifest_metadata)?;
        if root.hash_file(&mut input.manifest_file, 8 * 1024 * 1024)? != input.manifest_hash {
            return Err("input manifest changed".into());
        }
        if root.hash_file(&mut input.file, 512 * 1024 * 1024)? != input.hash {
            return Err(format!("input changed: {}", input.reference));
        }
    }
    if digest(&root.read(&policy_ref)?) != policy_sha {
        return Err("voice policies changed during materialization".into());
    }
    root.check()?;
    Ok((encoded, receipt, database))
}
pub fn run_scoped(root: &ResearchExecution, args: &[String]) -> Result<Value> {
    let mut output = None;
    let mut selected_software = None;
    let mut mode = None;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--build" | "--check" | "--validate-tracked" => {
                if mode.replace(arg.as_str()).is_some() {
                    return Err("select exactly one Reading mode".into());
                }
            }
            "--output-root" => {
                if output
                    .replace(it.next().ok_or("--output-root requires a value")?.clone())
                    .is_some()
                {
                    return Err("duplicate --output-root".into());
                }
            }
            "--software-root" => {
                if selected_software
                    .replace(it.next().ok_or("--software-root requires a value")?.clone())
                    .is_some()
                {
                    return Err("duplicate --software-root".into());
                }
            }
            _ => return Err(format!("unsupported Reading option: {arg}")),
        }
    }
    let mode = mode.ok_or("Reading mode required")?;
    let output = output.ok_or("--output-root required")?;
    let output = Path::new(&output);
    if !output.is_absolute() {
        return Err("Reading output root must be absolute".into());
    }
    let source_path = root.root().canonicalize().map_err(|e| e.to_string())?;
    if output.components().any(|c| {
        !matches!(
            c,
            std::path::Component::RootDir | std::path::Component::Normal(_)
        )
    }) {
        return Err("Reading output root must have normal absolute components".into());
    }
    let output_path = if output.exists() {
        output.canonicalize().map_err(|e| e.to_string())?
    } else {
        output.to_path_buf()
    };
    let software = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .ok_or("software root absent")?;
    if output_path != output
        || source_path != root.root()
        || output_path.starts_with(&source_path)
        || source_path.starts_with(&output_path)
        || output_path.starts_with(software)
        || software.starts_with(&output_path)
    {
        return Err(
            "output root must be separate from source data and software checkout without symlinks"
                .into(),
        );
    }
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let installed_software = if executable.ends_with("access/src/tos_access/tos-access") {
        executable
            .ancestors()
            .nth(4)
            .ok_or("installed software root absent")?
    } else {
        executable.parent().ok_or("executable directory absent")?
    };
    let mut software_roots = vec![installed_software.to_path_buf()];
    if let Some(selected) = selected_software {
        let selected = Path::new(&selected);
        if !selected.is_absolute() {
            return Err("selected software root must be absolute".into());
        }
        let canonical = selected.canonicalize().map_err(|e| e.to_string())?;
        if canonical != selected {
            return Err("selected software root may not contain symlinks".into());
        }
        software_roots.push(canonical);
    }
    for software in software_roots {
        if output_path.starts_with(&software) || software.starts_with(&output_path) {
            return Err("output root must be separate from source data and software checkout without symlinks".into());
        }
    }
    let out = root.select_output_directory(output, mode == "--build")?;
    let manifest_ref = format!("{ROUTE}/manifest.v1.json");
    if mode == "--validate-tracked" {
        let manifest = load(&out, &manifest_ref)?;
        for entry in arr(&manifest, "artifacts")? {
            root.tick(1)?;
            let reference = s(entry, "ref")?;
            if digest(&out.read(reference)?) != s(entry, "sha256")? {
                return Err(format!("tracked currentness drift: {reference}"));
            }
        }
        let policy = arr(&manifest, "inputs")?
            .iter()
            .find(|e| e["role"] == "source_visible_voice_policy")
            .ok_or("policy input missing")?;
        if digest(&root.read(s(policy, "ref")?)?) != s(policy, "sha256")? {
            return Err("voice policy changed without rebuild".into());
        }
        root.check()?;
        return Ok(json!({"tracked_currentness":"verified","method_version":METHOD}));
    }
    if mode == "--build"
        && (output_path.join(database_ref()).exists() || output_path.join(ROUTE).exists())
    {
        return Err(
            "build requires a new output dataset; existing reading output is immutable".into(),
        );
    }
    if mode == "--check" && !output_path.join(database_ref()).is_file() {
        return Err("reading dataset to check does not exist".into());
    }
    let (encoded, receipt, database) = materialize(root, &out)?;
    if mode == "--check" {
        if digest(&out.read(&database_ref())?) != digest(&database) {
            return Err("private database deterministic parity drift".into());
        }
        for (name, payload) in &encoded {
            root.tick(payload.len() as u64)?;
            if out.read(&format!("{ROUTE}/{name}"))? != *payload {
                return Err(format!("tracked deterministic parity drift: {name}"));
            }
        }
    } else {
        out.write(&database_ref(), &database, 0o600, true)?;
        // Preserve the v1 publication order; the manifest commits last.
        for name in [
            "coverage-receipt.v1.json",
            "reading-census.v1.jsonl",
            "quote-boundary-ledger.v1.jsonl",
            "gap-ledger.v1.jsonl",
            "formula-census.v1.jsonl",
            "manifest.v1.json",
        ] {
            out.write(&format!("{ROUTE}/{name}"), &encoded[name], 0o600, true)?;
        }
    }
    root.check()?;
    Ok(receipt)
}
