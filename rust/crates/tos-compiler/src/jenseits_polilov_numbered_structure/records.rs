//! Ordered record shapes ported from the maintained source recipe.
use super::*;
pub(super) fn map(
    manifest: &Value,
    pdf_entry: &Value,
    member: &Value,
    units: &[Out],
    matched: usize,
    inventory_digest: &str,
    boundary_digest: &str,
    source_map_digest: &str,
    map_id: &str,
    event_id: &str,
    provenance_ref: &str,
) -> Result<Out> {
    Ok(o(vec![
        (
            "$schema",
            q(
                "https://tree-of-sophia.local/ToS/contracts/target-numbered-unit-page-map.schema.json",
            ),
        ),
        ("schema_version", q("tos_target_numbered_unit_page_map_v1")),
        ("map_id", q(map_id)),
        (
            "work_ref",
            q("tos.work.friedrich-nietzsche.jenseits-von-gut-und-boese"),
        ),
        (
            "expression_ref",
            q("tos.expression.friedrich-nietzsche.jenseits-von-gut-und-boese.ru-polilov-mysl-1996"),
        ),
        (
            "edition_ref",
            q("tos.edition.friedrich-nietzsche.works-in-two-volumes.moscow-mysl-1996-volume-2"),
        ),
        ("item_ref", q(s(&manifest["item_id"])?)),
        (
            "scan_file",
            o(vec![
                ("file_ref", q(s(&pdf_entry["file_id"])?)),
                ("file_sha256", q(s(&pdf_entry["sha256"])?)),
                ("inventory_profile", q("pdf_pages_v1")),
            ]),
        ),
        (
            "inventory",
            o(vec![
                ("ref", q(INVENTORY_PATH)),
                ("sha256", q(inventory_digest)),
            ]),
        ),
        (
            "work_boundary",
            o(vec![
                ("ref", q(WORK_BOUNDARY_PATH)),
                ("sha256", q(boundary_digest)),
                ("member_sequence", n(2)),
                ("start_page", n(238)),
                ("end_page", n(406)),
                ("epistemic_status", q(s(&member["epistemic_status"])?)),
                ("review_status", q(s(&member["review_status"])?)),
            ]),
        ),
        (
            "map_authority",
            q("model_reviewed_target_structure_candidate_only"),
        ),
        ("source_text_included", Out::Bool(false)),
        (
            "method",
            o(vec![
                (
                    "name",
                    q("ordered-embedded-pdf-bbox-candidate-plus-source-visible-gap-review"),
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
                    "layer_trials",
                    Out::Array(vec![
                        o(vec![
                            ("layer", q("embedded_plain_text")),
                            (
                                "result",
                                q(
                                    "reading order, running heads, and damaged numeral labels prevent a complete deterministic map",
                                ),
                            ),
                            ("selected_role", q("rejected_as_complete_map_source")),
                        ]),
                        o(vec![
                            ("layer", q("embedded_bbox_unordered_candidates")),
                            (
                                "result",
                                q(
                                    "page geometry narrows numeral candidates but admits unrelated numbers without an order constraint",
                                ),
                            ),
                            ("selected_role", q("rejected_without_order_constraint")),
                        ]),
                        o(vec![
                            (
                                "layer",
                                q(
                                    "embedded_bbox_ordered_candidates_plus_source_visible_gap_review",
                                ),
                            ),
                            (
                                "result",
                                q(
                                    "265 ordered candidates plus 33 bounded visible page judgments close the target label sequence",
                                ),
                            ),
                            ("selected_role", q("selected_bounded_structure_route")),
                        ]),
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
                        ("maximum_stripped_character_count", n(5)),
                        (
                            "normalizations",
                            Out::Array(vec![
                                q("trim surrounding whitespace and terminal periods"),
                                q("map Cyrillic а/А to structural suffix a"),
                                q("map the isolated OCR glyph Cyrillic б to candidate 6"),
                            ]),
                        ),
                    ]),
                ),
                ("ordered_bbox_candidate_matches", n(matched as u64)),
                (
                    "source_visible_review",
                    o(vec![
                        ("maker_type", q("model")),
                        ("human_repeat_performed", Out::Bool(false)),
                        (
                            "gap_review_unit_keys",
                            Out::Array(PAGE_OVERRIDES.iter().map(|(k, _)| q(*k)).collect()),
                        ),
                        ("ocr_disambiguation_unit_keys", Out::Array(vec![q("6")])),
                        (
                            "supplemental_label_unit_keys",
                            Out::Array(vec![q("65a"), q("73a")]),
                        ),
                    ]),
                ),
                ("cross_lingual_text_matching_used", Out::Bool(false)),
                ("semantic_matching_used", Out::Bool(false)),
                ("no_source_text_emitted", Out::Bool(true)),
            ]),
        ),
        ("unit_starts", Out::Array(units.to_vec())),
        (
            "numbering_asymmetries",
            Out::Array(vec![o(vec![
                ("source_unit_key", q("237a")),
                ("source_map_ref", q(SOURCE_MAP_PATH)),
                ("source_map_sha256", q(source_map_digest)),
                (
                    "target_state",
                    q("corresponding_prose_present_without_repeated_unit_label"),
                ),
                ("target_numbered_unit_materialized", Out::Bool(false)),
                ("exact_translation_alignment_claimed", Out::Bool(false)),
            ])]),
        ),
        (
            "summary",
            o(vec![
                ("integer_numbered_unit_count", n(296)),
                (
                    "supplemental_numbered_units",
                    Out::Array(vec![q("65a"), q("73a")]),
                ),
                (
                    "source_only_nonmaterialized_numbered_units",
                    Out::Array(vec![q("237a")]),
                ),
                ("numbered_unit_count", n(298)),
                ("exact_start_page_candidates_materialized", n(298)),
                ("unresolved_unit_count", n(0)),
                ("start_pages_monotonic", Out::Bool(true)),
                ("all_anchor_statuses", Out::Array(vec![q("proposed")])),
                ("human_review_performed", Out::Bool(false)),
            ]),
        ),
        ("provenance_ref", q(provenance_ref)),
        ("provenance_event_ref", q(event_id)),
        ("map_version", n(1)),
        ("supersedes_map_ref", Out::Null),
        (
            "authority_boundary",
            q(
                "This map records model-reviewed numbered-label start-page candidates and proposed whole-page addresses for one exact translation scan.",
            ),
        ),
        (
            "does_not_establish",
            Out::Array(vec![
                q("target_text"),
                q("exact_line_boundaries"),
                q("exact_passage_end_boundaries"),
                q("accepted_translation_text"),
                q("translation_correspondence"),
                q("translation_equivalence"),
                q("translation_quality"),
                q("textual_identity"),
                q("semantics"),
                q("rights_clearance"),
                q("canon_promotion"),
            ]),
        ),
    ]))
}
pub(super) fn event(
    pdf_entry: &Value,
    matched: usize,
    inventory_digest: &str,
    boundary_digest: &str,
    source_map_digest: &str,
    map_digest: &str,
    anchor_digest: &str,
    map_ref: &str,
    anchor_ref: &str,
    event_id: &str,
    event_at: &str,
) -> Result<Out> {
    Ok(o(vec![
        ("schema_version", q("tos_provenance_event_v1")),
        ("event_id", q(event_id)),
        ("event_type", q("segmentation")),
        ("started_at", q(event_at)),
        ("ended_at", q(event_at)),
        (
            "agent_refs",
            Out::Array(vec![
                q("model:codex"),
                q("software:python-standard-library"),
                q("software:poppler-26.01.0"),
            ]),
        ),
        (
            "inputs",
            Out::Array(vec![
                o(vec![
                    ("ref", q(s(&pdf_entry["file_id"])?)),
                    ("role", q("target-visible-translation-scan-witness")),
                    ("sha256", q(s(&pdf_entry["sha256"])?)),
                ]),
                o(vec![
                    ("ref", q(INVENTORY_PATH)),
                    ("role", q("tracked-text-free-target-resource-inventory")),
                    ("sha256", q(inventory_digest)),
                ]),
                o(vec![
                    ("ref", q(WORK_BOUNDARY_PATH)),
                    ("role", q("tracked-target-work-boundary")),
                    ("sha256", q(boundary_digest)),
                ]),
                o(vec![
                    ("ref", q(SOURCE_MAP_PATH)),
                    ("role", q("numbering-asymmetry-reference-only")),
                    ("sha256", q(source_map_digest)),
                ]),
            ]),
        ),
        (
            "outputs",
            Out::Array(vec![
                o(vec![
                    ("ref", q(map_ref)),
                    ("role", q("tracked-text-free-target-numbered-unit-page-map")),
                    ("sha256", q(map_digest)),
                ]),
                o(vec![
                    ("ref", q(anchor_ref)),
                    ("role", q("tracked-proposed-whole-page-target-anchors")),
                    ("sha256", q(anchor_digest)),
                ]),
            ]),
        ),
        (
            "method",
            o(vec![
                ("maker_type", q("mixed")),
                (
                    "name",
                    q("ordered-embedded-pdf-bbox-candidate-plus-source-visible-gap-review"),
                ),
                ("version", q("1")),
                ("artifact_digest", Out::Null),
                (
                    "runtime",
                    q("Python standard library XML parser; Poppler pdftotext 26.01.0"),
                ),
                ("device", q("abyss-machine")),
                (
                    "configuration",
                    o(vec![
                        ("work_page_range", Out::Array(vec![n(238), n(406)])),
                        ("expected_integer_units", n(296)),
                        (
                            "target_supplemental_numbered_units",
                            Out::Array(vec![q("65a"), q("73a")]),
                        ),
                        (
                            "source_only_nonmaterialized_numbered_units",
                            Out::Array(vec![q("237a")]),
                        ),
                        ("ordered_bbox_candidate_matches", n(matched as u64)),
                        (
                            "source_visible_gap_review_count",
                            n(PAGE_OVERRIDES.len() as u64),
                        ),
                        ("source_visible_ocr_disambiguation_count", n(1)),
                        ("human_repeat_performed", Out::Bool(false)),
                        ("source_text_included", Out::Bool(false)),
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
                q(
                    "All 298 addresses are proposed target numbered-label start-page candidates, not exact line or passage-end boundaries.",
                ),
                q(
                    "The embedded PDF text layer supplied disposable navigation candidates only and was not accepted as Russian text.",
                ),
                q(
                    "The scan-visible review was model-performed and has not been independently repeated by a human.",
                ),
                q(
                    "The target visibly materializes labels 1-296, 65a, and 73a; source-only 237a remains an explicit nonmaterialized asymmetry.",
                ),
                q(
                    "This event records target-unit structure. Source-to-target alignment, translation assessment and semantic claims retain their existing states.",
                ),
            ]),
        ),
        ("receipt_refs", Out::Array(vec![q(map_ref), q(anchor_ref)])),
        ("rights_basis_ref", q(RIGHTS_PATH)),
        ("event_version", n(1)),
        ("supersedes_event_ref", Out::Null),
    ]))
}
