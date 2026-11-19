use super::*;
pub(super) fn schema(ctx: &ResearchExecution, packet: &Value) -> Result<()> {
    crate::source_text_foundation::unit_packet_schema(ctx, packet)
}

fn text_free(ctx: &ResearchExecution, v: &Value) -> Result<()> {
    ctx.tick(1)?;
    match v {
        Value::Object(m) => {
            for (key, value) in m {
                ensure(
                    ![
                        "source_text",
                        "text",
                        "content",
                        "exact_text",
                        "ocr_text",
                        "heading_text",
                        "label_text",
                        "translation",
                        "lemma",
                        "concept",
                        "sign",
                        "relation",
                    ]
                    .contains(&key.as_str()),
                    "content/semantic key escaped tracked boundary",
                )?;
                text_free(ctx, value)?;
            }
        }
        Value::Array(rows) => {
            for row in rows {
                text_free(ctx, row)?;
            }
        }
        _ => {}
    }
    Ok(())
}
fn array<'a>(v: &'a Value, key: &str) -> Result<&'a [Value]> {
    v[key]
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| format!("missing array: {key}"))
}
fn ordered_equal(left: &[Value], right: &[Value], key: &str) -> bool {
    left.iter()
        .map(|r| &r[key])
        .eq(right.iter().map(|r| &r[key]))
}
pub(super) fn tracked(
    ctx: &ResearchExecution,
    plan: &Value,
    screening: &Value,
) -> Result<(Value, Value)> {
    let issuance = issuance(ctx, plan, None)?;
    let p = load(ctx, out(plan, "packet_ref")?)?;
    schema(ctx, &p)?;
    let citations = load_lines(ctx, out(plan, "citation_spine_ref")?)?;
    let regions = load_lines(ctx, out(plan, "region_candidates_ref")?)?;
    let headings = load_lines(ctx, out(plan, "heading_candidates_ref")?)?;
    let screened = load_lines(ctx, out(plan, "heading_screening_ref")?)?;
    let overlay = load_lines(ctx, out(plan, "paragraph_segment_overlay_ref")?)?;
    let summary = load(ctx, out(plan, "summary_ref")?)?;
    let screen_summary = load(ctx, out(plan, "screening_summary_ref")?)?;
    for v in [
        &p,
        &json!(citations),
        &json!(regions),
        &json!(headings),
        &json!(screened),
        &json!(overlay),
        &summary,
        &screen_summary,
    ] {
        text_free(ctx, v)?;
    }
    for key in ["unit_id", "display_citation"] {
        ensure(
            citations
                .iter()
                .map(|r| s(&r[key]))
                .collect::<BTreeSet<_>>()
                .len()
                == citations.len(),
            "citation identity/display collision",
        )?;
    }
    let issued = array(&issuance, "units")?;
    for key in ["source_locator", "unit_id", "anchor_ref"] {
        ensure(
            ordered_equal(&citations, issued, key),
            "citation/issuance ordered binding drift",
        )?;
    }
    ensure(
        ordered_equal(array(&p, "units")?, &citations, "unit_id"),
        "packet/citation unit closure drift",
    )?;
    ensure(
        p["projections"][0]["artifact_sha256"] == sha(&ctx.read(out(plan, "citation_spine_ref")?)?),
        "packet citation digest drift",
    )?;
    ensure(
        regions.len() == a(&plan["region_candidates"]).len()
            && headings.len() == n(&plan["expected_counts"]["heading_candidates"]),
        "tracked region/heading count drift",
    )?;
    let roles = views::roles(screening)?;
    ensure(
        screened.len() == headings.len(),
        "heading screening count drift",
    )?;
    for (candidate, row) in headings.iter().zip(&screened) {
        ctx.tick(1)?;
        ensure(
            row["candidate_ordinal"] == candidate["candidate_ordinal"],
            "heading screening order drift",
        )?;
        for key in [
            "unit_id",
            "anchor_ref",
            "source_locator",
            "display_citation",
        ] {
            ensure(
                row[key] == candidate[key],
                "heading screening source binding drift",
            )?;
        }
        ensure(
            roles
                .get(&n(&candidate["candidate_ordinal"]))
                .is_some_and(|r| s(&row["technical_role"]) == r),
            "heading role drift",
        )?;
        ensure(
            row["real_human_reviewer"] == json!(false) && row["accepted_section"] == json!(false),
            "model screening widened review/section authority",
        )?;
    }
    let citation_by_unit: BTreeMap<_, _> =
        citations.iter().map(|r| (s(&r["unit_id"]), r)).collect();
    let mut overlay_by_unit = BTreeMap::<&str, Vec<&Value>>::new();
    for (ordinal, row) in overlay.iter().enumerate() {
        ctx.tick(1)?;
        ensure(
            row["overlay_segment_ordinal"] == json!(ordinal + 1),
            "overlay order drift",
        )?;
        let unit = s(&row["source_block_unit_id"]);
        let c = citation_by_unit
            .get(unit)
            .ok_or("overlay unresolved source unit")?;
        ensure(
            s(&c["unit_kind"]) == "paragraph" && row["source_locator"] == c["source_locator"],
            "overlay source kind/locator drift",
        )?;
        ensure(
            row["creates_new_text_unit_identity"] == json!(false)
                && row["accepted_paragraph"] == json!(false),
            "overlay widened identity/paragraph authority",
        )?;
        overlay_by_unit.entry(unit).or_default().push(row);
    }
    for (unit, rows) in &overlay_by_unit {
        ctx.tick(1)?;
        let c = citation_by_unit[unit];
        ensure(
            rows[0]["selector_start"] == c["selector_start"]
                && rows[rows.len() - 1]["selector_end"] == c["selector_end"],
            "overlay start/end coverage drift",
        )?;
        for (ordinal, row) in rows.iter().enumerate() {
            ensure(
                row["segment_ordinal_within_source_block"] == json!(ordinal + 1),
                "overlay local ordinal drift",
            )?;
        }
        ensure(
            rows.windows(2)
                .all(|pair| pair[1]["selector_start"] == pair[0]["selector_end"]),
            "overlay gap/overlap",
        )?;
    }
    let rule = &screening["paragraph_split_screening"];
    ensure(
        overlay_by_unit.len() == n(&rule["expected_affected_source_block_count"])
            && overlay.len() - overlay_by_unit.len() == n(&rule["expected_split_boundary_count"])
            && overlay.len() == n(&rule["expected_proposed_segment_count"]),
        "overlay count surface drift",
    )?;
    ensure(
        summary["tracked_unit_count"] == json!(citations.len())
            && summary["layout_block_paragraph_candidate_count"]
                == json!(
                    citations
                        .iter()
                        .filter(|r| s(&r["unit_kind"]) == "paragraph")
                        .count()
                )
            && summary["heading_candidate_count"] == json!(headings.len()),
        "summary count drift",
    )?;
    ensure(
        screen_summary["heading_candidate_count"] == json!(screened.len())
            && screen_summary["affected_source_block_count"] == json!(overlay_by_unit.len())
            && screen_summary["proposed_segment_count"] == json!(overlay.len()),
        "screening summary count drift",
    )?;
    ensure(
        screen_summary["human_review_performed"] == json!(false),
        "screening summary became human review",
    )?;
    for key in [
        "accepted_russian_text",
        "source_target_alignment_created",
        "semantic_fields_materialized",
    ] {
        ensure(
            summary[key] == json!(false),
            "summary widened source/alignment/semantic authority",
        )?;
    }
    ensure(
        p["rights_and_visibility"]["publication_authorized"] == json!(false)
            && p["reviews"] == json!([]),
        "packet widened publication/review authority",
    )?;
    ctx.check()?;
    Ok((summary, screen_summary))
}

/// Validate every available tracked artifact independently when a manifested
/// private member is not hydrated. Checks requiring absent operands are left
/// to the complete route after recovery.
pub(super) fn tracked_partial(
    ctx: &ResearchExecution,
    plan: &Value,
    screening: &Value,
    missing_names: &[String],
) -> Result<Vec<&'static str>> {
    let missing: BTreeSet<_> = missing_names.iter().map(String::as_str).collect();
    let available = |key: &str| -> Result<bool> {
        let reference = out(plan, key)?;
        Ok(!missing.contains(
            Path::new(reference)
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or("tracked output filename")?,
        ))
    };
    let issuance = issuance(ctx, plan, None)?;
    let packet = if available("packet_ref")? {
        let value = load(ctx, out(plan, "packet_ref")?)?;
        schema(ctx, &value)?;
        text_free(ctx, &value)?;
        Some(value)
    } else {
        None
    };
    let citations = if available("citation_spine_ref")? {
        let rows = load_lines(ctx, out(plan, "citation_spine_ref")?)?;
        text_free(ctx, &json!(rows))?;
        for key in ["unit_id", "display_citation"] {
            ensure(
                rows.iter()
                    .map(|row| s(&row[key]))
                    .collect::<BTreeSet<_>>()
                    .len()
                    == rows.len(),
                "citation identity/display collision",
            )?;
        }
        let issued = array(&issuance, "units")?;
        for key in ["source_locator", "unit_id", "anchor_ref"] {
            ensure(
                ordered_equal(&rows, issued, key),
                "citation/issuance ordered binding drift",
            )?;
        }
        Some(rows)
    } else {
        None
    };
    let regions = if available("region_candidates_ref")? {
        let rows = load_lines(ctx, out(plan, "region_candidates_ref")?)?;
        text_free(ctx, &json!(rows))?;
        ensure(
            rows.len() == a(&plan["region_candidates"]).len(),
            "tracked region count drift",
        )?;
        Some(rows)
    } else {
        None
    };
    let headings = if available("heading_candidates_ref")? {
        let rows = load_lines(ctx, out(plan, "heading_candidates_ref")?)?;
        text_free(ctx, &json!(rows))?;
        ensure(
            rows.len() == n(&plan["expected_counts"]["heading_candidates"]),
            "tracked heading count drift",
        )?;
        Some(rows)
    } else {
        None
    };
    let screened = if available("heading_screening_ref")? {
        let rows = load_lines(ctx, out(plan, "heading_screening_ref")?)?;
        text_free(ctx, &json!(rows))?;
        Some(rows)
    } else {
        None
    };
    if let (Some(headings), Some(screened)) = (&headings, &screened) {
        let roles = views::roles(screening)?;
        ensure(
            screened.len() == headings.len(),
            "heading screening count drift",
        )?;
        for (candidate, row) in headings.iter().zip(screened) {
            ctx.tick(1)?;
            ensure(
                row["candidate_ordinal"] == candidate["candidate_ordinal"],
                "heading screening order drift",
            )?;
            for key in [
                "unit_id",
                "anchor_ref",
                "source_locator",
                "display_citation",
            ] {
                ensure(
                    row[key] == candidate[key],
                    "heading screening source binding drift",
                )?;
            }
            ensure(
                roles
                    .get(&n(&candidate["candidate_ordinal"]))
                    .is_some_and(|role| s(&row["technical_role"]) == role)
                    && row["real_human_reviewer"] == json!(false)
                    && row["accepted_section"] == json!(false),
                "heading screening role/review boundary drift",
            )?;
        }
    }
    let overlay = if available("paragraph_segment_overlay_ref")? {
        let rows = load_lines(ctx, out(plan, "paragraph_segment_overlay_ref")?)?;
        text_free(ctx, &json!(rows))?;
        Some(rows)
    } else {
        None
    };
    let overlay_units = if let (Some(citations), Some(overlay)) = (&citations, &overlay) {
        let citation_by_unit: BTreeMap<_, _> = citations
            .iter()
            .map(|row| (s(&row["unit_id"]), row))
            .collect();
        let mut by_unit = BTreeMap::<&str, Vec<&Value>>::new();
        for (ordinal, row) in overlay.iter().enumerate() {
            ctx.tick(1)?;
            ensure(
                row["overlay_segment_ordinal"] == json!(ordinal + 1),
                "overlay order drift",
            )?;
            let unit = s(&row["source_block_unit_id"]);
            let citation = citation_by_unit
                .get(unit)
                .ok_or("overlay unresolved source unit")?;
            ensure(
                s(&citation["unit_kind"]) == "paragraph"
                    && row["source_locator"] == citation["source_locator"]
                    && row["creates_new_text_unit_identity"] == json!(false)
                    && row["accepted_paragraph"] == json!(false),
                "overlay source or authority boundary drift",
            )?;
            by_unit.entry(unit).or_default().push(row);
        }
        for (unit, rows) in &by_unit {
            let citation = citation_by_unit[unit];
            ensure(
                rows[0]["selector_start"] == citation["selector_start"]
                    && rows[rows.len() - 1]["selector_end"] == citation["selector_end"]
                    && rows.iter().enumerate().all(|(ordinal, row)| {
                        row["segment_ordinal_within_source_block"] == json!(ordinal + 1)
                    })
                    && rows
                        .windows(2)
                        .all(|pair| pair[1]["selector_start"] == pair[0]["selector_end"]),
                "overlay segment coverage/ordinal drift",
            )?;
        }
        Some(by_unit.len())
    } else {
        None
    };
    let summary = if available("summary_ref")? {
        let value = load(ctx, out(plan, "summary_ref")?)?;
        text_free(ctx, &value)?;
        if let (Some(citations), Some(headings)) = (&citations, &headings) {
            ensure(
                value["tracked_unit_count"] == json!(citations.len())
                    && value["layout_block_paragraph_candidate_count"]
                        == json!(
                            citations
                                .iter()
                                .filter(|row| s(&row["unit_kind"]) == "paragraph")
                                .count()
                        )
                    && value["heading_candidate_count"] == json!(headings.len())
                    && value["accepted_russian_text"] == json!(false)
                    && value["source_target_alignment_created"] == json!(false)
                    && value["semantic_fields_materialized"] == json!(false),
                "summary count or authority boundary drift",
            )?;
        }
        Some(value)
    } else {
        None
    };
    let screen_summary = if available("screening_summary_ref")? {
        let value = load(ctx, out(plan, "screening_summary_ref")?)?;
        text_free(ctx, &value)?;
        if let (Some(screened), Some(overlay), Some(overlay_units)) =
            (&screened, &overlay, overlay_units)
        {
            ensure(
                value["heading_candidate_count"] == json!(screened.len())
                    && value["affected_source_block_count"] == json!(overlay_units)
                    && value["proposed_segment_count"] == json!(overlay.len())
                    && value["human_review_performed"] == json!(false),
                "screening summary closure/review drift",
            )?;
        }
        Some(value)
    } else {
        None
    };
    if let (Some(packet), Some(citations)) = (&packet, &citations) {
        ensure(
            ordered_equal(array(packet, "units")?, citations, "unit_id")
                && packet["projections"][0]["artifact_sha256"]
                    == sha(&ctx.read(out(plan, "citation_spine_ref")?)?),
            "packet/citation closure drift",
        )?;
    }
    if available("provenance_ref")? {
        let provenance = load_lines(ctx, out(plan, "provenance_ref")?)?;
        text_free(ctx, &json!(provenance))?;
    }
    let mut checked = vec!["available artifact hashes, parsing and text-free boundary"];
    if packet.is_some() {
        checked.push("available packet schema");
    }
    if citations.is_some() {
        checked.push("citation identities and issuance binding");
    }
    if headings.is_some() {
        checked.push("heading census");
    }
    if regions.is_some() {
        checked.push("region census");
    }
    if screened.is_some() && headings.is_some() {
        checked.push("heading screening closure and role boundary");
    }
    if overlay.is_some() && citations.is_some() {
        checked.push("paragraph overlay source closure and local segments");
    }
    if summary.is_some() && citations.is_some() && headings.is_some() {
        checked.push("summary counts and authority boundary");
    }
    if screen_summary.is_some() && screened.is_some() && overlay.is_some() {
        checked.push("screening summary closure");
    }
    if packet.is_some() && citations.is_some() {
        checked.push("packet/citation closure");
    }
    ctx.check()?;
    Ok(checked)
}
