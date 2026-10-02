use super::*;
fn id(ids: &Ids, kind: &str, b: &str) -> String {
    ids[&(kind.to_string(), b.to_string())].clone()
}
fn public_flags(row: &mut Value) {
    for key in [
        "accepted",
        "source_text_included",
        "graph_effect",
        "canon_effect",
    ] {
        row[key] = json!(false);
    }
    row["review_status"] = json!("unreviewed");
}
fn public_contexts(
    root: &ResearchExecution,
    units: &[Value],
    speakers: &[Value],
    ids: &Ids,
) -> Result<(Rows, Rows)> {
    let mut public = vec![];
    for r in speakers {
        root.tick(1)?;
        let mut row = r.clone();
        row.as_object_mut().unwrap().remove("binding");
        row["schema_version"] = json!("tos_zarathustra_speaker_state_candidate_v1");
        row["speaker_state_candidate_id"] = json!(id(ids, "speaker", s(r, "binding")));
        row["review_status"] = json!("unreviewed");
        public.push(row);
    }
    let by: BTreeMap<_, _> = public
        .iter()
        .map(|r| (s(r, "context_unit_ref"), r))
        .collect();
    let mut readings = Vec::<((String, String), Vec<&Value>)>::new();
    let mut keys = BTreeMap::new();
    for u in units {
        root.tick(1)?;
        let key = (s(u, "language").into(), s(u, "reading_ref").into());
        let i = *keys.entry(key.clone()).or_insert_with(|| {
            readings.push((key, vec![]));
            readings.len() - 1
        });
        readings[i].1.push(u);
    }
    let mut contexts = vec![];
    for (_, mut rows) in readings {
        root.tick(1)?;
        rows.sort_by_key(|r| n(r, "witness_order"));
        for (i, r) in rows.iter().enumerate() {
            root.tick(1)?;
            let mut out = fields(
                r,
                &[
                    "context_unit_ref",
                    "language",
                    "part",
                    "reading_ref",
                    "unit_kind",
                    "witness_order",
                    "anchor_refs",
                    "exact_sha256",
                ],
            );
            out["schema_version"] = json!("tos_zarathustra_context_unit_candidate_v1");
            out["previous_context_unit_ref"] = if i > 0 {
                rows[i - 1]["context_unit_ref"].clone()
            } else {
                Value::Null
            };
            out["next_context_unit_ref"] = rows
                .get(i + 1)
                .map(|r| r["context_unit_ref"].clone())
                .unwrap_or(Value::Null);
            out["alignment_refs"] = json!(
                arr(&r["alignment_links"])
                    .iter()
                    .map(|x| x["alignment_ref"].clone())
                    .collect::<Vec<_>>()
            );
            out["alignment_statuses"] = unique(
                arr(&r["alignment_links"])
                    .iter()
                    .map(|x| x["status"].clone()),
            );
            out["alignment_shapes"] = unique(
                arr(&r["alignment_links"])
                    .iter()
                    .map(|x| x["shape"].clone()),
            );
            out["speaker_state_candidate_ref"] =
                by[s(r, "context_unit_ref")]["speaker_state_candidate_id"].clone();
            for k in [
                "source_text_included",
                "accepted",
                "graph_effect",
                "canon_effect",
            ] {
                root.tick(1)?;
                out[k] = json!(false);
            }
            contexts.push(out);
        }
    }
    let sort = |a: &Value, b: &Value| {
        (s(a, "language"), n(a, "witness_order")).cmp(&(s(b, "language"), n(b, "witness_order")))
    };
    contexts.sort_by(sort);
    public.sort_by(sort);
    Ok((contexts, public))
}
fn probe(
    root: &ResearchExecution,
    occ: &[Value],
    excl: &[Value],
    selected: &[Value],
    units: &[Value],
) -> Result<Value> {
    root.tick(1)?;
    let direct: Vec<_> = occ
        .iter()
        .filter(|r| s(r, "evidence_tier") == "direct_or_morphological")
        .collect();
    let by: BTreeMap<_, _> = units
        .iter()
        .map(|u| (s(u, "context_unit_ref"), u))
        .collect();
    let alignments = |lang: &str| {
        direct
            .iter()
            .filter(|r| s(r, "language") == lang && !r["context_unit_ref"].is_null())
            .flat_map(|r| {
                arr(&by[s(r, "context_unit_ref")]["alignment_links"])
                    .iter()
                    .map(|l| s(l, "alignment_ref").to_string())
            })
            .collect::<BTreeSet<_>>()
    };
    let de = alignments("de");
    let ru = alignments("ru");
    let sem: BTreeSet<_> = selected
        .iter()
        .filter(|r| s(r, "language") == "de" && s(r, "probe_display") == "Verhängniss")
        .map(|r| s(r, "analysis_key_sha256"))
        .collect();
    let c = |lang: &str, kind: Option<&str>| {
        direct
            .iter()
            .filter(|r| s(r, "language") == lang && kind.is_none_or(|k| s(r, "unit_kind") == k))
            .count()
    };
    Ok(
        json!({"de_direct_occurrences":c("de",None),"ru_direct_occurrences":c("ru",None),"de_direct_prose_occurrences":c("de",Some("paragraph")),"de_direct_verse_occurrences":c("de",Some("verse_line")),"ru_direct_prose_occurrences":c("ru",Some("paragraph")),"ru_direct_verse_occurrences":c("ru",Some("verse_line")),"parallel_alignment_intersection":de.intersection(&ru).count(),"de_only_alignment_groups":de.difference(&ru).count(),"ru_only_alignment_groups":ru.difference(&de).count(),"alignment_union":de.union(&ru).count(),"lowercase_los_hard_negative_occurrences":excl.iter().filter(|r|s(r,"control_code")=="de_los_particle_or_command").count(),"verhaengniss_family_semantic_candidates":occ.iter().filter(|r|s(r,"language")=="de"&&sem.contains(s(r,"analysis_key_sha256"))).count()}),
    )
}
#[allow(clippy::too_many_arguments)]
pub(super) fn render(
    root: &ResearchExecution,
    c: &Config,
    _plan: &Value,
    request: &Value,
    units: &[Value],
    speakers: &[Value],
    _all_forms: &[Value],
    selected: &[Value],
    occ: &[Value],
    rels: &[Value],
    gaps: &[Value],
    excl: &[Value],
    tasks: &[Value],
    ids: &Ids,
    census: &Value,
    maps: &source::Maps,
) -> Result<(BTreeMap<String, Vec<u8>>, Vec<u8>, Value)> {
    let request_id = id(ids, "request", &binding(request));
    let concept_id = id(ids, "concept", &format!("concept|{}", binding(request)));
    let (contexts, speaker_rows) = public_contexts(root, units, speakers, ids)?;
    let speaker_by: BTreeMap<_, _> = speaker_rows
        .iter()
        .map(|r| (s(r, "context_unit_ref"), r))
        .collect();
    let unit_by: BTreeMap<_, _> = units
        .iter()
        .map(|r| (s(r, "context_unit_ref"), r))
        .collect();
    let mut ordered = selected.to_vec();
    ordered.sort_by(|a, b| {
        (
            s(a, "language"),
            s(a, "analysis_key_sha256"),
            s(a, "selection_kind"),
        )
            .cmp(&(
                s(b, "language"),
                s(b, "analysis_key_sha256"),
                s(b, "selection_kind"),
            ))
    });
    let mut forms = vec![];
    let mut form_ids = BTreeMap::new();
    for r in &ordered {
        root.tick(1)?;
        let b = form_binding(r);
        let fid = id(ids, "form", &local(request, &b));
        form_ids.insert(b, fid.clone());
        let mut out = fields(
            r,
            &[
                "language",
                "analysis_key_sha256",
                "probe_normalized_sha256",
                "selection_kind",
                "selection_method",
                "occurrence_count",
                "status",
            ],
        );
        out["schema_version"] = json!("tos_zarathustra_request_form_family_candidate_v1");
        out["form_family_candidate_id"] = json!(fid);
        out["request_ref"] = json!(request_id);
        out["exact_form_variant_count"] = json!(arr(&r["exact_hashes"]).len());
        out["part_count"] = json!(arr(&r["parts"]).len());
        out["reading_count"] = json!(arr(&r["readings"]).len());
        public_flags(&mut out);
        forms.push(out);
    }
    let mut occurrences = vec![];
    let mut occurrence_ids = BTreeMap::new();
    let mut occurrence_by = BTreeMap::new();
    for r in occ {
        root.tick(1)?;
        let oid = id(ids, "occurrence", &local(request, s(r, "binding")));
        occurrence_ids.insert(s(r, "binding").to_string(), oid.clone());
        let mut out = r.clone();
        for k in ["binding", "form_binding", "surface_private"] {
            root.tick(1)?;
            out.as_object_mut().unwrap().remove(k);
        }
        out["schema_version"] = json!("tos_zarathustra_concept_occurrence_candidate_v1");
        out["occurrence_candidate_id"] = json!(oid);
        out["concept_candidate_ref"] = json!(concept_id);
        out["form_family_candidate_ref"] = json!(form_ids[s(r, "form_binding")]);
        out["speaker_state_candidate_ref"] = if r["context_unit_ref"].is_null() {
            Value::Null
        } else {
            speaker_by[s(r, "context_unit_ref")]["speaker_state_candidate_id"].clone()
        };
        public_flags(&mut out);
        occurrence_by.insert(s(r, "binding").to_string(), out.clone());
        occurrences.push(out);
    }
    let mut english = vec![];
    for task in tasks {
        root.tick(1)?;
        let mut row = fields(
            task,
            &[
                "source_context_unit_ref",
                "source_form_sha256",
                "alignment_refs",
                "russian_comparator_context_refs",
                "required_views",
                "required_analysis_stages",
            ],
        );
        row["schema_version"] = json!("tos_zarathustra_english_on_demand_task_v1");
        row["english_task_id"] =
            json!(id(ids, "english_task", &local(request, s(task, "binding"))));
        row["request_ref"] = json!(request_id);
        row["source_occurrence_ref"] = json!(occurrence_ids[s(task, "source_occurrence_binding")]);
        row["target_language"] = json!("en");
        row["etymology_state"] = json!("citation_required_before_claim");
        row["recognized_english_comparator_state"] = json!("sealed_until_candidate_frozen");
        row["task_status"] = json!("ready_for_on_demand_ai_candidate");
        public_flags(&mut row);
        validate(
            root,
            &format!("{ROUTE}/english-on-demand-task.v1.schema.json"),
            &row,
        )?;
        english.push(row);
    }
    let mut relations = vec![];
    for r in rels {
        root.tick(1)?;
        let subjects: Vec<_> = arr(&r["subject_bindings"])
            .iter()
            .map(|v| occurrence_ids[v.as_str().unwrap()].clone())
            .collect();
        let objects: Vec<_> = arr(&r["object_bindings"])
            .iter()
            .map(|v| match s(r, "object_kind") {
                "occurrence_candidate" => occurrence_ids[v.as_str().unwrap()].clone(),
                "speaker_candidate" => s(
                    speaker_by[v.as_str().unwrap()],
                    "speaker_state_candidate_id",
                )
                .to_string(),
                "form_candidate" => form_ids[v.as_str().unwrap()].clone(),
                _ => concept_id.clone(),
            })
            .collect();
        let mut evidence: BTreeSet<String> = arr(&r["subject_bindings"])
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect();
        if s(r, "object_kind") == "occurrence_candidate" {
            evidence.extend(
                arr(&r["object_bindings"])
                    .iter()
                    .map(|v| v.as_str().unwrap().to_string()),
            );
        }
        let contexts = unique(evidence.iter().filter_map(|b| {
            let v = &occurrence_by[b]["context_unit_ref"];
            if v.is_null() { None } else { Some(v.clone()) }
        }));
        let evidence_refs: Vec<_> = evidence.iter().map(|b| occurrence_ids[b].clone()).collect();
        let mut roles: Rows = subjects
            .iter()
            .map(|e| json!({"evidence_ref":e,"role":"subject_occurrence"}))
            .collect();
        if s(r, "object_kind") == "occurrence_candidate" {
            roles.extend(
                objects
                    .iter()
                    .map(|e| json!({"evidence_ref":e,"role":"object_occurrence"})),
            );
        }
        let row = json!({"schema_version":"tos_zarathustra_concept_relation_candidate_v1","relation_candidate_id":id(ids,"relation",&local(request,s(r,"binding"))),"request_ref":request_id,"relation_type":r["relation_type"],"subject_refs":subjects,"object_refs":objects,"support":r["support"],"status":r["status"],"evidence_refs":evidence_refs,"evidence_roles":roles,"source_context_refs":contexts,"alignment_ref":r["alignment_ref"],"provenance_event_ref":event(request),"maker":{"maker_kind":"software","software_ref":GENERATOR,"method_output_posture":"candidate_not_truth"},"certainty":{"meaning":"candidate_status_not_truth_probability","value":0.5},"competing_relation_refs":[],"accepted":false,"review_status":"unreviewed","semantic_fact_asserted":false,"graph_effect":false,"canon_effect":false});
        validate(
            root,
            &format!("{ROUTE}/concept-relation-candidate.v1.schema.json"),
            &row,
        )?;
        relations.push(row);
    }
    let mut labels = request["labels"].clone();
    labels["identity_dependency"] = json!(false);
    let refs =
        |rows: &[Value], k: &str| json!(rows.iter().map(|r| r[k].clone()).collect::<Vec<_>>());
    let concept = json!({"schema_version":"tos_zarathustra_generic_concept_candidate_v1","concept_candidate_id":concept_id,"request_id":request_id,"workbench_ref":id(ids,"workbench","foundation-v1"),"labels":labels,"work_ref":"tos.work.friedrich-nietzsche.also-sprach-zarathustra","languages":["de","ru"],"distinctions":request["distinctions"],"direct_or_morphological_occurrence_refs":occurrences.iter().filter(|r|s(r,"evidence_tier")=="direct_or_morphological").map(|r|r["occurrence_candidate_id"].clone()).collect::<Vec<_>>(),"semantic_neighbor_occurrence_refs":occurrences.iter().filter(|r|s(r,"evidence_tier")=="semantic_neighbor").map(|r|r["occurrence_candidate_id"].clone()).collect::<Vec<_>>(),"form_family_candidate_refs":refs(&forms,"form_family_candidate_id"),"relation_candidate_refs":refs(&relations,"relation_candidate_id"),"sign_id":null,"concept_id":null,"review_status":"unreviewed","accepted":false,"semantic_fact_asserted":false,"graph_effect":false,"canon_effect":false});
    let mut groups = BTreeMap::<String, Vec<Value>>::new();
    for row in &speaker_rows {
        root.tick(1)?;
        if !row["exception_group"].is_null() {
            groups
                .entry(s(row, "exception_group").into())
                .or_default()
                .push(row["speaker_state_candidate_id"].clone());
        }
    }
    let worklist = json!({"schema_version":"tos_zarathustra_speaker_exception_worklist_v1","speaker_population":speaker_rows.len(),"exception_groups":groups.iter().map(|(k,refs)|json!({"exception_group":k,"candidate_refs":refs,"candidate_count":refs.len()})).collect::<Vec<_>>(),"exception_group_count":groups.len(),"review_outcome_recorded":false});
    let probe = probe(root, &occurrences, excl, selected, units)?;
    if s(request, "request_key") == "fate"
        && probe
            != json!({"de_direct_occurrences":26,"ru_direct_occurrences":29,"de_direct_prose_occurrences":26,"de_direct_verse_occurrences":0,"ru_direct_prose_occurrences":29,"ru_direct_verse_occurrences":0,"parallel_alignment_intersection":20,"de_only_alignment_groups":4,"ru_only_alignment_groups":7,"alignment_union":31,"lowercase_los_hard_negative_occurrences":10,"verhaengniss_family_semantic_candidates":6})
    {
        return Err(format!("fate acceptance probe drift: {probe}"));
    }
    let evidence = count(&occurrences, "evidence_tier");
    let relation_counts = count(&relations, "relation_type");
    let complete = occurrences.iter().all(|r| !r["context_unit_ref"].is_null());
    let coverage = json!({"schema_version":"tos_zarathustra_concept_request_coverage_v1","request_id":request_id,"concept_candidate_ref":concept_id,"alignment_units_scanned":maps.count,"witness_context_units_scanned":units.len(),"exact_occurrences_scanned":n(census,"de_work_scope_occurrences")+n(census,"ru_exact_occurrences"),"source_item_occurrences_examined":n(census,"de_exact_occurrences")+n(census,"ru_exact_occurrences"),"outside_work_occurrences_excluded":n(census,"de_exact_occurrences")-n(census,"de_work_scope_occurrences"),"parts_scanned":4,"languages_scanned":["de","ru"],"form_candidate_count":forms.len(),"occurrence_candidate_count":occurrences.len(),"evidence_tier_counts":evidence,"relation_type_counts":relation_counts,"probe_gap_count":gaps.len(),"selected_occurrences_without_content_context":occurrences.iter().filter(|r|r["context_unit_ref"].is_null()).count(),"english_on_demand_task_count":english.len(),"english_translation_candidate_count":0,"source_return_verified":true,"minimum_form_frequency":1,"semantic_neighbors_counted_as_direct_mentions":false,"acceptance_probe":if s(request,"request_key")=="fate"{probe}else{Value::Null},"requested_probe_exact_absence_count":gaps.len(),"mechanically_complete_with_explicit_probe_gaps":complete,"complete_for_declared_request":complete});
    let referenced: BTreeSet<_> = relations
        .iter()
        .flat_map(|r| {
            arr(&r["subject_refs"])
                .iter()
                .chain(arr(&r["object_refs"]))
                .filter_map(Value::as_str)
        })
        .collect();
    let mut nodes =
        vec![json!({"id":concept_id,"kind":"concept_candidate","labels":request["labels"]})];
    for r in forms
        .iter()
        .filter(|r| referenced.contains(s(r, "form_family_candidate_id")))
    {
        root.tick(1)?;
        nodes.push(json!({"id":r["form_family_candidate_id"],"kind":"form_family_candidate","language":r["language"],"status":r["status"]}));
    }
    for r in occurrences
        .iter()
        .filter(|r| referenced.contains(s(r, "occurrence_candidate_id")))
    {
        root.tick(1)?;
        nodes.push(json!({"id":r["occurrence_candidate_id"],"kind":"occurrence_candidate","language":r["language"],"reading_ref":r["reading_ref"],"status":r["status"]}));
    }
    for r in speaker_rows
        .iter()
        .filter(|r| referenced.contains(s(r, "speaker_state_candidate_id")))
    {
        root.tick(1)?;
        nodes.push(json!({"id":r["speaker_state_candidate_id"],"kind":"speaker_state_candidate","language":r["language"],"status":r["attribution_status"]}));
    }
    let graph = json!({"schema_version":"tos_zarathustra_candidate_concept_graph_v1","request_ref":request_id,"topology":"concept_hub_not_occurrence_clique","empty_result":occurrences.is_empty(),"nodes":nodes,"edges":relations.iter().map(|r|json!({"relation_candidate_ref":r["relation_candidate_id"],"relation_type":r["relation_type"],"subject_refs":r["subject_refs"],"object_refs":r["object_refs"],"status":r["status"]})).collect::<Vec<_>>(),"accepted_edge_count":0,"semantic_fact_asserted":false,"graph_effect":false,"canon_effect":false});
    let token_count = |lang: &str| {
        units
            .iter()
            .filter(|u| s(u, "language") == lang)
            .map(|u| arr(&u["analysis_tokens"]).len())
            .sum::<usize>()
    };
    let all_coverage = json!({"schema_version":"tos_zarathustra_all_form_coverage_v1","alignment_unit_count":maps.count,"witness_context_unit_counts":{"de":3815,"ru":3928},"unit_kind_counts":{"de_paragraph":3447,"de_verse_line":368,"ru_paragraph":3569,"ru_verse_line":359},"source_item_exact_occurrence_counts":{"de":census["de_exact_occurrences"],"ru":census["ru_exact_occurrences"]},"work_scope_exact_occurrence_counts":{"de":census["de_work_scope_occurrences"],"ru":census["ru_exact_occurrences"]},"outside_work_exact_occurrence_counts":{"de":n(census,"de_exact_occurrences")-n(census,"de_work_scope_occurrences"),"ru":0},"work_scope_exact_form_counts":{"de":census["de_work_scope_exact_forms"],"ru":census["ru_exact_forms"]},"work_scope_analysis_form_counts":{"de":census["de_work_scope_analysis_forms"],"ru":census["ru_analysis_forms"]},"analysis_token_counts_in_content_units":{"de":token_count("de"),"ru":token_count("ru")},"minimum_form_frequency":1,"frequency_one_forms_retained":true,"all_analysis_forms_have_explicit_state":true,"exact_and_analysis_tokens_distinct":true,"line_join_reconstruction_overwrites_witness":false,"readable_forms_tracked":false,"private_index_ref":c.private_db,"accepted_lemma_count":0,"semantic_effect":false});
    let summary = json!({"schema_version":"tos_zarathustra_concept_workbench_summary_v1","workbench_id":id(ids,"workbench","foundation-v1"),"alignment_unit_count":maps.count,"alignment_status_counts":maps.statuses,"alignment_shape_counts":maps.shapes,"witness_context_unit_count":units.len(),"speaker_candidate_count":speaker_rows.len(),"context_unit_count":contexts.len(),"all_form_minimum_frequency":1,"english_on_demand_task_count":english.len(),"english_translation_candidate_count":0,"work_scope_exact_occurrence_count":n(census,"de_work_scope_occurrences")+n(census,"ru_exact_occurrences"),"source_item_exact_occurrence_count":n(census,"de_exact_occurrences")+n(census,"ru_exact_occurrences"),"request_count":1,"sample_request":request["request_key"],"sample_form_candidate_count":forms.len(),"sample_occurrence_candidate_count":occurrences.len(),"sample_direct_or_morphological_occurrence_count":evidence["direct_or_morphological"].as_u64().unwrap_or(0),"sample_semantic_neighbor_occurrence_count":evidence["semantic_neighbor"].as_u64().unwrap_or(0),"sample_relation_candidate_count":relations.len(),"sample_exclusion_count":excl.len(),"accepted_candidate_count":0,"human_review_count":0,"graph_effect":false,"canon_effect":false});
    let mut outputs = BTreeMap::new();
    for (key, value) in [
        ("all_forms", all_coverage),
        ("speaker_worklist", worklist),
        ("concept", concept),
        ("graph", graph),
        ("coverage", coverage),
        ("summary", summary.clone()),
    ] {
        root.tick(1)?;
        outputs.insert(c.output(key).to_string(), encode(&value, true));
    }
    for (key, rows) in [
        ("contexts", contexts),
        ("speakers", speaker_rows.clone()),
        ("forms", forms),
        ("occurrences", occurrences.clone()),
        ("relations", relations),
        ("gaps", gaps.to_vec()),
        ("exclusions", excl.to_vec()),
        ("english_tasks", english),
    ] {
        root.tick(1)?;
        outputs.insert(c.output(key).to_string(), jsonl(&rows));
    }
    let selected_refs: BTreeSet<_> = occurrences
        .iter()
        .filter_map(|r| r["context_unit_ref"].as_str())
        .collect();
    let mut readings = Vec::<((String, String), Vec<&Value>)>::new();
    let mut keys = BTreeMap::new();
    for u in units {
        root.tick(1)?;
        let key = (s(u, "language").into(), s(u, "reading_ref").into());
        let i = *keys.entry(key.clone()).or_insert_with(|| {
            readings.push((key, vec![]));
            readings.len() - 1
        });
        readings[i].1.push(u);
    }
    let mut private_contexts = vec![];
    for (_, mut rows) in readings {
        root.tick(1)?;
        rows.sort_by_key(|r| n(r, "witness_order"));
        for (i, r) in rows
            .iter()
            .enumerate()
            .filter(|(_, r)| selected_refs.contains(s(r, "context_unit_ref")))
        {
            root.tick(1)?;
            let window = &rows[i.saturating_sub(1)..(i + 2).min(rows.len())];
            private_contexts.push(json!({"context_unit_ref":r["context_unit_ref"],"language":r["language"],"reading_ref":r["reading_ref"],"speaker_candidate":speaker_by[s(r,"context_unit_ref")],"window":window.iter().map(|u|json!({"context_unit_ref":u["context_unit_ref"],"exact_text":u["text"]})).collect::<Vec<_>>()}));
        }
    }
    let private = json!({"schema_version":"tos_zarathustra_concept_request_private_analysis_v1","request_id":request_id,"concept_candidate_ref":concept_id,"content_posture":"private_exact_source_return_and_mutable_analysis_not_semantic_authority","request":request,"selected_forms":selected,"contexts":private_contexts,"english_generation_policy":request["english_generation"],"english_tasks":tasks.iter().map(|t|json!({"english_task_id":id(ids,"english_task",&local(request,s(t,"binding"))),"source_occurrence_ref":occurrence_ids[s(t,"source_occurrence_binding")],"source_surface":t["source_surface_private"],"source_context_unit_ref":t["source_context_unit_ref"],"source_context":unit_by[s(t,"source_context_unit_ref")]["text"],"russian_comparators":arr(&t["russian_comparator_context_refs"]).iter().map(|r|json!({"context_unit_ref":r,"exact_text":unit_by[r.as_str().unwrap()]["text"]})).collect::<Vec<_>>(),"candidate_output_schema_ref":format!("{ROUTE}/english-translation-candidate.v1.schema.json"),"candidate_materialized":false})).collect::<Vec<_>>()});
    Ok((outputs, encode(&private, true), summary))
}
pub(super) fn manifest(
    root: &ResearchExecution,
    c: &Config,
    plan: &Value,
    request: &Value,
    ids: &Ids,
    db: &[u8],
    private: &[u8],
    outputs: &mut BTreeMap<String, Vec<u8>>,
) -> Result<()> {
    let mut input_refs = plan_input_refs(root)?;
    for r in [
        format!("{ROUTE}/concept-request.v2.schema.json"),
        format!("{ROUTE}/concept-relation-candidate.v1.schema.json"),
        format!("{ROUTE}/english-on-demand-task.v1.schema.json"),
        format!("{ROUTE}/english-translation-candidate.v1.schema.json"),
        format!("{ROUTE}/concept-search-result.v1.schema.json"),
        format!("{ROUTE}/word-analysis-task.v1.schema.json"),
        QUERY.into(),
        WORD.into(),
        c.request_ref.clone(),
        s(&request["english_generation"]["reference_register"], "ref").into(),
        s(&request["english_generation"]["etymology_route"], "ref").into(),
    ] {
        input_refs.push(json!(r));
    }
    let mut refs: Vec<Value> = c.outputs.iter().map(|(_, p)| json!(p)).collect();
    refs.extend([json!(c.private_db), json!(c.private_request)]);
    outputs.insert(c.output("provenance").into(),jsonl(&[json!({"schema_version":"tos_zarathustra_concept_workbench_event_v1","event_id":event(request),"event_type":"candidate_workbench_and_sample_request_built","event_at":plan["frozen_at"],"input_refs":input_refs,"output_refs":refs,"authority_effect":"candidate_only_no_human_review_graph_or_canon_effect"})]));
    let artifacts: Rows = c
        .outputs
        .iter()
        .filter(|(k, _)| k != "manifest")
        .map(|(k, p)| json!({"role":k,"ref":p,"sha256":hash(&outputs[p])}))
        .collect();
    let mut manifest = json!({"schema_version":"tos_zarathustra_concept_workbench_manifest_v1","route_id":"zarathustra-concept-workbench-v1","workbench_id":id(ids,"workbench","foundation-v1"),"request_refs":[{"ref":c.request_ref,"sha256":file_hash(root,&c.request_ref)?}],"identity_issuance_ref":c.issuance,"identity_issuance_sha256":file_hash(root,&c.issuance)?,"generator_ref":GENERATOR,"generator_sha256":GENERATOR_SHA,"artifacts":artifacts,"private_artifacts":[{"ref":c.private_db,"sha256":hash(db),"mode":"0600","tracked":false},{"ref":c.private_request,"sha256":hash(private),"mode":"0600","tracked":false}],"accepted_candidate_count":0,"human_review_count":0,"graph_effect":false,"canon_effect":false});
    for (key, p) in [
        ("plan", format!("{ROUTE}/plan.v1.json")),
        (
            "request_schema",
            format!("{ROUTE}/concept-request.v2.schema.json"),
        ),
        (
            "relation_schema",
            format!("{ROUTE}/concept-relation-candidate.v1.schema.json"),
        ),
        (
            "english_task_schema",
            format!("{ROUTE}/english-on-demand-task.v1.schema.json"),
        ),
        (
            "english_candidate_schema",
            format!("{ROUTE}/english-translation-candidate.v1.schema.json"),
        ),
        (
            "concept_search_result_schema",
            format!("{ROUTE}/concept-search-result.v1.schema.json"),
        ),
        ("concept_search_query", QUERY.into()),
        (
            "word_analysis_task_schema",
            format!("{ROUTE}/word-analysis-task.v1.schema.json"),
        ),
        ("word_analysis_prepare", WORD.into()),
    ] {
        manifest[format!("{key}_ref")] = json!(p);
        manifest[format!("{key}_sha256")] = json!(match key {
            "concept_search_query" => QUERY_SHA.to_string(),
            "word_analysis_prepare" => WORD_SHA.to_string(),
            _ => file_hash(root, &p)?,
        });
    }
    outputs.insert(c.output("manifest").into(), encode(&manifest, true));
    Ok(())
}
