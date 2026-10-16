//! Ordered source structure records, preserving authored recipe field order.
use super::*;
pub(super) fn map(d: &Data) -> Result<Out> {
    Ok(o(vec![
        (
            "$schema",
            q(
                "https://tree-of-sophia.local/ToS/contracts/hierarchical-source-numbered-unit-page-map.schema.json",
            ),
        ),
        (
            "schema_version",
            q("tos_hierarchical_source_numbered_unit_page_map_v1"),
        ),
        ("map_id", v(&d.config["map_id"])?),
        ("work_ref", v(&d.config["work_ref"])?),
        ("expression_ref", v(&d.config["expression_ref"])?),
        ("edition_ref", v(&d.address_manifest["embodiment_ref"])?),
        ("address_witness", d.address_binding.clone()),
        ("navigation_witness", d.navigation_binding.clone()),
        (
            "navigation_relation",
            o(vec![
                ("relation_scope", v(&d.config["navigation_relation_scope"])?),
                (
                    "address_page_from_navigation_page_offset",
                    v(&d.config["navigation_offset"])?,
                ),
                (
                    "source_visible_compared_address_pages",
                    v(&d.config["compared_address_pages"])?,
                ),
                ("full_container_page_identity_claimed", Out::Bool(false)),
                ("textual_identity_claimed", Out::Bool(false)),
            ]),
        ),
        (
            "represented_page_range",
            o(vec![
                ("start_page", v(&d.config["represented_start_page"])?),
                ("end_page", v(&d.config["represented_end_page"])?),
                ("basis", v(&d.config["represented_basis"])?),
            ]),
        ),
        ("work_boundary", d.work_boundary.clone()),
        (
            "map_authority",
            q("machine_candidates_with_bounded_model_source_visible_gap_review_only"),
        ),
        ("source_text_included", Out::Bool(false)),
        (
            "method",
            o(vec![
                (
                    "name",
                    q(
                        "series-scoped-fixed-page-number-candidate-verification-plus-source-visible-gap-review",
                    ),
                ),
                ("version", q("1")),
                ("maker_type", q("mixed")),
                ("local_payloads_read", Out::Bool(true)),
                (
                    "candidate_verification_profiles",
                    Out::Array(vec![v(&d.config["candidate_profile"])?]),
                ),
                (
                    "numeric_candidate_normalizations",
                    Out::Array(vec![
                        q("remove non-ASCII alphanumeric characters"),
                        q("ASCII casefold"),
                        q("map i l x to 1, o to 0, s to 5, z to 2"),
                        q("join at most two adjacent same-baseline PDF short-line fragments"),
                    ]),
                ),
                ("series_trials", Out::Array(d.trials.to_vec())),
                (
                    "source_visible_review",
                    o(vec![
                        ("maker_type", q("model")),
                        ("human_repeat_performed", Out::Bool(false)),
                        (
                            "reviewed_address_pages",
                            Out::Array(d.reviewed.iter().map(|v| n(*v)).collect()),
                        ),
                        (
                            "override_unit_refs",
                            Out::Array(d.overrides.iter().map(q).collect()),
                        ),
                    ]),
                ),
                ("source_to_target_text_compared", Out::Bool(false)),
                ("translation_alignment_inferred", Out::Bool(false)),
                ("semantic_matching_used", Out::Bool(false)),
                ("no_source_text_emitted", Out::Bool(true)),
            ]),
        ),
        ("series", Out::Array(d.series.to_vec())),
        (
            "summary",
            o(vec![
                ("series_count", n(d.series.len() as u64)),
                ("numbered_unit_count", n(d.total as u64)),
                ("machine_number_candidate_count", n(d.machine as u64)),
                (
                    "source_visible_override_unit_count",
                    n(d.overrides.len() as u64),
                ),
                (
                    "source_visible_reviewed_page_count",
                    n(d.reviewed.len() as u64),
                ),
                (
                    "exact_start_page_candidates_materialized",
                    n(d.total as u64),
                ),
                ("unresolved_unit_count", n(0)),
                ("start_pages_monotonic_within_series", Out::Bool(true)),
                ("all_anchor_statuses", Out::Array(vec![q("proposed")])),
                ("human_review_performed", Out::Bool(false)),
                ("accepted_german_unit_count", n(0)),
            ]),
        ),
        ("provenance_ref", q(&d.paths["provenance"])),
        ("provenance_event_ref", v(&d.config["event_id"])?),
        ("map_version", n(1)),
        ("supersedes_map_ref", Out::Null),
        (
            "authority_boundary",
            q(
                "This map binds German start-page candidates to exact scans and independently named, series-qualified printed number labels. Embedded or provider OCR supports navigation; explicit OCR gaps receive model-visible review. Anchors retain proposed whole-page scope and OCR retains its recorded assessment status.",
            ),
        ),
        (
            "does_not_establish",
            Out::Array(vec![
                q("source_text"),
                q("exact_line_boundaries"),
                q("exact_passage_end_boundaries"),
                q("accepted_german"),
                q("provider_ocr_correctness"),
                q("full_container_page_identity"),
                q("source_to_target_passage_alignment"),
                q("translation_correspondence"),
                q("translation_equivalence"),
                q("translation_quality"),
                q("textual_identity"),
                q("edition_equivalence"),
                q("semantics"),
                q("rights_clearance"),
                q("eligible_target_unit"),
                q("target_gold"),
                q("canon_promotion"),
            ]),
        ),
    ]))
}
pub(super) fn event(d: &Data) -> Result<Out> {
    Ok(o(vec![
        ("schema_version", q("tos_provenance_event_v1")),
        ("event_id", v(&d.config["event_id"])?),
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
        ("inputs", Out::Array(d.inputs.to_vec())),
        (
            "outputs",
            Out::Array(vec![
                o(vec![
                    ("ref", q(&d.paths["map"])),
                    (
                        "role",
                        q("tracked-text-free-hierarchical-source-numbered-unit-page-map"),
                    ),
                    ("sha256", q(d.map_digest)),
                ]),
                o(vec![
                    ("ref", q(&d.paths["anchors"])),
                    (
                        "role",
                        q("tracked-proposed-whole-page-hierarchical-source-anchors"),
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
                        "series-scoped-fixed-page-number-candidate-verification-plus-source-visible-gap-review",
                    ),
                ),
                ("version", q("1")),
                ("artifact_digest", Out::Null),
                (
                    "runtime",
                    q(format!(
                        "Python standard-library XML parser; Poppler pdftotext {}",
                        PDFTOTEXT_VERSION
                    )),
                ),
                ("device", q("abyss-machine")),
                (
                    "configuration",
                    o(vec![
                        (
                            "represented_page_range",
                            Out::Array(vec![
                                v(&d.config["represented_start_page"])?,
                                v(&d.config["represented_end_page"])?,
                            ]),
                        ),
                        ("series_count", n(d.series.len() as u64)),
                        ("numbered_unit_count", n(d.total as u64)),
                        ("machine_number_candidate_count", n(d.machine as u64)),
                        (
                            "source_visible_override_unit_count",
                            n(d.overrides.len() as u64),
                        ),
                        (
                            "source_visible_reviewed_page_count",
                            n(d.reviewed.len() as u64),
                        ),
                        ("navigation_offset", v(&d.config["navigation_offset"])?),
                        (
                            "navigation_relation_scope",
                            v(&d.config["navigation_relation_scope"])?,
                        ),
                        ("human_repeat_performed", Out::Bool(false)),
                        ("source_text_included", Out::Bool(false)),
                        ("accepted_german_unit_count", n(0)),
                        ("source_to_target_text_compared", Out::Bool(false)),
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
                    "All {} addresses are proposed series-qualified German source number-label start-page candidates, not exact line or passage-end boundaries.",
                    d.total
                )),
                q(
                    "Embedded or provider OCR supplied disposable navigation only and was not accepted as German text.",
                ),
                q(format!(
                    "The model visibly checked {} address pages for {} OCR-gap labels; no human repeat exists.",
                    d.reviewed.len(),
                    d.overrides.len()
                )),
                q(
                    "Any navigation/address page relation is bounded to the declared series evidence and does not establish whole-container page or textual identity.",
                ),
                q(
                    "This event records source structure. Alignment, translation, German-text assessment, semantics, eligibility, target gold, publication, human work and canon retain their existing states.",
                ),
            ]),
        ),
        (
            "receipt_refs",
            Out::Array(vec![q(&d.paths["map"]), q(&d.paths["anchors"])]),
        ),
        ("rights_basis_ref", q(d.rights_ref)),
        ("event_version", n(1)),
        ("supersedes_event_ref", Out::Null),
    ]))
}
