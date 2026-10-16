use super::*;
pub(super) fn validate(c: &mut Context<'_>) -> io::Result<()> {
    let path = NUMBERED_UNIT_LABEL_MAP_PATH;
    let Some(p) = c.load_map(path, NUMBERED_UNIT_LABEL_SCHEMA_PATH)? else {
        return Ok(());
    };
    let source = &p["source_witness"];
    let target = &p["target_witness"];
    if !source.is_object() || !target.is_object() {
        c.issue(path, "numbered-label witness bindings are invalid")?;
        return Ok(());
    }
    let mut inputs = BTreeMap::new();
    for (w, expected_path, role) in [
        (source, NUMBERED_UNIT_MAP_PATH, "source"),
        (target, TARGET_NUMBERED_UNIT_MAP_PATH, "target"),
    ] {
        if w["numbered_unit_map_ref"] != expected_path {
            c.issue(path, format!("{role} numbered-unit map ref drifted"))?;
            continue;
        }
        let Some(v) = c.json(expected_path)? else {
            continue;
        };
        let digest = c.digest(expected_path)?;
        c.require(
            w["numbered_unit_map_sha256"] == digest,
            path,
            format!("{role} numbered-unit map digest drifted"),
        )?;
        let expected = json!({"expression_ref":v["expression_ref"],"edition_ref":v["edition_ref"],"item_ref":v["item_ref"],"file_ref":v["scan_file"]["file_ref"],"file_sha256":v["scan_file"]["file_sha256"],"numbered_unit_map_ref":expected_path,"numbered_unit_map_sha256":digest});
        c.require(
            *w == expected,
            path,
            format!("{role} witness binding drifted"),
        )?;
        inputs.insert(role, v);
    }
    let source_units = index(
        &inputs.get("source").unwrap_or(&Value::Null)["unit_starts"],
        "unit_key",
    );
    let target_units = index(
        &inputs.get("target").unwrap_or(&Value::Null)["unit_starts"],
        "unit_key",
    );
    let source_keys = keys(&source_units);
    let target_keys = keys(&target_units);
    c.require(
        source_keys
            .difference(&target_keys)
            .cloned()
            .collect::<BTreeSet<_>>()
            == strs(&["237a"]),
        path,
        "source-only numbered-label set drifted",
    )?;
    c.require(
        target_keys.is_subset(&source_keys),
        path,
        "unexpected target-only numbered labels appeared",
    )?;
    let source_anchors = c.jsonl(NUMBERED_UNIT_ANCHOR_RECORDS_PATH)?;
    let target_anchors = c.jsonl(TARGET_NUMBERED_UNIT_ANCHOR_RECORDS_PATH)?;
    let sa: Index<'_> = source_anchors
        .iter()
        .filter_map(|a| a["anchor_id"].as_str().map(|id| (id, a)))
        .collect();
    let ta: Index<'_> = target_anchors
        .iter()
        .filter_map(|a| a["anchor_id"].as_str().map(|id| (id, a)))
        .collect();
    let pairings: Vec<_> = objects(&p["pairings"]).collect();
    c.require(
        values(pairings.iter().map(|v| &v["unit_key"])) == json!(numbered::unit_keys(true)),
        path,
        "shared numbered-label pairing sequence drifted",
    )?;
    c.require(
        values(pairings.iter().map(|v| &v["sequence"])) == json!((1..=298).collect::<Vec<_>>()),
        path,
        "shared numbered-label ordinal sequence drifted",
    )?;
    for pair in &pairings {
        let key = text(&pair["unit_key"]);
        let su = get(&source_units, &pair["unit_key"]);
        let tu = get(&target_units, &pair["unit_key"]);
        if su.is_null() || tu.is_null() {
            c.issue(path, format!("pairing {key} does not resolve in both maps"))?;
            continue;
        }
        let expected = json!({"source_anchor_ref":su["anchor_ref"],"source_pdf_page":su["pdf_page"],"target_anchor_ref":tu["anchor_ref"],"target_pdf_page":tu["pdf_page"],"basis":"shared_materialized_number_label_key","status":"proposed","human_review_performed":false,"translation_alignment_claimed":false});
        c.fields(pair, &expected, path, |f| {
            format!("pairing {key} field {f} drifted")
        })?;
        for (role, anchors, expression) in [
            ("source", &sa, "de-naumann-1886"),
            ("target", &ta, "ru-polilov-mysl-1996"),
        ] {
            let a = get(anchors, &pair[format!("{role}_anchor_ref")]);
            c.require(!a.is_null()&&a["passage_id"]==format!("tos.passage.friedrich-nietzsche.jenseits-von-gut-und-boese.{expression}.unit-{key}"),path,format!("pairing {key} {role} anchor drifted"))?;
        }
    }
    let unpaired = arr(&p["unpaired_units"]);
    c.require(
        unpaired.len() == 1
            && unpaired[0]["unit_key"] == "237a"
            && unpaired[0]["target_unit_materialized"] == false
            && unpaired[0]["translation_alignment_claimed"] == false
            && source_keys.contains("237a")
            && !target_keys.contains("237a"),
        path,
        "source-only 237a pairing posture drifted",
    )?;
    let rights: Vec<_> = objects(&p["rights_basis"]).collect();
    c.require(
        rights
            .iter()
            .filter_map(|v| v["role"].as_str())
            .map(str::to_owned)
            .collect::<BTreeSet<_>>()
            == strs(&["source", "target"]),
        path,
        "numbered-label rights basis roles drifted",
    )?;
    for r in &rights {
        let role = text(&r["role"]);
        let Some(reference) = r["ref"].as_str() else {
            c.issue(path, "numbered-label rights ref is invalid")?;
            continue;
        };
        let expected = match role {
            "source" => NUMBERED_UNIT_LABEL_SOURCE_RIGHTS_PATH,
            "target" => NUMBERED_UNIT_LABEL_TARGET_RIGHTS_PATH,
            _ => "",
        };
        if expected.is_empty() || reference != expected {
            c.issue(path, format!("{role} rights basis ref drifted"))?;
            continue;
        }
        if c.json(reference)?.is_some() {
            let digest = c.digest(reference)?;
            c.require(
                r["sha256"] == digest,
                path,
                format!("rights basis digest drifted for {reference}"),
            )?;
        }
    }
    let expected = json!({"source_numbered_unit_count":source_units.len(),"target_numbered_unit_count":target_units.len(),"shared_materialized_label_count":source_keys.intersection(&target_keys).count(),"pairing_count":pairings.len(),"source_only_unit_keys":["237a"],"target_only_unit_keys":[],"all_pairing_statuses":["proposed"],"human_review_performed":false,"translation_alignment_claimed":false});
    c.fields(&p["summary"], &expected, path, |f| {
        format!("numbered-label summary {f} drifted")
    })?;
    let Some(prov) = p["provenance_ref"]
        .as_str()
        .filter(|s| *s == NUMBERED_UNIT_LABEL_PROVENANCE_PATH)
    else {
        c.issue(path, "numbered-label provenance_ref drifted")?;
        return Ok(());
    };
    let events = c.events(prov)?;
    if let Some(event) = c.event(
        &events,
        &p["provenance_event_ref"],
        Some(NUMBERED_UNIT_LABEL_EVENT_ID),
        prov,
        "numbered-label provenance event does not resolve",
    )? {
        let outputs = BTreeSet::from([c.bound_output(
            path,
            "tracked-text-free-shared-number-label-pairing-candidates",
        )?]);
        c.require(
            tuple_set(&event["outputs"], &["ref", "role", "sha256"]) == outputs,
            prov,
            "numbered-label event outputs drifted",
        )?;
        let mut inputs: BTreeSet<_> = [source, target]
            .into_iter()
            .map(|w| {
                tuple(&[
                    w["numbered_unit_map_ref"].clone(),
                    w["numbered_unit_map_sha256"].clone(),
                ])
            })
            .collect();
        inputs.extend(
            rights
                .iter()
                .map(|r| tuple(&[r["ref"].clone(), r["sha256"].clone()])),
        );
        c.require(
            tuple_set(&event["inputs"], &["ref", "sha256"]) == inputs,
            prov,
            "numbered-label event inputs drifted",
        )?;
    }
    Ok(())
}
