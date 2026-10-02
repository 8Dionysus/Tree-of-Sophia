use super::*;
// SQLite field JSON follows the legacy standard separators, including spaces.
fn spaced(v: &Value) -> String {
    fn walk(v: &Value) -> String {
        match v {
            Value::Array(a) => format!("[{}]", a.iter().map(walk).collect::<Vec<_>>().join(", ")),
            Value::Object(o) => format!(
                "{{{}}}",
                o.iter()
                    .map(|(k, v)| format!("{}: {}", serde_json::to_string(k).unwrap(), walk(v)))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            _ => serde_json::to_string(v).unwrap(),
        }
    }
    walk(v)
}
// Bounded scalar diagnostics preserve the first SQL failure before scope cleanup.
// These control observations contain no SQL text, parameters, or source payload.
fn phase_budget(root: &ResearchExecution, stage: &'static str) -> Result<Value> {
    root.check()?;
    let budget = root.budget_report();
    eprintln!("concept private database phase {stage}: {budget}");
    root.check()?;
    Ok(budget)
}
fn sql_failure(
    root: &ResearchExecution,
    stage: &'static str,
    before: &Value,
    error: rusqlite::Error,
) -> String {
    let extended_code = match &error {
        rusqlite::Error::SqliteFailure(code, _) => Some(code.extended_code),
        _ => None,
    };
    format!(
        "concept private database {stage}: {error}; sqlite_extended_code={extended_code:?}; phase_budget_before={before}; phase_budget_failure={}",
        root.budget_report()
    )
}
pub(super) fn build(
    root: &ResearchExecution,
    _c: &Config,
    units: &[Value],
    speakers: &[Value],
    occ: &[Value],
) -> Result<Vec<u8>> {
    const MIB: u64 = 1024 * 1024;
    // Planning ceilings preserve the frozen recipe; they confer no storage grant.
    let mut scope = root.sqlite_scope(tos_source_store::PinnedSqliteAuxLimits {
        main_logical_bytes: 200 * MIB,
        main_allocated_bytes: 200 * MIB,
        temp_db_logical_bytes: 200 * MIB,
        temp_db_allocated_bytes: 200 * MIB,
        main_journal_logical_bytes: 0,
        main_journal_allocated_bytes: 0,
        temp_journal_logical_bytes: 4 * MIB,
        temp_journal_allocated_bytes: 4 * MIB,
        other_aux_aggregate_logical_bytes: 8 * MIB,
        other_aux_aggregate_allocated_bytes: 8 * MIB,
        max_live_aux: 8,
    })?;
    let result = (|| -> Result<()> {
        let before = phase_budget(root, "open")?;
        let mut db = scope
            .scope_mut()
            .open_connection()
            .map_err(|e| format!("concept private database open: {e}; phase_budget_before={before}; phase_budget_failure={}", root.budget_report()))?;
        let deadline = root.deadline();
        db.progress_handler(1000, Some(move || std::time::Instant::now() >= deadline));
        let before = phase_budget(root, "cache policy")?;
        // A 200 MiB suggested main-cache target may reduce indexed-insert rereads.
        // VACUUM can copy this target to its temporary pager: two suggested
        // targets total 400 MiB, plus overhead; the outer RAM limit remains binding.
        db.execute_batch("PRAGMA main.cache_size=-204800; PRAGMA max_page_count=65536;")
            .map_err(|e| sql_failure(root, "cache policy", &before, e))?;
        let cache_kib: i64 = db
            .query_row("PRAGMA main.cache_size", [], |row| row.get(0))
            .map_err(|e| sql_failure(root, "cache readback", &before, e))?;
        root.check()?;
        if cache_kib != -204800 {
            return Err("concept private database cache policy readback mismatch".into());
        }
        let before = phase_budget(root, "schema")?;
        db.execute_batch(include_str!("research_concept_workbench.sql"))
            .map_err(|e| sql_failure(root, "schema", &before, e))?;
        let before = phase_budget(root, "transaction")?;
        let tx = db
            .transaction()
            .map_err(|e| sql_failure(root, "transaction", &before, e))?;
        let _before = phase_budget(root, "form inventory")?;
        let forms = form_inventory(root, occ)?;
        let before = phase_budget(root, "metadata insert")?;
        for (k, v) in [
            (
                "schema_version",
                "tos_zarathustra_concept_workbench_private_index_v1".into(),
            ),
            (
                "authority_boundary",
                "private_exact_source_return_and_candidate_analysis_not_semantic_authority".into(),
            ),
            ("context_unit_count", units.len().to_string()),
            ("exact_occurrence_count", occ.len().to_string()),
            ("analysis_form_count", forms.len().to_string()),
        ] {
            root.tick(1)?;
            tx.execute("INSERT INTO metadata VALUES(?1,?2)", params![k, v])
                .map_err(|e| sql_failure(root, "metadata insert", &before, e))?;
        }
        let by: BTreeMap<_, _> = speakers
            .iter()
            .map(|v| (s(v, "context_unit_ref"), v))
            .collect();
        {
            let before = phase_budget(root, "context units insert")?;
            let mut stmt = tx
                .prepare("INSERT INTO context_units VALUES(?,?,?,?,?,?,?,?,?,?,?,?)")
                .map_err(|e| sql_failure(root, "context units insert", &before, e))?;
            for r in units {
                root.tick(1)?;
                let speaker = by[s(r, "context_unit_ref")];
                stmt.execute(params![
                    s(r, "context_unit_ref"),
                    s(r, "language"),
                    n(r, "part"),
                    s(r, "reading_ref"),
                    s(r, "unit_kind"),
                    n(r, "witness_order"),
                    s(r, "text"),
                    s(r, "exact_sha256"),
                    spaced(&r["analysis_tokens"]),
                    spaced(&r["alignment_links"]),
                    s(speaker, "primary_role"),
                    s(speaker, "attribution_status")
                ])
                .map_err(|e| sql_failure(root, "context units insert", &before, e))?;
            }
        }
        {
            let before = phase_budget(root, "exact occurrences insert")?;
            let mut stmt = tx
                .prepare(
                    "INSERT INTO exact_occurrences VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
                )
                .map_err(|e| sql_failure(root, "exact occurrences insert", &before, e))?;
            for r in occ {
                root.tick(1)?;
                let scope = r["in_work_scope"] == true;
                stmt.execute(params![
                    s(r, "existing_occurrence_ref"),
                    s(r, "language"),
                    n(r, "part"),
                    r["context_unit_ref"].as_str(),
                    r["reading_ref"].as_str(),
                    s(r, "unit_kind"),
                    n(r, "witness_order"),
                    n(r, "token_ordinal"),
                    s(r, "surface"),
                    s(r, "exact_sha256"),
                    s(r, "normalized"),
                    s(r, "normalized_sha256"),
                    s(r, "analysis_key"),
                    s(r, "analysis_key_sha256"),
                    s(r, "source_locator_sha256"),
                    n(r, "start"),
                    n(r, "end"),
                    if scope {
                        "tos.work.friedrich-nietzsche.also-sprach-zarathustra"
                    } else {
                        "tos.work.friedrich-nietzsche.dionysos-dithyramben"
                    },
                    scope as i64,
                    if scope {
                        None
                    } else {
                        Some("appended_separate_work_part4_div21")
                    }
                ])
                .map_err(|e| sql_failure(root, "exact occurrences insert", &before, e))?;
            }
        }
        let mut forms = forms;
        forms.sort_by(|a, b| {
            (s(a, "language"), s(a, "analysis_key")).cmp(&(s(b, "language"), s(b, "analysis_key")))
        });
        {
            let before = phase_budget(root, "analysis forms insert")?;
            let mut stmt = tx
                .prepare("INSERT INTO analysis_forms VALUES(?,?,?,?,?,?)")
                .map_err(|e| sql_failure(root, "analysis forms insert", &before, e))?;
            for r in forms {
                root.tick(1)?;
                stmt.execute(params![
                    s(&r, "language"),
                    s(&r, "analysis_key"),
                    s(&r, "analysis_key_sha256"),
                    n(&r, "occurrence_count"),
                    arr(&r["exact_hashes"]).len(),
                    "unresolved_candidate_available_for_request_expansion"
                ])
                .map_err(|e| sql_failure(root, "analysis forms insert", &before, e))?;
            }
        }
        let before = phase_budget(root, "commit")?;
        tx.commit()
            .map_err(|e| sql_failure(root, "commit", &before, e))?;
        let before = phase_budget(root, "vacuum")?;
        db.execute_batch("VACUUM;")
            .map_err(|e| sql_failure(root, "vacuum", &before, e))?;
        let before = phase_budget(root, "close")?;
        db.close()
            .map_err(|(_retained, error)| sql_failure(root, "close", &before, error))?;
        let _after = phase_budget(root, "closed")?;
        Ok(())
    })();
    scope.complete(result, 200 * MIB)
}
