use super::*;
use views::Views;
fn method(plan: &Value) -> Value {
    json!({"maker_kind":"software","agent_ref":"software:tos-antonovsky-1911-technical-markup-builder","method_name":"exact PDF Poppler bbox-layout technical proposal","method_version":"1","software_refs":[BUILDER,"pdftotext:26.01.0"],"model_ref":null,"configuration_ref":plan_ref(),"locale":"ru","unicode_version":null,"unicode_revision":null,"tailoring_ref":null,"provenance_event_ref":EVENT,"made_at":plan["created_at"],"output_posture":"method_result_not_source_or_linguistic_truth"})
}
fn packet(
    ctx: &ResearchExecution,
    doc: &Document,
    plan: &Value,
    issuance: &Value,
    v: &Views,
    citation_digest: &str,
) -> Result<Value> {
    let fixed = &issuance["fixed_ids"];
    let src = &plan["source_item"];
    let method = method(plan);
    let p = json!({"$schema":"https://tree-of-sophia.local/ToS/contracts/source-text-unit-packet-v1.schema.json","schema_version":"tos_source_text_unit_packet_v1","packet_id":fixed["packet_id"],"packet_version":1,"supersedes_packet_ref":null,"content_posture":"source_bound",
        "source_scope":{"work_ref":plan["work_ref"],"expression_ref":src["expression_ref"],"edition_ref":src["edition_ref"],"item_ref":src["item_ref"],"file_ref":src["file_ref"],"file_sha256":src["file_sha256"]},
        "source_layer":{"text_layer_ref":plan["outputs"]["private_text_layer_ref"],"text_layer_sha256":doc.text_sha,"language":plan["language"],"media_type":"text/plain; charset=utf-8","unicode_form":"source_preserved","position_unit":"unicode_code_point","interval":"half_open","immutable":true,"visibility":"local_only","publication_authorized":false},
        "schemes":[{"scheme_id":fixed["scheme_id"],"scheme_version":1,"supersedes_scheme_ref":null,"identity_policy":"opaque-id-independent-of-name-label-text-ordinal-offset-and-current-analysis","scheme_name":"Antonovsky 1911 PDF page-panel-layout-block proposal v1","analysis_role":"source_layout","boundary_basis":"rule_based","unit_kinds":["document","section","paragraph"],"method":method,"policies":{"normalization":"no-text-mutation-separate-successor-layer","punctuation":"included_in_neighbor","whitespace":"included_in_neighbor","line_break":"included_in_neighbor","hyphenation":"preserve_source","unreported_gaps_allowed":false,"overlap":"nested_only"},"authority_limit":{"algorithmic_output_is_source_truth":false,"algorithmic_output_is_linguistic_truth":false,"model_subword_is_lexeme":false,"unit_identity_is_semantic_identity":false,"text_mutation_allowed":false}}],
        "anchors":v.anchors,"units":v.units,
        "segmentations":[{"segmentation_id":fixed["segmentation_id"],"segmentation_version":1,"supersedes_segmentation_ref":null,"identity_policy":"opaque-id-independent-of-scheme-name-unit-order-text-and-current-boundaries","scheme_ref":fixed["scheme_id"],"ordered_unit_refs":v.units.iter().map(|r|&r["unit_id"]).collect::<Vec<_>>(),"coverage":{"scope_anchor_ref":v.anchors[0]["anchor_ref"],"coverage_posture":"exhaustive_nested","excluded_anchor_refs":[],"unreported_gaps_allowed":false,"overlap_requires_declaration":true,"source_reconstruction_required":true},"status":"proposed","status_reason":"PDF pages are exact, while midpoint panels, Poppler blocks, generated separators, region spans, and heading candidates remain unreviewed method results.","maker":method,"competing_segmentation_refs":[],"review_refs":[],"declared_uses":["navigation","source_observation","interchange"],"source_text_authority":false,"linguistic_authority":false,"semantic_authority":false}],
        "reviews":[],"projections":[{"projection_id":fixed["projection_id"],"projection_kind":"other","source_segmentation_refs":[fixed["segmentation_id"]],"artifact_ref":plan["outputs"]["citation_spine_ref"],"artifact_sha256":citation_digest,"admission_posture":"proposal_preserving","preserves_unit_ids":true,"preserves_status":true,"preserves_source_return":true,"runtime_authority":false,"source_text_authority":false,"linguistic_authority":false,"semantic_authority":false,"visibility":"local_only"}],
        "rights_and_visibility":{"source_visibility":"local_only","packet_visibility":"public_metadata_only","effective_visibility":"local_only","rights_record_refs":[src["rights_ref"]],"private_source_used":true,"publication_authorized":false,"inheritance_policy":"most-restrictive-source-packet-and-destination-wins"},
        "authority_boundary":{"tree_role":"orientation","graph_role":"relation","source_role":"authority","validators_prove_mechanics_not_truth":true,"segmentation_is_method_result_not_source_truth":true,"source_unit_is_not_lexeme_sign_or_concept":true,"model_token_is_not_linguistic_token":true,"projection_is_owner_truth":false,"legacy_bulk_migration_authorized":false}});
    validation::schema(ctx, &p)?;
    Ok(p)
}
fn summary(doc: &Document, plan: &Value, v: &Views) -> Value {
    let (blocks, lines, words, _) = doc.counts(&(0..doc.blocks.len()).collect::<Vec<_>>());
    json!({"schema_version":"tos_zarathustra_antonovsky_technical_markup_summary_v1","language":"ru","item_ref":plan["source_item"]["item_ref"],"source_file_sha256":plan["source_item"]["file_sha256"],"bbox_sha256":sha(&doc.bbox),"pdftotext_version":"26.01.0","warning_count":2601,"private_text_layer_sha256":doc.text_sha,"private_text_layer_code_points":doc.offsets.len()-1,"pdf_page_count":doc.pages.len(),"observed_panel_count":doc.panels.len(),"poppler_flow_count":doc.flow_counts.iter().sum::<usize>(),"layout_block_paragraph_candidate_count":blocks,"physical_line_counted_not_unitized":lines,"embedded_word_counted_not_unitized":words,"tracked_unit_count":1+doc.pages.len()+doc.panels.len()+blocks,"region_candidate_count":v.regions.len(),"heading_candidate_count":v.headings.len(),"heading_candidate_coverage_posture":plan["heading_candidate_rule"]["coverage_posture"],"regions":v.regions,"segmentation_status":"proposed","human_review_status":"unreviewed","accepted_russian_text":false,"accepted_paragraphs":false,"accepted_sections":false,"source_target_alignment_created":false,"german_numbering_copied":false,"semantic_fields_materialized":false,"source_text_included":false,"publication_authorized":false,"authority_boundary":plan["authority_boundary"]})
}
fn output_rows(tracked: &BTreeMap<String, Vec<u8>>, role: &str) -> Vec<Value> {
    tracked
        .iter()
        .map(|(reference, payload)| json!({"ref":reference,"role":role,"sha256":sha(payload)}))
        .collect()
}
fn bound(ctx: &ResearchExecution, reference: &str, role: &str) -> Result<Value> {
    let mut b = binding(ctx, reference)?;
    b["role"] = json!(role);
    Ok(b)
}
fn provenance(
    ctx: &ResearchExecution,
    doc: &Document,
    plan: &Value,
    tracked: &BTreeMap<String, Vec<u8>>,
    builder_sha: &str,
) -> Result<Value> {
    let source = &plan["source_item"];
    let outputs = output_rows(tracked, "tracked-text-free-technical-markup-artifact");
    Ok(
        json!({"schema_version":"tos_provenance_event_v1","event_id":EVENT,"event_type":"segmentation","started_at":plan["created_at"],"ended_at":plan["created_at"],"agent_refs":["software:tos-antonovsky-1911-technical-markup-builder","model:codex"],
        "inputs":[bound(ctx,&plan_ref(),"tracked-technical-markup-plan")?,bound(ctx,s(&plan["identity_policy"]["issuance_ref"]),"tracked-opaque-identity-issuance")?,{"ref":source["file_ref"],"role":"local-fixity-bound-antonovsky-1911-pdf","sha256":source["file_sha256"]},bound(ctx,s(&source["resource_inventory_ref"]),"tracked-text-free-PDF-page-resource-inventory")?,bound(ctx,s(&source["rights_ref"]),"tracked-layered-rights-record")?],
        "outputs":outputs,"method":{"maker_type":"software","name":"exact-antonovsky-1911-poppler-bbox-layout-technical-proposal","version":"1","artifact_digest":builder_sha,"runtime":null,"device":"cpu","configuration":{"pdf_pages":doc.pages.len(),"source_text_tracked":false,"opaque_identity_issuance_reused":true,"page_boundary_status":"source_attested","panel_block_region_heading_status":"proposed","human_review_status":"unreviewed","source_target_alignment_created":false,"semantic_fields_materialized":false},"prompt_or_instruction_ref":plan_ref()},
        "status":"completed_with_warnings","warnings":["Poppler layout blocks are paragraph candidates, not accepted Russian paragraphs.","The typographic heading set is deliberately incomplete and unreviewed.","The six region spans are witness-local page-side candidates and create no German correspondence.","The exact PDF, bbox observation, and sequential embedded-text layer remain local-only mode-0600 files.","The operation creates a technical markup proposal. Correction, translation, semantic assessment, graph, canon, rights, publication and transfer follow their corresponding owner routes."],"receipt_refs":outputs.iter().map(|r|&r["ref"]).collect::<Vec<_>>(),"rights_basis_ref":source["rights_ref"],"event_version":1,"supersedes_event_ref":null}),
    )
}
fn screening_provenance(
    ctx: &ResearchExecution,
    plan: &Value,
    screening: &Value,
    base: &BTreeMap<String, Vec<u8>>,
    screen: &BTreeMap<String, Vec<u8>>,
    builder_sha: &str,
) -> Result<Value> {
    let outputs = output_rows(screen, "tracked-text-free-model-screening-projection");
    let heading = out(plan, "heading_candidates_ref")?;
    let citation = out(plan, "citation_spine_ref")?;
    Ok(
        json!({"schema_version":"tos_provenance_event_v1","event_id":SCREEN_EVENT,"event_type":"annotation","started_at":screening["created_at"],"ended_at":screening["created_at"],"agent_refs":["model:codex"],"inputs":[bound(ctx,&screening_ref(),"tracked-text-free-technical-screening-plan")?,{"ref":heading,"role":"tracked-text-free-heading-candidate-set","sha256":sha(&base[heading])},{"ref":citation,"role":"tracked-text-free-source-block-citation-spine","sha256":sha(&base[citation])},{"ref":plan["source_item"]["file_ref"],"role":"local-source-visible-exact-PDF","sha256":plan["source_item"]["file_sha256"]}],"outputs":outputs,"method":{"maker_type":"model_source_visible","name":"antonovsky-1911-heading-and-indent-boundary-screening","version":"1","artifact_digest":builder_sha,"runtime":null,"device":"cpu","configuration":{"heading_candidate_coverage":"all_enumerated_candidates","split_sample_block_count":screening["paragraph_split_screening"]["source_visible_sample"]["sample_block_count"],"packet_review_created":false,"real_human_reviewer":false,"source_target_alignment_created":false,"semantic_fields_materialized":false},"prompt_or_instruction_ref":screening_ref()},"status":"completed_with_warnings","warnings":["The source-visible screening records model authorship; real-human text-unit review remains outstanding.","Heading roles and section boundaries retain their technical-candidate status.","Paragraph spans remain an overlay over unchanged Poppler block units and issue no successor identities.","The split screening covers the selected 42-block sample; remaining boundaries retain their prior review status.","Translation alignment, semantics, graph assessment, canon, publication and transfer retain their corresponding owner decision routes."],"receipt_refs":screen.keys().collect::<Vec<_>>(),"rights_basis_ref":plan["source_item"]["rights_ref"],"event_version":1,"supersedes_event_ref":null}),
    )
}
pub(super) struct Artifacts {
    pub tracked: BTreeMap<String, Vec<u8>>,
    pub private: BTreeMap<String, Vec<u8>>,
}
pub(super) fn build(
    ctx: &ResearchExecution,
    doc: &Document,
    plan: &Value,
    screening: &Value,
    issuance: &Value,
) -> Result<Artifacts> {
    let v = views::build(ctx, doc, plan, issuance)?;
    let citations = jsonl(ctx, &v.citations)?;
    let p = packet(ctx, doc, plan, issuance, &v, &sha(&citations))?;
    let mut tracked = BTreeMap::new();
    tracked.insert(out(plan, "packet_ref")?.into(), pretty(ctx, &p)?);
    tracked.insert(out(plan, "citation_spine_ref")?.into(), citations);
    tracked.insert(
        out(plan, "region_candidates_ref")?.into(),
        jsonl(ctx, &v.regions)?,
    );
    tracked.insert(
        out(plan, "heading_candidates_ref")?.into(),
        jsonl(ctx, &v.headings)?,
    );
    tracked.insert(
        out(plan, "summary_ref")?.into(),
        pretty(ctx, &summary(doc, plan, &v))?,
    );
    // Historical builder bytes bind the retained source event only; they are
    // never loaded as code or passed to a Python process by this producer.
    let builder_sha = sha(&recipe_bytes(ctx)?);
    ensure(
        builder_sha == BUILDER_SHA,
        "producer source binding changed during operation",
    )?;
    let event = provenance(ctx, doc, plan, &tracked, &builder_sha)?;
    let (headings, overlay, summary) = views::screen(ctx, doc, plan, screening, &v)?;
    let mut screen = BTreeMap::new();
    screen.insert(
        out(plan, "heading_screening_ref")?.into(),
        jsonl(ctx, &headings)?,
    );
    screen.insert(
        out(plan, "paragraph_segment_overlay_ref")?.into(),
        jsonl(ctx, &overlay)?,
    );
    screen.insert(
        out(plan, "screening_summary_ref")?.into(),
        pretty(ctx, &summary)?,
    );
    let screen_event = screening_provenance(ctx, plan, screening, &tracked, &screen, &builder_sha)?;
    tracked.extend(screen);
    tracked.insert(
        out(plan, "provenance_ref")?.into(),
        jsonl(ctx, &[event, screen_event])?,
    );
    let private = BTreeMap::from([
        (
            out(plan, "private_text_layer_ref")?.into(),
            doc.text.as_bytes().to_vec(),
        ),
        (out(plan, "private_bbox_ref")?.into(), doc.bbox.clone()),
    ]);
    ctx.check()?;
    Ok(Artifacts { tracked, private })
}
