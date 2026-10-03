//! Explicit normalized-snapshot observation. No assessment or source writes.
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
use tos_foundation::{JsonLimits, JsonMode, emit_value_preserved_json, parse_json};
use tos_query::knowledge_presentation::HUMAN_FORM_ROLES as ROLES;
use tos_query::{
    AbortProbe,
    source_diagnostic::{DiagnosticError, LegacyStore, Limits},
};
const ROW_BYTES: usize = 4 * 1024 * 1024;
const GRAPH_BYTES: usize = 16 * 1024 * 1024;
const MAX_GROUPS: usize = 4096;
const LIMITATIONS: [&str; 7] = [
    "Source objects absent from the snapshot are not enumerated.",
    "Source record pointers do not establish byte preservation against live source.",
    "Source-marked wording reports projected provenance, not verified source authorship.",
    "Display presence and ready forms do not establish substantive quality or admission.",
    "Role absence does not establish applicability; source owner must review the gap.",
    "No rights, payload availability, language, or truth is inferred from missing data.",
    "The report describes this exact projection, not generated currentness or runtime health.",
];
fn validate_language(language: &str) -> Result<(), String> {
    if language.len() <= 128
        && (matches!(language, "auto" | "original")
            || tos_query::knowledge_lens_spec::language(language))
    {
        Ok(())
    } else {
        Err("invalid coverage language preference".into())
    }
}
fn truth(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
        Value::String(s) => !s.is_empty(),
        Value::Number(n) => n.to_string() != "0",
    }
}
fn required<'a>(v: &'a Value, name: &str) -> Result<&'a Value, String> {
    v.get(name)
        .ok_or_else(|| format!("coverage carrier missing {name}"))
}
/// Pure packet observation; all display/Form selection remains with QRY.
pub fn coverage_row(item: &Value, language: &str) -> Result<Value, String> {
    validate_language(language)?;
    if !item.is_object() {
        return Err("coverage carrier must be an object".into());
    }
    let raw = serde_json::to_vec(item).map_err(|e| e.to_string())?;
    let carrier = parse_json(
        &raw,
        JsonMode::RequestLastWins,
        JsonLimits {
            max_bytes: ROW_BYTES,
            max_depth: 64,
            max_visits: 200000,
            max_integer_digits: 4300,
        },
    )
    .map_err(|e| e.to_string())?
    .into_root();
    let selected =
        tos_query::knowledge_presentation::lens_carrier(&carrier, "full", Some(language))
            .map_err(|e| e.to_string())?;
    let selected: Value = serde_json::from_slice(
        &emit_value_preserved_json(
            &selected,
            JsonLimits {
                max_bytes: ROW_BYTES * 2,
                max_depth: 64,
                max_visits: 400000,
                max_integer_digits: 4300,
            },
        )
        .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let relation = item.get("from_id").is_some();
    let mapping = &item[if relation {
        "predicate_mapping"
    } else {
        "type_mapping"
    }];
    let attrs = &item["attributes"];
    let provenance = &item["display"]["provenance"];
    let mut display = serde_json::Map::new();
    for (field, choice) in selected["display_selection"]["fields"]
        .as_object()
        .ok_or("coverage display selection missing")?
    {
        let available = choice["content_available"].as_bool().unwrap_or(false);
        let derivation = &provenance[field];
        let state = if !available {
            "missing"
        } else if matches!(
            derivation.as_str(),
            Some(
                "identifier-fallback"
                    | "projected-path"
                    | "navigation-template"
                    | "record-version-navigation"
                    | "endpoint-label-synthesis"
            )
        ) {
            "derived-navigation"
        } else if matches!(
            derivation.as_str(),
            Some(
                "source-derived"
                    | "authored"
                    | "exact-record-quotation"
                    | "projected-label"
                    | "projected-predicate-label"
                    | "registry-label"
            )
        ) {
            "source-marked"
        } else {
            "unclassified"
        };
        display.insert(field.clone(), json!({"content_available": available, "wording_state": state, "derivation": derivation, "selected_key": choice["selected_key"], "actual_language": choice["actual_language"], "selection_reason": choice["reason"], "source_pointer": choice["source_form_pointer"]}));
    }
    let forms = selected.get("human_form_selection");
    let mut roles = serde_json::Map::new();
    for role in ROLES {
        let chosen = forms.map(|f| &f["roles"][role]);
        // QRY selects the exact form and decides whether its packet fits the
        // delivery budget. Recover observation fields from that retained exact
        // packet rather than duplicating the v2 compression codec.
        let packet = chosen
            .filter(|s| s["state"] == "ready")
            .and_then(|s| {
                attrs["human_forms"]
                    .as_array()?
                    .iter()
                    .find(|p| p["form"] == s["form"])
            })
            .unwrap_or(&Value::Null);
        let mut states: BTreeMap<String, u64> = BTreeMap::new();
        if let Some(candidates) = forms.and_then(|f| f["candidates"].as_array()) {
            for c in candidates.iter().filter(|c| c["role"] == role) {
                *states
                    .entry(c["state"].as_str().ok_or("invalid candidate state")?.into())
                    .or_default() += 1;
            }
        }
        roles.insert(role.into(), json!({"state": chosen.map(|s| &s["state"]).cloned().unwrap_or(json!("not-provided")), "reason": chosen.map(|s| &s["reason"]).cloned().unwrap_or(json!("no-materialized-form-collection")), "candidate_states": states, "form": chosen.map(|s| &s["form"]).cloned().unwrap_or(Value::Null), "derivation": packet["derivation"], "language": packet["language"], "assessment_snapshot_present": truth(&packet["assessment_snapshot"]), "subject_assessment_present": truth(&packet["subject_assessment"])}));
    }
    let pointers: Vec<_> = ["source_record", "source_claim", "record_version_view"]
        .iter()
        .filter(|k| attrs.get(**k).is_some())
        .map(|k| format!("/attributes/{k}"))
        .collect();
    let mut actions = vec![];
    if mapping["status"] != "mapped" {
        actions.push("review-source-mapping-with-semantic-registry-owner");
    }
    if display.values().any(|v| v["content_available"] == false) {
        actions.push("review-missing-wording-at-source");
    }
    if forms.is_none() {
        actions.push("source-owner-must-declare-form-adapter-or-explicit-gap");
    } else if forms.is_some_and(|f| f["state"] != "available") {
        actions.push("inspect-form-collection-with-source-owner");
    }
    if forms.is_some() && roles.values().any(|r| r["state"] != "ready") {
        actions.push("inspect-role-gaps-and-applicability-with-source-owner");
    }
    Ok(
        json!({"kind": if relation {"relation"} else {"node"}, "id": required(item,"id")?, "entity_id": item["entity_id"], "source_graph": item["source_graph"], "content_revision": required(item,"content_revision")?, "source_refs": item.get("source_refs").cloned().unwrap_or(json!([])), "mapping": {"status": mapping.get("status").cloned().unwrap_or(json!("unknown")), "semantic_id": item[if relation {"relation_type_id"} else {"type_id"}], "source_id": item[if relation {"predicate_id"} else {"kind_id"}], "basis": "declared-normalizer-mapping-not-semantic-acceptance"}, "retained_record_pointers": pointers, "display": display, "forms": {"collection_state": forms.map(|f| &f["state"]).cloned().unwrap_or(json!("not-provided")), "source_ref": forms.map(|f| &f["source_ref"]).cloned().unwrap_or(Value::Null), "issues": forms.map(|f| &f["issues"]).cloned().unwrap_or(json!([])), "roles": roles}, "next_actions": actions}),
    )
}
#[derive(Default)]
struct Group {
    carriers: u64,
    mapping: BTreeMap<String, u64>,
    display: BTreeMap<String, BTreeMap<String, u64>>,
    form_collections: BTreeMap<String, u64>,
    form_roles: BTreeMap<String, BTreeMap<String, u64>>,
    candidate_states: BTreeMap<String, BTreeMap<String, u64>>,
    retained: u64,
}
fn bump(
    map: &mut BTreeMap<String, u64>,
    key: &str,
    n: u64,
    retained: &mut usize,
) -> Result<(), String> {
    if !map.contains_key(key) {
        charge(retained, key.len() + 96)?;
    }
    *map.entry(key.into()).or_default() += n;
    Ok(())
}
fn charge(retained: &mut usize, bytes: usize) -> Result<(), String> {
    *retained = retained
        .checked_add(bytes)
        .filter(|n| *n <= 4 * 1024 * 1024)
        .ok_or("coverage aggregate byte budget exceeded")?;
    Ok(())
}
fn state(v: &Value) -> Result<&str, String> {
    v.as_str()
        .ok_or_else(|| "coverage state must be a string".into())
}
#[derive(Default)]
struct Report {
    groups: BTreeMap<(String, Option<String>), Group>,
    nodes: u64,
    relations: u64,
    aggregate_charge: usize,
}
fn category(v: &Value) -> Result<(), String> {
    match v {
        Value::String(s) if s.len() > 4096 => {
            Err("coverage aggregation key byte budget exceeded".into())
        }
        Value::Array(a) => {
            for v in a {
                category(v)?;
            }
            Ok(())
        }
        Value::Object(o) => {
            for (k, v) in o {
                if k.len() > 4096 {
                    return Err("coverage aggregation key byte budget exceeded".into());
                }
                category(v)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
impl Report {
    fn observe(&mut self, row: &Value) -> Result<(), String> {
        for v in [
            &row["source_graph"],
            &row["mapping"]["status"],
            &row["display"],
            &row["forms"]["collection_state"],
        ] {
            category(v)?;
        }
        let kind = state(&row["kind"])?;
        let source = if row["source_graph"].is_null() {
            None
        } else {
            Some(state(&row["source_graph"])?.into())
        };
        let key = (kind.into(), source);
        if !self.groups.contains_key(&key) && self.groups.len() >= MAX_GROUPS {
            return Err("coverage group budget exceeded".into());
        }
        if !self.groups.contains_key(&key) {
            charge(
                &mut self.aggregate_charge,
                key.0.len() + key.1.as_ref().map_or(0, String::len) + 1024,
            )?;
        }
        if kind == "node" {
            self.nodes += 1
        } else {
            self.relations += 1
        };
        let g = self.groups.entry(key).or_default();
        g.carriers += 1;
        bump(
            &mut g.mapping,
            state(&row["mapping"]["status"])?,
            1,
            &mut self.aggregate_charge,
        )?;
        g.retained += u64::from(truth(&row["retained_record_pointers"]));
        for (field, v) in row["display"]
            .as_object()
            .ok_or("invalid coverage display")?
        {
            let counts = g.display.entry(field.clone()).or_default();
            bump(
                counts,
                if truth(&v["content_available"]) {
                    "available"
                } else {
                    "missing"
                },
                1,
                &mut self.aggregate_charge,
            )?;
            let derivation = v["derivation"].as_str().unwrap_or("None");
            bump(
                counts,
                &format!("derivation:{derivation}"),
                1,
                &mut self.aggregate_charge,
            )?;
            bump(
                counts,
                &format!("wording:{}", state(&v["wording_state"])?),
                1,
                &mut self.aggregate_charge,
            )?;
        }
        bump(
            &mut g.form_collections,
            state(&row["forms"]["collection_state"])?,
            1,
            &mut self.aggregate_charge,
        )?;
        for role in ROLES {
            let v = &row["forms"]["roles"][role];
            bump(
                g.form_roles.entry(role.into()).or_default(),
                state(&v["state"])?,
                1,
                &mut self.aggregate_charge,
            )?;
            let c = g.candidate_states.entry(role.into()).or_default();
            for (s, n) in v["candidate_states"]
                .as_object()
                .ok_or("invalid coverage candidate states")?
            {
                bump(
                    c,
                    s,
                    n.as_u64().ok_or("invalid candidate count")?,
                    &mut self.aggregate_charge,
                )?;
            }
        }
        Ok(())
    }
    fn finish(self, revision: &Value, language: &str) -> Value {
        let groups: Vec<_> = self.groups.into_iter().map(|((kind,source),g)| json!({"kind":kind,"source_graph":source,"carriers":g.carriers,"mapping":g.mapping,"display":g.display,"form_collections":g.form_collections,"form_roles":g.form_roles,"candidate_states":g.candidate_states,"retained_record_carriers":g.retained})).collect();
        json!({"schema_version":"tos_knowledge_coverage_v1","source_revision":revision,"language":language,"scope":"normalized-snapshot-carriers-only","enumeration_complete":true,"counting_unit":"carrier-not-distinct-subject-or-assessed-content","nodes":self.nodes,"relations":self.relations,"groups":groups,"limitations":LIMITATIONS,"performs_assessment":false,"writes_to_source":false})
    }
}
fn emit(out: &mut dyn Write, value: &Value) -> Result<(), String> {
    serde_json::to_writer(&mut *out, value).map_err(|e| e.to_string())?;
    out.write_all(b"\n")
        .and_then(|_| out.flush())
        .map_err(|e| e.to_string())
}
struct Deadline(Instant);
impl AbortProbe for Deadline {
    fn reason(&self) -> Option<tos_query::AbortReason> {
        (Instant::now() >= self.0).then_some(tos_query::AbortReason::DeadlineExceeded)
    }
}
fn read_json(path: &str, cap: usize, deadline: Instant) -> Result<Value, String> {
    let mut bytes = Vec::new();
    if path == "-" {
        let mut stdin = std::io::stdin();
        let mut chunk = [0u8; 65536];
        loop {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or("coverage input deadline exceeded")?;
            let mut fd = libc::pollfd {
                fd: 0,
                events: libc::POLLIN,
                revents: 0,
            };
            let milliseconds = remaining.as_millis().min(i32::MAX as u128).max(1) as i32;
            // poll only the inherited stdin FD; no filesystem/software discovery.
            let ready = unsafe { libc::poll(&mut fd, 1, milliseconds) };
            if ready == 0 {
                return Err("coverage input deadline exceeded".into());
            }
            if ready < 0 {
                let e = std::io::Error::last_os_error();
                if e.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(e.to_string());
            }
            let n = stdin.read(&mut chunk).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            if n > cap.saturating_sub(bytes.len()) {
                return Err("coverage input byte budget exceeded".into());
            }
            bytes.extend_from_slice(&chunk[..n]);
        }
    } else {
        let file = tos_fd_open::open_absolute_regular(Path::new(path), cap as u64)
            .map_err(|e| e.to_string())?;
        (&file)
            .take(cap as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
    }
    let value = parse_json(
        &bytes,
        JsonMode::RequestLastWins,
        JsonLimits {
            max_bytes: cap,
            max_depth: 64,
            max_visits: 1000000,
            max_integer_digits: 4300,
        },
    )
    .map_err(|e| e.to_string())?
    .into_root();
    serde_json::from_slice(
        &emit_value_preserved_json(
            &value,
            JsonLimits {
                max_bytes: cap,
                max_depth: 64,
                max_visits: 1000000,
                max_integer_digits: 4300,
            },
        )
        .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
fn selected(root: &Path, env: &str, relative: &str) -> PathBuf {
    let p = std::env::var_os(env)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(relative));
    if p.is_absolute() { p } else { root.join(p) }
}
/// Select the authenticated maintained normalized snapshot without reconstructing
/// a graph. Source comparison diagnostics share this exact input binding seam.
pub fn open_knowledge_store(
    root: &Path,
    limits: Limits,
    deadline: Instant,
) -> Result<LegacyStore, String> {
    if !root.is_absolute() {
        return Err("coverage --root must be absolute".into());
    }
    let specs = [
        (
            "TOS_CORPUS_INDEX_PATH",
            "ToS/derived-exports/tos_corpus_index.min.json",
        ),
        (
            "TOS_PHILOSOPHY_GRAPH_PROJECTION_PATH",
            "ToS/derived-exports/philosophy_graph_projection.min.json",
        ),
        (
            "TOS_BIBLIOGRAPHIC_GRAPH_PATH",
            "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json",
        ),
        (
            "TOS_ENTITY_TYPE_REGISTRY_PATH",
            "ToS/doctrine/semantic-interchange/entity-types.v1.json",
        ),
        (
            "TOS_RELATION_TYPE_REGISTRY_PATH",
            "ToS/doctrine/semantic-interchange/relation-types.v1.json",
        ),
    ];
    let inputs: Vec<_> = specs
        .into_iter()
        .map(|(env, rel)| (rel.into(), selected(root, env, rel)))
        .collect();
    let path = selected(
        root,
        "TOS_QUERY_STORE_PATH",
        "ToS/derived-exports/runtime/knowledge.sqlite3",
    );
    LegacyStore::open(
        &path,
        &inputs,
        limits,
        deadline,
        Arc::new(Deadline(deadline)),
    )
    .map_err(|e| e.to_string())
}

fn report_graph(
    graph: &Value,
    language: &str,
    row_limit: u64,
    deadline: Instant,
    rows: bool,
    out: &mut dyn Write,
) -> Result<(), String> {
    let mut report = Report::default();
    let revision = required(graph, "source_revision")?;
    for collection in ["nodes", "relations"] {
        for item in required(graph, collection)?
            .as_array()
            .ok_or("coverage collection must be an array")?
        {
            if Instant::now() >= deadline || report.nodes + report.relations >= row_limit {
                return Err("coverage deadline/row budget exceeded".into());
            }
            let row = coverage_row(item, language)?;
            report.observe(&row)?;
            if rows {
                emit(
                    out,
                    &json!({"schema_version":"tos_knowledge_coverage_row_v1","source_revision":revision,"observation":row}),
                )?;
            }
        }
    }
    if Instant::now() >= deadline {
        return Err("coverage deadline exceeded before completion".into());
    }
    emit(out, &report.finish(revision, language))
}

fn execute(args: &[String], out: &mut dyn Write) -> Result<(), String> {
    let mut language = "auto";
    let mut input = None;
    let mut root = None;
    let mut rows = false;
    let mut bytes = 256 * 1024 * 1024u64;
    let mut row_limit = 1000000u64;
    let mut seconds = 30u64;
    let mut i = 1;
    while i < args.len() {
        let flag = args[i].as_str();
        i += 1;
        if flag == "--rows" {
            rows = true;
            continue;
        }
        let value = args.get(i).ok_or("coverage option needs a value")?;
        i += 1;
        match flag {
            "--language" => language = value,
            "--graph" | "--input" => {
                if input.replace(value.as_str()).is_some() {
                    return Err("duplicate coverage input".into());
                }
            }
            "--root" => {
                if root.replace(value.as_str()).is_some() {
                    return Err("duplicate coverage root".into());
                }
            }
            "--max-input-bytes" => bytes = value.parse().map_err(|_| "invalid byte budget")?,
            "--max-rows" => row_limit = value.parse().map_err(|_| "invalid row budget")?,
            "--max-seconds" => seconds = value.parse().map_err(|_| "invalid deadline")?,
            _ => return Err(format!("unknown coverage option {flag}")),
        }
    }
    validate_language(language)?;
    if bytes == 0
        || bytes > 4 * 1024 * 1024 * 1024
        || row_limit == 0
        || row_limit > 10000000
        || seconds == 0
        || seconds > 3600
    {
        return Err("coverage budget outside supported envelope".into());
    }
    if input.is_some() == root.is_some() {
        return Err("coverage requires exactly one --root or --graph/--input".into());
    }
    let deadline = Instant::now() + Duration::from_secs(seconds);
    if args[0] == "coverage-row" {
        if rows || root.is_some() {
            return Err("coverage-row requires --input".into());
        }
        let row = coverage_row(
            &read_json(input.unwrap(), ROW_BYTES.min(bytes as usize), deadline)?,
            language,
        )?;
        if Instant::now() >= deadline {
            return Err("coverage-row deadline exceeded".into());
        }
        return emit(out, &row);
    }
    if let Some(path) = input {
        let graph = read_json(path, GRAPH_BYTES.min(bytes as usize), deadline)?;
        report_graph(&graph, language, row_limit, deadline, rows, out)
    } else {
        let mut report = Report::default();
        let root = Path::new(root.unwrap());
        if !root.is_absolute() {
            return Err("coverage --root must be absolute".into());
        }
        let mut store = open_knowledge_store(
            root,
            Limits {
                max_input_bytes: bytes,
                max_json_bytes: ROW_BYTES,
                max_rows: row_limit,
                max_work_steps: 100000000,
                max_sql_vm_steps: 100000000,
                sqlite_cache_kib: 8192,
            },
            deadline,
        )?;
        let revision = required(&store.graph_header, "source_revision")?.clone();
        let counts=store.visit_knowledge_carriers(|item| {let row=coverage_row(item,language).map_err(DiagnosticError)?;report.observe(&row).map_err(DiagnosticError)?;if rows{emit(out,&json!({"schema_version":"tos_knowledge_coverage_row_v1","source_revision":revision,"observation":row})).map_err(DiagnosticError)?;} Ok(())}).map_err(|e|e.to_string())?;
        if counts != (report.nodes, report.relations) {
            return Err("coverage enumeration count mismatch".into());
        }
        store.verify_currentness().map_err(|e| e.to_string())?;
        emit(out, &report.finish(&revision, language))
    }
}
pub fn run_if_requested(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> Option<i32> {
    if !args
        .first()
        .is_some_and(|s| matches!(s.as_str(), "coverage" | "coverage-row"))
    {
        return None;
    }
    if args.len() == 2 && matches!(args[1].as_str(), "--help" | "-h") {
        let _ = writeln!(
            out,
            "coverage --root ABS | --graph ABS_JSON|- [--language auto|original|TAG] [--rows] [--max-input-bytes N --max-rows N --max-seconds N]\ncoverage-row --input ABS_JSON|- [--language TAG]\nThe root route requires a completed snapshot-bound offline query store. Rows without a terminal summary are incomplete. No source assessment or mutation."
        );
        return Some(0);
    }
    Some(match execute(args, out) {
        Ok(()) => 0,
        Err(error) => {
            let _ = writeln!(err, "coverage_refused: {error}");
            2
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn carrier() -> Value {
        json!({"id":"same-carrier", "entity_id":"same-subject", "source_graph":"synthetic", "content_revision":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "source_refs":["test:owner"], "kind_id":"unknown", "type_mapping":{"status":"unmapped"}, "attributes":{"source_record":{}}, "display":{"title":{"default":"navigation ID"},"kind_label":{"en":"Kind"},"summary":{"en":"A placeholder notice"},"provenance":{"title":"identifier-fallback","source_title_available":false,"source_summary_available":false,"summary":"source-derived"}}})
    }
    #[test]
    fn carrier_observation_keeps_missing_provenance_and_duplicate_subjects() {
        let input = carrier();
        let before = input.clone();
        let row = coverage_row(&input, "en").unwrap();
        assert_eq!(input, before);
        assert_eq!(row["display"]["title"]["wording_state"], "missing");
        assert_eq!(row["display"]["summary"]["content_available"], false);
        assert_eq!(row["forms"]["collection_state"], "not-provided");
        assert_eq!(
            row["retained_record_pointers"],
            json!(["/attributes/source_record"])
        );
        assert!(
            row["next_actions"]
                .as_array()
                .unwrap()
                .contains(&json!("review-source-mapping-with-semantic-registry-owner"))
        );
        let mut report = Report::default();
        report.observe(&row).unwrap();
        report.observe(&row).unwrap();
        let result = report.finish(&json!("cut"), "en");
        assert_eq!(result["nodes"], 2);
        assert_eq!(result["groups"][0]["mapping"]["unmapped"], 2);
        assert_eq!(result["enumeration_complete"], true);
        assert_eq!(result["performs_assessment"], false);
        assert!(
            !serde_json::to_string(&result)
                .unwrap()
                .contains("navigation ID")
        );
    }
    #[test]
    fn partial_enumeration_and_sink_failure_never_emit_completion() {
        let graph =
            json!({"source_revision":"exact-cut","nodes":[carrier(),carrier()],"relations":[]});
        let mut output = Vec::new();
        assert!(
            report_graph(
                &graph,
                "auto",
                1,
                Instant::now() + Duration::from_secs(5),
                true,
                &mut output
            )
            .is_err()
        );
        let frames: Vec<Value> = output
            .split(|b| *b == b'\n')
            .filter(|b| !b.is_empty())
            .map(|b| serde_json::from_slice(b).unwrap())
            .collect();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0]["schema_version"], "tos_knowledge_coverage_row_v1");
        assert!(frames[0].get("enumeration_complete").is_none());
        struct Broken;
        impl Write for Broken {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("closed sink"))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        assert!(
            report_graph(
                &graph,
                "auto",
                10,
                Instant::now() + Duration::from_secs(5),
                true,
                &mut Broken
            )
            .is_err()
        );
    }
    #[test]
    fn exact_language_grammar_and_bad_carrier_refuse() {
        for tag in ["auto", "original", "de-CH-1996", "x-private", "i-klingon"] {
            validate_language(tag).unwrap();
        }
        for tag in ["en/../../source", "en\n", "e", "en--GB", "éé"] {
            assert!(validate_language(tag).is_err());
        }
        let mut bad = carrier();
        bad.as_object_mut().unwrap().remove("content_revision");
        assert!(coverage_row(&bad, "auto").is_err());
    }
}
