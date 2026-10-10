use super::*;
pub(super) fn validate(c: &mut Context<'_>) -> io::Result<()> {
    let path = PARALLEL_MAP_PATH;
    let Some(p) = c.load_map(path, PARALLEL_SCHEMA_PATH)? else {
        return Ok(());
    };
    let source = &p["source_witness"];
    let target = &p["target_witness"];
    let mut files = BTreeMap::new();
    for (role, w) in [("source", source), ("target", target)] {
        if !w.is_object() {
            c.issue(path, format!("{role}_witness is invalid"))?;
            continue;
        }
        if let Some(f) = c.inventory(w, true, &format!("{path}:{role}_witness"))? {
            if let Some(id) = w["item_ref"].as_str() {
                files.insert(id.to_owned(), f);
            }
        }
    }
    c.require(
        source["language"] != target["language"],
        path,
        "parallel witnesses must have distinct languages",
    )?;
    let binding = &p["method"]["source_numbered_unit_map"];
    if !binding.is_object() {
        c.issue(path, "source numbered-unit map binding is invalid")?;
    } else if binding["ref"] != NUMBERED_UNIT_MAP_PATH {
        c.issue(path, "source numbered-unit map ref drifted")?;
    } else if let Some(units) = c.json(NUMBERED_UNIT_MAP_PATH)? {
        let digest = c.digest(NUMBERED_UNIT_MAP_PATH)?;
        c.require(
            binding["sha256"] == digest,
            path,
            "source numbered-unit map digest drifted",
        )?;
        for (f, expected) in [
            ("work_ref", &p["work_ref"]),
            ("expression_ref", &source["expression_ref"]),
            ("item_ref", &source["item_ref"]),
        ] {
            c.require(
                units[f] == *expected,
                path,
                format!("source numbered-unit map {f} differs from witness"),
            )?;
        }
    }
    let postures = &p["method"]["supplemental_unit_postures"];
    let by_key = index(postures, "unit_key");
    let target_states = [
        ("65a", "not_individually_reviewed"),
        ("73a", "not_individually_reviewed"),
        (
            "237a",
            "corresponding_prose_present_without_repeated_unit_label",
        ),
    ];
    if by_key.len() != arr(postures).len() || keys(&by_key) != strs(&["65a", "73a", "237a"]) {
        c.issue(path, "supplemental-unit posture set drifted")?;
    } else {
        for (key, state) in target_states {
            c.require(
                get(&by_key, &json!(key))["target_state"] == state,
                path,
                format!("supplemental unit {key} target posture drifted"),
            )?;
        }
    }
    let anchors = c.anchors(PARALLEL_ANCHOR_RECORDS_PATH)?;
    let by_id: Index<'_> = anchors
        .iter()
        .filter_map(|a| a["anchor_id"].as_str().map(|id| (id, a)))
        .collect();
    let resource_sets: BTreeMap<&str, Index<'_>> = files
        .iter()
        .map(|(id, f)| (id.as_str(), index(&f["resources"], "resource_id")))
        .collect();
    let empty = Index::new();
    let divisions: Vec<_> = objects(&p["divisions"]).collect();
    let mut source_pages = vec![];
    let mut target_pages = vec![];
    let mut expected_ids = BTreeSet::new();
    let mut spans = vec![];
    let mut supplemental = vec![];
    for (i, d) in divisions.iter().enumerate() {
        let loc = format!("{path}:divisions[{}]", i + 1);
        let span = &d["numbered_unit_span"];
        if d["division_kind"] == "numbered_main_division" {
            if !span.is_object() {
                c.issue(&loc, "numbered division lacks a span")?;
            } else if let (Some(first), Some(last)) =
                (span["first"].as_i64(), span["last"].as_i64())
            {
                if !(1 <= first && first <= last && last <= 296) {
                    c.issue(&loc, "numbered span is reversed or outside 1..296")?;
                }
                spans.push((first, last));
            }
            c.require(
                arr(&d["correspondence_basis"]).contains(&json!("numbered_unit_boundary")),
                &loc,
                "numbered division lacks boundary basis",
            )?;
        } else {
            c.require(span.is_null(), &loc, "non-numbered division carries a span")?;
        }
        supplemental.extend(arr(&d["supplemental_numbered_units"]).iter().cloned());
        for (role, w, pages) in [
            ("source", source, &mut source_pages),
            ("target", target, &mut target_pages),
        ] {
            let l = &d[role];
            if !l.is_object() || !w.is_object() {
                c.issue(&loc, format!("{role} locator is invalid"))?;
                continue;
            }
            for field in ["item_ref", "file_ref"] {
                c.require(
                    l[field] == w[field],
                    &loc,
                    format!("{role} {field} differs from witness"),
                )?;
            }
            c.require(
                l["inventory_ref"] == w["inventory"]["ref"],
                &loc,
                format!("{role} inventory_ref differs from witness"),
            )?;
            let resources = resource_sets.get(text(&l["item_ref"])).unwrap_or(&empty);
            let r = get(resources, &l["resource_id"]);
            if r.is_null() {
                c.issue(&loc, format!("{role} resource is unresolved"))?;
            } else {
                c.require(
                    r["resource_kind"] == "pdf_page" && r["locator"]["page_index"] == l["pdf_page"],
                    &loc,
                    format!("{role} page differs from inventory"),
                )?;
            }
            if let Some(n) = l["pdf_page"].as_i64() {
                pages.push(n);
            }
            if let Some(id) = l["anchor_ref"].as_str() {
                expected_ids.insert(id.to_owned());
            }
            let a = get(&by_id, &l["anchor_ref"]);
            if a.is_null() {
                c.issue(&loc, format!("{role} anchor is unresolved"))?;
                continue;
            }
            c.require(
                a["item_id"] == w["item_ref"]
                    && a["file_id"] == w["file_ref"]
                    && a["file_sha256"] == w["file_sha256"],
                &loc,
                format!("{role} anchor crosses its witness"),
            )?;
            c.require(
                a["status"] == "proposed",
                &loc,
                format!("{role} anchor is not proposed"),
            )?;
            c.require(
                a["provenance_event_ref"] == p["provenance_event_ref"],
                &loc,
                format!("{role} anchor provenance differs from map"),
            )?;
            let selectors = arr(&a["selectors"]);
            if selectors.len() != 1 || !selectors[0].is_object() {
                c.issue(&loc, format!("{role} anchor must have one selector"))?;
            } else {
                c.require(
                    selectors[0] == whole_page(&l["pdf_page"]),
                    &loc,
                    format!("{role} anchor is not the declared whole page"),
                )?;
            }
        }
    }
    c.require(
        values(divisions.iter().map(|v| &v["sequence"]))
            == json!((1..=divisions.len()).collect::<Vec<_>>()),
        path,
        "division sequence is not contiguous",
    )?;
    c.require(
        monotonic(&source_pages, true),
        path,
        "source division pages are not strictly monotonic",
    )?;
    c.require(
        monotonic(&target_pages, true),
        path,
        "target division pages are not strictly monotonic",
    )?;
    c.require(
        keys(&by_id) == expected_ids,
        path,
        "anchor records differ from division bindings",
    )?;
    let flattened: Vec<_> = spans
        .iter()
        .filter(|(a, b)| 1 <= *a && *a <= *b && *b <= 296)
        .flat_map(|(a, b)| *a..=*b)
        .collect();
    c.require(
        flattened == (1..=296).collect::<Vec<_>>(),
        path,
        "numbered division spans do not cover integers 1 through 296 once",
    )?;
    if let Some(reference) = target["work_boundary"]["ref"].as_str() {
        if let Some(boundary) = c.json(reference)? {
            let members: Vec<_> = objects(&boundary["members"])
                .filter(|m| {
                    m["work_ref"] == p["work_ref"]
                        && m["expression_ref"] == target["expression_ref"]
                })
                .collect();
            if members.len() != 1 {
                c.issue(path, "target work boundary does not resolve exactly once")?;
            } else {
                let m = members[0];
                c.require(
                    target_pages.iter().all(|v| {
                        m["start_page"].as_i64().is_some_and(|x| x <= *v)
                            && m["end_page"].as_i64().is_some_and(|x| *v <= x)
                    }),
                    path,
                    "target division page leaves work boundary",
                )?;
            }
        }
    }
    c.fields(&p["summary"],&json!({"division_correspondence_count":divisions.len(),"anchor_count":anchors.len(),"numbered_division_count":spans.len(),"integer_numbered_units_covered":flattened.len(),"supplemental_numbered_units":supplemental}),path,|f|format!("summary {f} drifted"))?;
    let Some(prov) = p["provenance_ref"].as_str() else {
        c.issue(path, "provenance_ref is invalid")?;
        return Ok(());
    };
    let events = c.events(prov)?;
    if let Some(event) = c.event(
        &events,
        &p["provenance_event_ref"],
        Some(PARALLEL_EVENT_ID),
        prov,
        "parallel provenance event does not resolve",
    )? {
        let outputs = BTreeSet::from([
            c.bound_output(path, "tracked_text_free_parallel_structure_candidate")?,
            c.bound_output(
                PARALLEL_ANCHOR_RECORDS_PATH,
                "tracked_proposed_parallel_page_anchors",
            )?,
        ]);
        c.require(
            tuple_set(&event["outputs"], &["ref", "role", "sha256"]) == outputs,
            prov,
            "parallel event outputs drifted",
        )?;
        let mut inputs = BTreeSet::new();
        for w in [source, target] {
            if !w.is_object() {
                continue;
            }
            inputs.insert(tuple(&[w["file_ref"].clone(), w["file_sha256"].clone()]));
            for field in ["inventory", "work_boundary"] {
                if w[field].is_object() {
                    inputs.insert(tuple(&[
                        w[field]["ref"].clone(),
                        w[field]["sha256"].clone(),
                    ]));
                }
            }
        }
        if binding.is_object() {
            inputs.insert(tuple(&[binding["ref"].clone(), binding["sha256"].clone()]));
        }
        c.require(
            tuple_set(&event["inputs"], &["ref", "sha256"]) == inputs,
            prov,
            "parallel event inputs drifted",
        )?;
    }
    Ok(())
}
