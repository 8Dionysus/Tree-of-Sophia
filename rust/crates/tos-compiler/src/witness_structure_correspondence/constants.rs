pub(super) const SCHEMA_REF: &str =
    "https://tree-of-sophia.local/ToS/contracts/witness-structure-correspondence.schema.json";
pub(super) const ANCHOR_SET_SCHEMA_REF: &str =
    "https://tree-of-sophia.local/ToS/contracts/witness-structure-anchor-set.schema.json";
pub(super) const MAP_ID: &str =
    "tos.structure-map.friedrich-nietzsche.also-sprach-zarathustra.dta-parts-to-naumann-1893";
pub(super) const ANCHOR_SET_ID: &str = "tos.structure-anchor-set.friedrich-nietzsche.also-sprach-zarathustra.dta-parts-to-naumann-1893";
pub(super) const EVENT_ID: &str = "tos.event.structure-correspondence.friedrich-nietzsche.also-sprach-zarathustra.dta-parts-to-naumann-1893.2026-07-28";
pub(super) const ANCHOR_EVENT_ID: &str = "tos.event.structure-anchors.friedrich-nietzsche.also-sprach-zarathustra.dta-parts-to-naumann-1893.2026-07-28";
pub(super) const WORK_REF: &str = "tos.work.friedrich-nietzsche.also-sprach-zarathustra";
pub(super) const AUTHORITY_BOUNDARY: &str = "This record connects named structural starts and locator candidates to their source witnesses.";
pub(super) const ANCHOR_AUTHORITY_BOUNDARY: &str =
    "This set gives named structural-start candidates stable proposed addresses.";
pub(super) const NORMALIZATION: &str = "unicode-nfkc-casefold-alpha-token-sequence";
pub(super) const TARGET_EPUB_MANIFEST_REF: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/expressions/de-naumann-1893/editions/leipzig-c-g-naumann-1893/items/internet-archive-cornell-auto-epub/item.manifest.json";
pub(super) const TARGET_PDF_MANIFEST_REF: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/expressions/de-naumann-1893/editions/leipzig-c-g-naumann-1893/items/internet-archive-image-container-pdf/item.manifest.json";
pub(super) const OUTPUT_PATH: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/alignments/structure/naumann-1893-dta-parts/structure-correspondence.json";
pub(super) const ANCHOR_SET_PATH: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/alignments/structure/naumann-1893-dta-parts/structure-anchor-set.json";
pub(super) const ANCHOR_RECORDS_PATH: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/alignments/structure/naumann-1893-dta-parts/structure-anchors.jsonl";
pub(super) const PROVENANCE_PATH: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/alignments/structure/naumann-1893-dta-parts/provenance.jsonl";
pub(super) const DOES_NOT_ESTABLISH: &[&str] = &[
    "source_text",
    "exact_textual_identity",
    "edition_equivalence",
    "accepted_german",
    "translation_correspondence",
    "semantic_correspondence",
    "canon_promotion",
];
pub(super) const ANCHOR_DOES_NOT_ESTABLISH: &[&str] = &[
    "source_text",
    "exact_passage_boundary",
    "exact_textual_identity",
    "edition_equivalence",
    "accepted_german",
    "translation_correspondence",
    "semantic_correspondence",
    "rights_clearance",
    "canon_promotion",
];
