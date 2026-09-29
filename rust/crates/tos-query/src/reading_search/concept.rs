//! Exact retained concept predecessor semantics, without loading executable
//! builders from a selected source dataset.
use super::local::{Reader, Root, array, corrupt, hash, string};
use super::*;
use std::collections::{BTreeMap, BTreeSet};
use unicode_normalization::UnicodeNormalization;
const ROUTE: &str = "ToS/candidate-intake/zarathustra/concept-workbench-v1";
const PRIVATE: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/local-content/concept-workbench-v1";
const DE_SUFFIXES: &[&str] = &[
    "ern", "est", "em", "en", "er", "es", "te", "st", "et", "e", "n", "s", "t",
];
const RU_SUFFIXES: &[&str] = &[
    "иями",
    "ями",
    "ами",
    "ого",
    "ему",
    "ому",
    "ими",
    "ыми",
    "аться",
    "яться",
    "ить",
    "ать",
    "ять",
    "ешь",
    "ишь",
    "ете",
    "ите",
    "ут",
    "ют",
    "ат",
    "ят",
    "ла",
    "ли",
    "ло",
    "ого",
    "его",
    "ому",
    "ему",
    "ой",
    "ей",
    "ий",
    "ый",
    "ая",
    "яя",
    "ую",
    "юю",
    "ов",
    "ев",
    "ам",
    "ям",
    "ах",
    "ях",
    "ом",
    "ем",
    "ою",
    "ею",
    "ы",
    "и",
    "а",
    "я",
    "у",
    "ю",
    "е",
    "о",
    "ь",
];
fn fold(read: &mut Reader<'_>, s: &str) -> Result<String> {
    read.work(s.len())?;
    let n = s.chars().count();
    tos_foundation::python_casefold_unicode16_v1(
        s,
        n,
        n.checked_mul(3).ok_or_else(budget_error)?,
        s.len().checked_mul(3).ok_or_else(budget_error)?,
    )
    .map_err(|_| budget_error())
}
fn normalize(read: &mut Reader<'_>, s: &str, lang: &str) -> Result<String> {
    read.work(s.len())?;
    let normalized = if lang == "en" {
        s.nfkc().collect::<String>()
    } else {
        s.nfc().collect::<String>()
    };
    let mut value = fold(read, &normalized)?;
    if lang == "ru" {
        value = value
            .chars()
            .map(|c| match c {
                'ѣ' => 'е',
                'і' | 'ї' | 'ѵ' => 'и',
                'ѳ' => 'ф',
                _ => c,
            })
            .collect();
        if value.chars().count() > 2 && value.ends_with('ъ') {
            value.pop();
        }
    } else if lang == "en" {
        // Python str.split includes U+001C..U+001F in whitespace.
        value = value
            .split(|c: char| c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c))
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
    }
    Ok(value)
}
fn signatures(read: &mut Reader<'_>, value: &str, lang: &str) -> Result<BTreeSet<String>> {
    let mut value = fold(read, &value.nfc().collect::<String>())?;
    if lang == "de" {
        value = value
            .replace('ä', "a")
            .replace('ö', "o")
            .replace('ü', "u")
            .replace('ß', "ss");
    }
    let mut out = BTreeSet::from([value.clone()]);
    for suffix in if lang == "de" {
        DE_SUFFIXES
    } else {
        RU_SUFFIXES
    } {
        read.work(1)?;
        if let Some(stem) = value.strip_suffix(suffix) {
            if stem.chars().count() >= 4 {
                out.insert(stem.to_owned());
            }
        }
    }
    Ok(out)
}
fn aliases(read: &mut Reader<'_>, request: &Value) -> Result<Vec<Value>> {
    let mut rows = Vec::new();
    for (lang, display) in request["labels"]
        .as_object()
        .ok_or_else(|| corrupt("concept labels absent"))?
    {
        let display = display
            .as_str()
            .ok_or_else(|| corrupt("concept label not string"))?;
        rows.push(json!({"language":lang,"display":display,"normalized":normalize(read,display,lang)?,"role":"preferred_label","tier":"direct"}));
    }
    for lang in ["de", "ru"] {
        for (field, role, tier) in [
            ("lexical_probes", "direct_lexical_probe", "direct"),
            (
                "semantic_neighbor_probes",
                "semantic_neighbor_probe",
                "semantic_neighbor",
            ),
        ] {
            for display in array(&request[field], lang)? {
                let display = display
                    .as_str()
                    .ok_or_else(|| corrupt("concept probe not string"))?;
                rows.push(json!({"language":lang,"display":display,"normalized":normalize(read,display,lang)?,"role":role,"tier":tier}));
            }
        }
    }
    let rank = |r: &Value| match r["role"].as_str() {
        Some("preferred_label") => 0,
        Some("direct_lexical_probe") => 1,
        _ => 2,
    };
    let mut strongest = BTreeMap::new();
    for row in rows {
        read.work(1)?;
        let key = (
            string(&row, "language")?.to_owned(),
            string(&row, "normalized")?.to_owned(),
            string(&row, "tier")?.to_owned(),
        );
        if strongest.get(&key).is_none_or(|old| rank(&row) < rank(old)) {
            strongest.insert(key, row);
        }
    }
    let mut rows = strongest.into_values().collect::<Vec<_>>();
    rows.sort_by(|a, b| {
        rank(a)
            .cmp(&rank(b))
            .then_with(|| a["language"].as_str().cmp(&b["language"].as_str()))
            .then_with(|| a["normalized"].as_str().cmp(&b["normalized"].as_str()))
    });
    Ok(rows)
}
fn resolve(read: &mut Reader<'_>, query: &str, lang: &str, aliases: &[Value]) -> Result<Value> {
    let normalized = normalize(read, query, lang)?;
    let mut candidates = Vec::new();
    let mut query_signatures = None;
    for alias in aliases {
        read.work(1)?;
        if string(alias, "language")? != lang {
            continue;
        }
        let direct = string(alias, "tier")? == "direct";
        if normalized == string(alias, "normalized")? {
            candidates.push((if direct { 0 } else { 2 }, alias, "exact_alias"));
        } else if lang != "en" {
            if query_signatures.is_none() {
                query_signatures = Some(signatures(read, &normalized, lang)?);
            }
            if !query_signatures
                .as_ref()
                .expect("query signatures initialized for morphology comparison")
                .is_disjoint(&signatures(read, string(alias, "normalized")?, lang)?)
            {
                candidates.push((
                    if direct { 1 } else { 3 },
                    alias,
                    "morphology_alias_candidate",
                ));
            }
        }
    }
    candidates.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| a.1["normalized"].as_str().cmp(&b.1["normalized"].as_str()))
            .then_with(|| a.1["display"].as_str().cmp(&b.1["display"].as_str()))
    });
    let (_, alias, method) = candidates.first().ok_or_else(|| {
        error(
            ReadingSearchErrorCode::InvalidRequest,
            "no concept-search route for query",
        )
    })?;
    Ok(
        json!({"input":query,"language":lang,"normalized":normalized,
        "matched_alias":alias["display"],"matched_alias_language":alias["language"],"matched_alias_role":alias["role"],
        "match_method":method,"resolution_tier":alias["tier"],
        "resolution_status":if *method=="exact_alias" && alias["tier"]=="direct" {"proposed"}else{"ambiguous"}}),
    )
}
fn put_by(rows: Vec<Value>, key: &str) -> Result<BTreeMap<String, Value>> {
    rows.into_iter()
        .map(|r| Ok((string(&r, key)?.to_owned(), r)))
        .collect()
}
fn refs(row: &Value, key: &str) -> Result<Vec<String>> {
    array(row, key)?
        .iter()
        .map(|r| {
            r.as_str()
                .map(str::to_owned)
                .ok_or_else(|| corrupt("concept ref not string"))
        })
        .collect()
}
/// Returns every selected German card, never a prefix. The inherited result
/// identity uses Python sys.maxsize (the supported 64-bit installed profile).
pub(super) fn baseline(
    read: &mut Reader<'_>,
    query: &str,
    r: &ReadingSearchRequest,
) -> Result<(Value, Vec<String>)> {
    let request_ref = r.request_ref.as_deref().unwrap_or(DEFAULT_REQUEST_REF);
    let request_raw = read.bytes(Root::Source, request_ref, false)?;
    read.validate(REQUEST_SCHEMA_REF, &request_raw)?;
    let request = read.parse(&request_raw)?;
    let key = string(&request, "request_key")?;
    let version = request["request_version"]
        .as_u64()
        .ok_or_else(|| corrupt("concept request version invalid"))?;
    let identity = string(&request, "request_identity_key")?;
    let default = key == "fate" && request_ref == DEFAULT_REQUEST_REF;
    let scope = format!(
        "{key}-v{version}-{}",
        identity
            .rsplit('-')
            .next()
            .ok_or_else(|| corrupt("concept identity invalid"))?
    );
    let output = if default {
        format!("{ROUTE}/outputs/fate")
    } else {
        format!("{ROUTE}/outputs/{scope}")
    };
    let manifest_ref = if default {
        format!("{ROUTE}/manifest.v1.json")
    } else {
        format!("{output}/manifest.v1.json")
    };
    let private_request_ref = format!(
        "{PRIVATE}/requests/{}.request-analysis.v1.json",
        if default { "fate" } else { &scope }
    );
    let db_ref = format!("{PRIVATE}/workbench-index.v1.sqlite3");
    let private_request_file = read.open_private_request(&private_request_ref)?;
    drop(private_request_file);
    let private_db = read.open_private_request(&db_ref)?;
    let aliases = aliases(read, &request)?;
    let query_analysis = resolve(read, query, &r.language, &aliases)?;
    let semantic =
        r.include_semantic_neighbors || query_analysis["resolution_tier"] == "semantic_neighbor";
    let concept_ref = format!("{output}/concept-candidate.v1.json");
    let occurrences_ref = format!("{output}/occurrence-spine.v1.jsonl");
    let relations_ref = format!("{output}/relation-candidates.v1.jsonl");
    let tasks_ref = format!("{output}/english-on-demand-worklist.v1.jsonl");
    let contexts_ref = format!("{ROUTE}/context-unit-spine.v1.jsonl");
    let manifest = read.json(Root::Source, &manifest_ref, false)?;
    if string(&manifest, "schema_version")? != "tos_zarathustra_concept_workbench_manifest_v1" {
        return Err(corrupt("unsupported concept manifest"));
    }
    let current_schema_sha = read.file_hash(Root::Software, CONCEPT_SCHEMA_REF)?;
    // Exact reviewed v1 historical schema differs only in adapter provenance.
    // Source snapshot fixity remains unchanged; every native result validates
    // against the current embedded schema and reports its current digest.
    const HISTORICAL_CONCEPT_SCHEMA_SHA: &str =
        "5f67d5b3abf88ecd88dcdb94f70eee0cb685b7ac81b7abbd41bc2b14542c3cc3";
    let declared_schema_sha = string(&manifest, "concept_search_result_schema_sha256")?;
    if declared_schema_sha != current_schema_sha
        && declared_schema_sha != HISTORICAL_CONCEPT_SCHEMA_SHA
    {
        return Err(corrupt("concept result schema differs from manifest"));
    }
    let tracked = array(&manifest, "artifacts")?
        .iter()
        .map(|v| {
            Ok((
                string(v, "ref")?.to_owned(),
                string(v, "sha256")?.to_owned(),
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    let private = array(&manifest, "private_artifacts")?
        .iter()
        .map(|v| {
            Ok((
                string(v, "ref")?.to_owned(),
                string(v, "sha256")?.to_owned(),
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    for reference in [
        &concept_ref,
        &occurrences_ref,
        &relations_ref,
        &tasks_ref,
        &contexts_ref,
    ] {
        read.verified_file(
            Root::Source,
            reference,
            tracked
                .get(reference)
                .ok_or_else(|| corrupt("concept tracked fixity absent"))?,
            false,
        )?;
    }
    for reference in [&private_request_ref, &db_ref] {
        read.verified_file(
            Root::Source,
            reference,
            private
                .get(reference)
                .ok_or_else(|| corrupt("concept private fixity absent"))?,
            true,
        )?;
    }
    let concept = read.json(Root::Source, &concept_ref, false)?;
    let occurrences = read.jsonl(Root::Source, &occurrences_ref)?;
    let relations = read.jsonl(Root::Source, &relations_ref)?;
    let tasks = read.jsonl(Root::Source, &tasks_ref)?;
    let public_contexts = put_by(read.jsonl(Root::Source, &contexts_ref)?, "context_unit_ref")?;
    let private_request = read.json(Root::Source, &private_request_ref, true)?;
    if private_request["concept_candidate_ref"] != concept["concept_candidate_id"] {
        return Err(corrupt("private and tracked concept candidates disagree"));
    }
    let concept_id = string(&concept, "concept_candidate_id")?;
    let mut source = occurrences
        .iter()
        .filter(|o| {
            o["language"] == "de" && (semantic || o["evidence_tier"] == "direct_or_morphological")
        })
        .collect::<Vec<_>>();
    let ordinal = |v: &Value, k: &str| v[k].as_i64().unwrap_or(i64::MIN);
    source.sort_by(|a, b| {
        ordinal(a, "part")
            .cmp(&ordinal(b, "part"))
            .then_with(|| ordinal(a, "witness_order").cmp(&ordinal(b, "witness_order")))
            .then_with(|| ordinal(a, "token_ordinal").cmp(&ordinal(b, "token_ordinal")))
            .then_with(|| {
                a["occurrence_candidate_id"]
                    .as_str()
                    .cmp(&b["occurrence_candidate_id"].as_str())
            })
    });
    let db = read.database(&private_db)?;
    // Exact predecessor oracle fetches all request occurrence refs, including
    // Russian/semantic members, then all source contexts for alignment lookup.
    // Chunking only changes the SQL bind count, never selected membership.
    let mut exact = BTreeMap::new();
    for chunk in occurrences.chunks(500) {
        read.work(chunk.len())?;
        let args = chunk
            .iter()
            .map(|o| o["existing_occurrence_ref"].clone())
            .collect::<Vec<_>>();
        let placeholders = vec!["?"; args.len()].join(",");
        exact.extend(put_by(read.sql(&db,&format!("SELECT * FROM exact_occurrences WHERE existing_occurrence_ref IN ({placeholders})"),&args)?,"existing_occurrence_ref")?);
    }
    if occurrences.iter().any(|o| {
        o["existing_occurrence_ref"]
            .as_str()
            .is_none_or(|id| !exact.contains_key(id))
    }) {
        return Err(corrupt(
            "concept source return missing selected occurrences",
        ));
    }
    let mut contexts = BTreeMap::new();
    let mut alignment_members = BTreeMap::<String, BTreeMap<String, Vec<String>>>::new();
    for mut context in read.sql(
        &db,
        "SELECT * FROM context_units ORDER BY language,witness_order",
        &[],
    )? {
        context["alignment_links"] = local::list(&context, "alignment_links_json", read)?;
        let object = context
            .as_object_mut()
            .ok_or_else(|| corrupt("concept context invalid"))?;
        object.remove("alignment_links_json");
        object.remove("analysis_tokens_json");
        let context_id = string(&context, "context_unit_ref")?.to_owned();
        for link in array(&context, "alignment_links")? {
            alignment_members
                .entry(string(link, "alignment_ref")?.to_owned())
                .or_default()
                .entry(string(&context, "language")?.to_owned())
                .or_default()
                .push(context_id.clone());
        }
        contexts.insert(context_id, context);
    }
    let mut realizations = BTreeMap::new();
    let mut translation_relations = BTreeMap::<String, Vec<String>>::new();
    for relation in &relations {
        read.work(1)?;
        let kind = string(relation, "relation_type")?;
        if [
            "lexical_realization",
            "morphological_realization",
            "semantic_neighbor_candidate",
        ]
        .contains(&kind)
            && refs(relation, "object_refs")?
                .iter()
                .any(|r| r == concept_id)
        {
            for id in refs(relation, "subject_refs")? {
                realizations.insert(id, relation);
            }
        }
        if kind == "translation_parallel_candidate" {
            for id in refs(relation, "subject_refs")?
                .into_iter()
                .chain(refs(relation, "object_refs")?)
            {
                translation_relations
                    .entry(id)
                    .or_default()
                    .push(string(relation, "relation_candidate_id")?.to_owned());
            }
        }
    }
    let english = tasks
        .iter()
        .map(|t| {
            Ok((
                string(t, "source_occurrence_ref")?.to_owned(),
                t["english_task_id"].clone(),
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    let mut occurrences_by_context = BTreeMap::<String, Vec<&Value>>::new();
    for occurrence in &occurrences {
        if let Some(id) = occurrence["context_unit_ref"]
            .as_str()
            .filter(|s| !s.is_empty())
        {
            occurrences_by_context
                .entry(id.to_owned())
                .or_default()
                .push(occurrence);
        }
    }
    let probes = array(&private_request, "selected_forms")?
        .iter()
        .map(|f| {
            Ok((
                (
                    string(f, "language")?.to_owned(),
                    string(f, "analysis_key_sha256")?.to_owned(),
                    string(f, "selection_kind")?.to_owned(),
                ),
                f["probe_display"].clone(),
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    let route_id = format!(
        "tos.navigation.concept-search-route.sid-{}",
        &hash(&format!("zarathustra-concept-search-route\n{identity}"))[..32]
    );
    let result_id = format!(
        "tos.navigation.concept-search-result.sid-{}",
        &hash(&format!(
            "{route_id}\n{}\n{}\nsemantic={}\nlimit=9223372036854775807",
            r.language,
            string(&query_analysis, "normalized")?,
            if semantic { "True" } else { "False" }
        ))[..32]
    );
    let mut rows = Vec::new();
    let mut counts = BTreeMap::<String, usize>::new();
    for (rank, occurrence) in source.iter().enumerate() {
        read.work(1)?;
        *counts
            .entry(string(occurrence, "evidence_tier")?.to_owned())
            .or_default() += 1;
        let occurrence_id = string(occurrence, "occurrence_candidate_id")?;
        let exact_row = exact
            .get(string(occurrence, "existing_occurrence_ref")?)
            .ok_or_else(|| corrupt("concept exact occurrence absent"))?;
        let context_id = string(occurrence, "context_unit_ref")?;
        let context = contexts
            .get(context_id)
            .ok_or_else(|| corrupt("concept context absent"))?;
        let relation = realizations
            .get(occurrence_id)
            .ok_or_else(|| corrupt("source occurrence has no concept realization path"))?;
        let mut ru_refs = BTreeSet::new();
        for link in array(context, "alignment_links")? {
            if let Some(members) = alignment_members
                .get(string(link, "alignment_ref")?)
                .and_then(|m| m.get("ru"))
            {
                ru_refs.extend(members.iter().cloned());
            }
        }
        let mut comparators = Vec::new();
        for id in ru_refs {
            let russian = contexts
                .get(&id)
                .ok_or_else(|| corrupt("Russian context absent"))?;
            let selected = occurrences_by_context.get(&id).cloned().unwrap_or_default();
            let mut surfaces = Vec::new();
            for o in &selected {
                surfaces.push(
                    exact
                        .get(string(o, "existing_occurrence_ref")?)
                        .ok_or_else(|| corrupt("Russian exact occurrence absent"))?["exact_form"]
                        .clone(),
                );
            }
            comparators.push(json!({"context_unit_ref":id,"exact_text":russian["exact_text"],
                "selected_occurrence_refs":selected.iter().map(|o|o["occurrence_candidate_id"].clone()).collect::<Vec<_>>(),
                "selected_surfaces":surfaces,"role":"historical_translation_comparator_not_source_authority"}));
        }
        let probe = probes
            .get(&(
                "de".to_owned(),
                string(occurrence, "analysis_key_sha256")?.to_owned(),
                string(occurrence, "selection_kind")?.to_owned(),
            ))
            .ok_or_else(|| corrupt("concept source probe absent"))?;
        let mut translation = translation_relations
            .get(occurrence_id)
            .cloned()
            .unwrap_or_default();
        translation.sort();
        let public_context = public_contexts
            .get(context_id)
            .ok_or_else(|| corrupt("public concept context absent"))?;
        rows.push(json!({"rank":rank+1,"source_language":"de","evidence_tier":occurrence["evidence_tier"],
            "selection_kind":occurrence["selection_kind"],"part":occurrence["part"],"reading_ref":occurrence["reading_ref"],
            "unit_kind":occurrence["unit_kind"],"witness_order":occurrence["witness_order"],"token_ordinal":occurrence["token_ordinal"],
            "occurrence_ordinal_within_context":occurrence["occurrence_ordinal_within_context"],
            "source_occurrence_candidate_ref":occurrence_id,"source_existing_occurrence_ref":occurrence["existing_occurrence_ref"],
            "source_context_unit_ref":context_id,"source_surface":exact_row["exact_form"],"source_analysis_form":exact_row["analysis_key"],
            "source_headword_candidate":probe,"source_headword_status":"request_probe_not_accepted_lemma",
            "source_context":context["exact_text"],"anchor_refs":public_context["anchor_refs"],
            "speaker":{"role":context["speaker_role"],"status":context["speaker_status"]},
            "alignment_candidates":context["alignment_links"],"russian_comparators":comparators,
            "realization_relation_candidate_ref":relation["relation_candidate_id"],"translation_parallel_candidate_refs":translation,
            "english_on_demand_task_ref":english.get(occurrence_id),
            "query_to_source_path":[
                {"step":"query_alias_match","from_ref":format!("query:{}:{}",r.language,string(&query_analysis,"normalized")?),"to_ref":route_id,"status":query_analysis["resolution_status"]},
                {"step":"routes_to_concept_candidate","from_ref":route_id,"to_ref":concept_id,"status":"navigation_only"},
                {"step":"candidate_realization","from_ref":concept_id,"to_ref":occurrence_id,"status":format!("reverse_navigation_over_{}_{}",string(relation,"status")?,string(relation,"relation_type")?)},
                {"step":"source_return","from_ref":occurrence_id,"to_ref":occurrence["existing_occurrence_ref"],"status":"exact_witness_return"}],
            "accepted":false,"review_status":"unreviewed","semantic_fact_asserted":false,"translation_truth_asserted":false,"graph_effect":false,"canon_effect":false}));
    }
    let result = json!({"schema_version":"tos_zarathustra_concept_search_result_v1","search_result_id":result_id,
        "query_analysis":query_analysis,"concept_search_route":{"route_id":route_id,"identity_basis_ref":identity,
            "identity_posture":"stable_navigation_identity_not_semantic_concept_identity","labels":request["labels"],"aliases":aliases,
            "current_request_ref":request_ref,"current_request_id":concept["request_id"],"accepted_concept_ref":concept["concept_id"]},
        "concept_candidate_ref":concept_id,"work_ref":request["scope"]["work_ref"],"content_posture":"local_runtime_exact_source_return_not_tracked",
        "authority_boundary":"navigation_to_candidate_evidence_only_no_semantic_translation_graph_or_canon_acceptance",
        "provenance":{"source_manifest_ref":manifest_ref,"source_manifest_sha256":read.file_hash(Root::Source,&manifest_ref)?,
            "query_adapter_ref":CONCEPT_ADAPTER_REF,"query_adapter_sha256":read.file_hash(Root::Software,CONCEPT_ADAPTER_REF)?,
            "result_schema_ref":CONCEPT_SCHEMA_REF,"result_schema_sha256":read.file_hash(Root::Software,CONCEPT_SCHEMA_REF)?},
        "coverage":{"total_source_results":rows.len(),"returned_source_results":rows.len(),"source_evidence_tier_counts":counts,
            "semantic_neighbors_included":semantic,"original_language":"de","russian_query_is_source_authority":false},"results":rows});
    let body = read.emit(&result)?;
    read.validate(CONCEPT_SCHEMA_REF, &body)?;
    let db_hashes = private
        .into_iter()
        .filter(|(r, _)| r.ends_with(".sqlite3"))
        .map(|(_, sha)| sha)
        .collect();
    Ok((result, db_hashes))
}

#[cfg(test)]
mod controls {
    use super::*;
    // These are exact Python16 primitive oracle outcomes. Full SQL/provider
    // controls are maintained in the reading-search integration target.
    #[test]
    fn unicode_normalization_uses_exact_declared_profile() {
        assert_eq!("Scho\u{308}n".nfc().collect::<String>(), "Schön");
        assert_eq!("Ｆａｔｅ".nfkc().collect::<String>(), "Fate");
        assert_eq!(unicode_normalization::UNICODE_VERSION, (16, 0, 0));
    }
}
