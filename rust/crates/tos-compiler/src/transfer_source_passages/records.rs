//! Source-owned transfer source record shapes.
use super::*;
pub(super) fn private(
    source_candidate_id: &str,
    candidate: &Value,
    expression_ref: &str,
    content_witness: &Value,
    boundary_evidence: &Value,
    layer: &str,
    start: &Value,
    end_exclusive: &Value,
    private_records: &[Value],
    automatic_text: &str,
) -> Value {
    json!({
    "schema_version": "tos_private_transfer_source_passage_candidate_v1",
    "source_passage_candidate_id": source_candidate_id,
    "target_passage_candidate_id": candidate["passage_candidate_id"],
    "qualified_unit_key": candidate["qualified_unit_key"],
    "source_expression_ref": expression_ref,
    "content_witness": content_witness,
    "boundary_evidence": boundary_evidence,
    "boundary_layer": layer,
    "start": start,
    "end_exclusive": end_exclusive,
    "records": private_records,
    "automatic_candidate_text": automatic_text,
    "authority_boundary": "A private exact slice of the named automatic source layer, retaining its source, rights and proposed-candidate status."
    })
}
pub(super) fn candidate(
    source_candidate_id: &str,
    candidate: &Value,
    work_ref: &str,
    expression_ref: &str,
    passage_ref: &str,
    layer: &str,
    start: &Value,
    end_exclusive: &Value,
    navigation_span: &[usize],
    address_span: &[usize],
    content_witness: &Value,
    address_witness: &Value,
    boundary_evidence: &Value,
    relation: &str,
    source_anchor_id: &str,
    private_ref: &str,
    private_bytes: &[u8],
    automatic_text: &str,
    selected_count: usize,
    word_count: usize,
) -> Value {
    json!({
    "source_passage_candidate_id": source_candidate_id,
    "target_passage_candidate_id": candidate["passage_candidate_id"],
    "frozen_page_candidate_id": candidate["frozen_page_candidate_id"],
    "qualified_unit_key": candidate["qualified_unit_key"],
    "work_ref": work_ref,
    "source_expression_ref": expression_ref,
    "source_passage_ref": passage_ref,
    "source_structural_anchor_ref": candidate["source_structural_anchor_ref"],
    "status": "materialized-layer-exact-candidate",
    "boundary_layer": layer,
    "start": start,
    "end_exclusive": end_exclusive,
    "navigation_page_span": navigation_span,
    "address_page_span": address_span,
    "content_witness": content_witness,
    "address_witness": address_witness,
    "boundary_evidence": boundary_evidence,
    "navigation_relation": relation,
    "source_passage_anchor_ref": source_anchor_id,
    "private_content_ref": private_ref,
    "private_content_sha256": sha(private_bytes),
    "private_content_bytes": private_bytes.len(),
    "text_character_count": automatic_text.chars().count(),
    "record_count": selected_count,
    "word_count": word_count,
    "unresolved_boundaries": [],
    "human_review_performed": false,
    "accepted_source_text": false,
    "source_to_target_alignment_created": false,
    "eligible_for_variant_execution": false,
    "limitations": ["the boundary is exact only inside the named automatic or model-visible-marker-supported source layer", "The private slice reproduces the named automatic German layer and retains its unreviewed candidate status.", "Matching numbers and paired structural starts supply a route for passage and translation alignment review.", "the candidate remains ineligible and has no target gold or human review", "private source text is local-only and not authorized for publication"]
    })
}
pub(super) fn anchor(
    source_anchor_id: &str,
    content_witness: &Value,
    passage_ref: &str,
    selectors: &[Value],
    output_ref: &str,
    event_id: &str,
) -> Value {
    json!({
    "schema_version": "tos_source_anchor_v1",
    "anchor_id": source_anchor_id,
    "item_id": content_witness["item_ref"],
    "file_id": content_witness["file_ref"],
    "file_sha256": content_witness["file_sha256"],
    "passage_id": passage_ref,
    "selectors": selectors,
    "selector_method": {
    "maker_type": "mixed",
    "method": "numbered label or source-visible marker to next boundary slice",
    "version": "3",
    "configuration_ref": output_ref
    },
    "status": "proposed",
    "provenance_event_ref": event_id,
    "anchor_version": 1,
    "supersedes_anchor_ref": null,
    "review_ref": null
    })
}
pub(super) fn output(
    set_id: &str,
    target_digest: &str,
    input_refs: &[Value],
    results: &[Value],
    materialized_count: usize,
    unresolved_count: usize,
    event_id: &str,
) -> Value {
    json!({
    "$schema": SCHEMA_REF,
    "schema_version": "tos_transfer_source_passage_candidate_set_v1",
    "candidate_set_id": set_id,
    "target_passage_candidate_set_ref": TARGET_CANDIDATE_PATH,
    "target_passage_candidate_set_sha256": target_digest,
    "inputs": input_refs,
    "method": {
    "name": "numbered-label-or-source-visible-marker-to-next-boundary-slice",
    "version": "3",
    "maker_type": "mixed",
    "layers": ["abbyy-xml-paragraph", "djvu-xml-line", "jp2-visible-marker-plus-djvu-xml-line", "pdf-visible-marker-plus-poppler-pdf-bbox-line", "poppler-pdf-bbox-line"],
    "local_payloads_read": true,
    "private_content_written": true,
    "human_review_performed": false
    },
    "passage_candidates": results,
    "summary": {
    "conservative_source_route_count": 35,
    "materialized_source_passage_candidate_count": materialized_count,
    "unresolved_source_boundary_count": unresolved_count,
    "accepted_source_passage_count": 0,
    "source_to_target_alignment_count": 0,
    "eligible_target_unit_count": 0,
    "target_gold_count": 0,
    "human_review_count": 0
    },
    "effects": {
    "candidate_frame_changed": false,
    "private_source_text_materialized": true,
    "tracked_source_text_created": false,
    "layer_exact_source_boundary_candidates_created": true,
    "accepted_source_text_created": false,
    "source_to_target_passage_alignment_created": false,
    "translation_alignment_created": false,
    "target_unit_eligible": false,
    "target_gold_created": false,
    "semantic_work_opened": false,
    "human_work_scheduled": false,
    "canon_effect": false
    },
    "provenance_event_ref": event_id,
    "status": "prepared-complete-ineligible",
    "authority_boundary": "This record covers thirty-five private source-passage candidates bound to their named automatic or model-visible-marker-supported layers. Tracked data retains text-free provenance and each candidate's proposed status.",
    "does_not_establish": ["diplomatic_transcription", "accepted_german", "critical_text", "source_to_target_passage_alignment", "translation_correspondence", "translation_equivalence", "translation_quality", "eligible_target_unit", "target_gold", "human_review", "semantic_relation", "rights_clearance", "publication_permission", "canon_promotion"]
    })
}
pub(super) fn event(
    event_id: &str,
    observed_at: &str,
    version: &str,
    input_refs: &[Value],
    output_entities: &[Value],
    builder_digest: &str,
    materialized_count: usize,
    unresolved_count: usize,
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
    "name": "numbered-label-or-source-visible-marker-to-next-boundary-slice",
    "version": "3",
    "artifact_digest": builder_digest,
    "runtime": "Python standard library plus pdftotext 26.01.0",
    "device": "abyss-machine",
    "configuration": {
    "conservative_source_routes": 35,
    "materialized_source_passage_candidates": materialized_count,
    "unresolved_source_boundaries": unresolved_count,
    "private_content_files": private_count,
    "source_visible_abbyy_marker_overrides": 1usize,
    "model_visible_jp2_marker_returns": 3usize,
    "model_visible_pdf_marker_returns": 2usize,
    "tracked_text_created": false,
    "human_review_count": 0,
    "accepted_source_passages": 0,
    "source_to_target_alignments": 0,
    "eligible_target_units": 0,
    "target_gold_count": 0
    },
    "prompt_or_instruction_ref": "ToS/research-packets/foundation-laboratory-2026-07/GOLDEN_KERNEL_TRANSFER_REPORT.md"
    },
    "status": "completed_with_warnings",
    "warnings": ["Exactness is measured against the named automatic German source layer; diplomatic transcription and source acceptance retain their recorded review status.", "three Antichrist number markers use exact JP2 source-visible return plus the first following DjVuXML line and have no human repeat", "two PDF number markers use exact embedded image-mask return plus the first following Poppler bbox line and have no human repeat", "the Antichrist navigation Item is not asserted textually identical to the address Item", "Shared numbering supplies a structural route; passage and translation alignment retain their recorded status.", "This event records source-passage candidates. Eligibility, gold, human work, semantics, publication and canon retain their existing states."],
    "receipt_refs": [output_ref],
    "rights_basis_ref": null,
    "event_version": 3,
    "supersedes_event_ref": null
    })
}
