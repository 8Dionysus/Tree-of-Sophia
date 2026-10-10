use super::*;

pub(super) fn unit_keys(target: bool) -> Vec<String> {
    let mut keys = vec![];
    for n in 1..=296 {
        keys.push(n.to_string());
        if [65, 73].contains(&n) || (!target && n == 237) {
            keys.push(format!("{n}a"));
        }
    }
    keys
}
pub(super) fn validate(c: &mut Context<'_>, target: bool) -> io::Result<()> {
    let (path, anchor_path, schema, event_id, expression) = if target {
        (
            TARGET_NUMBERED_UNIT_MAP_PATH,
            TARGET_NUMBERED_UNIT_ANCHOR_RECORDS_PATH,
            TARGET_NUMBERED_UNIT_SCHEMA_PATH,
            TARGET_NUMBERED_UNIT_EVENT_ID,
            "ru-polilov-mysl-1996",
        )
    } else {
        (
            NUMBERED_UNIT_MAP_PATH,
            NUMBERED_UNIT_ANCHOR_RECORDS_PATH,
            NUMBERED_UNIT_SCHEMA_PATH,
            NUMBERED_UNIT_EVENT_ID,
            "de-naumann-1886",
        )
    };
    let Some(p) = c.load_map(path, schema)? else {
        return Ok(());
    };
    let prefix = if target { "target " } else { "" };
    let inventory_binding = &p["inventory"];
    let Some(inventory_ref) = inventory_binding["ref"].as_str() else {
        c.issue(
            path,
            format!("{prefix}resource inventory binding is invalid"),
        )?;
        return Ok(());
    };
    let Some(inventory) = c.json(inventory_ref)? else {
        return Ok(());
    };
    let digest = c.digest(inventory_ref)?;
    c.require(
        inventory_binding["sha256"] == digest,
        path,
        format!("{prefix}resource inventory digest drifted"),
    )?;
    let inv_files = index(&inventory["files"], "profile");
    let pdf = if target {
        let files: Vec<_> = objects(&inventory["files"]).collect();
        if files.len() != 1 {
            c.issue(path, "target resource inventory file set drifted")?;
            return Ok(());
        }
        c.require(
            files[0]["profile"] == "pdf_pages_v1" && files[0]["summary"]["page_count"] == 831,
            path,
            "target PDF inventory profile or count drifted",
        )?;
        files[0]
    } else {
        c.require(
            keys(&inv_files) == strs(&["pdf_pages_v1", "djvu_xml_pages_v1", "abbyy_xml_pages_v1"]),
            path,
            "resource inventory profile set drifted",
        )?;
        for profile in ["pdf_pages_v1", "djvu_xml_pages_v1", "abbyy_xml_pages_v1"] {
            c.require(
                get(&inv_files, &json!(profile))["summary"]["page_count"] == 274,
                path,
                format!("{profile} page count drifted"),
            )?;
        }
        get(&inv_files, &json!("pdf_pages_v1"))
    };
    let scan = &p["scan_file"];
    c.require(
        scan["file_ref"] == pdf["file_id"]
            && scan["file_sha256"] == pdf["file_sha256"]
            && (!target
                || (scan["inventory_profile"] == "pdf_pages_v1"
                    && inventory["item_id"] == p["item_ref"])),
        path,
        format!("{prefix}scan file differs from inventory"),
    )?;
    if !target {
        let nav = index(&p["navigation_files"], "inventory_profile");
        c.require(
            keys(&nav) == strs(&["djvu_xml_pages_v1", "abbyy_xml_pages_v1"]),
            path,
            "navigation profile set drifted",
        )?;
        for (profile, role) in [
            ("djvu_xml_pages_v1", "djvu_xml"),
            ("abbyy_xml_pages_v1", "abbyy_xml_gzip"),
        ] {
            let b = get(&nav, &json!(profile));
            let f = get(&inv_files, &json!(profile));
            c.require(
                b["role"] == role
                    && b["file_ref"] == f["file_id"]
                    && b["file_sha256"] == f["file_sha256"],
                path,
                format!("{profile} binding differs from inventory"),
            )?;
        }
    }
    let mut member = Value::Null;
    if target {
        let b = &p["work_boundary"];
        let Some(reference) = b["ref"].as_str() else {
            c.issue(path, "target work-boundary binding is invalid")?;
            return Ok(());
        };
        let Some(boundary) = c.json(reference)? else {
            return Ok(());
        };
        let digest = c.digest(reference)?;
        c.require(
            b["sha256"] == digest,
            path,
            "target work-boundary digest drifted",
        )?;
        let matches: Vec<_> = objects(&boundary["members"])
            .filter(|m| {
                m["sequence"] == b["member_sequence"]
                    && m["work_ref"] == p["work_ref"]
                    && m["expression_ref"] == p["expression_ref"]
            })
            .collect();
        if matches.len() != 1 {
            c.issue(path, "target work boundary does not resolve exactly once")?;
        } else {
            member = matches[0].clone();
            for field in [
                "start_page",
                "end_page",
                "epistemic_status",
                "review_status",
            ] {
                c.require(
                    b[field] == member[field],
                    path,
                    format!("target work-boundary {field} drifted"),
                )?;
            }
        }
    }
    let resources = index(&pdf["resources"], "resource_id");
    let units: Vec<_> = objects(&p["unit_starts"]).collect();
    let count = if target { 298 } else { 299 };
    c.require(
        values(units.iter().map(|u| &u["unit_key"])) == json!(unit_keys(target)),
        path,
        format!("{prefix}numbered-unit sequence drifted"),
    )?;
    c.require(
        values(units.iter().map(|u| &u["sequence"])) == json!((1..=count).collect::<Vec<_>>()),
        path,
        format!("{prefix}numbered-unit ordinal sequence drifted"),
    )?;
    let pages = numbers(units.iter().map(|u| u["pdf_page"].clone()));
    c.require(
        monotonic(&pages, false),
        path,
        format!("{prefix}numbered-unit pages are not monotonic"),
    )?;
    if target {
        c.require(
            !units.iter().any(|u| u["unit_key"] == "237a"),
            path,
            "source-only 237a was materialized in target",
        )?;
        if !member.is_null() {
            c.require(
                pages.iter().all(|v| {
                    member["start_page"].as_i64().is_some_and(|x| x <= *v)
                        && member["end_page"].as_i64().is_some_and(|x| *v <= x)
                }),
                path,
                "target numbered-unit page leaves work boundary",
            )?;
        }
    }
    let anchors = c.anchors(anchor_path)?;
    let by_id: Index<'_> = anchors
        .iter()
        .filter_map(|a| a["anchor_id"].as_str().map(|id| (id, a)))
        .collect();
    let mut expected_ids = BTreeSet::new();
    let mut counts = BTreeMap::<String, usize>::new();
    let mut basis_keys = BTreeMap::<String, BTreeSet<String>>::new();
    for u in &units {
        c.tick()?;
        let key = text(&u["unit_key"]);
        let resource = get(&resources, &u["resource_id"]);
        if let Some(s) = u["anchor_ref"].as_str() {
            expected_ids.insert(s.to_owned());
        }
        if let Some(b) = u["basis"].as_str() {
            *counts.entry(b.into()).or_default() += 1;
            basis_keys.entry(b.into()).or_default().insert(key.into());
        }
        c.require(
            !resource.is_null()
                && resource["resource_kind"] == "pdf_page"
                && resource["locator"]["page_index"] == u["pdf_page"],
            path,
            format!("{prefix}numbered unit {key} leaves inventory"),
        )?;
        let a = get(&by_id, &u["anchor_ref"]);
        if a.is_null() {
            c.issue(path, format!("{prefix}numbered unit {key} has no anchor"))?;
            continue;
        }
        c.require(a["item_id"]==p["item_ref"]&&a["file_id"]==scan["file_ref"]&&a["file_sha256"]==scan["file_sha256"]&&a["passage_id"]==format!("tos.passage.friedrich-nietzsche.jenseits-von-gut-und-boese.{expression}.unit-{key}")&&a["status"]=="proposed"&&a["provenance_event_ref"]==p["provenance_event_ref"],path,format!("{prefix}numbered unit {key} anchor binding drifted"))?;
        let selectors = arr(&a["selectors"]);
        if values(
            selectors
                .iter()
                .filter(|s| s.is_object())
                .map(|s| &s["type"]),
        ) != json!(["structural", "page_region"])
        {
            c.issue(
                path,
                format!("{prefix}numbered unit {key} selector bundle drifted"),
            )?;
            continue;
        }
        let expected = json!({"type":"structural","path":["work:jenseits-von-gut-und-boese",format!("expression:{expression}"),format!("numbered-unit:{key}")],"scheme":if target{"jgb-target-numbered-unit-start-v1"}else{"jgb-numbered-unit-start-v1"}});
        c.require(
            selectors.first() == Some(&expected),
            path,
            format!("{prefix}numbered unit {key} structural selector drifted"),
        )?;
        c.require(
            selectors.get(1) == Some(&whole_page(&u["pdf_page"])),
            path,
            format!("{prefix}numbered unit {key} page selector drifted"),
        )?;
    }
    c.require(
        keys(&by_id) == expected_ids && anchors.len() == count as usize,
        path,
        format!("{prefix}numbered-unit anchor set differs from map"),
    )?;
    let expected_counts = if target {
        json!({"embedded_pdf_bbox_order_candidate":264,"source_visible_gap_review":33,"source_visible_ocr_disambiguation":1})
    } else {
        json!({"ocr_order_candidate":266,"source_visible_gap_review":22,"source_visible_ocr_disambiguation":10,"source_visible_repeated_number_review":1})
    };
    c.require(
        json!(counts) == expected_counts,
        path,
        format!("{prefix}numbered-unit basis counts drifted"),
    )?;
    let review = &p["method"]["source_visible_review"];
    if target {
        for (basis, field, message) in [
            (
                "source_visible_gap_review",
                "gap_review_unit_keys",
                "target declared gap-review set drifted",
            ),
            (
                "source_visible_ocr_disambiguation",
                "ocr_disambiguation_unit_keys",
                "target declared OCR-disambiguation set drifted",
            ),
        ] {
            c.require(
                string_set(&review[field]) == basis_keys.get(basis).cloned().unwrap_or_default(),
                path,
                message,
            )?;
        }
        c.require(
            review["supplemental_label_unit_keys"] == json!(["65a", "73a"]),
            path,
            "target supplemental-label review drifted",
        )?;
    } else {
        let mut declared = BTreeSet::new();
        let mut by_basis = BTreeMap::new();
        for (basis, field) in [
            ("source_visible_gap_review", "gap_review_unit_keys"),
            (
                "source_visible_ocr_disambiguation",
                "ocr_disambiguation_unit_keys",
            ),
            (
                "source_visible_repeated_number_review",
                "repeated_number_unit_keys",
            ),
        ] {
            let set = string_set(&review[field]);
            c.require(
                declared.is_disjoint(&set),
                path,
                format!("declared source-visible {basis} set overlaps"),
            )?;
            declared.extend(set.iter().cloned());
            by_basis.insert(basis, set);
        }
        let materialized: BTreeSet<_> = units
            .iter()
            .filter(|u| u["basis"] != "ocr_order_candidate")
            .map(|u| text(&u["unit_key"]).to_owned())
            .collect();
        c.require(
            declared == materialized,
            path,
            "declared source-visible review set drifted",
        )?;
        for u in &units {
            let key = text(&u["unit_key"]);
            let expected = by_basis
                .iter()
                .find(|(_, set)| set.contains(key))
                .map(|(b, _)| *b)
                .unwrap_or("ocr_order_candidate");
            c.require(
                u["basis"] == expected,
                path,
                format!("numbered unit {key} review basis drifted"),
            )?;
        }
    }
    let mut source_map_ref = None;
    let mut asymmetry = Value::Null;
    if target {
        let a = arr(&p["numbering_asymmetries"]);
        if a.len() != 1 || !a[0].is_object() {
            c.issue(path, "target numbering asymmetry set drifted")?;
        } else {
            asymmetry = a[0].clone();
        }
        if let Some(reference) = asymmetry["source_map_ref"].as_str() {
            source_map_ref = Some(reference.to_owned());
            if let Some(source) = c.json(reference)? {
                let digest = c.digest(reference)?;
                c.require(
                    asymmetry["source_map_sha256"] == digest,
                    path,
                    "target source-map asymmetry digest drifted",
                )?;
                c.require(
                    objects(&source["unit_starts"]).any(|u| u["unit_key"] == "237a"),
                    path,
                    "bound source map does not contain 237a",
                )?;
            }
        } else {
            c.issue(path, "target source-map asymmetry ref is invalid")?;
        }
        c.require(
            asymmetry["source_unit_key"] == "237a"
                && asymmetry["target_numbered_unit_materialized"] == false
                && asymmetry["exact_translation_alignment_claimed"] == false,
            path,
            "target 237a asymmetry posture drifted",
        )?;
    }
    let expected_summary = if target {
        json!({"integer_numbered_unit_count":296,"supplemental_numbered_units":["65a","73a"],"source_only_nonmaterialized_numbered_units":["237a"],"numbered_unit_count":units.len(),"exact_start_page_candidates_materialized":units.len(),"unresolved_unit_count":0,"start_pages_monotonic":monotonic(&pages,false),"all_anchor_statuses":["proposed"],"human_review_performed":false})
    } else {
        json!({"numbered_unit_count":units.len(),"exact_start_page_candidates_materialized":units.len()})
    };
    c.fields(&p["summary"], &expected_summary, path, |f| {
        format!("{prefix}summary {f} drifted")
    })?;
    let Some(prov) = p["provenance_ref"].as_str() else {
        c.issue(path, format!("{prefix}provenance_ref is invalid"))?;
        return Ok(());
    };
    let events = c.events(prov)?;
    if let Some(event) = c.event(
        &events,
        &p["provenance_event_ref"],
        Some(event_id),
        prov,
        &format!("{prefix}numbered-unit provenance event does not resolve"),
    )? {
        let expected_outputs = BTreeSet::from([
            c.bound_output(
                path,
                if target {
                    "tracked-text-free-target-numbered-unit-page-map"
                } else {
                    "tracked-text-free-numbered-unit-page-map"
                },
            )?,
            c.bound_output(
                anchor_path,
                if target {
                    "tracked-proposed-whole-page-target-anchors"
                } else {
                    "tracked-proposed-whole-page-source-anchors"
                },
            )?,
        ]);
        c.require(
            tuple_set(&event["outputs"], &["ref", "role", "sha256"]) == expected_outputs,
            prov,
            format!("{prefix}event outputs drifted"),
        )?;
        let mut inputs = BTreeSet::from([
            tuple(&[scan["file_ref"].clone(), scan["file_sha256"].clone()]),
            tuple(&[json!(inventory_ref), inventory_binding["sha256"].clone()]),
        ]);
        if target {
            inputs.insert(tuple(&[
                p["work_boundary"]["ref"].clone(),
                p["work_boundary"]["sha256"].clone(),
            ]));
            inputs.insert(tuple(&[
                json!(source_map_ref),
                asymmetry["source_map_sha256"].clone(),
            ]));
        } else {
            for n in objects(&p["navigation_files"]) {
                inputs.insert(tuple(&[n["file_ref"].clone(), n["file_sha256"].clone()]));
            }
        }
        c.require(
            tuple_set(&event["inputs"], &["ref", "sha256"]) == inputs,
            prov,
            format!("{prefix}event inputs drifted"),
        )?;
    }
    Ok(())
}
