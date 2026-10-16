//! Ordered record shapes from the retained Mysl structural recipe.
use super::*;
pub(super) fn map(d: &Data) -> Result<Out> {
    Ok(o(vec![
        (
            "$schema",
            q(
                "https://tree-of-sophia.local/ToS/contracts/hierarchical-target-numbered-unit-page-map.schema.json",
            ),
        ),
        (
            "schema_version",
            q("tos_hierarchical_target_numbered_unit_page_map_v1"),
        ),
        ("map_id", v(&d.config["map_id"])?),
        ("work_ref", v(&d.config["work_ref"])?),
        ("expression_ref", v(&d.config["expression_ref"])?),
        ("edition_ref", v(&d.manifest["embodiment_ref"])?),
        ("item_ref", v(&d.manifest["item_id"])?),
        (
            "scan_file",
            o(vec![
                ("file_ref", v(&d.pdf_entry["file_id"])?),
                ("file_sha256", v(&d.pdf_entry["sha256"])?),
                ("inventory_profile", q("pdf_pages_v1")),
            ]),
        ),
        (
            "inventory",
            o(vec![
                ("ref", q(INVENTORY_PATH)),
                ("sha256", q(d.inventory_digest)),
            ]),
        ),
        (
            "work_boundary",
            o(vec![
                ("ref", q(WORK_BOUNDARY_PATH)),
                ("sha256", q(d.boundary_digest)),
                ("member_sequence", v(&d.config["member_sequence"])?),
                ("start_page", v(&d.config["start_page"])?),
                ("end_page", v(&d.config["end_page"])?),
                ("epistemic_status", v(&d.member["epistemic_status"])?),
                ("review_status", v(&d.member["review_status"])?),
            ]),
        ),
        (
            "map_authority",
            q("machine_candidates_with_bounded_model_gap_review_only"),
        ),
        ("target_text_included", Out::Bool(false)),
        (
            "method",
            o(vec![
                (
                    "name",
                    q(
                        "series-scoped-ordered-embedded-pdf-bbox-candidate-plus-source-visible-gap-review",
                    ),
                ),
                ("version", q("1")),
                ("maker_type", q("mixed")),
                ("local_payloads_read", Out::Bool(true)),
                (
                    "navigation_tool",
                    o(vec![
                        ("name", q("pdftotext")),
                        ("version", q(PDFTOTEXT_VERSION)),
                        ("mode", q("bbox-layout")),
                    ]),
                ),
                (
                    "candidate_filter",
                    o(vec![
                        ("single_word_line_only", Out::Bool(true)),
                        (
                            "x_min_points",
                            o(vec![("minimum", n(145)), ("maximum", n(175))]),
                        ),
                        (
                            "y_min_points",
                            o(vec![("minimum", n(40)), ("maximum", n(490))]),
                        ),
                        ("maximum_stripped_character_count", n(3)),
                        (
                            "accepted_pattern",
                            q("positive integer with optional terminal period"),
                        ),
                        (
                            "normalizations",
                            Out::Array(vec![
                                q("trim surrounding whitespace"),
                                q("remove one terminal period"),
                            ]),
                        ),
                    ]),
                ),
                ("series_trials", Out::Array(d.series_trials.to_vec())),
                (
                    "source_visible_review",
                    o(vec![
                        ("maker_type", q("model")),
                        ("human_repeat_performed", Out::Bool(false)),
                        (
                            "reviewed_target_pdf_pages",
                            Out::Array(d.reviewed_pages.iter().map(|v| n(*v)).collect()),
                        ),
                        (
                            "override_unit_refs",
                            Out::Array(d.override_unit_refs.iter().map(q).collect()),
                        ),
                    ]),
                ),
                ("cross_lingual_text_matching_used", Out::Bool(false)),
                ("semantic_matching_used", Out::Bool(false)),
                ("no_target_text_emitted", Out::Bool(true)),
            ]),
        ),
        ("series", Out::Array(d.series_payloads.to_vec())),
        (
            "summary",
            o(vec![
                ("series_count", n(d.series_payloads.len() as u64)),
                ("numbered_unit_count", n(d.total_units as u64)),
                (
                    "ordered_bbox_candidate_match_count",
                    n(d.total_machine_matches as u64),
                ),
                (
                    "source_visible_override_unit_count",
                    n(d.override_unit_refs.len() as u64),
                ),
                (
                    "source_visible_reviewed_page_count",
                    n(d.reviewed_pages.len() as u64),
                ),
                (
                    "exact_start_page_candidates_materialized",
                    n(d.total_units as u64),
                ),
                ("unresolved_unit_count", n(0)),
                ("start_pages_monotonic", Out::Bool(true)),
                ("all_anchor_statuses", Out::Array(vec![q("proposed")])),
                ("human_review_performed", Out::Bool(false)),
            ]),
        ),
        ("provenance_ref", q(&d.paths["map_provenance"])),
        ("provenance_event_ref", v(&d.config["map_event_id"])?),
        ("map_version", n(1)),
        ("supersedes_map_ref", Out::Null),
        (
            "authority_boundary",
            q(
                "This map records machine-derived, series-qualified numbered-label start-page candidates, model review of explicit gaps and proposed whole-page addresses in one exact translation scan. Series identity distinguishes repeated numeral labels.",
            ),
        ),
        (
            "does_not_establish",
            Out::Array(vec![
                q("target_text"),
                q("exact_line_boundaries"),
                q("exact_passage_end_boundaries"),
                q("accepted_translation_text"),
                q("source_to_target_passage_alignment"),
                q("translation_correspondence"),
                q("translation_equivalence"),
                q("translation_quality"),
                q("textual_identity"),
                q("semantics"),
                q("rights_clearance"),
                q("eligible_target_unit"),
                q("target_gold"),
                q("canon_promotion"),
            ]),
        ),
    ]))
}
pub(super) fn map_event(d: &Data) -> Result<Out> {
    Ok(o(vec![
        ("schema_version", q("tos_provenance_event_v1")),
        ("event_id", v(&d.config["map_event_id"])?),
        ("event_type", q("segmentation")),
        ("started_at", q(d.event_at)),
        ("ended_at", q(d.event_at)),
        (
            "agent_refs",
            Out::Array(vec![
                q("model:codex"),
                q("software:python-standard-library"),
                q(format!("software:poppler-{}", PDFTOTEXT_VERSION)),
            ]),
        ),
        (
            "inputs",
            Out::Array(vec![
                o(vec![
                    ("ref", v(&d.pdf_entry["file_id"])?),
                    ("role", q("target-visible-local-translation-scan-witness")),
                    ("sha256", v(&d.pdf_entry["sha256"])?),
                ]),
                o(vec![
                    ("ref", q(INVENTORY_PATH)),
                    ("role", q("tracked-text-free-target-resource-inventory")),
                    ("sha256", q(d.inventory_digest)),
                ]),
                o(vec![
                    ("ref", q(WORK_BOUNDARY_PATH)),
                    ("role", q("tracked-target-work-boundary")),
                    ("sha256", q(d.boundary_digest)),
                ]),
                o(vec![
                    ("ref", q(RIGHTS_PATH)),
                    ("role", q("target-rights-basis")),
                    ("sha256", q(d.rights_digest)),
                ]),
            ]),
        ),
        (
            "outputs",
            Out::Array(vec![
                o(vec![
                    ("ref", q(&d.paths["map"])),
                    (
                        "role",
                        q("tracked-text-free-hierarchical-target-numbered-unit-page-map"),
                    ),
                    ("sha256", q(d.map_digest)),
                ]),
                o(vec![
                    ("ref", q(&d.paths["anchors"])),
                    (
                        "role",
                        q("tracked-proposed-whole-page-hierarchical-target-anchors"),
                    ),
                    ("sha256", q(d.anchor_digest)),
                ]),
            ]),
        ),
        (
            "method",
            o(vec![
                ("maker_type", q("mixed")),
                (
                    "name",
                    q(
                        "series-scoped-ordered-embedded-pdf-bbox-candidate-plus-source-visible-gap-review",
                    ),
                ),
                ("version", q("1")),
                ("artifact_digest", Out::Null),
                (
                    "runtime",
                    q(format!(
                        "Python standard library XML parser; Poppler pdftotext {}",
                        PDFTOTEXT_VERSION
                    )),
                ),
                ("device", q("abyss-machine")),
                (
                    "configuration",
                    o(vec![
                        (
                            "work_page_range",
                            Out::Array(vec![
                                v(&d.config["start_page"])?,
                                v(&d.config["end_page"])?,
                            ]),
                        ),
                        ("series_count", n(d.series_payloads.len() as u64)),
                        ("numbered_unit_count", n(d.total_units as u64)),
                        (
                            "ordered_bbox_candidate_match_count",
                            n(d.total_machine_matches as u64),
                        ),
                        (
                            "source_visible_override_unit_count",
                            n(d.override_unit_refs.len() as u64),
                        ),
                        (
                            "source_visible_reviewed_page_count",
                            n(d.reviewed_pages.len() as u64),
                        ),
                        ("human_repeat_performed", Out::Bool(false)),
                        ("target_text_included", Out::Bool(false)),
                        ("cross_lingual_text_matching_used", Out::Bool(false)),
                    ]),
                ),
                (
                    "prompt_or_instruction_ref",
                    q("ToS/doctrine/CORPUS_FOUNDATION.md#address-law"),
                ),
            ]),
        ),
        ("status", q("completed_with_warnings")),
        (
            "warnings",
            Out::Array(vec![
                q(format!(
                    "All {} addresses are proposed series-qualified target numbered-label start-page candidates, not exact line or passage-end boundaries.",
                    d.total_units
                )),
                q(
                    "The embedded PDF layer supplied disposable navigation only and was not accepted as Russian text.",
                ),
                q(format!(
                    "The model visibly checked {} pages for {} OCR-gap labels; no human repeat exists.",
                    d.reviewed_pages.len(),
                    d.override_unit_refs.len()
                )),
                q(
                    "Repeated numeral labels are disambiguated only by proposed series identity; no German parallel unit map was created.",
                ),
                q(
                    "This event records target structure. Alignment, translation, semantics, eligibility, target gold and publication retain their existing states.",
                ),
            ]),
        ),
        (
            "receipt_refs",
            Out::Array(vec![q(&d.paths["map"]), q(&d.paths["anchors"])]),
        ),
        ("rights_basis_ref", q(RIGHTS_PATH)),
        ("event_version", n(1)),
        ("supersedes_event_ref", Out::Null),
    ]))
}
pub(super) fn crosswalk(d: &Data) -> Result<Out> {
    Ok(o(vec![
        (
            "$schema",
            q(
                "https://tree-of-sophia.local/ToS/contracts/transfer-candidate-target-structural-crosswalk.schema.json",
            ),
        ),
        (
            "schema_version",
            q("tos_transfer_candidate_target_structural_crosswalk_v1"),
        ),
        ("crosswalk_id", v(&d.config["crosswalk_id"])?),
        ("status", q("prepared-target-structural-only-ineligible")),
        ("work_ref", v(&d.config["work_ref"])?),
        ("target_expression_ref", v(&d.config["expression_ref"])?),
        ("target_item_ref", v(&d.manifest["item_id"])?),
        (
            "inputs",
            o(vec![
                (
                    "transfer_plan",
                    o(vec![
                        ("ref", q(TRANSFER_PLAN_PATH)),
                        ("sha256", q(d.plan_digest)),
                    ]),
                ),
                (
                    "candidate_anchor_set",
                    o(vec![
                        ("ref", q(TRANSFER_ANCHOR_PATH)),
                        ("sha256", q(d.transfer_anchors_digest)),
                    ]),
                ),
                (
                    "target_hierarchical_numbered_unit_map",
                    o(vec![
                        ("ref", q(&d.paths["map"])),
                        ("sha256", q(d.map_digest)),
                    ]),
                ),
                (
                    "target_rights",
                    o(vec![
                        ("ref", q(RIGHTS_PATH)),
                        ("sha256", q(d.rights_digest)),
                    ]),
                ),
            ]),
        ),
        (
            "method",
            o(vec![
                (
                    "name",
                    q("tracked-target-page-to-hierarchical-proposed-unit-start-crosswalk"),
                ),
                ("version", q("1")),
                ("selection_frame_changed", Out::Bool(false)),
                ("source_or_target_text_read", Out::Bool(false)),
                (
                    "page_intersection_rule",
                    q(
                        "preceding proposed target unit start plus every proposed target unit start on the candidate page",
                    ),
                ),
                ("source_parallel_unit_map_available", Out::Bool(false)),
            ]),
        ),
        (
            "summary",
            o(vec![
                ("candidate_page_count", n(d.candidates.len() as u64)),
                ("random_page_count", n(d.random_pages as u64)),
                ("hard_page_count", n(d.hard_pages as u64)),
                ("page_with_unit_start_count", n(d.pages_with_starts as u64)),
                (
                    "page_without_unit_start_count",
                    n((d.candidates.len() - d.pages_with_starts) as u64),
                ),
                (
                    "possible_target_unit_route_count",
                    n(d.possible_routes as u64),
                ),
                ("source_parallel_route_count", n(0)),
                ("human_review_count", n(0)),
                ("eligible_target_unit_count", n(0)),
                ("target_gold_count", n(0)),
            ]),
        ),
        ("candidates", Out::Array(d.crosswalk_candidates.to_vec())),
        ("source_text_included", Out::Bool(false)),
        ("target_text_included", Out::Bool(false)),
        (
            "effects",
            o(vec![
                ("candidate_frame_changed", Out::Bool(false)),
                ("exact_passage_boundary_created", Out::Bool(false)),
                ("source_text_accepted", Out::Bool(false)),
                ("target_text_accepted", Out::Bool(false)),
                ("source_to_target_alignment_created", Out::Bool(false)),
                ("translation_alignment_created", Out::Bool(false)),
                ("target_unit_eligible", Out::Bool(false)),
                ("target_gold_created", Out::Bool(false)),
                ("semantic_work_opened", Out::Bool(false)),
                ("human_work_scheduled", Out::Bool(false)),
            ]),
        ),
        ("provenance_event_ref", v(&d.config["crosswalk_event_id"])?),
        (
            "does_not_establish",
            Out::Array(vec![
                q("source_text"),
                q("target_text"),
                q("exact_line_boundaries"),
                q("exact_passage_end_boundaries"),
                q("source_parallel_unit_route"),
                q("source_to_target_passage_alignment"),
                q("translation_correspondence"),
                q("translation_equivalence"),
                q("translation_quality"),
                q("textual_identity"),
                q("accepted_german"),
                q("accepted_russian"),
                q("target_gold"),
                q("eligible_target_unit"),
                q("semantics"),
                q("rights_clearance"),
                q("canon_promotion"),
            ]),
        ),
        (
            "authority_boundary",
            q(
                "This crosswalk narrows frozen whole-page candidates through a proposed hierarchical target numbered-unit map. Its scope is target-side structural routing; the German parallel map remains unprepared.",
            ),
        ),
    ]))
}
pub(super) fn crosswalk_event(d: &Data) -> Result<Out> {
    Ok(o(vec![
        ("schema_version", q("tos_provenance_event_v1")),
        ("event_id", v(&d.config["crosswalk_event_id"])?),
        ("event_type", q("alignment")),
        ("started_at", q(d.event_at)),
        ("ended_at", q(d.event_at)),
        (
            "agent_refs",
            Out::Array(vec![q("software:python-standard-library")]),
        ),
        ("inputs", Out::Array(d.crosswalk_inputs.to_vec())),
        (
            "outputs",
            Out::Array(vec![o(vec![
                ("ref", q(&d.paths["crosswalk"])),
                (
                    "role",
                    q("tracked-text-free-target-only-transfer-candidate-crosswalk"),
                ),
                ("sha256", q(d.crosswalk_digest)),
            ])]),
        ),
        (
            "method",
            o(vec![
                ("maker_type", q("software")),
                ("name", q("tracked-target-hierarchical-page-crosswalk")),
                ("version", q("1")),
                ("artifact_digest", Out::Null),
                ("runtime", q("Python standard library")),
                ("device", Out::Null),
                (
                    "configuration",
                    o(vec![
                        ("candidate_page_count", n(d.candidates.len() as u64)),
                        (
                            "possible_target_unit_route_count",
                            n(d.possible_routes as u64),
                        ),
                        (
                            "page_with_proposed_unit_starts_count",
                            n(d.pages_with_starts as u64),
                        ),
                        (
                            "page_without_proposed_unit_starts_count",
                            n((d.candidates.len() - d.pages_with_starts) as u64),
                        ),
                        ("local_payloads_read", Out::Bool(false)),
                        ("source_text_read", Out::Bool(false)),
                        ("target_text_read", Out::Bool(false)),
                        ("selection_frame_changed", Out::Bool(false)),
                        ("source_parallel_unit_map_available", Out::Bool(false)),
                        ("source_parallel_route_count", n(0)),
                        (
                            "exact_passage_end_boundaries_materialized",
                            Out::Bool(false),
                        ),
                        ("translation_alignment_inferred", Out::Bool(false)),
                        ("eligible_target_unit_count", n(0)),
                        ("target_gold_count", n(0)),
                        ("human_review_count", n(0)),
                    ]),
                ),
                (
                    "prompt_or_instruction_ref",
                    q("ToS/doctrine/CORPUS_FOUNDATION.md#address-law"),
                ),
            ]),
        ),
        ("status", q("completed_with_warnings")),
        (
            "warnings",
            Out::Array(vec![
                q(format!(
                    "The crosswalk narrows only six frozen {} target pages to {} possible target-side series-qualified unit routes.",
                    s(&d.config["slug"])?,
                    d.possible_routes
                )),
                q(
                    "No German parallel numbered-unit map exists in this route; source parallel route count is explicitly zero.",
                ),
                q(
                    "No source or target text was read, compared, transcribed, aligned, or accepted by the crosswalk event.",
                ),
                q(
                    "These proposals locate target starts. Passage ends, translation correspondence and quality, semantics, rights, eligibility, target gold and canon each require their own recorded assessment or authorization.",
                ),
                q("No human work was requested or scheduled by this event."),
            ]),
        ),
        ("receipt_refs", Out::Array(vec![q(&d.paths["crosswalk"])])),
        ("rights_basis_ref", Out::Null),
        ("event_version", n(1)),
        ("supersedes_event_ref", Out::Null),
    ]))
}
