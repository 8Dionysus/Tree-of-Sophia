use super::*;
pub(super) fn validate(c: &mut Context<'_>) -> io::Result<()> {
    let path = MAP_PATH;
    let Some(p) = c.load_map(path, SCHEMA_PATH)? else {
        return Ok(());
    };
    let parts = index(&p["source_parts"], "part_label");
    let routes = index(&p["part_routes"], "part_label");
    let roman = strs(&["I", "II", "III", "IV"]);
    c.require(
        keys(&parts) == roman,
        path,
        "source_parts must cover I, II, III, and IV exactly",
    )?;
    c.require(
        keys(&routes) == roman,
        path,
        "part_routes must cover I, II, III, and IV exactly",
    )?;
    let epub = &p["target_witnesses"]["epub"];
    let pdf = &p["target_witnesses"]["pdf"];
    let mut witnesses: Vec<&Value> = parts.values().copied().collect();
    witnesses.extend([epub, pdf].into_iter().filter(|v| v.is_object()));
    let mut inventories = BTreeMap::<String, Value>::new();
    for w in &witnesses {
        if let Some(f) = c.inventory(w, false, &format!("{path}:{}", text(&w["item_ref"])))? {
            if let Some(id) = w["item_ref"].as_str() {
                inventories.insert(id.into(), f);
            }
        }
    }
    let resource_sets: BTreeMap<&str, Index<'_>> = inventories
        .iter()
        .map(|(id, v)| (id.as_str(), index(&v["resources"], "resource_id")))
        .collect();
    let empty = Index::new();
    let epub_res = resource_sets.get(text(&epub["item_ref"])).unwrap_or(&empty);
    let pdf_res = resource_sets.get(text(&pdf["item_ref"])).unwrap_or(&empty);
    let mut epages: Vec<_> = epub_res
        .values()
        .filter_map(|r| page_member(&r["locator"]["member_path"]))
        .collect();
    epages.sort();
    c.require(
        epages == (0..529).collect::<Vec<_>>(),
        path,
        "target EPUB page-member enumeration is not 0..528",
    )?;
    let mut ppages: Vec<_> = pdf_res
        .values()
        .filter(|r| r["resource_kind"] == "pdf_page")
        .filter_map(|r| r["locator"]["page_index"].as_i64())
        .collect();
    ppages.sort();
    c.require(
        ppages == (1..530).collect::<Vec<_>>(),
        path,
        "target PDF page enumeration is not 1..529",
    )?;
    let mut ids = BTreeSet::new();
    let mut source_keys = BTreeSet::new();
    let mut sequences = BTreeMap::<String, Vec<i64>>::new();
    let mut pages = BTreeMap::<String, Vec<i64>>::new();
    let mut modes = BTreeMap::<String, usize>::new();
    let correspondences: Vec<_> = objects(&p["correspondences"]).collect();
    for (i, v) in correspondences.iter().enumerate() {
        let loc = format!("{path}:correspondences[{}]", i + 1);
        let id = v["correspondence_id"].as_str();
        c.require(
            id.is_some_and(|s| ids.insert(s.to_owned())),
            &loc,
            "correspondence_id is invalid or duplicated",
        )?;
        let part = text(&v["part_label"]);
        if let Some(n) = v["sequence"].as_i64() {
            sequences.entry(part.into()).or_default().push(n);
        }
        let source = &v["source"];
        let witness = get(&parts, &v["part_label"]);
        for f in ["item_ref", "file_ref", "inventory_ref"] {
            c.require(
                source[f] == witness[f],
                &loc,
                format!("source {f} differs from part witness"),
            )?;
        }
        c.require(
            source_keys.insert(tuple(&[
                source["item_ref"].clone(),
                source["resource_id"].clone(),
            ])),
            &loc,
            "source division resource is duplicated",
        )?;
        let source_res = resource_sets
            .get(text(&source["item_ref"]))
            .unwrap_or(&empty);
        let r = get(source_res, &source["resource_id"]);
        if r.is_null() {
            c.issue(&loc, "source division resource is unresolved")?;
        } else {
            for f in ["tei_path", "tei_depth", "tei_page_label"] {
                c.require(
                    source[f] == r["locator"][f],
                    &loc,
                    format!("source {f} differs from inventory"),
                )?;
            }
            c.require(
                source["label_fingerprint"] == r["label_fingerprint"],
                &loc,
                "source label_fingerprint differs from inventory",
            )?;
        }
        let te = &v["target_epub"];
        for f in ["item_ref", "file_ref", "inventory_ref"] {
            c.require(
                te[f] == epub[f],
                &loc,
                format!("target_epub {f} differs from witness"),
            )?;
        }
        let r = get(epub_res, &te["resource_id"]);
        if r.is_null() {
            c.issue(&loc, "target EPUB resource is unresolved")?;
        } else {
            let expected = json!({"member_path":r["locator"]["member_path"],"member_sha256":r["sha256"],"spine_index":r["locator"]["spine_index"],"content_fingerprint":r["content_fingerprint"]});
            c.fields(te, &expected, &loc, |f| {
                format!("target_epub {f} differs from inventory")
            })?;
        }
        let page = page_member(&te["member_path"]);
        if let Some(n) = page {
            c.require(
                te["scan_page_number"] == n,
                &loc,
                "target EPUB scan page differs from member path",
            )?;
            pages.entry(part.into()).or_default().push(n);
        } else {
            c.issue(&loc, "target EPUB member path is not a scan page")?;
        }
        let tp = &v["target_pdf"];
        for f in ["item_ref", "file_ref", "inventory_ref"] {
            c.require(
                tp[f] == pdf[f],
                &loc,
                format!("target_pdf {f} differs from witness"),
            )?;
        }
        let r = get(pdf_res, &tp["resource_id"]);
        if r.is_null() {
            c.issue(&loc, "target PDF resource is unresolved")?;
        } else {
            c.require(
                tp["page_index"] == r["locator"]["page_index"],
                &loc,
                "target PDF page differs from inventory",
            )?;
        }
        if let Some(n) = page {
            c.require(
                tp["page_index"] == n + 1,
                &loc,
                "EPUB-to-PDF page formula drifted",
            )?;
        }
        let evidence = &v["match"];
        if let Some(mode) = evidence["mode"].as_str() {
            *modes.entry(mode.into()).or_default() += 1;
        }
        c.require(
            evidence["search_page_range"]
                == get(&routes, &v["part_label"])["target_epub_member_page_range"],
            &loc,
            "match search range differs from part route",
        )?;
        if let Some(n) = page {
            c.require(
                arr(&evidence["target_window_pages"])
                    .first()
                    .is_some_and(|v| *v == n),
                &loc,
                "target window does not start at selected page",
            )?;
        }
    }
    let mut part_counts = BTreeMap::new();
    for part in ["I", "II", "III", "IV"] {
        let seq = sequences.get(part).cloned().unwrap_or_default();
        part_counts.insert(part, seq.len());
        c.require(
            seq == (1..=seq.len() as i64).collect::<Vec<_>>(),
            path,
            format!("part {part} sequence is not contiguous"),
        )?;
        c.require(
            monotonic(pages.get(part).map(Vec::as_slice).unwrap_or(&[]), false),
            path,
            format!("part {part} target pages are not monotonic"),
        )?;
    }
    c.fields(&p["summary"],&json!({"correspondence_count":correspondences.len(),"part_counts":part_counts,"match_mode_counts":modes}),path,|f|format!("summary {f} drifted"))?;
    let anchor_set = c
        .load_map(ANCHOR_SET_PATH, ANCHOR_SET_SCHEMA_PATH)?
        .unwrap_or(Value::Null);
    let anchors = c.anchors(ANCHOR_RECORDS_PATH)?;
    let by_id: Index<'_> = anchors
        .iter()
        .filter_map(|a| a["anchor_id"].as_str().map(|id| (id, a)))
        .collect();
    if !anchor_set.is_null() {
        let digest = c.digest(path)?;
        c.require(
            anchor_set["correspondence_map"] == json!({"ref":path,"sha256":digest}),
            ANCHOR_SET_PATH,
            "correspondence_map ref or digest drifted",
        )?;
        if c.source.is_file(ANCHOR_RECORDS_PATH)? {
            let digest = c.digest(ANCHOR_RECORDS_PATH)?;
            c.require(
                anchor_set["anchor_records"] == json!({"ref":ANCHOR_RECORDS_PATH,"sha256":digest}),
                ANCHOR_SET_PATH,
                "anchor_records ref or digest drifted",
            )?;
        }
        for field in ["work_ref", "provenance_ref"] {
            c.require(
                anchor_set[field] == p[field],
                ANCHOR_SET_PATH,
                format!("{field} differs from map"),
            )?;
        }
    }
    for (i, a) in anchors.iter().enumerate() {
        let loc = format!("{ANCHOR_RECORDS_PATH}:{}", i + 1);
        c.require(
            a["status"] == "proposed",
            &loc,
            "structural anchor must remain proposed",
        )?;
        c.require(
            a["provenance_event_ref"] == ANCHOR_EVENT_ID,
            &loc,
            "anchor provenance event drifted",
        )?;
        c.require(
            !objects(&a["selectors"])
                .any(|s| matches!(s["type"].as_str(), Some("text_quote" | "text_position"))),
            &loc,
            "text-bearing selectors are forbidden in the structural anchor set",
        )?;
    }
    let by_correspondence = index(&p["correspondences"], "correspondence_id");
    let mut binding_ids = BTreeSet::new();
    let mut expected_order = vec![];
    for (i, b) in objects(&anchor_set["bindings"]).enumerate() {
        let loc = format!("{ANCHOR_SET_PATH}:bindings[{}]", i + 1);
        let id = text(&b["correspondence_id"]);
        let v = get(&by_correspondence, &b["correspondence_id"]);
        if v.is_null() || !binding_ids.insert(id.to_owned()) {
            c.issue(&loc, "correspondence binding is unresolved or duplicated")?;
            continue;
        }
        for f in ["part_label", "sequence"] {
            c.require(b[f] == v[f], &loc, format!("{f} differs from map"))?;
        }
        for (role, id_role, locator, digest, selectors, method) in [
            (
                "source_tei",
                "dta",
                &v["source"],
                &get(&parts, &v["part_label"])["file_sha256"],
                json!([{"type":"structural","path":[v["source"]["tei_path"]],"scheme":"tei-xpath-like-inventory-v1"}]),
                "resource-inventory TEI division locator",
            ),
            (
                "target_epub",
                "naumann-1893-epub",
                &v["target_epub"],
                &epub["file_sha256"],
                json!([{"type":"container_member","member_path":v["target_epub"]["member_path"],"member_sha256":v["target_epub"]["member_sha256"]}]),
                "resource-inventory exact EPUB member locator",
            ),
            (
                "target_pdf",
                "naumann-1893-pdf",
                &v["target_pdf"],
                &pdf["file_sha256"],
                json!([whole_page(&v["target_pdf"]["page_index"])]),
                "resource-inventory whole-page PDF locator",
            ),
        ] {
            let aid = format!(
                "tos.anchor.zarathustra-structure.{id_role}-{}",
                id.strip_prefix("structure-").unwrap_or(id)
            );
            expected_order.push(aid.clone());
            c.require(b[role]==json!({"anchor_ref":aid,"item_ref":locator["item_ref"],"file_ref":locator["file_ref"],"resource_id":locator["resource_id"]}),&loc,format!("{role} binding differs from correspondence"))?;
            let a = get(&by_id, &json!(aid));
            if a.is_null() {
                c.issue(&loc, format!("{role} anchor is unresolved: {aid}"))?;
                continue;
            }
            let expected = json!({"item_id":locator["item_ref"],"file_id":locator["file_ref"],"file_sha256":digest,"selectors":selectors,"selector_method":{"maker_type":"software","method":method,"version":"1","configuration_ref":format!("{path}#{id}")},"status":"proposed","provenance_event_ref":ANCHOR_EVENT_ID,"anchor_version":1,"supersedes_anchor_ref":null,"review_ref":null,"passage_id":null});
            c.fields(a, &expected, &loc, |f| {
                format!("{role} anchor {f} differs from source map")
            })?;
        }
    }
    c.require(
        binding_ids == keys(&by_correspondence),
        ANCHOR_SET_PATH,
        "anchor bindings do not cover map correspondences exactly",
    )?;
    c.require(
        values(
            anchors
                .iter()
                .filter(|a| a["anchor_id"].is_string())
                .map(|a| &a["anchor_id"]),
        ) == json!(expected_order),
        ANCHOR_RECORDS_PATH,
        "anchor record order differs from correspondence role order",
    )?;
    c.require(
        keys(&by_id) == expected_order.iter().cloned().collect(),
        ANCHOR_RECORDS_PATH,
        "anchor records do not close over the expected stable ID set",
    )?;
    if !anchor_set.is_null() {
        let n = by_correspondence.len();
        c.require(anchor_set["summary"]==json!({"correspondence_count":n,"anchor_count":n*3,"anchors_per_correspondence":3,"role_counts":{"source_tei":n,"target_epub":n,"target_pdf":n},"all_anchor_statuses":["proposed"]}),ANCHOR_SET_PATH,"anchor summary drifted")?;
    }
    let Some(prov) = p["provenance_ref"].as_str() else {
        c.issue(path, "provenance_ref is invalid")?;
        return Ok(());
    };
    let events = c.events(prov)?;
    if let Some(event) = c.event(
        &events,
        &p["provenance_event_ref"],
        None,
        prov,
        "provenance_event_ref does not resolve exactly once",
    )? {
        let digest = c.digest(path)?;
        let output = json!({"ref":path,"role":"tracked_text_free_structure_correspondence_candidate","sha256":digest});
        c.require(
            arr(&event["outputs"]).contains(&output),
            prov,
            "event lacks digest-bound map output",
        )?;
        let inputs = witnesses
            .iter()
            .map(|w| tuple(&[w["file_ref"].clone(), w["file_sha256"].clone()]))
            .collect();
        c.require(
            tuple_set(&event["inputs"], &["ref", "sha256"]) == inputs,
            prov,
            "event input file/digest set drifted",
        )?;
    }
    if let Some(event) = c.event(
        &events,
        &anchor_set["provenance_event_ref"],
        None,
        prov,
        "anchor provenance_event_ref does not resolve exactly once",
    )? {
        if c.source.is_file(ANCHOR_SET_PATH)? && c.source.is_file(ANCHOR_RECORDS_PATH)? {
            let digest = c.digest(path)?;
            c.require(
                tuple_set(&event["inputs"], &["ref", "sha256"])
                    == BTreeSet::from([tuple(&[json!(path), json!(digest)])]),
                prov,
                "anchor event input map/digest set drifted",
            )?;
            let expected = BTreeSet::from([
                c.bound_output(ANCHOR_SET_PATH, "tracked_proposed_structure_anchor_set")?,
                c.bound_output(
                    ANCHOR_RECORDS_PATH,
                    "tracked_proposed_source_anchor_records",
                )?,
            ]);
            c.require(
                tuple_set(&event["outputs"], &["ref", "role", "sha256"]) == expected,
                prov,
                "anchor event output ref/role/digest set drifted",
            )?;
        }
    }
    Ok(())
}
