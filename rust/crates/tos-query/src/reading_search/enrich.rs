use super::local::{Reader, array, corrupt, hash, list, string};
use super::*;
use rusqlite::Connection;
use std::collections::{BTreeMap, BTreeSet};
type Alignments = BTreeMap<String, Vec<Value>>;
fn offset(v: &Value, k: &str) -> Result<usize> {
    usize::try_from(
        v[k].as_u64()
            .ok_or_else(|| corrupt("invalid reading offset"))?,
    )
    .map_err(|_| corrupt("reading offset overflow"))
}
fn contains(a: &Value, b: &Value) -> Result<bool> {
    Ok(offset(a, "start_offset")? <= offset(b, "start_offset")?
        && offset(a, "end_offset")? >= offset(b, "end_offset")?)
}
fn overlaps(a: &Value, b: &Value) -> Result<bool> {
    Ok(offset(a, "start_offset")? < offset(b, "end_offset")?
        && offset(a, "end_offset")? > offset(b, "start_offset")?)
}
fn checked_anchor(read: &mut Reader<'_>, row: &Value, context: &str) -> Result<Value> {
    read.work(context.len())?;
    let start = offset(row, "start_offset")?;
    let end = offset(row, "end_offset")?;
    if start >= end {
        return Err(corrupt("invalid source-local reading anchor"));
    }
    // Offsets belong to Unicode code points, including astral characters.
    let exact = context
        .chars()
        .skip(start)
        .take(end - start)
        .collect::<String>();
    if context.chars().count() < end
        || exact != string(row, "exact_text")?
        || hash(&exact) != string(row, "exact_sha256")?
    {
        return Err(corrupt("reading evidence is not exact source substring"));
    }
    Ok(
        json!({"context_unit_ref":row["context_unit_ref"],"start_offset":start,"end_offset":end,
        "offset_unit":"unicode_codepoint","offset_scope":"context_local_half_open","exact_text":exact,"exact_sha256":row["exact_sha256"]}),
    )
}
fn rows(read: &mut Reader<'_>, db: &Connection, table: &str, context: &str) -> Result<Vec<Value>> {
    // table names originate only in fixed internal call sites.
    read.sql(
        db,
        &format!("SELECT * FROM {table} WHERE context_unit_ref=? ORDER BY start_offset,end_offset"),
        &[json!(context)],
    )
}
pub(super) fn alignment_index(read: &mut Reader<'_>, db: &Connection) -> Result<Alignments> {
    let mut index = Alignments::new();
    for row in read.sql(
        db,
        "SELECT * FROM translation_alignments ORDER BY alignment_id",
        &[],
    )? {
        if local::truthy(&row["semantic_equivalence_asserted"])
            || local::truthy(&row["human_acceptance"])
        {
            return Err(corrupt(
                "candidate alignment unexpectedly asserts acceptance",
            ));
        }
        let packet = json!({"alignment_id":row["alignment_id"],"granularity":row["granularity"],
            "candidate_role":row["candidate_role"],"status":row["status"],"shape":row["correspondence_shape"],
            "source_unit_refs":list(&row,"ordered_source_unit_refs_json",read)?,"target_unit_refs":list(&row,"ordered_target_unit_refs_json",read)?,
            "exact_source_text":row["exact_source_text"],"exact_target_text":row["exact_target_text"],
            "parent_paragraph_alignment_ref":row["parent_paragraph_alignment_ref"],"reason_codes":list(&row,"reason_codes_json",read)?,
            "competing_alignment_refs":list(&row,"competing_alignment_refs_json",read)?,"word_correspondence_asserted":false,"translation_truth_asserted":false});
        for id in array(&packet, "source_unit_refs")? {
            let id = id
                .as_str()
                .ok_or_else(|| corrupt("alignment source unit invalid"))?;
            read.work(id.len())?;
            index
                .entry(id.to_owned())
                .or_default()
                .push(read.clone_value(&packet)?);
        }
    }
    Ok(index)
}
pub(super) fn enrich(
    read: &mut Reader<'_>,
    db: &Connection,
    mut card: Value,
    alignments: &Alignments,
    explicit: Option<Vec<Value>>,
) -> Result<Value> {
    let context_ref = string(&card, "source_context_unit_ref")?.to_owned();
    let context = string(&card, "source_context")?.to_owned();
    read.work(context.len())?;
    let span_rows = if let Some(rows) = explicit {
        rows
    } else {
        read.sql(db,"SELECT * FROM occurrence_spans WHERE existing_occurrence_ref=? ORDER BY start_offset,end_offset",&[card["source_existing_occurrence_ref"].clone()])?
    };
    let mut spans = Vec::new();
    for row in span_rows {
        if string(&row, "context_unit_ref")? != context_ref
            || string(&row, "exact_text")? != string(&card, "source_surface")?
        {
            return Err(corrupt("occurrence crosswalk binds wrong source text"));
        }
        let mut span = checked_anchor(read, &row, &context)?;
        span["surface_unit_ref"] = row["surface_unit_ref"].clone();
        span["status"] = row["status"].clone();
        spans.push(span);
    }
    let span = if spans.len() == 1 {
        Some(spans[0].clone())
    } else {
        None
    };
    card["source_occurrence_anchor_status"] = json!(if span.is_some() {
        "exact"
    } else if !spans.is_empty() {
        "ambiguous"
    } else {
        "deferred"
    });
    let mut predecessor = card["speaker"].clone();
    predecessor["scope"] = json!("paragraph_candidate_not_occurrence_attribution");
    predecessor["layer"] = json!("concept-workbench-v1");
    card["speaker_predecessor"] = predecessor;
    let mut discourse = Vec::new();
    for row in rows(read, db, "discourse_segments", &context_ref)? {
        if let Some(span) = &span {
            if overlaps(&row, span)? {
                discourse.push(json!({"segment_id":row["segment_id"],"sentence_unit_ref":row["sentence_unit_ref"],
                "source_anchor":checked_anchor(read,&row,&context)?,"role":row["speaker_role"],"status":row["speaker_status"],
                "candidates":list(&row,"speaker_candidates_json",read)?,"evidence_refs":list(&row,"evidence_refs_json",read)?,
                "kind":row["kind"],"quote_depth":row["quote_depth"],"speech_turn_id":row["speech_turn_id"],
                "utterer_role":row["utterer_role"],"performed_role":row["performed_role"],"modality":row["modality"],
                "attribution_basis":row["attribution_basis"],"contains_occurrence":contains(&row,span)?}));
            }
        }
    }
    let selected = discourse
        .iter()
        .filter(|d| d["contains_occurrence"] == true)
        .collect::<Vec<_>>();
    let speaker = if selected.len() == 1 {
        let d = selected[0];
        json!({"role":d["role"],"status":d["status"],"candidates":d["candidates"],"evidence_refs":d["evidence_refs"],
            "segment_id":d["segment_id"],"scope":"occurrence_containing_segment","utterer_role":d["utterer_role"],
            "performed_role":d["performed_role"],"modality":d["modality"],"attribution_basis":d["attribution_basis"]})
    } else {
        let roles = discourse
            .iter()
            .map(|d| string(d, "role").map(str::to_owned))
            .collect::<Result<BTreeSet<_>>>()?;
        json!({"role":"unresolved","status":if !discourse.is_empty() || !spans.is_empty(){"ambiguous"}else{"deferred"},
            "scope":"occurrence_not_resolved","segment_id":null,"candidates":roles,"evidence_refs":[],
            "utterer_role":null,"performed_role":null,"modality":null,"attribution_basis":"no_unique_containing_segment"})
    };
    card["speaker"] = speaker.clone();
    card["source_occurrence_spans"] = json!(spans);
    card["discourse_segments"] = json!(discourse);
    let mut unit_refs = Vec::new();
    for (table, id_field, output) in [
        ("source_sentences", "sentence_unit_ref", "source_sentences"),
        ("source_clauses", "clause_id", "source_clauses"),
    ] {
        let mut units = Vec::new();
        for row in rows(read, db, table, &context_ref)? {
            if let Some(span) = &span {
                if overlaps(&row, span)? {
                    unit_refs.push(string(&row, id_field)?.to_owned());
                    units.push(json!({"unit_ref":row[id_field],"source_anchor":checked_anchor(read,&row,&context)?,
                    "contains_occurrence":contains(&row,span)?,"status":row.get("status").cloned().unwrap_or(json!("proposed"))}));
                }
            }
        }
        card[output] = json!(units);
    }
    // Python dictionary keeps first insertion order on replacement. Preserve
    // sentence/clause traversal and alignment encounter order exactly.
    let mut fine = Vec::<Value>::new();
    let mut fine_index = BTreeMap::new();
    for id in unit_refs {
        for row in alignments.get(&id).into_iter().flatten() {
            read.work(1)?;
            let key = string(row, "alignment_id")?.to_owned();
            if let Some(i) = fine_index.get(&key) {
                fine[*i] = read.clone_value(row)?;
            } else {
                fine_index.insert(key, fine.len());
                fine.push(read.clone_value(row)?);
            }
        }
    }
    card["alignment_granularity_note"] = json!(if fine.is_empty() {
        "paragraph_comparator_only_or_explicit_alignment_gap"
    } else {
        "sentence_and_clause_candidates_not_word_alignment"
    });
    card["fine_alignment_candidates"] = json!(fine);
    let mut formulas = Vec::new();
    let mut nearby = Vec::new();
    for row in read.sql(db,"SELECT fo.*,f.normalized_text,f.token_count,f.occurrence_count,f.reading_count FROM formula_occurrences fo JOIN formulas f USING(formula_id) WHERE fo.context_unit_ref=? ORDER BY fo.start_offset,fo.formula_id,fo.occurrence_id",&[json!(context_ref)])? {
        let member=if let Some(span)=&span {contains(&row,span)?}else{false};
        let packet=json!({"formula_id":row["formula_id"],"occurrence_id":row["occurrence_id"],"normalized_text":row["normalized_text"],
            "token_count":row["token_count"],"occurrence_count":row["occurrence_count"],"reading_count":row["reading_count"],
            "status":row["status"],"source_anchor":checked_anchor(read,&row,&context)?,"relation":if member{"contains_occurrence"}else{"same_context_only"}});
        if member {formulas.push(packet);}else{nearby.push(packet);}
    }
    let formula_refs = formulas
        .iter()
        .map(|f| string(f, "formula_id").map(str::to_owned))
        .collect::<Result<BTreeSet<_>>>()?;
    card["formula_memberships"] = json!(formulas);
    card["context_formula_memberships"] = json!(nearby);
    card["english_analysis_context"] = json!({"task_ref":card["english_on_demand_task_ref"],"execution_status":"not_executed","source_language":"de",
        "source_sentence_refs":array(&card,"source_sentences")?.iter().map(|r|r["unit_ref"].clone()).collect::<Vec<_>>(),
        "speaker_segment_ref":speaker["segment_id"],"formula_refs":formula_refs,
        "historical_etymology_requires_cited_evidence":true,"english_is_generated_candidate_not_witness":true});
    read.admit_value(&card)?;
    Ok(card)
}
/// Rust regex '\\w' includes marks/joiners, unlike Python Unicode '\\w'.
/// Describe Python's letter and word categories explicitly using the already
/// pinned category library, so combining marks do not silently change bounds.
fn py_word(c: char) -> bool {
    use unicode_general_category::{GeneralCategory::*, get_general_category};
    c == '_'
        || matches!(
            get_general_category(c),
            UppercaseLetter
                | LowercaseLetter
                | TitlecaseLetter
                | ModifierLetter
                | OtherLetter
                | DecimalNumber
                | LetterNumber
                | OtherNumber
        )
}
fn py_letter(c: char) -> bool {
    // [^\W\d_] admits Python alphanumeric non-decimal numeric letters too.
    py_word(c)
        && c != '_'
        && unicode_general_category::get_general_category(c)
            != unicode_general_category::GeneralCategory::DecimalNumber
}
fn fold(read: &mut Reader<'_>, s: &str) -> Result<String> {
    read.work(s.len())?;
    let n = s.chars().count();
    tos_foundation::python_casefold_unicode16_v1(
        s,
        n,
        n.saturating_mul(3),
        s.len().saturating_mul(3),
    )
    .map_err(|_| budget_error())
}
fn dehyphenations(
    read: &mut Reader<'_>,
    text: &str,
    forms: &BTreeSet<String>,
) -> Result<Vec<Value>> {
    let chars = text.chars().collect::<Vec<_>>();
    read.work(chars.len())?;
    let mut out = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        if !py_letter(chars[start]) || (start > 0 && py_word(chars[start - 1])) {
            start += 1;
            continue;
        }
        let mut mark = start;
        while mark < chars.len() && py_letter(chars[mark]) {
            mark += 1;
        }
        if chars.get(mark) != Some(&'¬') {
            start += 1;
            continue;
        }
        let mut next = mark + 1;
        while matches!(chars.get(next), Some(' ' | '\t')) {
            next += 1;
        }
        if chars.get(next) == Some(&'\r') {
            next += 1;
        }
        if chars.get(next) != Some(&'\n') {
            start += 1;
            continue;
        }
        next += 1;
        while matches!(chars.get(next), Some(' ' | '\t')) {
            next += 1;
        }
        let removed_end = next;
        while next < chars.len() && py_letter(chars[next]) {
            next += 1;
        }
        if next == removed_end || (next < chars.len() && py_word(chars[next])) {
            start += 1;
            continue;
        }
        let normalized = chars[start..mark]
            .iter()
            .chain(chars[removed_end..next].iter())
            .collect::<String>();
        if forms.contains(&fold(read, &normalized)?) {
            let exact = chars[start..next].iter().collect::<String>();
            out.push(json!({"start_offset":start,"end_offset":next,"exact_text":exact,"exact_sha256":hash(&exact),"normalized_form":normalized,
                "normalization_operations":[{"operation":"remove_explicit_linebreak_hyphen_mark","start_offset":mark,"end_offset":removed_end,
                    "exact_removed_text":chars[mark..removed_end].iter().collect::<String>(),"offset_scope":"context_local_half_open"}]}));
        }
        // re.finditer consumes the whole match even if selected forms don't fit.
        start = next;
    }
    Ok(out)
}
pub(super) fn normalization_candidates(
    read: &mut Reader<'_>,
    db: &Connection,
    cards: &[Value],
    alignments: &Alignments,
) -> Result<Vec<Value>> {
    let mut forms = BTreeSet::new();
    let mut existing = BTreeSet::new();
    for card in cards {
        for field in ["source_analysis_form", "source_surface"] {
            forms.insert(fold(read, string(card, field)?)?);
        }
        for span in array(card, "source_occurrence_spans")? {
            existing.insert((
                string(card, "source_context_unit_ref")?.to_owned(),
                offset(span, "start_offset")?,
                offset(span, "end_offset")?,
            ));
        }
    }
    let mut candidates = Vec::new();
    for context in read.sql(
        db,
        "SELECT * FROM contexts WHERE language='de' ORDER BY part,witness_order",
        &[],
    )? {
        let text = string(&context, "exact_text")?;
        let context_ref = string(&context, "context_unit_ref")?;
        read.work(text.len())?;
        if hash(text) != string(&context, "exact_sha256")? {
            return Err(corrupt("normalization context hash mismatch"));
        }
        for matched in dehyphenations(read, text, &forms)? {
            if existing.contains(&(
                context_ref.to_owned(),
                offset(&matched, "start_offset")?,
                offset(&matched, "end_offset")?,
            )) {
                continue;
            }
            let id = format!(
                "tos.annotation.normalization-search-candidate.sid-{}",
                &hash(&format!(
                    "explicit-linebreak-v1\n{context_ref}\n{}\n{}\n{}",
                    offset(&matched, "start_offset")?,
                    offset(&matched, "end_offset")?,
                    string(&matched, "exact_sha256")?
                ))[..32]
            );
            let candidate = json!({"candidate_id":id,"source_occurrence_candidate_ref":id,"source_existing_occurrence_ref":null,
                "legacy_occurrence_id_asserted":false,"source_context_unit_ref":context_ref,"source_language":"de","source_context":text,
                "source_surface":matched["exact_text"],"source_analysis_form":matched["normalized_form"],
                "part":context["part"],"reading_ref":context["reading_ref"],"witness_order":context["witness_order"],"rank":candidates.len()+1,
                "evidence_tier":"normalization_candidate","selection_reason":"explicit_linebreak_normalization_matches_selected_german_form",
                "normalization_operations":matched["normalization_operations"],"speaker":{"role":"not_applicable","status":"not_applicable"},
                "english_on_demand_task_ref":null,"russian_comparators":[],"accepted":false,"semantic_fact_asserted":false,
                "translation_truth_asserted":false,"graph_effect":false,"canon_effect":false});
            let mut span = matched.clone();
            span["context_unit_ref"] = json!(context_ref);
            span["surface_unit_ref"] = Value::Null;
            span["status"] = json!("normalization_candidate");
            let mut enriched = enrich(read, db, candidate, alignments, Some(vec![span]))?;
            enriched["speaker_predecessor"] = json!({"role":"not_applicable","status":"not_applicable","scope":"no_legacy_occurrence","layer":"not_applicable"});
            enriched["english_analysis_context"]["source_normalization_candidate_ref"] = json!(id);
            enriched["english_analysis_context"]["normalization_operations"] =
                matched["normalization_operations"].clone();
            let mut comparators = Vec::new();
            for parallel in array(&enriched, "fine_alignment_candidates")? {
                if parallel["granularity"] == "sentence" && parallel["candidate_role"] == "primary"
                {
                    comparators.push(json!({"alignment_id":parallel["alignment_id"],"exact_text":parallel["exact_target_text"],
                        "target_unit_refs":parallel["target_unit_refs"],"status":parallel["status"],"granularity":"sentence_candidate",
                        "role":"historical_translation_comparator_not_source_authority"}));
                }
            }
            enriched["russian_comparators"] = json!(comparators);
            candidates.push(enriched);
        }
    }
    Ok(candidates)
}
pub(super) fn groups(
    read: &mut Reader<'_>,
    cards: &[Value],
    returned: &[Value],
    group_by: &[String],
) -> Result<Value> {
    let returned_refs = returned
        .iter()
        .map(|r| string(r, "source_occurrence_candidate_ref").map(str::to_owned))
        .collect::<Result<BTreeSet<_>>>()?;
    let mut result = json!({"scope":"all_matching_source_occurrences_before_limit"});
    if group_by.iter().any(|s| s == "speaker") {
        let mut speakers = BTreeMap::<Vec<String>, Vec<String>>::new();
        for card in cards {
            read.work(1)?;
            let key = [
                "role",
                "status",
                "utterer_role",
                "performed_role",
                "modality",
            ]
            .iter()
            .map(|field| card["speaker"][field].as_str().unwrap_or("").to_owned())
            .collect();
            speakers
                .entry(key)
                .or_default()
                .push(string(card, "source_occurrence_candidate_ref")?.to_owned());
        }
        let speakers=speakers.into_iter().map(|(key,refs)|json!({"role":key[0],"status":key[1],"matching_count":refs.len(),
            "utterer_role":if key[2].is_empty(){None}else{Some(&key[2])},"performed_role":if key[3].is_empty(){None}else{Some(&key[3])},
            "modality":if key[4].is_empty(){None}else{Some(&key[4])},"returned_occurrence_refs":refs.into_iter().filter(|r|returned_refs.contains(r)).collect::<Vec<_>>()})).collect::<Vec<_>>();
        result["by_speaker"] = json!(speakers);
    }
    if group_by.iter().any(|s| s == "formula") {
        let mut formulas = BTreeMap::<String, (BTreeSet<String>, Value)>::new();
        let mut no_membership = 0;
        for card in cards {
            read.work(1)?;
            let members = array(card, "formula_memberships")?;
            if members.is_empty() {
                no_membership += 1;
            }
            for member in members {
                let entry = formulas
                    .entry(string(member, "formula_id")?.to_owned())
                    .or_insert_with(|| (BTreeSet::new(), member.clone()));
                entry
                    .0
                    .insert(string(card, "source_occurrence_candidate_ref")?.to_owned());
                entry.1 = member.clone();
            }
        }
        let groups=formulas.into_iter().map(|(id,(refs,description))|json!({"formula_id":id,"normalized_text":description["normalized_text"],
            "matching_count":refs.len(),"whole_book_occurrence_count":description["occurrence_count"],
            "returned_occurrence_refs":refs.intersection(&returned_refs).cloned().collect::<Vec<_>>()})).collect::<Vec<_>>();
        result["by_formula"] = json!(groups);
        result["no_formula_membership_count"] = json!(no_membership);
        result["formula_group_posture"] =
            json!("overlapping_exact_normalized_formulas_not_semantic_equivalence");
    }
    Ok(result)
}
