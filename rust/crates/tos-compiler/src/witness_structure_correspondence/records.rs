//! Ordered correspondence records retained from the authored source recipe.
use super::*;
pub(super) fn map(d: &Data) -> Result<Out> {
    Ok(o(vec![
        (
            "$schema",
            q(
                "https://tree-of-sophia.local/ToS/contracts/witness-structure-correspondence.schema.json",
            ),
        ),
        (
            "schema_version",
            q("tos_witness_structure_correspondence_v1"),
        ),
        (
            "correspondence_map_id",
            q(
                "tos.structure-map.friedrich-nietzsche.also-sprach-zarathustra.dta-parts-to-naumann-1893",
            ),
        ),
        (
            "work_ref",
            q("tos.work.friedrich-nietzsche.also-sprach-zarathustra"),
        ),
        ("correspondence_authority", q("mechanical_candidate_only")),
        ("source_text_included", Out::Bool(false)),
        ("source_parts", Out::Array(d.source_parts.clone())),
        (
            "target_witnesses",
            o(vec![("epub", d.epub.clone()), ("pdf", d.pdf.clone())]),
        ),
        (
            "scan_resource_relation",
            o(vec![
                ("state", q("mechanical_enumeration_candidate")),
                (
                    "epub_member_page_range",
                    o(vec![("first", n(0)), ("last", n(528))]),
                ),
                (
                    "pdf_page_index_range",
                    o(vec![("first", n(1)), ("last", n(529))]),
                ),
                ("formula", q("pdf_page_index = epub_member_page_number + 1")),
                (
                    "basis",
                    Out::Array(vec![
                        q("complete contiguous EPUB page-member enumeration"),
                        q("complete contiguous PDF page-index enumeration"),
                        q("shared Internet Archive source-item lineage"),
                    ]),
                ),
                ("exact_content_identity_claimed", Out::Bool(false)),
            ]),
        ),
        (
            "method",
            o(vec![
                (
                    "name",
                    q("named-division-heading-and-context-correspondence"),
                ),
                ("version", q("1")),
                (
                    "normalization",
                    q("unicode-nfkc-casefold-alpha-token-sequence"),
                ),
                ("selection_unit", q("named_primary_tei_division_start")),
                ("source_context_token_limit", n(160)),
                ("target_window_page_count", n(2)),
                (
                    "score",
                    o(vec![
                        ("context_cosine_weight", Out::Float(0.8)),
                        ("heading_token_coverage_weight", Out::Float(0.2)),
                    ]),
                ),
                ("no_llm", Out::Bool(true)),
                ("no_source_text_emitted", Out::Bool(true)),
            ]),
        ),
        ("part_routes", Out::Array(d.routes.clone())),
        ("correspondences", Out::Array(d.rows.clone())),
        (
            "summary",
            o(vec![
                ("correspondence_count", n(d.rows.len() as u64)),
                ("part_counts", d.part_counts.clone()),
                ("match_mode_counts", d.mode_counts.clone()),
                ("monotonic_within_each_part", Out::Bool(true)),
            ]),
        ),
        ("provenance_ref", q(PROVENANCE_PATH)),
        (
            "provenance_event_ref",
            q(
                "tos.event.structure-correspondence.friedrich-nietzsche.also-sprach-zarathustra.dta-parts-to-naumann-1893.2026-07-28",
            ),
        ),
        ("map_version", n(1)),
        ("supersedes_map_ref", Out::Null),
        (
            "authority_boundary",
            q(
                "This record connects named structural starts and locator candidates to their source witnesses.",
            ),
        ),
        (
            "does_not_establish",
            Out::Array(vec![
                q("source_text"),
                q("exact_textual_identity"),
                q("edition_equivalence"),
                q("accepted_german"),
                q("translation_correspondence"),
                q("semantic_correspondence"),
                q("canon_promotion"),
            ]),
        ),
    ]))
}
pub(super) fn anchor_set(d: &Data) -> Result<Out> {
    Ok(o(vec![
        (
            "$schema",
            q(
                "https://tree-of-sophia.local/ToS/contracts/witness-structure-anchor-set.schema.json",
            ),
        ),
        ("schema_version", q("tos_witness_structure_anchor_set_v1")),
        (
            "anchor_set_id",
            q(
                "tos.structure-anchor-set.friedrich-nietzsche.also-sprach-zarathustra.dta-parts-to-naumann-1893",
            ),
        ),
        (
            "work_ref",
            q("tos.work.friedrich-nietzsche.also-sprach-zarathustra"),
        ),
        (
            "correspondence_map",
            o(vec![("ref", q(OUTPUT_PATH)), ("sha256", q(&d.map_digest))]),
        ),
        (
            "anchor_records",
            o(vec![
                ("ref", q(ANCHOR_RECORDS_PATH)),
                ("sha256", q(&d.anchor_digest)),
            ]),
        ),
        ("anchor_authority", q("proposed_structural_address_only")),
        ("source_text_included", Out::Bool(false)),
        ("bindings", Out::Array(d.bindings.clone())),
        (
            "summary",
            o(vec![
                ("correspondence_count", n(d.rows.len() as u64)),
                ("anchor_count", n(d.anchors.len() as u64)),
                ("anchors_per_correspondence", n(3)),
                (
                    "role_counts",
                    o(vec![
                        ("source_tei", n(d.rows.len() as u64)),
                        ("target_epub", n(d.rows.len() as u64)),
                        ("target_pdf", n(d.rows.len() as u64)),
                    ]),
                ),
                ("all_anchor_statuses", Out::Array(vec![q("proposed")])),
            ]),
        ),
        ("provenance_ref", q(PROVENANCE_PATH)),
        (
            "provenance_event_ref",
            q(
                "tos.event.structure-anchors.friedrich-nietzsche.also-sprach-zarathustra.dta-parts-to-naumann-1893.2026-07-28",
            ),
        ),
        ("anchor_set_version", n(1)),
        ("supersedes_anchor_set_ref", Out::Null),
        (
            "authority_boundary",
            q("This set gives named structural-start candidates stable proposed addresses."),
        ),
        (
            "does_not_establish",
            Out::Array(vec![
                q("source_text"),
                q("exact_passage_boundary"),
                q("exact_textual_identity"),
                q("edition_equivalence"),
                q("accepted_german"),
                q("translation_correspondence"),
                q("semantic_correspondence"),
                q("rights_clearance"),
                q("canon_promotion"),
            ]),
        ),
    ]))
}
pub(super) fn map_event(d: &Data) -> Result<Out> {
    Ok(o(vec![
        ("schema_version", q("tos_provenance_event_v1")),
        (
            "event_id",
            q(
                "tos.event.structure-correspondence.friedrich-nietzsche.also-sprach-zarathustra.dta-parts-to-naumann-1893.2026-07-28",
            ),
        ),
        ("event_type", q("alignment")),
        ("started_at", q(d.event_at)),
        ("ended_at", q(d.event_at)),
        (
            "agent_refs",
            Out::Array(vec![
                q("model:codex"),
                q("software:python-standard-library"),
            ]),
        ),
        ("inputs", Out::Array(d.inputs.clone())),
        (
            "outputs",
            Out::Array(vec![o(vec![
                ("ref", q(OUTPUT_PATH)),
                (
                    "role",
                    q("tracked_text_free_structure_correspondence_candidate"),
                ),
                ("sha256", q(&d.map_digest)),
            ])]),
        ),
        (
            "method",
            o(vec![
                ("maker_type", q("software")),
                (
                    "name",
                    q("named-division-heading-and-context-correspondence"),
                ),
                ("version", q("1")),
                ("artifact_digest", Out::Null),
                (
                    "runtime",
                    q("Python standard library XML, ZIP, and HTML parsers"),
                ),
                ("device", q("host-cpu")),
                (
                    "configuration",
                    o(vec![
                        ("correspondence_count", n(d.rows.len() as u64)),
                        ("source_text_included", Out::Bool(false)),
                        ("candidate_only", Out::Bool(true)),
                        (
                            "normalization",
                            q("unicode-nfkc-casefold-alpha-token-sequence"),
                        ),
                    ]),
                ),
                (
                    "prompt_or_instruction_ref",
                    q("ToS/source-witnesses/README.md"),
                ),
            ]),
        ),
        ("status", q("completed_with_warnings")),
        (
            "warnings",
            Out::Array(vec![
                q(
                    "Normalized heading and context agreement is a locator candidate, not exact textual identity or edition equivalence.",
                ),
                q(
                    "The EPUB-to-PDF page formula is a mechanical enumeration candidate and not a content-identity assertion.",
                ),
                q(
                    "This event records structural correspondence; German-text assessment, translation, semantics and canon retain their existing states.",
                ),
            ]),
        ),
        ("receipt_refs", Out::Array(vec![q(OUTPUT_PATH)])),
        ("rights_basis_ref", Out::Null),
        ("event_version", n(1)),
        ("supersedes_event_ref", Out::Null),
    ]))
}
pub(super) fn anchor_event(d: &Data) -> Result<Out> {
    Ok(o(vec![
        ("schema_version", q("tos_provenance_event_v1")),
        (
            "event_id",
            q(
                "tos.event.structure-anchors.friedrich-nietzsche.also-sprach-zarathustra.dta-parts-to-naumann-1893.2026-07-28",
            ),
        ),
        ("event_type", q("segmentation")),
        ("started_at", q(d.event_at)),
        ("ended_at", q(d.event_at)),
        (
            "agent_refs",
            Out::Array(vec![
                q("model:codex"),
                q("software:python-standard-library"),
            ]),
        ),
        (
            "inputs",
            Out::Array(vec![o(vec![
                ("ref", q(OUTPUT_PATH)),
                (
                    "role",
                    q("tracked_text_free_structure_correspondence_candidate"),
                ),
                ("sha256", q(&d.map_digest)),
            ])]),
        ),
        (
            "outputs",
            Out::Array(vec![
                o(vec![
                    ("ref", q(ANCHOR_SET_PATH)),
                    ("role", q("tracked_proposed_structure_anchor_set")),
                    ("sha256", q(&d.set_digest)),
                ]),
                o(vec![
                    ("ref", q(ANCHOR_RECORDS_PATH)),
                    ("role", q("tracked_proposed_source_anchor_records")),
                    ("sha256", q(&d.anchor_digest)),
                ]),
            ]),
        ),
        (
            "method",
            o(vec![
                ("maker_type", q("software")),
                (
                    "name",
                    q("structure-correspondence-to-stable-source-anchors"),
                ),
                ("version", q("1")),
                ("artifact_digest", Out::Null),
                ("runtime", q("Python standard library")),
                ("device", q("host-cpu")),
                (
                    "configuration",
                    o(vec![
                        ("correspondence_count", n(d.rows.len() as u64)),
                        ("anchor_count", n(d.anchors.len() as u64)),
                        ("anchors_per_correspondence", n(3)),
                        ("source_text_included", Out::Bool(false)),
                        ("candidate_only", Out::Bool(true)),
                    ]),
                ),
                (
                    "prompt_or_instruction_ref",
                    q("ToS/doctrine/CORPUS_FOUNDATION.md"),
                ),
            ]),
        ),
        ("status", q("completed_with_warnings")),
        (
            "warnings",
            Out::Array(vec![
                q(
                    "Every emitted anchor remains proposed and identifies only a structural path, exact container member, or whole scan page.",
                ),
                q(
                    "A three-way anchor binding does not establish an exact passage boundary, textual identity, or edition equivalence.",
                ),
                q(
                    "This event records structural correspondence; rights, German-text assessment, translation, semantics and canon retain their existing states.",
                ),
            ]),
        ),
        (
            "receipt_refs",
            Out::Array(vec![q(ANCHOR_SET_PATH), q(ANCHOR_RECORDS_PATH)]),
        ),
        ("rights_basis_ref", Out::Null),
        ("event_version", n(1)),
        ("supersedes_event_ref", Out::Null),
    ]))
}
pub(super) fn anchor(
    anchor_id: &str,
    w: &Value,
    selector: Out,
    method: &str,
    correspondence_id: &str,
) -> Result<Out> {
    Ok(o(vec![
        ("schema_version", q("tos_source_anchor_v1")),
        ("anchor_id", q(anchor_id)),
        ("item_id", v(&w["item_ref"])?),
        ("file_id", v(&w["file_ref"])?),
        ("file_sha256", v(&w["file_sha256"])?),
        ("passage_id", Out::Null),
        ("selectors", Out::Array(vec![selector])),
        (
            "selector_method",
            o(vec![
                ("maker_type", q("software")),
                ("method", q(method)),
                ("version", q("1")),
                (
                    "configuration_ref",
                    q(format!("{OUTPUT_PATH}#{correspondence_id}")),
                ),
            ]),
        ),
        ("status", q("proposed")),
        (
            "provenance_event_ref",
            q(
                "tos.event.structure-anchors.friedrich-nietzsche.also-sprach-zarathustra.dta-parts-to-naumann-1893.2026-07-28",
            ),
        ),
        ("anchor_version", n(1)),
        ("supersedes_anchor_ref", Out::Null),
        ("review_ref", Out::Null),
    ]))
}
