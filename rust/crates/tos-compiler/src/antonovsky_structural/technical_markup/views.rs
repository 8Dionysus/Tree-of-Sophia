use super::*;
pub(super) struct Views {
    pub anchors: Vec<Value>,
    pub units: Vec<Value>,
    pub citations: Vec<Value>,
    pub regions: Vec<Value>,
    pub headings: Vec<Value>,
}
struct Node<'a> {
    kind: &'a str,
    role: &'a str,
    locator: &'a str,
    start: usize,
    end: usize,
    parent: Option<&'a str>,
    children: Vec<&'a str>,
    regions: Vec<Value>,
    page: Option<usize>,
    panel: Option<&'a str>,
    bbox: Value,
    blocks: Vec<usize>,
    display: String,
    source_return: String,
    block: Option<usize>,
}
fn emit(
    ctx: &ResearchExecution,
    doc: &Document,
    plan: &Value,
    ids: &BTreeMap<&str, &Value>,
    v: &mut Views,
    node: Node<'_>,
) -> Result<()> {
    ctx.tick(1)?;
    let identity = ids.get(node.locator).ok_or("unissued source locator")?;
    let ordinal = v.units.len() + 1;
    let source_attested = matches!(node.role, "pdf_item_scope" | "source_attested_pdf_page");
    let boundary = if source_attested {
        "source_attested"
    } else {
        "method_proposed"
    };
    let certainty = if source_attested {
        1.
    } else if node.kind == "paragraph" {
        0.55
    } else {
        0.85
    };
    let reason = match node.role {
        "pdf_item_scope" => "The exact item-level PDF document boundary is source-attested.",
        "source_attested_pdf_page" => {
            "The exact PDF page boundary is source-attested; printed-page interpretation is separate."
        }
        "two_up_page_side_panel_candidate" => {
            "The two-up page-side panel is assigned by the fixed bbox-center midpoint rule."
        }
        _ => {
            "The unit is one Poppler bbox-layout block counted only as an unreviewed paragraph candidate."
        }
    };
    let children = node
        .children
        .iter()
        .map(|loc| {
            ids.get(loc)
                .ok_or_else(|| "child identity".to_string())
                .map(|id| id["unit_id"].clone())
        })
        .collect::<Result<Vec<_>>>()?;
    let parent = node
        .parent
        .map(|loc| {
            ids.get(loc)
                .ok_or("parent identity")
                .map(|id| id["unit_id"].clone())
        })
        .transpose()?;
    let digest = sha(doc.slice(node.start, node.end)?.as_bytes());
    v.anchors.push(json!({"anchor_ref":identity["anchor_ref"],"ordinal":ordinal,"text_layer_ref":plan["outputs"]["private_text_layer_ref"],"text_layer_sha256":doc.text_sha,"selector":{"type":"text_position","start":node.start,"end":node.end,"position_unit":"unicode_code_point","interval":"half_open"},"exact_sha256":digest,"anchor_role":if node.kind=="paragraph" {"content"} else {"scope"},"source_return":{"required":true,"locator_ref":node.source_return}}));
    v.units.push(json!({"unit_id":identity["unit_id"],"unit_version":1,"supersedes_unit_ref":null,"identity_policy":"opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis","unit_kind":node.kind,"surface_posture":"source_bearing","continuity":"contiguous","ordered_anchor_refs":[identity["anchor_ref"]],"parent_unit_refs":parent.clone().into_iter().collect::<Vec<_>>(),"ordered_child_unit_refs":children,"boundary_posture":boundary,"certainty":{"value":certainty,"meaning":"maker-declared-boundary-confidence-not-truth-probability"},"status_reason":reason,"source_text_mutated":false,"semantic_promotion":false}));
    let (blocks, lines, words, headings) = doc.counts(&node.blocks);
    let resource = node
        .page
        .map(|p| doc.inventory[p - 1]["resource_id"].clone());
    let flow = node.block.map(|i| doc.blocks[i].flow);
    let blockordinal = node.block.map(|i| doc.blocks[i].ordinal);
    v.citations.push(json!({"schema_version":"tos_zarathustra_antonovsky_technical_citation_v1","ordinal":ordinal,"unit_id":identity["unit_id"],"anchor_ref":identity["anchor_ref"],"unit_kind":node.kind,"structural_role":node.role,"source_locator":node.locator,"display_citation":node.display,"parent_unit_id":parent,"ordered_child_unit_ids":children,"region_candidate_ids":node.regions,"pdf_page":node.page,"panel":node.panel,"source_resource_id":resource,"poppler_flow_ordinal":flow,"poppler_block_ordinal":blockordinal,"bbox_points":node.bbox,"line_count":lines,"word_count":words,"contained_paragraph_candidate_count":blocks,"contained_heading_candidate_count":headings,"selector_start":node.start,"selector_end":node.end,"exact_sha256":digest,"boundary_posture":boundary,"source_text_included":false,"semantic_promotion":false}));
    if let Some(i) = node.block {
        if doc.headings[i] {
            let b = &doc.blocks[i];
            v.headings.push(json!({"schema_version":"tos_zarathustra_antonovsky_heading_candidate_v1","candidate_ordinal":v.headings.len()+1,"unit_id":identity["unit_id"],"anchor_ref":identity["anchor_ref"],"source_locator":b.locator,"display_citation":node.display,"region_candidate_id":doc.region[i],"pdf_page":b.page,"panel":b.panel,"source_resource_id":resource,"bbox_points":b.bbox,"line_count":b.lines.len(),"word_count":b.words,"median_word_height_points":round(median(&b.word_heights),6),"gap_before_points":round(doc.gaps[i].0,6),"gap_after_points":round(doc.gaps[i].1,6),"rule_version":plan["heading_candidate_rule"]["rule_version"],"status":"proposed","coverage_posture":plan["heading_candidate_rule"]["coverage_posture"],"source_text_included":false,"accepted_section":false,"semantic_promotion":false}));
        }
    }
    Ok(())
}
pub(super) fn build(
    ctx: &ResearchExecution,
    doc: &Document,
    plan: &Value,
    issuance: &Value,
) -> Result<Views> {
    let ids: BTreeMap<_, _> = a(&issuance["units"])
        .iter()
        .map(|r| (s(&r["source_locator"]), r))
        .collect();
    let mut v = Views {
        anchors: vec![],
        units: vec![],
        citations: vec![],
        regions: vec![],
        headings: vec![],
    };
    let source = &plan["source_item"];
    emit(
        ctx,
        doc,
        plan,
        &ids,
        &mut v,
        Node {
            kind: "document",
            role: "pdf_item_scope",
            locator: "pdf-document",
            start: 0,
            end: doc.offsets.len() - 1,
            parent: None,
            children: doc.pages.iter().map(|p| p.locator.as_str()).collect(),
            regions: a(&plan["region_candidates"])
                .iter()
                .map(|r| r["region_id"].clone())
                .collect(),
            page: None,
            panel: None,
            bbox: Value::Null,
            blocks: (0..doc.blocks.len()).collect(),
            display: "Za-RU-Ant1911".into(),
            source_return: s(&source["manifest_ref"]).into(),
            block: None,
        },
    )?;
    for page in &doc.pages {
        ctx.tick(1)?;
        let panels: Vec<_> = doc.panels.iter().filter(|p| p.page == page.page).collect();
        let mut regions = Vec::new();
        for panel in &panels {
            let r = json!(doc.region[panel.blocks[0]]);
            if !regions.contains(&r) {
                regions.push(r);
            }
        }
        let [width, height] = doc.dimensions[page.page - 1];
        let page_citation = format!("Za-RU-Ant1911.pdf{:03}", page.page);
        let resource = s(&doc.inventory[page.page - 1]["resource_id"]);
        emit(
            ctx,
            doc,
            plan,
            &ids,
            &mut v,
            Node {
                kind: "section",
                role: "source_attested_pdf_page",
                locator: &page.locator,
                start: page.start,
                end: page.end,
                parent: Some("pdf-document"),
                children: panels.iter().map(|p| p.locator.as_str()).collect(),
                regions,
                page: Some(page.page),
                panel: None,
                bbox: json!([0., 0., width, height]),
                blocks: page.blocks.clone(),
                display: page_citation.clone(),
                source_return: format!("{}#{resource}", s(&source["resource_inventory_ref"])),
                block: None,
            },
        )?;
        for panel in panels {
            ctx.tick(1)?;
            let panel_name = panel.panel.as_deref().ok_or("panel name")?;
            let panel_citation = format!(
                "{page_citation}.{}",
                if panel_name == "left" { "L" } else { "R" }
            );
            let source_panel = format!(
                "{}#{resource}/panel-{panel_name}",
                s(&source["manifest_ref"])
            );
            emit(
                ctx,
                doc,
                plan,
                &ids,
                &mut v,
                Node {
                    kind: "section",
                    role: "two_up_page_side_panel_candidate",
                    locator: &panel.locator,
                    start: panel.start,
                    end: panel.end,
                    parent: Some(&page.locator),
                    children: panel
                        .blocks
                        .iter()
                        .map(|i| doc.blocks[*i].locator.as_str())
                        .collect(),
                    regions: vec![json!(doc.region[panel.blocks[0]])],
                    page: Some(page.page),
                    panel: Some(panel_name),
                    bbox: json!([
                        if panel_name == "left" { 0. } else { width / 2. },
                        0.,
                        if panel_name == "left" {
                            width / 2.
                        } else {
                            width
                        },
                        height
                    ]),
                    blocks: panel.blocks.clone(),
                    display: panel_citation.clone(),
                    source_return: source_panel,
                    block: None,
                },
            )?;
            for (ordinal, i) in panel.blocks.iter().enumerate() {
                let b = &doc.blocks[*i];
                emit(
                    ctx,
                    doc,
                    plan,
                    &ids,
                    &mut v,
                    Node {
                        kind: "paragraph",
                        role: "layout_block_paragraph_candidate",
                        locator: &b.locator,
                        start: doc.starts[*i],
                        end: doc.ends[*i],
                        parent: Some(&panel.locator),
                        children: vec![],
                        regions: vec![json!(doc.region[*i])],
                        page: Some(page.page),
                        panel: Some(panel_name),
                        bbox: json!(b.bbox),
                        blocks: vec![*i],
                        display: format!("{panel_citation}.b{:04}", ordinal + 1),
                        source_return: format!(
                            "{}#{resource}/panel-{panel_name}/flow-{:04}/block-{:04}",
                            s(&source["manifest_ref"]),
                            b.flow,
                            b.ordinal
                        ),
                        block: Some(*i),
                    },
                )?;
            }
        }
    }
    for (ordinal, reg) in a(&plan["region_candidates"]).iter().enumerate() {
        ctx.tick(1)?;
        let selected: Vec<_> = doc
            .panels
            .iter()
            .filter(|p| doc.region[p.blocks[0]] == s(&reg["region_id"]))
            .collect();
        ensure(!selected.is_empty(), "empty region candidate")?;
        let first = selected[0];
        let last = selected[selected.len() - 1];
        let lookup = |loc: &str| {
            v.citations
                .iter()
                .find(|r| s(&r["source_locator"]) == loc)
                .ok_or("region citation")
        };
        let first = lookup(&first.locator)?;
        let last = lookup(&last.locator)?;
        let indices = selected
            .iter()
            .flat_map(|p| p.blocks.clone())
            .collect::<Vec<_>>();
        let (blocks, lines, words, heads) = doc.counts(&indices);
        v.regions.push(json!({"schema_version":"tos_zarathustra_antonovsky_region_candidate_v1","region_ordinal":ordinal+1,"region_candidate_id":reg["region_id"],"display_label":reg["display_label"],"declared_start":reg["start"],"declared_end":reg["end"],"first_observed_panel_citation":first["display_citation"],"first_observed_panel_unit_id":first["unit_id"],"last_observed_panel_citation":last["display_citation"],"last_observed_panel_unit_id":last["unit_id"],"observed_panel_count":selected.len(),"paragraph_candidate_count":blocks,"physical_line_count":lines,"embedded_word_count":words,"heading_candidate_count":heads,"status":"proposed","human_review_performed":false,"german_correspondence_created":false,"source_text_included":false,"semantic_promotion":false}));
    }
    ensure(
        v.units.len() == v.anchors.len() && v.units.len() == v.citations.len(),
        "unit/anchor/citation ordinal closure",
    )?;
    Ok(v)
}
pub(super) fn roles(screening: &Value) -> Result<BTreeMap<usize, String>> {
    let mut roles = BTreeMap::new();
    for group in a(&screening["heading_screening"]["classification_groups"]) {
        for value in a(&group["candidate_ordinals"]) {
            ensure(
                roles
                    .insert(n(value), s(&group["technical_role"]).into())
                    .is_none(),
                "duplicate heading ordinal",
            )?;
        }
    }
    Ok(roles)
}
fn splits(doc: &Document, i: usize, rule: &Value, guard: bool) -> Vec<usize> {
    let b = &doc.blocks[i];
    let f = |k: &str| rule[k].as_f64().unwrap_or(f64::NAN);
    let height = if b.word_heights.is_empty() {
        0.
    } else {
        median(&b.word_heights)
    };
    if !a(&rule["scope_region_candidate_ids"])
        .iter()
        .any(|r| s(r) == doc.region[i])
        || !(f("minimum_block_median_word_height_points") <= height
            && height <= f("maximum_block_median_word_height_points"))
    {
        return vec![];
    }
    let result: Vec<_> = b
        .lines
        .iter()
        .zip(&b.line_word_counts)
        .enumerate()
        .skip(1)
        .filter(|(_, ((_, bbox), words))| {
            let indent = bbox[0] - b.bbox[0];
            f("minimum_indent_from_block_left_points") <= indent
                && indent <= f("maximum_indent_from_block_left_points")
                && **words >= n(&rule["minimum_words_on_split_line"])
        })
        .map(|(i, _)| i + 1)
        .collect();
    let g = &rule["verse_guard"];
    if guard
        && b.lines.len() >= n(&g["minimum_block_line_count"])
        && !result.is_empty()
        && result.len() as f64 / (b.lines.len() - 1) as f64
            > g["maximum_candidate_split_ratio"]
                .as_f64()
                .unwrap_or(f64::NAN)
    {
        vec![]
    } else {
        result
    }
}
pub(super) fn screen(
    ctx: &ResearchExecution,
    doc: &Document,
    plan: &Value,
    screening: &Value,
    v: &Views,
) -> Result<(Vec<Value>, Vec<Value>, Value)> {
    ensure(
        s(&screening["private_text_layer_sha256"]) == doc.text_sha,
        "screening text fixity drift",
    )?;
    let roles = roles(screening)?;
    let mut headings = Vec::new();
    let mut counts = BTreeMap::<String, usize>::new();
    for h in &v.headings {
        ctx.tick(1)?;
        let role = roles
            .get(&n(&h["candidate_ordinal"]))
            .ok_or("heading role")?;
        increment(&mut counts, role);
        headings.push(json!({"schema_version":"tos_zarathustra_antonovsky_heading_screening_v1","screening_id":screening["screening_id"],"candidate_ordinal":h["candidate_ordinal"],"unit_id":h["unit_id"],"anchor_ref":h["anchor_ref"],"source_locator":h["source_locator"],"display_citation":h["display_citation"],"region_candidate_id":h["region_candidate_id"],"technical_role":role,"screening_outcome":"retained_role_classified","maker_kind":"model_source_visible","source_visible":true,"real_human_reviewer":false,"packet_review_created":false,"accepted_section":false,"source_text_included":false,"semantic_promotion":false}));
    }
    let rule = &screening["paragraph_split_screening"];
    let citations: BTreeMap<_, _> = v
        .citations
        .iter()
        .map(|r| (s(&r["source_locator"]), r))
        .collect();
    let mut segments = Vec::new();
    let mut affected = 0;
    let mut split_count = 0;
    let mut raw_blocks = 0;
    let mut raw_splits = 0;
    let mut excluded_blocks = 0;
    let mut excluded_splits = 0;
    for (i, b) in doc.blocks.iter().enumerate() {
        ctx.tick(1)?;
        let raw = splits(doc, i, rule, false);
        let split = splits(doc, i, rule, true);
        if !raw.is_empty() {
            raw_blocks += 1;
            raw_splits += raw.len();
        }
        if !raw.is_empty() && split.is_empty() {
            excluded_blocks += 1;
            excluded_splits += raw.len();
        }
        if split.is_empty() {
            continue;
        }
        affected += 1;
        split_count += split.len();
        let c = citations[&b.locator.as_str()];
        let mut starts = Vec::new();
        let mut position = doc.starts[i];
        for (j, (line, _)) in b.lines.iter().enumerate() {
            starts.push(position);
            position += line.chars().count();
            if j + 1 < b.lines.len() {
                position += 1;
            }
        }
        ensure(position == doc.ends[i], "screening line offset drift")?;
        let boundaries = std::iter::once(1)
            .chain(split)
            .chain(std::iter::once(b.lines.len() + 1))
            .collect::<Vec<_>>();
        for (within, pair) in boundaries.windows(2).enumerate() {
            ctx.tick(1)?;
            let start = starts[pair[0] - 1];
            let end = if pair[1] == b.lines.len() + 1 {
                doc.ends[i]
            } else {
                starts[pair[1] - 1]
            };
            ensure(start < end, "empty screened span")?;
            segments.push(json!({"schema_version":"tos_zarathustra_antonovsky_paragraph_segment_overlay_v1","screening_id":screening["screening_id"],"overlay_segment_ordinal":segments.len()+1,"source_block_unit_id":c["unit_id"],"source_block_anchor_ref":c["anchor_ref"],"source_locator":b.locator,"source_block_display_citation":c["display_citation"],"region_candidate_id":doc.region[i],"segment_ordinal_within_source_block":within+1,"line_start_ordinal":pair[0],"line_end_ordinal":pair[1]-1,"selector_start":start,"selector_end":end,"exact_sha256":sha(doc.slice(start,end)?.as_bytes()),"boundary_before":if within==0 {"inherited_poppler_block_start"}else{"proposed_source_observed_indent"},"boundary_after":if pair[1]==b.lines.len()+1 {"inherited_poppler_block_end"}else{"proposed_source_observed_indent"},"split_rule_version":rule["rule_version"],"status":"proposed","creates_new_text_unit_identity":false,"source_block_unit_replaced":false,"human_review_performed":false,"accepted_paragraph":false,"source_text_included":false,"semantic_promotion":false}));
        }
    }
    ensure(
        affected == n(&rule["expected_affected_source_block_count"])
            && split_count == n(&rule["expected_split_boundary_count"])
            && segments.len() == n(&rule["expected_proposed_segment_count"]),
        "screening count surface drift",
    )?;
    let summary = json!({"schema_version":"tos_zarathustra_antonovsky_technical_screening_summary_v1","screening_id":screening["screening_id"],"source_packet_ref":screening["source_packet_ref"],"source_file_sha256":screening["source_file_sha256"],"source_bbox_sha256":screening["source_bbox_sha256"],"private_text_layer_sha256":screening["private_text_layer_sha256"],"heading_candidate_count":headings.len(),"heading_technical_role_counts":counts,"heading_screening_coverage":"all_enumerated_candidates","full_witness_heading_recall_claimed":false,"raw_in_scope_indent_candidate_block_count":raw_blocks,"raw_in_scope_indent_boundary_count":raw_splits,"verse_guard_excluded_block_count":excluded_blocks,"verse_guard_excluded_boundary_count":excluded_splits,"affected_source_block_count":affected,"split_boundary_count":split_count,"proposed_segment_count":segments.len(),"unaffected_inherited_source_block_count":n(&plan["expected_counts"]["layout_block_paragraph_candidates"])-affected,"effective_proposed_paragraph_candidate_count":n(&plan["expected_counts"]["layout_block_paragraph_candidates"])+split_count,"source_visible_heading_candidate_count":headings.len(),"source_visible_split_sample_block_count":rule["source_visible_sample"]["sample_block_count"],"maker_kind":"model_source_visible","status":"proposed","packet_review_created":false,"human_review_performed":false,"accepted_sections":false,"accepted_paragraphs":false,"source_block_units_replaced":false,"new_text_unit_identities_issued":false,"source_target_alignment_created":false,"full_alignment_readiness_claimed":false,"semantic_fields_materialized":false,"source_text_included":false});
    Ok((headings, segments, summary))
}
