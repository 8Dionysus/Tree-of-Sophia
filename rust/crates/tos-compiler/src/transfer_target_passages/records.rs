//! Source-owned transfer target record shapes.
use super::*;
pub(super) fn private(
    passage_candidate_id: &str,
    frozen: &Value,
    qualified_unit_key: &Value,
    start: &Value,
    end_exclusive: &Value,
    private_lines: &[Value],
    automatic_text: &str,
) -> Result<Value> {
    let file_digest = s(&frozen["file_ref"])?
        .strip_prefix("tos.file.sha256.")
        .ok_or("file identity digest prefix")?;
    Ok(json!({
    "schema_version": "tos_private_transfer_target_passage_candidate_v1",
    "passage_candidate_id": passage_candidate_id,
    "frozen_page_candidate_id": frozen["unit_id"],
    "frozen_candidate_page": frozen["page"],
    "work_ref": frozen["work_ref"],
    "expression_ref": frozen["expression_ref"],
    "item_ref": frozen["item_ref"],
    "file_ref": frozen["file_ref"],
    "file_sha256": file_digest,
    "qualified_unit_key": qualified_unit_key,
    "source_layer": "embedded-pdf-bbox-pdftotext-automatic-candidate",
    "start": start,
    "end_exclusive": end_exclusive,
    "lines": private_lines,
    "automatic_candidate_text": automatic_text,
    "authority_boundary": "Private passage slice from the named automatic bbox layer, retained under its recorded rights and proposal status."
    }))
}
pub(super) fn candidate(
    passage_candidate_id: &str,
    frozen: &Value,
    row: &Value,
    unit: &Value,
    anchor_id: &str,
    start: &Value,
    end_exclusive: &Value,
    page_span: &[usize],
    candidate_page_line_count: usize,
    intersects: bool,
    private_ref: &str,
    private_bytes: &[u8],
    automatic_text: &str,
    selected_line_count: usize,
    word_count: usize,
) -> Value {
    json!({
    "passage_candidate_id": passage_candidate_id,
    "frozen_page_candidate_id": frozen["unit_id"],
    "frozen_candidate_page": frozen["page"],
    "stratum": frozen["stratum"],
    "work_ref": frozen["work_ref"],
    "expression_ref": frozen["expression_ref"],
    "qualified_unit_key": row["qualified_unit_key"],
    "passage_ref": unit["passage_ref"],
    "passage_anchor_ref": anchor_id,
    "prior_start_anchor_ref": unit["start"]["anchor_ref"],
    "source_structural_anchor_ref": row["source_anchor_ref"],
    "source_structural_start_page": row["source_start_page"],
    "route_basis_ref": row["route_basis_ref"],
    "start": start,
    "end_exclusive": end_exclusive,
    "page_span": page_span,
    "candidate_page_line_count": candidate_page_line_count,
    "candidate_page_intersects_passage": intersects,
    "private_content_ref": private_ref,
    "private_content_sha256": sha(private_bytes),
    "private_content_bytes": private_bytes.len(),
    "text_character_count": automatic_text.chars().count(),
    "line_count": selected_line_count,
    "word_count": word_count,
    "source_layer": "embedded-pdf-bbox-pdftotext-automatic-candidate",
    "boundary_posture": "layer-exact-proposed-not-diplomatic",
    "status": if intersects { "proposed-intersecting-layer-exact" } else { "rejected-nonintersecting-layer-exact" },
    "human_review_performed": false,
    "accepted_target_text": false,
    "eligible_for_variant_execution": false,
    "target_gold_status": "not_started",
    "limitations": ["the boundary is exact only inside the automatic embedded-PDF bbox layer", "The private slice reproduces the automatic Russian text layer and retains its unreviewed candidate status.", "Shared numbering supplies a source structural route for passage and translation alignment review.", "the candidate remains ineligible and has no target gold or human review", "private target text is local-only and not authorized for publication"]
    })
}
pub(super) fn anchor(
    anchor_id: &str,
    manifest: &Value,
    pdf_entry: &Value,
    unit: &Value,
    selectors: &[Value],
    output_ref: &str,
    intersects: bool,
    event_id: &str,
) -> Value {
    json!({
    "schema_version": "tos_source_anchor_v1",
    "anchor_id": anchor_id,
    "item_id": manifest["item_id"],
    "file_id": pdf_entry["file_id"],
    "file_sha256": pdf_entry["sha256"],
    "passage_id": unit["passage_ref"],
    "selectors": selectors,
    "selector_method": {
    "maker_type": "mixed",
    "method": "expected numbered-unit label to next same-series label in Poppler bbox layer",
    "version": "1",
    "configuration_ref": output_ref
    },
    "status": if intersects { "proposed" } else { "rejected" },
    "provenance_event_ref": event_id,
    "anchor_version": 1,
    "supersedes_anchor_ref": null,
    "review_ref": null
    })
}
pub(super) fn output(
    set_id: &str,
    plan_digest: &str,
    manifest: &Value,
    pdf_entry: &Value,
    rights_digest: &str,
    input_refs: &[Value],
    version: &str,
    candidates: &[Value],
    intersection_count: usize,
    event_id: &str,
) -> Value {
    json!({
    "$schema": SCHEMA_REF,
    "schema_version": "tos_transfer_target_passage_candidate_set_v1",
    "candidate_set_id": set_id,
    "transfer_plan_ref": PLAN_PATH,
    "transfer_plan_sha256": plan_digest,
    "target_witness": {
    "item_ref": manifest["item_id"],
    "file_ref": pdf_entry["file_id"],
    "file_sha256": pdf_entry["sha256"],
    "rights_ref": RIGHTS_PATH,
    "rights_sha256": rights_digest
    },
    "inputs": input_refs,
    "method": {
    "name": "expected-structural-label-to-next-label-bbox-slice",
    "version": "1",
    "maker_type": "mixed",
    "navigation_tool": {
    "name": "pdftotext",
    "version": version,
    "mode": "bbox-layout"
    },
    "selection_law": "include expected numbered-unit label line and following bbox lines until the next same-series expected label, excluding the next label and page footer",
    "local_payloads_read": true,
    "private_content_written": true,
    "human_review_performed": false
    },
    "passage_candidates": candidates,
    "summary": {
    "frozen_page_candidate_count": 20,
    "conservative_structural_route_count": candidates.len(),
    "layer_exact_passage_candidate_count": candidates.len(),
    "candidate_page_intersection_count": intersection_count,
    "nonintersecting_route_count": (candidates.len() - intersection_count),
    "accepted_target_passage_count": 0,
    "eligible_target_unit_count": 0,
    "target_gold_count": 0,
    "human_review_count": 0
    },
    "effects": {
    "candidate_frame_changed": false,
    "private_target_text_materialized": true,
    "tracked_target_text_created": false,
    "layer_exact_target_boundary_candidates_created": true,
    "accepted_target_text_created": false,
    "source_passage_boundary_created": false,
    "source_to_target_passage_alignment_created": false,
    "translation_alignment_created": false,
    "target_unit_eligible": false,
    "target_gold_created": false,
    "semantic_work_opened": false,
    "human_work_scheduled": false,
    "canon_effect": false
    },
    "provenance_event_ref": event_id,
    "status": "prepared-ineligible",
    "authority_boundary": "Private target passage materialization within one fixity-bound automatic embedded-PDF bbox layer. Tracked metadata is text-free and preserves proposed or rejected boundary status.",
    "does_not_establish": ["diplomatic_transcription", "accepted_russian", "source_passage_boundary", "source_to_target_passage_alignment", "translation_correspondence", "translation_equivalence", "translation_quality", "target_gold", "eligible_target_unit", "human_review", "semantic_relation", "rights_clearance", "publication_permission", "canon_promotion"]
    })
}
pub(super) fn event(
    event_id: &str,
    observed_at: &str,
    version: &str,
    input_refs: &[Value],
    output_entities: &[Value],
    builder_digest: &str,
    candidates: &[Value],
    intersection_count: usize,
    private_count: usize,
    output_ref: &str,
) -> Value {
    json!({
    "schema_version": "tos_provenance_event_v1",
    "event_id": event_id,
    "event_type": "segmentation",
    "started_at": observed_at,
    "ended_at": observed_at,
    "agent_refs": ["model:codex", "software:python-standard-library", format!("software:poppler-{}", version)],
    "inputs": input_refs,
    "outputs": output_entities,
    "method": {
    "maker_type": "mixed",
    "name": "expected-structural-label-to-next-label-bbox-slice",
    "version": "1",
    "artifact_digest": builder_digest,
    "runtime": "Python standard library plus pdftotext 26.01.0",
    "device": "abyss-machine",
    "configuration": {
    "frozen_page_candidates": 20,
    "conservative_routes": candidates.len(),
    "layer_exact_passage_candidates": candidates.len(),
    "candidate_page_intersections": intersection_count,
    "nonintersecting_routes": (candidates.len() - intersection_count),
    "private_content_files": private_count,
    "source_visible_marker_overrides": 7usize,
    "tracked_text_created": false,
    "human_review_count": 0,
    "accepted_target_passages": 0,
    "eligible_target_units": 0,
    "target_gold_count": 0
    },
    "prompt_or_instruction_ref": "ToS/research-packets/foundation-laboratory-2026-07/GOLDEN_KERNEL_TRANSFER_REPORT.md"
    },
    "status": "completed_with_warnings",
    "warnings": ["Exactness is measured against the named bbox layer; diplomatic transcription and target-text acceptance retain their recorded review status.", "a conservative page route may be rejected when the bounded unit has no line on that frozen page", "Shared numbering and source starts supply a structural route for passage and translation alignment review.", "private text remains ignored, local-only, and unauthorized for publication", "This event records target-passage candidates. Target gold, eligibility, human work, semantics, transfer execution and canon retain their existing states."],
    "receipt_refs": [output_ref],
    "rights_basis_ref": RIGHTS_PATH,
    "event_version": 1,
    "supersedes_event_ref": null
    })
}
